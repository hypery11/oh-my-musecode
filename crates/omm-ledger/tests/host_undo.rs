//! PLAN.md 1.1 against the real binary: the `HostUndoer` restores a settings
//! key and a trust entry by targeted merge and removes a managed skill with
//! `muse skills uninstall`, and the ledger's uninstall leaves the host's
//! `settings.json` byte-identical. Skips with a message when `OMM_MUSE_BIN`
//! is unset. Everything runs in a throwaway HOME/XDG sandbox with a temp cwd;
//! nothing touches the real config root; no login, no provider.

use std::path::PathBuf;

use serde_json::json;
use tempfile::TempDir;

use omm_host::fsx;
use omm_host::settings::{CommitOptions, PatchOp, SettingsDoc};
use omm_host::trust::{TrustDecision, TrustStore};
use omm_host::{Invoker, Roots, Sandbox};

use omm_ledger::containment::Bases;
use omm_ledger::schema::{
    Class, Entry, HostInfo, Kind, Ledger, Mechanism, Registration, RelPath, Scope,
};
use omm_ledger::uninstall::{self, HostUndoer, UndoOutcome, UndoStep, Undoer};
use omm_ledger::{store, Base};

struct Host {
    inv: Invoker,
    sb: Sandbox,
    roots: Roots,
    _tmp: TempDir,
}

fn host() -> Option<Host> {
    let bin = std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)?;
    let tmp = tempfile::Builder::new()
        .prefix("omm-ledger-host-")
        .tempdir()
        .expect("temp dir");
    let sb = Sandbox::create(tmp.path()).expect("sandbox");
    let cfg = sb.config_home.join("muse");
    std::fs::create_dir_all(&cfg).expect("config dir");
    std::fs::write(cfg.join("settings.json"), b"{\"schema_version\":1}\n").expect("settings");
    let cwd = tmp.path().join("cwd");
    std::fs::create_dir_all(&cwd).expect("cwd");
    let inv = Invoker::new(bin).sandboxed(&sb).cwd(&cwd);
    let roots = sb.roots().expect("roots");
    Some(Host {
        inv,
        sb,
        roots,
        _tmp: tmp,
    })
}

