---
name: omm-flaky-test
description: Investigate a test that fails intermittently (flaky, sometimes fails, passes on retry, only in CI) by reproducing the flake rate, then isolating shared state, timing or order dependence; Do not use for a test that fails every run (omm-test-triage).
metadata:
  triggers: flaky, flaky test, intermittent, sometimes fails, passes on retry, passes when rerun, nondeterministic, only fails in ci, only fails on ci, race condition in the test, fails randomly
---

# Flaky test

Goal: a flake with a measured rate and a proven cause, or an honest
"could not reproduce in N runs" with the exact loop used. A flaky test is
never "fixed" by a retry annotation without the cause written down.

## 1. Measure before theorising

Loop the single test in the narrowest command the framework offers and count
failures, with `bash`:

```
for i in $(seq 1 30); do <rerun cmd> >/dev/null 2>&1 || echo "fail $i"; done | wc -l
```

Then the same loop with the whole suite or the same package, if the single
test never fails alone: that difference is the first clue (order or shared
state). Record: runs, failures, alone vs in suite, local vs CI.

## 2. Classify from the evidence

| symptom | likely cause | how to confirm |
|---|---|---|
| fails only in suite | shared state, test order | run with a fixed seed / `--test-threads=1` / `-p no:randomly`; find the writer with `search` for the shared path, global, env var, port |
| fails under load or only in CI | timing: sleeps, timeouts, wall-clock assertions | grep the test for `sleep`, `timeout`, `now()`; run under `nice -n 19` or with CPU throttled |
| different output each run | unordered collections, map iteration, random data without a seed | print the failing value; look for `HashMap`/`set`/`Math.random` |
| fails on first run after checkout | missing fixture, cache, network | run in a fresh temp dir |
| passes with `--nocapture`/verbose | output buffering, stdin/tty assumptions | compare both modes |

## 3. Prove the cause

Make the flake deterministic first: force the order, the seed, the delay
(inject a `sleep` where the race is suspected) so it fails 30/30. Only then
fix it at the source: remove the shared state, await the real condition
instead of sleeping, seed the randomness, isolate the resource (temp dir,
free port).

## 4. Prove the fix

The same loop, 30 runs minimum, in the configuration that used to fail,
pasted with its count. Report: rate before, cause, fix, rate after. If the
cause is in the code under test (a real race), say so - that is a bug, not a
flaky test, and it needs its own regression test.
