//! `omm theme <name>` / `omm theme list` (ARCHITECTURE.md §7; e2e scenario 7).
//!
//! Host facts (host-reality.md "Paths": themes; research/musecode/
//! tui-slash-theme.md §3.4, PROVEN): custom themes are read from
//! `$XDG_CONFIG_HOME/muse/themes/*.tmTheme` only (case-insensitive
//! extension, the data root is NOT scanned, JSON is ignored), and selecting
//! one writes `tui.theme = "custom:<file stem>"`. So `omm theme <name>`
//! copies the theme file — the user overlay `$OMM/custom/themes/<name>.tmTheme`
//! before the bundled `content/themes/<name>.tmTheme` (R7) — to
//! `<config>/themes/<name>.tmTheme` as a ledgered `copy` entry, then sets
//! `tui.theme` through the validated settings transaction with its prior
//! recorded, so uninstall restores it (scenario 7).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use omm_host::fsx;
use omm_host::settings::{PatchOp, SettingsDoc, MUSE_CONFIG_BASE};
use omm_ledger::audit::{self, Audit, Event};
use omm_ledger::reconcile::{self, Outcome};
use omm_ledger::{hash, Base, Class, Entry, Kind, Mechanism, Observed};
use omm_manifest::catalog::AssetKind;
use omm_manifest::overlay;

use super::settings_tx::{self, Tx};
use super::{
    check_name, modify_ledger, read_ledger, read_regular_file, rel, ContentSource, WriteReport,
};
use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Action, Table};

/// The extension the host accepts (case-insensitively); omm writes this spelling.
pub const THEME_EXT: &str = "tmTheme";
/// `tui.theme` prefix for a file under the themes dir.
pub const CUSTOM_PREFIX: &str = "custom:";
/// The converge category of the theme file.
pub const CATEGORY: &str = "themes";

/// Where a theme file came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    /// `$OMM/custom/themes/<id>.tmTheme` (R7, searched first).
    Custom,
    /// `content/themes/<id>.tmTheme`.
    Bundled,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Provider::Custom => "custom",
            Provider::Bundled => "bundled",
        }
    }
}

/// A resolved theme source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeFile {
    pub id: String,
    pub path: PathBuf,
    pub provider: Provider,
}

/// `custom:<id>` → `<id>`; anything else unchanged.
pub fn normalise(name: &str) -> &str {
    name.strip_prefix(CUSTOM_PREFIX).unwrap_or(name)
}

/// `tui.theme` value for a theme id.
pub fn tui_value(id: &str) -> String {
    format!("{CUSTOM_PREFIX}{id}")
}

/// The file stem of a `*.tmTheme` (case-insensitive), else `None`.
pub fn theme_stem(name: &str) -> Option<&str> {
    let (stem, ext) = name.rsplit_once('.')?;
    (ext.eq_ignore_ascii_case(THEME_EXT) && !stem.is_empty()).then_some(stem)
}

/// Resolve a theme id: the overlay first, then the catalog (by asset id or
/// by file stem).
pub fn resolve(ctx: &Ctx, id: &str) -> Result<ThemeFile> {
    check_name("theme", id)?;
    let custom = overlay::custom_path(&ctx.omm_root(), AssetKind::Theme, id);
    if custom.exists() {
        return Ok(ThemeFile {
            id: id.to_string(),
            path: custom,
            provider: Provider::Custom,
        });
    }
    let source = ContentSource::require(&ctx.omm_root())?;
    let catalog = source.catalog()?;
    let asset = match catalog.resolve(id)? {
        Some(a) if a.kind == AssetKind::Theme => Some(a),
        _ => catalog.shipped_of(AssetKind::Theme).find(|a| {
            Path::new(&a.path)
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(theme_stem)
                == Some(id)
        }),
    };
    let Some(asset) = asset else {
        let known: Vec<String> = catalog
            .shipped_of(AssetKind::Theme)
            .map(|a| a.id.clone())
            .collect();
        return Err(OmmError::Usage(format!(
            "unknown theme `{id}`; bundled: {}; a custom theme goes to {}",
            known.join(" "),
            custom.display()
        )));
    };
    Ok(ThemeFile {
        id: asset.id.clone(),
        path: source.asset_path(&asset.path)?,
        provider: Provider::Bundled,
    })
}

