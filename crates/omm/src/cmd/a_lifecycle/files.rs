//! The files both install paths write into the host's config root, each a
//! ledger entry (ARCHITECTURE.md §2): the personal rules file (seeded from
//! the template, user region preserved — [`super::rules`]), the themes
//! (`$CONFIG_DIR/themes/<name>.tmTheme`, host-reality.md "Paths": themes),
//! the default profile's typed `settings.json` keys (R9/R10, each with its
//! prior in a registration) and the workspace's `trust.json` entry
//! (merged, never rewritten; host-reality.md "Paths": trust).
//!
//! Two invariants every step here keeps (Gate 1): the ledger is saved right
//! after each write it records — before the audit line, so a line's
//! presence implies the ledger's — and a shared file that existed before
//! omm's first write keeps its exact bytes and mode in its entry
//! (`omm_ledger::shared`), recorded at the start of `omm install`, before
//! the host's own rewrites of `settings.json`.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use omm_host::fsx;
use omm_host::settings::{CommitOptions, PatchOp, SettingsDoc};
use omm_host::trust::{TrustDecision, TrustStore};
use omm_host::Invoker;
use omm_ledger::audit::{Event, ACTION_INSTALL, ACTION_UPDATE};
use omm_ledger::containment::State;
use omm_ledger::reconcile::{self, ApplyOptions, Candidate, Outcome, Staged};
use omm_ledger::shared::{self, Original};
use omm_ledger::uninstall;
use omm_ledger::{hash, Base, Class, Entry, Kind, Mechanism, Observed, RelPath};

use super::rules;
use super::session::Session;
use super::source::Source;
use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Action, Converge};

pub const CAT_RULES: &str = "rules";
pub const CAT_THEMES: &str = "themes";
pub const CAT_SETTINGS: &str = "settings";
pub const CAT_TRUST: &str = "trust";

/// `OMM_THEMES_DIR=<dir>`: where `omm install` places the themes instead of
/// `$CONFIG_DIR/themes` — a way out when a dotfile manager symlinks that
/// directory out of the config root (Gate 1 decision A). The directory must
/// lie inside the config root (R4: the four bases; a theme is ledgered
/// base-relative), and the host's theme picker reads only
/// `$CONFIG_DIR/themes` (host-reality.md "Paths": themes), so a theme
/// placed elsewhere is staged for the user, not active.
pub const THEMES_DIR_ENV: &str = "OMM_THEMES_DIR";

/// The directory the themes go to: [`THEMES_DIR_ENV`] when set (absolute
/// or relative to the cwd; refused unless it lies under the config root),
/// else `Roots::themes_dir`.
pub fn themes_dir(ctx: &Ctx) -> Result<PathBuf> {
    let Some(raw) = std::env::var_os(THEMES_DIR_ENV).filter(|v| !v.is_empty()) else {
        return Ok(ctx.roots.themes_dir());
    };
    let given = PathBuf::from(raw);
    let abs = if given.is_absolute() {
        given
    } else {
        std::env::current_dir()
            .map_err(|e| OmmError::io("current directory", e))?
            .join(given)
    };
    let config = ctx.roots.muse_config();
    let inside = abs
        .strip_prefix(&config)
        .map(|r| !r.as_os_str().is_empty())
        .unwrap_or(false)
        && !abs
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir));
    if !inside {
        return Err(OmmError::Usage(format!(
            "{THEMES_DIR_ENV}={} is not a directory inside the host's config root {} (R4: omm writes only under its four bases, and a theme is ledgered relative to that root); unset it, or point it at a directory under the root",
            abs.display(),
            config.display()
        )));
    }
    Ok(abs)
}

/// The `prior` key on a rules entry holding the sha of the managed region
/// omm wrote — how update tells "the user edited only the user region"
/// from "the user edited what omm owns" without the ancestor bytes
/// (`omm_ledger::uninstall::RULES_PRIOR_MANAGED_SHA`, read back there).
pub const PRIOR_MANAGED_SHA: &str = uninstall::RULES_PRIOR_MANAGED_SHA;
/// The `prior` key recording an unledgered file the seed replaced and
/// folded into the user region — its sha, its backup and its exact bytes
/// (`omm_ledger::uninstall::replaced_prior`), so uninstall puts it back
/// byte for byte (Gate 1: the snapshot backup alone rolled away with
/// `$OMM/snapshots`, and the file was unlinked).
pub const PRIOR_REPLACED: &str = uninstall::RULES_PRIOR_REPLACED;

/// Install (converge; a missing ledgered file is recreated) or update (R3;
/// a missing ledgered file is staged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Install,
    Update,
}

impl Mode {
    fn writer(self) -> &'static str {
        match self {
            Mode::Install => "omm install",
            Mode::Update => "omm update",
        }
    }
    fn action(self) -> &'static str {
        match self {
            Mode::Install => ACTION_INSTALL,
            Mode::Update => ACTION_UPDATE,
        }
    }
}

/// A base-relative path under the config root for one of the host's files.
pub(super) fn config_rel(ctx: &Ctx, abs: &Path) -> Result<RelPath> {
    let rel = abs.strip_prefix(ctx.roots.muse_config()).map_err(|_| {
        OmmError::Usage(format!(
            "{} is not under the config root {}",
            abs.display(),
            ctx.roots.muse_config().display()
        ))
    })?;
    Ok(RelPath::new(omm_manifest::posix(rel))?)
}

// ---------------------------------------------------------------------------
// rules
// ---------------------------------------------------------------------------

