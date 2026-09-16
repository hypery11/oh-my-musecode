//! Load and save `omm.lock.json`.
//!
//! * A corrupt ledger (unparseable, unknown field, wrong schema version,
//!   duplicate key, invalid path) is renamed `omm.lock.json.bad-<ts>` —
//!   never deleted — reported as [`Loaded::Absent`] with a [`Quarantined`]
//!   record, written to the audit log, and then treated as absent.
//! * Saves are `omm_host::fsx::write_atomic` on the realpath, entries
//!   re-sorted (ARCHITECTURE.md §4: byte-stable reruns).
//! * [`modify`] wraps load–modify–save in the exclusive advisory lock
//!   `$OMM/locks/ledger.lock` (`fsx::lock_exclusive`, flock(2)), so two omm
//!   processes cannot interleave their writes.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omm_host::fsx::{self, FileLock};
use omm_host::paths::OMM_LOCKS_DIR;

use crate::audit::{Audit, Event, ACTION_QUARANTINE};
use crate::error::{LedgerError, Result};
use crate::schema::Ledger;

/// `$OMM/omm.lock.json`.
pub const LEDGER_FILE: &str = "omm.lock.json";
/// `$OMM/locks/ledger.lock`.
pub const LEDGER_LOCK: &str = "ledger.lock";
/// Prefix of a quarantined ledger: `omm.lock.json.bad-<ts>[-n]`.
pub const BAD_SUFFIX: &str = ".bad-";

/// `<omm_root>/omm.lock.json`.
pub fn ledger_path(omm_root: &Path) -> PathBuf {
    omm_root.join(LEDGER_FILE)
}

/// `<omm_root>/locks/ledger.lock`.
pub fn lock_path(omm_root: &Path) -> PathBuf {
    omm_root.join(OMM_LOCKS_DIR).join(LEDGER_LOCK)
}

/// True for a file name this module produced by quarantining.
pub fn is_quarantine_name(name: &str) -> bool {
    name.strip_prefix(LEDGER_FILE)
        .and_then(|rest| rest.strip_prefix(BAD_SUFFIX))
        .map(|ts| !ts.is_empty())
        .unwrap_or(false)
}

/// A corrupt ledger that was moved aside.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quarantined {
    pub from: PathBuf,
    pub to: PathBuf,
    pub detail: String,
}

impl Quarantined {
    /// The loud message.
    pub fn message(&self) -> String {
        format!(
            "WARNING: the ledger {} is corrupt ({}); it was moved to {} and omm now treats the install as absent. Doctor D13 reports this; the moved file keeps every record for hand recovery.",
            self.from.display(),
            self.detail,
            self.to.display()
        )
    }
}

/// The result of [`load`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Loaded {
    Present(Ledger),
    /// No ledger; `quarantined` is set when one was just moved aside.
    Absent {
        quarantined: Option<Quarantined>,
    },
}

impl Loaded {
    pub fn ledger(&self) -> Option<&Ledger> {
        match self {
            Loaded::Present(l) => Some(l),
            Loaded::Absent { .. } => None,
        }
    }
    pub fn into_ledger(self) -> Option<Ledger> {
        match self {
            Loaded::Present(l) => Some(l),
            Loaded::Absent { .. } => None,
        }
    }
    pub fn quarantined(&self) -> Option<&Quarantined> {
        match self {
            Loaded::Absent { quarantined } => quarantined.as_ref(),
            Loaded::Present(_) => None,
        }
    }
}

/// Load the ledger of `omm_root`. A missing file is `Absent`; a corrupt one
/// is quarantined (see the module docs) and `Absent { quarantined: Some }`.
pub fn load(omm_root: &Path) -> Result<Loaded> {
    let path = ledger_path(omm_root);
    let bytes = match fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Loaded::Absent { quarantined: None })
        }
        Err(e) => return Err(LedgerError::io("read ledger", &path, e)),
    };
    match Ledger::from_bytes(&bytes, &path) {
        Ok(ledger) => Ok(Loaded::Present(ledger)),
        Err(LedgerError::Schema { detail, .. }) => {
            let to = quarantine(&path)?;
            let q = Quarantined {
                from: path,
                to,
                detail,
            };
            // Loud and durable: the audit log carries it even if nobody
            // prints `message()`. The audit write must not mask the load.
            let _ = Audit::new(omm_root, "?").append(&Event::new(ACTION_QUARANTINE).note(format!(
                "{} -> {}: {}",
                q.from.display(),
                q.to.display(),
                q.detail
            )));
            Ok(Loaded::Absent {
                quarantined: Some(q),
            })
        }
        Err(e) => Err(e),
    }
}

