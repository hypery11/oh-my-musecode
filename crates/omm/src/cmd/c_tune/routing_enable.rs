//! `omm enable skill-routing` / `omm disable skill-routing` — the two
//! ledgered transactions and the order-200 measurement between them
//! (PLAN.md 3.1; ARCHITECTURE.md §4, §7; R1, R2, R3, R4, R12, R18).
//!
//! `enable`, in a trusted git workspace (`.git` present; the project hook
//! tier is trust-gated and a router in an untrusted workspace never loads
//! — silently, skill-routing.md V4.1):
//!
//! 1. measures the order-200 `skills_catalog` this workspace composes — one
//!    `muse exec --provider echo --trust-workspace hi` in a throwaway copy of
//!    the workspace's project skill roots (`.agents/skills`, `.codex/skills`,
//!    `.claude/skills`; never the workspace itself, so no project hook of
//!    the user's runs) against a throwaway data root seeded with the host's
//!    plugin store (`omm_doctor::session` does the same), the config root
//!    as it is; the catalog estimate stands in when that fails;
//! 2. copies the library (`content/routing/library/<id>/`) to
//!    `<ws>/.omm/skills/<id>/` — every file a `workspace` ledger entry
//!    (`copy`, `exclusive`) reconciled with R3's four outcomes, the entry
//!    saved BEFORE the bytes land (§4), an edited file left alone and
//!    named, an ancestor symlink refused (R2 sentinel);
//! 3. writes `<ws>/.muse/hooks.json`: created (`copy`, `exclusive`, so
//!    uninstall removes it when unedited) or merged into (`hooks-merge`,
//!    `shared-key`, the pre-omm bytes and mode in `prior.original`, one
//!    handler in its own group — a hooks file that does not parse is
//!    refused, never clobbered, R10);
//! 4. writes `<ws>/.omm/routing.json` (the router's per-workspace state,
//!    ledgered like a skill) and `$OMM/config.json` → `skill_routing: true`
//!    + `routing: {…}` so `omm run` exports both gates.
//!
//! `disable` reverses it: every ledgered library file removed when it still
//! holds omm's bytes (kept and named when edited), the hooks file removed
//! when omm created it and it is unedited, else omm's handler taken out and
//! the pre-omm bytes and mode written back when nothing else changed, the
//! directories omm created removed when empty, the config keys removed.
//! Run in the workspace. `omm uninstall` covers the same entries by the
//! general rules (§4) and names a merged hooks file it cannot restore.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use omm_host::host_reality as hr;
use omm_host::trust::TrustStore;
use omm_host::{fsx, probe};
use omm_ledger::audit::{self, Audit, Event};
use omm_ledger::containment::{Bases, Resolved, State as FsState};
use omm_ledger::hash::{self, Observed};
use omm_ledger::reconcile::{self, Outcome};
use omm_ledger::shared::Original;
use omm_ledger::{Base, Class, Entry, Kind, Ledger, Mechanism, RelPath};
use omm_manifest::lint::ID_PREFIX as SKILL_ID_PREFIX;

use super::routing::{self, LibrarySkill, State};
use super::{
    modify_ledger, read_ledger, read_regular_file, rel, thousands, ContentSource, OmmConfig,
    WriteReport,
};
use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::Action;

/// The converge categories.
pub const CAT_SKILLS: &str = "routed-skills";
pub const CAT_HOOKS: &str = "hooks";
pub const CAT_STATE: &str = "routing-state";
pub const CAT_CONFIG: &str = "config";
/// The ledger writers.
pub const WRITER_ENABLE: &str = "omm enable skill-routing";
pub const WRITER_DISABLE: &str = "omm disable skill-routing";
/// The snapshot base directory name for workspace backups.
pub const WORKSPACE_BASE: &str = "workspace";
/// The `prior` key on a hooks file omm created: the directories it made.
pub const PRIOR_CREATED_DIRS: &str = "created_dirs";
/// The `routing` record key telling `disable` that `enable` created
/// `$OMM/config.json`.
pub const CONFIG_CREATED_KEY: &str = "config_created";
/// The project skill roots the measurement mirrors (host-reality "Paths":
/// project skills, in the host's inverted foreign order).
pub const PROJECT_SKILL_ROOTS: [&str; 3] = [".agents/skills", ".codex/skills", ".claude/skills"];
/// The measurement prompt (echo never sends it anywhere).
pub const MEASURE_PROMPT: &str = "hi";
/// `order200_source` values.
pub const SOURCE_MEASURED: &str = "measured";

/// `thousands` for a signed number (`-1,234`).
pub fn signed_thousands(n: i64) -> String {
    if n < 0 {
        format!("-{}", thousands(n.unsigned_abs()))
    } else {
        thousands(n as u64)
    }
}

/// True when `path` is the root of a git checkout (`.git` dir, or the file
/// a worktree carries) — the workspace shape `omm install` trusts.
pub fn is_git_workspace(path: &Path) -> bool {
    let git = path.join(".git");
    std::fs::symlink_metadata(&git)
        .map(|m| m.is_dir() || m.is_file())
        .unwrap_or(false)
}

/// The workspace `omm enable` / `omm disable` act on: the canonical cwd,
/// which must be a git checkout.
fn workspace(ctx: &Ctx) -> Result<PathBuf> {
    let ws = ctx.workspace()?;
    if !is_git_workspace(&ws) {
        return Err(OmmError::Usage(format!(
            "{} is not the root of a git checkout; skill routing is enabled per workspace — run this at the repository root",
            ws.display()
        )));
    }
    Ok(ws)
}

