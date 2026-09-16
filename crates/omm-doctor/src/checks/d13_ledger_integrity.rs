//! D13 — ledger integrity (ARCHITECTURE.md §6 row D13, §4): a corrupt or
//! missing ledger, ledgered files that vanished, files present under our
//! roots but unlisted, and registrations that no longer match the host.

use std::path::Path;

use serde_json::Value;

use crate::check::{Check, Context, InspectFailure};
use crate::ledger::{self, Ledger, LedgerState, BASE_MUSE_CONFIG};

pub const ID: &str = "D13";
pub const TITLE: &str = "ledger integrity";
/// ARCHITECTURE.md R1/R2: the ledger is the only record of what omm wrote —
/// the host keeps no such record, `plugins list --json → warning: null`
/// throughout (host-reality.md "Marketplaces": update signal), and a skill
/// directory or theme file that omm never ledgered is indistinguishable from
/// a user's own to every host command. Without the ledger, uninstall cannot
/// be byte-identical (R5) and update cannot three-way merge (R3).
pub const WHY_SILENT: &str = "the host records nothing about what omm installed: a ledgered file that vanished, a file under our roots that the ledger does not list, or a registration the host no longer holds are invisible to every muse command, and without an intact ledger uninstall cannot be byte-identical (R5) nor update merge safely (R3) (ARCHITECTURE.md §4, R1–R2; host-reality.md \"Marketplaces\": update signal)";
/// `omm reconcile` re-reads the ledger against the disk and the host (drops
/// what vanished, adopts what the host holds); `omm install` puts back what
/// the ledger lost (adopts an identical file, recreates a missing one). The
/// fix names both, in that order, whenever a rebuilt ledger would still
/// leave files unlisted (Gate 1: after a quarantine `omm reconcile` alone
/// left "3 present but unlisted" with the same no-op fix). The install is
/// spelled in the mode the ledger records: a `--no-plugin` ledger
/// (`muse-skills-install` entries, no `muse-plugin` registration) makes a
/// plain `omm install` refuse, so that fix would not run (Gate 1).
pub const FIX: &str = "omm reconcile";
/// See [`FIX`].
pub const FIX_RECONCILE_THEN_INSTALL: &str = crate::check::FIX_RECONCILE_THEN_INSTALL;
/// See [`FIX`].
pub const FIX_INSTALL: &str = crate::check::FIX_INSTALL;
/// [`FIX_RECONCILE_THEN_INSTALL`] for a managed-store (`--no-plugin`)
/// install — the mode read from the ledger, or from the host when the
/// ledger is corrupt or absent (Gate 1 decision E).
pub const FIX_RECONCILE_THEN_INSTALL_NO_PLUGIN: &str =
    crate::check::FIX_RECONCILE_THEN_INSTALL_NO_PLUGIN;
/// [`FIX_INSTALL`] for a managed-store (`--no-plugin`) install.
pub const FIX_INSTALL_NO_PLUGIN: &str = crate::check::FIX_INSTALL_NO_PLUGIN;

