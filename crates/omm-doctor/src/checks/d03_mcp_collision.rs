//! D3 — `mcpServers` and `mcp_servers` both present (ARCHITECTURE.md §6 row D3).

use omm_host::host_reality as hr;

use crate::check::{Check, Context};
use crate::checks::settings_unreadable;

pub const ID: &str = "D3";
pub const TITLE: &str = "mcp key collision";
/// `docs/host-reality.md` "Exit codes": both spellings present — even both
/// `{}` — make every settings-MUTATING command exit 1 with `MCP configuration
/// error …; MCP is disabled for this runtime`, while every read-only lane and
/// every session stays silent (00-DECISION.md §1.1 row 20). The legacy
/// spelling alone works but is destroyed by the host's next settings rewrite
/// (host-reality.md "Paths": settings rewrite, measured 2026-09-01).
pub const WHY_SILENT: &str = "with both spellings present every settings-mutating muse command exits 1 (`MCP configuration error …; MCP is disabled for this runtime`) and MCP is silently absent from every session, while read-only lanes and sessions print nothing (host-reality.md \"Exit codes\"; 00-DECISION.md §1.1 row 20); the legacy `mcp_servers` spelling alone is destroyed by the host's next settings rewrite (host-reality.md \"Paths\": settings rewrite)";
pub const FIX: &str = "omm settings fix-mcp-collision";

pub fn run(ctx: &Context) -> Check {
    let doc = match ctx.settings() {
        Ok(d) => d,
        Err(e) => return settings_unreadable(ID, TITLE, WHY_SILENT, e),
    };
    if doc.has_mcp_collision() {
        return Check::critical(
            ID,
            TITLE,
            format!(
                "both `mcpServers` and `mcp_servers` are present in {}: every settings-writing muse command (and `omm install`) exits 1 (`{} …`)",
                doc.path().display(),
                hr::MCP_COLLISION_MESSAGE
            ),
            WHY_SILENT,
            FIX,
        );
    }
    let has_canonical = doc.get(&["mcpServers".to_string()]).is_some();
    let legacy_keys: Vec<String> = hr::settings_keys()
        .map(|k| {
            k.legacy_spellings
                .iter()
                .filter(|l| doc.get(std::slice::from_ref(&l.key)).is_some())
                .map(|l| l.key.clone())
                .collect()
        })
        .unwrap_or_default();
    if let Some(legacy) = legacy_keys.first() {
        return Check::warn(
            ID,
            TITLE,
            format!(
                "only the legacy `{legacy}` spelling is present: it works today, but the host's next settings rewrite (any `muse skills enable|disable`, `plugins approve`, TUI settings change) destroys it and MCP silently vanishes"
            ),
            WHY_SILENT,
            FIX,
        );
    }
    Check::info(
        ID,
        TITLE,
        if has_canonical {
            "`mcpServers` only (no legacy spelling)"
        } else {
            "no MCP servers declared in settings.json (none of the two spellings)"
        },
        WHY_SILENT,
    )
}
