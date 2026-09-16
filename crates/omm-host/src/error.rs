//! The one error type of `omm-host`.
//!
//! Every variant names the host fact it protects; the message is what `omm`
//! prints to the user, so it says what to do next where a fix exists.

use std::path::PathBuf;
use std::time::Duration;

use thiserror::Error;

/// Errors produced by `omm-host`.
#[derive(Debug, Error)]
pub enum HostError {
    /// Neither `HOME` nor the relevant `XDG_*` variable is set. The host has
    /// exactly two candidates per root (`research/musecode/config-paths.md` §2.2);
    /// so do we.
    #[error("no home directory: neither HOME nor the XDG_* override is set (the host resolves exactly two candidates per root)")]
    NoHome,

    /// A filesystem operation failed.
    #[error("{context} {path}: {source}")]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// No Muse binary could be located.
    #[error("muse binary not found; tried: {}", tried.join(", "))]
    BinaryNotFound { tried: Vec<String> },

    /// The path resolves to the launcher shell script, which auto-updates
    /// hourly and is never exec'd by omm (`research/musecode/cli-surface.md` §2).
    #[error("{path} is the muse launcher script, not the binary; omm only runs muse-bin-<version> (the launcher auto-updates)")]
    LauncherScript { path: PathBuf },

    /// The located file has no execute bit; spawning it would fail later with
    /// `Permission denied`, so the search refuses it up front.
    #[error("{path} is not executable (no x bit); point OMM_MUSE_BIN at the muse-bin-<version> binary or chmod +x it")]
    NotExecutable { path: PathBuf },

    /// The argv failed the R20 allowlist.
    #[error("argv rejected: {0}")]
    Argv(String),

    /// The host process could not be spawned.
    #[error("failed to spawn {program}: {source}")]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// The host printed something that is not the JSON we asked for.
    #[error("{context}: invalid JSON from host: {source}\nstdout: {stdout}\nstderr: {stderr}")]
    Json {
        context: String,
        #[source]
        source: serde_json::Error,
        stdout: String,
        stderr: String,
    },

    /// A host command exited non-zero (or died of a signal) where we required success.
    #[error("`muse {argv}` {}: {stderr}", exit_status_word(.code))]
    Command {
        argv: String,
        code: Option<i32>,
        stderr: String,
    },

    /// The host did not finish within the budget of [`crate::invoke::Invoker::run`]
    /// and was killed. A host stuck on I/O never exits on its own
    /// (`docs/host-data/muse-cli.json` `hidden_argv_modes`: the PTY-gate mode
    /// hangs until killed, exit 125), so every captured invocation carries a
    /// wall-clock budget. Everything the host had spawned (hook shells and
    /// their children, each possibly in its own process group) is killed
    /// leaves-first before the host ([`crate::residue::kill_tree`]); a
    /// descendant still alive after the re-walk is listed in `survivors`
    /// (empty on a clean kill). The registry entries the killed process left
    /// under the host's runtime dir are swept by the caller ([`crate::residue`]).
    #[error("`muse {argv}` did not finish within {} s and was killed (pid {pid}){}; partial stdout: {stdout}; partial stderr: {stderr}", .timeout.as_secs_f64(), survivors_note(.survivors))]
    Timeout {
        argv: String,
        timeout: Duration,
        pid: u32,
        /// Descendants of the host that outlived the kill (pids), or empty.
        survivors: Vec<u32>,
        stdout: String,
        stderr: String,
    },

    /// The host answered with a structured `{"error":{"code","message"}}`.
    #[error("host error {code}: {message}")]
    HostReported { code: String, message: String },

    /// Host output did not match the shape the research recorded.
    #[error("could not parse {what}: {detail}")]
    Parse { what: &'static str, detail: String },

    /// R9: only the typed keys of `docs/host-data/settings-keys.json` may be written.
    #[error("settings key {0:?} is not one of the typed settings.json keys (docs/host-data/settings-keys.json); Muse would destroy it on its next rewrite, refusing to write it")]
    UnknownSettingsKey(String),

    /// D3: both spellings present. Every settings-mutating Muse command exits 1
    /// in this state (`research/experiments/settings-plugins.md` verification C5).
    #[error("settings.json declares both `mcpServers` and `mcp_servers`; every settings-mutating muse command exits 1 in this state and MCP is silently disabled — resolve the collision first (`omm settings fix-mcp-collision`)")]
    McpCollision,

    /// The candidate settings document was rejected by one of the validators.
    #[error("settings candidate rejected at {stage}: {detail}")]
    SettingsRejected { stage: &'static str, detail: String },

    /// The file changed between load and commit; refusing to clobber.
    #[error("{path} changed on disk since it was loaded (expected sha256 {expected}, found {found}); reload and retry")]
    Stale {
        path: PathBuf,
        expected: String,
        found: String,
    },

    /// The verified backup does not match the original.
    #[error("backup verification failed for {path}: {detail}")]
    BackupMismatch { path: PathBuf, detail: String },

    /// `trust.json` is malformed or unsupported.
    #[error("trust store {path}: {detail}")]
    Trust { path: PathBuf, detail: String },

    /// A compiled-in data list failed to parse (a build-time bug, never a user error).
    #[error("bundled data file {name} failed to parse: {detail}")]
    Data { name: &'static str, detail: String },

    /// The path is a symlink whose target does not exist. Writing through it
    /// would either replace the user's link with a regular file or create the
    /// target somewhere the user did not point us; neither is a targeted merge.
    #[error("{path} is a dangling symlink to {target}; create the target or remove the link before omm writes here")]
    DanglingSymlink { path: PathBuf, target: PathBuf },

    /// A commit needs omm's own root for its lock (`$OMM/locks/`) and its
    /// verified pre-write backup (`$OMM/snapshots/`), and none is known
    /// (ARCHITECTURE.md §2: nothing omm writes lands beside the host's files).
    /// Load the document via `for_roots` or pass `CommitOptions::omm_root`.
    #[error("no omm root known for the lock and pre-write backup of {path}; load via for_roots() or set CommitOptions.omm_root (ARCHITECTURE §2 keeps omm's files under $OMM/, never beside the host's)")]
    NoOmmRoot { path: PathBuf },

    /// The advisory lock another writer holds was not released in time.
    #[error("{path} is held by another writer (waited {}); another omm — or a hung one — is committing to the host's config; retry, or remove the lock file once no omm process is running", format_wait(.wait))]
    Locked { path: PathBuf, wait: Duration },

    /// A probe could not observe what it needed.
    #[error("{0}")]
    Probe(String),
}

/// `Result` alias used throughout the crate.
pub type Result<T> = std::result::Result<T, HostError>;

/// `exited 1` for a code, `killed by signal` for none — a host killed by a
/// signal has no exit code and used to render as `exited None`.
fn exit_status_word(code: &Option<i32>) -> String {
    match code {
        Some(c) => format!("exited {c}"),
        None => "killed by signal".to_string(),
    }
}

fn format_wait(wait: &Duration) -> String {
    format!("{:.1} s", wait.as_secs_f64())
}

/// `; N descendant process(es) survived the kill: [..]` or nothing.
fn survivors_note(survivors: &[u32]) -> String {
    if survivors.is_empty() {
        String::new()
    } else {
        format!(
            "; {} descendant process(es) survived the kill: {survivors:?}",
            survivors.len()
        )
    }
}

impl HostError {
    pub(crate) fn io(
        context: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        HostError::Io {
            context,
            path: path.into(),
            source,
        }
    }
}
