//! A read-only view of `$OMM/omm.lock.json` for D13 (ARCHITECTURE.md §4).
//!
//! Doctor never writes the ledger; it needs only to tell absent from corrupt
//! from loaded, to walk the file entries, and to read the `muse-plugin` /
//! `muse-marketplace` registrations. The schema is the one §4 fixes:
//! entries carry a `base` (`muse-config | muse-data | omm | workspace`) and a
//! base-relative `path`; registrations are `{kind: muse-plugin, id, approved}`
//! and `{kind: muse-marketplace, name, source}`. Every field beyond the
//! structural ones is optional here so a newer ledger still reads; the
//! spellings are `omm-ledger`'s (`schema::Registration` is a `kind`-tagged
//! kebab-case enum, `Mechanism::is_file` excludes only `settings-patch` and
//! `trust-merge`). Doctor deliberately does not go through
//! `omm_ledger::store::load`: that lane quarantines a corrupt file as
//! `.bad-<ts>` and appends an audit line — writes — and `omm doctor` writes
//! nothing (ARCHITECTURE.md §7); a corrupt ledger is reported here and left
//! for `omm reconcile`. The containment rule (R4) is applied before any path
//! is touched: relative, no `..`, and — when the file exists —
//! `canonicalize()` + `strip_prefix()` against the canonical base.

use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use omm_host::{fsx, Roots};

/// The ledger file name under `$OMM/` (ARCHITECTURE.md §2).
pub const LEDGER_FILE: &str = "omm.lock.json";
/// `base` values (§4).
pub const BASE_MUSE_CONFIG: &str = "muse-config";
/// See [`BASE_MUSE_CONFIG`].
pub const BASE_MUSE_DATA: &str = "muse-data";
/// See [`BASE_MUSE_CONFIG`].
pub const BASE_OMM: &str = "omm";
/// See [`BASE_MUSE_CONFIG`].
pub const BASE_WORKSPACE: &str = "workspace";
/// `registrations[].kind` values (§4).
pub const REGISTRATION_PLUGIN: &str = "muse-plugin";
/// See [`REGISTRATION_PLUGIN`].
pub const REGISTRATION_MARKETPLACE: &str = "muse-marketplace";
/// `registrations[].kind` of a typed `settings.json` key omm wrote.
pub const REGISTRATION_SETTINGS_KEY: &str = "settings-key";
/// `registrations[].kind` of a `trust.json` project entry omm wrote.
pub const REGISTRATION_TRUST: &str = "trust";
/// `mechanism` values that name a file on disk (§4; `omm_ledger::Mechanism::is_file`):
/// `settings-patch` and `trust-merge` name a shared file omm does not own and
/// are undone by value, so doctor never stats them.
pub const FILE_MECHANISMS: [&str; 3] = ["copy", "muse-skills-install", "muse-plugins-install"];

/// `host` of the ledger.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LedgerHost {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub sha256: String,
}

/// One ledgered file or key (§4).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Entry {
    pub base: String,
    pub path: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub source_version: String,
    #[serde(default)]
    pub writer: String,
    #[serde(default)]
    pub mechanism: String,
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub prior: Option<Value>,
}

impl Entry {
    /// True when the entry names a file the installer put on disk.
    pub fn is_file(&self) -> bool {
        FILE_MECHANISMS.contains(&self.mechanism.as_str())
    }
}

/// One thing that is not a file but must be undone (§4).
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Registration {
    pub kind: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub approved: Option<Vec<String>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub package_sha256: Option<String>,
    /// `installed.source.path`, the pinned marketplace generation.
    #[serde(default)]
    pub generation_path: Option<String>,
    /// `settings-key`: the dotted key omm wrote (`trust`: unused).
    #[serde(default)]
    pub path: Option<String>,
    /// `trust`: the canonical workspace key omm wrote.
    #[serde(default)]
    pub project: Option<String>,
    /// `settings-key` / `trust`: the value before omm touched it.
    #[serde(default)]
    pub prior: Option<Value>,
    /// `settings-key` / `trust`: what omm wrote last (absent in older ledgers).
    #[serde(default)]
    pub value: Option<Value>,
}

/// The ledger document.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Ledger {
    pub schema_version: u64,
    #[serde(default)]
    pub omm_version: String,
    #[serde(default)]
    pub host: Option<LedgerHost>,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub registrations: Vec<Registration>,
}

