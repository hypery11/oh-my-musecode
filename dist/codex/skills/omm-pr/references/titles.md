# Titles that pass, and the fallbacks when `gh` is not there

Companion to omm-pr. Load with `read_file` when a title will not come out
under 70 characters, or when the repo is not on GitHub.

## Title pairs

| Draft | Problem | Passing |
|---|---|---|
| `Fix bug` | says nothing | `Reject whitespace-only usernames at signup` |
| `PROJ-412` | reviewer must click through | `[PROJ-412] Retry webhook delivery with backoff` |
| `Update parser, add tests, fix lint` | three PRs | split; or `Parse quoted trigger phrases` (tests ride along) |
| `Some auth improvements` | no claim to review | `Cache JWKS keys per issuer` |
| `Refactoring` | no outcome | `Extract JWKS fetch into a client type` |
| `Fixed the thing from yesterday` | past tense, no referent | `Restore retry on 503 from the events API` |
| `Add feature (WIP)` | draft state in the title | title the finished change; `--draft` carries the state |
| `fix-jwks-cache` | the branch name | `Cache JWKS keys per issuer` |

Measure: `printf '%s' "<title>" | wc -c`. Over 70: drop qualifiers before
nouns (`the`, `some`, `new`, `basic`), never the verb or the object.

## Opening the request without `gh`

- GitLab:
  ```
  glab mr create --target-branch <base> --title "<title>" --description-file - <<'EOF'
  <body>
  EOF
  ```
  Draft: `--draft`. Checks: `glab ci status --live`. Ready: `glab mr update <n> --ready`.
  Reviewer: `glab mr update <n> --reviewer <user>`.
- No CLI at all: `git push -u origin HEAD`, then print the compare URL and
  the title and body for pasting. Build the URL from
  `git remote get-url origin`:
  - GitHub: `https://github.com/<owner>/<repo>/compare/<base>...<branch>?expand=1`
  - GitLab: `https://gitlab.com/<group>/<repo>/-/merge_requests/new?merge_request[source_branch]=<branch>&merge_request[target_branch]=<base>`
  Say plainly that the PR is not open until the user submits that form, and
  that CI could not be watched from here.

## Reading a red check

- `gh pr checks <n>` lists every check with state and URL; `gh run list
  --branch <branch> --limit 5` gives run ids.
- `gh run view <id> --log-failed` prints only failed steps. The first
  failure is the cause; later ones are usually fallout.
- `gh run rerun <id> --failed` reruns failed jobs only. Once. Say it was a
  rerun in the PR body's How tested section if it went green the second time.
