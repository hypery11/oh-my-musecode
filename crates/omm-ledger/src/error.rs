//! The one error type of `omm-ledger`.
//!
//! Every variant names what was refused and why; the message is what `omm`
//! prints, so it says what to do next where a fix exists.

use std::path::PathBuf;

use omm_host::HostError;
use thiserror::Error;

use crate::schema::Base;

/// Errors produced by `omm-ledger`.
#[derive(Debug, Error)]
pub enum LedgerError {
    /// A filesystem primitive of `omm_host::fsx` (atomic write, lock,
    /// snapshot, canonicalize) failed.
    #[error(transparent)]
    Host(#[from] HostError),

    /// A local filesystem operation failed.
    #[error("{context} {path}: {source}")]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A base-relative path failed the grammar (`schema::RelPath`): empty,
    /// absolute, a backslash, a `.`/`..`/empty component, NUL, a trailing
    /// slash or a drive letter.
    #[error("invalid ledger path {path:?}: {reason}")]
    InvalidPath { path: String, reason: String },

    /// R4: the resolved path is not strictly inside its base, or is one of
    /// the refused roots (`/`, `$HOME`, a base root itself).
    #[error("path {path:?} refused under base {base}: {reason}")]
    Containment {
        base: Base,
        path: String,
        reason: String,
    },

    /// No root is known for a base — the workspace base without a workspace.
    #[error("no root known for ledger base {0}; pass the workspace to Bases::from_roots")]
    NoBaseRoot(Base),

    /// The ledger document (or one being saved) violates the schema.
    #[error("ledger {path}: {detail}")]
    Schema { path: PathBuf, detail: String },

    /// A version label that cannot be a single directory name under
    /// `updates/` or `snapshots/`.
    #[error("invalid version label {0:?}: must be one path component of [A-Za-z0-9._+-]")]
    BadVersionLabel(String),

    /// The uninstall plan cannot be applied as it stands.
    #[error("uninstall refused: {0}")]
    Refused(String),

    /// A host-side undo step (plugin / marketplace / skill removal, settings
    /// or trust restore) failed.
    #[error("undo of {step} failed: {detail}")]
    Undo { step: String, detail: String },
}

/// `Result` alias used throughout the crate.
pub type Result<T> = std::result::Result<T, LedgerError>;

impl LedgerError {
    /// True for a containment refusal whose path canonicalizes outside its
    /// base (`containment::ESCAPE_REASON_PREFIX`) — as opposed to one that
    /// lands on `/`, `$HOME` or a base root.
    pub fn is_escape(&self) -> bool {
        matches!(
            self,
            LedgerError::Containment { reason, .. }
                if reason.starts_with(crate::containment::ESCAPE_REASON_PREFIX)
        )
    }

    pub(crate) fn io(
        context: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        LedgerError::Io {
            context,
            path: path.into(),
            source,
        }
    }
}
