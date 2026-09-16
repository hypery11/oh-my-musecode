//! Behaviour probes — R15: capabilities are observed, never inferred from the
//! version string.
//!
//! Each probe runs one host command through [`Invoker`], parses the exact
//! output shape recorded in `research/` (and re-verified live on
//! `1.0.1-R2006.1`), and returns a typed value plus the raw JSON where the
//! caller may want more.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;

use crate::error::{HostError, Result};
use crate::fsx;
use crate::host_reality as hr;
use crate::invoke::{first_line, Invoker, Outcome, OutcomeKind, Sandbox};

fn tempdir(prefix: &str) -> Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .map_err(|e| HostError::io("create temp dir", std::env::temp_dir(), e))
}

// ---------------------------------------------------------------------------
// --version
// ---------------------------------------------------------------------------

/// The parsed `--version` line, e.g. `Muse Code 1.0.1 (1.0.1-R2006.1)`
/// (`research/musecode/cli-surface.md` header). Informational only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub product: String,
    pub version: String,
    pub build: String,
    pub raw: String,
}

/// Run `muse --version`.
pub fn version(inv: &Invoker) -> Result<Version> {
    let out = inv
        .clone()
        .allow_root_flags()
        .run(&["--version"])?
        .expect_ok()?;
    parse_version(first_line(&out.stdout))
}

/// Parse `<product words> <version> (<build>)`.
pub fn parse_version(line: &str) -> Result<Version> {
    let line = line.trim();
    let err = || HostError::Parse {
        what: "--version line",
        detail: line.to_string(),
    };
    let open = line.rfind('(').ok_or_else(err)?;
    let close = line.rfind(')').ok_or_else(err)?;
    if close <= open {
        return Err(err());
    }
    let build = line[open + 1..close].trim().to_string();
    let head = line[..open].trim();
    let (product, version) = head.rsplit_once(' ').ok_or_else(err)?;
    if product.is_empty() || version.is_empty() || build.is_empty() {
        return Err(err());
    }
    Ok(Version {
        product: product.to_string(),
        version: version.to_string(),
        build,
        raw: line.to_string(),
    })
}

// ---------------------------------------------------------------------------
// gates from the bootstrap trace
// ---------------------------------------------------------------------------

/// `source="default"` vs `source="override"` on a `gate.resolve` line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateSource {
    Default,
    Override,
}

/// One `event="gate.resolve" gate="<id>" enabled=<bool> source="<src>"` line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateResolution {
    pub id: String,
    pub enabled: bool,
    pub source: GateSource,
}

/// The `event="feature_config.cache" state=… gate_count=…` line of a
/// bootstrap trace (`hr::FEATURE_CONFIG_CACHE_EVENT`): whether a remote
/// feature-config cache was consulted and how many gates it carried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureConfigCache {
    /// `missing` on every offline run.
    pub state: String,
    /// `0` unless server-side overrides are cached.
    pub gate_count: u64,
}

/// All gate resolutions of one bootstrap, in trace order (which matches the
/// binary's registry order — gates.json `probe.note`).
#[derive(Clone, Debug, Default)]
pub struct GateReport {
    pub gates: Vec<GateResolution>,
    /// How many `plugins enable` runs it took to obtain a complete trace
    /// (1 = first try; the host's trace writer is lossy under load).
    pub attempts: u32,
    /// Wall-clock time from the first attempt to the accepted trace.
    pub elapsed: std::time::Duration,
    /// Non-empty lines of the trace that was accepted.
    pub trace_lines: usize,
    /// The feature-config cache line, when the trace carried one.
    pub feature_config: Option<FeatureConfigCache>,
}

impl GateReport {
    /// Ids that resolved `enabled=true source="default"`.
    pub fn default_on_ids(&self) -> Vec<&str> {
        self.gates
            .iter()
            .filter(|g| g.enabled && g.source == GateSource::Default)
            .map(|g| g.id.as_str())
            .collect()
    }
    /// Ids that resolved `enabled=true` from either source.
    pub fn enabled_ids(&self) -> Vec<&str> {
        self.gates
            .iter()
            .filter(|g| g.enabled)
            .map(|g| g.id.as_str())
            .collect()
    }
    /// Look one gate up.
    pub fn get(&self, id: &str) -> Option<&GateResolution> {
        self.gates.iter().find(|g| g.id == id)
    }
}

/// How long [`gates`] keeps re-running the probe verb for a complete
/// bootstrap trace. The loss is a race between the host's exit and its
/// trace-writer thread, so it lasts as long as the machine is busy: under an
/// 8-wide storm of concurrent host sessions a fixed budget of eight attempts
/// (≈ 2–4 s) still failed 2 of 12 suite runs, because a burst of sessions
/// outlives it. A wall-clock deadline does not: it is set well under
/// [`crate::invoke::RUN_TIMEOUT_DEFAULT`] (30 s), so a probe never outlasts
/// the budget of the verb it wraps. A normal run completes on the first try
/// in ~0.2 s and never waits.
pub const GATE_PROBE_DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);
/// First backoff of the retry schedule; doubles per attempt up to
/// [`GATE_PROBE_BACKOFF_CAP`] (`100 ms, 200, 400, …, 2 s, 2 s, …`).
pub const GATE_PROBE_BACKOFF_MIN: std::time::Duration = std::time::Duration::from_millis(100);
/// Ceiling of one backoff sleep, so the deadline is spent in probes rather
/// than in one long wait.
pub const GATE_PROBE_BACKOFF_CAP: std::time::Duration = std::time::Duration::from_secs(2);
/// A ceiling on attempts that the schedule cannot reach before
/// [`GATE_PROBE_DEADLINE`] (13 sleeps already sum to ≈ 17 s), kept as a
/// guard against a clock that stands still: [`gates`] never spins unbounded.
pub const GATE_PROBE_MAX_ATTEMPTS: u32 = 64;

/// Gate probes of one process run one at a time: omm never adds to the very
/// storm that truncates the trace, and hostcheck / tests in the same process
/// stop racing each other.
static GATE_PROBE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Backoff before retry `attempt` (1-based): [`GATE_PROBE_BACKOFF_MIN`] ×
/// 2^(attempt−1), capped at [`GATE_PROBE_BACKOFF_CAP`], ±50 % jitter from
/// the clock so concurrent processes de-synchronise.
fn gate_probe_backoff(attempt: u32) -> std::time::Duration {
    let doublings = attempt.saturating_sub(1).min(16);
    let base = GATE_PROBE_BACKOFF_MIN
        .saturating_mul(1u32 << doublings)
        .min(GATE_PROBE_BACKOFF_CAP)
        .as_millis() as u64;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let jitter = (u64::from(nanos) % (base + 1)).saturating_sub(base / 2);
    std::time::Duration::from_millis(base / 2 + jitter)
}

/// What a bootstrap trace looks like, structurally. The host's writer is
/// lossy under concurrent bootstraps: a log may be empty, a head-only prefix
/// (no `trust.resolve`), or a mid-stream suffix (no `startup`) — measured
/// 2026-09-02 on 1.0.1-R2006.1, gates.json `probe.note`. Only a trace with
/// BOTH markers is authoritative; a partial gate table is never reported.
#[derive(Clone, Debug, Default)]
pub struct TraceShape {
    /// First non-empty line is `event="startup"`.
    pub head: bool,
    /// An `event="trust.resolve"` line follows the last `gate.resolve` line.
    pub tail: bool,
    /// Non-empty lines.
    pub lines: usize,
    /// The gate lines that were present, complete or not.
    pub gates: Vec<GateResolution>,
    /// The feature-config cache line, if present.
    pub feature_config: Option<FeatureConfigCache>,
}

