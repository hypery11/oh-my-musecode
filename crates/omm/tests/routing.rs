//! Skill routing end to end (PLAN.md 3.1; `docs/ROUTING.md`) against the
//! pinned host in a throwaway HOME/XDG sandbox with a git workspace:
//! `omm enable skill-routing` → the host runs `omm hook route` from
//! `<ws>/.muse/hooks.json` under both gates → the order-201 block names the
//! routed skill (and measures exactly what the formula predicts) → `omm
//! disable skill-routing` leaves the workspace and the config root byte for
//! byte as they were. The `read_skill` half of the proof needs a scripted
//! model and lives in `tools/mockprovider/run-skill-routing.sh`.
//!
//! Every host-touching test skips with a message when `OMM_MUSE_BIN` is
//! unset; the tests are serialised (the host's bootstrap-trace writer is
//! lossy under concurrent bootstraps, and one test times the hook).

use std::collections::BTreeMap;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

use omm_host::host_reality as hr;
use omm_host::{fsx, probe, Invoker, Sandbox};

static SERIAL: Mutex<()> = Mutex::new(());

struct World {
    _guard: MutexGuard<'static, ()>,
    _tmp: TempDir,
    sb: Sandbox,
    ws: PathBuf,
    muse: PathBuf,
}

fn host_bin() -> Option<PathBuf> {
    std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

macro_rules! world_or_skip {
    () => {
        match World::new() {
            Some(w) => w,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

impl World {
    fn new() -> Option<World> {
        let muse = host_bin()?;
        let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
        let tmp = tempfile::Builder::new()
            .prefix("omm-routing-")
            .tempdir()
            .expect("temp dir");
        let root = std::fs::canonicalize(tmp.path()).expect("canonical temp dir");
        let sb = Sandbox::create(&root.join("sandbox")).expect("sandbox");
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
        Some(World {
            _guard: guard,
            _tmp: tmp,
            sb,
            ws,
            muse,
        })
    }

    fn omm_root(&self) -> PathBuf {
        self.sb.config_home.join("omm")
    }

    /// `omm <args>` in the sandbox, `cwd` given, one JSON document parsed.
    fn omm_in(&self, cwd: &Path, args: &[&str]) -> (i32, Value, String, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_omm"))
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.sb.home)
            .env("XDG_CONFIG_HOME", &self.sb.config_home)
            .env("XDG_DATA_HOME", &self.sb.data_home)
            .env("OMM_MUSE_BIN", &self.muse)
            .env("MUSE_NO_AUTO_UPDATE", "1")
            .env("TMPDIR", std::env::temp_dir())
            .current_dir(cwd)
            .stdin(Stdio::null())
            .output()
            .expect("run omm");
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let json = serde_json::from_str::<Value>(stdout.trim()).unwrap_or(Value::Null);
        (out.status.code().unwrap_or(-1), json, stdout, stderr)
    }

    fn omm(&self, args: &[&str]) -> (i32, Value, String, String) {
        self.omm_in(&self.ws, args)
    }

    /// `omm hook route` with the host's scrubbed hook environment (HOME + PATH),
    /// the workspace as cwd.
    fn hook(&self, payload: &[u8]) -> (i32, Value, Duration) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_omm"));
        cmd.args(["hook", "route"])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &self.sb.home)
            .current_dir(&self.ws)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
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
        assert!(
            out.stderr.is_empty(),
            "hook stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        let json = serde_json::from_str::<Value>(stdout.trim()).unwrap_or(Value::Null);
        (out.status.code().unwrap_or(-1), json, elapsed)
    }

    /// One real session in the workspace under both gates: the hook
    /// terminals `(status, error)` and the order-201 block text.
    fn session(
        &self,
        prompt: &str,
        gates: bool,
    ) -> (Vec<(String, Option<String>)>, Option<String>) {
        let data = self
            ._tmp
            .path()
            .join(format!("session-{}", fsx::timestamp()));
        std::fs::create_dir_all(&data).expect("data dir");
        let mut inv = Invoker::new(&self.muse)
            .sandboxed(&self.sb)
            .data_home(&data)
            .cwd(&self.ws);
        if gates {
            inv = inv
                .env(hr::ENV_ROUTING_GATE, hr::GATE_ON_VALUE)
                .env(hr::ENV_ROUTING_APPLY_GATE, hr::GATE_ON_VALUE);
        }
        let out = inv
            .run(&["exec", "--provider", "echo", "--trust-workspace", prompt])
            .expect("muse exec");
        assert!(out.ok(), "muse exec failed: {}", out.stderr);
        let log =
            probe::newest_session_log(&data.join("muse").join("sessions")).expect("session log");
        let facts = probe::parse_session_log(&log).expect("parse session log");
        let block = facts
            .block(u64::from(hr::CONTEXT_ORDER_SELECTED_SKILLS))
            .map(|b| b.text.clone());
        let mut terminals = Vec::new();
        for line in std::fs::read_to_string(&log).expect("read log").lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            collect_terminals(&v, &mut terminals);
        }
        (terminals, block)
    }
}

fn collect_terminals(v: &Value, out: &mut Vec<(String, Option<String>)>) {
    match v {
        Value::Object(m) => {
            if m.get("kind").and_then(Value::as_str) == Some("hook_run_terminal") {
                out.push((
                    m.get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("?")
                        .to_string(),
                    m.get("error").and_then(Value::as_str).map(str::to_string),
                ));
            }
            if let Some(children) = m.get("children").and_then(Value::as_array) {
                for c in children {
                    if let Some(s) = c.get("record_json").and_then(Value::as_str) {
                        if let Ok(rec) = serde_json::from_str::<Value>(s) {
                            collect_terminals(&rec, out);
                        }
                    }
                }
            }
            for x in m.values() {
                collect_terminals(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_terminals(x, out)),
        _ => {}
    }
}

/// `rel → "file sha256=… mode=…" | "dir mode=…" | "symlink → …"` under `dir`,
/// `skip` prefixes excluded.
fn snapshot(dir: &Path, skip: &[&str]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if !dir.exists() {
        return out;
    }
    for entry in walkdir::WalkDir::new(dir)
        .follow_links(false)
        .sort_by_file_name()
    {
        let entry = entry.expect("walk");
        let p = entry.path();
        if p == dir {
            continue;
        }
        let rel = p
            .strip_prefix(dir)
            .expect("under dir")
            .to_string_lossy()
            .replace('\\', "/");
        if skip
            .iter()
            .any(|s| rel == *s || rel.starts_with(&format!("{s}/")))
        {
            continue;
        }
        let meta = std::fs::symlink_metadata(p).expect("stat");
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            meta.permissions().mode() & 0o7777
        };
        #[cfg(not(unix))]
        let mode = 0u32;
        let desc = if meta.file_type().is_symlink() {
            format!(
                "symlink -> {}",
                std::fs::read_link(p).expect("link").display()
            )
        } else if meta.is_dir() {
            format!("dir mode={mode:o}")
        } else {
            format!(
                "file sha256={} mode={mode:o}",
                fsx::sha256_file(p).expect("sha")
            )
        };
        out.insert(rel, desc);
    }
    out
}

fn diff(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for (k, v) in a {
        match b.get(k) {
            None => out.push(format!("- {k} ({v})")),
            Some(w) if w != v => out.push(format!("~ {k}: {v} -> {w}")),
            _ => {}
        }
    }
    for (k, v) in b {
        if !a.contains_key(k) {
            out.push(format!("+ {k} ({v})"));
        }
    }
    out
}

fn ledger(w: &World) -> Value {
    serde_json::from_slice(&std::fs::read(w.omm_root().join("omm.lock.json")).expect("ledger"))
        .expect("ledger json")
}

fn workspace_entries(l: &Value) -> Vec<String> {
    l["entries"]
        .as_array()
        .map(|es| {
            es.iter()
                .filter(|e| e["base"] == "workspace")
                .map(|e| {
                    format!(
                        "{}:{}:{}",
                        e["path"].as_str().unwrap_or("?"),
                        e["mechanism"].as_str().unwrap_or("?"),
                        e["class"].as_str().unwrap_or("?")
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

const MATCHING_PROMPT: &str = "please write the commit message for what is staged";

#[test]
fn enable_routes_a_matching_prompt_in_a_real_session_and_disable_is_byte_identical() {
    let w = world_or_skip!();
    // Untrusted: refused with the way out, nothing written.
    let (code, _, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("omm trust ."), "{err}");
    assert!(!w.ws.join(".omm").exists() && !w.ws.join(".muse").exists());
    // Not a git checkout: refused.
    let plain = w._tmp.path().join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let (code, _, _, err) = w.omm_in(&plain, &["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("git"), "{err}");
    // An unknown feature: usage error.
    let (code, _, _, err) = w.omm(&["--json", "--yes", "enable", "zzz"]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("skill-routing"), "{err}");

    // Trust the workspace (creates trust.json and the ledger), then baseline.
    let (code, _, _, err) = w.omm(&["--json", "--yes", "trust", "."]);
    assert_eq!(code, 0, "{err}");
    let skip = [
        "omm/audit.log",
        "omm/snapshots",
        "omm/locks",
        "muse/.settings.json.lock",
        "muse/.auth.json.lock",
    ];
    let ws_before = snapshot(&w.ws, &[".git"]);
    let cfg_before = snapshot(&w.sb.config_home, &skip);
    assert!(!w.omm_root().join("config.json").exists());

    // Enable.
    let (code, doc, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    let skills: Vec<String> = doc["skills"]
        .as_array()
        .expect("skills")
        .iter()
        .map(|s| s.as_str().unwrap().to_string())
        .collect();
    assert!(
        skills.contains(&"omm-commit-message".to_string()),
        "{skills:?}"
    );
    assert_eq!(doc["order200_source"], "measured", "{doc}");
    let order200 = doc["order200_bytes"].as_u64().expect("order200");
    assert!(
        order200 >= hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF,
        "{order200}"
    );
    assert!(doc["headroom_bytes"].as_i64().unwrap() > 0);
    let handler = doc["handler_command"]
        .as_str()
        .expect("handler")
        .to_string();
    assert!(handler.ends_with(" hook route"), "{handler}");
    assert!(handler.contains("omm"), "{handler}");
    for id in &skills {
        let f = w.ws.join(".omm/skills").join(id).join("SKILL.md");
        let m = std::fs::symlink_metadata(&f).expect("routed skill file");
        assert!(m.is_file(), "{} must be a regular file", f.display());
    }
    let hooks: Value =
        serde_json::from_slice(&std::fs::read(w.ws.join(".muse/hooks.json")).unwrap()).unwrap();
    let h = &hooks["hooks"]["UserPromptSubmit"][0]["hooks"][0];
    assert_eq!(h["command"], handler);
    assert_eq!(h["outputCapabilities"], json!(["skills.v1"]));
    assert_eq!(h["timeout"], 5);
    let state: Value =
        serde_json::from_slice(&std::fs::read(w.ws.join(".omm/routing.json")).unwrap()).unwrap();
    assert_eq!(state["order200_bytes"], order200);
    let config: Value =
        serde_json::from_slice(&std::fs::read(w.omm_root().join("config.json")).unwrap()).unwrap();
    assert_eq!(config["skill_routing"], true);
    assert_eq!(config["routing"]["workspace"], w.ws.display().to_string());
    assert_eq!(config["routing"]["config_created"], true);
    let entries = workspace_entries(&ledger(&w));
    assert!(
        entries.contains(&".muse/hooks.json:copy:exclusive".to_string()),
        "{entries:?}"
    );
    assert!(
        entries.contains(&".omm/routing.json:copy:exclusive".to_string()),
        "{entries:?}"
    );
    assert!(
        entries
            .iter()
            .any(|e| e.starts_with(".omm/skills/omm-commit-message/SKILL.md:copy:exclusive")),
        "{entries:?}"
    );
    // `omm run` now exports both gates.
    let (code, plan, _, err) = w.omm(&["--json", "--dry-run", "run", "--", "--version"]);
    assert_eq!(code, 0, "{err}");
    let names: Vec<&str> = plan["env"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&hr::ENV_ROUTING_GATE) && names.contains(&hr::ENV_ROUTING_APPLY_GATE),
        "{names:?}"
    );

    // The hook itself, as the host runs it: the selection, and its budget.
    let ev = json!({"hook_event_name": "UserPromptSubmit", "prompt": MATCHING_PROMPT, "session_id": "s", "turn_id": "t",
        "cwd": w.ws.display().to_string(), "transcript_path": null, "model": "unknown", "permission_mode": "default",
        "supported_output_capabilities": ["skills.v1"]});
    let (code, out, _) = w.hook(ev.to_string().as_bytes());
    assert_eq!(code, 0);
    let sel = out["hookSpecificOutput"]["selectedSkills"]
        .as_array()
        .expect("selection");
    assert_eq!(sel[0]["id"], "omm-commit-message", "{out}");
    assert!(sel.len() <= 3);
    let mut unneg = ev.clone();
    unneg
        .as_object_mut()
        .unwrap()
        .remove("supported_output_capabilities");
    assert_eq!(
        w.hook(unneg.to_string().as_bytes()).1,
        json!({}),
        "not negotiated → {{}}"
    );

    // A real session under both gates: the router runs, the block names the skill,
    // and its size is exactly what the formula predicts for that selection.
    let (terminals, block) = w.session(MATCHING_PROMPT, true);
    assert!(
        terminals
            .iter()
            .any(|(s, e)| s == "completed" && e.is_none()),
        "hook terminals: {terminals:?}"
    );
    let block = block.expect("order-201 block");
    assert!(block.contains("id=\"omm-commit-message\""), "{block}");
    let hooks_file = w.ws.join(".muse/hooks.json");
    let predicted: u64 = 280
        + sel
            .iter()
            .map(|s| {
                301 + s["id"].as_str().unwrap().len() as u64
                    + s["path"].as_str().unwrap().len() as u64
                    + hooks_file.as_os_str().len() as u64
                    + s["description"].as_str().unwrap().len() as u64
            })
            .sum::<u64>();
    assert_eq!(
        block.len() as u64,
        predicted,
        "the measured formula holds: {block}"
    );
    assert!(block.len() as u64 + order200 <= hr::ROUTING_BUDGET_BYTES);
    // Gates off: the same router answers `{}` and no block composes.
    let (terminals, block) = w.session(MATCHING_PROMPT, false);
    assert!(
        terminals.iter().all(|(s, _)| s == "completed"),
        "{terminals:?}"
    );
    assert!(block.is_none(), "no block without the gates");
    // A prompt that matches nothing: no block, the hook still completes.
    let (terminals, block) = w.session("hello, what time is it", true);
    assert!(
        terminals.iter().any(|(s, _)| s == "completed"),
        "{terminals:?}"
    );
    assert!(block.is_none());

    // Doctor D16 reads it all back as a pass.
    let (code, doctor, _, err) = w.omm(&["--json", "--yes", "doctor", "--fast"]);
    assert!(code == 0 || code == 1, "{err}");
    let d16 = doctor["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "D16")
        .expect("D16 row");
    assert_eq!(d16["severity"], "info", "{d16}");
    assert!(d16["observed"].as_str().unwrap().contains("on in"), "{d16}");

    // Idempotent: a second enable changes nothing and the ledger is byte-identical.
    let ledger_bytes = std::fs::read(w.omm_root().join("omm.lock.json")).unwrap();
    let ws_enabled = snapshot(&w.ws, &[".git", ".omm/routing.json"]);
    let (code, again, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        again["categories"]["routed-skills"]["updated"], 0,
        "{again}"
    );
    assert_eq!(again["categories"]["hooks"]["updated"], 0, "{again}");
    assert_eq!(
        std::fs::read(w.omm_root().join("omm.lock.json")).unwrap(),
        ledger_bytes
    );
    assert!(diff(
        &ws_enabled,
        &snapshot(&w.ws, &[".git", ".omm/routing.json"])
    )
    .is_empty());

    // Disable: the workspace and the config root are as before.
    let (code, off, _, err) = w.omm(&["--json", "--yes", "disable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    assert!(
        off["categories"]["routed-skills"]["removed"]
            .as_u64()
            .unwrap()
            >= skills.len() as u64,
        "{off}"
    );
    let ws_diff = diff(&ws_before, &snapshot(&w.ws, &[".git"]));
    assert!(
        ws_diff.is_empty(),
        "workspace differs after disable:\n{}",
        ws_diff.join("\n")
    );
    let cfg_diff = diff(&cfg_before, &snapshot(&w.sb.config_home, &skip));
    assert!(
        cfg_diff.is_empty(),
        "config root differs after disable:\n{}",
        cfg_diff.join("\n")
    );
    assert!(
        !w.omm_root().join("config.json").exists(),
        "config.json omm enable created is gone"
    );
    assert!(workspace_entries(&ledger(&w)).is_empty());
    // `omm run` exports nothing any more; the hook routes nothing.
    let (_, plan, _, _) = w.omm(&["--json", "--dry-run", "run", "--", "--version"]);
    assert!(!plan["env"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == hr::ENV_ROUTING_GATE));
    assert_eq!(w.hook(ev.to_string().as_bytes()).1, json!({}));
    // Disable twice is a no-op.
    let (code, _, _, err) = w.omm(&["--json", "--yes", "disable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
}

#[test]
fn a_pre_existing_hooks_file_is_merged_into_and_restored_byte_for_byte() {
    let w = world_or_skip!();
    let (code, _, _, err) = w.omm(&["--json", "--yes", "trust", "."]);
    assert_eq!(code, 0, "{err}");
    // A compact, 0600 hooks file with the user's own handlers.
    let theirs = br#"{"hooks":{"SessionStart":[{"matcher":"startup","hooks":[{"type":"command","command":"echo hi"}]}],"UserPromptSubmit":[{"hooks":[{"type":"command","command":"./their-router.sh","outputCapabilities":["skills.v1"]}]}]},"note":"mine"}"#;
    let hooks_path = w.ws.join(".muse/hooks.json");
    std::fs::create_dir_all(w.ws.join(".muse")).unwrap();
    std::fs::write(&hooks_path, theirs).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hooks_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let ws_before = snapshot(&w.ws, &[".git"]);

    let (code, doc, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    let merged: Value = serde_json::from_slice(&std::fs::read(&hooks_path).unwrap()).unwrap();
    let groups = merged["hooks"]["UserPromptSubmit"].as_array().unwrap();
    assert_eq!(
        groups.len(),
        2,
        "theirs first, ours in its own group: {merged}"
    );
    assert_eq!(groups[0]["hooks"][0]["command"], "./their-router.sh");
    assert_eq!(groups[1]["hooks"][0]["command"], doc["handler_command"]);
    assert_eq!(merged["note"], "mine");
    assert_eq!(merged["hooks"]["SessionStart"][0]["matcher"], "startup");
    let entries = workspace_entries(&ledger(&w));
    assert!(
        entries.contains(&".muse/hooks.json:hooks-merge:shared-key".to_string()),
        "{entries:?}"
    );
    let entry = ledger(&w)["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"] == ".muse/hooks.json")
        .cloned()
        .unwrap();
    assert_eq!(
        entry["prior"]["original"]["text"]
            .as_str()
            .unwrap()
            .as_bytes(),
        theirs
    );
    #[cfg(unix)]
    assert_eq!(entry["prior"]["original"]["mode"], "0600");
    // The backup of the pre-omm file landed in a snapshot.
    let snaps = snapshot(&w.omm_root().join("snapshots"), &[]);
    assert!(
        snaps.keys().any(|k| k.ends_with("workspace/hooks.json")),
        "{snaps:?}"
    );

    // Both routers run in one turn under the gates (theirs fails: no such
    // script; ours completes) and the block carries our skill.
    let (terminals, block) = w.session(MATCHING_PROMPT, true);
    assert!(
        terminals
            .iter()
            .any(|(s, e)| s == "completed" && e.is_none()),
        "{terminals:?}"
    );
    assert!(block
        .map(|b| b.contains("omm-commit-message"))
        .unwrap_or(false));

    // Disable: their bytes and mode come back exactly.
    let (code, _, _, err) = w.omm(&["--json", "--yes", "disable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(std::fs::read(&hooks_path).unwrap(), theirs);
    let ws_diff = diff(&ws_before, &snapshot(&w.ws, &[".git"]));
    assert!(ws_diff.is_empty(), "{}", ws_diff.join("\n"));

    // Enable again, then the user edits the merged file: disable takes only
    // omm's handler out and keeps everything else.
    let (code, _, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    let mut edited: Value = serde_json::from_slice(&std::fs::read(&hooks_path).unwrap()).unwrap();
    edited["note"] = json!("edited after enable");
    std::fs::write(&hooks_path, serde_json::to_vec_pretty(&edited).unwrap()).unwrap();
    let (code, off, _, err) = w.omm(&["--json", "--yes", "disable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    let after: Value = serde_json::from_slice(&std::fs::read(&hooks_path).unwrap()).unwrap();
    assert_eq!(after["note"], "edited after enable");
    assert_eq!(
        after["hooks"]["UserPromptSubmit"].as_array().unwrap().len(),
        1,
        "{after}"
    );
    assert_eq!(
        after["hooks"]["UserPromptSubmit"][0]["hooks"][0]["command"],
        "./their-router.sh"
    );
    assert!(
        off["categories"]["hooks"]["updated"].as_u64().unwrap() >= 1,
        "{off}"
    );
    assert!(workspace_entries(&ledger(&w)).is_empty());
}

#[test]
fn an_edited_routed_skill_is_kept_and_named_and_the_hook_stays_under_five_ms() {
    let w = world_or_skip!();
    let (code, _, _, err) = w.omm(&["--json", "--yes", "trust", "."]);
    assert_eq!(code, 0, "{err}");
    let (code, _, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    // The user edits one routed skill: a rerun of enable leaves it alone
    // (R3: an unchanged upstream is a no-op whatever the disk holds),
    // disable keeps it and says so, the rest goes.
    let edited = w.ws.join(".omm/skills/omm-flaky-test/SKILL.md");
    let mut bytes = std::fs::read(&edited).unwrap();
    bytes.extend_from_slice(b"\nMy own note.\n");
    std::fs::write(&edited, &bytes).unwrap();
    let (code, again, _, err) = w.omm(&["--json", "--yes", "enable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(
        again["categories"]["routed-skills"]["unchanged"], 7,
        "{again}"
    );
    assert_eq!(
        again["categories"]["routed-skills"]["updated"], 0,
        "{again}"
    );
    assert_eq!(std::fs::read(&edited).unwrap(), bytes, "never overwritten");

    // Timing: 20 cold runs of `omm hook route` on a matching prompt (R16:
    // sub-5 ms). This binary is the test profile's — unoptimised, ~4 ms of
    // process start alone — so the bound here is 12 ms; the release binary
    // is held to 5 ms the way e2e scenario 9 holds the guard
    // (`OMM_ROUTING_HOOK_BOUND_MS` overrides). Three warm-ups, one retry
    // against noise.
    let bound = std::env::var("OMM_ROUTING_HOOK_BOUND_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(if cfg!(debug_assertions) {
            Duration::from_millis(12)
        } else {
            Duration::from_millis(5)
        });
    let ev = json!({"hook_event_name": "UserPromptSubmit", "prompt": MATCHING_PROMPT, "cwd": w.ws.display().to_string(),
        "session_id": "s", "turn_id": "t", "supported_output_capabilities": ["skills.v1"]})
    .to_string();
    for _ in 0..3 {
        assert_eq!(w.hook(ev.as_bytes()).0, 0);
    }
    let mut rounds = Vec::new();
    let mut fast = false;
    for _ in 0..2 {
        let mut samples: Vec<Duration> = (0..20)
            .map(|_| {
                let (code, out, t) = w.hook(ev.as_bytes());
                assert_eq!(code, 0);
                assert_eq!(
                    out["hookSpecificOutput"]["selectedSkills"][0]["id"],
                    "omm-commit-message"
                );
                t
            })
            .collect();
        samples.sort();
        let p50 = (samples[9] + samples[10]) / 2;
        eprintln!(
            "omm hook route, 20 cold runs: p50 {p50:?} min {:?} max {:?}",
            samples[0], samples[19]
        );
        rounds.push(p50);
        if p50 < bound {
            fast = true;
            break;
        }
    }
    assert!(
        fast,
        "omm hook route p50 over {bound:?} in every round: {rounds:?} (R16)"
    );

    let (code, off, _, err) = w.omm(&["--json", "--yes", "disable", "skill-routing"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(off["categories"]["routed-skills"]["skipped"], 1, "{off}");
    assert!(edited.exists(), "the edited skill is the user's now");
    assert!(!w.ws.join(".omm/skills/omm-commit-message").exists());
    assert!(!w.ws.join(".muse/hooks.json").exists());
    let entries = workspace_entries(&ledger(&w));
    assert_eq!(
        entries.len(),
        1,
        "the edited file stays listed: {entries:?}"
    );
    assert!(entries[0].starts_with(".omm/skills/omm-flaky-test/SKILL.md"));
}
