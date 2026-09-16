//! The on-disk shape of `omm.lock.json` (ARCHITECTURE.md §4).
//!
//! ```jsonc
//! { "schema_version": 1, "omm_version": "0.1.0",
//!   "host": { "version": "1.0.1-R2006.1", "sha256": "…" },   // observed at install
//!   "scope": "user",
//!   "entries": [ { "base": "muse-config", "path": "skills/x/SKILL.md", "kind": "skill",
//!                  "sha256": "…", "source_version": "0.1.0", "writer": "omm install",
//!                  "mechanism": "copy", "class": "exclusive", "prior": null } ],
//!   "registrations": [ { "kind": "muse-plugin", "id": "omm", "package_sha256": "…",
//!                        "generation_path": "…", "approved": ["plugin:omm:hook:x"] },
//!                      { "kind": "muse-marketplace", "name": "ohmy", "source": "…" },
//!                      { "kind": "settings-key", "path": "tui.theme", "prior": null },
//!                      { "kind": "trust", "project": "/abs/ws", "prior": null } ] }
//! ```
//!
//! R2: an entry records what omm **wrote** — `sha256` is the ancestor of the
//! next reconcile — never a scan of the destination. Paths are ALWAYS
//! base-relative with forward slashes ([`RelPath`]); nothing here names an
//! asset (R8). Every document is `deny_unknown_fields`: a field this version
//! does not know is a corrupt ledger, not a silently dropped one. Entries are
//! kept sorted by `(base, path)` so reruns are byte-stable.
//!
//! Settings keys and trust entries are undone by value, not by file: they are
//! [`Registration::SettingsKey`] / [`Registration::Trust`] carrying the
//! recorded `prior` (00-DECISION.md §2.2: "restore by targeted merge", never
//! the file wholesale). A file entry whose mechanism is `settings-patch` or
//! `trust-merge` is therefore a *non-file* entry: [`Mechanism::is_file`] is
//! false and no module removes, overwrites or hashes it — with one
//! convention for R5 (byte-identical uninstall on a clean home): such an
//! entry with `class: seeded` records that omm **created** the shared file,
//! and uninstall unlinks it after the restores when nothing but
//! `schema_version` (and empty objects) is left in it.

use std::collections::BTreeSet;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{LedgerError, Result};

/// The only schema this crate reads or writes.
pub const SCHEMA_VERSION: u32 = 1;

/// The four allowlisted bases (R4). Declaration order is the sort order and
/// matches the alphabetical order of the on-disk spellings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Base {
    /// `$XDG_CONFIG_HOME/muse` else `~/.config/muse` (`omm_host::Roots::muse_config`).
    MuseConfig,
    /// `$XDG_DATA_HOME/muse` else `~/.local/share/muse` (`Roots::muse_data`).
    MuseData,
    /// `$XDG_CONFIG_HOME/omm` else `~/.config/omm` (`Roots::omm_root`).
    Omm,
    /// The workspace `omm` was run in (project scope).
    Workspace,
}

impl Base {
    /// Every base, in sort order.
    pub const ALL: [Base; 4] = [Base::MuseConfig, Base::MuseData, Base::Omm, Base::Workspace];

