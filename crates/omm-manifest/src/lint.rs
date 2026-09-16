//! `omm lint` (ARCHITECTURE.md §5.3; R11, R12, R13, R19) — every rule reads
//! its constants from `omm_host::host_reality` and its lists from the
//! `docs/host-data/*.json` files compiled into it (R8: nothing hand-copied).
//!
//! Findings carry `{rule, severity, path, message, fix}`. The structural
//! rules the loader can see live in [`crate::content`] (`Content::problems`);
//! the cross-asset, budget, package and host rules live here. Rule ids are
//! stable kebab-case strings so fixtures and users can key on them.
//!
//! Order of work in [`run`]: catalog → content → identity → skill/hook/
//! reminder/profile/rules rules → foreign vocabulary → budget → the rendered
//! native package (size, entries, depth, inventory) → the committed catalogs →
//! the host checkpoints (R13: `skills validate` per skill, then `plugins
//! validate` on the four predicates; the post-install `skills list` is the
//! installer's checkpoint).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use omm_host::probe;
use omm_host::Invoker;
use serde::Serialize;
use serde_json::Value;

use crate::budget::{self, BudgetReport};
use crate::catalog::{AssetKind, Catalog};
use crate::content::Content;
use crate::error::Result;
use crate::generate::{self, marketplace, native, BuildOptions, DigestSource, Package};
use crate::hr;
use crate::Repo;

/// The SKILL.md body ceiling (bytes after the front matter) — the §5.3 lint
/// constant; overflow goes to `references/*.md` beside the skill.
pub const SKILL_BODY_MAX_BYTES: usize = 8_192;
/// The clause every description must carry, joined to its first sentence
/// with `;` so it survives `first_sentence` (§5.3; context-slimming.md §3).
pub const NEGATIVE_TRIGGER: &str = "do not use";
/// The id prefix of every omm asset (R19).
pub const ID_PREFIX: &str = "omm-";
/// The markers `rules/AGENTS.md.tmpl` carries around the omm-managed region
/// (`content/VALIDATION.md` §7); `omm install` merges inside them.
pub const RULES_MANAGED_START: &str = "<!-- omm:managed-start -->";
/// See [`RULES_MANAGED_START`].
pub const RULES_MANAGED_END: &str = "<!-- omm:managed-end -->";
/// Foreign tool names a skill body must not instruct the model to call
/// (§5.3; `content/translation/muse.md` maps each to Muse's own tool).
/// Matched only as a backticked name or as `the <Name> tool` — bare prose
/// verbs (`Read everything`, `Write it`) are not matched.
pub const FOREIGN_TOOL_NAMES: [&str; 8] = [
    "Task",
    "TodoWrite",
    "Read",
    "Write",
    "Edit",
    "Glob",
    "Grep",
    "AskUserQuestion",
];
/// Asset kinds whose whole purpose is to name foreign tool vocabulary
/// (§5.3): the translation block and the rules template map those names to
/// Muse's. Any other asset opts in with `foreign_vocab: true` in the
/// catalog ([`crate::catalog::Asset::foreign_vocab`]) — no content path is
/// spelled in Rust (R8).
pub const FOREIGN_VOCAB_KINDS: [AssetKind; 2] = [AssetKind::Translation, AssetKind::Rules];
/// The `hooks` argv every shipped hook must start with (R16): `omm hook <name>`.
pub const HOOK_ARGV_PREFIX: [&str; 2] = ["omm", "hook"];
/// The only `outputCapabilities` value the host accepts, and only on
/// foreground `UserPromptSubmit` / `PostToolUse` (plugin-contract.md §1.8).
pub const HOOK_OUTPUT_CAPABILITY_SKILLS: &str = "skills.v1";

/// Error or warning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

/// One lint result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub rule: String,
    pub severity: Severity,
    pub path: PathBuf,
    pub message: String,
    pub fix: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} {} {}: {} — fix: {}",
            self.severity,
            self.rule,
            self.path.display(),
            self.message,
            self.fix
        )
    }
}

/// The result of one lint run.
#[derive(Clone, Debug, Default)]
pub struct LintReport {
    pub findings: Vec<Finding>,
    /// The budget estimate, when content loaded.
    pub budget: Option<BudgetReport>,
    /// Whether the host checkpoints ran.
    pub host_checked: bool,
    /// Files in the rendered native package, when it rendered.
    pub package_files: Option<usize>,
}

impl LintReport {
    /// Error-severity findings.
    pub fn errors(&self) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .collect()
    }
    /// Warning-severity findings.
    pub fn warnings(&self) -> Vec<&Finding> {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Warning)
            .collect()
    }
    /// No errors (warnings allowed).
    pub fn is_clean(&self) -> bool {
        self.errors().is_empty()
    }
    /// Rule ids present, deduplicated.
    pub fn rules(&self) -> BTreeSet<&str> {
        self.findings.iter().map(|f| f.rule.as_str()).collect()
    }
    /// Findings of one rule.
    pub fn of(&self, rule: &str) -> Vec<&Finding> {
        self.findings.iter().filter(|f| f.rule == rule).collect()
    }
}

/// What [`run`] may do beyond the pure rules.
#[derive(Clone, Debug, Default)]
pub struct LintOptions<'a> {
    /// Run the R13 host checkpoints with this invoker.
    pub host: Option<&'a Invoker>,
    /// Also lint the committed marketplace catalogs at the repo root
    /// (existence, shapes, and — with a host — the digest).
    pub check_marketplace: bool,
}

