//! `plugins/<pid>/.muse-plugin/plugin.json` — the NATIVE package
//! (ARCHITECTURE.md §5.2; `research/experiments/plugin-contract.md` §1;
//! `.host/contract/create-plugin/references/native-plugin-contract.md`).
//!
//! Top-level key set is EXACTLY `schemaVersion name displayName version
//! description compat capabilities` (unknown keys are warnings and warnings
//! are failures — plugin-contract.md §1.2). `capabilities` carries the five
//! families in the manifest spelling (`hr::CAPABILITY_FAMILIES_MANIFEST`),
//! always all five, empty arrays where nothing ships, so the family set is
//! visible at a glance:
//!
//! * `skills[]` / `commands[]`: `{id, path, enabledDefault}` — `enabledDefault`
//!   a real JSON boolean (only a literal `false` disables; §1.6);
//! * `hooks[]`: the CLOSED entry from `content/hooks/<id>.json` verbatim —
//!   `command` only, the argv `["omm","hook","<name>"]` (R16); the loader
//!   already refused `commandWindows` and every other unknown field;
//! * `mcpServers[]`: verbatim, with `env`/`cwd`/`headers` refused by the
//!   loader because the family is OPEN and drops them silently (§1.9);
//! * `reminders[]`: the declaration verbatim (its `decision` block is the
//!   host's to validate), `enabledDefault` filled in from the catalog when
//!   the file omits it.
//!
//! Copied content: every skill directory (SKILL.md + `references/*`), each
//! command file, each reminder duty file. Themes, profiles, rules and the
//! translation block are not plugin capabilities and stay out; agents are not
//! packaged in Phase 1 (the native family auto-discovers `agents/`,
//! `research/musecode/plugins.md` §3.10, and nothing ships one yet).

use serde_json::{json, Map, Value};

use crate::catalog::Catalog;
use crate::content::Content;
use crate::error::Result;
use crate::generate::Package;
use crate::hr;

/// `displayName` of the native manifest — the product name, optional for the
/// host, shown by `plugins list`.
pub const PLUGIN_DISPLAY_NAME: &str = "oh-my-musecode";
/// `compat.source` — not validated by the host (plugin-contract.md D2) but
/// what the bundled contract writes.
pub const COMPAT_SOURCE: &str = "native";
/// `.muse-plugin/plugin.json`.
pub fn manifest_path() -> String {
    format!("{}/plugin.json", hr::MANIFEST_DIR_NATIVE)
}

/// Render the native package.
pub fn render(
    catalog: &Catalog,
    content: &Content,
    version: &str,
    description: &str,
) -> Result<Package> {
    let mut pkg = Package::new();
    let mut skills = Vec::new();
    for skill in &content.skills {
        skills.push(json!({
            "id": skill.asset.id,
            "path": skill.asset.path,
            "enabledDefault": skill.asset.enabled_default(),
        }));
        for f in &skill.files {
            pkg.copy_file(f.rel.clone(), f)?;
        }
    }
    let mut commands = Vec::new();
    for cmd in &content.commands {
        commands.push(json!({
            "id": cmd.asset.id,
            "path": cmd.asset.path,
            "enabledDefault": cmd.asset.enabled_default(),
        }));
        pkg.copy_file(cmd.asset.path.clone(), &cmd.file)?;
    }
    let hooks: Vec<Value> = content
        .hooks
        .iter()
        .map(|h| Value::Object(h.raw.clone()))
        .collect();
    let mcp_servers: Vec<Value> = content
        .mcp_servers
        .iter()
        .map(|m| Value::Object(m.raw.clone()))
        .collect();
    let mut reminders = Vec::new();
    for r in &content.reminders {
        let mut entry: Map<String, Value> = r.raw.clone();
        entry
            .entry("enabledDefault")
            .or_insert(Value::Bool(r.enabled_default));
        reminders.push(Value::Object(entry));
        pkg.copy_file(r.path.clone(), &r.duty)?;
    }
    let manifest = json!({
        "schemaVersion": hr::PLUGIN_MANIFEST_SCHEMA_VERSION,
        "name": catalog.plugin_id,
        "displayName": PLUGIN_DISPLAY_NAME,
        "version": version,
        "description": description,
        "compat": {
            "source": COMPAT_SOURCE,
            "manifestDir": hr::MANIFEST_DIR_NATIVE,
        },
        "capabilities": {
            hr::CAPABILITY_FAMILIES_MANIFEST[0]: skills,
            hr::CAPABILITY_FAMILIES_MANIFEST[1]: commands,
            hr::CAPABILITY_FAMILIES_MANIFEST[2]: hooks,
            hr::CAPABILITY_FAMILIES_MANIFEST[3]: mcp_servers,
            hr::CAPABILITY_FAMILIES_MANIFEST[4]: reminders,
        },
    });
    pkg.insert_json(manifest_path(), &manifest)?;
    Ok(pkg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_has_exactly_the_native_key_set_and_all_five_families() {
        let repo = crate::Repo::from_cargo_manifest_dir().unwrap();
        let catalog = Catalog::load(&repo.catalog_path()).unwrap();
        let content = Content::load(&catalog, &repo.content_dir()).unwrap();
        let pkg = render(&catalog, &content, "9.9.9", "desc").unwrap();
        let m = pkg.get_json(&manifest_path()).unwrap();
        let keys: Vec<&str> = m.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "schemaVersion",
                "name",
                "displayName",
                "version",
                "description",
                "compat",
                "capabilities"
            ]
        );
        assert_eq!(m["schemaVersion"], 1);
        assert_eq!(m["name"], "oh-my-musecode");
        assert_eq!(m["version"], "9.9.9");
        assert_eq!(m["compat"]["manifestDir"], ".muse-plugin");
        let caps = m["capabilities"].as_object().unwrap();
        let families: Vec<&str> = caps.keys().map(String::as_str).collect();
        assert_eq!(families, hr::CAPABILITY_FAMILIES_MANIFEST.to_vec());
        assert!(caps["skills"][0]["enabledDefault"].is_boolean());
        assert!(caps["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h.get("commandWindows").is_none()));
        assert!(caps["hooks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|h| h["command"][0] == "omm"));
        assert!(caps["reminders"][0]["enabledDefault"].is_boolean());
        // Only capability content is packaged.
        assert!(
            pkg.paths().iter().all(|p| {
                p.starts_with("skills/")
                    || p.starts_with("commands/")
                    || p.starts_with("reminders/")
                    || *p == manifest_path()
            }),
            "{:?}",
            pkg.paths()
        );
        assert!(!pkg
            .paths()
            .iter()
            .any(|p| p.starts_with("themes/") || p.starts_with("profiles/")));
        assert!(pkg.get("hooks/README.md").is_none());
    }
}