    /// The on-disk spelling (also the directory name under `snapshots/<ts>/`
    /// and `updates/<version>/`).
    pub fn as_str(self) -> &'static str {
        match self {
            Base::MuseConfig => "muse-config",
            Base::MuseData => "muse-data",
            Base::Omm => "omm",
            Base::Workspace => "workspace",
        }
    }
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What an entry is (ARCHITECTURE.md §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Skill,
    Command,
    Hook,
    Agent,
    Theme,
    Rules,
    SettingsKey,
    Trust,
    Plugin,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Skill => "skill",
            Kind::Command => "command",
            Kind::Hook => "hook",
            Kind::Agent => "agent",
            Kind::Theme => "theme",
            Kind::Rules => "rules",
            Kind::SettingsKey => "settings-key",
            Kind::Trust => "trust",
            Kind::Plugin => "plugin",
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How the entry got onto disk (ARCHITECTURE.md §4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mechanism {
    /// omm copied the bytes itself.
    Copy,
    /// `muse skills install` put it in the managed personal store
    /// (`research/musecode/config-paths.md` §3: `skills/.muse/{lock.json,audit.log}`).
    MuseSkillsInstall,
    /// `muse plugins install` — the host owns the tree under `plugins/`.
    MusePluginsInstall,
    /// A targeted `settings.json` key patch (R9/R10) — undone by value.
    SettingsPatch,
    /// A `trust.json` project merge — undone by value.
    TrustMerge,
    /// One handler merged into a pre-existing project `.muse/hooks.json`
    /// (`omm enable skill-routing`, PLAN.md 3.1): the file is the user's,
    /// its pre-omm bytes and mode ride in `prior.original`; `omm disable
    /// skill-routing` removes the handler and restores those bytes when
    /// nothing else changed, and uninstall restores them only once the
    /// handler is gone (else keeps the file and names it).
    HooksMerge,
}

impl Mechanism {
    pub fn as_str(self) -> &'static str {
        match self {
            Mechanism::Copy => "copy",
            Mechanism::MuseSkillsInstall => "muse-skills-install",
            Mechanism::MusePluginsInstall => "muse-plugins-install",
            Mechanism::SettingsPatch => "settings-patch",
            Mechanism::TrustMerge => "trust-merge",
            Mechanism::HooksMerge => "hooks-merge",
        }
    }

    /// True when the entry names a file omm may hash, overwrite, snapshot or
    /// remove. `settings-patch`, `trust-merge` and `hooks-merge` entries name
    /// a shared file omm does not own; their undo is a [`Registration`] with
    /// a `prior` (or, for `hooks-merge`, the handler removal of `omm disable
    /// skill-routing`).
    pub fn is_file(self) -> bool {
        !matches!(
            self,
            Mechanism::SettingsPatch | Mechanism::TrustMerge | Mechanism::HooksMerge
        )
    }
}

impl fmt::Display for Mechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who else may legitimately touch the entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Class {
    /// Ours alone: removed on uninstall when unedited.
    Exclusive,
    /// One key of a file we share (`settings.json`, `trust.json`).
    SharedKey,
    /// We seeded it and the user is expected to edit it (`AGENTS.md`).
    Seeded,
}

impl Class {
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Exclusive => "exclusive",
            Class::SharedKey => "shared-key",
            Class::Seeded => "seeded",
        }
    }
}

/// Install scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    User,
    Project,
}

/// The host observed at install.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostInfo {
    /// `muse --version` (informational, never gated on — R15).
    pub version: String,
    /// SHA-256 of the `muse-bin-<version>` binary.
    pub sha256: String,
}

/// A base-relative path: non-empty, forward slashes only, no leading `/`, no
/// backslash, no `.`/`..`/empty component, no NUL, no trailing slash, no
/// drive letter. Validated on construction and on deserialize, so a ledger
/// that carries an absolute or escaping path is corrupt, not dangerous.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RelPath(String);

impl RelPath {
    /// Validate and wrap.
    pub fn new(s: impl Into<String>) -> Result<RelPath> {
        let s = s.into();
        validate_rel(&s).map_err(|reason| LedgerError::InvalidPath {
            path: s.clone(),
            reason,
        })?;
        Ok(RelPath(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Forward-slash components.
    pub fn components(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }

    /// Number of components (`a/b/c` → 3).
    pub fn depth(&self) -> usize {
        self.0.split('/').count()
    }

    /// The path joined under `root` with the platform separator.
    pub fn under(&self, root: &Path) -> PathBuf {
        let mut p = root.to_path_buf();
        for c in self.components() {
            p.push(c);
        }
        p
    }
}

impl fmt::Display for RelPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RelPath {
    type Error = LedgerError;
    fn try_from(s: String) -> Result<RelPath> {
        RelPath::new(s)
    }
}

impl TryFrom<&str> for RelPath {
    type Error = LedgerError;
    fn try_from(s: &str) -> Result<RelPath> {
        RelPath::new(s)
    }
}

impl From<RelPath> for String {
    fn from(p: RelPath) -> String {
        p.0
    }
}

/// The grammar behind [`RelPath`]; `Err(reason)` names the first violation.
pub fn validate_rel(s: &str) -> std::result::Result<(), String> {
    if s.is_empty() {
        return Err("empty".to_string());
    }
    if s.contains('\0') {
        return Err("contains NUL".to_string());
    }
    if s.contains('\\') {
        return Err("contains a backslash; forward slashes only".to_string());
    }
    if s.starts_with('/') {
        return Err("absolute; paths are base-relative".to_string());
    }
    let b = s.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return Err("drive-letter prefix; paths are base-relative".to_string());
    }
    if s.ends_with('/') {
        return Err("trailing slash".to_string());
    }
    for c in s.split('/') {
        match c {
            "" => return Err("empty component (`//`)".to_string()),
            "." => return Err("`.` component".to_string()),
            ".." => return Err("`..` component".to_string()),
            _ => {}
        }
    }
    Ok(())
}

/// `(base, path)` — the identity of an entry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntryKey {
    pub base: Base,
    pub path: RelPath,
}

impl fmt::Display for EntryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.base, self.path)
    }
}

