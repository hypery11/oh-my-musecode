//! The real run (PLAN.md 1.2 acceptance): generate from `content/` into a
//! temp dir, run the R13 checkpoints and R11's four predicates against the
//! binary, obtain the digest from the binary, and assert it is what
//! `marketplace add` + `list --available --json` + `install` report in a
//! sandbox. Skipped with a message when `OMM_MUSE_BIN` is unset.

mod common;

use std::path::PathBuf;

use common::Tree;
use omm_host::probe;
use omm_host::Invoker;
use omm_manifest::catalog::Catalog;
use omm_manifest::content::Content;
use omm_manifest::generate::{self, marketplace, BuildOptions, DigestSource};
use omm_manifest::lint::{self, LintOptions};
use omm_manifest::Repo;

fn host() -> Option<Invoker> {
    let bin = std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)?;
    Some(Invoker::new(bin))
}

macro_rules! host_or_skip {
    () => {
        match host() {
            Some(h) => h,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

#[test]
fn shipped_content_passes_the_host_checkpoints_and_the_digest_round_trips() {
    let inv = host_or_skip!();
    let repo = Repo::from_cargo_manifest_dir().unwrap();
    // R13 checkpoints 1 and 2 through the lint.
    let report = lint::run(
        &repo,
        &LintOptions {
            host: Some(&inv),
            check_marketplace: false,
        },
    )
    .unwrap();
    assert!(report.host_checked);
    let errors: Vec<String> = report.errors().iter().map(|f| f.to_string()).collect();
    assert!(errors.is_empty(), "{errors:#?}");

    // Generate for real into a temp repo, digest from the binary.
    let catalog = Catalog::load(&repo.catalog_path()).unwrap();
    let content = Content::load(&catalog, &repo.content_dir()).unwrap();
    let out = generate::build(
        &repo,
        &catalog,
        &content,
        &BuildOptions {
            version: None,
            description: None,
            digest: DigestSource::Host(&inv),
        },
    )
    .unwrap();
    assert!(marketplace::is_digest(&out.digest));
    let tmp = tempfile::Builder::new()
        .prefix("omm-host-test-")
        .tempdir()
        .unwrap();
    let temp_repo = Repo::new(tmp.path()).unwrap();
    generate::write(&temp_repo, &out).unwrap();

    // R11 on the landed package.
    let v =
        probe::plugins_validate(&inv, &temp_repo.native_package_dir(&catalog.plugin_id)).unwrap();
    assert!(v.passes(), "{:?}", v.four_predicates());
    assert_eq!(
        v.declarations.len(),
        content.skills.len()
            + content.commands.len()
            + content.hooks.len()
            + content.reminders.len()
            + content.mcp_servers.len(),
        "one `plugins validate` declaration per skill, command, hook, reminder and MCP server"
    );

    // The projections validate in their own families (no fatal error).
    let c = probe::plugins_validate(&inv, &temp_repo.dist_claude_dir()).unwrap();
    assert!(c.valid && c.error.is_none(), "{:?}", c.raw);
    assert_eq!(c.manifest_family.as_deref(), Some("claude-compatible"));
    let x = probe::plugins_validate(&inv, &temp_repo.dist_codex_dir()).unwrap();
    assert!(x.valid && x.error.is_none(), "{:?}", x.raw);
    assert_eq!(x.manifest_family.as_deref(), Some("codex-compatible"));

    // marketplace-precedence.md §6.3: add → list → install, digest equal, family native.
    let check = marketplace::verify_install(&inv, &temp_repo.root, &catalog.plugin_id).unwrap();
    assert!(check.passes().is_empty(), "{check:#?}");
    assert_eq!(check.listed_digest.as_deref(), Some(out.digest.as_str()));
    assert_eq!(check.package_sha256.as_deref(), Some(out.digest.as_str()));
    let inspect = check.inspect.as_ref().unwrap();
    assert_eq!(inspect.manifest_family.as_deref(), Some("native"));
    assert_eq!(
        inspect.runtime_capabilities.len(),
        content.hooks.len() + content.reminders.len() + content.mcp_servers.len(),
        "every hook/reminder/MCP line is present (review_needed until approved)"
    );
}

#[test]
fn committed_repo_output_is_current_against_the_binary() {
    let inv = host_or_skip!();
    let repo = Repo::from_cargo_manifest_dir().unwrap();
    let drift = omm_manifest::drift::check(&repo, Some(&inv)).unwrap();
    assert!(drift.digest_checked);
    assert!(drift.is_clean(), "{:#?}", drift.drifts);
    let catalog = Catalog::load(&repo.catalog_path()).unwrap();
    let check = marketplace::verify_install(&inv, &repo.root, &catalog.plugin_id).unwrap();
    assert!(check.passes().is_empty(), "{check:#?}");
    let report = lint::run(
        &repo,
        &LintOptions {
            host: Some(&inv),
            check_marketplace: true,
        },
    )
    .unwrap();
    let errors: Vec<String> = report.errors().iter().map(|f| f.to_string()).collect();
    assert!(errors.is_empty(), "{errors:#?}");
}

#[test]
fn digest_is_content_addressed_and_the_mini_fixture_validates() {
    let inv = host_or_skip!();
    let tree = Tree::minimal();
    let (catalog, content) = tree.load();
    let pkg = generate::native::render(&catalog, &content, "0.1.0", "d").unwrap();
    let d1 = marketplace::package_digest(&inv, &pkg).unwrap();
    let d2 = marketplace::package_digest(&inv, &pkg).unwrap();
    assert_eq!(d1, d2);
    let pkg2 = generate::native::render(&catalog, &content, "0.1.1", "d").unwrap();
    assert_ne!(marketplace::package_digest(&inv, &pkg2).unwrap(), d1);
    let report = lint::run(
        &tree.repo(),
        &LintOptions {
            host: Some(&inv),
            check_marketplace: false,
        },
    )
    .unwrap();
    assert!(report.findings.is_empty(), "{:#?}", report.findings);
    // A hook carrying commandWindows never reaches the host: the loader stops it first.
    tree.write("hooks/omm-start.json", "{\"id\":\"omm-start\",\"event\":\"SessionStart\",\"command\":[\"omm\",\"hook\",\"start\"],\"timeoutMs\":2000,\"commandWindows\":[\"omm\",\"hook\",\"start\"]}");
    let report = lint::run(
        &tree.repo(),
        &LintOptions {
            host: Some(&inv),
            check_marketplace: false,
        },
    )
    .unwrap();
    assert!(report.rules().contains("hook-field"));
    assert!(
        !report.rules().contains("host-plugins-validate"),
        "host checkpoints are skipped while problems remain"
    );
}

#[test]
fn host_checkpoints_report_a_skill_the_validator_rejects() {
    let inv = host_or_skip!();
    let tree = Tree::minimal();
    // Unknown front-matter fields are validator warnings, and warnings are
    // failures for creation (plugin-contract.md §1.2 / §1.13).
    tree.write(
        "skills/omm-alpha/SKILL.md",
        &format!(
            "---\nname: omm-alpha\ndescription: {}\nallowed-tools: Bash\n---\n",
            common::SKILL_DESC
        ),
    );
    let report = lint::run(
        &tree.repo(),
        &LintOptions {
            host: Some(&inv),
            check_marketplace: false,
        },
    )
    .unwrap();
    assert!(
        report.rules().contains("host-skills-validate"),
        "{:#?}",
        report.findings
    );
}
