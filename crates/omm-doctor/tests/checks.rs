//! PLAN.md 1.3 acceptance — every check both ways against the real binary:
//! one pristine pass and one planted failure, each in its own sandbox.

mod common;

use serde_json::json;

use omm_doctor::checks;
use omm_doctor::{Options, Severity};
use omm_host::probe;
use omm_host::settings::CommitOptions;
use omm_host::trust::{TrustDecision, TrustStore};

use common::write_ledger;

fn run(ctx: &omm_doctor::Context, id: &str) -> omm_doctor::Check {
    checks::run_one(ctx, id).unwrap_or_else(|| panic!("no check {id}"))
}

// ---- D1 --------------------------------------------------------------------

#[test]
fn d1_pristine_all_trusted_enabled() {
    let h = host_or_skip!();
    h.install_and_approve("omm-five");
    let ctx = h.ctx().with_plugin_id("omm-five");
    let c = run(&ctx, "D1");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.starts_with("3/3 trusted_enabled (manifest)"),
        "{}",
        c.observed
    );
    assert!(c.fix.is_none());
    assert!(c.why_silent.contains("trusted_enabled"));
}

#[test]
fn d1_planted_disable_is_critical_and_the_printed_fix_restores_green() {
    // e2e scenario 5: install → `muse plugins disable omm` → D1 critical with
    // the exact fix; run it; green.
    let h = host_or_skip!();
    h.install_and_approve("omm-five");
    h.muse(&["plugins", "disable", "omm-five", "--json"]);
    let ctx = h.ctx().with_plugin_id("omm-five");
    let c = run(&ctx, "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("disabled"), "{}", c.observed);
    assert!(
        c.observed.contains("plugin:omm-five:mcp_server:omm-mcp"),
        "{}",
        c.observed
    );
    let fix = c.fix.clone().expect("fix");
    assert_eq!(fix, "muse plugins enable omm-five");
    let argv: Vec<&str> = fix.split_whitespace().skip(1).collect();
    h.muse(&argv);
    let again = run(&h.ctx().with_plugin_id("omm-five"), "D1");
    assert_eq!(again.severity, Severity::Info, "{again:?}");
}

#[test]
fn d1_planted_reject_names_the_capability_and_the_approve_command() {
    let h = host_or_skip!();
    h.install_and_approve("omm-five");
    h.muse(&[
        "plugins",
        "reject",
        "plugin:omm-five:hook:omm-hook",
        "--json",
    ]);
    let ctx = h.ctx().with_plugin_id("omm-five");
    let c = run(&ctx, "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(
        c.observed.starts_with("2/3 trusted_enabled"),
        "{}",
        c.observed
    );
    assert!(
        c.observed
            .contains("plugin:omm-five:hook:omm-hook trusted_disabled"),
        "{}",
        c.observed
    );
    assert_eq!(
        c.fix.as_deref(),
        Some("muse plugins approve plugin:omm-five:hook:omm-hook")
    );
    h.muse(&[
        "plugins",
        "approve",
        "plugin:omm-five:hook:omm-hook",
        "--json",
    ]);
    assert_eq!(
        run(&h.ctx().with_plugin_id("omm-five"), "D1").severity,
        Severity::Info
    );
}

#[test]
fn d1_planted_not_installed_and_ledger_expecting_more_than_declared() {
    let h = host_or_skip!();
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("not installed"), "{}", c.observed);
    assert_eq!(c.fix.as_deref(), Some("omm install"));
    // Installed and approved, but the ledger says a capability was approved
    // that the installed package no longer declares → ABSENT → reinstall.
    h.install_and_approve("omm-five");
    let ctx = h
        .ctx()
        .with_plugin_id("omm-five")
        .with_expected_capabilities(vec![
            "plugin:omm-five:hook:omm-hook".into(),
            "plugin:omm-five:mcp_server:omm-gone".into(),
        ]);
    let c = run(&ctx, "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("(ledger)"), "{}", c.observed);
    assert!(
        c.observed
            .contains("plugin:omm-five:mcp_server:omm-gone ABSENT"),
        "{}",
        c.observed
    );
    assert_eq!(c.fix.as_deref(), Some("omm install --reinstall"));
}

#[test]
fn d1_manifest_fallback_leaves_a_capability_shipped_disabled_alone() {
    // Gate 1 `doc`: with no usable ledger D1 demanded approval of the
    // reminder that ships `enabledDefault: false` — a fix that would switch
    // on what the author shipped off.
    let h = host_or_skip!();
    let mut caps = omm_host::fixtures::five_family_capabilities().unwrap();
    caps["reminders"][0]["enabledDefault"] = json!(false);
    let spec = omm_host::fixtures::PackageSpec::native("omm-off", caps);
    let root =
        omm_host::fixtures::write_package(&h.sb.root.join("pkgs"), "omm-off", &spec).unwrap();
    h.muse(&["plugins", "install", root.to_str().unwrap(), "--json"]);
    let c = run(&h.ctx().with_plugin_id("omm-off"), "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    let fix = c.fix.clone().expect("fix");
    assert!(
        !fix.contains("reminder:omm-rem"),
        "the fix must not approve a capability shipped off: {fix}"
    );
    assert!(
        c.observed.contains("left review_needed by design"),
        "{}",
        c.observed
    );
    assert!(
        c.observed.contains("0/2 trusted_enabled (manifest)"),
        "{}",
        c.observed
    );
    for line in fix.lines() {
        let argv: Vec<&str> = line.split_whitespace().skip(1).collect();
        h.muse(&argv);
    }
    let c = run(&h.ctx().with_plugin_id("omm-off"), "D1");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.starts_with("2/2 trusted_enabled (manifest)"),
        "{}",
        c.observed
    );
    assert!(
        c.observed.contains("plugin:omm-off:reminder:omm-rem"),
        "the skipped capability is still named: {}",
        c.observed
    );
    // A ledger that lists the reminder as approved (the user enabled it by
    // hand) still asserts it: the ledger, not the manifest, decides then.
    let ctx = h
        .ctx()
        .with_plugin_id("omm-off")
        .with_expected_capabilities(vec!["plugin:omm-off:reminder:omm-rem".into()]);
    let c = run(&ctx, "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert_eq!(
        c.fix.as_deref(),
        Some("muse plugins approve plugin:omm-off:reminder:omm-rem")
    );
}

#[test]
fn d1_and_d10_know_a_managed_store_install() {
    // Gate 1 `np` / e2e s02: after `omm install --no-plugin` D1 was critical
    // "plugin omm is not installed" with a fix install itself refused.
    let h = host_or_skip!();
    let src = h.sb.root.join("skill-src").join("omm-ms");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("SKILL.md"),
        "---\nname: omm-ms\ndescription: Managed-store doctor fixture; do not use it for real work.\n---\n\nBody.\n",
    )
    .unwrap();
    h.muse(&[
        "skills",
        "install",
        src.to_str().unwrap(),
        "--scope",
        "user",
        "--json",
    ]);
    write_ledger(
        &h.roots,
        &json!({"schema_version": 1, "omm_version": "0.1.0", "scope": "user",
            "entries": [
                {"base": "muse-config", "path": "skills/omm-ms/SKILL.md", "kind": "skill", "sha256": "x", "source_version": "0.1.0", "writer": "omm install", "mechanism": "muse-skills-install", "class": "exclusive", "prior": null},
                {"base": "muse-config", "path": "AGENTS.md", "kind": "rules", "sha256": "y", "mechanism": "copy", "class": "seeded"}
            ],
            "registrations": []}),
    );
    let c = run(&h.ctx(), "D1");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("managed-store install"),
        "{}",
        c.observed
    );
    assert!(c.observed.contains("1/1"), "{}", c.observed);
    assert!(
        c.observed.contains("no plugin capabilities to approve"),
        "{}",
        c.observed
    );
    let c = run(&h.ctx(), "D10");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("managed-store install"),
        "{}",
        c.observed
    );
    // A ledgered skill the host no longer lists: critical, and the fix is
    // the install mode the ledger records.
    h.muse(&["skills", "uninstall", "omm-ms", "--json"]);
    let c = run(&h.ctx(), "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("omm-ms"), "{}", c.observed);
    assert_eq!(c.fix.as_deref(), Some("omm install --no-plugin"));
    // No ledger: the plugin lane as before.
    std::fs::remove_file(h.roots.omm_root().join("omm.lock.json")).unwrap();
    let c = run(&h.ctx(), "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert_eq!(c.fix.as_deref(), Some("omm install"));
}

#[test]
fn doctor_derives_the_managed_store_mode_from_the_host_without_a_ledger() {
    // Gate 1 decision E: with the ledger corrupt or absent, the mode comes
    // from the host — managed-store `<pid>-*` skills and no plugin — and
    // every fix D1, D10 and D13 print is the managed-store one (round 4:
    // they printed `omm install`, which stacked the bundle on top of twelve
    // managed skills and never converged).
    let h = host_or_skip!();
    let src = h.sb.root.join("skill-src").join("omm-ms");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("SKILL.md"),
        "---\nname: omm-ms\ndescription: Managed-store doctor fixture; do not use it for real work.\n---\n\nBody.\n",
    )
    .unwrap();
    h.muse(&[
        "skills",
        "install",
        src.to_str().unwrap(),
        "--scope",
        "user",
        "--json",
    ]);
    // Corrupt ledger.
    let path = write_ledger(&h.roots, &json!({"schema_version": 1}));
    std::fs::write(&path, b"{corrupt").unwrap();
    let c = run(&h.ctx(), "D13");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert_eq!(
        c.fix.as_deref(),
        Some("omm reconcile\nomm install --no-plugin"),
        "{c:?}"
    );
    let c = run(&h.ctx(), "D1");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("managed-store") && c.observed.contains("host"),
        "{}",
        c.observed
    );
    assert!(c.observed.contains("omm-ms"), "{}", c.observed);
    let c = run(&h.ctx(), "D10");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("managed-store"), "{}", c.observed);
    // Absent ledger: nothing records the store's omm skills — a warning with
    // the managed-store fix, not "omm never installed here".
    std::fs::remove_file(&path).unwrap();
    let c = run(&h.ctx(), "D13");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(c.observed.contains("omm-ms"), "{}", c.observed);
    assert_eq!(
        c.fix.as_deref(),
        Some("omm reconcile\nomm install --no-plugin"),
        "{c:?}"
    );
    // A ledgered skill the host no longer lists, judged from the host: D1
    // names it with the managed-store fix.
    h.muse(&["skills", "uninstall", "omm-ms", "--json"]);
    let c = run(&h.ctx(), "D13");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("omm never installed here"),
        "{}",
        c.observed
    );
    let c = run(&h.ctx(), "D1");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert_eq!(c.fix.as_deref(), Some("omm install"));
}

