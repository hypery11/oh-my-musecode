//! The content model over `content/` (ARCHITECTURE.md §5.1):
//! `skills/<id>/SKILL.md` (+ `references/*`), `commands/<id>.md`,
//! `hooks/<id>.json` (one hook per file, in the NATIVE `capabilities.hooks[]`
//! shape — a CLOSED field set, `command` only, never `commandWindows`: R16,
//! `docs/experiments/marketplace-precedence.md` §7), `reminders/<id>.json`
//! (+ its duty file, passed through and validated by the host),
//! `mcp/<id>.json`, `agents/<name>.md`, `themes/<name>.tmTheme`,
//! `profiles/<name>.json`, `rules/AGENTS.md.tmpl`, `translation/muse.md`.
//!
//! Loading is bidirectional against the catalog: every shipped asset must
//! exist on disk and every file on disk must be claimed by an asset (a skill
//! claims its whole directory, a reminder its duty file) or be one of the two
//! documentation files (`README.md`, `VALIDATION.md`) that are never packaged
//! (`content/VALIDATION.md` §8 note 8). Structural problems do not abort the
//! load — they are collected as lint findings in [`Content::problems`] so
//! `omm lint` can report all of them, and the generator refuses to run while
//! any remain ([`Content::require_clean`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::catalog::{Asset, AssetKind, Catalog};
use crate::error::{ManifestError, Result};
use crate::frontmatter::{self, FrontMatter};
use crate::hr;
use crate::lint::{Finding, Severity};
use crate::posix;

/// Documentation files under `content/` that are neither cataloged nor
/// packaged (`content/VALIDATION.md` §8 note 8).
pub const DOC_FILE_NAMES: [&str; 2] = ["README.md", "VALIDATION.md"];
/// `content/routing/` — the routed skill library `omm enable skill-routing`
/// copies into a workspace (PLAN.md 3.1; `content/routing/README.md`). Claimed
/// as a whole and never packaged: the router's whole point is to carry what
/// the order-200 catalog does not, so nothing under it is a catalog asset.
/// The scan still covers it (a symlink or backslash there is a finding, R12).
pub const ROUTING_DIR: &str = "routing";
/// The skill entry file (`research/musecode/skills.md` §2).
pub const SKILL_FILE: &str = "SKILL.md";

/// A regular file under `content/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentFile {
    /// `/`-separated path relative to `content/`.
    pub rel: String,
    /// Canonical absolute path.
    pub abs: PathBuf,
    pub bytes: u64,
}

impl ContentFile {
    /// Read the file.
    pub fn read(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.abs).map_err(|e| ManifestError::io("read", &self.abs, e))
    }
}

/// `skills/<id>/SKILL.md` and everything beside it.
#[derive(Clone, Debug)]
pub struct Skill {
    pub asset: Asset,
    /// `skills/<id>`.
    pub dir: String,
    pub skill_md: ContentFile,
    /// Every file under the skill directory, SKILL.md first, then sorted.
    pub files: Vec<ContentFile>,
    pub frontmatter: FrontMatter,
    /// The raw `description` (the catalog renders it whitespace-collapsed).
    pub description: String,
}

impl Skill {
    /// Bytes of the body after the front matter (§5.3: ≤ 8,192 B).
    pub fn body_bytes(&self) -> usize {
        self.frontmatter.body.len()
    }
}

/// `commands/<id>.md`.
#[derive(Clone, Debug)]
pub struct CommandDef {
    pub asset: Asset,
    pub file: ContentFile,
    pub frontmatter: FrontMatter,
    pub description: String,
    pub argument_hint: Option<String>,
}

/// `hooks/<id>.json` — one native `capabilities.hooks[]` entry.
#[derive(Clone, Debug)]
pub struct Hook {
    pub asset: Asset,
    pub file: ContentFile,
    /// The entry verbatim (closed field set, already checked).
    pub raw: Map<String, Value>,
    pub id: String,
    pub event: String,
    pub command: Vec<String>,
    pub timeout_ms: Option<i64>,
    pub status_message: Option<String>,
    pub is_async: bool,
    pub compatibility_name: Option<String>,
    pub output_capabilities: Vec<String>,
}

