# Container skeletons by stack

Companion to `omm-containers`. Every skeleton: pinned base (add `@sha256:<digest>`
from `docker buildx imagetools inspect <image>:<tag>` for anything shipped), lockfile
install before source copy, non-root final stage, exec-form entrypoint, healthcheck
using a binary the final image has. Swap the version for the repo's own pin.

## Node (native addon, so slim not alpine)

```dockerfile
# syntax=docker/dockerfile:1
FROM node:22.12.0-bookworm-slim AS build
WORKDIR /app
COPY package.json package-lock.json ./
RUN apt-get update && apt-get install -y --no-install-recommends python3 make g++ \
 && npm ci && rm -rf /var/lib/apt/lists/*
COPY . .
RUN npm run build && npm prune --omit=dev

FROM node:22.12.0-bookworm-slim
ENV NODE_ENV=production
WORKDIR /app
COPY --from=build --chown=node:node /app/package.json ./
COPY --from=build --chown=node:node /app/node_modules ./node_modules
COPY --from=build --chown=node:node /app/dist ./dist
USER node
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD ["node","-e","fetch('http://127.0.0.1:8080/healthz').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"]
ENTRYPOINT ["node","dist/server.js"]
```

`.dockerignore`: `.git`, `node_modules`, `dist`, `.env*`, `Dockerfile*`, `compose*`,
`coverage`, `*.md`.

## Python (uv, lockfile)

```dockerfile
# syntax=docker/dockerfile:1
FROM python:3.12.7-slim-bookworm AS build
COPY --from=ghcr.io/astral-sh/uv:0.5.4 /uv /usr/local/bin/uv
WORKDIR /app
ENV UV_PROJECT_ENVIRONMENT=/opt/venv
COPY pyproject.toml uv.lock ./
RUN --mount=type=cache,target=/root/.cache/uv uv sync --frozen --no-dev --no-install-project
COPY . .
RUN --mount=type=cache,target=/root/.cache/uv uv sync --frozen --no-dev

FROM python:3.12.7-slim-bookworm
ENV PYTHONUNBUFFERED=1 PATH=/opt/venv/bin:$PATH
RUN adduser --system --uid 10001 --no-create-home app
WORKDIR /app
COPY --from=build --chown=app /opt/venv /opt/venv
COPY --from=build --chown=app /app/src ./src
USER app
EXPOSE 8000
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
  CMD ["python","-c","import urllib.request,sys;sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:8000/healthz').status==200 else 1)"]
ENTRYPOINT ["python","-m","uvicorn","src.app:app","--host","0.0.0.0","--port","8000"]
```

`pip` instead of uv: `pip install --no-cache-dir -r requirements.txt` into a venv in
the build stage; copy the venv. Never `pip install` into the system interpreter.

## Go (static binary, distroless)

```dockerfile
# syntax=docker/dockerfile:1
FROM golang:1.23.4-bookworm AS build
WORKDIR /src
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod go mod download
COPY . .
RUN --mount=type=cache,target=/go/pkg/mod --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o /out/app ./cmd/app

FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /out/app /app
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 CMD ["/app","-healthcheck"]
ENTRYPOINT ["/app"]
```

Distroless has no shell and no `curl`: the binary needs its own `-healthcheck` flag
(one HTTP GET, exit 0/1), or drop the `HEALTHCHECK` and let the orchestrator probe.

## Rust

```dockerfile
# syntax=docker/dockerfile:1
FROM rust:1.83.0-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release --locked && rm -rf src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry touch src/main.rs && cargo build --release --locked

FROM debian:bookworm-slim
RUN adduser --system --uid 10001 --no-create-home app
COPY --from=build --chown=app /src/target/release/app /usr/local/bin/app
USER app
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/app"]
```

A fully static build (`--target x86_64-unknown-linux-musl`) can ship on
`gcr.io/distroless/static-debian12:nonroot` as in the Go skeleton.

## JVM

Build with the `-jdk` image (`eclipse-temurin:21.0.5_11-jdk`), run on the `-jre`
one; `jlink` when size matters. Copy the fat jar or the layered app image;
`ENTRYPOINT ["java","-XX:MaxRAMPercentage=75","-jar","/app/app.jar"]`. The image
ships user `1000`? Check with `docker run --rm <img> id -u`; else `adduser`.

## Users per base

| Base | Existing unprivileged user | Otherwise |
|---|---|---|
| `node:*` | `node` (uid 1000) | - |
| `python:*-slim`, `debian:*-slim` | none | `RUN adduser --system --uid 10001 --no-create-home app` |
| `*-alpine` | none | `RUN adduser -S -u 10001 -H app` |
| `gcr.io/distroless/*:nonroot` | `nonroot` (uid 65532) | - |
| `scratch` | none | `USER 10001` (numeric; no passwd file) |

## Healthchecks without curl

| Runtime | Command |
|---|---|
| node | `["node","-e","fetch('http://127.0.0.1:PORT/healthz').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"]` |
| python | `["python","-c","import urllib.request,sys;sys.exit(0 if urllib.request.urlopen('http://127.0.0.1:PORT/healthz').status==200 else 1)"]` |
| static binary | a `-healthcheck` flag in the binary, or `wget -qO- http://127.0.0.1:PORT/healthz` when busybox is present (alpine) |
| postgres (compose) | `["CMD-SHELL","pg_isready -U $$POSTGRES_USER"]` |
| redis (compose) | `["CMD","redis-cli","ping"]` |

## Cache mounts

`RUN --mount=type=cache,target=<dir>` keeps package caches out of layers and across
builds: `/root/.npm`, `/root/.cache/uv` or `/root/.cache/pip`, `/go/pkg/mod`,
`/usr/local/cargo/registry`, `/root/.m2`, `/root/.gradle`. Requires BuildKit (the
default in Docker 23+; `DOCKER_BUILDKIT=1` on older daemons).
