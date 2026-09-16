//! `omm hook <name>` — the in-binary hook dispatcher (R16; ARCHITECTURE.md
//! §7; `content/hooks/README.md`; e2e scenario 9).
//!
//! Every shipped hook is `["omm","hook","<name>"]` in the native plugin
//! manifest; the host runs it with the event as JSON on stdin and reads one
//! JSON object from stdout (research/musecode/hooks.md §5.2). Facts this
//! module is built on, all PROVEN there and in `docs/host-data/hook-events.json`:
//!
//! * only exit 2 blocks; 0 completes, 1/3/127 fail open. A deny travels in
//!   the JSON, so this process always exits 0 and never blocks by accident;
//! * `PreToolUse`: `hookSpecificOutput.permissionDecision: "deny"` needs a
//!   non-empty `permissionDecisionReason`; a bare `allow` is rejected
//!   (`updatedInput` required) so the allow path is `{}`; `continue` and a
//!   top-level `additionalContext` fail the hook (§5.3);
//! * `Stop`: `{"decision":"block","reason":…}` requests one continuation,
//!   capped by `settings.max_consecutive_stop_hook_continuations`;
//!   `stop_hook_active` is true on every continuation after the first
//!   (§4.2), `additionalContext` is unsupported (§5.4);
//! * `SessionStart`: `hookSpecificOutput.additionalContext` becomes a
//!   developer-role context block at order 900000+ (§5.7); `decision:block`
//!   is unsupported (§5.4);
//! * stdout ceiling 16,384 B (`hr::HOOK_STDOUT_MAX_BYTES`); the hook's env is
//!   the 16-key scrubbed allow-list (§3.6): `HOME` and `PATH` pass, `XDG_*`
//!   and `OMM_MUSE_BIN` do not, so everything here resolves from `HOME`
//!   (`Roots::from_env_fast`) and nothing spawns.
//!
//! The dispatcher reads at most [`STDIN_LIMIT_BYTES`], decides in memory,
//! prints exactly once. An unknown name, or an event that is not the
//! handler's, is `{}`. A payload that cannot be evaluated — malformed,
//! empty, or cut at the bound — is `{}` for every hook but the guard, which
//! answers a DENY carried in the JSON (Gate 1 decision G; round 4: a bash
//! command padded past the old 1 MiB cut arrived truncated, failed to parse
//! and passed as `{}`): still exit 0, fail-open at the process level (R16).
//!
//! The guard's rule table is `content/hooks/README.md` → omm-guard: a
//! heuristic over the literal command text, never a security boundary.

use serde_json::{json, Value};
use std::io::{IsTerminal, Read};

use omm_host::host_reality as hr;
use omm_host::Roots;
use omm_manifest::catalog::AssetKind;

use super::{thousands, ContentSource, OmmConfig};
use crate::cmd::OMM_VERSION;

/// The most stdin the dispatcher reads: 4 MiB (a hook payload is a few KB;
/// Gate 1 decision G raised the bound from 1 MiB and made the guard deny
/// what is cut at it).
pub const STDIN_LIMIT_BYTES: u64 = 4 << 20;

/// The guard's reason on a payload it cannot evaluate.
pub const UNEVALUABLE_REASON: &str = "omm guard: event too large or malformed to evaluate";

/// The shells whose `-c <body>` the guard evaluates as a command line of
/// its own (README: `sh -c`), and the wrappers whose leading flags are
/// dropped before the program word is read.
pub const SHELLS: [&str; 4] = ["sh", "bash", "zsh", "dash"];
pub const WRAPPERS: [&str; 16] = [
    "sudo",
    "env",
    "command",
    "nohup",
    "time",
    "exec",
    "builtin",
    "doas",
    "xargs",
    // Gate 1 round 5 (LOW): scheduling / namespace / buffering wrappers that
    // run the rest of the line as a command (`timeout 5 rm -rf /`, `nice -n 10
    // rm -rf /`, `setsid rm -rf /`, `stdbuf -o0 rm -rf /`, `chroot / rm -rf /`).
    "timeout",
    "nice",
    "ionice",
    "setsid",
    "caffeinate",
    "stdbuf",
    "chroot",
];

/// Leading shell keywords and group openers dropped before the program word
/// (`{ rm -rf /; }`, `( rm -rf / )`, `if true; then rm -rf /; fi`,
/// `for x in 1; do rm -rf /; done`, `! rm -rf /`; Gate 1 round 5 LOW). `time`
/// is handled as a wrapper.
pub const SHELL_KEYWORDS: [&str; 4] = ["then", "do", "else", "!"];

/// The handler names — the third argv word of `content/hooks/*.json`.
pub const HANDLER_GUARD: &str = "guard";
pub const HANDLER_SESSION_START: &str = "session-start";
pub const HANDLER_STOP: &str = "stop";
pub const HANDLER_PRE_COMPACT: &str = "pre-compact";
/// The skill router (PLAN.md 3.1): not a plugin hook but the project-tier
/// `<ws>/.muse/hooks.json` handler `omm enable skill-routing` writes, on
/// `UserPromptSubmit` with `"outputCapabilities":["skills.v1"]`; its
/// decision is `super::routing::route`.
pub const HANDLER_ROUTE: &str = "route";

/// The events each handler answers (hook-events.json `items[].name`).
pub const EVENT_PRE_TOOL_USE: &str = "PreToolUse";
pub const EVENT_SESSION_START: &str = "SessionStart";
pub const EVENT_STOP: &str = "Stop";
pub const EVENT_PRE_COMPACT: &str = "PreCompact";

/// The tools whose `tool_input.command` the guard inspects.
pub const BASH_TOOLS: [&str; 2] = ["bash", "bash_input"];

/// The one-line verify nudge (`content/hooks/README.md` → omm-stop).
pub const STOP_NUDGE: &str = "omm: before calling this done, run the check that proves it (the tests, build or lint you touched) and report its exit status.";

/// Words that make a message a completion claim (README → omm-stop).
pub const CLAIM_WORDS: [&str; 7] = [
    "done",
    "fixed",
    "complete",
    "completed",
    "passing",
    "implemented",
    "ready",
];

/// The longest command echo in a deny reason.
pub const REASON_COMMAND_MAX: usize = 160;

/// What arrived on stdin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// stdin is a terminal: nobody sent an event (a human ran it by hand).
    Absent,
    /// One JSON document.
    Parsed(Value),
    /// Bytes that are not one JSON document (an empty pipe included).
    Malformed,
    /// More than [`STDIN_LIMIT_BYTES`] arrived: the rest was not read, so
    /// no decision can be made on what was.
    Truncated,
}

/// The event on stdin. Bounded so a runaway writer cannot hold the hook
/// past its `timeoutMs`; one byte past the bound marks it truncated.
pub fn read_event() -> Event {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return Event::Absent;
    }
    let mut bytes = Vec::new();
    if stdin
        .lock()
        .take(STDIN_LIMIT_BYTES + 1)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return Event::Malformed;
    }
    parse_event(&bytes)
}

/// [`read_event`]'s decision on the bytes read, pure.
pub fn parse_event(bytes: &[u8]) -> Event {
    if bytes.len() as u64 > STDIN_LIMIT_BYTES {
        return Event::Truncated;
    }
    match serde_json::from_slice(bytes) {
        Ok(v) => Event::Parsed(v),
        Err(_) => Event::Malformed,
    }
}

