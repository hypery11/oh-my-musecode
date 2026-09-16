//! §5.4 / R7 overlay resolution — first-hit-wins per id, published as a table:
//!
//! ```text
//! 1  $OMM/custom/<kind>/<id>            (user overlay — REPLACE)
//! 2  bundled content
//! +  $OMM/custom/<kind>/<id>_append.md  (APPEND, resolved independently)
//! −  config.json → "disabled": ["skill:omm-pdf", "hook:omm-verify"]   (capability-qualified ids)
//! ```
//!
//! `$OMM` is `Roots::omm_root()` (`$XDG_CONFIG_HOME/omm`, else `~/.config/omm`,
//! ARCHITECTURE.md §2). `<kind>` is the plural directory of
//! [`AssetKind`]; a skill overlay is the directory `custom/skills/<id>/`
//! (with its own `SKILL.md`), every other kind a single file with the kind's
//! extension. Shadowed providers are retained on the resolved item as
//! `_shadowed` so `omm list --json` can explain what hides what. A custom
//! entry with no bundled counterpart is a user addition and resolves too.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::catalog::{AssetKind, Catalog};
use crate::content::Content;
use crate::error::{ManifestError, Result};
use crate::lint::Finding;

/// `$OMM/custom/` (ARCHITECTURE.md §2).
pub const CUSTOM_DIR: &str = "custom";
/// `$OMM/config.json` (ARCHITECTURE.md §2).
pub const CONFIG_FILE: &str = "config.json";
/// The `config.json` key holding capability-qualified disabled ids.
pub const CONFIG_KEY_DISABLED: &str = "disabled";
/// The suffix of an append overlay (`<id>_append.md`).
pub const APPEND_SUFFIX: &str = "_append.md";

/// Who provided a resolved item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// `content/` as shipped in the package.
    Bundled,
    /// `$OMM/custom/`.
    Custom,
}

/// Where an item (or a shadowed candidate) came from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Provenance {
    pub provider: Provider,
    pub path: PathBuf,
    /// `user` for the overlay, `bundled` for content.
    pub level: &'static str,
}

/// One resolved asset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ResolvedItem {
    pub id: String,
    pub kind: AssetKind,
    /// `<kind>:<id>`.
    pub qualified_id: String,
    pub provider: Provider,
    pub path: PathBuf,
    pub level: &'static str,
    /// Providers this item hides, highest precedence first.
    #[serde(rename = "_shadowed")]
    pub shadowed: Vec<Provenance>,
    /// `<id>_append.md`, when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub append: Option<PathBuf>,
    /// Listed in `config.json → disabled`.
    pub disabled: bool,
}

/// The resolution table.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Resolution {
    pub items: Vec<ResolvedItem>,
    /// `disabled` entries that name nothing.
    pub unknown_disabled: Vec<String>,
    /// Overlay entries that could not be used (escaping symlinks, bad names).
    pub problems: Vec<Finding>,
}

impl Resolution {
    /// Items not disabled.
    pub fn active(&self) -> impl Iterator<Item = &ResolvedItem> {
        self.items.iter().filter(|i| !i.disabled)
    }
    /// Look an item up by id.
    pub fn get(&self, id: &str) -> Option<&ResolvedItem> {
        self.items.iter().find(|i| i.id == id)
    }
    /// Items of one kind.
    pub fn of_kind(&self, kind: AssetKind) -> impl Iterator<Item = &ResolvedItem> {
        self.items.iter().filter(move |i| i.kind == kind)
    }
}

/// `$OMM/config.json` — the part the overlay reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OverlayConfig {
    /// Capability-qualified ids (`skill:omm-pdf`).
    pub disabled: Vec<String>,
}

/// Read `$OMM/config.json` (absent → defaults). `disabled` must be an array
/// of `<kind>:<id>` strings; anything else is an error, never a guess.
pub fn load_config(omm_root: &Path) -> Result<OverlayConfig> {
    let path = omm_root.join(CONFIG_FILE);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(OverlayConfig::default()),
        Err(e) => return Err(ManifestError::io("read", &path, e)),
    };
    let doc: Value = serde_json::from_slice(&bytes).map_err(|e| ManifestError::Json {
        path: path.clone(),
        source: e,
    })?;
    let mut disabled = Vec::new();
    match doc.get(CONFIG_KEY_DISABLED) {
        None | Some(Value::Null) => {}
        Some(Value::Array(items)) => {
            for item in items {
                let Some(s) = item.as_str() else {
                    return Err(ManifestError::Overlay {
                        path,
                        detail: format!("`disabled` entry {item} is not a string"),
                    });
                };
                match s.split_once(':') {
                    Some((kind, id)) if AssetKind::from_singular(kind).is_some() && !id.is_empty() => {
                        disabled.push(s.to_string())
                    }
                    _ => {
                        return Err(ManifestError::Overlay {
                            path,
                            detail: format!(
                                "`disabled` entry {s:?} is not capability-qualified (`<kind>:<id>`, kind one of {})",
                                AssetKind::ALL.iter().map(|k| k.singular()).collect::<Vec<_>>().join(" ")
                            ),
                        })
                    }
                }
            }
        }
        Some(other) => {
            return Err(ManifestError::Overlay {
                path,
                detail: format!("`disabled` must be an array, got {other}"),
            })
        }
    }
    Ok(OverlayConfig { disabled })
}

