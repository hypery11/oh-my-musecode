---
name: "omm-refactor"
description: "Use for refactor, extract, or rename across files: green baseline, seam list, one behaviour-preserving move then tests, callers migrated with it, stop on design calls; Do not use for a feature, a bug fix, or dead-code removal (omm-cleanup)."
---

# Refactor

Contract: structure changes, observable behaviour does not. Every step ends with the
same test run as the baseline, still green, summary line pasted. A public name is
unchanged or every caller moves in the same step. A step that needs a design decision
is reported, not taken. "Cleaner" is never a reason to change what the code does.

## 0. Baseline

- `bash` -> `git status --porcelain`. Dirty tree: stop and ask the user to commit or
  stash; your edits must be separable from theirs. Never commit, stash, or reset for
  them.
- Find the test command: repo wrapper first (`make test`, `npm test`, `just test`,
  `scripts/test*`), else the raw runner (`pytest -x -q`, `npx vitest run`, `npx jest`,
  `cargo test`, `go test ./...`, `bundle exec rspec`, `./gradlew test`, `dotnet test`,
  `swift test`, `mix test`). Add the type checker or linter CI runs (`tsc --noEmit`,
  `mypy`, `cargo clippy`, `go vet`); a compiler is a test. Call the whole thing SUITE.
- Run SUITE with `bash` (`yield_time_ms` up to 300000). Paste the summary line. Name
  every pre-existing failure; leave it red, never claim green over it. A failure you
  cannot name -> stop; a refactor on an unexplained red baseline is unmeasurable.
- `search` (mode `literal`) for tests that reach the code you will touch. None ->
  write characterisation tests first (assert current output, odd cases included, even
  outputs that look wrong; recipe in `references/moves.md`), run them green, say so in
  the report. Would take longer than the refactor -> stop and report before writing.

## 1. Seam list

- `read_file` every file in the target area, in full.
- For each symbol you will move, rename, split, or delete: `search` its name (mode
  `literal`, `word` true; repeat with `hidden` true) across code, tests, docs, configs,
  templates, and strings: reflection, `getattr`, DI registries, mock paths, CLI names,
  serialized keys. Record the count and the files. Put the list in the message before
  editing.
- Classify each symbol. Internal: free to change. Public: imported outside its
  package, exported from an entry point, documented, or used by code not in this repo.
  Not a refactor: serialized keys, DB columns, wire or schema fields, CLI flags, env
  vars, URLs, event and metric names, error codes callers catch by name; changing one
  is a migration -> section 3.
- `write_todos`: one item per step, one mechanical move each, with its count:
  "extract parse_header from load (3 callers)", "move Cache to util/cache.py (7)",
  "rename fetchAll -> listAll (12)", "inline helper_x (1)", "delete dead export foo
  (0)". Leaf-first: fewest dependents first. Every step must build and pass SUITE on
  its own. Over ~30 sites: chunk by directory and report chunk boundaries.

## 2. Step loop (per todo)

1. Mark it `in_progress`. Reread the lines you are about to touch.
2. One kind of change, with `edit_file` (`write_file` only for a new file): move,
   rename, extract, inline, or dedupe. Never two kinds in one step, never a move and a
   fix. Move, do not rewrite: copy bodies verbatim, then delete the original. No
   reformatting outside the touched lines.
3. Public name touched: keep the old name as a thin delegating alias, or migrate every
   caller now, tests and mock paths included. No third state. Rerun the `search`:
   zero hits for the old name outside comments, changelogs, and the alias itself.
4. Run SUITE. Same passes as the baseline, no new failure, no test skipped, deleted,
   weakened, or its expectation edited. Paste the summary line.
5. Green -> mark `completed`; name the commit point. Commit only if the user asked;
   then one commit per green step.
   Red -> the step is wrong, not the test. Undo it: reverse the `edit_file` calls in
   reverse order (`git restore -- <file>` only for files clean at baseline; `rm` a
   file you created). Rerun SUITE to prove you are back on green. Red means a missed
   site or a behaviour change: re-list seams, split the step, retry. Red after two
   splits -> section 3. Never fix forward.
6. Noticed a bug, a feature, a rename you did not plan? Add a `later:` todo and finish
   the current step. Keep the bug: fixing it here makes the test run unable to tell
   you anything.

## 3. Stop and report

Stop at the end of the current green step, and do not guess, when:

- two target shapes are both reasonable and the choice changes a caller-facing surface;
- a public name cannot be kept and consumers exist outside this repo or you cannot see
  them (a deprecated forwarding alias is the default; removing it is their decision);
- the step needs a behaviour change: the old code looks wrong, or two "duplicate" sites
  differ (diff the copies before any dedupe; copies that differ are a decision);
- a test asserts structure (private names, mock call counts) and its intent is unclear;
- the seam list crosses a serialized name, wire field, flag, or other migration;
- a step stays red after two splits.

Report: steps done (each green, kept in the tree), the step stopped at (undone), the
decision with options, one line of trade-off each, one recommendation, what remains.

## 4. Finish

- Run SUITE once more; paste its summary next to the baseline line.
- `bash` -> `git diff --stat`. `search` for leftovers: old names, aliases past their
  step, unused imports, dead exports, debug output.
- Formatting: at most one final step, the project's own configured formatter, only
  over files already touched. Never mixed into a move.
- Report: steps completed, baseline vs final suite line, public API changes (none /
  alias kept / N callers migrated), `later:` items.

## Judgment calls

- Step size: one you could revert from memory. A diff that needs a paragraph to
  explain: split it.
- Structure-coupled tests (private names, mock paths): update in the same step and say
  so; the asserted outcome must not change. A test asserting only implementation: ask
  before deleting.
- Dead code: delete only when `search` shows zero references, strings and docs
  included; otherwise report.
- Slow suite: targeted subset per step, full SUITE at chunk boundaries and at the end;
  say which ran where.
- Big and parallelizable: `subagent_spawn` one child per independent chunk with
  `worktree_isolation` on, each running its own SUITE; integrate one chunk at a time,
  SUITE after each.
- Renames: per-site `edit_file` from the `search` list, or the language's rename
  tool. A tree-wide text replace hits strings, comments, and unrelated identifiers.

## Example (Python)

"Clean up the tax logic; it is duplicated in orders and invoices." Baseline
`128 passed`. Step 1: extract `compute_tax` into `pricing.py` from `orders.py`,
verbatim; `128 passed`. Step 2: point `invoice.py` at it; `127 passed, 1 failed`
(`test_invoice_rounds_half_up`). Diff of the two blocks: invoice rounds half-up,
orders half-even. Not a duplicate; a decision. Undo step 2; `128 passed`. Report:
step 1 kept, stopped at step 2; (a) add a `rounding` parameter, no behaviour change;
(b) unify the rounding, a product decision. Recommend (a) now, (b) as its own task.

Recipes: extract, move, rename across files, inline, dedupe, characterisation test,
undo without commits in `references/moves.md`; per-language aliases and deprecation
markers, signature changes, module-move shims, rename tools, the not-a-refactor list
in `references/api-migration.md`.
