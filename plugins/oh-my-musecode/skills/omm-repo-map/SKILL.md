---
name: omm-repo-map
description: Existing repo, first session (how does this project work, set me up): run build, test, lint, run commands; write proven ones and conventions to AGENTS.md; Do not use when scaffolding a new project (bundled:greenfield-project-scaffolding).
---

# Repo map

Contract: a command reaches `AGENTS.md` only after it ran in this session and your
final message pastes its decisive line and exit code. A README command is a rumour; a
`muse init` template is a detector guess. You write two things: the project rules file
and, for machine-local facts, memory. Nothing else changes: never fix code, config or
toolchain to make a command pass; no commit (bundled:git). A directory with no manifest
and no source is a scaffolding request, not a repo: stop and say so.

## 0. Orient, in ten minutes, not the whole repo

- `write_todos`: layout, commands, verify, conventions, write, report.
- Rules files first: `search` with `glob: ["**/AGENTS.md", "**/CLAUDE.md"]`,
  `pattern: "^"`, `mode: "regex"`, `output_mode: "files_with_matches"`; `read_file`
  each hit. Muse loads `AGENTS.md` from the VCS root down to cwd, deeper wins;
  `CLAUDE.md` only as a same-directory fallback, so writing `AGENTS.md` beside one
  shadows it: fold its content in and say so. An `AGENTS.md` exists: this is an
  update pass; keep its headings; every command it lists is re-run in step 2.
- Shape, with `bash`: `git ls-files | cut -d/ -f1 | sort | uniq -c | sort -rn | head
  -20`; `git log --oneline -30`. Manifests: `search` (`glob`, excluding
  `!**/node_modules/**`, `!**/vendor/**`, `!**/target/**`) for the names in the table
  below; `read_file` each hit. README: headings, then only install and run.

## 1. Find the commands; CI is the source of truth

Trust order: the pipeline (`.github/workflows/*.yml`, `.gitlab-ci.yml`, `Jenkinsfile`,
`.circleci/config.yml`), the task runner (`Makefile`, `justfile`, `package.json`
scripts, `tox.ini`, `noxfile.py`), README, then the framework default. A README
command CI does not run is documentation, not the build.

| manifest | install / build | test | lint / format check |
|---|---|---|---|
| `package.json` | `npm ci`; `scripts.build` | `scripts.test` | `scripts.lint`; `npx prettier --check .` |
| `Cargo.toml` | `cargo build` | `cargo test` | `cargo clippy -- -D warnings`; `cargo fmt --check` |
| `pyproject.toml` | `uv sync` or `pip install -e .` in `.venv` | `pytest` | `ruff check .`; `ruff format --check .` |
| `go.mod` | `go build ./...` | `go test ./...` | `go vet ./...`; `gofmt -l .` |
| `Package.swift` | `swift build` | `swift test` | `swiftformat --lint .` |
| `pom.xml`, `build.gradle*` | `mvn -q compile`, `./gradlew assemble` | `mvn -q test`, `./gradlew test` | spotless, checkstyle when configured |

- Per purpose, ONE command: install, build, test, lint, format-check, typecheck, run.
- Toolchain: the pin (`.nvmrc`, `engines`, `rust-toolchain.toml`, `requires-python`,
  `.tool-versions`, CI's setup step) against the installed version (`node --version`,
  `cargo --version`). A mismatch is the first fact worth writing.
- Python: `read_skill bundled:python-env` before creating any environment.

## 2. Verify by running, one command at a time

