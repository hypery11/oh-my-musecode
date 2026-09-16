---
name: "omm-perf"
description: "Use when code is slow or asked to optimise, cut latency, or reduce memory: metric, target, profiled baseline, ONE hypothesis, change, re-measure, keep or revert on the number; Do not use when correctness is still broken (omm-debug first)."
---

# Performance loop

Contract: no edit for speed before a baseline number exists. The change is done only
when the final message pastes baseline and after from the same harness, input, build
and machine, with run-to-run spread, verbatim from `bash` output. One hypothesis at a
time; every change is kept or reverted on the number alone. "Should be faster" is
not done.

## 0. Gate

- Tests red, or the reported bug not fixed yet: stop; that is omm-debug or omm-tdd.
  A fast wrong answer is a wrong answer.
- "Feels slow" with no number: step 1 makes the number. Already inside target: report
  that and stop. Do not optimise on a hunch.
- `write_todos`: metric, baseline, one item per hypothesis, report.

## 1. Metric and target

One metric, one unit, one workload, written down before any tool runs.

| Complaint | Metric | Source |
|---|---|---|
| "endpoint is slow" | p50 and p95 latency at a stated concurrency | `hey`, `wrk`, `k6`, or the app's own timing log |
| "script takes forever" | wall-clock and CPU seconds of the whole run | `/usr/bin/time -v` (Linux) or `-l` (macOS), `hyperfine` |
| "memory usage" | peak RSS, or heap after GC, or allocations per op | same `time`, language heap profiler |
| "build / suite is slow" | wall-clock per stage | the tool's own timing flags |

- Target: a number and where it comes from ("p95 under 200 ms, SLO"). None given:
  propose one from the cost and ask once; unanswered, use it and say so in the
  report. "Faster" is not a target.
- Workload: production-shaped size and distribution, the same bytes every run (a
  committed fixture, or generated with a fixed seed). A 10-row fixture hides the
  N-squared loop that a 50k-row export hits.

## 2. Baseline

- Build one command, HARNESS, that prints the metric. Run it with `bash`
  (`yield_time_ms` up to 300000). Discard a warm-up run unless cold start is the
  metric; then at least 5 runs. Report median and min..max, or p50/p95. Never one run,
  never the mean alone.
- Release or optimised build (`--release`, `-O2`, `NODE_ENV=production`), no coverage,
  no debugger, no debug logging. Quiet machine, the same one for before and after. A
  debug-build number is not a number.
- Noise floor: run HARNESS twice with nothing changed; the gap between the two medians
  is the floor. Floor wider than the gain you hope for: fix the harness first (more
  runs, larger input, pinned core). Nothing is measurable until then.
- Paste the baseline verbatim with `git rev-parse --short HEAD`. Keep the harness in a
  scratch file outside the repo unless the repo has a bench directory.

## 3. Profile, then ONE hypothesis

- Profile the real workload with a sampling profiler; per-language commands and traps
  in `references/profilers.md` beside this file. Tool missing: say so, install only
  with the project's own package manager if permitted, else instrument boundaries
  with timing logs. Never guess.
- Study the profile in this order: largest inclusive time in project code, then hottest
  self time, then call count. Memory: allocation count before size. Wall time far
  above CPU time means waiting (I/O, locks, round trips): count the calls at the
  boundary; a CPU profile will not show it.
- Hypothesis as a prediction with a location and a size: "`get_customer`
  (`app/import.py:41`) is 71% of samples, called once per row; loading all customers
  once drops median from 9.4 s to under 3 s." Predicted gain smaller than the floor:
  pick another. Nothing clears it: stop and report; do not start micro-optimising.
- Order of attack: do less work (quadratic loop, N+1 query, repeated work in a loop),
  then I/O (batch, fewer round trips, cache), then allocation, then micro (inlining,
  data layout). Never start at micro. A flat profile means the fix is algorithmic.
  Several candidates: `write_todos`, exactly one `in_progress`.

## 4. Change, re-measure, keep or revert

