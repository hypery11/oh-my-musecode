//! One fixture per lint rule (PLAN.md 1.2): the minimal tree is the negative
//! fixture for every rule (it lints with zero findings); each test mutates
//! one thing and asserts exactly that rule fires.

mod common;

use common::{pretty, reminder_decision, row, skill_row, Tree};
use serde_json::json;

fn assert_fires(tree: &Tree, rule: &str) {
    let report = tree.lint();
    let rules = report.rules();
    assert!(
        rules.contains(rule),
        "expected rule `{rule}` to fire; findings: {:#?}",
        report.findings
    );
}

fn assert_message(tree: &Tree, rule: &str, needle: &str) {
    let report = tree.lint();
    let hits = report.of(rule);
    assert!(
        hits.iter().any(|f| f.message.contains(needle)),
        "rule `{rule}` message lacks {needle:?}: {hits:#?}"
    );
}

#[test]
fn the_minimal_tree_is_the_negative_fixture_for_every_rule() {
    let tree = Tree::minimal();
    let report = tree.lint();
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    assert!(report.is_clean());
    assert_eq!(
        report.package_files,
        Some(1 + 3 + 1 + 1),
        "manifest, alpha (2 files), beta, command, duty"
    );
    let budget = report.budget.unwrap();
    assert_eq!(budget.entries.len(), 2);
    assert!(budget.stale().is_empty());
}

// ---- catalog ---------------------------------------------------------------

#[test]
fn catalog_load() {
    let tree = Tree::minimal();
    tree.raw_catalog("{not json");
    assert_fires(&tree, "catalog-load");
    let mut tree = Tree::minimal();
    tree.edit_asset("omm-alpha", |a| a["lifecycle"] = json!("deprecated"));
    assert_message(&tree, "catalog-load", "core asset");
}

#[test]
fn catalog_plugin_id() {
    let mut tree = Tree::minimal();
    tree.edit_catalog(|c| c["plugin_id"] = json!("muse-core"));
    assert_fires(&tree, "catalog-plugin-id");
}

#[test]
fn catalog_missing_file_and_orphan_file() {
    let tree = Tree::minimal();
    tree.remove("commands/omm-cmd.md");
    assert_fires(&tree, "catalog-missing-file");
    let tree = Tree::minimal();
    tree.write("commands/omm-extra.md", "---\ndescription: x\n---\n");
    assert_fires(&tree, "catalog-orphan-file");
    // A stray file inside a skill directory is claimed by the skill; a doc file is tolerated.
    let tree = Tree::minimal();
    tree.write("skills/omm-alpha/references/more.md", "x\n");
    tree.write("hooks/README.md", "docs\n");
    assert!(tree.lint().findings.is_empty());
}

#[test]
fn catalog_budget_stale() {
    let mut tree = Tree::minimal();
    tree.edit_asset("omm-alpha", |a| a["budget_bytes"] = json!(1));
    let report = tree.lint();
    let hit = report.of("catalog-budget-stale");
    assert_eq!(hit.len(), 1, "{:#?}", report.findings);
    assert_eq!(hit[0].severity, omm_manifest::lint::Severity::Warning);
    assert!(hit[0].fix.contains("set budget_bytes to"));
}

// ---- filesystem hygiene (R12) ----------------------------------------------

#[cfg(unix)]
#[test]
fn fs_symlink() {
    let tree = Tree::minimal();
    std::os::unix::fs::symlink(
        tree.path("README.md"),
        tree.path("skills/omm-alpha/link.md"),
    )
    .unwrap();
    assert_fires(&tree, "fs-symlink");
}

#[cfg(unix)]
#[test]
fn fs_backslash() {
    let tree = Tree::minimal();
    tree.write("skills/omm-alpha/references/bad\\name.md", "x\n");
    assert_fires(&tree, "fs-backslash");
}

// ---- skills -----------------------------------------------------------------

#[test]
fn skill_path() {
    let mut tree = Tree::minimal();
    tree.write(
        "skills/other-dir/SKILL.md",
        "---\nname: omm-alpha\ndescription: x; Do not use.\n---\n",
    );
    tree.edit_asset("omm-alpha", |a| {
        a["path"] = json!("skills/other-dir/SKILL.md")
    });
    tree.remove("skills/omm-alpha");
    assert_fires(&tree, "skill-path");
}

