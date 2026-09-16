//! D4 — shape lint of `settings.plugins` and `settings.runtime_capabilities`,
//! plus the file-level failures the same lint sees first (ARCHITECTURE.md §6 row D4).

use serde_json::Value;

use omm_host::host_reality as hr;

use crate::check::{Check, Context};
use crate::checks::wired_reminders;

pub const ID: &str = "D4";
pub const TITLE: &str = "settings shape lint";
/// `research/experiments/settings-plugins.md` §4.3, §8 rows 22/24 and its
/// verification C3: `plugins` / `runtime_capabilities` are lazy `RawValue` /
/// `Value` members — a non-object root, a non-object entry, or an `enabled`
/// that is not a bool takes BOTH wired reminders down (`reminder enablement
/// settings are invalid`), and a single malformed entry anywhere in
/// `runtime_capabilities` — under a key unrelated to reminders — does the
/// same; the notice is TUI-only, every CLI lane is silent. A wrong TYPE on a
/// typed key makes the whole file malformed (every settings-consuming
/// command exits 1) while `--version`, `--help`, `export`, `init` still
/// succeed (host-reality.md "Exit codes": malformed settings.json is lazy).
pub const WHY_SILENT: &str = "`plugins` and `runtime_capabilities` are lazy members: a non-object root, a non-object entry, or an `enabled` that is not a bool — under ANY key, even one unrelated to reminders — takes both wired reminders down with a TUI-only notice and no CLI symptom (settings-plugins.md §4.3, verification C3); a wrong type on a typed key fails every settings-consuming command with exit 1 while `--version`/`--help`/`export`/`init` still succeed (host-reality.md \"Exit codes\"); unknown top-level and `tui.*` keys are destroyed by the next settings rewrite with no validator reporting them (host-reality.md \"Paths\": settings rewrite)";
pub const FIX: &str = "omm settings lint --fix";
/// The fix of a structural member missing from its object (Gate 1 decision
/// D): `omm settings set <top>.<member> <value>` — the value is the host's
/// only accepted one (`schema_version` must be 1, settings-keys.json).
pub const FIX_STRUCTURAL_PREFIX: &str = "omm settings set";

