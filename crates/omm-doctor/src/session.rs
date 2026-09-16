//! One live echo session — the only oracle for what actually composes.
//!
//! `muse exec --provider echo --trust-workspace hi` in a fresh, empty,
//! probe-owned workspace (`probe::EchoSessionOptions::fresh_workspace`) with
//! the data root redirected to a throwaway directory, so the session log never
//! lands in the user's store (ARCHITECTURE.md §7: `omm cost` "runs one echo
//! session in a temp XDG_DATA_HOME"). Because installed plugins live under
//! the data root (`plugins/installed.json` + `plugins/cache/`, host-reality
//! "Paths"; `cache_path` is data-root-relative — measured 2026-09-02), a bare
//! temp root would compose no plugin at all; the throwaway root is therefore
//! seeded with a copy of the plugin store (and of `memory/`, whose snapshot
//! block is data-root-scoped) before the run. The config root is read as-is
//! — the user's skills, rules, trust and `runtime_capabilities` approvals
//! compose exactly as in the user's own sessions — unless a caller supplies a
//! throwaway copy of it (`omm cost` measures its settings variants that way).
//!
//! Never `--provider meta` with `hi`: that prompt is answered from an embedded
//! replay fixture and records no `model_request_configured` event
//! (`docs/experiments/context-slimming.md` §7.1).

use std::path::{Path, PathBuf};

use serde::Serialize;

use omm_host::host_reality as hr;
use omm_host::probe::{self, EchoSessionOptions, SessionFacts};
use omm_host::{fsx, HostError, Invoker, Roots};

use crate::catalog::Catalog;
use crate::error::{DoctorError, Result};

/// The most a seeded subtree may copy into the throwaway data root; past it
/// the copy stops and the measurement says so (`Seeded::truncated`).
pub const SEED_CAP_BYTES: u64 = 64 * 1024 * 1024;

/// What to copy into the throwaway data root before the run.
#[derive(Clone, Copy, Debug)]
pub struct SeedOptions {
    /// `plugins/installed.json` and `plugins/cache/` (the runtime reads the
    /// cache, never the source — `docs/experiments/marketplace-precedence.md` §5.1).
    pub plugins: bool,
    /// `memory/` (the order-`u32::MAX` snapshot block, host-reality "Budgets").
    pub memory: bool,
}

impl Default for SeedOptions {
    fn default() -> Self {
        SeedOptions {
            plugins: true,
            memory: true,
        }
    }
}

/// What one seeded subtree amounted to.
#[derive(Clone, Debug, Serialize)]
pub struct Seeded {
    pub what: &'static str,
    pub files: usize,
    pub bytes: u64,
    /// The copy stopped at [`SEED_CAP_BYTES`].
    pub truncated: bool,
}

/// Options for [`run`].
#[derive(Clone, Debug, Default)]
pub struct LiveOptions {
    /// Read the config root from here instead of the invoker's (a throwaway
    /// copy, for settings variants).
    pub config_home: Option<PathBuf>,
    pub seed: SeedOptions,
}

/// One measured session.
#[derive(Clone, Debug)]
pub struct LiveSession {
    /// The fresh workspace the session ran in (already removed).
    pub workspace: PathBuf,
    /// Always `--trust-workspace` (host-reality's P0/P1 rows assume trusted).
    pub trusted: bool,
    pub facts: SessionFacts,
    /// The parsed order-200 block, when the session composed one.
    pub catalog: Option<Catalog>,
    /// The host's stderr (the stage-3 catalog drop is announced only there).
    pub stderr: String,
    /// stderr named the 32,000-byte cap — stage-3 degradation (entries dropped).
    pub cap_named_on_stderr: bool,
    pub seeded: Vec<Seeded>,
}

impl LiveSession {
    /// Bytes of the block at `order`, if it composed.
    pub fn block_bytes(&self, order: u32) -> Option<usize> {
        self.facts.block(u64::from(order)).map(|b| b.bytes)
    }
    /// Bytes of every run-context block together.
    pub fn run_context_bytes(&self) -> usize {
        self.facts.context_blocks.iter().map(|b| b.bytes).sum()
    }
    /// The catalog block's bytes (0 when none composed).
    pub fn catalog_bytes(&self) -> usize {
        self.catalog.as_ref().map(|c| c.total_bytes).unwrap_or(0)
    }
}

fn tempdir(prefix: &str) -> Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .map_err(|e| DoctorError::io("create temp dir", std::env::temp_dir(), e))
}