/// One thing omm wrote (ARCHITECTURE.md §4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub base: Base,
    /// ALWAYS base-relative, forward slashes, never absolute, never `..`.
    pub path: RelPath,
    pub kind: Kind,
    /// What WE wrote (R2) — the ancestor of the next reconcile.
    pub sha256: String,
    pub source_version: String,
    /// `omm install` / `omm update` / …
    pub writer: String,
    pub mechanism: Mechanism,
    pub class: Class,
    /// For settings-key / trust entries: the value before we touched it.
    #[serde(default)]
    pub prior: Option<Value>,
}

impl Entry {
    pub fn key(&self) -> EntryKey {
        EntryKey {
            base: self.base,
            path: self.path.clone(),
        }
    }

    /// See [`Mechanism::is_file`].
    pub fn is_file(&self) -> bool {
        self.mechanism.is_file()
    }
}

/// Things that are not files but must be undone (ARCHITECTURE.md §4,
/// docs/experiments/marketplace-precedence.md §6.4 step 5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Registration {
    /// `muse plugins install <id>@<marketplace>`; undone with
    /// `muse plugins remove <id> --delete-data` (00-DECISION.md §2.2).
    MusePlugin {
        id: String,
        /// `installed.package_sha256` — compared with the marketplace digest
        /// on update (equal → no-op).
        package_sha256: String,
        /// `installed.source.path`, the pinned marketplace generation
        /// (host-reality.md "marketplace generations"); for doctor.
        generation_path: String,
        /// Stable capability ids approved and verified `trusted_enabled` (R14).
        approved: Vec<String>,
    },
    /// `muse plugins marketplace add <name> <source>`; undone with
    /// `muse plugins marketplace remove <name>`.
    MuseMarketplace { name: String, source: String },
    /// One typed `settings.json` key (R9), `path` dotted from the top level;
    /// restored to `prior` (`None` = the key was absent) by a targeted patch
    /// — but only while the key still holds `value`, what omm wrote last:
    /// a key the user edited since is preserved like an edited file (Gate
    /// 1: `omm theme` then a hand edit of `tui.theme` lost the user's
    /// choice at uninstall). `value` is `None` in a ledger written before
    /// it was recorded; such a key is restored unconditionally.
    SettingsKey {
        path: String,
        #[serde(default)]
        prior: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
        /// The profile whose slice wrote the key last (`omm install` /
        /// `omm profile use`); `None` for every other writer. A profile
        /// switch restores every key tagged with another profile that the
        /// incoming slice leaves alone (Gate 1 `prof`: default → strict →
        /// default kept strict's `permissions.*`), so the restore never
        /// depends on `config.json` naming the outgoing profile.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        profile: Option<String>,
    },
    /// One `trust.json` project key (the canonical workspace root); restored
    /// to `prior` (`None` = the project had no entry) while the entry still
    /// equals `value`, the entry omm wrote (same rule as a settings key).
    Trust {
        project: String,
        #[serde(default)]
        prior: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        value: Option<Value>,
    },
}

