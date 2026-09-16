//! `content/catalog.json` (ARCHITECTURE.md §5.1) — THE asset list.
//!
//! ```jsonc
//! { "schema_version": 1, "plugin_id": "omm",
//!   "assets": [ { "id": "omm-plan", "kind": "skill", "path": "skills/omm-plan/SKILL.md",
//!                 "lifecycle": "active", "core": true, "canonical": null,
//!                 "since": "0.1.0", "sunset": null, "budget_bytes": 240 } ] }
//! ```
//!
//! R8: nothing in Rust knows an asset name — only the kinds and lifecycles are
//! typed here. `lifecycle ∈ active | alias | merged | deprecated | internal`;
//! `canonical` forwards an alias or a merged id; `core` items cannot be
//! deactivated (the build gate: a core asset that is not `active` is an error).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{ManifestError, Result};

/// `schema_version` this crate reads.
pub const CATALOG_SCHEMA_VERSION: u64 = 1;

/// What an asset is. The plural is its directory under `content/` and under
/// `$OMM/custom/`; the singular is the capability-qualified prefix in
/// `config.json → disabled` (`skill:omm-pdf`) and in the host's own stable ids
/// (`plugin:<pid>:<kind>:<cap-id>`, reserved-ids.json `capability_stable_ids`).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Skill,
    Command,
    Hook,
    Reminder,
    McpServer,
    Agent,
    Theme,
    Profile,
    Rules,
    Translation,
}

impl AssetKind {
    /// Every kind, in the order the generators and lists use.
    pub const ALL: [AssetKind; 10] = [
        AssetKind::Skill,
        AssetKind::Command,
        AssetKind::Hook,
        AssetKind::Reminder,
        AssetKind::McpServer,
        AssetKind::Agent,
        AssetKind::Theme,
        AssetKind::Profile,
        AssetKind::Rules,
        AssetKind::Translation,
    ];

    /// The capability-qualified prefix (`skill`, `mcp_server`, …).
    pub fn singular(self) -> &'static str {
        match self {
            AssetKind::Skill => "skill",
            AssetKind::Command => "command",
            AssetKind::Hook => "hook",
            AssetKind::Reminder => "reminder",
            AssetKind::McpServer => "mcp_server",
            AssetKind::Agent => "agent",
            AssetKind::Theme => "theme",
            AssetKind::Profile => "profile",
            AssetKind::Rules => "rules",
            AssetKind::Translation => "translation",
        }
    }

    /// The directory name under `content/` and `$OMM/custom/`.
    pub fn plural(self) -> &'static str {
        match self {
            AssetKind::Skill => "skills",
            AssetKind::Command => "commands",
            AssetKind::Hook => "hooks",
            AssetKind::Reminder => "reminders",
            AssetKind::McpServer => "mcp",
            AssetKind::Agent => "agents",
            AssetKind::Theme => "themes",
            AssetKind::Profile => "profiles",
            AssetKind::Rules => "rules",
            AssetKind::Translation => "translation",
        }
    }

    /// Parse the singular form.
    pub fn from_singular(s: &str) -> Option<AssetKind> {
        AssetKind::ALL.into_iter().find(|k| k.singular() == s)
    }

    /// True for the kinds that become native plugin capabilities
    /// (`hr::CAPABILITY_FAMILIES_MANIFEST`); themes, profiles, rules and the
    /// translation block are installed by omm itself, never packaged.
    pub fn is_plugin_capability(self) -> bool {
        matches!(
            self,
            AssetKind::Skill
                | AssetKind::Command
                | AssetKind::Hook
                | AssetKind::McpServer
                | AssetKind::Reminder
        )
    }

    /// Kinds whose overlay entry is a Markdown document that `<id>_append.md`
    /// can extend (§5.4).
    pub fn supports_append(self) -> bool {
        matches!(
            self,
            AssetKind::Skill | AssetKind::Command | AssetKind::Agent | AssetKind::Rules
        )
    }
}

/// `lifecycle` (§5.1).
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    /// Shipped and listed.
    Active,
    /// A renamed id: `canonical` names the asset it forwards to; nothing ships.
    Alias,
    /// Folded into `canonical`; nothing ships under this id.
    Merged,
    /// Still shipped until `sunset`; `canonical` may name a replacement.
    Deprecated,
    /// Shipped, not user-facing (hidden from `omm list` by default).
    Internal,
}

