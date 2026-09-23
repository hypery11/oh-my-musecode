//! Base-relative paths and the containment rule (R4).
//!
//! Exactly four allowlisted bases ([`Base`]): the host's config root, the
//! host's data root, omm's root and the workspace. Every path is resolved with
//! `canonicalize()` + `strip_prefix()` against its base before any filesystem
//! operation; `/`, `$HOME` and any base root itself are refused; a `..`,
//! absolute or backslash path never gets past [`RelPath`]. A symlink at the
//! leaf is reported ([`State::Symlink`]) and never followed: omm never wrote
//! one (R12), and writing *through* one would land the bytes wherever the
//! link points. An intermediate symlink is followed by `canonicalize` and the
//! result must still be inside the base — and it is REPORTED
//! ([`Resolved::via_symlink`]): omm never wrote a symlink (R12), so an
//! ancestor that is one now, even inside the base, is not what omm wrote
//! (the R2 sentinel — Gate 1 decision C: a managed skill directory replaced
//! by a symlink to the user's own directory holding byte-identical files
//! had its files removed at uninstall).
//!
//! Sources: R4 (MATRIX M5); the residue roots named here come from
//! `docs/host-reality.md` "residue outside XDG" / "runtime dir" / "shell
//! sandbox" via `omm_host::Roots`.
//!
//! Layering: this [`Bases::resolve`] is authoritative for writes. The two
//! read-only companions differ by design — `omm_manifest::contained` is
//! existing-paths-only (missing is `Err`), `omm_doctor::ledger::contained_path`
//! reports a missing-but-inside path as `Some` (D13 `vanished`, not `escaped`).
//! `crates/omm/tests/containment_parity.rs` locks the security agreement.

use std::fs;
use std::path::{Path, PathBuf};

use omm_host::fsx;
use omm_host::Roots;

use crate::error::{LedgerError, Result};
use crate::schema::{Base, RelPath};

/// How [`Bases::resolve`] words a path that canonicalizes outside its base
/// (an intermediate symlink pointing out). The uninstall planner tells this
/// reason apart from a refused root: an escaping entry is preserved and
/// named, a refused root aborts the plan.
pub const ESCAPE_REASON_PREFIX: &str = "resolves outside the base";

/// The four base roots plus the roots that are refused outright.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bases {
    /// `Roots::muse_config()`.
    pub muse_config: PathBuf,
    /// `Roots::muse_data()`.
    pub muse_data: PathBuf,
    /// `Roots::omm_root()`.
    pub omm: PathBuf,
    /// The workspace, when omm runs in one.
    pub workspace: Option<PathBuf>,
    /// The account home (`Roots::home`) — refused as a target.
    pub home: Option<PathBuf>,
    /// Paths the host writes outside every base that uninstall must NAME and
    /// never touch (host-reality.md "residue outside XDG", "runtime dir",
    /// "shell sandbox").
    pub residue: Vec<PathBuf>,
}

/// What was found at a resolved path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Missing,
    File,
    Dir,
    /// A symlink at the leaf (target not followed); the path is the link.
    Symlink,
    /// Socket, fifo, device.
    Other,
}

/// A contained absolute path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub base: Base,
    /// The canonicalized base root.
    pub base_root: PathBuf,
    /// The realpath: canonical when it exists (a leaf symlink keeps the
    /// link's own path), else the canonical deepest existing ancestor joined
    /// with the remaining components.
    pub path: PathBuf,
    pub state: State,
    /// The first ancestor between the (canonical) base root and the leaf
    /// that is a symlink now — followed for containment, never written
    /// through or removed through by omm (R2 sentinel). `None` when every
    /// ancestor is a real directory (or does not exist yet).
    pub via_symlink: Option<PathBuf>,
}

impl Bases {
    /// From the host roots and an optional workspace.
    pub fn from_roots(roots: &Roots, workspace: Option<&Path>) -> Bases {
        let mut residue = Vec::new();
        if let Some(p) = roots.session_name_authority_residue() {
            residue.push(p);
        }
        if let Some(p) = roots.runtime_dir() {
            residue.push(p);
        }
        Bases {
            muse_config: roots.muse_config(),
            muse_data: roots.muse_data(),
            omm: roots.omm_root(),
            workspace: workspace.map(Path::to_path_buf),
            home: roots.home.clone(),
            residue,
        }
    }

