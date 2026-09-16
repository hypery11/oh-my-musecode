//! `omm profile {use,list,show}` and `omm keymap <preset>` (ARCHITECTURE.md
//! §7; PLAN.md 2.4 grows `profile use` into the full transaction — settings
//! slice + permission profile + activation map + rules variant + theme +
//! gate env; Phase 1 lands the settings slice as ONE ledgered transaction).
//!
//! A profile is `content/profiles/<name>.json` (or the overlay's
//! `$OMM/custom/profiles/<name>.json`, R7): an object of typed
//! `settings.json` keys (settings-keys.json; the manifest lint enforces the
//! key set). It is flattened to leaf paths so an unrelated user value under
//! the same top-level key (`run.workflow_trigger_mode` beside
//! `run.context_slimming`) survives, every leaf gets its own prior in the
//! ledger, and `settings_tx::guard_full_ids` enforces the R18 trap. R22 (a
//! profile can only tighten) is Phase 2's lattice; nothing here weakens a
//! permission on its own initiative, it applies what the slice says.
//!
//! A keymap preset is `tui.keymap : { <context> : { <action> : [key-spec] } }`
//! (research/musecode/tui-slash-theme.md §2.3, PROVEN): `default` removes
//! the key (host defaults return), any other name is a JSON file from
//! `$OMM/custom/keymaps/`, `content/keymaps/` or a literal path. The shape is
//! checked here; the semantics (unknown context/action, duplicate binding,
//! bad key spec) are the defaults-plane validator's, which `SettingsDoc::
//! validate` runs on the projected `tui` member inside the enterprise
//! wrapper (context-slimming.md §0 last row).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use omm_host::host_reality as hr;
use omm_host::settings::{PatchOp, SettingsDoc};
use omm_manifest::catalog::AssetKind;
use omm_manifest::overlay;

use super::settings_tx::{self, Tx};
use super::{check_name, read_ledger, read_regular_file, ContentSource, OmmConfig, WriteReport};
use crate::cmd::Ctx;
use crate::error::{OmmError, Result};
use crate::output::{Action, Table};
use omm_ledger::Registration;

/// The converge category of `$OMM/config.json`'s `profile` key.
pub const CATEGORY_PROFILE: &str = "profile";
/// `omm keymap default` — restore the host's own bindings.
pub const KEYMAP_DEFAULT: &str = "default";
/// Where keymap presets live under `$OMM/custom/` and `content/`.
pub const KEYMAPS_DIR: &str = "keymaps";

/// A resolved profile file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileFile {
    /// The name the user asked for (file stem).
    pub name: String,
    /// The catalog id when bundled (`omm-default`).
    pub id: Option<String>,
    pub path: PathBuf,
    pub provider: super::theme::Provider,
}

/// The file stem of `profiles/<stem>.json`.
fn json_stem(path: &str) -> Option<&str> {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".json"))
}

/// Resolve a profile: the overlay first, then the catalog by stem or id.
pub fn resolve(ctx: &Ctx, name: &str) -> Result<ProfileFile> {
    check_name("profile", name)?;
    let custom = overlay::custom_path(&ctx.omm_root(), AssetKind::Profile, name);
    if custom.exists() {
        return Ok(ProfileFile {
            name: name.to_string(),
            id: None,
            path: custom,
            provider: super::theme::Provider::Custom,
        });
    }
    let source = ContentSource::require(&ctx.omm_root())?;
    let catalog = source.catalog()?;
    let asset = catalog
        .shipped_of(AssetKind::Profile)
        .find(|a| json_stem(&a.path) == Some(name))
        .or(match catalog.resolve(name)? {
            Some(a) if a.kind == AssetKind::Profile => Some(a),
            _ => None,
        });
    let Some(asset) = asset else {
        let known: Vec<&str> = catalog
            .shipped_of(AssetKind::Profile)
            .filter_map(|a| json_stem(&a.path))
            .collect();
        return Err(OmmError::Usage(format!(
            "unknown profile `{name}`; bundled: {}; a custom profile goes to {}",
            known.join(" "),
            custom.display()
        )));
    };
    Ok(ProfileFile {
        name: json_stem(&asset.path).unwrap_or(&asset.id).to_string(),
        id: Some(asset.id.clone()),
        path: source.asset_path(&asset.path)?,
        provider: super::theme::Provider::Bundled,
    })
}