// ---- D2 --------------------------------------------------------------------

#[test]
fn d2_provider_set_or_unset_is_info_and_warns_only_when_omm_wrote_it() {
    let h = host_or_skip!();
    h.write_settings(json!({"schema_version": 1, "provider": "meta"}));
    let c = run(&h.ctx(), "D2");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert_eq!(c.observed, "provider = \"meta\"");
    // Gate 1 decision: a pristine install never sets `provider` (the host's
    // login flow does), so an unset key is Info — naming what breaks and
    // the command that sets it — never a false positive.
    h.write_settings(json!({"schema_version": 1}));
    let c = run(&h.ctx(), "D2");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.fix.is_none());
    assert!(c.observed.contains("one-tool session"), "{}", c.observed);
    assert!(
        c.observed.contains("omm settings set provider meta"),
        "{}",
        c.observed
    );
    assert!(c.why_silent.contains("muse serve"));
    // A wrong type is still a warning.
    h.write_settings(json!({"schema_version": 1, "provider": 3}));
    let c = run(&h.ctx(), "D2");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    // Warn only when the ledger shows omm itself wrote the key and it is
    // gone now (omm removed or changed it).
    common::write_ledger(
        &h.roots,
        &json!({
            "schema_version": 1, "omm_version": "0.1.0", "scope": "user",
            "host": {"version": "v", "sha256": "s"},
            "entries": [],
            "registrations": [
                {"kind": "settings-key", "path": "provider", "prior": "openai", "value": "meta"}
            ]
        }),
    );
    h.write_settings(json!({"schema_version": 1}));
    let c = run(&h.ctx(), "D2");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert_eq!(c.fix.as_deref(), Some("omm settings set provider meta"));
    assert!(
        c.observed.contains("ledger records omm wrote it"),
        "{}",
        c.observed
    );
    assert!(c.observed.contains("\"openai\""), "{}", c.observed);
}

