//! PLAN.md 0.2 acceptance — every probe and writer against `.host/bin/*`.
//! Each test skips with a message when `OMM_MUSE_BIN` is unset. Everything
//! runs in a throwaway HOME/XDG sandbox; nothing touches the real config root.

use std::path::{Path, PathBuf};

use serde_json::json;
use tempfile::TempDir;

use omm_host::fixtures::{self, PackageSpec};
use omm_host::host_reality as hr;
use omm_host::invoke::OutcomeKind;
use omm_host::probe::{self, EchoSessionOptions, GateSource, SchemaKind, SkillsListOptions};
use omm_host::settings::{CommitOptions, PatchOp, SettingsDoc};
use omm_host::trust::{TrustDecision, TrustStore};
use omm_host::{HostError, Invoker, Roots, Sandbox};

struct Host {
    inv: Invoker,
    sb: Sandbox,
    _tmp: TempDir,
}

fn host() -> Option<Host> {
    let bin = std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)?;
    let tmp = tempfile::Builder::new()
        .prefix("omm-probes-")
        .tempdir()
        .expect("temp dir");
    let sb = Sandbox::create(tmp.path()).expect("sandbox");
    let cfg = sb.config_home.join("muse");
    std::fs::create_dir_all(&cfg).expect("config dir");
    std::fs::write(cfg.join("settings.json"), b"{\"schema_version\":1}\n").expect("settings");
    let inv = Invoker::new(bin).sandboxed(&sb);
    Some(Host { inv, sb, _tmp: tmp })
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

fn settings_path(h: &Host) -> PathBuf {
    h.sb.config_home.join("muse").join("settings.json")
}

fn roots(h: &Host) -> Roots {
    h.sb.roots().expect("roots")
}

/// Names beside a host file that omm put there (host-owned files such as
/// `.auth.json.lock` are the host's business).
fn omm_footprint(dir: &Path) -> Vec<String> {
    dir.read_dir()
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("omm"))
        .collect()
}

#[test]
fn version_probe() {
    let h = host_or_skip!();
    let v = probe::version(&h.inv).expect("version");
    assert_eq!(v.product, hr::VERSION_STRING_PRODUCT);
    assert!(!v.version.is_empty() && !v.build.is_empty(), "{v:?}");
}

#[test]
fn gates_probe_and_override_detection() {
    let h = host_or_skip!();
    let report = probe::gates(&h.inv).expect("gates");
    // The gates a build must show come from gates.json: every untagged row
    // plus every `since`-tagged row its trace shows; a tagged row it lacks is
    // OLDER-BUILD (1.0.1-R2006.1 lacks `ultra_reasoning_effort`; 1.0.3-R2198.1 lacks the four
    // 1.3.0 gates `official_plugin_marketplace`, `memory_repository_sync`,
    // `native_connector_delivery` and `vim`, and defaults
    // `voice`, `voice_default_on` and `todo_reminder` to off — each `since` group
    // passes whole or not at all; 1.0.3-R2198.1
    // shows it). An unlisted or a missing untagged gate is drift (R15).
    let data = hr::gates().expect("gates.json");
    let observed: Vec<String> = report.gates.iter().map(|g| g.id.clone()).collect();
    let part = data.partition_for_build(&observed);
    let observed_set: std::collections::BTreeSet<String> = observed.iter().cloned().collect();
    assert_eq!(
        observed_set, part.expected,
        "gate drift (older-build rows: {:?})",
        part.older_build
    );
    assert_eq!(report.gates.len() + part.older_build.len(), hr::GATES_TOTAL);
    let expected_on = data.default_on_expected(&part);
    assert_eq!(report.default_on_ids().len(), expected_on.len());
    assert_eq!(
        expected_on.len()
            + part
                .older_build
                .iter()
                .filter(|id| data.default_on_ids.contains(id))
                .count(),
        hr::GATES_DEFAULT_ON
    );
    assert!(report.attempts >= 1 && report.attempts <= probe::GATE_PROBE_MAX_ATTEMPTS);
    // 8 fixed lines plus one per gate (49 on a 41-gate build, 50 on 1.0.3-R2198.1).
    assert_eq!(
        report.trace_lines,
        hr::BOOTSTRAP_TRACE_FIXED_LINES + report.gates.len()
    );
    assert!(report.trace_lines <= hr::BOOTSTRAP_TRACE_LINES_OBSERVED);
    // Setting a gate in the invoker's environment shows up as an override.
    let forced = probe::gates(&h.inv.clone().env("MUSE_EXPERIMENTAL_VOICE", "1")).expect("gates");
    let voice = forced.get("voice").expect("voice gate present");
    assert!(voice.enabled);
    assert_eq!(voice.source, GateSource::Override);
    // The probe left nothing in the sandbox data root: it used its own temp root.
    assert!(!h.sb.data_home.join("muse").join("plugins").exists());
}