/// Rename `path` to `<path>.bad-<ts>[-n]`; never overwrites an earlier
/// quarantine.
fn quarantine(path: &Path) -> Result<PathBuf> {
    let stamp = fsx::timestamp();
    let base = format!(
        "{}{BAD_SUFFIX}{stamp}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(LEDGER_FILE)
    );
    let parent = path.parent().unwrap_or(Path::new("."));
    let mut n = 0u32;
    loop {
        let candidate = if n == 0 {
            parent.join(&base)
        } else {
            parent.join(format!("{base}-{n}"))
        };
        if candidate.exists() {
            n += 1;
            continue;
        }
        fs::rename(path, &candidate).map_err(|e| LedgerError::io("quarantine ledger", path, e))?;
        return Ok(candidate);
    }
}

/// Save atomically on the realpath; entries are sorted by `to_bytes`. The
/// ledger is validated first — a document this crate would quarantine on
/// load is never written.
pub fn save(omm_root: &Path, ledger: &Ledger) -> Result<PathBuf> {
    let path = ledger_path(omm_root);
    ledger.validate().map_err(|detail| LedgerError::Schema {
        path: path.clone(),
        detail,
    })?;
    fsx::create_dir_all(omm_root)?;
    let realpath = fsx::realpath_for_write(&path)?;
    fsx::write_atomic(&realpath, &ledger.to_bytes()?)?;
    Ok(realpath)
}

/// Remove the ledger file (uninstall's last step). Missing is fine.
pub fn remove(omm_root: &Path) -> Result<bool> {
    let path = ledger_path(omm_root);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(LedgerError::io("remove ledger", &path, e)),
    }
}

/// Take the ledger lock (created with `$OMM/locks/`), waiting up to `wait`.
pub fn lock(omm_root: &Path, wait: Duration) -> Result<FileLock> {
    Ok(fsx::lock_exclusive(&lock_path(omm_root), wait)?)
}