/// `reminders/<id>.json` — one native `capabilities.reminders[]` entry plus its duty file.
#[derive(Clone, Debug)]
pub struct Reminder {
    pub asset: Asset,
    pub file: ContentFile,
    pub duty: ContentFile,
    /// The declaration verbatim (the host validates the `decision` block).
    pub raw: Map<String, Value>,
    pub id: String,
    /// The `path` the declaration carries (package-relative duty path).
    pub path: String,
    pub enabled_default: bool,
}

impl Reminder {
    /// `decision.envelope.template`, if declared.
    pub fn envelope_template(&self) -> Option<&str> {
        self.raw
            .get("decision")?
            .get("envelope")?
            .get("template")?
            .as_str()
    }
}

/// `mcp/<id>.json` — one native `capabilities.mcpServers[]` entry.
#[derive(Clone, Debug)]
pub struct McpServer {
    pub asset: Asset,
    pub file: ContentFile,
    pub raw: Map<String, Value>,
    pub id: String,
    /// `stdio` (default) or `http`.
    pub transport: String,
}

/// A plain file asset (agent, theme, profile, rules, translation).
#[derive(Clone, Debug)]
pub struct FileAsset {
    pub asset: Asset,
    pub file: ContentFile,
}

/// `profiles/<name>.json` — a settings slice of typed keys.
#[derive(Clone, Debug)]
pub struct Profile {
    pub asset: Asset,
    pub file: ContentFile,
    pub settings: Map<String, Value>,
}

/// The loaded tree.
#[derive(Clone, Debug)]
pub struct Content {
    /// Canonical `content/`.
    pub root: PathBuf,
    pub plugin_id: String,
    pub skills: Vec<Skill>,
    pub commands: Vec<CommandDef>,
    pub hooks: Vec<Hook>,
    pub reminders: Vec<Reminder>,
    pub mcp_servers: Vec<McpServer>,
    pub agents: Vec<FileAsset>,
    pub themes: Vec<FileAsset>,
    pub profiles: Vec<Profile>,
    pub rules: Vec<FileAsset>,
    pub translations: Vec<FileAsset>,
    /// `README.md` / `VALIDATION.md` — kept out of every package.
    pub docs: Vec<ContentFile>,
    /// Everything under [`ROUTING_DIR`] — the routed library, kept out of
    /// every package (copied per workspace by `omm enable skill-routing`).
    pub routing: Vec<ContentFile>,
    /// Every regular file found, sorted by relative path.
    pub files: Vec<ContentFile>,
    /// Structural problems found while loading (lint findings, all errors).
    pub problems: Vec<Finding>,
}

/// `mcpServers` entry keys the host accepts and then DROPS at launch
/// (`docs/host-reality.md` "mcpServers entries"; plugin-contract.md §1.9) —
/// the generator refuses them because the family is OPEN and validates clean.
pub const MCP_DROPPED_FIELDS: [&str; 3] = ["env", "cwd", "headers"];
/// `mcpServers` entry keys that mean something (plugin-contract.md §1.9).
pub const MCP_KNOWN_FIELDS: [&str; 5] = ["id", "transport", "command", "url", "enabledDefault"];
/// `mcpServers.transport` values (host-reality.md "mcpServers entries").
pub const MCP_TRANSPORTS: [&str; 2] = ["stdio", "http"];

