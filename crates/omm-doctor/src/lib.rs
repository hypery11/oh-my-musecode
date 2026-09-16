//! `omm-doctor` — checks and cost (ARCHITECTURE.md §6).
//!
//! Every check carries an `id`, a `severity`, what was observed, why it is
//! silent at session time, and the exact fix command; `omm doctor` exits
//! non-zero on any `critical` ([`Report::exit_code`]). Every check exists
//! because something fails silently at session time
//! (`research/experiments/00-DECISION.md` next action 2). Doctor compares what
//! **composed in a live session** against what was installed — never disk
//! against disk (`docs/host-reality.md` "Server-side risk"): the D8 catalog
//! measurement and `omm cost` run one real `muse exec --provider echo hi` in a
//! fresh trusted workspace against a throwaway data root seeded with the
//! host's plugin store ([`session`]).
//!
//! Module map: [`check`] (the `Check` / `Report` types and the shared
//! [`Context`]), [`checks`] (D1–D15, one module each), [`cost`] (`omm cost`),
//! [`catalog`] (the order-200 `skills_catalog` parser), [`session`] (the live
//! echo-session measurement), [`ledger`] (the read-only view of
//! `omm.lock.json` D13 needs), [`report`] (rendering).

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub mod catalog;
pub mod check;
pub mod checks;
pub mod cost;
pub mod error;
pub mod ledger;
pub mod report;
pub mod session;

pub use check::{Check, Context, HostInfo, Options, Report, Severity};
pub use error::{DoctorError, Result};

/// Run every check (D1–D15, in order) and build the report.
pub fn run(ctx: &Context) -> Report {
    checks::run_all(ctx)
}
