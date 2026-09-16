//! The three-way merge (R3) with `ancestor = entry.sha256` (what omm wrote),
//! `theirs = new source sha`, `mine = on-disk sha`:
//!
//! | ancestor vs theirs | mine vs ancestor | outcome |
//! |---|---|---|
//! | equal | any | **no-op** |
//! | differ | equal | **overwrite** (the user never touched it) |
//! | differ | equal to theirs | **adopt** (the user already has the new content) |
//! | differ | differ | **stage** → `$OMM/updates/<version>/<base>/<path>` + report; on-disk untouched |
//!
//! Plus the two edge rows the table implies: a ledgered file that is
//! **missing** on disk stages (the user removed it; recreating it silently
//! would be a scan-driven write, R2), and a non-regular entry on disk (the
//! R2 sentinel) stages. A candidate with no ledger entry — a new asset — is
//! written when nothing is on disk and staged when an unledgered file is.
//!
//! [`decide`] is pure; [`plan`] reads on-disk hashes and writes nothing;
//! [`apply`] writes atomically (`fsx::write_atomic`), stages conflicts with a
//! machine-readable `report.json`, and updates the ledger in memory — the
//! caller persists it (`store::modify`). Never overwrite a user edit; never
//! freeze; idempotent: applying a plan and planning again yields no-ops for
//! everything that was written or adopted and the same stage for every
//! conflict.

use std::path::{Path, PathBuf};

use omm_host::fsx;
use omm_host::paths::OMM_SNAPSHOTS_DIR;
use serde::{Deserialize, Serialize};

use crate::audit::{Audit, Event, ACTION_UPDATE};
use crate::containment::{Bases, State};
use crate::error::{LedgerError, Result};
use crate::hash::{self, Observed};
use crate::schema::{Base, Class, Entry, EntryKey, Kind, Ledger, Mechanism, RelPath};
use crate::snapshot::{self, SnapshotReport};

/// `$OMM/updates/` (ARCHITECTURE.md §2).
pub const UPDATES_DIR: &str = "updates";
/// The machine-readable report under `updates/<version>/`.
pub const REPORT_FILE: &str = "report.json";

/// The four outcomes of R3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    NoOp,
    Overwrite,
    Adopt,
    Stage,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::NoOp => "no-op",
            Outcome::Overwrite => "overwrite",
            Outcome::Adopt => "adopt",
            Outcome::Stage => "stage",
        }
    }
}

/// The new content of one asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub base: Base,
    pub path: RelPath,
    pub kind: Kind,
    pub mechanism: Mechanism,
    pub class: Class,
    pub source_version: String,
    pub bytes: Vec<u8>,
}

impl Candidate {
    pub fn key(&self) -> EntryKey {
        EntryKey {
            base: self.base,
            path: self.path.clone(),
        }
    }
    /// `theirs`.
    pub fn sha256(&self) -> String {
        hash::sha256_bytes(&self.bytes)
    }
}

/// The pure R3 decision. `ancestor` is `None` for a candidate with no ledger
/// entry.
pub fn decide(ancestor: Option<&str>, theirs: &str, mine: &Observed) -> (Outcome, String) {
    match ancestor {
        None => match mine {
            Observed::Missing => (Outcome::Overwrite, "new asset, nothing on disk".into()),
            Observed::Content(m) if m == theirs => (
                Outcome::Adopt,
                "new asset, identical content already on disk".into(),
            ),
            Observed::Content(_) => (
                Outcome::Stage,
                "new asset, but an unledgered file is present".into(),
            ),
            Observed::NonRegular(s) => (Outcome::Stage, format!("non-regular entry on disk ({s})")),
        },
        Some(a) if a == theirs => (Outcome::NoOp, "source unchanged".into()),
        Some(a) => match mine {
            Observed::Content(m) if m == a => (Outcome::Overwrite, "user never touched it".into()),
            Observed::Content(m) if m == theirs => {
                (Outcome::Adopt, "user already has the new content".into())
            }
            Observed::Content(_) => (Outcome::Stage, "user edit on disk".into()),
            Observed::Missing => (Outcome::Stage, "missing on disk".into()),
            Observed::NonRegular(s) => (Outcome::Stage, format!("non-regular entry on disk ({s})")),
        },
    }
}

