---
name: omm-dep-upgrade
description: Upgrade or bump one dependency (bump the version, update the package, dependabot PR, outdated crates) with the lockfile, changelog delta and a green build proven; Do not use when adding a new dependency or when the user only wants the current version reported.
metadata:
  triggers: upgrade, bump, bump the version, dependency upgrade, update the dependency, dependabot, cargo update, npm update, pip upgrade, outdated, newer version, go get -u
---

# Dependency upgrade

Goal: one dependency moved to one target version, with the reason the change
is safe written down, not assumed. Output: the manifest and lockfile diff, the
upstream changes that matter, the proof commands and their results.

## 1. Pin the target

- Read the manifest and lockfile entries with `search` (`Cargo.toml` +
  `Cargo.lock`, `package.json` + the lockfile, `pyproject`/`requirements` +
  lock, `go.mod` + `go.sum`). Record current version, requested range, and
  whether the dependency is direct or transitive. A transitive one is bumped
  through its parent or the lockfile only; do not add it to the manifest.
- Target = what the user named; if they said "latest", resolve it with the
  ecosystem's tool (`cargo search`/`cargo info`, `npm view <pkg> version`,
  `pip index versions`, `go list -m -versions`) and state it back.

## 2. Read the delta before changing anything

- Upstream changelog or release notes between the two versions: `bash` with
  `curl` to the project's CHANGELOG or releases page when network is allowed;
  otherwise the vendored source under the lock's cache directory. List every
  BREAKING or deprecation line that touches an API this repo uses (`search`
  for the identifiers).
- Major version jump: expect breaking changes and say so up front; a minor or
  patch jump with a breaking line is still breaking.

## 3. Apply

- One dependency per change. Use the tool, not a hand edit of the lockfile:
  `cargo update -p <crate> --precise <ver>` (and the manifest range if it
  must widen), `npm install <pkg>@<ver>`, `pip-compile`/`uv lock --upgrade-package`,
  `go get <mod>@<ver> && go mod tidy`.
- Fix compile or type errors from the breaking lines only; no drive-by
  refactors.

## 4. Prove

Run the build and the tests that exercise the dependency (find them with
`search` for its identifiers), paste the tail of the output with its exit
status. Report: old -> new, the lockfile lines that changed (count), the
breaking items handled, anything deferred. Never claim "no breaking changes"
without having read the changelog.
