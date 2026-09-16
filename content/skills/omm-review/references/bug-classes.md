# Bug classes by domain - hunt list for omm-review

Each line: what to look for -> the scenario shape a finding must name. Report only
what you can prove on the code in front of you; a matching class is not a finding.

## Boundaries and error paths
- Empty collection, zero, negative, max-size, single element -> index or divide fault.
- Null/None/undefined flowing where the old code guaranteed a value -> crash or
  silent default.
- Off-by-one on ranges: inclusive vs exclusive end, `<` vs `<=`, `len` vs `len-1`.
- Early return that skips cleanup, unlock, or a counter update -> leak or drift.
- Error swallowed (`catch {}`, `_ = f()`, `.ok()`) where the caller checks success.
- "Not found" and "failed" collapsed into one return -> caller retries or reports wrongly.
- Partial failure in a loop: item 3 of 5 fails; are items 1-2 rolled back or reported?
- Unicode and encoding: byte length vs char length, normalization, case-folding.

## Callers and contracts
- Signature change: every call site updated, including reflection, string-named
  dispatch, generated code, and tests.
- Return-value meaning changed (empty vs error, 0 vs None) -> caller branches on
  the old meaning.
- New precondition (non-empty, sorted, non-null) -> caller that does not satisfy it.
- Default parameter changed -> callers relying on the old default.
- Public API, CLI flag, env var, or config key renamed without alias -> silent
  breakage for existing users.
- Config key or feature-flag name written here differs from the one read elsewhere.

## Ordering, concurrency, lifetime
- Check-then-act on shared state without a lock or atomic -> race window.
- Lock order differs between two paths -> deadlock.
- Await/yield inside a critical section -> state observed mid-update.
- Callback or event fires before the listener is registered -> missed event.
- Object used after close/drop/free; handle escaping its owner's scope.
- Retry without idempotency -> duplicate side effect (payment, email, insert).
- Timeout or cancellation not propagated -> orphaned work.
- Init-before-use or flush-before-read order assumed but not enforced.

## Persistence and wire compatibility
- Migration: reversible? Runs on a non-empty table? Locks a hot table?
- Serialized shape changed (field renamed, type widened) -> old readers or
  stored rows fail to parse.
- Enum variant added -> exhaustive match elsewhere; removed -> stored value
  unreadable.
- Default value changed on a persisted field -> old rows now mean something else.
- Cache key does not include the input that changed -> stale hits.
- Clock, timezone, DST: naive datetimes compared with aware ones.

## Trust boundaries
- Input from request, file, env, or argv used in a path, shell, SQL, HTML, URL, or
  regex without escaping -> injection.
- Authorization check moved, made conditional, or done after the side effect.
- Secret, token, or personal data in a log line, error message, URL, or response.
- Path from input joined without a containment check -> traversal.
- Comparison of secrets with `==` instead of constant-time.
- Deserialization of untrusted data into rich types.

## Resource and cost
- Unbounded growth: list, map, or queue appended without eviction.
- N+1 query or per-item network call introduced in a loop.
- Recursion on user-sized input without a depth bound.
- File or socket opened on a path that can throw before close.

## Tests
- Test asserts the current behavior rather than the intended one.
- Mock replaces the exact code path the change touched -> test cannot fail.
- Test data avoids the boundary the change added (empty, max, absent id).
- Only the happy path is exercised; none of the error paths above has a test.
- Skipped, `only`, or commented-out tests introduced by the diff.