/// The trust gate: the project hook tier loads only for a workspace whose
/// `trust.json` decision is literally `trusted`.
fn require_trusted(ctx: &Ctx, ws: &Path) -> Result<()> {
    let store = TrustStore::for_roots(&ctx.roots)?;
    match store.decision_for(ws)? {
        Some(d) if d == "trusted" => Ok(()),
        other => Err(OmmError::Usage(format!(
            "{} is {} in {}; a project hooks file never loads there and the router would fail silently (host-reality \"Trust lifecycle\"): run `omm trust .` first",
            ws.display(),
            other.map(|d| format!("`{d}`")).unwrap_or_else(|| "not recorded".into()),
            store.path().display()
        ))),
    }
}

// ---- the measurement ---------------------------------------------------------

/// The order-200 bytes this workspace composes, measured live (see the
/// module docs); `Err` when the host could not be run, so the caller can
/// fall back to the estimate and say so.
pub fn measure_order200(ctx: &Ctx, ws: &Path) -> Result<u64> {
    let tmp = tempfile::Builder::new()
        .prefix("omm-routing-measure-")
        .tempdir()
        .map_err(|e| OmmError::io("create temp dir", e))?;
    let data = tmp.path().join("data");
    let plugins = data.join("muse").join("plugins");
    fsx::create_dir_all(&plugins)?;
    let src = ctx.roots.plugins_dir();
    let installed = src.join("installed.json");
    if installed.is_file() {
        fsx::write_atomic(
            &plugins.join("installed.json"),
            &fsx::read_bytes(&installed)?,
        )?;
    }
    omm_doctor::session::copy_tree(&src.join("cache"), &plugins.join("cache"), "plugins")?;
    let ws_copy = tmp.path().join("ws");
    fsx::create_dir_all(&ws_copy)?;
    for root in PROJECT_SKILL_ROOTS {
        let from = ws.join(root);
        if from.is_dir() {
            omm_doctor::session::copy_tree(&from, &ws_copy.join(root), "project skills")?;
        }
    }
    let inv = ctx.invoker()?.clone().data_home(&data).cwd(&ws_copy);
    let out = inv.run(&[
        "exec",
        "--provider",
        "echo",
        "--trust-workspace",
        MEASURE_PROMPT,
    ])?;
    if !out.ok() {
        return Err(OmmError::Usage(format!(
            "the measurement session failed (exit {:?}): {}",
            out.code,
            out.stderr.lines().next().unwrap_or("").trim()
        )));
    }
    let log = probe::newest_session_log(&data.join("muse").join("sessions"))?;
    let facts = probe::parse_session_log(&log)?;
    Ok(facts
        .block(u64::from(hr::CONTEXT_ORDER_SKILLS_CATALOG))
        .map(|b| b.bytes as u64)
        .unwrap_or(0))
}

// ---- one ledgered workspace file ----------------------------------------------

/// What [`place`] decided for one file.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Placed {
    rel: RelPath,
    abs: PathBuf,
    outcome: Outcome,
    reason: String,
    /// The bytes to land and their hash (`Overwrite` / `Adopt`).
    bytes: Vec<u8>,
    sha256: String,
    /// The on-disk content before (for the backup and the audit line).
    before: Observed,
}

/// Decide one file against the ledger and the disk (R3): `NoOp` (already
/// ours), `Adopt` (identical bytes, unrecorded), `Overwrite` (ours or
/// absent), `Stage` (the user's — left alone). An ancestor symlink or a
/// non-regular leaf is `Stage` with its reason (R2).
fn place(
    bases: &Bases,
    ledger: Option<&Ledger>,
    rel_path: &RelPath,
    bytes: Vec<u8>,
) -> Result<Placed> {
    let resolved: Resolved = bases.resolve(Base::Workspace, rel_path)?;
    let sha256 = fsx::sha256_bytes(&bytes);
    let before = hash::observe(&resolved.path)?;
    if let Some(link) = &resolved.via_symlink {
        return Ok(Placed {
            rel: rel_path.clone(),
            abs: resolved.path,
            outcome: Outcome::Stage,
            reason: format!(
                "behind a symlink ({}); omm never writes through one (R2)",
                link.display()
            ),
            bytes,
            sha256,
            before,
        });
    }
    if matches!(
        resolved.state,
        FsState::Symlink | FsState::Dir | FsState::Other
    ) {
        return Ok(Placed {
            rel: rel_path.clone(),
            abs: resolved.path,
            outcome: Outcome::Stage,
            reason: "not a regular file now (R2 sentinel); left alone".into(),
            bytes,
            sha256,
            before,
        });
    }
    let ancestor = ledger
        .and_then(|l| l.find(Base::Workspace, rel_path))
        .map(|e| e.sha256.clone());
    let (outcome, reason) = reconcile::decide(ancestor.as_deref(), &sha256, &before);
    Ok(Placed {
        rel: rel_path.clone(),
        abs: resolved.path,
        outcome,
        reason,
        bytes,
        sha256,
        before,
    })
}

/// The ledger entry for a placed file.
fn entry_for(p: &Placed, kind: Kind, prior: Option<Value>) -> Entry {
    Entry {
        base: Base::Workspace,
        path: p.rel.clone(),
        kind,
        sha256: p.sha256.clone(),
        source_version: OMM_VERSION.to_string(),
        writer: WRITER_ENABLE.to_string(),
        mechanism: Mechanism::Copy,
        class: Class::Exclusive,
        prior,
    }
}