/// Read a profile slice: a JSON object whose top-level keys are typed
/// settings keys (never `schema_version`, never a legacy spelling).
pub fn load_slice(path: &Path) -> Result<Map<String, Value>> {
    let bytes = read_regular_file(path)?;
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|e| OmmError::Usage(format!("{}: not JSON: {e}", path.display())))?;
    let Value::Object(map) = value else {
        return Err(OmmError::Usage(format!(
            "{}: a profile is a JSON object of settings keys",
            path.display()
        )));
    };
    let keys = hr::settings_keys()?;
    for k in map.keys() {
        if k == "schema_version" || keys.canonical_spelling(k).is_some() || !keys.is_known(k) {
            return Err(OmmError::Usage(format!(
                "{}: `{k}` is not one of the {} typed settings keys omm may write (R9)",
                path.display(),
                keys.counts.top_level_keys
            )));
        }
    }
    Ok(map)
}

/// Flatten a slice to leaf patches: objects recurse, everything else
/// (scalars, arrays, empty objects) is a leaf set at its full path.
pub fn flatten(slice: &Map<String, Value>) -> Vec<PatchOp> {
    let mut out = Vec::new();
    for (k, v) in slice {
        flatten_into(vec![k.clone()], v, &mut out);
    }
    out
}

fn flatten_into(prefix: Vec<String>, v: &Value, out: &mut Vec<PatchOp>) {
    match v {
        Value::Object(m) if !m.is_empty() => {
            for (k, child) in m {
                let mut p = prefix.clone();
                p.push(k.clone());
                flatten_into(p, child, out);
            }
        }
        leaf => out.push(PatchOp::set_path(prefix, leaf.clone())),
    }
}

/// One key of the outgoing profile put back where the incoming slice does
/// not set it (see [`outgoing_restores`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restore {
    pub key: String,
    /// The ledgered prior the key goes back to (`None` = removed).
    pub prior: Option<Value>,
    /// The profile the ledger tags the key with, when it does.
    pub set_by: Option<String>,
}

/// The keys a switch to `incoming` must consider putting back, from the
/// ledger alone: every settings key tagged with a profile other than
/// `incoming` (Gate 1 decision: registrations carry the profile name, so
/// the restore never depends on `config.json` naming the outgoing profile
/// or on that profile's slice still being loadable).
pub fn tagged_outgoing(ledger: &omm_ledger::Ledger, incoming: &str) -> Vec<PatchOp> {
    ledger
        .settings_keys_by_profile()
        .into_iter()
        .filter(|(_, tag)| tag != incoming)
        .map(|(key, _)| PatchOp::remove(&key))
        .collect()
}

/// What a switch away from `outgoing` must put back: every leaf of the
/// outgoing slice the incoming slice leaves alone, at its ledgered prior —
/// but only while the key still holds the value omm wrote (a hand edit
/// since is left as is and reported). A leaf with no registration was never
/// changed by omm and is skipped. Gate 1 `prof`: default → strict →
/// default left strict's `permissions.*` active under a config.json that
/// said default.
pub fn outgoing_restores(
    outgoing: &[PatchOp],
    incoming: &[PatchOp],
    ledger: &omm_ledger::Ledger,
    doc: &SettingsDoc,
) -> (Vec<Restore>, Vec<String>) {
    let mut restores = Vec::new();
    let mut edited = Vec::new();
    for op in outgoing {
        if incoming.iter().any(|i| i.path == op.path) {
            continue;
        }
        let key = op.path.join(".");
        if restores.iter().any(|r: &Restore| r.key == key) || edited.contains(&key) {
            continue;
        }
        let Some(Registration::SettingsKey {
            prior,
            value,
            profile,
            ..
        }) = ledger.settings_key(&key)
        else {
            continue;
        };
        let current = doc.get(&op.path).cloned();
        if value.is_some() && current != *value {
            edited.push(key);
            continue;
        }
        if current == *prior {
            continue;
        }
        restores.push(Restore {
            key,
            prior: prior.clone(),
            set_by: profile.clone(),
        });
    }
    (restores, edited)
}

