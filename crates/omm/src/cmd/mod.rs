//! Command bodies, one file per group so three authors never edit the same
//! file (PLAN.md 1.5):
//!
//! | file          | commands                                                          |
//! |---------------|-------------------------------------------------------------------|
//! | [`lifecycle`] | install, uninstall, update, reconcile, trust, list                |
//! | [`inspect`]   | doctor, cost, lint, build                                         |
//! | [`mcp`]       | mcp — the stdio MCP server (PLAN.md 2.2; [`mcp_wire`] is its framing) |
//! | [`tune`]      | theme, keymap, profile, settings, memory, enable, disable, run, hook |
//!
//! Each group file also owns the `clap::Args` structs of its commands —
//! `cli.rs` only names them — so a new flag never touches `cli.rs`. This file
//! holds the two shared pieces: the [`Ctx`] every body receives and the
//! [`dispatch`] table. Both are complete for Phase 1.
//!
//! Conventions every body follows:
//! - return `Ok(ExitCode)` for "ran" (a doctor with a critical row is
//!   `Ok(ExitCode::from(1))`), `Err` for "failed / refused" (`error.rs`);
//! - a write command calls [`Ctx::require_consent`] before its first write
//!   (R6), honours `ctx.dry_run`, and ends with `ctx.out.report(&converge)`
//!   (`output::Converge`, ARCHITECTURE.md §7 "Global");
//! - a read command ends with one `ctx.out.emit(..)` / `ctx.out.report(..)`;
//! - the host is reached only through [`Ctx::invoker`]; the roots only
//!   through `ctx.roots` (never `~/.config/muse` spelled by hand).

pub mod inspect;
pub mod lifecycle;
pub mod mcp;
pub mod mcp_wire;
pub mod tune;

use std::cell::OnceCell;
use std::ffi::OsStr;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;

use omm_host::{fsx, host_reality as hr, Invoker, Roots};

use crate::cli::{Cli, Command};
use crate::error::{OmmError, Result};
use crate::output::Output;

/// omm's own version — the ledger's `omm_version`, the audit actor
/// (`omm <version>`), the session-start context line.
pub const OMM_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The global flags of [`Cli`] as a plain value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    pub json: bool,
    pub dry_run: bool,
    pub yes: bool,
    pub verbose: bool,
}

impl Flags {
    pub fn from_cli(cli: &Cli) -> Flags {
        Flags {
            json: cli.json,
            dry_run: cli.dry_run,
            yes: cli.yes,
            verbose: cli.verbose,
        }
    }
}

/// What every command body receives.
///
/// The host binary is located lazily ([`Ctx::invoker`]): `omm hook` must
/// answer in under 5 ms and never runs the host (R16), and the hook's
/// environment is the host's 16-key scrubbed allow-list — `HOME` and `PATH`
/// pass, `XDG_*` and `OMM_MUSE_BIN` do not (`research/musecode/hooks.md`
/// §3.6) — so a context that insisted on locating the binary up front would
/// turn every hook into a no-op in a sandboxed session.
#[derive(Debug)]
pub struct Ctx {
    /// The host's config/data roots and omm's own root (ARCHITECTURE.md §2).
    pub roots: Roots,
    /// `--json`: stdout carries one JSON document.
    pub json: bool,
    /// `--dry-run`: plan and report, write nothing.
    pub dry_run: bool,
    /// `--yes`: proceed without a terminal (R6).
    pub yes: bool,
    /// `--verbose`: trace every host process; `out.note` prints.
    pub verbose: bool,
    /// The terminal.
    pub out: Output,
    invoker: OnceCell<Invoker>,
}

impl Ctx {
    /// The user's machine: roots from the process environment (one passwd
    /// lookup for the residue root, ~20 ms, `paths::account_home`).
    pub fn from_env(flags: Flags) -> Result<Ctx> {
        Ok(Ctx::new(Roots::from_env()?, flags))
    }

    /// The spawn-free context of `omm hook` (R16): roots without the passwd
    /// lookup (`Roots::from_env_fast`) and a silent [`Output`] — the hook
    /// prints its one JSON line itself and its stderr stays empty.
    pub fn fast(flags: Flags) -> Result<Ctx> {
        let mut ctx = Ctx::new(Roots::from_env_fast()?, flags);
        ctx.out = Output::silent();
        Ok(ctx)
    }

    /// Explicit roots (tests, sandboxes: `Ctx::new(sandbox.roots()?, flags)
    /// .with_invoker(Invoker::new(bin).sandboxed(&sandbox))`).
    pub fn new(roots: Roots, flags: Flags) -> Ctx {
        Ctx {
            roots,
            json: flags.json,
            dry_run: flags.dry_run,
            yes: flags.yes,
            verbose: flags.verbose,
            out: Output::new(flags.json, flags.verbose),
            invoker: OnceCell::new(),
        }
    }