// ---- D3 --------------------------------------------------------------------

#[test]
fn d3_collision_is_critical_legacy_alone_warns_canonical_passes() {
    let h = host_or_skip!();
    h.write_settings(json!({"schema_version": 1, "mcpServers": {}}));
    assert_eq!(run(&h.ctx(), "D3").severity, Severity::Info);
    h.write_settings(json!({"schema_version": 1, "mcpServers": {}, "mcp_servers": {}}));
    let c = run(&h.ctx(), "D3");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert_eq!(c.fix.as_deref(), Some("omm settings fix-mcp-collision"));
    assert!(c.observed.contains("exits 1"), "{}", c.observed);
    // The host agrees: a settings-mutating verb fails in this state.
    let out = h
        .inv
        .run(&["skills", "disable", "bundled:doctor", "--scope", "built-in"])
        .unwrap();
    assert!(!out.ok());
    h.write_settings(json!({"schema_version": 1, "mcp_servers": {}}));
    let c = run(&h.ctx(), "D3");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("legacy `mcp_servers`"),
        "{}",
        c.observed
    );
}

// ---- D4 --------------------------------------------------------------------

#[test]
fn d4_shape_lint_both_stores_and_the_file_itself() {
    let h = host_or_skip!();
    h.install_and_approve("omm-five");
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("3 entries"), "{}", c.observed);
    // One malformed entry under an unrelated key takes both wired reminders down.
    h.write_settings(json!({"schema_version": 1, "runtime_capabilities": {"a": 1}}));
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("`runtime_capabilities.a` is a number"),
        "{}",
        c.observed
    );
    assert_eq!(c.fix.as_deref(), Some("omm settings lint --fix"));
    h.write_settings(json!({"schema_version": 1, "plugins": "garbage"}));
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("`plugins` is a string"),
        "{}",
        c.observed
    );
    h.write_settings(json!({"schema_version": 1, "plugins": {"skill-reminder": {"enabled": "x"}}}));
    let c = run(&h.ctx(), "D4");
    assert!(
        c.observed
            .contains("`plugins.skill-reminder.enabled` is a string"),
        "{}",
        c.observed
    );
    // Keys the host destroys on its next rewrite.
    h.write_settings(json!({"schema_version": 1, "_omm": 1, "tui": {"theme": "x", "zz": 1}}));
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("_omm") && c.observed.contains("tui.zz"),
        "{}",
        c.observed
    );
    // A malformed file: every settings-consuming command exits 1.
    h.write_settings_raw(b"{\"schema_version\":1,\"tui\":{},\"tui\":{}}");
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("duplicate field"), "{}", c.observed);
    let d2 = run(&h.ctx(), "D2");
    assert_eq!(d2.severity, Severity::Warn);
    assert!(d2.observed.contains("see D4"), "{}", d2.observed);
}

