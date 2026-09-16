//! The pre-omm record of a shared file — `settings.json`, `trust.json` —
//! and the merge that puts the user's untyped keys back after a host rewrite.
//!
//! Neither file is omm's: both are written by targeted merges (R10), and the
//! host rewrites `settings.json` from its typed struct on `plugins install /
//! approve / remove` and `skills uninstall`, destroying every key it does
//! not type and landing the file at the temp file's mode (Gate 1: a user's
//! 0600 `settings.json` with two untyped keys came back 0644 without them;
//! a compact one-line `trust.json` came back pretty-printed). So the first
//! time omm touches a shared file that already exists, its exact bytes and
//! permission bits go into the file's ledger entry as `prior.original`
//! ([`Original::to_prior`]); uninstall, after every host step and key
//! restore, merges the untyped members back and — when the document then
//! equals the pre-omm one — writes those bytes back at that mode, else
//! keeps the merged document and re-applies the mode (`uninstall`). The
//! record lives in the ledger, never only in `$OMM/snapshots/` (rolling,
//! and removed by the uninstall itself).

use std::path::Path;

use serde_json::{Map, Value};

use omm_host::fsx;

use crate::error::{LedgerError, Result};
use crate::schema::{Base, Class, Entry, Kind, Ledger, Mechanism, RelPath};

/// The `prior` key holding the record.
pub const PRIOR_ORIGINAL: &str = "original";
/// `original.sha256` — the recorded bytes' hash.
pub const ORIGINAL_SHA256: &str = "sha256";
/// `original.mode` — the permission bits as four octal digits (`"0600"`).
pub const ORIGINAL_MODE: &str = "mode";
/// `original.text` — the bytes when UTF-8.
pub const ORIGINAL_TEXT: &str = "text";
/// `original.hex` — the bytes hex-encoded when not.
pub const ORIGINAL_HEX: &str = "hex";

/// A shared file as omm found it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Original {
    pub sha256: String,
    /// The permission bits (`0o600`); `None` where the platform has none.
    pub mode: Option<u32>,
    pub bytes: Vec<u8>,
}

impl Original {
    /// Read `path` as it stands; `None` when no regular file is there (a
    /// dangling symlink included — the writers refuse it anyway).
    pub fn read(path: &Path) -> Result<Option<Original>> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LedgerError::io("stat", path, e)),
        };
        if !meta.is_file() {
            return Ok(None);
        }
        let bytes = fsx::read_bytes(path)?;
        Ok(Some(Original {
            sha256: fsx::sha256_bytes(&bytes),
            mode: file_mode(&meta),
            bytes,
        }))
    }

    /// The record as the ledger stores it: `{"original": {sha256, mode, text|hex}}`.
    pub fn to_prior(&self) -> Value {
        let mut m = Map::new();
        m.insert(PRIOR_ORIGINAL.to_string(), self.to_value());
        Value::Object(m)
    }

    /// The `original` object alone.
    pub fn to_value(&self) -> Value {
        let mut m = Map::new();
        m.insert(
            ORIGINAL_SHA256.to_string(),
            Value::String(self.sha256.clone()),
        );
        m.insert(
            ORIGINAL_MODE.to_string(),
            self.mode
                .map(|mode| Value::String(format!("{mode:04o}")))
                .unwrap_or(Value::Null),
        );
        match std::str::from_utf8(&self.bytes) {
            Ok(text) => m.insert(ORIGINAL_TEXT.to_string(), Value::String(text.to_string())),
            Err(_) => m.insert(
                ORIGINAL_HEX.to_string(),
                Value::String(hex::encode(&self.bytes)),
            ),
        };
        Value::Object(m)
    }

    /// The record read back from an entry's `prior`.
    pub fn from_prior(prior: Option<&Value>) -> Option<Original> {
        let o = prior?.get(PRIOR_ORIGINAL)?;
        let bytes = match o.get(ORIGINAL_TEXT).and_then(Value::as_str) {
            Some(text) => text.as_bytes().to_vec(),
            None => hex::decode(o.get(ORIGINAL_HEX)?.as_str()?).ok()?,
        };
        let sha256 = o
            .get(ORIGINAL_SHA256)
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| fsx::sha256_bytes(&bytes));
        let mode = o
            .get(ORIGINAL_MODE)
            .and_then(Value::as_str)
            .and_then(|m| u32::from_str_radix(m, 8).ok());
        Some(Original {
            sha256,
            mode,
            bytes,
        })
    }

    /// The recorded bytes parsed as JSON, when they are.
    pub fn document(&self) -> Option<Value> {
        serde_json::from_slice(&self.bytes).ok()
    }
}

/// The permission bits of a file's metadata (`0o644`), unix only.
#[cfg(unix)]
pub fn file_mode(meta: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(meta.permissions().mode() & 0o777)
}

