//! `omm cost` — the context cost model (ARCHITECTURE.md §6 "Cost").
//!
//! Everything here is MEASURED from one live echo session per number, never
//! estimated from disk: the per-source byte table of the order-200 skills
//! catalog (bundled / filesystem / plugin, in the host's render order — plugin
//! is starved first), the refundable built-in tax (Meta's 20 built-ins:
//! 14,593 B of entries, 15,427 B as a block with the plugins gate on, 15,000 B
//! with 19 entries and the gate off — `hr::BUILTIN_SKILLS_CATALOG_*`; both
//! blocks include the 264-B `plugin:threejs:threejs` entry), the
//! memory snapshot (16,305 B / 48 files per scope, both silent), rules
//! (256,000 B per file, 65,536 B aggregate, warned on stderr), the 1,038-B
//! `workflow_cookbook` and the ~690-B `session_identity`, plus a "what to
//! cut" list where every saving is the byte difference between the baseline
//! session and one more session with exactly that setting applied to a
//! throwaway copy of the user's config root (`docs/experiments/context-slimming.md`).
//! Tokens are bytes/4, labelled as the documented heuristic (the host's own
//! `startup_estimated_tokens` is the same bytes/4 — measured 2026-09-02).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{json, Value};

use omm_host::host_reality as hr;
use omm_host::settings::{PatchOp, SettingsDoc};
use omm_host::{fsx, Invoker};

use crate::catalog::{Catalog, Source, SourceRow, Stage};
use crate::check::{Context, HostInfo};
use crate::checks::thousands;
use crate::error::{DoctorError, Result};
use crate::session::{self, LiveOptions, LiveSession, Seeded};

/// The documented heuristic (ARCHITECTURE.md §6 "Cost").
pub const TOKENS_PER_BYTE_DIVISOR: usize = 4;
/// How the token figures were derived.
pub const TOKEN_METHOD: &str = "bytes/4 — the documented heuristic (ARCHITECTURE.md §6); the host's own `skills list --json` startup_estimated_tokens is the same bytes/4 (measured 2026-09-02)";

/// Options for [`measure`].
#[derive(Clone, Debug)]
pub struct CostOptions {
    /// Measure the "what to cut" variants (four more echo sessions against a
    /// throwaway copy of the config root).
    pub cuts: bool,
}

impl Default for CostOptions {
    fn default() -> Self {
        CostOptions { cuts: true }
    }
}

/// One rendered catalog entry.
#[derive(Clone, Debug, Serialize)]
pub struct EntryCost {
    pub id: String,
    pub source: Source,
    pub bytes: usize,
    pub has_description: bool,
}

/// The order-200 block, per source.
#[derive(Clone, Debug, Serialize)]
pub struct CatalogCost {
    pub total_bytes: usize,
    pub cap_bytes: u64,
    pub headroom_bytes: u64,
    pub header_bytes: usize,
    pub footer_bytes: usize,
    pub entries: usize,
    pub with_description: usize,
    pub stage: Stage,
    /// In render order: bundled, filesystem, plugin.
    pub by_source: Vec<SourceRow>,
    pub entries_detail: Vec<EntryCost>,
    pub estimated_tokens: usize,
}

/// Meta's built-ins as rendered, refundable one by one.
#[derive(Clone, Debug, Serialize)]
pub struct BuiltinTax {
    pub entries: Vec<EntryCost>,
    pub entries_bytes: usize,
    /// `hr::BUILTIN_SKILLS_CATALOG_BYTES` (15 entries).
    pub documented_entries_bytes: u64,
    /// `hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES` / `_GATE_OFF`.
    pub documented_block_bytes_gate_on: u64,
    pub documented_block_bytes_gate_off: u64,
    pub refund_command: String,
}

/// Files and bytes under a directory.
#[derive(Clone, Debug, Serialize)]
pub struct DirUsage {
    pub path: PathBuf,
    pub files: usize,
    pub bytes: u64,
}

