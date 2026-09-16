# Engines and tools

What locks or rewrites per engine, the migration tool's commands, copy recipes,
and the verification queries. Confirm the engine version with `bash` first
(`SELECT version();`, `SELECT sqlite_version();`); several rules below are
version-gated.

## PostgreSQL

- Add nullable column, or NOT NULL with a constant default (11+): metadata only,
  brief ACCESS EXCLUSIVE lock. Volatile default (`now()`, `gen_random_uuid()`),
  most type changes: full table rewrite under ACCESS EXCLUSIVE. Widening
  `varchar(n)` or numeric precision: metadata only.
- `CREATE INDEX`: SHARE lock, blocks writes for the whole build. `CONCURRENTLY`:
  no write block, cannot run inside a transaction (mark the migration
  non-transactional in the tool), and a failed build leaves an INVALID index:
  `DROP INDEX CONCURRENTLY` and retry.
- `ADD CONSTRAINT ... FOREIGN KEY | CHECK ... NOT VALID` then
  `ALTER TABLE ... VALIDATE CONSTRAINT` (SHARE UPDATE EXCLUSIVE, no write block).
- `SET NOT NULL`: full scan under ACCESS EXCLUSIVE, unless (12+) a validated
  `CHECK (col IS NOT NULL)` already exists; then it is metadata only. Drop the
  CHECK afterwards.
- `DROP COLUMN`: metadata only but takes ACCESS EXCLUSIVE; it queues behind every
  open transaction on the table and everything queues behind it. Hence
  `lock_timeout`.
- Before DDL: `SET lock_timeout = '5s'; SET statement_timeout = '30s';`
- Watch locks while a step runs:
  `SELECT pid, mode, granted, left(query, 80) FROM pg_locks JOIN pg_stat_activity USING (pid) WHERE relation = 'users'::regclass;`
- Long-running transactions that will block DDL:
  `SELECT pid, now() - xact_start, left(query, 80) FROM pg_stat_activity WHERE xact_start < now() - interval '1 min';`

## MySQL and MariaDB

- DDL is not transactional: a failed migration leaves half a change. One
  statement per migration file.
- `ALTER TABLE ... , ALGORITHM=INSTANT` (8.0+, add or drop column, rename column)
  or `ALGORITHM=INPLACE, LOCK=NONE`. Naming the algorithm makes the server error
  instead of silently copying the table; that error is the signal.
- Type changes and most index changes on large tables: `gh-ost` or
  `pt-online-schema-change`, never a plain `ALTER`.
- `RENAME COLUMN` is instant but still breaks the old release: the compat window
  still applies.

## SQLite

- `ALTER TABLE` supports ADD COLUMN, RENAME, and DROP COLUMN (3.35+) only. Type
  change, constraint add, column reorder: create the new table, `INSERT INTO new
  SELECT ... FROM old`, drop old, `ALTER TABLE new RENAME TO old`, all in one
  transaction with `PRAGMA foreign_keys=OFF` before it and
  `PRAGMA foreign_key_check` after.
- One writer at a time: batching a backfill matters less than keeping each
  transaction short so readers are not starved.

## SQL Server

- `CREATE INDEX ... WITH (ONLINE = ON)` (Enterprise). Adding a NOT NULL column
  with a constant default is metadata only on 2012+ Enterprise.
- Schema changes take a Sch-M lock; `SET LOCK_TIMEOUT 5000;` first.

## Migration tools

| Tool | New | Apply | SQL preview | Status |
|---|---|---|---|---|
| Alembic | `alembic revision -m x` (`--autogenerate`) | `alembic upgrade head` | `alembic upgrade head --sql` | `alembic current` |
| Django | `manage.py makemigrations app` | `manage.py migrate` | `manage.py sqlmigrate app 0007` | `manage.py showmigrations` |
| Rails | `rails g migration X` | `rails db:migrate` | none; read the file | `rails db:migrate:status` |
| Prisma | `prisma migrate dev --create-only` | `prisma migrate deploy` | `prisma migrate diff --script` | `prisma migrate status` |
| Knex | `knex migrate:make x` | `knex migrate:latest` | none | `knex migrate:status` |
| Flyway | `V7__x.sql` | `flyway migrate` | `flyway validate` | `flyway info` |
| Liquibase | changeset in the changelog | `liquibase update` | `liquibase updateSQL` | `liquibase status` |
| sqlx | `sqlx migrate add x` | `sqlx migrate run` | none | `sqlx migrate info` |
| Diesel | `diesel migration generate x` | `diesel migration run` | none | `diesel migration list` |
| goose | `goose create x sql` | `goose up` | none | `goose status` |
| golang-migrate | `migrate create -ext sql -dir d x` | `migrate -path d up` | none | `migrate version` |
| Atlas | `atlas migrate diff` | `atlas migrate apply` | `atlas migrate lint` | `atlas migrate status` |
| dbmate | `dbmate new x` | `dbmate up` | `dbmate dump` | `dbmate status` |
| EF Core | `dotnet ef migrations add X` | `dotnet ef database update` | `dotnet ef migrations script` | `dotnet ef migrations list` |

Non-transactional step (`CONCURRENTLY`, MySQL DDL): Alembic
`with op.get_context().autocommit_block():`; Rails `disable_ddl_transaction!`;
Django `atomic = False`; Flyway `-- flyway:executeInTransaction=false`; Knex
`config.transaction = false`; sqlx `-- no-transaction`.

## Copy recipes

- PostgreSQL: `CREATE DATABASE scratch TEMPLATE prod;` (no open connections to
  `prod`), or `pg_dump -Fc prod | pg_restore -d scratch`. Schema only for a
  generated-data copy: `pg_dump -s prod | psql scratch`.
- MySQL: `mysqldump --single-transaction prod | mysql scratch`.
- SQLite: `sqlite3 app.db ".backup scratch.db"` (copies WAL content too; a plain
  `cp` misses `-wal`).
- SQL Server: `BACKUP DATABASE prod TO DISK = ...; RESTORE DATABASE scratch FROM
  DISK = ... WITH MOVE ...`.

## Batched backfill (psql, from bash)

```
lo=$(psql -Atc "SELECT min(id) FROM t"); hi=$(psql -Atc "SELECT max(id) FROM t")
for ((a=lo; a<=hi; a+=5000)); do
  psql -Atc "UPDATE t SET new_col = f(old_col) WHERE id BETWEEN $a AND $((a+4999)) AND new_col IS NULL"
  echo "$((a+4999))" > backfill.last   # resume point
  sleep 0.1
done
```

Rerun after a crash: start from `backfill.last`; the `IS NULL` guard makes an
overlap harmless.

## Verification queries

- Unfilled: `SELECT count(*) FROM t WHERE new_col IS NULL;`
- Mismatch: `SELECT count(*) FROM t WHERE new_col IS DISTINCT FROM f(old_col);`
- Duplicates before a unique index: `SELECT k, count(*) FROM t GROUP BY k HAVING count(*) > 1;`
- Orphans before an FK: `SELECT count(*) FROM child c LEFT JOIN parent p ON p.id = c.parent_id WHERE p.id IS NULL;`
- Checksum, before and after: `SELECT count(*), sum(amount), md5(string_agg(id::text, ',' ORDER BY id)) FROM t;`
- Index valid (PostgreSQL): `SELECT indexrelid::regclass FROM pg_index WHERE NOT indisvalid;` must return nothing.
