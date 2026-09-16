//! Doctor D16 — skill routing (PLAN.md 3.1; ARCHITECTURE.md §6 shape: id,
//! severity, what was observed, why it is silent, the exact fix).
//!
//! Appended by `omm doctor` after the crate-level D1–D14 (the routing model
//! lives in this crate, beside the router: the same formula, the same
//! handler shape, the same library reader — never a second copy in
//! `omm-doctor`). Off is a pass: routing is opt-in and nothing is at
//! stake while it is off. On, the row checks what the router depends on
//! at session time and the host never reports: the workspace's trust
//! (a project hooks file never loads untrusted, no record), the handler
//! in `.muse/hooks.json` and that its program word still resolves (a
//! failing handler is a `failed` terminal at most, never an error), the
//! library files (a routed path that vanished is injected and then
//! `skill-file-missing` on read), and the combined 31,984-byte budget
//! against the LIVE order-200 size when the echo session ran (`--fast`
//! skips it: the recorded size then) — above it the host drops every
//! description silently first and rejects the selection second.

use std::path::Path;

use serde_json::Value;

use omm_doctor::{Check, Report};
use omm_host::host_reality as hr;
use omm_host::trust::TrustStore;

use super::routing::{self, State};
use super::{thousands, OmmConfig};
use crate::cmd::Ctx;

pub const ID: &str = "D16";
pub const TITLE: &str = "skill routing";
pub const WHY_SILENT: &str = "a project-tier router in an untrusted workspace never loads (no hook run, no record); a handler whose program does not resolve or whose file lost the `outputCapabilities` line fails open with at most a `failed` hook terminal; a routed path that vanished is injected and only `read_skill` reports `skill-file-missing`; and past `order200 + order201 = 31,984 B` the host drops every routed description silently (`status: completed`, no diagnostic) before it rejects the selection (research/experiments/skill-routing.md V1, V2, V4; host-reality.md \"Budgets\": routing budget)";
pub const FIX_ENABLE: &str = "omm enable skill-routing";
pub const FIX_SLIM: &str =
    "omm settings set run.context_slimming.skill_catalog_descriptions first_sentence\nomm enable skill-routing";

/// Append D16 to a report (recomputing `ok`, which a Warn never changes).
pub fn append(ctx: &Ctx, dctx: &omm_doctor::Context, report: &mut Report) {
    let row = check(ctx, dctx);
    let critical = row.severity == omm_doctor::Severity::Critical;
    report.checks.push(row);
    if critical {
        report.ok = false;
    }
}

