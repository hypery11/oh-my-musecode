---
name: "omm-migrate"
description: "Use for migration, alter table, schema change, or backfill: expand-migrate-contract, compat window, idempotent forward-only script, dry run on a copy, rollback plan first, invariants verified; Do not use when only code moves (omm-refactor)."
---

# Migrate

Contract: a schema or data change ships as ordered steps, each safe with the code
version before it and after it. Every step is idempotent, dry-run on a copy first,
with its rollback written in the message before it runs. Done means the verification
queries and results are pasted from `bash` output, not "migration applied". Never
edit an applied migration; write the next one.

## 0. Before writing anything

- Find the tool with `search`: `migrations/`, `alembic.ini`, `db/migrate`,
  `prisma/schema.prisma`, `knexfile*`, `flyway*`, `liquibase*`, `diesel.toml`,
  `sqlx`, `manage.py`. Use the repo's tool and naming; tool commands and engine
  lock rules: `references/engines.md` beside this file. No tool: stop and ask.
- `read_file` the two newest migrations: transaction wrapping, up/down shape, how
  raw SQL is embedded. Copy that.
- Deployment shape: do old and new code run at once (rolling deploy, replicas)?
  Unknown: assume yes; that forces the compat window.
- Blast radius with `bash`: row count and size of every table touched, indexes,
  foreign keys, dependent views and triggers. Readers and writers: `search`
  (literal mode) for the table and column names across code, ORM models, raw SQL,
  views, triggers, reports, fixtures. A site the plan does not cover is a stop.
- `write_todos`: one item per step in section 2, each with its own deploy and
  verify. Skip it for one additive change.

## 1. Classify the change

| Change | One step? | Shape |
|---|---|---|
| add nullable column, add table, add enum value | yes | expand only |
| add index | yes | online build (`CONCURRENTLY`, `LOCK=NONE`), outside a transaction |
| add NOT NULL, CHECK, or FK constraint | no | add nullable, backfill, constraint `NOT VALID`, then `VALIDATE` |
| rename column or table | no | add new, dual-write, backfill, read new, drop old: five deploys, or an agreed outage |
| change type or narrow | widen in place if the engine allows; else no | new column, dual-write, backfill, swap reads, drop |
| drop column or table | no | contract step, one release after the last reader is gone |
| backfill or data fix | no | own script, batched, idempotent, never inside the schema migration |

Every "no" row is expand-migrate-contract. One row per migration file; a migration
that needs "and" is two. Document stores, files and event payloads follow the same
shape: readers accept both forms, rewrite in batches, then reject the old.

## 2. Expand, migrate, contract

1. Expand: additive only. New column nullable or with a constant default; new
   table; index built online. Old code must ignore it. Deploy.
2. Migrate: code writes old and new, reads old. Deploy. Backfill (section 3).
   Verify (section 5). Code reads new, still writes both. Deploy.
3. Contract: stop writing old. Deploy. One release later, `search` the old name
   again; zero hits outside migrations and changelogs: drop it, own file. Never in
   the same deploy as expand.

A step the previous release cannot run against is a contract step; it waits.
Forward-only: fix a bad step with a new corrective migration, idempotent against
both states, never by editing the applied one.

## 3. Draft the script

- `write_file` new migration files only; never `edit_file` an applied one.
- Idempotent: `IF NOT EXISTS` / `IF EXISTS`, `WHERE new_col IS NULL`, check the
  catalog before altering. A crash halfway plus a rerun reaches the same end state.
- Backfill in primary-key ranges: 1k-10k rows per statement, commit each, log the
  last key so a restart resumes, short sleep between batches on a hot table. Never
  an unbounded `UPDATE` or `DELETE` on a table over ~100k rows.
- Locks: `SET lock_timeout = '5s'` and a `statement_timeout` before DDL; on timeout
  retry, never queue behind a long transaction. Which statements rewrite the table
  or take an exclusive lock, and which engines allow DDL in a transaction (MySQL
  does not: one statement per file): the reference. Backfills never in one.