#[test]
fn d4_names_a_permissions_object_missing_its_structural_schema_version() {
    // Gate 1 decision D: `permissions` is a deny_unknown_fields struct with a
    // required `schema_version`; without it the host refuses the whole
    // object (`Named permission profiles are unavailable: missing field
    // schema_version`) on stderr only, and every named profile is inert.
    let h = host_or_skip!();
    h.write_settings(json!({"schema_version": 1, "permissions": {"schema_version": 1, "profiles": {"mine": {"extends": ":ask-me", "approval": "prompt_unmatched", "reviewer": "human"}}}}));
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    h.write_settings(json!({"schema_version": 1, "permissions": {"profiles": {"mine": {"extends": ":ask-me", "approval": "prompt_unmatched", "reviewer": "human"}}}}));
    let c = run(&h.ctx(), "D4");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("`permissions`") && c.observed.contains("schema_version"),
        "{}",
        c.observed
    );
    assert_eq!(
        c.fix.as_deref(),
        Some("omm settings set permissions.schema_version 1"),
        "{c:?}"
    );
    // An empty object is the host's own default shape: nothing to warn about.
    h.write_settings(json!({"schema_version": 1, "permissions": {}}));
    assert_eq!(run(&h.ctx(), "D4").severity, Severity::Info);
}

// ---- D5 --------------------------------------------------------------------

#[test]
fn d5_reminder_conflict() {
    let h = host_or_skip!();
    h.write_settings(json!({"schema_version": 1,
        "plugins": {"skill-reminder": {"enabled": true}},
        "runtime_capabilities": {"plugin:tbh-reminders:reminder:skill-reminder": {"enabled": true}}}));
    let c = run(&h.ctx(), "D5");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.starts_with("2 wired reminder ids agree"),
        "{}",
        c.observed
    );
    h.write_settings(json!({"schema_version": 1,
        "plugins": {"skill-reminder": {"enabled": false}},
        "runtime_capabilities": {"plugin:tbh-reminders:reminder:skill-reminder": {"enabled": true}}}));
    let c = run(&h.ctx(), "D5");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed
            .contains("skill-reminder: legacy enabled=true vs canonical enabled=false"),
        "{}",
        c.observed
    );
    assert_eq!(c.fix.as_deref(), Some("omm settings reconcile-reminders"));
    // Canonical alone is a no-op — reported, not failed.
    h.write_settings(
        json!({"schema_version": 1, "plugins": {"goal-reminder": {"enabled": false}}}),
    );
    let c = run(&h.ctx(), "D5");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed
            .contains("goal-reminder: canonical enabled=false with no legacy entry"),
        "{}",
        c.observed
    );
}

// ---- D6 --------------------------------------------------------------------

#[test]
fn d6_safe_mode() {
    let h = host_or_skip!();
    assert_eq!(run(&h.ctx(), "D6").severity, Severity::Info);
    h.write_settings(json!({"schema_version": 1, "agent_definitions": {"safe_mode": true}}));
    let c = run(&h.ctx(), "D6");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert_eq!(
        c.fix.as_deref(),
        Some("omm settings set agent_definitions.safe_mode false")
    );
    // The host still loads the file (typed key, right type).
    assert!(h.inv.run(&["skills", "list", "--json"]).unwrap().ok());
}

// ---- D7 --------------------------------------------------------------------

#[test]
fn d7_plugin_count_counts_enabled_plugins_from_the_host() {
    let h = host_or_skip!();
    h.install_fixture("omm-a");
    h.install_fixture("omm-b");
    h.install_fixture("omm-c");
    h.muse(&["plugins", "disable", "omm-c", "--json"]);
    let c = run(&h.ctx().with_plugin_id("omm-a"), "D7");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.starts_with("2 enabled of 3 installed"),
        "{}",
        c.observed
    );
    // The threshold is the tunable; the count is the host's.
    let ctx = h.ctx().with_plugin_id("omm-a").with_options(Options {
        live: false,
        host_drift: false,
        plugin_count_warn_at: 2,
        ..Options::default()
    });
    let c = run(&ctx, "D7");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.fix
            .as_deref()
            .unwrap()
            .starts_with("muse plugins disable omm-b"),
        "{:?}",
        c.fix
    );
    let ctx = h.ctx().with_plugin_id("omm-a").with_options(Options {
        live: false,
        host_drift: false,
        plugin_count_warn_at: 1,
        plugin_count_max: 1,
        ..Options::default()
    });
    let c = run(&ctx, "D7");
    assert!(
        c.observed.contains("plugin_preflight_overflow"),
        "{}",
        c.observed
    );
}

// ---- D8 --------------------------------------------------------------------

#[test]
fn d8_with_the_live_session_off_is_an_info_row_that_says_so() {
    // Gate 1: `omm doctor --fast` reported D8 as WARN ("no live
    // measurement"), so a fast doctor could never be green (zero warn /
    // critical) even on a pristine install.
    let h = host_or_skip!();
    let c = run(&h.ctx(), "D8");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("disabled by options"), "{}", c.observed);
    assert!(c.observed.contains("--fast"), "{}", c.observed);
    assert!(c.fix.is_none());
}