/// Why [`dispatch`] answered the way it did. Pure observability: the wire
/// behaviour (stdout JSON, exit 0, empty stderr — R16) is unchanged; this
/// names the decision so a silent `{}` can be told apart after the fact
/// (unknown handler vs absent/malformed/truncated event vs a handler that
/// evaluated and allowed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HookObservation {
    /// The kill switch matched (`config.json → disabled`); the handler never ran.
    Disabled {
        /// The shipped hook id that matched (`omm-guard`).
        hook_id: String,
    },
    /// The guard denied: `rule` is the [`Verdict::rule`] that matched, or
    /// `"unevaluable"` for a payload cut at the bound / malformed / empty.
    GuardDeny { rule: &'static str },
    /// The guard evaluated a `PreToolUse` bash event and allowed it.
    GuardAllow,
    /// The guard saw a parsed event that is not `PreToolUse`.
    GuardWrongEvent,
    /// A parsed event was routed to its handler (`stop`, `session-start`,
    /// `route`, `subagent-start`, `subagent-stop`); whether that handler
    /// answered `{}` is its own documented contract.
    Handled { handler: &'static str },
    /// A skill/intent gate denied a mutating tool: `gate` is the handler
    /// name (`skill-gate`, `intent-gate`).
    GateDeny { gate: &'static str },
    /// A skill/intent gate evaluated a `PreToolUse` event and allowed it.
    GateAllow { gate: &'static str },
    /// Answered `{}` without evaluating: `reason` is `"unknown-handler"`,
    /// `"absent-event"` (a human ran it by hand), `"malformed-event"` or
    /// `"truncated-event"`.
    Noop { reason: &'static str },
}

/// The observation behind a skill/intent gate decision: a deny carries
/// `hookSpecificOutput.permissionDecision`, anything else allowed.
fn gate_observation(gate: &'static str, decision: &Value) -> (Value, HookObservation) {
    let denied = decision
        .get("hookSpecificOutput")
        .and_then(|o| o.get("permissionDecision"))
        .and_then(Value::as_str)
        == Some("deny");
    let observation = if denied {
        HookObservation::GateDeny { gate }
    } else {
        HookObservation::GateAllow { gate }
    };
    (decision.clone(), observation)
}

/// Route a handler name to its decision. Unknown → `{}`; an unevaluable
/// payload → `{}` for every hook but the guard, which denies.
pub fn dispatch(name: &str, event: &Event, roots: &Roots) -> Value {
    dispatch_observed(name, event, roots).0
}

/// [`dispatch`] plus the [`HookObservation`] behind the decision. The `Value`
/// is byte-identical to what [`dispatch`] returns; only the reason travels
/// with it, for logs and tests — never onto the hook's stdout.
pub fn dispatch_observed(name: &str, event: &Event, roots: &Roots) -> (Value, HookObservation) {
    // The documented kill switch (content/hooks/README.md, ARCHITECTURE §5.4):
    // `config.json → "disabled": ["hook:omm-guard"]` turns a shipped hook
    // into a no-op. It is consulted HERE, on the dispatch path, so it cannot
    // be the inert flag the ecosystem survey found elsewhere (research/ohmy/
    // 00-MATRIX.md, oh-my-codex's `OMX_HOOK_PLUGINS=0`). Same spawn-free,
    // lenient read as `session_start`; a missing or malformed file disables
    // nothing.
    if hook_disabled(name, roots) {
        return (
            json!({}),
            HookObservation::Disabled {
                hook_id: hook_id_for(name),
            },
        );
    }
    // An unknown handler answers `{}` whatever arrived (R16): naming it
    // takes precedence over the event problem, which is moot then.
    if !matches!(
        name,
        HANDLER_GUARD
            | HANDLER_STOP
            | HANDLER_SESSION_START
            | HANDLER_PRE_COMPACT
            | HANDLER_ROUTE
            | super::hook_gates::HANDLER_SKILL_GATE
            | super::hook_gates::HANDLER_INTENT_GATE
            | super::hook_gates::HANDLER_SUBAGENT_START
            | super::hook_gates::HANDLER_SUBAGENT_STOP
    ) {
        return (
            json!({}),
            HookObservation::Noop {
                reason: "unknown-handler",
            },
        );
    }
    match event {
        Event::Parsed(v) => match name {
            HANDLER_GUARD => {
                let decision = guard(v);
                if decision.get("hookSpecificOutput").is_some() {
                    let rule = v
                        .get("tool_input")
                        .and_then(|i| i.get("command"))
                        .and_then(Value::as_str)
                        .and_then(|c| guard_verdict_root(c, event_cwd(v)))
                        .map(|verdict| verdict.rule)
                        .unwrap_or("unevaluable");
                    (decision, HookObservation::GuardDeny { rule })
                } else if event_name(v) != Some(EVENT_PRE_TOOL_USE) {
                    (decision, HookObservation::GuardWrongEvent)
                } else {
                    (decision, HookObservation::GuardAllow)
                }
            }
            HANDLER_STOP => (
                stop(v),
                HookObservation::Handled {
                    handler: HANDLER_STOP,
                },
            ),
            HANDLER_SESSION_START => (
                session_start(v, roots),
                HookObservation::Handled {
                    handler: HANDLER_SESSION_START,
                },
            ),
            HANDLER_PRE_COMPACT => (
                super::hook_gates::pre_compact_log(v),
                HookObservation::Handled {
                    handler: HANDLER_PRE_COMPACT,
                },
            ),
            name if name == super::hook_gates::HANDLER_SKILL_GATE => {
                let decision = super::hook_gates::skill_gate(v);
                gate_observation(super::hook_gates::HANDLER_SKILL_GATE, &decision)
            }
            name if name == super::hook_gates::HANDLER_INTENT_GATE => {
                let decision = super::hook_gates::intent_gate(v);
                gate_observation(super::hook_gates::HANDLER_INTENT_GATE, &decision)
            }
            name if name == super::hook_gates::HANDLER_SUBAGENT_START => (
                super::hook_gates::subagent_log(
                    super::hook_gates::HANDLER_SUBAGENT_START,
                    "start",
                    super::hook_gates::EVENT_SUBAGENT_START,
                    v,
                ),
                HookObservation::Handled {
                    handler: super::hook_gates::HANDLER_SUBAGENT_START,
                },
            ),
            name if name == super::hook_gates::HANDLER_SUBAGENT_STOP => (
                super::hook_gates::subagent_log(
                    super::hook_gates::HANDLER_SUBAGENT_STOP,
                    "stop",
                    super::hook_gates::EVENT_SUBAGENT_STOP,
                    v,
                ),
                HookObservation::Handled {
                    handler: super::hook_gates::HANDLER_SUBAGENT_STOP,
                },
            ),
            // HANDLER_ROUTE is the only remaining known name.
            _ => (
                super::routing::route(v, roots),
                HookObservation::Handled {
                    handler: HANDLER_ROUTE,
                },
            ),
        },
        Event::Absent => (
            json!({}),
            HookObservation::Noop {
                reason: "absent-event",
            },
        ),
        Event::Malformed => {
            if name == HANDLER_GUARD {
                (
                    deny(UNEVALUABLE_REASON),
                    HookObservation::GuardDeny {
                        rule: "unevaluable",
                    },
                )
            } else {
                (
                    json!({}),
                    HookObservation::Noop {
                        reason: "malformed-event",
                    },
                )
            }
        }
        Event::Truncated => {
            if name == HANDLER_GUARD {
                (
                    deny(UNEVALUABLE_REASON),
                    HookObservation::GuardDeny {
                        rule: "unevaluable",
                    },
                )
            } else {
                (
                    json!({}),
                    HookObservation::Noop {
                        reason: "truncated-event",
                    },
                )
            }
        }
    }
}

/// The shipped hook id for a handler name: `guard` → `omm-guard` (the ids in
/// `content/hooks/*.json` are the handler name with the `omm-` prefix).
fn hook_id_for(name: &str) -> String {
    format!("omm-{name}")
}

/// Is this handler switched off in `$OMM/config.json`? Accepts the shipped id
/// (`hook:omm-guard`) and the bare handler name (`hook:guard`).
fn hook_disabled(name: &str, roots: &Roots) -> bool {
    let Ok(bytes) = std::fs::read(OmmConfig::path(&roots.omm_root())) else {
        return false;
    };
    let config = OmmConfig::from_bytes_lenient(&bytes);
    config.is_disabled("hook", &hook_id_for(name)) || config.is_disabled("hook", name)
}

/// The PreToolUse deny shape (README: the reason is required and non-empty).
pub(crate) fn deny(reason: &str) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": EVENT_PRE_TOOL_USE,
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    })
}

fn event_name(event: &Value) -> Option<&str> {
    event.get("hook_event_name").and_then(Value::as_str)
}

// ---- omm-guard (PreToolUse) --------------------------------------------------

/// A guard rule that matched.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// `rm-root` / `find-root-delete` / `force-push-main` / `reset-hard` /
    /// `drop-table` / `write-outside-jail`.
    pub rule: &'static str,
    /// The simple command that matched, whitespace-collapsed.
    pub command: String,
}

impl Verdict {
    /// `omm guard: <rule> - `<command>``.
    pub fn reason(&self) -> String {
        let mut cmd = self.command.clone();
        if cmd.chars().count() > REASON_COMMAND_MAX {
            cmd = cmd.chars().take(REASON_COMMAND_MAX).collect::<String>() + "…";
        }
        format!("omm guard: {} - `{cmd}`", self.rule)
    }
}