impl TraceShape {
    /// Head and tail present and at least one gate line between them.
    pub fn complete(&self) -> bool {
        self.head && self.tail && !self.gates.is_empty()
    }
    /// One-line description for diagnostics.
    pub fn describe(&self) -> String {
        format!(
            "{} lines, startup head {}, trust.resolve tail {}, {} gate.resolve lines",
            self.lines,
            if self.head { "present" } else { "MISSING" },
            if self.tail { "present" } else { "MISSING" },
            self.gates.len()
        )
    }
}

/// Inspect a bootstrap trace text for completeness (see [`TraceShape`]).
pub fn inspect_gate_trace(text: &str) -> TraceShape {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let head = lines
        .first()
        .map(|l| l.contains(hr::BOOTSTRAP_TRACE_HEAD_EVENT))
        .unwrap_or(false);
    let last_gate = lines
        .iter()
        .rposition(|l| l.contains(hr::GATE_RESOLVE_EVENT));
    let tail = match last_gate {
        Some(i) => lines[i + 1..]
            .iter()
            .any(|l| l.contains(hr::BOOTSTRAP_TRACE_TAIL_EVENT)),
        None => false,
    };
    TraceShape {
        head,
        tail,
        lines: lines.len(),
        gates: parse_gate_lines(text),
        feature_config: parse_feature_config_line(text),
    }
}

/// The `feature_config.cache` line of a trace, if any.
pub fn parse_feature_config_line(text: &str) -> Option<FeatureConfigCache> {
    text.lines()
        .find(|l| l.contains(hr::FEATURE_CONFIG_CACHE_EVENT))
        .and_then(|l| {
            Some(FeatureConfigCache {
                state: attr(l, "state")?.to_string(),
                gate_count: attr(l, "gate_count")?.parse().ok()?,
            })
        })
}

/// Parse a bootstrap trace into a [`GateReport`], refusing an incomplete one
/// with `HostError::Probe("truncated bootstrap trace …")`.
pub fn parse_gate_trace(text: &str) -> Result<GateReport> {
    let shape = inspect_gate_trace(text);
    if !shape.complete() {
        return Err(HostError::Probe(format!(
            "truncated bootstrap trace ({}); the host's trace writer is lossy under concurrent bootstraps, a partial gate table is never trusted",
            shape.describe()
        )));
    }
    Ok(GateReport {
        gates: shape.gates,
        attempts: 1,
        elapsed: std::time::Duration::ZERO,
        trace_lines: shape.lines,
        feature_config: shape.feature_config,
    })
}

/// Resolve the gate table (gates.json; 41 rows on 1.0.1-R2006.1, 42 on
/// 1.0.3-R2198.1) by running a cheap plugin *mutation* verb — only a
/// mutation subcommand emits the full `gate.resolve` table
/// (`research/experiments/canary-diff.md` §1; `plugins list` does not) — with
/// the data root redirected to a temp dir so the trace (and the
/// `plugins/.installed.lock` the verb creates) never touch the user's data
/// root. The gate values reflect the invoker's environment.
///
/// The trace is accepted only when complete (startup head AND trust.resolve
/// tail — [`TraceShape`]); otherwise the verb is re-run, in a fresh temp data
/// root, with an exponential jittered backoff until [`GATE_PROBE_DEADLINE`]
/// has elapsed (never more than [`GATE_PROBE_MAX_ATTEMPTS`] times), and the
/// last failure is reported as `HostError::Probe` saying how many attempts
/// were made and how long they took. A partial [`GateReport`] is never
/// returned; a complete one records its `attempts` and `elapsed`.
pub fn gates(inv: &Invoker) -> Result<GateReport> {
    gates_within(inv, GATE_PROBE_DEADLINE)
}

/// [`gates`] with an explicit wall-clock deadline for the retries (the last
/// attempt may start just before it and run its own course).
pub fn gates_within(inv: &Invoker, deadline: std::time::Duration) -> Result<GateReport> {
    // A poisoned lock only means another probe panicked; the guard holds no data.
    let _serial = GATE_PROBE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let started = std::time::Instant::now();
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match gates_once(inv) {
            Ok(mut report) => {
                report.attempts = attempt;
                report.elapsed = started.elapsed();
                return Ok(report);
            }
            Err(e) => {
                let elapsed = started.elapsed();
                if elapsed >= deadline || attempt >= GATE_PROBE_MAX_ATTEMPTS {
                    return Err(HostError::Probe(format!(
                        "gate probe: no complete bootstrap trace after {attempt} attempt(s) in {:.1} s (deadline {:.1} s, backoff {} ms doubling to {} s); last attempt: {e}",
                        elapsed.as_secs_f64(),
                        deadline.as_secs_f64(),
                        GATE_PROBE_BACKOFF_MIN.as_millis(),
                        GATE_PROBE_BACKOFF_CAP.as_secs()
                    )));
                }
                let remaining = deadline - elapsed;
                std::thread::sleep(gate_probe_backoff(attempt).min(remaining));
            }
        }
    }
}

fn gates_once(inv: &Invoker) -> Result<GateReport> {
    let tmp = tempdir("omm-gates-")?;
    let data = tmp.path().join("data");
    fsx::create_dir_all(&data)?;
    let probe = inv.clone().data_home(&data);
    // The verb and the trace location come from gates.json `probe` (R8).
    // Exit 1 (`plugin `__omm_gate_probe__` is not installed`) is expected.
    let spec = &hr::gates()?.probe;
    let out = probe.run(&spec.argv())?;
    let trace_dir = data.join("muse").join(
        spec.trace_subdir()
            .unwrap_or_else(|| hr::BOOTSTRAP_TRACE_SUBDIR.to_string()),
    );
    let mut text = String::new();
    if let Ok(entries) = std::fs::read_dir(&trace_dir) {
        for entry in entries.flatten() {
            if entry.path().extension().and_then(|e| e.to_str()) == Some("log") {
                text.push_str(&fsx::read_to_string(&entry.path())?);
                text.push('\n');
            }
        }
    }
    parse_gate_trace(&text).map_err(|e| {
        HostError::Probe(format!(
            "{e} — in {} (exit {:?}: {})",
            trace_dir.display(),
            out.code,
            first_line(&out.stderr)
        ))
    })
}

