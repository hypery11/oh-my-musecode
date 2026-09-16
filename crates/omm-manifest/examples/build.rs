//! `omm build` before the CLI is wired (PLAN.md 1.5): lint, regenerate every
//! output with the digest obtained from the binary, and land it in the repo.
//!
//! ```sh
//! OMM_MUSE_BIN=.host/bin/muse-bin-<version> cargo run -p omm-manifest --example build [-- <repo-root>]
//! ```

use std::path::PathBuf;

use omm_manifest::catalog::Catalog;
use omm_manifest::content::Content;
use omm_manifest::generate::{self, BuildOptions, DigestSource};
use omm_manifest::lint::{self, LintOptions};
use omm_manifest::Repo;

fn main() {
    if let Err(e) = run() {
        eprintln!("build: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).map(PathBuf::from);
    let repo = match root {
        Some(r) => Repo::new(&r)?,
        None => Repo::from_cargo_manifest_dir()?,
    };
    let inv = omm_host::Invoker::from_env()?;
    let report = lint::run(
        &repo,
        &LintOptions {
            host: Some(&inv),
            check_marketplace: false,
        },
    )?;
    for f in &report.findings {
        eprintln!("{f}");
    }
    if !report.is_clean() {
        return Err(format!("{} lint error(s)", report.errors().len()).into());
    }
    let catalog = Catalog::load(&repo.catalog_path())?;
    let content = Content::load(&catalog, &repo.content_dir())?;
    let out = generate::build(
        &repo,
        &catalog,
        &content,
        &BuildOptions {
            version: None,
            description: None,
            digest: DigestSource::Host(&inv),
        },
    )?;
    let summary = generate::write(&repo, &out)?;
    println!(
        "omm {} digest {}\nplugins/{}: {} written, {} unchanged, {} removed\ndist/claude: {} written, {} unchanged, {} removed\ndist/codex: {} written, {} unchanged, {} removed",
        out.version,
        out.digest,
        out.plugin_id,
        summary.native.written.len(),
        summary.native.unchanged.len(),
        summary.native.removed.len(),
        summary.claude.written.len(),
        summary.claude.unchanged.len(),
        summary.claude.removed.len(),
        summary.codex.written.len(),
        summary.codex.unchanged.len(),
        summary.codex.removed.len(),
    );
    for (path, changed) in &summary.catalogs {
        println!("{path}: {}", if *changed { "written" } else { "unchanged" });
    }
    if let Some(b) = &report.budget {
        println!(
            "catalog estimate {} / {} B (first_sentence {} / {} B)",
            b.total_full, b.limit_full, b.total_first_sentence, b.limit_first_sentence
        );
    }
    Ok(())
}
