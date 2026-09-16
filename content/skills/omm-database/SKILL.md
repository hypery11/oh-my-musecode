---
name: omm-database
description: "Use for SQL or ORM work (query is slow, add an index, N+1, transaction): schema and indexes first, plan before and after, parameters only, backup verified before deletes; Do not use for migration files or backfills (omm-migrate)."
---

# Database

Contract: no query, index, or schema decision before the schema and the plan are
read and pasted. Done means the final message shows the plan before and after on
the same data, the statement count before and after where N+1 was suspected, and
the test run, all from `bash`. "Should be faster" is not done. Every statement
carries its values as parameters; every destructive statement runs after a count
and a verified backup. Migration mechanics (files, online DDL, backfills) are
omm-migrate's: decide the index here, ship it there.

## 0. Study first

- Engine and version with `bash`: `SELECT version();`, `SELECT @@version;`,
  `SELECT sqlite_version();`. Name the database you are connected to in the
  message before the first write. Use the repo's client; never install another.
- Schema of every table the task touches, from the engine, not from the model
  file: `\d+ t` (psql), `SHOW CREATE TABLE t` (MySQL), `.schema t` (SQLite);
  per-engine commands in `references/inspect.md` beside this file. Record: primary
  key, every index with its column order, foreign keys, approximate row count.
- The SQL the ORM emits, not the ORM call: query log, `str(qs.query)`,
  `.toSQL()`. An ORM line whose SQL you have not seen is an unread query.
- More than one query or table: `write_todos`, one item per query.

## 1. Plan before changing anything

- `EXPLAIN (ANALYZE, BUFFERS)` (PostgreSQL), `EXPLAIN ANALYZE` (MySQL 8+),
  `EXPLAIN QUERY PLAN` (SQLite), on production-shaped row counts: a 200-row dev
  database calls every scan fine. Load a copy or generate rows to the real count;
  say when you could not. `ANALYZE` executes the statement: plan a write inside
  `BEGIN ... ROLLBACK`.
- Paste the plan, then read it in this order: the node with the largest actual
  time; estimated vs actual rows off by 10x (stale statistics: `ANALYZE t`,
  re-plan); Seq Scan with a selective Filter; Index Scan followed by a Filter
  that removes most rows (index does not match); Sort spilling to `Disk`.
- One hypothesis with a predicted effect, written before the edit: "index on
  `(status, created_at)` turns Sort + Seq Scan into an Index Scan, 1.8 s to 5 ms."
- Statement count before statement time: one fast query per row in the log is
  N+1. Count per request (ORM log, `pg_stat_statements`) first.

## 2. Change

| Plan or symptom | Change |
|---|---|
| Seq Scan, selective WHERE, big table | index on the predicate; composite: equality columns first, range or sort column last |
| index exists, Filter removes most rows | match the index to the query: leading column, `lower(col)` expression index, cast or function moved off the column, no leading-wildcard `LIKE` |
| one query per row in the log | N+1: JOIN, `WHERE id = ANY($1)` batch, or the ORM's eager load (`select_related`, `includes`, `include`) |
| `OFFSET n` growing with the page | keyset: `WHERE (sort_col, id) > ($1, $2) ORDER BY sort_col, id LIMIT k` |
| Sort node under `ORDER BY ... LIMIT` | index in the sort order and direction |

- Index chosen: online build (`CREATE INDEX CONCURRENTLY`, `ALGORITHM=INPLACE,
  LOCK=NONE`), own migration file, write-path cost stated; drop the index it makes
  redundant. The mechanics: omm-migrate.
- Re-plan the same statement on the same data; paste before and after. Result set
  unchanged: row count, or `EXCEPT` both ways, on the test data. Tests for the
  touched path pasted; the repo's query-count assertion (`assertNumQueries`) where
  one exists. Inside the noise, or a test red: revert.

## 3. Transactions

- Open late, commit early. Nothing inside but statements: no HTTP call, no queue
  publish, no user wait. Every exit commits or rolls back: a `with` / `try`
  block, not a flag.
- Check-then-write on shared rows races under READ COMMITTED (the default nearly
  everywhere). Use `SELECT ... FOR UPDATE`, `INSERT ... ON CONFLICT`, a unique
  constraint with the conflict handled, or an atomic `UPDATE ... WHERE balance >=
  $1`. Never a read, an `if`, then a write.
