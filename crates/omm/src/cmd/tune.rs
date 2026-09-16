//! Group C — the tuning knobs and the two runtime entry points: `theme`,
//! `keymap`, `profile`, `settings`, `memory`, `enable` / `disable`, `run`,
//! `hook` (ARCHITECTURE.md §7; R9, R10, R16, R18, R20, R22).
//!
//! This file is the only one group C edits, with its helpers under
//! `cmd/c_tune/`. The `Args` structs and action enums here ARE the clap
//! surface of these commands (`cli.rs` only names them), so a new flag or
//! sub-action lands here, beside its body.
//!
//! What lands where: every settings write goes through
//! `c_tune::settings_tx` — `SettingsDoc::patch_typed` → the host's two
//! validators → atomic commit under the muse-config lock with a verified
//! backup → the ledger (`settings.json` entry + one `settings-key`
//! registration per key with its FIRST prior) → one audit line (R9, R10).
//! `theme` also copies the `.tmTheme` as a ledgered `copy` entry reconciled
//! with R3's four outcomes; `profile use` flattens the slice to leaves and
//! refuses the R18 trap; `run` is the R20 allowlist then `exec`; `hook`
//! decides in memory from `HOME` alone and prints once (R16). `memory`
//! (`c_tune/memory.rs`) seeds, lists, backs up and collects the host's
//! `personal_project` roots, its ledger entries under `muse-data` and its
//! tars under `$OMM/snapshots/memory/` (PLAN.md 2.3); `enable` / `disable`
//! are Phase 3.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};

#[path = "c_tune/mod.rs"]
pub(crate) mod c_tune;
#[path = "c_tune/memory.rs"]
pub(crate) mod memory;
#[path = "c_tune/memory_tar.rs"]
pub(crate) mod memory_tar;

use crate::cmd::Ctx;
use crate::error::{OmmError, Result};

/// `omm theme <name>` (or `omm theme list`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct ThemeArgs {
    /// Theme name or `custom:<file-stem>`; `list` shows bundled, custom and installed themes
    pub name: String,
}

/// `omm keymap <preset>` (or `omm keymap list`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct KeymapArgs {
    /// Preset name: `default` (the host's bindings), a name under $OMM/custom/keymaps/, or a .json path; `list` shows them
    pub preset: String,
}

/// `omm profile <action>`.
#[derive(Args, Clone, Debug, PartialEq, Eq)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub action: ProfileAction,
}

/// `omm profile …`
#[derive(Subcommand, Clone, Debug, PartialEq, Eq)]
pub enum ProfileAction {
    /// Switch to a profile as one ledgered transaction
    Use {
        /// Profile name (content/profiles/<name>.json or custom/profiles/<name>.json)
        name: String,
        /// Apply the slice over a key the user edited since a profile set it (otherwise such a key is left as is and named)
        #[arg(long)]
        force: bool,
    },
    /// List the bundled and custom profiles and the active one
    List,
    /// Show a profile's settings slice against the current settings.json
    Show {
        /// Profile name
        name: String,
    },
}

impl ProfileArgs {
    /// The user-facing name, e.g. `profile use`.
    pub fn name(&self) -> String {
        match self.action {
            ProfileAction::Use { .. } => "profile use".into(),
            ProfileAction::List => "profile list".into(),
            ProfileAction::Show { .. } => "profile show".into(),
        }
    }

    /// Whether the action writes (needs R6 consent).
    pub fn writes(&self) -> bool {
        matches!(self.action, ProfileAction::Use { .. })
    }
}

/// `omm settings <action>` — the targeted settings.json patches doctor
/// D2–D6 print as fixes (ARCHITECTURE.md §6).
#[derive(Args, Clone, Debug, PartialEq, Eq)]
pub struct SettingsArgs {
    #[command(subcommand)]
    pub action: SettingsAction,
}

/// `omm settings …`
#[derive(Subcommand, Clone, Debug, PartialEq, Eq)]
pub enum SettingsAction {
    /// Set one typed settings.json key (dotted), validated first, prior value recorded (D2, D6)
    Set {
        /// Dotted key, e.g. `provider` or `agent_definitions.safe_mode`
        key: String,
        /// JSON value; a bare word is taken as a string
        value: String,
    },
    /// Resolve a `mcpServers` + `mcp_servers` collision (D3)
    FixMcpCollision,
    /// Lint the `plugins` and `runtime_capabilities` shapes (D4)
    Lint {
        /// Rewrite the malformed entries
        #[arg(long)]
        fix: bool,
    },
    /// Make canonical and legacy reminder enablement agree (D5)
    ReconcileReminders,
}

