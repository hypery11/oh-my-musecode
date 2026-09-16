---
name: "omm-tdd"
description: "Use when asked to implement, add a feature, or fix a bug in a repo that has a test framework - failing test first, shown red, minimum code, green run pasted, refactor, re-run; Do not use when only docs or config change."
---

# TDD loop

Contract: the change is done only when your final message pastes the RED run,
the GREEN run, and the suite summary, verbatim from `bash` output. "Tests
should pass" is not done. Never write the test and the implementation in the
same step; you must see red first.

## 0. Before the first test

- Find the runner with `search`: `package.json` (scripts.test), `pytest.ini`,
  `pyproject.toml`, `Cargo.toml`, `go.mod`, `*.csproj`, `Package.swift`,
  `build.gradle*`, `pom.xml`, `mix.exs`. Commands per framework (detect, one
  test, one file, package): `references/runners.md` beside this file.
- No third-party framework: use the language's built-in runner if one exists
  (`unittest`, `node --test`, `cargo test`, `go test`, `swift test`). None:
  stop and ask which to add; never pick one silently.
- Baseline: run the relevant package suite once with `bash`. Record
  pre-existing failures by name; do not fix them unless asked.
- One-shot commands only, never watch mode. Slow suite: raise `yield_time_ms`
  (up to 300000) so the run finishes in one call; do not poll with
  `bash_input`, the output arrives on its own. Do not commit.
- Bug report: the failing test IS the reproduction. Draft it before reading
  the implementation in depth.
- `write_todos`: one item per behaviour, sub-steps red / green / refactor.
  Close an item only after its refactor run is green.

## 1. Pick the test level

Lowest level that fails today for the right reason:

| Change | Level |
|---|---|
| pure function, parser, calculation | unit, no I/O |
| crosses a boundary (db, http, fs, clock) | integration if the suite has one; else a fake at the boundary |
| bug | lowest level where it reproduces; add a unit test once localized |
| CLI or UI | observable output: exit code, stdout, rendered text |
| legacy code, no seam | characterization test of current behaviour, refactor a seam, then the real test |
| user-visible flow no lower level can express | end-to-end, and only then |

Never mock the unit under test. Fake the boundary, assert the result.

## 2. Add ONE failing test

- `read_file` the nearest existing test file; copy its fixtures, imports and
  naming. Add the test there or in a sibling file. Do not invent a layout.
- Name the behaviour, not the method: `test_<unit>_<condition>_<expected>` or
  `it("<verb phrase> when <condition>")`. `test_rejects_expired_token`, not
  `test_validate`. If the name needs "and", split it.
- One behaviour: one arrange, one act, one logical assertion. Several asserts
  on the same result are fine; two acts are two tests.
- Assert the observable outcome: return value, raised error, written file,
  response body. Not internals, not "was called with".
- `write_file` for a new file, `edit_file` to add to an existing one.
- Do not batch. One test, red, green, next. Ten tests before any code is
  test-after in disguise.

## 3. RED: run it and show it failing

- Run only the new test with `bash`. Paste the failing lines verbatim.
- Check the reason. It must fail on the missing behaviour, not on an import,
  typo, fixture or compile error. Wrong reason: add the smallest stub, re-run,
  until it fails on the assertion.
- Passes immediately: the behaviour already exists (stop, say so) or the
  assertion is empty (tighten it). Never proceed on a test you did not see red.

## 4. GREEN: implement the minimum

- Smallest `edit_file` that makes this one test pass. Hard-code only when the
  general solution is unclear; the next test forces the generalization.
- Touch nothing unrelated. No drive-by features, renames or reformatting.
  Anything else you notice goes into `write_todos`, not into this change.
- Re-run the single test: paste green. Then the package suite: paste the
  summary line. Anything newly red: fix it before continuing.
- Still red after two attempts: re-read the failure, not the code, and say
  what you learned before the third.

## 5. REFACTOR: only on green

- Rename, extract, dedupe, delete dead branches. Behaviour and tests unchanged.
- Re-run the suite after each refactor step. Red after refactor: revert the
  refactor, never the test.
- Nothing to refactor is a valid outcome. Say so.

## 6. Loop or stop

Next todo: back to step 2. Done: every todo green, relevant suite green, runs
pasted. Final message lists the test file and name, the RED excerpt (the
assertion line), the GREEN line, the suite summary, what was refactored, and
pre-existing failures left untouched.

## Judgment calls

- Slow suite: targeted subset inside the loop, full suite once at the end;
  say which is which.
- Flaky test: run it three times. If it flips, name it as flaky and keep
  going; never delete an assertion to make it stable.
- Time, randomness, network: inject a clock, seed or fake at the boundary.
  Never sleep in a test.
- Changing an existing expectation: only when the requirement changed. State
  old and new in the message.
- Suite red at baseline: run your tests in isolation, report the pre-existing
  failures by name, never claim the suite green.
- User says no tests: offer the one-test version once (name and cost). If
  declined, comply and say in the final message that the change is untested.

## Refuse

- Implementation first, tests after.
- `skip`, `xfail`, `.only`, `@Ignore` or a commented-out assertion to get green.
- Loosening or deleting an assertion because it is inconvenient. If it is
  wrong, say so and ask.
- Snapshot-everything tests with no stated intent.
- Asserting mock call counts instead of results.
- "All tests pass" without the pasted output. Never describe a run you did
  not execute.

## Micro-example (pytest)

Goal: `slugify` turns a title into a URL slug. First behaviour: spaces become
hyphens.

1. `read_file tests/test_text.py` for style. `edit_file` to append:
   ```python
   from app.text import slugify

   def test_slugify_replaces_spaces_with_hyphens():
       assert slugify("hello world") == "hello-world"
   ```
2. `bash`: `pytest tests/test_text.py -k slugify -q`
   ```
   E   ImportError: cannot import name 'slugify' from 'app.text'
   ```
   Wrong reason (wiring). Stub `def slugify(s): return s`, re-run:
   ```
   E   AssertionError: assert 'hello world' == 'hello-world'
   ```
   Correct RED: it fails on behaviour.
3. `edit_file app/text.py`, replace the stub:
   ```python
   def slugify(s):
       return s.replace(" ", "-")
   ```
4. Re-run: `1 passed in 0.01s`. Suite `pytest tests -q`: `41 passed`.
5. Refactor: nothing. Next todo (`"Hello World!"` gives `"hello-world"`,
   lowercase and punctuation): back to step 2.
