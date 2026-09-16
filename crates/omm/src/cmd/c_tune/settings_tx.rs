//! The one `settings.json` transaction behind every tuning write (R9, R10):
//! typed patch → validate (`muse config validate --plane defaults` on each
//! touched key inside the enterprise wrapper, then the host's own loader —
//! `SettingsDoc::validate`) → atomic commit on the realpath under the
//! muse-config lock with a verified backup → ledger (one `settings.json`
//! entry + one `settings-key` registration per key, carrying the FIRST prior
//! omm saw) → one audit line.
//!
//! Host facts this leans on: the host rewrites `settings.json` from a typed
//! struct and destroys unknown keys (host-reality.md "Paths": settings
//! rewrite), so only the 29 typed keys are ever patched; `tui.theme` is
//! `field_not_activated` on the defaults plane and `permissions` /
//! `plugins` / `runtime_capabilities` are `unknown_member` there, so the
//! loader probe is the deciding oracle for those (host-reality.md "Identity
//! constraints": `config validate`); and `full_skill_description_ids`
//! REPLACES the host default `["bundled:git"]` (host-reality.md "Budgets";
//! R18 trap), which [`guard_full_ids`] refuses to lose.

use std::path::PathBuf;

use serde_json::{Map, Value};

use omm_host::host_reality as hr;
use omm_host::settings::{CommitOptions, PatchOp, SettingsDoc, MUSE_CONFIG_BASE};
use omm_host::{fsx, HostError};
use omm_ledger::audit::{self, Audit, Event};
use omm_ledger::shared::{self, Original};
use omm_ledger::{Base, Class, Entry, Kind, Mechanism};

use super::{modify_ledger, rel};
use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Action, Converge};

/// The ledger path of the shared file every transaction touches.
pub const SETTINGS_REL: &str = "settings.json";
/// The converge category of settings keys.
pub const CATEGORY: &str = "settings";

/// One transaction: who writes, what changes, and the audit note.
#[derive(Clone, Debug, Default)]
pub struct Tx {
    /// The ledger `writer` / audit note prefix, e.g. `omm theme`.
    pub writer: &'static str,
    pub ops: Vec<PatchOp>,
    /// Dotted keys among `ops` that put a ledgered prior back (a profile
    /// switch restoring the outgoing profile's keys): their registration is
    /// dropped after the commit instead of recorded, and a removal prunes
    /// the empty parent objects it leaves (an empty object and an absent
    /// key are the same to the host's typed loader; the document then
    /// equals the one before the key was set).
    pub restores: Vec<String>,
    /// The profile whose slice this transaction applies (`omm profile use`):
    /// every key it sets is tagged with it in the ledger, so the next switch
    /// knows which keys to put back; `None` for every other writer.
    pub profile: Option<String>,
}

/// One key the transaction changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Changed {
    pub key: String,
    pub prior: Option<Value>,
    pub value: Option<Value>,
}

/// What [`apply`] did.
#[derive(Clone, Debug, Default)]
pub struct TxOutcome {
    pub changed: Vec<Changed>,
    pub unchanged: Vec<String>,
    /// Restores dropped from the transaction because the leaf is a required
    /// structural member of an object that still holds the user's members
    /// (Gate 1 decision D): `(dotted key, why)`; the key keeps its
    /// registration so a later switch or the uninstall tries again.
    pub kept_structural: Vec<(String, String)>,
    /// The settings file (realpath once it exists).
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    /// False under `--dry-run` and when nothing changed.
    pub written: bool,
    pub sha256: Option<String>,
}