pub fn run(ctx: &Context) -> Check {
    let doc = match ctx.settings() {
        Ok(d) => d,
        Err(e) => {
            return Check::critical(
                ID,
                TITLE,
                format!("settings.json cannot be loaded: {e}; every settings-consuming muse command exits 1 (`{} …`)", hr::SETTINGS_MALFORMED_MESSAGE),
                WHY_SILENT,
                FIX,
            )
        }
    };
    let mut problems: Vec<String> = Vec::new();
    let wired: Vec<String> = wired_reminders()
        .map(|w| w.into_iter().map(|r| r.plugin_id).collect())
        .unwrap_or_default();

    // settings.plugins — Map<PluginId, {enabled: bool, …}>.
    if let Some(plugins) = doc.get(&["plugins".to_string()]) {
        match plugins.as_object() {
            None => problems.push(format!(
                "`plugins` is {} — must be an object (both wired reminders inactive)",
                kind(plugins)
            )),
            Some(map) => {
                for (id, entry) in map {
                    match entry.as_object() {
                        None => problems.push(format!(
                            "`plugins.{id}` is {} — must be an object{}",
                            kind(entry),
                            if wired.contains(id) {
                                " (a wired reminder id: it goes inactive)"
                            } else {
                                ""
                            }
                        )),
                        Some(obj) => {
                            if let Some(en) = obj.get("enabled") {
                                if !en.is_boolean() {
                                    problems.push(format!(
                                        "`plugins.{id}.enabled` is {} — must be a bool",
                                        kind(en)
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // settings.runtime_capabilities — object of RuntimeCapabilityState.
    if let Some(rc) = doc.get(&["runtime_capabilities".to_string()]) {
        match rc.as_object() {
            None => problems.push(format!(
                "`runtime_capabilities` is {} — must be an object (every approval and both wired reminders inactive)",
                kind(rc)
            )),
            Some(map) => {
                for (key, entry) in map {
                    match entry.as_object() {
                        None => problems.push(format!(
                            "`runtime_capabilities.{key}` is {} — must be a RuntimeCapabilityState object (this one bad entry takes both wired reminders down)",
                            kind(entry)
                        )),
                        Some(obj) => {
                            match obj.get("enabled") {
                                Some(en) if en.is_boolean() => {}
                                Some(en) => problems.push(format!(
                                    "`runtime_capabilities.{key}.enabled` is {} — must be a bool",
                                    kind(en)
                                )),
                                None => problems.push(format!(
                                    "`runtime_capabilities.{key}` has no `enabled`"
                                )),
                            }
                            for hash in ["trusted_definition_hash", "trusted_blocking_definition_hash"] {
                                if let Some(h) = obj.get(hash) {
                                    if !h.is_string() {
                                        problems.push(format!(
                                            "`runtime_capabilities.{key}.{hash}` is {} — must be a string",
                                            kind(h)
                                        ));
                                    }
                                }
                            }
                            if key.split(':').count() != 4 || !key.starts_with("plugin:") {
                                problems.push(format!(
                                    "`runtime_capabilities.{key}` is not a full stable id `plugin:<pid>:<kind>:<cap>` — the host honours only that form"
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // A deny_unknown_fields object missing a required structural member
    // (settings-keys.json `structural`; Gate 1 decision D): the host refuses
    // the whole object on stderr only — `Named permission profiles are
    // unavailable: missing field schema_version` — and every named profile
    // in it is silently inert. The fix restores the one accepted value.
    let mut structural_fix: Option<String> = None;
    if let Ok(keys) = hr::settings_keys() {
        for key in keys.items.iter().filter(|k| k.structural.is_some()) {
            if let Ok(missing) =
                omm_host::settings::missing_structural_members(doc.value(), &key.key)
            {
                for member in missing {
                    problems.push(format!(
                        "`{}` object lacks its required `{member}` — the host refuses the whole object (`Named permission profiles are unavailable: missing field {member}`), so every named profile in it is silently inert",
                        key.key
                    ));
                    if structural_fix.is_none() {
                        structural_fix = Some(format!(
                            "{FIX_STRUCTURAL_PREFIX} {}.{member} {}",
                            key.key,
                            hr::SETTINGS_SCHEMA_VERSION
                        ));
                    }
                }
            }
        }
    }
    // Keys the host's rewrite destroys (host-reality "Paths": settings rewrite).
    let mut doomed: Vec<String> = Vec::new();
    if let Ok(keys) = doc.unknown_top_level_keys() {
        let settings_keys = hr::settings_keys().ok();
        doomed.extend(keys.into_iter().filter(|k| {
            // The legacy MCP spelling is D3's finding.
            settings_keys
                .map(|s| s.canonical_spelling(k).is_none())
                .unwrap_or(true)
        }));
    }
    if let Ok(keys) = doc.unknown_tui_keys() {
        doomed.extend(keys);
    }
    if !doomed.is_empty() {
        problems.push(format!(
            "unknown keys the host destroys on its next settings rewrite: {}",
            doomed.join(", ")
        ));
    }

    if problems.is_empty() {
        let has_plugins = doc.get(&["plugins".to_string()]).is_some();
        let rc_count = doc
            .get(&["runtime_capabilities".to_string()])
            .and_then(Value::as_object)
            .map(|m| m.len())
            .unwrap_or(0);
        Check::info(
            ID,
            TITLE,
            format!(
                "settings.json loads; `plugins` {}; `runtime_capabilities` well-formed ({rc_count} entr{}); no doomed keys",
                if has_plugins { "well-formed" } else { "absent" },
                if rc_count == 1 { "y" } else { "ies" }
            ),
            WHY_SILENT,
        )
    } else {
        Check::warn(
            ID,
            TITLE,
            problems.join("; "),
            WHY_SILENT,
            structural_fix.unwrap_or_else(|| FIX.to_string()),
        )
    }
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a bool",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}
