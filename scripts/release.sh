#!/usr/bin/env bash
# release.sh — cut a release: bump, regenerate, gate, build, package (PLAN.md 2.5).
#
#   scripts/release.sh <version> [--dry-run] [--targets <triple,...>]
#
# In order; the first failure stops everything and nothing after it happens:
#   1. refuse on a dirty tree (dist-release/ excepted), an existing tag v<version>, a
#      version below the current one, a missing LICENSE or README.md, no host binary
#   2. bump the version — `[workspace.package]` in Cargo.toml (every crate inherits it),
#      or a crate's own literal `[package] version` — and `cargo update --workspace`
#   3. `cargo build --release` for this machine, then `omm build` (the catalogs carry
#      the version) and `omm build --check` (no drift may remain)
#   4. scripts/gate.sh — red → the bump and the regeneration are reverted, nothing packaged
#   5. build every requested target whose std is installed (`rustup target list
#      --installed`); one that is not installed is named and skipped — a cross toolchain
#      is never assumed (the release runs on one machine per platform, or the CI matrix)
#   6. dist-release/omm-<version>-<target>.tar.gz — `omm`, LICENSE, README.md, flat — a
#      copy of install.sh, and dist-release/SHA256SUMS
#   7. print the install one-liner and the commands that follow (commit, tag, push,
#      upload). This script never commits, tags or pushes.
#
# --dry-run runs the checks of step 1, prints the plan and writes nothing.
#
# Environment:
#   OMM_RELEASE_TARGETS   comma-separated Rust triples (default: the four released ones)
#   OMM_RELEASE_BASE_URL  the releases page the one-liner names (see install.sh)
#   OMM_MUSE_BIN          the host binary (default: the newest .host/bin/muse-bin-*)
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
export PATH="$HOME/.cargo/bin:$PATH"

dist="$root/dist-release"
default_targets="aarch64-apple-darwin,x86_64-apple-darwin,aarch64-unknown-linux-musl,x86_64-unknown-linux-musl"
targets_csv="${OMM_RELEASE_TARGETS:-$default_targets}"
base_url="${OMM_RELEASE_BASE_URL:-https://github.com/hypery11/oh-my-musecode/releases}"
base_url="${base_url%/}"
version=""
dry_run=0

die() {
  echo "release: $*" >&2
  exit 1
}

note() {
  echo "release: $*" >&2
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      dry_run=1
      shift
      ;;
    --targets)
      [[ $# -ge 2 ]] || die "--targets needs a value"
      targets_csv="$2"
      shift 2
      ;;
    --targets=*)
      targets_csv="${1#--targets=}"
      shift
      ;;
    -h | --help)
      sed -n '2,29p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*)
      die "unknown option: $1 (try --help)"
      ;;
    *)
      [[ -z "$version" ]] || die "one version only"
      version="$1"
      shift
      ;;
  esac
done
[[ -n "$version" ]] || die "usage: scripts/release.sh <version> [--dry-run] [--targets <triple,...>]"
version="${version#v}"
semver_pattern='^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$'
[[ "$version" =~ $semver_pattern ]] ||
  die "'$version' is not a semantic version (MAJOR.MINOR.PATCH[-pre])"

for tool in git cargo rustc rustup tar gzip awk; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool is required"
done

sha256_file() {
  local output
  if command -v sha256sum >/dev/null 2>&1; then
    output="$(sha256sum "$1")" || return 1
  elif command -v shasum >/dev/null 2>&1; then
    output="$(shasum -a 256 "$1")" || return 1
  else
    die "sha256sum or shasum is required"
  fi
  printf '%s\n' "${output%% *}"
}