#[test]
fn gates_probe_survives_concurrent_host_load() {
    // The host's bootstrap-trace writer is lossy under concurrent bootstraps
    // (measured 2026-09-02 on 1.0.1-R2006.1: 8 parallel `plugins enable` +
    // 6 echo sessions → logs of 0, 14, 22 or 48 lines instead of 49). Every
    // probe must still return the complete gate table — never a partial one.
    let h = host_or_skip!();
    let ws = h.sb.root.join("ws-load");
    std::fs::create_dir_all(&ws).unwrap();
    std::thread::scope(|scope| {
        let inv = &h.inv;
        let ws = &ws;
        let mut probes = Vec::new();
        for _ in 0..6 {
            probes.push(scope.spawn(move || probe::gates(inv)));
        }
        let mut sessions = Vec::new();
        for _ in 0..6 {
            sessions.push(scope.spawn(move || {
                inv.clone().data_home(&ws.join("data")).cwd(ws).run(&[
                    "exec",
                    "--provider",
                    "echo",
                    "--trust-workspace",
                    "hi",
                ])
            }));
        }
        let data = hr::gates().expect("gates.json");
        for p in probes {
            let report = p.join().expect("thread").expect("gate probe under load");
            let observed: Vec<String> = report.gates.iter().map(|g| g.id.clone()).collect();
            let part = data.partition_for_build(&observed);
            assert_eq!(
                report.gates.len(),
                part.expected.len(),
                "partial gate table returned"
            );
            assert_eq!(
                report.default_on_ids().len(),
                data.default_on_expected(&part).len()
            );
        }
        for s in sessions {
            let _ = s.join().expect("thread");
        }
    });
}

#[test]
fn skills_list_probe_sees_the_gate() {
    let h = host_or_skip!();
    let opts = SkillsListOptions {
        source: Some("built-in".into()),
        ..Default::default()
    };
    let without = probe::skills_list(&h.inv, &opts).expect("skills list");
    assert_eq!(without.skills.len(), hr::BUNDLED_SKILLS_VISIBLE_DEFAULT);
    assert!(without.diagnostics.is_empty());
    let with = probe::skills_list(&h.inv.clone().with_plugins_gate(), &opts).expect("skills list");
    assert_eq!(with.skills.len(), hr::BUNDLED_SKILLS);
    assert!(with.skills.iter().all(|s| s.id.starts_with("bundled:")));
}

#[test]
fn config_probes() {
    let h = host_or_skip!();
    let status = probe::config_status(&h.inv).expect("config status");
    assert_eq!(status.generation, hr::ENTERPRISE_GENERATION);
    assert!(
        status.sources.len() >= hr::ENTERPRISE_SOURCES,
        "{:?}",
        status.sources
    );

    let dir = h.sb.root.join("cfgv");
    std::fs::create_dir_all(&dir).unwrap();
    let ok = dir.join("ok.json");
    std::fs::write(
        &ok,
        b"{\"schema_version\":1,\"settings\":{\"provider\":\"echo\"}}",
    )
    .unwrap();
    assert!(probe::config_validate(&h.inv, "defaults", &ok)
        .expect("validate")
        .is_accepted());
    let v2 = dir.join("v2.json");
    std::fs::write(&v2, b"{\"schema_version\":2}").unwrap();
    let r = probe::config_validate(&h.inv, "defaults", &v2).expect("validate");
    assert_eq!(r.reason(), Some("unsupported_schema_version"));
    let wrong = dir.join("wrong.json");
    std::fs::write(
        &wrong,
        b"{\"schema_version\":1,\"settings\":{\"provider\":12345}}",
    )
    .unwrap();
    let r = probe::config_validate(&h.inv, "defaults", &wrong).expect("validate");
    assert_eq!(r.reason(), Some("wrong_type"));
}