/// Run the transaction. Refuses up front on the D3 collision (every
/// settings-writing host command exits 1 in that state — report it, do not
/// add to it: ARCHITECTURE.md §3). Under `--dry-run` the candidate is still
/// validated by the host and nothing is written or ledgered.
pub fn apply(ctx: &Ctx, tx: Tx, converge: &mut Converge) -> Result<TxOutcome> {
    let mut doc = SettingsDoc::for_roots(&ctx.roots)?;
    refuse_collision(&doc)?;
    // The file as found: the audit line's `sha256_before`, and the pre-omm
    // record its entry keeps the first time omm writes it (`omm_ledger::shared`).
    let found = if doc.existed() {
        Original::read(doc.path())?
    } else {
        None
    };
    let before_sha = found.as_ref().map(|o| o.sha256.clone());
    // A restore that would strip a required structural member from an
    // object the user's own members still need is dropped here and named
    // (Gate 1 decision D).
    let (ops, kept_structural) = keep_structural_members(doc.value(), tx.ops, &tx.restores)?;
    let tx = Tx { ops, ..tx };
    let priors = doc.patch_typed(&tx.ops)?;
    for op in tx.ops.iter().filter(|op| op.value.is_none()) {
        if tx.restores.contains(&op.path.join(".")) {
            prune_empty_parents(&mut doc, &op.path)?;
        }
    }
    guard_full_ids(&tx.ops, doc.value())?;

    let mut outcome = TxOutcome {
        path: doc.path().to_path_buf(),
        kept_structural,
        ..TxOutcome::default()
    };
    for (op, prior) in tx.ops.iter().zip(priors) {
        let key = prior.key();
        if prior.prior == op.value {
            outcome.unchanged.push(key);
        } else {
            outcome.changed.push(Changed {
                key,
                prior: prior.prior,
                value: op.value.clone(),
            });
        }
    }
    converge.record_n(CATEGORY, Action::Unchanged, outcome.unchanged.len());
    if outcome.changed.is_empty() {
        return Ok(outcome);
    }

    let inv = ctx.invoker()?;
    let report = doc.commit(
        inv,
        &CommitOptions::for_roots(&ctx.roots).dry_run(ctx.dry_run),
    )?;
    for w in &report.validation.warnings {
        if w.contains("destroyed") || w.contains("drops it") {
            ctx.out.warn(w);
        } else {
            ctx.out.note(w);
        }
    }
    outcome.path = report.path.clone();
    outcome.backup = report.backup.clone();
    outcome.written = report.written;
    outcome.sha256 = Some(report.sha256.clone());
    converge.record_n(CATEGORY, Action::Updated, outcome.changed.len());
    if report.backup.is_some() {
        converge.record(CATEGORY, Action::BackedUp);
    }
    if ctx.dry_run {
        return Ok(outcome);
    }

    ledger_settings_write(
        ctx,
        tx.writer,
        !doc.existed(),
        &report.sha256,
        &outcome.changed,
        &tx.restores,
        tx.profile.as_deref(),
        found.as_ref(),
    )?;
    audit_settings_write(
        ctx,
        tx.writer,
        before_sha.as_deref(),
        &report.sha256,
        &outcome.changed,
    )?;
    Ok(outcome)
}

/// The D3 refusal, with the fix doctor prints.
pub fn refuse_collision(doc: &SettingsDoc) -> Result<()> {
    if doc.has_mcp_collision() {
        return Err(OmmError::Usage(format!(
            "{}: both `mcpServers` and `mcp_servers` are present — every settings-writing command exits 1 in that state (`{} …`); run `omm settings fix-mcp-collision` first (doctor D3)",
            doc.path().display(),
            hr::MCP_COLLISION_MESSAGE
        )));
    }
    Ok(())
}

/// R18: when a transaction touches `run.context_slimming.full_skill_description_ids`
/// (or a parent of it) the staged array must re-list every host default
/// (`hr::CONTEXT_SLIMMING_DEFAULT_FULL_IDS`), because a user value REPLACES
/// the default and `bundled:git` silently loses its 740-B description
/// (docs/experiments/context-slimming.md §0). A document that already lacks
/// them is left alone by unrelated transactions.
pub fn guard_full_ids(ops: &[PatchOp], staged: &Value) -> Result<()> {
    let target = ["run", "context_slimming", "full_skill_description_ids"];
    let touches = ops
        .iter()
        .any(|op| op.path.len() <= target.len() && op.path.iter().zip(target).all(|(a, b)| a == b));
    if !touches {
        return Ok(());
    }
    let Some(Value::Array(items)) = staged
        .get("run")
        .and_then(|r| r.get("context_slimming"))
        .and_then(|c| c.get("full_skill_description_ids"))
    else {
        return Ok(());
    };
    let missing: Vec<&str> = hr::CONTEXT_SLIMMING_DEFAULT_FULL_IDS
        .iter()
        .copied()
        .filter(|id| !items.iter().any(|i| i.as_str() == Some(id)))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(OmmError::Usage(format!(
        "run.context_slimming.full_skill_description_ids would drop {}: a user value REPLACES the host default {:?}, so the bundled entry loses its full description (R18 trap, docs/experiments/context-slimming.md §0); list it again",
        missing.join(", "),
        hr::CONTEXT_SLIMMING_DEFAULT_FULL_IDS
    )))
}

