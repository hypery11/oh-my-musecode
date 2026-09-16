---
name: omm-ci-fix
description: "CI is red, pipeline failed, passes locally but fails in CI: read the job log, reproduce with CI's exact command and env, classify real/flaky/infra/drift, fix the class, re-run; Do not use when the failing test output is already in hand."
---

# CI failure diagnosis

Contract: the job is fixed only when the same job, on the fix, is green and
your final message pastes its conclusion line with the run id. "Should pass
now" is not done. Study the log before you touch anything; classify before
you re-run anything. Sorting a red run, measuring a flake and finding a real
cause (omm-debug) are separate jobs; this skill decides which you are in and
proves the job green afterwards. Run every command with `bash`.

## 0. Study the failing job's log, not the summary

- Locate the job: `gh run list --branch <branch> --limit 5`, then
  `gh run view <run-id> --log-failed 2>&1 | tail -n 300`. GitLab: `glab ci
  trace <job>`. Other systems, or no `gh`: `web_fetch` the raw log URL when
  that tool is present; otherwise ask for the failing step's tail and stop.
  Never guess from a job name. Per-system commands: `references/systems.md`
  beside this file.
- Bound it. A long log goes to a scratch file outside the repo; `read_file`
  it in slices (`offset`/`limit`) around the hits of `grep -n -m 20 -Ei
  'error|fail|panic'`; never the whole log into context.
- Start at the FIRST error in run order, not the red summary at the bottom.
  Later errors are consequences: a poisoned fixture, an artifact a failed
  step never produced.
- Record five facts before anything else: the failing step; its exact
  command (`search` the pipeline file: `.github/workflows/*.yml`,
  `.gitlab-ci.yml`, `Jenkinsfile`, `.buildkite/`, `.circleci/config.yml`);
  runner image or OS; the versions the setup step printed; the SHA it ran.
- More than one red job: `write_todos`, one item per distinct failure
  signature. Setup and install failures first; they invalidate every later
  row.

## 1. Reproduce with CI's command in CI's world

- Same command, verbatim from the pipeline file. `npm test` is not
  `npm ci && npm test -- --ci`; `cargo test` is not `cargo test --locked`
  under `RUSTFLAGS=-D warnings`. Put the step's env (`CI=true`, `TZ`,
  `LANG`, feature flags) on the command line.
- Same commit. Your tree has edits CI never saw:
  `W=$(mktemp -d); git worktree add -q "$W" <sha>`, run there,
  `git worktree remove --force "$W"` after.
- Same starting state. CI starts empty: `npm ci` not `install`, a fresh venv,
  `cargo clean` once. A warm local cache is the usual "passes for me".
- Same image when one is named and `docker` exists (command in the
  reference). Slow: one `bash` call, `yield_time_ms` 300000. Paste the tail
  and `echo exit=$?`.
- Reproduced: step 2, real or drift. Not reproduced: the difference between
  the two environments is the lead (toolchain version, lockfile, env, CPU
  count, network, case-sensitive file system); do not say flaky yet.

## 2. Classify

One class per failure signature. Evidence first, name second.

| Class | Evidence that earns the name | What fixing the class means |
|---|---|---|
| Real | reproduces at CI's SHA with CI's command; first error is in project code or a project test | omm-debug: one hypothesis, regression test, minimal fix. Never `skip` |
| Flaky | the SAME SHA has a green and a red run of the same job, or the local loop fails some of N runs; trace names time, order, a port, the network | measure the rate over N runs, remove the shared state or await the real condition; else quarantine (rule below). Never a blanket `retry:` |
| Infra | error before the project's first command, or from the platform: runner lost, `exit 137`, disk full, registry 5xx, rate limit, DNS, checkout auth | one re-run, named as infra. Repeats: cache, pin a mirror, more resources, split the job. Project code untouched |
| Drift | CI's world moved with no commit: `latest` tag, unpinned action or orb, toolchain minor, expired token or cert, runner image rollover, renamed default branch | pin what moved to an exact version or digest, in the job that broke; message states old -> new |

One red run is not evidence of flakiness. The last commit's diff is a
hypothesis, not a verdict. Green on re-run with no change is flaky or infra
and still gets a class.

