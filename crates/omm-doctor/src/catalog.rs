//! The order-200 `skills_catalog` block, parsed entry by entry.
//!
//! Shape (measured live on 1.0.1-R2006.1, `docs/experiments/context-slimming.md`
//! §1): a 363-B header ending in `<skill-catalog>\n`
//! (`hr::SKILLS_CATALOG_HEADER_BYTES`), then one entry per skill —
//! `<skill id="…" scope="bundled|user|project|plugin" path="…">\n<description>…</description>[\n<short-description>…</short-description>]\n</skill>\n`
//! — then the 35-B footer `</skill-catalog>\n</system-reminder>`
//! (`hr::SKILLS_CATALOG_FOOTER_BYTES`). Render order is bundled →
//! project/user filesystem → plugin, so plugin entries are starved first
//! (`docs/host-reality.md` "Budgets": render order). Under stage-2
//! degradation an entry is rendered `<skill …/>` with no description and no
//! diagnostic anywhere (`context-slimming.md` §2 `big_full`; host-reality
//! "degradation") — the block itself is the only oracle, which is why doctor
//! parses it instead of asking `skills list --json`. Every byte count here is
//! UTF-8 bytes of the block text, never chars (context-slimming.md §0).

use serde::Serialize;

use omm_host::host_reality as hr;

/// Where an entry came from, in the host's render order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// `scope="bundled"` — Meta's built-ins (`bundled:<id>`), rendered first.
    Bundled,
    /// `scope="user"` or `scope="project"` — skills on the filesystem.
    Filesystem,
    /// `scope="plugin"` — installed plugin skills, rendered last and starved first.
    Plugin,
    /// Any other `scope` value (none observed).
    Other,
}

impl Source {
    /// Map the entry's `scope` attribute.
    pub fn from_scope(scope: &str) -> Source {
        match scope {
            "bundled" => Source::Bundled,
            "user" | "project" => Source::Filesystem,
            "plugin" => Source::Plugin,
            _ => Source::Other,
        }
    }
    /// Lower-case label.
    pub fn label(self) -> &'static str {
        match self {
            Source::Bundled => "bundled",
            Source::Filesystem => "filesystem",
            Source::Plugin => "plugin",
            Source::Other => "other",
        }
    }
}

/// One `<skill …>` entry of the block.
#[derive(Clone, Debug, Serialize)]
pub struct Entry {
    /// Position in the rendered block (0-based).
    pub order: usize,
    /// The `id` attribute, e.g. `bundled:git`, `cs-00`, `plugin:omm:omm-plan`.
    pub id: String,
    /// The raw `scope` attribute.
    pub scope: String,
    /// Where it came from.
    pub source: Source,
    /// The `path` attribute (a display locator, not always a file).
    pub path: String,
    /// The rendered `<description>` text (XML-escaped as rendered), `None`
    /// under stage-2 degradation.
    pub description: Option<String>,
    /// The rendered `<short-description>`, when the skill carries one.
    pub short_description: Option<String>,
    /// UTF-8 bytes of the whole entry including its trailing newline —
    /// exactly what disabling the skill refunds.
    pub bytes: usize,
}

/// Which degradation stage the block is in (host-reality "degradation").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Every entry carries its description.
    Intact,
    /// At least one entry was rendered without its description — silent.
    DescriptionsDropped,
}

/// Per-source totals, in render order.
#[derive(Clone, Debug, Serialize)]
pub struct SourceRow {
    pub source: Source,
    pub entries: usize,
    pub with_description: usize,
    pub bytes: usize,
}

/// The parsed block.
#[derive(Clone, Debug, Serialize)]
pub struct Catalog {
    /// Bytes of the whole block text.
    pub total_bytes: usize,
    /// Bytes before the first entry (535 on 1.3.0-R3057.1).
    pub header_bytes: usize,
    /// Bytes from `</skill-catalog>` to the end (35 on the observed build).
    pub footer_bytes: usize,
    pub entries: Vec<Entry>,
}

const OPEN_TAG: &str = "<skill ";
const CLOSE_TAG: &str = "</skill>";
const CATALOG_CLOSE: &str = "</skill-catalog>";
const DESCRIPTION_OPEN: &str = "<description>";
const DESCRIPTION_CLOSE: &str = "</description>";
const SHORT_OPEN: &str = "<short-description>";
const SHORT_CLOSE: &str = "</short-description>";

