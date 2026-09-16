//! The absorbed upstream gates at the binary boundary (`content/hooks/README.md`
//! → omm-skill-gate / omm-intent-gate / omm-stop / omm-subagent-start|stop):
//! every case below runs the real `omm hook <name>` against a scratch
//! workspace, with the environment cleared so no ambient `HOME` or `OMM_DIR`
//! can leak in.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{json, Value};

struct Ctx {
    home: tempfile::TempDir,
    ws: tempfile::TempDir,
}

impl Ctx {
    fn new() -> Ctx {
        Ctx {
            home: tempfile::Builder::new()
                .prefix("omm-gates-home-")
                .tempdir()
                .expect("home tempdir"),
            ws: tempfile::Builder::new()
                .prefix("omm-gates-ws-")
                .tempdir()
                .expect("workspace tempdir"),
        }
    }

    fn run(&self, name: &str, payload: &Value) -> (Value, String) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_omm"))
            .args(["hook", name])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.home.path())
            .current_dir(self.ws.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn omm hook");
        let body = serde_json::to_vec(payload).expect("payload JSON");
        child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(&body)
            .expect("write payload");
        let out = child.wait_with_output().expect("wait for omm hook");
        assert_eq!(
            out.status.code(),
            Some(0),
            "omm hook {name}: rc={:?} stderr={:?}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr),
        );
        assert!(
            out.stderr.is_empty(),
            "omm hook {name}: stderr must stay empty, got {:?}",
            String::from_utf8_lossy(&out.stderr),
        );
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let decision: Value =
            serde_json::from_str(stdout.trim()).expect("stdout is one JSON object");
        (decision, stdout)
    }

    fn omm_dir(&self) -> PathBuf {
        self.ws.path().join(".omm")
    }

    fn write_state(&self, rel: &str, body: &str) {
        let path = self.omm_dir().join(rel);
        std::fs::create_dir_all(path.parent().expect("state parent")).expect("create state parent");
        std::fs::write(&path, body).expect("write state file");
    }

    fn pre_tool_use(&self, tool: &str, input: Value) -> Value {
        json!({
            "hook_event_name": "PreToolUse",
            "tool_name": tool,
            "tool_input": input,
            "cwd": self.ws.path().to_str().expect("ws is UTF-8"),
        })
    }
}

fn deny_reason(decision: &Value) -> Option<String> {
    decision
        .get("hookSpecificOutput")?
        .get("permissionDecisionReason")?
        .as_str()
        .map(str::to_string)
}

fn is_deny(decision: &Value) -> bool {
    decision
        .get("hookSpecificOutput")
        .and_then(|o| o.get("permissionDecision"))
        .and_then(Value::as_str)
        == Some("deny")
}

#[test]
fn gates_allow_without_workspace_state() {
    // No `.omm/` at all: both gates are inert, whatever the tool.
    let ctx = Ctx::new();
    for gate in ["skill-gate", "intent-gate"] {
        for payload in [
            ctx.pre_tool_use("Write", json!({"path": "x"})),
            ctx.pre_tool_use("bash", json!({"command": "rm -rf /tmp/x"})),
            ctx.pre_tool_use("bash", json!({"command": "cargo test"})),
        ] {
            let (decision, _) = ctx.run(gate, &payload);
            assert_eq!(
                decision,
                json!({}),
                "{gate} without state must allow: {payload}"
            );
        }
    }
}

#[test]
fn gates_never_guess_the_workspace() {
    // A gate event with no usable `cwd` answers `{}` — even though the hook
    // process itself runs with the workspace as its current directory.
    let ctx = Ctx::new();
    ctx.write_state("skill-gate.json", r#"{"enabled": true, "required": ["a"]}"#);
    ctx.write_state("intent-gate.json", r#"{"required": ["plan"]}"#);
    for gate in ["skill-gate", "intent-gate"] {
        let payload = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Write",
            "tool_input": {"path": "x"},
        });
        let (decision, _) = ctx.run(gate, &payload);
        assert_eq!(decision, json!({}), "{gate} without cwd must allow");
        let payload = json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Write",
            "tool_input": {"path": "x"},
            "cwd": "relative/path",
        });
        let (decision, _) = ctx.run(gate, &payload);
        assert_eq!(decision, json!({}), "{gate} with a relative cwd must allow");
    }
}

