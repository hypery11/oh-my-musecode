---
name: omm-legacy
description: "Change legacy code (old codebase, no tests here, I do not understand this code): map entry points and data flow, pin current behaviour with characterisation tests, change one seam, note in AGENTS.md; Do not use when the work is greenfield."
---

# Legacy change

Contract: you are editing code nobody fully understands, you included. Done means the
final message holds the map (entry point to change site to output), a green
characterisation run from BEFORE the edit and one from AFTER, both pasted, a diff that
touches one seam, and the note left in `AGENTS.md`. Everything outside the requested
change behaves exactly as before, including the parts that look wrong. "It looks
unused" and "this is clearly a bug" are hypotheses, not permissions.

## 0. How much of this you need

| Situation | Do |
|---|---|
| tests already reach the site and pass | run them, skip section 2, then omm-tdd |
| no tests, pure function under ~50 lines | short map, 3-5 pins, change |
| no tests, I/O at the site (db, fs, network, clock) | map, seam first (section 3), pins through the seam |
| "explain this code", no change asked | section 1 only; report the map; no edit |
| "rewrite this" | not this skill: make the change the small way; a rewrite is a separate decision |

More than three steps ahead: `write_todos`, one item per section, one `in_progress`.

## 1. Map before you read line by line

- Entry points, with `search` (`mode` `literal`, `word` true; again with `hidden`
  true): the symbol, then the symbol in quotes (string dispatch, config, reflection,
  DI registries, cron tables). Count callers. A caller you did not expect is the most
  important line of the map.
- Age and reasons, with `bash`: `git log --date=short --format='%h %ad %s' -- <path>
  | head -15`; `git log -S'<symbol>' --oneline`; `git blame -L <a>,<b> <file>` on the
  odd lines. Old with many callers: assume every quirk is depended on.
- Data flow as a numbered list, `<file:line> - <what happens to the data>`, 5-12
  lines: what enters, what it becomes, where it leaves (return, write, message, log,
  exit code). `read_file` every function on the path in full; only files on the path.
- Run it once for real with `bash`: the CLI, the job, one test, or an import and a
  call on a small fixture. Capture the output verbatim. Cannot run it: say what
  blocks you; that fact goes into the note.
- Unknowns go into the map as questions (`? line 140 uses now(), ignores today`).
  Never answer one by editing. Three unknowns on the path and no way to run, or a map
  costing more than the change itself: stop, report the map so far, ask.

## 2. Pin current behaviour

Characterisation tests detect change; they do not judge correctness.

1. Inputs, 3-8: the real invocation from section 1; each branch the code visibly
   takes (`if x is None`, the `else`, the `except`); the case the comments call
   impossible; the input the change will alter; one input the change must NOT alter.
2. Run the code on each with `bash`; paste the actual outputs; assert exactly those.
   Never type an expected value from your head. An output that looks wrong is
   asserted as is, commented `# current behaviour`, and gets a `later:` todo.
3. Place them where the repo's tests live, in its style (`read_file` the nearest
   test file first). No framework: the language's built-in runner (`unittest`,
   `node --test`, `cargo test`, `go test`); never add one silently.
4. Run: green, pasted. Red or flapping on the first run is nondeterminism: pin it at
   the boundary (fake clock, sorted output, fixed seed, temp dir), not in the code.
5. Sensitivity: a pin that cannot fail is noise. Change one constant at the change
   site, run red, revert. Say that you did.

Golden files, fakes per boundary, per-language seam idioms: `references/seams.md`
beside this file.

## 3. Change one seam

A seam is where behaviour can change without editing in place. First that works:

1. An existing parameter or flag already threads through.
2. A new optional parameter whose default is the old behaviour.
3. Sprout: new logic in a new function with its own test; the old code gains one call.
4. Wrap: rename the old function `_x_legacy`, body unchanged; a new `x` calls it and
   adds the behaviour.
5. Subclass or override, when construction is the only place you control.