/// What the rules step did.
#[derive(Clone, Debug)]
pub struct RulesOutcome {
    pub path: PathBuf,
    pub action: Action,
    pub note: String,
    pub backup: Option<PathBuf>,
    pub staged: Option<Staged>,
}

impl RulesOutcome {
    pub fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "action": self.action.key(),
            "note": self.note,
            "backup": self.backup,
            "staged_to": self.staged.as_ref().map(|s| s.staged_to.clone()),
        })
    }
    pub fn line(&self) -> String {
        format!(
            "rules {}: {} — {}{}",
            self.path.display(),
            self.action.label(),
            self.note,
            self.backup
                .as_ref()
                .map(|b| format!(" (prior backed up to {})", b.display()))
                .unwrap_or_default()
        )
    }
}

fn write_rules(session: &Session, resolved_path: &Path, bytes: &[u8]) -> Result<()> {
    if session.dry_run {
        return Ok(());
    }
    if let Some(parent) = resolved_path.parent() {
        fsx::create_dir_all(parent)?;
    }
    let realpath = fsx::realpath_for_write(resolved_path)?;
    fsx::write_atomic(&realpath, bytes)?;
    Ok(())
}

fn stage(
    session: &Session,
    version: &str,
    rel: &RelPath,
    bytes: &[u8],
    reason: &str,
    ancestor: Option<&str>,
    mine: Option<&str>,
) -> Result<Staged> {
    reconcile::check_version_label(version)?;
    let dest = reconcile::staging_path(&session.omm_root, version, Base::MuseConfig, rel);
    if !session.dry_run {
        if let Some(parent) = dest.parent() {
            fsx::create_dir_all(parent)?;
        }
        let realpath = fsx::realpath_for_write(&dest)?;
        fsx::write_atomic(&realpath, bytes)?;
    }
    Ok(Staged {
        base: Base::MuseConfig,
        path: rel.clone(),
        ancestor: ancestor.map(str::to_string),
        theirs: hash::sha256_bytes(bytes),
        mine: mine.map(str::to_string),
        staged_to: dest,
        reason: reason.to_string(),
    })
}

