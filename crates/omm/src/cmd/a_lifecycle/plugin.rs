//! The plugin bundle transaction — the measured sequence of
//! `docs/experiments/marketplace-precedence.md` §6.4 (ARCHITECTURE.md §7,
//! R11, R14):
//!
//! install: `plugins marketplace add <name> <checkout>` (``already
//! configured`` → `marketplace update <name>`) → `plugins list --available`
//! (the entry must be `status: available`; keep its `digest`) → `plugins
//! install <pid>@<name>` (assert `manifest_family == "native"` and
//! `package_sha256 == digest`) → approve every runtime capability → verify
//! through `plugins inspect` that each line is PRESENT and literally
//! `trusted_enabled` → cross-check `skills list`.
//!
//! update: `marketplace update` (rc ≠ 0 → stop, nothing changed) → digest
//! compare against the installed `package_sha256` (equal → no-op, never
//! remove) → pre-flight install in a throwaway sandbox → `plugins remove`
//! → `plugins install` → approve → verify. **Never `plugins update <pid>`**:
//! it refreshes the pinned generation and fails with
//! `plugin-source-unavailable` after two rotations (§5.3, R14).
//!
//! Every host verb runs through the R20-allowlisted `Invoker`, which adds the
//! install-time gate for `plugins …` on its own. What the host recorded lands
//! in the ledger as registrations (`muse-marketplace {name, source}`,
//! `muse-plugin {id, package_sha256, generation_path, approved}`), never as
//! file entries: the package cache under the data root is the host's.
//!
//! A capability whose catalog row says `enabled_default: false` is left
//! `review_needed` on purpose: `plugins approve` enables whatever it trusts
//! (measured 2026-09-02 on 1.0.1-R2006.1 — approving the default-off
//! reminder made it `trusted_enabled`), so approving it would activate a
//! reminder the content author shipped off. Such ids are reported, kept out
//! of the registration's `approved` list (what doctor D1 asserts), and the
//! user enables them with `muse plugins approve <stable-id>`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use omm_doctor::check::{self, DeclaredCapability, InstalledPlugin};
use omm_host::host_reality as hr;
use omm_host::probe::{self, SkillsListOptions};
use omm_host::{Invoker, Outcome};
use omm_ledger::audit::{Event, ACTION_APPROVE, ACTION_INSTALL, ACTION_UPDATE};
use omm_ledger::{Ledger, Registration};
use omm_manifest::catalog::AssetKind;
use omm_manifest::generate::marketplace;

use super::files;
use super::session::Session;
use super::source::Source;
use crate::cmd::Ctx;
use crate::error::{OmmError, Result};
use crate::output::{Action, Converge};

/// The converge categories this module reports under.
pub const CAT_MARKETPLACE: &str = "marketplace";
pub const CAT_PLUGIN: &str = "plugin";
pub const CAT_CAPABILITIES: &str = "capabilities";

/// A failure of this transaction that is neither a host error nor a ledger
/// error: a verification the host passed but omm refuses (exit 1).
fn fail(context: &str, detail: impl Into<String>) -> OmmError {
    OmmError::io(context, std::io::Error::other(detail.into()))
}

/// The host's `{"error":{code,message}}` or its first stderr line.
fn host_failure(out: &Outcome, what: &str) -> OmmError {
    match out.host_reported_error() {
        Some(e) => fail(what, e.to_string()),
        None => fail(
            what,
            format!(
                "exit {:?}: {}",
                out.code,
                out.stderr.lines().next().unwrap_or("").trim()
            ),
        ),
    }
}

/// `error.code` of a host document, if any.
fn error_code(out: &Outcome) -> Option<String> {
    out.first_json()
        .ok()?
        .get("error")?
        .get("code")?
        .as_str()
        .map(str::to_string)
}

// ---------------------------------------------------------------------------
// marketplace
// ---------------------------------------------------------------------------

/// What `marketplace add` / `update` / `list` reported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketplaceState {
    pub name: String,
    /// `source.kind`: `local-dir | local-file | git`.
    pub kind: String,
    /// `source.path` as the host recorded it (a path or a git URL).
    pub source: String,
    /// `source.worktree_path` (git sources), data-root-relative.
    pub worktree_path: Option<String>,
    pub plugin_count: u64,
    pub skipped: Vec<Value>,
    /// `Updated` when the marketplace was added, `Unchanged` when it was
    /// already configured and its snapshot refreshed.
    pub action: Action,
}

fn parse_marketplace(name: &str, m: &Value, action: Action) -> Result<MarketplaceState> {
    let state = MarketplaceState {
        name: name.to_string(),
        kind: m["source"]["kind"].as_str().unwrap_or("").to_string(),
        source: m["source"]["path"].as_str().unwrap_or("").to_string(),
        worktree_path: m["source"]["worktree_path"].as_str().map(str::to_string),
        plugin_count: m["plugin_count"].as_u64().unwrap_or(0),
        skipped: m["skipped"].as_array().cloned().unwrap_or_default(),
        action,
    };
    if state.plugin_count == 0 || !state.skipped.is_empty() {
        return Err(fail(
            "marketplace",
            format!(
                "`{name}` lists {} plugin(s), skipped {}: the catalog is unusable ({m})",
                state.plugin_count,
                state.skipped.len()
            ),
        ));
    }
    Ok(state)
}