/// The PreToolUse decision: `{}` unless the tool is bash and the command
/// matches a rule; then the deny shape with a non-empty reason.
pub fn guard(event: &Value) -> Value {
    if event_name(event) != Some(EVENT_PRE_TOOL_USE) {
        return json!({});
    }
    let tool = event.get("tool_name").and_then(Value::as_str).unwrap_or("");
    if !BASH_TOOLS.contains(&tool) {
        return json!({});
    }
    let Some(command) = event
        .get("tool_input")
        .and_then(|i| i.get("command"))
        .and_then(Value::as_str)
    else {
        return json!({});
    };
    match guard_verdict_root(command, event_cwd(event)) {
        None => json!({}),
        Some(v) => deny(&v.reason()),
    }
}

/// The workspace root a write jail is measured against: the event's
/// absolute `cwd`, normalised; anything else (missing, relative) means the
/// jail cannot be evaluated and only the rootless rules apply.
fn event_cwd(event: &Value) -> Option<String> {
    let cwd = event.get("cwd").and_then(Value::as_str)?;
    let n = normalise_target(cwd);
    if n.starts_with('/') {
        Some(n)
    } else {
        None
    }
}

/// The rule table of `content/hooks/README.md` → omm-guard, applied to each
/// simple command of a shell line (split on `;`, `&&`, `||`, `|`, `&`,
/// newlines outside quotes) after leading wrappers (`sudo`, `env`, `xargs`,
/// … with their flags) and assignments; a `sh -c <body>` is evaluated as a
/// line of its own.
#[cfg(test)]
pub fn guard_verdict(command: &str) -> Option<Verdict> {
    guard_verdict_root(command, None)
}

/// The rule table with a workspace root for the write jail: `None` keeps
/// the historical rootless behaviour (the jail does not apply).
pub fn guard_verdict_root(command: &str, root: Option<String>) -> Option<Verdict> {
    guard_verdict_depth(command, root, 0)
}

/// How deep `sh -c 'sh -c …'` is followed.
const SHELL_NEST_MAX: usize = 4;

fn guard_verdict_depth(command: &str, root: Option<String>, depth: usize) -> Option<Verdict> {
    if drop_statement(command) {
        return Some(Verdict {
            rule: "drop-table",
            command: collapse(command),
        });
    }
    // `cd / && rm -rf .`: the working directory of a later segment is the
    // root when an earlier one changed into it. The jail resolves relative
    // targets against the same tracked directory; an unresolvable `cd`
    // (`cd -`, bare `cd`, `~`) turns the jail off for what follows.
    let mut cwd = root.clone();
    for segment in segments(command) {
        let tokens = tokens(&segment);
        if tokens.is_empty() {
            continue;
        }
        if tokens[0] == "cd" || tokens[0] == "pushd" {
            cwd = cd_target(&tokens[1..], cwd.as_deref());
            continue;
        }
        // `sh -c "<body>"`: the body is a command line of its own.
        if depth < SHELL_NEST_MAX && SHELLS.iter().any(|s| is_program(&tokens[0], s)) {
            if let Some(body) = shell_c_body(&segment) {
                if let Some(v) = guard_verdict_depth(&body, cwd.clone(), depth + 1) {
                    return Some(v);
                }
                continue;
            }
        }
        let cwd_is_root = cwd.as_deref().is_some_and(is_root_target);
        let rule = if is_program(&tokens[0], "rm") && rm_root(&tokens[1..], cwd_is_root) {
            Some("rm-root")
        } else if is_program(&tokens[0], "find") && find_root_delete(&tokens[1..], cwd_is_root) {
            Some("find-root-delete")
        } else if is_program(&tokens[0], "git") {
            if force_push_main(&tokens[1..]) {
                Some("force-push-main")
            } else if reset_hard(&tokens[1..]) {
                Some("reset-hard")
            } else {
                None
            }
        } else {
            match root.as_deref() {
                Some(r) if write_outside_jail(&tokens, cwd.as_deref(), r) => {
                    Some("write-outside-jail")
                }
                _ => None,
            }
        };
        if let Some(rule) = rule {
            return Some(Verdict {
                rule,
                command: collapse(&segment),
            });
        }
    }
    None
}

/// The directory a `cd`/`pushd` leaves behind: the argument resolved against
/// the current one, or `None` when it cannot be resolved lexically.
fn cd_target(args: &[String], cwd: Option<&str>) -> Option<String> {
    let mut args = args.iter().peekable();
    if args.peek().map(|a| a.as_str()) == Some("--") {
        args.next();
    }
    let arg = args.next()?;
    if arg == "-" {
        return None;
    }
    resolve_target(arg, cwd)
}

/// A write target resolved lexically: absolute paths as written, `~` and
/// `$HOME` spellings kept literal (they never name the workspace), anything
/// with a command substitution or an unresolvable variable left unknown.
fn resolve_target(t: &str, cwd: Option<&str>) -> Option<String> {
    if t.contains('`') {
        return None;
    }
    let home_prefixed = t == "~"
        || t.starts_with("~/")
        || t == "$HOME"
        || t.starts_with("$HOME/")
        || t == "${HOME}"
        || t.starts_with("${HOME}/");
    if t.contains('$') && !home_prefixed {
        return None;
    }
    let n = normalise_target(t);
    if n.starts_with('/') || home_prefixed {
        return Some(n);
    }
    let cwd = cwd?;
    Some(normalise_target(&format!("{cwd}/{n}")))
}

/// Scratch and sink targets no jail needs to stop: the workspace itself,
/// `/dev/null` and the standard-device spellings, and the OS temp roots
/// (`/tmp`, macOS `/private/tmp` and `/var/folders`).
fn in_jail(resolved: &str, root: &str) -> bool {
    if root == "/" {
        if resolved.starts_with('/') {
            return true;
        }
    } else if resolved == root || resolved.starts_with(&format!("{root}/")) {
        return true;
    }
    if matches!(
        resolved,
        "/dev/null" | "/dev/stdout" | "/dev/stderr" | "/dev/stdin" | "/dev/tty"
    ) {
        return true;
    }
    ["/tmp/", "/private/tmp/", "/var/folders/"]
        .iter()
        .any(|p| resolved == p.trim_end_matches('/') || resolved.starts_with(p))
}

/// A redirect operator split off a token (after leading fd digits): whether
/// it writes, and the attached remainder (`""` when the target follows).
/// Longer operators first so `>>` wins over `>`, `<<<` over `<`.
fn split_redirect(rest: &str) -> Option<(bool, &str)> {
    for op in ["&>>", "&>", ">>", ">|", "<>", "<<<", "<<", ">", "<"] {
        if let Some(after) = rest.strip_prefix(op) {
            let write = !matches!(op, "<" | "<<" | "<<<");
            return Some((write, after));
        }
    }
    None
}

/// A no-target remainder: a descriptor dup (`>&1`, `>&-`), a bare fd
/// (`>&2` after the `&` is consumed below), or `>|`'s pipe.
fn is_dup_target(after: &str) -> bool {
    after == "|"
        || after == "-"
        || after.starts_with('&')
        || after.chars().all(|c| c.is_ascii_digit())
}

/// `write-outside-jail`: a redirect, a `tee` operand, or a `cp`/`mv`/
/// `install` destination that resolves outside the workspace root. Reads and
/// unresolvable targets pass through: the jail judges writes it can see.
fn write_outside_jail(tokens: &[String], cwd: Option<&str>, root: &str) -> bool {
    let mut targets: Vec<&str> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i].as_str();
        let rest = tok.trim_start_matches(|c: char| c.is_ascii_digit());
        if let Some((write, after)) = split_redirect(rest) {
            if write {
                if after.is_empty() {
                    if let Some(next) = tokens.get(i + 1) {
                        targets.push(next);
                        i += 1;
                    }
                } else if !is_dup_target(after) {
                    targets.push(after);
                }
            } else if after.is_empty() {
                i += 1;
            }
            i += 1;
            continue;
        }
        i += 1;
    }
    if !tokens.is_empty() {
        if is_program(&tokens[0], "tee") {
            targets.extend(flag_operands(&tokens[1..]));
        } else if is_program(&tokens[0], "cp") || is_program(&tokens[0], "mv") {
            targets.extend(copy_destination(&tokens[1..]));
        } else if is_program(&tokens[0], "install") {
            targets.extend(install_destinations(&tokens[1..]));
        }
    }
    targets.iter().any(|t| match resolve_target(t, cwd) {
        None => false,
        Some(r) => !in_jail(&r, root),
    })
}