    /// Use an already-built invoker (a sandboxed one in tests) instead of
    /// locating the binary.
    pub fn with_invoker(self, inv: Invoker) -> Ctx {
        let _ = self.invoker.set(inv);
        self
    }

    /// The global flags.
    pub fn flags(&self) -> Flags {
        Flags {
            json: self.json,
            dry_run: self.dry_run,
            yes: self.yes,
            verbose: self.verbose,
        }
    }

    /// The host, located on first use (`$OMM_MUSE_BIN` → launcher dir →
    /// `PATH`, never the launcher script; ARCHITECTURE.md §3) and wrapped
    /// with `--verbose` tracing. Its clean environment passes `HOME` /
    /// `XDG_*` through, so it runs against the same roots as `self.roots`.
    pub fn invoker(&self) -> Result<&Invoker> {
        if let Some(inv) = self.invoker.get() {
            return Ok(inv);
        }
        let located = omm_host::locate()?;
        self.out.note(format!(
            "host binary: {} ({:?})",
            located.binary.display(),
            located.source
        ));
        let inv = Invoker::new(located.binary).trace(self.verbose);
        Ok(self.invoker.get_or_init(|| inv))
    }

    /// `$OMM` (ARCHITECTURE.md §2): the ledger, audit log, config.json,
    /// custom/, updates/, snapshots/, locks/.
    pub fn omm_root(&self) -> PathBuf {
        self.roots.omm_root()
    }

    /// The workspace a command acts on when none is given: the canonical
    /// current directory (the key the host uses in `trust.json` and the
    /// `personal_project` formula).
    pub fn workspace(&self) -> Result<PathBuf> {
        let cwd = std::env::current_dir().map_err(|e| OmmError::io("current directory", e))?;
        Ok(fsx::canonicalize(&cwd)?)
    }

    /// R6: refuse to write under `CI=true` or without a terminal unless
    /// `--yes` was given. `--dry-run` never needs consent. Call before the
    /// first write of every write command, with the command's name.
    pub fn require_consent(&self, command: &str) -> Result<()> {
        match consent_needed(
            std::env::var_os("CI").as_deref(),
            std::io::stdin().is_terminal(),
            self.flags(),
        ) {
            None => Ok(()),
            Some(reason) => Err(OmmError::ConsentRequired {
                command: command.to_string(),
                reason,
            }),
        }
    }
}

/// The R6 rule as a pure function: the reason consent is missing, or `None`.
/// `CI` counts as set unless empty, `0` or `false`.
pub(crate) fn consent_needed(
    ci: Option<&OsStr>,
    stdin_is_terminal: bool,
    flags: Flags,
) -> Option<String> {
    if flags.dry_run || flags.yes {
        return None;
    }
    if let Some(v) = ci {
        let v = v.to_string_lossy();
        let off = v.is_empty() || v == "0" || v.eq_ignore_ascii_case("false");
        if !off {
            return Some(format!("CI={v} is set"));
        }
    }
    if !stdin_is_terminal {
        return Some("stdin is not a terminal".to_string());
    }
    None
}

/// The plugin id (`omm`, R19) from `reserved-ids.json` `omm_identity` — data,
/// never a literal in code (R8).
pub fn plugin_id() -> Result<String> {
    Ok(hr::reserved_ids()?.omm_identity.plugin_id.clone())
}

/// Run the parsed command line.
pub fn dispatch(cli: Cli) -> Result<ExitCode> {
    let flags = Flags::from_cli(&cli);
    match cli.command {
        Command::Hook(args) => Ok(hook_fail_open(flags, &args)),
        command => {
            let ctx = Ctx::from_env(flags)?;
            run(&ctx, command)
        }
    }
}

/// The dispatch table proper.
fn run(ctx: &Ctx, command: Command) -> Result<ExitCode> {
    match command {
        Command::Install(a) => lifecycle::install(ctx, &a),
        Command::Uninstall(a) => lifecycle::uninstall(ctx, &a),
        Command::Update(a) => lifecycle::update(ctx, &a),
        Command::Reconcile(a) => lifecycle::reconcile(ctx, &a),
        Command::Trust(a) => lifecycle::trust(ctx, &a),
        Command::List(a) => lifecycle::list(ctx, &a),
        Command::Doctor(a) => inspect::doctor(ctx, &a),
        Command::Cost(a) => inspect::cost(ctx, &a),
        Command::Lint(a) => inspect::lint(ctx, &a),
        Command::Build(a) => inspect::build(ctx, &a),
        Command::Mcp(a) => mcp::mcp(ctx, &a),
        Command::Theme(a) => tune::theme(ctx, &a),
        Command::Keymap(a) => tune::keymap(ctx, &a),
        Command::Profile(a) => tune::profile(ctx, &a),
        Command::Settings(a) => tune::settings(ctx, &a),
        Command::Memory(a) => tune::memory(ctx, &a),
        Command::Enable(a) => tune::enable(ctx, &a),
        Command::Disable(a) => tune::disable(ctx, &a),
        Command::Run(a) => tune::run(ctx, &a),
        Command::Hook(a) => tune::hook(ctx, &a),
    }
}

