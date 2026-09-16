//! `omm settings {set,fix-mcp-collision,lint [--fix],reconcile-reminders}` —
//! the fixes doctor D2–D6 print (ARCHITECTURE.md §6), each a targeted,
//! validated `settings.json` patch with its prior in the ledger (R9, R10).
//!
//! Host facts: `mcpServers` + `mcp_servers` both present makes every
//! settings-MUTATING host command exit 1 while sessions stay silent, and the
//! legacy spelling alone is destroyed by the host's next settings rewrite
//! (host-reality.md "Exit codes", "Paths": settings rewrite — D3);
//! `plugins` / `runtime_capabilities` are lazy members where one malformed
//! entry takes both wired reminders down with a TUI-only notice
//! (research/experiments/settings-plugins.md §4.3 — D4); when a wired
//! reminder's legacy `runtime_capabilities` line and its canonical
//! `plugins.<id>.enabled` disagree the reminder is inactive with a TUI-only
//! notice, and the legacy line alone decides (settings-plugins.md §4.4, all
//! nine cells — D5), so `reconcile-reminders` makes the canonical store
//! follow the legacy one.

use serde_json::{json, Map, Value};

use omm_host::fsx;
use omm_host::host_reality as hr;
use omm_host::settings::{PatchOp, SettingsDoc};
use omm_ledger::audit::{self, Audit, Event};
use omm_ledger::Base;

use super::settings_tx::{self, Changed, Tx, CATEGORY, SETTINGS_REL};
use super::{json_kind, parse_value, rel, show, WriteReport};
use crate::cmd::Ctx;
use crate::error::{OmmError, Result};
use crate::output::{Action, Render, Table};

/// `omm settings set <key> <value>` (D2: `provider meta`; D6:
/// `agent_definitions.safe_mode false`).
pub fn set(ctx: &Ctx, key: &str, raw: &str) -> Result<WriteReport> {
    let path: Vec<String> = key
        .split('.')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if path.is_empty() {
        return Err(OmmError::Usage("settings set: the key is empty".into()));
    }
    let value = parse_value(raw);
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("key", json!(path.join(".")));
    report.field("value", value.clone());
    let tx = settings_tx::apply(
        ctx,
        Tx {
            writer: "omm settings set",
            ops: vec![PatchOp::set_path(path.clone(), value.clone())],
            ..Tx::default()
        },
        &mut report.converge,
    )?;
    describe(&mut report, &tx.changed, &tx.unchanged, ctx.dry_run);
    if let Some(b) = &tx.backup {
        report.field("backup", json!(b.display().to_string()));
    }
    Ok(report)
}

/// The per-key lines every settings action prints.
fn describe(report: &mut WriteReport, changed: &[Changed], unchanged: &[String], dry_run: bool) {
    for c in changed {
        report.line(format!(
            "{} = {} (was {}){}",
            c.key,
            show(c.value.as_ref()),
            show(c.prior.as_ref()),
            if dry_run { " — dry run" } else { "" }
        ));
    }
    for k in unchanged {
        report.line(format!("{k} unchanged"));
    }
    report.field(
        "changed",
        Value::Array(changed.iter().map(|c| json!(c.key)).collect()),
    );
}

// ---- D3 -------------------------------------------------------------------

/// The merge `fix-mcp-collision` lands, computed without touching disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum McpFix {
    /// Neither spelling, or the canonical one alone: nothing to do.
    Nothing,
    /// The legacy spelling alone: renamed to `mcpServers`.
    Rename(Map<String, Value>),
    /// Both present: the union, when no server id differs between them.
    Merge(Map<String, Value>),
}

/// Decide the fix for a document. Both spellings must be objects; a server
/// id defined differently under each is refused (omm never guesses which
/// definition the user meant).
pub fn plan_mcp_fix(doc: &Value) -> Result<McpFix> {
    let canonical = doc.get("mcpServers");
    let legacy = doc.get("mcp_servers");
    let as_map = |name: &str, v: &Value| -> Result<Map<String, Value>> {
        v.as_object().cloned().ok_or_else(|| {
            OmmError::Usage(format!(
                "`{name}` is {}, not an object; fix it by hand first",
                json_kind(v)
            ))
        })
    };
    match (canonical, legacy) {
        (_, None) => Ok(McpFix::Nothing),
        (None, Some(l)) => Ok(McpFix::Rename(as_map("mcp_servers", l)?)),
        (Some(c), Some(l)) => {
            let mut merged = as_map("mcpServers", c)?;
            let legacy = as_map("mcp_servers", l)?;
            let mut conflicts = Vec::new();
            for (id, def) in legacy {
                match merged.get(&id) {
                    Some(existing) if existing != &def => conflicts.push(id),
                    Some(_) => {}
                    None => {
                        merged.insert(id, def);
                    }
                }
            }
            if !conflicts.is_empty() {
                return Err(OmmError::Usage(format!(
                    "server{} {} defined differently under `mcpServers` and `mcp_servers`; keep one definition by hand, then rerun",
                    if conflicts.len() == 1 { "" } else { "s" },
                    conflicts.join(", ")
                )));
            }
            Ok(McpFix::Merge(merged))
        }
    }
}

