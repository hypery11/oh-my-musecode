//! Containment parity across the three layers (R4).
//!
//! The three `canonicalize() + strip_prefix()` checks are intentionally
//! layered, not duplicated:
//!
//! * `omm_ledger::Bases::resolve` is authoritative for writes: full refused-
//!   root handling (`/`, `$HOME`, base roots), missing files resolved via the
//!   deepest existing ancestor, leaf symlinks reported, intermediate symlinks
//!   followed-then-checked and reported via `via_symlink`.
//! * `omm_manifest::contained` is existing-paths-only for repo trees
//!   (overlay custom entries, generated parents that were just created):
//!   a missing child is `Err`, by design.
//! * `omm_doctor::ledger::contained_path` is the read-only D13 lane for
//!   ledger-relative strings: a lexically-inside but missing path is `Some`
//!   (reported downstream as `vanished`, never as `escaped`).
//!
//! These tests lock the security-critical agreement (escapes, absolute
//! paths, symlinks out are refused everywhere) and the one intentional
//! divergence (a missing-but-inside path).

use std::fs;
use std::path::{Path, PathBuf};

use omm_doctor::ledger::contained_path;
use omm_ledger::{Base, Bases, RelPath};

fn bases(dir: &Path) -> Bases {
    let b = Bases {
        muse_config: dir.join("config").join("muse"),
        muse_data: dir.join("data").join("muse"),
        omm: dir.join("config").join("omm"),
        workspace: Some(dir.join("ws")),
        home: Some(dir.join("home")),
        residue: vec![],
    };
    for p in [&b.muse_config, &b.muse_data, &b.omm, &dir.join("home")] {
        fs::create_dir_all(p).unwrap();
    }
    fs::create_dir_all(dir.join("ws")).unwrap();
    b
}

fn rel(s: &str) -> RelPath {
    RelPath::new(s).unwrap()
}

#[test]
fn an_existing_file_inside_is_contained_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let b = bases(dir.path());
    fs::create_dir_all(b.muse_config.join("skills/x")).unwrap();
    fs::write(b.muse_config.join("skills/x/SKILL.md"), b"s").unwrap();

    let resolved = b
        .resolve(Base::MuseConfig, &rel("skills/x/SKILL.md"))
        .unwrap();
    assert_eq!(resolved.state, omm_ledger::State::File);

    let root = fs::canonicalize(&b.muse_config).unwrap();
    assert!(
        omm_manifest::contained(&root, &root.join("skills/x/SKILL.md")).is_ok(),
        "manifest must accept an existing inside file"
    );
    assert!(
        contained_path(&b.muse_config, "skills/x/SKILL.md").is_some(),
        "doctor must accept an existing inside file"
    );
}

#[test]
fn escapes_are_refused_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let b = bases(dir.path());
    let root = fs::canonicalize(&b.muse_config).unwrap();

    // Grammar layer: `..` and absolute paths never become a RelPath.
    assert!(RelPath::new("../outside").is_err());
    assert!(RelPath::new("/etc/passwd").is_err());
    assert!(RelPath::new("").is_err());

    // Doctor layer: the same shapes are `None`.
    assert!(contained_path(&b.muse_config, "../outside").is_none());
    assert!(contained_path(&b.muse_config, "/etc/passwd").is_none());
    assert!(contained_path(&b.muse_config, "").is_none());

    // Manifest layer: an absolute outside path is `Err`.
    let outside: PathBuf = dir.path().join("outside");
    fs::write(&outside, b"y").unwrap();
    assert!(omm_manifest::contained(&root, &outside).is_err());
}

#[cfg(unix)]
#[test]
fn a_symlink_pointing_out_is_refused_everywhere() {
    use std::os::unix::fs::symlink;
    let dir = tempfile::tempdir().unwrap();
    let b = bases(dir.path());
    let root = fs::canonicalize(&b.muse_config).unwrap();

    let outside = dir.path().join("outside");
    fs::write(&outside, b"y").unwrap();
    fs::create_dir_all(b.muse_config.join("skills")).unwrap();
    symlink(&outside, b.muse_config.join("skills/link")).unwrap();

    let leaf = b.resolve(Base::MuseConfig, &rel("skills/link")).unwrap();
    assert_eq!(
        leaf.state,
        omm_ledger::State::Symlink,
        "ledger reports a leaf symlink without following it"
    );
    // Through the link: must be refused, never followed.
    assert!(
        b.resolve(Base::MuseConfig, &rel("skills/link/child"))
            .is_err(),
        "ledger must refuse a path through a symlink out"
    );
    assert!(
        contained_path(&b.muse_config, "skills/link").is_none(),
        "doctor must refuse a symlink out"
    );
    assert!(
        omm_manifest::contained(&root, &root.join("skills/link")).is_err(),
        "manifest must refuse a symlink out"
    );
}

#[test]
fn a_missing_but_inside_path_diverges_by_design() {
    let dir = tempfile::tempdir().unwrap();
    let b = bases(dir.path());
    let root = fs::canonicalize(&b.muse_config).unwrap();

    // Ledger: missing-but-inside resolves (state Missing) so callers can
    // create it; doctor: Some so D13 reports `vanished`, not `escaped`.
    let r = b
        .resolve(Base::MuseConfig, &rel("skills/new/SKILL.md"))
        .unwrap();
    assert_eq!(r.state, omm_ledger::State::Missing);
    assert!(contained_path(&b.muse_config, "skills/new/SKILL.md").is_some());
    // Manifest: existing-only, so missing is Err — callers only pass paths
    // they just created or stat-ed (overlay custom entries, generate
    // parents). This asymmetry is the documented contract, not a drift.
    assert!(omm_manifest::contained(&root, &root.join("skills/new/SKILL.md")).is_err());
}