pub fn run(ctx: &Context) -> Check {
    let installed = ctx.inspect().is_ok();
    let path = ctx.ledger_path();
    let mode = ctx.install_mode().clone();
    let ledger = match ctx.ledger() {
        LedgerState::Absent(_) => {
            return if installed {
                Check::warn(
                    ID,
                    TITLE,
                    format!(
                        "no ledger at {} but plugin `{}` is installed: nothing records what omm wrote",
                        path.display(),
                        ctx.plugin_id
                    ),
                    WHY_SILENT,
                    mode.reconcile_then_install_fix(),
                )
            } else if mode.is_managed() {
                Check::warn(
                    ID,
                    TITLE,
                    format!(
                        "no ledger at {} but the managed store holds {} `{}*` skill(s) ({}) and no plugin is installed (a --no-plugin install, judged from the host): nothing records what omm wrote",
                        path.display(),
                        mode.skills.len(),
                        crate::check::MANAGED_ID_PREFIX,
                        mode.skills.join(", ")
                    ),
                    WHY_SILENT,
                    mode.reconcile_then_install_fix(),
                )
            } else {
                Check::info(
                    ID,
                    TITLE,
                    format!(
                        "no ledger at {} and plugin `{}` not installed (omm never installed here)",
                        path.display(),
                        ctx.plugin_id
                    ),
                    WHY_SILENT,
                )
            }
        }
        LedgerState::Corrupt { detail, .. } => {
            return Check::critical(
                ID,
                TITLE,
                format!(
                    "ledger {} is corrupt: {detail}; install mode judged from the host: {}",
                    path.display(),
                    match mode.mode {
                        crate::check::InstallMode::Bundle => "the plugin bundle".to_string(),
                        crate::check::InstallMode::ManagedStore => format!(
                            "managed store ({} `{}*` skills, no plugin)",
                            mode.skills.len(),
                            crate::check::MANAGED_ID_PREFIX
                        ),
                        crate::check::InstallMode::None => "nothing installed".to_string(),
                    }
                ),
                WHY_SILENT,
                mode.reconcile_then_install_fix(),
            )
        }
        LedgerState::Loaded(l) => l,
    };

    let ws = ctx.workspace();
    let mut vanished = Vec::new();
    let mut escaped = Vec::new();
    let mut counted = 0usize;
    for e in ledger.file_entries() {
        counted += 1;
        let Some(root) = ledger::base_root(&ctx.roots, ws.as_deref(), &e.base) else {
            escaped.push(format!("{}:{} (unknown base)", e.base, e.path));
            continue;
        };
        match ledger::contained_path(&root, &e.path) {
            None => escaped.push(format!("{}:{}", e.base, e.path)),
            Some(p) if !p.exists() => vanished.push(format!("{}:{}", e.base, e.path)),
            Some(_) => {}
        }
    }

    // Files under our roots that look like ours but are not ledgered.
    // Content ids, not the plugin id: the managed store holds `omm-*`
    // whatever the plugin is named.
    let prefix = crate::check::MANAGED_ID_PREFIX;
    let mut unlisted = Vec::new();
    for dir_entry in read_dir_names(&ctx.roots.personal_skills_dir()) {
        if dir_entry.starts_with(prefix) {
            let rel = format!("skills/{dir_entry}/SKILL.md");
            if !ledger.has(BASE_MUSE_CONFIG, &rel)
                && !ledger.has(BASE_MUSE_CONFIG, &format!("skills/{dir_entry}"))
            {
                unlisted.push(format!("{BASE_MUSE_CONFIG}:{rel}"));
            }
        }
    }
    for file in read_dir_names(&ctx.roots.themes_dir()) {
        if file.starts_with(prefix) {
            let rel = format!("themes/{file}");
            if !ledger.has(BASE_MUSE_CONFIG, &rel) {
                unlisted.push(format!("{BASE_MUSE_CONFIG}:{rel}"));
            }
        }
    }
    // A personal rules file carrying omm's managed block with no ledger
    // entry — an interrupt after the write but before the ledger save left
    // omm's block the host's active personal rules that no uninstall knows
    // about (Gate 1 round 5 M2). The marker is a structural literal, not an
    // asset name (R8: `host_reality::RULES_MANAGED_START_MARKER`).
    let rules_file = ctx.roots.personal_rules_file();
    if let Ok(text) = std::fs::read_to_string(&rules_file) {
        let rel = rules_file
            .strip_prefix(ctx.roots.muse_config())
            .ok()
            .map(|r| r.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|| "AGENTS.md".to_string());
        if text.contains(omm_host::host_reality::RULES_MANAGED_START_MARKER)
            && !ledger.has(BASE_MUSE_CONFIG, &rel)
        {
            unlisted.push(format!("{BASE_MUSE_CONFIG}:{rel}"));
        }
    }

    // Registrations vs the host.
    let mut drift = Vec::new();
    let mut needs_reconcile = !escaped.is_empty() || !vanished.is_empty();
    let mut needs_install = !vanished.is_empty() || !unlisted.is_empty();
    let registered = ledger.plugin_registration(&ctx.plugin_id).is_some();
    match (registered, ctx.inspect()) {
        (true, Err(InspectFailure::NotInstalled(_))) => {
            needs_install = true;
            drift.push(format!(
                "ledger registers plugin `{}` but the host has no such plugin",
                ctx.plugin_id
            ))
        }
        (false, Ok(_)) => {
            needs_reconcile = true;
            drift.push(format!(
                "plugin `{}` is installed but the ledger has no muse-plugin registration for it",
                ctx.plugin_id
            ))
        }
        _ => {}
    }
    let managed = mode.is_managed();
    let fix = match (needs_reconcile, needs_install, managed) {
        (true, true, false) => FIX_RECONCILE_THEN_INSTALL,
        (true, true, true) => FIX_RECONCILE_THEN_INSTALL_NO_PLUGIN,
        (false, true, false) => FIX_INSTALL,
        (false, true, true) => FIX_INSTALL_NO_PLUGIN,
        _ => FIX,
    };
    // Current but never registered (a ledger rebuilt from the host knows no
    // prior for them): uninstall leaves them and names them.
    let unregistered = unregistered(ctx, ledger);

    let mut problems = Vec::new();
    if !escaped.is_empty() {
        problems.push(format!(
            "{} entr{} not contained in their base: {}",
            escaped.len(),
            if escaped.len() == 1 { "y" } else { "ies" },
            escaped.join(", ")
        ));
    }
    if !vanished.is_empty() {
        problems.push(format!(
            "{} ledgered file(s) vanished: {}",
            vanished.len(),
            vanished.join(", ")
        ));
    }
    if !unlisted.is_empty() {
        problems.push(format!(
            "{} present but unlisted: {}",
            unlisted.len(),
            unlisted.join(", ")
        ));
    }
    problems.extend(drift);

    let summary = format!(
        "ledger {} (omm {}): {} entr{} ({counted} file{}), {} registration(s)",
        path.display(),
        ledger.omm_version,
        ledger.entries.len(),
        if ledger.entries.len() == 1 {
            "y"
        } else {
            "ies"
        },
        if counted == 1 { "" } else { "s" },
        ledger.registrations.len()
    );
    let unregistered_note = if unregistered.is_empty() {
        String::new()
    } else {
        format!(
            "; current but unregistered (prior unknown; `omm uninstall` leaves and names {}): {}",
            if unregistered.len() == 1 {
                "it"
            } else {
                "them"
            },
            unregistered.join(", ")
        )
    };
    if problems.is_empty() {
        Check::info(
            ID,
            TITLE,
            format!("{summary}; every file present and contained{unregistered_note}"),
            WHY_SILENT,
        )
    } else if !escaped.is_empty() {
        Check::critical(
            ID,
            TITLE,
            format!("{summary}; {}{unregistered_note}", problems.join("; ")),
            WHY_SILENT,
            fix,
        )
    } else {
        Check::warn(
            ID,
            TITLE,
            format!("{summary}; {}{unregistered_note}", problems.join("; ")),
            WHY_SILENT,
            fix,
        )
    }
}

