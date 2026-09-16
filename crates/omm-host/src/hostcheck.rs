//! hostcheck — re-measure `docs/host-reality.md` against the live binary.
//!
//! The P0 block (schema fingerprints, MSP counts, plugin `schemaVersion`
//! accept/reject, the five capability families + `tools` reject, settings
//! `schema_version` 0/2 reject, the eight context-block orders of one echo
//! session, export/frame schema versions) and the P1 counts (the gates of
//! gates.json / 14 on, 17 hook events, 15 bundled skills, 26 active tools,
//! 16 commands, the `--reasoning-effort` value list, the exit-code matrix;
//! slash commands against the data file because the picker is TUI-only).
//! Runs in a throwaway HOME/XDG sandbox in a few seconds.
//! `omm doctor --self-test` and `tests/hostcheck.rs` both call [`run`].
//! Every check is recorded, never aborts the run, and the report renders as a
//! diff table so drift is readable at a glance (R15: verify by behaviour).
//!
//! Facts tagged `since: <build>` in the data files (gates.json `since_rule`,
//! muse-cli.json `values_since`) are expected wherever probing finds them and
//! reported as [`Status::OlderBuild`] — a pass with a note — where probing
//! finds them absent AND every other fact of the same `since` group is absent
//! too, so the build as a whole behaves like one that predates the tag. A
//! mixed group, a missing untagged fact or an unlisted gate stays a failure.
//! The version string appears in the note only; it never decides.

use std::collections::BTreeSet;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde_json::{json, Value};

use crate::error::{HostError, Result};
use crate::fixtures::{self, PackageSpec};
use crate::fsx;
use crate::host_reality as hr;
use crate::invoke::{Invoker, OutcomeKind, Sandbox};
use crate::probe::{self, EchoSessionOptions, SchemaKind, SkillsListOptions};

/// Which block of `host-reality.md` a check belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    /// Run on every channel poll (< 5 s).
    P0,
    /// Run per release.
    P1,
}

impl Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Tier::P0 => "P0",
            Tier::P1 => "P1",
        })
    }
}

/// Outcome of one check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
    /// Could not be measured offline; asserted against the data file instead.
    DataOnly,
    /// A `since`-tagged fact the build does not show, on a build whose whole
    /// since group is absent: a pass whose note quotes the version string.
    OlderBuild,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Status::Pass => "PASS",
            Status::Fail => "FAIL",
            Status::DataOnly => "DATA",
            Status::OlderBuild => "OLDER-BUILD",
        })
    }
}

/// One row of the report.
#[derive(Clone, Debug)]
pub struct Check {
    pub id: String,
    pub tier: Tier,
    pub expected: String,
    pub observed: String,
    pub status: Status,
    pub detail: String,
}

/// The whole run.
#[derive(Clone, Debug)]
pub struct Report {
    pub binary: PathBuf,
    pub version: Option<String>,
    pub checks: Vec<Check>,
    pub elapsed_ms: u128,
}

impl Report {
    /// No failures.
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Fail)
    }
    /// The failed rows.
    pub fn failures(&self) -> Vec<&Check> {
        self.checks
            .iter()
            .filter(|c| c.status == Status::Fail)
            .collect()
    }
    /// The OLDER-BUILD rows: `since`-tagged facts this build predates.
    pub fn older_build(&self) -> Vec<&Check> {
        self.checks
            .iter()
            .filter(|c| c.status == Status::OlderBuild)
            .collect()
    }
    /// A diff table.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let older = self.older_build().len();
        out.push_str(&format!(
            "hostcheck: {} ({}) — {} checks, {} failed{}, {} ms\n",
            self.binary.display(),
            self.version.as_deref().unwrap_or("version unknown"),
            self.checks.len(),
            self.failures().len(),
            if older > 0 {
                format!(", {older} OLDER-BUILD")
            } else {
                String::new()
            },
            self.elapsed_ms
        ));
        let w_id = self
            .checks
            .iter()
            .map(|c| c.id.len())
            .max()
            .unwrap_or(8)
            .max(8);
        let w_exp = self
            .checks
            .iter()
            .map(|c| c.expected.len())
            .max()
            .unwrap_or(8)
            .clamp(8, 40);
        let w_st = if older > 0 { 11 } else { 6 };
        out.push_str(&format!(
            "{:<w_st$} {:<4} {:<w_id$}  {:<w_exp$}  OBSERVED\n",
            "STATUS", "TIER", "CHECK", "EXPECTED"
        ));
        for c in &self.checks {
            out.push_str(&format!(
                "{:<w_st$} {:<4} {:<w_id$}  {:<w_exp$}  {}\n",
                c.status.to_string(),
                c.tier.to_string(),
                c.id,
                clip(&c.expected, w_exp),
                c.observed
            ));
            if matches!(c.status, Status::Fail | Status::OlderBuild) && !c.detail.is_empty() {
                out.push_str(&format!("{:w_st$} {}\n", "", c.detail));
            }
        }
        out
    }
    /// JSON for `--json`.
    pub fn to_json(&self) -> Value {
        json!({
            "binary": self.binary,
            "version": self.version,
            "ok": self.ok(),
            "elapsed_ms": self.elapsed_ms,
            "checks": self.checks.iter().map(|c| json!({
                "id": c.id, "tier": c.tier.to_string(), "status": c.status.to_string(),
                "expected": c.expected, "observed": c.observed, "detail": c.detail,
            })).collect::<Vec<_>>(),
        })
    }
}

