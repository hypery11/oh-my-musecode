//! `omm build` (ARCHITECTURE.md §5.2): content → the native package, the two
//! projections and the three marketplace catalogs.
//!
//! Every generator renders into an in-memory [`Package`] (relative path →
//! bytes), which is what the golden tests compare, what the digest probe
//! hands to the host, and what [`Package::write_to`] lands on disk: regular
//! files only (R12 — sources are read through, never linked), every write
//! atomic, every destination path canonicalized and contained, stale files
//! removed so a dropped asset disappears from the committed tree.

pub mod claude;
pub mod codex;
pub mod marketplace;
pub mod native;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use omm_host::fsx;
use omm_host::Invoker;
use serde_json::Value;

use crate::catalog::Catalog;
use crate::content::{Content, ContentFile};
use crate::error::{ManifestError, Result};
use crate::version::{self, OmmVersion};
use crate::{contained, posix, Repo};

/// A package tree in memory: `/`-separated relative paths → bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Package {
    files: BTreeMap<String, Vec<u8>>,
}

impl Package {
    /// An empty package.
    pub fn new() -> Package {
        Package::default()
    }

    /// Add a file. The path must be relative, `/`-separated, without `..`,
    /// empty segments or backslashes (plugin-contract.md §1.4).
    pub fn insert(&mut self, rel: impl Into<String>, bytes: Vec<u8>) -> Result<()> {
        let rel = rel.into();
        check_rel(&rel)?;
        if self.files.contains_key(&rel) {
            return Err(ManifestError::Generate(format!(
                "package path `{rel}` emitted twice"
            )));
        }
        self.files.insert(rel, bytes);
        Ok(())
    }

    /// Add a JSON document, pretty-printed with a trailing newline.
    pub fn insert_json(&mut self, rel: impl Into<String>, value: &Value) -> Result<()> {
        self.insert(rel, json_bytes(value)?)
    }

    /// Copy a content file in, dereferenced: the bytes are read through
    /// whatever the path is, but a symlink source is refused outright so the
    /// scan-first rule of R12 holds even when the loader was bypassed.
    pub fn copy_file(&mut self, rel: impl Into<String>, file: &ContentFile) -> Result<()> {
        let meta = std::fs::symlink_metadata(&file.abs)
            .map_err(|e| ManifestError::io("stat", &file.abs, e))?;
        if meta.file_type().is_symlink() {
            return Err(ManifestError::Symlink {
                path: file.abs.clone(),
            });
        }
        self.insert(rel, file.read()?)
    }

    /// The bytes at `rel`.
    pub fn get(&self, rel: &str) -> Option<&[u8]> {
        self.files.get(rel).map(Vec::as_slice)
    }

    /// The JSON document at `rel`.
    pub fn get_json(&self, rel: &str) -> Result<Value> {
        let bytes = self
            .get(rel)
            .ok_or_else(|| ManifestError::Generate(format!("package has no `{rel}`")))?;
        serde_json::from_slice(bytes).map_err(|e| ManifestError::Json {
            path: PathBuf::from(rel),
            source: e,
        })
    }

    /// `(path, bytes)` in path order.
    pub fn files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.files.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    /// Paths in order.
    pub fn paths(&self) -> Vec<&str> {
        self.files.keys().map(String::as_str).collect()
    }

    /// Number of files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// True when no file was emitted.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Filesystem entries the package materialises: files plus the distinct
    /// directories above them (`hr::PACKAGE_MAX_FS_ENTRIES`).
    pub fn fs_entries(&self) -> usize {
        let mut dirs: BTreeSet<&str> = BTreeSet::new();
        for path in self.files.keys() {
            for (i, ch) in path.char_indices() {
                if ch == '/' {
                    dirs.insert(&path[..i]);
                }
            }
        }
        self.files.len() + dirs.len()
    }

    /// The deepest path, in components (`hr::PACKAGE_MAX_PATH_DEPTH`).
    pub fn max_depth(&self) -> usize {
        self.files
            .keys()
            .map(|p| p.split('/').count())
            .max()
            .unwrap_or(0)
    }

    /// Total bytes.
    pub fn total_bytes(&self) -> u64 {
        self.files.values().map(|v| v.len() as u64).sum()
    }