/// Lint the repository's content.
pub fn run(repo: &Repo, opts: &LintOptions<'_>) -> Result<LintReport> {
    let mut report = LintReport::default();
    let catalog = match Catalog::load(&repo.catalog_path()) {
        Ok(c) => c,
        Err(e) => {
            report.findings.push(Finding::error(
                "catalog-load",
                repo.catalog_path(),
                e.to_string(),
                "fix content/catalog.json",
            ));
            return Ok(report);
        }
    };
    report.findings.extend(lint_catalog(&catalog)?);
    let content = Content::load(&catalog, &repo.content_dir())?;
    report.findings.extend(content.problems.iter().cloned());
    report.findings.extend(lint_content(&catalog, &content)?);
    let estimate = budget::estimate(&content);
    report
        .findings
        .extend(lint_budget(&catalog, &content, &estimate));
    report.budget = Some(estimate);
    if !content.problems.is_empty() {
        return Ok(report);
    }
    let v = match generate::resolve_version(
        repo,
        &BuildOptions {
            version: None,
            description: None,
            digest: DigestSource::Fixed(String::new()),
        },
    ) {
        Ok(v) => v,
        Err(e) => {
            report.findings.push(Finding::error(
                "version-load",
                repo.omm_cli_manifest(),
                e.to_string(),
                "give crates/omm/Cargo.toml a version and a description",
            ));
            return Ok(report);
        }
    };
    let pkg = native::render(&catalog, &content, &v.version, &v.description)?;
    report.package_files = Some(pkg.len());
    report.findings.extend(lint_package(&pkg, &content));
    let mut digest: Option<String> = None;
    if let Some(inv) = opts.host {
        report.host_checked = true;
        let (findings, d) = host_checkpoints(inv, &content, &pkg)?;
        report.findings.extend(findings);
        digest = d;
    }
    if opts.check_marketplace {
        report
            .findings
            .extend(lint_marketplace(repo, &catalog, digest.as_deref())?);
    }
    Ok(report)
}

// ---------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------

/// `^[a-z0-9][a-z0-9._-]{0,79}$` (`hr::ID_GRAMMAR`, reserved-ids.json).
pub fn is_valid_id(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || bytes.len() > hr::ID_MAX_BYTES {
        return false;
    }
    let first_ok = bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit();
    first_ok
        && bytes[1..].iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
}

/// The catalog's own identity: the plugin id is the one reserved-ids.json
/// names for omm and is not itself reserved; the marketplace name is not reserved.
pub fn lint_catalog(catalog: &Catalog) -> Result<Vec<Finding>> {
    let reserved = hr::reserved_ids()?;
    let mut out = Vec::new();
    if catalog.plugin_id != reserved.omm_identity.plugin_id {
        out.push(Finding::error(
            "catalog-plugin-id",
            catalog.path.clone(),
            format!(
                "plugin_id `{}` is not omm's identity `{}` (reserved-ids.json omm_identity)",
                catalog.plugin_id, reserved.omm_identity.plugin_id
            ),
            format!("set plugin_id to `{}`", reserved.omm_identity.plugin_id),
        ));
    }
    if reserved.is_reserved_plugin_id(&catalog.plugin_id) || !is_valid_id(&catalog.plugin_id) {
        out.push(Finding::error(
            "catalog-plugin-id",
            catalog.path.clone(),
            format!("plugin_id `{}` is reserved or malformed", catalog.plugin_id),
            "use omm's identity",
        ));
    }
    if reserved.is_reserved_marketplace_name(crate::MARKETPLACE_NAME) {
        out.push(Finding::error(
            "marketplace-name",
            catalog.path.clone(),
            format!(
                "marketplace name `{}` is reserved by the host",
                crate::MARKETPLACE_NAME
            ),
            "pick another marketplace name",
        ));
    }
    Ok(out)
}

/// Every cross-asset rule over loaded content.
pub fn lint_content(catalog: &Catalog, content: &Content) -> Result<Vec<Finding>> {
    let mut out = Vec::new();
    out.extend(lint_ids(catalog, content)?);
    out.extend(lint_skills(content));
    out.extend(lint_hooks(content)?);
    out.extend(lint_reminders(content)?);
    out.extend(lint_profiles(content)?);
    out.extend(lint_rules(content));
    out.extend(lint_foreign_vocabulary(catalog, content)?);
    Ok(out)
}