#[test]
fn schema_probe() {
    let h = host_or_skip!();
    let out = h.sb.root.join("schema");
    let export = probe::schema_export(&h.inv, SchemaKind::JsonSchema, false, &out).expect("schema");
    assert_eq!(
        export.manifest.as_ref().map(|m| m.fingerprint.as_str()),
        Some(hr::MSP_STABLE_FINGERPRINT)
    );
    let counts = probe::schema_counts(&out.join("msp.schema.json")).expect("counts");
    assert_eq!(
        (counts.methods, counts.notifications, counts.errors),
        (hr::MSP_METHODS, hr::MSP_NOTIFICATIONS, hr::MSP_ERROR_CODES)
    );
}

#[test]
fn plugin_lifecycle_probe_validate_install_inspect_approve_remove() {
    let h = host_or_skip!();
    let spec = PackageSpec::native("omm-five", fixtures::five_family_capabilities().unwrap());
    let root = fixtures::write_package(&h.sb.root, "five", &spec).unwrap();

    let v = probe::plugins_validate(&h.inv, &root).expect("validate");
    assert!(v.passes(), "{:?}", v.four_predicates());
    assert_eq!(v.summary.as_deref(), Some("full"));

    let installed = h
        .inv
        .run(&[
            "plugins".to_string(),
            "install".to_string(),
            root.to_string_lossy().into_owned(),
            "--json".to_string(),
        ])
        .expect("install")
        .expect_ok()
        .expect("install ok");
    assert!(installed.stdout.contains("\"manifest_family\": \"native\""));
    // Installing writes nothing into settings.json (settings-plugins.md §5).
    assert_eq!(
        std::fs::read_to_string(settings_path(&h)).unwrap().trim(),
        "{\"schema_version\":1}"
    );

    let before = probe::plugins_inspect(&h.inv, "omm-five").expect("inspect");
    assert_eq!(before.manifest_family.as_deref(), Some("native"));
    assert_eq!(before.runtime_capabilities.len(), 3);
    assert!(before
        .runtime_capabilities
        .iter()
        .all(|c| c.status == "review_needed"));
    assert_eq!(before.effective_capabilities.len(), 2);
    assert!(!before.is_trusted_enabled("plugin:omm-five:hook:omm-hook"));

    let approved = probe::plugins_approve(&h.inv, "omm-five").expect("approve");
    assert_eq!(approved.len(), 3);
    let after = probe::plugins_inspect(&h.inv, "omm-five").expect("inspect");
    for id in [
        "plugin:omm-five:hook:omm-hook",
        "plugin:omm-five:mcp_server:omm-mcp",
        "plugin:omm-five:reminder:omm-rem",
    ] {
        assert!(
            after.is_trusted_enabled(id),
            "{id}: {:?}",
            after.capability(id)
        );
    }
    // Approval lands in runtime_capabilities — the legacy store that decides.
    let settings: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(settings_path(&h)).unwrap()).unwrap();
    assert!(
        settings["runtime_capabilities"]["plugin:omm-five:hook:omm-hook"]["enabled"]
            .as_bool()
            .unwrap()
    );

    h.inv
        .run(&["plugins", "remove", "omm-five", "--delete-data", "--json"])
        .expect("remove")
        .expect_ok()
        .expect("remove ok");
    match probe::plugins_inspect(&h.inv, "omm-five") {
        Err(HostError::HostReported { code, .. }) => assert_eq!(code, "unknown-plugin"),
        other => panic!("expected unknown-plugin, got {other:?}"),
    }
}