- Smallest `edit_file` that tests the hypothesis. Nothing else moves in the same step:
  no rename, no reformat, no second optimisation "while here". Anything else noticed
  goes to `write_todos`.
- Run the tests that cover the touched code; behaviour is unchanged or this is not a
  perf change. Then HARNESS, same flags, same runs. Paste both.
- Decide on the number. Gain at or beyond the prediction, clear of the floor: keep.
  Inside the floor, or a test went red: revert your edit (`edit_file` the original
  text back, or `git checkout -- <file>` for a file that was clean before you touched
  it) even when the new code reads better; the measured delta goes in the report. A
  refutation is a fact, not a failure. An unmeasured cleanup is omm-refactor's.
- Cache or memoisation: state the invalidation rule in the same change, or do not
  cache. Cannot write it in one sentence: stop and ask.
- Re-profile after every kept change. The hotspot moved; yesterday's list is stale.
- Stop when the target is met, when the next predicted gain is below the floor, or
  after three consecutive reverts: report what was ruled out and the remaining hotspot.
- Before reporting, measure the metric you traded against once (memory when you
  bought time, and the reverse).

## 5. Report

```text
Metric: <what, unit, workload>; target <n> (<source>)
HARNESS: `<command>`; <n> runs, warm-up <yes|no>; build <mode>; floor <n>
Baseline <sha>: median <x>, range <a..b>
After:          median <x>, range <a..b>   (<pct> faster / <n> MB less)
Kept: <path:line - one line per change, with its own delta>
Reverted: <change - measured delta - reason>
Profile now: <top item and share>
Tests: <suite summary line>; not measured: <production traffic, cold cache, ...>
```

A number you did not measure in this session does not appear.

## Judgment calls

- Micro-benchmark only for a pure function the profile already proved hot, with
  production-shaped input; a micro win that leaves HARNESS flat is not a win. Report
  HARNESS, never the micro number alone.
- Numbers that lie: an unused result the compiler deleted (consume it, `black_box`, a
  printed checksum); JIT or cache not warmed; tiny N; debug build; thermal throttling
  (re-run the baseline after the change; if it moved, both numbers are void); a
  tracing profiler inflating small functions (sample instead); a client clock that
  includes the network; a mean that hides the tail.
- The user names the fix ("add an index", "use a map"): baseline first anyway. Gains
  nothing: paste the number and offer the profile's actual hotspot.
- "It used to be fast": HARNESS with a threshold exit code is the reproduction;
  bisect with omm-debug's recipe. Slow only in production: get a production profile
  or timing log first; none exists: instrument the boundaries, do not guess locally.

## Refuse

- Any edit for speed before the baseline. Two optimisations in one measurement.
- "Obviously faster" or "now O(n)" as evidence; a published benchmark or a number
  from another machine as baseline or after.
- Keeping a change that measured inside the floor "because it should help".
- Optimising code the profile shows as cold, or while a test on the path is red.
- A deleted test, weakened assertion, or widened tolerance to let the fast path pass.
- A single run, a debug build, or a micro-benchmark as the result.

## Micro-example (Python)

"The CSV import is slow." Metric: wall-clock of `python -m app.import
fixtures/orders-50k.csv`; target under 2 s (nightly budget, user). HARNESS
`hyperfine -w 1 -r 5 '<cmd>'` at `a1b2c3d`: `9.412 s +- 0.084 s`; unchanged
re-run 9.39 s, floor 0.02 s. `py-spy record -o prof.svg -- <cmd>`: 71% of samples
in `Customer.objects.get` under `import_row`, 50k calls. Hypothesis: N+1 query;
one bulk load drops the median below 3 s. `edit_file app/import.py`: a dict of
customers before the loop. `pytest tests/test_import.py -q`: `12 passed`. HARNESS:
`2.731 s +- 0.041 s`. Keep. Re-profile: 58% in `Decimal(row["amount"])`; `float`
gives `1.902 s` but `test_import_rounding` fails: revert, correctness is not for
sale. Report: baseline 9.41 s, after 2.73 s, one kept, one reverted with its
number, target not met, remaining hotspot named.