/// `omm settings fix-mcp-collision`: fold `mcp_servers` into `mcpServers`.
/// The typed patch refuses the legacy spelling by design, so the document is
/// replaced whole through `settings_tx::replace_document` (same validators,
/// lock, backup and atomic rename). The ledger records `mcpServers` with its
/// prior; the legacy map is not registered because a restore through the
/// typed patch would be refused too — the host's next rewrite would have
/// destroyed it anyway, and the backup under `$OMM/snapshots/` keeps it.
pub fn fix_mcp_collision(ctx: &Ctx) -> Result<WriteReport> {
    let doc = SettingsDoc::for_roots(&ctx.roots)?;
    let loaded_sha = if doc.existed() {
        Some(fsx::sha256_file(doc.path())?)
    } else {
        None
    };
    let mut report = WriteReport::new(ctx.dry_run);
    let (merged, label) = match plan_mcp_fix(doc.value())? {
        McpFix::Nothing => {
            report.line("no `mcp_servers` key: nothing to fix");
            return Ok(report);
        }
        McpFix::Rename(m) => (m, "renamed `mcp_servers` to `mcpServers`"),
        McpFix::Merge(m) => (m, "folded `mcp_servers` into `mcpServers`"),
    };
    let Some(current) = doc.value().as_object() else {
        return Err(OmmError::Usage("settings.json is not an object".into()));
    };
    let prior = current.get("mcpServers").cloned();
    let mut candidate = current.clone();
    candidate.remove("mcp_servers");
    candidate.insert("mcpServers".to_string(), Value::Object(merged.clone()));

    let changed = vec![Changed {
        key: "mcpServers".to_string(),
        prior,
        value: Some(Value::Object(merged.clone())),
    }];
    if !ctx.dry_run {
        // Ledger first (see `settings_tx::apply`): the prior stays restorable
        // even if the replacement below fails half-way.
        let mut candidate_bytes = serde_json::to_vec_pretty(&Value::Object(candidate.clone()))
            .map_err(|e| OmmError::Usage(format!("settings candidate: {e}")))?;
        candidate_bytes.push(b'\n');
        let found = if doc.existed() {
            omm_ledger::shared::Original::read(doc.path())?
        } else {
            None
        };
        settings_tx::ledger_settings_write(
            ctx,
            "omm settings fix-mcp-collision",
            !doc.existed(),
            &fsx::sha256_bytes(&candidate_bytes),
            &changed,
            &[],
            None,
            found.as_ref(),
        )?;
    }
    let replaced = settings_tx::replace_document(ctx, &doc, loaded_sha.as_deref(), &candidate)?;
    report.line(format!(
        "{}: {label} ({} server{}){}",
        replaced.path.display(),
        merged.len(),
        if merged.len() == 1 { "" } else { "s" },
        if replaced.written { "" } else { " — dry run" }
    ));
    report.converge.record(CATEGORY, Action::Updated);
    report.converge.record(CATEGORY, Action::Removed);
    if replaced.backup.is_some() {
        report.converge.record(CATEGORY, Action::BackedUp);
    }
    report.field("servers", json!(merged.keys().cloned().collect::<Vec<_>>()));
    if let Some(b) = &replaced.backup {
        report.field("backup", json!(b.display().to_string()));
    }
    if ctx.dry_run {
        return Ok(report);
    }
    Audit::new(&ctx.omm_root(), crate::cmd::OMM_VERSION).append(
        &Event::new(audit::ACTION_UPDATE)
            .at(Base::MuseConfig, &rel(SETTINGS_REL)?)
            .before(loaded_sha.as_deref())
            .after(Some(&replaced.sha256))
            .note(format!("omm settings fix-mcp-collision: {label}")),
    )?;
    Ok(report)
}

