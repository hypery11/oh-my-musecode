---
name: "omm-debug"
description: "Debug error output, a failing test, a crash, or why-does-this-happen: reproduce, refute ONE hypothesis (logs, narrowed repro, bisect), then fix behind a regression test; Do not use when the root cause is known and only the edit is wanted."
metadata:
  short-description: "Reproduce, one hypothesis, refute, then fix"
---

# Debug

Find the cause, prove it, then make the smallest change a new test locks in. No edit
outside the test suite before step 5. Hold one hypothesis at a time. Every step ends
with a fact you did not have before.

Three or more steps left? Record this checklist with `write_todos`; keep exactly one
item `in_progress`.

## 1. Reproduce

- Get the exact failing command and its full output. Run it yourself with `bash`
  (`yield_time_ms` up to 300000 for a slow build or suite). Cannot reproduce? Stop and
  ask for the command, input, and environment. An unreproduced bug gets a report of
  what you tried, not an edit.
- Save the shortest reliable repro as one command, REPRO, that exits non-zero on
  failure (the test runner's exit code, or `... 2>&1 | grep -q '<error>'`). You will
  run it many times.
- Flaky? `for i in $(seq 10); do REPRO || echo FAIL $i; done`. Fewer than 10 failures
  means ordering, timing, or shared state: "which run fails and what differs" is the
  first question.

## 2. Narrow

- Take the failure literally. The FIRST error, not the last. The first frame in project
  code, not in a dependency.
- `search` (mode `literal`) for the message and the symbols in the trace; `read_file`
  the frame that threw and its caller. Never reason about code you have not opened.
- Shrink REPRO: one test (`-k`, `--filter`, `--test`), one input, one code path. Spend
  up to a quarter of the effort here; a smaller repro makes every later step cheaper.
- Worked before? Find the breaking commit with the bisect recipe below.
- "Works here" is itself a hypothesis: diff versions, env vars, locale, TZ, cwd, file
  order between the two environments before dismissing it.

## 3. One hypothesis

- State it as a prediction: "If X, then <observable> will show Y." Pick in this order:
  what changed most recently; what the error names literally; the simplest story that
  covers every observation. Several candidates? List them with `write_todos`; one active.
- Name the cheapest observation that could prove it WRONG, then get it: a log line at a
  boundary, an assert, the value printed at the frame, REPRO with the suspect input
  removed. A probe that can only agree with you is noise.
- Refuted: keep the fact, drop the hypothesis, return to step 2. Survives: step 4.
- Second refutation: stop guessing; bisect or narrow mechanically. Third: stop, report
  what is known and what was ruled out, and ask.

## 4. Regression test before the fix

- Add the smallest test that fails for this bug, in the existing suite, following its
  conventions (`edit_file` an existing file or `write_file` a sibling). Name it for the
  behaviour (`total_after_summing`), not the bug (`issue_412`). No suite? Save REPRO as
  a script and treat it as the test.
- Run it. It must FAIL for the predicted reason: the same assertion or exception as
  REPRO. A typo, an import error, or a missing fixture is not red. Passes before the
  fix? The test misses the bug or the hypothesis is wrong; settle that before step 5.

## 5. Fix

- One minimal `edit_file` at the site the evidence implicates. Not at the crash site,
  not a catch-all, not two changes at once. A `?.`, null check, try/catch, or retry where
  the bad value was noticed is a symptom patch; the value was made somewhere else.
- Run: the new test (green), REPRO (green), the surrounding suite (no new failures).
- Remove every log line, assert, and print added in step 3.
- Stop and ask before a fix that changes behaviour the user did not mention.

## 6. Report

Symptom, root cause in one line, the evidence that survived, the change, the test, and
anything else the same cause could affect. An environment trap others will hit?
`add_memory` one line.

## Bisect recipe (bash)

Needs REPRO that exits 0 when good and non-zero when bad, with no prompts, and that
depends only on committed files. Run in a throwaway worktree so the user's checkout
never moves. One `bash` call: `cd` does not persist between calls.

```sh
T=$(mktemp -d) && git worktree add -q "$T" HEAD && cd "$T" || exit 1
GOOD=<sha|tag>      # or $(git rev-list -n1 --before="14 days ago" HEAD); REPRO must PASS there
git bisect start HEAD "$GOOD"
git bisect run sh -c '<build> || exit 125; <REPRO>'   # 0 good, 125 skip, 1..127 bad
git bisect log | tail -3                              # the "first bad commit" line
git bisect reset; cd - >/dev/null; git worktree remove --force "$T"
```

Then `git show <first-bad>` and treat the diff as evidence, not the answer: it says
where behaviour changed; the fix may belong elsewhere. A flaky REPRO makes bisect lie;
fix the flake first. Slow? One `bash` call with `yield_time_ms` 300000, or, when
`subagent_spawn` is available, hand the script to a child with `worktree_isolation` and
collect it with `subagent_wait` then `subagent_read_result`. Nothing in git history (a
dependency version, a config value, an input size)? Halve the interval by hand, run
REPRO, keep the failing half. Run scripts, skips, merges, dependency and input
bisection, test-order leaks, log walk-back: `references/bisect.md`.

## Stop signals

Editing before reproducing. Two changes at once "to see". Wrapping the symptom.
Probes that can only agree with you. Rerunning a flaky test until it passes. Blaming a
dependency before reading the project frame. Skipping the test because the fix is one
line. "Try this and see."
