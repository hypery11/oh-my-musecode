//! The end-to-end scenarios against the RELEASE binary and the pinned host:
//! Phase 1 (PLAN.md "e2e scenarios" 1–10, plus 11–27) and the Phase 2 / 3
//! surface (28 the MCP server through the mock provider, 29 memory, 30
//! skill routing through the mock provider, 31 `scripts/release.sh`, 32
//! `install.sh` against a local HTTP server).
//!
//! Every scenario runs `target/release/omm` (built once, `cargo build -p omm
//! --release`, or `$OMM_E2E_BIN`) as a subprocess inside a fresh throwaway
//! `HOME` / `XDG_CONFIG_HOME` / `XDG_DATA_HOME`, with a fresh git workspace as
//! its cwd (never the repository — the host scaffolds files into its cwd) and
//! the repository root as the marketplace source (`omm install --source
//! <repo>`). The host is reached only through `omm_host::Invoker` (sandboxed,
//! `--provider echo` at most, never a login). Skipped with a message when
//! `OMM_MUSE_BIN` is unset. Scenarios are serialised through one mutex: the
//! host's bootstrap-trace writer is lossy under concurrent bootstraps
//! (host-reality.md P1 "experimental gates") and scenario 9 times the hook
//! dispatcher.
//!
//! R5 (byte-identical) as measured here — `snapshot()` records `{path →
//! sha256 | dir | symlink target, mode}` of `$XDG_CONFIG_HOME/**` and
//! `$XDG_DATA_HOME/muse/**`. The baseline is taken after the host has run two
//! read-only verbs once (`skills list`, `plugins list`): the host lazily
//! materialises its own bundled-skill store on first use, which is not omm's
//! footprint. After uninstall, `$XDG_CONFIG_HOME/omm` must not exist; the
//! uninstall preview must name, UNCONDITIONALLY, the host-owned residue of
//! `host_owned_residue` (Gate 1 decision: the session logs, the tracing
//! lanes, the host's plugin store, the two startup locks, the managed
//! store's metadata, and the `getpwuid` session-name-authority dir) and
//! nothing else inside the sandbox; the snapshot scope EXCLUDES exactly those
//! paths and everything else under both roots must be byte- AND
//! mode-identical (`check_r5`).

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

use omm_host::host_reality as hr;
use omm_host::probe::{self, PluginInspect, SkillsListOptions};
use omm_host::{Invoker, Sandbox};

// ---------------------------------------------------------------------------
// harness
// ---------------------------------------------------------------------------

static SERIAL: Mutex<()> = Mutex::new(());
static RELEASE_BIN: OnceLock<PathBuf> = OnceLock::new();

/// The repository this crate was built in (`CARGO_MANIFEST_DIR/../..`).
fn repo_root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".."))
        .expect("repo root")
}

/// `target/release/omm`, built once per test process (`$OMM_E2E_BIN` overrides).
fn release_bin() -> &'static Path {
    RELEASE_BIN.get_or_init(|| {
        if let Some(p) = std::env::var_os("OMM_E2E_BIN").filter(|v| !v.is_empty()) {
            return PathBuf::from(p);
        }
        let cargo = std::env::var_os("CARGO")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO")));
        let out = Command::new(&cargo)
            .args([
                "build",
                "-p",
                "omm",
                "--release",
                "--message-format=json-render-diagnostics",
            ])
            .current_dir(repo_root())
            .stdin(Stdio::null())
            .output()
            .expect("run cargo build --release");
        assert!(
            out.status.success(),
            "cargo build -p omm --release failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let is_bin = v["target"]["kind"]
                .as_array()
                .map(|k| k.iter().any(|x| x == "bin"))
                .unwrap_or(false);
            if v["reason"] == "compiler-artifact" && v["target"]["name"] == "omm" && is_bin {
                if let Some(exe) = v["executable"].as_str() {
                    return PathBuf::from(exe);
                }
            }
        }
        panic!("cargo build --release emitted no `omm` bin artifact:\n{stdout}");
    })
}

fn host_bin() -> Option<PathBuf> {
    std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// The test process's `PATH` with `bin`'s directory first.
fn path_with_bin_dir(bin: &Path) -> std::ffi::OsString {
    let mut dirs: Vec<PathBuf> = bin.parent().map(Path::to_path_buf).into_iter().collect();
    if let Some(p) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&p));
    }
    std::env::join_paths(dirs).unwrap_or_default()
}

/// One subprocess run of the release binary.
struct Run {
    code: i32,
    stdout: String,
    stderr: String,
    /// `stdout` parsed as one JSON document (`Null` when it is not one).
    json: Value,
}

impl Run {
    fn ctx(&self) -> String {
        format!(
            "rc={} stdout={} stderr={}",
            self.code,
            self.stdout.trim(),
            self.stderr.trim()
        )
    }
}

/// `path → "file sha256=<hex> mode=<octal>" | "dir mode=…" | "symlink -> <target> mode=…"`,
/// keyed relative to the sandbox root (`config/…`, `data/muse/…`).
type Snapshot = BTreeMap<String, String>;

/// Findings a scenario collects instead of stopping at the first one, so one
/// run reports everything the orchestrator asked about.
#[derive(Default)]
struct Findings(Vec<String>);

impl Findings {
    fn check(&mut self, ok: bool, msg: impl FnOnce() -> String) {
        if !ok {
            let m = msg();
            eprintln!("FINDING: {m}");
            self.0.push(m);
        }
    }
    fn finish(self, scenario: &str) {
        assert!(
            self.0.is_empty(),
            "{scenario}: {} finding(s):\n- {}",
            self.0.len(),
            self.0.join("\n- ")
        );
    }
}

/// One scenario's world: the sandbox, the workspace, the two binaries.
struct E2e {
    _guard: MutexGuard<'static, ()>,
    tmp: Option<TempDir>,
    sb: Sandbox,
    /// The XDG roots sit at HOME's defaults (`~/.config`, `~/.local/share`)
    /// instead of beside it — what a process that sees only `HOME` (the
    /// host-spawned `omm mcp`, scenario 28) resolves to.
    home_layout: bool,
    /// The git workspace, cwd of every omm run.
    ws: PathBuf,
    muse: PathBuf,
    omm: PathBuf,
    repo: PathBuf,
}

macro_rules! e2e_or_skip {
    () => {
        e2e_or_skip!(E2e::new())
    };
    (home) => {
        e2e_or_skip!(E2e::new_home_layout())
    };
    ($ctor:expr) => {
        match $ctor {
            Some(h) => h,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

impl E2e {
    fn new() -> Option<E2e> {
        E2e::new_with(false)
    }

    /// The sandbox with the XDG roots at HOME's defaults: the host's 16-key
    /// scrubbed child environment carries `HOME` and `PATH` but no `XDG_*`
    /// (host-reality.md "Trust lifecycle"; `crates/omm/src/cmd/mcp.rs`), so
    /// a server the host spawns must find the installer's roots through
    /// `HOME` alone.
    fn new_home_layout() -> Option<E2e> {
        E2e::new_with(true)
    }

    fn new_with(home_layout: bool) -> Option<E2e> {
        let muse = host_bin()?;
        let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let omm = release_bin().to_path_buf();
        let tmp = tempfile::Builder::new()
            .prefix("omm-e2e-")
            .tempdir()
            .expect("temp dir");
        let root = std::fs::canonicalize(tmp.path()).expect("canonical temp dir");
        let sb = if home_layout {
            let sandbox = root.join("sandbox");
            let home = sandbox.join("home");
            let sb = Sandbox {
                root: sandbox,
                config_home: home.join(".config"),
                data_home: home.join(".local").join("share"),
                home,
            };
            for d in [&sb.home, &sb.config_home, &sb.data_home] {
                std::fs::create_dir_all(d).expect("sandbox dir");
            }
            sb
        } else {
            Sandbox::create(&root.join("sandbox")).expect("sandbox")
        };
        let ws = root.join("ws");
        std::fs::create_dir_all(&ws).expect("workspace");
        let git = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&ws)
            .stdin(Stdio::null())
            .output();
        if !git.map(|o| o.status.success()).unwrap_or(false) {
            std::fs::create_dir_all(ws.join(".git")).expect(".git");
        }
        Some(E2e {
            _guard: guard,
            tmp: Some(tmp),
            sb,
            home_layout,
            ws,
            muse,
            omm,
            repo: repo_root(),
        })
    }

    /// Entries of `$HOME` beyond what the layout itself puts there.
    fn foreign_home_entries(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.sb.home)
            .expect("home")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| !(self.home_layout && (n == ".config" || n == ".local")))
            .collect();
        names.sort();
        names
    }

    fn config_root(&self) -> PathBuf {
        self.sb.config_home.join("muse")
    }
    fn omm_root(&self) -> PathBuf {
        self.sb.config_home.join("omm")
    }
    fn data_root(&self) -> PathBuf {
        self.sb.data_home.join("muse")
    }
    fn settings_path(&self) -> PathBuf {
        self.config_root().join("settings.json")
    }
    fn ledger_path(&self) -> PathBuf {
        self.omm_root().join("omm.lock.json")
    }
    fn repo_str(&self) -> &str {
        self.repo.to_str().expect("utf-8 repo path")
    }

    /// The host, sandboxed, with the workspace as cwd (the ONLY way to reach it).
    fn invoker(&self) -> Invoker {
        Invoker::new(&self.muse).sandboxed(&self.sb).cwd(&self.ws)
    }

    fn inspect(&self) -> Result<PluginInspect, omm_host::HostError> {
        probe::plugins_inspect(&self.invoker(), "oh-my-musecode")
    }

    /// Skill ids the host lists for `source` (`plugin` / `user`).
    fn skills_listed(&self, source: &str) -> BTreeSet<String> {
        let list = probe::skills_list(
            &self.invoker(),
            &SkillsListOptions {
                source: Some(source.to_string()),
                ..SkillsListOptions::default()
            },
        )
        .expect("skills list --json");
        list.skills.into_iter().map(|s| s.id).collect()
    }

    fn marketplace_names(&self) -> Vec<String> {
        let out = self
            .invoker()
            .run_json(&["plugins", "marketplace", "list", "--json"])
            .expect("marketplace list");
        out.json["marketplaces"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn installed_plugin_ids(&self) -> Vec<String> {
        let out = self
            .invoker()
            .run_json(&["plugins", "list", "--json"])
            .expect("plugins list");
        out.json["plugins"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r["record"]["id"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Two read-only host verbs, so the host's first-use materialisation of
    /// its own stores is in the baseline (see the module docs).
    fn warm_up_host(&self) {
        let inv = self.invoker();
        inv.run(&["skills", "list", "--json"])
            .expect("skills list")
            .expect_ok()
            .expect("skills list ok");
        inv.run(&["plugins", "list", "--json"])
            .expect("plugins list")
            .expect_ok()
            .expect("plugins list ok");
    }

    fn baseline(&self) -> Snapshot {
        self.warm_up_host();
        self.snapshot()
    }

    /// The omm binary in the sandbox: a clean environment, the workspace as
    /// cwd, the binary's own directory first on PATH — the host spawns
    /// `omm hook …` and `omm mcp` through the PATH it inherits, and doctor
    /// D15 is critical when that PATH holds no `omm`.
    fn omm_cmd(&self, args: &[&str], cwd: &Path) -> Command {
        let mut cmd = Command::new(&self.omm);
        cmd.args(args)
            .env_clear()
            .env("PATH", path_with_bin_dir(&self.omm))
            .env("HOME", &self.sb.home)
            .env("XDG_CONFIG_HOME", &self.sb.config_home)
            .env("XDG_DATA_HOME", &self.sb.data_home)
            .env("OMM_MUSE_BIN", &self.muse)
            .env("MUSE_NO_AUTO_UPDATE", "1")
            .env("TMPDIR", std::env::temp_dir())
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        cmd
    }

    fn run_cmd(mut cmd: Command) -> Run {
        let out = cmd.output().expect("run omm");
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let json = serde_json::from_str::<Value>(stdout.trim()).unwrap_or(Value::Null);
        Run {
            code: out.status.code().unwrap_or(-1),
            stdout,
            stderr,
            json,
        }
    }

    /// `omm <args>` verbatim.
    fn omm_raw(&self, args: &[&str]) -> Run {
        E2e::run_cmd(self.omm_cmd(args, &self.ws))
    }

    /// `omm <args> --json --yes`.
    fn omm(&self, args: &[&str]) -> Run {
        let mut argv = args.to_vec();
        argv.push("--json");
        argv.push("--yes");
        self.omm_raw(&argv)
    }

    /// `omm <args> --json --yes` with extra environment.
    fn omm_env(&self, args: &[&str], yes: bool, env: &[(&str, &str)]) -> Run {
        let mut argv = args.to_vec();
        argv.push("--json");
        if yes {
            argv.push("--yes");
        }
        let mut cmd = self.omm_cmd(&argv, &self.ws);
        for (k, v) in env {
            cmd.env(k, v);
        }
        E2e::run_cmd(cmd)
    }

    fn install(&self) -> Run {
        self.omm(&["install", "--source", self.repo_str()])
    }

    /// `omm doctor [--fast] --json`; the parsed report.
    fn doctor(&self, fast: bool) -> Run {
        let mut argv = vec!["doctor"];
        if fast {
            argv.push("--fast");
        }
        self.omm(&argv)
    }

    /// `omm hook <name>` with the host's scrubbed hook environment (HOME + PATH).
    fn hook(&self, name: &str, payload: &[u8], home: Option<&Path>) -> (Run, Duration) {
        let mut cmd = Command::new(&self.omm);
        cmd.args(["hook", name])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .current_dir(&self.ws)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(h) = home {
            cmd.env("HOME", h);
        }
        let started = Instant::now();
        let mut child = cmd.spawn().expect("spawn omm hook");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(payload)
            .expect("write payload");
        let out = child.wait_with_output().expect("wait");
        let elapsed = started.elapsed();
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let json = serde_json::from_str::<Value>(stdout.trim()).unwrap_or(Value::Null);
        (
            Run {
                code: out.status.code().unwrap_or(-1),
                stdout,
                stderr,
                json,
            },
            elapsed,
        )
    }

    fn ledger(&self) -> Option<Value> {
        std::fs::read(self.ledger_path())
            .ok()
            .map(|b| serde_json::from_slice(&b).expect("ledger is JSON"))
    }

    /// The skill ids a ledger records for the managed store (`muse-skills-install`
    /// entries under `skills/<id>/…`), sorted; empty without a ledger.
    fn ledgered_skill_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .ledger()
            .and_then(|l| l["entries"].as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter(|e| e["mechanism"] == "muse-skills-install")
            .filter_map(|e| e["path"].as_str().map(str::to_string))
            .filter_map(|p| {
                p.strip_prefix("skills/")
                    .and_then(|r| r.split_once('/'))
                    .map(|(id, _)| id.to_string())
            })
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Drop `id` from the host's store lockfile (`skills/.muse/lock.json`):
    /// the state an install killed inside `muse skills install` leaves —
    /// the directory copied, the lockfile not yet written, the host listing
    /// the skill with no provenance and refusing to uninstall it.
    fn drop_lock_entry(&self, id: &str) {
        let lock = self
            .config_root()
            .join("skills")
            .join(".muse")
            .join("lock.json");
        let mut v: Value = serde_json::from_slice(&std::fs::read(&lock).expect("lock.json"))
            .expect("lock.json is JSON");
        assert!(
            v["skills"]
                .as_object_mut()
                .expect("skills")
                .remove(id)
                .is_some(),
            "{id} is not in the lockfile"
        );
        std::fs::write(&lock, serde_json::to_vec_pretty(&v).expect("json")).expect("lock.json");
    }

    /// Whether the host's store lockfile lists `id`.
    fn lock_lists(&self, id: &str) -> bool {
        std::fs::read(
            self.config_root()
                .join("skills")
                .join(".muse")
                .join("lock.json"),
        )
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|v| v["skills"].get(id).is_some())
        .unwrap_or(false)
    }

    /// Drop every ledger entry whose path starts with `prefix` — the ledger
    /// an install killed before that step's save would have.
    fn drop_ledger_entries(&self, prefix: &str) {
        let mut v = self.ledger().expect("ledger");
        let entries = v["entries"].as_array_mut().expect("entries");
        let before = entries.len();
        entries.retain(|e| !e["path"].as_str().unwrap_or("").starts_with(prefix));
        assert!(entries.len() < before, "no ledger entry under {prefix}");
        std::fs::write(
            self.ledger_path(),
            serde_json::to_vec_pretty(&v).expect("json"),
        )
        .expect("ledger");
    }

    /// `{relative path → sha256}` of every regular file under `dir`.
    fn tree_digest(dir: &Path) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("read_dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    let rel = p
                        .strip_prefix(dir)
                        .expect("under dir")
                        .display()
                        .to_string();
                    out.insert(
                        rel,
                        omm_host::fsx::sha256_bytes(&std::fs::read(&p).expect("read")),
                    );
                }
            }
        }
        out
    }

    /// `omm-*` directories in the managed store, sorted.
    fn store_skill_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = std::fs::read_dir(self.config_root().join("skills"))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| e.path().is_dir())
                    .filter_map(|e| e.file_name().to_str().map(str::to_string))
                    .filter(|n| n.starts_with("omm-"))
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// Spawn `omm <args> --json --yes`, poll `trigger` every 2 ms and SIGKILL
    /// the process the moment it holds (an interrupted install), then wait
    /// until nothing under the roots changes for 400 ms — an orphaned host
    /// child finishes its own write. Returns whether the trigger fired
    /// before the process exited on its own.
    fn omm_killed_when(&self, args: &[&str], trigger: impl Fn() -> bool) -> bool {
        let mut argv = args.to_vec();
        argv.push("--json");
        argv.push("--yes");
        let mut child = self.omm_cmd(&argv, &self.ws).spawn().expect("spawn omm");
        let started = Instant::now();
        let fired = loop {
            if trigger() {
                let _ = child.kill();
                break true;
            }
            if let Ok(Some(_)) = child.try_wait() {
                break false;
            }
            if started.elapsed() > Duration::from_secs(90) {
                let _ = child.kill();
                break false;
            }
            std::thread::sleep(Duration::from_millis(2));
        };
        let _ = child.wait();
        self.settle();
        fired
    }

    /// Wait until two snapshots 400 ms apart agree (bounded at 15 s).
    fn settle(&self) {
        let started = Instant::now();
        let mut last = self.snapshot();
        loop {
            std::thread::sleep(Duration::from_millis(400));
            let now = self.snapshot();
            if now == last || started.elapsed() > Duration::from_secs(15) {
                return;
            }
            last = now;
        }
    }

    fn settings(&self) -> Value {
        std::fs::read(self.settings_path())
            .ok()
            .map(|b| serde_json::from_slice(&b).expect("settings.json is JSON"))
            .unwrap_or(Value::Null)
    }

    fn write_settings(&self, v: &Value) {
        std::fs::create_dir_all(self.config_root()).expect("config root");
        let mut bytes = serde_json::to_vec_pretty(v).expect("json");
        bytes.push(b'\n');
        std::fs::write(self.settings_path(), bytes).expect("write settings.json");
    }

    fn snapshot(&self) -> Snapshot {
        let mut out = Snapshot::new();
        for (scope, dir) in [
            ("config", self.sb.config_home.clone()),
            ("data/muse", self.data_root()),
        ] {
            walk(&dir, &dir, scope, &mut out);
        }
        out
    }

    /// [`E2e::snapshot`] minus the host-owned residue of
    /// [`host_owned_residue`] — what a read-only host verb (`muse
    /// --version`, the install plan's reachability probe) may leave behind
    /// on its own: bootstrap traces, the plugin cache, the startup locks.
    fn snapshot_sans_host_residue(&self) -> Snapshot {
        self.snapshot()
            .into_iter()
            .filter(|(k, _)| !is_host_owned(k))
            .collect()
    }

    /// An absolute residue path from the preview as a snapshot key
    /// (`config/…`, `data/…`) when it lies under one of the two roots, else
    /// sandbox-relative when it lies inside the sandbox at all.
    fn residue_key(&self, abs: &str) -> Option<String> {
        let p = Path::new(abs);
        for (scope, dir) in [
            ("config", &self.sb.config_home),
            ("data", &self.sb.data_home),
        ] {
            if let Ok(r) = p.strip_prefix(dir) {
                return Some(format!("{scope}/{}", r.to_string_lossy()));
            }
        }
        p.strip_prefix(&self.sb.root)
            .ok()
            .map(|r| r.to_string_lossy().into_owned())
    }

    /// The inverse of [`E2e::residue_key`]: where a snapshot key lives.
    fn residue_abs(&self, rel: &str) -> PathBuf {
        if let Some(r) = rel.strip_prefix("config/") {
            return self.sb.config_home.join(r);
        }
        if let Some(r) = rel.strip_prefix("data/") {
            return self.sb.data_home.join(r);
        }
        self.sb.root.join(rel)
    }

    /// R5 (see the module docs): `$OMM` gone; the preview names exactly the
    /// fixed host-owned residue (unconditionally, whether or not it exists);
    /// with those paths excluded, every path under both roots is byte- and
    /// mode-identical to the baseline.
    fn check_r5(
        &self,
        f: &mut Findings,
        before: &Snapshot,
        after: &Snapshot,
        preview_residue: &[String],
    ) {
        f.check(!self.omm_root().exists(), || {
            format!(
                "R5: {} still exists after uninstall",
                self.omm_root().display()
            )
        });
        // 1. The preview names every fixed host-owned path, unconditionally.
        let required = host_owned_residue();
        for rel in &required {
            let abs = self.residue_abs(rel).display().to_string();
            f.check(preview_residue.contains(&abs), || {
                format!(
                    "uninstall preview does not name the host-owned residue {rel} under kept (preview.residue = {preview_residue:?})"
                )
            });
        }
        f.check(
            preview_residue
                .iter()
                .any(|r| r.ends_with("session-name-authority")),
            || format!("uninstall preview does not name the session-name-authority dir ({preview_residue:?})"),
        );
        // 2. … and nothing else inside the sandbox: the scope the snapshot
        //    excludes is exactly the preview's in-sandbox residue.
        let named: BTreeSet<String> = preview_residue
            .iter()
            .filter_map(|r| self.residue_key(r))
            .collect();
        let extra: Vec<&String> = named.iter().filter(|n| !required.contains(n)).collect();
        f.check(extra.is_empty(), || {
            format!("uninstall preview names residue inside the sandbox beyond the fixed host-owned list: {extra:?}")
        });
        // 3. Everything else under both roots: byte- and mode-identical.
        let excluded = |rel: &str| {
            required
                .iter()
                .any(|n| rel == n || rel.starts_with(&format!("{n}/")))
        };
        let is_ancestor_of_excluded =
            |rel: &str| required.iter().any(|n| n.starts_with(&format!("{rel}/")));
        let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
        let mut differing = Vec::new();
        let mut tolerated = Vec::new();
        for k in keys {
            if before.get(k) == after.get(k) {
                continue;
            }
            let line = format!(
                "{k}: before={} after={}",
                before.get(k).map(String::as_str).unwrap_or("absent"),
                after.get(k).map(String::as_str).unwrap_or("absent")
            );
            let is_dir = after.get(k).map(|d| d.starts_with("dir ")).unwrap_or(false);
            if excluded(k) || (is_dir && is_ancestor_of_excluded(k)) {
                tolerated.push(line);
            } else {
                differing.push(line);
            }
        }
        if !tolerated.is_empty() {
            eprintln!(
                "R5 excluded host-owned residue (named in the uninstall preview), {}:\n  {}",
                tolerated.len(),
                tolerated.join("\n  ")
            );
        }
        f.check(differing.is_empty(), || {
            format!(
                "R5: {} path(s) under the roots differ (bytes or mode) after uninstall outside the excluded host-owned residue:\n  {}",
                differing.len(),
                differing.join("\n  ")
            )
        });
        let installed = self.data_root().join("plugins").join("installed.json");
        if let Ok(bytes) = std::fs::read(&installed) {
            let v: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
            let empty = match &v["plugins"] {
                Value::Object(m) => m.is_empty(),
                Value::Array(a) => a.is_empty(),
                _ => false,
            };
            f.check(empty, || {
                format!(
                    "{} still lists plugins after uninstall: {v}",
                    installed.display()
                )
            });
        }
    }
}

impl Drop for E2e {
    fn drop(&mut self) {
        if std::env::var_os("OMM_E2E_KEEP").is_some() {
            if let Some(tmp) = self.tmp.take() {
                let kept = tmp.keep();
                eprintln!("OMM_E2E_KEEP: sandbox kept at {}", kept.display());
            }
        }
    }
}

/// The host-owned residue inside the sandbox the uninstall preview must
/// name unconditionally (Gate 1 decision; host-reality.md "Paths"): the
/// session logs, the tracing lanes, the host's plugin store itself, the two
/// startup locks and the managed skill store's metadata — sandbox-relative
/// snapshot keys. The `getpwuid` session-name-authority dir lies outside
/// the sandbox and is asserted by suffix.
fn host_owned_residue() -> Vec<String> {
    [
        "data/muse/sessions",
        "data/muse/local-tracing",
        "data/muse/plugins",
        "data/muse/runtime",
        "config/muse/.settings.json.lock",
        "config/muse/.auth.json.lock",
        "config/muse/skills/.muse",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// A path the host creates or keeps on its own: under one of the
/// [`host_owned_residue`] roots.
fn is_host_owned(rel: &str) -> bool {
    host_owned_residue()
        .iter()
        .any(|p| rel == p || rel.starts_with(&format!("{p}/")))
}

/// Relative names under a leftover dir, for failure messages.
fn leftover_list(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && !p.is_symlink() {
                stack.push(p.clone());
            }
            out.push(
                p.strip_prefix(dir)
                    .map(|r| r.display().to_string())
                    .unwrap_or_else(|_| p.display().to_string()),
            );
        }
    }
    out.sort();
    out
}

fn walk(base: &Path, dir: &Path, scope: &str, out: &mut Snapshot) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let rel = p.strip_prefix(base).expect("under base");
        let key = format!("{scope}/{}", rel.to_string_lossy());
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        let mode = file_mode(&meta);
        let desc = if meta.file_type().is_symlink() {
            format!(
                "symlink -> {} mode={mode:o}",
                std::fs::read_link(&p)
                    .map(|t| t.display().to_string())
                    .unwrap_or_default()
            )
        } else if meta.is_dir() {
            format!("dir mode={mode:o}")
        } else {
            format!(
                "file sha256={} mode={mode:o}",
                omm_host::fsx::sha256_file(&p).unwrap_or_else(|_| "unreadable".into())
            )
        };
        out.insert(key, desc);
        if meta.is_dir() && !meta.file_type().is_symlink() {
            walk(base, &p, scope, out);
        }
    }
}

