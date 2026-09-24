//! The uninstall planner (R4): a ledger traversal in reverse, deepest-first.
//!
//! [`plan`] is pure apart from reading on-disk hashes. It produces:
//!
//! * `remove` — file entries whose on-disk sha equals the ledger's (or, with
//!   `force`, any regular file), ordered deepest-first;
//! * `preserve` — file entries the user edited (sha differs), or that are a
//!   symlink / directory / socket now (the R2 sentinel: never ours, never
//!   removed even with `force`), or whose path canonicalizes outside its
//!   base through a symlinked ancestor (named, never followed — one planted
//!   symlink must not block uninstalling everything else), or whose
//!   ancestor is a symlink INSIDE the base (`Resolved::via_symlink` — the
//!   same sentinel: omm never wrote a symlink, so what lies behind one is
//!   the user's even when byte-identical; Gate 1 decision C), plus every
//!   non-file entry, plus a settings key or trust entry the user edited
//!   since omm wrote it (its registration records `value`; the key is
//!   restored only while it still holds that, unless `force`);
//! * `rules` — the personal rules file (`Kind::Rules`, when
//!   [`Options::rules`] names the region markers; [`crate::rules`] has the
//!   file model): restored byte for byte to the pre-existing file when omm's
//!   block is all that was added to it since, else rewritten with exactly
//!   omm's contribution — the managed block, its marker lines, the
//!   template's recorded frame — taken away and every other byte kept, so
//!   the managed block (which names a plugin that is being removed) never
//!   outlives the plugin and nothing the user wrote outside the markers is
//!   lost (Gate 1 round 5); removed only when omm created the file and
//!   nothing else remains (a seed byte for byte as written included: its
//!   user region is then the template's own default text); preserved when
//!   the managed region was edited by hand or the markers removed;
//! * `missing` — file entries with nothing on disk;
//! * `shared` — a shared file omm found before it wrote (`settings.json`,
//!   `trust.json`; its entry carries `prior.original`, [`crate::shared`]):
//!   after every host step and key restore it goes back to those bytes and
//!   that mode when the document then equals the pre-omm one — the user's
//!   untyped keys, which the host's rewrites destroy, merged in first —
//!   else the untyped keys are merged back and the mode re-applied;
//! * `dropped` — a settings-key or trust registration whose shared file is
//!   gone when the plan is made: nothing to restore into, the registration
//!   is dropped and said so (Gate 1: restoring into a file the user had
//!   deleted recreated `{"schema_version":1}`); a plugin, marketplace or
//!   managed skill the host no longer holds when the plan is made
//!   ([`Options::host`]): already gone, the registration is dropped with
//!   the host's reason (Gate 1 decision B — the undo is idempotent, and
//!   [`HostUndoer`] treats the host's `not installed` / `not configured`
//!   / `skill not installed` as done too, for the race between plan and
//!   apply); and a settings key that is a required structural member of
//!   its object (settings-keys.json `structural`, `permissions.
//!   schema_version`) while the object still holds members omm did not
//!   write: kept and named, never stripped (Gate 1 decision D — the host
//!   refuses the whole object without it); such a leaf is restored last
//!   among the settings keys so it goes only with the last other member;
//! * `host_steps` — the undo of every registration in **reverse** order
//!   (`muse plugins remove <id> --delete-data`, `muse plugins marketplace
//!   remove <name>` — docs/experiments/marketplace-precedence.md §6.4; a
//!   settings key restored to its `prior` by targeted patch and a trust entry
//!   put back — 00-DECISION.md §2.2 "never restore settings.json wholesale"),
//!   preceded by `muse skills uninstall <id>` for every skill the host's
//!   managed store installed (`research/musecode/cli-surface.md` §3:
//!   `muse skills uninstall <skill-id> [--keep-files] [--json]`), all-or-nothing
//!   per skill: a skill with one edited file is preserved whole;
//! * `omm_state` — what to remove under `$OMM/` (everything omm owns; the
//!   user's `custom/` overlay and unknown names are kept unless `force`);
//! * `residue` — the host's own leftovers outside every base, NAMED and never
//!   touched (host-reality.md "residue outside XDG": the `getpwuid`
//!   `session-name-authority/` dir, the per-uid runtime dir);
//! * `refused` — entries that resolve onto `/`, `$HOME` or a base root;
//!   [`apply`] refuses the whole plan while any exist.
//!
//! [`apply`] runs host steps first (the ones most likely to fail — abort
//! before any file is gone), then the shared files' originals, then the
//! shared files omm created (or that a host step recreated empty — removed
//! again), then the rules file, then unlinks deepest-first
//! and prunes the empty directories it left between each file and its base
//! root, then `$OMM/` state, then the ledger last. A failing step is
//! recorded and the ledger is saved with everything still undone, so a
//! rerun continues.
//!
//! The dotted settings-key helpers (`segments`, `still_ours`, …) live in
//! [`crate::uninstall_keys`].

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omm_host::fsx;
use omm_host::settings::{self, CommitOptions, PatchOp, SettingsDoc};
use omm_host::trust::TrustStore;
use omm_host::{probe, Invoker, Roots};
use serde_json::Value;

use crate::audit::{self, Audit, Event, ACTION_UNINSTALL};
use crate::containment::{Bases, State};
use crate::error::{LedgerError, Result};
use crate::hash::{self, Observed};
use crate::rules::{self, Markers};
use crate::schema::{Base, Class, Entry, EntryKey, Kind, Ledger, Mechanism, Registration};
use crate::shared::{self, Original};
use crate::store;
#[cfg(test)]
use crate::uninstall_keys::remove_leaf_and_prune;
use crate::uninstall_keys::{
    dotted_value, is_structural_leaf, predict_structural_keep, segments, shared_file_key,
    still_ours,
};

/// The `prior` key on a `rules` entry holding the sha of the managed region
/// omm wrote — how update tells "the user edited only the user region" from
/// "the user edited what omm owns" without the ancestor bytes.
pub const RULES_PRIOR_MANAGED_SHA: &str = "managed_sha256";
/// The `prior` key on a `rules` entry recording the unledgered file the
/// seed replaced and folded into the user region ([`replaced_prior`]), so
/// uninstall can put it back byte for byte.
pub const RULES_PRIOR_REPLACED: &str = "replaced";
/// `replaced.sha256` — the replaced file's content hash.
pub const REPLACED_SHA256: &str = "sha256";
/// `replaced.backup` — the verified copy under `$OMM/snapshots/` (rolling,
/// so never relied on; `null` under a dry run).
pub const REPLACED_BACKUP: &str = "backup";
/// `replaced.text` — the replaced bytes when they are UTF-8.
pub const REPLACED_TEXT: &str = "text";
/// `replaced.hex` — the replaced bytes, hex-encoded, when they are not.
pub const REPLACED_HEX: &str = "hex";
/// The `prior` key on a `rules` entry recording the template's text outside
/// the managed block that the seed contributed ([`crate::rules::Frame`]),
/// so uninstall takes exactly that away and keeps the rest.
pub const RULES_PRIOR_FRAME: &str = "frame";

/// The `prior` of a `rules` entry: `{managed_sha256, replaced, frame}`.
pub fn rules_prior(
    managed_sha256: &str,
    replaced: Option<Value>,
    frame: Option<&rules::Frame>,
) -> Value {
    let mut m = serde_json::Map::new();
    m.insert(
        RULES_PRIOR_MANAGED_SHA.to_string(),
        Value::String(managed_sha256.to_string()),
    );
    m.insert(
        RULES_PRIOR_REPLACED.to_string(),
        replaced.unwrap_or(Value::Null),
    );
    m.insert(
        RULES_PRIOR_FRAME.to_string(),
        frame.map(rules::Frame::to_json).unwrap_or(Value::Null),
    );
    Value::Object(m)
}

/// The `replaced` record of a pre-existing rules file: its hash, its backup
/// path and its exact bytes (as `text` when UTF-8, else `hex`). The bytes
/// live in the ledger because the snapshot backup rolls away (keep 5, one
/// directory per settings/trust write) long before an uninstall.
pub fn replaced_prior(sha256: &str, backup: Option<&Path>, original: &[u8]) -> Value {
    let mut m = serde_json::Map::new();
    m.insert(
        REPLACED_SHA256.to_string(),
        Value::String(sha256.to_string()),
    );
    m.insert(
        REPLACED_BACKUP.to_string(),
        backup
            .map(|b| Value::String(b.display().to_string()))
            .unwrap_or(Value::Null),
    );
    match std::str::from_utf8(original) {
        Ok(text) => m.insert(REPLACED_TEXT.to_string(), Value::String(text.to_string())),
        Err(_) => m.insert(
            REPLACED_HEX.to_string(),
            Value::String(hex::encode(original)),
        ),
    };
    Value::Object(m)
}

/// The exact bytes a `replaced` record carries, if any.
pub fn replaced_bytes(replaced: &Value) -> Option<Vec<u8>> {
    if let Some(text) = replaced.get(REPLACED_TEXT).and_then(Value::as_str) {
        return Some(text.as_bytes().to_vec());
    }
    replaced
        .get(REPLACED_HEX)
        .and_then(Value::as_str)
        .and_then(|h| hex::decode(h).ok())
}

/// A file to unlink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileStep {
    pub key: EntryKey,
    /// The contained realpath.
    pub abs: PathBuf,
    /// The canonical base root (directory pruning stops here).
    pub base_root: PathBuf,
    pub ledger_sha256: String,
    pub on_disk_sha256: String,
    pub mechanism: Mechanism,
    /// `force` removed an edited file.
    pub forced: bool,
}

/// A file left in place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preserved {
    pub key: EntryKey,
    pub abs: Option<PathBuf>,
    pub reason: String,
    pub ledger_sha256: String,
    pub on_disk: Observed,
}

/// A ledgered file with nothing on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Missing {
    pub key: EntryKey,
    pub abs: PathBuf,
}

/// What happens to the rules file (see the module docs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RulesAction {
    /// Write the pre-existing file back byte for byte.
    Restore { bytes: Vec<u8> },
    /// The managed block, its marker lines and the recorded frame go;
    /// every other byte stays.
    Rewrite { bytes: Vec<u8> },
}

/// The rules file, rewritten rather than unlinked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RulesStep {
    pub key: EntryKey,
    /// The contained realpath.
    pub abs: PathBuf,
    pub base_root: PathBuf,
    pub ledger_sha256: String,
    pub on_disk_sha256: String,
    pub action: RulesAction,
}

fn action_bytes(action: &RulesAction) -> &[u8] {
    match action {
        RulesAction::Restore { bytes } | RulesAction::Rewrite { bytes } => bytes,
    }
}

impl RulesStep {
    /// The bytes that land.
    pub fn bytes(&self) -> &[u8] {
        action_bytes(&self.action)
    }
    /// The preview annotation.
    pub fn label(&self) -> &'static str {
        match self.action {
            RulesAction::Restore { .. } => "restored to the file that was there before omm",
            RulesAction::Rewrite { .. } => {
                "rewritten: managed block removed, everything else kept byte for byte"
            }
        }
    }
}

/// A shared file omm created (`settings-patch` / `trust-merge` entry with
/// `class: seeded`, see `schema`): unlinked after the restores if it then
/// holds nothing but `schema_version` and empty objects. Also a shared file
/// of either class that was ABSENT when the plan was made (the user removed
/// it after install): a host step of this uninstall may recreate it empty
/// (`plugins remove --delete-data` writes `{"schema_version":1}`), and that
/// is removed again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedFile {
    pub key: EntryKey,
    pub abs: PathBuf,
    /// Not on disk when the plan was made.
    pub absent_at_plan: bool,
}

/// A shared file omm found before it wrote — its entry carries the pre-omm
/// bytes and mode ([`crate::shared`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SharedStep {
    pub key: EntryKey,
    /// The contained realpath.
    pub abs: PathBuf,
    pub mechanism: Mechanism,
    pub original: Original,
    /// Members the host does not type, present when the plan was made (the
    /// user added them after install); merged back after the host's rewrite.
    pub untyped_now: Vec<(Vec<String>, Value)>,
}

impl SharedStep {
    /// The preview annotation.
    pub fn label(&self) -> &'static str {
        "back to its pre-omm bytes and mode once every key is restored; else its untyped keys merged back"
    }
}

/// A registration whose shared file is gone: nothing to restore into.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DroppedStep {
    pub step: UndoStep,
    pub reason: String,
}

/// What [`apply`] did with one [`SharedStep`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedOutcome {
    /// The document equalled the pre-omm one: the recorded bytes and mode landed.
    Restored,
    /// The document differs (edited since): the untyped keys named were merged back, the mode re-applied.
    Merged { keys: Vec<String> },
    /// The document differs and nothing was missing: left as it stands, the mode re-applied.
    Kept,
    /// Not on disk any more.
    Gone,
}

/// An entry the plan will not act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub key: EntryKey,
    pub reason: String,
}

/// One host-side undo.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoStep {
    /// `muse skills uninstall <id> --json`.
    SkillsUninstall { id: String },
    /// `muse plugins remove <id> --delete-data --json`.
    PluginRemove { id: String },
    /// `muse plugins marketplace remove <name> --json`.
    MarketplaceRemove { name: String },
    /// Targeted patch of one typed key back to `prior` (`None` removes it).
    RestoreSettingsKey { path: String, prior: Option<Value> },
    /// `trust.json` `projects.<project>` back to `prior` (`None` removes it).
    RestoreTrust {
        project: String,
        prior: Option<Value>,
    },
}

impl UndoStep {
    pub fn label(&self) -> String {
        match self {
            UndoStep::SkillsUninstall { id } => format!("muse skills uninstall {id}"),
            UndoStep::PluginRemove { id } => format!("muse plugins remove {id} --delete-data"),
            UndoStep::MarketplaceRemove { name } => {
                format!("muse plugins marketplace remove {name}")
            }
            UndoStep::RestoreSettingsKey { path, prior } => match prior {
                Some(_) => format!("restore settings key {path} to its prior value"),
                None => format!("remove settings key {path} (absent before omm)"),
            },
            UndoStep::RestoreTrust { project, prior } => match prior {
                Some(_) => format!("restore trust entry {project} to its prior value"),
                None => format!("remove trust entry {project} (absent before omm)"),
            },
        }
    }

    /// The registration this step undoes, if it came from one.
    fn from_registration(reg: &Registration) -> UndoStep {
        match reg {
            Registration::MusePlugin { id, .. } => UndoStep::PluginRemove { id: id.clone() },
            Registration::MuseMarketplace { name, .. } => {
                UndoStep::MarketplaceRemove { name: name.clone() }
            }
            Registration::SettingsKey { path, prior, .. } => UndoStep::RestoreSettingsKey {
                path: path.clone(),
                prior: prior.clone(),
            },
            Registration::Trust { project, prior, .. } => UndoStep::RestoreTrust {
                project: project.clone(),
                prior: prior.clone(),
            },
        }
    }
}

/// What to do under `$OMM/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmmState {
    pub root: PathBuf,
    /// Children to remove (files or whole directories), the ledger excluded.
    pub remove: Vec<PathBuf>,
    /// Children kept (the user's overlay, unknown names).
    pub keep: Vec<PathBuf>,
    /// The ledger file — removed last.
    pub ledger: PathBuf,
    /// Remove the root directory itself once empty.
    pub remove_root_if_empty: bool,
}

/// Names under `$OMM/` that are omm's own state (ARCHITECTURE.md §2) — not
/// assets, so R8 does not apply. Everything else is the user's unless `force`.
pub const OMM_STATE_NAMES: [&str; 7] = [
    audit::AUDIT_FILE,
    "config.json",
    "install-provenance.json",
    omm_host::paths::OMM_SNAPSHOTS_DIR,
    omm_host::paths::OMM_LOCKS_DIR,
    crate::reconcile::UPDATES_DIR,
    store::LEDGER_FILE,
];
/// The user's overlay (R7): kept on uninstall unless `force`.
pub const OMM_OVERLAY_DIR: &str = "custom";

