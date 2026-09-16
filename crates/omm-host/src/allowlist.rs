//! R20 — the argv allowlist.
//!
//! There is **no unknown-command exit** in Muse: an unrecognised first token is
//! the `[PROMPT]` positional and silently starts a billed session
//! (`research/experiments/loose-ends.md` §1.1, `docs/host-data/muse-cli.json`
//! `unknown_token_rule`). So every argv omm passes to the binary is checked
//! here first, against the 17 top-level commands loaded from
//! `docs/host-data/muse-cli.json` — nothing is hand-copied into Rust (R8).
//!
//! A flag-first argv is the root/TUI parser (`muse [OPTIONS] [PROMPT]`), and
//! the prompt can hide *behind* the flags: `muse --provider echo hi`,
//! `muse -- hi`, a lone `muse -` and `muse -w hi` all start the TUI with `hi`
//! (or stdin) as the prompt (muse-cli.json `root_flag_walk_rule`, measured
//! 2026-09-02). So when root flags are allowed the whole argv is walked with
//! each flag's value arity from `root_flags.items` — a `<VALUE>` flag takes
//! the next token, a `[<VALUE>]` flag takes it only when it is one of the
//! flag's listed `values` (`-w off`, never `-w hi`) — and any bare token left
//! over is refused as the `[PROMPT]` unless the caller opted in with
//! [`ArgvPolicy::allow_prompt`].

use crate::error::{HostError, Result};
use crate::host_reality::{self, RootFlagArity};

/// How strict to be about a first token that is a flag, and about a prompt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArgvPolicy {
    /// Allow an argv whose first token is a flag (`--version`, `--help`, …).
    /// Off by default because `muse --provider echo` *starts the TUI*; only
    /// `omm run` opts in on the user's explicit behalf.
    pub allow_root_flags: bool,
    /// Allow a bare token to remain after the root flags — the `[PROMPT]`
    /// positional, which submits that text to a billed session. Off in every
    /// built-in policy; a caller opts in only when the user typed the prompt
    /// on purpose. `--` and a lone `-` are refused regardless: they hand the
    /// rest of argv (or stdin) to the prompt with no way to tell.
    pub allow_prompt: bool,
}

impl ArgvPolicy {
    /// The strict default: a top-level command must come first.
    pub const STRICT: ArgvPolicy = ArgvPolicy {
        allow_root_flags: false,
        allow_prompt: false,
    };
    /// Root flags allowed first (`omm run -- --version`, `omm run -- --provider echo`);
    /// a trailing prompt is still refused.
    pub const ROOT_FLAGS_OK: ArgvPolicy = ArgvPolicy {
        allow_root_flags: true,
        allow_prompt: false,
    };
    /// Root flags and an explicit prompt (`omm run --prompt -- --provider echo hi`).
    pub const PROMPT_OK: ArgvPolicy = ArgvPolicy {
        allow_root_flags: true,
        allow_prompt: true,
    };
}

/// The 17 top-level commands (`docs/host-data/muse-cli.json` → `argv_allowlist`).
pub fn top_level_commands() -> Result<&'static [String]> {
    Ok(&host_reality::muse_cli()?.argv_allowlist)
}

/// True when `token` is one of the 17 top-level commands.
pub fn is_top_level_command(token: &str) -> Result<bool> {
    Ok(top_level_commands()?.iter().any(|c| c == token))
}

/// What the host would make of a first token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstToken {
    /// One of the 17 commands.
    Command,
    /// A `-`/`--` flag — the root/TUI parser.
    RootFlag,
    /// `--` (everything after it is the prompt) or a lone `-` (a prompt too).
    PromptMarker,
    /// Anything else: the `[PROMPT]` positional.
    Prompt,
}

/// Classify a first token.
pub fn classify(token: &str) -> Result<FirstToken> {
    if token == "--" || token == "-" {
        return Ok(FirstToken::PromptMarker);
    }
    if token.starts_with('-') {
        return Ok(FirstToken::RootFlag);
    }
    if is_top_level_command(token)? {
        return Ok(FirstToken::Command);
    }
    Ok(FirstToken::Prompt)
}

/// Validate a full argv (without the program name).
pub fn validate_argv<S: AsRef<str>>(argv: &[S], policy: ArgvPolicy) -> Result<()> {
    let Some(first) = argv.first() else {
        return Err(HostError::Argv(
            "empty argv would start the interactive TUI (muse [OPTIONS] [PROMPT])".to_string(),
        ));
    };
    let first = first.as_ref();
    match classify(first)? {
        FirstToken::Command => Ok(()),
        FirstToken::PromptMarker => Err(prompt_marker_error(first)),
        FirstToken::RootFlag if policy.allow_root_flags => walk_root_flags(argv, policy),
        FirstToken::RootFlag => Err(HostError::Argv(format!(
            "`{first}` is a root flag; a flag-first invocation runs the TUI unless explicitly allowed"
        ))),
        FirstToken::Prompt => Err(HostError::Argv(format!(
            "`{first}` is not a Muse command — the host would treat it as a prompt and start a billed session; known commands: {}",
            top_level_commands()?.join(" ")
        ))),
    }
}