# The `version = "…"` of one TOML table (`[package]`, `[workspace.package]`);
# `version.workspace = true` is not a match, exactly as omm-manifest reads it.
read_toml_version() {
  awk -v table="$2" '
    /^[[:space:]]*\[/ {
      h = $0
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", h)
      in_table = (h == "[" table "]")
      next
    }
    in_table && /^[[:space:]]*version[[:space:]]*=/ {
      v = $0
      sub(/^[^"]*"/, "", v)
      sub(/".*$/, "", v)
      print v
      exit
    }' "$1"
}

# Rewrite that one line; every other byte of the file is kept.
bump_toml_version() {
  local file="$1" table="$2" new="$3"
  awk -v table="$table" -v new="$new" '
    /^[[:space:]]*\[/ {
      h = $0
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", h)
      in_table = (h == "[" table "]")
    }
    in_table && /^[[:space:]]*version[[:space:]]*=/ && !done {
      print "version = \"" new "\""
      done = 1
      next
    }
    { print }
    END { exit done ? 0 : 1 }' "$file" >"$file.tmp.$$" || {
    rm -f "$file.tmp.$$"
    die "$file has no 'version = …' line under [$table]"
  }
  mv -f -- "$file.tmp.$$" "$file"
}

# ---- 1. preconditions -------------------------------------------------------
dirty="$(git status --porcelain --untracked-files=all -- . ':(exclude)dist-release')"
[[ -z "$dirty" ]] || die "the tree is not clean — commit or stash first:"$'\n'"$dirty"
if git rev-parse -q --verify "refs/tags/v$version" >/dev/null; then
  die "tag v$version already exists"
fi

omm_manifest="crates/omm/Cargo.toml"
bump_files=()
if grep -qE '^[[:space:]]*version\.workspace[[:space:]]*=[[:space:]]*true' "$omm_manifest"; then
  current="$(read_toml_version Cargo.toml workspace.package)"
  [[ -n "$current" ]] || die "$omm_manifest inherits its version but Cargo.toml has no [workspace.package] version"
  bump_files+=("Cargo.toml:workspace.package")
else
  current="$(read_toml_version "$omm_manifest" package)"
  [[ -n "$current" ]] || die "$omm_manifest has no [package] version"
  bump_files+=("$omm_manifest:package")
fi
# Members that spell the same version out literally move with it.
for f in crates/*/Cargo.toml; do
  [[ "$f" != "$omm_manifest" ]] || continue
  v="$(read_toml_version "$f" package)"
  [[ -n "$v" && "$v" == "$current" ]] && bump_files+=("$f:package")
done
if [[ "$version" != "$current" ]]; then
  lowest="$(printf '%s\n%s\n' "$current" "$version" | sort -V | head -n 1)"
  [[ "$lowest" == "$current" ]] || die "$version is below the current version $current"
fi

[[ -f LICENSE ]] || die "LICENSE is missing at the repo root (it ships in every tarball)"
[[ -f README.md ]] || die "README.md is missing at the repo root (it ships in every tarball)"
[[ -f install.sh ]] || die "install.sh is missing at the repo root"

if [[ -z "${OMM_MUSE_BIN:-}" ]]; then
  candidate="$(printf '%s\n' "$root"/.host/bin/muse-bin-* | sort -V | tail -n 1)"
  [[ -e "$candidate" ]] || die "OMM_MUSE_BIN is unset and .host/bin/ holds no muse-bin-*; run scripts/fetch-host.sh"
  export OMM_MUSE_BIN="$candidate"
fi
[[ -x "$OMM_MUSE_BIN" ]] || die "OMM_MUSE_BIN=$OMM_MUSE_BIN is not an executable file"

host_target="$(rustc -vV | sed -n 's/^host: //p')"
[[ -n "$host_target" ]] || die "rustc -vV names no host"
installed="$(rustup target list --installed 2>/dev/null || true)"
build_targets=()
skipped_targets=()
IFS=',' read -r -a requested <<<"$targets_csv"
have_host=0
for t in ${requested[@]+"${requested[@]}"}; do
  t="${t//[[:space:]]/}"
  [[ -n "$t" ]] || continue
  [[ "$t" != "$host_target" ]] || have_host=1
  if [[ "$t" == "$host_target" ]] || grep -qx -- "$t" <<<"$installed"; then
    build_targets+=("$t")
  else
    skipped_targets+=("$t")
  fi
done
[[ "$have_host" == 1 ]] || build_targets=("$host_target" ${build_targets[@]+"${build_targets[@]}"})

epoch="$(git log -1 --format=%ct)"
commit="$(git rev-parse --short HEAD)"

note "omm $current → $version at $commit"
note "bump: ${bump_files[*]}"
note "host: $OMM_MUSE_BIN"
note "targets: ${build_targets[*]}"
[[ ${#skipped_targets[@]} -eq 0 ]] ||
  note "skipped (std not installed — \`rustup target add <triple>\`; the linux-musl triples also need a linker or cargo-zigbuild): ${skipped_targets[*]}"
if [[ "$dry_run" == 1 ]]; then
  note "dry run: would bump, \`cargo update --workspace\`, \`cargo build --release\`, \`omm build\`, \`omm build --check\`, scripts/gate.sh,"
  for t in "${build_targets[@]}"; do
    note "  dist-release/omm-$version-$t.tar.gz"
  done
  note "  dist-release/install.sh, dist-release/SHA256SUMS; nothing was written"
  exit 0
fi

# ---- 2. bump ----------------------------------------------------------------
for spec in "${bump_files[@]}"; do
  bump_toml_version "${spec%%:*}" "${spec#*:}" "$version"
done
cargo update --workspace --offline >/dev/null 2>&1 || cargo update --workspace
[[ "$(read_toml_version "${bump_files[0]%%:*}" "${bump_files[0]#*:}")" == "$version" ]] ||
  die "the bump did not land in ${bump_files[0]%%:*}"

revert() {
  note "reverting the bump and the regenerated files (the tree was clean before)"
  git checkout -- .
  git clean -fdq --exclude=dist-release -- .
}

# ---- 3. build, regenerate, drift-check --------------------------------------
note "cargo build --release --locked -p omm ($host_target)"
cargo build --release --locked -p omm || {
  revert
  die "the release build failed"
}
omm_bin="$root/target/release/omm"
[[ -x "$omm_bin" ]] || die "no binary at $omm_bin"

# Every host run sandboxed: the generator's lint runs `plugins validate`.
sandbox="$(mktemp -d "${TMPDIR:-/tmp}/omm-release.XXXXXX")"
trap 'rm -rf "$sandbox"' EXIT
mkdir -p "$sandbox/home" "$sandbox/config" "$sandbox/data"
run_omm() {
  env HOME="$sandbox/home" XDG_CONFIG_HOME="$sandbox/config" XDG_DATA_HOME="$sandbox/data" \
    MUSE_NO_AUTO_UPDATE=1 OMM_MUSE_BIN="$OMM_MUSE_BIN" "$omm_bin" "$@"
}
note "omm build (the catalogs carry $version)"
run_omm build "$root" || {
  revert
  die "omm build refused"
}
note "omm build --check"
run_omm build --check "$root" || {
  revert
  die "the regenerated files still drift from the committed ones"
}

# ---- 4. gate ----------------------------------------------------------------
gate_log="${TMPDIR:-/tmp}/omm-release-gate.$version.$$.log"
note "scripts/gate.sh (log: $gate_log)"
if ! scripts/gate.sh 2>&1 | tee "$gate_log"; then
  revert
  die "the gate is red (see $gate_log); nothing was packaged"
fi

# ---- 5. every installed target ----------------------------------------------
# The host target's binary is the one already built (target/release/omm);
# every other target lands under target/<triple>/release/omm.
binary_for() {
  if [[ "$1" == "$host_target" ]]; then
    printf '%s\n' "$omm_bin"
  else
    printf '%s\n' "$root/target/$1/release/omm"
  fi
}
for t in "${build_targets[@]}"; do
  [[ "$t" != "$host_target" ]] || continue
  note "cargo build --release --locked -p omm --target $t"
  cargo build --release --locked -p omm --target "$t" ||
    die "the build for $t failed (its std is installed; a linker for it is not, or it needs cargo-zigbuild)"
  [[ -x "$(binary_for "$t")" ]] || die "no binary at $(binary_for "$t")"
done

# ---- 6. package -------------------------------------------------------------
mkdir -p "$dist"
rm -rf "$dist"/.stage-* "$dist"/omm-*.tar.gz "$dist/SHA256SUMS" "$dist/install.sh"

# A stable mtime (the commit's) on every member, gzip without a name or a
# timestamp: the same commit packs to the same bytes on the same toolchain.
stamp() {
  local when
  when="$(date -u -r "$epoch" +%Y%m%d%H%M.%S 2>/dev/null || date -u -d "@$epoch" +%Y%m%d%H%M.%S)"
  touch -t "$when" "$@"
}
pack() {
  local target="$1" bin="$2"
  local stage="$dist/.stage-$target" out="$dist/omm-$version-$target.tar.gz"
  rm -rf "$stage"
  mkdir -p "$stage"
  cp -f -- "$bin" "$stage/omm"
  cp -f -- LICENSE "$stage/LICENSE"
  cp -f -- README.md "$stage/README.md"
  chmod 0755 "$stage/omm"
  chmod 0644 "$stage/LICENSE" "$stage/README.md"
  stamp "$stage/omm" "$stage/LICENSE" "$stage/README.md"
  if tar --version 2>/dev/null | grep -q 'GNU tar'; then
    tar --owner=0 --group=0 --numeric-owner --format=ustar -cf - -C "$stage" omm LICENSE README.md
  else
    COPYFILE_DISABLE=1 tar --uid 0 --gid 0 --uname root --gname root --format=ustar -cf - -C "$stage" omm LICENSE README.md
  fi | gzip -n -9 >"$out.tmp.$$"
  mv -f -- "$out.tmp.$$" "$out"
  rm -rf "$stage"
}
for t in "${build_targets[@]}"; do
  pack "$t" "$(binary_for "$t")"
done
cp -f -- install.sh "$dist/install.sh"
chmod 0755 "$dist/install.sh"
(
  cd "$dist"
  for f in omm-"$version"-*.tar.gz install.sh; do
    printf '%s  %s\n' "$(sha256_file "$f")" "$f"
  done
) >"$dist/SHA256SUMS.tmp.$$"
mv -f -- "$dist/SHA256SUMS.tmp.$$" "$dist/SHA256SUMS"

# ---- 7. report --------------------------------------------------------------
echo
echo "release $version — dist-release/"
sed 's/^/  /' "$dist/SHA256SUMS"
if [[ ${#skipped_targets[@]} -gt 0 ]]; then
  echo "  not built (std not installed here): ${skipped_targets[*]}"
fi
echo
echo "the tree now carries the bump and the regenerated catalogs; next:"
echo "  git add -A && git commit -m \"release $version\""
echo "  git tag -a v$version -m \"omm $version\" && git push --follow-tags"
echo "  gh release create v$version dist-release/*"
echo
echo "install (once the release is published):"
echo "  curl -fsSL $base_url/latest/download/install.sh | sh"
echo "  curl -fsSL $base_url/download/v$version/install.sh | sh -s -- --version $version"