/// The identity rules (R19; §5.3): grammar, `omm-` prefix, the 9 reserved
/// ids, the 15 bundled skill ids, the 53 slash names (for skills AND commands
/// — plugin skills surface as slash shortcuts, `content/VALIDATION.md`
/// "Note"), Windows reserved stems, command ≠ skill, reminder ≠ skill/command,
/// `len(pid) + len(server-id) ≤ 18`. An MCP server id is namespaced by the
/// host (`plugin:<pid>:mcp_server:<sid>`, `mcp__plugin_<pid>_<sid>`), so the
/// prefix and bundled-skill rules do not apply to it — only the grammar, the
/// reserved and Windows stems, and the 18-char budget.
pub fn lint_ids(catalog: &Catalog, content: &Content) -> Result<Vec<Finding>> {
    let reserved = hr::reserved_ids()?;
    let bundled = &reserved.bundled_skill_ids;
    let slash = hr::slash_commands()?.collision_names();
    let mut out = Vec::new();
    let path = catalog.path.clone();
    for a in &catalog.assets {
        if !is_valid_id(&a.id) {
            out.push(Finding::error(
                "id-grammar",
                path.clone(),
                format!(
                    "id `{}` does not match {} (max {} bytes)",
                    a.id,
                    hr::ID_GRAMMAR,
                    hr::ID_MAX_BYTES
                ),
                "lowercase ASCII letters, digits, `.`, `_`, `-`; start with a letter or digit",
            ));
        }
        // An MCP server id lives in its own namespaces — the stable id
        // `plugin:<pid>:mcp_server:<sid>` and the wire namespace
        // `mcp__plugin_<pid>_<sid>` — never among skill, command, slash or
        // plugin ids; the `omm-` prefix would only spend the 18-char budget
        // (`mcp-id-length` below is its identity rule). A server id therefore
        // collides with neither a slash name nor a bundled skill of the same
        // spelling. (The host sanitizes `-` to `_` on the wire; the budget
        // counts source characters, so sanitizing never breaks it.)
        let namespaced = a.kind == AssetKind::McpServer;
        if !namespaced && !a.id.starts_with(ID_PREFIX) {
            out.push(Finding::error(
                "id-prefix",
                path.clone(),
                format!("id `{}` lacks the `{ID_PREFIX}` prefix (R19)", a.id),
                format!("rename it `{ID_PREFIX}{}`", a.id),
            ));
        }
        if reserved.is_reserved_plugin_id(&a.id) {
            out.push(Finding::error(
                "id-reserved",
                path.clone(),
                format!(
                    "id `{}` is a reserved plugin id (installs 'successfully' and does nothing)",
                    a.id
                ),
                "rename it",
            ));
        }
        if !namespaced && bundled.iter().any(|b| b == &a.id) {
            out.push(Finding::error(
                "id-bundled-skill",
                path.clone(),
                format!("id `{}` collides with a bundled skill id", a.id),
                "rename it",
            ));
        }
        if matches!(a.kind, AssetKind::Skill | AssetKind::Command)
            && slash.iter().any(|s| s == &a.id)
        {
            out.push(Finding::error(
                "id-slash-collision",
                path.clone(),
                format!(
                    "id `{}` collides with a built-in slash command name (`/{}`)",
                    a.id, a.id
                ),
                "rename it",
            ));
        }
        let stem = a.id.split('.').next().unwrap_or("").to_ascii_lowercase();
        if reserved
            .windows_reserved_stems
            .ids
            .iter()
            .any(|s| s == &stem)
        {
            out.push(Finding::error(
                "id-windows-stem",
                path.clone(),
                format!("id `{}` has a Windows reserved basename `{stem}`", a.id),
                "rename it",
            ));
        }
        if a.kind == AssetKind::McpServer
            && catalog.plugin_id.len() + a.id.len() > hr::MCP_ID_LENGTH_BUDGET
        {
            out.push(Finding::error(
                "mcp-id-length",
                path.clone(),
                format!(
                    "len(`{}`) + len(`{}`) = {} > {} — the wire namespace `mcp__plugin_{}_{}` is rewritten and `<ns>__<tool>` stops dispatching (R19, mcp-tools-call.md §3)",
                    catalog.plugin_id,
                    a.id,
                    catalog.plugin_id.len() + a.id.len(),
                    hr::MCP_ID_LENGTH_BUDGET,
                    catalog.plugin_id,
                    a.id
                ),
                "shorten the server id",
            ));
        }
    }
    let skill_ids: BTreeSet<&str> = content.skills.iter().map(|s| s.asset.id.as_str()).collect();
    let command_ids: BTreeSet<&str> = content
        .commands
        .iter()
        .map(|c| c.asset.id.as_str())
        .collect();
    for c in &content.commands {
        if skill_ids.contains(c.asset.id.as_str()) {
            out.push(Finding::error(
                "id-command-equals-skill",
                c.file.abs.clone(),
                format!("command id `{}` equals a skill id; plugin skills surface as `/{}` too and the host rejects the duplicate", c.asset.id, c.asset.id),
                "rename the command",
            ));
        }
    }
    for r in &content.reminders {
        if skill_ids.contains(r.id.as_str()) || command_ids.contains(r.id.as_str()) {
            out.push(Finding::error(
                "id-reminder-collision",
                r.file.abs.clone(),
                format!(
                    "reminder id `{}` duplicates a skill or command id (`duplicate-capability-id`)",
                    r.id
                ),
                "rename the reminder (e.g. `<id>-nudge`)",
            ));
        }
    }
    Ok(out)
}

/// SKILL.md rules: `name` == directory id, description ≤ 240 chars on one
/// line with a `Do not use` clause that survives `first_sentence`, body ≤ 8 KiB.
pub fn lint_skills(content: &Content) -> Vec<Finding> {
    let mut out = Vec::new();
    for s in &content.skills {
        let p = s.skill_md.abs.clone();
        match s.frontmatter.get("name") {
            Some(name) if name == s.asset.id => {}
            Some(name) => out.push(Finding::error(
                "skill-name",
                p.clone(),
                format!("front-matter `name: {name}` differs from the directory id `{}` (the skills validator accepts the mismatch; only the generator can catch it — R13)", s.asset.id),
                format!("set `name: {}`", s.asset.id),
            )),
            None => out.push(Finding::error(
                "skill-name",
                p.clone(),
                "front matter has no `name`".to_string(),
                format!("add `name: {}`", s.asset.id),
            )),
        }
        let desc = &s.description;
        let chars = desc.chars().count();
        if chars > hr::SKILL_DESCRIPTION_MAX_CHARS {
            out.push(Finding::error(
                "skill-description-length",
                p.clone(),
                format!(
                    "description is {chars} chars, over {}",
                    hr::SKILL_DESCRIPTION_MAX_CHARS
                ),
                "shorten it; move detail into the body",
            ));
        }
        if desc.contains('\n') || desc.contains('\r') {
            out.push(Finding::error(
                "skill-description-multiline",
                p.clone(),
                "description spans lines".to_string(),
                "write it on one line",
            ));
        }
        let lower = desc.to_ascii_lowercase();
        if !lower.contains(NEGATIVE_TRIGGER) {
            out.push(Finding::error(
                "skill-negative-trigger",
                p.clone(),
                "description has no `Do not use …` clause".to_string(),
                "append `; Do not use when …` to the first sentence",
            ));
        } else if !budget::first_sentence(desc)
            .to_ascii_lowercase()
            .contains(NEGATIVE_TRIGGER)
        {
            out.push(Finding::error(
                "skill-negative-trigger",
                p.clone(),
                "the `Do not use …` clause sits after the first sentence and vanishes under `run.context_slimming.skill_catalog_descriptions: \"first_sentence\"`".to_string(),
                "join the clause to the first sentence with `;` instead of `. `",
            ));
        }
        let body = s.body_bytes();
        if body > SKILL_BODY_MAX_BYTES {
            out.push(Finding::error(
                "skill-body-size",
                p.clone(),
                format!("body is {body} B, over {SKILL_BODY_MAX_BYTES}"),
                "move detail into references/*.md beside the skill",
            ));
        }
    }
    out
}