/// Seed or refresh `$CONFIG_DIR/AGENTS.md` (see the module docs and
/// [`super::rules`]).
pub fn converge_rules(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    mode: Mode,
    version: &str,
    converge: &mut Converge,
) -> Result<RulesOutcome> {
    let abs = ctx.roots.personal_rules_file();
    let Some(asset) = source.content.rules.first() else {
        converge.record(CAT_RULES, Action::Skipped);
        return Ok(RulesOutcome {
            path: abs,
            action: Action::Skipped,
            note: "the source ships no rules template".into(),
            backup: None,
            staged: None,
        });
    };
    let template = String::from_utf8_lossy(&asset.file.read()?).into_owned();
    let new_managed = rules::managed_sha256(&template).ok_or_else(|| {
        OmmError::Manifest(omm_manifest::ManifestError::Content {
            path: asset.file.abs.clone(),
            detail: "rules template lacks the managed/user markers (`omm lint` rules-markers)"
                .into(),
        })
    })?;
    let rel = config_rel(ctx, &abs)?;
    let resolved = session.probe(Base::MuseConfig, &rel)?;
    let entry = session
        .ledger
        .as_ref()
        .and_then(|l| l.find(Base::MuseConfig, &rel))
        .cloned();
    let mine_bytes: Option<Vec<u8>> = match resolved.state {
        State::File => Some(fsx::read_bytes(&resolved.path)?),
        State::Missing => None,
        State::Dir | State::Symlink | State::Other => {
            converge.record(CAT_RULES, Action::Skipped);
            return Ok(RulesOutcome {
                path: resolved.path,
                action: Action::Skipped,
                note: format!(
                    "{:?} at the path — not a file omm wrote (R2 sentinel); left alone",
                    resolved.state
                ),
                backup: None,
                staged: None,
            });
        }
    };
    let mine: Option<String> = mine_bytes
        .as_deref()
        .map(|b| String::from_utf8_lossy(b).into_owned());
    // The managed block omm owns (round 5 decision H2: omm owns ONLY this),
    // and the template's own frame — its text outside the block.
    let new_managed_region = rules::split(&template).map(|r| r.managed).ok_or_else(|| {
        OmmError::Manifest(omm_manifest::ManifestError::Content {
            path: asset.file.abs.clone(),
            detail: "rules template lacks the user markers".into(),
        })
    })?;
    let template_frame = rules::parts(&template)
        .map(|p| p.frame())
        .unwrap_or_default();
    // A file omm has an entry for carries the frame and the replaced bytes
    // it was written with; a fresh install computes them.
    let mut backup = None;
    let mut replaced: Value = entry
        .as_ref()
        .and_then(|e| e.prior.as_ref())
        .and_then(|p| p.get(PRIOR_REPLACED))
        .cloned()
        .unwrap_or(Value::Null);
    let mut frame: Value = entry
        .as_ref()
        .and_then(|e| e.prior.as_ref())
        .and_then(|p| p.get(uninstall::RULES_PRIOR_FRAME))
        .cloned()
        .unwrap_or(Value::Null);
    let (action, note, bytes, staged): (Action, String, Option<String>, Option<Staged>) =
        match (&entry, &mine) {
            (None, None) => {
                // Clean install: the template is the seed; its whole text is
                // omm's, recorded as the frame so uninstall can take it away.
                frame = template_frame.to_json();
                (
                    Action::Updated,
                    "written from the template".into(),
                    Some(template.clone()),
                    None,
                )
            }
            (None, Some(text)) => {
                let mine_sha = hash::sha256_bytes(text.as_bytes());
                if rules::parts(text).is_some() {
                    // The file already carries the managed markers (a seed of
                    // an earlier omm whose ledger is gone — round 5: the
                    // corrupt-ledger recovery — or an interrupted install's
                    // write): replace the block in place, keep every other
                    // byte. The frame recorded is the template's: uninstall
                    // strips a piece outside the block only while it still
                    // equals the template's piece (`Markers::without_managed`),
                    // so a seed left exactly as omm wrote it goes whole and
                    // anything the user put around the block stays.
                    let refreshed = rules::replace_managed(text, &new_managed_region)
                        .unwrap_or_else(|| text.clone());
                    frame = template_frame.to_json();
                    if refreshed == *text {
                        (
                            Action::Unchanged,
                            "an identical managed block was already there; adopted".into(),
                            Some(refreshed),
                            None,
                        )
                    } else {
                        (
                            Action::Updated,
                            "the managed block was refreshed in place; every other byte kept"
                                .into(),
                            Some(refreshed),
                            None,
                        )
                    }
                } else {
                    // No markers: insert the block at the top, the user's
                    // file kept below byte for byte (its bytes recorded so
                    // uninstall restores it exactly).
                    let original = mine_bytes.as_deref().unwrap_or_default();
                    if !session.dry_run {
                        let b = fsx::snapshot_backup(
                            &resolved.path,
                            &session.snapshots_dir(),
                            Base::MuseConfig.as_str(),
                        )?;
                        replaced = uninstall::replaced_prior(&mine_sha, Some(&b), original);
                        backup = Some(b);
                    } else {
                        replaced = uninstall::replaced_prior(&mine_sha, None, original);
                    }
                    frame = rules::inserted_frame().to_json();
                    converge.record(CAT_RULES, Action::BackedUp);
                    (
                        Action::Updated,
                        "the managed block was inserted at the top; your file is kept below".into(),
                        Some(rules::insert_managed(text, &new_managed_region)),
                        None,
                    )
                }
            }
            (Some(_), None) => match mode {
                Mode::Install => {
                    frame = template_frame.to_json();
                    replaced = Value::Null;
                    (
                        Action::Updated,
                        "missing on disk; recreated from the template".into(),
                        Some(template.clone()),
                        None,
                    )
                }
                Mode::Update => {
                    let e = entry.as_ref().map(|e| e.sha256.clone());
                    let s = stage(
                        session,
                        version,
                        &rel,
                        template.as_bytes(),
                        "missing on disk",
                        e.as_deref(),
                        None,
                    )?;
                    (
                        Action::Skipped,
                        "missing on disk; the new file is staged".into(),
                        None,
                        Some(s),
                    )
                }
            },
            (Some(e), Some(text)) => {
                let mine_sha = hash::sha256_bytes(text.as_bytes());
                let recorded_managed = e
                    .prior
                    .as_ref()
                    .and_then(|p| p.get(PRIOR_MANAGED_SHA))
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let mine_managed = rules::split(text)
                    .map(|r| hash::sha256_bytes(r.managed.as_bytes()))
                    .or_else(|| {
                        rules::parts(text).map(|p| hash::sha256_bytes(p.managed.as_bytes()))
                    });
                // Refresh the managed block in place, keeping every byte the
                // user wrote outside it (round 5: the H2 win).
                let refreshed = rules::replace_managed(text, &new_managed_region);
                let refreshed_sha = refreshed.as_ref().map(|r| hash::sha256_bytes(r.as_bytes()));
                if mine_sha == e.sha256 {
                    match refreshed {
                        Some(r) if hash::sha256_bytes(r.as_bytes()) == mine_sha => {
                            (Action::Unchanged, "current".into(), None, None)
                        }
                        Some(r) => (
                            Action::Updated,
                            "managed block refreshed (file untouched since omm wrote it)".into(),
                            Some(r),
                            None,
                        ),
                        None => (Action::Unchanged, "current".into(), None, None),
                    }
                } else if refreshed_sha.as_deref() == Some(mine_sha.as_str()) {
                    (
                        Action::Unchanged,
                        "adopted: the file already carries the new managed block".into(),
                        refreshed,
                        None,
                    )
                } else if mine_managed.is_some() && mine_managed == recorded_managed {
                    // The block is still omm's; the user edited outside it.
                    (
                        Action::Updated,
                        "managed block refreshed; your edits outside it kept".into(),
                        refreshed,
                        None,
                    )
                } else {
                    let reason = if mine_managed.is_none() {
                        "the markers were removed; the file is no longer the one omm wrote"
                    } else {
                        "the managed block was edited by hand"
                    };
                    let staged_bytes = refreshed.clone().unwrap_or_else(|| template.clone());
                    let s = stage(
                        session,
                        version,
                        &rel,
                        staged_bytes.as_bytes(),
                        reason,
                        Some(&e.sha256),
                        Some(&mine_sha),
                    )?;
                    (
                        Action::Skipped,
                        format!("{reason}; the refreshed file is staged"),
                        None,
                        Some(s),
                    )
                }
            }
        };
    converge.record(CAT_RULES, action);
    if let Some(text) = &bytes {
        let sha = hash::sha256_bytes(text.as_bytes());
        let before = entry
            .as_ref()
            .map(|e| e.sha256.clone())
            .or_else(|| mine.as_ref().map(|m| hash::sha256_bytes(m.as_bytes())));
        let frame_struct = rules::Frame::from_json(&frame);
        // Ledgered and saved BEFORE the write (Gate 1 round 5 M2: a Ctrl-C
        // between the write and a later save left a full AGENTS.md carrying
        // omm's managed block with no ledger entry — after uninstall it
        // stayed the host's active personal rules). The entry records the
        // bytes we are about to write; a kill after the save but before the
        // write leaves the file as it was (its sha then differs from the
        // entry — uninstall preserves it), which is harmless.
        let ledger = session.ledger_or_new(inv)?;
        ledger.upsert(Entry {
            base: Base::MuseConfig,
            path: rel.clone(),
            kind: Kind::Rules,
            sha256: sha.clone(),
            source_version: source.version.clone(),
            writer: mode.writer().to_string(),
            mechanism: Mechanism::Copy,
            class: Class::Seeded,
            prior: Some(uninstall::rules_prior(
                &new_managed,
                Some(replaced),
                frame_struct.as_ref(),
            )),
        });
        session.save()?;
        if action == Action::Updated {
            write_rules(session, &resolved.path, text.as_bytes())?;
        }
        session.record(
            &Event::new(mode.action())
                .at(Base::MuseConfig, &rel)
                .before(before.as_deref())
                .after(Some(&sha))
                .note(format!("{}: {note}", action.label())),
        )?;
    } else if let Some(s) = &staged {
        session.record(
            &Event::new(mode.action())
                .at(Base::MuseConfig, &rel)
                .before(s.ancestor.as_deref())
                .after(s.mine.as_deref())
                .note(format!("stage -> {}: {}", s.staged_to.display(), s.reason)),
        )?;
    }
    session.save()?;
    Ok(RulesOutcome {
        path: resolved.path,
        action,
        note,
        backup,
        staged,
    })
}