/// `plugins marketplace add <name> <source>`; on ``marketplace `<name>` is
/// already configured`` → `marketplace update <name>` (a fresh generation,
/// rolling keep-2 — §5.2). A marketplace of that name configured from a
/// different local directory is refused rather than silently updated.
pub fn ensure_marketplace(inv: &Invoker, name: &str, source: &Path) -> Result<MarketplaceState> {
    let src = source.to_string_lossy().into_owned();
    let add = inv.run_json(&["plugins", "marketplace", "add", name, &src, "--json"])?;
    if add.outcome.ok() {
        return parse_marketplace(name, &add.json["marketplace"], Action::Updated);
    }
    let already = add
        .outcome
        .host_reported_error()
        .map(|e| e.to_string().contains("already configured"))
        .unwrap_or(false);
    if !already {
        return Err(host_failure(&add.outcome, "plugins marketplace add"));
    }
    let state = update_marketplace(inv, name)?;
    if state.kind == "local-dir" && !same_dir(&state.source, source) {
        return Err(OmmError::Usage(format!(
            "marketplace `{name}` is configured from {} but this install's source is {}; run `muse plugins marketplace remove {name}` first or pass --source {}",
            state.source,
            source.display(),
            state.source
        )));
    }
    Ok(state)
}

/// `plugins marketplace update <name>`; rc ≠ 0 leaves the host untouched
/// (§5.2: a failed refresh is atomic).
pub fn update_marketplace(inv: &Invoker, name: &str) -> Result<MarketplaceState> {
    let upd = inv.run_json(&["plugins", "marketplace", "update", name, "--json"])?;
    if !upd.outcome.ok() {
        return Err(host_failure(&upd.outcome, "plugins marketplace update"));
    }
    parse_marketplace(name, &upd.json["marketplace"], Action::Unchanged)
}

/// `error.code == unknown-marketplace` (``marketplace `<n>` is not configured``).
pub fn is_unknown_marketplace(out: &Outcome) -> bool {
    error_code(out).as_deref() == Some("unknown-marketplace")
        || out
            .host_reported_error()
            .map(|e| e.to_string().contains("is not configured"))
            .unwrap_or(false)
}

/// The configured marketplace named `name`, from `marketplace list --json`.
pub fn configured_marketplace(inv: &Invoker, name: &str) -> Result<Option<MarketplaceState>> {
    let list = inv.run_json(&["plugins", "marketplace", "list", "--json"])?;
    if !list.outcome.ok() {
        return Err(host_failure(&list.outcome, "plugins marketplace list"));
    }
    let Some(row) = list.json["marketplaces"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["name"].as_str() == Some(name)))
    else {
        return Ok(None);
    };
    Ok(Some(parse_marketplace(name, row, Action::Unchanged)?))
}

fn same_dir(recorded: &str, wanted: &Path) -> bool {
    let a = std::fs::canonicalize(recorded).unwrap_or_else(|_| PathBuf::from(recorded));
    let b = std::fs::canonicalize(wanted).unwrap_or_else(|_| wanted.to_path_buf());
    a == b
}

// ---------------------------------------------------------------------------
// available / installed / install / remove
// ---------------------------------------------------------------------------

/// The `plugins list --available --json` row of our plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Available {
    pub digest: String,
    pub status: String,
    pub version: String,
    /// `install.source` — the package directory the host will read.
    pub source: String,
}

/// The marketplace entry of `pid` in `name`; `available` with a digest or an error.
pub fn available(inv: &Invoker, name: &str, pid: &str) -> Result<Available> {
    let list = inv.run_json(&["plugins", "list", "--available", "--json"])?;
    if !list.outcome.ok() {
        return Err(host_failure(&list.outcome, "plugins list --available"));
    }
    let row = list.json["available"]
        .as_array()
        .and_then(|rows| {
            rows.iter().find(|r| {
                r["name"].as_str() == Some(pid)
                    && r["marketplace"].as_str().map(|m| m == name).unwrap_or(true)
            })
        })
        .ok_or_else(|| {
            fail(
                "plugins list --available",
                format!(
                    "marketplace `{name}` offers no `{pid}` entry: {}",
                    list.json
                ),
            )
        })?;
    let entry = Available {
        digest: row["digest"].as_str().unwrap_or("").to_string(),
        status: row["status"].as_str().unwrap_or("").to_string(),
        version: row["version"].as_str().unwrap_or("").to_string(),
        source: row["install"]["source"].as_str().unwrap_or("").to_string(),
    };
    if entry.status != marketplace::AVAILABILITY_AVAILABLE {
        return Err(fail(
            "plugins list --available",
            format!(
                "`{pid}` in marketplace `{name}` is {:?}, not {:?}; the host refuses to install it",
                entry.status,
                marketplace::AVAILABILITY_AVAILABLE
            ),
        ));
    }
    if !marketplace::is_digest(&entry.digest) {
        return Err(fail(
            "plugins list --available",
            format!(
                "`{pid}` digest {:?} is not `sha256:` + 64 hex",
                entry.digest
            ),
        ));
    }
    Ok(entry)
}

/// Every installed plugin id (`plugins list --json`) — the host's view for
/// the uninstall plan (Gate 1 decision B).
pub fn installed_ids(inv: &Invoker) -> Result<Vec<String>> {
    Ok(check::plugins_list(inv)?
        .plugins
        .into_iter()
        .map(|p| p.id)
        .collect())
}

