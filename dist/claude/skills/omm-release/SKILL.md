---
name: "omm-release"
description: "Cut a release (release, cut a version, tag vX.Y.Z): clean tree, green gate, version bumped everywhere, CHANGELOG dated, tag, artifacts, checksums, notes, clean install proven; Do not use when only the changelog prose is wanted (omm-docs)."
---

# Cut a release

Done means: tag pushed, artifacts and `SHA256SUMS` attached, notes published,
and an install where nothing is cached printed exactly `<x.y.z>`. Every step
pastes its command and decisive line; a step not run goes under "Not done".
Each step gates the next; any red stops the release, and a release fixes
nothing (omm-debug, then back to step 0). bundled:git governs the writes:
"release vX", "cut vX", "tag vX" name the version commit, the tag and the
push; "bump the version" or "prepare the release" name none of them - stop
after step 4 with the edits uncommitted. Before the first write, list in one
line the writes this release makes (commit, tag, push, release, registry).
A registry publish is irreversible and happens only when the request or the
repo's release script names it. `write_todos`: one item per numbered step.

## 0. Preconditions

`bash`, each as `<cmd>; echo exit=$?`:

- `git status --porcelain --untracked-files=all` prints nothing. Anything
  else: stop. Never stash, discard, or commit to make the tree clean.
- `git fetch origin` then `git status -sb`: on the branch the repo releases
  from (`main`, `master`, `release/*`), neither ahead nor behind.
- `git describe --tags --abbrev=0` is `<prev>`; `git log --oneline
  <prev>..HEAD` is non-empty. Empty: nothing to release; say so and stop.
- `search` for the repo's own procedure: `RELEASING*`, `release` in
  `Makefile`, `justfile`, `package.json` scripts, `release.sh`,
  `.goreleaser*`, `cliff.toml`, `.bumpversion*`, a tag-triggered workflow.
  One exists: it IS the procedure. `read_file` it, run its steps; this skill
  covers only what it skips (gate, checksums, clean install).

## 1. Gate

The exact commands CI runs (`search` the CI config, `Makefile`, `justfile`
for `test`, `lint`, `typecheck`, `build`), locally, after a clean build
(`cargo clean`, `rm -rf dist build`); `bash` with `yield_time_ms` up to
300000, each as `<cmd>; echo exit=$?`. Paste every summary line. `0 tests
ran`, skipped tests, a cached build: not green. Any red: stop.

## 2. Version

- Scheme from `git tag --list --sort=-v:refname | head -5`: prefix (`v` or
  none, never both), semver or calver. Keep it.
- Bump from the commits and the Unreleased section, not the request alone:
  `!:` or `BREAKING`, a removed flag, a changed default -> major (minor
  while 0.x); new surface -> minor; fixes only -> patch. The user named a
  smaller bump than the diff needs: say so once, then follow the user.
- `git tag -l <tag>` must print nothing. It does: stop; never `-f`.

## 3. Bump everywhere the version lives

- `search` the previous version string (literal, `hidden: true`,
  `glob: ["!CHANGELOG*"]`). It lives in the manifest, the lockfile, often a
  `__version__`/`VERSION` file, `plugin.json`, `action.yml`, `CITATION.cff`,
  Dockerfile labels, `openapi.*` and README install lines (`@1.1.0`, a
  `/v1.1.0/` in a curl URL). Homes per ecosystem: `references/ecosystems.md`.
- Every hit is a decision: bump it, or name why it stays (a fixture, a
  "removed in" note). Prefer the ecosystem's bump tool (`cargo set-version`,
  `npm version --no-git-tag-version`, `poetry version`); else `edit_file`
  per hit. Lockfiles through their tool, never by hand (`cargo check`,
  `npm install --package-lock-only`, `uv lock`).
- Re-run the `search`: every remaining hit is one you named. Rebuild; the
  only lockfile change is the package's own version line. Then `<bin>
  --version` (or `node -p "require('./package.json').version"`, `python -c
  "import <pkg>; print(<pkg>.__version__)"`); paste the new number.

## 4. CHANGELOG

- `read_file` the head; keep the file's format. `## [Unreleased]` becomes
  `## [<x.y.z>] - $(date -u +%F)`, never a guessed date; an empty
  Unreleased goes above; compare links at the foot follow if present.
- Every commit in `git log <prev>..HEAD --format='%h %s'` maps to a line or
  a deliberate skip (refactor, CI, formatting). Line prose: omm-docs
  section 5; release-notes checklist: omm-docs section 7. No CHANGELOG:
  say so, step 7 takes its notes from the commit list; create one only if
  asked.

