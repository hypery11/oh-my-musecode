//! `omm install --no-plugin` (Tier 0, 00-DECISION.md §2.1): every skill of
//! the source installed with `muse skills install <dir> --scope user --json`
//! into the host's managed personal store `$CONFIG_DIR/skills/<id>/`
//! (`research/musecode/config-paths.md` §3), and updated with the R3
//! three-way merge decided per file and applied per skill.
//!
//! The host writes the files and its own `skills/.muse/{lock.json,
//! audit.log,.skills.lock}` beside them; omm records one ledger entry per
//! file it asked the host to write (`mechanism: muse-skills-install`, R2:
//! the shas the host reports in `installed.provenance.files[]` must equal
//! the source bytes), never the store's metadata, which stays the host's
//! and is named as kept residue at uninstall. A skill is all-or-nothing:
//! one edited file stages the whole skill's new content and leaves the
//! store untouched; an untouched skill is refreshed with `skills uninstall`
//! followed by `skills install` (the host re-reads its lockfile's source
//! path on `skills update`, which a moved checkout would defeat).
//!
//! The ledger is saved after every `muse skills install` — the host holds
//! the files the moment the verb returns, so the ledger records them before
//! anything else can fail (Gate 1: twelve installs, one save at the end; an
//! interrupt left the skills in the store with no ledger, and uninstall
//! saw nothing to do). A ledgered skill whose files vanished from the store
//! (a hand `muse skills uninstall`) is reinstalled by `omm install
//! --no-plugin` — the fix doctor D1 prints — as long as what is left on
//! disk is what omm wrote.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use omm_host::Invoker;
use omm_ledger::audit::{Event, ACTION_INSTALL, ACTION_UPDATE};
use omm_ledger::containment::{Bases, State};
use omm_ledger::reconcile::{self, ApplyOptions, Candidate, Outcome, Staged};
use omm_ledger::{hash, Base, Class, Entry, Kind, Mechanism, RelPath};

use super::session::Session;
use super::source::Source;
use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Action, Converge};

/// The converge category.
pub const CAT: &str = "skills";

/// What the skills step did.
#[derive(Clone, Debug, Default)]
pub struct SkillsReport {
    pub installed: Vec<String>,
    pub updated: Vec<String>,
    pub adopted: Vec<String>,
    pub unchanged: Vec<String>,
    /// `(id, why)`.
    pub skipped: Vec<(String, String)>,
    pub staged: Vec<Staged>,
    /// Ledgered skills the source no longer ships (left alone).
    pub orphans: Vec<String>,
}

impl SkillsReport {
    pub fn to_json(&self) -> Value {
        json!({
            "installed": self.installed,
            "updated": self.updated,
            "adopted": self.adopted,
            "unchanged": self.unchanged,
            "skipped": self.skipped.iter().map(|(id, why)| json!({"id": id, "why": why})).collect::<Vec<_>>(),
            "staged": self.staged.iter().map(|s| json!({
                "base": s.base.as_str(), "path": s.path.as_str(), "staged_to": s.staged_to, "reason": s.reason,
            })).collect::<Vec<_>>(),
            "orphans": self.orphans,
        })
    }

    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!(
            "skills (muse skills install --scope user): {} installed, {} updated, {} adopted, {} unchanged, {} skipped, {} file(s) staged",
            self.installed.len(),
            self.updated.len(),
            self.adopted.len(),
            self.unchanged.len(),
            self.skipped.len(),
            self.staged.len()
        ));
        for (id, why) in &self.skipped {
            out.push(format!("  skipped {id}: {why}"));
        }
        for s in &self.staged {
            out.push(format!(
                "  staged {}:{} -> {} ({})",
                s.base,
                s.path,
                s.staged_to.display(),
                s.reason
            ));
        }
        if !self.orphans.is_empty() {
            out.push(format!(
                "  no longer shipped (left in place): {}",
                self.orphans.join(", ")
            ));
        }
        out
    }
}

/// One skill's desired state: its id, source directory and file candidates.
struct Desired {
    id: String,
    dir: PathBuf,
    candidates: Vec<Candidate>,
}

