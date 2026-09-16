//! Terminal output shared by every command (ARCHITECTURE.md §7 "Global"):
//! one switch between the human rendering and the `--json` document, a tiny
//! column table, and the per-category converge summary every write command
//! prints (updated / unchanged / skipped / backed-up / removed).
//!
//! Rules: under `--json` stdout carries exactly one JSON document and nothing
//! else — human lines are dropped and warnings go to stderr. The hook
//! dispatcher runs with [`Output::silent`]: its stdout *is* the decision and
//! its stderr must stay empty (R16, `content/hooks/README.md`), so it prints
//! its one line itself. Write failures (a closed pipe under `| head`) are
//! ignored rather than aborted on.

use std::collections::BTreeMap;
use std::io::Write as _;

use serde_json::{json, Value};

/// A result that can be shown both ways.
pub trait Render {
    /// The human rendering: one or more lines, no trailing newline.
    fn render(&self) -> String;
    /// The `--json` document.
    fn to_json(&self) -> Value;
}

/// Where and how a command talks to the terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Output {
    json: bool,
    verbose: bool,
    silent: bool,
}

impl Output {
    /// Human output, or one JSON document when `json` is set.
    pub fn new(json: bool, verbose: bool) -> Output {
        Output {
            json,
            verbose,
            silent: false,
        }
    }

    /// Nothing on either stream — the hook dispatcher owns its stdout.
    pub fn silent() -> Output {
        Output {
            json: false,
            verbose: false,
            silent: true,
        }
    }

    /// `--json` is in effect: stdout is reserved for the document.
    pub fn is_json(&self) -> bool {
        self.json
    }

    /// `--verbose` is in effect ([`Output::note`] prints).
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// Every method is a no-op.
    pub fn is_silent(&self) -> bool {
        self.silent
    }

    /// A human line on stdout. Dropped under `--json` and when silent.
    pub fn line(&self, text: impl AsRef<str>) {
        if !self.json && !self.silent {
            stdout_line(text.as_ref());
        }
    }

    /// A warning on stderr (`warning: …`), in both modes; never when silent.
    pub fn warn(&self, text: impl AsRef<str>) {
        if !self.silent {
            stderr_line(&format!("warning: {}", text.as_ref()));
        }
    }

    /// A stderr line shown only under `--verbose`.
    pub fn note(&self, text: impl AsRef<str>) {
        if self.verbose && !self.silent {
            stderr_line(text.as_ref());
        }
    }

    /// The result of a command: the JSON document under `--json` (one line),
    /// else the human rendering. Exactly one closure runs; an empty human
    /// rendering prints nothing.
    pub fn emit(&self, human: impl FnOnce() -> String, json: impl FnOnce() -> Value) {
        if self.silent {
            return;
        }
        if self.json {
            stdout_line(&json().to_string());
        } else {
            let text = human();
            if !text.is_empty() {
                stdout_line(&text);
            }
        }
    }

    /// [`Output::emit`] for anything that implements [`Render`].
    pub fn report(&self, r: &dyn Render) {
        self.emit(|| r.render(), || r.to_json());
    }
}

fn stdout_line(text: &str) {
    let stdout = std::io::stdout();
    let mut lock = stdout.lock();
    let _ = writeln!(lock, "{text}");
    let _ = lock.flush();
}

fn stderr_line(text: &str) {
    let stderr = std::io::stderr();
    let mut lock = stderr.lock();
    let _ = writeln!(lock, "{text}");
}

// ---- table --------------------------------------------------------------

/// Column alignment of a [`Table`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Left,
    Right,
}