/// Hook rules beyond the closed field set: `omm hook <name>` argv (R16), a
/// timeout ≥ 1 s (hook-events.json `protocol.timeout`), `outputCapabilities`
/// only where the host accepts it, no built-in tool name as `compatibilityName`.
pub fn lint_hooks(content: &Content) -> Result<Vec<Finding>> {
    let reserved = hr::reserved_ids()?;
    let mut out = Vec::new();
    for h in &content.hooks {
        let p = h.file.abs.clone();
        let prefix_ok = h.command.len() >= HOOK_ARGV_PREFIX.len()
            && h.command
                .iter()
                .zip(HOOK_ARGV_PREFIX.iter())
                .all(|(a, b)| a == b);
        if !prefix_ok || h.command.len() != HOOK_ARGV_PREFIX.len() + 1 {
            out.push(Finding::error(
                "hook-argv",
                p.clone(),
                format!("command {:?} is not `omm hook <name>` — every hook dispatches into the one static binary, shell-neutral (R16)", h.command),
                "write `\"command\": [\"omm\", \"hook\", \"<name>\"]`",
            ));
        }
        match h.timeout_ms {
            Some(ms) if ms >= (hr::HOOK_TIMEOUT_MIN_SECS * 1000) as i64 => {}
            Some(ms) => out.push(Finding::error(
                "hook-timeout",
                p.clone(),
                format!(
                    "timeoutMs {ms} is below the {}-second minimum the host clamps to",
                    hr::HOOK_TIMEOUT_MIN_SECS
                ),
                "set timeoutMs to at least 1000",
            )),
            None => out.push(Finding::error(
                "hook-timeout",
                p.clone(),
                "no timeoutMs — an omitted timeout lets the hook block forever".to_string(),
                "set timeoutMs (2000–3000 for an in-binary handler)",
            )),
        }
        if !h.output_capabilities.is_empty() {
            let allowed_event = h.event == "UserPromptSubmit" || h.event == "PostToolUse";
            let exact = h.output_capabilities == [HOOK_OUTPUT_CAPABILITY_SKILLS.to_string()];
            if !exact || !allowed_event || h.is_async {
                out.push(Finding::error(
                    "hook-output-capabilities",
                    p.clone(),
                    format!("outputCapabilities {:?} on {} (async: {}) — the host accepts exactly [\"{HOOK_OUTPUT_CAPABILITY_SKILLS}\"] on a foreground UserPromptSubmit or PostToolUse hook", h.output_capabilities, h.event, h.is_async),
                    "drop outputCapabilities or move the hook to an accepted event",
                ));
            }
        }
        if let Some(name) = &h.compatibility_name {
            if reserved
                .builtin_tool_names
                .ids
                .iter()
                .any(|t| t.eq_ignore_ascii_case(name))
            {
                out.push(Finding::error(
                    "hook-compatibility-name",
                    p.clone(),
                    format!(
                        "compatibilityName `{name}` collides with a built-in tool matcher name"
                    ),
                    "pick another compatibilityName",
                ));
            }
        }
    }
    Ok(out)
}

/// Reminder rules: the envelope must not use a host-reserved tag (it can
/// never be activated headlessly — reserved-ids.json
/// `reserved_reminder_envelope_tags`) and must carry exactly one `{text}` slot.
pub fn lint_reminders(content: &Content) -> Result<Vec<Finding>> {
    let reserved = hr::reserved_ids()?;
    let tags: Vec<String> = reserved
        .reserved_reminder_envelope_tags
        .tags
        .iter()
        .filter_map(|t| t.split_whitespace().next().map(str::to_string))
        .collect();
    let mut out = Vec::new();
    for r in &content.reminders {
        let p = r.file.abs.clone();
        match r.envelope_template() {
            Some(template) => {
                for tag in &tags {
                    if template.contains(tag.as_str()) {
                        out.push(Finding::error(
                            "reminder-envelope",
                            p.clone(),
                            format!("envelope template uses the host-reserved tag `{tag}`; the capability would be `blocked` (`reminder_envelope_elevated_approval`) and never spawn headlessly"),
                            "use an omm-owned tag such as <omm-reminder>",
                        ));
                    }
                }
                if template.matches("{text}").count() != 1 {
                    out.push(Finding::error(
                        "reminder-envelope",
                        p.clone(),
                        "envelope template must contain exactly one literal {text} slot"
                            .to_string(),
                        "write `<tag>\\n{text}\\n</tag>`",
                    ));
                }
            }
            None => out.push(Finding::error(
                "reminder-envelope",
                p.clone(),
                "decision.envelope.template is missing".to_string(),
                "add the envelope (version 1, template with one {text} slot)",
            )),
        }
    }
    Ok(out)
}