    /// Read a committed package tree back (for the drift check). Symlinks are
    /// an error: a generated tree never contains one.
    pub fn read_from(dir: &Path) -> Result<Package> {
        let mut pkg = Package::new();
        if !dir.exists() {
            return Ok(pkg);
        }
        let root = fsx::canonicalize(dir)?;
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .sort_by_file_name()
        {
            let entry = entry.map_err(|e| ManifestError::Io {
                context: "walk",
                path: root.clone(),
                source: e
                    .into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("walk error")),
            })?;
            if entry.path_is_symlink() {
                return Err(ManifestError::Symlink {
                    path: entry.path().to_path_buf(),
                });
            }
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = posix(entry.path().strip_prefix(&root).unwrap_or(entry.path()));
            let bytes = std::fs::read(entry.path())
                .map_err(|e| ManifestError::io("read", entry.path(), e))?;
            pkg.insert(rel, bytes)?;
        }
        Ok(pkg)
    }

    /// Materialise under `dest` (created if missing): files not in the package
    /// are removed (deepest first, then empty directories), unchanged files are
    /// left alone, everything else is written atomically onto its realpath. A
    /// symlink found inside `dest` is removed, never followed — the tree is
    /// generated output, so nothing in it is the user's.
    pub fn write_to(&self, dest: &Path) -> Result<WriteReport> {
        fsx::create_dir_all(dest)?;
        let root = fsx::canonicalize(dest)?;
        let mut report = WriteReport::default();
        // Pass 1: remove what should not be there.
        for entry in walkdir::WalkDir::new(&root)
            .follow_links(false)
            .contents_first(true)
        {
            let entry = entry.map_err(|e| ManifestError::Io {
                context: "walk",
                path: root.clone(),
                source: e
                    .into_io_error()
                    .unwrap_or_else(|| std::io::Error::other("walk error")),
            })?;
            let path = entry.path();
            if path == root {
                continue;
            }
            let rel = posix(path.strip_prefix(&root).unwrap_or(path));
            if entry.path_is_symlink() {
                std::fs::remove_file(path)
                    .map_err(|e| ManifestError::io("remove symlink", path, e))?;
                report.removed.push(rel);
            } else if entry.file_type().is_file() {
                if !self.files.contains_key(&rel) {
                    std::fs::remove_file(path).map_err(|e| ManifestError::io("remove", path, e))?;
                    report.removed.push(rel);
                }
            } else if entry.file_type().is_dir() {
                let needed = self.files.keys().any(|k| k.starts_with(&format!("{rel}/")));
                if !needed {
                    // Empty now that its files went (contents_first); a
                    // non-empty foreign dir is left in place and reported.
                    match std::fs::remove_dir(path) {
                        Ok(()) => report.removed.push(rel),
                        Err(e) => report.kept_foreign.push(format!("{rel}: {e}")),
                    }
                }
            }
        }
        // Pass 2: write.
        for (rel, bytes) in &self.files {
            let path = root.join(rel);
            if let Some(parent) = path.parent() {
                fsx::create_dir_all(parent)?;
                contained(&root, parent)?;
            }
            let existing = std::fs::symlink_metadata(&path).ok();
            if let Some(meta) = &existing {
                if meta.file_type().is_symlink() {
                    std::fs::remove_file(&path)
                        .map_err(|e| ManifestError::io("remove symlink", &path, e))?;
                } else if meta.is_file() {
                    let current =
                        std::fs::read(&path).map_err(|e| ManifestError::io("read", &path, e))?;
                    if &current == bytes {
                        source_mode(&path)?;
                        report.unchanged.push(rel.clone());
                        continue;
                    }
                } else if meta.is_dir() {
                    return Err(ManifestError::Generate(format!(
                        "{} is a directory where the package needs a file",
                        path.display()
                    )));
                }
            }
            let realpath = fsx::realpath_for_write(&path)?;
            if !realpath.starts_with(&root) {
                return Err(ManifestError::Containment {
                    path: realpath,
                    root: root.clone(),
                });
            }
            fsx::write_atomic(&realpath, bytes)?;
            source_mode(&realpath)?;
            report.written.push(rel.clone());
        }
        Ok(report)
    }
}

