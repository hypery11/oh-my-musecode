//! `omm-ledger` — the ownership ledger (ARCHITECTURE.md §4).
//!
//! R1: the ledger is the primitive. `install`, `update`, `doctor` and
//! `uninstall` are the same traversal over `$XDG_CONFIG_HOME/omm/omm.lock.json`.
//! R2: it records what the installer wrote, never a scan of the destination.
//!
//! * [`schema`] — the document: [`Ledger`], [`Entry`], [`Registration`], the
//!   base-relative [`RelPath`].
//! * [`store`] — load (with `.bad-<ts>` quarantine of a corrupt file), atomic
//!   save, the `locks/ledger.lock` flock around load-modify-save.
//! * [`containment`] — the four bases and `canonicalize()` + `strip_prefix()`
//!   containment (R4).
//! * [`hash`] — SHA-256 of files and trees, and the non-regular sentinel (R2).
//! * [`reconcile`] — the R3 three-way merge: no-op / overwrite / adopt / stage.
//! * [`snapshot`] — rolling pre-update snapshots (keep 5).
//! * [`audit`] — the append-only JSONL audit log.
//! * [`rules`] — the marked-region text logic of the rules file, shared by
//!   the installer (seed / re-seed) and [`uninstall`] (restore / strip).
//! * [`shared`] — the pre-omm bytes and mode of a shared file
//!   (`settings.json`, `trust.json`) kept in its entry, and the merge that
//!   puts the user's untyped keys back after a host rewrite.
//! * [`uninstall`] — the R4 planner and its executor.
//!
//! Nothing here names an asset (R8), gates on a host version (R15) or writes
//! a file non-atomically; every host fact carries its research citation.

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub mod audit;
pub mod containment;
pub mod error;
pub mod hash;
pub mod reconcile;
pub mod rules;
pub mod schema;
pub mod shared;
pub mod snapshot;
pub mod store;
pub mod uninstall;
pub mod uninstall_keys;

pub use containment::{Bases, Resolved, State};
pub use error::{LedgerError, Result};
pub use hash::Observed;
pub use schema::{
    Base, Class, Entry, EntryKey, HostInfo, Kind, Ledger, Mechanism, Registration, RelPath, Scope,
    SCHEMA_VERSION,
};
pub use store::{Loaded, Quarantined};
