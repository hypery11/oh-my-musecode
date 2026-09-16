//! D15 — the programs the installed plugin's hooks and MCP servers name
//! resolve on `PATH` (ARCHITECTURE.md §6 row D15; PLAN.md 2.2).
//!
//! The bundle's hooks are `["omm","hook","<name>"]` and its MCP server is
//! `["omm","mcp"]` (R16, `content/mcp/doctor.json`). The host spawns each
//! as `command[0]` resolved through the `PATH` **it** inherited, with no
//! other hint (`research/experiments/plugin-mcp.md` §4.2, re-measured
//! 2026-09-02: a bare `command[1]` is handed over verbatim, `argv[0]`
//! absolute from `PATH`). An `omm` that is not on that `PATH` — installed
//! into `~/.local/bin` by a script that never touched the shell profile, or
//! run by its absolute path — means every hook and the MCP server never
//! start, and the symptom is identical to an unapproved capability: the
//! namespace is absent from `tools[]`, the hook never fires, nothing is
//! logged.
//!
//! The check reads the program words from what the host holds (`plugins
//! inspect --json → plugin.capabilities.{hooks,mcp_servers}[].command[0]`),
//! never from the catalog (R8: nothing here names an asset or spells `omm`),
//! and resolves each through this process's `PATH` — the one the host
//! inherits when both are started from the same shell. A word that is this
//! very binary's file name is also compared with `current_exe`: a *different*
//! `omm` first on `PATH` is what the host would run, so it is named.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use serde_json::Value;

use omm_host::fsx;

use crate::check::{Check, Context, InspectFailure};

pub const ID: &str = "D15";
pub const TITLE: &str = "omm on PATH";
/// `research/experiments/plugin-mcp.md` §4.1–4.2 and `mcp-tools-call.md`
/// §2.4: the child environment is the 16-key scrub with `PATH` verbatim,
/// `command[0]` is resolved through it, and a spawn that fails leaves no
/// stderr, no session record and no trace line — the model just sees no
/// `mcp__plugin_…` namespace and a hook just does not run.
pub const WHY_SILENT: &str = "the host spawns a plugin hook or MCP server as `command[0]` resolved through the PATH it inherited, with no other hint (plugin-mcp.md §4.2); a word that does not resolve fails before the handshake with no stderr, no session record and no trace line — the tool namespace is simply absent from tools[] and the hook never fires, exactly like an unapproved capability (mcp-tools-call.md §2.4)";

/// One program word and the capabilities that spawn it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Program {
    pub hooks: Vec<String>,
    pub mcp_servers: Vec<String>,
}

/// The `command[0]` words of every hook and MCP server the host lists for
/// the plugin, keyed by word.
pub fn programs(plugin: &Value) -> BTreeMap<String, Program> {
    let mut out: BTreeMap<String, Program> = BTreeMap::new();
    let Some(caps) = plugin.get("capabilities").and_then(Value::as_object) else {
        return out;
    };
    for (family, is_hook) in [("hooks", true), ("mcp_servers", false)] {
        let Some(rows) = caps.get(family).and_then(Value::as_array) else {
            continue;
        };
        for row in rows {
            let Some(word) = row
                .get("command")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
            else {
                continue;
            };
            let id = row
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            let entry = out.entry(word.to_string()).or_default();
            if is_hook {
                entry.hooks.push(id);
            } else {
                entry.mcp_servers.push(id);
            }
        }
    }
    out
}

/// The first `dir/word` on `path` that is an executable file.
pub fn resolve_on_path(word: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    let path = path?;
    std::env::split_paths(path)
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.join(word))
        .find(|p| fsx::is_executable(p))
}

/// The inputs of the pure evaluation, so a test can plant any of them.
#[derive(Clone, Debug, Default)]
pub struct Inputs {
    pub programs: BTreeMap<String, Program>,
    /// This process's `PATH`.
    pub path: Option<std::ffi::OsString>,
    /// The installed package's cache root (`record.cache_path`), for a
    /// relative `command[0]`.
    pub cache_path: Option<PathBuf>,
    /// This binary, for the "a different one is first on PATH" comparison.
    pub current_exe: Option<PathBuf>,
}