/// Every configured marketplace name (`plugins marketplace list --json`).
pub fn configured_marketplaces(inv: &Invoker) -> Result<Vec<String>> {
    let list = inv.run_json(&["plugins", "marketplace", "list", "--json"])?;
    if !list.outcome.ok() {
        return Err(host_failure(&list.outcome, "plugins marketplace list"));
    }
    Ok(list.json["marketplaces"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default())
}

/// The installed record of `pid`, if any (`plugins list --json`).
pub fn installed(inv: &Invoker, pid: &str) -> Result<Option<InstalledPlugin>> {
    Ok(check::plugins_list(inv)?
        .plugins
        .into_iter()
        .find(|p| p.id == pid))
}

/// What `plugins install` recorded.
#[derive(Clone, Debug)]
pub struct Installed {
    pub package_sha256: String,
    pub manifest_family: String,
    /// `installed.source.path` — the generation (git) or the live directory
    /// (local-dir) the package was read from.
    pub source_path: String,
    pub warning: Option<String>,
    pub declared: Vec<DeclaredCapability>,
}

/// `plugins install <pid>@<name>`, asserting `manifest_family == "native"`
/// (R11, D10) and `package_sha256 == expected` (§6.3). A package that lands
/// under a foreign family is removed again before the error is returned.
pub fn install(inv: &Invoker, pid: &str, name: &str, expected_digest: &str) -> Result<Installed> {
    let spec = format!("{pid}@{name}");
    let out = inv.run_json(&["plugins", "install", &spec, "--json"])?;
    if !out.outcome.ok() {
        return Err(host_failure(&out.outcome, "plugins install"));
    }
    let rec = &out.json["installed"];
    let installed = Installed {
        package_sha256: rec["package_sha256"].as_str().unwrap_or("").to_string(),
        manifest_family: rec["manifest_family"].as_str().unwrap_or("").to_string(),
        source_path: rec["source"]["path"].as_str().unwrap_or("").to_string(),
        warning: out.json["warning"].as_str().map(str::to_string),
        declared: check::declared_capabilities(&out.json["plugin"], pid),
    };
    let mut problems = Vec::new();
    if installed.manifest_family != hr::MANIFEST_FAMILY_NATIVE {
        problems.push(format!(
            "manifest_family {:?} is not {:?} (the package loaded under foreign semantics; is the checkout's root marketplace.json present?)",
            installed.manifest_family,
            hr::MANIFEST_FAMILY_NATIVE
        ));
    }
    if installed.package_sha256 != expected_digest {
        problems.push(format!(
            "package_sha256 {} differs from the listed digest {expected_digest}",
            installed.package_sha256
        ));
    }
    if !problems.is_empty() {
        // Leave nothing half-trusted behind.
        let _ = remove(inv, pid, false);
        return Err(fail(
            "plugins install",
            format!("{spec}: {}; removed again", problems.join("; ")),
        ));
    }
    Ok(installed)
}

/// `plugins install <path>` from a local package directory — the rollback
/// of a failed update, back onto the generation the ledger recorded.
fn install_path(inv: &Invoker, path: &str) -> Result<Installed> {
    let out = inv.run_json(&["plugins", "install", path, "--json"])?;
    if !out.outcome.ok() {
        return Err(host_failure(&out.outcome, "plugins install (rollback)"));
    }
    let rec = &out.json["installed"];
    Ok(Installed {
        package_sha256: rec["package_sha256"].as_str().unwrap_or("").to_string(),
        manifest_family: rec["manifest_family"].as_str().unwrap_or("").to_string(),
        source_path: rec["source"]["path"].as_str().unwrap_or("").to_string(),
        warning: out.json["warning"].as_str().map(str::to_string),
        declared: check::declared_capabilities(&out.json["plugin"], pid_of(rec)),
    })
}

fn pid_of(rec: &Value) -> &str {
    rec["id"].as_str().unwrap_or("")
}

/// `plugins remove <pid> [--delete-data]`. An already-absent plugin is fine.
pub fn remove(inv: &Invoker, pid: &str, delete_data: bool) -> Result<bool> {
    let mut argv = vec!["plugins", "remove", pid];
    if delete_data {
        argv.push("--delete-data");
    }
    argv.push("--json");
    let out = inv.run(&argv)?;
    if out.ok() {
        return Ok(true);
    }
    if error_code(&out).as_deref() == Some("unknown-plugin") {
        return Ok(false);
    }
    Err(host_failure(&out, "plugins remove"))
}

// ---------------------------------------------------------------------------
// approve + verify (R14)
// ---------------------------------------------------------------------------

/// The trust outcome of one approve/verify pass.
#[derive(Clone, Debug)]
pub struct Approval {
    /// Approved by this run (were not `trusted_enabled` before).
    pub approved: Vec<String>,
    /// Every non-skipped line verified `trusted_enabled` afterwards.
    pub trusted: Vec<String>,
    /// Left `review_needed` by design (`enabled_default: false`).
    pub skipped: Vec<String>,
}

/// Stable ids the catalog ships disabled (`enabled_default: false`) — never
/// approved by omm (see the module docs).
pub fn by_design_unapproved(source: &Source, pid: &str) -> BTreeSet<String> {
    source
        .catalog
        .assets
        .iter()
        .filter(|a| a.lifecycle.ships())
        .filter(|a| {
            matches!(
                a.kind,
                AssetKind::Hook | AssetKind::McpServer | AssetKind::Reminder
            )
        })
        .filter(|a| !a.enabled_default())
        .map(|a| check::stable_id(pid, a.kind.singular(), &a.id))
        .collect()
}

/// Approve every runtime capability line the host lists (minus `skip`) and
/// verify each is PRESENT and literally `trusted_enabled` afterwards; also
/// require every capability the package declares to have a line at all.
/// Under `--dry-run` nothing is approved; the lines that would be are
/// reported as such.
pub fn approve_and_verify(
    inv: &Invoker,
    pid: &str,
    skip: &BTreeSet<String>,
    session: &Session,
) -> Result<Approval> {
    let before = probe::plugins_inspect(inv, pid)?;
    let declared = check::declared_capabilities(&before.raw["plugin"], pid);
    let absent: Vec<&str> = declared
        .iter()
        .filter(|d| before.capability(&d.stable_id).is_none())
        .map(|d| d.stable_id.as_str())
        .collect();
    if !absent.is_empty() {
        return Err(fail(
            "verify plugin capabilities",
            format!(
                "declared by the package but without a runtime-capability line in `plugins inspect`: {} (nothing can spawn from a missing line; `omm install --reinstall`)",
                absent.join(", ")
            ),
        ));
    }
    let mut approved = Vec::new();
    let mut skipped = Vec::new();
    for cap in &before.runtime_capabilities {
        if skip.contains(&cap.stable_id) {
            skipped.push(cap.stable_id.clone());
            continue;
        }
        if cap.is_trusted_enabled() {
            continue;
        }
        if !session.dry_run {
            probe::plugins_approve(inv, &cap.stable_id)?;
            session.record(&Event::new(ACTION_APPROVE).note(format!(
                "muse plugins approve {} ({} -> {})",
                cap.stable_id,
                cap.status,
                hr::RUNTIME_CAPABILITY_ACTIVE_STATE
            )))?;
        }
        approved.push(cap.stable_id.clone());
    }
    let after = if session.dry_run {
        before
    } else {
        probe::plugins_inspect(inv, pid)?
    };
    let mut trusted = Vec::new();
    let mut failed = Vec::new();
    for cap in &after.runtime_capabilities {
        if skip.contains(&cap.stable_id) {
            continue;
        }
        let would = session.dry_run && approved.contains(&cap.stable_id);
        if cap.is_trusted_enabled() || would {
            trusted.push(cap.stable_id.clone());
        } else {
            failed.push(format!(
                "{} {}{}",
                cap.stable_id,
                cap.status,
                cap.diagnostic_code
                    .as_deref()
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default()
            ));
        }
    }
    if !failed.is_empty() {
        return Err(fail(
            "verify plugin capabilities",
            format!(
                "not {} after approval: {}",
                hr::RUNTIME_CAPABILITY_ACTIVE_STATE,
                failed.join("; ")
            ),
        ));
    }
    Ok(Approval {
        approved,
        trusted,
        skipped,
    })
}

/// `skills list --json` must show every skill the source ships as
/// `plugin:<pid>:<id>` (R13 checkpoint 3). Returns the ids it does not.
pub fn skills_cross_check(
    inv: &Invoker,
    pid: &str,
    source: &Source,
) -> Result<(usize, Vec<String>)> {
    let list = probe::skills_list(
        inv,
        &SkillsListOptions {
            source: Some("plugin".to_string()),
            ..SkillsListOptions::default()
        },
    )?;
    let prefix = format!("plugin:{pid}:");
    let listed: BTreeSet<&str> = list
        .skills
        .iter()
        .map(|s| s.id.as_str())
        .filter(|id| id.starts_with(&prefix))
        .collect();
    let missing: Vec<String> = source
        .content
        .skills
        .iter()
        .map(|s| s.asset.id.clone())
        .filter(|id| !listed.contains(format!("{prefix}{id}").as_str()))
        .collect();
    Ok((listed.len(), missing))
}

/// The §6.4 pre-flight: install from `marketplace_root` in a throwaway
/// sandbox and require `native` + `package_sha256 == expected` before
/// anything is removed from the user's host.
pub fn preflight(inv: &Invoker, marketplace_root: &Path, pid: &str, expected: &str) -> Result<()> {
    let check = marketplace::verify_install(inv, marketplace_root, pid)?;
    let mut failed = check.passes();
    if check.listed_digest.as_deref() != Some(expected) {
        failed.push(format!(
            "pre-flight listed digest {:?} != {expected}",
            check.listed_digest
        ));
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(fail(
            "pre-flight install",
            format!(
                "{} — nothing was removed; fix the source and rerun `omm update`",
                failed.join("; ")
            ),
        ))
    }
}

// ---------------------------------------------------------------------------
// the transactions
// ---------------------------------------------------------------------------

/// What one bundle transaction did.
#[derive(Clone, Debug)]
pub struct BundleReport {
    pub marketplace: Option<MarketplaceState>,
    pub plugin: Action,
    pub digest: String,
    pub generation_path: String,
    pub approved: Vec<String>,
    pub trusted: Vec<String>,
    pub skipped: Vec<String>,
    pub skills_listed: usize,
    pub skills_missing: Vec<String>,
    pub warning: Option<String>,
    pub rolled_back: Option<String>,
    pub notes: Vec<String>,
}

impl BundleReport {
    pub fn to_json(&self) -> Value {
        json!({
            "marketplace": self.marketplace.as_ref().map(|m| json!({
                "name": m.name, "kind": m.kind, "source": m.source, "action": m.action.key(),
            })),
            "plugin": self.plugin.key(),
            "digest": self.digest,
            "generation_path": self.generation_path,
            "approved": self.approved,
            "trusted": self.trusted,
            "unapproved_by_design": self.skipped,
            "skills_listed": self.skills_listed,
            "skills_missing": self.skills_missing,
            "warning": self.warning,
            "rolled_back": self.rolled_back,
            "notes": self.notes,
        })
    }

    /// The terminal lines.
    pub fn lines(&self, pid: &str, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(m) = &self.marketplace {
            out.push(format!(
                "marketplace {}: {} ({} {})",
                m.name,
                match m.action {
                    Action::Updated => "added",
                    _ => "already configured, snapshot refreshed",
                },
                m.kind,
                m.source
            ));
        }
        out.push(format!(
            "plugin {pid}@{name}: {} {} ({})",
            match self.plugin {
                Action::Updated => "installed",
                Action::Unchanged => "unchanged",
                Action::Skipped => "skipped",
                Action::BackedUp => "backed up",
                Action::Removed => "removed",
            },
            short(&self.digest),
            self.generation_path
        ));
        out.push(format!(
            "capabilities: {} {} ({} approved now){}",
            self.trusted.len(),
            hr::RUNTIME_CAPABILITY_ACTIVE_STATE,
            self.approved.len(),
            if self.skipped.is_empty() {
                String::new()
            } else {
                format!(
                    "; {} left review_needed by design (enabledDefault false): {} — `muse plugins approve <id>` enables one",
                    self.skipped.len(),
                    self.skipped.join(", ")
                )
            }
        ));
        out.push(format!(
            "skills: {} listed by the host as plugin:{pid}:*{}",
            self.skills_listed,
            if self.skills_missing.is_empty() {
                String::new()
            } else {
                format!("; MISSING: {}", self.skills_missing.join(", "))
            }
        ));
        if let Some(r) = &self.rolled_back {
            out.push(format!("rolled back: {r}"));
        }
        out.extend(self.notes.iter().cloned());
        out
    }
}

/// `sha256:abcd…` → `abcd…` (12 hex).
pub fn short(digest: &str) -> String {
    let t = digest.trim_start_matches(hr::MARKETPLACE_DIGEST_PREFIX);
    let end = t.len().min(12);
    format!("{}…", &t[..end])
}

/// The digest the checkout's own `marketplace.json` carries — what a dry
/// run compares against without touching the host.
fn committed_digest(source: &Source) -> Result<String> {
    let path = source.repo.marketplace_native_path();
    let bytes = omm_host::fsx::read_bytes(&path)?;
    let v: Value = serde_json::from_slice(&bytes)
        .map_err(|e| fail("read marketplace.json", format!("{}: {e}", path.display())))?;
    let pid = source.catalog.plugin_id.as_str();
    v["plugins"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["name"].as_str() == Some(pid)))
        .and_then(|r| r["integrity"]["digest"].as_str())
        .map(str::to_string)
        .ok_or_else(|| {
            fail(
                "read marketplace.json",
                format!("{} has no `{pid}` entry with a digest", path.display()),
            )
        })
}

