//! D7 — enabled plugins vs the 256 ceiling, warning at 200 (ARCHITECTURE.md §6 row D7).

use crate::check::{Check, Context};

pub const ID: &str = "D7";
pub const TITLE: &str = "plugin count";
/// `research/experiments/quotas.md` (00-DECISION.md §1.2 row 33):
/// `plugin_preflight_overflow` counts ENABLED plugins — 256 clean, 257 fires —
/// and is soft: skills, commands, hooks and MCP still work, only the
/// agent-definition review lane is disarmed, with no session-time notice.
pub const WHY_SILENT: &str = "the 257th enabled plugin trips `plugin_preflight_overflow`, a soft advisory that disarms only the agent-definition review lane while everything else keeps working — nothing in a session says so (00-DECISION.md §1.2 row 33; host-reality.md \"Budgets\": enabled plugins)";

pub fn run(ctx: &Context) -> Check {
    let list = match ctx.plugins() {
        Ok(l) => l,
        Err(e) => {
            return Check::warn(
                ID,
                TITLE,
                format!("could not list plugins: {e}"),
                WHY_SILENT,
                "muse plugins list --json",
            )
        }
    };
    let enabled = list.enabled();
    let n = enabled.len();
    let max = ctx.options.plugin_count_max;
    let warn_at = ctx.options.plugin_count_warn_at;
    let observed = format!(
        "{n} enabled of {} installed (warn at {warn_at}, ceiling {max})",
        list.plugins.len()
    );
    if n >= warn_at {
        let others: Vec<&str> = enabled
            .iter()
            .filter(|p| p.id != ctx.plugin_id)
            .map(|p| p.id.as_str())
            .take(5)
            .collect();
        let target = others.first().copied().unwrap_or("<id>");
        let fix = format!(
            "muse plugins disable {target}{}",
            if others.len() > 1 {
                format!("   # candidates: {}", others.join(", "))
            } else {
                String::new()
            }
        );
        let detail = if n > max {
            format!("{observed}: over the ceiling — plugin_preflight_overflow, agent-definition review lane disarmed")
        } else {
            observed
        };
        return Check::warn(ID, TITLE, detail, WHY_SILENT, fix);
    }
    Check::info(ID, TITLE, observed, WHY_SILENT)
}
