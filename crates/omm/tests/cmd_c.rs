//! Group C against the real binary in a throwaway HOME/XDG sandbox
//! (PLAN.md e2e scenarios 7, 8, 9 and the settings fixes doctor prints).
//! Every host-touching test skips with a message when `OMM_MUSE_BIN` is
//! unset; nothing here touches the user's real config root.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;

use omm::cmd::tune::{
    self, KeymapArgs, ProfileAction, ProfileArgs, SettingsAction, SettingsArgs, ThemeArgs,
};
use omm::{Ctx, Flags, OmmError};
use omm_host::{Invoker, Roots, Sandbox};
use omm_ledger::{store, Base, Kind, Registration};

struct Host {
    ctx: Ctx,
    sb: Sandbox,
    bin: PathBuf,
    _tmp: TempDir,
}

fn host_bin() -> Option<PathBuf> {
    std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

fn host_with(flags: Flags) -> Option<Host> {
    let bin = host_bin()?;
    let tmp = tempfile::Builder::new()
        .prefix("omm-cmd-c-")
        .tempdir()
        .expect("temp dir");
    let sb = Sandbox::create(tmp.path()).expect("sandbox");
    let roots = sb.roots().expect("roots");
    let ctx = Ctx::new(roots, flags).with_invoker(Invoker::new(&bin).sandboxed(&sb));
    Some(Host {
        ctx,
        sb,
        bin,
        _tmp: tmp,
    })
}

fn host() -> Option<Host> {
    host_with(Flags {
        yes: true,
        ..Flags::default()
    })
}

macro_rules! host_or_skip {
    () => {
        match host() {
            Some(h) => h,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

fn roots(h: &Host) -> Roots {
    h.sb.roots().expect("roots")
}

fn settings(h: &Host) -> Value {
    let path = roots(h).settings_file();
    match std::fs::read(&path) {
        Ok(b) => serde_json::from_slice(&b).expect("settings.json is JSON"),
        Err(_) => Value::Null,
    }
}

fn write_settings(h: &Host, v: &Value) {
    let path = roots(h).settings_file();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes = serde_json::to_vec_pretty(v).unwrap();
    bytes.push(b'\n');
    std::fs::write(path, bytes).unwrap();
}

fn ledger(h: &Host) -> Option<omm_ledger::Ledger> {
    store::load(&h.ctx.omm_root())
        .expect("ledger loads")
        .into_ledger()
}

// `OmmError` is large by design (many variants, cold path); boxing it would
// churn the public API for no runtime gain, so the two tests below that
// close over it carry a targeted allow instead.

fn settings_key_registrations(l: &omm_ledger::Ledger, key: &str) -> Vec<Option<Value>> {
    l.registrations
        .iter()
        .filter_map(|r| match r {
            Registration::SettingsKey { path, prior, .. } if path == key => Some(prior.clone()),
            _ => None,
        })
        .collect()
}

fn content_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("content")
        .canonicalize()
        .expect("content/")
}

/// The omm binary against the sandbox, argv after the program name.
fn omm_cmd(h: &Host, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omm"));
    cmd.args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &h.sb.home)
        .env("XDG_CONFIG_HOME", &h.sb.config_home)
        .env("XDG_DATA_HOME", &h.sb.data_home)
        .env("OMM_MUSE_BIN", &h.bin)
        .env("MUSE_NO_AUTO_UPDATE", "1")
        .env("TMPDIR", std::env::temp_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn run_omm(h: &Host, args: &[&str]) -> (Option<i32>, String, String) {
    let out = omm_cmd(h, args).output().expect("run omm");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn run_hook(h: &Host, name: &str, payload: &[u8]) -> (Option<i32>, String, String, Duration) {
    let mut cmd = omm_cmd(h, &["hook", name]);
    cmd.stdin(Stdio::piped());
    let started = Instant::now();
    let mut child = cmd.spawn().expect("spawn omm hook");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(payload)
        .expect("write payload");
    let out = child.wait_with_output().expect("wait");
    let elapsed = started.elapsed();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        elapsed,
    )
}

// ---- scenario 7: theme -----------------------------------------------------------

#[test]
fn theme_copies_the_file_sets_tui_theme_and_ledgers_both_with_prior() {
    let h = host_or_skip!();
    let r = roots(&h);
    let dest = r.themes_dir().join("omm-carbon.tmTheme");
    let bundled = std::fs::read(content_root().join("themes/omm-carbon.tmTheme")).unwrap();

    // Dry run: validated by the host, nothing written.
    let dry = Ctx::new(
        r.clone(),
        Flags {
            dry_run: true,
            ..Flags::default()
        },
    )
    .with_invoker(Invoker::new(&h.bin).sandboxed(&h.sb));
    tune::theme(
        &dry,
        &ThemeArgs {
            name: "omm-carbon".into(),
        },
    )
    .expect("dry run");
    assert!(!dest.exists(), "dry run copies nothing");
    assert_eq!(settings(&h), Value::Null, "dry run writes no settings");
    assert!(ledger(&h).is_none(), "dry run ledgers nothing");

    tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "omm-carbon".into(),
        },
    )
    .expect("theme");
    assert_eq!(
        std::fs::read(&dest).unwrap(),
        bundled,
        "byte-identical copy"
    );
    let s = settings(&h);
    assert_eq!(s["schema_version"], 1);
    assert_eq!(s["tui"]["theme"], "custom:omm-carbon");
    let l = ledger(&h).expect("ledger created");
    let entry = l
        .find(
            Base::MuseConfig,
            &omm_ledger::RelPath::new("themes/omm-carbon.tmTheme").unwrap(),
        )
        .expect("theme entry");
    assert_eq!(entry.kind, Kind::Theme);
    assert_eq!(entry.sha256, omm_host::fsx::sha256_bytes(&bundled));
    assert_eq!(entry.writer, "omm theme");
    let settings_entry = l
        .find(
            Base::MuseConfig,
            &omm_ledger::RelPath::new("settings.json").unwrap(),
        )
        .expect("settings entry");
    assert_eq!(settings_entry.kind, Kind::SettingsKey);
    assert_eq!(
        settings_entry.class,
        omm_ledger::Class::Seeded,
        "omm created settings.json, so uninstall may unlink it when empty"
    );
    assert_eq!(
        settings_key_registrations(&l, "tui.theme"),
        vec![None],
        "prior recorded: absent before omm"
    );
    assert!(!l.host.version.is_empty() && l.host.sha256.len() == 64);
    let audit = omm_ledger::audit::read(&h.ctx.omm_root()).unwrap();
    assert!(audit.len() >= 2, "{audit:?}");
    assert!(audit.iter().any(|a| a.action == "install"));
    assert!(audit.iter().any(|a| a.action == "update"));

    // Idempotent: a second run changes nothing and adds no registration.
    let before = std::fs::read(r.settings_file()).unwrap();
    tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "custom:omm-carbon".into(),
        },
    )
    .expect("theme again");
    assert_eq!(std::fs::read(r.settings_file()).unwrap(), before);
    let l = ledger(&h).unwrap();
    assert_eq!(settings_key_registrations(&l, "tui.theme").len(), 1);

    // Switching keeps the FIRST prior, so uninstall restores the original.
    tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "omm-paper".into(),
        },
    )
    .expect("switch theme");
    assert_eq!(settings(&h)["tui"]["theme"], "custom:omm-paper");
    let l = ledger(&h).unwrap();
    assert_eq!(settings_key_registrations(&l, "tui.theme"), vec![None]);
    assert!(r.themes_dir().join("omm-paper.tmTheme").is_file());

    // A user-edited file at the destination is never overwritten (R3 stage).
    std::fs::write(&dest, b"<!-- my own carbon -->").unwrap();
    tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "omm-carbon".into(),
        },
    )
    .expect("theme over an edited file");
    assert_eq!(std::fs::read(&dest).unwrap(), b"<!-- my own carbon -->");
    assert_eq!(settings(&h)["tui"]["theme"], "custom:omm-carbon");

    // Unknown and disabled names are refused before anything runs.
    let err = tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "no-such-theme".into(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, OmmError::Usage(_)), "{err}");
    assert!(err.to_string().contains("omm-carbon"), "{err}");
    std::fs::write(
        h.ctx.omm_root().join("config.json"),
        br#"{"disabled":["theme:omm-slate"]}"#,
    )
    .unwrap();
    let err = tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "omm-slate".into(),
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("disabled"), "{err}");

    // The custom overlay wins over the bundled file (R7).
    let custom_dir = h.ctx.omm_root().join("custom/themes");
    std::fs::create_dir_all(&custom_dir).unwrap();
    std::fs::write(custom_dir.join("mine.tmTheme"), b"<plist/>").unwrap();
    tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "mine".into(),
        },
    )
    .expect("custom theme");
    assert_eq!(
        std::fs::read(r.themes_dir().join("mine.tmTheme")).unwrap(),
        b"<plist/>"
    );
    assert_eq!(settings(&h)["tui"]["theme"], "custom:mine");
    tune::theme(
        &h.ctx,
        &ThemeArgs {
            name: "list".into(),
        },
    )
    .expect("theme list");
    // Nothing of ours beside the host's files.
    let names: Vec<String> = r
        .muse_config()
        .read_dir()
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    // (`.auth.json.lock` is the host's own; only omm-named leftovers count.)
    assert!(
        names.iter().all(|n| !n.contains("omm")),
        "omm left something beside the host's files: {names:?}"
    );
    assert!(names.contains(&"settings.json".to_string()) && names.contains(&"themes".to_string()));
}

