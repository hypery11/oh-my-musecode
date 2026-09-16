# Seams, pins, and fakes for legacy code

Companion to `omm-legacy`. Every recipe keeps the old body textually intact and
puts the new behaviour behind a point a test can reach.

## Seam catalogue (cheapest first)

| Seam | When | Shape |
|---|---|---|
| existing parameter | a flag or argument already reaches the site | pass a new value; no signature change |
| optional parameter | the site reads a global, a clock, a constant | `def f(x, now=None): now = now or datetime.now()` (Python); `f(x, now = Date.now)` default arg (TS); `fn f(x: T, clock: &dyn Clock)` with the old call site passing `&SystemClock` (Rust); functional option or second constructor (Go) |
| sprout function | new logic, one call from the old body | new function, own tests; old body gains one line |
| sprout class | new logic needs state or several helpers | new type, own tests; old body constructs and calls it |
| wrap function | behaviour before or after the old body | rename old to `x_legacy` unchanged; new `x` calls it |
| extract and override | a method uses a collaborator you cannot construct in a test | move the collaborator call into a protected method; subclass in the test and override |
| parameterise constructor | the class builds its own dependency | keep the old constructor, add one that accepts the dependency; the old one delegates with the old default |
| link or import seam | a module-level function is called directly (`requests.get`, `os.environ`, `fs.readFileSync`) | patch at import site in the test (`unittest.mock.patch("mod.requests.get")`, `vi.mock`, `jest.mock`); production code untouched |
| environment seam | behaviour keyed on env var or config file | set the variable or point the config path at a temp file in the test setup; restore after |

Rules: one seam per change. Never introduce a seam and a behaviour change in the
same `edit_file`. A seam that requires editing more than the signature line and the
one call site is a refactor; stop and use `omm-refactor` first.

## Pin recipes by output shape

- Scalar or small structure: one assertion per input, literal expected value copied
  from the pasted run.
- Large text (report, HTML, CSV, log): golden file per input under
  `tests/golden/<case>.<ext>`, written once from the captured run, compared with a
  plain equality or the framework's snapshot (`assert out == path.read_text()`,
  `expect(out).toMatchFileSnapshot()`, `insta::assert_snapshot!`). Regenerate only
  on purpose, in its own commit, with the diff shown.
- Side effects (rows written, files created, messages sent): fake the sink at the
  boundary and assert the calls and payloads; assert the payload content, not the
  call count alone.
- Exceptions and exit codes: `assertRaises` / `expect(...).toThrow` / `assert!(
  matches!(err, ...))` on the exact type and message the run produced.
- Ordering that varies between runs: sort before asserting and note it in the test
  name (`..._unordered`); a sorted pin still catches a lost or extra element.

## Fakes at the boundary, by kind

| Boundary | Fake |
|---|---|
| clock | fixed `datetime`/`Date`/`Instant` injected or patched at the import site |
| randomness | seed set in setup, or the generator injected |
| filesystem | temp dir (`tempfile.TemporaryDirectory`, `fs.mkdtempSync`, `tempfile::tempdir`) with the fixture files copied in |
| network | patch the client call; return the captured real response body saved as a fixture file |
| database | the repo's test database if one exists; else an in-memory engine only when the code already runs on it (SQLite for SQLAlchemy, `:memory:`); otherwise fake the repository/DAO layer, not the driver |
| environment | set and restore in setup/teardown (`monkeypatch.setenv`, `process.env` save/restore, `temp_env`) |
| time zone and locale | set `TZ` and `LC_ALL` explicitly for the run; legacy code often depends on the machine default |

## Reading the map faster

- `git log -L :<function>:<file>` shows the history of one function; the commit
  that introduced a quirk usually says why.
- `git blame -w -M -C <file>` ignores whitespace and follows moves; the oldest
  lines are the ones with the most silent dependants.
- A `search` for the error strings the site emits finds the callers that catch and
  branch on them; those callers define the contract more than any doc.
- Constants that look arbitrary (`* 1.1`, `- 3`, `[:8]`) are business rules until
  proven otherwise; pin them, question them in the note, never round them off.

## AGENTS.md note template

```text
## <path or area>
- Entry: <file:line> (<n> callers); also named in <config/registry>.
- Flow: <one line: in -> transform -> out>.
- Trap: <invariant not enforced | wrong-looking output that is depended on | cannot run because ...>.
- Pins: <exact command>.
```

Three to eight lines. Facts only; a future reader wants where, how to run, and
what not to touch, not how you felt about the code.
