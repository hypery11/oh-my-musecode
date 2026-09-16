//! `omm-manifest` — content → packages (ARCHITECTURE.md §5).
//!
//! `content/` is the hand-authored canonical source and `content/catalog.json`
//! is THE asset list (R8: nothing in Rust knows an asset name). The generators
//! write the native package, the `.claude-plugin` / `.codex-plugin` projections
//! and the three marketplace catalogs; lint enforces the host's identity and
//! package rules from the data files in `omm-host`; the overlay resolver
//! implements R7; the drift check regenerates everything into memory and diffs
//! it against the committed trees for CI.
//!
//! Module map:
//!
//! * [`catalog`] — `content/catalog.json` (§5.1): lifecycle, the core gate,
//!   alias forwarding.
//! * [`content`] — the tree under `content/`, parsed and cross-checked against
//!   the catalog in both directions.
//! * [`frontmatter`] — the SKILL.md / command front-matter loader, with the
//!   host's quirks (`docs/host-reality.md` "SKILL.md frontmatter loader").
//! * [`budget`] — the catalog byte estimate (R18) with the measured entry formula.
//! * [`generate`] — `omm build`: native package, the two projections, the three
//!   marketplace catalogs; the native `integrity.digest` comes from the binary.
//! * [`lint`] — every §5.3 rule plus the three host checkpoints (R13).
//! * [`overlay`] — §5.4 resolution with `_shadowed` provenance.
//! * [`drift`] — regenerate and diff against the committed output (CI gate).
//! * [`version`] — the `omm` crate version and description from its `Cargo.toml`.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub mod budget;
pub mod catalog;
pub mod content;
pub mod drift;
pub mod error;
pub mod frontmatter;
pub mod generate;
pub mod lint;
pub mod overlay;
pub mod version;

use std::path::{Path, PathBuf};

pub use error::{ManifestError, Result};
pub use omm_host::host_reality as hr;

/// The marketplace name every catalog and install verb uses:
/// `oh-my-musecode@omm` works in every tool (ARCHITECTURE.md §5.2;
/// `docs/experiments/marketplace-precedence.md` §6.1 consequence 3 — the
/// Claude-schema file's `name` must therefore be this).
/// Held against `reserved-ids.json → reserved_marketplace_names` by the lint.
pub const MARKETPLACE_NAME: &str = "omm";

/// `content/` — the canonical source (ARCHITECTURE.md §1).
pub const CONTENT_DIR: &str = "content";
/// `content/catalog.json` (§5.1).
pub const CATALOG_FILE: &str = "catalog.json";
/// `plugins/<plugin-id>/` — the generated native package (§1).
pub const PLUGINS_DIR: &str = "plugins";
/// `dist/` — the generated projections (§1; committed, drift-gated).
pub const DIST_DIR: &str = "dist";
/// `dist/claude/` — the `.claude-plugin` projection.
pub const DIST_CLAUDE_DIR: &str = "claude";
/// `dist/codex/` — the `.codex-plugin` projection.
pub const DIST_CODEX_DIR: &str = "codex";
/// `crates/omm/Cargo.toml` — where the shipped version and description live.
pub const OMM_CLI_MANIFEST: &str = "crates/omm/Cargo.toml";

/// The repository layout (ARCHITECTURE.md §1), rooted at a canonical path.
/// Every path this crate reads or writes is derived from here and checked to
/// stay inside `root`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    /// Canonical repository root.
    pub root: PathBuf,
}

impl Repo {
    /// Canonicalize `root` (it must exist).
    pub fn new(root: &Path) -> Result<Repo> {
        Ok(Repo {
            root: omm_host::fsx::canonicalize(root)?,
        })
    }