/// Replace (or add) the `muse-plugin` registration of `pid`.
fn set_plugin_registration(ledger: &mut Ledger, reg: Registration) -> bool {
    let id = match &reg {
        Registration::MusePlugin { id, .. } => id.clone(),
        _ => return ledger.register(reg),
    };
    if ledger.registrations.contains(&reg) {
        return false;
    }
    ledger
        .registrations
        .retain(|r| !matches!(r, Registration::MusePlugin { id: i, .. } if *i == id));
    ledger.registrations.push(reg);
    true
}

/// Replace (or add) the `muse-marketplace` registration of `name`.
fn set_marketplace_registration(ledger: &mut Ledger, name: &str, source: &str) -> bool {
    let reg = Registration::MuseMarketplace {
        name: name.to_string(),
        source: source.to_string(),
    };
    if ledger.registrations.contains(&reg) {
        return false;
    }
    ledger
        .registrations
        .retain(|r| !matches!(r, Registration::MuseMarketplace { name: n, .. } if n == name));
    // Keep the marketplace registration ahead of the plugin's so the undo
    // (reverse order) removes the plugin before the marketplace.
    let at = ledger
        .registrations
        .iter()
        .position(|r| matches!(r, Registration::MusePlugin { .. }))
        .unwrap_or(ledger.registrations.len());
    ledger.registrations.insert(at, reg);
    true
}