// ---- D4 -------------------------------------------------------------------

/// One shape problem `settings lint` found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub path: String,
    pub problem: String,
    /// What `--fix` does about it (`None` = report only).
    pub fix: Option<String>,
}

/// The lint result: findings, the typed patches `--fix` applies, and the
/// keys the host will destroy that omm only reports (a removal through the
/// typed patch is refused for unknown keys, and their loss is the host's
/// own doing — move them out by hand).
#[derive(Clone, Debug, Default)]
pub struct Lint {
    pub findings: Vec<Finding>,
    pub fixes: Vec<PatchOp>,
    pub doomed: Vec<String>,
}

impl Lint {
    /// Findings `--fix` cannot repair.
    pub fn unfixable(&self) -> usize {
        self.findings.iter().filter(|f| f.fix.is_none()).count()
    }
}

/// `true`/`false` strings and 0/1 numbers become bools; anything else `None`.
fn coerce_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        Value::Number(n) => match n.as_u64() {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
        _ => None,
    }
}

/// The D4 rules over a document (pure): `plugins` and `runtime_capabilities`
/// shapes, plus the doomed unknown keys.
pub fn lint_document(doc: &SettingsDoc) -> Result<Lint> {
    let mut lint = Lint::default();
    let plugins_key = ["plugins".to_string()];
    if let Some(plugins) = doc.get(&plugins_key) {
        match plugins.as_object() {
            None => {
                lint.findings.push(Finding {
                    path: "plugins".into(),
                    problem: format!(
                        "is {} — must be an object (both wired reminders inactive)",
                        json_kind(plugins)
                    ),
                    fix: Some("remove the key".into()),
                });
                lint.fixes.push(PatchOp::remove("plugins"));
            }
            Some(map) => {
                let mut fixed = map.clone();
                let mut dirty = false;
                for (id, entry) in map {
                    match entry.as_object() {
                        None => {
                            lint.findings.push(Finding {
                                path: format!("plugins.{id}"),
                                problem: format!("is {} — must be an object", json_kind(entry)),
                                fix: Some("remove the entry".into()),
                            });
                            fixed.remove(id);
                            dirty = true;
                        }
                        Some(obj) => {
                            if let Some(en) = obj.get("enabled").filter(|e| !e.is_boolean()) {
                                let coerced = coerce_bool(en);
                                lint.findings.push(Finding {
                                    path: format!("plugins.{id}.enabled"),
                                    problem: format!("is {} — must be a bool", json_kind(en)),
                                    fix: Some(match coerced {
                                        Some(b) => format!("set to {b}"),
                                        None => "remove the field".into(),
                                    }),
                                });
                                if let Some(Value::Object(o)) = fixed.get_mut(id) {
                                    match coerced {
                                        Some(b) => {
                                            o.insert("enabled".into(), Value::Bool(b));
                                        }
                                        None => {
                                            o.remove("enabled");
                                        }
                                    }
                                }
                                dirty = true;
                            }
                        }
                    }
                }
                if dirty {
                    lint.fixes
                        .push(PatchOp::set("plugins", Value::Object(fixed)));
                }
            }
        }
    }
    let rc_key = ["runtime_capabilities".to_string()];
    if let Some(rc) = doc.get(&rc_key) {
        match rc.as_object() {
            None => {
                lint.findings.push(Finding {
                    path: "runtime_capabilities".into(),
                    problem: format!(
                        "is {} — must be an object (every approval and both wired reminders inactive)",
                        json_kind(rc)
                    ),
                    fix: Some("remove the key".into()),
                });
                lint.fixes.push(PatchOp::remove("runtime_capabilities"));
            }
            Some(map) => {
                let mut fixed = map.clone();
                let mut dirty = false;
                for (key, entry) in map {
                    let Some(obj) = entry.as_object() else {
                        lint.findings.push(Finding {
                            path: format!("runtime_capabilities.{key}"),
                            problem: format!(
                                "is {} — must be a RuntimeCapabilityState object (this one entry takes both wired reminders down)",
                                json_kind(entry)
                            ),
                            fix: Some("remove the entry".into()),
                        });
                        fixed.remove(key);
                        dirty = true;
                        continue;
                    };
                    if key.split(':').count() != 4 || !key.starts_with("plugin:") {
                        lint.findings.push(Finding {
                            path: format!("runtime_capabilities.{key}"),
                            problem: "is not a full stable id `plugin:<pid>:<kind>:<cap>` — the host honours only that form".into(),
                            fix: Some("remove the entry".into()),
                        });
                        fixed.remove(key);
                        dirty = true;
                        continue;
                    }
                    let mut entry_fixed = obj.clone();
                    let mut drop_entry = false;
                    match obj.get("enabled") {
                        Some(en) if en.is_boolean() => {}
                        Some(en) => {
                            let coerced = coerce_bool(en);
                            lint.findings.push(Finding {
                                path: format!("runtime_capabilities.{key}.enabled"),
                                problem: format!("is {} — must be a bool", json_kind(en)),
                                fix: Some(match coerced {
                                    Some(b) => format!("set to {b}"),
                                    None => "remove the entry".into(),
                                }),
                            });
                            match coerced {
                                Some(b) => {
                                    entry_fixed.insert("enabled".into(), Value::Bool(b));
                                }
                                None => drop_entry = true,
                            }
                        }
                        None => {
                            lint.findings.push(Finding {
                                path: format!("runtime_capabilities.{key}"),
                                problem: "has no `enabled`".into(),
                                fix: Some("remove the entry".into()),
                            });
                            drop_entry = true;
                        }
                    }
                    for hash in [
                        "trusted_definition_hash",
                        "trusted_blocking_definition_hash",
                    ] {
                        if let Some(h) = obj.get(hash).filter(|h| !h.is_string()) {
                            lint.findings.push(Finding {
                                path: format!("runtime_capabilities.{key}.{hash}"),
                                problem: format!("is {} — must be a string", json_kind(h)),
                                fix: Some("remove the field".into()),
                            });
                            entry_fixed.remove(hash);
                        }
                    }
                    if drop_entry {
                        fixed.remove(key);
                        dirty = true;
                    } else if &entry_fixed != obj {
                        fixed.insert(key.clone(), Value::Object(entry_fixed));
                        dirty = true;
                    }
                }
                if dirty {
                    lint.fixes
                        .push(PatchOp::set("runtime_capabilities", Value::Object(fixed)));
                }
            }
        }
    }
    let keys = hr::settings_keys()?;
    for k in doc.unknown_top_level_keys()? {
        // The legacy MCP spelling is D3's finding.
        if keys.canonical_spelling(&k).is_none() {
            lint.doomed.push(k);
        }
    }
    lint.doomed.extend(doc.unknown_tui_keys()?);
    Ok(lint)
}