impl Ledger {
    /// The `muse-plugin` registration for `id`.
    pub fn plugin_registration(&self, id: &str) -> Option<&Registration> {
        self.registrations
            .iter()
            .find(|r| r.kind == REGISTRATION_PLUGIN && r.id.as_deref() == Some(id))
    }
    /// The `muse-marketplace` registration, if any.
    pub fn marketplace_registration(&self) -> Option<&Registration> {
        self.registrations
            .iter()
            .find(|r| r.kind == REGISTRATION_MARKETPLACE)
    }
    /// The `settings-key` registration of a dotted key — proof that omm
    /// wrote that key (D2 warns about an unset `provider` only then).
    pub fn settings_key(&self, key: &str) -> Option<&Registration> {
        self.registrations
            .iter()
            .find(|r| r.kind == REGISTRATION_SETTINGS_KEY && r.path.as_deref() == Some(key))
    }
    /// Entries that name files on disk.
    pub fn file_entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.is_file())
    }
    /// The skill ids a managed-store install (`omm install --no-plugin`)
    /// ledgered: `muse-skills-install` entries under `<store_prefix>/<id>/…`
    /// of the config base, deduplicated and sorted. `store_prefix` is the
    /// personal skills dir relative to the config root (`Roots`), never
    /// spelled here.
    pub fn managed_skill_ids(&self, store_prefix: &str) -> Vec<String> {
        let prefix = format!("{}/", store_prefix.trim_end_matches('/'));
        let mut ids: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.base == BASE_MUSE_CONFIG && e.mechanism == "muse-skills-install")
            .filter_map(|e| e.path.strip_prefix(&prefix))
            .filter_map(|rest| rest.split_once('/').map(|(id, _)| id.to_string()))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
    /// True when an entry records `(base, path)`.
    pub fn has(&self, base: &str, path: &str) -> bool {
        self.entries
            .iter()
            .any(|e| e.base == base && e.path == path)
    }
}

/// What the file on disk amounts to.
#[derive(Clone, Debug)]
pub enum LedgerState {
    /// No file at the path.
    Absent(PathBuf),
    /// A file that is not a ledger (unparseable, wrong schema, not an object).
    Corrupt {
        path: PathBuf,
        detail: String,
    },
    Loaded(Ledger),
}

/// Read the ledger at `path`.
pub fn load(path: &Path) -> LedgerState {
    if fsx::is_dangling_symlink(path) {
        return LedgerState::Corrupt {
            path: path.to_path_buf(),
            detail: "dangling symlink".to_string(),
        };
    }
    if !path.exists() {
        return LedgerState::Absent(path.to_path_buf());
    }
    let bytes = match fsx::read_bytes(path) {
        Ok(b) => b,
        Err(e) => {
            return LedgerState::Corrupt {
                path: path.to_path_buf(),
                detail: e.to_string(),
            }
        }
    };
    match serde_json::from_slice::<Ledger>(&bytes) {
        Ok(l) if l.schema_version == 1 => LedgerState::Loaded(l),
        Ok(l) => LedgerState::Corrupt {
            path: path.to_path_buf(),
            detail: format!("unsupported schema_version {}", l.schema_version),
        },
        Err(e) => LedgerState::Corrupt {
            path: path.to_path_buf(),
            detail: e.to_string(),
        },
    }
}

/// The root a `base` names (R4: exactly the four allowlisted bases).
pub fn base_root(roots: &Roots, workspace: Option<&Path>, base: &str) -> Option<PathBuf> {
    match base {
        BASE_MUSE_CONFIG => Some(roots.muse_config()),
        BASE_MUSE_DATA => Some(roots.muse_data()),
        BASE_OMM => Some(roots.omm_root()),
        BASE_WORKSPACE => workspace.map(Path::to_path_buf),
        _ => None,
    }
}