impl Content {
    /// Walk `root` and bind every shipped catalog asset to its files.
    pub fn load(catalog: &Catalog, root: &Path) -> Result<Content> {
        let root = omm_host::fsx::canonicalize(root)?;
        let mut problems = Vec::new();
        let files = scan(&root, &mut problems)?;
        let index: BTreeMap<&str, &ContentFile> =
            files.iter().map(|f| (f.rel.as_str(), f)).collect();
        let mut claimed: BTreeSet<String> = BTreeSet::new();
        claimed.insert(crate::CATALOG_FILE.to_string());

        let mut content = Content {
            root: root.clone(),
            plugin_id: catalog.plugin_id.clone(),
            skills: Vec::new(),
            commands: Vec::new(),
            hooks: Vec::new(),
            reminders: Vec::new(),
            mcp_servers: Vec::new(),
            agents: Vec::new(),
            themes: Vec::new(),
            profiles: Vec::new(),
            rules: Vec::new(),
            translations: Vec::new(),
            docs: Vec::new(),
            routing: Vec::new(),
            files: Vec::new(),
            problems: Vec::new(),
        };

        for asset in catalog.shipped() {
            let Some(file) = index.get(asset.path.as_str()).copied() else {
                problems.push(Finding::error(
                    "catalog-missing-file",
                    root.join(&asset.path),
                    format!(
                        "catalog asset `{}` ({}) names `{}` which does not exist",
                        asset.id,
                        asset.kind.singular(),
                        asset.path
                    ),
                    "create the file or remove the catalog row",
                ));
                continue;
            };
            claimed.insert(file.rel.clone());
            match asset.kind {
                AssetKind::Skill => {
                    if let Some(skill) = load_skill(asset, file, &files, &mut problems) {
                        for f in &skill.files {
                            claimed.insert(f.rel.clone());
                        }
                        content.skills.push(skill);
                    }
                }
                AssetKind::Command => {
                    if let Some(c) = load_command(asset, file, &mut problems) {
                        content.commands.push(c);
                    }
                }
                AssetKind::Hook => {
                    if let Some(h) = load_hook(asset, file, &mut problems)? {
                        content.hooks.push(h);
                    }
                }
                AssetKind::Reminder => {
                    if let Some(r) = load_reminder(asset, file, &index, &mut problems) {
                        claimed.insert(r.duty.rel.clone());
                        content.reminders.push(r);
                    }
                }
                AssetKind::McpServer => {
                    if let Some(m) = load_mcp(asset, file, &mut problems) {
                        content.mcp_servers.push(m);
                    }
                }
                AssetKind::Profile => {
                    if let Some(p) = load_profile(asset, file, &mut problems) {
                        content.profiles.push(p);
                    }
                }
                AssetKind::Agent => content.agents.push(plain(asset, file)),
                AssetKind::Theme => {
                    if theme_ok(file, &mut problems) {
                        content.themes.push(plain(asset, file));
                    }
                }
                AssetKind::Rules => content.rules.push(plain(asset, file)),
                AssetKind::Translation => content.translations.push(plain(asset, file)),
            }
        }

        for f in &files {
            if claimed.contains(&f.rel) {
                continue;
            }
            let name = f.rel.rsplit('/').next().unwrap_or(&f.rel);
            if DOC_FILE_NAMES.contains(&name) {
                content.docs.push(f.clone());
                continue;
            }
            if f.rel.starts_with(&format!("{ROUTING_DIR}/")) {
                content.routing.push(f.clone());
                continue;
            }
            problems.push(Finding::error(
                "catalog-orphan-file",
                f.abs.clone(),
                format!("`{}` is not named by any catalog asset (nor a skill directory or reminder duty)", f.rel),
                "add a catalog row for it, move it under the skill it belongs to, or delete it",
            ));
        }
        content.files = files;
        content.problems = problems;
        Ok(content)
    }

    /// Error unless [`Content::problems`] is empty — the generator's gate.
    pub fn require_clean(&self) -> Result<()> {
        match self.problems.first() {
            None => Ok(()),
            Some(first) => Err(ManifestError::ContentProblems {
                count: self.problems.len(),
                first: format!("{} {}: {}", first.rule, first.path.display(), first.message),
            }),
        }
    }

    /// Every shipped asset, in catalog order, with its primary file.
    pub fn assets(&self) -> Vec<(&Asset, &ContentFile)> {
        let mut out: Vec<(&Asset, &ContentFile)> = Vec::new();
        out.extend(self.skills.iter().map(|s| (&s.asset, &s.skill_md)));
        out.extend(self.commands.iter().map(|c| (&c.asset, &c.file)));
        out.extend(self.hooks.iter().map(|h| (&h.asset, &h.file)));
        out.extend(self.reminders.iter().map(|r| (&r.asset, &r.file)));
        out.extend(self.mcp_servers.iter().map(|m| (&m.asset, &m.file)));
        out.extend(self.agents.iter().map(|a| (&a.asset, &a.file)));
        out.extend(self.themes.iter().map(|a| (&a.asset, &a.file)));
        out.extend(self.profiles.iter().map(|p| (&p.asset, &p.file)));
        out.extend(self.rules.iter().map(|a| (&a.asset, &a.file)));
        out.extend(self.translations.iter().map(|a| (&a.asset, &a.file)));
        out
    }

