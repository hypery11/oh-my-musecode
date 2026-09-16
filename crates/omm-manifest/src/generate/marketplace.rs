//! The three catalogs at the repo root, EXACTLY as
//! `docs/experiments/marketplace-precedence.md` §6.1 specifies — one file per
//! consumer, each pointing at that consumer's package:
//!
//! | file | parser | points at |
//! |---|---|---|
//! | `marketplace.json` | Muse (native, probed FIRST; the only file Muse reads) | `plugins/omm` |
//! | `.agents/plugins/marketplace.json` | hosts of the Codex schema | `dist/codex` |
//! | `.claude-plugin/marketplace.json` | hosts of the Claude schema | `dist/claude` |
//!
//! Native shape (§2, all fields required): `schemaVersion:1`, `source:"local"`,
//! per entry `name` (== manifest `name`, a mismatch fails only at install —
//! §3.2 B5), `version`, `install{transport:"local-path",source:<rel>}`,
//! `integrity{digest:"sha256:<64hex>"}` (verified at INSTALL, not at add —
//! a stale digest breaks every user's install, §6.3), `availability{status}`.
//! The digest is Muse's content-addressed `package_sha256`; its construction
//! did not match 16 candidate schemes, so it is never computed here — the
//! binary reports it after a temp Codex-catalog add
//! ([`package_digest`], §2: `plugins validate --json` does NOT emit it).
//!
//! Codex entry: `name` + `source` OBJECT `{"source":"local","path":<rel>}`
//! (a string source is skipped, §3.3). Claude entry: `name` + `source`
//! STRING (`./dist/claude`; an object source — even a local one — is remote
//! and skipped under a local-dir marketplace, §3.3); marketplace `name` must
//! be `ohmy` so `omm@ohmy` works in every tool (§6.1 consequence 3).

use omm_host::probe;
use omm_host::{Invoker, Sandbox};
use serde_json::{json, Value};

use crate::error::{ManifestError, Result};
use crate::generate::{native, Package};
use crate::hr;

/// The `$schema` id of the Claude-schema marketplace file — the literal the
/// binary itself carries (`research/musecode/plugins.md` §10.2).
pub const CLAUDE_MARKETPLACE_SCHEMA: &str =
    "https://json.schemastore.org/claude-code-marketplace.json";
/// `owner.name` of the Claude-schema catalog (§6.1).
pub const MARKETPLACE_OWNER: &str = "oh-my-musecode";
/// `availability.status` of a shipped entry (`hr::MARKETPLACE_NATIVE_*`).
pub const AVAILABILITY_AVAILABLE: &str = "available";
/// The throwaway marketplace name [`package_digest`] registers in its
/// sandbox (never the user's data root; not `tbh-curated`).
pub const DIGEST_MARKETPLACE_NAME: &str = "omm-digest";

/// `sha256:` + 64 lowercase hex.
pub fn is_digest(s: &str) -> bool {
    s.strip_prefix(hr::MARKETPLACE_DIGEST_PREFIX)
        .map(|h| h.len() == 64 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')))
        .unwrap_or(false)
}

/// The native root catalog.
pub fn render_native(plugin_id: &str, version: &str, source_rel: &str, digest: &str) -> Value {
    json!({
        "schemaVersion": hr::MARKETPLACE_NATIVE_SCHEMA_VERSION,
        "source": hr::MARKETPLACE_NATIVE_SOURCE,
        "plugins": [
            {
                "name": plugin_id,
                "version": version,
                "install": { "transport": hr::MARKETPLACE_NATIVE_TRANSPORT, "source": source_rel },
                "integrity": { "digest": digest },
                "availability": { "status": AVAILABILITY_AVAILABLE },
            }
        ]
    })
}

/// The Codex-schema catalog (`.agents/plugins/marketplace.json`).
pub fn render_codex(plugin_id: &str, version: &str, description: &str, path_rel: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "plugins": [
            {
                "name": plugin_id,
                "version": version,
                "description": description,
                "source": { "source": "local", "path": path_rel },
            }
        ]
    })
}

/// The Claude-schema catalog (`.claude-plugin/marketplace.json`).
pub fn render_claude(
    marketplace_name: &str,
    plugin_id: &str,
    version: &str,
    description: &str,
    source_rel: &str,
) -> Value {
    json!({
        "$schema": CLAUDE_MARKETPLACE_SCHEMA,
        "name": marketplace_name,
        "owner": { "name": MARKETPLACE_OWNER },
        "plugins": [
            {
                "name": plugin_id,
                "version": version,
                "description": description,
                "source": source_rel,
            }
        ]
    })
}

