//! Group B — the read-only inspectors: `doctor`, `cost`, `lint`, `build`
//! (ARCHITECTURE.md §7, §5.2, §5.3, §6). The Phase-2 `mcp` server (PLAN.md
//! 2.2) lives in `cmd/mcp.rs` and reuses [`doctor_context`] for both tools.
//!
//! This file is the only one group B edits. The `Args` structs here ARE the
//! clap surface of these commands (`cli.rs` only names them), so a new flag
//! lands here, beside its body.
//!
//! What each command touches:
//! - `doctor` and `cost` write nothing (§7). Their live measurements run one
//!   `muse exec --provider echo hi` against a throwaway data root seeded
//!   from the user's plugin store (`omm_doctor::session`), the cost cuts
//!   against a throwaway copy of the config root, and the host self-test in
//!   a temp sandbox (`omm_host::hostcheck::run_in`). The ledger is read
//!   through `omm_doctor::ledger` — never `omm_ledger::store::load`, whose
//!   quarantine of a corrupt file is a write.
//! - `lint` writes nothing in the repo; the R13 checkpoints render the
//!   package into a temp dir and ask the binary (`skills validate`,
//!   `plugins validate`, the digest through a throwaway marketplace).
//! - `build` writes repo files only — `plugins/<pid>/`, `dist/claude/`,
//!   `dist/codex/` and the three catalogs, every one atomically onto a
//!   canonical path inside the repo (`omm_manifest::generate::write`) — never
//!   Muse config, so it needs no R6 consent and runs in CI unattended.
//!   `--check` is the CI drift gate (§5.2: a version bump alone is never
//!   drift), `--dry-run` plans the same writes and lands none.
//!
//! The host is always reached through [`Ctx::invoker`]; for `lint` / `build`
//! the invoker's working directory is a temp dir, so no host verb ever runs
//! with the repository as its cwd (the host scaffolds files into a cwd it
//! treats as a workspace).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Args;
use serde_json::{json, Value};

use omm_doctor::cost::{self, CostOptions};
use omm_doctor::Options as DoctorOptions;
use omm_host::{fsx, hostcheck, Invoker};
use omm_manifest::budget::BudgetReport;
use omm_manifest::catalog::Catalog;
use omm_manifest::content::Content;
use omm_manifest::drift::{self, DriftReport};
use omm_manifest::generate::{
    self, BuildOptions, BuildOutput, DigestSource, Package, WriteReport, WriteSummary,
};
use omm_manifest::lint::{self, LintOptions, LintReport, Severity};
use omm_manifest::{Repo, CATALOG_FILE, CONTENT_DIR};

use crate::cmd::Ctx;
use crate::error::{OmmError, Result};
use crate::output::{Action, Converge, Render, Table};

/// `omm doctor [--self-test] [--report-drift] [--fast]` (+ global `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct DoctorArgs {
    /// Re-measure docs/host-reality.md against the live binary (P0 + P1)
    #[arg(long)]
    pub self_test: bool,
    /// Print the golden-constant diff table (D11): the rows that drifted; exit 1 if any
    #[arg(long)]
    pub report_drift: bool,
    /// Skip the two slow probes: the live echo session (D8) and the host self-test (D11)
    #[arg(long)]
    pub fast: bool,
}

/// `omm cost [--no-cuts]` (+ global `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct CostArgs {
    /// Skip the four extra echo sessions that measure the "what to cut" savings
    #[arg(long)]
    pub no_cuts: bool,
}

/// `omm lint [path] [--no-host]` (+ global `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct LintArgs {
    /// Repository root or its content/ directory (default: the checkout containing the cwd)
    pub path: Option<PathBuf>,
    /// Pure rules only: skip the R13 host checkpoints and the digest check
    #[arg(long)]
    pub no_host: bool,
}

/// `omm build [--check] [path]` (dev; + global `--dry-run`, `--json`).
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildArgs {
    /// Drift mode for CI: regenerate in memory, diff against the committed files, exit 1 on drift
    #[arg(long)]
    pub check: bool,
    /// Repository root (default: the checkout containing the cwd)
    pub path: Option<PathBuf>,
}

// ---- doctor -------------------------------------------------------------

/// `omm doctor`: D1–D15 with exact fix commands (ARCHITECTURE.md §6); exit
/// 1 on any critical row. `--self-test` runs the P0 + P1 fixture set instead
/// (`hostcheck`, every row, `OmmError::SelfTest` on a failure);
/// `--report-drift` runs the same set and prints only the drifted rows — the
/// table D11's fix command points at — exiting 1 when there are any.
pub fn doctor(ctx: &Ctx, args: &DoctorArgs) -> Result<ExitCode> {
    if args.self_test {
        return self_test(ctx);
    }
    if args.report_drift {
        return report_drift(ctx);
    }
    let dctx = doctor_context(ctx, args.fast)?;
    let mut report = omm_doctor::run(&dctx);
    // D16 (skill routing, PLAN.md 3.1) lives beside the router in this crate.
    crate::cmd::tune::c_tune::routing_doctor::append(ctx, &dctx, &mut report);
    ctx.out.emit(
        || report.render().trim_end().to_string(),
        || report.to_json(),
    );
    Ok(code(report.exit_code()))
}

