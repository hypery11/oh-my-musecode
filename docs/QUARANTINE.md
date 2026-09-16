# Ledger quarantine runbook

When `omm` loads `$OMM/omm.lock.json` through `omm-ledger::store` and the
file is corrupt — unparseable, wrong `schema_version`, duplicate key,
invalid path — it is **renamed, never deleted**, to
`omm.lock.json.bad-<ts>` (a `-n` suffix when the timestamp collides), an
audit line (`ACTION_QUARANTINE`) is appended, and the session continues with
the ledger treated as absent. The quarantine record (`from`, `to`, `detail`)
is surfaced loudly by the session (`warn_quarantine`); a later write adopts
whatever the plan rebuilds.

Owning code: `crates/omm-ledger/src/store.rs` (`quarantine`,
`is_quarantine_name`, `BAD_SUFFIX`), `crates/omm/src/cmd/a_lifecycle/session.rs`
(`warn_quarantine`).

## What does not quarantine

`omm doctor` reads through the read-only lane (`omm-doctor::ledger::load`),
which never writes: a corrupt ledger is reported (D13) and left for
`omm reconcile`. Only a write path (`install`, `update`, `reconcile`, …)
moves the file aside.

## Triage

1. Read the warning: it names the `from → to` pair and the parse detail.
2. Confirm scope: `ls "$OMM"/omm.lock.json.bad-*` — earlier quarantines are
   never overwritten, so more than one means repeated corruption.
3. Check the audit log for the matching quarantine line and what ran after
   (a rebuild already happened when a later command succeeded).
4. Inspect, do not hand-edit, the `.bad-<ts>` file: it is evidence of what
   the ledger held. A truncated file usually still parses partially —
   every registration that parses can be re-adopted (see below).

## Recovery

- Normal path: `omm reconcile` (then `omm install --no-plugin --source
  <checkout>` when doctor still asks for it). The rebuilt ledger cannot know
  priors the corrupt file no longer carries: seeded settings keys and trust
  entries come back under "kept, prior unknown" and a later uninstall leaves
  them in place while naming them (`docs/KNOWN_ISSUES.md` §1).
- When the quarantined file parses as JSON but fails validation, prefer
  re-adopting its `settings-key` / `trust` registrations (they carry `prior`
  and `value`) over re-seeding from scratch.
- Never delete a `.bad-<ts>` file to "fix" the warning: the warning follows
  the record, and deleting evidence removes the only copy of the priors.

## Retention

Quarantines are evidence, not snapshots: nothing prunes them automatically
(snapshots under `$OMM/snapshots/` keep 5 and are the rolling mechanism).
Remove a `.bad-<ts>` file by hand only after the rebuilt ledger has been
verified (`omm doctor`, and an `omm uninstall --dry-run` that names nothing
unexpected).