/// Land the bytes of an `Overwrite`: parent directories created, an
/// existing file backed up under `$OMM/snapshots/<ts>/workspace/`, the
/// write atomic on the realpath (mode preserved).
fn land(ctx: &Ctx, p: &Placed) -> Result<Option<PathBuf>> {
    let parent = p
        .abs
        .parent()
        .ok_or_else(|| OmmError::Usage(format!("{} has no parent", p.abs.display())))?;
    fsx::create_dir_all(parent)?;
    let realpath = fsx::realpath_for_write(&p.abs)?;
    let backup = if matches!(p.before, Observed::Content(_)) {
        Some(fsx::snapshot_backup(
            &realpath,
            &ctx.roots.snapshots_dir(),
            WORKSPACE_BASE,
        )?)
    } else {
        None
    };
    fsx::write_atomic(&realpath, &p.bytes)?;
    Ok(backup)
}

/// Every regular file under a library skill directory, as `(relative,
/// bytes)`, sorted; a symlink anywhere is refused (R12).
fn skill_files(skill: &LibrarySkill) -> Result<Vec<(String, Vec<u8>)>> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(&skill.dir)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
    {
        let entry = entry.map_err(|e| OmmError::Usage(format!("{}: {e}", skill.dir.display())))?;
        if entry.path() == skill.dir {
            continue;
        }
        if entry.path_is_symlink() {
            return Err(OmmError::Usage(format!(
                "{} is a symlink; the library holds regular files only (R12)",
                entry.path().display()
            )));
        }
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(&skill.dir).map_err(|_| {
            OmmError::Usage(format!(
                "{} escapes its skill directory",
                entry.path().display()
            ))
        })?;
        let rel = omm_manifest::posix(rel);
        if rel.contains('\\') {
            return Err(OmmError::Usage(format!(
                "{}: backslash in a file name (R12)",
                entry.path().display()
            )));
        }
        out.push((rel, read_regular_file(entry.path())?));
    }
    Ok(out)
}

// ---- enable ------------------------------------------------------------------