#[test]
fn echo_session_probe_trusted_vs_untrusted() {
    let h = host_or_skip!();
    let ws = h.sb.root.join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    let trusted = probe::echo_session(
        &h.inv,
        &EchoSessionOptions::in_workspace(&ws)
            .trusted(true)
            .with_export(true),
    )
    .expect("echo session");
    let expected: Vec<u64> = hr::CONTEXT_BLOCK_ORDERS
        .iter()
        .map(|o| u64::from(*o))
        .collect();
    assert_eq!(trusted.facts.orders(), expected);
    assert_eq!(trusted.facts.active_tools.len(), hr::ACTIVE_TOOLS);
    assert_eq!(
        trusted.facts.frame_schema_version,
        Some(hr::FRAME_SCHEMA_VERSION)
    );
    assert_eq!(
        trusted.export_schema_version,
        Some(hr::EXPORT_SCHEMA_VERSION)
    );
    assert_eq!(
        trusted
            .facts
            .block(u64::from(hr::CONTEXT_ORDER_WORKFLOW_COOKBOOK))
            .map(|b| b.bytes as u64),
        Some(hr::WORKFLOW_COOKBOOK_BYTES)
    );
    // The sandbox data root stays empty of sessions: the probe used its own temp root.
    assert!(!h.sb.data_home.join("muse").join("sessions").exists());

    // Untrusted: no subagent_delegation block (186) and no subagent_* tools —
    // 7 blocks / 20 tools (context-slimming.md §7.2).
    let untrusted =
        probe::echo_session(&h.inv, &EchoSessionOptions::in_workspace(&ws)).expect("echo session");
    let expected: Vec<u64> = hr::CONTEXT_BLOCK_ORDERS_UNTRUSTED
        .iter()
        .map(|o| u64::from(*o))
        .collect();
    assert_eq!(untrusted.facts.orders(), expected);
    assert!(!untrusted
        .facts
        .orders()
        .contains(&u64::from(hr::CONTEXT_ORDER_SUBAGENT_DELEGATION)));
    assert_eq!(
        untrusted.facts.active_tools.len(),
        hr::ACTIVE_TOOLS_UNTRUSTED
    );
    assert!(!untrusted
        .facts
        .active_tools
        .iter()
        .any(|t| t.starts_with("subagent_")));
    assert_eq!(
        trusted
            .facts
            .block(u64::from(hr::CONTEXT_ORDER_SUBAGENT_DELEGATION))
            .map(|b| b.bytes as u64),
        Some(hr::SUBAGENT_DELEGATION_BYTES)
    );

    // A fresh probe-owned workspace measures the same as an empty explicit one,
    // and never the shared system temp dir (where AGENTS.md / .agents/skills
    // could compose under trust).
    let fresh = EchoSessionOptions::fresh_workspace().expect("fresh workspace");
    assert_ne!(fresh.workspace, std::env::temp_dir());
    let session = probe::echo_session(&h.inv, &fresh).expect("echo session");
    assert_eq!(session.facts.active_tools.len(), hr::ACTIVE_TOOLS);
    assert_eq!(
        session.facts.context_blocks.len(),
        hr::CONTEXT_BLOCK_ORDERS.len()
    );
    assert_eq!(
        fresh.workspace.read_dir().unwrap().count(),
        0,
        "still empty"
    );
}

