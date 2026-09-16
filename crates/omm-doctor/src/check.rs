//! The `Check` / `Report` types and the shared [`Context`] every check runs
//! over (ARCHITECTURE.md §6): the host invoker, the roots, the ledger, and
//! the probes several checks share — `settings.json`, `plugins list --json`,
//! `plugins inspect --json` and one live echo session — each taken at most
//! once per run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use omm_host::host_reality as hr;
use omm_host::probe::{self, PluginInspect};
use omm_host::settings::SettingsDoc;
use omm_host::{HostError, Invoker, Roots};

use crate::error::Result;
use crate::ledger::{self, LedgerState};
use crate::session::{self, LiveOptions, LiveSession, SeedOptions};

/// How bad a finding is. `Info` is a pass or an informational row; any
/// `Critical` makes `omm doctor` exit 1 ([`Report::exit_code`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Critical,
}

impl Severity {
    /// The four-letter tag of the human report.
    pub fn tag(self) -> &'static str {
        match self {
            Severity::Info => "ok  ",
            Severity::Warn => "WARN",
            Severity::Critical => "CRIT",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Severity::Info => "info",
            Severity::Warn => "warn",
            Severity::Critical => "critical",
        })
    }
}

/// One row of the report. `why_silent` states why the host gives no
/// session-time signal for this condition — the reason the check exists.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub title: String,
    pub severity: Severity,
    pub observed: String,
    pub why_silent: String,
    /// The exact command that fixes it; `None` on a pass or an informational row.
    pub fix: Option<String>,
}

impl Check {
    /// A pass or an informational row.
    pub fn info(id: &str, title: &str, observed: impl Into<String>, why: &str) -> Check {
        Check {
            id: id.to_string(),
            title: title.to_string(),
            severity: Severity::Info,
            observed: observed.into(),
            why_silent: why.to_string(),
            fix: None,
        }
    }
    /// A warning with its fix.
    pub fn warn(
        id: &str,
        title: &str,
        observed: impl Into<String>,
        why: &str,
        fix: impl Into<String>,
    ) -> Check {
        Check {
            id: id.to_string(),
            title: title.to_string(),
            severity: Severity::Warn,
            observed: observed.into(),
            why_silent: why.to_string(),
            fix: Some(fix.into()),
        }
    }
    /// A critical finding with its fix.
    pub fn critical(
        id: &str,
        title: &str,
        observed: impl Into<String>,
        why: &str,
        fix: impl Into<String>,
    ) -> Check {
        Check {
            id: id.to_string(),
            title: title.to_string(),
            severity: Severity::Critical,
            observed: observed.into(),
            why_silent: why.to_string(),
            fix: Some(fix.into()),
        }
    }
    /// Anything above `Info`.
    pub fn failed(&self) -> bool {
        self.severity != Severity::Info
    }
}

/// What the report was measured against.
#[derive(Clone, Debug, Serialize)]
pub struct HostInfo {
    pub binary: PathBuf,
    /// `muse --version`, informational only (R15).
    pub version: Option<String>,
    pub config_root: PathBuf,
    pub data_root: PathBuf,
    pub omm_root: PathBuf,
    pub plugin_id: String,
    pub workspace: Option<PathBuf>,
}

/// The whole run.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub host: HostInfo,
    pub checks: Vec<Check>,
    /// No `critical` row.
    pub ok: bool,
    pub elapsed_ms: u128,
}

impl Report {
    /// Build, computing `ok`.
    pub fn new(host: HostInfo, checks: Vec<Check>, elapsed_ms: u128) -> Report {
        let ok = !checks.iter().any(|c| c.severity == Severity::Critical);
        Report {
            host,
            checks,
            ok,
            elapsed_ms,
        }
    }
    /// `1` on any critical row, else `0`.
    pub fn exit_code(&self) -> i32 {
        if self.ok {
            0
        } else {
            1
        }
    }
    /// Rows above `Info`.
    pub fn failures(&self) -> Vec<&Check> {
        self.checks.iter().filter(|c| c.failed()).collect()
    }
    /// One row by id (`D3`).
    pub fn get(&self, id: &str) -> Option<&Check> {
        self.checks.iter().find(|c| c.id == id)
    }
}

