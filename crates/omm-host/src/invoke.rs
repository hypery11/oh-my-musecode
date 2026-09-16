//! Run the Muse binary under a controlled environment.
//!
//! Every subprocess omm starts goes through [`Invoker`]:
//!
//! * `MUSE_NO_AUTO_UPDATE=1` always (launcher-only, harmless on the binary —
//!   `research/musecode/cli-surface.md` §2) and `NO_COLOR=1` always;
//! * `MUSE_EXPERIMENTAL_PLUGINS=1` **only** when the first token is `plugins`
//!   (install-time gate; runtime composition is ungated —
//!   `research/experiments/loose-ends.md` §4.6), or when a probe asks for it
//!   explicitly;
//! * a clean environment by default: only a short allowlist passes through, so a
//!   stray `MUSE_ENABLE_WEB_TOOLS=bogus` (which bricks even `--version`,
//!   cli-surface.md §4.2) or a user's gate cannot leak into a probe; `HOME` and
//!   `XDG_*` pass through unless a [`Sandbox`] overrides them; the clean
//!   environment also carries `TBH_DISABLE_FEATURE_CONFIG=1` so a probe
//!   measures the binary's gates, never the server's overrides (gates.json
//!   `remote_override`) — `omm run` inherits the user's environment untouched;
//! * the R20 allowlist is applied before spawning;
//! * every captured run has a wall-clock budget ([`RUN_TIMEOUT_DEFAULT`],
//!   [`RUN_TIMEOUT_LONG`] for `exec`/`plugins`/`serve`): a host stuck on I/O is
//!   killed — together with everything it spawned, leaves first, hook shells
//!   in their own process groups included (`residue::kill_tree`; a plain
//!   `SIGKILL` of the host pid left a `SessionStart` hook's `sleep` running
//!   as an orphan, measured 2026-09-02) — reported as [`HostError::Timeout`]
//!   with the partial output and any descendant that survived, and the
//!   registry entries it left in its runtime dir are swept
//!   (`crate::residue`); a grandchild that merely inherited the pipes cannot
//!   hold the call open past [`RUN_OUTPUT_GRACE`] on a normal exit, nor past
//!   [`RUN_KILL_OUTPUT_GRACE`] after a kill;
//! * each pipe is retained up to [`CAPTURE_CAP_BYTES`] and drained past it, so
//!   a host (or a hook it ran) that floods stdout can neither block on a full
//!   pipe nor grow omm without bound; what was dropped is counted on the
//!   [`Outcome`];
//! * exit codes are typed: `2` argv rejected (parse error OR missing gate —
//!   indistinguishable), `1` ran and failed, `0` ok (loose-ends.md §1.3).

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::allowlist::{validate_argv, ArgvPolicy};
use crate::error::{HostError, Result};
use crate::fsx;
use crate::host_reality::{
    ENV_DISABLE_FEATURE_CONFIG, ENV_NO_AUTO_UPDATE, ENV_PLUGINS_GATE, EXIT_ARGV_REJECTED, EXIT_OK,
    EXIT_RUN_FAILED, GATE_ON_VALUE,
};
use crate::locate;
use crate::paths::{EnvView, Roots};
use crate::residue;

/// Wall-clock budget of [`Invoker::run`] for every verb not listed in
/// [`default_timeout_for`]. The slowest ordinary probe (`plugins validate` on
/// a large package) finishes in ~2 s; a host that is still running after this
/// is stuck.
pub const RUN_TIMEOUT_DEFAULT: Duration = Duration::from_secs(30);
/// Budget for `exec` (an echo session composes hooks that may legitimately
/// sleep), `plugins` (a marketplace add/update clones a git repository) and
/// `serve`.
pub const RUN_TIMEOUT_LONG: Duration = Duration::from_secs(120);
/// After the host has exited, how long [`Invoker::run`] waits for EOF on its
/// pipes before returning what was captured. A grandchild that inherited the
/// pipes (`( sleep 8 ) &` in a hook) held `Command::output()` open for the
/// whole 8 s on the measured fake host; this caps that at a quarter second.
pub const RUN_OUTPUT_GRACE: Duration = Duration::from_millis(250);
/// After a timeout kill, how long [`Invoker::run`] waits for EOF on its pipes
/// when every descendant is confirmed dead. Once no writer is left the EOF is
/// immediate and the partial output is complete whatever the load — the
/// wait only ever runs its course when a pipe is held by a process the tree
/// walk could not see (one that re-parented itself to pid 1 before the
/// snapshot). With a survivor recorded, [`RUN_OUTPUT_GRACE`] applies instead.
pub const RUN_KILL_OUTPUT_GRACE: Duration = Duration::from_secs(2);
/// How often the waiter re-checks the child while a pipe is still open.
const RUN_POLL: Duration = Duration::from_millis(25);
/// Bytes of each pipe [`Invoker::run`] retains. Past this the pipe is still
/// read — so the host never blocks on a full pipe — but the bytes are counted
/// and dropped ([`Outcome::dropped`]). A wedged host flooding stdout (`exec
/// yes` as the fake host) drove omm to a 12 GB resident set and a kill 2.5×
/// past its 2 s budget when every chunk was kept; the largest honest output
/// (`plugins validate --json` on a big package, a session trace) is under a
/// megabyte.
pub const CAPTURE_CAP_BYTES: usize = 16 * 1024 * 1024;

/// The budget a verb gets when the invoker carries none of its own.
pub fn default_timeout_for<S: AsRef<str>>(argv: &[S]) -> Duration {
    match argv.first().map(AsRef::as_ref) {
        Some("exec") | Some("plugins") | Some("serve") => RUN_TIMEOUT_LONG,
        _ => RUN_TIMEOUT_DEFAULT,
    }
}

