//! The CLI's error type and its exit-code mapping.
//!
//! Exit codes: `2` = the invocation was rejected before anything ran (an
//! argv the R20 allowlist refused, a Phase-1 stub, bad user input), `1` =
//! ran and failed or refused (R6 consent, a host/ledger/manifest/doctor
//! failure). Commands that succeed with findings (`doctor` on a critical
//! check) return their exit code as a value, not as an error.

use std::process::ExitCode;

use omm_host::HostError;
use thiserror::Error;

/// Every way `omm` can fail.
#[derive(Debug, Error)]
pub enum OmmError {
    /// A Phase-1 stub (exit 2). Legacy: no command constructs this any more
    /// (locked by `tests/hook_fail_open.rs →
    /// no_command_is_a_stub_any_more_and_the_hook_never_was` and
    /// `tests/no_new_stubs.rs`); retained so the exit-code contract stays
    /// total for out-of-tree callers.
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// The host layer failed (`HostError::Argv` exits 2, the rest 1).
    #[error(transparent)]
    Host(#[from] HostError),

    /// The ledger layer failed (containment, schema, quarantine, undo).
    #[error(transparent)]
    Ledger(#[from] omm_ledger::LedgerError),

    /// The content / package layer failed (catalog, content, generator, lint).
    #[error(transparent)]
    Manifest(#[from] omm_manifest::ManifestError),

    /// A doctor context or a cost measurement could not be built.
    #[error(transparent)]
    Doctor(#[from] omm_doctor::DoctorError),

    /// `omm doctor --self-test` found drift (exit 1).
    #[error("self-test: {failed} of {total} checks failed")]
    SelfTest { failed: usize, total: usize },

    /// A local I/O failure.
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    /// R6: a write command was asked to run with nobody to answer for it —
    /// under `CI=true` or without a terminal — and no `--yes` was given.
    /// Nothing was written.
    #[error("{command}: refusing to write without a human in the loop ({reason}); pass --yes to proceed or --dry-run to preview (R6)")]
    ConsentRequired { command: String, reason: String },

    /// User input rejected before anything ran (an unknown theme, preset,
    /// feature or profile name; a value that does not parse). Exit 2.
    #[error("{0}")]
    Usage(String),
}

impl OmmError {
    /// `2` for rejections before anything ran, `1` for everything else.
    pub fn exit_code(&self) -> i32 {
        match self {
            OmmError::NotImplemented(_) | OmmError::Usage(_) => 2,
            OmmError::Host(HostError::Argv(_)) => 2,
            _ => 1,
        }
    }

    /// [`OmmError::exit_code`] as the process exit status.
    pub fn exit(&self) -> ExitCode {
        ExitCode::from(u8::try_from(self.exit_code()).unwrap_or(1))
    }

    /// An I/O failure with the operation it interrupted.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> OmmError {
        OmmError::Io {
            context: context.into(),
            source,
        }
    }
}

/// `Result` alias for the CLI.
pub type Result<T> = std::result::Result<T, OmmError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_rejected_versus_failed_split() {
        assert_eq!(OmmError::NotImplemented("x".into()).exit_code(), 2);
        assert_eq!(OmmError::Usage("bad".into()).exit_code(), 2);
        assert_eq!(OmmError::Host(HostError::Argv("zzz".into())).exit_code(), 2);
        assert_eq!(OmmError::Host(HostError::NoHome).exit_code(), 1);
        assert_eq!(
            OmmError::SelfTest {
                failed: 1,
                total: 2
            }
            .exit_code(),
            1
        );
        assert_eq!(
            OmmError::ConsentRequired {
                command: "install".into(),
                reason: "CI=true is set".into()
            }
            .exit_code(),
            1
        );
        assert_eq!(
            OmmError::io("read x", std::io::Error::other("boom")).exit_code(),
            1
        );
    }

    #[test]
    fn consent_message_names_the_way_out() {
        let e = OmmError::ConsentRequired {
            command: "install".into(),
            reason: "stdin is not a terminal".into(),
        };
        let text = e.to_string();
        assert!(text.contains("--yes") && text.contains("--dry-run") && text.contains("install"));
    }
}