/// `(key, why)` of the restores [`keep_structural_members`] dropped.
pub type KeptStructural = Vec<(String, String)>;

/// Gate 1 decision D: among the restores of a transaction (removals of
/// keys back to an absent prior), a leaf that is a required structural
/// member of its object (settings-keys.json `structural`) is dropped from
/// the transaction when the object would still hold other members once
/// every other restore has landed — the host refuses the object without
/// it. The others are applied first in the simulation, so a structural leaf
/// goes only with the last other member. Returns the ops kept, structural
/// removals last, and `(key, why)` for each dropped one.
pub fn keep_structural_members(
    doc: &Value,
    ops: Vec<PatchOp>,
    restores: &[String],
) -> Result<(Vec<PatchOp>, KeptStructural)> {
    let keys = hr::settings_keys()?;
    let is_structural = |op: &PatchOp| {
        op.value.is_none()
            && restores.contains(&op.path.join("."))
            && matches!(op.path.as_slice(), [top, leaf] if keys.structural_required(top).iter().any(|r| r == leaf))
    };
    if !ops.iter().any(&is_structural) {
        return Ok((ops, Vec::new()));
    }
    let mut sim = doc.clone();
    for op in &ops {
        if is_structural(op) {
            continue;
        }
        if op.value.is_none() {
            remove_leaf_and_prune(&mut sim, &op.path);
        } else if let Some(v) = &op.value {
            set_leaf(&mut sim, &op.path, v.clone());
        }
    }
    let (structural, mut kept_ops): (Vec<PatchOp>, Vec<PatchOp>) =
        ops.into_iter().partition(is_structural);
    let mut kept = Vec::new();
    for op in structural {
        match omm_host::settings::structural_keep_reason(&sim, &op.path)? {
            Some(why) => kept.push((op.path.join("."), why)),
            None => kept_ops.push(op),
        }
    }
    Ok((kept_ops, kept))
}

fn set_leaf(doc: &mut Value, path: &[String], value: Value) {
    let Some((leaf, parents)) = path.split_last() else {
        return;
    };
    let mut cur = doc;
    for p in parents {
        let Some(obj) = cur.as_object_mut() else {
            return;
        };
        cur = obj
            .entry(p.clone())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    if let Some(obj) = cur.as_object_mut() {
        obj.insert(leaf.clone(), value);
    }
}

/// The value at `path` under `root`, mutable.
fn value_at_mut<'a>(root: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let mut cur = root;
    for p in path {
        cur = cur.get_mut(p)?;
    }
    Some(cur)
}

/// Remove the leaf at `path` from a plain document and the empty objects
/// that leaves above it, the top level included (what [`prune_empty_parents`]
/// does on a `SettingsDoc`).
fn remove_leaf_and_prune(doc: &mut Value, path: &[String]) {
    let mut depth = path.len();
    while depth >= 1 {
        let (parents, leaf) = (&path[..depth - 1], &path[depth - 1]);
        let Some(obj) = value_at_mut(doc, parents).and_then(Value::as_object_mut) else {
            return;
        };
        let remove = depth == path.len()
            || obj
                .get(leaf)
                .and_then(Value::as_object)
                .map(|o| o.is_empty())
                .unwrap_or(false);
        if !remove {
            return;
        }
        obj.remove(leaf);
        depth -= 1;
    }
}

/// Remove the empty objects a leaf removal left above `path` (`tui: {}`
/// after `tui.theme`), stopping at the first non-empty one.
pub fn prune_empty_parents(doc: &mut SettingsDoc, path: &[String]) -> Result<()> {
    let mut segments: Vec<String> = path.to_vec();
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
        doc.patch_typed(&[PatchOp::remove_path(segments.clone())])?;
    }
    Ok(())
}

