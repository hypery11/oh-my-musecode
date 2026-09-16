//! `omm build` refreshes the catalog's budget numbers and `omm build
//! --check` treats a stale one as drift (Gate 1: the totals in
//! `content/catalog.json` went stale with only an `omm lint` warning to say
//! so, while `--check` reported no drift).

mod common;

use common::Tree;
use omm_manifest::budget;
use omm_manifest::drift::{self, DriftKind};
use omm_manifest::generate;
use serde_json::{json, Value};

fn catalog_doc(tree: &Tree) -> Value {
    serde_json::from_slice(&std::fs::read(tree.repo().catalog_path()).unwrap()).unwrap()
}

#[test]
fn build_refreshes_stale_budget_numbers_and_check_reports_them_as_drift() {
    let mut tree = Tree::minimal();
    // The author's header with stale totals, and a stale row.
    tree.edit_catalog(|c| {
        c["budget"] = json!({
            "bundle_limit_bytes": 21542,
            "bundle_limit_first_sentence_bytes": 27168,
            "skills_total_bytes": 1,
            "skills_total_first_sentence_bytes": 1
        });
    });
    tree.edit_asset("omm-alpha", |a| {
        a["budget_bytes"] = json!(1);
        a["budget_bytes_first_sentence"] = json!(1);
    });
    let repo = tree.repo();
    let (_, content) = tree.load();
    let estimate = budget::estimate(&content);
    assert!(estimate.total_full > 1);

    // A build lands the refreshed numbers; the file keeps its shape.
    let out = tree.build();
    assert_ne!(out.catalog, std::fs::read(repo.catalog_path()).unwrap());
    let summary = generate::write(&repo, &out).unwrap();
    assert_eq!(
        summary.catalog,
        Some(("content/catalog.json".to_string(), true))
    );
    let doc = catalog_doc(&tree);
    assert_eq!(doc["budget"]["skills_total_bytes"], estimate.total_full);
    assert_eq!(
        doc["budget"]["skills_total_first_sentence_bytes"],
        estimate.total_first_sentence
    );
    let alpha = doc["assets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == "omm-alpha")
        .unwrap();
    let alpha_cost = &estimate
        .entries
        .iter()
        .find(|e| e.id == "omm-alpha")
        .unwrap()
        .cost;
    assert_eq!(alpha["budget_bytes"], alpha_cost.full);
    assert_eq!(
        alpha["budget_bytes_first_sentence"],
        alpha_cost.first_sentence
    );
    assert_eq!(
        doc["budget"]["bundle_limit_bytes"], 21542,
        "untouched keys stay"
    );
    // Key order and formatting are the catalog's own: a second build is a no-op.
    let again = generate::write(&repo, &tree.build()).unwrap();
    assert!(again.is_noop(), "{again:?}");
    assert!(drift::check(&repo, None).unwrap().is_clean());

    // A stale total is drift for --check, and only that path.
    tree.catalog = catalog_doc(&tree);
    tree.edit_catalog(|c| c["budget"]["skills_total_bytes"] = json!(7));
    let report = drift::check(&repo, None).unwrap();
    assert_eq!(
        report
            .drifts
            .iter()
            .map(|d| (d.path.as_str(), d.kind))
            .collect::<Vec<_>>(),
        vec![("content/catalog.json", DriftKind::Changed)]
    );
    // `omm build` heals it.
    let healed = generate::write(&repo, &tree.build()).unwrap();
    assert_eq!(
        healed.catalog,
        Some(("content/catalog.json".to_string(), true))
    );
    assert!(drift::check(&repo, None).unwrap().is_clean());
    assert!(!budget::estimate(&tree.load().1)
        .stale()
        .iter()
        .any(|e| e.id == "omm-alpha"));
}