/// The ledger's registrations for this plugin.
pub fn registrations(ledger: &Ledger, pid: &str) -> (Option<Registration>, Option<Registration>) {
    let plugin = ledger
        .registrations
        .iter()
        .find(|r| matches!(r, Registration::MusePlugin { id, .. } if id == pid))
        .cloned();
    let mkt = ledger
        .registrations
        .iter()
        .find(|r| matches!(r, Registration::MuseMarketplace { .. }))
        .cloned();
    (plugin, mkt)
}

fn record_registrations(
    session: &mut Session,
    inv: &Invoker,
    pid: &str,
    mkt: Option<&MarketplaceState>,
    digest: &str,
    generation_path: &str,
    approved: &[String],
) -> Result<()> {
    let ledger = session.ledger_or_new(inv)?;
    if let Some(m) = mkt {
        set_marketplace_registration(ledger, &m.name, &m.source);
    }
    set_plugin_registration(
        ledger,
        Registration::MusePlugin {
            id: pid.to_string(),
            package_sha256: digest.to_string(),
            generation_path: generation_path.to_string(),
            approved: approved.to_vec(),
        },
    );
    Ok(())
}

/// `omm install`'s bundle step (see the module docs). `settings_existed_before`
/// says whether `settings.json` was there when the command started: the
/// host creates it during the approvals when it is not, on omm's behalf, so
/// it is ledgered as omm's (class `seeded`) the moment the package is
/// installed — before an approval can run — and uninstall unlinks it once
/// every key is restored (Gate 1: recorded only by the profile step, an
/// interrupt in between left `{"schema_version":1}` behind).
pub fn converge_install(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    reinstall: bool,
    settings_existed_before: bool,
    converge: &mut Converge,
) -> Result<BundleReport> {
    let pid = source.catalog.plugin_id.as_str();
    let name = omm_manifest::MARKETPLACE_NAME;
    source.require_built(pid)?;
    let skip = by_design_unapproved(source, pid);

    if session.dry_run {
        let digest = committed_digest(source)?;
        let inst = installed(inv, pid)?;
        let plugin = match &inst {
            Some(p) if p.package_sha256.as_deref() == Some(digest.as_str()) && !reinstall => {
                Action::Unchanged
            }
            _ => Action::Updated,
        };
        let mkt = configured_marketplace(inv, name)?;
        converge.record(
            CAT_MARKETPLACE,
            if mkt.is_some() {
                Action::Unchanged
            } else {
                Action::Updated
            },
        );
        converge.record(CAT_PLUGIN, plugin);
        let mut report = BundleReport {
            marketplace: mkt,
            plugin,
            digest,
            generation_path: inst
                .as_ref()
                .and_then(|p| p.source_path.as_ref())
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| source.repo.native_package_dir(pid).display().to_string()),
            approved: Vec::new(),
            trusted: Vec::new(),
            skipped: skip.iter().cloned().collect(),
            skills_listed: 0,
            skills_missing: Vec::new(),
            warning: None,
            rolled_back: None,
            notes: vec![format!(
                "dry run: would {} from {}, then approve every runtime capability and verify {}",
                if plugin == Action::Updated {
                    "install the bundle"
                } else {
                    "keep the installed bundle"
                },
                source.marketplace_root().display(),
                hr::RUNTIME_CAPABILITY_ACTIVE_STATE
            )],
        };
        if inst.is_some() {
            // Read-only under a dry run: `inspect` and `skills list` only.
            if let Ok(a) = approve_and_verify(inv, pid, &skip, session) {
                converge.record_n(CAT_CAPABILITIES, Action::Updated, a.approved.len());
                converge.record_n(
                    CAT_CAPABILITIES,
                    Action::Unchanged,
                    a.trusted.len().saturating_sub(a.approved.len()),
                );
                report.approved = a.approved;
                report.trusted = a.trusted;
            }
            if let Ok((listed, missing)) = skills_cross_check(inv, pid, source) {
                report.skills_listed = listed;
                report.skills_missing = missing;
            }
        } else {
            report.notes.push(
                "not installed: capabilities and skills are checked after the install".into(),
            );
        }
        converge.record_n(CAT_CAPABILITIES, Action::Skipped, skip.len());
        return Ok(report);
    }

    // The host's version (the ledger's `host` record) is asked for before
    // the first host mutation, so the first save follows `marketplace add`
    // at once (Gate 1: `muse --version` ran between the add and the save, a
    // 0.3 s hole with the marketplace registered and no ledger).
    session.ledger_or_new(inv)?;
    let mkt = ensure_marketplace(inv, name, source.marketplace_root())?;
    converge.record(CAT_MARKETPLACE, mkt.action);
    // The host now holds a registration: the ledger records it before
    // anything else can fail (Gate 1: an install interrupted after
    // `marketplace add` or `plugins install` left both on the host with no
    // ledger, and `omm uninstall` saw nothing to undo).
    set_marketplace_registration(session.ledger_or_new(inv)?, &mkt.name, &mkt.source);
    session.save()?;
    let avail = available(inv, name, pid)?;
    let inst = installed(inv, pid)?;
    let (plugin, generation_path, warning) = match inst {
        Some(p)
            if p.package_sha256.as_deref() == Some(avail.digest.as_str())
                && p.manifest_family.as_deref() == Some(hr::MANIFEST_FAMILY_NATIVE)
                && !reinstall =>
        {
            (
                Action::Unchanged,
                p.source_path
                    .map(|s| s.display().to_string())
                    .unwrap_or_default(),
                None,
            )
        }
        other => {
            if other.is_some() {
                remove(inv, pid, false)?;
                session.record(&Event::new(ACTION_INSTALL).note(format!(
                    "muse plugins remove {pid} (reinstall: {})",
                    if reinstall {
                        "--reinstall"
                    } else {
                        "digest or family differs"
                    }
                )))?;
            }
            let i = install(inv, pid, name, &avail.digest)?;
            // Registered (approved: nothing yet) and saved before the audit
            // line, so the line's presence implies the ledger's.
            record_registrations(
                session,
                inv,
                pid,
                Some(&mkt),
                &i.package_sha256,
                &i.source_path,
                &[],
            )?;
            if !settings_existed_before {
                files::record_seeded_settings(ctx, inv, session, &source.version)?;
            }
            session.save()?;
            session.record(
                &Event::new(ACTION_INSTALL)
                    .after(Some(&i.package_sha256))
                    .note(format!(
                        "muse plugins install {pid}@{name} ({} capabilities declared) from {}",
                        i.declared.len(),
                        i.source_path
                    )),
            )?;
            (Action::Updated, i.source_path, i.warning)
        }
    };
    converge.record(CAT_PLUGIN, plugin);
    let approval = approve_and_verify(inv, pid, &skip, session)?;
    converge.record_n(CAT_CAPABILITIES, Action::Updated, approval.approved.len());
    converge.record_n(
        CAT_CAPABILITIES,
        Action::Unchanged,
        approval
            .trusted
            .len()
            .saturating_sub(approval.approved.len()),
    );
    converge.record_n(CAT_CAPABILITIES, Action::Skipped, approval.skipped.len());
    let (skills_listed, skills_missing) = skills_cross_check(inv, pid, source)?;
    record_registrations(
        session,
        inv,
        pid,
        Some(&mkt),
        &avail.digest,
        &generation_path,
        &approval.trusted,
    )?;
    session.save()?;
    if !skills_missing.is_empty() {
        ctx.out.warn(format!(
            "skills list --json does not show {} of {} shipped skills: {}",
            skills_missing.len(),
            source.content.skills.len(),
            skills_missing.join(", ")
        ));
    }
    Ok(BundleReport {
        marketplace: Some(mkt),
        plugin,
        digest: avail.digest,
        generation_path,
        approved: approval.approved,
        trusted: approval.trusted,
        skipped: approval.skipped,
        skills_listed,
        skills_missing,
        warning,
        rolled_back: None,
        notes: Vec::new(),
    })
}

