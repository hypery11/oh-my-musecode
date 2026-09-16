//! Skill routing — the spawn-free core (PLAN.md 3.1; ARCHITECTURE.md §7
//! `enable`/`disable`, R16, R18; `research/experiments/skill-routing.md`;
//! host-reality.md "Budgets", "Trust lifecycle", "Paths": hooks tiers).
//!
//! Facts this module is built on, every one measured on 1.0.1-R2006.1:
//!
//! * The router is a `UserPromptSubmit` handler in the project tier
//!   (`<ws>/.muse/hooks.json`, trust-gated: in an untrusted workspace it never
//!   loads, silently) declaring `"outputCapabilities":["skills.v1"]`. The host
//!   then writes `supported_output_capabilities:["skills.v1"]` on the hook's
//!   stdin — that field IS the negotiation — and, under both gates
//!   (`hr::ENV_ROUTING_GATE` + `hr::ENV_ROUTING_APPLY_GATE`), renders the
//!   returned `hookSpecificOutput.selectedSkills` as the order-201 block
//!   (`hr::CONTEXT_ORDER_SELECTED_SKILLS`). Returning `selectedSkills` without
//!   the field on stdin is `selected_skills:rejected:capability-not-negotiated`,
//!   so the router answers `{}` unless it sees the field (skill-routing.md §1).
//! * A routed skill must be a REAL regular file inside the workspace root,
//!   not under a discovery root (`.agents/skills/` → `base-id-collision`),
//!   never a symlink or behind one (`read_skill` refuses: `skill-body-path-
//!   unsafe`, V2); `read_skill` then resolves it by id or absolute path (V3).
//!   Hence `<ws>/.omm/skills/<id>/SKILL.md`, copied, never linked.
//! * One bad entry discards the whole selection: absolute path under the
//!   root, no `..`, basename `SKILL.md`, id `^[a-z0-9][a-z0-9._-]{0,79}$`,
//!   description non-empty ≤ 1,024 B, unique paths, ≤ 32 per turn, hook
//!   stdout ≤ 16,384 B (§5). The router validates every entry before it
//!   prints and drops what would not pass.
//! * Order 200 and order 201 share ONE budget: `A + B_full ≤ 31,984` renders
//!   the block whole; `A + B_slim ≤ 31,984` drops EVERY description silently
//!   (`status: completed`, no diagnostic); above that the selection is
//!   rejected outright (`combined-budget`), V1. The rendered block costs
//!   [`BLOCK_FRAME_BYTES`] + Σ [`entry_bytes`] (measured 2026-09-03 by
//!   bisection: 868 B for one 2-char id / 10-byte description, +588 for a
//!   second identical entry, +5 for one more byte of label in a path that
//!   appears twice and one more of description — 2,058 B predicted and
//!   measured for three entries). The router budgets every turn against
//!   the last measured order-200 size and trims or drops to stay under the
//!   cap by [`BUDGET_MARGIN_BYTES`].
//! * Order 201 is `cache_class=runtime_prefix`: it changes every turn and
//!   invalidates the prompt-cache prefix from 201 on (§6) — which, with the
//!   two experimental gates, is why the whole feature is opt-in.

use std::path::{Component, Path, PathBuf};

use serde_json::{json, Map, Value};

use omm_host::host_reality as hr;
use omm_host::Roots;
use omm_manifest::frontmatter::{self, FrontMatter};

use super::hook::{estimate_catalog, HANDLER_ROUTE};
use super::{check_name, OmmConfig};
use crate::error::{OmmError, Result};

/// The feature name of `omm enable` / `omm disable`.
pub const FEATURE: &str = "skill-routing";
/// The routing event (hook-events.json: the ONLY event that accepts
/// `selectedSkills` end to end).
pub const EVENT_USER_PROMPT_SUBMIT: &str = "UserPromptSubmit";
/// The capability the handler declares and the host advertises back.
pub const SKILLS_V1: &str = "skills.v1";
/// The library inside `content/` (`content/routing/README.md`).
pub const LIBRARY_REL: &str = "routing/library";
/// The workspace library: not a discovery root, so nothing here reaches the
/// order-200 catalog and no routed id can `base-id-collision`.
pub const WS_SKILLS_DIR: &str = ".omm/skills";
/// The per-workspace router state (`order200_bytes`, the handler, the ids):
/// read by the hook from `cwd`, whatever its scrubbed environment resolves
/// `$OMM` to (host-reality "Paths": `XDG_*` does not reach a hook).
pub const WS_STATE_FILE: &str = ".omm/routing.json";
/// The project-tier hooks file (host-reality "Paths": hooks tiers).
pub const WS_HOOKS_FILE: &str = ".muse/hooks.json";
/// The skill entry file.
pub const SKILL_FILE: &str = "SKILL.md";
/// `metadata.triggers` — comma-separated phrases (README: a top-level
/// `triggers` key validates but is reported under `unknown_fields`).
pub const TRIGGERS_KEY: &str = "triggers";
/// Routed skills per turn, at most.
pub const MAX_SELECTED: usize = 3;
/// The handler's `timeout` (SECONDS in the hooks.json tiers; an omitted one
/// blocks forever, `0` is clamped to 1 — hooks.md §3.3, R2).
pub const HANDLER_TIMEOUT_SECS: u64 = 5;
/// The handler's `statusMessage`.
pub const HANDLER_STATUS: &str = "omm: routing skills";
/// Bytes of the order-201 block around its entries (measured 2026-09-03).
pub const BLOCK_FRAME_BYTES: u64 = 280;
/// Bytes of one entry beyond its id, path, hooks-file path and description
/// (measured 2026-09-03; the path appears once, the hooks file once inside
/// `hook-handler`, the digest and order attributes are constant-width).
pub const ENTRY_BASE_BYTES: u64 = 301;
/// Kept free below the combined cap: `hook-configured-order` grows a digit
/// per ten handlers ahead of ours, and the measured order-200 size is a
/// snapshot, not a promise.
pub const BUDGET_MARGIN_BYTES: u64 = 256;
/// A description is trimmed at a word boundary down to this many bytes to
/// fit the budget, never below (an entry that still does not fit is dropped).
pub const DESCRIPTION_MIN_BYTES: usize = 48;
/// At most this many `.omm/skills/*` directories are read per turn (sub-5 ms).
pub const LIBRARY_SCAN_MAX: usize = 64;
/// The `routing` record of `$OMM/config.json` (`omm enable skill-routing`).
pub const CONFIG_KEY_ROUTING: &str = "routing";
/// The state file's schema.
pub const STATE_SCHEMA_VERSION: u64 = 1;

// ---- the library -----------------------------------------------------------

/// One routable skill: its id, where its `SKILL.md` is, what the model sees
/// at order 201 and what routes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibrarySkill {
    pub id: String,
    /// The skill directory (`…/<id>/`).
    pub dir: PathBuf,
    /// `<dir>/SKILL.md`.
    pub path: PathBuf,
    /// The frontmatter description, whitespace-collapsed (what the host's
    /// loader would render; here it is the hook's own string).
    pub description: String,
    /// Normalised trigger phrases ([`normalise`]), in file order.
    pub triggers: Vec<String>,
}