#[test]
fn skill_frontmatter() {
    let tree = Tree::minimal();
    tree.write("skills/omm-alpha/SKILL.md", "no front matter\n");
    assert_fires(&tree, "skill-frontmatter");
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/SKILL.md",
        "---\nname: omm-alpha\nname: omm-alpha\ndescription: x; Do not use.\n---\n",
    );
    assert_message(&tree, "skill-frontmatter", "duplicate");
}

#[test]
fn skill_bom() {
    let tree = Tree::minimal();
    let mut bytes = b"\xef\xbb\xbf".to_vec();
    bytes.extend_from_slice(
        format!(
            "---\nname: omm-alpha\ndescription: {}\n---\n",
            common::SKILL_DESC
        )
        .as_bytes(),
    );
    tree.write_bytes("skills/omm-alpha/SKILL.md", &bytes);
    assert_fires(&tree, "skill-bom");
}

#[test]
fn skill_description_missing() {
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/SKILL.md",
        "---\nname: omm-alpha\n---\nbody\n",
    );
    assert_fires(&tree, "skill-description-missing");
}

#[test]
fn skill_name() {
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/SKILL.md",
        &format!(
            "---\nname: omm-other\ndescription: {}\n---\n",
            common::SKILL_DESC
        ),
    );
    assert_fires(&tree, "skill-name");
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/SKILL.md",
        &format!("---\ndescription: {}\n---\n", common::SKILL_DESC),
    );
    assert_message(&tree, "skill-name", "no `name`");
}

#[test]
fn skill_description_length() {
    let tree = Tree::minimal();
    let long = format!("Use when {}; Do not use otherwise.", "x".repeat(240));
    tree.write(
        "skills/omm-alpha/SKILL.md",
        &format!("---\nname: omm-alpha\ndescription: {long}\n---\n"),
    );
    let rules = tree.rules();
    assert!(rules.contains("skill-description-length"), "{rules:?}");
}

#[test]
fn skill_negative_trigger() {
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/SKILL.md",
        "---\nname: omm-alpha\ndescription: Use when alpha applies.\n---\n",
    );
    assert_message(&tree, "skill-negative-trigger", "no `Do not use");
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/SKILL.md",
        "---\nname: omm-alpha\ndescription: Use when alpha applies. Do not use otherwise.\n---\n",
    );
    assert_message(&tree, "skill-negative-trigger", "first_sentence");
}

#[test]
fn skill_body_size() {
    let tree = Tree::minimal();
    let body = "x".repeat(omm_manifest::lint::SKILL_BODY_MAX_BYTES + 1);
    tree.write(
        "skills/omm-alpha/SKILL.md",
        &format!(
            "---\nname: omm-alpha\ndescription: {}\n---\n{body}",
            common::SKILL_DESC
        ),
    );
    assert_fires(&tree, "skill-body-size");
}

// ---- commands -----------------------------------------------------------------

#[test]
fn command_frontmatter_and_description() {
    let tree = Tree::minimal();
    tree.write("commands/omm-cmd.md", "no front matter\n");
    assert_fires(&tree, "command-frontmatter");
    let tree = Tree::minimal();
    tree.write("commands/omm-cmd.md", "---\nargument-hint: x\n---\nbody\n");
    assert_fires(&tree, "command-description");
}

// ---- hooks --------------------------------------------------------------------

fn hook(extra: &str) -> String {
    format!("{{\"id\":\"omm-start\",\"event\":\"SessionStart\",\"command\":[\"omm\",\"hook\",\"start\"],\"timeoutMs\":2000{extra}}}")
}

#[test]
fn hook_json() {
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "[1]");
    assert_fires(&tree, "hook-json");
}