#[cfg(unix)]
fn file_mode(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode()
}

#[cfg(not(unix))]
fn file_mode(_meta: &std::fs::Metadata) -> u32 {
    0
}

fn copy_tree(src: &Path, dest: &Path) {
    std::fs::create_dir_all(dest).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir").flatten() {
        let ty = e.file_type().expect("file type");
        let to = dest.join(e.file_name());
        if ty.is_dir() {
            copy_tree(&e.path(), &to);
        } else if ty.is_file() {
            std::fs::copy(e.path(), &to).expect("copy");
        } else {
            panic!("{}: not a file or directory", e.path().display());
        }
    }
}

/// A copy of the repository's source of truth (`content/` + the two
/// `Cargo.toml` the version is read from) under `dir`, without generated output.
fn source_copy(repo: &Path, dir: &Path) {
    copy_tree(&repo.join("content"), &dir.join("content"));
    std::fs::create_dir_all(dir.join("crates").join("omm")).expect("crates/omm");
    std::fs::copy(
        repo.join("crates").join("omm").join("Cargo.toml"),
        dir.join("crates").join("omm").join("Cargo.toml"),
    )
    .expect("copy crate manifest");
    std::fs::copy(repo.join("Cargo.toml"), dir.join("Cargo.toml"))
        .expect("copy workspace manifest");
}

/// The active skill ids of `content/catalog.json` (R8: read, never listed).
fn catalog_skill_ids(repo: &Path) -> BTreeSet<String> {
    let v: Value = serde_json::from_slice(
        &std::fs::read(repo.join("content").join("catalog.json")).expect("catalog.json"),
    )
    .expect("catalog is JSON");
    v["assets"]
        .as_array()
        .expect("assets")
        .iter()
        .filter(|a| a["kind"] == "skill" && a["lifecycle"] == "active")
        .filter_map(|a| a["id"].as_str().map(str::to_string))
        .collect()
}

/// The digest the committed `marketplace.json` carries for `omm`.
fn committed_digest(repo: &Path) -> String {
    let v: Value =
        serde_json::from_slice(&std::fs::read(repo.join("marketplace.json")).expect("marketplace"))
            .expect("marketplace.json is JSON");
    v["plugins"]
        .as_array()
        .expect("plugins")
        .iter()
        .find(|p| p["name"] == "oh-my-musecode")
        .and_then(|p| p["integrity"]["digest"].as_str())
        .expect("plugin digest")
        .to_string()
}

fn check(doc: &Value, id: &str) -> Value {
    doc["checks"]
        .as_array()
        .expect("checks[]")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no check {id} in {doc}"))
        .clone()
}