/// Tunables of a run.
#[derive(Clone, Debug)]
pub struct Options {
    /// Run the live echo session (D8; `omm cost` always does).
    pub live: bool,
    /// Run the full P0+P1 host self-test for D11 (about two seconds).
    pub host_drift: bool,
    /// D7 warns from this many enabled plugins (`hr::ENABLED_PLUGINS_WARN_AT`).
    pub plugin_count_warn_at: usize,
    /// D7's hard ceiling (`hr::ENABLED_PLUGINS_MAX`).
    pub plugin_count_max: usize,
    /// What the live session's throwaway data root is seeded with.
    pub seed: SeedOptions,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            live: true,
            host_drift: true,
            plugin_count_warn_at: hr::ENABLED_PLUGINS_WARN_AT,
            plugin_count_max: hr::ENABLED_PLUGINS_MAX,
            seed: SeedOptions::default(),
        }
    }
}

/// Why `plugins inspect <id>` gave no answer.
#[derive(Clone, Debug)]
pub enum InspectFailure {
    /// `unknown-plugin` — the plugin is not installed.
    NotInstalled(String),
    /// Anything else (the host failed, the JSON did not parse, …).
    Other(String),
}

/// One runtime capability a package declares (`hooks`, `mcpServers`,
/// `reminders` — the three families that need approval; `plugins.md` §9.3).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DeclaredCapability {
    /// `hook | mcp_server | reminder`, as the stable id spells it.
    pub kind: String,
    pub id: String,
    /// `plugin:<pid>:<kind>:<id>` (host-reality "Trust lifecycle": approve).
    pub stable_id: String,
    /// The manifest's `enabledDefault` (host-reality.md "Identity
    /// constraints": defaults true, only a literal `false` disables). The
    /// host's `plugins inspect` / `plugins list` projection of a capability
    /// drops the field (measured 2026-09-02 on 1.0.1-R2006.1: a reminder row
    /// carries `id path source_path tools blocking …`, never
    /// `enabledDefault`), so it is read from the cached package manifest
    /// (`record.cache_path/<manifest dir>/plugin.json`) by
    /// [`Context::declared_capabilities`]; `true` when that cannot be read.
    pub enabled_default: bool,
}

/// The validator/inspect spelling of each approval-gated family and the
/// stable-id kind it maps to (`plugins.md` §9.3; `plugins approve --json`
/// output measured 2026-09-02).
const RUNTIME_FAMILIES: [(&str, &str); 3] = [
    ("hooks", "hook"),
    ("mcp_servers", "mcp_server"),
    ("reminders", "reminder"),
];

/// The same families as the native manifest spells them
/// (`capabilities.{hooks,mcpServers,reminders}`; host-reality.md "plugin
/// capability families").
const MANIFEST_FAMILIES: [(&str, &str); 3] = [
    ("hooks", "hook"),
    ("mcpServers", "mcp_server"),
    ("reminders", "reminder"),
];

/// `stable id → enabledDefault` from a native package manifest document
/// (`plugin.json`); entries without the field are `true` (the host's default).
pub fn manifest_enabled_defaults(manifest: &Value, plugin_id: &str) -> BTreeMap<String, bool> {
    let mut out = BTreeMap::new();
    let Some(caps) = manifest.get("capabilities").and_then(Value::as_object) else {
        return out;
    };
    for (family, kind) in MANIFEST_FAMILIES {
        if let Some(rows) = caps.get(family).and_then(Value::as_array) {
            for row in rows {
                if let Some(id) = row.get("id").and_then(Value::as_str) {
                    let enabled = row
                        .get("enabledDefault")
                        .and_then(Value::as_bool)
                        .unwrap_or(true);
                    out.insert(stable_id(plugin_id, kind, id), enabled);
                }
            }
        }
    }
    out
}

/// `plugin:<pid>:<kind>:<cap>`.
pub fn stable_id(plugin_id: &str, kind: &str, cap: &str) -> String {
    format!("plugin:{plugin_id}:{kind}:{cap}")
}

