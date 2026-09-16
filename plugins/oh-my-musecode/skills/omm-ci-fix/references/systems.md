# omm-ci-fix: per-system commands and quarantine annotations

Run everything with `bash`. Bound every log read: `2>&1 | tail -n 300`, or
write to a scratch file outside the repo and `read_file` it in slices.
Where a CLI is missing, `web_fetch` the raw log URL when that tool is
present; otherwise ask the user for the failing step's tail.

## Find the run, read the log, re-run

| System | Pipeline file | Find the red run | Failing log | Re-run same SHA | Watch |
|---|---|---|---|---|---|
| GitHub Actions | `.github/workflows/*.yml` | `gh run list --branch <b> --limit 5`; `gh run list --workflow <file> --status failure` | `gh run view <id> --log-failed`; one job: `gh run view <id> --job <job-id> --log` | `gh run rerun <id> --failed` | `gh run watch <id> --exit-status` |
| GitLab CI | `.gitlab-ci.yml` | `glab ci status`; `glab ci list --status failed` | `glab ci trace <job-id>` | `glab ci retry <job-id>` | `glab ci status --live` |
| Buildkite | `.buildkite/pipeline.yml` | `bk build list --pipeline <p> --state failed` (needs `bk`) | `bk build view <n>`; raw log: web UI or API | `bk build rebuild <n>` | poll `bk build view <n>` |
| CircleCI | `.circleci/config.yml` | `circleci` CLI has no job list; use the web UI or the API | job page "Download" / API `.../output` URL via `web_fetch` | web UI "Rerun failed jobs" | none |
| Jenkins | `Jenkinsfile` | `<job>/lastFailedBuild/` | `<job>/<n>/consoleText` via `web_fetch` | "Rebuild" or `curl -X POST <job>/build` when the user allows | none |

`gh run view <id> --json jobs,conclusion,headSha` gives the SHA and per-job
conclusions without the log; `gh api repos/{owner}/{repo}/actions/runs/<id>/jobs`
lists job ids for `--job`.

Same SHA, several runs (the flaky test):
`gh run list --commit <sha> --workflow <file> --json databaseId,conclusion`.
Both `success` and `failure` present for the same job = flaky or infra, never
real.

## Reproduce CI's environment locally

- Versions the setup step printed: the `Set up <tool>` / `Install` step in
  the log. Match them before comparing results (`node --version`,
  `python --version`, `rustc -V`, `go version`).
- Env the step sets: the `env:` block at workflow, job, and step level;
  GitLab `variables:`; Jenkins `environment {}`. `CI=true` alone changes
  behaviour in many test runners (no watch mode, no colour, stricter
  warnings, `--ci` snapshot mode).
- Fresh state: `npm ci`, `pnpm install --frozen-lockfile`,
  `yarn install --immutable`, `pip install -r requirements.txt` in a new
  venv, `cargo build --locked`, `go build ./...` with `GOFLAGS=-mod=mod`
  unset.
- Container: `docker run --rm -v "$W:/w" -w /w <image> sh -c '<cmd>'`, with
  `<image>` taken from `container:` / `image:` in the pipeline file, tag
  included. Runner images such as `ubuntu-latest` have no local equivalent;
  note the gap and compare tool versions instead.
- Matrix axis: reproduce that axis only (`-p 3.12`, `--target x86_64-pc-windows-msvc`
  via cross when available). Cannot run that OS: say so; do not claim the
  axis reproduced.

## Drift: what to pin and how

| Moved | Pin |
|---|---|
| `uses: actions/setup-node@v4` | `uses: actions/setup-node@<40-char sha> # v4.0.3` |
| `image: python:3` | `image: python:3.12.6` (or `@sha256:...`) |
| `runs-on: ubuntu-latest` after a rollover | `runs-on: ubuntu-22.04` with a todo to move forward |
| toolchain resolved as "latest stable" | `rust-toolchain.toml`, `.nvmrc`, `.python-version`, `go.mod` `toolchain` |
| orb / plugin `@latest` | exact version |
| an installer that rewrote the lockfile | `--frozen-lockfile` / `--immutable` / `--locked` / `npm ci` |

State old -> new in the message. One pin per moved thing; do not pin
everything in the file while you are there.

## Quarantine annotations

The reason string carries the rate and the issue. Nothing else counts as a
quarantine.

| Framework | Annotation |
|---|---|
| pytest | `@pytest.mark.skip(reason="flaky 3/20, #412")` |
| Jest / Vitest | `it.skip("...")` with `// flaky 3/20, #412` on the line above |
| Go | `t.Skip("flaky 3/20, #412")` as the first line of the test |
| Rust | `#[ignore = "flaky 3/20, #412"]` |
| JUnit 5 | `@Disabled("flaky 3/20, #412")` |
| RSpec | `skip "flaky 3/20, #412"` inside the example |
| Swift Testing / XCTest | `.disabled("flaky 3/20, #412")` trait / `throw XCTSkip("flaky 3/20, #412")` |

Never a framework-level retry (`jest-retry`, `pytest-rerunfailures`,
`flaky` decorator, `retry: 2` in the pipeline) as the quarantine: it hides the
rate and has no expiry.

## Infra signals worth memorising

| Log line | Class |
|---|---|
| `The runner has received a shutdown signal`, `lost communication with the server` | infra, re-run once |
| `exit code 137`, `Killed`, `OOMKilled` | infra (memory) unless a new test allocates; check the diff first |
| `No space left on device` | infra; caches or artifacts grew, prune them |
| `429 Too Many Requests`, `503` from a registry or mirror | infra; cache the dependency or pin a mirror |
| `Resource not accessible by integration`, `Bad credentials` on a fork PR | expected on forks; make the step conditional |
| `Error: Unable to resolve action ... repository not found` | drift or infra: action renamed, or GitHub outage; check the action's repo |