/// Extract `key=value` / `key="value"` from a trace line.
fn attr<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!(" {key}=");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    if let Some(quoted) = rest.strip_prefix('"') {
        let end = quoted.find('"')?;
        Some(&quoted[..end])
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

/// Parse every `gate.resolve` line in a trace text.
pub fn parse_gate_lines(text: &str) -> Vec<GateResolution> {
    text.lines()
        .filter(|l| l.contains(hr::GATE_RESOLVE_EVENT))
        .filter_map(|l| {
            let id = attr(l, "gate")?;
            let enabled = attr(l, "enabled")? == "true";
            let source = match attr(l, "source")? {
                "default" => GateSource::Default,
                _ => GateSource::Override,
            };
            Some(GateResolution {
                id: id.to_string(),
                enabled,
                source,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// skills list --json
// ---------------------------------------------------------------------------

/// A diagnostic row as the host emits it (`{code, message, scope?, path?, severity?}`).
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
}

/// One row of `skills list --json` (cli-surface.md verification, "skills list --json schema").
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SkillEntry {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub activation: String,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// The `{skills:[…], diagnostics:[…]}` document.
#[derive(Clone, Debug, Default)]
pub struct SkillsList {
    pub skills: Vec<SkillEntry>,
    pub diagnostics: Vec<Diagnostic>,
    pub raw: Value,
}

/// Options for `skills list`.
#[derive(Clone, Debug, Default)]
pub struct SkillsListOptions {
    /// `--source all|user|project|built-in|plugin`.
    pub source: Option<String>,
    /// `--workspace <path>`.
    pub workspace: Option<PathBuf>,
    /// `--trust-workspace` (project skills are trust-gated).
    pub trust_workspace: bool,
    /// `--enabled-only`.
    pub enabled_only: bool,
}

/// Run `skills list --json`. The bundled `create-plugin` skill is listed only
/// when the invoker carries the plugins gate (bundled-skills.json).
pub fn skills_list(inv: &Invoker, opts: &SkillsListOptions) -> Result<SkillsList> {
    let mut argv: Vec<String> = vec!["skills".into(), "list".into()];
    if let Some(s) = &opts.source {
        argv.push("--source".into());
        argv.push(s.clone());
    }
    if let Some(ws) = &opts.workspace {
        argv.push("--workspace".into());
        argv.push(ws.to_string_lossy().into_owned());
    }
    if opts.trust_workspace {
        argv.push("--trust-workspace".into());
    }
    if opts.enabled_only {
        argv.push("--enabled-only".into());
    }
    argv.push("--json".into());
    let out = inv.run(&argv)?;
    if !out.ok() {
        return Err(HostError::Command {
            argv: out.argv_string(),
            code: out.code,
            stderr: first_line(&out.stderr).to_string(),
        });
    }
    let raw = out.first_json()?;
    let skills = raw
        .get("skills")
        .map(|v| serde_json::from_value::<Vec<SkillEntry>>(v.clone()))
        .transpose()
        .map_err(|e| HostError::Parse {
            what: "skills list --json skills[]",
            detail: e.to_string(),
        })?
        .unwrap_or_default();
    let diagnostics = raw
        .get("diagnostics")
        .map(|v| serde_json::from_value::<Vec<Diagnostic>>(v.clone()))
        .transpose()
        .map_err(|e| HostError::Parse {
            what: "skills list --json diagnostics[]",
            detail: e.to_string(),
        })?
        .unwrap_or_default();
    Ok(SkillsList {
        skills,
        diagnostics,
        raw,
    })
}

// ---------------------------------------------------------------------------
// plugins inspect / validate / approve
// ---------------------------------------------------------------------------

/// One `runtime_capabilities[]` row of `plugins inspect --json`
/// (`research/musecode/plugins.md` §9.3, verified live).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeCapability {
    pub stable_id: String,
    pub kind: String,
    pub plugin_id: String,
    pub capability_id: String,
    /// `review_needed | trusted_enabled | trusted_disabled | modified | invalid | blocked`.
    pub status: String,
    pub diagnostic_code: Option<String>,
    pub diagnostic_message: Option<String>,
}

impl RuntimeCapability {
    /// The only state in which the capability spawns (R14).
    pub fn is_trusted_enabled(&self) -> bool {
        self.status == hr::RUNTIME_CAPABILITY_ACTIVE_STATE
    }
}

/// One `effective_capabilities[]` row (skills and commands, active without review).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct EffectiveCapability {
    pub kind: String,
    pub stable_id: String,
}

/// `plugins inspect <id> --json`.
#[derive(Clone, Debug)]
pub struct PluginInspect {
    pub id: String,
    pub manifest_family: Option<String>,
    pub enabled: Option<bool>,
    pub valid: Option<bool>,
    pub active: Option<bool>,
    pub warning: Option<String>,
    pub runtime_capabilities: Vec<RuntimeCapability>,
    pub effective_capabilities: Vec<EffectiveCapability>,
    pub diagnostics: Vec<Diagnostic>,
    pub capability_diagnostics: Vec<Diagnostic>,
    pub raw: Value,
}

impl PluginInspect {
    /// Find a runtime capability line by stable id.
    pub fn capability(&self, stable_id: &str) -> Option<&RuntimeCapability> {
        self.runtime_capabilities
            .iter()
            .find(|c| c.stable_id == stable_id)
    }
    /// R14: the line is PRESENT and literally `trusted_enabled`.
    pub fn is_trusted_enabled(&self, stable_id: &str) -> bool {
        self.capability(stable_id)
            .map(RuntimeCapability::is_trusted_enabled)
            .unwrap_or(false)
    }
}

fn diagnostics_at(v: &Value, key: &str) -> Vec<Diagnostic> {
    v.get(key)
        .and_then(|d| serde_json::from_value::<Vec<Diagnostic>>(d.clone()).ok())
        .unwrap_or_default()
}

fn str_at(v: &Value, path: &[&str]) -> Option<String> {
    let mut cur = v;
    for p in path {
        cur = cur.get(p)?;
    }
    cur.as_str().map(str::to_string)
}

/// Run `plugins inspect <id> --json` (the plugins gate is added automatically).
pub fn plugins_inspect(inv: &Invoker, id: &str) -> Result<PluginInspect> {
    let out = inv.run(&["plugins", "inspect", id, "--json"])?;
    if let Some(err) = out.host_reported_error() {
        return Err(err);
    }
    let raw = out.first_json()?;
    if !out.ok() {
        return Err(HostError::Command {
            argv: out.argv_string(),
            code: out.code,
            stderr: first_line(&out.stderr).to_string(),
        });
    }
    let runtime_capabilities = raw
        .get("runtime_capabilities")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let c = row.get("candidate")?;
                    Some(RuntimeCapability {
                        stable_id: str_at(c, &["stable_id"])?,
                        kind: str_at(c, &["kind"]).unwrap_or_default(),
                        plugin_id: str_at(c, &["plugin_id"]).unwrap_or_default(),
                        capability_id: str_at(c, &["capability_id"]).unwrap_or_default(),
                        status: str_at(row, &["status"]).unwrap_or_default(),
                        diagnostic_code: str_at(row, &["diagnostic", "code"]),
                        diagnostic_message: str_at(row, &["diagnostic", "message"]),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let effective_capabilities = raw
        .get("effective_capabilities")
        .and_then(|d| serde_json::from_value::<Vec<EffectiveCapability>>(d.clone()).ok())
        .unwrap_or_default();
    Ok(PluginInspect {
        id: str_at(&raw, &["record", "id"]).unwrap_or_else(|| id.to_string()),
        manifest_family: str_at(&raw, &["record", "manifest_family"])
            .or_else(|| str_at(&raw, &["plugin", "manifest_family"])),
        enabled: raw
            .get("record")
            .and_then(|r| r.get("enabled"))
            .and_then(Value::as_bool),
        valid: raw.get("valid").and_then(Value::as_bool),
        active: raw.get("active").and_then(Value::as_bool),
        warning: str_at(&raw, &["warning"]),
        runtime_capabilities,
        effective_capabilities,
        diagnostics: diagnostics_at(&raw, "diagnostics"),
        capability_diagnostics: diagnostics_at(&raw, "capability_diagnostics"),
        raw,
    })
}

/// One `plugin.compatibility.declarations[]` row of `plugins validate --json`
/// (`research/experiments/canary-diff.md` §2.1).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Declaration {
    pub id: String,
    pub kind: String,
    pub classification: String,
}

/// `plugins validate <path> --json`, success or error shape.
#[derive(Clone, Debug)]
pub struct PluginValidation {
    pub exit: OutcomeKind,
    pub valid: bool,
    /// `(code, message)` of the `error` document on a fatal result.
    pub error: Option<(String, String)>,
    pub diagnostics: Vec<Diagnostic>,
    pub manifest_family: Option<String>,
    /// Keys of `plugin.capabilities` as the validator spells them
    /// (`skills hooks mcp_servers commands reminders`).
    pub capabilities: Vec<String>,
    /// `plugin.compatibility.summary`: `full | partial | unsupported`.
    pub summary: Option<String>,
    pub declarations: Vec<Declaration>,
    pub raw: Value,
}

impl PluginValidation {
    /// R11 — the four predicates: `rc==0 && valid && diagnostics==[] &&
    /// manifest_family=="native" && every declaration "supported"`. Returns
    /// the list of failed predicates (empty = pass). Never `valid` alone.
    pub fn four_predicates(&self) -> Vec<String> {
        let mut failed = Vec::new();
        if self.exit != OutcomeKind::Ok {
            failed.push(format!("exit status {:?} is not 0", self.exit));
        }
        if !self.valid {
            failed.push("valid != true".to_string());
        }
        if !self.diagnostics.is_empty() {
            failed.push(format!(
                "diagnostics not empty: {}",
                self.diagnostics
                    .iter()
                    .map(|d| d.code.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if self.manifest_family.as_deref() != Some(hr::MANIFEST_FAMILY_NATIVE) {
            failed.push(format!(
                "manifest_family {:?} is not {:?}",
                self.manifest_family,
                hr::MANIFEST_FAMILY_NATIVE
            ));
        }
        let unsupported: Vec<&str> = self
            .declarations
            .iter()
            .filter(|d| d.classification != "supported")
            .map(|d| d.id.as_str())
            .collect();
        if !unsupported.is_empty() {
            failed.push(format!(
                "declarations not supported: {}",
                unsupported.join(", ")
            ));
        }
        if self.declarations.is_empty() {
            failed.push("no capability declarations (a capability-less package)".to_string());
        }
        failed
    }

    /// True when [`PluginValidation::four_predicates`] is empty.
    pub fn passes(&self) -> bool {
        self.four_predicates().is_empty()
    }
}

/// Run `plugins validate <path> --json`.
pub fn plugins_validate(inv: &Invoker, package: &Path) -> Result<PluginValidation> {
    let out = inv.run(&[
        "plugins".to_string(),
        "validate".to_string(),
        package.to_string_lossy().into_owned(),
        "--json".to_string(),
    ])?;
    let raw = out.first_json()?;
    Ok(parse_plugin_validation(&out, raw))
}

fn parse_plugin_validation(out: &Outcome, raw: Value) -> PluginValidation {
    // Error shape: {"error":{"code","message","details":{valid,plugin,capabilities,diagnostics}}}
    let (body, error) = match raw.get("error") {
        Some(e) => (
            e.get("details").cloned().unwrap_or(Value::Null),
            Some((
                str_at(e, &["code"]).unwrap_or_default(),
                str_at(e, &["message"]).unwrap_or_default(),
            )),
        ),
        None => (raw.clone(), None),
    };
    let plugin = body.get("plugin").cloned().unwrap_or(Value::Null);
    let capabilities = plugin
        .get("capabilities")
        .and_then(Value::as_object)
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default();
    let declarations = plugin
        .get("compatibility")
        .and_then(|c| c.get("declarations"))
        .and_then(|d| serde_json::from_value::<Vec<Declaration>>(d.clone()).ok())
        .unwrap_or_default();
    PluginValidation {
        exit: out.kind,
        valid: body.get("valid").and_then(Value::as_bool).unwrap_or(false),
        error,
        diagnostics: diagnostics_at(&body, "diagnostics"),
        manifest_family: str_at(&plugin, &["manifest_family"]),
        capabilities,
        summary: str_at(&plugin, &["compatibility", "summary"]),
        declarations,
        raw,
    }
}

/// One row of `plugins approve … --json` → `runtime_capabilities[]`.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct ApprovedCapability {
    pub stable_id: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub trusted_definition_hash: Option<String>,
}

/// Run `plugins approve <selector> --json` (non-interactive; selector is a
/// plugin id, `<pid>:<kind>:<cap>` or a full stable id — plugins.md §9.3).
/// Approval alone is not verification: follow with [`plugins_inspect`] (R14).
pub fn plugins_approve(inv: &Invoker, selector: &str) -> Result<Vec<ApprovedCapability>> {
    let out = inv.run(&["plugins", "approve", selector, "--json"])?;
    if let Some(err) = out.host_reported_error() {
        return Err(err);
    }
    let out = out.expect_ok()?;
    let raw = out.first_json()?;
    raw.get("runtime_capabilities")
        .map(|v| serde_json::from_value::<Vec<ApprovedCapability>>(v.clone()))
        .transpose()
        .map_err(|e| HostError::Parse {
            what: "plugins approve --json runtime_capabilities[]",
            detail: e.to_string(),
        })
        .map(Option::unwrap_or_default)
}

// ---------------------------------------------------------------------------
// config validate / status
// ---------------------------------------------------------------------------

/// The one-line answer of `muse config validate --plane <p> --file <f>`
/// (cli-surface.md §3.11; config-paths.md §7.3 and its verification C5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigValidation {
    /// `valid: plane=defaults schema_version=1`, exit 0.
    Valid {
        plane: String,
        schema_version: Option<u64>,
    },
    /// `field_not_activated: plane=defaults` — the field exists in the schema
    /// but its gate is off in this build; type checks already passed
    /// (`research/experiments/loose-ends.md` §3.4). Exit 1.
    FieldNotActivated { plane: String },
    /// `enterprise_document_invalid | enterprise_document_malformed |
    /// enterprise_schema_unsupported | enterprise_source_unreadable: plane=… reason=… [location=…]`.
    Rejected {
        class: String,
        plane: Option<String>,
        reason: Option<String>,
        location: Option<String>,
        raw: String,
    },
}

impl ConfigValidation {
    /// `valid` or `field_not_activated`.
    pub fn is_accepted(&self) -> bool {
        !matches!(self, ConfigValidation::Rejected { .. })
    }
    /// The `reason=` of a rejection.
    pub fn reason(&self) -> Option<&str> {
        match self {
            ConfigValidation::Rejected { reason, .. } => reason.as_deref(),
            _ => None,
        }
    }
    /// One-line rendering.
    pub fn summary(&self) -> String {
        match self {
            ConfigValidation::Valid { plane, .. } => format!("valid (plane={plane})"),
            ConfigValidation::FieldNotActivated { plane } => {
                format!("field_not_activated (plane={plane})")
            }
            ConfigValidation::Rejected { raw, .. } => raw.clone(),
        }
    }
}

/// Parse the validator's first output line.
pub fn parse_config_validation(line: &str) -> Result<ConfigValidation> {
    let line = line.trim();
    let (class, rest) = line.split_once(':').ok_or_else(|| HostError::Parse {
        what: "config validate output",
        detail: line.to_string(),
    })?;
    let kv = |key: &str| -> Option<String> {
        rest.split_whitespace()
            .find_map(|tok| tok.strip_prefix(&format!("{key}=")))
            .map(str::to_string)
    };
    let plane = kv("plane");
    match class.trim() {
        "valid" => Ok(ConfigValidation::Valid {
            plane: plane.unwrap_or_default(),
            schema_version: kv("schema_version").and_then(|v| v.parse().ok()),
        }),
        "field_not_activated" => Ok(ConfigValidation::FieldNotActivated {
            plane: plane.unwrap_or_default(),
        }),
        // Since 1.3.0-R3057.1 the same verdict wears enterprise clothes:
        // `enterprise_document_invalid: plane=… reason=field_not_activated
        // [location=…]`. The meaning is unchanged (the field exists, its
        // gate is off on this plane, type checks passed), so it parses to
        // the same variant; any other enterprise reason stays rejected.
        c if c.starts_with("enterprise_") => {
            if kv("reason").as_deref() == Some("field_not_activated") {
                return Ok(ConfigValidation::FieldNotActivated {
                    plane: plane.unwrap_or_default(),
                });
            }
            Ok(ConfigValidation::Rejected {
                class: c.to_string(),
                plane,
                reason: kv("reason"),
                location: kv("location"),
                raw: line.to_string(),
            })
        }
        _ => Err(HostError::Parse {
            what: "config validate output",
            detail: line.to_string(),
        }),
    }
}

/// Run `config validate --plane <plane> --file <file>` (no gate needed —
/// config-paths.md verification R7).
pub fn config_validate(inv: &Invoker, plane: &str, file: &Path) -> Result<ConfigValidation> {
    let out = inv.run(&[
        "config".to_string(),
        "validate".to_string(),
        "--plane".to_string(),
        plane.to_string(),
        "--file".to_string(),
        file.to_string_lossy().into_owned(),
    ])?;
    let line = out
        .stdout
        .lines()
        .chain(out.stderr.lines())
        .map(str::trim)
        .find(|l| {
            l.starts_with("valid:")
                || l.starts_with("field_not_activated:")
                || l.starts_with("enterprise_")
        })
        .ok_or_else(|| HostError::Parse {
            what: "config validate output",
            detail: format!(
                "exit {:?}; stdout {:?}; stderr {:?}",
                out.code,
                first_line(&out.stdout),
                first_line(&out.stderr)
            ),
        })?;
    parse_config_validation(line)
}

/// One `plane=… source_class=… state=…` row of `muse config status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnterpriseSource {
    pub plane: String,
    pub source_class: String,
    pub state: String,
}

/// `muse config status` (doctor D9's enterprise probe; no `--json` exists).
#[derive(Clone, Debug)]
pub struct ConfigStatus {
    /// `Generation: sha256:…`.
    pub generation: String,
    pub sources: Vec<EnterpriseSource>,
    pub raw: String,
}

/// Run `config status`.
pub fn config_status(inv: &Invoker) -> Result<ConfigStatus> {
    let out = inv.run(&["config", "status"])?.expect_ok()?;
    parse_config_status(&out.stdout)
}

/// Parse `config status` text.
pub fn parse_config_status(text: &str) -> Result<ConfigStatus> {
    let generation = text
        .lines()
        .find_map(|l| l.trim().strip_prefix("Generation:"))
        .map(|g| g.trim().to_string())
        .ok_or_else(|| HostError::Parse {
            what: "config status",
            detail: "no Generation: line".to_string(),
        })?;
    let sources = text
        .lines()
        .filter(|l| l.trim().starts_with("plane="))
        .map(|l| {
            let kv = |key: &str| {
                l.split_whitespace()
                    .find_map(|tok| tok.strip_prefix(&format!("{key}=")))
                    .unwrap_or("")
                    .to_string()
            };
            EnterpriseSource {
                plane: kv("plane"),
                source_class: kv("source_class"),
                state: kv("state"),
            }
        })
        .collect();
    Ok(ConfigStatus {
        generation,
        sources,
        raw: text.to_string(),
    })
}

// ---------------------------------------------------------------------------
// schema exports
// ---------------------------------------------------------------------------

/// Which export to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaKind {
    /// `generate-json-schema` → `manifest.json` + `msp.schema.json`.
    JsonSchema,
    /// `generate-ts` → `msp.d.ts`.
    TypeScript,
}

/// `manifest.json` of a JSON-schema export (msp-protocol.md §2.3).
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct SchemaManifest {
    pub experimental: bool,
    pub fingerprint: String,
    #[serde(rename = "schemaVersion")]
    pub schema_version: u64,
}

/// The result of one `muse schema …` export.
#[derive(Clone, Debug)]
pub struct SchemaExport {
    pub kind: SchemaKind,
    pub experimental: bool,
    pub out_dir: PathBuf,
    pub manifest: Option<SchemaManifest>,
    /// `(file name, sha256)` for every emitted file.
    pub files: Vec<(String, String)>,
}

impl SchemaExport {
    /// The sha256 of one emitted file.
    pub fn sha256_of(&self, name: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, s)| s.as_str())
    }
}

/// Run `schema generate-json-schema|generate-ts --out <dir> [--experimental]`.
pub fn schema_export(
    inv: &Invoker,
    kind: SchemaKind,
    experimental: bool,
    out_dir: &Path,
) -> Result<SchemaExport> {
    let verb = match kind {
        SchemaKind::JsonSchema => "generate-json-schema",
        SchemaKind::TypeScript => "generate-ts",
    };
    let mut argv = vec![
        "schema".to_string(),
        verb.to_string(),
        "--out".to_string(),
        out_dir.to_string_lossy().into_owned(),
    ];
    if experimental {
        argv.push("--experimental".to_string());
    }
    inv.run(&argv)?.expect_ok()?;
    let mut files = Vec::new();
    let mut manifest = None;
    let mut names: Vec<PathBuf> = std::fs::read_dir(out_dir)
        .map_err(|e| HostError::io("read dir", out_dir, e))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    names.sort();
    for path in names {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name == "manifest.json" {
            let text = fsx::read_to_string(&path)?;
            manifest = Some(serde_json::from_str::<SchemaManifest>(&text).map_err(|e| {
                HostError::Parse {
                    what: "schema manifest.json",
                    detail: e.to_string(),
                }
            })?);
        }
        files.push((name, fsx::sha256_file(&path)?));
    }
    Ok(SchemaExport {
        kind,
        experimental,
        out_dir: out_dir.to_path_buf(),
        manifest,
        files,
    })
}

/// Method / notification / error counts of an `msp.schema.json`
/// (`methods` and `notifications` are objects, `errors` is an array —
/// verified live; msp-protocol.md §1 gives 31 / 23 / 29).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaCounts {
    pub methods: usize,
    pub notifications: usize,
    pub errors: usize,
}

/// Count the MSP surface from an exported schema bundle.
pub fn schema_counts(schema_json: &Path) -> Result<SchemaCounts> {
    let text = fsx::read_to_string(schema_json)?;
    let v: Value = serde_json::from_str(&text).map_err(|e| HostError::Parse {
        what: "msp.schema.json",
        detail: e.to_string(),
    })?;
    let len = |key: &str| -> usize {
        match v.get(key) {
            Some(Value::Object(m)) => m.len(),
            Some(Value::Array(a)) => a.len(),
            _ => 0,
        }
    };
    Ok(SchemaCounts {
        methods: len("methods"),
        notifications: len("notifications"),
        errors: len("errors"),
    })
}

// ---------------------------------------------------------------------------
// echo session → session.jsonl
// ---------------------------------------------------------------------------

/// One `run_context_messages[]` entry of the `model_request_configured` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextBlock {
    pub order: u64,
    pub id: String,
    pub source: String,
    pub bytes: usize,
    pub text: String,
}

/// What one `session.jsonl` says about the composed run context.
#[derive(Clone, Debug, Default)]
pub struct SessionFacts {
    pub context_blocks: Vec<ContextBlock>,
    pub active_tools: Vec<String>,
    /// `toolset.source`: `default` or `settings` (config-paths.md M3).
    pub toolset_source: Option<String>,
    /// `frame_schema_version` of the retained-frame wrapper record.
    pub frame_schema_version: Option<u64>,
    /// `schema_version` values seen on records.
    pub record_schema_versions: BTreeSet<u64>,
    /// Lines in the log.
    pub records: usize,
}

impl SessionFacts {
    /// Orders of the context blocks, ascending as composed.
    pub fn orders(&self) -> Vec<u64> {
        self.context_blocks.iter().map(|b| b.order).collect()
    }
    /// Look up a block by its order.
    pub fn block(&self, order: u64) -> Option<&ContextBlock> {
        self.context_blocks.iter().find(|b| b.order == order)
    }
}

/// Parse a `session.jsonl`. The first line is a `retained_frame` wrapper whose
/// `children[].record_json` are stringified records; the remaining lines are
/// records; the `model_request_configured` event sits at
/// `payload.event` (verified live on 1.0.1-R2006.1).
pub fn parse_session_log(path: &Path) -> Result<SessionFacts> {
    let text = fsx::read_to_string(path)?;
    let mut facts = SessionFacts::default();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        facts.records += 1;
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if v.get("retained_frame").is_some() {
            if let Some(f) = v.get("frame_schema_version").and_then(Value::as_u64) {
                facts.frame_schema_version = Some(f);
            }
            if let Some(children) = v.get("children").and_then(Value::as_array) {
                for child in children {
                    if let Some(Ok(rec)) = child
                        .get("record_json")
                        .and_then(Value::as_str)
                        .map(serde_json::from_str::<Value>)
                    {
                        absorb_record(&mut facts, &rec);
                    }
                }
            }
            continue;
        }
        absorb_record(&mut facts, &v);
    }
    Ok(facts)
}