// ---- keymap ----------------------------------------------------------------------

#[test]
fn keymap_presets_are_validated_by_the_host_before_landing() {
    let h = host_or_skip!();
    let dir = h.ctx.omm_root().join("custom/keymaps");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("mine.json"),
        br#"{"app":{"clear-terminal":["ctrl+q","alt+j"]},"editor":{"transpose":[]}}"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("broken.json"),
        br#"{"zzz":{"transpose":["ctrl+q"]}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("shape.json"), br#"{"app":{"commands":5}}"#).unwrap();

    tune::keymap(
        &h.ctx,
        &KeymapArgs {
            preset: "mine".into(),
        },
    )
    .expect("keymap mine");
    assert_eq!(
        settings(&h)["tui"]["keymap"]["app"]["clear-terminal"],
        json!(["ctrl+q", "alt+j"])
    );
    let l = ledger(&h).unwrap();
    assert_eq!(settings_key_registrations(&l, "tui.keymap"), vec![None]);

    // The host's validator refuses an unknown context; nothing lands.
    let before = std::fs::read(roots(&h).settings_file()).unwrap();
    let err = tune::keymap(
        &h.ctx,
        &KeymapArgs {
            preset: "broken".into(),
        },
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            OmmError::Host(omm_host::HostError::SettingsRejected { .. })
        ),
        "{err}"
    );
    assert_eq!(std::fs::read(roots(&h).settings_file()).unwrap(), before);
    // A shape error is refused locally, before any host run.
    let err = tune::keymap(
        &h.ctx,
        &KeymapArgs {
            preset: "shape".into(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, OmmError::Usage(_)), "{err}");
    let err = tune::keymap(
        &h.ctx,
        &KeymapArgs {
            preset: "nope".into(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, OmmError::Usage(_)), "{err}");

    // `default` removes the key; the registration keeps its first prior.
    tune::keymap(
        &h.ctx,
        &KeymapArgs {
            preset: "default".into(),
        },
    )
    .expect("keymap default");
    assert!(settings(&h)["tui"].get("keymap").is_none());
    let l = ledger(&h).unwrap();
    assert_eq!(settings_key_registrations(&l, "tui.keymap"), vec![None]);
    tune::keymap(
        &h.ctx,
        &KeymapArgs {
            preset: "list".into(),
        },
    )
    .expect("keymap list");
}

// ---- profile ---------------------------------------------------------------------

#[test]
#[allow(clippy::result_large_err)]
fn profile_use_is_one_transaction_with_first_priors_and_the_r18_guard() {
    let h = host_or_skip!();
    let r = roots(&h);
    // A user value that must survive the profile's `run.*` keys.
    write_settings(
        &h,
        &json!({"schema_version": 1, "run": {"workflow_trigger_mode": "off"}}),
    );
    let use_profile = |name: &str| {
        tune::profile(
            &h.ctx,
            &ProfileArgs {
                action: ProfileAction::Use {
                    name: name.into(),
                    force: false,
                },
            },
        )
    };
    use_profile("default").expect("profile use default");
    let s = settings(&h);
    assert_eq!(
        s["run"]["context_slimming"]["skill_catalog_descriptions"],
        "first_sentence"
    );
    assert_eq!(
        s["run"]["context_slimming"]["full_skill_description_ids"],
        json!(["bundled:git"]),
        "R18: bundled:git re-listed"
    );
    assert_eq!(
        s["run"]["workflow_trigger_mode"], "off",
        "user key survived"
    );
    let config: Value =
        serde_json::from_slice(&std::fs::read(h.ctx.omm_root().join("config.json")).unwrap())
            .unwrap();
    assert_eq!(config["profile"], "default");
    let first_default = settings(&h);
    let l = ledger(&h).unwrap();
    assert_eq!(
        settings_key_registrations(&l, "run.context_slimming.skill_catalog_descriptions"),
        vec![None]
    );
    let entry = l
        .find(
            Base::MuseConfig,
            &omm_ledger::RelPath::new("settings.json").unwrap(),
        )
        .unwrap();
    assert_eq!(
        entry.class,
        omm_ledger::Class::SharedKey,
        "the user's file existed before omm"
    );
    let backups = std::fs::read_dir(r.snapshots_dir()).unwrap().count();
    assert!(backups >= 1, "a verified backup was taken");

    // Switching profiles keeps the first prior per key.
    for name in ["fast", "strict", "ci"] {
        use_profile(name).unwrap_or_else(|e| panic!("profile use {name}: {e}"));
        let l = ledger(&h).unwrap();
        assert_eq!(
            settings_key_registrations(&l, "run.context_slimming.skill_catalog_descriptions"),
            vec![None],
            "{name}"
        );
        assert_eq!(
            settings(&h)["run"]["workflow_trigger_mode"],
            "off",
            "{name}"
        );
    }
    assert_eq!(settings(&h)["max_consecutive_stop_hook_continuations"], 1);
    assert_eq!(
        settings(&h)["permissions"]["default_profile"],
        "omm-ci",
        "the ci profile's permission slice landed"
    );
    // Every key the ci slice set is tagged with the profile in the ledger
    // (Gate 1 decision: registrations carry the profile name).
    let l = ledger(&h).unwrap();
    for key in [
        "max_consecutive_stop_hook_continuations",
        "permissions.default_profile",
        "run.context_slimming.skill_catalog_descriptions",
    ] {
        match l.settings_key(key) {
            Some(Registration::SettingsKey { profile, .. }) => {
                assert_eq!(profile.as_deref(), Some("ci"), "{key}")
            }
            other => panic!("{key}: {other:?}"),
        }
    }
    // Back to default: the SAME document as the first application — every
    // leaf fast/strict/ci introduced (permissions.*, reasoning_effort,
    // max_consecutive_stop_hook_continuations, telemetry …) is back at its
    // prior (Gate 1 `prof`: strict's permissions.* stayed active under a
    // config.json that said default), every registration still singular.
    use_profile("default").expect("back to default");
    assert_eq!(
        settings(&h),
        first_default,
        "default → fast → strict → ci → default must return to the first default document"
    );
    let l = ledger(&h).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for reg in &l.registrations {
        if let Registration::SettingsKey { path, .. } = reg {
            assert!(seen.insert(path.clone()), "duplicate registration {path}");
        }
    }
    assert!(
        settings_key_registrations(&l, "permissions.default_profile").is_empty()
            && settings_key_registrations(&l, "max_consecutive_stop_hook_continuations").is_empty(),
        "restored keys lose their registration: {:?}",
        l.registrations
    );
    // The restore comes from the ledger's tags, not from config.json naming
    // the outgoing profile: strict, then config.json gone, then default.
    use_profile("strict").expect("strict");
    assert_eq!(settings(&h)["permissions"]["default_profile"], "omm-strict");
    std::fs::remove_file(h.ctx.omm_root().join("config.json")).expect("drop config.json");
    use_profile("default").expect("default without config.json");
    assert_eq!(
        settings(&h),
        first_default,
        "strict's permissions.* must go back to their prior with no config.json to name strict"
    );
    let l = ledger(&h).unwrap();
    assert!(settings_key_registrations(&l, "permissions.default_profile").is_empty());

    // The R18 trap is refused before the host is consulted or anything lands.
    let custom = h.ctx.omm_root().join("custom/profiles");
    std::fs::create_dir_all(&custom).unwrap();
    std::fs::write(
        custom.join("trap.json"),
        br#"{"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence","full_skill_description_ids":[]}}}"#,
    )
    .unwrap();
    let before = std::fs::read(r.settings_file()).unwrap();
    let err = use_profile("trap").unwrap_err();
    assert!(matches!(err, OmmError::Usage(_)), "{err}");
    assert!(err.to_string().contains("bundled:git"), "{err}");
    assert_eq!(std::fs::read(r.settings_file()).unwrap(), before);
    assert_eq!(
        serde_json::from_slice::<Value>(
            &std::fs::read(h.ctx.omm_root().join("config.json")).unwrap()
        )
        .unwrap()["profile"],
        "default"
    );
    // A foreign key in a custom profile is refused too.
    std::fs::write(custom.join("bad.json"), br#"{"nope": 1}"#).unwrap();
    assert!(matches!(use_profile("bad"), Err(OmmError::Usage(_))));

    tune::profile(
        &h.ctx,
        &ProfileArgs {
            action: ProfileAction::List,
        },
    )
    .expect("profile list");
    tune::profile(
        &h.ctx,
        &ProfileArgs {
            action: ProfileAction::Show {
                name: "fast".into(),
            },
        },
    )
    .expect("profile show");
}

/// Gate 1: `omm profile use strict` overwrote a hand edit of a key the
/// incoming slice also sets (`run.context_slimming.excluded_tool_names`,
/// set by default, edited to `["workflow"]`), with `left_edited: []` —
/// DECISION 5 / R3 (never overwrite a user edit; preserve and report) was
/// applied to the outgoing restores only.
#[test]
fn profile_use_leaves_a_hand_edited_key_the_incoming_slice_sets_unless_forced() {
    let h = host_or_skip!();
    let key = "run.context_slimming.excluded_tool_names";
    let profile_use = |args: &[&str]| -> (Option<i32>, Value, String) {
        let mut argv = vec!["profile", "use"];
        argv.extend_from_slice(args);
        argv.extend_from_slice(&["--json", "--yes"]);
        let (code, out, err) = run_omm(&h, &argv);
        let json = serde_json::from_str::<Value>(out.trim()).unwrap_or(Value::Null);
        (code, json, err)
    };
    let (code, doc, err) = profile_use(&["default"]);
    assert_eq!(code, Some(0), "{doc}\n{err}");
    assert_eq!(
        settings(&h)["run"]["context_slimming"]["excluded_tool_names"],
        json!([])
    );
    // The user's hand edit of a key the default profile set.
    let mut s = settings(&h);
    s["run"]["context_slimming"]["excluded_tool_names"] = json!(["workflow"]);
    write_settings(&h, &s);

    // strict sets the same key: the edit stays, and is named.
    let (code, doc, err) = profile_use(&["strict"]);
    assert_eq!(code, Some(0), "{doc}\n{err}");
    assert_eq!(
        settings(&h)["run"]["context_slimming"]["excluded_tool_names"],
        json!(["workflow"]),
        "the hand edit must survive the switch: {doc}"
    );
    assert_eq!(doc["left_edited"], json!([key]), "{doc}");
    assert!(
        !doc["changed"].as_array().unwrap().iter().any(|k| k == key),
        "{doc}"
    );
    // strict's other keys landed: the transaction went through.
    assert_eq!(settings(&h)["permissions"]["default_profile"], "omm-strict");
    // The registration still says what omm wrote last (default's value), so
    // a switch back sees the edit again and leaves it again.
    let l = ledger(&h).unwrap();
    match l.settings_key(key) {
        Some(Registration::SettingsKey { value, profile, .. }) => {
            assert_eq!(value.as_ref(), Some(&json!([])), "{l:?}");
            assert_eq!(profile.as_deref(), Some("default"));
        }
        other => panic!("{other:?}"),
    }
    let (code, doc, err) = profile_use(&["default"]);
    assert_eq!(code, Some(0), "{doc}\n{err}");
    assert_eq!(
        settings(&h)["run"]["context_slimming"]["excluded_tool_names"],
        json!(["workflow"])
    );
    assert_eq!(doc["left_edited"], json!([key]), "{doc}");

    // --force applies the slice over the edit; the first prior is kept.
    let (code, doc, err) = profile_use(&["strict", "--force"]);
    assert_eq!(code, Some(0), "{doc}\n{err}");
    assert_eq!(
        settings(&h)["run"]["context_slimming"]["excluded_tool_names"],
        json!([])
    );
    assert_eq!(doc["left_edited"], json!([]), "{doc}");
    assert!(
        doc["changed"].as_array().unwrap().iter().any(|k| k == key),
        "{doc}"
    );
    let l = ledger(&h).unwrap();
    assert_eq!(settings_key_registrations(&l, key), vec![None]);
    match l.settings_key(key) {
        Some(Registration::SettingsKey { profile, .. }) => {
            assert_eq!(profile.as_deref(), Some("strict"))
        }
        other => panic!("{other:?}"),
    }
}

// ---- settings: D2–D6 fixes -----------------------------------------------------------

#[test]
#[allow(clippy::result_large_err)]
fn settings_set_fix_mcp_collision_lint_and_reconcile_reminders() {
    let h = host_or_skip!();
    let r = roots(&h);
    let settings_cmd = |action: SettingsAction| tune::settings(&h.ctx, &SettingsArgs { action });

    // D2 / D6.
    settings_cmd(SettingsAction::Set {
        key: "provider".into(),
        value: "meta".into(),
    })
    .expect("set provider");
    settings_cmd(SettingsAction::Set {
        key: "agent_definitions.safe_mode".into(),
        value: "false".into(),
    })
    .expect("set safe_mode");
    let s = settings(&h);
    assert_eq!(s["provider"], "meta");
    assert_eq!(s["agent_definitions"]["safe_mode"], false);
    let l = ledger(&h).unwrap();
    assert_eq!(settings_key_registrations(&l, "provider"), vec![None]);
    assert_eq!(
        settings_key_registrations(&l, "agent_definitions.safe_mode"),
        vec![None]
    );
    // A wrong type is refused by the host's validator; nothing lands.
    let before = std::fs::read(r.settings_file()).unwrap();
    let err = settings_cmd(SettingsAction::Set {
        key: "provider".into(),
        value: "5".into(),
    })
    .unwrap_err();
    assert!(
        matches!(
            err,
            OmmError::Host(omm_host::HostError::SettingsRejected { .. })
        ),
        "{err}"
    );
    assert_eq!(std::fs::read(r.settings_file()).unwrap(), before);
    // An unknown key is refused before the host runs.
    let err = settings_cmd(SettingsAction::Set {
        key: "_omm".into(),
        value: "1".into(),
    })
    .unwrap_err();
    assert!(
        matches!(
            err,
            OmmError::Host(omm_host::HostError::UnknownSettingsKey(_))
        ),
        "{err}"
    );

    // D3: both spellings present → every write refuses until fixed.
    let mut planted = settings(&h);
    planted["mcpServers"] = json!({"a": {"command": "a-server", "transport": "stdio"}});
    planted["mcp_servers"] = json!({"b": {"command": "b-server", "transport": "stdio"}});
    write_settings(&h, &planted);
    let err = settings_cmd(SettingsAction::Set {
        key: "provider".into(),
        value: "meta".into(),
    })
    .unwrap_err();
    assert!(err.to_string().contains("fix-mcp-collision"), "{err}");
    settings_cmd(SettingsAction::FixMcpCollision).expect("fix collision");
    let s = settings(&h);
    assert!(s.get("mcp_servers").is_none());
    assert_eq!(s["mcpServers"]["a"]["command"], "a-server");
    assert_eq!(s["mcpServers"]["b"]["command"], "b-server");
    assert_eq!(s["provider"], "meta", "the rest of the document survived");
    let l = ledger(&h).unwrap();
    let prior = settings_key_registrations(&l, "mcpServers");
    assert_eq!(prior.len(), 1);
    assert_eq!(prior[0].as_ref().unwrap()["a"]["command"], "a-server");
    // Legacy alone → renamed; nothing → nothing to do.
    let mut legacy_only = settings(&h);
    legacy_only.as_object_mut().unwrap().remove("mcpServers");
    legacy_only["mcp_servers"] = json!({"c": {"command": "c-server", "transport": "stdio"}});
    write_settings(&h, &legacy_only);
    settings_cmd(SettingsAction::FixMcpCollision).expect("rename legacy");
    let s = settings(&h);
    assert_eq!(s["mcpServers"]["c"]["command"], "c-server");
    assert!(s.get("mcp_servers").is_none());
    settings_cmd(SettingsAction::FixMcpCollision).expect("nothing to do");
    // Conflicting definitions are refused, untouched.
    let mut conflict = settings(&h);
    conflict["mcp_servers"] = json!({"c": {"command": "other", "transport": "stdio"}});
    write_settings(&h, &conflict);
    let before = std::fs::read(r.settings_file()).unwrap();
    let err = settings_cmd(SettingsAction::FixMcpCollision).unwrap_err();
    assert!(err.to_string().contains("defined differently"), "{err}");
    assert_eq!(std::fs::read(r.settings_file()).unwrap(), before);
    conflict.as_object_mut().unwrap().remove("mcp_servers");
    write_settings(&h, &conflict);

    // D4: malformed shapes → lint exits 1, --fix repairs.
    let wired = omm_doctor::checks::wired_reminders().unwrap();
    let reminder = &wired[0];
    let mut bad = settings(&h);
    bad["plugins"] = json!({"x": 5, "y": {"enabled": "true"}});
    bad["runtime_capabilities"] =
        json!({"plugin:omm:hook:z": {"enabled": true}, "short": {"enabled": true}});
    write_settings(&h, &bad);
    let code = settings_cmd(SettingsAction::Lint { fix: false }).expect("lint");
    assert_eq!(code, std::process::ExitCode::from(1));
    assert_eq!(settings(&h), bad, "lint alone writes nothing");
    let code = settings_cmd(SettingsAction::Lint { fix: true }).expect("lint --fix");
    assert_eq!(code, std::process::ExitCode::SUCCESS);
    let s = settings(&h);
    assert_eq!(s["plugins"], json!({"y": {"enabled": true}}));
    assert_eq!(
        s["runtime_capabilities"],
        json!({"plugin:omm:hook:z": {"enabled": true}})
    );
    let l = ledger(&h).unwrap();
    assert_eq!(
        settings_key_registrations(&l, "plugins"),
        vec![Some(json!({"x": 5, "y": {"enabled": "true"}}))],
        "the whole prior map is restorable"
    );
    let code = settings_cmd(SettingsAction::Lint { fix: false }).expect("lint clean");
    assert_eq!(code, std::process::ExitCode::SUCCESS);

    // D5: the legacy line decides; the canonical store follows it.
    let mut conflict = settings(&h);
    conflict["plugins"][&reminder.plugin_id] = json!({"enabled": true});
    conflict["runtime_capabilities"][&reminder.stable_id] = json!({"enabled": false});
    write_settings(&h, &conflict);
    settings_cmd(SettingsAction::ReconcileReminders).expect("reconcile");
    let s = settings(&h);
    assert_eq!(s["plugins"][&reminder.plugin_id]["enabled"], false);
    assert_eq!(
        s["runtime_capabilities"][&reminder.stable_id]["enabled"],
        false
    );
    settings_cmd(SettingsAction::ReconcileReminders).expect("nothing to do");
    let l = ledger(&h).unwrap();
    let key = format!("plugins.{}.enabled", reminder.plugin_id);
    assert_eq!(
        settings_key_registrations(&l, &key),
        vec![Some(json!(true))]
    );
}

#[test]
fn dry_run_settings_writes_validate_but_land_nothing() {
    let Some(h) = host_with(Flags {
        dry_run: true,
        json: true,
        ..Flags::default()
    }) else {
        eprintln!("skipped: OMM_MUSE_BIN is unset");
        return;
    };
    tune::settings(
        &h.ctx,
        &SettingsArgs {
            action: SettingsAction::Set {
                key: "provider".into(),
                value: "meta".into(),
            },
        },
    )
    .expect("dry run set");
    assert_eq!(settings(&h), Value::Null);
    assert!(ledger(&h).is_none());
    assert!(!h.ctx.omm_root().join("audit.log").exists());
    tune::profile(
        &h.ctx,
        &ProfileArgs {
            action: ProfileAction::Use {
                name: "fast".into(),
                force: false,
            },
        },
    )
    .expect("dry run profile");
    assert!(!h.ctx.omm_root().join("config.json").exists());
}

// ---- scenario 8: run -------------------------------------------------------------------

#[test]
fn run_refuses_prompts_and_execs_root_flags() {
    let h = host_or_skip!();
    let (code, _, err) = run_omm(&h, &["run", "--", "zzznotacommand"]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("not a Muse command"), "{err}");
    let (code, _, err) = run_omm(&h, &["run", "--", "--provider", "echo", "hi"]);
    assert_eq!(code, Some(2), "{err}");
    let (code, _, err) = run_omm(&h, &["run", "--", "--", "hi"]);
    assert_eq!(code, Some(2), "{err}");
    let (code, _, err) = run_omm(&h, &["run"]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("TUI"), "{err}");
    // No session was started by any refusal.
    assert!(
        !roots(&h).sessions_dir().exists(),
        "a refused argv never reached the host"
    );

    let (code, out, err) = run_omm(&h, &["run", "--", "--version"]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        out.contains(omm_host::host_reality::VERSION_STRING_PRODUCT),
        "{out:?}"
    );

    // The plan under --dry-run, with the profile's gates.
    std::fs::create_dir_all(h.ctx.omm_root()).unwrap();
    std::fs::write(
        h.ctx.omm_root().join("config.json"),
        br#"{"skill_routing": true, "gates": ["plugins"]}"#,
    )
    .unwrap();
    let (code, out, err) = run_omm(&h, &["--dry-run", "--json", "run", "--", "skills", "list"]);
    assert_eq!(code, Some(0), "{err}");
    let plan: Value = serde_json::from_str(out.trim()).expect("json plan");
    assert_eq!(plan["argv"], json!(["skills", "list"]));
    let names: Vec<&str> = plan["env"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"MUSE_NO_AUTO_UPDATE"));
    assert!(names.contains(&"MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS"));
    assert!(names.contains(&"MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY"));
    assert!(names.contains(&"MUSE_EXPERIMENTAL_PLUGINS"));
    assert_eq!(plan["skill_routing"], true);
    let (code, _, err) = run_omm(&h, &["--dry-run", "run", "--", "zzz"]);
    assert_eq!(code, Some(2), "{err}");
    std::fs::write(
        h.ctx.omm_root().join("config.json"),
        br#"{"gates": ["no_such_gate"]}"#,
    )
    .unwrap();
    let (code, _, err) = run_omm(&h, &["--dry-run", "run", "--", "--version"]);
    assert_eq!(code, Some(2), "{err}");
    assert!(err.contains("no_such_gate"), "{err}");
}

// ---- scenario 9: hook ------------------------------------------------------------------

#[test]
fn hook_decisions_travel_in_json_with_exit_0() {
    let h = host_or_skip!();
    let deny = json!({"hook_event_name": "PreToolUse", "tool_name": "bash", "tool_input": {"command": "git push --force origin main"}, "tool_use_id": "t"});
    let (code, out, err, _) = run_hook(&h, "guard", deny.to_string().as_bytes());
    assert_eq!(code, Some(0), "{err}");
    assert!(err.is_empty(), "{err}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "deny");
    assert_eq!(v["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert!(v["hookSpecificOutput"]["permissionDecisionReason"]
        .as_str()
        .unwrap()
        .starts_with("omm guard: force-push-main"));
    let allow = json!({"hook_event_name": "PreToolUse", "tool_name": "bash", "tool_input": {"command": "cargo test"}});
    let (code, out, _, _) = run_hook(&h, "guard", allow.to_string().as_bytes());
    assert_eq!(code, Some(0));
    assert_eq!(out.trim(), "{}");

    let stop = json!({"hook_event_name": "Stop", "stop_hook_active": false, "last_assistant_message": "All done, the feature is implemented."});
    let (code, out, _, _) = run_hook(&h, "stop", stop.to_string().as_bytes());
    assert_eq!(code, Some(0));
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["decision"], "block");
    assert!(v["reason"].as_str().unwrap().starts_with("omm:"));

    // session-start reads the profile from $OMM/config.json under HOME
    // (the hook's env is scrubbed to HOME + PATH — hooks.md §3.6).
    let omm_root = h.sb.home.join(".config/omm");
    std::fs::create_dir_all(&omm_root).unwrap();
    std::fs::write(omm_root.join("config.json"), br#"{"profile":"fast"}"#).unwrap();
    let start = json!({"hook_event_name": "SessionStart", "source": "startup"});
    let mut cmd = omm_cmd(&h, &["hook", "session-start"]);
    cmd.env_remove("XDG_CONFIG_HOME")
        .env_remove("XDG_DATA_HOME")
        .env_remove("OMM_MUSE_BIN")
        .stdin(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(start.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let ctx = v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(ctx.starts_with("omm "), "{ctx}");
    assert!(ctx.contains("profile fast"), "{ctx}");
    assert!(ctx.contains("/32,000 B ("), "{ctx}");
    assert!(out.stderr.is_empty());
}

#[test]
fn hook_dispatch_is_under_five_milliseconds() {
    let h = host_or_skip!();
    let payload = json!({"hook_event_name": "PreToolUse", "tool_name": "bash", "tool_input": {"command": "ls -la"}}).to_string();
    let mut medians = Vec::new();
    for _round in 0..3 {
        let mut samples: Vec<Duration> = (0..21)
            .map(|_| run_hook(&h, "guard", payload.as_bytes()).3)
            .collect();
        samples.sort();
        let median = samples[samples.len() / 2];
        medians.push(median);
        eprintln!(
            "omm hook guard: median {:?}, min {:?}, max {:?}",
            median,
            samples[0],
            samples[samples.len() - 1]
        );
        if median < Duration::from_millis(5) {
            return;
        }
    }
    panic!("omm hook guard median over 5 ms in every round: {medians:?} (R16 sub-5 ms dispatch)");
}

// ---- memory path -----------------------------------------------------------------------

#[test]
fn memory_path_prints_the_personal_project_root_of_the_cwd() {
    let h = host_or_skip!();
    let ws = tempfile::tempdir().unwrap();
    let ws_canonical = ws.path().canonicalize().unwrap();
    let expected = roots(&h)
        .memory_personal_project_dir(&ws_canonical)
        .unwrap();
    let mut cmd = omm_cmd(&h, &["memory", "path"]);
    cmd.current_dir(&ws_canonical);
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(text.lines().next().unwrap(), expected.display().to_string());
    assert!(text.contains("trust: not in trust.json"), "{text}");
    let mut cmd = omm_cmd(&h, &["--json", "memory", "path"]);
    cmd.current_dir(&ws_canonical);
    let out = cmd.output().unwrap();
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["path"], expected.display().to_string());
    assert_eq!(v["exists"], false);
    assert_eq!(v["trust"], Value::Null);
    // `memory gc` (PLAN.md 2.3) in a sandbox with no memory roots at all:
    // exit 0, nothing removed, nothing kept, no snapshot dir created.
    let (code, out, err) = run_omm(&h, &["--json", "-y", "memory", "gc"]);
    assert_eq!(code, Some(0), "{err}");
    let v: Value = serde_json::from_str(&out).unwrap_or_else(|e| panic!("{out:?}: {e}"));
    assert_eq!(v["dry_run"], false, "{out}");
    assert_eq!(v["removed"], json!([]), "{out}");
    assert_eq!(v["kept"], json!([]), "{out}");
    assert_eq!(v["pruned"], json!([]), "{out}");
    let (code, out, err) = run_omm(&h, &["-y", "memory", "gc"]);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        !out.contains("removed ") && !out.contains("kept "),
        "nothing to report on an empty sandbox: {out}"
    );
    assert!(!roots(&h).snapshots_dir().join("memory").exists(), "{out}");
}
