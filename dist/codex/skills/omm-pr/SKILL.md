---
name: "omm-pr"
description: "Open a pull request (open a PR, make a PR, ready for review): screen diff, push, imperative title, what/why/tested/risks body, issue link, checklist, CI green first; Do not use when only commits (omm-commit) or body text are wanted."
---

# Open a pull request

Contract: a PR is ready for review only when the branch is pushed, the head
commit's CI is green and pasted, the title is one imperative sentence, and
the body says what changed, why, how it was proven, and what could break.
bundled:git decides WHETHER the push may happen; this skill decides what the
PR carries. "Open a PR" names the push of the current branch and nothing
else: never `--force`, never another branch, never a base you guessed.
Commit shaping is omm-commit's; a body for a PR already open is plain writing.
Run every command with `bash`. `write_todos`: one item per step below.

## 1. Preflight

- `git status --porcelain`. Dirty: stop; the uncommitted hunks are
  omm-commit's job. Never `git stash` around them.
- `git rev-parse --abbrev-ref HEAD`. On `main`/`master` itself: stop, a PR
  needs a branch.
- Base: the branch the user names, else `gh repo view --json
  defaultBranchRef -q .defaultBranchRef.name`, else `main`. `git fetch
  origin <base>`. Conflicts: report; rebase only on the ask. Merely behind
  is fine.
- `git log --oneline origin/<base>..HEAD`. Every line must read as one step
  of ONE change. A `wip`, `fixup`, or `address comments` line on an unpushed
  branch earns an offer to re-split (omm-commit); do it only on the ask.
- `gh pr view --json number,isDraft,url`. A PR exists: update it (`gh pr
  edit`), never open a second.
- Run omm-verify's checklist now (suite, lint, diff scope after the last
  edit). Its evidence block is the body's How tested section; without it
  there is no PR.
- No `gh` (GitLab: `glab`): do not install silently; draft title and body
  anyway and use the no-CLI fallback in `references/titles.md`.

## 2. Screen the whole diff

`git diff origin/<base>...HEAD`, every hunk, before writing a word. Then
`git diff origin/<base>...HEAD | grep -nE -- '-----BEGIN|AKIA[0-9A-Z]{12}|ghp_|password=|secret='`.
Remove by a new commit on the branch (never `--amend` on a pushed one):

- Secrets: `.env*`, `*.pem`, `id_rsa*`, `credentials*`, the patterns above, a
  URL with credentials. Already pushed: compromised, rotation required, and a
  follow-up commit does not undo it. No PR until rotated.
- Generated junk: lockfile churn no dependency change explains, `dist/`,
  `coverage/`, `.DS_Store`, snapshots regenerated without intent, whole-file
  reformats.
- Debug residue: prints you added, `TODO remove`, commented-out code,
  `<<<<<<<` markers.
- Scope creep: a hunk the title will not cover. Its own PR, or named in the
  body as deliberate.

## 3. Title, 70 characters or fewer

Imperative, completes "This PR will ...", no trailing period. Prefix or
ticket only in the shape the last 20 merged titles use (`gh pr list --state
merged --limit 20 --json title -q '.[].title'`); invent none. One commit:
its subject. Several: the outcome, not the commits. `Some auth
improvements` makes no claim a reviewer can check; `Cache JWKS keys per
issuer` does. More pairs: `references/titles.md`.

## 4. Body

`read_file` the template first: `.github/PULL_REQUEST_TEMPLATE.md` (or the
`PULL_REQUEST_TEMPLATE/` dir, `.gitlab/merge_request_templates/`). Fill
every section, headings and checkboxes verbatim, delete none. No template:
these, under one screen.

- **What**: two to four sentences, the behaviour as a user sees it. Not the
  commit list, not the file list.
- **Why**: the bug, limit, or request that forced it; each rejected
  alternative in one line, so the reviewer does not re-propose it.
- **How tested**: omm-verify's block: command, decisive line, exit code.
  Then what was NOT tested and why. "Tests pass" alone is not a section.
- **Risks / rollout**: migration, config, flag, compatibility, revert plan.
  "None" when true, never when unexamined.
- **Issue**: from the user's text, the branch name (`fix/123-...`) or commit
  trailers; confirm with `gh issue view <n> --json title`. `Fixes #N` only
  when merging closes it entirely; `Refs #N` otherwise, in the keyword the
  repo's merged PRs use. No issue: no line; never invent one.