    /// Look a shipped asset up by id.
    pub fn asset(&self, id: &str) -> Option<&Asset> {
        self.assets()
            .into_iter()
            .find(|(a, _)| a.id == id)
            .map(|(a, _)| a)
    }

    /// The files a skill packages, by id.
    pub fn skill(&self, id: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.asset.id == id)
    }
}

/// Every regular file under `root`, sorted; symlinks, backslashes and
/// non-UTF-8 names become problems (R12; plugin-contract.md §1.4).
fn scan(root: &Path, problems: &mut Vec<Finding>) -> Result<Vec<ContentFile>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
    {
        let entry = entry.map_err(|e| ManifestError::Io {
            context: "walk",
            path: e
                .path()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| root.to_path_buf()),
            source: e
                .into_io_error()
                .unwrap_or_else(|| std::io::Error::other("walk error")),
        })?;
        let path = entry.path();
        if path == root {
            continue;
        }
        if entry.path_is_symlink() {
            problems.push(Finding::error(
                "fs-symlink",
                path.to_path_buf(),
                "symlink under content/; the packer never emits or follows one (R12) and any symlink in a package is fatal (`invalid-plugin-package`)".to_string(),
                "replace the symlink with a regular file or directory",
            ));
            continue;
        }
        let rel_path = path.strip_prefix(root).unwrap_or(path);
        let Some(rel_str) = rel_path.to_str() else {
            problems.push(Finding::error(
                "fs-non-utf8-name",
                path.to_path_buf(),
                "file name is not UTF-8 (`skill package paths must be UTF-8`)".to_string(),
                "rename the file",
            ));
            continue;
        };
        if rel_str.contains('\\') {
            problems.push(Finding::error(
                "fs-backslash",
                path.to_path_buf(),
                "file name contains a backslash; the host reads it as a literal and the package fails (R12; plugin-contract.md §1.4)".to_string(),
                "rename the file",
            ));
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        files.push(ContentFile {
            rel: posix(rel_path),
            abs: path.to_path_buf(),
            bytes,
        });
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(files)
}

fn plain(asset: &Asset, file: &ContentFile) -> FileAsset {
    FileAsset {
        asset: asset.clone(),
        file: file.clone(),
    }
}

fn load_skill(
    asset: &Asset,
    file: &ContentFile,
    all: &[ContentFile],
    problems: &mut Vec<Finding>,
) -> Option<Skill> {
    let expected = format!("{}/{}/{SKILL_FILE}", AssetKind::Skill.plural(), asset.id);
    if asset.path != expected {
        problems.push(Finding::error(
            "skill-path",
            file.abs.clone(),
            format!(
                "skill `{}` lives at `{}`; the directory must be the id: `{expected}`",
                asset.id, asset.path
            ),
            "rename the directory (and the catalog path) to the skill id",
        ));
        return None;
    }
    let dir = asset.dir().to_string();
    let bytes = match file.read() {
        Ok(b) => b,
        Err(e) => {
            problems.push(Finding::error(
                "skill-frontmatter",
                file.abs.clone(),
                e.to_string(),
                "make the file readable",
            ));
            return None;
        }
    };
    let fm = match frontmatter::parse(&bytes) {
        Ok(fm) => fm,
        Err(e) => {
            problems.push(Finding::error(
                "skill-frontmatter",
                file.abs.clone(),
                format!("SKILL.md {e}"),
                "fix the YAML front matter: `---`, `name:`, `description:`, `---`",
            ));
            return None;
        }
    };
    if fm.bom {
        problems.push(Finding::error(
            "skill-bom",
            file.abs.clone(),
            "SKILL.md starts with a UTF-8 BOM (`bom_forbidden`; the plugin validator never opens SKILL.md, only the generator can catch this — R13)".to_string(),
            "strip the BOM",
        ));
    }
    if !fm.duplicate_keys.is_empty() {
        problems.push(Finding::error(
            "skill-frontmatter",
            file.abs.clone(),
            format!(
                "duplicate front-matter key(s) {:?} (`duplicate_key`)",
                fm.duplicate_keys
            ),
            "keep one occurrence of each key",
        ));
    }
    let description = match fm.get("description") {
        Some(d) if !d.trim().is_empty() => d.to_string(),
        _ => {
            problems.push(Finding::error(
                "skill-description-missing",
                file.abs.clone(),
                "SKILL.md frontmatter must include description".to_string(),
                "add a one-line `description:` with a `Do not use …` clause",
            ));
            return None;
        }
    };
    let prefix = format!("{dir}/");
    let mut files: Vec<ContentFile> = all
        .iter()
        .filter(|f| f.rel.starts_with(&prefix))
        .cloned()
        .collect();
    files.sort_by(|a, b| {
        (a.rel != asset.path)
            .cmp(&(b.rel != asset.path))
            .then_with(|| a.rel.cmp(&b.rel))
    });
    Some(Skill {
        asset: asset.clone(),
        dir,
        skill_md: file.clone(),
        files,
        frontmatter: fm,
        description,
    })
}

