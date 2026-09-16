//! `omm cost` against the real binary: the numbers of `docs/host-reality.md`
//! "Budgets" reproduced by measurement, and every "what to cut" saving
//! measured rather than guessed.

mod common;

use omm_doctor::catalog::{Source, Stage};
use omm_doctor::cost::{self, CostOptions};
use omm_doctor::Options;
use omm_host::host_reality as hr;

fn live_options() -> Options {
    Options {
        live: true,
        host_drift: false,
        ..Options::default()
    }
}

#[test]
fn pristine_catalog_reproduces_the_bundled_tax_gate_off_and_on() {
    let h = host_or_skip!();
    // Gate off: 20 entries (the 19 visible bundled skills plus the second
    // builtin plugin's skill, `plugin:threejs:threejs`), 15,000 B.
    let ctx = h.ctx().with_options(live_options());
    let report = cost::measure(&ctx, &CostOptions { cuts: false }).unwrap();
    let c = &report.catalog;
    assert_eq!(
        c.total_bytes as u64,
        hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF
    );
    assert_eq!(c.header_bytes as u64, hr::SKILLS_CATALOG_HEADER_BYTES);
    assert_eq!(c.footer_bytes as u64, hr::SKILLS_CATALOG_FOOTER_BYTES);
    assert_eq!(c.entries, hr::BUNDLED_SKILLS_VISIBLE_DEFAULT + 1);
    assert_eq!(c.with_description, c.entries);
    assert_eq!(c.stage, Stage::Intact);
    assert_eq!(c.by_source.len(), 2);
    assert_eq!(c.by_source[0].source, Source::Bundled);
    assert_eq!(c.by_source[0].entries, hr::BUNDLED_SKILLS_VISIBLE_DEFAULT);
    assert_eq!(c.by_source[1].source, Source::Plugin);
    assert_eq!(c.by_source[1].entries, 1);
    assert!(c
        .entries_detail
        .iter()
        .any(|e| e.id == "plugin:threejs:threejs"));
    assert_eq!(
        c.headroom_bytes,
        hr::SKILLS_CATALOG_CAP_BYTES - hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF
    );
    assert_eq!(c.estimated_tokens, c.total_bytes / 4);
    assert_eq!(
        report.tokens.run_context_tokens,
        report.tokens.run_context_bytes / 4
    );
    assert!(report.tokens.method.contains("bytes/4"));
    assert_eq!(
        report.cookbook.cookbook_bytes,
        Some(hr::WORKFLOW_COOKBOOK_BYTES as usize)
    );
    assert_eq!(
        report.cookbook.workflow_choice_bytes,
        Some(hr::WORKFLOW_CHOICE_BYTES as usize)
    );
    // The block embeds the session-log path, so its size follows the temp
    // root's path length (776–786 B under the experiment's sandbox path,
    // context-slimming.md §2; ~740 B under a shorter one).
    let sid = report.session_identity.bytes.unwrap() as u64;
    assert!((600..=1_000).contains(&sid), "{sid}");
    assert!(report.live.trusted);
    assert_eq!(report.live.active_tools, hr::ACTIVE_TOOLS);
    assert_eq!(report.memory.cap_bytes, hr::MEMORY_SNAPSHOT_BYTES);
    assert_eq!(
        report.rules.cap_aggregate_bytes,
        hr::RULES_AGGREGATE_MAX_BYTES
    );
    assert!(report.notes.iter().any(|n| n.contains("cuts off")));
    // Refund rows: one per built-in plus the total, exact entry bytes.
    let refunds: Vec<&cost::Cut> = report
        .cuts
        .iter()
        .filter(|c| c.title.starts_with("disable bundled:"))
        .collect();
    assert_eq!(refunds.len(), hr::BUNDLED_SKILLS_VISIBLE_DEFAULT);
    assert!(
        refunds
            .windows(2)
            .all(|w| w[0].saves_bytes >= w[1].saves_bytes),
        "sorted largest first"
    );
    let browser = refunds
        .iter()
        .find(|c| c.title == "disable bundled:browser-app-delivery")
        .unwrap();
    assert_eq!(
        browser.saves_bytes,
        Some(1_912),
        "host-reality: browser-app-delivery alone is 1,912 B"
    );
    assert_eq!(
        browser.command,
        "muse skills disable bundled:browser-app-delivery --scope built-in"
    );
    let all = report
        .cuts
        .iter()
        .find(|c| c.title == "disable every built-in")
        .unwrap();
    assert_eq!(
        all.saves_bytes,
        Some(report.builtin_tax.entries_bytes as u64)
    );

    // Gate on: 21 entries (20 bundled plus threejs), 15,427 B,
    // bundled entries 14,593 B.
    let ctx = h
        .ctx_with(h.inv.clone().with_plugins_gate())
        .with_options(live_options());
    let report = cost::measure(&ctx, &CostOptions { cuts: false }).unwrap();
    assert_eq!(
        report.catalog.total_bytes as u64,
        hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES
    );
    assert_eq!(report.catalog.entries, hr::BUNDLED_SKILLS + 1);
    assert_eq!(
        report.builtin_tax.entries_bytes as u64,
        hr::BUILTIN_SKILLS_CATALOG_BYTES
    );
    assert_eq!(
        report.catalog.header_bytes as u64
            + report.builtin_tax.entries_bytes as u64
            + hr::THREEJS_PLUGIN_CATALOG_BYTES
            + report.catalog.footer_bytes as u64,
        hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES
    );
    let text = report.render();
    assert!(
        text.contains("built-in tax: 20 entries, 14,593 B"),
        "{text}"
    );
    assert!(
        text.contains("skills catalog (order 200): 15,427 B of 32,000"),
        "{text}"
    );
    let json = report.to_json();
    assert_eq!(json["catalog"]["total_bytes"], 15_427);
    assert_eq!(
        json["builtin_tax"]["entries"].as_array().unwrap().len(),
        hr::BUNDLED_SKILLS
    );
}

