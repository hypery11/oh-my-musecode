//! Content hashes and the non-regular-entry sentinel (R2).
//!
//! A ledger `sha256` is always 64 lowercase hex characters (SHA-256 of the
//! bytes omm wrote, or the tree hash of a directory it wrote). Anything on
//! disk that is not a regular file — a symlink, socket, fifo, device, or a
//! directory containing one — observes as a [`Observed::NonRegular`] sentinel
//! whose text starts with [`SENTINEL_PREFIX`] and therefore can never equal
//! a content hash. Reconcile stages it, uninstall preserves it, snapshot skips
//! it: omm never wrote a symlink (R12) so whatever is there is not ours.

use std::fs;
use std::path::Path;

use omm_host::fsx;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::{LedgerError, Result};

/// Prefix of every sentinel; contains `:` so no hex digest can start with it.
pub const SENTINEL_PREFIX: &str = "non-regular:";

/// What hashing a ledgered path observed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observed {
    /// Nothing at the path.
    Missing,
    /// A regular file (or an all-regular tree): the content hash.
    Content(String),
    /// A symlink, socket, fifo, device or a tree containing one: the
    /// sentinel text (`non-regular:<kind>:<relative path>`).
    NonRegular(String),
}

impl Observed {
    /// True only for a content hash equal to `sha`.
    pub fn equals(&self, sha: &str) -> bool {
        matches!(self, Observed::Content(h) if h == sha)
    }

    /// The hash or sentinel text, `None` when missing.
    pub fn text(&self) -> Option<&str> {
        match self {
            Observed::Missing => None,
            Observed::Content(s) | Observed::NonRegular(s) => Some(s),
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, Observed::Missing)
    }
}

/// True when `s` is a sentinel rather than a content hash.
pub fn is_sentinel(s: &str) -> bool {
    s.starts_with(SENTINEL_PREFIX)
}

/// Build a sentinel for a non-regular entry of `kind` at `rel`.
pub fn sentinel(kind: &str, rel: &str) -> String {
    format!("{SENTINEL_PREFIX}{kind}:{rel}")
}

/// Hex SHA-256 of bytes.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    fsx::sha256_bytes(bytes)
}

/// Hex SHA-256 of a regular file. A symlink is NOT followed: it errors, so a
/// caller that wants a verdict rather than a hash uses [`observe`].
pub fn sha256_file(path: &Path) -> Result<String> {
    let meta = fs::symlink_metadata(path).map_err(|e| LedgerError::io("stat", path, e))?;
    if !meta.file_type().is_file() {
        return Err(LedgerError::Io {
            context: "hash",
            path: path.to_path_buf(),
            source: std::io::Error::other(format!(
                "not a regular file ({})",
                kind_name(&meta.file_type())
            )),
        });
    }
    Ok(fsx::sha256_file(path)?)
}

/// Observe whatever is at `path` without following a leaf symlink.
pub fn observe(path: &Path) -> Result<Observed> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Observed::Missing),
        Err(e) => return Err(LedgerError::io("stat", path, e)),
    };
    let ft = meta.file_type();
    if ft.is_file() {
        return Ok(Observed::Content(fsx::sha256_file(path)?));
    }
    if ft.is_dir() {
        return observe_tree(path);
    }
    Ok(Observed::NonRegular(sentinel(kind_name(&ft), "")))
}

/// Deterministic hash of a directory tree: one line per entry in sorted
/// order, `d <rel>` for a directory and `f <rel> <sha256>` for a file, over
/// SHA-256. The first non-regular entry (symlink, socket, fifo, device — the
/// walk never follows links) short-circuits to a sentinel naming it.
pub fn observe_tree(root: &Path) -> Result<Observed> {
    let root_meta = fs::symlink_metadata(root).map_err(|e| LedgerError::io("stat", root, e))?;
    if root_meta.file_type().is_symlink() {
        return Ok(Observed::NonRegular(sentinel("symlink", "")));
    }
    if !root_meta.is_dir() {
        return observe(root);
    }
    let mut hasher = Sha256::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .min_depth(1)
        .sort_by_file_name()
    {
        let entry = entry.map_err(|e| LedgerError::Io {
            context: "walk",
            path: root.to_path_buf(),
            source: std::io::Error::other(e.to_string()),
        })?;
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| LedgerError::Io {
                context: "strip prefix of",
                path: entry.path().to_path_buf(),
                source: std::io::Error::other(e.to_string()),
            })?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let ft = entry.file_type();
        if ft.is_dir() {
            hasher.update(format!("d {rel}\n").as_bytes());
        } else if ft.is_file() {
            let sha = fsx::sha256_file(entry.path())?;
            hasher.update(format!("f {rel} {sha}\n").as_bytes());
        } else {
            return Ok(Observed::NonRegular(sentinel(kind_name(&ft), &rel)));
        }
    }
    Ok(Observed::Content(hex::encode(hasher.finalize())))
}