/// One planned action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub key: EntryKey,
    /// Index into [`Plan::candidates`].
    pub candidate: usize,
    pub outcome: Outcome,
    pub ancestor: Option<String>,
    pub theirs: String,
    pub mine: Observed,
    pub reason: String,
}

/// A reconcile plan: pure data, nothing written yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The `updates/<version>/` label.
    pub version: String,
    pub candidates: Vec<Candidate>,
    pub decisions: Vec<Decision>,
    /// Ledgered file entries no candidate names (removed upstream); left alone.
    pub orphans: Vec<EntryKey>,
}

impl Plan {
    pub fn count(&self, outcome: Outcome) -> usize {
        self.decisions
            .iter()
            .filter(|d| d.outcome == outcome)
            .count()
    }
    pub fn staged(&self) -> impl Iterator<Item = &Decision> {
        self.decisions
            .iter()
            .filter(|d| d.outcome == Outcome::Stage)
    }
    /// True when nothing would be written, staged or adopted.
    pub fn is_noop(&self) -> bool {
        self.decisions.iter().all(|d| d.outcome == Outcome::NoOp)
    }
}

/// A version label usable as one directory name.
pub fn check_version_label(version: &str) -> Result<()> {
    let ok = !version.is_empty()
        && version != "."
        && version != ".."
        && version
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'+' || b == b'-')
        && !version.starts_with('.');
    if ok {
        Ok(())
    } else {
        Err(LedgerError::BadVersionLabel(version.to_string()))
    }
}

/// Decide every candidate against the ledger and the disk. Reads hashes,
/// writes nothing. A containment refusal is an error: a ledger that names an
/// escaping path is not reconciled at all.
pub fn plan(
    ledger: &Ledger,
    bases: &Bases,
    version: &str,
    candidates: Vec<Candidate>,
) -> Result<Plan> {
    check_version_label(version)?;
    let mut decisions = Vec::with_capacity(candidates.len());
    for (i, c) in candidates.iter().enumerate() {
        if !c.mechanism.is_file() {
            return Err(LedgerError::Refused(format!(
                "candidate {} has mechanism {}; settings and trust are reconciled by value, not by file",
                c.key(),
                c.mechanism
            )));
        }
        let key = c.key();
        let ancestor = ledger.find(c.base, &c.path).map(|e| e.sha256.clone());
        let theirs = c.sha256();
        let resolved = bases.resolve(c.base, &c.path)?;
        let mine = match resolved.state {
            State::Missing => Observed::Missing,
            State::File | State::Dir => hash::observe(&resolved.path)?,
            State::Symlink => Observed::NonRegular(hash::sentinel("symlink", c.path.as_str())),
            State::Other => Observed::NonRegular(hash::sentinel("other", c.path.as_str())),
        };
        let (outcome, reason) = decide(ancestor.as_deref(), &theirs, &mine);
        decisions.push(Decision {
            key,
            candidate: i,
            outcome,
            ancestor,
            theirs,
            mine,
            reason,
        });
    }
    let orphans = ledger
        .file_entries()
        .map(Entry::key)
        .filter(|k| !candidates.iter().any(|c| c.key() == *k))
        .collect();
    Ok(Plan {
        version: version.to_string(),
        candidates,
        decisions,
        orphans,
    })
}

/// Options of [`apply`].
#[derive(Clone, Debug)]
pub struct ApplyOptions {
    /// `$OMM/` — staging, snapshots and the audit log live here.
    pub omm_root: PathBuf,
    /// The `writer` recorded on touched entries (`omm update`).
    pub writer: String,
    /// The `omm <version>` actor of audit lines.
    pub omm_version: String,
    /// Snapshot every ledgered file first (ARCHITECTURE.md §4). Skipped when
    /// the plan is a no-op.
    pub snapshot: bool,
    /// Decide and report, write nothing, leave the ledger untouched.
    pub dry_run: bool,
}

/// One staged conflict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Staged {
    pub base: Base,
    pub path: RelPath,
    pub ancestor: Option<String>,
    pub theirs: String,
    pub mine: Option<String>,
    pub staged_to: PathBuf,
    pub reason: String,
}

/// The machine-readable `updates/<version>/report.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageReport {
    pub schema_version: u32,
    pub version: String,
    pub ts: String,
    pub staged: Vec<Staged>,
}

