---
name: "omm-errors"
description: "Error handling, exceptions, crashes silently, unwrap everywhere, or error-path design: classify expected/bug/external, fail fast at boundaries, never swallow, wrap with context, log once; Do not use when a whole-diff review is wanted."
---

# Error handling

Two asks route here. Review ("audit the exceptions", "why does this fail silently"):
read-only, findings in the section 5 format. Fix or design ("add error handling",
"unwrap everywhere"): every edit behind a test of the caller-visible outcome, runs
pasted. Both classify every error site before judging any: a site is wrong only
relative to the kind of error it handles. An unexplained crash is omm-debug; the
whole diff is omm-review.

## 0. Find the surface

- Target: a diff (`bash` `git diff HEAD`, or `<base>...HEAD`), a path, or a module
  named in prose.
- `search` (regex mode, `glob`-scoped) for the language's idioms:
  `catch|except|rescue`, `unwrap\(|expect\(|\.ok\(\)|try!|as!`, `if err != nil`,
  `panic|raise|throw`, `exit\(|Fatal`, `log(ger)?\.(error|warn|exception)`. Per-language
  shapes, swallow patterns, HTTP mapping: `references/idioms.md` beside this file.
- `read_file` the boundaries first: `main`, argument parsing, request handlers, queue
  consumers, config loading; each is a site even with no hit. Then one call chain,
  boundary to leaf; error handling is judged along a chain, not per line.
- Over 3 files: `write_todos`, one item per boundary chain.

## 1. Classify every site

Ask: who can act on this failure? Decide from the origin, not from how the code
treats it today.

| Kind | Who acts | Examples | Correct response |
|---|---|---|---|
| Expected | the caller or the user | not found, invalid input, conflict, permission denied, empty result | a typed error or variant the caller branches on; a message in user words; no stack trace; a test per variant |
| Bug | the developer | null where a value was guaranteed, unreachable arm, index out of range, API misuse | fail fast at the site (assert, panic, unchecked throw); caught only by the top boundary, never retried; fix the code, not the handler |
| External | the operator, or a retry | network, disk full, timeout, dependency down, malformed upstream response | wrap with context; bounded retry if idempotent, else surface; observable |

Misclassification is the root finding; the rest follows. A catch-all returning a
default treats Bug as Expected (hides it). `unwrap()` on parsed user input treats
Expected as Bug (crashes on a normal path). A retry around a parse error treats
Expected as External. The same exception differs by site: file-not-found on a
user-supplied path is expected; on a resource shipped in the package it is a bug.
Classify at the origin; a swallowed expected error resurfaces as a bug far away.

## 2. Rules per site

- Fail fast at the boundary. Validate where data enters (args, request body, config,
  env, message payload), convert once to a typed expected error, and let interior code
  trust its inputs. An interior check the boundary lacks moves up; a duplicate goes.
- Never swallow. `catch {}`, `except: pass`, `_ = f()`, `.ok()` dropped,
  `if err != nil { return nil }`, `rescue nil`, `|| true`, `2>/dev/null`. Each is a
  finding unless a comment on the line names the ignored case and a test covers the
  path. Null or a default on failure collapses "not found" with "failed"; the caller
  cannot tell.
- Wrap with context, not noise. Each layer adds only what it alone knows (path, id,
  operation) and keeps the cause chained (`raise X from e`, `%w`, `{ cause }`,
  `.context()`). Same message rethrown, cause dropped, or the inner message restated
  is noise.
- Log once, at the top. The boundary that decides the outcome (main, handler, worker
  loop) logs the full chain exactly once: bug, error level with stack; external, warn
  with the operation and the retry decision; expected, no error log. Interior code
  never logs an error it also returns; catch-log-rethrow makes N lines for one failure.
- User-facing message: what happened, in the user's words, and what to do next. No
  stack trace, internal name, SQL, path or secret; a correlation id if there is one.
  CLI: non-zero exit on any failure, message on stderr.
