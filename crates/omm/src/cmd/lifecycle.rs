//! Group A — the ledger lifecycle: `install`, `uninstall`, `update`,
//! `reconcile`, `trust`, `list` (ARCHITECTURE.md §7; R1–R6, R14).
//!
//! This file owns the clap surface of these commands (`cli.rs` only names
//! the `Args` structs) and their bodies; the pieces live beside it under
//! `a_lifecycle/`:
//!
//! | module | what |
//! |---|---|
//! | `source`  | the content checkout (`--source` / `$OMM_SOURCE` / the build checkout), catalog + content, the R18 budget line |
//! | `session` | the ledger under its lock, the four bases, the audit writer, the host info |
//! | `plugin`  | the marketplace + plugin transaction of `docs/experiments/marketplace-precedence.md` §6.4 (R11, R14) |
//! | `skills`  | `--no-plugin`: `muse skills install` per skill, updated per skill under R3 |
//! | `files`   | `AGENTS.md` (user region preserved), themes, the default profile's settings keys, the workspace trust entry |
//! | `rules`   | the marked-region text logic of the rules file (pure) |
//!
//! Every write command asks for R6 consent first, honours `--dry-run`
//! (nothing under `$OMM/` or the host's roots is created — a corrupt ledger
//! is still quarantined on load, loudly), records every write in the
//! ledger with an audit line, and ends with the converge summary; under
//! `--json` stdout carries exactly one document. Idempotent: a second run
//! converges to "unchanged" everywhere.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use serde_json::{json, Map, Value};

use omm_host::fsx;
use omm_host::probe::{self, SkillsListOptions};
use omm_host::settings::SettingsDoc;
use omm_host::Invoker;
use omm_ledger::audit::{Event, ACTION_INSTALL, ACTION_SNAPSHOT, ACTION_UNINSTALL, ACTION_UPDATE};
use omm_ledger::containment::{Bases, State};
use omm_ledger::reconcile::{StageReport, Staged, REPORT_FILE, UPDATES_DIR};
use omm_ledger::shared::Original;
use omm_ledger::uninstall::{
    self, CreatedFile, HostUndoer, HostView, RecordingUndoer, UndoStep, Undoer,
};
use omm_ledger::{hash, snapshot, store, Base, EntryKey, Ledger, Registration, RelPath};
use omm_manifest::catalog::{AssetKind, Catalog};
use omm_manifest::overlay;

use crate::cmd::tune::c_tune::{profile, read_ledger, ContentSource};
use crate::cmd::{plugin_id, Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Action, Converge, Render, Table};

#[path = "a_lifecycle/files.rs"]
mod files;
#[path = "a_lifecycle/plugin.rs"]
mod plugin;
#[path = "a_lifecycle/rules.rs"]
pub mod rules;
#[path = "a_lifecycle/session.rs"]
mod session;
#[path = "a_lifecycle/skills.rs"]
mod skills;
#[path = "a_lifecycle/source.rs"]
pub mod source;

use files::Mode;
use session::Session;
use source::Source;

/// The file stem of the profile `omm install` applies (ARCHITECTURE.md §7:
/// "Both: `AGENTS.md`, default profile, themes"). A convention of the
/// content tree (`content/profiles/<stem>.json`), overridable with
/// `--profile <id>`; the profile's id and keys stay catalog data (R8).
pub const DEFAULT_PROFILE_STEM: &str = "default";

/// `omm install [--no-plugin] [--reinstall] [--source <checkout>] [--workspace <dir>] [--profile <id>]`
/// (+ global `--yes`, `--dry-run`, `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct InstallArgs {
    /// Install each skill with `muse skills install` instead of the plugin bundle
    #[arg(long)]
    pub no_plugin: bool,
    /// Remove and install the plugin again even when its digest is unchanged (the fix doctor D1/D10 print)
    #[arg(long)]
    pub reinstall: bool,
    /// The content checkout to install from (default: $OMM_SOURCE, else the checkout the ledger's marketplace registration names, else the checkout this binary was built from)
    #[arg(long, value_name = "DIR")]
    pub source: Option<PathBuf>,
    /// The workspace to trust when it is a git checkout (default: the current directory)
    #[arg(long, value_name = "DIR")]
    pub workspace: Option<PathBuf>,
    /// The settings profile to apply (default: the `default` profile of the source)
    #[arg(long, value_name = "ID")]
    pub profile: Option<String>,
    /// Proceed although settings.json holds keys the host does not type: the bundle's host verbs rewrite the file and drop them (recorded, put back after the host's rewrite and at uninstall)
    #[arg(long)]
    pub drop_unknown_settings_keys: bool,
    /// Leave a step out of the plan (repeatable): themes, rules, profile, trust — e.g. `--skip themes` when a dotfile manager symlinks `themes/` out of the config root
    #[arg(long, value_name = "STEP")]
    pub skip: Vec<String>,
}

/// `omm uninstall [--force] [--reconcile-host]` (+ global `--dry-run`, `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct UninstallArgs {
    /// Also remove files the user edited and the whole $OMM tree (never a symlink or directory at a ledgered path)
    #[arg(long)]
    pub force: bool,
    /// Also remove omm's plugin, marketplace and managed-store skills the host holds but the ledger does not list (an interrupted install; with no ledger at all, that is everything) and say so
    #[arg(long)]
    pub reconcile_host: bool,
}

/// `omm update [--reinstall] [--source <checkout>]` (+ global `--dry-run`, `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct UpdateArgs {
    /// Reinstall the plugin even when the marketplace digest is unchanged
    #[arg(long)]
    pub reinstall: bool,
    /// The content checkout to update from (default: $OMM_SOURCE, else the checkout the ledger's marketplace registration names, else the build checkout)
    #[arg(long, value_name = "DIR")]
    pub source: Option<PathBuf>,
}

/// `omm reconcile` (+ global `--dry-run`, `--json`): doctor D13's fix.
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct ReconcileArgs {}

/// `omm trust [path]`.
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustArgs {
    /// Workspace root (default: the current directory)
    pub path: Option<PathBuf>,
}

/// `omm list [--source <checkout>] [--kind <kind>]` (+ global `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct ListArgs {
    /// The content checkout to resolve against (default: $OMM_SOURCE, else the checkout the ledger's marketplace registration names, else the build checkout)
    #[arg(long, value_name = "DIR")]
    pub source: Option<PathBuf>,
    /// Only assets of this kind (skill, command, hook, reminder, mcp_server, agent, theme, profile, rules, translation)
    #[arg(long, value_name = "KIND")]
    pub kind: Option<String>,
}

// ---------------------------------------------------------------------------
// shared bits
// ---------------------------------------------------------------------------

/// The one `--json` document of a command, built as it goes.
struct Doc(Map<String, Value>);

impl Doc {
    fn new(command: &str, ctx: &Ctx) -> Doc {
        let mut m = Map::new();
        m.insert("command".into(), json!(command));
        m.insert("omm_version".into(), json!(OMM_VERSION));
        m.insert("dry_run".into(), json!(ctx.dry_run));
        Doc(m)
    }
    fn set(&mut self, key: &str, v: Value) -> &mut Doc {
        self.0.insert(key.to_string(), v);
        self
    }
    fn finish(mut self, converge: &Converge) -> Value {
        self.set("converge", converge.to_json());
        Value::Object(self.0)
    }
}

/// The canonical workspace: `--workspace`, else the cwd when it exists.
fn resolve_workspace(ctx: &Ctx, explicit: Option<&Path>) -> Result<Option<PathBuf>> {
    match explicit {
        Some(p) => Ok(Some(fsx::canonicalize(p)?)),
        None => Ok(ctx.workspace().ok()),
    }
}

/// Refuse up front when `settings.json` or `trust.json` is malformed the
/// way the host's own loaders judge it (Gate 1 round 5 M3): every
/// config-loading verb — `plugins install`, `skills list`, the R13
/// cross-check — exits 1 while either is, so an install that did not check
/// registered the plugin, the marketplace and the settings keys and then
/// died at the cross-check, wedging uninstall until the file was repaired
/// by hand. `SettingsDoc::load` / `TrustStore::check_shape` are the same
/// judgements the host applies; the refusal carries the host's reason and
/// the repair hint, before the first host mutation, nothing written.
fn refuse_host_config_shape(ctx: &Ctx) -> Result<()> {
    let settings = ctx.roots.settings_file();
    if let Err(e) = SettingsDoc::load(&settings) {
        return Err(OmmError::Usage(format!(
            "{} is not a settings.json the host can load ({e}); every config-loading muse verb (`plugins install`, `skills list`) exits 1 while it is, so this install would half-complete and wedge uninstall. Repair or move it, then rerun. Nothing was written",
            settings.display()
        )));
    }
    let trust = ctx.roots.trust_file();
    if let Err(e) = omm_host::trust::TrustStore::load(&trust) {
        return Err(OmmError::Usage(format!(
            "{} is not a trust.json the host can load ({e}); every config-loading muse verb (`plugins install`, `skills list`) exits 1 while it is, so this install would half-complete and wedge uninstall. Repair or move it, then rerun. Nothing was written",
            trust.display()
        )));
    }
    Ok(())
}

/// The host's own judgement of its config, before the first mutation:
/// `muse skills list --json` loads `settings.json` and `trust.json` the way
/// every install-time verb does, where `--version` answers over a malformed
/// file (host-reality.md "malformed settings.json": lazy). The shape mirror
/// ([`refuse_host_config_shape`]) names the common cases precisely; a typed
/// error only the host's loader sees (`"tui": 5`) is refused here with the
/// host's reason, nothing written (Gate 1 round 5 M3).
fn refuse_unloadable_host_config(ctx: &Ctx, inv: &Invoker) -> Result<()> {
    if let Err(e) = probe::skills_list(inv, &SkillsListOptions::default()) {
        return Err(OmmError::Usage(format!(
            "the host cannot load its config ({e}); every config-loading muse verb (`plugins install`, `skills install`, `skills list`) exits 1 while it is, so this run would half-complete and wedge uninstall. Repair the file the host names ({} / {}), or move it aside, then rerun. Nothing was written",
            ctx.roots.settings_file().display(),
            ctx.roots.trust_file().display()
        )));
    }
    Ok(())
}

/// The host-side refusal every settings-writing verb hits (D3): refuse
/// before anything is written rather than add to the collision.
fn refuse_mcp_collision(ctx: &Ctx) -> Result<()> {
    let doc = SettingsDoc::for_roots(&ctx.roots)?;
    if doc.has_mcp_collision() {
        return Err(OmmError::Host(omm_host::HostError::McpCollision));
    }
    Ok(())
}

/// The keys of `settings.json` the host does not type (D4's finding), which
/// the bundle install's own host verbs — `plugins install` and every
/// `plugins approve` rewrite the file from the host's typed struct — will
/// destroy (Gate 1: silently, with the file's mode). Refused before the
/// first host mutation unless `--drop-unknown-settings-keys` says the user
/// knows; then the install records the file's bytes and mode in the ledger
/// and uninstall puts them back. `--no-plugin` needs no opt-in: `muse
/// skills install` does not rewrite the file (measured 2026-09-02) and
/// omm's own patch keeps every key.
fn refuse_untyped_settings_keys(ctx: &Ctx, opted_in: bool) -> Result<Vec<String>> {
    let doc = SettingsDoc::for_roots(&ctx.roots)?;
    let keys: Vec<String> = omm_host::settings::untyped_members(doc.value())?
        .into_iter()
        .map(|(path, _)| path.join("."))
        .collect();
    if keys.is_empty() {
        return Ok(keys);
    }
    if opted_in {
        ctx.out.warn(format!(
            "settings.json: {} key(s) the host does not type will be dropped by its rewrite during this install ({}); the file's bytes and mode are recorded in the ledger and put back at uninstall (--drop-unknown-settings-keys)",
            keys.len(),
            keys.join(", ")
        ));
        return Ok(keys);
    }
    Err(OmmError::Usage(format!(
        "{}: {} key(s) the host does not type and destroys on its next settings rewrite: {}. This install runs `muse plugins install` and `plugins approve`, which rewrite the file, so they would be lost. Move them out of settings.json, or pass --drop-unknown-settings-keys: omm then records the file's bytes and mode in the ledger and puts them back at uninstall (doctor D4 keeps warning meanwhile)",
        doc.path().display(),
        keys.len(),
        keys.join(", ")
    )))
}