/// The approval-gated capabilities a `plugin` document (`plugins inspect` /
/// `plugins list` → `plugin.capabilities`) declares. Present even after
/// `plugins disable` deleted every runtime-capability line (measured
/// 2026-09-02), which is what lets D1 name what is missing.
pub fn declared_capabilities(plugin: &Value, plugin_id: &str) -> Vec<DeclaredCapability> {
    let mut out = Vec::new();
    let Some(caps) = plugin.get("capabilities").and_then(Value::as_object) else {
        return out;
    };
    for (family, kind) in RUNTIME_FAMILIES {
        if let Some(rows) = caps.get(family).and_then(Value::as_array) {
            for row in rows {
                if let Some(id) = row.get("id").and_then(Value::as_str) {
                    out.push(DeclaredCapability {
                        kind: kind.to_string(),
                        id: id.to_string(),
                        stable_id: stable_id(plugin_id, kind, id),
                        enabled_default: true,
                    });
                }
            }
        }
    }
    out
}

/// One row of `plugins list --json` (shape measured 2026-09-02 on
/// 1.0.1-R2006.1: `{record:{id,enabled,manifest_family,package_sha256,
/// source.path,cache_path}, plugin:{capabilities:{…}}, valid, active}`).
#[derive(Clone, Debug, Serialize)]
pub struct InstalledPlugin {
    pub id: String,
    pub enabled: bool,
    pub active: bool,
    pub valid: bool,
    pub manifest_family: Option<String>,
    pub package_sha256: Option<String>,
    pub source_path: Option<PathBuf>,
    pub cache_path: Option<PathBuf>,
    pub declared: Vec<DeclaredCapability>,
}

/// `plugins list --json`.
#[derive(Clone, Debug)]
pub struct PluginsList {
    pub plugins: Vec<InstalledPlugin>,
    pub raw: Value,
}

impl PluginsList {
    /// Enabled plugins (`record.enabled == true`).
    pub fn enabled(&self) -> Vec<&InstalledPlugin> {
        self.plugins.iter().filter(|p| p.enabled).collect()
    }
    /// One plugin by id.
    pub fn get(&self, id: &str) -> Option<&InstalledPlugin> {
        self.plugins.iter().find(|p| p.id == id)
    }
}

/// Run `plugins list --json` (the plugins gate is added by the invoker).
pub fn plugins_list(inv: &Invoker) -> Result<PluginsList> {
    let out = inv.run(&["plugins", "list", "--json"])?.expect_ok()?;
    let raw = out.first_json()?;
    let plugins = raw
        .get("plugins")
        .and_then(Value::as_array)
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    let record = row.get("record")?;
                    let id = record.get("id")?.as_str()?.to_string();
                    let plugin = row.get("plugin").cloned().unwrap_or(Value::Null);
                    Some(InstalledPlugin {
                        declared: declared_capabilities(&plugin, &id),
                        enabled: record
                            .get("enabled")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        active: row.get("active").and_then(Value::as_bool).unwrap_or(false),
                        valid: row.get("valid").and_then(Value::as_bool).unwrap_or(false),
                        manifest_family: record
                            .get("manifest_family")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        package_sha256: record
                            .get("package_sha256")
                            .and_then(Value::as_str)
                            .map(str::to_string),
                        source_path: record
                            .get("source")
                            .and_then(|s| s.get("path"))
                            .and_then(Value::as_str)
                            .map(PathBuf::from),
                        cache_path: record
                            .get("cache_path")
                            .and_then(Value::as_str)
                            .map(PathBuf::from),
                        id,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(PluginsList { plugins, raw })
}

/// The skill/theme id prefix a `--no-plugin` install writes to the managed
/// store. Catalog ids are `omm-` prefixed by the R19 lint
/// (`omm_manifest::lint::ID_PREFIX`); the plugin id is a different namespace
/// and must not be used here — the host lists managed skills by directory,
/// never by plugin.
pub const MANAGED_ID_PREFIX: &str = "omm-";

/// How omm is installed here (Gate 1 decision E): judged by the ledger
/// first and, when the ledger cannot say — corrupt or absent — by the host
/// itself: the plugin installed → the bundle; managed-store skills under
/// [`MANAGED_ID_PREFIX`] and no plugin → `--no-plugin`. Every fix a check
/// prints is spelled for this mode (round 4: a corrupt `--no-plugin` ledger
/// made D1/D13 print `omm install`, which stacked the bundle on top of
/// twelve managed skills and never converged).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstallMode {
    /// The plugin bundle.
    Bundle,
    /// `omm install --no-plugin`: the managed personal store.
    ManagedStore,
    /// Nothing of omm's, by either witness.
    None,
}

/// [`InstallMode`] with where it was read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModeEvidence {
    pub mode: InstallMode,
    /// `ledger` or `host`.
    pub source: &'static str,
    /// The managed-store skill ids the witness names (ledgered, or listed
    /// by the host under [`MANAGED_ID_PREFIX`]); empty for the bundle.
    pub skills: Vec<String>,
}

