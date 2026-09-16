//! Rolling pre-update snapshots (ARCHITECTURE.md §4): before any update every
//! ledgered file is copied to `$OMM/snapshots/<timestamp>[-n]/<base>/<path>`
//! (verified copy, `omm_host::fsx::verified_copy`); the newest
//! [`omm_host::fsx::SNAPSHOTS_KEEP`] snapshot directories are kept. The
//! directory naming and pruning are `fsx::new_snapshot_dir` /
//! `fsx::prune_snapshots`, shared with the settings/trust pre-write backups
//! so both kinds roll together.
//!
//! Non-file entries (settings / trust) and anything that is not a regular
//! file on disk (missing, symlink, socket — the R2 sentinel) are skipped and
//! named in the report, never followed.

use std::path::{Path, PathBuf};

use omm_host::fsx;

use crate::containment::{Bases, State};
use crate::error::Result;
use crate::schema::{EntryKey, Ledger};

/// What one snapshot did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnapshotReport {
    /// `<snapshots_dir>/<timestamp>[-n]/`.
    pub dir: PathBuf,
    /// Entries copied, with the copy's path.
    pub copied: Vec<(EntryKey, PathBuf)>,
    /// Entries skipped, with why.
    pub skipped: Vec<(EntryKey, String)>,
    /// Older snapshot directories removed by the roll.
    pub pruned: Vec<PathBuf>,
}

/// Copy every ledgered file into a fresh snapshot directory under
/// `snapshots_dir` (`Roots::snapshots_dir()`), then roll to the newest five.
pub fn take(ledger: &Ledger, bases: &Bases, snapshots_dir: &Path) -> Result<SnapshotReport> {
    let dir = fsx::new_snapshot_dir(snapshots_dir)?;
    let mut report = SnapshotReport {
        dir: dir.clone(),
        copied: Vec::new(),
        skipped: Vec::new(),
        pruned: Vec::new(),
    };
    for entry in &ledger.entries {
        let key = entry.key();
        if !entry.is_file() {
            report
                .skipped
                .push((key, format!("{} entry is not a file", entry.mechanism)));
            continue;
        }
        let resolved = match bases.resolve(entry.base, &entry.path) {
            Ok(r) => r,
            Err(e) => {
                report.skipped.push((key, e.to_string()));
                continue;
            }
        };
        match resolved.state {
            State::File => {}
            State::Missing => {
                report.skipped.push((key, "missing on disk".to_string()));
                continue;
            }
            State::Dir => {
                report
                    .skipped
                    .push((key, "a directory, not a file".to_string()));
                continue;
            }
            State::Symlink | State::Other => {
                report
                    .skipped
                    .push((key, "non-regular entry on disk (R2 sentinel)".to_string()));
                continue;
            }
        }
        let dest = entry.path.under(&dir.join(entry.base.as_str()));
        if let Some(parent) = dest.parent() {
            fsx::create_dir_all(parent)?;
        }
        fsx::verified_copy(&resolved.path, &dest)?;
        report.copied.push((key, dest));
    }
    report.pruned = fsx::prune_snapshots(snapshots_dir, fsx::SNAPSHOTS_KEEP)?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Base, Class, Entry, HostInfo, Kind, Mechanism, RelPath, Scope};

    fn entry(base: Base, path: &str, mechanism: Mechanism) -> Entry {
        Entry {
            base,
            path: RelPath::new(path).unwrap(),
            kind: Kind::Skill,
            sha256: "x".into(),
            source_version: "0.1.0".into(),
            writer: "omm install".into(),
            mechanism,
            class: Class::Exclusive,
            prior: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn copies_files_skips_the_rest_and_rolls_to_five() {
        let dir = tempfile::tempdir().unwrap();
        let bases = Bases {
            muse_config: dir.path().join("config/muse"),
            muse_data: dir.path().join("data/muse"),
            omm: dir.path().join("config/omm"),
            workspace: None,
            home: None,
            residue: vec![],
        };
        std::fs::create_dir_all(bases.muse_config.join("skills/x")).unwrap();
        std::fs::create_dir_all(&bases.muse_data).unwrap();
        std::fs::create_dir_all(&bases.omm).unwrap();
        std::fs::write(bases.muse_config.join("skills/x/SKILL.md"), b"skill").unwrap();
        std::os::unix::fs::symlink("SKILL.md", bases.muse_config.join("skills/x/link")).unwrap();
        let mut ledger = Ledger::new(
            "0.1.0",
            HostInfo {
                version: "v".into(),
                sha256: "s".into(),
            },
            Scope::User,
        );
        ledger.entries = vec![
            entry(Base::MuseConfig, "skills/x/SKILL.md", Mechanism::Copy),
            entry(Base::MuseConfig, "skills/x/link", Mechanism::Copy),
            entry(Base::MuseConfig, "skills/x", Mechanism::Copy),
            entry(Base::MuseConfig, "gone.md", Mechanism::Copy),
            entry(Base::MuseConfig, "settings.json", Mechanism::SettingsPatch),
            entry(Base::Workspace, "a", Mechanism::Copy),
        ];
        let snaps = bases.omm.join("snapshots");
        let r = take(&ledger, &bases, &snaps).unwrap();
        assert_eq!(r.copied.len(), 1);
        assert!(r.copied[0].1.ends_with("muse-config/skills/x/SKILL.md"));
        assert!(r.copied[0].1.starts_with(&r.dir));
        assert_eq!(std::fs::read(&r.copied[0].1).unwrap(), b"skill");
        let reasons: Vec<&str> = r.skipped.iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(reasons.len(), 5, "{reasons:?}");
        assert!(reasons.iter().any(|s| s.contains("non-regular")));
        assert!(reasons.iter().any(|s| s.contains("directory")));
        assert!(reasons.iter().any(|s| s.contains("missing")));
        assert!(reasons.iter().any(|s| s.contains("not a file")));
        assert!(reasons.iter().any(|s| s.contains("workspace")));
        for _ in 0..6 {
            take(&ledger, &bases, &snaps).unwrap();
        }
        let dirs = snaps.read_dir().unwrap().count();
        assert_eq!(dirs, fsx::SNAPSHOTS_KEEP);
        assert!(!r.dir.exists(), "the first snapshot rolled off");
    }
}
