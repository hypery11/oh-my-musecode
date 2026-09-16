//! Group C helpers (`cmd/tune.rs` is the only caller): where the content
//! bundle is found on the user's machine, `$OMM/config.json`, the ledger
//! plumbing every tuning write shares, and the report every write command
//! prints (ARCHITECTURE.md §2, §4, §7).
//!
//! Layout: [`settings_tx`] is the one settings.json transaction (patch →
//! validate → commit → ledger → audit) behind `theme`, `keymap`, `profile
//! use` and every `settings` action; [`theme`], [`profile`] (profiles and
//! keymaps), [`settings`], [`run`] and [`hook`] hold one command each.

pub mod hook;
pub mod hook_gates;
pub mod profile;
pub mod routing;
pub mod routing_doctor;
pub mod routing_enable;
pub mod run;
pub mod settings;
pub mod settings_tx;
pub mod theme;

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use omm_host::host_reality as hr;
use omm_host::{fsx, Invoker};
use omm_ledger::{store, HostInfo, Ledger, RelPath, Scope};
use omm_manifest::catalog::Catalog;
use omm_manifest::Repo;

use crate::cmd::{Ctx, OMM_VERSION};
use crate::error::{OmmError, Result};
use crate::output::{Converge, Render};

// ---- $OMM/config.json ---------------------------------------------------

/// The keys of `$OMM/config.json` group C reads and writes (ARCHITECTURE.md
/// §2: "user config: profile, disabled ids, overlay opts"). The overlay's
/// `disabled` list (`omm_manifest::overlay::load_config`) and any key not
/// named here round-trip untouched.
pub const CONFIG_KEY_PROFILE: &str = "profile";
/// `"skill_routing": true` — the M4 opt-in (`omm enable skill-routing`,
/// Phase 3); `omm run` then carries the two routing gates
/// (`hr::ENV_ROUTING_GATE`, `hr::ENV_ROUTING_APPLY_GATE`; host-reality.md
/// "Trust lifecycle": routing gates).
pub const CONFIG_KEY_SKILL_ROUTING: &str = "skill_routing";
/// `"gates": ["<gate id or MUSE_EXPERIMENTAL_* name>", …]` — extra gates the
/// active profile turns on for `omm run`, each checked against gates.json
/// (R8) before it reaches the environment.
pub const CONFIG_KEY_GATES: &str = "gates";

/// `$OMM/config.json`, loaded as a whole so unknown keys survive a save.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OmmConfig {
    doc: Map<String, Value>,
}

impl OmmConfig {
    /// `<omm_root>/config.json`.
    pub fn path(omm_root: &Path) -> PathBuf {
        omm_root.join(omm_manifest::overlay::CONFIG_FILE)
    }