/// Generated files are source: world-readable (`0644`), never the `0600` a
/// temp file is born with (`write_atomic` keeps the temp file's mode, which
/// is right for the host's config files and wrong for a committed tree).
#[cfg(unix)]
fn source_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta = std::fs::metadata(path).map_err(|e| ManifestError::io("stat", path, e))?;
    if meta.permissions().mode() & 0o777 != 0o644 {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
            .map_err(|e| ManifestError::io("chmod", path, e))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn source_mode(_path: &Path) -> Result<()> {
    Ok(())
}

/// What [`Package::write_to`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub written: Vec<String>,
    pub unchanged: Vec<String>,
    pub removed: Vec<String>,
    /// Directories that were not ours to empty (non-empty, not in the package).
    pub kept_foreign: Vec<String>,
}

impl WriteReport {
    /// True when nothing changed on disk.
    pub fn is_noop(&self) -> bool {
        self.written.is_empty() && self.removed.is_empty()
    }
}

/// Relative, `/`-separated, no `..`, no empty segment, no backslash, not
/// absolute (plugin-contract.md §1.4; R12 for the backslash).
pub fn check_rel(rel: &str) -> Result<()> {
    let bad = rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == "..");
    if bad {
        return Err(ManifestError::Generate(format!(
            "package path {rel:?} must be relative, `/`-separated, without `.`/`..`, empty segments or backslashes"
        )));
    }
    Ok(())
}

/// Pretty JSON with a trailing newline — the one serialization every
/// generated document uses, so the committed trees are byte-stable.
pub fn json_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| ManifestError::Generate(format!("serialize JSON: {e}")))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Where the native catalog's `integrity.digest` comes from.
#[derive(Clone, Debug)]
pub enum DigestSource<'a> {
    /// Ask the binary (`marketplace::package_digest`) — the only source a
    /// release may use (§5.2: never hand-computed, never copied).
    Host(&'a Invoker),
    /// A digest the caller already holds (golden tests; the drift check
    /// re-using the committed one when no host is available).
    Fixed(String),
}

/// Inputs of one build.
#[derive(Clone, Debug)]
pub struct BuildOptions<'a> {
    /// Override the version (default: `crates/omm/Cargo.toml`).
    pub version: Option<String>,
    /// Override the description (default: `crates/omm/Cargo.toml`).
    pub description: Option<String>,
    pub digest: DigestSource<'a>,
}

/// Everything one build produced.
#[derive(Clone, Debug)]
pub struct BuildOutput {
    pub plugin_id: String,
    pub version: String,
    pub description: String,
    /// `sha256:<64 hex>` — Muse's `package_sha256` of `native`.
    pub digest: String,
    pub native: Package,
    pub claude: Package,
    pub codex: Package,
    pub marketplace_native: Vec<u8>,
    pub marketplace_codex: Vec<u8>,
    pub marketplace_claude: Vec<u8>,
    /// `content/catalog.json` with its budget numbers refreshed
    /// (`budget::refreshed_catalog`): the one source file a build lands,
    /// and drift for `--check` while a number is stale.
    pub catalog: Vec<u8>,
}

impl BuildOutput {
    /// Every repo-relative path this build owns (packages flattened, catalogs).
    pub fn owned_paths(&self, repo: &Repo) -> Vec<String> {
        let mut out = Vec::new();
        let native = Repo::native_package_rel(&self.plugin_id);
        out.extend(self.native.paths().iter().map(|p| format!("{native}/{p}")));
        let claude = Repo::dist_claude_rel();
        out.extend(self.claude.paths().iter().map(|p| format!("{claude}/{p}")));
        let codex = Repo::dist_codex_rel();
        out.extend(self.codex.paths().iter().map(|p| format!("{codex}/{p}")));
        out.extend(repo.marketplace_paths().iter().map(|p| repo.relative(p)));
        out
    }
}

/// Resolve version and description from the options or the CLI crate.
pub fn resolve_version(repo: &Repo, opts: &BuildOptions<'_>) -> Result<OmmVersion> {
    match (&opts.version, &opts.description) {
        (Some(v), Some(d)) => Ok(OmmVersion {
            version: v.clone(),
            description: d.clone(),
        }),
        _ => {
            let read = version::omm_version(repo)?;
            Ok(OmmVersion {
                version: opts.version.clone().unwrap_or(read.version),
                description: opts.description.clone().unwrap_or(read.description),
            })
        }
    }
}

