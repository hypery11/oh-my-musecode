//! The CI drift gate (ARCHITECTURE.md §5.2): regenerate everything into
//! memory and diff it against the committed `plugins/<pid>/`, `dist/claude/`,
//! `dist/codex/` and the three root catalogs.
//!
//! `version` is normalised: the regeneration uses the version the committed
//! native manifest carries, so a version bump alone never fails the gate —
//! only a content change does. With a host, the digest is asked of the binary
//! (so a stale committed digest IS drift); without one, the committed digest
//! is reused and only the rest is compared.

use std::path::Path;

use omm_host::Invoker;
use serde::Serialize;
use serde_json::Value;

use crate::catalog::Catalog;
use crate::content::Content;
use crate::error::{ManifestError, Result};
use crate::generate::{self, native, BuildOptions, BuildOutput, DigestSource, Package};
use crate::Repo;

/// How a committed path differs from the regeneration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DriftKind {
    /// Generated but not committed.
    Missing,
    /// Committed but not generated.
    Extra,
    /// Bytes differ.
    Changed,
}

/// One differing path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Drift {
    /// Repo-relative.
    pub path: String,
    pub kind: DriftKind,
}

/// The result of a drift check.
#[derive(Clone, Debug, Default, Serialize)]
pub struct DriftReport {
    pub drifts: Vec<Drift>,
    /// The version the committed native manifest carries (the one regenerated with).
    pub committed_version: Option<String>,
    /// `crates/omm/Cargo.toml`'s version — informational.
    pub crate_version: String,
    /// Whether the digest was re-obtained from the binary.
    pub digest_checked: bool,
}

impl DriftReport {
    /// No drift.
    pub fn is_clean(&self) -> bool {
        self.drifts.is_empty()
    }
    /// A bump is pending when the committed version is not the crate's.
    pub fn version_bump_pending(&self) -> bool {
        self.committed_version.as_deref() != Some(self.crate_version.as_str())
    }
}

/// The committed native manifest's `version` and the native catalog's digest.
fn committed_facts(repo: &Repo, plugin_id: &str) -> (Option<String>, Option<String>) {
    let manifest = repo
        .native_package_dir(plugin_id)
        .join(native::manifest_path());
    let version = std::fs::read(&manifest)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v["version"].as_str().map(str::to_string));
    let digest = std::fs::read(repo.marketplace_native_path())
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| {
            v["plugins"][0]["integrity"]["digest"]
                .as_str()
                .map(str::to_string)
        });
    (version, digest)
}

/// Regenerate with the committed version and diff.
pub fn check(repo: &Repo, host: Option<&Invoker>) -> Result<DriftReport> {
    let catalog = Catalog::load(&repo.catalog_path())?;
    let content = Content::load(&catalog, &repo.content_dir())?;
    let (committed_version, committed_digest) = committed_facts(repo, &catalog.plugin_id);
    let crate_version = crate::version::omm_version(repo)?.version;
    let digest = match (host, &committed_digest) {
        (Some(inv), _) => DigestSource::Host(inv),
        (None, Some(d)) => DigestSource::Fixed(d.clone()),
        (None, None) => return Err(ManifestError::Generate(
            "no host to obtain the digest from and no committed marketplace.json to reuse it from"
                .to_string(),
        )),
    };
    let opts = BuildOptions {
        version: committed_version.clone(),
        description: None,
        digest,
    };
    let out = generate::build(repo, &catalog, &content, &opts)?;
    let mut report = DriftReport {
        drifts: Vec::new(),
        committed_version,
        crate_version,
        digest_checked: host.is_some(),
    };
    report.drifts.extend(diff_tree(
        &Repo::native_package_rel(&out.plugin_id),
        &out.native,
        &repo.native_package_dir(&out.plugin_id),
    )?);
    report.drifts.extend(diff_tree(
        &Repo::dist_claude_rel(),
        &out.claude,
        &repo.dist_claude_dir(),
    )?);
    report.drifts.extend(diff_tree(
        &Repo::dist_codex_rel(),
        &out.codex,
        &repo.dist_codex_dir(),
    )?);
    report.drifts.extend(diff_catalogs(repo, &out));
    // The catalog's own budget numbers (§5.3): stale is drift, `omm build` refreshes.
    let catalog_path = repo.catalog_path();
    match std::fs::read(&catalog_path) {
        Ok(c) if c != out.catalog => report.drifts.push(Drift {
            path: repo.relative(&catalog_path),
            kind: DriftKind::Changed,
        }),
        _ => {}
    }
    Ok(report)
}

fn diff_tree(prefix: &str, generated: &Package, committed_dir: &Path) -> Result<Vec<Drift>> {
    let committed = Package::read_from(committed_dir)?;
    let mut out = Vec::new();
    for (path, bytes) in generated.files() {
        match committed.get(path) {
            None => out.push(Drift {
                path: format!("{prefix}/{path}"),
                kind: DriftKind::Missing,
            }),
            Some(c) if c != bytes => out.push(Drift {
                path: format!("{prefix}/{path}"),
                kind: DriftKind::Changed,
            }),
            Some(_) => {}
        }
    }
    for (path, _) in committed.files() {
        if generated.get(path).is_none() {
            out.push(Drift {
                path: format!("{prefix}/{path}"),
                kind: DriftKind::Extra,
            });
        }
    }
    Ok(out)
}

fn diff_catalogs(repo: &Repo, out: &BuildOutput) -> Vec<Drift> {
    let files = [
        (repo.marketplace_native_path(), &out.marketplace_native),
        (repo.marketplace_codex_path(), &out.marketplace_codex),
        (repo.marketplace_claude_path(), &out.marketplace_claude),
    ];
    let mut drifts = Vec::new();
    for (path, bytes) in files {
        let rel = repo.relative(&path);
        match std::fs::read(&path) {
            Err(_) => drifts.push(Drift {
                path: rel,
                kind: DriftKind::Missing,
            }),
            Ok(c) if &c != bytes => drifts.push(Drift {
                path: rel,
                kind: DriftKind::Changed,
            }),
            Ok(_) => {}
        }
    }
    drifts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_repo_output_is_current() {
        // The generator ran into the repo (`cargo run -p omm-manifest --example
        // build`); without a host the committed digest is reused, so this
        // asserts every generated byte except the digest.
        let repo = Repo::from_cargo_manifest_dir().unwrap();
        let report = check(&repo, None).unwrap();
        assert!(report.is_clean(), "{:#?}", report.drifts);
        assert!(!report.digest_checked);
        assert!(!report.version_bump_pending(), "{report:?}");
    }
}