/// Ledger: the `settings.json` entry (class `seeded` when omm created the
/// file, kept once set — R5 unlinks it after the restores; a user's file
/// keeps its pre-omm bytes and mode, `original`, from the first writer on)
/// and one `settings-key` registration per changed key — first prior wins,
/// `value` is what was written last; a key in `restores` loses its
/// registration instead (it is back at its prior).
#[allow(clippy::too_many_arguments)]
pub fn ledger_settings_write(
    ctx: &Ctx,
    writer: &str,
    created: bool,
    sha256: &str,
    changed: &[Changed],
    restores: &[String],
    profile: Option<&str>,
    original: Option<&Original>,
) -> Result<()> {
    let path = rel(SETTINGS_REL)?;
    modify_ledger(ctx, |ledger| {
        let class = match ledger.find(Base::MuseConfig, &path) {
            Some(e) if e.class == Class::Seeded => Class::Seeded,
            _ if created => Class::Seeded,
            _ => Class::SharedKey,
        };
        ledger.upsert_keep_prior(Entry {
            base: Base::MuseConfig,
            path: path.clone(),
            kind: Kind::SettingsKey,
            sha256: sha256.to_string(),
            source_version: OMM_VERSION.to_string(),
            writer: writer.to_string(),
            mechanism: Mechanism::SettingsPatch,
            class,
            prior: None,
        });
        if let (Class::SharedKey, Some(o)) = (class, original) {
            shared::attach_original(ledger, Base::MuseConfig, &path, o);
        }
        for c in changed {
            if restores.contains(&c.key) {
                ledger.forget_settings_key(&c.key);
            } else {
                ledger.record_settings_key(&c.key, c.prior.clone(), c.value.clone(), profile);
            }
        }
        Ok(())
    })
}

/// One audit line for the write (`action: update`, the file, both hashes).
pub fn audit_settings_write(
    ctx: &Ctx,
    writer: &str,
    before: Option<&str>,
    after: &str,
    changed: &[Changed],
) -> Result<()> {
    let keys: Vec<&str> = changed.iter().map(|c| c.key.as_str()).collect();
    Audit::new(&ctx.omm_root(), OMM_VERSION).append(
        &Event::new(audit::ACTION_UPDATE)
            .at(Base::MuseConfig, &rel(SETTINGS_REL)?)
            .before(before)
            .after(Some(after))
            .note(format!("{writer}: {}", keys.join(", "))),
    )?;
    Ok(())
}

