//! D12 — the workspace is in `trust.json` (ARCHITECTURE.md §6 row D12).

use omm_host::trust::TrustStore;
use omm_host::HostError;

use crate::check::{Check, Context};

pub const ID: &str = "D12";
pub const TITLE: &str = "workspace trust";
/// `docs/host-reality.md` "Paths" and "Trust lifecycle": project skills
/// (`.agents/skills/`), project hooks (`.muse/hooks.json`), project rules
/// (`AGENTS.md`), workflows, agents, plugin installs and the
/// `personal_project` memory scope are all trust-gated and silently inert
/// without an entry (00-DECISION.md §1.1 row 15: a project-tier router in an
/// untrusted workspace — no hook run, no terminal record, no error). Only
/// the TUI's first-run prompt writes the store; `--trust-workspace` is
/// per-run and persists nothing; a malformed store aborts every session.
pub const WHY_SILENT: &str = "in a workspace absent from trust.json the project skills, hooks, rules, workflows, agents and the personal_project memory scope are all silently inert — no hook run, no record, no error (host-reality.md \"Paths\": project skills/rules/hooks; 00-DECISION.md §1.1 row 15); nothing but the TUI's first-run prompt writes the store and `--trust-workspace` persists nothing";

pub fn run(ctx: &Context) -> Check {
    let Some(ws) = ctx.workspace() else {
        return Check::warn(
            ID,
            TITLE,
            "no workspace to check (no cwd)",
            WHY_SILENT,
            "omm trust <workspace>",
        );
    };
    let arg = ctx.workspace_arg();
    let fix = format!("omm trust {arg}");
    if !ws.is_dir() {
        return Check::warn(
            ID,
            TITLE,
            format!("workspace {} does not exist", ws.display()),
            WHY_SILENT,
            fix,
        );
    }
    let path = ctx.roots.trust_file();
    let store = match TrustStore::load(&path) {
        Ok(s) => s,
        Err(HostError::Trust { detail, .. }) => {
            return Check::critical(
                ID,
                TITLE,
                format!("{} is malformed ({detail}): every session aborts with exit 1 (`malformed trust store`)", path.display()),
                WHY_SILENT,
                format!("repair {} by hand (schema_version 1, projects.<abs>.decision trusted|untrusted), then {fix}", path.display()),
            )
        }
        Err(e) => {
            return Check::critical(
                ID,
                TITLE,
                format!("{} cannot be read: {e}", path.display()),
                WHY_SILENT,
                fix,
            )
        }
    };
    if !store.existed() {
        return Check::warn(
            ID,
            TITLE,
            format!(
                "no trust.json at {}: every workspace is untrusted",
                path.display()
            ),
            WHY_SILENT,
            fix,
        );
    }
    match store.decision_for(&ws) {
        Ok(Some(d)) if d == "trusted" => Check::info(
            ID,
            TITLE,
            format!("{} is trusted", ws.display()),
            WHY_SILENT,
        ),
        Ok(Some(d)) => Check::warn(
            ID,
            TITLE,
            format!("{} is recorded as `{d}`", ws.display()),
            WHY_SILENT,
            fix,
        ),
        Ok(None) => Check::warn(
            ID,
            TITLE,
            format!(
                "{} is not in trust.json ({} project(s) recorded)",
                ws.display(),
                projects(&store)
            ),
            WHY_SILENT,
            fix,
        ),
        Err(e) => Check::warn(
            ID,
            TITLE,
            format!("could not resolve {}: {e}", ws.display()),
            WHY_SILENT,
            fix,
        ),
    }
}

fn projects(store: &TrustStore) -> usize {
    store
        .value()
        .get("projects")
        .and_then(|p| p.as_object())
        .map(|m| m.len())
        .unwrap_or(0)
}
