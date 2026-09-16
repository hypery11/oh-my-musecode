---
name: "omm-cleanup"
description: "Clean up, dead code, tech debt, lint errors, unused imports or deps, stale TODOs: finder output not eyes, one category per pass, tests green after each, no behaviour change; Do not use when code must move or be renamed (omm-refactor)."
---

# Cleanup sweep

Contract: what the code does is identical before and after. Every deletion is
justified by a finder's output line plus a `search` that came back empty, never by
reading alone. One category per pass; SUITE green after each pass, summary pasted; one
commit per category when commits were asked for (bundled:git). A move, split, or rename
is omm-refactor's job; a bug seen on the way is a `later:` todo, never a fix.

## 0. Baseline and inventory

- `bash` -> `git status --porcelain`. Dirty tree: stop and ask; your edits must be
  separable from theirs. Never stash or reset for them.
- SUITE = tests + type checker + linter, check mode, CI's exact flags (`search` the CI
  config, `Makefile`, `package.json`, `pyproject.toml`). Run it (`yield_time_ms` up to
  300000), paste the summary lines, name every pre-existing failure. A red you cannot
  name: stop; a sweep on an unexplained red is unmeasurable.
- Inventory before any edit: run each finder in the table, full output to a scratch
  dir outside the repo (`mktemp -d`). That output is the worklist; nothing off it gets
  touched. Exclude generated and vendored paths.

| Ecosystem | dead code, unused exports | unused deps | lint by rule |
|---|---|---|---|
| JS/TS | `npx knip`, `npx ts-prune` | `npx knip`, `npx depcheck` | `npx eslint . -f json`, `npx tsc --noEmit --noUnusedLocals` |
| Python | `vulture . --min-confidence 80` | `deptry .` | `ruff check . --statistics` |
| Rust | `cargo clippy --all-targets` (dead_code, unused) | `cargo machete` | `cargo clippy --message-format=short` |
| Go | `deadcode ./...`, `staticcheck -checks U1000 ./...` | `go mod tidy`, then `git diff go.mod` | `golangci-lint run`, `go vet ./...` |

Markers: `search` mode `regex`, pattern `\b(TODO|FIXME|XXX|HACK)\b`. Commented-out
code: `^\s*(#|//)\s*(if|for|return|import|def|fn|let)\b`; debug: `print(`, `dbg!(`.

Tool missing: run it ephemerally (`npx --yes <tool>`, `uvx <tool>`, `go run
<module>@latest`); otherwise ask before installing. Still missing: the category is
`not scanned` in the report, never done by eye.

- `write_todos`: one item per category with its count, in table order (a dependency
  looks used until its dead import is gone).

## 1. Categories, one pass each

| # | Category | Delete only when |
|---|---|---|
| 1 | unused imports, locals | the import has no side effect (no registration, init hook, CSS or polyfill import, trait brought into scope) |
| 2 | dead functions, types, exports, files | `search` (mode `literal`, `word` true, `hidden` true; code, docs, configs, templates, strings) returns only the definition |
| 3 | unused dependencies | zero import hits AND not a plugin, CLI, peer, runtime-loaded or build-time dep; removed with the package tool, never by hand in the lockfile |
| 4 | TODO / FIXME / XXX / HACK | section 2 |
| 5 | lint debt, one rule per pass, highest count first | each site read; the autofix diff read to the end |
| 6 | commented-out code, debug output | git has the history; a comment that explains WHY stays |

Pass loop, per todo:
1. Mark `in_progress`. Reread the finder's lines for this category only.
2. Where the last column needs a `search`: run it per candidate, record hits. Any
   hit outside the definition: keep, list under "kept" with the hit.
3. `edit_file` per site; `rm` for a whole file (its test file goes in the same pass).
   Autofix only for this category (`ruff check --fix --select F401`, `eslint --fix
   --rule ...`); never `--unsafe-fixes`; then read all of `git diff`. A hunk that
   changes a condition, value, order, or message is not a fix; revert it.
4. Run SUITE. Same passes as baseline, no new failure, no test skipped or edited.
   Paste the summary. Red: undo this pass (`git restore -- <paths>` for files clean at
   baseline, `rm` files you created), rerun SUITE to prove green, halve the pass,
   retry. Red after two halvings: the finder was wrong; keep, report. Never fix forward.
