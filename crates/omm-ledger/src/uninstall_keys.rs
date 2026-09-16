//! Dotted settings-key helpers for the uninstall planner (split from
//! [`crate::uninstall`]).
//!
//! These are pure apart from [`still_ours`] and [`shared_file_key`], which
//! read one file at most: splitting a dotted registration path, reading a
//! nested value, deciding whether a registration still holds what omm wrote,
//! and pruning a restored-to-absent leaf with its newly-empty parents. The
//! plan ordering in [`crate::uninstall`] (`order_structural_last`,
//! `predict_structural_keep`) builds on [`remove_leaf_and_prune`].

use std::fs;
use std::path::Path;

use omm_host::host_reality as hr;
use omm_host::settings;
use serde_json::Value;

use crate::containment::Bases;
use crate::schema::{Base, EntryKey, RelPath};

/// The ledger key a shared file is reported under: its path relative to the
/// config base when it lies there, else its file name.
pub(crate) fn shared_file_key(bases: &Bases, file: &Path) -> Option<EntryKey> {
    let rel = file
        .strip_prefix(&bases.muse_config)
        .ok()
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .or_else(|| file.file_name().map(|n| n.to_string_lossy().into_owned()))?;
    RelPath::new(rel).ok().map(|path| EntryKey {
        base: Base::MuseConfig,
        path,
    })
}

/// The value of a dotted key in a JSON document (`None` when absent).
pub(crate) fn dotted_value<'a>(doc: &'a Value, dotted: &str) -> Option<&'a Value> {
    let mut cur = doc;
    for seg in dotted.split('.').filter(|s| !s.is_empty()) {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

/// Whether a settings key / trust entry registration is still what omm
/// wrote: `true` → restore, `false` → the user edited it (preserve unless
/// force). A missing file, an unreadable file, or a registration without
/// `value` counts as ours (the undoer decides the rest).
pub(crate) fn still_ours(
    file: Option<&Path>,
    lookup: impl Fn(&Value) -> Option<Value>,
    value: &Option<Value>,
) -> bool {
    let (Some(file), Some(value)) = (file, value) else {
        return true;
    };
    let Ok(bytes) = fs::read(file) else {
        return true;
    };
    let Ok(doc) = serde_json::from_slice::<Value>(&bytes) else {
        return true;
    };
    lookup(&doc).as_ref() == Some(value)
}

/// The dotted key of a settings registration as path segments.
pub(crate) fn segments(dotted: &str) -> Vec<String> {
    dotted
        .split('.')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// True for `<top>.<member>` where `member` is a required structural member
/// of `top`'s object (settings-keys.json `structural`; Gate 1 decision D).
pub(crate) fn is_structural_leaf(path: &[String]) -> bool {
    match path {
        [top, leaf] => hr::settings_keys()
            .map(|k| k.structural_required(top).iter().any(|r| r == leaf))
            .unwrap_or(false),
        _ => false,
    }
}

fn value_at_mut<'a>(root: &'a mut Value, path: &[String]) -> Option<&'a mut Value> {
    let mut cur = root;
    for p in path {
        cur = cur.get_mut(p)?;
    }
    Some(cur)
}

/// Remove the leaf at `path` and the empty objects that leaves above it,
/// the top level included — what the undoer does for a key restored to
/// absent.
pub(crate) fn remove_leaf_and_prune(doc: &mut Value, path: &[String]) {
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

/// The plan's prediction of [`settings::structural_keep_reason`] for a
/// structural leaf this plan restores to absent: every other key the plan
/// restores to absent is removed from a copy of the document first (the
/// leaf is restored last, so that is the document it will see), then the
/// rule decides. `None` when the leaf may go.
pub(crate) fn predict_structural_keep(
    doc: Option<&Value>,
    path: &[String],
    removals: &[Vec<String>],
) -> Option<String> {
    let mut sim = doc?.clone();
    for other in removals.iter().filter(|o| o.as_slice() != path) {
        remove_leaf_and_prune(&mut sim, other);
    }
    settings::structural_keep_reason(&sim, path).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn segments_split_and_skip_empties() {
        assert_eq!(segments("a.b.c"), vec!["a", "b", "c"]);
        assert_eq!(segments(".a..b."), vec!["a", "b"]);
        assert!(segments("").is_empty());
    }

    #[test]
    fn dotted_value_reads_nested_keys() {
        let doc = json!({"a": {"b": {"c": 1}}});
        assert_eq!(dotted_value(&doc, "a.b.c"), Some(&json!(1)));
        assert_eq!(dotted_value(&doc, "a.b.missing"), None);
        assert_eq!(dotted_value(&doc, ""), Some(&doc));
    }

    #[test]
    fn remove_leaf_and_prune_removes_empty_parents_only() {
        let mut doc = json!({"permissions": {"schema_version": 1, "profiles": {"omm-strict": {"extends": 1}}}});
        remove_leaf_and_prune(
            &mut doc,
            &segments("permissions.profiles.omm-strict.extends"),
        );
        assert_eq!(doc, json!({"permissions": {"schema_version": 1}}));
        remove_leaf_and_prune(&mut doc, &segments("permissions.schema_version"));
        assert_eq!(doc, json!({}));
        // A leaf whose siblings survive is removed without touching parents.
        let mut doc = json!({"a": {"keep": 1, "drop": 2}});
        remove_leaf_and_prune(&mut doc, &segments("a.drop"));
        assert_eq!(doc, json!({"a": {"keep": 1}}));
    }

    #[test]
    fn still_ours_treats_missing_or_valueless_as_ours() {
        assert!(still_ours(None, |_| None, &Some(json!(1))));
        assert!(still_ours(None, |_| None, &None));
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such.json");
        assert!(still_ours(Some(&missing), |_| None, &Some(json!(1))));
        let file = dir.path().join("doc.json");
        std::fs::write(&file, br#"{"a": 1}"#).unwrap();
        assert!(still_ours(
            Some(&file),
            |doc| dotted_value(doc, "a").cloned(),
            &Some(json!(1))
        ));
        assert!(!still_ours(
            Some(&file),
            |doc| dotted_value(doc, "a").cloned(),
            &Some(json!(2))
        ));
    }
}