/// `omm enable skill-routing` (see the module docs).
pub fn enable(ctx: &Ctx) -> Result<WriteReport> {
    let ws = workspace(ctx)?;
    require_trusted(ctx, &ws)?;
    let omm_root = ctx.omm_root();
    let source = ContentSource::require(&omm_root)?;
    let catalog = source.catalog()?;
    let mut taken: Vec<String> = catalog.ids().into_iter().map(str::to_string).collect();
    taken.extend(hr::bundled_skills()?.items.iter().map(|b| b.id.clone()));
    let lib_dir = source.asset_path(routing::LIBRARY_REL)?;
    let library = routing::load_library(&lib_dir, SKILL_ID_PREFIX, &taken)?;
    if library.is_empty() {
        return Err(OmmError::Usage(format!(
            "{} holds no skills",
            lib_dir.display()
        )));
    }
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("workspace", json!(ws.display().to_string()));
    report.field("library", json!(lib_dir.display().to_string()));

    // 1. The order-200 size the router will budget against.
    let (order200, source200): (u64, String) = match measure_order200(ctx, &ws) {
        Ok(b) => (b, SOURCE_MEASURED.to_string()),
        Err(e) => {
            let (b, s) = routing::order200_for(&ctx.roots, &ws);
            ctx.out.warn(format!(
                "order 200 could not be measured ({e}); budgeting against the {} value {} B",
                s.as_str(),
                thousands(b)
            ));
            (b, s.as_str().to_string())
        }
    };

    // 2. The library, file by file.
    let bases = Bases::from_roots(&ctx.roots, Some(&ws));
    let ledger = read_ledger(ctx)?;
    let mut placed: Vec<(Placed, Kind)> = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    for skill in &library {
        ids.push(skill.id.clone());
        for (file_rel, bytes) in skill_files(skill)? {
            let rel_path = rel(&format!(
                "{}/{}/{file_rel}",
                routing::WS_SKILLS_DIR,
                skill.id
            ))?;
            placed.push((
                place(&bases, ledger.as_ref(), &rel_path, bytes)?,
                Kind::Skill,
            ));
        }
    }
    // Ledgered library files the library no longer carries: removed when
    // still omm's bytes, else kept and named.
    let prefix = format!("{}/", routing::WS_SKILLS_DIR);
    let mut stale: Vec<(RelPath, PathBuf, bool)> = Vec::new();
    if let Some(l) = &ledger {
        for e in l
            .entries
            .iter()
            .filter(|e| e.base == Base::Workspace && e.is_file())
        {
            let p = e.path.as_str();
            let listed = placed.iter().any(|(x, _)| x.rel == e.path);
            if p.starts_with(&prefix) && !listed {
                let r = bases.resolve(Base::Workspace, &e.path)?;
                let ours = r.via_symlink.is_none()
                    && r.state == FsState::File
                    && hash::observe(&r.path)?.equals(&e.sha256);
                stale.push((e.path.clone(), r.path, ours));
            }
        }
    }

    // 3. The state file and the hooks file.
    let ws_lib = ws.join(routing::WS_SKILLS_DIR);
    let placed_skills: Vec<LibrarySkill> = library
        .iter()
        .map(|s| LibrarySkill {
            dir: ws_lib.join(&s.id),
            path: ws_lib.join(&s.id).join(routing::SKILL_FILE),
            ..s.clone()
        })
        .collect();
    let hooks_abs = ws.join(routing::WS_HOOKS_FILE);
    let order201_max = routing::worst_case_bytes(&placed_skills, &hooks_abs);
    let exe = std::env::current_exe().map_err(|e| OmmError::io("locate this executable", e))?;
    let program = routing::handler_program(&exe, std::env::var_os("PATH").as_deref());
    let command = routing::handler_command(&program);
    let mut state = State {
        workspace: Some(ws.clone()),
        hooks_file: Some(hooks_abs.clone()),
        handler_command: Some(command.clone()),
        order200_bytes: Some(order200),
        order200_source: Some(source200.clone()),
        measured_at: Some(fsx::timestamp()),
        skills: ids.clone(),
        order201_max_bytes: Some(order201_max),
    };
    // A rerun that measured the same thing keeps the earlier timestamp, so
    // the state file and the config record stay byte-identical (idempotent).
    if let Ok(b) = std::fs::read(ws.join(routing::WS_STATE_FILE)) {
        let earlier = State::from_bytes_lenient(&b);
        if earlier.same_measurement(&state) && earlier.measured_at.is_some() {
            state.measured_at = earlier.measured_at.clone();
        }
    }
    let state_doc = state.to_value(OMM_VERSION);
    let mut state_bytes = serde_json::to_vec_pretty(&state_doc)
        .map_err(|e| OmmError::Usage(format!("routing.json: {e}")))?;
    state_bytes.push(b'\n');
    let state_rel = rel(routing::WS_STATE_FILE)?;
    placed.push((
        place(&bases, ledger.as_ref(), &state_rel, state_bytes)?,
        Kind::Hook,
    ));

    let hooks_rel = rel(routing::WS_HOOKS_FILE)?;
    let hooks = plan_hooks(&bases, ledger.as_ref(), &ws, &hooks_rel, &command)?;

    // 4. Ledger first (§4: the entry precedes the bytes), then the files.
    let writes: Vec<&(Placed, Kind)> = placed
        .iter()
        .filter(|(p, _)| matches!(p.outcome, Outcome::Overwrite | Outcome::Adopt))
        .collect();
    if !ctx.dry_run {
        let entries: Vec<Entry> = writes.iter().map(|(p, k)| entry_for(p, *k, None)).collect();
        let stale_ours: Vec<RelPath> = stale
            .iter()
            .filter(|(_, _, ours)| *ours)
            .map(|(r, _, _)| r.clone())
            .collect();
        let hooks_entry = hooks.entry.clone();
        modify_ledger(ctx, |l| {
            for e in entries {
                l.upsert(e);
            }
            for r in &stale_ours {
                l.remove(Base::Workspace, r);
            }
            if let Some(e) = hooks_entry {
                l.upsert(e);
            }
            Ok(())
        })?;
    }
    let audit = Audit::new(&omm_root, OMM_VERSION);
    for (p, _) in &placed {
        let action = match p.outcome {
            Outcome::NoOp => Action::Unchanged,
            Outcome::Adopt | Outcome::Overwrite => Action::Updated,
            Outcome::Stage => Action::Skipped,
        };
        let cat = if p.rel == state_rel {
            CAT_STATE
        } else {
            CAT_SKILLS
        };
        report.converge.record(cat, action);
        match p.outcome {
            Outcome::Overwrite => {
                let mut backup = None;
                if !ctx.dry_run {
                    backup = land(ctx, p)?;
                    audit.append(
                        &Event::new(audit::ACTION_INSTALL)
                            .at(Base::Workspace, &p.rel)
                            .before(p.before.text())
                            .after(Some(&p.sha256))
                            .note(format!("{WRITER_ENABLE}: {}", p.reason)),
                    )?;
                }
                if backup.is_some() {
                    report.converge.record(cat, Action::BackedUp);
                }
                report.line(format!(
                    "{} {} ({} B, {})",
                    p.abs.display(),
                    if ctx.dry_run {
                        "would be written"
                    } else {
                        "written"
                    },
                    p.bytes.len(),
                    p.reason
                ));
            }
            Outcome::Adopt => {
                if !ctx.dry_run {
                    audit.append(
                        &Event::new(audit::ACTION_INSTALL)
                            .at(Base::Workspace, &p.rel)
                            .before(p.before.text())
                            .after(Some(&p.sha256))
                            .note(format!("{WRITER_ENABLE}: adopted, identical bytes on disk")),
                    )?;
                }
                report.line(format!(
                    "{} already holds these bytes; recorded",
                    p.abs.display()
                ));
            }
            Outcome::NoOp => {
                report.line(format!("{} unchanged", p.abs.display()));
            }
            Outcome::Stage => {
                ctx.out
                    .warn(format!("{} left untouched: {}", p.abs.display(), p.reason));
                report.line(format!("{} skipped ({})", p.abs.display(), p.reason));
            }
        }
    }
    for (r, abs, ours) in &stale {
        if *ours {
            if !ctx.dry_run {
                std::fs::remove_file(abs)
                    .map_err(|e| OmmError::io(format!("remove {}", abs.display()), e))?;
                prune_empty_dirs(&ws, abs.parent(), &[routing::WS_SKILLS_DIR, ".omm"]);
                audit.append(
                    &Event::new(audit::ACTION_UNINSTALL)
                        .at(Base::Workspace, r)
                        .note(format!("{WRITER_ENABLE}: no longer in the library")),
                )?;
            }
            report.converge.record(CAT_SKILLS, Action::Removed);
            report.line(format!(
                "{} {} (no longer in the library)",
                abs.display(),
                if ctx.dry_run {
                    "would be removed"
                } else {
                    "removed"
                }
            ));
        } else {
            report.converge.record(CAT_SKILLS, Action::Skipped);
            report.line(format!(
                "{} kept: no longer in the library but not omm's bytes",
                abs.display()
            ));
        }
    }
    apply_hooks(ctx, &hooks, &audit, &mut report)?;

    // 5. `$OMM/config.json`.
    let config_path = OmmConfig::path(&omm_root);
    let config_existed = config_path.exists();
    let mut config = OmmConfig::load(&omm_root)?;
    let created = config
        .get(routing::CONFIG_KEY_ROUTING)
        .and_then(|r| r.get(CONFIG_CREATED_KEY))
        .and_then(Value::as_bool)
        .unwrap_or(!config_existed);
    let mut record = state_doc.clone();
    if let Some(m) = record.as_object_mut() {
        m.insert(CONFIG_CREATED_KEY.into(), json!(created));
    }
    config.set(super::CONFIG_KEY_SKILL_ROUTING, json!(true));
    config.set(routing::CONFIG_KEY_ROUTING, record);
    if ctx.dry_run {
        report.converge.record(CAT_CONFIG, Action::Updated);
    } else {
        let (_, changed) = config.save(&omm_root)?;
        report.converge.record(
            CAT_CONFIG,
            if changed {
                Action::Updated
            } else {
                Action::Unchanged
            },
        );
    }

    // 6. The summary.
    let headroom = hr::ROUTING_BUDGET_BYTES as i64 - order200 as i64 - order201_max as i64;
    report.line(format!(
        "budget: order 200 {} B ({source200}) + order 201 at most {} B (three of {} skills) = {} of {} B; headroom {} B{}",
        thousands(order200),
        thousands(order201_max),
        ids.len(),
        thousands(order200 + order201_max),
        thousands(hr::ROUTING_BUDGET_BYTES),
        signed_thousands(headroom),
        if headroom < routing::BUDGET_MARGIN_BYTES as i64 {
            " — the router will trim or drop entries every turn; slim order 200 (`omm settings set run.context_slimming.skill_catalog_descriptions first_sentence`)"
        } else {
            ""
        }
    ));
    report.line(format!(
        "gates: `omm run` exports {}=1 and {}=1; a plain `muse` needs them exported in the shell",
        hr::ENV_ROUTING_GATE,
        hr::ENV_ROUTING_APPLY_GATE
    ));
    report.line("order 201 changes every turn and invalidates the prompt-cache prefix from there on (why this is opt-in)".to_string());
    report.field("skills", json!(ids));
    report.field("hooks_file", json!(hooks_abs.display().to_string()));
    report.field("handler_command", json!(command));
    report.field("order200_bytes", json!(order200));
    report.field("order200_source", json!(source200));
    report.field("order201_max_bytes", json!(order201_max));
    report.field("headroom_bytes", json!(headroom));
    report.field(
        "gates",
        json!([hr::ENV_ROUTING_GATE, hr::ENV_ROUTING_APPLY_GATE]),
    );
    Ok(report)
}