5. Green: `completed`; name the commit point. Commits only if the user asked; then one
   per category, shaped by omm-commit, subject naming category and tool.

## 2. TODO triage

Each hit gets exactly one outcome; "leave it" is not one.

- Delete: the work is done (`search` proves the thing exists or the bug is fixed), or
  the text is unactionable (`TODO: improve`, `FIXME: hacky`) with no owner or link.
- Issue: actionable and more than a line, or needs a decision. Draft title and body
  (file:line, the comment, what "done" means). `gh issue create` only when the user
  asked; otherwise the drafts go in the report. Once a number exists, rewrite the
  comment to `TODO(#<n>): <one line>`.
- Fix it here: never, even under ten lines; `later:` todo.

## 3. Finish

SUITE once more; paste it beside the baseline line. Study the full `git diff` once:
every hunk is a deletion, a lint fix with a nameable rule id, or a marker edit; anything
else is undone. Report:

```text
Baseline: <suite line>   Final: <suite line>
| category | found | removed | kept (why) |
TODO: <n> deleted (done/obsolete), <n> open -> <issue ids | drafts in report>
Lint: <rule> <before> -> <after> ...
Not scanned: <category> - <missing tool>
later: <bugs, refactors, design questions seen>
```

## Judgment calls

- Finder says dead, `search` finds one hit in a string, template, config, or doc:
  keep. Dynamic dispatch is where finders lie: `getattr`, reflection, DI containers,
  plugin registries, ORM models and migrations, signal handlers, pytest fixtures,
  entry points in `pyproject` or `package.json`, framework lifecycle methods.
- An export of a package others install is public API, never dead on this evidence;
  report it.
- Code behind a flag that is off is not dead; behind a flag no config sets any more,
  it is, and the check goes with it.
- Zero-import dependency with a plugin, `postinstall`, `@types/*`, pytest-plugin or
  `build-system.requires` role: keep, note it. A removed dependency: run the install so
  the lockfile follows, same commit.
- A rule suppressed inline more than ten times is policy, not debt: report, do not pick.
- Two "duplicate" functions, an unused parameter on a public signature, a dead branch
  whose removal changes a return type: structural; stop this pass, omm-refactor.
- Whole-repo formatter: only when asked; last pass, own commit. Slow suite: targeted
  subset per pass, full SUITE at the end.
- Large repo: `subagent_spawn` one child per top-level directory, `worktree_isolation`
  on, one category each; integrate one child's diff at a time, SUITE after each.

## Refuse

- Deleting by eye; "looks unused" is never a finding.
- `# noqa`, `eslint-disable`, `#[allow(dead_code)]`, `//nolint` to reach a clean run.
  One suppression per proven false positive, reason on the same line, is the limit.
- Editing lint config to silence a category, unless asked.
- Two categories in one diff or one commit; reformatting inside a non-formatter pass.
- Deleting or weakening a test that still passes; only tests of deleted code go.
- Claiming green without the pasted SUITE line after the last edit.

## Micro-example (Python: ruff, vulture, deptry)

Baseline `pytest -q` -> `212 passed`; `ruff check .` -> `Found 37 errors`. Inventory:
F401 14; `vulture src --min-confidence 80` -> 3 lines, one `src/legacy/export.py:12:
unused function 'to_csv'`; `deptry .` -> `DEP002 'requests' ... not used`; TODO 9.

Pass 1: `ruff check --select F401 --fix`. Diff: 14 imports gone; one is `from . import
signals` in `src/app/__init__.py`; `search` `signals` -> handlers register on import.
Restored as `from . import signals  # noqa: F401  registers handlers`. `212 passed`.
Pass 2: `search` `to_csv` (literal, word, hidden) -> the definition and
`docs/export.md:40`. Kept, reported: a documented API nobody calls is a decision. The
other two: zero hits, deleted with their tests. `210 passed`; two fewer, said so.
Pass 3: `search` `requests` -> `scripts/release.py:3`; `deptry` scanned `src/` only.
Wrong group, not unused: reported, not removed.
Pass 4: 4 TODOs reference merged PRs, deleted; 5 drafted as issues; no `gh` writes
allowed, so the drafts are in the report. Final: `210 passed` / `Found 21 errors`.
