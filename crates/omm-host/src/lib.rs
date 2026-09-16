//! `omm-host` — the Muse binary as a dependency (ARCHITECTURE.md §3).
//!
//! Everything omm knows about the host lives here: where its files are
//! ([`paths`]), how to find the binary ([`locate`]), how to run it under a
//! controlled environment ([`invoke`]) behind the R20 argv allowlist
//! ([`allowlist`]), how to observe its behaviour instead of its version
//! ([`probe`]), how to edit the two files omm shares with it
//! ([`settings`], [`trust`]), what a killed host leaves behind outside every
//! sandbox and how to sweep it ([`residue`]), and the golden facts every other
//! crate builds against ([`host_reality`]) together with the self-test that
//! re-measures them ([`hostcheck`]).
//!
//! Every host fact in this crate carries a doc comment naming the report under
//! `research/` it was measured in. Nothing here gates on a version number
//! (R15) and nothing here names an omm asset (R8).

#![forbid(unsafe_code)]
#![cfg_attr(not(test), deny(clippy::unwrap_used))]
#![warn(missing_debug_implementations)]

pub mod allowlist;
pub mod error;
pub mod fixtures;
pub mod fsx;
pub mod host_reality;
pub mod hostcheck;
pub mod invoke;
pub mod locate;
pub mod paths;
pub mod probe;
pub mod residue;
pub mod settings;
pub mod trust;

pub use error::{HostError, Result};
pub use invoke::{Invoker, Outcome, OutcomeKind, Sandbox};
pub use locate::{locate, Located};
pub use paths::Roots;