/// Evaluate without the host.
pub fn evaluate(inputs: &Inputs) -> Check {
    if inputs.programs.is_empty() {
        return Check::info(
            ID,
            TITLE,
            "the plugin declares no hook or MCP server command for the host to spawn",
            WHY_SILENT,
        );
    }
    let exe_name = inputs
        .current_exe
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned());
    let exe_dir = inputs
        .current_exe
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf);
    let mut rows: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut shadowed: Vec<String> = Vec::new();
    for (word, uses) in &inputs.programs {
        let uses_text = format!(
            "{} hook(s), {} MCP server(s)",
            uses.hooks.len(),
            uses.mcp_servers.len()
        );
        if word.contains('/') || word.contains('\\') {
            let p = Path::new(word);
            let full = if p.is_absolute() {
                p.to_path_buf()
            } else {
                inputs
                    .cache_path
                    .as_ref()
                    .map(|c| c.join(p))
                    .unwrap_or_else(|| p.to_path_buf())
            };
            if fsx::is_executable(&full) {
                rows.push(format!("`{word}` → {} ({uses_text})", full.display()));
            } else {
                missing.push(word.clone());
                rows.push(format!(
                    "`{word}` → {} is not an executable file ({uses_text})",
                    full.display()
                ));
            }
            continue;
        }
        match resolve_on_path(word, inputs.path.as_deref()) {
            Some(found) => {
                let same = match (&inputs.current_exe, exe_name.as_deref()) {
                    (Some(exe), Some(name)) if name == word => {
                        let a = std::fs::canonicalize(&found).unwrap_or(found.clone());
                        let b = std::fs::canonicalize(exe).unwrap_or(exe.clone());
                        Some(a == b)
                    }
                    _ => None,
                };
                if same == Some(false) {
                    shadowed.push(word.clone());
                    rows.push(format!(
                        "`{word}` → {} is first on PATH but this doctor is {} — the host would run the other one ({uses_text})",
                        found.display(),
                        inputs
                            .current_exe
                            .as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_default()
                    ));
                } else {
                    rows.push(format!("`{word}` → {} ({uses_text})", found.display()));
                }
            }
            None => {
                missing.push(word.clone());
                rows.push(format!("`{word}` is not on PATH ({uses_text})"));
            }
        }
    }
    let observed = rows.join("; ");
    if !missing.is_empty() {
        let fixes: Vec<String> = missing
            .iter()
            .map(|w| match (&exe_dir, exe_name.as_deref()) {
                (Some(dir), Some(name)) if name == w => format!(
                    "export PATH=\"{}:$PATH\"   # then add that line to your shell profile (~/.zshrc, ~/.bashrc)",
                    dir.display()
                ),
                _ => format!("install `{w}` or add its directory to PATH"),
            })
            .collect();
        return Check::critical(
            ID,
            TITLE,
            format!(
                "{} program(s) the host would spawn do not resolve — every hook and MCP server that names them silently never starts: {observed}",
                missing.len()
            ),
            WHY_SILENT,
            fixes.join("\n"),
        );
    }
    if !shadowed.is_empty() {
        let fix = exe_dir
            .as_ref()
            .map(|d| {
                format!(
                    "export PATH=\"{}:$PATH\"   # put this omm first",
                    d.display()
                )
            })
            .unwrap_or_else(|| "put the omm you installed first on PATH".to_string());
        return Check::warn(
            ID,
            TITLE,
            format!("a different binary is first on PATH: {observed}"),
            WHY_SILENT,
            fix,
        );
    }
    Check::info(ID, TITLE, observed, WHY_SILENT)
}

