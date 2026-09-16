---
name: omm-commit-message
description: Write or reword the message for an already-shaped commit or staged index (commit message, reword, amend the message, conventional commit subject); Do not use to decide what to stage or whether to commit (omm-commit, bundled:git).
metadata:
  triggers: commit message, commit msg, reword, amend the message, conventional commit, subject line, write the message, message for this commit
---

# Commit message

Goal: one message that a reader of `git log` understands without the diff.
Input: the staged index (`git diff --cached`) or one existing commit
(`git show <sha>`). Output: the message text, and the command that applies it
only when the user asked for the write.

## Read first

1. `git diff --cached --stat` then `git diff --cached` (or `git show --stat <sha>`
   and `git show <sha>`). Read all of it; a message drafted from the stat alone
   names files, not the change.
2. `git log --oneline -20`: the repo's convention - prefix (`feat:`, `fix(scope):`,
   ticket id, none), casing, subject length. Match it exactly; never introduce a
   convention the log does not use.

## Shape

- Subject: imperative mood, no trailing period, at most 72 characters, says
  WHAT changed for the user or maintainer of the code, not which files moved.
  Bad: `update parser.rs`. Good: `parse quoted trigger phrases in frontmatter`.
- Blank line, then a body only when the subject cannot carry the WHY: the
  problem observed, the decision taken, what was rejected and why. Wrap at 72.
- Trailers last, one per line, only ones the repo already uses
  (`Fixes: #123`, `Refs:`). No tool, model or generator attribution unless the
  repo's own log carries it.
- One logical change per message. A diff that needs "and" in the subject is
  two commits: say so and stop instead of writing a message that hides it.

## Apply

- New commit, only when asked: `git commit -F - <<'MSG' ... MSG` (stdin keeps
  the newlines). Never `-m` with a multi-line body.
- Reword the last unpushed commit, only when asked: `git commit --amend -F -`.
  Refuse on a pushed or shared ref (`git status -sb` shows `ahead` only when
  unpushed); a pushed commit gets a new commit, not a rewrite.
- Show the final message verbatim in the reply.
