//! `omm memory {path, seed, list, backup, gc}` — the host's `personal_project`
//! memory root of a workspace (PLAN.md 2.3; ARCHITECTURE.md §7).
//!
//! Host facts (host-reality.md "Paths": memory personal_project; "Budgets":
//! memory snapshot; `research/experiments/loose-ends.md` §2, proven by
//! prediction on a never-used path): the root is
//! `$XDG_DATA_HOME/muse/memory/projects/<slug96>-<fnv1a64hex>/` —
//! `omm_host::paths::personal_project_dir_name` is the formula — and the
//! order-`u32::MAX` `memory_snapshot` block renders its `MEMORY.md` first,
//! then the names of its other Markdown files, capped at 16,305 B / 48
//! listed files per scope, silently. host-reality.md lists the block as
//! trust-gated (loose-ends.md §2.3: "on an untrusted workspace it is not
//! emitted at all"), and the workspace's own skills, hooks and rules are
//! (doctor D12), so `seed` names `omm trust` when the workspace is not
//! trusted. Measured 2026-09-03 on 1.0.1-R2006.1 (`muse exec --provider
//! echo hi`, no `--trust-workspace`): the `personal_project` scope of the
//! snapshot renders a seeded `MEMORY.md` in EVERY trust state — a
//! `trust.json` entry `trusted`, `untrusted`, absent, and no store at all
//! — so the read side of the block is not gated on this build; the test
//! `seeded_memory_composes_the_order_max_block…` locks that, and the
//! host-reality row is reported for correction (the `add_memory` write
//! side is untested here). A non-Markdown sidecar in the root is neither
//! listed nor an error.
//!
//! The slug is lossy (`/a.b` and `/a-b` share one; `omm-w1` and `omm/w2`
//! share one) and the hash is one way, so the workspace behind a root is
//! known only when something recorded it: the sidecar `omm-workspace.json`
//! that `seed` writes beside `MEMORY.md`, or a `trust.json` key / the current
//! directory whose formula reproduces the directory name exactly (a
//! full-hash hit, never a slug guess). `gc` removes a root only on the
//! sidecar's word, and only after a tar of it landed under
//! `$OMM/snapshots/memory/`.
//!
//! Ownership: omm owns nothing inside a memory root after seeding. `seed`
//! writes `MEMORY.md` only when it is missing — never over a file, whatever
//! it holds — and ledgers it as `class: seeded` under `muse-data`, so
//! uninstall removes it only while byte-identical to the seed
//! (`omm_ledger::uninstall`: an edited file is preserved and named); the
//! sidecar is `exclusive`. The ledger's `kind` set has no memory variant
//! (a schema change outside this task): both entries carry
//! [`LEDGER_KIND`], the one kind no module interprets, so the uninstall
//! planner's plain file rule applies and names an edit as an edit (the
//! `rules` kind would route the file through the managed-region logic of
//! `AGENTS.md` and word the preserved file as a marker problem).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use omm_host::fsx;
use omm_host::host_reality as hr;
use omm_host::paths::personal_project_dir_name;
use omm_host::trust::TrustStore;
use omm_ledger::audit::{self, Audit, Event};
use omm_ledger::reconcile::{self, Outcome};
use omm_ledger::{hash, Base, Class, Entry, Kind, Ledger, Mechanism, Observed, RelPath};

use super::c_tune::{modify_ledger, read_ledger, rel, thousands, WriteReport};
use super::memory_tar;
use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Action, Table};

/// The file the memory snapshot renders first (loose-ends.md §2.3).
pub const MEMORY_FILE: &str = "MEMORY.md";
/// The sidecar `seed` writes beside it: the absolute workspace this root
/// belongs to, which the lossy slug and the one-way hash cannot give back.
pub const SIDECAR_FILE: &str = "omm-workspace.json";
/// `schema_version` of the sidecar.
pub const SIDECAR_SCHEMA_VERSION: u64 = 1;
/// The lock the host keeps in every root it writes to (loose-ends.md §2.2);
/// never counted against the caps.
pub const HOST_LOCK_FILE: &str = ".muse-memory.lock";
/// The seeded `MEMORY.md`. No managed region, no marker: once it lands the
/// file is the user's (and the agent's) alone.
pub const SEED_TEMPLATE: &str = "# Project memory\n\nNotes kept across sessions for this workspace: decisions, conventions, things\nthat took time to find out. Rewrite freely; nothing here is managed by omm.\n";
/// Converge category of the seeded files and the removed roots.
pub const CATEGORY: &str = "memory";
/// Converge category of the tar backups.
pub const CATEGORY_BACKUPS: &str = "memory-backups";
/// `$OMM/snapshots/<this>/<ts>/<dir name>.tar` — rolls with
/// `fsx::SNAPSHOTS_KEEP`, separately from the ledger's snapshots.
pub const SNAPSHOT_SUBDIR: &str = "memory";
/// The `writer` recorded on the ledger entries.
pub const WRITER: &str = "omm memory seed";
/// The kind the ledger records for both files until the schema gains a
/// `memory` kind (see the module docs): `agent` is interpreted by no
/// module — `rules` is, by the uninstall planner. Flip this one constant
/// when the schema changes.
pub const LEDGER_KIND: Kind = Kind::Agent;

// ---- the sidecar --------------------------------------------------------

/// `omm-workspace.json`: what `seed` knows and the directory name cannot say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sidecar {
    pub schema_version: u64,
    /// The canonical workspace root (the host's `trust.json` key and the
    /// input of the directory formula).
    pub workspace: String,
    pub writer: String,
}

/// What reading a sidecar found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SidecarRead {
    Absent,
    Ok(Sidecar),
    /// Present but unusable, with why.
    Bad(String),
}

impl Sidecar {
    /// The sidecar for a canonical workspace.
    pub fn new(workspace: &Path) -> Sidecar {
        Sidecar {
            schema_version: SIDECAR_SCHEMA_VERSION,
            workspace: workspace.to_string_lossy().into_owned(),
            writer: WRITER.to_string(),
        }
    }

    /// 2-space pretty JSON with a trailing newline.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|e| OmmError::Usage(format!("{SIDECAR_FILE}: {e}")))?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Read `<root>/omm-workspace.json`.
    pub fn read(root: &Path) -> SidecarRead {
        let path = root.join(SIDECAR_FILE);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return SidecarRead::Absent,
            Err(e) => return SidecarRead::Bad(format!("stat: {e}")),
        };
        if !meta.is_file() {
            return SidecarRead::Bad("not a regular file".to_string());
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) => return SidecarRead::Bad(format!("read: {e}")),
        };
        match serde_json::from_slice::<Sidecar>(&bytes) {
            Ok(s) if s.schema_version == SIDECAR_SCHEMA_VERSION => SidecarRead::Ok(s),
            Ok(s) => SidecarRead::Bad(format!(
                "schema_version {} is not {SIDECAR_SCHEMA_VERSION}",
                s.schema_version
            )),
            Err(e) => SidecarRead::Bad(format!("not a sidecar: {e}")),
        }
    }

    /// The workspace this sidecar names — accepted only when it is absolute
    /// and the formula reproduces `dir_name` from it (a hand-edited or
    /// misplaced sidecar is refused with the reason, never trusted).
    pub fn workspace_for(&self, dir_name: &str) -> std::result::Result<PathBuf, String> {
        let ws = PathBuf::from(&self.workspace);
        if !ws.is_absolute() {
            return Err(format!(
                "sidecar workspace {:?} is not absolute",
                self.workspace
            ));
        }
        let predicted = personal_project_dir_name(&ws);
        if predicted != dir_name {
            return Err(format!(
                "sidecar names {}, whose root would be {predicted}, not this directory",
                ws.display()
            ));
        }
        Ok(ws)
    }
}

// ---- naming and usage ---------------------------------------------------

/// `<slug96>-<fnv1a64hex>` → `(slug, hash)`; `None` for a name the formula
/// cannot have produced.
pub fn parse_dir_name(name: &str) -> Option<(String, String)> {
    let (slug, hash) = name.rsplit_once('-')?;
    let hash_ok = hash.len() == 16
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let slug_ok = !slug.is_empty()
        && slug.len() <= 96
        && slug.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    (hash_ok && slug_ok).then(|| (slug.to_string(), hash.to_string()))
}

/// The directory name of a root path.
fn dir_name_of(root: &Path) -> Result<String> {
    root.file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| OmmError::Usage(format!("{} has no directory name", root.display())))
}

/// What the caps measure: the Markdown files the snapshot lists and their
/// bytes (the host's lock file and other files are counted apart).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub md_files: usize,
    pub md_bytes: u64,
    pub other_files: usize,
}

impl Usage {
    /// Past either silent cap.
    pub fn over_cap(&self) -> bool {
        self.md_bytes > hr::MEMORY_SNAPSHOT_BYTES || self.md_files > hr::MEMORY_LISTED_FILES_MAX
    }
    /// `n/48`.
    pub fn files_cell(&self) -> String {
        format!("{}/{}", self.md_files, hr::MEMORY_LISTED_FILES_MAX)
    }
    /// `1,204/16,305`.
    pub fn bytes_cell(&self) -> String {
        format!(
            "{}/{}",
            thousands(self.md_bytes),
            thousands(hr::MEMORY_SNAPSHOT_BYTES)
        )
    }
}