/// Managed-store skills that came from an omm content checkout, told with
/// no ledger to say so: a skill `muse skills list --source user` lists whose
/// host lockfile source path lies in a checkout whose `content/catalog.json`
/// ships it (`skills/.muse/lock.json`, `skills.<id>.source.path`), or whose
/// id the resolvable source catalog ships. `(id, evidence)`, sorted.
fn managed_omm_skills(ctx: &Ctx, inv: &Invoker) -> Result<Vec<(String, String)>> {
    let listed = probe::skills_list(
        inv,
        &SkillsListOptions {
            source: Some("user".to_string()),
            ..SkillsListOptions::default()
        },
    )?;
    if listed.skills.is_empty() {
        return Ok(Vec::new());
    }
    let lock: Value = std::fs::read(
        ctx.roots
            .personal_skills_dir()
            .join(".muse")
            .join("lock.json"),
    )
    .ok()
    .and_then(|b| serde_json::from_slice(&b).ok())
    .unwrap_or(Value::Null);
    let resolved = Source::resolve_with_ledger(None, &ctx.omm_root()).ok();
    let mut out = Vec::new();
    for skill in &listed.skills {
        let id = skill.id.as_str();
        let from_lock = lock["skills"][id]["source"]["path"]
            .as_str()
            .map(PathBuf::from)
            .and_then(|p| {
                let content = p.parent()?.parent()?.to_path_buf();
                let catalog = Catalog::load(&content.join(omm_manifest::CATALOG_FILE)).ok()?;
                let shipped = catalog.shipped_of(AssetKind::Skill).any(|a| a.id == id);
                shipped.then(|| format!("installed from {} (an omm content checkout)", p.display()))
            });
        let from_source = resolved.as_ref().and_then(|s| {
            s.content
                .skills
                .iter()
                .any(|k| k.asset.id == id)
                .then(|| format!("shipped by the source at {}", s.repo.root.display()))
        });
        if let Some(evidence) = from_lock.or(from_source) {
            out.push((id.to_string(), evidence));
        }
    }
    out.sort();
    Ok(out)
}

/// The personal rules file when it carries omm's managed block but the
/// ledger has no entry for it (Gate 1 round 5 M2): a full `AGENTS.md` left
/// by an interrupt after the write but before the ledger save. `None` when
/// there is an entry, no file, or no block.
fn unlisted_managed_rules(ctx: &Ctx, ledger: &Ledger) -> Option<String> {
    let path = ctx.roots.personal_rules_file();
    let text = std::fs::read_to_string(&path).ok()?;
    if !text.contains(omm_host::host_reality::RULES_MANAGED_START_MARKER) {
        return None;
    }
    let rel = files::config_rel(ctx, &path).ok()?;
    ledger
        .find(Base::MuseConfig, &rel)
        .is_none()
        .then(|| path.display().to_string())
}

/// Whether the managed-store directory of `id` is safe to hand the host a
/// `skills uninstall` for — resolved through [`Bases::resolve`] (Gate 1
/// round 5 decision H1). `Some(reason)` when it is a symlink now, is reached
/// through one, or resolves outside the base: omm never wrote a symlink
/// (R12), so what lies behind one is the user's, and the host would empty
/// whatever it points at — never asked, preserved with the R2-sentinel
/// reason. `None` when it is a real directory (or gone) inside the base.
/// A resolve that fails for any other reason is treated as unsafe too:
/// omm never follows a path it cannot contain.
fn unsafe_store_dir(ctx: &Ctx, bases: &Bases, id: &str) -> Result<Option<String>> {
    let rel = skills::store_dir_rel(ctx, id)?;
    Ok(match bases.resolve(Base::MuseConfig, &rel) {
        Ok(r) if r.state == State::Symlink => Some(format!(
            "the managed-store directory {} is a symlink now — not what omm wrote (R2 sentinel); the host is never asked to uninstall it",
            r.path.display()
        )),
        Ok(r) if r.via_symlink.is_some() => Some(format!(
            "an ancestor ({}) of the managed-store directory of `{id}` is a symlink now — not what omm wrote (R2 sentinel); the host is never asked to uninstall it",
            r.via_symlink.as_deref().map(|p| p.display().to_string()).unwrap_or_default()
        )),
        Ok(_) => None,
        Err(e) if e.is_escape() => Some(format!(
            "the managed-store directory of `{id}` {e}; the host is never asked to uninstall it"
        )),
        Err(e) => Some(format!(
            "the managed-store directory of `{id}` does not resolve inside the base ({e}); the host is never asked to uninstall it"
        )),
    })
}

/// Settings keys and trust entries that are current but carry no
/// registration — a profile's leaf holding the profile's value, the
/// workspace's trust entry — after a ledger was rebuilt from the host
/// (`omm reconcile` on a corrupt one): their priors are unknown, so
/// uninstall leaves them and names them (Gate 1: it left them silently).
fn unregistered_shared_state(ctx: &Ctx, ledger: &Ledger, workspace: Option<&Path>) -> Vec<String> {
    let mut out = Vec::new();
    if let (Some(source), Ok(doc)) = (
        ContentSource::locate(&ctx.omm_root()),
        SettingsDoc::for_roots(&ctx.roots),
    ) {
        if let Ok(catalog) = source.catalog() {
            for asset in catalog.shipped_of(AssetKind::Profile) {
                let Ok(path) = source.asset_path(&asset.path) else {
                    continue;
                };
                let Ok(slice) = profile::load_slice(&path) else {
                    continue;
                };
                let stem = Path::new(&asset.path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&asset.id)
                    .to_string();
                for op in profile::flatten(&slice) {
                    let key = op.path.join(".");
                    if ledger.settings_key(&key).is_some() {
                        continue;
                    }
                    if doc.get(&op.path) == op.value.as_ref() {
                        let line = format!(
                            "settings key {key} = {} (profile {stem}; prior unknown, kept)",
                            op.value.clone().unwrap_or(Value::Null)
                        );
                        if !out
                            .iter()
                            .any(|l: &String| l.starts_with(&format!("settings key {key} ")))
                        {
                            out.push(line);
                        }
                    }
                }
            }
        }
    }
    if let Some(ws) = workspace {
        if let Ok(store) = omm_host::trust::TrustStore::for_roots(&ctx.roots) {
            if let (Ok(key), Ok(Some(decision))) = (
                omm_host::trust::TrustStore::key_for(ws),
                store.decision_for(ws),
            ) {
                let registered = ledger
                    .registrations
                    .iter()
                    .any(|r| matches!(r, Registration::Trust { project, .. } if *project == key));
                if !registered {
                    out.push(format!(
                        "trust entry {key} = {decision} (prior unknown, kept)"
                    ));
                }
            }
        }
    }
    out
}

/// The source's plugin id must be the one the data files reserve (R19).
fn checked_source(ctx: &Ctx, explicit: Option<&Path>) -> Result<(Source, String)> {
    let source = Source::resolve_with_ledger(explicit, &ctx.omm_root())?;
    let pid = plugin_id()?;
    if source.catalog.plugin_id != pid {
        return Err(OmmError::Usage(format!(
            "{}: catalog plugin_id {:?} is not the reserved omm identity {:?} (docs/host-data/reserved-ids.json)",
            source.repo.catalog_path().display(),
            source.catalog.plugin_id,
            pid
        )));
    }
    Ok((source, pid))
}

fn source_json(source: &Source) -> Value {
    json!({
        "root": source.repo.root,
        "origin": source.origin.label(),
        "version": source.version,
    })
}

/// The profile asset `omm install` applies: `--profile <id>`, else the
/// catalog's profile whose file stem is [`DEFAULT_PROFILE_STEM`].
fn default_profile_id(source: &Source, explicit: Option<&str>) -> Result<String> {
    if let Some(id) = explicit {
        return Ok(id.to_string());
    }
    source
        .content
        .profiles
        .iter()
        .find(|p| {
            Path::new(&p.file.rel).file_stem().and_then(|s| s.to_str())
                == Some(DEFAULT_PROFILE_STEM)
        })
        .map(|p| p.asset.id.clone())
        .ok_or_else(|| {
            OmmError::Usage(format!(
                "the source ships no `profiles/{DEFAULT_PROFILE_STEM}.json`; pass --profile <id>"
            ))
        })
}

fn staged_json(s: &Staged) -> Value {
    json!({
        "base": s.base.as_str(),
        "path": s.path.as_str(),
        "staged_to": s.staged_to,
        "reason": s.reason,
        "ancestor": s.ancestor,
        "theirs": s.theirs,
        "mine": s.mine,
    })
}

/// Host-owned paths inside our bases that an uninstall must NAME and never
/// touch (host-reality.md "Paths": the startup locks, the managed-store
/// metadata, the plugin store, the bootstrap traces, the session logs) —
/// added to the plan's residue list unconditionally: the uninstall's own
/// `skills uninstall` / `plugins remove` create `.settings.json.lock` after
/// the plan was made, and a session log is the host's whenever it exists
/// (Gate 1: the preview named only what already existed at plan time).
/// The fixed host names come from `Roots`, never spelled here.
pub fn host_residue(ctx: &Ctx) -> Vec<PathBuf> {
    let cfg = ctx.roots.muse_config();
    let mut out = vec![
        cfg.join(".auth.json.lock"),
        cfg.join(".settings.json.lock"),
        ctx.roots.personal_skills_dir().join(".muse"),
        ctx.roots.plugins_dir(),
        ctx.roots.sessions_dir(),
    ];
    // The whole tracing dir, not only `bootstrap/` (the host adds lanes).
    out.push(
        ctx.roots
            .bootstrap_trace_dir()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| ctx.roots.bootstrap_trace_dir()),
    );
    // The fallback session-registry root: outside our bases on macOS, inside
    // them on Linux — named unconditionally either way.
    out.push(ctx.roots.runtime_fallback_dir());
    out
}

fn name_host_residue(ctx: &Ctx, bases: &mut Bases) {
    for p in host_residue(ctx) {
        if !bases.residue.contains(&p) {
            bases.residue.push(p);
        }
    }
}

/// One thing of omm's the host holds that the ledger does not list.
struct HostExtra {
    step: UndoStep,
    reason: String,
}

/// A managed-store directory the host lists with no host provenance (an
/// install interrupted inside `muse skills install`; `provenance-missing`)
/// that omm removes itself — `muse skills uninstall` refuses it (Gate 1
/// round 5 M1).
struct Orphan {
    id: String,
    dir: PathBuf,
    files: Vec<PathBuf>,
    reason: String,
}

/// What the host holds of omm's beyond a ledger (or with no ledger at all).
#[derive(Default)]
struct Beyond {
    /// Host verbs to run: `skills uninstall` for a lockfile-listed skill,
    /// the plugin removal, the marketplace removal.
    extras: Vec<HostExtra>,
    /// Store directories the host has no provenance for that omm removes
    /// itself (identical to the source, inside the base, no symlink).
    orphans: Vec<Orphan>,
    /// Left in place and named: a symlinked store dir (H1), a dir whose
    /// content differs, or one omm cannot verify.
    kept: Vec<(String, String)>,
}

impl Beyond {
    /// Whether the host holds anything of omm's the ledger does not — the
    /// things a plain uninstall refuses and `--reconcile-host` undoes.
    fn any_reconcilable(&self) -> bool {
        !self.extras.is_empty() || !self.orphans.is_empty()
    }
    /// A count for the preview header.
    fn reconcilable_len(&self) -> usize {
        self.extras.len() + self.orphans.len()
    }
}

