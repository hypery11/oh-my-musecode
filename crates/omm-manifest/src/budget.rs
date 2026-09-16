//! The catalog byte estimate (R18) — what the bundle costs in the order-200
//! `skills_catalog` block, per entry, before a session ever runs.
//!
//! Entry cost, measured (`docs/host-reality.md` "Budgets" → entry cost;
//! `docs/experiments/context-slimming.md` §2; re-verified byte-for-byte on
//! the shipped bundle in `content/VALIDATION.md` §6):
//!
//! ```text
//! 38 + len(display_id) + len(display_path) + len(xml_escaped_description) + 36
//!   + 40 + len(short_description)                     when metadata.short-description is set
//! display_id   = plugin:<pid>:<id>
//! display_path = plugin://<pid>/<path>
//! ```
//!
//! (`hr::CATALOG_ENTRY_BASE_BYTES`, `hr::CATALOG_ENTRY_DESCRIPTION_OVERHEAD_BYTES`;
//! the `<short-description>` element — `research/musecode/skills.md` §5 — adds
//! its tag pair, 40 B.) The description is rendered after the front-matter
//! loader collapsed its whitespace and XML-escaped it (`&apos;` counts).
//!
//! Under `run.context_slimming.skill_catalog_descriptions: "first_sentence"`
//! (`hr::CONTEXT_SLIMMING_FIRST_SENTENCE`, context-slimming.md §3) every
//! description is cut after the first run of ASCII `.`/`?`/`!` that is
//! followed by a space — [`first_sentence`] — which is why every shipped
//! description joins its `Do not use …` clause with `;` (§5.3).

use std::path::Path;

use serde_json::Value;

use crate::catalog::AssetKind;
use crate::content::Content;
use crate::error::{ManifestError, Result};
use crate::frontmatter::collapse_whitespace;
use crate::hr;

/// Bytes of the `<short-description>…</short-description>` element pair
/// around a rendered short description (context-slimming.md §2 formula as
/// recorded in `content/catalog.json → budget.entry_formula`).
pub const SHORT_DESCRIPTION_OVERHEAD_BYTES: u64 = 40;

/// XML-escape as the catalog renderer does (`&` `<` `>` `"` `'`).
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// The first sentence as `first_sentence` cuts it: whitespace collapsed, then
/// everything after the first run of ASCII `.`/`?`/`!` that a space follows.
/// `.)`, `."`, `1.2`, `。` are not boundaries; with no boundary the whole
/// (collapsed) text is kept (context-slimming.md §3).
pub fn first_sentence(description: &str) -> String {
    let collapsed = collapse_whitespace(description);
    let bytes = collapsed.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if matches!(bytes[i], b'.' | b'?' | b'!') {
            let mut j = i;
            while j < bytes.len() && matches!(bytes[j], b'.' | b'?' | b'!') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b' ' {
                return collapsed[..j].to_string();
            }
            i = j;
        } else {
            i += 1;
        }
    }
    collapsed
}

/// The rendered description of one entry in both modes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryCost {
    /// Bytes with `skill_catalog_descriptions: "full"` (the default).
    pub full: u64,
    /// Bytes under `"first_sentence"`.
    pub first_sentence: u64,
}

/// The entry formula for a plugin-scope skill.
pub fn entry_cost(
    plugin_id: &str,
    skill_id: &str,
    path: &str,
    description: &str,
    short_description: Option<&str>,
) -> EntryCost {
    let display_id = format!("plugin:{plugin_id}:{skill_id}");
    let display_path = format!("plugin://{plugin_id}/{path}");
    let fixed = hr::CATALOG_ENTRY_BASE_BYTES
        + display_id.len() as u64
        + display_path.len() as u64
        + hr::CATALOG_ENTRY_DESCRIPTION_OVERHEAD_BYTES
        + short_description
            .map(|s| {
                SHORT_DESCRIPTION_OVERHEAD_BYTES + xml_escape(&collapse_whitespace(s)).len() as u64
            })
            .unwrap_or(0);
    let full = xml_escape(&collapse_whitespace(description)).len() as u64;
    let first = xml_escape(&first_sentence(description)).len() as u64;
    EntryCost {
        full: fixed + full,
        first_sentence: fixed + first,
    }
}

