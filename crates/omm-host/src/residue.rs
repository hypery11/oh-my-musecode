//! What the host leaves outside every sandbox, and how to sweep what a killed
//! host left behind.
//!
//! Measured 2026-09-02 on `1.0.1-R2006.1` (`docs/host-reality.md` "Paths"):
//! every session — a sandboxed probe included, whatever `HOME` / `XDG_*` say —
//! writes into a per-uid runtime dir `/private/tmp/tbh-<uid>-rt/muse/`
//! (macOS; the binary's fallback literals are `$XDG_DATA_HOME/muse/runtime/muse`
//! and `~/.local/share/muse/runtime/muse`):
//!
//! * `sessions/<uuid>.json` — the session-messaging registry entry, carrying
//!   `workspace_label`, `target_eligibility` and `process_generation_hint:
//!   "pid=<pid>"`, so a running probe is visible to the user's live sessions
//!   (`local_session_messaging` is a default-ON gate);
//! * `ms-<id>.sock` + `ms-<id>.sock.lease` (`{"pid":<pid>,…}`) for a
//!   message-capable session;
//! * a persistent, empty `.session-registry.mutation.lock`;
//!
//! plus `$TMPDIR/muse-shell-sandbox-<uuid>/`. All but the mutation lock are
//! removed on normal exit and all survive a kill — 716 stale socket/lease
//! pairs and 742 shell-sandbox dirs were found on the measuring machine after
//! earlier killed sessions. The shell-sandbox dir carries no pid and cannot be
//! attributed, so only the registry is swept here; the sweep never touches
//! the mutation lock, a live pid, or any name it does not recognise.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::{HostError, Result};

/// `tbh-<uid>-rt`, under [`RUNTIME_TMP_ROOT`].
pub const RUNTIME_DIR_PREFIX: &str = "tbh-";
/// See [`RUNTIME_DIR_PREFIX`].
pub const RUNTIME_DIR_SUFFIX: &str = "-rt";
/// The temp root the runtime dir lives under. The binary hard-codes
/// `/private/tmp` (its string table); measured on macOS only — on other
/// unixes `/tmp` is the same directory or the obvious twin.
#[cfg(target_os = "macos")]
pub const RUNTIME_TMP_ROOT: &str = "/private/tmp";
/// See the macOS definition.
#[cfg(not(target_os = "macos"))]
pub const RUNTIME_TMP_ROOT: &str = "/tmp";
/// The registry subdirectory of the runtime dir.
pub const RUNTIME_SESSIONS_SUBDIR: &str = "sessions";
/// The persistent lock file the sweep never touches.
pub const RUNTIME_MUTATION_LOCK: &str = ".session-registry.mutation.lock";
/// Prefix of the per-session shell sandbox dir under `$TMPDIR`.
pub const SHELL_SANDBOX_PREFIX: &str = "muse-shell-sandbox-";
/// Socket / lease name prefix and suffixes.
pub const ENDPOINT_PREFIX: &str = "ms-";
/// See [`ENDPOINT_PREFIX`].
pub const ENDPOINT_SOCKET_SUFFIX: &str = ".sock";
/// See [`ENDPOINT_PREFIX`].
pub const ENDPOINT_LEASE_SUFFIX: &str = ".sock.lease";

/// `/private/tmp/tbh-<uid>-rt/muse` for a uid.
pub fn runtime_dir_for_uid(uid: u32) -> PathBuf {
    Path::new(RUNTIME_TMP_ROOT)
        .join(format!("{RUNTIME_DIR_PREFIX}{uid}{RUNTIME_DIR_SUFFIX}"))
        .join("muse")
}

/// The runtime dir of the current user, if the uid is known.
pub fn runtime_dir() -> Option<PathBuf> {
    crate::fsx::current_uid().map(runtime_dir_for_uid)
}

/// What one sweep removed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Swept {
    /// `ms-*.sock` / `ms-*.sock.lease` removed, by path.
    pub endpoints: Vec<PathBuf>,
    /// `sessions/<uuid>.json` removed, by path.
    pub session_entries: Vec<PathBuf>,
    /// The pids those entries named.
    pub pids: BTreeSet<u32>,
}

impl Swept {
    /// Nothing was removed.
    pub fn is_empty(&self) -> bool {
        self.endpoints.is_empty() && self.session_entries.is_empty()
    }
}

