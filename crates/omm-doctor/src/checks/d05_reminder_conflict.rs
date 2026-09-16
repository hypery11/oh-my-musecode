//! D5 — canonical (`settings.plugins.<id>.enabled`) vs legacy
//! (`settings.runtime_capabilities["plugin:tbh-reminders:reminder:<id>"].enabled`)
//! reminder enablement disagree (ARCHITECTURE.md §6 row D5).

use serde_json::Value;

use crate::check::{Check, Context};
use crate::checks::{settings_unreadable, wired_reminders};

pub const ID: &str = "D5";
pub const TITLE: &str = "reminder conflict";
/// `research/experiments/settings-plugins.md` §4.4 (all nine cells
/// reproduced by its verifier): when the two stores disagree for a wired id
/// the reminder is `inactive` with the TUI-only notice `legacy and canonical
/// reminder settings conflict`; no CLI lane reports it, and `muse plugins
/// approve plugin:tbh-reminders:reminder:<id>` manufactures the conflict
/// silently when `settings.plugins` disagrees (verification C2;
/// 00-DECISION.md §1.1 row 19). The canonical store alone is a no-op.
pub const WHY_SILENT: &str = "when `settings.plugins.<id>.enabled` and `settings.runtime_capabilities[\"plugin:tbh-reminders:reminder:<id>\"].enabled` disagree the reminder is inactive with a TUI-only `legacy and canonical reminder settings conflict` notice and no CLI symptom; `muse plugins approve` creates the conflict silently when the canonical store disagrees (settings-plugins.md §4.4, verification C2; 00-DECISION.md §1.1 row 19)";
pub const FIX: &str = "omm settings reconcile-reminders";

pub fn run(ctx: &Context) -> Check {
    let doc = match ctx.settings() {
        Ok(d) => d,
        Err(e) => return settings_unreadable(ID, TITLE, WHY_SILENT, e),
    };
    let wired = match wired_reminders() {
        Ok(w) => w,
        Err(e) => {
            return Check::warn(
                ID,
                TITLE,
                format!("could not read the wired reminder ids: {e}"),
                WHY_SILENT,
                FIX,
            )
        }
    };
    let plugins = doc.get(&["plugins".to_string()]);
    let legacy = doc.get(&["runtime_capabilities".to_string()]);
    let mut conflicts = Vec::new();
    let mut notes = Vec::new();
    for r in &wired {
        let canonical = plugins
            .and_then(|p| p.get(&r.plugin_id))
            .and_then(|e| e.get("enabled"))
            .and_then(Value::as_bool);
        let legacy_enabled = legacy
            .and_then(|l| l.get(&r.stable_id))
            .and_then(|e| e.get("enabled"))
            .and_then(Value::as_bool);
        match (legacy_enabled, canonical) {
            (Some(l), Some(c)) if l != c => conflicts.push(format!(
                "{}: legacy enabled={l} vs canonical enabled={c}",
                r.plugin_id
            )),
            (None, Some(c)) => notes.push(format!(
                "{}: canonical enabled={c} with no legacy entry — a no-op, the reminder stays active",
                r.plugin_id
            )),
            _ => {}
        }
    }
    if !conflicts.is_empty() {
        return Check::warn(
            ID,
            TITLE,
            format!("conflict: {}", conflicts.join("; ")),
            WHY_SILENT,
            FIX,
        );
    }
    let mut observed = format!(
        "{} wired reminder ids agree across both stores",
        wired.len()
    );
    if !notes.is_empty() {
        observed.push_str(&format!(" ({})", notes.join("; ")));
    }
    Check::info(ID, TITLE, observed, WHY_SILENT)
}