/// The keys of an incoming slice the switch must leave alone: a key whose
/// `settings-key` registration recorded what omm wrote (`value`) while the
/// document now holds something else — the user edited it since, and R3
/// preserves a user edit and reports it, on the way in as on the way out
/// (Gate 1: `omm profile use strict` overwrote a hand edit of a key both
/// profiles set, with `left_edited: []`). `force` applies the slice anyway.
/// Returns the retained ops and the keys left, in slice order.
pub fn incoming_edits(
    incoming: Vec<PatchOp>,
    ledger: Option<&omm_ledger::Ledger>,
    doc: &SettingsDoc,
    force: bool,
) -> (Vec<PatchOp>, Vec<String>) {
    let mut kept = Vec::with_capacity(incoming.len());
    let mut edited = Vec::new();
    for op in incoming {
        let key = op.path.join(".");
        let recorded = ledger.and_then(|l| match l.settings_key(&key) {
            Some(Registration::SettingsKey { value: Some(v), .. }) => Some(v.clone()),
            _ => None,
        });
        match recorded {
            Some(v) if !force && doc.get(&op.path) != Some(&v) => edited.push(key),
            _ => kept.push(op),
        }
    }
    (kept, edited)
}

/// `omm profile use <name> [--force]`.
pub fn use_profile(ctx: &Ctx, name: &str, force: bool) -> Result<WriteReport> {
    let profile = resolve(ctx, name)?;
    let slice = load_slice(&profile.path)?;
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("profile", json!(profile.name));
    report.field("provider", json!(profile.provider.as_str()));
    report.field("source", json!(profile.path.display().to_string()));
    let incoming = flatten(&slice);
    report.line(format!(
        "profile {} ({}, {}): {} key{}",
        profile.name,
        profile.provider.as_str(),
        profile.path.display(),
        incoming.len(),
        if incoming.len() == 1 { "" } else { "s" }
    ));

    // A key of the slice the user edited since a profile set it is left as
    // is and named (R3), unless --force; the outgoing restores below apply
    // the same rule to the keys the slice leaves alone.
    let omm_root = ctx.omm_root();
    let mut config = OmmConfig::load(&omm_root)?;
    let mut restores: Vec<String> = Vec::new();
    let ledger = read_ledger(ctx)?;
    let doc = SettingsDoc::for_roots(&ctx.roots)?;
    let (mut ops, mut edited) = incoming_edits(incoming, ledger.as_ref(), &doc, force);
    for k in &edited {
        report.line(format!(
            "  {k} left as is: edited since a profile set it (--force applies {} over it)",
            profile.name
        ));
    }

    // The outgoing profile's keys the incoming slice does not set go back
    // to their ledgered prior in this same transaction, and lose their
    // registration. Outgoing = every ledger key tagged with another profile
    // (the authority) ∪ the slice of the profile config.json names (for a
    // ledger written before the tags existed).
    let previous = config
        .profile()
        .filter(|p| *p != profile.name)
        .map(str::to_string);
    let mut outgoing: Vec<PatchOp> = ledger
        .as_ref()
        .map(|l| tagged_outgoing(l, &profile.name))
        .unwrap_or_default();
    if let Some(previous) = &previous {
        match resolve(ctx, previous).and_then(|f| load_slice(&f.path)) {
            Ok(out_slice) => {
                for op in flatten(&out_slice) {
                    if !outgoing.iter().any(|o| o.path == op.path) {
                        outgoing.push(op);
                    }
                }
            }
            Err(e) => {
                ctx.out.warn(format!(
                    "previous profile `{previous}` cannot be loaded ({e}); only the keys the ledger tags with a profile are restored"
                ));
            }
        }
    }
    let mut back: Vec<Restore> = Vec::new();
    if let Some(l) = &ledger {
        let (restored, out_edited) = outgoing_restores(&outgoing, &ops, l, &doc);
        back = restored;
        for k in out_edited {
            if !edited.contains(&k) {
                report.line(format!("  {k} left as is: edited since a profile set it"));
                edited.push(k);
            }
        }
    }
    {
        for r in &back {
            report.line(format!(
                "  {} restored to {} (set by profile {}, not by {})",
                r.key,
                super::show(r.prior.as_ref()),
                r.set_by
                    .clone()
                    .or_else(|| previous.clone())
                    .unwrap_or_else(|| "?".into()),
                profile.name
            ));
            ops.push(match &r.prior {
                Some(v) => PatchOp::set(&r.key, v.clone()),
                None => PatchOp::remove(&r.key),
            });
            restores.push(r.key.clone());
        }
        report.field(
            "restored",
            Value::Array(back.iter().map(|r| json!(r.key)).collect()),
        );
        report.field(
            "left_edited",
            Value::Array(edited.iter().map(|k| json!(k)).collect()),
        );
    }
    let tx = settings_tx::apply(
        ctx,
        Tx {
            writer: "omm profile use",
            ops,
            restores: restores.clone(),
            profile: Some(profile.name.clone()),
        },
        &mut report.converge,
    )?;
    for c in tx.changed.iter().filter(|c| !restores.contains(&c.key)) {
        report.line(format!(
            "  {} = {} (was {})",
            c.key,
            super::show(c.value.as_ref()),
            super::show(c.prior.as_ref())
        ));
    }
    for k in &tx.unchanged {
        report.line(format!("  {k} unchanged"));
    }
    for (k, why) in &tx.kept_structural {
        report.line(format!("  {k} kept: {why}"));
    }
    report.field(
        "changed",
        Value::Array(tx.changed.iter().map(|c| json!(c.key)).collect()),
    );
    report.field(
        "kept_structural",
        Value::Array(tx.kept_structural.iter().map(|(k, _)| json!(k)).collect()),
    );
    if let Some(b) = &tx.backup {
        report.field("backup", json!(b.display().to_string()));
    }

    // Remember the active profile in $OMM/config.json (omm's own state,
    // read by `omm run` and the session-start hook).
    if config.profile() == Some(profile.name.as_str()) {
        report.converge.record(CATEGORY_PROFILE, Action::Unchanged);
    } else {
        config.set_profile(&profile.name);
        if !ctx.dry_run {
            config.save(&omm_root)?;
        }
        report.converge.record(CATEGORY_PROFILE, Action::Updated);
        report.line(format!(
            "{}: profile = {:?}{}",
            OmmConfig::path(&omm_root).display(),
            profile.name,
            if ctx.dry_run { " — dry run" } else { "" }
        ));
    }
    Ok(report)
}

