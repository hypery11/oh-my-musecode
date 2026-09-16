//! D6 — `agent_definitions.safe_mode: true` (ARCHITECTURE.md §6 row D6).

use serde_json::Value;

use crate::check::{Check, Context};
use crate::checks::settings_unreadable;

pub const ID: &str = "D6";
pub const TITLE: &str = "agent safe_mode";
/// `research/experiments/00-DECISION.md` §1.2 row 43 (proven by exhaustive
/// elimination incl. a purpose-built plugin agent-definition capability):
/// `agent_definitions.safe_mode` suppresses the `user`, `project` AND
/// `plugin` composition slots; only `managed` + `built_in` survive, neither
/// reachable by a framework. No diagnostic anywhere.
pub const WHY_SILENT: &str = "`agent_definitions.safe_mode: true` suppresses the user, project and plugin agent-definition slots (only managed + built_in survive), so every shipped agent pack silently never loads (00-DECISION.md §1.2 row 43; settings-keys.json row 3)";
pub const FIX: &str = "omm settings set agent_definitions.safe_mode false";

pub fn run(ctx: &Context) -> Check {
    if let Err(e) = ctx.settings() {
        return settings_unreadable(ID, TITLE, WHY_SILENT, e);
    }
    match ctx.setting("agent_definitions.safe_mode") {
        Some(Value::Bool(true)) => Check::warn(
            ID,
            TITLE,
            "agent_definitions.safe_mode = true: user, project and plugin agent packs never load",
            WHY_SILENT,
            FIX,
        ),
        Some(Value::Bool(false)) => Check::info(
            ID,
            TITLE,
            "agent_definitions.safe_mode = false",
            WHY_SILENT,
        ),
        Some(other) => Check::warn(
            ID,
            TITLE,
            format!("agent_definitions.safe_mode is {other} (not a bool); the host refuses the whole file on a wrong type"),
            WHY_SILENT,
            FIX,
        ),
        None => Check::info(ID, TITLE, "agent_definitions.safe_mode unset (off)", WHY_SILENT),
    }
}