    /// Load; absent → empty. A file that is not a JSON object is refused
    /// (there is nothing to merge into), never overwritten.
    pub fn load(omm_root: &Path) -> Result<OmmConfig> {
        let path = OmmConfig::path(omm_root);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(OmmConfig::default()),
            Err(e) => return Err(OmmError::io(format!("read {}", path.display()), e)),
        };
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|e| OmmError::Usage(format!("{}: not JSON: {e}", path.display())))?;
        match value {
            Value::Object(doc) => Ok(OmmConfig { doc }),
            other => Err(OmmError::Usage(format!(
                "{}: expected a JSON object, found {}",
                path.display(),
                json_kind(&other)
            ))),
        }
    }

    /// Parse from bytes (the hook's spawn-free path; malformed → empty).
    pub fn from_bytes_lenient(bytes: &[u8]) -> OmmConfig {
        match serde_json::from_slice::<Value>(bytes) {
            Ok(Value::Object(doc)) => OmmConfig { doc },
            _ => OmmConfig::default(),
        }
    }

    /// The active profile name (`omm profile use`).
    pub fn profile(&self) -> Option<&str> {
        self.doc.get(CONFIG_KEY_PROFILE).and_then(Value::as_str)
    }

    pub fn set_profile(&mut self, name: &str) {
        self.doc.insert(
            CONFIG_KEY_PROFILE.to_string(),
            Value::String(name.to_string()),
        );
    }

    /// `skill_routing: true`.
    pub fn skill_routing(&self) -> bool {
        self.doc
            .get(CONFIG_KEY_SKILL_ROUTING)
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    /// Capability-qualified ids the user switched off (`"disabled":
    /// ["skill:omm-pdf", "hook:omm-guard"]`, ARCHITECTURE §5.4). Read
    /// leniently — a malformed entry is skipped, never a reason to fail — so
    /// the hook dispatcher can consult it on its spawn-free path.
    pub fn disabled(&self) -> impl Iterator<Item = &str> {
        self.doc
            .get(omm_manifest::overlay::CONFIG_KEY_DISABLED)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
    }

    /// Is the capability-qualified id `<kind>:<id>` switched off?
    pub fn is_disabled(&self, kind: &str, id: &str) -> bool {
        self.disabled()
            .any(|d| d.split_once(':').is_some_and(|(k, i)| k == kind && i == id))
    }

    /// Any top-level key as it is (the `routing` record of
    /// `omm enable skill-routing`, `routing::CONFIG_KEY_ROUTING`).
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.doc.get(key)
    }

    /// Set (or replace) one top-level key.
    pub fn set(&mut self, key: &str, value: Value) {
        self.doc.insert(key.to_string(), value);
    }

    /// Remove one top-level key; what it held, if anything.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        self.doc.remove(key)
    }

    /// True when no key is left (a file omm created can go).
    pub fn is_empty(&self) -> bool {
        self.doc.is_empty()
    }

    /// The `gates` list (strings only; anything else is ignored here and
    /// reported by [`run::gate_env`]).
    pub fn gates(&self) -> Vec<String> {
        self.doc
            .get(CONFIG_KEY_GATES)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The raw `gates` value, for the shape error.
    pub fn gates_raw(&self) -> Option<&Value> {
        self.doc.get(CONFIG_KEY_GATES)
    }

    /// 2-space pretty JSON with a trailing newline.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(self.doc.clone()))
            .map_err(|e| OmmError::Usage(format!("config.json: {e}")))?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    /// Atomic write on the realpath (`fsx::write_atomic`), creating `$OMM/`.
    /// Returns the path and whether the bytes changed.
    pub fn save(&self, omm_root: &Path) -> Result<(PathBuf, bool)> {
        let path = OmmConfig::path(omm_root);
        let bytes = self.to_bytes()?;
        fsx::create_dir_all(omm_root)?;
        let realpath = fsx::realpath_for_write(&path)?;
        if realpath.exists() && fsx::read_bytes(&realpath)? == bytes {
            return Ok((realpath, false));
        }
        fsx::write_atomic(&realpath, &bytes)?;
        Ok((realpath, true))
    }
}

/// A short noun for a JSON value's type.
pub fn json_kind(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a bool",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

// ---- where the content bundle is ----------------------------------------

/// `OMM_CONTENT_DIR`: an explicit `content/` directory (must hold
/// `catalog.json`). Checked first.
pub const ENV_CONTENT_DIR: &str = "OMM_CONTENT_DIR";

/// How the content root was found, in probe order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentOrigin {
    /// `$OMM_CONTENT_DIR`.
    Env,
    /// The ledger's `muse-marketplace` registration source (the checkout
    /// `omm install` registered with `plugins marketplace add`), `/content`.
    Ledger,
    /// A checkout containing the running executable (a `target/` build).
    Executable,
    /// The build tree this binary was compiled in (`CARGO_MANIFEST_DIR`).
    BuildTree,
}

impl ContentOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            ContentOrigin::Env => "env",
            ContentOrigin::Ledger => "ledger",
            ContentOrigin::Executable => "executable",
            ContentOrigin::BuildTree => "build-tree",
        }
    }
}

/// The `content/` directory the tuning commands read themes, profiles and
/// the catalog from. Themes and profiles are not plugin capabilities and are
/// never packaged (`omm_manifest::generate::native`), so they come from the
/// checkout, never from the installed plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentSource {
    /// Canonical `content/`.
    pub root: PathBuf,
    pub origin: ContentOrigin,
}