/// The D16 row.
pub fn check(ctx: &Ctx, dctx: &omm_doctor::Context) -> Check {
    let omm_root = ctx.omm_root();
    let config = match OmmConfig::load(&omm_root) {
        Ok(c) => c,
        Err(e) => {
            return Check::warn(
                ID,
                TITLE,
                format!(
                    "{} could not be read ({e}); the routing state is unknown",
                    OmmConfig::path(&omm_root).display()
                ),
                WHY_SILENT,
                format!(
                    "repair {} (a JSON object), then {FIX_ENABLE}",
                    OmmConfig::path(&omm_root).display()
                ),
            )
        }
    };
    if !config.skill_routing() {
        return Check::info(
            ID,
            TITLE,
            format!("off (opt-in): `{FIX_ENABLE}` in a trusted git workspace turns it on; nothing is routed and nothing is at stake"),
            WHY_SILENT,
        );
    }
    let Some(state) = State::from_config(&config) else {
        return Check::warn(
            ID,
            TITLE,
            "skill_routing is on in config.json but no `routing` record says where; `omm run` exports the gates for a router that was never written",
            WHY_SILENT,
            FIX_ENABLE,
        );
    };
    let ws = match state.workspace.clone().or_else(|| dctx.workspace()) {
        Some(w) => w,
        None => {
            return Check::warn(
                ID,
                TITLE,
                "on, but no workspace is recorded and there is no cwd",
                WHY_SILENT,
                FIX_ENABLE,
            )
        }
    };
    let mut problems: Vec<String> = Vec::new();
    let mut fixes: Vec<&str> = Vec::new();

    // Trust: the project tier loads only for a `trusted` workspace.
    let trust_fix = format!("omm trust {}", ws.display());
    match TrustStore::for_roots(&ctx.roots).and_then(|s| s.decision_for(&ws)) {
        Ok(Some(d)) if d == "trusted" => {}
        Ok(Some(d)) => {
            problems.push(format!(
                "{} is `{d}` in trust.json: the router never loads",
                ws.display()
            ));
            fixes.push(&trust_fix);
        }
        Ok(None) => {
            problems.push(format!(
                "{} is not in trust.json: the router never loads",
                ws.display()
            ));
            fixes.push(&trust_fix);
        }
        Err(e) => problems.push(format!("trust.json could not be read ({e}); see D12")),
    }

    // The handler and its program word.
    let hooks_file = state
        .hooks_file
        .clone()
        .unwrap_or_else(|| ws.join(routing::WS_HOOKS_FILE));
    match std::fs::read(&hooks_file) {
        Ok(b) => match serde_json::from_slice::<Value>(&b) {
            Ok(doc) => {
                let commands = routing::handler_commands(&doc);
                match (&state.handler_command, commands.first()) {
                    (_, None) => {
                        problems.push(format!(
                            "{} carries no router handler",
                            hooks_file.display()
                        ));
                        fixes.push(FIX_ENABLE);
                    }
                    (Some(want), Some(have)) if want != have => {
                        problems.push(format!(
                            "{} carries the handler `{have}`, the ledger recorded `{want}`",
                            hooks_file.display()
                        ));
                        fixes.push(FIX_ENABLE);
                    }
                    (_, Some(have)) => {
                        if let Some(why) = program_problem(have) {
                            problems.push(why);
                            fixes.push(FIX_ENABLE);
                        }
                    }
                }
            }
            Err(e) => {
                problems.push(format!(
                    "{} is not JSON ({e}): the host drops the whole file",
                    hooks_file.display()
                ));
                fixes.push(FIX_ENABLE);
            }
        },
        Err(_) => {
            problems.push(format!("{} is missing", hooks_file.display()));
            fixes.push(FIX_ENABLE);
        }
    }

    // The library files: regular, non-link, present.
    let lib_dir = ws.join(routing::WS_SKILLS_DIR);
    let mut missing: Vec<String> = Vec::new();
    for id in &state.skills {
        let p = lib_dir.join(id).join(routing::SKILL_FILE);
        match std::fs::symlink_metadata(&p) {
            Ok(m) if m.is_file() => {}
            _ => missing.push(id.clone()),
        }
    }
    if !missing.is_empty() {
        problems.push(format!(
            "{} of {} routed skill files missing or not regular under {} ({})",
            missing.len(),
            state.skills.len(),
            lib_dir.display(),
            missing.join(" ")
        ));
        fixes.push(FIX_ENABLE);
    }
    let library = routing::load_workspace_library(&lib_dir);

    // The combined budget, against the live catalog when it was measured.
    let order201_max = routing::worst_case_bytes(&library, &hooks_file);
    let recorded = state.order200_bytes;
    let live: Option<u64> = dctx
        .live()
        .ok()
        .and_then(|l| l.block_bytes(hr::CONTEXT_ORDER_SKILLS_CATALOG))
        .map(|b| b as u64);
    let (basis, basis_label) = match (live, recorded) {
        (Some(l), Some(r)) if l > r => (l, "live"),
        (Some(_), Some(r)) => (r, "recorded"),
        (Some(l), None) => (l, "live"),
        (None, Some(r)) => (r, "recorded; no live session (--fast)"),
        (None, None) => (
            hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES,
            "built-ins only, nothing measured",
        ),
    };
    let headroom = hr::ROUTING_BUDGET_BYTES as i64 - basis as i64 - order201_max as i64;
    let budget_line = format!(
        "budget: order 200 {} B ({basis_label}) + order 201 at most {} B = {} of {} B, headroom {} B",
        thousands(basis),
        thousands(order201_max),
        thousands(basis + order201_max),
        thousands(hr::ROUTING_BUDGET_BYTES),
        super::routing_enable::signed_thousands(headroom)
    );
    if headroom < routing::BUDGET_MARGIN_BYTES as i64 {
        problems.push(format!(
            "{budget_line}: within {} B of the cap, the router trims or drops entries every turn and a fuller catalog silently loses every routed description",
            routing::BUDGET_MARGIN_BYTES
        ));
        fixes.push(FIX_SLIM);
    }
    if let (Some(l), Some(r)) = (live, recorded) {
        if l > r + routing::BUDGET_MARGIN_BYTES {
            problems.push(format!(
                "the router budgets against a recorded order 200 of {} B but the live session composes {} B (re-measure)",
                thousands(r),
                thousands(l)
            ));
            fixes.push(FIX_ENABLE);
        }
    }

    // Gates: informational — `omm run` exports them; a plain `muse` needs them.
    let set = |name: &str| {
        std::env::var_os(name)
            .map(|v| v == hr::GATE_ON_VALUE)
            .unwrap_or(false)
    };
    let gates_line = format!(
        "gates {}/{}: exported by `omm run`; in this shell {}",
        hr::ENV_ROUTING_GATE,
        hr::ENV_ROUTING_APPLY_GATE,
        match (set(hr::ENV_ROUTING_GATE), set(hr::ENV_ROUTING_APPLY_GATE)) {
            (true, true) => "both set",
            (false, false) => "unset (a plain `muse` here routes nothing)",
            _ => "only one set (both are needed; a plain `muse` here routes nothing)",
        }
    );

    if problems.is_empty() {
        return Check::info(
            ID,
            TITLE,
            format!(
                "on in {}: {} routed skills under {}, handler `{}` in {}; {budget_line}; {gates_line}",
                ws.display(),
                library.len(),
                lib_dir.display(),
                state.handler_command.as_deref().unwrap_or("?"),
                hooks_file.display(),
            ),
            WHY_SILENT,
        );
    }
    fixes.dedup();
    let mut fix_lines: Vec<&str> = Vec::new();
    for f in fixes {
        if !fix_lines.contains(&f) {
            fix_lines.push(f);
        }
    }
    if fix_lines.is_empty() {
        fix_lines.push(FIX_ENABLE);
    }
    Check::warn(
        ID,
        TITLE,
        format!(
            "on in {}: {}; {gates_line}",
            ws.display(),
            problems.join("; ")
        ),
        WHY_SILENT,
        fix_lines.join("\n"),
    )
}