/// `omm update`'s bundle step: the §6.4 update transaction.
pub fn converge_update(
    ctx: &Ctx,
    inv: &Invoker,
    session: &mut Session,
    source: &Source,
    reinstall: bool,
    converge: &mut Converge,
) -> Result<BundleReport> {
    let pid = source.catalog.plugin_id.as_str();
    let (plugin_reg, mkt_reg) = match &session.ledger {
        Some(l) => registrations(l, pid),
        None => (None, None),
    };
    let Some(Registration::MusePlugin {
        package_sha256: old_sha,
        generation_path: old_path,
        ..
    }) = plugin_reg
    else {
        return Err(OmmError::Usage(format!(
            "the ledger has no muse-plugin registration for `{pid}`; run `omm install` (or `omm install --no-plugin` installs are updated as skills)"
        )));
    };
    let name = match &mkt_reg {
        Some(Registration::MuseMarketplace { name, .. }) => name.clone(),
        _ => omm_manifest::MARKETPLACE_NAME.to_string(),
    };
    let skip = by_design_unapproved(source, pid);

    if session.dry_run {
        let inst = installed(inv, pid)?;
        let avail = available(inv, &name, pid).ok();
        let digest = avail
            .as_ref()
            .map(|a| a.digest.clone())
            .unwrap_or_else(|| old_sha.clone());
        let plugin = match &inst {
            Some(p) if p.package_sha256.as_deref() == Some(digest.as_str()) && !reinstall => {
                Action::Unchanged
            }
            _ => Action::Updated,
        };
        converge.record(CAT_PLUGIN, plugin);
        let (approved, trusted) = if inst.is_some() {
            match approve_and_verify(inv, pid, &skip, session) {
                Ok(a) => (a.approved, a.trusted),
                Err(_) => (Vec::new(), Vec::new()),
            }
        } else {
            (Vec::new(), Vec::new())
        };
        converge.record_n(CAT_CAPABILITIES, Action::Updated, approved.len());
        converge.record_n(
            CAT_CAPABILITIES,
            Action::Unchanged,
            trusted.len().saturating_sub(approved.len()),
        );
        converge.record_n(CAT_CAPABILITIES, Action::Skipped, skip.len());
        let (skills_listed, skills_missing) = if inst.is_some() {
            skills_cross_check(inv, pid, source).unwrap_or((0, Vec::new()))
        } else {
            (0, Vec::new())
        };
        return Ok(BundleReport {
            marketplace: None,
            plugin,
            digest,
            generation_path: old_path,
            approved,
            trusted,
            skipped: skip.into_iter().collect(),
            skills_listed,
            skills_missing,
            warning: None,
            rolled_back: None,
            notes: vec![format!(
                "dry run: `marketplace update {name}` was not run; the snapshot's digest is compared as it stands (installed {})",
                short(&old_sha)
            )],
        });
    }

    let mut notes = Vec::new();
    // 1. Refresh the marketplace snapshot; a missing marketplace is re-added
    //    (§6.5: the plugin kept running from cache meanwhile).
    let upd = inv.run_json(&["plugins", "marketplace", "update", &name, "--json"])?;
    let mkt = if upd.outcome.ok() {
        parse_marketplace(&name, &upd.json["marketplace"], Action::Unchanged)?
    } else if is_unknown_marketplace(&upd.outcome) {
        ensure_marketplace(inv, &name, source.marketplace_root())?
    } else {
        return Err(host_failure(&upd.outcome, "plugins marketplace update"));
    };
    // 1b. `--source` names another checkout than the one registered at
    //     install: the marketplace is re-registered from it, so the digest
    //     compare below sees the new package (Gate 1: the plugin stayed at
    //     the old digest while rules, themes and skills came from the new
    //     checkout — a mixed-version install with no warning). `marketplace
    //     remove` leaves the installed plugin running from its cache
    //     (measured 2026-09-02); the ledger follows each host step.
    let mkt = if mkt.kind == "local-dir" && !same_dir(&mkt.source, source.marketplace_root()) {
        let old_source = mkt.source.clone();
        inv.run(&["plugins", "marketplace", "remove", &name, "--json"])?
            .expect_ok()?;
        if let Some(l) = session.ledger.as_mut() {
            l.registrations.retain(
                |r| !matches!(r, Registration::MuseMarketplace { name: n, .. } if *n == name),
            );
        }
        session.save()?;
        session.record(&Event::new(ACTION_UPDATE).note(format!(
            "muse plugins marketplace remove {name} (registered from {old_source}; --source is {})",
            source.marketplace_root().display()
        )))?;
        let added = ensure_marketplace(inv, &name, source.marketplace_root())?;
        set_marketplace_registration(session.ledger_or_new(inv)?, &added.name, &added.source);
        session.save()?;
        session.record(&Event::new(ACTION_UPDATE).note(format!(
            "muse plugins marketplace add {name} {}",
            added.source
        )))?;
        notes.push(format!(
            "marketplace {name}: re-registered from {} (was {old_source})",
            added.source
        ));
        MarketplaceState {
            action: Action::Updated,
            ..added
        }
    } else {
        mkt
    };
    converge.record(CAT_MARKETPLACE, mkt.action);
    // 2. Digest compare.
    let avail = available(inv, &name, pid)?;
    let inst = installed(inv, pid)?;
    let (plugin, generation_path, warning) = match inst {
        Some(p)
            if p.package_sha256.as_deref() == Some(avail.digest.as_str())
                && p.manifest_family.as_deref() == Some(hr::MANIFEST_FAMILY_NATIVE)
                && !reinstall =>
        {
            notes.push(format!(
                "digest {} unchanged; the installed bundle stays (never `plugins update`)",
                short(&avail.digest)
            ));
            (
                Action::Unchanged,
                p.source_path
                    .map(|s| s.display().to_string())
                    .unwrap_or(old_path.clone()),
                None,
            )
        }
        inst => {
            // 3. Pre-flight before anything is removed.
            let root = preflight_root(ctx, &mkt);
            preflight(inv, &root, pid, &avail.digest)?;
            // 4. remove → install → approve → verify, with a rollback onto
            //    the previous generation if the install fails.
            if inst.is_some() {
                remove(inv, pid, false)?;
                session.record(
                    &Event::new(ACTION_UPDATE)
                        .before(Some(&old_sha))
                        .note(format!(
                            "muse plugins remove {pid} (update {} -> {})",
                            short(&old_sha),
                            short(&avail.digest)
                        )),
                )?;
            }
            match install(inv, pid, &name, &avail.digest) {
                Ok(i) => {
                    // The new generation is on the host: ledger first, then
                    // the audit line, then the approvals.
                    record_registrations(
                        session,
                        inv,
                        pid,
                        Some(&mkt),
                        &i.package_sha256,
                        &i.source_path,
                        &[],
                    )?;
                    session.save()?;
                    session.record(
                        &Event::new(ACTION_UPDATE)
                            .before(Some(&old_sha))
                            .after(Some(&i.package_sha256))
                            .note(format!(
                                "muse plugins install {pid}@{name} from {}",
                                i.source_path
                            )),
                    )?;
                    (Action::Updated, i.source_path, i.warning)
                }
                Err(e) => {
                    let back = if Path::new(&old_path).is_dir() {
                        match install_path(inv, &old_path) {
                            Ok(i) => {
                                let _ = approve_and_verify(inv, pid, &skip, session);
                                format!("reinstalled {} from {old_path}", short(&i.package_sha256))
                            }
                            Err(r) => format!("rollback from {old_path} failed too: {r}"),
                        }
                    } else {
                        format!("previous generation {old_path} is gone; nothing is installed now — run `omm install`")
                    };
                    session.record(
                        &Event::new(ACTION_UPDATE).note(format!("FAILED install: {e}; {back}")),
                    )?;
                    return Err(fail("omm update", format!("{e}; {back}")));
                }
            }
        }
    };
    converge.record(CAT_PLUGIN, plugin);
    let approval = approve_and_verify(inv, pid, &skip, session)?;
    converge.record_n(CAT_CAPABILITIES, Action::Updated, approval.approved.len());
    converge.record_n(
        CAT_CAPABILITIES,
        Action::Unchanged,
        approval
            .trusted
            .len()
            .saturating_sub(approval.approved.len()),
    );
    converge.record_n(CAT_CAPABILITIES, Action::Skipped, approval.skipped.len());
    let (skills_listed, skills_missing) = skills_cross_check(inv, pid, source)?;
    record_registrations(
        session,
        inv,
        pid,
        Some(&mkt),
        &avail.digest,
        &generation_path,
        &approval.trusted,
    )?;
    session.save()?;
    Ok(BundleReport {
        marketplace: Some(mkt),
        plugin,
        digest: avail.digest,
        generation_path,
        approved: approval.approved,
        trusted: approval.trusted,
        skipped: approval.skipped,
        skills_listed,
        skills_missing,
        warning,
        rolled_back: None,
        notes,
    })
}