/// Parse one `SKILL.md`: `name`, `description`, `metadata.triggers`.
pub fn parse_skill(id: &str, dir: &Path, bytes: &[u8]) -> Result<LibrarySkill> {
    let path = dir.join(SKILL_FILE);
    let fm: FrontMatter = frontmatter::parse(bytes)
        .map_err(|e| OmmError::Usage(format!("{}: {e}", path.display())))?;
    if fm.bom {
        return Err(OmmError::Usage(format!(
            "{}: starts with a BOM (bom_forbidden)",
            path.display()
        )));
    }
    if let Some(k) = fm.duplicate_keys.first() {
        return Err(OmmError::Usage(format!(
            "{}: duplicate frontmatter key `{k}`",
            path.display()
        )));
    }
    let name = fm.get("name").map(str::trim).unwrap_or("");
    if name != id {
        return Err(OmmError::Usage(format!(
            "{}: frontmatter name {name:?} is not the directory name {id:?}",
            path.display()
        )));
    }
    let description = frontmatter::collapse_whitespace(fm.get("description").unwrap_or(""));
    check_description(&description)
        .map_err(|why| OmmError::Usage(format!("{}: {why}", path.display())))?;
    let triggers = parse_triggers(fm.get_nested("metadata", TRIGGERS_KEY).unwrap_or(""));
    if triggers.is_empty() {
        return Err(OmmError::Usage(format!(
            "{}: no `metadata.{TRIGGERS_KEY}` (comma-separated phrases) — nothing would ever route it",
            path.display()
        )));
    }
    Ok(LibrarySkill {
        id: id.to_string(),
        dir: dir.to_path_buf(),
        path,
        description,
        triggers,
    })
}

/// The host's rule on a routed description: non-empty, ≤ 1,024 bytes
/// (`selectedSkills.description exceeds 1024 bytes`).
pub fn check_description(description: &str) -> std::result::Result<(), String> {
    if description.is_empty() {
        return Err("description is empty (the host rejects an empty routed description)".into());
    }
    let max = hr::ROUTED_SKILL_DESCRIPTION_MAX_BYTES as usize;
    if description.len() > max {
        return Err(format!(
            "description is {} bytes; the host caps a routed description at {max}",
            description.len()
        ));
    }
    Ok(())
}

/// `a, "b c", d` → normalised phrases, empties dropped, duplicates dropped.
pub fn parse_triggers(raw: &str) -> Vec<String> {
    let raw = raw.trim();
    let raw = raw
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .unwrap_or(raw);
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let p = frontmatter::unquote(part.trim());
        let n = normalise(&p);
        if !n.is_empty() && !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// The `omm-` style id rule the library enforces on top of the grammar:
/// the content-skill prefix (`omm_manifest::lint::ID_PREFIX`, R19 — not the
/// plugin id, which may differ), so a routed id can never shadow a user's own.
pub fn check_library_id(skill_prefix: &str, id: &str) -> Result<()> {
    check_name("routed skill", id)?;
    if !id.starts_with(skill_prefix) || id.len() == skill_prefix.len() {
        return Err(OmmError::Usage(format!(
            "routed skill id {id:?} must carry the `{skill_prefix}` prefix (R19)"
        )));
    }
    Ok(())
}

/// Load a library directory strictly (the content tree at `omm enable`):
/// every `<dir>/<id>/SKILL.md` must parse, be a regular file behind regular
/// directories (R12: omm copies what it can hash), carry the content-skill
/// prefix, and no id may collide with `taken` (catalog and bundled skill ids —
/// those are at order 200 already; a routed twin would be pointless and,
/// for a discovered one, `base-id-collision`).
pub fn load_library(dir: &Path, skill_prefix: &str, taken: &[String]) -> Result<Vec<LibrarySkill>> {
    let meta = std::fs::symlink_metadata(dir)
        .map_err(|e| OmmError::io(format!("stat {}", dir.display()), e))?;
    if !meta.is_dir() {
        return Err(OmmError::Usage(format!(
            "{} is not a directory (the routed library)",
            dir.display()
        )));
    }
    let mut names: Vec<String> = Vec::new();
    for entry in
        std::fs::read_dir(dir).map_err(|e| OmmError::io(format!("read {}", dir.display()), e))?
    {
        let entry = entry.map_err(|e| OmmError::io(format!("read {}", dir.display()), e))?;
        let ft = entry
            .file_type()
            .map_err(|e| OmmError::io(format!("stat {}", entry.path().display()), e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if ft.is_symlink() {
            return Err(OmmError::Usage(format!(
                "{} is a symlink; the library holds regular files only (R12)",
                entry.path().display()
            )));
        }
        if ft.is_dir() {
            names.push(name);
        } else if name != "README.md" {
            return Err(OmmError::Usage(format!(
                "{}: a stray file in the library (one directory per skill)",
                entry.path().display()
            )));
        }
    }
    names.sort();
    let mut out = Vec::with_capacity(names.len());
    for id in names {
        check_library_id(skill_prefix, &id)?;
        if taken.iter().any(|t| t == &id) {
            return Err(OmmError::Usage(format!(
                "routed skill id {id:?} collides with a catalog or bundled skill (it is at order 200 already)"
            )));
        }
        let skill_dir = dir.join(&id);
        let bytes = super::read_regular_file(&skill_dir.join(SKILL_FILE))?;
        out.push(parse_skill(&id, &skill_dir, &bytes)?);
    }
    Ok(out)
}

/// Load a workspace library leniently (the hook, every turn): a directory
/// that does not parse, is a symlink, or lacks a regular `SKILL.md` is
/// skipped — the router fails open, never loud. At most
/// [`LIBRARY_SCAN_MAX`] directories are read.
pub fn load_workspace_library(dir: &Path) -> Vec<LibrarySkill> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    names.truncate(LIBRARY_SCAN_MAX);
    let mut out = Vec::new();
    for id in names {
        let skill_dir = dir.join(&id);
        let file = skill_dir.join(SKILL_FILE);
        let Ok(meta) = std::fs::symlink_metadata(&file) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        if let Ok(s) = parse_skill(&id, &skill_dir, &bytes) {
            out.push(s);
        }
    }
    out
}

// ---- matching --------------------------------------------------------------

/// Lowercase, every non-alphanumeric run → one space, trimmed: the form
/// both the prompt and the triggers are compared in.
pub fn normalise(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut space = true;
    for c in text.chars() {
        if c.is_alphanumeric() {
            for l in c.to_lowercase() {
                out.push(l);
            }
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

/// A trigger's score against a normalised prompt: `1 + its word count`
/// when it occurs as whole words (a phrase as a word-bounded substring, a
/// single word as a word), else 0. A three-word phrase therefore outranks
/// three loose words.
pub fn trigger_score(prompt_norm: &str, trigger: &str) -> u32 {
    if trigger.is_empty() || prompt_norm.is_empty() {
        return 0;
    }
    let padded = format!(" {prompt_norm} ");
    let needle = format!(" {trigger} ");
    if padded.contains(&needle) {
        1 + trigger.split(' ').count() as u32
    } else {
        0
    }
}

/// The sum over every trigger.
pub fn score(prompt_norm: &str, skill: &LibrarySkill) -> u32 {
    skill
        .triggers
        .iter()
        .map(|t| trigger_score(prompt_norm, t))
        .sum()
}

/// One entry of the selection, as it will be printed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selected {
    pub id: String,
    pub path: PathBuf,
    pub description: String,
    pub score: u32,
}

/// The top [`MAX_SELECTED`] skills with a positive score, by score
/// descending then id ascending (deterministic across turns).
pub fn select(prompt: &str, library: &[LibrarySkill]) -> Vec<Selected> {
    let norm = normalise(prompt);
    let mut scored: Vec<Selected> = library
        .iter()
        .map(|s| Selected {
            id: s.id.clone(),
            path: s.path.clone(),
            description: s.description.clone(),
            score: score(&norm, s),
        })
        .filter(|s| s.score > 0)
        .collect();
    scored.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.id.cmp(&b.id)));
    scored.truncate(MAX_SELECTED);
    scored
}

// ---- the budget ------------------------------------------------------------

/// Bytes of `s` once the host XML-escapes it inside `<description>`.
pub fn xml_escaped_len(s: &str) -> u64 {
    s.bytes()
        .map(|b| match b {
            b'&' => 5,
            b'<' | b'>' => 4,
            b'"' | b'\'' => 6,
            _ => 1,
        })
        .sum()
}

/// Bytes one entry adds to the order-201 block.
pub fn entry_bytes(id: &str, path: &Path, hooks_file: &Path, description: &str) -> u64 {
    ENTRY_BASE_BYTES
        + id.len() as u64
        + path.as_os_str().len() as u64
        + hooks_file.as_os_str().len() as u64
        + xml_escaped_len(description)
}

/// Bytes of the whole block for a selection (0 for an empty one: no block).
pub fn block_bytes(selected: &[Selected], hooks_file: &Path) -> u64 {
    if selected.is_empty() {
        return 0;
    }
    BLOCK_FRAME_BYTES
        + selected
            .iter()
            .map(|s| entry_bytes(&s.id, &s.path, hooks_file, &s.description))
            .sum::<u64>()
}

/// The largest block any turn can produce from a library: the
/// [`MAX_SELECTED`] most expensive entries (what `omm enable` records as
/// `order201_max_bytes` and doctor D16 checks headroom against).
pub fn worst_case_bytes(library: &[LibrarySkill], hooks_file: &Path) -> u64 {
    let mut costs: Vec<u64> = library
        .iter()
        .map(|s| entry_bytes(&s.id, &s.path, hooks_file, &s.description))
        .collect();
    costs.sort_unstable_by(|a, b| b.cmp(a));
    if costs.is_empty() {
        return 0;
    }
    BLOCK_FRAME_BYTES + costs.iter().take(MAX_SELECTED).sum::<u64>()
}

/// Room for the block: `31,984 − order200 − margin`, floored at 0.
pub fn available_bytes(order200: u64) -> u64 {
    hr::ROUTING_BUDGET_BYTES
        .saturating_sub(order200)
        .saturating_sub(BUDGET_MARGIN_BYTES)
}

/// `s` cut at the last word boundary within `max` bytes (never inside a
/// UTF-8 sequence); `None` when even [`DESCRIPTION_MIN_BYTES`] do not fit.
pub fn trim_description(s: &str, max: usize) -> Option<String> {
    if s.len() <= max {
        return Some(s.to_string());
    }
    if max < DESCRIPTION_MIN_BYTES {
        return None;
    }
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &s[..cut];
    let at_word = head.rfind(' ').filter(|i| *i >= DESCRIPTION_MIN_BYTES);
    let out = match at_word {
        Some(i) => head[..i].trim_end_matches([',', ';', ':', ' ']),
        None => head.trim_end(),
    };
    if out.len() < DESCRIPTION_MIN_BYTES {
        return None;
    }
    Some(out.to_string())
}

/// Make the selection fit `available` bytes: first every description is
/// trimmed (the last entry first, so the best match keeps its whole text),
/// then entries are dropped from the end. What is left always fits.
pub fn fit(selected: &mut Vec<Selected>, available: u64, hooks_file: &Path) {
    let fits = |sel: &[Selected]| block_bytes(sel, hooks_file) <= available;
    while !selected.is_empty() && !fits(selected) {
        let over = block_bytes(selected, hooks_file) - available;
        let mut trimmed = false;
        for i in (0..selected.len()).rev() {
            let cur = xml_escaped_len(&selected[i].description);
            let target = cur.saturating_sub(over) as usize;
            if let Some(t) = trim_description(&selected[i].description, target) {
                if t.len() < selected[i].description.len() {
                    selected[i].description = t;
                    trimmed = true;
                    break;
                }
            }
        }
        if !trimmed {
            selected.pop();
        }
    }
}

// ---- validation before printing -------------------------------------------

/// Every rule the host applies to a `selectedSkills` entry (skill-routing.md
/// §5): the reason the entry would be rejected, or `None`.
pub fn entry_problem(root: &Path, s: &Selected) -> Option<String> {
    if check_name("routed skill", &s.id).is_err() {
        return Some(format!("id {:?} is not a valid id", s.id));
    }
    if !s.path.is_absolute() {
        return Some(format!("{} is not absolute", s.path.display()));
    }
    if s.path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Some(format!(
            "{} carries a `.` or `..` component",
            s.path.display()
        ));
    }
    if !s.path.starts_with(root) {
        return Some(format!(
            "{} is outside the workspace {}",
            s.path.display(),
            root.display()
        ));
    }
    if s.path.file_name().and_then(|n| n.to_str()) != Some(SKILL_FILE) {
        return Some(format!("{} is not a {SKILL_FILE}", s.path.display()));
    }
    match std::fs::symlink_metadata(&s.path) {
        Ok(m) if m.is_file() => {}
        Ok(_) => return Some(format!("{} is not a regular file", s.path.display())),
        Err(_) => return Some(format!("{} does not exist", s.path.display())),
    }
    if let Err(why) = check_description(&s.description) {
        return Some(why);
    }
    None
}

/// Drop every entry the host would reject, duplicates included, and cap
/// the count at the host's per-turn limit.
pub fn validate(root: &Path, selected: Vec<Selected>) -> Vec<Selected> {
    let mut out: Vec<Selected> = Vec::new();
    for s in selected {
        if entry_problem(root, &s).is_some() {
            continue;
        }
        if out.iter().any(|o| o.path == s.path || o.id == s.id) {
            continue;
        }
        out.push(s);
        if out.len() >= hr::ROUTED_SKILLS_PER_TURN {
            break;
        }
    }
    out
}

/// The wire shape: exactly `id`, `path`, `description` per entry.
pub fn payload(selected: &[Selected]) -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": EVENT_USER_PROMPT_SUBMIT,
            "selectedSkills": selected.iter().map(|s| json!({
                "id": s.id,
                "path": s.path.display().to_string(),
                "description": s.description,
            })).collect::<Vec<_>>(),
        }
    })
}

