//! The absorbed upstream gates (`hooks/skill_gate.py`, the `verify.json`
//! half of `bin/lib/engines.mjs → cmdVerify`, `hooks/subagent_start.py` and
//! `hooks/subagent_stop.py` on hypery11/oh-my-musecode): a skill gate and an
//! intent gate on `PreToolUse`, a pending-verify block on `Stop`, and a team
//! log append on `SubagentStart`/`SubagentStop`.
//!
//! All four read workspace state from `<cwd>/.omm/` — the same directory the
//! upstream hooks use, so a workspace configured for the Python hooks keeps
//! working: the file shapes (`skill-gate.json`, `read-skills.json`,
//! `intent-gate.json`, `plan.json`, `verify.json`, `team/log.jsonl`) and the
//! deny reasons are unchanged. What changed is the hardening around them:
//!
//! * the workspace comes from the event's `cwd` (else `workspace_root`,
//!   else `working_directory`), and only when it is absolute. There is no
//!   environment or process-cwd fallback: a hook that guesses the workspace
//!   reads and writes another project's state. No usable `cwd` → `{}`;
//! * state files are capped at [`STATE_FILE_MAX_BYTES`]; oversize or
//!   malformed state disables that gate (fail open), it never denies. A
//!   corrupt `skill-gate.json` denying every mutating tool would brick the
//!   session worse than an open gate;
//! * the hooks never create `<cwd>/.omm/` itself. The team log appends under
//!   an existing `.omm/` only — a hook run must not surprise the workspace
//!   with a new directory;
//! * everything is spawn-free and in-memory (R16), like the other handlers.
use serde_json::{json, Value};
use std::io::Read;
use std::path::{Path, PathBuf};

use super::hook::{deny, EVENT_PRE_COMPACT, EVENT_PRE_TOOL_USE, EVENT_STOP};

/// The handler names — the third argv word of `content/hooks/*.json`.
pub const HANDLER_SKILL_GATE: &str = "skill-gate";
pub const HANDLER_INTENT_GATE: &str = "intent-gate";
pub const HANDLER_SUBAGENT_START: &str = "subagent-start";
pub const HANDLER_SUBAGENT_STOP: &str = "subagent-stop";

/// The events the subagent handlers answer
/// (`docs/host-data/hook-events.json` `items[].name`).
pub const EVENT_SUBAGENT_START: &str = "SubagentStart";
pub const EVENT_SUBAGENT_STOP: &str = "SubagentStop";

/// The workspace state directory name (`<cwd>/.omm`, upstream convention).
pub const STATE_DIR_NAME: &str = ".omm";
/// The most any one state file may hold: the gate files are a few hundred
/// bytes; anything past this is not a gate file.
pub const STATE_FILE_MAX_BYTES: u64 = 64 << 10;
/// The longest claim echo in a pending-verify block reason.
pub const CLAIM_MAX_CHARS: usize = 120;

/// A loaded `skill-gate.json` value → is the gate on?
fn gate_enabled(gate: &Option<Value>) -> bool {
    gate.as_ref()
        .and_then(|v| v.get("enabled")?.as_bool())
        .unwrap_or(false)
}

/// `omm hook skill-gate`: deny a mutating tool until every skill the gate
/// requires is marked read. Anything else — wrong event, gate off or
/// absent, non-mutating tool, requirements met — is `{}`.
pub fn skill_gate(event: &Value) -> Value {
    if event_name(event) != Some(EVENT_PRE_TOOL_USE) {
        return json!({});
    }
    let (name, command) = tool_call(event);
    if !looks_mutating(&name, &command) {
        return json!({});
    }
    let Some(dir) = state_dir(event) else {
        return json!({});
    };
    let gate = read_state_file(&dir, "skill-gate.json");
    if !gate_enabled(&gate) {
        return json!({});
    }
    let needed = required_skills(&gate);
    if needed.is_empty() {
        return json!({});
    }
    let already = read_set(&read_state_file(&dir, "read-skills.json"));
    let missing: Vec<&str> = needed
        .iter()
        .map(String::as_str)
        .filter(|s| !already.contains(*s))
        .collect();
    if missing.is_empty() {
        return json!({});
    }
    deny(&format!(
        "Oh My Muse Code skill gate: mutating tool {} blocked until these skills are marked read in .omm/read-skills.json: {}",
        display_tool(&name),
        missing.join(", "),
    ))
}

