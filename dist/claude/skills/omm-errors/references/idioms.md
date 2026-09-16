# Error idioms by language

Loaded on demand from `omm-errors`; the decisions are in SKILL.md. The "swallow patterns"
column is a `search` list (regex mode) for finding sites; the other columns say what the
correct shape looks like in that language's own convention. Follow the codebase's existing
style where it already has one.

| Language | Expected-error shape | Wrap and keep the cause | Cleanup on every path | Swallow patterns to hunt | Fail fast and exit |
|---|---|---|---|---|---|
| Python | one module hierarchy (`class AppError(Exception)`, a subclass per distinct caller action); `None` return only when "absent" is not a failure | `raise New(...) from e`; a bare `raise New(...)` inside `except` keeps `__context__` but marks it implicit | `with`, `try/finally`, `contextlib.ExitStack` | `except:` or `except Exception:` with no re-raise, `except .*: *pass`, `except .*: *return (None|\{\}|\[\])`, `contextlib.suppress(Exception)`, `except .*: *log`, then fall through | `assert` (stripped by `-O`, so not for input checks); `sys.exit` only in `main`; `log.exception` once at the top |
| Go | sentinel `var ErrX = errors.New(...)` or a typed error; callers use `errors.Is` / `errors.As`; never `nil, nil` | `fmt.Errorf("op %s: %w", x, err)` | `defer`; check `Close()` on writers | `_ = f()`, `if err != nil { return nil }`, `if err != nil {}`, `log.Printf(...)` then `return nil`, `recover()` outside a top-level handler | `panic` for invariants only; `log.Fatal` / `os.Exit` only in `main`; `recover` only in one top-level handler that logs and returns 500 or re-panics |
| Rust | `Result<T, E>` with an enum `E` (`thiserror`) in libraries; `anyhow::Result` at the binary top only | `.with_context(...)` (anyhow) or `#[source]` / `#[from]` fields (thiserror); `?` | `Drop`, scope guards | `.unwrap()`, `.expect()`, `.ok()` dropped, `let _ = f()` on a `Result`, `.unwrap_or_default()` on a fallible op, `if let Ok(x)` with no else arm | `panic!`, `unreachable!`, `debug_assert!` for invariants; `std::process::exit` only in `main` |
| TypeScript / JS | subclass `Error` with a `code`; or a result union (`ok: true` with `value`, `ok: false` with `error`) if the codebase already uses one | `new Error(msg, { cause: e })`; `AggregateError` for fan-out | `try/finally`, `using` | `catch {}`, `catch (e) { return null }`, `.catch(() => {})`, an unawaited promise, `process.on('unhandledRejection')` that only logs | `throw` on invariants; `process.exit` only in the CLI entry; one error middleware last in the chain |
| Java / Kotlin | checked exceptions or a sealed result type per the codebase; a base exception per module, two levels deep at most | `new X(msg, cause)`; never `e.printStackTrace()` then continue | `try-with-resources`, `use {}` | `catch (Exception e) {}`, `catch (Throwable`, `catch (Exception e) { log... }` with no rethrow, `Optional` returned on failure | `IllegalStateException` / `IllegalArgumentException` for invariants; `System.exit` only in `main` |
| C# | a base exception per module; `TryX(out ...)` for expected lookups | `throw new X(msg, e)`; `throw;` to rethrow (`throw e;` resets the stack) | `using`, `finally` | `catch {}`, `catch (Exception) { return null; }`, `async void` methods (exceptions vanish), `.Result` / `.Wait()` wrapping in `AggregateException` | `Debug.Assert` for invariants; `Environment.Exit` only at the entry point |
| Swift | `enum AppError: Error` with associated values; `throws` in signatures | throw a new case that carries the inner error | `defer` | `try?` on anything but a truly optional lookup, `try!`, `!` force unwrap on a derived value, `as!`, `catch {}` | `precondition` / `fatalError` for invariants; `exit()` only in `main` |
| Ruby | a module hierarchy (`class Error < StandardError`) | `raise New, msg` inside `rescue` (Ruby sets `cause` automatically) | `ensure`, block form of `File.open` | `rescue => e` with no re-raise, `rescue nil`, `rescue Exception` (catches interrupts), `rescue StandardError; end` | `raise` for invariants; `exit` / `abort` only in the executable |
| Shell | non-zero exit and a message on stderr | `die() { echo "$0: $*" >&2; exit 1; }` naming the failing command | `trap cleanup EXIT` | `cmd \|\| true`, `2>/dev/null`, no `set -euo pipefail`, `$(cmd)` unchecked, `cd dir` without `\|\| exit` | `set -e` plus explicit checks around pipelines; distinct exit codes only if a caller branches |
| C / C++ | return code plus `errno`, or `std::expected` / `std::optional` for expected outcomes; exceptions only if the codebase already uses them | `std::throw_with_nested`, or an error struct with a cause field | RAII; `goto cleanup` in C | ignored return value (`(void)f()` with no comment; build with `-Wunused-result`), `catch (...)` with no rethrow, `errno` read after another call | `assert` (off under `NDEBUG`, so not for input); `abort()` for invariants; `exit()` only in `main` |

## User-facing message checklist

- What failed, in the user's vocabulary ("config file", not `ConfigLoader`).
- Which thing: the path, the id, the name, quoted.
- What to do next: the command, the flag, the line to fix.
- Not: a stack trace, an internal type name, a secret or token, or a message that
  blames the user for an internal fault ("invalid state" when the bug is yours).
- Same failure, same message every time: users and tests grep for it.

## HTTP boundary mapping

| Kind | Status | Body | Log |
|---|---|---|---|
| expected: invalid input | 400 or 422 | field and reason | none (a metric at most) |
| expected: not found / forbidden | 404 / 403 | resource named; return 404 for both when existence must not leak | none |
| expected: conflict | 409 | what conflicted | none |
| external: dependency down, timeout | 502 / 503 / 504 | generic text, `Retry-After` when known, correlation id | warn or error once, with the operation |
| bug | 500 | generic text, correlation id | error once, with the stack |

Never `200` with an error body unless the API's existing contract does so everywhere.

## Retry policy in one line

External kind, idempotent operation, N attempts (3 is the usual default), exponential
backoff with jitter, a total deadline shorter than the caller's timeout, one log line on
the final failure only. Anything else: no retry.

## Partial failure in a batch

Decide before writing the loop: all-or-nothing (transaction or rollback, first error
aborts, nothing reported as done) or best-effort (every item attempted, the result
carries the failed items with their errors, exit code non-zero when any failed). Name
the choice in the function's doc or return type. Silent skip is neither.