// ---- the order-200 size the router budgets against ------------------------

/// Where the order-200 figure came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order200Source {
    /// `$OMM/config.json` → `routing.order200_bytes` (this workspace).
    Config,
    /// `<ws>/.omm/routing.json` → `order200_bytes`.
    Workspace,
    /// The catalog estimate of the session-start hook (`estimate_catalog`).
    Estimate,
    /// Nothing measured and no content bundle: the built-in block alone.
    BuiltinsOnly,
}

impl Order200Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Order200Source::Config => "config",
            Order200Source::Workspace => "workspace",
            Order200Source::Estimate => "estimate",
            Order200Source::BuiltinsOnly => "builtins-only",
        }
    }
}

/// The per-workspace router state, as `omm enable` writes it and the hook
/// reads it (`WS_STATE_FILE`; mirrored in `$OMM/config.json` → `routing`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub workspace: Option<PathBuf>,
    pub hooks_file: Option<PathBuf>,
    pub handler_command: Option<String>,
    pub order200_bytes: Option<u64>,
    pub order200_source: Option<String>,
    pub measured_at: Option<String>,
    pub skills: Vec<String>,
    pub order201_max_bytes: Option<u64>,
}

impl State {
    /// From a `routing` object (`config.json`) or the state file document.
    pub fn from_value(v: &Value) -> State {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        State {
            workspace: s("workspace").map(PathBuf::from),
            hooks_file: s("hooks_file").map(PathBuf::from),
            handler_command: s("handler_command"),
            order200_bytes: v.get("order200_bytes").and_then(Value::as_u64),
            order200_source: s("order200_source"),
            measured_at: s("measured_at"),
            skills: v
                .get("skills")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            order201_max_bytes: v.get("order201_max_bytes").and_then(Value::as_u64),
        }
    }