impl SettingsArgs {
    /// The user-facing name, e.g. `settings set`.
    pub fn name(&self) -> String {
        match self.action {
            SettingsAction::Set { .. } => "settings set".into(),
            SettingsAction::FixMcpCollision => "settings fix-mcp-collision".into(),
            SettingsAction::Lint { .. } => "settings lint".into(),
            SettingsAction::ReconcileReminders => "settings reconcile-reminders".into(),
        }
    }

    /// Whether the action writes (needs R6 consent).
    pub fn writes(&self) -> bool {
        !matches!(self.action, SettingsAction::Lint { fix: false })
    }
}

/// `omm memory <action>`.
#[derive(Args, Clone, Debug, PartialEq, Eq)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub action: MemoryAction,
}

/// `omm memory …`
#[derive(Subcommand, Clone, Debug, PartialEq, Eq)]
pub enum MemoryAction {
    /// Print the resolved personal_project root of the current workspace
    Path,
    /// Seed MEMORY.md (and the omm-workspace.json sidecar) for the current workspace; warns when the workspace is not trusted
    Seed,
    /// List every personal_project root with the workspace it maps to and its bytes / files against the silent caps
    List,
    /// Tar the current workspace's memory root under $OMM/snapshots/memory/<ts>/ (newest five kept)
    Backup {
        /// Write the tar into this directory instead (not rolled)
        #[arg(long, value_name = "DIR")]
        to: Option<PathBuf>,
    },
    /// Remove the roots whose sidecar names a workspace that no longer exists (a tar of each is taken first); --dry-run previews
    Gc,
}

impl MemoryArgs {
    /// The user-facing name, e.g. `memory seed`.
    pub fn name(&self) -> String {
        match self.action {
            MemoryAction::Path => "memory path".into(),
            MemoryAction::Seed => "memory seed".into(),
            MemoryAction::List => "memory list".into(),
            MemoryAction::Backup { .. } => "memory backup".into(),
            MemoryAction::Gc => "memory gc".into(),
        }
    }

    /// Whether the action writes (needs R6 consent).
    pub fn writes(&self) -> bool {
        !matches!(self.action, MemoryAction::List | MemoryAction::Path)
    }
}

/// `omm enable <feature>` / `omm disable <feature>`.
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureArgs {
    /// Feature name (e.g. skill-routing)
    pub feature: String,
}