fn clip(s: &str, w: usize) -> String {
    if s.chars().count() <= w {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(w.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// Run the whole check set against `bin` in a fresh temp sandbox.
pub fn run(bin: &Path) -> Result<Report> {
    let tmp = tempfile::Builder::new()
        .prefix("omm-hostcheck-")
        .tempdir()
        .map_err(|e| HostError::io("create temp dir", std::env::temp_dir(), e))?;
    run_in(&Invoker::new(bin), tmp.path())
}

/// Run against an invoker, sandboxing it under `root`.
pub fn run_in(inv: &Invoker, root: &Path) -> Result<Report> {
    let started = Instant::now();
    let sb = Sandbox::create(root)?;
    let cfg = sb.config_home.join("muse");
    fsx::create_dir_all(&cfg)?;
    write(&cfg.join("settings.json"), b"{\"schema_version\":1}\n")?;
    let ws = root.join("ws");
    fsx::create_dir_all(&ws)?;
    let inv = inv.clone().sandboxed(&sb);
    let mut c = Checker {
        inv,
        sb,
        ws,
        root: root.to_path_buf(),
        checks: Vec::new(),
        version: None,
        since_facts: Vec::new(),
    };
    c.guard("data-files", Tier::P0, Checker::check_data_files);
    c.guard("version", Tier::P0, Checker::check_version);
    c.guard("msp-schema", Tier::P0, Checker::check_msp_schema);
    c.guard("enterprise-generation", Tier::P0, Checker::check_enterprise);
    c.guard(
        "plugin-schema-version",
        Tier::P0,
        Checker::check_plugin_schema_version,
    );
    c.guard(
        "capability-families",
        Tier::P0,
        Checker::check_capability_families,
    );
    c.guard(
        "settings-schema-version",
        Tier::P0,
        Checker::check_settings_schema_version,
    );
    c.guard("echo-session", Tier::P0, Checker::check_echo_session);
    // The `since` groups are resolved after every probe that can see a tagged
    // fact has run (`cli-values`, `gates`); `since` then emits their rows.
    c.guard("cli-values", Tier::P1, Checker::check_cli_values);
    c.guard("gates", Tier::P1, Checker::check_gates);
    c.guard("since", Tier::P1, Checker::check_since_groups);
    c.guard("hook-events", Tier::P1, Checker::check_hook_events);
    c.guard("bundled-skills", Tier::P1, Checker::check_bundled_skills);
    c.guard("cli-commands", Tier::P1, Checker::check_cli_commands);
    c.guard("exit-codes", Tier::P1, Checker::check_exit_codes);
    c.guard("slash-commands", Tier::P1, Checker::check_slash_commands);
    Ok(Report {
        binary: c.inv.bin().to_path_buf(),
        version: c.version.clone(),
        checks: c.checks,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| HostError::io("write", path, e))
}

struct Checker {
    inv: Invoker,
    sb: Sandbox,
    ws: PathBuf,
    root: PathBuf,
    checks: Vec<Check>,
    version: Option<String>,
    /// Every `since`-tagged fact probed so far, resolved by
    /// [`Checker::check_since_groups`].
    since_facts: Vec<SinceFact>,
}

/// One `since`-tagged fact and whether this build shows it.
#[derive(Clone, Debug)]
struct SinceFact {
    /// The build the fact was first observed on (the group key).
    since: String,
    /// Row id, e.g. `gates/since/ultra_reasoning_effort`.
    id: String,
    /// What the data file says the fact is.
    expected: String,
    present: bool,
    /// What probing showed.
    observed: String,
}

impl Checker {
    /// Record a `since`-tagged fact for the final group resolution.
    fn since_fact(
        &mut self,
        since: &str,
        id: impl Into<String>,
        expected: impl Into<String>,
        present: bool,
        observed: impl Into<String>,
    ) {
        self.since_facts.push(SinceFact {
            since: since.to_string(),
            id: id.into(),
            expected: expected.into(),
            present,
            observed: observed.into(),
        });
    }

    /// Whether every fact of the `since` group is absent on this build — the
    /// behavioural definition of "the build predates `since`" (gates.json
    /// `since_rule`). An empty group is not "absent".
    fn since_group_absent(&self, since: &str) -> bool {
        let group: Vec<&SinceFact> = self
            .since_facts
            .iter()
            .filter(|f| f.since == since)
            .collect();
        !group.is_empty() && group.iter().all(|f| !f.present)
    }

    /// The `since`-tagged gate ids of `part.older_build` whose group is absent
    /// as a whole; the rest are drift.
    fn older_build_gates(
        &self,
        data: &hr::Gates,
        part: &hr::GatePartition,
    ) -> (Vec<String>, Vec<String>) {
        let mut older = Vec::new();
        let mut drift = Vec::new();
        for id in &part.older_build {
            let since = data
                .items
                .iter()
                .find(|g| &g.id == id)
                .and_then(|g| g.since.as_deref())
                .unwrap_or("");
            if self.since_group_absent(since) {
                older.push(id.clone());
            } else {
                drift.push(id.clone());
            }
        }
        (older, drift)
    }

    /// The build's version string for OLDER-BUILD notes — informational only (R15).
    fn version_note(&self) -> String {
        self.version
            .clone()
            .unwrap_or_else(|| "version unknown".to_string())
    }
    fn guard(&mut self, id: &str, tier: Tier, f: fn(&mut Checker) -> Result<()>) {
        let before = self.checks.len();
        if let Err(e) = f(self) {
            self.checks.push(Check {
                id: format!("{id}/error"),
                tier,
                expected: "probe succeeds".to_string(),
                observed: "probe failed".to_string(),
                status: Status::Fail,
                detail: e.to_string(),
            });
        }
        if self.checks.len() == before {
            self.checks.push(Check {
                id: format!("{id}/empty"),
                tier,
                expected: "at least one row".to_string(),
                observed: "none".to_string(),
                status: Status::Fail,
                detail: "check produced no rows".to_string(),
            });
        }
    }

    fn record<E: Display, O: Display>(
        &mut self,
        id: impl Into<String>,
        tier: Tier,
        expected: E,
        observed: O,
        pass: bool,
        detail: impl Into<String>,
    ) {
        self.checks.push(Check {
            id: id.into(),
            tier,
            expected: expected.to_string(),
            observed: observed.to_string(),
            status: if pass { Status::Pass } else { Status::Fail },
            detail: detail.into(),
        });
    }

    fn eq<T: PartialEq + Display>(&mut self, id: &str, tier: Tier, expected: T, observed: T) {
        let pass = expected == observed;
        self.record(id, tier, expected, observed, pass, "");
    }

    fn scratch(&self, name: &str) -> Result<PathBuf> {
        let dir = self.root.join("scratch").join(name);
        fsx::create_dir_all(&dir)?;
        Ok(dir)
    }

    // ---- P0 --------------------------------------------------------------

    fn check_data_files(&mut self) -> Result<()> {
        let problems = hr::verify_data()?;
        let pass = problems.is_empty();
        self.record(
            "data-files/consistent",
            Tier::P0,
            "7 data files parse, counts match constants",
            if pass {
                "consistent".to_string()
            } else {
                problems.join("; ")
            },
            pass,
            "",
        );
        Ok(())
    }

    fn check_version(&mut self) -> Result<()> {
        let v = probe::version(&self.inv)?;
        self.version = Some(v.raw.clone());
        self.record(
            "version/parses",
            Tier::P0,
            format!("{} <version> (<build>)", hr::VERSION_STRING_PRODUCT),
            &v.raw,
            v.product == hr::VERSION_STRING_PRODUCT,
            "informational — never gate on it (R15)",
        );
        Ok(())
    }

    fn check_msp_schema(&mut self) -> Result<()> {
        let stable = probe::schema_export(
            &self.inv,
            SchemaKind::JsonSchema,
            false,
            &self.scratch("schema-stable")?,
        )?;
        let fp = stable
            .manifest
            .as_ref()
            .map(|m| m.fingerprint.clone())
            .unwrap_or_default();
        self.eq(
            "msp/stable-fingerprint",
            Tier::P0,
            hr::MSP_STABLE_FINGERPRINT.to_string(),
            fp,
        );
        self.eq(
            "msp/envelope-schema-version",
            Tier::P0,
            hr::MSP_ENVELOPE_SCHEMA_VERSION,
            stable
                .manifest
                .as_ref()
                .map(|m| m.schema_version)
                .unwrap_or(0),
        );
        self.eq(
            "msp/schema-json-sha256",
            Tier::P0,
            hr::MSP_SCHEMA_JSON_SHA256.to_string(),
            stable
                .sha256_of("msp.schema.json")
                .unwrap_or("")
                .to_string(),
        );
        let counts = probe::schema_counts(&stable.out_dir.join("msp.schema.json"))?;
        self.eq("msp/methods", Tier::P0, hr::MSP_METHODS, counts.methods);
        self.eq(
            "msp/notifications",
            Tier::P0,
            hr::MSP_NOTIFICATIONS,
            counts.notifications,
        );
        self.eq(
            "msp/error-codes",
            Tier::P0,
            hr::MSP_ERROR_CODES,
            counts.errors,
        );

        let exp = probe::schema_export(
            &self.inv,
            SchemaKind::JsonSchema,
            true,
            &self.scratch("schema-exp")?,
        )?;
        self.eq(
            "msp/experimental-fingerprint",
            Tier::P0,
            hr::MSP_EXPERIMENTAL_FINGERPRINT.to_string(),
            exp.manifest.map(|m| m.fingerprint).unwrap_or_default(),
        );
        let ts = probe::schema_export(
            &self.inv,
            SchemaKind::TypeScript,
            false,
            &self.scratch("schema-ts")?,
        )?;
        self.eq(
            "msp/dts-sha256",
            Tier::P0,
            hr::MSP_DTS_SHA256.to_string(),
            ts.sha256_of("msp.d.ts").unwrap_or("").to_string(),
        );
        Ok(())
    }

    fn check_enterprise(&mut self) -> Result<()> {
        let status = probe::config_status(&self.inv)?;
        self.eq(
            "enterprise/generation",
            Tier::P0,
            hr::ENTERPRISE_GENERATION.to_string(),
            status.generation,
        );
        Ok(())
    }

    fn check_plugin_schema_version(&mut self) -> Result<()> {
        let dir = self.scratch("plugin-sv")?;
        let ok = PackageSpec::native("omm-sv-ok", fixtures::single_family_capabilities("skills")?);
        let root = fixtures::write_package(&dir, "sv1", &ok)?;
        let v = probe::plugins_validate(&self.inv, &root)?;
        let failed = v.four_predicates();
        self.record(
            "plugin/schemaVersion-1-accepted",
            Tier::P0,
            "valid, no diagnostics, native, all supported",
            if failed.is_empty() {
                "accepted".to_string()
            } else {
                failed.join("; ")
            },
            failed.is_empty(),
            "",
        );
        for bad in [json!(0), json!(2), json!(99), json!("1")] {
            let mut spec = ok.clone();
            spec.schema_version = bad.clone();
            let root = fixtures::write_package(&dir, &format!("sv-{bad}"), &spec)?;
            let v = probe::plugins_validate(&self.inv, &root)?;
            let code = v.error.as_ref().map(|e| e.0.clone()).unwrap_or_default();
            let pass =
                !v.valid && v.exit == OutcomeKind::RunFailed && code == "invalid-manifest-schema";
            self.record(
                format!("plugin/schemaVersion-{bad}-rejected"),
                Tier::P0,
                "exit 1, invalid-manifest-schema",
                format!("exit {:?}, valid={}, code={code}", v.exit, v.valid),
                pass,
                "",
            );
        }
        Ok(())
    }

    fn check_capability_families(&mut self) -> Result<()> {
        let dir = self.scratch("families")?;
        let five = PackageSpec::native("omm-five", fixtures::five_family_capabilities()?);
        let root = fixtures::write_package(&dir, "five", &five)?;
        let v = probe::plugins_validate(&self.inv, &root)?;
        let failed = v.four_predicates();
        self.record(
            "families/five-accepted",
            Tier::P0,
            "valid, no diagnostics, native, 5 supported",
            if failed.is_empty() {
                format!("accepted, summary={}", v.summary.as_deref().unwrap_or("?"))
            } else {
                failed.join("; ")
            },
            failed.is_empty(),
            "",
        );
        let expected: BTreeSet<&str> = hr::CAPABILITY_FAMILIES_VALIDATOR.into_iter().collect();
        let observed: BTreeSet<&str> = v.capabilities.iter().map(String::as_str).collect();
        self.eq(
            "families/exactly-five",
            Tier::P0,
            expected.iter().copied().collect::<Vec<_>>().join(","),
            observed.iter().copied().collect::<Vec<_>>().join(","),
        );
        self.eq(
            "families/declarations",
            Tier::P0,
            5usize,
            v.declarations.len(),
        );
        for family in hr::CAPABILITY_FAMILIES_MANIFEST {
            let spec = PackageSpec::native(
                &format!("omm-only-{}", family.to_ascii_lowercase()),
                fixtures::single_family_capabilities(family)?,
            );
            let root = fixtures::write_package(&dir, &format!("only-{family}"), &spec)?;
            let v = probe::plugins_validate(&self.inv, &root)?;
            let failed = v.four_predicates();
            self.record(
                format!("families/{family}-accepted"),
                Tier::P0,
                "accepted, summary full",
                if failed.is_empty() {
                    format!("accepted, summary={}", v.summary.as_deref().unwrap_or("?"))
                } else {
                    failed.join("; ")
                },
                failed.is_empty() && v.summary.as_deref() == Some("full"),
                "",
            );
        }
        let tools_only = PackageSpec::native("omm-tools", fixtures::tools_capability());
        let root = fixtures::write_package(&dir, "tools-only", &tools_only)?;
        let v = probe::plugins_validate(&self.inv, &root)?;
        let code = v.error.as_ref().map(|e| e.0.clone()).unwrap_or_default();
        self.record(
            "families/tools-only-rejected",
            Tier::P0,
            "exit 1, unsupported-capability",
            format!("exit {:?}, valid={}, code={code}", v.exit, v.valid),
            !v.valid && code == "unsupported-capability",
            "",
        );
        let mut along = fixtures::single_family_capabilities("skills")?;
        if let (Some(m), Some(t)) = (
            along.as_object_mut(),
            fixtures::tools_capability().as_object(),
        ) {
            for (k, v) in t {
                m.insert(k.clone(), v.clone());
            }
        }
        let spec = PackageSpec::native("omm-tools-along", along);
        let root = fixtures::write_package(&dir, "tools-along", &spec)?;
        let v = probe::plugins_validate(&self.inv, &root)?;
        let warned = v
            .diagnostics
            .iter()
            .any(|d| d.code == "unsupported-capability");
        self.record(
            "families/tools-alongside-warns",
            Tier::P0,
            "valid:true + unsupported-capability warning, summary partial",
            format!(
                "valid={}, warning={warned}, summary={}",
                v.valid,
                v.summary.as_deref().unwrap_or("?")
            ),
            v.valid && warned && v.summary.as_deref() == Some("partial"),
            "the four-predicate rule (R11) exists because `valid` alone hides this",
        );
        Ok(())
    }

    fn check_settings_schema_version(&mut self) -> Result<()> {
        let settings = self.sb.config_home.join("muse").join("settings.json");
        for bad in [0u64, 2u64] {
            write(
                &settings,
                format!("{{\"schema_version\":{bad}}}\n").as_bytes(),
            )?;
            let out = self
                .inv
                .run(&["skills", "list", "--source", "user", "--json"])?;
            let msg = out.stderr.lines().next().unwrap_or("").trim().to_string();
            let pass = out.kind == OutcomeKind::RunFailed
                && msg.starts_with(hr::SETTINGS_UNSUPPORTED_VERSION_MESSAGE);
            self.record(
                format!("settings/schema_version-{bad}-rejected"),
                Tier::P0,
                format!(
                    "exit 1, `{} {bad} …`",
                    hr::SETTINGS_UNSUPPORTED_VERSION_MESSAGE
                ),
                format!("exit {:?}, `{}`", out.code, clip(&msg, 60)),
                pass,
                "",
            );
        }
        write(&settings, b"{\"schema_version\":1}\n")?;
        let out = self
            .inv
            .run(&["skills", "list", "--source", "user", "--json"])?;
        self.record(
            "settings/schema_version-1-accepted",
            Tier::P0,
            "exit 0",
            format!("exit {:?}", out.code),
            out.ok(),
            "",
        );
        Ok(())
    }

    fn check_echo_session(&mut self) -> Result<()> {
        let opts = EchoSessionOptions::in_workspace(&self.ws)
            .trusted(true)
            .with_export(true);
        let session = probe::echo_session(&self.inv, &opts)?;
        let facts = &session.facts;
        let expected: Vec<u64> = hr::CONTEXT_BLOCK_ORDERS
            .iter()
            .map(|o| u64::from(*o))
            .collect();
        self.eq(
            "context/block-orders",
            Tier::P0,
            format!("{expected:?}"),
            format!("{:?}", facts.orders()),
        );
        let ids: Vec<String> = facts.context_blocks.iter().map(|b| b.id.clone()).collect();
        self.record(
            "context/block-ids",
            Tier::P0,
            "8 blocks",
            ids.join(","),
            facts.context_blocks.len() == hr::CONTEXT_BLOCK_ORDERS.len(),
            "",
        );
        if let Some(b) = facts.block(u64::from(hr::CONTEXT_ORDER_WORKFLOW_COOKBOOK)) {
            self.eq(
                "context/workflow-cookbook-bytes",
                Tier::P1,
                hr::WORKFLOW_COOKBOOK_BYTES,
                b.bytes as u64,
            );
        }
        self.eq(
            "tools/active-count",
            Tier::P1,
            hr::ACTIVE_TOOLS,
            facts.active_tools.len(),
        );
        let expected_tools: BTreeSet<&str> = hr::reserved_ids()?
            .builtin_tool_names
            .ids
            .iter()
            .map(String::as_str)
            .collect();
        let observed_tools: BTreeSet<&str> =
            facts.active_tools.iter().map(String::as_str).collect();
        let missing: Vec<&str> = expected_tools
            .difference(&observed_tools)
            .copied()
            .collect();
        let extra: Vec<&str> = observed_tools
            .difference(&expected_tools)
            .copied()
            .collect();
        self.record(
            "tools/active-set",
            Tier::P1,
            "the 29 names of reserved-ids.json",
            if missing.is_empty() && extra.is_empty() {
                "identical".to_string()
            } else {
                format!("missing={missing:?} extra={extra:?}")
            },
            missing.is_empty() && extra.is_empty(),
            "",
        );
        self.eq(
            "session/frame-schema-version",
            Tier::P0,
            hr::FRAME_SCHEMA_VERSION,
            facts.frame_schema_version.unwrap_or(0),
        );
        self.eq(
            "session/record-schema-versions",
            Tier::P0,
            "{1}".to_string(),
            format!("{:?}", facts.record_schema_versions),
        );
        self.eq(
            "export/schema-version",
            Tier::P0,
            hr::EXPORT_SCHEMA_VERSION,
            session.export_schema_version.unwrap_or(0),
        );
        // The same session untrusted: 8 blocks (no 186, with the order-u32::MAX
        // memory snapshot) and 23 tools (no
        // `subagent_*`) — the qualifier the P1 rows carry.
        let untrusted =
            probe::echo_session(&self.inv, &EchoSessionOptions::in_workspace(&self.ws))?;
        let expected: Vec<u64> = hr::CONTEXT_BLOCK_ORDERS_UNTRUSTED
            .iter()
            .map(|o| u64::from(*o))
            .collect();
        self.eq(
            "context/block-orders-untrusted",
            Tier::P1,
            format!("{expected:?}"),
            format!("{:?}", untrusted.facts.orders()),
        );
        self.eq(
            "tools/active-count-untrusted",
            Tier::P1,
            hr::ACTIVE_TOOLS_UNTRUSTED,
            untrusted.facts.active_tools.len(),
        );
        Ok(())
    }

    // ---- P1 --------------------------------------------------------------

    fn check_gates(&mut self) -> Result<()> {
        let report = probe::gates(&self.inv)?;
        self.record(
            "gates/trace-complete",
            Tier::P1,
            format!(
                "startup head + trust.resolve tail within {:.0} s",
                probe::GATE_PROBE_DEADLINE.as_secs_f64()
            ),
            format!(
                "complete on attempt {} after {} ms ({} lines)",
                report.attempts,
                report.elapsed.as_millis(),
                report.trace_lines
            ),
            report.attempts >= 1
                && report.attempts <= probe::GATE_PROBE_MAX_ATTEMPTS
                && report.elapsed <= probe::GATE_PROBE_DEADLINE + crate::invoke::RUN_TIMEOUT_DEFAULT,
            "the host's bootstrap-trace writer is lossy under concurrent bootstraps (gates.json probe.note)",
        );
        // The remote feature-config cache: a populated one would move
        // `gates/default-on` and `bundled/*` with no binary change, so the
        // line is asserted explicitly (probes run with TBH_DISABLE_FEATURE_CONFIG=1).
        let fc = report.feature_config.as_ref();
        self.record(
            "gates/feature-config-cache",
            Tier::P1,
            "state=\"missing\" gate_count=0",
            fc.map(|f| format!("state=\"{}\" gate_count={}", f.state, f.gate_count))
                .unwrap_or_else(|| "no feature_config.cache line in the trace".to_string()),
            fc.map(|f| f.gate_count == 0).unwrap_or(false),
            "server-side overrides in play (host-reality.md \"Server-side risk\"); every gate and bundled row below may reflect them",
        );
        let data = hr::gates()?;
        let observed_ids: Vec<String> = report.gates.iter().map(|g| g.id.clone()).collect();
        let observed: BTreeSet<&str> = observed_ids.iter().map(String::as_str).collect();
        let known: BTreeSet<&str> = data.items.iter().map(|g| g.id.as_str()).collect();
        // Every `since`-tagged gate is a fact of its group: present or not.
        for (since, id) in data.since_tagged() {
            let present = observed.contains(id);
            self.since_fact(
                since,
                format!("gates/since/{id}"),
                format!("gate.resolve line for `{id}` (since {since})"),
                present,
                if present {
                    report
                        .get(id)
                        .map(|g| format!("present, enabled={} source={:?}", g.enabled, g.source))
                        .unwrap_or_else(|| "present".to_string())
                } else {
                    "absent from the trace".to_string()
                },
            );
        }
        // What this build must show: every untagged row plus every tagged row
        // it does show; tagged rows it lacks are OLDER-BUILD only when their
        // whole group is absent (gates.json `since_rule`).
        let part = data.partition_for_build(&observed_ids);
        let (older, tagged_drift) = self.older_build_gates(data, &part);
        let unlisted: Vec<&str> = observed.difference(&known).copied().collect();
        let missing: Vec<String> = part
            .expected
            .iter()
            .filter(|id| !observed.contains(id.as_str()))
            .cloned()
            .collect();
        let expected_total = part.expected.len();
        self.record(
            "gates/total",
            Tier::P1,
            if older.is_empty() {
                format!(
                    "{expected_total} ({} listed in gates.json)",
                    data.items.len()
                )
            } else {
                format!(
                    "{expected_total} ({} listed in gates.json, {} OLDER-BUILD)",
                    data.items.len(),
                    older.len()
                )
            },
            report.gates.len(),
            report.gates.len() == expected_total && unlisted.is_empty() && missing.is_empty(),
            "",
        );
        let on: BTreeSet<&str> = report.default_on_ids().into_iter().collect();
        let expected_on_set = data.default_on_expected(&part);
        let expected_on: BTreeSet<&str> = expected_on_set.iter().map(String::as_str).collect();
        self.eq("gates/default-on", Tier::P1, expected_on.len(), on.len());
        let diff: Vec<String> = expected_on
            .symmetric_difference(&on)
            .map(|s| s.to_string())
            .collect();
        self.record(
            "gates/default-on-set",
            Tier::P1,
            format!("the {} default-ON ids of gates.json", expected_on.len()),
            if diff.is_empty() {
                "identical".to_string()
            } else {
                format!("differs: {diff:?}")
            },
            diff.is_empty(),
            "",
        );
        let mut problems = Vec::new();
        if !unlisted.is_empty() {
            problems.push(format!("unlisted gate(s) in the trace {unlisted:?} — a new gate is drift until gates.json lists it"));
        }
        if !missing.is_empty() {
            problems.push(format!("listed gate(s) missing from the trace {missing:?} — an untagged fact is never OLDER-BUILD"));
        }
        if !tagged_drift.is_empty() {
            problems.push(format!(
                "tagged gate(s) missing while their since group is partly present {tagged_drift:?}"
            ));
        }
        self.record(
            "gates/id-set",
            Tier::P1,
            if older.is_empty() {
                format!("the {} ids of gates.json", data.items.len())
            } else {
                format!(
                    "the {} ids of gates.json minus OLDER-BUILD {older:?}",
                    data.items.len()
                )
            },
            if problems.is_empty() {
                "identical".to_string()
            } else {
                problems.join("; ")
            },
            problems.is_empty(),
            "",
        );
        // Effect probes: what a gate withholds, measured (gates.json `effect_probe`).
        for gate in data.items.iter().filter(|g| g.effect_probe.is_some()) {
            let Some(probe) = gate.effect_probe.as_ref() else {
                continue;
            };
            let present = observed.contains(gate.id.as_str());
            self.check_gate_effect(gate, probe, present, &older)?;
        }
        Ok(())
    }

    /// Run one `effect_probe` (`settings_echo_session`): with the gate present
    /// in the trace, the closed and the open (variable set) session; with it
    /// absent, the `absent` expectation — recorded as OLDER-BUILD when the
    /// gate's absence is, as drift otherwise.
    fn check_gate_effect(
        &mut self,
        gate: &hr::Gate,
        probe: &hr::GateEffectProbe,
        present: bool,
        older: &[String],
    ) -> Result<()> {
        if probe.kind != hr::GATE_EFFECT_PROBE_SETTINGS_ECHO_SESSION {
            self.record(
                format!("gates/effect/{}", gate.id),
                Tier::P1,
                hr::GATE_EFFECT_PROBE_SETTINGS_ECHO_SESSION,
                &probe.kind,
                false,
                "unknown effect_probe.kind in gates.json",
            );
            return Ok(());
        }
        let settings = self.sb.config_home.join("muse").join("settings.json");
        let prior = std::fs::read(&settings).map_err(|e| HostError::io("read", &settings, e))?;
        let doc = serde_json::to_vec(&probe.settings).map_err(|e| HostError::Parse {
            what: "gates.json effect_probe.settings",
            detail: e.to_string(),
        })?;
        write(&settings, &doc)?;
        let closed_prefix = probe.closed.stderr_prefix.clone().unwrap_or_default();
        let observe = |inv: &Invoker, ws: &Path| -> Result<(Vec<u64>, String)> {
            let opts = EchoSessionOptions::in_workspace(ws).trusted(true);
            let session = probe::echo_session(inv, &opts)?;
            let notice = session
                .exec
                .stderr
                .lines()
                .map(str::trim)
                .find(|l| !closed_prefix.is_empty() && l.starts_with(closed_prefix.as_str()))
                .unwrap_or("")
                .to_string();
            Ok((session.facts.orders(), notice))
        };
        let judge =
            |exp: &hr::GateEffectExpectation, orders: &[u64], notice: &str| -> (bool, String) {
                let orders_ok = orders == exp.context_orders.as_slice();
                let notice_ok = match &exp.stderr_prefix {
                    Some(p) => notice.starts_with(p.as_str()),
                    None => notice.is_empty(),
                };
                (
                    orders_ok && notice_ok,
                    format!(
                        "orders {orders:?}, notice {}",
                        if notice.is_empty() {
                            "none".to_string()
                        } else {
                            format!("`{notice}`")
                        }
                    ),
                )
            };
        let expected_text = |exp: &hr::GateEffectExpectation| {
            format!(
                "orders {:?}, notice {}",
                exp.context_orders,
                exp.stderr_prefix
                    .as_deref()
                    .map(|p| format!("`{p}`"))
                    .unwrap_or_else(|| "none".to_string())
            )
        };
        let outcome: Result<()> = (|| {
            if present {
                let (orders, notice) = observe(&self.inv, &self.ws)?;
                let (ok, observed) = judge(&probe.closed, &orders, &notice);
                self.record(
                    format!("gates/effect/{}/closed", gate.id),
                    Tier::P1,
                    expected_text(&probe.closed),
                    observed,
                    ok,
                    format!("{} closed: {}", gate.env, gate.evidence),
                );
                let open_inv = self.inv.clone().env(&gate.env, hr::GATE_ON_VALUE);
                let (orders, notice) = observe(&open_inv, &self.ws)?;
                let (ok, observed) = judge(&probe.open, &orders, &notice);
                self.record(
                    format!("gates/effect/{}/open", gate.id),
                    Tier::P1,
                    expected_text(&probe.open),
                    observed,
                    ok,
                    format!("{}={}", gate.env, hr::GATE_ON_VALUE),
                );
            } else {
                let (orders, notice) = observe(&self.inv, &self.ws)?;
                let (ok, observed) = judge(&probe.absent, &orders, &notice);
                let id = format!("gates/effect/{}/absent", gate.id);
                let expected = expected_text(&probe.absent);
                if ok && older.contains(&gate.id) {
                    self.checks.push(Check {
                        id,
                        tier: Tier::P1,
                        expected,
                        observed,
                        status: Status::OlderBuild,
                        detail: format!(
                            "no `{}` gate on this build ({}) — the effect the gate withholds since {} composes unconditionally, as gates.json `effect_probe.absent` says",
                            gate.id,
                            self.version_note(),
                            gate.since.as_deref().unwrap_or("?")
                        ),
                    });
                } else {
                    self.record(
                        id,
                        Tier::P1,
                        expected,
                        observed,
                        ok && older.contains(&gate.id),
                        "",
                    );
                }
            }
            Ok(())
        })();
        write(&settings, &prior)?;
        outcome
    }

    /// The `--reasoning-effort` value list of muse-cli.json, value by value:
    /// a listed value under `--provider echo` fails on the provider, an
    /// unlisted one on the value — the order of the host's two checks makes
    /// the list observable without a model. Values tagged `values_since` are
    /// facts of their since group.
    fn check_cli_values(&mut self) -> Result<()> {
        let cli = hr::muse_cli()?;
        let flag = cli
            .root_flag("--reasoning-effort")
            .ok_or_else(|| {
                HostError::Probe("muse-cli.json lists no --reasoning-effort root flag".to_string())
            })?
            .clone();
        let mut accepted = Vec::new();
        let mut refused = Vec::new();
        for value in flag.accepted_values() {
            let out = self.inv.run(&[
                "exec",
                "--provider",
                "echo",
                "--reasoning-effort",
                value,
                "hi",
            ])?;
            let msg = out.stderr.lines().next().unwrap_or("").trim().to_string();
            let is_accepted = out.kind == OutcomeKind::ArgvRejected
                && msg.starts_with(hr::REASONING_EFFORT_ECHO_REFUSED_MESSAGE);
            match flag.values_since.get(value) {
                Some(since) => self.since_fact(
                    since,
                    format!("cli/reasoning-effort/{value}"),
                    format!("`--reasoning-effort {value}` accepted (since {since})"),
                    is_accepted,
                    format!("exit {:?}, `{}`", out.code, clip(&msg, 70)),
                ),
                None => {
                    if is_accepted {
                        accepted.push(value.to_string());
                    } else {
                        refused.push(format!("{value} (exit {:?}: {})", out.code, clip(&msg, 60)));
                    }
                }
            }
        }
        let untagged = flag
            .accepted_values()
            .iter()
            .filter(|v| !flag.values_since.contains_key(**v))
            .count();
        self.record(
            "cli/reasoning-effort-values",
            Tier::P1,
            format!(
                "{untagged} listed values accepted (`{}` after the value check)",
                hr::REASONING_EFFORT_ECHO_REFUSED_MESSAGE
            ),
            if refused.is_empty() {
                format!("{} accepted", accepted.len())
            } else {
                format!("{} accepted, refused {refused:?}", accepted.len())
            },
            refused.is_empty() && accepted.len() == untagged,
            "",
        );
        let out = self.inv.run(&[
            "exec",
            "--provider",
            "echo",
            "--reasoning-effort",
            "zzznotaneffort",
            "hi",
        ])?;
        let msg = out.stderr.lines().next().unwrap_or("").trim().to_string();
        self.record(
            "cli/reasoning-effort-bogus-rejected",
            Tier::P1,
            format!("exit 2, `{} …`", hr::REASONING_EFFORT_UNSUPPORTED_MESSAGE),
            format!("exit {:?}, `{}`", out.code, clip(&msg, 70)),
            out.kind == OutcomeKind::ArgvRejected
                && msg.starts_with(hr::REASONING_EFFORT_UNSUPPORTED_MESSAGE),
            "",
        );
        Ok(())
    }

    /// Resolve every `since` group: a present fact passes; an absent fact is
    /// OLDER-BUILD when its whole group is absent (note quotes the version
    /// string, informational) and FAIL when the group is mixed.
    fn check_since_groups(&mut self) -> Result<()> {
        let facts = self.since_facts.clone();
        if facts.is_empty() {
            self.record(
                "since/groups",
                Tier::P1,
                "no since-tagged facts in the data files",
                "none probed",
                true,
                "",
            );
            return Ok(());
        }
        let mut groups: BTreeSet<&str> = BTreeSet::new();
        for f in &facts {
            groups.insert(f.since.as_str());
        }
        let version = self.version_note();
        for since in groups {
            let group: Vec<&SinceFact> = facts.iter().filter(|f| f.since == since).collect();
            let present: Vec<&str> = group
                .iter()
                .filter(|f| f.present)
                .map(|f| f.id.as_str())
                .collect();
            let absent: Vec<&str> = group
                .iter()
                .filter(|f| !f.present)
                .map(|f| f.id.as_str())
                .collect();
            let whole_absent = present.is_empty();
            let mixed = !present.is_empty() && !absent.is_empty();
            for f in &group {
                let status = if f.present {
                    Status::Pass
                } else if whole_absent {
                    Status::OlderBuild
                } else {
                    Status::Fail
                };
                let detail = match status {
                    Status::OlderBuild => format!(
                        "absent, as is every fact tagged since {since} ({} of {}); this build reports `{version}` — quoted for the reader, never used to decide (R15)",
                        absent.len(),
                        group.len()
                    ),
                    Status::Fail => format!(
                        "since group {since} is mixed: present {present:?}, absent {absent:?} — a build that shows one fact of the group must show them all"
                    ),
                    _ => String::new(),
                };
                self.checks.push(Check {
                    id: f.id.clone(),
                    tier: Tier::P1,
                    expected: f.expected.clone(),
                    observed: f.observed.clone(),
                    status,
                    detail,
                });
            }
            self.record(
                format!("since/{since}"),
                Tier::P1,
                format!("{} facts all present or all absent", group.len()),
                if whole_absent {
                    format!("all {} absent (OLDER-BUILD)", group.len())
                } else if mixed {
                    format!("mixed: present {present:?}, absent {absent:?}")
                } else {
                    format!("all {} present", group.len())
                },
                !mixed,
                "",
            );
        }
        Ok(())
    }

    fn check_hook_events(&mut self) -> Result<()> {
        let dir = self.scratch("hooks")?;
        let events = hr::hook_events()?;
        let mut accepted = 0usize;
        let mut rejected_names = Vec::new();
        for ev in &events.items {
            let spec = PackageSpec::native(
                "omm-hook",
                json!({"hooks": [fixtures::hook_entry(&ev.name)]}),
            );
            let root = fixtures::write_package(&dir, &format!("ev-{}", ev.name), &spec)?;
            let v = probe::plugins_validate(&self.inv, &root)?;
            if v.passes() {
                accepted += 1;
            } else {
                rejected_names.push(ev.name.clone());
            }
        }
        self.record(
            "hooks/17-events-accepted",
            Tier::P1,
            format!("{} accepted", hr::HOOK_EVENTS),
            format!(
                "{accepted} accepted{}",
                if rejected_names.is_empty() {
                    String::new()
                } else {
                    format!(", rejected {rejected_names:?}")
                }
            ),
            accepted == hr::HOOK_EVENTS && events.items.len() == hr::HOOK_EVENTS,
            "",
        );
        // A foreign `Setup`, a made-up name and a snake_case spelling must all be rejected.
        let mut rejected = 0usize;
        let bogus = ["Setup", "PreSubmit", "session_start"];
        for name in bogus {
            let spec =
                PackageSpec::native("omm-hook", json!({"hooks": [fixtures::hook_entry(name)]}));
            let root = fixtures::write_package(&dir, &format!("bogus-{name}"), &spec)?;
            let v = probe::plugins_validate(&self.inv, &root)?;
            if !v.valid
                && v.error
                    .as_ref()
                    .map(|e| e.0 == "unsupported-hook-event")
                    .unwrap_or(false)
            {
                rejected += 1;
            }
        }
        self.eq(
            "hooks/bogus-events-rejected",
            Tier::P1,
            bogus.len(),
            rejected,
        );
        Ok(())
    }

    fn check_bundled_skills(&mut self) -> Result<()> {
        let opts = SkillsListOptions {
            source: Some("built-in".to_string()),
            ..Default::default()
        };
        let visible = probe::skills_list(&self.inv, &opts)?;
        self.eq(
            "bundled/visible-without-gate",
            Tier::P1,
            hr::BUNDLED_SKILLS_VISIBLE_DEFAULT,
            visible.skills.len(),
        );
        let all = probe::skills_list(&self.inv.clone().with_plugins_gate(), &opts)?;
        self.eq(
            "bundled/with-gate",
            Tier::P1,
            hr::BUNDLED_SKILLS,
            all.skills.len(),
        );
        let data = hr::bundled_skills()?;
        let expected: BTreeSet<&str> = data.items.iter().map(|s| s.qualified_id.as_str()).collect();
        let observed: BTreeSet<&str> = all.skills.iter().map(|s| s.id.as_str()).collect();
        let diff: Vec<String> = expected
            .symmetric_difference(&observed)
            .map(|s| s.to_string())
            .collect();
        self.record(
            "bundled/id-set",
            Tier::P1,
            "the 15 ids of bundled-skills.json",
            if diff.is_empty() {
                "identical".to_string()
            } else {
                format!("differs: {diff:?}")
            },
            diff.is_empty(),
            "",
        );
        let tree = self.sb.roots()?.bundled_skills_dir();
        let files = walkdir::WalkDir::new(&tree)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .count();
        self.eq(
            "bundled/package-files",
            Tier::P1,
            hr::BUNDLED_SKILL_PACKAGE_FILES,
            files,
        );
        Ok(())
    }

    fn check_cli_commands(&mut self) -> Result<()> {
        let cli = hr::muse_cli()?;
        let mut ok = 0usize;
        let mut failed = Vec::new();
        for cmd in &cli.argv_allowlist {
            let out = self.inv.run(&[cmd.as_str(), "--help"])?;
            if out.ok() {
                ok += 1;
            } else {
                failed.push(format!("{cmd}={:?}", out.code));
            }
        }
        self.record(
            "cli/top-commands-help",
            Tier::P1,
            format!("{} × `<cmd> --help` exit 0", hr::TOP_LEVEL_COMMANDS),
            format!(
                "{ok} ok{}",
                if failed.is_empty() {
                    String::new()
                } else {
                    format!(", failed {failed:?}")
                }
            ),
            ok == hr::TOP_LEVEL_COMMANDS && cli.argv_allowlist.len() == hr::TOP_LEVEL_COMMANDS,
            "",
        );
        let help = self
            .inv
            .clone()
            .allow_root_flags()
            .run(&["--help"])?
            .expect_ok()?;
        let advertised = help
            .stdout
            .lines()
            .skip_while(|l| l.trim() != "Commands:")
            .skip(1)
            .take_while(|l| !l.trim().is_empty())
            .count();
        self.eq(
            "cli/advertised-commands",
            Tier::P1,
            hr::ADVERTISED_COMMANDS,
            advertised,
        );
        let ungated = self
            .inv
            .clone()
            .env_remove(hr::ENV_PLUGINS_GATE)
            .run(&["skills", "list", "--json"])?; // control: gate irrelevant here
        self.record(
            "cli/skills-list-ok",
            Tier::P1,
            "exit 0",
            format!("exit {:?}", ungated.code),
            ungated.ok(),
            "",
        );
        let workflows = self.inv.run(&["workflows", "list"])?;
        self.record(
            "cli/workflows-hidden-but-live",
            Tier::P1,
            "exit 0",
            format!("exit {:?}", workflows.code),
            workflows.ok(),
            "",
        );
        Ok(())
    }

    fn check_exit_codes(&mut self) -> Result<()> {
        let rows: [(&str, Vec<&str>, bool, OutcomeKind); 5] = [
            (
                "exit/unknown-root-flag",
                vec!["--zzznotaflag"],
                true,
                OutcomeKind::ArgvRejected,
            ),
            (
                "exit/unknown-subcommand",
                vec!["skills", "zzznotaverb"],
                false,
                OutcomeKind::ArgvRejected,
            ),
            ("exit/version", vec!["--version"], true, OutcomeKind::Ok),
            (
                "exit/plugins-gated-ok",
                vec!["plugins", "list"],
                false,
                OutcomeKind::Ok,
            ),
            (
                "exit/config-validate-invalid",
                vec![],
                false,
                OutcomeKind::RunFailed,
            ),
        ];
        for (id, argv, root_flags, expected) in rows {
            let out = if argv.is_empty() {
                let file = self.scratch("exit")?.join("bad.json");
                write(&file, b"{\"schema_version\":2}")?;
                self.inv.run(&[
                    "config".to_string(),
                    "validate".to_string(),
                    "--plane".to_string(),
                    "defaults".to_string(),
                    "--file".to_string(),
                    file.to_string_lossy().into_owned(),
                ])?
            } else if root_flags {
                self.inv.clone().allow_root_flags().run(&argv)?
            } else {
                self.inv.run(&argv)?
            };
            self.eq(
                id,
                Tier::P1,
                format!("{expected:?}"),
                format!("{:?}", out.kind),
            );
        }
        // The plugins gate is what separates exit 2 from exit 0 on `plugins list`.
        let ungated = self
            .inv
            .clone()
            .without_plugins_gate()
            .run(&["plugins", "list"])?;
        self.eq(
            "exit/plugins-ungated",
            Tier::P1,
            hr::EXIT_ARGV_REJECTED,
            ungated.code.unwrap_or(-1),
        );
        Ok(())
    }

    fn check_slash_commands(&mut self) -> Result<()> {
        let data = hr::slash_commands()?;
        let pass = data.items.len() == hr::BUILTIN_SLASH_COMMANDS;
        self.checks.push(Check {
            id: "slash/39-rows".to_string(),
            tier: Tier::P1,
            expected: hr::BUILTIN_SLASH_COMMANDS.to_string(),
            observed: data.items.len().to_string(),
            status: if pass { Status::DataOnly } else { Status::Fail },
            detail: "the composer picker is TUI-only (pty harness, tui-slash-theme.md §1.4); asserted against docs/host-data/slash-commands.json, not the binary".to_string(),
        });
        Ok(())
    }
}