- Install first, the project's way, into the project (`npm ci`, `uv sync`, `cargo
  fetch`). Never `-g`, `sudo`, a system package manager, or a shell-profile edit; a
  missing toolchain is reported with its install line, not installed.
- Each command in `bash` as `<cmd>; echo exit=$?`, `workdir` = the manifest's
  directory, `yield_time_ms` up to 300000. One-shot only: no watch mode. Record argv,
  exit code, wall time, one decisive output line (`41 passed`).
- Tests: the full suite once. Pre-existing failures by name, not fixed; they go into
  the file as known red. Over ten minutes: the subset CI's fastest job runs, the full
  command tagged `(full suite ~N min, not run here)`.
- Lint and format in check mode. Red on a clean tree means the rule is not enforced:
  a convention fact, not something to fix.
- Run: start it, wait for the ready line or port, one request, stop it (`timeout 20
  <cmd>` when it will not exit). Never leave a process behind.
- Red: fix the invocation (cwd, env var, prerequisite), not the repo. Two attempts,
  then record `(unverified: <first error line>)` and move on.
- Never run what deploys, publishes, tags, migrates a shared database, or costs money.
  Study them; write them tagged `(not run: <reason>)`.

## 3. Conventions from evidence, not prose

- Commits: `git log --format=%s -40`. Count the prefix pattern (`feat(scope):`, ticket
  id, none), mood, length, trailers. What 37 of 40 commits do is the convention; what
  CONTRIBUTING.md says and the log ignores is aspiration. Keep the first, note the second.
- Layout by example: `git log --diff-filter=A --name-only --format= -40 | sort -u`.
  Where a test lives relative to its source; where a new module goes.
- Formatting: the config that exists (`.editorconfig`, `.prettierrc*`, `rustfmt.toml`,
  `[tool.ruff]`) and whether step 2 showed it enforced.
- Generated files: `search` for `Code generated`, `@generated`, `DO NOT EDIT`. Name the
  path and the generator command; a hand edit there is the classic trap.
- Two idioms compete: count with `search`, write the winner, name the loser as legacy.

## 4. Draft the rules file

- Target: the existing rules file (`edit_file` under its own headings; never a
  wholesale rewrite; never delete a line you did not disprove by running it). None:
  `write_file` `AGENTS.md` at the repo root, or at the package root in a monorepo when
  the work lives there (it loads on top of the root, never instead).
- It is inlined into every turn: under 60 lines, about 3 KB. Only what a fresh agent
  would get wrong: one command per purpose with cwd and env, the toolchain pin,
  known-red tests, generated paths, the commit convention, the placement rule for new
  code, the trap you hit. Not a directory tour, not the README again, never a secret
  or a machine path.

```markdown
## Commands (verified 2026-09-05 at 3f2a9c1)
- install: `pnpm install --frozen-lockfile` (node 20 per .nvmrc)
- test: `pnpm -r test -- --run` (~75 s; `api/db.test.ts` red on main, #412)
- lint: `pnpm -r lint`; format: `pnpm exec prettier --check .` (not enforced)
- run: `pnpm --filter api dev` -> http://localhost:3000/health
## Conventions
- Commits: `type(scope): subject`, imperative, <= 72 chars, `Refs #n` trailer
- Tests sit beside source as `*.test.ts`; new routes under `packages/api/src/routes/`
- `packages/api/src/db/schema.generated.ts` is generated: `pnpm db:generate`, never edit
```

- Machine-local facts (installed versions, a proxy): `add_memory`, scope
  `personal_project`, one line each; never the shared file.
- Muse warned the workspace is untrusted: rules files do not load this session. Put
  it anyway; tell the user `omm trust .` or `--trust-workspace`.

## 5. Report

Every command with its decisive line and exit code; each convention with its evidence
(the count, the config path); `git diff --stat`; unverified and not-run items with the
reason; drift found (README vs CI, CONTRIBUTING vs log, pin vs installed). `git status
--porcelain` shows `AGENTS.md` plus install output, nothing else. No summary of what the
project is for; the reader owns it.

## Judgment calls

- "How does this project work" with no setup ask: same discovery, the reply is the
  map, offer the file write in one line. "Set me up" or a first session: write it.
- Monorepo: the root once, then one package: the one named, else the most recently
  touched (`git log -20 --name-only --format=`).
- No CI, no runner: framework default, run it, write `no CI; commands derived by
  running them on <date>`. Docker-only build: only if `docker info` succeeds.
- Existing file contradicts a run: the run wins, change the line, say so.
- 4+ packages: `subagent_spawn` read-only children in the shared checkout (no
  `worktree_isolation`), one per package, discovery only (steps 0, 1, 3). Step 2 stays
  with you; parallel builds fight over caches.

## Refuse

- An untagged command in the file that did not run this session with exit 0.
- Fixing red tests, formatting the tree, bumping dependencies while mapping.
- Global installs, `sudo`, editing `~/.zshrc` or `~/.bashrc`.
- Leaving a dev server, watcher, or container running.
- A rules file over ~3 KB, one that restates the README, or any line without a source.