/// What [`plan_hooks`] decided for `.muse/hooks.json`.
#[derive(Clone, Debug)]
struct HooksPlan {
    abs: PathBuf,
    /// The bytes to write, `None` when nothing changes.
    bytes: Option<Vec<u8>>,
    /// The entry to upsert (`None`: nothing of omm's to record).
    entry: Option<Entry>,
    /// The file existed before this run.
    existed: bool,
    /// `.muse/` will be created by this run.
    creates_dir: bool,
    line: String,
}

/// Decide the hooks file: parse what is there (refuse what does not parse,
/// R10), merge the handler, shape the entry (created → `copy`/`exclusive`;
/// pre-existing → `hooks-merge`/`shared-key` with the FIRST original kept).
fn plan_hooks(
    bases: &Bases,
    ledger: Option<&Ledger>,
    ws: &Path,
    hooks_rel: &RelPath,
    command: &str,
) -> Result<HooksPlan> {
    let resolved = bases.resolve(Base::Workspace, hooks_rel)?;
    if let Some(link) = &resolved.via_symlink {
        return Err(OmmError::Usage(format!(
            "{} lies behind a symlink ({}); omm never writes through one (R2)",
            resolved.path.display(),
            link.display()
        )));
    }
    let existing: Option<Vec<u8>> = match resolved.state {
        FsState::File => Some(fsx::read_bytes(&resolved.path)?),
        FsState::Missing => None,
        _ => {
            return Err(OmmError::Usage(format!(
                "{} is not a regular file; move it aside",
                resolved.path.display()
            )))
        }
    };
    let mut doc: Value = match &existing {
        Some(b) => serde_json::from_slice(b).map_err(|e| {
            OmmError::Usage(format!(
                "{} is not JSON ({e}); omm never overwrites a hooks file it cannot parse — repair it by hand",
                resolved.path.display()
            ))
        })?,
        None => json!({}),
    };
    let changed = routing::merge_handler(&mut doc, command)?;
    let prior_entry = ledger
        .and_then(|l| l.find(Base::Workspace, hooks_rel))
        .cloned();
    let creates_dir = !ws.join(".muse").exists();
    if !changed && existing.is_some() {
        // The handler is already there. Recorded → keep the record current;
        // unrecorded → not omm's, left alone.
        let line = match &prior_entry {
            Some(_) => format!(
                "{} unchanged (router handler present)",
                resolved.path.display()
            ),
            None => format!(
                "{} already carries a router handler omm did not write; left as is",
                resolved.path.display()
            ),
        };
        return Ok(HooksPlan {
            abs: resolved.path,
            bytes: None,
            entry: prior_entry,
            existed: true,
            creates_dir,
            line,
        });
    }
    let bytes = routing::pretty(&doc)?;
    let sha256 = fsx::sha256_bytes(&bytes);
    let (mechanism, class, prior) = match (&existing, &prior_entry) {
        (None, _) => (
            Mechanism::Copy,
            Class::Exclusive,
            Some(
                json!({ PRIOR_CREATED_DIRS: if creates_dir { vec![".muse"] } else { Vec::<&str>::new() } }),
            ),
        ),
        (Some(_), Some(e)) => (e.mechanism, e.class, e.prior.clone()),
        (Some(b), None) => {
            let original = Original {
                sha256: fsx::sha256_bytes(b),
                mode: std::fs::metadata(&resolved.path)
                    .ok()
                    .and_then(|m| omm_ledger::shared::file_mode(&m)),
                bytes: b.clone(),
            };
            (
                Mechanism::HooksMerge,
                Class::SharedKey,
                Some(original.to_prior()),
            )
        }
    };
    let entry = Entry {
        base: Base::Workspace,
        path: hooks_rel.clone(),
        kind: Kind::Hook,
        sha256,
        source_version: OMM_VERSION.to_string(),
        writer: WRITER_ENABLE.to_string(),
        mechanism,
        class,
        prior,
    };
    let line = format!(
        "{} {} ({} B, router handler `{}`)",
        resolved.path.display(),
        match (existing.is_some(), changed) {
            (false, _) => "created",
            (true, true) => "merged",
            (true, false) => "unchanged",
        },
        bytes.len(),
        command
    );
    Ok(HooksPlan {
        abs: resolved.path,
        bytes: (changed || existing.is_none()).then_some(bytes),
        entry: Some(entry),
        existed: existing.is_some(),
        creates_dir,
        line,
    })
}