/// A small column table: a header row, string cells, two-space gutters,
/// widths measured in characters. `to_json` is an array of objects keyed by
/// the header ([`Table::key`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Table {
    headers: Vec<String>,
    aligns: Vec<Align>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// A table with these headers and no rows.
    pub fn new<I, S>(headers: I) -> Table
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let headers: Vec<String> = headers.into_iter().map(Into::into).collect();
        let aligns = vec![Align::Left; headers.len()];
        Table {
            headers,
            aligns,
            rows: Vec::new(),
        }
    }

    /// Right-align column `col` (numbers). An index past the last column is ignored.
    pub fn right(mut self, col: usize) -> Table {
        if let Some(a) = self.aligns.get_mut(col) {
            *a = Align::Right;
        }
        self
    }

    /// Append a row. Missing cells are empty; cells past the header count are dropped.
    pub fn row<I, S>(&mut self, cells: I) -> &mut Table
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut row: Vec<String> = cells.into_iter().map(Into::into).collect();
        row.resize(self.headers.len(), String::new());
        self.rows.push(row);
        self
    }

    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    pub fn rows(&self) -> &[Vec<String>] {
        &self.rows
    }

    /// Number of rows (the header is not a row).
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The JSON key of a header: lowercase, every run of non-alphanumerics
    /// folded to one `_` (`"backed-up"` → `backed_up`, `"Bytes (B)"` → `bytes_b`).
    pub fn key(header: &str) -> String {
        let mut out = String::with_capacity(header.len());
        let mut pending = false;
        for c in header.chars() {
            if c.is_alphanumeric() {
                if pending && !out.is_empty() {
                    out.push('_');
                }
                pending = false;
                out.extend(c.to_lowercase());
            } else {
                pending = true;
            }
        }
        out
    }
}

impl Render for Table {
    fn render(&self) -> String {
        let n = self.headers.len();
        let mut widths: Vec<usize> = self.headers.iter().map(|h| h.chars().count()).collect();
        for row in &self.rows {
            for (i, cell) in row.iter().enumerate().take(n) {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
        let render_row = |cells: &[String]| -> String {
            let mut line = String::new();
            for (i, cell) in cells.iter().enumerate().take(n) {
                if i > 0 {
                    line.push_str("  ");
                }
                let pad = widths[i].saturating_sub(cell.chars().count());
                let last = i + 1 == n;
                match self.aligns[i] {
                    Align::Right => {
                        line.extend(std::iter::repeat_n(' ', pad));
                        line.push_str(cell);
                    }
                    Align::Left => {
                        line.push_str(cell);
                        if !last {
                            line.extend(std::iter::repeat_n(' ', pad));
                        }
                    }
                }
            }
            line
        };
        let mut lines = Vec::with_capacity(self.rows.len() + 1);
        lines.push(render_row(&self.headers));
        for row in &self.rows {
            lines.push(render_row(row));
        }
        lines
            .iter()
            .map(|l| l.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn to_json(&self) -> Value {
        Value::Array(
            self.rows
                .iter()
                .map(|row| {
                    let mut obj = serde_json::Map::new();
                    for (h, c) in self.headers.iter().zip(row) {
                        obj.insert(Table::key(h), Value::String(c.clone()));
                    }
                    Value::Object(obj)
                })
                .collect(),
        )
    }
}

// ---- converge summary ---------------------------------------------------

/// What happened to one item of a write command (ARCHITECTURE.md §7:
/// "every write prints a per-category converge summary").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Written or rewritten.
    Updated,
    /// Already identical; not touched.
    Unchanged,
    /// Left alone on purpose (user edit, disabled id, staged conflict, dry run).
    Skipped,
    /// A verified backup / snapshot was taken (counts beside the write it protected).
    BackedUp,
    /// Removed.
    Removed,
}

impl Action {
    /// Render order of the summary columns.
    pub const ALL: [Action; 5] = [
        Action::Updated,
        Action::Unchanged,
        Action::Skipped,
        Action::BackedUp,
        Action::Removed,
    ];

    /// The column label.
    pub fn label(self) -> &'static str {
        match self {
            Action::Updated => "updated",
            Action::Unchanged => "unchanged",
            Action::Skipped => "skipped",
            Action::BackedUp => "backed-up",
            Action::Removed => "removed",
        }
    }

    /// The JSON key.
    pub fn key(self) -> &'static str {
        match self {
            Action::BackedUp => "backed_up",
            other => other.label(),
        }
    }
}

/// Per-action counts of one category (or the total).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub updated: usize,
    pub unchanged: usize,
    pub skipped: usize,
    pub backed_up: usize,
    pub removed: usize,
}

