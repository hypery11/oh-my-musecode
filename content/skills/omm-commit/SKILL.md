---
name: omm-commit
description: Use when asked to commit or save work - one logical change per commit, related hunks only, screened index, imperative subject, why-not-what body, no secrets; Do not use to open the PR (omm-pr) or for a squash, rebase, or history rewrite.
---

# Commit shaping

bundled:git decides WHETHER a history write may happen (the user asked for that
exact write, this session). This skill decides WHAT goes into each commit and
what its message says. Never `git add -A`, `git add .`, or `git commit -a`;
name paths. Never rewrite pushed or shared history: no `commit --amend`,
`rebase`, `reset`, or `push --force` on such refs; a wrong pushed commit is
fixed by a new commit. Never commit just to give a tool a clean tree; stash
and restore instead.

## Loop, once per commit

1. Survey with `bash`: `git status --porcelain`, `git diff`,
   `git log --oneline -15`. The log gives the repo's subject convention
   (prefix, casing, ticket style). A changed path you did not touch and
   cannot explain: stop and report (bundled:git).
2. Partition the diff into logical changes with the table below. Three or
   more partitions: list them with `write_todos`, one `in_progress` at a time.
   When a plan or the user already names the split, keep it.
3. Stage one partition: `git add <path>...` for whole files. For part of a
   file, run `git add -p <path>` with `tty: true` and answer each hunk prompt
   through `bash_input` (`chars`: `y`, `n`, `s` to split, `q` to stop). Hunks
   that `s` cannot separate: `references/partial-staging.md`.
4. Screen: `git diff --cached`, all of it, against the reject list below.
   `git restore --staged <path>` anything that does not belong and fix it at
   the source. Only then write the message; a message drafted before this
   read describes the intent, not the commit.
5. Commit from stdin so newlines survive:
   ```
   git commit -F - <<'MSG'
   <subject>

   <body>
   MSG
   ```
6. Verify: `git show --stat HEAD`. Subject and paths must match the
   partition. Wrong: add a fix-up as a new commit. Amend only when the user
   asked to amend and the commit is unpushed.
7. Next partition. Finish with `git status` showing only what was left out on
   purpose, and say what that is.

## Partition decisions

| Situation | Decision |
|---|---|
| A fix needs a preparatory rename or extraction | Two commits: mechanical change first (no behaviour change), fix second |
| Test and implementation for one behaviour | One commit; the test is the evidence for the change |
| Reformatting mixed with logic | Drop the churn, or a separate "Format ..." commit; never inside the logic commit |
| Tracked generated file whose source changed (lockfile, snapshot, schema) | Same commit as its source; an untracked or unrelated regeneration never |
| Dependency bump that a feature needs | Own commit first; subject names package and version |
| Two changes touch the same function | `git add -p`; if hunks cannot be separated, one commit whose body names both and says why |
| Drive-by fix you noticed | Own commit, or leave it unstaged and mention it |
| Tool-driven mass change (rename, codemod) | One commit; body gives the exact command so it is reproducible |
| "Save my work" on something half done | One commit per finished group; the unfinished remainder gets `WIP: <what works>` with a body listing what is missing; never push it |
| Unsure whether a hunk belongs | Leave it out. A missing hunk is one more commit; an extra one is history |
| Test: would a reviewer revert this commit alone, cleanly? | Yes: the partition is right |

## Message

- Subject: imperative, completes "If applied, this commit will ...". 72 chars
  hard limit, 50 preferred, no trailing period. Follow the prefix the log
  shows (`fix(auth):`, `[PROJ-9]`, plain); invent none.
- Body after a blank line, wrapped at 72. It answers: why was this needed;
  what was rejected and why; what is deliberately unchanged; how to verify if
  not obvious. It never lists files or narrates the diff.
- No body only when the subject leaves no question (typo, comment, version
  string).
- Issue references and trailers exactly as the log already uses them; add no
  trailer, sign-off, or tool credit the repo does not have.
- Banned subjects: `fix`, `wip`, `update`, `changes`, `misc`, `more`,
  `address comments`, and anything ending in `...`.
- Passing examples and a worked three-commit split: `references/examples.md`.

## Reject list for `git diff --cached`

- Secrets: `.env*`, `*.pem`, `id_rsa*`, `credentials*`, `*token*`; in the
  text `-----BEGIN`, `AKIA`, `ghp_`, `sk-`, `password=`, `secret=`. Unstage;
  never commit one "temporarily". A secret already pushed is compromised and
  must be rotated; deleting it later does not undo that.
- Generated or environment junk: `dist/`, `build/`, `node_modules/`,
  `coverage/`, `*.min.*`, `__pycache__/`, `.DS_Store`, editor swap files, a
  lockfile changed by an install you did not intend.
- Debug residue: `print(`/`console.log(`/`dbg!`/`debugger` you added,
  `TODO remove`, commented-out code, `<<<<<<<` markers, whole-file reindents.
- Anything belonging to another partition, or staged by pattern
  (`git add src/`) when only some files in it are yours.
Extended patterns and lockfile rules: `references/partial-staging.md`.

## PR

Title: the rules of a subject. Body: why (one paragraph), how to verify,
what is out of scope, the linked issue. Not the commit list, not the diff.
Run `git log --oneline <base>..HEAD` first; every line should read as one
step of the change. A "fix previous commit" line on an unpushed branch earns
an offer to re-split, done only on the user's ask. Push and open the PR only
on explicit request (bundled:git).
