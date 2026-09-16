---
name: "omm-handoff"
description: "On wrap up, hand off, I have to stop, or a long task ending with work left: write a resume note (changed, verified how, left, open questions, exact next command) to the PR or project notes, durable facts to memory; Do not use mid-task."
---

# Handoff

Contract: the next actor (the user tomorrow, a teammate, a fresh session) runs
ONE pasted command and is where you are now, without re-deriving anything. The
note is built from tool output taken now, never from your recollection of the
session; after compaction your recollection is a claim and `git diff` is the
fact. Nothing in it may say more than a run showed. No commit, push, stash, or
`git checkout .` to "leave it clean" (bundled:git); the tree as it stands is
part of the handoff. Lessons for future sessions are omm-reflect's job; this
note carries state.

## 0. Freeze

- Stop editing. Every edit after this point invalidates every run you cite.
- `subagent_status` each child you spawned. Running: `subagent_wait` once
  (`timeout_ms` up to 300000), then `subagent_cancel` with a reason; a child
  cannot be handed off. `git worktree list`: a kept child worktree is a path
  the resumer must know.
- A `bash` session still running (dev server, watcher): it dies with the
  session. Its start command goes into Left or Next; `bash_input` to stop it.
- `git status -sb --porcelain=v1 --untracked-files=all`, `git diff --stat`,
  `git diff --cached --stat`, `git stash list`, `git rev-parse --short HEAD`.
  A stash is invisible to a resumer: pop it, or name it in Tree with its
  contents. Scratch files you created: delete them now; anything else stays.

## 1. Runs: current or stale

- The decisive check for each claim you will make (tests, build, lint, repro).
  Under ~2 minutes: run it now with `bash`, `<cmd>; echo exit=$?`, paste the
  summary line and the exit code.
- Longer: cite the last run, marked `stale: ran before <edit>`. A stale run
  is a fact about the past; the resumer reruns it, so put the command in Next.
- Red is handed off as red, verbatim. Never "tests should pass" or "almost
  green"; the count of failures and their names is the state.

## 2. Draft the note

```text
## Handoff <yyyy-mm-dd HH:MMZ> - <branch> @ <sha>
Goal: <the task in one sentence, as the user stated it>
Done: <behaviour now true, with path:line; not a file list, not a diary>
Decided: <choice made without the user> over <rejected>, because <one clause>
Verified: - <claim>: `<cmd>` -> <decisive line>, exit <n> (current | stale: ran before <edit>)
Not verified: <claim> - <reason>; did <substitute> instead   (or: none)
Left: 1. <smallest unit, path:line, what done looks like>  2. ...
Open: <decision the resumer must take>: <option a> | <option b>; I lean <a> because <clause>
Traps: <symptom> -> <fix>   (only what cost an attempt)
Tree: <n> modified, <n> untracked, <n> staged, uncommitted; stash: <none|name>; worktrees: <none|paths>
Next: <one command, exact argv, from the repo root>
```

- `date -u +%Y-%m-%dT%H:%MZ` for the stamp. Every line is derivable from a
  tool result in front of you; delete the line rather than guess.
- Left comes from your `write_todos` list, which does not survive the session:
  open items move into the note in order, each with the file that carries it.
  "Polish" and "cleanup" are not items; a test name is.
- The best Next is the red check: it proves the resumer is on the right
  state and names the first task. No red check: the command that reruns the
  stale check, or the exact one-liner that starts step 1 of Left. Never a
  goal ("finish the parser"), never a placeholder (`<path>`), never two.
- One screen; a note the resumer cannot read in a minute will not be read.
- Secrets, tokens, personal paths: none; a PR comment is visible to the repo.

## 3. Place it

| situation | destination |
|---|---|
| an open PR for this branch (`gh pr view --json number,url`) | `gh pr comment <n> --body-file -` with the note on stdin (`<<'EOF'`); the body stays untouched, it describes the change, the comment describes the state |
| GitLab | `glab mr note <n> --message "$(cat <file>)"`; no CLI: the repo file below |
| no PR, the repo keeps notes (`HANDOFF.md`, `NOTES.md`, `docs/notes/`; `search` with `glob`, `pattern: "^"`, `mode: "regex"`, `output_mode: "files_with_matches"`) | append there in that file's format (`edit_file`) |
| neither | `write_file` `HANDOFF.md` at the repo root, untracked; `git status` shows it, which is how it gets found; the resumer deletes it when the task closes |
| repo is not the user's, or read-only | a scratch path outside the repo, named in the reply |