- Down: write it for expand steps (drop what you added). For a contract step the
  down is "restore from backup"; say so instead of writing a fake one.

## 4. Rollback plan, written before the first run

In the message, before any `bash` run against a real database:

- What failure looks like: error text, lock wait, latency, wrong reads, and the
  query or metric that shows it.
- Per step: the exact undo command, whether the previous code version keeps
  working meanwhile, and the data lost ("none" must be true). Expand: `DROP` the
  addition. Backfill: nothing, the column is unread. Contract: irreversible; name
  the backup taken first, with its command and timestamp. A down step that drops
  populated data is labelled destructive, not rollback.
- The previous release must run against the expanded schema, or the order is wrong.

## 5. Dry run on a copy, run, verify

- Copy: `CREATE DATABASE scratch TEMPLATE prod`, `pg_dump | psql`, `mysqldump`,
  `.backup` for SQLite. No production access: fresh database plus generated data at
  production row counts; say timings are a lower bound. An empty DB tests syntax
  only.
- Run the step with `bash` (`yield_time_ms` up to 300000); paste output and wall
  time, scaled to production against the maintenance budget. Run it again: the
  second run must be a no-op (`UPDATE 0`, "already exists"), else fix before
  touching anything real. Rollback on the copy, forward again. App suite against
  the migrated copy, pasted.
- Real run only when the user asks and names the environment, with the backup
  named, the dry run pasted, and the window's code deployed. One step, paste
  output. Then the queries, pasted with results: row count before and after
  (unchanged for schema steps); rows still unfilled = 0; old vs new mismatch = 0;
  orphans against a new FK = 0; a checksum of a key column unchanged; index valid,
  constraint validated. One read and one write through the new path. Any non-zero:
  stop, do not contract, report.
- Per step in the final message: duration, rows affected, verification queries and
  results, what remains before contract.

## Judgment calls

- Small table (under ~10k rows), one process, no replicas: one step is fine; say
  why. Ceremony scales with blast radius.
- ORM autogenerated migration: `read_file` every emitted statement. Autogen drops
  what it does not recognise and turns a rename into drop plus add. Amend to the plan.
- Duplicates or nulls found by the invariant query before a constraint: stop and
  report the rows; which to keep is a product decision, not a `DISTINCT ON`.
- Lock waits or replica lag during a backfill: smaller batches, longer sleep, never
  a bigger transaction.
- User wants the fast path (`RENAME` on a live table): state the outage and the
  alternative in one line each; do what they choose and say so.

## Refuse

- A run against a real database with no pasted dry run or no named backup.
- A rollback plan written after the run, a fake `down` for a contract step, or
  "revert" without the data-loss statement.
- `DROP`, `RENAME`, or narrowing in the same deploy as the code that stops using
  the old name. Unbounded `UPDATE`/`DELETE`; a backfill inside the schema migration.
- Editing an applied migration or the migration state table by hand.
- "Applied cleanly" without the pasted verification queries.

## Micro-example (PostgreSQL)

Goal: `users.email` unique case-insensitively; 800k rows, rolling deploy. Naive
`ADD CONSTRAINT ... UNIQUE (lower(email))` locks the table for a full scan and
fails on the first duplicate. Instead: rollback plan first (`DROP COLUMN
email_norm`, no data lost; later `DROP INDEX CONCURRENTLY`). Expand: `ADD COLUMN
email_norm text`; code writes `lower(email)` into it. Deploy. Backfill 5000 ids per
batch `WHERE ... AND email_norm IS NULL`: copy, 160 batches, 41 s; second run
`UPDATE 0`; real run on the user's ask, pasted. Verify: unfilled -> 0; duplicates
-> 3 rows: stop, report the ids, ask which to merge; resolved, rerun -> 0.
`CREATE UNIQUE INDEX CONCURRENTLY` outside a transaction, then `CHECK (email_norm
IS NOT NULL) NOT VALID` and `VALIDATE`. Code reads `email_norm`. Deploy.