macro_rules! host_or_skip {
    () => {
        match host() {
            Some(h) => h,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

fn entry(base: Base, rel: &str, sha: &str, kind: Kind, mechanism: Mechanism) -> Entry {
    Entry {
        base,
        path: RelPath::new(rel).unwrap(),
        kind,
        sha256: sha.to_string(),
        source_version: "0.1.0".into(),
        writer: "omm install".into(),
        mechanism,
        class: Class::Exclusive,
        prior: None,
    }
}

#[test]
fn host_undoer_restores_keys_and_uninstalls_a_managed_skill() {
    let h = host_or_skip!();
    let settings_path = h.roots.settings_file();
    // A clean home: neither settings.json nor trust.json exists before omm.
    std::fs::remove_file(&settings_path).unwrap();
    let omm_root = h.roots.omm_root();
    let ws = h.sb.root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let bases = Bases::from_roots(&h.roots, Some(&ws));
    let mut ledger = Ledger::new(
        "0.1.0",
        HostInfo {
            version: "test".into(),
            sha256: "0".repeat(64),
        },
        Scope::User,
    );

    // 1. A managed skill through the host's own installer.
    let src = h.sb.root.join("skill-src").join("omm-ht-x");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("SKILL.md"),
        "---\nname: omm-ht-x\ndescription: Host-test skill for the ledger crate; do not use it for real work.\n---\n\nBody.\n",
    )
    .unwrap();
    let installed = h
        .inv
        .run(&[
            "skills".to_string(),
            "install".to_string(),
            src.to_string_lossy().into_owned(),
            "--json".to_string(),
        ])
        .expect("skills install")
        .expect_ok()
        .expect("skills install ok");
    assert!(
        installed.stdout.contains("omm-ht-x"),
        "{}",
        installed.stdout
    );
    let skill_file = h
        .roots
        .personal_skills_dir()
        .join("omm-ht-x")
        .join("SKILL.md");
    assert!(skill_file.exists(), "{}", skill_file.display());
    let sha = fsx::sha256_file(&skill_file).unwrap();
    ledger.upsert(entry(
        Base::MuseConfig,
        "skills/omm-ht-x/SKILL.md",
        &sha,
        Kind::Skill,
        Mechanism::MuseSkillsInstall,
    ));

    // 2. A settings key by targeted patch (prior: absent); the file is created.
    let mut doc = SettingsDoc::for_roots(&h.roots).unwrap();
    assert!(!doc.existed());
    let priors = doc
        .patch_typed(&[PatchOp::set("tui.theme", json!("custom:omm-ht"))])
        .unwrap();
    assert_eq!(priors[0].prior, None);
    doc.commit(&h.inv, &CommitOptions::for_roots(&h.roots))
        .expect("settings commit");
    ledger.register(Registration::SettingsKey {
        path: "tui.theme".into(),
        prior: None,
        value: Some(json!("custom:omm-ht")),
        profile: None,
    });
    let mut created = entry(
        Base::MuseConfig,
        "settings.json",
        &fsx::sha256_file(&settings_path).unwrap(),
        Kind::SettingsKey,
        Mechanism::SettingsPatch,
    );
    created.class = Class::Seeded;
    ledger.upsert(created);

    // 3. A trust entry (prior: absent).
    let mut trust = TrustStore::for_roots(&h.roots).unwrap();
    assert!(!trust.existed());
    let merge = trust.merge_project(&ws, TrustDecision::Trusted).unwrap();
    trust
        .commit(&CommitOptions::for_roots(&h.roots))
        .expect("trust commit");
    ledger.register(Registration::Trust {
        project: merge.key.clone(),
        prior: merge.prior.clone(),
        value: Some(json!({"decision": "trusted"})),
    });
    let mut created = entry(
        Base::MuseConfig,
        "trust.json",
        &fsx::sha256_file(&h.roots.trust_file()).unwrap(),
        Kind::Trust,
        Mechanism::TrustMerge,
    );
    created.class = Class::Seeded;
    ledger.upsert(created);

    // 4. A plain copy.
    let theme = h.roots.themes_dir().join("omm-ht.tmTheme");
    fsx::create_dir_all(h.roots.themes_dir().as_path()).unwrap();
    fsx::write_atomic(&fsx::realpath_for_write(&theme).unwrap(), b"<plist/>").unwrap();
    ledger.upsert(entry(
        Base::MuseConfig,
        "themes/omm-ht.tmTheme",
        &fsx::sha256_bytes(b"<plist/>"),
        Kind::Theme,
        Mechanism::Copy,
    ));
    store::save(&omm_root, &ledger).unwrap();

    // The host sees what we set.
    let now: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).unwrap()).unwrap();
    assert_eq!(now["tui"]["theme"], "custom:omm-ht");
    assert_eq!(
        TrustStore::load(&h.roots.trust_file())
            .unwrap()
            .decision_for(&ws)
            .unwrap()
            .as_deref(),
        Some("trusted")
    );

    // 5. Plan + apply through the real host; both keys still hold what omm
    //    wrote, so both are restored.
    let opts = uninstall::Options {
        settings_file: Some(h.roots.settings_file()),
        trust_file: Some(h.roots.trust_file()),
        ..uninstall::Options::default()
    };
    let plan = uninstall::plan(&ledger, &bases, opts).unwrap();
    assert!(plan.refused.is_empty(), "{:?}", plan.refused);
    assert_eq!(
        plan.host_steps,
        vec![
            UndoStep::SkillsUninstall {
                id: "omm-ht-x".into()
            },
            UndoStep::RestoreTrust {
                project: merge.key.clone(),
                prior: None
            },
            UndoStep::RestoreSettingsKey {
                path: "tui.theme".into(),
                prior: None
            },
        ]
    );
    assert_eq!(plan.remove.len(), 2);
    assert_eq!(plan.created.len(), 2);
    let mut undoer = HostUndoer::new(h.inv.clone(), h.roots.clone());
    let report = uninstall::apply(
        &plan,
        ledger,
        &bases,
        &mut undoer,
        &uninstall::ApplyOptions {
            omm_version: "0.1.0".into(),
            dry_run: false,
        },
    )
    .unwrap();
    assert!(report.complete(), "{:?}", report.errors);
    assert_eq!(report.registrations_undone, 3);
    assert!(report.ledger_removed);
    // The skill went through `muse skills uninstall`, so the file was already
    // gone when the unlink step reached it.
    assert!(!skill_file.exists());
    assert!(!h.roots.personal_skills_dir().join("omm-ht-x").exists());
    assert!(!theme.exists());
    assert!(
        !h.roots.themes_dir().exists(),
        "the empty themes/ dir we created is pruned"
    );
    assert_eq!(report.removed, 4, "{report:?}");
    // The restores emptied both files omm created, so both are gone (R5).
    assert!(
        !settings_path.exists(),
        "settings.json created by omm is removed once empty"
    );
    assert!(
        !h.roots.trust_file().exists(),
        "trust.json created by omm is removed once empty"
    );
    assert!(!store::ledger_path(&omm_root).exists());
    // What the host itself leaves behind is not ours: nothing omm-named remains.
    let leftovers: Vec<String> = h
        .roots
        .muse_config()
        .read_dir()
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("omm") || n == "themes" || n == "settings.json" || n == "trust.json")
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    // The host still loads its config.
    assert!(h
        .inv
        .run(&["skills", "list", "--source", "user", "--json"])
        .unwrap()
        .ok());
}

