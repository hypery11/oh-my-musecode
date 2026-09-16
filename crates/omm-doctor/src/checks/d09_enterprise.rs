//! D9 — `muse config status` rows, informational (ARCHITECTURE.md §6 row D9).

use omm_host::host_reality as hr;
use omm_host::probe;

use crate::check::{Check, Context};

pub const ID: &str = "D9";
pub const TITLE: &str = "enterprise probe";
/// `docs/host-reality.md` "Paths": enterprise — the defaults plane is live,
/// the policy plane inert (`field_not_activated`), the delivery directory
/// unknown (00-DECISION.md §1.2 row 42; R21: no enterprise tier). A present
/// defaults document would change what composes with nothing in a session
/// saying where the value came from; doctor names the sources so a user can
/// tell managed defaults from their own settings.
pub const WHY_SILENT: &str = "an enterprise defaults document (system file or macOS managed preferences) changes what composes with nothing in a session saying where a value came from; the policy plane is inert and the delivery directory unknown, so omm builds nothing on it — this row only names what the host sees (host-reality.md \"Paths\": enterprise; 00-DECISION.md §1.2 row 42; R21)";

pub fn run(ctx: &Context) -> Check {
    match probe::config_status(&ctx.inv) {
        Ok(status) => {
            let rows: Vec<String> = status
                .sources
                .iter()
                .map(|s| format!("{}/{} {}", s.plane, s.source_class, s.state))
                .collect();
            let present: Vec<&str> = status
                .sources
                .iter()
                .filter(|s| s.state != "absent")
                .map(|s| s.source_class.as_str())
                .collect();
            let generation = if status.generation == hr::ENTERPRISE_GENERATION {
                "generation matches host-reality".to_string()
            } else {
                format!(
                    "generation {} (host-reality has {})",
                    status.generation,
                    hr::ENTERPRISE_GENERATION
                )
            };
            let mut observed = format!("{} sources: {}; {generation}", rows.len(), rows.join(", "));
            if !present.is_empty() {
                observed.push_str(&format!(
                    "; a document is PRESENT from {} — managed defaults are in play",
                    present.join(", ")
                ));
            }
            Check::info(ID, TITLE, observed, WHY_SILENT)
        }
        Err(e) => Check::warn(
            ID,
            TITLE,
            format!("`muse config status` failed: {e}"),
            WHY_SILENT,
            "muse config status",
        ),
    }
}
