//! Where the host keeps things, and where omm keeps its own things.
//!
//! Sources: `research/musecode/config-paths.md` §2 (the resolver has **exactly
//! two candidates per root, XDG first, HOME second** — the failure literals are
//! `no usable config directory from XDG_CONFIG_HOME or HOME` and the data-root
//! twin; there is no `MUSE_HOME` / `MUSE_CONFIG_DIR`), its verification C7
//! (an empty XDG value falls back to HOME; a relative XDG value is honoured
//! relative to the cwd), `docs/host-reality.md` "Paths", and
//! `research/experiments/loose-ends.md` §2 (the `personal_project` memory root).
//!
//! Nothing here touches the filesystem except [`Roots::memory_personal_project_dir`],
//! which must canonicalize the workspace because the host does.
//!
//! Two different "homes": the XDG fallbacks use `$HOME` exactly as the host's
//! resolver does, while the residue the host writes via `getpwuid`
//! (`~/Library/Application Support/Muse/session-name-authority/`,
//! `research/experiments/loose-ends.md` §0 Trap B) is rooted at the *account*
//! home — the passwd entry of the process uid ([`account_home`]), read
//! regardless of `$HOME`: `std::env::home_dir()` follows `$HOME` first, so
//! under a redirected `HOME` it named `<redirected>/Library/…` while the host
//! wrote the real account home (Gate 0 residual, 2026-09-02). A third residue
//! root is the per-uid runtime dir (`crate::residue`), keyed by the process
//! uid and independent of every environment variable.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error::{HostError, Result};
use crate::fsx;

/// The account home of the process uid from its passwd entry — what
/// `getpwuid(3)` returns to the host — with `$HOME` (`std::env::home_dir()`)
/// only as the fallback when the entry cannot be read. Independent of a
/// redirected `HOME`, which is what every sandbox does. Resolved once per
/// process; see [`account_home_from_passwd`] for the lookup.
pub fn account_home() -> Option<PathBuf> {
    static ACCOUNT_HOME: OnceLock<Option<PathBuf>> = OnceLock::new();
    ACCOUNT_HOME
        .get_or_init(|| account_home_from_passwd().or_else(std::env::home_dir))
        .clone()
}

/// The passwd home of the process uid, uncached and without any `$HOME`
/// fallback. This crate forbids `unsafe`, so `getpwuid_r` is not called
/// directly; the system's own tools answer instead:
///
/// * macOS: `id -un` names the account of the real uid (`getpwuid`, never
///   `$USER`), then `dscl /Search -read /Users/<name> NFSHomeDirectory` reads
///   Directory Services, where local and directory accounts live (they are
///   not in `/etc/passwd`);
/// * other unixes: `getent passwd <uid>`, field 6.
///
/// `None` when a tool is missing, fails, or names a relative path.
#[cfg(target_os = "macos")]
pub fn account_home_from_passwd() -> Option<PathBuf> {
    let name = capture("id", &["-un"])?;
    let name = name.trim();
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let record = format!("/Users/{name}");
    let out = capture("dscl", &["/Search", "-read", &record, "NFSHomeDirectory"])?;
    out.lines()
        .find_map(|l| l.strip_prefix("NFSHomeDirectory:"))
        .map(|p| PathBuf::from(p.trim()))
        .filter(|p| p.is_absolute())
}

/// See the macOS definition.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn account_home_from_passwd() -> Option<PathBuf> {
    let uid = fsx::current_uid()?;
    let out = capture("getent", &["passwd", &uid.to_string()])?;
    out.lines()
        .next()?
        .split(':')
        .nth(5)
        .map(|p| PathBuf::from(p.trim()))
        .filter(|p| p.is_absolute())
}

/// See the macOS definition; no passwd database here.
#[cfg(not(unix))]
pub fn account_home_from_passwd() -> Option<PathBuf> {
    None
}

/// Run a system tool and return its stdout on exit 0 (stdin closed, stderr
/// dropped). Neither tool reads `HOME`; the environment is left as is.
#[cfg(unix)]
fn capture(program: &str, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Which of the two candidates a root came from
/// (`event="path.resolved" kind="config_root" source="xdg|home"`, config-paths.md §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootSource {
    /// `$XDG_CONFIG_HOME` / `$XDG_DATA_HOME` was set and non-empty.
    Xdg,
    /// Fell back to `$HOME/.config` / `$HOME/.local/share`.
    Home,
}