/// The pid a lease names (`{"pid":<n>,…}`).
pub fn lease_pid(text: &str) -> Option<u32> {
    let v: Value = serde_json::from_str(text).ok()?;
    v.get("pid")
        .and_then(Value::as_u64)
        .and_then(|p| u32::try_from(p).ok())
}

/// The pid a registry entry names (`"process_generation_hint":"pid=<n>"`).
pub fn session_entry_pid(text: &str) -> Option<u32> {
    let v: Value = serde_json::from_str(text).ok()?;
    v.get("process_generation_hint")
        .and_then(Value::as_str)?
        .strip_prefix("pid=")?
        .parse()
        .ok()
}

/// Remove every endpoint pair and registry entry of `dir` whose pid satisfies
/// `dead`. A missing `dir` is an empty sweep. Names outside the two shapes
/// (the mutation lock, anything else) are never touched; a lease or entry
/// that names no pid is kept.
pub fn sweep_runtime_dir(dir: &Path, dead: &dyn Fn(u32) -> bool) -> Result<Swept> {
    let mut swept = Swept::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(swept),
        Err(e) => return Err(HostError::io("read dir", dir, e)),
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !(name.starts_with(ENDPOINT_PREFIX) && name.ends_with(ENDPOINT_LEASE_SUFFIX)) {
            continue;
        }
        let lease = entry.path();
        let Some(pid) = std::fs::read_to_string(&lease)
            .ok()
            .and_then(|t| lease_pid(&t))
        else {
            continue;
        };
        if !dead(pid) {
            continue;
        }
        let socket = dir.join(name.trim_end_matches(".lease"));
        for p in [socket, lease] {
            match std::fs::remove_file(&p) {
                Ok(()) => swept.endpoints.push(p),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(HostError::io("remove stale endpoint", &p, e)),
            }
        }
        swept.pids.insert(pid);
    }
    let sessions = dir.join(RUNTIME_SESSIONS_SUBDIR);
    if let Ok(entries) = std::fs::read_dir(&sessions) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Some(pid) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|t| session_entry_pid(&t))
            else {
                continue;
            };
            if !dead(pid) {
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    swept.session_entries.push(path);
                    swept.pids.insert(pid);
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(HostError::io("remove stale registry entry", &path, e)),
            }
        }
    }
    Ok(swept)
}

/// Sweep what one killed host (by pid) left in the current user's runtime dir.
/// Best effort: a missing dir or an unknown uid is an empty sweep.
pub fn sweep_killed_pid(pid: u32) -> Result<Swept> {
    match runtime_dir() {
        Some(dir) => sweep_runtime_dir(&dir, &|p| p == pid),
        None => Ok(Swept::default()),
    }
}

/// Which of `pids` are alive, by one `ps -o pid= -p <list>` call (unix). A
/// pid that `ps` cannot see counts as dead. When `ps` cannot run, or refuses
/// the request (macOS: `process id too large` rejects the whole list), every
/// pid counts as alive so nothing is swept on a bad guess.
#[cfg(unix)]
pub fn pids_alive(pids: &BTreeSet<u32>) -> BTreeSet<u32> {
    if pids.is_empty() {
        return BTreeSet::new();
    }
    let list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    match std::process::Command::new("ps")
        .args(["-o", "pid=", "-p", &list])
        .stdin(std::process::Stdio::null())
        .output()
    {
        Ok(out) if out.stderr.is_empty() => String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .filter_map(|t| t.parse::<u32>().ok())
            .filter(|p| pids.contains(p))
            .collect(),
        // `ps` complained (an unparsable pid, a missing option): no verdict.
        Ok(_) | Err(_) => pids.clone(),
    }
}

/// See the unix definition; without `ps` every pid counts as alive.
#[cfg(not(unix))]
pub fn pids_alive(pids: &BTreeSet<u32>) -> BTreeSet<u32> {
    pids.clone()
}

/// One row of `ps -A -o pid=,ppid=,pgid=,stat=`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessRow {
    pub pid: u32,
    pub ppid: u32,
    /// The process group — a hook the host spawns can sit in its own
    /// (`set -m`, `setsid`), which is why a kill by pid walks the tree
    /// instead of signalling one group.
    pub pgid: u32,
    /// `stat` starts with `Z`: exited, not yet reaped — dead for every purpose here.
    pub zombie: bool,
}

/// A snapshot of every process `ps` can see, taken to find what a host
/// spawned before the host itself is killed (once the host is gone its
/// children are reparented to pid 1 and the tree is unrecoverable). Measured
/// 2026-09-02 on 1.0.1-R2006.1: a `SessionStart` hook killed with the host by
/// a plain `SIGKILL` of the host pid survived as an orphan (own pgid, ppid 1)
/// for the whole of its `sleep`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcessTree {
    pub rows: Vec<ProcessRow>,
}