/// `omm profile list`: bundled and custom profiles, keys, the active one.
pub fn list(ctx: &Ctx) -> Result<Table> {
    let omm_root = ctx.omm_root();
    let active = OmmConfig::load(&omm_root)?.profile().map(str::to_string);
    let mut rows: Vec<(String, String, PathBuf)> = Vec::new();
    if let Some(source) = ContentSource::locate(&omm_root) {
        ctx.out.note(format!(
            "content: {} ({})",
            source.root.display(),
            source.origin.as_str()
        ));
        let catalog = source.catalog()?;
        for a in catalog.shipped_of(AssetKind::Profile) {
            if let Some(stem) = json_stem(&a.path) {
                rows.push((
                    stem.to_string(),
                    "bundled".to_string(),
                    source.asset_path(&a.path)?,
                ));
            }
        }
    } else {
        ctx.out
            .warn("no content bundle found; bundled profiles not listed");
    }
    let custom_dir = omm_root
        .join(overlay::CUSTOM_DIR)
        .join(AssetKind::Profile.plural());
    let mut custom: Vec<(String, PathBuf)> = std::fs::read_dir(&custom_dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let name = e.file_name().to_str()?.to_string();
                    let stem = name.strip_suffix(".json")?.to_string();
                    Some((stem, e.path()))
                })
                .collect()
        })
        .unwrap_or_default();
    custom.sort();
    for (stem, path) in custom {
        match rows.iter_mut().find(|(s, _, _)| *s == stem) {
            Some(row) => {
                row.1 = "custom (shadows bundled)".to_string();
                row.2 = path;
            }
            None => rows.push((stem, "custom".to_string(), path)),
        }
    }
    let mut table = Table::new(["profile", "source", "keys", "active"]);
    for (stem, source, path) in rows {
        let keys = match load_slice(&path) {
            Ok(slice) => format!(
                "{} ({} leaf{})",
                slice.keys().cloned().collect::<Vec<_>>().join(" "),
                flatten(&slice).len(),
                if flatten(&slice).len() == 1 { "" } else { "s" }
            ),
            Err(e) => format!("(unreadable: {e})"),
        };
        table.row([
            stem.clone(),
            source,
            keys,
            if active.as_deref() == Some(stem.as_str()) {
                "*"
            } else {
                ""
            }
            .to_string(),
        ]);
    }
    Ok(table)
}