/// Operands after flags: `--` ends flags, lone `-` and `--flag=value`
/// spellings are operands, every other `-…` token is skipped.
fn flag_operands(args: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = args.iter().peekable();
    while let Some(a) = rest.next() {
        if a == "--" {
            out.extend(rest.map(|s| s.as_str()));
            break;
        }
        if a.starts_with('-') && a.len() > 1 {
            continue;
        }
        out.push(a);
    }
    out
}

/// The destination of `cp`/`mv`: the `-t`/`--target-directory` value when
/// given, else the last operand. Sources are reads; the jail leaves them.
fn copy_destination(args: &[String]) -> Vec<&str> {
    let mut rest = args.iter().peekable();
    while let Some(a) = rest.next() {
        if a == "--" {
            break;
        }
        if let Some(v) = a.strip_prefix("--target-directory=") {
            return vec![v];
        }
        if a == "-t" || a == "--target-directory" {
            return rest.next().map(|v| vec![v.as_str()]).unwrap_or_default();
        }
    }
    flag_operands(args).last().copied().into_iter().collect()
}

/// The destinations of `install`: every operand with `-d` (all are created
/// directories), the `-t` value when given, else the last operand.
fn install_destinations(args: &[String]) -> Vec<&str> {
    let mut mkdirs = false;
    let mut rest = args.iter().peekable();
    while let Some(a) = rest.next() {
        if a == "--" {
            break;
        }
        if a == "-d" || a == "--directory" {
            mkdirs = true;
        }
        if let Some(v) = a.strip_prefix("--target-directory=") {
            return vec![v];
        }
        if a == "-t" || a == "--target-directory" {
            return rest.next().map(|v| vec![v.as_str()]).unwrap_or_default();
        }
    }
    let ops = flag_operands(args);
    if mkdirs {
        return ops;
    }
    ops.last().copied().into_iter().collect()
}