fn prompt_marker_error(token: &str) -> HostError {
    HostError::Argv(format!(
        "`{token}` hands the rest of argv (or stdin) to the host as the [PROMPT] positional and starts a billed session; omm never passes it"
    ))
}

/// Walk a flag-first argv with each root flag's value arity
/// (`docs/host-data/muse-cli.json` → `root_flags.items`), refusing `--`, `-`,
/// a command in prompt position (the host refuses those too, exit 2) and any
/// bare token — the `[PROMPT]` — unless `policy.allow_prompt`. The root
/// positional `resume` ends the walk: what follows belongs to its parser.
///
/// A `<VALUE>` flag always takes the next token; a `[<VALUE>]` flag takes it
/// only when it is one of the flag's listed values — measured on
/// 1.0.1-R2006.1 in a pty: `muse -w off zebra` and `muse -w create zebra`
/// consume the mode and submit `zebra` as the prompt, `muse -w zebra` creates
/// the worktree (bare `-w` = create) and submits `zebra` too, while `-w=hi`
/// and `--worktree=hi` are exit 2 with no session. An attached `=value` is
/// therefore never a prompt whatever it says; a detached one is a prompt
/// unless the host would accept it as the flag's value.
fn walk_root_flags<S: AsRef<str>>(argv: &[S], policy: ArgvPolicy) -> Result<()> {
    let cli = host_reality::muse_cli()?;
    let mut i = 0;
    while i < argv.len() {
        let tok = argv[i].as_ref();
        if tok == "--" || tok == "-" {
            return Err(prompt_marker_error(tok));
        }
        if tok.starts_with('-') {
            let (name, attached) = match tok.split_once('=') {
                Some((n, v)) => (n, Some(v)),
                None => (tok, None),
            };
            // A host-internal `--internal-…` flag (muse-cli.json
            // `root_flags.internal_root_flags`) is a bare switch; an unknown
            // flag is a parse error on the host (exit 2, no session) and is
            // walked as a bare switch too.
            let flag = cli.root_flag(name);
            let arity = cli.root_flag_arity(name).unwrap_or(RootFlagArity::None);
            i += 1;
            if attached.is_none() {
                match arity {
                    RootFlagArity::One => i += 1,
                    RootFlagArity::Optional => {
                        let next = argv.get(i).map(AsRef::as_ref);
                        if let (Some(f), Some(next)) = (flag, next) {
                            if f.accepts_value(next) {
                                i += 1;
                            }
                        }
                    }
                    RootFlagArity::None => {}
                }
            }
            continue;
        }
        if cli.is_root_positional_command(tok) {
            return Ok(());
        }
        if is_top_level_command(tok)? {
            return Err(HostError::Argv(format!(
                "root flags cannot precede `{tok}` (the host refuses it: `{tok}` is a command, not a root option); put the command first"
            )));
        }
        if policy.allow_prompt {
            return Ok(());
        }
        return Err(HostError::Argv(format!(
            "`{tok}` after the root flags is the [PROMPT] positional — the host would start a billed session with it; pass it only with an explicit prompt opt-in"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seventeen_commands_from_data() {
        let cmds = top_level_commands().unwrap();
        assert_eq!(cmds.len(), 17);
        for c in [
            "resume",
            "exec",
            "plugins",
            "workflows",
            "skills",
            "config",
            "mcp",
        ] {
            assert!(is_top_level_command(c).unwrap(), "{c}");
        }
        assert!(!is_top_level_command("doctor").unwrap());
        assert!(!is_top_level_command("zzznotacommand").unwrap());
    }

    #[test]
    fn empty_and_prompt_refused() {
        let empty: [&str; 0] = [];
        assert!(matches!(
            validate_argv(&empty, ArgvPolicy::STRICT),
            Err(HostError::Argv(_))
        ));
        assert!(matches!(
            validate_argv(&["zzznotacommand"], ArgvPolicy::ROOT_FLAGS_OK),
            Err(HostError::Argv(_))
        ));
        assert!(validate_argv(&["skills", "list"], ArgvPolicy::STRICT).is_ok());
    }

    #[test]
    fn prompt_cannot_hide_behind_root_flags() {
        // Measured on 1.0.1-R2006.1 (stdin=/dev/null): every one of these
        // starts the TUI with the trailing token as the prompt.
        for argv in [
            vec!["--", "hi"],
            vec!["--provider", "echo", "hi"],
            vec!["--provider", "echo", "--", "hi"],
            vec!["-"],
            vec!["--trust-workspace", "hi"],
            vec!["-w", "off", "hi"],
            // A `[<VALUE>]` flag takes the next token only when it is one of
            // its values: `muse -w hi` submits `hi` as the prompt (pty, 8
            // screen mentions, 5 in session.jsonl; `-w=hi` is exit 2).
            vec!["-w", "hi"],
            vec!["-w", "OFF"],
            vec!["--worktree", "hi"],
            vec!["-w", "off", "-w", "hi"],
        ] {
            assert!(
                matches!(
                    validate_argv(&argv, ArgvPolicy::ROOT_FLAGS_OK),
                    Err(HostError::Argv(_))
                ),
                "{argv:?} must be refused"
            );
        }
        // Root flags alone (the TUI on the user's explicit behalf) stay allowed,
        // and so do the value-taking forms the host accepts.
        for argv in [
            vec!["--version"],
            vec!["--provider", "echo"],
            vec!["--provider=echo", "--version"],
            vec!["-w"],
            vec!["-w", "--version"],
            vec!["-w", "off"],
            vec!["-w", "create", "--version"],
            vec!["-w", "off", "--version"],
            vec!["-w=off", "--version"],
            vec!["--worktree=off", "--version"],
            vec!["--worktree", "existing", "--worktree-existing", "/x"],
            vec!["--zzznotaflag"],
            vec!["--provider", "echo", "resume", "--last"],
        ] {
            assert!(
                validate_argv(&argv, ArgvPolicy::ROOT_FLAGS_OK).is_ok(),
                "{argv:?} must pass"
            );
        }
    }

    #[test]
    fn prompt_marker_and_command_after_flags() {
        // `--` and `-` are refused even where a prompt is allowed.
        for argv in [vec!["--"], vec!["-"], vec!["--provider", "echo", "--"]] {
            assert!(
                validate_argv(&argv, ArgvPolicy::PROMPT_OK).is_err(),
                "{argv:?}"
            );
        }
        // A command in prompt position is a host parse error; refused with a hint.
        match validate_argv(
            &["--provider", "echo", "exec", "hi"],
            ArgvPolicy::ROOT_FLAGS_OK,
        ) {
            Err(HostError::Argv(msg)) => assert!(msg.contains("put the command first"), "{msg}"),
            other => panic!("{other:?}"),
        }
        // The explicit opt-in lets a prompt through, but never `--`.
        assert!(validate_argv(&["--provider", "echo", "hi"], ArgvPolicy::PROMPT_OK).is_ok());
        // `hi` is not one of -w's values, so the host leaves it to the prompt.
        assert!(
            matches!(
                validate_argv(&["-w", "hi"], ArgvPolicy::ROOT_FLAGS_OK),
                Err(HostError::Argv(_))
            ),
            "`hi` after -w is the prompt, not -w's value"
        );
        assert!(validate_argv(&["-w", "hi"], ArgvPolicy::PROMPT_OK).is_ok());
        // An attached value never reaches the prompt whatever it is (`-w=hi`
        // is a host parse error, exit 2, no session).
        assert!(validate_argv(&["-w=hi"], ArgvPolicy::ROOT_FLAGS_OK).is_ok());
        assert!(
            validate_argv(&["--provider"], ArgvPolicy::ROOT_FLAGS_OK).is_ok(),
            "missing value: host exit 2"
        );
        assert_eq!(classify("--").unwrap(), FirstToken::PromptMarker);
        assert_eq!(classify("-").unwrap(), FirstToken::PromptMarker);
        assert_eq!(classify("-V").unwrap(), FirstToken::RootFlag);
    }

    #[test]
    fn root_flags_need_explicit_permission() {
        assert!(validate_argv(&["--version"], ArgvPolicy::STRICT).is_err());
        assert!(validate_argv(&["--version"], ArgvPolicy::ROOT_FLAGS_OK).is_ok());
        assert!(validate_argv(&["-V"], ArgvPolicy::ROOT_FLAGS_OK).is_ok());
    }

    #[test]
    fn internal_root_flags_walk_as_switches_from_the_data_file() {
        // muse-cli.json `root_flags.internal_root_flags`: parsed by nothing
        // before; now the walker knows them as bare switches, so a token
        // after one is still the [PROMPT] and refused.
        let internal = host_reality::muse_cli().unwrap().internal_root_flags();
        assert_eq!(internal.len(), 2);
        for flag in internal {
            assert!(
                validate_argv(&[flag], ArgvPolicy::ROOT_FLAGS_OK).is_ok(),
                "{flag} alone is a switch"
            );
            assert!(
                validate_argv(&[flag, "--version"], ArgvPolicy::ROOT_FLAGS_OK).is_ok(),
                "{flag} consumes no value"
            );
            assert!(
                matches!(
                    validate_argv(&[flag, "hi"], ArgvPolicy::ROOT_FLAGS_OK),
                    Err(HostError::Argv(_))
                ),
                "`hi` after {flag} is the prompt"
            );
            assert!(validate_argv(&[flag], ArgvPolicy::STRICT).is_err());
        }
    }
}