/// Planner options.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    /// Remove edited files and the whole `$OMM/` tree too; restore every
    /// settings key and trust entry whatever it holds now.
    pub force: bool,
    /// The region markers of the rules file (`content/rules/AGENTS.md.tmpl`,
    /// passed by the CLI); without them a `rules` entry is a plain copy.
    pub rules: Option<Markers>,
    /// The host's `settings.json` (`Roots::settings_file`), read to compare
    /// each registered key with the `value` omm wrote; without it every key
    /// is restored unconditionally.
    pub settings_file: Option<PathBuf>,
    /// The host's `trust.json` (`Roots::trust_file`); same rule.
    pub trust_file: Option<PathBuf>,
    /// What the host holds now (the CLI asks `plugins list`, `marketplace
    /// list`, `skills list --source user` before planning): a registered
    /// plugin, marketplace or managed skill the host no longer has is
    /// dropped as already gone instead of run and failed. `None` (library
    /// callers, tests) plans every registration.
    pub host: Option<HostView>,
}

/// What the host holds of the kinds omm registers, as [`Options::host`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostView {
    /// Installed plugin ids (`plugins list`).
    pub plugins: BTreeSet<String>,
    /// Configured marketplace names (`plugins marketplace list`).
    pub marketplaces: BTreeSet<String>,
    /// Managed-store skill ids (`skills list --source user`).
    pub skills: BTreeSet<String>,
}

/// The two-section preview plus everything [`apply`] needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub force: bool,
    /// Deepest-first.
    pub remove: Vec<FileStep>,
    pub preserve: Vec<Preserved>,
    pub missing: Vec<Missing>,
    /// The rules file, restored or rewritten in place.
    pub rules: Vec<RulesStep>,
    /// Shared files omm created; removed after the restores when empty.
    pub created: Vec<CreatedFile>,
    /// Shared files omm found before it wrote; back to their originals.
    pub shared: Vec<SharedStep>,
    /// In execution order (reverse of registration).
    pub host_steps: Vec<UndoStep>,
    /// Registrations with no file to restore into; dropped and named.
    pub dropped: Vec<DroppedStep>,
    pub omm_state: OmmState,
    pub residue: Vec<PathBuf>,
    pub refused: Vec<Refusal>,
}

impl Plan {
    /// The `✗ remove / ✓ preserve` preview text.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("✗ remove ({})\n", self.remove.len()));
        for s in &self.remove {
            out.push_str(&format!(
                "  {}{}\n",
                s.abs.display(),
                if s.forced { "  (edited; --force)" } else { "" }
            ));
        }
        for p in &self.omm_state.remove {
            out.push_str(&format!("  {}\n", p.display()));
        }
        for r in &self.rules {
            out.push_str(&format!("  {}  ({})\n", r.abs.display(), r.label()));
        }
        for c in &self.created {
            out.push_str(&format!(
                "  {}  ({})\n",
                c.abs.display(),
                if c.absent_at_plan {
                    "absent now; removed again if a host step recreates it empty"
                } else {
                    "created by omm; removed if empty after the restores"
                }
            ));
        }
        for s in &self.shared {
            out.push_str(&format!("  {}  ({})\n", s.abs.display(), s.label()));
        }
        out.push_str(&format!(
            "  {}  (the ledger, last)\n",
            self.omm_state.ledger.display()
        ));
        for s in &self.host_steps {
            out.push_str(&format!("  {}\n", s.label()));
        }
        for d in &self.dropped {
            out.push_str(&format!("  skipped: {} — {}\n", d.step.label(), d.reason));
        }
        out.push_str(&format!(
            "✓ preserve ({})\n",
            self.preserve.len() + self.omm_state.keep.len()
        ));
        for p in &self.preserve {
            out.push_str(&format!(
                "  {}  ({})\n",
                p.abs
                    .as_ref()
                    .map(|a| a.display().to_string())
                    .unwrap_or_else(|| p.key.to_string()),
                p.reason
            ));
        }
        for p in &self.omm_state.keep {
            out.push_str(&format!("  {}  (yours)\n", p.display()));
        }
        if !self.missing.is_empty() {
            out.push_str(&format!("– missing ({})\n", self.missing.len()));
            for m in &self.missing {
                out.push_str(&format!("  {}\n", m.abs.display()));
            }
        }
        if !self.residue.is_empty() {
            out.push_str("· residue kept (the host's, outside every base)\n");
            for r in &self.residue {
                out.push_str(&format!("  {}\n", r.display()));
            }
        }
        if !self.refused.is_empty() {
            out.push_str(&format!(
                "! refused ({}) — a refused root; nothing will be applied\n",
                self.refused.len()
            ));
            for r in &self.refused {
                out.push_str(&format!("  {}: {}\n", r.key, r.reason));
            }
        }
        out
    }
}

/// The skill id of a managed-store path `skills/<id>/…`, if it has that shape.
fn managed_skill_id(key: &EntryKey) -> Option<String> {
    let mut parts = key.path.components();
    match (parts.next(), parts.next(), parts.next()) {
        (Some("skills"), Some(id), Some(_)) if key.base == Base::MuseConfig => Some(id.to_string()),
        _ => None,
    }
}

/// What the planner decided for a `rules` entry that is ours to act on.
enum RulesDecision {
    Restore(Vec<u8>),
    Rewrite(Vec<u8>),
    Remove,
}

/// The rules decision (see the module docs and [`crate::rules`]): `None`
/// when the managed region was edited by hand or the markers removed (the
/// file is not what omm wrote — preserved, unless `force`: a pre-existing
/// file then comes back, a seed goes).
fn decide_rules(
    markers: &Markers,
    entry: &Entry,
    on_disk: &[u8],
    on_disk_sha: &str,
    force: bool,
) -> Option<RulesDecision> {
    let text = String::from_utf8_lossy(on_disk);
    let parts = markers.parts(&text);
    let prior = entry.prior.as_ref();
    let recorded_managed = prior
        .and_then(|p| p.get(RULES_PRIOR_MANAGED_SHA))
        .and_then(Value::as_str);
    let frame = prior
        .and_then(|p| p.get(RULES_PRIOR_FRAME))
        .and_then(rules::Frame::from_json);
    let replaced = prior
        .and_then(|p| p.get(RULES_PRIOR_REPLACED))
        .and_then(replaced_bytes);
    let untouched = on_disk_sha == entry.sha256;
    let managed_intact = parts
        .as_ref()
        .map(|p| Some(hash::sha256_bytes(p.managed.as_bytes()).as_str()) == recorded_managed)
        .unwrap_or(false);
    if !(untouched || managed_intact || force) {
        return None;
    }
    let Some(parts) = parts else {
        // Forced and unmarked: nothing of ours to strip. A file that was
        // there before omm comes back; a seed goes.
        return Some(match replaced {
            Some(original) => RulesDecision::Restore(original),
            None => RulesDecision::Remove,
        });
    };
    // omm's contribution taken away (the block, its marker lines, the
    // recorded frame), every other byte kept — never the ledger sha, which
    // an install adopts the user's outside edits into (Gate 1 round 5: the
    // untouched-seed shortcut then removed a file the user had prepended a
    // title and appended a rule to).
    let rest = markers.without_managed(&parts, frame.as_ref());
    if let Some(original) = replaced {
        // A file was there before omm: byte for byte back when stripping
        // omm's block leaves exactly that file, or leaves nothing (the user
        // emptied everything omm did not own — the file that was there
        // before omm is the right end state, never a delete).
        if rest.as_bytes() == original.as_slice() || rest.trim().is_empty() {
            return Some(RulesDecision::Restore(original));
        }
        return Some(RulesDecision::Rewrite(rest.into_bytes()));
    }
    if rest.trim().is_empty() {
        // omm created the file and nothing else remains.
        Some(RulesDecision::Remove)
    } else {
        Some(RulesDecision::Rewrite(rest.into_bytes()))
    }
}

/// Put every structural settings-key restore after the last other
/// settings-key restore, so it runs once its siblings are gone (the object
/// is then omm's alone and may be emptied); the rest keeps its order.
fn order_structural_last(steps: Vec<UndoStep>) -> Vec<UndoStep> {
    let is_structural = |s: &UndoStep| matches!(s, UndoStep::RestoreSettingsKey { path, prior: None } if is_structural_leaf(&segments(path)));
    if !steps.iter().any(&is_structural) {
        return steps;
    }
    let (structural, mut rest): (Vec<UndoStep>, Vec<UndoStep>) =
        steps.into_iter().partition(is_structural);
    let Some(last) = rest
        .iter()
        .rposition(|s| matches!(s, UndoStep::RestoreSettingsKey { .. }))
    else {
        // The only settings-key restores are structural: leave them where
        // the reverse registration order put them.
        let mut all = rest;
        all.extend(structural);
        return all;
    };
    for (i, s) in structural.into_iter().enumerate() {
        rest.insert(last + 1 + i, s);
    }
    rest
}

