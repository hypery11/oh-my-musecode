# scripts/

The repository's operational scripts: the gate, the host download, the release
cut, and (at the repo root) the installer the release ships. `.github/workflows/ci.yml`
wires the first three into CI. Every script is bash except `install.sh`, which is
POSIX `sh` because it runs as `curl … | sh`; all four are shellcheck-clean.

| Script | Does | Writes |
|---|---|---|
| `scripts/gate.sh` | fmt, clippy, every test of every crate against the pinned host (`OMM_MUSE_BIN`, default the newest `.host/bin/muse-bin-*`); per-target summary; exits red if anything was red | nothing |
| `scripts/fetch-host.sh [version]` | downloads and verifies the Muse binary into `.host/bin/muse-bin-<version>`; prints the version | `.host/bin/`, `.host/fetch/` (gitignored) |
| `scripts/release.sh <version>` | bump → regenerate → gate → build → `dist-release/` tarballs + `SHA256SUMS`; prints the install one-liner; never tags or pushes | `Cargo.toml`, `Cargo.lock`, the generated package and catalogs, `dist-release/` |
| `install.sh` (repo root) | the `curl \| sh` entry: downloads the release tarball for this platform, verifies it against `SHA256SUMS`, places `omm`; with `--modify-path` also one PATH line | `$OMM_INSTALL_DIR/omm` and nothing else |

## fetch-host.sh — the host binary as a verified download

```
scripts/fetch-host.sh              # the channel's current version
scripts/fetch-host.sh 1.0.3-R2198.1     # the pinned stable build (docs/host-data/muse-cli.json observed_build)
scripts/fetch-host.sh --query      # print the channel's version, download nothing
```

Reproduces the research download that pinned the host (`docs/host-reality.md`), the
way `.host/muse-launcher.sh` does it:

1. GET the channel document (`MUSE_CHANNEL_URL`, default
   `https://api.meta.ai/muse-code/channels/muse-stable`) → `{version, manifest_url}`.
2. GET the release manifest → `artifacts.<platform>.{url, checksum, size}`, platform from
   `uname -s`/`-m` (`aarch64_macos`, `x86_macos`, `aarch64_linux`, `x86_linux`).
3. GET the artifact with the launcher's `User-Agent: muse-code/launcher-2`, verify the
   **size and the sha256** from the manifest, place it atomically at
   `.host/bin/muse-bin-<version>`, mode 0755. Never executed.
4. Record the channel document and the manifest under `.host/fetch/`.

Asking for a version other than the channel's derives the manifest URL by substituting
the version into the channel's `manifest_url` (it is a query parameter); the manifest's
own `version` must then match. `MUSE_MANIFEST_URL` overrides the manifest URL outright.

Idempotent: with the binary in place it is re-verified against the manifest and nothing
is downloaded; with the network down it is verified against the recorded manifest. A
tampered manifest or a wrong-sized/-hashed download is refused with both values named and
nothing placed (no `.part` left behind). The version is the only thing on stdout, so
`v="$(scripts/fetch-host.sh)"` works.

Needs `curl` and `jq` (or `python3`). `OMM_HOST_DIR` moves `.host/` (tests);
`OMM_FETCH_ALLOW_HTTP=1` accepts `http://` for a loopback test server and nothing else —
the default is https-only on every hop, TLS 1.2+, at most three redirects.

Measured 2026-09-03: the channel served `1.0.2-R2040.1` (241,592,688 B, sha256
`41d37e49…`, 22 s anonymous download); the pinned `1.0.1-R2006.1` manifest is still
served and verifies the checked-in binary (`b9c7f9ba…`). 2026-09-05: the channel serves
`1.0.3-R2198.1` (241,984,144 B, sha256 `4c0f9600…`), now the pin; `1.0.1-R2006.1` stays in
`.host/bin/` as the previous stable build the gate also runs against (ci.yml matrix).

## release.sh — cut a release

```
scripts/release.sh 0.2.0              # the whole thing
scripts/release.sh 0.2.0 --dry-run    # the checks and the plan, nothing written
scripts/release.sh 0.2.0 --targets aarch64-apple-darwin
```

In order; the first failure stops everything:

1. **Refuse** on a dirty tree (`dist-release/` excepted — it is this script's output),
   an existing `v<version>` tag, a version below the current one, a missing `LICENSE`
   or `README.md` at the repo root (both ship in every tarball), or no host binary.
2. **Bump**: `crates/omm/Cargo.toml` inherits `version` from `[workspace.package]` in the
   root `Cargo.toml`, so that is the line rewritten (a crate that spells the version out
   literally is rewritten too); then `cargo update --workspace` for `Cargo.lock`. The
   same version twice is a no-op, so a rerun after a fixed gate is fine.
3. `cargo build --release --locked -p omm`, then `omm build` — the native package and
   the three catalogs carry the version, and the digest is obtained from the host — and
   `omm build --check`, which must report no drift. The host runs in a throwaway
   `HOME`/`XDG_*` sandbox.
4. `scripts/gate.sh`. Red → the bump and the regenerated files are reverted (the tree
   was clean, so `git checkout -- . && git clean -fd` restores it exactly), the log is
   named, nothing is packaged.
5. Every requested target (`--targets` / `OMM_RELEASE_TARGETS`, default
   `aarch64-apple-darwin,x86_64-apple-darwin,aarch64-unknown-linux-musl,x86_64-unknown-linux-musl`)
   whose std is installed (`rustup target list --installed`) is built with
   `--target`; one that is not installed is **named and skipped**, never assumed — the
   linux-musl triples also need a linker (or `cargo-zigbuild`). The full set comes from
   one machine per platform, or a CI matrix, not from one laptop.
6. `dist-release/omm-<version>-<target>.tar.gz` (flat: `omm`, `LICENSE`, `README.md`;
   mtimes = the commit's, owner 0/0, `gzip -n`, so the same commit packs to the same bytes
   on the same toolchain), a copy of `install.sh`, and `dist-release/SHA256SUMS` in
   `sha256sum -c` format.
7. Prints the checksum list, the commands that follow, and the install one-liners.

It never commits, tags or pushes. Afterwards the tree carries the bump and the
regenerated catalogs; the release engineer runs:

```
git add -A && git commit -m "release <version>"
git tag -a v<version> -m "omm <version>" && git push --follow-tags
gh release create v<version> dist-release/*
```

`dist-release/` is not in `.gitignore` yet (this script excludes it from its own
clean-tree check); add it there.

## install.sh — the two-phase entry (R6)

```
curl -fsSL https://github.com/hypery11/oh-my-musecode/releases/latest/download/install.sh | sh
curl -fsSL … | sh -s -- --modify-path
OMM_VERSION=0.2.0 OMM_INSTALL_DIR=~/bin sh install.sh
```

Phase one places the binary and writes nothing else — only `omm install` touches Muse
config (ARCHITECTURE.md R6). Steps: GET `SHA256SUMS` for the release (`latest`, or
`--version`/`OMM_VERSION`), pick the line for this platform's target triple
(`aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-musl`,
`x86_64-unknown-linux-musl`; `OMM_TARGET` overrides), GET that tarball, verify its sha256,
unpack `omm`, place it atomically at `$OMM_INSTALL_DIR/omm` (default `~/.local/bin/omm`),
run `omm --version` (a binary that does not run is removed again). Then:

- the directory is on `PATH` → nothing more;
- `--modify-path` → one line appended to the shell's profile (`.zshrc`, `.bashrc`, a
  fish `conf.d` file, else `.profile`), never twice;
- otherwise → the line is printed for you to add.

`--dry-run` downloads and verifies and places nothing. The last line is always
"run `omm install` when ready".

**The release URL is a variable.** `OMM_RELEASE_BASE_URL` defaults to
`https://github.com/hypery11/oh-my-musecode/releases`, the shape GitHub gives release assets
(`<base>/latest/download/<asset>`, `<base>/download/v<version>/<asset>`); no repository
exists under that name yet, so the default is one line at the top of the script to change
when it does. `OMM_INSTALL_ALLOW_HTTP=1` accepts `http://` for a loopback test server.

## CI — `.github/workflows/ci.yml`

- **gate** (push, pull request; macOS arm64 and Linux x86_64): `cargo fmt --check`,
  `cargo clippy -D warnings`, shellcheck on the four scripts, `scripts/fetch-host.sh
  <pinned>` with `.host/` cached by version, `scripts/gate.sh`, then `cargo build
  --release` and `omm build --check` in a sandboxed `HOME`/`XDG_*`. The pinned version is
  `observed_build` in `docs/host-data/muse-cli.json` — read there, never copied.
- **hostcheck** (daily schedule and `workflow_dispatch`; R15 — probe behaviour on every
  channel poll): `scripts/fetch-host.sh --query` asks the channel what it serves now, that
  binary is fetched (cached by its version), `cargo test -p omm-host --test hostcheck`
  re-measures the P0/P1 tables of `docs/host-reality.md` against it, then the whole gate
  runs against it. A moved channel is announced as a workflow notice next to the pinned
  version; a red run is the drift signal.

Runners must reach `api.meta.ai` and `lookaside.facebook.com` anonymously (the channel
is `"state":"public"`; the download needs no login).

## Testing the scripts locally

Every script was exercised against a loopback server, never the real config root:

- `fetch-host.sh`: a small Python server answering `/channel` and
  `/download/?version=…&file=…` from a directory of fake manifests and random-byte
  "binaries" (`MUSE_CHANNEL_URL=http://127.0.0.1:<port>/channel OMM_FETCH_ALLOW_HTTP=1
  OMM_HOST_DIR=<tmp>`): fresh download, idempotent rerun (no artifact GET), an older
  version through URL substitution, checksum and size tampering refused, offline reruns
  from the recorded manifest, the `python3` fallback without `jq`. Then the real channel:
  `--query`, the pinned version verified in place, a fresh download of the current one.
- `install.sh`: `python3 -m http.server` over a directory shaped like the release page
  (`latest/download/`, `download/v<version>/`) holding a real tarball and `SHA256SUMS`
  (`OMM_RELEASE_BASE_URL=http://127.0.0.1:<port> OMM_INSTALL_ALLOW_HTTP=1 HOME=<tmp>`):
  under `dash`, `sh`, `bash --posix` and as `curl … | sh -s -- --modify-path`; a fresh
  `HOME` holds exactly `.local/bin/omm` afterwards (plus `.zshrc` with `--modify-path`,
  once); a tampered `SHA256SUMS` and a binary that does not run are refused with nothing
  left behind; `--dry-run` writes nothing.
- `release.sh`: in a throwaway clone with a `LICENSE` and `README.md` committed and
  `OMM_MUSE_BIN` pointing at the pinned binary: `--dry-run`, the full run, the dirty-tree
  refusal, and the produced tarball installed through `install.sh` from its own
  `SHA256SUMS`.