/// Where a ledgered path resolves, or `None` when it is not contained:
/// absolute, empty, carrying `..`, or — when it exists — canonicalizing
/// outside the canonical base (a symlink pointing out).
///
/// Read-only D13 lane: a lexically-inside but missing path is `Some` so the
/// caller reports it as `vanished`, never `escaped`. For writes use
/// `omm_ledger::Bases::resolve`; the parity test
/// `crates/omm/tests/containment_parity.rs` locks the agreement.
pub fn contained_path(base_root: &Path, rel: &str) -> Option<PathBuf> {
    let rel_path = Path::new(rel);
    if rel.is_empty()
        || rel_path.is_absolute()
        || rel_path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return None;
    }
    let joined = base_root.join(rel_path);
    if joined.exists() {
        let base = fsx::canonicalize(base_root).ok()?;
        let real = fsx::canonicalize(&joined).ok()?;
        real.strip_prefix(&base).ok()?;
    }
    Some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn loads_absent_corrupt_and_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(LEDGER_FILE);
        assert!(matches!(load(&path), LedgerState::Absent(_)));
        std::fs::write(&path, b"{not json").unwrap();
        assert!(matches!(load(&path), LedgerState::Corrupt { .. }));
        std::fs::write(&path, json!({"schema_version": 2}).to_string()).unwrap();
        assert!(matches!(load(&path), LedgerState::Corrupt { .. }));
        std::fs::write(
            &path,
            json!({
                "schema_version": 1, "omm_version": "0.1.0", "scope": "user",
                "entries": [
                    {"base": "muse-config", "path": "skills/omm-plan/SKILL.md", "kind": "skill", "sha256": "x", "mechanism": "copy", "class": "exclusive", "prior": null},
                    {"base": "muse-config", "path": "settings.json#tui.theme", "kind": "settings-key", "mechanism": "settings-patch", "prior": "old"}
                ],
                "registrations": [
                    {"kind": "muse-plugin", "id": "omm", "approved": ["plugin:omm:hook:omm-guard"]},
                    {"kind": "muse-marketplace", "name": "ohmy", "source": "/repo"}
                ]
            })
            .to_string(),
        )
        .unwrap();
        let LedgerState::Loaded(l) = load(&path) else {
            panic!("expected loaded");
        };
        assert_eq!(l.file_entries().count(), 1);
        assert!(l.has("muse-config", "skills/omm-plan/SKILL.md"));
        assert!(
            l.managed_skill_ids("skills").is_empty(),
            "a copy is not a managed install"
        );
        let mut managed = l.clone();
        managed.entries[0].mechanism = "muse-skills-install".into();
        managed.entries.push(Entry {
            base: "muse-config".into(),
            path: "skills/omm-plan/references/r.md".into(),
            mechanism: "muse-skills-install".into(),
            ..Entry::default()
        });
        managed.entries.push(Entry {
            base: "muse-config".into(),
            path: "skills/omm-zed/SKILL.md".into(),
            mechanism: "muse-skills-install".into(),
            ..Entry::default()
        });
        assert_eq!(
            managed.managed_skill_ids("skills"),
            vec!["omm-plan", "omm-zed"]
        );
        assert_eq!(
            managed.managed_skill_ids("skills/"),
            vec!["omm-plan", "omm-zed"]
        );
        assert_eq!(
            l.plugin_registration("omm")
                .and_then(|r| r.approved.clone()),
            Some(vec!["plugin:omm:hook:omm-guard".to_string()])
        );
        assert_eq!(
            l.marketplace_registration().and_then(|r| r.name.clone()),
            Some("ohmy".to_string())
        );
        assert!(l.plugin_registration("other").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn containment_refuses_escapes_and_symlinks_out() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("base");
        std::fs::create_dir_all(base.join("skills")).unwrap();
        std::fs::write(base.join("skills/f"), b"x").unwrap();
        let outside = dir.path().join("outside");
        std::fs::write(&outside, b"y").unwrap();
        std::os::unix::fs::symlink(&outside, base.join("skills/link")).unwrap();
        assert!(contained_path(&base, "skills/f").is_some());
        assert!(
            contained_path(&base, "skills/missing").is_some(),
            "a missing file is still contained"
        );
        assert!(contained_path(&base, "../outside").is_none());
        assert!(contained_path(&base, "/etc/passwd").is_none());
        assert!(contained_path(&base, "").is_none());
        assert!(
            contained_path(&base, "skills/link").is_none(),
            "symlink out of the base"
        );
        let roots = Roots::resolve(&omm_host::paths::EnvView {
            home: Some("/h".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            base_root(&roots, None, BASE_MUSE_CONFIG),
            Some(PathBuf::from("/h/.config/muse"))
        );
        assert_eq!(base_root(&roots, None, BASE_WORKSPACE), None);
        assert_eq!(base_root(&roots, None, "zzz"), None);
    }
}