#[test]
fn hook_field_rejects_command_windows_and_unknown_fields() {
    let tree = Tree::minimal();
    tree.write(
        "hooks/omm-start.json",
        &hook(",\"commandWindows\":[\"omm\",\"hook\",\"start\"]"),
    );
    assert_message(&tree, "hook-field", "commandWindows");
    assert_message(&tree, "hook-field", "Windows twin");
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", &hook(",\"matcher\":\"*\""));
    assert_message(&tree, "hook-field", "matcher");
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", &hook(",\"zzz\":1"));
    assert_message(&tree, "hook-field", "zzz");
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", &hook(",\"async\":\"no\""));
    assert_message(&tree, "hook-field", "async");
}

#[test]
fn hook_event() {
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"session_start\",\"command\":[\"omm\",\"hook\",\"start\"],\"timeoutMs\":2000}");
    assert_message(&tree, "hook-event", "PascalCase");
    let mut tree = Tree::minimal();
    tree.edit_asset("omm-start", |a| a["event"] = json!("Stop"));
    assert_message(&tree, "hook-event", "catalog says");
}

#[test]
fn hook_command() {
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"SessionStart\",\"command\":\"omm hook start\",\"timeoutMs\":2000}");
    assert_fires(&tree, "hook-command");
}

#[test]
fn hook_argv_timeout_output_capabilities_compatibility_name() {
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"SessionStart\",\"command\":[\"sh\",\"hooks/x.sh\"],\"timeoutMs\":2000}");
    assert_fires(&tree, "hook-argv");
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"SessionStart\",\"command\":[\"omm\",\"hook\",\"start\"]}");
    assert_message(&tree, "hook-timeout", "no timeoutMs");
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"SessionStart\",\"command\":[\"omm\",\"hook\",\"start\"],\"timeoutMs\":0}");
    assert_message(&tree, "hook-timeout", "below");
    let tree = Tree::minimal();
    tree.write(
        "hooks/omm-start.json",
        &hook(",\"outputCapabilities\":[\"skills.v1\"]"),
    );
    assert_fires(&tree, "hook-output-capabilities");
    let tree = Tree::minimal();
    tree.write(
        "hooks/omm-start.json",
        &hook(",\"compatibilityName\":\"Bash\""),
    );
    assert_fires(&tree, "hook-compatibility-name");
    // Accepted: skills.v1 on a foreground UserPromptSubmit hook.
    let mut tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"UserPromptSubmit\",\"command\":[\"omm\",\"hook\",\"start\"],\"timeoutMs\":2000,\"outputCapabilities\":[\"skills.v1\"],\"compatibilityName\":\"omm_router\"}");
    tree.edit_asset("omm-start", |a| a["event"] = json!("UserPromptSubmit"));
    assert!(
        tree.lint().findings.is_empty(),
        "{:#?}",
        tree.lint().findings
    );
}

#[test]
fn id_mismatch() {
    let tree = Tree::minimal();
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-other\",\"event\":\"SessionStart\",\"command\":[\"omm\",\"hook\",\"start\"],\"timeoutMs\":2000}");
    assert_fires(&tree, "id-mismatch");
}

// ---- reminders ----------------------------------------------------------------

#[test]
fn reminder_shape_path_envelope() {
    let tree = Tree::minimal();
    tree.write(
        "reminders/omm-nudge.json",
        "{\"id\":\"omm-nudge\",\"path\":\"reminders/omm-nudge.md\"}",
    );
    assert_message(&tree, "reminder-shape", "decision");
    let tree = Tree::minimal();
    let r = json!({"id":"omm-nudge","path":"reminders/other.md","decision": reminder_decision()});
    tree.write("reminders/omm-nudge.json", &pretty(&r));
    assert_fires(&tree, "reminder-path");
    let tree = Tree::minimal();
    let mut d = reminder_decision();
    d["envelope"]["template"] = json!("<system-reminder>\n{text}\n</system-reminder>");
    let r = json!({"id":"omm-nudge","path":"reminders/omm-nudge.md","decision": d});
    tree.write("reminders/omm-nudge.json", &pretty(&r));
    assert_message(&tree, "reminder-envelope", "system-reminder");
    let tree = Tree::minimal();
    let mut d = reminder_decision();
    d["envelope"]["template"] = json!("<omm-reminder>{text}{text}</omm-reminder>");
    let r = json!({"id":"omm-nudge","path":"reminders/omm-nudge.md","decision": d});
    tree.write("reminders/omm-nudge.json", &pretty(&r));
    assert_message(&tree, "reminder-envelope", "exactly one");
}