fn absorb_record(facts: &mut SessionFacts, rec: &Value) {
    if let Some(s) = rec.get("schema_version").and_then(Value::as_u64) {
        facts.record_schema_versions.insert(s);
    }
    let Some(event) = rec.get("payload").and_then(|p| p.get("event")) else {
        return;
    };
    if event.get("kind").and_then(Value::as_str) != Some(hr::MODEL_REQUEST_CONFIGURED_EVENT) {
        return;
    }
    if let Some(msgs) = event.get("run_context_messages").and_then(Value::as_array) {
        facts.context_blocks = msgs
            .iter()
            .map(|m| {
                let text = m
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                ContextBlock {
                    order: m.get("order").and_then(Value::as_u64).unwrap_or(0),
                    id: m
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    source: m
                        .get("source")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    bytes: text.len(),
                    text,
                }
            })
            .collect();
    }
    if let Some(ts) = event.get("toolset") {
        facts.toolset_source = ts.get("source").and_then(Value::as_str).map(str::to_string);
        facts.active_tools = ts
            .get("active_tools")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
    }
}

/// The newest `session.jsonl` under a sessions dir (`YYYY/MM/DD/<uuid>/session.jsonl`).
pub fn newest_session_log(sessions_dir: &Path) -> Result<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in walkdir::WalkDir::new(sessions_dir)
        .max_depth(5)
        .into_iter()
        .flatten()
    {
        if entry.file_type().is_file() && entry.file_name() == "session.jsonl" {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if best.as_ref().map(|(t, _)| modified >= *t).unwrap_or(true) {
                best = Some((modified, entry.into_path()));
            }
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| {
        HostError::Probe(format!("no session.jsonl under {}", sessions_dir.display()))
    })
}

