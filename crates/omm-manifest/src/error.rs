//! The one error type of `omm-manifest`. Every variant names the file or rule
//! it protects; the message is what `omm build` / `omm lint` print.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced by `omm-manifest`.
#[derive(Debug, Error)]
pub enum ManifestError {
    /// Anything `omm-host` refused (paths, invocation, host output).
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

    /// A JSON file did not parse.
    #[error("{path}: invalid JSON: {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    /// `content/catalog.json` violates §5.1 (schema, duplicate id, core gate,
    /// alias forwarding).
    #[error("catalog {path}: {detail}")]
    Catalog { path: PathBuf, detail: String },

    /// A content file cannot be read into the model at all (the soft problems
    /// are lint findings, see `content::Content::problems`).
    #[error("content {path}: {detail}")]
    Content { path: PathBuf, detail: String },

    /// The generator refuses to run over content with unresolved problems.
    #[error("content has {count} problem(s); run `omm lint` — first: {first}")]
    ContentProblems { count: usize, first: String },

    /// A path resolved outside the root it must stay under.
    #[error("{path} escapes {root}")]
    Containment { path: PathBuf, root: PathBuf },

    /// R12: the packer never follows or emits a symlink.
    #[error("{path} is a symlink; omm never packages or follows one (R12) — replace it with a regular file")]
    Symlink { path: PathBuf },

    /// A generator invariant failed.
    #[error("generate: {0}")]
    Generate(String),

    /// The host's output did not have the shape the research recorded.
    #[error("host output: {0}")]
    HostShape(String),

    /// `crates/omm/Cargo.toml` (or the workspace manifest) lacks what we read.
    #[error("version: {0}")]
    Version(String),

    /// `$OMM/config.json` or an overlay entry is malformed.
    #[error("overlay {path}: {detail}")]
    Overlay { path: PathBuf, detail: String },
}

/// `Result` alias used throughout the crate.
pub type Result<T> = std::result::Result<T, ManifestError>;

impl ManifestError {
    pub(crate) fn io(
        context: &'static str,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        ManifestError::Io {
            context,
            path: path.into(),
            source,
        }
    }
}