## 3. Fix the class, not the symptom

- Real: where the evidence points, behind a test (omm-debug), then the CI
  command again locally.
- Drift: pin, do not chase. `setup-node@v4` -> its SHA; `image: python:3` ->
  `python:3.12.6`. Never widen a range or drop `--locked` to get green. Pin
  OR upgrade, never both.
- Infra: a retry only on the exact signal (`exit_codes: [137]`, the network
  step), never on the whole job.
- Flaky: the cause (shared state, timing, order), or quarantine.
- Pipeline file edits only for infra and drift. Broken `main` blocking
  others: offer `git revert <sha>` first; commit and push need the user's
  request (bundled:git).

Quarantine rule. All four, or no quarantine: (1) a measured rate, pasted
(`for i in $(seq 20); do <rerun cmd> >/dev/null 2>&1 || echo FAIL $i; done`);
(2) a tracking issue naming test, rate, suspected cause; (3) the skip reason
cites it (`@pytest.mark.skip(reason="flaky 3/20, #412")`; other frameworks
in the reference); (4) an expiry date or an owner. Never quarantine a test
that fails 20/20 (that is real), one guarding auth, payments or data
integrity, or several at once. Never delete the test or an assertion.
Quarantine is its own commit.

## 4. Re-run the exact job

- A re-run runs the same SHA: right for confirming infra or a flake
  (`gh run rerun <run-id> --failed`, `glab ci retry <job-id>`), useless after
  a fix. A fix needs a push, and a push needs the user's ask (bundled:git);
  without it, stop with the diff and the exact re-run command.
- Watch to the end: `gh run watch <run-id> --exit-status` in one `bash` call,
  `yield_time_ms` 300000. Paste the conclusion line and run id. A different
  job going green proves nothing.
- The fix touched shared config (pipeline file, lockfile, image, cache key):
  the whole workflow must be green, not only the job that was red.
- Flaky, fixed: one green run proves nothing. Three green re-runs, or the
  20-run loop at 0 failures, pasted.
- Still red: back to step 0 with the new log. A moved first error is
  progress, not done.

Final message: signature, class with the evidence that earned it, cause in
one line, the change, the re-run conclusion line, anything quarantined with
its issue. An environment trap others will hit: `add_memory`, one line.

## Judgment calls

- Red on main and on your branch: main first; yours is not yours until you
  merge main and re-run.
- Several jobs red at once: find the shared step (setup, install, base image,
  a secret); fix it once.
- One matrix axis red (OS, version): that axis's log. Usually a real failure
  the other axes hide (path separators, map order, a removed API); an
  `if: matrix...` exclusion is a last resort with a stated reason.
- Timed out: a hang repeats the same last line for minutes. Raise a timeout
  only for a measured slowdown. Secret missing on a fork PR: expected; skip
  that step conditionally.

## Refuse

Re-running before reading the log ("let's see if it passes again").
`continue-on-error: true`, `allow_failure: true`, `|| true`, `--no-verify`,
`-Wno-error`, `[skip ci]`, deleting the check. Rerunning a flake until green
and calling it fixed. "Should pass now" without the pasted re-run.

## Micro-example

"CI is red on main." `gh run list --branch main --limit 3`: run 8123 failed,
job `test`. `gh run view 8123 --log-failed 2>&1 | tail -n 80`, first error:

```
npm ERR! `npm ci` can only install packages when your package.json and
npm ERR! package-lock.json are in sync. Missing: zod@3.23.8 from lock file
```

Reproduce: `W=$(mktemp -d); git worktree add -q "$W" 4f1c2a9; (cd "$W" &&
npm ci); echo exit=$?` -> same error, `exit=1`. Locally `npm install` had
passed: it rewrote the lock silently. Class: real (a commit landed with a
stale lock); not drift (nothing in CI moved); not flaky (3 of 3 runs red at
this SHA). Symptom fix, refused: switch CI to `npm install`. Class fix:
`npm install --package-lock-only`, commit both files. User pushes; run 8125;
`gh run watch 8125 --exit-status` -> `test completed with 'success'`,
`exit=0`. `add_memory`: "CI runs `npm ci`; run it before pushing
package.json."