/// The environment inputs the resolver reads, captured so tests never mutate
/// the process environment.
#[derive(Clone, Debug, Default)]
pub struct EnvView {
    pub home: Option<OsString>,
    pub xdg_config_home: Option<OsString>,
    pub xdg_data_home: Option<OsString>,
    /// Used only to absolutise a relative `XDG_*` value (config-paths.md C7).
    pub cwd: Option<PathBuf>,
    /// The account home from the passwd entry ([`account_home`]; `$HOME`
    /// only when the entry cannot be read) — the root of the host's
    /// `getpwuid` residue, unmoved by a redirected `HOME`. Never used for the
    /// XDG fallbacks, which follow `$HOME` alone like the host.
    pub account_home: Option<PathBuf>,
    /// The process uid — the key of the host's runtime dir
    /// (`/private/tmp/tbh-<uid>-rt/muse`, `crate::residue`).
    pub uid: Option<u32>,
    /// `std::env::temp_dir()` — where the host puts `muse-shell-sandbox-<uuid>/`.
    pub tmpdir: Option<PathBuf>,
}

impl EnvView {
    /// Snapshot the real process environment, including the account home
    /// from the passwd entry ([`account_home`]: two tool spawns, ~20 ms once
    /// per process, then cached). The default for every command that may
    /// name the host's residue (doctor, uninstall preview).
    pub fn from_process() -> Self {
        EnvView {
            account_home: account_home(),
            ..EnvView::from_process_fast()
        }
    }

    /// The same snapshot without the passwd lookup — `account_home` is
    /// `$HOME` as `std::env::home_dir()` sees it — for the sub-5 ms hook
    /// dispatcher (R16), which never names residue and cannot afford a
    /// spawn. Under a redirected `HOME` the residue root of these roots is
    /// wrong, which is exactly why [`EnvView::from_process`] is the default.
    pub fn from_process_fast() -> Self {
        EnvView {
            home: home_from_process(),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
            cwd: std::env::current_dir().ok(),
            account_home: std::env::home_dir(),
            uid: fsx::current_uid(),
            tmpdir: Some(std::env::temp_dir()),
        }
    }
}

/// `$OMM/snapshots` (ARCHITECTURE.md §2, §4).
pub const OMM_SNAPSHOTS_DIR: &str = "snapshots";
/// `$OMM/locks` — advisory locks for the host's config files (never beside them).
pub const OMM_LOCKS_DIR: &str = "locks";

#[cfg(windows)]
fn home_from_process() -> Option<OsString> {
    // The Windows build's failure literal is `no usable config directory from
    // XDG_CONFIG_HOME, HOME, or USERPROFILE` (loose-ends.md §5.3): USERPROFILE is
    // the Windows spelling of HOME, not a third candidate.
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
}

#[cfg(not(windows))]
fn home_from_process() -> Option<OsString> {
    std::env::var_os("HOME")
}

fn non_empty(v: &Option<OsString>) -> Option<&OsString> {
    v.as_ref().filter(|s| !s.is_empty())
}

/// The two host roots plus omm's own root, resolved once.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Roots {
    /// The account home the host's `getpwuid` residue is rooted at
    /// (`EnvView::account_home`, else `$HOME`). `None` only when neither is
    /// known, in which case the residue path cannot be named.
    pub home: Option<PathBuf>,
    /// `$XDG_CONFIG_HOME` else `$HOME/.config`.
    pub config_home: PathBuf,
    /// `$XDG_DATA_HOME` else `$HOME/.local/share`.
    pub data_home: PathBuf,
    pub config_source: RootSource,
    pub data_source: RootSource,
    /// The process uid (`EnvView::uid`), `None` when unknown.
    pub uid: Option<u32>,
    /// `EnvView::tmpdir`.
    pub tmpdir: Option<PathBuf>,
}