/// One skill's estimate beside the catalog's recorded value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntryEstimate {
    pub id: String,
    pub path: String,
    pub cost: EntryCost,
    /// `catalog.json → budget_bytes`.
    pub catalog_full: u64,
    /// `catalog.json → budget_bytes_first_sentence`.
    pub catalog_first_sentence: Option<u64>,
}

impl EntryEstimate {
    /// True when the catalog's numbers match the recomputed ones.
    pub fn catalog_is_current(&self) -> bool {
        self.catalog_full == self.cost.full
            && self
                .catalog_first_sentence
                .map(|v| v == self.cost.first_sentence)
                .unwrap_or(true)
    }
}

/// The bundle's estimate against the three budgets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetReport {
    pub entries: Vec<EntryEstimate>,
    pub total_full: u64,
    pub total_first_sentence: u64,
    /// `hr::BUNDLE_BUDGET_BYTES` — built-ins on, descriptions full.
    pub limit_full: u64,
    /// `hr::BUNDLE_BUDGET_FIRST_SENTENCE_BYTES` — built-ins on, first sentence.
    pub limit_first_sentence: u64,
    /// `hr::BUNDLE_BUDGET_BUILTINS_DISABLED_BYTES`.
    pub limit_builtins_disabled: u64,
}

impl BudgetReport {
    /// Within R18 with the built-ins on and descriptions full.
    pub fn within_full(&self) -> bool {
        self.total_full <= self.limit_full
    }
    /// Within R18 under `first_sentence`.
    pub fn within_first_sentence(&self) -> bool {
        self.total_first_sentence <= self.limit_first_sentence
    }
    /// Entries whose catalog numbers are stale.
    pub fn stale(&self) -> Vec<&EntryEstimate> {
        self.entries
            .iter()
            .filter(|e| !e.catalog_is_current())
            .collect()
    }
}

/// Estimate every shipped skill of `content`.
pub fn estimate(content: &Content) -> BudgetReport {
    let pid = content.plugin_id.as_str();
    let mut entries = Vec::new();
    for skill in &content.skills {
        let short = skill
            .frontmatter
            .get_nested("metadata", "short-description");
        let cost = entry_cost(
            pid,
            &skill.asset.id,
            &skill.asset.path,
            &skill.description,
            short,
        );
        entries.push(EntryEstimate {
            id: skill.asset.id.clone(),
            path: skill.asset.path.clone(),
            cost,
            catalog_full: skill.asset.budget_bytes,
            catalog_first_sentence: skill.asset.budget_bytes_first_sentence,
        });
    }
    let total_full = entries.iter().map(|e| e.cost.full).sum();
    let total_first_sentence = entries.iter().map(|e| e.cost.first_sentence).sum();
    BudgetReport {
        entries,
        total_full,
        total_first_sentence,
        limit_full: hr::BUNDLE_BUDGET_BYTES,
        limit_first_sentence: hr::BUNDLE_BUDGET_FIRST_SENTENCE_BYTES,
        limit_builtins_disabled: hr::BUNDLE_BUDGET_BUILTINS_DISABLED_BYTES,
    }
}

/// `content/catalog.json` with its budget numbers recomputed from `report`:
/// the header's `skills_total_bytes` / `skills_total_first_sentence_bytes`
/// and every skill row's `budget_bytes` / `budget_bytes_first_sentence` —
/// only keys the author already declared are updated, none is added. The
/// bytes come back in the catalog's own shape (2-space pretty JSON, key
/// order kept, one trailing newline), so an up-to-date catalog round-trips
/// byte for byte: `omm build` lands this file and `omm build --check`
/// treats a stale number as drift (Gate 1: the totals went stale with
/// nothing but a lint warning to say so).
pub fn refreshed_catalog(catalog_path: &Path, report: &BudgetReport) -> Result<Vec<u8>> {
    let bytes =
        std::fs::read(catalog_path).map_err(|e| ManifestError::io("read", catalog_path, e))?;
    let json_err = |e: serde_json::Error| ManifestError::Json {
        path: catalog_path.to_path_buf(),
        source: e,
    };
    let mut doc: Value = serde_json::from_slice(&bytes).map_err(json_err)?;
    if let Some(header) = doc.get_mut("budget").and_then(Value::as_object_mut) {
        for (key, recomputed) in [
            ("skills_total_bytes", report.total_full),
            (
                "skills_total_first_sentence_bytes",
                report.total_first_sentence,
            ),
        ] {
            if header.contains_key(key) {
                header.insert(key.to_string(), Value::from(recomputed));
            }
        }
    }
    if let Some(rows) = doc.get_mut("assets").and_then(Value::as_array_mut) {
        for row in rows.iter_mut() {
            let Some(estimate) = row
                .get("id")
                .and_then(Value::as_str)
                .and_then(|id| report.entries.iter().find(|e| e.id == id))
            else {
                continue;
            };
            let Some(obj) = row.as_object_mut() else {
                continue;
            };
            for (key, recomputed) in [
                ("budget_bytes", estimate.cost.full),
                ("budget_bytes_first_sentence", estimate.cost.first_sentence),
            ] {
                if obj.contains_key(key) {
                    obj.insert(key.to_string(), Value::from(recomputed));
                }
            }
        }
    }
    let mut out = serde_json::to_vec_pretty(&doc).map_err(json_err)?;
    out.push(b'\n');
    Ok(out)
}