/// A whole-document replacement for the one edit `SettingsDoc::patch_typed`
/// cannot express — removing the legacy `mcp_servers` spelling, which the
/// typed patch refuses by design (`omm settings fix-mcp-collision`). Same
/// guarantees as `SettingsDoc::commit`: the host's loader probe and the
/// defaults-plane validator on the projected `mcp_servers` member first,
/// then — under `$OMM/locks/muse-config.lock` — a stale check against the
/// bytes loaded, a verified backup under `$OMM/snapshots/<ts>/muse-config/`,
/// and an atomic rename on the realpath. Nothing is written under `--dry-run`.
pub fn replace_document(
    ctx: &Ctx,
    doc: &SettingsDoc,
    loaded_sha256: Option<&str>,
    candidate: &Map<String, Value>,
) -> Result<RawReplace> {
    let inv = ctx.invoker()?;
    let mut bytes = serde_json::to_vec_pretty(&Value::Object(candidate.clone()))
        .map_err(|e| OmmError::Usage(format!("settings candidate: {e}")))?;
    bytes.push(b'\n');

    // 1. The defaults-plane validator on the projected mcp_servers member
    //    (it takes the snake_case spelling — settings-keys.json
    //    `enterprise_defaults_plane`).
    let plane = hr::enterprise_defaults_plane()?;
    if let Some(servers) = candidate.get("mcpServers") {
        let projected_key = plane
            .renames
            .get("mcpServers")
            .cloned()
            .unwrap_or_else(|| "mcpServers".to_string());
        let tmp = tempfile::Builder::new()
            .prefix("omm-settings-mcp-")
            .tempdir()
            .map_err(|e| OmmError::io("create temp dir", e))?;
        let file = tmp.path().join("mcp.json");
        let mut settings = Map::new();
        settings.insert(projected_key, servers.clone());
        let mut envelope = Map::new();
        envelope.insert(
            "schema_version".to_string(),
            Value::from(hr::SETTINGS_SCHEMA_VERSION),
        );
        envelope.insert("settings".to_string(), Value::Object(settings));
        let projected = serde_json::to_vec(&Value::Object(envelope))
            .map_err(|e| OmmError::Usage(format!("projected settings: {e}")))?;
        std::fs::write(&file, projected)
            .map_err(|e| OmmError::io(format!("write {}", file.display()), e))?;
        let verdict = omm_host::probe::config_validate(inv, &plane.plane, &file)?;
        let blind = verdict
            .reason()
            .map(|r| plane.blind_outcomes.iter().any(|b| b == r))
            .unwrap_or(false);
        if !verdict.is_accepted() && !blind {
            return Err(HostError::SettingsRejected {
                stage: "enterprise-validator",
                detail: format!("`mcpServers`: {}", verdict.summary()),
            }
            .into());
        }
        if blind {
            ctx.out.note(format!(
                "`mcpServers`: the enterprise validator answered {}; relying on the host loader probe",
                verdict.summary()
            ));
        }
    }
    // 2. The host's own loader.
    let load = omm_host::probe::settings_load_probe(inv, &bytes)?;
    if !load.accepted {
        return Err(HostError::SettingsRejected {
            stage: "host-loader",
            detail: load.detail,
        }
        .into());
    }

    let sha256 = fsx::sha256_bytes(&bytes);
    if ctx.dry_run {
        return Ok(RawReplace {
            path: doc.path().to_path_buf(),
            backup: None,
            written: false,
            sha256,
        });
    }

    // 3. Under the shared lock: stale check, backup, atomic rename.
    let omm_root = ctx.omm_root();
    let _lock = fsx::lock_exclusive(
        &omm_host::settings::muse_config_lock_path(&omm_root),
        fsx::LOCK_WAIT_DEFAULT,
    )?;
    let path = doc.path();
    if let Some(parent) = path.parent() {
        fsx::create_dir_all(parent)?;
    }
    let realpath = fsx::realpath_for_write(path)?;
    let mut backup = None;
    if realpath.exists() {
        // Under the lock, immediately before the rename: the bytes must be
        // the ones the caller loaded (`loaded_sha256`), else somebody else
        // landed in between and this candidate is stale.
        let found = fsx::sha256_file(&realpath)?;
        match loaded_sha256 {
            Some(expected) if expected == found => {}
            other => {
                return Err(HostError::Stale {
                    path: realpath,
                    expected: other.unwrap_or("<absent at load>").to_string(),
                    found,
                }
                .into())
            }
        }
        backup = Some(fsx::snapshot_backup(
            &realpath,
            &ctx.roots.snapshots_dir(),
            MUSE_CONFIG_BASE,
        )?);
    }
    fsx::write_atomic(&realpath, &bytes)?;
    Ok(RawReplace {
        path: realpath,
        backup,
        written: true,
        sha256,
    })
}

/// What [`replace_document`] did.
#[derive(Clone, Debug)]
pub struct RawReplace {
    pub path: PathBuf,
    pub backup: Option<PathBuf>,
    pub written: bool,
    pub sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn set(path: &str, v: Value) -> PatchOp {
        PatchOp::set(path, v)
    }