impl ContentSource {
    /// Probe `$OMM_CONTENT_DIR` → the ledger's marketplace source → the
    /// executable's checkout → the build tree. `None` when nothing holds a
    /// `catalog.json`.
    pub fn locate(omm_root: &Path) -> Option<ContentSource> {
        if let Some(dir) = std::env::var_os(ENV_CONTENT_DIR).filter(|v| !v.is_empty()) {
            if let Some(root) = content_root_of(Path::new(&dir)) {
                return Some(ContentSource {
                    root,
                    origin: ContentOrigin::Env,
                });
            }
        }
        if let Some(src) = ledger_marketplace_source(omm_root) {
            if let Some(root) = content_root_of(&src.join(omm_manifest::CONTENT_DIR)) {
                return Some(ContentSource {
                    root,
                    origin: ContentOrigin::Ledger,
                });
            }
        }
        if let Some(root) = exe_checkout_content() {
            return Some(ContentSource {
                root,
                origin: ContentOrigin::Executable,
            });
        }
        if let Ok(repo) = Repo::from_cargo_manifest_dir() {
            if let Some(root) = content_root_of(&repo.content_dir()) {
                return Some(ContentSource {
                    root,
                    origin: ContentOrigin::BuildTree,
                });
            }
        }
        None
    }

    /// Like [`ContentSource::locate`], as an error naming every probe.
    pub fn require(omm_root: &Path) -> Result<ContentSource> {
        ContentSource::locate(omm_root).ok_or_else(|| {
            OmmError::Usage(format!(
                "no content bundle found: set {ENV_CONTENT_DIR} to a content/ directory, run `omm install` from a checkout, or run this binary from one"
            ))
        })
    }

    /// `content/catalog.json`, validated.
    pub fn catalog(&self) -> Result<Catalog> {
        Ok(Catalog::load(&self.root.join(omm_manifest::CATALOG_FILE))?)
    }

    /// `content/<rel>` for a catalog asset path, contained in the root.
    pub fn asset_path(&self, rel: &str) -> Result<PathBuf> {
        Ok(omm_manifest::contained(&self.root, &self.root.join(rel))?)
    }
}

/// `dir` canonicalized when it holds `catalog.json`.
fn content_root_of(dir: &Path) -> Option<PathBuf> {
    let root = fsx::canonicalize(dir).ok()?;
    root.join(omm_manifest::CATALOG_FILE)
        .is_file()
        .then_some(root)
}

