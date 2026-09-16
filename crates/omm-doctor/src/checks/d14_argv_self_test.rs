//! D14 — the exit-code matrix and the R20 argv allowlist, re-measured
//! (ARCHITECTURE.md §6 row D14; 00-DECISION.md §2.2 `omm doctor --self-test`).

use omm_host::allowlist::{validate_argv, ArgvPolicy};
use omm_host::host_reality as hr;
use omm_host::invoke::OutcomeKind;

use crate::check::{Check, Context};

pub const ID: &str = "D14";
pub const TITLE: &str = "argv self-test";
/// `docs/host-reality.md` "Exit codes": there is NO unknown-command exit — an
/// unrecognised first token is the `[PROMPT]` positional and starts a billed
/// session, and a prompt can hide behind root flags (`muse -w hi`); `2` is a
/// parse error OR a missing gate, indistinguishable. A wrapper typo would
/// therefore silently bill a turn; the allowlist (R20) and this matrix are
/// what stand in the way.
pub const WHY_SILENT: &str = "the host has no unknown-command exit: an unrecognised first token (or a bare token behind root flags) is the [PROMPT] positional and silently starts a billed session, and exit 2 means parse error OR missing gate — so a wrapper typo bills a turn with no error; only the R20 allowlist and this re-measured matrix prevent it (host-reality.md \"Exit codes\"; muse-cli.json unknown_token_rule)";

pub fn run(ctx: &Context) -> Check {
    let mut failed: Vec<String> = Vec::new();
    let mut passed = 0usize;
    let mut record = |name: &str, ok: bool, detail: String| {
        if ok {
            passed += 1;
        } else {
            failed.push(format!("{name}: {detail}"));
        }
    };

    // Allowlist sanity — pure, from the data file (R8).
    match hr::muse_cli() {
        Ok(cli) => {
            let all_ok = cli
                .argv_allowlist
                .iter()
                .all(|c| validate_argv(&[c.as_str()], ArgvPolicy::STRICT).is_ok());
            record(
                "allowlist/16-commands-pass",
                all_ok && cli.argv_allowlist.len() == hr::TOP_LEVEL_COMMANDS,
                format!("{} commands", cli.argv_allowlist.len()),
            );
        }
        Err(e) => record("allowlist/data", false, e.to_string()),
    }
    let refused: [&[&str]; 6] = [
        &["zzznotacommand"],
        &["--"],
        &["-"],
        &["--provider", "echo", "hi"],
        &["-w", "hi"],
        &["--provider", "echo", "exec", "hi"],
    ];
    for argv in refused {
        record(
            "allowlist/prompt-refused",
            validate_argv(argv, ArgvPolicy::ROOT_FLAGS_OK).is_err(),
            format!("{argv:?} must be refused"),
        );
    }
    record(
        "allowlist/root-flags-need-opt-in",
        validate_argv(&["--version"], ArgvPolicy::STRICT).is_err()
            && validate_argv(&["--version"], ArgvPolicy::ROOT_FLAGS_OK).is_ok()
            && validate_argv(&["-w", "off"], ArgvPolicy::ROOT_FLAGS_OK).is_ok(),
        "--version strict/root-flags, -w off".to_string(),
    );
    match hr::verify_data() {
        Ok(problems) if problems.is_empty() => record("data/consistent", true, String::new()),
        Ok(problems) => record("data/consistent", false, problems.join("; ")),
        Err(e) => record("data/consistent", false, e.to_string()),
    }

    // The exit-code matrix, against the binary.
    let rows: [(&str, &[&str], bool, OutcomeKind); 4] = [
        (
            "exit/unknown-root-flag",
            &["--zzznotaflag"],
            true,
            OutcomeKind::ArgvRejected,
        ),
        (
            "exit/unknown-subcommand",
            &["skills", "zzznotaverb"],
            false,
            OutcomeKind::ArgvRejected,
        ),
        ("exit/version", &["--version"], true, OutcomeKind::Ok),
        (
            "exit/plugins-gated-ok",
            &["plugins", "list"],
            false,
            OutcomeKind::Ok,
        ),
    ];
    for (name, argv, root_flags, expected) in rows {
        let out = if root_flags {
            ctx.inv.clone().allow_root_flags().run(argv)
        } else {
            ctx.inv.run(argv)
        };
        match out {
            Ok(o) => record(
                name,
                o.kind == expected,
                format!("expected {expected:?}, observed {:?}", o.kind),
            ),
            Err(e) => record(name, false, e.to_string()),
        }
    }
    match ctx
        .inv
        .clone()
        .without_plugins_gate()
        .run(&["plugins", "list"])
    {
        Ok(o) => record(
            "exit/plugins-ungated",
            o.code == Some(hr::EXIT_ARGV_REJECTED),
            format!(
                "expected exit {}, observed {:?}",
                hr::EXIT_ARGV_REJECTED,
                o.code
            ),
        ),
        Err(e) => record("exit/plugins-ungated", false, e.to_string()),
    }
    match tempfile::Builder::new().prefix("omm-doctor-d14-").tempdir() {
        Ok(tmp) => {
            let file = tmp.path().join("bad.json");
            let written = std::fs::write(&file, b"{\"schema_version\":2}");
            if let Err(e) = written {
                record("exit/config-validate-invalid", false, e.to_string());
            } else {
                match ctx.inv.run(&[
                    "config".to_string(),
                    "validate".to_string(),
                    "--plane".to_string(),
                    "defaults".to_string(),
                    "--file".to_string(),
                    file.to_string_lossy().into_owned(),
                ]) {
                    Ok(o) => record(
                        "exit/config-validate-invalid",
                        o.kind == OutcomeKind::RunFailed,
                        format!("expected RunFailed, observed {:?}", o.kind),
                    ),
                    Err(e) => record("exit/config-validate-invalid", false, e.to_string()),
                }
            }
        }
        Err(e) => record("exit/config-validate-invalid", false, e.to_string()),
    }

    let total = passed + failed.len();
    if failed.is_empty() {
        Check::info(
            ID,
            TITLE,
            format!("{total} rows pass: exit 0/1/2 matrix and the {}-command allowlist behave as measured", hr::TOP_LEVEL_COMMANDS),
            WHY_SILENT,
        )
    } else {
        Check::critical(
            ID,
            TITLE,
            format!("{} of {total} rows failed: {}", failed.len(), failed.join("; ")),
            WHY_SILENT,
            "omm doctor --self-test   # then report the drift: the binary's exit-code contract moved",
        )
    }
}