// ---------------------------------------------------------------------------
// themes
// ---------------------------------------------------------------------------

/// What the themes step did.
#[derive(Clone, Debug, Default)]
pub struct ThemesOutcome {
    pub written: Vec<String>,
    pub unchanged: Vec<String>,
    /// `(path, why)` — left alone on install (a file omm did not write is in the way).
    pub skipped: Vec<(String, String)>,
    pub staged: Vec<Staged>,
    /// The whole step was left out (`--skip themes`): the flag, for the report.
    pub step_skipped: Option<String>,
}

impl ThemesOutcome {
    /// The outcome of a step `--skip` left out.
    pub fn skipped_step(flag: &str) -> ThemesOutcome {
        ThemesOutcome {
            step_skipped: Some(flag.to_string()),
            ..ThemesOutcome::default()
        }
    }
    pub fn to_json(&self) -> Value {
        json!({
            "written": self.written,
            "unchanged": self.unchanged,
            "skipped": self.skipped.iter().map(|(p, w)| json!({"path": p, "why": w})).collect::<Vec<_>>(),
            "staged": self.staged.iter().map(|s| json!({"path": s.path.as_str(), "staged_to": s.staged_to, "reason": s.reason})).collect::<Vec<_>>(),
            "step_skipped": self.step_skipped,
        })
    }
    pub fn line(&self) -> String {
        if let Some(flag) = &self.step_skipped {
            return format!("themes: step skipped ({flag})");
        }
        let mut s = format!(
            "themes: {} written, {} unchanged, {} skipped, {} staged",
            self.written.len(),
            self.unchanged.len(),
            self.skipped.len(),
            self.staged.len()
        );
        for (p, w) in &self.skipped {
            s.push_str(&format!("\n  skipped {p}: {w}"));
        }
        for st in &self.staged {
            s.push_str(&format!(
                "\n  staged {} -> {} ({})",
                st.path,
                st.staged_to.display(),
                st.reason
            ));
        }
        s
    }
}

/// The theme files an install would write: `(base-relative path, bytes)`,
/// for the install plan (Gate 1 decision A).
pub fn plan_themes(ctx: &Ctx, source: &Source) -> Result<Vec<(RelPath, u64)>> {
    Ok(theme_candidates(ctx, source)?
        .into_iter()
        .map(|c| (c.path, c.bytes.len() as u64))
        .collect())
}

/// The rules file an install would write, for the install plan.
pub fn plan_rules(ctx: &Ctx, source: &Source) -> Result<Option<(RelPath, u64)>> {
    let Some(asset) = source.content.rules.first() else {
        return Ok(None);
    };
    let bytes = asset.file.read()?.len() as u64;
    Ok(Some((
        config_rel(ctx, &ctx.roots.personal_rules_file())?,
        bytes,
    )))
}

fn theme_candidates(ctx: &Ctx, source: &Source) -> Result<Vec<Candidate>> {
    let dir = themes_dir(ctx)?;
    let mut out = Vec::new();
    for t in &source.content.themes {
        let name = Path::new(&t.file.rel)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t.asset.id.clone());
        out.push(Candidate {
            base: Base::MuseConfig,
            path: config_rel(ctx, &dir.join(name))?,
            kind: Kind::Theme,
            mechanism: Mechanism::Copy,
            class: Class::Exclusive,
            source_version: source.version.clone(),
            bytes: t.file.read()?,
        });
    }
    Ok(out)
}

