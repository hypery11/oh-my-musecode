//! Targeted, validated, atomic edits of the host's `settings.json` (R9, R10).
//!
//! The host rewrites `settings.json` from a typed struct and **destroys unknown
//! keys** (`research/musecode/config-paths.md` verification R6), so only the 29
//! typed keys of `docs/host-data/settings-keys.json` may ever be written, each
//! recorded with its prior value for the ledger. Before landing a candidate:
//!
//! 1. local shape checks and the `mcpServers`/`mcp_servers` collision refusal
//!    (every settings-mutating host command exits 1 in that state —
//!    `research/experiments/settings-plugins.md` verification C5);
//! 2. `muse config validate --plane defaults --file <tmp>` on a projection of
//!    each touched key (the enterprise defaults plane sees 20 of the 29 keys
//!    and reports `field_not_activated` for gate-off fields that already passed
//!    type checks — `crates/omm-host/data/enterprise-defaults-plane.json`);
//! 3. the host's own settings loader, via `skills list --source user --json`
//!    in a throwaway config root (`research/experiments/loose-ends.md` §1.4);
//! 4. under the exclusive advisory lock `$OMM/locks/muse-config.lock`
//!    ([`MUSE_CONFIG_LOCK`], shared with `trust.json`): a re-hash of the
//!    realpath — refusing if the file changed since load — a verified backup
//!    under `$OMM/snapshots/<ts>/muse-config/settings.json` (sha-checked,
//!    rolling keep-5 — ARCHITECTURE.md §2: nothing omm writes into Muse's
//!    config root is unledgered footprint, so no `.pre-omm.*` beside the
//!    host's file) and an atomic rename on the canonicalized realpath. The
//!    lock is what makes the stale check honest: without it two writers that
//!    loaded the same document both passed the check (measured 4 of 4 rounds)
//!    and one update vanished with no backup of it anywhere. A dangling
//!    symlink at the path is refused rather than silently replaced by a
//!    regular file.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::error::{HostError, Result};
use crate::fsx;
use crate::host_reality as hr;
use crate::invoke::Invoker;
use crate::paths::{Roots, OMM_LOCKS_DIR, OMM_SNAPSHOTS_DIR};
use crate::probe::{self, ConfigValidation, LoadProbe};

/// One targeted change: set or remove the value at `path`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatchOp {
    /// Key path from the top level, e.g. `["tui", "theme"]`.
    pub path: Vec<String>,
    /// `Some(value)` sets, `None` removes.
    pub value: Option<Value>,
}

impl PatchOp {
    /// Set `a.b.c` (split on `.`) to `value`. Use [`PatchOp::set_path`] when a
    /// segment itself contains dots (skill activation locators do).
    pub fn set(dotted: &str, value: Value) -> Self {
        PatchOp {
            path: split_dotted(dotted),
            value: Some(value),
        }
    }
    /// Remove `a.b.c`.
    pub fn remove(dotted: &str) -> Self {
        PatchOp {
            path: split_dotted(dotted),
            value: None,
        }
    }
    /// Set an explicit path.
    pub fn set_path(path: Vec<String>, value: Value) -> Self {
        PatchOp {
            path,
            value: Some(value),
        }
    }
    /// Remove an explicit path.
    pub fn remove_path(path: Vec<String>) -> Self {
        PatchOp { path, value: None }
    }
}

fn split_dotted(dotted: &str) -> Vec<String> {
    dotted
        .split('.')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// The value at a path before it was touched — the ledger's `prior`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PriorValue {
    pub path: Vec<String>,
    pub prior: Option<Value>,
}

impl PriorValue {
    /// `a.b.c`.
    pub fn key(&self) -> String {
        self.path.join(".")
    }
}

/// True when both spellings are present (D3).
pub fn has_mcp_collision(value: &Value) -> bool {
    value
        .as_object()
        .map(|m| m.contains_key("mcpServers") && m.contains_key("mcp_servers"))
        .unwrap_or(false)
}

/// The members of a settings document the host does not type and destroys
/// on its next rewrite, each with its value: every unknown top-level key —
/// the legacy `mcp_servers` spelling excluded, that one is D3's finding and
/// a collision to refuse, never a key to carry — and every `tui.<key>`
/// outside the typed `tui` fields (`SettingsDoc::unknown_top_level_keys`,
/// `SettingsDoc::unknown_tui_keys`). What an uninstall merges back after the
/// host's rewrites (`omm_ledger::shared`).
pub fn untyped_members(value: &Value) -> Result<Vec<(Vec<String>, Value)>> {
    let keys = hr::settings_keys()?;
    let mut out = Vec::new();
    let Some(obj) = value.as_object() else {
        return Ok(out);
    };
    for (k, v) in obj {
        if !keys.is_known(k) && keys.canonical_spelling(k).is_none() {
            out.push((vec![k.clone()], v.clone()));
        }
    }
    if let Some(tui) = obj.get("tui").and_then(Value::as_object) {
        for (k, v) in tui {
            if !keys.tui_fields.items.iter().any(|f| &f.key == k) {
                out.push((vec!["tui".to_string(), k.clone()], v.clone()));
            }
        }
    }
    Ok(out)
}

