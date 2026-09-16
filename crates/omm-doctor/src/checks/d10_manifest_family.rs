//! D10 — the installed plugin is `manifest_family == "native"`
//! (ARCHITECTURE.md §6 row D10), plus the three registration observations of
//! `docs/experiments/marketplace-precedence.md` §6.5.

use serde_json::Value;

use omm_host::host_reality as hr;

use crate::check::{Check, Context, InspectFailure};
use crate::ledger::LedgerState;

pub const ID: &str = "D10";
pub const TITLE: &str = "manifest family";
/// `docs/host-reality.md` "Identity constraints": `compat.manifestDir` is a
/// consistency rule, not a value rule — the same bytes validate clean in all
/// three dot-dirs with DIFFERENT runtime semantics (`enabledDefault` ignored
/// under `claude-compatible`; 00-DECISION.md §1.1 row 22). A missing root
/// `marketplace.json` silently serves the foreign projection instead
/// (marketplace-precedence.md §1 F3). No diagnostic distinguishes the families.
pub const WHY_SILENT: &str = "the same manifest bytes validate clean as native, claude-compatible or codex-compatible with different runtime semantics (`enabledDefault` ignored outside native), and a missing root marketplace.json makes the marketplace serve the foreign projection — nothing in `plugins list`, `inspect` or a session says which family loaded (host-reality.md \"Identity constraints\": compat.manifestDir; marketplace-precedence.md §1 F3, §6.5)";
pub const FIX: &str = "omm install --reinstall";

pub fn run(ctx: &Context) -> Check {
    let pid = ctx.plugin_id.as_str();
    let mode = ctx.install_mode();
    if mode.is_managed() {
        return Check::info(
            ID,
            TITLE,
            format!(
                "managed-store install (--no-plugin, {} skills, mode from the {}): no plugin package, so no manifest family to check",
                mode.skills.len(),
                mode.source
            ),
            WHY_SILENT,
        );
    }
    let ins = match ctx.inspect() {
        Ok(i) => i,
        Err(InspectFailure::NotInstalled(_)) => {
            return Check::info(
                ID,
                TITLE,
                format!("plugin `{pid}` is not installed (D1 carries the fix)"),
                WHY_SILENT,
            )
        }
        Err(InspectFailure::Other(e)) => {
            return Check::warn(
                ID,
                TITLE,
                format!("could not inspect plugin `{pid}`: {e}"),
                WHY_SILENT,
                FIX,
            )
        }
    };
    let family = ins.manifest_family.clone().unwrap_or_default();
    let mut notes: Vec<String> = Vec::new();
    // §6.5: the installed generation may have been rotated out.
    let source_path = ins
        .raw
        .get("record")
        .and_then(|r| r.get("source"))
        .and_then(|s| s.get("path"))
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from);
    if let Some(p) = &source_path {
        if !p.exists() {
            notes.push(format!(
                "installed source {} is gone (marketplace generation rotated out): the plugin runs from cache and the next `omm update` reinstalls",
                p.display()
            ));
        }
    }
    // §6.5: `list --available` digest ≠ installed package_sha256 = update available.
    let installed_sha = ins
        .raw
        .get("record")
        .and_then(|r| r.get("package_sha256"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let (Some(sha), Ok(out)) = (
        &installed_sha,
        ctx.inv.run(&["plugins", "list", "--available", "--json"]),
    ) {
        if let Ok(v) = out.first_json() {
            if let Some(row) = v
                .get("available")
                .and_then(Value::as_array)
                .and_then(|rows| {
                    rows.iter()
                        .find(|r| r.get("name").and_then(Value::as_str) == Some(pid))
                })
            {
                match row.get("digest").and_then(Value::as_str) {
                    Some(d) if d != sha => notes.push(format!(
                        "update available: marketplace digest {} ≠ installed package_sha256 {} (the host never says so)",
                        short(d),
                        short(sha)
                    )),
                    _ => {}
                }
            }
        }
    }
    // §6.5: the ledger's marketplace registration should still be configured.
    if let LedgerState::Loaded(l) = ctx.ledger() {
        if let Some(name) = l.marketplace_registration().and_then(|r| r.name.clone()) {
            if let Ok(out) = ctx.inv.run(&["plugins", "marketplace", "list", "--json"]) {
                if let Ok(v) = out.first_json() {
                    let present = v
                        .get("marketplaces")
                        .and_then(Value::as_array)
                        .map(|m| {
                            m.iter().any(|x| {
                                x.get("name").and_then(Value::as_str) == Some(name.as_str())
                                    || x.as_str() == Some(name.as_str())
                            })
                        })
                        .unwrap_or(false);
                    if !present {
                        notes.push(format!(
                            "marketplace `{name}` recorded in the ledger is no longer configured (the plugin keeps running from cache; `omm update` will re-add it)"
                        ));
                    }
                }
            }
        }
    }
    let mut observed = format!("plugin `{pid}` manifest_family = {family:?}");
    if !notes.is_empty() {
        observed.push_str(&format!("; {}", notes.join("; ")));
    }
    if family == hr::MANIFEST_FAMILY_NATIVE {
        Check::info(ID, TITLE, observed, WHY_SILENT)
    } else {
        Check::critical(
            ID,
            TITLE,
            format!(
                "{observed} — not {:?}: the package loaded under foreign semantics",
                hr::MANIFEST_FAMILY_NATIVE
            ),
            WHY_SILENT,
            FIX,
        )
    }
}

fn short(s: &str) -> String {
    let t = s.trim_start_matches(hr::MARKETPLACE_DIGEST_PREFIX);
    let mut end = 12;
    while !t.is_char_boundary(end.min(t.len())) {
        end -= 1;
    }
    format!("{}…", &t[..end.min(t.len())])
}