- Cleanup on every path: `finally`, `defer`, `with`, RAII; partial writes atomic (temp
  file + rename) or rolled back. A return that skips unlock, close, or rollback is a
  Blocker, however rare.
- Partial failure in a loop: all-or-nothing, or continue and report the failed items;
  decided explicitly, visible in the return. Item 3 fails, call returns success: a
  finding.

## 3. Judgment calls

- `unwrap`, `expect`, `!`, `as!`, an unchecked lookup: fine when the invariant is local
  and stated (`expect("invariant: <why it holds>")`), in tests, and in `main` once the
  error is reported. On anything from input, I/O, or another module: `?` or a typed
  error instead.
- Retry: external kind only, idempotent operation only, bounded attempts, backoff with
  jitter, a deadline, the last error propagated with the attempt count. Never on a bug
  (it repeats) or an expected error (a 4xx will not change).
- A catch-all: exactly one per boundary; it logs once, translates to a response or exit
  code, keeps serving or exits. Anywhere else it is a swallow unless it re-raises what
  it does not recognise.
- A library returns errors and never exits the process; `sys.exit`, `os.Exit`,
  `log.Fatal`, `process.exit` in library code is a finding.
- How many error types: one variant per distinct caller action, not one per site.
  Callers only ask "did it fail?": one type. They branch (retry, report, ignore):
  variants. `search` the callers before adding one.
- Exceptions or result values: the codebase's convention; a second style is a finding,
  not a fix. Assertions in production: keep them; deleting one moves the crash.

## 4. Fix mode

Per todo: a test that fails today and asserts the caller-visible outcome (returned
variant, status code, exit code, stderr text), never the log line; `bash`, paste red;
smallest `edit_file`; paste green and the suite summary. External kind: fake the
boundary to fail; never hit the network. "Crashes silently": make each catch-all and
ignored return rethrow first; the crash becomes visible; now classify.

## 5. Report (review mode)

```text
## Error paths
1. [Blocker] src/x.py:123 - <kind> handled as <kind>: <defect in one sentence>
   Scenario: <input or state> -> <what the user or operator sees>
   Fix: <one line>
## Verdict
<Merge | Merge after fixing #N | Do not merge> - <one sentence>. Tests: <ran | not run>.
```

Blocker: swallowed or collapsed errors, skipped cleanup, data loss on the error path.
Major: wrong kind, cause dropped, unbounded retry, secret in a message. Minor:
duplicate logs, context-free wraps, message quality. Every item has `path:line` and a
Scenario; one finding per root cause. Zero findings is a valid result.

## Refuse

- A try/catch at the crash site to stop a symptom; classify first, or it is omm-debug.
- A default returned from a catch to make a test pass; branching on an error message
  string; logging and rethrowing at the same site.
- "Handle errors properly" as a finding; name site, kind, scenario.

## Micro-example (Python)

"The CLI crashes with a KeyError when the config path is wrong." `load_config(path)`
wraps `open` and `json.load` in `except Exception: return {}`; `main` then reads
`cfg["db_url"]`. Classify: missing file and malformed JSON are expected; the
`KeyError` is the swallow resurfacing as a bug two calls later. Shape: one
`ConfigError` (callers only ask "did it load?"), two branches because the user's
next action differs, cause chained, no log here.

```python
def load_config(path):
    try:
        with open(path) as f:
            return json.load(f)
    except OSError as e:
        raise ConfigError(f"cannot read {path}: {e.strerror}") from e
    except json.JSONDecodeError as e:
        raise ConfigError(f"{path}:{e.lineno}: invalid JSON: {e.msg}") from e
```

`main` is the one boundary: `except ConfigError as e:` prints `error: {e}. Check the
path or run 'app init'.` to stderr, returns 2; a last `except Exception:` calls
`log.exception("internal error")`, prints `internal error, see log`, returns 1: a bug,
logged once with the stack. Tests assert message and exit code, never the log line.
`PermissionError` is not split out: same user action as not-found.
