//! Where the content comes from: a checkout of this repository holding
//! `content/` (the canonical source, ARCHITECTURE.md §5.1) and — for the
//! bundle path — the generated `marketplace.json` + `plugins/<pid>/` that
//! `muse plugins marketplace add` reads (§5.2; `docs/experiments/
//! marketplace-precedence.md` §6.1: the root native catalog is the only one
//! the host opens).
//!
//! Resolution order: `--source <path>` → `$OMM_SOURCE` → the checkout the
//! ledger's `muse-marketplace` registration names, when it still holds a
//! catalog (an install from a copy of the repository: doctor's `omm install
//! --reinstall` / `omm install` / `omm update` then heal from where the
//! plugin was installed — Gate 1: they refused with "marketplace `omm` is
//! configured from <copy> but this install's source is <repo>") → the
//! checkout this binary was built from (`Repo::from_cargo_manifest_dir`,
//! the dev path; a shipped binary must be told). Phase 1 takes local paths
//! only; a git URL source is PLAN.md 2.5's `install.sh` work.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use omm_ledger::{store, Registration};
use omm_manifest::budget::{self, BudgetReport};
use omm_manifest::catalog::Catalog;
use omm_manifest::content::Content;
use omm_manifest::{version, Repo};

use crate::error::{OmmError, Result};

/// The environment variable naming the checkout when no `--source` is given.
pub const SOURCE_ENV: &str = "OMM_SOURCE";

/// How the source was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Flag,
    Env,
    /// The checkout the ledger's marketplace registration names.
    Ledger,
    BuildCheckout,
}

impl Origin {
    pub fn label(self) -> &'static str {
        match self {
            Origin::Flag => "--source",
            Origin::Env => SOURCE_ENV,
            Origin::Ledger => "ledger",
            Origin::BuildCheckout => "build checkout",
        }
    }
}

/// A loaded content source.
#[derive(Debug)]
pub struct Source {
    pub repo: Repo,
    pub catalog: Catalog,
    pub content: Content,
    /// `crates/omm/Cargo.toml` of the source — the `source_version` of every
    /// entry it produces and the `updates/<version>` label.
    pub version: String,
    pub description: String,
    pub origin: Origin,
}

impl Source {
    /// Resolve and load (catalog + content; content problems are errors:
    /// `omm lint` names them) without a ledger to consult.
    pub fn resolve(explicit: Option<&Path>) -> Result<Source> {
        Source::load(locate(explicit, None)?)
    }

    /// [`Source::resolve`] with the ledger under `omm_root` as the third
    /// candidate (see the module docs).
    pub fn resolve_with_ledger(explicit: Option<&Path>, omm_root: &Path) -> Result<Source> {
        let hint = registered_checkout(omm_root);
        Source::load(locate(explicit, hint.as_deref())?)
    }

    fn load((root, origin): (PathBuf, Origin)) -> Result<Source> {
        let repo = Repo::new(&root)?;
        let catalog_path = repo.catalog_path();
        if !catalog_path.is_file() {
            return Err(OmmError::Usage(format!(
                "{} ({}) is not a content checkout: {} is missing; pass --source <checkout> or set {SOURCE_ENV}",
                repo.root.display(),
                origin.label(),
                catalog_path.display()
            )));
        }
        let catalog = Catalog::load(&catalog_path)?;
        let content = Content::load(&catalog, &repo.content_dir())?;
        content.require_clean()?;
        let v = version::omm_version(&repo)?;
        Ok(Source {
            repo,
            catalog,
            content,
            version: v.version,
            description: v.description,
            origin,
        })
    }

    /// The marketplace root `muse plugins marketplace add` reads: the checkout.
    pub fn marketplace_root(&self) -> &Path {
        &self.repo.root
    }

    /// The bundle path needs the generated catalog and package (`omm build`).
    pub fn require_built(&self, plugin_id: &str) -> Result<()> {
        let catalog = self.repo.marketplace_native_path();
        let package = self.repo.native_package_dir(plugin_id);
        if !catalog.is_file() || !package.is_dir() {
            return Err(OmmError::Usage(format!(
                "{} has no generated marketplace ({} / {}); run `omm build` in that checkout, or install with --no-plugin",
                self.repo.root.display(),
                catalog.display(),
                package.display()
            )));
        }
        Ok(())
    }

    /// The R18 catalog estimate of this content.
    pub fn budget(&self) -> Budget {
        Budget::from_report(budget::estimate(&self.content))
    }
}

/// The checkout the ledger's `muse-marketplace` registration names, when
/// it is a directory holding `content/catalog.json` now (a moved checkout
/// is no source; D10 names it). The file is read leniently — a ledger this
/// binary cannot load, or none, is simply no hint; nothing is written
/// (`store::load` would quarantine a corrupt file, which is the session's
/// job, loudly).
pub fn registered_checkout(omm_root: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(store::ledger_path(omm_root)).ok()?;
    let doc: Value = serde_json::from_slice(&bytes).ok()?;
    let root = doc
        .get("registrations")?
        .as_array()?
        .iter()
        .filter_map(|r| serde_json::from_value::<Registration>(r.clone()).ok())
        .find_map(|r| match r {
            Registration::MuseMarketplace { source, .. } => Some(PathBuf::from(source)),
            _ => None,
        })?;
    let repo = Repo::new(&root).ok()?;
    repo.catalog_path().is_file().then_some(root)
}