/// Inventory units of a package: 1 per class + 2 per skill + 1 per command,
/// hook, MCP server and reminder (`hr::INVENTORY_BUDGET_UNITS`, quotas.md).
pub fn inventory_units(content: &Content) -> usize {
    let classes = hr::CAPABILITY_FAMILIES_MANIFEST.len();
    classes
        + 2 * content.skills.len()
        + content.commands.len()
        + content.hooks.len()
        + content.mcp_servers.len()
        + content.reminders.len()
}

/// The per-class maximum for one plugin (quotas.md; `hr::PER_CLASS_MAX_*`).
pub fn per_class_max(kind: AssetKind) -> Option<usize> {
    match kind {
        AssetKind::Skill => Some(hr::PER_CLASS_MAX_SKILLS),
        AssetKind::Command => Some(hr::PER_CLASS_MAX_COMMANDS),
        AssetKind::Hook => Some(hr::PER_CLASS_MAX_HOOKS),
        AssetKind::McpServer => Some(hr::PER_CLASS_MAX_MCP_SERVERS),
        AssetKind::Reminder => Some(hr::PER_CLASS_MAX_REMINDERS),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sentence_follows_the_measured_rule() {
        assert_eq!(first_sentence("Foo bar. Baz qux."), "Foo bar.");
        assert_eq!(first_sentence("Is it? Yes! No."), "Is it?");
        assert_eq!(first_sentence("Wait... then go. Done."), "Wait...");
        assert_eq!(first_sentence("v1.2 is out. Next"), "v1.2 is out.");
        assert_eq!(first_sentence("(see x.) more. end"), "(see x.) more.");
        assert_eq!(
            first_sentence("He said \"go.\" Then left. x"),
            "He said \"go.\" Then left."
        );
        assert_eq!(first_sentence("No boundary here"), "No boundary here");
        assert_eq!(first_sentence("Trailing dot."), "Trailing dot.");
        assert_eq!(first_sentence("a;\tb.\nc"), "a; b.");
        assert_eq!(first_sentence("日本。次の文. x"), "日本。次の文.");
    }

    #[test]
    fn entry_cost_matches_the_formula() {
        // 38 + len("plugin:omm:omm-x")=16 + len("plugin://omm/skills/omm-x/SKILL.md")=34 + 36 + len("A. B")=4 = 128
        let c = entry_cost("omm", "omm-x", "skills/omm-x/SKILL.md", "A. B", None);
        assert_eq!(c.full, 128);
        assert_eq!(c.first_sentence, 126);
        // XML escaping counts; a short description adds 40 + its length.
        let c = entry_cost("omm", "omm-x", "skills/omm-x/SKILL.md", "a & b", Some("s"));
        assert_eq!(c.full, 38 + 16 + 34 + 36 + 9 + 40 + 1);
        assert_eq!(xml_escape("<'\">&"), "&lt;&apos;&quot;&gt;&amp;");
    }

    #[test]
    fn per_class_and_inventory_constants_are_wired() {
        assert_eq!(per_class_max(AssetKind::Reminder), Some(43));
        assert_eq!(per_class_max(AssetKind::Theme), None);
    }
}
