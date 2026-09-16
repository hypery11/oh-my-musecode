//! The one error type of `omm-doctor`. Checks never abort a run — a probe
//! failure becomes a row of the report — so this type surfaces only from the
//! entry points that must build something (a `Context`, a cost report).

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by `omm-doctor`.
#[derive(Debug, Error)]
pub enum DoctorError {
    /// The host crate refused or failed.
    #[error(transparent)]
    Host(#[from] omm_host::HostError),

    /// A filesystem operation failed.
    #[error("{context} {path}: {source}")]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Host output (or a data file) did not have the shape the research recorded.
    #[error("could not parse {what}: {detail}")]
    Parse { what: &'static str, detail: String },

    /// A live measurement could not be taken.
    #[error("{0}")]
    Measure(String),
}

/// `Result` alias used throughout the crate.
pub type Result<T> = std::result::Result<T, DoctorError>;

impl DoctorError {
    pub(crate) fn io(
        context: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        DoctorError::Io {
            context,
            path: path.into(),
            source,
        }
    }
}
