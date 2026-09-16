//! Find the Muse *binary* — never the launcher.
//!
//! Order (ARCHITECTURE.md §3): `$OMM_MUSE_BIN` → the launcher's install dir
//! (`$MUSE_INSTALL_DIR` else `~/.local/bin`, `install.sh:5`) where
//! `.muse-version` names the active `muse-bin-<version>` (`muse-launcher.sh`
//! `active_binary()`) → `muse` on `PATH`, resolved through the same
//! `.muse-version` rule when it is the launcher script.
//!
//! The launcher (`muse-launcher.sh`, 1138 lines of bash) auto-updates hourly
//! and `exec`s the binary; it adds no CLI surface (`research/musecode/cli-surface.md`
//! §2). A `.pkg` install puts the raw binary at `/usr/local/bin/muse` with no
//! launcher at all (`research/experiments/loose-ends.md` §3.3), which the PATH
//! rung handles because a non-script file is accepted as-is.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::{HostError, Result};
use crate::fsx;

/// Which rung of the search found the binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocateSource {
    /// `$OMM_MUSE_BIN`.
    EnvOverride,
    /// `<install dir>/.muse-version` → `muse-bin-<version>`.
    LauncherDir(PathBuf),
    /// A `muse` on `PATH` (a real binary, or a launcher resolved via its dir).
    PathSearch(PathBuf),
}

/// A located binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Located {
    /// Canonical path to `muse-bin-<version>` (or the raw binary).
    pub binary: PathBuf,
    pub source: LocateSource,
    /// The `.muse-version` contents when a launcher dir was consulted.
    /// Informational only — never gate on it (R15).
    pub version_hint: Option<String>,
}

/// Environment inputs of the search, captured for testability.
#[derive(Clone, Debug, Default)]
pub struct LocateEnv {
    pub omm_muse_bin: Option<OsString>,
    pub muse_install_dir: Option<OsString>,
    pub home: Option<OsString>,
    pub path: Option<OsString>,
}

impl LocateEnv {
    pub fn from_process() -> Self {
        LocateEnv {
            omm_muse_bin: std::env::var_os("OMM_MUSE_BIN"),
            muse_install_dir: std::env::var_os("MUSE_INSTALL_DIR"),
            home: std::env::var_os("HOME"),
            path: std::env::var_os("PATH"),
        }
    }
}

/// Locate using the real process environment.
pub fn locate() -> Result<Located> {
    locate_with(&LocateEnv::from_process())
}

/// Locate using an explicit environment.
pub fn locate_with(env: &LocateEnv) -> Result<Located> {
    let mut tried: Vec<String> = Vec::new();

    // 1. Explicit override. A missing file is `BinaryNotFound` with the
    // `tried` list (not a bare canonicalize error); a file without the x bit
    // is refused here rather than at spawn time (`Permission denied`).
    if let Some(p) = env.omm_muse_bin.as_ref().filter(|s| !s.is_empty()) {
        let path = PathBuf::from(p);
        tried.push(format!("OMM_MUSE_BIN={}", path.display()));
        let canonical = match fsx::canonicalize(&path) {
            Ok(c) => c,
            Err(HostError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(HostError::BinaryNotFound { tried });
            }
            Err(e) => return Err(e),
        };
        if !canonical.is_file() {
            return Err(HostError::BinaryNotFound { tried });
        }
        if fsx::is_shell_script(&canonical) {
            return Err(HostError::LauncherScript { path: canonical });
        }
        if !fsx::is_executable(&canonical) {
            return Err(HostError::NotExecutable { path: canonical });
        }
        return Ok(Located {
            binary: canonical,
            source: LocateSource::EnvOverride,
            version_hint: None,
        });
    }

    // 2. The launcher's install dir.
    let install_dir = match env.muse_install_dir.as_ref().filter(|s| !s.is_empty()) {
        Some(d) => Some(PathBuf::from(d)),
        None => env
            .home
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|h| PathBuf::from(h).join(".local").join("bin")),
    };
    if let Some(dir) = install_dir {
        tried.push(format!("{}/.muse-version", dir.display()));
        if let Some((bin, version)) = active_binary(&dir) {
            return Ok(Located {
                binary: fsx::canonicalize(&bin)?,
                source: LocateSource::LauncherDir(dir),
                version_hint: Some(version),
            });
        }
    }

    // 3. PATH search.
    if let Some(path) = &env.path {
        for dir in std::env::split_paths(path) {
            let candidate = dir.join(muse_file_name());
            if !candidate.is_file() {
                continue;
            }
            tried.push(candidate.display().to_string());
            let canonical = fsx::canonicalize(&candidate)?;
            if fsx::is_shell_script(&canonical) {
                // The launcher: resolve through its own dir, like resolve_self().
                if let Some(launcher_dir) = canonical.parent() {
                    if let Some((bin, version)) = active_binary(launcher_dir) {
                        return Ok(Located {
                            binary: fsx::canonicalize(&bin)?,
                            source: LocateSource::PathSearch(canonical.clone()),
                            version_hint: Some(version),
                        });
                    }
                    tried.push(format!("{}/.muse-version", launcher_dir.display()));
                }
                continue;
            }
            if fsx::is_executable(&canonical) {
                return Ok(Located {
                    binary: canonical.clone(),
                    source: LocateSource::PathSearch(canonical),
                    version_hint: None,
                });
            }
        }
    }

    Err(HostError::BinaryNotFound { tried })
}