fn load_command(
    asset: &Asset,
    file: &ContentFile,
    problems: &mut Vec<Finding>,
) -> Option<CommandDef> {
    let bytes = match file.read() {
        Ok(b) => b,
        Err(e) => {
            problems.push(Finding::error(
                "command-frontmatter",
                file.abs.clone(),
                e.to_string(),
                "make the file readable",
            ));
            return None;
        }
    };
    let fm = match frontmatter::parse(&bytes) {
        Ok(fm) => fm,
        Err(e) => {
            problems.push(Finding::error(
                "command-frontmatter",
                file.abs.clone(),
                format!("command file {e}"),
                "fix the front matter: `---`, `description:`, optional `argument-hint:`, `---`",
            ));
            return None;
        }
    };
    if fm.bom {
        problems.push(Finding::error(
            "command-frontmatter",
            file.abs.clone(),
            "command file starts with a UTF-8 BOM".to_string(),
            "strip the BOM",
        ));
    }
    let description = match fm.get("description") {
        Some(d) if !d.trim().is_empty() => d.to_string(),
        _ => {
            problems.push(Finding::error(
                "command-description",
                file.abs.clone(),
                "command front matter has no `description` (the composer picker and /help show it)"
                    .to_string(),
                "add a one-line `description:`",
            ));
            return None;
        }
    };
    Some(CommandDef {
        asset: asset.clone(),
        file: file.clone(),
        argument_hint: fm.get("argument-hint").map(str::to_string),
        description,
        frontmatter: fm,
    })
}

fn read_object(
    file: &ContentFile,
    rule: &'static str,
    problems: &mut Vec<Finding>,
) -> Option<Map<String, Value>> {
    let bytes = match file.read() {
        Ok(b) => b,
        Err(e) => {
            problems.push(Finding::error(
                rule,
                file.abs.clone(),
                e.to_string(),
                "make the file readable",
            ));
            return None;
        }
    };
    match serde_json::from_slice::<Value>(&bytes) {
        Ok(Value::Object(m)) => Some(m),
        Ok(_) => {
            problems.push(Finding::error(
                rule,
                file.abs.clone(),
                "not a JSON object".to_string(),
                "write one JSON object per file",
            ));
            None
        }
        Err(e) => {
            problems.push(Finding::error(
                rule,
                file.abs.clone(),
                format!("invalid JSON: {e}"),
                "fix the JSON",
            ));
            None
        }
    }
}

fn expect_id(
    raw: &Map<String, Value>,
    asset: &Asset,
    file: &ContentFile,
    rule: &'static str,
    problems: &mut Vec<Finding>,
) -> Option<String> {
    match raw.get("id").and_then(Value::as_str) {
        Some(id) if id == asset.id => Some(id.to_string()),
        Some(id) => {
            problems.push(Finding::error(
                "id-mismatch",
                file.abs.clone(),
                format!(
                    "file declares id `{id}` but the catalog row is `{}`",
                    asset.id
                ),
                "make the file's `id` equal the catalog id",
            ));
            None
        }
        None => {
            problems.push(Finding::error(
                rule,
                file.abs.clone(),
                "missing string `id`".to_string(),
                "add `id`",
            ));
            None
        }
    }
}

