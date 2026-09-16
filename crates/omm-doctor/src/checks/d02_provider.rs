//! D2 — `settings.provider` (ARCHITECTURE.md §6 row D2).
//!
//! A pristine install does NOT set `provider`: it is a credential-adjacent
//! choice the host's own login flow makes (Gate 1 decision). An unset key is
//! therefore an informational row that says what breaks and how to set it;
//! it becomes a warning only when the ledger shows omm itself wrote the key
//! (a `settings-key` registration for `provider`) and it is gone now — omm
//! removed or changed something the user had.

use serde_json::Value;

use crate::check::{Check, Context};
use crate::checks::settings_unreadable;
use crate::ledger::LedgerState;

pub const ID: &str = "D2";
pub const TITLE: &str = "settings.provider";
/// `docs/host-reality.md` "Trust lifecycle": `muse serve` requires
/// `settings.json → provider`; without it the agent runtime is never
/// constructed — a one-tool session (`active_tools: ["write_todos"]`), no MCP,
/// no capability composition (00-DECISION.md §1.1 row 8). Nothing reports it.
pub const WHY_SILENT: &str = "`muse serve` (every MSP/SDK host) requires `settings.json → provider`; without it the runtime is never constructed — a one-tool session with no MCP and no capability composition — and nothing says so (host-reality.md \"Trust lifecycle\": `muse serve`; 00-DECISION.md §1.1 row 8)";
pub const FIX: &str = "omm settings set provider meta";
/// The dotted settings key.
pub const KEY: &str = "provider";

pub fn run(ctx: &Context) -> Check {
    if let Err(e) = ctx.settings() {
        return settings_unreadable(ID, TITLE, WHY_SILENT, e);
    }
    match ctx.setting(KEY) {
        Some(Value::String(p)) if !p.is_empty() => {
            Check::info(ID, TITLE, format!("provider = \"{p}\""), WHY_SILENT)
        }
        Some(other) => Check::warn(
            ID,
            TITLE,
            format!("provider is {other} (not a string); the host refuses the whole file on a wrong type"),
            WHY_SILENT,
            FIX,
        ),
        None => {
            let written_by_omm = match ctx.ledger() {
                LedgerState::Loaded(l) => l.settings_key(KEY).cloned(),
                _ => None,
            };
            match written_by_omm {
                Some(reg) => Check::warn(
                    ID,
                    TITLE,
                    format!(
                        "provider is unset, but the ledger records omm wrote it (prior {}, wrote {}): omm removed or changed a key the user had; `muse serve`/SDK gets a one-tool session and no MCP until it is set",
                        reg.prior.as_ref().map(Value::to_string).unwrap_or_else(|| "absent".into()),
                        reg.value.as_ref().map(Value::to_string).unwrap_or_else(|| "unrecorded".into()),
                    ),
                    WHY_SILENT,
                    FIX,
                ),
                None => Check::info(
                    ID,
                    TITLE,
                    format!(
                        "provider is unset (the host's login flow sets it; omm never does): `muse exec`/TUI default to meta, but `muse serve`/SDK gets a one-tool session and no MCP — to set it: `{FIX}`"
                    ),
                    WHY_SILENT,
                ),
            }
        }
    }
}