/// Count the regular files under `root` (recursive, links never followed).
pub fn usage(root: &Path) -> Usage {
    let mut u = Usage::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(ft) = entry.file_type() else {
                continue;
            };
            if ft.is_dir() {
                stack.push(entry.path());
            } else if ft.is_file() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name == HOST_LOCK_FILE {
                    continue;
                }
                let is_md = name
                    .rsplit_once('.')
                    .map(|(_, ext)| ext.eq_ignore_ascii_case("md"))
                    .unwrap_or(false);
                if is_md {
                    u.md_files += 1;
                    u.md_bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
                } else {
                    u.other_files += 1;
                }
            }
        }
    }
    u
}

// ---- scanning the projects dir -------------------------------------------

/// How a root's workspace became known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Via {
    /// The sidecar `seed` wrote (the only source `gc` acts on).
    Sidecar,
    /// A `trust.json` key whose formula reproduces the name.
    Trust,
    /// The current directory reproduces the name.
    Cwd,
    /// Nothing recorded it: slug only.
    Unknown,
}

impl Via {
    pub fn as_str(self) -> &'static str {
        match self {
            Via::Sidecar => "sidecar",
            Via::Trust => "trust.json",
            Via::Cwd => "cwd",
            Via::Unknown => "-",
        }
    }
}

/// One `projects/<dir>` as `list` and `gc` see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootInfo {
    pub name: String,
    pub path: PathBuf,
    /// `(slug, hash)` when the name is formula-shaped.
    pub parsed: Option<(String, String)>,
    pub workspace: Option<PathBuf>,
    pub via: Via,
    /// A symlink, an unrecognised name, a bad or disagreeing sidecar.
    pub note: Option<String>,
    pub usage: Usage,
}

impl RootInfo {
    /// Whether the workspace exists on disk (`None` when unknown).
    pub fn workspace_exists(&self) -> Option<bool> {
        self.workspace
            .as_deref()
            .map(|ws| fs::symlink_metadata(ws).is_ok())
    }
}