fn apply_hooks(ctx: &Ctx, plan: &HooksPlan, audit: &Audit, report: &mut WriteReport) -> Result<()> {
    let Some(bytes) = &plan.bytes else {
        report.converge.record(CAT_HOOKS, Action::Unchanged);
        report.line(plan.line.clone());
        return Ok(());
    };
    let mut backup = None;
    if !ctx.dry_run {
        if let Some(parent) = plan.abs.parent() {
            fsx::create_dir_all(parent)?;
        }
        let realpath = fsx::realpath_for_write(&plan.abs)?;
        let before = if plan.existed {
            backup = Some(fsx::snapshot_backup(
                &realpath,
                &ctx.roots.snapshots_dir(),
                WORKSPACE_BASE,
            )?);
            Some(fsx::sha256_file(&realpath)?)
        } else {
            None
        };
        fsx::write_atomic(&realpath, bytes)?;
        if let Some(e) = &plan.entry {
            audit.append(
                &Event::new(audit::ACTION_INSTALL)
                    .at(Base::Workspace, &e.path)
                    .before(before.as_deref())
                    .after(Some(&e.sha256))
                    .note(format!(
                        "{WRITER_ENABLE}: hooks file {}",
                        if plan.existed {
                            "merged (pre-omm bytes in the entry)"
                        } else {
                            "created"
                        }
                    )),
            )?;
        }
    }
    report.converge.record(CAT_HOOKS, Action::Updated);
    if backup.is_some() {
        report.converge.record(CAT_HOOKS, Action::BackedUp);
    }
    report.line(if ctx.dry_run {
        plan.line
            .replace("created", "would be created")
            .replace("merged", "would be merged")
    } else {
        plan.line.clone()
    });
    let _ = plan.creates_dir;
    Ok(())
}

/// Remove `dir` and its parents up to (and including) each of `stops`
/// under `ws` while they are empty; never anything else.
fn prune_empty_dirs(ws: &Path, dir: Option<&Path>, stops: &[&str]) {
    let Some(mut cur) = dir.map(Path::to_path_buf) else {
        return;
    };
    let stop_paths: Vec<PathBuf> = stops.iter().map(|s| ws.join(s)).collect();
    loop {
        if !cur.starts_with(ws) || cur == ws {
            return;
        }
        let allowed = stop_paths.iter().any(|s| cur.starts_with(s));
        if !allowed {
            return;
        }
        let empty = match std::fs::read_dir(&cur) {
            Ok(mut rd) => rd.next().is_none(),
            Err(_) => false,
        };
        if !empty || std::fs::remove_dir(&cur).is_err() {
            return;
        }
        match cur.parent() {
            Some(p) => cur = p.to_path_buf(),
            None => return,
        }
    }
}

// ---- disable -----------------------------------------------------------------

