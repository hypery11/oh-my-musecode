//! Filesystem primitives shared by every writer in this crate.
//!
//! R10: every write to a file we do not own is atomic, lands on the resolved
//! realpath, and is preceded by a verified backup. ARCHITECTURE.md §2: that
//! backup lives under `$OMM/snapshots/<timestamp>/<base>/<path>`, never beside
//! the host's file — everything omm writes into Muse's roots is a ledger entry,
//! and R5 (byte-identical uninstall) cannot hold for stray `*.pre-omm.*` files.
//! Nothing in this module knows what the bytes mean.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::error::{HostError, Result};

/// Rolling snapshot retention under `$OMM/snapshots/` (ARCHITECTURE.md §4:
/// "keep 5"); shared by the pre-write backups here and the ledger's
/// pre-update snapshots.
pub const SNAPSHOTS_KEEP: usize = 5;

/// Hex-encoded SHA-256 of a byte slice.
pub fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Hex-encoded SHA-256 of a file, streamed.
pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|e| HostError::io("open", path, e))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| HostError::io("read", path, e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Read a whole file into bytes.
pub fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| HostError::io("read", path, e))
}

/// Read a whole file as UTF-8.
pub fn read_to_string(path: &Path) -> Result<String> {
    fs::read_to_string(path).map_err(|e| HostError::io("read", path, e))
}

/// `create_dir_all` with our error type.
pub fn create_dir_all(path: &Path) -> Result<()> {
    fs::create_dir_all(path).map_err(|e| HostError::io("create directory", path, e))
}

/// Canonicalize an existing path.
pub fn canonicalize(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path).map_err(|e| HostError::io("canonicalize", path, e))
}

/// The link target of a symlink, absolutised against the link's directory.
fn symlink_target(path: &Path) -> Result<PathBuf> {
    let target = fs::read_link(path).map_err(|e| HostError::io("read link", path, e))?;
    Ok(if target.is_absolute() {
        target
    } else {
        path.parent().map(|p| p.join(&target)).unwrap_or(target)
    })
}

/// True when `path` is a symlink whose target does not exist. `Path::exists`
/// follows links, so a dangling link looks like a missing file to it; a
/// writer that trusted that would replace the user's link with a regular file.
pub fn is_dangling_symlink(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|m| m.file_type().is_symlink() && !path.exists())
        .unwrap_or(false)
}

/// The realpath a write should land on: the canonicalized file when it exists
/// (a live symlink resolves to its target, so the link survives the write),
/// otherwise the canonicalized parent joined with the file name. A dangling
/// symlink is refused ([`HostError::DanglingSymlink`]) — it is neither a file
/// to replace nor a missing file to create. The parent must exist — callers
/// create it deliberately, never implicitly.
pub fn realpath_for_write(path: &Path) -> Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            let target = symlink_target(path)?;
            if !path.exists() {
                return Err(HostError::DanglingSymlink {
                    path: path.to_path_buf(),
                    target,
                });
            }
            canonicalize(path)
        }
        Ok(_) => canonicalize(path),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let parent = path.parent().ok_or_else(|| HostError::Io {
                context: "resolve parent of",
                path: path.to_path_buf(),
                source: std::io::Error::other("path has no parent"),
            })?;
            let name = path.file_name().ok_or_else(|| HostError::Io {
                context: "resolve file name of",
                path: path.to_path_buf(),
                source: std::io::Error::other("path has no file name"),
            })?;
            Ok(canonicalize(parent)?.join(name))
        }
        Err(e) => Err(HostError::io("stat", path, e)),
    }
}

/// UTC timestamp suitable for a file suffix, e.g. `20260902T042000Z`.
pub fn timestamp() -> String {
    let fmt = time::macros::format_description!("[year][month][day]T[hour][minute][second]Z");
    time::OffsetDateTime::now_utc()
        .format(&fmt)
        .unwrap_or_else(|_| "00000000T000000Z".to_string())
}