/// `omm hook intent-gate`: when `.omm/intent-gate.json` requires a plan,
/// deny a mutating tool until `.omm/plan.json` exists. Same `{}` contract
/// as the skill gate otherwise.
pub fn intent_gate(event: &Value) -> Value {
    if event_name(event) != Some(EVENT_PRE_TOOL_USE) {
        return json!({});
    }
    let (name, command) = tool_call(event);
    if !looks_mutating(&name, &command) {
        return json!({});
    }
    let Some(dir) = state_dir(event) else {
        return json!({});
    };
    if !plan_required(&read_state_file(&dir, "intent-gate.json")) {
        return json!({});
    }
    if dir.join("plan.json").is_file() {
        return json!({});
    }
    deny(&format!(
        "Oh My Muse Code intent-gate: mutating tool {} blocked until .omm/plan.json exists",
        display_tool(&name),
    ))
}

/// The Stop-side verify gate: when `.omm/verify.json` holds a `pending`
/// claim, stopping now would end the session with the check still open, so
/// block and name the claim. A recorded `pass`/`fail`, or no file at all,
/// leaves the decision to the caller. `None` = no block.
pub fn pending_verify_block(event: &Value) -> Option<Value> {
    if event_name(event) != Some(EVENT_STOP) {
        return None;
    }
    let dir = state_dir(event)?;
    let claim = pending_claim(&read_state_file(&dir, "verify.json")?)?;
    Some(json!({
        "decision": "block",
        "reason": format!(
            "omm: verify is still pending ({}) — record the result before stopping.",
            truncate_chars(&claim, CLAIM_MAX_CHARS),
        ),
    }))
}

/// `omm hook pre-compact`: append one line per compaction to
/// `<cwd>/.omm/compact.log.jsonl` under an already-opted-in `.omm/`, always
/// answer `{}`. Context injection is unsupported on PreCompact, and vetoing
/// only skips that compaction while pressure keeps building, so the handler
/// never vetoes and only observes. A hook that cannot log still allows;
/// logging never blocks.
pub fn pre_compact_log(event: &Value) -> Value {
    if event_name(event) != Some(EVENT_PRE_COMPACT) {
        return json!({});
    }
    if let Some(dir) = state_dir(event) {
        if dir.is_dir() {
            let rec = json!({
                "ts": utc_now(),
                "kind": "pre-compact",
                "hook": "pre-compact",
                "trigger": str_field(event, &["trigger"]),
                "session_id": str_field(event, &["session_id", "sessionId"]),
                "turn_id": str_field(event, &["turn_id", "turnId"]),
            });
            append_jsonl(&dir.join("compact.log.jsonl"), &rec);
        }
    }
    json!({})
}

/// `omm hook subagent-start` / `subagent-stop`: append one line to
/// `<cwd>/.omm/team/log.jsonl` under an already-opted-in `.omm/`, always
/// answer `{}`. A hook that cannot log still allows; logging never blocks.
///
/// The 1.3.0 payload names the child `subagent_id` / `child_session_id`
/// (there is no `agent_id` / `agent_type` on the wire); a stop additionally
/// records `outcome`, the first non-empty terminal word the payload carries
/// (`status`, `outcome`, `terminal`, …) — including cancellations when the
/// host reports them.
pub fn subagent_log(hook: &'static str, kind: &str, event_name_want: &str, event: &Value) -> Value {
    if event_name(event) != Some(event_name_want) {
        return json!({});
    }
    if let Some(dir) = state_dir(event) {
        if dir.is_dir() {
            let mut rec = json!({
                "ts": utc_now(),
                "kind": kind,
                "hook": hook,
                "subagent_id": str_field(event, &["subagent_id", "subagentId"]),
                "child_session_id": str_field(event, &["child_session_id", "childSessionId"]),
                "session_id": str_field(event, &["session_id", "sessionId"]),
            });
            if kind == "stop" {
                rec["outcome"] = Value::String(first_present(
                    event,
                    &[
                        "status",
                        "outcome",
                        "terminal",
                        "result",
                        "stop_reason",
                        "reason",
                    ],
                ));
            }
            append_jsonl(&dir.join("team").join("log.jsonl"), &rec);
        }
    }
    json!({})
}