/// The lint table.
pub fn lint_table(lint: &Lint) -> Table {
    let mut table = Table::new(["path", "problem", "fix"]);
    for f in &lint.findings {
        table.row([
            f.path.clone(),
            f.problem.clone(),
            f.fix.clone().unwrap_or_else(|| "(report only)".into()),
        ]);
    }
    table
}

/// `omm settings lint [--fix]`. Returns the report and whether problems
/// remain (the exit code).
pub fn lint(ctx: &Ctx, fix: bool) -> Result<(WriteReport, bool)> {
    let doc = SettingsDoc::for_roots(&ctx.roots)?;
    let lint = lint_document(&doc)?;
    let mut report = WriteReport::new(ctx.dry_run);
    report.field(
        "findings",
        Value::Array(
            lint.findings
                .iter()
                .map(|f| json!({"path": f.path, "problem": f.problem, "fix": f.fix}))
                .collect(),
        ),
    );
    report.field("doomed", json!(lint.doomed));
    for k in &lint.doomed {
        ctx.out.warn(format!(
            "`{k}` is unknown to the host and will be destroyed by its next settings rewrite; move it out of settings.json by hand (no validator reports it)"
        ));
    }
    if lint.findings.is_empty() {
        report.line(format!(
            "{}: `plugins` and `runtime_capabilities` well-formed",
            doc.path().display()
        ));
        return Ok((report, false));
    }
    report.line(lint_table(&lint).render());
    if !fix {
        report.line(format!(
            "{} finding{}; run `omm settings lint --fix` to repair",
            lint.findings.len(),
            if lint.findings.len() == 1 { "" } else { "s" }
        ));
        return Ok((report, true));
    }
    let tx = settings_tx::apply(
        ctx,
        Tx {
            writer: "omm settings lint --fix",
            ops: lint.fixes.clone(),
            ..Tx::default()
        },
        &mut report.converge,
    )?;
    describe(&mut report, &tx.changed, &tx.unchanged, ctx.dry_run);
    if let Some(b) = &tx.backup {
        report.field("backup", json!(b.display().to_string()));
    }
    Ok((report, lint.unfixable() > 0))
}