/// Environment variables that pass through in [`EnvMode::Clean`].
const PASSTHROUGH: &[&str] = &[
    "PATH",
    "HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "TMPDIR",
    "TEMP",
    "TMP",
    "USER",
    "LOGNAME",
    "SHELL",
    "LANG",
    "LC_ALL",
    "LC_CTYPE",
    "TERM",
    "TZ",
    // Honoured by the host only for Codex import/compat (config-paths.md §8.2).
    "CODEX_HOME",
    #[cfg(windows)]
    "USERPROFILE",
    #[cfg(windows)]
    "SystemRoot",
    #[cfg(windows)]
    "APPDATA",
    #[cfg(windows)]
    "LOCALAPPDATA",
    #[cfg(windows)]
    "PATHEXT",
    #[cfg(windows)]
    "COMSPEC",
];

/// A throwaway `HOME` / `XDG_CONFIG_HOME` / `XDG_DATA_HOME` triple.
///
/// Note (loose-ends.md §0 Trap B): a redirected `HOME` does **not** stop the
/// host writing `~/Library/Application Support/Muse/session-name-authority/`
/// in the real home via `getpwuid`. That residue is the host's, not ours.
#[derive(Clone, Debug)]
pub struct Sandbox {
    pub root: PathBuf,
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub data_home: PathBuf,
}

impl Sandbox {
    /// Create `home/`, `config/`, `data/` under `root`.
    pub fn create(root: &Path) -> Result<Sandbox> {
        let sb = Sandbox {
            root: root.to_path_buf(),
            home: root.join("home"),
            config_home: root.join("config"),
            data_home: root.join("data"),
        };
        for d in [&sb.home, &sb.config_home, &sb.data_home] {
            fsx::create_dir_all(d)?;
        }
        Ok(sb)
    }

    /// The host roots this sandbox resolves to.
    pub fn roots(&self) -> Result<Roots> {
        Roots::resolve(&self.env_view())
    }

    /// The environment view of this sandbox. The uid, the temp dir and the
    /// account home are the process's own: a sandbox redirects the config
    /// and data roots, not the host's per-uid runtime dir (`crate::residue`)
    /// nor the `getpwuid` home its session-name-authority residue lands in
    /// (`paths::account_home`).
    pub fn env_view(&self) -> EnvView {
        EnvView {
            home: Some(self.home.clone().into()),
            xdg_config_home: Some(self.config_home.clone().into()),
            xdg_data_home: Some(self.data_home.clone().into()),
            cwd: None,
            account_home: crate::paths::account_home(),
            uid: fsx::current_uid(),
            tmpdir: Some(std::env::temp_dir()),
        }
    }
}

/// How the child's environment is built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvMode {
    /// `env_clear()` then the passthrough allowlist. Default.
    Clean,
    /// Inherit the whole environment (for `omm run`, where the user's own gates
    /// must survive). The forced variables are still applied on top.
    Inherit,
}

/// Whether `MUSE_EXPERIMENTAL_PLUGINS=1` is set for a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginsGate {
    /// Only when the first token is `plugins` (the install-time gate).
    Auto,
    /// Always — needed to see the gated `create-plugin` bundled skill.
    Force,
    /// Never — to measure the ungated exit 2 of `plugins list`.
    Never,
}

/// Typed classification of the host's exit status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeKind {
    /// Exit 0.
    Ok,
    /// Exit 1: parsed, ran, failed.
    RunFailed,
    /// Exit 2: argv rejected — a parse error OR a missing gate.
    ArgvRejected,
    /// Any other exit code — `125` is the host's own timeout kill of its
    /// hidden PTY-gate mode (`hr::EXIT_PTY_GATE_TIMEOUT`, not a product surface).
    Other(i32),
    /// Killed by a signal (no exit code).
    Signal,
}

impl OutcomeKind {
    fn from_code(code: Option<i32>) -> Self {
        match code {
            Some(EXIT_OK) => OutcomeKind::Ok,
            Some(EXIT_RUN_FAILED) => OutcomeKind::RunFailed,
            Some(EXIT_ARGV_REJECTED) => OutcomeKind::ArgvRejected,
            Some(n) => OutcomeKind::Other(n),
            None => OutcomeKind::Signal,
        }
    }
}

/// Bytes each pipe produced beyond [`CAPTURE_CAP_BYTES`] and therefore
/// drained but not kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dropped {
    pub stdout: u64,
    pub stderr: u64,
}

impl Dropped {
    /// True when either pipe hit the cap.
    pub fn any(&self) -> bool {
        self.stdout > 0 || self.stderr > 0
    }
}

/// The captured result of one host invocation.
#[derive(Clone, Debug)]
pub struct Outcome {
    /// The argv that was run (without the program).
    pub argv: Vec<String>,
    pub code: Option<i32>,
    pub kind: OutcomeKind,
    /// The first [`CAPTURE_CAP_BYTES`] of the pipe (lossy UTF-8).
    pub stdout: String,
    /// The first [`CAPTURE_CAP_BYTES`] of the pipe (lossy UTF-8).
    pub stderr: String,
    /// What was drained past the cap on each pipe.
    pub dropped: Dropped,
}

impl Outcome {
    /// Exit 0.
    pub fn ok(&self) -> bool {
        self.kind == OutcomeKind::Ok
    }

    /// True when `stdout` or `stderr` is only the head of what the host wrote.
    pub fn truncated(&self) -> bool {
        self.dropped.any()
    }

