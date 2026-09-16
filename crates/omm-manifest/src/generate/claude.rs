//! `dist/claude/.claude-plugin/plugin.json` — the Claude-schema projection
//! (ARCHITECTURE.md §5.2; `research/musecode/plugins.md` §4).
//!
//! A separate package tree (a package holds exactly ONE manifest dir,
//! plugins.md §2.1). The manifest carries `name`, `version`, `description`,
//! `skills` (an array of `./skills/<id>` directory paths), `commands`
//! (an array of `./commands/<id>.md` paths) and `hooks` (a path to
//! `hooks/hooks.json` in that schema's shape:
//! `{"hooks":{"<Event>":[{"hooks":[{"type":"command","command":"<shell string>",
//! "timeout":<seconds>,"statusMessage":…,"async":…}]}]}}` — plugins.md §4.1/§4.2:
//! `timeout` is SECONDS, the `command` is a shell string, the host derives
//! hook ids from the entry content). Presentation-only fields (`author`,
//! `homepage`, `license`, `keywords`) are dropped with a diagnostic by the
//! host (§4.2) and therefore never emitted. Reminders are not a capability
//! of that schema and stay out; `mcpServers` is added when Phase 2 ships one.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};

use crate::catalog::Catalog;
use crate::content::Content;
use crate::error::Result;
use crate::generate::Package;

/// The manifest directory of that family (`research/musecode/plugins.md` §0).
pub const MANIFEST_DIR: &str = ".claude-plugin";
/// Where the hooks document lives inside the projection.
pub const HOOKS_FILE: &str = "hooks/hooks.json";

/// `.claude-plugin/plugin.json`.
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
    let mut manifest = Map::new();
    manifest.insert("name".into(), Value::String(catalog.plugin_id.clone()));
    manifest.insert("version".into(), Value::String(version.to_string()));
    manifest.insert("description".into(), Value::String(description.to_string()));
    let mut skills = Vec::new();
    for skill in &content.skills {
        skills.push(Value::String(format!("./{}", skill.dir)));
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
    if !skills.is_empty() {
        manifest.insert("skills".into(), Value::Array(skills));
    }
    let mut commands = Vec::new();
    for cmd in &content.commands {
        commands.push(Value::String(format!("./{}", cmd.asset.path)));
        pkg.insert(cmd.asset.path.clone(), cmd.frontmatter.to_yaml_safe_bytes())?;
    }
    if !commands.is_empty() {
        manifest.insert("commands".into(), Value::Array(commands));
    }
    if !content.hooks.is_empty() {
        manifest.insert("hooks".into(), Value::String(format!("./{HOOKS_FILE}")));
        pkg.insert_json(HOOKS_FILE, &hooks_document(content))?;
    }
    pkg.insert_json(manifest_path(), &Value::Object(manifest))?;
    Ok(pkg)
}

/// The `hooks.json` document: one matcher-less group per event, handlers in
/// catalog order.
pub fn hooks_document(content: &Content) -> Value {
    let mut by_event: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for h in &content.hooks {
        let mut handler = Map::new();
        handler.insert("type".into(), Value::String("command".into()));
        handler.insert("command".into(), Value::String(shell_join(&h.command)));
        if let Some(ms) = h.timeout_ms {
            handler.insert("timeout".into(), Value::from(timeout_seconds(ms)));
        }
        if let Some(msg) = &h.status_message {
            handler.insert("statusMessage".into(), Value::String(msg.clone()));
        }
        if h.is_async {
            handler.insert("async".into(), Value::Bool(true));
        }
        by_event
            .entry(h.event.as_str())
            .or_default()
            .push(Value::Object(handler));
    }
    let mut events = Map::new();
    for (event, handlers) in by_event {
        events.insert(event.to_string(), json!([{ "hooks": handlers }]));
    }
    json!({ "hooks": events })
}

/// Milliseconds → whole seconds, rounded up, never below 1 (`timeout` is
/// seconds in that schema and `0` means the host's minimum).
pub fn timeout_seconds(ms: i64) -> u64 {
    let ms = ms.max(0) as u64;
    ms.div_ceil(1000).max(1)
}

/// Join argv into one POSIX shell string: bare tokens stay bare, anything
/// else is single-quoted with `'\''` for embedded quotes.
pub fn shell_join(argv: &[String]) -> String {
    argv.iter()
        .map(|t| {
            let bare = !t.is_empty()
                && t.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-./=:@%+,".contains(c));
            if bare {
                t.clone()
            } else {
                format!("'{}'", t.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_join_and_timeouts() {
        assert_eq!(
            shell_join(&["omm".into(), "hook".into(), "guard".into()]),
            "omm hook guard"
        );
        assert_eq!(
            shell_join(&["a b".into(), "it's".into()]),
            "'a b' 'it'\\''s'"
        );
        assert_eq!(timeout_seconds(2000), 2);
        assert_eq!(timeout_seconds(2001), 3);
        assert_eq!(timeout_seconds(0), 1);
        assert_eq!(timeout_seconds(-5), 1);
    }

    #[test]
    fn projection_has_that_schema_shape() {
        let repo = crate::Repo::from_cargo_manifest_dir().unwrap();
        let catalog = Catalog::load(&repo.catalog_path()).unwrap();
        let content = Content::load(&catalog, &repo.content_dir()).unwrap();
        let pkg = render(&catalog, &content, "1.0.0", "d").unwrap();
        let m = pkg.get_json(&manifest_path()).unwrap();
        assert_eq!(m["name"], "oh-my-musecode");
        assert!(m["skills"][0].as_str().unwrap().starts_with("./skills/"));
        assert!(m["commands"][0].as_str().unwrap().ends_with(".md"));
        assert_eq!(m["hooks"], "./hooks/hooks.json");
        let hooks = pkg.get_json(HOOKS_FILE).unwrap();
        let first_event = hooks["hooks"].as_object().unwrap().values().next().unwrap();
        let handler = &first_event[0]["hooks"][0];
        assert_eq!(handler["type"], "command");
        assert!(handler["command"]
            .as_str()
            .unwrap()
            .starts_with("omm hook "));
        assert!(handler["timeout"].is_number());
        assert!(handler.get("matcher").is_none());
        assert!(
            pkg.get(".muse-plugin/plugin.json").is_none(),
            "one manifest dir per package"
        );
    }
}