/// `omm profile show <name>`: the slice and each leaf's current value.
pub fn show(ctx: &Ctx, name: &str) -> Result<(String, Value)> {
    let profile = resolve(ctx, name)?;
    let slice = load_slice(&profile.path)?;
    let ops = flatten(&slice);
    let doc = SettingsDoc::for_roots(&ctx.roots).ok();
    let mut lines = vec![format!(
        "profile {} ({}, {})",
        profile.name,
        profile.provider.as_str(),
        profile.path.display()
    )];
    let mut keys = Vec::new();
    for op in &ops {
        let current = doc.as_ref().and_then(|d| d.get(&op.path)).cloned();
        let wanted = op.value.clone().unwrap_or(Value::Null);
        let state = if current.as_ref() == Some(&wanted) {
            "applied"
        } else {
            "differs"
        };
        lines.push(format!(
            "  {} = {} (current {}, {state})",
            op.path.join("."),
            wanted,
            super::show(current.as_ref())
        ));
        keys.push(json!({
            "key": op.path.join("."),
            "value": wanted,
            "current": current,
            "applied": state == "applied",
        }));
    }
    let json = json!({
        "profile": profile.name,
        "provider": profile.provider.as_str(),
        "source": profile.path.display().to_string(),
        "settings": Value::Object(slice),
        "keys": keys,
    });
    Ok((lines.join("\n"), json))
}

// ---- keymap ---------------------------------------------------------------

/// The four keymap contexts (tui-slash-theme.md §2.1) are the validator's
/// business; the local check is the serde shape only: an object of objects
/// of arrays of strings, an empty array meaning "unbound".
pub fn check_keymap_shape(v: &Value) -> std::result::Result<(), String> {
    let Value::Object(contexts) = v else {
        return Err(format!(
            "tui.keymap must be an object, found {}",
            super::json_kind(v)
        ));
    };
    for (ctx_name, actions) in contexts {
        let Value::Object(actions) = actions else {
            return Err(format!(
                "tui.keymap.{ctx_name} must be an object, found {}",
                super::json_kind(actions)
            ));
        };
        for (action, keys) in actions {
            let Value::Array(keys) = keys else {
                return Err(format!(
                    "tui.keymap.{ctx_name}.{action} must be an array of key specs, found {}",
                    super::json_kind(keys)
                ));
            };
            if let Some(bad) = keys.iter().find(|k| !k.is_string()) {
                return Err(format!(
                    "tui.keymap.{ctx_name}.{action} holds {} where a key-spec string is expected",
                    super::json_kind(bad)
                ));
            }
        }
    }
    Ok(())
}