    /// The (uncanonicalized) root of a base.
    pub fn root(&self, base: Base) -> Result<&Path> {
        match base {
            Base::MuseConfig => Ok(&self.muse_config),
            Base::MuseData => Ok(&self.muse_data),
            Base::Omm => Ok(&self.omm),
            Base::Workspace => self
                .workspace
                .as_deref()
                .ok_or(LedgerError::NoBaseRoot(Base::Workspace)),
        }
    }

    /// Every base with a known root, in sort order.
    pub fn known(&self) -> Vec<Base> {
        Base::ALL
            .iter()
            .copied()
            .filter(|b| self.root(*b).is_ok())
            .collect()
    }

    /// The roots no operation may ever target: `/`, `$HOME` and every base
    /// root, canonicalized where they exist.
    pub fn refused_roots(&self) -> Vec<PathBuf> {
        let mut out = vec![PathBuf::from("/")];
        let mut push = |p: &Path| {
            out.push(fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()));
        };
        if let Some(h) = &self.home {
            push(h);
        }
        for b in Base::ALL {
            if let Ok(r) = self.root(b) {
                push(r);
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// True when `canonical` is one of [`Bases::refused_roots`].
    pub fn is_refused_root(&self, canonical: &Path) -> bool {
        self.refused_roots().iter().any(|r| r == canonical)
    }

    /// True when the base root is absent from the disk (canonicalize fails
    /// with NotFound): everything under it is Missing, never refused. A
    /// kill between the ledger write and the filesystem write leaves
    /// exactly this — the entry is recorded before its file lands.
    pub fn root_absent(&self, base: Base) -> bool {
        match self.root(base) {
            Ok(root) => matches!(
                fs::canonicalize(root),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound
            ),
            // No known root: let `resolve` report that.
            Err(_) => false,
        }
    }

    /// Resolve `rel` under `base` (R4). The base root must exist.
    pub fn resolve(&self, base: Base, rel: &RelPath) -> Result<Resolved> {
        let refuse = |reason: String| LedgerError::Containment {
            base,
            path: rel.as_str().to_string(),
            reason,
        };
        let root = self.root(base)?;
        let base_root = fs::canonicalize(root)
            .map_err(|e| refuse(format!("base root {} unusable: {e}", root.display())))?;
        if base_root == Path::new("/") {
            return Err(refuse("base root is `/`".to_string()));
        }
        if let Some(h) = &self.home {
            if fs::canonicalize(h).map(|c| c == base_root).unwrap_or(false)
                && base != Base::Workspace
            {
                return Err(refuse("base root is $HOME".to_string()));
            }
        }
        let joined = rel.under(&base_root);
        let via_symlink = symlinked_ancestor(&base_root, rel)?;
        match fs::symlink_metadata(&joined) {
            Ok(meta) if meta.file_type().is_symlink() => {
                // Contain the link itself (its parent canonicalized), never
                // its target.
                let parent = joined.parent().ok_or_else(|| refuse("no parent".into()))?;
                let canon_parent = fs::canonicalize(parent)
                    .map_err(|e| refuse(format!("parent unusable: {e}")))?;
                let name = joined
                    .file_name()
                    .ok_or_else(|| refuse("no file name".into()))?;
                let path = canon_parent.join(name);
                self.check_inside(base, rel, &base_root, &path)?;
                Ok(Resolved {
                    base,
                    base_root,
                    path,
                    state: State::Symlink,
                    via_symlink,
                })
            }
            Ok(meta) => {
                let path = fs::canonicalize(&joined)
                    .map_err(|e| refuse(format!("canonicalize failed: {e}")))?;
                self.check_inside(base, rel, &base_root, &path)?;
                let state = if meta.is_file() {
                    State::File
                } else if meta.is_dir() {
                    State::Dir
                } else {
                    State::Other
                };
                Ok(Resolved {
                    base,
                    base_root,
                    path,
                    state,
                    via_symlink,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Deepest existing ancestor, canonicalized; the rest is
                // appended lexically (RelPath guarantees no `..`).
                let mut ancestor = joined.clone();
                let mut rest: Vec<std::ffi::OsString> = Vec::new();
                loop {
                    let name = ancestor
                        .file_name()
                        .ok_or_else(|| refuse("walked off the base".into()))?
                        .to_owned();
                    let parent = ancestor
                        .parent()
                        .ok_or_else(|| refuse("walked off the base".into()))?
                        .to_path_buf();
                    rest.push(name);
                    ancestor = parent;
                    match fs::symlink_metadata(&ancestor) {
                        Ok(_) => break,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            if ancestor == base_root || !ancestor.starts_with(&base_root) {
                                return Err(refuse("base root vanished".into()));
                            }
                        }
                        Err(e) => return Err(refuse(format!("stat failed: {e}"))),
                    }
                }
                let canon_ancestor = fs::canonicalize(&ancestor)
                    .map_err(|e| refuse(format!("ancestor unusable: {e}")))?;
                if canon_ancestor != base_root {
                    self.check_inside(base, rel, &base_root, &canon_ancestor)?;
                }
                let mut path = canon_ancestor;
                for c in rest.iter().rev() {
                    path.push(c);
                }
                self.check_inside(base, rel, &base_root, &path)?;
                Ok(Resolved {
                    base,
                    base_root,
                    path,
                    state: State::Missing,
                    via_symlink,
                })
            }
            Err(e) => Err(refuse(format!("stat failed: {e}"))),
        }
    }

    /// `canonical` must be strictly inside `base_root` and not a refused root.
    fn check_inside(
        &self,
        base: Base,
        rel: &RelPath,
        base_root: &Path,
        canonical: &Path,
    ) -> Result<()> {
        let refuse = |reason: String| LedgerError::Containment {
            base,
            path: rel.as_str().to_string(),
            reason,
        };
        match canonical.strip_prefix(base_root) {
            Ok(rest) if rest.as_os_str().is_empty() => {
                Err(refuse("resolves to the base root itself".into()))
            }
            Ok(_) => {
                if self.is_refused_root(canonical) {
                    return Err(refuse(format!(
                        "resolves to a refused root {}",
                        canonical.display()
                    )));
                }
                Ok(())
            }
            Err(_) => Err(refuse(format!(
                "{ESCAPE_REASON_PREFIX}: {} is not under {}",
                canonical.display(),
                base_root.display()
            ))),
        }
    }
}

/// The first symlink among the ancestors of `rel` under the canonical
/// `base_root`, walking down one component at a time (the leaf excluded —
/// a leaf symlink is [`State::Symlink`]); the walk stops at the first
/// component that does not exist. A symlink is reported wherever it
/// points — inside or outside the base — because it is never something
/// omm wrote (R12).
fn symlinked_ancestor(base_root: &Path, rel: &RelPath) -> Result<Option<PathBuf>> {
    let mut cur = base_root.to_path_buf();
    let n = rel.depth();
    for comp in rel.components().take(n.saturating_sub(1)) {
        cur.push(comp);
        match fs::symlink_metadata(&cur) {
            Ok(meta) if meta.file_type().is_symlink() => return Ok(Some(cur)),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(LedgerError::io("stat", &cur, e)),
        }
    }
    Ok(None)
}

/// Canonicalize an existing path (our error type).
pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    Ok(fsx::canonicalize(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bases(dir: &Path) -> Bases {
        let b = Bases {
            muse_config: dir.join("config").join("muse"),
            muse_data: dir.join("data").join("muse"),
            omm: dir.join("config").join("omm"),
            workspace: Some(dir.join("ws")),
            home: Some(dir.join("home")),
            residue: vec![],
        };
        for p in [
            &b.muse_config,
            &b.muse_data,
            &b.omm,
            dir.join("home").as_path(),
        ] {
            fs::create_dir_all(p).unwrap();
        }
        fs::create_dir_all(dir.join("ws")).unwrap();
        b
    }

    fn rel(s: &str) -> RelPath {
        RelPath::new(s).unwrap()
    }

    #[test]
    fn contained_paths_resolve_missing_or_present() {
        let dir = tempfile::tempdir().unwrap();
        let b = bases(dir.path());
        let r = b
            .resolve(Base::MuseConfig, &rel("skills/x/SKILL.md"))
            .unwrap();
        assert_eq!(r.state, State::Missing);
        assert!(r.path.starts_with(&r.base_root));
        assert!(r.path.ends_with("skills/x/SKILL.md"));
        fs::create_dir_all(b.muse_config.join("skills/x")).unwrap();
        fs::write(b.muse_config.join("skills/x/SKILL.md"), b"s").unwrap();
        let r = b
            .resolve(Base::MuseConfig, &rel("skills/x/SKILL.md"))
            .unwrap();
        assert_eq!(r.state, State::File);
        assert_eq!(
            b.resolve(Base::MuseConfig, &rel("skills/x")).unwrap().state,
            State::Dir
        );
        // The workspace base needs a workspace.
        let mut nb = b.clone();
        nb.workspace = None;
        assert!(matches!(
            nb.resolve(Base::Workspace, &rel("a")),
            Err(LedgerError::NoBaseRoot(Base::Workspace))
        ));
        assert_eq!(
            nb.known(),
            vec![Base::MuseConfig, Base::MuseData, Base::Omm]
        );
        assert!(b.known().contains(&Base::Workspace));
    }

    #[test]
    fn grammar_refuses_dotdot_absolute_backslash() {
        // These never become a RelPath, so resolve cannot even be asked.
        for bad in ["../x", "a/../../etc/passwd", "/etc/passwd", "a\\b", "C:\\x"] {
            assert!(RelPath::new(bad).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlink_escape_is_refused_and_leaf_symlink_is_reported() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let b = bases(dir.path());
        let outside = dir.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret"), b"s").unwrap();
        // An intermediate symlink pointing outside the base: refused.
        symlink(&outside, b.muse_config.join("esc")).unwrap();
        match b.resolve(Base::MuseConfig, &rel("esc/secret")) {
            Err(LedgerError::Containment { reason, .. }) => {
                assert!(reason.contains("outside the base"), "{reason}")
            }
            other => panic!("{other:?}"),
        }
        // …also when the target does not exist yet.
        match b.resolve(Base::MuseConfig, &rel("esc/new/file")) {
            Err(LedgerError::Containment { .. }) => {}
            other => panic!("{other:?}"),
        }
        // A leaf symlink is reported as such and its target is not followed.
        symlink(outside.join("secret"), b.muse_config.join("leaf")).unwrap();
        let r = b.resolve(Base::MuseConfig, &rel("leaf")).unwrap();
        assert_eq!(r.state, State::Symlink);
        assert_eq!(
            r.path,
            fs::canonicalize(&b.muse_config).unwrap().join("leaf")
        );
        // A symlink to `.` resolves to the base root itself: refused.
        symlink(".", b.muse_config.join("self")).unwrap();
        match b.resolve(Base::MuseConfig, &rel("self/x")) {
            // `self/x` does not exist; ancestor `self` canonicalizes to the root → ok as ancestor,
            // and `x` under it is inside. That is contained and fine.
            Ok(r) => assert_eq!(r.path, fs::canonicalize(&b.muse_config).unwrap().join("x")),
            Err(e) => panic!("{e}"),
        }
        // A directory symlink to the base root, resolved as a directory: the
        // root itself, refused.
        symlink(&b.muse_config, b.muse_config.join("rootlink")).unwrap();
        assert_eq!(
            b.resolve(Base::MuseConfig, &rel("rootlink")).unwrap().state,
            State::Symlink
        );
        // An intermediate symlink to the home dir: refused (it is a refused root
        // and outside the base).
        symlink(dir.path().join("home"), b.muse_config.join("homelink")).unwrap();
        assert!(b.resolve(Base::MuseConfig, &rel("homelink/x")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn an_ancestor_symlink_inside_the_base_is_reported_not_refused() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let b = bases(dir.path());
        // The user's own directory, and the managed skill dir replaced by a
        // symlink to it (Gate 1 decision C).
        let mine = b.muse_config.join("skills").join("my-commit");
        fs::create_dir_all(mine.join("references")).unwrap();
        fs::write(mine.join("SKILL.md"), b"s").unwrap();
        fs::write(mine.join("references/r.md"), b"r").unwrap();
        symlink("my-commit", b.muse_config.join("skills").join("omm-commit")).unwrap();
        let canon_link = fs::canonicalize(&b.muse_config)
            .unwrap()
            .join("skills")
            .join("omm-commit");
        for rel_s in [
            "skills/omm-commit/SKILL.md",
            "skills/omm-commit/references/r.md",
        ] {
            let r = b.resolve(Base::MuseConfig, &rel(rel_s)).unwrap();
            assert_eq!(r.state, State::File, "{rel_s}: contained and present");
            assert_eq!(
                r.via_symlink.as_deref(),
                Some(canon_link.as_path()),
                "{rel_s}"
            );
            assert!(r.path.starts_with(&r.base_root));
        }
        // A missing leaf under the symlinked ancestor reports it too.
        let r = b
            .resolve(Base::MuseConfig, &rel("skills/omm-commit/gone.md"))
            .unwrap();
        assert_eq!(r.state, State::Missing);
        assert_eq!(r.via_symlink.as_deref(), Some(canon_link.as_path()));
        // Real directories all the way down: nothing reported.
        let r = b
            .resolve(Base::MuseConfig, &rel("skills/my-commit/SKILL.md"))
            .unwrap();
        assert_eq!(r.via_symlink, None);
        // A leaf symlink is the leaf's own state, not an ancestor.
        symlink(mine.join("SKILL.md"), b.muse_config.join("leaf.md")).unwrap();
        let r = b.resolve(Base::MuseConfig, &rel("leaf.md")).unwrap();
        assert_eq!(r.state, State::Symlink);
        assert_eq!(r.via_symlink, None);
        // A directory directly under the root that does not exist yet.
        let r = b
            .resolve(Base::MuseConfig, &rel("themes/x.tmTheme"))
            .unwrap();
        assert_eq!((r.state, r.via_symlink), (State::Missing, None));
    }

    #[test]
    fn refused_roots_cover_slash_home_and_every_base() {
        let dir = tempfile::tempdir().unwrap();
        let b = bases(dir.path());
        let roots = b.refused_roots();
        assert!(roots.contains(&PathBuf::from("/")));
        for p in [&b.muse_config, &b.muse_data, &b.omm, &dir.path().join("ws")] {
            assert!(
                roots.contains(&fs::canonicalize(p).unwrap()),
                "{}",
                p.display()
            );
        }
        assert!(roots.contains(&fs::canonicalize(dir.path().join("home")).unwrap()));
        assert!(b.is_refused_root(&fs::canonicalize(&b.omm).unwrap()));
        assert!(!b.is_refused_root(&fs::canonicalize(&b.omm).unwrap().join("x")));
        // A base root of `/` is refused outright.
        let mut slash = b.clone();
        slash.muse_data = PathBuf::from("/");
        assert!(matches!(
            slash.resolve(Base::MuseData, &rel("etc")),
            Err(LedgerError::Containment { .. })
        ));
        // A base root equal to $HOME is refused (workspace excepted).
        let mut homey = b.clone();
        homey.muse_config = dir.path().join("home");
        assert!(homey.resolve(Base::MuseConfig, &rel("x")).is_err());
        homey.workspace = Some(dir.path().join("home"));
        assert!(homey.resolve(Base::Workspace, &rel("x")).is_ok());
    }

    #[test]
    fn from_roots_names_residue() {
        let roots = omm_host::paths::Roots::resolve(&omm_host::paths::EnvView {
            home: Some("/h".into()),
            xdg_config_home: Some("/xc".into()),
            xdg_data_home: Some("/xd".into()),
            cwd: None,
            account_home: Some(PathBuf::from("/Users/real")),
            uid: Some(501),
            tmpdir: None,
        })
        .unwrap();
        let b = Bases::from_roots(&roots, Some(Path::new("/ws")));
        assert_eq!(b.muse_config, PathBuf::from("/xc/muse"));
        assert_eq!(b.muse_data, PathBuf::from("/xd/muse"));
        assert_eq!(b.omm, PathBuf::from("/xc/omm"));
        assert_eq!(b.workspace, Some(PathBuf::from("/ws")));
        assert_eq!(b.home, Some(PathBuf::from("/Users/real")));
        assert!(b
            .residue
            .iter()
            .any(|p| p.ends_with("session-name-authority")));
        assert!(b.residue.iter().any(|p| p.ends_with("tbh-501-rt/muse")));
    }

    #[test]
    fn root_absent_spots_a_gone_base_root() {
        let dir = tempfile::tempdir().unwrap();
        let b = bases(dir.path());
        assert!(!b.root_absent(Base::MuseConfig));
        fs::remove_dir_all(&b.muse_config).unwrap();
        assert!(b.root_absent(Base::MuseConfig));
        // A root that was never known is reported by `resolve`, not here.
        let mut no_ws = b.clone();
        no_ws.workspace = None;
        assert!(!no_ws.root_absent(Base::Workspace));
    }
}