/// Why removing the leaf at `path` must be skipped, or `None` when it may
/// go: the leaf is a required structural member of its top-level parent
/// (settings-keys.json `items[].structural.required` — `permissions.
/// schema_version`) and the parent object still holds members other than
/// its structural ones, which the host would refuse whole without it
/// (`Named permission profiles are unavailable: missing field
/// schema_version`). A parent left with nothing but structural members is
/// omm's to empty: the leaf goes, and the empty parent with it.
pub fn structural_keep_reason(doc: &Value, path: &[String]) -> Result<Option<String>> {
    let [top, leaf] = path else {
        return Ok(None);
    };
    let keys = hr::settings_keys()?;
    let required = keys.structural_required(top);
    if !required.iter().any(|r| r == leaf) {
        return Ok(None);
    }
    let Some(parent) = doc.get(top).and_then(Value::as_object) else {
        return Ok(None);
    };
    let others: Vec<&String> = parent
        .keys()
        .filter(|k| !required.iter().any(|r| r == *k))
        .collect();
    if others.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "`{top}.{leaf}` is a required member of the `{top}` object, which still holds {} (not omm's); removing it would make the host refuse the whole object",
        others
            .iter()
            .map(|k| format!("`{top}.{k}`"))
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

/// The members of the object under top-level `top` that are missing from
/// `doc` although the host requires them whenever the object is present
/// (see [`structural_keep_reason`]); empty when the object is absent, not
/// an object, or empty — the host's own default shape.
pub fn missing_structural_members(doc: &Value, top: &str) -> Result<Vec<String>> {
    let keys = hr::settings_keys()?;
    let required = keys.structural_required(top);
    let Some(obj) = doc.get(top).and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    if obj.is_empty() {
        return Ok(Vec::new());
    }
    Ok(required
        .iter()
        .filter(|r| !obj.contains_key(r.as_str()))
        .cloned()
        .collect())
}

/// The report of [`SettingsDoc::validate`].
#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    /// Non-fatal observations (unknown keys the host will destroy, validator
    /// blind spots).
    pub warnings: Vec<String>,
    /// `(top-level key, validator answer)` for every key the enterprise
    /// validator could see.
    pub enterprise: Vec<(String, ConfigValidation)>,
    /// The host loader's verdict on the full candidate.
    pub load_probe: Option<LoadProbe>,
}

/// Options for [`SettingsDoc::commit`] and [`crate::trust::TrustStore::commit`].
#[derive(Clone, Debug)]
pub struct CommitOptions {
    /// Take a verified backup first (default true), under
    /// `<omm_root>/snapshots/`.
    pub backup: bool,
    /// Validate and report, write nothing (default false).
    pub dry_run: bool,
    /// `$OMM/` — where the lock (`locks/muse-config.lock`) and the backup
    /// (`snapshots/<ts>/muse-config/<file>`) live; else the root the document
    /// learned from its `Roots` (`for_roots`). With neither known a
    /// non-dry-run commit is refused ([`HostError::NoOmmRoot`]) rather than
    /// writing unlocked or dropping a backup beside the host's file.
    pub omm_root: Option<PathBuf>,
}

impl CommitOptions {
    /// Lock and backups under `roots.omm_root()`.
    pub fn for_roots(roots: &Roots) -> Self {
        CommitOptions {
            backup: true,
            dry_run: false,
            omm_root: Some(roots.omm_root()),
        }
    }
    /// Toggle dry-run.
    pub fn dry_run(mut self, on: bool) -> Self {
        self.dry_run = on;
        self
    }
}

impl Default for CommitOptions {
    fn default() -> Self {
        CommitOptions {
            backup: true,
            dry_run: false,
            omm_root: None,
        }
    }
}

/// The one lock file both writers of the host's config root take:
/// `$OMM/locks/muse-config.lock`.
pub const MUSE_CONFIG_LOCK: &str = "muse-config.lock";

/// `<omm_root>/locks/muse-config.lock`.
pub fn muse_config_lock_path(omm_root: &Path) -> PathBuf {
    omm_root.join(OMM_LOCKS_DIR).join(MUSE_CONFIG_LOCK)
}

/// What [`replace_locked`] did.
#[derive(Clone, Debug)]
pub(crate) struct Replaced {
    pub realpath: PathBuf,
    pub backup: Option<PathBuf>,
}

