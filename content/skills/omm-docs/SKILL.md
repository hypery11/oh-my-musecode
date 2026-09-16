---
name: omm-docs
description: Use when a feature lands or on (document, update the README, changelog): make README, CHANGELOG, and API docs match the code for a reader who never saw the diff, one example per new capability, structure kept; Do not use for code comments.
---

# Document a change

Docs state what the code does now, for a reader who has not seen the diff
and will not read it. The CHANGELOG states what changed. Never write a
sentence you have not checked against the code or a run. Do not touch code
or inline comments; a public docstring that a doc generator renders is docs,
an inline comment is not.

## 1. Fix the change

- Named PR, branch, or range: `bash` -> `git diff <base>...<ref>` and
  `git log <base>..<ref> --format=%s%n%b`. Nothing named: `git diff HEAD`
  plus `git status --short`; both empty: diff the branch against main or
  master. No git: ask which change.
- `read_file` the enclosing definition of every changed public symbol; a
  hunk's three context lines are not context.
- Build the change table before writing anything:
  `surface | old | new | kind (added/changed/deprecated/removed/fixed)`.
  Surface = command, flag, option, function, type, config key, env var,
  endpoint, default, error message, exit code, file format, install or
  upgrade step, minimum version. Only user-visible rows get docs.
- Nothing user-visible (refactor, tests, CI): at most one CHANGELOG line;
  say so and stop.
- More than three rows or files: `write_todos`, one item per row.

## 2. Find every place the docs mention it

- `search` each name in the table, old and new spelling, across README*,
  CHANGELOG*, docs/, *.md, *.rst, examples/, man pages, help text, config
  templates, OpenAPI or schema files, and public docstrings. Every hit is a
  line to update or delete. Zero hits for a changed name: the docs never
  covered it; pick the place a reader would look first.
- `read_file` each hit's file (whole file under ~300 lines, else the
  section) and copy the house style: heading levels, ordering, tense,
  code-fence language, how examples show output, how issues are linked.
- Note the docs build, doctest, and link-check commands from CI config or
  the Makefile; step 6 runs them.

## 3. Update the docs, one row at a time

`edit_file` per row. Say what it does, when to use it, and its constraints
(default, required inputs, platform, since-version); not how it is
implemented and not why it changed.

| Situation | Decision |
|---|---|
| New command, flag, option, config key, function | Reference entry (what, values, default) in the existing position, plus one example (step 4) |
| Default or behaviour changed | Rewrite the sentence to the new value. Old value and how to keep it go in the CHANGELOG, never "Note: as of vX" in the reference |
| Removed or renamed | Delete or redirect every hit from step 2. Old name stays findable one cycle as "Removed in X; use Y" |
| Bug fix | CHANGELOG line only, unless the docs described the bug as intended; then fix the description, no history |
| Behaviour inferred from the diff, not observed | Run it with `bash` first. Cannot run it: do not write it; report it as unverified |
| Code does X, PR text says Y | Document X; tell the user about Y |
| Behind a feature flag or marked experimental | Document the gated path only, marked the way the project marks experimental things |
| Existing docs wrong about something unrelated | Leave it; report it |
| User-facing and contributor-facing both affected | README or docs/ for the user; CONTRIBUTING or docs/dev for the contributor; never one paragraph for both |
| Same fact needed in two places | State it once; link from the other |
| Change is large and the README already long | Summary plus link in README; detail in docs/ |
| No section fits | Add the smallest section at the place the reader would look, in the existing heading order |

Never document what the code does not do: no planned work, no "will
support", no option you did not see in the diff or in a run. Never reorder
or reword unrelated text.

## 4. Examples

One per new capability, none per refactor or fix. The shortest invocation
that shows the result, run with `bash`, output pasted as printed (trim long
output and say so). Only the flags it needs; only paths that exist in the
repo. An example you cannot run is dropped, not guessed.

## 5. CHANGELOG

- Follow the file's format (Keep a Changelog sections or the repo's own).
  No CHANGELOG in the repo: say so; create one only on request.
- One line per visible row under Unreleased (or the version the user
  names), in the file's category. Lead with what the reader can now do or
  must now do: "`--timeout` accepts `30s`, `5m` suffixes", not "refactor
  duration parsing". Per visible change, not per commit.
- Breaking change: mark it the way the file does; migration in the same line.
- Issue or PR link exactly as neighbouring entries do; none if the file has none.
- Banned words: various, misc, improved, cleanup, powerful, seamless, robust.

## 6. Verify

- Run the docs build, doctests, and link checker from step 2 (`cargo test
  --doc`, `python -m doctest`, `npm run docs`, `mkdocs build --strict`, or
  the repo's equivalent). Fix what fails; paste the summary line.
- Every name in the changed docs: `search` it in the code at HEAD. Zero
  hits is the defect this skill exists to prevent.
- Every link, anchor, and path you touched: `search` the target heading, or
  `bash` -> `ls` the file.
- `git diff --stat`: only doc files and public docstrings changed. Then
  `git diff -- '*.md'` top to bottom as the reader who never saw the
  change; any sentence that needs the diff to make sense gets rewritten.
- Report: files touched; each row and where it is documented; examples
  run, with output; rows left undocumented and why; anything unverified.

## 7. Release notes checklist

Run when a version is cut or the user asks for release notes. Template,
cross-checks, and anti-patterns: `references/release-notes.md` next to this
file. Every box checked or named as open:

- [ ] Version and date match the tag, package manifest, and CHANGELOG heading.
- [ ] Every commit since the last tag maps to a line or a conscious skip;
      nothing outside the diff range is listed.
- [ ] Breaking changes first, each with before -> after and the migration step.
- [ ] Each new capability: one sentence plus the example from step 4.
- [ ] Each deprecation: what, the replacement, the removal version.
- [ ] Each fix: the symptom a user saw, with the issue link.
- [ ] Upgrade requirements: minimum runtime or toolchain, migrations to run,
      renamed config keys, new permissions.
- [ ] No internal refactors, "misc cleanup", or dependency churn without
      user impact.
- [ ] Credits in the repo's existing style, or none.

## Worked example

Diff: `cli/list.py` gains `--format {table,json}` (default `table`);
`config.py` changes the `timeout` default 30 -> 10; `utils/fmt.py` splits in
two. Table: added flag, changed default; the split is invisible. `search`
hits: README Usage mentions `list`; README Configuration row `timeout | 30`;
CHANGELOG uses Keep a Changelog headings and `(#123)` refs. `bash`:
`tool list --format json | head -3`, keep the output. `edit_file` README
Usage: one `--format json` example with those lines; Configuration row:
`timeout | 10`. `edit_file` CHANGELOG under Unreleased: Added "`list
--format json` prints one JSON object per line (#148)"; Changed "Default
`timeout` is 10 s, was 30; set `timeout: 30` to keep it (#151)". The split
gets nothing. Verify per step 6; report the two rows and anything unverified.