/// A profile is a settings slice: every top-level key must be one of the 29
/// typed keys (R9; settings-keys.json) or Muse destroys it on its next rewrite.
pub fn lint_profiles(content: &Content) -> Result<Vec<Finding>> {
    let keys = hr::settings_keys()?;
    let mut out = Vec::new();
    for p in &content.profiles {
        for key in p.settings.keys() {
            if !keys.is_known(key) {
                let hint = keys
                    .canonical_spelling(key)
                    .map(|c| format!(" (did you mean `{c}`?)"))
                    .unwrap_or_default();
                out.push(Finding::error(
                    "profile-key",
                    p.file.abs.clone(),
                    format!(
                        "`{key}` is not a typed settings.json key{hint}; the host would destroy it"
                    ),
                    "use only keys from docs/host-data/settings-keys.json",
                ));
            }
        }
    }
    Ok(out)
}

/// The rules template must carry the omm-managed markers.
pub fn lint_rules(content: &Content) -> Vec<Finding> {
    let mut out = Vec::new();
    for r in &content.rules {
        let text = String::from_utf8_lossy(&r.file.read().unwrap_or_default()).into_owned();
        if !text.contains(RULES_MANAGED_START) || !text.contains(RULES_MANAGED_END) {
            out.push(Finding::error(
                "rules-markers",
                r.file.abs.clone(),
                format!("rules template lacks `{RULES_MANAGED_START}` / `{RULES_MANAGED_END}`"),
                "wrap the omm-managed lines in the markers",
            ));
        }
    }
    out
}