/// The memory snapshot scope caps vs what is on disk and what composed.
#[derive(Clone, Debug, Serialize)]
pub struct MemoryCost {
    pub personal: DirUsage,
    pub project: Option<DirUsage>,
    /// Bytes of the order-`u32::MAX` block in the live session, if it composed.
    pub live_block_bytes: Option<usize>,
    pub cap_bytes: u64,
    pub cap_files: usize,
}

/// One rules file.
#[derive(Clone, Debug, Serialize)]
pub struct FileUsage {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Rules caps vs the files that would load.
#[derive(Clone, Debug, Serialize)]
pub struct RulesCost {
    /// `$CONFIG_DIR/AGENTS.md`, the first rung of the personal ladder.
    pub personal_rules: Option<FileUsage>,
    /// `<workspace>/AGENTS.md` when a workspace is known.
    pub workspace_rules: Option<FileUsage>,
    pub live_block_bytes: Option<usize>,
    pub cap_file_bytes: u64,
    pub cap_aggregate_bytes: u64,
}

/// The workflow blocks.
#[derive(Clone, Debug, Serialize)]
pub struct CookbookCost {
    pub cookbook_bytes: Option<usize>,
    pub workflow_choice_bytes: Option<usize>,
    pub drop_setting: &'static str,
    pub documented_saving_bytes: u64,
    pub documented_wire_tool_bytes: u64,
}

/// The order-240 block.
#[derive(Clone, Debug, Serialize)]
pub struct SessionIdentityCost {
    pub bytes: Option<usize>,
    pub drop_setting: &'static str,
}

/// Token estimates, labelled.
#[derive(Clone, Debug, Serialize)]
pub struct TokenEstimate {
    pub run_context_bytes: usize,
    pub run_context_tokens: usize,
    pub catalog_tokens: usize,
    pub method: &'static str,
}

/// One thing to cut, with the bytes it buys.
#[derive(Clone, Debug, Serialize)]
pub struct Cut {
    pub title: String,
    /// The `settings.json` fragment (or `null` for a host command).
    pub setting: Value,
    /// The exact command(s).
    pub command: String,
    /// Run-context bytes saved, measured (`None` when the variant could not run).
    pub saves_bytes: Option<u64>,
    /// Catalog (order-200) bytes saved, measured.
    pub saves_catalog_bytes: Option<u64>,
    pub measured: bool,
    pub note: String,
}

/// One run-context block of the live session.
#[derive(Clone, Debug, Serialize)]
pub struct BlockRow {
    pub order: u64,
    pub id: String,
    pub bytes: usize,
}

/// What the baseline session looked like.
#[derive(Clone, Debug, Serialize)]
pub struct LiveSummary {
    pub trusted: bool,
    pub active_tools: usize,
    pub blocks: Vec<BlockRow>,
    pub seeded: Vec<Seeded>,
}

/// The whole cost report.
#[derive(Clone, Debug, Serialize)]
pub struct CostReport {
    pub host: HostInfo,
    pub live: LiveSummary,
    pub catalog: CatalogCost,
    pub builtin_tax: BuiltinTax,
    pub memory: MemoryCost,
    pub rules: RulesCost,
    pub cookbook: CookbookCost,
    pub session_identity: SessionIdentityCost,
    pub tokens: TokenEstimate,
    pub cuts: Vec<Cut>,
    pub notes: Vec<String>,
}

fn entry_costs<'a>(entries: impl Iterator<Item = &'a crate::catalog::Entry>) -> Vec<EntryCost> {
    entries
        .map(|e| EntryCost {
            id: e.id.clone(),
            source: e.source,
            bytes: e.bytes,
            has_description: e.description.is_some(),
        })
        .collect()
}