/// The shared write path of `settings.json` and `trust.json`: take the
/// muse-config lock, resolve the realpath, re-hash it against
/// `loaded_sha256` (the stale check, now inside the lock), back it up under
/// `<omm_root>/snapshots/<ts>/<base>/<name>` when it exists and `opts.backup`,
/// then rename the new bytes into place. `parent_missing` says the caller
/// found no parent directory before locking; it is created under the lock.
pub(crate) fn replace_locked(
    path: &Path,
    parent_missing: bool,
    loaded_sha256: Option<&str>,
    bytes: &[u8],
    opts: &CommitOptions,
    doc_omm_root: Option<&Path>,
    base: &str,
) -> Result<Replaced> {
    let omm_root =
        opts.omm_root
            .as_deref()
            .or(doc_omm_root)
            .ok_or_else(|| HostError::NoOmmRoot {
                path: path.to_path_buf(),
            })?;
    let _lock = fsx::lock_exclusive(&muse_config_lock_path(omm_root), fsx::LOCK_WAIT_DEFAULT)?;
    if parent_missing {
        if let Some(parent) = path.parent() {
            fsx::create_dir_all(parent)?;
        }
    }
    let realpath = fsx::realpath_for_write(path)?;
    let mut backup = None;
    if realpath.exists() {
        // Under the lock, immediately before the rename: nobody else can
        // land between this hash and the write.
        let found = fsx::sha256_file(&realpath)?;
        match loaded_sha256 {
            Some(expected) if expected == found => {}
            Some(expected) => {
                return Err(HostError::Stale {
                    path: realpath,
                    expected: expected.to_string(),
                    found,
                })
            }
            None => {
                return Err(HostError::Stale {
                    path: realpath,
                    expected: "<absent at load>".to_string(),
                    found,
                })
            }
        }
        if opts.backup {
            backup = Some(fsx::snapshot_backup(
                &realpath,
                &omm_root.join(OMM_SNAPSHOTS_DIR),
                base,
            )?);
        }
    }
    fsx::write_atomic(&realpath, bytes)?;
    Ok(Replaced { realpath, backup })
}

/// What [`SettingsDoc::commit`] did.
#[derive(Clone, Debug)]
pub struct CommitReport {
    /// The realpath written (or that would have been).
    pub path: PathBuf,
    /// The verified backup, `$OMM/snapshots/<ts>/muse-config/settings.json`.
    pub backup: Option<PathBuf>,
    pub bytes: usize,
    pub sha256: String,
    pub written: bool,
    pub created_dir: bool,
    pub validation: ValidationReport,
}

/// A loaded `settings.json` plus the edits staged against it.
#[derive(Clone, Debug)]
pub struct SettingsDoc {
    path: PathBuf,
    value: Value,
    existed: bool,
    loaded_sha256: Option<String>,
    touched: BTreeSet<String>,
    /// `$OMM/`, known when loaded via [`SettingsDoc::for_roots`].
    omm_root: Option<PathBuf>,
}

/// The ledger base `settings.json` / `trust.json` belong to (ARCHITECTURE.md §4).
pub const MUSE_CONFIG_BASE: &str = "muse-config";