#[test]
fn id_reminder_collision_and_command_equals_skill() {
    let mut tree = Tree::minimal();
    tree.write("reminders/omm-alpha.md", "duty\n");
    let r =
        json!({"id":"omm-alpha","path":"reminders/omm-alpha.md","decision": reminder_decision()});
    tree.write("reminders/omm-alpha.json", &pretty(&r));
    tree.edit_catalog(|c| {
        let a = c["assets"].as_array_mut().unwrap();
        a.retain(|r| r["id"] != "omm-nudge");
        let mut row = row("omm-alpha-rem", "reminder", "reminders/omm-alpha.json");
        row["duty"] = json!("reminders/omm-alpha.md");
        a.push(row);
    });
    tree.remove("reminders/omm-nudge.json");
    tree.remove("reminders/omm-nudge.md");
    // The file says `omm-alpha`, the catalog `omm-alpha-rem`: id-mismatch fires first.
    assert_fires(&tree, "id-mismatch");
    // With a consistent id that equals a skill id:
    let mut tree = Tree::minimal();
    tree.write("commands/omm-alpha.md", "---\ndescription: clash\n---\n");
    tree.push_asset(row("omm-alpha", "command", "commands/omm-alpha.md"));
    assert_message(&tree, "catalog-load", "duplicate asset id");
    let mut tree = Tree::minimal();
    let r =
        json!({"id":"omm-cmd2","path":"reminders/omm-nudge.md","decision": reminder_decision()});
    tree.write("reminders/omm-cmd2.json", &pretty(&r));
    tree.write("commands/omm-cmd2.md", "---\ndescription: two\n---\n");
    tree.push_asset(row("omm-cmd2", "command", "commands/omm-cmd2.md"));
    let mut rr = row("omm-cmd2-rem", "reminder", "reminders/omm-cmd2.json");
    rr["duty"] = json!("reminders/omm-nudge.md");
    tree.push_asset(rr);
    // catalog id omm-cmd2-rem vs file id omm-cmd2 → id-mismatch; make them agree:
    tree.edit_asset("omm-cmd2-rem", |a| a["id"] = json!("omm-cmd2"));
    assert_message(&tree, "catalog-load", "duplicate asset id");
}

#[test]
fn id_command_equals_skill_via_content() {
    // Two catalog rows cannot share an id, so the collision the host reports
    // (`command capability id duplicates a skill id`) is reached through the
    // content loader with a lenient catalog: emulate by a command whose file
    // id is checked against skills in lint_ids.
    let mut tree = Tree::minimal();
    tree.write("commands/omm-beta.md", "---\ndescription: clash\n---\n");
    tree.edit_asset("omm-cmd", |a| {
        a["path"] = json!("commands/omm-beta.md");
    });
    tree.remove("commands/omm-cmd.md");
    // The command row is still `omm-cmd`; rename it to the skill id through a fresh row list.
    tree.edit_catalog(|c| {
        for r in c["assets"].as_array_mut().unwrap() {
            if r["kind"] == "skill" && r["id"] == "omm-beta" {
                r["id"] = json!("omm-beta2");
                r["path"] = json!("skills/omm-beta2/SKILL.md");
            }
            if r["id"] == "omm-cmd" {
                r["id"] = json!("omm-beta");
            }
        }
    });
    std::fs::rename(tree.path("skills/omm-beta"), tree.path("skills/omm-beta2")).unwrap();
    tree.write(
        "skills/omm-beta2/SKILL.md",
        &format!(
            "---\nname: omm-beta2\ndescription: \"{}\"\n---\n",
            common::SKILL2_DESC
        ),
    );
    // Now a command `omm-beta` and skills omm-alpha / omm-beta2: no clash. Add a skill named like the command:
    tree.write(
        "skills/omm-beta/SKILL.md",
        &format!(
            "---\nname: omm-beta\ndescription: \"{}\"\n---\n",
            common::SKILL2_DESC
        ),
    );
    tree.push_asset(json!({
        "id": "omm-beta", "kind": "skill", "path": "skills/omm-beta/SKILL.md", "lifecycle": "active",
        "core": false, "canonical": null, "since": "0.1.0", "sunset": null, "budget_bytes": 0
    }));
    assert_message(&tree, "catalog-load", "duplicate asset id");
}

