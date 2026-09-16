//! `$OMM/audit.log` — append-only JSONL, one line per install / update /
//! uninstall / approve / quarantine action, in the shape of the host's own
//! `skills/.muse/audit.log` (`research/musecode/config-paths.md` §3.2,
//! verbatim: `{"time":"2026-09-01T14:31:33.643375Z","action":"install",
//! "skill":"myskill","source":"local","result":"ok"}` — one object per line,
//! a UTC timestamp with microseconds, an `action`). Ours:
//!
//! ```json
//! {"ts":"…","action":"update","actor":"omm 0.1.0","base":"muse-config","path":"skills/x/SKILL.md",
//!  "sha256_before":"…","sha256_after":"…","note":"overwrite"}
//! ```
//!
//! Every key is always present (`null` when it does not apply) so the line
//! shape is fixed. A line is one `write(2)` on a descriptor opened with
//! `O_APPEND`, which is the atomic primitive for an append-only file — an
//! `fsx::write_atomic` rename would replace the whole log and lose a
//! concurrent writer's line.

use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{LedgerError, Result};
use crate::schema::{Base, RelPath};

/// `$OMM/audit.log` (ARCHITECTURE.md §2).
pub const AUDIT_FILE: &str = "audit.log";

/// `action` values this crate writes.
pub const ACTION_INSTALL: &str = "install";
pub const ACTION_UPDATE: &str = "update";
pub const ACTION_UNINSTALL: &str = "uninstall";
pub const ACTION_APPROVE: &str = "approve";
pub const ACTION_SNAPSHOT: &str = "snapshot";
pub const ACTION_QUARANTINE: &str = "quarantine";

/// One audit line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditLine {
    /// UTC, `YYYY-MM-DDTHH:MM:SS.ffffffZ`.
    pub ts: String,
    pub action: String,
    /// `omm <version>`.
    pub actor: String,
    pub base: Option<String>,
    pub path: Option<String>,
    pub sha256_before: Option<String>,
    pub sha256_after: Option<String>,
    pub note: Option<String>,
}

/// What to record; every field but `action` is optional.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Event {
    pub action: String,
    pub base: Option<Base>,
    pub path: Option<RelPath>,
    pub sha256_before: Option<String>,
    pub sha256_after: Option<String>,
    pub note: Option<String>,
}

impl Event {
    pub fn new(action: &str) -> Event {
        Event {
            action: action.to_string(),
            ..Event::default()
        }
    }
    pub fn at(mut self, base: Base, path: &RelPath) -> Event {
        self.base = Some(base);
        self.path = Some(path.clone());
        self
    }
    pub fn before(mut self, sha: Option<&str>) -> Event {
        self.sha256_before = sha.map(str::to_string);
        self
    }
    pub fn after(mut self, sha: Option<&str>) -> Event {
        self.sha256_after = sha.map(str::to_string);
        self
    }
    pub fn note(mut self, note: impl Into<String>) -> Event {
        self.note = Some(note.into());
        self
    }
}

/// The writer.
#[derive(Clone, Debug)]
pub struct Audit {
    path: PathBuf,
    actor: String,
}