/// Classify one managed-store skill the ledger does not list: the host's
/// verb when it has provenance, an omm removal when it is an orphan of an
/// interrupted install, or kept when it is a symlink / differs / cannot be
/// verified (Gate 1 round 5 H1 + M1). `source` is needed to compare bytes;
/// without it a non-lockfile dir is kept and named rather than handed a
/// failing `muse skills uninstall`.
fn classify_beyond_skill(
    ctx: &Ctx,
    bases: &Bases,
    source: Option<&Source>,
    id: &str,
    why: &str,
    beyond: &mut Beyond,
) -> Result<()> {
    // A store directory that is (or is reached through) a symlink is the
    // user's — never handed to the host, never removed (H1).
    if let Some(reason) = unsafe_store_dir(ctx, bases, id)? {
        beyond.kept.push((id.to_string(), reason));
        return Ok(());
    }
    match source {
        Some(source) => match skills::classify_orphan_store_dir(ctx, bases, source, id)? {
            skills::OrphanVerdict::HostOwned => beyond.extras.push(HostExtra {
                reason: format!("managed-store skill `{id}` {why}; the ledger has no entry for it"),
                step: UndoStep::SkillsUninstall { id: id.to_string() },
            }),
            skills::OrphanVerdict::Remove { dir, files } => beyond.orphans.push(Orphan {
                id: id.to_string(),
                dir,
                files,
                reason: format!(
                    "managed-store skill `{id}` {why}, but the host has no provenance for it (an install interrupted inside `muse skills install`); identical to the source — omm removes it"
                ),
            }),
            skills::OrphanVerdict::Keep(reason) => {
                beyond.kept.push((id.to_string(), reason))
            }
        },
        None => {
            // No content source to compare against: hand the host verb only
            // when it has provenance, else keep and name it.
            if skills::store_dir_managed_by_host(ctx, id) {
                beyond.extras.push(HostExtra {
                    reason: format!(
                        "managed-store skill `{id}` {why}; the ledger has no entry for it"
                    ),
                    step: UndoStep::SkillsUninstall { id: id.to_string() },
                });
            } else {
                beyond.kept.push((
                    id.to_string(),
                    "the host has no provenance for it and no content source is available to verify it is omm's; `omm install --no-plugin --source <checkout>` reconciles it".into(),
                ));
            }
        }
    }
    Ok(())
}

/// What the host holds of omm's beyond a ledger: the plugin with no
/// `muse-plugin` registration, the marketplace with no `muse-marketplace`
/// registration, managed-store skills from an omm checkout the ledger has
/// no entries for — an install interrupted between a host verb and the
/// ledger save that follows it (Gate 1: uninstall traversed the ledger,
/// reported rc 0 with no errors, and left the plugin — or a skill — on
/// the host; only a second, ledger-less run refused). A store directory the
/// host lists but has no provenance for (killed inside `muse skills
/// install`) is removed by omm, not the host (M1); a symlinked one is left
/// (H1). In the order the host wants the verbs undone: skills, plugin,
/// marketplace.
fn host_beyond_ledger(
    ctx: &Ctx,
    inv: &Invoker,
    ledger: &Ledger,
    pid: &str,
    source: Option<&Source>,
) -> Result<Beyond> {
    let (plugin_reg, mkt_reg) = plugin::registrations(ledger, pid);
    let ledgered = skills::ledgered_ids(ctx, ledger);
    let bases = Bases::from_roots(&ctx.roots, ctx.workspace().ok().as_deref());
    let mut beyond = Beyond::default();
    for (id, why) in managed_omm_skills(ctx, inv)? {
        if ledgered.contains(&id) {
            continue;
        }
        classify_beyond_skill(ctx, &bases, source, &id, &why, &mut beyond)?;
    }
    if plugin_reg.is_none() {
        if let Some(p) = plugin::installed(inv, pid)? {
            beyond.extras.push(HostExtra {
                step: UndoStep::PluginRemove {
                    id: pid.to_string(),
                },
                reason: format!(
                    "plugin `{pid}` is installed ({}) with no muse-plugin registration",
                    p.package_sha256
                        .as_deref()
                        .map(plugin::short)
                        .unwrap_or_else(|| "digest unknown".into())
                ),
            });
        }
    }
    if mkt_reg.is_none() {
        if let Some(m) = plugin::configured_marketplace(inv, omm_manifest::MARKETPLACE_NAME)? {
            beyond.extras.push(HostExtra {
                reason: format!(
                    "marketplace `{}` is configured ({}) with no muse-marketplace registration",
                    m.name, m.source
                ),
                step: UndoStep::MarketplaceRemove { name: m.name },
            });
        }
    }
    Ok(beyond)
}

/// Put the extra host steps where the plan's order wants them: a skill
/// after the plan's own `skills uninstall` steps, the plugin before the
/// marketplace removal (the host removes a plugin before its marketplace),
/// the marketplace last.
fn splice_host_extras(steps: &mut Vec<UndoStep>, extras: &[HostExtra]) {
    for x in extras {
        let at = match &x.step {
            UndoStep::SkillsUninstall { .. } => steps
                .iter()
                .rposition(|s| matches!(s, UndoStep::SkillsUninstall { .. }))
                .map(|i| i + 1)
                .unwrap_or(0),
            UndoStep::PluginRemove { .. } => steps
                .iter()
                .position(|s| matches!(s, UndoStep::MarketplaceRemove { .. }))
                .unwrap_or(steps.len()),
            _ => steps.len(),
        };
        steps.insert(at, x.step.clone());
    }
}

/// `plugins remove --delete-data` writes `{"schema_version":1}` when there
/// is no `settings.json`. The bundle install ledgers the file as `seeded`
/// the moment the package is installed — but a kill between `plugins
/// install` and that save leaves the ledger without it, so the plan of a
/// `--reconcile-host` uninstall that removes the plugin must still know to
/// remove the file again when it comes back empty (R5; e2e s15c).
fn name_recreated_settings(ctx: &Ctx, bases: &Bases, plan: &mut uninstall::Plan) -> Result<()> {
    let abs = ctx.roots.settings_file();
    if abs.exists() {
        return Ok(());
    }
    let key = EntryKey {
        base: Base::MuseConfig,
        path: files::config_rel(ctx, &abs)?,
    };
    if plan.created.iter().any(|c| c.key == key) {
        return Ok(());
    }
    let resolved = match bases.resolve(key.base, &key.path) {
        Ok(r) => r.path,
        // The whole config root is gone with the file (a kill before
        // either landed): `abs` is already the Missing path `resolve`
        // would report, straight from `Roots`.
        Err(_) if bases.root_absent(key.base) => abs.clone(),
        Err(e) => return Err(e.into()),
    };
    plan.created.push(CreatedFile {
        key,
        abs: resolved,
        absent_at_plan: true,
    });
    Ok(())
}

/// The `omm_ledger::uninstall::Options` of this CLI: the rules markers and
/// the shared files' paths come from the content constants and `Roots`;
/// `host` is what the host holds now ([`host_view`]).
fn uninstall_options(ctx: &Ctx, force: bool, host: Option<HostView>) -> uninstall::Options {
    uninstall::Options {
        force,
        rules: Some(rules::markers()),
        settings_file: Some(ctx.roots.settings_file()),
        trust_file: Some(ctx.roots.trust_file()),
        host,
    }
}

/// What the host holds now of the kinds omm registers — asked before the
/// plan is made (Gate 1 decision B): a registered plugin, marketplace or
/// managed skill the host no longer has is dropped as already gone.
fn host_view(ctx: &Ctx, inv: &Invoker) -> Result<HostView> {
    let _ = ctx;
    let plugins = plugin::installed_ids(inv)?.into_iter().collect();
    let marketplaces = plugin::configured_marketplaces(inv)?.into_iter().collect();
    let skills = probe::skills_list(
        inv,
        &SkillsListOptions {
            source: Some("user".to_string()),
            ..SkillsListOptions::default()
        },
    )?
    .skills
    .into_iter()
    .map(|s| s.id)
    .collect();
    Ok(HostView {
        plugins,
        marketplaces,
        skills,
    })
}