// ---- identity -------------------------------------------------------------------

#[test]
fn id_grammar_prefix_reserved_bundled_slash_windows() {
    let mut tree = Tree::minimal();
    tree.push_asset(row("Omm-Bad", "theme", "themes/omm-dark.tmTheme"));
    assert_fires(&tree, "id-grammar");
    let mut tree = Tree::minimal();
    tree.push_asset(row("plainname", "theme", "themes/omm-dark.tmTheme"));
    assert_fires(&tree, "id-prefix");
    let mut tree = Tree::minimal();
    tree.push_asset(row("muse-core", "theme", "themes/omm-dark.tmTheme"));
    assert_fires(&tree, "id-reserved");
    let mut tree = Tree::minimal();
    tree.push_asset(row("git", "theme", "themes/omm-dark.tmTheme"));
    assert_fires(&tree, "id-bundled-skill");
    let mut tree = Tree::minimal();
    tree.write("commands/plan.md", "---\ndescription: clash\n---\n");
    tree.push_asset(row("plan", "command", "commands/plan.md"));
    assert_fires(&tree, "id-slash-collision");
    let mut tree = Tree::minimal();
    tree.write("commands/cost.md", "---\ndescription: alias clash\n---\n");
    tree.push_asset(row("cost", "command", "commands/cost.md"));
    assert_message(&tree, "id-slash-collision", "/cost");
    let mut tree = Tree::minimal();
    tree.write("commands/login.md", "---\ndescription: hidden clash\n---\n");
    tree.push_asset(row("login", "command", "commands/login.md"));
    assert_message(&tree, "id-slash-collision", "/login");
    let mut tree = Tree::minimal();
    tree.push_asset(row("con.omm", "theme", "themes/omm-dark.tmTheme"));
    assert_fires(&tree, "id-windows-stem");
}

#[test]
fn mcp_id_length_and_shape() {
    let mut tree = Tree::minimal();
    tree.write(
        "mcp/omm-doctor-and-cost-x.json",
        "{\"id\":\"omm-doctor-and-cost-x\",\"transport\":\"stdio\",\"command\":[\"omm\",\"mcp\"]}",
    );
    tree.push_asset(row(
        "omm-doctor-and-cost-x",
        "mcp_server",
        "mcp/omm-doctor-and-cost-x.json",
    ));
    assert_fires(&tree, "mcp-id-length");
    let mut tree = Tree::minimal();
    tree.write("mcp/doc.json", "{\"id\":\"doc\",\"transport\":\"stdio\",\"command\":[\"omm\",\"mcp\"],\"env\":{\"A\":\"1\"}}");
    tree.push_asset(row("doc", "mcp_server", "mcp/doc.json"));
    assert_fires(&tree, "mcp-dropped-field");
    let mut tree = Tree::minimal();
    tree.write(
        "mcp/doc.json",
        "{\"id\":\"doc\",\"transport\":\"sse\",\"url\":\"x\"}",
    );
    tree.push_asset(row("doc", "mcp_server", "mcp/doc.json"));
    assert_fires(&tree, "mcp-shape");
    // A well-formed stdio server within the 18-char budget is clean and lands in both projections.
    let mut tree = Tree::minimal();
    tree.write(
        "mcp/doc.json",
        "{\"id\":\"doc\",\"transport\":\"stdio\",\"command\":[\"omm\",\"mcp\",\"--x\"]}",
    );
    tree.push_asset(row("doc", "mcp_server", "mcp/doc.json"));
    assert!(
        tree.lint().findings.is_empty(),
        "{:#?}",
        tree.lint().findings
    );
    let out = tree.build();
    let m = out.native.get_json(".muse-plugin/plugin.json").unwrap();
    assert_eq!(m["capabilities"]["mcpServers"][0]["id"], "doc");
    let mcp = out.codex.get_json(".mcp.json").unwrap();
    assert_eq!(mcp["mcpServers"]["doc"]["command"], "omm");
    assert_eq!(mcp["mcpServers"]["doc"]["args"], json!(["mcp", "--x"]));
}