impl Lifecycle {
    /// Whether an asset in this state has files on disk and goes into a package.
    pub fn ships(self) -> bool {
        matches!(
            self,
            Lifecycle::Active | Lifecycle::Deprecated | Lifecycle::Internal
        )
    }
    /// Whether this state must carry a `canonical` forward.
    pub fn requires_canonical(self) -> bool {
        matches!(self, Lifecycle::Alias | Lifecycle::Merged)
    }
    /// Whether this state may carry a `canonical` forward.
    pub fn allows_canonical(self) -> bool {
        matches!(
            self,
            Lifecycle::Alias | Lifecycle::Merged | Lifecycle::Deprecated
        )
    }
}

/// One catalog row. Fields beyond the §5.1 core (`event`, `duty`,
/// `enabled_default`, `budget_bytes_first_sentence`, theme palettes …) are
/// kept in `extra` and passed through untouched.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Asset {
    pub id: String,
    pub kind: AssetKind,
    /// Relative to `content/`, `/`-separated.
    pub path: String,
    pub lifecycle: Lifecycle,
    #[serde(default)]
    pub core: bool,
    #[serde(default)]
    pub canonical: Option<String>,
    pub since: String,
    #[serde(default)]
    pub sunset: Option<String>,
    /// The author's estimate of the asset's catalog cost (bytes); the lint
    /// recomputes it (`budget::estimate`) and reports a stale value.
    #[serde(default)]
    pub budget_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_bytes_first_sentence: Option<u64>,
    /// Hooks: the event (also inside the hook file; cross-checked).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Reminders: the duty file the declaration's `path` points at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duty: Option<String>,
    /// `enabledDefault` for the manifest entry (defaults to `true`, the host's
    /// own default — `research/experiments/plugin-contract.md` §1.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled_default: Option<bool>,
    /// The asset may name foreign tool vocabulary because it translates it
    /// (§5.3 lint `foreign-tool-vocabulary`); the `translation` and `rules`
    /// kinds are exempt by kind. Catalog data, never a path in Rust (R8).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub foreign_vocab: bool,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Asset {
    /// The manifest `enabledDefault` (a real JSON boolean, never a string).
    pub fn enabled_default(&self) -> bool {
        self.enabled_default.unwrap_or(true)
    }
    /// `<kind>:<id>` — the capability-qualified id used by `disabled` (§5.4).
    pub fn qualified_id(&self) -> String {
        format!("{}:{}", self.kind.singular(), self.id)
    }
    /// The parent directory of `path` (a skill's whole directory is the asset).
    pub fn dir(&self) -> &str {
        self.path.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
    }
}

/// The catalog document.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Catalog {
    pub schema_version: u64,
    /// The plugin id (`omm`, reserved-ids.json `omm_identity.plugin_id`).
    pub plugin_id: String,
    /// The author's budget block (`bundle_limit_bytes`, `entry_formula`,
    /// totals); informational, re-derived by `budget::estimate`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<Value>,
    pub assets: Vec<Asset>,
    /// Where it was loaded from (not serialized).
    #[serde(skip)]
    pub path: PathBuf,
}

impl Catalog {
    /// Load and validate `catalog.json` (§5.1 rules; see [`Catalog::validate`]).
    pub fn load(path: &Path) -> Result<Catalog> {
        let bytes = std::fs::read(path).map_err(|e| ManifestError::io("read", path, e))?;
        let mut catalog: Catalog =
            serde_json::from_slice(&bytes).map_err(|e| ManifestError::Json {
                path: path.to_path_buf(),
                source: e,
            })?;
        catalog.path = path.to_path_buf();
        catalog.validate()?;
        Ok(catalog)
    }

    /// Parse from bytes (tests, in-memory fixtures).
    pub fn from_slice(bytes: &[u8], path: &Path) -> Result<Catalog> {
        let mut catalog: Catalog =
            serde_json::from_slice(bytes).map_err(|e| ManifestError::Json {
                path: path.to_path_buf(),
                source: e,
            })?;
        catalog.path = path.to_path_buf();
        catalog.validate()?;
        Ok(catalog)
    }

    fn err(&self, detail: String) -> ManifestError {
        ManifestError::Catalog {
            path: self.path.clone(),
            detail,
        }
    }