#[test]
fn host_undoer_treats_an_already_absent_plugin_marketplace_and_skill_as_done() {
    // Gate 1 decision B: the host's `not installed` / `not configured` /
    // `skill not installed` are success — the undo is idempotent, the
    // registration is dropped with the host's reason (round 4: one such
    // answer wedged every rerun of `omm uninstall`).
    let h = host_or_skip!();
    let mut undoer = HostUndoer::new(h.inv.clone(), h.roots.clone());
    for step in [
        UndoStep::PluginRemove {
            id: "omm-ht-nope".into(),
        },
        UndoStep::MarketplaceRemove {
            name: "omm-ht-nope".into(),
        },
        UndoStep::SkillsUninstall {
            id: "omm-ht-nope".into(),
        },
    ] {
        match undoer.undo(&step) {
            Ok(UndoOutcome::AlreadyGone(reason)) => {
                assert!(!reason.is_empty(), "{step:?}");
                assert!(
                    reason.contains("omm-ht-nope"),
                    "{step:?}: the reason names the object: {reason}"
                );
            }
            other => panic!("{step:?}: expected AlreadyGone, got {other:?}"),
        }
    }
    // An unknown settings key is refused before anything is written (R9).
    let before = std::fs::read(h.roots.settings_file()).unwrap();
    let err = undoer
        .undo(&UndoStep::RestoreSettingsKey {
            path: "_omm_marker".into(),
            prior: Some(json!(1)),
        })
        .unwrap_err();
    assert!(err.contains("typed settings.json keys"), "{err}");
    assert_eq!(std::fs::read(h.roots.settings_file()).unwrap(), before);
}

