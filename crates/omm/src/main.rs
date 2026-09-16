//! `omm` — the CLI binary (ARCHITECTURE.md §7). Everything lives in the
//! library (`lib.rs`); this is parse → dispatch → exit code.
//!
//! Every subcommand is implemented and dispatched to its group module
//! (`cmd/{lifecycle,inspect,tune}.rs`); `tests/hook_fail_open.rs` locks that
//! no command is a stub any more. `hook` never was one: exit 2 is the one
//! blocking hook exit code (R16), so it fails open (`{}`, exit 0).
//! `OmmError::NotImplemented` survives only as a legacy variant (exit 2)
//! that dispatch never constructs; its display arm below stays so the
//! mapping is total.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]

use std::process::ExitCode;

use clap::Parser;

use omm::{Cli, OmmError};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json = cli.json;
    match omm::dispatch(cli) {
        Ok(code) => code,
        Err(e) => {
            report_error(json, &e);
            e.exit()
        }
    }
}

/// One line on stderr; under `--json` a `{"error", "exit_code"}` object so
/// a caller parsing stdout never sees prose there.
fn report_error(json: bool, e: &OmmError) {
    match e {
        OmmError::NotImplemented(cmd) => eprintln!("not implemented: {cmd}"),
        _ if json => eprintln!(
            "{}",
            serde_json::json!({"error": e.to_string(), "exit_code": e.exit_code()})
        ),
        _ => eprintln!("omm: {e}"),
    }
}