#[test]
fn settings_apply_end_to_end() {
    let h = host_or_skip!();
    let path = settings_path(&h);
    let original = std::fs::read(&path).unwrap();

    let mut doc = SettingsDoc::load(&path).expect("load");
    let priors = doc
        .patch_typed(&[
            PatchOp::set("tui.theme", json!("custom:omm-dusk")),
            PatchOp::set("run.workflow_trigger_mode", json!("off")),
            PatchOp::set("tui.terminal_background", json!("dark")), // enterprise-blind, loader-checked
        ])
        .expect("patch");
    assert_eq!(priors.len(), 3);
    assert!(priors.iter().all(|p| p.prior.is_none()));

    let dry = doc
        .commit(&h.inv, &CommitOptions::for_roots(&roots(&h)).dry_run(true))
        .expect("dry run");
    assert!(!dry.written);
    assert_eq!(
        std::fs::read(&path).unwrap(),
        original,
        "dry run wrote nothing"
    );

    let report = doc
        .commit(&h.inv, &CommitOptions::for_roots(&roots(&h)))
        .expect("commit");
    assert!(report.written);
    let backup = report.backup.clone().expect("backup taken");
    assert_eq!(
        std::fs::read(&backup).unwrap(),
        original,
        "backup is byte-identical to the original"
    );
    assert!(
        backup.starts_with(roots(&h).snapshots_dir()),
        "{}",
        backup.display()
    );
    assert!(report
        .validation
        .enterprise
        .iter()
        .any(|(k, v)| k == "run" && v.is_accepted()));
    assert!(
        report
            .validation
            .enterprise
            .iter()
            .any(|(k, v)| k == "tui" && v.is_accepted()),
        "tui.theme is field_not_activated, which counts as accepted"
    );
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(written["tui"]["theme"], "custom:omm-dusk");
    assert_eq!(written["run"]["workflow_trigger_mode"], "off");
    assert_eq!(written["schema_version"], 1);
    // The host loads what we wrote.
    assert!(h
        .inv
        .run(&["skills", "list", "--source", "user", "--json"])
        .unwrap()
        .ok());

    // Reload: priors are the values we just set; a second commit restores.
    let mut doc2 = SettingsDoc::load(&path).expect("reload");
    let priors = doc2
        .patch_typed(&[PatchOp::remove("tui.theme")])
        .expect("patch");
    assert_eq!(priors[0].prior, Some(json!("custom:omm-dusk")));

    // A wrong-typed value is refused by the enterprise validator.
    let mut bad = SettingsDoc::for_roots(&roots(&h)).expect("load");
    bad.patch_typed(&[PatchOp::set("provider", json!(12345))])
        .unwrap();
    match bad.commit(&h.inv, &CommitOptions::default()) {
        Err(HostError::SettingsRejected { stage, detail }) => {
            assert_eq!(stage, "enterprise-validator");
            assert!(detail.contains("wrong_type"), "{detail}");
        }
        other => panic!("expected rejection, got {other:?}"),
    }
    // A bad enum on an enterprise-blind field is caught by the host loader.
    let mut bad = SettingsDoc::for_roots(&roots(&h)).expect("load");
    bad.patch_typed(&[PatchOp::set("tui.verbose_output", json!("nope"))])
        .unwrap();
    match bad.commit(&h.inv, &CommitOptions::default()) {
        Err(HostError::SettingsRejected { .. }) => {}
        other => panic!("expected rejection, got {other:?}"),
    }
    // A commit that knows no omm root is refused before writing, rather than
    // writing unlocked or dropping a backup beside the host's file.
    let mut orphan = SettingsDoc::load(&path).expect("load");
    orphan
        .patch_typed(&[PatchOp::set("tui.theme", json!("custom:zz"))])
        .unwrap();
    assert!(matches!(
        orphan.commit(&h.inv, &CommitOptions::default()),
        Err(HostError::NoOmmRoot { .. })
    ));
    assert!(omm_footprint(path.parent().unwrap()).is_empty());
    assert_eq!(
        written,
        serde_json::from_slice::<serde_json::Value>(&std::fs::read(&path).unwrap()).unwrap(),
        "rejected commits leave the file untouched"
    );
}

#[test]
fn settings_backup_lands_in_omm_snapshots_not_beside_the_file() {
    // ARCHITECTURE §2: everything omm writes into Muse's config root is a
    // ledger entry; pre-write snapshots live under `$OMM/snapshots/<ts>/`.
    let h = host_or_skip!();
    let roots = roots(&h);
    let path = settings_path(&h);
    let cfg_dir = path.parent().unwrap().to_path_buf();
    let original = std::fs::read(&path).unwrap();
    let mut doc = SettingsDoc::for_roots(&roots).expect("load");
    doc.patch_typed(&[PatchOp::set("tui.theme", json!("custom:a"))])
        .unwrap();
    let report = doc
        .commit(&h.inv, &CommitOptions::default())
        .expect("commit");
    let beside = omm_footprint(&cfg_dir);
    assert!(
        beside.is_empty(),
        "footprint beside the host's file: {beside:?} (backup reported at {:?})",
        report.backup
    );
    let backup = report.backup.expect("a verified backup was taken");
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    let snapshots = roots.snapshots_dir();
    assert!(backup.starts_with(&snapshots), "{}", backup.display());
    assert!(
        backup.ends_with("muse-config/settings.json"),
        "{}",
        backup.display()
    );
    // Every commit gets its own snapshot directory; the newest five are kept.
    for i in 0..6 {
        let mut doc = SettingsDoc::for_roots(&roots).expect("load");
        doc.patch_typed(&[PatchOp::set("tui.theme", json!(format!("custom:{i}")))])
            .unwrap();
        doc.commit(&h.inv, &CommitOptions::default())
            .expect("commit");
    }
    let dirs = snapshots.read_dir().unwrap().flatten().count();
    assert_eq!(dirs, omm_host::fsx::SNAPSHOTS_KEEP, "rolling keep-5");
    assert!(omm_footprint(&cfg_dir).is_empty());
    // trust.json takes the same route.
    let mut store = TrustStore::for_roots(&roots).expect("load");
    store
        .merge_project(&h.sb.root, TrustDecision::Untrusted)
        .unwrap();
    let first = store.commit(&CommitOptions::default()).expect("commit");
    assert!(first.backup.is_none(), "nothing to back up on creation");
    let mut store = TrustStore::for_roots(&roots).expect("reload");
    store
        .merge_project(&h.sb.root, TrustDecision::Trusted)
        .unwrap();
    let second = store.commit(&CommitOptions::default()).expect("commit");
    let backup = second.backup.expect("backup");
    assert!(
        backup.ends_with("muse-config/trust.json"),
        "{}",
        backup.display()
    );
    assert!(omm_footprint(&cfg_dir).is_empty());
}