/// The doctor `Context` over this CLI's host and roots: the workspace is the
/// process cwd (as `Context::from_env` spells it, so D12's fix prints `.`),
/// D1's expected capabilities are the ledger registration's `approved` list
/// when an installer recorded one, and `--fast` turns the two probes that
/// cost seconds off (D8's echo session, D11's hostcheck). `omm mcp` builds
/// the same context for its two tools.
pub(crate) fn doctor_context(ctx: &Ctx, fast: bool) -> Result<omm_doctor::Context> {
    let inv = ctx.invoker()?.clone();
    let mut dctx = omm_doctor::Context::new(inv, ctx.roots.clone())?
        .with_workspace(std::env::current_dir().ok());
    if let Some(approved) = ledger_approved(&dctx) {
        dctx = dctx.with_expected_capabilities(approved);
    }
    if fast {
        dctx = dctx.with_options(DoctorOptions {
            live: false,
            host_drift: false,
            ..DoctorOptions::default()
        });
    }
    Ok(dctx)
}

/// The stable ids the installer approved, from the ledger's `muse-plugin`
/// registration (ARCHITECTURE.md §4 `registrations[].approved`), read through
/// the doctor's read-only lane: `omm_ledger::store::load` quarantines a corrupt
/// file and appends an audit line, and `omm doctor` writes nothing (§7). A
/// missing or corrupt ledger yields `None` and D1 falls back to what the
/// installed package declares.
fn ledger_approved(dctx: &omm_doctor::Context) -> Option<Vec<String>> {
    match omm_doctor::ledger::load(&dctx.ledger_path()) {
        omm_doctor::ledger::LedgerState::Loaded(l) => l
            .plugin_registration(&dctx.plugin_id)
            .and_then(|r| r.approved.clone()),
        _ => None,
    }
}

/// `omm doctor --self-test`: locate the binary, re-measure host-reality.md
/// in a temp root; exit 1 (`OmmError::SelfTest`) on any failed row.
fn self_test(ctx: &Ctx) -> Result<ExitCode> {
    let inv = ctx.invoker()?;
    let tmp = tempdir("omm-self-test-")?;
    let report = hostcheck::run_in(inv, tmp.path())?;
    ctx.out.emit(
        || report.render().trim_end().to_string(),
        || report.to_json(),
    );
    if report.ok() {
        Ok(ExitCode::SUCCESS)
    } else {
        Err(OmmError::SelfTest {
            failed: report.failures().len(),
            total: report.checks.len(),
        })
    }
}