#[test]
fn an_mcp_server_id_is_namespaced_by_the_host_so_prefix_and_bundled_rules_do_not_apply() {
    // The shipped server is `doc` (wire namespace
    // `mcp__plugin_oh_my_musecode_doc` — the host sanitizes `-` to `_`;
    // stable id `plugin:oh-my-musecode:mcp_server:doc`): neither a slash name
    // nor a bundled skill can collide with it, and `omm-` would only spend
    // the 18-char budget.
    let mut tree = Tree::minimal();
    tree.write(
        "mcp/doc.json",
        "{\"id\":\"doc\",\"transport\":\"stdio\",\"command\":[\"omm\",\"mcp\"],\"enabledDefault\":true}",
    );
    tree.push_asset(row("doc", "mcp_server", "mcp/doc.json"));
    let report = tree.lint();
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    let out = tree.build();
    let m = out.native.get_json(".muse-plugin/plugin.json").unwrap();
    assert_eq!(
        m["capabilities"]["mcpServers"],
        json!([{"id": "doc", "transport": "stdio", "command": ["omm", "mcp"], "enabledDefault": true}])
    );
    assert_eq!(
        out.codex.get_json(".mcp.json").unwrap()["mcpServers"]["doc"],
        json!({"type": "stdio", "command": "omm", "args": ["mcp"]})
    );
    // The same id on a skill or command still fires both rules; a reserved
    // plugin id and a Windows stem still fire for a server.
    let mut tree = Tree::minimal();
    tree.write("commands/doctor.md", "---\ndescription: x\n---\nbody\n");
    tree.push_asset(row("doctor", "command", "commands/doctor.md"));
    assert_fires(&tree, "id-prefix");
    assert_fires(&tree, "id-bundled-skill");
    let mut tree = Tree::minimal();
    tree.write(
        "mcp/loop.json",
        "{\"id\":\"loop\",\"transport\":\"stdio\",\"command\":[\"omm\",\"mcp\"]}",
    );
    tree.push_asset(row("loop", "mcp_server", "mcp/loop.json"));
    assert_fires(&tree, "id-reserved");
    let mut tree = Tree::minimal();
    tree.write(
        "mcp/con.json",
        "{\"id\":\"con\",\"transport\":\"stdio\",\"command\":[\"omm\",\"mcp\"]}",
    );
    tree.push_asset(row("con", "mcp_server", "mcp/con.json"));
    assert_fires(&tree, "id-windows-stem");
}

// ---- profiles, rules, themes ----------------------------------------------------

#[test]
fn profile_json_and_key() {
    let tree = Tree::minimal();
    tree.write("profiles/omm-fast.json", "[]");
    assert_fires(&tree, "profile-json");
    let tree = Tree::minimal();
    tree.write(
        "profiles/omm-fast.json",
        "{\"mcp_servers\": {}, \"omm_stuff\": 1}",
    );
    assert_message(&tree, "profile-key", "mcpServers");
    assert_message(&tree, "profile-key", "omm_stuff");
}

#[test]
fn rules_markers_and_theme_shape() {
    let tree = Tree::minimal();
    tree.write("rules/AGENTS.md.tmpl", "# no markers\n");
    assert_fires(&tree, "rules-markers");
    let tree = Tree::minimal();
    tree.write("themes/omm-dark.tmTheme", "not a plist\n");
    assert_fires(&tree, "theme-shape");
}

// ---- foreign vocabulary ------------------------------------------------------------