impl Roots {
    /// Resolve from the real process environment ([`EnvView::from_process`]:
    /// the passwd lookup for the residue root runs once, ~20 ms, cached).
    pub fn from_env() -> Result<Roots> {
        Roots::resolve(&EnvView::from_process())
    }

    /// Resolve from the real process environment without the passwd lookup
    /// ([`EnvView::from_process_fast`]) — for `omm hook`, whose whole budget
    /// is 5 ms (R16) and which never names the host's residue.
    pub fn from_env_fast() -> Result<Roots> {
        Roots::resolve(&EnvView::from_process_fast())
    }

    /// Resolve from an explicit environment view (pure).
    pub fn resolve(env: &EnvView) -> Result<Roots> {
        let home = non_empty(&env.home).map(PathBuf::from);
        let absolutise = |p: PathBuf| -> PathBuf {
            if p.is_absolute() {
                p
            } else if let Some(cwd) = &env.cwd {
                cwd.join(p)
            } else {
                p
            }
        };
        let (config_home, config_source) = match non_empty(&env.xdg_config_home) {
            Some(x) => (absolutise(PathBuf::from(x)), RootSource::Xdg),
            None => (
                home.clone().ok_or(HostError::NoHome)?.join(".config"),
                RootSource::Home,
            ),
        };
        let (data_home, data_source) = match non_empty(&env.xdg_data_home) {
            Some(x) => (absolutise(PathBuf::from(x)), RootSource::Xdg),
            None => (
                home.clone()
                    .ok_or(HostError::NoHome)?
                    .join(".local")
                    .join("share"),
                RootSource::Home,
            ),
        };
        // HOME may legitimately be unset when both XDG roots are given; the
        // residue root is the account home then (getpwuid on unix), never a
        // guess derived from the XDG roots.
        let home = env.account_home.clone().or(home);
        Ok(Roots {
            home,
            config_home,
            data_home,
            config_source,
            data_source,
            uid: env.uid,
            tmpdir: env.tmpdir.clone(),
        })
    }

    // ---- host config root ------------------------------------------------

    /// `$XDG_CONFIG_HOME/muse` else `~/.config/muse` (config-paths.md §3).
    pub fn muse_config(&self) -> PathBuf {
        self.config_home.join("muse")
    }
    /// `settings.json` — the 29-key typed user settings (settings-keys.json).
    pub fn settings_file(&self) -> PathBuf {
        self.muse_config().join("settings.json")
    }
    /// `trust.json` — the workspace trust store (security-permissions.md §5.2).
    pub fn trust_file(&self) -> PathBuf {
        self.muse_config().join("trust.json")
    }
    /// `auth.json` — never read or written by omm; listed so uninstall can name it.
    pub fn auth_file(&self) -> PathBuf {
        self.muse_config().join("auth.json")
    }
    /// `$CONFIG_DIR/AGENTS.md` — Muse's own personal rules file; exactly one of
    /// the four-rung ladder loads and this rung is first (config-paths.md M1).
    pub fn personal_rules_file(&self) -> PathBuf {
        self.muse_config().join("AGENTS.md")
    }
    /// `$CONFIG_DIR/skills/` — the managed personal skill store (config-paths.md §3).
    pub fn personal_skills_dir(&self) -> PathBuf {
        self.muse_config().join("skills")
    }
    /// `$CONFIG_DIR/themes/*.tmTheme` — case-insensitive extension; the data
    /// dir is NOT scanned (host-reality.md "Paths": themes).
    pub fn themes_dir(&self) -> PathBuf {
        self.muse_config().join("themes")
    }
    /// `$CONFIG_DIR/workflows/<name>.js` — user-scope named workflows (cli-surface.md §3.7).
    pub fn workflows_dir(&self) -> PathBuf {
        self.muse_config().join("workflows")
    }

    // ---- host data root --------------------------------------------------