impl Registration {
    /// A short label for reports and audit lines.
    pub fn label(&self) -> String {
        match self {
            Registration::MusePlugin { id, .. } => format!("muse-plugin {id}"),
            Registration::MuseMarketplace { name, .. } => format!("muse-marketplace {name}"),
            Registration::SettingsKey { path, .. } => format!("settings-key {path}"),
            Registration::Trust { project, .. } => format!("trust {project}"),
        }
    }
}

/// `omm.lock.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ledger {
    pub schema_version: u32,
    pub omm_version: String,
    pub host: HostInfo,
    pub scope: Scope,
    #[serde(default)]
    pub entries: Vec<Entry>,
    #[serde(default)]
    pub registrations: Vec<Registration>,
}

impl Ledger {
    /// An empty ledger for this omm and host.
    pub fn new(omm_version: impl Into<String>, host: HostInfo, scope: Scope) -> Ledger {
        Ledger {
            schema_version: SCHEMA_VERSION,
            omm_version: omm_version.into(),
            host,
            scope,
            entries: Vec::new(),
            registrations: Vec::new(),
        }
    }

    /// Sort entries by `(base, path)`. Stable, so equal keys keep their
    /// order (and are then refused by [`Ledger::validate`]).
    pub fn sort(&mut self) {
        self.entries.sort_by_key(Entry::key);
    }

    /// True when [`Ledger::sort`] would change nothing.
    pub fn is_sorted(&self) -> bool {
        self.entries.windows(2).all(|w| w[0].key() <= w[1].key())
    }

