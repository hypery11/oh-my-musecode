//! The shipped `omm` version and description, read from `crates/omm/Cargo.toml`
//! — the one place the release number lives (ARCHITECTURE.md §5.2: the native
//! manifest's `version` "from crates/omm/Cargo.toml"). `version.workspace =
//! true` forwards to the root `Cargo.toml` `[workspace.package]` table, exactly
//! as cargo resolves it. No TOML crate: the two files are plain `key = "value"`
//! tables and the parser refuses anything it does not understand.

use std::path::Path;

use crate::error::{ManifestError, Result};
use crate::Repo;

/// What the CLI crate's manifest says about the shipped binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmmVersion {
    /// `package.version` (resolved through the workspace when inherited).
    pub version: String,
    /// `package.description` — the native manifest's required non-empty
    /// `description` (`research/experiments/plugin-contract.md` §1.2).
    pub description: String,
}

/// Read `crates/omm/Cargo.toml` (and the workspace manifest when inherited).
pub fn omm_version(repo: &Repo) -> Result<OmmVersion> {
    let cli = repo.omm_cli_manifest();
    let text = read(&cli)?;
    let version = match table_value(&text, "package", "version") {
        Some(TomlValue::Str(v)) => v,
        Some(TomlValue::Workspace) => {
            let root = repo.root.join("Cargo.toml");
            let root_text = read(&root)?;
            match table_value(&root_text, "workspace.package", "version") {
                Some(TomlValue::Str(v)) => v,
                _ => {
                    return Err(ManifestError::Version(format!(
                        "{} inherits `version` from the workspace but {} has no `[workspace.package] version`",
                        cli.display(),
                        root.display()
                    )))
                }
            }
        }
        _ => {
            return Err(ManifestError::Version(format!(
                "{} has no `[package] version`",
                cli.display()
            )))
        }
    };
    let description = match table_value(&text, "package", "description") {
        Some(TomlValue::Str(d)) if !d.trim().is_empty() => d,
        _ => {
            return Err(ManifestError::Version(format!(
                "{} has no non-empty `[package] description` (the native manifest requires one)",
                cli.display()
            )))
        }
    };
    if version.trim().is_empty() {
        return Err(ManifestError::Version(format!(
            "{}: empty version",
            cli.display()
        )));
    }
    Ok(OmmVersion {
        version,
        description,
    })
}

fn read(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| ManifestError::io("read", path, e))
}

/// A value the tiny parser understands.
#[derive(Clone, Debug, PartialEq, Eq)]
enum TomlValue {
    Str(String),
    /// `key.workspace = true`.
    Workspace,
}

/// Find `key` inside `[table]`: `key = "value"` or `key.workspace = true`.
/// Comments and blank lines are skipped; the search stops at the next table
/// header.
fn table_value(text: &str, table: &str, key: &str) -> Option<TomlValue> {
    let mut in_table = false;
    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(header) = line.strip_prefix('[') {
            let name = header.trim_end_matches(']').trim();
            in_table = name == table;
            continue;
        }
        if !in_table {
            continue;
        }
        let (k, v) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };
        if k == key {
            let v = v.trim_matches('"');
            return Some(TomlValue::Str(v.to_string()));
        }
        if k == format!("{key}.workspace") && v == "true" {
            return Some(TomlValue::Workspace);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_direct_and_inherited_versions() {
        let direct = "[package]\nname = \"x\"\nversion = \"1.2.3\" # note\ndescription = \"d\"\n";
        assert_eq!(
            table_value(direct, "package", "version"),
            Some(TomlValue::Str("1.2.3".into()))
        );
        let inherited = "[package]\nname = \"omm\"\nversion.workspace = true\n\n[dependencies]\nversion = \"9\"\n";
        assert_eq!(
            table_value(inherited, "package", "version"),
            Some(TomlValue::Workspace)
        );
        let ws = "[workspace]\nmembers = []\n\n[workspace.package]\nversion = \"0.1.0\"\n";
        assert_eq!(
            table_value(ws, "workspace.package", "version"),
            Some(TomlValue::Str("0.1.0".into()))
        );
        assert_eq!(table_value(ws, "package", "version"), None);
    }

    #[test]
    fn reads_the_real_cli_manifest() {
        let repo = Repo::from_cargo_manifest_dir().unwrap();
        let v = omm_version(&repo).unwrap();
        assert_eq!(v.version, env!("CARGO_PKG_VERSION"));
        assert!(!v.description.is_empty());
    }

    #[test]
    fn missing_fields_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo::new(dir.path()).unwrap();
        let cli = repo.omm_cli_manifest();
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, "[package]\nname = \"omm\"\nversion = \"1.0.0\"\n").unwrap();
        assert!(matches!(omm_version(&repo), Err(ManifestError::Version(_))));
        std::fs::write(
            &cli,
            "[package]\nname = \"omm\"\nversion.workspace = true\ndescription = \"d\"\n",
        )
        .unwrap();
        assert!(omm_version(&repo).is_err(), "no workspace manifest");
        std::fs::write(
            repo.root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"2.0.0\"\n",
        )
        .unwrap();
        assert_eq!(omm_version(&repo).unwrap().version, "2.0.0");
    }
}
