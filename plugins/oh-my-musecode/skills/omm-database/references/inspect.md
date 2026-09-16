# Inspect, plan, verify per engine

Commands for section 0 (schema), 1 (plan), 3 (locks), 5 (backup). Confirm the
engine and version with `bash` first; several columns below are version-gated.

## Schema and statistics

| Need | PostgreSQL | MySQL / MariaDB | SQLite |
|---|---|---|---|
| version | `SELECT version();` | `SELECT @@version;` | `SELECT sqlite_version();` |
| table + indexes | `\d+ t` (psql) or `SELECT indexdef FROM pg_indexes WHERE tablename='t';` | `SHOW CREATE TABLE t; SHOW INDEX FROM t;` | `.schema t` or `PRAGMA index_list('t'); PRAGMA index_info('i');` |
| row estimate | `SELECT reltuples::bigint FROM pg_class WHERE relname='t';` | `SELECT table_rows FROM information_schema.tables WHERE table_name='t';` | `SELECT count(*) FROM t;` (small only) |
| table + index size | `SELECT pg_size_pretty(pg_total_relation_size('t'));` | `data_length + index_length` from `information_schema.tables` | `PRAGMA page_count; PRAGMA page_size;` |
| refresh statistics | `ANALYZE t;` | `ANALYZE TABLE t;` | `ANALYZE t;` |
| unused indexes | `SELECT indexrelname, idx_scan FROM pg_stat_user_indexes WHERE idx_scan = 0;` | `sys.schema_unused_indexes` (8.0+) | none; read the plans |
| identifier quoting | `"name"` | `` `name` `` | `"name"` |

## Plan

| Engine | Command | Key fields |
|---|---|---|
| PostgreSQL | `EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT) <stmt>;` | `actual time`, `rows` vs estimate, `Rows Removed by Filter`, `Sort Method`, `Buffers: shared read` (disk) |
| MySQL 8+ | `EXPLAIN ANALYZE <stmt>;` (`EXPLAIN FORMAT=JSON` for cost) | `actual time`, `rows`, `Table scan on`, `Index lookup on`, `Filter:` |
| MySQL 5.7 | `EXPLAIN <stmt>;` | `type` (`ALL` = full scan), `key`, `rows`, `Extra: Using filesort`, `Using temporary` |
| SQLite | `EXPLAIN QUERY PLAN <stmt>;` | `SCAN t` (full) vs `SEARCH t USING INDEX`, `USE TEMP B-TREE FOR ORDER BY` |
| SQL Server | `SET STATISTICS PROFILE ON;` then the statement | `Table Scan`, `Key Lookup`, `EstimateRows` vs `Rows` |

Plan a write without applying it: `BEGIN; EXPLAIN ANALYZE <update>; ROLLBACK;`
(PostgreSQL, SQLite). MySQL `EXPLAIN ANALYZE` on a write executes it: use a copy.

Node vocabulary, PostgreSQL names (others differ, same shapes):

- Seq Scan + Filter removing most rows: missing or unmatched index.
- Index Scan + Filter removing most rows: index leading column or expression does
  not match; or the predicate casts the column (`WHERE id::text = $1`).
- Index Only Scan with `Heap Fetches` high: table needs `VACUUM`.
- Bitmap Heap Scan: fine for medium selectivity; a sign the index is not
  selective when it returns most of the table.
- Nested Loop with outer rows in the thousands: inner side must be an index
  lookup, or the join needs a Hash Join (statistics or a missing index).
- Sort `external merge Disk`: `work_mem` too small for the sort, or the sort
  should be an index.
- Estimate off by 10x or more anywhere: `ANALYZE` first. Persisting: correlated
  columns, `CREATE STATISTICS` (10+).

## Counting statements per request

- Django: `django.db.connection.queries` in a test with `CaptureQueriesContext`,
  or `assertNumQueries(n)`; `select_related` (FK) / `prefetch_related` (M2M, reverse).
- Rails: `ActiveRecord::Base.logger = Logger.new($stdout)`; `includes`,
  `preload`, `eager_load`; Bullet gem in test.
- SQLAlchemy: `echo=True` on the engine; `selectinload`, `joinedload`.
- Prisma: `log: ['query']` in the client options; `include`.
- Knex / Objection: `.on('query', ...)`; `withGraphFetched`.
- Entity Framework: `LogTo(Console.WriteLine)`; `Include`.
- sqlx (Rust): `RUST_LOG=sqlx=debug`.
- Engine side: `pg_stat_statements` (`calls`, `mean_exec_time`), MySQL
  `performance_schema.events_statements_summary_by_digest`.

## Locks and long transactions (PostgreSQL)

```sql
SELECT pid, now() - xact_start AS age, state, left(query, 80)
FROM pg_stat_activity
WHERE xact_start < now() - interval '5 seconds' ORDER BY xact_start;

SELECT blocked.pid, blocked.query AS blocked_query, blocking.pid AS by_pid, blocking.query
FROM pg_stat_activity blocked
JOIN pg_stat_activity blocking ON blocking.pid = ANY(pg_blocking_pids(blocked.pid));
```

MySQL: `SHOW ENGINE INNODB STATUS\G` (TRANSACTIONS section),
`SELECT * FROM sys.innodb_lock_waits;`. SQLite: one writer; `PRAGMA busy_timeout`.

Retryable error codes: PostgreSQL `40001` (serialization_failure), `40P01`
(deadlock_detected); MySQL `1213` (deadlock), `1205` (lock wait timeout);
SQLite `SQLITE_BUSY` (5). Anything else is a bug in the statement, not a retry.

## Backup verification

A backup is verified when a restore of it into a scratch database yields a count
or checksum that matches the source. Paste both numbers.

| Engine | Take | Restore to scratch | Check |
|---|---|---|---|
| PostgreSQL | `pg_dump -Fc -f t.dump -t t db` | `createdb scratch && pg_restore -d scratch t.dump` | `SELECT count(*), sum(hashtext(id::text)) FROM t;` on both |
| MySQL | `mysqldump db t > t.sql` | `mysql scratch < t.sql` | `CHECKSUM TABLE t;` on both |
| SQLite | `sqlite3 db.sqlite ".backup t.bak"` | `sqlite3 t.bak` | `SELECT count(*) FROM t;` on both |
| Managed (RDS, Cloud SQL, etc.) | snapshot in the console or CLI | restore to a new instance | same count query against the new instance |

`ls -l` on a dump file is not verification. A snapshot whose restore was never
attempted is not verification.