/// Whitespace runs collapsed to one space, trimmed.
fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split a shell line into simple commands outside quotes.
fn segments(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in command.chars() {
        if escaped {
            cur.push(c);
            escaped = false;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            cur.push(c);
            continue;
        }
        match c {
            '\\' => {
                escaped = true;
                cur.push(c);
            }
            '\'' | '"' => {
                quote = Some(c);
                cur.push(c);
            }
            ';' | '\n' | '|' | '&' => {
                if !cur.trim().is_empty() {
                    out.push(std::mem::take(&mut cur));
                } else {
                    cur.clear();
                }
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// The text after the first `-c` token of a `<shell> -c <body>` segment,
/// one pair of surrounding quotes removed; `None` when there is no `-c`.
fn shell_c_body(segment: &str) -> Option<String> {
    let mut rest = segment;
    let mut base = 0usize;
    loop {
        let trimmed = rest.trim_start();
        base += rest.len() - trimmed.len();
        rest = trimmed;
        if rest.is_empty() {
            return None;
        }
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let tok = rest[..end].replace(['"', '\''], "");
        if tok == "-c" {
            let body = segment[base + end..].trim();
            let unquoted = match body.as_bytes() {
                [b'"', .., b'"'] | [b'\'', .., b'\''] if body.len() >= 2 => {
                    &body[1..body.len() - 1]
                }
                _ => body,
            };
            return Some(unquoted.to_string());
        }
        base += end;
        rest = &rest[end..];
    }
}

/// The short flags of a wrapper that take the next token as their value
/// (`sudo -u root rm …`, `xargs -I {} rm …`); every other leading `-…`
/// token is dropped alone.
fn wrapper_value_flags(wrapper: &str) -> &'static [&'static str] {
    match wrapper {
        "sudo" => &["-C", "-D", "-g", "-h", "-p", "-r", "-t", "-T", "-u", "-U"],
        "doas" => &["-C", "-u"],
        "env" => &["-u", "-C", "-S"],
        "xargs" => &["-I", "-n", "-L", "-P", "-s", "-d", "-E", "-a", "-J", "-R"],
        "time" => &["-f", "-o"],
        "timeout" => &["-s", "--signal", "-k", "--kill-after"],
        "nice" => &["-n", "--adjustment"],
        "ionice" => &["-c", "-n", "-p", "-P", "-t"],
        "stdbuf" => &["-i", "-o", "-e", "--input", "--output", "--error"],
        "caffeinate" => &["-t", "-w"],
        "chroot" => &["--userspec", "--groups"],
        _ => &[],
    }
}

/// How many bare positional tokens a wrapper consumes before the command it
/// runs: `timeout DURATION cmd`, `chroot NEWROOT cmd` (Gate 1 round 5 LOW).
fn wrapper_positional_args(wrapper: &str) -> usize {
    match wrapper {
        "timeout" | "chroot" => 1,
        _ => 0,
    }
}

/// Whitespace tokens with every quote character removed (`"$HOME"/` and
/// `'/'` spell the same target as the bare word — Gate 1), leading wrappers
/// ([`WRAPPERS`], each with its flags — `sudo -u root`, `env -i`, `xargs
/// -I{}` — README) and `NAME=value` assignments dropped, and one leading
/// backslash stripped from the program word (`\rm`, the shell's alias
/// bypass, runs `rm` — Gate 1: it passed the guard).
fn tokens(segment: &str) -> Vec<String> {
    let mut toks: Vec<String> = segment
        .split_whitespace()
        .map(|t| t.replace(['"', '\''], ""))
        .filter(|t| !t.is_empty())
        .collect();
    while let Some(first) = toks.first_mut() {
        if let Some(rest) = first.strip_prefix('\\') {
            *first = rest.to_string();
            if first.is_empty() {
                toks.remove(0);
            }
            continue;
        }
        // A leading group opener (`{ rm …`, `( rm …`, `{rm`, `(rm`) is
        // dropped, one character at a time (Gate 1 round 5 LOW).
        if let Some(rest) = first.strip_prefix(|c| c == '{' || c == '(') {
            *first = rest.to_string();
            if first.is_empty() {
                toks.remove(0);
            }
            continue;
        }
        // A leading shell keyword (`then rm …`, `do rm …`, `! rm …`).
        if SHELL_KEYWORDS.contains(&first.as_str()) {
            toks.remove(0);
            continue;
        }
        let wrapper = WRAPPERS.iter().find(|w| first.as_str() == **w).copied();
        let assignment = first
            .split_once('=')
            .map(|(name, _)| {
                !name.is_empty()
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    && !name.starts_with(|c: char| c.is_ascii_digit())
            })
            .unwrap_or(false);
        if let Some(w) = wrapper {
            toks.remove(0);
            // The wrapper's own flags; `--` ends them.
            while let Some(flag) = toks.first().cloned() {
                if flag == "--" {
                    toks.remove(0);
                    break;
                }
                if !flag.starts_with('-') || flag == "-" {
                    break;
                }
                toks.remove(0);
                if wrapper_value_flags(w).contains(&flag.as_str()) && !toks.is_empty() {
                    toks.remove(0);
                }
            }
            // Then its bare positional(s) before the command (`timeout 5 …`,
            // `chroot / …`), never a `-flag` (Gate 1 round 5 LOW).
            for _ in 0..wrapper_positional_args(w) {
                if toks.first().map(|t| !t.starts_with('-')).unwrap_or(false) {
                    toks.remove(0);
                }
            }
        } else if assignment {
            toks.remove(0);
        } else {
            break;
        }
    }
    // A trailing group closer (`rm -rf /)`, `( rm -rf / )`, `{ rm -rf / }`)
    // is dropped from the end of the simple command the same way (Gate 1
    // round 5 LOW); `{}` — find's placeholder — is not a closer.
    while let Some(last) = toks.last_mut() {
        if last == ")" || last == "}" {
            toks.pop();
            continue;
        }
        let trimmed = last.trim_end_matches(')');
        if trimmed.len() == last.len() {
            break;
        }
        *last = trimmed.to_string();
        if last.is_empty() {
            toks.pop();
        }
    }
    toks
}

fn is_program(token: &str, name: &str) -> bool {
    token == name || token.ends_with(&format!("/{name}"))
}

/// `rm -r`/`-R`/`-rf`/`-fr`/`--recursive` with a root-ish target — or,
/// when the line changed into a root first (`cd / && …`), with the working
/// directory itself (`.`, `./`, `*`, `./*`) as the target.
fn rm_root(args: &[String], cwd_is_root: bool) -> bool {
    let mut recursive = false;
    let mut targets: Vec<&str> = Vec::new();
    let mut only_targets = false;
    for a in args {
        if only_targets {
            targets.push(a);
        } else if a == "--" {
            only_targets = true;
        } else if a == "--recursive" {
            recursive = true;
        } else if a.starts_with("--") {
            // other long flags
        } else if let Some(cluster) = a.strip_prefix('-') {
            if cluster.contains('r') || cluster.contains('R') {
                recursive = true;
            }
        } else {
            targets.push(a);
        }
    }
    recursive
        && targets
            .iter()
            .any(|t| is_root_target(t) || (cwd_is_root && is_cwd_target(t)))
}

/// `find <root> … -delete` (or `-exec rm …`): the walk starts at `/`, `~`,
/// `$HOME` or — after a `cd` into one — the working directory, and the
/// action removes what it finds. The starting points are the leading
/// arguments up to the first `-…` expression (find's grammar); `-delete`
/// and `-exec rm` anywhere after them (Gate 1: the review checklist's one
/// denied line the guard let through).
fn find_root_delete(args: &[String], cwd_is_root: bool) -> bool {
    let mut starts: Vec<&str> = Vec::new();
    let mut rest: &[String] = &[];
    for (i, a) in args.iter().enumerate() {
        if a.starts_with('-') || a == "(" || a == "!" {
            rest = &args[i..];
            break;
        }
        starts.push(a);
    }
    if rest.is_empty() && starts.len() == args.len() {
        return false;
    }
    let deletes = rest.iter().enumerate().any(|(i, a)| {
        a == "-delete"
            || ((a == "-exec" || a == "-execdir" || a == "-ok" || a == "-okdir")
                && rest
                    .get(i + 1)
                    .map(|p| {
                        is_program(p, "rm") || is_program(p, "rmdir") || is_program(p, "unlink")
                    })
                    .unwrap_or(false))
    });
    if !deletes {
        return false;
    }
    if starts.is_empty() {
        // No starting point: find walks the working directory.
        return cwd_is_root;
    }
    starts
        .iter()
        .any(|t| is_root_target(t) || (cwd_is_root && is_cwd_target(t)))
}

/// A target normalised for [`is_root_target`]: runs of `/` collapsed to
/// one, trailing `/*`, `/**` and `/` stripped repeatedly (a lone `/`
/// stays), then the path resolved lexically — `.` segments dropped and a
/// `..` consuming the segment before it (never a root, a `~`/`$HOME`
/// prefix, or another `..`) — so `//`, `/*/`, `~//`, `$HOME/*`, `/./`,
/// `/home/../` and `$HOME/x/..` all name the root (Gate 1). A relative
/// path that resolves to nothing is `.`, the working directory.
fn normalise_target(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        if c == '/' && s.ends_with('/') {
            continue;
        }
        s.push(c);
    }
    loop {
        let stars = s.trim_end_matches('*');
        if stars.len() < s.len() && stars.ends_with('/') {
            s.truncate(stars.len());
            continue;
        }
        if s.len() > 1 && s.ends_with('/') {
            s.pop();
            continue;
        }
        break;
    }
    if !s.contains('/') {
        return s;
    }
    let absolute = s.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for seg in s.split('/') {
        match seg {
            "" | "." => {}
            ".." => match parts.last() {
                Some(&last) if last != ".." && !is_home_prefix(last) => {
                    parts.pop();
                }
                Some(_) => parts.push(".."),
                None if absolute => {}
                None => parts.push(".."),
            },
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    match (absolute, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_string(),
        (false, false) => joined,
    }
}

/// `~`, `$HOME`, `${HOME}` — a first segment `..` must not consume.
fn is_home_prefix(seg: &str) -> bool {
    matches!(seg, "~" | "$HOME" | "${HOME}")
}

/// `/`, `~`, `$HOME`, `${HOME}` in any of their `/`, `/*`, `//` spellings.
fn is_root_target(t: &str) -> bool {
    matches!(
        normalise_target(t).as_str(),
        "/" | "~" | "$HOME" | "${HOME}"
    )
}

/// `.`, `./`, `*`, `./*` — the working directory as a target.
fn is_cwd_target(t: &str) -> bool {
    let n = normalise_target(t);
    matches!(n.as_str(), "." | "*" | "./*") || n.trim_end_matches('*') == "."
}

/// `git push` with `--force` / `-f…` / `--force-with-lease[=…]` /
/// `--force-if-includes` or a `+refspec`, to `main` / `master`.
fn force_push_main(args: &[String]) -> bool {
    let Some(push) = args.iter().position(|a| a == "push") else {
        return false;
    };
    let after = &args[push + 1..];
    let mut force = false;
    let mut to_main = false;
    for a in after {
        if a == "--force" || a.starts_with("--force-with-lease") || a == "--force-if-includes" {
            force = true;
        } else if a.starts_with("--") {
            // other long flags (`--set-upstream`, `--tags`)
        } else if let Some(cluster) = a.strip_prefix('-') {
            if cluster.contains('f') {
                force = true;
            }
        } else {
            let (forced, spec) = match a.strip_prefix('+') {
                Some(rest) => (true, rest),
                None => (false, a.as_str()),
            };
            if forced {
                force = true;
            }
            let dest = spec.rsplit_once(':').map(|(_, d)| d).unwrap_or(spec);
            let dest = dest.strip_prefix("refs/heads/").unwrap_or(dest);
            if dest == "main" || dest == "master" {
                to_main = true;
            }
        }
    }
    force && to_main
}

fn reset_hard(args: &[String]) -> bool {
    args.iter().any(|a| a == "reset") && args.iter().any(|a| a == "--hard")
}

/// `DROP TABLE` / `DROP DATABASE` / `DROP SCHEMA`, case-insensitive,
/// anywhere in the line.
fn drop_statement(command: &str) -> bool {
    let lower = collapse(command).to_ascii_lowercase();
    ["drop table", "drop database", "drop schema"]
        .iter()
        .any(|p| lower.contains(p))
}

// ---- omm-stop (Stop) ----------------------------------------------------------

/// The Stop decision: `{}` on a continuation, on a message that claims no
/// completion, or on one that already cites evidence; else the block nudge.
pub fn stop(event: &Value) -> Value {
    if event_name(event) != Some(EVENT_STOP) {
        return json!({});
    }
    if event
        .get("stop_hook_active")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return json!({});
    }
    // The verify gate (absorbed from the upstream `verify.json` flow): a
    // pending claim means the session opened a check it never closed, so
    // stop into one continuation that records the result first.
    if let Some(block) = super::hook_gates::pending_verify_block(event) {
        return block;
    }
    let message = event
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !claims_completion(message) || cites_evidence(message) {
        return json!({});
    }
    json!({"decision": "block", "reason": STOP_NUDGE})
}

fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_ascii_lowercase())
        .collect()
}

/// One of [`CLAIM_WORDS`] as a whole word.
pub fn claims_completion(message: &str) -> bool {
    words(message)
        .iter()
        .any(|w| CLAIM_WORDS.contains(&w.as_str()))
}

/// An exit status, a pass/fail count, or a diff/cmp result in the message.
pub fn cites_evidence(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    const PHRASES: [&str; 10] = [
        "exit status",
        "exit code",
        "exited with",
        "exit 0",
        "rc=",
        "returned 0",
        "return code",
        "status 0",
        "status: 0",
        "all tests pass",
    ];
    if PHRASES.iter().any(|p| lower.contains(p)) {
        return true;
    }
    let w = words(&lower);
    const COUNTED: [&str; 10] = [
        "passed", "failed", "pass", "fail", "ok", "failures", "errors", "error", "passing", "tests",
    ];
    for (i, tok) in w.iter().enumerate() {
        if tok.chars().all(|c| c.is_ascii_digit()) {
            let window = &w[i + 1..(i + 3).min(w.len())];
            if window.iter().any(|n| COUNTED.contains(&n.as_str())) {
                return true;
            }
        }
    }
    if lower.contains("diff") || lower.contains("cmp") {
        const RESULTS: [&str; 7] = [
            "identical",
            "no differences",
            "no difference",
            "byte-identical",
            "differ",
            "no changes",
            "clean",
        ];
        if RESULTS.iter().any(|r| lower.contains(r)) {
            return true;
        }
    }
    false
}

// ---- omm-session-start (SessionStart) -------------------------------------------

/// The skills-catalog estimate behind the context line (host-reality.md
/// "Budgets": 32,000-B cap, header/footer, the built-in block with and
/// without `first_sentence`, gate off — a session has no plugins gate — and
/// the shipped bundle's own catalog numbers).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogEstimate {
    pub bytes: u64,
    pub cap: u64,
    pub entries: usize,
    /// Descriptions the host would drop tail-first (stage 2) to fit.
    pub dropped: usize,
    pub first_sentence: bool,
}