/// `omm uninstall` with no ledger. Nothing recorded what omm wrote, so the
/// host is asked: a plugin or marketplace of ours still registered means
/// an install that never reached its ledger (Gate 1: an interrupted
/// install left both behind and uninstall reported nothing to do). Without
/// `--reconcile-host` that is refused, naming both recovery paths; with it
/// the two registrations are removed from the host (`plugins remove
/// --delete-data`, `marketplace remove`) and reported — nothing else is
/// touched, because nothing else is known. With nothing on the host, only
/// omm's own audit log and locks can be left under `$OMM/`; they go, and
/// the root with them when empty.
fn uninstall_without_ledger(
    ctx: &Ctx,
    reconcile_host: bool,
    mut doc: Doc,
    mut converge: Converge,
) -> Result<ExitCode> {
    let omm_root = ctx.omm_root();
    let pid = plugin_id()?;
    let inv = ctx.invoker()?;
    let installed = plugin::installed(inv, &pid)?;
    let marketplace = plugin::configured_marketplace(inv, omm_manifest::MARKETPLACE_NAME)?;
    // A `--no-plugin` install interrupted after its first `muse skills
    // install` (Gate 1: the closed finding covered the plugin path only).
    // A store directory that is (or is reached through) a symlink is the
    // user's: the host is never asked to uninstall it — it would empty
    // whatever the link points at (Gate 1 round 5 H1). Preserved and named,
    // never a reason to refuse or to reconcile.
    let workspace_for_check = ctx.workspace().ok();
    let bases_for_check = Bases::from_roots(&ctx.roots, workspace_for_check.as_deref());
    let source = Source::resolve_with_ledger(None, &omm_root).ok();
    // Every managed-store skill the host lists, classified (H1 + M1): a
    // symlinked store dir is the user's (kept); a dir the host has
    // provenance for is `muse skills uninstall`'d; one it has no provenance
    // for that is byte-identical to the source is removed by omm, which
    // `muse skills uninstall` refuses; anything else is kept and named.
    let mut host_uninstall_ids: Vec<(String, String)> = Vec::new();
    let mut orphans: Vec<Orphan> = Vec::new();
    let mut preserved_skills: Vec<(String, String)> = Vec::new();
    for (id, why) in managed_omm_skills(ctx, inv)? {
        if let Some(reason) = unsafe_store_dir(ctx, &bases_for_check, &id)? {
            preserved_skills.push((id, reason));
            continue;
        }
        match source.as_ref() {
            Some(src) => match skills::classify_orphan_store_dir(ctx, &bases_for_check, src, &id)? {
                skills::OrphanVerdict::HostOwned => host_uninstall_ids.push((id, why)),
                skills::OrphanVerdict::Remove { dir, files } => orphans.push(Orphan {
                    reason: format!(
                        "managed-store skill `{id}` {why}, but the host has no provenance for it (an install interrupted inside `muse skills install`); identical to the source — omm removes it"
                    ),
                    id,
                    dir,
                    files,
                }),
                skills::OrphanVerdict::Keep(reason) => preserved_skills.push((id, reason)),
            },
            None if skills::store_dir_managed_by_host(ctx, &id) => {
                host_uninstall_ids.push((id, why))
            }
            None => preserved_skills.push((
                id,
                "the host has no provenance for it and no content source is available to verify it is omm's; `omm install --no-plugin --source <checkout>` reconciles it".into(),
            )),
        }
    }
    let settings = ctx.roots.settings_file();
    let settings_absent_before = !settings.exists();
    let mut reconciled: Vec<String> = Vec::new();
    let reconcilable = installed.is_some()
        || marketplace.is_some()
        || !host_uninstall_ids.is_empty()
        || !orphans.is_empty();
    if reconcilable {
        let mut present = Vec::new();
        if installed.is_some() {
            present.push(format!("plugin `{pid}` is installed"));
        }
        if let Some(m) = &marketplace {
            present.push(format!(
                "marketplace `{}` is configured ({})",
                m.name, m.source
            ));
        }
        let skill_notes: Vec<String> = host_uninstall_ids
            .iter()
            .map(|(id, why)| format!("{id} ({why})"))
            .chain(
                orphans
                    .iter()
                    .map(|o| format!("{} (no host provenance)", o.id)),
            )
            .collect();
        if !skill_notes.is_empty() {
            present.push(format!(
                "{} managed-store skill(s) from an omm checkout: {}",
                skill_notes.len(),
                skill_notes.join(", ")
            ));
        }
        if !reconcile_host {
            return Err(OmmError::Ledger(omm_ledger::LedgerError::Refused(format!(
                "no ledger at {}, but the host still holds omm's registrations: {}. `omm uninstall --reconcile-host` removes them from the host (an interrupted install left them; nothing else is touched); `omm reconcile` adopts them into a ledger instead, and `omm install` converges",
                store::ledger_path(&omm_root).display(),
                present.join("; ")
            ))));
        }
        for (id, _) in &host_uninstall_ids {
            if !ctx.dry_run {
                skills::host_uninstall(inv, id)?;
            }
            reconciled.push(format!("muse skills uninstall {id}"));
        }
        for o in &orphans {
            if !ctx.dry_run {
                skills::remove_orphan_store_dir(&o.dir, &o.files)?;
            }
            reconciled.push(format!(
                "omm removed the store directory of {} (no host provenance)",
                o.id
            ));
        }
        if installed.is_some() {
            if !ctx.dry_run {
                plugin::remove(inv, &pid, true)?;
            }
            reconciled.push(format!("muse plugins remove {pid} --delete-data"));
        }
        if let Some(m) = &marketplace {
            if !ctx.dry_run {
                let out = inv.run(&["plugins", "marketplace", "remove", &m.name, "--json"])?;
                if !out.ok() && !plugin::is_unknown_marketplace(&out) {
                    out.expect_ok()?;
                }
            }
            reconciled.push(format!("muse plugins marketplace remove {}", m.name));
        }
        converge.record_n("registrations", Action::Removed, reconciled.len());
        // `plugins remove --delete-data` writes `{"schema_version":1}` when
        // there was no settings.json: that is this run's, and goes again
        // (R5; the same rule as a ledgered `seeded` file).
        if settings_absent_before && !ctx.dry_run && settings.is_file() {
            let bytes = std::fs::read(&settings)
                .map_err(|e| OmmError::io(format!("read {}", settings.display()), e))?;
            if uninstall::is_empty_shared_document(&bytes) {
                std::fs::remove_file(&settings)
                    .map_err(|e| OmmError::io(format!("remove {}", settings.display()), e))?;
                reconciled.push(format!(
                    "removed {} (recreated empty by `plugins remove --delete-data`)",
                    settings.display()
                ));
                converge.record("files", Action::Removed);
            }
        }
    }
    // Only omm's own state may go; anything else under $OMM is the user's.
    let removable = [
        omm_root.join(omm_ledger::audit::AUDIT_FILE),
        ctx.roots.locks_dir(),
    ];
    let mut others: Vec<PathBuf> = Vec::new();
    let mut leftovers: Vec<PathBuf> = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&omm_root) {
        for e in rd.flatten() {
            let p = e.path();
            if removable.contains(&p) {
                leftovers.push(p);
            } else {
                others.push(p);
            }
        }
    }
    leftovers.sort();
    others.sort();
    let mut removed = Vec::new();
    if others.is_empty() && !ctx.dry_run {
        for p in &leftovers {
            let r = if p.is_dir() && !p.is_symlink() {
                std::fs::remove_dir_all(p)
            } else {
                std::fs::remove_file(p)
            };
            match r {
                Ok(()) => removed.push(p.clone()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(OmmError::io(format!("remove {}", p.display()), e)),
            }
        }
        let _ = std::fs::remove_dir(&omm_root);
    }
    converge.record_n(
        "omm-state",
        Action::Removed,
        if ctx.dry_run {
            leftovers.len()
        } else {
            removed.len()
        },
    );
    // The host-owned residue is named unconditionally, ledger or not.
    let workspace = ctx.workspace().ok();
    let mut bases = Bases::from_roots(&ctx.roots, workspace.as_deref());
    name_host_residue(ctx, &mut bases);
    doc.set("ledger", Value::Null)
        .set(
            "host",
            json!({
                "plugin": installed.is_some(),
                "marketplace": marketplace.is_some(),
                "skills": host_uninstall_ids.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                "orphans": orphans.iter().map(|o| o.id.clone()).collect::<Vec<_>>(),
            }),
        )
        .set("host_reconciled", json!(reconciled))
        .set(
            "preserved_symlinked_skills",
            json!(preserved_skills
                .iter()
                .map(|(id, why)| json!({"id": id, "why": why}))
                .collect::<Vec<_>>()),
        )
        .set("omm_state_removed", json!(removed))
        .set("omm_state_kept", json!(others))
        .set("preview", json!({"residue": bases.residue}));
    let json = doc.finish(&converge);
    ctx.out.emit(
        || {
            let mut text = if reconciled.is_empty() {
                format!(
                    "nothing to uninstall: no ledger at {}, and the host holds no omm plugin, marketplace or managed-store skill\n",
                    store::ledger_path(&omm_root).display()
                )
            } else {
                format!(
                    "no ledger at {}; the host's omm registrations (an interrupted install) {}:\n  {}\n",
                    store::ledger_path(&omm_root).display(),
                    if ctx.dry_run { "would be removed" } else { "were removed" },
                    reconciled.join("\n  ")
                )
            };
            if !preserved_skills.is_empty() {
                text.push_str("✓ kept (the host lists it, but its store directory is a symlink — never followed)\n");
                for (id, why) in &preserved_skills {
                    text.push_str(&format!("  {id}  ({why})\n"));
                }
            }
            if !others.is_empty() {
                text.push_str(&format!(
                    "{} kept (not omm's state): {}\n",
                    omm_root.display(),
                    others.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                ));
            } else if !leftovers.is_empty() {
                text.push_str(&format!(
                    "{} leftover(s) of an interrupted run {}: {}\n",
                    leftovers.len(),
                    if ctx.dry_run { "would be removed" } else { "removed" },
                    leftovers.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", ")
                ));
            }
            if !bases.residue.is_empty() {
                text.push_str("· residue kept (the host's)\n");
                for r in &bases.residue {
                    text.push_str(&format!("  {}\n", r.display()));
                }
            }
            text.push_str(&converge.render());
            text
        },
        || json,
    );
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// install
// ---------------------------------------------------------------------------

/// A step of `omm install` that `--skip <STEP>` leaves out of the plan
/// (Gate 1 decision A: `--skip themes` is the way out when `themes/` is
/// symlinked out of the config root by a dotfile manager).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Step {
    Rules,
    Themes,
    Profile,
    Trust,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Rules, Step::Themes, Step::Profile, Step::Trust];

    pub fn name(self) -> &'static str {
        match self {
            Step::Rules => "rules",
            Step::Themes => "themes",
            Step::Profile => "profile",
            Step::Trust => "trust",
        }
    }

    pub fn parse(s: &str) -> Option<Step> {
        Step::ALL.iter().copied().find(|st| st.name() == s)
    }

    /// `--skip <name>`, as the report spells it.
    pub fn flag(self) -> String {
        format!("--skip {}", self.name())
    }
}