impl Catalog {
    /// Parse the block text.
    pub fn parse(text: &str) -> Catalog {
        let catalog_close = text.find(CATALOG_CLOSE);
        let footer_bytes = catalog_close.map(|p| text.len() - p).unwrap_or(0);
        let body_end = catalog_close.unwrap_or(text.len());
        let mut entries = Vec::new();
        let mut pos = 0usize;
        let mut header_bytes = body_end;
        while let Some(rel) = text[pos..body_end].find(OPEN_TAG) {
            let start = pos + rel;
            if entries.is_empty() {
                header_bytes = start;
            }
            let Some(tag_end_rel) = text[start..body_end].find('>') else {
                break;
            };
            let tag_end = start + tag_end_rel; // index of '>'
            let open_tag = &text[start..tag_end];
            let self_closing = open_tag.ends_with('/');
            let attrs_text = open_tag.trim_end_matches('/');
            let (body, end) = if self_closing {
                ("", tag_end + 1)
            } else {
                match text[tag_end + 1..body_end].find(CLOSE_TAG) {
                    Some(c) => (
                        &text[tag_end + 1..tag_end + 1 + c],
                        tag_end + 1 + c + CLOSE_TAG.len(),
                    ),
                    None => ("", body_end),
                }
            };
            // The entry owns its trailing newline.
            let end = if text[end..].starts_with('\n') {
                end + 1
            } else {
                end
            };
            let scope = attr(attrs_text, "scope").unwrap_or_default();
            entries.push(Entry {
                order: entries.len(),
                id: attr(attrs_text, "id").unwrap_or_default(),
                source: Source::from_scope(&scope),
                scope,
                path: attr(attrs_text, "path").unwrap_or_default(),
                description: between(body, DESCRIPTION_OPEN, DESCRIPTION_CLOSE),
                short_description: between(body, SHORT_OPEN, SHORT_CLOSE),
                bytes: end - start,
            });
            pos = end;
        }
        Catalog {
            total_bytes: text.len(),
            header_bytes,
            footer_bytes,
            entries,
        }
    }

    /// Bytes of all entries.
    pub fn entries_bytes(&self) -> usize {
        self.entries.iter().map(|e| e.bytes).sum()
    }
    /// Entries that carry a description.
    pub fn with_description(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.description.is_some())
            .count()
    }
    /// The degradation stage of the block.
    pub fn stage(&self) -> Stage {
        if self.with_description() == self.entries.len() {
            Stage::Intact
        } else {
            Stage::DescriptionsDropped
        }
    }
    /// Entries without a description, in render order.
    pub fn dropped(&self) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.description.is_none())
            .collect()
    }
    /// Per-source totals in render order (bundled, filesystem, plugin, other),
    /// omitting sources with no entries.
    pub fn by_source(&self) -> Vec<SourceRow> {
        [
            Source::Bundled,
            Source::Filesystem,
            Source::Plugin,
            Source::Other,
        ]
        .into_iter()
        .map(|source| {
            let rows: Vec<&Entry> = self.entries.iter().filter(|e| e.source == source).collect();
            SourceRow {
                source,
                entries: rows.len(),
                with_description: rows.iter().filter(|e| e.description.is_some()).count(),
                bytes: rows.iter().map(|e| e.bytes).sum(),
            }
        })
        .filter(|r| r.entries > 0)
        .collect()
    }
    /// Entries of one source.
    pub fn entries_of(&self, source: Source) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.source == source).collect()
    }
    /// Bytes left under the hard cap (`hr::SKILLS_CATALOG_CAP_BYTES`).
    pub fn headroom_bytes(&self) -> u64 {
        hr::SKILLS_CATALOG_CAP_BYTES.saturating_sub(self.total_bytes as u64)
    }
    /// The largest entry of a source, if any.
    pub fn largest_of(&self, source: Source) -> Option<&Entry> {
        self.entries_of(source).into_iter().max_by_key(|e| e.bytes)
    }
}

/// `key="value"` from an open tag's attribute text (values are XML-escaped
/// and contain no raw `"`).
fn attr(tag: &str, key: &str) -> Option<String> {
    let needle = format!(" {key}=\"");
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')?;
    Some(tag[start..start + end].to_string())
}