/// Options for [`echo_session`]. There is no `Default`: a workspace is always
/// chosen deliberately, because in a trusted workspace the host composes
/// whatever lies there — `AGENTS.md` root→cwd, `.agents/skills` one level
/// deep, `.muse/hooks.json` (`docs/host-reality.md` Paths) — into the
/// measurement. The shared system temp dir is never a workspace.
#[derive(Clone, Debug)]
pub struct EchoSessionOptions {
    /// The workspace to run in (must exist).
    pub workspace: PathBuf,
    /// Pass `--trust-workspace`. The order-186 `subagent_delegation` block and
    /// the six `subagent_*` tools compose only in a trusted workspace
    /// (measured 2026-09-02 on 1.0.1-R2006.1: untrusted → 7 blocks / 20 tools,
    /// trusted → 8 / 26; `docs/experiments/context-slimming.md` §7.2);
    /// `docs/host-reality.md` P0/P1 rows assume trusted. Trust also turns on
    /// the workspace composition described on the struct.
    pub trust_workspace: bool,
    /// The prompt (`hi`).
    pub prompt: String,
    /// Also run `muse export --last` and record `export_schema_version`.
    pub with_export: bool,
    /// Keeps a probe-owned workspace alive for as long as the options live.
    owned: Option<Arc<tempfile::TempDir>>,
}