/// The first non-empty string under any of `keys`, else `""`.
fn first_present(event: &Value, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|k| event.get(k))
        .filter_map(Value::as_str)
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_string()
}

// ---- event plumbing ---------------------------------------------------------

fn event_name(event: &Value) -> Option<&str> {
    event.get("hook_event_name").and_then(Value::as_str)
}

/// The tool call: `(name, command)`, accepting the camelCase aliases the
/// upstream hooks accept. `command` is a string or, as upstream allows, a
/// list joined with spaces.
fn tool_call(event: &Value) -> (String, String) {
    let name = str_field(event, &["tool_name", "toolName"]);
    let input = event
        .get("tool_input")
        .or_else(|| event.get("toolInput"))
        .cloned()
        .unwrap_or(Value::Null);
    let command = input
        .get("command")
        .or_else(|| input.get("cmd"))
        .map(|c| match c {
            Value::String(s) => s.clone(),
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        })
        .unwrap_or_default();
    (name, command)
}

fn str_field(event: &Value, keys: &[&str]) -> String {
    keys.iter()
        .filter_map(|k| event.get(*k))
        .filter_map(Value::as_str)
        .next()
        .unwrap_or("")
        .to_string()
}

fn display_tool(name: &str) -> String {
    if name.is_empty() {
        "(unknown)".to_string()
    } else {
        name.to_string()
    }
}

// ---- mutating-tool heuristic ------------------------------------------------
//
// A port of `skill_gate.py → looks_mutating`/`WRITEISH`: the write/edit
// tool families by name, and `bash`-family tools whose command line spells
// a write (a writer program word, `sed -i`/`awk -i`, or a `>` redirect
// outside quotes). Like the guard it reads the literal text, so the same
// caveat applies: scripts, aliases and variables pass unseen.

/// Tool names that write by definition (normalised: lowercase, `-`/`_`
/// stripped — `Write`, `str_replace` and `strreplace` are one family).
fn is_write_tool(normalised: &str) -> bool {
    matches!(normalised, "write" | "edit" | "strreplace")
}

/// Tool names whose `command`/`cmd` input is inspected for writes.
fn is_shell_tool(name: &str, normalised: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "bash" | "shell" | "run")
        || matches!(normalised, "bash" | "shell")
}

/// Program words that write (upstream `WRITEISH`, minus the redirect half
/// which is handled separately so quoting is honoured).
const WRITE_PROGRAMS: [&str; 12] = [
    "tee", "rm", "mv", "cp", "mkdir", "touch", "chmod", "chown", "dd", "install", "sed", "awk",
];

/// Is this tool call a mutation? Pure.
pub fn looks_mutating(name: &str, command: &str) -> bool {
    let normalised: String = name
        .chars()
        .filter(|c| *c != '-' && *c != '_')
        .collect::<String>()
        .to_ascii_lowercase();
    if is_write_tool(&normalised) {
        return true;
    }
    if !is_shell_tool(name, &normalised) || command.is_empty() {
        return false;
    }
    command_writes(command)
}