/// `omm doctor --report-drift`: the golden-constant diff table (D11's fix).
/// Only the drifted rows are printed (every row under `--verbose`); exit 1
/// when any row drifted, as a result rather than an error — the report is
/// the point of the command.
fn report_drift(ctx: &Ctx) -> Result<ExitCode> {
    let inv = ctx.invoker()?;
    let tmp = tempdir("omm-report-drift-")?;
    let report = hostcheck::run_in(inv, tmp.path())?;
    let view = DriftRows {
        report: &report,
        all: ctx.verbose,
    };
    ctx.out.report(&view);
    Ok(if report.ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// The hostcheck report as a diff table: drifted rows only unless `all`.
struct DriftRows<'a> {
    report: &'a hostcheck::Report,
    all: bool,
}

impl Render for DriftRows<'_> {
    fn render(&self) -> String {
        let failures = self.report.failures();
        let mut out = format!(
            "host drift: {} ({}) — {} of {} rows drifted ({} ms)",
            self.report.binary.display(),
            self.report.version.as_deref().unwrap_or("version unknown"),
            failures.len(),
            self.report.checks.len(),
            self.report.elapsed_ms
        );
        // OLDER-BUILD rows are not drift but are worth a line: the data
        // carries facts this build predates (hostcheck module docs).
        let older = self.report.older_build();
        let rows: Vec<&hostcheck::Check> = if self.all {
            self.report.checks.iter().collect()
        } else {
            failures.clone()
        };
        if rows.is_empty() {
            if !older.is_empty() {
                out.push_str(&format!(
                    "\n{} OLDER-BUILD row(s) — facts the data tags `since` a later build, absent here as a group: {}",
                    older.len(),
                    older
                        .iter()
                        .map(|c| c.id.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            out.push_str("\nno drift: every P0/P1 row matches docs/host-reality.md");
            return out;
        }
        let mut table = Table::new(["status", "tier", "check", "expected", "observed"]);
        for c in &rows {
            table.row([
                c.status.to_string(),
                c.tier.to_string(),
                c.id.clone(),
                c.expected.clone(),
                c.observed.clone(),
            ]);
        }
        out.push('\n');
        out.push_str(&table.render());
        for c in failures.iter().filter(|c| !c.detail.is_empty()) {
            out.push_str(&format!("\n{}: {}", c.id, c.detail));
        }
        if !failures.is_empty() {
            out.push_str(
                "\nfix: re-measure and update docs/host-reality.md and crates/omm-host/src/host_reality.rs (R15: the contract is probed, never the version string)",
            );
        }
        out
    }

    fn to_json(&self) -> Value {
        let mut doc = self.report.to_json();
        let drifted: Vec<Value> = self
            .report
            .failures()
            .iter()
            .map(|c| Value::String(c.id.clone()))
            .collect();
        doc["drifted"] = Value::Array(drifted);
        doc["exit_code"] = json!(if self.report.ok() { 0 } else { 1 });
        doc
    }
}

// ---- cost ---------------------------------------------------------------

/// `omm cost`: the per-source byte table of the skills catalog, the
/// refundable built-in tax, memory / rules / cookbook budgets and the
/// bytes/4 token estimate (§6 "Cost"), from one echo session in a throwaway
/// data root — plus, unless `--no-cuts`, four more sessions against a
/// throwaway copy of the config root that measure each saving. The user's
/// roots are never written.
pub fn cost(ctx: &Ctx, args: &CostArgs) -> Result<ExitCode> {
    let dctx = doctor_context(ctx, false)?;
    let report = cost::measure(
        &dctx,
        &CostOptions {
            cuts: !args.no_cuts,
        },
    )?;
    ctx.out.emit(
        || report.render().trim_end().to_string(),
        || report.to_json(),
    );
    Ok(ExitCode::SUCCESS)
}

// ---- lint ---------------------------------------------------------------

/// `omm lint [path]`: §5.3 — ids, collisions, symlinks, budgets, the
/// package limits, the committed marketplace catalogs, then the three host
/// checkpoints (R13) unless `--no-host`. Exit 1 on any error-severity
/// finding; warnings alone exit 0.
pub fn lint(ctx: &Ctx, args: &LintArgs) -> Result<ExitCode> {
    let repo = resolve_repo(args.path.as_deref())?;
    let tmp = tempdir("omm-lint-")?;
    let host: Option<Invoker> = if args.no_host {
        None
    } else {
        Some(ctx.invoker()?.clone().cwd(tmp.path()))
    };
    let report = lint::run(
        &repo,
        &LintOptions {
            host: host.as_ref(),
            check_marketplace: true,
        },
    )?;
    let view = LintView {
        repo: &repo,
        report: &report,
        host_skipped: args.no_host,
    };
    ctx.out.report(&view);
    Ok(if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// A lint report rendered against its repository (paths repo-relative).
struct LintView<'a> {
    repo: &'a Repo,
    report: &'a LintReport,
    /// `--no-host` was given (so "skipped" is by choice, not a failure).
    host_skipped: bool,
}

impl LintView<'_> {
    fn finding_lines(&self) -> Vec<String> {
        let mut lines = Vec::with_capacity(self.report.findings.len() * 2);
        for f in &self.report.findings {
            lines.push(format!(
                "{:<7} {:<30} {}: {}",
                severity_label(f.severity),
                f.rule,
                self.repo.relative(&f.path),
                f.message
            ));
            lines.push(format!("        fix: {}", f.fix));
        }
        lines
    }

    fn summary_line(&self) -> String {
        let mut s = format!(
            "omm lint {}: {} error(s), {} warning(s)",
            self.repo.root.display(),
            self.report.errors().len(),
            self.report.warnings().len()
        );
        if let Some(b) = &self.report.budget {
            s.push_str(&format!(
                "; catalog estimate {} / {} B (first_sentence {} / {} B; {} B with the built-ins disabled)",
                b.total_full,
                b.limit_full,
                b.total_first_sentence,
                b.limit_first_sentence,
                b.limit_builtins_disabled
            ));
        }
        if let Some(n) = self.report.package_files {
            s.push_str(&format!("; package files {n}"));
        }
        s.push_str(if self.report.host_checked {
            "; host checkpoints ran"
        } else if self.host_skipped {
            "; host checkpoints skipped (--no-host)"
        } else {
            "; host checkpoints did not run"
        });
        s
    }
}

impl Render for LintView<'_> {
    fn render(&self) -> String {
        let mut lines = self.finding_lines();
        lines.push(self.summary_line());
        lines.join("\n")
    }

    fn to_json(&self) -> Value {
        let findings: Vec<Value> = self
            .report
            .findings
            .iter()
            .map(|f| {
                json!({
                    "rule": f.rule,
                    "severity": severity_label(f.severity),
                    "path": self.repo.relative(&f.path),
                    "message": f.message,
                    "fix": f.fix,
                })
            })
            .collect();
        json!({
            "repo": self.repo.root,
            "clean": self.report.is_clean(),
            "exit_code": if self.report.is_clean() { 0 } else { 1 },
            "errors": self.report.errors().len(),
            "warnings": self.report.warnings().len(),
            "findings": findings,
            "budget": self.report.budget.as_ref().map(budget_json),
            "host_checked": self.report.host_checked,
            "package_files": self.report.package_files,
        })
    }
}

fn severity_label(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// The budget estimate (R18) as JSON: the three limits of host-reality.md
/// "Budgets" and whether the bundle fits each.
fn budget_json(b: &BudgetReport) -> Value {
    json!({
        "entries": b.entries.len(),
        "total_full": b.total_full,
        "limit_full": b.limit_full,
        "within_full": b.within_full(),
        "total_first_sentence": b.total_first_sentence,
        "limit_first_sentence": b.limit_first_sentence,
        "within_first_sentence": b.within_first_sentence(),
        "limit_builtins_disabled": b.limit_builtins_disabled,
        "stale": b.stale().iter().map(|e| e.id.clone()).collect::<Vec<_>>(),
    })
}

// ---- build --------------------------------------------------------------

/// `omm build`: §5.2 — lint (pure rules + R13 host checkpoints; a lint
/// error refuses the build, exit 1), regenerate the native package, the two
/// projections and the three marketplace catalogs with the digest obtained
/// from the binary, and land them in the repo — or, under `--dry-run`, plan
/// the same writes and land none. Ends with the converge summary per
/// destination. `--check` is the CI drift gate instead.
pub fn build(ctx: &Ctx, args: &BuildArgs) -> Result<ExitCode> {
    let repo = resolve_repo(args.path.as_deref())?;
    let tmp = tempdir("omm-build-")?;
    let inv = ctx.invoker()?.clone().cwd(tmp.path());
    if args.check {
        return build_check(ctx, &repo, &inv);
    }
    let lint_report = lint::run(
        &repo,
        &LintOptions {
            host: Some(&inv),
            check_marketplace: false,
        },
    )?;
    let lint_view = LintView {
        repo: &repo,
        report: &lint_report,
        host_skipped: false,
    };
    if !lint_report.is_clean() {
        ctx.out.emit(
            || {
                format!(
                    "{}\nomm build: refused — {} lint error(s); nothing was written",
                    lint_view.render(),
                    lint_report.errors().len()
                )
            },
            || {
                json!({
                    "repo": repo.root,
                    "refused": true,
                    "exit_code": 1,
                    "dry_run": ctx.dry_run,
                    "lint": lint_view.to_json(),
                })
            },
        );
        return Ok(ExitCode::from(1));
    }
    let catalog = Catalog::load(&repo.catalog_path())?;
    let content = Content::load(&catalog, &repo.content_dir())?;
    let out = generate::build(
        &repo,
        &catalog,
        &content,
        &BuildOptions {
            version: None,
            description: None,
            digest: DigestSource::Host(&inv),
        },
    )?;
    let summary = if ctx.dry_run {
        plan_write(&repo, &out)?
    } else {
        generate::write(&repo, &out)?
    };
    let converge = converge_of(&summary, &out.plugin_id, ctx.dry_run);
    let view = BuildView {
        repo: &repo,
        out: &out,
        summary: &summary,
        converge: &converge,
        lint: &lint_view,
    };
    ctx.out.report(&view);
    Ok(ExitCode::SUCCESS)
}

/// `omm build --check`: regenerate in memory with the committed version and
/// the digest from the binary, diff against the committed files; exit 1 on
/// drift. A version bump alone is never drift (`omm_manifest::drift`).
fn build_check(ctx: &Ctx, repo: &Repo, inv: &Invoker) -> Result<ExitCode> {
    let report = drift::check(repo, Some(inv))?;
    let view = DriftView {
        repo,
        report: &report,
    };
    ctx.out.report(&view);
    Ok(if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

/// What a real `generate::write` would do, computed without writing: every
/// generated file against the committed tree, the three catalogs against
/// their bytes on disk.
fn plan_write(repo: &Repo, out: &BuildOutput) -> Result<WriteSummary> {
    let catalogs = [
        (repo.marketplace_native_path(), &out.marketplace_native),
        (repo.marketplace_codex_path(), &out.marketplace_codex),
        (repo.marketplace_claude_path(), &out.marketplace_claude),
    ]
    .into_iter()
    .map(|(path, bytes)| {
        let changed = std::fs::read(&path).map(|c| &c != bytes).unwrap_or(true);
        (repo.relative(&path), changed)
    })
    .collect();
    let catalog_path = repo.catalog_path();
    let catalog_changed = std::fs::read(&catalog_path)
        .map(|c| c != out.catalog)
        .unwrap_or(true);
    Ok(WriteSummary {
        native: plan_package(&out.native, &repo.native_package_dir(&out.plugin_id))?,
        claude: plan_package(&out.claude, &repo.dist_claude_dir())?,
        codex: plan_package(&out.codex, &repo.dist_codex_dir())?,
        catalogs,
        catalog: Some((repo.relative(&catalog_path), catalog_changed)),
    })
}

/// The write report `Package::write_to` would produce for `generated` over
/// the tree at `committed_dir` (missing → written; equal → unchanged;
/// committed-only → removed). A symlink inside the committed tree is an
/// error here (a real write removes it): the plan names it instead.
fn plan_package(generated: &Package, committed_dir: &Path) -> Result<WriteReport> {
    let committed = Package::read_from(committed_dir)?;
    let mut report = WriteReport::default();
    for (path, bytes) in generated.files() {
        match committed.get(path) {
            Some(c) if c == bytes => report.unchanged.push(path.to_string()),
            _ => report.written.push(path.to_string()),
        }
    }
    for (path, _) in committed.files() {
        if generated.get(path).is_none() {
            report.removed.push(path.to_string());
        }
    }
    Ok(report)
}

/// The converge summary of a build: one category per destination tree
/// (`plugins/<pid>`, `dist/claude`, `dist/codex`) and one for the three
/// catalogs. A non-empty foreign directory `write_to` left in place counts
/// as skipped.
fn converge_of(summary: &WriteSummary, plugin_id: &str, dry_run: bool) -> Converge {
    let mut converge = Converge::new(dry_run);
    let trees = [
        (Repo::native_package_rel(plugin_id), &summary.native),
        (Repo::dist_claude_rel(), &summary.claude),
        (Repo::dist_codex_rel(), &summary.codex),
    ];
    for (name, r) in trees {
        converge
            .record_n(name.clone(), Action::Updated, r.written.len())
            .record_n(name.clone(), Action::Unchanged, r.unchanged.len())
            .record_n(name.clone(), Action::Removed, r.removed.len())
            .record_n(name, Action::Skipped, r.kept_foreign.len());
    }
    for (_, changed) in &summary.catalogs {
        converge.record(
            "marketplace catalogs",
            if *changed {
                Action::Updated
            } else {
                Action::Unchanged
            },
        );
    }
    if let Some((_, changed)) = &summary.catalog {
        converge.record(
            "catalog budget",
            if *changed {
                Action::Updated
            } else {
                Action::Unchanged
            },
        );
    }
    converge
}

/// A finished build: header, lint warnings, the converge table.
struct BuildView<'a> {
    repo: &'a Repo,
    out: &'a BuildOutput,
    summary: &'a WriteSummary,
    converge: &'a Converge,
    lint: &'a LintView<'a>,
}

impl Render for BuildView<'_> {
    fn render(&self) -> String {
        let mut lines = vec![format!(
            "omm build {} — plugin `{}` — digest {} — {}",
            self.out.version,
            self.out.plugin_id,
            self.out.digest,
            self.repo.root.display()
        )];
        lines.extend(self.lint.finding_lines());
        for tree in [
            &self.summary.native,
            &self.summary.claude,
            &self.summary.codex,
        ] {
            for kept in &tree.kept_foreign {
                lines.push(format!("kept foreign: {kept}"));
            }
        }
        lines.push(self.converge.render());
        lines.join("\n")
    }

    fn to_json(&self) -> Value {
        let kept: Vec<&String> = [
            &self.summary.native,
            &self.summary.claude,
            &self.summary.codex,
        ]
        .into_iter()
        .flat_map(|t| t.kept_foreign.iter())
        .collect();
        json!({
            "repo": self.repo.root,
            "plugin_id": self.out.plugin_id,
            "version": self.out.version,
            "description": self.out.description,
            "digest": self.out.digest,
            "dry_run": self.converge.is_dry_run(),
            "exit_code": 0,
            "lint": self.lint.to_json(),
            "kept_foreign": kept,
            "converge": self.converge.to_json(),
        })
    }
}

/// The drift gate's result, paths repo-relative.
struct DriftView<'a> {
    repo: &'a Repo,
    report: &'a DriftReport,
}

impl Render for DriftView<'_> {
    fn render(&self) -> String {
        let r = self.report;
        let versions = format!(
            "committed version {}, crate version {}{}; digest {}",
            r.committed_version.as_deref().unwrap_or("none"),
            r.crate_version,
            if r.version_bump_pending() {
                " (bump pending — not drift by itself)"
            } else {
                ""
            },
            if r.digest_checked {
                "re-obtained from the binary"
            } else {
                "reused from the committed catalog"
            }
        );
        if r.is_clean() {
            return format!(
                "omm build --check {}: no drift — {versions}",
                self.repo.root.display()
            );
        }
        let mut table = Table::new(["path", "drift"]);
        for d in &r.drifts {
            table.row([d.path.clone(), format!("{:?}", d.kind).to_lowercase()]);
        }
        format!(
            "omm build --check {}: {} path(s) drifted — {versions}\n{}\nfix: run `omm build` and commit the result",
            self.repo.root.display(),
            r.drifts.len(),
            table.render()
        )
    }

    fn to_json(&self) -> Value {
        let mut doc = serde_json::to_value(self.report).unwrap_or_else(|_| json!({}));
        doc["repo"] = json!(self.repo.root);
        doc["clean"] = json!(self.report.is_clean());
        doc["exit_code"] = json!(if self.report.is_clean() { 0 } else { 1 });
        doc["version_bump_pending"] = json!(self.report.version_bump_pending());
        doc
    }
}

// ---- shared -------------------------------------------------------------

/// The repository `lint` / `build` act on: the given path (a repo root or
/// its `content/` directory), else the checkout containing the cwd. A
/// shipped binary never assumes it runs inside the repo (`Repo` docs), so
/// nothing here is baked in at compile time.
fn resolve_repo(path: Option<&Path>) -> Result<Repo> {
    let root = match path {
        Some(p) => {
            let p = fsx::canonicalize(p)?;
            repo_root_of(&p).ok_or_else(|| {
                OmmError::Usage(format!(
                    "{}: neither a repository root ({CONTENT_DIR}/{CATALOG_FILE}) nor a content directory ({CATALOG_FILE})",
                    p.display()
                ))
            })?
        }
        None => {
            let cwd = std::env::current_dir().map_err(|e| OmmError::io("current directory", e))?;
            find_repo_root(&cwd).ok_or_else(|| {
                OmmError::Usage(format!(
                    "not inside an oh-my-musecode checkout (no {CONTENT_DIR}/{CATALOG_FILE} in {} or above); pass the repository path",
                    cwd.display()
                ))
            })?
        }
    };
    Ok(Repo::new(&root)?)
}

/// `p` as a repository root (`<p>/content/catalog.json`) or as the content
/// directory itself (`<p>/catalog.json` → its parent is the root).
fn repo_root_of(p: &Path) -> Option<PathBuf> {
    if p.join(CONTENT_DIR).join(CATALOG_FILE).is_file() {
        return Some(p.to_path_buf());
    }
    if p.join(CATALOG_FILE).is_file() {
        return p.parent().map(Path::to_path_buf);
    }
    None
}

/// The nearest ancestor of `start` (itself included) holding
/// `content/catalog.json`.
fn find_repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| d.join(CONTENT_DIR).join(CATALOG_FILE).is_file())
        .map(Path::to_path_buf)
}