/// Write `bytes` to `realpath` atomically: a uniquely named sibling temp file
/// (`.<name>.omm-tmp-<random>`), fsync, rename. On any failure only the temp
/// file this call created is removed — never a pre-existing file, whatever its
/// name.
///
/// The mode of the file that lands is the mode of the file it replaces
/// (`fchmod` on the temp file, so the umask does not interfere), and for a
/// new file what a plain `create(2)` would give — `0666 & !umask`, the
/// user's own default (typically 0644). Without this every rewrite landed
/// the temp file's private 0600 (Gate 1: a user's 0644 `AGENTS.md`,
/// `settings.json` and `trust.json` all came back 0600, and the R5 snapshot
/// differed in mode after uninstall).
pub fn write_atomic(realpath: &Path, bytes: &[u8]) -> Result<()> {
    let dir = realpath.parent().ok_or_else(|| HostError::Io {
        context: "resolve parent of",
        path: realpath.to_path_buf(),
        source: std::io::Error::other("path has no parent"),
    })?;
    let name = realpath
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let existing = fs::metadata(realpath).ok().map(|m| m.permissions());
    // `NamedTempFile` removes its own file on drop, so every early return
    // below cleans up exactly what this call created.
    let prefix = format!(".{name}.omm-tmp-");
    let mut builder = tempfile::Builder::new();
    builder.prefix(&prefix);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The mode `open(2)` is asked for; the umask applies, as it does to
        // any file the user creates.
        builder.permissions(fs::Permissions::from_mode(0o666));
    }
    let mut tmp = builder
        .tempfile_in(dir)
        .map_err(|e| HostError::io("create temp file in", dir, e))?;
    if let Some(perm) = existing {
        tmp.as_file()
            .set_permissions(perm)
            .map_err(|e| HostError::io("set mode of temp file", tmp.path(), e))?;
    }
    tmp.as_file_mut()
        .write_all(bytes)
        .map_err(|e| HostError::io("write temp file", tmp.path(), e))?;
    tmp.as_file()
        .sync_all()
        .map_err(|e| HostError::io("fsync temp file", tmp.path(), e))?;
    // `persist` is `rename(2)`; on failure the temp file comes back in the
    // error and is removed when it drops.
    tmp.persist(realpath)
        .map_err(|e| HostError::io("rename into place", realpath, e.error))?;
    // Best effort: persist the directory entry too.
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(())
}

/// Copy `src` to `dest` and verify the copy's SHA-256 matches the original
/// before returning. A mismatching copy is removed.
pub fn verified_copy(src: &Path, dest: &Path) -> Result<()> {
    let original = sha256_file(src)?;
    fs::copy(src, dest).map_err(|e| HostError::io("copy backup to", dest, e))?;
    let copied = sha256_file(dest)?;
    if copied != original {
        let _ = fs::remove_file(dest);
        return Err(HostError::BackupMismatch {
            path: dest.to_path_buf(),
            detail: format!("original sha256 {original}, backup sha256 {copied}"),
        });
    }
    Ok(())
}

/// True for a snapshot directory name this crate creates: `<timestamp>` or
/// `<timestamp>-<n>` with the [`timestamp`] shape `YYYYMMDDTHHMMSSZ`.
pub fn is_snapshot_dir_name(name: &str) -> bool {
    let (stamp, suffix) = name.split_at(name.len().min(16));
    let b = stamp.as_bytes();
    b.len() == 16
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'T'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'Z'
        && (suffix.is_empty()
            || (suffix.len() > 1
                && suffix.starts_with('-')
                && suffix[1..].bytes().all(|c| c.is_ascii_digit())))
}

/// Create a fresh `<snapshots_dir>/<timestamp>[-n]/` for one snapshot. The
/// same-second sequence number is one past the highest already present for
/// that stamp — a slot freed by pruning is never reused, so names stay
/// chronological.
pub fn new_snapshot_dir(snapshots_dir: &Path) -> Result<PathBuf> {
    create_dir_all(snapshots_dir)?;
    let stamp = timestamp();
    let mut n = fs::read_dir(snapshots_dir)
        .map_err(|e| HostError::io("read dir", snapshots_dir, e))?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|name| is_snapshot_dir_name(name) && name.starts_with(&stamp))
        .map(|name| snapshot_order_key(&name).1 + 1)
        .max()
        .unwrap_or(0);
    loop {
        let dir = if n == 0 {
            snapshots_dir.join(&stamp)
        } else {
            snapshots_dir.join(format!("{stamp}-{n}"))
        };
        match fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => n += 1,
            Err(e) => return Err(HostError::io("create snapshot directory", &dir, e)),
        }
    }
}

