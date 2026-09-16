//! `trust.json` — merge one project key, never rewrite the map.
//!
//! Shape (`research/musecode/security-permissions.md` §5.2, recovered by
//! construction): `{"schema_version":1,"projects":{"<abs>":{"decision":"trusted"}}}`.
//! `decision` is `trusted | untrusted`; the key is the canonical workspace
//! root (`workspace root does not match its trusted binding`); unknown
//! per-entry fields are tolerated; a malformed store aborts the session
//! (exit 1) and `schema_version` 2 is `unsupported trust store schema version 2`
//! (verification 11). Nothing but the TUI's first-run prompt writes this file;
//! hand-writing works (`docs/host-reality.md` "Paths": trust). Trust gates
//! project skills, hooks, rules, workflows, agents, plugin installs and the
//! `personal_project` memory scope — all silently inert without it (D12).

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{HostError, Result};
use crate::fsx;
use crate::host_reality as hr;
use crate::paths::Roots;
use crate::settings::{replace_locked, CommitOptions, MUSE_CONFIG_BASE};

/// `projects.<abs>.decision`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrustDecision {
    Trusted,
    Untrusted,
}

impl TrustDecision {
    /// The literal the host expects.
    pub fn as_str(self) -> &'static str {
        match self {
            TrustDecision::Trusted => "trusted",
            TrustDecision::Untrusted => "untrusted",
        }
    }
}

/// What one merge changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustMerge {
    /// The canonical workspace key.
    pub key: String,
    /// The whole prior entry (for exact undo), `None` if absent.
    pub prior: Option<Value>,
    /// False when the entry already said the same thing.
    pub changed: bool,
}

/// What [`TrustStore::commit`] did.
#[derive(Clone, Debug)]
pub struct TrustCommit {
    pub path: PathBuf,
    /// The verified backup, `$OMM/snapshots/<ts>/muse-config/trust.json`.
    pub backup: Option<PathBuf>,
    pub written: bool,
    pub created_dir: bool,
    pub sha256: String,
}

/// A loaded `trust.json`.
#[derive(Clone, Debug)]
pub struct TrustStore {
    path: PathBuf,
    value: Value,
    existed: bool,
    loaded_sha256: Option<String>,
    /// `$OMM/`, known when loaded via [`TrustStore::for_roots`].
    omm_root: Option<PathBuf>,
}

impl TrustStore {
    /// Load `path`; a missing file becomes an empty store. A dangling symlink
    /// is refused (see `SettingsDoc::load`), and so is a document that repeats
    /// a key at any depth — the host calls that a malformed trust store.
    pub fn load(path: &Path) -> Result<TrustStore> {
        if fsx::is_dangling_symlink(path) {
            return Err(fsx::realpath_for_write(path).err().unwrap_or(
                HostError::DanglingSymlink {
                    path: path.to_path_buf(),
                    target: path.to_path_buf(),
                },
            ));
        }
        if !path.exists() {
            return Ok(TrustStore {
                path: path.to_path_buf(),
                value: empty_store(),
                existed: false,
                loaded_sha256: None,
                omm_root: None,
            });
        }
        let bytes = fsx::read_bytes(path)?;
        let value: Value = serde_json::from_slice(&bytes).map_err(|e| HostError::Trust {
            path: path.to_path_buf(),
            detail: format!("malformed JSON: {e}"),
        })?;
        // The host: `malformed trust store at …: duplicate field `projects``,
        // exit 1; a Value parse would keep the last occurrence and a commit
        // would rewrite the store without the first (measured 2026-09-02).
        crate::settings::reject_duplicate_keys(&bytes).map_err(|detail| HostError::Trust {
            path: path.to_path_buf(),
            detail: format!("malformed JSON: {detail}"),
        })?;
        check_shape(path, &value)?;
        Ok(TrustStore {
            path: path.to_path_buf(),
            value,
            existed: true,
            loaded_sha256: Some(fsx::sha256_bytes(&bytes)),
            omm_root: None,
        })
    }

