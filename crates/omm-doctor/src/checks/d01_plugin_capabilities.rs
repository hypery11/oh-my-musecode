//! D1 — every declared `mcp_server` / `hook` / `reminder` line of the plugin
//! is PRESENT in `plugins inspect --json` and literally `trusted_enabled`
//! (R14; ARCHITECTURE.md §6 row D1).

use std::collections::BTreeSet;

use omm_host::host_reality as hr;

use crate::check::{Check, Context, InspectFailure};

pub const ID: &str = "D1";
pub const TITLE: &str = "plugin capabilities";
/// `docs/host-reality.md` "Trust lifecycle": MCP servers, hooks and reminders
/// spawn only in the literal `trusted_enabled` state; `review_needed` (the
/// default after install), `trusted_disabled`, `modified` and an absent line
/// are all skipped silently — no stderr, no session record, no trace line
/// (00-DECISION.md §1.1 row 2); `muse plugins disable` DELETES the capability
/// line instead of marking it; any byte change to the package flips every
/// capability to `modified`. The model-side symptom is only `tool
/// unavailable: unknown tool …`, and the turn continues.
pub const WHY_SILENT: &str = "the host spawns a plugin MCP server, hook or reminder only in the literal `trusted_enabled` state; `review_needed` (the default after install), `trusted_disabled`, `modified` and an absent line are skipped with no stderr, no session record and no trace line; `muse plugins disable` deletes the capability line instead of marking it, and any byte change to the package turns every line `modified` (host-reality.md \"Trust lifecycle\"; 00-DECISION.md §1.1 rows 2–3)";

/// The fix of a managed-store install whose ledgered skills the host no
/// longer lists: the install mode the ledger records (a plain `omm install`
/// refuses a `--no-plugin` ledger).
pub const FIX_MANAGED_STORE: &str = "omm install --no-plugin";

/// A `--no-plugin` install has no plugin capabilities to approve; what can
/// go silent there is a ledgered skill the managed store no longer lists
/// (`muse skills uninstall` by hand, a store rebuilt from its lockfile).
/// With no ledger to list them (corrupt or absent) the mode and the ids
/// come from the host itself (Gate 1 decision E), and nothing is missing
/// by construction.
fn managed_store(ctx: &Context, ids: &[String], source: &str) -> Check {
    let listed = match ctx.user_skills() {
        Ok(list) => list.iter().cloned().collect::<BTreeSet<String>>(),
        Err(e) => {
            return Check::warn(
                ID,
                TITLE,
                format!(
                    "managed-store install ({} skills, mode from the {source}): `muse skills list --source user` failed: {e}",
                    ids.len()
                ),
                WHY_SILENT,
                FIX_MANAGED_STORE,
            )
        }
    };
    let missing: Vec<&str> = ids
        .iter()
        .filter(|id| !listed.contains(id.as_str()))
        .map(String::as_str)
        .collect();
    let observed = if source == "ledger" {
        format!(
            "managed-store install (--no-plugin): {}/{} ledgered skills listed by `skills list --source user`; no plugin capabilities to approve",
            ids.len() - missing.len(),
            ids.len()
        )
    } else {
        format!(
            "managed-store install (--no-plugin, mode derived from the host: no ledger to consult): {} `{}-*` skills listed by `skills list --source user` ({}), no plugin installed; no plugin capabilities to approve",
            ids.len(),
            ctx.plugin_id,
            ids.join(", ")
        )
    };
    if missing.is_empty() {
        Check::info(ID, TITLE, observed, WHY_SILENT)
    } else {
        Check::critical(
            ID,
            TITLE,
            format!("{observed}; MISSING: {}", missing.join(", ")),
            WHY_SILENT,
            FIX_MANAGED_STORE,
        )
    }
}

pub fn run(ctx: &Context) -> Check {
    let pid = ctx.plugin_id.as_str();
    let mode = ctx.install_mode().clone();
    if mode.is_managed() {
        return managed_store(ctx, &mode.skills, mode.source);
    }
    let ins = match ctx.inspect() {
        Ok(i) => i,
        Err(InspectFailure::NotInstalled(msg)) => {
            return Check::critical(
                ID,
                TITLE,
                format!("plugin `{pid}` is not installed (`muse plugins inspect {pid}` → {msg})"),
                WHY_SILENT,
                "omm install",
            )
        }
        Err(InspectFailure::Other(e)) => {
            return Check::warn(
                ID,
                TITLE,
                format!("could not inspect plugin `{pid}`: {e}"),
                WHY_SILENT,
                "omm install",
            )
        }
    };
    let declared = ctx.declared_capabilities();
    // Without a ledger the manifest decides — minus what the author shipped
    // off (`enabledDefault: false`): `plugins approve` would switch it on,
    // and the installer leaves it `review_needed` on purpose.
    let by_design: Vec<String> = declared
        .iter()
        .filter(|d| !d.enabled_default)
        .map(|d| d.stable_id.clone())
        .collect();
    let (expected, source): (Vec<String>, &str) = match ctx.expected_capabilities() {
        Some(ids) => (ids, "ledger"),
        None => (
            declared
                .iter()
                .filter(|d| d.enabled_default)
                .map(|d| d.stable_id.clone())
                .collect(),
            "manifest",
        ),
    };
    let by_design_note = if source == "manifest" && !by_design.is_empty() {
        format!(
            "; {} left review_needed by design (enabledDefault false): {}",
            by_design.len(),
            by_design.join(", ")
        )
    } else {
        String::new()
    };
    if ins.enabled == Some(false) {
        return Check::critical(
            ID,
            TITLE,
            format!(
                "plugin `{pid}` is disabled: `muse plugins disable` deleted every runtime-capability line; {} declared ({}) — none can spawn",
                expected.len(),
                expected.join(", ")
            ),
            WHY_SILENT,
            format!("muse plugins enable {pid}"),
        );
    }
    if expected.is_empty() {
        return Check::info(
            ID,
            TITLE,
            format!("plugin `{pid}` declares no hook, MCP server or reminder to approve ({source}){by_design_note}"),
            WHY_SILENT,
        );
    }
    let mut rows = Vec::new();
    let mut fixes: Vec<String> = Vec::new();
    let mut bad = 0usize;
    for id in &expected {
        match ins.capability(id) {
            None => {
                bad += 1;
                rows.push(format!("{id} ABSENT"));
                fixes.push("omm install --reinstall".to_string());
            }
            Some(c) if c.is_trusted_enabled() => rows.push(format!("{id} {}", c.status)),
            Some(c) => {
                bad += 1;
                let diag = c
                    .diagnostic_code
                    .as_deref()
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                rows.push(format!("{id} {}{diag}", c.status));
                let fix = match c.status.as_str() {
                    "invalid" | "blocked" => "omm install --reinstall".to_string(),
                    _ => format!("muse plugins approve {id}"),
                };
                fixes.push(fix);
            }
        }
    }
    fixes.sort();
    fixes.dedup();
    let observed = format!(
        "{}/{} {} ({source}): {}{by_design_note}",
        expected.len() - bad,
        expected.len(),
        hr::RUNTIME_CAPABILITY_ACTIVE_STATE,
        rows.join("; ")
    );
    if bad == 0 {
        Check::info(ID, TITLE, observed, WHY_SILENT)
    } else {
        Check::critical(ID, TITLE, observed, WHY_SILENT, fixes.join("\n"))
    }
}