## 4. Memory

Two kinds of line, both in `personal_project` (`read_memory` `scope`,
`path: "MEMORY.md"` first; the snapshot is stale after any write):

- Pointer, exactly one per task: `- Handoff for <task>: <PR url | path>,
  branch <name> (<date>)`. An older pointer for the same task exists:
  `edit_memory` it; else `add_memory` (`type: "project"`). Never `project`
  scope: it lands in version control and a teammate inherits a stale pointer.
- Durable facts, in omm-reflect's form `- <symptom> -> <fix> -- <evidence>
  (<date>)`: a trap that will recur, a command that cost an attempt, an
  environment fact. Not the branch, the open items, or the goal; those live in
  the note. Three or more: omm-reflect after the note.

## 5. Reply

Where the note is (URL or path), the Next line verbatim, what stays uncommitted
(paths), and anything you could not settle. Nothing else.

## Judgment calls

- Work finished, suite green, tree clean: omm-verify's report is the handoff.
  Leave a note only if a PR waits on someone (Left: review) or the user asked.
- Half-written code: two minutes of `edit_file` make it compile: do it, rerun,
  say so. More: leave it, name it in Left with path:line and what it was going
  to do. Never delete in-flight work to get a tidy tree.
- "Save my work" or "commit it" inside the wrap-up: omm-commit (its `WIP:`
  row); the note still lists what the WIP commit lacks.
- "I have to stop NOW": Tree, Left, Next, in the reply, three lines; the rest
  only if time remains.
- Resumer is you: same note; you after compaction are the stranger it is for.
- A fact about the user's preferences: `personal` scope, `type: "feedback"`.

## Refuse

- Committing, pushing, stashing, resetting, or reverting to hand off "clean".
- A note reconstructed from memory of the session instead of tool output.
- "Should pass", "nearly done", "minor cleanup left".
- A Next that is a goal, a placeholder, or a list.
- Narrating what was tried in what order; only surviving state ships.
- Session state in `project` memory, or the note itself in any memory.
- Leaving a running child, a stash, or your scratch file behind unnamed.

## Micro-example

User: "have to stop, hand this off". Goal: lock `/login` after five failures.
`subagent_status`: none. `git status -sb`: `M src/auth/login.py`, `M src/auth/store.py`,
`?? tests/test_lockout.py`; stash empty. `pytest tests/test_lockout.py -q; echo exit=$?`
(30 s): `2 passed, 1 failed`, `FAILED test_lockout_resets_after_window`, `exit=1`.
`gh pr view --json number` -> 212. Comment on #212:

```text
## Handoff 2026-09-05T14:10Z - feat/login-lockout @ 3f9c2a1
Goal: lock /login for 15 min after five failed attempts per account.
Done: counter and lock in src/auth/store.py:41-77; 423 response in src/auth/login.py:88.
Decided: per-account over per-IP, because shared NAT would lock whole offices.
Verified: - lock engages: `pytest tests/test_lockout.py -q` -> 2 passed, 1 failed, exit 1 (current)
Not verified: full suite - 6 min; did the auth package only.
Left: 1. reset after window: store.py:70 compares naive and aware datetimes; test names it.
      2. full suite. 3. CHANGELOG line (omm-docs).
Open: reset on successful login inside the window? spec silent; I lean yes.
Traps: `pytest -p no:cacheprovider` needed, tests/ is read-only in CI image.
Tree: 2 modified, 1 untracked, 0 staged, uncommitted; stash: none; worktrees: none.
Next: pytest tests/test_lockout.py -q -k resets_after_window
```

`read_memory` `personal_project` `MEMORY.md`: no pointer. `add_memory`: `- Handoff
for login lockout: PR #212 comment, branch feat/login-lockout (2026-09-05)`; and
`- pytest writes cache into tests/ -> run with -p no:cacheprovider in CI image (2026-09-05)`.
Reply: the comment URL, the Next line, the three uncommitted paths.