/// Measure everything for `ctx` (its live session is the baseline; the cut
/// variants run against a throwaway copy of the config root).
pub fn measure(ctx: &Context, opts: &CostOptions) -> Result<CostReport> {
    let live = ctx
        .live()
        .map_err(|e| DoctorError::Measure(format!("baseline echo session: {e}")))?;
    let mut notes = Vec::new();
    let catalog = live.catalog.clone().unwrap_or_else(|| {
        notes.push("the baseline session composed no skills_catalog block".to_string());
        Catalog::parse("")
    });
    let bundled = catalog.entries_of(Source::Bundled);
    let bundled_bytes: usize = bundled.iter().map(|e| e.bytes).sum();
    let builtin_tax = BuiltinTax {
        entries: entry_costs(bundled.iter().copied()),
        entries_bytes: bundled_bytes,
        documented_entries_bytes: hr::BUILTIN_SKILLS_CATALOG_BYTES,
        documented_block_bytes_gate_on: hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES,
        documented_block_bytes_gate_off: hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF,
        refund_command: hr::bundled_skills()
            .map(|b| b.catalog_cost.refund_command.clone())
            .unwrap_or_else(|_| "muse skills disable bundled:<id> --scope built-in".to_string()),
    };
    let catalog_cost = CatalogCost {
        total_bytes: catalog.total_bytes,
        cap_bytes: hr::SKILLS_CATALOG_CAP_BYTES,
        headroom_bytes: catalog.headroom_bytes(),
        header_bytes: catalog.header_bytes,
        footer_bytes: catalog.footer_bytes,
        entries: catalog.entries.len(),
        with_description: catalog.with_description(),
        stage: catalog.stage(),
        by_source: catalog.by_source(),
        entries_detail: entry_costs(catalog.entries.iter()),
        estimated_tokens: catalog.total_bytes / TOKENS_PER_BYTE_DIVISOR,
    };

    let ws = ctx.workspace();
    let memory = MemoryCost {
        personal: dir_usage(&ctx.roots.memory_personal_dir()),
        project: ws
            .as_deref()
            .and_then(|w| ctx.roots.memory_personal_project_dir(w).ok())
            .map(|p| dir_usage(&p)),
        live_block_bytes: live.block_bytes(hr::CONTEXT_ORDER_MEMORY_SNAPSHOT),
        cap_bytes: hr::MEMORY_SNAPSHOT_BYTES,
        cap_files: hr::MEMORY_LISTED_FILES_MAX,
    };
    let rules = RulesCost {
        personal_rules: file_usage(&ctx.roots.personal_rules_file()),
        workspace_rules: ws.as_deref().and_then(|w| file_usage(&w.join("AGENTS.md"))),
        live_block_bytes: live.block_bytes(hr::CONTEXT_ORDER_RULES_FILE),
        cap_file_bytes: hr::RULES_FILE_MAX_BYTES,
        cap_aggregate_bytes: hr::RULES_AGGREGATE_MAX_BYTES,
    };
    let cookbook = CookbookCost {
        cookbook_bytes: live.block_bytes(hr::CONTEXT_ORDER_WORKFLOW_COOKBOOK),
        workflow_choice_bytes: live.block_bytes(180),
        drop_setting: "run.workflow_trigger_mode: \"off\" (or run.context_slimming.excluded_tool_names: [\"workflow\"], 539 B cheaper)",
        documented_saving_bytes: hr::WORKFLOW_OFF_RUN_CONTEXT_SAVING_BYTES,
        documented_wire_tool_bytes: hr::WORKFLOW_TOOL_WIRE_BYTES,
    };
    let session_identity = SessionIdentityCost {
        bytes: live.block_bytes(hr::CONTEXT_ORDER_SESSION_IDENTITY),
        drop_setting: "run.context_slimming.session_identity_enabled: false",
    };
    let run_context_bytes = live.run_context_bytes();
    let tokens = TokenEstimate {
        run_context_bytes,
        run_context_tokens: run_context_bytes / TOKENS_PER_BYTE_DIVISOR,
        catalog_tokens: catalog.total_bytes / TOKENS_PER_BYTE_DIVISOR,
        method: TOKEN_METHOD,
    };

    let mut cuts = Vec::new();
    if opts.cuts {
        match measure_cuts(ctx, live) {
            Ok(measured) => cuts.extend(measured),
            Err(e) => notes.push(format!("settings variants not measured: {e}")),
        }
    } else {
        notes.push("settings variants not measured (cuts off)".to_string());
    }
    // Refunds are exact: disabling a built-in removes its rendered entry.
    let mut refunds: Vec<&crate::catalog::Entry> = bundled.clone();
    refunds.sort_by_key(|a| std::cmp::Reverse(a.bytes));
    for e in refunds {
        cuts.push(Cut {
            title: format!("disable {}", e.id),
            setting: Value::Null,
            command: builtin_tax.refund_command.replace("bundled:<id>", &e.id),
            saves_bytes: Some(e.bytes as u64),
            saves_catalog_bytes: Some(e.bytes as u64),
            measured: true,
            note: "the entry's rendered bytes in the baseline catalog (what the refund removes)"
                .to_string(),
        });
    }
    if bundled_bytes > 0 {
        cuts.push(Cut {
            title: "disable every built-in".to_string(),
            setting: Value::Null,
            command: bundled
                .iter()
                .map(|e| builtin_tax.refund_command.replace("bundled:<id>", &e.id))
                .collect::<Vec<_>>()
                .join("\n"),
            saves_bytes: Some(bundled_bytes as u64),
            saves_catalog_bytes: Some(bundled_bytes as u64),
            measured: true,
            note: format!(
                "the whole built-in tax ({} entries); the bundle budget grows from {} to {} B",
                bundled.len(),
                thousands(hr::BUNDLE_BUDGET_BYTES),
                thousands(hr::BUNDLE_BUDGET_BUILTINS_DISABLED_BYTES)
            ),
        });
    }

    Ok(CostReport {
        host: ctx.host_info(),
        live: LiveSummary {
            trusted: live.trusted,
            active_tools: live.facts.active_tools.len(),
            blocks: live
                .facts
                .context_blocks
                .iter()
                .map(|b| BlockRow {
                    order: b.order,
                    id: b.id.clone(),
                    bytes: b.bytes,
                })
                .collect(),
            seeded: live.seeded.clone(),
        },
        catalog: catalog_cost,
        builtin_tax,
        memory,
        rules,
        cookbook,
        session_identity,
        tokens,
        cuts,
        notes,
    })
}

