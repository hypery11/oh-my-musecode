---
name: omm-test-triage
description: Triage a red test run (tests failed, failing test output, CI is red) into the first real failure, its minimal rerun command and a cause hypothesis; Do not use for an intermittent failure (omm-flaky-test) or once the cause is known and only the fix is wanted.
metadata:
  triggers: test failure, tests failed, failing test, failing tests, red tests, ci failed, ci is red, test output, which test failed, why did the tests fail
---

# Test failure triage

Goal: turn a wall of test output into one ranked list of distinct failures,
each with the exact command that reproduces it alone, before anyone edits code.
Output: the table below and the first rerun command's real result.

## 1. Capture, do not skim

- If the output is in context, read all of it once. If it is a CI link or a
  file, `read_file` it in slices; if the run must be repeated, run it with
  `bash` and keep the output bounded: `2>&1 | tail -n 200`, never the whole log
  into context.
- Find the FIRST failing assertion or panic in run order. Later failures are
  usually consequences (a shared fixture, a poisoned state, a missing file).

## 2. Group

One row per distinct failure signature (same assertion text or same panic
location), not per test name:

| # | signature | tests | first seen at | kind |
|---|---|---|---|---|

`kind` is one of: assertion (expected vs actual), panic/crash, timeout,
compile/link, environment (missing binary, port, env var, network), harness
(the runner itself). Environment and harness rows come first in the fix
order; they invalidate every other row.

## 3. Minimal rerun, per row

The narrowest command the framework offers for exactly those tests:
`cargo test -p <crate> <name> -- --exact --nocapture`, `pytest path::test -x`,
`npm test -- -t "<name>"`, `go test ./pkg -run '^TestName$' -v`. Run the first
row's command now with `bash` and paste its real result; a triage that never
re-ran anything is a guess.

## 4. Hypothesis, one per row

State the single most likely cause and the observation that would refute it.
Distinguish: the test is wrong (expectation drifted), the code is wrong, the
environment is wrong. Do not fix yet; hand the ranked table to the user, or,
if they asked for the fix, take row 1 only and prove it green with the rerun
command before touching row 2.
