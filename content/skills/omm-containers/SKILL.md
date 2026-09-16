---
name: omm-containers
description: "Write or fix a Dockerfile or compose file (containerise it, run it in docker): pinned slim base, multi-stage, non-root, .dockerignore, healthcheck, runtime secrets, build and run pasted; Do not use when the task is a cloud deploy pipeline."
---

# Containerise

Contract: the image is done only when your final message pastes, verbatim from
`bash` with `echo exit=$?` after each: the build tail, the image size, the uid it
runs as, the healthcheck reaching `healthy`, and for compose `docker compose up
--wait`. "It builds" is not done. Root or a secret in a layer is a bug, not a style
choice. Over two files to touch: `write_todos`, one item per file.

## 0. Decide before typing

- Runtime: `command -v docker podman; docker compose version` with `bash`. None:
  write the files, say every run below is unverified. `podman` takes the same
  Dockerfile, flags and compose file.
- Existing files: `search` (`glob: ["**/Dockerfile*", "**/compose*.y*ml",
  "**/docker-compose*.y*ml", "**/.dockerignore"]`, `pattern: "^"`, `mode: "regex"`,
  `output_mode: "files_with_matches"`); `read_file` each. One Dockerfile per
  service: fix it in place, keep its base family, stage names and port unless they
  break a section 1 rule; never add a second.
- What runs: `read_file` the manifest (`package.json`, `pyproject.toml`, `go.mod`,
  `Cargo.toml`) for the start command and the version pin the repo already has
  (`.nvmrc`, `.python-version`, `rust-toolchain.toml`, `engines`). The image runs
  THAT version, never one from memory. Port: `search` for `listen(` or `PORT`.
- Stages: two when a toolchain builds what a smaller runtime runs (Go, Rust, JVM,
  TS, native deps); one when nothing compiles, still pinned and non-root.
- Base: Debian `-slim` by default. Alpine only when the repo already runs musl with
  no native addon. `distroless`/`scratch` for a static binary. Full `-bookworm`,
  `-jdk`, `-sdk` images: build stage only.
- Per-stack skeletons, healthchecks without curl, users per base, cache mounts:
  `references/stacks.md` beside this file (directory from the `locator:` line of
  the read_skill result).

## 1. Dockerfile rules

| Rule | Not this | This |
|---|---|---|
| Pin the base | `FROM node`, `node:22`, `:latest` | `FROM node:22.12.0-bookworm-slim`, plus `@sha256:<digest>` (from `docker buildx imagetools inspect <image>:<tag>`) for anything shipped |
| Order layers by change rate | `COPY . .` then install | `COPY package.json package-lock.json ./`, install, then `COPY . .` |
| Install from the lockfile | `npm install`, `pip install -r requirements.txt` | `npm ci --omit=dev`, `uv sync --frozen --no-dev`, `cargo build --locked`, `go mod download` after `COPY go.sum` |
| Clean in the same layer | `RUN apt-get update`, `RUN apt-get install x` | `RUN apt-get update && apt-get install -y --no-install-recommends x && rm -rf /var/lib/apt/lists/*` |
| Non-root before CMD | no `USER`, `USER root` in the final stage | `USER node` / `USER 10001` after `COPY --chown`; `distroless:nonroot`; port above 1024 |
| `.dockerignore` beside the Dockerfile | none: `.git`, `.env`, `node_modules` copied | `.git .env* node_modules target dist **/__pycache__ *.log Dockerfile* compose*`; a context over a few MB has a leak |
| Exec form | `CMD npm start` (a shell is PID 1, SIGTERM lost) | `ENTRYPOINT ["node","src/server.js"]`, `CMD` for default args; `--init` or `tini` when it forks |
| Healthcheck | none on a long-running service | `HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 CMD [...]` on the real readiness path, using a binary the image has |
| `ENV` is public | `ENV API_KEY=...` | `ENV` only for non-secret defaults (`NODE_ENV=production`, `PYTHONUNBUFFERED=1`) |
| Reproducible steps | `curl ... \| sh`, `ADD <url>`, `apt-get upgrade` | versioned packages, checksummed downloads, `# syntax=docker/dockerfile:1` |

## 2. Secrets: build time is not run time

- Build needs a registry token or an SSH key: `RUN
  --mount=type=secret,id=npmrc,target=/root/.npmrc npm ci` and `docker build
  --secret id=npmrc,src="$HOME/.npmrc"`; `--ssh default` for git. Never `ARG TOKEN`,
  never `COPY .env`: both survive in `docker history` and the layer tarball.