#[test]
fn foreign_tool_vocabulary_and_product_name() {
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/references/notes.md",
        "Call the `Task` tool.\n",
    );
    assert_fires(&tree, "foreign-tool-vocabulary");
    let tree = Tree::minimal();
    tree.write(
        "commands/omm-cmd.md",
        "---\ndescription: x\n---\nUse the Grep tool.\n",
    );
    assert_fires(&tree, "foreign-tool-vocabulary");
    // R8: the allowlist is catalog data — an asset flagged `foreign_vocab`
    // may name foreign tools (it teaches the mapping); the translation and
    // rules kinds are exempt by kind. No path is spelled in Rust.
    let mut tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/references/notes.md",
        "Call the `Task` tool.\n",
    );
    assert_fires(&tree, "foreign-tool-vocabulary");
    tree.edit_asset("omm-alpha", |a| a["foreign_vocab"] = json!(true));
    assert!(
        tree.lint().findings.is_empty(),
        "{:#?}",
        tree.lint().findings
    );
    // Bare prose verbs are not tool references; translation content is allowlisted.
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/references/notes.md",
        "## Read everything\n\nWrite it before you Edit.\n",
    );
    tree.write(
        "translation/muse.md",
        "| `Task` | `subagent_spawn` |\nthe TodoWrite tool\n",
    );
    tree.write(
        "rules/AGENTS.md.tmpl",
        "<!-- omm:managed-start -->\n`Read` maps to read_file\n<!-- omm:managed-end -->\n",
    );
    assert!(
        tree.lint().findings.is_empty(),
        "{:#?}",
        tree.lint().findings
    );
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/references/notes.md",
        &format!("Written for {}.\n", ["Claude", "Code"].join(" ")),
    );
    assert_fires(&tree, "foreign-product-name");
    let tree = Tree::minimal();
    tree.write(
        "skills/omm-alpha/references/notes.md",
        "Ported from Codex.\n",
    );
    assert_message(&tree, "foreign-product-name", "bare product word");
    // Path/format literals are exempt.
    let tree = Tree::minimal();
    tree.write("skills/omm-alpha/references/notes.md", "See .claude-plugin, CLAUDE.md, ~/.claude/skills and `muse skills import --from claude|codex`.\n");
    assert!(
        tree.lint().findings.is_empty(),
        "{:#?}",
        tree.lint().findings
    );
}

// ---- budget ---------------------------------------------------------------------------

#[test]
fn budget_full_and_first_sentence() {
    let mut tree = Tree::minimal();
    let desc = format!(
        "Use when {} Do not use otherwise",
        "very long trigger words ".repeat(8)
    );
    assert!(desc.chars().count() <= 240, "{}", desc.chars().count());
    for i in 0..80 {
        let id = format!("omm-s{i:02}");
        tree.write(
            &format!("skills/{id}/SKILL.md"),
            &format!("---\nname: {id}\ndescription: {desc}\n---\n"),
        );
        tree.push_asset(skill_row(&id, &desc, None, false));
    }
    let report = tree.lint();
    let rules = report.rules();
    assert!(rules.contains("budget-full"), "{rules:?}");
    assert!(rules.contains("budget-first-sentence"), "{rules:?}");
    let b = report.budget.unwrap();
    assert!(b.total_full > b.limit_full && b.total_first_sentence > b.limit_first_sentence);
}

// ---- package limits ------------------------------------------------------------------

#[test]
fn package_depth_and_entries() {
    let tree = Tree::minimal();
    let deep = format!(
        "skills/omm-alpha/{}/x.md",
        (0..15).map(|_| "d").collect::<Vec<_>>().join("/")
    );
    tree.write(&deep, "deep\n");
    assert_fires(&tree, "package-depth");
    let tree = Tree::minimal();
    for i in 0..(omm_manifest::hr::PACKAGE_MAX_FS_ENTRIES + 1) {
        tree.write(&format!("skills/omm-alpha/references/f{i}.md"), "x\n");
    }
    assert_fires(&tree, "package-entries");
}