/// A settings variant to measure.
struct Variant {
    title: &'static str,
    note: &'static str,
    ops: fn(&SettingsDoc) -> Vec<PatchOp>,
    command: fn(&SettingsDoc) -> String,
    setting: fn(&SettingsDoc) -> Value,
}

const SLIMMING: &str = hr::SETTINGS_CONTEXT_SLIMMING_KEY;

fn full_ids_with_default(doc: &SettingsDoc) -> Vec<Value> {
    let mut ids: Vec<Value> = doc
        .get(&[
            "run".to_string(),
            "context_slimming".to_string(),
            "full_skill_description_ids".to_string(),
        ])
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for d in hr::CONTEXT_SLIMMING_DEFAULT_FULL_IDS {
        if !ids.iter().any(|v| v.as_str() == Some(d)) {
            ids.push(Value::String(d.to_string()));
        }
    }
    ids
}

fn excluded_with_workflow(doc: &SettingsDoc) -> Vec<Value> {
    let mut names: Vec<Value> = doc
        .get(&[
            "run".to_string(),
            "context_slimming".to_string(),
            "excluded_tool_names".to_string(),
        ])
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !names.iter().any(|v| v.as_str() == Some("workflow")) {
        names.push(Value::String("workflow".to_string()));
    }
    names
}