/// `muse-launcher.sh` `active_binary()`: read `<dir>/.muse-version`, return
/// `<dir>/muse-bin-<version>` when it exists and is executable.
pub fn active_binary(dir: &Path) -> Option<(PathBuf, String)> {
    let version = std::fs::read_to_string(dir.join(".muse-version")).ok()?;
    let version = version.trim();
    if version.is_empty()
        || version.contains('/')
        || version.contains('\\')
        || version.chars().any(char::is_whitespace)
    {
        return None;
    }
    let bin = dir.join(format!("muse-bin-{version}"));
    if fsx::is_executable(&bin) && !fsx::is_shell_script(&bin) {
        Some((bin, version.to_string()))
    } else {
        None
    }
}

#[cfg(windows)]
fn muse_file_name() -> &'static str {
    "muse.exe"
}

#[cfg(not(windows))]
fn muse_file_name() -> &'static str {
    "muse"
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn fake_bin(path: &Path) {
        fs::write(path, b"\xcf\xfa\xed\xfe fake").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn launcher(dir: &Path, version: &str) {
        let script = dir.join("muse");
        fs::write(&script, "#!/usr/bin/env bash\nexec muse-bin \"$@\"\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(dir.join(".muse-version"), format!("{version}\n")).unwrap();
        fake_bin(&dir.join(format!("muse-bin-{version}")));
    }

    #[test]
    fn env_override_wins_but_never_a_script() {
        let d = tempfile::tempdir().unwrap();
        launcher(d.path(), "1.0.1-R2006.1");
        let env = LocateEnv {
            omm_muse_bin: Some(d.path().join("muse-bin-1.0.1-R2006.1").into()),
            ..Default::default()
        };
        let got = locate_with(&env).unwrap();
        assert_eq!(got.source, LocateSource::EnvOverride);
        assert!(got.binary.ends_with("muse-bin-1.0.1-R2006.1"));

        let env = LocateEnv {
            omm_muse_bin: Some(d.path().join("muse").into()),
            ..Default::default()
        };
        assert!(matches!(
            locate_with(&env),
            Err(HostError::LauncherScript { .. })
        ));
    }

    #[test]
    fn launcher_dir_via_home_then_path() {
        let d = tempfile::tempdir().unwrap();
        let bin_dir = d.path().join(".local").join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        launcher(&bin_dir, "9.9.9-R1");
        let env = LocateEnv {
            home: Some(d.path().into()),
            ..Default::default()
        };
        let got = locate_with(&env).unwrap();
        assert_eq!(got.version_hint.as_deref(), Some("9.9.9-R1"));
        assert!(matches!(got.source, LocateSource::LauncherDir(_)));

        // No HOME, only PATH containing the launcher script.
        let env = LocateEnv {
            path: Some(bin_dir.clone().into()),
            ..Default::default()
        };
        let got = locate_with(&env).unwrap();
        assert!(got.binary.ends_with("muse-bin-9.9.9-R1"));
        assert!(matches!(got.source, LocateSource::PathSearch(_)));
    }

    #[test]
    fn path_raw_binary_is_accepted_and_missing_is_reported() {
        let d = tempfile::tempdir().unwrap();
        fake_bin(&d.path().join("muse"));
        let env = LocateEnv {
            path: Some(d.path().into()),
            ..Default::default()
        };
        assert!(locate_with(&env).is_ok());

        let empty = tempfile::tempdir().unwrap();
        let env = LocateEnv {
            path: Some(empty.path().into()),
            home: Some(empty.path().into()),
            ..Default::default()
        };
        match locate_with(&env) {
            Err(HostError::BinaryNotFound { tried }) => assert!(!tried.is_empty()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn env_override_reports_a_missing_file_as_not_found_with_the_tried_list() {
        let d = tempfile::tempdir().unwrap();
        let env = LocateEnv {
            omm_muse_bin: Some(d.path().join("nope").into()),
            ..Default::default()
        };
        match locate_with(&env) {
            Err(HostError::BinaryNotFound { tried }) => {
                assert_eq!(tried.len(), 1, "{tried:?}");
                assert!(tried[0].starts_with("OMM_MUSE_BIN="), "{tried:?}");
            }
            other => panic!("expected BinaryNotFound, got {other:?}"),
        }
    }

    #[test]
    fn env_override_refuses_a_non_executable_file() {
        let d = tempfile::tempdir().unwrap();
        let bin = d.path().join("muse-bin-1.0.0");
        fs::write(&bin, b"\xcf\xfa\xed\xfe fake").unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o644)).unwrap();
        let env = LocateEnv {
            omm_muse_bin: Some(bin.clone().into()),
            ..Default::default()
        };
        match locate_with(&env) {
            Err(HostError::NotExecutable { path }) => {
                assert_eq!(path, fs::canonicalize(&bin).unwrap());
            }
            other => panic!("expected NotExecutable, got {other:?}"),
        }
        // A directory is not a binary either.
        let env = LocateEnv {
            omm_muse_bin: Some(d.path().into()),
            ..Default::default()
        };
        assert!(matches!(
            locate_with(&env),
            Err(HostError::BinaryNotFound { .. })
        ));
    }

    #[test]
    fn active_binary_rejects_bad_version_files() {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join(".muse-version"), "../escape\n").unwrap();
        assert!(active_binary(d.path()).is_none());
        fs::write(d.path().join(".muse-version"), "1.0.0\n").unwrap();
        assert!(active_binary(d.path()).is_none(), "binary file missing");
        fake_bin(&d.path().join("muse-bin-1.0.0"));
        assert!(active_binary(d.path()).is_some());
    }
}