/// `omm theme <name>`.
pub fn apply(ctx: &Ctx, name: &str) -> Result<WriteReport> {
    let id = normalise(name);
    let disabled = overlay::load_config(&ctx.omm_root())?.disabled;
    let qualified = format!("{}:{id}", AssetKind::Theme.singular());
    if disabled.contains(&qualified) {
        return Err(OmmError::Usage(format!(
            "theme `{id}` is disabled in {} (`disabled: [\"{qualified}\"]`)",
            super::OmmConfig::path(&ctx.omm_root()).display()
        )));
    }
    let theme = resolve(ctx, id)?;
    let bytes = read_regular_file(&theme.path)?;
    let theirs = fsx::sha256_bytes(&bytes);
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("theme", json!(theme.id));
    report.field("source", json!(theme.path.display().to_string()));
    report.field("provider", json!(theme.provider.as_str()));

    // 1. The theme file, reconciled against the ledger (R3 outcomes).
    let rel_path = rel(&format!("themes/{}.{THEME_EXT}", theme.id))?;
    let dest = rel_path.under(&ctx.roots.muse_config());
    let ancestor = read_ledger(ctx)?.and_then(|l| {
        l.find(Base::MuseConfig, &rel_path)
            .map(|e| e.sha256.clone())
    });
    let observed = hash::observe(&dest)?;
    let (outcome, reason) = reconcile::decide(ancestor.as_deref(), &theirs, &observed);
    let mut file_action = Action::Unchanged;
    let mut backup: Option<PathBuf> = None;
    match outcome {
        Outcome::NoOp => {
            report.line(format!(
                "theme {}: {} unchanged ({reason})",
                theme.id,
                dest.display()
            ));
        }
        Outcome::Adopt => {
            file_action = Action::Updated;
            report.line(format!(
                "theme {}: {} already holds these bytes; recorded ({reason})",
                theme.id,
                dest.display()
            ));
        }
        Outcome::Overwrite => {
            file_action = Action::Updated;
            if !ctx.dry_run {
                backup = write_theme(ctx, &dest, &bytes, &observed)?;
            }
            report.line(format!(
                "theme {}: {} {} ({} B, {reason})",
                theme.id,
                dest.display(),
                if ctx.dry_run {
                    "would be written"
                } else {
                    "written"
                },
                bytes.len()
            ));
        }
        Outcome::Stage => {
            file_action = Action::Skipped;
            ctx.out.warn(format!(
                "{} is not the file omm wrote ({reason}); left untouched — move it aside to install the {} copy",
                dest.display(),
                theme.provider.as_str()
            ));
            report.line(format!(
                "theme {}: {} skipped ({reason})",
                theme.id,
                dest.display()
            ));
        }
    }
    report.converge.record(CATEGORY, file_action);
    if backup.is_some() {
        report.converge.record(CATEGORY, Action::BackedUp);
    }
    if !ctx.dry_run && matches!(outcome, Outcome::Overwrite | Outcome::Adopt) {
        let before = match &observed {
            Observed::Content(sha) => Some(sha.clone()),
            _ => None,
        };
        let rel_for_ledger = rel_path.clone();
        modify_ledger(ctx, |ledger| {
            ledger.upsert(Entry {
                base: Base::MuseConfig,
                path: rel_for_ledger,
                kind: Kind::Theme,
                sha256: theirs.clone(),
                source_version: OMM_VERSION.to_string(),
                writer: "omm theme".to_string(),
                mechanism: Mechanism::Copy,
                class: Class::Exclusive,
                prior: None,
            });
            Ok(())
        })?;
        Audit::new(&ctx.omm_root(), OMM_VERSION).append(
            &Event::new(audit::ACTION_INSTALL)
                .at(Base::MuseConfig, &rel_path)
                .before(before.as_deref())
                .after(Some(&theirs))
                .note(format!(
                    "omm theme: {} copy of {}{}",
                    theme.provider.as_str(),
                    theme.id,
                    if outcome == Outcome::Adopt {
                        " (adopted, identical bytes on disk)"
                    } else {
                        ""
                    }
                )),
        )?;
    }

    // 2. `tui.theme`, validated first, prior recorded (scenario 7).
    let value = tui_value(&theme.id);
    let tx = settings_tx::apply(
        ctx,
        Tx {
            writer: "omm theme",
            ops: vec![PatchOp::set("tui.theme", Value::String(value.clone()))],
            ..Tx::default()
        },
        &mut report.converge,
    )?;
    match tx.changed.first() {
        Some(c) => report.line(format!(
            "tui.theme = {value:?} (was {}){}",
            super::show(c.prior.as_ref()),
            if ctx.dry_run { " — dry run" } else { "" }
        )),
        None => report.line(format!("tui.theme = {value:?} unchanged")),
    };
    report.field("tui_theme", json!(value));
    report.field("path", json!(dest.display().to_string()));
    if let Some(b) = backup.or(tx.backup) {
        report.field("backup", json!(b.display().to_string()));
    }
    Ok(report)
}