fn load_hook(
    asset: &Asset,
    file: &ContentFile,
    problems: &mut Vec<Finding>,
) -> Result<Option<Hook>> {
    let Some(raw) = read_object(file, "hook-json", problems) else {
        return Ok(None);
    };
    let protocol = &hr::hook_events()?.protocol;
    let allowed = &protocol.plugin_hook_fields.fields;
    let mut ok = true;
    for key in raw.keys() {
        if allowed.iter().any(|f| f == key) {
            continue;
        }
        ok = false;
        let windows = hr::PLUGIN_HOOK_REJECTED_FIELDS.contains(&key.as_str());
        let known_rejected = protocol
            .plugin_hook_fields
            .rejected
            .iter()
            .any(|r| r == key);
        let why = if windows {
            "the native hooks family rejects the Windows twin (`unsupported-field`); `omm hook <name>` is shell-neutral, so `command` alone is the R16 answer"
        } else if known_rejected {
            "a foreign hook field the native family rejects (`unsupported-field`)"
        } else {
            "not in the CLOSED native hook field set (`unsupported-field`)"
        };
        problems.push(Finding::error(
            "hook-field",
            file.abs.clone(),
            format!(
                "hook field `{key}`: {why}; accepted fields: {}",
                allowed.join(" ")
            ),
            format!("remove `{key}`"),
        ));
    }
    let Some(id) = expect_id(&raw, asset, file, "hook-json", problems) else {
        return Ok(None);
    };
    let event = match raw.get("event").and_then(Value::as_str) {
        Some(e) => e.to_string(),
        None => {
            problems.push(Finding::error(
                "hook-event",
                file.abs.clone(),
                "missing string `event`".to_string(),
                "add a PascalCase `event`",
            ));
            return Ok(None);
        }
    };
    let events = &hr::hook_events()?.items;
    if !events.iter().any(|e| e.name == event) {
        problems.push(Finding::error(
            "hook-event",
            file.abs.clone(),
            format!("event `{event}` is not one of the {} PascalCase hook events (`unsupported-hook-event`)", events.len()),
            format!("use one of: {}", events.iter().map(|e| e.name.as_str()).collect::<Vec<_>>().join(" ")),
        ));
        ok = false;
    }
    if let Some(cat_event) = &asset.event {
        if cat_event != &event {
            problems.push(Finding::error(
                "hook-event",
                file.abs.clone(),
                format!("catalog says event `{cat_event}`, file says `{event}`"),
                "make them agree",
            ));
            ok = false;
        }
    }
    let command: Vec<String> = match raw.get("command") {
        Some(Value::Array(items))
            if !items.is_empty()
                && items
                    .iter()
                    .all(|i| i.as_str().map(|s| !s.is_empty()).unwrap_or(false)) =>
        {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        }
        _ => {
            problems.push(Finding::error(
                "hook-command",
                file.abs.clone(),
                "`command` must be a non-empty argv ARRAY of non-empty strings (structured argv, no shell)".to_string(),
                "write `\"command\": [\"omm\", \"hook\", \"<name>\"]`",
            ));
            return Ok(None);
        }
    };
    let timeout_ms = match raw.get("timeoutMs") {
        None => None,
        Some(Value::Number(n)) if n.as_i64().is_some() => n.as_i64(),
        Some(_) => {
            problems.push(Finding::error(
                "hook-field",
                file.abs.clone(),
                "`timeoutMs` must be an integer".to_string(),
                "write an integer number of milliseconds",
            ));
            ok = false;
            None
        }
    };
    let status_message = match raw.get("statusMessage") {
        None => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            problems.push(Finding::error(
                "hook-field",
                file.abs.clone(),
                "`statusMessage` must be a string".to_string(),
                "quote it",
            ));
            ok = false;
            None
        }
    };
    let is_async = match raw.get("async") {
        None => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            problems.push(Finding::error(
                "hook-field",
                file.abs.clone(),
                "`async` must be a JSON boolean".to_string(),
                "write true or false",
            ));
            ok = false;
            false
        }
    };
    let compatibility_name = match raw.get("compatibilityName") {
        None => None,
        Some(Value::String(s)) => Some(s.clone()),
        Some(_) => {
            problems.push(Finding::error(
                "hook-field",
                file.abs.clone(),
                "`compatibilityName` must be a string".to_string(),
                "quote it",
            ));
            ok = false;
            None
        }
    };
    let output_capabilities: Vec<String> = match raw.get("outputCapabilities") {
        None => Vec::new(),
        Some(Value::Array(items)) if items.iter().all(|i| i.is_string()) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(_) => {
            problems.push(Finding::error(
                "hook-field",
                file.abs.clone(),
                "`outputCapabilities` must be an array of strings".to_string(),
                "write [\"skills.v1\"] or omit it",
            ));
            ok = false;
            Vec::new()
        }
    };
    if !ok {
        return Ok(None);
    }
    Ok(Some(Hook {
        asset: asset.clone(),
        file: file.clone(),
        raw,
        id,
        event,
        command,
        timeout_ms,
        status_message,
        is_async,
        compatibility_name,
        output_capabilities,
    }))
}

