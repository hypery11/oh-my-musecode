//! `dist/codex/.codex-plugin/plugin.json` — the Codex-schema projection
//! (ARCHITECTURE.md §5.2; `research/musecode/plugins.md` §5).
//!
//! That schema imports only skills and MCP servers (`commands` and `hooks`
//! "are not imported in this phase", plugins.md §5.2). `skills` is a single
//! string path to a directory of skill directories; `mcpServers` is a path to
//! a conventional `.mcp.json` (`{"mcpServers":{"<id>":{"type":"stdio",
//! "command":…,"args":[…]}}}`, no `$schema` needed); `interface.displayName`
//! is honoured. `author`/`repository`/`license` are known but never surfaced,
//! so they are not emitted.

use serde_json::{json, Map, Value};

use crate::catalog::{AssetKind, Catalog};
use crate::content::Content;
use crate::error::Result;
use crate::generate::native::PLUGIN_DISPLAY_NAME;
use crate::generate::Package;

/// The manifest directory of that family (`research/musecode/plugins.md` §0).
pub const MANIFEST_DIR: &str = ".codex-plugin";
/// The conventional MCP document name that schema reads (plugins.md §5.1).
pub const MCP_FILE: &str = ".mcp.json";

/// `.codex-plugin/plugin.json`.
pub fn manifest_path() -> String {
    format!("{MANIFEST_DIR}/plugin.json")
}

/// Render the projection.
pub fn render(
    catalog: &Catalog,
    content: &Content,
    version: &str,
    description: &str,
) -> Result<Package> {
    let mut pkg = Package::new();
    for skill in &content.skills {
        for f in &skill.files {
            if f.rel == skill.asset.path {
                // Strict-YAML front matter for a foreign parser (see
                // `FrontMatter::to_yaml_safe_bytes`); the body is verbatim.
                pkg.insert(f.rel.clone(), skill.frontmatter.to_yaml_safe_bytes())?;
            } else {
                pkg.copy_file(f.rel.clone(), f)?;
            }
        }
    }
    let manifest = json!({
        "name": catalog.plugin_id,
        "version": version,
        "description": description,
        "interface": { "displayName": PLUGIN_DISPLAY_NAME },
        "skills": AssetKind::Skill.plural(),
        "mcpServers": MCP_FILE,
    });
    pkg.insert_json(MCP_FILE, &mcp_document(content))?;
    pkg.insert_json(manifest_path(), &manifest)?;
    Ok(pkg)
}

/// The `.mcp.json` document (`{"mcpServers":{…}}`): one entry per shipped
/// server (`content/mcp/<id>.json`), a stdio one as `command` + `args`.
pub fn mcp_document(content: &Content) -> Value {
    let mut servers = Map::new();
    for m in &content.mcp_servers {
        let mut entry = Map::new();
        entry.insert("type".into(), Value::String(m.transport.clone()));
        if m.transport == "http" {
            if let Some(url) = m.raw.get("url") {
                entry.insert("url".into(), url.clone());
            }
        } else if let Some(Value::Array(argv)) = m.raw.get("command") {
            if let Some(first) = argv.first() {
                entry.insert("command".into(), first.clone());
            }
            entry.insert(
                "args".into(),
                Value::Array(argv.iter().skip(1).cloned().collect()),
            );
        }
        servers.insert(m.id.clone(), Value::Object(entry));
    }
    json!({ "mcpServers": servers })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_carries_skills_dir_and_mcp_path_only() {
        let repo = crate::Repo::from_cargo_manifest_dir().unwrap();
        let catalog = Catalog::load(&repo.catalog_path()).unwrap();
        let content = Content::load(&catalog, &repo.content_dir()).unwrap();
        let pkg = render(&catalog, &content, "1.0.0", "d").unwrap();
        let m = pkg.get_json(&manifest_path()).unwrap();
        assert_eq!(m["skills"], "skills");
        assert_eq!(m["mcpServers"], ".mcp.json");
        assert!(m.get("commands").is_none() && m.get("hooks").is_none());
        // One entry per shipped server, in the conventional shape
        // (`{"type":"stdio","command":<argv[0]>,"args":<argv[1..]>}`).
        let servers = pkg.get_json(MCP_FILE).unwrap()["mcpServers"].clone();
        let servers = servers.as_object().unwrap();
        assert_eq!(servers.len(), content.mcp_servers.len());
        for s in &content.mcp_servers {
            let entry = &servers[&s.id];
            assert_eq!(entry["type"], s.transport);
            if let Some(Value::Array(argv)) = s.raw.get("command") {
                assert_eq!(entry["command"], argv[0]);
                assert_eq!(entry["args"], Value::Array(argv[1..].to_vec()));
            }
        }
        assert!(pkg
            .paths()
            .iter()
            .all(|p| p.starts_with("skills/") || *p == MCP_FILE || *p == manifest_path()));
    }
}
