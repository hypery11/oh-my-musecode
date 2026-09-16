//! D1–D15 (ARCHITECTURE.md §6), one module each, run in order over one
//! shared [`Context`]. Every check exists because the condition it detects
//! produces no error, warning or log record when a session runs
//! (`research/experiments/00-DECISION.md` §0); each module's `WHY_SILENT`
//! says why, citing the `docs/host-reality.md` row that measured it.

use std::time::Instant;

use serde::Deserialize;

use omm_host::host_reality as hr;

use crate::check::{stable_id, Check, Context, Report};
use crate::error::{DoctorError, Result};

pub mod d01_plugin_capabilities;
pub mod d02_provider;
pub mod d03_mcp_collision;
pub mod d04_shape_lint;
pub mod d05_reminder_conflict;
pub mod d06_safe_mode;
pub mod d07_plugin_count;
pub mod d08_catalog_pressure;
pub mod d09_enterprise;
pub mod d10_manifest_family;
pub mod d11_host_drift;
pub mod d12_trust;
pub mod d13_ledger_integrity;
pub mod d14_argv_self_test;
pub mod d15_omm_on_path;

/// One registered check.
#[derive(Clone, Copy, Debug)]
pub struct Def {
    pub id: &'static str,
    pub title: &'static str,
    pub run: fn(&Context) -> Check,
}

/// The fifteen checks, in report order.
pub fn registry() -> [Def; 15] {
    macro_rules! def {
        ($m:ident) => {
            Def {
                id: $m::ID,
                title: $m::TITLE,
                run: $m::run,
            }
        };
    }
    [
        def!(d01_plugin_capabilities),
        def!(d02_provider),
        def!(d03_mcp_collision),
        def!(d04_shape_lint),
        def!(d05_reminder_conflict),
        def!(d06_safe_mode),
        def!(d07_plugin_count),
        def!(d08_catalog_pressure),
        def!(d09_enterprise),
        def!(d10_manifest_family),
        def!(d11_host_drift),
        def!(d12_trust),
        def!(d13_ledger_integrity),
        def!(d14_argv_self_test),
        def!(d15_omm_on_path),
    ]
}

/// Run every check and build the report.
pub fn run_all(ctx: &Context) -> Report {
    let started = Instant::now();
    let checks: Vec<Check> = registry().iter().map(|d| (d.run)(ctx)).collect();
    Report::new(ctx.host_info(), checks, started.elapsed().as_millis())
}

/// Run one check by id (`D3`).
pub fn run_one(ctx: &Context, id: &str) -> Option<Check> {
    registry()
        .iter()
        .find(|d| d.id.eq_ignore_ascii_case(id))
        .map(|d| (d.run)(ctx))
}

/// The row every settings-reading check emits when `settings.json` cannot
/// be loaded: D4 carries the finding, the others point at it.
pub(crate) fn settings_unreadable(id: &str, title: &str, why: &str, err: &str) -> Check {
    Check::warn(
        id,
        title,
        format!("not checked: settings.json could not be loaded ({err}); see D4"),
        why,
        d04_shape_lint::FIX,
    )
}

/// The reminder plugin ids whose enablement is routed through
/// `settings.plugins` (reserved-ids.json `items[].wired_in_settings_plugins`)
/// and the stable ids of their `runtime_capabilities` twins
/// (`bundled_reminder_capability_ids`). Read from the data file (R8), never
/// spelled in Rust.
#[derive(Clone, Debug)]
pub struct WiredReminder {
    /// The `settings.plugins` key (`skill-reminder`).
    pub plugin_id: String,
    /// `plugin:tbh-reminders:reminder:<id>`.
    pub stable_id: String,
}

#[derive(Deserialize)]
struct ReservedDoc {
    items: Vec<ReservedItem>,
    bundled_reminder_capability_ids: BundledReminders,
}

#[derive(Deserialize)]
struct ReservedItem {
    id: String,
    #[serde(default)]
    wired_in_settings_plugins: Option<bool>,
}

#[derive(Deserialize)]
struct BundledReminders {
    plugin: String,
}

/// The wired reminder ids (two on the observed build).
pub fn wired_reminders() -> Result<Vec<WiredReminder>> {
    let doc: ReservedDoc =
        serde_json::from_str(hr::RAW_RESERVED_IDS).map_err(|e| DoctorError::Parse {
            what: "reserved-ids.json",
            detail: e.to_string(),
        })?;
    Ok(doc
        .items
        .iter()
        .filter(|i| i.wired_in_settings_plugins == Some(true))
        .map(|i| WiredReminder {
            plugin_id: i.id.clone(),
            stable_id: stable_id(
                &doc.bundled_reminder_capability_ids.plugin,
                "reminder",
                &i.id,
            ),
        })
        .collect())
}

/// `1,234` for report text.
pub(crate) fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_d1_to_d15_in_order() {
        let ids: Vec<&str> = registry().iter().map(|d| d.id).collect();
        let expected: Vec<String> = (1..=15).map(|n| format!("D{n}")).collect();
        assert_eq!(ids, expected);
    }

    #[test]
    fn wired_reminders_come_from_the_data_file() {
        let w = wired_reminders().unwrap();
        let ids: Vec<&str> = w.iter().map(|r| r.plugin_id.as_str()).collect();
        assert_eq!(ids, vec!["skill-reminder", "goal-reminder"]);
        assert_eq!(
            w[0].stable_id,
            "plugin:tbh-reminders:reminder:skill-reminder"
        );
    }

    #[test]
    fn thousands_separators() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(32000), "32,000");
        assert_eq!(thousands(1234567), "1,234,567");
    }
}