/// The file extension an overlay file of `kind` carries.
pub fn overlay_extension(kind: AssetKind) -> &'static str {
    match kind {
        AssetKind::Skill => "",
        AssetKind::Command | AssetKind::Agent | AssetKind::Translation => "md",
        AssetKind::Hook | AssetKind::Reminder | AssetKind::McpServer | AssetKind::Profile => "json",
        AssetKind::Theme => "tmTheme",
        AssetKind::Rules => "tmpl",
    }
}

/// `$OMM/custom/<kind>/<id>[.<ext>]` — for a skill, its directory.
pub fn custom_path(omm_root: &Path, kind: AssetKind, id: &str) -> PathBuf {
    let dir = omm_root.join(CUSTOM_DIR).join(kind.plural());
    match overlay_extension(kind) {
        "" => dir.join(id),
        ext => dir.join(format!("{id}.{ext}")),
    }
}

/// `$OMM/custom/<kind>/<id>_append.md`.
pub fn append_path(omm_root: &Path, kind: AssetKind, id: &str) -> PathBuf {
    omm_root
        .join(CUSTOM_DIR)
        .join(kind.plural())
        .join(format!("{id}{APPEND_SUFFIX}"))
}

/// Resolve every bundled asset and every custom entry under `omm_root`.
pub fn resolve(catalog: &Catalog, content: &Content, omm_root: &Path) -> Result<Resolution> {
    let config = load_config(omm_root)?;
    let disabled: BTreeSet<&str> = config.disabled.iter().map(String::as_str).collect();
    let mut res = Resolution::default();
    let custom_root = omm_root.join(CUSTOM_DIR);
    let custom_canonical = if custom_root.is_dir() {
        Some(omm_host::fsx::canonicalize(&custom_root)?)
    } else {
        None
    };

    let mut seen: BTreeSet<(AssetKind, String)> = BTreeSet::new();
    for (asset, file) in content.assets() {
        let kind = asset.kind;
        let bundled = Provenance {
            provider: Provider::Bundled,
            path: if kind == AssetKind::Skill {
                content.root.join(asset.dir())
            } else {
                file.abs.clone()
            },
            level: "bundled",
        };
        let qualified = asset.qualified_id();
        let custom = usable_custom(
            &custom_canonical,
            custom_path(omm_root, kind, &asset.id),
            kind,
            &mut res.problems,
        );
        let (provider, path, level, shadowed) = match custom {
            Some(p) => (Provider::Custom, p, "user", vec![bundled]),
            None => (
                Provider::Bundled,
                bundled.path.clone(),
                bundled.level,
                Vec::new(),
            ),
        };
        let append = if kind.supports_append() {
            usable_custom_file(
                &custom_canonical,
                append_path(omm_root, kind, &asset.id),
                &mut res.problems,
            )
        } else {
            None
        };
        seen.insert((kind, asset.id.clone()));
        res.items.push(ResolvedItem {
            id: asset.id.clone(),
            kind,
            disabled: disabled.contains(qualified.as_str()),
            qualified_id: qualified,
            provider,
            path,
            level,
            shadowed,
            append,
        });
    }
    // Non-shipping catalog rows (aliases) forward to their canonical item.
    for a in &catalog.assets {
        if a.lifecycle.ships() {
            continue;
        }
        if let Some(target) = catalog.resolve(&a.id)? {
            if let Some(item) = res.items.iter().find(|i| i.id == target.id).cloned() {
                let mut alias = item;
                alias.id = a.id.clone();
                alias.qualified_id = a.qualified_id();
                alias.disabled = disabled.contains(a.qualified_id().as_str()) || alias.disabled;
                seen.insert((a.kind, a.id.clone()));
                res.items.push(alias);
            }
        }
    }
    // User additions: custom entries with no bundled counterpart.
    if let Some(root) = &custom_canonical {
        for kind in AssetKind::ALL {
            let dir = root.join(kind.plural());
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut names: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
            names.sort();
            for path in names {
                let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if name.ends_with(APPEND_SUFFIX) {
                    continue;
                }
                let id = match overlay_extension(kind) {
                    "" => name.to_string(),
                    ext => match name.strip_suffix(&format!(".{ext}")) {
                        Some(stem) => stem.to_string(),
                        None => continue,
                    },
                };
                if seen.contains(&(kind, id.clone())) {
                    continue;
                }
                let Some(p) =
                    usable_custom(&custom_canonical, path.clone(), kind, &mut res.problems)
                else {
                    continue;
                };
                let qualified = format!("{}:{id}", kind.singular());
                let append = if kind.supports_append() {
                    usable_custom_file(
                        &custom_canonical,
                        append_path(omm_root, kind, &id),
                        &mut res.problems,
                    )
                } else {
                    None
                };
                res.items.push(ResolvedItem {
                    id: id.clone(),
                    kind,
                    disabled: disabled.contains(qualified.as_str()),
                    qualified_id: qualified,
                    provider: Provider::Custom,
                    path: p,
                    level: "user",
                    shadowed: Vec::new(),
                    append,
                });
                seen.insert((kind, id));
            }
        }
    }
    let known: BTreeSet<&str> = res.items.iter().map(|i| i.qualified_id.as_str()).collect();
    res.unknown_disabled = config
        .disabled
        .iter()
        .filter(|d| !known.contains(d.as_str()))
        .cloned()
        .collect();
    Ok(res)
}