/// The base-relative store path of a skill file: `skills/<id>/<rel in skill>`.
fn store_rel(ctx: &Ctx, skill_id: &str, rel_in_skill: &str) -> Result<RelPath> {
    let store = ctx.roots.personal_skills_dir();
    let base = ctx.roots.muse_config();
    let prefix = store
        .strip_prefix(&base)
        .map_err(|_| OmmError::Usage("the skills store is not under the config root".into()))?;
    let mut rel = omm_manifest::posix(prefix);
    rel.push('/');
    rel.push_str(skill_id);
    rel.push('/');
    rel.push_str(rel_in_skill);
    Ok(RelPath::new(rel)?)
}

fn desired(ctx: &Ctx, source: &Source) -> Result<Vec<Desired>> {
    let mut out = Vec::new();
    for skill in &source.content.skills {
        let id = skill.asset.id.clone();
        let mut candidates = Vec::new();
        for file in &skill.files {
            let in_skill = file
                .rel
                .strip_prefix(&format!("{}/", skill.dir))
                .unwrap_or(&file.rel);
            candidates.push(Candidate {
                base: Base::MuseConfig,
                path: store_rel(ctx, &id, in_skill)?,
                kind: Kind::Skill,
                mechanism: Mechanism::MuseSkillsInstall,
                class: Class::Exclusive,
                source_version: source.version.clone(),
                bytes: file.read()?,
            });
        }
        out.push(Desired {
            id,
            dir: source.content.root.join(&skill.dir),
            candidates,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Every file a `--no-plugin` install asks the host to write:
/// `(base-relative store path, bytes)`, for the install plan (Gate 1
/// decision A: containment is checked before the first host mutation).
pub fn plan_files(ctx: &Ctx, source: &Source) -> Result<Vec<(RelPath, u64)>> {
    Ok(desired(ctx, source)?
        .iter()
        .flat_map(|d| {
            d.candidates
                .iter()
                .map(|c| (c.path.clone(), c.bytes.len() as u64))
        })
        .collect())
}

/// The base-relative path of a skill's store directory (`skills/<id>`).
pub fn store_dir_rel(ctx: &Ctx, skill_id: &str) -> Result<RelPath> {
    RelPath::new(
        store_rel(ctx, skill_id, "SKILL.md")?
            .as_str()
            .trim_end_matches("/SKILL.md"),
    )
    .map_err(OmmError::from)
}

/// The skill id of a ledger entry the managed store wrote (`skills/<id>/…`).
fn ledgered_skill_id(ctx: &Ctx, e: &Entry) -> Option<String> {
    if e.base != Base::MuseConfig || e.mechanism != Mechanism::MuseSkillsInstall {
        return None;
    }
    let store = ctx.roots.personal_skills_dir();
    let prefix = omm_manifest::posix(store.strip_prefix(ctx.roots.muse_config()).ok()?);
    let rest = e.path.as_str().strip_prefix(&format!("{prefix}/"))?;
    let (id, tail) = rest.split_once('/')?;
    (!tail.is_empty()).then(|| id.to_string())
}

/// `muse skills install <dir> --scope user --json` → the files the host
/// reports, `(relative path, sha256 hex)`.
fn host_install(inv: &Invoker, dir: &std::path::Path) -> Result<Vec<(String, String)>> {
    let out = inv.run_json(&[
        "skills".to_string(),
        "install".to_string(),
        dir.to_string_lossy().into_owned(),
        "--scope".to_string(),
        "user".to_string(),
        "--json".to_string(),
    ])?;
    if !out.outcome.ok() {
        let detail = out
            .outcome
            .host_reported_error()
            .map(|e| e.to_string())
            .unwrap_or_else(|| out.outcome.stderr.lines().next().unwrap_or("").to_string());
        return Err(OmmError::io(
            format!("muse skills install {}", dir.display()),
            std::io::Error::other(detail),
        ));
    }
    Ok(out.json["installed"]["provenance"]["files"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    Some((
                        r["relative_path"].as_str()?.to_string(),
                        r["sha256"]
                            .as_str()?
                            .trim_start_matches("sha256:")
                            .to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// `muse skills uninstall <id> --json`; an unknown skill is fine.
pub(super) fn host_uninstall(inv: &Invoker, id: &str) -> Result<()> {
    let out = inv.run(&["skills", "uninstall", id, "--json"])?;
    if out.ok() {
        return Ok(());
    }
    let not_installed = out
        .host_reported_error()
        .map(|e| {
            e.to_string().contains("not installed") || e.to_string().contains("skill-not-installed")
        })
        .unwrap_or(false);
    if not_installed {
        return Ok(());
    }
    Err(OmmError::io(
        format!("muse skills uninstall {id}"),
        std::io::Error::other(
            out.host_reported_error()
                .map(|e| e.to_string())
                .unwrap_or_else(|| out.stderr.lines().next().unwrap_or("").to_string()),
        ),
    ))
}

/// Install one skill through the host and ledger every file it wrote,
/// cross-checking the host's shas against the source bytes (R2).
fn install_one(
    inv: &Invoker,
    session: &mut Session,
    d: &Desired,
    writer: &str,
    action: &str,
) -> Result<()> {
    if session.dry_run {
        return Ok(());
    }
    let reported = host_install(inv, &d.dir)?;
    let by_rel: BTreeMap<&str, &str> = reported
        .iter()
        .map(|(r, s)| (r.as_str(), s.as_str()))
        .collect();
    let mut written: Vec<(&Candidate, String)> = Vec::new();
    for c in &d.candidates {
        let theirs = c.sha256();
        let in_skill = c
            .path
            .as_str()
            .rsplit_once(&format!("/{}/", d.id))
            .map(|(_, tail)| tail)
            .unwrap_or(c.path.as_str());
        match by_rel.get(in_skill) {
            Some(sha) if *sha == theirs => {}
            Some(sha) => {
                return Err(OmmError::io(
                    format!("muse skills install {}", d.id),
                    std::io::Error::other(format!(
                        "the host wrote {in_skill} with sha256 {sha}, the source has {theirs}"
                    )),
                ))
            }
            None => {
                return Err(OmmError::io(
                    format!("muse skills install {}", d.id),
                    std::io::Error::other(format!(
                        "the host did not report {in_skill} among the installed files"
                    )),
                ))
            }
        }
        written.push((c, theirs));
    }
    // The host holds the files: the ledger records them and is saved before
    // the audit lines, so a line's presence implies the ledger's.
    let mut befores = Vec::with_capacity(written.len());
    {
        let ledger = session.ledger_or_new(inv)?;
        for (c, theirs) in &written {
            let prior = ledger.find(c.base, &c.path).and_then(|e| e.prior.clone());
            befores.push(ledger.find(c.base, &c.path).map(|e| e.sha256.clone()));
            ledger.upsert(Entry {
                base: c.base,
                path: c.path.clone(),
                kind: c.kind,
                sha256: theirs.clone(),
                source_version: c.source_version.clone(),
                writer: writer.to_string(),
                mechanism: c.mechanism,
                class: c.class,
                prior,
            });
        }
    }
    session.save()?;
    for ((c, theirs), before) in written.iter().zip(befores) {
        session.record(
            &Event::new(action)
                .at(c.base, &c.path)
                .before(before.as_deref())
                .after(Some(theirs))
                .note(format!(
                    "muse skills install {} --scope user",
                    d.dir.display()
                )),
        )?;
    }
    Ok(())
}

/// `omm install --no-plugin`: install what is missing, leave what omm
/// already wrote (edits and source changes are `omm update`'s work).
pub fn converge_install(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    converge: &mut Converge,
) -> Result<SkillsReport> {
    let mut report = SkillsReport::default();
    let wanted = desired(ctx, source)?;
    // The host's version is asked for before the first host mutation, so
    // the first save follows the first `skills install` at once.
    session.ledger_or_new(inv)?;
    for d in &wanted {
        let n = d.candidates.len();
        let entries: Vec<Entry> = session
            .ledger
            .as_ref()
            .map(|l| {
                l.entries
                    .iter()
                    .filter(|e| ledgered_skill_id(ctx, e).as_deref() == Some(d.id.as_str()))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        if !entries.is_empty() {
            let mut missing = 0usize;
            let mut edited = false;
            let mut source_changed = false;
            for c in &d.candidates {
                let Some(e) = entries.iter().find(|e| e.path == c.path) else {
                    source_changed = true;
                    continue;
                };
                if e.sha256 != c.sha256() {
                    source_changed = true;
                }
                let r = session.probe(c.base, &c.path)?;
                match r.state {
                    State::Missing => missing += 1,
                    State::File => {
                        if !hash::observe(&r.path)?.equals(&e.sha256) {
                            edited = true;
                        }
                    }
                    _ => edited = true,
                }
            }
            if missing > 0 && !edited {
                // Vanished from the store (a hand `muse skills uninstall`,
                // an `rm`); what is left is what omm wrote, so the host
                // installs it again — the fix doctor D1 prints (Gate 1: it
                // was skipped as "differs from the ledger").
                if !session.dry_run && managed_by_host(ctx, &d.id) {
                    host_uninstall(inv, &d.id)?;
                }
                install_one(inv, session, d, "omm install", ACTION_INSTALL)?;
                converge.record_n(CAT, Action::Updated, n);
                report.installed.push(d.id.clone());
            } else if edited {
                converge.record_n(CAT, Action::Skipped, n);
                report.skipped.push((
                    d.id.clone(),
                    "a file differs from what omm wrote; `omm update` stages the new content"
                        .into(),
                ));
            } else if source_changed {
                converge.record_n(CAT, Action::Skipped, n);
                report.skipped.push((
                    d.id.clone(),
                    "the source changed since omm wrote it; `omm update` reconciles it".into(),
                ));
            } else {
                converge.record_n(CAT, Action::Unchanged, n);
                report.unchanged.push(d.id.clone());
            }
            continue;
        }
        // Nothing ledgered: what is at the store path?
        let at = session.probe(Base::MuseConfig, &store_dir_rel(ctx, &d.id)?)?;
        match at.state {
            State::Missing => {
                install_one(inv, session, d, "omm install", ACTION_INSTALL)?;
                converge.record_n(CAT, Action::Updated, n);
                report.installed.push(d.id.clone());
            }
            State::Dir => {
                // A managed install omm never ledgered (an earlier omm, or the
                // user's own `muse skills install` of the same source): adopt
                // it only when every file already equals the source.
                let identical = identical_to_source(session, d)?;
                if identical && managed_by_host(ctx, &d.id) {
                    adopt_skill(inv, session, d, "omm install")?;
                    converge.record_n(CAT, Action::Unchanged, n);
                    report.adopted.push(d.id.clone());
                } else {
                    // An install interrupted inside `muse skills install`
                    // leaves the directory the host lists with no provenance
                    // (Gate 1 round 5 M1): identical to the source and inside
                    // the base → omm removes it and installs it fresh, because
                    // `muse skills uninstall` refuses `provenance-missing`.
                    match classify_orphan_store_dir(ctx, &session.bases, source, &d.id)? {
                        OrphanVerdict::Remove { dir, files } => {
                            if !session.dry_run {
                                let removed = remove_orphan_store_dir(&dir, &files)?;
                                session.record(
                                    &Event::new(ACTION_INSTALL)
                                        .at(Base::MuseConfig, &store_dir_rel(ctx, &d.id)?)
                                        .note(format!(
                                            "removed the store directory the host had no provenance for (an interrupted `muse skills install`; {} path(s)); reinstalling",
                                            removed.len()
                                        )),
                                )?;
                                install_one(inv, session, d, "omm install", ACTION_INSTALL)?;
                            }
                            converge.record_n(CAT, Action::Updated, n);
                            report.installed.push(d.id.clone());
                        }
                        OrphanVerdict::HostOwned => {
                            return Err(OmmError::Usage(format!(
                                "{} exists but was not written by omm (content differs from the source); remove it or `muse skills uninstall {}` first",
                                at.path.display(),
                                d.id
                            )));
                        }
                        OrphanVerdict::Keep(reason) => {
                            // `muse skills uninstall` refuses a directory the
                            // host has no provenance for: the way out is by hand.
                            return Err(OmmError::Usage(format!(
                                "{} exists but was not written by omm and the host has no provenance for it ({reason}); `muse skills uninstall {}` refuses it (provenance-missing) — move or remove the directory by hand, then rerun",
                                at.path.display(),
                                d.id
                            )));
                        }
                    }
                }
            }
            other => {
                return Err(OmmError::Usage(format!(
                    "{} is {:?}, not a directory omm can install into",
                    at.path.display(),
                    other
                )))
            }
        }
    }
    session.save()?;
    Ok(report)
}

/// True when every file of the skill on disk equals the source bytes.
fn identical_to_source(session: &Session, d: &Desired) -> Result<bool> {
    d.candidates.iter().try_fold(true, |ok, c| -> Result<bool> {
        let r = session.probe(c.base, &c.path)?;
        Ok(ok && r.via_symlink.is_none() && hash::observe(&r.path)?.equals(&c.sha256()))
    })
}

/// Ledger every file of a managed skill already in the store as omm's
/// (`adopt`): the entries, saved, then the audit lines.
fn adopt_skill(inv: &Invoker, session: &mut Session, d: &Desired, writer: &str) -> Result<()> {
    {
        let ledger = session.ledger_or_new(inv)?;
        for c in &d.candidates {
            ledger.upsert(Entry {
                base: c.base,
                path: c.path.clone(),
                kind: c.kind,
                sha256: c.sha256(),
                source_version: c.source_version.clone(),
                writer: writer.to_string(),
                mechanism: c.mechanism,
                class: c.class,
                prior: None,
            });
        }
    }
    session.save()?;
    for c in &d.candidates {
        session.record(
            &Event::new(ACTION_INSTALL)
                .at(c.base, &c.path)
                .after(Some(&c.sha256()))
                .note(format!(
                    "{writer}: adopt — identical managed install already present"
                )),
        )?;
    }
    Ok(())
}

/// `(id, why)` pairs of skills a step left alone.
pub type IdReasons = Vec<(String, String)>;

/// `omm reconcile` (Gate 1 decision E): every skill of `source` the host's
/// managed store holds — identical to the source, listed by the store's
/// lockfile — that the ledger has no entries for is adopted into the
/// ledger; nothing is written to the host. Returns `(adopted ids, (id,
/// why) not adopted)`, sorted.
pub fn adopt_identical(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    writer: &str,
) -> Result<(Vec<String>, IdReasons)> {
    let mut adopted = Vec::new();
    let mut skipped = Vec::new();
    let ledgered = session
        .ledger
        .as_ref()
        .map(|l| ledgered_ids(ctx, l))
        .unwrap_or_default();
    for d in desired(ctx, source)? {
        if ledgered.contains(&d.id) {
            continue;
        }
        let at = session.probe(Base::MuseConfig, &store_dir_rel(ctx, &d.id)?)?;
        if at.state != State::Dir {
            continue;
        }
        if at.via_symlink.is_some() {
            skipped.push((
                d.id.clone(),
                "the store directory lies behind a symlink (R2 sentinel)".into(),
            ));
            continue;
        }
        if !managed_by_host(ctx, &d.id) {
            skipped.push((
                d.id.clone(),
                "not in the host's managed store lockfile".into(),
            ));
            continue;
        }
        if !identical_to_source(session, &d)? {
            skipped.push((d.id.clone(), "content differs from the source".into()));
            continue;
        }
        if !session.dry_run {
            adopt_skill(inv, session, &d, writer)?;
        }
        adopted.push(d.id.clone());
    }
    Ok((adopted, skipped))
}

/// What to do with a managed-store directory the host lists but that has no
/// ledger entry — an install interrupted inside `muse skills install`
/// leaves the directory the host reports with no provenance
/// (`provenance-missing`; Gate 1 round 5 M1).
#[derive(Debug)]
pub enum OrphanVerdict {
    /// The store lockfile lists it: the host has provenance, so `muse skills
    /// uninstall <id>` works.
    HostOwned,
    /// A regular directory inside the base (not a symlink, not reached
    /// through one), the lockfile does not list it, and every file equals
    /// the source with none besides: omm removes it itself, contained and
    /// deepest-first, because `muse skills uninstall` refuses it.
    Remove { dir: PathBuf, files: Vec<PathBuf> },
    /// Left in place and named (a symlink, content that differs, a file the
    /// source does not ship).
    Keep(String),
}

/// Classify a managed-store directory the host lists but the ledger does not
/// (Gate 1 round 5 M1). Reads the store lockfile, the store directory
/// through [`Bases::resolve`], and the source bytes; writes nothing.
pub fn classify_orphan_store_dir(
    ctx: &Ctx,
    bases: &Bases,
    source: &Source,
    id: &str,
) -> Result<OrphanVerdict> {
    if managed_by_host(ctx, id) {
        return Ok(OrphanVerdict::HostOwned);
    }
    let dir_rel = store_dir_rel(ctx, id)?;
    let dir = match bases.resolve(Base::MuseConfig, &dir_rel) {
        Ok(r) => r,
        Err(e) => {
            return Ok(OrphanVerdict::Keep(format!(
                "its store directory does not resolve inside the base ({e})"
            )))
        }
    };
    if dir.state != State::Dir || dir.via_symlink.is_some() {
        return Ok(OrphanVerdict::Keep(
            "its store directory is a symlink or is reached through one — not what omm wrote (R2 sentinel)".into(),
        ));
    }
    let Some(d) = desired(ctx, source)?.into_iter().find(|d| d.id == id) else {
        return Ok(OrphanVerdict::Keep(
            "the source does not ship it; omm cannot tell it is its own".into(),
        ));
    };
    let mut files = Vec::new();
    for c in &d.candidates {
        let r = match bases.resolve(c.base, &c.path) {
            Ok(r) => r,
            Err(e) => {
                return Ok(OrphanVerdict::Keep(format!(
                    "a store file does not resolve inside the base ({e})"
                )))
            }
        };
        if r.via_symlink.is_some() {
            return Ok(OrphanVerdict::Keep(
                "a store file is reached through a symlink — not what omm wrote (R2 sentinel)"
                    .into(),
            ));
        }
        match r.state {
            // A file the interrupted copy never reached: nothing to compare,
            // nothing of the user's.
            State::Missing => {}
            State::File if hash::observe(&r.path)?.equals(&c.sha256()) => files.push(r.path),
            _ => {
                return Ok(OrphanVerdict::Keep(
                    "its content differs from the source; omm cannot tell it is its own".into(),
                ))
            }
        }
    }
    // No file on disk beyond the source's (else the directory holds the
    // user's own data and is not omm's to remove).
    let source_files: BTreeSet<&PathBuf> = files.iter().collect();
    for p in walk_regular_files(&dir.path) {
        if !source_files.contains(&p) {
            return Ok(OrphanVerdict::Keep(format!(
                "its store directory holds {}, which the source does not ship",
                p.display()
            )));
        }
    }
    Ok(OrphanVerdict::Remove {
        dir: dir.path,
        files,
    })
}

/// Remove an orphaned store directory omm-style: the files deepest-first,
/// then the directories left empty up to and including the store directory
/// (`dir`, an already-contained canonical path). Returns the paths removed.
pub fn remove_orphan_store_dir(dir: &Path, files: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let mut ordered: Vec<&PathBuf> = files.iter().collect();
    ordered.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
    for f in ordered {
        match std::fs::remove_file(f) {
            Ok(()) => removed.push(f.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(OmmError::io(format!("remove {}", f.display()), e)),
        }
    }
    // The directory is now empty (classify verified nothing else is in it).
    match std::fs::remove_dir_all(dir) {
        Ok(()) => removed.push(dir.to_path_buf()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(OmmError::io(format!("remove {}", dir.display()), e)),
    }
    Ok(removed)
}

/// Every regular file under `dir`, canonical paths, symlinks reported as
/// their own path (never followed).
fn walk_regular_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let Ok(meta) = std::fs::symlink_metadata(&p) else {
                out.push(p);
                continue;
            };
            if meta.file_type().is_symlink() {
                out.push(p);
            } else if meta.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}

/// True when the host's store lockfile lists `id` — the host has provenance
/// and `muse skills uninstall <id>` works (public view of [`managed_by_host`]).
pub fn store_dir_managed_by_host(ctx: &Ctx, id: &str) -> bool {
    managed_by_host(ctx, id)
}

/// True when the host's store lockfile lists `id` (`skills/.muse/lock.json`).
fn managed_by_host(ctx: &Ctx, id: &str) -> bool {
    let lock = ctx
        .roots
        .personal_skills_dir()
        .join(".muse")
        .join("lock.json");
    std::fs::read(&lock)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|v| v["skills"].get(id).is_some())
        .unwrap_or(false)
}

/// `omm update` for a `--no-plugin` install: the R3 merge decided per file,
/// applied per skill (see the module docs). `version` labels `updates/`.
pub fn converge_update(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    version: &str,
    converge: &mut Converge,
) -> Result<SkillsReport> {
    let mut report = SkillsReport::default();
    let wanted = desired(ctx, source)?;
    let all: Vec<Candidate> = wanted.iter().flat_map(|d| d.candidates.clone()).collect();
    let ledger = session.ledger_or_new(inv)?.clone();
    let plan = reconcile::plan(&ledger, &session.bases, version, all)?;
    for d in &wanted {
        let decisions: Vec<_> = plan
            .decisions
            .iter()
            .filter(|dec| d.candidates.iter().any(|c| c.key() == dec.key))
            .collect();
        let n = d.candidates.len();
        if decisions.iter().all(|x| x.outcome == Outcome::NoOp) {
            converge.record_n(CAT, Action::Unchanged, n);
            report.unchanged.push(d.id.clone());
            continue;
        }
        if decisions.iter().any(|x| x.outcome == Outcome::Stage) {
            let staged_candidates: Vec<Candidate> = decisions
                .iter()
                .filter(|x| x.outcome == Outcome::Stage)
                .filter_map(|x| plan.candidates.get(x.candidate).cloned())
                .collect();
            let staged_n = staged_candidates.len();
            let sub = reconcile::plan(&ledger, &session.bases, version, staged_candidates)?;
            let mut scratch = ledger.clone();
            let r = reconcile::apply(
                &sub,
                &mut scratch,
                &session.bases,
                &ApplyOptions {
                    omm_root: session.omm_root.clone(),
                    writer: "omm update".into(),
                    omm_version: OMM_VERSION.into(),
                    snapshot: false,
                    dry_run: session.dry_run,
                },
            )?;
            report.staged.extend(r.staged);
            converge.record_n(CAT, Action::Skipped, n);
            report.skipped.push((
                d.id.clone(),
                format!("{staged_n} of {n} file(s) differ on disk from what omm wrote; the skill is kept whole and the new files are staged"),
            ));
            continue;
        }
        // Overwrite / Adopt only: the host rewrites the skill from the source.
        let had_entries = decisions.iter().any(|x| x.ancestor.is_some());
        if !session.dry_run {
            if had_entries {
                host_uninstall(inv, &d.id)?;
            }
            install_one(inv, session, d, "omm update", ACTION_UPDATE)?;
        }
        let overwritten = decisions
            .iter()
            .filter(|x| x.outcome == Outcome::Overwrite)
            .count();
        converge.record_n(CAT, Action::Updated, overwritten);
        converge.record_n(CAT, Action::Unchanged, n - overwritten);
        if had_entries {
            report.updated.push(d.id.clone());
        } else {
            report.installed.push(d.id.clone());
        }
    }
    let wanted_ids: Vec<&str> = wanted.iter().map(|d| d.id.as_str()).collect();
    let mut orphans: Vec<String> = ledger
        .entries
        .iter()
        .filter_map(|e| ledgered_skill_id(ctx, e))
        .filter(|id| !wanted_ids.contains(&id.as_str()))
        .collect();
    orphans.sort();
    orphans.dedup();
    report.orphans = orphans;
    session.save()?;
    Ok(report)
}

/// Ledgered skill ids (for `omm install` mode checks and uninstall notes).
pub fn ledgered_ids(ctx: &Ctx, ledger: &omm_ledger::Ledger) -> Vec<String> {
    let mut ids: Vec<String> = ledger
        .entries
        .iter()
        .filter_map(|e| ledgered_skill_id(ctx, e))
        .collect();
    ids.sort();
    ids.dedup();
    ids
}