#[test]
fn concurrent_settings_commits_serialize_and_lose_nothing() {
    // Two writers that loaded the same document: exactly one may land, the
    // other must see `Stale`. Without a lock both passed the sha check (4 of 4
    // rounds measured) and one update vanished with no backup of it anywhere.
    let h = host_or_skip!();
    let roots = roots(&h);
    let path = settings_path(&h);
    let original = std::fs::read(&path).unwrap();
    let barrier = std::sync::Barrier::new(2);
    let results: Vec<Result<omm_host::settings::CommitReport, HostError>> =
        std::thread::scope(|scope| {
            let handles: Vec<_> = ["custom:A", "custom:B"]
                .into_iter()
                .map(|theme| {
                    let (inv, roots, barrier) = (&h.inv, &roots, &barrier);
                    scope.spawn(move || {
                        let mut doc = SettingsDoc::for_roots(roots).expect("load");
                        doc.patch_typed(&[PatchOp::set("tui.theme", json!(theme))])
                            .unwrap();
                        barrier.wait();
                        doc.commit(inv, &CommitOptions::default())
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|t| t.join().expect("thread"))
                .collect()
        });
    let written: Vec<&omm_host::settings::CommitReport> =
        results.iter().filter_map(|r| r.as_ref().ok()).collect();
    let stale = results
        .iter()
        .filter(|r| matches!(r, Err(HostError::Stale { .. })))
        .count();
    assert_eq!(
        written.len(),
        1,
        "exactly one commit may land; got {:?}",
        results
            .iter()
            .map(|r| r
                .as_ref()
                .map(|c| c.sha256.clone())
                .map_err(|e| e.to_string()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        stale, 1,
        "the other commit must be refused as Stale: {results:?}"
    );
    let on_disk = std::fs::read(&path).unwrap();
    assert_eq!(
        omm_host::fsx::sha256_bytes(&on_disk),
        written[0].sha256,
        "the file holds exactly the landed commit"
    );
    let backup = written[0].backup.clone().expect("backup");
    assert_eq!(std::fs::read(&backup).unwrap(), original);
    assert!(omm_footprint(path.parent().unwrap()).is_empty());
}

#[test]
fn settings_mcp_collision_is_refused() {
    let h = host_or_skip!();
    let path = settings_path(&h);
    std::fs::write(
        &path,
        b"{\"schema_version\":1,\"mcpServers\":{},\"mcp_servers\":{}}\n",
    )
    .unwrap();
    let mut doc = SettingsDoc::load(&path).expect("load");
    assert!(doc.has_mcp_collision());
    // Patching a colliding document is refused before anything is staged…
    assert!(matches!(
        doc.patch_typed(&[PatchOp::set("tui.theme", json!("x"))]),
        Err(HostError::McpCollision)
    ));
    // …and so is validating or committing it as-is.
    assert!(matches!(doc.validate(&h.inv), Err(HostError::McpCollision)));
    assert!(matches!(
        doc.commit(&h.inv, &CommitOptions::default()),
        Err(HostError::McpCollision)
    ));
    // The host agrees: a settings-mutating command exits 1 with the MCP error.
    let out = h
        .inv
        .run(&["skills", "disable", "bundled:doctor", "--scope", "built-in"])
        .unwrap();
    assert_eq!(out.kind, OutcomeKind::RunFailed);
    assert!(
        out.stderr.contains(hr::MCP_COLLISION_MESSAGE),
        "{}",
        out.stderr
    );
}

#[test]
fn settings_load_probe_rejects_malformed_documents() {
    let h = host_or_skip!();
    let ok = probe::settings_load_probe(&h.inv, b"{\"schema_version\":1}").unwrap();
    assert!(ok.accepted);
    let bad = probe::settings_load_probe(&h.inv, b"{\"schema_version\":1,\"provider\":1}").unwrap();
    assert!(!bad.accepted);
    assert!(
        bad.detail.starts_with(hr::SETTINGS_MALFORMED_MESSAGE),
        "{}",
        bad.detail
    );
    let v2 = probe::settings_load_probe(&h.inv, b"{\"schema_version\":2}").unwrap();
    assert!(!v2.accepted);
    assert!(
        v2.detail
            .starts_with(hr::SETTINGS_UNSUPPORTED_VERSION_MESSAGE),
        "{}",
        v2.detail
    );
    // A duplicate top-level field is malformed to the host (serde's typed
    // struct), even though a Value parse would silently keep the last one.
    let dup = probe::settings_load_probe(
        &h.inv,
        b"{\"schema_version\":1,\"tui\":{\"theme\":\"a\"},\"tui\":{\"theme\":\"b\"}}",
    )
    .unwrap();
    assert!(!dup.accepted, "{}", dup.detail);
    assert!(
        dup.detail.starts_with(hr::SETTINGS_MALFORMED_MESSAGE)
            && dup.detail.contains("duplicate field `tui`"),
        "{}",
        dup.detail
    );
    // And omm's own loader refuses the same bytes before any validator runs.
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    std::fs::write(
        &file,
        b"{\"schema_version\":1,\"tui\":{\"theme\":\"a\"},\"tui\":{\"theme\":\"b\"}}",
    )
    .unwrap();
    match SettingsDoc::load(&file) {
        Err(HostError::SettingsRejected { stage, detail }) => {
            assert_eq!(stage, "load");
            assert!(detail.contains("duplicate field `tui`"), "{detail}");
        }
        other => panic!("duplicate keys must be refused at load: {other:?}"),
    }
}

#[test]
fn trust_merge_is_read_by_the_host() {
    let h = host_or_skip!();
    let ws = h.sb.root.join("trusted-ws");
    std::fs::create_dir_all(&ws).unwrap();
    let trust_path = h.sb.config_home.join("muse").join("trust.json");
    // A store without `projects` is what the host accepts too: every
    // workspace untrusted, no `malformed trust store` (measured 2026-09-02).
    std::fs::write(&trust_path, b"{\"schema_version\":1}\n").unwrap();
    let out = h
        .inv
        .clone()
        .cwd(&ws)
        .run(&["exec", "--provider", "echo", "hi"])
        .expect("exec");
    assert!(out.ok(), "{}", out.stderr);
    assert!(
        !out.stderr.contains("malformed trust store"),
        "{}",
        out.stderr
    );
    let mut store = TrustStore::for_roots(&roots(&h)).expect("a projects-less store loads");
    assert!(store.existed());
    let merge = store
        .merge_project(&ws, TrustDecision::Trusted)
        .expect("merge");
    assert!(merge.changed);
    let commit = store.commit(&CommitOptions::default()).expect("commit");
    assert!(commit.backup.is_some());
    assert!(omm_footprint(trust_path.parent().unwrap()).is_empty());

    let out = h
        .inv
        .clone()
        .cwd(&ws)
        .run(&["exec", "--provider", "echo", "hi"])
        .expect("exec");
    assert!(out.ok(), "{}", out.stderr);
    assert!(
        out.stderr
            .contains("workspace trust: trusted source=user-config"),
        "host did not read our trust.json: {}",
        out.stderr
    );
    assert_eq!(
        TrustStore::load(&trust_path)
            .unwrap()
            .decision_for(&ws)
            .unwrap()
            .as_deref(),
        Some("trusted")
    );
}

#[test]
fn timeout_kill_sweeps_the_killed_hosts_registry_entry() {
    // A SessionStart hook that sleeps holds the session open; the budget
    // kills the host, and the `sessions/<uuid>.json` it registered under the
    // per-uid runtime dir (which no sandbox redirects) must not survive it.
    let h = host_or_skip!();
    let ws = h.sb.root.join("ws-wedge");
    std::fs::create_dir_all(&ws).unwrap();
    // The hook records its own pid: a plain SIGKILL of the host left this
    // `sleep` running as an orphan in its own process group (Gate 0
    // residual); the timeout kill must take the host's descendants with it.
    let hook_pid_file = ws.join("hook.pid");
    let hook_command = format!("echo $$ > '{}'; exec sleep 30", hook_pid_file.display());
    std::fs::write(
        settings_path(&h),
        serde_json::json!({
            "schema_version": 1,
            "hooks": {"SessionStart": [{"matcher": "*", "hooks": [
                {"type": "command", "command": hook_command, "timeout": 60}
            ]}]}
        })
        .to_string(),
    )
    .unwrap();
    let runtime = Roots::from_env().unwrap().runtime_dir().expect("uid known");
    let started = std::time::Instant::now();
    let budget = std::time::Duration::from_secs(3);
    let err = h
        .inv
        .clone()
        .cwd(&ws)
        .timeout(budget)
        .run(&["exec", "--provider", "echo", "hi"])
        .unwrap_err();
    let elapsed = started.elapsed();
    let pid = match &err {
        HostError::Timeout {
            pid,
            timeout,
            survivors,
            ..
        } => {
            assert_eq!(*timeout, budget);
            assert!(survivors.is_empty(), "descendants survived: {survivors:?}");
            *pid
        }
        other => panic!("expected Timeout, got {other:?}"),
    };
    assert!(
        elapsed < budget + std::time::Duration::from_secs(5),
        "{elapsed:?}"
    );
    let hook_pid: u32 = std::fs::read_to_string(&hook_pid_file)
        .expect("the SessionStart hook ran and wrote its pid before the kill")
        .trim()
        .parse()
        .unwrap();
    assert!(
        omm_host::residue::pids_alive(&std::collections::BTreeSet::from([hook_pid])).is_empty(),
        "the host's SessionStart hook (pid {hook_pid}, `sleep 30`) outlived the kill of host {pid}"
    );
    let needle = format!("\"pid={pid}\"");
    let leftovers: Vec<String> = walkdir::WalkDir::new(&runtime)
        .max_depth(2)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            std::fs::read_to_string(e.path())
                .map(|t| t.contains(&needle) || t.contains(&format!("\"pid\":{pid}")))
                .unwrap_or(false)
        })
        .map(|e| e.path().display().to_string())
        .collect();
    assert!(
        leftovers.is_empty(),
        "registry entries of killed pid {pid} survived under {}: {leftovers:?}",
        runtime.display()
    );
    // The sandbox itself is untouched by the sweep and the host is really gone.
    assert!(h.sb.config_home.join("muse").join("settings.json").exists());
    assert!(
        !omm_host::residue::pids_alive(&std::collections::BTreeSet::from([pid])).contains(&pid)
    );
}

#[test]
fn invoker_refuses_prompts_and_hides_process_start_env() {
    let h = host_or_skip!();
    assert!(matches!(
        h.inv.run(&["zzznotacommand"]),
        Err(HostError::Argv(_))
    ));
    // R20 behind root flags: none of these may reach the binary even when
    // root flags are allowed (each would start the TUI with `hi` as the prompt).
    // `-w hi`: the `[<MODE>]` flag takes a detached token only when it is one
    // of `off|create|existing`, so `hi` is the prompt (pty, 2026-09-02: 8
    // screen mentions, 5 in session.jsonl; `-w=hi` is exit 2).
    for argv in [
        vec!["--", "hi"],
        vec!["--provider", "echo", "hi"],
        vec!["--provider", "echo", "--", "hi"],
        vec!["-"],
        vec!["-w", "hi"],
        vec!["-w", "OFF"],
        vec!["--worktree", "hi"],
        vec!["-w", "off", "hi"],
    ] {
        assert!(
            matches!(
                h.inv.clone().allow_root_flags().run(&argv),
                Err(HostError::Argv(_))
            ),
            "{argv:?}"
        );
    }
    assert!(
        !h.sb.data_home.join("muse").join("sessions").exists(),
        "no session was started"
    );
    // A poisoned MUSE_ENABLE_WEB_TOOLS in *our* environment must not reach the host.
    std::env::set_var("MUSE_ENABLE_WEB_TOOLS", "bogus");
    let v = probe::version(&h.inv);
    std::env::remove_var("MUSE_ENABLE_WEB_TOOLS");
    assert!(v.is_ok(), "{v:?}");
    let _ = Path::new("/");
}