impl Counts {
    pub fn get(&self, action: Action) -> usize {
        match action {
            Action::Updated => self.updated,
            Action::Unchanged => self.unchanged,
            Action::Skipped => self.skipped,
            Action::BackedUp => self.backed_up,
            Action::Removed => self.removed,
        }
    }

    pub fn bump(&mut self, action: Action, n: usize) {
        let slot = match action {
            Action::Updated => &mut self.updated,
            Action::Unchanged => &mut self.unchanged,
            Action::Skipped => &mut self.skipped,
            Action::BackedUp => &mut self.backed_up,
            Action::Removed => &mut self.removed,
        };
        *slot += n;
    }

    pub fn add(&mut self, other: &Counts) {
        for a in Action::ALL {
            self.bump(a, other.get(a));
        }
    }

    /// Items that touched the disk: updated + backed-up + removed.
    pub fn changed(&self) -> usize {
        self.updated + self.backed_up + self.removed
    }

    /// Every counted item.
    pub fn total(&self) -> usize {
        Action::ALL.iter().map(|a| self.get(*a)).sum()
    }

    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        for a in Action::ALL {
            obj.insert(a.key().to_string(), json!(self.get(a)));
        }
        Value::Object(obj)
    }
}

/// The converge summary of one write command: counts per category
/// (`skills`, `settings`, `trust`, `plugin`, …, sorted) plus a total.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Converge {
    dry_run: bool,
    categories: BTreeMap<String, Counts>,
}

impl Converge {
    /// An empty summary; `dry_run` labels the rendering "nothing was written".
    pub fn new(dry_run: bool) -> Converge {
        Converge {
            dry_run,
            categories: BTreeMap::new(),
        }
    }

    /// Count one item.
    pub fn record(&mut self, category: impl Into<String>, action: Action) -> &mut Converge {
        self.record_n(category, action, 1)
    }

    /// Count `n` items at once.
    pub fn record_n(
        &mut self,
        category: impl Into<String>,
        action: Action,
        n: usize,
    ) -> &mut Converge {
        self.categories
            .entry(category.into())
            .or_default()
            .bump(action, n);
        self
    }

    pub fn category(&self, name: &str) -> Option<&Counts> {
        self.categories.get(name)
    }

    /// Categories in render (sorted) order.
    pub fn categories(&self) -> impl Iterator<Item = (&str, &Counts)> {
        self.categories.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn total(&self) -> Counts {
        let mut total = Counts::default();
        for c in self.categories.values() {
            total.add(c);
        }
        total
    }

    /// Nothing touched the disk (every item unchanged or skipped).
    pub fn is_noop(&self) -> bool {
        self.total().changed() == 0
    }

    pub fn is_dry_run(&self) -> bool {
        self.dry_run
    }

    /// Fold another summary in (same categories add up).
    pub fn merge(&mut self, other: &Converge) {
        for (name, counts) in &other.categories {
            self.categories.entry(name.clone()).or_default().add(counts);
        }
    }
}

impl Render for Converge {
    fn render(&self) -> String {
        let suffix = if self.dry_run {
            "\ndry run: nothing was written"
        } else {
            ""
        };
        if self.categories.is_empty() {
            return format!("nothing to do{suffix}");
        }
        let mut table =
            Table::new(std::iter::once("category").chain(Action::ALL.iter().map(|a| a.label())));
        for col in 1..=Action::ALL.len() {
            table = table.right(col);
        }
        let row_of = |name: &str, c: &Counts| {
            std::iter::once(name.to_string())
                .chain(Action::ALL.iter().map(|a| c.get(*a).to_string()))
                .collect::<Vec<_>>()
        };
        for (name, counts) in &self.categories {
            table.row(row_of(name, counts));
        }
        if self.categories.len() > 1 {
            table.row(row_of("total", &self.total()));
        }
        format!("{}{suffix}", table.render())
    }