/// A custom entry that exists, is the right shape for its kind (a directory
/// holding `SKILL.md` for a skill, a regular file otherwise) and resolves
/// inside `$OMM/custom/` (a symlink pointing elsewhere is reported and skipped).
fn usable_custom(
    custom_root: &Option<PathBuf>,
    path: PathBuf,
    kind: AssetKind,
    problems: &mut Vec<Finding>,
) -> Option<PathBuf> {
    let root = custom_root.as_ref()?;
    if kind == AssetKind::Skill {
        let skill_md = path.join(crate::content::SKILL_FILE);
        if !skill_md.is_file() {
            return None;
        }
        return match crate::contained(root, &path) {
            Ok(canonical) if canonical.is_dir() => Some(canonical),
            Ok(_) => None,
            Err(e) => {
                problems.push(Finding::error(
                    "overlay-containment",
                    path,
                    e.to_string(),
                    "keep overlay entries inside $OMM/custom (no symlinks out of it)",
                ));
                None
            }
        };
    }
    usable_custom_file(custom_root, path, problems)
}

fn usable_custom_file(
    custom_root: &Option<PathBuf>,
    path: PathBuf,
    problems: &mut Vec<Finding>,
) -> Option<PathBuf> {
    let root = custom_root.as_ref()?;
    if !path.is_file() {
        return None;
    }
    match crate::contained(root, &path) {
        Ok(canonical) => Some(canonical),
        Err(e) => {
            problems.push(Finding::error(
                "overlay-containment",
                path,
                e.to_string(),
                "keep overlay entries inside $OMM/custom (no symlinks out of it)",
            ));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_and_config_parse() {
        let root = Path::new("/x/omm");
        assert_eq!(
            custom_path(root, AssetKind::Skill, "omm-a"),
            PathBuf::from("/x/omm/custom/skills/omm-a")
        );
        assert_eq!(
            custom_path(root, AssetKind::Command, "omm-a"),
            PathBuf::from("/x/omm/custom/commands/omm-a.md")
        );
        assert_eq!(
            custom_path(root, AssetKind::Hook, "omm-a"),
            PathBuf::from("/x/omm/custom/hooks/omm-a.json")
        );
        assert_eq!(
            custom_path(root, AssetKind::Theme, "omm-a"),
            PathBuf::from("/x/omm/custom/themes/omm-a.tmTheme")
        );
        assert_eq!(
            append_path(root, AssetKind::Skill, "omm-a"),
            PathBuf::from("/x/omm/custom/skills/omm-a_append.md")
        );
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_config(dir.path()).unwrap(), OverlayConfig::default());
        std::fs::write(
            dir.path().join("config.json"),
            b"{\"profile\":\"fast\",\"disabled\":[\"skill:omm-a\",\"hook:omm-b\"]}",
        )
        .unwrap();
        assert_eq!(
            load_config(dir.path()).unwrap().disabled,
            vec!["skill:omm-a", "hook:omm-b"]
        );
        std::fs::write(
            dir.path().join("config.json"),
            b"{\"disabled\":[\"omm-a\"]}",
        )
        .unwrap();
        assert!(
            matches!(load_config(dir.path()), Err(ManifestError::Overlay { .. })),
            "unqualified id"
        );
        std::fs::write(
            dir.path().join("config.json"),
            b"{\"disabled\":\"skill:omm-a\"}",
        )
        .unwrap();
        assert!(load_config(dir.path()).is_err());
        std::fs::write(
            dir.path().join("config.json"),
            b"{\"disabled\":[\"widget:omm-a\"]}",
        )
        .unwrap();
        assert!(load_config(dir.path()).is_err());
        std::fs::write(dir.path().join("config.json"), b"not json").unwrap();
        assert!(matches!(
            load_config(dir.path()),
            Err(ManifestError::Json { .. })
        ));
    }
}