/// `omm disable skill-routing` (see the module docs).
pub fn disable(ctx: &Ctx) -> Result<WriteReport> {
    let ws = workspace(ctx)?;
    let omm_root = ctx.omm_root();
    let bases = Bases::from_roots(&ctx.roots, Some(&ws));
    let ledger = read_ledger(ctx)?;
    let audit = Audit::new(&omm_root, OMM_VERSION);
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("workspace", json!(ws.display().to_string()));
    let prefix = format!("{}/", routing::WS_SKILLS_DIR);
    let state_rel = rel(routing::WS_STATE_FILE)?;
    let hooks_rel = rel(routing::WS_HOOKS_FILE)?;

    let mut drop_entries: Vec<RelPath> = Vec::new();
    let mut removed_ids: Vec<String> = Vec::new();
    if let Some(l) = &ledger {
        // 1. The library and the state file: omm's bytes go, edits stay.
        let mut files: Vec<&Entry> = l
            .entries
            .iter()
            .filter(|e| {
                e.base == Base::Workspace
                    && e.is_file()
                    && (e.path.as_str().starts_with(&prefix) || e.path == state_rel)
            })
            .collect();
        files.sort_by_key(|e| std::cmp::Reverse(e.path.depth()));
        for e in files {
            let cat = if e.path == state_rel {
                CAT_STATE
            } else {
                CAT_SKILLS
            };
            let r = bases.resolve(Base::Workspace, &e.path)?;
            if r.via_symlink.is_some() {
                report.converge.record(cat, Action::Skipped);
                report.line(format!(
                    "{} kept: behind a symlink (R2 sentinel)",
                    r.path.display()
                ));
                continue;
            }
            let observed = hash::observe(&r.path)?;
            match observed {
                Observed::Missing => {
                    drop_entries.push(e.path.clone());
                    report.converge.record(cat, Action::Unchanged);
                    report.line(format!("{} already gone", r.path.display()));
                }
                Observed::Content(ref sha) if sha == &e.sha256 => {
                    if !ctx.dry_run {
                        std::fs::remove_file(&r.path).map_err(|err| {
                            OmmError::io(format!("remove {}", r.path.display()), err)
                        })?;
                        prune_empty_dirs(&ws, r.path.parent(), &[routing::WS_SKILLS_DIR, ".omm"]);
                        audit.append(
                            &Event::new(audit::ACTION_UNINSTALL)
                                .at(Base::Workspace, &e.path)
                                .before(Some(&e.sha256))
                                .note(WRITER_DISABLE),
                        )?;
                    }
                    drop_entries.push(e.path.clone());
                    if let Some(id) = e
                        .path
                        .as_str()
                        .strip_prefix(&prefix)
                        .and_then(|p| p.split('/').next())
                    {
                        if !removed_ids.contains(&id.to_string()) {
                            removed_ids.push(id.to_string());
                        }
                    }
                    report.converge.record(cat, Action::Removed);
                    report.line(format!(
                        "{} {}",
                        r.path.display(),
                        if ctx.dry_run {
                            "would be removed"
                        } else {
                            "removed"
                        }
                    ));
                }
                _ => {
                    report.converge.record(cat, Action::Skipped);
                    report.line(format!(
                        "{} kept: not the bytes omm wrote (edited since); the ledger still lists it",
                        r.path.display()
                    ));
                }
            }
        }

        // 2. The hooks file.
        if let Some(e) = l.find(Base::Workspace, &hooks_rel) {
            let r = bases.resolve(Base::Workspace, &hooks_rel)?;
            let outcome = undo_hooks(ctx, e, &r, &ws, &audit)?;
            report.converge.record(CAT_HOOKS, outcome.0);
            report.line(outcome.1);
            if outcome.2 {
                drop_entries.push(hooks_rel.clone());
            }
        } else {
            let hooks_abs = ws.join(routing::WS_HOOKS_FILE);
            if let Ok(b) = std::fs::read(&hooks_abs) {
                if let Ok(doc) = serde_json::from_slice::<Value>(&b) {
                    if !routing::handler_commands(&doc).is_empty() {
                        ctx.out.warn(format!(
                            "{} carries a router handler the ledger does not record; left as is",
                            hooks_abs.display()
                        ));
                    }
                }
            }
        }
        if !ctx.dry_run && !drop_entries.is_empty() {
            let drop = drop_entries.clone();
            modify_ledger(ctx, |l| {
                for r in &drop {
                    l.remove(Base::Workspace, r);
                }
                Ok(())
            })?;
        }
    }

    // 3. `$OMM/config.json`.
    let config_path = OmmConfig::path(&omm_root);
    if config_path.exists() {
        let mut config = OmmConfig::load(&omm_root)?;
        let record = config.remove(routing::CONFIG_KEY_ROUTING);
        let was_on = config.skill_routing();
        config.remove(super::CONFIG_KEY_SKILL_ROUTING);
        let created = record
            .as_ref()
            .and_then(|r| r.get(CONFIG_CREATED_KEY))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if record.is_some() || was_on {
            if !ctx.dry_run {
                if created && config.is_empty() {
                    std::fs::remove_file(&config_path).map_err(|e| {
                        OmmError::io(format!("remove {}", config_path.display()), e)
                    })?;
                    report.converge.record(CAT_CONFIG, Action::Removed);
                    report.line(format!(
                        "{} removed (omm enable created it)",
                        config_path.display()
                    ));
                } else {
                    config.save(&omm_root)?;
                    report.converge.record(CAT_CONFIG, Action::Updated);
                    report.line(format!(
                        "{}: skill_routing and routing removed",
                        config_path.display()
                    ));
                }
            } else {
                report.converge.record(CAT_CONFIG, Action::Updated);
            }
        } else {
            report.converge.record(CAT_CONFIG, Action::Unchanged);
        }
    } else {
        report.converge.record(CAT_CONFIG, Action::Unchanged);
    }
    report.field("removed_skills", json!(removed_ids));
    report.line(format!(
        "gates: `omm run` no longer exports {} / {}; unset them in any shell that exported them by hand",
        hr::ENV_ROUTING_GATE,
        hr::ENV_ROUTING_APPLY_GATE
    ));
    Ok(report)
}