    /// `argv` joined for messages.
    pub fn argv_string(&self) -> String {
        self.argv.join(" ")
    }

    /// Error unless exit 0.
    pub fn expect_ok(self) -> Result<Outcome> {
        if self.ok() {
            Ok(self)
        } else {
            Err(HostError::Command {
                argv: self.argv_string(),
                code: self.code,
                stderr: first_line(&self.stderr).to_string(),
            })
        }
    }

    /// The first JSON document on stdout. Some error paths of
    /// `plugins validate --json` append a plain-text line after the JSON
    /// (`research/experiments/canary-diff.md` §2.1 tooling gotcha), so only the
    /// first document is parsed.
    pub fn first_json(&self) -> Result<Value> {
        let mut stream = serde_json::Deserializer::from_str(&self.stdout).into_iter::<Value>();
        match stream.next() {
            Some(Ok(v)) => Ok(v),
            Some(Err(e)) => Err(self.json_error(e)),
            None => match serde_json::from_str::<Value>("") {
                Err(e) => Err(self.json_error(e)),
                // Unreachable: parsing "" always fails.
                Ok(v) => Ok(v),
            },
        }
    }

    fn json_error(&self, source: serde_json::Error) -> HostError {
        let context = if self.truncated() {
            format!(
                "muse {} (output truncated at {CAPTURE_CAP_BYTES} bytes per pipe; {} stdout / {} stderr bytes dropped)",
                self.argv_string(),
                self.dropped.stdout,
                self.dropped.stderr
            )
        } else {
            format!("muse {}", self.argv_string())
        };
        HostError::Json {
            context,
            source,
            stdout: truncate(&self.stdout, 2000),
            stderr: truncate(&self.stderr, 2000),
        }
    }