/// What [`apply`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyReport {
    pub overwritten: Vec<EntryKey>,
    pub adopted: Vec<EntryKey>,
    pub staged: Vec<Staged>,
    pub noop: usize,
    /// `updates/<version>/report.json` when anything was staged.
    pub report_path: Option<PathBuf>,
    pub snapshot: Option<SnapshotReport>,
    pub dry_run: bool,
}

/// `<omm_root>/updates/<version>/<base>/<path>`.
pub fn staging_path(omm_root: &Path, version: &str, base: Base, path: &RelPath) -> PathBuf {
    path.under(&omm_root.join(UPDATES_DIR).join(version).join(base.as_str()))
}

/// Execute a plan: atomic writes for overwrites, ledger updates for adopts,
/// staged copies + `report.json` for conflicts; one audit line per action.
/// The ledger is updated in memory; persist it with `store::modify`.
pub fn apply(
    plan: &Plan,
    ledger: &mut Ledger,
    bases: &Bases,
    opts: &ApplyOptions,
) -> Result<ApplyReport> {
    check_version_label(&plan.version)?;
    let mut report = ApplyReport {
        overwritten: Vec::new(),
        adopted: Vec::new(),
        staged: Vec::new(),
        noop: plan.count(Outcome::NoOp),
        report_path: None,
        snapshot: None,
        dry_run: opts.dry_run,
    };
    if opts.dry_run {
        for d in &plan.decisions {
            match d.outcome {
                Outcome::Overwrite => report.overwritten.push(d.key.clone()),
                Outcome::Adopt => report.adopted.push(d.key.clone()),
                Outcome::Stage => report.staged.push(Staged {
                    base: d.key.base,
                    path: d.key.path.clone(),
                    ancestor: d.ancestor.clone(),
                    theirs: d.theirs.clone(),
                    mine: d.mine.text().map(str::to_string),
                    staged_to: staging_path(&opts.omm_root, &plan.version, d.key.base, &d.key.path),
                    reason: d.reason.clone(),
                }),
                Outcome::NoOp => {}
            }
        }
        return Ok(report);
    }
    let audit = Audit::new(&opts.omm_root, &opts.omm_version);
    if opts.snapshot && !plan.is_noop() {
        let snap = snapshot::take(ledger, bases, &opts.omm_root.join(OMM_SNAPSHOTS_DIR))?;
        audit.append(&Event::new(crate::audit::ACTION_SNAPSHOT).note(format!(
            "{} ({} files)",
            snap.dir.display(),
            snap.copied.len()
        )))?;
        report.snapshot = Some(snap);
    }
    for d in &plan.decisions {
        let c = plan.candidates.get(d.candidate).ok_or_else(|| {
            LedgerError::Refused(format!(
                "plan names candidate {} which does not exist",
                d.candidate
            ))
        })?;
        match d.outcome {
            Outcome::NoOp => {}
            Outcome::Overwrite => {
                let resolved = bases.resolve(c.base, &c.path)?;
                match resolved.state {
                    State::Missing | State::File => {}
                    other => {
                        return Err(LedgerError::Refused(format!(
                            "{} is {:?} on disk now; refusing to write through it",
                            d.key, other
                        )))
                    }
                }
                if let Some(parent) = resolved.path.parent() {
                    fsx::create_dir_all(parent)?;
                }
                let realpath = fsx::realpath_for_write(&resolved.path)?;
                fsx::write_atomic(&realpath, &c.bytes)?;
                upsert(ledger, c, &d.theirs, &opts.writer);
                audit.append(
                    &Event::new(ACTION_UPDATE)
                        .at(c.base, &c.path)
                        .before(d.ancestor.as_deref())
                        .after(Some(&d.theirs))
                        .note(format!("overwrite: {}", d.reason)),
                )?;
                report.overwritten.push(d.key.clone());
            }
            Outcome::Adopt => {
                upsert(ledger, c, &d.theirs, &opts.writer);
                audit.append(
                    &Event::new(ACTION_UPDATE)
                        .at(c.base, &c.path)
                        .before(d.ancestor.as_deref())
                        .after(Some(&d.theirs))
                        .note(format!("adopt: {}", d.reason)),
                )?;
                report.adopted.push(d.key.clone());
            }
            Outcome::Stage => {
                let dest = staging_path(&opts.omm_root, &plan.version, c.base, &c.path);
                if let Some(parent) = dest.parent() {
                    fsx::create_dir_all(parent)?;
                }
                let dest_is_symlink = std::fs::symlink_metadata(&dest)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                if dest_is_symlink {
                    return Err(LedgerError::Refused(format!(
                        "staging path {} is a symlink; refusing to write through it",
                        dest.display()
                    )));
                }
                let realpath = fsx::realpath_for_write(&dest)?;
                fsx::write_atomic(&realpath, &c.bytes)?;
                audit.append(
                    &Event::new(ACTION_UPDATE)
                        .at(c.base, &c.path)
                        .before(d.ancestor.as_deref())
                        .after(d.mine.text())
                        .note(format!("stage -> {}: {}", realpath.display(), d.reason)),
                )?;
                report.staged.push(Staged {
                    base: c.base,
                    path: c.path.clone(),
                    ancestor: d.ancestor.clone(),
                    theirs: d.theirs.clone(),
                    mine: d.mine.text().map(str::to_string),
                    staged_to: realpath,
                    reason: d.reason.clone(),
                });
            }
        }
    }
    if !report.staged.is_empty() {
        let dir = opts.omm_root.join(UPDATES_DIR).join(&plan.version);
        fsx::create_dir_all(&dir)?;
        let path = dir.join(REPORT_FILE);
        let doc = StageReport {
            schema_version: 1,
            version: plan.version.clone(),
            ts: crate::audit::now_ts(),
            staged: report.staged.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&doc).map_err(|e| LedgerError::Schema {
            path: path.clone(),
            detail: e.to_string(),
        })?;
        bytes.push(b'\n');
        let realpath = fsx::realpath_for_write(&path)?;
        fsx::write_atomic(&realpath, &bytes)?;
        report.report_path = Some(realpath);
    }
    Ok(report)
}