/// A temp dir for a sandboxed host run (removed on drop).
fn tempdir(prefix: &str) -> Result<tempfile::TempDir> {
    tempfile::Builder::new()
        .prefix(prefix)
        .tempdir()
        .map_err(|e| OmmError::io("create temp dir", e))
}

/// A report's exit code as the process exit status (anything outside `u8`
/// is a failure).
fn code(n: i32) -> ExitCode {
    ExitCode::from(u8::try_from(n).unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omm_manifest::drift::{Drift, DriftKind};
    use omm_manifest::lint::Finding;

    fn temp_repo() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        std::fs::create_dir_all(root.join(CONTENT_DIR).join("skills")).unwrap();
        std::fs::write(
            root.join(CONTENT_DIR).join(CATALOG_FILE),
            b"{\"schema_version\":1,\"plugin_id\":\"omm\",\"assets\":[]}",
        )
        .unwrap();
        (tmp, root)
    }

    #[test]
    fn repo_root_is_found_from_the_root_the_content_dir_or_below() {
        let (_tmp, root) = temp_repo();
        assert_eq!(repo_root_of(&root), Some(root.clone()));
        assert_eq!(repo_root_of(&root.join(CONTENT_DIR)), Some(root.clone()));
        assert_eq!(repo_root_of(&root.join(CONTENT_DIR).join("skills")), None);
        assert_eq!(
            find_repo_root(&root.join(CONTENT_DIR).join("skills")),
            Some(root.clone())
        );
        assert_eq!(find_repo_root(&root), Some(root.clone()));
        let other = tempfile::tempdir().unwrap();
        assert_eq!(find_repo_root(other.path()), None);
        assert_eq!(repo_root_of(other.path()), None);
        // resolve_repo: explicit paths canonicalize; a bad one is a usage error (exit 2).
        assert_eq!(
            resolve_repo(Some(&root.join(CONTENT_DIR))).unwrap().root,
            root
        );
        let err = resolve_repo(Some(other.path())).unwrap_err();
        assert!(matches!(err, OmmError::Usage(_)), "{err}");
        assert_eq!(err.exit_code(), 2);
        assert!(resolve_repo(Some(&root.join("missing"))).is_err());
    }

    #[test]
    fn a_planned_write_reports_written_unchanged_and_removed_without_touching_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let committed = tmp.path().join("plugins").join("omm");
        std::fs::create_dir_all(committed.join("skills")).unwrap();
        std::fs::write(committed.join("same.txt"), b"same").unwrap();
        std::fs::write(committed.join("skills").join("old.md"), b"old").unwrap();
        std::fs::write(committed.join("changed.txt"), b"before").unwrap();
        let mut generated = Package::new();
        generated.insert("same.txt", b"same".to_vec()).unwrap();
        generated.insert("changed.txt", b"after".to_vec()).unwrap();
        generated.insert("new/file.txt", b"new".to_vec()).unwrap();
        let plan = plan_package(&generated, &committed).unwrap();
        assert_eq!(plan.written, vec!["changed.txt", "new/file.txt"]);
        assert_eq!(plan.unchanged, vec!["same.txt"]);
        assert_eq!(plan.removed, vec!["skills/old.md"]);
        assert!(plan.kept_foreign.is_empty());
        // Nothing moved.
        assert_eq!(
            std::fs::read(committed.join("changed.txt")).unwrap(),
            b"before"
        );
        assert!(committed.join("skills").join("old.md").exists());
        assert!(!committed.join("new").exists());
        // A missing committed tree: everything is written.
        let plan = plan_package(&generated, &tmp.path().join("absent")).unwrap();
        assert_eq!(plan.written.len(), 3);
        assert!(plan.unchanged.is_empty() && plan.removed.is_empty());
    }

    #[test]
    fn the_converge_summary_has_one_category_per_destination() {
        let summary = WriteSummary {
            native: WriteReport {
                written: vec!["a".into(), "b".into()],
                unchanged: vec!["c".into()],
                removed: vec!["d".into()],
                kept_foreign: vec!["x: not empty".into()],
            },
            claude: WriteReport::default(),
            codex: WriteReport {
                unchanged: vec!["e".into()],
                ..WriteReport::default()
            },
            catalogs: vec![
                ("marketplace.json".into(), true),
                (".agents/plugins/marketplace.json".into(), false),
                (".claude-plugin/marketplace.json".into(), false),
            ],
            catalog: Some(("content/catalog.json".into(), true)),
        };
        let c = converge_of(&summary, "omm", true);
        let budget = c.category("catalog budget").unwrap();
        assert_eq!((budget.updated, budget.unchanged), (1, 0));
        assert!(c.is_dry_run());
        let native = c.category("plugins/omm").unwrap();
        assert_eq!(
            (
                native.updated,
                native.unchanged,
                native.removed,
                native.skipped
            ),
            (2, 1, 1, 1)
        );
        assert_eq!(c.category("dist/codex").unwrap().unchanged, 1);
        assert_eq!(c.category("dist/claude").unwrap().total(), 0);
        let catalogs = c.category("marketplace catalogs").unwrap();
        assert_eq!((catalogs.updated, catalogs.unchanged), (1, 2));
        // 2 native + 1 marketplace catalog + the refreshed content/catalog.json.
        assert_eq!(c.total().updated, 4);
        assert!(!c.is_noop());
        assert!(c.render().ends_with("dry run: nothing was written"));
        let noop = converge_of(&WriteSummary::default(), "omm", false);
        assert!(noop.is_noop());
    }

    #[test]
    fn lint_view_renders_relative_paths_and_a_summary() {
        let (_tmp, root) = temp_repo();
        let repo = Repo::new(&root).unwrap();
        let report = LintReport {
            findings: vec![
                Finding::error(
                    "id-prefix",
                    root.join("content/skills/x/SKILL.md"),
                    "id `x` lacks the omm- prefix",
                    "rename it omm-x",
                ),
                Finding::warning(
                    "catalog-budget-stale",
                    root.join("content/catalog.json"),
                    "budget_bytes 10 != 12",
                    "run omm build",
                ),
            ],
            budget: None,
            host_checked: false,
            package_files: Some(7),
        };
        let view = LintView {
            repo: &repo,
            report: &report,
            host_skipped: true,
        };
        let text = view.render();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("error   id-prefix"), "{}", lines[0]);
        assert!(lines[0].contains(" content/skills/x/SKILL.md: id `x`"));
        assert_eq!(lines[1], "        fix: rename it omm-x");
        assert!(lines[2].starts_with("warning catalog-budget-stale"));
        assert!(
            lines[4].contains("1 error(s), 1 warning(s)"),
            "{}",
            lines[4]
        );
        assert!(lines[4].contains("package files 7"));
        assert!(lines[4].contains("skipped (--no-host)"));
        let j = view.to_json();
        assert_eq!(j["clean"], false);
        assert_eq!(j["exit_code"], 1);
        assert_eq!(j["errors"], 1);
        assert_eq!(j["warnings"], 1);
        assert_eq!(j["findings"][0]["path"], "content/skills/x/SKILL.md");
        assert_eq!(j["findings"][0]["severity"], "error");
        assert_eq!(j["findings"][1]["severity"], "warning");
        assert_eq!(j["budget"], Value::Null);
        assert_eq!(j["package_files"], 7);
        assert_eq!(j["host_checked"], false);
        // Clean report: exit 0, summary only.
        let clean = LintReport::default();
        let view = LintView {
            repo: &repo,
            report: &clean,
            host_skipped: false,
        };
        assert_eq!(view.render().lines().count(), 1);
        assert!(view.render().contains("did not run"));
        assert_eq!(view.to_json()["exit_code"], 0);
    }

    #[test]
    fn drift_view_distinguishes_clean_from_drifted_and_names_a_pending_bump() {
        let (_tmp, root) = temp_repo();
        let repo = Repo::new(&root).unwrap();
        let clean = DriftReport {
            drifts: vec![],
            committed_version: Some("0.1.0".into()),
            crate_version: "0.2.0".into(),
            digest_checked: true,
        };
        let view = DriftView {
            repo: &repo,
            report: &clean,
        };
        let text = view.render();
        assert!(text.contains("no drift"), "{text}");
        assert!(text.contains("bump pending"), "{text}");
        assert!(text.contains("re-obtained from the binary"));
        let j = view.to_json();
        assert_eq!(j["clean"], true);
        assert_eq!(j["exit_code"], 0);
        assert_eq!(j["version_bump_pending"], true);
        assert_eq!(j["digest_checked"], true);
        let dirty = DriftReport {
            drifts: vec![
                Drift {
                    path: "plugins/omm/skills/omm-x/SKILL.md".into(),
                    kind: DriftKind::Changed,
                },
                Drift {
                    path: "marketplace.json".into(),
                    kind: DriftKind::Missing,
                },
            ],
            committed_version: None,
            crate_version: "0.1.0".into(),
            digest_checked: false,
        };
        let view = DriftView {
            repo: &repo,
            report: &dirty,
        };
        let text = view.render();
        assert!(text.contains("2 path(s) drifted"), "{text}");
        assert!(
            text.contains("plugins/omm/skills/omm-x/SKILL.md  changed"),
            "{text}"
        );
        assert!(
            text.contains("marketplace.json                   missing"),
            "{text}"
        );
        assert!(text.ends_with("fix: run `omm build` and commit the result"));
        let j = view.to_json();
        assert_eq!(j["clean"], false);
        assert_eq!(j["exit_code"], 1);
        assert_eq!(j["drifts"][1]["kind"], "missing");
        assert_eq!(j["committed_version"], Value::Null);
    }

    #[test]
    fn drift_rows_print_only_the_failed_hostcheck_rows_unless_all() {
        use hostcheck::{Check, Report, Status, Tier};
        let row = |id: &str, status: Status, detail: &str| Check {
            id: id.into(),
            tier: Tier::P0,
            expected: format!("exp-{id}"),
            observed: format!("obs-{id}"),
            status,
            detail: detail.into(),
        };
        let report = Report {
            binary: PathBuf::from("/bin/muse-bin-x"),
            version: Some("1.0.1".into()),
            checks: vec![
                row("version", Status::Pass, ""),
                row("gates/default-on", Status::Fail, "two gates flipped"),
                row("data-files", Status::Pass, ""),
            ],
            elapsed_ms: 12,
        };
        let view = DriftRows {
            report: &report,
            all: false,
        };
        let text = view.render();
        assert!(
            text.starts_with("host drift: /bin/muse-bin-x (1.0.1) — 1 of 3 rows drifted (12 ms)")
        );
        assert!(text.contains("gates/default-on"));
        assert!(!text.contains("data-files"), "{text}");
        assert!(text.contains("gates/default-on: two gates flipped"));
        assert!(text.contains("fix: re-measure"));
        let j = view.to_json();
        assert_eq!(j["drifted"], json!(["gates/default-on"]));
        assert_eq!(j["exit_code"], 1);
        assert_eq!(j["checks"].as_array().unwrap().len(), 3);
        let all = DriftRows {
            report: &report,
            all: true,
        };
        assert!(all.render().contains("data-files"));
        let clean = Report {
            checks: report
                .checks
                .iter()
                .cloned()
                .map(|mut c| {
                    c.status = Status::Pass;
                    c
                })
                .collect(),
            ..report.clone()
        };
        let view = DriftRows {
            report: &clean,
            all: false,
        };
        assert!(view
            .render()
            .ends_with("no drift: every P0/P1 row matches docs/host-reality.md"));
        assert_eq!(view.to_json()["drifted"], json!([]));
        assert_eq!(view.to_json()["exit_code"], 0);
    }

    #[test]
    fn budget_json_carries_the_three_limits() {
        let b = BudgetReport {
            entries: vec![],
            total_full: 10,
            total_first_sentence: 8,
            limit_full: 21_542,
            limit_first_sentence: 27_168,
            limit_builtins_disabled: 31_602,
        };
        let j = budget_json(&b);
        assert_eq!(j["within_full"], true);
        assert_eq!(j["limit_builtins_disabled"], 31_602);
        assert_eq!(j["stale"], json!([]));
        assert_eq!(j["entries"], 0);
    }

    #[test]
    fn exit_codes_clamp_to_u8() {
        let show = |c: ExitCode| format!("{c:?}");
        assert_eq!(show(code(0)), show(ExitCode::SUCCESS));
        assert_eq!(show(code(1)), show(ExitCode::from(1)));
        assert_eq!(show(code(-1)), show(ExitCode::from(1)));
        assert_eq!(show(code(300)), show(ExitCode::from(1)));
    }
}
