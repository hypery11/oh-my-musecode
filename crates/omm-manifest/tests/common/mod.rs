//! A throwaway repository with a minimal, lint-clean content tree, mutated by
//! the fixture tests one rule at a time.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tempfile::TempDir;

use omm_manifest::budget::entry_cost;
use omm_manifest::catalog::Catalog;
use omm_manifest::content::Content;
use omm_manifest::hr;
use omm_manifest::lint::{self, LintOptions, LintReport};
use omm_manifest::Repo;

pub const VERSION: &str = "0.1.0";
pub const DESCRIPTION: &str = "omm fixture bundle.";
pub const SKILL_DESC: &str = "Use when the fixture skill applies; Do not use otherwise.";
pub const SKILL2_DESC: &str = "Use for the second fixture skill; Do not use for the first.";

pub struct Tree {
    pub tmp: TempDir,
    pub root: PathBuf,
    pub content: PathBuf,
    pub catalog: Value,
}

impl Tree {
    /// A repo root with `crates/omm/Cargo.toml` and a content tree of one
    /// asset per kind (two skills), all lint-clean.
    pub fn minimal() -> Tree {
        let tmp = tempfile::Builder::new()
            .prefix("omm-manifest-fixture-")
            .tempdir()
            .unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let content = root.join("content");
        let mut t = Tree {
            tmp,
            root,
            content,
            catalog: Value::Null,
        };
        t.write_repo_file(
            "crates/omm/Cargo.toml",
            &format!("[package]\nname = \"omm\"\nversion = \"{VERSION}\"\ndescription = \"{DESCRIPTION}\"\n"),
        );
        t.write(
            "skills/omm-alpha/SKILL.md",
            &format!("---\nname: omm-alpha\ndescription: {SKILL_DESC}\n---\n\n# Alpha\n\nBody of alpha.\n"),
        );
        t.write(
            "skills/omm-alpha/references/notes.md",
            "# Notes\n\nMore about alpha.\n",
        );
        t.write(
            "skills/omm-beta/SKILL.md",
            &format!("---\nname: omm-beta\ndescription: \"{SKILL2_DESC}\"\nmetadata:\n  short-description: Second fixture\n---\n\n# Beta\n"),
        );
        t.write(
            "commands/omm-cmd.md",
            "---\ndescription: Run the fixture command\nargument-hint: \"[thing]\"\n---\nDo the fixture thing with $ARGUMENTS.\n",
        );
        t.write(
            "hooks/omm-start.json",
            "{\n  \"id\": \"omm-start\",\n  \"event\": \"SessionStart\",\n  \"command\": [\"omm\", \"hook\", \"start\"],\n  \"timeoutMs\": 2000,\n  \"statusMessage\": \"omm: fixture\",\n  \"async\": false\n}\n",
        );
        t.write(
            "reminders/omm-nudge.md",
            "Decide whether to nudge. Default to none.\n",
        );
        let decision = reminder_decision();
        let reminder = json!({
            "id": "omm-nudge",
            "path": "reminders/omm-nudge.md",
            "enabledDefault": false,
            "tools": ["read_file"],
            "blocking": false,
            "decision": decision,
        });
        t.write("reminders/omm-nudge.json", &pretty(&reminder));
        t.write(
            "themes/omm-dark.tmTheme",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>name</key>\n  <string>omm Dark</string>\n  <key>settings</key>\n  <array></array>\n</dict>\n</plist>\n",
        );
        t.write(
            "profiles/omm-fast.json",
            "{\n  \"run\": {\n    \"context_slimming\": {\n      \"skill_catalog_descriptions\": \"first_sentence\",\n      \"full_skill_description_ids\": [\"bundled:git\"]\n    }\n  }\n}\n",
        );
        t.write(
            "rules/AGENTS.md.tmpl",
            "# Rules\n\n<!-- omm:managed-start -->\n- Run the check.\n<!-- omm:managed-end -->\n\n<!-- omm:user-start -->\n<!-- omm:user-end -->\n",
        );
        t.write(
            "translation/muse.md",
            "# Tool vocabulary\n\n| foreign | Muse |\n|---|---|\n| `Read` | `read_file` |\n",
        );
        t.write("README.md", "fixture docs\n");
        let mut hook_row = row("omm-start", "hook", "hooks/omm-start.json");
        hook_row["event"] = json!("SessionStart");
        let mut reminder_row = row("omm-nudge", "reminder", "reminders/omm-nudge.json");
        reminder_row["duty"] = json!("reminders/omm-nudge.md");
        reminder_row["enabled_default"] = json!(false);
        t.catalog = json!({
            "schema_version": 1,
            "plugin_id": "oh-my-musecode",
            "assets": [
                skill_row("omm-alpha", SKILL_DESC, None, true),
                skill_row("omm-beta", SKILL2_DESC, Some("Second fixture"), false),
                row("omm-cmd", "command", "commands/omm-cmd.md"),
                hook_row,
                reminder_row,
                row("omm-dark", "theme", "themes/omm-dark.tmTheme"),
                row("omm-fast", "profile", "profiles/omm-fast.json"),
                row("omm-agents-rules", "rules", "rules/AGENTS.md.tmpl"),
                row("omm-translation", "translation", "translation/muse.md"),
            ]
        });
        t.flush_catalog();
        t
    }

