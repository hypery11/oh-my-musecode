---
name: omm-pr-description
description: Write the pull request description for a branch (PR description, PR body, describe the PR, merge request text) from its real diff and commits, in the repo's template, with a review guide and test evidence; Do not use to open or merge the PR, and not for a commit message (omm-commit-message).
metadata:
  triggers: pr description, pull request description, pr body, describe the pr, describe this pr, merge request description, mr description, pr text, pr summary, write the pr
---

# Pull request description

Goal: a description that lets a reviewer approve without re-deriving the
change: what, why, how to review it, how it was proven. Output: the text,
and the command to attach it only when the user asked for the write.

## 1. Read the whole change

```
git log --oneline <base>..HEAD              # base = the target branch (git merge-base)
git diff --stat <base>...HEAD
git diff <base>...HEAD
```

Read every hunk once; a description written from the stat lists files and
misses the behaviour. Note anything in the diff the commits do not explain.

## 2. Use the repo's template

`read_file` `.github/PULL_REQUEST_TEMPLATE.md` (or `.gitlab/merge_request_templates/*`,
`PULL_REQUEST_TEMPLATE` in `docs/`). Fill every section it has; keep its
headings and checkboxes verbatim. No template: use the sections below.

## 3. Sections

- **Summary** - two to four sentences: the problem, the change, the user-visible
  effect. Lead with the behaviour, not the implementation.
- **Why** - the constraint or bug that forced it; link the issue the way the
  repo does (`Fixes #123`, `Closes`).
- **Changes** - bullet per logical change, grouped by area; each names the
  surface touched (command, flag, endpoint, type), not the file.
- **Review guide** - where to start reading, what is mechanical (renames,
  generated files) and can be skimmed, the one decision worth debating.
- **Testing** - the exact commands run and their results (paste the tail),
  plus what was NOT tested and why.
- **Risk / rollout** - migrations, config changes, feature flags, backward
  compatibility; "none" is an acceptable answer when true.

Keep it under a screen. No tool or model attribution unless the repo's own
PRs carry it.

## 4. Attach, only when asked

`gh pr create --title "<subject>" --body-file -` or `gh pr edit --body-file -`
with the text on stdin (`<<'EOF'`), or `glab mr create --description-file`.
Never merge, never push a branch the user did not ask to push. Show the final
text in the reply either way.