    fn to_json(&self) -> Value {
        let categories: serde_json::Map<String, Value> = self
            .categories
            .iter()
            .map(|(k, v)| (k.clone(), v.to_json()))
            .collect();
        json!({
            "dry_run": self.dry_run,
            "noop": self.is_noop(),
            "categories": Value::Object(categories),
            "total": self.total().to_json(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pads_columns_by_char_width_and_right_aligns_numbers() {
        let mut t = Table::new(["source", "entries", "bytes"]).right(1).right(2);
        t.row(["bundled", "15", "10060"]);
        t.row(["plugin — omm", "12", "6"]);
        t.row(["x"]);
        let expected = [
            "source        entries  bytes",
            "bundled            15  10060",
            "plugin — omm       12      6",
            "x",
        ]
        .join("\n");
        assert_eq!(t.render(), expected);
        let json = t.to_json();
        assert_eq!(json[0]["source"], "bundled");
        assert_eq!(json[0]["bytes"], "10060");
        assert_eq!(json[2]["entries"], "");
        assert_eq!(t.len(), 3);
        assert!(!t.is_empty());
        assert_eq!(t.headers().len(), 3);
        assert_eq!(t.rows()[2].len(), 3);
    }

    #[test]
    fn header_keys_are_snake_case() {
        assert_eq!(Table::key("backed-up"), "backed_up");
        assert_eq!(Table::key("Bytes (B)"), "bytes_b");
        assert_eq!(Table::key("  why silent "), "why_silent");
        assert_eq!(Table::key("Δ bytes"), "δ_bytes");
    }

    #[test]
    fn converge_counts_render_and_serialise() {
        let mut c = Converge::new(false);
        c.record("skills", Action::Updated)
            .record("skills", Action::Unchanged)
            .record_n("settings", Action::BackedUp, 2)
            .record("settings", Action::Updated);
        assert_eq!(c.total().updated, 2);
        assert_eq!(c.total().backed_up, 2);
        assert_eq!(c.total().total(), 5);
        assert!(!c.is_noop());
        let text = c.render();
        assert!(text.starts_with("category  updated  unchanged  skipped  backed-up  removed"));
        assert!(text.contains("settings        1          0        0          2        0"));
        assert!(text.contains("total           2          1        0          2        0"));
        assert!(!text.contains("dry run"));
        let json = c.to_json();
        assert_eq!(json["categories"]["skills"]["updated"], 1);
        assert_eq!(json["total"]["backed_up"], 2);
        assert_eq!(json["noop"], false);
        assert_eq!(json["dry_run"], false);
    }

    #[test]
    fn converge_noop_and_dry_run_labels() {
        let mut c = Converge::new(true);
        assert_eq!(c.render(), "nothing to do\ndry run: nothing was written");
        assert!(c.is_noop());
        c.record("themes", Action::Unchanged)
            .record("themes", Action::Skipped);
        assert!(c.is_noop());
        assert!(c.render().ends_with("dry run: nothing was written"));
        // A single category prints no total row.
        assert_eq!(c.render().lines().count(), 3);
        let mut other = Converge::new(true);
        other.record("themes", Action::Removed);
        c.merge(&other);
        assert!(!c.is_noop());
        assert_eq!(c.category("themes").map(|x| x.removed), Some(1));
    }

    #[test]
    fn action_labels_and_keys() {
        for a in Action::ALL {
            assert!(!a.label().is_empty());
            assert!(!a.key().contains('-'));
        }
        assert_eq!(Action::BackedUp.label(), "backed-up");
        assert_eq!(Action::BackedUp.key(), "backed_up");
    }

    #[test]
    fn silent_output_is_inert() {
        let o = Output::silent();
        assert!(o.is_silent() && !o.is_json() && !o.is_verbose());
        let o = Output::new(true, true);
        assert!(o.is_json() && o.is_verbose() && !o.is_silent());
    }
}