/// Render everything. Refuses content with unresolved problems.
pub fn build(
    repo: &Repo,
    catalog: &Catalog,
    content: &Content,
    opts: &BuildOptions<'_>,
) -> Result<BuildOutput> {
    content.require_clean()?;
    let v = resolve_version(repo, opts)?;
    let native = native::render(catalog, content, &v.version, &v.description)?;
    let claude = claude::render(catalog, content, &v.version, &v.description)?;
    let codex = codex::render(catalog, content, &v.version, &v.description)?;
    let digest = match &opts.digest {
        DigestSource::Host(inv) => marketplace::package_digest(inv, &native)?,
        DigestSource::Fixed(d) => {
            if !marketplace::is_digest(d) {
                return Err(ManifestError::Generate(format!(
                    "fixed digest {d:?} is not `sha256:` + 64 hex"
                )));
            }
            d.clone()
        }
    };
    let pid = catalog.plugin_id.as_str();
    let marketplace_native = json_bytes(&marketplace::render_native(
        pid,
        &v.version,
        &Repo::native_package_rel(pid),
        &digest,
    ))?;
    let marketplace_codex = json_bytes(&marketplace::render_codex(
        pid,
        &v.version,
        &v.description,
        &Repo::dist_codex_rel(),
    ))?;
    let marketplace_claude = json_bytes(&marketplace::render_claude(
        crate::MARKETPLACE_NAME,
        pid,
        &v.version,
        &v.description,
        &format!("./{}", Repo::dist_claude_rel()),
    ))?;
    let catalog_bytes =
        crate::budget::refreshed_catalog(&repo.catalog_path(), &crate::budget::estimate(content))?;
    Ok(BuildOutput {
        plugin_id: pid.to_string(),
        version: v.version,
        description: v.description,
        digest,
        native,
        claude,
        codex,
        marketplace_native,
        marketplace_codex,
        marketplace_claude,
        catalog: catalog_bytes,
    })
}

/// Per-destination write reports of [`write`].
#[derive(Clone, Debug, Default)]
pub struct WriteSummary {
    pub native: WriteReport,
    pub claude: WriteReport,
    pub codex: WriteReport,
    /// The three catalogs, `(repo-relative path, changed)`.
    pub catalogs: Vec<(String, bool)>,
    /// `content/catalog.json` with its budget numbers refreshed,
    /// `(repo-relative path, changed)`.
    pub catalog: Option<(String, bool)>,
}

impl WriteSummary {
    /// True when the repo was already up to date.
    pub fn is_noop(&self) -> bool {
        self.native.is_noop()
            && self.claude.is_noop()
            && self.codex.is_noop()
            && self.catalogs.iter().all(|(_, changed)| !changed)
            && self.catalog.as_ref().map(|(_, c)| !c).unwrap_or(true)
    }
}

/// Land a build in the repo: `plugins/<pid>/`, `dist/claude/`, `dist/codex/`
/// and the three catalogs (ARCHITECTURE.md §1).
pub fn write(repo: &Repo, out: &BuildOutput) -> Result<WriteSummary> {
    let mut summary = WriteSummary {
        native: out
            .native
            .write_to(&repo.native_package_dir(&out.plugin_id))?,
        claude: out.claude.write_to(&repo.dist_claude_dir())?,
        codex: out.codex.write_to(&repo.dist_codex_dir())?,
        catalogs: Vec::new(),
        catalog: None,
    };
    let catalogs = [
        (repo.marketplace_native_path(), &out.marketplace_native),
        (repo.marketplace_codex_path(), &out.marketplace_codex),
        (repo.marketplace_claude_path(), &out.marketplace_claude),
    ];
    for (path, bytes) in catalogs {
        let changed = write_file(&repo.root, &path, bytes)?;
        summary.catalogs.push((repo.relative(&path), changed));
    }
    let catalog_path = repo.catalog_path();
    let changed = write_file(&repo.root, &catalog_path, &out.catalog)?;
    summary.catalog = Some((repo.relative(&catalog_path), changed));
    Ok(summary)
}