/// The `source` of the ledger's `muse-marketplace` registration, read
/// without the quarantine side effect of `store::load` (a read-only lookup
/// must never rename the user's ledger).
pub fn ledger_marketplace_source(omm_root: &Path) -> Option<PathBuf> {
    let bytes = std::fs::read(store::ledger_path(omm_root)).ok()?;
    let doc: Value = serde_json::from_slice(&bytes).ok()?;
    doc.get("registrations")?
        .as_array()?
        .iter()
        .find(|r| r.get("kind").and_then(Value::as_str) == Some("muse-marketplace"))
        .and_then(|r| r.get("source"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
}

/// Walk up from the running executable looking for `content/catalog.json`
/// (a `target/{debug,release}/omm` inside a checkout).
fn exe_checkout_content() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = fsx::canonicalize(&exe).ok()?;
    exe.ancestors()
        .skip(1)
        .take(8)
        .find_map(|dir| content_root_of(&dir.join(omm_manifest::CONTENT_DIR)))
}

// ---- ledger plumbing ----------------------------------------------------

/// A base-relative ledger path from a `/`-joined string.
pub fn rel(path: &str) -> Result<RelPath> {
    Ok(RelPath::new(path)?)
}

/// The host as the ledger records it (`--version` + the binary's SHA-256;
/// informational, never gated on — R15).
pub fn host_info(inv: &Invoker) -> omm_host::Result<HostInfo> {
    let version = omm_host::probe::version(inv)?;
    let sha256 = fsx::sha256_file(inv.bin())?;
    Ok(HostInfo {
        version: version.build,
        sha256,
    })
}

/// Load–modify–save the ledger under its lock (`store::modify`), creating it
/// when absent (host info from one `--version` run). A quarantined corrupt
/// ledger is reported loudly on stderr, as the store asks.
pub fn modify_ledger<T>(ctx: &Ctx, f: impl FnOnce(&mut Ledger) -> Result<T>) -> Result<T> {
    let omm_root = ctx.omm_root();
    let (inner, quarantined) = store::modify(&omm_root, |slot| {
        if slot.is_none() {
            let inv = match ctx.invoker() {
                Ok(inv) => inv,
                Err(e) => return Ok(Err(e)),
            };
            let info = host_info(inv).map_err(omm_ledger::LedgerError::Host)?;
            *slot = Some(Ledger::new(OMM_VERSION, info, Scope::User));
        }
        match slot.as_mut() {
            Some(ledger) => Ok(f(ledger)),
            None => Ok(Err(OmmError::Usage("ledger could not be created".into()))),
        }
    })?;
    if let Some(q) = quarantined {
        ctx.out.warn(q.message());
    }
    inner
}

/// Read the ledger without side effects: a corrupt one is reported and
/// treated as absent (the next write quarantines it through `store`).
pub fn read_ledger(ctx: &Ctx) -> Result<Option<Ledger>> {
    let path = store::ledger_path(&ctx.omm_root());
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(OmmError::io(format!("read {}", path.display()), e)),
    };
    match Ledger::from_bytes(&bytes, &path) {
        Ok(l) => Ok(Some(l)),
        Err(omm_ledger::LedgerError::Schema { detail, .. }) => {
            ctx.out.warn(format!(
                "{}: corrupt ledger ({detail}); treated as absent — the next write quarantines it",
                path.display()
            ));
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

// ---- reports ------------------------------------------------------------

/// What a write command prints: free lines (human mode only), extra JSON
/// fields, and the converge summary (ARCHITECTURE.md §7 "Global").
#[derive(Clone, Debug, Default)]
pub struct WriteReport {
    pub lines: Vec<String>,
    pub fields: Map<String, Value>,
    pub converge: Converge,
}

impl WriteReport {
    pub fn new(dry_run: bool) -> WriteReport {
        WriteReport {
            lines: Vec::new(),
            fields: Map::new(),
            converge: Converge::new(dry_run),
        }
    }

    pub fn line(&mut self, text: impl Into<String>) -> &mut WriteReport {
        self.lines.push(text.into());
        self
    }

    pub fn field(&mut self, key: &str, value: Value) -> &mut WriteReport {
        self.fields.insert(key.to_string(), value);
        self
    }
}

impl Render for WriteReport {
    fn render(&self) -> String {
        let mut text = self.lines.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&self.converge.render());
        text
    }

    fn to_json(&self) -> Value {
        let mut obj = match self.converge.to_json() {
            Value::Object(m) => m,
            other => {
                let mut m = Map::new();
                m.insert("converge".to_string(), other);
                m
            }
        };
        for (k, v) in &self.fields {
            obj.insert(k.clone(), v.clone());
        }
        Value::Object(obj)
    }
}

// ---- small shared helpers ------------------------------------------------

/// `omm settings set <key> <value>`: JSON when it parses, else a string
/// (`meta` → `"meta"`, `false` → `false`, `3` → `3`, `custom:x` → `"custom:x"`).
pub fn parse_value(raw: &str) -> Value {
    serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

/// `18420` → `18,420`.
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A one-line rendering of a JSON value for reports (`null` for absent).
pub fn show(v: Option<&Value>) -> String {
    match v {
        None => "(absent)".to_string(),
        Some(v) => v.to_string(),
    }
}

/// A user-supplied name that becomes a file stem: no separators, no `..`,
/// no NUL, non-empty, and within the host's id grammar
/// (`hr::ID_GRAMMAR`: `^[a-z0-9][a-z0-9._-]{0,79}$`).
pub fn check_name(what: &str, name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= hr::ID_MAX_BYTES
        && name != "."
        && name != ".."
        && name.bytes().enumerate().all(|(i, b)| match b {
            b'a'..=b'z' | b'0'..=b'9' => true,
            b'.' | b'_' | b'-' => i > 0,
            _ => false,
        });
    if ok {
        Ok(())
    } else {
        Err(OmmError::Usage(format!(
            "{what} name {name:?} is not a valid id ({})",
            hr::ID_GRAMMAR
        )))
    }
}

/// `path` is a regular file (never a symlink — R12 spirit: omm copies bytes
/// it can hash) and returns its bytes.
pub fn read_regular_file(path: &Path) -> Result<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| OmmError::io(format!("stat {}", path.display()), e))?;
    if meta.file_type().is_symlink() {
        return Err(OmmError::Usage(format!(
            "{} is a symlink; omm copies regular files only",
            path.display()
        )));
    }
    if !meta.is_file() {
        return Err(OmmError::Usage(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    Ok(fsx::read_bytes(path)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn config_round_trips_unknown_keys_and_reads_its_own() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("omm");
        assert_eq!(OmmConfig::load(&root).unwrap(), OmmConfig::default());
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("config.json"),
            br#"{"disabled":["skill:omm-x"],"gates":["hook_selected_skills",3],"skill_routing":true}"#,
        )
        .unwrap();
        let mut c = OmmConfig::load(&root).unwrap();
        assert_eq!(c.profile(), None);
        assert!(c.skill_routing());
        assert_eq!(c.gates(), vec!["hook_selected_skills"]);
        c.set_profile("fast");
        let (path, changed) = c.save(&root).unwrap();
        assert!(changed);
        let back: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(back["disabled"], json!(["skill:omm-x"]));
        assert_eq!(back["profile"], "fast");
        let (_, changed) = c.save(&root).unwrap();
        assert!(!changed, "identical bytes are not rewritten");
        std::fs::write(root.join("config.json"), b"[]").unwrap();
        assert!(matches!(OmmConfig::load(&root), Err(OmmError::Usage(_))));
        assert_eq!(OmmConfig::from_bytes_lenient(b"nope"), OmmConfig::default());
    }

    #[test]
    fn values_parse_as_json_or_string() {
        assert_eq!(parse_value("meta"), json!("meta"));
        assert_eq!(parse_value("false"), json!(false));
        assert_eq!(parse_value("3"), json!(3));
        assert_eq!(parse_value("custom:omm-carbon"), json!("custom:omm-carbon"));
        assert_eq!(parse_value("[\"a\"]"), json!(["a"]));
        assert_eq!(parse_value("\"5\""), json!("5"));
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(18420), "18,420");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn names_follow_the_id_grammar() {
        for good in ["omm-carbon", "a", "x.y_z-1"] {
            assert!(check_name("theme", good).is_ok(), "{good}");
        }
        for bad in ["", "-x", "A", "a/b", "..", "a b", "a\\b", ".hidden"] {
            assert!(check_name("theme", bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn content_source_is_found_from_the_build_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let src = ContentSource::locate(tmp.path()).expect("the checkout holds content/");
        assert!(src.root.join("catalog.json").is_file());
        assert!(matches!(
            src.origin,
            ContentOrigin::Executable | ContentOrigin::BuildTree | ContentOrigin::Env
        ));
        let catalog = src.catalog().unwrap();
        assert!(!catalog.assets.is_empty());
        assert!(src.asset_path("../../etc/passwd").is_err());
    }

    #[test]
    fn marketplace_source_is_read_without_touching_the_ledger() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(ledger_marketplace_source(tmp.path()), None);
        std::fs::write(
            tmp.path().join("omm.lock.json"),
            br#"{"registrations":[{"kind":"muse-plugin","id":"omm"},{"kind":"muse-marketplace","name":"ohmy","source":"/src/omm"}]}"#,
        )
        .unwrap();
        assert_eq!(
            ledger_marketplace_source(tmp.path()),
            Some(PathBuf::from("/src/omm"))
        );
        std::fs::write(tmp.path().join("omm.lock.json"), b"garbage").unwrap();
        assert_eq!(ledger_marketplace_source(tmp.path()), None);
        assert!(tmp.path().join("omm.lock.json").exists(), "never renamed");
    }

    #[test]
    fn write_report_merges_fields_into_the_converge_document() {
        let mut r = WriteReport::new(true);
        r.line("theme omm-carbon")
            .field("theme", json!("omm-carbon"));
        r.converge.record("themes", crate::output::Action::Updated);
        let text = r.render();
        assert!(text.starts_with("theme omm-carbon\ncategory"));
        assert!(text.ends_with("dry run: nothing was written"));
        let j = r.to_json();
        assert_eq!(j["theme"], "omm-carbon");
        assert_eq!(j["dry_run"], true);
        assert_eq!(j["categories"]["themes"]["updated"], 1);
    }
}
