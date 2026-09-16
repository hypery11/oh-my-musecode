//! R16 / `content/hooks/README.md`: `omm hook <name>` must never exit 2 unless it
//! means to block. A stub or an unknown name prints `{}` and exits 0, so the
//! binary on PATH can never turn every PreToolUse and Stop into a block.

use std::io::Write;
use std::process::{Command, Stdio};

fn run_hook(name: &str, stdin: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omm"))
        .args(["hook", name])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn omm");
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(stdin)
        .expect("write payload");
    child.wait_with_output().expect("wait for omm")
}

fn assert_fail_open(name: &str, stdin: &[u8]) {
    let out = run_hook(name, stdin);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "omm hook {name} with stdin {:?}: rc={:?} stdout={stdout:?} stderr={stderr:?}",
        String::from_utf8_lossy(stdin),
        out.status.code()
    );
    assert_eq!(
        stdout.trim(),
        "{}",
        "omm hook {name}: stdout must be `{{}}`"
    );
    let decision: serde_json::Value = serde_json::from_str(stdout.trim()).expect("stdout is JSON");
    assert_eq!(decision, serde_json::json!({}));
    assert!(
        stderr.is_empty(),
        "omm hook {name}: stderr must be empty, got {stderr:?}"
    );
}

#[test]
fn shipped_hook_names_fail_open_on_an_empty_event() {
    // The ids in content/hooks/*.json, each with the README's minimal payload.
    for name in [
        "guard",
        "session-start",
        "stop",
        "skill-gate",
        "intent-gate",
        "subagent-start",
        "subagent-stop",
    ] {
        assert_fail_open(name, b"{}");
    }
}

#[test]
fn unknown_hook_name_and_bad_payloads_fail_open() {
    assert_fail_open("no-such-hook", b"{}");
    assert_fail_open("stop", b"");
    assert_fail_open("session-start", b"not json at all");
    assert_fail_open(
        "stop",
        b"{\"hook_event_name\":\"Stop\",\"stop_hook_active\":true}",
    );
}

/// The guard's one exception to `{}` (Gate 1 decision G): a payload it cannot
/// evaluate — malformed, empty, or cut at the 4 MiB stdin bound — is a deny
/// carried in the JSON, exit 0 and no stderr (still fail-open at the process
/// level, R16). Below the bound a padded command is evaluated whole.
fn assert_guard_denies_unevaluable(stdin: &[u8]) {
    let out = run_hook("guard", stdin);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "rc={:?} stdout={stdout:?} stderr={stderr:?}",
        out.status.code()
    );
    assert!(stderr.is_empty(), "stderr must be empty, got {stderr:?}");
    let decision: serde_json::Value = serde_json::from_str(stdout.trim()).expect("stdout is JSON");
    assert_eq!(
        decision["hookSpecificOutput"]["hookEventName"], "PreToolUse",
        "{decision}"
    );
    assert_eq!(
        decision["hookSpecificOutput"]["permissionDecision"], "deny",
        "{decision}"
    );
    assert!(
        decision["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .contains("too large or malformed to evaluate"),
        "{decision}"
    );
}

#[test]
fn guard_denies_what_it_cannot_evaluate_and_reads_up_to_four_mib() {
    assert_guard_denies_unevaluable(b"");
    assert_guard_denies_unevaluable(b"not json at all");
    let padded = |n: usize| {
        serde_json::json!({"hook_event_name": "PreToolUse", "tool_name": "bash",
            "tool_input": {"command": format!("rm -rf / #{}", "x".repeat(n))}, "tool_use_id": "t"})
        .to_string()
    };
    assert_guard_denies_unevaluable(padded(4 << 20).as_bytes());
    // Below the bound: evaluated whole and denied on its rule.
    let out = run_hook("guard", padded(3 << 20).as_bytes());
    assert_eq!(out.status.code(), Some(0));
    let decision: serde_json::Value =
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).expect("JSON");
    assert_eq!(decision["hookSpecificOutput"]["permissionDecision"], "deny");
    assert!(
        decision["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap_or("")
            .starts_with("omm guard: rm-root"),
        "{decision}"
    );
}