impl SettingsDoc {
    /// Load `path`; a missing file becomes `{"schema_version": 1}`. A file that
    /// is not a JSON object is refused: there is nothing to merge into, and so
    /// is one that repeats a key at any depth — the host's loader calls that
    /// `malformed settings file … duplicate field` (exit 1), where a plain
    /// `Value` parse would silently keep the last occurrence and a commit
    /// would rewrite the file de-duplicated. A dangling symlink is refused
    /// too — `exists()` would call it missing and a commit would then replace
    /// the user's link with a regular file.
    pub fn load(path: &Path) -> Result<SettingsDoc> {
        if fsx::is_dangling_symlink(path) {
            // realpath_for_write names the target in the error.
            return Err(fsx::realpath_for_write(path).err().unwrap_or(
                HostError::DanglingSymlink {
                    path: path.to_path_buf(),
                    target: path.to_path_buf(),
                },
            ));
        }
        if !path.exists() {
            return Ok(SettingsDoc {
                path: path.to_path_buf(),
                value: base_document(),
                existed: false,
                loaded_sha256: None,
                touched: BTreeSet::new(),
                omm_root: None,
            });
        }
        let bytes = fsx::read_bytes(path)?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|e| HostError::SettingsRejected {
                stage: "load",
                detail: format!("{}: {e}", path.display()),
            })?;
        // The host refuses a repeated key; a Value parse would keep the last.
        reject_duplicate_keys(&bytes).map_err(|detail| HostError::SettingsRejected {
            stage: "load",
            detail: format!("{}: {detail}", path.display()),
        })?;
        if !value.is_object() {
            return Err(HostError::SettingsRejected {
                stage: "load",
                detail: format!("{} is not a JSON object", path.display()),
            });
        }
        Ok(SettingsDoc {
            path: path.to_path_buf(),
            value,
            existed: true,
            loaded_sha256: Some(fsx::sha256_bytes(&bytes)),
            touched: BTreeSet::new(),
            omm_root: None,
        })
    }

    /// Load the settings file of these roots; the lock and the backups go
    /// under `roots.omm_root()`.
    pub fn for_roots(roots: &Roots) -> Result<SettingsDoc> {
        let mut doc = SettingsDoc::load(&roots.settings_file())?;
        doc.omm_root = Some(roots.omm_root());
        Ok(doc)
    }

    /// The path this document was loaded from.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// The current (possibly patched) document.
    pub fn value(&self) -> &Value {
        &self.value
    }
    /// Whether the file existed at load.
    pub fn existed(&self) -> bool {
        self.existed
    }
    /// Top-level keys touched by [`SettingsDoc::patch_typed`] so far.
    pub fn touched_keys(&self) -> Vec<String> {
        self.touched.iter().cloned().collect()
    }

    /// Read a value at a path.
    pub fn get(&self, path: &[String]) -> Option<&Value> {
        let mut cur = &self.value;
        for p in path {
            cur = cur.get(p)?;
        }
        Some(cur)
    }

    /// D3.
    pub fn has_mcp_collision(&self) -> bool {
        has_mcp_collision(&self.value)
    }

    /// Top-level keys the host does not know and will destroy on its next
    /// rewrite (config-paths.md R6). Legacy `mcp_servers` is reported too: the
    /// host's rewrite drops it as well (measured 2026-09-01, `muse skills
    /// disable bundled:doctor --scope built-in` rewrote `{schema_version, tui,
    /// skills}` only).
    pub fn unknown_top_level_keys(&self) -> Result<Vec<String>> {
        let keys = hr::settings_keys()?;
        Ok(self
            .value
            .as_object()
            .map(|m| m.keys().filter(|k| !keys.is_known(k)).cloned().collect())
            .unwrap_or_default())
    }

    /// `tui.<key>` members outside the 14 typed `TuiSettings` fields
    /// (settings-keys.json `tui_fields`): the host's rewrite drops them, and
    /// neither validator sees them (the enterprise plane answers
    /// `unknown_member` for the whole projected document, the loader ignores
    /// them). Reported as `tui.<key>`. Phase 1 D4 extends this to
    /// `mcpServers.<name>.<key>` once the host's treatment of unknown server
    /// fields is measured.
    pub fn unknown_tui_keys(&self) -> Result<Vec<String>> {
        let keys = hr::settings_keys()?;
        Ok(self
            .value
            .get("tui")
            .and_then(Value::as_object)
            .map(|tui| {
                tui.keys()
                    .filter(|k| !keys.tui_fields.items.iter().any(|f| &f.key == *k))
                    .map(|k| format!("tui.{k}"))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Apply typed patches. Refuses any top-level key not in
    /// `settings-keys.json`, the legacy `mcp_servers` spelling, a
    /// `schema_version` other than 1, and any patch set that would produce
    /// the `mcpServers`/`mcp_servers` collision. Returns the prior value of
    /// every path touched, in order. Nothing is applied unless every op is
    /// acceptable.
    pub fn patch_typed(&mut self, ops: &[PatchOp]) -> Result<Vec<PriorValue>> {
        let keys = hr::settings_keys()?;
        let mut staged = self.value.clone();
        let mut priors = Vec::with_capacity(ops.len());
        let mut touched = BTreeSet::new();
        for op in ops {
            let Some(top) = op.path.first() else {
                return Err(HostError::SettingsRejected {
                    stage: "patch",
                    detail: "empty key path".to_string(),
                });
            };
            if keys.canonical_spelling(top).is_some() || !keys.is_known(top) {
                return Err(HostError::UnknownSettingsKey(op.path.join(".")));
            }
            if top == "schema_version" && op.value != Some(Value::from(hr::SETTINGS_SCHEMA_VERSION))
            {
                return Err(HostError::SettingsRejected {
                    stage: "patch",
                    detail: format!(
                        "schema_version must stay {} (0 and 2 are rejected by the host)",
                        hr::SETTINGS_SCHEMA_VERSION
                    ),
                });
            }
            let prior = value_at(&staged, &op.path).cloned();
            apply_op(&mut staged, op)?;
            priors.push(PriorValue {
                path: op.path.clone(),
                prior,
            });
            touched.insert(top.clone());
        }
        if has_mcp_collision(&staged) {
            return Err(HostError::McpCollision);
        }
        self.value = staged;
        self.touched.extend(touched);
        Ok(priors)
    }

    /// Serialize as the host does: 2-space pretty JSON, trailing newline.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(&self.value).map_err(|e| HostError::Parse {
            what: "settings document",
            detail: e.to_string(),
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Run every validator on the current document (see the module docs).
    pub fn validate(&self, inv: &Invoker) -> Result<ValidationReport> {
        let mut report = ValidationReport::default();

        // 1. Local shape.
        let obj = self
            .value
            .as_object()
            .ok_or_else(|| HostError::SettingsRejected {
                stage: "local",
                detail: "document is not a JSON object".to_string(),
            })?;
        match obj.get("schema_version").and_then(Value::as_u64) {
            Some(v) if v == hr::SETTINGS_SCHEMA_VERSION => {}
            other => {
                return Err(HostError::SettingsRejected {
                    stage: "local",
                    detail: format!(
                        "schema_version must be the integer {} (found {other:?})",
                        hr::SETTINGS_SCHEMA_VERSION
                    ),
                })
            }
        }
        if self.has_mcp_collision() {
            return Err(HostError::McpCollision);
        }
        for k in self.unknown_top_level_keys()? {
            report.warnings.push(format!(
                "top-level key `{k}` is unknown to the host and will be destroyed on its next settings rewrite"
            ));
        }
        for k in self.unknown_tui_keys()? {
            report.warnings.push(format!(
                "`{k}` is not one of the typed `tui` fields; the host drops it on its next settings rewrite and no validator reports it"
            ));
        }

        // 2. Enterprise defaults-plane validator, one touched key per document.
        let plane = hr::enterprise_defaults_plane()?;
        let keys_to_check: Vec<String> = if self.touched.is_empty() {
            obj.keys()
                .filter(|k| k.as_str() != "schema_version")
                .cloned()
                .collect()
        } else {
            self.touched.iter().cloned().collect()
        };
        let tmp = tempfile::Builder::new()
            .prefix("omm-settings-validate-")
            .tempdir()
            .map_err(|e| HostError::io("create temp dir", std::env::temp_dir(), e))?;
        for key in keys_to_check {
            let Some(value) = obj.get(&key) else {
                continue; // removed by a patch
            };
            let projected_key = plane
                .renames
                .get(&key)
                .cloned()
                .unwrap_or_else(|| key.clone());
            if !plane.accepted_members.contains(&projected_key) {
                report.warnings.push(format!(
                    "`{key}`: not a member of the enterprise defaults plane; relying on the host loader probe"
                ));
                continue;
            }
            let mut projected = value.clone();
            if let (Some(drops), Some(m)) =
                (plane.user_only_subkeys.get(&key), projected.as_object_mut())
            {
                for d in drops {
                    m.remove(d);
                }
            }
            let mut settings = Map::new();
            settings.insert(projected_key, projected);
            let mut doc = Map::new();
            doc.insert("schema_version".to_string(), Value::from(1));
            doc.insert("settings".to_string(), Value::Object(settings));
            let file = tmp.path().join(format!("{}.json", sanitize(&key)));
            let bytes = serde_json::to_vec(&Value::Object(doc)).map_err(|e| HostError::Parse {
                what: "projected settings document",
                detail: e.to_string(),
            })?;
            std::fs::write(&file, bytes).map_err(|e| HostError::io("write", &file, e))?;
            let verdict = probe::config_validate(inv, &plane.plane, &file)?;
            match &verdict {
                v if v.is_accepted() => {}
                v if v
                    .reason()
                    .map(|r| plane.blind_outcomes.iter().any(|b| b == r))
                    .unwrap_or(false) =>
                {
                    report.warnings.push(format!(
                        "`{key}`: the enterprise validator answered {}; relying on the host loader probe",
                        v.summary()
                    ));
                }
                v => {
                    return Err(HostError::SettingsRejected {
                        stage: "enterprise-validator",
                        detail: format!("`{key}`: {}", v.summary()),
                    })
                }
            }
            report.enterprise.push((key, verdict));
        }

        // 3. The host's own loader.
        let load = probe::settings_load_probe(inv, &self.to_bytes()?)?;
        if !load.accepted {
            return Err(HostError::SettingsRejected {
                stage: "host-loader",
                detail: load.detail,
            });
        }
        report.load_probe = Some(load);
        Ok(report)
    }

    /// Validate, then — under the muse-config lock — re-check staleness, back
    /// up, and atomically replace the file on its realpath ([`replace_locked`]).
    pub fn commit(&self, inv: &Invoker, opts: &CommitOptions) -> Result<CommitReport> {
        let validation = self.validate(inv)?;
        let bytes = self.to_bytes()?;
        let sha256 = fsx::sha256_bytes(&bytes);
        let parent = self.path.parent().ok_or_else(|| HostError::Io {
            context: "resolve parent of",
            path: self.path.clone(),
            source: std::io::Error::other("path has no parent"),
        })?;
        let created_dir = !parent.exists();
        if opts.dry_run {
            return Ok(CommitReport {
                path: if created_dir {
                    self.path.clone()
                } else {
                    fsx::realpath_for_write(&self.path)?
                },
                backup: None,
                bytes: bytes.len(),
                sha256,
                written: false,
                created_dir,
                validation,
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
        Ok(CommitReport {
            path: replaced.realpath,
            backup: replaced.backup,
            bytes: bytes.len(),
            sha256,
            written: true,
            created_dir,
            validation,
        })
    }
}

/// Refuse a JSON document that repeats a key inside any object, the way the
/// host's typed loader does (`malformed settings file at …: duplicate field
/// \`tui\` at line 1 column 45`, exit 1 — measured 2026-09-02 with `muse
/// skills list --source user --json`). A `serde_json::Value` parse keeps the
/// last occurrence silently, so without this check a commit would land the
/// de-duplicated document — the first value lost, the last recorded as the
/// ledger prior — and the host's verdict on the on-disk bytes would never be
/// consulted (`validate` probes the re-serialized candidate). The error text
/// carries the key and serde_json's line/column, mirroring the host's.
pub(crate) fn reject_duplicate_keys(bytes: &[u8]) -> std::result::Result<(), String> {
    serde_json::from_slice::<NoDuplicateKeys>(bytes)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// A deserialize target that accepts every JSON value and fails only on a
/// repeated object key at any depth.
struct NoDuplicateKeys;

impl<'de> serde::Deserialize<'de> for NoDuplicateKeys {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        d.deserialize_any(NoDuplicateKeysVisitor)
    }
}

struct NoDuplicateKeysVisitor;

impl<'de> serde::de::Visitor<'de> for NoDuplicateKeysVisitor {
    type Value = NoDuplicateKeys;

    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a JSON value without duplicate object keys")
    }
    fn visit_bool<E>(self, _: bool) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_i64<E>(self, _: i64) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_u64<E>(self, _: u64) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_f64<E>(self, _: f64) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_str<E>(self, _: &str) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(NoDuplicateKeys)
    }
    fn visit_some<D: serde::Deserializer<'de>>(
        self,
        d: D,
    ) -> std::result::Result<Self::Value, D::Error> {
        d.deserialize_any(NoDuplicateKeysVisitor)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(
        self,
        mut seq: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        while seq.next_element::<NoDuplicateKeys>()?.is_some() {}
        Ok(NoDuplicateKeys)
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(
        self,
        mut map: A,
    ) -> std::result::Result<Self::Value, A::Error> {
        let mut seen = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!("duplicate field `{key}`")));
            }
            map.next_value::<NoDuplicateKeys>()?;
        }
        Ok(NoDuplicateKeys)
    }
}

fn base_document() -> Value {
    let mut m = Map::new();
    m.insert(
        "schema_version".to_string(),
        Value::from(hr::SETTINGS_SCHEMA_VERSION),
    );
    Value::Object(m)
}

fn value_at<'a>(root: &'a Value, path: &[String]) -> Option<&'a Value> {
    let mut cur = root;
    for p in path {
        cur = cur.get(p)?;
    }
    Some(cur)
}

