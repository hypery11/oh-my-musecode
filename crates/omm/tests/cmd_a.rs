//! Group A against the real binary (PLAN.md 1.5 e2e scenarios 1–4, 6, 7's
//! trust half, 10): a temp content checkout built with the host's digest, a
//! throwaway HOME/XDG sandbox, a temp git workspace as the cwd, the `omm`
//! binary run with `--json`. Skipped with a message when `OMM_MUSE_BIN` is
//! unset. Nothing touches the real config root; no login, no provider.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use tempfile::TempDir;

use omm_host::{Invoker, Sandbox};
use omm_manifest::catalog::Catalog;
use omm_manifest::content::Content;
use omm_manifest::generate::{self, BuildOptions, DigestSource};
use omm_manifest::Repo;

struct Harness {
    _tmp: TempDir,
    sb: Sandbox,
    inv: Invoker,
    bin: PathBuf,
    /// The temp checkout (`content/` + generated package + catalogs).
    source: PathBuf,
    /// A git workspace, the cwd of every run.
    ws: PathBuf,
}

fn host_bin() -> Option<PathBuf> {
    std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

macro_rules! harness_or_skip {
    () => {
        match Harness::new() {
            Some(h) => h,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

fn copy_tree(src: &Path, dest: &Path) {
    for entry in walkdir_files(src) {
        let rel = entry.strip_prefix(src).unwrap();
        let to = dest.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(&entry, &to).unwrap();
    }
}

fn walkdir_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let meta = std::fs::symlink_metadata(&p).unwrap();
            if meta.file_type().is_symlink() {
                continue;
            }
            if meta.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

impl Harness {
    fn new() -> Option<Harness> {
        let bin = host_bin()?;
        let tmp = tempfile::Builder::new()
            .prefix("omm-cmd-a-")
            .tempdir()
            .expect("temp dir");
        let sb = Sandbox::create(&tmp.path().join("sandbox")).expect("sandbox");
        let inv = Invoker::new(&bin).sandboxed(&sb);
        let ws = tmp.path().join("ws");
        std::fs::create_dir_all(ws.join(".git")).unwrap();
        let source = tmp.path().join("checkout");
        let real = Repo::from_cargo_manifest_dir().unwrap();
        copy_tree(&real.content_dir(), &source.join("content"));
        std::fs::create_dir_all(source.join("crates").join("omm")).unwrap();
        std::fs::copy(
            real.omm_cli_manifest(),
            source.join("crates").join("omm").join("Cargo.toml"),
        )
        .unwrap();
        // The workspace-inherited version resolves through the root Cargo.toml.
        std::fs::copy(real.root.join("Cargo.toml"), source.join("Cargo.toml")).unwrap();
        let h = Harness {
            _tmp: tmp,
            sb,
            inv,
            bin,
            source,
            ws,
        };
        h.rebuild_source();
        Some(h)
    }

    /// Regenerate the package and catalogs of the temp checkout (the
    /// digest from the binary), as `omm build` does.
    fn rebuild_source(&self) {
        self.rebuild_at(&self.source);
    }

    /// The same for any checkout (a second copy under the temp dir).
    fn rebuild_at(&self, checkout: &Path) {
        let repo = Repo::new(checkout).unwrap();
        let catalog = Catalog::load(&repo.catalog_path()).unwrap();
        let content = Content::load(&catalog, &repo.content_dir()).unwrap();
        let out = generate::build(
            &repo,
            &catalog,
            &content,
            &BuildOptions {
                version: None,
                description: None,
                digest: DigestSource::Host(&self.inv),
            },
        )
        .unwrap();
        generate::write(&repo, &out).unwrap();
    }

    fn config_root(&self) -> PathBuf {
        self.sb.config_home.join("muse")
    }
    fn omm_root(&self) -> PathBuf {
        self.sb.config_home.join("omm")
    }

    /// A second checkout: the temp source copied to `<tmp>/<name>`, built.
    fn second_checkout(&self, name: &str) -> PathBuf {
        let copy = self._tmp.path().join(name);
        copy_tree(&self.source.join("content"), &copy.join("content"));
        std::fs::create_dir_all(copy.join("crates").join("omm")).unwrap();
        std::fs::copy(
            self.source.join("crates").join("omm").join("Cargo.toml"),
            copy.join("crates").join("omm").join("Cargo.toml"),
        )
        .unwrap();
        std::fs::copy(self.source.join("Cargo.toml"), copy.join("Cargo.toml")).unwrap();
        self.rebuild_at(&copy);
        copy
    }

    /// `muse <argv>` in the sandbox, exit 0 required; the parsed JSON.
    fn muse_json(&self, argv: &[&str]) -> Value {
        let out = self.inv.run(argv).expect("run muse");
        assert!(out.ok(), "muse {argv:?}: {} {}", out.stdout, out.stderr);
        out.first_json().unwrap_or(Value::Null)
    }

    /// Skill ids the host lists for `--source user`.
    fn user_skills(&self) -> Vec<String> {
        let list = omm_host::probe::skills_list(
            &self.inv,
            &omm_host::probe::SkillsListOptions {
                source: Some("user".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let mut ids: Vec<String> = list.skills.into_iter().map(|s| s.id).collect();
        ids.sort();
        ids
    }

    /// The digest the checkout's committed `marketplace.json` carries for the plugin.
    fn committed_digest(checkout: &Path) -> String {
        let v: Value =
            serde_json::from_slice(&std::fs::read(checkout.join("marketplace.json")).unwrap())
                .unwrap();
        v["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == "oh-my-musecode")
            .and_then(|p| p["integrity"]["digest"].as_str())
            .unwrap()
            .to_string()
    }

    /// Run `omm <args> --json --yes` in the sandbox: `(exit code, stdout JSON, stderr)`.
    fn omm(&self, args: &[&str]) -> (i32, Value, String) {
        self.omm_env(args, &[])
    }

    fn omm_env(&self, args: &[&str], extra_env: &[(&str, &str)]) -> (i32, Value, String) {
        // The binary's own directory first on PATH: the host spawns
        // `omm hook …` / `omm mcp` through the PATH it inherits, and doctor
        // D15 is critical when that PATH holds no `omm`.
        let bin = Path::new(env!("CARGO_BIN_EXE_omm"));
        let mut dirs: Vec<PathBuf> = bin.parent().map(Path::to_path_buf).into_iter().collect();
        if let Some(p) = std::env::var_os("PATH") {
            dirs.extend(std::env::split_paths(&p));
        }
        let path = std::env::join_paths(dirs).unwrap_or_default();
        let mut cmd = Command::new(bin);
        cmd.args(args)
            .arg("--json")
            .env_clear()
            .env("PATH", path)
            .env("HOME", &self.sb.home)
            .env("XDG_CONFIG_HOME", &self.sb.config_home)
            .env("XDG_DATA_HOME", &self.sb.data_home)
            .env("OMM_MUSE_BIN", &self.bin)
            .env("OMM_SOURCE", &self.source)
            .env("TMPDIR", std::env::temp_dir())
            .current_dir(&self.ws)
            .stdin(Stdio::null());
        if !extra_env.iter().any(|(k, _)| *k == "CI") {
            cmd.arg("--yes");
        }
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        let out = cmd.output().expect("run omm");
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let json = serde_json::from_str::<Value>(stdout.trim()).unwrap_or(Value::Null);
        (out.status.code().unwrap_or(-1), json, stderr)
    }

    fn ledger(&self) -> Option<Value> {
        std::fs::read(self.omm_root().join("omm.lock.json"))
            .ok()
            .map(|b| serde_json::from_slice(&b).unwrap())
    }

    fn inspect(&self) -> Result<omm_host::probe::PluginInspect, omm_host::HostError> {
        omm_host::probe::plugins_inspect(&self.inv, "oh-my-musecode")
    }

    /// `rel path -> sha256` of every regular file under the config root,
    /// minus the host's own residue (startup locks, the managed-store
    /// metadata, the bootstrap traces).
    fn config_snapshot(&self) -> BTreeMap<String, String> {
        let root = self.sb.config_home.clone();
        let mut out = BTreeMap::new();
        for f in walkdir_files(&root) {
            let rel = f
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if rel.ends_with(".lock") || rel.contains("skills/.muse/") {
                continue;
            }
            out.insert(rel, omm_host::fsx::sha256_file(&f).unwrap());
        }
        out
    }
}

fn converge(doc: &Value) -> &Value {
    &doc["converge"]
}

#[test]
fn bundle_install_is_idempotent_and_uninstall_restores_the_config_root() {
    let h = harness_or_skip!();
    let before = h.config_snapshot();
    assert!(before.is_empty(), "{before:?}");

    // 1. install
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["command"], "install");
    assert_eq!(doc["mode"], "plugin");
    assert_eq!(doc["bundle"]["plugin"], "updated");
    assert_eq!(doc["bundle"]["marketplace"]["action"], "updated");
    assert!(doc["bundle"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let trusted = doc["bundle"]["trusted"].as_array().unwrap();
    assert!(trusted.len() >= 3, "{trusted:?}");
    assert!(trusted
        .iter()
        .all(|t| t.as_str().unwrap().starts_with("plugin:oh-my-musecode:")));
    // The catalog ships one reminder off by design: never approved.
    let by_design = doc["bundle"]["unapproved_by_design"].as_array().unwrap();
    assert!(
        by_design
            .iter()
            .any(|s| s.as_str().unwrap().contains(":reminder:")),
        "{doc}"
    );
    assert_eq!(doc["bundle"]["skills_missing"], serde_json::json!([]));
    assert!(doc["bundle"]["skills_listed"].as_u64().unwrap() >= 10);
    assert_eq!(doc["rules"]["action"], "updated");
    assert_eq!(doc["themes"]["written"].as_array().unwrap().len(), 3);
    assert!(
        doc["settings"]["set"].as_array().unwrap().len() >= 3,
        "{doc}"
    );
    assert_eq!(doc["trust"]["action"], "updated");
    assert_eq!(doc["budget"]["tokens_heuristic"], "bytes/4");
    assert_eq!(doc["budget"]["within_budget"], true);
    assert_eq!(converge(&doc)["noop"], false);
    assert_eq!(converge(&doc)["categories"]["plugin"]["updated"], 1);

    // The host agrees: every trusted line literally trusted_enabled, the
    // skills visible, the settings keys landed, trust.json holds the ws.
    let ins = h.inspect().unwrap();
    assert_eq!(ins.manifest_family.as_deref(), Some("native"));
    for id in trusted {
        assert!(ins.is_trusted_enabled(id.as_str().unwrap()), "{id}");
    }
    let settings: Value =
        serde_json::from_slice(&std::fs::read(h.config_root().join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        settings["run"]["context_slimming"]["skill_catalog_descriptions"],
        "first_sentence"
    );
    let trust: Value =
        serde_json::from_slice(&std::fs::read(h.config_root().join("trust.json")).unwrap())
            .unwrap();
    let ws_key = std::fs::canonicalize(&h.ws).unwrap();
    assert_eq!(
        trust["projects"][ws_key.to_string_lossy().as_ref()]["decision"],
        "trusted"
    );
    let agents = std::fs::read_to_string(h.config_root().join("AGENTS.md")).unwrap();
    assert!(
        agents.contains("<!-- omm:managed-start -->") && agents.contains("<!-- omm:user-end -->")
    );
    assert!(h.config_root().join("themes").read_dir().unwrap().count() == 3);
    let ledger = h.ledger().expect("ledger written");
    let regs = ledger["registrations"].as_array().unwrap();
    assert!(regs
        .iter()
        .any(|r| r["kind"] == "muse-marketplace" && r["name"] == "omm"));
    let plugin_reg = regs.iter().find(|r| r["kind"] == "muse-plugin").unwrap();
    assert_eq!(plugin_reg["package_sha256"], doc["bundle"]["digest"]);
    assert_eq!(
        plugin_reg["approved"].as_array().unwrap().len(),
        trusted.len()
    );
    assert!(regs.iter().any(|r| r["kind"] == "settings-key"));
    assert!(regs.iter().any(|r| r["kind"] == "trust"));
    let entries = ledger["entries"].as_array().unwrap();
    assert!(entries
        .iter()
        .any(|e| e["path"] == "AGENTS.md" && e["class"] == "seeded"));
    assert!(entries
        .iter()
        .any(|e| e["path"] == "settings.json" && e["class"] == "seeded"));
    assert!(entries
        .iter()
        .any(|e| e["path"] == "trust.json" && e["class"] == "seeded"));
    assert_eq!(entries.iter().filter(|e| e["kind"] == "theme").count(), 3);
    assert!(h.omm_root().join("audit.log").exists());
    assert!(
        h.sb.home.read_dir().unwrap().next().is_none(),
        "nothing lands in $HOME"
    );
    // The settings-patch audit lines carry the hash of the file as the
    // profile step found it (the host had created it during the approvals;
    // Gate 1: `sha256_before: null` although the file existed).
    let audit = std::fs::read_to_string(h.omm_root().join("audit.log")).unwrap();
    let patches: Vec<Value> = audit
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|l| {
            l["note"]
                .as_str()
                .map(|n| n.starts_with("settings-patch "))
                .unwrap_or(false)
        })
        .collect();
    assert!(patches.len() >= 3, "{audit}");
    for p in &patches {
        assert!(p["sha256_before"].is_string(), "{p}");
        assert!(p["sha256_after"].is_string(), "{p}");
    }

    // 2. install again: everything unchanged, the ledger byte-stable.
    let ledger_bytes = std::fs::read(h.omm_root().join("omm.lock.json")).unwrap();
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(converge(&doc)["noop"], true, "{doc}");
    assert_eq!(doc["bundle"]["plugin"], "unchanged");
    assert_eq!(doc["bundle"]["approved"], serde_json::json!([]));
    assert_eq!(doc["rules"]["action"], "unchanged");
    assert_eq!(doc["settings"]["set"], serde_json::json!([]));
    assert_eq!(doc["trust"]["action"], "unchanged");
    assert_eq!(
        std::fs::read(h.omm_root().join("omm.lock.json")).unwrap(),
        ledger_bytes
    );

    // 3. a dry-run uninstall previews and touches nothing.
    let (rc, doc, err) = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["dry_run"], true);
    let steps = doc["preview"]["host_steps"].as_array().unwrap();
    assert!(steps.iter().any(|s| s
        .as_str()
        .unwrap()
        .contains("plugins remove oh-my-musecode --delete-data")));
    assert!(steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("marketplace remove omm")));
    assert!(steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("settings key")));
    assert!(steps
        .iter()
        .any(|s| s.as_str().unwrap().contains("trust entry")));
    let residue = doc["preview"]["residue"].as_array().unwrap();
    assert!(
        residue
            .iter()
            .any(|r| r.as_str().unwrap().contains("session-name-authority")),
        "{residue:?}"
    );
    assert!(h.ledger().is_some());
    assert!(h.inspect().is_ok());

    // 4. uninstall: host registrations undone, files gone, ledger last,
    //    the config root as found (modulo the host's own lock files).
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["report"]["errors"], serde_json::json!([]));
    assert_eq!(doc["report"]["ledger_removed"], true);
    assert!(h.ledger().is_none());
    assert!(!h.omm_root().join("audit.log").exists());
    assert!(h.inspect().is_err(), "the plugin is removed");
    assert!(!h.config_root().join("AGENTS.md").exists());
    assert!(!h.config_root().join("themes").exists());
    assert!(
        !h.config_root().join("trust.json").exists(),
        "seeded trust.json unlinked"
    );
    assert!(
        !h.config_root().join("settings.json").exists(),
        "seeded settings.json unlinked"
    );
    let mkt =
        omm_host::Invoker::run(&h.inv, &["plugins", "marketplace", "list", "--json"]).unwrap();
    assert!(!mkt.stdout.contains("\"ohmy\""), "{}", mkt.stdout);
    assert_eq!(h.config_snapshot(), before);
}

#[test]
fn uninstall_without_a_ledger_refuses_host_registrations_unless_reconcile_host() {
    // Gate 1: an install interrupted after `plugins install` and before its
    // ledger left `oh-my-musecode` and `omm` on the host, and `omm uninstall`
    // said "nothing to uninstall". The host state of that moment, by hand:
    let h = harness_or_skip!();
    let src = h.source.to_string_lossy().into_owned();
    h.inv
        .run(&["plugins", "marketplace", "add", "omm", &src, "--json"])
        .expect("marketplace add")
        .expect_ok()
        .expect("marketplace add ok");
    h.inv
        .run(&["plugins", "install", "oh-my-musecode@omm", "--json"])
        .expect("plugins install")
        .expect_ok()
        .expect("plugins install ok");
    assert!(h.inspect().is_ok());
    assert!(!h.omm_root().exists());
    let marketplaces = |h: &Harness| -> Vec<String> {
        let out = h
            .inv
            .run_json(&["plugins", "marketplace", "list", "--json"])
            .expect("marketplace list");
        out.json["marketplaces"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(marketplaces(&h), vec!["omm".to_string()]);

    // Without the flag: refused (exit 1), both recovery paths named, nothing touched.
    let (code, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(code, 1, "{err}");
    assert!(doc.is_null(), "{doc}");
    assert!(err.contains("--reconcile-host"), "{err}");
    assert!(err.contains("omm reconcile"), "{err}");
    assert!(h.inspect().is_ok(), "nothing removed without the flag");
    assert_eq!(marketplaces(&h), vec!["omm".to_string()]);
    assert!(!h.omm_root().exists());

    // A dry run names the two host steps and removes nothing.
    let (code, doc, err) = h.omm(&["uninstall", "--reconcile-host", "--dry-run"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["dry_run"], true);
    let steps = doc["host_reconciled"].as_array().expect("host_reconciled");
    assert_eq!(steps.len(), 2, "{doc}");
    assert!(h.inspect().is_ok());
    assert_eq!(marketplaces(&h), vec!["omm".to_string()]);

    // With the flag: both registrations gone, said so, host clean, no $OMM.
    let (code, doc, err) = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(code, 0, "{err}");
    let steps: Vec<&str> = doc["host_reconciled"]
        .as_array()
        .expect("host_reconciled")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(steps.len(), 3, "{doc}");
    assert_eq!(
        &steps[..2],
        &[
            "muse plugins remove oh-my-musecode --delete-data",
            "muse plugins marketplace remove omm"
        ]
    );
    // `plugins remove --delete-data` wrote `{"schema_version":1}` into a
    // config root that had no settings.json: removed again, said so.
    assert!(
        steps[2].starts_with("removed ") && steps[2].contains("settings.json"),
        "{doc}"
    );
    assert!(
        !h.config_root().join("settings.json").exists(),
        "settings.json left behind by the host's remove"
    );
    assert_eq!(doc["host"]["plugin"], true);
    assert_eq!(doc["host"]["marketplace"], true);
    assert!(h.inspect().is_err(), "the plugin is gone");
    assert!(marketplaces(&h).is_empty(), "the marketplace is gone");
    assert!(!h.omm_root().exists());
    // Nothing left to reconcile: the plain path reports nothing to do, exit 0.
    let (code, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["host"]["plugin"], false);
    assert_eq!(doc["host_reconciled"], serde_json::json!([]));
}

#[test]
fn no_plugin_install_uses_the_managed_store_and_uninstalls_through_it() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install", "--no-plugin"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["mode"], "skills");
    let installed = doc["skills"]["installed"].as_array().unwrap();
    assert!(installed.len() >= 10, "{doc}");
    let store = h.config_root().join("skills");
    assert!(store
        .join(installed[0].as_str().unwrap())
        .join("SKILL.md")
        .exists());
    let ledger = h.ledger().unwrap();
    let skill_entries: Vec<&Value> = ledger["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|e| e["mechanism"] == "muse-skills-install")
        .collect();
    assert!(skill_entries.len() > installed.len(), "one entry per file");
    let list = omm_host::probe::skills_list(
        &h.inv,
        &omm_host::probe::SkillsListOptions {
            source: Some("user".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(list.skills.len(), installed.len());
    // Mixing modes is refused.
    let (rc, _, err) = h.omm(&["install"]);
    assert_eq!(rc, 2, "{err}");
    assert!(err.contains("--no-plugin"), "{err}");
    // Idempotent.
    let (rc, doc, err) = h.omm(&["install", "--no-plugin"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(converge(&doc)["noop"], true, "{doc}");
    // Uninstall goes through `muse skills uninstall`; the store's metadata
    // is the host's and is named as residue.
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let steps = doc["preview"]["host_steps"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .filter(|s| s.as_str().unwrap().starts_with("muse skills uninstall"))
            .count()
            >= 10
    );
    assert!(doc["preview"]["residue"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r.as_str().unwrap().ends_with("skills/.muse")));
    assert_eq!(doc["report"]["errors"], serde_json::json!([]));
    assert!(list.skills.iter().all(|s| !store.join(&s.id).exists()));
    assert!(h.ledger().is_none());
    assert!(h.config_snapshot().is_empty(), "{:?}", h.config_snapshot());
}

#[test]
fn update_stages_user_edits_preserves_the_user_region_and_reinstalls_a_changed_bundle() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let old_digest = doc["bundle"]["digest"].as_str().unwrap().to_string();

    // The user edits one theme and writes into the rules' user region.
    let themes = h.config_root().join("themes");
    let mut names: Vec<PathBuf> = themes
        .read_dir()
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    let edited_theme = names[0].clone();
    std::fs::write(&edited_theme, b"<!-- my theme -->\n").unwrap();
    let agents_path = h.config_root().join("AGENTS.md");
    let agents = std::fs::read_to_string(&agents_path).unwrap();
    let with_rule = agents.replace(
        "<!-- omm:user-end -->",
        "- my own rule\n<!-- omm:user-end -->",
    );
    std::fs::write(&agents_path, &with_rule).unwrap();

    // Upstream changes every theme, the managed rules and a skill body.
    for t in walkdir_files(&h.source.join("content").join("themes")) {
        let mut text = std::fs::read_to_string(&t).unwrap();
        text.push_str("<!-- v2 -->\n");
        std::fs::write(&t, text).unwrap();
    }
    let tmpl = h
        .source
        .join("content")
        .join("rules")
        .join("AGENTS.md.tmpl");
    let text = std::fs::read_to_string(&tmpl).unwrap();
    std::fs::write(
        &tmpl,
        text.replace(
            "<!-- omm:managed-end -->",
            "- a new managed line\n<!-- omm:managed-end -->",
        ),
    )
    .unwrap();
    let skill = h
        .source
        .join("content")
        .join("skills")
        .join("omm-self")
        .join("SKILL.md");
    let mut text = std::fs::read_to_string(&skill).unwrap();
    text.push_str("\nMore body.\n");
    std::fs::write(&skill, text).unwrap();
    h.rebuild_source();

    // Dry run first: nothing written, the plan visible.
    let (rc, doc, err) = h.omm(&["update", "--dry-run"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["dry_run"], true);
    assert_eq!(doc["rules"]["action"], "updated");
    assert_eq!(std::fs::read_to_string(&agents_path).unwrap(), with_rule);
    assert!(!h.omm_root().join("updates").exists());

    let (rc, doc, err) = h.omm(&["update"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    // The edited theme is STAGED, on disk untouched, named in the report.
    let staged = doc["staged"].as_array().unwrap();
    assert_eq!(staged.len(), 1, "{doc}");
    let staged_rel = staged[0]["path"].as_str().unwrap();
    assert!(
        edited_theme.ends_with(staged_rel),
        "{staged_rel} vs {}",
        edited_theme.display()
    );
    assert_eq!(
        std::fs::read(&edited_theme).unwrap(),
        b"<!-- my theme -->\n"
    );
    let staged_to = PathBuf::from(staged[0]["staged_to"].as_str().unwrap());
    assert!(staged_to.exists());
    assert!(String::from_utf8_lossy(&std::fs::read(&staged_to).unwrap()).contains("<!-- v2 -->"));
    let report: Value = serde_json::from_slice(
        &std::fs::read(
            staged_to
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("report.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(report["staged"].as_array().unwrap().len(), 1);
    // The other two themes were overwritten.
    for t in &names[1..] {
        assert!(String::from_utf8_lossy(&std::fs::read(t).unwrap()).contains("<!-- v2 -->"));
    }
    // Rules: managed region updated, the user's line preserved.
    assert_eq!(doc["rules"]["action"], "updated", "{doc}");
    let agents = std::fs::read_to_string(&agents_path).unwrap();
    assert!(agents.contains("- a new managed line"));
    assert!(agents.contains("- my own rule"));
    // The bundle: digest changed → remove → install → approve → verify.
    assert_eq!(doc["bundle"]["plugin"], "updated", "{doc}");
    let new_digest = doc["bundle"]["digest"].as_str().unwrap();
    assert_ne!(new_digest, old_digest);
    let ins = h.inspect().unwrap();
    for id in doc["bundle"]["trusted"].as_array().unwrap() {
        assert!(ins.is_trusted_enabled(id.as_str().unwrap()), "{id}");
    }
    let ledger = h.ledger().unwrap();
    let reg = ledger["registrations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["kind"] == "muse-plugin")
        .unwrap();
    assert_eq!(reg["package_sha256"], new_digest);
    assert!(h.omm_root().join("snapshots").read_dir().unwrap().count() >= 1);
    // A second update is a no-op apart from the standing conflict.
    let (rc, doc, err) = h.omm(&["update"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["bundle"]["plugin"], "unchanged");
    assert_eq!(doc["rules"]["action"], "unchanged");
    assert_eq!(doc["staged"].as_array().unwrap().len(), 1);
    assert_eq!(converge(&doc)["total"]["updated"], 0, "{doc}");
}

#[test]
fn uninstall_preserves_an_edited_file_unless_forced() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let theme = std::fs::canonicalize(
        h.config_root()
            .join("themes")
            .read_dir()
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap();
    std::fs::write(&theme, b"edited").unwrap();
    // The preview lists the edit under preserve; a dry run changes nothing.
    let (rc, doc, err) = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let preserved = doc["preview"]["preserve"].as_array().unwrap();
    assert!(
        preserved
            .iter()
            .any(|p| p["path"].as_str() == Some(theme.to_str().unwrap())),
        "{doc}"
    );
    assert!(h.ledger().is_some());
    // --force removes it (never a symlink or directory at a ledgered path).
    let (rc, doc, err) = h.omm(&["uninstall", "--force"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert!(doc["preview"]["remove"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["path"].as_str() == Some(theme.to_str().unwrap()) && r["forced"] == true));
    assert!(!theme.exists());
    assert!(!h.config_root().join("themes").exists(), "{doc}");
    assert!(h.ledger().is_none());
    // Without --force the edit survives and the ledger still goes: an
    // uninstall that preserved something has still uninstalled.
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    std::fs::write(&theme, b"edited again").unwrap();
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["report"]["preserved"], 1, "{doc}");
    assert_eq!(std::fs::read(&theme).unwrap(), b"edited again");
    assert!(h.ledger().is_none());
    // A reinstall never overwrites the unledgered edit: it is skipped.
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert!(
        doc["themes"]["skipped"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["path"].as_str() == Some(theme.to_str().unwrap())),
        "{doc}"
    );
    assert_eq!(std::fs::read(&theme).unwrap(), b"edited again");
}

#[test]
fn install_refuses_under_ci_without_yes_and_writes_nothing() {
    let h = harness_or_skip!();
    let (rc, _, err) = h.omm_env(&["install"], &[("CI", "true")]);
    assert_eq!(rc, 1, "{err}");
    assert!(err.contains("--yes"), "{err}");
    assert!(!h.omm_root().exists());
    assert!(h.config_snapshot().is_empty());
    assert!(h.inspect().is_err());
    // --dry-run never needs consent and writes nothing either.
    let (rc, doc, err) = h.omm_env(&["install", "--dry-run"], &[("CI", "true")]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["dry_run"], true);
    assert!(
        !h.omm_root().exists(),
        "a dry run creates nothing under $OMM"
    );
    assert!(h.config_snapshot().is_empty());
    assert!(h.inspect().is_err());
}

#[test]
fn install_refuses_to_touch_settings_with_the_mcp_key_collision() {
    let h = harness_or_skip!();
    std::fs::create_dir_all(h.config_root()).unwrap();
    std::fs::write(
        h.config_root().join("settings.json"),
        b"{\"schema_version\":1,\"mcpServers\":{},\"mcp_servers\":{}}\n",
    )
    .unwrap();
    let before = h.config_snapshot();
    let (rc, _, err) = h.omm(&["install"]);
    assert_eq!(rc, 1, "{err}");
    assert!(err.contains("fix-mcp-collision"), "{err}");
    assert_eq!(h.config_snapshot(), before);
    assert!(!h.omm_root().exists());
    assert!(h.inspect().is_err());
}

#[test]
fn trust_list_and_reconcile() {
    let h = harness_or_skip!();
    // trust alone creates a ledger with the registration and its prior.
    let (rc, doc, err) = h.omm(&["trust", h.ws.to_str().unwrap()]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["trust"]["action"], "updated");
    assert_eq!(doc["trust"]["prior"], Value::Null);
    let ledger = h.ledger().unwrap();
    assert!(ledger["registrations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "trust"));
    let (rc, doc, _) = h.omm(&["trust", h.ws.to_str().unwrap()]);
    assert_eq!(rc, 0);
    assert_eq!(doc["trust"]["action"], "unchanged");
    // list: bundled provenance, then an overlay shadows one skill.
    let (rc, doc, err) = h.omm(&["list"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let items = doc["items"].as_array().unwrap();
    assert!(items.len() >= 20, "{doc}");
    assert!(items
        .iter()
        .all(|i| i["provider"] == "bundled" && i["_shadowed"].as_array().unwrap().is_empty()));
    let custom = h.omm_root().join("custom").join("skills").join("omm-self");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(
        custom.join("SKILL.md"),
        "---\nname: omm-self\ndescription: mine\n---\n",
    )
    .unwrap();
    std::fs::write(
        h.omm_root().join("config.json"),
        b"{\"disabled\":[\"skill:omm-tdd\"]}",
    )
    .unwrap();
    let (rc, doc, err) = h.omm(&["list", "--kind", "skill"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let items = doc["items"].as_array().unwrap();
    let mine = items.iter().find(|i| i["id"] == "omm-self").unwrap();
    assert_eq!(mine["provider"], "custom");
    assert_eq!(mine["level"], "user");
    assert_eq!(mine["_shadowed"][0]["provider"], "bundled");
    assert_eq!(
        items.iter().find(|i| i["id"] == "omm-tdd").unwrap()["disabled"],
        true
    );
    assert!(items.iter().all(|i| i["kind"] == "skill"));
    // reconcile after the host lost the plugin and a theme vanished.
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    omm_host::Invoker::run(
        &h.inv,
        &[
            "plugins",
            "remove",
            "oh-my-musecode",
            "--delete-data",
            "--json",
        ],
    )
    .unwrap();
    let theme = h
        .config_root()
        .join("themes")
        .read_dir()
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::remove_file(&theme).unwrap();
    let (rc, doc, err) = h.omm(&["reconcile"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let notes = doc["notes"].as_array().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.as_str().unwrap().contains("registration dropped")),
        "{doc}"
    );
    assert!(
        notes
            .iter()
            .any(|n| n.as_str().unwrap().contains("vanished")),
        "{doc}"
    );
    let ledger = h.ledger().unwrap();
    assert!(!ledger["registrations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["kind"] == "muse-plugin"));
    assert_eq!(
        ledger["entries"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "theme")
            .count(),
        2
    );
    // A fresh install after that heals the registration.
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["bundle"]["plugin"], "updated");
}

#[test]
fn no_plugin_update_refreshes_untouched_skills_and_stages_an_edited_one() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install", "--no-plugin"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let store = h.config_root().join("skills");
    // The user edits one skill file; upstream changes that skill and another.
    let edited = store.join("omm-commit").join("SKILL.md");
    let mut text = std::fs::read_to_string(&edited).unwrap();
    text.push_str("\nmy note\n");
    std::fs::write(&edited, &text).unwrap();
    for id in ["omm-commit", "omm-debug"] {
        let p = h
            .source
            .join("content")
            .join("skills")
            .join(id)
            .join("SKILL.md");
        let mut t = std::fs::read_to_string(&p).unwrap();
        t.push_str("\nv2 body.\n");
        std::fs::write(&p, t).unwrap();
    }
    h.rebuild_source();
    // The user's settings.json holds a key the host does not type, at 0600;
    // the update's `muse skills uninstall` rewrites the file without the key
    // (Gate 1) — the key comes back after the last host step. Since
    // 1.3.0-R3057.1 the host rewrite preserves the mode bits, so there is no
    // mode to re-apply (measured: 0600 in, 0600 out); on 1.0.x the rewrite
    // landed 0644 and `mode_reapplied` was true.
    {
        use std::os::unix::fs::PermissionsExt;
        let settings = h.config_root().join("settings.json");
        let mut v: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
        v["my_custom_top_level"] = serde_json::json!({"keep": true});
        std::fs::write(&settings, serde_json::to_vec_pretty(&v).unwrap()).unwrap();
        std::fs::set_permissions(&settings, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let (rc, doc, err) = h.omm(&["update"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    {
        use std::os::unix::fs::PermissionsExt;
        let settings = h.config_root().join("settings.json");
        let v: Value = serde_json::from_slice(&std::fs::read(&settings).unwrap()).unwrap();
        assert_eq!(v["my_custom_top_level"]["keep"], true, "{v}");
        assert_eq!(
            std::fs::metadata(&settings).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            doc["settings"]["keys_restored"],
            serde_json::json!(["my_custom_top_level"]),
            "{doc}"
        );
        assert_eq!(doc["settings"]["mode_reapplied"], false, "{doc}");
    }
    let skills = &doc["skills"];
    assert_eq!(skills["updated"], serde_json::json!(["omm-debug"]), "{doc}");
    assert!(skills["skipped"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s["id"] == "omm-commit"));
    let staged = doc["staged"].as_array().unwrap();
    assert_eq!(staged.len(), 1, "{doc}");
    assert_eq!(staged[0]["path"], "skills/omm-commit/SKILL.md");
    // The edited skill is kept whole; the other one carries the new body
    // through the host's store (its lockfile knows it).
    assert_eq!(std::fs::read_to_string(&edited).unwrap(), text);
    let debug = std::fs::read_to_string(store.join("omm-debug").join("SKILL.md")).unwrap();
    assert!(debug.ends_with("v2 body.\n"));
    let lock: Value =
        serde_json::from_slice(&std::fs::read(store.join(".muse").join("lock.json")).unwrap())
            .unwrap();
    assert!(lock["skills"]["omm-debug"].is_object());
    let ledger = h.ledger().unwrap();
    let e = ledger["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["path"] == "skills/omm-debug/SKILL.md")
        .unwrap();
    assert_eq!(e["writer"], "omm update");
    assert_eq!(e["sha256"], omm_host::fsx::sha256_bytes(debug.as_bytes()));
    // Again: only the standing conflict.
    let (rc, doc, err) = h.omm(&["update"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["skills"]["updated"], serde_json::json!([]));
    assert_eq!(doc["staged"].as_array().unwrap().len(), 1);
    // Uninstall keeps the edited skill whole and removes the rest.
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert!(edited.exists());
    assert!(!store.join("omm-debug").exists());
    assert_eq!(doc["report"]["errors"], serde_json::json!([]));
}

// ---------------------------------------------------------------------------
// Gate 1 fix loop: the findings below each had a failing run before the fix.
// ---------------------------------------------------------------------------

/// Gate 1: an `omm install --no-plugin` interrupted after a few `muse skills
/// install` calls left the skills in the managed store with no ledger, and
/// `omm uninstall` (with or without `--reconcile-host`) reported nothing to
/// do. The host state of that moment, by hand: two catalog skills installed
/// from the checkout, plus one of the user's own in the same store.
#[test]
fn uninstall_without_a_ledger_reconciles_managed_store_skills_only_with_the_flag() {
    let h = harness_or_skip!();
    for id in ["omm-commit", "omm-debug"] {
        let dir = h.source.join("content").join("skills").join(id);
        h.muse_json(&[
            "skills",
            "install",
            dir.to_str().unwrap(),
            "--scope",
            "user",
            "--json",
        ]);
    }
    let mine = h._tmp.path().join("my-own-skill");
    std::fs::create_dir_all(&mine).unwrap();
    std::fs::write(
        mine.join("SKILL.md"),
        "---\nname: my-own-skill\ndescription: The user's own managed skill; never omm's.\n---\n\nBody.\n",
    )
    .unwrap();
    h.muse_json(&[
        "skills",
        "install",
        mine.to_str().unwrap(),
        "--scope",
        "user",
        "--json",
    ]);
    assert_eq!(
        h.user_skills(),
        vec!["my-own-skill", "omm-commit", "omm-debug"]
    );
    assert!(!h.omm_root().exists());

    // Without the flag: refused, the two skills named, nothing touched.
    let (code, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(code, 1, "{doc}\n{err}");
    assert!(err.contains("--reconcile-host"), "{err}");
    assert!(
        err.contains("omm-commit") && err.contains("omm-debug"),
        "{err}"
    );
    assert!(!err.contains("my-own-skill"), "{err}");
    assert_eq!(
        h.user_skills(),
        vec!["my-own-skill", "omm-commit", "omm-debug"]
    );
    // A dry run names the two host steps and removes nothing.
    let (code, doc, err) = h.omm(&["uninstall", "--reconcile-host", "--dry-run"]);
    assert_eq!(code, 0, "{doc}\n{err}");
    let steps: Vec<&str> = doc["host_reconciled"]
        .as_array()
        .expect("host_reconciled")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        steps,
        vec![
            "muse skills uninstall omm-commit",
            "muse skills uninstall omm-debug"
        ],
        "{doc}"
    );
    assert_eq!(
        h.user_skills(),
        vec!["my-own-skill", "omm-commit", "omm-debug"]
    );
    // With the flag: the two omm skills go through `muse skills uninstall`,
    // the user's own stays, and it is all said so.
    let (code, doc, err) = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(code, 0, "{doc}\n{err}");
    let steps: Vec<&str> = doc["host_reconciled"]
        .as_array()
        .expect("host_reconciled")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        steps,
        vec![
            "muse skills uninstall omm-commit",
            "muse skills uninstall omm-debug"
        ]
    );
    assert_eq!(
        doc["host"]["skills"],
        serde_json::json!(["omm-commit", "omm-debug"])
    );
    assert_eq!(h.user_skills(), vec!["my-own-skill"]);
    assert!(!h.config_root().join("skills").join("omm-commit").exists());
    assert!(h.config_root().join("skills").join("my-own-skill").exists());
    assert!(!h.omm_root().exists());
    // Nothing left to reconcile: the plain path reports nothing to do.
    let (code, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(code, 0, "{err}");
    assert_eq!(doc["host_reconciled"], serde_json::json!([]));
    assert_eq!(doc["host"]["skills"], serde_json::json!([]));
}

/// Gate 1: `omm update --source <checkout>` refreshed the marketplace
/// registered at install and left the plugin at the old digest while the
/// rules, themes and skills came from the new checkout.
#[test]
fn update_with_a_different_source_reinstalls_the_plugin_from_that_checkout() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let old_digest = doc["bundle"]["digest"].as_str().unwrap().to_string();
    let copy = h.second_checkout("checkout2");
    // The copy differs in one skill body, so its package digest differs.
    let skill = copy
        .join("content")
        .join("skills")
        .join("omm-tdd")
        .join("SKILL.md");
    let mut text = std::fs::read_to_string(&skill).unwrap();
    text.push_str("\nBumped in the second checkout.\n");
    std::fs::write(&skill, text).unwrap();
    h.rebuild_at(&copy);
    let new_digest = Harness::committed_digest(&copy);
    assert_ne!(new_digest, old_digest);

    let (rc, doc, err) = h.omm(&["update", "--source", copy.to_str().unwrap()]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["bundle"]["plugin"], "updated", "{doc}");
    assert_eq!(doc["bundle"]["digest"], new_digest, "{doc}");
    let canonical_copy = std::fs::canonicalize(&copy).unwrap();
    let registered =
        std::fs::canonicalize(doc["bundle"]["marketplace"]["source"].as_str().unwrap()).unwrap();
    assert_eq!(registered, canonical_copy, "{doc}");
    let ledger = h.ledger().unwrap();
    let regs = ledger["registrations"].as_array().unwrap();
    let mkt = regs
        .iter()
        .find(|r| r["kind"] == "muse-marketplace")
        .unwrap();
    assert_eq!(
        std::fs::canonicalize(mkt["source"].as_str().unwrap()).unwrap(),
        canonical_copy
    );
    let plugin = regs.iter().find(|r| r["kind"] == "muse-plugin").unwrap();
    assert_eq!(plugin["package_sha256"], new_digest);
    let ins = h.inspect().unwrap();
    assert_eq!(
        ins.raw["record"]["package_sha256"].as_str(),
        Some(new_digest.as_str())
    );
    for id in doc["bundle"]["trusted"].as_array().unwrap() {
        assert!(ins.is_trusted_enabled(id.as_str().unwrap()), "{id}");
    }
    // A second update from the same checkout is a no-op.
    let (rc, doc, err) = h.omm(&["update", "--source", copy.to_str().unwrap()]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["bundle"]["plugin"], "unchanged", "{doc}");
    assert_eq!(doc["bundle"]["digest"], new_digest);
}

/// Gate 1: doctor D1's fix for a tampered host cache is `omm install
/// --reinstall`; installed from a copy of the repository, that refused
/// ("marketplace `omm` is configured from <copy> but this install's source
/// is <repo>; pass --source <copy>") — the printed fix did not run. With no
/// `--source` and no `$OMM_SOURCE`, the checkout the ledger's marketplace
/// registration names is the source.
#[test]
fn install_without_a_source_defaults_to_the_registered_marketplace_checkout() {
    let h = harness_or_skip!();
    let copy = h.second_checkout("checkout2");
    let canonical_copy = std::fs::canonicalize(&copy).unwrap();
    let (rc, doc, err) = h.omm(&["install", "--source", copy.to_str().unwrap()]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["source"]["origin"], "--source");
    // `$OMM_SOURCE` naming another checkout still wins, and the mismatch is
    // refused as before rather than silently re-pointed.
    let (rc, _doc, err) = h.omm(&["install", "--reinstall"]);
    assert_eq!(rc, 2, "{err}");
    assert!(err.contains("pass --source"), "{err}");
    assert!(h.inspect().is_ok());
    // The finding's scenario: a byte appended to a file of the host's cached
    // package flips every capability (D1 critical, the host reports the
    // cache invalid) and doctor prints `omm install --reinstall`. No flag,
    // no env: the ledger's registration names the copy, and the fix heals.
    let cache = h.inspect().unwrap().raw["record"]["cache_path"]
        .as_str()
        .expect("record.cache_path")
        .to_string();
    let cached_skill = walkdir_files(Path::new(&cache))
        .into_iter()
        .find(|p| p.ends_with("omm-security/SKILL.md"))
        .expect("a cached skill file");
    let mut bytes = std::fs::read(&cached_skill).unwrap();
    bytes.extend_from_slice(b"\ntampered\n");
    std::fs::write(&cached_skill, bytes).unwrap();
    let d1 = |doc: &Value| -> Value {
        doc["checks"]
            .as_array()
            .expect("checks[]")
            .iter()
            .find(|c| c["id"] == "D1")
            .expect("D1")
            .clone()
    };
    let (rc, doc, err) = h.omm(&["doctor", "--fast"]);
    assert_eq!(rc, 1, "{doc}\n{err}");
    let row = d1(&doc);
    assert_eq!(row["severity"], "critical", "{row}");
    let fix = row["fix"].as_str().unwrap().to_string();
    assert_eq!(fix, "omm install --reinstall", "{row}");
    let argv: Vec<&str> = fix.split_whitespace().skip(1).collect();
    let (rc, doc, err) = h.omm_env(&argv, &[("OMM_SOURCE", "")]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["source"]["origin"], "ledger", "{doc}");
    assert_eq!(
        std::fs::canonicalize(doc["source"]["root"].as_str().unwrap()).unwrap(),
        canonical_copy
    );
    assert_eq!(doc["bundle"]["plugin"], "updated", "{doc}");
    assert_eq!(doc["bundle"]["digest"], Harness::committed_digest(&copy));
    let (rc, doc, err) = h.omm(&["doctor", "--fast"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(d1(&doc)["severity"], "info", "{}", d1(&doc));
    // `omm update` and `omm list` resolve the same way.
    let (rc, doc, err) = h.omm_env(&["update"], &[("OMM_SOURCE", "")]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["source"]["origin"], "ledger", "{doc}");
    assert_eq!(doc["bundle"]["plugin"], "unchanged", "{doc}");
    let (rc, doc, err) = h.omm_env(&["list"], &[("OMM_SOURCE", "")]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["source"]["origin"], "ledger", "{doc}");
    // An explicit --source wins over the ledger.
    let (rc, doc, err) = h.omm_env(
        &["list", "--source", h.source.to_str().unwrap()],
        &[("OMM_SOURCE", "")],
    );
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["source"]["origin"], "--source", "{doc}");
    // Uninstalled, nothing names the copy: the build checkout is the default.
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let (rc, doc, err) = h.omm_env(&["list"], &[("OMM_SOURCE", "")]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["source"]["origin"], "build checkout", "{doc}");
}

/// Gate 1: an install killed between `plugins install` and the ledger save
/// left the plugin on the host with a ledger that knew the marketplace
/// only; `omm uninstall` removed what the ledger listed, reported rc 0 with
/// no errors, and left the plugin behind (a second, ledger-less uninstall
/// then refused). Same class for a `--no-plugin` install one skill ahead
/// of its ledger. The host state of those moments, by hand.
#[test]
fn uninstall_with_a_partial_ledger_names_what_the_host_holds_beyond_it_and_reconciles_with_the_flag(
) {
    let h = harness_or_skip!();
    let baseline = h.config_snapshot();
    let marketplaces = |h: &Harness| -> Vec<String> {
        let out = h
            .inv
            .run_json(&["plugins", "marketplace", "list", "--json"])
            .expect("marketplace list");
        out.json["marketplaces"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|r| r["name"].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let ledger_path = h.omm_root().join("omm.lock.json");
    let rewrite_ledger = |f: &dyn Fn(&mut Value)| {
        let mut l: Value = serde_json::from_slice(&std::fs::read(&ledger_path).unwrap()).unwrap();
        f(&mut l);
        std::fs::write(&ledger_path, serde_json::to_vec_pretty(&l).unwrap()).unwrap();
    };

    // The bundle: the plugin installed, its registration not yet saved.
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    rewrite_ledger(&|l| {
        l["registrations"]
            .as_array_mut()
            .unwrap()
            .retain(|r| r["kind"] != "muse-plugin")
    });
    // Without the flag: refused (exit 1), the plugin named, nothing touched.
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 1, "{doc}\n{err}");
    assert!(doc.is_null(), "{doc}");
    assert!(err.contains("--reconcile-host"), "{err}");
    assert!(err.contains("plugin `oh-my-musecode`"), "{err}");
    assert!(h.inspect().is_ok(), "nothing removed without the flag");
    assert_eq!(marketplaces(&h), vec!["omm".to_string()]);
    assert!(ledger_path.exists(), "the ledger stays for the rerun");
    // A dry run names the extra step first and removes nothing.
    let (rc, doc, err) = h.omm(&["uninstall", "--reconcile-host", "--dry-run"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(
        doc["host_reconciled"],
        serde_json::json!(["muse plugins remove oh-my-musecode --delete-data"]),
        "{doc}"
    );
    let steps = doc["preview"]["host_steps"].as_array().unwrap();
    assert!(
        steps
            .iter()
            .any(|s| s == "muse plugins remove oh-my-musecode --delete-data"),
        "{doc}"
    );
    assert!(h.inspect().is_ok());
    assert!(ledger_path.exists());
    // With the flag: the plugin goes with the rest, said so, config root back.
    let (rc, doc, err) = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(
        doc["host_reconciled"],
        serde_json::json!(["muse plugins remove oh-my-musecode --delete-data"]),
        "{doc}"
    );
    assert_eq!(doc["report"]["errors"], serde_json::json!([]), "{doc}");
    assert!(h.inspect().is_err(), "the plugin is gone");
    assert!(marketplaces(&h).is_empty(), "the marketplace is gone");
    assert!(!h.omm_root().exists());
    assert_eq!(h.config_snapshot(), baseline);

    // The managed store: one skill ahead of its ledger.
    let (rc, doc, err) = h.omm(&["install", "--no-plugin"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    rewrite_ledger(&|l| {
        l["entries"].as_array_mut().unwrap().retain(|e| {
            !e["path"]
                .as_str()
                .unwrap_or("")
                .starts_with("skills/omm-docs/")
        })
    });
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 1, "{doc}\n{err}");
    assert!(err.contains("--reconcile-host"), "{err}");
    assert!(err.contains("omm-docs"), "{err}");
    assert!(!err.contains("omm-commit"), "{err}");
    assert!(h.user_skills().contains(&"omm-docs".to_string()));
    assert!(ledger_path.exists());
    let (rc, doc, err) = h.omm(&["uninstall", "--reconcile-host"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(
        doc["host_reconciled"],
        serde_json::json!(["muse skills uninstall omm-docs"]),
        "{doc}"
    );
    assert_eq!(doc["report"]["errors"], serde_json::json!([]), "{doc}");
    assert!(h.user_skills().is_empty(), "{:?}", h.user_skills());
    assert!(!h.omm_root().exists());
    assert_eq!(h.config_snapshot(), baseline);
    // A complete ledger needs no flag and reports nothing beyond it.
    let (rc, doc, err) = h.omm(&["install", "--no-plugin"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["host_reconciled"], serde_json::json!([]), "{doc}");
    assert_eq!(h.config_snapshot(), baseline);
}

/// Gate 1: after a hand `muse skills uninstall`, D1 printed
/// `omm install --no-plugin`, which skipped the skill ("differs from the
/// ledger or the source") and left D1 critical.
#[test]
fn no_plugin_doctor_d1_fix_heals_a_hand_uninstalled_skill() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install", "--no-plugin"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    h.muse_json(&["skills", "uninstall", "omm-tdd", "--json"]);
    assert!(!h.config_root().join("skills").join("omm-tdd").exists());
    let (rc, doc, err) = h.omm(&["doctor", "--fast"]);
    assert_eq!(rc, 1, "{doc}\n{err}");
    let d1 = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "D1")
        .unwrap()
        .clone();
    assert_eq!(d1["severity"], "critical", "{d1}");
    assert!(d1["observed"].as_str().unwrap().contains("omm-tdd"), "{d1}");
    let fix = d1["fix"].as_str().unwrap().to_string();
    assert_eq!(fix, "omm install --no-plugin");
    let argv: Vec<&str> = fix.split_whitespace().skip(1).collect();
    let (rc, doc, err) = h.omm(&argv);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(
        doc["skills"]["installed"],
        serde_json::json!(["omm-tdd"]),
        "{doc}"
    );
    assert!(h
        .config_root()
        .join("skills")
        .join("omm-tdd")
        .join("SKILL.md")
        .exists());
    let (rc, doc, err) = h.omm(&["doctor", "--fast"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let d1 = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "D1")
        .unwrap()
        .clone();
    assert_eq!(d1["severity"], "info", "{d1}");
    assert!(h.user_skills().contains(&"omm-tdd".to_string()));
}

/// Gate 1: `omm uninstall` recreated `settings.json` / `trust.json` that the
/// user had deleted after install (`{"schema_version":1}` and
/// `{"schema_version":1,"projects":{}}`), and the host's `plugins remove
/// --delete-data` recreated settings.json too.
#[test]
fn uninstall_after_the_user_removed_the_shared_files_recreates_neither() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let settings = h.config_root().join("settings.json");
    let trust = h.config_root().join("trust.json");
    std::fs::remove_file(&settings).unwrap();
    std::fs::remove_file(&trust).unwrap();
    let (rc, doc, err) = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let dropped: Vec<&str> = doc["preview"]["dropped"]
        .as_array()
        .expect("preview.dropped")
        .iter()
        .filter_map(|d| d["step"].as_str())
        .collect();
    assert!(dropped.iter().any(|d| d.contains("settings key")), "{doc}");
    assert!(dropped.iter().any(|d| d.contains("trust entry")), "{doc}");
    assert!(
        !settings.exists() && !trust.exists(),
        "a dry run creates nothing"
    );
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["report"]["errors"], serde_json::json!([]), "{doc}");
    assert!(!settings.exists(), "settings.json recreated: {doc}");
    assert!(!trust.exists(), "trust.json recreated: {doc}");
    assert!(h.ledger().is_none());
    assert!(!h.omm_root().exists());
    assert!(h.inspect().is_err());
    assert!(h.config_snapshot().is_empty(), "{:?}", h.config_snapshot());
}

/// Gate 1: a corrupt ledger → D13 critical with fix `omm reconcile` → a
/// fresh ledger with the registrations only → D13 warn "present but
/// unlisted" with the same no-op fix; `omm install` then healed D13 but
/// could not re-register the profile's settings keys (no prior known), and
/// the later uninstall silently left them in settings.json.
#[test]
fn a_corrupt_ledger_heals_through_the_doctor_fix_and_uninstall_names_the_unregistered_keys() {
    let h = harness_or_skip!();
    let (rc, doc, err) = h.omm(&["install"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let ledger_path = h.omm_root().join("omm.lock.json");
    let bytes = std::fs::read(&ledger_path).unwrap();
    std::fs::write(&ledger_path, &bytes[..bytes.len() / 2]).unwrap();
    let d13 = |h: &Harness| -> (i32, Value) {
        let (rc, doc, err) = h.omm(&["doctor", "--fast"]);
        assert!(doc.is_object(), "{err}");
        let row = doc["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "D13")
            .unwrap()
            .clone();
        (rc, row)
    };
    let (rc, row) = d13(&h);
    assert_eq!(rc, 1, "{row}");
    assert_eq!(row["severity"], "critical", "{row}");
    let fix = row["fix"].as_str().unwrap().to_string();
    let lines: Vec<&str> = fix.lines().collect();
    assert_eq!(lines, vec!["omm reconcile", "omm install"], "{row}");
    for line in &lines {
        let argv: Vec<&str> = line.split_whitespace().skip(1).collect();
        let (rc, doc, err) = h.omm(&argv);
        assert_eq!(rc, 0, "fix `{line}`: {doc}\n{err}");
    }
    let (rc, row) = d13(&h);
    assert_eq!(rc, 0, "{row}");
    assert_eq!(row["severity"], "info", "{row}");
    // The keys the profile set are current but unregistered (prior unknown):
    // doctor and the uninstall preview both say they will stay.
    let observed = row["observed"].as_str().unwrap();
    assert!(
        observed.contains("run.context_slimming.skill_catalog_descriptions"),
        "{observed}"
    );
    let (rc, doc, err) = h.omm(&["uninstall", "--dry-run"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    let unregistered: Vec<&str> = doc["preview"]["unregistered"]
        .as_array()
        .expect("preview.unregistered")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        unregistered
            .iter()
            .any(|u| u.contains("run.context_slimming.skill_catalog_descriptions")),
        "{doc}"
    );
    let (rc, doc, err) = h.omm(&["uninstall"]);
    assert_eq!(rc, 0, "{doc}\n{err}");
    assert_eq!(doc["report"]["errors"], serde_json::json!([]));
    let settings: Value =
        serde_json::from_slice(&std::fs::read(h.config_root().join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(
        settings["run"]["context_slimming"]["skill_catalog_descriptions"], "first_sentence",
        "prior unknown: kept, and named"
    );
    assert!(h.inspect().is_err());
    assert!(h.ledger().is_none());
}
