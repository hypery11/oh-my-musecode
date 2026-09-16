//! Golden-file tests for every generator (PLAN.md 1.2): the minimal fixture
//! tree renders byte-for-byte into `tests/golden/mini/`. Regenerate the
//! goldens with `OMM_UPDATE_GOLDEN=1 cargo test -p omm-manifest --test golden`
//! and review the diff.

mod common;

use std::path::{Path, PathBuf};

use common::Tree;
use omm_manifest::generate::{self, BuildOptions, DigestSource};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join("mini")
}

fn update() -> bool {
    std::env::var_os("OMM_UPDATE_GOLDEN").is_some()
}

fn compare(rel: &str, actual: &[u8]) -> Option<String> {
    let path = golden_dir().join(rel);
    if update() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return None;
    }
    match std::fs::read(&path) {
        Ok(expected) if expected == actual => None,
        Ok(expected) => Some(format!(
            "{rel} differs from the golden\n--- golden\n{}\n--- actual\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(actual)
        )),
        Err(_) => Some(format!("{rel}: golden file missing at {}", path.display())),
    }
}

#[test]
fn every_generator_matches_its_golden() {
    let tree = Tree::minimal();
    let out = tree.build();
    assert_eq!(out.version, common::VERSION);
    assert_eq!(out.description, common::DESCRIPTION);
    let mut failures = Vec::new();
    let mut expected_paths = Vec::new();
    for (prefix, pkg) in [
        ("plugins/omm", &out.native),
        ("dist/claude", &out.claude),
        ("dist/codex", &out.codex),
    ] {
        for (rel, bytes) in pkg.files() {
            let full = format!("{prefix}/{rel}");
            expected_paths.push(full.clone());
            if let Some(f) = compare(&full, bytes) {
                failures.push(f);
            }
        }
    }
    for (rel, bytes) in [
        ("marketplace.json", &out.marketplace_native),
        (".agents/plugins/marketplace.json", &out.marketplace_codex),
        (".claude-plugin/marketplace.json", &out.marketplace_claude),
    ] {
        expected_paths.push(rel.to_string());
        if let Some(f) = compare(rel, bytes) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    // No golden file is left over from a dropped output.
    if !update() {
        let mut on_disk: Vec<String> = common::tree_files(&golden_dir())
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        on_disk.sort();
        expected_paths.sort();
        assert_eq!(on_disk, expected_paths, "stale golden files");
    }
}

#[test]
fn build_lands_in_a_repo_and_is_idempotent_and_drift_free() {
    let tree = Tree::minimal();
    let out = tree.build();
    let repo = tree.repo();
    let summary = generate::write(&repo, &out).unwrap();
    assert!(!summary.is_noop());
    assert!(repo
        .native_package_dir("oh-my-musecode")
        .join(".muse-plugin/plugin.json")
        .is_file());
    assert!(repo
        .dist_claude_dir()
        .join(".claude-plugin/plugin.json")
        .is_file());
    assert!(repo
        .dist_codex_dir()
        .join(".codex-plugin/plugin.json")
        .is_file());
    assert!(repo.marketplace_native_path().is_file());
    assert!(repo.marketplace_codex_path().is_file());
    assert!(repo.marketplace_claude_path().is_file());
    let again = generate::write(&repo, &out).unwrap();
    assert!(again.is_noop(), "{again:?}");
    // Drift: clean right after a build; a content edit is drift; a version
    // bump alone is not.
    let report = omm_manifest::drift::check(&repo, None).unwrap();
    assert!(report.is_clean(), "{:#?}", report.drifts);
    assert!(!report.version_bump_pending());
    tree.write_repo_file(
        "crates/omm/Cargo.toml",
        &format!(
            "[package]\nname = \"omm\"\nversion = \"9.9.9\"\ndescription = \"{}\"\n",
            common::DESCRIPTION
        ),
    );
    let report = omm_manifest::drift::check(&repo, None).unwrap();
    assert!(
        report.is_clean(),
        "a version bump alone must not be drift: {:#?}",
        report.drifts
    );
    assert!(report.version_bump_pending());
    tree.write("skills/omm-alpha/references/notes.md", "changed\n");
    let report = omm_manifest::drift::check(&repo, None).unwrap();
    let paths: Vec<&str> = report.drifts.iter().map(|d| d.path.as_str()).collect();
    assert!(
        paths.contains(&"plugins/oh-my-musecode/skills/omm-alpha/references/notes.md"),
        "{paths:?}"
    );
    assert!(paths.contains(&"dist/claude/skills/omm-alpha/references/notes.md"));
    assert!(paths.contains(&"dist/codex/skills/omm-alpha/references/notes.md"));
    // A stale committed file is an Extra; a removed one is Missing.
    std::fs::write(
        repo.native_package_dir("oh-my-musecode").join("stray.txt"),
        b"x",
    )
    .unwrap();
    std::fs::remove_file(repo.marketplace_claude_path()).unwrap();
    let report = omm_manifest::drift::check(&repo, None).unwrap();
    assert!(report
        .drifts
        .iter()
        .any(|d| d.path == "plugins/oh-my-musecode/stray.txt"
            && d.kind == omm_manifest::drift::DriftKind::Extra));
    assert!(report
        .drifts
        .iter()
        .any(|d| d.path == ".claude-plugin/marketplace.json"
            && d.kind == omm_manifest::drift::DriftKind::Missing));
}

#[test]
fn build_refuses_problems_and_bad_digests() {
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let bad = BuildOptions {
        version: None,
        description: None,
        digest: DigestSource::Fixed("sha256:short".to_string()),
    };
    assert!(generate::build(&tree.repo(), &catalog, &content, &bad).is_err());
    tree.write("skills/omm-alpha/SKILL.md", "no frontmatter\n");
    let (catalog, content) = tree.load();
    assert!(!content.problems.is_empty());
    let ok = BuildOptions {
        version: None,
        description: None,
        digest: DigestSource::Fixed(Tree::fixed_digest()),
    };
    let err = generate::build(&tree.repo(), &catalog, &content, &ok).unwrap_err();
    assert!(err.to_string().contains("problem"), "{err}");
}