/// The estimate from `settings.json` (mode) and the bundle's `catalog.json`
/// (`budget` totals, per-asset fallback). `None` without a content bundle.
pub fn estimate_catalog(roots: &Roots) -> Option<CatalogEstimate> {
    let settings: Value = std::fs::read(roots.settings_file())
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let first_sentence = settings
        .pointer("/run/context_slimming/skill_catalog_descriptions")
        .and_then(Value::as_str)
        == Some(hr::CONTEXT_SLIMMING_FIRST_SENTENCE);
    let source = ContentSource::locate(&roots.omm_root())?;
    let catalog = source.catalog().ok()?;
    let skills: Vec<_> = catalog.shipped_of(AssetKind::Skill).collect();
    let per_asset = |a: &omm_manifest::catalog::Asset| -> u64 {
        if first_sentence {
            a.budget_bytes_first_sentence.unwrap_or(a.budget_bytes)
        } else {
            a.budget_bytes
        }
    };
    let total_key = if first_sentence {
        "skills_total_first_sentence_bytes"
    } else {
        "skills_total_bytes"
    };
    let bundle: u64 = catalog
        .budget
        .as_ref()
        .and_then(|b| b.get(total_key))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| skills.iter().map(|a| per_asset(a)).sum());
    let builtin = if first_sentence {
        hr::BUILTIN_SKILLS_FIRST_SENTENCE_BLOCK_BYTES_GATE_OFF
    } else {
        hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF
    };
    let cap = hr::SKILLS_CATALOG_CAP_BYTES;
    let mut bytes = builtin + bundle;
    let mut dropped = 0usize;
    let mut fitted = bytes;
    for a in skills.iter().rev() {
        if fitted <= cap {
            break;
        }
        let base = hr::CATALOG_ENTRY_BASE_BYTES
            + format!("plugin:{}:{}", catalog.plugin_id, a.id).len() as u64
            + format!("plugin://{}/{}", catalog.plugin_id, a.path).len() as u64;
        fitted = fitted.saturating_sub(per_asset(a).saturating_sub(base));
        dropped += 1;
    }
    if dropped == 0 {
        bytes = fitted;
    }
    Some(CatalogEstimate {
        bytes,
        cap,
        // The bundled visible skills plus the second builtin plugin's skill
        // (`plugin:threejs:threejs` composes unconditionally since
        // 1.3.0-R3057.1) plus the workspace skills.
        entries: hr::BUNDLED_SKILLS_VISIBLE_DEFAULT + 1 + skills.len(),
        dropped,
        first_sentence,
    })
}

/// `omm 0.1.0 · profile default · catalog 18,420/32,000 B (27 entries)`
/// (+ the stage-2 warning when over budget).
pub fn context_line(version: &str, profile: Option<&str>, est: Option<&CatalogEstimate>) -> String {
    let mut line = format!("omm {version} · profile {}", profile.unwrap_or("none"));
    if let Some(e) = est {
        line.push_str(&format!(
            " · catalog {}/{} B ({} entries{})",
            thousands(e.bytes),
            thousands(e.cap),
            e.entries,
            if e.first_sentence {
                ", first_sentence"
            } else {
                ""
            }
        ));
        if e.dropped > 0 {
            line.push_str(&format!(
                " · WARNING: {} skill description{} silently dropped (stage 2) - run omm cost",
                e.dropped,
                if e.dropped == 1 { "" } else { "s" }
            ));
        }
    }
    line
}

