//! `omm` — the CLI (ARCHITECTURE.md §7) as a library, so the binary in
//! `main.rs` is a one-line wrapper and the integration tests under `tests/`
//! can drive a command in-process against a sandboxed [`cmd::Ctx`].
//!
//! Module map: [`cli`] (the clap surface — globals and the command table),
//! [`cmd`] (the shared [`cmd::Ctx`], the dispatch table, and one file per
//! command group), [`output`] (human / `--json` printing, the column table,
//! the converge summary), [`error`] (the one error type and its exit codes).

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub mod cli;
pub mod cmd;
pub mod error;
pub mod output;

pub use cli::{Cli, Command};
pub use cmd::{dispatch, Ctx, Flags, OMM_VERSION};
pub use error::{OmmError, Result};
pub use output::{Action, Converge, Counts, Output, Render, Table};