/// Ask the binary for the package's `package_sha256`
/// (marketplace-precedence.md §2, `muse_digest()`): write the package beside
/// a one-entry Codex catalog in a temp marketplace root, `marketplace add`
/// it under a throwaway XDG sandbox, and read `plugins list --available
/// --json → available[].digest`. ~60 ms, offline, nothing touches the user's
/// data root. The package must be a native one (its manifest `name` is the
/// catalog entry name).
pub fn package_digest(inv: &Invoker, pkg: &Package) -> Result<String> {
    let manifest = pkg.get_json(&native::manifest_path())?;
    let name = manifest
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| ManifestError::Generate("native manifest has no `name`".to_string()))?
        .to_string();
    let tmp = tempfile::Builder::new()
        .prefix("omm-digest-")
        .tempdir()
        .map_err(|e| ManifestError::io("create temp dir", std::env::temp_dir(), e))?;
    let sb = Sandbox::create(&tmp.path().join("sandbox"))?;
    let ws = tmp.path().join("ws");
    omm_host::fsx::create_dir_all(&ws)?;
    let mkt = tmp.path().join("mkt");
    pkg.write_to(&mkt.join("pkg"))?;
    let codex_catalog = json!({
        "schemaVersion": 1,
        "plugins": [ { "name": name, "source": { "source": "local", "path": "pkg" } } ]
    });
    let catalog_path = mkt.join(hr::MARKETPLACE_PROBE_ORDER[1]);
    let catalog_root = omm_host::fsx::canonicalize(&mkt)?;
    super::write_file(
        &catalog_root,
        &catalog_path,
        &super::json_bytes(&codex_catalog)?,
    )?;
    let probe = inv.clone().sandboxed(&sb).cwd(&ws);
    let add = probe.run_json(&[
        "plugins".to_string(),
        "marketplace".to_string(),
        "add".to_string(),
        DIGEST_MARKETPLACE_NAME.to_string(),
        mkt.to_string_lossy().into_owned(),
        "--json".to_string(),
    ])?;
    if !add.outcome.ok() {
        let detail = add
            .outcome
            .host_reported_error()
            .map(|e| e.to_string())
            .unwrap_or_else(|| add.outcome.stderr.lines().next().unwrap_or("").to_string());
        return Err(ManifestError::HostShape(format!(
            "marketplace add for the digest probe failed (exit {:?}): {detail}",
            add.outcome.code
        )));
    }
    let count = add.json["marketplace"]["plugin_count"]
        .as_u64()
        .unwrap_or(0);
    let skipped = add.json["marketplace"]["skipped"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    if count != 1 || skipped != 0 {
        return Err(ManifestError::HostShape(format!(
            "digest probe: marketplace add listed {count} plugin(s), skipped {skipped}: {}",
            add.json
        )));
    }
    let list = probe.run_json(&["plugins", "list", "--available", "--json"])?;
    let digest = list.json["available"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|r| r["name"].as_str() == Some(name.as_str()))
                .and_then(|r| r["digest"].as_str())
                .map(str::to_string)
        })
        .ok_or_else(|| {
            ManifestError::HostShape(format!(
                "plugins list --available reported no `{name}` entry with a digest: {}",
                list.json
            ))
        })?;
    if !is_digest(&digest) {
        return Err(ManifestError::HostShape(format!(
            "digest {digest:?} is not `sha256:` + 64 hex"
        )));
    }
    Ok(digest)
}