    #[test]
    fn full_ids_guard_fires_only_when_the_key_is_touched() {
        let missing = json!({"run": {"context_slimming": {"full_skill_description_ids": []}}});
        // Unrelated transaction: the document may already lack the default.
        assert!(guard_full_ids(&[set("tui.theme", json!("x"))], &missing).is_ok());
        // Touching the ids, a parent, or `run` wholesale: refused.
        for op in [
            set("run.context_slimming.full_skill_description_ids", json!([])),
            set(
                "run.context_slimming",
                json!({"full_skill_description_ids": []}),
            ),
            set(
                "run",
                json!({"context_slimming": {"full_skill_description_ids": []}}),
            ),
        ] {
            let err = guard_full_ids(std::slice::from_ref(&op), &missing).unwrap_err();
            assert!(err.to_string().contains("bundled:git"), "{err}");
            assert_eq!(err.exit_code(), 2);
        }
        // Re-listed: fine; absent: fine (the host default applies).
        let listed = json!({"run": {"context_slimming": {"full_skill_description_ids": ["bundled:git", "bundled:taste"]}}});
        assert!(guard_full_ids(
            &[set(
                "run.context_slimming.full_skill_description_ids",
                json!(["bundled:git"])
            )],
            &listed
        )
        .is_ok());
        let absent =
            json!({"run": {"context_slimming": {"skill_catalog_descriptions": "first_sentence"}}});
        assert!(guard_full_ids(
            &[set(
                "run.context_slimming.skill_catalog_descriptions",
                json!("first_sentence")
            )],
            &absent
        )
        .is_ok());
        // A removal of the key is fine too (the default comes back).
        assert!(guard_full_ids(
            &[PatchOp::remove(
                "run.context_slimming.full_skill_description_ids"
            )],
            &absent
        )
        .is_ok());
    }

    #[test]
    fn a_structural_restore_is_kept_while_the_user_owns_siblings_and_ordered_last() {
        // Gate 1 decision D (round 4, K): default → strict → user adds
        // permissions.profiles.mine → default.
        let doc = json!({"schema_version": 1, "permissions": {"schema_version": 1, "default_profile": "omm-strict",
            "profiles": {"omm-strict": {"extends": ":ask-me", "approval": "prompt_unmatched", "reviewer": "human"},
                         "mine": {"extends": ":ask-me"}}}});
        let restores: Vec<String> = [
            "permissions.schema_version",
            "permissions.default_profile",
            "permissions.profiles.omm-strict.extends",
            "permissions.profiles.omm-strict.approval",
            "permissions.profiles.omm-strict.reviewer",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let ops: Vec<PatchOp> = std::iter::once(PatchOp::set(
            "run.context_slimming.skill_catalog_descriptions",
            json!("first_sentence"),
        ))
        .chain(restores.iter().map(|k| PatchOp::remove(k)))
        .collect();
        let (kept_ops, kept) = keep_structural_members(&doc, ops.clone(), &restores).unwrap();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert_eq!(kept[0].0, "permissions.schema_version");
        assert!(kept[0].1.contains("permissions.profiles"), "{}", kept[0].1);
        assert_eq!(kept_ops.len(), ops.len() - 1);
        assert!(!kept_ops
            .iter()
            .any(|op| op.path.join(".") == "permissions.schema_version"));
        // Without the user's member: the leaf goes, ordered last.
        let doc = json!({"schema_version": 1, "permissions": {"schema_version": 1, "default_profile": "omm-strict",
            "profiles": {"omm-strict": {"extends": ":ask-me", "approval": "prompt_unmatched", "reviewer": "human"}}}});
        let (kept_ops, kept) = keep_structural_members(&doc, ops.clone(), &restores).unwrap();
        assert!(kept.is_empty(), "{kept:?}");
        assert_eq!(kept_ops.len(), ops.len());
        assert_eq!(
            kept_ops.last().map(|op| op.path.join(".")).as_deref(),
            Some("permissions.schema_version")
        );
        // Not a restore (a plain removal by `settings set`): untouched.
        let (same, kept) =
            keep_structural_members(&doc, ops.clone(), &["tui.theme".to_string()]).unwrap();
        assert_eq!(same, ops);
        assert!(kept.is_empty());
    }

    #[test]
    fn collision_is_refused_before_any_patch() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("settings.json");
        std::fs::write(
            &file,
            b"{\"schema_version\":1,\"mcpServers\":{},\"mcp_servers\":{}}",
        )
        .unwrap();
        let doc = SettingsDoc::load(&file).unwrap();
        let err = refuse_collision(&doc).unwrap_err();
        assert!(err.to_string().contains("fix-mcp-collision"));
        std::fs::write(&file, b"{\"schema_version\":1,\"mcpServers\":{}}").unwrap();
        assert!(refuse_collision(&SettingsDoc::load(&file).unwrap()).is_ok());
    }
}