fn between(body: &str, open: &str, close: &str) -> Option<String> {
    let start = body.find(open)? + open.len();
    let end = body[start..].find(close)?;
    Some(body[start..start + end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "<system-reminder source=\"skills\">\nMuse Code loaded available skills at session open. These are summaries only.\n\n<skill-catalog>\n";
    const FOOTER: &str = "</skill-catalog>\n</system-reminder>";

    #[test]
    fn parses_entries_header_footer_and_bytes() {
        let bundled = "<skill id=\"bundled:git\" scope=\"bundled\" path=\"bundled://muse-core/skills/git/SKILL.md\">\n<description>Source-control safety for Git.</description>\n</skill>\n";
        let user = "<skill id=\"cs-00\" scope=\"user\" path=\"$CONFIG_DIR/skills/cs-00/SKILL.md\">\n<description>User probe. Second.</description>\n<short-description>short</short-description>\n</skill>\n";
        let plugin = "<skill id=\"plugin:omm:omm-plan\" scope=\"plugin\" path=\"plugin://omm/skills/omm-plan/SKILL.md\">\n<description>Plan &amp; go.</description>\n</skill>\n";
        let dropped =
            "<skill id=\"cs-99\" scope=\"user\" path=\"$CONFIG_DIR/skills/cs-99/SKILL.md\"/>\n";
        let text = format!("{HEADER}{bundled}{user}{plugin}{dropped}{FOOTER}");
        let c = Catalog::parse(&text);
        assert_eq!(c.total_bytes, text.len());
        assert_eq!(c.header_bytes, HEADER.len());
        assert_eq!(c.footer_bytes, FOOTER.len());
        assert_eq!(FOOTER.len() as u64, hr::SKILLS_CATALOG_FOOTER_BYTES);
        assert_eq!(c.entries.len(), 4);
        assert_eq!(c.entries[0].id, "bundled:git");
        assert_eq!(c.entries[0].source, Source::Bundled);
        assert_eq!(c.entries[0].bytes, bundled.len());
        assert_eq!(
            c.entries[0].description.as_deref(),
            Some("Source-control safety for Git.")
        );
        assert_eq!(c.entries[1].source, Source::Filesystem);
        assert_eq!(c.entries[1].short_description.as_deref(), Some("short"));
        assert_eq!(c.entries[2].source, Source::Plugin);
        assert_eq!(c.entries[2].description.as_deref(), Some("Plan &amp; go."));
        assert_eq!(c.entries[3].description, None);
        assert_eq!(c.entries[3].bytes, dropped.len());
        assert_eq!(c.stage(), Stage::DescriptionsDropped);
        assert_eq!(c.dropped().len(), 1);
        assert_eq!(c.with_description(), 3);
        assert_eq!(
            c.header_bytes + c.entries_bytes() + c.footer_bytes,
            c.total_bytes,
            "header + entries + footer account for every byte"
        );
        let rows = c.by_source();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].source, Source::Bundled);
        assert_eq!(rows[1].entries, 2);
        assert_eq!(rows[1].with_description, 1);
        assert_eq!(
            c.largest_of(Source::Bundled).map(|e| e.id.as_str()),
            Some("bundled:git")
        );
        // The plugin-scope entry cost formula of host-reality "Budgets":
        // 38 + len(id) + len(path) + len(desc) + 36.
        let e = &c.entries[2];
        assert_eq!(
            e.bytes as u64,
            hr::CATALOG_ENTRY_BASE_BYTES
                + e.id.len() as u64
                + e.path.len() as u64
                + e.description.as_ref().map(|d| d.len() as u64).unwrap_or(0)
                + hr::CATALOG_ENTRY_DESCRIPTION_OVERHEAD_BYTES
        );
        let u = &c.entries[0];
        assert_eq!(u.source, Source::Bundled);
    }

    #[test]
    fn empty_catalog_is_header_and_footer_only() {
        let text = format!("{HEADER}{FOOTER}");
        let c = Catalog::parse(&text);
        assert!(c.entries.is_empty());
        assert_eq!(c.header_bytes, HEADER.len());
        assert_eq!(c.footer_bytes, FOOTER.len());
        assert_eq!(c.stage(), Stage::Intact);
        assert!(c.by_source().is_empty());
        assert_eq!(
            c.headroom_bytes(),
            hr::SKILLS_CATALOG_CAP_BYTES - text.len() as u64
        );
        let none = Catalog::parse("");
        assert_eq!(none.total_bytes, 0);
        assert_eq!(none.header_bytes, 0);
    }
}
