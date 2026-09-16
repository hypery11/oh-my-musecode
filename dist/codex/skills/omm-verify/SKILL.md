---
name: "omm-verify"
description: "Before claiming done, fixed, passing, or complete, and before any commit or PR, even if trivial: rerun checks after the last edit, paste real output, check diff scope, lint, stray files; Do not use mid-task or when no files changed."
---

# Verify before claiming done

A claim without a pasted run is a guess. Run this checklist before the words
done, fixed, complete, passing, or works appear in your reply, and before any
commit or PR. A one-line change gets the full checklist; that is where the
untested typo lives. Commit contents are omm-commit's job and commit
authorization is bundled:git's; this skill runs before either.

## Checklist

1. **List the claims.** One per line, each with the command that would prove
   it: tests pass, builds, bug X no longer reproduces, lint clean, no
   behaviour change, task item Y done. More than two: `write_todos`.

2. **Re-run every check, now.** With `bash`, after the last edit, as
   `<cmd>; echo exit=$?`. A run that predates any edit is stale. Use the
   project's own runner with the exact invocation CI uses: `search` the CI
   config, `Makefile`, `justfile`, `package.json` scripts, `pyproject.toml`.
   - tests: the suite for the affected package, plus the tests you added or
     changed, by name. Full suite when the change touches shared code,
     config, dependencies, or the build, and always before a commit or PR.
   - build: the mode CI uses (release, all targets, type-check). Incremental
     caches lie: clean-build once (`cargo clean`, `rm -rf build`,
     `--no-cache`) before saying "builds".
   - bug fix, both halves: the original repro passes now, AND the regression
     test fails on the old code. `W=$(mktemp -d); git worktree add -q "$W"
     HEAD` (or the pre-fix commit), copy the new test in, run it there,
     `git worktree remove "$W"`. Never red on old code: the claim is "test
     added", not "bug fixed". No repro exists: write one and run it.
   - lint, formatter, typecheck: in check mode, same flags as CI.
   - generated files, migrations, lockfiles you claim are current: run the
     generator and confirm a clean diff.

3. **Inspect the output, do not skim it.** Exit code and summary line. Greens
   that hide a red: `0 tests collected`, `no tests ran`, `skipped`, `xfail`,
   `warning` (an error in CI), a cached result, a watch mode that never
   finished, a script you wrote that prints PASS.

4. **Paste it.** The command, the summary line, counts, exit code, every error
   line, verbatim. Long output: last 20 lines plus exit code, full log in a
   scratch file outside the repo whose path you name. Never reword.
   "Tests pass" is a summary of evidence, not evidence.

5. **Check the diff scope.** `git status --porcelain --untracked-files=all` and
   `git diff --stat` (plus `git diff --cached --stat` if anything is staged).
   Every path traces to the request or gets a one-sentence justification in
   the report. Delete what you created: scratch scripts, probe logs,
   `.orig`/`.rej`/`.bak`, editor swap files, untracked build output. Remove
   debug prints and commented-out code. Drop unrelated hunks with
   `git restore -p <path>`. Leave files you did not create; report them.
   Re-run status to show the tree clean. Outside git: `read_file` each file
   you wrote and list them.

6. **Name what is not verified.** Anything asked for that has no run behind
   it is stated as unverified, with the exact command not run, the reason (no
   toolchain, needs network or credentials, too slow, cannot reproduce
   locally), and the substitute you did instead (typecheck, dry-run, reading
   the code path). Never let "should work" stand in for a run.

7. **Report, then claim.** The final reply carries this block; only after it
   may the reply say done, fixed, or complete:

   ```
   Verified
   - <claim>: `<command>` -> <decisive line>, exit <code>
   Diff: <n> files, in scope: <paths>
   Not verified: <claim> - <reason>; did <substitute> instead   (or: none)
   ```

## Decisions

- **What counts as evidence.** Output that could have disagreed with you: the
  project runner's summary and exit code, a `diff` against a golden file, a
  real request and its response body. Not evidence: a file existing, a
  function being defined, a `search` hit, an open port, a mock of the thing
  under test, a screenshot never read back with `read_file`.
- **Full suite over ten minutes.** Run the focused set now, say so, hand the
  full run to `subagent_spawn` with `worktree_isolation` on, collect with
  `subagent_wait` then `subagent_read_result` before the final claim.
- **Flaky.** Never from one run:
  `for i in 1 2 3 4 5; do <cmd> || echo FAIL $i; done`. Any failure is a
  finding to report, not a pass with an excuse.
- **Cannot run it** (no runtime, credentials, device, network): one install
  or setup attempt, then step 6.
- **"No need to test"** waives writing tests, not running the deliverable.
  Only an explicit "do not run anything" waives the run; then the report says
  the deliverable is unverified, in those words.
- **Visual deliverable the user will eyeball:** confirm it loads without
  console errors, no unrequested end-to-end sweep. A formula, parser, or data
  transform they cannot judge by eye still gets a real run on real input.
- **Verification found a problem.** Fix it, back to step 1. Every edit
  invalidates every earlier run.

## Rules

- Red is a result. Paste the failure and stop; partial success is not done.
- Never weaken a check to pass it: no skip, no `.only`, no `--no-verify`, no
  deleted test, no relaxed lint rule, no widened tolerance. A wrong check:
  say so and ask.
- No hedged verbs. "Should work", "ought to", "I believe": a claim was either
  run or is unverified; the report says which.
- Verification is read-only for the repository: no commit, `reset`,
  `checkout .`, `stash`, or `gc` to test old behaviour; use a worktree. Kill
  only processes you started.
- Commits and PRs: checklist first. The evidence block goes in the PR
  description or the reply, not the commit message. Committing itself still
  needs the user's request (bundled:git).

Worked example: `references/example.md`.