fn variants() -> Vec<Variant> {
    vec![
        Variant {
            title: "first_sentence descriptions (bundled:git re-listed)",
            note: "cuts every <description> to its first sentence; the default full_skill_description_ids [\"bundled:git\"] is REPLACED by a user value, so it is re-listed (context-slimming.md §2, §6)",
            ops: |doc| {
                vec![
                    PatchOp::set(
                        &format!("{SLIMMING}.skill_catalog_descriptions"),
                        Value::String(hr::CONTEXT_SLIMMING_FIRST_SENTENCE.to_string()),
                    ),
                    PatchOp::set(
                        &format!("{SLIMMING}.full_skill_description_ids"),
                        Value::Array(full_ids_with_default(doc)),
                    ),
                ]
            },
            command: |doc| {
                format!(
                    "omm settings set {SLIMMING}.skill_catalog_descriptions {}\nomm settings set {SLIMMING}.full_skill_description_ids '{}'",
                    hr::CONTEXT_SLIMMING_FIRST_SENTENCE,
                    Value::Array(full_ids_with_default(doc))
                )
            },
            setting: |doc| {
                json!({"run": {"context_slimming": {
                    "skill_catalog_descriptions": hr::CONTEXT_SLIMMING_FIRST_SENTENCE,
                    "full_skill_description_ids": full_ids_with_default(doc)
                }}})
            },
        },
        Variant {
            title: "exclude the workflow tool",
            note: "removes the tool and the order-180 + 181 blocks with no replacement; the 15,766-B tool definition also leaves the wire (context-slimming.md §7.4)",
            ops: |doc| {
                vec![PatchOp::set(
                    &format!("{SLIMMING}.excluded_tool_names"),
                    Value::Array(excluded_with_workflow(doc)),
                )]
            },
            command: |doc| {
                format!(
                    "omm settings set {SLIMMING}.excluded_tool_names '{}'",
                    Value::Array(excluded_with_workflow(doc))
                )
            },
            setting: |doc| {
                json!({"run": {"context_slimming": {"excluded_tool_names": excluded_with_workflow(doc)}}})
            },
        },
        Variant {
            title: "session_identity off",
            note: "removes the order-240 session_identity block (context-slimming.md §2)",
            ops: |_| {
                vec![PatchOp::set(
                    &format!("{SLIMMING}.session_identity_enabled"),
                    Value::Bool(false),
                )]
            },
            command: |_| format!("omm settings set {SLIMMING}.session_identity_enabled false"),
            setting: |_| json!({"run": {"context_slimming": {"session_identity_enabled": false}}}),
        },
        Variant {
            title: "workflow_trigger_mode off",
            note: "drops the cookbook and the workflow tool but adds the 539-B workflow_availability_off block, so the exclusion above is cheaper (context-slimming.md §7.4)",
            ops: |_| {
                vec![PatchOp::set(
                    "run.workflow_trigger_mode",
                    Value::String("off".to_string()),
                )]
            },
            command: |_| "omm settings set run.workflow_trigger_mode off".to_string(),
            setting: |_| json!({"run": {"workflow_trigger_mode": "off"}}),
        },
    ]
}