    /// Schema version 1, no duplicate `(base, path)`, no duplicate
    /// registration. `path` is validated by [`RelPath`] on construction.
    pub fn validate(&self) -> std::result::Result<(), String> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(format!(
                "schema_version {} is not {SCHEMA_VERSION}",
                self.schema_version
            ));
        }
        let mut seen = BTreeSet::new();
        for e in &self.entries {
            if !seen.insert(e.key()) {
                return Err(format!("duplicate entry {}", e.key()));
            }
        }
        let mut regs = Vec::new();
        for r in &self.registrations {
            if regs.contains(r) {
                return Err(format!("duplicate registration {}", r.label()));
            }
            regs.push(r.clone());
        }
        Ok(())
    }

    pub fn find(&self, base: Base, path: &RelPath) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.base == base && &e.path == path)
    }

    pub fn find_mut(&mut self, base: Base, path: &RelPath) -> Option<&mut Entry> {
        self.entries
            .iter_mut()
            .find(|e| e.base == base && &e.path == path)
    }

    /// Insert or replace by key; keeps the vector sorted. Returns what was
    /// replaced.
    pub fn upsert(&mut self, entry: Entry) -> Option<Entry> {
        let key = entry.key();
        match self.entries.binary_search_by(|e| e.key().cmp(&key)) {
            Ok(i) => Some(std::mem::replace(&mut self.entries[i], entry)),
            Err(i) => {
                if self.is_sorted() {
                    self.entries.insert(i, entry);
                } else {
                    self.entries.push(entry);
                    self.sort();
                }
                None
            }
        }
    }

    /// [`Ledger::upsert`] that keeps the present entry's `prior` when the
    /// new entry carries none — a shared file's pre-omm record
    /// (`shared::PRIOR_ORIGINAL`) must survive every later writer of the
    /// same entry (`omm theme`, a profile switch, a second install).
    pub fn upsert_keep_prior(&mut self, mut entry: Entry) -> Option<Entry> {
        if entry.prior.is_none() {
            if let Some(existing) = self.find(entry.base, &entry.path) {
                entry.prior = existing.prior.clone();
            }
        }
        self.upsert(entry)
    }

    /// Remove by key.
    pub fn remove(&mut self, base: Base, path: &RelPath) -> Option<Entry> {
        let i = self
            .entries
            .iter()
            .position(|e| e.base == base && &e.path == path)?;
        Some(self.entries.remove(i))
    }

    /// Entries that name a file omm owns ([`Mechanism::is_file`]).
    pub fn file_entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter().filter(|e| e.is_file())
    }

    /// Add a registration unless an identical one is present.
    pub fn register(&mut self, reg: Registration) -> bool {
        if self.registrations.contains(&reg) {
            return false;
        }
        self.registrations.push(reg);
        true
    }

    /// Record one settings key omm wrote: the FIRST prior wins (an existing
    /// registration keeps its `prior`), `value` and the `profile` tag are
    /// always the last writer's. Returns true when a registration was added.
    pub fn record_settings_key(
        &mut self,
        key: &str,
        prior: Option<Value>,
        value: Option<Value>,
        profile: Option<&str>,
    ) -> bool {
        for r in &mut self.registrations {
            if let Registration::SettingsKey {
                path,
                value: v,
                profile: p,
                ..
            } = r
            {
                if path == key {
                    *v = value;
                    *p = profile.map(str::to_string);
                    return false;
                }
            }
        }
        self.registrations.push(Registration::SettingsKey {
            path: key.to_string(),
            prior,
            value,
            profile: profile.map(str::to_string),
        });
        true
    }

    /// Every settings key tagged with a profile: `(key, profile)`.
    pub fn settings_keys_by_profile(&self) -> Vec<(String, String)> {
        self.registrations
            .iter()
            .filter_map(|r| match r {
                Registration::SettingsKey {
                    path,
                    profile: Some(p),
                    ..
                } => Some((path.clone(), p.clone())),
                _ => None,
            })
            .collect()
    }

    /// The `settings-key` registration of `key`, if any.
    pub fn settings_key(&self, key: &str) -> Option<&Registration> {
        self.registrations
            .iter()
            .find(|r| matches!(r, Registration::SettingsKey { path, .. } if path == key))
    }

    /// Drop the `settings-key` registration of `key`; true when one was there.
    pub fn forget_settings_key(&mut self, key: &str) -> bool {
        let before = self.registrations.len();
        self.registrations
            .retain(|r| !matches!(r, Registration::SettingsKey { path, .. } if path == key));
        self.registrations.len() != before
    }

    /// Record one trust entry omm wrote, with the same first-prior /
    /// last-value rule as [`Ledger::record_settings_key`].
    pub fn record_trust(
        &mut self,
        project: &str,
        prior: Option<Value>,
        value: Option<Value>,
    ) -> bool {
        for r in &mut self.registrations {
            if let Registration::Trust {
                project: p,
                value: v,
                ..
            } = r
            {
                if p == project {
                    *v = value;
                    return false;
                }
            }
        }
        self.registrations.push(Registration::Trust {
            project: project.to_string(),
            prior,
            value,
        });
        true
    }

    /// 2-space pretty JSON with a trailing newline, entries sorted.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut sorted = self.clone();
        sorted.sort();
        let mut bytes = serde_json::to_vec_pretty(&sorted).map_err(|e| LedgerError::Schema {
            path: PathBuf::from(crate::store::LEDGER_FILE),
            detail: e.to_string(),
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Parse and validate; `path` only labels errors.
    pub fn from_bytes(bytes: &[u8], path: &Path) -> Result<Ledger> {
        let ledger: Ledger = serde_json::from_slice(bytes).map_err(|e| LedgerError::Schema {
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
        ledger.validate().map_err(|detail| LedgerError::Schema {
            path: path.to_path_buf(),
            detail,
        })?;
        Ok(ledger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(base: Base, path: &str, sha: &str) -> Entry {
        Entry {
            base,
            path: RelPath::new(path).unwrap(),
            kind: Kind::Skill,
            sha256: sha.to_string(),
            source_version: "0.1.0".to_string(),
            writer: "omm install".to_string(),
            mechanism: Mechanism::Copy,
            class: Class::Exclusive,
            prior: None,
        }
    }

    fn ledger() -> Ledger {
        Ledger::new(
            "0.1.0",
            HostInfo {
                version: "1.0.1-R2006.1".into(),
                sha256: "b9c7".into(),
            },
            Scope::User,
        )
    }

    #[test]
    fn relpath_grammar() {
        for bad in [
            "", "/abs", "a/../b", "..", "../x", "a/./b", ".", "a//b", "a\\b", "a/", "C:x", "c:/x",
            "a\0b",
        ] {
            assert!(RelPath::new(bad).is_err(), "{bad:?} must be refused");
        }
        for good in [
            "a",
            "a/b",
            "skills/omm-x/SKILL.md",
            "..a",
            "a..",
            ".hidden",
            "a b/c",
        ] {
            let p = RelPath::new(good).unwrap();
            assert_eq!(p.as_str(), good);
        }
        assert_eq!(RelPath::new("a/b/c").unwrap().depth(), 3);
        assert_eq!(
            RelPath::new("a/b").unwrap().under(Path::new("/r")),
            PathBuf::from("/r/a/b")
        );
        // Deserialize goes through the same grammar.
        let e: std::result::Result<Entry, _> = serde_json::from_value(json!({
            "base": "muse-config", "path": "/etc/passwd", "kind": "skill", "sha256": "x",
            "source_version": "0", "writer": "w", "mechanism": "copy", "class": "exclusive"
        }));
        assert!(e.is_err());
    }

    #[test]
    fn spellings_round_trip() {
        assert_eq!(
            serde_json::to_value(Base::MuseConfig).unwrap(),
            "muse-config"
        );
        assert_eq!(
            serde_json::to_value(Kind::SettingsKey).unwrap(),
            "settings-key"
        );
        assert_eq!(
            serde_json::to_value(Mechanism::MuseSkillsInstall).unwrap(),
            "muse-skills-install"
        );
        assert_eq!(
            serde_json::to_value(Class::SharedKey).unwrap(),
            "shared-key"
        );
        let r: Registration = serde_json::from_value(json!({
            "kind": "muse-plugin", "id": "omm", "package_sha256": "d", "generation_path": "/g",
            "approved": ["plugin:omm:hook:x"]
        }))
        .unwrap();
        assert!(matches!(r, Registration::MusePlugin { .. }));
        let r: Registration =
            serde_json::from_value(json!({"kind": "trust", "project": "/ws"})).unwrap();
        assert_eq!(
            r,
            Registration::Trust {
                project: "/ws".into(),
                prior: None,
                value: None,
            }
        );
        // A settings key written before `value` was recorded reads back with
        // `value: None` and serializes without the field (byte-stable).
        let r: Registration = serde_json::from_value(
            json!({"kind": "settings-key", "path": "tui.theme", "prior": "old"}),
        )
        .unwrap();
        assert_eq!(
            r,
            Registration::SettingsKey {
                path: "tui.theme".into(),
                prior: Some(json!("old")),
                value: None,
                profile: None,
            }
        );
        assert_eq!(
            serde_json::to_value(&r).unwrap(),
            json!({"kind": "settings-key", "path": "tui.theme", "prior": "old"})
        );
        // A profile tag round-trips and is omitted when absent.
        let tagged = Registration::SettingsKey {
            path: "permissions.default_profile".into(),
            prior: None,
            value: Some(json!("omm-strict")),
            profile: Some("strict".into()),
        };
        let v = serde_json::to_value(&tagged).unwrap();
        assert_eq!(v["profile"], "strict");
        assert_eq!(serde_json::from_value::<Registration>(v).unwrap(), tagged);
        // Unknown fields are corruption, not noise.
        assert!(serde_json::from_value::<Registration>(
            json!({"kind": "trust", "project": "/ws", "extra": 1})
        )
        .is_err());
        assert!(serde_json::from_value::<Ledger>(json!({
            "schema_version": 1, "omm_version": "0", "host": {"version": "v", "sha256": "s"},
            "scope": "user", "entries": [], "registrations": [], "bogus": true
        }))
        .is_err());
    }

    #[test]
    fn entries_sorted_by_base_then_path_and_bytes_are_stable() {
        let mut l = ledger();
        l.entries.push(entry(Base::Workspace, "a", "1"));
        l.entries.push(entry(Base::MuseConfig, "z", "2"));
        l.entries.push(entry(Base::MuseConfig, "a/b", "3"));
        l.entries.push(entry(Base::Omm, "m", "4"));
        l.entries.push(entry(Base::MuseData, "m", "5"));
        assert!(!l.is_sorted());
        let bytes1 = l.to_bytes().unwrap();
        l.sort();
        assert!(l.is_sorted());
        let keys: Vec<String> = l.entries.iter().map(|e| e.key().to_string()).collect();
        assert_eq!(
            keys,
            vec![
                "muse-config:a/b",
                "muse-config:z",
                "muse-data:m",
                "omm:m",
                "workspace:a"
            ]
        );
        let bytes2 = l.to_bytes().unwrap();
        assert_eq!(bytes1, bytes2, "to_bytes sorts regardless of memory order");
        assert!(bytes2.ends_with(b"\n"));
        let back = Ledger::from_bytes(&bytes2, Path::new("x")).unwrap();
        assert_eq!(back, l);
        // upsert keeps order; a replaced entry comes back.
        assert!(l.upsert(entry(Base::MuseConfig, "b", "6")).is_none());
        assert!(l.is_sorted());
        let old = l.upsert(entry(Base::MuseConfig, "b", "7")).unwrap();
        assert_eq!(old.sha256, "6");
        assert_eq!(
            l.find(Base::MuseConfig, &RelPath::new("b").unwrap())
                .unwrap()
                .sha256,
            "7"
        );
        assert!(l
            .remove(Base::MuseConfig, &RelPath::new("b").unwrap())
            .is_some());
        assert!(l
            .remove(Base::MuseConfig, &RelPath::new("b").unwrap())
            .is_none());
    }

    #[test]
    fn validate_refuses_duplicates_and_wrong_schema() {
        let mut l = ledger();
        l.entries.push(entry(Base::Omm, "a", "1"));
        l.entries.push(entry(Base::Omm, "a", "2"));
        assert!(l.validate().unwrap_err().contains("duplicate entry"));
        l.entries.pop();
        assert!(l.validate().is_ok());
        l.schema_version = 2;
        assert!(l.validate().unwrap_err().contains("schema_version"));
        l.schema_version = 1;
        let reg = Registration::MuseMarketplace {
            name: "ohmy".into(),
            source: "/src".into(),
        };
        assert!(l.register(reg.clone()));
        assert!(!l.register(reg.clone()));
        l.registrations.push(reg);
        assert!(l.validate().unwrap_err().contains("duplicate registration"));
    }

    #[test]
    fn settings_and_trust_registrations_keep_the_first_prior_and_the_last_value() {
        let mut l = ledger();
        assert!(l.record_settings_key(
            "tui.theme",
            Some(json!("ayu")),
            Some(json!("custom:a")),
            None
        ));
        assert!(!l.record_settings_key(
            "tui.theme",
            Some(json!("custom:a")),
            Some(json!("custom:b")),
            None
        ));
        assert_eq!(
            l.settings_key("tui.theme"),
            Some(&Registration::SettingsKey {
                path: "tui.theme".into(),
                prior: Some(json!("ayu")),
                value: Some(json!("custom:b")),
                profile: None,
            })
        );
        assert_eq!(l.registrations.len(), 1);
        assert!(l.forget_settings_key("tui.theme"));
        assert!(!l.forget_settings_key("tui.theme"));
        assert!(l.registrations.is_empty());
        assert!(l.record_trust("/ws", None, Some(json!({"decision": "trusted"}))));
        assert!(!l.record_trust(
            "/ws",
            Some(json!("x")),
            Some(json!({"decision": "trusted", "extra": 1}))
        ));
        assert_eq!(
            l.registrations,
            vec![Registration::Trust {
                project: "/ws".into(),
                prior: None,
                value: Some(json!({"decision": "trusted", "extra": 1})),
            }]
        );
    }

    #[test]
    fn non_file_mechanisms() {
        assert!(Mechanism::Copy.is_file());
        assert!(Mechanism::MuseSkillsInstall.is_file());
        assert!(Mechanism::MusePluginsInstall.is_file());
        assert!(!Mechanism::SettingsPatch.is_file());
        assert!(!Mechanism::TrustMerge.is_file());
        assert_eq!(Base::ALL.len(), 4);
        assert!(Base::MuseConfig < Base::MuseData && Base::Omm < Base::Workspace);
    }
}