    /// The host's structured `{"error":{"code","message"}}` if present.
    pub fn host_reported_error(&self) -> Option<HostError> {
        let v = self.first_json().ok()?;
        let e = v.get("error")?;
        Some(HostError::HostReported {
            code: e
                .get("code")
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .to_string(),
            message: e
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
    }
}

/// A JSON-producing invocation.
#[derive(Clone, Debug)]
pub struct JsonOutcome {
    pub outcome: Outcome,
    pub json: Value,
}

/// Builder over `std::process::Command` for the Muse binary.
#[derive(Clone, Debug)]
pub struct Invoker {
    bin: PathBuf,
    env_mode: EnvMode,
    overrides: BTreeMap<OsString, Option<OsString>>,
    cwd: Option<PathBuf>,
    plugins_gate: PluginsGate,
    policy: ArgvPolicy,
    trace: bool,
    timeout: Option<Duration>,
}

impl Invoker {
    /// Wrap an already-located binary.
    pub fn new(bin: impl Into<PathBuf>) -> Invoker {
        Invoker {
            bin: bin.into(),
            env_mode: EnvMode::Clean,
            overrides: BTreeMap::new(),
            cwd: None,
            plugins_gate: PluginsGate::Auto,
            policy: ArgvPolicy::STRICT,
            trace: false,
            timeout: None,
        }
    }

    /// Locate the binary (`locate::locate`) and wrap it.
    pub fn from_env() -> Result<Invoker> {
        Ok(Invoker::new(locate::locate()?.binary))
    }

    /// The binary this invoker runs.
    pub fn bin(&self) -> &Path {
        &self.bin
    }

    /// Point `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` at a sandbox.
    pub fn sandboxed(mut self, sb: &Sandbox) -> Invoker {
        self.overrides
            .insert("HOME".into(), Some(sb.home.clone().into()));
        self.overrides.insert(
            "XDG_CONFIG_HOME".into(),
            Some(sb.config_home.clone().into()),
        );
        self.overrides
            .insert("XDG_DATA_HOME".into(), Some(sb.data_home.clone().into()));
        self
    }

    /// Override only the data root (`XDG_DATA_HOME`), e.g. so a probe's
    /// session or trace lands in a temp dir while the user's config is read.
    pub fn data_home(mut self, dir: &Path) -> Invoker {
        self.overrides
            .insert("XDG_DATA_HOME".into(), Some(dir.as_os_str().to_owned()));
        self
    }

    /// Override only the config root (`XDG_CONFIG_HOME`).
    pub fn config_home(mut self, dir: &Path) -> Invoker {
        self.overrides
            .insert("XDG_CONFIG_HOME".into(), Some(dir.as_os_str().to_owned()));
        self
    }

    /// Set one environment variable for the child.
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Invoker {
        self.overrides
            .insert(key.as_ref().to_owned(), Some(value.as_ref().to_owned()));
        self
    }

    /// Remove one environment variable from the child.
    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Invoker {
        self.overrides.insert(key.as_ref().to_owned(), None);
        self
    }

    /// Inherit the full parent environment (see [`EnvMode::Inherit`]).
    pub fn inherit_env(mut self) -> Invoker {
        self.env_mode = EnvMode::Inherit;
        self
    }

    /// Run the child in `dir` (the workspace for `exec`).
    pub fn cwd(mut self, dir: &Path) -> Invoker {
        self.cwd = Some(dir.to_path_buf());
        self
    }

    /// Allow a flag-first argv (`--version`, `--help`).
    pub fn allow_root_flags(mut self) -> Invoker {
        self.policy = ArgvPolicy::ROOT_FLAGS_OK;
        self
    }

    /// Set `MUSE_EXPERIMENTAL_PLUGINS=1` even when the first token is not
    /// `plugins` — needed to see the gated `create-plugin` bundled skill.
    pub fn with_plugins_gate(mut self) -> Invoker {
        self.plugins_gate = PluginsGate::Force;
        self
    }

    /// Never set the plugins gate, even for `plugins …` — the ungated exit-2
    /// measurement (hostcheck `exit/plugins-ungated`).
    pub fn without_plugins_gate(mut self) -> Invoker {
        self.plugins_gate = PluginsGate::Never;
        self
    }

    /// Print every spawn to stderr.
    pub fn trace(mut self, on: bool) -> Invoker {
        self.trace = on;
        self
    }

    /// Replace the per-verb default budget of [`Invoker::run`]
    /// ([`default_timeout_for`]).
    pub fn timeout(mut self, budget: Duration) -> Invoker {
        self.timeout = Some(budget);
        self
    }

    /// The budget [`Invoker::run`] applies to `argv`.
    pub fn timeout_for<S: AsRef<str>>(&self, argv: &[S]) -> Duration {
        self.timeout.unwrap_or_else(|| default_timeout_for(argv))
    }

    /// Whether this invocation would carry the plugins gate.
    pub fn plugins_gate_for(&self, argv: &[String]) -> bool {
        match self.plugins_gate {
            PluginsGate::Force => true,
            PluginsGate::Never => false,
            PluginsGate::Auto => argv.first().map(String::as_str) == Some("plugins"),
        }
    }

    /// Build the `Command` (validated, environment applied, stdio piped).
    pub fn command<S: AsRef<OsStr>>(&self, argv: &[S]) -> Result<(Command, Vec<String>)> {
        let argv_strings: Vec<String> = argv
            .iter()
            .map(|a| a.as_ref().to_string_lossy().into_owned())
            .collect();
        validate_argv(&argv_strings, self.policy)?;

        let mut cmd = Command::new(&self.bin);
        cmd.args(argv);
        if self.env_mode == EnvMode::Clean {
            cmd.env_clear();
            for key in PASSTHROUGH {
                if let Some(v) = std::env::var_os(key) {
                    cmd.env(key, v);
                }
            }
            // Applied before the overrides so `env_remove` can lift it for a
            // deliberate live-session measurement.
            cmd.env(ENV_DISABLE_FEATURE_CONFIG, "1");
        }
        for (k, v) in &self.overrides {
            match v {
                Some(v) => {
                    cmd.env(k, v);
                }
                None => {
                    cmd.env_remove(k);
                }
            }
        }
        cmd.env(ENV_NO_AUTO_UPDATE, "1");
        cmd.env("NO_COLOR", "1");
        if self.plugins_gate_for(&argv_strings) {
            cmd.env(ENV_PLUGINS_GATE, GATE_ON_VALUE);
        }
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if self.trace {
            eprintln!(
                "omm: spawn {} {}",
                self.bin.display(),
                argv_strings.join(" ")
            );
        }
        Ok((cmd, argv_strings))
    }

    /// Run and capture, within the budget of [`Invoker::timeout_for`].
    pub fn run<S: AsRef<OsStr>>(&self, argv: &[S]) -> Result<Outcome> {
        let (mut cmd, argv_strings) = self.command(argv)?;
        let budget = self.timeout_for(&argv_strings);
        let mut child = cmd.spawn().map_err(|e| HostError::Spawn {
            program: self.bin.clone(),
            source: e,
        })?;
        let pid = child.id();
        let (eof_tx, eof_rx) = mpsc::channel::<()>();
        let stdout = drain(child.stdout.take(), eof_tx.clone());
        let stderr = drain(child.stderr.take(), eof_tx);
        let mut eofs = 0usize;
        let deadline = Instant::now() + budget;
        let status = loop {
            if let Some(status) = child.try_wait().map_err(|e| HostError::Spawn {
                program: self.bin.clone(),
                source: e,
            })? {
                break Some(status);
            }
            let now = Instant::now();
            if now >= deadline {
                break None;
            }
            let slice = RUN_POLL.min(deadline - now);
            if eofs >= 2 {
                // Both pipes are closed but the host is still running: nothing
                // to wake us up, so poll.
                std::thread::sleep(slice.min(Duration::from_millis(5)));
            } else {
                match eof_rx.recv_timeout(slice) {
                    Ok(()) => eofs += 1,
                    Err(RecvTimeoutError::Disconnected) => eofs = 2,
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        };
        let status = match status {
            Some(status) => status,
            None => {
                let killed = self.kill(&mut child, pid, budget);
                // Every writer is dead: EOF is immediate and the partial
                // output complete. A survivor may still hold a pipe, so only
                // the short grace applies then.
                let grace = if killed.survivors.is_empty() && !killed.tree_unavailable {
                    RUN_KILL_OUTPUT_GRACE
                } else {
                    RUN_OUTPUT_GRACE
                };
                await_eof(&eof_rx, &mut eofs, grace);
                return Err(HostError::Timeout {
                    argv: argv_strings.join(" "),
                    timeout: budget,
                    pid,
                    survivors: killed.survivors,
                    stdout: partial(&take(&stdout).0, 2000),
                    stderr: partial(&take(&stderr).0, 2000),
                });
            }
        };
        // The host has exited; give the pipes a moment to deliver EOF (a
        // grandchild holding them is not waited for).
        await_eof(&eof_rx, &mut eofs, RUN_OUTPUT_GRACE);
        let (out, out_dropped) = take(&stdout);
        let (err, err_dropped) = take(&stderr);
        Ok(outcome(
            argv_strings,
            status,
            &out,
            &err,
            Dropped {
                stdout: out_dropped,
                stderr: err_dropped,
            },
        ))
    }

    /// Kill a host that overran its budget — its descendants first, leaves
    /// first, then the host, then a re-walk for anything that appeared in
    /// between ([`residue::kill_tree`]) — reap it, and sweep the registry
    /// entries it would have removed on a normal exit. Under `trace` one line
    /// names what was killed and what survived.
    fn kill(&self, child: &mut Child, pid: u32, budget: Duration) -> residue::TreeKill {
        let killed = residue::kill_tree(pid, || {
            let _ = child.kill();
            let _ = child.wait();
        });
        if self.trace {
            eprintln!(
                "omm: killed pid {pid} after {:.1} s: {}",
                budget.as_secs_f64(),
                killed.summary()
            );
        }
        match residue::sweep_killed_pid(pid) {
            Ok(swept) if self.trace && !swept.is_empty() => {
                eprintln!(
                    "omm: swept {} endpoint file(s) and {} registry entr(y/ies) of killed pid {pid}",
                    swept.endpoints.len(),
                    swept.session_entries.len()
                );
            }
            Ok(_) => {}
            Err(e) if self.trace => eprintln!("omm: residue sweep after killing pid {pid}: {e}"),
            Err(_) => {}
        }
        killed
    }

    /// Run, capture, and parse the first JSON document on stdout. The exit
    /// code is *not* checked here: the host's `{"error":…}` documents arrive
    /// with exit 1 and are still JSON.
    pub fn run_json<S: AsRef<OsStr>>(&self, argv: &[S]) -> Result<JsonOutcome> {
        let outcome = self.run(argv)?;
        let json = outcome.first_json()?;
        Ok(JsonOutcome { outcome, json })
    }

    /// Replace the current process with the host (the `omm run` shim). Stdio
    /// is inherited. Only returns on failure to exec.
    #[cfg(unix)]
    pub fn exec<S: AsRef<OsStr>>(&self, argv: &[S]) -> Result<std::convert::Infallible> {
        use std::os::unix::process::CommandExt;
        let (mut cmd, _) = self.command(argv)?;
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let err = cmd.exec();
        Err(HostError::Spawn {
            program: self.bin.clone(),
            source: err,
        })
    }

    /// Run with inherited stdio and return the exit status (portable fallback
    /// for [`Invoker::exec`]).
    pub fn spawn_inherited<S: AsRef<OsStr>>(&self, argv: &[S]) -> Result<Option<i32>> {
        let (mut cmd, _) = self.command(argv)?;
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        let status = cmd.status().map_err(|e| HostError::Spawn {
            program: self.bin.clone(),
            source: e,
        })?;
        Ok(status.code())
    }
}

/// The capture state of one pipe: the retained head, what was drained past
/// the cap, and whether the caller already took the head.
#[derive(Default)]
struct Capture {
    bytes: Vec<u8>,
    dropped: u64,
    /// Set by [`take`]: an orphaned drain thread (a grandchild still holding
    /// the pipe after the host exited) then retains nothing for the life of
    /// the omm process.
    detached: bool,
}

/// Shared capture buffer of one pipe.
type Captured = Arc<Mutex<Capture>>;

/// Copy a pipe into a shared buffer on its own thread, keeping at most
/// [`CAPTURE_CAP_BYTES`] and counting the rest; `eof` fires once the pipe
/// closes (or immediately when there is no pipe). The thread ends with the
/// pipe — a grandchild holding it keeps only the thread alive, never the
/// caller — and reading never stops, so the writer never blocks on omm.
fn drain<R: Read + Send + 'static>(pipe: Option<R>, eof: mpsc::Sender<()>) -> Captured {
    let buf: Captured = Arc::new(Mutex::new(Capture::default()));
    match pipe {
        None => {
            let _ = eof.send(());
        }
        Some(mut pipe) => {
            let sink = Arc::clone(&buf);
            std::thread::spawn(move || {
                let mut chunk = [0u8; 64 * 1024];
                loop {
                    match pipe.read(&mut chunk) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let mut c = sink
                                .lock()
                                .unwrap_or_else(std::sync::PoisonError::into_inner);
                            if c.detached {
                                continue;
                            }
                            let keep = n.min(CAPTURE_CAP_BYTES.saturating_sub(c.bytes.len()));
                            c.bytes.extend_from_slice(&chunk[..keep]);
                            c.dropped += (n - keep) as u64;
                        }
                    }
                }
                let _ = eof.send(());
            });
        }
    }
    buf
}

/// Wait up to `grace` for the pipes still open to close.
fn await_eof(rx: &mpsc::Receiver<()>, eofs: &mut usize, grace: Duration) {
    let deadline = Instant::now() + grace;
    while *eofs < 2 {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        match rx.recv_timeout(deadline - now) {
            Ok(()) => *eofs += 1,
            Err(RecvTimeoutError::Disconnected) => *eofs = 2,
            Err(RecvTimeoutError::Timeout) => break,
        }
    }
}

/// Take the retained head and the dropped count, detaching the drain thread.
fn take(buf: &Captured) -> (Vec<u8>, u64) {
    let mut c = buf
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    c.detached = true;
    (std::mem::take(&mut c.bytes), c.dropped)
}

fn outcome(
    argv: Vec<String>,
    status: ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
    dropped: Dropped,
) -> Outcome {
    let code = status.code();
    Outcome {
        argv,
        code,
        kind: OutcomeKind::from_code(code),
        stdout: String::from_utf8_lossy(stdout).into_owned(),
        stderr: String::from_utf8_lossy(stderr).into_owned(),
        dropped,
    }
}

/// The first `max` bytes of a capture as text, converted from only the head
/// it needs — the Timeout partials must not walk the whole buffer.
fn partial(bytes: &[u8], max: usize) -> String {
    let head = &bytes[..bytes.len().min(max + 4)];
    let text = String::from_utf8_lossy(head);
    if bytes.len() > max {
        truncate(&text, max)
    } else {
        text.into_owned()
    }
}

pub(crate) fn first_line(s: &str) -> &str {
    s.lines().next().unwrap_or("").trim()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let mut end = max;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_kinds_map_exit_codes() {
        assert_eq!(OutcomeKind::from_code(Some(0)), OutcomeKind::Ok);
        assert_eq!(OutcomeKind::from_code(Some(1)), OutcomeKind::RunFailed);
        assert_eq!(OutcomeKind::from_code(Some(2)), OutcomeKind::ArgvRejected);
        assert_eq!(OutcomeKind::from_code(Some(125)), OutcomeKind::Other(125));
        assert_eq!(OutcomeKind::from_code(None), OutcomeKind::Signal);
    }

    #[test]
    fn first_json_takes_the_first_document_only() {
        let o = Outcome {
            argv: vec![],
            code: Some(1),
            kind: OutcomeKind::RunFailed,
            stdout: "{\"error\":{\"code\":\"x\",\"message\":\"m\"}}\nplain trailing text\n".into(),
            stderr: String::new(),
            dropped: Dropped::default(),
        };
        let v = o.first_json().unwrap();
        assert_eq!(v["error"]["code"], "x");
        assert!(matches!(
            o.host_reported_error(),
            Some(HostError::HostReported { .. })
        ));
        let empty = Outcome {
            stdout: String::new(),
            ..o
        };
        assert!(matches!(empty.first_json(), Err(HostError::Json { .. })));
    }

    #[test]
    fn plugins_gate_only_for_plugins_or_forced() {
        let inv = Invoker::new("/nonexistent/muse");
        assert!(inv.plugins_gate_for(&["plugins".into(), "list".into()]));
        assert!(!inv.plugins_gate_for(&["skills".into(), "list".into()]));
        assert!(inv
            .clone()
            .with_plugins_gate()
            .plugins_gate_for(&["skills".into()]));
        assert!(!inv
            .clone()
            .without_plugins_gate()
            .plugins_gate_for(&["plugins".into(), "list".into()]));
        let (cmd, _) = inv
            .clone()
            .without_plugins_gate()
            .command(&["plugins", "list"])
            .unwrap();
        assert!(
            !cmd.get_envs()
                .any(|(k, _)| k == OsStr::new(ENV_PLUGINS_GATE)),
            "Never must not set the gate at all"
        );
    }

    #[test]
    fn clean_env_disables_the_remote_feature_config_but_inherit_does_not() {
        // gates.json `remote_override`: the feature-config fetch can flip gates
        // server-side; a probe must measure the binary, not the server.
        let clean = Invoker::new("/nonexistent/muse");
        let (cmd, _) = clean.command(&["skills", "list"]).unwrap();
        let envs: BTreeMap<_, _> = cmd
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(|v| v.to_owned())))
            .collect();
        assert_eq!(
            envs.get(OsStr::new(crate::host_reality::ENV_DISABLE_FEATURE_CONFIG)),
            Some(&Some(OsString::from("1"))),
            "clean environment must carry TBH_DISABLE_FEATURE_CONFIG=1"
        );
        // `omm run` inherits the user's environment untouched.
        let (cmd, _) = clean
            .clone()
            .inherit_env()
            .command(&["skills", "list"])
            .unwrap();
        assert!(
            !cmd.get_envs()
                .any(|(k, _)| k == OsStr::new(crate::host_reality::ENV_DISABLE_FEATURE_CONFIG)),
            "inherit mode must not force the variable"
        );
        // An explicit override lifts it (doctor's live-session lane).
        let (cmd, _) = clean
            .clone()
            .env_remove(crate::host_reality::ENV_DISABLE_FEATURE_CONFIG)
            .command(&["skills", "list"])
            .unwrap();
        // After `env_clear` a removal drops the entry outright (nothing to
        // unset in the child), so the variable must simply be absent.
        assert!(
            !cmd.get_envs().any(|(k, v)| {
                k == OsStr::new(crate::host_reality::ENV_DISABLE_FEATURE_CONFIG) && v.is_some()
            }),
            "env_remove must win over the forced value"
        );
    }

    #[cfg(unix)]
    fn fake_host(dir: &Path, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("muse-bin-fake");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// The tests that measure wall-clock time or memory, or flood a pipe,
    /// run one at a time: a `yes` flooding 64 KB chunks or a 64 MB `head`
    /// on a sibling thread is exactly the load that made a 1.5 s budget
    /// miss a shell start. A poisoned lock only means a sibling panicked.
    static TIMING_LOCK: Mutex<()> = Mutex::new(());

    fn timing_guard() -> std::sync::MutexGuard<'static, ()> {
        TIMING_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Poll for a file the fake host writes (its start barrier) up to `wait`.
    #[cfg(unix)]
    fn read_marker(path: &Path, wait: Duration) -> Option<String> {
        let deadline = Instant::now() + wait;
        loop {
            if let Ok(text) = std::fs::read_to_string(path) {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_returns_when_the_host_exits_even_if_a_grandchild_holds_the_pipes() {
        // A hook that backgrounds `( sleep 6 ) &` inherits stdout; `output()`
        // waited for EOF and blocked 8.3 s on the measured fake host.
        let _serial = timing_guard();
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_host(dir.path(), "#!/bin/sh\n( sleep 6 ) &\necho done\nexit 0\n");
        let started = std::time::Instant::now();
        let out = Invoker::new(&bin).run(&["skills", "list"]).unwrap();
        let elapsed = started.elapsed();
        assert!(out.ok());
        assert_eq!(out.stdout.trim(), "done");
        assert!(
            elapsed < std::time::Duration::from_secs(3),
            "run() waited {elapsed:?} for a grandchild that merely inherited the pipes"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_kills_a_wedged_host_at_the_deadline_and_reports_partial_output() {
        // muse-cli.json `hidden_argv_modes`: the PTY-gate mode hangs forever;
        // `Command::output()` waited 20.33 s on the measured fake host.
        //
        // Deterministic under load: the timing tests are serialized behind
        // `TIMING_LOCK`; the fake host writes a `ready` marker only AFTER its
        // partial output is in the pipes (an explicit start barrier the test
        // checks, so a shell that never got that far fails loudly instead of
        // as a mismatched capture); the budget is 3 s, not 1.5 s; and on the
        // kill path every writer is dead, so the partial output is read to
        // EOF rather than for whatever a 250 ms grace happened to deliver.
        let _serial = timing_guard();
        let dir = tempfile::tempdir().unwrap();
        let ready = dir.path().join("ready");
        let bin = fake_host(
            dir.path(),
            &format!(
                "#!/bin/sh\necho partial\necho oops >&2\necho $$ > '{}'\nsleep 20\necho done\n",
                ready.display()
            ),
        );
        let budget = Duration::from_secs(3);
        let started = std::time::Instant::now();
        let err = Invoker::new(&bin)
            .timeout(budget)
            .run(&["skills", "list"])
            .unwrap_err();
        let elapsed = started.elapsed();
        let host_pid: u32 = read_marker(&ready, Duration::ZERO)
            .unwrap_or_else(|| {
                panic!("the fake host never reached its start barrier within {budget:?} (elapsed {elapsed:?}); the shell did not start in time")
            })
            .trim()
            .parse()
            .unwrap();
        match &err {
            HostError::Timeout {
                argv,
                timeout,
                pid,
                survivors,
                stdout,
                stderr,
            } => {
                assert_eq!(argv, "skills list");
                assert_eq!(*timeout, budget);
                assert_eq!(*pid, host_pid);
                assert_eq!(stdout.trim(), "partial");
                assert_eq!(stderr.trim(), "oops");
                assert!(survivors.is_empty(), "{survivors:?}");
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
        assert!(
            elapsed >= budget,
            "killed at {elapsed:?}, before the {budget:?} budget"
        );
        assert!(
            elapsed < budget + Duration::from_secs(5),
            "kill took {elapsed:?}; the host must not be waited for"
        );
        // The host and the `sleep 20` it spawned are both gone.
        assert!(residue::pids_alive(&std::collections::BTreeSet::from([host_pid])).is_empty());
        let msg = err.to_string();
        assert!(msg.contains("did not finish within 3 s"), "{msg}");
        assert!(!msg.contains("survived"), "{msg}");
        // Budgets: per verb by default, the builder overrides.
        let inv = Invoker::new(&bin);
        assert_eq!(inv.timeout_for(&["skills", "list"]), RUN_TIMEOUT_DEFAULT);
        assert_eq!(inv.timeout_for(&["exec", "hi"]), RUN_TIMEOUT_LONG);
        assert_eq!(
            inv.timeout_for(&["plugins", "install", "x"]),
            RUN_TIMEOUT_LONG
        );
        assert_eq!(
            inv.clone()
                .timeout(Duration::from_secs(1))
                .timeout_for(&["exec", "hi"]),
            Duration::from_secs(1)
        );
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kill_takes_the_hosts_descendants_in_their_own_process_groups_with_it() {
        // Gate 0 residual: `Child::kill` SIGKILLs the host pid alone, so a
        // hook the host spawned survived as an orphan — own pgid, ppid 1 —
        // for the rest of its `sleep`. The fake host mirrors a backgrounded
        // hook: a subshell child that puts its own `sleep` (the grandchild)
        // into a new process group with `set -m`, plus the host's own sleep.
        let _serial = timing_guard();
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let bin = fake_host(
            dir.path(),
            &format!(
                "#!/bin/sh\n\
                 ( set -m; sleep 30 & echo $! > '{o}/grandchild.pid'; ps -o pgid= -p $! > '{o}/grandchild.pgid'; wait ) &\n\
                 echo $! > '{o}/child.pid'\n\
                 ps -o pgid= -p $$ > '{o}/host.pgid'\n\
                 echo $$ > '{o}/host.pid'\n\
                 sleep 30\n",
                o = out.display()
            ),
        );
        let err = Invoker::new(&bin)
            .timeout(Duration::from_secs(3))
            .trace(true)
            .run(&["skills", "list"])
            .unwrap_err();
        let pid_in = |name: &str| -> u32 {
            read_marker(&out.join(name), Duration::ZERO)
                .unwrap_or_else(|| {
                    panic!("{name} was never written: the fake host did not start in time")
                })
                .trim()
                .parse()
                .unwrap()
        };
        let (host, child, grandchild) = (
            pid_in("host.pid"),
            pid_in("child.pid"),
            pid_in("grandchild.pid"),
        );
        let (host_pgid, grandchild_pgid) = (pid_in("host.pgid"), pid_in("grandchild.pgid"));
        assert_ne!(
            grandchild_pgid, host_pgid,
            "the grandchild must sit in its own process group for this test to mean anything"
        );
        let survivors = match &err {
            HostError::Timeout { pid, survivors, .. } => {
                assert_eq!(*pid, host);
                survivors.clone()
            }
            other => panic!("expected Timeout, got {other:?}"),
        };
        assert!(survivors.is_empty(), "{survivors:?}");
        let alive =
            residue::pids_alive(&std::collections::BTreeSet::from([host, child, grandchild]));
        assert!(
            alive.is_empty(),
            "orphans of killed host {host} still alive: {alive:?} (child {child}, grandchild {grandchild} in pgid {grandchild_pgid})"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_captures_large_output_and_a_signal_death() {
        let _serial = timing_guard();
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_host(
            dir.path(),
            "#!/bin/sh\nhead -c 3000000 /dev/zero | tr '\\0' 'x'\nkill -9 $$\n",
        );
        let out = Invoker::new(&bin).run(&["skills", "list"]).unwrap();
        assert_eq!(out.kind, OutcomeKind::Signal);
        assert_eq!(out.code, None);
        assert_eq!(
            out.stdout.len(),
            3_000_000,
            "output before the signal is kept"
        );
        let msg = out.expect_ok().unwrap_err().to_string();
        assert!(msg.contains("killed by signal"), "{msg}");
    }

    #[cfg(unix)]
    fn rss_kib_of_self() -> u64 {
        let out = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn run_bounds_what_it_retains_from_a_flooding_wedged_host() {
        // `exec yes …` under a 2 s budget drove omm to a 12 GB RSS and a 5 s
        // kill (measured): every 64 KB chunk was appended to an unbounded
        // Vec and the Timeout partials walked the whole buffer.
        let _serial = timing_guard();
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_host(
            dir.path(),
            "#!/bin/sh\nexec yes xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n",
        );
        // 2 s (the measured budget) also covers a cold `/bin/sh` + `yes`
        // start under a parallel test load.
        let budget = Duration::from_secs(2);
        let started = Instant::now();
        let err = Invoker::new(&bin)
            .timeout(budget)
            .run(&["skills", "list"])
            .unwrap_err();
        let elapsed = started.elapsed();
        let rss = rss_kib_of_self();
        match &err {
            HostError::Timeout { stdout, .. } => {
                assert!(stdout.starts_with("xxxx"), "{stdout:?}");
                assert!(stdout.chars().count() <= 2001, "{}", stdout.len());
            }
            other => panic!("expected Timeout, got {other:?}"),
        }
        assert!(
            elapsed < budget + RUN_OUTPUT_GRACE + Duration::from_millis(1500),
            "kill took {elapsed:?} for a {budget:?} budget"
        );
        assert!(
            rss < 100 * 1024,
            "omm's own RSS is {rss} KiB after draining a flooding host"
        );
    }

    #[cfg(unix)]
    #[test]
    fn run_caps_each_pipe_and_reports_the_truncation() {
        // 64 MB of stdout: only the head is retained, the rest is drained and
        // counted so the host never blocks on a full pipe.
        let _serial = timing_guard();
        let dir = tempfile::tempdir().unwrap();
        let bin = fake_host(
            dir.path(),
            "#!/bin/sh\nhead -c 67108864 /dev/zero | tr '\\0' 'x'\necho tail >&2\nexit 0\n",
        );
        let out = Invoker::new(&bin).run(&["skills", "list"]).unwrap();
        assert!(out.ok());
        assert_eq!(out.stdout.len(), CAPTURE_CAP_BYTES);
        assert!(out.stdout.bytes().all(|b| b == b'x'));
        assert_eq!(out.dropped.stdout, 67_108_864 - CAPTURE_CAP_BYTES as u64);
        assert_eq!(out.dropped.stderr, 0);
        assert!(out.truncated());
        assert_eq!(out.stderr.trim(), "tail");
        // A JSON parse of the truncated stream says why it failed.
        let msg = out.first_json().unwrap_err().to_string();
        assert!(msg.contains("truncated"), "{msg}");
        // The 3 MB run above stays whole.
        assert!(!Outcome {
            argv: vec![],
            code: Some(0),
            kind: OutcomeKind::Ok,
            stdout: String::new(),
            stderr: String::new(),
            dropped: Dropped::default(),
        }
        .truncated());
    }

    #[test]
    fn a_signal_death_renders_as_killed_by_signal() {
        let e = HostError::Command {
            argv: "skills list".into(),
            code: None,
            stderr: String::new(),
        };
        let msg = e.to_string();
        assert!(msg.contains("killed by signal"), "{msg}");
        assert!(!msg.contains("None"), "{msg}");
        let e = HostError::Command {
            argv: "skills list".into(),
            code: Some(1),
            stderr: "boom".into(),
        };
        assert!(e.to_string().contains("exited 1: boom"), "{e}");
    }

    #[test]
    fn command_builds_clean_env_and_refuses_prompts() {
        let inv = Invoker::new("/nonexistent/muse").env("ZZ_TEST", "1");
        let (cmd, argv) = inv.command(&["plugins", "list"]).unwrap();
        assert_eq!(argv, vec!["plugins", "list"]);
        let envs: BTreeMap<_, _> = cmd
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(|v| v.to_owned())))
            .collect();
        assert_eq!(
            envs.get(OsStr::new(ENV_PLUGINS_GATE)),
            Some(&Some(OsString::from("1")))
        );
        assert_eq!(
            envs.get(OsStr::new(ENV_NO_AUTO_UPDATE)),
            Some(&Some(OsString::from("1")))
        );
        assert_eq!(
            envs.get(OsStr::new("ZZ_TEST")),
            Some(&Some(OsString::from("1")))
        );
        assert!(matches!(
            inv.command(&["zzznotacommand"]),
            Err(HostError::Argv(_))
        ));
        assert!(inv.command(&["--version"]).is_err());
        assert!(inv
            .clone()
            .allow_root_flags()
            .command(&["--version"])
            .is_ok());
    }
}
