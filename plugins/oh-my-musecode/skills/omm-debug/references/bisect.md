# Bisect: run scripts, edge cases, variants

Loaded on demand from `omm-debug`; the core recipe is in SKILL.md. Every variant needs
a predicate: REPRO exits 0 for good and non-zero for bad, with no prompts. Run it once
at a known-good and once at a known-bad point BEFORE bisecting. A predicate that is
wrong at an endpoint yields a confident, wrong answer.

## Picking the good commit

- Last release tag: `git describe --tags --abbrev=0`.
- By date: `git rev-list -n1 --before="2026-08-01" HEAD`.
- Verify it: check it out in the worktree and run REPRO. A wrong "good" makes every
  bisect answer wrong.

## The run script

Put it in a file outside the tree so checkouts cannot clobber it and quoting stays sane.

```sh
P=$(mktemp)
cat > "$P" <<'SH'
#!/bin/sh
<clean>  || exit 125      # cannot build here: skip this commit
<build>  || exit 125
<REPRO>  || exit 1        # 0 = good; the wrapper keeps a segfault (>= 128) from aborting
SH
chmod +x "$P"; git bisect run "$P"
```

- Never `exit 125` for a genuine test failure; 125 means "cannot judge".
- Exit 128 and above abort the whole bisect; hence `|| exit 1` around REPRO.
- Flaky REPRO: run it 3 times inside the script and fail if any run fails, or fix the
  flake first. A silently flaky predicate is the usual cause of a wrong culprit.
- Merge-heavy history: `git bisect start --first-parent HEAD "$GOOD"` walks only the
  mainline and reports the merge that introduced the change.
- Only some paths matter: `git bisect start HEAD "$GOOD" -- path/a path/b`.

## Slow builds

- One `bash` call with `yield_time_ms` 300000. A command still running after that keeps
  running and its output arrives when it finishes; do not poll it.
- Cut the step count first: a nearer good commit, or the path filter above.

## Untracked files

The worktree holds committed files only. REPRO that needs an uncommitted test, fixture,
or config: have the run script copy it in (`cp /abs/path/fixture.json "$PWD/test/"`)
before the build step.

## A dependency version

Enumerate versions, then binary-search by index. Swap the install line for the
package manager in use.

```sh
V=($(npm view <pkg> versions --json | tr -d '[]", ' | tr '\n' ' '))
lo=0; hi=$(( ${#V[@]} - 1 ))            # V[lo] known good, V[hi] known bad
while [ $((hi - lo)) -gt 1 ]; do
  mid=$(( (lo + hi) / 2 ))
  npm i --no-save "<pkg>@${V[$mid]}" >/dev/null 2>&1 \
    || { echo "skip ${V[$mid]}"; lo=$mid; continue; }
  if <REPRO>; then lo=$mid; else hi=$mid; fi
done
echo "first bad: ${V[$hi]}"
```

Then read that version's changelog and diff as evidence, not as the answer.

## Input, config, or fixture (delta debugging)

Halve the file; keep whichever half still fails; repeat until one line remains. Works
on JSON arrays, CSV rows, config keys, and SQL fixtures alike.

```sh
cp input.txt work.txt
while [ "$(wc -l < work.txt)" -gt 1 ]; do
  n=$(( $(wc -l < work.txt) / 2 ))
  head -n "$n" work.txt > a.txt; tail -n +"$((n + 1))" work.txt > b.txt
  if ! <REPRO> a.txt; then cp a.txt work.txt
  elif ! <REPRO> b.txt; then cp b.txt work.txt
  else break   # neither half fails alone: the bug needs a pair; bisect one half against the other
  fi
done
cat work.txt   # minimal failing input
```

Config flags: toggle half of the diff between the working and failing config at a time.
Keep a two-column log (value, pass/fail) as you go; it is the evidence for the report.

## Test ordering and shared state

Run the failing test alone. If it passes, the bug is in what ran before it. Bisect
over the list of preceding tests: run the target after the first half of that list,
then after the second, keep the half that still fails, repeat. The single test left is
the one leaking state (globals, env, temp files, database rows).

## Time in a log

The first error line is rarely the first anomaly. Find it, then walk backwards.

```sh
grep -n -m1 -E 'ERROR|panic|Traceback' app.log                  # first error, line N
sed -n "$((N - 200)),${N}p" app.log | grep -v -E 'INFO|DEBUG'   # what was odd just before
```

The last unusual line before N is the first hypothesis.

## Reading the result

The first bad commit is where the observable failure started, not necessarily where
the defect lives. `git show <sha> --stat`, then `read_file` the changed files, and form
the step-3 hypothesis from them.