/// Where a keymap preset comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeymapSource {
    /// `omm keymap default`: remove `tui.keymap`.
    HostDefault,
    /// A JSON file.
    File(PathBuf),
}

/// Resolve a preset name: `default`, then `$OMM/custom/keymaps/<name>.json`,
/// `content/keymaps/<name>.json`, then a literal `*.json` path.
pub fn resolve_keymap(ctx: &Ctx, preset: &str) -> Result<KeymapSource> {
    if preset == KEYMAP_DEFAULT {
        return Ok(KeymapSource::HostDefault);
    }
    if preset.ends_with(".json") {
        let path = Path::new(preset);
        if path.is_file() {
            return Ok(KeymapSource::File(omm_host::fsx::canonicalize(path)?));
        }
        return Err(OmmError::Usage(format!("keymap file {preset} not found")));
    }
    check_name("keymap preset", preset)?;
    let omm_root = ctx.omm_root();
    let custom = omm_root
        .join(overlay::CUSTOM_DIR)
        .join(KEYMAPS_DIR)
        .join(format!("{preset}.json"));
    if custom.is_file() {
        return Ok(KeymapSource::File(custom));
    }
    if let Some(source) = ContentSource::locate(&omm_root) {
        let bundled = source.root.join(KEYMAPS_DIR).join(format!("{preset}.json"));
        if bundled.is_file() {
            return Ok(KeymapSource::File(bundled));
        }
    }
    Err(OmmError::Usage(format!(
        "unknown keymap preset `{preset}`: use `{KEYMAP_DEFAULT}` (the host's bindings), a file at {}, or a path to a .json file holding {{\"<context>\": {{\"<action>\": [\"<key-spec>\"]}}}}",
        custom.display()
    )))
}

/// `omm keymap <preset>`.
pub fn keymap(ctx: &Ctx, preset: &str) -> Result<WriteReport> {
    let source = resolve_keymap(ctx, preset)?;
    let mut report = WriteReport::new(ctx.dry_run);
    report.field("preset", json!(preset));
    let op = match &source {
        KeymapSource::HostDefault => {
            report.line("keymap default: tui.keymap removed (the host's bindings apply)");
            PatchOp::remove("tui.keymap")
        }
        KeymapSource::File(path) => {
            let bytes = read_regular_file(path)?;
            let value: Value = serde_json::from_slice(&bytes)
                .map_err(|e| OmmError::Usage(format!("{}: not JSON: {e}", path.display())))?;
            check_keymap_shape(&value)
                .map_err(|why| OmmError::Usage(format!("{}: {why}", path.display())))?;
            let bindings: usize = value
                .as_object()
                .map(|c| {
                    c.values()
                        .filter_map(Value::as_object)
                        .map(|a| a.len())
                        .sum()
                })
                .unwrap_or(0);
            report.field("source", json!(path.display().to_string()));
            report.line(format!(
                "keymap {preset}: {} ({bindings} binding{})",
                path.display(),
                if bindings == 1 { "" } else { "s" }
            ));
            PatchOp::set("tui.keymap", value)
        }
    };
    let tx = settings_tx::apply(
        ctx,
        Tx {
            writer: "omm keymap",
            ops: vec![op],
            ..Tx::default()
        },
        &mut report.converge,
    )?;
    match tx.changed.first() {
        Some(c) => report.line(format!(
            "tui.keymap {} (was {}){}",
            match &c.value {
                Some(_) => "set".to_string(),
                None => "removed".to_string(),
            },
            super::show(c.prior.as_ref()),
            if ctx.dry_run { " — dry run" } else { "" }
        )),
        None => report.line("tui.keymap unchanged"),
    };
    if let Some(b) = &tx.backup {
        report.field("backup", json!(b.display().to_string()));
    }
    Ok(report)
}