// ---- D5 -------------------------------------------------------------------

/// One wired reminder whose two stores disagree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReminderConflict {
    pub plugin_id: String,
    pub stable_id: String,
    pub legacy: bool,
    pub canonical: bool,
}

/// The D5 rule over a document (pure): the canonical store follows the legacy
/// one, because the legacy line alone decides (settings-plugins.md §4.4).
pub fn reminder_conflicts(doc: &Value) -> Result<Vec<ReminderConflict>> {
    let wired = omm_doctor::checks::wired_reminders()?;
    let plugins = doc.get("plugins");
    let legacy = doc.get("runtime_capabilities");
    let mut out = Vec::new();
    for r in wired {
        let canonical = plugins
            .and_then(|p| p.get(&r.plugin_id))
            .and_then(|e| e.get("enabled"))
            .and_then(Value::as_bool);
        let legacy_enabled = legacy
            .and_then(|l| l.get(&r.stable_id))
            .and_then(|e| e.get("enabled"))
            .and_then(Value::as_bool);
        if let (Some(l), Some(c)) = (legacy_enabled, canonical) {
            if l != c {
                out.push(ReminderConflict {
                    plugin_id: r.plugin_id,
                    stable_id: r.stable_id,
                    legacy: l,
                    canonical: c,
                });
            }
        }
    }
    Ok(out)
}