/// The directory the pre-flight adds as a marketplace: a local-dir source's
/// path, or a git source's current worktree under the data root.
fn preflight_root(ctx: &Ctx, mkt: &MarketplaceState) -> PathBuf {
    match (&mkt.kind[..], &mkt.worktree_path) {
        ("git", Some(wt)) => ctx.roots.data_home.join(wt),
        _ => PathBuf::from(&mkt.source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registrations_replace_by_identity_and_keep_marketplace_first() {
        let mut l = Ledger::new(
            "0.1.0",
            omm_ledger::HostInfo {
                version: "v".into(),
                sha256: "s".into(),
            },
            omm_ledger::Scope::User,
        );
        assert!(set_plugin_registration(
            &mut l,
            Registration::MusePlugin {
                id: "omm".into(),
                package_sha256: "a".into(),
                generation_path: "/g1".into(),
                approved: vec![],
            }
        ));
        assert!(set_marketplace_registration(&mut l, "ohmy", "/src"));
        assert!(!set_marketplace_registration(&mut l, "ohmy", "/src"));
        assert!(matches!(
            l.registrations[0],
            Registration::MuseMarketplace { .. }
        ));
        assert!(set_plugin_registration(
            &mut l,
            Registration::MusePlugin {
                id: "omm".into(),
                package_sha256: "b".into(),
                generation_path: "/g2".into(),
                approved: vec!["plugin:omm:hook:x".into()],
            }
        ));
        assert_eq!(l.registrations.len(), 2);
        let (p, m) = registrations(&l, "omm");
        assert!(
            matches!(p, Some(Registration::MusePlugin { package_sha256, .. }) if package_sha256 == "b")
        );
        assert!(m.is_some());
        assert!(set_marketplace_registration(&mut l, "ohmy", "/other"));
        assert_eq!(l.registrations.len(), 2);
        assert!(l.validate().is_ok());
        assert_eq!(short("sha256:abcdef0123456789ff"), "abcdef012345…");
    }

    #[test]
    fn a_marketplace_with_no_usable_plugin_is_refused() {
        let m = json!({"source": {"kind": "local-dir", "path": "/x"}, "plugin_count": 0, "skipped": []});
        assert!(parse_marketplace("ohmy", &m, Action::Updated).is_err());
        let m = json!({"source": {"kind": "local-dir", "path": "/x"}, "plugin_count": 1, "skipped": [{"name": "omm"}]});
        assert!(parse_marketplace("ohmy", &m, Action::Updated).is_err());
        let m = json!({"source": {"kind": "git", "path": "file:///r", "worktree_path": "plugins/marketplaces/ohmy/source"}, "plugin_count": 1, "skipped": []});
        let s = parse_marketplace("ohmy", &m, Action::Unchanged).unwrap();
        assert_eq!(
            s.worktree_path.as_deref(),
            Some("plugins/marketplaces/ohmy/source")
        );
        assert_eq!(s.kind, "git");
    }
}