#[test]
fn cuts_are_measured_against_host_reality_constants() {
    let h = host_or_skip!();
    // A user skill and a settings document with existing lists, to prove the
    // variants merge instead of replace.
    h.user_skills(1, 40);
    h.write_settings(serde_json::json!({"schema_version": 1, "run": {"context_slimming": {"excluded_tool_names": ["web_search"]}}}));
    let ctx = h
        .ctx_with(h.inv.clone().with_plugins_gate())
        .with_options(live_options());
    let report = cost::measure(&ctx, &CostOptions::default()).unwrap();
    let by_title = |t: &str| {
        report
            .cuts
            .iter()
            .find(|c| c.title.starts_with(t))
            .unwrap_or_else(|| {
                panic!(
                    "no cut {t}: {:?}",
                    report.cuts.iter().map(|c| &c.title).collect::<Vec<_>>()
                )
            })
    };
    let base_catalog = report.catalog.total_bytes as u64;
    assert_eq!(report.catalog.entries, hr::BUNDLED_SKILLS + 2);
    assert_eq!(report.catalog.by_source.len(), 3);
    assert_eq!(report.catalog.by_source[0].source, Source::Bundled);
    assert_eq!(report.catalog.by_source[0].entries, hr::BUNDLED_SKILLS);
    assert_eq!(report.catalog.by_source[1].source, Source::Filesystem);
    assert_eq!(report.catalog.by_source[2].source, Source::Plugin);
    assert_eq!(report.catalog.by_source[2].entries, 1);

    // first_sentence: the 20 built-ins go 15,427 → 7,076 B (bundled:git re-listed);
    // the one user skill loses its second sentence too.
    let fs = by_title("first_sentence");
    assert!(fs.measured, "{fs:?}");
    let user_entry = report
        .catalog
        .entries_detail
        .iter()
        .find(|e| e.source == Source::Filesystem)
        .unwrap();
    let bundled_saving =
        hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES - hr::BUILTIN_SKILLS_FIRST_SENTENCE_BLOCK_BYTES;
    let saved = fs.saves_catalog_bytes.unwrap();
    assert!(
        saved > bundled_saving && saved < bundled_saving + user_entry.bytes as u64,
        "saved {saved}, bundled {bundled_saving}, user entry {}",
        user_entry.bytes
    );
    assert_eq!(
        fs.saves_bytes, fs.saves_catalog_bytes,
        "first_sentence touches only the catalog"
    );
    assert!(
        fs.command
            .contains("skill_catalog_descriptions first_sentence"),
        "{}",
        fs.command
    );
    assert!(fs.command.contains("\"bundled:git\""), "{}", fs.command);
    assert_eq!(
        fs.setting["run"]["context_slimming"]["full_skill_description_ids"],
        serde_json::json!(["bundled:git"])
    );
    assert!(fs.note.contains("re-listed"));

    // Excluding the workflow tool removes orders 180 + 181: −4,459 B, catalog untouched.
    let wf = by_title("exclude the workflow tool");
    assert_eq!(
        wf.saves_bytes,
        Some(hr::WORKFLOW_EXCLUDED_RUN_CONTEXT_SAVING_BYTES),
        "{wf:?}"
    );
    assert_eq!(wf.saves_catalog_bytes, Some(0));
    assert_eq!(
        wf.setting["run"]["context_slimming"]["excluded_tool_names"],
        serde_json::json!(["web_search", "workflow"]),
        "merged with the user's list"
    );

    // session_identity off: exactly the order-240 block (path-dependent size,
    // 776–786 B under the experiment's sandbox path; the two temp roots here
    // have equal-length paths, so the saving is the block within a few bytes).
    let sid = by_title("session_identity off");
    let s = sid.saves_bytes.unwrap();
    let block = report.session_identity.bytes.unwrap() as u64;
    assert!(s.abs_diff(block) <= 16, "saved {s} vs block {block}");
    assert!((600..=1_000).contains(&s), "{s}");
    assert_eq!(sid.saves_catalog_bytes, Some(0));

    // workflow_trigger_mode off: −3,920 B (539 B dearer than the exclusion).
    let off = by_title("workflow_trigger_mode off");
    assert_eq!(
        off.saves_bytes,
        Some(hr::WORKFLOW_OFF_RUN_CONTEXT_SAVING_BYTES),
        "{off:?}"
    );
    assert_eq!(
        wf.saves_bytes.unwrap() - off.saves_bytes.unwrap(),
        hr::WORKFLOW_AVAILABILITY_OFF_BYTES
    );

    // Refunds are entry bytes; the whole tax is their sum and never exceeds the block.
    let all = by_title("disable every built-in");
    assert_eq!(all.saves_bytes, Some(hr::BUILTIN_SKILLS_CATALOG_BYTES));
    assert!(all.saves_bytes.unwrap() < base_catalog);
    let text = report.render();
    assert!(
        text.contains("what to cut (bytes measured, not guessed)"),
        "{text}"
    );
    assert!(
        text.contains("−4,459 B  exclude the workflow tool"),
        "{text}"
    );
    // The user's real settings.json was never touched by the variants.
    let on_disk: serde_json::Value =
        serde_json::from_slice(&std::fs::read(h.settings_path()).unwrap()).unwrap();
    assert_eq!(
        on_disk["run"]["context_slimming"]["excluded_tool_names"],
        serde_json::json!(["web_search"])
    );
    assert!(on_disk["run"]["context_slimming"]
        .get("skill_catalog_descriptions")
        .is_none());
    assert!(!h.sb.data_home.join("muse").join("sessions").exists());
}