    /// The §5.1 invariants: schema version, non-empty plugin id, unique ids,
    /// the core gate, `canonical` forwarding (present exactly when the
    /// lifecycle needs it, resolving to a shipped asset, no cycles), a
    /// relative `/`-separated path with no `..` or backslash.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != CATALOG_SCHEMA_VERSION {
            return Err(self.err(format!(
                "schema_version {} is not {CATALOG_SCHEMA_VERSION}",
                self.schema_version
            )));
        }
        if self.plugin_id.is_empty() {
            return Err(self.err("plugin_id is empty".to_string()));
        }
        let mut seen = BTreeSet::new();
        for a in &self.assets {
            if !seen.insert(a.id.as_str()) {
                return Err(self.err(format!("duplicate asset id `{}`", a.id)));
            }
            if a.core && a.lifecycle != Lifecycle::Active {
                return Err(self.err(format!(
                    "core asset `{}` is `{:?}`; a core asset cannot be deactivated (build gate)",
                    a.id, a.lifecycle
                )));
            }
            if a.lifecycle.requires_canonical() && a.canonical.is_none() {
                return Err(self.err(format!(
                    "`{}` is {:?} but names no `canonical` forward",
                    a.id, a.lifecycle
                )));
            }
            if a.canonical.is_some() && !a.lifecycle.allows_canonical() {
                return Err(self.err(format!(
                    "`{}` is {:?} and must not carry `canonical`",
                    a.id, a.lifecycle
                )));
            }
            if a.path.is_empty()
                || a.path.starts_with('/')
                || a.path.contains('\\')
                || a.path.split('/').any(|seg| seg == ".." || seg.is_empty())
            {
                return Err(self.err(format!(
                    "`{}` path {:?} must be relative, `/`-separated, without `..`",
                    a.id, a.path
                )));
            }
            if a.kind == AssetKind::Reminder && a.lifecycle.ships() && a.duty.is_none() {
                return Err(self.err(format!(
                    "reminder `{}` names no `duty` file (the declaration's `path` points at it)",
                    a.id
                )));
            }
        }
        for a in &self.assets {
            if let Some(target) = &a.canonical {
                let resolved = self.resolve_from(a, self.assets.len() + 1)?;
                if !resolved.lifecycle.ships() {
                    return Err(self.err(format!(
                        "`{}` forwards to `{target}` which does not ship ({:?})",
                        a.id, resolved.lifecycle
                    )));
                }
            }
        }
        Ok(())
    }

    /// Look an id up without forwarding.
    pub fn get(&self, id: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// Follow `canonical` forwards from `id` to the asset that ships (alias
    /// forwarding). `None` for an unknown id; an error on a broken chain.
    pub fn resolve(&self, id: &str) -> Result<Option<&Asset>> {
        match self.get(id) {
            None => Ok(None),
            Some(a) => self.resolve_from(a, self.assets.len() + 1).map(Some),
        }
    }

    fn resolve_from<'a>(&'a self, start: &'a Asset, budget: usize) -> Result<&'a Asset> {
        let mut cur = start;
        let mut hops = 0usize;
        while let Some(target) = &cur.canonical {
            hops += 1;
            if hops > budget {
                return Err(self.err(format!("`canonical` forwarding from `{}` cycles", start.id)));
            }
            cur = self.get(target).ok_or_else(|| {
                self.err(format!("`{}` forwards to unknown asset `{target}`", cur.id))
            })?;
        }
        Ok(cur)
    }

    /// Assets that ship (`Lifecycle::ships`), in catalog order.
    pub fn shipped(&self) -> impl Iterator<Item = &Asset> {
        self.assets.iter().filter(|a| a.lifecycle.ships())
    }

    /// Shipped assets of one kind, in catalog order.
    pub fn shipped_of(&self, kind: AssetKind) -> impl Iterator<Item = &Asset> {
        self.shipped().filter(move |a| a.kind == kind)
    }

    /// Every id, shipped or not.
    pub fn ids(&self) -> Vec<&str> {
        self.assets.iter().map(|a| a.id.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(assets: &str) -> String {
        format!("{{\"schema_version\":1,\"plugin_id\":\"omm\",\"assets\":[{assets}]}}")
    }
    fn asset(id: &str, kind: &str, lifecycle: &str, core: bool, canonical: Option<&str>) -> String {
        let canonical = canonical
            .map(|c| format!("\"{c}\""))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"id\":\"{id}\",\"kind\":\"{kind}\",\"path\":\"{p}/{id}/SKILL.md\",\"lifecycle\":\"{lifecycle}\",\"core\":{core},\"canonical\":{canonical},\"since\":\"0.1.0\",\"sunset\":null,\"budget_bytes\":10}}",
            p = "skills"
        )
    }
    fn parse(assets: &str) -> Result<Catalog> {
        Catalog::from_slice(doc(assets).as_bytes(), Path::new("catalog.json"))
    }

    #[test]
    fn loads_the_real_catalog() {
        let repo = crate::Repo::from_cargo_manifest_dir().unwrap();
        let c = Catalog::load(&repo.catalog_path()).unwrap();
        assert_eq!(c.schema_version, 1);
        assert_eq!(c.plugin_id, "oh-my-musecode");
        assert!(c.shipped_of(AssetKind::Skill).count() >= 10);
        // R19: every id is `omm-` prefixed, except an MCP server id, which
        // the host namespaces itself and which must keep
        // `len(plugin_id) + len(id) ≤ 18` (lint `mcp-id-length`).
        for a in &c.assets {
            if a.kind == AssetKind::McpServer {
                assert!(c.plugin_id.len() + a.id.len() <= 18, "{}", a.id);
            } else {
                assert!(a.id.starts_with("omm-"), "{}", a.id);
            }
        }
        assert!(c.shipped_of(AssetKind::McpServer).count() >= 1);
        // Round-trips without losing the extra fields.
        let json = serde_json::to_value(&c).unwrap();
        assert!(json["assets"][0].get("id").is_some());
    }

    #[test]
    fn core_gate_and_alias_forwarding() {
        assert!(parse(&asset("omm-a", "skill", "active", true, None)).is_ok());
        let e = parse(&asset("omm-a", "skill", "deprecated", true, None)).unwrap_err();
        assert!(e.to_string().contains("core asset"), "{e}");
        let e = parse(&asset("omm-a", "skill", "alias", false, None)).unwrap_err();
        assert!(e.to_string().contains("canonical"), "{e}");
        let two = format!(
            "{},{}",
            asset("omm-a", "skill", "active", false, None),
            asset("omm-old", "skill", "alias", false, Some("omm-a"))
        );
        let c = parse(&two).unwrap();
        assert_eq!(c.resolve("omm-old").unwrap().unwrap().id, "omm-a");
        assert_eq!(c.resolve("omm-a").unwrap().unwrap().id, "omm-a");
        assert!(c.resolve("nope").unwrap().is_none());
        assert_eq!(c.shipped().count(), 1);
        // Forward to an unknown id, a cycle, a non-shipping target, an active with canonical.
        assert!(parse(&asset("omm-old", "skill", "alias", false, Some("zz"))).is_err());
        let cyc = format!(
            "{},{}",
            asset("omm-a", "skill", "alias", false, Some("omm-b")),
            asset("omm-b", "skill", "merged", false, Some("omm-a"))
        );
        assert!(parse(&cyc).unwrap_err().to_string().contains("cycles"));
        let bad = format!(
            "{},{}",
            asset("omm-a", "skill", "active", false, Some("omm-b")),
            asset("omm-b", "skill", "active", false, None)
        );
        assert!(parse(&bad).is_err());
        let dup = format!(
            "{},{}",
            asset("omm-a", "skill", "active", false, None),
            asset("omm-a", "skill", "active", false, None)
        );
        assert!(parse(&dup).unwrap_err().to_string().contains("duplicate"));
    }

    #[test]
    fn schema_kind_and_path_are_checked() {
        let bad_schema = "{\"schema_version\":2,\"plugin_id\":\"omm\",\"assets\":[]}";
        assert!(Catalog::from_slice(bad_schema.as_bytes(), Path::new("c")).is_err());
        assert!(parse(&asset("omm-a", "widget", "active", false, None)).is_err());
        let traversal = "{\"id\":\"omm-a\",\"kind\":\"skill\",\"path\":\"../x\",\"lifecycle\":\"active\",\"since\":\"0.1.0\"}";
        assert!(parse(traversal).is_err());
        let reminder = "{\"id\":\"omm-r\",\"kind\":\"reminder\",\"path\":\"reminders/r.json\",\"lifecycle\":\"active\",\"since\":\"0.1.0\"}";
        assert!(parse(reminder).unwrap_err().to_string().contains("duty"));
        assert_eq!(
            AssetKind::from_singular("mcp_server"),
            Some(AssetKind::McpServer)
        );
        assert_eq!(AssetKind::McpServer.plural(), "mcp");
        assert!(AssetKind::Skill.supports_append() && !AssetKind::Hook.supports_append());
        assert!(!AssetKind::Theme.is_plugin_capability());
    }
}