- Retry only serialization failures and deadlocks (`40001`, `40P01`, MySQL
  `1213`): the whole transaction, bounded, idempotent.

## 4. Parameters only

- `$1`, `?`, `:name`, or the ORM builder. Never `+`, f-string, `format`, template
  literal, or `%` into SQL, including the raw escape hatches (`.raw()`,
  `$queryRaw`, `knex.raw`, `text()`).
- Identifiers (table, column, sort direction) cannot be parameters: map them from
  a fixed allowlist in code and quote with the driver's identifier quoting.
- Concatenated SQL found on the way: fix the lines you own, report the rest as
  `path:line` (omm-security).

## 5. Destructive statements

`DELETE`, `UPDATE` without a primary-key predicate, `DROP`, `TRUNCATE`, restore.
Against any database the user has not declared disposable (repeat it back):

1. Only when the user asked and named the environment. Name the database
   (`SELECT current_database()`, host) in the message.
2. `SELECT count(*)` with the same `WHERE`. Paste. Matches the expectation, or
   stop and show the rows.
3. Backup verified, not present: `pg_dump -Fc` / `mysqldump` / SQLite `.backup`,
   restored into a scratch database, a row count or checksum from the copy that
   matches the source, pasted. A dump nobody restored is not a backup.
4. `BEGIN`, run, compare affected rows to step 2, `COMMIT` only when it matches;
   else `ROLLBACK`. Batch by key over ~100k rows. MySQL: `SET autocommit = 0`.

## Judgment calls

- Another index or not: every index taxes every write on the table. Show the
  write rate before adding a fourth.
- ORM or raw SQL: ORM for CRUD; raw when the plan needs what the ORM hides
  (window function, CTE, `ON CONFLICT`), in the repository layer, parameterised,
  its plan still pasted.
- Low-cardinality column (`status`, 3 values): alone, rarely used; leading a
  composite whose second column carries the `ORDER BY`, it wins.
- Slow but not the query: pool exhausted, lock wait, a trigger, round trips.
  `pg_stat_activity` / `SHOW PROCESSLIST` first. Slow only in production:
  `pg_stat_statements`, `auto_explain`, the slow query log; none: the change is
  the instrumentation, not a local guess.
- Cache or denormalised column: invalidation in one sentence, or stop and ask.
- User names the fix ("add an index on x"): plan first anyway; if it says stale
  statistics or N+1, paste it and offer that instead.

## Refuse

- An index, rewrite, or planner hint without the before plan; a result without
  the after plan on the same data; an index kept when the after plan ignores it.
- SQL built from strings, including "just the table name".
- A transaction wrapping network calls or user interaction.
- A destructive statement without the count, the named database, and the
  verified backup; a constraint disabled to make a write succeed.

## Micro-example (PostgreSQL)

"The open orders page is slow." ORM log for one request: 1 query on `orders`,
then 50 `SELECT ... FROM customers WHERE id = $1`. `read_file
app/orders/views.py`: `order.customer.name` inside the template loop.

1. `\d+ orders`: 2.1M rows; indexes `orders_pkey`, `orders_customer_id_idx`.
2. `EXPLAIN (ANALYZE, BUFFERS) SELECT ... FROM orders WHERE status = 'open'
   ORDER BY created_at DESC LIMIT 50;`
   ```
   Limit (actual time=1843.2..1843.3 rows=50)
     -> Sort (actual time=1843.1..1843.2)  Sort Method: external merge  Disk: 41208kB
        -> Seq Scan on orders (actual rows=412000)
             Filter: (status = 'open')  Rows Removed by Filter: 1690000
   ```
   Hypothesis A: index `(status, created_at DESC)`; Sort and Seq Scan become an
   Index Scan that stops at 50 rows; under 5 ms. Hypothesis B: eager-load
   `customer`; 51 statements become 2. `write_todos`: A, B.
3. A: `CREATE INDEX CONCURRENTLY orders_status_created_idx ON orders (status,
   created_at DESC);` as its own migration via omm-migrate. Re-plan: `Index Scan
   using orders_status_created_idx (actual time=0.05..0.61 rows=50)`.
4. B: `edit_file` the queryset: `.select_related("customer")`. Log: 2 statements.
   `pytest tests/test_orders.py -q`: `18 passed`.
5. Report: both plans, 51 -> 2, write cost of one more index on 200 inserts/s.