/// `"<id> <severity>: <observed> (fix: <fix>)"` for every non-info row.
fn non_info_rows(doc: &Value) -> Vec<String> {
    doc["checks"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter(|c| c["severity"] != "info")
                .map(|c| {
                    format!(
                        "{} {}: {} (fix: {})",
                        c["id"].as_str().unwrap_or("?"),
                        c["severity"].as_str().unwrap_or("?"),
                        c["observed"].as_str().unwrap_or(""),
                        c["fix"].as_str().unwrap_or("-")
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn has_critical(doc: &Value) -> bool {
    doc["checks"]
        .as_array()
        .map(|rows| rows.iter().any(|c| c["severity"] == "critical"))
        .unwrap_or(true)
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Assertions every successful bundle install must satisfy, on the omm
/// document, the ledger, the host's own view and the filesystem.
fn assert_bundle_installed(h: &E2e, doc: &Value) {
    assert_eq!(doc["command"], "install");
    assert_eq!(doc["mode"], "plugin");
    let digest = doc["bundle"]["digest"]
        .as_str()
        .expect("digest")
        .to_string();
    assert!(digest.starts_with("sha256:"), "{digest}");
    let trusted = strings(&doc["bundle"]["trusted"]);
    assert!(trusted.len() >= 3, "{trusted:?}");
    assert!(trusted
        .iter()
        .all(|t| t.starts_with("plugin:oh-my-musecode:")));
    assert_eq!(doc["bundle"]["skills_missing"], json!([]));
    let skills = catalog_skill_ids(&h.repo);
    assert_eq!(
        doc["bundle"]["skills_listed"]
            .as_u64()
            .expect("skills_listed") as usize,
        skills.len()
    );
    assert_eq!(doc["budget"]["within_budget"], true);
    // The host's view: native, every trusted line literally trusted_enabled,
    // every shipped skill listed as plugin:oh-my-musecode:<id>.
    let ins = h.inspect().expect("plugins inspect --json");
    assert_eq!(
        ins.manifest_family.as_deref(),
        Some(hr::MANIFEST_FAMILY_NATIVE)
    );
    for id in &trusted {
        assert!(ins.is_trusted_enabled(id), "{id}: {:?}", ins.capability(id));
    }
    let listed = h.skills_listed("plugin");
    for id in &skills {
        assert!(
            listed.contains(&format!("plugin:oh-my-musecode:{id}")),
            "skills list lacks plugin:oh-my-musecode:{id}: {listed:?}"
        );
    }
    assert_eq!(h.installed_plugin_ids(), vec!["oh-my-musecode".to_string()]);
    assert!(h.marketplace_names().contains(&"omm".to_string()));
    // The ledger.
    let ledger = h.ledger().expect("ledger written");
    let regs = ledger["registrations"].as_array().expect("registrations");
    let mkt = regs
        .iter()
        .find(|r| r["kind"] == "muse-marketplace")
        .expect("marketplace registration");
    assert_eq!(mkt["name"], "omm");
    let plugin_reg = regs
        .iter()
        .find(|r| r["kind"] == "muse-plugin")
        .expect("plugin registration");
    assert_eq!(plugin_reg["id"], "oh-my-musecode");
    assert_eq!(plugin_reg["package_sha256"], digest);
    assert_eq!(strings(&plugin_reg["approved"]), trusted);
    assert!(regs.iter().any(|r| r["kind"] == "settings-key"));
    assert!(regs.iter().any(|r| r["kind"] == "trust"));
    let entries = ledger["entries"].as_array().expect("entries");
    for (path, class) in [
        ("AGENTS.md", "seeded"),
        ("settings.json", "seeded"),
        ("trust.json", "seeded"),
    ] {
        assert!(
            entries
                .iter()
                .any(|e| e["path"] == path && e["class"] == class),
            "{path} {class}: {entries:?}"
        );
    }
    assert_eq!(entries.iter().filter(|e| e["kind"] == "theme").count(), 3);
    // The filesystem.
    let settings = h.settings();
    assert_eq!(
        settings["run"]["context_slimming"]["skill_catalog_descriptions"],
        "first_sentence"
    );
    for id in &trusted {
        assert_eq!(
            settings["runtime_capabilities"][id]["enabled"], true,
            "{id} in settings.runtime_capabilities"
        );
    }
    let trust: Value = serde_json::from_slice(
        &std::fs::read(h.config_root().join("trust.json")).expect("trust.json"),
    )
    .expect("trust.json is JSON");
    let ws_key = std::fs::canonicalize(&h.ws).expect("ws");
    assert_eq!(
        trust["projects"][ws_key.to_string_lossy().as_ref()]["decision"],
        "trusted"
    );
    assert!(h.config_root().join("AGENTS.md").is_file());
    assert_eq!(
        h.config_root()
            .join("themes")
            .read_dir()
            .expect("themes")
            .count(),
        3
    );
    assert!(h.omm_root().join("audit.log").is_file());
    let foreign = h.foreign_home_entries();
    assert!(foreign.is_empty(), "nothing lands in $HOME: {foreign:?}");
}

/// What every completed bundle uninstall must leave: no plugin, no
/// marketplace, no omm files, no ledger.
fn assert_bundle_uninstalled(h: &E2e, doc: &Value) {
    assert_eq!(doc["report"]["errors"], json!([]), "{doc}");
    assert_eq!(doc["report"]["ledger_removed"], true);
    assert!(h.ledger().is_none());
    assert!(!h.omm_root().exists(), "$OMM removed");
    assert!(h.inspect().is_err(), "the plugin is removed");
    assert!(h.installed_plugin_ids().is_empty());
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(!h.config_root().join("themes").exists());
    assert!(!h.config_root().join("trust.json").exists());
    assert!(!h.settings_path().exists());
}

// ---------------------------------------------------------------------------
// 1. install (bundle) → doctor green → uninstall → byte-identical
// ---------------------------------------------------------------------------

#[test]
fn s01_bundle_install_doctor_green_uninstall_byte_identical() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();

    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_bundle_installed(&h, &r.json);
    assert_eq!(r.json["bundle"]["plugin"], "updated");
    assert_eq!(r.json["bundle"]["marketplace"]["action"], "updated");
    assert_eq!(r.json["bundle"]["digest"], committed_digest(&h.repo));
    assert_eq!(r.json["converge"]["noop"], false);

    // doctor green: exit 0, no critical row (warn rows are reported; scenario 12 asserts on them).
    let d = h.doctor(false);
    assert!(d.json.is_object(), "doctor --json: {}", d.ctx());
    let rows = non_info_rows(&d.json);
    if !rows.is_empty() {
        eprintln!("doctor non-info rows after a bundle install: {rows:?}");
    }
    f.check(d.code == 0 && !has_critical(&d.json), || {
        format!(
            "doctor is not green after `omm install`: exit {} rows {rows:?}",
            d.code
        )
    });

    // One echo session in the sandbox, so the host's session store exists and
    // the preview has to name it.
    let echo = h
        .invoker()
        .run(&["exec", "--provider", "echo", "hi"])
        .expect("exec echo");
    assert!(echo.ok(), "echo session: {:?} {}", echo.code, echo.stderr);
    assert!(h.data_root().join("sessions").is_dir());

    // The preview: two sections, host steps, kept residue; nothing touched.
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    assert_eq!(dry.json["dry_run"], true);
    let steps = strings(&dry.json["preview"]["host_steps"]);
    for needle in [
        "plugins remove oh-my-musecode --delete-data",
        "marketplace remove omm",
        "settings key",
        "trust entry",
    ] {
        assert!(
            steps.iter().any(|s| s.contains(needle)),
            "{needle}: {steps:?}"
        );
    }
    let residue = strings(&dry.json["preview"]["residue"]);
    assert!(
        residue.iter().any(|r| r.contains("session-name-authority")),
        "{residue:?}"
    );
    assert!(h.ledger().is_some());
    assert!(h.inspect().is_ok());

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_bundle_uninstalled(&h, &un.json);
    let residue = strings(&un.json["preview"]["residue"]);
    eprintln!("uninstall preview residue: {residue:?}");
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 1");
}

// ---------------------------------------------------------------------------
// 2. install --no-plugin → doctor green → uninstall → byte-identical
// ---------------------------------------------------------------------------

#[test]
fn s02_no_plugin_install_doctor_green_uninstall_byte_identical() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let skills = catalog_skill_ids(&h.repo);

    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(r.json["mode"], "skills");
    let installed: BTreeSet<String> = strings(&r.json["skills"]["installed"])
        .into_iter()
        .collect();
    assert_eq!(installed, skills);
    let store = h.config_root().join("skills");
    for id in &skills {
        assert!(store.join(id).join("SKILL.md").is_file(), "{id}");
    }
    // The host's view: every skill in the managed store; no plugin.
    let listed = h.skills_listed("user");
    assert_eq!(listed, skills, "skills list --source user");
    assert!(h.inspect().is_err(), "no plugin under --no-plugin");
    assert!(h.installed_plugin_ids().is_empty());
    // The ledger: one entry per file, mechanism muse-skills-install.
    let ledger = h.ledger().expect("ledger");
    let entries = ledger["entries"].as_array().expect("entries");
    let skill_entries = entries
        .iter()
        .filter(|e| e["mechanism"] == "muse-skills-install")
        .count();
    assert!(skill_entries > skills.len(), "{skill_entries}");
    assert!(!ledger["registrations"]
        .as_array()
        .expect("regs")
        .iter()
        .any(|r| r["kind"] == "muse-plugin"));

    let d = h.doctor(false);
    assert!(d.json.is_object(), "doctor --json: {}", d.ctx());
    let rows = non_info_rows(&d.json);
    f.check(d.code == 0 && !has_critical(&d.json), || {
        format!(
            "doctor is not green after `omm install --no-plugin`: exit {} rows {rows:?}",
            d.code
        )
    });

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    let steps = strings(&un.json["preview"]["host_steps"]);
    assert_eq!(
        steps
            .iter()
            .filter(|s| s.starts_with("muse skills uninstall"))
            .count(),
        skills.len()
    );
    let residue = strings(&un.json["preview"]["residue"]);
    eprintln!("uninstall preview residue: {residue:?}");
    assert!(
        residue.iter().any(|r| r.ends_with("skills/.muse")),
        "the managed store's metadata is named: {residue:?}"
    );
    for id in &skills {
        assert!(!store.join(id).exists(), "{id} removed");
    }
    assert!(h.skills_listed("user").is_empty());
    assert!(h.ledger().is_none());
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 2");
}

// ---------------------------------------------------------------------------
// 3. install → user edits a skill → update with a changed upstream → STAGED
// ---------------------------------------------------------------------------

#[test]
fn s03_update_stages_a_user_edited_skill_and_leaves_the_disk_untouched() {
    let h = e2e_or_skip!();
    let skills = catalog_skill_ids(&h.repo);
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let store = h.config_root().join("skills");
    // The user edits one skill.
    let edited = store.join("omm-commit").join("SKILL.md");
    let mut text = std::fs::read_to_string(&edited).expect("skill");
    text.push_str("\nmy note\n");
    std::fs::write(&edited, &text).expect("edit");
    // Upstream changes that skill and another one.
    let copy = h.sb.root.join("upstream");
    source_copy(&h.repo, &copy);
    for id in ["omm-commit", "omm-debug"] {
        let p = copy
            .join("content")
            .join("skills")
            .join(id)
            .join("SKILL.md");
        let mut t = std::fs::read_to_string(&p).expect("upstream skill");
        t.push_str("\nv2 body.\n");
        std::fs::write(&p, t).expect("upstream edit");
    }
    let copy_str = copy.to_str().expect("utf-8");
    // Dry run first: the plan, nothing written.
    let dry = h.omm(&["update", "--dry-run", "--source", copy_str]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    assert_eq!(dry.json["dry_run"], true);
    assert_eq!(std::fs::read_to_string(&edited).expect("skill"), text);
    assert!(!h.omm_root().join("updates").exists());

    let up = h.omm(&["update", "--source", copy_str]);
    assert_eq!(up.code, 0, "{}", up.ctx());
    let staged = up.json["staged"].as_array().expect("staged");
    assert_eq!(staged.len(), 1, "{}", up.ctx());
    assert_eq!(staged[0]["path"], "skills/omm-commit/SKILL.md");
    assert_eq!(staged[0]["base"], "muse-config");
    // On disk untouched; the new content staged under $OMM/updates/<version>/.
    assert_eq!(std::fs::read_to_string(&edited).expect("skill"), text);
    let staged_to = PathBuf::from(staged[0]["staged_to"].as_str().expect("staged_to"));
    assert!(staged_to.starts_with(h.omm_root().join("updates")));
    assert!(std::fs::read_to_string(&staged_to)
        .expect("staged file")
        .ends_with("v2 body.\n"));
    let version = up.json["version"].as_str().expect("version");
    let report: Value = serde_json::from_slice(
        &std::fs::read(
            h.omm_root()
                .join("updates")
                .join(version)
                .join("report.json"),
        )
        .expect("report.json"),
    )
    .expect("report is JSON");
    assert_eq!(report["staged"][0]["path"], "skills/omm-commit/SKILL.md");
    // The untouched skill was overwritten through the host's store.
    assert_eq!(up.json["skills"]["updated"], json!(["omm-debug"]));
    assert!(
        up.json["skills"]["skipped"]
            .as_array()
            .expect("skipped")
            .iter()
            .any(|s| s["id"] == "omm-commit"),
        "the edited skill is reported as skipped: {}",
        up.ctx()
    );
    let debug = std::fs::read_to_string(store.join("omm-debug").join("SKILL.md")).expect("debug");
    assert!(debug.ends_with("v2 body.\n"));
    // The ledger records what omm wrote (R2), the host still lists everything.
    let ledger = h.ledger().expect("ledger");
    let e = ledger["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["path"] == "skills/omm-debug/SKILL.md")
        .expect("omm-debug entry");
    assert_eq!(e["writer"], "omm update");
    assert_eq!(e["sha256"], omm_host::fsx::sha256_bytes(debug.as_bytes()));
    assert_eq!(h.skills_listed("user"), skills);
    assert!(
        h.omm_root()
            .join("snapshots")
            .read_dir()
            .expect("snapshots")
            .count()
            >= 1
    );
    // Again: only the standing conflict.
    let again = h.omm(&["update", "--source", copy_str]);
    assert_eq!(again.code, 0, "{}", again.ctx());
    assert_eq!(again.json["skills"]["updated"], json!([]));
    assert_eq!(again.json["staged"].as_array().expect("staged").len(), 1);
    assert_eq!(std::fs::read_to_string(&edited).expect("skill"), text);
}

/// The bundle half of scenario 3: a user edit to a ledgered file is staged,
/// the user region of the rules file survives, and a changed upstream
/// package is reinstalled and re-approved as one transaction (R3 + R14).
#[test]
fn s03b_bundle_update_stages_a_theme_edit_and_reinstalls_a_changed_package() {
    let h = e2e_or_skip!();
    // The upstream has to be the marketplace source, so it is a built copy.
    let copy = h.sb.root.join("upstream");
    source_copy(&h.repo, &copy);
    let copy_str = copy.to_str().expect("utf-8");
    let b = h.omm(&["build", copy_str]);
    assert_eq!(b.code, 0, "{}", b.ctx());
    assert!(copy.join("marketplace.json").is_file());
    let r = h.omm(&["install", "--source", copy_str]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let old_digest = r.json["bundle"]["digest"]
        .as_str()
        .expect("digest")
        .to_string();

    // The user edits one theme and writes into the rules' user region.
    let themes = h.config_root().join("themes");
    let edited_theme = std::fs::canonicalize(themes.join("omm-carbon.tmTheme")).expect("theme");
    std::fs::write(&edited_theme, b"<!-- my theme -->\n").expect("edit theme");
    let agents_path = h.config_root().join("AGENTS.md");
    let agents = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    assert!(agents.contains("<!-- omm:user-end -->"), "{agents}");
    let with_rule = agents.replace(
        "<!-- omm:user-end -->",
        "- my own rule\n<!-- omm:user-end -->",
    );
    std::fs::write(&agents_path, &with_rule).expect("edit rules");

    // Upstream changes every theme, the managed rules and a skill body.
    for e in std::fs::read_dir(copy.join("content").join("themes"))
        .expect("themes")
        .flatten()
    {
        let p = e.path();
        let mut t = std::fs::read_to_string(&p).expect("theme");
        t.push_str("<!-- v2 -->\n");
        std::fs::write(&p, t).expect("upstream theme");
    }
    let tmpl = copy.join("content").join("rules").join("AGENTS.md.tmpl");
    let t = std::fs::read_to_string(&tmpl).expect("tmpl");
    assert!(t.contains("<!-- omm:managed-end -->"));
    std::fs::write(
        &tmpl,
        t.replace(
            "<!-- omm:managed-end -->",
            "- a new managed line\n<!-- omm:managed-end -->",
        ),
    )
    .expect("upstream rules");
    let skill = copy
        .join("content")
        .join("skills")
        .join("omm-self")
        .join("SKILL.md");
    let mut t = std::fs::read_to_string(&skill).expect("skill");
    t.push_str("\nMore body.\n");
    std::fs::write(&skill, t).expect("upstream skill");
    let b = h.omm(&["build", copy_str]);
    assert_eq!(b.code, 0, "{}", b.ctx());
    let new_committed = committed_digest(&copy);
    assert_ne!(new_committed, old_digest);

    let up = h.omm(&["update", "--source", copy_str]);
    assert_eq!(up.code, 0, "{}", up.ctx());
    let staged = up.json["staged"].as_array().expect("staged");
    assert_eq!(staged.len(), 1, "{}", up.ctx());
    assert_eq!(staged[0]["path"], "themes/omm-carbon.tmTheme");
    assert_eq!(
        std::fs::read(&edited_theme).expect("theme"),
        b"<!-- my theme -->\n",
        "on disk untouched"
    );
    let staged_to = PathBuf::from(staged[0]["staged_to"].as_str().expect("staged_to"));
    assert!(std::fs::read_to_string(&staged_to)
        .expect("staged")
        .contains("<!-- v2 -->"));
    for name in ["omm-paper.tmTheme", "omm-slate.tmTheme"] {
        assert!(
            std::fs::read_to_string(themes.join(name))
                .expect("theme")
                .contains("<!-- v2 -->"),
            "{name} overwritten (user never touched it)"
        );
    }
    let agents = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    assert!(agents.contains("- a new managed line"), "{agents}");
    assert!(agents.contains("- my own rule"), "{agents}");
    // The bundle: digest changed → remove → install → approve → verify.
    assert_eq!(up.json["bundle"]["plugin"], "updated", "{}", up.ctx());
    let new_digest = up.json["bundle"]["digest"].as_str().expect("digest");
    assert_eq!(new_digest, new_committed);
    let ins = h.inspect().expect("inspect");
    assert_eq!(
        ins.manifest_family.as_deref(),
        Some(hr::MANIFEST_FAMILY_NATIVE)
    );
    let trusted = strings(&up.json["bundle"]["trusted"]);
    assert!(trusted.len() >= 3);
    for id in &trusted {
        assert!(ins.is_trusted_enabled(id), "{id}");
    }
    let ledger = h.ledger().expect("ledger");
    let reg = ledger["registrations"]
        .as_array()
        .expect("regs")
        .iter()
        .find(|r| r["kind"] == "muse-plugin")
        .expect("plugin reg");
    assert_eq!(reg["package_sha256"], new_digest);
    let listed = h.skills_listed("plugin");
    assert!(listed.contains("plugin:oh-my-musecode:omm-self"));
    // A second update is a no-op apart from the standing conflict.
    let again = h.omm(&["update", "--source", copy_str]);
    assert_eq!(again.code, 0, "{}", again.ctx());
    assert_eq!(again.json["bundle"]["plugin"], "unchanged");
    assert_eq!(again.json["rules"]["action"], "unchanged");
    assert_eq!(again.json["staged"].as_array().expect("staged").len(), 1);
    assert_eq!(
        again.json["converge"]["total"]["updated"],
        0,
        "{}",
        again.ctx()
    );
}

// ---------------------------------------------------------------------------
// 4. install → user edits → uninstall preserves; --force removes
// ---------------------------------------------------------------------------

#[test]
fn s04_uninstall_preserves_a_user_edit_unless_forced() {
    let h = e2e_or_skip!();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let theme = std::fs::canonicalize(h.config_root().join("themes").join("omm-slate.tmTheme"))
        .expect("theme");
    std::fs::write(&theme, b"edited").expect("edit");
    let theme_str = theme.to_str().expect("utf-8");

    // The preview lists the edit under ✓ preserve; a dry run changes nothing.
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let preserved = dry.json["preview"]["preserve"]
        .as_array()
        .expect("preserve");
    assert!(
        preserved.iter().any(|p| p["path"] == theme_str),
        "{}",
        dry.ctx()
    );
    assert!(!dry.json["preview"]["remove"]
        .as_array()
        .expect("remove")
        .iter()
        .any(|p| p["path"] == theme_str));
    let human = h.omm_raw(&["uninstall", "--dry-run", "--yes"]);
    assert_eq!(human.code, 0, "{}", human.ctx());
    assert!(human.stdout.contains("✓ preserve (1)"), "{}", human.stdout);
    assert!(
        human.stdout.contains(theme_str) && human.stdout.contains("edited since omm wrote it"),
        "{}",
        human.stdout
    );
    assert!(h.ledger().is_some());
    assert!(h.inspect().is_ok());
    assert_eq!(std::fs::read(&theme).expect("theme"), b"edited");

    // Without --force the edit survives; everything else goes.
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["preserved"], 1, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]));
    assert_eq!(std::fs::read(&theme).expect("theme"), b"edited");
    assert!(!h
        .config_root()
        .join("themes")
        .join("omm-carbon.tmTheme")
        .exists());
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(h.ledger().is_none());
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());

    // A reinstall never overwrites the unledgered edit (skipped); the edit
    // is not omm's any more (R2), so even --force leaves it alone.
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(
        r.json["themes"]["skipped"]
            .as_array()
            .expect("skipped")
            .iter()
            .any(|s| s["path"] == theme_str),
        "{}",
        r.ctx()
    );
    assert_eq!(std::fs::read(&theme).expect("theme"), b"edited");
    let un = h.omm(&["uninstall", "--force"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert!(
        !un.json["preview"]["remove"]
            .as_array()
            .expect("remove")
            .iter()
            .any(|p| p["path"] == theme_str),
        "an unledgered file is never removed: {}",
        un.ctx()
    );
    assert_eq!(std::fs::read(&theme).expect("theme"), b"edited");
    assert!(h.ledger().is_none());
    std::fs::remove_file(&theme).expect("clean up the unledgered edit");
    std::fs::remove_dir(h.config_root().join("themes")).expect("themes dir now empty");

    // Same edit on a fresh install: --force removes it (ledgered, sha differs).
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(r.json["themes"]["skipped"]
        .as_array()
        .expect("skipped")
        .is_empty());
    std::fs::write(&theme, b"edited again").expect("edit");
    let forced = h.omm(&["uninstall", "--force"]);
    assert_eq!(forced.code, 0, "{}", forced.ctx());
    assert!(
        forced.json["preview"]["remove"]
            .as_array()
            .expect("remove")
            .iter()
            .any(|p| p["path"] == theme_str && p["forced"] == true),
        "{}",
        forced.ctx()
    );
    assert_eq!(forced.json["report"]["preserved"], 0, "{}", forced.ctx());
    assert!(!theme.exists());
    assert!(!h.config_root().join("themes").exists());
    assert_bundle_uninstalled(&h, &forced.json);
}

// ---------------------------------------------------------------------------
// 5. install → muse plugins disable omm → doctor D1 critical with fix → fix → green
// ---------------------------------------------------------------------------

#[test]
fn s05_doctor_d1_after_plugins_disable_prints_the_exact_fix_and_the_fix_heals() {
    let h = e2e_or_skip!();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let approved = strings(
        &h.ledger().expect("ledger")["registrations"]
            .as_array()
            .expect("regs")
            .iter()
            .find(|r| r["kind"] == "muse-plugin")
            .expect("plugin reg")["approved"],
    );
    assert!(approved.len() >= 3);

    let out = h
        .invoker()
        .run(&["plugins", "disable", "oh-my-musecode", "--json"])
        .expect("plugins disable");
    assert!(out.ok(), "plugins disable: {}", out.stderr);
    let ins = h.inspect().expect("inspect after disable");
    assert_eq!(ins.enabled, Some(false), "{:?}", ins.raw["record"]);
    assert!(
        ins.runtime_capabilities.is_empty(),
        "`plugins disable` deletes every capability line: {:?}",
        ins.runtime_capabilities
    );

    let d = h.doctor(true);
    assert_eq!(d.code, 1, "{}", d.ctx());
    assert_eq!(d.json["ok"], false);
    let d1 = check(&d.json, "D1");
    assert_eq!(d1["severity"], "critical", "{d1}");
    let fix = d1["fix"].as_str().expect("fix").to_string();
    assert_eq!(fix, "muse plugins enable oh-my-musecode", "{d1}");
    assert!(
        d1["observed"]
            .as_str()
            .expect("observed")
            .contains("disabled"),
        "{d1}"
    );
    let human = h.omm_raw(&["doctor", "--fast"]);
    assert_eq!(human.code, 1);
    assert!(human.stdout.contains("CRIT D1 "), "{}", human.stdout);
    assert!(
        human.stdout.contains(&format!("fix: {fix}")),
        "{}",
        human.stdout
    );

    // Run exactly what doctor printed, until D1 is green (bounded).
    let mut applied = Vec::new();
    let mut next = Some(fix);
    for _round in 0..4 {
        let Some(fix) = next.take() else {
            break;
        };
        for line in fix.lines() {
            let argv: Vec<&str> = line.split_whitespace().collect();
            match argv.first().copied() {
                Some("muse") => {
                    let mut a: Vec<&str> = argv[1..].to_vec();
                    a.push("--json");
                    let out = h.invoker().run(&a).expect("run fix");
                    assert!(
                        out.ok(),
                        "fix `{line}` failed: {} {}",
                        out.stdout,
                        out.stderr
                    );
                }
                Some("omm") => {
                    let mut a: Vec<&str> = argv[1..].to_vec();
                    if matches!(a.first().copied(), Some("install") | Some("update")) {
                        a.push("--source");
                        a.push(h.repo_str());
                    }
                    let out = h.omm(&a);
                    assert_eq!(out.code, 0, "fix `{line}` failed: {}", out.ctx());
                }
                other => panic!("doctor printed an unrunnable fix {line:?} ({other:?})"),
            }
            applied.push(line.to_string());
        }
        let d = h.doctor(true);
        let d1 = check(&d.json, "D1");
        if d1["severity"] == "info" {
            break;
        }
        next = d1["fix"].as_str().map(str::to_string);
        assert!(next.is_some(), "D1 failed without a fix: {d1}");
    }
    eprintln!("fixes applied: {applied:?}");
    let d = h.doctor(false);
    assert_eq!(d.code, 0, "{} (fixes applied: {applied:?})", d.ctx());
    assert!(!has_critical(&d.json), "{}", d.ctx());
    assert_eq!(check(&d.json, "D1")["severity"], "info");
    let ins = h.inspect().expect("inspect after fix");
    for id in &approved {
        assert!(ins.is_trusted_enabled(id), "{id}: {:?}", ins.capability(id));
    }
    assert_eq!(h.installed_plugin_ids(), vec!["oh-my-musecode".to_string()]);
}

// ---------------------------------------------------------------------------
// 6. install → plant mcp_servers → doctor D3 → install refuses → fix → green
// ---------------------------------------------------------------------------

#[test]
fn s06_doctor_d3_mcp_collision_install_refuses_the_fix_heals() {
    let h = e2e_or_skip!();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let clean = h.settings();

    // The legacy spelling alone: works today, destroyed by the next rewrite (D3 warn).
    let mut legacy = clean.clone();
    legacy["mcp_servers"] = json!({});
    h.write_settings(&legacy);
    let d = h.doctor(true);
    let d3 = check(&d.json, "D3");
    assert_eq!(d3["severity"], "warn", "{d3}");
    assert_eq!(d3["fix"], "omm settings fix-mcp-collision");

    // Both spellings: every settings-writing verb (and omm install) refuses.
    let mut planted = clean.clone();
    planted["mcpServers"] = json!({});
    planted["mcp_servers"] = json!({});
    h.write_settings(&planted);
    let settings_bytes = std::fs::read(h.settings_path()).expect("settings");
    let ledger_bytes = std::fs::read(h.ledger_path()).expect("ledger");
    let d = h.doctor(true);
    assert_eq!(d.code, 1, "{}", d.ctx());
    let d3 = check(&d.json, "D3");
    assert_eq!(d3["severity"], "critical", "{d3}");
    assert_eq!(d3["fix"], "omm settings fix-mcp-collision");
    assert!(
        d3["observed"]
            .as_str()
            .expect("observed")
            .contains("both `mcpServers` and `mcp_servers`"),
        "{d3}"
    );
    let refused = h.install();
    assert_eq!(refused.code, 1, "{}", refused.ctx());
    assert!(
        refused.stderr.contains("fix-mcp-collision"),
        "{}",
        refused.ctx()
    );
    assert_eq!(
        std::fs::read(h.settings_path()).expect("settings"),
        settings_bytes,
        "the refused install did not touch settings.json"
    );
    assert_eq!(
        std::fs::read(h.ledger_path()).expect("ledger"),
        ledger_bytes,
        "the refused install did not touch the ledger"
    );
    let refused_theme = h.omm(&["theme", "omm-paper"]);
    assert_ne!(refused_theme.code, 0, "{}", refused_theme.ctx());
    assert_eq!(
        std::fs::read(h.settings_path()).expect("settings"),
        settings_bytes
    );

    // The fix, then green.
    let fix = h.omm(&["settings", "fix-mcp-collision"]);
    assert_eq!(fix.code, 0, "{}", fix.ctx());
    let s = h.settings();
    assert!(s.get("mcp_servers").is_none(), "{s}");
    assert!(s.get("mcpServers").is_some(), "{s}");
    assert_eq!(
        s["run"]["context_slimming"]["skill_catalog_descriptions"], "first_sentence",
        "the rest of the document survived"
    );
    let d = h.doctor(false);
    assert_eq!(d.code, 0, "{}", d.ctx());
    assert_eq!(check(&d.json, "D3")["severity"], "info");
    assert!(!has_critical(&d.json), "{}", d.ctx());
    let again = h.install();
    assert_eq!(again.code, 0, "{}", again.ctx());
    assert_eq!(again.json["bundle"]["plugin"], "unchanged");
    assert_eq!(again.json["settings"]["set"], json!([]));
    let ins = h.inspect().expect("inspect");
    for id in strings(&again.json["bundle"]["trusted"]) {
        assert!(ins.is_trusted_enabled(&id), "{id}");
    }
}

// ---------------------------------------------------------------------------
// 7. install → omm theme <x> → tui.theme set, validated, prior recorded; uninstall restores
// ---------------------------------------------------------------------------

#[test]
fn s07_theme_sets_tui_theme_records_the_prior_and_uninstall_restores_it() {
    let h = e2e_or_skip!();
    // A prior worth restoring: a bundled theme id the host accepts.
    let seed = json!({"schema_version": 1, "tui": {"theme": "ayu-dark"}});
    h.write_settings(&seed);
    h.warm_up_host();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(r.json["settings"]["seeded"], false);

    let t = h.omm(&["theme", "omm-carbon"]);
    assert_eq!(t.code, 0, "{}", t.ctx());
    assert_eq!(t.json["tui_theme"], "custom:omm-carbon");
    assert_eq!(t.json["provider"], "bundled");
    let theme_file = PathBuf::from(t.json["path"].as_str().expect("path"));
    assert!(theme_file.is_file());
    assert_eq!(
        std::fs::read(&theme_file).expect("theme"),
        std::fs::read(
            h.repo
                .join("content")
                .join("themes")
                .join("omm-carbon.tmTheme")
        )
        .expect("bundled"),
        "byte-identical copy"
    );
    let s = h.settings();
    assert_eq!(s["tui"]["theme"], "custom:omm-carbon");
    assert_eq!(s["schema_version"], 1);
    // Validated: the host still loads the document.
    let list = h
        .invoker()
        .run(&["skills", "list", "--json"])
        .expect("skills list");
    assert!(list.ok(), "the host rejects settings.json: {}", list.stderr);
    // The ledger: the settings-key registration carries the FIRST prior.
    let regs = |h: &E2e| -> Vec<Value> {
        h.ledger().expect("ledger")["registrations"]
            .as_array()
            .expect("regs")
            .iter()
            .filter(|r| r["kind"] == "settings-key" && r["path"] == "tui.theme")
            .cloned()
            .collect()
    };
    let tui = regs(&h);
    assert_eq!(tui.len(), 1, "{tui:?}");
    assert_eq!(tui[0]["prior"], "ayu-dark");
    let ledger = h.ledger().expect("ledger");
    assert!(ledger["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .any(|e| e["path"] == "themes/omm-carbon.tmTheme" && e["kind"] == "theme"));
    let settings_entry = ledger["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["path"] == "settings.json")
        .expect("settings entry");
    assert_eq!(
        settings_entry["class"], "shared-key",
        "the user's file existed before omm"
    );
    // Switching keeps the first prior; a second registration never appears.
    let t2 = h.omm(&["theme", "omm-paper"]);
    assert_eq!(t2.code, 0, "{}", t2.ctx());
    assert_eq!(h.settings()["tui"]["theme"], "custom:omm-paper");
    let tui = regs(&h);
    assert_eq!(tui.len(), 1);
    assert_eq!(tui[0]["prior"], "ayu-dark");
    // An unknown theme is refused before anything runs.
    let bad = h.omm(&["theme", "no-such-theme"]);
    assert_eq!(bad.code, 2, "{}", bad.ctx());
    assert_eq!(h.settings()["tui"]["theme"], "custom:omm-paper");

    // Uninstall restores the prior by a targeted patch and keeps the user's file.
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    let steps = strings(&un.json["preview"]["host_steps"]);
    assert!(
        steps
            .iter()
            .any(|s| s == "restore settings key tui.theme to its prior value"),
        "{steps:?}"
    );
    let after = h.settings();
    assert_eq!(after, seed, "settings.json back to the user's document");
    assert!(h.settings_path().is_file(), "the user's file is kept");
    assert!(!h.config_root().join("themes").exists());
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(!h.config_root().join("trust.json").exists());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(h.ledger().is_none());
    assert!(h.inspect().is_err());
    let list = h
        .invoker()
        .run(&["skills", "list", "--json"])
        .expect("skills list");
    assert!(list.ok(), "{}", list.stderr);
}

// ---------------------------------------------------------------------------
// 8. omm run: allowlist
// ---------------------------------------------------------------------------

#[test]
fn s08_run_refuses_prompts_by_the_allowlist_and_execs_known_verbs() {
    let h = e2e_or_skip!();
    let refused = h.omm_raw(&["run", "--", "zzznotacommand"]);
    assert_eq!(refused.code, 2, "{}", refused.ctx());
    assert!(
        refused.stderr.contains("not a Muse command"),
        "{}",
        refused.ctx()
    );
    assert!(refused.stdout.is_empty());
    // Root-flag prompt smuggling and a bare prompt are refused too.
    for argv in [
        vec!["run", "--", "--provider", "echo", "hi"],
        vec!["run", "--", "--", "hi"],
        vec!["run", "--", "-w", "hi"],
        vec!["run"],
    ] {
        let r = h.omm_raw(&argv);
        assert_eq!(r.code, 2, "{argv:?}: {}", r.ctx());
    }
    assert!(
        !h.data_root().join("sessions").exists(),
        "a refused argv never reached the host"
    );
    // Known verbs and root flags are exec'd.
    let v = h.omm_raw(&["run", "--", "--version"]);
    assert_eq!(v.code, 0, "{}", v.ctx());
    assert!(v.stdout.contains(hr::VERSION_STRING_PRODUCT), "{}", v.ctx());
    let list = h.omm_raw(&["run", "--", "skills", "list", "--json"]);
    assert_eq!(list.code, 0, "{}", list.ctx());
    assert!(list.json["skills"].is_array(), "{}", list.ctx());
    assert!(
        !h.data_root().join("sessions").exists(),
        "a read-only verb starts no session"
    );
    // The plan under --dry-run carries MUSE_NO_AUTO_UPDATE.
    let plan = h.omm_raw(&["--dry-run", "--json", "run", "--", "skills", "list"]);
    assert_eq!(plan.code, 0, "{}", plan.ctx());
    assert_eq!(plan.json["argv"], json!(["skills", "list"]));
    let names: Vec<&str> = plan.json["env"]
        .as_array()
        .expect("env")
        .iter()
        .filter_map(|e| e["name"].as_str())
        .collect();
    assert!(names.contains(&"MUSE_NO_AUTO_UPDATE"), "{names:?}");
    assert!(!h.omm_root().exists(), "run writes nothing");
}

// ---------------------------------------------------------------------------
// 9. omm hook <name>: JSON out, < 5 ms, fail-open
// ---------------------------------------------------------------------------

#[test]
fn s09_hook_dispatch_answers_in_json_under_five_ms_and_fails_open() {
    let h = e2e_or_skip!();
    let home = h.sb.home.clone();
    let one_json_line = |r: &Run| {
        assert_eq!(r.code, 0, "{}", r.ctx());
        assert!(r.stderr.is_empty(), "stderr must be empty: {}", r.ctx());
        assert_eq!(r.stdout.lines().count(), 1, "one line: {}", r.ctx());
        assert!(r.json.is_object(), "JSON: {}", r.ctx());
    };
    // Decisions travel in the JSON, the exit code is always 0.
    let deny = json!({"hook_event_name": "PreToolUse", "tool_name": "bash",
        "tool_input": {"command": "git push --force origin main"}, "tool_use_id": "t"});
    let (r, _) = h.hook("guard", deny.to_string().as_bytes(), Some(&home));
    one_json_line(&r);
    assert_eq!(r.json["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(r.json["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert!(r.json["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .expect("reason")
        .starts_with("omm guard: force-push-main"));
    let allow = json!({"hook_event_name": "PreToolUse", "tool_name": "bash",
        "tool_input": {"command": "cargo test"}});
    let (r, _) = h.hook("guard", allow.to_string().as_bytes(), Some(&home));
    one_json_line(&r);
    assert_eq!(r.json, json!({}));
    let stop = json!({"hook_event_name": "Stop", "stop_hook_active": false,
        "last_assistant_message": "All done, the feature is implemented."});
    let (r, _) = h.hook("stop", stop.to_string().as_bytes(), Some(&home));
    one_json_line(&r);
    assert_eq!(r.json["decision"], "block");
    let start = json!({"hook_event_name": "SessionStart", "source": "startup"});
    let (r, _) = h.hook("session-start", start.to_string().as_bytes(), Some(&home));
    one_json_line(&r);
    assert!(r.json["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("context")
        .starts_with("omm "));
    // Fail-open: an unknown handler, a handler whose context cannot be built
    // (no HOME at all), a handler whose state files are unreadable — always
    // one JSON object and exit 0, `{}` wherever no decision is possible
    // (R16: only exit 2 blocks). The guard is the one exception (Gate 1
    // decision G): a payload it cannot evaluate — malformed, empty, or cut
    // at the 4 MiB stdin bound — is a DENY carried in the JSON (still exit
    // 0), never a silent allow; every other hook keeps answering `{}`.
    let deny_s = deny.to_string();
    let start_s = start.to_string();
    let stop_active = b"{\"hook_event_name\":\"Stop\",\"stop_hook_active\":true}";
    let cases: [(&str, &[u8], Option<&Path>, bool); 6] = [
        ("no-such-hook", b"{}", Some(home.as_path()), true),
        ("stop", b"not json at all", Some(home.as_path()), true),
        ("session-start", b"", Some(home.as_path()), true),
        ("stop", stop_active, Some(home.as_path()), true),
        ("guard", deny_s.as_bytes(), None, false),
        (
            "session-start",
            start_s.as_bytes(),
            Some(Path::new("/nonexistent/omm-e2e-home")),
            false,
        ),
    ];
    for (name, payload, hook_home, must_be_empty) in cases {
        let (r, _) = h.hook(name, payload, hook_home);
        assert_eq!(
            r.code,
            0,
            "omm hook {name} (home {hook_home:?}): {}",
            r.ctx()
        );
        assert!(r.stderr.is_empty(), "omm hook {name}: {}", r.ctx());
        assert!(r.json.is_object(), "omm hook {name}: {}", r.ctx());
        if must_be_empty {
            assert_eq!(r.json, json!({}), "omm hook {name}: {}", r.ctx());
        }
    }
    // The guard's input bound (Gate 1 decision G): 4 MiB. A bash command
    // padded past the old 1 MiB cut used to arrive truncated, fail to parse
    // and pass as `{}`; now a 3 MiB payload is evaluated whole and denied on
    // its rule, and one past 4 MiB (or malformed, or empty) is denied for
    // being unevaluable — exit 0 in every case (fail-open at the process
    // level), the decision in the JSON.
    let padded = |n: usize| {
        json!({"hook_event_name": "PreToolUse", "tool_name": "bash",
            "tool_input": {"command": format!("rm -rf / #{}", "x".repeat(n))}, "tool_use_id": "t"})
        .to_string()
    };
    let three_mib = padded(3 << 20);
    let (r, _) = h.hook("guard", three_mib.as_bytes(), Some(&home));
    one_json_line(&r);
    assert_eq!(r.json["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        r.json["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .starts_with("omm guard: rm-root"),
        "{}",
        r.ctx()
    );
    let over = padded(4 << 20);
    let unevaluable: [&[u8]; 3] = [over.as_bytes(), b"not json at all", b""];
    for payload in unevaluable {
        let (r, _) = h.hook("guard", payload, Some(&home));
        one_json_line(&r);
        assert_eq!(
            r.json["hookSpecificOutput"]["permissionDecision"],
            "deny",
            "guard on an unevaluable payload ({} bytes): {}",
            payload.len(),
            r.ctx()
        );
        assert_eq!(r.json["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert!(
            r.json["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap_or("")
                .contains("too large or malformed to evaluate"),
            "{}",
            r.ctx()
        );
    }
    // Timing: 20 cold runs (a fresh process each), p50 < 5 ms. Three untimed
    // runs first so the binary's pages are warm; one retry against noise.
    let payload = json!({"hook_event_name": "PreToolUse", "tool_name": "bash",
        "tool_input": {"command": "ls -la"}})
    .to_string();
    for _ in 0..3 {
        let (r, _) = h.hook("guard", payload.as_bytes(), Some(&home));
        assert_eq!(r.code, 0);
    }
    let mut rounds = Vec::new();
    for _round in 0..2 {
        let mut samples: Vec<Duration> = (0..20)
            .map(|_| {
                let (r, t) = h.hook("guard", payload.as_bytes(), Some(&home));
                assert_eq!(r.code, 0, "{}", r.ctx());
                assert_eq!(r.json, json!({}));
                t
            })
            .collect();
        samples.sort();
        let p50 = (samples[9] + samples[10]) / 2;
        eprintln!(
            "omm hook guard, 20 cold runs: p50 {:?} min {:?} max {:?}",
            p50, samples[0], samples[19]
        );
        rounds.push(p50);
        if p50 < Duration::from_millis(5) {
            return;
        }
    }
    panic!("omm hook guard p50 over 5 ms in every round of 20 cold runs: {rounds:?} (R16)");
}

// ---------------------------------------------------------------------------
// 10. install under CI=true without --yes → refused, nothing written
// ---------------------------------------------------------------------------

#[test]
fn s10_install_under_ci_without_yes_is_refused_and_writes_nothing() {
    let h = e2e_or_skip!();
    let before = h.baseline();
    let r = h.omm_env(
        &["install", "--source", h.repo_str()],
        false,
        &[("CI", "true")],
    );
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(r.stderr.contains("--yes"), "{}", r.ctx());
    assert!(r.stderr.contains("CI=true"), "{}", r.ctx());
    assert!(r.stdout.is_empty(), "{}", r.ctx());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert_eq!(h.snapshot(), before, "nothing written anywhere");
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());
    assert!(h.marketplace_names().is_empty());
    // Same without CI but with stdin not a terminal: refused as well.
    let r = h.omm_env(&["install", "--source", h.repo_str()], false, &[]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(r.stderr.contains("not a terminal"), "{}", r.ctx());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    // --dry-run never needs consent and writes nothing under $OMM or the config root.
    let config_before: Snapshot = before
        .iter()
        .filter(|(k, _)| k.starts_with("config/"))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let r = h.omm_env(
        &["install", "--dry-run", "--source", h.repo_str()],
        false,
        &[("CI", "true")],
    );
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(r.json["dry_run"], true);
    assert!(
        !h.omm_root().exists(),
        "a dry run creates nothing under $OMM"
    );
    let config_after: Snapshot = h
        .snapshot()
        .into_iter()
        .filter(|(k, _)| k.starts_with("config/"))
        .collect();
    assert_eq!(
        config_after, config_before,
        "dry run touched the config root"
    );
    assert!(h.inspect().is_err());
    assert!(h.marketplace_names().is_empty());
    let data_after = h.snapshot();
    let foreign: Vec<&String> = data_after
        .keys()
        .filter(|k| before.get(*k) != data_after.get(*k))
        .filter(|k| !is_host_owned(k))
        .collect();
    assert!(
        foreign.is_empty(),
        "dry run wrote under the data root: {foreign:?}"
    );
}

// ---------------------------------------------------------------------------
// 11. a second install is a no-op
// ---------------------------------------------------------------------------

#[test]
fn s11_second_install_is_a_noop_with_a_byte_identical_ledger() {
    let h = e2e_or_skip!();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_bundle_installed(&h, &r.json);
    let ledger_bytes = std::fs::read(h.ledger_path()).expect("ledger");
    let settings_bytes = std::fs::read(h.settings_path()).expect("settings");
    let trust_bytes = std::fs::read(h.config_root().join("trust.json")).expect("trust");
    let agents_bytes = std::fs::read(h.config_root().join("AGENTS.md")).expect("AGENTS.md");
    let before = h.snapshot();

    let again = h.install();
    assert_eq!(again.code, 0, "{}", again.ctx());
    let converge = &again.json["converge"];
    assert_eq!(converge["noop"], true, "{}", again.ctx());
    assert_eq!(converge["total"]["updated"], 0);
    assert_eq!(converge["total"]["removed"], 0);
    assert_eq!(converge["total"]["backed_up"], 0);
    for (cat, counts) in converge["categories"].as_object().expect("categories") {
        assert_eq!(counts["updated"], 0, "{cat}: {counts}");
        assert_eq!(counts["removed"], 0, "{cat}: {counts}");
        assert_eq!(counts["backed_up"], 0, "{cat}: {counts}");
    }
    assert_eq!(again.json["bundle"]["plugin"], "unchanged");
    assert_eq!(again.json["bundle"]["marketplace"]["action"], "unchanged");
    assert_eq!(again.json["bundle"]["approved"], json!([]));
    assert_eq!(again.json["rules"]["action"], "unchanged");
    assert_eq!(again.json["themes"]["written"], json!([]));
    assert_eq!(
        again.json["themes"]["unchanged"]
            .as_array()
            .expect("unchanged")
            .len(),
        3
    );
    assert_eq!(again.json["settings"]["set"], json!([]));
    assert_eq!(again.json["trust"]["action"], "unchanged");
    assert_eq!(
        std::fs::read(h.ledger_path()).expect("ledger"),
        ledger_bytes
    );
    assert_eq!(
        std::fs::read(h.settings_path()).expect("settings"),
        settings_bytes
    );
    assert_eq!(
        std::fs::read(h.config_root().join("trust.json")).expect("trust"),
        trust_bytes
    );
    assert_eq!(
        std::fs::read(h.config_root().join("AGENTS.md")).expect("AGENTS.md"),
        agents_bytes
    );
    // Host view unchanged; the only files that moved are the host's own —
    // `$XDG_DATA_HOME/muse/plugins/**` is the host's store (ARCHITECTURE.md
    // §2; a rerun's `marketplace update omm` rotates a generation there by
    // design, §7) — plus omm's own audit log / locks under $OMM.
    assert_bundle_installed(&h, &again.json);
    let after = h.snapshot();
    let moved: Vec<&String> = after
        .keys()
        .chain(before.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .filter(|k| {
            !is_host_owned(k)
                && !k.starts_with("config/omm/")
                && !k.starts_with("data/muse/plugins/")
        })
        .collect();
    assert!(moved.is_empty(), "a no-op install moved: {moved:?}");
    let host_moved: Vec<&String> = after
        .keys()
        .chain(before.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .filter(|k| k.starts_with("data/muse/plugins/"))
        .collect();
    eprintln!("no-op install: host plugin-store paths that moved: {host_moved:?}");
}

// ---------------------------------------------------------------------------
// 12. doctor on a pristine install: zero warn / critical; --json parses
// ---------------------------------------------------------------------------

#[test]
fn s12_doctor_on_a_pristine_install_has_no_warn_or_critical_and_json_parses() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());

    let d = h.doctor(false);
    assert!(
        d.json.is_object(),
        "doctor --json does not parse: {}",
        d.ctx()
    );
    assert_eq!(d.stdout.lines().count(), 1, "one JSON line: {}", d.ctx());
    // D1–D15 from omm-doctor plus D16 (skill routing), appended by the CLI;
    // routing is off on a pristine install, so D16 is an Info row.
    assert_eq!(d.json["checks"].as_array().expect("checks").len(), 16);
    assert_eq!(check(&d.json, "D16")["severity"], "info", "{}", d.ctx());
    assert_eq!(d.json["host"]["plugin_id"], "oh-my-musecode");
    for c in d.json["checks"].as_array().expect("checks") {
        for key in ["id", "title", "severity", "observed", "why_silent"] {
            assert!(c[key].is_string(), "{key} missing in {c}");
        }
        assert!(
            matches!(c["severity"].as_str(), Some("info" | "warn" | "critical")),
            "{c}"
        );
    }
    assert_eq!(d.json["exit_code"], d.code);
    let pristine_rows = non_info_rows(&d.json);
    f.check(d.code == 0 && pristine_rows.is_empty(), || {
        format!(
            "doctor on a pristine bundle install (exit {}) reports non-info rows: {pristine_rows:?}",
            d.code
        )
    });
    let human = h.omm_raw(&["doctor"]);
    assert_eq!(human.code, d.code, "{}", human.ctx());
    assert!(
        human.stdout.starts_with("omm doctor — "),
        "{}",
        human.stdout
    );
    assert!(human.stdout.contains("16 checks:"), "{}", human.stdout);
    // `--fast` skips the two slow probes; a skipped probe is an Info row
    // (Gate 1: D8 warned "no live measurement" on every --fast run, so a
    // fast doctor could never be green on a pristine install).
    let fast = h.doctor(true);
    assert!(fast.json.is_object(), "{}", fast.ctx());
    let fast_rows = non_info_rows(&fast.json);
    f.check(fast.code == 0 && fast_rows.is_empty(), || {
        format!(
            "doctor --fast on a pristine bundle install (exit {}) reports non-info rows: {fast_rows:?}",
            fast.code
        )
    });
    assert!(
        check(&fast.json, "D8")["observed"]
            .as_str()
            .expect("observed")
            .contains("--fast"),
        "{}",
        fast.ctx()
    );

    // With the provider set (the D2 fix), is anything else non-info?
    let set = h.omm(&["settings", "set", "provider", "meta"]);
    assert_eq!(set.code, 0, "{}", set.ctx());
    let d = h.doctor(false);
    assert!(d.json.is_object(), "{}", d.ctx());
    let rows = non_info_rows(&d.json);
    f.check(d.code == 0 && rows.is_empty(), || {
        format!(
            "doctor after `omm settings set provider meta` (exit {}) still reports non-info rows: {rows:?}",
            d.code
        )
    });
    f.finish("scenario 12");
}

// ---------------------------------------------------------------------------
// 13. omm build --check passes on the committed tree
// ---------------------------------------------------------------------------

#[test]
fn s13_build_check_passes_on_the_committed_tree() {
    let h = e2e_or_skip!();
    let repo_agents_before = h.repo.join("AGENTS.md").exists();
    let r = h.omm_raw(&["--json", "build", "--check", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(r.json["clean"], true, "{}", r.ctx());
    assert_eq!(r.json["exit_code"], 0);
    assert_eq!(r.json["digest_checked"], true, "{}", r.ctx());
    assert_eq!(r.json["drifts"], json!([]), "{}", r.ctx());
    assert_eq!(r.json["repo"], Value::String(h.repo_str().to_string()));
    let human = h.omm_raw(&["build", "--check", h.repo_str()]);
    assert_eq!(human.code, 0, "{}", human.ctx());
    assert!(human.stdout.contains("no drift"), "{}", human.stdout);
    // The committed digest is the one the binary computes for plugins/omm.
    let c = std::fs::read(h.repo.join("marketplace.json")).expect("marketplace.json");
    assert!(String::from_utf8_lossy(&c).contains(&committed_digest(&h.repo)));
    // Nothing of that ran against the sandbox roots or the repository as cwd.
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    for name in ["AGENTS.md", "settings.json", "trust.json", "themes"] {
        assert!(
            !h.config_root().join(name).exists(),
            "build --check wrote {name} into the config root"
        );
    }
    assert_eq!(
        h.repo.join("AGENTS.md").exists(),
        repo_agents_before,
        "the host scaffolded into the repository root"
    );
}

// ---------------------------------------------------------------------------
// 14. a pre-existing AGENTS.md (and settings.json) → install → uninstall →
//     byte- and mode-identical; a user-region edit survives as the user
//     region alone (Gate 1: the folded original was unlinked, its only
//     backup gone with $OMM/snapshots; a user-region edit kept the whole
//     file, stale managed block included). Round 5 (decision H2): omm owns
//     ONLY the managed block — a pre-existing file gets the block inserted
//     above it, its own bytes untouched; scenario 23 has the rest of the
//     file model
// ---------------------------------------------------------------------------

#[test]
fn s14_pre_existing_rules_come_back_byte_for_byte_and_a_user_region_edit_survives_alone() {
    use std::os::unix::fs::PermissionsExt;
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let agents_path = h.config_root().join("AGENTS.md");
    let original = b"my own rules\n- never delete me";
    std::fs::create_dir_all(h.config_root()).expect("config root");
    std::fs::write(&agents_path, original).expect("seed AGENTS.md");
    std::fs::set_permissions(&agents_path, std::fs::Permissions::from_mode(0o644)).expect("mode");
    // The user's own settings.json: one line, no trailing newline, private
    // (0600), holding two keys the host does not type — a top-level one and
    // a `tui` member — which the host destroys on its next settings rewrite
    // (Gate 1: `plugins install/approve` did exactly that during install,
    // silently, and the file came back 0644 with the keys gone; a compact
    // trust.json came back pretty-printed).
    let seed_bytes: &[u8] = br#"{"schema_version":1,"provider":"meta","my_custom_top_level":{"keep":true},"tui":{"theme":"ayu-dark","my_tui_extra":42}}"#;
    std::fs::write(h.settings_path(), seed_bytes).expect("seed settings.json");
    std::fs::set_permissions(h.settings_path(), std::fs::Permissions::from_mode(0o600))
        .expect("mode");
    let trust_path = h.config_root().join("trust.json");
    let trust_bytes: &[u8] =
        br#"{"schema_version":1,"projects":{"/Users/someone/other":{"decision":"trusted"}}}"#;
    std::fs::write(&trust_path, trust_bytes).expect("seed trust.json");
    std::fs::set_permissions(&trust_path, std::fs::Permissions::from_mode(0o644)).expect("mode");
    let before = h.baseline();

    // Without the explicit opt-in the bundle install refuses before its
    // first host mutation, naming both keys and the flag; nothing is written.
    let refused = h.install();
    assert_eq!(refused.code, 2, "{}", refused.ctx());
    for needle in [
        "my_custom_top_level",
        "tui.my_tui_extra",
        "--drop-unknown-settings-keys",
    ] {
        assert!(
            refused.stderr.contains(needle),
            "{needle}: {}",
            refused.ctx()
        );
    }
    assert!(!h.omm_root().exists(), "a refused install writes nothing");
    assert!(h.inspect().is_err() && h.marketplace_names().is_empty());
    assert_eq!(h.snapshot(), before, "a refused install touched the roots");

    let r = h.omm(&[
        "install",
        "--source",
        h.repo_str(),
        "--drop-unknown-settings-keys",
    ]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(r.json["rules"]["action"], "updated", "{}", r.ctx());
    assert!(
        r.json["rules"]["note"]
            .as_str()
            .expect("note")
            .contains("inserted at the top; your file is kept below"),
        "{}",
        r.ctx()
    );
    // Round 5 (decision H2): the block goes in above the user's file and
    // the file's own bytes are never touched — no fold into a user region.
    let seeded = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    assert!(
        seeded.starts_with("<!-- omm:managed-start -->\n"),
        "{seeded}"
    );
    assert!(
        seeded.contains("<!-- omm:managed-end -->\n\nmy own rules\n- never delete me"),
        "{seeded}"
    );
    assert!(
        seeded.ends_with("my own rules\n- never delete me"),
        "{seeded}"
    );
    assert!(!seeded.contains("<!-- omm:user-start -->"), "{seeded}");
    let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o777;
    assert_eq!(
        mode(&agents_path),
        0o644,
        "the rewrite keeps the file's mode"
    );
    // The original bytes live in the ledger entry, not only in a snapshot.
    let ledger = h.ledger().expect("ledger");
    let entry = ledger["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["path"] == "AGENTS.md")
        .expect("AGENTS.md entry");
    assert_eq!(entry["kind"], "rules");
    assert_eq!(
        entry["prior"]["replaced"]["text"],
        String::from_utf8_lossy(original).as_ref(),
        "{entry}"
    );
    // …and the frame the insertion contributed (the blank line after the
    // block), so uninstall takes exactly that away again.
    assert_eq!(entry["prior"]["frame"]["mid"], "\n\n", "{entry}");
    assert_eq!(entry["prior"]["frame"]["pre"], "", "{entry}");
    // So do the shared files' pre-omm bytes and modes.
    for (path, bytes, expected_mode) in [
        ("settings.json", seed_bytes, "0600"),
        ("trust.json", trust_bytes, "0644"),
    ] {
        let entry = ledger["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .find(|e| e["path"] == path)
            .unwrap_or_else(|| panic!("{path} entry in {ledger}"));
        assert_eq!(entry["class"], "shared-key", "{entry}");
        assert_eq!(
            entry["prior"]["original"]["text"],
            String::from_utf8_lossy(bytes).as_ref(),
            "{entry}"
        );
        assert_eq!(entry["prior"]["original"]["mode"], expected_mode, "{entry}");
    }
    // The host's rewrite dropped the two keys during `plugins install` /
    // `approve` (what the flag accepted) and landed its own 0644; after its
    // last host verb the install put the keys back (the host's loader
    // ignores them until its next rewrite), so D4 keeps warning about the
    // doomed keys, as the refusal text promised, and the file stays private.
    // Since 1.3.0-R3057.1 the host rewrite preserves the mode bits (Gate 1
    // decision F now holds trivially: nothing to re-apply); on 1.0.x the
    // rewrite landed 0644 and `mode_reapplied` was true.
    let now = h.settings();
    assert_eq!(now["my_custom_top_level"]["keep"], true, "{now}");
    assert_eq!(now["tui"]["my_tui_extra"], 42, "{now}");
    assert_eq!(
        mode(&h.settings_path()),
        0o600,
        "settings.json mode after the bundle's host rewrites"
    );
    assert_eq!(
        r.json["settings_after_host"]["mode_reapplied"],
        false,
        "{}",
        r.ctx()
    );
    let d4 = check(&h.doctor(true).json, "D4");
    assert_eq!(d4["severity"], "warn", "{d4}");
    assert!(
        d4["observed"]
            .as_str()
            .expect("observed")
            .contains("my_custom_top_level"),
        "{d4}"
    );

    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let rules = dry.json["preview"]["rules"].as_array().expect("rules");
    assert_eq!(rules.len(), 1, "{}", dry.ctx());
    assert!(
        rules[0]["action"]
            .as_str()
            .expect("action")
            .contains("restored to the file that was there before omm"),
        "{}",
        dry.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(&agents_path).expect("AGENTS.md"),
        seeded
    );

    let shared: Vec<String> = dry.json["preview"]["shared"]
        .as_array()
        .expect("preview.shared")
        .iter()
        .filter_map(|s| s["path"].as_str().map(str::to_string))
        .collect();
    assert!(
        shared.iter().any(|p| p.ends_with("settings.json"))
            && shared.iter().any(|p| p.ends_with("trust.json")),
        "the preview names both shared files: {}",
        dry.ctx()
    );

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(un.json["report"]["rules_rewritten"], 1, "{}", un.ctx());
    assert_eq!(
        std::fs::read(&agents_path).expect("AGENTS.md"),
        original,
        "byte-identical, no trailing newline added"
    );
    assert_eq!(mode(&agents_path), 0o644);
    // Both shared files come back byte for byte — unknown keys, one-line
    // formatting, no trailing newline — and at their own modes: after the
    // targeted restores the documents equal the pre-omm ones, so the
    // recorded bytes land (the host had rewritten settings.json at 0644).
    assert_eq!(
        std::fs::read(h.settings_path()).expect("settings.json"),
        seed_bytes,
        "settings.json back to the user's bytes"
    );
    assert_eq!(mode(&h.settings_path()), 0o600);
    assert_eq!(
        std::fs::read(&trust_path).expect("trust.json"),
        trust_bytes,
        "trust.json back to the user's bytes"
    );
    assert_eq!(mode(&trust_path), 0o644);
    assert_eq!(un.json["report"]["shared_restored"], 2, "{}", un.ctx());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 14 (pre-existing rules)");

    // The second half installs without the opt-in: the user's keys must be
    // out of the file first (they are theirs to move), the rest stays.
    let seed = json!({"schema_version": 1, "provider": "meta", "tui": {"theme": "ayu-dark"}});
    h.write_settings(&seed);

    // Second half: a fresh seed, a rule added inside the user region, then
    // uninstall → the file holds the user region alone (no managed block
    // pointing at the removed plugin).
    std::fs::remove_file(&agents_path).expect("clean slate");
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(
        r.json["rules"]["note"].as_str().expect("note"),
        "written from the template",
        "{}",
        r.ctx()
    );
    let seeded = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    let edited = seeded.replace(
        "<!-- omm:user-end -->",
        "- MY PERSONAL RULE\n<!-- omm:user-end -->",
    );
    assert_ne!(edited, seeded);
    std::fs::write(&agents_path, &edited).expect("edit");
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(un.json["report"]["rules_rewritten"], 1, "{}", un.ctx());
    let left = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    assert!(left.contains("- MY PERSONAL RULE"), "{left}");
    assert!(!left.contains("omm:managed-start"), "{left}");
    assert!(
        !left.contains("read_skill"),
        "the managed block is gone: {left}"
    );
    assert!(
        !left.contains("omm:user-start"),
        "the markers are gone: {left}"
    );
    assert!(h.ledger().is_none());
    assert!(h.inspect().is_err());
    // A plain seed left untouched is removed outright (the placeholder in
    // the template's user region is omm's, not the user's).
    std::fs::remove_file(&agents_path).expect("clean slate");
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert!(!agents_path.exists(), "an untouched seed is removed");
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(un.json["report"]["ledger_removed"], true);
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    assert!(!h.config_root().join("themes").exists());
    // The user's own settings.json and trust.json are kept, back at their
    // documents, as every uninstall of this scenario leaves them.
    assert_eq!(h.settings(), seed);
    assert_eq!(std::fs::read(&trust_path).expect("trust.json"), trust_bytes);
}

// ---------------------------------------------------------------------------
// 15. an interrupted install leaves nothing uninstall cannot undo (the ledger
//     is saved after every host mutation — Gate 1: a SIGINT after
//     `plugins install` left settings.json, AGENTS.md and up to twelve
//     managed skills behind with a ledger that knew none of them), and what
//     the host holds beyond the ledger (the step before its save) is named
//     and refused, or removed with `--reconcile-host` — never left behind
//     under rc 0
// ---------------------------------------------------------------------------

#[test]
fn s15_an_interrupted_bundle_install_is_undone_by_uninstall() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let settings = h.settings_path();
    // The host creates settings.json on the first `plugins approve`: the
    // moment it appears the process is killed (approvals, rules, themes,
    // profile and trust are all still ahead).
    let fired = h.omm_killed_when(&["install", "--source", h.repo_str()], || settings.exists());
    assert!(fired, "the install finished before settings.json appeared");
    let ledger = h.ledger();
    f.check(ledger.is_some(), || {
        "no ledger after `plugins install`: the registrations were not saved".into()
    });
    if let Some(l) = &ledger {
        let regs = l["registrations"].as_array().cloned().unwrap_or_default();
        f.check(regs.iter().any(|r| r["kind"] == "muse-marketplace"), || {
            format!("the marketplace registration is missing: {l}")
        });
        f.check(regs.iter().any(|r| r["kind"] == "muse-plugin"), || {
            format!("the plugin registration is missing: {l}")
        });
        let entries = l["entries"].as_array().cloned().unwrap_or_default();
        f.check(
            entries
                .iter()
                .any(|e| e["path"] == "settings.json" && e["class"] == "seeded"),
            || format!("settings.json, created by the host on omm's behalf, is not ledgered as seeded: {l}"),
        );
    }
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    f.check(!settings.exists(), || {
        format!(
            "settings.json left behind after uninstalling the interrupted install: {}",
            std::fs::read_to_string(&settings).unwrap_or_default()
        )
    });
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 15 (interrupted bundle install)");
}

#[test]
fn s15b_an_interrupted_no_plugin_install_is_at_most_one_skill_ahead_of_its_ledger() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let fired = h.omm_killed_when(
        &["install", "--no-plugin", "--source", h.repo_str()],
        || h.store_skill_ids().len() >= 3,
    );
    assert!(fired, "the install finished before three skills landed");
    let on_disk = h.store_skill_ids();
    let ledgered = h.ledgered_skill_ids();
    eprintln!("killed with {on_disk:?} on disk, ledger knows {ledgered:?}");
    // Every skill but the one the host may have finished after the kill is
    // in the ledger (it is saved after every `muse skills install`).
    f.check(on_disk.len() <= ledgered.len() + 1, || {
        format!(
            "the ledger fell {} skills behind the store: on disk {on_disk:?}, ledgered {ledgered:?}",
            on_disk.len().saturating_sub(ledgered.len())
        )
    });
    f.check(ledgered.iter().all(|id| on_disk.contains(id)), || {
        format!("the ledger lists a skill that is not on disk: {ledgered:?} vs {on_disk:?}")
    });
    // The in-flight skill the host finished after the kill is in the store
    // and not in the ledger: `omm uninstall` names it and refuses; with
    // `--reconcile-host` it goes with the rest (Gate 1: the test removed
    // it by hand first, and without that the uninstall reported rc 0 with
    // the skill still in the store).
    let extra: Vec<String> = on_disk
        .iter()
        .filter(|id| !ledgered.contains(id))
        .cloned()
        .collect();
    let plain = h.omm(&["uninstall"]);
    let un = if ledgered.is_empty() || !extra.is_empty() {
        assert_eq!(plain.code, 1, "{}", plain.ctx());
        assert!(plain.stderr.contains("--reconcile-host"), "{}", plain.ctx());
        for id in &extra {
            assert!(plain.stderr.contains(id.as_str()), "{}", plain.ctx());
        }
        assert_eq!(
            h.store_skill_ids(),
            on_disk,
            "nothing removed without the flag"
        );
        assert_eq!(h.ledger().is_some(), !ledgered.is_empty());
        let un = h.omm(&["uninstall", "--reconcile-host"]);
        assert_eq!(un.code, 0, "{}", un.ctx());
        let reconciled = strings(&un.json["host_reconciled"]);
        f.check(
            extra
                .iter()
                .all(|id| reconciled.contains(&format!("muse skills uninstall {id}"))),
            || format!("--reconcile-host did not report the in-flight skill(s) {extra:?}: {reconciled:?}"),
        );
        un
    } else {
        assert_eq!(plain.code, 0, "{}", plain.ctx());
        assert_eq!(plain.json["host_reconciled"], json!([]), "{}", plain.ctx());
        plain
    };
    if !ledgered.is_empty() {
        assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    }
    assert!(h.store_skill_ids().is_empty(), "{:?}", h.store_skill_ids());
    assert!(h.skills_listed("user").is_empty());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 15b (interrupted --no-plugin install)");
}

#[test]
fn s15c_a_bundle_install_killed_as_the_host_lists_the_plugin_is_undone_with_reconcile_host() {
    // Gate 1: SIGKILL the instant `installed.json` lists `omm` — between
    // `plugins install` returning and the ledger save — left a ledger that
    // knew the marketplace only; `omm uninstall` removed that, reported
    // rc 0 with no errors, and left the plugin served from the host cache.
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let installed = h.data_root().join("plugins").join("installed.json");
    let lists_omm = || {
        std::fs::read(&installed)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| {
                v["plugins"]
                    .as_object()
                    .map(|m| m.contains_key("oh-my-musecode"))
            })
            .unwrap_or(false)
    };
    let fired = h.omm_killed_when(&["install", "--source", h.repo_str()], lists_omm);
    assert!(
        fired,
        "the install finished before the host listed the plugin"
    );
    assert!(
        h.installed_plugin_ids()
            .contains(&"oh-my-musecode".to_string()),
        "the host must hold the plugin the trigger saw"
    );
    let ledger = h.ledger();
    let regs = ledger
        .as_ref()
        .and_then(|l| l["registrations"].as_array().cloned())
        .unwrap_or_default();
    let has_plugin = regs.iter().any(|r| r["kind"] == "muse-plugin");
    eprintln!(
        "killed with the plugin listed; ledger present: {}, plugin registered: {has_plugin}",
        ledger.is_some()
    );
    f.check(regs.iter().any(|r| r["kind"] == "muse-marketplace"), || {
        format!("the marketplace registration is missing: {ledger:?}")
    });
    // Whatever the ledger got to record: the plain uninstall either undoes
    // the plugin (registered) or refuses and names it — never rc 0 with
    // the plugin left behind.
    let plain = h.omm(&["uninstall"]);
    let un = if has_plugin {
        assert_eq!(plain.code, 0, "{}", plain.ctx());
        assert_eq!(plain.json["host_reconciled"], json!([]), "{}", plain.ctx());
        plain
    } else {
        assert_eq!(plain.code, 1, "{}", plain.ctx());
        assert!(
            plain.stderr.contains("plugin `oh-my-musecode`")
                && plain.stderr.contains("--reconcile-host"),
            "{}",
            plain.ctx()
        );
        assert!(
            h.installed_plugin_ids()
                .contains(&"oh-my-musecode".to_string()),
            "nothing removed without the flag"
        );
        assert_eq!(h.ledger().is_some(), ledger.is_some());
        let un = h.omm(&["uninstall", "--reconcile-host"]);
        assert_eq!(un.code, 0, "{}", un.ctx());
        f.check(
            strings(&un.json["host_reconciled"])
                .contains(&"muse plugins remove oh-my-musecode --delete-data".to_string()),
            || format!("--reconcile-host did not report the plugin: {}", un.ctx()),
        );
        un
    };
    if ledger.is_some() {
        assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    }
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    f.check(!h.settings_path().exists(), || {
        format!(
            "settings.json left behind after uninstalling the interrupted install: {}",
            std::fs::read_to_string(h.settings_path()).unwrap_or_default()
        )
    });
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 15c (bundle install killed as the host lists the plugin)");
}

// ---------------------------------------------------------------------------
// 16. --no-plugin with the user's own compact settings.json and trust.json →
//     install → uninstall → byte- and mode-identical (Gate 1: both came back
//     pretty-printed; `muse skills uninstall` rewrote settings.json to 0644
//     without the keys the host does not type)
// ---------------------------------------------------------------------------

#[test]
fn s16_no_plugin_install_uninstall_returns_compact_shared_files_byte_for_byte() {
    use std::os::unix::fs::PermissionsExt;
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    std::fs::create_dir_all(h.config_root()).expect("config root");
    let seed_bytes: &[u8] = br#"{"schema_version":1,"provider":"meta","my_custom_top_level":{"keep":true},"tui":{"theme":"ayu-dark","my_tui_extra":42}}"#;
    std::fs::write(h.settings_path(), seed_bytes).expect("seed settings.json");
    std::fs::set_permissions(h.settings_path(), std::fs::Permissions::from_mode(0o600))
        .expect("mode");
    let trust_path = h.config_root().join("trust.json");
    let trust_bytes: &[u8] =
        br#"{"schema_version":1,"projects":{"/Users/someone/other":{"decision":"trusted"}}}"#;
    std::fs::write(&trust_path, trust_bytes).expect("seed trust.json");
    let before = h.baseline();
    let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o777;

    // `muse skills install` does not rewrite settings.json, and omm's own
    // patch keeps every key: no opt-in is needed, the keys survive the
    // install lifetime (D4 keeps warning that the host will drop them).
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let now = h.settings();
    assert_eq!(now["my_custom_top_level"]["keep"], true, "{now}");
    assert_eq!(now["tui"]["my_tui_extra"], 42, "{now}");
    assert_eq!(
        mode(&h.settings_path()),
        0o600,
        "omm's rewrite keeps the mode"
    );
    assert_eq!(
        now["run"]["context_slimming"]["skill_catalog_descriptions"],
        "first_sentence"
    );

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    // `muse skills uninstall` (twelve times) rewrote settings.json without
    // the two keys and at 0644; the keys omm captured before its first host
    // step come back, the document then equals the pre-omm one, and the
    // recorded bytes and mode land.
    assert_eq!(
        std::fs::read(h.settings_path()).expect("settings.json"),
        seed_bytes
    );
    assert_eq!(mode(&h.settings_path()), 0o600);
    assert_eq!(std::fs::read(&trust_path).expect("trust.json"), trust_bytes);
    assert!(h.ledger().is_none());
    assert!(h.skills_listed("user").is_empty());
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 16 (--no-plugin, compact shared files)");
}

// ---------------------------------------------------------------------------
// 17. the host lost a registration by hand (marketplace, plugin, a managed
//     skill) → uninstall still completes: the undo is idempotent — `not
//     installed` / `not configured` / `skill not installed` count as done,
//     the registration is dropped and audited with the reason (Gate 1
//     decision B; round 4: every rerun failed identically and $OMM stayed)
// ---------------------------------------------------------------------------

#[test]
fn s17_uninstall_completes_after_the_host_lost_a_registration_by_hand() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let dropped_reasons = |doc: &Value| -> Vec<String> {
        doc["preview"]["dropped"]
            .as_array()
            .map(|d| {
                d.iter()
                    .filter_map(|x| x["reason"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    // (a) `muse plugins marketplace remove omm` by hand.
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let out = h
        .invoker()
        .run(&["plugins", "marketplace", "remove", "omm", "--json"])
        .expect("marketplace remove");
    assert!(out.ok(), "{} {}", out.stdout, out.stderr);
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(
        un.json["report"]["registrations_dropped"]
            .as_u64()
            .unwrap_or(0)
            >= 1,
        "{}",
        un.ctx()
    );
    let reasons = dropped_reasons(&un.json);
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("already gone") && r.contains("omm")),
        "{reasons:?}"
    );
    assert_bundle_uninstalled(&h, &un.json);
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);

    // (b) `muse plugins remove oh-my-musecode --delete-data` by hand.
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let out = h
        .invoker()
        .run(&[
            "plugins",
            "remove",
            "oh-my-musecode",
            "--delete-data",
            "--json",
        ])
        .expect("plugins remove");
    assert!(out.ok(), "{} {}", out.stdout, out.stderr);
    assert!(h.inspect().is_err());
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    let reasons = dropped_reasons(&un.json);
    assert!(
        reasons
            .iter()
            .any(|r| r.contains("already gone") && r.contains("plugin `oh-my-musecode`")),
        "{reasons:?}"
    );
    assert_bundle_uninstalled(&h, &un.json);
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);

    // (c) --no-plugin: `muse skills uninstall omm-docs` by hand.
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let out = h
        .invoker()
        .run(&["skills", "uninstall", "omm-docs", "--json"])
        .expect("skills uninstall");
    assert!(out.ok(), "{} {}", out.stdout, out.stderr);
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.store_skill_ids().is_empty(), "{:?}", h.store_skill_ids());
    assert!(h.skills_listed("user").is_empty());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 17 (host registrations removed by hand)");
}

// ---------------------------------------------------------------------------
// 18. plan-then-execute install (Gate 1 decision A): `themes/` symlinked out
//     of the base by a dotfile manager → the whole plan is validated before
//     the first host mutation and refused with the exact path and the way
//     out; NOTHING is written. `--skip themes` installs the rest;
//     `OMM_THEMES_DIR=<dir>` (inside the config root) places the themes
//     elsewhere; one outside the root is refused up front too.
// ---------------------------------------------------------------------------

#[test]
fn s18_install_refuses_up_front_when_themes_escape_the_base_and_skip_themes_installs() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let dotfiles = h.sb.root.join("dotfiles").join("themes");
    std::fs::create_dir_all(&dotfiles).expect("dotfiles");
    std::fs::write(dotfiles.join("mine.tmTheme"), b"<mine/>").expect("theme");
    std::fs::create_dir_all(h.config_root()).expect("config root");
    std::os::unix::fs::symlink(&dotfiles, h.config_root().join("themes")).expect("symlink");
    let before = h.baseline();
    let dotfiles_untouched = |h: &E2e| {
        let names: Vec<String> = std::fs::read_dir(&dotfiles)
            .expect("dotfiles")
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["mine.tmTheme".to_string()], "{names:?}");
        assert!(h.config_root().join("themes").is_symlink());
    };

    // Refused before the first write: nothing under $OMM, nothing on the
    // host, no file in the config root, the roots byte-identical.
    let r = h.install();
    assert_eq!(r.code, 2, "{}", r.ctx());
    for needle in [
        "themes",
        "outside the base",
        "omm install --skip themes",
        "OMM_THEMES_DIR",
    ] {
        assert!(r.stderr.contains(needle), "{needle}: {}", r.ctx());
    }
    assert!(!h.omm_root().exists(), "a refused install wrote under $OMM");
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());
    assert!(h.marketplace_names().is_empty());
    for name in ["AGENTS.md", "settings.json", "trust.json"] {
        assert!(
            !h.config_root().join(name).exists(),
            "{name} written by a refused install"
        );
    }
    let before_sans = before
        .iter()
        .filter(|(k, _)| !is_host_owned(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect::<Snapshot>();
    assert_eq!(
        h.snapshot_sans_host_residue(),
        before_sans,
        "a refused install touched the roots"
    );
    dotfiles_untouched(&h);

    // `--skip themes`: the plan without the themes step runs whole.
    let r = h.omm(&["install", "--source", h.repo_str(), "--skip", "themes"]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(
        strings(&r.json["skipped_steps"]),
        vec!["themes".to_string()]
    );
    assert_eq!(
        r.json["themes"]["step_skipped"],
        "--skip themes",
        "{}",
        r.ctx()
    );
    assert_eq!(r.json["mode"], "plugin");
    assert!(h.inspect().is_ok(), "the bundle is installed");
    assert!(h.config_root().join("AGENTS.md").is_file());
    let ledger = h.ledger().expect("ledger");
    assert_eq!(
        ledger["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .filter(|e| e["kind"] == "theme")
            .count(),
        0,
        "no theme is ledgered"
    );
    dotfiles_untouched(&h);
    let d = h.doctor(true);
    let rows = non_info_rows(&d.json);
    f.check(d.code == 0 && rows.is_empty(), || {
        format!(
            "doctor after `omm install --skip themes` (exit {}): {rows:?}",
            d.code
        )
    });
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.ledger().is_none() && !h.omm_root().exists());
    dotfiles_untouched(&h);
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);

    // `OMM_THEMES_DIR` inside the config root: the themes land there, are
    // ledgered under that path, and come back out at uninstall.
    let alt = h.config_root().join("omm-themes");
    let alt_str = alt.to_str().expect("utf-8");
    let r = h.omm_env(
        &["install", "--source", h.repo_str()],
        true,
        &[("OMM_THEMES_DIR", alt_str)],
    );
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(alt.read_dir().expect("alt themes").count(), 3);
    let ledger = h.ledger().expect("ledger");
    assert!(
        ledger["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .any(|e| e["kind"] == "theme" && e["path"] == "omm-themes/omm-carbon.tmTheme"),
        "{ledger}"
    );
    dotfiles_untouched(&h);
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(!alt.exists(), "the alternative themes dir is pruned");
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);

    // `OMM_THEMES_DIR` outside the config root is refused up front (R4: the
    // four bases), nothing written.
    let r = h.omm_env(
        &["install", "--source", h.repo_str()],
        true,
        &[("OMM_THEMES_DIR", dotfiles.to_str().expect("utf-8"))],
    );
    assert_eq!(r.code, 2, "{}", r.ctx());
    assert!(r.stderr.contains("OMM_THEMES_DIR"), "{}", r.ctx());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(h.inspect().is_err() && h.marketplace_names().is_empty());
    assert_eq!(h.snapshot_sans_host_residue(), before_sans);
    dotfiles_untouched(&h);
    f.finish("scenario 18 (themes escape the base)");
}

// ---------------------------------------------------------------------------
// 19. structural keys (Gate 1 decision D): `permissions.schema_version` is a
//     required member of a deny_unknown_fields struct; a profile switch and
//     an uninstall restoring that leaf to absent keep it while the user's
//     own members remain under `permissions`, so the host keeps loading the
//     user's named profiles; doctor D4 names a `permissions` object lacking
//     it, with the fix
// ---------------------------------------------------------------------------

#[test]
fn s19_a_profile_switch_and_uninstall_keep_permissions_schema_version_for_the_users_profile() {
    let h = e2e_or_skip!();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    let s = h.omm(&["profile", "use", "strict"]);
    assert_eq!(s.code, 0, "{}", s.ctx());
    assert_eq!(h.settings()["permissions"]["schema_version"], 1);
    assert_eq!(h.settings()["permissions"]["default_profile"], "omm-strict");
    // The user adds a named profile of their own beside omm's, extending a
    // different builtin base. On 1.3.0-R3057.1 two profiles extending the
    // same builtin base fail every session closed (`missing field
    // schema_version`); a `:read-only` child beside the `:ask-me` one
    // loads.
    let mut doc = h.settings();
    doc["permissions"]["profiles"]["mine"] = json!({"extends": ":read-only"});
    h.write_settings(&doc);
    assert!(
        !s19_profiles_unavailable(&h),
        "the host accepts the object as seeded"
    );
    s19_switch_back_to_default(&h);
    s19_planted_gap_heals(&h);
    s19_uninstall_keeps_user_profile(&h);
}

/// The host's verdict on named permission profiles, from a real session.
///
/// A small free function on purpose: on 1.3.0-R3057.1 a failing session's
/// shutdown writeback can persist a `schema_version`-less object and fail
/// the CLI lane, and that path reacts to the client shape around the probe
/// — one single-shot session per call, no retries or extra witnesses.
fn s19_profiles_unavailable(h: &E2e) -> bool {
    let out = h
        .invoker()
        .run(&["exec", "--provider", "echo", "hi"])
        .expect("exec echo");
    // 1.3.0 refuses a `schema_version`-less object with exit 1; older hosts
    // failed the lane silently at exit 0. Either way the marker on stderr
    // is the verdict — anything else is a broken session, not a verdict.
    if out
        .stderr
        .contains("Named permission profiles are unavailable")
    {
        return true;
    }
    assert!(out.ok(), "echo session: {:?} {}", out.code, out.stderr);
    false
}

/// Switching back to `default`: the structural member is kept, the managed
/// profile is gone, and the user's own profile survives whole.
fn s19_switch_back_to_default(h: &E2e) {
    let back = h.omm(&["profile", "use", "default"]);
    assert_eq!(back.code, 0, "{}", back.ctx());
    let s = h.settings();
    assert_eq!(
        s["permissions"]["schema_version"], 1,
        "kept: the user's profile needs the structural member: {s}"
    );
    assert!(s["permissions"].get("default_profile").is_none(), "{s}");
    assert!(
        s["permissions"]["profiles"].get("omm-strict").is_none(),
        "{s}"
    );
    assert_eq!(
        s["permissions"]["profiles"]["mine"]["extends"],
        ":read-only"
    );
    let kept = strings(&back.json["kept_structural"]);
    assert!(
        kept.iter().any(|k| k == "permissions.schema_version"),
        "{}",
        back.ctx()
    );
    assert!(
        !s19_profiles_unavailable(h),
        "the host must still load the user's named profile after the switch"
    );
    let d4 = check(&h.doctor(true).json, "D4");
    assert_eq!(d4["severity"], "info", "{d4}");
}

/// Planted: the structural member removed by hand. The host then refuses
/// the whole object silently for every CLI lane but stderr; D4 says so
/// and the printed fix heals it.
fn s19_planted_gap_heals(h: &E2e) {
    let mut doc = h.settings();
    doc["permissions"]
        .as_object_mut()
        .expect("permissions object")
        .remove("schema_version");
    h.write_settings(&doc);
    assert!(
        s19_profiles_unavailable(h),
        "the host refuses the object without it"
    );
    let d4 = check(&h.doctor(true).json, "D4");
    assert_eq!(d4["severity"], "warn", "{d4}");
    assert!(
        d4["observed"]
            .as_str()
            .expect("observed")
            .contains("permissions")
            && d4["observed"]
                .as_str()
                .expect("observed")
                .contains("schema_version"),
        "{d4}"
    );
    let fix = d4["fix"].as_str().expect("fix").to_string();
    assert_eq!(fix, "omm settings set permissions.schema_version 1", "{d4}");
    let argv: Vec<&str> = fix.split_whitespace().skip(1).collect();
    let fixed = h.omm(&argv);
    assert_eq!(fixed.code, 0, "{}", fixed.ctx());
    assert_eq!(check(&h.doctor(true).json, "D4")["severity"], "info");
    assert!(!s19_profiles_unavailable(h));
}

/// Uninstall: the leaf omm set is registered with prior absent, but the
/// user's object still needs it — kept and named; the user's profile
/// survives whole and the host keeps loading it.
fn s19_uninstall_keeps_user_profile(h: &E2e) {
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let dropped: Vec<String> = dry.json["preview"]["dropped"]
        .as_array()
        .expect("dropped")
        .iter()
        .filter_map(|d| d["reason"].as_str().map(str::to_string))
        .collect();
    assert!(
        dropped
            .iter()
            .any(|r| r.contains("permissions.schema_version") && r.contains("kept")),
        "{dropped:?}"
    );
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    let s = h.settings();
    assert_eq!(s["permissions"]["schema_version"], 1, "{s}");
    assert_eq!(
        s["permissions"]["profiles"]["mine"]["extends"],
        ":read-only"
    );
    assert!(s["permissions"].get("default_profile").is_none(), "{s}");
    assert!(
        !s19_profiles_unavailable(h),
        "the host must still load the user's named profile after uninstall"
    );
    assert!(h.ledger().is_none());
    assert!(h.inspect().is_err());
}

// ---------------------------------------------------------------------------
// 20. mode detection without a ledger (Gate 1 decision E): a corrupt ledger
//     in a --no-plugin install → doctor derives the mode from the host (the
//     managed store holds omm-* skills, no plugin) and every fix it prints
//     is the managed-store one; `omm reconcile` adopts the store's skills
//     into a fresh ledger; the printed fixes heal to zero Warn without ever
//     installing the bundle on top
// ---------------------------------------------------------------------------

#[test]
fn s20_a_corrupt_ledger_in_managed_store_mode_heals_through_the_printed_fixes() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let skills = catalog_skill_ids(&h.repo);
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    std::fs::write(h.ledger_path(), b"{\"schema_version\":1, broken").expect("corrupt");

    let d = h.doctor(true);
    let d13 = check(&d.json, "D13");
    assert_eq!(d13["severity"], "critical", "{d13}");
    assert_eq!(
        d13["fix"], "omm reconcile\nomm install --no-plugin",
        "the fix must be spelled for the mode the host shows: {d13}"
    );
    let d1 = check(&d.json, "D1");
    f.check(
        d1["severity"] != "critical"
            && d1["observed"]
                .as_str()
                .unwrap_or("")
                .contains("managed-store"),
        || format!("D1 does not read the managed-store mode from the host: {d1}"),
    );
    let d10 = check(&d.json, "D10");
    f.check(d10["severity"] == "info", || format!("D10: {d10}"));

    // Run exactly what D13 printed.
    let mut applied = Vec::new();
    for line in d13["fix"].as_str().expect("fix").lines() {
        let mut argv: Vec<&str> = line.split_whitespace().skip(1).collect();
        if matches!(argv.first().copied(), Some("install") | Some("update")) {
            argv.push("--source");
            argv.push(h.repo_str());
        }
        let out = h.omm(&argv);
        assert_eq!(out.code, 0, "fix `{line}`: {}", out.ctx());
        applied.push(line.to_string());
    }
    eprintln!("fixes applied: {applied:?}");
    assert!(h.inspect().is_err(), "the fixes never installed the bundle");
    assert!(h.installed_plugin_ids().is_empty());
    assert_eq!(
        h.ledgered_skill_ids(),
        skills.iter().cloned().collect::<Vec<_>>(),
        "reconcile adopted every managed-store skill"
    );
    let d = h.doctor(false);
    let rows = non_info_rows(&d.json);
    f.check(d.code == 0 && rows.is_empty(), || {
        format!("doctor after the printed fixes (exit {}): {rows:?}", d.code)
    });

    // The rebuilt ledger knows no prior for the profile's keys nor for the
    // trust entry, and cannot know omm seeded settings.json / trust.json:
    // both are named as current-but-unregistered and left (ARCHITECTURE
    // §4); everything else comes back byte for byte.
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let unregistered = strings(&dry.json["preview"]["unregistered"]);
    assert!(
        unregistered
            .iter()
            .any(|u| u.contains("run.context_slimming.skill_catalog_descriptions"))
            && unregistered.iter().any(|u| u.starts_with("trust entry")),
        "{unregistered:?}"
    );
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.store_skill_ids().is_empty());
    assert!(h.ledger().is_none() && !h.omm_root().exists());
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(!h.config_root().join("themes").exists());
    let residue = strings(&un.json["preview"]["residue"]);
    let after: Snapshot = h
        .snapshot()
        .into_iter()
        .filter(|(k, _)| k != "config/muse/settings.json" && k != "config/muse/trust.json")
        .collect();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 20 (corrupt ledger, managed-store mode)");
}

// ---------------------------------------------------------------------------
// 21. settings.json mode identity (Gate 1 decision F): a private 0600 file
//     stays 0600 through the bundle's host rewrites at install, a second
//     install, an update and the uninstall
// ---------------------------------------------------------------------------

#[test]
fn s21_settings_json_mode_survives_every_host_rewrite_of_the_install_lifetime() {
    use std::os::unix::fs::PermissionsExt;
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let seed = json!({"schema_version": 1, "provider": "meta", "tui": {"theme": "ayu-dark"}});
    h.write_settings(&seed);
    std::fs::set_permissions(h.settings_path(), std::fs::Permissions::from_mode(0o600))
        .expect("mode");
    let seed_bytes = std::fs::read(h.settings_path()).expect("settings");
    let before = h.baseline();
    let mode = |p: &Path| std::fs::metadata(p).expect("meta").permissions().mode() & 0o777;

    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    f.check(mode(&h.settings_path()) == 0o600, || {
        format!(
            "settings.json is {:04o} after `omm install` (was 0600)",
            mode(&h.settings_path())
        )
    });
    // Since 1.3.0-R3057.1 the host rewrite preserves the mode bits, so the
    // file stays 0600 with nothing to re-apply (on 1.0.x the rewrite landed
    // 0644 and this was true).
    assert_eq!(
        r.json["settings_after_host"]["mode_reapplied"],
        false,
        "{}",
        r.ctx()
    );
    let again = h.install();
    assert_eq!(again.code, 0, "{}", again.ctx());
    f.check(mode(&h.settings_path()) == 0o600, || {
        format!(
            "settings.json is {:04o} after a second `omm install`",
            mode(&h.settings_path())
        )
    });
    let up = h.omm(&["update", "--source", h.repo_str()]);
    assert_eq!(up.code, 0, "{}", up.ctx());
    f.check(mode(&h.settings_path()) == 0o600, || {
        format!(
            "settings.json is {:04o} after `omm update`",
            mode(&h.settings_path())
        )
    });
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(
        std::fs::read(h.settings_path()).expect("settings"),
        seed_bytes
    );
    assert_eq!(mode(&h.settings_path()), 0o600);
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 21 (settings.json mode identity)");
}

// ---------------------------------------------------------------------------
// 22. an ancestor symlink INSIDE the base (Gate 1 decision C): a managed
//     skill directory replaced by a symlink to the user's own directory
//     holding byte-identical files → the entries are preserved with the
//     reason (R2 sentinel), the plan continues, no `skills uninstall` for
//     that id, and the user's files survive (round 4: my-commit/SKILL.md
//     was deleted)
// ---------------------------------------------------------------------------

#[test]
fn s22_uninstall_preserves_a_skill_behind_an_in_base_ancestor_symlink_to_identical_user_files() {
    let h = e2e_or_skip!();
    let skills = catalog_skill_ids(&h.repo);
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let store = h.config_root().join("skills");
    let mine = store.join("my-commit");
    copy_tree(&store.join("omm-commit"), &mine);
    std::fs::write(mine.join("mine.md"), b"mine").expect("mine.md");
    std::fs::remove_dir_all(store.join("omm-commit")).expect("rm omm-commit");
    std::os::unix::fs::symlink("my-commit", store.join("omm-commit")).expect("symlink");
    let skill_md = mine.join("SKILL.md");
    assert!(skill_md.is_file());

    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let remove: Vec<String> = dry.json["preview"]["remove"]
        .as_array()
        .expect("remove")
        .iter()
        .filter_map(|p| p["path"].as_str().map(str::to_string))
        .collect();
    assert!(
        !remove.iter().any(|p| p.contains("my-commit")),
        "the user's files are listed for removal: {remove:?}"
    );
    let preserved: Vec<(String, String)> = dry.json["preview"]["preserve"]
        .as_array()
        .expect("preserve")
        .iter()
        .map(|p| {
            (
                p["key"].as_str().unwrap_or("").to_string(),
                p["reason"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    assert!(
        preserved
            .iter()
            .any(|(k, why)| k.contains("skills/omm-commit/SKILL.md") && why.contains("symlink")),
        "{preserved:?}"
    );
    let steps = strings(&dry.json["preview"]["host_steps"]);
    assert!(
        !steps
            .iter()
            .any(|s| s == "muse skills uninstall omm-commit"),
        "{steps:?}"
    );

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(skill_md.is_file(), "the user's SKILL.md survives");
    assert!(mine.join("mine.md").is_file());
    assert!(
        store.join("omm-commit").is_symlink(),
        "the user's symlink survives"
    );
    for id in skills.iter().filter(|id| id.as_str() != "omm-commit") {
        assert!(!store.join(id).exists(), "{id} removed");
    }
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
}

// ---------------------------------------------------------------------------
// 23. the rules file model (Gate 1 round 5, decision H2): omm owns ONLY the
//     managed block — a title the user put above it and a rule below it
//     survive a second install, an update that refreshes the block in place,
//     and the uninstall, byte for byte; the block, its markers and the
//     template's own frame go
// ---------------------------------------------------------------------------

#[test]
fn s23_a_title_above_and_a_rule_below_the_managed_block_survive_install_update_and_uninstall() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let agents_path = h.config_root().join("AGENTS.md");
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(
        r.json["rules"]["note"].as_str().expect("note"),
        "written from the template",
        "{}",
        r.ctx()
    );
    let seeded = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    // The user's edits, both outside the markers: a title above the block
    // (before the template's own title) and a rule after the user-end marker.
    let edited = format!("# MY TITLE\n{seeded}- MY TRAILING RULE\n");
    std::fs::write(&agents_path, &edited).expect("edit");

    // A second install: the block is current, so the file is adopted as it
    // stands — not rewritten to the template.
    let again = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(again.code, 0, "{}", again.ctx());
    assert_eq!(
        again.json["rules"]["action"],
        "unchanged",
        "{}",
        again.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(&agents_path).expect("AGENTS.md"),
        edited,
        "a second install rewrote the user's bytes"
    );

    // Upstream changes the managed block: update refreshes it in place and
    // keeps every other byte.
    let copy = h.sb.root.join("upstream");
    source_copy(&h.repo, &copy);
    let tmpl = copy.join("content").join("rules").join("AGENTS.md.tmpl");
    let t = std::fs::read_to_string(&tmpl).expect("template");
    assert_eq!(t.matches("<!-- omm:managed-end -->").count(), 1);
    std::fs::write(
        &tmpl,
        t.replace(
            "<!-- omm:managed-end -->",
            "- v2 managed rule\n<!-- omm:managed-end -->",
        ),
    )
    .expect("upstream template");
    let copy_str = copy.to_str().expect("utf-8");
    let up = h.omm(&["update", "--source", copy_str]);
    assert_eq!(up.code, 0, "{}", up.ctx());
    assert_eq!(up.json["rules"]["action"], "updated", "{}", up.ctx());
    let refreshed = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    assert!(
        refreshed.starts_with("# MY TITLE\n# Personal rules"),
        "{refreshed}"
    );
    assert!(
        refreshed.contains("- v2 managed rule\n<!-- omm:managed-end -->"),
        "{refreshed}"
    );
    assert!(
        refreshed.ends_with("<!-- omm:user-end -->\n- MY TRAILING RULE\n"),
        "{refreshed}"
    );
    assert_eq!(
        refreshed.replace("- v2 managed rule\n", ""),
        edited,
        "the update touched bytes outside the managed block"
    );

    // Uninstall: the block, its marker lines and the template's frame go;
    // the title and the rule stay, byte for byte, with nothing of omm's
    // between them.
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let rules = dry.json["preview"]["rules"].as_array().expect("rules");
    assert_eq!(rules.len(), 1, "{}", dry.ctx());
    assert!(
        rules[0]["action"]
            .as_str()
            .expect("action")
            .contains("everything else kept byte for byte"),
        "{}",
        dry.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(&agents_path).expect("AGENTS.md"),
        refreshed,
        "the dry run wrote"
    );
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(un.json["report"]["rules_rewritten"], 1, "{}", un.ctx());
    let left = std::fs::read_to_string(&agents_path).expect("AGENTS.md");
    assert_eq!(left, "# MY TITLE\n- MY TRAILING RULE\n");
    assert!(!left.contains("omm:"), "a marker line survived: {left}");
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(h.store_skill_ids().is_empty());
    let residue = strings(&un.json["preview"]["residue"]);
    // R5 modulo the rules file the user made theirs.
    let after: Snapshot = h
        .snapshot()
        .into_iter()
        .filter(|(k, _)| k != "config/muse/AGENTS.md")
        .collect();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 23 (rules file model)");
}

// ---------------------------------------------------------------------------
// 24. an install killed inside `muse skills install` leaves a store directory
//     the host lists with no provenance and refuses to uninstall
//     (`provenance-missing`); the lockfile does not list it, the ledger has no
//     entry (Gate 1 round 5 M1). omm classifies it — a regular directory inside
//     the base, not a link, every file byte-identical to the source, nothing
//     the source does not ship — removes it itself, and both
//     `install --no-plugin` and `uninstall --reconcile-host` converge
// ---------------------------------------------------------------------------

#[test]
fn s24_a_store_dir_the_host_has_no_provenance_for_is_removed_by_omm_and_both_verbs_converge() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let skills = catalog_skill_ids(&h.repo);
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let store = h.config_root().join("skills");
    let orphan_dir = store.join("omm-commit");
    // The kill inside `muse skills install omm-commit`: the directory is
    // there, the host's lockfile does not list it, the ledger (saved after
    // the verb) has no entry for it.
    let make_orphan = |h: &E2e| {
        h.drop_lock_entry("omm-commit");
        h.drop_ledger_entries("skills/omm-commit/");
    };
    make_orphan(&h);
    assert!(h.skills_listed("user").contains("omm-commit"));
    let refused = h
        .invoker()
        .run(&["skills", "uninstall", "omm-commit", "--json"])
        .expect("run skills uninstall");
    assert!(
        !refused.ok() && refused.stdout.contains("provenance-missing"),
        "the host uninstalls a directory it has no provenance for: {} {}",
        refused.stdout,
        refused.stderr
    );
    assert!(orphan_dir.is_dir());
    let d13 = check(&h.doctor(true).json, "D13");
    assert_eq!(d13["severity"], "warn", "{d13}");
    assert!(
        d13["observed"]
            .as_str()
            .expect("observed")
            .contains("skills/omm-commit/SKILL.md"),
        "{d13}"
    );

    // Path 1: `omm install --no-plugin`. A copy the kill cut short (one
    // reference file never landed) is still omm's: removed, installed fresh.
    let cut = orphan_dir.join("references");
    let reference = std::fs::read_dir(&cut)
        .expect("references")
        .flatten()
        .next()
        .expect("a reference file")
        .path();
    std::fs::remove_file(&reference).expect("cut the copy short");
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(
        strings(&r.json["skills"]["installed"]).contains(&"omm-commit".to_string()),
        "{}",
        r.ctx()
    );
    assert!(reference.is_file(), "the fresh install landed every file");
    assert!(h.lock_lists("omm-commit"), "the host has provenance again");
    assert_eq!(
        h.ledgered_skill_ids(),
        skills.iter().cloned().collect::<Vec<_>>()
    );
    let d = h.doctor(true);
    let rows = non_info_rows(&d.json);
    assert!(rows.is_empty(), "doctor after the reinstall: {rows:?}");

    // Path 2: the orphan again; a plain uninstall names it under `!` and
    // refuses; `--reconcile-host` removes it omm-style with the rest.
    make_orphan(&h);
    let plain = h.omm(&["uninstall"]);
    assert_eq!(plain.code, 1, "{}", plain.ctx());
    for needle in ["omm-commit", "no provenance", "--reconcile-host"] {
        assert!(plain.stderr.contains(needle), "{needle}: {}", plain.ctx());
    }
    assert!(orphan_dir.is_dir(), "nothing removed without the flag");
    assert!(h.ledger().is_some());
    let dry = h.omm(&["uninstall", "--reconcile-host", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    assert_eq!(
        dry.json["beyond_ledger_orphans"][0]["id"],
        "omm-commit",
        "{}",
        dry.ctx()
    );
    assert!(orphan_dir.is_dir(), "the dry run removed");
    let un = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(
        un.json["beyond_ledger_orphans_removed"],
        json!(["omm-commit"]),
        "{}",
        un.ctx()
    );
    assert!(
        strings(&un.json["host_reconciled"])
            .iter()
            .any(|s| s.contains("omm removed the store directory of omm-commit")),
        "{}",
        un.ctx()
    );
    assert!(!orphan_dir.exists());
    assert!(h.store_skill_ids().is_empty(), "{:?}", h.store_skill_ids());
    assert!(h.skills_listed("user").is_empty());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 24 (store dir with no host provenance)");
}

// ---------------------------------------------------------------------------
// 25. a managed-store directory that is a symlink is never handed to the
//     host for removal (Gate 1 round 5 decision H1): `muse skills uninstall`
//     would empty whatever the link points at. Beyond the ledger and with no
//     ledger at all, `--reconcile-host` keeps it, names it under ✓ with the
//     R2-sentinel reason, and the link's target is untouched
// ---------------------------------------------------------------------------

#[test]
fn s25_reconcile_host_never_hands_a_linked_store_dir_to_the_host() {
    let h = e2e_or_skip!();
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let store = h.config_root().join("skills");
    // The user moves the directory out and links it back; the ledger lost
    // its entries (an install interrupted before that skill's save).
    let mine = h.sb.home.join("mine-commit");
    std::fs::rename(store.join("omm-commit"), &mine).expect("move out");
    std::os::unix::fs::symlink(&mine, store.join("omm-commit")).expect("link back");
    h.drop_ledger_entries("skills/omm-commit/");
    let target_before = E2e::tree_digest(&mine);
    assert!(target_before.contains_key("SKILL.md"));
    assert!(h.skills_listed("user").contains("omm-commit"));

    let dry = h.omm(&["uninstall", "--reconcile-host", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let kept = dry.json["beyond_ledger_kept"].as_array().expect("kept");
    assert!(
        kept.iter().any(|k| {
            k["id"] == "omm-commit"
                && k["why"]
                    .as_str()
                    .map(|w| w.contains("symlink") && w.contains("R2"))
                    .unwrap_or(false)
        }),
        "{}",
        dry.ctx()
    );
    let steps = strings(&dry.json["preview"]["host_steps"]);
    assert!(
        !steps
            .iter()
            .any(|s| s == "muse skills uninstall omm-commit"),
        "{steps:?}"
    );
    assert!(
        !strings(&dry.json["host_reconciled"])
            .iter()
            .any(|s| s.contains("omm-commit")),
        "{}",
        dry.ctx()
    );
    let un = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(
        E2e::tree_digest(&mine),
        target_before,
        "the link's target was touched"
    );
    assert!(
        store.join("omm-commit").is_symlink(),
        "the user's link survives"
    );
    // `store_skill_ids` follows the link; every other skill directory is gone.
    assert_eq!(
        h.store_skill_ids(),
        vec!["omm-commit".to_string()],
        "the other skills are gone"
    );
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );

    // No ledger now, the host still lists the linked directory: the
    // ledger-less reconciliation keeps it too.
    assert!(h.skills_listed("user").contains("omm-commit"));
    let again = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(again.code, 0, "{}", again.ctx());
    let preserved = again.json["preserved_symlinked_skills"]
        .as_array()
        .expect("preserved_symlinked_skills");
    assert!(
        preserved.iter().any(|p| p["id"] == "omm-commit"),
        "{}",
        again.ctx()
    );
    assert!(!strings(&again.json["host_reconciled"])
        .iter()
        .any(|s| s.contains("omm-commit")));
    assert_eq!(
        E2e::tree_digest(&mine),
        target_before,
        "the link's target was touched"
    );
    assert!(store.join("omm-commit").is_symlink());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
}

// ---------------------------------------------------------------------------
// 26. the ledger entry lands BEFORE the write (Gate 1 round 5 M2): killed the
//     instant AGENTS.md — or the first theme — appears, the ledger already
//     lists it. A rules file carrying omm's block with no entry (a legacy
//     install) is named by doctor D13 and the uninstall preview, adopted by
//     `omm install`, and then removed by uninstall
// ---------------------------------------------------------------------------

#[test]
fn s26_the_ledger_lists_the_rules_file_and_the_themes_before_they_exist_on_disk() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let agents_path = h.config_root().join("AGENTS.md");
    let themes_dir = h.config_root().join("themes");
    let entry_for = |h: &E2e, path: &str| -> Option<Value> {
        h.ledger().and_then(|l| {
            l["entries"]
                .as_array()
                .and_then(|es| es.iter().find(|e| e["path"] == path).cloned())
        })
    };
    let fired = h.omm_killed_when(
        &["install", "--no-plugin", "--source", h.repo_str()],
        || agents_path.exists(),
    );
    eprintln!("killed as AGENTS.md appeared: {fired}");
    assert!(agents_path.is_file());
    let e =
        entry_for(&h, "AGENTS.md").expect("AGENTS.md is on disk, so its entry is in the ledger");
    assert_eq!(e["kind"], "rules", "{e}");
    assert!(!e["prior"]["frame"].is_null(), "{e}");
    let on_disk = std::fs::read(&agents_path).expect("AGENTS.md");
    assert_eq!(
        e["sha256"],
        omm_host::fsx::sha256_bytes(&on_disk),
        "the entry records the bytes about to be written"
    );

    let themes_on_disk = |dir: &Path| -> Vec<String> {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| e.file_name().to_str().map(str::to_string))
                    .filter(|n| n.starts_with("omm-") && n.ends_with(".tmTheme"))
                    .collect()
            })
            .unwrap_or_default()
    };
    let fired = h.omm_killed_when(
        &["install", "--no-plugin", "--source", h.repo_str()],
        || !themes_on_disk(&themes_dir).is_empty(),
    );
    eprintln!("killed as the first theme appeared: {fired}");
    let landed = themes_on_disk(&themes_dir);
    assert!(!landed.is_empty());
    for name in &landed {
        let e = entry_for(&h, &format!("themes/{name}"))
            .unwrap_or_else(|| panic!("{name} is on disk, so its entry is in the ledger"));
        assert_eq!(e["kind"], "theme", "{e}");
    }

    // The rerun completes; the ledger is whole.
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let d = h.doctor(true);
    let rows = non_info_rows(&d.json);
    assert!(rows.is_empty(), "doctor after the rerun: {rows:?}");

    // A rules file carrying omm's block that the ledger does not list (an
    // install of an omm that saved after the write, killed in between):
    // D13 names it, the uninstall preview names it and leaves it, `omm
    // install` adopts it, and the uninstall then removes the untouched seed.
    h.drop_ledger_entries("AGENTS.md");
    let d13 = check(&h.doctor(true).json, "D13");
    assert_eq!(d13["severity"], "warn", "{d13}");
    let observed = d13["observed"].as_str().expect("observed");
    assert!(
        observed.contains("present but unlisted") && observed.contains("muse-config:AGENTS.md"),
        "{d13}"
    );
    assert_eq!(d13["fix"], "omm install --no-plugin", "{d13}");
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    assert_eq!(
        dry.json["unlisted_managed_rules"],
        json!(agents_path.display().to_string()),
        "{}",
        dry.ctx()
    );
    assert_eq!(std::fs::read(&agents_path).expect("AGENTS.md"), on_disk);
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_eq!(
        r.json["rules"]["note"],
        "an identical managed block was already there; adopted",
        "{}",
        r.ctx()
    );
    assert_eq!(std::fs::read(&agents_path).expect("AGENTS.md"), on_disk);
    assert!(entry_for(&h, "AGENTS.md").is_some());
    let d = h.doctor(true);
    let rows = non_info_rows(&d.json);
    assert!(rows.is_empty(), "doctor after the adoption: {rows:?}");
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(
        un.json["unlisted_managed_rules"],
        Value::Null,
        "{}",
        un.ctx()
    );
    assert!(!agents_path.exists(), "the adopted seed is omm's: removed");
    assert!(!themes_dir.exists());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 26 (ledger before the write)");
}

// ---------------------------------------------------------------------------
// 26b. a rerun rewrites ledgered-but-missing themes: scenario 26's kill
//     window (ledger saved, theme unwritten) made deterministic. A rerun
//     that trusted the ledger would NoOp and leave doctor D13 warning;
//     install converges, so the files come back and the uninstall after
//     removes them again.
// ---------------------------------------------------------------------------

#[test]
fn s26b_rerun_rewrites_ledgered_but_missing_themes() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let themes_dir = h.config_root().join("themes");
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let names: Vec<String> = std::fs::read_dir(&themes_dir)
        .expect("themes")
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    assert!(names.len() >= 2, "the source ships themes: {names:?}");
    // What a kill between the ledger save and the theme write leaves.
    for n in names.iter().take(2) {
        std::fs::remove_file(themes_dir.join(n)).expect("remove theme");
    }
    let r = h.omm(&["install", "--no-plugin", "--source", h.repo_str()]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    let written = r.json["themes"]["written"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        written.len(),
        2,
        "the missing themes are rewritten, not trusted from the ledger: {}",
        r.ctx()
    );
    for n in &names {
        assert!(themes_dir.join(n).is_file(), "{n} is back");
    }
    let d = h.doctor(true);
    let rows = non_info_rows(&d.json);
    assert!(rows.is_empty(), "doctor after the rerun: {rows:?}");
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 26b (rerun rewrites ledgered-but-missing themes)");
}

// ---------------------------------------------------------------------------
// 27. a malformed settings.json / trust.json is caught by the plan, not
//     mid-install (Gate 1 round 5 M3): the shape mirror and the host's own
//     loader (`skills list --json`) refuse up front with the reason and a
//     repair hint, nothing written. An uninstall over a trust.json that went
//     bad under the install degrades to a blind undo and completes instead of
//     stopping; the repaired rerun finishes clean
// ---------------------------------------------------------------------------

#[test]
fn s27_malformed_host_config_is_refused_by_the_plan_and_an_uninstall_over_it_completes_blind() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    // The seed bed is the test's own doing, not omm's: create it before the
    // baseline, so the "a refused install touched the roots" assert below
    // measures omm alone. (On platforms where the host leaves config/muse
    // absent, the dir stood out as a diff.)
    std::fs::create_dir_all(h.config_root()).expect("config root");
    let before = h.baseline();
    let trust_path = h.config_root().join("trust.json");
    let nothing_written = |h: &E2e, r: &Run, needles: &[&str]| {
        assert_eq!(r.code, 2, "{}", r.ctx());
        for n in needles {
            assert!(r.stderr.contains(n), "{n}: {}", r.ctx());
        }
        assert!(
            !h.omm_root().exists(),
            "a refused install wrote: {}",
            r.ctx()
        );
        assert!(h.inspect().is_err());
        assert!(h.marketplace_names().is_empty());
        assert!(!h.config_root().join("AGENTS.md").exists());
    };
    // A repeated key: the shape mirror (`SettingsDoc::load`) sees it.
    std::fs::write(
        h.settings_path(),
        br#"{"schema_version":1,"tui":{"theme":"ayu-dark"},"tui":{}}"#,
    )
    .expect("seed");
    let r = h.install();
    nothing_written(
        &h,
        &r,
        &["settings.json", "duplicate", "Nothing was written"],
    );
    // A typed error only the host's loader sees: the plan's `skills list
    // --json` probe carries the host's reason.
    std::fs::write(h.settings_path(), br#"{"schema_version":1,"tui":5}"#).expect("seed");
    let r = h.install();
    nothing_written(&h, &r, &["malformed settings file", "Nothing was written"]);
    std::fs::remove_file(h.settings_path()).expect("clean");
    // A trust store with a decision the host does not know.
    std::fs::write(
        &trust_path,
        br#"{"schema_version":1,"projects":{"/x":{"decision":"typo"}}}"#,
    )
    .expect("seed");
    let r = h.install();
    nothing_written(&h, &r, &["trust.json", "typo", "Nothing was written"]);
    std::fs::remove_file(&trust_path).expect("clean");
    assert_eq!(h.snapshot(), before, "a refused install touched the roots");

    // A good install; then the user's hand edit of trust.json goes wrong
    // under it (a decision the host does not know). The host's listing verb
    // exits 1, so the uninstall goes blind: it says so, undoes every
    // registration anyway (the host's `plugins remove` / `marketplace
    // remove` still work over such a file), removes every ledgered file,
    // and completes — the edited trust entry is the user's now, preserved
    // and named, never an error.
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_bundle_installed(&h, &r.json);
    let good = std::fs::read(&trust_path).expect("trust.json");
    let typo = String::from_utf8_lossy(&good).replace("\"trusted\"", "\"typo\"");
    assert_ne!(typo.as_bytes(), good.as_slice());
    std::fs::write(&trust_path, &typo).expect("edit trust.json");
    let un = h.omm(&["uninstall"]);
    assert!(
        un.stderr.contains("undone blind") && un.stderr.contains("malformed trust store"),
        "the uninstall did not say it went blind with the host's reason: {}",
        un.ctx()
    );
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(
        un.json["preview"]["preserve"]
            .as_array()
            .expect("preserve")
            .iter()
            .any(|p| {
                p["key"] == "muse-config:trust.json"
                    && p["reason"]
                        .as_str()
                        .map(|r| r.contains("edited since omm set it"))
                        .unwrap_or(false)
            }),
        "{}",
        un.ctx()
    );
    // Everything but the user's trust.json is gone (`assert_bundle_uninstalled`
    // would want that file gone too).
    assert!(h.inspect().is_err());
    assert!(h.installed_plugin_ids().is_empty());
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(!h.config_root().join("themes").exists());
    assert!(
        !h.settings_path().exists(),
        "omm created settings.json; it goes"
    );
    assert_eq!(
        std::fs::read_to_string(&trust_path).expect("trust.json"),
        typo,
        "the user's edited trust.json is theirs: left as it stands"
    );
    std::fs::remove_file(&trust_path).expect("the user's file, cleared for the next half");

    // The same, with trust.json cut short (not JSON): the blind undo still
    // completes — every host step and every ledgered file — but the trust
    // restore cannot load the file, so the shared files wait, their entries
    // kept for the rerun (rc 1, not a stop). The repaired rerun finishes
    // clean.
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_bundle_installed(&h, &r.json);
    let good = std::fs::read(&trust_path).expect("trust.json");
    let cut = &good[..good.len() / 2];
    assert!(serde_json::from_slice::<Value>(cut).is_err());
    std::fs::write(&trust_path, cut).expect("cut trust.json");
    let un = h.omm(&["uninstall"]);
    eprintln!(
        "blind uninstall over a cut trust.json: rc {} errors {}",
        un.code, un.json["report"]["errors"]
    );
    assert!(un.stderr.contains("undone blind"), "{}", un.ctx());
    assert_eq!(un.code, 1, "{}", un.ctx());
    let errors = strings(&un.json["report"]["errors"]);
    assert!(!errors.is_empty(), "{}", un.ctx());
    assert!(
        errors.iter().all(|e| e.contains("trust")),
        "an error beyond the trust file: {errors:?}"
    );
    assert!(h.inspect().is_err(), "the plugin is still installed");
    assert!(h.installed_plugin_ids().is_empty());
    assert!(!h.marketplace_names().contains(&"omm".to_string()));
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(!h.config_root().join("themes").exists());
    assert!(
        h.ledger().is_some(),
        "the ledger keeps the deferred entries for the rerun"
    );
    assert_eq!(
        std::fs::read(&trust_path).expect("trust.json"),
        cut,
        "the malformed file was rewritten"
    );
    std::fs::write(&trust_path, &good).expect("repair");
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(!trust_path.exists(), "omm created trust.json; it goes");
    assert!(
        !h.settings_path().exists(),
        "omm created settings.json; it goes"
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 27 (malformed host config)");
}

// ---------------------------------------------------------------------------
// Phase 2 / 3 harness: the mock provider, a static file server, the session
// records beyond the context blocks, workspace snapshots
// ---------------------------------------------------------------------------

/// `python3` (or `python`) on PATH — the mock provider and the static server
/// are Python (`tools/mockprovider/`).
fn python3() -> Option<PathBuf> {
    ["python3", "python"].iter().find_map(|p| {
        Command::new(p)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
            .then(|| PathBuf::from(p))
    })
}

macro_rules! python_or_skip {
    () => {
        match python3() {
            Some(p) => p,
            None => {
                eprintln!("skipped: no python3 on PATH");
                return;
            }
        }
    };
}

/// Wait (bounded at 10 s) until `127.0.0.1:port` accepts a connection.
fn wait_for_port(port: u16) -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(10) {
        if std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// A child process killed when the scenario ends.
struct Background(std::process::Child);

impl Drop for Background {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// `tools/mockprovider/mock.py` + `respond-toolcall.py` on `127.0.0.1:<port>`
/// playing a scripted model (`tools/mockprovider/README.md`): the plan is a
/// list of `{"call": …}` / `{"text": …}` steps, step *n* answering the
/// request that carries *n* `function_call_output` items.
struct MockProvider {
    _child: Background,
    port: u16,
    log: PathBuf,
}

impl MockProvider {
    fn start(python: &Path, repo: &Path, work: &Path, port: u16, plan: &Value) -> MockProvider {
        let state = work.join("state");
        let logs = work.join("logs");
        std::fs::create_dir_all(&state).expect("mock state");
        std::fs::create_dir_all(&logs).expect("mock logs");
        std::fs::write(
            state.join("plan.json"),
            serde_json::to_vec_pretty(plan).expect("plan"),
        )
        .expect("plan.json");
        let log = logs.join("mock.log");
        let stderr = std::fs::File::create(logs.join("mock.stderr")).expect("mock stderr");
        let tools = repo.join("tools").join("mockprovider");
        let child = Command::new(python)
            .arg(tools.join("mock.py"))
            .env("MOCK_PORT", port.to_string())
            .env("MOCK_STATE", &state)
            .env("MOCK_LOG", &log)
            .env("MOCK_RESPONDER", tools.join("respond-toolcall.py"))
            .env("MOCK_MODELS", tools.join("models.json"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("spawn the mock provider");
        let mock = MockProvider {
            _child: Background(child),
            port,
            log,
        };
        assert!(
            wait_for_port(port),
            "the mock provider did not listen on 127.0.0.1:{port}: {}",
            std::fs::read_to_string(logs.join("mock.stderr")).unwrap_or_default()
        );
        mock
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Every `POST /responses` body the provider received, in order.
    fn posts(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok())
            .filter(|r| r["m"] == "POST")
            .map(|r| {
                r["body"]
                    .as_str()
                    .and_then(|b| serde_json::from_str::<Value>(b).ok())
                    .unwrap_or(Value::Null)
            })
            .collect()
    }
}

/// `python3 -m http.server` serving `dir` on `127.0.0.1:<port>`.
struct StaticServer {
    _child: Background,
}

impl StaticServer {
    fn start(python: &Path, dir: &Path, port: u16) -> StaticServer {
        let mut child = Command::new(python)
            .args([
                "-m",
                "http.server",
                &port.to_string(),
                "--bind",
                "127.0.0.1",
            ])
            .arg("--directory")
            .arg(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn http.server");
        if !wait_for_port(port) {
            // An early exit reports its status; a stuck server is killed
            // and reaped so its stderr names the real cause (a bind
            // failure, a traceback) instead of a bare timeout.
            let early = child.try_wait().ok().flatten();
            let _ = child.kill();
            let err = child
                .wait_with_output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stderr)
                        .chars()
                        .take(500)
                        .collect::<String>()
                })
                .unwrap_or_default();
            match early {
                Some(status) => panic!(
                    "http.server ({}) exited early with {status} instead of listening on 127.0.0.1:{port}; stderr: {err}",
                    python.display(),
                ),
                None => panic!(
                    "http.server ({}) did not listen on 127.0.0.1:{port} within 10 s; stderr: {err}",
                    python.display(),
                ),
            }
        }
        StaticServer {
            _child: Background(child),
        }
    }
}

/// What one `session.jsonl` records beyond the context blocks: every hook
/// terminal `(status, error)`, every committed tool-call name, every tool
/// result text (retained-frame children included, like `probe`).
#[derive(Default, Debug)]
struct SessionRecords {
    terminals: Vec<(String, Option<String>)>,
    tool_calls: Vec<String>,
    tool_results: Vec<String>,
}

fn session_records(log: &Path) -> SessionRecords {
    let mut out = SessionRecords::default();
    for line in std::fs::read_to_string(log).expect("session log").lines() {
        if let Ok(v) = serde_json::from_str::<Value>(line) {
            walk_records(&v, &mut out);
        }
    }
    out
}

fn walk_records(v: &Value, out: &mut SessionRecords) {
    match v {
        Value::Object(m) => {
            match m.get("kind").and_then(Value::as_str) {
                Some("hook_run_terminal") => out.terminals.push((
                    m.get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string(),
                    m.get("error").and_then(Value::as_str).map(str::to_string),
                )),
                Some("assistant_tool_calls_committed") => {
                    for c in m
                        .get("tool_calls")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        if let Some(n) = c.get("name").and_then(Value::as_str) {
                            out.tool_calls.push(n.to_string());
                        }
                    }
                }
                Some("tool_result_batch_committed") => {
                    for r in m
                        .get("results")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        out.tool_results.push(
                            r.get("text")
                                .and_then(Value::as_str)
                                .map(str::to_string)
                                .unwrap_or_else(|| r.to_string()),
                        );
                    }
                }
                _ => {}
            }
            for c in m
                .get("children")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                if let Some(Ok(rec)) = c
                    .get("record_json")
                    .and_then(Value::as_str)
                    .map(serde_json::from_str::<Value>)
                {
                    walk_records(&rec, out);
                }
            }
            for x in m.values() {
                walk_records(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| walk_records(x, out)),
        _ => {}
    }
}

/// The lines that differ between two snapshots, for a readable failure.
fn snapshot_diff(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let keys: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    keys.into_iter()
        .filter(|k| before.get(*k) != after.get(*k))
        .map(|k| {
            format!(
                "{k}: before={} after={}",
                before.get(k).map(String::as_str).unwrap_or("absent"),
                after.get(k).map(String::as_str).unwrap_or("absent")
            )
        })
        .collect()
}

/// `{relative path → sha256}` of every regular file under `dir`, sorted —
/// what a home directory holds after an installer ran.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = E2e::tree_digest(dir).into_keys().collect();
    out.sort();
    out
}

#[cfg(unix)]
fn mode_of(p: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p).expect("meta").permissions().mode() & 0o777
}

impl E2e {
    /// The workspace's files, `.git` excluded.
    fn ws_snapshot(&self) -> Snapshot {
        let mut out = Snapshot::new();
        walk(&self.ws, &self.ws, "ws", &mut out);
        out.retain(|k, _| k != "ws/.git" && !k.starts_with("ws/.git/"));
        out
    }

    /// [`E2e::snapshot`] minus `data/muse/model-catalog/**`: a real provider
    /// session (`--provider meta`, the mock lane) starts with `GET
    /// <base>/muse-code/models` and the host caches that catalog under its
    /// data root — a `<provider>__<host>.json` per endpoint — which an echo
    /// session never does, so it is outside the fixed host-owned residue the
    /// uninstall preview names (reported: a candidate for that list). Not
    /// omm's footprint; R5 is measured around it in scenarios 28 and 30.
    fn snapshot_sans_model_catalog(&self) -> Snapshot {
        let after = self.snapshot();
        let cached: Vec<&String> = after
            .keys()
            .filter(|k| k.starts_with("data/muse/model-catalog"))
            .collect();
        if !cached.is_empty() {
            eprintln!(
                "host residue of the provider session, outside the uninstall preview's list: {cached:?}"
            );
        }
        after
            .into_iter()
            .filter(|(k, _)| !k.starts_with("data/muse/model-catalog"))
            .collect()
    }

    /// The newest `session.jsonl` under the sandbox's data root.
    fn newest_session_log(&self) -> PathBuf {
        probe::newest_session_log(&self.data_root().join("sessions")).expect("session log")
    }

    /// One `muse exec` through the mock provider, the way the harnesses do
    /// it (`tools/mockprovider/run-omm-mcp.sh` step 5): `--provider meta
    /// --base-url`, `--yolo`, a dummy key, the file credential backend, the
    /// omm binary's directory first on the PATH the host inherits (it spawns
    /// `omm mcp` and `omm hook …` through it), `extra_env` on top. Returns
    /// the outcome and the session log it wrote.
    fn exec_via_mock(
        &self,
        mock: &MockProvider,
        prompt: &str,
        max_steps: u32,
        extra_env: &[(&str, &str)],
    ) -> (omm_host::Outcome, PathBuf) {
        let mut inv = self
            .invoker()
            .env("PATH", path_with_bin_dir(&self.omm))
            .env("META_API_KEY", "dummy")
            .env("TBH_CREDENTIAL_BACKEND", "file")
            .timeout(Duration::from_secs(180));
        for (k, v) in extra_env {
            inv = inv.env(k, v);
        }
        let base = mock.base_url();
        let steps = max_steps.to_string();
        let argv: Vec<&str> = vec![
            "exec",
            "--provider",
            "meta",
            "--base-url",
            &base,
            "--model",
            "test-model",
            "--yolo",
            "--max-model-steps",
            &steps,
            prompt,
        ];
        let out = self.invoker_run(&inv, &argv);
        let log = self.newest_session_log();
        (out, log)
    }

    fn invoker_run(&self, inv: &Invoker, argv: &[&str]) -> omm_host::Outcome {
        inv.run(argv).expect("muse exec through the mock provider")
    }
}

/// A committed copy of this checkout — every tracked and untracked-unignored
/// file — as its own git repository, so a script that inspects `git status`
/// sees a clean tree that is not this one.
fn git_checkout_copy(src: &Path, dest: &Path) {
    let listed = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(src)
        .stdin(Stdio::null())
        .output()
        .expect("git ls-files");
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    for rel in listed.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let rel = Path::new(std::str::from_utf8(rel).expect("utf-8 path"));
        let from = src.join(rel);
        if !from.is_file() {
            continue;
        }
        let to = dest.join(rel);
        std::fs::create_dir_all(to.parent().expect("parent")).expect("mkdir");
        std::fs::copy(&from, &to).unwrap_or_else(|e| panic!("copy {}: {e}", from.display()));
    }
    let hooks = dest.join(".no-hooks");
    std::fs::create_dir_all(&hooks).expect("hooks dir");
    for argv in [
        vec!["init", "-q"],
        vec!["add", "-A"],
        vec!["commit", "-q", "-m", "e2e snapshot of the checkout"],
    ] {
        let out = Command::new("git")
            .args(["-c", "user.name=omm-e2e"])
            .args(["-c", "user.email=omm-e2e@example.invalid"])
            .args(["-c", "commit.gpgsign=false"])
            .arg("-c")
            .arg(format!("core.hooksPath={}", hooks.display()))
            .args(&argv)
            .current_dir(dest)
            .stdin(Stdio::null())
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {argv:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    std::fs::remove_dir(&hooks).expect("hooks dir");
}

fn git_porcelain(repo: &Path) -> String {
    let out = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(repo)
        .stdin(Stdio::null())
        .output()
        .expect("git status");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The `version = "…"` under `[workspace.package]` of a Cargo.toml.
fn workspace_version(cargo_toml: &Path) -> String {
    let text = std::fs::read_to_string(cargo_toml).expect("Cargo.toml");
    let mut in_table = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_table = t == "[workspace.package]";
            continue;
        }
        if in_table && t.starts_with("version") {
            if let Some(v) = t.split('"').nth(1) {
                return v.to_string();
            }
        }
    }
    panic!("no [workspace.package] version in {}", cargo_toml.display());
}

// ---------------------------------------------------------------------------
// 28. the MCP server (PLAN.md 2.2): bundle install → `plugins inspect` holds
//     plugin:oh-my-musecode:mcp_server:doc trusted_enabled with command [omm, mcp] →
//     D15 green with omm on PATH, critical with the exact fix without it →
//     the host spawns `omm mcp` through PATH and a scripted model's
//     `tools/call omm_doctor {fast:true}` comes back as the doctor JSON in
//     the next provider request (the mock provider lane of
//     docs/experiments/mcp-tools-call.md, three instruments cross-checked)
//     → uninstall → R5. The sandbox puts the XDG roots at HOME's defaults:
//     the server sees the host's scrubbed environment (HOME and PATH, no
//     XDG_*, no OMM_MUSE_BIN), resolves the roots the installer used through
//     HOME and the Muse binary through ~/.local/bin/.muse-version
// ---------------------------------------------------------------------------

#[test]
fn s28_mcp_server_round_trips_omm_doctor_through_the_mock_provider_and_d15_names_a_missing_path() {
    let h = e2e_or_skip!(home);
    let python = python_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let r = h.install();
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert_bundle_installed(&h, &r.json);
    let server_id = "plugin:oh-my-musecode:mcp_server:doc";
    assert!(
        strings(&r.json["bundle"]["trusted"]).contains(&server_id.to_string()),
        "{}",
        r.ctx()
    );
    let ins = h.inspect().expect("plugins inspect --json");
    assert!(
        ins.is_trusted_enabled(server_id),
        "{server_id}: {:?}",
        ins.capability(server_id)
    );
    let servers = ins.raw["plugin"]["capabilities"]["mcp_servers"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        servers
            .iter()
            .any(|s| s["id"] == "doc" && s["command"] == json!(["omm", "mcp"])),
        "the host holds the doc server with command [omm, mcp]: {servers:?}"
    );

    // D15 green: the doctor's PATH (omm_cmd) has this binary's directory first.
    let d = h.doctor(true);
    let d15 = check(&d.json, "D15");
    f.check(
        d15["severity"] == "info"
            && d15["observed"]
                .as_str()
                .unwrap_or("")
                .contains(&format!("`omm` → {}", h.omm.display())),
        || format!("D15 with omm on PATH: {d15}"),
    );
    let rows = non_info_rows(&d.json);
    f.check(d.code == 0 && rows.is_empty(), || {
        format!(
            "doctor --fast after the install (exit {}): {rows:?}",
            d.code
        )
    });
    // D15 critical: a PATH without omm; the fix is the export line for this
    // binary's directory, and the human report prints it.
    let bare_path = "/usr/bin:/bin:/usr/sbin:/sbin";
    let d = h.omm_env(&["doctor", "--fast"], true, &[("PATH", bare_path)]);
    assert_eq!(d.code, 1, "{}", d.ctx());
    let d15 = check(&d.json, "D15");
    assert_eq!(d15["severity"], "critical", "{d15}");
    assert!(
        d15["observed"]
            .as_str()
            .unwrap_or("")
            .contains("`omm` is not on PATH"),
        "{d15}"
    );
    let omm_dir = h.omm.parent().expect("omm dir").display().to_string();
    let fix = d15["fix"].as_str().unwrap_or("").to_string();
    assert!(
        fix.starts_with(&format!("export PATH=\"{omm_dir}:$PATH\"")),
        "{d15}"
    );
    let mut human = h.omm_cmd(&["doctor", "--fast"], &h.ws);
    human.env("PATH", bare_path);
    let human = E2e::run_cmd(human);
    assert_eq!(human.code, 1, "{}", human.ctx());
    assert!(human.stdout.contains("CRIT D15"), "{}", human.stdout);
    assert!(
        human
            .stdout
            .contains(&format!("fix: {}", fix.lines().next().unwrap_or(""))),
        "{}",
        human.stdout
    );

    // The launcher rung the scrubbed server finds the host through
    // (`omm_host::locate`: ~/.local/bin/.muse-version → muse-bin-<version>).
    let version = h
        .muse
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix("muse-bin-"))
        .unwrap_or("local")
        .to_string();
    let launcher_dir = h.sb.home.join(".local").join("bin");
    std::fs::create_dir_all(&launcher_dir).expect("launcher dir");
    std::os::unix::fs::symlink(&h.muse, launcher_dir.join(format!("muse-bin-{version}")))
        .expect("muse-bin link");
    std::fs::write(launcher_dir.join(".muse-version"), format!("{version}\n"))
        .expect(".muse-version");

    // The scripted model: one `omm_doctor {fast:true}` call, then an answer.
    let plan = json!([
        {"call": {"name": "mcp__plugin_oh_my_musecode_doc__omm_doctor", "arguments": {"fast": true}}},
        {"text": "MOCK-FINAL-ANSWER omm_doctor done"}
    ]);
    let mock = MockProvider::start(&python, &h.repo, &h.sb.root.join("mock"), 8751, &plan);
    let (out, log) = h.exec_via_mock(
        &mock,
        "Run the omm doctor tool and report what it says.",
        8,
        &[],
    );
    assert!(
        out.ok(),
        "muse exec through the mock provider: rc {:?}\n--- stdout\n{}\n--- stderr\n{}",
        out.code,
        out.stdout,
        out.stderr
    );
    assert!(out.stdout.contains("MOCK-FINAL-ANSWER"), "{}", out.stdout);

    // Instrument 1, the provider transcript: request #1 advertises the
    // namespace group with both tools; request #2 carries the call the
    // model emitted and the doctor JSON as its function_call_output.
    let posts = mock.posts();
    assert!(
        posts.len() >= 2,
        "{} provider request(s): {posts:?}",
        posts.len()
    );
    let namespaces: Vec<&Value> = posts[0]["tools"]
        .as_array()
        .expect("tools[]")
        .iter()
        .filter(|t| t["name"] == "mcp__plugin_oh_my_musecode_doc")
        .collect();
    assert_eq!(
        namespaces.len(),
        1,
        "the namespace on the wire: {}",
        posts[0]["tools"]
    );
    let tools: BTreeSet<&str> = namespaces[0]["tools"]
        .as_array()
        .expect("namespace tools")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(
        tools,
        BTreeSet::from(["omm_cost", "omm_doctor"]),
        "both tools in the namespace group (the host orders them by name)"
    );
    // On 1.3.0 the host interleaves reminder-subagent requests (tools with
    // no mcp namespace) between the call and its output, so find the call
    // and its output by shape instead of by request number.
    let inputs: Vec<&Vec<Value>> = posts[1..]
        .iter()
        .map(|p| p["input"].as_array().expect("input[]"))
        .collect();
    let call = inputs
        .iter()
        .flat_map(|items| items.iter())
        .find(|i| {
            i["type"] == "function_call"
                && i["name"]
                    .as_str()
                    .unwrap_or("")
                    .starts_with("mcp__plugin_oh_my_musecode_doc")
        })
        .expect("the call echoed back after request #1");
    assert_eq!(call["arguments"], "{\"fast\": true}", "{call}");
    let call_id = call["call_id"].clone();
    let output = inputs
        .iter()
        .flat_map(|items| items.iter())
        .find(|i| i["type"] == "function_call_output" && i["call_id"] == call_id)
        .expect("the function_call_output for the call");
    assert_eq!(output["call_id"], call["call_id"]);
    let doc: Value = serde_json::from_str(output["output"].as_str().expect("output string"))
        .expect("the tool result is the doctor JSON");
    assert_eq!(doc["host"]["plugin_id"], "oh-my-musecode", "{doc}");
    let ids: Vec<&str> = doc["checks"]
        .as_array()
        .expect("checks[]")
        .iter()
        .filter_map(|c| c["id"].as_str())
        .collect();
    assert!(
        ids.first() == Some(&"D1") && ids.contains(&"D15"),
        "{ids:?}"
    );
    assert_eq!(
        doc["host"]["config_root"],
        json!(h.config_root()),
        "the server resolved the installer's roots through HOME: {}",
        doc["host"]
    );
    f.check(doc["exit_code"] == 0 && !has_critical(&doc), || {
        format!(
            "the doctor through the pipe is not green: exit {} rows {:?}",
            doc["exit_code"],
            non_info_rows(&doc)
        )
    });
    // Instrument 2, the server's own trace under MUSE_PLUGIN_DATA_DIR: the
    // spawn in the workspace and the tools/call with the scripted arguments.
    let trace = h
        .data_root()
        .join("plugins")
        .join("data")
        .join("oh-my-musecode")
        .join("omm-mcp.log");
    let text = std::fs::read_to_string(&trace).unwrap_or_else(|e| {
        panic!(
            "{}: {e} — the host never spawned the server",
            trace.display()
        )
    });
    assert!(text.contains("\tEV=spawn\t"), "{text}");
    assert!(
        text.contains("\"name\":\"omm_doctor\",\"arguments\":{\"fast\":true}"),
        "{text}"
    );
    assert!(text.contains("\tEV=tool\t"), "{text}");
    // Instrument 3, session.jsonl: the canonical id on the call, the JSON
    // on the result.
    let rec = session_records(&log);
    assert!(
        rec.tool_calls
            .iter()
            .any(|n| n == "mcp__plugin_oh_my_musecode_doc__omm_doctor"),
        "{:?}",
        rec.tool_calls
    );
    assert!(
        rec.tool_results
            .iter()
            .any(|t| t.contains("\"checks\"") && t.contains("\"D15\"")),
        "{:?}",
        rec.tool_results
            .iter()
            .map(|t| t.chars().take(200).collect::<String>())
            .collect::<Vec<_>>()
    );

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_bundle_uninstalled(&h, &un.json);
    assert!(
        !trace.exists(),
        "`plugins remove oh-my-musecode --delete-data` takes the server's data dir with it"
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot_sans_model_catalog();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 28 (MCP server through the mock provider)");
}

// ---------------------------------------------------------------------------
// 29. memory (PLAN.md 2.3): trust → seed writes MEMORY.md and the sidecar
//     into the personal_project root, both ledgered under muse-data → a real
//     echo session renders the seed at order u32::MAX → list maps the root
//     back through the sidecar → backup tars it under $OMM/snapshots/memory
//     → gc (dry run and real) removes nothing while the workspace exists →
//     uninstall removes the untouched seed and prunes the root → R5
// ---------------------------------------------------------------------------

#[test]
fn s29_memory_seed_composes_the_block_backup_and_gc_keep_it_and_uninstall_removes_the_seed() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let ws = std::fs::canonicalize(&h.ws).expect("ws");
    let ws_str = ws.display().to_string();
    let t = h.omm(&["trust", "."]);
    assert_eq!(t.code, 0, "{}", t.ctx());

    let p = h.omm(&["memory", "path"]);
    assert_eq!(p.code, 0, "{}", p.ctx());
    assert_eq!(p.json["workspace"], json!(ws_str), "{}", p.ctx());
    assert_eq!(p.json["exists"], false, "{}", p.ctx());
    assert_eq!(p.json["trust"], "trusted", "{}", p.ctx());
    let root = PathBuf::from(p.json["path"].as_str().expect("path"));
    let name = omm_host::paths::personal_project_dir_name(&ws);
    assert_eq!(
        root,
        h.data_root().join("memory").join("projects").join(&name),
        "the formula of host-reality.md \"Paths\""
    );
    let memory_md = root.join("MEMORY.md");
    let sidecar = root.join("omm-workspace.json");

    let s = h.omm(&["memory", "seed"]);
    assert_eq!(s.code, 0, "{}", s.ctx());
    assert_eq!(s.json["root_created"], true, "{}", s.ctx());
    assert_eq!(s.json["memory_md"]["outcome"], "written", "{}", s.ctx());
    assert_eq!(
        s.json["omm_workspace_json"]["outcome"],
        "written",
        "{}",
        s.ctx()
    );
    assert_eq!(s.json["trust"], "trusted", "{}", s.ctx());
    let seed = std::fs::read_to_string(&memory_md).expect("MEMORY.md");
    assert!(seed.starts_with("# Project memory\n"), "{seed}");
    let side: Value =
        serde_json::from_slice(&std::fs::read(&sidecar).expect("sidecar")).expect("sidecar JSON");
    assert_eq!(side["schema_version"], 1, "{side}");
    assert_eq!(side["workspace"], json!(ws_str), "{side}");
    let ledger = h.ledger().expect("ledger");
    let entries = ledger["entries"].as_array().expect("entries");
    for (file, class) in [("MEMORY.md", "seeded"), ("omm-workspace.json", "exclusive")] {
        let path = format!("memory/projects/{name}/{file}");
        assert!(
            entries
                .iter()
                .any(|e| e["base"] == "muse-data" && e["path"] == path && e["class"] == class),
            "{path} {class}: {entries:?}"
        );
    }
    // A second seed is a no-op on both files.
    let again = h.omm(&["memory", "seed"]);
    assert_eq!(again.code, 0, "{}", again.ctx());
    assert_eq!(again.json["root_created"], false, "{}", again.ctx());
    assert_eq!(
        again.json["memory_md"]["outcome"],
        "unchanged",
        "{}",
        again.ctx()
    );
    assert_eq!(
        again.json["omm_workspace_json"]["outcome"],
        "unchanged",
        "{}",
        again.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(&memory_md).expect("MEMORY.md"),
        seed
    );

    // A real echo session in the trusted workspace: the memory_snapshot
    // block (order u32::MAX) renders the seed and never the sidecar.
    let echo = h
        .invoker()
        .run(&["exec", "--provider", "echo", "hi"])
        .expect("exec echo");
    assert!(echo.ok(), "echo session: {:?} {}", echo.code, echo.stderr);
    let facts = probe::parse_session_log(&h.newest_session_log()).expect("parse session log");
    let block = facts
        .block(u64::from(hr::CONTEXT_ORDER_MEMORY_SNAPSHOT))
        .unwrap_or_else(|| panic!("no memory_snapshot block; orders {:?}", facts.orders()));
    assert!(
        block.text.contains("## Memory scope: personal_project"),
        "{}",
        block.text
    );
    assert!(block.text.contains(seed.trim_end()), "{}", block.text);
    assert!(
        !block.text.contains("omm-workspace.json"),
        "the sidecar is not Markdown: {}",
        block.text
    );
    assert!(block.bytes as u64 <= hr::MEMORY_SNAPSHOT_BYTES);

    // list: the root maps back to the workspace through the sidecar.
    let l = h.omm(&["memory", "list"]);
    assert_eq!(l.code, 0, "{}", l.ctx());
    assert!(
        l.stdout.contains(&name) && l.stdout.contains(&ws_str) && l.stdout.contains("sidecar"),
        "{}",
        l.ctx()
    );
    let human = h.omm_raw(&["memory", "list"]);
    assert_eq!(human.code, 0, "{}", human.ctx());
    assert!(human.stdout.contains(&name), "{}", human.stdout);

    // backup: a tar of the root under $OMM/snapshots/memory/<ts>/.
    let b = h.omm(&["memory", "backup"]);
    assert_eq!(b.code, 0, "{}", b.ctx());
    let tar = PathBuf::from(b.json["backup"].as_str().expect("backup"));
    assert!(
        tar.starts_with(h.omm_root().join("snapshots").join("memory")),
        "{}",
        tar.display()
    );
    assert!(tar.is_file(), "{}", tar.display());
    assert_eq!(b.json["files"], 2, "{}", b.ctx());
    let listing = Command::new("tar")
        .arg("-tf")
        .arg(&tar)
        .stdin(Stdio::null())
        .output()
        .expect("tar -tf");
    let members = String::from_utf8_lossy(&listing.stdout);
    assert!(
        listing.status.success()
            && members.contains("MEMORY.md")
            && members.contains("omm-workspace.json"),
        "tar -tf: {members} {}",
        String::from_utf8_lossy(&listing.stderr)
    );

    // gc: the workspace exists, so the root is kept — dry run and for real.
    let dry = h.omm(&["memory", "gc", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    assert_eq!(dry.json["dry_run"], true);
    assert_eq!(dry.json["removed"], json!([]), "{}", dry.ctx());
    let kept = dry.json["kept"].as_array().expect("kept");
    assert_eq!(kept.len(), 1, "{}", dry.ctx());
    assert_eq!(kept[0]["root"], name, "{}", dry.ctx());
    assert!(
        kept[0]["why"].as_str().unwrap_or("").contains("exists"),
        "{}",
        dry.ctx()
    );
    let gc = h.omm(&["memory", "gc"]);
    assert_eq!(gc.code, 0, "{}", gc.ctx());
    assert_eq!(gc.json["removed"], json!([]), "{}", gc.ctx());
    assert!(
        memory_md.is_file() && sidecar.is_file(),
        "gc removed the live root"
    );
    assert_eq!(
        std::fs::read_to_string(&memory_md).expect("MEMORY.md"),
        seed
    );

    // Uninstall: the preview lists both files under remove; the untouched
    // seed and the sidecar go, the root is pruned, the tar goes with $OMM.
    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    let remove: Vec<String> = dry.json["preview"]["remove"]
        .as_array()
        .expect("remove")
        .iter()
        .filter_map(|p| p["path"].as_str().map(str::to_string))
        .collect();
    for file in [&memory_md, &sidecar] {
        let abs = std::fs::canonicalize(file)
            .expect("file")
            .display()
            .to_string();
        assert!(remove.contains(&abs), "{abs} not in {remove:?}");
    }
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(un.json["report"]["preserved"], 0, "{}", un.ctx());
    assert!(!memory_md.exists(), "the untouched seed is removed");
    assert!(!sidecar.exists(), "the sidecar is removed");
    assert!(!root.exists(), "the root omm created is pruned");
    assert!(!h.config_root().join("trust.json").exists());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 29 (memory seed, block, backup, gc, uninstall)");
}

/// The other half of the seed rule: a `MEMORY.md` the user wrote into is
/// theirs — the uninstall preview names it under ✓ preserve, the uninstall
/// keeps it and removes the sidecar, a later `seed` skips it and never
/// overwrites, and even `--force` leaves the unledgered file alone.
#[test]
fn s29b_an_edited_memory_seed_is_preserved_and_named_by_uninstall_and_never_reseeded() {
    let h = e2e_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let t = h.omm(&["trust", "."]);
    assert_eq!(t.code, 0, "{}", t.ctx());
    let s = h.omm(&["memory", "seed"]);
    assert_eq!(s.code, 0, "{}", s.ctx());
    let root = PathBuf::from(s.json["path"].as_str().expect("path"));
    let memory_md = root.join("MEMORY.md");
    let sidecar = root.join("omm-workspace.json");
    let seed = std::fs::read_to_string(&memory_md).expect("MEMORY.md");
    let mine = format!("{seed}\n- decision: keep the sidecar format\n");
    std::fs::write(&memory_md, &mine).expect("edit MEMORY.md");
    let memory_abs = std::fs::canonicalize(&memory_md)
        .expect("MEMORY.md")
        .display()
        .to_string();

    let dry = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(dry.code, 0, "{}", dry.ctx());
    assert!(
        dry.json["preview"]["preserve"]
            .as_array()
            .expect("preserve")
            .iter()
            .any(|p| p["path"] == memory_abs
                && p["reason"]
                    .as_str()
                    .map(|r| r.contains("edited since omm wrote it"))
                    .unwrap_or(false)),
        "{}",
        dry.ctx()
    );
    let human = h.omm_raw(&["uninstall", "--dry-run", "--yes"]);
    assert_eq!(human.code, 0, "{}", human.ctx());
    assert!(
        human.stdout.contains("✓ preserve (1)") && human.stdout.contains(&memory_abs),
        "{}",
        human.stdout
    );
    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert_eq!(un.json["report"]["preserved"], 1, "{}", un.ctx());
    assert_eq!(
        std::fs::read_to_string(&memory_md).expect("MEMORY.md"),
        mine,
        "the user's memory file is kept byte for byte"
    );
    assert!(!sidecar.exists(), "the sidecar is omm's: removed");
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(!h.config_root().join("trust.json").exists());

    // Seeded again: the user's file is skipped, never overwritten; only the
    // sidecar is written back. A forced uninstall still leaves the
    // unledgered file alone (R2).
    let t = h.omm(&["trust", "."]);
    assert_eq!(t.code, 0, "{}", t.ctx());
    let s = h.omm(&["memory", "seed"]);
    assert_eq!(s.code, 0, "{}", s.ctx());
    assert_eq!(s.json["root_created"], false, "{}", s.ctx());
    assert_eq!(s.json["memory_md"]["outcome"], "skipped", "{}", s.ctx());
    assert_eq!(
        s.json["omm_workspace_json"]["outcome"],
        "written",
        "{}",
        s.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(&memory_md).expect("MEMORY.md"),
        mine
    );
    let forced = h.omm(&["uninstall", "--force"]);
    assert_eq!(forced.code, 0, "{}", forced.ctx());
    assert_eq!(
        forced.json["report"]["errors"],
        json!([]),
        "{}",
        forced.ctx()
    );
    assert!(
        !forced.json["preview"]["remove"]
            .as_array()
            .expect("remove")
            .iter()
            .any(|p| p["path"] == memory_abs),
        "an unledgered file is never removed: {}",
        forced.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(&memory_md).expect("MEMORY.md"),
        mine
    );
    assert!(!sidecar.exists());
    assert!(h.ledger().is_none());

    // With the user's file taken away by hand, nothing of omm's is left
    // under either root (the root dirs stayed only because the file did).
    std::fs::remove_file(&memory_md).expect("the user's file, cleared");
    for dir in [
        root.clone(),
        h.data_root().join("memory").join("projects"),
        h.data_root().join("memory"),
    ] {
        if dir.is_dir() {
            std::fs::remove_dir(&dir).unwrap_or_else(|e| {
                panic!(
                    "{} is not empty after the uninstall: {e} ({:?})",
                    dir.display(),
                    files_under(&dir)
                )
            });
        }
    }
    let residue = strings(&forced.json["preview"]["residue"]);
    let after = h.snapshot();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 29b (edited memory seed)");
}

// ---------------------------------------------------------------------------
// 30. skill routing (PLAN.md 3.1, docs/ROUTING.md): enable in a trusted
//     workspace whose `.muse/hooks.json` the user already owns → the handler
//     is merged in after theirs (their bytes and mode in the ledger) → the
//     `omm run` plan carries both gates → a matching prompt through the mock
//     provider: the router completes, the host renders the order-201 block
//     naming the routed skill, the scripted model's `read_skill` returns the
//     routed body → order 200 + 201 within the 31,984 B budget, at enable
//     time and in the session → D16 on → disable → workspace byte-identical,
//     their hooks file back at its bytes and mode → uninstall → R5
// ---------------------------------------------------------------------------

#[test]
fn s30_skill_routing_merges_a_user_hooks_file_routes_through_the_mock_provider_and_disable_restores_it(
) {
    let h = e2e_or_skip!();
    let python = python_or_skip!();
    let mut f = Findings::default();
    let before = h.baseline();
    let t = h.omm(&["trust", "."]);
    assert_eq!(t.code, 0, "{}", t.ctx());
    // The user's own hooks file: compact, private, a SessionStart handler
    // and a key of their own.
    let theirs: &[u8] = br#"{"hooks":{"SessionStart":[{"matcher":"startup","hooks":[{"type":"command","command":"echo user-hook"}]}]},"note":"mine"}"#;
    let hooks_path = h.ws.join(".muse").join("hooks.json");
    std::fs::create_dir_all(h.ws.join(".muse")).expect(".muse");
    std::fs::write(&hooks_path, theirs).expect("hooks.json");
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hooks_path, std::fs::Permissions::from_mode(0o600))
            .expect("mode");
    }
    let ws_before = h.ws_snapshot();
    let untouched = h.omm_raw(&["--json", "--dry-run", "run", "--", "--version"]);
    assert_eq!(untouched.code, 0, "{}", untouched.ctx());
    assert!(
        !untouched.json["env"]
            .as_array()
            .expect("env")
            .iter()
            .any(|e| e["name"] == hr::ENV_ROUTING_GATE),
        "no gate before enable: {}",
        untouched.ctx()
    );

    let en = h.omm(&["enable", "skill-routing"]);
    assert_eq!(en.code, 0, "{}", en.ctx());
    let skills = strings(&en.json["skills"]);
    assert!(
        skills.contains(&"omm-commit-message".to_string()),
        "{skills:?}"
    );
    assert_eq!(en.json["order200_source"], "measured", "{}", en.ctx());
    let order200 = en.json["order200_bytes"].as_u64().expect("order200_bytes");
    let order201_max = en.json["order201_max_bytes"]
        .as_u64()
        .expect("order201_max_bytes");
    assert!(
        order200 + order201_max <= hr::ROUTING_BUDGET_BYTES,
        "budget at enable: {order200} + {order201_max} > {}",
        hr::ROUTING_BUDGET_BYTES
    );
    assert!(
        en.json["headroom_bytes"].as_i64().expect("headroom") > 0,
        "{}",
        en.ctx()
    );
    let handler = en.json["handler_command"]
        .as_str()
        .expect("handler_command")
        .to_string();
    assert!(handler.ends_with(" hook route"), "{handler}");
    // Merged, not replaced: theirs first, ours in its own group, their key
    // kept, the mode kept; the library copied as regular files.
    let merged: Value =
        serde_json::from_slice(&std::fs::read(&hooks_path).expect("hooks.json")).expect("JSON");
    assert_eq!(merged["note"], "mine", "{merged}");
    assert_eq!(
        merged["hooks"]["SessionStart"][0]["hooks"][0]["command"], "echo user-hook",
        "{merged}"
    );
    let groups = merged["hooks"]["UserPromptSubmit"]
        .as_array()
        .expect("UserPromptSubmit");
    assert_eq!(groups.len(), 1, "{merged}");
    let ours = &groups[0]["hooks"][0];
    assert_eq!(ours["command"], handler, "{merged}");
    assert_eq!(ours["outputCapabilities"], json!(["skills.v1"]), "{merged}");
    assert_eq!(mode_of(&hooks_path), 0o600, "the merge keeps the mode");
    let entry = h.ledger().expect("ledger")["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|e| e["base"] == "workspace" && e["path"] == ".muse/hooks.json")
        .cloned()
        .expect("hooks.json entry");
    assert_eq!(entry["mechanism"], "hooks-merge", "{entry}");
    assert_eq!(entry["class"], "shared-key", "{entry}");
    assert_eq!(
        entry["prior"]["original"]["text"]
            .as_str()
            .expect("prior text")
            .as_bytes(),
        theirs
    );
    assert_eq!(entry["prior"]["original"]["mode"], "0600", "{entry}");
    for id in &skills {
        let file = h.ws.join(".omm").join("skills").join(id).join("SKILL.md");
        assert!(
            std::fs::symlink_metadata(&file)
                .map(|m| m.is_file())
                .unwrap_or(false),
            "{} must be a regular file",
            file.display()
        );
    }
    let config: Value = serde_json::from_slice(
        &std::fs::read(h.omm_root().join("config.json")).expect("config.json"),
    )
    .expect("config JSON");
    assert_eq!(config["skill_routing"], true, "{config}");
    // The `omm run` plan carries both gates.
    let plan = h.omm_raw(&["--json", "--dry-run", "run", "--", "--version"]);
    assert_eq!(plan.code, 0, "{}", plan.ctx());
    let gates: BTreeMap<&str, &str> = plan.json["env"]
        .as_array()
        .expect("env")
        .iter()
        .filter_map(|e| Some((e["name"].as_str()?, e["value"].as_str().unwrap_or(""))))
        .collect();
    for gate in [hr::ENV_ROUTING_GATE, hr::ENV_ROUTING_APPLY_GATE] {
        assert_eq!(
            gates.get(gate).copied(),
            Some(hr::GATE_ON_VALUE),
            "{gate} in the run plan: {}",
            plan.ctx()
        );
    }

    // A matching prompt through the mock provider under both gates: the
    // router completes, the host renders order 201 with the routed skill,
    // the scripted model reads it back.
    let scripted = json!([
        {"call": {"name": "read_skill", "arguments": {"name": "omm-commit-message"}}},
        {"text": "MOCK-FINAL-ANSWER read_skill done"}
    ]);
    let mock = MockProvider::start(&python, &h.repo, &h.sb.root.join("mock"), 8752, &scripted);
    let gate_env = [
        (hr::ENV_ROUTING_GATE, hr::GATE_ON_VALUE),
        (hr::ENV_ROUTING_APPLY_GATE, hr::GATE_ON_VALUE),
    ];
    let (out, log) = h.exec_via_mock(
        &mock,
        "Write the commit message for the staged changes, then read the routed skill.",
        4,
        &gate_env,
    );
    assert!(
        out.ok(),
        "muse exec through the mock provider: rc {:?}\n--- stdout\n{}\n--- stderr\n{}",
        out.code,
        out.stdout,
        out.stderr
    );
    assert!(out.stdout.contains("MOCK-FINAL-ANSWER"), "{}", out.stdout);
    let facts = probe::parse_session_log(&log).expect("parse session log");
    let block = facts
        .block(u64::from(hr::CONTEXT_ORDER_SELECTED_SKILLS))
        .unwrap_or_else(|| panic!("no order-201 block; orders {:?}", facts.orders()));
    assert!(
        block.text.contains("id=\"omm-commit-message\""),
        "{}",
        block.text
    );
    assert!(
        block.text.contains("<description>"),
        "the descriptions were dropped (the silent outcome of the combined budget): {}",
        block.text
    );
    let catalog = facts
        .block(u64::from(hr::CONTEXT_ORDER_SKILLS_CATALOG))
        .expect("order-200 block");
    assert!(
        catalog.bytes as u64 + block.bytes as u64 <= hr::ROUTING_BUDGET_BYTES,
        "budget in the session: {} + {} > {}",
        catalog.bytes,
        block.bytes,
        hr::ROUTING_BUDGET_BYTES
    );
    assert!(
        block.bytes as u64 <= order201_max,
        "the rendered block ({} B) exceeds the worst case enable reported ({order201_max} B)",
        block.bytes
    );
    let rec = session_records(&log);
    assert!(
        rec.terminals
            .iter()
            .any(|(s, e)| s == "completed" && e.is_none()),
        "hook terminals: {:?}",
        rec.terminals
    );
    assert!(
        rec.tool_calls.iter().any(|n| n.ends_with("read_skill")),
        "{:?}",
        rec.tool_calls
    );
    let routed = rec
        .tool_results
        .iter()
        .find(|t| t.contains("<read-skill-result name=\"omm-commit-message\" status=\"ok\">"))
        .unwrap_or_else(|| {
            panic!(
                "read_skill did not resolve the routed skill: {:?}",
                rec.tool_results
            )
        });
    assert!(
        routed.contains("# Commit message"),
        "the routed body came back: {routed}"
    );
    // …and the provider saw that body as the function_call_output — in
    // whatever request after #1 carries it (1.3.0 interleaves subagent
    // requests between the call and its output).
    let posts = mock.posts();
    assert!(posts.len() >= 2, "{} provider request(s)", posts.len());
    assert!(
        posts[1..]
            .iter()
            .any(|p| p["input"]
                .as_array()
                .expect("input[]")
                .iter()
                .any(|i| i["type"] == "function_call_output"
                    && i["output"].as_str().unwrap_or("").contains(
                        "<read-skill-result name=\"omm-commit-message\" status=\"ok\">"
                    ))),
        "{posts:?}"
    );
    // Doctor D16 reads it back as on.
    let d = h.doctor(true);
    let d16 = check(&d.json, "D16");
    f.check(
        d16["severity"] == "info" && d16["observed"].as_str().unwrap_or("").contains("on in"),
        || format!("D16 with routing on: {d16}"),
    );

    // Disable: their bytes and mode back, the workspace as before, no
    // gates in the run plan any more.
    let off = h.omm(&["disable", "skill-routing"]);
    assert_eq!(off.code, 0, "{}", off.ctx());
    assert_eq!(std::fs::read(&hooks_path).expect("hooks.json"), theirs);
    assert_eq!(mode_of(&hooks_path), 0o600);
    let ws_diff = snapshot_diff(&ws_before, &h.ws_snapshot());
    assert!(
        ws_diff.is_empty(),
        "workspace differs after disable:\n  {}",
        ws_diff.join("\n  ")
    );
    assert!(!h.omm_root().join("config.json").exists());
    let plan = h.omm_raw(&["--json", "--dry-run", "run", "--", "--version"]);
    assert!(
        !plan.json["env"]
            .as_array()
            .expect("env")
            .iter()
            .any(|e| e["name"] == hr::ENV_ROUTING_GATE),
        "{}",
        plan.ctx()
    );

    let un = h.omm(&["uninstall"]);
    assert_eq!(un.code, 0, "{}", un.ctx());
    assert_eq!(un.json["report"]["errors"], json!([]), "{}", un.ctx());
    assert!(h.ledger().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
    assert!(!h.config_root().join("trust.json").exists());
    let ws_diff = snapshot_diff(&ws_before, &h.ws_snapshot());
    assert!(ws_diff.is_empty(), "{}", ws_diff.join("\n"));
    let residue = strings(&un.json["preview"]["residue"]);
    let after = h.snapshot_sans_model_catalog();
    h.check_r5(&mut f, &before, &after, &residue);
    f.finish("scenario 30 (skill routing through the mock provider)");
}

// ---------------------------------------------------------------------------
// 31. release (PLAN.md 2.5): `scripts/release.sh` on a committed copy of
//     this checkout refuses a dirty tree (an untracked file, a modified
//     file — named), a non-semver, a version below the current one and an
//     existing tag; the clean `--dry-run` prints the plan and writes nothing
// ---------------------------------------------------------------------------

#[test]
fn s31_release_script_refuses_a_dirty_tree_and_bad_versions_and_dry_runs_clean_without_writing() {
    let h = e2e_or_skip!();
    let repo = h.sb.root.join("release-repo");
    git_checkout_copy(&h.repo, &repo);
    assert_eq!(git_porcelain(&repo), "", "the copy starts clean");
    let cargo_toml = repo.join("Cargo.toml");
    let cargo_bytes = std::fs::read(&cargo_toml).expect("Cargo.toml");
    let current = workspace_version(&cargo_toml);
    let mut parts: Vec<u64> = current
        .split('.')
        .map(|p| p.parse().expect("numeric version"))
        .collect();
    assert_eq!(parts.len(), 3, "{current}");
    parts[2] += 1;
    let next = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    let release = |args: &[&str]| -> Run {
        let mut cmd = Command::new("bash");
        cmd.arg(repo.join("scripts").join("release.sh"))
            .args(args)
            .env("OMM_MUSE_BIN", &h.muse)
            .env_remove("OMM_RELEASE_TARGETS")
            .current_dir(&repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        E2e::run_cmd(cmd)
    };
    let nothing_written = |why: &str| {
        assert_eq!(git_porcelain(&repo), "", "{why}: the tree moved");
        assert!(
            !repo.join("dist-release").exists(),
            "{why}: dist-release/ appeared"
        );
        assert_eq!(
            std::fs::read(&cargo_toml).expect("Cargo.toml"),
            cargo_bytes,
            "{why}: Cargo.toml was bumped"
        );
    };

    // An untracked file: refused, named, nothing else touched.
    std::fs::write(repo.join("scratch.txt"), b"wip\n").expect("scratch");
    let r = release(&[&next, "--dry-run"]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(
        r.stderr.contains("the tree is not clean") && r.stderr.contains("scratch.txt"),
        "{}",
        r.ctx()
    );
    assert!(!repo.join("dist-release").exists());
    std::fs::remove_file(repo.join("scratch.txt")).expect("scratch");
    // A modified tracked file: refused too.
    let readme = repo.join("README.md");
    let readme_bytes = std::fs::read(&readme).expect("README.md");
    std::fs::write(&readme, [&readme_bytes[..], b"\nlocal edit\n"].concat()).expect("edit");
    let r = release(&[&next, "--dry-run"]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(
        r.stderr.contains("the tree is not clean") && r.stderr.contains("README.md"),
        "{}",
        r.ctx()
    );
    std::fs::write(&readme, &readme_bytes).expect("restore");
    nothing_written("after the refusals");

    // Bad versions on the clean tree.
    let r = release(&["abc", "--dry-run"]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(r.stderr.contains("not a semantic version"), "{}", r.ctx());
    let r = release(&["0.0.1", "--dry-run"]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(
        r.stderr
            .contains(&format!("0.0.1 is below the current version {current}")),
        "{}",
        r.ctx()
    );
    let tag = Command::new("git")
        .args(["tag", &format!("v{next}")])
        .current_dir(&repo)
        .stdin(Stdio::null())
        .output()
        .expect("git tag");
    assert!(
        tag.status.success(),
        "{}",
        String::from_utf8_lossy(&tag.stderr)
    );
    let r = release(&[&next, "--dry-run"]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(
        r.stderr.contains(&format!("tag v{next} already exists")),
        "{}",
        r.ctx()
    );
    let untag = Command::new("git")
        .args(["tag", "-d", &format!("v{next}")])
        .current_dir(&repo)
        .stdin(Stdio::null())
        .output()
        .expect("git tag -d");
    assert!(untag.status.success());
    nothing_written("after the bad versions");

    // The clean dry run: the plan, nothing written.
    let r = release(&[&next, "--dry-run"]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(
        r.stderr.contains(&format!("omm {current} → {next}")),
        "{}",
        r.ctx()
    );
    assert!(
        r.stderr.contains("dry run: would bump") && r.stderr.contains("scripts/gate.sh"),
        "{}",
        r.ctx()
    );
    assert!(
        r.stderr.contains(&format!("dist-release/omm-{next}-")),
        "{}",
        r.ctx()
    );
    assert!(r.stderr.contains("nothing was written"), "{}", r.ctx());
    nothing_written("after the clean dry run");
}

// ---------------------------------------------------------------------------
// 32. install.sh (PLAN.md 2.5, R6 two-phase) against a local HTTP server and
//     a SHA256SUMS written here: the list names the asset, the tarball is
//     verified against it, only the binary is placed (`~/.local/bin/omm`,
//     0755, the bytes of the release binary), the PATH hint is printed and
//     nothing else is written without `--modify-path`; with it exactly one
//     profile line; `--dry-run` places nothing; a tampered list, a list
//     without this target and a plain-http URL without the opt-in are
//     refused with nothing placed
// ---------------------------------------------------------------------------

#[test]
fn s32_install_sh_verifies_the_checksum_places_only_the_binary_and_refuses_a_bad_list() {
    let h = e2e_or_skip!();
    let python = python_or_skip!();
    let version = "0.9.9-e2e";
    let target = "e2e-target";
    let asset = format!("omm-{version}-{target}.tar.gz");
    // The release layout release.sh produces: omm, LICENSE, README.md, flat.
    let stage = h.sb.root.join("stage");
    std::fs::create_dir_all(&stage).expect("stage");
    std::fs::copy(&h.omm, stage.join("omm")).expect("omm");
    std::fs::copy(h.repo.join("LICENSE"), stage.join("LICENSE")).expect("LICENSE");
    std::fs::copy(h.repo.join("README.md"), stage.join("README.md")).expect("README.md");
    let www = h.sb.root.join("www");
    let vdir = www.join("download").join(format!("v{version}"));
    std::fs::create_dir_all(&vdir).expect("vdir");
    let packed = Command::new("tar")
        .arg("-czf")
        .arg(vdir.join(&asset))
        .arg("-C")
        .arg(&stage)
        .args(["omm", "LICENSE", "README.md"])
        .stdin(Stdio::null())
        .output()
        .expect("tar");
    assert!(
        packed.status.success(),
        "{}",
        String::from_utf8_lossy(&packed.stderr)
    );
    std::fs::copy(h.repo.join("install.sh"), vdir.join("install.sh")).expect("install.sh");
    let sha = omm_host::fsx::sha256_file(&vdir.join(&asset)).expect("sha256");
    let sums = format!(
        "{sha}  {asset}\n{}  install.sh\n",
        omm_host::fsx::sha256_file(&vdir.join("install.sh")).expect("sha256")
    );
    std::fs::write(vdir.join("SHA256SUMS"), &sums).expect("SHA256SUMS");
    // `latest/download/` mirrors it (what the one-liner without --version fetches).
    let latest = www.join("latest").join("download");
    std::fs::create_dir_all(&latest).expect("latest");
    for name in [asset.as_str(), "SHA256SUMS", "install.sh"] {
        std::fs::copy(vdir.join(name), latest.join(name)).expect("mirror");
    }
    let _server = StaticServer::start(&python, &www, 8753);
    let base = "http://127.0.0.1:8753";
    let omm_sha = omm_host::fsx::sha256_file(&h.omm).expect("sha256");

    let home_for = |name: &str| -> PathBuf {
        let d = h.sb.root.join(name);
        std::fs::create_dir_all(&d).expect("home");
        d
    };
    let install = |home: &Path, args: &[&str], extra: &[(&str, &str)]| -> Run {
        let mut cmd = Command::new("sh");
        cmd.arg(h.repo.join("install.sh"))
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
            .env("HOME", home)
            .env("TMPDIR", std::env::temp_dir())
            .env("SHELL", "/bin/bash")
            .env("OMM_RELEASE_BASE_URL", base)
            .env("OMM_INSTALL_ALLOW_HTTP", "1")
            .env("OMM_TARGET", target)
            .current_dir(home)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in extra {
            cmd.env(k, v);
        }
        E2e::run_cmd(cmd)
    };
    let placed_binary = |home: &Path| {
        let placed = home.join(".local").join("bin").join("omm");
        assert!(placed.is_file(), "{}", placed.display());
        assert_eq!(mode_of(&placed), 0o755);
        assert_eq!(
            omm_host::fsx::sha256_file(&placed).expect("sha256"),
            omm_sha,
            "the placed binary is the release binary"
        );
    };

    // 1. `--version`: verified, placed, the PATH hint, nothing else written.
    let home = home_for("home-version");
    let r = install(&home, &["--version", version], &[]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(
        r.stdout.is_empty(),
        "everything goes to stderr: {}",
        r.ctx()
    );
    assert!(
        r.stderr.contains(&format!("sha256 verified: {sha}")),
        "{}",
        r.ctx()
    );
    assert!(
        r.stderr
            .contains(&format!("omm {version} for {target}: {asset}")),
        "{}",
        r.ctx()
    );
    placed_binary(&home);
    assert!(
        r.stderr.contains("is not on your PATH") && r.stderr.contains("--modify-path"),
        "{}",
        r.ctx()
    );
    assert!(
        r.stderr.contains("Nothing but the binary was written"),
        "{}",
        r.ctx()
    );
    assert_eq!(
        files_under(&home),
        vec![".local/bin/omm".to_string()],
        "only the binary is written"
    );
    // 2. No version: `latest/download/` and the version read from the list.
    let home = home_for("home-latest");
    let r = install(&home, &[], &[]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(
        r.stderr
            .contains(&format!("omm {version} for {target}: {asset}")),
        "{}",
        r.ctx()
    );
    placed_binary(&home);
    assert_eq!(files_under(&home), vec![".local/bin/omm".to_string()]);
    // 3. `--modify-path`: exactly one line in the shell's profile, and
    //    nothing else; a rerun finds it and adds nothing.
    let home = home_for("home-modify");
    let r = install(&home, &["--version", version, "--modify-path"], &[]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    placed_binary(&home);
    let line = format!(
        "export PATH=\"{}:$PATH\"  # added by omm install.sh",
        home.join(".local").join("bin").display()
    );
    let bashrc = std::fs::read_to_string(home.join(".bashrc")).expect(".bashrc");
    assert_eq!(bashrc, format!("\n{line}\n"), "{bashrc}");
    assert!(r.stderr.contains("appended to ~/.bashrc"), "{}", r.ctx());
    assert_eq!(
        files_under(&home),
        vec![".bashrc".to_string(), ".local/bin/omm".to_string()]
    );
    let again = install(&home, &["--version", version, "--modify-path"], &[]);
    assert_eq!(again.code, 0, "{}", again.ctx());
    assert!(
        again.stderr.contains("already carries the PATH line"),
        "{}",
        again.ctx()
    );
    assert_eq!(
        std::fs::read_to_string(home.join(".bashrc")).expect(".bashrc"),
        bashrc
    );
    // 4. `--dry-run`: verified, nothing placed.
    let home = home_for("home-dry");
    let r = install(&home, &["--version", version, "--dry-run"], &[]);
    assert_eq!(r.code, 0, "{}", r.ctx());
    assert!(
        r.stderr.contains(&format!("sha256 verified: {sha}"))
            && r.stderr.contains("dry run: would place"),
        "{}",
        r.ctx()
    );
    assert!(files_under(&home).is_empty(), "{:?}", files_under(&home));
    // 5. A tampered list: refused with both digests, nothing placed.
    let wrong: String = sha.chars().rev().collect();
    std::fs::write(vdir.join("SHA256SUMS"), sums.replacen(&sha, &wrong, 1)).expect("tamper");
    let home = home_for("home-tampered");
    let r = install(&home, &["--version", version], &[]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(
        r.stderr.contains("sha256 mismatch")
            && r.stderr.contains(&wrong)
            && r.stderr.contains(&sha)
            && r.stderr.contains("nothing was installed"),
        "{}",
        r.ctx()
    );
    assert!(files_under(&home).is_empty(), "{:?}", files_under(&home));
    // 6. A list without this target: refused naming what is released.
    std::fs::write(
        vdir.join("SHA256SUMS"),
        sums.replace(&asset, &format!("omm-{version}-other-target.tar.gz")),
    )
    .expect("other target");
    let home = home_for("home-no-asset");
    let r = install(&home, &["--version", version], &[]);
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(
        r.stderr.contains(&format!("no asset for {target}"))
            && r.stderr.contains("omm-0.9.9-e2e-other-target.tar.gz"),
        "{}",
        r.ctx()
    );
    assert!(files_under(&home).is_empty());
    std::fs::write(vdir.join("SHA256SUMS"), &sums).expect("restore");
    // 7. Plain http without the opt-in: refused before any download.
    let home = home_for("home-http");
    let r = install(
        &home,
        &["--version", version],
        &[("OMM_INSTALL_ALLOW_HTTP", "0")],
    );
    assert_eq!(r.code, 1, "{}", r.ctx());
    assert!(r.stderr.contains("is not https"), "{}", r.ctx());
    assert!(files_under(&home).is_empty());
    // The sandbox's own HOME and roots never entered into it.
    assert!(h.sb.home.read_dir().expect("home").next().is_none());
    assert!(
        !h.omm_root().exists(),
        "omm root left behind: {:?}",
        leftover_list(&h.omm_root())
    );
}