fn upsert(ledger: &mut Ledger, c: &Candidate, theirs: &str, writer: &str) {
    let prior = ledger.find(c.base, &c.path).and_then(|e| e.prior.clone());
    ledger.upsert(Entry {
        base: c.base,
        path: c.path.clone(),
        kind: c.kind,
        sha256: theirs.to_string(),
        source_version: c.source_version.clone(),
        writer: writer.to_string(),
        mechanism: c.mechanism,
        class: c.class,
        prior,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{HostInfo, Scope};

    fn sha(s: &str) -> String {
        hash::sha256_bytes(s.as_bytes())
    }

    #[test]
    fn the_four_rows_and_the_edges() {
        let a = sha("ancestor");
        let t = sha("theirs");
        let other = sha("other");
        let content = |s: &String| Observed::Content(s.clone());
        // Row 1: ancestor == theirs → no-op, whatever mine is.
        for mine in [
            content(&a),
            content(&other),
            Observed::Missing,
            Observed::NonRegular(hash::sentinel("symlink", "x")),
        ] {
            assert_eq!(decide(Some(&a), &a, &mine).0, Outcome::NoOp);
        }
        // Row 2: differ, mine == ancestor → overwrite.
        assert_eq!(decide(Some(&a), &t, &content(&a)).0, Outcome::Overwrite);
        // Row 3: differ, mine == theirs → adopt.
        assert_eq!(decide(Some(&a), &t, &content(&t)).0, Outcome::Adopt);
        // Row 4: differ, mine differs from both → stage.
        assert_eq!(decide(Some(&a), &t, &content(&other)).0, Outcome::Stage);
        // Edges: missing → stage with reason; sentinel → stage.
        let (o, r) = decide(Some(&a), &t, &Observed::Missing);
        assert_eq!((o, r.as_str()), (Outcome::Stage, "missing on disk"));
        let (o, r) = decide(
            Some(&a),
            &t,
            &Observed::NonRegular(hash::sentinel("socket", "x")),
        );
        assert_eq!(o, Outcome::Stage);
        assert!(r.contains("non-regular"));
        // No entry (new asset).
        assert_eq!(decide(None, &t, &Observed::Missing).0, Outcome::Overwrite);
        assert_eq!(decide(None, &t, &content(&t)).0, Outcome::Adopt);
        assert_eq!(decide(None, &t, &content(&other)).0, Outcome::Stage);
        assert_eq!(
            decide(None, &t, &Observed::NonRegular(hash::sentinel("fifo", ""))).0,
            Outcome::Stage
        );
    }

    /// A tiny deterministic generator (xorshift) — no dev-dependency needed.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            let mut x = self.0;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.0 = x;
            x
        }
        fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
            &items[(self.next() % items.len() as u64) as usize]
        }
    }

    #[test]
    fn property_never_clobbers_and_is_idempotent() {
        // Over random triples from a small alphabet of hashes (so equalities
        // are frequent): an edit (mine ∉ {ancestor, theirs}) is never
        // overwritten; the outcome is a pure function of the three equalities;
        // applying the outcome and deciding again is a no-op or the same stage.
        let hashes: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| sha(s)).collect();
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        for _ in 0..5000 {
            let ancestor = rng.pick(&hashes).clone();
            let theirs = rng.pick(&hashes).clone();
            let mine = match rng.next() % 6 {
                0 => Observed::Missing,
                1 => Observed::NonRegular(hash::sentinel("symlink", "p")),
                _ => Observed::Content(rng.pick(&hashes).clone()),
            };
            let has_entry = !rng.next().is_multiple_of(4);
            let anc = has_entry.then_some(ancestor.as_str());
            let (outcome, _) = decide(anc, &theirs, &mine);
            let mine_is_edit = match &mine {
                Observed::Content(m) => Some(m.as_str()) != anc && m != &theirs,
                _ => true,
            };
            if outcome == Outcome::Overwrite {
                assert!(
                    !mine_is_edit || (mine.is_missing() && !has_entry),
                    "overwrite of an edit: anc={anc:?} theirs={theirs} mine={mine:?}"
                );
            }
            if has_entry && ancestor == theirs {
                assert_eq!(outcome, Outcome::NoOp);
            }
            // Simulate apply, then decide again.
            let (anc2, mine2) = match outcome {
                Outcome::Overwrite => (Some(theirs.clone()), Observed::Content(theirs.clone())),
                Outcome::Adopt => (Some(theirs.clone()), mine.clone()),
                Outcome::NoOp | Outcome::Stage => (anc.map(str::to_string), mine.clone()),
            };
            let (again, _) = decide(anc2.as_deref(), &theirs, &mine2);
            match outcome {
                Outcome::Overwrite | Outcome::Adopt | Outcome::NoOp => {
                    assert_eq!(again, Outcome::NoOp, "second run must be a no-op: first {outcome:?} anc={anc:?} theirs={theirs} mine={mine:?}")
                }
                Outcome::Stage => assert_eq!(again, Outcome::Stage, "a conflict stays a conflict"),
            }
        }
    }

    fn fixture() -> (tempfile::TempDir, Bases, Ledger) {
        let dir = tempfile::tempdir().unwrap();
        let bases = Bases {
            muse_config: dir.path().join("config/muse"),
            muse_data: dir.path().join("data/muse"),
            omm: dir.path().join("config/omm"),
            workspace: None,
            home: Some(dir.path().join("home")),
            residue: vec![],
        };
        for p in [&bases.muse_config, &bases.muse_data, &bases.omm] {
            std::fs::create_dir_all(p).unwrap();
        }
        let ledger = Ledger::new(
            "0.1.0",
            HostInfo {
                version: "v".into(),
                sha256: "s".into(),
            },
            Scope::User,
        );
        (dir, bases, ledger)
    }

    fn cand(path: &str, bytes: &str) -> Candidate {
        Candidate {
            base: Base::MuseConfig,
            path: RelPath::new(path).unwrap(),
            kind: Kind::Skill,
            mechanism: Mechanism::Copy,
            class: Class::Exclusive,
            source_version: "0.2.0".into(),
            bytes: bytes.as_bytes().to_vec(),
        }
    }

    fn opts(bases: &Bases, dry_run: bool) -> ApplyOptions {
        ApplyOptions {
            omm_root: bases.omm.clone(),
            writer: "omm update".into(),
            omm_version: "0.2.0".into(),
            snapshot: true,
            dry_run,
        }
    }

    #[test]
    fn plan_and_apply_end_to_end_twice() {
        let (_dir, bases, mut ledger) = fixture();
        // Install v1 of four skills by applying against an empty ledger.
        let v1: Vec<Candidate> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| cand(&format!("skills/{n}/SKILL.md"), &format!("{n} v1")))
            .collect();
        let p = plan(&ledger, &bases, "0.1.0", v1.clone()).unwrap();
        assert_eq!(p.count(Outcome::Overwrite), 4);
        let mut o = opts(&bases, false);
        o.writer = "omm install".into();
        let r = apply(&p, &mut ledger, &bases, &o).unwrap();
        assert_eq!(r.overwritten.len(), 4);
        assert!(
            r.snapshot.is_some(),
            "a snapshot dir is made even for a fresh install"
        );
        assert_eq!(ledger.entries.len(), 4);
        assert!(ledger.entries.iter().all(|e| e.writer == "omm install"));
        // Idempotent: same candidates again → all no-op, nothing written.
        let again = plan(&ledger, &bases, "0.1.0", v1.clone()).unwrap();
        assert!(again.is_noop());
        let before = std::fs::read_dir(bases.omm.join("snapshots"))
            .unwrap()
            .count();
        apply(&again, &mut ledger, &bases, &o).unwrap();
        assert_eq!(
            std::fs::read_dir(bases.omm.join("snapshots"))
                .unwrap()
                .count(),
            before,
            "a no-op plan takes no snapshot"
        );

        // User edits b, already has v2 of c, deletes d; a is untouched.
        std::fs::write(bases.muse_config.join("skills/b/SKILL.md"), b"b edited").unwrap();
        std::fs::write(bases.muse_config.join("skills/c/SKILL.md"), b"c v2").unwrap();
        std::fs::remove_file(bases.muse_config.join("skills/d/SKILL.md")).unwrap();
        let v2: Vec<Candidate> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| cand(&format!("skills/{n}/SKILL.md"), &format!("{n} v2")))
            .collect();
        let mut v2 = v2;
        v2.push(cand("skills/e/SKILL.md", "e v2")); // new asset
        let p = plan(&ledger, &bases, "0.2.0", v2.clone()).unwrap();
        let by_path = |n: &str| {
            p.decisions
                .iter()
                .find(|d| d.key.path.as_str() == format!("skills/{n}/SKILL.md"))
                .unwrap()
        };
        assert_eq!(by_path("a").outcome, Outcome::Overwrite);
        assert_eq!(by_path("b").outcome, Outcome::Stage);
        assert_eq!(by_path("c").outcome, Outcome::Adopt);
        assert_eq!(by_path("d").outcome, Outcome::Stage);
        assert_eq!(by_path("d").reason, "missing on disk");
        assert_eq!(by_path("e").outcome, Outcome::Overwrite);
        assert!(p.orphans.is_empty());

        // Dry run: nothing changes.
        let ledger_before = ledger.clone();
        let dry = apply(&p, &mut ledger, &bases, &opts(&bases, true)).unwrap();
        assert!(dry.dry_run && dry.staged.len() == 2 && dry.overwritten.len() == 2);
        assert_eq!(ledger, ledger_before);
        assert!(!bases.omm.join("updates").exists());
        assert_eq!(
            std::fs::read(bases.muse_config.join("skills/a/SKILL.md")).unwrap(),
            b"a v1"
        );

        let r = apply(&p, &mut ledger, &bases, &opts(&bases, false)).unwrap();
        assert_eq!(r.overwritten.len(), 2);
        assert_eq!(r.adopted.len(), 1);
        assert_eq!(r.staged.len(), 2);
        assert_eq!(
            std::fs::read(bases.muse_config.join("skills/a/SKILL.md")).unwrap(),
            b"a v2"
        );
        assert_eq!(
            std::fs::read(bases.muse_config.join("skills/b/SKILL.md")).unwrap(),
            b"b edited",
            "the user edit is untouched"
        );
        assert!(!bases.muse_config.join("skills/d/SKILL.md").exists());
        assert_eq!(
            std::fs::read(bases.muse_config.join("skills/e/SKILL.md")).unwrap(),
            b"e v2"
        );
        let staged_b = bases
            .omm
            .join("updates/0.2.0/muse-config/skills/b/SKILL.md");
        assert_eq!(std::fs::read(&staged_b).unwrap(), b"b v2");
        let report_path = r.report_path.clone().unwrap();
        assert_eq!(
            report_path,
            std::fs::canonicalize(bases.omm.join("updates/0.2.0"))
                .unwrap()
                .join(REPORT_FILE)
        );
        let doc: StageReport =
            serde_json::from_slice(&std::fs::read(&report_path).unwrap()).unwrap();
        assert_eq!(doc.staged.len(), 2);
        assert!(doc
            .staged
            .iter()
            .any(|s| s.path.as_str() == "skills/b/SKILL.md" && s.reason == "user edit on disk"));
        // Snapshot holds the pre-update bytes of every file that existed.
        let snap = r.snapshot.unwrap();
        assert_eq!(snap.copied.len(), 3, "{snap:?}");
        let a_snap = snap
            .copied
            .iter()
            .find(|(k, _)| k.path.as_str() == "skills/a/SKILL.md")
            .unwrap();
        assert_eq!(std::fs::read(&a_snap.1).unwrap(), b"a v1");
        // Ledger: a and e at v2 sha, c adopted, b and d unchanged (still v1 ancestor).
        let e = |n: &str| {
            ledger
                .find(
                    Base::MuseConfig,
                    &RelPath::new(format!("skills/{n}/SKILL.md")).unwrap(),
                )
                .unwrap()
                .clone()
        };
        assert_eq!(e("a").sha256, sha("a v2"));
        assert_eq!(e("a").writer, "omm update");
        assert_eq!(e("a").source_version, "0.2.0");
        assert_eq!(e("c").sha256, sha("c v2"));
        assert_eq!(e("b").sha256, sha("b v1"));
        assert_eq!(e("d").sha256, sha("d v1"));
        assert_eq!(e("e").sha256, sha("e v2"));
        // Reconcile twice == once: a second plan is no-op except the two
        // stages, and applying it leaves everything byte-identical.
        let p2 = plan(&ledger, &bases, "0.2.0", v2.clone()).unwrap();
        assert_eq!(p2.count(Outcome::NoOp), 3);
        assert_eq!(p2.count(Outcome::Stage), 2);
        let ledger_before = ledger.clone();
        let staged_before = std::fs::read(&staged_b).unwrap();
        apply(&p2, &mut ledger, &bases, &opts(&bases, false)).unwrap();
        assert_eq!(ledger, ledger_before);
        assert_eq!(std::fs::read(&staged_b).unwrap(), staged_before);
        // Audit lines were written for every non-no-op action.
        let lines = crate::audit::read(&bases.omm).unwrap();
        assert!(lines.iter().filter(|l| l.action == ACTION_UPDATE).count() >= 5 + 2);
        assert!(lines.iter().all(|l| l.actor.starts_with("omm ")));
        // Orphans: a ledgered file no candidate names is reported, untouched.
        let p3 = plan(
            &ledger,
            &bases,
            "0.3.0",
            vec![cand("skills/a/SKILL.md", "a v2")],
        )
        .unwrap();
        assert_eq!(p3.orphans.len(), 4);
    }

    #[test]
    fn refusals() {
        let (_dir, bases, ledger) = fixture();
        assert!(matches!(
            plan(&ledger, &bases, "../x", vec![]),
            Err(LedgerError::BadVersionLabel(_))
        ));
        assert!(matches!(
            plan(&ledger, &bases, "", vec![]),
            Err(LedgerError::BadVersionLabel(_))
        ));
        assert!(plan(&ledger, &bases, "0.1.0-rc.1+build", vec![]).is_ok());
        let mut c = cand("settings.json", "{}");
        c.mechanism = Mechanism::SettingsPatch;
        assert!(matches!(
            plan(&ledger, &bases, "0.1.0", vec![c]),
            Err(LedgerError::Refused(_))
        ));
        // A workspace candidate without a workspace is a containment error.
        let mut c = cand("x", "y");
        c.base = Base::Workspace;
        assert!(matches!(
            plan(&ledger, &bases, "0.1.0", vec![c]),
            Err(LedgerError::NoBaseRoot(Base::Workspace))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_at_the_path_is_staged_never_written_through() {
        let (dir, bases, mut ledger) = fixture();
        let outside = dir.path().join("outside.md");
        std::fs::write(&outside, b"precious").unwrap();
        std::fs::create_dir_all(bases.muse_config.join("skills/x")).unwrap();
        std::os::unix::fs::symlink(&outside, bases.muse_config.join("skills/x/SKILL.md")).unwrap();
        let p = plan(
            &ledger,
            &bases,
            "0.1.0",
            vec![cand("skills/x/SKILL.md", "new")],
        )
        .unwrap();
        assert_eq!(p.decisions[0].outcome, Outcome::Stage);
        assert!(matches!(p.decisions[0].mine, Observed::NonRegular(_)));
        apply(&p, &mut ledger, &bases, &opts(&bases, false)).unwrap();
        assert_eq!(std::fs::read(&outside).unwrap(), b"precious");
        assert!(ledger.entries.is_empty());
    }
}