/// `omm settings reconcile-reminders`.
pub fn reconcile_reminders(ctx: &Ctx) -> Result<WriteReport> {
    let doc = SettingsDoc::for_roots(&ctx.roots)?;
    let conflicts = reminder_conflicts(doc.value())?;
    let mut report = WriteReport::new(ctx.dry_run);
    report.field(
        "conflicts",
        Value::Array(
            conflicts
                .iter()
                .map(|c| json!({"plugin_id": c.plugin_id, "stable_id": c.stable_id, "legacy": c.legacy, "canonical": c.canonical}))
                .collect(),
        ),
    );
    if conflicts.is_empty() {
        report.line("wired reminders agree across both stores: nothing to do");
        return Ok(report);
    }
    let ops: Vec<PatchOp> = conflicts
        .iter()
        .map(|c| {
            PatchOp::set_path(
                vec![
                    "plugins".to_string(),
                    c.plugin_id.clone(),
                    "enabled".to_string(),
                ],
                Value::Bool(c.legacy),
            )
        })
        .collect();
    for c in &conflicts {
        report.line(format!(
            "{}: legacy `{}` enabled={} decides; canonical plugins.{}.enabled {} → {}",
            c.plugin_id, c.stable_id, c.legacy, c.plugin_id, c.canonical, c.legacy
        ));
    }
    let tx = settings_tx::apply(
        ctx,
        Tx {
            writer: "omm settings reconcile-reminders",
            ops,
            ..Tx::default()
        },
        &mut report.converge,
    )?;
    describe(&mut report, &tx.changed, &tx.unchanged, ctx.dry_run);
    if let Some(b) = &tx.backup {
        report.field("backup", json!(b.display().to_string()));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(v: Value) -> SettingsDoc {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("settings.json");
        std::fs::write(&file, serde_json::to_vec(&v).unwrap()).unwrap();
        SettingsDoc::load(&file).unwrap()
    }

    #[test]
    fn mcp_fix_plans_rename_merge_and_refuses_conflicts() {
        assert_eq!(
            plan_mcp_fix(&json!({"schema_version": 1})).unwrap(),
            McpFix::Nothing
        );
        assert_eq!(
            plan_mcp_fix(&json!({"mcpServers": {"a": {}}})).unwrap(),
            McpFix::Nothing
        );
        match plan_mcp_fix(&json!({"mcp_servers": {"a": {"command": "x"}}})).unwrap() {
            McpFix::Rename(m) => assert_eq!(m["a"]["command"], "x"),
            other => panic!("{other:?}"),
        }
        match plan_mcp_fix(&json!({"mcpServers": {"a": {"command": "x"}}, "mcp_servers": {"b": {"command": "y"}, "a": {"command": "x"}}})).unwrap() {
            McpFix::Merge(m) => {
                assert_eq!(m.len(), 2);
                assert_eq!(m["b"]["command"], "y");
            }
            other => panic!("{other:?}"),
        }
        let err = plan_mcp_fix(
            &json!({"mcpServers": {"a": {"command": "x"}}, "mcp_servers": {"a": {"command": "z"}}}),
        )
        .unwrap_err();
        assert!(err.to_string().contains("defined differently"), "{err}");
        assert!(plan_mcp_fix(&json!({"mcpServers": 5, "mcp_servers": {}})).is_err());
    }

    #[test]
    fn lint_finds_and_fixes_the_d4_shapes() {
        let d = doc(json!({
            "schema_version": 1,
            "plugins": {"ok": {"enabled": true}, "bad": 5, "str": {"enabled": "false"}, "junk": {"enabled": "maybe"}},
            "runtime_capabilities": {
                "plugin:omm:hook:x": {"enabled": true, "trusted_definition_hash": "h"},
                "plugin:omm:hook:y": {"enabled": 1},
                "plugin:omm:hook:z": {"trusted_definition_hash": 3},
                "short:id": {"enabled": true},
                "plugin:omm:hook:w": []
            },
            "_omm": 1,
            "tui": {"theme": "x", "bogus": 1}
        }));
        let lint = lint_document(&d).unwrap();
        let paths: Vec<&str> = lint.findings.iter().map(|f| f.path.as_str()).collect();
        assert!(paths.contains(&"plugins.bad"));
        assert!(paths.contains(&"plugins.str.enabled"));
        assert!(paths.contains(&"plugins.junk.enabled"));
        assert!(paths.contains(&"runtime_capabilities.plugin:omm:hook:y.enabled"));
        assert!(paths.contains(&"runtime_capabilities.plugin:omm:hook:z"));
        assert!(paths.contains(&"runtime_capabilities.short:id"));
        assert!(paths.contains(&"runtime_capabilities.plugin:omm:hook:w"));
        assert_eq!(lint.doomed, vec!["_omm", "tui.bogus"]);
        assert_eq!(lint.unfixable(), 0);
        assert_eq!(lint.fixes.len(), 2);
        let plugins = lint.fixes[0].value.clone().unwrap();
        assert_eq!(
            plugins,
            json!({"ok": {"enabled": true}, "str": {"enabled": false}, "junk": {}})
        );
        let rc = lint.fixes[1].value.clone().unwrap();
        assert_eq!(
            rc,
            json!({"plugin:omm:hook:x": {"enabled": true, "trusted_definition_hash": "h"}, "plugin:omm:hook:y": {"enabled": true}})
        );
        let clean = doc(json!({"schema_version": 1, "plugins": {"a": {"enabled": false}}}));
        let lint = lint_document(&clean).unwrap();
        assert!(lint.findings.is_empty() && lint.fixes.is_empty() && lint.doomed.is_empty());
        let roots_bad =
            doc(json!({"schema_version": 1, "plugins": [], "runtime_capabilities": "x"}));
        let lint = lint_document(&roots_bad).unwrap();
        assert_eq!(lint.fixes.len(), 2);
        assert!(lint.fixes.iter().all(|f| f.value.is_none()));
    }

    #[test]
    fn reminder_conflicts_follow_the_legacy_line() {
        let wired = omm_doctor::checks::wired_reminders().unwrap();
        assert!(!wired.is_empty());
        let r = &wired[0];
        let v = json!({
            "plugins": {&r.plugin_id: {"enabled": true}},
            "runtime_capabilities": {&r.stable_id: {"enabled": false}}
        });
        let c = reminder_conflicts(&v).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].plugin_id, r.plugin_id);
        assert!(!c[0].legacy && c[0].canonical);
        // Canonical alone is a no-op; agreeing stores are fine.
        assert!(
            reminder_conflicts(&json!({"plugins": {&r.plugin_id: {"enabled": false}}}))
                .unwrap()
                .is_empty()
        );
        assert!(reminder_conflicts(&json!({
            "plugins": {&r.plugin_id: {"enabled": true}},
            "runtime_capabilities": {&r.stable_id: {"enabled": true}}
        }))
        .unwrap()
        .is_empty());
    }

    #[test]
    fn bools_coerce_conservatively() {
        assert_eq!(coerce_bool(&json!("true")), Some(true));
        assert_eq!(coerce_bool(&json!(" False ")), Some(false));
        assert_eq!(coerce_bool(&json!(1)), Some(true));
        assert_eq!(coerce_bool(&json!(0)), Some(false));
        assert_eq!(coerce_bool(&json!(2)), None);
        assert_eq!(coerce_bool(&json!("yes")), None);
        assert_eq!(coerce_bool(&json!(null)), None);
    }
}