    /// The JSON object (the same shape in both places).
    pub fn to_value(&self, omm_version: &str) -> Value {
        let mut m = Map::new();
        m.insert("schema_version".into(), json!(STATE_SCHEMA_VERSION));
        m.insert("omm_version".into(), json!(omm_version));
        if let Some(w) = &self.workspace {
            m.insert("workspace".into(), json!(w.display().to_string()));
        }
        if let Some(h) = &self.hooks_file {
            m.insert("hooks_file".into(), json!(h.display().to_string()));
        }
        if let Some(c) = &self.handler_command {
            m.insert("handler_command".into(), json!(c));
        }
        m.insert("order200_bytes".into(), json!(self.order200_bytes));
        m.insert("order200_source".into(), json!(self.order200_source));
        m.insert("measured_at".into(), json!(self.measured_at));
        m.insert("order201_max_bytes".into(), json!(self.order201_max_bytes));
        m.insert("skills".into(), json!(self.skills));
        m.insert(
            "gates".into(),
            json!([hr::ENV_ROUTING_GATE, hr::ENV_ROUTING_APPLY_GATE]),
        );
        Value::Object(m)
    }

    /// True when `other` records the same measurement and setup — every
    /// field but `measured_at` — so a rerun of `omm enable` can keep the
    /// earlier timestamp and leave the files byte-identical.
    pub fn same_measurement(&self, other: &State) -> bool {
        let strip = |s: &State| State {
            measured_at: None,
            ..s.clone()
        };
        strip(self) == strip(other)
    }

    /// Parse the state file's bytes (malformed → default).
    pub fn from_bytes_lenient(bytes: &[u8]) -> State {
        serde_json::from_slice::<Value>(bytes)
            .map(|v| State::from_value(&v))
            .unwrap_or_default()
    }

    /// The `routing` record of a config document, if any.
    pub fn from_config(config: &OmmConfig) -> Option<State> {
        config.get(CONFIG_KEY_ROUTING).map(State::from_value)
    }
}

/// The order-200 size to budget against for `ws`: the larger of the two
/// recorded measurements (`$OMM/config.json` when it names this workspace,
/// `<ws>/.omm/routing.json`), else the catalog estimate, else the built-in
/// block alone. Never spawns.
pub fn order200_for(roots: &Roots, ws: &Path) -> (u64, Order200Source) {
    let from_config = std::fs::read(OmmConfig::path(&roots.omm_root()))
        .ok()
        .map(|b| OmmConfig::from_bytes_lenient(&b))
        .and_then(|c| State::from_config(&c))
        .filter(|s| s.workspace.as_deref() == Some(ws))
        .and_then(|s| s.order200_bytes);
    let from_ws = std::fs::read(ws.join(WS_STATE_FILE))
        .ok()
        .map(|b| State::from_bytes_lenient(&b))
        .and_then(|s| s.order200_bytes);
    match (from_config, from_ws) {
        (Some(c), Some(w)) if c >= w => (c, Order200Source::Config),
        (Some(_), Some(w)) => (w, Order200Source::Workspace),
        (Some(c), None) => (c, Order200Source::Config),
        (None, Some(w)) => (w, Order200Source::Workspace),
        (None, None) => match estimate_catalog(roots) {
            Some(e) => (e.bytes, Order200Source::Estimate),
            None => (
                hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES,
                Order200Source::BuiltinsOnly,
            ),
        },
    }
}

// ---- the hooks.json handler ------------------------------------------------