- **Reviewer checklist**: three to six boxes, each a place and a question:
  `- [ ] src/auth/jwks.rs:40 - key is (issuer, kid); can tenants collide?`.
  Never `- [ ] code is clean`.
- **UI change**: one capture per state (before/after, empty, error) into a
  scratch dir outside the repo; `read_file` each image, it must show what
  you claim. `gh` cannot upload images: list the paths under
  `## Screenshots` for the user to drop in. Never commit them.

No tool or model attribution unless the repo's merged PRs carry it.

## 5. Push and open

- `git push -u origin HEAD`. The current branch to its own name, once.
- `--draft` when the user asked for early visibility, the change is
  knowingly incomplete, or nobody should be notified before CI has run.
- Body from stdin so newlines survive; never `--fill` (it skips step 4):
  ```
  gh pr create --base <base> --title "<title>" --body-file - <<'EOF'
  <body>
  EOF
  gh pr view --json url --jq .url
  ```
  Existing PR: `gh pr edit <n> --title "<title>" --body-file -`. GitLab
  (`glab mr create`) and the no-CLI fallback: `references/titles.md`.
- Reviewers, labels, assignees only when the user named them; CODEOWNERS
  already assigns. Never guess a reviewer.

## 6. CI before requesting review

- `gh pr checks <n> --watch --fail-fast; echo exit=$?` with `yield_time_ms`
  300000. The output arrives on its own; do not poll with `bash_input`.
  Still pending after the wait: report "CI pending", never "green".
- `no checks reported`: `gh run list --branch <branch> --limit 3`, retry
  once; the workflow may not have registered. Truly none: say so; How
  tested carries the local run.
- Red: `gh run view <id> --log-failed`, read the FIRST real failure. Your
  change: fix behind a test (omm-tdd), new commit, push, re-watch. A flake:
  `gh run rerun <id> --failed` once, and say so; red twice is a failure.
- Green: paste the `gh pr checks` lines verbatim. Draft: `gh pr ready`.
  Named reviewers: `gh pr edit --add-reviewer <user>`. Only then say
  "ready for review".

## Judgment calls

- "Ready for review" with a draft PR: steps 1, 2, 6, then `gh pr ready`.
  With no PR: ask in one line before pushing; the phrase alone does not
  name a push.
- Two unrelated changes on one branch: offer two PRs before opening one.
- Base is not the default branch (release, stacked): `--base` explicit and
  the body's first line says why.
- Revert: title `Revert "<original>"`; body names the failure and the plan
  to re-land.
- `gh auth status` fails: report it; never paste a token.

## Refuse

- Opening from a dirty tree, from `main` itself, or a branch you did not
  diff in full.
- Titles `Fix`, `Update`, `WIP`, `Changes`, `Misc`, the branch name, a
  ticket id alone, or anything ending in `...`.
- `--fill`, `--force`, `--force-with-lease`, amend or rebase of a pushed
  ref, `[skip ci]`, disabling or `continue-on-error` on a failing check,
  `gh pr merge` or auto-merge. Merge is a separate request.
- Requesting review on red, pending, or unreported CI, unless the user
  waives it in so many words; then the body opens with "CI pending".
- A body that narrates the diff, or claims tests pass without the run.

## Micro-example

Branch `fix-jwks-cache` on `main`, tree clean, no template, no PR yet. The
diff shows a leftover `eprintln!` in `src/auth/jwks.rs`: new commit on the
user's ask. omm-verify: `cargo test -p auth` -> `27 passed`, exit 0. Merged
titles are plain imperative: `Cache JWKS keys per issuer`. Push; `gh pr
create` with What (JWKS cached per issuer for 10 min), Why (the IdP
rate-limited us; a process-wide cache rejected, tenants share a process),
How tested (the pasted line; rotation inside the TTL not tested), Risks
(stale key at most 10 min, revert safe), `Fixes #412`, checklist item
`src/auth/jwks.rs:40 - key is (issuer, kid); can tenants collide?`.
`gh pr checks 87 --watch --fail-fast` -> `test pass 1m12s`, `lint pass 38s`,
`exit=0`, pasted. No reviewer named. Report the URL. Done.