/// Install the themes: the pure R3 decision per file, written by omm with
/// `install` audit lines; a conflict is skipped (install never stages).
pub fn install_themes(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    converge: &mut Converge,
) -> Result<ThemesOutcome> {
    let mut out = ThemesOutcome::default();
    for c in theme_candidates(ctx, source)? {
        let theirs = c.sha256();
        let resolved = session.probe(c.base, &c.path)?;
        if let Some(link) = &resolved.via_symlink {
            // An ancestor inside the base is a symlink: omm never writes
            // through one (what lies behind it is the user's — R2 sentinel,
            // Gate 1 decision C), and uninstall would preserve the entry.
            converge.record(CAT_THEMES, Action::Skipped);
            out.skipped.push((
                resolved.path.display().to_string(),
                format!(
                    "an ancestor ({}) is a symlink — omm never writes through one (R2 sentinel); left alone",
                    link.display()
                ),
            ));
            continue;
        }
        let ancestor = session
            .ledger
            .as_ref()
            .and_then(|l| l.find(c.base, &c.path))
            .map(|e| e.sha256.clone());
        let mine = match resolved.state {
            State::Missing => Observed::Missing,
            State::File => hash::observe(&resolved.path)?,
            State::Dir => Observed::NonRegular(hash::sentinel("directory", c.path.as_str())),
            State::Symlink => Observed::NonRegular(hash::sentinel("symlink", c.path.as_str())),
            State::Other => Observed::NonRegular(hash::sentinel("other", c.path.as_str())),
        };
        let (outcome, reason) = reconcile::decide(ancestor.as_deref(), &theirs, &mine);
        let label = resolved.path.display().to_string();
        match outcome {
            Outcome::NoOp => {
                converge.record(CAT_THEMES, Action::Unchanged);
                out.unchanged.push(label);
            }
            Outcome::Stage => {
                converge.record(CAT_THEMES, Action::Skipped);
                out.skipped.push((
                    label,
                    format!("{reason}; not overwritten (`omm update` stages it)"),
                ));
            }
            Outcome::Overwrite | Outcome::Adopt => {
                // Ledgered and saved BEFORE the write (Gate 1 round 5 M2: a
                // Ctrl-C between the theme write and a later save left an
                // unledgered theme file that survived uninstall). A kill
                // after the save but before the write leaves the file
                // absent — uninstall then finds a missing entry, harmless.
                let ledger = session.ledger_or_new(inv)?;
                ledger.upsert(Entry {
                    base: c.base,
                    path: c.path.clone(),
                    kind: c.kind,
                    sha256: theirs.clone(),
                    source_version: c.source_version.clone(),
                    writer: "omm install".into(),
                    mechanism: c.mechanism,
                    class: c.class,
                    prior: None,
                });
                session.save()?;
                if outcome == Outcome::Overwrite {
                    if !session.dry_run {
                        if let Some(parent) = resolved.path.parent() {
                            fsx::create_dir_all(parent)?;
                        }
                        let realpath = fsx::realpath_for_write(&resolved.path)?;
                        fsx::write_atomic(&realpath, &c.bytes)?;
                    }
                    converge.record(CAT_THEMES, Action::Updated);
                    out.written.push(label);
                } else {
                    converge.record(CAT_THEMES, Action::Unchanged);
                    out.unchanged.push(label);
                }
                session.record(
                    &Event::new(ACTION_INSTALL)
                        .at(c.base, &c.path)
                        .before(ancestor.as_deref())
                        .after(Some(&theirs))
                        .note(format!("{}: {reason}", outcome.as_str())),
                )?;
            }
        }
    }
    session.save()?;
    Ok(out)
}