/// Why a handler's program word would not run under `$SHELL -c` in the
/// hook's scrubbed environment (`PATH` passes, nothing else): `None` when
/// it resolves.
pub fn program_problem(command: &str) -> Option<String> {
    let program = program_word(command);
    if program.is_empty() {
        return Some("the handler command is empty".into());
    }
    if program.contains('/') {
        let p = Path::new(&program);
        return match std::fs::metadata(p) {
            Ok(m) if m.is_file() => None,
            _ => Some(format!("handler program `{program}` does not exist; the hook fails open and nothing is routed")),
        };
    }
    let found = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).any(|d| d.join(&program).is_file()))
        .unwrap_or(false);
    if found {
        None
    } else {
        Some(format!(
            "handler program `{program}` is not on PATH; the hook fails open and nothing is routed"
        ))
    }
}

/// The first shell word of a command: a single-quoted word unquoted, else
/// up to the first space.
pub fn program_word(command: &str) -> String {
    let c = command.trim_start();
    if let Some(rest) = c.strip_prefix('\'') {
        let mut out = String::new();
        let mut chars = rest.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\'' {
                // `'\''` — an escaped quote inside a quoted word.
                if chars.peek() == Some(&'\\') {
                    chars.next();
                    if chars.next() == Some('\'') && chars.next() == Some('\'') {
                        out.push('\'');
                        continue;
                    }
                }
                break;
            }
            out.push(ch);
        }
        return out;
    }
    c.split(' ').next().unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_words_and_resolution() {
        assert_eq!(program_word("omm hook route"), "omm");
        assert_eq!(
            program_word("'/Volumes/OWC Envoy/omm' hook route"),
            "/Volumes/OWC Envoy/omm"
        );
        assert_eq!(program_word("'/it'\\''s/omm' hook route"), "/it's/omm");
        assert_eq!(program_word(""), "");
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("omm");
        std::fs::write(&exe, b"").unwrap();
        assert_eq!(
            program_problem(&format!("{} hook route", exe.display())),
            None
        );
        assert!(program_problem("/nonexistent/omm hook route").is_some());
        assert!(program_problem("").is_some());
        assert!(program_problem("zzz-no-such-program-omm hook route").is_some());
        assert_eq!(
            program_problem("sh hook route"),
            None,
            "sh is on every PATH"
        );
    }
}