/// Atomic write of one file under `root`; returns whether it changed.
pub fn write_file(root: &Path, path: &Path, bytes: &[u8]) -> Result<bool> {
    if let Some(parent) = path.parent() {
        fsx::create_dir_all(parent)?;
        contained(root, parent)?;
    }
    if fsx::is_dangling_symlink(path) {
        std::fs::remove_file(path).map_err(|e| ManifestError::io("remove symlink", path, e))?;
    }
    if path.is_file() {
        let current = std::fs::read(path).map_err(|e| ManifestError::io("read", path, e))?;
        if current == bytes {
            source_mode(path)?;
            return Ok(false);
        }
    }
    let realpath = fsx::realpath_for_write(path)?;
    if !realpath.starts_with(root) {
        return Err(ManifestError::Containment {
            path: realpath,
            root: root.to_path_buf(),
        });
    }
    fsx::write_atomic(&realpath, bytes)?;
    source_mode(&realpath)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_paths_are_checked_and_counted() {
        let mut p = Package::new();
        p.insert("a/b/c.txt", b"1".to_vec()).unwrap();
        p.insert("a/d.txt", b"22".to_vec()).unwrap();
        p.insert("e.txt", b"".to_vec()).unwrap();
        assert!(p.insert("a/d.txt", vec![]).is_err(), "twice");
        assert!(p.insert("../x", vec![]).is_err());
        assert!(p.insert("/abs", vec![]).is_err());
        assert!(p.insert("a\\b", vec![]).is_err());
        assert!(p.insert("a//b", vec![]).is_err());
        assert!(p.insert("./a", vec![]).is_err());
        assert_eq!(p.len(), 3);
        assert_eq!(p.fs_entries(), 3 + 2, "a/ and a/b/");
        assert_eq!(p.max_depth(), 3);
        assert_eq!(p.total_bytes(), 3);
        assert_eq!(p.paths(), vec!["a/b/c.txt", "a/d.txt", "e.txt"]);
    }

    #[test]
    fn write_to_lands_removes_stale_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out");
        std::fs::create_dir_all(dest.join("stale/deep")).unwrap();
        std::fs::write(dest.join("stale/deep/old.txt"), b"old").unwrap();
        std::fs::write(dest.join("keep.txt"), b"same").unwrap();
        let mut p = Package::new();
        p.insert("keep.txt", b"same".to_vec()).unwrap();
        p.insert("new/file.json", b"{}\n".to_vec()).unwrap();
        let r = p.write_to(&dest).unwrap();
        assert_eq!(r.written, vec!["new/file.json"]);
        assert_eq!(r.unchanged, vec!["keep.txt"]);
        assert_eq!(r.removed, vec!["stale/deep/old.txt", "stale/deep", "stale"]);
        assert!(!dest.join("stale").exists());
        assert_eq!(std::fs::read(dest.join("new/file.json")).unwrap(), b"{}\n");
        let again = p.write_to(&dest).unwrap();
        assert!(again.is_noop(), "{again:?}");
        assert_eq!(Package::read_from(&dest).unwrap(), p);
        assert!(Package::read_from(&dir.path().join("missing"))
            .unwrap()
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn write_to_never_follows_a_symlink_in_the_generated_tree() {
        let dir = tempfile::tempdir().unwrap();
        let outside = dir.path().join("outside.txt");
        std::fs::write(&outside, b"user file").unwrap();
        let dest = dir.path().join("out");
        std::fs::create_dir_all(&dest).unwrap();
        std::os::unix::fs::symlink(&outside, dest.join("a.txt")).unwrap();
        let mut p = Package::new();
        p.insert("a.txt", b"generated".to_vec()).unwrap();
        p.write_to(&dest).unwrap();
        assert_eq!(
            std::fs::read(&outside).unwrap(),
            b"user file",
            "target untouched"
        );
        assert!(!std::fs::symlink_metadata(dest.join("a.txt"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(dest.join("a.txt")).unwrap(), b"generated");
        // A symlink source is refused by copy_file.
        let link = dir.path().join("link.md");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let cf = ContentFile {
            rel: "link.md".into(),
            abs: link,
            bytes: 0,
        };
        assert!(matches!(
            Package::new().copy_file("x.md", &cf),
            Err(ManifestError::Symlink { .. })
        ));
        // read_from refuses a tree with a symlink.
        std::os::unix::fs::symlink(&outside, dest.join("b.txt")).unwrap();
        assert!(matches!(
            Package::read_from(&dest),
            Err(ManifestError::Symlink { .. })
        ));
    }

    #[test]
    fn write_file_is_atomic_and_reports_change() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        let f = root.join("sub").join("m.json");
        assert!(write_file(&root, &f, b"{}\n").unwrap());
        assert!(!write_file(&root, &f, b"{}\n").unwrap());
        assert!(write_file(&root, &f, b"{\"a\":1}\n").unwrap());
        let names: Vec<String> = root
            .join("sub")
            .read_dir()
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["m.json"], "no temp files left");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
            assert_eq!(
                mode, 0o644,
                "generated files are source, not 0600 temp files"
            );
        }
    }
}