#[cfg(unix)]
fn kind_name(ft: &fs::FileType) -> &'static str {
    use std::os::unix::fs::FileTypeExt;
    if ft.is_symlink() {
        "symlink"
    } else if ft.is_socket() {
        "socket"
    } else if ft.is_fifo() {
        "fifo"
    } else if ft.is_block_device() || ft.is_char_device() {
        "device"
    } else if ft.is_dir() {
        "directory"
    } else if ft.is_file() {
        "file"
    } else {
        "other"
    }
}

#[cfg(not(unix))]
fn kind_name(ft: &fs::FileType) -> &'static str {
    if ft.is_symlink() {
        "symlink"
    } else if ft.is_dir() {
        "directory"
    } else if ft.is_file() {
        "file"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentinel_never_equals_a_content_hash() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("f");
        fs::write(&f, b"hello").unwrap();
        let sha = sha256_file(&f).unwrap();
        assert_eq!(sha.len(), 64);
        assert!(sha.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(sha, sha256_bytes(b"hello"));
        for s in [
            sentinel("symlink", ""),
            sentinel("socket", "a/b"),
            sentinel("fifo", "x"),
        ] {
            assert!(is_sentinel(&s));
            assert_ne!(s, sha);
            assert!(!s.bytes().all(|b| b.is_ascii_hexdigit()));
            assert!(!Observed::NonRegular(s.clone()).equals(&s));
        }
        assert!(!is_sentinel(&sha));
        assert!(Observed::Content(sha.clone()).equals(&sha));
        assert!(!Observed::Missing.equals(&sha));
        assert_eq!(Observed::Missing.text(), None);
        assert_eq!(
            observe(&dir.path().join("nope")).unwrap(),
            Observed::Missing
        );
        assert_eq!(observe(&f).unwrap(), Observed::Content(sha));
    }

    #[cfg(unix)]
    #[test]
    fn tree_hash_is_deterministic_and_a_symlink_is_a_sentinel() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("t");
        fs::create_dir_all(t.join("sub")).unwrap();
        fs::write(t.join("b.txt"), b"b").unwrap();
        fs::write(t.join("sub").join("a.txt"), b"a").unwrap();
        let h1 = observe_tree(&t).unwrap();
        let h2 = observe(&t).unwrap();
        assert_eq!(h1, h2);
        let Observed::Content(c) = &h1 else {
            panic!("{h1:?}")
        };
        assert_eq!(c.len(), 64);
        // Same content elsewhere hashes the same; a byte change differs.
        let u = dir.path().join("u");
        fs::create_dir_all(u.join("sub")).unwrap();
        fs::write(u.join("b.txt"), b"b").unwrap();
        fs::write(u.join("sub").join("a.txt"), b"a").unwrap();
        assert_eq!(observe_tree(&u).unwrap(), h1);
        fs::write(u.join("sub").join("a.txt"), b"A").unwrap();
        assert_ne!(observe_tree(&u).unwrap(), h1);
        // A symlink anywhere in the tree is a sentinel naming it.
        std::os::unix::fs::symlink("b.txt", t.join("sub").join("link")).unwrap();
        match observe_tree(&t).unwrap() {
            Observed::NonRegular(s) => {
                assert!(is_sentinel(&s));
                assert!(s.ends_with("symlink:sub/link"), "{s}");
            }
            other => panic!("{other:?}"),
        }
        // A fifo too.
        let fdir = dir.path().join("fifo-tree");
        fs::create_dir_all(&fdir).unwrap();
        let status = std::process::Command::new("mkfifo")
            .arg(fdir.join("pipe"))
            .status()
            .unwrap();
        assert!(status.success());
        match observe_tree(&fdir).unwrap() {
            Observed::NonRegular(s) => assert!(s.contains("fifo"), "{s}"),
            other => panic!("{other:?}"),
        }
        // A leaf symlink is not followed by observe / sha256_file.
        let link = dir.path().join("leaf");
        std::os::unix::fs::symlink(t.join("b.txt"), &link).unwrap();
        assert!(matches!(observe(&link).unwrap(), Observed::NonRegular(_)));
        assert!(sha256_file(&link).is_err());
        // A symlinked root is a sentinel, not the target's tree hash.
        let root_link = dir.path().join("root-link");
        std::os::unix::fs::symlink(&u, &root_link).unwrap();
        assert!(matches!(
            observe_tree(&root_link).unwrap(),
            Observed::NonRegular(_)
        ));
    }
}
