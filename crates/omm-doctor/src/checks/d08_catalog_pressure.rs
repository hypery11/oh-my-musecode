//! D8 — catalog pressure from a real echo session (ARCHITECTURE.md §6 row D8):
//! entries / entries-with-description / bytes of 32,000, and the silent
//! stage-2 description drop.

use omm_host::host_reality as hr;

use crate::catalog::{Source, Stage};
use crate::check::{Check, Context};
use crate::checks::thousands;

pub const ID: &str = "D8";
pub const TITLE: &str = "catalog pressure";
/// `docs/host-reality.md` "Budgets": the order-200 `skills_catalog` is capped
/// at 32,000 B and degrades in three stages — stage 2 drops descriptions
/// tail-first, COMPLETELY SILENTLY (`skills list --json` still reports every
/// skill with `diagnostics: []`, rc 0); only stage 3 (entries dropped) prints
/// on stderr. Render order is bundled → filesystem → plugin, so the plugin's
/// skills are the first to lose their descriptions (00-DECISION.md §1.2 rows
/// 36–37; context-slimming.md §2 `big_full`: 48 of 114 descriptions gone, no
/// diagnostic). The block itself, read from `session.jsonl`, is the only oracle.
pub const WHY_SILENT: &str = "past 32,000 B the host drops skill descriptions tail-first — plugin skills first — with rc 0, `skills list --json` still listing every skill and `diagnostics: []`; only the later entry drop prints on stderr; the composed block in session.jsonl is the only oracle (host-reality.md \"Budgets\": degradation, render order; 00-DECISION.md §1.2 rows 36–37)";
/// Warn once the block is within this many bytes of the cap.
pub const HEADROOM_WARN_BYTES: u64 = 3_200;

pub fn run(ctx: &Context) -> Check {
    if !ctx.options.live {
        // `omm doctor --fast` turns the echo session off on purpose: a
        // probe the user chose to skip is not a finding (Gate 1: it warned,
        // so a fast doctor could never be green on a pristine install).
        return Check::info(
            ID,
            TITLE,
            "not measured: the live echo session is disabled by options (--fast); `omm doctor` without --fast or `omm cost` measures the catalog",
            WHY_SILENT,
        );
    }
    let live = match ctx.live() {
        Ok(l) => l,
        Err(e) => {
            return Check::warn(
                ID,
                TITLE,
                format!("no live measurement: {e}"),
                WHY_SILENT,
                "omm cost",
            )
        }
    };
    let Some(catalog) = &live.catalog else {
        return Check::warn(
            ID,
            TITLE,
            "the echo session composed no order-200 skills_catalog block",
            WHY_SILENT,
            "omm cost",
        );
    };
    let per_source: Vec<String> = catalog
        .by_source()
        .iter()
        .map(|r| {
            format!(
                "{} {}/{} {} B",
                r.source.label(),
                r.with_description,
                r.entries,
                thousands(r.bytes as u64)
            )
        })
        .collect();
    let observed = format!(
        "{} entries / {} with description / {} B of {} ({} B headroom; {})",
        catalog.entries.len(),
        catalog.with_description(),
        thousands(catalog.total_bytes as u64),
        thousands(hr::SKILLS_CATALOG_CAP_BYTES),
        thousands(catalog.headroom_bytes()),
        per_source.join(", ")
    );
    let refund = catalog
        .largest_of(Source::Bundled)
        .map(|e| {
            format!(
                "muse skills disable {} --scope built-in   # refunds {} B",
                e.id,
                thousands(e.bytes as u64)
            )
        })
        .unwrap_or_else(|| "muse skills disable bundled:<id> --scope built-in".to_string());
    let fix = format!(
        "{refund}\nomm settings set run.context_slimming.skill_catalog_descriptions first_sentence   # and re-list bundled:git in full_skill_description_ids\nomm cost"
    );
    if live.cap_named_on_stderr {
        return Check::critical(
            ID,
            TITLE,
            format!("{observed}; stage 3: the host dropped entries and named the cap on stderr"),
            WHY_SILENT,
            fix,
        );
    }
    if catalog.stage() == Stage::DescriptionsDropped {
        let dropped: Vec<&str> = catalog
            .dropped()
            .iter()
            .map(|e| e.id.as_str())
            .take(8)
            .collect();
        return Check::critical(
            ID,
            TITLE,
            format!(
                "{observed}; stage 2: {} description(s) silently dropped ({}{})",
                catalog.dropped().len(),
                dropped.join(", "),
                if catalog.dropped().len() > dropped.len() {
                    ", …"
                } else {
                    ""
                }
            ),
            WHY_SILENT,
            fix,
        );
    }
    if catalog.headroom_bytes() < HEADROOM_WARN_BYTES {
        return Check::warn(
            ID,
            TITLE,
            format!(
                "{observed}; under {} B of headroom — the next skill starts the silent drop",
                thousands(HEADROOM_WARN_BYTES)
            ),
            WHY_SILENT,
            fix,
        );
    }
    Check::info(ID, TITLE, observed, WHY_SILENT)
}