/// Does a shell command line spell a write? Pure: a [`WRITE_PROGRAMS`]
/// word (`sed`/`awk` only with `-i`), or a `>` redirect outside quotes.
fn command_writes(command: &str) -> bool {
    if has_redirect_outside_quotes(command) {
        return true;
    }
    let lower = command.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_' && c != '/')
        .filter(|w| !w.is_empty())
        .collect();
    let mut i = 0;
    while i < words.len() {
        let program = words[i].rsplit('/').next().unwrap_or(words[i]);
        if WRITE_PROGRAMS.contains(&program) {
            if program == "sed" || program == "awk" {
                if words[i + 1..].iter().any(|w| {
                    *w == "-i"
                        || w.starts_with("-i") && w[1..].chars().all(|c| c.is_ascii_alphanumeric())
                }) {
                    return true;
                }
            } else {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// A `>` anywhere outside single/double quotes (a backslash escapes the
/// next character). `>>`, `> file`, `2>` all count — any stdout redirect
/// writes.
fn has_redirect_outside_quotes(command: &str) -> bool {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in command.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some(q) => {
                if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => match c {
                '\'' | '"' => quote = Some(c),
                '\\' => escaped = true,
                '>' => return true,
                _ => {}
            },
        }
    }
    false
}

// ---- gate file shapes -------------------------------------------------------
//
// Shapes follow `skill_gate.py`: `skill-gate.json` enables with
// `required` (also `requiredSkills`, `skills`); `read-skills.json` is a
// list or a `skills`/`read`/`ids` dict; `intent-gate.json` requires a plan
// with `required` containing `plan`/`plan.json`; `verify.json` blocks a
// stop while `status` is `pending`.

/// The skills the gate requires. Pure.
pub fn required_skills(gate: &Option<Value>) -> Vec<String> {
    let Some(gate) = gate.as_ref() else {
        return Vec::new();
    };
    let raw = gate
        .get("required")
        .or_else(|| gate.get("requiredSkills"))
        .or_else(|| gate.get("skills"));
    match raw {
        Some(Value::String(s)) => {
            if s.is_empty() {
                Vec::new()
            } else {
                vec![s.clone()]
            }
        }
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// The skills already marked read. Pure.
pub fn read_set(payload: &Option<Value>) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    match payload {
        None => {}
        Some(Value::Array(items)) => {
            out.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
        }
        Some(Value::Object(map)) => {
            let mut listed = false;
            for key in ["skills", "read", "ids"] {
                if let Some(Value::Array(items)) = map.get(key) {
                    out.extend(items.iter().filter_map(Value::as_str).map(str::to_string));
                    listed = true;
                }
            }
            if !listed {
                out.extend(
                    map.iter()
                        .filter(|(_, v)| v.as_bool().unwrap_or(false))
                        .map(|(k, _)| k.clone()),
                );
            }
        }
        _ => {}
    }
    out
}

/// Does the intent gate require a plan file? Pure.
pub fn plan_required(intent: &Option<Value>) -> bool {
    required_skills(intent)
        .iter()
        .any(|s| s == "plan" || s == "plan.json")
}

/// The pending claim in a `verify.json`, if it blocks a stop. Pure.
pub fn pending_claim(verify: &Value) -> Option<String> {
    if verify.get("status")?.as_str()? != "pending" {
        return None;
    }
    let claim = verify
        .get("claim")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if claim.is_empty() {
        return None;
    }
    Some(claim)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

// ---- workspace state --------------------------------------------------------

/// `<cwd>/.omm` from the event: `cwd`, else `workspace_root`, else
/// `working_directory` — absolute only. `None` when the event names no
/// workspace: the hook answers `{}` rather than guess one.
pub fn state_dir(event: &Value) -> Option<PathBuf> {
    for key in ["cwd", "workspace_root", "working_directory"] {
        if let Some(dir) = event.get(key).and_then(Value::as_str) {
            let dir = dir.trim();
            if dir.is_empty() {
                continue;
            }
            let path = PathBuf::from(dir);
            if path.is_absolute() {
                return Some(path.join(STATE_DIR_NAME));
            }
        }
    }
    None
}

/// Read one JSON state file, capped at [`STATE_FILE_MAX_BYTES`]. `None`
/// when missing, oversize, unreadable or malformed — every one of those
/// disables the gate that wanted it (fail open).
pub fn read_state_file(dir: &Path, file: &str) -> Option<Value> {
    let path = dir.join(file);
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > STATE_FILE_MAX_BYTES {
        return None;
    }
    let fh = std::fs::File::open(&path).ok()?;
    let mut bytes = Vec::new();
    fh.take(STATE_FILE_MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > STATE_FILE_MAX_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn append_jsonl(path: &Path, rec: &Value) {
    let Ok(mut line) = serde_json::to_string(rec) else {
        return;
    };
    line.push('\n');
    use std::io::Write;
    let Ok(parent) = path.parent().ok_or(()) else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(mut fh) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = fh.write_all(line.as_bytes());
    }
}

/// Seconds since the Unix epoch as `YYYY-MM-DDTHH:MM:SSZ` (Howard Hinnant's
/// civil-from-days; the workspace has no date dependency and a hook must
/// not spawn `date`). Falls back to the epoch on a clock before 1970.
pub fn utc_now() -> String {
    utc_from_epoch(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
    )
}

fn utc_from_epoch(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let secs_of_day = secs % 86_400;
    // civil_from_days (Hinnant): days since 1970-01-01 → (y, m, d).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!(
        "{year:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        m,
        d,
        secs_of_day / 3_600,
        secs_of_day % 3_600 / 60,
        secs_of_day % 60,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn write_tools_mutate_by_name() {
        for name in [
            "Write",
            "write",
            "Edit",
            "EDIT",
            "strreplace",
            "str_replace",
            "StrReplace",
        ] {
            assert!(looks_mutating(name, ""), "{name}");
        }
    }

    #[test]
    fn shell_writes_need_a_write_spelling() {
        assert!(looks_mutating("bash", "rm -rf /tmp/x"));
        assert!(looks_mutating("bash", "echo hi > out.txt"));
        assert!(looks_mutating("bash", "echo hi >> out.txt"));
        assert!(looks_mutating("bash", "make 2> err.log"));
        assert!(looks_mutating("bash", "sed -i 's/a/b/' f"));
        assert!(looks_mutating("bash", "sudo tee /etc/x"));
        assert!(looks_mutating("bash", "python3 gen.py > data.json"));
        assert!(!looks_mutating("bash", "cargo test"));
        assert!(!looks_mutating("bash", "sed -n 's/a/b/p' f"));
        assert!(!looks_mutating("bash", "echo 'a > b'"));
        assert!(!looks_mutating("bash", ""));
        assert!(!looks_mutating("read", ""));
        assert!(!looks_mutating("", "rm -rf /"));
    }

    #[test]
    fn required_and_read_shapes() {
        assert_eq!(
            required_skills(&Some(json!({"required": ["a", "b", 1]}))),
            vec!["a".to_string(), "b".to_string()]
        );
        assert_eq!(
            required_skills(&Some(json!({"requiredSkills": "solo"}))),
            vec!["solo".to_string()]
        );
        assert!(required_skills(&None).is_empty());
        let set = read_set(&Some(json!(["a", "b"])));
        assert!(set.contains("a") && set.contains("b"));
        let set = read_set(&Some(json!({"skills": ["a"]})));
        assert!(set.contains("a") && set.len() == 1);
        let set = read_set(&Some(json!({"a": true, "b": false})));
        assert!(set.contains("a") && !set.contains("b"));
        assert!(read_set(&None).is_empty());
        assert!(plan_required(&Some(json!({"required": ["plan"]}))));
        assert!(plan_required(&Some(json!({"required": "plan.json"}))));
        assert!(!plan_required(&Some(json!({"required": ["planner"]}))));
    }

    #[test]
    fn pending_claim_only_blocks_pending() {
        assert_eq!(
            pending_claim(&json!({"claim": "x", "status": "pending", "ok": false})),
            Some("x".to_string())
        );
        assert_eq!(
            pending_claim(&json!({"claim": "x", "status": "pass"})),
            None
        );
        assert_eq!(
            pending_claim(&json!({"claim": "x", "status": "fail"})),
            None
        );
        assert_eq!(pending_claim(&json!({"status": "pending"})), None);
        assert_eq!(pending_claim(&json!({})), None);
    }

    #[test]
    fn state_dir_needs_an_absolute_cwd() {
        let e = json!({"cwd": "/tmp/ws"});
        assert_eq!(state_dir(&e), Some(PathBuf::from("/tmp/ws/.omm")));
        let e = json!({"workspace_root": "/w"});
        assert_eq!(state_dir(&e), Some(PathBuf::from("/w/.omm")));
        assert_eq!(state_dir(&json!({"cwd": "relative"})), None);
        assert_eq!(state_dir(&json!({"cwd": ""})), None);
        assert_eq!(state_dir(&json!({})), None);
    }

    #[test]
    fn utc_shapes_and_known_epochs() {
        assert_eq!(utc_from_epoch(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_from_epoch(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(utc_from_epoch(951_782_400), "2000-02-29T00:00:00Z");
        let now = utc_now();
        assert!(now.len() == 20 && now.ends_with('Z'), "{now}");
    }
}