impl EchoSessionOptions {
    /// An existing workspace, UNTRUSTED (nothing in it composes), prompt `hi`,
    /// no export.
    pub fn in_workspace(workspace: &Path) -> Self {
        EchoSessionOptions {
            workspace: workspace.to_path_buf(),
            trust_workspace: false,
            prompt: "hi".to_string(),
            with_export: false,
            owned: None,
        }
    }
    /// A fresh, empty, probe-owned temp workspace, TRUSTED — the host-reality
    /// measurement (nothing can compose from an empty directory). Removed when
    /// the last clone of the options is dropped.
    pub fn fresh_workspace() -> Result<Self> {
        let dir = tempdir("omm-echo-ws-")?;
        Ok(EchoSessionOptions {
            workspace: dir.path().to_path_buf(),
            trust_workspace: true,
            prompt: "hi".to_string(),
            with_export: false,
            owned: Some(Arc::new(dir)),
        })
    }
    /// Set `--trust-workspace`.
    pub fn trusted(mut self, on: bool) -> Self {
        self.trust_workspace = on;
        self
    }
    /// Set the prompt.
    pub fn prompt(mut self, prompt: &str) -> Self {
        self.prompt = prompt.to_string();
        self
    }
    /// Also run `muse export --last`.
    pub fn with_export(mut self, on: bool) -> Self {
        self.with_export = on;
        self
    }
    /// True when the workspace is a probe-owned temp directory.
    pub fn owns_workspace(&self) -> bool {
        self.owned.is_some()
    }
}

/// The result of one echo session.
#[derive(Clone, Debug)]
pub struct EchoSession {
    pub facts: SessionFacts,
    pub export_schema_version: Option<u64>,
    pub exec: Outcome,
}

/// Run `muse exec --provider echo [--trust-workspace] <prompt>` with the data
/// root redirected to a temp dir (the session log never lands in the user's
/// store), then parse the newest `session.jsonl`. The config root is whatever
/// the invoker carries, so the user's own skills/rules compose when wanted.
/// No prompt is ever sent anywhere: the echo provider never leaves the process.
pub fn echo_session(inv: &Invoker, opts: &EchoSessionOptions) -> Result<EchoSession> {
    let tmp = tempdir("omm-echo-")?;
    let data = tmp.path().join("data");
    fsx::create_dir_all(&data)?;
    let probe = inv.clone().data_home(&data).cwd(&opts.workspace);
    let mut argv = vec![
        "exec".to_string(),
        "--provider".to_string(),
        "echo".to_string(),
    ];
    if opts.trust_workspace {
        argv.push("--trust-workspace".to_string());
    }
    argv.push(opts.prompt.clone());
    let exec = probe.run(&argv)?;
    if !exec.ok() {
        return Err(HostError::Command {
            argv: exec.argv_string(),
            code: exec.code,
            stderr: first_line(&exec.stderr).to_string(),
        });
    }
    let sessions = data.join("muse").join("sessions");
    let log = newest_session_log(&sessions)?;
    let facts = parse_session_log(&log)?;
    let export_schema_version = if opts.with_export {
        let out_file = tmp.path().join("export.json");
        Some(export_last(&probe, &out_file)?.export_schema_version)
    } else {
        None
    };
    Ok(EchoSession {
        facts,
        export_schema_version,
        exec,
    })
}