/// `omm keymap list`: `default` plus the custom and bundled preset files.
pub fn keymap_list(ctx: &Ctx) -> Result<Table> {
    let mut table = Table::new(["preset", "source"]);
    table.row([KEYMAP_DEFAULT, "host defaults (removes tui.keymap)"]);
    let omm_root = ctx.omm_root();
    let mut dirs = vec![(
        omm_root.join(overlay::CUSTOM_DIR).join(KEYMAPS_DIR),
        "custom",
    )];
    if let Some(source) = ContentSource::locate(&omm_root) {
        dirs.push((source.root.join(KEYMAPS_DIR), "bundled"));
    }
    for (dir, label) in dirs {
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .filter_map(|e| {
                        e.file_name()
                            .to_str()
                            .and_then(|n| n.strip_suffix(".json"))
                            .map(str::to_string)
                    })
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        for n in names {
            table.row([n, label.to_string()]);
        }
    }
    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slices_flatten_to_leaves_and_keep_arrays_whole() {
        let slice: Map<String, Value> = serde_json::from_str(
            r#"{"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence","excluded_tool_names":[]},"empty":{}},"reasoning_effort":"medium"}"#,
        )
        .unwrap();
        let ops = flatten(&slice);
        let keys: Vec<String> = ops.iter().map(|o| o.path.join(".")).collect();
        assert_eq!(
            keys,
            vec![
                "run.context_slimming.skill_catalog_descriptions",
                "run.context_slimming.excluded_tool_names",
                "run.empty",
                "reasoning_effort"
            ]
        );
        assert_eq!(ops[1].value, Some(json!([])));
        assert_eq!(ops[2].value, Some(json!({})));
    }

    #[test]
    fn bundled_profiles_load_and_relist_the_default_full_ids() {
        // The four shipped profiles are the fixtures here; each must pass the
        // slice check and the R18 guard the transaction applies.
        let tmp = tempfile::tempdir().unwrap();
        let src = ContentSource::locate(tmp.path()).expect("content");
        let catalog = src.catalog().unwrap();
        let mut seen = 0;
        for a in catalog.shipped_of(AssetKind::Profile) {
            let slice = load_slice(&src.asset_path(&a.path).unwrap()).unwrap();
            let ops = flatten(&slice);
            let staged = json!({"run": slice.get("run").cloned().unwrap_or(Value::Null)});
            settings_tx::guard_full_ids(&ops, &staged).unwrap_or_else(|e| panic!("{}: {e}", a.id));
            seen += 1;
        }
        assert!(seen >= 4, "{seen}");
    }

    #[test]
    fn a_slice_with_a_foreign_key_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("x.json");
        std::fs::write(&p, br#"{"bogus": 1}"#).unwrap();
        assert!(matches!(load_slice(&p), Err(OmmError::Usage(_))));
        std::fs::write(&p, br#"{"mcp_servers": {}}"#).unwrap();
        assert!(matches!(load_slice(&p), Err(OmmError::Usage(_))));
        std::fs::write(&p, br#"{"schema_version": 1}"#).unwrap();
        assert!(matches!(load_slice(&p), Err(OmmError::Usage(_))));
        std::fs::write(&p, b"[]").unwrap();
        assert!(matches!(load_slice(&p), Err(OmmError::Usage(_))));
        std::fs::write(&p, br#"{"provider": "meta"}"#).unwrap();
        assert_eq!(load_slice(&p).unwrap().len(), 1);
    }

    #[test]
    fn keymap_shape_check() {
        assert!(check_keymap_shape(&json!({})).is_ok());
        assert!(check_keymap_shape(&json!({"app": {}})).is_ok());
        assert!(
            check_keymap_shape(&json!({"app": {"clear-terminal": ["ctrl+q", "alt+j"]}})).is_ok()
        );
        assert!(check_keymap_shape(&json!({"editor": {"transpose": []}})).is_ok());
        assert!(check_keymap_shape(&json!(5)).is_err());
        assert!(check_keymap_shape(&json!({"app": 5})).is_err());
        assert!(check_keymap_shape(&json!({"app": {"commands": 5}})).is_err());
        assert!(check_keymap_shape(&json!({"app": {"commands": [5]}})).is_err());
        assert!(check_keymap_shape(&json!({"app": {"commands": [{}]}})).is_err());
    }
}