    /// Load the trust store of these roots; the lock and the backups go under
    /// `roots.omm_root()`.
    pub fn for_roots(roots: &Roots) -> Result<TrustStore> {
        let mut store = TrustStore::load(&roots.trust_file())?;
        store.omm_root = Some(roots.omm_root());
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn value(&self) -> &Value {
        &self.value
    }
    pub fn existed(&self) -> bool {
        self.existed
    }

    /// The canonical key the host binds trust to.
    pub fn key_for(workspace: &Path) -> Result<String> {
        Ok(fsx::canonicalize(workspace)?.to_string_lossy().into_owned())
    }

    /// The recorded decision for a workspace, if any.
    pub fn decision_for(&self, workspace: &Path) -> Result<Option<String>> {
        let key = TrustStore::key_for(workspace)?;
        Ok(self
            .value
            .get("projects")
            .and_then(|p| p.get(&key))
            .and_then(|e| e.get("decision"))
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    /// Set `projects.<canonical>.decision`, keeping every other key of the
    /// store and every other field of the entry.
    pub fn merge_project(
        &mut self,
        workspace: &Path,
        decision: TrustDecision,
    ) -> Result<TrustMerge> {
        let key = TrustStore::key_for(workspace)?;
        let projects = self.projects_mut();
        let prior = projects.get(&key).cloned();
        let changed = prior
            .as_ref()
            .and_then(|e| e.get("decision"))
            .and_then(Value::as_str)
            != Some(decision.as_str());
        let entry = projects
            .entry(key.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        if let Some(obj) = entry.as_object_mut() {
            obj.insert(
                "decision".to_string(),
                Value::String(decision.as_str().to_string()),
            );
        }
        Ok(TrustMerge {
            key,
            prior,
            changed,
        })
    }

    /// Remove `projects.<canonical>` entirely.
    pub fn remove_project(&mut self, workspace: &Path) -> Result<TrustMerge> {
        let key = TrustStore::key_for(workspace)?;
        let prior = self.projects_mut().remove(&key);
        Ok(TrustMerge {
            changed: prior.is_some(),
            key,
            prior,
        })
    }

    /// Put an entry back exactly as it was (uninstall's `prior`), by key.
    pub fn restore_project(&mut self, key: &str, prior: Option<Value>) {
        let projects = self.projects_mut();
        match prior {
            Some(v) => {
                projects.insert(key.to_string(), v);
            }
            None => {
                projects.remove(key);
            }
        }
    }

    fn projects_mut(&mut self) -> &mut Map<String, Value> {
        if !self.value.is_object() {
            self.value = empty_store();
        }
        let root = self
            .value
            .as_object_mut()
            .unwrap_or_else(|| unreachable!("value was just made an object"));
        root.entry("schema_version".to_string())
            .or_insert_with(|| Value::from(hr::TRUST_SCHEMA_VERSION));
        let projects = root
            .entry("projects".to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !projects.is_object() {
            *projects = Value::Object(Map::new());
        }
        projects
            .as_object_mut()
            .unwrap_or_else(|| unreachable!("projects was just made an object"))
    }

    /// 2-space pretty JSON, trailing newline.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(&self.value).map_err(|e| HostError::Parse {
            what: "trust store",
            detail: e.to_string(),
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Shape-check, then — under the muse-config lock — re-check staleness,
    /// back up, and atomically replace the file on its realpath
    /// (`settings::replace_locked`).
    pub fn commit(&self, opts: &CommitOptions) -> Result<TrustCommit> {
        check_shape(&self.path, &self.value)?;
        let bytes = self.to_bytes()?;
        let sha256 = fsx::sha256_bytes(&bytes);
        let parent = self.path.parent().ok_or_else(|| HostError::Io {
            context: "resolve parent of",
            path: self.path.clone(),
            source: std::io::Error::other("path has no parent"),
        })?;
        let created_dir = !parent.exists();
        if opts.dry_run {
            return Ok(TrustCommit {
                path: self.path.clone(),
                backup: None,
                written: false,
                created_dir,
                sha256,
            });
        }
        let replaced = replace_locked(
            &self.path,
            created_dir,
            self.loaded_sha256.as_deref(),
            &bytes,
            opts,
            self.omm_root.as_deref(),
            MUSE_CONFIG_BASE,
        )?;
        Ok(TrustCommit {
            path: replaced.realpath,
            backup: replaced.backup,
            written: true,
            created_dir,
            sha256,
        })
    }
}

fn empty_store() -> Value {
    let mut m = Map::new();
    m.insert(
        "schema_version".to_string(),
        Value::from(hr::TRUST_SCHEMA_VERSION),
    );
    m.insert("projects".to_string(), Value::Object(Map::new()));
    Value::Object(m)
}

fn check_shape(path: &Path, value: &Value) -> Result<()> {
    let bad = |detail: String| HostError::Trust {
        path: path.to_path_buf(),
        detail,
    };
    let obj = value
        .as_object()
        .ok_or_else(|| bad("not a JSON object".to_string()))?;
    match obj.get("schema_version").and_then(Value::as_u64) {
        Some(v) if v == hr::TRUST_SCHEMA_VERSION => {}
        Some(v) => return Err(bad(format!("unsupported trust store schema version {v}"))),
        None => return Err(bad("missing field `schema_version`".to_string())),
    }
    match obj.get("projects") {
        Some(Value::Object(projects)) => {
            for (k, entry) in projects {
                let decision = entry
                    .get("decision")
                    .and_then(Value::as_str)
                    .ok_or_else(|| bad(format!("projects[{k:?}] has no `decision`")))?;
                if decision != "trusted" && decision != "untrusted" {
                    return Err(bad(format!(
                        "projects[{k:?}].decision is {decision:?}, expected `trusted` or `untrusted`"
                    )));
                }
            }
        }
        Some(_) => return Err(bad("`projects` is not an object".to_string())),
        // The host accepts a store without `projects` (every workspace
        // untrusted; measured 2026-09-02: `{"schema_version":1}` → `muse exec
        // --provider echo hi` exit 0, no trust line); `projects_mut` inserts it.
        None => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn merge_keeps_other_keys_and_reports_prior() {
        let dir = tempfile::tempdir().unwrap();
        let ws = dir.path().join("ws");
        std::fs::create_dir(&ws).unwrap();
        let file = dir.path().join("trust.json");
        std::fs::write(
            &file,
            json!({"schema_version": 1, "projects": {"/other": {"decision": "untrusted", "extra": true}}, "zzz": 1}).to_string(),
        )
        .unwrap();
        let mut store = TrustStore::load(&file).unwrap();
        let m = store.merge_project(&ws, TrustDecision::Trusted).unwrap();
        assert!(m.changed);
        assert_eq!(m.prior, None);
        assert_eq!(m.key, fsx::canonicalize(&ws).unwrap().to_string_lossy());
        assert_eq!(store.decision_for(&ws).unwrap().as_deref(), Some("trusted"));
        assert_eq!(store.value()["zzz"], 1);
        assert_eq!(store.value()["projects"]["/other"]["extra"], true);

        let again = store.merge_project(&ws, TrustDecision::Trusted).unwrap();
        assert!(!again.changed);
        assert_eq!(again.prior, Some(json!({"decision": "trusted"})));

        let opts = CommitOptions {
            omm_root: Some(dir.path().join("omm")),
            ..CommitOptions::default()
        };
        let report = store.commit(&opts).unwrap();
        assert!(report.written);
        let backup = report.backup.clone().unwrap();
        assert!(backup.starts_with(dir.path().join("omm").join("snapshots")));
        assert!(backup.ends_with("muse-config/trust.json"));
        assert!(dir.path().join("omm/locks/muse-config.lock").exists());
        assert_eq!(
            dir.path().read_dir().unwrap().count(),
            3,
            "ws, trust.json and omm/ — nothing beside the store"
        );
        // Without a known omm root the lock and the backup have nowhere legitimate to go.
        assert!(matches!(
            TrustStore::load(&file)
                .unwrap()
                .commit(&CommitOptions::default()),
            Err(HostError::NoOmmRoot { .. })
        ));
        let reloaded = TrustStore::load(&file).unwrap();
        assert_eq!(
            reloaded.value()["projects"]["/other"]["decision"],
            "untrusted"
        );

        let mut reloaded = reloaded;
        let removed = reloaded.remove_project(&ws).unwrap();
        assert!(removed.changed);
        reloaded.restore_project(&removed.key, removed.prior.clone());
        assert_eq!(
            reloaded.decision_for(&ws).unwrap().as_deref(),
            Some("trusted")
        );
    }

    #[test]
    fn missing_store_is_empty_and_commit_creates_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("muse").join("trust.json");
        let ws = dir.path();
        let mut store = TrustStore::load(&file).unwrap();
        assert!(!store.existed());
        store.merge_project(ws, TrustDecision::Trusted).unwrap();
        // Nothing to back up, but the write is still locked: an omm root is needed.
        assert!(matches!(
            store.commit(&CommitOptions::default()),
            Err(HostError::NoOmmRoot { .. })
        ));
        assert!(!file.exists());
        let opts = CommitOptions {
            omm_root: Some(dir.path().join("omm")),
            ..CommitOptions::default()
        };
        let report = store.commit(&opts).unwrap();
        assert!(report.created_dir);
        assert!(report.backup.is_none());
        assert!(file.exists());
    }

    #[test]
    fn concurrent_trust_commits_land_exactly_one() {
        // Two stores loaded from the same file race on commit: the lock
        // serializes them and the second sees the first's bytes as Stale.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.json");
        std::fs::write(
            &file,
            json!({"schema_version": 1, "projects": {}}).to_string(),
        )
        .unwrap();
        let opts = CommitOptions {
            omm_root: Some(dir.path().join("omm")),
            ..CommitOptions::default()
        };
        let barrier = std::sync::Barrier::new(2);
        let results: Vec<Result<TrustCommit>> = std::thread::scope(|scope| {
            let handles: Vec<_> = ["a", "b"]
                .into_iter()
                .map(|ws| {
                    let (file, opts, barrier, dir) = (&file, &opts, &barrier, dir.path());
                    scope.spawn(move || {
                        let ws = dir.join(ws);
                        std::fs::create_dir_all(&ws).unwrap();
                        let mut store = TrustStore::load(file).unwrap();
                        store.merge_project(&ws, TrustDecision::Trusted).unwrap();
                        barrier.wait();
                        store.commit(opts)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let landed = results.iter().filter(|r| r.is_ok()).count();
        let stale = results
            .iter()
            .filter(|r| matches!(r, Err(HostError::Stale { .. })))
            .count();
        assert_eq!((landed, stale), (1, 1), "{results:?}");
        let projects = TrustStore::load(&file).unwrap().value()["projects"]
            .as_object()
            .unwrap()
            .len();
        assert_eq!(projects, 1, "only the landed merge is on disk");
    }

    #[test]
    fn malformed_and_unsupported_stores_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.json");
        std::fs::write(&file, "{").unwrap();
        assert!(matches!(
            TrustStore::load(&file),
            Err(HostError::Trust { .. })
        ));
        std::fs::write(
            &file,
            json!({"schema_version": 2, "projects": {}}).to_string(),
        )
        .unwrap();
        assert!(matches!(
            TrustStore::load(&file),
            Err(HostError::Trust { .. })
        ));
        std::fs::write(
            &file,
            json!({"schema_version": 1, "projects": {"/x": {"decision": "trust"}}}).to_string(),
        )
        .unwrap();
        assert!(matches!(
            TrustStore::load(&file),
            Err(HostError::Trust { .. })
        ));
    }

    #[test]
    fn store_without_projects_loads_like_the_host() {
        // `{"schema_version":1}`: the host runs (`workspace root: … (cwd default)`,
        // every workspace untrusted) — measured 2026-09-02 on 1.0.1-R2006.1.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.json");
        std::fs::write(&file, json!({"schema_version": 1}).to_string()).unwrap();
        let mut store = TrustStore::load(&file).expect("a projects-less store is valid");
        assert_eq!(store.decision_for(dir.path()).unwrap(), None);
        let m = store
            .merge_project(dir.path(), TrustDecision::Trusted)
            .unwrap();
        assert!(m.changed);
        assert!(store.value()["projects"].is_object());
        // The other refusals stay: non-object projects, missing/typo decision.
        for bad in [
            json!({"schema_version": 1, "projects": []}),
            json!({"schema_version": 1, "projects": {"/x": {}}}),
            json!({"schema_version": 1, "projects": {"/x": {"decision": "trust"}}}),
        ] {
            std::fs::write(&file, bad.to_string()).unwrap();
            assert!(
                matches!(TrustStore::load(&file), Err(HostError::Trust { .. })),
                "{bad}"
            );
        }
    }

    #[test]
    fn stale_store_is_not_clobbered() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.json");
        std::fs::write(
            &file,
            json!({"schema_version": 1, "projects": {}}).to_string(),
        )
        .unwrap();
        let mut store = TrustStore::load(&file).unwrap();
        store
            .merge_project(dir.path(), TrustDecision::Trusted)
            .unwrap();
        std::fs::write(
            &file,
            json!({"schema_version": 1, "projects": {"/x": {"decision": "trusted"}}}).to_string(),
        )
        .unwrap();
        let opts = CommitOptions {
            omm_root: Some(dir.path().join("omm")),
            ..CommitOptions::default()
        };
        assert!(matches!(store.commit(&opts), Err(HostError::Stale { .. })));
        assert!(
            !dir.path().join("omm").join("snapshots").exists(),
            "a refused commit takes no backup"
        );
    }

    #[test]
    fn duplicate_keys_are_refused_at_load_like_the_host() {
        // `muse exec --provider echo hi` with a repeated `projects` key:
        // `malformed trust store at …: duplicate field `projects` at line 1
        // column 181`, exit 1 (2026-09-02). A Value parse keeps the last one.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("trust.json");
        std::fs::write(
            &file,
            b"{\"schema_version\":1,\"projects\":{\"/x\":{\"decision\":\"trusted\"}},\"projects\":{}}",
        )
        .unwrap();
        match TrustStore::load(&file) {
            Err(HostError::Trust { path, detail }) => {
                assert_eq!(path, file);
                assert!(detail.contains("duplicate field `projects`"), "{detail}");
            }
            other => panic!("expected HostError::Trust, got {other:?}"),
        }
        std::fs::write(
            &file,
            b"{\"schema_version\":1,\"projects\":{\"/x\":{\"decision\":\"trusted\",\"decision\":\"untrusted\"}}}",
        )
        .unwrap();
        assert!(matches!(
            TrustStore::load(&file),
            Err(HostError::Trust { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_store_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("trust.json");
        std::os::unix::fs::symlink(dir.path().join("gone.json"), &link).unwrap();
        assert!(matches!(
            TrustStore::load(&link),
            Err(HostError::DanglingSymlink { .. })
        ));
    }
}