/// The top of a `muse export` document (cli-surface.md §3.9).
#[derive(Clone, Debug, Deserialize)]
pub struct ExportDoc {
    pub export_schema_version: u64,
    #[serde(default)]
    pub redaction: Option<String>,
}

/// Run `export --last --out <file>` for the invoker's cwd/workspace.
pub fn export_last(inv: &Invoker, out_file: &Path) -> Result<ExportDoc> {
    inv.run(&[
        "export".to_string(),
        "--last".to_string(),
        "--out".to_string(),
        out_file.to_string_lossy().into_owned(),
    ])?
    .expect_ok()?;
    let text = fsx::read_to_string(out_file)?;
    serde_json::from_str::<ExportDoc>(&text).map_err(|e| HostError::Parse {
        what: "export document",
        detail: e.to_string(),
    })
}

// ---------------------------------------------------------------------------
// settings load probe
// ---------------------------------------------------------------------------

/// What the host's own settings loader said about a candidate document.
#[derive(Clone, Debug)]
pub struct LoadProbe {
    pub accepted: bool,
    /// The first stderr line on rejection (`malformed settings file …` /
    /// `unsupported settings schema version N …`).
    pub detail: String,
}

/// Feed a candidate `settings.json` to a settings-consuming read lane
/// (`skills list --source user --json`) in a throwaway config root. Malformed
/// settings are a *lazy* failure: `--version`/`--help`/`export`/`init`/
/// `workflows list` never read the file, `skills list` does
/// (`research/experiments/loose-ends.md` §1.4). Read lanes are silent on the
/// `mcpServers`/`mcp_servers` collision, which callers must check themselves.
pub fn settings_load_probe(inv: &Invoker, candidate: &[u8]) -> Result<LoadProbe> {
    let tmp = tempdir("omm-settings-probe-")?;
    let sb = Sandbox::create(tmp.path())?;
    let cfg = sb.config_home.join("muse");
    fsx::create_dir_all(&cfg)?;
    std::fs::write(cfg.join("settings.json"), candidate)
        .map_err(|e| HostError::io("write", cfg.join("settings.json"), e))?;
    let out = inv
        .clone()
        .sandboxed(&sb)
        .run(&["skills", "list", "--source", "user", "--json"])?;
    let accepted = out.ok()
        && out
            .first_json()
            .map(|v| v.get("skills").is_some())
            .unwrap_or(false);
    Ok(LoadProbe {
        accepted,
        detail: if accepted {
            String::new()
        } else {
            let line = first_line(&out.stderr);
            if line.is_empty() {
                format!("exit {:?}, no diagnostic", out.code)
            } else {
                line.to_string()
            }
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_line_parses() {
        let v = parse_version("Muse Code 1.0.1 (1.0.1-R2006.1)").unwrap();
        assert_eq!(v.product, "Muse Code");
        assert_eq!(v.version, "1.0.1");
        assert_eq!(v.build, "1.0.1-R2006.1");
        assert!(parse_version("garbage").is_err());
    }

    #[test]
    fn gate_lines_parse() {
        let text = "2026-09-01T14:25:38Z INFO tbh.local.bootstrap x.rs:1 event=\"gate.resolve\" gate=\"workflow_tool\" enabled=true source=\"default\"\n\
                    junk line\n\
                    … event=\"gate.resolve\" gate=\"plugins\" enabled=true source=\"override\"\n\
                    … event=\"gate.resolve\" gate=\"voice\" enabled=false source=\"default\"\n";
        let g = parse_gate_lines(text);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].id, "workflow_tool");
        assert!(g[0].enabled);
        assert_eq!(g[1].source, GateSource::Override);
        assert!(!g[2].enabled);
        let report = GateReport {
            gates: g,
            ..Default::default()
        };
        assert_eq!(report.default_on_ids(), vec!["workflow_tool"]);
    }

    const HEAD: &str = "2026-09-01T21:18:43.659949Z INFO tbh.local.host x.rs:45 event=\"startup\" host=\"cli\"\n\
        2026-09-01T21:18:43.660019Z INFO tbh.local.bootstrap x.rs:78 event=\"process.identity\" mode=\"plugin_mutation\"\n\
        2026-09-01T21:18:43.660031Z INFO tbh.local.bootstrap x.rs:453 event=\"path.resolved\" kind=\"config_root\" source=\"xdg\" state=\"present\"\n\
        2026-09-01T21:18:43.660040Z INFO tbh.local.config x.rs:47 event=\"feature_config.cache\" state=\"missing\" gate_count=0\n";
    const GATES: &str =
        "… event=\"gate.resolve\" gate=\"workflow_tool\" enabled=true source=\"default\"\n\
        … event=\"gate.resolve\" gate=\"plugins\" enabled=true source=\"override\"\n\
        … event=\"gate.resolve\" gate=\"voice\" enabled=false source=\"default\"\n";
    const TAIL: &str = "2026-09-01T21:18:43.660278Z INFO tbh.local.security x.rs:16 event=\"trust.resolve\" outcome=\"untrusted\" source=\"missing_store\" duration_ms=0\n";

    #[test]
    fn truncated_bootstrap_traces_are_never_a_gate_report() {
        // Complete: head + table + tail.
        let full = format!("{HEAD}{GATES}{TAIL}");
        let r = parse_gate_trace(&full).unwrap();
        assert_eq!(r.gates.len(), 3);
        assert_eq!(r.trace_lines, 8);
        assert_eq!(r.attempts, 1);
        assert_eq!(
            r.feature_config,
            Some(FeatureConfigCache {
                state: "missing".into(),
                gate_count: 0
            })
        );
        assert_eq!(
            parse_feature_config_line(
                "x event=\"feature_config.cache\" state=\"present\" gate_count=7"
            ),
            Some(FeatureConfigCache {
                state: "present".into(),
                gate_count: 7
            })
        );
        assert_eq!(parse_feature_config_line(GATES), None);
        // Head-truncated (starts mid-stream at a gate line): rejected even
        // though gate lines and the tail are present.
        let head_cut = format!("{GATES}{TAIL}");
        let shape = inspect_gate_trace(&head_cut);
        assert!(!shape.head && shape.tail && shape.gates.len() == 3);
        match parse_gate_trace(&head_cut) {
            Err(HostError::Probe(msg)) => assert!(msg.contains("truncated"), "{msg}"),
            other => panic!("{other:?}"),
        }
        // Tail-truncated (the writer stopped inside or right after the table).
        let tail_cut = format!("{HEAD}{GATES}");
        assert!(!inspect_gate_trace(&tail_cut).complete());
        assert!(parse_gate_trace(&tail_cut).is_err());
        // Empty log, head only, and a tail that precedes the gates.
        assert!(parse_gate_trace("").is_err());
        assert!(parse_gate_trace(HEAD).is_err());
        assert!(parse_gate_trace(&format!("{HEAD}{TAIL}{GATES}")).is_err());
    }

    #[test]
    fn gate_probe_backoff_doubles_to_the_cap_with_jitter_under_the_deadline() {
        let cap = GATE_PROBE_BACKOFF_CAP.as_millis() as u64;
        let mut schedule_ms = 0u64;
        for attempt in 1..=GATE_PROBE_MAX_ATTEMPTS {
            let base = (GATE_PROBE_BACKOFF_MIN.as_millis() as u64)
                .saturating_mul(1u64 << u64::from(attempt.saturating_sub(1).min(16)))
                .min(cap);
            let d = gate_probe_backoff(attempt).as_millis() as u64;
            assert!(
                d >= base / 2 && d <= base + base / 2,
                "attempt {attempt}: {d} ms around base {base}"
            );
            if attempt <= 13 {
                schedule_ms += base;
            }
        }
        assert!(gate_probe_backoff(1).as_millis() <= 150);
        assert!(
            gate_probe_backoff(6).as_millis() >= 1000,
            "2^5 × 100 ms ± 50 %"
        );
        assert!(gate_probe_backoff(40).as_millis() <= 3000, "capped");
        // The schedule reaches the deadline before the attempt ceiling.
        assert!(
            schedule_ms >= 15_000,
            "13 backoffs sum to {schedule_ms} ms, short of the deadline"
        );
        assert!(GATE_PROBE_DEADLINE < crate::invoke::RUN_TIMEOUT_DEFAULT);
        assert!(GATE_PROBE_DEADLINE >= std::time::Duration::from_secs(15));
    }

    #[cfg(unix)]
    #[test]
    fn gate_probe_retries_until_the_deadline_and_says_how_long_it_tried() {
        // A fake host that never writes a trace: every attempt is a
        // truncation, and the probe must keep trying until the deadline —
        // not stop after a fixed handful of attempts — then report the
        // attempt count and the time spent. A 1.5 s deadline exercises the
        // same loop as the 20 s one (`gates` is `gates_within(GATE_PROBE_DEADLINE)`).
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("muse-bin-fake");
        std::fs::write(&bin, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let deadline = std::time::Duration::from_millis(1500);
        let started = std::time::Instant::now();
        let err = gates_within(&Invoker::new(&bin), deadline).unwrap_err();
        let elapsed = started.elapsed();
        let msg = err.to_string();
        assert!(msg.contains("attempt(s) in"), "{msg}");
        assert!(msg.contains("deadline 1.5 s"), "{msg}");
        assert!(msg.contains("truncated bootstrap trace"), "{msg}");
        let attempts: u32 = msg
            .split("no complete bootstrap trace after ")
            .nth(1)
            .and_then(|s| s.split_whitespace().next())
            .and_then(|n| n.parse().ok())
            .unwrap();
        // 100 + 200 + 400 ms of backoff (−50 % jitter at worst) fit four
        // attempts into 1.5 s; the old loop stopped on a count, this one on
        // the clock.
        assert!(attempts >= 4, "{attempts} attempts in {elapsed:?}: {msg}");
        assert!(attempts <= GATE_PROBE_MAX_ATTEMPTS);
        assert!(
            elapsed >= deadline && elapsed < deadline + std::time::Duration::from_secs(5),
            "tried for {elapsed:?}"
        );
    }

    #[test]
    fn fresh_echo_workspace_is_empty_owned_and_removed_on_drop() {
        let a = EchoSessionOptions::fresh_workspace().unwrap();
        let b = EchoSessionOptions::fresh_workspace().unwrap();
        assert!(a.owns_workspace() && a.trust_workspace);
        assert_ne!(a.workspace, b.workspace);
        assert_ne!(a.workspace, std::env::temp_dir());
        assert!(a.workspace.is_dir());
        assert_eq!(a.workspace.read_dir().unwrap().count(), 0, "empty");
        let path = a.workspace.clone();
        let clone = a.clone();
        drop(a);
        assert!(path.exists(), "a clone keeps the workspace alive");
        drop(clone);
        assert!(!path.exists(), "removed with the last clone");
        let explicit = EchoSessionOptions::in_workspace(&b.workspace);
        assert!(!explicit.trust_workspace && !explicit.owns_workspace());
    }

    #[test]
    fn config_validation_lines_parse() {
        assert_eq!(
            parse_config_validation("valid: plane=defaults schema_version=1").unwrap(),
            ConfigValidation::Valid {
                plane: "defaults".into(),
                schema_version: Some(1)
            }
        );
        assert!(matches!(
            parse_config_validation("field_not_activated: plane=defaults").unwrap(),
            ConfigValidation::FieldNotActivated { .. }
        ));
        // Since 1.3.0-R3057.1 the same verdict arrives in enterprise shape.
        assert!(matches!(
            parse_config_validation(
                "enterprise_document_invalid: plane=defaults reason=field_not_activated location=settings.tui.theme"
            )
            .unwrap(),
            ConfigValidation::FieldNotActivated { .. }
        ));
        match parse_config_validation(
            "enterprise_document_invalid: plane=defaults reason=wrong_type location=settings.provider",
        )
        .unwrap()
        {
            ConfigValidation::Rejected {
                reason, location, ..
            } => {
                assert_eq!(reason.as_deref(), Some("wrong_type"));
                assert_eq!(location.as_deref(), Some("settings.provider"));
            }
            other => panic!("{other:?}"),
        }
        assert!(parse_config_validation("nonsense").is_err());
    }

    #[test]
    fn config_status_parses() {
        let s = parse_config_status(
            "Enterprise configuration status\nGeneration: sha256:abc\nSources:\n  plane=defaults source_class=system_file state=absent\n  plane=policy source_class=system_file state=absent\n",
        )
        .unwrap();
        assert_eq!(s.generation, "sha256:abc");
        assert_eq!(s.sources.len(), 2);
        assert_eq!(s.sources[1].plane, "policy");
    }

    #[test]
    fn plugin_validation_shapes() {
        let ok = Outcome {
            argv: vec![],
            code: Some(0),
            kind: OutcomeKind::Ok,
            stdout: String::new(),
            stderr: String::new(),
            dropped: Default::default(),
        };
        let raw = serde_json::json!({
            "valid": true, "diagnostics": [],
            "plugin": {"manifest_family": "native",
                        "capabilities": {"skills": [], "hooks": [], "mcp_servers": [], "commands": [], "reminders": []},
                        "compatibility": {"summary": "full", "declarations": [{"id": "skill:x", "kind": "skill", "classification": "supported"}]}}
        });
        let v = parse_plugin_validation(&ok, raw);
        assert!(v.passes(), "{:?}", v.four_predicates());
        assert_eq!(v.capabilities.len(), 5);

        let bad = Outcome {
            code: Some(1),
            kind: OutcomeKind::RunFailed,
            ..ok
        };
        let raw = serde_json::json!({"error": {"code": "invalid-manifest-schema", "message": "plugin manifest must declare schemaVersion 1",
            "details": {"valid": false, "diagnostics": [], "plugin": {"compatibility": {}}}}});
        let v = parse_plugin_validation(&bad, raw);
        assert!(!v.valid);
        assert_eq!(
            v.error.as_ref().map(|e| e.0.as_str()),
            Some("invalid-manifest-schema")
        );
        assert!(!v.passes());
    }

    #[test]
    fn session_log_parses_frames_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("session.jsonl");
        let inner = serde_json::json!({"schema_version": 1, "payload": {"kind": "run", "event": {"kind": "other"}}}).to_string();
        let frame = serde_json::json!({"retained_frame": "x", "frame_schema_version": 1, "children": [{"child_index": 0, "record_json": inner}]});
        let event = serde_json::json!({"schema_version": 1, "payload": {"kind": "run", "event": {
            "kind": "model_request_configured",
            "toolset": {"source": "default", "mode": "all", "active_tools": ["a", "b"]},
            "run_context_messages": [{"order": 85, "id": "workspace_identity", "source": "workspace_identity", "text": "xyz"}]
        }}});
        std::fs::write(&log, format!("{frame}\n{event}\n")).unwrap();
        let facts = parse_session_log(&log).unwrap();
        assert_eq!(facts.frame_schema_version, Some(1));
        assert_eq!(facts.orders(), vec![85]);
        assert_eq!(facts.block(85).unwrap().bytes, 3);
        assert_eq!(facts.active_tools, vec!["a", "b"]);
        assert_eq!(facts.record_schema_versions.len(), 1);
        assert_eq!(newest_session_log(dir.path()).unwrap(), log);
    }
}