    /// `$XDG_DATA_HOME/muse` else `~/.local/share/muse` (config-paths.md §4).
    pub fn muse_data(&self) -> PathBuf {
        self.data_home.join("muse")
    }
    /// `plugins/` — owned by `muse plugins install`; omm only records registrations.
    pub fn plugins_dir(&self) -> PathBuf {
        self.muse_data().join("plugins")
    }
    /// `plugins/installed.json` — what `plugins install/enable/disable` write
    /// (settings-plugins.md §5).
    pub fn plugins_installed_file(&self) -> PathBuf {
        self.plugins_dir().join("installed.json")
    }
    /// `sessions/YYYY/MM/DD/<uuid>/session.jsonl` lives under here.
    pub fn sessions_dir(&self) -> PathBuf {
        self.muse_data().join("sessions")
    }
    /// `local-tracing/bootstrap/cli-<uuid>.log` — where `gate.resolve` lines land.
    pub fn bootstrap_trace_dir(&self) -> PathBuf {
        self.muse_data().join("local-tracing").join("bootstrap")
    }
    /// `runtime/` — the fallback session-registry root (`$XDG_DATA_HOME/muse/runtime/muse`).
    /// On macOS the host uses the per-uid `/private/tmp` dir instead (residue.rs),
    /// but on Linux the registry (sessions, the mutation lock) lands here, inside
    /// our bases — so uninstall names it and never touches it on every OS.
    pub fn runtime_fallback_dir(&self) -> PathBuf {
        self.muse_data().join("runtime")
    }
    /// `skills/bundled/muse-core/` — the 15 built-ins, materialised by skill loading.
    pub fn bundled_skills_dir(&self) -> PathBuf {
        self.muse_data()
            .join("skills")
            .join("bundled")
            .join("muse-core")
    }
    /// `memory/personal/` (sessions-memory-rules.md §3.3).
    pub fn memory_personal_dir(&self) -> PathBuf {
        self.muse_data().join("memory").join("personal")
    }
    /// `memory/projects/` — parent of every `personal_project` root.
    pub fn memory_projects_dir(&self) -> PathBuf {
        self.muse_data().join("memory").join("projects")
    }
    /// `memory/projects/<slug96>-<fnv1a64hex>/` for a workspace, canonicalised
    /// exactly as the host does (loose-ends.md §2.2). Trust-gated at runtime.
    pub fn memory_personal_project_dir(&self, workspace: &Path) -> Result<PathBuf> {
        let canonical = fsx::canonicalize(workspace)?;
        Ok(self
            .memory_projects_dir()
            .join(personal_project_dir_name(&canonical)))
    }

    // ---- omm's own root --------------------------------------------------

    /// `$XDG_CONFIG_HOME/omm` else `~/.config/omm` — everything omm owns
    /// (ARCHITECTURE.md §2).
    pub fn omm_root(&self) -> PathBuf {
        self.config_home.join("omm")
    }
    /// `$OMM/snapshots/` — rolling pre-update snapshots (ledger) and the
    /// verified pre-write backups of `settings.json` / `trust.json`, as
    /// `<timestamp>/<base>/<path>`; newest five kept (ARCHITECTURE.md §2, §4).
    pub fn snapshots_dir(&self) -> PathBuf {
        self.omm_root().join(OMM_SNAPSHOTS_DIR)
    }
    /// `$OMM/locks/` — the advisory locks every writer of the host's config
    /// files holds from its stale check through its rename
    /// (`settings::MUSE_CONFIG_LOCK`).
    pub fn locks_dir(&self) -> PathBuf {
        self.omm_root().join(OMM_LOCKS_DIR)
    }

    // ---- residue ---------------------------------------------------------

    /// `~/Library/Application Support/Muse/session-name-authority/` — written
    /// by the host via `getpwuid` on every run, i.e. the *real* home even when
    /// `HOME` is redirected (loose-ends.md §0 Trap B). Named in the uninstall
    /// preview as deliberately kept residue. Rooted at the account home from
    /// the passwd entry ([`account_home`]), so a redirected `HOME` — every
    /// sandbox — still names the directory the host actually writes; `$HOME`
    /// is used only when no passwd entry can be read, and with no home known
    /// at all the path is `None`.
    pub fn session_name_authority_residue(&self) -> Option<PathBuf> {
        Some(
            self.home
                .as_ref()?
                .join("Library")
                .join("Application Support")
                .join("Muse")
                .join("session-name-authority"),
        )
    }
    /// `/private/tmp/tbh-<uid>-rt/muse/` — the session-messaging runtime dir
    /// and registry every session writes to, sandboxed probes included
    /// (`crate::residue`); keyed by the uid, not by any environment variable.
    /// `None` when the uid is unknown.
    pub fn runtime_dir(&self) -> Option<PathBuf> {
        self.uid.map(crate::residue::runtime_dir_for_uid)
    }
    /// The directory under which every session creates
    /// `muse-shell-sandbox-<uuid>/` (`residue::SHELL_SANDBOX_PREFIX`), i.e. the
    /// process temp dir; the uuid cannot be attributed to a pid.
    pub fn shell_sandbox_parent(&self) -> Option<PathBuf> {
        self.tmpdir.clone()
    }
}