    pub fn repo(&self) -> Repo {
        Repo::new(&self.root).unwrap()
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.content.join(rel)
    }

    /// Write a content file (creating parents).
    pub fn write(&self, rel: &str, text: &str) {
        self.write_bytes(rel, text.as_bytes());
    }

    pub fn write_bytes(&self, rel: &str, bytes: &[u8]) {
        let p = self.path(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, bytes).unwrap();
    }

    /// Write a file relative to the repo root.
    pub fn write_repo_file(&self, rel: &str, text: &str) {
        let p = self.root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    pub fn remove(&self, rel: &str) {
        let p = self.path(rel);
        if p.is_dir() {
            std::fs::remove_dir_all(p).unwrap();
        } else {
            std::fs::remove_file(p).unwrap();
        }
    }

    /// Mutate the catalog document and rewrite it.
    pub fn edit_catalog(&mut self, f: impl FnOnce(&mut Value)) {
        f(&mut self.catalog);
        self.flush_catalog();
    }

    /// Mutate one asset row by id.
    pub fn edit_asset(&mut self, id: &str, f: impl FnOnce(&mut Value)) {
        self.edit_catalog(|c| {
            let row = c["assets"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|a| a["id"] == id)
                .unwrap();
            f(row);
        });
    }

    pub fn push_asset(&mut self, row: Value) {
        self.edit_catalog(|c| c["assets"].as_array_mut().unwrap().push(row));
    }

    pub fn flush_catalog(&self) {
        self.write("catalog.json", &pretty(&self.catalog));
    }

    /// Replace the catalog file with raw text (for malformed documents).
    pub fn raw_catalog(&self, text: &str) {
        self.write("catalog.json", text);
    }

    pub fn load(&self) -> (Catalog, Content) {
        let catalog = Catalog::load(&self.repo().catalog_path()).unwrap();
        let content = Content::load(&catalog, &self.content).unwrap();
        (catalog, content)
    }

    pub fn lint(&self) -> LintReport {
        lint::run(&self.repo(), &LintOptions::default()).unwrap()
    }

    pub fn lint_with_marketplace(&self) -> LintReport {
        lint::run(
            &self.repo(),
            &LintOptions {
                host: None,
                check_marketplace: true,
            },
        )
        .unwrap()
    }

    pub fn rules(&self) -> BTreeSet<String> {
        self.lint()
            .rules()
            .into_iter()
            .map(str::to_string)
            .collect()
    }

    /// The fixed digest golden/no-host builds use.
    pub fn fixed_digest() -> String {
        format!("sha256:{}", "0".repeat(64))
    }

    pub fn build(&self) -> omm_manifest::generate::BuildOutput {
        let (catalog, content) = self.load();
        omm_manifest::generate::build(
            &self.repo(),
            &catalog,
            &content,
            &omm_manifest::generate::BuildOptions {
                version: None,
                description: None,
                digest: omm_manifest::generate::DigestSource::Fixed(Tree::fixed_digest()),
            },
        )
        .unwrap()
    }
}

pub fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).unwrap();
    s.push('\n');
    s
}

pub fn row(id: &str, kind: &str, path: &str) -> Value {
    json!({
        "id": id, "kind": kind, "path": path, "lifecycle": "active", "core": false,
        "canonical": null, "since": "0.1.0", "sunset": null, "budget_bytes": 0
    })
}

pub fn skill_row(id: &str, description: &str, short: Option<&str>, core: bool) -> Value {
    let path = format!("skills/{id}/SKILL.md");
    let cost = entry_cost("oh-my-musecode", id, &path, description, short);
    json!({
        "id": id, "kind": "skill", "path": path, "lifecycle": "active", "core": core,
        "canonical": null, "since": "0.1.0", "sunset": null,
        "budget_bytes": cost.full, "budget_bytes_first_sentence": cost.first_sentence
    })
}

/// The bundled reminder decision block with an omm-owned envelope tag.
pub fn reminder_decision() -> Value {
    let mut d = hr::reminder_decision_fixture().unwrap().decision.clone();
    d["envelope"]["template"] = json!("<omm-reminder>\n{text}\n</omm-reminder>");
    d
}

/// Every file under a directory as `(relative path, bytes)`.
pub fn tree_files(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for e in walkdir_sorted(dir) {
        let rel = e.strip_prefix(dir).unwrap();
        let rel = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        out.push((rel, std::fs::read(&e).unwrap()));
    }
    out
}

fn walkdir_sorted(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !dir.exists() {
        return out;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&d)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}