/// POSIX single-quoting for `$SHELL -c` (hooks.md: the handler string is
/// run by the user's login shell, `/bin/sh` when `SHELL` is unset). A word
/// of safe characters passes as is.
pub fn sh_quote(s: &str) -> String {
    let safe = !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b'/' | b'.' | b'_' | b'-' | b'+' | b':' | b'=' | b'@' | b'%' | b','
                )
        });
    if safe {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// The program word of a handler command: `omm` when `omm` on `PATH` is
/// this very executable (then a reinstall elsewhere on `PATH` keeps
/// working), else this executable's absolute path, shell-quoted.
pub fn handler_program(exe: &Path, path_var: Option<&std::ffi::OsStr>) -> String {
    let canonical = omm_host::fsx::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    if let Some(path) = path_var {
        let name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("omm");
        for dir in std::env::split_paths(path) {
            let candidate = dir.join(name);
            if omm_host::fsx::canonicalize(&candidate).ok().as_deref() == Some(canonical.as_path())
            {
                return name.to_string();
            }
            if candidate.is_file() {
                // Another `omm` shadows this one on PATH: name ours by path.
                break;
            }
        }
    }
    sh_quote(&canonical.display().to_string())
}

/// `<program> hook route`.
pub fn handler_command(program: &str) -> String {
    format!("{program} hook {HANDLER_ROUTE}")
}

/// The handler document (hooks.md §3.3: `type`, `command`, `timeout` in
/// seconds, `statusMessage`, `outputCapabilities`; nothing else — an
/// unknown field drops the handler).
pub fn handler_value(command: &str) -> Value {
    json!({
        "type": "command",
        "command": command,
        "timeout": HANDLER_TIMEOUT_SECS,
        "statusMessage": HANDLER_STATUS,
        "outputCapabilities": [SKILLS_V1],
    })
}

/// True for a handler omm wrote (any program word): `… hook route` with
/// the capability declared.
pub fn is_omm_handler(v: &Value) -> bool {
    let cmd = v.get("command").and_then(Value::as_str).unwrap_or("");
    let caps = v.get("outputCapabilities");
    cmd.ends_with(&format!(" hook {HANDLER_ROUTE}")) && caps == Some(&json!([SKILLS_V1]))
}

/// Ensure `doc` (an object, possibly empty) carries exactly one omm handler
/// with `command`, in its own matcher-less group under
/// `hooks.UserPromptSubmit`. Returns true when the document changed. An
/// existing omm handler with another command is replaced in place.
pub fn merge_handler(doc: &mut Value, command: &str) -> Result<bool> {
    let obj = doc.as_object_mut().ok_or_else(|| {
        OmmError::Usage("hooks.json is not a JSON object; repair it by hand first".into())
    })?;
    let hooks = obj
        .entry("hooks".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let hooks = hooks.as_object_mut().ok_or_else(|| {
        OmmError::Usage("hooks.json: `hooks` is not an object (the host rejects the file)".into())
    })?;
    let groups = hooks
        .entry(EVENT_USER_PROMPT_SUBMIT.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    let groups = groups.as_array_mut().ok_or_else(|| {
        OmmError::Usage(format!(
            "hooks.json: `hooks.{EVENT_USER_PROMPT_SUBMIT}` is not an array (the host rejects the event)"
        ))
    })?;
    let wanted = handler_value(command);
    let mut changed = false;
    let mut found = false;
    for group in groups.iter_mut() {
        let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        for h in handlers.iter_mut() {
            if is_omm_handler(h) {
                if found {
                    continue;
                }
                found = true;
                if *h != wanted {
                    *h = wanted.clone();
                    changed = true;
                }
            }
        }
    }
    if !found {
        groups.push(json!({ "hooks": [wanted] }));
        changed = true;
    }
    Ok(changed)
}

/// Remove every omm handler from `doc`; a group, the event array and the
/// `hooks` object left empty by that go too (so a file omm merged into
/// returns to its pre-omm document). Returns how many handlers went.
pub fn remove_handler(doc: &mut Value) -> usize {
    let mut removed = 0;
    let Some(obj) = doc.as_object_mut() else {
        return 0;
    };
    let mut hooks_empty = false;
    if let Some(hooks) = obj.get_mut("hooks").and_then(Value::as_object_mut) {
        let mut drop_event = false;
        if let Some(groups) = hooks
            .get_mut(EVENT_USER_PROMPT_SUBMIT)
            .and_then(Value::as_array_mut)
        {
            groups.retain_mut(|group| {
                let Some(handlers) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                    return true;
                };
                let before = handlers.len();
                handlers.retain(|h| !is_omm_handler(h));
                removed += before - handlers.len();
                // Only a group omm emptied goes; a user's empty group stays.
                !(before > handlers.len()
                    && handlers.is_empty()
                    && group.as_object().map(|g| g.len() == 1).unwrap_or(false))
            });
            drop_event = removed > 0 && groups.is_empty();
        }
        if drop_event {
            hooks.remove(EVENT_USER_PROMPT_SUBMIT);
        }
        hooks_empty = removed > 0 && hooks.is_empty();
    }
    if hooks_empty {
        obj.remove("hooks");
    }
    removed
}

/// The omm handler commands a hooks document carries.
pub fn handler_commands(doc: &Value) -> Vec<String> {
    doc.get("hooks")
        .and_then(|h| h.get(EVENT_USER_PROMPT_SUBMIT))
        .and_then(Value::as_array)
        .map(|groups| {
            groups
                .iter()
                .filter_map(|g| g.get("hooks").and_then(Value::as_array))
                .flatten()
                .filter(|h| is_omm_handler(h))
                .filter_map(|h| h.get("command").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Pretty JSON (2 spaces) with a trailing newline — how omm writes a hooks
/// document it created or merged into.
pub fn pretty(doc: &Value) -> Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec_pretty(doc).map_err(|e| OmmError::Usage(format!("hooks.json: {e}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

// ---- the hook --------------------------------------------------------------

/// The `UserPromptSubmit` decision of `omm hook route`: `{}` unless the
/// event is the routing event with the capability negotiated, a prompt and
/// an absolute `cwd`; else the validated, budgeted selection from
/// `<cwd>/.omm/skills/`, or `{}` when nothing matches. Never spawns, never
/// errors: a router that cannot decide routes nothing (R16).
pub fn route(event: &Value, roots: &Roots) -> Value {
    if event.get("hook_event_name").and_then(Value::as_str) != Some(EVENT_USER_PROMPT_SUBMIT) {
        return json!({});
    }
    let negotiated = event
        .get("supported_output_capabilities")
        .and_then(Value::as_array)
        .map(|a| a.iter().any(|c| c.as_str() == Some(SKILLS_V1)))
        .unwrap_or(false);
    if !negotiated {
        return json!({});
    }
    let Some(prompt) = event.get("prompt").and_then(Value::as_str) else {
        return json!({});
    };
    let cwd = event
        .get("cwd")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let Some(cwd) = cwd.filter(|p| p.is_absolute()) else {
        return json!({});
    };
    let root = omm_host::fsx::canonicalize(&cwd).unwrap_or(cwd);
    decide(prompt, &root, roots)
}

/// [`route`] after the event checks: prompt + workspace root → decision.
pub fn decide(prompt: &str, root: &Path, roots: &Roots) -> Value {
    let library = load_workspace_library(&root.join(WS_SKILLS_DIR));
    if library.is_empty() {
        return json!({});
    }
    let mut selected = select(prompt, &library);
    if selected.is_empty() {
        return json!({});
    }
    let hooks_file = root.join(WS_HOOKS_FILE);
    let (order200, _) = order200_for(roots, root);
    fit(&mut selected, available_bytes(order200), &hooks_file);
    let mut selected = validate(root, selected);
    // The hook's stdout ceiling is a hard reject of the whole selection.
    loop {
        if selected.is_empty() {
            return json!({});
        }
        let out = payload(&selected);
        if out.to_string().len() as u64 <= hr::HOOK_STDOUT_MAX_BYTES {
            return out;
        }
        selected.pop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skill(id: &str, desc: &str, triggers: &str) -> LibrarySkill {
        LibrarySkill {
            id: id.into(),
            dir: PathBuf::from(format!("/ws/.omm/skills/{id}")),
            path: PathBuf::from(format!("/ws/.omm/skills/{id}/SKILL.md")),
            description: desc.into(),
            triggers: parse_triggers(triggers),
        }
    }

    #[test]
    fn frontmatter_triggers_parse_and_normalise() {
        assert_eq!(
            parse_triggers("Commit message, \"reword\", conventional  commit,, ,reword"),
            vec!["commit message", "reword", "conventional commit"]
        );
        assert_eq!(parse_triggers("[a, b]"), vec!["a", "b"]);
        assert!(parse_triggers("").is_empty());
        assert_eq!(
            normalise("  Fix the FLAKY-test, please!  "),
            "fix the flaky test please"
        );
        assert_eq!(normalise("Ünïcode Ok"), "ünïcode ok");
    }

    #[test]
    fn a_skill_parses_from_its_frontmatter_and_refuses_the_host_rejections() {
        let dir = Path::new("/lib/omm-x");
        let ok = b"---\nname: omm-x\ndescription: Do   the thing; Do not use otherwise.\nmetadata:\n  triggers: do the thing, thing\n---\nbody\n";
        let s = parse_skill("omm-x", dir, ok).unwrap();
        assert_eq!(s.description, "Do the thing; Do not use otherwise.");
        assert_eq!(s.triggers, vec!["do the thing", "thing"]);
        assert_eq!(s.path, dir.join("SKILL.md"));
        let wrong_name = b"---\nname: omm-y\ndescription: d\nmetadata:\n  triggers: a\n---\n";
        assert!(parse_skill("omm-x", dir, wrong_name).is_err());
        let no_triggers = b"---\nname: omm-x\ndescription: d\n---\n";
        assert!(parse_skill("omm-x", dir, no_triggers).is_err());
        let empty_desc = b"---\nname: omm-x\ndescription: \nmetadata:\n  triggers: a\n---\n";
        assert!(parse_skill("omm-x", dir, empty_desc).is_err());
        let long = format!(
            "---\nname: omm-x\ndescription: {}\nmetadata:\n  triggers: a\n---\n",
            "x".repeat(1025)
        );
        assert!(parse_skill("omm-x", dir, long.as_bytes()).is_err());
        let bom = b"\xef\xbb\xbf---\nname: omm-x\ndescription: d\nmetadata:\n  triggers: a\n---\n";
        assert!(parse_skill("omm-x", dir, bom).is_err());
        assert!(check_library_id("omm-", "omm-x").is_ok());
        assert!(check_library_id("omm-", "x").is_err());
        assert!(check_library_id("omm-", "omm-").is_err());
        assert!(check_library_id("omm-", "omm-X").is_err());
    }

    #[test]
    fn scoring_prefers_phrases_and_selection_is_deterministic() {
        let lib = vec![
            skill("omm-a", "A", "commit message, reword"),
            skill("omm-b", "B", "release notes, changelog"),
            skill("omm-c", "C", "flaky, intermittent"),
            skill("omm-d", "D", "message"),
        ];
        let norm = normalise("Please write the commit message and reword the subject");
        assert_eq!(score(&norm, &lib[0]), 3 + 2);
        assert_eq!(score(&norm, &lib[3]), 2);
        assert_eq!(score(&norm, &lib[1]), 0);
        // "committed" does not match "commit message"; a word matches whole.
        assert_eq!(
            trigger_score(&normalise("we committed the message"), "commit message"),
            0
        );
        assert_eq!(trigger_score(&normalise("messages galore"), "message"), 0);
        assert_eq!(trigger_score(&normalise("the message"), "message"), 2);
        let sel = select("commit message for the changelog and the flaky test", &lib);
        let ids: Vec<&str> = sel.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["omm-a", "omm-b", "omm-c"],
            "top three by score, then id"
        );
        assert!(select("nothing relevant here", &lib).is_empty());
        let tie = select(
            "message",
            &[
                skill("omm-z", "Z", "message"),
                skill("omm-y", "Y", "message"),
            ],
        );
        assert_eq!(tie[0].id, "omm-y", "ties break on id");
        // A bare word never matches a phrase trigger: only omm-d routes.
        assert_eq!(select("message message message message", &lib).len(), 1);
    }

    #[test]
    fn budget_arithmetic_reproduces_the_measured_blocks() {
        // Measured 2026-09-03 (`scratchpad/measure/m.sh`): one 2-char id with a
        // 10-byte description under a 141-byte path and a 134-byte hooks path
        // rendered 868 B; two such entries 1,456 B; three (paths +2, third id
        // 3 chars) 2,058 B.
        let ws = Path::new("/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/measure/T_one/ws");
        let hooks = ws.join(".muse/hooks.json");
        let e = |id: &str, ws: &Path| Selected {
            id: id.into(),
            path: ws.join(".omm/skills").join(id).join("SKILL.md"),
            description: "d".repeat(10),
            score: 1,
        };
        assert_eq!(hooks.as_os_str().len(), 134);
        assert_eq!(block_bytes(&[e("aa", ws)], &hooks), 868);
        assert_eq!(block_bytes(&[e("aa", ws), e("bb", ws)], &hooks), 1_456);
        let ws3 = Path::new("/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/measure/T_three/ws");
        let hooks3 = ws3.join(".muse/hooks.json");
        assert_eq!(
            block_bytes(&[e("aa", ws3), e("bb", ws3), e("ccc", ws3)], &hooks3),
            2_058
        );
        assert_eq!(block_bytes(&[], &hooks), 0);
        assert_eq!(xml_escaped_len("a&b<c>\"'"), 1 + 5 + 1 + 4 + 1 + 4 + 6 + 6);
        assert_eq!(
            available_bytes(10_031),
            31_984 - 10_031 - BUDGET_MARGIN_BYTES
        );
        assert_eq!(available_bytes(40_000), 0);
    }

    #[test]
    fn fit_trims_descriptions_from_the_last_entry_then_drops() {
        let hooks = Path::new("/ws/.muse/hooks.json");
        let long = "word ".repeat(60).trim_end().to_string(); // 299 B
        let mk = |id: &str| Selected {
            id: id.into(),
            path: PathBuf::from(format!("/ws/.omm/skills/{id}/SKILL.md")),
            description: long.clone(),
            score: 1,
        };
        let mut sel = vec![mk("omm-a"), mk("omm-b"), mk("omm-c")];
        let full = block_bytes(&sel, hooks);
        fit(&mut sel, full, hooks);
        assert_eq!(sel.len(), 3, "fits as is");
        assert!(sel.iter().all(|s| s.description == long));
        // 100 bytes short: the LAST description is trimmed, the first intact.
        let mut sel = vec![mk("omm-a"), mk("omm-b"), mk("omm-c")];
        fit(&mut sel, full - 100, hooks);
        assert_eq!(sel.len(), 3);
        assert_eq!(sel[0].description, long);
        assert!(sel[2].description.len() <= long.len() - 100);
        assert!(sel[2].description.len() >= DESCRIPTION_MIN_BYTES);
        assert!(!sel[2].description.ends_with(' '));
        assert!(block_bytes(&sel, hooks) <= full - 100);
        // Far too little room: entries drop from the end, the best stays.
        let mut sel = vec![mk("omm-a"), mk("omm-b"), mk("omm-c")];
        let one = block_bytes(&[mk("omm-a")], hooks);
        fit(&mut sel, one, hooks);
        assert_eq!(
            sel.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            vec!["omm-a"]
        );
        assert_eq!(sel[0].description, long);
        // No room at all: nothing.
        let mut sel = vec![mk("omm-a")];
        fit(&mut sel, 10, hooks);
        assert!(sel.is_empty());
        assert_eq!(trim_description("short", 100), Some("short".into()));
        assert_eq!(trim_description(&long, 10), None);
        let t = trim_description(&long, 60).unwrap();
        assert!(t.len() <= 60 && t.len() >= DESCRIPTION_MIN_BYTES && !t.ends_with(' '));
        let utf8 = format!("{} ééééééééé", "a".repeat(50));
        let t = trim_description(&utf8, 55).unwrap();
        assert!(t.is_char_boundary(t.len()) && t.len() <= 55);
    }

    #[test]
    fn worst_case_takes_the_three_most_expensive_entries() {
        let hooks = Path::new("/ws/.muse/hooks.json");
        let lib = vec![
            skill("omm-a", "x", "a"),
            skill("omm-b", &"y".repeat(100), "b"),
            skill("omm-c", &"z".repeat(50), "c"),
            skill("omm-d", &"w".repeat(200), "d"),
        ];
        let worst = worst_case_bytes(&lib, hooks);
        let expected = BLOCK_FRAME_BYTES
            + entry_bytes("omm-d", &lib[3].path, hooks, &lib[3].description)
            + entry_bytes("omm-b", &lib[1].path, hooks, &lib[1].description)
            + entry_bytes("omm-c", &lib[2].path, hooks, &lib[2].description);
        assert_eq!(worst, expected);
        assert_eq!(worst_case_bytes(&[], hooks), 0);
    }

    #[test]
    fn validation_mirrors_the_host_rejections() {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let dir = root.join(".omm/skills/omm-a");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), b"x").unwrap();
        let good = Selected {
            id: "omm-a".into(),
            path: dir.join("SKILL.md"),
            description: "d".into(),
            score: 1,
        };
        assert_eq!(entry_problem(&root, &good), None);
        let bad = |f: &dyn Fn(&mut Selected)| {
            let mut s = good.clone();
            f(&mut s);
            entry_problem(&root, &s).is_some()
        };
        assert!(bad(&|s| s.id = "Omm-A".into()));
        assert!(bad(&|s| s.path = PathBuf::from("relative/SKILL.md")));
        assert!(bad(
            &|s| s.path = root.join(".omm/skills/omm-a/../omm-a/SKILL.md")
        ));
        assert!(bad(&|s| s.path = PathBuf::from("/etc/passwd/SKILL.md")));
        assert!(bad(&|s| s.path = dir.clone()));
        assert!(bad(&|s| s.path = root.join(".omm/skills/omm-a/other.md")));
        assert!(bad(&|s| s.path = root.join(".omm/skills/omm-zz/SKILL.md")));
        assert!(bad(&|s| s.description = String::new()));
        assert!(bad(&|s| s.description = "x".repeat(1025)));
        #[cfg(unix)]
        {
            let link = root.join(".omm/skills/omm-l");
            std::fs::create_dir_all(&link).unwrap();
            std::os::unix::fs::symlink(dir.join("SKILL.md"), link.join("SKILL.md")).unwrap();
            assert!(
                bad(&|s| s.path = link.join("SKILL.md")),
                "a symlink is refused"
            );
        }
        let dup = vec![
            good.clone(),
            good.clone(),
            Selected {
                id: "omm-b".into(),
                ..good.clone()
            },
        ];
        let v = validate(&root, dup);
        assert_eq!(v.len(), 1, "duplicate paths and ids collapse");
        let p = payload(&v);
        assert_eq!(p["hookSpecificOutput"]["hookEventName"], "UserPromptSubmit");
        let entry = &p["hookSpecificOutput"]["selectedSkills"][0];
        assert_eq!(
            entry.as_object().unwrap().len(),
            3,
            "exactly id, path, description"
        );
        assert_eq!(entry["id"], "omm-a");
    }

    #[test]
    fn handler_shape_merge_and_remove_round_trip() {
        assert_eq!(sh_quote("/usr/local/bin/omm"), "/usr/local/bin/omm");
        assert_eq!(
            sh_quote("/Volumes/OWC Envoy/omm"),
            "'/Volumes/OWC Envoy/omm'"
        );
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        let cmd = handler_command("omm");
        assert_eq!(cmd, "omm hook route");
        let h = handler_value(&cmd);
        assert_eq!(h["type"], "command");
        assert_eq!(h["timeout"], HANDLER_TIMEOUT_SECS);
        assert_eq!(h["outputCapabilities"], json!(["skills.v1"]));
        assert_eq!(
            h.as_object().unwrap().len(),
            5,
            "no unknown field (it would drop the handler)"
        );
        assert!(is_omm_handler(&h));
        assert!(!is_omm_handler(
            &json!({"type":"command","command":"omm hook route"})
        ));
        assert!(!is_omm_handler(
            &json!({"type":"command","command":"x","outputCapabilities":["skills.v1"]})
        ));

        // Fresh document.
        let mut doc = json!({});
        assert!(merge_handler(&mut doc, &cmd).unwrap());
        assert_eq!(doc["hooks"]["UserPromptSubmit"][0]["hooks"][0], h);
        assert!(!merge_handler(&mut doc, &cmd).unwrap(), "idempotent");
        assert_eq!(handler_commands(&doc), vec![cmd.clone()]);
        assert_eq!(remove_handler(&mut doc), 1);
        assert_eq!(doc, json!({}), "back to the empty document");

        // A user's file: their handlers stay, ours joins in its own group.
        let original = json!({
            "hooks": {
                "SessionStart": [{"matcher": "startup", "hooks": [{"type": "command", "command": "echo hi"}]}],
                "UserPromptSubmit": [{"hooks": [{"type": "command", "command": "./their-router.sh", "outputCapabilities": ["skills.v1"]}]}]
            },
            "note": "kept"
        });
        let mut doc = original.clone();
        assert!(merge_handler(&mut doc, &cmd).unwrap());
        assert_eq!(
            doc["hooks"]["UserPromptSubmit"].as_array().unwrap().len(),
            2
        );
        assert_eq!(
            doc["hooks"]["UserPromptSubmit"][0],
            original["hooks"]["UserPromptSubmit"][0]
        );
        // Replace in place when the command changes.
        let other = handler_command("/x/omm");
        assert!(merge_handler(&mut doc, &other).unwrap());
        assert_eq!(handler_commands(&doc), vec![other.clone()]);
        assert_eq!(
            doc["hooks"]["UserPromptSubmit"].as_array().unwrap().len(),
            2
        );
        assert_eq!(remove_handler(&mut doc), 1);
        assert_eq!(doc, original, "byte-for-byte document restored");
        // Their empty group survives a removal that touched nothing in it.
        let mut doc = json!({"hooks": {"UserPromptSubmit": [{"hooks": []}]}});
        assert!(merge_handler(&mut doc, &cmd).unwrap());
        assert_eq!(remove_handler(&mut doc), 1);
        assert_eq!(doc, json!({"hooks": {"UserPromptSubmit": [{"hooks": []}]}}));
        // A malformed document is refused, not clobbered.
        assert!(merge_handler(&mut json!([]), &cmd).is_err());
        assert!(merge_handler(&mut json!({"hooks": 5}), &cmd).is_err());
        assert!(merge_handler(&mut json!({"hooks": {"UserPromptSubmit": {}}}), &cmd).is_err());
    }

    #[test]
    fn the_program_word_is_omm_only_when_path_resolves_to_this_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("omm");
        std::fs::write(&exe, b"#!/bin/sh\n").unwrap();
        let canonical = std::fs::canonicalize(&exe).unwrap();
        let path = std::env::join_paths([bin.clone()]).unwrap();
        assert_eq!(handler_program(&exe, Some(&path)), "omm");
        assert_eq!(
            handler_program(&exe, None),
            sh_quote(&canonical.display().to_string())
        );
        // Another omm ahead on PATH: ours is named by path.
        let other = tmp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("omm"), b"").unwrap();
        let path = std::env::join_paths([other, bin]).unwrap();
        assert_eq!(
            handler_program(&exe, Some(&path)),
            sh_quote(&canonical.display().to_string())
        );
        assert!(handler_command(&handler_program(&exe, None)).ends_with(" hook route"));
    }

    #[test]
    fn state_round_trips_and_order200_prefers_the_larger_measurement() {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let roots = sb.roots().unwrap();
        let ws = std::fs::canonicalize(tmp.path()).unwrap().join("ws");
        std::fs::create_dir_all(ws.join(".omm")).unwrap();
        let state = State {
            workspace: Some(ws.clone()),
            hooks_file: Some(ws.join(".muse/hooks.json")),
            handler_command: Some("omm hook route".into()),
            order200_bytes: Some(12_000),
            order200_source: Some("measured".into()),
            measured_at: Some("t".into()),
            skills: vec!["omm-a".into()],
            order201_max_bytes: Some(1_500),
        };
        let v = state.to_value("0.1.0");
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["gates"][0], hr::ENV_ROUTING_GATE);
        assert_eq!(State::from_value(&v), state);
        // Nothing recorded: the estimate (the checkout's catalog) or the built-ins.
        let (bytes, src) = order200_for(&roots, &ws);
        assert!(
            matches!(src, Order200Source::Estimate | Order200Source::BuiltinsOnly),
            "{src:?}"
        );
        assert!(bytes >= hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF);
        // The workspace file.
        std::fs::write(ws.join(WS_STATE_FILE), serde_json::to_vec(&v).unwrap()).unwrap();
        assert_eq!(
            order200_for(&roots, &ws),
            (12_000, Order200Source::Workspace)
        );
        // config.json for THIS workspace, larger: wins; for another: ignored.
        let mut cfg = OmmConfig::default();
        let mut bigger = state.clone();
        bigger.order200_bytes = Some(13_000);
        cfg.set(CONFIG_KEY_ROUTING, bigger.to_value("0.1.0"));
        cfg.save(&roots.omm_root()).unwrap();
        assert_eq!(order200_for(&roots, &ws), (13_000, Order200Source::Config));
        let mut elsewhere = bigger.clone();
        elsewhere.workspace = Some(PathBuf::from("/elsewhere"));
        elsewhere.order200_bytes = Some(30_000);
        cfg.set(CONFIG_KEY_ROUTING, elsewhere.to_value("0.1.0"));
        cfg.save(&roots.omm_root()).unwrap();
        assert_eq!(
            order200_for(&roots, &ws),
            (12_000, Order200Source::Workspace)
        );
        assert_eq!(State::from_bytes_lenient(b"nope"), State::default());
    }

    #[test]
    fn the_hook_routes_only_a_negotiated_prompt_and_budgets_the_answer() {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let roots = sb.roots().unwrap();
        let ws = std::fs::canonicalize(tmp.path()).unwrap().join("ws");
        for (id, desc, trig) in [
            ("omm-a", "Write the commit message.", "commit message"),
            ("omm-b", "Draft release notes.", "release notes, changelog"),
            ("omm-c", "Never routed.", "zzz"),
        ] {
            let d = ws.join(WS_SKILLS_DIR).join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(
                d.join(SKILL_FILE),
                format!("---\nname: {id}\ndescription: {desc}\nmetadata:\n  triggers: {trig}\n---\nbody\n"),
            )
            .unwrap();
        }
        let ev = |caps: Value, prompt: &str| {
            json!({"hook_event_name": "UserPromptSubmit", "prompt": prompt, "cwd": ws.display().to_string(),
                "session_id": "s", "turn_id": "t", "supported_output_capabilities": caps})
        };
        // Negotiated: the two matching skills, best first, exactly three keys each.
        let out = route(
            &ev(
                json!(["skills.v1"]),
                "write the commit message and the changelog",
            ),
            &roots,
        );
        let sel = out["hookSpecificOutput"]["selectedSkills"]
            .as_array()
            .unwrap();
        assert_eq!(sel.len(), 2);
        assert_eq!(sel[0]["id"], "omm-a");
        assert_eq!(sel[1]["id"], "omm-b");
        assert_eq!(
            sel[0]["path"],
            ws.join(".omm/skills/omm-a/SKILL.md").display().to_string()
        );
        assert_eq!(sel[0]["description"], "Write the commit message.");
        // Not negotiated (either gate off, or the field absent): `{}`, never a
        // `capability-not-negotiated` failure.
        assert_eq!(route(&ev(json!([]), "commit message"), &roots), json!({}));
        let mut no_field = ev(json!([]), "commit message");
        no_field
            .as_object_mut()
            .unwrap()
            .remove("supported_output_capabilities");
        assert_eq!(route(&no_field, &roots), json!({}));
        // No match, wrong event, no prompt, relative cwd: `{}`.
        assert_eq!(
            route(&ev(json!(["skills.v1"]), "hello there"), &roots),
            json!({})
        );
        let mut wrong = ev(json!(["skills.v1"]), "commit message");
        wrong["hook_event_name"] = json!("SessionStart");
        assert_eq!(route(&wrong, &roots), json!({}));
        let mut no_prompt = ev(json!(["skills.v1"]), "commit message");
        no_prompt.as_object_mut().unwrap().remove("prompt");
        assert_eq!(route(&no_prompt, &roots), json!({}));
        let mut rel = ev(json!(["skills.v1"]), "commit message");
        rel["cwd"] = json!("relative");
        assert_eq!(route(&rel, &roots), json!({}));
        // An order-200 measurement that leaves no room: nothing is routed
        // rather than a silently description-less or rejected block.
        let state = State {
            workspace: Some(ws.clone()),
            order200_bytes: Some(hr::ROUTING_BUDGET_BYTES),
            ..State::default()
        };
        std::fs::write(
            ws.join(WS_STATE_FILE),
            serde_json::to_vec(&state.to_value("t")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            route(&ev(json!(["skills.v1"]), "commit message"), &roots),
            json!({})
        );
        // A workspace without a library: `{}`.
        let empty = ev(json!(["skills.v1"]), "commit message");
        let mut e = empty.clone();
        e["cwd"] = json!(std::fs::canonicalize(tmp.path())
            .unwrap()
            .display()
            .to_string());
        assert_eq!(route(&e, &roots), json!({}));
    }

    #[test]
    fn the_decision_itself_takes_well_under_a_millisecond() {
        // R16 budgets the whole hook at 5 ms; process start takes most of
        // it, so the in-memory decision over the shipped library — read,
        // parse, score, budget, validate, serialise — must stay far below.
        let source = super::super::ContentSource::locate(Path::new("/nonexistent"))
            .expect("the checkout's content/");
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let roots = sb.roots().unwrap();
        let ws = std::fs::canonicalize(tmp.path()).unwrap().join("ws");
        let lib = source.root.join(LIBRARY_REL);
        for entry in std::fs::read_dir(&lib).unwrap().flatten() {
            if !entry.file_type().unwrap().is_dir() {
                continue;
            }
            let dest = ws.join(WS_SKILLS_DIR).join(entry.file_name());
            std::fs::create_dir_all(&dest).unwrap();
            std::fs::copy(entry.path().join(SKILL_FILE), dest.join(SKILL_FILE)).unwrap();
        }
        let prompt = "please write the commit message and the release notes for the flaky test fix";
        let first = decide(prompt, &ws, &roots);
        assert!(!first["hookSpecificOutput"]["selectedSkills"]
            .as_array()
            .unwrap()
            .is_empty());
        let mut samples: Vec<std::time::Duration> = (0..50)
            .map(|_| {
                let t = std::time::Instant::now();
                let out = decide(prompt, &ws, &roots);
                assert_eq!(out, first);
                t.elapsed()
            })
            .collect();
        samples.sort();
        let p50 = samples[25];
        eprintln!(
            "route decision over the shipped library: p50 {p50:?} max {:?}",
            samples[49]
        );
        assert!(
            p50 < std::time::Duration::from_millis(1),
            "the decision alone must stay under 1 ms (p50 {p50:?})"
        );
    }

    #[test]
    fn the_shipped_library_loads_strictly_and_fits_any_measured_catalog() {
        let source = super::super::ContentSource::locate(Path::new("/nonexistent"))
            .expect("the checkout's content/");
        let catalog = source.catalog().unwrap();
        let taken: Vec<String> = catalog.ids().into_iter().map(str::to_string).collect();
        let lib = load_library(
            &source.root.join(LIBRARY_REL),
            omm_manifest::lint::ID_PREFIX,
            &taken,
        )
        .unwrap();
        assert!(
            lib.len() >= 6 && lib.len() <= 8,
            "6–8 sharply-triggered skills: {}",
            lib.len()
        );
        for s in &lib {
            assert!(
                s.description.len() <= 320,
                "{}: keep descriptions short ({} B)",
                s.id,
                s.description.len()
            );
            assert!(
                s.description.contains("Do not use"),
                "{}: negative clause",
                s.id
            );
            assert!(s.triggers.len() >= 4, "{}: sharp triggers", s.id);
        }
        // Every skill routes on its own first trigger and outranks the rest.
        for s in &lib {
            let sel = select(&s.triggers[0], &lib);
            assert_eq!(
                sel.first().map(|x| x.id.as_str()),
                Some(s.id.as_str()),
                "{}: {:?}",
                s.id,
                s.triggers[0]
            );
        }
        // Worst case under the bundle's own full-description catalog with the
        // built-ins on (R18 ceiling), in a long workspace path.
        let ws = Path::new("/Users/someone/src/a-rather-long-workspace-name/nested/project");
        let hooks = ws.join(WS_HOOKS_FILE);
        let placed: Vec<LibrarySkill> = lib
            .iter()
            .map(|s| LibrarySkill {
                path: ws.join(WS_SKILLS_DIR).join(&s.id).join(SKILL_FILE),
                ..s.clone()
            })
            .collect();
        let worst = worst_case_bytes(&placed, &hooks);
        assert!(worst < 4_000, "three entries stay small: {worst}");
        // Room beside the built-ins and the bundle's own catalog bytes, under
        // the two settings a user actually runs with:
        // (a) the default profile — `first_sentence` descriptions — plus 8 KB
        //     of the user's own skills: the routed block must always fit;
        // (b) `full` descriptions with no user skills: the worst case no
        //     longer fits whole (2,657 B of block against ~267 B of room on
        //     1.3.0-R3057.1), so the router must trim — `fit` upholds the
        //     contract by emitting fewer entries (or none), never overrunning
        //     the shared budget (docs/ROUTING.md, "The caps and the shared
        //     budget"). That is the host's budget, not a defect.
        let budget_field = |key: &str| {
            catalog
                .budget
                .as_ref()
                .and_then(|b| b.get(key))
                .and_then(serde_json::Value::as_u64)
        };
        let bundle_full = budget_field("skills_total_bytes").unwrap_or(hr::BUNDLE_BUDGET_BYTES);
        let bundle_first = budget_field("skills_total_first_sentence_bytes").unwrap_or(bundle_full);
        assert!(
            worst
                <= available_bytes(
                    hr::BUILTIN_SKILLS_FIRST_SENTENCE_BLOCK_BYTES + bundle_first + 8_000
                ),
            "worst {worst} B beside {bundle_first} B of bundle (first_sentence) + 8 KB user skills"
        );
        let room_full = available_bytes(hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES + bundle_full);
        let mut fitted: Vec<Selected> = placed
            .iter()
            .map(|s| Selected {
                id: s.id.clone(),
                path: s.path.clone(),
                description: s.description.clone(),
                score: 0,
            })
            .collect();
        fitted.sort_by_key(|s| {
            std::cmp::Reverse(entry_bytes(&s.id, &s.path, &hooks, &s.description))
        });
        fitted.truncate(MAX_SELECTED);
        fit(&mut fitted, room_full, &hooks);
        assert!(
            block_bytes(&fitted, &hooks) <= room_full,
            "fitted {} B must fit in {room_full} B of room (full), no user skills",
            block_bytes(&fitted, &hooks)
        );
        // A collision with a catalog id is refused.
        let mut taken2 = taken.clone();
        taken2.push(lib[0].id.clone());
        assert!(load_library(
            &source.root.join(LIBRARY_REL),
            omm_manifest::lint::ID_PREFIX,
            &taken2
        )
        .is_err());
    }
}