/// The content-relative prefixes under which foreign tool vocabulary is
/// allowed: every shipped asset flagged `foreign_vocab` in the catalog (a
/// skill's whole directory) and every asset of a [`FOREIGN_VOCAB_KINDS`]
/// kind.
pub fn foreign_vocab_allowlist(catalog: &Catalog) -> Vec<String> {
    let mut out: Vec<String> = catalog
        .shipped()
        .filter(|a| a.foreign_vocab || FOREIGN_VOCAB_KINDS.contains(&a.kind))
        .map(|a| {
            if a.kind == AssetKind::Skill {
                a.dir().to_string()
            } else {
                a.path.clone()
            }
        })
        .filter(|p| !p.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// §5.3 foreign vocabulary: a backticked foreign tool name or `the <Name>
/// tool` in a skill, command or rules body (the catalog's
/// [`foreign_vocab_allowlist`] excepted), and the foreign product names of
/// `crates/omm-host/tests/repo_lint.rs`
/// (plus their bare first words as whole words) anywhere under `content/`,
/// with the format/path literals (`.claude-plugin`, `CLAUDE.md`, `.claude/`,
/// `--from claude|codex`) exempt by construction — they are never capitalised
/// whole words.
pub fn lint_foreign_vocabulary(catalog: &Catalog, content: &Content) -> Result<Vec<Finding>> {
    let mut out = Vec::new();
    let root = &content.root;
    let allowlist = foreign_vocab_allowlist(catalog);
    let bodies: Vec<(&Path, String)> = content
        .files
        .iter()
        .filter(|f| {
            let ext = f.rel.rsplit('.').next().unwrap_or("");
            matches!(ext, "md" | "tmpl" | "json" | "tmTheme" | "txt")
        })
        .filter_map(|f| {
            f.read()
                .ok()
                .map(|b| (f.abs.as_path(), String::from_utf8_lossy(&b).into_owned()))
        })
        .collect();
    let names = product_names();
    for (abs, text) in &bodies {
        let rel = abs.strip_prefix(root).map(crate::posix).unwrap_or_default();
        let allowlisted = allowlist
            .iter()
            .any(|a| rel == *a || rel.starts_with(&format!("{a}/")));
        let is_body = rel.starts_with("skills/")
            || rel.starts_with("commands/")
            || rel.starts_with("rules/")
            || rel.starts_with("agents/");
        for (n, line) in text.lines().enumerate() {
            for name in &names {
                if line.contains(name.as_str()) {
                    out.push(Finding::error(
                        "foreign-product-name",
                        abs.to_path_buf(),
                        format!("line {}: names a foreign product ({name:?}); only its format names may appear", n + 1),
                        "reword to the format name (`.claude-plugin` / `Claude schema`)",
                    ));
                }
            }
            for word in product_words() {
                if contains_whole_word(line, word) {
                    out.push(Finding::error(
                        "foreign-product-name",
                        abs.to_path_buf(),
                        format!("line {}: bare product word `{word}`; only path/format literals may carry it", n + 1),
                        "reword, or use the path/format literal",
                    ));
                }
            }
            if is_body && !allowlisted {
                for tool in FOREIGN_TOOL_NAMES {
                    let ticked = format!("`{tool}`");
                    let phrase = format!("the {tool} tool");
                    if line.contains(&ticked) || line.contains(&phrase) {
                        out.push(Finding::error(
                            "foreign-tool-vocabulary",
                            abs.to_path_buf(),
                            format!("line {}: names the foreign tool `{tool}`; Muse has no such tool and `run.toolset` rejects the alias", n + 1),
                            "use the Muse tool from content/translation/muse.md",
                        ));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The two product names, assembled the way `repo_lint.rs` does so this file
/// carries neither.
fn product_names() -> Vec<String> {
    vec![
        ["Claude", "Code"].join(" "),
        ["Codex", "CLI"].join(" "),
        ["Claude", "authored"].join("-"),
    ]
}

/// Their bare first words.
fn product_words() -> [&'static str; 2] {
    ["Claude", "Codex"]
}

/// Case-sensitive whole-word match. Before the word, an ASCII alphanumeric,
/// `-`, `_`, `.` or `/` is not a boundary (`.claude-plugin`, `omm-Claude`,
/// `foo/Claude`); after it, an alphanumeric, `-` or `_` is not, nor a `.` or
/// `/` that continues into a path segment (`Claude.md`, `Claude/x`) — a
/// sentence-ending `Codex.` still matches.
pub fn contains_whole_word(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let mut start = 0;
    while let Some(pos) = line[start..].find(word) {
        let i = start + pos;
        let j = i + word.len();
        let before = i == 0 || {
            let b = bytes[i - 1];
            !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'/'))
        };
        let after = j >= bytes.len() || {
            let b = bytes[j];
            let continues_path = matches!(b, b'.' | b'/')
                && bytes
                    .get(j + 1)
                    .map(u8::is_ascii_alphanumeric)
                    .unwrap_or(false);
            !(b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_') || continues_path)
        };
        if before && after {
            return true;
        }
        start = j;
    }
    false
}

/// R18: the estimate against the budgets, and stale catalog numbers (per
/// asset, and the informational `budget` header block when it carries totals).
pub fn lint_budget(catalog: &Catalog, content: &Content, estimate: &BudgetReport) -> Vec<Finding> {
    let mut out = Vec::new();
    let catalog_path = content.root.join(crate::CATALOG_FILE);
    if let Some(header) = &catalog.budget {
        let checks = [
            ("skills_total_bytes", estimate.total_full),
            (
                "skills_total_first_sentence_bytes",
                estimate.total_first_sentence,
            ),
            ("bundle_limit_bytes", estimate.limit_full),
            (
                "bundle_limit_first_sentence_bytes",
                estimate.limit_first_sentence,
            ),
        ];
        for (key, recomputed) in checks {
            if let Some(v) = header.get(key).and_then(Value::as_u64) {
                if v != recomputed {
                    out.push(Finding::warning(
                        "catalog-budget-stale",
                        catalog_path.clone(),
                        format!("budget.{key} is {v} in the catalog, recomputed {recomputed}"),
                        format!("set budget.{key} to {recomputed}"),
                    ));
                }
            }
        }
    }
    if !estimate.within_full() {
        out.push(Finding::error(
            "budget-full",
            catalog_path.clone(),
            format!(
                "catalog estimate {} B exceeds {} B with the built-ins on (R18); {} B with them disabled",
                estimate.total_full, estimate.limit_full, estimate.limit_builtins_disabled
            ),
            "shorten descriptions or drop a skill; `first_sentence` buys up to 24,924 B",
        ));
    }
    if !estimate.within_first_sentence() {
        out.push(Finding::error(
            "budget-first-sentence",
            catalog_path.clone(),
            format!(
                "first_sentence estimate {} B exceeds {} B",
                estimate.total_first_sentence, estimate.limit_first_sentence
            ),
            "shorten first sentences",
        ));
    }
    for e in estimate.stale() {
        out.push(Finding::warning(
            "catalog-budget-stale",
            catalog_path.clone(),
            format!(
                "`{}` budget_bytes {} / first_sentence {:?} in the catalog, recomputed {} / {}",
                e.id, e.catalog_full, e.catalog_first_sentence, e.cost.full, e.cost.first_sentence
            ),
            format!(
                "set budget_bytes to {} and budget_bytes_first_sentence to {}",
                e.cost.full, e.cost.first_sentence
            ),
        ));
    }
    out
}

/// Package limits (host-reality.md "Budgets": manifest ≤ 131,072 B, ≤ 4,096
/// entries, depth ≤ 16, inventory ≤ 4,094 units, per-class maxima).
pub fn lint_package(pkg: &Package, content: &Content) -> Vec<Finding> {
    let mut out = Vec::new();
    let manifest_rel = native::manifest_path();
    let manifest_path = PathBuf::from(&manifest_rel);
    let manifest_bytes = pkg.get(&manifest_rel).map(|b| b.len() as u64).unwrap_or(0);
    if manifest_bytes > hr::PLUGIN_MANIFEST_MAX_BYTES {
        out.push(Finding::error(
            "package-manifest-size",
            manifest_path.clone(),
            format!(
                "manifest is {manifest_bytes} B, over {}",
                hr::PLUGIN_MANIFEST_MAX_BYTES
            ),
            "ship fewer reminders (each carries ~3 KB of decision block)",
        ));
    }
    if pkg.fs_entries() > hr::PACKAGE_MAX_FS_ENTRIES {
        out.push(Finding::error(
            "package-entries",
            manifest_path.clone(),
            format!(
                "package has {} filesystem entries, over {}",
                pkg.fs_entries(),
                hr::PACKAGE_MAX_FS_ENTRIES
            ),
            "drop reference files",
        ));
    }
    if pkg.max_depth() > hr::PACKAGE_MAX_PATH_DEPTH {
        out.push(Finding::error(
            "package-depth",
            manifest_path.clone(),
            format!(
                "deepest path has {} components, over {}",
                pkg.max_depth(),
                hr::PACKAGE_MAX_PATH_DEPTH
            ),
            "flatten the tree",
        ));
    }
    let units = budget::inventory_units(content);
    if units > hr::INVENTORY_BUDGET_UNITS {
        out.push(Finding::error(
            "package-inventory",
            manifest_path.clone(),
            format!(
                "inventory is {units} units (1/class + 2/skill + 1/other), over {}",
                hr::INVENTORY_BUDGET_UNITS
            ),
            "ship fewer capabilities",
        ));
    }
    let counts = [
        (AssetKind::Skill, content.skills.len()),
        (AssetKind::Command, content.commands.len()),
        (AssetKind::Hook, content.hooks.len()),
        (AssetKind::McpServer, content.mcp_servers.len()),
        (AssetKind::Reminder, content.reminders.len()),
    ];
    for (kind, n) in counts {
        if let Some(max) = budget::per_class_max(kind) {
            if n > max {
                out.push(Finding::error(
                    "package-class-max",
                    manifest_path.clone(),
                    format!(
                        "{n} {} capabilities, over the per-plugin maximum {max}",
                        kind.plural()
                    ),
                    "ship fewer",
                ));
            }
        }
    }
    out
}

/// The committed catalogs at the repo root (marketplace-precedence.md §6.3):
/// the native file present (its absence silently degrades installs to the
/// Codex projection), shapes as §6.1, every `name` == the plugin id, every
/// source relative without `..`, the Claude-schema `name` == `ohmy`, and the
/// native digest equal to `expected_digest` when one is known.
pub fn lint_marketplace(
    repo: &Repo,
    catalog: &Catalog,
    expected_digest: Option<&str>,
) -> Result<Vec<Finding>> {
    let mut out = Vec::new();
    let pid = catalog.plugin_id.as_str();
    let native_path = repo.marketplace_native_path();
    match read_json(&native_path) {
        None => out.push(Finding::error(
            "marketplace-root-missing",
            native_path.clone(),
            "the native `marketplace.json` is missing; Muse would fall through to the Codex-schema file and install the projection as `codex-compatible`".to_string(),
            "run `omm build`",
        )),
        Some(Err(e)) => out.push(Finding::error("marketplace-shape", native_path.clone(), e, "run `omm build`")),
        Some(Ok(v)) => {
            if v["schemaVersion"].as_u64() != Some(hr::MARKETPLACE_NATIVE_SCHEMA_VERSION)
                || v["source"].as_str() != Some(hr::MARKETPLACE_NATIVE_SOURCE)
            {
                out.push(Finding::error("marketplace-shape", native_path.clone(), "native catalog must declare schemaVersion 1 and source \"local\"".to_string(), "run `omm build`"));
            }
            let entries = v["plugins"].as_array().cloned().unwrap_or_default();
            if entries.len() != 1 {
                out.push(Finding::error("marketplace-shape", native_path.clone(), format!("native catalog lists {} entries, expected 1", entries.len()), "run `omm build`"));
            }
            for e in &entries {
                check_entry_name(&mut out, &native_path, e, pid);
                let source = e["install"]["source"].as_str().unwrap_or("");
                if e["install"]["transport"].as_str() != Some(hr::MARKETPLACE_NATIVE_TRANSPORT) || !rel_source_ok(source) {
                    out.push(Finding::error("marketplace-source", native_path.clone(), format!("install {{transport, source}} must be local-path + a relative source without `..`, got {}", e["install"]), "run `omm build`"));
                }
                if source != Repo::native_package_rel(pid) {
                    out.push(Finding::error("marketplace-source", native_path.clone(), format!("install.source `{source}` is not `{}`", Repo::native_package_rel(pid)), "run `omm build`"));
                }
                let digest = e["integrity"]["digest"].as_str().unwrap_or("");
                if !marketplace::is_digest(digest) {
                    out.push(Finding::error("marketplace-digest", native_path.clone(), format!("integrity.digest {digest:?} is not `sha256:` + 64 hex"), "run `omm build` with the host available"));
                } else if let Some(expected) = expected_digest {
                    if expected != digest {
                        out.push(Finding::error(
                            "marketplace-digest",
                            native_path.clone(),
                            format!("integrity.digest {digest} is stale; the package digests to {expected} — every `omm install` would fail its integrity check"),
                            "run `omm build`",
                        ));
                    }
                }
                if e["availability"]["status"].as_str().is_none() {
                    out.push(Finding::error("marketplace-shape", native_path.clone(), "availability.status missing".to_string(), "run `omm build`"));
                }
            }
        }
    }
    let codex_path = repo.marketplace_codex_path();
    match read_json(&codex_path) {
        None => out.push(Finding::warning(
            "marketplace-projection-missing",
            codex_path.clone(),
            "the Codex-schema catalog is missing (Muse never reads it; hosts of that schema do)"
                .to_string(),
            "run `omm build`",
        )),
        Some(Err(e)) => out.push(Finding::error(
            "marketplace-shape",
            codex_path.clone(),
            e,
            "run `omm build`",
        )),
        Some(Ok(v)) => {
            for e in v["plugins"].as_array().cloned().unwrap_or_default() {
                check_entry_name(&mut out, &codex_path, &e, pid);
                let src = &e["source"];
                let path = src["path"].as_str().unwrap_or("");
                if !src.is_object()
                    || src["source"].as_str() != Some("local")
                    || !rel_source_ok(path)
                {
                    out.push(Finding::error("marketplace-source", codex_path.clone(), format!("entry source must be the OBJECT {{\"source\":\"local\",\"path\":<rel>}}, got {src}"), "run `omm build`"));
                }
            }
        }
    }
    let claude_path = repo.marketplace_claude_path();
    match read_json(&claude_path) {
        None => out.push(Finding::warning(
            "marketplace-projection-missing",
            claude_path.clone(),
            "the Claude-schema catalog is missing (Muse never reads it; hosts of that schema do)"
                .to_string(),
            "run `omm build`",
        )),
        Some(Err(e)) => out.push(Finding::error(
            "marketplace-shape",
            claude_path.clone(),
            e,
            "run `omm build`",
        )),
        Some(Ok(v)) => {
            if v["name"].as_str() != Some(crate::MARKETPLACE_NAME) {
                out.push(Finding::error(
                    "marketplace-name",
                    claude_path.clone(),
                    format!(
                        "marketplace name {} is not `{}` (`omm@{}` must work in every tool)",
                        v["name"],
                        crate::MARKETPLACE_NAME,
                        crate::MARKETPLACE_NAME
                    ),
                    "run `omm build`",
                ));
            }
            for e in v["plugins"].as_array().cloned().unwrap_or_default() {
                check_entry_name(&mut out, &claude_path, &e, pid);
                match e["source"].as_str() {
                    Some(s) if rel_source_ok(s) => {}
                    _ => out.push(Finding::error("marketplace-source", claude_path.clone(), format!("entry source must be a relative STRING (an object is remote and skipped), got {}", e["source"]), "run `omm build`")),
                }
            }
        }
    }
    Ok(out)
}

fn check_entry_name(out: &mut Vec<Finding>, path: &Path, entry: &Value, pid: &str) {
    if entry["name"].as_str() != Some(pid) {
        out.push(Finding::error(
            "marketplace-name",
            path.to_path_buf(),
            format!("entry name {} is not the manifest name `{pid}` (a mismatch fails only at install time)", entry["name"]),
            "run `omm build`",
        ));
    }
}

fn rel_source_ok(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('/')
        && !s.contains('\\')
        && !s.split('/').any(|seg| seg == "..")
}

fn read_json(path: &Path) -> Option<std::result::Result<Value, String>> {
    if !path.exists() {
        return None;
    }
    Some(
        std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string())),
    )
}

/// The R13 checkpoints against the real binary: `muse skills validate
/// <dir> --json` per skill (valid, no diagnostics, `compatibility.result ==
/// compatible`), then `muse plugins validate <pkg> --json` on R11's four
/// predicates, on the package written to a temp dir. Returns the findings and
/// the package digest (obtained on the way, for the marketplace check).
pub fn host_checkpoints(
    inv: &Invoker,
    content: &Content,
    pkg: &Package,
) -> Result<(Vec<Finding>, Option<String>)> {
    let mut out = Vec::new();
    for s in &content.skills {
        let dir = content.root.join(&s.dir);
        let outcome = inv.run(&[
            "skills".to_string(),
            "validate".to_string(),
            dir.to_string_lossy().into_owned(),
            "--json".to_string(),
        ])?;
        let json = outcome.first_json().unwrap_or(Value::Null);
        let valid = json["valid"].as_bool().unwrap_or(false);
        let diagnostics = json["diagnostics"].as_array().map(Vec::len).unwrap_or(0);
        let compat = json["compatibility"]["result"].as_str().unwrap_or("");
        if !outcome.ok() || !valid || diagnostics != 0 || compat != "compatible" {
            let detail = json["error"]["message"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| json["diagnostics"].to_string());
            out.push(Finding::error(
                "host-skills-validate",
                s.skill_md.abs.clone(),
                format!("`muse skills validate` exit {:?}, valid {valid}, {diagnostics} diagnostic(s), compatibility {compat:?}: {detail}", outcome.code),
                "fix the skill until the validator is clean (warnings are failures)",
            ));
        }
    }
    let tmp = tempfile::Builder::new()
        .prefix("omm-lint-pkg-")
        .tempdir()
        .map_err(|e| crate::ManifestError::io("create temp dir", std::env::temp_dir(), e))?;
    let pkg_dir = tmp.path().join("pkg");
    pkg.write_to(&pkg_dir)?;
    let validation = probe::plugins_validate(inv, &pkg_dir)?;
    for failed in validation.four_predicates() {
        out.push(Finding::error(
            "host-plugins-validate",
            PathBuf::from(native::manifest_path()),
            format!(
                "`muse plugins validate`: {failed}{}",
                validation
                    .error
                    .as_ref()
                    .map(|(c, m)| format!(" ({c}: {m})"))
                    .unwrap_or_default()
            ),
            "fix the package until all four predicates hold (R11)",
        ));
    }
    let digest = if validation.passes() {
        Some(marketplace::package_digest(inv, pkg)?)
    } else {
        None
    };
    Ok((out, digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_grammar_matches_the_host() {
        assert!(is_valid_id("a"));
        assert!(is_valid_id("0"));
        assert!(is_valid_id("ok.id_1-2"));
        assert!(is_valid_id(&"a".repeat(80)));
        assert!(!is_valid_id(&"a".repeat(81)));
        assert!(!is_valid_id("Upper"));
        assert!(!is_valid_id("-lead"));
        assert!(!is_valid_id(".lead"));
        assert!(!is_valid_id("_lead"));
        assert!(is_valid_id("trail-"));
        assert!(!is_valid_id("has space"));
        assert!(!is_valid_id("caf\u{e9}"));
        assert!(!is_valid_id(""));
    }

    #[test]
    fn whole_word_matching_exempts_path_literals() {
        assert!(contains_whole_word("use Claude here", "Claude"));
        assert!(contains_whole_word("Claude", "Claude"));
        assert!(!contains_whole_word(".claude-plugin", "Claude"));
        assert!(!contains_whole_word("CLAUDE.md", "Claude"));
        assert!(!contains_whole_word("omm-Claude", "Claude"));
        assert!(!contains_whole_word("Claude-schema", "Claude"));
        assert!(contains_whole_word("(Claude)", "Claude"));
        assert!(!contains_whole_word("Claudette", "Claude"));
    }

    #[test]
    fn the_shipped_content_lints_clean_without_a_host() {
        let repo = crate::Repo::from_cargo_manifest_dir().unwrap();
        let report = run(&repo, &LintOptions::default()).unwrap();
        let errors: Vec<String> = report.errors().iter().map(|f| f.to_string()).collect();
        assert!(errors.is_empty(), "{errors:#?}");
        assert!(!report.host_checked);
        assert!(report.package_files.unwrap() > 12);
        let b = report.budget.as_ref().unwrap();
        assert!(b.within_full() && b.within_first_sentence());
        assert!(
            b.stale().is_empty(),
            "catalog budget numbers are stale: {:#?}",
            b.stale()
        );
        // Not a single finding: the catalog's header totals are data the
        // content owner keeps exact (Gate 1: `omm lint` warned
        // catalog-budget-stale on a tree `omm build --check` called clean).
        assert!(report.findings.is_empty(), "{:#?}", report.findings);
    }
}