/// See the unix definition.
#[cfg(not(unix))]
pub fn file_mode(_meta: &std::fs::Metadata) -> Option<u32> {
    None
}

/// Re-apply permission bits (a host rewrite lands its own).
#[cfg(unix)]
pub fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
        .map_err(|e| LedgerError::io("set mode", path, e))
}

/// See the unix definition.
#[cfg(not(unix))]
pub fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}

/// Insert every `(path, value)` that is absent from `doc`, creating object
/// parents as needed and leaving every present member alone. Returns the
/// dotted keys that were inserted, in order.
pub fn merge_members(doc: &mut Value, members: &[(Vec<String>, Value)]) -> Vec<String> {
    let mut inserted = Vec::new();
    for (path, value) in members {
        let Some((leaf, parents)) = path.split_last() else {
            continue;
        };
        let Some(obj) = descend(doc, parents) else {
            continue;
        };
        if !obj.contains_key(leaf) {
            obj.insert(leaf.clone(), value.clone());
            inserted.push(path.join("."));
        }
    }
    inserted
}

/// The object at `parents` under `doc`, creating empty objects on the way;
/// `None` when a non-object is in the way (never overwritten).
fn descend<'a>(doc: &'a mut Value, parents: &[String]) -> Option<&'a mut Map<String, Value>> {
    let mut cur = doc;
    for p in parents {
        cur = cur
            .as_object_mut()?
            .entry(p.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    cur.as_object_mut()
}

/// 2-space pretty JSON with a trailing newline — how the host and omm both
/// serialise a shared file.
pub fn pretty(doc: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(doc).map_err(|e| LedgerError::Schema {
        path: std::path::PathBuf::from("shared document"),
        detail: e.to_string(),
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// `prior` with the `original` record set (other keys kept).
pub fn with_original(prior: Option<Value>, original: &Original) -> Value {
    let mut m = match prior {
        Some(Value::Object(m)) => m,
        _ => Map::new(),
    };
    m.insert(PRIOR_ORIGINAL.to_string(), original.to_value());
    Value::Object(m)
}

/// Attach an [`Original`] read earlier to the entry `(base, rel)` unless it
/// already carries one (the FIRST record wins). For a writer that read the
/// file before its own commit (`omm theme`, a profile switch) and ledgers
/// after it. Returns true when the record was added.
pub fn attach_original(
    ledger: &mut Ledger,
    base: Base,
    rel: &RelPath,
    original: &Original,
) -> bool {
    let Some(e) = ledger.find_mut(base, rel) else {
        return false;
    };
    if Original::from_prior(e.prior.as_ref()).is_some() {
        return false;
    }
    e.prior = Some(with_original(e.prior.take(), original));
    true
}

/// Record the pre-omm bytes and mode of the shared file at `path` in the
/// ledger entry `(base, rel)` — once: an entry that already carries an
/// `original` keeps it (the FIRST record is the pre-omm one), and a file
/// that does not exist records nothing (a file omm creates is `seeded`, and
/// unlinked at uninstall). Returns true when a record was added.
#[allow(clippy::too_many_arguments)]
pub fn record_original(
    ledger: &mut Ledger,
    base: Base,
    rel: &RelPath,
    kind: Kind,
    mechanism: Mechanism,
    path: &Path,
    writer: &str,
    source_version: &str,
) -> Result<bool> {
    if let Some(e) = ledger.find(base, rel) {
        // Recorded already, or a file omm itself created (`seeded`): what is
        // on disk now is omm's, never a pre-omm original.
        if e.class == Class::Seeded || Original::from_prior(e.prior.as_ref()).is_some() {
            return Ok(false);
        }
    }
    let Some(original) = Original::read(path)? else {
        return Ok(false);
    };
    let entry = match ledger.find(base, rel).cloned() {
        Some(mut e) => {
            e.prior = Some(with_original(e.prior.take(), &original));
            e
        }
        None => Entry {
            base,
            path: rel.clone(),
            kind,
            sha256: original.sha256.clone(),
            source_version: source_version.to_string(),
            writer: writer.to_string(),
            mechanism,
            class: Class::SharedKey,
            prior: Some(original.to_prior()),
        },
    };
    ledger.upsert(entry);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn record_round_trips_bytes_and_mode() {
        let o = Original {
            sha256: fsx::sha256_bytes(b"{\"a\":1}"),
            mode: Some(0o600),
            bytes: b"{\"a\":1}".to_vec(),
        };
        let prior = o.to_prior();
        assert_eq!(prior["original"]["mode"], "0600");
        assert_eq!(prior["original"]["text"], "{\"a\":1}");
        assert_eq!(Original::from_prior(Some(&prior)), Some(o.clone()));
        assert_eq!(o.document(), Some(json!({"a": 1})));
        let raw = Original {
            sha256: "s".into(),
            mode: None,
            bytes: b"\xff\xfe".to_vec(),
        };
        let prior = raw.to_prior();
        assert!(prior["original"].get("hex").is_some());
        assert_eq!(prior["original"]["mode"], Value::Null);
        assert_eq!(Original::from_prior(Some(&prior)), Some(raw));
        assert_eq!(Original::from_prior(None), None);
        assert_eq!(
            Original::from_prior(Some(&json!({"managed_sha256": "x"}))),
            None
        );
        // Other prior keys survive beside the record.
        let merged = with_original(Some(json!({"keep": true})), &o);
        assert_eq!(merged["keep"], true);
        assert_eq!(merged["original"]["mode"], "0600");
    }

    #[test]
    fn merge_inserts_only_absent_members_and_creates_parents() {
        let mut doc = json!({"schema_version": 1, "tui": {"theme": "x"}});
        let inserted = merge_members(
            &mut doc,
            &[
                (vec!["my_top".into()], json!({"keep": true})),
                (vec!["tui".into(), "extra".into()], json!(42)),
                (vec!["tui".into(), "theme".into()], json!("never")),
                (vec!["new".into(), "deep".into()], json!(1)),
            ],
        );
        assert_eq!(inserted, vec!["my_top", "tui.extra", "new.deep"]);
        assert_eq!(
            doc,
            json!({"schema_version": 1, "tui": {"theme": "x", "extra": 42}, "my_top": {"keep": true}, "new": {"deep": 1}})
        );
        // A scalar in the way is never overwritten.
        let mut scalar = json!({"tui": "flat"});
        assert!(
            merge_members(&mut scalar, &[(vec!["tui".into(), "x".into()], json!(1))]).is_empty()
        );
        assert_eq!(scalar, json!({"tui": "flat"}));
        assert!(pretty(&json!({"a": 1})).unwrap().ends_with(b"}\n"));
    }

    #[cfg(unix)]
    #[test]
    fn record_original_reads_once_and_keeps_the_first() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(&file, b"{\"schema_version\":1}").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        let mut ledger = Ledger::new(
            "0.1.0",
            crate::schema::HostInfo {
                version: "v".into(),
                sha256: "s".into(),
            },
            crate::schema::Scope::User,
        );
        let rel = RelPath::new("settings.json").unwrap();
        assert!(record_original(
            &mut ledger,
            Base::MuseConfig,
            &rel,
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
            &file,
            "omm install",
            "0.1.0"
        )
        .unwrap());
        let e = ledger.find(Base::MuseConfig, &rel).unwrap().clone();
        assert_eq!(e.class, Class::SharedKey);
        let o = Original::from_prior(e.prior.as_ref()).unwrap();
        assert_eq!(o.mode, Some(0o600));
        assert_eq!(o.bytes, b"{\"schema_version\":1}");
        // A second call (the file changed since) keeps the first record.
        std::fs::write(&file, b"{\"schema_version\":1,\"x\":1}").unwrap();
        assert!(!record_original(
            &mut ledger,
            Base::MuseConfig,
            &rel,
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
            &file,
            "omm theme",
            "0.1.0"
        )
        .unwrap());
        let again =
            Original::from_prior(ledger.find(Base::MuseConfig, &rel).unwrap().prior.as_ref())
                .unwrap();
        assert_eq!(again.bytes, b"{\"schema_version\":1}");
        // A file omm created (a `seeded` entry) records nothing either: what
        // is on disk is omm's, not a pre-omm original.
        let seeded_rel = RelPath::new("seeded.json").unwrap();
        let seeded = dir.path().join("seeded.json");
        std::fs::write(&seeded, b"{}").unwrap();
        ledger.upsert(Entry {
            base: Base::MuseConfig,
            path: seeded_rel.clone(),
            kind: Kind::SettingsKey,
            sha256: "x".into(),
            source_version: "0".into(),
            writer: "omm install".into(),
            mechanism: Mechanism::SettingsPatch,
            class: Class::Seeded,
            prior: None,
        });
        assert!(!record_original(
            &mut ledger,
            Base::MuseConfig,
            &seeded_rel,
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
            &seeded,
            "omm install",
            "0.1.0"
        )
        .unwrap());
        assert!(ledger
            .find(Base::MuseConfig, &seeded_rel)
            .unwrap()
            .prior
            .is_none());
        // A missing file records nothing.
        assert!(!record_original(
            &mut ledger,
            Base::MuseConfig,
            &RelPath::new("trust.json").unwrap(),
            Kind::Trust,
            Mechanism::TrustMerge,
            &dir.path().join("trust.json"),
            "omm trust",
            "0.1.0"
        )
        .unwrap());
        assert!(ledger
            .find(Base::MuseConfig, &RelPath::new("trust.json").unwrap())
            .is_none());
    }
}
