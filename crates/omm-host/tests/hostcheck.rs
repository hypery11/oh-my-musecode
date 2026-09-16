//! PLAN.md 0.3 — the P0 + P1 tables of docs/host-reality.md against the real
//! binary. Skipped with a clear message when `OMM_MUSE_BIN` is unset.

use std::path::PathBuf;

fn muse_bin() -> Option<PathBuf> {
    std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

#[test]
fn hostcheck_p0_p1_against_the_stable_binary() {
    let Some(bin) = muse_bin() else {
        eprintln!(
            "hostcheck SKIPPED: OMM_MUSE_BIN is unset — point it at .host/bin/muse-bin-<version> to run the P0/P1 host checks"
        );
        return;
    };
    let report = omm_host::hostcheck::run(&bin).expect("hostcheck should run to completion");
    eprintln!("{}", report.render());
    assert!(
        report.ok(),
        "host drift detected ({} failed):\n{}",
        report.failures().len(),
        report.render()
    );
    let p0 = report
        .checks
        .iter()
        .filter(|c| c.tier == omm_host::hostcheck::Tier::P0)
        .count();
    assert!(p0 >= 20, "expected the full P0 block, got {p0} rows");
    // The remote feature-config cache is reported explicitly: a populated one
    // would move `gates/default-on` and `bundled/*` with no binary change.
    let fc = report
        .checks
        .iter()
        .find(|c| c.id == "gates/feature-config-cache")
        .unwrap_or_else(|| panic!("no gates/feature-config-cache row:\n{}", report.render()));
    assert_eq!(fc.status, omm_host::hostcheck::Status::Pass, "{fc:?}");
    assert!(fc.observed.contains("gate_count=0"), "{fc:?}");
}
