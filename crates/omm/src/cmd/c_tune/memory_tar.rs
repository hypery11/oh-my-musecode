//! A minimal POSIX ustar writer for `omm memory backup` and `omm memory gc`:
//! one `personal_project` root becomes one `<dir name>.tar` (PLAN.md 2.3).
//!
//! Regular files and directories only, in sorted order, every member under
//! the archive prefix `<dir name>/`. A symlink, socket, fifo or device is
//! named in the report and left out — omm never archives through a link
//! (R12 spirit). No compression, no pax or GNU extensions: a member name
//! must fit the ustar 100-byte `name` / 155-byte `prefix` split at a `/`.
//! The root directory itself is therefore NOT a member — a host-formula root
//! name is up to 113 characters (96 + 1 + 16), past the `name` field with no
//! `/` to split on (found by the first smoke run on a real sandbox path) —
//! and every extractor creates it from the members below it; those split as
//! `<dir name>` / `<rest>`, which fits every root plus a 100-byte file name.
//! A path that does not fit is an error naming it, never a silent
//! truncation. A tree past [`MAX_ARCHIVE_BYTES`] is refused before any byte
//! lands. The archive is built in memory so the caller can write it
//! atomically and verify it (`fsx::write_atomic` + SHA-256).

use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::error::{OmmError, Result};

/// The tar block size.
pub const BLOCK: usize = 512;
/// The most one archive may hold (file bytes); a memory scope is capped at
/// 16,305 B by the host, so anything near this is not a memory root.
pub const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
/// ustar `name` field width.
const NAME_LEN: usize = 100;
/// ustar `prefix` field width.
const PREFIX_LEN: usize = 155;

/// What [`archive`] produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Archive {
    /// The archive, blocks and end marker included.
    pub bytes: Vec<u8>,
    /// Regular files archived.
    pub files: usize,
    /// Directories archived below the root.
    pub dirs: usize,
    /// File bytes archived (before padding).
    pub file_bytes: u64,
    /// `(relative path, why)` for every entry left out.
    pub skipped: Vec<(String, String)>,
    /// Member names in archive order (`<prefix>/…`).
    pub members: Vec<String>,
}

/// Archive the tree at `root` under `prefix/` (the root's directory name).
/// `root` must be an existing directory; a symlink is refused, not followed.
pub fn archive(root: &Path, prefix: &str) -> Result<Archive> {
    if prefix.is_empty() || prefix.contains('/') || prefix == "." || prefix == ".." {
        return Err(OmmError::Usage(format!(
            "tar prefix {prefix:?} is not a single path component"
        )));
    }
    let meta = fs::symlink_metadata(root)
        .map_err(|e| OmmError::io(format!("stat {}", root.display()), e))?;
    if meta.file_type().is_symlink() {
        return Err(OmmError::Usage(format!(
            "{} is a symlink; a memory root is archived only as a real directory",
            root.display()
        )));
    }
    if !meta.is_dir() {
        return Err(OmmError::Usage(format!(
            "{} is not a directory",
            root.display()
        )));
    }
    let mut out = Archive::default();
    walk(root, prefix, "", &mut out)?;
    out.bytes.extend(std::iter::repeat_n(0u8, 2 * BLOCK));
    Ok(out)
}

/// One directory level, entries sorted by name; recursion follows real
/// directories only.
fn walk(dir: &Path, prefix: &str, rel: &str, out: &mut Archive) -> Result<()> {
    let mut names: Vec<std::ffi::OsString> = fs::read_dir(dir)
        .map_err(|e| OmmError::io(format!("read {}", dir.display()), e))?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| OmmError::io(format!("read {}", dir.display()), e))?;
    names.sort();
    for name in names {
        let path = dir.join(&name);
        let Some(name) = name.to_str() else {
            out.skipped.push((
                format!("{rel}{}", name.to_string_lossy()),
                "name is not UTF-8".to_string(),
            ));
            continue;
        };
        let rel_here = format!("{rel}{name}");
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| OmmError::io(format!("stat {}", path.display()), e))?;
        let ft = meta.file_type();
        if ft.is_symlink() {
            out.skipped
                .push((rel_here, "symlink; never followed".to_string()));
        } else if ft.is_dir() {
            push_dir(out, &format!("{prefix}/{rel_here}/"), &meta)?;
            walk(&path, prefix, &format!("{rel_here}/"), out)?;
        } else if ft.is_file() {
            let bytes =
                fs::read(&path).map_err(|e| OmmError::io(format!("read {}", path.display()), e))?;
            if out.file_bytes + bytes.len() as u64 > MAX_ARCHIVE_BYTES {
                return Err(OmmError::Usage(format!(
                    "{} exceeds the {} MiB archive limit at {}",
                    dir.display(),
                    MAX_ARCHIVE_BYTES / (1024 * 1024),
                    path.display()
                )));
            }
            push_file(out, &format!("{prefix}/{rel_here}"), &meta, &bytes)?;
        } else {
            out.skipped
                .push((rel_here, "not a regular file or directory".to_string()));
        }
    }
    Ok(())
}