fn load_reminder(
    asset: &Asset,
    file: &ContentFile,
    index: &BTreeMap<&str, &ContentFile>,
    problems: &mut Vec<Finding>,
) -> Option<Reminder> {
    let raw = read_object(file, "reminder-shape", problems)?;
    let id = expect_id(&raw, asset, file, "reminder-shape", problems)?;
    let path = match raw.get("path").and_then(Value::as_str) {
        Some(p) => p.to_string(),
        None => {
            problems.push(Finding::error(
                "reminder-shape",
                file.abs.clone(),
                "missing string `path` (the duty file)".to_string(),
                "add `path`",
            ));
            return None;
        }
    };
    if !raw.get("decision").map(Value::is_object).unwrap_or(false) {
        problems.push(Finding::error(
            "reminder-shape",
            file.abs.clone(),
            "missing `decision` object (all nine members are required by the host — plugin-contract.md §1.10.1)".to_string(),
            "copy the decision block from a validated reminder",
        ));
        return None;
    }
    let duty_rel = asset.duty.clone().unwrap_or_default();
    if path != duty_rel {
        problems.push(Finding::error(
            "reminder-path",
            file.abs.clone(),
            format!("declaration `path` is `{path}` but the catalog `duty` is `{duty_rel}`"),
            "make them agree",
        ));
        return None;
    }
    let Some(duty) = index.get(duty_rel.as_str()).copied() else {
        problems.push(Finding::error(
            "reminder-path",
            file.abs.clone(),
            format!("duty file `{duty_rel}` does not exist"),
            "create the duty file",
        ));
        return None;
    };
    let enabled_default = match raw.get("enabledDefault") {
        None => asset.enabled_default(),
        Some(Value::Bool(b)) => *b,
        Some(_) => {
            problems.push(Finding::error(
                "reminder-shape",
                file.abs.clone(),
                "`enabledDefault` must be a boolean".to_string(),
                "write true or false",
            ));
            return None;
        }
    };
    Some(Reminder {
        asset: asset.clone(),
        file: file.clone(),
        duty: duty.clone(),
        raw,
        id,
        path,
        enabled_default,
    })
}

fn load_mcp(asset: &Asset, file: &ContentFile, problems: &mut Vec<Finding>) -> Option<McpServer> {
    let raw = read_object(file, "mcp-shape", problems)?;
    let id = expect_id(&raw, asset, file, "mcp-shape", problems)?;
    let mut ok = true;
    for key in raw.keys() {
        if MCP_DROPPED_FIELDS.contains(&key.as_str()) {
            problems.push(Finding::error(
                "mcp-dropped-field",
                file.abs.clone(),
                format!("`{key}` validates and is then DROPPED at launch (the mcpServers family is OPEN); the server would start without it"),
                format!("remove `{key}` and pass it through the server's own argv"),
            ));
            ok = false;
        } else if !MCP_KNOWN_FIELDS.contains(&key.as_str()) {
            problems.push(Finding::error(
                "mcp-shape",
                file.abs.clone(),
                format!(
                    "unknown mcpServers field `{key}` (silently accepted by the host, meaningless)"
                ),
                format!("remove `{key}`"),
            ));
            ok = false;
        }
    }
    let transport = raw
        .get("transport")
        .and_then(Value::as_str)
        .unwrap_or("stdio")
        .to_string();
    if !MCP_TRANSPORTS.contains(&transport.as_str()) {
        problems.push(Finding::error(
            "mcp-shape",
            file.abs.clone(),
            format!("transport `{transport}` is unsupported"),
            "use stdio or http",
        ));
        ok = false;
    }
    let has_command = matches!(raw.get("command"), Some(Value::Array(a)) if !a.is_empty());
    let has_url = raw
        .get("url")
        .and_then(Value::as_str)
        .map(|u| !u.is_empty())
        .unwrap_or(false);
    if (transport == "stdio" && !has_command) || (transport == "http" && !has_url) {
        problems.push(Finding::error(
            "mcp-shape",
            file.abs.clone(),
            "stdio needs a non-empty `command` array; http needs a non-empty `url`".to_string(),
            "declare the transport's required field",
        ));
        ok = false;
    }
    if !ok {
        return None;
    }
    Some(McpServer {
        asset: asset.clone(),
        file: file.clone(),
        raw,
        id,
        transport,
    })
}