/// FNV-1a 64-bit over raw bytes — the hash the host uses for the
/// `personal_project` directory suffix (loose-ends.md §2.2: identified by brute
/// force as the only candidate matching three independent samples).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// The slug half of a `personal_project` directory name: drop leading `/`,
/// map every char outside `[A-Za-z0-9]` to `-` (case preserved, runs not
/// collapsed), truncate to 96 chars (loose-ends.md §2.2 steps 2–3).
pub fn personal_project_slug(canonical: &str) -> String {
    canonical
        .trim_start_matches('/')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(96)
        .collect()
}

/// `<slug96>-<fnv1a64 of the full canonical path, %016x>` (loose-ends.md §2.2).
/// The caller must pass the canonicalised workspace root.
pub fn personal_project_dir_name(canonical: &Path) -> String {
    let text = canonical.to_string_lossy();
    let hash = fnv1a64(path_bytes(canonical));
    format!("{}-{hash:016x}", personal_project_slug(&text))
}

#[cfg(unix)]
fn path_bytes(p: &Path) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    p.as_os_str().as_bytes()
}

#[cfg(not(unix))]
fn path_bytes(p: &Path) -> &[u8] {
    // Non-UTF-8 paths cannot be hashed faithfully here; the lossy form is the
    // best available approximation.
    p.as_os_str().to_str().map(str::as_bytes).unwrap_or(b"")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(home: Option<&str>, xc: Option<&str>, xd: Option<&str>) -> EnvView {
        EnvView {
            home: home.map(OsString::from),
            xdg_config_home: xc.map(OsString::from),
            xdg_data_home: xd.map(OsString::from),
            cwd: Some(PathBuf::from("/cwd")),
            account_home: None,
            uid: None,
            tmpdir: None,
        }
    }

    #[test]
    fn runtime_dir_follows_the_uid_not_the_environment() {
        let mut e = env(Some("/h"), Some("/xc"), Some("/xd"));
        let r = Roots::resolve(&e).unwrap();
        assert_eq!(r.runtime_dir(), None, "unknown uid: no path to name");
        assert_eq!(r.shell_sandbox_parent(), None);
        e.uid = Some(501);
        e.tmpdir = Some(PathBuf::from("/var/folders/x/T"));
        let r = Roots::resolve(&e).unwrap();
        assert!(r.runtime_dir().unwrap().ends_with("tbh-501-rt/muse"));
        assert_eq!(
            r.shell_sandbox_parent(),
            Some(PathBuf::from("/var/folders/x/T"))
        );
        assert_eq!(r.locks_dir(), PathBuf::from("/xc/omm/locks"));
        let live = Roots::from_env().unwrap();
        assert!(
            live.runtime_dir().is_some(),
            "the process uid is always known on unix"
        );
    }

    #[test]
    fn residue_root_is_the_account_home_not_an_xdg_guess() {
        // HOME unset, both XDG roots set: the host still writes its residue
        // under the passwd home; naming `/Library/…` (the config home's parent)
        // in the uninstall preview would be wrong.
        let mut e = env(None, Some("/xc"), Some("/xd"));
        let r = Roots::resolve(&e).unwrap();
        assert_eq!(r.home, None);
        assert_eq!(r.session_name_authority_residue(), None);
        e.account_home = Some(PathBuf::from("/Users/real"));
        let r = Roots::resolve(&e).unwrap();
        assert_eq!(r.muse_config(), PathBuf::from("/xc/muse"));
        assert_eq!(
            r.session_name_authority_residue(),
            Some(PathBuf::from(
                "/Users/real/Library/Application Support/Muse/session-name-authority"
            ))
        );
        // The account home never feeds the XDG fallbacks.
        let e = EnvView {
            account_home: Some(PathBuf::from("/Users/real")),
            ..env(Some("/h"), None, None)
        };
        let r = Roots::resolve(&e).unwrap();
        assert_eq!(r.muse_config(), PathBuf::from("/h/.config/muse"));
        assert_eq!(r.home.as_deref(), Some(Path::new("/Users/real")));
        assert_eq!(r.snapshots_dir(), PathBuf::from("/h/.config/omm/snapshots"));
        // With only HOME and no passwd answer, the residue follows HOME.
        let r = Roots::resolve(&env(Some("/h"), None, None)).unwrap();
        assert_eq!(r.home.as_deref(), Some(Path::new("/h")));
        assert!(EnvView::from_process().account_home.is_some());
    }

    #[cfg(unix)]
    #[test]
    fn account_home_comes_from_the_passwd_entry() {
        let passwd = account_home_from_passwd().expect("passwd home of the test uid");
        assert!(passwd.is_absolute(), "{}", passwd.display());
        assert!(passwd.is_dir(), "{}", passwd.display());
        assert_eq!(account_home().as_deref(), Some(passwd.as_path()));
        assert_eq!(
            EnvView::from_process().account_home.as_deref(),
            Some(passwd.as_path())
        );
        // Cached: the same answer, no second lookup needed.
        assert_eq!(account_home(), account_home());
    }

    /// Inner half of `account_home_ignores_a_redirected_home`: runs only when
    /// re-executed by it with `HOME` pointed elsewhere.
    #[cfg(unix)]
    #[test]
    fn account_home_ignores_a_redirected_home_inner() {
        let Some(expected) = std::env::var_os("OMM_TEST_EXPECTED_ACCOUNT_HOME") else {
            return;
        };
        let redirected = std::env::var_os("HOME").expect("HOME is redirected here");
        assert_ne!(redirected, expected, "the outer test must redirect HOME");
        let home = account_home().expect("account home");
        assert_eq!(home, PathBuf::from(&expected));
        assert_ne!(home, PathBuf::from(&redirected));
        assert_eq!(
            std::env::home_dir().as_deref(),
            Some(Path::new(&redirected)),
            "std::env::home_dir() follows $HOME — the very thing the passwd lookup avoids"
        );
        let roots = Roots::from_env().unwrap();
        assert_eq!(roots.home.as_deref(), Some(Path::new(&expected)));
        assert!(roots
            .session_name_authority_residue()
            .unwrap()
            .starts_with(&expected));
        // The XDG fallbacks still follow the redirected HOME, like the host.
        assert!(roots.muse_config().starts_with(&redirected));
        // The spawn-free variant for the hook dispatcher follows HOME for the
        // residue root too — the documented trade-off, same XDG roots.
        let fast = Roots::from_env_fast().unwrap();
        assert_eq!(fast.home.as_deref(), Some(Path::new(&redirected)));
        assert_eq!(fast.muse_config(), roots.muse_config());
        assert_eq!(fast.omm_root(), roots.omm_root());
        // A sandbox names the same account home as its residue root.
        let tmp = tempfile::tempdir().unwrap();
        let sb = crate::invoke::Sandbox::create(tmp.path()).unwrap();
        let sb_roots = sb.roots().unwrap();
        assert_eq!(sb_roots.home.as_deref(), Some(Path::new(&expected)));
        assert!(sb_roots.muse_config().starts_with(&sb.config_home));
    }

    #[cfg(unix)]
    #[test]
    fn account_home_ignores_a_redirected_home() {
        // Gate 0 residual: with HOME redirected, the residue path named
        // `<redirected>/Library/…` while the host wrote the real account home
        // (getpwuid). Re-run this binary's inner test under a redirected HOME.
        let expected = account_home().expect("account home");
        let redirected = tempfile::tempdir().unwrap();
        let exe = std::env::current_exe().unwrap();
        let out = std::process::Command::new(&exe)
            .args([
                "paths::tests::account_home_ignores_a_redirected_home_inner",
                "--exact",
                "--nocapture",
            ])
            .env("HOME", redirected.path())
            .env("OMM_TEST_EXPECTED_ACCOUNT_HOME", &expected)
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_DATA_HOME")
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "inner test failed under HOME={}:\n{}\n{}",
            redirected.path().display(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.contains("1 passed"), "{stdout}");
    }

    #[test]
    fn two_candidates_xdg_first() {
        let r = Roots::resolve(&env(Some("/h"), Some("/xc"), Some("/xd"))).unwrap();
        assert_eq!(r.muse_config(), PathBuf::from("/xc/muse"));
        assert_eq!(r.muse_data(), PathBuf::from("/xd/muse"));
        assert_eq!(r.omm_root(), PathBuf::from("/xc/omm"));
        assert_eq!(r.config_source, RootSource::Xdg);
        assert_eq!(r.data_source, RootSource::Xdg);
    }

    #[test]
    fn home_fallback() {
        let r = Roots::resolve(&env(Some("/h"), None, None)).unwrap();
        assert_eq!(
            r.settings_file(),
            PathBuf::from("/h/.config/muse/settings.json")
        );
        assert_eq!(
            r.sessions_dir(),
            PathBuf::from("/h/.local/share/muse/sessions")
        );
        assert_eq!(r.config_source, RootSource::Home);
    }

    #[test]
    fn empty_xdg_is_unset_and_relative_xdg_is_cwd_relative() {
        let r = Roots::resolve(&env(Some("/h"), Some(""), Some("rel"))).unwrap();
        assert_eq!(r.muse_config(), PathBuf::from("/h/.config/muse"));
        assert_eq!(r.muse_data(), PathBuf::from("/cwd/rel/muse"));
    }

    #[test]
    fn no_home_no_xdg_is_an_error() {
        assert!(matches!(
            Roots::resolve(&env(None, None, None)),
            Err(HostError::NoHome)
        ));
        // Both XDG roots set: HOME is not required.
        assert!(Roots::resolve(&env(None, Some("/xc"), Some("/xd"))).is_ok());
    }

    #[test]
    fn personal_project_worked_examples() {
        // loose-ends.md §2.2 table.
        let cases = [
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
        ];
        for (path, want) in cases {
            assert_eq!(personal_project_dir_name(Path::new(path)), want, "{path}");
        }
    }

    #[test]
    fn personal_project_truncates_slug_to_96_and_hash_disambiguates() {
        // loose-ends.md §2.2: two workspaces sharing a 96-char slug prefix are
        // told apart by the hash alone. Expected values come from the report's
        // reference implementation (§2.2b), which predicted a live directory
        // name exactly; its prose table lists these two rows the other way round.
        let a84 = format!("/private/tmp/{}", "a".repeat(84));
        let a85 = format!("/private/tmp/{}", "a".repeat(85));
        let n84 = personal_project_dir_name(Path::new(&a84));
        let n85 = personal_project_dir_name(Path::new(&a85));
        assert_eq!(&n84[..96], &n85[..96], "slugs collide after truncation");
        assert_eq!(n84.len(), 96 + 1 + 16);
        assert_eq!(n85.len(), 96 + 1 + 16);
        assert!(n84.ends_with("-a41238a33aafa120"), "{n84}");
        assert!(n85.ends_with("-7a977e5cb86f0173"), "{n85}");
        assert_ne!(n84, n85);
    }

    #[test]
    fn personal_project_non_ascii_maps_each_char_to_a_dash() {
        // Case preserved, runs not collapsed, one dash per non-alphanumeric char.
        let n = personal_project_dir_name(Path::new("/tmp/Ünï cøde//x"));
        assert!(n.starts_with("tmp--n--c-de--x-"), "{n}");
    }

    #[test]
    fn fnv1a64_reference_vectors() {
        assert_eq!(fnv1a64(b""), 0xcbf29ce484222325);
        assert_eq!(fnv1a64(b"a"), 0xaf63dc4c8601ec8c);
    }
}