#[test]
fn installed_plugin_composes_in_the_measurement_from_a_seeded_data_root() {
    let h = host_or_skip!();
    h.install_and_approve("omm-five");
    let ctx = h
        .ctx()
        .with_plugin_id("omm-five")
        .with_options(live_options());
    let report = cost::measure(&ctx, &CostOptions { cuts: false }).unwrap();
    let plugin_rows: Vec<_> = report
        .catalog
        .by_source
        .iter()
        .filter(|r| r.source == Source::Plugin)
        .collect();
    assert_eq!(plugin_rows.len(), 1, "{:?}", report.catalog.by_source);
    assert_eq!(plugin_rows[0].entries, 2, "{:?}", report.catalog.by_source);
    assert!(report
        .catalog
        .entries_detail
        .iter()
        .any(|e| e.id == "plugin:threejs:threejs"));
    assert!(report
        .catalog
        .entries_detail
        .iter()
        .any(|e| e.id == "plugin:omm-five:omm-fx"));
    // Render order: bundled first, plugin last.
    assert_eq!(
        report.catalog.entries_detail.first().map(|e| e.source),
        Some(Source::Bundled)
    );
    assert_eq!(
        report.catalog.entries_detail.last().map(|e| e.source),
        Some(Source::Plugin)
    );
    assert!(
        report
            .live
            .seeded
            .iter()
            .any(|s| s.what == "plugins" && s.files > 0),
        "{:?}",
        report.live.seeded
    );
    // Since 1.3.0-R3057.1 `snooze_reminder` is in the default set, so an
    // approved reminder adds no tool: 29, not 30.
    assert_eq!(report.live.active_tools, hr::ACTIVE_TOOLS);
    assert!(!h.sb.data_home.join("muse").join("sessions").exists());
}