/// Build the plan. Reads hashes, writes nothing.
pub fn plan(ledger: &Ledger, bases: &Bases, opts: Options) -> Result<Plan> {
    let mut remove = Vec::new();
    let mut preserve = Vec::new();
    let mut missing = Vec::new();
    let mut created = Vec::new();
    let mut shared_steps = Vec::new();
    let mut dropped = Vec::new();
    let mut rules_steps = Vec::new();
    let mut refused = Vec::new();
    // Managed skills: id → (any preserved?, file steps).
    let mut skills: BTreeMap<String, (bool, Vec<FileStep>)> = BTreeMap::new();

    for entry in &ledger.entries {
        let key = entry.key();
        if !entry.is_file() {
            let original = Original::from_prior(entry.prior.as_ref());
            match bases.resolve(entry.base, &entry.path) {
                Ok(r) if r.state == State::File => {
                    if entry.class == Class::Seeded {
                        created.push(CreatedFile {
                            key,
                            abs: r.path,
                            absent_at_plan: false,
                        });
                    } else if let Some(original) = original {
                        let untyped_now = if entry.mechanism == Mechanism::SettingsPatch {
                            fs::read(&r.path)
                                .ok()
                                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                                .and_then(|d| settings::untyped_members(&d).ok())
                                .unwrap_or_default()
                        } else {
                            Vec::new()
                        };
                        shared_steps.push(SharedStep {
                            key,
                            abs: r.path,
                            mechanism: entry.mechanism,
                            original,
                            untyped_now,
                        });
                    } else {
                        preserve.push(Preserved {
                            key,
                            abs: Some(r.path),
                            reason: format!(
                                "{} entry: the file is shared; the key is restored by its registration",
                                entry.mechanism
                            ),
                            ledger_sha256: entry.sha256.clone(),
                            on_disk: Observed::Missing,
                        });
                    }
                }
                Ok(r) if r.state == State::Missing => {
                    // Gone since install (the user removed it): nothing to
                    // restore into, whatever omm recorded; a host step may
                    // recreate it empty, and that is removed again.
                    created.push(CreatedFile {
                        key,
                        abs: r.path,
                        absent_at_plan: true,
                    });
                }
                Ok(r) => preserve.push(Preserved {
                    key,
                    abs: Some(r.path),
                    reason: "not a regular file now (R2 sentinel); never removed".into(),
                    ledger_sha256: entry.sha256.clone(),
                    on_disk: Observed::NonRegular(hash::sentinel("other", entry.path.as_str())),
                }),
                Err(e) if e.is_escape() => preserve.push(Preserved {
                    key,
                    abs: None,
                    reason: format!("{e}; never followed"),
                    ledger_sha256: entry.sha256.clone(),
                    on_disk: Observed::NonRegular(hash::sentinel("escape", entry.path.as_str())),
                }),
                Err(e) => {
                    if let Ok(root) = bases.root(entry.base) {
                        if bases.root_absent(entry.base) {
                            // The base root itself is gone (a kill between
                            // the ledger write and the file write): nothing
                            // to restore into — the Missing case one level
                            // up. A host step may recreate the file empty,
                            // and that is removed again.
                            created.push(CreatedFile {
                                key,
                                abs: entry.path.under(root),
                                absent_at_plan: true,
                            });
                            continue;
                        }
                    }
                    refused.push(Refusal {
                        key,
                        reason: e.to_string(),
                    })
                }
            }
            continue;
        }
        let skill = if entry.mechanism == Mechanism::MuseSkillsInstall {
            managed_skill_id(&key)
        } else {
            None
        };
        let resolved = match bases.resolve(entry.base, &entry.path) {
            Ok(r) => r,
            Err(e) if e.is_escape() => {
                // A symlinked ancestor now points out of the base: not what
                // omm wrote, never followed, and no reason to refuse the
                // rest of the plan.
                if let Some(id) = &skill {
                    skills.entry(id.clone()).or_insert((false, Vec::new())).0 = true;
                }
                preserve.push(Preserved {
                    key,
                    abs: None,
                    reason: format!("{e}; never followed"),
                    ledger_sha256: entry.sha256.clone(),
                    on_disk: Observed::NonRegular(hash::sentinel("escape", entry.path.as_str())),
                });
                continue;
            }
            Err(e) => {
                if let Ok(root) = bases.root(entry.base) {
                    if bases.root_absent(entry.base) {
                        // As above, for file entries: the ledger names a
                        // file whose whole base root is gone.
                        missing.push(Missing {
                            key,
                            abs: entry.path.under(root),
                        });
                        if let Some(id) = skill {
                            skills.entry(id).or_insert((false, Vec::new()));
                        }
                        continue;
                    }
                }
                refused.push(Refusal {
                    key,
                    reason: e.to_string(),
                });
                continue;
            }
        };
        if let Some(link) = &resolved.via_symlink {
            // An ancestor is a symlink now, inside the base: what lies
            // behind it is the user's even when byte-identical (omm never
            // wrote a symlink — R12); preserved, never followed, and a
            // managed skill kept whole (Gate 1 decision C).
            if let Some(id) = &skill {
                skills.entry(id.clone()).or_insert((false, Vec::new())).0 = true;
            }
            preserve.push(Preserved {
                key,
                abs: Some(resolved.path.clone()),
                reason: format!(
                    "an ancestor ({}) is a symlink now — not what omm wrote (R2 sentinel); never followed",
                    link.display()
                ),
                ledger_sha256: entry.sha256.clone(),
                on_disk: Observed::NonRegular(hash::sentinel("symlink", entry.path.as_str())),
            });
            continue;
        }
        match resolved.state {
            State::Missing => {
                missing.push(Missing {
                    key,
                    abs: resolved.path,
                });
                if let Some(id) = skill {
                    skills.entry(id).or_insert((false, Vec::new()));
                }
            }
            State::File => {
                let on_disk = hash::sha256_file(&resolved.path)?;
                let same = on_disk == entry.sha256;
                if entry.kind == Kind::Rules {
                    if let Some(markers) = &opts.rules {
                        let bytes = fs::read(&resolved.path)
                            .map_err(|e| LedgerError::io("read", &resolved.path, e))?;
                        match decide_rules(markers, entry, &bytes, &on_disk, opts.force) {
                            Some(RulesDecision::Restore(b)) | Some(RulesDecision::Rewrite(b))
                                if b == bytes =>
                            {
                                // Already exactly what would be written: the
                                // file is the user's alone; nothing to do.
                                preserve.push(Preserved {
                                    key,
                                    abs: Some(resolved.path),
                                    reason: "already holds the user's content alone".into(),
                                    ledger_sha256: entry.sha256.clone(),
                                    on_disk: Observed::Content(on_disk),
                                });
                            }
                            Some(RulesDecision::Restore(b)) => rules_steps.push(RulesStep {
                                key,
                                abs: resolved.path,
                                base_root: resolved.base_root,
                                ledger_sha256: entry.sha256.clone(),
                                on_disk_sha256: on_disk,
                                action: RulesAction::Restore { bytes: b },
                            }),
                            Some(RulesDecision::Rewrite(b)) => rules_steps.push(RulesStep {
                                key,
                                abs: resolved.path,
                                base_root: resolved.base_root,
                                ledger_sha256: entry.sha256.clone(),
                                on_disk_sha256: on_disk,
                                action: RulesAction::Rewrite { bytes: b },
                            }),
                            Some(RulesDecision::Remove) => remove.push(FileStep {
                                key,
                                abs: resolved.path,
                                base_root: resolved.base_root,
                                ledger_sha256: entry.sha256.clone(),
                                on_disk_sha256: on_disk,
                                mechanism: entry.mechanism,
                                forced: !same,
                            }),
                            None => preserve.push(Preserved {
                                key,
                                abs: Some(resolved.path),
                                reason: "the managed region was edited by hand or the markers removed — not what omm wrote; --force removes it".into(),
                                ledger_sha256: entry.sha256.clone(),
                                on_disk: Observed::Content(on_disk),
                            }),
                        }
                        continue;
                    }
                }
                if same || opts.force {
                    let step = FileStep {
                        key,
                        abs: resolved.path,
                        base_root: resolved.base_root,
                        ledger_sha256: entry.sha256.clone(),
                        on_disk_sha256: on_disk,
                        mechanism: entry.mechanism,
                        forced: !same,
                    };
                    match skill {
                        Some(id) => skills.entry(id).or_insert((false, Vec::new())).1.push(step),
                        None => remove.push(step),
                    }
                } else {
                    let p = Preserved {
                        key,
                        abs: Some(resolved.path),
                        reason: "edited since omm wrote it (sha differs); --force removes it"
                            .into(),
                        ledger_sha256: entry.sha256.clone(),
                        on_disk: Observed::Content(on_disk),
                    };
                    match skill {
                        Some(id) => {
                            skills.entry(id).or_insert((false, Vec::new())).0 = true;
                            preserve.push(p);
                        }
                        None => preserve.push(p),
                    }
                }
            }
            State::Dir | State::Symlink | State::Other => {
                let what = match resolved.state {
                    State::Dir => "a directory",
                    State::Symlink => "a symlink",
                    _ => "a non-regular entry",
                };
                if let Some(id) = &skill {
                    skills.entry(id.clone()).or_insert((false, Vec::new())).0 = true;
                }
                preserve.push(Preserved {
                    key,
                    abs: Some(resolved.path.clone()),
                    reason: format!("{what} is at the path now — not what omm wrote (R2 sentinel); never removed"),
                    ledger_sha256: entry.sha256.clone(),
                    on_disk: hash::observe(&resolved.path).unwrap_or(Observed::NonRegular(
                        hash::sentinel("other", entry.path.as_str()),
                    )),
                });
            }
        }
    }

    // Managed skills: all-or-nothing per id.
    let mut host_steps = Vec::new();
    for (id, (any_preserved, steps)) in skills {
        if any_preserved {
            for s in steps {
                preserve.push(Preserved {
                    key: s.key,
                    abs: Some(s.abs),
                    reason: format!("skill {id} has an edited file; kept whole"),
                    ledger_sha256: s.ledger_sha256,
                    on_disk: Observed::Content(s.on_disk_sha256),
                });
            }
        } else {
            match &opts.host {
                Some(h) if !h.skills.contains(&id) => dropped.push(DroppedStep {
                    reason: format!(
                        "already gone from the host: skill `{id}` is not installed (its ledgered files are removed below)"
                    ),
                    step: UndoStep::SkillsUninstall { id },
                }),
                _ => host_steps.push(UndoStep::SkillsUninstall { id }),
            }
            remove.extend(steps);
        }
    }
    // The settings document as the plan finds it, and every settings key
    // this plan restores to absent — the structural rule's simulation.
    let settings_doc: Option<Value> = opts
        .settings_file
        .as_deref()
        .and_then(|f| fs::read(f).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let removals: Vec<Vec<String>> = ledger
        .registrations
        .iter()
        .filter_map(|reg| match reg {
            Registration::SettingsKey {
                path,
                prior: None,
                value,
                ..
            } if opts.force
                || still_ours(
                    opts.settings_file.as_deref(),
                    |doc| dotted_value(doc, path).cloned(),
                    value,
                ) =>
            {
                Some(segments(path))
            }
            _ => None,
        })
        .collect();
    // Registrations, reverse order; a settings key or trust entry the user
    // edited since omm wrote it is preserved (its registration recorded the
    // value omm set), unless `force`.
    for reg in ledger.registrations.iter().rev() {
        // A shared file the user removed since install: nothing to restore
        // into (a restore would recreate it), the registration is dropped.
        let gone = match reg {
            Registration::SettingsKey { .. } => opts
                .settings_file
                .as_deref()
                .map(|f| !f.exists())
                .unwrap_or(false),
            Registration::Trust { .. } => opts
                .trust_file
                .as_deref()
                .map(|f| !f.exists())
                .unwrap_or(false),
            _ => false,
        };
        if gone {
            let file = match reg {
                Registration::Trust { .. } => "trust.json",
                _ => "settings.json",
            };
            dropped.push(DroppedStep {
                step: UndoStep::from_registration(reg),
                reason: format!("{file} is gone; nothing to restore into (registration dropped)"),
            });
            continue;
        }
        // The host no longer holds it: already gone, dropped with the reason.
        if let Some(h) = &opts.host {
            let absent = match reg {
                Registration::MusePlugin { id, .. } if !h.plugins.contains(id) => {
                    Some(format!("plugin `{id}` is not installed"))
                }
                Registration::MuseMarketplace { name, .. } if !h.marketplaces.contains(name) => {
                    Some(format!("marketplace `{name}` is not configured"))
                }
                _ => None,
            };
            if let Some(why) = absent {
                dropped.push(DroppedStep {
                    step: UndoStep::from_registration(reg),
                    reason: format!("already gone from the host: {why} (registration dropped)"),
                });
                continue;
            }
        }
        // A required structural member the user's own members still need
        // stays (Gate 1 decision D): predicted here, checked again at apply.
        if let Registration::SettingsKey {
            path, prior: None, ..
        } = reg
        {
            let segs = segments(path);
            if is_structural_leaf(&segs) && removals.contains(&segs) {
                if let Some(why) = predict_structural_keep(settings_doc.as_ref(), &segs, &removals)
                {
                    dropped.push(DroppedStep {
                        step: UndoStep::from_registration(reg),
                        reason: format!("kept: {why} (registration dropped)"),
                    });
                    continue;
                }
            }
        }
        let (ours, preserved) = match reg {
            Registration::SettingsKey { path, value, .. } => {
                let ours = opts.force
                    || still_ours(
                        opts.settings_file.as_deref(),
                        |doc| dotted_value(doc, path).cloned(),
                        value,
                    );
                let now = opts
                    .settings_file
                    .as_deref()
                    .and_then(|f| fs::read(f).ok())
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .and_then(|d| dotted_value(&d, path).cloned());
                (
                    ours,
                    opts.settings_file.as_deref().and_then(|f| shared_file_key(bases, f)).map(|key| Preserved {
                        key,
                        abs: None,
                        reason: format!(
                            "settings key {path} edited since omm set it (omm wrote {}, now {}); --force restores the prior",
                            value.as_ref().map(Value::to_string).unwrap_or_else(|| "absent".into()),
                            now.map(|v| v.to_string()).unwrap_or_else(|| "absent".into())
                        ),
                        ledger_sha256: String::new(),
                        on_disk: Observed::Missing,
                    }),
                )
            }
            Registration::Trust { project, value, .. } => {
                let lookup =
                    |doc: &Value| doc.get("projects").and_then(|p| p.get(project)).cloned();
                let ours = opts.force || still_ours(opts.trust_file.as_deref(), lookup, value);
                let now = opts
                    .trust_file
                    .as_deref()
                    .and_then(|f| fs::read(f).ok())
                    .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                    .and_then(|d| lookup(&d));
                (
                    ours,
                    opts.trust_file.as_deref().and_then(|f| shared_file_key(bases, f)).map(|key| Preserved {
                        key,
                        abs: None,
                        reason: format!(
                            "trust entry {project} edited since omm set it (omm wrote {}, now {}); --force restores the prior",
                            value.as_ref().map(Value::to_string).unwrap_or_else(|| "absent".into()),
                            now.map(|v| v.to_string()).unwrap_or_else(|| "absent".into())
                        ),
                        ledger_sha256: String::new(),
                        on_disk: Observed::Missing,
                    }),
                )
            }
            _ => (true, None),
        };
        if ours {
            host_steps.push(UndoStep::from_registration(reg));
        } else if let Some(p) = preserved {
            preserve.push(p);
        }
    }
    let host_steps = order_structural_last(host_steps);

    // Deepest first: more components first, then reverse lexical so siblings
    // are deterministic.
    remove.sort_by(|a, b| {
        b.abs
            .components()
            .count()
            .cmp(&a.abs.components().count())
            .then_with(|| b.abs.cmp(&a.abs))
    });

    let omm_state = omm_state(bases, opts.force)?;
    Ok(Plan {
        force: opts.force,
        remove,
        preserve,
        missing,
        rules: rules_steps,
        created,
        shared: shared_steps,
        host_steps,
        dropped,
        omm_state,
        residue: bases.residue.clone(),
        refused,
    })
}

/// True when a JSON document holds nothing but `schema_version` and empty
/// objects — what a settings or trust file omm created looks like once every
/// key it set has been restored to absent.
pub fn is_empty_shared_document(bytes: &[u8]) -> bool {
    serde_json::from_slice::<Value>(bytes)
        .ok()
        .and_then(|v| v.as_object().cloned())
        .map(|m| {
            m.iter().all(|(k, v)| {
                k == "schema_version" || v.as_object().map(|o| o.is_empty()).unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// Remove omm's own atomic-write droppings ([`fsx::is_tmp_name`]) from
/// `dir`, returning how many went. A run killed between the temp create
/// and the rename leaves them in managed dirs too, unledgered; the prune
/// below calls this before each `remove_dir` so a dropping never blocks it.
/// Runs under the exclusive ledger lock, like every other removal here.
fn sweep_tmps(dir: &Path) -> usize {
    let mut swept = 0;
    if let Ok(rd) = fs::read_dir(dir) {
        for entry in rd.flatten() {
            let name = entry.file_name();
            if fsx::is_tmp_name(&name.to_string_lossy()) && fs::remove_file(entry.path()).is_ok() {
                swept += 1;
            }
        }
    }
    swept
}

fn omm_state(bases: &Bases, force: bool) -> Result<OmmState> {
    let root = &bases.omm;
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
    if canonical == Path::new("/")
        || bases
            .home
            .as_ref()
            .map(|h| fs::canonicalize(h).map(|c| c == canonical).unwrap_or(false))
            .unwrap_or(false)
    {
        return Err(LedgerError::Refused(format!(
            "omm root {} is / or $HOME",
            canonical.display()
        )));
    }
    let mut remove = Vec::new();
    let mut keep = Vec::new();
    match fs::read_dir(&canonical) {
        Ok(entries) => {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name == store::LEDGER_FILE {
                    continue;
                }
                let owned = OMM_STATE_NAMES.contains(&name.as_str())
                    || store::is_quarantine_name(&name)
                    // A run killed between the temp create and the rename
                    // (scenario 26): omm's own dropping, removed with the rest.
                    || fsx::is_tmp_name(&name);
                if owned || force {
                    remove.push(e.path());
                } else {
                    keep.push(e.path());
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(LedgerError::io("read omm root", &canonical, e)),
    }
    remove.sort();
    keep.sort();
    Ok(OmmState {
        ledger: store::ledger_path(&canonical),
        root: canonical,
        remove,
        keep,
        remove_root_if_empty: true,
    })
}

/// What one [`Undoer::undo`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoOutcome {
    /// The step ran and the host (or the file) is as it was before omm.
    Undone,
    /// The host no longer held it — `not installed`, `not configured`,
    /// `skill not installed` — so there was nothing to undo: the same end
    /// state, the registration is dropped with this reason (Gate 1
    /// decision B: the undo is idempotent).
    AlreadyGone(String),
    /// A settings key left as it stands because removing it would break the
    /// user's own data (a required structural member of an object that
    /// still holds their members; Gate 1 decision D); the reason names it.
    Kept(String),
}

/// Executes host-side undo steps. The real one is [`HostUndoer`];
/// [`RecordingUndoer`] records for dry runs and tests.
pub trait Undoer {
    fn undo(&mut self, step: &UndoStep) -> std::result::Result<UndoOutcome, String>;
    /// Whether the host loads `candidate` as its `settings.json` — asked
    /// before a merged shared document is written back ([`SharedStep`]).
    /// The default accepts; [`HostUndoer`] runs the host's loader probe.
    fn validate_settings(&mut self, _candidate: &[u8]) -> std::result::Result<(), String> {
        Ok(())
    }
}

/// Records every step, never touches the host.
#[derive(Debug, Default)]
pub struct RecordingUndoer {
    pub steps: Vec<UndoStep>,
    /// Labels of steps to fail (substring match), for tests.
    pub fail_matching: Vec<String>,
    /// `(label substring, path, bytes)`: a file written when a matching step
    /// runs — what a host verb leaves behind (`plugins remove --delete-data`
    /// recreates `settings.json`), for tests.
    pub side_effects: Vec<(String, PathBuf, Vec<u8>)>,
    /// Labels of steps the "host" answers `not installed` to (substring
    /// match): [`UndoOutcome::AlreadyGone`], for tests.
    pub already_gone: Vec<String>,
}

impl Undoer for RecordingUndoer {
    fn undo(&mut self, step: &UndoStep) -> std::result::Result<UndoOutcome, String> {
        self.steps.push(step.clone());
        let label = step.label();
        for (needle, path, bytes) in &self.side_effects {
            if label.contains(needle.as_str()) {
                fs::write(path, bytes)
                    .map_err(|e| format!("side effect {}: {e}", path.display()))?;
            }
        }
        if self
            .fail_matching
            .iter()
            .any(|f| label.contains(f.as_str()))
        {
            return Err(format!("simulated failure of `{label}`"));
        }
        if self.already_gone.iter().any(|f| label.contains(f.as_str())) {
            return Ok(UndoOutcome::AlreadyGone(format!(
                "already gone from the host (simulated): `{label}`"
            )));
        }
        Ok(UndoOutcome::Undone)
    }
}

/// The host-backed undoer: `muse plugins remove` / `marketplace remove` /
/// `skills uninstall` through the R20-allowlisted [`Invoker`] (the plugins
/// gate is applied by the invoker for `plugins …`), settings keys through
/// `SettingsDoc::patch_typed` + validated commit, trust through
/// `TrustStore::restore_project` + commit — both under the muse-config lock
/// with a verified backup under `$OMM/snapshots/`.
#[derive(Debug)]
pub struct HostUndoer {
    pub inv: Invoker,
    pub roots: Roots,
}

impl HostUndoer {
    pub fn new(inv: Invoker, roots: Roots) -> HostUndoer {
        HostUndoer { inv, roots }
    }

    /// Run a host verb; `gone` tells the host's "nothing to remove" answer
    /// from a failure (Gate 1 decision B: the undo is idempotent, so that
    /// answer is [`UndoOutcome::AlreadyGone`], never an error that wedges
    /// every rerun).
    fn run(
        &self,
        argv: &[&str],
        gone: impl Fn(&str, &str) -> bool,
    ) -> std::result::Result<UndoOutcome, String> {
        let out = self.inv.run(argv).map_err(|e| e.to_string())?;
        if out.ok() {
            return Ok(UndoOutcome::Undone);
        }
        let (code, message) = match out.host_reported_error() {
            Some(omm_host::HostError::HostReported { code, message }) => (code, message),
            Some(e) => (String::new(), e.to_string()),
            None => (
                String::new(),
                out.stderr.lines().next().unwrap_or("").trim().to_string(),
            ),
        };
        if gone(&code, &message) {
            return Ok(UndoOutcome::AlreadyGone(format!(
                "already gone from the host: {}",
                if message.is_empty() {
                    code.clone()
                } else {
                    message.clone()
                }
            )));
        }
        Err(match out.expect_ok() {
            Ok(_) => "unexpected success".to_string(),
            Err(e) => e.to_string(),
        })
    }
}

/// The host's "not installed" answers per verb (`error.code`, else the
/// message): `unknown-plugin` / `is not installed`, `unknown-marketplace` /
/// `is not configured`, `skill-not-installed` / `not installed`.
fn plugin_gone(code: &str, message: &str) -> bool {
    code == "unknown-plugin" || message.contains("not installed")
}
fn marketplace_gone(code: &str, message: &str) -> bool {
    code == "unknown-marketplace" || message.contains("not configured")
}
fn skill_gone(code: &str, message: &str) -> bool {
    code == "skill-not-installed" || message.contains("not installed")
}

impl Undoer for HostUndoer {
    fn validate_settings(&mut self, candidate: &[u8]) -> std::result::Result<(), String> {
        let load = probe::settings_load_probe(&self.inv, candidate).map_err(|e| e.to_string())?;
        if load.accepted {
            Ok(())
        } else {
            Err(format!(
                "the host's loader rejects the merged settings.json: {}",
                load.detail
            ))
        }
    }

    fn undo(&mut self, step: &UndoStep) -> std::result::Result<UndoOutcome, String> {
        match step {
            UndoStep::SkillsUninstall { id } => {
                self.run(&["skills", "uninstall", id.as_str(), "--json"], skill_gone)
            }
            UndoStep::PluginRemove { id } => self.run(
                &["plugins", "remove", id.as_str(), "--delete-data", "--json"],
                plugin_gone,
            ),
            UndoStep::MarketplaceRemove { name } => self.run(
                &["plugins", "marketplace", "remove", name.as_str(), "--json"],
                marketplace_gone,
            ),
            UndoStep::RestoreSettingsKey { path, prior } => {
                let mut doc = SettingsDoc::for_roots(&self.roots).map_err(|e| e.to_string())?;
                let segments: Vec<String> = path
                    .split('.')
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
                    .collect();
                if prior.is_none() {
                    // A required structural member of an object that still
                    // holds the user's members stays (Gate 1 decision D).
                    if let Some(why) = settings::structural_keep_reason(doc.value(), &segments)
                        .map_err(|e| e.to_string())?
                    {
                        return Ok(UndoOutcome::Kept(format!("kept: {why}")));
                    }
                }
                let op = match prior {
                    Some(v) => PatchOp::set(path, v.clone()),
                    None => PatchOp::remove(path),
                };
                doc.patch_typed(&[op]).map_err(|e| e.to_string())?;
                if prior.is_none() {
                    // A removal leaves `{}` parents behind (`tui: {}` after
                    // `tui.theme`); an empty object and an absent key are the
                    // same to the host's typed loader, so prune them for an
                    // exact-looking undo.
                    let mut segments = segments;
                    while segments.len() > 1 {
                        segments.pop();
                        let empty = doc
                            .get(&segments)
                            .and_then(Value::as_object)
                            .map(|o| o.is_empty())
                            .unwrap_or(false);
                        if !empty {
                            break;
                        }
                        doc.patch_typed(&[PatchOp::remove_path(segments.clone())])
                            .map_err(|e| e.to_string())?;
                    }
                }
                doc.commit(&self.inv, &CommitOptions::for_roots(&self.roots))
                    .map(|_| UndoOutcome::Undone)
                    .map_err(|e| e.to_string())
            }
            UndoStep::RestoreTrust { project, prior } => {
                let mut store = TrustStore::for_roots(&self.roots).map_err(|e| e.to_string())?;
                store.restore_project(project, prior.clone());
                store
                    .commit(&CommitOptions::for_roots(&self.roots))
                    .map(|_| UndoOutcome::Undone)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

/// Options of [`apply`].
#[derive(Clone, Debug)]
pub struct ApplyOptions {
    /// The `omm <version>` actor of audit lines.
    pub omm_version: String,
    /// Report only; the undoer is still asked (a [`RecordingUndoer`] then
    /// lists what would run).
    pub dry_run: bool,
}

/// Per-category counts and what went wrong.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub removed: usize,
    pub preserved: usize,
    pub missing: usize,
    /// Rules files restored to the pre-omm file or rewritten as the user region.
    pub rules_rewritten: usize,
    /// Shared files back at their pre-omm bytes and mode.
    pub shared_restored: usize,
    /// Shared files kept (edited since omm arrived), untyped keys merged back where any were missing.
    pub shared_kept: usize,
    pub registrations_undone: usize,
    /// Registrations dropped because their shared file was gone, or the
    /// host no longer held the object (already gone), or the key is a
    /// structural member the user's data still needs (kept).
    pub registrations_dropped: usize,
    /// Of `registrations_dropped`, the settings keys kept for the user's
    /// sake ([`UndoOutcome::Kept`], or predicted so by the plan).
    pub registrations_kept: usize,
    pub dirs_removed: usize,
    /// Atomic-write droppings swept from managed dirs before the prune.
    pub tmps_removed: usize,
    pub omm_state_removed: usize,
    pub ledger_removed: bool,
    pub errors: Vec<String>,
    pub dry_run: bool,
}

impl Report {
    /// True when nothing failed.
    pub fn complete(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Execute the plan (see the module docs for the order). Takes the ledger
/// lock; on any failure the remaining ledger is saved and `errors` names the
/// failures; on success the ledger and `$OMM/` state are gone.
pub fn apply(
    plan: &Plan,
    ledger: Ledger,
    bases: &Bases,
    undoer: &mut dyn Undoer,
    opts: &ApplyOptions,
) -> Result<Report> {
    if !plan.refused.is_empty() {
        return Err(LedgerError::Refused(format!(
            "{} entries resolve onto a refused root: {}",
            plan.refused.len(),
            plan.refused
                .iter()
                .map(|r| format!("{} ({})", r.key, r.reason))
                .collect::<Vec<_>>()
                .join("; ")
        )));
    }
    let mut report = Report {
        preserved: plan.preserve.len(),
        missing: plan.missing.len(),
        dry_run: opts.dry_run,
        ..Report::default()
    };
    if opts.dry_run {
        for step in &plan.host_steps {
            match undoer.undo(step) {
                Err(e) => report.errors.push(e),
                Ok(UndoOutcome::Undone) => report.registrations_undone += 1,
                Ok(UndoOutcome::AlreadyGone(_)) => report.registrations_dropped += 1,
                Ok(UndoOutcome::Kept(_)) => {
                    report.registrations_dropped += 1;
                    report.registrations_kept += 1;
                }
            }
        }
        report.removed = plan.remove.len() + plan.created.len();
        report.rules_rewritten = plan.rules.len();
        report.shared_restored = plan.shared.len();
        report.registrations_dropped += plan.dropped.len();
        report.registrations_kept += plan
            .dropped
            .iter()
            .filter(|d| d.reason.starts_with("kept"))
            .count();
        report.omm_state_removed = plan.omm_state.remove.len();
        return Ok(report);
    }

    let omm_root = &plan.omm_state.root;
    let _lock = store::lock(omm_root, fsx::LOCK_WAIT_DEFAULT)?;
    let audit = Audit::new(omm_root, &opts.omm_version);
    let mut ledger = ledger;

    // 1. Host steps, in plan order (reverse of registration). A step the
    //    host answers "not installed" to is done (idempotent undo): the
    //    registration is dropped and the reason audited.
    let mut registrations = ledger.registrations.clone();
    for step in &plan.host_steps {
        match undoer.undo(step) {
            Ok(UndoOutcome::Undone) => {
                report.registrations_undone += 1;
                audit.append(&Event::new(ACTION_UNINSTALL).note(step.label()))?;
                registrations.retain(|r| UndoStep::from_registration(r) != *step);
            }
            Ok(UndoOutcome::AlreadyGone(reason)) => {
                report.registrations_dropped += 1;
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .note(format!("{}: {reason}; registration dropped", step.label())),
                )?;
                registrations.retain(|r| UndoStep::from_registration(r) != *step);
            }
            Ok(UndoOutcome::Kept(reason)) => {
                report.registrations_dropped += 1;
                report.registrations_kept += 1;
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .note(format!("{}: {reason}; registration dropped", step.label())),
                )?;
                registrations.retain(|r| UndoStep::from_registration(r) != *step);
            }
            Err(e) => {
                let msg = format!("{}: {e}", step.label());
                audit.append(&Event::new(ACTION_UNINSTALL).note(format!("FAILED {msg}")))?;
                report.errors.push(msg);
            }
        }
    }
    for d in &plan.dropped {
        audit.append(&Event::new(ACTION_UNINSTALL).note(format!(
            "skipped {}: {}",
            d.step.label(),
            d.reason
        )))?;
        registrations.retain(|r| UndoStep::from_registration(r) != d.step);
        report.registrations_dropped += 1;
        if d.reason.starts_with("kept") {
            report.registrations_kept += 1;
        }
    }
    ledger.registrations = registrations;
    // The host's verbs above (`plugins remove`, `skills uninstall`) rewrite
    // settings.json at their own mode: the recorded pre-omm mode comes back
    // right after them (Gate 1 decision F), and again with the bytes below.
    for s in plan
        .shared
        .iter()
        .filter(|s| s.mechanism == Mechanism::SettingsPatch)
    {
        if let Some(mode) = s.original.mode {
            if s.abs.is_file() {
                shared::set_mode(&s.abs, mode)?;
            }
        }
    }
    // A shared file is finalised — the pre-omm bytes back, or the emptied
    // file removed — only once every registration into it is undone: with
    // a host step failed above (Gate 1 round 5: a malformed trust.json
    // makes every restore that runs the host's validator fail) the
    // document is not final, so both phases wait for the rerun and their
    // entries stay in the ledger.
    let host_steps_failed = !report.errors.is_empty();
    if host_steps_failed && !(plan.shared.is_empty() && plan.created.is_empty()) {
        audit.append(&Event::new(ACTION_UNINSTALL).note(format!(
            "deferred: {} shared file(s) kept as they stand until every host step succeeds ({} failed); the rerun finalises them",
            plan.shared.len() + plan.created.len(),
            report.errors.len()
        )))?;
    }

    // 1a. Shared files omm found before it wrote: back to their originals.
    for s in plan.shared.iter().filter(|_| !host_steps_failed) {
        if bases.is_refused_root(&s.abs) {
            return Err(LedgerError::Refused(format!(
                "{} is a refused root",
                s.abs.display()
            )));
        }
        match restore_shared(s, undoer, omm_root) {
            Ok(outcome) => {
                let note = match &outcome {
                    SharedOutcome::Restored => {
                        report.shared_restored += 1;
                        "restored to the bytes and mode found before omm".to_string()
                    }
                    SharedOutcome::Merged { keys } => {
                        report.shared_kept += 1;
                        format!(
                            "kept (edited since omm arrived); untyped keys the host's rewrite dropped merged back: {}; mode re-applied",
                            keys.join(", ")
                        )
                    }
                    SharedOutcome::Kept => {
                        report.shared_kept += 1;
                        "kept (edited since omm arrived); mode re-applied".to_string()
                    }
                    SharedOutcome::Gone => {
                        report.missing += 1;
                        "gone since the plan was made".to_string()
                    }
                };
                ledger.remove(s.key.base, &s.key.path);
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .at(s.key.base, &s.key.path)
                        .before(Some(&s.original.sha256))
                        .note(note),
                )?;
            }
            Err(e) => report.errors.push(format!("{}: {e}", s.abs.display())),
        }
    }

    // 1b. Shared files omm created (or that a host step recreated), now that
    //     their keys are restored.
    for c in plan.created.iter().filter(|_| !host_steps_failed) {
        if bases.is_refused_root(&c.abs) {
            return Err(LedgerError::Refused(format!(
                "{} is a refused root",
                c.abs.display()
            )));
        }
        let bytes = match fs::read(&c.abs) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                ledger.remove(c.key.base, &c.key.path);
                if c.absent_at_plan {
                    audit.append(
                        &Event::new(ACTION_UNINSTALL)
                            .at(c.key.base, &c.key.path)
                            .note("absent before and after; nothing to restore into"),
                    )?;
                } else {
                    report.missing += 1;
                }
                continue;
            }
            Err(e) => {
                report.errors.push(format!("read {}: {e}", c.abs.display()));
                continue;
            }
        };
        if serde_json::from_slice::<Value>(&bytes).is_err() {
            // Not a JSON document: neither omm's nor the host's writing;
            // left as it stands, the entry kept for a rerun once repaired.
            report.errors.push(format!(
                "{}: not a JSON document; left in place (its entry is kept for a rerun once the file is repaired)",
                c.abs.display()
            ));
            continue;
        }
        if !is_empty_shared_document(&bytes) {
            report.preserved += 1;
            audit.append(
                &Event::new(ACTION_UNINSTALL)
                    .at(c.key.base, &c.key.path)
                    .note(if c.absent_at_plan {
                        "preserved: absent when uninstall started, recreated by a host step and no longer empty"
                    } else {
                        "preserved: created by omm but no longer empty after the restores"
                    }),
            )?;
            ledger.remove(c.key.base, &c.key.path);
            continue;
        }
        match fs::remove_file(&c.abs) {
            Ok(()) => {
                report.removed += 1;
                ledger.remove(c.key.base, &c.key.path);
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .at(c.key.base, &c.key.path)
                        .before(Some(&hash::sha256_bytes(&bytes)))
                        .note(if c.absent_at_plan {
                            "removed: absent when uninstall started, recreated empty by a host step"
                        } else {
                            "removed: created by omm, empty after the restores"
                        }),
                )?;
            }
            Err(e) => report
                .errors
                .push(format!("remove {}: {e}", c.abs.display())),
        }
    }

    // 1c. The rules file: back to the pre-omm bytes, or the user region alone.
    for step in &plan.rules {
        let now = match hash::observe(&step.abs) {
            Ok(o) => o,
            Err(e) => {
                report.errors.push(format!("{}: {e}", step.abs.display()));
                continue;
            }
        };
        if !now.equals(&step.on_disk_sha256) {
            report.errors.push(format!(
                "{} changed since the plan was made ({now:?}); left in place",
                step.abs.display()
            ));
            report.preserved += 1;
            continue;
        }
        if bases.is_refused_root(&step.abs) || !step.abs.starts_with(&step.base_root) {
            return Err(LedgerError::Refused(format!(
                "{} is not strictly inside {}",
                step.abs.display(),
                step.base_root.display()
            )));
        }
        match fsx::write_atomic(&step.abs, step.bytes()) {
            Ok(()) => {
                report.rules_rewritten += 1;
                ledger.remove(step.key.base, &step.key.path);
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .at(step.key.base, &step.key.path)
                        .before(Some(&step.on_disk_sha256))
                        .after(Some(&hash::sha256_bytes(step.bytes())))
                        .note(step.label()),
                )?;
            }
            Err(e) => report
                .errors
                .push(format!("rewrite {}: {e}", step.abs.display())),
        }
    }

    // 2. Files, deepest first; then prune the directories left empty.
    let mut touched_dirs: Vec<(PathBuf, PathBuf)> = Vec::new();
    for step in &plan.remove {
        // Re-check right before unlinking: the plan may be stale.
        let now = match hash::observe(&step.abs) {
            Ok(o) => o,
            Err(e) => {
                report.errors.push(format!("{}: {e}", step.abs.display()));
                continue;
            }
        };
        match now {
            Observed::Missing if step.mechanism == Mechanism::MuseSkillsInstall => {
                // `muse skills uninstall` (a host step above) took it.
                ledger.remove(step.key.base, &step.key.path);
                report.removed += 1;
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .at(step.key.base, &step.key.path)
                        .before(Some(&step.on_disk_sha256))
                        .note("removed by muse skills uninstall"),
                )?;
                continue;
            }
            Observed::Missing => {
                ledger.remove(step.key.base, &step.key.path);
                report.missing += 1;
                continue;
            }
            Observed::Content(ref h) if h == &step.on_disk_sha256 => {}
            Observed::Content(_) if step.forced => {}
            other => {
                report.errors.push(format!(
                    "{} changed since the plan was made ({other:?}); left in place",
                    step.abs.display()
                ));
                report.preserved += 1;
                continue;
            }
        }
        if bases.is_refused_root(&step.abs) || !step.abs.starts_with(&step.base_root) {
            return Err(LedgerError::Refused(format!(
                "{} is not strictly inside {}",
                step.abs.display(),
                step.base_root.display()
            )));
        }
        match fs::remove_file(&step.abs) {
            Ok(()) => {
                report.removed += 1;
                ledger.remove(step.key.base, &step.key.path);
                audit.append(
                    &Event::new(ACTION_UNINSTALL)
                        .at(step.key.base, &step.key.path)
                        .before(Some(&step.on_disk_sha256))
                        .note(if step.forced {
                            "removed (--force, edited)"
                        } else {
                            "removed"
                        }),
                )?;
                if let Some(parent) = step.abs.parent() {
                    touched_dirs.push((parent.to_path_buf(), step.base_root.clone()));
                }
            }
            Err(e) => report
                .errors
                .push(format!("remove {}: {e}", step.abs.display())),
        }
    }
    for m in &plan.missing {
        ledger.remove(m.key.base, &m.key.path);
        audit.append(
            &Event::new(ACTION_UNINSTALL)
                .at(m.key.base, &m.key.path)
                .note("missing on disk; ledger entry dropped"),
        )?;
    }
    for p in &plan.preserve {
        audit.append(
            &Event::new(ACTION_UNINSTALL)
                .at(p.key.base, &p.key.path)
                .before(p.on_disk.text())
                .note(format!("preserved: {}", p.reason)),
        )?;
    }
    // Deepest directories first; stop at the first non-empty one.
    touched_dirs.sort_by(|a, b| {
        b.0.components()
            .count()
            .cmp(&a.0.components().count())
            .then_with(|| b.0.cmp(&a.0))
    });
    touched_dirs.dedup();
    for (dir, base_root) in touched_dirs {
        let mut cur = dir;
        while cur != base_root && cur.starts_with(&base_root) && !bases.is_refused_root(&cur) {
            // Sweep omm's own atomic-write droppings first: a run killed
            // between the temp create and the rename (scenario 26) leaves
            // `.<name>.omm-tmp-<random>` behind, and an unledgered dropping
            // must not block the prune of a dir omm just emptied.
            report.tmps_removed += sweep_tmps(&cur);
            match fs::remove_dir(&cur) {
                Ok(()) => report.dirs_removed += 1,
                Err(_) => break, // not empty (or gone): stop climbing
            }
            match cur.parent() {
                Some(p) => cur = p.to_path_buf(),
                None => break,
            }
        }
    }

    if !report.complete() {
        // Keep what is left so a rerun can continue.
        store::save(omm_root, &ledger)?;
        return Ok(report);
    }

    // 3. Entries that were preserved stay in the ledger? No: the ledger is
    // removed last, and the preserved files are named in the audit log. An
    // uninstall that preserved something has still uninstalled.
    // 4. `$OMM/` state, then the ledger last.
    drop(_lock);
    // A fresh listing: the lock file and the audit lines this very run
    // wrote are omm's state too. Only owned names, or what the previewed
    // plan already listed (the `force` case), are removed.
    let fresh = omm_state(bases, false)?;
    let mut targets: Vec<PathBuf> = fresh.remove;
    for p in &plan.omm_state.remove {
        if !targets.contains(p) {
            targets.push(p.clone());
        }
    }
    for p in &targets {
        let is_symlink = fs::symlink_metadata(p)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        let r = if p.is_dir() && !is_symlink {
            fs::remove_dir_all(p)
        } else {
            fs::remove_file(p)
        };
        match r {
            Ok(()) => report.omm_state_removed += 1,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => report.errors.push(format!("remove {}: {e}", p.display())),
        }
    }
    if report.complete() {
        report.ledger_removed = store::remove(omm_root)?;
        if plan.omm_state.remove_root_if_empty {
            let _ = fs::remove_dir(omm_root);
        }
    } else {
        store::save(omm_root, &ledger)?;
    }
    Ok(report)
}

/// One [`SharedStep`] (see the module docs): the untyped members of the
/// pre-omm document and of the document as the plan found it are merged
/// into the file as the host steps left it; equal to the pre-omm document
/// → the recorded bytes land; else the merged document (validated by the
/// undoer when anything was merged); the recorded mode in both cases. Under
/// the muse-config lock, like every writer of these files.
fn restore_shared(
    s: &SharedStep,
    undoer: &mut dyn Undoer,
    omm_root: &Path,
) -> std::result::Result<SharedOutcome, String> {
    let current = match fs::read(&s.abs) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(SharedOutcome::Gone),
        Err(e) => return Err(format!("read: {e}")),
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&current) else {
        // Not JSON any more: not what omm or the host wrote; theirs.
        return Ok(SharedOutcome::Kept);
    };
    let mut merged = doc.clone();
    let mut members: Vec<(Vec<String>, Value)> = Vec::new();
    if s.mechanism == Mechanism::SettingsPatch {
        if let Some(original_doc) = s.original.document() {
            members.extend(settings::untyped_members(&original_doc).map_err(|e| e.to_string())?);
        }
        members.extend(s.untyped_now.iter().cloned());
    }
    let inserted = shared::merge_members(&mut merged, &members);
    let original_doc = s.original.document();
    let bytes = if Some(&merged) == original_doc.as_ref() {
        Some(s.original.bytes.clone())
    } else if !inserted.is_empty() {
        let candidate = shared::pretty(&merged).map_err(|e| e.to_string())?;
        if s.mechanism == Mechanism::SettingsPatch {
            undoer.validate_settings(&candidate)?;
        }
        Some(candidate)
    } else {
        None
    };
    let _lock = fsx::lock_exclusive(
        &settings::muse_config_lock_path(omm_root),
        fsx::LOCK_WAIT_DEFAULT,
    )
    .map_err(|e| e.to_string())?;
    if let Some(bytes) = &bytes {
        if *bytes != current {
            fsx::write_atomic(&s.abs, bytes).map_err(|e| e.to_string())?;
        }
    }
    if let Some(mode) = s.original.mode {
        shared::set_mode(&s.abs, mode).map_err(|e| e.to_string())?;
    }
    Ok(match bytes {
        Some(b) if b == s.original.bytes => SharedOutcome::Restored,
        Some(_) => SharedOutcome::Merged { keys: inserted },
        None => SharedOutcome::Kept,
    })
}

/// The default wait for the ledger lock in [`apply`].
pub const LOCK_WAIT: Duration = fsx::LOCK_WAIT_DEFAULT;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Entry, HostInfo, Kind, RelPath, Scope};
    use serde_json::json;

    struct Fx {
        _dir: tempfile::TempDir,
        bases: Bases,
        ledger: Ledger,
    }

    fn write(bases: &Bases, base: Base, rel: &str, bytes: &[u8]) -> String {
        let p = RelPath::new(rel).unwrap().under(bases.root(base).unwrap());
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, bytes).unwrap();
        hash::sha256_bytes(bytes)
    }

    fn entry(base: Base, rel: &str, sha: &str, mechanism: Mechanism) -> Entry {
        Entry {
            base,
            path: RelPath::new(rel).unwrap(),
            kind: Kind::Skill,
            sha256: sha.to_string(),
            source_version: "0.1.0".into(),
            writer: "omm install".into(),
            mechanism,
            class: Class::Exclusive,
            prior: None,
        }
    }

    fn fixture() -> Fx {
        let dir = tempfile::tempdir().unwrap();
        let bases = Bases {
            muse_config: dir.path().join("config/muse"),
            muse_data: dir.path().join("data/muse"),
            omm: dir.path().join("config/omm"),
            workspace: Some(dir.path().join("ws")),
            home: Some(dir.path().join("home")),
            residue: vec![dir
                .path()
                .join("home/Library/Application Support/Muse/session-name-authority")],
        };
        for p in [
            &bases.muse_config,
            &bases.muse_data,
            &bases.omm,
            &dir.path().join("ws"),
            &dir.path().join("home"),
        ] {
            fs::create_dir_all(p).unwrap();
        }
        let mut ledger = Ledger::new(
            "0.1.0",
            HostInfo {
                version: "v".into(),
                sha256: "s".into(),
            },
            Scope::User,
        );
        // Plain copies: one untouched, one edited, one missing, one deep.
        let s = write(&bases, Base::MuseConfig, "themes/omm-x.tmTheme", b"theme");
        ledger.upsert(entry(
            Base::MuseConfig,
            "themes/omm-x.tmTheme",
            &s,
            Mechanism::Copy,
        ));
        let s = write(&bases, Base::MuseConfig, "AGENTS.md", b"rules");
        ledger.upsert(entry(Base::MuseConfig, "AGENTS.md", &s, Mechanism::Copy));
        fs::write(bases.muse_config.join("AGENTS.md"), b"rules + mine").unwrap();
        ledger.upsert(entry(
            Base::MuseConfig,
            "gone.md",
            &"0".repeat(64),
            Mechanism::Copy,
        ));
        let s = write(&bases, Base::Workspace, ".omm/skills/deep/a/b.md", b"deep");
        ledger.upsert(entry(
            Base::Workspace,
            ".omm/skills/deep/a/b.md",
            &s,
            Mechanism::Copy,
        ));
        // Managed skills: x clean, y with an edited file.
        for (id, edit) in [("x", false), ("y", true)] {
            let rel = format!("skills/omm-{id}/SKILL.md");
            let s = write(&bases, Base::MuseConfig, &rel, b"skill");
            ledger.upsert(entry(
                Base::MuseConfig,
                &rel,
                &s,
                Mechanism::MuseSkillsInstall,
            ));
            let rel2 = format!("skills/omm-{id}/references/r.md");
            let s2 = write(&bases, Base::MuseConfig, &rel2, b"ref");
            ledger.upsert(entry(
                Base::MuseConfig,
                &rel2,
                &s2,
                Mechanism::MuseSkillsInstall,
            ));
            if edit {
                fs::write(bases.muse_config.join(&rel2), b"ref edited").unwrap();
            }
        }
        // A shared-key entry and the registrations.
        write(
            &bases,
            Base::MuseConfig,
            "settings.json",
            b"{\"schema_version\":1}\n",
        );
        let mut sk = entry(
            Base::MuseConfig,
            "settings.json",
            "n/a",
            Mechanism::SettingsPatch,
        );
        sk.kind = Kind::SettingsKey;
        sk.class = Class::SharedKey;
        ledger.upsert(sk);
        ledger.register(Registration::MuseMarketplace {
            name: "ohmy".into(),
            source: "/src".into(),
        });
        ledger.register(Registration::MusePlugin {
            id: "omm".into(),
            package_sha256: "d".into(),
            generation_path: "/g".into(),
            approved: vec![],
        });
        ledger.register(Registration::SettingsKey {
            path: "tui.theme".into(),
            prior: Some(json!("old")),
            value: None,
            profile: None,
        });
        ledger.register(Registration::Trust {
            project: "/ws".into(),
            prior: None,
            value: None,
        });
        // omm state + the user's overlay.
        fs::create_dir_all(bases.omm.join("snapshots/20260101T000000Z")).unwrap();
        fs::write(bases.omm.join("snapshots/20260101T000000Z/x"), b"x").unwrap();
        fs::create_dir_all(bases.omm.join("custom/skills")).unwrap();
        fs::write(bases.omm.join("custom/skills/mine.md"), b"mine").unwrap();
        fs::write(bases.omm.join("config.json"), b"{}").unwrap();
        store::save(&bases.omm, &ledger).unwrap();
        Fx {
            _dir: dir,
            bases,
            ledger,
        }
    }

    #[test]
    fn plan_sections_order_and_host_steps() {
        let fx = fixture();
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let removed: Vec<String> = p.remove.iter().map(|s| s.key.to_string()).collect();
        // x is removed whole (both files), the theme, the deep workspace file.
        assert_eq!(removed.len(), 4, "{removed:?}");
        assert!(removed.contains(&"muse-config:skills/omm-x/SKILL.md".to_string()));
        assert!(removed.contains(&"muse-config:skills/omm-x/references/r.md".to_string()));
        assert!(removed.contains(&"muse-config:themes/omm-x.tmTheme".to_string()));
        assert!(removed.contains(&"workspace:.omm/skills/deep/a/b.md".to_string()));
        // Deepest first.
        let depths: Vec<usize> = p
            .remove
            .iter()
            .map(|s| s.abs.components().count())
            .collect();
        assert!(depths.windows(2).all(|w| w[0] >= w[1]), "{depths:?}");
        assert!(
            p.remove[0].abs.ends_with(".omm/skills/deep/a/b.md")
                || p.remove[0].abs.ends_with("references/r.md")
        );
        // Preserve: AGENTS.md (edited), y whole (2 files), the settings entry.
        let preserved: Vec<String> = p.preserve.iter().map(|s| s.key.to_string()).collect();
        assert_eq!(preserved.len(), 4, "{preserved:?}");
        assert!(preserved.contains(&"muse-config:AGENTS.md".to_string()));
        assert!(preserved.contains(&"muse-config:skills/omm-y/SKILL.md".to_string()));
        assert!(preserved.contains(&"muse-config:skills/omm-y/references/r.md".to_string()));
        assert!(preserved.contains(&"muse-config:settings.json".to_string()));
        assert_eq!(p.missing.len(), 1);
        // Host steps: skills uninstall x (not y), then registrations reversed.
        assert_eq!(
            p.host_steps,
            vec![
                UndoStep::SkillsUninstall { id: "omm-x".into() },
                UndoStep::RestoreTrust {
                    project: "/ws".into(),
                    prior: None
                },
                UndoStep::RestoreSettingsKey {
                    path: "tui.theme".into(),
                    prior: Some(json!("old"))
                },
                UndoStep::PluginRemove { id: "omm".into() },
                UndoStep::MarketplaceRemove {
                    name: "ohmy".into()
                },
            ]
        );
        // omm state: owned names removed, custom kept, ledger last.
        let names = |v: &Vec<PathBuf>| {
            v.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&p.omm_state.remove), vec!["config.json", "snapshots"]);
        assert_eq!(names(&p.omm_state.keep), vec!["custom"]);
        assert!(p.omm_state.ledger.ends_with(store::LEDGER_FILE));
        assert_eq!(p.residue.len(), 1);
        let text = p.render();
        assert!(text.starts_with("✗ remove (4)"));
        assert!(text.contains("✓ preserve (5)"));
        assert!(text.contains("residue kept"));
        assert!(text.contains("muse plugins remove omm --delete-data"));
        // Force: edited files and the overlay go too; sentinels still do not.
        let f = plan(
            &fx.ledger,
            &fx.bases,
            Options {
                force: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            f.remove.len(),
            7,
            "{:?}",
            f.remove
                .iter()
                .map(|s| s.key.to_string())
                .collect::<Vec<_>>()
        );
        assert!(f.remove.iter().any(|s| s.forced));
        assert_eq!(f.preserve.len(), 1, "only the shared-key entry");
        assert!(f
            .host_steps
            .contains(&UndoStep::SkillsUninstall { id: "omm-y".into() }));
        assert_eq!(
            names(&f.omm_state.remove),
            vec!["config.json", "custom", "snapshots"]
        );
    }

    #[test]
    fn apply_removes_ours_keeps_theirs_and_reports_counts() {
        let fx = fixture();
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        let mut undoer = RecordingUndoer::default();
        // Dry run: the undoer is consulted, nothing changes.
        let dry = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0.1.0".into(),
                dry_run: true,
            },
        )
        .unwrap();
        assert!(dry.dry_run && dry.removed == 4 && dry.registrations_undone == 5);
        assert!(fx.bases.muse_config.join("themes/omm-x.tmTheme").exists());
        assert!(store::ledger_path(&fx.bases.omm).exists());
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0.1.0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert_eq!(
            (r.removed, r.preserved, r.missing, r.registrations_undone),
            (4, 4, 1, 5)
        );
        assert!(r.ledger_removed);
        assert_eq!(undoer.steps, p.host_steps);
        // Ours gone, dirs pruned up to (not including) the base roots.
        assert!(!fx.bases.muse_config.join("themes").exists());
        assert!(!fx.bases.muse_config.join("skills/omm-x").exists());
        assert!(
            fx.bases.muse_config.join("skills").exists(),
            "skills/ still holds omm-y"
        );
        assert!(!fx.bases.workspace.as_ref().unwrap().join(".omm").exists());
        assert!(fx.bases.workspace.as_ref().unwrap().exists());
        assert!(fx.bases.muse_config.exists());
        // Theirs kept.
        assert_eq!(
            fs::read(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            b"rules + mine"
        );
        assert_eq!(
            fs::read(fx.bases.muse_config.join("skills/omm-y/references/r.md")).unwrap(),
            b"ref edited"
        );
        assert_eq!(
            fs::read(fx.bases.muse_config.join("skills/omm-y/SKILL.md")).unwrap(),
            b"skill"
        );
        assert!(fx.bases.muse_config.join("settings.json").exists());
        // omm root: only custom/ remains (and the audit log is gone with the state).
        let left: Vec<String> = fx
            .bases
            .omm
            .read_dir()
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["custom"]);
        assert!(r.dirs_removed >= 4, "{r:?}");
    }

    #[test]
    fn a_failed_host_step_keeps_the_ledger_for_a_rerun() {
        let fx = fixture();
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        let mut undoer = RecordingUndoer {
            fail_matching: vec!["plugins remove".into()],
            ..RecordingUndoer::default()
        };
        let r = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0.1.0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(!r.complete());
        assert_eq!(r.registrations_undone, 4);
        assert_eq!(r.removed, 4, "files are still removed after a host failure");
        assert!(!r.ledger_removed);
        let left = store::load(&fx.bases.omm).unwrap().into_ledger().unwrap();
        assert_eq!(
            left.registrations,
            vec![Registration::MusePlugin {
                id: "omm".into(),
                package_sha256: "d".into(),
                generation_path: "/g".into(),
                approved: vec![]
            }]
        );
        assert!(left
            .entries
            .iter()
            .all(|e| !e.path.as_str().starts_with("themes")));
        assert!(
            fx.bases.omm.join("snapshots").exists(),
            "omm state untouched until complete"
        );
        let lines = audit::read(&fx.bases.omm).unwrap();
        assert!(lines.iter().any(|l| l
            .note
            .as_deref()
            .map(|n| n.starts_with("FAILED"))
            .unwrap_or(false)));
        // Rerun: only the plugin removal is left.
        let p2 = plan(&left, &fx.bases, Options::default()).unwrap();
        assert_eq!(
            p2.host_steps,
            vec![UndoStep::PluginRemove { id: "omm".into() }]
        );
        assert!(p2.remove.is_empty());
        let mut ok = RecordingUndoer::default();
        let r2 = apply(
            &p2,
            left,
            &fx.bases,
            &mut ok,
            &ApplyOptions {
                omm_version: "0.1.0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r2.complete() && r2.ledger_removed);
    }

    #[cfg(unix)]
    #[test]
    fn refuses_roots_escapes_and_sentinels() {
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        // A ledger whose entry resolves onto a refused root through a symlink.
        std::os::unix::fs::symlink(
            fx.bases.home.as_ref().unwrap(),
            fx.bases.muse_config.join("escape"),
        )
        .unwrap();
        ledger.upsert(entry(
            Base::MuseConfig,
            "escape/.bashrc",
            &"0".repeat(64),
            Mechanism::Copy,
        ));
        let p = plan(
            &ledger,
            &fx.bases,
            Options {
                force: true,
                ..Options::default()
            },
        )
        .unwrap();
        // An escape is preserved and named, never a reason to refuse the rest.
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let escaped = p
            .preserve
            .iter()
            .find(|x| x.key.path.as_str() == "escape/.bashrc")
            .unwrap();
        assert!(
            escaped.reason.contains("outside the base"),
            "{}",
            escaped.reason
        );
        // A refused root inside a base — here omm's own root placed under the
        // config root — is the one thing that aborts the plan.
        let mut inside = fx.bases.clone();
        inside.omm = fx.bases.muse_config.join("ommdir");
        fs::create_dir_all(&inside.omm).unwrap();
        let mut bad = fx.ledger.clone();
        bad.upsert(entry(
            Base::MuseConfig,
            "ommdir",
            &"0".repeat(64),
            Mechanism::Copy,
        ));
        let p = plan(&bad, &inside, Options::default()).unwrap();
        assert_eq!(p.refused.len(), 1, "{:?}", p.refused);
        assert!(
            p.refused[0].reason.contains("refused root"),
            "{}",
            p.refused[0].reason
        );
        assert!(p.render().contains("refused (1)"));
        let mut undoer = RecordingUndoer::default();
        assert!(matches!(
            apply(
                &p,
                bad.clone(),
                &inside,
                &mut undoer,
                &ApplyOptions {
                    omm_version: "0".into(),
                    dry_run: false
                }
            ),
            Err(LedgerError::Refused(_))
        ));
        assert!(undoer.steps.is_empty(), "nothing ran");
        assert!(fx.bases.muse_config.join("themes/omm-x.tmTheme").exists());
        // A symlink at a ledgered path is preserved even with --force.
        let mut ledger = fx.ledger.clone();
        let target = fx._dir.path().join("precious");
        fs::write(&target, b"p").unwrap();
        fs::remove_file(fx.bases.muse_config.join("themes/omm-x.tmTheme")).unwrap();
        std::os::unix::fs::symlink(&target, fx.bases.muse_config.join("themes/omm-x.tmTheme"))
            .unwrap();
        // A directory where a file was: also preserved.
        fs::remove_file(fx.bases.muse_config.join("AGENTS.md")).unwrap();
        fs::create_dir(fx.bases.muse_config.join("AGENTS.md")).unwrap();
        ledger.upsert(entry(
            Base::MuseConfig,
            "AGENTS.md",
            &"0".repeat(64),
            Mechanism::Copy,
        ));
        let p = plan(
            &ledger,
            &fx.bases,
            Options {
                force: true,
                ..Options::default()
            },
        )
        .unwrap();
        let theme = p
            .preserve
            .iter()
            .find(|x| x.key.path.as_str() == "themes/omm-x.tmTheme")
            .unwrap();
        assert!(theme.reason.contains("symlink"), "{}", theme.reason);
        assert!(matches!(theme.on_disk, Observed::NonRegular(_)));
        let agents = p
            .preserve
            .iter()
            .find(|x| x.key.path.as_str() == "AGENTS.md")
            .unwrap();
        assert!(agents.reason.contains("directory"), "{}", agents.reason);
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger,
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert!(fx.bases.muse_config.join("themes/omm-x.tmTheme").exists());
        assert_eq!(fs::read(&target).unwrap(), b"p");
        assert!(fx.bases.muse_config.join("AGENTS.md").is_dir());
        // An omm root of `/` or $HOME is refused at planning time.
        let mut bad = fx.bases.clone();
        bad.omm = PathBuf::from("/");
        assert!(matches!(
            plan(&fx.ledger, &bad, Options::default()),
            Err(LedgerError::Refused(_))
        ));
        bad.omm = fx.bases.home.clone().unwrap();
        assert!(matches!(
            plan(&fx.ledger, &bad, Options::default()),
            Err(LedgerError::Refused(_))
        ));
    }

    #[test]
    fn a_gone_base_root_is_missing_never_refused() {
        // A kill between the ledger write and the file write (scenario 15c):
        // the whole config root is absent, so every entry under it resolves
        // to nothing — the uninstall completes instead of refusing.
        let fx = fixture();
        fs::remove_dir_all(&fx.bases.muse_config).unwrap();
        assert!(fx.bases.root_absent(Base::MuseConfig));
        assert!(!fx.bases.root_absent(Base::MuseData));
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let created: Vec<(&str, bool)> = p
            .created
            .iter()
            .map(|c| (c.key.path.as_str(), c.absent_at_plan))
            .collect();
        assert_eq!(created, vec![("settings.json", true)], "{created:?}");
        let mut missing: Vec<&str> = p.missing.iter().map(|m| m.key.path.as_str()).collect();
        missing.sort_unstable();
        assert_eq!(
            missing,
            vec![
                "AGENTS.md",
                "gone.md",
                "skills/omm-x/SKILL.md",
                "skills/omm-x/references/r.md",
                "skills/omm-y/SKILL.md",
                "skills/omm-y/references/r.md",
                "themes/omm-x.tmTheme",
            ],
            "{missing:?}"
        );
        // The workspace root still stands: its file is removed as usual.
        assert_eq!(p.remove.len(), 1, "{:?}", p.remove);
        assert_eq!(p.remove[0].key.path.as_str(), ".omm/skills/deep/a/b.md");
    }

    #[test]
    fn omm_state_removes_atomic_write_droppings() {
        // A run killed between the temp create and the rename (scenario 26)
        // leaves `.<name>.omm-tmp-<random>` behind: omm's own residue, so the
        // uninstall removes it with the rest instead of keeping the root.
        let fx = fixture();
        fs::write(fx.bases.omm.join(".omm.lock.json.omm-tmp-ABC123"), b"stale").unwrap();
        fs::write(fx.bases.omm.join("notes.txt"), b"mine").unwrap();
        let st = omm_state(&fx.bases, false).unwrap();
        let names = |v: &Vec<PathBuf>| {
            v.iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        assert!(
            names(&st.remove).contains(&".omm.lock.json.omm-tmp-ABC123".to_string()),
            "{:?}",
            names(&st.remove)
        );
        assert!(
            names(&st.keep).contains(&"notes.txt".to_string()),
            "{:?}",
            names(&st.keep)
        );
        assert!(
            !names(&st.keep)
                .iter()
                .any(|n| omm_host::fsx::is_tmp_name(n)),
            "{:?}",
            names(&st.keep)
        );
    }

    #[test]
    fn sweep_tmps_clears_only_omm_droppings() {
        // A killed write_atomic leaves `.<name>.omm-tmp-<random>` in a
        // managed dir; the prune calls this so the dropping never blocks
        // it, while real files (even dotfiles) are left alone.
        let dir = tempfile::tempdir().unwrap();
        let dir = dir.path();
        fs::write(dir.join(".omm-carbon.tmTheme.omm-tmp-ABC123"), b"stale").unwrap();
        fs::write(dir.join(".omm-tmp-"), b"not a dropping").unwrap();
        fs::write(dir.join(".keepme"), b"mine").unwrap();
        fs::write(dir.join("omm-slate.tmTheme"), b"managed").unwrap();
        assert_eq!(sweep_tmps(dir), 1);
        assert!(!dir.join(".omm-carbon.tmTheme.omm-tmp-ABC123").exists());
        assert!(dir.join(".omm-tmp-").exists());
        assert!(dir.join(".keepme").exists());
        assert!(dir.join("omm-slate.tmTheme").exists());
        assert_eq!(sweep_tmps(dir), 0);
        // Missing dirs are not an error: nothing to sweep.
        assert_eq!(sweep_tmps(&dir.join("gone")), 0);
    }

    #[test]
    fn a_created_shared_file_is_removed_only_when_empty_after_the_restores() {
        assert!(is_empty_shared_document(b"{\"schema_version\":1}"));
        assert!(is_empty_shared_document(
            b"{\"schema_version\":1,\"projects\":{},\"tui\":{}}"
        ));
        assert!(!is_empty_shared_document(
            b"{\"schema_version\":1,\"provider\":\"echo\"}"
        ));
        assert!(!is_empty_shared_document(b"[]"));
        assert!(!is_empty_shared_document(b"nope"));
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        // settings.json (already `{"schema_version":1}`) and trust.json created by omm.
        let mut created = entry(
            Base::MuseConfig,
            "settings.json",
            "n/a",
            Mechanism::SettingsPatch,
        );
        created.kind = Kind::SettingsKey;
        created.class = Class::Seeded;
        ledger.upsert(created);
        write(
            &fx.bases,
            Base::MuseConfig,
            "trust.json",
            b"{\"schema_version\":1,\"projects\":{\"/ws\":{\"decision\":\"trusted\"}}}\n",
        );
        let mut trust = entry(Base::MuseConfig, "trust.json", "n/a", Mechanism::TrustMerge);
        trust.kind = Kind::Trust;
        trust.class = Class::Seeded;
        ledger.upsert(trust);
        let p = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert_eq!(p.created.len(), 2, "{:?}", p.created);
        assert!(p.render().contains("created by omm"));
        // The recording undoer restores nothing, so trust.json still holds a
        // project and is preserved; settings.json is empty and goes.
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger,
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert!(!fx.bases.muse_config.join("settings.json").exists());
        assert!(fx.bases.muse_config.join("trust.json").exists());
        assert_eq!(r.removed, 5);
        assert_eq!(r.preserved, 4, "AGENTS.md, y x2, trust.json");
    }

    #[test]
    fn a_file_changed_between_plan_and_apply_is_left_in_place() {
        let fx = fixture();
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        fs::write(
            fx.bases.muse_config.join("themes/omm-x.tmTheme"),
            b"changed after plan",
        )
        .unwrap();
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(!r.complete());
        assert!(r
            .errors
            .iter()
            .any(|e| e.contains("changed since the plan")));
        assert_eq!(
            fs::read(fx.bases.muse_config.join("themes/omm-x.tmTheme")).unwrap(),
            b"changed after plan"
        );
        assert!(store::ledger_path(&fx.bases.omm).exists());
    }

    fn markers() -> crate::rules::Markers {
        crate::rules::Markers {
            managed_start: "<!-- omm:managed-start -->".into(),
            managed_end: "<!-- omm:managed-end -->".into(),
            user_start: "<!-- omm:user-start -->".into(),
            user_end: "<!-- omm:user-end -->".into(),
        }
    }

    /// A rendered rules document: the managed block plus `user` between the
    /// user markers, as the installer seeds it from the template.
    fn rules_doc(user: &str) -> String {
        format!(
            "# Rules\n\n<!-- omm:managed-start -->\n- managed one\n<!-- omm:managed-end -->\n\n<!-- omm:user-start -->\n{}<!-- omm:user-end -->\n",
            crate::rules::normalise(user)
        )
    }

    /// The seed's frame: its text outside the managed block, `user` being
    /// the template's default user region.
    fn seed_frame(doc: &str) -> crate::rules::Frame {
        markers().parts(doc).unwrap().frame()
    }

    /// A rules entry for `doc` on disk with `frame` recorded; `replaced`
    /// records a pre-existing file's bytes.
    fn rules_entry_framed(
        bases: &Bases,
        ledger: &mut Ledger,
        doc: &str,
        replaced: Option<&[u8]>,
        frame: &crate::rules::Frame,
    ) {
        let sha = write(bases, Base::MuseConfig, "AGENTS.md", doc.as_bytes());
        let managed = markers().managed_sha256(doc).unwrap();
        let mut e = entry(Base::MuseConfig, "AGENTS.md", &sha, Mechanism::Copy);
        e.kind = Kind::Rules;
        e.class = Class::Seeded;
        e.prior = Some(rules_prior(
            &managed,
            replaced.map(|b| replaced_prior(&hash::sha256_bytes(b), None, b)),
            Some(frame),
        ));
        ledger.upsert(e);
    }

    /// A rules entry for a template seed: the frame is the seed's own.
    fn rules_entry(bases: &Bases, ledger: &mut Ledger, doc: &str, replaced: Option<&[u8]>) {
        rules_entry_framed(bases, ledger, doc, replaced, &seed_frame(doc));
    }

    fn rules_opts() -> Options {
        Options {
            rules: Some(markers()),
            ..Options::default()
        }
    }

    fn run_plan(fx: &Fx, ledger: &Ledger, opts: Options) -> (Plan, Report) {
        let p = plan(ledger, &fx.bases, opts).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        (p, r)
    }

    #[test]
    fn a_pre_existing_rules_file_comes_back_byte_for_byte() {
        // Gate 1: `pre` — a user AGENTS.md taken over at install was
        // unlinked at uninstall, its only backup gone with $OMM. Round 5:
        // the block is inserted at the top of such a file and the file's
        // own bytes are never touched.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let original = b"my own rules\n- never delete me";
        let m = markers();
        let seeded = m.insert_managed("my own rules\n- never delete me", "- managed one\n");
        assert!(seeded.ends_with("my own rules\n- never delete me"));
        rules_entry_framed(
            &fx.bases,
            &mut ledger,
            &seeded,
            Some(original),
            &m.inserted_frame(),
        );
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert_eq!(p.rules.len(), 1, "{:?}", p.rules);
        assert!(matches!(p.rules[0].action, RulesAction::Restore { .. }));
        assert!(p
            .render()
            .contains("restored to the file that was there before omm"));
        assert_eq!(r.rules_rewritten, 1);
        assert_eq!(
            fs::read(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            original,
            "byte-identical, no trailing newline added"
        );
        // The same file with a rule the user appended after install: the
        // block goes, the original and the rule stay (never the stale
        // original alone, never removed).
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry_framed(
            &fx.bases,
            &mut ledger,
            &seeded,
            Some(original),
            &m.inserted_frame(),
        );
        fs::write(
            fx.bases.muse_config.join("AGENTS.md"),
            format!("{seeded}\n- appended later\n"),
        )
        .unwrap();
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert!(matches!(p.rules[0].action, RulesAction::Rewrite { .. }));
        assert_eq!(r.rules_rewritten, 1);
        assert_eq!(
            fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            "my own rules\n- never delete me\n- appended later\n"
        );
        // Forced with the markers removed by hand: the pre-existing file
        // comes back rather than going.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry_framed(
            &fx.bases,
            &mut ledger,
            &seeded,
            Some(original),
            &m.inserted_frame(),
        );
        fs::write(
            fx.bases.muse_config.join("AGENTS.md"),
            b"- managed one\nmine",
        )
        .unwrap();
        let (p, _) = run_plan(
            &fx,
            &ledger,
            Options {
                force: true,
                ..rules_opts()
            },
        );
        assert!(matches!(p.rules[0].action, RulesAction::Restore { .. }));
        assert_eq!(
            fs::read(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            original
        );
        // The round trip survives hex-encoded (non-UTF-8) originals too.
        let raw = b"\xff\xfe not utf-8\n";
        let prior = replaced_prior("s", None, raw);
        assert!(prior.get(REPLACED_HEX).is_some() && prior.get(REPLACED_TEXT).is_none());
        assert_eq!(replaced_bytes(&prior).unwrap(), raw);
        let text = replaced_prior("s", Some(Path::new("/b")), b"plain\n");
        assert_eq!(text[REPLACED_BACKUP], "/b");
        assert_eq!(replaced_bytes(&text).unwrap(), b"plain\n");
        assert_eq!(replaced_bytes(&Value::Null), None);
    }

    #[test]
    fn a_user_region_edit_keeps_the_region_and_drops_the_managed_block() {
        // Gate 1: `rec3` — after a --no-plugin uninstall the whole file stayed,
        // managed rules pointing at the removed plugin included.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let seeded = rules_doc("");
        rules_entry(&fx.bases, &mut ledger, &seeded, None);
        // The user adds a rule inside the markers after install.
        let edited = rules_doc("- MY PERSONAL RULE\n");
        fs::write(fx.bases.muse_config.join("AGENTS.md"), &edited).unwrap();
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert!(
            matches!(p.rules[0].action, RulesAction::Rewrite { .. }),
            "{:?}",
            p.rules
        );
        assert!(p.render().contains("managed block removed"));
        assert_eq!(r.rules_rewritten, 1);
        let left = fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap();
        assert_eq!(left, "- MY PERSONAL RULE\n");
        assert!(!left.contains("managed"));
        // A legacy entry that recorded no frame (written before frames
        // were): nothing outside the block is known to be omm's, so the
        // title and blank lines stay with the user's region.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry(&fx.bases, &mut ledger, &rules_doc(""), None);
        let mut e = ledger
            .find(Base::MuseConfig, &RelPath::new("AGENTS.md").unwrap())
            .unwrap()
            .clone();
        e.prior.as_mut().unwrap()[RULES_PRIOR_FRAME] = Value::Null;
        ledger.upsert(e);
        fs::write(fx.bases.muse_config.join("AGENTS.md"), rules_doc("mine\n")).unwrap();
        let (_, r) = run_plan(&fx, &ledger, rules_opts());
        assert_eq!(r.rules_rewritten, 1);
        assert_eq!(
            fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            "# Rules\n\n\n\nmine\n\n"
        );
    }

    #[test]
    fn text_outside_the_markers_survives_install_edits_and_uninstall_byte_for_byte() {
        // Gate 1 round 5 (decision H2): a title prepended above the block and
        // a rule appended after the user-end marker were dropped by every
        // rewrite (reinstall, update, uninstall) and reported as preserved.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let placeholder = "<!-- Your rules. -->\n";
        let seed = rules_doc(placeholder);
        rules_entry(&fx.bases, &mut ledger, &seed, None);
        let m = markers();
        // The user's edits, outside both regions.
        let edited = format!("MY LEADING TITLE\n{seed}MY TRAILING RULE\n");
        // What install/update do to such a file: the managed region
        // replaced in place, nothing else touched.
        let refreshed = m.replace_managed(&edited, "- managed two\n").unwrap();
        assert!(refreshed.starts_with(
            "MY LEADING TITLE\n# Rules\n\n<!-- omm:managed-start -->\n- managed two\n"
        ));
        assert!(refreshed.ends_with("<!-- omm:user-end -->\nMY TRAILING RULE\n"));
        assert_eq!(
            m.replace_managed(&edited, "- managed one\n").unwrap(),
            edited
        );
        fs::write(fx.bases.muse_config.join("AGENTS.md"), &edited).unwrap();
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert!(
            matches!(p.rules[0].action, RulesAction::Rewrite { .. }),
            "{:?}",
            p.rules
        );
        assert!(p.render().contains("everything else kept byte for byte"));
        assert_eq!(r.rules_rewritten, 1);
        let left = fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap();
        assert_eq!(left, "MY LEADING TITLE\nMY TRAILING RULE\n");
        assert!(!left.contains("omm:"), "every marker line is gone: {left}");
        // The same edits plus a rule inside the user region: all three stay.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry(&fx.bases, &mut ledger, &seed, None);
        let edited = format!(
            "MY LEADING TITLE\n{}MY TRAILING RULE\n",
            rules_doc(&format!("{placeholder}- inside\n"))
        );
        fs::write(fx.bases.muse_config.join("AGENTS.md"), &edited).unwrap();
        let (_, r) = run_plan(&fx, &ledger, rules_opts());
        assert_eq!(r.rules_rewritten, 1);
        assert_eq!(
            fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            format!("MY LEADING TITLE\n{placeholder}- inside\nMY TRAILING RULE\n")
        );
        // Only the title, on a seed whose managed region a later update
        // refreshed in place (the entry then records the new managed sha).
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let refreshed_seed = m.replace_managed(&seed, "- managed two\n").unwrap();
        rules_entry(&fx.bases, &mut ledger, &refreshed_seed, None);
        fs::write(
            fx.bases.muse_config.join("AGENTS.md"),
            format!("MY LEADING TITLE\n{refreshed_seed}"),
        )
        .unwrap();
        let (_, r) = run_plan(&fx, &ledger, rules_opts());
        assert_eq!(r.rules_rewritten, 1);
        assert_eq!(
            fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            "MY LEADING TITLE\n"
        );
    }

    #[test]
    fn a_failed_host_step_defers_the_shared_files_and_keeps_their_entries() {
        // Gate 1 round 5 (M3): with a malformed trust.json every restore that
        // runs the host's validator fails; the emptied shared files must not
        // be finalised (nor their entries dropped) until the rerun.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let mut created = entry(
            Base::MuseConfig,
            "settings.json",
            "n/a",
            Mechanism::SettingsPatch,
        );
        created.kind = Kind::SettingsKey;
        created.class = Class::Seeded;
        ledger.upsert(created);
        write(
            &fx.bases,
            Base::MuseConfig,
            "trust.json",
            b"{\"schema_version\":1,\"projects\":{\"/ws\":{\"trust\":\"typo\"}}",
        );
        let mut trust = entry(Base::MuseConfig, "trust.json", "n/a", Mechanism::TrustMerge);
        trust.kind = Kind::Trust;
        trust.class = Class::Seeded;
        ledger.upsert(trust);
        let p = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert_eq!(p.created.len(), 2);
        let mut undoer = RecordingUndoer {
            fail_matching: vec!["trust entry".into()],
            ..RecordingUndoer::default()
        };
        let r = apply(
            &p,
            ledger,
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(!r.complete());
        assert!(
            fx.bases.muse_config.join("settings.json").exists(),
            "the emptied settings.json waits for the rerun"
        );
        let left = store::load(&fx.bases.omm).unwrap().into_ledger().unwrap();
        for path in ["settings.json", "trust.json"] {
            assert!(
                left.find(Base::MuseConfig, &RelPath::new(path).unwrap())
                    .is_some(),
                "{path} entry kept for the rerun: {:?}",
                left.entries
            );
        }
        assert!(left
            .registrations
            .iter()
            .any(|r| matches!(r, Registration::Trust { .. })));
        let lines = audit::read(&fx.bases.omm).unwrap();
        assert!(lines.iter().any(|l| l
            .note
            .as_deref()
            .map(|n| n.starts_with("deferred"))
            .unwrap_or(false)));
        // The rerun (the host repaired): the trust entry restored, the
        // created files finalised — trust.json is not JSON, so it is left
        // with an error and its entry kept; settings.json goes.
        let p2 = plan(&left, &fx.bases, Options::default()).unwrap();
        let mut ok = RecordingUndoer::default();
        let r2 = apply(
            &p2,
            left,
            &fx.bases,
            &mut ok,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(!fx.bases.muse_config.join("settings.json").exists());
        assert!(fx.bases.muse_config.join("trust.json").exists());
        assert!(
            r2.errors.iter().any(|e| e.contains("not a JSON document")),
            "{:?}",
            r2.errors
        );
        let left2 = store::load(&fx.bases.omm).unwrap().into_ledger().unwrap();
        assert!(left2
            .find(Base::MuseConfig, &RelPath::new("trust.json").unwrap())
            .is_some());
        assert!(left2
            .find(Base::MuseConfig, &RelPath::new("settings.json").unwrap())
            .is_none());
    }

    #[test]
    fn an_untouched_seeded_rules_file_is_removed_and_a_managed_edit_preserved() {
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry(&fx.bases, &mut ledger, &rules_doc(""), None);
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert!(p.rules.is_empty());
        assert!(p.remove.iter().any(|s| s.key.path.as_str() == "AGENTS.md"));
        assert!(r.complete());
        assert!(!fx.bases.muse_config.join("AGENTS.md").exists());
        // The managed region edited by hand: not ours any more, kept whole.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry(&fx.bases, &mut ledger, &rules_doc("mine\n"), None);
        let hand = rules_doc("mine\n").replace("- managed one", "- managed one, edited");
        fs::write(fx.bases.muse_config.join("AGENTS.md"), &hand).unwrap();
        let (p, _) = run_plan(&fx, &ledger, rules_opts());
        assert!(p.rules.is_empty());
        let kept = p
            .preserve
            .iter()
            .find(|x| x.key.path.as_str() == "AGENTS.md")
            .unwrap();
        assert!(kept.reason.contains("managed region"), "{}", kept.reason);
        assert_eq!(
            fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            hand
        );
        // Without markers in the options a rules entry is a plain copy (the
        // legacy behaviour): the edited file is preserved by sha alone.
        let (p, _) = run_plan(&fx, &ledger, Options::default());
        assert!(p.rules.is_empty());
        assert!(p
            .preserve
            .iter()
            .any(|x| x.key.path.as_str() == "AGENTS.md"));
    }

    #[test]
    fn an_untouched_seed_whose_user_region_is_the_template_placeholder_is_removed() {
        // Gate 1 regression: the shipped template's user region is not empty
        // (a placeholder comment), so a byte-for-byte untouched seed was
        // "rewritten" to that placeholder instead of removed, and R5 failed
        // on AGENTS.md after every plain install → uninstall.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let placeholder = "<!-- Your rules. omm preserves everything between the markers. -->\n";
        rules_entry(&fx.bases, &mut ledger, &rules_doc(placeholder), None);
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert!(p.rules.is_empty(), "{:?}", p.rules);
        assert!(p.remove.iter().any(|s| s.key.path.as_str() == "AGENTS.md"));
        assert!(r.removed >= 1 && r.rules_rewritten == 0, "{r:?}");
        assert!(!fx.bases.muse_config.join("AGENTS.md").exists());
        // The same seed with a rule added below the placeholder keeps the
        // whole user region (placeholder included: it is inside the user's
        // region) and drops the managed block.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        rules_entry(&fx.bases, &mut ledger, &rules_doc(placeholder), None);
        let edited = rules_doc(&format!("{placeholder}- mine\n"));
        fs::write(fx.bases.muse_config.join("AGENTS.md"), &edited).unwrap();
        let (p, r) = run_plan(&fx, &ledger, rules_opts());
        assert!(matches!(p.rules[0].action, RulesAction::Rewrite { .. }));
        assert_eq!(r.rules_rewritten, 1);
        assert_eq!(
            fs::read_to_string(fx.bases.muse_config.join("AGENTS.md")).unwrap(),
            format!("{placeholder}- mine\n")
        );
    }

    #[test]
    fn a_settings_key_or_trust_entry_edited_since_omm_set_it_is_preserved() {
        // Gate 1: `theme` — a hand edit of tui.theme after `omm theme` was
        // clobbered by the restore to prior.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let settings = fx.bases.muse_config.join("settings.json");
        fs::write(
            &settings,
            b"{\"schema_version\":1,\"tui\":{\"theme\":\"ayu-dark\"},\"provider\":\"meta\"}\n",
        )
        .unwrap();
        let trust = fx.bases.muse_config.join("trust.json");
        fs::write(
            &trust,
            b"{\"schema_version\":1,\"projects\":{\"/ws\":{\"decision\":\"untrusted\"},\"/other\":{\"decision\":\"trusted\"}}}\n",
        )
        .unwrap();
        ledger.registrations.clear();
        ledger.record_settings_key("tui.theme", None, Some(json!("custom:omm-carbon")), None);
        ledger.record_settings_key("provider", None, Some(json!("meta")), None);
        ledger.record_trust("/ws", None, Some(json!({"decision": "trusted"})));
        ledger.record_trust("/other", None, Some(json!({"decision": "trusted"})));
        let opts = Options {
            settings_file: Some(settings.clone()),
            trust_file: Some(trust.clone()),
            ..Options::default()
        };
        let p = plan(&ledger, &fx.bases, opts.clone()).unwrap();
        // provider and /other still hold what omm wrote: restored. tui.theme
        // and /ws were edited: preserved, named, with the --force hint.
        assert_eq!(
            p.host_steps,
            vec![
                UndoStep::SkillsUninstall { id: "omm-x".into() },
                UndoStep::RestoreTrust {
                    project: "/other".into(),
                    prior: None
                },
                UndoStep::RestoreSettingsKey {
                    path: "provider".into(),
                    prior: None
                },
            ]
        );
        let reasons: Vec<&str> = p.preserve.iter().map(|x| x.reason.as_str()).collect();
        assert!(
            reasons.iter().any(
                |r| r.contains("settings key tui.theme edited since omm set it")
                    && r.contains("--force")
            ),
            "{reasons:?}"
        );
        assert!(
            reasons
                .iter()
                .any(|r| r.contains("trust entry /ws edited since omm set it")),
            "{reasons:?}"
        );
        assert!(p.render().contains("tui.theme edited since omm set it"));
        // --force restores everything; a legacy registration without `value`
        // is restored unconditionally either way.
        ledger.register(Registration::SettingsKey {
            path: "run.workflow_trigger_mode".into(),
            prior: Some(json!("off")),
            value: None,
            profile: None,
        });
        let forced = plan(
            &ledger,
            &fx.bases,
            Options {
                force: true,
                ..opts.clone()
            },
        )
        .unwrap();
        // (--force also removes the edited skill y: one more `skills uninstall`.)
        assert_eq!(forced.host_steps.len(), 7, "{:?}", forced.host_steps);
        let lenient = plan(&ledger, &fx.bases, opts).unwrap();
        assert_eq!(lenient.host_steps.len(), 4, "{:?}", lenient.host_steps);
        // No file paths given: the legacy unconditional behaviour.
        let legacy = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert_eq!(legacy.host_steps.len(), 6);
    }

    fn shared_entry(
        bases: &Bases,
        ledger: &mut Ledger,
        name: &str,
        kind: Kind,
        mechanism: Mechanism,
        original: &[u8],
        mode: u32,
    ) {
        use std::os::unix::fs::PermissionsExt;
        let p = bases.muse_config.join(name);
        fs::write(&p, original).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
        let o = Original::read(&p).unwrap().unwrap();
        assert_eq!(o.mode, Some(mode));
        let mut e = entry(Base::MuseConfig, name, "n/a", mechanism);
        e.kind = kind;
        e.class = Class::SharedKey;
        e.prior = Some(o.to_prior());
        ledger.upsert(e);
    }

    fn mode_of(p: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(p).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn a_shared_file_comes_back_byte_for_byte_when_the_document_equals_the_pre_omm_one() {
        use std::os::unix::fs::PermissionsExt;
        // Gate 1: a compact 0600 settings.json with two keys the host does
        // not type came back pretty-printed, 0644, without them; trust.json
        // pretty-printed.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        ledger.registrations.clear();
        let settings_original =
            br#"{"schema_version":1,"provider":"meta","my_top":{"keep":true},"tui":{"theme":"ayu-dark","my_extra":42}}"#;
        shared_entry(
            &fx.bases,
            &mut ledger,
            "settings.json",
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
            settings_original,
            0o600,
        );
        let trust_original =
            br#"{"schema_version":1,"projects":{"/other":{"decision":"trusted"}}}"#;
        shared_entry(
            &fx.bases,
            &mut ledger,
            "trust.json",
            Kind::Trust,
            Mechanism::TrustMerge,
            trust_original,
            0o644,
        );
        // What the host's rewrite and the key restores leave: the typed
        // keys, pretty-printed, at the host's mode; the untyped ones gone.
        let settings = fx.bases.muse_config.join("settings.json");
        fs::write(&settings, b"{\n  \"schema_version\": 1,\n  \"provider\": \"meta\",\n  \"tui\": {\n    \"theme\": \"ayu-dark\"\n  }\n}\n").unwrap();
        fs::set_permissions(&settings, fs::Permissions::from_mode(0o644)).unwrap();
        let trust = fx.bases.muse_config.join("trust.json");
        fs::write(&trust, b"{\n  \"schema_version\": 1,\n  \"projects\": {\n    \"/other\": {\n      \"decision\": \"trusted\"\n    }\n  }\n}\n").unwrap();
        let p = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert_eq!(p.shared.len(), 2, "{:?}", p.shared);
        assert!(p.render().contains("back to its pre-omm bytes"));
        assert!(!p
            .preserve
            .iter()
            .any(|x| x.key.path.as_str() == "settings.json"));
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert_eq!(r.shared_restored, 2, "{r:?}");
        assert_eq!(fs::read(&settings).unwrap(), settings_original);
        assert_eq!(mode_of(&settings), 0o600);
        assert_eq!(fs::read(&trust).unwrap(), trust_original);
        assert_eq!(mode_of(&trust), 0o644);
        assert!(r.ledger_removed);
        // Edited since (a typed key changed): kept, but the untyped keys the
        // rewrite dropped are merged back and the mode re-applied.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        ledger.registrations.clear();
        shared_entry(
            &fx.bases,
            &mut ledger,
            "settings.json",
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
            settings_original,
            0o600,
        );
        let settings = fx.bases.muse_config.join("settings.json");
        fs::write(
            &settings,
            b"{\"schema_version\":1,\"provider\":\"echo\",\"tui\":{\"theme\":\"ayu-dark\"}}\n",
        )
        .unwrap();
        fs::set_permissions(&settings, fs::Permissions::from_mode(0o644)).unwrap();
        let p = plan(&ledger, &fx.bases, Options::default()).unwrap();
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert_eq!((r.shared_restored, r.shared_kept), (0, 1), "{r:?}");
        let doc: Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
        assert_eq!(doc["provider"], "echo", "the user's edit stays");
        assert_eq!(doc["my_top"]["keep"], true, "the untyped key is back");
        assert_eq!(doc["tui"]["my_extra"], 42);
        assert_eq!(doc["tui"]["theme"], "ayu-dark");
        assert_eq!(mode_of(&settings), 0o600);
        // A key the user added after install (present at plan time) also
        // survives the host's rewrite between plan and the shared step.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        ledger.registrations.clear();
        shared_entry(
            &fx.bases,
            &mut ledger,
            "settings.json",
            Kind::SettingsKey,
            Mechanism::SettingsPatch,
            b"{\"schema_version\":1}",
            0o644,
        );
        let settings = fx.bases.muse_config.join("settings.json");
        fs::write(&settings, b"{\"schema_version\":1,\"later\":true}").unwrap();
        let p = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert_eq!(
            p.shared[0].untyped_now,
            vec![(vec!["later".to_string()], json!(true))]
        );
        // The host's rewrite (simulated) drops it before the shared step runs.
        fs::write(&settings, b"{\n  \"schema_version\": 1\n}\n").unwrap();
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        let doc: Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
        assert_eq!(doc["later"], true);
    }

    #[test]
    fn a_shared_file_the_user_removed_is_neither_restored_into_nor_recreated() {
        // Gate 1: `rm settings.json trust.json` after install; uninstall
        // wrote `{"schema_version":1}` and `{"schema_version":1,"projects":{}}`
        // back, and the host's `plugins remove` recreated settings.json too.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let settings = fx.bases.muse_config.join("settings.json");
        fs::remove_file(&settings).unwrap();
        let trust = fx.bases.muse_config.join("trust.json");
        let mut created = entry(Base::MuseConfig, "trust.json", "n/a", Mechanism::TrustMerge);
        created.kind = Kind::Trust;
        created.class = Class::Seeded;
        ledger.upsert(created);
        let opts = Options {
            settings_file: Some(settings.clone()),
            trust_file: Some(trust.clone()),
            ..Options::default()
        };
        let p = plan(&ledger, &fx.bases, opts).unwrap();
        // Neither restore runs; both registrations are dropped and named.
        assert!(
            !p.host_steps.iter().any(|s| matches!(
                s,
                UndoStep::RestoreSettingsKey { .. } | UndoStep::RestoreTrust { .. }
            )),
            "{:?}",
            p.host_steps
        );
        assert_eq!(p.dropped.len(), 2, "{:?}", p.dropped);
        assert!(p.dropped.iter().all(|d| d.reason.contains("is gone")));
        assert!(p
            .render()
            .contains("skipped: restore settings key tui.theme"));
        assert!(
            p.created.iter().all(|c| c.absent_at_plan),
            "{:?}",
            p.created
        );
        assert_eq!(p.created.len(), 2);
        // The host's `plugins remove --delete-data` recreates settings.json empty.
        let mut undoer = RecordingUndoer {
            side_effects: vec![(
                "plugins remove".into(),
                settings.clone(),
                b"{\"schema_version\":1}\n".to_vec(),
            )],
            ..RecordingUndoer::default()
        };
        let r = apply(
            &p,
            ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert_eq!(r.registrations_dropped, 2);
        assert!(
            !settings.exists(),
            "recreated by a host step, removed again"
        );
        assert!(!trust.exists());
        assert!(r.ledger_removed);
        // Not a shared file but still a registration: without the file
        // paths in the options (legacy callers) every key is restored.
        let legacy = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert!(legacy.dropped.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn an_entry_escaping_through_a_symlinked_ancestor_is_preserved_not_refused() {
        // Gate 1: `own` — one planted symlink blocked the whole uninstall.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        let outside = fx._dir.path().join("mydir");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("SKILL.md"), b"theirs").unwrap();
        // The managed skill omm-x replaced by a symlink to the user's dir.
        let x = fx.bases.muse_config.join("skills/omm-x");
        fs::remove_dir_all(&x).unwrap();
        std::os::unix::fs::symlink(&outside, &x).unwrap();
        // And a plain copy whose ancestor now escapes.
        std::os::unix::fs::symlink(
            fx.bases.home.as_ref().unwrap(),
            fx.bases.muse_config.join("escape"),
        )
        .unwrap();
        ledger.upsert(entry(
            Base::MuseConfig,
            "escape/.bashrc",
            &"0".repeat(64),
            Mechanism::Copy,
        ));
        let p = plan(&ledger, &fx.bases, Options::default()).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        let escaped: Vec<&Preserved> = p
            .preserve
            .iter()
            .filter(|x| x.reason.contains("outside the base"))
            .collect();
        assert_eq!(escaped.len(), 3, "{:?}", p.preserve);
        assert!(p.render().contains("outside the base"));
        // omm-x is kept whole: no `skills uninstall` for it, y as before.
        assert!(!p
            .host_steps
            .contains(&UndoStep::SkillsUninstall { id: "omm-x".into() }));
        // Everything else still goes.
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger,
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert!(!fx.bases.muse_config.join("themes").exists());
        assert!(x.is_symlink());
        assert_eq!(fs::read(outside.join("SKILL.md")).unwrap(), b"theirs");
        assert!(r.ledger_removed);
    }

    #[test]
    fn registrations_the_host_no_longer_holds_are_dropped_as_already_gone() {
        // Gate 1 decision B (round 4, U/W3/W4): a plugin, marketplace or
        // skill removed by hand wedged every rerun of `omm uninstall`.
        let fx = fixture();
        let opts = Options {
            host: Some(HostView {
                plugins: BTreeSet::new(),
                marketplaces: BTreeSet::new(),
                skills: ["omm-y".to_string()].into_iter().collect(),
            }),
            ..Options::default()
        };
        let p = plan(&fx.ledger, &fx.bases, opts).unwrap();
        assert!(
            !p.host_steps.iter().any(|s| matches!(
                s,
                UndoStep::PluginRemove { .. }
                    | UndoStep::MarketplaceRemove { .. }
                    | UndoStep::SkillsUninstall { .. }
            )),
            "{:?}",
            p.host_steps
        );
        assert_eq!(p.dropped.len(), 3, "{:?}", p.dropped);
        assert!(p
            .dropped
            .iter()
            .all(|d| d.reason.contains("already gone from the host")));
        assert!(p.render().contains("already gone from the host"));
        // The files omm ledgered for omm-x still go.
        assert!(p
            .remove
            .iter()
            .any(|s| s.key.path.as_str() == "skills/omm-x/SKILL.md"));
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert_eq!((r.registrations_dropped, r.registrations_undone), (3, 2));
        assert!(r.ledger_removed);
        assert!(!fx.bases.muse_config.join("skills/omm-x").exists());
        // Without a host view every registration is planned; a host that
        // answers "not installed" at apply time is done, not failed — the
        // registration goes with the reason audited (a later failing step
        // keeps the ledger here so the audit can be read).
        let fx = fixture();
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        let mut undoer = RecordingUndoer {
            already_gone: vec!["marketplace remove".into(), "plugins remove".into()],
            fail_matching: vec!["trust entry".into()],
            ..RecordingUndoer::default()
        };
        let r = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(!r.complete());
        assert_eq!(
            (r.registrations_undone, r.registrations_dropped),
            (2, 2),
            "{r:?}"
        );
        let left = store::load(&fx.bases.omm).unwrap().into_ledger().unwrap();
        assert!(
            left.registrations
                .iter()
                .all(|reg| matches!(reg, Registration::Trust { .. })),
            "{:?}",
            left.registrations
        );
        let lines = audit::read(&fx.bases.omm).unwrap();
        assert!(
            lines.iter().any(|l| l
                .note
                .as_deref()
                .map(|n| n.contains("already gone") && n.contains("registration dropped"))
                .unwrap_or(false)),
            "{lines:?}"
        );
        // The rerun has only the failed step left.
        let p2 = plan(&left, &fx.bases, Options::default()).unwrap();
        assert_eq!(p2.host_steps.len(), 1, "{:?}", p2.host_steps);
    }

    #[cfg(unix)]
    #[test]
    fn an_in_base_ancestor_symlink_to_identical_user_files_is_preserved() {
        // Gate 1 decision C (round 4, C2): the managed skill dir replaced by
        // a symlink to the user's own directory holding byte-identical
        // files — the plan listed the user's canonical path under remove.
        let fx = fixture();
        let store = fx.bases.muse_config.join("skills");
        let mine = store.join("my-x");
        fs::create_dir_all(mine.join("references")).unwrap();
        fs::write(mine.join("SKILL.md"), b"skill").unwrap();
        fs::write(mine.join("references/r.md"), b"ref").unwrap();
        fs::write(mine.join("mine.md"), b"mine").unwrap();
        fs::remove_dir_all(store.join("omm-x")).unwrap();
        std::os::unix::fs::symlink("my-x", store.join("omm-x")).unwrap();
        let p = plan(&fx.ledger, &fx.bases, Options::default()).unwrap();
        assert!(p.refused.is_empty(), "{:?}", p.refused);
        assert!(
            !p.remove
                .iter()
                .any(|s| s.abs.to_string_lossy().contains("my-x")),
            "{:?}",
            p.remove
        );
        let kept: Vec<&Preserved> = p
            .preserve
            .iter()
            .filter(|x| x.key.path.as_str().starts_with("skills/omm-x/"))
            .collect();
        assert_eq!(kept.len(), 2, "{:?}", p.preserve);
        assert!(
            kept.iter()
                .all(|x| x.reason.contains("symlink") && x.reason.contains("R2")),
            "{kept:?}"
        );
        assert!(!p
            .host_steps
            .contains(&UndoStep::SkillsUninstall { id: "omm-x".into() }));
        // --force does not change it: a sentinel is never removed.
        let f = plan(
            &fx.ledger,
            &fx.bases,
            Options {
                force: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(!f
            .remove
            .iter()
            .any(|s| s.key.path.as_str().starts_with("skills/omm-x/")));
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            fx.ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: false,
            },
        )
        .unwrap();
        assert!(r.complete(), "{:?}", r.errors);
        assert_eq!(fs::read(mine.join("SKILL.md")).unwrap(), b"skill");
        assert_eq!(fs::read(mine.join("references/r.md")).unwrap(), b"ref");
        assert!(mine.join("mine.md").exists());
        assert!(store.join("omm-x").is_symlink());
        assert!(
            !fx.bases.muse_config.join("themes").exists(),
            "the rest of the plan ran"
        );
        assert!(r.ledger_removed);
    }

    #[test]
    fn a_structural_settings_key_is_kept_for_the_users_members_and_restored_last_otherwise() {
        // Gate 1 decision D (round 4, K): `permissions.schema_version`
        // restored to absent as an independent leaf broke the user's own
        // named profiles.
        let fx = fixture();
        let mut ledger = fx.ledger.clone();
        ledger.registrations.clear();
        let settings = fx.bases.muse_config.join("settings.json");
        fs::write(
            &settings,
            br#"{"schema_version":1,"permissions":{"schema_version":1,"default_profile":"omm-strict","profiles":{"omm-strict":{"extends":":ask-me"},"mine":{"extends":":ask-me"}}}}"#,
        )
        .unwrap();
        for (key, value) in [
            ("permissions.schema_version", json!(1)),
            ("permissions.default_profile", json!("omm-strict")),
            ("permissions.profiles.omm-strict.extends", json!(":ask-me")),
        ] {
            ledger.record_settings_key(key, None, Some(value), Some("strict"));
        }
        let opts = Options {
            settings_file: Some(settings.clone()),
            ..Options::default()
        };
        let p = plan(&ledger, &fx.bases, opts.clone()).unwrap();
        assert!(
            p.dropped.iter().any(|d| d.reason.starts_with("kept")
                && d.reason.contains("permissions.schema_version")
                && d.reason.contains("permissions.profiles")),
            "{:?}",
            p.dropped
        );
        assert!(!p.host_steps.iter().any(|s| matches!(
            s,
            UndoStep::RestoreSettingsKey { path, .. } if path == "permissions.schema_version"
        )));
        assert!(p.render().contains("kept:"));
        let mut undoer = RecordingUndoer::default();
        let r = apply(
            &p,
            ledger.clone(),
            &fx.bases,
            &mut undoer,
            &ApplyOptions {
                omm_version: "0".into(),
                dry_run: true,
            },
        )
        .unwrap();
        assert_eq!((r.registrations_dropped, r.registrations_kept), (1, 1));
        // Without the user's member: restored, but LAST among the settings
        // keys, so it goes only with the last other member.
        fs::write(
            &settings,
            br#"{"schema_version":1,"permissions":{"schema_version":1,"default_profile":"omm-strict","profiles":{"omm-strict":{"extends":":ask-me"}}}}"#,
        )
        .unwrap();
        let p = plan(&ledger, &fx.bases, opts).unwrap();
        assert!(p.dropped.is_empty(), "{:?}", p.dropped);
        let keys: Vec<&str> = p
            .host_steps
            .iter()
            .filter_map(|s| match s {
                UndoStep::RestoreSettingsKey { path, .. } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(keys.len(), 3, "{keys:?}");
        assert_eq!(keys.last().copied(), Some("permissions.schema_version"));
        // The simulation behind the prediction prunes the way the undoer does.
        let mut doc = json!({"permissions": {"schema_version": 1, "profiles": {"omm-strict": {"extends": 1}}}});
        remove_leaf_and_prune(
            &mut doc,
            &segments("permissions.profiles.omm-strict.extends"),
        );
        assert_eq!(doc, json!({"permissions": {"schema_version": 1}}));
        remove_leaf_and_prune(&mut doc, &segments("permissions.schema_version"));
        assert_eq!(doc, json!({}));
    }
}