/// R16 / `content/hooks/README.md`: only exit code 2 blocks, and it is
/// reserved for an intentional deny. A context that cannot be built or a
/// handler that returns `Err` (a stub, a malformed payload, an internal
/// error) becomes `{}` on stdout and exit 0, so a stale or partial `omm` on
/// PATH can never block a tool call or a Stop. Handlers therefore decide in
/// memory and print exactly once, at the end.
fn hook_fail_open(flags: Flags, args: &tune::HookArgs) -> ExitCode {
    let Ok(ctx) = Ctx::fast(flags) else {
        return hook_noop();
    };
    tune::hook(&ctx, args).unwrap_or_else(|_| hook_noop())
}

/// The fail-open decision: consume the event, print `{}`, exit 0.
pub fn hook_noop() -> ExitCode {
    drain_stdin();
    // Never `println!` here: a closed read end would turn EPIPE into a panic
    // and a SIGABRT, and a hook exits 0 whatever the pipe does (R16).
    {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let _ = out.write_all(b"{}\n");
        let _ = out.flush();
    }
    ExitCode::SUCCESS
}

/// Consume the JSON event the host writes on stdin so it never sees EPIPE.
/// Skipped when stdin is a terminal (a human ran it by hand); bounded so a
/// runaway writer cannot hold the hook past its `timeoutMs`.
pub fn drain_stdin() {
    use std::io::Read;
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return;
    }
    let _ = std::io::copy(
        &mut stdin.lock().take(tune::c_tune::hook::STDIN_LIMIT_BYTES),
        &mut std::io::sink(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn flags(dry_run: bool, yes: bool) -> Flags {
        Flags {
            dry_run,
            yes,
            ..Flags::default()
        }
    }

    #[test]
    fn consent_is_needed_under_ci_or_without_a_terminal_unless_yes_or_dry_run() {
        let ci = OsString::from("true");
        assert_eq!(
            consent_needed(Some(&ci), true, flags(false, false)).as_deref(),
            Some("CI=true is set")
        );
        assert_eq!(
            consent_needed(None, false, flags(false, false)).as_deref(),
            Some("stdin is not a terminal")
        );
        assert_eq!(consent_needed(None, true, flags(false, false)), None);
        assert_eq!(consent_needed(Some(&ci), false, flags(false, true)), None);
        assert_eq!(consent_needed(Some(&ci), false, flags(true, false)), None);
        for off in ["", "0", "false", "FALSE"] {
            let v = OsString::from(off);
            assert_eq!(
                consent_needed(Some(&v), true, flags(false, false)),
                None,
                "{off:?}"
            );
        }
        let one = OsString::from("1");
        assert_eq!(
            consent_needed(Some(&one), true, flags(false, false)).as_deref(),
            Some("CI=1 is set")
        );
    }

    #[test]
    fn plugin_id_comes_from_the_data_file() {
        let id = plugin_id().unwrap();
        // The host's plugin-id grammar (reserved-ids.json `id_grammar`):
        // lowercase ASCII open, then letters, digits, `.`, `_`, `-`.
        assert!(!id.is_empty());
        let mut chars = id.chars();
        assert!(chars
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert!(chars.all(|c| c.is_ascii_lowercase()
            || c.is_ascii_digit()
            || c == '.'
            || c == '_'
            || c == '-'));
    }

    #[test]
    fn context_over_explicit_roots_keeps_a_given_invoker() {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let roots = sb.roots().unwrap();
        let ctx = Ctx::new(roots.clone(), flags(true, true))
            .with_invoker(Invoker::new(tmp.path().join("no-such-muse")).sandboxed(&sb));
        assert_eq!(ctx.roots, roots);
        assert!(ctx.dry_run && ctx.yes && !ctx.json && !ctx.verbose);
        assert_eq!(ctx.omm_root(), roots.omm_root());
        assert_eq!(
            ctx.invoker().unwrap().bin(),
            tmp.path().join("no-such-muse")
        );
        assert!(ctx.require_consent("install").is_ok());
    }
}