impl Audit {
    /// `<omm_root>/audit.log`, acting as `omm <omm_version>`.
    pub fn new(omm_root: &Path, omm_version: &str) -> Audit {
        Audit {
            path: omm_root.join(AUDIT_FILE),
            actor: format!("omm {omm_version}"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn actor(&self) -> &str {
        &self.actor
    }

    /// The line an event becomes (stamped now).
    pub fn line(&self, ev: &Event) -> AuditLine {
        AuditLine {
            ts: now_ts(),
            action: ev.action.clone(),
            actor: self.actor.clone(),
            base: ev.base.map(|b| b.as_str().to_string()),
            path: ev.path.as_ref().map(|p| p.as_str().to_string()),
            sha256_before: ev.sha256_before.clone(),
            sha256_after: ev.sha256_after.clone(),
            note: ev.note.clone(),
        }
    }

    /// Append one line (creating the file and `$OMM/` if needed).
    pub fn append(&self, ev: &Event) -> Result<AuditLine> {
        let line = self.line(ev);
        let mut text = serde_json::to_string(&line).map_err(|e| LedgerError::Schema {
            path: self.path.clone(),
            detail: format!("audit line: {e}"),
        })?;
        text.push('\n');
        if let Some(parent) = self.path.parent() {
            omm_host::fsx::create_dir_all(parent)?;
        }
        let mut f = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .map_err(|e| LedgerError::io("open audit log", &self.path, e))?;
        f.write_all(text.as_bytes())
            .map_err(|e| LedgerError::io("append audit log", &self.path, e))?;
        f.flush()
            .map_err(|e| LedgerError::io("flush audit log", &self.path, e))?;
        Ok(line)
    }
}

/// Every line of `<omm_root>/audit.log` (missing file = empty). A line that
/// does not parse is returned as an error naming its number.
pub fn read(omm_root: &Path) -> Result<Vec<AuditLine>> {
    let path = omm_root.join(AUDIT_FILE);
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(LedgerError::io("open audit log", &path, e)),
    };
    let mut out = Vec::new();
    for (n, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|e| LedgerError::io("read audit log", &path, e))?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed: AuditLine = serde_json::from_str(&line).map_err(|e| LedgerError::Schema {
            path: path.clone(),
            detail: format!("line {}: {e}", n + 1),
        })?;
        out.push(parsed);
    }
    Ok(out)
}

/// UTC now as `YYYY-MM-DDTHH:MM:SS.ffffffZ` — the host's audit stamp shape.
pub fn now_ts() -> String {
    let fmt = time::macros::format_description!(
        "[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:6]Z"
    );
    time::OffsetDateTime::now_utc()
        .format(&fmt)
        .unwrap_or_else(|_| "0000-00-00T00:00:00.000000Z".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_shape_matches_the_contract_and_appends() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("omm");
        let audit = Audit::new(&root, "0.1.0");
        assert_eq!(audit.actor(), "omm 0.1.0");
        let ev = Event::new(ACTION_UPDATE)
            .at(
                Base::MuseConfig,
                &RelPath::new("skills/x/SKILL.md").unwrap(),
            )
            .before(Some("aaa"))
            .after(Some("bbb"))
            .note("overwrite");
        let line = audit.append(&ev).unwrap();
        audit.append(&Event::new(ACTION_INSTALL)).unwrap();
        let text = std::fs::read_to_string(audit.path()).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        let v: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "ts",
                "action",
                "actor",
                "base",
                "path",
                "sha256_before",
                "sha256_after",
                "note"
            ]
        );
        assert_eq!(v["action"], "update");
        assert_eq!(v["actor"], "omm 0.1.0");
        assert_eq!(v["base"], "muse-config");
        assert_eq!(v["path"], "skills/x/SKILL.md");
        assert_eq!(v["sha256_before"], "aaa");
        assert_eq!(v["sha256_after"], "bbb");
        assert_eq!(v["note"], "overwrite");
        let ts = v["ts"].as_str().unwrap();
        assert_eq!(ts.len(), "2026-09-01T14:31:33.643375Z".len(), "{ts}");
        assert!(ts.ends_with('Z') && ts.as_bytes()[10] == b'T' && ts.as_bytes()[19] == b'.');
        assert_eq!(line.ts, ts);
        // Every key present on the minimal line too, as null.
        let v2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert!(v2["base"].is_null() && v2["note"].is_null());
        assert_eq!(read(&root).unwrap().len(), 2);
        assert_eq!(read(&dir.path().join("nothing")).unwrap().len(), 0);
        // A broken line is an error naming its number.
        std::fs::write(audit.path(), "{\"ts\":1}\n").unwrap();
        match read(&root) {
            Err(LedgerError::Schema { detail, .. }) => {
                assert!(detail.starts_with("line 1"), "{detail}")
            }
            other => panic!("{other:?}"),
        }
    }
}
