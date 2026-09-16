//! The clap surface — every subcommand of ARCHITECTURE.md §7.
//!
//! This file owns only the globals and the command TABLE. Each command's own
//! flags live beside its body in `cmd/<group>.rs` as a `clap::Args` struct,
//! so the three groups extend their surface without touching this file.
//! Beyond the §7 table: `settings` (the D2–D6 fixes doctor prints, §6) and
//! `reconcile` (D13's fix), so every fix command doctor can print parses.

use clap::{Parser, Subcommand};

use crate::cmd::{inspect, lifecycle, mcp, tune};

/// oh-my-musecode: a curated content bundle, thin CLI and git marketplace for
/// Meta Muse Code. It never touches the Muse binary; everything goes through
/// Muse's own config surface and CLI verbs.
#[derive(Debug, Parser)]
#[command(name = "omm", version, about, long_about = None, propagate_version = true)]
pub struct Cli {
    /// Machine-readable JSON output (every read command).
    #[arg(long, global = true)]
    pub json: bool,
    /// Plan and report, write nothing (every write command).
    #[arg(long, global = true)]
    pub dry_run: bool,
    /// Proceed without a human in the loop: required under CI=true or a
    /// non-TTY for every write command (R6).
    #[arg(short, long, global = true)]
    pub yes: bool,
    /// Print every host process omm spawns.
    #[arg(short, long, global = true)]
    pub verbose: bool,
    #[command(subcommand)]
    pub command: Command,
}

/// The §7 command table.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install the bundle: marketplace add → plugins install → approve every capability → verify
    Install(lifecycle::InstallArgs),
    /// Remove what the ledger records: two-section preview, deepest-first (R4)
    Uninstall(lifecycle::UninstallArgs),
    /// Snapshot → reconcile (R3) → reinstall + re-approve → report staged conflicts
    Update(lifecycle::UpdateArgs),
    /// Re-check every ledger entry against the disk and the host (doctor D13's fix)
    Reconcile(lifecycle::ReconcileArgs),
    /// Merge a workspace into trust.json (default: cwd)
    Trust(lifecycle::TrustArgs),
    /// Resolved assets with provenance and `_shadowed`
    List(lifecycle::ListArgs),
    /// Run the D1–D15 checks with exact fix commands
    Doctor(inspect::DoctorArgs),
    /// Per-source byte table of the skills catalog and the other context budgets
    Cost(inspect::CostArgs),
    /// Lint content: ids, collisions, symlinks, budgets, host checkpoints
    Lint(inspect::LintArgs),
    /// Regenerate the native package, projections and marketplace indexes (dev)
    Build(inspect::BuildArgs),
    /// The stdio MCP server the bundle declares (omm_doctor, omm_cost); the host spawns it
    Mcp(mcp::McpArgs),
    /// Set tui.theme — validated first, prior value recorded
    Theme(tune::ThemeArgs),
    /// Apply a keymap preset
    Keymap(tune::KeymapArgs),
    /// Settings profiles: strict / default / fast / ci
    Profile(tune::ProfileArgs),
    /// Targeted settings.json patches — the fixes doctor D2–D6 print
    Settings(tune::SettingsArgs),
    /// The personal_project memory root
    Memory(tune::MemoryArgs),
    /// Enable an opt-in feature (e.g. skill-routing)
    Enable(tune::FeatureArgs),
    /// Disable an opt-in feature
    Disable(tune::FeatureArgs),
    /// The launcher shim: allowlist check (R20), controlled env, then exec muse
    Run(tune::RunArgs),
    /// In-binary hook dispatcher: JSON event on stdin, JSON out, fail-open (R16)
    Hook(tune::HookArgs),
}

impl Command {
    /// The user-facing name, e.g. `profile use`.
    pub fn name(&self) -> String {
        match self {
            Command::Install(_) => "install".into(),
            Command::Uninstall(_) => "uninstall".into(),
            Command::Update(_) => "update".into(),
            Command::Reconcile(_) => "reconcile".into(),
            Command::Trust(_) => "trust".into(),
            Command::List(_) => "list".into(),
            Command::Doctor(_) => "doctor".into(),
            Command::Cost(_) => "cost".into(),
            Command::Lint(_) => "lint".into(),
            Command::Build(_) => "build".into(),
            Command::Mcp(_) => "mcp".into(),
            Command::Theme(_) => "theme".into(),
            Command::Keymap(_) => "keymap".into(),
            Command::Profile(a) => a.name(),
            Command::Settings(a) => a.name(),
            Command::Memory(a) => a.name(),
            Command::Enable(_) => "enable".into(),
            Command::Disable(_) => "disable".into(),
            Command::Run(_) => "run".into(),
            Command::Hook(_) => "hook".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    /// The §7 table, plus `settings` and `reconcile` (the fixes doctor prints).
    const EXPECTED: [&str; 20] = [
        "install",
        "uninstall",
        "update",
        "reconcile",
        "trust",
        "list",
        "doctor",
        "cost",
        "lint",
        "build",
        "mcp",
        "theme",
        "keymap",
        "profile",
        "settings",
        "memory",
        "enable",
        "disable",
        "run",
        "hook",
    ];

    #[test]
    fn every_section_7_command_is_present() {
        let cmd = Cli::command();
        let names: Vec<String> = cmd
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        for expected in EXPECTED {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
        assert_eq!(names.len(), EXPECTED.len());
        Cli::command().debug_assert();
    }

    #[test]
    fn every_fix_command_doctor_prints_parses() {
        // The fix strings of ARCHITECTURE.md §6 that start with `omm`.
        for argv in [
            "omm install",
            "omm install --reinstall",
            "omm settings set provider meta",
            "omm settings fix-mcp-collision",
            "omm settings lint --fix",
            "omm settings reconcile-reminders",
            "omm settings set agent_definitions.safe_mode false",
            "omm cost",
            "omm doctor --report-drift",
            "omm trust .",
            "omm reconcile",
        ] {
            let cli =
                Cli::try_parse_from(argv.split(' ')).unwrap_or_else(|e| panic!("{argv}: {e}"));
            assert!(!cli.command.name().is_empty());
        }
    }

    #[test]
    fn globals_and_trailing_run_args_parse() {
        let cli =
            Cli::try_parse_from(["omm", "--json", "--dry-run", "run", "--", "--version"]).unwrap();
        assert!(cli.json && cli.dry_run && !cli.yes);
        match cli.command {
            Command::Run(tune::RunArgs { args }) => assert_eq!(args, vec!["--version"]),
            other => panic!("{other:?}"),
        }
        let cli = Cli::try_parse_from(["omm", "profile", "use", "strict"]).unwrap();
        assert_eq!(cli.command.name(), "profile use");
        // `--yes` is global: before or after the subcommand.
        let cli = Cli::try_parse_from(["omm", "install", "--yes", "--no-plugin"]).unwrap();
        assert!(cli.yes);
        match cli.command {
            Command::Install(a) => assert!(a.no_plugin && !a.reinstall),
            other => panic!("{other:?}"),
        }
        let cli = Cli::try_parse_from(["omm", "-y", "uninstall", "--force"]).unwrap();
        assert!(cli.yes);
        assert_eq!(cli.command.name(), "uninstall");
        let cli = Cli::try_parse_from(["omm", "memory", "gc"]).unwrap();
        assert_eq!(cli.command.name(), "memory gc");
        let cli = Cli::try_parse_from(["omm", "settings", "lint"]).unwrap();
        assert_eq!(cli.command.name(), "settings lint");
    }
}