#[test]
fn host_undoer_keeps_a_structural_member_the_users_siblings_need() {
    // Gate 1 decision D: `permissions` is a deny_unknown_fields struct with a
    // required `schema_version` (settings-keys.json `structural`). Restoring
    // that leaf to its prior (absent) must not strip it while other members
    // remain — the host would refuse the whole object (`Named permission
    // profiles are unavailable: missing field schema_version`) and the
    // user's own profiles would silently stop working (round 4, K).
    let h = host_or_skip!();
    let settings_path = h.roots.settings_file();
    std::fs::write(
        &settings_path,
        br#"{"schema_version":1,"permissions":{"schema_version":1,"default_profile":"omm-strict","profiles":{"omm-strict":{"extends":":ask-me","approval":"prompt_unmatched","reviewer":"human"},"mine":{"extends":":ask-me","approval":"prompt_unmatched","reviewer":"human"}}}}
"#,
    )
    .unwrap();
    let omm_root = h.roots.omm_root();
    let bases = Bases::from_roots(&h.roots, None);
    let mut ledger = Ledger::new(
        "0.1.0",
        HostInfo {
            version: "test".into(),
            sha256: "0".repeat(64),
        },
        Scope::User,
    );
    // Registered in the order a profile slice lands them; the structural
    // leaf first, so a naive reverse-order restore would strip it first.
    for (key, value) in [
        ("permissions.schema_version", json!(1)),
        ("permissions.default_profile", json!("omm-strict")),
        ("permissions.profiles.omm-strict.extends", json!(":ask-me")),
        (
            "permissions.profiles.omm-strict.approval",
            json!("prompt_unmatched"),
        ),
        ("permissions.profiles.omm-strict.reviewer", json!("human")),
    ] {
        ledger.record_settings_key(key, None, Some(value), Some("strict"));
    }
    let mut shared = entry(
        Base::MuseConfig,
        "settings.json",
        "n/a",
        Kind::SettingsKey,
        Mechanism::SettingsPatch,
    );
    shared.class = Class::SharedKey;
    ledger.upsert(shared);
    store::save(&omm_root, &ledger).unwrap();
    let plan = uninstall::plan(
        &ledger,
        &bases,
        uninstall::Options {
            settings_file: Some(settings_path.clone()),
            ..uninstall::Options::default()
        },
    )
    .unwrap();
    // The structural leaf is predicted kept (the user's `mine` needs it) and
    // is not among the steps; every other leaf is restored.
    assert!(
        plan.dropped
            .iter()
            .any(|d| d.reason.contains("permissions.schema_version") && d.reason.contains("kept")),
        "{:?}",
        plan.dropped
    );
    assert!(
        !plan.host_steps.iter().any(|s| matches!(
            s,
            UndoStep::RestoreSettingsKey { path, .. } if path == "permissions.schema_version"
        )),
        "{:?}",
        plan.host_steps
    );
    let mut undoer = HostUndoer::new(h.inv.clone(), h.roots.clone());
    let report = uninstall::apply(
        &plan,
        ledger,
        &bases,
        &mut undoer,
        &uninstall::ApplyOptions {
            omm_version: "0.1.0".into(),
            dry_run: false,
        },
    )
    .unwrap();
    assert!(report.complete(), "{:?}", report.errors);
    let doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).unwrap()).unwrap();
    assert_eq!(doc["permissions"]["schema_version"], 1, "{doc}");
    assert!(doc["permissions"].get("default_profile").is_none(), "{doc}");
    assert!(
        doc["permissions"]["profiles"].get("omm-strict").is_none(),
        "{doc}"
    );
    assert_eq!(doc["permissions"]["profiles"]["mine"]["reviewer"], "human");
    // The host loads the document with the user's named profile intact.
    let probe =
        omm_host::probe::settings_load_probe(&h.inv, &std::fs::read(&settings_path).unwrap())
            .unwrap();
    assert!(probe.accepted, "{}", probe.detail);
    // The runtime safety net: a restore asked directly while siblings remain
    // is kept, never stripped.
    let mut undoer = HostUndoer::new(h.inv.clone(), h.roots.clone());
    match undoer.undo(&UndoStep::RestoreSettingsKey {
        path: "permissions.schema_version".into(),
        prior: None,
    }) {
        Ok(UndoOutcome::Kept(reason)) => assert!(reason.contains("permissions"), "{reason}"),
        other => panic!("expected Kept, got {other:?}"),
    }
    let doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).unwrap()).unwrap();
    assert_eq!(doc["permissions"]["schema_version"], 1, "{doc}");
    // With no other member left, the leaf goes and the empty parent with it.
    std::fs::write(
        &settings_path,
        br#"{"schema_version":1,"permissions":{"schema_version":1}}
"#,
    )
    .unwrap();
    let mut undoer = HostUndoer::new(h.inv.clone(), h.roots.clone());
    assert!(matches!(
        undoer.undo(&UndoStep::RestoreSettingsKey {
            path: "permissions.schema_version".into(),
            prior: None,
        }),
        Ok(UndoOutcome::Undone)
    ));
    let doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&settings_path).unwrap()).unwrap();
    assert!(doc.get("permissions").is_none(), "{doc}");
}