/// The trust store's project keys, each mapped to the directory name its
/// formula produces (a full-hash hit). Read errors become a warning.
fn known_from_trust(ctx: &Ctx) -> BTreeMap<String, PathBuf> {
    let store = match TrustStore::for_roots(&ctx.roots) {
        Ok(s) => s,
        Err(e) => {
            ctx.out.warn(format!("trust.json could not be read: {e}"));
            return BTreeMap::new();
        }
    };
    store
        .value()
        .get("projects")
        .and_then(Value::as_object)
        .map(|projects| {
            projects
                .keys()
                .map(|key| {
                    let ws = PathBuf::from(key);
                    (personal_project_dir_name(&ws), ws)
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Every `projects/<dir>`, sorted by name. `cwd` is the current workspace
/// when the command runs in one (a third full-hash source).
pub fn scan(ctx: &Ctx, cwd: Option<&Path>) -> Result<Vec<RootInfo>> {
    let projects = ctx.roots.memory_projects_dir();
    let entries = match fs::read_dir(&projects) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(OmmError::io(format!("read {}", projects.display()), e)),
    };
    let trusted = known_from_trust(ctx);
    let cwd_name = cwd.map(personal_project_dir_name);
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| OmmError::io(format!("read {}", projects.display()), e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| OmmError::io(format!("stat {}", path.display()), e))?;
        let parsed = parse_dir_name(&name);
        let mut info = RootInfo {
            name: name.clone(),
            path: path.clone(),
            parsed: parsed.clone(),
            workspace: None,
            via: Via::Unknown,
            note: None,
            usage: Usage::default(),
        };
        if meta.file_type().is_symlink() {
            info.note = Some("symlink; never followed".to_string());
            out.push(info);
            continue;
        }
        if !meta.is_dir() {
            info.note = Some("not a directory".to_string());
            out.push(info);
            continue;
        }
        info.usage = usage(&path);
        if parsed.is_none() {
            info.note = Some("name is not <slug96>-<fnv1a64hex>".to_string());
        }
        match Sidecar::read(&path) {
            SidecarRead::Ok(s) => match s.workspace_for(&name) {
                Ok(ws) => {
                    info.workspace = Some(ws);
                    info.via = Via::Sidecar;
                }
                Err(why) => info.note = Some(why),
            },
            SidecarRead::Bad(why) => info.note = Some(format!("{SIDECAR_FILE}: {why}")),
            SidecarRead::Absent => {}
        }
        if info.workspace.is_none() {
            if let Some(ws) = trusted.get(&name) {
                info.workspace = Some(ws.clone());
                info.via = Via::Trust;
            } else if cwd_name.as_deref() == Some(name.as_str()) {
                info.workspace = cwd.map(Path::to_path_buf);
                info.via = Via::Cwd;
            }
        }
        out.push(info);
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// Slugs shared by more than one root: `(slug, names)`. The hash keeps them
/// apart on disk; the slug alone would not.
pub fn collisions(roots: &[RootInfo]) -> Vec<(String, Vec<String>)> {
    let mut by_slug: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for r in roots {
        if let Some((slug, _)) = &r.parsed {
            by_slug
                .entry(slug.clone())
                .or_default()
                .push(r.name.clone());
        }
    }
    by_slug.into_iter().filter(|(_, v)| v.len() > 1).collect()
}

// ---- path ---------------------------------------------------------------

/// `trust.json`'s decision for a workspace; a read error is a warning.
pub fn trust_decision(ctx: &Ctx, workspace: &Path) -> Option<String> {
    match TrustStore::for_roots(&ctx.roots) {
        Ok(store) => store.decision_for(workspace).ok().flatten(),
        Err(e) => {
            ctx.out.warn(format!("trust.json could not be read: {e}"));
            None
        }
    }
}

/// The `omm trust` fix for an untrusted workspace.
fn trust_hint(workspace: &Path) -> String {
    format!("omm trust {}", workspace.display())
}

/// `omm memory path`: the resolved root, whether it exists, the trust state.
pub fn path_report(ctx: &Ctx, workspace: &Path) -> Result<(String, Value)> {
    let path = ctx.roots.memory_personal_project_dir(workspace)?;
    let trusted = trust_decision(ctx, workspace);
    let exists = path.is_dir();
    let text = format!(
        "{}\nworkspace: {}\nexists: {}\ntrust: {}",
        path.display(),
        workspace.display(),
        if exists { "yes" } else { "no" },
        trusted.clone().unwrap_or_else(|| {
            format!(
                "not in trust.json (host-reality lists the memory block as trust-gated: {})",
                trust_hint(workspace)
            )
        })
    );
    let json = json!({
        "path": path.display().to_string(),
        "workspace": workspace.display().to_string(),
        "exists": exists,
        "trust": trusted,
    });
    Ok((text, json))
}

// ---- seed ---------------------------------------------------------------

/// What `seed` decided for one file.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Verdict {
    Write,
    /// Identical bytes already on disk, not yet ledgered.
    Adopt,
    Unchanged,
    Skip(String),
}

impl Verdict {
    fn label(&self) -> &'static str {
        match self {
            Verdict::Write => "written",
            Verdict::Adopt => "adopted",
            Verdict::Unchanged => "unchanged",
            Verdict::Skip(_) => "skipped",
        }
    }
    fn action(&self) -> Action {
        match self {
            Verdict::Write | Verdict::Adopt => Action::Updated,
            Verdict::Unchanged => Action::Unchanged,
            Verdict::Skip(_) => Action::Skipped,
        }
    }
}

/// `MEMORY.md`: written only when missing; identical bytes are adopted or
/// already ours; anything else is the user's and is never touched.
fn memory_verdict(observed: &Observed, seed_sha: &str, ledgered: Option<&str>) -> Verdict {
    match observed {
        Observed::Missing => Verdict::Write,
        Observed::Content(sha) if sha == seed_sha => {
            if ledgered == Some(seed_sha) {
                Verdict::Unchanged
            } else {
                Verdict::Adopt
            }
        }
        Observed::Content(_) => Verdict::Skip("holds your content; never overwritten".into()),
        Observed::NonRegular(s) => Verdict::Skip(format!("not a regular file ({s})")),
    }
}

/// The sidecar is omm's own: the R3 rule (`reconcile::decide`) applies —
/// rewritten while untouched, kept when edited, adopted when identical —
/// except that a missing one is always written back.
fn sidecar_verdict(observed: &Observed, theirs: &str, ledgered: Option<&str>) -> Verdict {
    if observed.is_missing() {
        return Verdict::Write;
    }
    match reconcile::decide(ledgered, theirs, observed) {
        (Outcome::NoOp, _) => Verdict::Unchanged,
        (Outcome::Adopt, _) => Verdict::Adopt,
        (Outcome::Overwrite, _) => Verdict::Write,
        (Outcome::Stage, reason) => Verdict::Skip(reason),
    }
}

/// The realpath a write into a memory root lands on, contained in the
/// canonical `memory/projects/` and never through a symlinked root.
fn contained_realpath(ctx: &Ctx, root: &Path, file: &Path) -> Result<PathBuf> {
    let projects = fsx::canonicalize(&ctx.roots.memory_projects_dir())?;
    let root_meta = fs::symlink_metadata(root)
        .map_err(|e| OmmError::io(format!("stat {}", root.display()), e))?;
    if root_meta.file_type().is_symlink() {
        return Err(OmmError::Usage(format!(
            "{} is a symlink — not what omm would create; refusing to write through it",
            root.display()
        )));
    }
    let realpath = fsx::realpath_for_write(file)?;
    if realpath == projects || !realpath.starts_with(&projects) {
        return Err(OmmError::Usage(format!(
            "{} resolves outside {} — refusing to write",
            file.display(),
            projects.display()
        )));
    }
    Ok(realpath)
}

/// `memory/projects/<dir>/` as a `/`-joined string — the ledger prefix of a
/// root under `muse-data`.
fn ledger_prefix(ctx: &Ctx, name: &str) -> Result<String> {
    let projects = ctx.roots.memory_projects_dir();
    let data = ctx.roots.muse_data();
    let under = projects
        .strip_prefix(&data)
        .map_err(|_| OmmError::Usage("memory/projects is not under the data root".into()))?;
    let mut parts: Vec<String> = under
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.push(name.to_string());
    parts.push(String::new());
    Ok(parts.join("/"))
}

/// The ledger path of a file in a root: `memory/projects/<dir>/<file>`.
fn ledger_rel(ctx: &Ctx, name: &str, file: &str) -> Result<RelPath> {
    rel(&format!("{}{file}", ledger_prefix(ctx, name)?))
}

/// The ledger's sha for an entry, if any.
fn ledgered_sha(ledger: Option<&Ledger>, path: &RelPath) -> Option<String> {
    ledger.and_then(|l| l.find(Base::MuseData, path).map(|e| e.sha256.clone()))
}

/// `omm memory seed` for `workspace` (the canonical current directory).
pub fn seed(ctx: &Ctx, workspace: &Path) -> Result<WriteReport> {
    let root = ctx.roots.memory_personal_project_dir(workspace)?;
    let name = dir_name_of(&root)?;
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("path", json!(root.display().to_string()));
    report.field("workspace", json!(workspace.display().to_string()));

    let trust = trust_decision(ctx, workspace);
    report.field("trust", json!(trust));
    if trust.as_deref() != Some("trusted") {
        let hint = trust_hint(workspace);
        ctx.out.warn(format!(
            "{} is not trusted in {}: host-reality lists the memory block as trust-gated, and the workspace's skills, hooks and rules are (doctor D12); run `{hint}`",
            workspace.display(),
            ctx.roots.trust_file().display()
        ));
        report.line(format!(
            "trust: {} — run `{hint}`",
            trust.as_deref().unwrap_or("not in trust.json")
        ));
    }

    let root_existed = root.is_dir();
    if !ctx.dry_run && !root_existed {
        fsx::create_dir_all(&root)?;
    }
    report.field("root_created", json!(!root_existed));
    report.line(format!(
        "root {}: {}",
        root.display(),
        match (root_existed, ctx.dry_run) {
            (true, _) => "exists",
            (false, true) => "would be created",
            (false, false) => "created",
        }
    ));

    let ledger = read_ledger(ctx)?;
    let seed_bytes = SEED_TEMPLATE.as_bytes();
    let seed_sha = fsx::sha256_bytes(seed_bytes);
    let sidecar_bytes = Sidecar::new(workspace).to_bytes()?;
    let sidecar_sha = fsx::sha256_bytes(&sidecar_bytes);
    let memory_rel = ledger_rel(ctx, &name, MEMORY_FILE)?;
    let sidecar_rel = ledger_rel(ctx, &name, SIDECAR_FILE)?;
    let memory_path = root.join(MEMORY_FILE);
    let sidecar_path = root.join(SIDECAR_FILE);

    let memory_seen = hash::observe(&memory_path)?;
    let sidecar_seen = hash::observe(&sidecar_path)?;
    let memory_v = memory_verdict(
        &memory_seen,
        &seed_sha,
        ledgered_sha(ledger.as_ref(), &memory_rel).as_deref(),
    );
    let sidecar_v = sidecar_verdict(
        &sidecar_seen,
        &sidecar_sha,
        ledgered_sha(ledger.as_ref(), &sidecar_rel).as_deref(),
    );
    if let Verdict::Skip(why) = &sidecar_v {
        // A sidecar that is not ours and names another workspace is a hash
        // collision or a hand edit: said loudly, never overwritten.
        let names = match Sidecar::read(&root) {
            SidecarRead::Ok(s) if s.workspace != workspace.to_string_lossy() => {
                format!(" — it names {}", s.workspace)
            }
            _ => String::new(),
        };
        ctx.out.warn(format!(
            "{} left as is ({why}){names}",
            sidecar_path.display()
        ));
    }

    let plans = [
        (
            MEMORY_FILE,
            &memory_path,
            &memory_rel,
            seed_bytes,
            &seed_sha,
            &memory_v,
            Class::Seeded,
            &memory_seen,
        ),
        (
            SIDECAR_FILE,
            &sidecar_path,
            &sidecar_rel,
            sidecar_bytes.as_slice(),
            &sidecar_sha,
            &sidecar_v,
            Class::Exclusive,
            &sidecar_seen,
        ),
    ];
    let mut to_ledger: Vec<(RelPath, String, Class, Option<String>, &'static str)> = Vec::new();
    for (file, path, rel_path, bytes, sha, verdict, class, seen) in plans {
        let what = match verdict {
            Verdict::Write if ctx.dry_run => "would be written".to_string(),
            Verdict::Write => {
                let realpath = contained_realpath(ctx, &root, path)?;
                fsx::write_atomic(&realpath, bytes)?;
                format!("written ({} B)", bytes.len())
            }
            Verdict::Adopt => "already holds these bytes; recorded".to_string(),
            Verdict::Unchanged => "unchanged".to_string(),
            Verdict::Skip(why) => format!("skipped ({why})"),
        };
        report.line(format!("{file}: {what}"));
        report.field(
            &Table::key(file),
            json!({"path": path.display().to_string(), "outcome": verdict.label()}),
        );
        report.converge.record(CATEGORY, verdict.action());
        if !ctx.dry_run && matches!(verdict, Verdict::Write | Verdict::Adopt) {
            let before = match seen {
                Observed::Content(s) => Some(s.clone()),
                _ => None,
            };
            let note = if matches!(verdict, Verdict::Adopt) {
                "adopted, identical bytes on disk"
            } else {
                "seeded"
            };
            to_ledger.push((rel_path.clone(), sha.clone(), class, before, note));
        }
    }

    if !to_ledger.is_empty() {
        let entries = to_ledger.clone();
        modify_ledger(ctx, move |ledger| {
            for (path, sha256, class, _, _) in entries {
                ledger.upsert(Entry {
                    base: Base::MuseData,
                    path,
                    kind: LEDGER_KIND,
                    sha256,
                    source_version: OMM_VERSION.to_string(),
                    writer: WRITER.to_string(),
                    mechanism: Mechanism::Copy,
                    class,
                    prior: None,
                });
            }
            Ok(())
        })?;
        let audit = Audit::new(&ctx.omm_root(), OMM_VERSION);
        for (path, sha256, _, before, note) in &to_ledger {
            audit.append(
                &Event::new(audit::ACTION_INSTALL)
                    .at(Base::MuseData, path)
                    .before(before.as_deref())
                    .after(Some(sha256))
                    .note(format!("{WRITER}: {note}")),
            )?;
        }
    }
    Ok(report)
}

// ---- list ---------------------------------------------------------------

/// `omm memory list`: every root with the workspace it maps to, the source
/// of that mapping, and the Markdown files / bytes against the silent caps.
/// Slug collisions and over-cap roots are warnings.
pub fn list(ctx: &Ctx, cwd: Option<&Path>) -> Result<Table> {
    let projects = ctx.roots.memory_projects_dir();
    ctx.out
        .note(format!("memory roots: {}", projects.display()));
    let roots = scan(ctx, cwd)?;
    let mut table = Table::new([
        "root",
        "workspace",
        "via",
        "md files",
        "bytes",
        "workspace exists",
        "note",
    ])
    .right(3)
    .right(4);
    for r in &roots {
        let mut note = r.note.clone().unwrap_or_default();
        if r.usage.over_cap() {
            note = if note.is_empty() {
                "over the silent cap".to_string()
            } else {
                format!("{note}; over the silent cap")
            };
            ctx.out.warn(format!(
                "{}: {} B / {} Markdown files exceed the silent memory caps ({} B / {} files); the snapshot is cut without notice",
                r.path.display(),
                thousands(r.usage.md_bytes),
                r.usage.md_files,
                thousands(hr::MEMORY_SNAPSHOT_BYTES),
                hr::MEMORY_LISTED_FILES_MAX
            ));
        }
        table.row([
            r.name.clone(),
            r.workspace
                .as_ref()
                .map(|w| w.display().to_string())
                .unwrap_or_else(|| "slug only".to_string()),
            r.via.as_str().to_string(),
            r.usage.files_cell(),
            r.usage.bytes_cell(),
            match r.workspace_exists() {
                Some(true) => "yes",
                Some(false) => "no",
                None => "-",
            }
            .to_string(),
            note,
        ]);
    }
    for (slug, names) in collisions(&roots) {
        ctx.out.warn(format!(
            "slug collision: {} roots share the slug {slug:?} and are told apart by the hash alone ({})",
            names.len(),
            names.join(", ")
        ));
    }
    Ok(table)
}

// ---- backup -------------------------------------------------------------

/// `$OMM/snapshots/memory/`.
fn backups_dir(ctx: &Ctx) -> PathBuf {
    ctx.roots.snapshots_dir().join(SNAPSHOT_SUBDIR)
}

/// A tar of `root` written atomically to `file` and verified by SHA-256.
fn write_tar(root: &Path, name: &str, file: &Path) -> Result<memory_tar::Archive> {
    let archive = memory_tar::archive(root, name)?;
    let realpath = fsx::realpath_for_write(file)?;
    fsx::write_atomic(&realpath, &archive.bytes)?;
    let landed = fsx::sha256_file(&realpath)?;
    if landed != fsx::sha256_bytes(&archive.bytes) {
        let _ = fs::remove_file(&realpath);
        return Err(OmmError::Usage(format!(
            "{} did not verify after writing; removed",
            realpath.display()
        )));
    }
    Ok(archive)
}

/// `<dir>/<name>-<ts>.tar`, with `-<n>` when taken.
fn unique_tar_path(dir: &Path, name: &str) -> PathBuf {
    let stamp = fsx::timestamp();
    let mut n = 0u32;
    loop {
        let candidate = if n == 0 {
            dir.join(format!("{name}-{stamp}.tar"))
        } else {
            dir.join(format!("{name}-{stamp}-{n}.tar"))
        };
        if fs::symlink_metadata(&candidate).is_err() {
            return candidate;
        }
        n += 1;
    }
}

/// `omm memory backup [--to <dir>]` for `workspace`: a tar of its root under
/// `$OMM/snapshots/memory/<ts>/` (rolling five) or in `--to`.
pub fn backup(ctx: &Ctx, workspace: &Path, to: Option<&Path>) -> Result<WriteReport> {
    let root = ctx.roots.memory_personal_project_dir(workspace)?;
    let name = dir_name_of(&root)?;
    if !root.is_dir() {
        return Err(OmmError::Usage(format!(
            "no memory root for {} at {}: nothing to back up (`omm memory seed` creates one)",
            workspace.display(),
            root.display()
        )));
    }
    let root_real = fsx::canonicalize(&root)?;
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("path", json!(root.display().to_string()));
    report.field("workspace", json!(workspace.display().to_string()));

    let dest = match to {
        Some(dir) => {
            if !dir.is_dir() {
                return Err(OmmError::Usage(format!(
                    "--to {}: not an existing directory",
                    dir.display()
                )));
            }
            let real = fsx::canonicalize(dir)?;
            if real.starts_with(&root_real) {
                return Err(OmmError::Usage(format!(
                    "--to {}: inside the memory root being backed up",
                    dir.display()
                )));
            }
            real
        }
        None => backups_dir(ctx),
    };
    let rolling = to.is_none();
    let usage = usage(&root);
    if ctx.dry_run {
        let where_ = if rolling {
            format!("{}/<ts>/{name}.tar", dest.display())
        } else {
            format!("{}/{name}-<ts>.tar", dest.display())
        };
        report.line(format!(
            "would archive {} ({} Markdown files, {} B) to {where_}{}",
            root.display(),
            usage.md_files,
            thousands(usage.md_bytes),
            if rolling {
                format!(" (newest {} kept)", fsx::SNAPSHOTS_KEEP)
            } else {
                String::new()
            }
        ));
        report.field("backup", json!(where_));
        report.converge.record(CATEGORY_BACKUPS, Action::Updated);
        return Ok(report);
    }

    let file = if rolling {
        fsx::new_snapshot_dir(&dest)?.join(format!("{name}.tar"))
    } else {
        unique_tar_path(&dest, &name)
    };
    let archive = write_tar(&root, &name, &file)?;
    for (rel_path, why) in &archive.skipped {
        ctx.out.warn(format!(
            "{}/{rel_path}: left out of the backup ({why})",
            root.display()
        ));
    }
    let pruned = if rolling {
        fsx::prune_snapshots(&dest, fsx::SNAPSHOTS_KEEP)?
    } else {
        Vec::new()
    };
    Audit::new(&ctx.omm_root(), OMM_VERSION).append(
        &Event::new(audit::ACTION_SNAPSHOT)
            .after(Some(&fsx::sha256_bytes(&archive.bytes)))
            .note(format!(
                "omm memory backup: {} -> {} ({} files, {} B{})",
                root.display(),
                file.display(),
                archive.files,
                archive.file_bytes,
                if rolling {
                    String::new()
                } else {
                    "; outside $OMM, not rolled".to_string()
                }
            )),
    )?;
    report.line(format!(
        "{} -> {} ({} files, {} B in {} B of tar)",
        root.display(),
        file.display(),
        archive.files,
        thousands(archive.file_bytes),
        thousands(archive.bytes.len() as u64)
    ));
    for p in &pruned {
        report.line(format!("rolled off: {}", p.display()));
    }
    report.field("backup", json!(file.display().to_string()));
    report.field("files", json!(archive.files));
    report.field("file_bytes", json!(archive.file_bytes));
    report.field("tar_bytes", json!(archive.bytes.len()));
    report.field(
        "skipped",
        json!(archive
            .skipped
            .iter()
            .map(|(p, w)| json!({"path": p, "why": w}))
            .collect::<Vec<_>>()),
    );
    report.field(
        "pruned",
        json!(pruned
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()),
    );
    report.converge.record(CATEGORY_BACKUPS, Action::Updated);
    report
        .converge
        .record_n(CATEGORY_BACKUPS, Action::Removed, pruned.len());
    Ok(report)
}

// ---- gc -----------------------------------------------------------------

/// Why a root is kept by `gc`.
fn keep_reason(r: &RootInfo) -> String {
    match (r.via, &r.workspace) {
        (Via::Sidecar, Some(ws)) => match fs::symlink_metadata(ws) {
            Ok(_) => format!("workspace {} exists", ws.display()),
            Err(e) => format!("workspace {} cannot be checked: {e}", ws.display()),
        },
        (Via::Trust, Some(ws)) | (Via::Cwd, Some(ws)) => format!(
            "no sidecar; the mapping to {} comes from {} and is not acted on",
            ws.display(),
            r.via.as_str()
        ),
        _ => match &r.note {
            Some(note) => note.clone(),
            None => "no sidecar names its workspace (the slug is lossy); never guessed".to_string(),
        },
    }
}

/// A root `gc` may remove: its sidecar names a workspace that is gone.
fn removable(r: &RootInfo) -> bool {
    matches!((r.via, &r.workspace), (Via::Sidecar, Some(ws))
        if matches!(fs::symlink_metadata(ws), Err(e) if e.kind() == std::io::ErrorKind::NotFound))
}

/// Drop every ledger entry under `memory/projects/<name>/`; the count.
fn forget_root(ctx: &Ctx, name: &str) -> Result<usize> {
    let prefix = ledger_prefix(ctx, name)?;
    let has_any = read_ledger(ctx)?
        .map(|l| {
            l.entries
                .iter()
                .any(|e| e.base == Base::MuseData && e.path.as_str().starts_with(&prefix))
        })
        .unwrap_or(false);
    if !has_any {
        return Ok(0);
    }
    modify_ledger(ctx, move |ledger| {
        let before = ledger.entries.len();
        ledger
            .entries
            .retain(|e| !(e.base == Base::MuseData && e.path.as_str().starts_with(&prefix)));
        Ok(before - ledger.entries.len())
    })
}

/// `omm memory gc [--dry-run]`: remove the roots whose sidecar names a
/// workspace that no longer exists — a tar of each lands under
/// `$OMM/snapshots/memory/<ts>/` first, the ledger forgets their entries,
/// the audit log names them. Everything else is kept and the reason shown.
pub fn gc(ctx: &Ctx) -> Result<WriteReport> {
    let roots = scan(ctx, None)?;
    let mut report = WriteReport::new(ctx.dry_run);
    let projects = fsx::canonicalize(&ctx.roots.memory_projects_dir()).ok();
    let mut removed = Vec::new();
    let mut kept = Vec::new();
    let mut snapshot_dir: Option<PathBuf> = None;
    for r in &roots {
        if !removable(r) {
            let why = keep_reason(r);
            report.line(format!("kept {}: {why}", r.name));
            kept.push(json!({"root": r.name, "why": why}));
            report.converge.record(CATEGORY, Action::Skipped);
            continue;
        }
        let ws = r.workspace.clone().unwrap_or_default();
        if ctx.dry_run {
            report.line(format!(
                "would remove {} (workspace {} is gone; a tar goes to {}/<ts>/ first)",
                r.path.display(),
                ws.display(),
                backups_dir(ctx).display()
            ));
            removed.push(json!({"root": r.name, "workspace": ws.display().to_string(), "backup": Value::Null}));
            report.converge.record(CATEGORY, Action::Removed);
            continue;
        }
        // Containment: a real directory directly under the canonical projects dir.
        let real = fsx::canonicalize(&r.path)?;
        let inside = projects
            .as_ref()
            .map(|p| real.parent() == Some(p.as_path()))
            .unwrap_or(false);
        if !inside {
            let why = format!(
                "{} does not resolve directly under memory/projects",
                r.path.display()
            );
            report.line(format!("kept {}: {why}", r.name));
            kept.push(json!({"root": r.name, "why": why}));
            report.converge.record(CATEGORY, Action::Skipped);
            continue;
        }
        let dir = match &snapshot_dir {
            Some(d) => d.clone(),
            None => {
                let d = fsx::new_snapshot_dir(&backups_dir(ctx))?;
                snapshot_dir = Some(d.clone());
                d
            }
        };
        let file = dir.join(format!("{}.tar", r.name));
        let archive = write_tar(&real, &r.name, &file)?;
        report.converge.record(CATEGORY, Action::BackedUp);
        fs::remove_dir_all(&real)
            .map_err(|e| OmmError::io(format!("remove {}", real.display()), e))?;
        let forgotten = forget_root(ctx, &r.name)?;
        Audit::new(&ctx.omm_root(), OMM_VERSION).append(
            &Event::new(audit::ACTION_UNINSTALL)
                .note(format!(
                    "omm memory gc: removed {} (workspace {} is gone); backup {} ({} files, {} B); {forgotten} ledger entries dropped",
                    real.display(),
                    ws.display(),
                    file.display(),
                    archive.files,
                    archive.file_bytes
                )),
        )?;
        report.line(format!(
            "removed {} (workspace {} is gone; backup {}, {} ledger entries dropped)",
            real.display(),
            ws.display(),
            file.display(),
            forgotten
        ));
        removed.push(json!({
            "root": r.name,
            "workspace": ws.display().to_string(),
            "backup": file.display().to_string(),
            "ledger_entries_dropped": forgotten,
        }));
        report.converge.record(CATEGORY, Action::Removed);
    }
    let pruned = if snapshot_dir.is_some() {
        fsx::prune_snapshots(&backups_dir(ctx), fsx::SNAPSHOTS_KEEP)?
    } else {
        Vec::new()
    };
    for p in &pruned {
        report.line(format!("rolled off: {}", p.display()));
    }
    report.field("removed", Value::Array(removed));
    report.field("kept", Value::Array(kept));
    report.field(
        "pruned",
        json!(pruned
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()),
    );
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Flags;
    use crate::output::{Output, Render};
    use omm_host::settings::CommitOptions;
    use omm_host::trust::TrustDecision;
    use omm_host::{Invoker, Sandbox};
    use omm_ledger::{store, HostInfo, Scope};

    /// A sandbox with a canonical workspace and a silent context.
    struct World {
        _tmp: tempfile::TempDir,
        sb: Sandbox,
        ctx: Ctx,
        ws: PathBuf,
    }

    fn flags(dry_run: bool) -> Flags {
        Flags {
            dry_run,
            yes: true,
            ..Flags::default()
        }
    }

    fn world(dry_run: bool) -> World {
        let tmp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        let sb = Sandbox::create(&root.join("sandbox")).unwrap();
        let ws = root.join("ws");
        fs::create_dir_all(&ws).unwrap();
        let mut ctx = Ctx::new(sb.roots().unwrap(), flags(dry_run));
        ctx.out = Output::silent();
        World {
            _tmp: tmp,
            sb,
            ctx,
            ws,
        }
    }

    /// A ledger on disk, so `modify_ledger` never needs the host.
    fn pre_ledger(w: &World) {
        store::save(
            &w.ctx.omm_root(),
            &Ledger::new(
                OMM_VERSION,
                HostInfo {
                    version: "test".into(),
                    sha256: "0".repeat(64),
                },
                Scope::User,
            ),
        )
        .unwrap();
    }

    fn ledger_of(w: &World) -> Ledger {
        store::load(&w.ctx.omm_root())
            .unwrap()
            .into_ledger()
            .expect("ledger")
    }

    fn host_bin() -> Option<PathBuf> {
        std::env::var_os("OMM_MUSE_BIN")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    }

    /// A root created by hand from the formula, no sidecar.
    fn bare_root(w: &World, workspace: &Path) -> PathBuf {
        let root = w
            .ctx
            .roots
            .memory_projects_dir()
            .join(personal_project_dir_name(workspace));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(MEMORY_FILE), b"# mine\n").unwrap();
        root
    }

    #[test]
    fn the_formula_predicts_the_worked_examples_and_a_fresh_sandbox_root() {
        // loose-ends.md §2.2 table, verbatim.
        for (path, want) in [
            ("/private/tmp/omm-w1", "private-tmp-omm-w1-cb608784b444ce8a"),
            ("/private/tmp/omm/w2", "private-tmp-omm-w2-bcf99c84acf0b0a9"),
            (
                "/private/tmp/UPPER_Case+weird#chars",
                "private-tmp-UPPER-Case-weird-chars-013792409fd16a19",
            ),
            (
                "/private/tmp/has space.and.dots",
                "private-tmp-has-space-and-dots-49eb1ace9a5112dd",
            ),
        ] {
            assert_eq!(personal_project_dir_name(Path::new(path)), want, "{path}");
            let (slug, hash) = parse_dir_name(want).expect("formula-shaped");
            assert_eq!(format!("{slug}-{hash}"), want);
        }
        // A never-used workspace: the root is under the sandbox's data root
        // and its name is the formula of the canonical path.
        let w = world(false);
        let root = w.ctx.roots.memory_personal_project_dir(&w.ws).unwrap();
        assert_eq!(root.parent().unwrap(), w.ctx.roots.memory_projects_dir());
        assert!(root.starts_with(w.sb.data_home.join("muse/memory/projects")));
        assert_eq!(
            root.file_name().unwrap().to_str().unwrap(),
            personal_project_dir_name(&w.ws)
        );
        assert!(!root.exists());
        let (text, json) = path_report(&w.ctx, &w.ws).unwrap();
        assert!(text.starts_with(&root.display().to_string()));
        assert!(text.contains("exists: no"));
        assert!(text.contains("omm trust "));
        assert_eq!(json["exists"], false);
        assert!(json["trust"].is_null());
    }

    #[test]
    fn dir_names_parse_only_when_formula_shaped() {
        assert_eq!(
            parse_dir_name("a-0123456789abcdef"),
            Some(("a".into(), "0123456789abcdef".into()))
        );
        for bad in [
            "0123456789abcdef",
            "-0123456789abcdef",
            "a-0123456789ABCDEF",
            "a-0123456789abcde",
            "a b-0123456789abcdef",
            "a_b-0123456789abcdef",
            "plain",
            "",
        ] {
            assert_eq!(parse_dir_name(bad), None, "{bad:?}");
        }
        let long = format!("{}-{}", "x".repeat(97), "0".repeat(16));
        assert_eq!(parse_dir_name(&long), None, "slug past 96");
        let ok = format!("{}-{}", "x".repeat(96), "0".repeat(16));
        assert!(parse_dir_name(&ok).is_some());
    }

    #[test]
    fn seed_writes_once_ledgers_both_files_and_never_overwrites() {
        let w = world(false);
        pre_ledger(&w);
        let root = w.ctx.roots.memory_personal_project_dir(&w.ws).unwrap();
        let name = dir_name_of(&root).unwrap();

        // Dry run: nothing lands, the report says so.
        let dry = world(true);
        let r = seed(&dry.ctx, &dry.ws).unwrap();
        assert!(!dry.ctx.roots.memory_projects_dir().exists());
        assert!(r.render().contains("would be created"));
        assert_eq!(r.to_json()["dry_run"], true);

        let r = seed(&w.ctx, &w.ws).unwrap();
        assert_eq!(
            fs::read(root.join(MEMORY_FILE)).unwrap(),
            SEED_TEMPLATE.as_bytes()
        );
        let side: Sidecar =
            serde_json::from_slice(&fs::read(root.join(SIDECAR_FILE)).unwrap()).unwrap();
        assert_eq!(side.workspace, w.ws.to_string_lossy());
        assert_eq!(side.schema_version, SIDECAR_SCHEMA_VERSION);
        assert_eq!(side.workspace_for(&name).unwrap(), w.ws);
        let j = r.to_json();
        assert_eq!(j["root_created"], true);
        assert!(j["trust"].is_null(), "not trusted yet");
        assert_eq!(j["memory_md"]["outcome"], "written");
        assert_eq!(j["omm_workspace_json"]["outcome"], "written");
        assert_eq!(j["categories"][CATEGORY]["updated"], 2);
        assert!(r.render().contains("omm trust "), "{}", r.render());
        // Exactly the two files in the root.
        let mut names: Vec<String> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec![MEMORY_FILE, SIDECAR_FILE]);
        // Ledgered under muse-data, seeded / exclusive, with the seed's sha.
        let ledger = ledger_of(&w);
        let mem = ledger
            .find(
                Base::MuseData,
                &ledger_rel(&w.ctx, &name, MEMORY_FILE).unwrap(),
            )
            .expect("MEMORY.md entry");
        assert_eq!(mem.class, Class::Seeded);
        assert_eq!(mem.mechanism, Mechanism::Copy);
        assert_eq!(mem.kind, LEDGER_KIND);
        assert_eq!(mem.sha256, fsx::sha256_bytes(SEED_TEMPLATE.as_bytes()));
        assert_eq!(mem.writer, WRITER);
        assert_eq!(
            mem.path.as_str(),
            format!("memory/projects/{name}/{MEMORY_FILE}")
        );
        let side = ledger
            .find(
                Base::MuseData,
                &ledger_rel(&w.ctx, &name, SIDECAR_FILE).unwrap(),
            )
            .expect("sidecar entry");
        assert_eq!(side.class, Class::Exclusive);
        assert_eq!(ledger.entries.len(), 2);
        let lines = omm_ledger::audit::read(&w.ctx.omm_root()).unwrap();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.action == audit::ACTION_INSTALL));

        // Idempotent.
        let again = seed(&w.ctx, &w.ws).unwrap();
        assert!(again.converge.is_noop());
        assert_eq!(again.to_json()["memory_md"]["outcome"], "unchanged");
        assert_eq!(ledger_of(&w).entries.len(), 2);

        // A user edit is never overwritten; the ledger keeps the seed's sha.
        fs::write(root.join(MEMORY_FILE), b"# Project memory\nmy notes\n").unwrap();
        let edited = seed(&w.ctx, &w.ws).unwrap();
        assert_eq!(edited.to_json()["memory_md"]["outcome"], "skipped");
        assert_eq!(
            fs::read(root.join(MEMORY_FILE)).unwrap(),
            b"# Project memory\nmy notes\n"
        );
        assert_eq!(
            ledger_of(&w)
                .find(Base::MuseData, &mem.path)
                .unwrap()
                .sha256,
            mem.sha256
        );
        // Deleted: seeded again.
        fs::remove_file(root.join(MEMORY_FILE)).unwrap();
        let back = seed(&w.ctx, &w.ws).unwrap();
        assert_eq!(back.to_json()["memory_md"]["outcome"], "written");
        assert_eq!(
            fs::read(root.join(MEMORY_FILE)).unwrap(),
            SEED_TEMPLATE.as_bytes()
        );

        // Identical bytes already on disk, no ledger entry: adopted.
        let w2 = world(false);
        pre_ledger(&w2);
        let root2 = w2.ctx.roots.memory_personal_project_dir(&w2.ws).unwrap();
        fs::create_dir_all(&root2).unwrap();
        fs::write(root2.join(MEMORY_FILE), SEED_TEMPLATE).unwrap();
        let adopted = seed(&w2.ctx, &w2.ws).unwrap();
        assert_eq!(adopted.to_json()["memory_md"]["outcome"], "adopted");
        assert_eq!(adopted.to_json()["root_created"], false);
        assert_eq!(ledger_of(&w2).entries.len(), 2);
        // Trusted: no hint.
        let mut store = TrustStore::for_roots(&w2.ctx.roots).unwrap();
        store.merge_project(&w2.ws, TrustDecision::Trusted).unwrap();
        store
            .commit(&CommitOptions::for_roots(&w2.ctx.roots))
            .unwrap();
        let trusted = seed(&w2.ctx, &w2.ws).unwrap();
        assert_eq!(trusted.to_json()["trust"], "trusted");
        assert!(!trusted.render().contains("omm trust "));
    }

    #[test]
    fn uninstall_removes_the_seed_only_while_byte_identical() {
        use omm_ledger::uninstall::{plan, Options};
        use omm_ledger::Bases;
        let w = world(false);
        pre_ledger(&w);
        seed(&w.ctx, &w.ws).unwrap();
        let root = w.ctx.roots.memory_personal_project_dir(&w.ws).unwrap();
        let bases = Bases::from_roots(&w.ctx.roots, None);
        // The CLI's options: rules markers, both shared files.
        let opts = || Options {
            rules: Some(crate::cmd::lifecycle::rules::markers()),
            settings_file: Some(w.ctx.roots.settings_file()),
            trust_file: Some(w.ctx.roots.trust_file()),
            ..Options::default()
        };
        let p = plan(&ledger_of(&w), &bases, opts()).unwrap();
        let removed: Vec<PathBuf> = p.remove.iter().map(|s| s.abs.clone()).collect();
        assert!(removed.contains(&fs::canonicalize(root.join(MEMORY_FILE)).unwrap()));
        assert!(removed.contains(&fs::canonicalize(root.join(SIDECAR_FILE)).unwrap()));
        assert!(
            p.preserve.is_empty() && p.refused.is_empty(),
            "{}",
            p.render()
        );
        assert!(p.remove.iter().all(|s| !s.forced));
        // Edited: preserved and named; the sidecar still goes.
        fs::write(root.join(MEMORY_FILE), b"# Project memory\nmine\n").unwrap();
        let p = plan(&ledger_of(&w), &bases, opts()).unwrap();
        assert_eq!(p.remove.len(), 1, "{}", p.render());
        assert!(p.remove[0].abs.ends_with(SIDECAR_FILE));
        assert_eq!(p.preserve.len(), 1);
        assert!(p.preserve[0].abs.as_ref().unwrap().ends_with(MEMORY_FILE));
        // The generic path (no markers) decides the same.
        let p2 = plan(&ledger_of(&w), &bases, Options::default()).unwrap();
        assert_eq!(p2.remove.len(), 1);
        assert_eq!(p2.preserve.len(), 1);
        // `--force` removes it anyway, marked as forced.
        let p3 = plan(
            &ledger_of(&w),
            &bases,
            Options {
                force: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(p3.remove.len(), 2);
        assert!(p3.remove.iter().any(|s| s.forced));
    }

    #[test]
    fn seed_refuses_a_symlinked_root_and_keeps_a_foreign_sidecar() {
        let w = world(false);
        pre_ledger(&w);
        let root = w.ctx.roots.memory_personal_project_dir(&w.ws).unwrap();
        #[cfg(unix)]
        {
            let elsewhere = w.sb.root.join("elsewhere");
            fs::create_dir_all(&elsewhere).unwrap();
            fs::create_dir_all(root.parent().unwrap()).unwrap();
            std::os::unix::fs::symlink(&elsewhere, &root).unwrap();
            let err = seed(&w.ctx, &w.ws).unwrap_err();
            assert!(err.to_string().contains("symlink"), "{err}");
            assert!(!elsewhere.join(MEMORY_FILE).exists());
            assert!(ledger_of(&w).entries.is_empty());
            fs::remove_file(&root).unwrap();
        }
        // A sidecar that is not ours (unledgered, different bytes) stays.
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(SIDECAR_FILE),
            b"{\"schema_version\":1,\"workspace\":\"/other\",\"writer\":\"x\"}\n",
        )
        .unwrap();
        let r = seed(&w.ctx, &w.ws).unwrap();
        assert_eq!(r.to_json()["omm_workspace_json"]["outcome"], "skipped");
        assert_eq!(r.to_json()["memory_md"]["outcome"], "written");
        assert!(fs::read_to_string(root.join(SIDECAR_FILE))
            .unwrap()
            .contains("/other"));
        assert_eq!(ledger_of(&w).entries.len(), 1, "only MEMORY.md ledgered");
    }

    #[test]
    fn list_maps_roots_through_sidecar_trust_and_cwd_and_flags_collisions() {
        let w = world(false);
        pre_ledger(&w);
        seed(&w.ctx, &w.ws).unwrap();
        let seeded = personal_project_dir_name(&w.ws);
        // A trusted workspace with a bare root (no sidecar).
        let trusted_ws = w.sb.root.join("trusted-ws");
        fs::create_dir_all(&trusted_ws).unwrap();
        let mut store = TrustStore::for_roots(&w.ctx.roots).unwrap();
        store
            .merge_project(&trusted_ws, TrustDecision::Trusted)
            .unwrap();
        store
            .commit(&CommitOptions::for_roots(&w.ctx.roots))
            .unwrap();
        bare_root(&w, &trusted_ws);
        // The cwd's bare root.
        let cwd_ws = w.sb.root.join("cwd-ws");
        fs::create_dir_all(&cwd_ws).unwrap();
        bare_root(&w, &cwd_ws);
        // Two workspaces with one slug: the hash tells them apart.
        let a = w.sb.root.join("col.lide");
        let b = w.sb.root.join("col-lide");
        bare_root(&w, &a);
        bare_root(&w, &b);
        assert_eq!(
            parse_dir_name(&personal_project_dir_name(&a)).unwrap().0,
            parse_dir_name(&personal_project_dir_name(&b)).unwrap().0
        );
        // An unrecognised name and a bad sidecar.
        let odd = w.ctx.roots.memory_projects_dir().join("not-a-formula-name");
        fs::create_dir_all(&odd).unwrap();
        let bad = bare_root(&w, &w.sb.root.join("bad-sidecar"));
        fs::write(bad.join(SIDECAR_FILE), b"not json").unwrap();
        // A sidecar that disagrees with its directory.
        let liar = bare_root(&w, &w.sb.root.join("liar"));
        fs::write(
            liar.join(SIDECAR_FILE),
            Sidecar::new(&w.sb.root.join("someone-else"))
                .to_bytes()
                .unwrap(),
        )
        .unwrap();

        let roots = scan(&w.ctx, Some(&cwd_ws)).unwrap();
        let by_name: BTreeMap<&str, &RootInfo> =
            roots.iter().map(|r| (r.name.as_str(), r)).collect();
        let s = by_name[seeded.as_str()];
        assert_eq!(
            (s.via, s.workspace.as_deref()),
            (Via::Sidecar, Some(w.ws.as_path()))
        );
        assert_eq!(s.workspace_exists(), Some(true));
        assert_eq!(s.usage.md_files, 1);
        assert_eq!(s.usage.md_bytes, SEED_TEMPLATE.len() as u64);
        assert_eq!(s.usage.other_files, 1, "the sidecar");
        let t = by_name[personal_project_dir_name(&trusted_ws).as_str()];
        assert_eq!(
            (t.via, t.workspace.as_deref()),
            (Via::Trust, Some(trusted_ws.as_path()))
        );
        let c = by_name[personal_project_dir_name(&cwd_ws).as_str()];
        assert_eq!(
            (c.via, c.workspace.as_deref()),
            (Via::Cwd, Some(cwd_ws.as_path()))
        );
        let a_info = by_name[personal_project_dir_name(&a).as_str()];
        assert_eq!(
            (a_info.via, a_info.workspace.as_deref()),
            (Via::Unknown, None)
        );
        assert_eq!(a_info.workspace_exists(), None);
        assert!(by_name["not-a-formula-name"]
            .note
            .as_deref()
            .unwrap()
            .contains("slug96"));
        assert!(
            by_name[personal_project_dir_name(&w.sb.root.join("bad-sidecar")).as_str()]
                .note
                .as_deref()
                .unwrap()
                .starts_with(SIDECAR_FILE)
        );
        let l = by_name[personal_project_dir_name(&w.sb.root.join("liar")).as_str()];
        assert_eq!(l.via, Via::Unknown);
        assert!(l.note.as_deref().unwrap().contains("not this directory"));
        let col = collisions(&roots);
        assert_eq!(col.len(), 1);
        assert_eq!(
            col[0].0,
            parse_dir_name(&personal_project_dir_name(&a)).unwrap().0
        );
        assert_eq!(col[0].1.len(), 2);

        let table = list(&w.ctx, Some(&cwd_ws)).unwrap();
        assert_eq!(table.len(), roots.len());
        let json = table.to_json();
        let row = json
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["root"] == seeded)
            .unwrap();
        assert_eq!(row["via"], "sidecar");
        assert_eq!(row["md_files"], "1/48");
        assert_eq!(row["bytes"], format!("{}/16,305", SEED_TEMPLATE.len()));
        assert_eq!(row["workspace_exists"], "yes");
        let slug_only = json
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["root"] == personal_project_dir_name(&a))
            .unwrap();
        assert_eq!(slug_only["workspace"], "slug only");
        assert_eq!(slug_only["workspace_exists"], "-");
        // No projects dir at all: an empty table, no error.
        let empty = world(false);
        assert!(list(&empty.ctx, None).unwrap().is_empty());
    }

    #[test]
    fn usage_counts_markdown_against_the_caps_and_ignores_the_host_lock() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("r");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join(MEMORY_FILE), b"12345").unwrap();
        fs::write(root.join("sub/n.MD"), b"123").unwrap();
        fs::write(root.join(HOST_LOCK_FILE), b"").unwrap();
        fs::write(root.join(SIDECAR_FILE), b"{}").unwrap();
        let u = usage(&root);
        assert_eq!((u.md_files, u.md_bytes, u.other_files), (2, 8, 1));
        assert!(!u.over_cap());
        assert_eq!(u.files_cell(), "2/48");
        assert_eq!(u.bytes_cell(), "8/16,305");
        fs::write(
            root.join("big.md"),
            vec![b'x'; hr::MEMORY_SNAPSHOT_BYTES as usize],
        )
        .unwrap();
        assert!(usage(&root).over_cap());
        assert_eq!(usage(&tmp.path().join("missing")), Usage::default());
    }

    #[test]
    fn backup_tars_the_root_rolls_to_five_and_honours_to() {
        let w = world(false);
        pre_ledger(&w);
        assert!(
            matches!(backup(&w.ctx, &w.ws, None), Err(OmmError::Usage(_))),
            "no root yet"
        );
        seed(&w.ctx, &w.ws).unwrap();
        let root = w.ctx.roots.memory_personal_project_dir(&w.ws).unwrap();
        let name = dir_name_of(&root).unwrap();
        fs::write(root.join("extra.md"), b"more\n").unwrap();

        // Dry run: the plan names the target shape, nothing lands.
        let mut dry_ctx = Ctx::new(w.ctx.roots.clone(), flags(true));
        dry_ctx.out = Output::silent();
        let dry = backup(&dry_ctx, &w.ws, None).unwrap();
        assert!(dry.converge.is_dry_run());
        assert!(dry.render().contains("would archive"), "{}", dry.render());
        assert!(dry.to_json()["backup"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/<ts>/{name}.tar")));
        assert!(!backups_dir(&w.ctx).exists());

        let r = backup(&w.ctx, &w.ws, None).unwrap();
        let file = PathBuf::from(r.to_json()["backup"].as_str().unwrap());
        assert!(file.starts_with(backups_dir(&w.ctx)), "{}", file.display());
        assert_eq!(
            file.file_name().unwrap().to_str().unwrap(),
            format!("{name}.tar")
        );
        assert!(fsx::is_snapshot_dir_name(
            file.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
        ));
        let members = memory_tar::tests::read(&fs::read(&file).unwrap());
        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                format!("{name}/{MEMORY_FILE}"),
                format!("{name}/extra.md"),
                format!("{name}/{SIDECAR_FILE}"),
            ]
        );
        assert_eq!(members[0].data, SEED_TEMPLATE.as_bytes());
        assert_eq!(r.to_json()["files"], 3);
        assert_eq!(r.to_json()["categories"][CATEGORY_BACKUPS]["updated"], 1);
        let lines = omm_ledger::audit::read(&w.ctx.omm_root()).unwrap();
        assert_eq!(lines.last().unwrap().action, audit::ACTION_SNAPSHOT);
        assert!(lines
            .last()
            .unwrap()
            .note
            .as_deref()
            .unwrap()
            .contains("omm memory backup"));
        // The ledger's own snapshot roll never touches `memory/`.
        assert!(fsx::prune_snapshots(&w.ctx.roots.snapshots_dir(), 0)
            .unwrap()
            .is_empty());
        assert!(file.exists());

        for _ in 0..6 {
            backup(&w.ctx, &w.ws, None).unwrap();
        }
        let dirs = fs::read_dir(backups_dir(&w.ctx)).unwrap().count();
        assert_eq!(dirs, fsx::SNAPSHOTS_KEEP);
        assert!(!file.exists(), "the first backup rolled off");

        // `--to`: a plain tar in the given dir, not rolled, refused inside the root.
        let to = w.sb.root.join("out");
        fs::create_dir_all(&to).unwrap();
        let r = backup(&w.ctx, &w.ws, Some(&to)).unwrap();
        let file = PathBuf::from(r.to_json()["backup"].as_str().unwrap());
        assert_eq!(file.parent().unwrap(), fs::canonicalize(&to).unwrap());
        assert!(file
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with(&format!("{name}-")));
        let second = backup(&w.ctx, &w.ws, Some(&to)).unwrap();
        assert_ne!(second.to_json()["backup"], r.to_json()["backup"]);
        assert!(matches!(
            backup(&w.ctx, &w.ws, Some(&root)),
            Err(OmmError::Usage(_))
        ));
        assert!(matches!(
            backup(&w.ctx, &w.ws, Some(&w.sb.root.join("missing"))),
            Err(OmmError::Usage(_))
        ));
        assert_eq!(
            fs::read_dir(backups_dir(&w.ctx)).unwrap().count(),
            fsx::SNAPSHOTS_KEEP
        );
    }

    #[test]
    fn gc_removes_only_sidecar_mapped_roots_whose_workspace_is_gone() {
        let w = world(false);
        pre_ledger(&w);
        // 1. Seeded, workspace removed afterwards → removable.
        let gone_ws = w.sb.root.join("gone");
        fs::create_dir_all(&gone_ws).unwrap();
        seed(&w.ctx, &gone_ws).unwrap();
        let gone_root = w.ctx.roots.memory_personal_project_dir(&gone_ws).unwrap();
        fs::write(gone_root.join("notes.md"), b"keep a copy\n").unwrap();
        let gone_name = dir_name_of(&gone_root).unwrap();
        fs::remove_dir_all(&gone_ws).unwrap();
        // 2. Seeded, workspace present → kept.
        seed(&w.ctx, &w.ws).unwrap();
        let live_root = w.ctx.roots.memory_personal_project_dir(&w.ws).unwrap();
        // 3. No sidecar, workspace gone → NEVER removed.
        let bare_ws = w.sb.root.join("bare");
        fs::create_dir_all(&bare_ws).unwrap();
        let bare = bare_root(&w, &bare_ws);
        fs::remove_dir_all(&bare_ws).unwrap();
        // 4. Trusted mapping, workspace gone, no sidecar → kept.
        let trusted_ws = w.sb.root.join("trusted");
        fs::create_dir_all(&trusted_ws).unwrap();
        let mut store = TrustStore::for_roots(&w.ctx.roots).unwrap();
        store
            .merge_project(&trusted_ws, TrustDecision::Trusted)
            .unwrap();
        store
            .commit(&CommitOptions::for_roots(&w.ctx.roots))
            .unwrap();
        let trusted_root = bare_root(&w, &trusted_ws);
        fs::remove_dir_all(&trusted_ws).unwrap();
        // 5. A sidecar that disagrees with its directory, workspace gone → kept.
        let liar = bare_root(&w, &w.sb.root.join("liar"));
        fs::write(
            liar.join(SIDECAR_FILE),
            Sidecar::new(&w.sb.root.join("nowhere")).to_bytes().unwrap(),
        )
        .unwrap();
        assert_eq!(ledger_of(&w).entries.len(), 4);

        // Dry run: the plan names the one root, nothing moves.
        let mut dry_ctx = Ctx::new(w.ctx.roots.clone(), flags(true));
        dry_ctx.out = Output::silent();
        let plan = gc(&dry_ctx).unwrap();
        assert!(
            plan.render()
                .contains(&format!("would remove {}", gone_root.display())),
            "{}",
            plan.render()
        );
        assert_eq!(plan.to_json()["removed"].as_array().unwrap().len(), 1);
        assert_eq!(plan.to_json()["kept"].as_array().unwrap().len(), 4);
        assert!(gone_root.exists());
        assert!(!backups_dir(&w.ctx).exists());

        let r = gc(&w.ctx).unwrap();
        assert!(!gone_root.exists());
        assert!(live_root.is_dir() && bare.is_dir() && trusted_root.is_dir() && liar.is_dir());
        let j = r.to_json();
        assert_eq!(j["removed"].as_array().unwrap().len(), 1);
        assert_eq!(j["removed"][0]["root"], gone_name);
        assert_eq!(j["removed"][0]["ledger_entries_dropped"], 2);
        assert_eq!(j["kept"].as_array().unwrap().len(), 4);
        assert_eq!(j["categories"][CATEGORY]["removed"], 1);
        assert_eq!(j["categories"][CATEGORY]["backed_up"], 1);
        assert_eq!(j["categories"][CATEGORY]["skipped"], 4);
        let kept: Vec<String> = j["kept"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k["why"].as_str().unwrap().to_string())
            .collect();
        assert!(kept.iter().any(|k| k.contains("never guessed")), "{kept:?}");
        assert!(kept.iter().any(|k| k.contains("trust.json")), "{kept:?}");
        assert!(kept.iter().any(|k| k.contains("exists")), "{kept:?}");
        assert!(
            kept.iter().any(|k| k.contains("not this directory")),
            "{kept:?}"
        );
        // The backup holds every file the root had.
        let backup = PathBuf::from(j["removed"][0]["backup"].as_str().unwrap());
        assert!(backup.starts_with(backups_dir(&w.ctx)));
        let members = memory_tar::tests::read(&fs::read(&backup).unwrap());
        let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
        assert!(
            names.contains(&format!("{gone_name}/notes.md").as_str()),
            "{names:?}"
        );
        assert!(names.contains(&format!("{gone_name}/{MEMORY_FILE}").as_str()));
        // The ledger forgot the root's two entries and keeps the live root's.
        let ledger = ledger_of(&w);
        assert_eq!(ledger.entries.len(), 2);
        assert!(ledger
            .entries
            .iter()
            .all(|e| !e.path.as_str().contains(&gone_name)));
        let lines = omm_ledger::audit::read(&w.ctx.omm_root()).unwrap();
        let last = lines.last().unwrap();
        assert_eq!(last.action, audit::ACTION_UNINSTALL);
        assert!(last.note.as_deref().unwrap().contains("omm memory gc"));
        // A second run has nothing to remove.
        let again = gc(&w.ctx).unwrap();
        assert_eq!(again.to_json()["removed"].as_array().unwrap().len(), 0);
        assert_eq!(fs::read_dir(backups_dir(&w.ctx)).unwrap().count(), 1);
    }

    #[test]
    fn seeded_memory_composes_the_order_max_block_in_a_trusted_workspace() {
        let Some(bin) = host_bin() else {
            eprintln!("skipped: OMM_MUSE_BIN is unset");
            return;
        };
        let w = world(false);
        let inv = Invoker::new(&bin).sandboxed(&w.sb);
        let mut ctx = Ctx::new(w.ctx.roots.clone(), flags(false)).with_invoker(inv.clone());
        ctx.out = Output::silent();
        // `omm trust` writes exactly this.
        let mut store = TrustStore::for_roots(&ctx.roots).unwrap();
        store.merge_project(&w.ws, TrustDecision::Trusted).unwrap();
        store.commit(&CommitOptions::for_roots(&ctx.roots)).unwrap();
        // No ledger yet: seed creates one through the host (`--version`).
        let r = seed(&ctx, &w.ws).unwrap();
        assert_eq!(r.to_json()["trust"], "trusted");
        assert_eq!(ledger_of(&w).entries.len(), 2);

        let out = inv
            .clone()
            .cwd(&w.ws)
            .run(&["exec", "--provider", "echo", "hi"])
            .unwrap()
            .expect_ok()
            .unwrap();
        assert!(out.stderr.contains("trusted"), "{}", out.stderr);
        let log =
            omm_host::probe::newest_session_log(&w.sb.data_home.join("muse").join("sessions"))
                .unwrap();
        let facts = omm_host::probe::parse_session_log(&log).unwrap();
        let block = facts
            .block(u64::from(hr::CONTEXT_ORDER_MEMORY_SNAPSHOT))
            .expect("the memory_snapshot block composed");
        assert!(
            block.text.contains("## Memory scope: personal_project"),
            "{}",
            block.text
        );
        assert!(
            block.text.contains(SEED_TEMPLATE.trim_end()),
            "{}",
            block.text
        );
        assert!(
            !block.text.contains(SIDECAR_FILE),
            "the sidecar is not Markdown"
        );
        assert!(block.bytes as u64 <= hr::MEMORY_SNAPSHOT_BYTES);

        // Control, measured 2026-09-03 on 1.0.1-R2006.1: the same seed in a
        // workspace with NO trust store composes the same section — the
        // read side of the snapshot is not trust-gated on this build,
        // whatever host-reality.md "Paths" says (reported; the row stands
        // until the doc is corrected). Locked here so a host that starts
        // gating it is noticed.
        let u = world(false);
        let uinv = Invoker::new(&bin).sandboxed(&u.sb);
        let mut uctx = Ctx::new(u.ctx.roots.clone(), flags(false)).with_invoker(uinv.clone());
        uctx.out = Output::silent();
        let r = seed(&uctx, &u.ws).unwrap();
        assert!(r.to_json()["trust"].is_null());
        uinv.cwd(&u.ws)
            .run(&["exec", "--provider", "echo", "hi"])
            .unwrap()
            .expect_ok()
            .unwrap();
        let log =
            omm_host::probe::newest_session_log(&u.sb.data_home.join("muse").join("sessions"))
                .unwrap();
        let facts = omm_host::probe::parse_session_log(&log).unwrap();
        let block = facts
            .block(u64::from(hr::CONTEXT_ORDER_MEMORY_SNAPSHOT))
            .unwrap_or_else(|| panic!("untrusted: no memory block among {:?}", facts.orders()));
        assert!(
            block.text.contains("## Memory scope: personal_project")
                && block.text.contains(SEED_TEMPLATE.trim_end()),
            "untrusted: {}",
            block.text
        );
    }
}