## 5. Commit and tag

- `git add <manifests> <lockfile> CHANGELOG.md <doc hits>`; `git diff
  --cached --stat` shows only step 3 and 4 paths. Subject shaped like the
  previous bump (`git log --oneline -1 <prev>`); `git commit -F -` with a
  heredoc. Re-run the tests that assert the version string.
- Annotated, never lightweight; signed when `git config tag.gpgSign` is
  true or `git tag -v <prev>` shows a signature: `git tag -a <tag> -m
  "<tag>"`. Confirm with `git describe --exact-match HEAD`. Nothing is
  pushed yet: a failure in step 6 deletes the local tag (`git tag -d`) and
  fixes forward.

## 6. Artifacts and checksums

- Build from the tag, not the working tree: `W=$(mktemp -d); git worktree
  add -q "$W" <tag>`; clean build inside `$W` with the repo's recipe and the
  workflow's target matrix, into `$W/dist`. Names follow the previous
  release (`gh release view <prev> --json assets -q '.assets[].name'`; else
  `<name>-<version>-<os>-<arch>.<ext>`). Recipe in a tag-triggered
  workflow: build nothing locally; step 7 downloads instead.
- `cd "$W/dist" && shasum -a 256 * > SHA256SUMS && shasum -a 256 -c
  SHA256SUMS` (`sha256sum` on Linux). One file, bare filenames, computed
  after the final build, never copied forward. Each binary once: `--version`
  prints `<x.y.z>`. Sign only as the previous release did (`cosign`, `gpg
  --detach-sign`, `minisign`) or not at all. `git worktree remove "$W"`.

## 7. Push and publish

- `git push origin <branch>`, then `git push origin <tag>` (a tag pushed
  first runs CI on an orphan). A pushed tag is never moved or deleted; a
  wrong release gets the next patch number.
- Tag workflow present: `gh run watch` to the end, then `gh release
  download <tag> -D <dir>` and verify its `SHA256SUMS` before publishing.
- Notes = this version's CHANGELOG section:
  `awk '/^## \[<x.y.z>\]/{p=1;next} /^## \[/{p=0} p' CHANGELOG.md`.
  `gh release create <tag> <artifacts> SHA256SUMS --verify-tag --title <tag>
  --notes-file -` with the notes on stdin; `--prerelease` for `-rc`/`-beta`,
  `--draft` if the repo drafts first. `gh release view <tag>`: asset count
  right. No `gh`: stop here and hand over the notes, artifact paths and
  `SHA256SUMS` for the user to attach.
- Registry publish (`cargo publish`, `npm publish`, `uv publish`, `gem
  push`) only when named; `--dry-run` first, output pasted; rules and the
  yank commands in `references/decisions.md`.

## 8. Clean-environment install

CI tested the code; this tests the path a user takes. Run the README install
command verbatim where nothing is cached: `D=$(mktemp -d)`, `HOME=$D` plus
the ecosystem's cache dir inside it (`references/ecosystems.md`), or
`docker run --rm <base-image> sh -c '<install cmd> && <bin> --version'`
when a container runtime exists. Checksum the download against the
`SHA256SUMS` from the release page, not the local copy. `--version` must
print `<x.y.z>`; then one real command. Registries index late: retry for up
to ten minutes, then report "not yet indexed", never "installed". Wrong
version or broken artifact: a failed release (pre-release or yank, report,
fix forward). Install command wrong: the release stands, the README fix
heads the next patch; say so.

## 9. Report

```
Released <tag> -> <sha>   (range <prev>..<tag>, <n> commits)
Gate: <cmd> -> <summary line>, exit 0 (re-run at <bump sha>)
Version: <n> files bumped; old string 0 hits outside CHANGELOG
Changelog: <n> commits -> <n> lines, <n> skipped (<reason>)
Artifacts: <names>; SHA256SUMS verified after download
Published: <release url>; registry: <name x.y.z | none>
Clean install: <cmd> -> <version line>
Not done: <step> - <reason>   (or: none)
```

Interrupted: the exact state (commit, tag, release, registry) and the
command that resumes; never "mostly done", never roll back pushed state on
your own. Worked example: `references/example.md`.

## Decisions and refusals

Monorepo, hotfix line, pre-release, defect after the push, interrupted
upload, prompting release script, and the refusal list: `references/decisions.md`.