fn apply_op(root: &mut Value, op: &PatchOp) -> Result<()> {
    let (leaf, parents) = op
        .path
        .split_last()
        .ok_or_else(|| HostError::SettingsRejected {
            stage: "patch",
            detail: "empty key path".to_string(),
        })?;
    let mut cur = root;
    for (i, p) in parents.iter().enumerate() {
        let obj = cur
            .as_object_mut()
            .ok_or_else(|| HostError::SettingsRejected {
                stage: "patch",
                detail: format!(
                    "`{}` is not an object; cannot set `{}` beneath it",
                    op.path[..i].join("."),
                    op.path.join(".")
                ),
            })?;
        if op.value.is_none() && !obj.contains_key(p) {
            return Ok(()); // removing beneath a missing parent: nothing to do
        }
        cur = obj
            .entry(p.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let obj = cur
        .as_object_mut()
        .ok_or_else(|| HostError::SettingsRejected {
            stage: "patch",
            detail: format!(
                "`{}` is not an object; cannot set `{leaf}` beneath it",
                parents.join(".")
            ),
        })?;
    match &op.value {
        Some(v) => {
            obj.insert(leaf.clone(), v.clone());
        }
        None => {
            obj.remove(leaf);
        }
    }
    Ok(())
}

fn sanitize(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn doc(v: Value) -> SettingsDoc {
        SettingsDoc {
            path: PathBuf::from("/nonexistent/settings.json"),
            value: v,
            existed: true,
            loaded_sha256: None,
            touched: BTreeSet::new(),
            omm_root: None,
        }
    }

    #[test]
    fn unknown_tui_members_are_reported() {
        let d = doc(
            json!({"schema_version": 1, "tui": {"theme": "old", "unknown_sub": true}, "provider": "echo"}),
        );
        assert_eq!(d.unknown_tui_keys().unwrap(), vec!["tui.unknown_sub"]);
        assert!(doc(json!({"schema_version": 1}))
            .unknown_tui_keys()
            .unwrap()
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_not_a_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("settings.json");
        std::os::unix::fs::symlink(dir.path().join("gone.json"), &link).unwrap();
        assert!(matches!(
            SettingsDoc::load(&link),
            Err(HostError::DanglingSymlink { .. })
        ));
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn commit_without_an_omm_root_is_refused_not_written_beside() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(&file, b"{\"schema_version\":1}\n").unwrap();
        let d = SettingsDoc::load(&file).unwrap();
        // No invoker is needed: the write path is exercised directly.
        let err = replace_locked(
            &file,
            false,
            d.loaded_sha256.as_deref(),
            b"{}",
            &CommitOptions::default(),
            d.omm_root.as_deref(),
            MUSE_CONFIG_BASE,
        )
        .unwrap_err();
        assert!(matches!(err, HostError::NoOmmRoot { .. }), "{err}");
        assert_eq!(dir.path().read_dir().unwrap().count(), 1);
        assert_eq!(std::fs::read(&file).unwrap(), b"{\"schema_version\":1}\n");
    }

    #[test]
    fn replace_locked_takes_the_lock_rehashes_and_backs_up_under_omm_root() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("muse").join("settings.json");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"{\"schema_version\":1}\n").unwrap();
        let omm = dir.path().join("omm");
        let opts = CommitOptions {
            omm_root: Some(omm.clone()),
            ..CommitOptions::default()
        };
        let loaded = fsx::sha256_file(&file).unwrap();
        // Holding the lock elsewhere makes the commit wait, then refuse.
        let held =
            fsx::lock_exclusive(&muse_config_lock_path(&omm), fsx::LOCK_WAIT_DEFAULT).unwrap();
        let t = std::thread::spawn({
            let (file, opts, loaded) = (file.clone(), opts.clone(), loaded.clone());
            move || {
                replace_locked(
                    &file,
                    false,
                    Some(&loaded),
                    b"{\"schema_version\":1,\"tui\":{}}\n",
                    &opts,
                    None,
                    MUSE_CONFIG_BASE,
                )
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(!t.is_finished(), "the writer must wait for the lock");
        drop(held);
        let done = t.join().unwrap().unwrap();
        assert!(done.backup.unwrap().starts_with(omm.join("snapshots")));
        assert!(muse_config_lock_path(&omm).exists());
        // The stale check runs against the bytes on disk at lock time.
        let err = replace_locked(
            &file,
            false,
            Some(&loaded),
            b"{}",
            &opts,
            None,
            MUSE_CONFIG_BASE,
        )
        .unwrap_err();
        assert!(matches!(err, HostError::Stale { .. }), "{err}");
        assert_eq!(
            file.parent().unwrap().read_dir().unwrap().count(),
            1,
            "nothing beside the host's file"
        );
    }

    #[test]
    fn patch_records_priors_and_creates_nesting() {
        let mut d = doc(json!({"schema_version": 1, "tui": {"theme": "old"}}));
        let priors = d
            .patch_typed(&[
                PatchOp::set("tui.theme", json!("custom:x")),
                PatchOp::set("run.workflow_trigger_mode", json!("off")),
                PatchOp::remove("tui.missing"),
            ])
            .unwrap();
        assert_eq!(priors[0].prior, Some(json!("old")));
        assert_eq!(priors[0].key(), "tui.theme");
        assert_eq!(priors[1].prior, None);
        assert_eq!(d.value()["run"]["workflow_trigger_mode"], "off");
        assert_eq!(d.touched_keys(), vec!["run", "tui"]);
    }

    #[test]
    fn patch_refuses_unknown_legacy_and_schema_changes() {
        let mut d = doc(json!({"schema_version": 1}));
        assert!(matches!(
            d.patch_typed(&[PatchOp::set("_omm_marker", json!(1))]),
            Err(HostError::UnknownSettingsKey(_))
        ));
        assert!(matches!(
            d.patch_typed(&[PatchOp::set("mcp_servers", json!({}))]),
            Err(HostError::UnknownSettingsKey(_))
        ));
        assert!(matches!(
            d.patch_typed(&[PatchOp::set("schema_version", json!(2))]),
            Err(HostError::SettingsRejected { .. })
        ));
        assert!(d
            .patch_typed(&[PatchOp::set("schema_version", json!(1))])
            .is_ok());
        // Nothing applied on failure.
        assert_eq!(d.value(), &json!({"schema_version": 1}));
    }

    #[test]
    fn patch_refuses_creating_the_mcp_collision() {
        let mut d = doc(json!({"schema_version": 1, "mcp_servers": {}}));
        assert!(matches!(
            d.patch_typed(&[PatchOp::set("mcpServers", json!({}))]),
            Err(HostError::McpCollision)
        ));
        assert!(has_mcp_collision(
            &json!({"mcpServers": {}, "mcp_servers": {}})
        ));
        assert!(!has_mcp_collision(&json!({"mcpServers": {}})));
    }

    #[test]
    fn patch_refuses_nesting_under_scalars() {
        let mut d = doc(json!({"schema_version": 1, "provider": "echo"}));
        assert!(d
            .patch_typed(&[PatchOp::set("provider.x", json!(1))])
            .is_err());
    }

    #[test]
    fn unknown_keys_are_reported() {
        let d = doc(json!({"schema_version": 1, "_marker": 1, "mcp_servers": {}, "tui": {}}));
        assert_eq!(
            d.unknown_top_level_keys().unwrap(),
            vec!["_marker", "mcp_servers"]
        );
        // The members an uninstall carries back: the legacy spelling is not
        // one of them (D3), the `tui` extras are.
        let members = untyped_members(&json!({
            "schema_version": 1, "_marker": 1, "mcp_servers": {}, "provider": "meta",
            "tui": {"theme": "x", "my_extra": 42}
        }))
        .unwrap();
        assert_eq!(
            members,
            vec![
                (vec!["_marker".to_string()], json!(1)),
                (vec!["tui".to_string(), "my_extra".to_string()], json!(42)),
            ]
        );
        assert!(untyped_members(&json!([])).unwrap().is_empty());
    }

    #[test]
    fn duplicate_keys_are_refused_at_load_like_the_host() {
        // The host: `malformed settings file at …: duplicate field `tui` at
        // line 1 column 45`, exit 1 (`muse skills list --source user --json`,
        // 2026-09-02). A Value parse keeps the last `tui` silently, and a
        // commit would then land the de-duplicated document with `a` lost and
        // `b` recorded as the ledger prior.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let top = b"{\"schema_version\":1,\"tui\":{\"theme\":\"a\"},\"tui\":{\"theme\":\"b\"}}";
        std::fs::write(&file, top).unwrap();
        match SettingsDoc::load(&file) {
            Err(HostError::SettingsRejected { stage, detail }) => {
                assert_eq!(stage, "load");
                assert!(detail.contains("duplicate field `tui`"), "{detail}");
                assert!(detail.contains(&file.display().to_string()), "{detail}");
            }
            other => panic!("expected SettingsRejected at load, got {other:?}"),
        }
        // Nested duplicates are typed-struct errors on the host too
        // (host-reality "settings loader, run.context_slimming").
        std::fs::write(
            &file,
            b"{\"schema_version\":1,\"tui\":{\"theme\":\"a\",\"theme\":\"b\"}}",
        )
        .unwrap();
        match SettingsDoc::load(&file) {
            Err(HostError::SettingsRejected { stage, detail }) => {
                assert_eq!(stage, "load");
                assert!(detail.contains("duplicate field `theme`"), "{detail}");
            }
            other => panic!("expected SettingsRejected at load, got {other:?}"),
        }
        // Inside an array of objects as well; an honest document still loads.
        std::fs::write(
            &file,
            b"{\"schema_version\":1,\"plugins\":{\"x\":[{\"k\":1,\"k\":2}]}}",
        )
        .unwrap();
        assert!(matches!(
            SettingsDoc::load(&file),
            Err(HostError::SettingsRejected { stage: "load", .. })
        ));
        std::fs::write(
            &file,
            b"{\"schema_version\":1,\"tui\":{\"theme\":\"a\"},\"provider\":\"echo\"}\n",
        )
        .unwrap();
        let d = SettingsDoc::load(&file).unwrap();
        assert_eq!(d.value()["tui"]["theme"], "a");
        assert!(reject_duplicate_keys(b"{\"a\":{\"b\":1},\"c\":[{\"b\":1},{\"b\":2}]}").is_ok());
        assert!(reject_duplicate_keys(b"[1,2,3]").is_ok());
        assert!(reject_duplicate_keys(b"{\"a\":1,\"a\":1}").is_err());
    }

    #[test]
    fn a_structural_member_is_kept_while_the_parent_has_other_members() {
        let path = |s: &str| -> Vec<String> { s.split('.').map(str::to_string).collect() };
        // The user's own profile beside omm's: schema_version must stay.
        let doc = json!({"schema_version": 1, "permissions": {"schema_version": 1, "profiles": {"mine": {}}}});
        let why = structural_keep_reason(&doc, &path("permissions.schema_version"))
            .unwrap()
            .expect("kept");
        assert!(why.contains("`permissions.profiles`"), "{why}");
        assert!(why.contains("refuse the whole object"), "{why}");
        // Nothing but structural members left: it may go.
        let doc = json!({"schema_version": 1, "permissions": {"schema_version": 1}});
        assert_eq!(
            structural_keep_reason(&doc, &path("permissions.schema_version")).unwrap(),
            None
        );
        // Not a structural leaf, or not a depth-2 path: never kept for this reason.
        let doc = json!({"schema_version": 1, "permissions": {"schema_version": 1, "default_profile": "x", "profiles": {"mine": {}}}});
        assert_eq!(
            structural_keep_reason(&doc, &path("permissions.default_profile")).unwrap(),
            None
        );
        assert_eq!(
            structural_keep_reason(&doc, &path("permissions.profiles.mine")).unwrap(),
            None
        );
        assert_eq!(
            structural_keep_reason(&doc, &path("tui.theme")).unwrap(),
            None
        );
        // The parent absent or not an object: nothing to keep.
        assert_eq!(
            structural_keep_reason(
                &json!({"schema_version": 1}),
                &path("permissions.schema_version")
            )
            .unwrap(),
            None
        );
        // D4's side: a present, non-empty object lacking the member.
        assert_eq!(
            missing_structural_members(
                &json!({"permissions": {"profiles": {"mine": {}}}}),
                "permissions"
            )
            .unwrap(),
            vec!["schema_version".to_string()]
        );
        assert!(
            missing_structural_members(&json!({"permissions": {}}), "permissions")
                .unwrap()
                .is_empty()
        );
        assert!(missing_structural_members(&json!({}), "permissions")
            .unwrap()
            .is_empty());
        assert!(missing_structural_members(&doc, "permissions")
            .unwrap()
            .is_empty());
        assert!(missing_structural_members(&doc, "tui").unwrap().is_empty());
    }

    #[test]
    fn missing_file_loads_as_base() {
        let d = SettingsDoc::load(Path::new("/nonexistent/omm/settings.json")).unwrap();
        assert!(!d.existed());
        assert_eq!(d.value(), &json!({"schema_version": 1}));
        assert!(d.to_bytes().unwrap().ends_with(b"\n"));
    }
}