/// Run the variants against a throwaway copy of the config root. The copy's
/// own unpatched session is the baseline of every difference, so the copy's
/// fidelity never leaks into a saving; `baseline` (the real config root) is
/// cross-checked against it and any gap is reported in the first cut's note.
fn measure_cuts(ctx: &Context, baseline: &LiveSession) -> Result<Vec<Cut>> {
    let tmp = tempfile::Builder::new()
        .prefix("omm-cost-")
        .tempdir()
        .map_err(|e| DoctorError::io("create temp dir", std::env::temp_dir(), e))?;
    let config_home = tmp.path().join("config");
    copy_config_root(&ctx.roots.muse_config(), &config_home.join("muse"))?;
    let settings_path = config_home.join("muse").join("settings.json");
    let base_bytes = if settings_path.exists() {
        Some(fsx::read_bytes(&settings_path)?)
    } else {
        None
    };
    let live_opts = LiveOptions {
        config_home: Some(config_home.clone()),
        seed: ctx.options.seed,
    };
    let copy_base = session::run(&ctx.inv, &ctx.roots, &live_opts)?;
    let fidelity = if copy_base.run_context_bytes() == baseline.run_context_bytes() {
        String::new()
    } else {
        format!(
            " (the config copy composed {} B of run context vs {} B in the real root; differences are against the copy)",
            thousands(copy_base.run_context_bytes() as u64),
            thousands(baseline.run_context_bytes() as u64)
        )
    };
    let mut cuts = Vec::new();
    for (i, v) in variants().iter().enumerate() {
        // Start every variant from the user's own document.
        restore(&settings_path, base_bytes.as_deref())?;
        let doc_result = SettingsDoc::load(&settings_path).and_then(|mut doc| {
            doc.patch_typed(&(v.ops)(&doc))?;
            Ok(doc)
        });
        let (setting, command, note_extra, result) = match doc_result {
            Ok(doc) => {
                let bytes = doc.to_bytes()?;
                fsx::write_atomic(&fsx::realpath_for_write(&settings_path)?, &bytes)?;
                let setting = (v.setting)(&doc);
                let command = (v.command)(&doc);
                (
                    setting,
                    command,
                    String::new(),
                    session::run(&ctx.inv, &ctx.roots, &live_opts),
                )
            }
            Err(e) => (
                Value::Null,
                String::new(),
                format!("; could not patch settings: {e}"),
                Err(DoctorError::Measure(e.to_string())),
            ),
        };
        let (saves, saves_catalog, measured, err) = match result {
            Ok(s) => (
                Some(
                    copy_base
                        .run_context_bytes()
                        .saturating_sub(s.run_context_bytes()) as u64,
                ),
                Some(copy_base.catalog_bytes().saturating_sub(s.catalog_bytes()) as u64),
                true,
                String::new(),
            ),
            Err(e) => (None, None, false, format!("; variant failed: {e}")),
        };
        cuts.push(Cut {
            title: v.title.to_string(),
            setting,
            command,
            saves_bytes: saves,
            saves_catalog_bytes: saves_catalog,
            measured,
            note: format!(
                "{}{}{}{}",
                v.note,
                if i == 0 { fidelity.as_str() } else { "" },
                note_extra,
                err
            ),
        });
    }
    Ok(cuts)
}

fn restore(settings_path: &Path, base: Option<&[u8]>) -> Result<()> {
    match base {
        Some(bytes) => {
            if let Some(parent) = settings_path.parent() {
                fsx::create_dir_all(parent)?;
            }
            fsx::write_atomic(&fsx::realpath_for_write(settings_path)?, bytes)?;
        }
        None => {
            if settings_path.exists() {
                std::fs::remove_file(settings_path)
                    .map_err(|e| DoctorError::io("remove", settings_path, e))?;
            }
        }
    }
    Ok(())
}

/// Files never copied into the throwaway config root: credentials and locks.
const CONFIG_COPY_SKIP: [&str; 2] = ["auth.json", ".auth.json.lock"];

/// Copy the host's config root (`$CONFIG_DIR/muse`) minus credentials and
/// lock files, symlinks skipped, every write atomic.
pub fn copy_config_root(src: &Path, dest: &Path) -> Result<()> {
    fsx::create_dir_all(dest)?;
    if !src.is_dir() {
        return Ok(());
    }
    let src = fsx::canonicalize(src)?;
    for entry in walkdir::WalkDir::new(&src)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy();
        if CONFIG_COPY_SKIP.contains(&name.as_ref()) || name.ends_with(".lock") {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(&src) else {
            continue;
        };
        let target = dest.join(rel);
        if let Some(parent) = target.parent() {
            fsx::create_dir_all(parent)?;
        }
        fsx::write_atomic(&target, &fsx::read_bytes(entry.path())?)?;
    }
    Ok(())
}

fn dir_usage(path: &Path) -> DirUsage {
    let mut files = 0usize;
    let mut bytes = 0u64;
    if path.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if entry.file_type().is_file() {
                files += 1;
                bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    DirUsage {
        path: path.to_path_buf(),
        files,
        bytes,
    }
}

fn file_usage(path: &Path) -> Option<FileUsage> {
    std::fs::metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| FileUsage {
            path: path.to_path_buf(),
            bytes: m.len(),
        })
}