impl ModeEvidence {
    pub fn is_managed(&self) -> bool {
        self.mode == InstallMode::ManagedStore
    }
    /// The install command that converges this mode.
    pub fn install_fix(&self) -> &'static str {
        if self.is_managed() {
            FIX_INSTALL_NO_PLUGIN
        } else {
            FIX_INSTALL
        }
    }
    /// `omm reconcile` then the install of this mode.
    pub fn reconcile_then_install_fix(&self) -> &'static str {
        if self.is_managed() {
            FIX_RECONCILE_THEN_INSTALL_NO_PLUGIN
        } else {
            FIX_RECONCILE_THEN_INSTALL
        }
    }
}

/// The fix commands spelled per mode (D1, D10, D13).
pub const FIX_INSTALL: &str = "omm install";
/// See [`FIX_INSTALL`].
pub const FIX_INSTALL_NO_PLUGIN: &str = "omm install --no-plugin";
/// See [`FIX_INSTALL`].
pub const FIX_RECONCILE_THEN_INSTALL: &str = "omm reconcile\nomm install";
/// See [`FIX_INSTALL`].
pub const FIX_RECONCILE_THEN_INSTALL_NO_PLUGIN: &str = "omm reconcile\nomm install --no-plugin";

/// Everything a check needs, with the shared probes taken lazily.
#[derive(Debug)]
pub struct Context {
    /// Runs the host against the roots below (the clean environment passes
    /// `HOME` / `XDG_*` through; a sandboxed invoker overrides them).
    pub inv: Invoker,
    pub roots: Roots,
    /// The plugin doctor looks at — `omm` (reserved-ids.json `omm_identity`)
    /// unless a test says otherwise.
    pub plugin_id: String,
    /// The workspace D12 checks; the process cwd when unset.
    pub workspace: Option<PathBuf>,
    /// Stable ids the installer approved (from the ledger registration when
    /// one exists); D1 falls back to what the installed package declares.
    pub expected_capabilities: Option<Vec<String>>,
    pub options: Options,
    version: OnceLock<Option<String>>,
    settings: OnceLock<std::result::Result<SettingsDoc, String>>,
    plugins: OnceLock<std::result::Result<PluginsList, String>>,
    inspect: OnceLock<std::result::Result<PluginInspect, InspectFailure>>,
    live: OnceLock<std::result::Result<LiveSession, String>>,
    ledger: OnceLock<LedgerState>,
    user_skills: OnceLock<std::result::Result<Vec<String>, String>>,
    mode: OnceLock<ModeEvidence>,
}

impl Context {
    /// A context over explicit roots; `inv` must point at the same roots.
    pub fn new(inv: Invoker, roots: Roots) -> Result<Context> {
        let plugin_id = hr::reserved_ids()?.omm_identity.plugin_id.clone();
        Ok(Context {
            inv,
            roots,
            plugin_id,
            workspace: None,
            expected_capabilities: None,
            options: Options::default(),
            version: OnceLock::new(),
            settings: OnceLock::new(),
            plugins: OnceLock::new(),
            inspect: OnceLock::new(),
            live: OnceLock::new(),
            ledger: OnceLock::new(),
            user_skills: OnceLock::new(),
            mode: OnceLock::new(),
        })
    }