#[test]
fn package_manifest_size_class_max_and_inventory() {
    let mut tree = Tree::minimal();
    let max = omm_manifest::hr::PER_CLASS_MAX_REMINDERS;
    for i in 0..(max + 2) {
        let id = format!("omm-rem{i:02}");
        tree.write(&format!("reminders/{id}.md"), "duty\n");
        let r = json!({"id": id, "path": format!("reminders/{id}.md"), "decision": reminder_decision()});
        tree.write(&format!("reminders/{id}.json"), &pretty(&r));
        let mut rr = row(&id, "reminder", &format!("reminders/{id}.json"));
        rr["duty"] = json!(format!("reminders/{id}.md"));
        tree.push_asset(rr);
    }
    let report = tree.lint();
    let rules = report.rules();
    assert!(rules.contains("package-class-max"), "{rules:?}");
    assert!(rules.contains("package-manifest-size"), "{rules:?}");
    // Inventory: 1/class + 2/skill + 1/other, checked directly on a synthetic content.
    let tree = Tree::minimal();
    let (_catalog, mut content) = tree.load();
    let proto = content.skills[0].clone();
    for i in 0..2100 {
        let mut s = proto.clone();
        s.asset.id = format!("omm-x{i}");
        content.skills.push(s);
    }
    // The inventory and per-class rules count capabilities, not files: an
    // empty package is enough (rendering 2,100 clones of one skill directory
    // would collide on the same paths).
    let pkg = omm_manifest::generate::Package::new();
    let findings = omm_manifest::lint::lint_package(&pkg, &content);
    let rules: std::collections::BTreeSet<&str> =
        findings.iter().map(|f| f.rule.as_str()).collect();
    assert!(rules.contains("package-inventory"), "{rules:?}");
    assert!(rules.contains("package-class-max"), "{rules:?}");
}

// ---- version ----------------------------------------------------------------------------

#[test]
fn version_load() {
    let tree = Tree::minimal();
    tree.write_repo_file(
        "crates/omm/Cargo.toml",
        "[package]\nname = \"omm\"\nversion = \"0.1.0\"\n",
    );
    assert_fires(&tree, "version-load");
}

// ---- marketplace ------------------------------------------------------------------------

#[test]
fn marketplace_rules() {
    let tree = Tree::minimal();
    let report = tree.lint_with_marketplace();
    let rules = report.rules();
    assert!(rules.contains("marketplace-root-missing"), "{rules:?}");
    assert!(rules.contains("marketplace-projection-missing"));
    // After a build everything is present and clean.
    let out = tree.build();
    omm_manifest::generate::write(&tree.repo(), &out).unwrap();
    let report = tree.lint_with_marketplace();
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    // Corruptions.
    let corrupt = |rel: &str, edit: &dyn Fn(&mut serde_json::Value)| {
        let p = tree.root.join(rel);
        let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        edit(&mut v);
        std::fs::write(&p, pretty(&v)).unwrap();
    };
    corrupt("marketplace.json", &|v| {
        v["plugins"][0]["name"] = json!("ohmy-omm")
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-name"));
    corrupt("marketplace.json", &|v| {
        v["plugins"][0]["name"] = json!("omm");
        v["plugins"][0]["install"]["source"] = json!("../plugins/omm");
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-source"));
    corrupt("marketplace.json", &|v| {
        v["plugins"][0]["install"]["source"] = json!("plugins/omm");
        v["plugins"][0]["integrity"]["digest"] = json!("sha256:nope");
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-digest"));
    corrupt("marketplace.json", &|v| {
        v["plugins"][0]["integrity"]["digest"] = json!(Tree::fixed_digest());
        v["source"] = json!("git");
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-shape"));
    corrupt("marketplace.json", &|v| v["source"] = json!("local"));
    corrupt(".claude-plugin/marketplace.json", &|v| {
        v["name"] = json!("other")
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-name"));
    corrupt(".claude-plugin/marketplace.json", &|v| {
        v["name"] = json!("ohmy");
        v["plugins"][0]["source"] = json!({"source": "local", "path": "dist/claude"});
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-source"));
    corrupt(".agents/plugins/marketplace.json", &|v| {
        v["plugins"][0]["source"] = json!("dist/codex")
    });
    assert!(tree
        .lint_with_marketplace()
        .rules()
        .contains("marketplace-source"));
    // A stale digest is caught when the expected one is known.
    let (catalog, _) = tree.load();
    let findings = omm_manifest::lint::lint_marketplace(
        &tree.repo(),
        &catalog,
        Some(&format!("sha256:{}", "f".repeat(64))),
    )
    .unwrap();
    assert!(
        findings
            .iter()
            .any(|f| f.rule == "marketplace-digest" && f.message.contains("stale")),
        "{findings:#?}"
    );
}