fn load_profile(asset: &Asset, file: &ContentFile, problems: &mut Vec<Finding>) -> Option<Profile> {
    let settings = read_object(file, "profile-json", problems)?;
    Some(Profile {
        asset: asset.clone(),
        file: file.clone(),
        settings,
    })
}

/// A `.tmTheme` is an XML plist (`research/musecode/tui-slash-theme.md`;
/// `content/VALIDATION.md` §4): the extension is case-insensitive for the host
/// but the file must be a plist.
fn theme_ok(file: &ContentFile, problems: &mut Vec<Finding>) -> bool {
    let ext_ok = file
        .rel
        .rsplit_once('.')
        .map(|(_, e)| e.eq_ignore_ascii_case("tmTheme"))
        .unwrap_or(false);
    let head = std::fs::File::open(&file.abs)
        .and_then(|mut f| {
            use std::io::Read;
            let mut buf = vec![0u8; 512];
            let n = f.read(&mut buf)?;
            buf.truncate(n);
            Ok(buf)
        })
        .unwrap_or_default();
    let text = String::from_utf8_lossy(&head);
    let plist = text.contains("<plist");
    if ext_ok && plist {
        return true;
    }
    problems.push(Finding::error(
        "theme-shape",
        file.abs.clone(),
        format!(
            "theme must be a `.tmTheme` XML plist (extension ok: {ext_ok}, `<plist` found: {plist})"
        ),
        "export the theme as a tmTheme plist",
    ));
    false
}

impl Finding {
    /// An error-severity finding.
    pub fn error(
        rule: &str,
        path: PathBuf,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Finding {
        Finding {
            rule: rule.to_string(),
            severity: Severity::Error,
            path,
            message: message.into(),
            fix: fix.into(),
        }
    }
    /// A warning-severity finding.
    pub fn warning(
        rule: &str,
        path: PathBuf,
        message: impl Into<String>,
        fix: impl Into<String>,
    ) -> Finding {
        Finding {
            rule: rule.to_string(),
            severity: Severity::Warning,
            path,
            message: message.into(),
            fix: fix.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_the_shipped_content_cleanly() {
        let repo = crate::Repo::from_cargo_manifest_dir().unwrap();
        let catalog = Catalog::load(&repo.catalog_path()).unwrap();
        let content = Content::load(&catalog, &repo.content_dir()).unwrap();
        assert!(content.problems.is_empty(), "{:#?}", content.problems);
        assert_eq!(
            content.skills.len(),
            catalog.shipped_of(AssetKind::Skill).count()
        );
        assert_eq!(
            content.commands.len(),
            catalog.shipped_of(AssetKind::Command).count()
        );
        assert_eq!(
            content.hooks.len(),
            catalog.shipped_of(AssetKind::Hook).count()
        );
        assert_eq!(
            content.reminders.len(),
            catalog.shipped_of(AssetKind::Reminder).count()
        );
        assert!(content
            .hooks
            .iter()
            .all(|h| h.command.first().map(String::as_str) == Some("omm")));
        assert!(content
            .skills
            .iter()
            .all(|s| s.files[0].rel == s.asset.path));
        assert!(
            content.skills.iter().any(|s| s.files.len() > 1),
            "references/ are claimed by their skill"
        );
        assert!(!content.docs.is_empty());
        assert!(content.require_clean().is_ok());
        assert!(content.skill(&content.skills[0].asset.id).is_some());
        assert!(content.asset("nope").is_none());
    }
}