    /// The user's machine: locate the binary, resolve the roots from the
    /// process environment, workspace = cwd.
    pub fn from_env() -> Result<Context> {
        let inv = Invoker::from_env()?;
        let roots = Roots::from_env()?;
        Ok(Context::new(inv, roots)?.with_workspace(std::env::current_dir().ok()))
    }

    /// Look at another plugin id (tests install a fixture package).
    pub fn with_plugin_id(mut self, id: &str) -> Context {
        self.plugin_id = id.to_string();
        self
    }
    /// The workspace D12 checks.
    pub fn with_workspace(mut self, ws: Option<PathBuf>) -> Context {
        self.workspace = ws;
        self
    }
    /// The stable ids the installer recorded as approved.
    pub fn with_expected_capabilities(mut self, ids: Vec<String>) -> Context {
        self.expected_capabilities = Some(ids);
        self
    }
    /// Replace the tunables.
    pub fn with_options(mut self, options: Options) -> Context {
        self.options = options;
        self
    }

    /// The workspace under test: the explicit one, else the process cwd.
    pub fn workspace(&self) -> Option<PathBuf> {
        self.workspace
            .clone()
            .or_else(|| std::env::current_dir().ok())
    }

    /// `$OMM/omm.lock.json` (ARCHITECTURE.md §2).
    pub fn ledger_path(&self) -> PathBuf {
        self.roots.omm_root().join(ledger::LEDGER_FILE)
    }

    /// `muse --version` (informational, R15), once.
    pub fn version(&self) -> Option<&str> {
        self.version
            .get_or_init(|| probe::version(&self.inv).ok().map(|v| v.raw))
            .as_deref()
    }

    /// What the report is about.
    pub fn host_info(&self) -> HostInfo {
        HostInfo {
            binary: self.inv.bin().to_path_buf(),
            version: self.version().map(str::to_string),
            config_root: self.roots.muse_config(),
            data_root: self.roots.muse_data(),
            omm_root: self.roots.omm_root(),
            plugin_id: self.plugin_id.clone(),
            workspace: self.workspace(),
        }
    }