- Run needs a database URL or API key: `-e KEY`, `--env-file` (git-ignored), or
  compose `env_file:`/`secrets:`. The Dockerfile never knows the value.
- Prove: `docker history --no-trunc <img> | grep -iE 'token|secret|passw'` prints
  nothing; `docker run --rm <img> env` shows no credential.

## 3. Compose for local multi-service

- File `compose.yaml`, no `version:` key. One service per process: the app `build:
  {context: ., target: <stage>}`, dependencies as pinned images (`postgres:16.6`),
  each with a `healthcheck:` (`pg_isready -U $$POSTGRES_USER`, `redis-cli ping`),
  and the app `depends_on: <svc>: condition: service_healthy`.
- Ports on loopback: `"127.0.0.1:8080:8080"`, never `0.0.0.0`. Data in named
  volumes, never in the image or a bind mount of the repo.
- Inline credentials only when obviously dev (`POSTGRES_PASSWORD: dev`); real values
  in a git-ignored `.env`. Hot reload, debugger ports, source bind mounts:
  `compose.override.yaml`, never shipped.
- Prove: `docker compose config -q`, then `docker compose up -d --wait; echo
  exit=$?`, `docker compose ps`, one request through the app to the db, `docker
  compose down` (`-v` deletes data volumes: only when asked).

## 4. Build, run, prove

With `bash`, `yield_time_ms` 300000, each followed by `echo exit=$?`:

1. `hadolint Dockerfile` (else `docker run --rm -i hadolint/hadolint < Dockerfile`);
   not installed: report `hadolint: not run`.
2. `docker build --pull -t app:dev . 2>&1 | tail -n 40; echo exit=${PIPESTATUS[0]}`.
   BuildKit warnings (`SecretsUsedInArgOrEnv`, `JSONArgsRecommended`) are findings.
   Build fails: the first error is the cause; `--progress=plain` for the full log.
3. `docker image inspect app:dev --format '{{.Config.User}} {{.Size}} {{if
   .Config.Healthcheck}}healthcheck{{else}}NO-HEALTHCHECK{{end}}'`. Too fat:
   `docker history app:dev` names the layer.
4. `docker run --rm app:dev id -u` -> not `0` (no shell: `docker top t`, UID column).
5. `docker run -d --name t --init -p 127.0.0.1:8080:8080 app:dev` (`--env-file
   .env.example` when the app needs config); `for i in $(seq 30); do s=$(docker
   inspect -f '{{.State.Health.Status}}' t); [ "$s" = healthy ] && break; sleep 1;
   done; echo "health=$s"`; one real request; `docker logs t | tail -n 20`; `time
   docker stop t` under 2 s (longer: shell-form CMD or SIGTERM ignored); `docker rm
   -f t`. Not healthy: `docker inspect t --format '{{json .State.Health.Log}}'`.
6. The section 2 secret grep. `trivy image app:dev` or `docker scout cves app:dev`
   when installed (a base CVE: bump the pinned tag, not a patch layer); else say
   not scanned.

Final message: files written or changed; base and why; stages; uid; size; every run
pasted; hadolint and scan lines or their absence; what was not run and why. A trap
others will hit (proxy, platform, registry): `add_memory`, one line.

## Judgment calls

- Native deps (`better-sqlite3`, `psycopg`, `sharp`): build tools in stage one only;
  the runtime keeps the shared lib (`libpq5`), not `-dev`.
- No shell in the runtime image: debug with `docker run --rm -it --pid=container:t
  --network=container:t busybox`; never add one to production for comfort.
- Digest pin for anything shipped; tag pin for local dev; either way one bump
  mechanism (renovate, dependabot) or it rots.
- Monorepo: context is the repo root; `.dockerignore` excludes sibling packages.
- Apple silicon host, amd64 target: `--platform linux/amd64` at build and run; a
  native build that passes proves nothing.
- Working but wrong Dockerfile: root, `:latest`, secrets first; layer order second;
  size last. Do not rewrite unasked. A one-shot CLI image needs no `HEALTHCHECK`.

## Refuse

- `:latest`, a bare major tag, or an untagged `FROM`; no digest on a shipped image.
- No `USER` or `USER root` in the final stage; `--privileged`; the docker socket in
  an app container; `network_mode: host` "for convenience".
- A credential in `ARG`, `ENV`, `COPY`, a committed `.env`, or a compose file.
- `COPY . .` with no `.dockerignore`; `curl | sh` or `ADD <url>`; `apt-get upgrade`;
  `VOLUME` in a Dockerfile (anonymous volumes leak).
- "It builds" or "it runs" without the build, uid, size, and `healthy` pasted.