/// Verified pre-write backup of `realpath` into a fresh snapshot directory:
/// `<snapshots_dir>/<timestamp>[-n]/<base>/<file name>` (ARCHITECTURE.md §2;
/// `base` is the ledger base the file belongs to, e.g. `muse-config`). Rolls
/// the snapshot directory to the newest [`SNAPSHOTS_KEEP`] afterwards and
/// returns the backup's path.
pub fn snapshot_backup(realpath: &Path, snapshots_dir: &Path, base: &str) -> Result<PathBuf> {
    let name = realpath.file_name().ok_or_else(|| HostError::Io {
        context: "resolve file name of",
        path: realpath.to_path_buf(),
        source: std::io::Error::other("path has no file name"),
    })?;
    let dir = new_snapshot_dir(snapshots_dir)?.join(base);
    create_dir_all(&dir)?;
    let dest = dir.join(name);
    verified_copy(realpath, &dest)?;
    prune_snapshots(snapshots_dir, SNAPSHOTS_KEEP)?;
    Ok(dest)
}

/// Chronological sort key of a snapshot directory name: the timestamp, then
/// the same-second sequence number (`<ts>` = 0, `<ts>-3` = 3).
fn snapshot_order_key(name: &str) -> (String, u32) {
    let (stamp, suffix) = name.split_at(name.len().min(16));
    let n = suffix
        .strip_prefix('-')
        .and_then(|d| d.parse::<u32>().ok())
        .unwrap_or(0);
    (stamp.to_string(), n)
}

/// Remove every snapshot directory beyond the newest `keep` (chronological by
/// name: timestamp, then same-second sequence number). Only directories whose
/// names match [`is_snapshot_dir_name`] are touched. Returns what was removed.
pub fn prune_snapshots(snapshots_dir: &Path, keep: usize) -> Result<Vec<PathBuf>> {
    let mut dirs: Vec<(String, PathBuf)> = match fs::read_dir(snapshots_dir) {
        Ok(entries) => entries
            .flatten()
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_string();
                is_snapshot_dir_name(&name).then(|| (name, e.path()))
            })
            .collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(HostError::io("read dir", snapshots_dir, e)),
    };
    dirs.sort_by_key(|(name, _)| snapshot_order_key(name));
    let mut removed = Vec::new();
    while dirs.len() > keep {
        let (_, oldest) = dirs.remove(0);
        fs::remove_dir_all(&oldest).map_err(|e| HostError::io("remove snapshot", &oldest, e))?;
        removed.push(oldest);
    }
    Ok(removed)
}

/// How long [`lock_exclusive`] waits for another writer by default: a commit
/// holds the lock for one hash + one backup + one rename (milliseconds), so
/// anything longer is a hung process.
pub const LOCK_WAIT_DEFAULT: Duration = Duration::from_secs(15);

/// An exclusive advisory lock (`flock(2)`) on a file omm owns, released when
/// dropped (closing the descriptor releases the lock, also on a crash). It
/// serializes writers of the host's config files across threads AND
/// processes; the lock file lives under `$OMM/locks/`, never beside the
/// host's file (ARCHITECTURE.md §2).
#[derive(Debug)]
pub struct FileLock {
    _file: File,
    path: PathBuf,
}