/// Run one live session for `roots` through `inv` (which must already point
/// at the same roots: `Invoker::from_env()` or a sandboxed invoker).
pub fn run(inv: &Invoker, roots: &Roots, opts: &LiveOptions) -> Result<LiveSession> {
    let tmp = tempdir("omm-live-")?;
    let data = tmp.path().join("data");
    fsx::create_dir_all(&data)?;
    let mut seeded = Vec::new();
    if opts.seed.plugins {
        seeded.push(seed_plugins(roots, &data)?);
    }
    if opts.seed.memory {
        seeded.push(copy_tree(
            &roots.muse_data().join("memory"),
            &data.join("muse").join("memory"),
            "memory",
        )?);
    }
    let ws = EchoSessionOptions::fresh_workspace()?;
    let mut probe = inv.clone().data_home(&data).cwd(&ws.workspace);
    if let Some(cfg) = &opts.config_home {
        probe = probe.config_home(cfg);
    }
    let argv = [
        "exec",
        "--provider",
        "echo",
        "--trust-workspace",
        ws.prompt.as_str(),
    ];
    let out = probe.run(&argv)?;
    if !out.ok() {
        return Err(DoctorError::Host(HostError::Command {
            argv: out.argv_string(),
            code: out.code,
            stderr: out.stderr.lines().next().unwrap_or("").trim().to_string(),
        }));
    }
    let log = probe::newest_session_log(&data.join("muse").join("sessions"))?;
    let facts = probe::parse_session_log(&log)?;
    let catalog = facts
        .block(u64::from(hr::CONTEXT_ORDER_SKILLS_CATALOG))
        .map(|b| Catalog::parse(&b.text));
    let cap_named_on_stderr = out
        .stderr
        .contains(&hr::SKILLS_CATALOG_CAP_BYTES.to_string());
    Ok(LiveSession {
        workspace: ws.workspace.clone(),
        trusted: ws.trust_workspace,
        facts,
        catalog,
        stderr: out.stderr,
        cap_named_on_stderr,
        seeded,
    })
}

/// Copy `plugins/installed.json` and `plugins/cache/` of the host's data
/// root into the throwaway one. The `cache_path` recorded in `installed.json`
/// is data-root-relative (measured 2026-09-02: `plugins/cache/local/<id>/<sha>/package`),
/// so the copy resolves under the new root; `source.path` stays absolute and
/// is not read at session time.
fn seed_plugins(roots: &Roots, data: &Path) -> Result<Seeded> {
    let src = roots.plugins_dir();
    let dest = data.join("muse").join("plugins");
    let mut total = Seeded {
        what: "plugins",
        files: 0,
        bytes: 0,
        truncated: false,
    };
    let installed = src.join("installed.json");
    if installed.is_file() {
        fsx::create_dir_all(&dest)?;
        let bytes = fsx::read_bytes(&installed)?;
        fsx::write_atomic(&dest.join("installed.json"), &bytes)?;
        total.files += 1;
        total.bytes += bytes.len() as u64;
    }
    let cache = copy_tree(&src.join("cache"), &dest.join("cache"), "plugins")?;
    total.files += cache.files;
    total.bytes += cache.bytes;
    total.truncated = cache.truncated;
    Ok(total)
}

/// Copy every regular file under `src` to the same relative path under
/// `dest` (atomic writes, symlinks skipped, never followed), up to
/// [`SEED_CAP_BYTES`]. A missing `src` is an empty copy.
pub fn copy_tree(src: &Path, dest: &Path, what: &'static str) -> Result<Seeded> {
    let mut seeded = Seeded {
        what,
        files: 0,
        bytes: 0,
        truncated: false,
    };
    if !src.is_dir() {
        return Ok(seeded);
    }
    let src = fsx::canonicalize(src)?;
    for entry in walkdir::WalkDir::new(&src)
        .follow_links(false)
        .into_iter()
        .flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = match entry.path().strip_prefix(&src) {
            Ok(r) => r.to_path_buf(),
            Err(_) => continue,
        };
        let bytes = fsx::read_bytes(entry.path())?;
        if seeded.bytes + bytes.len() as u64 > SEED_CAP_BYTES {
            seeded.truncated = true;
            break;
        }
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            fsx::create_dir_all(parent)?;
        }
        fsx::write_atomic(&target, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = entry.metadata() {
                let _ = std::fs::set_permissions(
                    &target,
                    std::fs::Permissions::from_mode(meta.permissions().mode()),
                );
            }
        }
        seeded.files += 1;
        seeded.bytes += bytes.len() as u64;
    }
    Ok(seeded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_tree_copies_regular_files_only_and_reports_totals() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        std::fs::create_dir_all(src.join("a/b")).unwrap();
        std::fs::write(src.join("a/b/f.txt"), b"hello").unwrap();
        std::fs::write(src.join("g.json"), b"{}").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(src.join("g.json"), src.join("link.json")).unwrap();
        let dest = dir.path().join("dest");
        let s = copy_tree(&src, &dest, "t").unwrap();
        assert_eq!(s.files, 2);
        assert_eq!(s.bytes, 7);
        assert!(!s.truncated);
        assert_eq!(std::fs::read(dest.join("a/b/f.txt")).unwrap(), b"hello");
        assert!(!dest.join("link.json").exists(), "symlinks are not copied");
        let missing = copy_tree(&dir.path().join("nope"), &dest, "t").unwrap();
        assert_eq!(missing.files, 0);
    }
}