    /// The user's `settings.json`, loaded once through the host's own rules
    /// (a repeated key or a non-object is refused like the host does).
    pub fn settings(&self) -> std::result::Result<&SettingsDoc, &str> {
        self.settings
            .get_or_init(|| {
                SettingsDoc::load(&self.roots.settings_file()).map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(String::as_str)
    }

    /// `plugins list --json`, once.
    pub fn plugins(&self) -> std::result::Result<&PluginsList, &str> {
        self.plugins
            .get_or_init(|| plugins_list(&self.inv).map_err(|e| e.to_string()))
            .as_ref()
            .map_err(String::as_str)
    }

    /// `plugins inspect <plugin_id> --json`, once.
    pub fn inspect(&self) -> std::result::Result<&PluginInspect, &InspectFailure> {
        self.inspect
            .get_or_init(
                || match probe::plugins_inspect(&self.inv, &self.plugin_id) {
                    Ok(i) => Ok(i),
                    Err(HostError::HostReported { code, message }) if code == "unknown-plugin" => {
                        Err(InspectFailure::NotInstalled(message))
                    }
                    Err(e) => Err(InspectFailure::Other(e.to_string())),
                },
            )
            .as_ref()
    }

    /// The live echo session, once (`Options::live` off → an error row).
    pub fn live(&self) -> std::result::Result<&LiveSession, &str> {
        self.live
            .get_or_init(|| {
                if !self.options.live {
                    return Err("live session disabled by options".to_string());
                }
                let opts = LiveOptions {
                    config_home: None,
                    seed: self.options.seed,
                };
                session::run(&self.inv, &self.roots, &opts).map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(String::as_str)
    }

    /// The ledger, read once.
    pub fn ledger(&self) -> &LedgerState {
        self.ledger
            .get_or_init(|| ledger::load(&self.ledger_path()))
    }

    /// The stable ids the installer approved: the explicit list, else the
    /// ledger's `muse-plugin` registration for this plugin, else `None`.
    pub fn expected_capabilities(&self) -> Option<Vec<String>> {
        if let Some(ids) = &self.expected_capabilities {
            return Some(ids.clone());
        }
        match self.ledger() {
            LedgerState::Loaded(l) => l
                .plugin_registration(&self.plugin_id)
                .and_then(|r| r.approved.clone()),
            _ => None,
        }
    }

    /// What the installed package declares, from `plugins inspect`, each
    /// with its `enabledDefault` read from the cached package manifest
    /// (`record.cache_path`; see [`DeclaredCapability::enabled_default`]).
    pub fn declared_capabilities(&self) -> Vec<DeclaredCapability> {
        let Ok(ins) = self.inspect() else {
            return Vec::new();
        };
        let mut declared = ins
            .raw
            .get("plugin")
            .map(|p| declared_capabilities(p, &self.plugin_id))
            .unwrap_or_default();
        let defaults = ins
            .raw
            .get("record")
            .and_then(|r| r.get("cache_path"))
            .and_then(Value::as_str)
            .map(|cache| {
                Path::new(cache)
                    .join(hr::MANIFEST_DIR_NATIVE)
                    .join("plugin.json")
            })
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .map(|m| manifest_enabled_defaults(&m, &self.plugin_id))
            .unwrap_or_default();
        for d in &mut declared {
            if let Some(enabled) = defaults.get(&d.stable_id) {
                d.enabled_default = *enabled;
            }
        }
        declared
    }

    /// The skill ids `skills list --source user` lists, once.
    pub fn user_skills(&self) -> std::result::Result<&[String], &str> {
        self.user_skills
            .get_or_init(|| {
                probe::skills_list(
                    &self.inv,
                    &probe::SkillsListOptions {
                        source: Some("user".to_string()),
                        ..probe::SkillsListOptions::default()
                    },
                )
                .map(|l| l.skills.into_iter().map(|s| s.id).collect())
                .map_err(|e| e.to_string())
            })
            .as_ref()
            .map(Vec::as_slice)
            .map_err(String::as_str)
    }

    /// How omm is installed here ([`InstallMode`]), once: the ledger when it
    /// loads and says so, else the host (Gate 1 decision E).
    pub fn install_mode(&self) -> &ModeEvidence {
        self.mode.get_or_init(|| {
            if let LedgerState::Loaded(l) = self.ledger() {
                if l.plugin_registration(&self.plugin_id).is_some() {
                    return ModeEvidence {
                        mode: InstallMode::Bundle,
                        source: "ledger",
                        skills: Vec::new(),
                    };
                }
                let prefix = self
                    .roots
                    .personal_skills_dir()
                    .strip_prefix(self.roots.muse_config())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                let ids = l.managed_skill_ids(&prefix);
                if !ids.is_empty() {
                    return ModeEvidence {
                        mode: InstallMode::ManagedStore,
                        source: "ledger",
                        skills: ids,
                    };
                }
            }
            if self.inspect().is_ok() {
                return ModeEvidence {
                    mode: InstallMode::Bundle,
                    source: "host",
                    skills: Vec::new(),
                };
            }
            let ids: Vec<String> = self
                .user_skills()
                .map(|ids| {
                    ids.iter()
                        .filter(|id| id.starts_with(MANAGED_ID_PREFIX))
                        .cloned()
                        .collect()
                })
                .unwrap_or_default();
            if !ids.is_empty() {
                return ModeEvidence {
                    mode: InstallMode::ManagedStore,
                    source: "host",
                    skills: ids,
                };
            }
            ModeEvidence {
                mode: InstallMode::None,
                source: "host",
                skills: Vec::new(),
            }
        })
    }

    /// The skill ids of a managed-store install (`omm install --no-plugin`):
    /// `Some` when [`Context::install_mode`] is the managed store — from the
    /// ledger's `muse-skills-install` entries, or from the host when the
    /// ledger cannot say — the mode D1 and D10 must judge by `skills list
    /// --source user`, not by `plugins inspect`.
    pub fn managed_store_ids(&self) -> Option<Vec<String>> {
        let m = self.install_mode();
        m.is_managed().then(|| m.skills.clone())
    }

    /// A readable `settings.json` value at a dotted path.
    pub fn setting(&self, dotted: &str) -> Option<&Value> {
        let doc = self.settings().ok()?;
        let path: Vec<String> = dotted.split('.').map(str::to_string).collect();
        doc.get(&path)
    }

    /// The workspace path as the fix command should spell it: `.` when it is
    /// the cwd, else the path.
    pub fn workspace_arg(&self) -> String {
        let ws = self.workspace();
        let cwd = std::env::current_dir().ok();
        match (ws, cwd) {
            (Some(w), Some(c)) if w == c => ".".to_string(),
            (Some(w), _) => w.display().to_string(),
            (None, _) => ".".to_string(),
        }
    }

    /// Whether a path exists, for reports.
    pub fn exists(p: &Path) -> bool {
        p.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn declared_capabilities_follow_the_three_runtime_families() {
        let plugin = json!({"capabilities": {
            "skills": [{"id": "omm-fx"}],
            "hooks": [{"id": "omm-hook"}],
            "mcp_servers": [{"id": "omm-mcp"}],
            "commands": [{"id": "omm-cmd"}],
            "reminders": [{"id": "omm-rem"}]
        }});
        let d = declared_capabilities(&plugin, "omm-five");
        let ids: Vec<&str> = d.iter().map(|c| c.stable_id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "plugin:omm-five:hook:omm-hook",
                "plugin:omm-five:mcp_server:omm-mcp",
                "plugin:omm-five:reminder:omm-rem"
            ]
        );
        assert!(
            d.iter().all(|c| c.enabled_default),
            "unknown → the host default, true"
        );
        assert!(declared_capabilities(&json!({}), "x").is_empty());
        // The manifest spelling carries enabledDefault; only a literal false disables.
        let manifest = json!({"capabilities": {
            "hooks": [{"id": "omm-hook", "enabledDefault": true}],
            "mcpServers": [{"id": "omm-mcp"}],
            "reminders": [{"id": "omm-rem", "enabledDefault": false}, {"id": "omm-str", "enabledDefault": "false"}]
        }});
        let m = manifest_enabled_defaults(&manifest, "omm-five");
        assert_eq!(m.get("plugin:omm-five:hook:omm-hook"), Some(&true));
        assert_eq!(m.get("plugin:omm-five:mcp_server:omm-mcp"), Some(&true));
        assert_eq!(m.get("plugin:omm-five:reminder:omm-rem"), Some(&false));
        assert_eq!(m.get("plugin:omm-five:reminder:omm-str"), Some(&true));
    }

    #[test]
    fn report_ok_and_exit_code_follow_critical_only() {
        let host = HostInfo {
            binary: PathBuf::from("/x"),
            version: None,
            config_root: PathBuf::from("/c"),
            data_root: PathBuf::from("/d"),
            omm_root: PathBuf::from("/o"),
            plugin_id: "omm".into(),
            workspace: None,
        };
        let warn = Check::warn("D2", "t", "o", "w", "fix");
        let r = Report::new(host.clone(), vec![warn.clone()], 1);
        assert!(r.ok);
        assert_eq!(r.exit_code(), 0);
        assert_eq!(r.failures().len(), 1);
        let crit = Check::critical("D3", "t", "o", "w", "fix");
        let r = Report::new(host, vec![warn, crit], 1);
        assert!(!r.ok);
        assert_eq!(r.exit_code(), 1);
        assert_eq!(r.get("D3").map(|c| c.severity), Some(Severity::Critical));
        assert!(!Check::info("D9", "t", "o", "w").failed());
        assert_eq!(Severity::Critical.to_string(), "critical");
        assert!(Severity::Info < Severity::Warn && Severity::Warn < Severity::Critical);
    }
}