impl ProcessTree {
    /// Take the snapshot with one `ps -A -o pid=,ppid=,pgid=,stat=` call
    /// (unix). `None` when `ps` cannot run or complains: no tree, no kill by
    /// walk — the caller then kills the host alone and says so.
    #[cfg(unix)]
    pub fn snapshot() -> Option<ProcessTree> {
        let out = std::process::Command::new("ps")
            .args(["-A", "-o", "pid=,ppid=,pgid=,stat="])
            .stdin(std::process::Stdio::null())
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(ProcessTree::parse(&String::from_utf8_lossy(&out.stdout)))
    }

    /// See the unix definition; without `ps` there is no tree.
    #[cfg(not(unix))]
    pub fn snapshot() -> Option<ProcessTree> {
        None
    }

    /// Parse `ps` rows (`pid ppid pgid stat`; a row without a stat column is
    /// taken as live). Unparsable rows are skipped.
    pub fn parse(text: &str) -> ProcessTree {
        let rows = text
            .lines()
            .filter_map(|line| {
                let mut cols = line.split_whitespace();
                let pid = cols.next()?.parse().ok()?;
                let ppid = cols.next()?.parse().ok()?;
                let pgid = cols.next()?.parse().ok()?;
                let zombie = cols.next().map(|s| s.starts_with('Z')).unwrap_or(false);
                Some(ProcessRow {
                    pid,
                    ppid,
                    pgid,
                    zombie,
                })
            })
            .collect();
        ProcessTree { rows }
    }

    /// The row of a pid.
    pub fn row(&self, pid: u32) -> Option<&ProcessRow> {
        self.rows.iter().find(|r| r.pid == pid)
    }

    /// Present and not a zombie.
    pub fn is_live(&self, pid: u32) -> bool {
        self.row(pid).map(|r| !r.zombie).unwrap_or(false)
    }

    /// Direct children of `pid`, in `ps` order.
    pub fn children(&self, pid: u32) -> Vec<u32> {
        self.rows
            .iter()
            .filter(|r| r.ppid == pid && r.pid != pid)
            .map(|r| r.pid)
            .collect()
    }

    /// Every descendant of `root` (excluding `root`), **leaves first**:
    /// deepest generation first, so a parent is never signalled before the
    /// children it could otherwise re-spawn or reap into a hole in the list.
    pub fn descendants(&self, root: u32) -> Vec<u32> {
        let mut by_depth: Vec<(usize, u32)> = Vec::new();
        let mut seen = BTreeSet::from([root]);
        let mut frontier = vec![(0usize, root)];
        while let Some((depth, pid)) = frontier.pop() {
            for child in self.children(pid) {
                if seen.insert(child) {
                    by_depth.push((depth + 1, child));
                    frontier.push((depth + 1, child));
                }
            }
        }
        by_depth.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        by_depth.into_iter().map(|(_, pid)| pid).collect()
    }
}