/// R11's install-side check for CI (marketplace-precedence.md §6.3), run in a
/// throwaway sandbox against a marketplace root (the repo, or a copy):
/// `marketplace add <name> <root>` → `plugin_count 1, skipped []`;
/// `list --available` → one `<pid>` entry, `status available`, its digest;
/// `install <pid>@<name>` → `manifest_family == "native"` and
/// `package_sha256 == digest`. Returns the digest the install verified.
pub fn verify_install(
    inv: &Invoker,
    marketplace_root: &std::path::Path,
    plugin_id: &str,
) -> Result<InstallCheck> {
    let tmp = tempfile::Builder::new()
        .prefix("omm-install-check-")
        .tempdir()
        .map_err(|e| ManifestError::io("create temp dir", std::env::temp_dir(), e))?;
    let sb = Sandbox::create(&tmp.path().join("sandbox"))?;
    let ws = tmp.path().join("ws");
    omm_host::fsx::create_dir_all(&ws)?;
    let probe = inv.clone().sandboxed(&sb).cwd(&ws);
    let name = "ci";
    let add = probe.run_json(&[
        "plugins".to_string(),
        "marketplace".to_string(),
        "add".to_string(),
        name.to_string(),
        marketplace_root.to_string_lossy().into_owned(),
        "--json".to_string(),
    ])?;
    let plugin_count = add.json["marketplace"]["plugin_count"]
        .as_u64()
        .unwrap_or(0);
    let skipped: Vec<Value> = add.json["marketplace"]["skipped"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let list = probe.run_json(&["plugins", "list", "--available", "--json"])?;
    let entry = list.json["available"]
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|r| r["name"].as_str() == Some(plugin_id))
                .cloned()
        })
        .unwrap_or(Value::Null);
    let listed_digest = entry["digest"].as_str().map(str::to_string);
    let status = entry["status"].as_str().map(str::to_string);
    let install = probe.run_json(&[
        "plugins".to_string(),
        "install".to_string(),
        format!("{plugin_id}@{name}"),
        "--json".to_string(),
    ])?;
    let installed = &install.json["installed"];
    Ok(InstallCheck {
        add_ok: add.outcome.ok(),
        plugin_count,
        skipped,
        status,
        listed_digest,
        install_ok: install.outcome.ok(),
        manifest_family: installed["manifest_family"].as_str().map(str::to_string),
        package_sha256: installed["package_sha256"].as_str().map(str::to_string),
        install_error: install.outcome.host_reported_error().map(|e| e.to_string()),
        inspect: probe::plugins_inspect(&probe, plugin_id).ok(),
    })
}

/// What [`verify_install`] observed.
#[derive(Debug)]
pub struct InstallCheck {
    pub add_ok: bool,
    pub plugin_count: u64,
    pub skipped: Vec<Value>,
    pub status: Option<String>,
    pub listed_digest: Option<String>,
    pub install_ok: bool,
    pub manifest_family: Option<String>,
    pub package_sha256: Option<String>,
    pub install_error: Option<String>,
    pub inspect: Option<probe::PluginInspect>,
}

impl InstallCheck {
    /// The §6.3 gate: everything as expected and the digest round-tripped.
    pub fn passes(&self) -> Vec<String> {
        let mut failed = Vec::new();
        if !self.add_ok {
            failed.push("marketplace add failed".to_string());
        }
        if self.plugin_count != 1 {
            failed.push(format!("plugin_count {} != 1", self.plugin_count));
        }
        if !self.skipped.is_empty() {
            failed.push(format!("skipped: {:?}", self.skipped));
        }
        if self.status.as_deref() != Some(AVAILABILITY_AVAILABLE) {
            failed.push(format!("status {:?}", self.status));
        }
        if !self.install_ok {
            failed.push(format!("install failed: {:?}", self.install_error));
        }
        if self.manifest_family.as_deref() != Some(hr::MANIFEST_FAMILY_NATIVE) {
            failed.push(format!("manifest_family {:?}", self.manifest_family));
        }
        if self.package_sha256.is_none() || self.package_sha256 != self.listed_digest {
            failed.push(format!(
                "package_sha256 {:?} != listed digest {:?}",
                self.package_sha256, self.listed_digest
            ));
        }
        failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_shapes_follow_section_6_1() {
        let d = format!("sha256:{}", "a".repeat(64));
        assert!(is_digest(&d));
        assert!(!is_digest("sha256:abc"));
        assert!(!is_digest(&format!("sha1:{}", "a".repeat(64))));
        assert!(!is_digest(&format!("sha256:{}", "A".repeat(64))));
        let n = render_native("omm", "0.1.0", "plugins/omm", &d);
        assert_eq!(n["schemaVersion"], 1);
        assert_eq!(n["source"], "local");
        assert_eq!(n["plugins"][0]["install"]["transport"], "local-path");
        assert_eq!(n["plugins"][0]["install"]["source"], "plugins/omm");
        assert_eq!(n["plugins"][0]["integrity"]["digest"], d);
        assert_eq!(n["plugins"][0]["availability"]["status"], "available");
        let c = render_codex("omm", "0.1.0", "d", "dist/codex");
        assert_eq!(c["plugins"][0]["source"]["source"], "local");
        assert_eq!(c["plugins"][0]["source"]["path"], "dist/codex");
        let cl = render_claude("ohmy", "omm", "0.1.0", "d", "./dist/claude");
        assert_eq!(cl["name"], "ohmy");
        assert!(cl["plugins"][0]["source"].is_string());
        assert_eq!(cl["plugins"][0]["source"], "./dist/claude");
    }
}