#[test]
fn d8_pristine_catalog_is_the_bundled_tax_only() {
    let h = host_or_skip!();
    let ctx = h.ctx().with_options(Options {
        live: true,
        host_drift: false,
        ..Options::default()
    });
    let c = run(&ctx, "D8");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed
            .starts_with("20 entries / 20 with description / 15,000 B of 32,000"),
        "{}",
        c.observed
    );
    // The per-source row is entries only: 15,000 − 535 header − 264 threejs − 35 footer.
    assert!(
        c.observed.contains("bundled 19/19 14,166 B"),
        "{}",
        c.observed
    );
    assert!(c.observed.contains("(17,000 B headroom"), "{}", c.observed);
}

#[test]
fn d8_planted_hundred_skills_drop_descriptions_silently() {
    // context-slimming.md §2 `big_full`: 100 user skills → descriptions
    // dropped tail-first with rc 0 and `diagnostics: []`.
    let h = host_or_skip!();
    h.user_skills(100, 180);
    let ctx = h.ctx().with_options(Options {
        live: true,
        host_drift: false,
        ..Options::default()
    });
    let c = run(&ctx, "D8");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("stage 2"), "{}", c.observed);
    assert!(c.observed.contains("120 entries"), "{}", c.observed);
    let fix = c.fix.clone().unwrap();
    assert!(fix.contains("muse skills disable bundled:"), "{fix}");
    assert!(fix.contains("first_sentence"), "{fix}");
    assert!(fix.contains("omm cost"), "{fix}");
    // The host itself reports nothing: every skill listed, no diagnostics.
    let list = probe::skills_list(&h.inv, &Default::default()).unwrap();
    assert_eq!(list.skills.len(), 120);
    assert!(list.diagnostics.is_empty());
}

// ---- D9 --------------------------------------------------------------------

#[test]
fn d9_enterprise_probe_rows_and_a_failing_host() {
    let h = host_or_skip!();
    let c = run(&h.ctx(), "D9");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.starts_with("4 sources:"), "{}", c.observed);
    assert!(
        c.observed.contains("defaults/system_file absent"),
        "{}",
        c.observed
    );
    assert!(
        c.observed.contains("generation matches host-reality"),
        "{}",
        c.observed
    );
    assert!(c.fix.is_none());
    let fake = h.fake_host(
        "d9",
        "if [ \"$1\" = config ] && [ \"$2\" = status ]; then echo boom >&2; exit 1; fi",
    );
    let c = run(&h.ctx_with(fake), "D9");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("`muse config status` failed"),
        "{}",
        c.observed
    );
}

// ---- D10 -------------------------------------------------------------------

#[test]
fn d10_native_passes_foreign_family_is_critical() {
    let h = host_or_skip!();
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D10");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("not installed"), "{}", c.observed);
    h.install_and_approve("omm-five");
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D10");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("manifest_family = \"native\""),
        "{}",
        c.observed
    );
    h.install_claude_fixture("omm-cl");
    let c = run(&h.ctx().with_plugin_id("omm-cl"), "D10");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("claude-compatible"), "{}", c.observed);
    assert_eq!(c.fix.as_deref(), Some("omm install --reinstall"));
}

#[test]
fn d10_notes_a_rotated_source_and_a_missing_marketplace_registration() {
    let h = host_or_skip!();
    let root = h.install_and_approve("omm-five");
    std::fs::remove_dir_all(&root).unwrap();
    write_ledger(
        &h.roots,
        &json!({"schema_version": 1, "omm_version": "0.1.0", "scope": "user", "entries": [],
            "registrations": [{"kind": "muse-marketplace", "name": "ohmy-test", "source": "/nowhere"}]}),
    );
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D10");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("is gone"), "{}", c.observed);
    assert!(
        c.observed
            .contains("marketplace `ohmy-test` recorded in the ledger is no longer configured"),
        "{}",
        c.observed
    );
}

// ---- D11 -------------------------------------------------------------------

#[test]
fn d11_host_drift_pristine_passes_and_a_moved_constant_is_named() {
    let h = host_or_skip!();
    let ctx = h.ctx().with_options(Options {
        live: false,
        host_drift: true,
        ..Options::default()
    });
    let c = run(&ctx, "D11");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("rows match host-reality.md"),
        "{}",
        c.observed
    );
    let fake = h.fake_host(
        "d11",
        "if [ \"$1\" = config ] && [ \"$2\" = status ]; then \"$REAL\" \"$@\" | sed 's/^Generation: .*/Generation: sha256:0000/'; exit ${PIPESTATUS[0]}; fi",
    );
    let ctx = h.ctx_with(fake).with_options(Options {
        live: false,
        host_drift: true,
        ..Options::default()
    });
    let c = run(&ctx, "D11");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("enterprise/generation"),
        "{}",
        c.observed
    );
    assert_eq!(c.fix.as_deref(), Some("omm doctor --report-drift"));
    let off = h.ctx();
    assert!(run(&off, "D11").observed.contains("skipped"));
}