fn locate(explicit: Option<&Path>, ledger_hint: Option<&Path>) -> Result<(PathBuf, Origin)> {
    if let Some(p) = explicit {
        return Ok((p.to_path_buf(), Origin::Flag));
    }
    if let Some(v) = std::env::var_os(SOURCE_ENV).filter(|v| !v.is_empty()) {
        return Ok((PathBuf::from(v), Origin::Env));
    }
    if let Some(p) = ledger_hint {
        return Ok((p.to_path_buf(), Origin::Ledger));
    }
    let build = Repo::from_cargo_manifest_dir().map_err(|e| {
        OmmError::Usage(format!(
            "no content source: pass --source <checkout> or set {SOURCE_ENV} (the build checkout is unavailable: {e})"
        ))
    })?;
    Ok((build.root, Origin::BuildCheckout))
}

/// The catalog bytes this content costs (R18) and the bytes/4 token figure
/// ARCHITECTURE.md §6 calls the documented heuristic.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Budget {
    pub bytes_full: u64,
    pub bytes_first_sentence: u64,
    pub limit_full: u64,
    pub limit_first_sentence: u64,
    pub entries: usize,
}

impl Budget {
    fn from_report(r: BudgetReport) -> Budget {
        Budget {
            bytes_full: r.total_full,
            bytes_first_sentence: r.total_first_sentence,
            limit_full: r.limit_full,
            limit_first_sentence: r.limit_first_sentence,
            entries: r.entries.len(),
        }
    }

    /// `bytes / 4`, the documented heuristic.
    pub fn tokens(bytes: u64) -> u64 {
        bytes / 4
    }

    pub fn within(&self) -> bool {
        self.bytes_full <= self.limit_full && self.bytes_first_sentence <= self.limit_first_sentence
    }

    /// One line for the terminal.
    pub fn render(&self) -> String {
        format!(
            "catalog budget: {} skill entr{}, {} B full / {} B first_sentence of {} / {} (≈ {} / {} tokens, bytes/4 heuristic){}",
            self.entries,
            if self.entries == 1 { "y" } else { "ies" },
            self.bytes_full,
            self.bytes_first_sentence,
            self.limit_full,
            self.limit_first_sentence,
            Budget::tokens(self.bytes_full),
            Budget::tokens(self.bytes_first_sentence),
            if self.within() { "" } else { " — OVER BUDGET (R18)" }
        )
    }

    pub fn to_json(&self) -> Value {
        json!({
            "entries": self.entries,
            "bytes_full": self.bytes_full,
            "bytes_first_sentence": self.bytes_first_sentence,
            "limit_full": self.limit_full,
            "limit_first_sentence": self.limit_first_sentence,
            "tokens_full": Budget::tokens(self.bytes_full),
            "tokens_first_sentence": Budget::tokens(self.bytes_first_sentence),
            "tokens_heuristic": "bytes/4",
            "within_budget": self.within(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_line_and_json_use_the_bytes_over_four_heuristic() {
        let b = Budget {
            bytes_full: 4_693,
            bytes_first_sentence: 4_625,
            limit_full: 21_542,
            limit_first_sentence: 27_168,
            entries: 12,
        };
        assert!(b.within());
        let line = b.render();
        assert!(line.contains("4693 B full / 4625 B first_sentence of 21542 / 27168"));
        assert!(line.contains("≈ 1173 / 1156 tokens, bytes/4 heuristic"));
        assert!(!line.contains("OVER"));
        let j = b.to_json();
        assert_eq!(j["tokens_full"], 1173);
        assert_eq!(j["tokens_heuristic"], "bytes/4");
        let over = Budget {
            bytes_full: 30_000,
            ..b
        };
        assert!(!over.within());
        assert!(over.render().contains("OVER BUDGET"));
    }

    #[test]
    fn the_ledger_names_a_checkout_only_while_it_holds_a_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let omm_root = tmp.path().join("omm");
        assert_eq!(registered_checkout(&omm_root), None, "no ledger");
        std::fs::create_dir_all(&omm_root).unwrap();
        let ledger = store::ledger_path(&omm_root);
        std::fs::write(&ledger, b"{corrupt").unwrap();
        assert_eq!(
            registered_checkout(&omm_root),
            None,
            "corrupt: no hint, no quarantine"
        );
        assert!(
            ledger.is_file(),
            "read-only: the corrupt file stays for the session to quarantine"
        );
        let checkout = tmp.path().join("checkout");
        let write = |source: &Path| {
            std::fs::write(
                &ledger,
                json!({"schema_version": 1, "registrations": [
                    {"kind": "muse-plugin", "id": "omm", "package_sha256": "", "generation_path": "", "approved": []},
                    {"kind": "muse-marketplace", "name": "ohmy", "source": source}
                ]})
                .to_string(),
            )
            .unwrap();
        };
        write(&checkout);
        assert_eq!(
            registered_checkout(&omm_root),
            None,
            "a moved checkout is no source"
        );
        std::fs::create_dir_all(checkout.join("content")).unwrap();
        std::fs::write(checkout.join("content").join("catalog.json"), b"{}").unwrap();
        assert_eq!(registered_checkout(&omm_root), Some(checkout.clone()));
        assert_eq!(
            locate(None, Some(&checkout)).unwrap(),
            (checkout.clone(), Origin::Ledger)
        );
        assert_eq!(
            locate(Some(tmp.path()), Some(&checkout)).unwrap(),
            (tmp.path().to_path_buf(), Origin::Flag),
            "--source wins"
        );
        assert_eq!(Origin::Ledger.label(), "ledger");
    }

    #[test]
    fn an_explicit_source_without_a_catalog_is_a_usage_error() {
        let dir = tempfile::tempdir().unwrap();
        match Source::resolve(Some(dir.path())) {
            Err(OmmError::Usage(msg)) => assert!(msg.contains("catalog.json"), "{msg}"),
            other => panic!("{other:?}"),
        }
        assert!(Source::resolve(Some(&dir.path().join("missing"))).is_err());
    }
}