fn push_dir(out: &mut Archive, name: &str, meta: &fs::Metadata) -> Result<()> {
    let header = header(name, 0, mode_of(meta, 0o755), mtime_of(meta), b'5')?;
    out.bytes.extend_from_slice(&header);
    out.members.push(name.to_string());
    out.dirs += 1;
    Ok(())
}

fn push_file(out: &mut Archive, name: &str, meta: &fs::Metadata, bytes: &[u8]) -> Result<()> {
    let header = header(
        name,
        bytes.len() as u64,
        mode_of(meta, 0o644),
        mtime_of(meta),
        b'0',
    )?;
    out.bytes.extend_from_slice(&header);
    out.bytes.extend_from_slice(bytes);
    let pad = (BLOCK - bytes.len() % BLOCK) % BLOCK;
    out.bytes.extend(std::iter::repeat_n(0u8, pad));
    out.members.push(name.to_string());
    out.files += 1;
    out.file_bytes += bytes.len() as u64;
    Ok(())
}

#[cfg(unix)]
fn mode_of(meta: &fs::Metadata, _default: u32) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(_meta: &fs::Metadata, default: u32) -> u32 {
    default
}

fn mtime_of(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Split a member name into ustar `(prefix, name)`; `Err` when no `/` split
/// fits the two fields.
fn split_name(full: &str) -> Result<(&str, &str)> {
    if full.len() <= NAME_LEN {
        return Ok(("", full));
    }
    for (i, b) in full.bytes().enumerate() {
        if b != b'/' {
            continue;
        }
        let (head, tail) = (&full[..i], &full[i + 1..]);
        if !head.is_empty()
            && !tail.is_empty()
            && head.len() <= PREFIX_LEN
            && tail.len() <= NAME_LEN
        {
            return Ok((head, tail));
        }
    }
    Err(OmmError::Usage(format!(
        "{full:?} does not fit a ustar header ({NAME_LEN}-byte name / {PREFIX_LEN}-byte prefix)"
    )))
}

/// One 512-byte ustar header.
fn header(full: &str, size: u64, mode: u32, mtime: u64, typeflag: u8) -> Result<[u8; BLOCK]> {
    let (prefix, name) = split_name(full)?;
    let mut h = [0u8; BLOCK];
    h[..name.len()].copy_from_slice(name.as_bytes());
    octal(&mut h[100..108], u64::from(mode));
    octal(&mut h[108..116], 0); // uid
    octal(&mut h[116..124], 0); // gid
    octal(&mut h[124..136], size);
    octal(&mut h[136..148], mtime);
    h[148..156].copy_from_slice(b"        ");
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    // uname / gname left empty; devmajor / devminor zero.
    h[345..345 + prefix.len()].copy_from_slice(prefix.as_bytes());
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    // Six octal digits, NUL, space — the historical checksum spelling every
    // reader accepts.
    let text = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(text.as_bytes());
    Ok(h)
}

/// Zero-padded octal, NUL-terminated, filling `field`.
fn octal(field: &mut [u8], value: u64) {
    let width = field.len() - 1;
    let text = format!("{value:0width$o}");
    let bytes = text.as_bytes();
    // A value too wide for the field is clipped to its low digits; every
    // caller stays under the field width (sizes under MAX_ARCHIVE_BYTES,
    // modes under 0o7777, mtimes for a few thousand years).
    let start = bytes.len().saturating_sub(width);
    field[..width].copy_from_slice(&bytes[start..]);
    field[width] = 0;
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A member as a test reader sees it.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct Member {
        pub name: String,
        pub typeflag: u8,
        pub size: u64,
        pub data: Vec<u8>,
    }

    fn field_str(h: &[u8]) -> String {
        let end = h.iter().position(|b| *b == 0).unwrap_or(h.len());
        String::from_utf8_lossy(&h[..end]).into_owned()
    }

    fn field_octal(h: &[u8]) -> u64 {
        let s = field_str(h);
        u64::from_str_radix(s.trim(), 8).unwrap_or(0)
    }

    /// Parse an archive produced by [`archive`], checking every checksum.
    pub(crate) fn read(bytes: &[u8]) -> Vec<Member> {
        let mut out = Vec::new();
        let mut at = 0;
        while at + BLOCK <= bytes.len() {
            let h = &bytes[at..at + BLOCK];
            if h.iter().all(|b| *b == 0) {
                break;
            }
            let mut copy = h.to_vec();
            copy[148..156].copy_from_slice(b"        ");
            let sum: u32 = copy.iter().map(|b| u32::from(*b)).sum();
            assert_eq!(field_octal(&h[148..156]), u64::from(sum), "checksum");
            assert_eq!(&h[257..263], b"ustar\0");
            let prefix = field_str(&h[345..500]);
            let name = field_str(&h[..100]);
            let full = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            let size = field_octal(&h[124..136]);
            let typeflag = h[156];
            at += BLOCK;
            let data = bytes[at..at + size as usize].to_vec();
            at += (size as usize).div_ceil(BLOCK) * BLOCK;
            out.push(Member {
                name: full,
                typeflag,
                size,
                data,
            });
        }
        out
    }

    /// The system `tar`, when there is one.
    fn system_tar() -> Option<std::path::PathBuf> {
        std::env::var_os("PATH").and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join("tar"))
                .find(|t| t.is_file())
        })
    }

    /// Extract with the system `tar`; `None` when there is none.
    fn extract(bytes: &[u8], dir: &Path) -> Option<()> {
        let tar = system_tar()?;
        let file = dir.join("archive.tar");
        std::fs::write(&file, bytes).unwrap();
        let dest = dir.join("extract");
        std::fs::create_dir_all(&dest).unwrap();
        let out = std::process::Command::new(tar)
            .args(["-xf", file.to_str().unwrap(), "-C", dest.to_str().unwrap()])
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        Some(())
    }

    /// A host-formula root name of the maximum length: 96 + 1 + 16.
    fn longest_root_name() -> String {
        format!("{}-{}", "p".repeat(96), "0123456789abcdef")
    }

    #[test]
    fn archives_files_and_dirs_sorted_skips_links_and_ends_with_two_zero_blocks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("root");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("MEMORY.md"), b"# hi\n").unwrap();
        std::fs::write(root.join("sub/note.md"), vec![b'x'; 513]).unwrap();
        std::fs::write(root.join("a.md"), b"").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("MEMORY.md", root.join("link.md")).unwrap();
        let a = archive(&root, "root").unwrap();
        assert_eq!(a.files, 3);
        assert_eq!(a.dirs, 1);
        assert_eq!(a.file_bytes, 5 + 513);
        assert_eq!(
            a.members,
            vec![
                "root/MEMORY.md",
                "root/a.md",
                "root/sub/",
                "root/sub/note.md"
            ]
        );
        #[cfg(unix)]
        assert_eq!(
            a.skipped,
            vec![("link.md".to_string(), "symlink; never followed".to_string())]
        );
        assert_eq!(a.bytes.len() % BLOCK, 0);
        assert!(a.bytes[a.bytes.len() - 2 * BLOCK..].iter().all(|b| *b == 0));
        let members = read(&a.bytes);
        assert_eq!(members.len(), 4);
        assert_eq!(members[0].name, "root/MEMORY.md");
        assert_eq!(members[0].typeflag, b'0');
        assert_eq!(members[0].data, b"# hi\n");
        assert_eq!(members[2].typeflag, b'5');
        assert_eq!(members[3].size, 513);
        assert_eq!(members[3].data.len(), 513);
        // Byte-stable for the same tree.
        assert_eq!(archive(&root, "root").unwrap().bytes, a.bytes);
    }

    #[test]
    fn the_system_tar_extracts_the_archive_byte_for_byte() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("private-tmp-omm-w1-cb608784b444ce8a");
        std::fs::create_dir_all(root.join("deep/er")).unwrap();
        std::fs::write(root.join("MEMORY.md"), b"# Project memory\nline\n").unwrap();
        std::fs::write(root.join("deep/er/x.md"), vec![b'y'; 1000]).unwrap();
        let a = archive(&root, "private-tmp-omm-w1-cb608784b444ce8a").unwrap();
        if extract(&a.bytes, tmp.path()).is_none() {
            eprintln!("skipped: no tar on PATH");
            return;
        }
        let back = tmp
            .path()
            .join("extract")
            .join("private-tmp-omm-w1-cb608784b444ce8a");
        assert_eq!(
            std::fs::read(back.join("MEMORY.md")).unwrap(),
            b"# Project memory\nline\n"
        );
        assert_eq!(
            std::fs::read(back.join("deep/er/x.md")).unwrap().len(),
            1000
        );
    }

    #[test]
    fn the_longest_host_root_name_fits_with_a_long_file_name_and_a_subdir() {
        // 113 chars of root name: every member splits as `<root>` /
        // `<rest>`; the root itself is not a member (no `/` to split on).
        let name = longest_root_name();
        assert_eq!(name.len(), 113);
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join(&name);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("MEMORY.md"), b"m\n").unwrap();
        let long_file = format!("{}.md", "f".repeat(97));
        std::fs::write(root.join(&long_file), b"f\n").unwrap();
        std::fs::write(root.join("sub/n.md"), b"n\n").unwrap();
        let a = archive(&root, &name).unwrap();
        assert_eq!(
            a.members,
            vec![
                format!("{name}/MEMORY.md"),
                format!("{name}/{long_file}"),
                format!("{name}/sub/"),
                format!("{name}/sub/n.md"),
            ]
        );
        let members = read(&a.bytes);
        assert_eq!(members[1].name, format!("{name}/{long_file}"));
        assert_eq!(members[2].typeflag, b'5');
        if extract(&a.bytes, tmp.path()).is_some() {
            let back = tmp.path().join("extract").join(&name);
            assert_eq!(std::fs::read(back.join("MEMORY.md")).unwrap(), b"m\n");
            assert_eq!(std::fs::read(back.join(&long_file)).unwrap(), b"f\n");
            assert_eq!(std::fs::read(back.join("sub/n.md")).unwrap(), b"n\n");
        }
        // A file name past the `name` field under the longest root: refused,
        // never truncated.
        std::fs::write(root.join(format!("{}.md", "g".repeat(101))), b"g").unwrap();
        assert!(matches!(archive(&root, &name), Err(OmmError::Usage(_))));
    }

    #[test]
    fn long_names_split_into_prefix_and_name_or_are_refused() {
        let dir = "d".repeat(113);
        let full = format!("{dir}/{}", "f".repeat(60));
        let (p, n) = split_name(&full).unwrap();
        assert_eq!(p, dir);
        assert_eq!(n, "f".repeat(60));
        let short = "a/b";
        assert_eq!(split_name(short).unwrap(), ("", "a/b"));
        // A directory member keeps its trailing slash in the name half.
        let d = format!("{dir}/sub/");
        assert_eq!(split_name(&d).unwrap(), (dir.as_str(), "sub/"));
        // No slash to split on: refused, never truncated.
        assert!(matches!(
            split_name(&"x".repeat(101)),
            Err(OmmError::Usage(_))
        ));
        assert!(split_name(&format!("{dir}/")).is_err(), "a bare long dir");
        // A leaf longer than the name field: refused.
        assert!(split_name(&format!("{dir}/{}", "f".repeat(101))).is_err());
        let h = header(&full, 7, 0o644, 1, b'0').unwrap();
        assert_eq!(&h[345..345 + 113], dir.as_bytes());
        assert_eq!(&h[..60], "f".repeat(60).as_bytes());
        assert_eq!(field_octal(&h[124..136]), 7);
    }

    #[test]
    fn refuses_a_symlink_root_a_file_root_and_a_bad_prefix() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(tmp.path().join("file"), b"x").unwrap();
        assert!(archive(&tmp.path().join("file"), "p").is_err());
        assert!(archive(&real, "a/b").is_err());
        assert!(archive(&real, "").is_err());
        #[cfg(unix)]
        {
            let link = tmp.path().join("link");
            std::os::unix::fs::symlink(&real, &link).unwrap();
            assert!(matches!(archive(&link, "p"), Err(OmmError::Usage(_))));
        }
        // An empty root is an empty archive: the end marker alone.
        let a = archive(&real, "p").unwrap();
        assert!(a.members.is_empty());
        assert_eq!(a.bytes.len(), 2 * BLOCK);
        assert!(read(&a.bytes).is_empty());
    }

    #[test]
    fn octal_fields_are_nul_terminated_and_zero_padded() {
        let mut f = [0xffu8; 8];
        octal(&mut f, 0o644);
        assert_eq!(&f, b"0000644\0");
        let mut s = [0xffu8; 12];
        octal(&mut s, 513);
        assert_eq!(&s, b"00000001001\0");
    }
}