// ---- D12 -------------------------------------------------------------------

#[test]
fn d12_trust_present_absent_and_malformed() {
    let h = host_or_skip!();
    let ws = h.workspace("ws");
    let c = run(&h.ctx(), "D12");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(c.observed.contains("no trust.json"), "{}", c.observed);
    assert_eq!(
        c.fix.as_deref(),
        Some(format!("omm trust {}", ws.display()).as_str())
    );
    let mut store = TrustStore::for_roots(&h.roots).unwrap();
    store.merge_project(&ws, TrustDecision::Trusted).unwrap();
    store.commit(&CommitOptions::default()).unwrap();
    let c = run(&h.ctx(), "D12");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.ends_with("is trusted"), "{}", c.observed);
    // Another workspace: recorded projects counted, this one absent.
    let other = h.workspace("other");
    let c = run(&h.ctx().with_workspace(Some(other)), "D12");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("(1 project(s) recorded)"),
        "{}",
        c.observed
    );
    // Untrusted decision.
    let mut store = TrustStore::for_roots(&h.roots).unwrap();
    store.merge_project(&ws, TrustDecision::Untrusted).unwrap();
    store.commit(&CommitOptions::default()).unwrap();
    assert!(run(&h.ctx(), "D12").observed.contains("`untrusted`"));
    // Malformed store: every session aborts.
    std::fs::write(
        h.roots.trust_file(),
        b"{\"schema_version\":1,\"projects\":{\"/x\":{\"decision\":\"trust\"}}}",
    )
    .unwrap();
    let c = run(&h.ctx(), "D12");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("malformed"), "{}", c.observed);
    let out = h
        .inv
        .clone()
        .cwd(&ws)
        .run(&["exec", "--provider", "echo", "hi"])
        .unwrap();
    assert!(!out.ok(), "the host aborts on a malformed trust store");
}

// ---- D13 -------------------------------------------------------------------