/// Copy the bytes into the themes dir: create it, refuse a destination that
/// escapes it (canonical containment), back up an existing file we own under
/// `$OMM/snapshots/<ts>/muse-config/`, write atomically on the realpath.
fn write_theme(
    ctx: &Ctx,
    dest: &Path,
    bytes: &[u8],
    observed: &Observed,
) -> Result<Option<PathBuf>> {
    let themes_dir = ctx.roots.themes_dir();
    fsx::create_dir_all(&themes_dir)?;
    let dir = fsx::canonicalize(&themes_dir)?;
    let realpath = fsx::realpath_for_write(dest)?;
    if !realpath.starts_with(&dir) {
        return Err(OmmError::Usage(format!(
            "{} resolves outside {} — refusing to write",
            dest.display(),
            dir.display()
        )));
    }
    let mut backup = None;
    if matches!(observed, Observed::Content(_)) {
        backup = Some(fsx::snapshot_backup(
            &realpath,
            &ctx.roots.snapshots_dir(),
            MUSE_CONFIG_BASE,
        )?);
    }
    fsx::write_atomic(&realpath, bytes)?;
    Ok(backup)
}

/// `omm theme list`: bundled (catalog), custom (overlay) and installed
/// (`<config>/themes/`) themes with the active `tui.theme`.
pub fn list(ctx: &Ctx) -> Result<Table> {
    let omm_root = ctx.omm_root();
    let mut rows: Vec<(String, String)> = Vec::new();
    if let Some(source) = ContentSource::locate(&omm_root) {
        ctx.out.note(format!(
            "content: {} ({})",
            source.root.display(),
            source.origin.as_str()
        ));
        let catalog = source.catalog()?;
        for a in catalog.shipped_of(AssetKind::Theme) {
            rows.push((a.id.clone(), Provider::Bundled.as_str().to_string()));
        }
    } else {
        ctx.out
            .warn("no content bundle found; bundled themes not listed");
    }
    for stem in stems_in(
        &omm_root
            .join(overlay::CUSTOM_DIR)
            .join(AssetKind::Theme.plural()),
    ) {
        match rows.iter_mut().find(|(id, _)| *id == stem) {
            Some(row) => row.1 = format!("{} (shadows bundled)", Provider::Custom.as_str()),
            None => rows.push((stem, Provider::Custom.as_str().to_string())),
        }
    }
    let installed = stems_in(&ctx.roots.themes_dir());
    for stem in &installed {
        if !rows.iter().any(|(id, _)| id == stem) {
            rows.push((stem.clone(), "installed only".to_string()));
        }
    }
    let active = match SettingsDoc::for_roots(&ctx.roots) {
        Ok(doc) => doc
            .get(&["tui".to_string(), "theme".to_string()])
            .and_then(Value::as_str)
            .map(str::to_string),
        Err(e) => {
            ctx.out
                .warn(format!("settings.json could not be read: {e}"));
            None
        }
    };
    let mut table = Table::new(["theme", "source", "installed", "active"]);
    for (id, source) in rows {
        let is_active = active.as_deref() == Some(tui_value(&id).as_str());
        table.row([
            id.clone(),
            source,
            if installed.contains(&id) { "yes" } else { "no" }.to_string(),
            if is_active { "*" } else { "" }.to_string(),
        ]);
    }
    Ok(table)
}

/// The `*.tmTheme` stems in a directory (missing dir → empty), sorted.
fn stems_in(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
                .filter_map(|e| {
                    e.file_name()
                        .to_str()
                        .and_then(theme_stem)
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_stems() {
        assert_eq!(normalise("custom:omm-carbon"), "omm-carbon");
        assert_eq!(normalise("omm-carbon"), "omm-carbon");
        assert_eq!(tui_value("omm-carbon"), "custom:omm-carbon");
        assert_eq!(theme_stem("x.tmTheme"), Some("x"));
        assert_eq!(theme_stem("x.TMTHEME"), Some("x"));
        assert_eq!(theme_stem("x.tmtheme"), Some("x"));
        assert_eq!(theme_stem("x.json"), None);
        assert_eq!(theme_stem(".tmTheme"), None);
        assert_eq!(theme_stem("noext"), None);
    }

    #[test]
    fn stems_are_listed_sorted_from_regular_files_only() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("b.tmTheme"), b"x").unwrap();
        std::fs::write(tmp.path().join("a.TMTHEME"), b"x").unwrap();
        std::fs::write(tmp.path().join("c.json"), b"x").unwrap();
        std::fs::create_dir(tmp.path().join("d.tmTheme")).unwrap();
        assert_eq!(stems_in(tmp.path()), vec!["a", "b"]);
        assert!(stems_in(&tmp.path().join("missing")).is_empty());
    }
}