impl FileLock {
    /// The lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Take the exclusive lock on `path` (created with its parent directory when
/// missing), polling every 20 ms for up to `wait`, then
/// [`HostError::Locked`]. On non-unix targets the file is only opened: the
/// writers there are not serialized yet (documented gap,
/// `docs/KNOWN_ISSUES.md` §20).
pub fn lock_exclusive(path: &Path, wait: Duration) -> Result<FileLock> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| HostError::io("open lock file", path, e))?;
    let deadline = Instant::now() + wait;
    loop {
        match try_flock(&file) {
            Ok(true) => {
                return Ok(FileLock {
                    _file: file,
                    path: path.to_path_buf(),
                })
            }
            Ok(false) => {
                if Instant::now() >= deadline {
                    return Err(HostError::Locked {
                        path: path.to_path_buf(),
                        wait,
                    });
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(HostError::io("lock", path, e)),
        }
    }
}

/// `Ok(true)` locked, `Ok(false)` held elsewhere.
#[cfg(unix)]
fn try_flock(file: &File) -> std::io::Result<bool> {
    use rustix::fs::{flock, FlockOperation};
    match flock(file, FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(true),
        Err(e) if e == rustix::io::Errno::WOULDBLOCK || e == rustix::io::Errno::AGAIN => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[cfg(not(unix))]
fn try_flock(_file: &File) -> std::io::Result<bool> {
    Ok(true)
}

/// The uid this process runs as, read off a file it just created — no FFI,
/// no environment guess. `None` where the metadata carries no owner.
#[cfg(unix)]
pub fn current_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    tempfile::tempfile()
        .and_then(|f| f.metadata())
        .ok()
        .map(|m| m.uid())
}

/// See the unix definition.
#[cfg(not(unix))]
pub fn current_uid() -> Option<u32> {
    None
}

/// The deepest existing ancestor of `path` (`path` itself when it exists),
/// by `symlink_metadata` so a dangling symlink counts as existing. `None`
/// when nothing up to the root exists.
pub fn deepest_existing(path: &Path) -> Option<PathBuf> {
    path.ancestors()
        .find(|p| fs::symlink_metadata(p).is_ok())
        .map(Path::to_path_buf)
}

/// Bytes this process may still write on the filesystem holding `path`
/// (the deepest existing ancestor), unix only; `None` where the answer is
/// unknown — a caller never refuses on `None`.
#[cfg(unix)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    let probe = deepest_existing(path)?;
    let st = rustix::fs::statvfs(&probe).ok()?;
    Some(st.f_bavail.saturating_mul(st.f_frsize))
}

/// See the unix definition.
#[cfg(not(unix))]
pub fn free_bytes(_path: &Path) -> Option<u64> {
    None
}

/// Whether this process may write at `path`: `access(2)` with `W_OK` on the
/// file when it exists, else on its deepest existing ancestor directory
/// (where the new file would be created). `None` where nothing exists up
/// to the root.
#[cfg(unix)]
pub fn is_writable(path: &Path) -> Option<bool> {
    let probe = deepest_existing(path)?;
    Some(rustix::fs::access(&probe, rustix::fs::Access::WRITE_OK).is_ok())
}

/// See the unix definition.
#[cfg(not(unix))]
pub fn is_writable(path: &Path) -> Option<bool> {
    let probe = deepest_existing(path)?;
    Some(
        !fs::metadata(&probe)
            .map(|m| m.permissions().readonly())
            .unwrap_or(true),
    )
}

/// True when the file starts with `#!` — a shell script, i.e. the launcher.
pub fn is_shell_script(path: &Path) -> bool {
    let mut buf = [0u8; 2];
    File::open(path)
        .and_then(|mut f| f.read(&mut buf))
        .map(|n| n == 2 && &buf == b"#!")
        .unwrap_or(false)
}

#[cfg(unix)]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
pub fn is_executable(path: &Path) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_probes_answer_from_the_deepest_existing_ancestor() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("a").join("b").join("c.txt");
        assert_eq!(deepest_existing(&missing), Some(dir.path().to_path_buf()));
        assert_eq!(is_writable(&missing), Some(true));
        assert!(free_bytes(&missing).map(|b| b > 0).unwrap_or(true));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let ro = dir.path().join("ro");
            fs::create_dir(&ro).unwrap();
            fs::set_permissions(&ro, fs::Permissions::from_mode(0o500)).unwrap();
            let is_root = current_uid() == Some(0);
            assert_eq!(is_writable(&ro.join("new.txt")), Some(is_root));
            fs::set_permissions(&ro, fs::Permissions::from_mode(0o700)).unwrap();
        }
        assert_eq!(
            deepest_existing(Path::new("/nonexistent-omm-root-zz/x")),
            Some(PathBuf::from("/"))
        );
    }

    #[test]
    fn atomic_write_then_snapshot_backup_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.json");
        write_atomic(&realpath_for_write(&file).unwrap(), b"{\"a\":1}").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"{\"a\":1}");
        let snapshots = dir.path().join("omm").join("snapshots");
        let backup = snapshot_backup(&file, &snapshots, "muse-config").unwrap();
        assert!(backup.exists());
        assert!(backup.starts_with(&snapshots));
        assert!(
            backup.ends_with("muse-config/f.json"),
            "{}",
            backup.display()
        );
        assert_eq!(sha256_file(&backup).unwrap(), sha256_file(&file).unwrap());
        assert_eq!(
            dir.path().read_dir().unwrap().count(),
            2,
            "only f.json and omm/ beside the file: no temp file, no backup"
        );
    }

    #[test]
    fn snapshots_roll_to_the_newest_five() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.json");
        fs::write(&file, b"x").unwrap();
        let snapshots = dir.path().join("snapshots");
        // A foreign directory and file are never pruned.
        fs::create_dir_all(snapshots.join("keep-me")).unwrap();
        fs::write(snapshots.join("00000000T000000Z"), b"a file, not a dir").unwrap();
        let mut made = Vec::new();
        for _ in 0..7 {
            made.push(snapshot_backup(&file, &snapshots, "muse-config").unwrap());
        }
        let kept: Vec<PathBuf> = made.iter().filter(|p| p.exists()).cloned().collect();
        assert_eq!(kept.len(), SNAPSHOTS_KEEP, "{made:?}");
        assert_eq!(&kept[..], &made[2..], "the two oldest were removed");
        assert!(snapshots.join("keep-me").exists());
        assert!(snapshots.join("00000000T000000Z").is_file());
        assert!(is_snapshot_dir_name("20260902T042000Z"));
        assert!(is_snapshot_dir_name("20260902T042000Z-3"));
        assert!(!is_snapshot_dir_name("20260902T042000Z-"));
        assert!(!is_snapshot_dir_name("keep-me"));
        assert!(!is_snapshot_dir_name("20260902T042000Zx"));
        // Same-second sequence numbers sort numerically after the bare stamp.
        let mut names = vec![
            "20260902T042000Z-10",
            "20260902T042000Z-2",
            "20260902T042000Z",
            "20260902T041959Z-3",
        ];
        names.sort_by_key(|n| snapshot_order_key(n));
        assert_eq!(
            names,
            vec![
                "20260902T041959Z-3",
                "20260902T042000Z",
                "20260902T042000Z-2",
                "20260902T042000Z-10"
            ]
        );
    }

    #[test]
    fn atomic_write_never_removes_a_foreign_temp_file() {
        // A crash + pid reuse (or any foreign file) can leave a file with the
        // name a fixed pid-derived temp name would pick; it is not ours to delete.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("f.json");
        let foreign = dir
            .path()
            .join(format!(".f.json.omm-tmp-{}", std::process::id()));
        fs::write(&foreign, b"stale").unwrap();
        let realpath = realpath_for_write(&file).unwrap();
        assert!(
            write_atomic(&realpath, b"{}").is_ok(),
            "a foreign file must not block the write"
        );
        assert_eq!(
            fs::read(&foreign).unwrap(),
            b"stale",
            "the foreign file survives"
        );
        assert_eq!(fs::read(&file).unwrap(), b"{}");
        // A failing rename (target is a non-empty directory) leaves no temp file
        // behind and still does not touch the foreign file.
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("x"), b"x").unwrap();
        let foreign2 = dir
            .path()
            .join(format!(".sub.omm-tmp-{}", std::process::id()));
        fs::write(&foreign2, b"stale").unwrap();
        assert!(write_atomic(&sub, b"{}").is_err());
        assert_eq!(fs::read(&foreign2).unwrap(), b"stale");
        let names: Vec<String> = dir
            .path()
            .read_dir()
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        let leftovers: Vec<&String> = names
            .iter()
            .filter(|n| n.contains("omm-tmp"))
            .filter(|n| **n != foreign.file_name().unwrap().to_string_lossy())
            .filter(|n| **n != foreign2.file_name().unwrap().to_string_lossy())
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_refused_not_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("settings.json");
        std::os::unix::fs::symlink(dir.path().join("missing-target.json"), &link).unwrap();
        assert!(is_dangling_symlink(&link));
        assert!(
            matches!(
                realpath_for_write(&link),
                Err(HostError::DanglingSymlink { .. })
            ),
            "a dangling symlink is not a missing file"
        );
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        // A live symlink resolves to its target and the write lands there.
        let target = dir.path().join("real.json");
        fs::write(&target, b"old").unwrap();
        let live = dir.path().join("live.json");
        std::os::unix::fs::symlink(&target, &live).unwrap();
        assert!(!is_dangling_symlink(&live));
        let rp = realpath_for_write(&live).unwrap();
        assert_eq!(rp, fs::canonicalize(&target).unwrap());
        write_atomic(&rp, b"new").unwrap();
        assert!(fs::symlink_metadata(&live)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read(&target).unwrap(), b"new");
        // A relative dangling link resolves against the link's own directory.
        let rel = dir.path().join("rel.json");
        std::os::unix::fs::symlink("nowhere.json", &rel).unwrap();
        match realpath_for_write(&rel) {
            Err(HostError::DanglingSymlink { target, .. }) => {
                assert_eq!(target, dir.path().join("nowhere.json"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn exclusive_lock_serializes_threads_and_times_out() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locks").join("muse-config.lock");
        let held = lock_exclusive(&path, Duration::from_millis(50)).unwrap();
        assert_eq!(held.path(), path);
        // A second taker in this process is refused after the wait.
        let started = Instant::now();
        assert!(matches!(
            lock_exclusive(&path, Duration::from_millis(120)),
            Err(HostError::Locked { .. })
        ));
        assert!(started.elapsed() >= Duration::from_millis(100));
        // …and gets in once the holder drops.
        let t = std::thread::spawn({
            let path = path.clone();
            move || {
                let started = Instant::now();
                let _l = lock_exclusive(&path, Duration::from_secs(5)).unwrap();
                started.elapsed()
            }
        });
        std::thread::sleep(Duration::from_millis(200));
        drop(held);
        let waited = t.join().unwrap();
        assert!(waited >= Duration::from_millis(150), "{waited:?}");
        assert!(current_uid().is_some());
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_keeps_the_existing_mode_and_creates_under_the_umask() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        // What a plain create yields here: 0666 & !umask (typically 0644).
        let probe = dir.path().join("probe");
        File::create(&probe).unwrap();
        let created_mode = fs::metadata(&probe).unwrap().permissions().mode() & 0o777;
        // A new file gets the same mode a plain create would, never the
        // temp file's private 0600.
        let fresh = dir.path().join("fresh.json");
        write_atomic(&realpath_for_write(&fresh).unwrap(), b"{}").unwrap();
        assert_eq!(
            fs::metadata(&fresh).unwrap().permissions().mode() & 0o777,
            created_mode
        );
        // A pre-existing file keeps its own mode across the rewrite — the
        // user's 0644 and a deliberately private 0600 alike.
        for mode in [0o644u32, 0o600, 0o664] {
            let file = dir.path().join(format!("f-{mode:o}"));
            fs::write(&file, b"old").unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(mode)).unwrap();
            write_atomic(&realpath_for_write(&file).unwrap(), b"new").unwrap();
            assert_eq!(fs::read(&file).unwrap(), b"new");
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                mode,
                "mode {mode:o} must survive the rewrite"
            );
        }
    }

    #[test]
    fn realpath_requires_existing_parent() {
        let dir = tempfile::tempdir().unwrap();
        assert!(realpath_for_write(&dir.path().join("missing").join("f")).is_err());
        assert!(realpath_for_write(&dir.path().join("f")).is_ok());
    }

    #[test]
    fn detects_shell_script() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("muse");
        fs::write(&script, "#!/usr/bin/env bash\necho hi\n").unwrap();
        let bin = dir.path().join("muse-bin-1");
        fs::write(&bin, b"\xcf\xfa\xed\xfe").unwrap();
        assert!(is_shell_script(&script));
        assert!(!is_shell_script(&bin));
    }
}
