//! §5.4 overlay resolution: precedence, append, disable, `_shadowed`.

mod common;

use common::Tree;
use omm_manifest::catalog::AssetKind;
use omm_manifest::overlay::{self, Provider};

fn omm_root(tree: &Tree) -> std::path::PathBuf {
    let root = tree.root.join("omm-config");
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn bundled_only_resolves_everything_unshadowed() {
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let root = omm_root(&tree);
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    assert_eq!(res.items.len(), catalog.assets.len());
    assert!(res.items.iter().all(|i| i.provider == Provider::Bundled
        && i.shadowed.is_empty()
        && !i.disabled
        && i.append.is_none()));
    assert!(res.problems.is_empty() && res.unknown_disabled.is_empty());
    let alpha = res.get("omm-alpha").unwrap();
    assert_eq!(alpha.qualified_id, "skill:omm-alpha");
    assert!(
        alpha.path.ends_with("skills/omm-alpha"),
        "{}",
        alpha.path.display()
    );
    assert_eq!(alpha.level, "bundled");
    let cmd = res.get("omm-cmd").unwrap();
    assert!(cmd.path.ends_with("commands/omm-cmd.md"));
    let json = serde_json::to_value(&res).unwrap();
    assert!(json["items"][0]["_shadowed"].is_array(), "{json}");
}

#[test]
fn custom_replaces_bundled_and_records_the_shadow() {
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let root = omm_root(&tree);
    let custom_skill = root.join("custom/skills/omm-alpha");
    std::fs::create_dir_all(&custom_skill).unwrap();
    std::fs::write(
        custom_skill.join("SKILL.md"),
        "---\nname: omm-alpha\ndescription: mine; Do not use.\n---\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("custom/commands")).unwrap();
    std::fs::write(
        root.join("custom/commands/omm-cmd.md"),
        "---\ndescription: mine\n---\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("custom/hooks")).unwrap();
    std::fs::write(root.join("custom/hooks/omm-start.json"), "{}").unwrap();
    // A skill dir without SKILL.md is not an overlay.
    std::fs::create_dir_all(root.join("custom/skills/omm-beta")).unwrap();
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    let alpha = res.get("omm-alpha").unwrap();
    assert_eq!(alpha.provider, Provider::Custom);
    assert_eq!(alpha.level, "user");
    assert_eq!(alpha.path, std::fs::canonicalize(&custom_skill).unwrap());
    assert_eq!(alpha.shadowed.len(), 1);
    assert_eq!(alpha.shadowed[0].provider, Provider::Bundled);
    assert_eq!(alpha.shadowed[0].level, "bundled");
    assert!(alpha.shadowed[0].path.ends_with("skills/omm-alpha"));
    assert_eq!(res.get("omm-cmd").unwrap().provider, Provider::Custom);
    assert_eq!(res.get("omm-start").unwrap().provider, Provider::Custom);
    assert_eq!(res.get("omm-beta").unwrap().provider, Provider::Bundled);
    let json = serde_json::to_value(&res).unwrap();
    let item = json["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "omm-alpha")
        .unwrap();
    assert_eq!(item["_shadowed"][0]["provider"], "bundled");
    assert_eq!(item["provider"], "custom");
}

#[test]
fn append_is_resolved_independently_of_replace() {
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let root = omm_root(&tree);
    std::fs::create_dir_all(root.join("custom/skills")).unwrap();
    std::fs::write(root.join("custom/skills/omm-alpha_append.md"), "extra\n").unwrap();
    std::fs::create_dir_all(root.join("custom/commands")).unwrap();
    std::fs::write(root.join("custom/commands/omm-cmd_append.md"), "extra\n").unwrap();
    std::fs::create_dir_all(root.join("custom/hooks")).unwrap();
    std::fs::write(
        root.join("custom/hooks/omm-start_append.md"),
        "ignored: hooks do not append\n",
    )
    .unwrap();
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    let alpha = res.get("omm-alpha").unwrap();
    assert_eq!(alpha.provider, Provider::Bundled, "append never replaces");
    assert!(alpha
        .append
        .as_ref()
        .unwrap()
        .ends_with("omm-alpha_append.md"));
    assert!(res.get("omm-cmd").unwrap().append.is_some());
    assert!(res.get("omm-start").unwrap().append.is_none());
    // The append file is not mistaken for a user-added asset.
    assert!(res.get("omm-alpha_append").is_none());
    assert_eq!(res.items.len(), catalog.assets.len());
}

#[test]
fn disabled_ids_are_capability_qualified_and_kept_in_the_table() {
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let root = omm_root(&tree);
    std::fs::write(root.join("config.json"), "{\"profile\":\"fast\",\"disabled\":[\"skill:omm-alpha\",\"hook:omm-start\",\"skill:omm-nope\"]}").unwrap();
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    assert!(res.get("omm-alpha").unwrap().disabled);
    assert!(res.get("omm-start").unwrap().disabled);
    assert!(!res.get("omm-beta").unwrap().disabled);
    assert_eq!(res.active().count(), res.items.len() - 2);
    assert_eq!(res.unknown_disabled, vec!["skill:omm-nope"]);
    // `command:omm-alpha` would not disable the skill.
    std::fs::write(
        root.join("config.json"),
        "{\"disabled\":[\"command:omm-alpha\"]}",
    )
    .unwrap();
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    assert!(!res.get("omm-alpha").unwrap().disabled);
    assert_eq!(res.unknown_disabled, vec!["command:omm-alpha"]);
}

#[test]
fn user_additions_and_aliases_resolve() {
    let mut tree = Tree::minimal();
    tree.push_asset(serde_json::json!({
        "id": "omm-old-alpha", "kind": "skill", "path": "skills/omm-alpha/SKILL.md", "lifecycle": "alias",
        "core": false, "canonical": "omm-alpha", "since": "0.1.0", "sunset": null, "budget_bytes": 0
    }));
    let (catalog, content) = tree.load();
    let root = omm_root(&tree);
    let custom_skill = root.join("custom/skills/omm-mine");
    std::fs::create_dir_all(&custom_skill).unwrap();
    std::fs::write(
        custom_skill.join("SKILL.md"),
        "---\nname: omm-mine\ndescription: mine; Do not use.\n---\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("custom/themes")).unwrap();
    std::fs::write(root.join("custom/themes/omm-mine.tmTheme"), "<plist/>").unwrap();
    std::fs::write(root.join("custom/themes/notes.txt"), "not a theme").unwrap();
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    let mine = res.get("omm-mine").unwrap();
    assert_eq!(mine.provider, Provider::Custom);
    assert!(mine.shadowed.is_empty());
    assert_eq!(res.of_kind(AssetKind::Theme).count(), 2);
    assert!(res.items.iter().all(|i| i.id != "notes"));
    let alias = res.get("omm-old-alpha").unwrap();
    assert_eq!(alias.qualified_id, "skill:omm-old-alpha");
    assert!(
        alias.path.ends_with("skills/omm-alpha"),
        "alias forwards to the canonical item"
    );
}

#[cfg(unix)]
#[test]
fn an_overlay_symlink_escaping_custom_is_reported_and_skipped() {
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let root = omm_root(&tree);
    std::fs::create_dir_all(root.join("custom/commands")).unwrap();
    let outside = tree.root.join("outside.md");
    std::fs::write(&outside, "---\ndescription: outside\n---\n").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("custom/commands/omm-cmd.md")).unwrap();
    let res = overlay::resolve(&catalog, &content, &root).unwrap();
    assert_eq!(res.get("omm-cmd").unwrap().provider, Provider::Bundled);
    assert_eq!(res.problems.len(), 1, "{:#?}", res.problems);
    assert_eq!(res.problems[0].rule, "overlay-containment");
}
