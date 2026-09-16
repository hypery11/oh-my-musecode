---
name: omm-review
description: Review a diff, branch, or PR (look over my changes, before I merge): read it all, then severity-ranked findings with file:line and a failure scenario each, correctness vs simplification, verdict; Do not use when asked to write or fix code.
---

# Review

Report-only. Never edit files or fix what you find. If the user wants
fixes afterwards, that is a separate task. Follow the steps in order.

## 1. Resolve the target

Take the first rule that applies; run the commands with `bash`.

- Nothing named ("my changes"): `git status --short`, then `git diff HEAD`. Both empty:
  `git diff <base>...HEAD`.
- A branch: `git diff <base>...<branch>` (three-dot = everything since the merge-base).
- A PR number or URL: `gh pr view <n> --json title,body,baseRefName`, then `gh pr diff <n>`.
  Context beyond the diff: `git fetch origin pull/<n>/head`, then `git show FETCH_HEAD:<path>`
  (`read_file` reads only the working tree). No `gh`: ask for the branch.
- A path: review the whole file, not a diff.

`<base>` is the branch the user names, else `git symbolic-ref --short refs/remotes/origin/HEAD`,
else `main`. Run `git diff --stat` on the same range first to learn the shape; never review
from the stat alone. Over 3 files or 300 lines: `write_todos`, one item per file group, tick
each as you finish reading it. Over ~1,500 lines: split by top-level directory or by commit
(`git log --oneline <base>..HEAD`) and review every part. Do not sample.

## 2. Study everything before writing anything

Judge only after the whole change is read: a later hunk often adds the guard an earlier
one seemed to miss. Order:

1. Intent: PR body, or `git log <base>..<branch> --format=%s%n%b`. The diff says what
   changed, not why. Where description and code disagree, the code is the fact.
2. The whole diff, top to bottom. Note questions; do not answer them yet.
3. For every hunk, `read_file` the enclosing function or type (`offset`/`limit`); the
   whole file when it is under 400 lines. The hunk's three context lines are not context.
4. For every changed signature, renamed field, altered return value or error path, changed
   default, or new precondition: `search` (literal mode, `glob`-scoped) for callers,
   implementers, and tests. A caller the diff did not update is a finding.
5. The tests in the diff: which changed branch has no test that would fail without it?

Multi-area diffs: `subagent_spawn` one read-only child per area in the shared checkout (no
`worktree_isolation`); objective = that area's file list plus steps 2-4 and the section 5
format copied in. Merge and re-rank the results yourself.

## 3. Hunt

Walk each hunk with these probes (per-domain expansion in `references/bug-classes.md`,
section 7):

- Intent: promised in the description but not in the code, or the reverse.
- Inputs: nil/None, empty, zero, negative, non-ASCII, oversized; off-by-one at every bound.
- Errors: swallowed exception; error path that skips cleanup or unlock; not-found blurred with failed.
- State: ordering assumption; check-then-act on shared state; retry without idempotency; partial write.
- Contracts: changed default; changed return meaning; removed validation; caller not updated.
- Persistence: migration, serialized shape, stored default, cache key missing an input.
- Resources: unclosed handle; unbounded growth; call inside a loop that belongs outside it.
- Security: string built into shell, SQL, HTML, or path; secret logged; auth check removed or moved.
- Tests: new behaviour with no test; an assertion weakened; a mock that replaces the changed path.

Record each candidate as `path:line - claim`.

## 4. Verify each candidate

`read_file` the exact lines again, and the caller that would trigger it. Keep the item only
if you can write a concrete failure: this input or state -> this wrong output, crash, or lost
data. Cannot write one: drop it, or put it under Questions. A suspicion is never a finding.

- Pre-existing bug: report only if the diff worsens it or touches the line; label it
  `pre-existing`; keep it out of the verdict.
- One finding per root cause; list the other affected sites inside that item.
- When cheap (under a minute, user did not forbid it), confirm with `bash`: the tests
  covering the touched files, or a one-line reproducer. Do not modify tracked files to do so.
- Zero findings is a legitimate result. Do not manufacture one.

Simplification is a separate pass and a separate section. Report only: code the diff adds
that is never reached; logic an existing helper already implements (`search` for it, name
it); an abstraction with one caller; state that is never read. Only when removal is clearly
safe. No naming, formatting, or style comments unless the user asked, except a style issue
that hides a defect (shadowed variable, misleading name behind a wrong call).

## 5. Report

Severity: Blocker = data loss, security, wrong output or crash on a normal path.
Major = wrong result on a plausible edge path, or a contract callers rely on broken.
Minor = wrong on an unlikely path, or a simplification that removes real risk.
Nit = style; omit unless asked.

```text
## Correctness
1. [Blocker] src/x.py:123 - <one-sentence defect>
   Scenario: <input or state> -> <wrong result>
   Fix: <one line, optional; the smallest fix, never a redesign>

## Simplification
- src/x.py:45 - <what to drop or reuse, and why it is safe>

## Questions
- src/x.py:200 - <what you could not verify, and why>

## Verdict
<Merge | Merge after fixing #N | Do not merge> - <one sentence>. Tests: <ran | not run>.
```

- Every Correctness item has `path:line` (line in the new version of the file) and a
  `Scenario` line. No exceptions.
- Order each section by severity, then by path and line. Omit empty sections; never omit
  Verdict.
- Do not merge if any Blocker or Major stands; Merge after fixing #N for Minor only; Merge
  when Correctness is empty.
- No preamble, no praise, no summary of what the diff does. The reader wrote it.
- "Add tests" only with the input the missing test would catch. Name the defect, not a
  rewrite. Never file a simplification as correctness to raise its severity.

## 6. Worked example

Hunk in `src/store.rs`:

```diff
-    let idx = items.iter().position(|i| i.id == id).unwrap();
-    items.remove(idx);
+    if let Some(idx) = items.iter().position(|i| i.id == id) {
+        items.remove(idx);
+    }
+    cache.invalidate(id);
```

`search` for `invalidate(` finds `src/audit.rs:41`, which logs a removal on every call. Report:

```text
## Correctness
1. [Major] src/store.rs:91 - cache is invalidated when nothing was removed.
   Scenario: two callers race on one id; the second finds no item, skips the remove,
   still invalidates -> src/audit.rs:41 logs a removal that did not happen.
   Fix: move `cache.invalidate(id)` inside the `if let`.
## Verdict
Merge after fixing #1. Tests: not run.
```

Not reported: the removed `unwrap()` (an improvement, not a finding); a separate "add a
test" item (its input is already named in #1).

## 7. Reference

Diff touches migrations, concurrency, auth, serialization, or an unfamiliar domain:
`read_file` `references/bug-classes.md` in this skill's directory (take the directory from
the `locator:` line of this read_skill result; the catalog's `plugin://` path is not
readable). Use it as a hunt list, not a checklist to report against.