pub fn run(ctx: &Context) -> Check {
    let pid = ctx.plugin_id.as_str();
    let mode = ctx.install_mode();
    if mode.is_managed() {
        return Check::info(
            ID,
            TITLE,
            format!(
                "managed-store install (--no-plugin, {} skills, mode from the {}): no plugin hooks or MCP servers for the host to spawn",
                mode.skills.len(),
                mode.source
            ),
            WHY_SILENT,
        );
    }
    let ins = match ctx.inspect() {
        Ok(i) => i,
        Err(InspectFailure::NotInstalled(_)) => {
            return Check::info(
                ID,
                TITLE,
                format!("plugin `{pid}` is not installed: nothing for the host to spawn yet (D1 carries the fix)"),
                WHY_SILENT,
            )
        }
        Err(InspectFailure::Other(e)) => {
            return Check::warn(
                ID,
                TITLE,
                format!("could not inspect plugin `{pid}`: {e}"),
                WHY_SILENT,
                crate::check::FIX_INSTALL,
            )
        }
    };
    let programs = ins.raw.get("plugin").map(programs).unwrap_or_default();
    let cache_path = ins
        .raw
        .get("record")
        .and_then(|r| r.get("cache_path"))
        .and_then(Value::as_str)
        .map(PathBuf::from);
    evaluate(&Inputs {
        programs,
        path: std::env::var_os("PATH"),
        cache_path,
        current_exe: std::env::current_exe().ok(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Severity;
    use serde_json::json;

    fn exe(dir: &Path, name: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        p
    }

    fn plugin() -> Value {
        json!({"capabilities": {
            "hooks": [
                {"id": "omm-session-start", "command": ["omm", "hook", "session-start"]},
                {"id": "omm-guard", "command": ["omm", "hook", "guard"]},
            ],
            "mcp_servers": [
                {"id": "doctor", "transport": "stdio", "command": ["omm", "mcp"]},
                {"id": "py", "transport": "stdio", "command": ["python3", "mcp/server.py"]},
            ],
        }})
    }

    #[test]
    fn programs_are_keyed_by_the_first_argv_word() {
        let p = programs(&plugin());
        assert_eq!(p.len(), 2);
        assert_eq!(p["omm"].hooks, vec!["omm-session-start", "omm-guard"]);
        assert_eq!(p["omm"].mcp_servers, vec!["doctor"]);
        assert_eq!(p["python3"].mcp_servers, vec!["py"]);
        assert!(programs(&json!({})).is_empty());
    }

    #[test]
    fn nothing_declared_is_info() {
        let c = evaluate(&Inputs::default());
        assert_eq!(c.severity, Severity::Info);
    }

    #[test]
    fn a_missing_word_is_critical_with_the_install_dir_to_add() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        exe(&bin, "python3");
        let here = tmp.path().join("here");
        std::fs::create_dir_all(&here).unwrap();
        let me = exe(&here, "omm");
        let c = evaluate(&Inputs {
            programs: programs(&plugin()),
            path: Some(bin.clone().into()),
            cache_path: None,
            current_exe: Some(me),
        });
        assert_eq!(c.severity, Severity::Critical, "{c:?}");
        assert!(
            c.observed
                .contains("`omm` is not on PATH (2 hook(s), 1 MCP server(s))"),
            "{}",
            c.observed
        );
        assert!(c.observed.contains("`python3` →"), "{}", c.observed);
        let fix = c.fix.unwrap();
        assert!(
            fix.starts_with(&format!("export PATH=\"{}:$PATH\"", here.display())),
            "{fix}"
        );
        assert!(fix.contains("shell profile"));
        // A word that is not this binary gets the generic fix.
        let c = evaluate(&Inputs {
            programs: programs(&plugin()),
            path: Some(here.clone().into()),
            cache_path: None,
            current_exe: Some(here.join("omm")),
        });
        assert_eq!(c.severity, Severity::Critical, "{c:?}");
        assert_eq!(
            c.fix.as_deref(),
            Some("install `python3` or add its directory to PATH")
        );
    }

    #[test]
    fn everything_resolving_is_info_and_a_shadowing_binary_is_a_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let me = exe(&bin, "omm");
        exe(&bin, "python3");
        let c = evaluate(&Inputs {
            programs: programs(&plugin()),
            path: Some(bin.clone().into()),
            cache_path: None,
            current_exe: Some(me.clone()),
        });
        assert_eq!(c.severity, Severity::Info, "{c:?}");
        assert!(
            c.observed.contains(&format!("`omm` → {}", me.display())),
            "{}",
            c.observed
        );
        assert!(c.fix.is_none());
        // Another omm first on PATH.
        let other = tmp.path().join("other");
        std::fs::create_dir_all(&other).unwrap();
        exe(&other, "omm");
        let path = std::env::join_paths([other.clone(), bin.clone()]).unwrap();
        let c = evaluate(&Inputs {
            programs: programs(&plugin()),
            path: Some(path),
            cache_path: None,
            current_exe: Some(me),
        });
        assert_eq!(c.severity, Severity::Warn, "{c:?}");
        assert!(
            c.observed.contains("the host would run the other one"),
            "{}",
            c.observed
        );
        assert!(c
            .fix
            .unwrap()
            .starts_with(&format!("export PATH=\"{}:$PATH\"", bin.display())));
        // No PATH at all.
        let c = evaluate(&Inputs {
            programs: programs(&plugin()),
            path: None,
            cache_path: None,
            current_exe: None,
        });
        assert_eq!(c.severity, Severity::Critical);
    }

    #[test]
    fn a_path_word_is_checked_relative_to_the_package_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache = tmp.path().join("cache");
        std::fs::create_dir_all(cache.join("bin")).unwrap();
        exe(&cache.join("bin"), "server");
        let rel = json!({"capabilities": {"mcp_servers": [{"id": "x", "command": ["bin/server", "--stdio"]}]}});
        let c = evaluate(&Inputs {
            programs: programs(&rel),
            path: None,
            cache_path: Some(cache.clone()),
            current_exe: None,
        });
        assert_eq!(c.severity, Severity::Info, "{c:?}");
        let c = evaluate(&Inputs {
            programs: programs(&rel),
            path: None,
            cache_path: Some(tmp.path().join("elsewhere")),
            current_exe: None,
        });
        assert_eq!(c.severity, Severity::Critical, "{c:?}");
        assert!(
            c.observed.contains("not an executable file"),
            "{}",
            c.observed
        );
        let abs = json!({"capabilities": {"hooks": [{"id": "h", "command": [cache.join("bin/server").display().to_string()]}]}});
        assert_eq!(
            evaluate(&Inputs {
                programs: programs(&abs),
                ..Inputs::default()
            })
            .severity,
            Severity::Info
        );
    }
}