#[test]
fn no_command_is_a_stub_any_more_and_the_hook_never_was() {
    // `mcp` was the last stub (PLAN.md 2.2); it is now the stdio server,
    // which exits 0 at EOF with nothing on either stream.
    let out = Command::new(env!("CARGO_BIN_EXE_omm"))
        .args(["mcp"])
        .stdin(Stdio::null())
        .output()
        .expect("run omm mcp");
    assert_eq!(out.status.code(), Some(0));
    assert!(
        out.stdout.is_empty(),
        "{:?}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(!String::from_utf8_lossy(&out.stderr).contains("not implemented"));
    let out = run_hook("guard", b"{}");
    assert!(!String::from_utf8_lossy(&out.stderr).contains("not implemented"));
}

/// The documented kill switch is enforced on the dispatch path (research/ohmy/
/// 01-PARITY.md row 42; content/hooks/README.md): `$OMM/config.json →
/// "disabled": ["hook:omm-guard"]` turns the guard into `{}`, other hooks stay
/// live, and a malformed config disables nothing.
#[test]
fn config_disabled_switches_a_hook_off_on_the_dispatch_path() {
    let deny_payload = br#"{"hook_event_name":"PreToolUse","tool_name":"bash","tool_input":{"command":"rm -rf /"},"tool_use_id":"t1"}"#;
    let stop_payload = br#"{"hook_event_name":"Stop","stop_hook_active":false,"last_assistant_message":"The feature is implemented and ready.","turn_id":"x"}"#;

    let run_with_config = |config: Option<&str>, name: &str, stdin: &[u8]| -> serde_json::Value {
        let home = tempfile::tempdir().expect("tempdir");
        let cfg = home.path().join("cfg");
        std::fs::create_dir_all(cfg.join("omm")).expect("mkdir omm");
        if let Some(c) = config {
            std::fs::write(cfg.join("omm").join("config.json"), c).expect("write config");
        }
        let mut child = Command::new(env!("CARGO_BIN_EXE_omm"))
            .args(["hook", name])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", home.path())
            .env("XDG_CONFIG_HOME", &cfg)
            .env("XDG_DATA_HOME", home.path().join("data"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn omm");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(stdin)
            .expect("write");
        let out = child.wait_with_output().expect("wait");
        assert_eq!(
            out.status.code(),
            Some(0),
            "{:?}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "{:?}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_str(String::from_utf8_lossy(&out.stdout).trim()).expect("JSON")
    };

    // Control: no config → the guard denies.
    let live = run_with_config(None, "guard", deny_payload);
    assert_eq!(
        live["hookSpecificOutput"]["permissionDecision"], "deny",
        "{live}"
    );

    // The shipped id switches it off.
    let off = run_with_config(
        Some(r#"{"disabled":["hook:omm-guard"]}"#),
        "guard",
        deny_payload,
    );
    assert_eq!(off, serde_json::json!({}), "{off}");

    // The bare handler name is accepted too.
    let off2 = run_with_config(
        Some(r#"{"disabled":["hook:guard"]}"#),
        "guard",
        deny_payload,
    );
    assert_eq!(off2, serde_json::json!({}), "{off2}");

    // Disabling one hook leaves the others live.
    let stop = run_with_config(
        Some(r#"{"disabled":["hook:omm-guard"]}"#),
        "stop",
        stop_payload,
    );
    assert_eq!(stop["decision"], "block", "stop must stay live: {stop}");

    // A skill id or a malformed config disables nothing.
    let other = run_with_config(
        Some(r#"{"disabled":["skill:omm-guard"]}"#),
        "guard",
        deny_payload,
    );
    assert_eq!(
        other["hookSpecificOutput"]["permissionDecision"], "deny",
        "{other}"
    );
    let bad = run_with_config(
        Some(r#"{"disabled": "hook:omm-guard""#),
        "guard",
        deny_payload,
    );
    assert_eq!(
        bad["hookSpecificOutput"]["permissionDecision"], "deny",
        "{bad}"
    );
}