/// Undo the hooks file entry: `(converge action, report line, drop the entry)`.
fn undo_hooks(
    ctx: &Ctx,
    e: &Entry,
    r: &Resolved,
    ws: &Path,
    audit: &Audit,
) -> Result<(Action, String, bool)> {
    if r.via_symlink.is_some() {
        return Ok((
            Action::Skipped,
            format!("{} kept: behind a symlink (R2 sentinel)", r.path.display()),
            false,
        ));
    }
    let current = match r.state {
        FsState::File => fsx::read_bytes(&r.path)?,
        FsState::Missing => {
            return Ok((
                Action::Unchanged,
                format!("{} already gone", r.path.display()),
                true,
            ))
        }
        _ => {
            return Ok((
                Action::Skipped,
                format!("{} kept: not a regular file now", r.path.display()),
                false,
            ))
        }
    };
    let created_dirs: Vec<String> = e
        .prior
        .as_ref()
        .and_then(|p| p.get(PRIOR_CREATED_DIRS))
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let unedited = fsx::sha256_bytes(&current) == e.sha256;
    if e.mechanism == Mechanism::Copy && unedited {
        // omm created it and nobody touched it: gone, with the dir it made.
        if !ctx.dry_run {
            std::fs::remove_file(&r.path)
                .map_err(|err| OmmError::io(format!("remove {}", r.path.display()), err))?;
            let stops: Vec<&str> = created_dirs.iter().map(String::as_str).collect();
            prune_empty_dirs(ws, r.path.parent(), &stops);
            audit.append(
                &Event::new(audit::ACTION_UNINSTALL)
                    .at(Base::Workspace, &e.path)
                    .before(Some(&e.sha256))
                    .note(WRITER_DISABLE),
            )?;
        }
        return Ok((
            Action::Removed,
            format!(
                "{} {}",
                r.path.display(),
                if ctx.dry_run {
                    "would be removed"
                } else {
                    "removed"
                }
            ),
            true,
        ));
    }
    // Edited since, or merged into a pre-existing file: take the handler
    // out surgically and restore the pre-omm bytes when that is all that
    // changed.
    let Ok(mut doc) = serde_json::from_slice::<Value>(&current) else {
        return Ok((
            Action::Skipped,
            format!(
                "{} kept: not JSON any more; remove the `omm hook route` handler by hand",
                r.path.display()
            ),
            false,
        ));
    };
    let removed = routing::remove_handler(&mut doc);
    let original = Original::from_prior(e.prior.as_ref());
    let (bytes, what) = match &original {
        Some(o) if o.document().as_ref() == Some(&doc) => {
            (o.bytes.clone(), "restored to its pre-omm bytes")
        }
        _ if removed > 0 => (
            routing::pretty(&doc)?,
            "kept with omm's handler removed (it differs from the pre-omm file)",
        ),
        _ => {
            return Ok((
                Action::Skipped,
                format!("{} kept: no omm handler in it any more", r.path.display()),
                true,
            ))
        }
    };
    if !ctx.dry_run {
        let realpath = fsx::realpath_for_write(&r.path)?;
        fsx::snapshot_backup(&realpath, &ctx.roots.snapshots_dir(), WORKSPACE_BASE)?;
        fsx::write_atomic(&realpath, &bytes)?;
        if let Some(mode) = original.as_ref().and_then(|o| o.mode) {
            omm_ledger::shared::set_mode(&realpath, mode)?;
        }
        audit.append(
            &Event::new(audit::ACTION_UNINSTALL)
                .at(Base::Workspace, &e.path)
                .before(Some(&fsx::sha256_bytes(&current)))
                .after(Some(&fsx::sha256_bytes(&bytes)))
                .note(format!("{WRITER_DISABLE}: {what}")),
        )?;
    }
    Ok((
        Action::Updated,
        format!(
            "{} {}{}",
            r.path.display(),
            if ctx.dry_run { "would be " } else { "" },
            what
        ),
        true,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_workspace_detection_and_empty_dir_pruning() {
        let tmp = tempfile::tempdir().unwrap();
        let ws = std::fs::canonicalize(tmp.path()).unwrap();
        assert!(!is_git_workspace(&ws));
        std::fs::create_dir(ws.join(".git")).unwrap();
        assert!(is_git_workspace(&ws));
        let deep = ws.join(".omm/skills/omm-a");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::create_dir_all(ws.join(".omm/other")).unwrap();
        prune_empty_dirs(&ws, Some(&deep), &[".omm/skills", ".omm"]);
        assert!(!ws.join(".omm/skills").exists(), "empty chain removed");
        assert!(ws.join(".omm/other").exists(), "a sibling keeps .omm alive");
        std::fs::remove_dir(ws.join(".omm/other")).unwrap();
        prune_empty_dirs(&ws, Some(&ws.join(".omm")), &[".omm"]);
        assert!(!ws.join(".omm").exists());
        // Never past the stops, never the workspace.
        std::fs::create_dir_all(ws.join("keep/me")).unwrap();
        prune_empty_dirs(&ws, Some(&ws.join("keep/me")), &[".omm"]);
        assert!(ws.join("keep/me").exists());
        prune_empty_dirs(&ws, Some(&ws), &[".omm"]);
        assert!(ws.exists());
    }
}
