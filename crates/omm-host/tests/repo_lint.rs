//! Repo-level rules that no unit test can see: the project rule that no
//! public-facing file names a foreign AI product (only its *format* names —
//! `.claude-plugin`, `Claude schema`, `Codex schema` — and the host's own
//! strings under `docs/host-data/` may appear), and the build plan staying in
//! step with the architecture.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("repo root")
}

/// Every file the lint scans: crate sources and tests, the top-level docs,
/// the experiment reports and the tool scripts. `research/` (raw material),
/// `docs/host-data/` (the host's own literals) and generated trees are out.
fn public_files() -> Vec<PathBuf> {
    let root = repo_root();
    let mut out = Vec::new();
    for top in ["crates", "docs", "tools", "scripts"] {
        let dir = root.join(top);
        if !dir.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&dir).into_iter().flatten() {
            let p = entry.path();
            if !entry.file_type().is_file() {
                continue;
            }
            if p.components().any(|c| {
                matches!(
                    c.as_os_str().to_str(),
                    Some("host-data") | Some("target") | Some("plans")
                )
            }) {
                continue;
            }
            match p.extension().and_then(|e| e.to_str()) {
                Some("rs") | Some("md") | Some("toml") | Some("sh") | Some("py") => {
                    out.push(p.to_path_buf())
                }
                _ => {}
            }
        }
    }
    assert!(out.len() > 20, "lint scanned only {} files", out.len());
    out
}

/// The product and vendor names, assembled so this file does not carry them
/// either. The third-party model vendor is here because its name had crept
/// in as the name of a *wire format* (`<vendor>-Responses SSE stream`, three
/// public files, Gate 0); the format is named by [`accepted_format_spellings`].
fn product_names() -> Vec<String> {
    vec![
        ["Claude", "Code"].join(" "),
        ["Codex", "CLI"].join(" "),
        ["Claude", "authored"].join("-"),
        ["Open", "AI"].join(""),
    ]
}

/// The spellings public files use for foreign formats: the manifest dirs and
/// catalog schemas of the two foreign plugin ecosystems, and the model wire
/// format the mock provider speaks (`tools/mockprovider/`). Recorded here so
/// the decision to name formats, never products, has one place to point at.
fn accepted_format_spellings() -> Vec<&'static str> {
    vec![
        ".claude-plugin",
        ".codex-plugin",
        "Claude schema",
        "Codex schema",
        "Responses-API SSE stream",
        "Responses API",
    ]
}

#[test]
fn the_wire_format_is_named_by_its_accepted_spelling() {
    // The three files that used to name the vendor now name the format.
    let root = repo_root();
    let format = accepted_format_spellings()
        .into_iter()
        .find(|s| s.contains("SSE"))
        .expect("the wire-format spelling");
    for rel in [
        "docs/experiments/context-slimming.md",
        "tools/mockprovider/mock.py",
        "tools/mockprovider/respond-toolcall.py",
    ] {
        let text = std::fs::read_to_string(root.join(rel)).unwrap();
        assert!(
            text.contains(format),
            "{rel} must name the wire format as `{format}`"
        );
    }
    // No accepted spelling contains a product name, so the two lists never
    // fight over a line.
    for spelling in accepted_format_spellings() {
        for name in product_names() {
            assert!(!spelling.contains(&name), "{spelling:?} carries {name:?}");
        }
    }
}

#[test]
fn no_public_file_names_a_foreign_ai_product() {
    let names = product_names();
    let mut hits = Vec::new();
    for file in public_files() {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        for (n, line) in text.lines().enumerate() {
            for name in &names {
                if line.contains(name.as_str()) {
                    hits.push(format!(
                        "{}:{}: {}",
                        file.strip_prefix(repo_root()).unwrap_or(&file).display(),
                        n + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        hits.is_empty(),
        "public files must name only formats, never the products:\n{}",
        hits.join("\n")
    );
}

#[test]
fn plan_task_2_1_generates_the_native_catalog_and_asserts_the_native_family() {
    // ARCHITECTURE §5.2: the root `marketplace.json` is the only catalog Muse
    // reads (first-found-wins); a plan row that generates only the two foreign
    // files makes `plugins install omm@ohmy` install the foreign projection.
    let plan = std::fs::read_to_string(repo_root().join("docs").join("PLAN.md")).unwrap();
    let row = plan
        .lines()
        .find(|l| l.starts_with("| 2.1 "))
        .expect("PLAN.md task 2.1 row");
    for needle in [
        "`marketplace.json`",
        "`.agents/plugins/marketplace.json`",
        "`.claude-plugin/marketplace.json`",
        "manifest_family",
        "native",
    ] {
        assert!(row.contains(needle), "task 2.1 row lacks {needle}:\n{row}");
    }
    let native_first = row.find("`marketplace.json`").unwrap();
    let foreign = row.find("`.agents/plugins/marketplace.json`").unwrap();
    assert!(
        native_first < foreign,
        "the native catalog must be named first (it is probed first):\n{row}"
    );
}

#[test]
fn plan_task_1_2_generates_all_three_marketplace_catalogs() {
    // The generator task must name the root `marketplace.json` — the only
    // catalog Muse reads — not just "both" foreign projections, or a Phase 1
    // agent implementing 1.2 from its row ships a marketplace Muse never opens.
    let plan = std::fs::read_to_string(repo_root().join("docs").join("PLAN.md")).unwrap();
    let row = plan
        .lines()
        .find(|l| l.starts_with("| 1.2 "))
        .expect("PLAN.md task 1.2 row");
    for needle in ["three marketplace catalogs", "`marketplace.json`", "§5.2"] {
        assert!(row.contains(needle), "task 1.2 row lacks {needle}:\n{row}");
    }
    assert!(
        !row.contains("both marketplace files"),
        "task 1.2 still says `both marketplace files`:\n{row}"
    );
}
