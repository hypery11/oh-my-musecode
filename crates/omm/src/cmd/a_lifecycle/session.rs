//! One command's view of the ledger (ARCHITECTURE.md §4): the four bases,
//! the loaded (or absent) `omm.lock.json`, the audit writer, and — for the
//! commands that write — the exclusive `$OMM/locks/ledger.lock` held from
//! load to the last save so two omm processes never interleave. Under
//! `--dry-run` nothing under `$OMM/` is created: no lock, no audit line, no
//! save.

use std::path::{Path, PathBuf};

use omm_host::fsx::{self, FileLock};
use omm_host::{probe, Invoker};

use omm_ledger::audit::{Audit, Event};
use omm_ledger::containment::{Bases, Resolved, State};
use omm_ledger::store::{self, Loaded, Quarantined};
use omm_ledger::{Base, HostInfo, Ledger, RelPath, Scope};

use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::Result;

/// The ledger session of one write command.
#[derive(Debug)]
pub struct Session {
    pub omm_root: PathBuf,
    pub bases: Bases,
    pub ledger: Option<Ledger>,
    pub quarantined: Option<Quarantined>,
    pub dry_run: bool,
    audit: Audit,
    _lock: Option<FileLock>,
}

impl Session {
    /// Lock (unless `--dry-run`) and load. A corrupt ledger is quarantined
    /// by the store on load — the one write a dry run may still do, and a
    /// loud one ([`Session::warn_quarantine`]).
    pub fn open(ctx: &Ctx, workspace: Option<&Path>) -> Result<Session> {
        let omm_root = ctx.omm_root();
        let lock = if ctx.dry_run {
            None
        } else {
            Some(store::lock(&omm_root, fsx::LOCK_WAIT_DEFAULT)?)
        };
        let loaded = store::load(&omm_root)?;
        let quarantined = loaded.quarantined().cloned();
        let ledger = match loaded {
            Loaded::Present(l) => Some(l),
            Loaded::Absent { .. } => None,
        };
        Ok(Session {
            bases: Bases::from_roots(&ctx.roots, workspace),
            audit: Audit::new(&omm_root, OMM_VERSION),
            omm_root,
            ledger,
            quarantined,
            dry_run: ctx.dry_run,
            _lock: lock,
        })
    }

    /// Print the quarantine warning, if a corrupt ledger was moved aside.
    pub fn warn_quarantine(&self, ctx: &Ctx) {
        if let Some(q) = &self.quarantined {
            ctx.out.warn(q.message());
        }
    }

    /// The ledger, created empty for this omm and host when absent.
    pub fn ledger_or_new(&mut self, inv: &Invoker) -> Result<&mut Ledger> {
        if self.ledger.is_none() {
            let host = host_info(inv)?;
            self.ledger = Some(Ledger::new(OMM_VERSION, host, Scope::User));
        }
        Ok(self
            .ledger
            .as_mut()
            .unwrap_or_else(|| unreachable!("ledger was just created")))
    }

    /// Persist the ledger (no-op under `--dry-run` or with no ledger).
    pub fn save(&self) -> Result<()> {
        if self.dry_run {
            return Ok(());
        }
        if let Some(l) = &self.ledger {
            store::save(&self.omm_root, l)?;
        }
        Ok(())
    }

    /// Append an audit line (no-op under `--dry-run`).
    pub fn record(&self, ev: &Event) -> Result<()> {
        if !self.dry_run {
            self.audit.append(ev)?;
        }
        Ok(())
    }

    /// `$OMM/snapshots/`.
    pub fn snapshots_dir(&self) -> PathBuf {
        self.omm_root.join(omm_host::paths::OMM_SNAPSHOTS_DIR)
    }

    /// `Bases::resolve` for a first install: a base root that does not
    /// exist yet (a fresh config root before the host created it) resolves
    /// to `Missing` at the lexical path instead of a containment error.
    /// Containment is still enforced by `RelPath`'s grammar; the root is
    /// created by the first write.
    pub fn probe(&self, base: Base, rel: &RelPath) -> Result<Resolved> {
        let root = self.bases.root(base)?;
        if !root.exists() {
            return Ok(Resolved {
                base,
                base_root: root.to_path_buf(),
                path: rel.under(root),
                state: State::Missing,
                via_symlink: None,
            });
        }
        Ok(self.bases.resolve(base, rel)?)
    }
}

/// The host as the ledger records it: `muse --version`'s build string
/// (informational, never gated on — R15) and the binary's SHA-256.
pub fn host_info(inv: &Invoker) -> Result<HostInfo> {
    let version = probe::version(inv)?;
    let sha256 = fsx::sha256_file(inv.bin())?;
    Ok(HostInfo {
        version: version.build,
        sha256,
    })
}

/// True when `path` is the root of a git checkout (`.git` directory or the
/// file a worktree carries) — the workspace shape `omm install` trusts.
pub fn is_git_workspace(path: &Path) -> bool {
    let git = path.join(".git");
    std::fs::symlink_metadata(&git)
        .map(|m| m.is_dir() || m.is_file())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Flags;

    #[test]
    fn dry_run_session_creates_nothing_under_omm_root() {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let ctx = Ctx::new(
            sb.roots().unwrap(),
            Flags {
                dry_run: true,
                yes: true,
                ..Flags::default()
            },
        );
        let s = Session::open(&ctx, None).unwrap();
        assert!(s.ledger.is_none() && s.quarantined.is_none() && s.dry_run);
        s.save().unwrap();
        s.record(&Event::new("install")).unwrap();
        assert!(!ctx.omm_root().exists(), "a dry run leaves $OMM untouched");
        // A real session takes the lock and creates the locks dir only.
        let ctx = Ctx::new(
            sb.roots().unwrap(),
            Flags {
                yes: true,
                ..Flags::default()
            },
        );
        let s = Session::open(&ctx, Some(tmp.path())).unwrap();
        assert!(ctx.omm_root().join("locks").join("ledger.lock").exists());
        assert_eq!(s.bases.workspace.as_deref(), Some(tmp.path()));
        assert!(s
            .bases
            .residue
            .iter()
            .any(|p| p.ends_with("session-name-authority")));
    }

    #[test]
    fn git_workspace_detection() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!is_git_workspace(tmp.path()));
        std::fs::create_dir(tmp.path().join(".git")).unwrap();
        assert!(is_git_workspace(tmp.path()));
        let wt = tmp.path().join("wt");
        std::fs::create_dir(&wt).unwrap();
        std::fs::write(wt.join(".git"), b"gitdir: ../.git/worktrees/wt\n").unwrap();
        assert!(is_git_workspace(&wt));
    }
}