#[test]
fn skill_gate_blocks_until_skills_are_read() {
    let ctx = Ctx::new();
    ctx.write_state(
        "skill-gate.json",
        r#"{"enabled": true, "required": ["planner", "executor"]}"#,
    );
    let write = ctx.pre_tool_use("Write", json!({"path": "x"}));
    let (decision, _) = ctx.run("skill-gate", &write);
    assert!(
        is_deny(&decision),
        "missing reads must deny, got {decision}"
    );
    let reason = deny_reason(&decision).expect("deny carries a reason");
    assert!(
        reason.contains("planner") && reason.contains("executor"),
        "{reason}"
    );

    ctx.write_state("read-skills.json", r#"["planner"]"#);
    let (decision, _) = ctx.run("skill-gate", &write);
    assert!(
        is_deny(&decision),
        "one missing read still denies, got {decision}"
    );
    assert!(
        deny_reason(&decision).expect("reason").contains("executor"),
        "the reason names what is still missing"
    );

    ctx.write_state("read-skills.json", r#"{"skills": ["planner", "executor"]}"#);
    let (decision, _) = ctx.run("skill-gate", &write);
    assert_eq!(decision, json!({}), "all reads marked allows");

    // A non-mutating tool passes even with reads outstanding.
    ctx.write_state("read-skills.json", r#"[]"#);
    let probe = ctx.pre_tool_use("bash", json!({"command": "cargo test"}));
    let (decision, _) = ctx.run("skill-gate", &probe);
    assert_eq!(decision, json!({}), "reads-only tools always pass");

    // A corrupt gate file disables the gate instead of denying everything.
    ctx.write_state("skill-gate.json", "{not json");
    let (decision, _) = ctx.run("skill-gate", &write);
    assert_eq!(decision, json!({}), "malformed gate state must fail open");
}

#[test]
fn skill_gate_watches_bash_spellings() {
    let ctx = Ctx::new();
    ctx.write_state("skill-gate.json", r#"{"enabled": true, "required": ["a"]}"#);
    for command in [
        "rm -rf /tmp/x",
        "echo rebuilt > out.txt",
        "sed -i 's/a/b/' file",
    ] {
        let payload = ctx.pre_tool_use("bash", json!({"command": command}));
        let (decision, _) = ctx.run("skill-gate", &payload);
        assert!(is_deny(&decision), "{command:?} must deny, got {decision}");
    }
    let payload = ctx.pre_tool_use("bash", json!({"command": "cargo test"}));
    let (decision, _) = ctx.run("skill-gate", &payload);
    assert_eq!(decision, json!({}), "a read-only command allows");
}

#[test]
fn guard_jails_writes_to_the_workspace() {
    let ctx = Ctx::new();
    for command in [
        "echo hi > /etc/motd",
        "echo hi | sudo tee /etc/motd",
        "cp new /etc/cron.d/x",
    ] {
        let payload = ctx.pre_tool_use("bash", json!({ "command": command }));
        let (decision, _) = ctx.run("guard", &payload);
        assert!(is_deny(&decision), "{command:?} must deny, got {decision}");
        assert!(
            deny_reason(&decision)
                .expect("reason")
                .contains("write-outside-jail"),
            "{command:?} names the rule, got {decision}"
        );
    }
    for command in ["echo hi > out.txt", "cargo test", "cmd 2>&1"] {
        let payload = ctx.pre_tool_use("bash", json!({ "command": command }));
        let (decision, _) = ctx.run("guard", &payload);
        assert_eq!(decision, json!({}), "{command:?} allows");
    }
}

#[test]
fn intent_gate_blocks_until_the_plan_exists() {
    let ctx = Ctx::new();
    // Absent gate file: fail open.
    let write = ctx.pre_tool_use("Write", json!({"path": "x"}));
    let (decision, _) = ctx.run("intent-gate", &write);
    assert_eq!(decision, json!({}), "absent intent-gate allows");

    ctx.write_state("intent-gate.json", r#"{"required": ["plan"]}"#);
    let (decision, _) = ctx.run("intent-gate", &write);
    assert!(is_deny(&decision), "a missing plan denies, got {decision}");
    assert!(
        deny_reason(&decision)
            .expect("reason")
            .contains(".omm/plan.json"),
        "the reason names the missing file"
    );

    ctx.write_state("plan.json", r#"{"steps": []}"#);
    let (decision, _) = ctx.run("intent-gate", &write);
    assert_eq!(decision, json!({}), "an existing plan allows");
}

#[test]
fn stop_blocks_a_pending_verify() {
    let ctx = Ctx::new();
    let stop = || {
        json!({
            "hook_event_name": "Stop",
            "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
            "last_assistant_message": "nothing to report yet",
        })
    };
    let (decision, _) = ctx.run("stop", &stop());
    assert_eq!(decision, json!({}), "no verify state: no claim, no block");

    ctx.write_state(
        "verify.json",
        r#"{"claim": "all green", "status": "pending", "ok": false, "checks": []}"#,
    );
    let (decision, _) = ctx.run("stop", &stop());
    assert_eq!(
        decision.get("decision").and_then(Value::as_str),
        Some("block"),
        "a pending claim blocks, got {decision}"
    );
    assert!(
        decision
            .get("reason")
            .and_then(Value::as_str)
            .expect("block carries a reason")
            .contains("all green"),
        "the block names the pending claim: {decision}"
    );

    ctx.write_state(
        "verify.json",
        r#"{"claim": "all green", "status": "pass", "ok": true, "checks": []}"#,
    );
    let (decision, _) = ctx.run("stop", &stop());
    assert_eq!(decision, json!({}), "a recorded pass leaves the stop alone");
}

#[test]
fn subagent_hooks_log_and_always_allow() {
    let ctx = Ctx::new();
    // No `.omm/`: allow, and create nothing in the workspace.
    for (handler, event) in [
        ("subagent-start", "SubagentStart"),
        ("subagent-stop", "SubagentStop"),
    ] {
        let payload = json!({
            "hook_event_name": event,
            "subagent_id": "verify-reminder",
            "session_id": "s1",
            "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
        });
        let (decision, _) = ctx.run(handler, &payload);
        assert_eq!(decision, json!({}), "{handler} always allows");
    }
    assert!(
        !ctx.omm_dir().exists(),
        "logging hooks must not create `.omm/`"
    );

    std::fs::create_dir_all(ctx.omm_dir()).expect("opt the workspace in");
    let payload = json!({
        "hook_event_name": "SubagentStart",
        "subagent_id": "verify-reminder",
        "child_session_id": "c1",
        "session_id": "s1",
        "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
    });
    let (decision, _) = ctx.run("subagent-start", &payload);
    assert_eq!(decision, json!({}), "logging still allows");
    let log = ctx.omm_dir().join("team").join("log.jsonl");
    let line = std::fs::read_to_string(&log).expect("team log line");
    let rec: Value = serde_json::from_str(line.trim()).expect("log line is JSON");
    assert_eq!(rec.get("kind").and_then(Value::as_str), Some("start"));
    assert_eq!(
        rec.get("subagent_id").and_then(Value::as_str),
        Some("verify-reminder")
    );
    assert_eq!(
        rec.get("child_session_id").and_then(Value::as_str),
        Some("c1")
    );
    assert_eq!(rec.get("session_id").and_then(Value::as_str), Some("s1"));
    assert_eq!(rec.get("outcome"), None, "a start carries no outcome");
    assert!(
        rec.get("ts")
            .and_then(Value::as_str)
            .expect("log line carries ts")
            .ends_with('Z'),
        "{rec}"
    );

    // A stop records its terminal word — including a cancellation.
    for (status, want) in [("completed", "completed"), ("cancelled", "cancelled")] {
        let payload = json!({
            "hook_event_name": "SubagentStop",
            "subagent_id": "verify-reminder",
            "child_session_id": "c1",
            "session_id": "s1",
            "status": status,
            "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
        });
        let (decision, _) = ctx.run("subagent-stop", &payload);
        assert_eq!(decision, json!({}), "a stop always allows");
        let line = std::fs::read_to_string(&log)
            .expect("log")
            .lines()
            .last()
            .expect("stop line")
            .to_string();
        let rec: Value = serde_json::from_str(&line).expect("log line is JSON");
        assert_eq!(rec.get("kind").and_then(Value::as_str), Some("stop"));
        assert_eq!(
            rec.get("outcome").and_then(Value::as_str),
            Some(want),
            "{rec}"
        );
    }

    // A wrong event is not logged and still allows.
    let payload = json!({
        "hook_event_name": "Stop",
        "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
    });
    let (decision, _) = ctx.run("subagent-stop", &payload);
    assert_eq!(decision, json!({}));
    assert_eq!(
        std::fs::read_to_string(&log).expect("log").lines().count(),
        3,
        "the wrong event appends nothing"
    );
}

#[test]
fn pre_compact_logs_and_always_allows() {
    let ctx = Ctx::new();
    // No `.omm/`: allow, and create nothing in the workspace.
    let payload = json!({
        "hook_event_name": "PreCompact",
        "trigger": "soft",
        "session_id": "s1",
        "turn_id": "t1",
        "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
    });
    let (decision, _) = ctx.run("pre-compact", &payload);
    assert_eq!(decision, json!({}), "pre-compact always allows");
    assert!(
        !ctx.omm_dir().exists(),
        "logging hooks must not create `.omm/`"
    );

    std::fs::create_dir_all(ctx.omm_dir()).expect("opt the workspace in");
    let (decision, _) = ctx.run("pre-compact", &payload);
    assert_eq!(decision, json!({}), "logging still allows");
    let log = ctx.omm_dir().join("compact.log.jsonl");
    let line = std::fs::read_to_string(&log).expect("compact log line");
    let rec: Value = serde_json::from_str(line.trim()).expect("log line is JSON");
    assert_eq!(rec.get("kind").and_then(Value::as_str), Some("pre-compact"));
    assert_eq!(rec.get("trigger").and_then(Value::as_str), Some("soft"));
    assert_eq!(rec.get("session_id").and_then(Value::as_str), Some("s1"));
    assert_eq!(rec.get("turn_id").and_then(Value::as_str), Some("t1"));
    assert!(
        rec.get("ts")
            .and_then(Value::as_str)
            .expect("log line carries ts")
            .ends_with('Z'),
        "{rec}"
    );

    // A wrong event is not logged and still allows.
    let payload = json!({
        "hook_event_name": "Stop",
        "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
    });
    let (decision, _) = ctx.run("pre-compact", &payload);
    assert_eq!(decision, json!({}));
    assert_eq!(
        std::fs::read_to_string(&log).expect("log").lines().count(),
        1,
        "the wrong event appends nothing"
    );
}

#[test]
fn gates_ignore_other_events() {
    let ctx = Ctx::new();
    ctx.write_state("skill-gate.json", r#"{"enabled": true, "required": ["a"]}"#);
    let payload = json!({
        "hook_event_name": "Stop",
        "cwd": ctx.ws.path().to_str().expect("ws is UTF-8"),
    });
    for gate in ["skill-gate", "intent-gate"] {
        let (decision, _) = ctx.run(gate, &payload);
        assert_eq!(decision, json!({}), "{gate} on Stop allows");
    }
}