impl CostReport {
    /// The `--json` document.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    /// The human table.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "omm cost — {} ({}) — one echo session, trusted fresh workspace, {} tools\n",
            self.host.binary.display(),
            self.host.version.as_deref().unwrap_or("version unknown"),
            self.live.active_tools
        ));
        out.push_str("\nrun context blocks\n");
        for b in &self.live.blocks {
            out.push_str(&format!(
                "  {:>10}  {:<24} {:>8} B\n",
                b.order,
                b.id,
                thousands(b.bytes as u64)
            ));
        }
        out.push_str(&format!(
            "  {:>10}  {:<24} {:>8} B  ≈ {} tokens ({})\n",
            "total",
            "",
            thousands(self.tokens.run_context_bytes as u64),
            thousands(self.tokens.run_context_tokens as u64),
            "bytes/4, heuristic"
        ));
        let c = &self.catalog;
        out.push_str(&format!(
            "\nskills catalog (order 200): {} B of {} ({} B headroom), {} entries / {} with description, header {} B, footer {} B, stage {:?}, ≈ {} tokens\n",
            thousands(c.total_bytes as u64),
            thousands(c.cap_bytes),
            thousands(c.headroom_bytes),
            c.entries,
            c.with_description,
            c.header_bytes,
            c.footer_bytes,
            c.stage,
            thousands(c.estimated_tokens as u64)
        ));
        out.push_str("  source (render order)  entries  with-desc     bytes\n");
        for r in &c.by_source {
            out.push_str(&format!(
                "  {:<21}  {:>7}  {:>9}  {:>8} B\n",
                r.source.label(),
                r.entries,
                r.with_description,
                thousands(r.bytes as u64)
            ));
        }
        let t = &self.builtin_tax;
        out.push_str(&format!(
            "\nbuilt-in tax: {} entries, {} B (documented {} B of entries; block {} B gate on / {} B gate off); refund: {}\n",
            t.entries.len(),
            thousands(t.entries_bytes as u64),
            thousands(t.documented_entries_bytes),
            thousands(t.documented_block_bytes_gate_on),
            thousands(t.documented_block_bytes_gate_off),
            t.refund_command
        ));
        let m = &self.memory;
        out.push_str(&format!(
            "memory: personal {} files / {} B at {}{}; live block {}; caps {} B / {} files per scope (silent)\n",
            m.personal.files,
            thousands(m.personal.bytes),
            m.personal.path.display(),
            m.project
                .as_ref()
                .map(|p| format!(", project {} files / {} B", p.files, thousands(p.bytes)))
                .unwrap_or_default(),
            m.live_block_bytes
                .map(|b| format!("{} B", thousands(b as u64)))
                .unwrap_or_else(|| "absent".to_string()),
            thousands(m.cap_bytes),
            m.cap_files
        ));
        let r = &self.rules;
        out.push_str(&format!(
            "rules: personal {}; workspace {}; live block {}; caps {} B/file, {} B aggregate (warned on stderr)\n",
            r.personal_rules
                .as_ref()
                .map(|f| format!("{} B", thousands(f.bytes)))
                .unwrap_or_else(|| "none".to_string()),
            r.workspace_rules
                .as_ref()
                .map(|f| format!("{} B", thousands(f.bytes)))
                .unwrap_or_else(|| "none".to_string()),
            r.live_block_bytes
                .map(|b| format!("{} B", thousands(b as u64)))
                .unwrap_or_else(|| "absent".to_string()),
            thousands(r.cap_file_bytes),
            thousands(r.cap_aggregate_bytes)
        ));
        let k = &self.cookbook;
        out.push_str(&format!(
            "workflow cookbook (181): {}; workflow_choice (180): {}; drop with {} (documented −{} B run context, −{} B tool JSON on the wire)\n",
            k.cookbook_bytes
                .map(|b| format!("{} B", thousands(b as u64)))
                .unwrap_or_else(|| "absent".to_string()),
            k.workflow_choice_bytes
                .map(|b| format!("{} B", thousands(b as u64)))
                .unwrap_or_else(|| "absent".to_string()),
            k.drop_setting,
            thousands(k.documented_saving_bytes),
            thousands(k.documented_wire_tool_bytes)
        ));
        out.push_str(&format!(
            "session_identity (240): {}; drop with {}\n",
            self.session_identity
                .bytes
                .map(|b| format!("{} B", thousands(b as u64)))
                .unwrap_or_else(|| "absent".to_string()),
            self.session_identity.drop_setting
        ));
        out.push_str("\nwhat to cut (bytes measured, not guessed)\n");
        for cut in &self.cuts {
            out.push_str(&format!(
                "  {:>9}  {}{}\n",
                cut.saves_bytes
                    .map(|b| format!("−{} B", thousands(b)))
                    .unwrap_or_else(|| "?".to_string()),
                cut.title,
                if cut.measured { "" } else { " (not measured)" }
            ));
            for line in cut.command.lines() {
                out.push_str(&format!("             {line}\n"));
            }
        }
        for n in &self.notes {
            out.push_str(&format!("note: {n}\n"));
        }
        out.push_str(&format!("tokens: {}\n", self.tokens.method));
        out
    }
}