#[test]
fn d13_ledger_states() {
    let h = host_or_skip!();
    // Absent, nothing installed: informational.
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed.contains("omm never installed here"),
        "{}",
        c.observed
    );
    // Absent but the plugin is installed: nothing records what omm wrote.
    // The fix rebuilds the ledger from the host, then lets install adopt
    // what a rebuilt ledger cannot list (Gate 1: `omm reconcile` alone left
    // "present but unlisted" with the same no-op fix).
    h.install_and_approve("omm-five");
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert_eq!(c.fix.as_deref(), Some("omm reconcile\nomm install"));
    // Corrupt.
    let path = write_ledger(&h.roots, &json!({"schema_version": 1}));
    std::fs::write(&path, b"{corrupt").unwrap();
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(c.observed.contains("corrupt"), "{}", c.observed);
    assert_eq!(c.fix.as_deref(), Some("omm reconcile\nomm install"));
    // Pristine: every ledgered file present, registration matches the host.
    let skill = h
        .roots
        .personal_skills_dir()
        .join("omm-fx")
        .join("SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::fs::write(
        &skill,
        b"---\nname: omm-fx\ndescription: x. Do not use.\n---\n",
    )
    .unwrap();
    let theme = h.roots.themes_dir().join("omm-carbon.tmTheme");
    std::fs::create_dir_all(theme.parent().unwrap()).unwrap();
    std::fs::write(&theme, b"<plist/>").unwrap();
    let ledger = json!({"schema_version": 1, "omm_version": "0.1.0", "scope": "user",
        "entries": [
            {"base": "muse-config", "path": "skills/omm-fx/SKILL.md", "kind": "skill", "sha256": "x", "source_version": "0.1.0", "writer": "omm install", "mechanism": "copy", "class": "exclusive", "prior": null},
            {"base": "muse-config", "path": "themes/omm-carbon.tmTheme", "kind": "theme", "sha256": "y", "mechanism": "copy", "class": "exclusive"},
            {"base": "muse-config", "path": "settings.json#tui.theme", "kind": "settings-key", "mechanism": "settings-patch", "prior": null}
        ],
        "registrations": [{"kind": "muse-plugin", "id": "omm-five", "approved": ["plugin:omm-five:hook:omm-hook", "plugin:omm-five:mcp_server:omm-mcp", "plugin:omm-five:reminder:omm-rem"]}]});
    write_ledger(&h.roots, &ledger);
    let ctx = h.ctx().with_plugin_id("omm-five");
    let c = run(&ctx, "D13");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(
        c.observed
            .contains("3 entries (2 files), 1 registration(s); every file present and contained"),
        "{}",
        c.observed
    );
    // The ledger's approved list feeds D1.
    assert_eq!(ctx.expected_capabilities().map(|v| v.len()), Some(3));
    assert!(run(&ctx, "D1").observed.contains("(ledger)"));
    // Vanished + unlisted.
    std::fs::remove_file(&theme).unwrap();
    // The unlisted scan keys on the managed content-id prefix (`omm-`,
    // whatever the plugin is named); a directory outside it is a user's own.
    for name in ["omm-five-zz", "mine-zz"] {
        let extra = h.roots.personal_skills_dir().join(name).join("SKILL.md");
        std::fs::create_dir_all(extra.parent().unwrap()).unwrap();
        std::fs::write(&extra, format!("---\nname: {name}\n---\n")).unwrap();
    }
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed
            .contains("1 ledgered file(s) vanished: muse-config:themes/omm-carbon.tmTheme"),
        "{}",
        c.observed
    );
    assert!(
        c.observed
            .contains("1 present but unlisted: muse-config:skills/omm-five-zz/SKILL.md"),
        "{}",
        c.observed
    );
    assert!(!c.observed.contains("mine-zz/"), "{}", c.observed);
    // A vanished file needs both: reconcile drops the entry, install recreates.
    assert_eq!(c.fix.as_deref(), Some("omm reconcile\nomm install"));
    // Unlisted alone: install adopts it, nothing to reconcile.
    std::fs::write(&theme, b"<plist/>").unwrap();
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert_eq!(c.fix.as_deref(), Some("omm install"), "{}", c.observed);
    // A workspace trust entry with no registration is current but
    // unregistered: named, never a warning on its own.
    let ws = h.workspace("ws");
    let mut store = TrustStore::load(&h.roots.trust_file()).unwrap();
    store.merge_project(&ws, TrustDecision::Trusted).unwrap();
    store
        .commit(&CommitOptions::for_roots(&h.roots))
        .expect("trust commit");
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert!(
        c.observed.contains("current but unregistered")
            && c.observed.contains(&format!(
                "trust {} (trusted)",
                TrustStore::key_for(&ws).unwrap()
            )),
        "{}",
        c.observed
    );
    std::fs::remove_file(&theme).unwrap();
    // Registration drift: the plugin is gone.
    h.muse(&["plugins", "remove", "omm-five", "--delete-data", "--json"]);
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert!(
        c.observed
            .contains("ledger registers plugin `omm-five` but the host has no such plugin"),
        "{}",
        c.observed
    );
    // A `--no-plugin` ledger (`muse-skills-install` entries, no `muse-plugin`
    // registration): the fix is spelled in that mode — a plain `omm install`
    // refuses such a ledger, so the printed fix would not run (Gate 1).
    let managed_skill = h
        .roots
        .personal_skills_dir()
        .join("omm-ms")
        .join("SKILL.md");
    std::fs::create_dir_all(managed_skill.parent().unwrap()).unwrap();
    std::fs::write(
        &managed_skill,
        b"---\nname: omm-ms\ndescription: x. Do not use.\n---\n",
    )
    .unwrap();
    let mut managed = json!({"schema_version": 1, "omm_version": "0.1.0", "scope": "user",
        "entries": [
            {"base": "muse-config", "path": "skills/omm-ms/SKILL.md", "kind": "skill", "sha256": "x", "source_version": "0.1.0", "writer": "omm install", "mechanism": "muse-skills-install", "class": "exclusive", "prior": null},
            {"base": "muse-config", "path": "skills/omm-gone/SKILL.md", "kind": "skill", "sha256": "x", "source_version": "0.1.0", "writer": "omm install", "mechanism": "muse-skills-install", "class": "exclusive", "prior": null}
        ],
        "registrations": []});
    write_ledger(&h.roots, &managed);
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed
            .contains("1 ledgered file(s) vanished: muse-config:skills/omm-gone/SKILL.md"),
        "{}",
        c.observed
    );
    assert_eq!(
        c.fix.as_deref(),
        Some("omm reconcile\nomm install --no-plugin"),
        "{}",
        c.observed
    );
    // Unlisted alone (`omm-five-zz` from above), same mode.
    managed["entries"].as_array_mut().unwrap().pop();
    write_ledger(&h.roots, &managed);
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Warn, "{c:?}");
    assert!(
        c.observed.contains("present but unlisted"),
        "{}",
        c.observed
    );
    assert_eq!(
        c.fix.as_deref(),
        Some("omm install --no-plugin"),
        "{}",
        c.observed
    );
    // An entry that escapes its base is critical.
    let mut escaped = ledger.clone();
    escaped["entries"][0]["path"] = json!("../../escape");
    write_ledger(&h.roots, &escaped);
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D13");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(
        c.observed
            .contains("not contained in their base: muse-config:../../escape"),
        "{}",
        c.observed
    );
}

// ---- D14 -------------------------------------------------------------------

#[test]
fn d14_exit_matrix_and_allowlist_pristine_and_a_host_that_accepts_a_typo() {
    let h = host_or_skip!();
    let c = run(&h.ctx(), "D14");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("rows pass"), "{}", c.observed);
    // A host that treats an unknown verb as success would let a wrapper typo through.
    let fake = h.fake_host(
        "d14",
        "if [ \"$1\" = skills ] && [ \"$2\" = zzznotaverb ]; then exit 0; fi",
    );
    let c = run(&h.ctx_with(fake), "D14");
    assert_eq!(c.severity, Severity::Critical, "{c:?}");
    assert!(
        c.observed.contains("exit/unknown-subcommand"),
        "{}",
        c.observed
    );
    assert!(c.fix.is_some());
    // No session was started by any of the argv rows.
    assert!(!h.sb.data_home.join("muse").join("sessions").exists());
}