Sprout and wrap leave the old body textually intact, so the diff shows exactly what
you understood. Never edit the middle of a 300-line untested function: every branch
there is a caller you have not met.

- One `edit_file` per seam, touched lines only. No renames, reformatting, or moving
  files. Match the file's conventions even when you dislike them. Noticed something:
  `later:` todo.
- Then omm-tdd for the requested behaviour: failing test, red, minimum code, green.
  Structure only: omm-refactor, one move. Never both in one step.
- A pin goes red that you did not expect: you changed something you did not map.
  Revert, extend the map, retry. Never edit the pin to match. Pins for the requested
  behaviour are updated in the same edit, old and new value in the message.

## 4. Prove, then leave the note

- Run the pins, the new test, and every suite you found; paste the summary lines.
  Repeat the section 1 real invocation: identical apart from the requested change.
- `AGENTS.md`: `edit_file` to append a `## <path or area>` block (none: `write_file`
  at the repo root), 3-8 lines of facts a stranger needs first: entry points and
  caller count; the one-line data flow; the command that runs the pins; each trap
  (invariant not enforced, wrong-looking output that is depended on, what could not
  be run). No narrative. A stale line is edited, never duplicated. User said not to
  touch docs: `add_memory` the same lines and say so.
- Final message: map, pin names and both runs, the seam used, `git diff --stat`,
  the note verbatim, the `later:` list. omm-verify before the word done.

## Judgment calls

- Wrong-looking output (rounding, off-by-one, swallowed error): pin it, `later:` it.
  Someone downstream compensates for it; a silent fix is the classic legacy regression.
- Two copies of the logic: change the one on the mapped path, note the other.
- Dead code on the path: leave it, note it. Deleting is omm-cleanup's job.
- Suite red at baseline: name the failures, leave them, count them as no coverage.
- Global or singleton at the site: seam = a parameter defaulting to the global.
- Function over ~200 lines: map at branch level, test it through its inputs, sprout.
- Framework magic (decorators, ORM hooks, DI): the entry point is the registry;
  `search` for the registration string, not the function.
- "Clean it up while you are there": land the change; omm-refactor afterwards.

## Refuse

- An edit before a green pin run, or on a function not read in full with callers counted.
- Rewriting a function "to understand it"; the rewrite destroys the evidence.
- Fixing behaviour nobody asked about, or editing a pin to make it pass.
- Renaming, reformatting, moving files, or upgrading a dependency in the same change.
- "Behaves the same" without both pasted runs.

## Micro-example (Python)

"In `billing/legacy_invoice.py`, waive the late fee for `vip` accounts. No tests
here." Map: `search` `build_invoice` -> one caller, `jobs/nightly.py:88`;
`"legacy_invoice"` also in `config/jobs.yaml`. Last commit 2019. Flow: `nightly.py:88`
loads `Account` rows -> `build_invoice(acct, today)` -> dict -> `render_pdf`.
`_late_fee(acct, today)` at line 140; `? the >90-day branch calls datetime.now()`.
Run on three fixtures -> `0.0`, `25.0`, `25.0` (third looks like 50; pinned
`# current behaviour`). `write_file tests/test_legacy_invoice_char.py`, `unittest`,
`now()` patched at the boundary: `Ran 4 tests ... OK`. Sensitivity: `25` -> `26`, two
fail, revert. Seam: wrap; `_late_fee` returns `0.0` for vip, else calls the untouched
body, now `_late_fee_legacy`. `test_late_fee_waived_for_vip`: red, then green; `Ran 5
tests ... OK`. Real run: only vip rows differ. Note appended:

```text
## billing/legacy_invoice.py
- Entry: jobs/nightly.py:88 (only caller); also named in config/jobs.yaml.
- _late_fee_legacy ignores `today` past 90 days (datetime.now()); pinned, not fixed.
- Pins: python -m unittest tests.test_legacy_invoice_char
```

`later:` the `now()` branch; the 25-vs-50 case.