/// Convenience for callers that hold only an invoker: the cost report needs
/// the same `Context` doctor uses.
pub fn context_for(inv: Invoker, roots: omm_host::Roots) -> Result<Context> {
    Context::new(inv, roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_ids_and_excluded_names_merge_with_existing_values() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        std::fs::write(
            &p,
            b"{\"schema_version\":1,\"run\":{\"context_slimming\":{\"full_skill_description_ids\":[\"cs-00\"],\"excluded_tool_names\":[\"web_search\"]}}}",
        )
        .unwrap();
        let doc = SettingsDoc::load(&p).unwrap();
        let ids = full_ids_with_default(&doc);
        assert_eq!(ids, vec![json!("cs-00"), json!("bundled:git")]);
        let names = excluded_with_workflow(&doc);
        assert_eq!(names, vec![json!("web_search"), json!("workflow")]);
        // Already present: not duplicated.
        std::fs::write(
            &p,
            b"{\"schema_version\":1,\"run\":{\"context_slimming\":{\"full_skill_description_ids\":[\"bundled:git\"],\"excluded_tool_names\":[\"workflow\"]}}}",
        )
        .unwrap();
        let doc = SettingsDoc::load(&p).unwrap();
        assert_eq!(full_ids_with_default(&doc).len(), 1);
        assert_eq!(excluded_with_workflow(&doc).len(), 1);
        // A missing document: the default id and workflow only.
        let doc = SettingsDoc::load(&dir.path().join("none.json")).unwrap();
        assert_eq!(full_ids_with_default(&doc), vec![json!("bundled:git")]);
        assert_eq!(variants().len(), 4);
    }

    #[test]
    fn config_copy_skips_credentials_and_locks() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("muse");
        std::fs::create_dir_all(src.join("skills/x")).unwrap();
        std::fs::write(src.join("settings.json"), b"{}").unwrap();
        std::fs::write(src.join("auth.json"), b"secret").unwrap();
        std::fs::write(src.join(".auth.json.lock"), b"").unwrap();
        std::fs::write(src.join("skills/.muse.lock"), b"").unwrap();
        std::fs::write(src.join("skills/x/SKILL.md"), b"---\nname: x\n---\n").unwrap();
        let dest = dir.path().join("copy");
        copy_config_root(&src, &dest).unwrap();
        assert!(dest.join("settings.json").exists());
        assert!(dest.join("skills/x/SKILL.md").exists());
        assert!(!dest.join("auth.json").exists());
        assert!(!dest.join(".auth.json.lock").exists());
        assert!(!dest.join("skills/.muse.lock").exists());
        let usage = dir_usage(&dest);
        assert_eq!(usage.files, 2);
        assert!(file_usage(&dest.join("settings.json")).is_some());
        assert!(file_usage(&dest.join("nope")).is_none());
        copy_config_root(&dir.path().join("absent"), &dir.path().join("copy2")).unwrap();
    }
}