/// `SIGKILL` one pid. A pid that is already gone is `Ok(())`; a pid this
/// process may not signal is the error. Never a process group: a descendant
/// in a foreign group could share it with something that is not the host's.
#[cfg(unix)]
pub fn kill_pid(pid: u32) -> std::io::Result<()> {
    use rustix::process::{kill_process, Pid, Signal};
    let raw =
        i32::try_from(pid).map_err(|_| std::io::Error::other(format!("pid {pid} out of range")))?;
    let pid = Pid::from_raw(raw)
        .ok_or_else(|| std::io::Error::other(format!("pid {raw} is not a process id")))?;
    match kill_process(pid, Signal::KILL) {
        Ok(()) => Ok(()),
        Err(e) if e == rustix::io::Errno::SRCH => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// See the unix definition; without signals nothing can be killed by pid.
#[cfg(not(unix))]
pub fn kill_pid(pid: u32) -> std::io::Result<()> {
    Err(std::io::Error::other(format!(
        "cannot signal pid {pid} on this platform"
    )))
}

/// What killing a host's process tree found and left behind
/// (`Invoker::run` on its timeout path; reported on `HostError::Timeout`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeKill {
    /// Descendants found under the host before anything was signalled,
    /// leaves first — the order they were killed in.
    pub descendants: Vec<u32>,
    /// Processes that appeared under the tree between the snapshot and the
    /// re-walk (a hook forking in the gap) and were killed on the re-walk.
    pub late: Vec<u32>,
    /// Still alive after the re-walk settled: a signal that did not take
    /// (not ours to signal, or stuck in the kernel), or a process that
    /// re-parented itself away before the snapshot and could not be seen.
    pub survivors: Vec<u32>,
    /// `ps` could not be consulted: only the host itself was killed and the
    /// other fields say nothing.
    pub tree_unavailable: bool,
}

impl TreeKill {
    /// One line for a trace or an error message.
    pub fn summary(&self) -> String {
        if self.tree_unavailable {
            return "process tree unavailable (ps failed), host killed alone".to_string();
        }
        format!(
            "{} descendant(s) killed leaves-first{}{}{}",
            self.descendants.len(),
            if self.descendants.is_empty() {
                String::new()
            } else {
                format!(" {:?}", self.descendants)
            },
            if self.late.is_empty() {
                String::new()
            } else {
                format!(", {} late {:?}", self.late.len(), self.late)
            },
            if self.survivors.is_empty() {
                String::new()
            } else {
                format!(", SURVIVORS {:?}", self.survivors)
            }
        )
    }
}

/// How long [`kill_tree`] keeps re-walking for the killed descendants to
/// leave the process table before the rest are recorded as survivors. A
/// `SIGKILL` takes effect in microseconds; under load the table lags.
pub const KILL_TREE_SETTLE: std::time::Duration = std::time::Duration::from_millis(500);

/// Kill everything a host spawned, then the host, then check. The order is:
/// snapshot the tree while the host is alive (afterwards its children are
/// reparented to pid 1 and cannot be told from anyone else's), `SIGKILL` the
/// descendants leaves first, run `kill_host` (the caller's `Child::kill` +
/// `Child::wait`, which also reaps the host), then re-walk: anything new
/// under the killed set is killed once, and whatever is still live when the
/// table has settled — up to [`KILL_TREE_SETTLE`] — is a survivor.
pub fn kill_tree(host: u32, kill_host: impl FnOnce()) -> TreeKill {
    let Some(tree) = ProcessTree::snapshot() else {
        kill_host();
        return TreeKill {
            tree_unavailable: true,
            ..TreeKill::default()
        };
    };
    let descendants = tree.descendants(host);
    for pid in &descendants {
        let _ = kill_pid(*pid);
    }
    kill_host();
    let mut victims: BTreeSet<u32> = descendants.iter().copied().collect();
    let mut late = Vec::new();
    let deadline = std::time::Instant::now() + KILL_TREE_SETTLE;
    let survivors = loop {
        let Some(tree) = ProcessTree::snapshot() else {
            // The re-walk is what would have found survivors; without it the
            // honest answer is "none seen".
            break Vec::new();
        };
        let newly: Vec<u32> = tree
            .rows
            .iter()
            .filter(|r| !r.zombie && !victims.contains(&r.pid))
            .filter(|r| r.ppid == host || victims.contains(&r.ppid))
            .map(|r| r.pid)
            .collect();
        for pid in newly {
            let _ = kill_pid(pid);
            victims.insert(pid);
            late.push(pid);
        }
        let alive: Vec<u32> = victims
            .iter()
            .copied()
            .filter(|p| tree.is_live(*p))
            .collect();
        if alive.is_empty() || std::time::Instant::now() >= deadline {
            break alive;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    TreeKill {
        descendants,
        late,
        survivors,
        tree_unavailable: false,
    }
}

/// Sweep every endpoint pair and registry entry of `dir` whose pid is dead
/// (doctor's lane). Pids are checked in one batch first so the predicate is
/// a set lookup.
pub fn sweep_dead(dir: &Path) -> Result<Swept> {
    let mut named = BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_lease = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with(ENDPOINT_PREFIX) && n.ends_with(ENDPOINT_LEASE_SUFFIX))
                .unwrap_or(false);
            if is_lease {
                if let Some(pid) = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| lease_pid(&t))
                {
                    named.insert(pid);
                }
            }
        }
    }
    if let Ok(entries) = std::fs::read_dir(dir.join(RUNTIME_SESSIONS_SUBDIR)) {
        for entry in entries.flatten() {
            if let Some(pid) = std::fs::read_to_string(entry.path())
                .ok()
                .and_then(|t| session_entry_pid(&t))
            {
                named.insert(pid);
            }
        }
    }
    let alive = pids_alive(&named);
    sweep_runtime_dir(dir, &|p| !alive.contains(&p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(dir: &Path, dead: u32, alive: u32) {
        std::fs::create_dir_all(dir.join(RUNTIME_SESSIONS_SUBDIR)).unwrap();
        std::fs::write(dir.join(RUNTIME_MUTATION_LOCK), b"").unwrap();
        std::fs::write(dir.join("ms-dead.sock"), b"").unwrap();
        std::fs::write(
            dir.join("ms-dead.sock.lease"),
            format!("{{\"schema_version\":1,\"endpoint_hint\":\"ms-dead.sock\",\"process_generation_hint\":\"pid={dead}\",\"pid\":{dead}}}"),
        )
        .unwrap();
        std::fs::write(dir.join("ms-live.sock"), b"").unwrap();
        std::fs::write(
            dir.join("ms-live.sock.lease"),
            format!("{{\"schema_version\":1,\"pid\":{alive}}}"),
        )
        .unwrap();
        std::fs::write(dir.join("ms-nopid.sock.lease"), b"{\"schema_version\":1}").unwrap();
        std::fs::write(dir.join("unrelated.txt"), b"keep").unwrap();
        std::fs::write(
            dir.join(RUNTIME_SESSIONS_SUBDIR).join("dead.json"),
            format!("{{\"schema_version\":1,\"session_id\":\"x\",\"workspace_label\":\"ws\",\"process_generation_hint\":\"pid={dead}\"}}"),
        )
        .unwrap();
        std::fs::write(
            dir.join(RUNTIME_SESSIONS_SUBDIR).join("live.json"),
            format!("{{\"schema_version\":1,\"process_generation_hint\":\"pid={alive}\"}}"),
        )
        .unwrap();
        std::fs::write(
            dir.join(RUNTIME_SESSIONS_SUBDIR).join("garbage.json"),
            b"not json",
        )
        .unwrap();
    }

    #[test]
    fn runtime_dir_shape() {
        let d = runtime_dir_for_uid(501);
        assert!(d.ends_with("tbh-501-rt/muse"), "{}", d.display());
        assert!(d.starts_with(RUNTIME_TMP_ROOT));
        assert_eq!(lease_pid("{\"pid\":42}"), Some(42));
        assert_eq!(lease_pid("{}"), None);
        assert_eq!(
            session_entry_pid("{\"process_generation_hint\":\"pid=7\"}"),
            Some(7)
        );
        assert_eq!(
            session_entry_pid("{\"process_generation_hint\":\"gen=7\"}"),
            None
        );
    }

    #[test]
    fn sweep_removes_only_the_named_dead_pid_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("muse");
        let me = std::process::id();
        fixture(&dir, 4_000_000, me);
        let swept = sweep_runtime_dir(&dir, &|p| p == 4_000_000).unwrap();
        assert_eq!(swept.endpoints.len(), 2, "{swept:?}");
        assert_eq!(swept.session_entries.len(), 1, "{swept:?}");
        assert_eq!(swept.pids, BTreeSet::from([4_000_000]));
        assert!(!dir.join("ms-dead.sock").exists());
        assert!(!dir.join("ms-dead.sock.lease").exists());
        assert!(!dir.join("sessions/dead.json").exists());
        for kept in [
            "ms-live.sock",
            "ms-live.sock.lease",
            "ms-nopid.sock.lease",
            "unrelated.txt",
            RUNTIME_MUTATION_LOCK,
            "sessions/live.json",
            "sessions/garbage.json",
        ] {
            assert!(dir.join(kept).exists(), "{kept} must survive");
        }
        // A second sweep of the same pid is a no-op; a missing dir is empty.
        assert!(sweep_runtime_dir(&dir, &|p| p == 4_000_000)
            .unwrap()
            .is_empty());
        assert!(sweep_runtime_dir(&tmp.path().join("absent"), &|_| true)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn process_tree_parses_and_walks_leaves_first() {
        // host 100 → 101 (hook shell, own pgid) → 102 (sleep); host → 103 (zombie);
        // 200 is a stranger whose ppid happens to be 1, 104 a grandchild of 101 too.
        let tree = ProcessTree::parse(
            "    1     0     1 Ss\n  100     1   100 S\n  101   100   101 S\n  102   101   101 S\n  103   100   100 Z\n  104   101   101 S\n  200     1   200 S\ngarbage row\n",
        );
        assert_eq!(tree.rows.len(), 7);
        assert!(tree.is_live(100) && tree.is_live(102));
        assert!(!tree.is_live(103), "a zombie is dead");
        assert!(!tree.is_live(999));
        assert_eq!(tree.children(100), vec![101, 103]);
        // Deepest generation first, then by pid; the root itself excluded.
        assert_eq!(tree.descendants(100), vec![102, 104, 101, 103]);
        assert_eq!(tree.descendants(101), vec![102, 104]);
        assert!(tree.descendants(200).is_empty());
        // A cycle in a corrupt table cannot loop forever.
        let looped = ProcessTree::parse("5 6 5 S\n6 5 5 S\n");
        assert_eq!(looped.descendants(5), vec![6]);
        assert_eq!(ProcessTree::parse("").descendants(1), Vec::<u32>::new());
        let summary = TreeKill {
            descendants: vec![102, 101],
            late: vec![],
            survivors: vec![101],
            tree_unavailable: false,
        }
        .summary();
        assert!(summary.contains("2 descendant(s)") && summary.contains("SURVIVORS [101]"));
        assert!(TreeKill {
            tree_unavailable: true,
            ..TreeKill::default()
        }
        .summary()
        .contains("unavailable"));
    }

    #[cfg(unix)]
    #[test]
    fn kill_tree_takes_a_grandchild_in_its_own_process_group() {
        // A shell child that puts its own child (`sleep`) into a new process
        // group with `set -m`, the way a backgrounded hook does; killing the
        // shell alone would orphan the sleep.
        let tmp = tempfile::tempdir().unwrap();
        let out = tmp.path().to_path_buf();
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "set -m; sleep 30 & echo $! > '{}/grandchild.pid'; ps -o pgid= -p $! > '{}/grandchild.pgid'; wait",
                out.display(),
                out.display()
            ))
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let host = child.id();
        let read_pid = |name: &str| -> u32 {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if let Some(p) = std::fs::read_to_string(out.join(name))
                    .ok()
                    .and_then(|t| t.trim().parse().ok())
                {
                    return p;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "{name} never appeared"
                );
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        let grandchild = read_pid("grandchild.pid");
        let grandchild_pgid = read_pid("grandchild.pgid");
        let my_pgid = ProcessTree::snapshot()
            .and_then(|t| t.row(std::process::id()).map(|r| r.pgid))
            .unwrap();
        assert_ne!(
            grandchild_pgid, my_pgid,
            "the grandchild must sit in its own group"
        );
        assert_eq!(grandchild_pgid, grandchild);
        let tree = ProcessTree::snapshot().unwrap();
        assert!(tree.descendants(host).contains(&grandchild), "{tree:?}");
        let result = kill_tree(host, || {
            let _ = child.kill();
            let _ = child.wait();
        });
        assert!(!result.tree_unavailable);
        assert!(result.descendants.contains(&grandchild), "{result:?}");
        assert_eq!(
            result.descendants.first(),
            Some(&grandchild),
            "leaves first: {result:?}"
        );
        assert!(result.survivors.is_empty(), "{result:?}");
        assert!(
            pids_alive(&BTreeSet::from([grandchild])).is_empty(),
            "the grandchild in its own process group must be dead"
        );
        // Killing a pid that is gone is not an error; an unusable pid is.
        assert!(kill_pid(grandchild).is_ok());
        assert!(kill_pid(0).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn sweep_dead_keeps_the_live_pid_and_drops_the_dead_one() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("muse");
        let me = std::process::id();
        // A pid that just exited (and was reaped) is dead and in range —
        // macOS `ps` refuses any out-of-range pid for the whole call.
        let mut gone = std::process::Command::new("true").spawn().unwrap();
        let dead = gone.id();
        gone.wait().unwrap();
        fixture(&dir, dead, me);
        let alive = pids_alive(&BTreeSet::from([me, dead]));
        assert_eq!(alive, BTreeSet::from([me]));
        // An out-of-range pid makes `ps` refuse the request: no verdict, so
        // every pid counts as alive and nothing would be swept.
        assert_eq!(
            pids_alive(&BTreeSet::from([me, 4_194_303, dead])),
            BTreeSet::from([me, 4_194_303, dead])
        );
        let swept = sweep_dead(&dir).unwrap();
        assert_eq!(swept.pids, BTreeSet::from([dead]));
        assert!(dir.join("ms-live.sock.lease").exists());
        assert!(dir.join("sessions/live.json").exists());
        assert!(!dir.join("sessions/dead.json").exists());
    }
}