/// The SessionStart decision.
pub fn session_start(event: &Value, roots: &Roots) -> Value {
    if event_name(event) != Some(EVENT_SESSION_START) {
        return json!({});
    }
    let config = std::fs::read(OmmConfig::path(&roots.omm_root()))
        .map(|b| OmmConfig::from_bytes_lenient(&b))
        .unwrap_or_default();
    let est = estimate_catalog(roots);
    json!({
        "hookSpecificOutput": {
            "hookEventName": EVENT_SESSION_START,
            "additionalContext": context_line(OMM_VERSION, config.profile(), est.as_ref()),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pre(tool: &str, cmd: &str) -> Value {
        json!({"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {"command": cmd}, "tool_use_id": "t1"})
    }

    #[test]
    fn guard_denies_the_readme_rules_and_nothing_else() {
        for (cmd, rule) in [
            ("rm -rf /", "rm-root"),
            ("rm -rf /*", "rm-root"),
            ("rm -r ~", "rm-root"),
            ("rm -Rf ~/", "rm-root"),
            ("rm -fr $HOME", "rm-root"),
            ("rm -rf ${HOME}/", "rm-root"),
            ("sudo rm -rf /", "rm-root"),
            ("cd /tmp && rm -rf /", "rm-root"),
            ("rm --recursive --force /", "rm-root"),
            ("FOO=1 rm -rf -- /", "rm-root"),
            // Gate 1: trivial respellings of the documented targets.
            ("rm -rf //", "rm-root"),
            ("rm -rf \"$HOME\"/", "rm-root"),
            ("rm -rf /*/", "rm-root"),
            ("rm -rf '/'", "rm-root"),
            ("rm -rf ~//", "rm-root"),
            ("rm -rf /**", "rm-root"),
            ("rm -rf \"${HOME}\"/*", "rm-root"),
            ("cd / && rm -rf ./", "rm-root"),
            ("cd / && rm -rf .", "rm-root"),
            ("cd ~ ; rm -rf *", "rm-root"),
            ("cd \"$HOME\" && ls && rm -r ./*", "rm-root"),
            // Gate 1: the alias-bypass spelling and `.` / `..` segments.
            ("\\rm -rf /", "rm-root"),
            ("sudo \\rm -rf /", "rm-root"),
            ("rm -rf /./", "rm-root"),
            ("rm -rf /home/../", "rm-root"),
            ("rm -rf ~/./", "rm-root"),
            ("rm -rf $HOME/x/..", "rm-root"),
            ("rm -rf /./*", "rm-root"),
            ("find /./ -delete", "find-root-delete"),
            // Gate 1 decision G: a shell body and a wrapper's flags.
            ("bash -c \"rm -rf /\"", "rm-root"),
            ("sh -c 'rm -rf /'", "rm-root"),
            ("sh -c 'cd / && rm -rf .'", "rm-root"),
            ("/bin/sh -c \"sh -c 'rm -rf ~'\"", "rm-root"),
            ("echo hi | xargs -I{} rm -rf /", "rm-root"),
            ("echo / | xargs -n1 rm -rf /", "rm-root"),
            ("sudo -u root rm -rf /", "rm-root"),
            ("env -i rm -rf /", "rm-root"),
            ("sudo -- rm -rf /", "rm-root"),
            ("bash -c 'git push --force origin main'", "force-push-main"),
            // Gate 1: the checklist's `find` line.
            ("find / -delete", "find-root-delete"),
            ("find / -name '*.log' -delete", "find-root-delete"),
            ("find ~ -type f -delete", "find-root-delete"),
            (
                "find \"$HOME\"/ -mtime +30 -exec rm -f {} \\;",
                "find-root-delete",
            ),
            ("sudo find /* -delete", "find-root-delete"),
            ("cd / && find . -delete", "find-root-delete"),
            ("cd ~ && find -delete", "find-root-delete"),
            ("git push --force origin main", "force-push-main"),
            ("git push -f origin master", "force-push-main"),
            ("git push --force-with-lease origin main", "force-push-main"),
            (
                "git push --force-with-lease=main origin HEAD:main",
                "force-push-main",
            ),
            ("git push origin +main", "force-push-main"),
            ("git push -fu origin refs/heads/main", "force-push-main"),
            ("git reset --hard", "reset-hard"),
            ("git reset --hard HEAD~3", "reset-hard"),
            ("psql -c 'DROP TABLE users'", "drop-table"),
            ("echo 'drop   database prod' | mysql", "drop-table"),
            ("sqlite3 x.db \"Drop Schema public\"", "drop-table"),
            // Gate 1 round 5 (LOW): scheduling / namespace wrappers and shell
            // group openers / keywords stripped before the program word.
            ("timeout 5 rm -rf /", "rm-root"),
            ("timeout -s KILL 5 rm -rf /", "rm-root"),
            ("nice rm -rf /", "rm-root"),
            ("nice -n 10 rm -rf /", "rm-root"),
            ("ionice -c 3 rm -rf /", "rm-root"),
            ("setsid rm -rf /", "rm-root"),
            ("caffeinate rm -rf /", "rm-root"),
            ("caffeinate -t 10 rm -rf /", "rm-root"),
            ("stdbuf -o0 rm -rf /", "rm-root"),
            ("chroot / rm -rf /", "rm-root"),
            ("timeout 5 find / -delete", "find-root-delete"),
            ("{ rm -rf /; }", "rm-root"),
            ("( rm -rf / )", "rm-root"),
            ("{ rm -rf /;}", "rm-root"),
            ("(rm -rf /)", "rm-root"),
            ("if true; then rm -rf /; fi", "rm-root"),
            ("for x in 1; do rm -rf /; done", "rm-root"),
            ("! rm -rf /", "rm-root"),
            ("{ ( ! timeout 5 rm -rf / ) }", "rm-root"),
            ("nice sudo rm -rf /", "rm-root"),
        ] {
            let v = guard_verdict(cmd).unwrap_or_else(|| panic!("{cmd} must be denied"));
            assert_eq!(v.rule, rule, "{cmd}");
            assert!(
                v.reason().starts_with(&format!("omm guard: {rule} - `")),
                "{}",
                v.reason()
            );
        }
        for cmd in [
            "rm -rf ./build",
            "rm -rf /tmp/x",
            "rm -rf /tmp//",
            "rm -rf ./",
            "rm -rf .",
            "rm -rf ../build",
            "rm -rf ./..",
            "rm -rf ~/..",
            "rm -rf /tmp/./x",
            "rm -rf /tmp/../tmp",
            "\\rm -rf ./build",
            "cd /tmp && rm -rf ./",
            "cd / && cd /tmp && rm -rf .",
            "cd / && rm -rf ./build",
            "rm /",
            "rm -f ~/notes.txt",
            "bash -c \"ls /\"",
            "sh -c 'rm -rf ./build'",
            "xargs rm -rf",
            "find / | xargs rm -rf",
            "sudo -n ls /",
            "sh -c",
            "find / -name '*.log'",
            "find /tmp/build -delete",
            "find . -name '*.o' -delete",
            "find ~/Downloads -mtime +30 -delete",
            "find / -type f -exec grep -l foo {} +",
            "git push origin main",
            "git push --force",
            "git push --force origin feature",
            "git push --force-with-lease origin feature/x",
            "git reset --soft HEAD~1",
            "git reset HEAD file",
            "ls -la /",
            "echo drop the table",
            "cargo test",
            "",
            // Gate 1 round 5 (LOW): the new wrappers and group tokens change
            // nothing about a harmless target.
            "timeout 5 rm -rf ./build",
            "nice -n 10 ls /",
            "(ls /)",
            "{ rm -rf ./build; }",
            "find / -name '*.o' -exec ls {} \\;",
        ] {
            assert!(guard_verdict(cmd).is_none(), "{cmd} must pass");
        }
    }

    #[test]
    fn guard_jails_writes_outside_the_workspace() {
        let root = || Some("/ws".to_string());
        for cmd in [
            "echo hi > /etc/motd",
            "echo hi >> /etc/motd",
            "echo hi > /",
            "make 2> /etc/log",
            "cmd 2>/etc/log",
            "echo hi | sudo tee /etc/motd",
            "tee -a /etc/motd",
            "cp new /etc/cron.d/x",
            "cp -t /etc new",
            "mv a.txt /tmp/../etc/x",
            "install -m 644 f /usr/local/bin/f",
            "install -d /etc/omm",
            "install -t /etc f",
            "echo hi > ~/notes.txt",
            "echo hi > $HOME/.bashrc",
            "echo hi > ${HOME}/.bashrc",
            "echo x > ../etc/x",
            "echo x > ../other/f",
            "cd - && echo hi > /etc/x",
            "cd /etc && echo hi > motd",
            "cd / && echo hi > motd",
            "sh -c 'echo hi > /etc/x'",
            "bash -c \"tee /etc/x\"",
        ] {
            let v =
                guard_verdict_root(cmd, root()).unwrap_or_else(|| panic!("{cmd} must be denied"));
            assert_eq!(v.rule, "write-outside-jail", "{cmd}");
            assert!(
                v.reason().starts_with("omm guard: write-outside-jail - `"),
                "{}",
                v.reason()
            );
        }
        for cmd in [
            "echo hi > out.txt",
            "echo hi > /ws/out.txt",
            "echo hi > ./sub/out.txt",
            "echo hi > /dev/null",
            "cmd 2>&1",
            "cmd | tee /tmp/x.log",
            "cmd | tee /ws/x.log",
            "cat < /etc/passwd",
            "cat <<EOF",
            "cp /etc/a /ws/b",
            "cp a b",
            "mv a /ws/b",
            "install a /ws/bin/a",
            "install -d /ws/etc",
            "cd /ws/sub && echo hi > ../other/f",
            "cd /ws && echo hi > motd",
            "cd - && echo hi > rel.txt",
            "echo hi > $OUT/x",
            "echo hi > `cmd`/x",
        ] {
            assert!(
                guard_verdict_root(cmd, root()).is_none(),
                "{cmd} must be allowed"
            );
        }
        // Without a workspace root the jail does not apply (rootless rules
        // still do).
        assert!(guard_verdict("echo hi > /etc/motd").is_none());
        assert_eq!(guard_verdict("rm -rf /").expect("rootless").rule, "rm-root");
    }

    #[test]
    fn guard_decision_shapes() {
        assert_eq!(guard(&json!({})), json!({}));
        assert_eq!(guard(&pre("read", "rm -rf /")), json!({}));
        assert_eq!(guard(&pre("bash", "ls")), json!({}));
        assert_eq!(
            guard(
                &json!({"hook_event_name": "Stop", "tool_name": "bash", "tool_input": {"command": "rm -rf /"}})
            ),
            json!({})
        );
        let deny = guard(&pre("bash_input", "git push --force origin main"));
        assert_eq!(deny["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert_eq!(deny["hookSpecificOutput"]["permissionDecision"], "deny");
        let reason = deny["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap();
        assert_eq!(
            reason,
            "omm guard: force-push-main - `git push --force origin main`"
        );
        assert!(deny.get("continue").is_none() && deny.get("additionalContext").is_none());
        let long = format!("rm -rf / {}", "x".repeat(400));
        let reason = guard_verdict(&long).unwrap().reason();
        assert!(reason.chars().count() < REASON_COMMAND_MAX + 40);
        assert!(reason.ends_with("…`"));
    }

    #[test]
    fn segments_respect_quotes() {
        assert_eq!(
            segments("a; b && c | d\ne"),
            vec!["a", " b ", " c ", " d", "e"]
        );
        assert_eq!(segments("echo 'a;b' && c"), vec!["echo 'a;b' ", " c"]);
        assert_eq!(segments("echo \"x|y\""), vec!["echo \"x|y\""]);
        assert!(segments("").is_empty());
        assert_eq!(tokens("sudo -- rm"), vec!["rm"]);
        assert_eq!(tokens("A=1 B=2 env rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("\"rm\" '-rf' \"/\""), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("\\rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("sudo \\rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("sudo -u root rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("xargs -I{} rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("xargs -I {} rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(tokens("env -i -- rm -rf /"), vec!["rm", "-rf", "/"]);
        assert_eq!(
            shell_c_body("bash -c \"rm -rf /\""),
            Some("rm -rf /".into())
        );
        assert_eq!(shell_c_body("sh -lc 'ls'"), None);
        assert_eq!(shell_c_body("sh -c"), Some(String::new()));
        for (t, n) in [
            ("/./", "/"),
            ("/home/../", "/"),
            ("/..", "/"),
            ("/./*", "/"),
            ("~/./", "~"),
            ("$HOME/x/..", "$HOME"),
            ("${HOME}/./", "${HOME}"),
            ("./", "."),
            ("./*", "."),
            ("x/..", "."),
            ("..", ".."),
            ("~/..", "~/.."),
            ("/tmp/./x", "/tmp/x"),
            ("/tmp/../tmp", "/tmp"),
            ("*", "*"),
        ] {
            assert_eq!(normalise_target(t), n, "{t}");
        }
    }

    #[test]
    fn stop_nudges_only_unproven_completion_claims() {
        let ev = |active: bool, msg: &str| json!({"hook_event_name": "Stop", "stop_hook_active": active, "last_assistant_message": msg, "turn_id": "t"});
        assert_eq!(stop(&json!({})), json!({}));
        assert_eq!(stop(&ev(true, "All done.")), json!({}));
        assert_eq!(stop(&ev(false, "Here is what I found so far.")), json!({}));
        assert_eq!(
            stop(&ev(false, "Done: cargo test → 42 passed, 0 failed.")),
            json!({})
        );
        assert_eq!(
            stop(&ev(false, "Fixed; the build exited with exit status 0.")),
            json!({})
        );
        assert_eq!(
            stop(&ev(
                false,
                "Implemented. diff against the golden file: identical."
            )),
            json!({})
        );
        let nudge = stop(&ev(false, "The feature is implemented and ready."));
        assert_eq!(nudge["decision"], "block");
        assert_eq!(nudge["reason"], STOP_NUDGE);
        assert!(nudge.get("additionalContext").is_none());
        assert!(claims_completion("It's DONE."));
        assert!(!claims_completion("undone business"));
        assert!(cites_evidence("rc=0"));
        assert!(cites_evidence("3 tests passed"));
        assert!(cites_evidence("0 failures"));
        assert!(!cites_evidence("I ran the tests"));
        assert!(!cites_evidence("diff the files"));
    }

    #[test]
    fn context_line_formats_and_warns() {
        assert_eq!(
            context_line("0.1.0", None, None),
            "omm 0.1.0 · profile none"
        );
        let est = CatalogEstimate {
            bytes: 18_420,
            cap: 32_000,
            entries: 27,
            dropped: 0,
            first_sentence: false,
        };
        assert_eq!(
            context_line("0.1.0", Some("default"), Some(&est)),
            "omm 0.1.0 · profile default · catalog 18,420/32,000 B (27 entries)"
        );
        let over = CatalogEstimate {
            dropped: 3,
            first_sentence: true,
            ..est
        };
        let line = context_line("0.1.0", Some("fast"), Some(&over));
        assert!(line.contains(", first_sentence)"));
        assert!(line
            .ends_with("WARNING: 3 skill descriptions silently dropped (stage 2) - run omm cost"));
    }

    #[test]
    fn session_start_estimate_from_the_checkout() {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let roots = sb.roots().unwrap();
        let est = estimate_catalog(&roots).expect("the checkout's content/");
        assert_eq!(est.cap, hr::SKILLS_CATALOG_CAP_BYTES);
        assert!(est.entries > hr::BUNDLED_SKILLS_VISIBLE_DEFAULT);
        assert!(est.bytes > hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF);
        assert!(
            est.bytes <= est.cap,
            "the shipped bundle fits (R18): {est:?}"
        );
        assert_eq!(est.dropped, 0);
        assert!(!est.first_sentence);
        let cfg = roots.muse_config();
        std::fs::create_dir_all(&cfg).unwrap();
        std::fs::write(
            cfg.join("settings.json"),
            br#"{"schema_version":1,"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence"}}}"#,
        )
        .unwrap();
        let fs = estimate_catalog(&roots).unwrap();
        assert!(fs.first_sentence && fs.bytes < est.bytes);
        let out = session_start(
            &json!({"hook_event_name": "SessionStart", "source": "startup"}),
            &roots,
        );
        let ctx = out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap();
        assert!(
            ctx.starts_with(&format!("omm {OMM_VERSION} · profile none · catalog ")),
            "{ctx}"
        );
        assert_eq!(session_start(&json!({}), &roots), json!({}));
        assert_eq!(
            dispatch("no-such", &Event::Parsed(json!({})), &roots),
            json!({})
        );
    }

    #[test]
    fn the_guard_denies_what_it_cannot_evaluate_and_the_rest_fails_open() {
        // Gate 1 decision G.
        let roots = omm_host::paths::Roots::resolve(&omm_host::paths::EnvView {
            home: Some("/h".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(parse_event(b""), Event::Malformed);
        assert_eq!(parse_event(b"not json"), Event::Malformed);
        assert_eq!(parse_event(b"{}"), Event::Parsed(json!({})));
        let over = vec![b' '; STDIN_LIMIT_BYTES as usize + 1];
        assert_eq!(parse_event(&over), Event::Truncated);
        let mut at_limit = vec![b' '; STDIN_LIMIT_BYTES as usize - 2];
        at_limit.extend_from_slice(b"{}");
        assert_eq!(parse_event(&at_limit), Event::Parsed(json!({})));
        for ev in [Event::Malformed, Event::Truncated] {
            let d = dispatch(HANDLER_GUARD, &ev, &roots);
            assert_eq!(
                d["hookSpecificOutput"]["permissionDecision"], "deny",
                "{ev:?}"
            );
            assert_eq!(d["hookSpecificOutput"]["hookEventName"], EVENT_PRE_TOOL_USE);
            assert_eq!(
                d["hookSpecificOutput"]["permissionDecisionReason"],
                UNEVALUABLE_REASON
            );
            assert_eq!(dispatch(HANDLER_STOP, &ev, &roots), json!({}), "{ev:?}");
            assert_eq!(dispatch(HANDLER_SESSION_START, &ev, &roots), json!({}));
            assert_eq!(dispatch("no-such", &ev, &roots), json!({}));
        }
        assert_eq!(dispatch(HANDLER_GUARD, &Event::Absent, &roots), json!({}));
        assert_eq!(
            dispatch(
                HANDLER_GUARD,
                &Event::Parsed(pre("bash", "rm -rf /")),
                &roots
            )["hookSpecificOutput"]["permissionDecision"],
            "deny"
        );
    }

    #[test]
    fn observed_dispatch_matches_dispatch_and_names_the_reason() {
        use HookObservation::*;
        let roots = omm_host::paths::Roots::resolve(&omm_host::paths::EnvView {
            home: Some("/h".into()),
            ..Default::default()
        })
        .unwrap();
        // Wire parity: the observed value is byte-identical to dispatch.
        let cases: Vec<(&str, Event)> = vec![
            (HANDLER_GUARD, Event::Parsed(pre("bash", "rm -rf /"))),
            (HANDLER_GUARD, Event::Parsed(pre("bash", "ls /tmp"))),
            (HANDLER_GUARD, Event::Parsed(json!({}))),
            (HANDLER_GUARD, Event::Malformed),
            (HANDLER_GUARD, Event::Truncated),
            (HANDLER_GUARD, Event::Absent),
            (HANDLER_STOP, Event::Parsed(json!({}))),
            (HANDLER_STOP, Event::Malformed),
            (HANDLER_SESSION_START, Event::Truncated),
            ("no-such", Event::Parsed(json!({}))),
            ("no-such", Event::Malformed),
        ];
        for (name, event) in &cases {
            let (value, _) = dispatch_observed(name, event, &roots);
            assert_eq!(value, dispatch(name, event, &roots), "{name} {event:?}");
        }
        // Reasons.
        assert!(matches!(
            dispatch_observed(
                HANDLER_GUARD,
                &Event::Parsed(pre("bash", "rm -rf /")),
                &roots
            )
            .1,
            GuardDeny { rule: "rm-root" }
        ));
        assert_eq!(
            dispatch_observed(
                HANDLER_GUARD,
                &Event::Parsed(pre("bash", "ls /tmp")),
                &roots
            )
            .1,
            GuardAllow
        );
        assert_eq!(
            dispatch_observed(HANDLER_GUARD, &Event::Parsed(json!({})), &roots).1,
            GuardWrongEvent
        );
        assert_eq!(
            dispatch_observed(HANDLER_GUARD, &Event::Malformed, &roots).1,
            GuardDeny {
                rule: "unevaluable"
            }
        );
        assert_eq!(
            dispatch_observed(HANDLER_STOP, &Event::Malformed, &roots).1,
            Noop {
                reason: "malformed-event"
            }
        );
        assert_eq!(
            dispatch_observed(HANDLER_STOP, &Event::Truncated, &roots).1,
            Noop {
                reason: "truncated-event"
            }
        );
        assert_eq!(
            dispatch_observed(HANDLER_STOP, &Event::Absent, &roots).1,
            Noop {
                reason: "absent-event"
            }
        );
        assert_eq!(
            dispatch_observed("no-such", &Event::Parsed(json!({})), &roots).1,
            Noop {
                reason: "unknown-handler"
            }
        );
        assert_eq!(
            dispatch_observed(
                HANDLER_STOP,
                &Event::Parsed(json!({"hook_event_name": "Stop"})),
                &roots
            )
            .1,
            Handled {
                handler: HANDLER_STOP
            }
        );
    }
}