/// `omm run -- <muse args>`.
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct RunArgs {
    /// Arguments for muse, after `--`
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

/// `omm hook <name>`.
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct HookArgs {
    /// Hook name (the `id` of a content/hooks/<id>.json)
    pub name: String,
}

/// The reserved word that lists instead of applying.
const LIST: &str = "list";

/// `omm theme <name>`: copy the theme file (overlay before bundled, R7)
/// into `<config>/themes/` as a ledgered entry and set `tui.theme =
/// "custom:<stem>"`, validated first, prior recorded (scenario 7).
/// `omm theme list` reads only.
pub fn theme(ctx: &Ctx, args: &ThemeArgs) -> Result<ExitCode> {
    if args.name == LIST {
        let table = c_tune::theme::list(ctx)?;
        ctx.out.report(&table);
        return Ok(ExitCode::SUCCESS);
    }
    ctx.require_consent("theme")?;
    let report = c_tune::theme::apply(ctx, &args.name)?;
    ctx.out.report(&report);
    Ok(ExitCode::SUCCESS)
}

/// `omm keymap <preset>`: `tui.keymap` as a targeted patch — `default`
/// removes it, a preset file sets it — validated offline by the host's
/// defaults-plane validator inside the enterprise wrapper before landing.
pub fn keymap(ctx: &Ctx, args: &KeymapArgs) -> Result<ExitCode> {
    if args.preset == LIST {
        let table = c_tune::profile::keymap_list(ctx)?;
        ctx.out.report(&table);
        return Ok(ExitCode::SUCCESS);
    }
    ctx.require_consent("keymap")?;
    let report = c_tune::profile::keymap(ctx, &args.preset)?;
    ctx.out.report(&report);
    Ok(ExitCode::SUCCESS)
}

/// `omm profile use <name>`: the settings slice as ONE ledgered transaction
/// (every leaf's prior recorded, validated before landing, the R18 trap
/// refused); `list` / `show` read only. R22 (a profile can only tighten)
/// is the Phase 2 lattice.
pub fn profile(ctx: &Ctx, args: &ProfileArgs) -> Result<ExitCode> {
    if args.writes() {
        ctx.require_consent(&args.name())?;
    }
    match &args.action {
        ProfileAction::Use { name, force } => {
            let report = c_tune::profile::use_profile(ctx, name, *force)?;
            ctx.out.report(&report);
        }
        ProfileAction::List => {
            let table = c_tune::profile::list(ctx)?;
            ctx.out.report(&table);
        }
        ProfileAction::Show { name } => {
            let (text, json) = c_tune::profile::show(ctx, name)?;
            ctx.out.emit(|| text, || json);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `omm settings …`: the D2–D6 fixes as targeted, validated patches.
/// `lint` without `--fix` reads only and exits 1 when it finds problems.
pub fn settings(ctx: &Ctx, args: &SettingsArgs) -> Result<ExitCode> {
    if args.writes() {
        ctx.require_consent(&args.name())?;
    }
    let report = match &args.action {
        SettingsAction::Set { key, value } => c_tune::settings::set(ctx, key, value)?,
        SettingsAction::FixMcpCollision => c_tune::settings::fix_mcp_collision(ctx)?,
        SettingsAction::Lint { fix } => {
            let (report, problems_remain) = c_tune::settings::lint(ctx, *fix)?;
            ctx.out.report(&report);
            return Ok(if problems_remain {
                ExitCode::from(1)
            } else {
                ExitCode::SUCCESS
            });
        }
        SettingsAction::ReconcileReminders => c_tune::settings::reconcile_reminders(ctx)?,
    };
    ctx.out.report(&report);
    Ok(ExitCode::SUCCESS)
}

/// `omm memory …`: the `personal_project` root of the current workspace
/// (trust-gated at runtime, so `omm trust` is a prerequisite; host-reality.md
/// "Paths": memory personal_project). `path` and `list` read; `seed`,
/// `backup` and `gc` write — every write ledgered or audited, `--dry-run`
/// honoured (`c_tune/memory.rs`).
pub fn memory(ctx: &Ctx, args: &MemoryArgs) -> Result<ExitCode> {
    if args.writes() {
        ctx.require_consent(&args.name())?;
    }
    match &args.action {
        MemoryAction::Path => {
            let (text, json) = memory::path_report(ctx, &ctx.workspace()?)?;
            ctx.out.emit(|| text, || json);
        }
        MemoryAction::Seed => {
            let report = memory::seed(ctx, &ctx.workspace()?)?;
            ctx.out.report(&report);
        }
        MemoryAction::List => {
            let table = memory::list(ctx, ctx.workspace().ok().as_deref())?;
            ctx.out.report(&table);
        }
        MemoryAction::Backup { to } => {
            let report = memory::backup(ctx, &ctx.workspace()?, to.as_deref())?;
            ctx.out.report(&report);
        }
        MemoryAction::Gc => {
            let report = memory::gc(ctx)?;
            ctx.out.report(&report);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// The opt-in features `omm enable` / `omm disable` know.
const FEATURES: [&str; 1] = [c_tune::routing::FEATURE];

fn unknown_feature(name: &str) -> OmmError {
    OmmError::Usage(format!(
        "unknown feature {name:?}; features: {}",
        FEATURES.join(" ")
    ))
}

/// `omm enable skill-routing` (PLAN.md 3.1, `docs/ROUTING.md`): in the
/// trusted git workspace that is the cwd, copy the routed library to
/// `.omm/skills/`, write the router handler into `.muse/hooks.json`, record
/// the measured order-200 size and turn `skill_routing` on in
/// `$OMM/config.json` so `omm run` exports both gates — every file
/// ledgered under the `workspace` base (`c_tune::routing_enable`).
pub fn enable(ctx: &Ctx, args: &FeatureArgs) -> Result<ExitCode> {
    match args.feature.as_str() {
        c_tune::routing::FEATURE => {
            ctx.require_consent("enable skill-routing")?;
            let report = c_tune::routing_enable::enable(ctx)?;
            ctx.out.report(&report);
            Ok(ExitCode::SUCCESS)
        }
        other => Err(unknown_feature(other)),
    }
}

/// `omm disable skill-routing`: reverse [`enable`] byte for byte — the
/// library files removed while they hold omm's bytes, the hooks file
/// removed or restored to its pre-omm bytes, the config keys removed.
pub fn disable(ctx: &Ctx, args: &FeatureArgs) -> Result<ExitCode> {
    match args.feature.as_str() {
        c_tune::routing::FEATURE => {
            ctx.require_consent("disable skill-routing")?;
            let report = c_tune::routing_enable::disable(ctx)?;
            ctx.out.report(&report);
            Ok(ExitCode::SUCCESS)
        }
        other => Err(unknown_feature(other)),
    }
}

/// `omm run -- <args>`: the launcher shim — R20 allowlist check first (an
/// unknown token is a prompt and would start a billed session; exit 2 with
/// the reason), then `exec` the binary (never the launcher) with the user's
/// environment, `MUSE_NO_AUTO_UPDATE=1` and the active profile's gates.
/// `--dry-run` prints the plan instead. Writes nothing.
pub fn run(ctx: &Ctx, args: &RunArgs) -> Result<ExitCode> {
    let plan = c_tune::run::plan(ctx, &args.args)?;
    if ctx.dry_run {
        ctx.out.emit(|| plan.render(), || plan.to_json());
        return Ok(ExitCode::SUCCESS);
    }
    c_tune::run::exec(ctx, &plan)
}

/// `omm hook <name>`: the in-binary dispatcher (R16). Reads the JSON event
/// on stdin (bounded), decides in memory, prints exactly one JSON line,
/// exits 0 always — a deny travels in the JSON. `ctx.out` is silent here;
/// `ctx.roots` came from `Roots::from_env_fast` (no spawn) and
/// `ctx.invoker()` is never called. Unknown name → `{}`.
pub fn hook(ctx: &Ctx, args: &HookArgs) -> Result<ExitCode> {
    let event = c_tune::hook::read_event();
    let decision = c_tune::hook::dispatch(&args.name, &event, &ctx.roots);
    // The host may close its read end before the decision lands (a cancelled
    // turn). `println!` would panic on EPIPE and the process would die by
    // SIGABRT; a hook must exit 0 whatever the pipe does (R16 fail-open), so
    // the write error is deliberately ignored.
    {
        use std::io::Write;
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let _ = writeln!(out, "{decision}");
        let _ = out.flush();
    }
    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_action_names_and_write_flags() {
        let s = SettingsArgs {
            action: SettingsAction::Lint { fix: false },
        };
        assert_eq!(s.name(), "settings lint");
        assert!(!s.writes());
        let s = SettingsArgs {
            action: SettingsAction::Lint { fix: true },
        };
        assert!(s.writes());
        let s = SettingsArgs {
            action: SettingsAction::Set {
                key: "provider".into(),
                value: "meta".into(),
            },
        };
        assert_eq!(s.name(), "settings set");
        assert!(s.writes());
        assert_eq!(
            SettingsArgs {
                action: SettingsAction::FixMcpCollision
            }
            .name(),
            "settings fix-mcp-collision"
        );
        assert_eq!(
            SettingsArgs {
                action: SettingsAction::ReconcileReminders
            }
            .name(),
            "settings reconcile-reminders"
        );
        let m = MemoryArgs {
            action: MemoryAction::List,
        };
        assert_eq!(m.name(), "memory list");
        assert!(!m.writes());
        let m = MemoryArgs {
            action: MemoryAction::Path,
        };
        assert_eq!(m.name(), "memory path");
        assert!(!m.writes());
        for a in [
            MemoryAction::Seed,
            MemoryAction::Backup { to: None },
            MemoryAction::Gc,
        ] {
            assert!(MemoryArgs { action: a }.writes());
        }
        assert_eq!(
            MemoryArgs {
                action: MemoryAction::Backup {
                    to: Some(PathBuf::from("/x"))
                }
            }
            .name(),
            "memory backup"
        );
    }

    #[test]
    fn memory_flags_parse() {
        use crate::cli::{Cli, Command};
        use clap::Parser;
        let cli = Cli::try_parse_from(["omm", "memory", "backup", "--to", "/tmp/x"]).unwrap();
        match cli.command {
            Command::Memory(MemoryArgs {
                action: MemoryAction::Backup { to },
            }) => assert_eq!(to.as_deref(), Some(std::path::Path::new("/tmp/x"))),
            other => panic!("{other:?}"),
        }
        // `gc --dry-run` is the global flag.
        let cli = Cli::try_parse_from(["omm", "memory", "gc", "--dry-run"]).unwrap();
        assert!(cli.dry_run);
        assert!(matches!(
            cli.command,
            Command::Memory(MemoryArgs {
                action: MemoryAction::Gc
            })
        ));
        let p = ProfileArgs {
            action: ProfileAction::Use {
                name: "x".into(),
                force: false,
            },
        };
        assert_eq!(p.name(), "profile use");
        assert!(p.writes());
        assert!(!ProfileArgs {
            action: ProfileAction::List
        }
        .writes());
        assert_eq!(
            ProfileArgs {
                action: ProfileAction::Show { name: "x".into() }
            }
            .name(),
            "profile show"
        );
    }

    #[test]
    fn the_reserved_list_word_cannot_be_a_theme_id() {
        // Bundled ids are `omm-` prefixed (R19), so `list` never collides.
        assert_eq!(LIST, "list");
        assert!(
            c_tune::check_name("theme", LIST).is_ok(),
            "grammar-valid, reserved by convention"
        );
    }
}