/// The `--skip` list as steps; an unknown name is refused before anything
/// runs.
pub fn parse_skips(raw: &[String]) -> Result<BTreeSet<Step>> {
    let mut out = BTreeSet::new();
    for name in raw {
        match Step::parse(name.trim()) {
            Some(st) => {
                out.insert(st);
            }
            None => {
                return Err(OmmError::Usage(format!(
                    "--skip {name:?}: not a step of `omm install`; one of {}",
                    Step::ALL
                        .iter()
                        .map(|s| s.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )))
            }
        }
    }
    Ok(out)
}

/// One file the install will write (or find current): its resolved
/// realpath and the containment verdict, decided before the first host
/// mutation (Gate 1 decision A).
#[derive(Clone, Debug)]
pub struct PlannedFile {
    /// `rules` / `themes` / `skills`.
    pub step: &'static str,
    pub key: EntryKey,
    /// The contained realpath (lexical under the base root when the file or
    /// the root does not exist yet).
    pub abs: PathBuf,
    pub bytes: u64,
    /// `Err(reason)` when the path resolves outside its base (R4).
    pub containment: std::result::Result<(), String>,
    /// An ancestor inside the base that is a symlink: the step leaves the
    /// file alone (R2 sentinel), never a refusal.
    pub via_symlink: Option<PathBuf>,
    /// `access(2)` on the file or its deepest existing ancestor; `None`
    /// when unknown.
    pub writable: Option<bool>,
}

impl PlannedFile {
    fn to_json(&self) -> Value {
        json!({
            "step": self.step,
            "key": self.key.to_string(),
            "path": self.abs,
            "bytes": self.bytes,
            "contained": self.containment.is_ok(),
            "refused": self.containment.as_ref().err(),
            "via_symlink": self.via_symlink,
            "writable": self.writable,
        })
    }
}

/// The complete plan of one `omm install`, built and validated before the
/// first host mutation: every file write with its resolved realpath and
/// containment result, every settings key, every host mutation, the trust
/// entry, the host's reachability and the free space (Gate 1 decision A —
/// round 4: a `themes/` symlinked out of the base aborted the install
/// half-way, after the plugin, marketplace, AGENTS.md and settings.json
/// were already applied).
#[derive(Debug)]
pub struct InstallPlan {
    source: Source,
    pid: String,
    profile: String,
    workspace: Option<PathBuf>,
    workspace_is_git: bool,
    skip: BTreeSet<Step>,
    no_plugin: bool,
    settings_existed_before: bool,
    /// `settings.json` as found (bytes and mode) before any host verb.
    settings_before: Option<Original>,
    dropped_keys: Vec<String>,
    pub files: Vec<PlannedFile>,
    pub settings_keys: Vec<String>,
    pub host_mutations: Vec<String>,
    pub trust_project: Option<String>,
    pub host_version: String,
    pub planned_bytes: u64,
    pub free_bytes: Option<u64>,
}

impl InstallPlan {
    fn to_json(&self) -> Value {
        json!({
            "mode": if self.no_plugin { "skills" } else { "plugin" },
            "skipped_steps": self.skip.iter().map(|s| s.name()).collect::<Vec<_>>(),
            "files": self.files.iter().map(PlannedFile::to_json).collect::<Vec<_>>(),
            "settings_keys": self.settings_keys,
            "host_mutations": self.host_mutations,
            "trust": self.trust_project,
            "host_version": self.host_version,
            "planned_bytes": self.planned_bytes,
            "free_bytes": self.free_bytes,
        })
    }

    /// The plan's own lines for the terminal.
    fn lines(&self) -> Vec<String> {
        let mut out = vec![format!(
            "plan: {} file(s) ({} B), {} settings key(s), {} host mutation(s){}{}",
            self.files.len(),
            self.planned_bytes,
            self.settings_keys.len(),
            self.host_mutations.len(),
            match &self.trust_project {
                Some(p) => format!(", trust {p}"),
                None => String::new(),
            },
            if self.skip.is_empty() {
                String::new()
            } else {
                format!(
                    "; skipped: {}",
                    self.skip
                        .iter()
                        .map(|s| s.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        )];
        for f in self.files.iter().filter(|f| f.via_symlink.is_some()) {
            out.push(format!(
                "  {} ({}): an ancestor is a symlink; left alone (R2 sentinel)",
                f.abs.display(),
                f.step
            ));
        }
        out
    }
}

/// Resolve one planned path the way the install's session will (a base
/// root that does not exist yet resolves lexically, as `Session::probe`).
fn planned_file(bases: &Bases, step: &'static str, rel: RelPath, bytes: u64) -> PlannedFile {
    let base = Base::MuseConfig;
    let key = EntryKey {
        base,
        path: rel.clone(),
    };
    let root = bases.muse_config.clone();
    let (abs, containment, via_symlink) = if !root.exists() {
        (rel.under(&root), Ok(()), None)
    } else {
        match bases.resolve(base, &rel) {
            Ok(r) => (r.path, Ok(()), r.via_symlink),
            Err(e) => (rel.under(&root), Err(e.to_string()), None),
        }
    };
    let writable = fsx::is_writable(&abs);
    PlannedFile {
        step,
        key,
        abs,
        bytes,
        containment,
        via_symlink,
        writable,
    }
}

/// Build the plan: read-only — the source, the ledger (read without the
/// quarantine side effect; the session quarantines a corrupt one later,
/// loudly), the roots, every path, the host's version.
fn build_plan(ctx: &Ctx, args: &InstallArgs) -> Result<InstallPlan> {
    let (source, pid) = checked_source(ctx, args.source.as_deref())?;
    let inv = ctx.invoker()?;
    let skip = parse_skips(&args.skip)?;
    refuse_host_config_shape(ctx)?;
    refuse_mcp_collision(ctx)?;
    let dropped_keys = if args.no_plugin {
        Vec::new()
    } else {
        refuse_untyped_settings_keys(ctx, args.drop_unknown_settings_keys)?
    };
    let settings_file = ctx.roots.settings_file();
    let settings_existed_before = settings_file.exists();
    let settings_before = Original::read(&settings_file)?;
    let workspace = resolve_workspace(ctx, args.workspace.as_deref())?;
    if let Some(l) = read_ledger(ctx)? {
        let (plugin_reg, _) = plugin::registrations(&l, &pid);
        let skill_ids = skills::ledgered_ids(ctx, &l);
        if args.no_plugin && plugin_reg.is_some() {
            return Err(OmmError::Usage(
                "the ledger records a plugin-bundle install; run `omm uninstall` before installing with --no-plugin".into(),
            ));
        }
        if !args.no_plugin && !skill_ids.is_empty() {
            return Err(OmmError::Usage(format!(
                "the ledger records a --no-plugin install ({} skills); rerun with --no-plugin, or `omm uninstall` first",
                skill_ids.len()
            )));
        }
    }
    let profile = default_profile_id(&source, args.profile.as_deref())?;
    if !args.no_plugin {
        source.require_built(&pid)?;
    }
    // The host must be reachable before anything is planned against it —
    // and able to load its config: `--version` answers even over a
    // malformed `settings.json` / `trust.json` (host-reality.md "malformed
    // settings.json": lazy), so the plan's probe is `skills list --json`,
    // which loads both the way every install-time verb does (Gate 1 round
    // 5 M3). The shape mirror above names the common cases precisely; a
    // typed error only the host's loader sees (`"tui": 5`) is refused here
    // with the host's own reason, nothing written.
    let host_version = probe::version(inv)?.build;
    refuse_unloadable_host_config(ctx, inv)?;

    let bases = Bases::from_roots(&ctx.roots, workspace.as_deref());
    let mut files = Vec::new();
    if !skip.contains(&Step::Rules) {
        if let Some((rel, bytes)) = files::plan_rules(ctx, &source)? {
            files.push(planned_file(&bases, "rules", rel, bytes));
        }
    }
    if !skip.contains(&Step::Themes) {
        for (rel, bytes) in files::plan_themes(ctx, &source)? {
            files.push(planned_file(&bases, "themes", rel, bytes));
        }
    }
    let mut host_mutations = Vec::new();
    if args.no_plugin {
        for (rel, bytes) in skills::plan_files(ctx, &source)? {
            files.push(planned_file(&bases, "skills", rel, bytes));
        }
        for skill in &source.content.skills {
            host_mutations.push(format!(
                "muse skills install {} --scope user",
                source.content.root.join(&skill.dir).display()
            ));
        }
    } else {
        let name = omm_manifest::MARKETPLACE_NAME;
        host_mutations.push(format!(
            "muse plugins marketplace add {name} {} (or `marketplace update {name}`)",
            source.marketplace_root().display()
        ));
        host_mutations.push(format!("muse plugins install {pid}@{name}"));
        host_mutations.push(format!(
            "muse plugins approve plugin:{pid}:<kind>:<id> for every runtime capability the package declares, then `plugins inspect` verifies {}",
            omm_host::host_reality::RUNTIME_CAPABILITY_ACTIVE_STATE
        ));
    }
    let settings_keys = if skip.contains(&Step::Profile) {
        Vec::new()
    } else {
        source
            .content
            .profiles
            .iter()
            .find(|p| p.asset.id == profile)
            .map(|p| {
                let mut leaves = Vec::new();
                files::leaves(&[], &Value::Object(p.settings.clone()), &mut leaves);
                leaves.into_iter().map(|(path, _)| path.join(".")).collect()
            })
            .unwrap_or_default()
    };
    let workspace_is_git = workspace
        .as_deref()
        .map(session::is_git_workspace)
        .unwrap_or(false);
    let trust_project = match (&workspace, skip.contains(&Step::Trust)) {
        (Some(ws), false) if workspace_is_git => omm_host::trust::TrustStore::key_for(ws).ok(),
        _ => None,
    };
    let planned_bytes = files.iter().map(|f| f.bytes).sum();
    let free_bytes = fsx::free_bytes(&ctx.roots.muse_config());
    Ok(InstallPlan {
        source,
        pid,
        profile,
        workspace,
        workspace_is_git,
        skip,
        no_plugin: args.no_plugin,
        settings_existed_before,
        settings_before,
        dropped_keys,
        files,
        settings_keys,
        host_mutations,
        trust_project,
        host_version,
        planned_bytes,
        free_bytes,
    })
}

/// The refusal text of a plan whose paths resolve outside their base: the
/// exact paths, and the way out per step.
pub fn containment_refusal(config_root: &Path, escaped: &[&PlannedFile]) -> String {
    let mut lines = vec![format!(
        "omm install refused before its first write: {} planned path(s) resolve outside the base (R4):",
        escaped.len()
    )];
    for f in escaped {
        lines.push(format!(
            "  {} ({} step): {}",
            f.abs.display(),
            f.step,
            f.containment
                .as_ref()
                .err()
                .map(String::as_str)
                .unwrap_or("")
        ));
    }
    let steps: BTreeSet<&str> = escaped.iter().map(|f| f.step).collect();
    for step in steps {
        lines.push(match step {
            "themes" => format!(
                "the way out: `omm install --skip themes` leaves the themes out, or `{}=<dir>` places them in a directory inside {} (the host's theme picker reads only {}/themes, so they are then staged for you, not active)",
                files::THEMES_DIR_ENV,
                config_root.display(),
                config_root.display()
            ),
            "rules" => "the way out: `omm install --skip rules` leaves AGENTS.md out".to_string(),
            "skills" => format!(
                "the way out: a --no-plugin install writes the managed store through that symlink; point {}/skills back inside the config root, or install the plugin bundle (`omm install`)",
                config_root.display()
            ),
            other => format!("the way out: `omm install --skip {other}`"),
        });
    }
    lines.push("nothing was written.".to_string());
    lines.join("\n")
}

/// Validate the whole plan before the first host mutation: containment
/// (every planned realpath inside its base), writability, free space —
/// the host's reachability was proven while building it.
fn validate_plan(ctx: &Ctx, plan: &InstallPlan) -> Result<()> {
    let escaped: Vec<&PlannedFile> = plan
        .files
        .iter()
        .filter(|f| f.containment.is_err())
        .collect();
    if !escaped.is_empty() {
        return Err(OmmError::Usage(containment_refusal(
            &ctx.roots.muse_config(),
            &escaped,
        )));
    }
    let unwritable: Vec<&PlannedFile> = plan
        .files
        .iter()
        .filter(|f| f.writable == Some(false) && f.via_symlink.is_none())
        .collect();
    if !unwritable.is_empty() {
        return Err(OmmError::Usage(format!(
            "omm install refused before its first write: {} planned path(s) are not writable by this user: {}; nothing was written",
            unwritable.len(),
            unwritable
                .iter()
                .map(|f| format!("{} ({} step)", f.abs.display(), f.step))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if let Some(free) = plan.free_bytes {
        if free < plan.planned_bytes {
            return Err(OmmError::Usage(format!(
                "omm install refused before its first write: {} B free under {}, {} B planned; nothing was written",
                free,
                ctx.roots.muse_config().display(),
                plan.planned_bytes
            )));
        }
    }
    Ok(())
}

/// `omm install`: the plan is built and validated whole before the first
/// host mutation ([`InstallPlan`]), then executed — the bundle (marketplace
/// add → list --available → install → approve every capability → verify
/// `trusted_enabled` → skills cross-check) or, with `--no-plugin`, `muse
/// skills install` per skill; both: `AGENTS.md`, the default profile,
/// themes, trust for the workspace, each step skippable with `--skip`.
/// Idempotent; ledger + audit; prints the converge summary and the catalog
/// budget consumed.
pub fn install(ctx: &Ctx, args: &InstallArgs) -> Result<ExitCode> {
    ctx.require_consent("install")?;
    let plan = build_plan(ctx, args)?;
    validate_plan(ctx, &plan)?;
    execute_plan(ctx, args, plan)
}

fn execute_plan(ctx: &Ctx, args: &InstallArgs, plan: InstallPlan) -> Result<ExitCode> {
    let inv = ctx.invoker()?;
    let source = &plan.source;
    let pid = plan.pid.as_str();
    let mut session = Session::open(ctx, plan.workspace.as_deref())?;
    session.warn_quarantine(ctx);
    // The shared files as found, before the first host mutation can rewrite
    // one (Gate 1): their bytes and modes go into the ledger now.
    let recorded = files::record_shared_originals(ctx, inv, &mut session, &source.version)?;

    let mut converge = Converge::new(ctx.dry_run);
    let mut doc = Doc::new("install", ctx);
    let mut lines: Vec<String> = Vec::new();
    doc.set(
        "mode",
        json!(if plan.no_plugin { "skills" } else { "plugin" }),
    )
    .set("source", source_json(source))
    .set("plan", plan.to_json())
    .set(
        "skipped_steps",
        json!(plan.skip.iter().map(|s| s.name()).collect::<Vec<_>>()),
    )
    .set("shared_recorded", json!(recorded))
    .set("dropped_unknown_settings_keys", json!(plan.dropped_keys));
    lines.push(format!(
        "source: {} ({}, version {})",
        source.repo.root.display(),
        source.origin.label(),
        source.version
    ));
    lines.extend(plan.lines());
    let mut exit = ExitCode::SUCCESS;

    if plan.no_plugin {
        let r = skills::converge_install(ctx, inv, &mut session, source, &mut converge)?;
        lines.extend(r.lines());
        doc.set("skills", r.to_json());
    } else {
        let r = plugin::converge_install(
            ctx,
            inv,
            &mut session,
            source,
            args.reinstall,
            plan.settings_existed_before,
            &mut converge,
        )?;
        lines.extend(r.lines(pid, omm_manifest::MARKETPLACE_NAME));
        if !r.skills_missing.is_empty() {
            exit = ExitCode::from(1);
        }
        doc.set("bundle", r.to_json());
    }
    // After the last host verb that can rewrite settings.json: the untyped
    // keys the rewrite dropped come back, and the recorded mode with them
    // (Gate 1 decision F).
    match &plan.settings_before {
        Some(before) => {
            let back = files::merge_untyped_back(ctx, inv, &session, before, ACTION_INSTALL)?;
            if let Some(line) = back.line() {
                lines.push(line);
            }
            converge.record_n(files::CAT_SETTINGS, Action::Updated, back.keys.len());
            doc.set("settings_after_host", back.to_json());
        }
        None => {
            doc.set("settings_after_host", Value::Null);
        }
    }

    if plan.skip.contains(&Step::Rules) {
        converge.record(files::CAT_RULES, Action::Skipped);
        lines.push(format!("rules: step skipped ({})", Step::Rules.flag()));
        doc.set("rules", json!({"step_skipped": Step::Rules.flag()}));
    } else {
        let rules = files::converge_rules(
            ctx,
            inv,
            &mut session,
            source,
            Mode::Install,
            &source.version,
            &mut converge,
        )?;
        lines.push(rules.line());
        doc.set("rules", rules.to_json());
    }
    let themes = if plan.skip.contains(&Step::Themes) {
        converge.record(files::CAT_THEMES, Action::Skipped);
        files::ThemesOutcome::skipped_step(&Step::Themes.flag())
    } else {
        files::install_themes(ctx, inv, &mut session, source, &mut converge)?
    };
    lines.push(themes.line());
    doc.set("themes", themes.to_json());
    if plan.skip.contains(&Step::Profile) {
        converge.record(files::CAT_SETTINGS, Action::Skipped);
        lines.push(format!("settings: step skipped ({})", Step::Profile.flag()));
        doc.set("settings", json!({"step_skipped": Step::Profile.flag()}));
    } else {
        let prof = files::apply_profile(
            ctx,
            inv,
            &mut session,
            source,
            &plan.profile,
            plan.settings_existed_before,
            &mut converge,
        )?;
        lines.push(prof.line());
        doc.set("settings", prof.to_json());
    }
    if plan.skip.contains(&Step::Trust) {
        converge.record(files::CAT_TRUST, Action::Skipped);
        lines.push(format!("trust: step skipped ({})", Step::Trust.flag()));
        doc.set("trust", json!({"step_skipped": Step::Trust.flag()}));
    } else {
        match &plan.workspace {
            Some(ws) if plan.workspace_is_git => {
                let t = files::trust_workspace(ctx, inv, &mut session, ws, &mut converge)?;
                lines.push(t.line());
                doc.set("trust", t.to_json());
            }
            Some(ws) => {
                lines.push(format!(
                    "trust: {} is not a git checkout; no trust entry (`omm trust <dir>` adds one)",
                    ws.display()
                ));
                doc.set("trust", Value::Null);
            }
            None => {
                doc.set("trust", Value::Null);
            }
        }
    }
    session.save()?;

    let budget = source.budget();
    doc.set("budget", budget.to_json());
    let json = doc.finish(&converge);
    ctx.out.emit(
        || {
            let mut text = lines.join("\n");
            text.push('\n');
            text.push_str(&converge.render());
            text.push('\n');
            text.push_str(&budget.render());
            text
        },
        || json,
    );
    Ok(exit)
}

// ---------------------------------------------------------------------------
// uninstall
// ---------------------------------------------------------------------------

/// `omm uninstall`: the R4 traversal with the two-section preview; host
/// registrations undone (`plugins remove --delete-data`, `marketplace
/// remove`, settings keys and trust entries back to their priors), files
/// deepest-first, `$OMM/` state, the ledger last. Kept residue is named.
/// The host is asked first what it holds of omm's beyond the ledger
/// ([`host_beyond_ledger`]): that is named under `!` and, without
/// `--reconcile-host`, refused before anything is applied; with the flag
/// it is undone with the rest and reported as `host_reconciled`.
pub fn uninstall(ctx: &Ctx, args: &UninstallArgs) -> Result<ExitCode> {
    ctx.require_consent("uninstall")?;
    let omm_root = ctx.omm_root();
    let loaded = store::load(&omm_root)?;
    if let Some(q) = loaded.quarantined() {
        ctx.out.warn(q.message());
    }
    let mut converge = Converge::new(ctx.dry_run);
    let mut doc = Doc::new("uninstall", ctx);
    doc.set("force", json!(args.force));
    let Some(ledger) = loaded.into_ledger() else {
        return uninstall_without_ledger(ctx, args.reconcile_host, doc, converge);
    };
    let pid = plugin_id()?;
    let inv = ctx.invoker()?;
    let source = Source::resolve_with_ledger(None, &omm_root).ok();
    // The host's listing verbs (`plugins list`, `skills list`) exit 1 while
    // `settings.json` or `trust.json` is malformed the host's way; the undo
    // verbs (`plugins remove`, `marketplace remove`) still work. Degrade to
    // a blind undo instead of wedging (Gate 1 round 5 M3): plan every
    // registration, tolerate the host's already-gone answers, and warn.
    let (view, blind) = match host_view(ctx, inv) {
        Ok(v) => (Some(v), false),
        Err(e) => {
            ctx.out.warn(format!(
                "the host's config cannot be listed ({e}); omm's registrations are undone blind (already-gone answers tolerated), and what the host holds beyond the ledger cannot be seen. Repair settings.json / trust.json and rerun to finish cleanly"
            ));
            (None, true)
        }
    };
    let beyond = if blind {
        Beyond::default()
    } else {
        host_beyond_ledger(ctx, inv, &ledger, &pid, source.as_ref())?
    };
    let extras = &beyond.extras;
    if args.reconcile_host && !blind && !beyond.any_reconcilable() {
        ctx.out
            .note("--reconcile-host: the host holds nothing of omm's beyond the ledger; the normal traversal runs");
    }
    let workspace = ctx.workspace().ok();
    let mut bases = Bases::from_roots(&ctx.roots, workspace.as_deref());
    name_host_residue(ctx, &mut bases);
    let mut plan = uninstall::plan(&ledger, &bases, uninstall_options(ctx, args.force, view))?;
    let mut reconciled: Vec<String> = Vec::new();
    if args.reconcile_host {
        splice_host_extras(&mut plan.host_steps, extras);
        reconciled.extend(extras.iter().map(|x| x.step.label()));
        reconciled.extend(beyond.orphans.iter().map(|o| {
            format!(
                "omm removed the store directory of {} (no host provenance)",
                o.id
            )
        }));
        if extras
            .iter()
            .any(|x| matches!(x.step, UndoStep::PluginRemove { .. }))
        {
            name_recreated_settings(ctx, &bases, &mut plan)?;
        }
    }
    let unregistered = unregistered_shared_state(ctx, &ledger, workspace.as_deref());
    // A personal rules file carrying omm's managed block that the ledger
    // does not list (Gate 1 round 5 M2 net; the install now ledgers before
    // it writes, so this is only a legacy or tampered state): named, left
    // in place — the traversal cannot know what to strip without the entry.
    let unlisted_rules = unlisted_managed_rules(ctx, &ledger);
    let mut preview = plan.render();
    if beyond.any_reconcilable() {
        preview.push_str(&format!(
            "! beyond the ledger (host has it, ledger does not) ({}){}\n",
            beyond.reconcilable_len(),
            if args.reconcile_host {
                " — removed with the rest (--reconcile-host)"
            } else {
                " — not touched; `omm uninstall --reconcile-host` removes them too"
            }
        ));
        for x in extras {
            preview.push_str(&format!("  {}  ({})\n", x.step.label(), x.reason));
        }
        for o in &beyond.orphans {
            preview.push_str(&format!("  {}  ({})\n", o.dir.display(), o.reason));
        }
    }
    if !beyond.kept.is_empty() {
        preview.push_str("✓ kept (the host lists it, but omm never wrote it — left in place)\n");
        for (id, why) in &beyond.kept {
            preview.push_str(&format!("  {id}  ({why})\n"));
        }
    }
    if !unregistered.is_empty() {
        preview.push_str("✓ kept (current, but never registered — prior unknown)\n");
        for u in &unregistered {
            preview.push_str(&format!("  {u}\n"));
        }
    }
    if let Some(p) = &unlisted_rules {
        preview.push_str(&format!(
            "! present but unlisted: {p} carries omm's managed block with no ledger entry — run `omm reconcile` then `omm install` to adopt it (or remove the block by hand); left in place\n"
        ));
    }
    doc.set("unlisted_managed_rules", json!(unlisted_rules));
    doc.set("host_reconciled", json!(reconciled));
    doc.set(
        "beyond_ledger_orphans",
        json!(beyond
            .orphans
            .iter()
            .map(|o| json!({"id": o.id, "dir": o.dir, "reason": o.reason}))
            .collect::<Vec<_>>()),
    );
    doc.set(
        "beyond_ledger_kept",
        json!(beyond
            .kept
            .iter()
            .map(|(id, why)| json!({"id": id, "why": why}))
            .collect::<Vec<_>>()),
    );
    doc.set(
        "preview",
        json!({
            "host_extra": extras.iter().map(|x| json!({"step": x.step.label(), "reason": x.reason})).collect::<Vec<_>>(),
            "remove": plan.remove.iter().map(|s| json!({"path": s.abs, "forced": s.forced})).collect::<Vec<_>>(),
            "rules": plan.rules.iter().map(|r| json!({"path": r.abs, "action": r.label()})).collect::<Vec<_>>(),
            "omm_state_remove": plan.omm_state.remove,
            "created_shared_files": plan.created.iter().map(|c| json!({"path": c.abs, "absent_at_plan": c.absent_at_plan})).collect::<Vec<_>>(),
            "shared": plan.shared.iter().map(|s| json!({"path": s.abs, "action": s.label(), "mode": s.original.mode.map(|m| format!("{m:04o}")), "untyped_now": s.untyped_now.iter().map(|(p, _)| p.join(".")).collect::<Vec<_>>()})).collect::<Vec<_>>(),
            "ledger": plan.omm_state.ledger,
            "host_steps": plan.host_steps.iter().map(|s| json!(s.label())).collect::<Vec<_>>(),
            "dropped": plan.dropped.iter().map(|d| json!({"step": d.step.label(), "reason": d.reason})).collect::<Vec<_>>(),
            "preserve": plan.preserve.iter().map(|p| json!({"key": p.key.to_string(), "path": p.abs, "reason": p.reason})).collect::<Vec<_>>(),
            "unregistered": unregistered,
            "omm_state_keep": plan.omm_state.keep,
            "missing": plan.missing.iter().map(|m| json!(m.abs)).collect::<Vec<_>>(),
            "residue": plan.residue,
            "refused": plan.refused.iter().map(|r| json!({"key": r.key.to_string(), "reason": r.reason})).collect::<Vec<_>>(),
        }),
    );
    if !plan.refused.is_empty() {
        ctx.out.line(&preview);
        return Err(OmmError::Ledger(omm_ledger::LedgerError::Refused(format!(
            "{} ledger entr{} resolve onto a refused root (/, $HOME or a base root); nothing was applied",
            plan.refused.len(),
            if plan.refused.len() == 1 { "y" } else { "ies" }
        ))));
    }
    if beyond.any_reconcilable() && !args.reconcile_host {
        ctx.out.line(&preview);
        let n = beyond.reconcilable_len();
        let mut reasons: Vec<String> = extras.iter().map(|x| x.reason.clone()).collect();
        reasons.extend(beyond.orphans.iter().map(|o| o.reason.clone()));
        return Err(OmmError::Ledger(omm_ledger::LedgerError::Refused(format!(
            "the host holds {} of omm's the ledger at {} does not list (an interrupted install): {}. `omm uninstall --reconcile-host` removes {} together with what the ledger records; `omm reconcile` (the plugin and marketplace) or `omm install --no-plugin` (an identical managed skill) adopts {} into the ledger instead; nothing was applied",
            if n == 1 { "1 thing".to_string() } else { format!("{n} things") },
            store::ledger_path(&omm_root).display(),
            reasons.join("; "),
            if n == 1 { "it" } else { "them" },
            if n == 1 { "it" } else { "them" }
        ))));
    }
    let mut recording = RecordingUndoer::default();
    let mut host: Option<HostUndoer> = None;
    if !ctx.dry_run && !plan.host_steps.is_empty() {
        host = Some(HostUndoer::new(ctx.invoker()?.clone(), ctx.roots.clone()));
    }
    let undoer: &mut dyn Undoer = match host.as_mut() {
        Some(h) => h,
        None => &mut recording,
    };
    let report = uninstall::apply(
        &plan,
        ledger,
        &bases,
        undoer,
        &uninstall::ApplyOptions {
            omm_version: OMM_VERSION.into(),
            dry_run: ctx.dry_run,
        },
    )?;
    // Store directories the host has no provenance for (an interrupted
    // `muse skills install`): omm removes them itself, `muse skills
    // uninstall` refuses them (Gate 1 round 5 M1). Only with the flag, only
    // when the ledgered plan applied cleanly.
    let mut orphans_removed: Vec<String> = Vec::new();
    if args.reconcile_host && !ctx.dry_run && report.complete() {
        for o in &beyond.orphans {
            let removed = skills::remove_orphan_store_dir(&o.dir, &o.files)?;
            orphans_removed.push(o.id.clone());
            converge.record_n("skills", Action::Removed, removed.len());
        }
    }
    doc.set("beyond_ledger_orphans_removed", json!(orphans_removed));
    converge.record_n("files", Action::Removed, report.removed);
    converge.record_n(
        "files",
        Action::Updated,
        report.rules_rewritten + report.shared_restored,
    );
    converge.record_n(
        "files",
        Action::Skipped,
        report.preserved + report.missing + report.shared_kept,
    );
    converge.record_n(
        "registrations",
        Action::Removed,
        report.registrations_undone + report.registrations_dropped,
    );
    converge.record_n(
        "omm-state",
        Action::Removed,
        report.omm_state_removed + usize::from(report.ledger_removed),
    );
    if ctx.dry_run {
        converge.record_n("omm-state", Action::Removed, 1);
    }
    doc.set(
        "report",
        json!({
            "removed": report.removed,
            "rules_rewritten": report.rules_rewritten,
            "shared_restored": report.shared_restored,
            "shared_kept": report.shared_kept,
            "preserved": report.preserved,
            "missing": report.missing,
            "registrations_undone": report.registrations_undone,
            "registrations_dropped": report.registrations_dropped,
            "registrations_kept": report.registrations_kept,
            "dirs_removed": report.dirs_removed,
            "tmps_removed": report.tmps_removed,
            "omm_state_removed": report.omm_state_removed,
            "ledger_removed": report.ledger_removed,
            "errors": report.errors,
        }),
    );
    for e in &report.errors {
        ctx.out.warn(e);
    }
    let complete = report.complete();
    let json = doc.finish(&converge);
    ctx.out.emit(
        || {
            let mut text = preview.trim_end().to_string();
            text.push('\n');
            if ctx.dry_run {
                text.push_str("dry run: nothing was removed\n");
            } else if complete {
                text.push_str("uninstalled; the ledger was removed last\n");
            } else {
                text.push_str(&format!(
                    "{} step(s) failed; the ledger keeps what is left so a rerun continues (a registration the host no longer holds is dropped on the rerun; `omm reconcile` drops it too)\n",
                    report.errors.len()
                ));
            }
            text.push_str(&converge.render());
            text
        },
        || json,
    );
    Ok(if complete {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

// ---------------------------------------------------------------------------
// update
// ---------------------------------------------------------------------------

/// `omm update`: snapshot → reconcile the content files (R3) → the §6.4
/// plugin transaction (or the per-skill merge of a `--no-plugin` install)
/// → report every staged conflict by path.
pub fn update(ctx: &Ctx, args: &UpdateArgs) -> Result<ExitCode> {
    ctx.require_consent("update")?;
    let (source, pid) = checked_source(ctx, args.source.as_deref())?;
    let inv = ctx.invoker()?;
    let workspace = ctx.workspace().ok();
    let mut session = Session::open(ctx, workspace.as_deref())?;
    session.warn_quarantine(ctx);
    let Some(ledger) = session.ledger.clone() else {
        return Err(OmmError::Usage(format!(
            "nothing to update: no ledger at {}; run `omm install`",
            store::ledger_path(&session.omm_root).display()
        )));
    };
    refuse_host_config_shape(ctx)?;
    refuse_unloadable_host_config(ctx, inv)?;
    refuse_mcp_collision(ctx)?;
    let version = source.version.clone();
    omm_ledger::reconcile::check_version_label(&version)?;
    // settings.json before the first host mutation: the host's rewrites
    // (`plugins remove / install / approve`, `skills uninstall`) drop the
    // keys it does not type and land their own mode; both come back after
    // the last host step (Gate 1).
    let settings_before = Original::read(&ctx.roots.settings_file())?;

    let mut converge = Converge::new(ctx.dry_run);
    let mut doc = Doc::new("update", ctx);
    let mut lines: Vec<String> = Vec::new();
    doc.set("source", source_json(&source))
        .set("version", json!(version));
    lines.push(format!(
        "source: {} ({}, version {})",
        source.repo.root.display(),
        source.origin.label(),
        source.version
    ));

    // 1. Snapshot every ledgered file (ARCHITECTURE.md §4), rolling keep-5.
    if !ctx.dry_run {
        let snap = snapshot::take(&ledger, &session.bases, &session.snapshots_dir())?;
        session.record(&Event::new(ACTION_SNAPSHOT).note(format!(
            "{} ({} files, {} skipped)",
            snap.dir.display(),
            snap.copied.len(),
            snap.skipped.len()
        )))?;
        converge.record_n("snapshot", Action::BackedUp, snap.copied.len());
        lines.push(format!(
            "snapshot: {} ({} file(s) copied)",
            snap.dir.display(),
            snap.copied.len()
        ));
        doc.set(
            "snapshot",
            json!({"dir": snap.dir, "copied": snap.copied.len(), "skipped": snap.skipped.len()}),
        );
    } else {
        lines.push("snapshot: skipped (dry run)".into());
    }

    let mut staged: Vec<Staged> = Vec::new();
    // 2. The content files under R3.
    let rules = files::converge_rules(
        ctx,
        inv,
        &mut session,
        &source,
        Mode::Update,
        &version,
        &mut converge,
    )?;
    lines.push(rules.line());
    staged.extend(rules.staged.iter().cloned());
    doc.set("rules", rules.to_json());
    let themes = files::update_themes(ctx, inv, &mut session, &source, &version, &mut converge)?;
    lines.push(themes.line());
    staged.extend(themes.staged.iter().cloned());
    doc.set("themes", themes.to_json());

    // 3. The plugin transaction, or the skills of a --no-plugin install.
    let (plugin_reg, _) = plugin::registrations(&ledger, &pid);
    let skill_ids = skills::ledgered_ids(ctx, &ledger);
    if plugin_reg.is_some() {
        let r = plugin::converge_update(
            ctx,
            inv,
            &mut session,
            &source,
            args.reinstall,
            &mut converge,
        )?;
        lines.extend(r.lines(&pid, omm_manifest::MARKETPLACE_NAME));
        doc.set("bundle", r.to_json());
    } else if !skill_ids.is_empty() {
        let r = skills::converge_update(ctx, inv, &mut session, &source, &version, &mut converge)?;
        lines.extend(r.lines());
        staged.extend(r.staged.iter().cloned());
        doc.set("skills", r.to_json());
    } else {
        lines.push("plugin: the ledger records neither a bundle nor managed skills; only the content files were reconciled".into());
    }
    if let Some(before) = &settings_before {
        let back = files::merge_untyped_back(ctx, inv, &session, before, ACTION_UPDATE)?;
        if let Some(line) = back.line() {
            lines.push(line);
        }
        converge.record_n("settings", Action::Updated, back.keys.len());
        doc.set("settings", back.to_json());
    }
    session.save()?;

    // 4. One report for every conflict of this version.
    if !staged.is_empty() && !ctx.dry_run {
        let dir = session.omm_root.join(UPDATES_DIR).join(&version);
        fsx::create_dir_all(&dir)?;
        let path = dir.join(REPORT_FILE);
        let report = StageReport {
            schema_version: 1,
            version: version.clone(),
            ts: omm_ledger::audit::now_ts(),
            staged: staged.clone(),
        };
        let mut bytes = serde_json::to_vec_pretty(&report)
            .map_err(|e| OmmError::io("render report.json", std::io::Error::other(e)))?;
        bytes.push(b'\n');
        let realpath = fsx::realpath_for_write(&path)?;
        fsx::write_atomic(&realpath, &bytes)?;
        lines.push(format!(
            "staged conflicts: {} (report {})",
            staged.len(),
            realpath.display()
        ));
    }
    for s in &staged {
        lines.push(format!(
            "  STAGED {}:{} -> {} ({}); on disk untouched",
            s.base,
            s.path,
            s.staged_to.display(),
            s.reason
        ));
    }
    doc.set(
        "staged",
        Value::Array(staged.iter().map(staged_json).collect()),
    );
    let budget = source.budget();
    doc.set("budget", budget.to_json());
    let json = doc.finish(&converge);
    ctx.out.emit(
        || {
            let mut text = lines.join("\n");
            text.push('\n');
            text.push_str(&converge.render());
            text.push('\n');
            text.push_str(&budget.render());
            text
        },
        || json,
    );
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// reconcile (D13's fix)
// ---------------------------------------------------------------------------

/// `omm reconcile`: re-check every ledger entry and registration against
/// the disk and the host — drop what vanished, adopt what the host holds
/// but the ledger does not, report what was edited (kept as is).
pub fn reconcile(ctx: &Ctx, args: &ReconcileArgs) -> Result<ExitCode> {
    let _ = args;
    ctx.require_consent("reconcile")?;
    let pid = plugin_id()?;
    let inv = ctx.invoker()?;
    let workspace = ctx.workspace().ok();
    let mut session = Session::open(ctx, workspace.as_deref())?;
    session.warn_quarantine(ctx);
    let mut converge = Converge::new(ctx.dry_run);
    let mut doc = Doc::new("reconcile", ctx);
    let mut notes: Vec<String> = Vec::new();

    let installed = plugin::installed(inv, &pid)?;
    // Managed-store skills from an omm checkout the host holds (Gate 1
    // decision E: with no ledger, or one that no longer lists them, they
    // are adopted below when identical to the source).
    let managed = managed_omm_skills(ctx, inv)?;
    if session.ledger.is_none() && installed.is_none() && managed.is_empty() {
        doc.set(
            "notes",
            json!(["no ledger, no installed plugin and no managed-store skill from an omm checkout: nothing to reconcile"]),
        );
        let json = doc.finish(&converge);
        ctx.out.emit(
            || format!("nothing to reconcile\n{}", converge.render()),
            || json,
        );
        return Ok(ExitCode::SUCCESS);
    }
    let ledger = session.ledger_or_new(inv)?.clone();

    // Registrations vs the host.
    let (plugin_reg, mkt_reg) = plugin::registrations(&ledger, &pid);
    let mut regs = ledger.registrations.clone();
    match (&plugin_reg, &installed) {
        (Some(_), None) => {
            regs.retain(|r| !matches!(r, Registration::MusePlugin { id, .. } if *id == pid));
            converge.record("registrations", Action::Removed);
            notes.push(format!(
                "plugin `{pid}` registered but not installed: registration dropped"
            ));
        }
        (reg, Some(p)) => {
            let inspect = omm_host::probe::plugins_inspect(inv, &pid)?;
            let trusted: Vec<String> = inspect
                .runtime_capabilities
                .iter()
                .filter(|c| c.is_trusted_enabled())
                .map(|c| c.stable_id.clone())
                .collect();
            let fresh = Registration::MusePlugin {
                id: pid.clone(),
                package_sha256: p.package_sha256.clone().unwrap_or_default(),
                generation_path: p
                    .source_path
                    .as_ref()
                    .map(|s| s.display().to_string())
                    .unwrap_or_default(),
                approved: trusted,
            };
            if reg.as_ref() == Some(&fresh) {
                converge.record("registrations", Action::Unchanged);
            } else {
                regs.retain(|r| !matches!(r, Registration::MusePlugin { id, .. } if *id == pid));
                regs.push(fresh);
                converge.record("registrations", Action::Updated);
                notes.push(format!(
                    "plugin `{pid}`: registration {} from the host ({})",
                    if reg.is_some() {
                        "refreshed"
                    } else {
                        "adopted"
                    },
                    p.package_sha256
                        .as_deref()
                        .map(plugin::short)
                        .unwrap_or_default()
                ));
            }
        }
        (None, None) => {}
    }
    let mkt_name = match &mkt_reg {
        Some(Registration::MuseMarketplace { name, .. }) => name.clone(),
        _ => omm_manifest::MARKETPLACE_NAME.to_string(),
    };
    match (
        mkt_reg.as_ref(),
        plugin::configured_marketplace(inv, &mkt_name)?,
    ) {
        (Some(_), None) => {
            regs.retain(|r| !matches!(r, Registration::MuseMarketplace { .. }));
            converge.record("registrations", Action::Removed);
            notes.push(format!(
                "marketplace `{mkt_name}` registered but not configured: registration dropped"
            ));
        }
        (Some(Registration::MuseMarketplace { source, .. }), Some(m)) if *source == m.source => {
            converge.record("registrations", Action::Unchanged);
        }
        (_, Some(m)) => {
            regs.retain(|r| !matches!(r, Registration::MuseMarketplace { .. }));
            regs.insert(
                0,
                Registration::MuseMarketplace {
                    name: m.name.clone(),
                    source: m.source.clone(),
                },
            );
            converge.record("registrations", Action::Updated);
            notes.push(format!(
                "marketplace `{}`: registration adopted from the host ({})",
                m.name, m.source
            ));
        }
        (None, None) => {}
    }

    // File entries vs the disk.
    let mut entries = ledger.entries.clone();
    let mut keep = Vec::with_capacity(entries.len());
    for e in entries.drain(..) {
        if !e.is_file() {
            keep.push(e);
            continue;
        }
        let key = e.key();
        match session.bases.resolve(e.base, &e.path) {
            Err(err) => {
                converge.record("files", Action::Skipped);
                notes.push(format!(
                    "{key}: {err} (left in the ledger; uninstall refuses it)"
                ));
                keep.push(e);
            }
            Ok(r) if r.via_symlink.is_some() => {
                converge.record("files", Action::Skipped);
                notes.push(format!(
                    "{key}: an ancestor is a symlink now (R2 sentinel); kept, never touched"
                ));
                keep.push(e);
            }
            Ok(r) => match r.state {
                State::Missing => {
                    converge.record("files", Action::Removed);
                    notes.push(format!("{key}: vanished from disk; entry dropped"));
                    session.record(
                        &Event::new(ACTION_UNINSTALL)
                            .at(key.base, &key.path)
                            .before(Some(&e.sha256))
                            .note("reconcile: vanished from disk; entry dropped"),
                    )?;
                }
                State::File | State::Dir => {
                    if hash::observe(&r.path)?.equals(&e.sha256) {
                        converge.record("files", Action::Unchanged);
                    } else {
                        converge.record("files", Action::Skipped);
                        notes.push(format!("{key}: edited since omm wrote it (kept; `omm update` stages the new content)"));
                    }
                    keep.push(e);
                }
                State::Symlink | State::Other => {
                    converge.record("files", Action::Skipped);
                    notes.push(format!(
                        "{key}: not a regular file now (R2 sentinel); kept, never touched"
                    ));
                    keep.push(e);
                }
            },
        }
    }
    {
        let l = session.ledger_or_new(inv)?;
        l.entries = keep;
        l.sort();
        l.registrations = regs;
    }
    session.save()?;
    // Managed-store skills the ledger lists no entries for: adopted when
    // identical to the source (nothing is written to the host).
    if !managed.is_empty() {
        match Source::resolve_with_ledger(None, &session.omm_root) {
            Ok(source) => {
                let (adopted, skipped) =
                    skills::adopt_identical(ctx, inv, &mut session, &source, "omm reconcile")?;
                for id in &adopted {
                    converge.record("skills", Action::Updated);
                    notes.push(format!(
                        "managed-store skill `{id}`: adopted from the host (identical to the source at {})",
                        source.repo.root.display()
                    ));
                }
                for (id, why) in &skipped {
                    converge.record("skills", Action::Skipped);
                    notes.push(format!(
                        "managed-store skill `{id}`: not adopted ({why}); `omm install --no-plugin` reports it"
                    ));
                }
                session.save()?;
            }
            Err(e) => notes.push(format!(
                "{} managed-store skill(s) from an omm checkout are not in the ledger ({}), and no content source is available to compare against ({e}); `omm install --no-plugin --source <checkout>` adopts them",
                managed.len(),
                managed
                    .iter()
                    .map(|(id, _)| id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }
    if let Some(l) = &session.ledger {
        for u in unregistered_shared_state(ctx, l, workspace.as_deref()) {
            notes.push(format!(
                "{u} — set before this ledger existed; `omm uninstall` names it and leaves it"
            ));
        }
    }
    doc.set("notes", json!(notes));
    let json = doc.finish(&converge);
    ctx.out.emit(
        || {
            let mut text = notes.join("\n");
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&converge.render());
            text
        },
        || json,
    );
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// trust
// ---------------------------------------------------------------------------

/// `omm trust [path]`: merge one workspace key into `trust.json`, ledgered
/// with its prior value.
pub fn trust(ctx: &Ctx, args: &TrustArgs) -> Result<ExitCode> {
    ctx.require_consent("trust")?;
    let ws = match &args.path {
        Some(p) => fsx::canonicalize(p)?,
        None => ctx.workspace()?,
    };
    if !ws.is_dir() {
        return Err(OmmError::Usage(format!(
            "{} is not a directory",
            ws.display()
        )));
    }
    let inv = ctx.invoker()?;
    let mut session = Session::open(ctx, Some(&ws))?;
    session.warn_quarantine(ctx);
    let mut converge = Converge::new(ctx.dry_run);
    let outcome = files::trust_workspace(ctx, inv, &mut session, &ws, &mut converge)?;
    session.save()?;
    let mut doc = Doc::new("trust", ctx);
    doc.set("trust", outcome.to_json());
    let json = doc.finish(&converge);
    ctx.out.emit(
        || format!("{}\n{}", outcome.line(), converge.render()),
        || json,
    );
    Ok(ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// list
// ---------------------------------------------------------------------------

/// `omm list`: the resolved assets with provenance and `_shadowed` (R7).
pub fn list(ctx: &Ctx, args: &ListArgs) -> Result<ExitCode> {
    let source = Source::resolve_with_ledger(args.source.as_deref(), &ctx.omm_root())?;
    let kind = match &args.kind {
        Some(k) => Some(AssetKind::from_singular(k).ok_or_else(|| {
            OmmError::Usage(format!(
                "unknown kind {k:?}; one of {}",
                AssetKind::ALL
                    .iter()
                    .map(|k| k.singular())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?),
        None => None,
    };
    let res = overlay::resolve(&source.catalog, &source.content, &ctx.omm_root())?;
    for p in &res.problems {
        ctx.out.warn(p.to_string());
    }
    for d in &res.unknown_disabled {
        ctx.out
            .warn(format!("config.json disables {d:?}, which names no asset"));
    }
    let mut table = Table::new([
        "id", "kind", "provider", "level", "disabled", "shadowed", "append", "path",
    ]);
    let mut items = Vec::new();
    for item in res
        .items
        .iter()
        .filter(|i| kind.map(|k| i.kind == k).unwrap_or(true))
    {
        table.row([
            item.id.clone(),
            item.kind.singular().to_string(),
            format!("{:?}", item.provider).to_lowercase(),
            item.level.to_string(),
            if item.disabled {
                "yes".into()
            } else {
                String::new()
            },
            item.shadowed
                .iter()
                .map(|s| format!("{:?}:{}", s.provider, s.path.display()).to_lowercase())
                .collect::<Vec<_>>()
                .join(" "),
            item.append
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            item.path.display().to_string(),
        ]);
        items.push(serde_json::to_value(item).unwrap_or(Value::Null));
    }
    let json = json!({
        "command": "list",
        "source": source_json(&source),
        "omm_root": ctx.omm_root(),
        "items": items,
        "unknown_disabled": res.unknown_disabled,
        "problems": res.problems.iter().map(|p| p.to_string()).collect::<Vec<_>>(),
    });
    ctx.out.emit(
        || {
            if table.is_empty() {
                "no assets".to_string()
            } else {
                table.render()
            }
        },
        || json,
    );
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Command};
    use clap::Parser;

    #[test]
    fn install_flags_parse() {
        let cli = Cli::try_parse_from([
            "omm",
            "install",
            "--source",
            "/src",
            "--workspace",
            "/ws",
            "--profile",
            "omm-fast",
            "--reinstall",
        ])
        .unwrap();
        match cli.command {
            Command::Install(a) => {
                assert_eq!(a.source.as_deref(), Some(Path::new("/src")));
                assert_eq!(a.workspace.as_deref(), Some(Path::new("/ws")));
                assert_eq!(a.profile.as_deref(), Some("omm-fast"));
                assert!(a.reinstall && !a.no_plugin);
            }
            other => panic!("{other:?}"),
        }
        let cli = Cli::try_parse_from(["omm", "list", "--kind", "skill"]).unwrap();
        match cli.command {
            Command::List(a) => assert_eq!(a.kind.as_deref(), Some("skill")),
            other => panic!("{other:?}"),
        }
        let cli =
            Cli::try_parse_from(["omm", "install", "--skip", "themes", "--skip", "trust"]).unwrap();
        match cli.command {
            Command::Install(a) => {
                assert_eq!(a.skip, vec!["themes", "trust"]);
                assert_eq!(
                    parse_skips(&a.skip)
                        .unwrap()
                        .into_iter()
                        .collect::<Vec<_>>(),
                    vec![Step::Themes, Step::Trust]
                );
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            parse_skips(&["widgets".to_string()]),
            Err(OmmError::Usage(_))
        ));
        let cli = Cli::try_parse_from(["omm", "install", "--drop-unknown-settings-keys"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Install(InstallArgs {
                drop_unknown_settings_keys: true,
                ..
            })
        ));
        let cli = Cli::try_parse_from(["omm", "uninstall", "--reconcile-host"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Uninstall(UninstallArgs {
                reconcile_host: true,
                force: false
            })
        ));
        let cli = Cli::try_parse_from(["omm", "update", "--reinstall"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Update(UpdateArgs {
                reinstall: true,
                ..
            })
        ));
    }

    #[test]
    fn a_planned_path_outside_the_base_is_refused_with_the_way_out() {
        // Gate 1 decision A: the refusal names the exact path, the step, the
        // reason, and the flags that leave the step out or move it.
        let config = Path::new("/cfg/muse");
        let escaped = PlannedFile {
            step: "themes",
            key: EntryKey {
                base: Base::MuseConfig,
                path: RelPath::new("themes/omm-carbon.tmTheme").unwrap(),
            },
            abs: PathBuf::from("/cfg/muse/themes/omm-carbon.tmTheme"),
            bytes: 10,
            containment: Err(
                "resolves outside the base: /home/me/dotfiles/themes is not under /cfg/muse".into(),
            ),
            via_symlink: None,
            writable: Some(true),
        };
        let text = containment_refusal(config, &[&escaped]);
        for needle in [
            "/cfg/muse/themes/omm-carbon.tmTheme",
            "outside the base",
            "omm install --skip themes",
            "OMM_THEMES_DIR=<dir>",
            "nothing was written",
        ] {
            assert!(text.contains(needle), "{needle}: {text}");
        }
        let skills = PlannedFile {
            step: "skills",
            containment: Err("resolves outside the base".into()),
            ..escaped.clone()
        };
        let text = containment_refusal(config, &[&skills]);
        assert!(text.contains("plugin bundle"), "{text}");
        // In-base ancestor symlinks are not refusals; a tempdir plan resolves.
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let roots = sb.roots().unwrap();
        let bases = Bases::from_roots(&roots, None);
        let f = planned_file(
            &bases,
            "themes",
            RelPath::new("themes/x.tmTheme").unwrap(),
            3,
        );
        assert!(f.containment.is_ok());
        assert_eq!(f.abs, roots.muse_config().join("themes/x.tmTheme"));
        assert_eq!(f.via_symlink, None);
        assert!(f.to_json()["contained"].as_bool().unwrap());
    }

    #[test]
    fn list_refuses_an_unknown_kind_before_touching_anything() {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let ctx = Ctx::new(sb.roots().unwrap(), crate::cmd::Flags::default());
        let args = ListArgs {
            source: Some(tmp.path().to_path_buf()),
            kind: Some("widget".into()),
        };
        // The source is checked first (no catalog here): a Usage error either way.
        assert!(matches!(list(&ctx, &args), Err(OmmError::Usage(_))));
    }
}
