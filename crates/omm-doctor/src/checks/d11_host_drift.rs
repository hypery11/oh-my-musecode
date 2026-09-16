//! D11 — golden constants vs live probes: `omm_host::hostcheck` reused
//! (ARCHITECTURE.md §6 row D11; PLAN.md 0.3). About two seconds.

use omm_host::hostcheck;

use crate::check::{Check, Context};

pub const ID: &str = "D11";
pub const TITLE: &str = "host drift";
/// `docs/host-reality.md` header and "Server-side risk": version numbers
/// track the product, not the contract (R15) — 45 axes, two cosmetic
/// differences across a full minor version — and Meta can allowlist
/// extension kinds server-side without shipping a binary. Only re-measuring
/// behaviour tells; nothing in a session announces a moved constant.
pub const WHY_SILENT: &str = "the extension contract is not versioned: a moved constant (catalog cap, capability families, schema fingerprints, gate defaults) changes what composes with no session-time notice, and the version string proves nothing (host-reality.md header, \"Server-side risk\"; R15) — only re-measuring the P0/P1 rows against the binary tells";
pub const FIX: &str = "omm doctor --report-drift";

pub fn run(ctx: &Context) -> Check {
    if !ctx.options.host_drift {
        return Check::info(ID, TITLE, "skipped (host_drift off)", WHY_SILENT);
    }
    let tmp = match tempfile::Builder::new()
        .prefix("omm-doctor-hostcheck-")
        .tempdir()
    {
        Ok(t) => t,
        Err(e) => {
            return Check::warn(
                ID,
                TITLE,
                format!("could not create a sandbox: {e}"),
                WHY_SILENT,
                FIX,
            )
        }
    };
    match hostcheck::run_in(&ctx.inv, tmp.path()) {
        Ok(report) => {
            let failures = report.failures();
            if failures.is_empty() {
                // OLDER-BUILD rows are passes: `since`-tagged facts this
                // build predates (hostcheck module docs); named so the reader
                // knows which data rows the binary does not carry yet.
                let older = report.older_build();
                let older_note = if older.is_empty() {
                    String::new()
                } else {
                    format!(
                        ", {} OLDER-BUILD ({})",
                        older.len(),
                        older
                            .iter()
                            .map(|c| c.id.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                Check::info(
                    ID,
                    TITLE,
                    format!(
                        "{} P0/P1 rows match host-reality.md{older_note} ({} ms; {})",
                        report.checks.len(),
                        report.elapsed_ms,
                        report.version.as_deref().unwrap_or("version unknown")
                    ),
                    WHY_SILENT,
                )
            } else {
                let ids: Vec<String> = failures
                    .iter()
                    .take(6)
                    .map(|c| {
                        format!(
                            "{} (expected {}, observed {})",
                            c.id, c.expected, c.observed
                        )
                    })
                    .collect();
                Check::warn(
                    ID,
                    TITLE,
                    format!(
                        "{} of {} rows drifted: {}{}",
                        failures.len(),
                        report.checks.len(),
                        ids.join("; "),
                        if failures.len() > ids.len() {
                            "; …"
                        } else {
                            ""
                        }
                    ),
                    WHY_SILENT,
                    FIX,
                )
            }
        }
        Err(e) => Check::warn(
            ID,
            TITLE,
            format!("host self-test did not run: {e}"),
            WHY_SILENT,
            FIX,
        ),
    }
}