/// Update the themes through the R3 engine: overwrite / adopt / stage with
/// `updates/<version>/report.json` for the conflicts.
pub fn update_themes(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    version: &str,
    converge: &mut Converge,
) -> Result<ThemesOutcome> {
    let candidates = theme_candidates(ctx, source)?;
    let bases = session.bases.clone();
    let opts = ApplyOptions {
        omm_root: session.omm_root.clone(),
        writer: "omm update".into(),
        omm_version: OMM_VERSION.into(),
        snapshot: false,
        dry_run: session.dry_run,
    };
    let ledger = session.ledger_or_new(inv)?;
    let plan = reconcile::plan(ledger, &bases, version, candidates)?;
    let report = reconcile::apply(&plan, ledger, &bases, &opts)?;
    let mut out = ThemesOutcome {
        written: report.overwritten.iter().map(|k| k.to_string()).collect(),
        unchanged: report.adopted.iter().map(|k| k.to_string()).collect(),
        skipped: Vec::new(),
        staged: report.staged,
        step_skipped: None,
    };
    out.unchanged.extend(
        plan.decisions
            .iter()
            .filter(|d| d.outcome == Outcome::NoOp)
            .map(|d| d.key.to_string()),
    );
    converge.record_n(CAT_THEMES, Action::Updated, report.overwritten.len());
    converge.record_n(
        CAT_THEMES,
        Action::Unchanged,
        report.adopted.len() + report.noop,
    );
    converge.record_n(CAT_THEMES, Action::Skipped, out.staged.len());
    session.save()?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// profile (settings)
// ---------------------------------------------------------------------------

/// What the profile step did.
#[derive(Clone, Debug, Default)]
pub struct ProfileOutcome {
    pub profile: String,
    pub set: Vec<String>,
    pub unchanged: Vec<String>,
    pub backup: Option<PathBuf>,
    pub seeded: bool,
}

impl ProfileOutcome {
    pub fn to_json(&self) -> Value {
        json!({"profile": self.profile, "set": self.set, "unchanged": self.unchanged, "backup": self.backup, "seeded": self.seeded})
    }
    pub fn line(&self) -> String {
        format!(
            "settings: profile {} — {} key(s) set (prior recorded), {} already current{}{}",
            self.profile,
            self.set.len(),
            self.unchanged.len(),
            if self.seeded {
                "; settings.json created"
            } else {
                ""
            },
            self.backup
                .as_ref()
                .map(|b| format!("; backup {}", b.display()))
                .unwrap_or_default()
        )
    }
}

/// Every leaf of a settings slice as a key path + value (objects recurse;
/// arrays, scalars and empty objects are leaves).
pub fn leaves(prefix: &[String], v: &Value, out: &mut Vec<(Vec<String>, Value)>) {
    match v.as_object() {
        Some(m) if !m.is_empty() => {
            for (k, child) in m {
                let mut p = prefix.to_vec();
                p.push(k.clone());
                leaves(&p, child, out);
            }
        }
        _ => out.push((prefix.to_vec(), v.clone())),
    }
}

/// Apply the profile's slice as targeted typed patches (R9/R10), one
/// registration per key with its prior value.
///
/// `settings_existed_before` says whether `settings.json` was there when
/// the command started: the host creates it on `plugins approve`, on omm's
/// behalf, so a file that appeared during this install is omm's (class
/// `seeded`) and uninstall unlinks it once every key is restored (R5).
pub fn apply_profile(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    profile_id: &str,
    settings_existed_before: bool,
    converge: &mut Converge,
) -> Result<ProfileOutcome> {
    let profile = source
        .content
        .profiles
        .iter()
        .find(|p| p.asset.id == profile_id)
        .ok_or_else(|| OmmError::Usage(format!("the source ships no profile `{profile_id}`")))?;
    let mut doc = SettingsDoc::for_roots(&ctx.roots)?;
    if doc.has_mcp_collision() {
        return Err(OmmError::Host(omm_host::HostError::McpCollision));
    }
    // The file as this step found it: the audit line's `sha256_before`
    // (Gate 1: null although the file pre-existed) and — when the file is
    // the user's — its pre-omm record, unless an earlier step (the install's
    // first) already took it.
    let found = if doc.existed() {
        Original::read(doc.path())?
    } else {
        None
    };
    let before_sha = found.as_ref().map(|o| o.sha256.clone());
    let mut wanted = Vec::new();
    leaves(&[], &Value::Object(profile.settings.clone()), &mut wanted);
    let mut out = ProfileOutcome {
        profile: profile_id.to_string(),
        seeded: !doc.existed() || !settings_existed_before,
        ..ProfileOutcome::default()
    };
    let mut ops = Vec::new();
    for (path, value) in &wanted {
        if doc.get(path) == Some(value) {
            out.unchanged.push(path.join("."));
        } else {
            ops.push(PatchOp::set_path(path.clone(), value.clone()));
        }
    }
    converge.record_n(CAT_SETTINGS, Action::Unchanged, out.unchanged.len());
    if ops.is_empty() {
        return Ok(out);
    }
    let priors = doc.patch_typed(&ops)?;
    let report = doc.commit(
        inv,
        &CommitOptions::for_roots(&ctx.roots).dry_run(session.dry_run),
    )?;
    for w in &report.validation.warnings {
        ctx.out.warn(format!("settings.json: {w}"));
    }
    out.backup = report.backup.clone();
    if report.backup.is_some() {
        converge.record(CAT_SETTINGS, Action::BackedUp);
    }
    let rel = config_rel(ctx, &ctx.roots.settings_file())?;
    let seeded = out.seeded;
    // The tag `omm profile use` resolves a bundled profile to: the file
    // stem (`default`), never the asset id — so a later switch sees the
    // install's keys as that profile's.
    let tag = Path::new(&profile.file.rel)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(profile_id)
        .to_string();
    let ledger = session.ledger_or_new(inv)?;
    for p in &priors {
        let key = p.key();
        // First prior wins; `value` is what this write set (uninstall
        // restores the key only while it still holds that); the profile
        // tag names who set it.
        let value = wanted
            .iter()
            .find(|(path, _)| *path == p.path)
            .map(|(_, v)| v.clone());
        ledger.record_settings_key(&key, p.prior.clone(), value, Some(&tag));
        out.set.push(key);
    }
    let class = match ledger.find(Base::MuseConfig, &rel) {
        Some(e) if e.class == Class::Seeded => Class::Seeded,
        Some(_) => Class::SharedKey,
        None if seeded => Class::Seeded,
        None => Class::SharedKey,
    };
    ledger.upsert_keep_prior(Entry {
        base: Base::MuseConfig,
        path: rel.clone(),
        kind: Kind::SettingsKey,
        sha256: report.sha256.clone(),
        source_version: source.version.clone(),
        writer: "omm install".into(),
        mechanism: Mechanism::SettingsPatch,
        class,
        prior: None,
    });
    if let (Class::SharedKey, Some(o)) = (class, &found) {
        shared::attach_original(ledger, Base::MuseConfig, &rel, o);
    }
    session.save()?;
    for p in &priors {
        session.record(
            &Event::new(ACTION_INSTALL)
                .at(Base::MuseConfig, &rel)
                .before(before_sha.as_deref())
                .after(Some(&report.sha256))
                .note(format!(
                    "settings-patch {} (prior {})",
                    p.key(),
                    p.prior
                        .as_ref()
                        .map(Value::to_string)
                        .unwrap_or_else(|| "absent".into())
                )),
        )?;
    }
    converge.record_n(CAT_SETTINGS, Action::Updated, priors.len());
    Ok(out)
}

// ---------------------------------------------------------------------------
// trust
// ---------------------------------------------------------------------------

/// What the trust step did.
#[derive(Clone, Debug)]
pub struct TrustOutcome {
    pub key: String,
    pub action: Action,
    pub backup: Option<PathBuf>,
    pub seeded: bool,
    pub prior: Option<Value>,
}

impl TrustOutcome {
    pub fn to_json(&self) -> Value {
        json!({"project": self.key, "action": self.action.key(), "backup": self.backup, "seeded": self.seeded, "prior": self.prior})
    }
    pub fn line(&self) -> String {
        format!(
            "trust: {} {}{}",
            self.key,
            match self.action {
                Action::Updated => "trusted (prior recorded)",
                _ => "already trusted",
            },
            if self.seeded {
                "; trust.json created"
            } else {
                ""
            }
        )
    }
}

/// Merge `workspace` into `trust.json` as `trusted`, ledgered with its prior
/// (R10; undone by the registration, never by rewriting the file).
pub fn trust_workspace(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    workspace: &Path,
    converge: &mut Converge,
) -> Result<TrustOutcome> {
    let mut store = TrustStore::for_roots(&ctx.roots)?;
    let existed = store.existed();
    let found = if existed {
        Original::read(store.path())?
    } else {
        None
    };
    let merge = store.merge_project(workspace, TrustDecision::Trusted)?;
    if !merge.changed {
        converge.record(CAT_TRUST, Action::Unchanged);
        return Ok(TrustOutcome {
            key: merge.key,
            action: Action::Unchanged,
            backup: None,
            seeded: false,
            prior: merge.prior,
        });
    }
    let report = store.commit(&CommitOptions::for_roots(&ctx.roots).dry_run(session.dry_run))?;
    if report.backup.is_some() {
        converge.record(CAT_TRUST, Action::BackedUp);
    }
    converge.record(CAT_TRUST, Action::Updated);
    let rel = config_rel(ctx, &ctx.roots.trust_file())?;
    // The entry as written (the prior entry with `decision` set), so
    // uninstall can tell a later hand edit from what omm wrote.
    let written = store
        .value()
        .get("projects")
        .and_then(|p| p.get(&merge.key))
        .cloned();
    let ledger = session.ledger_or_new(inv)?;
    ledger.record_trust(&merge.key, merge.prior.clone(), written);
    let class = match ledger.find(Base::MuseConfig, &rel) {
        Some(e) if e.class == Class::Seeded => Class::Seeded,
        Some(_) => Class::SharedKey,
        None if !existed => Class::Seeded,
        None => Class::SharedKey,
    };
    ledger.upsert_keep_prior(Entry {
        base: Base::MuseConfig,
        path: rel.clone(),
        kind: Kind::Trust,
        sha256: report.sha256.clone(),
        source_version: OMM_VERSION.into(),
        writer: "omm trust".into(),
        mechanism: Mechanism::TrustMerge,
        class,
        prior: None,
    });
    if let (Class::SharedKey, Some(o)) = (class, &found) {
        shared::attach_original(ledger, Base::MuseConfig, &rel, o);
    }
    session.save()?;
    session.record(
        &Event::new(ACTION_INSTALL)
            .at(Base::MuseConfig, &rel)
            .before(found.as_ref().map(|o| o.sha256.as_str()))
            .after(Some(&report.sha256))
            .note(format!(
                "trust-merge {} trusted (prior {})",
                merge.key,
                merge
                    .prior
                    .as_ref()
                    .map(Value::to_string)
                    .unwrap_or_else(|| "absent".into())
            )),
    )?;
    Ok(TrustOutcome {
        key: merge.key,
        action: Action::Updated,
        backup: report.backup,
        seeded: !existed,
        prior: merge.prior,
    })
}

// ---------------------------------------------------------------------------
// the shared files as found (Gate 1)
// ---------------------------------------------------------------------------

/// Record the pre-omm bytes and mode of `settings.json` and `trust.json` in
/// their ledger entries — the first thing `omm install` does after opening
/// the ledger, before any host verb can rewrite either (the bundle path's
/// `plugins install / approve` rewrite settings.json from the host's typed
/// struct, dropping every key it does not type and its mode). A file that
/// is not there records nothing (it is `seeded` when created later). Saves
/// when anything was recorded; returns the base-relative paths recorded.
pub fn record_shared_originals(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source_version: &str,
) -> Result<Vec<String>> {
    let files = [
        (
            ctx.roots.settings_file(),
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
        ),
        (ctx.roots.trust_file(), Kind::Trust, Mechanism::TrustMerge),
    ];
    let mut recorded = Vec::new();
    for (abs, kind, mechanism) in files {
        let rel = config_rel(ctx, &abs)?;
        let ledger = session.ledger_or_new(inv)?;
        if shared::record_original(
            ledger,
            Base::MuseConfig,
            &rel,
            kind,
            mechanism,
            &abs,
            "omm install",
            source_version,
        )? {
            recorded.push(rel.as_str().to_string());
        }
    }
    if !recorded.is_empty() {
        session.save()?;
    }
    Ok(recorded)
}

/// The `settings.json` entry of a file the host is about to create on omm's
/// behalf (`plugins approve` writes it when there is none): class `seeded`,
/// so uninstall unlinks it once every key is restored. Recorded right after
/// `plugins install`, before an approval runs; the caller saves.
pub fn record_seeded_settings(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source_version: &str,
) -> Result<()> {
    let abs = ctx.roots.settings_file();
    let rel = config_rel(ctx, &abs)?;
    let ledger = session.ledger_or_new(inv)?;
    if ledger.find(Base::MuseConfig, &rel).is_some() {
        return Ok(());
    }
    let sha256 = if abs.is_file() {
        fsx::sha256_file(&abs)?
    } else {
        String::new()
    };
    ledger.upsert(Entry {
        base: Base::MuseConfig,
        path: rel,
        kind: Kind::SettingsKey,
        sha256,
        source_version: source_version.to_string(),
        writer: "omm install".into(),
        mechanism: Mechanism::SettingsPatch,
        class: Class::Seeded,
        prior: None,
    });
    Ok(())
}

/// What [`merge_untyped_back`] did.
#[derive(Clone, Debug, Default)]
pub struct MergeBack {
    /// Dotted keys the host's rewrite had dropped, now back.
    pub keys: Vec<String>,
    /// The mode was re-applied (the host's rewrite landed another).
    pub mode_reapplied: bool,
}

impl MergeBack {
    pub fn to_json(&self) -> Value {
        json!({"keys_restored": self.keys, "mode_reapplied": self.mode_reapplied})
    }
    pub fn line(&self) -> Option<String> {
        if self.keys.is_empty() && !self.mode_reapplied {
            return None;
        }
        let mut s = String::from("settings.json:");
        if !self.keys.is_empty() {
            s.push_str(&format!(
                " {} untyped key(s) the host's rewrite dropped put back ({})",
                self.keys.len(),
                self.keys.join(", ")
            ));
        }
        if self.mode_reapplied {
            s.push_str(if self.keys.is_empty() {
                " mode re-applied after the host's rewrite"
            } else {
                "; mode re-applied"
            });
        }
        Some(s)
    }
}

/// After a command's host steps (`omm install` / `omm update`: `plugins
/// remove / install / approve`, `skills uninstall / install`), put back the
/// members of `settings.json` the host does not type and its rewrite
/// dropped, as the file held them before the first host step (`before`),
/// and the file's mode (Gate 1 decision F: the recorded mode comes back
/// after every host mutation that can rewrite the file). The merged
/// document is validated by the host's own loader first; the write runs
/// under the muse-config lock like every other. Nothing happens when
/// nothing was dropped. `action` labels the audit line.
pub fn merge_untyped_back(
    ctx: &Ctx,
    inv: &Invoker,
    session: &Session,
    before: &Original,
    action: &str,
) -> Result<MergeBack> {
    let mut out = MergeBack::default();
    if session.dry_run {
        return Ok(out);
    }
    let path = ctx.roots.settings_file();
    let Some(now) = Original::read(&path)? else {
        return Ok(out);
    };
    let Some(before_doc) = before.document() else {
        return Ok(out);
    };
    let Ok(mut doc) = serde_json::from_slice::<Value>(&now.bytes) else {
        return Ok(out);
    };
    let members = omm_host::settings::untyped_members(&before_doc)?;
    let inserted = shared::merge_members(&mut doc, &members);
    let _lock = fsx::lock_exclusive(
        &omm_host::settings::muse_config_lock_path(&ctx.omm_root()),
        fsx::LOCK_WAIT_DEFAULT,
    )?;
    if !inserted.is_empty() {
        let bytes = shared::pretty(&doc)?;
        let load = omm_host::probe::settings_load_probe(inv, &bytes)?;
        if !load.accepted {
            return Err(OmmError::Host(omm_host::HostError::SettingsRejected {
                stage: "host-loader",
                detail: load.detail,
            }));
        }
        let realpath = fsx::realpath_for_write(&path)?;
        fsx::write_atomic(&realpath, &bytes)?;
        session.record(
            &Event::new(action)
                .at(Base::MuseConfig, &config_rel(ctx, &path)?)
                .before(Some(&now.sha256))
                .after(Some(&fsx::sha256_bytes(&bytes)))
                .note(format!(
                    "untyped keys the host's rewrite dropped put back: {}",
                    inserted.join(", ")
                )),
        )?;
        out.keys = inserted;
    }
    if let (Some(want), Some(have)) = (before.mode, now.mode) {
        if want != have {
            shared::set_mode(&path, want)?;
            out.mode_reapplied = true;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omm_themes_dir_must_lie_inside_the_config_root() {
        // The env var is process-global: this test owns it for its span and
        // restores it; the cases are decided on the value alone.
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let ctx = Ctx::new(sb.roots().unwrap(), crate::cmd::Flags::default());
        let config = ctx.roots.muse_config();
        let saved = std::env::var_os(THEMES_DIR_ENV);
        std::env::remove_var(THEMES_DIR_ENV);
        assert_eq!(themes_dir(&ctx).unwrap(), config.join("themes"));
        std::env::set_var(THEMES_DIR_ENV, config.join("omm-themes"));
        assert_eq!(themes_dir(&ctx).unwrap(), config.join("omm-themes"));
        for bad in [
            tmp.path().join("elsewhere"),
            config.clone(),
            config.join("..").join("x"),
        ] {
            std::env::set_var(THEMES_DIR_ENV, &bad);
            let err = themes_dir(&ctx).unwrap_err();
            assert!(
                matches!(err, OmmError::Usage(_)),
                "{}: {err}",
                bad.display()
            );
            assert!(err.to_string().contains(THEMES_DIR_ENV), "{err}");
        }
        match saved {
            Some(v) => std::env::set_var(THEMES_DIR_ENV, v),
            None => std::env::remove_var(THEMES_DIR_ENV),
        }
    }

    #[test]
    fn profile_slices_flatten_to_typed_leaves() {
        let slice = json!({
            "run": {"context_slimming": {"skill_catalog_descriptions": "first_sentence", "full_skill_description_ids": ["bundled:git"], "excluded_tool_names": []}},
            "reasoning_effort": "medium",
            "tui": {}
        });
        let mut out = Vec::new();
        leaves(&[], &slice, &mut out);
        let keys: Vec<String> = out.iter().map(|(p, _)| p.join(".")).collect();
        assert_eq!(
            keys,
            vec![
                "run.context_slimming.skill_catalog_descriptions",
                "run.context_slimming.full_skill_description_ids",
                "run.context_slimming.excluded_tool_names",
                "reasoning_effort",
                "tui",
            ]
        );
        assert_eq!(out[1].1, json!(["bundled:git"]));
        assert_eq!(out[2].1, json!([]));
        assert_eq!(out[4].1, json!({}));
    }
}