    /// The repository this crate was built in — `CARGO_MANIFEST_DIR/../..`.
    /// For tests and the dev-only `omm build`; a shipped binary never assumes
    /// it runs inside the repo.
    pub fn from_cargo_manifest_dir() -> Result<Repo> {
        Repo::new(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".."))
    }

    /// `content/`.
    pub fn content_dir(&self) -> PathBuf {
        self.root.join(CONTENT_DIR)
    }
    /// `content/catalog.json`.
    pub fn catalog_path(&self) -> PathBuf {
        self.content_dir().join(CATALOG_FILE)
    }
    /// `plugins/<plugin_id>/`.
    pub fn native_package_dir(&self, plugin_id: &str) -> PathBuf {
        self.root.join(PLUGINS_DIR).join(plugin_id)
    }
    /// `plugins/<plugin_id>` as the catalog's relative `install.source`.
    pub fn native_package_rel(plugin_id: &str) -> String {
        format!("{PLUGINS_DIR}/{plugin_id}")
    }
    /// `dist/claude/`.
    pub fn dist_claude_dir(&self) -> PathBuf {
        self.root.join(DIST_DIR).join(DIST_CLAUDE_DIR)
    }
    /// `dist/claude` as the relative catalog source.
    pub fn dist_claude_rel() -> String {
        format!("{DIST_DIR}/{DIST_CLAUDE_DIR}")
    }
    /// `dist/codex/`.
    pub fn dist_codex_dir(&self) -> PathBuf {
        self.root.join(DIST_DIR).join(DIST_CODEX_DIR)
    }
    /// `dist/codex` as the relative catalog source.
    pub fn dist_codex_rel() -> String {
        format!("{DIST_DIR}/{DIST_CODEX_DIR}")
    }
    /// The three catalog files in the host's probe order
    /// (`hr::MARKETPLACE_PROBE_ORDER`): native root, Codex schema, Claude schema.
    pub fn marketplace_paths(&self) -> [PathBuf; 3] {
        let [a, b, c] = hr::MARKETPLACE_PROBE_ORDER;
        [self.root.join(a), self.root.join(b), self.root.join(c)]
    }
    /// `marketplace.json` — the native catalog Muse reads.
    pub fn marketplace_native_path(&self) -> PathBuf {
        self.root.join(hr::MARKETPLACE_PROBE_ORDER[0])
    }
    /// `.agents/plugins/marketplace.json` — the Codex-schema catalog.
    pub fn marketplace_codex_path(&self) -> PathBuf {
        self.root.join(hr::MARKETPLACE_PROBE_ORDER[1])
    }
    /// `.claude-plugin/marketplace.json` — the Claude-schema catalog.
    pub fn marketplace_claude_path(&self) -> PathBuf {
        self.root.join(hr::MARKETPLACE_PROBE_ORDER[2])
    }
    /// `crates/omm/Cargo.toml`.
    pub fn omm_cli_manifest(&self) -> PathBuf {
        self.root.join(OMM_CLI_MANIFEST)
    }
    /// A path inside the repo as a `/`-separated relative string.
    pub fn relative(&self, path: &Path) -> String {
        path.strip_prefix(&self.root)
            .map(posix)
            .unwrap_or_else(|_| path.display().to_string())
    }
}

/// A relative path as a `/`-separated string (lossy on non-UTF-8 components).
pub fn posix(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// `canonicalize(child)` must start with `root` (itself canonical). The
/// containment primitive every reader and writer here goes through.
///
/// Existing paths only: a missing child is `Err` by design — callers pass
/// paths they just created or stat-ed (overlay custom entries, generated
/// parents). For ledger-relative or may-be-missing paths use
/// `omm_ledger::Bases::resolve` (writes) or
/// `omm_doctor::ledger::contained_path` (read-only D13); the parity test
/// `crates/omm/tests/containment_parity.rs` locks the agreement.
pub fn contained(root: &Path, child: &Path) -> Result<PathBuf> {
    let canonical = omm_host::fsx::canonicalize(child)?;
    if canonical.starts_with(root) {
        Ok(canonical)
    } else {
        Err(ManifestError::Containment {
            path: child.to_path_buf(),
            root: root.to_path_buf(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_layout_follows_the_probe_order() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Repo::new(dir.path()).unwrap();
        let [native, codex, claude] = repo.marketplace_paths();
        assert!(native.ends_with("marketplace.json"));
        assert!(codex.ends_with(".agents/plugins/marketplace.json"));
        assert!(claude.ends_with(".claude-plugin/marketplace.json"));
        assert_eq!(repo.marketplace_native_path(), native);
        assert_eq!(Repo::native_package_rel("omm"), "plugins/omm");
        assert_eq!(Repo::dist_claude_rel(), "dist/claude");
        assert_eq!(Repo::dist_codex_rel(), "dist/codex");
        assert_eq!(
            repo.relative(&repo.root.join("content").join("x")),
            "content/x"
        );
        assert!(repo.native_package_dir("omm").ends_with("plugins/omm"));
    }

    #[test]
    fn containment_refuses_an_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        std::fs::write(root.join("inside"), b"x").unwrap();
        assert!(contained(&root, &root.join("inside")).is_ok());
        assert!(matches!(
            contained(&root, &root.join("..")),
            Err(ManifestError::Containment { .. })
        ));
        assert!(contained(&root, &root.join("missing")).is_err());
    }
}