/// Settings keys a shipped profile sets, holding the profile's value with
/// no `settings-key` registration, and the workspace's trust entry with no
/// `trust` registration. The profiles are read as plain JSON from the
/// checkout the ledger's marketplace registration names
/// (`<source>/content/profiles/*.json`; R8: never spelled here); without
/// one only the trust entry is checked.
fn unregistered(ctx: &Context, ledger: &Ledger) -> Vec<String> {
    let mut out = Vec::new();
    let profiles_dir = ledger
        .marketplace_registration()
        .and_then(|r| r.source.clone())
        .map(|s| Path::new(&s).join("content").join("profiles"));
    if let (Some(dir), Ok(doc)) = (profiles_dir, ctx.settings()) {
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        files.sort();
        for file in files
            .iter()
            .filter(|f| f.extension().is_some_and(|x| x == "json"))
        {
            let Some(slice) = std::fs::read(file)
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            else {
                continue;
            };
            let stem = file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("?")
                .to_string();
            let mut leaves = Vec::new();
            flatten(&[], &slice, &mut leaves);
            for (path, value) in leaves {
                let key = path.join(".");
                if ledger.settings_key(&key).is_some() {
                    continue;
                }
                if doc.get(&path) == Some(&value)
                    && !out
                        .iter()
                        .any(|o: &String| o.starts_with(&format!("{key} ")))
                {
                    out.push(format!("{key} (profile {stem})"));
                }
            }
        }
    }
    if let Some(ws) = ctx.workspace() {
        if let Ok(store) = omm_host::trust::TrustStore::load(&ctx.roots.trust_file()) {
            if let (Ok(key), Ok(Some(decision))) = (
                omm_host::trust::TrustStore::key_for(&ws),
                store.decision_for(&ws),
            ) {
                let registered = ledger.registrations.iter().any(|r| {
                    r.kind == crate::ledger::REGISTRATION_TRUST
                        && r.project.as_deref() == Some(&key)
                });
                if !registered {
                    out.push(format!("trust {key} ({decision})"));
                }
            }
        }
    }
    out
}

/// Every leaf of a profile slice as a key path + value (objects recurse;
/// scalars, arrays and empty objects are leaves).
fn flatten(prefix: &[String], v: &Value, out: &mut Vec<(Vec<String>, Value)>) {
    match v.as_object() {
        Some(m) if !m.is_empty() => {
            for (k, child) in m {
                let mut p = prefix.to_vec();
                p.push(k.clone());
                flatten(&p, child, out);
            }
        }
        _ => out.push((prefix.to_vec(), v.clone())),
    }
}

fn read_dir_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| e.file_name().to_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}