/// Load–modify–save under the ledger lock. `f` sees `None` when no ledger is
/// present (a corrupt one has already been quarantined) and may create one;
/// leaving `None` removes the file. Returns `f`'s value and any quarantine.
pub fn modify<T>(
    omm_root: &Path,
    f: impl FnOnce(&mut Option<Ledger>) -> Result<T>,
) -> Result<(T, Option<Quarantined>)> {
    let _lock = lock(omm_root, fsx::LOCK_WAIT_DEFAULT)?;
    let loaded = load(omm_root)?;
    let quarantined = loaded.quarantined().cloned();
    let mut slot = loaded.into_ledger();
    let out = f(&mut slot)?;
    match &slot {
        Some(ledger) => {
            save(omm_root, ledger)?;
        }
        None => {
            remove(omm_root)?;
        }
    }
    Ok((out, quarantined))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Base, Class, Entry, HostInfo, Kind, Mechanism, RelPath, Scope};

    fn ledger() -> Ledger {
        Ledger::new(
            "0.1.0",
            HostInfo {
                version: "v".into(),
                sha256: "s".into(),
            },
            Scope::User,
        )
    }

    fn entry(path: &str) -> Entry {
        Entry {
            base: Base::MuseConfig,
            path: RelPath::new(path).unwrap(),
            kind: Kind::Skill,
            sha256: "0".repeat(64),
            source_version: "0.1.0".into(),
            writer: "omm install".into(),
            mechanism: Mechanism::Copy,
            class: Class::Exclusive,
            prior: None,
        }
    }

    #[test]
    fn absent_save_load_roundtrip_sorted() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("omm");
        assert_eq!(load(&root).unwrap(), Loaded::Absent { quarantined: None });
        let mut l = ledger();
        l.entries.push(entry("z"));
        l.entries.push(entry("a"));
        let written = save(&root, &l).unwrap();
        assert_eq!(written, fs::canonicalize(&root).unwrap().join(LEDGER_FILE));
        let back = load(&root).unwrap().into_ledger().unwrap();
        assert!(back.is_sorted());
        assert_eq!(back.entries.len(), 2);
        // Byte-stable: saving the loaded ledger again changes nothing.
        let b1 = fs::read(&written).unwrap();
        save(&root, &back).unwrap();
        assert_eq!(fs::read(&written).unwrap(), b1);
        // Nothing beside the ledger but what we own.
        let names: Vec<String> = root
            .read_dir()
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec![LEDGER_FILE]);
        assert!(remove(&root).unwrap());
        assert!(!remove(&root).unwrap());
    }

    #[test]
    fn corrupt_ledger_is_quarantined_and_absent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("omm");
        fs::create_dir_all(&root).unwrap();
        fs::write(ledger_path(&root), b"{ not json").unwrap();
        let loaded = load(&root).unwrap();
        let q = loaded.quarantined().expect("quarantined").clone();
        assert!(loaded.ledger().is_none());
        assert_eq!(q.from, ledger_path(&root));
        assert!(!q.from.exists(), "the corrupt file was moved, not copied");
        assert!(q.to.exists());
        let name = q.to.file_name().unwrap().to_str().unwrap().to_string();
        assert!(is_quarantine_name(&name), "{name}");
        assert!(!is_quarantine_name(LEDGER_FILE));
        assert!(!is_quarantine_name("omm.lock.json.bad-"));
        assert_eq!(fs::read(&q.to).unwrap(), b"{ not json");
        assert!(q.message().contains("WARNING"));
        // A second corrupt file in the same second gets its own name.
        fs::write(ledger_path(&root), b"[]").unwrap();
        let q2 = load(&root).unwrap().quarantined().unwrap().clone();
        assert_ne!(q2.to, q.to);
        assert!(q.to.exists() && q2.to.exists());
        // Loud and durable: the audit log has both.
        let lines = crate::audit::read(&root).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.action == ACTION_QUARANTINE));
        // Now absent, plainly.
        assert_eq!(load(&root).unwrap(), Loaded::Absent { quarantined: None });
        // Schema-level corruption counts too: unknown field, wrong version,
        // duplicate key, escaping path.
        for bad in [
            r#"{"schema_version":1,"omm_version":"0","host":{"version":"v","sha256":"s"},"scope":"user","entries":[],"registrations":[],"x":1}"#,
            r#"{"schema_version":2,"omm_version":"0","host":{"version":"v","sha256":"s"},"scope":"user"}"#,
            r#"{"schema_version":1,"omm_version":"0","host":{"version":"v","sha256":"s"},"scope":"user","entries":[{"base":"omm","path":"../x","kind":"skill","sha256":"","source_version":"","writer":"","mechanism":"copy","class":"exclusive"}]}"#,
        ] {
            fs::write(ledger_path(&root), bad).unwrap();
            assert!(load(&root).unwrap().quarantined().is_some(), "{bad}");
        }
        // Saving an invalid ledger is refused before any byte lands.
        let mut dup = ledger();
        dup.entries.push(entry("a"));
        dup.entries.push(entry("a"));
        assert!(matches!(save(&root, &dup), Err(LedgerError::Schema { .. })));
        assert!(!ledger_path(&root).exists());
    }

    #[test]
    fn modify_locks_creates_and_removes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("omm");
        let (n, q) = modify(&root, |slot| {
            assert!(slot.is_none());
            let mut l = ledger();
            l.entries.push(entry("a"));
            *slot = Some(l);
            Ok(1)
        })
        .unwrap();
        assert_eq!((n, q), (1, None));
        assert!(ledger_path(&root).exists());
        assert!(lock_path(&root).exists());
        // Held elsewhere: modify waits, then refuses.
        let held = lock(&root, Duration::from_millis(10)).unwrap();
        let t = std::thread::spawn({
            let root = root.clone();
            move || modify(&root, |slot| Ok(slot.as_ref().map(|l| l.entries.len())))
        });
        std::thread::sleep(Duration::from_millis(100));
        assert!(!t.is_finished());
        drop(held);
        assert_eq!(t.join().unwrap().unwrap().0, Some(1));
        // Leaving None removes the file.
        modify(&root, |slot| {
            *slot = None;
            Ok(())
        })
        .unwrap();
        assert!(!ledger_path(&root).exists());
    }
}