// ---- D15 -------------------------------------------------------------------

#[test]
fn d15_reads_the_program_words_the_host_holds_and_resolves_them_on_path() {
    use omm_doctor::checks::d15_omm_on_path::{evaluate, programs, Inputs};
    let h = host_or_skip!();
    // Nothing installed: nothing to spawn, never a finding.
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D15");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("not installed"), "{}", c.observed);
    // The fixture's hook is `sh …` and its MCP server `python3 …`; both are
    // on the test process's PATH, which is what the host inherits.
    h.install_and_approve("omm-five");
    let ctx = h.ctx().with_plugin_id("omm-five");
    let c = run(&ctx, "D15");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
    assert!(c.observed.contains("`sh` →"), "{}", c.observed);
    assert!(c.observed.contains("`python3` →"), "{}", c.observed);
    assert!(
        c.observed.contains("1 hook(s), 0 MCP server(s)"),
        "{}",
        c.observed
    );
    assert!(
        c.observed.contains("0 hook(s), 1 MCP server(s)"),
        "{}",
        c.observed
    );
    assert!(c.why_silent.contains("PATH"), "{}", c.why_silent);
    // The words come from `plugins inspect`, not from any catalog; with an
    // empty PATH the same words are the planted failure, with its fix.
    let ins = ctx.inspect().expect("inspect");
    let words = programs(&ins.raw["plugin"]);
    assert_eq!(
        words.keys().cloned().collect::<Vec<_>>(),
        vec!["python3".to_string(), "sh".to_string()]
    );
    let planted = evaluate(&Inputs {
        programs: words,
        path: Some(h.sb.root.join("empty").into()),
        cache_path: None,
        current_exe: None,
    });
    assert_eq!(planted.severity, Severity::Critical, "{planted:?}");
    assert!(
        planted.observed.contains("`sh` is not on PATH"),
        "{}",
        planted.observed
    );
    assert!(
        planted.observed.contains("silently never starts"),
        "{}",
        planted.observed
    );
    let fix = planted.fix.expect("fix");
    assert!(
        fix.contains("install `sh` or add its directory to PATH"),
        "{fix}"
    );
    assert!(
        fix.contains("install `python3` or add its directory to PATH"),
        "{fix}"
    );
    // A disabled plugin still lists its capabilities: the words are still checked.
    h.muse(&["plugins", "disable", "omm-five", "--json"]);
    let c = run(&h.ctx().with_plugin_id("omm-five"), "D15");
    assert_eq!(c.severity, Severity::Info, "{c:?}");
}

// ---- the whole report ------------------------------------------------------

#[test]
fn full_report_after_a_correct_install_is_green_and_renders() {
    let h = host_or_skip!();
    h.install_and_approve("omm-five");
    h.write_settings(json!({"schema_version": 1, "provider": "meta"}));
    // Re-approve after the settings rewrite (approvals live in settings.json).
    probe::plugins_approve(&h.inv, "omm-five").unwrap();
    let ws = h.workspace("ws");
    let mut store = TrustStore::for_roots(&h.roots).unwrap();
    store.merge_project(&ws, TrustDecision::Trusted).unwrap();
    store.commit(&CommitOptions::default()).unwrap();
    let ctx = h
        .ctx()
        .with_plugin_id("omm-five")
        .with_options(Options::default());
    let report = omm_doctor::run(&ctx);
    let ids: Vec<&str> = report.checks.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids.len(), 15);
    assert_eq!(ids[0], "D1");
    assert_eq!(ids[13], "D14");
    assert_eq!(ids[14], "D15");
    for c in &report.checks {
        assert_ne!(c.severity, Severity::Critical, "{c:?}\n{}", report.render());
        assert!(!c.why_silent.is_empty(), "{c:?}");
    }
    assert!(report.ok, "{}", report.render());
    assert_eq!(report.exit_code(), 0);
    // The only warning on a pristine install is the ledger no installer wrote yet.
    let warns: Vec<&str> = report.failures().iter().map(|c| c.id.as_str()).collect();
    assert_eq!(warns, vec!["D13"], "{}", report.render());
    // D8 measured a real session with the plugin's skill composed.
    let d8 = report.get("D8").unwrap();
    assert!(d8.observed.contains("plugin 2/2"), "{}", d8.observed);
    assert!(
        d8.observed.contains("21 entries / 21 with description"),
        "{}",
        d8.observed
    );
    // Rendering: one line per check, fixes indented under failed rows.
    let text = report.render();
    assert!(text.lines().count() >= 16, "{text}");
    assert!(text.contains("        fix: omm reconcile"), "{text}");
    let json = report.to_json();
    assert_eq!(json["ok"], true);
    assert_eq!(json["checks"].as_array().unwrap().len(), 15);
    assert_eq!(json["host"]["plugin_id"], "omm-five");
    assert_eq!(json["checks"][0]["severity"], "info");
    // The sandbox data root holds no session: the measurement used its own root.
    assert!(!h.sb.data_home.join("muse").join("sessions").exists());
}
