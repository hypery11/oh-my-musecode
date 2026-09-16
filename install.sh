#!/bin/sh
# install.sh — place the `omm` binary and nothing else (ARCHITECTURE.md R6: two-phase;
# only `omm install` touches Muse config).
#
#   curl -fsSL https://github.com/hypery11/oh-my-musecode/releases/latest/download/install.sh | sh
#   curl -fsSL … | sh -s -- --modify-path
#   OMM_VERSION=0.2.0 OMM_INSTALL_DIR=~/bin sh install.sh
#
# What it does: download SHA256SUMS for the release, then the tarball for this
# platform, verify its sha256 against the list, unpack `omm`, place it at
# $OMM_INSTALL_DIR/omm (default ~/.local/bin/omm), run `omm --version`. With
# --modify-path it also appends one PATH line to your shell profile; without
# it, it prints the line for you to add. Nothing else is written.
#
# Options:
#   --version <v>    the release to install (default: latest)
#   --dir <path>     where to place the binary (default: $OMM_INSTALL_DIR or ~/.local/bin)
#   --modify-path    append the PATH line to the shell profile (never without this flag)
#   --dry-run        download and verify, place nothing, say what would happen
#   -h, --help
#
# Environment:
#   OMM_RELEASE_BASE_URL  the releases page assets hang off; default below. The
#                         repository does not exist yet, so this is a variable.
#   OMM_VERSION           same as --version
#   OMM_INSTALL_DIR       same as --dir
#   OMM_TARGET            override the detected Rust target triple
#   OMM_INSTALL_ALLOW_HTTP=1  accept http:// — loopback test servers only
set -eu

# <base>/latest/download/<asset> and <base>/download/v<version>/<asset> —
# the shape GitHub gives release assets. No repository exists under this
# name yet: change this one line (or set the variable) when it does.
base_url="${OMM_RELEASE_BASE_URL:-https://github.com/hypery11/oh-my-musecode/releases}"
version="${OMM_VERSION:-latest}"
install_dir="${OMM_INSTALL_DIR:-}"
modify_path=0
dry_run=0
proto='=https'
[ "${OMM_INSTALL_ALLOW_HTTP:-0}" != 1 ] || proto='=http,https'

usage() {
  sed -n '2,30p' "$0" 2>/dev/null | sed 's/^# \{0,1\}//'
}

die() {
  printf 'omm install.sh: %s\n' "$*" >&2
  exit 1
}

note() {
  printf 'omm install.sh: %s\n' "$*" >&2
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      [ $# -ge 2 ] || die "--version needs a value"
      version="$2"
      shift 2
      ;;
    --version=*)
      version="${1#--version=}"
      shift
      ;;
    --dir)
      [ $# -ge 2 ] || die "--dir needs a value"
      install_dir="$2"
      shift 2
      ;;
    --dir=*)
      install_dir="${1#--dir=}"
      shift
      ;;
    --modify-path)
      modify_path=1
      shift
      ;;
    --dry-run)
      dry_run=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1 (try --help)"
      ;;
  esac
done

if [ -z "$install_dir" ]; then
  [ -n "${HOME:-}" ] || die "HOME is not set; pass --dir"
  install_dir="$HOME/.local/bin"
fi
case "$install_dir" in
  /*) ;;
  *) install_dir="$(pwd)/$install_dir" ;;
esac
case "$version" in
  latest) ;;
  v*) version="${version#v}" ;;
esac
case "$version" in
  latest) ;;
  *[!0-9A-Za-z.-]* | "") die "'$version' is not a release version" ;;
esac

for tool in curl tar mktemp uname; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool is required"
done

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d ' ' -f 1
  elif command -v openssl >/dev/null 2>&1; then
    openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1
  else
    die "sha256sum, shasum or openssl is required"
  fi
}

detect_target() {
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64 | Darwin:aarch64) echo aarch64-apple-darwin ;;
    Darwin:x86_64 | Darwin:amd64) echo x86_64-apple-darwin ;;
    Linux:arm64 | Linux:aarch64) echo aarch64-unknown-linux-musl ;;
    Linux:x86_64 | Linux:amd64) echo x86_64-unknown-linux-musl ;;
    *) die "unsupported platform: $(uname -s) $(uname -m) (set OMM_TARGET to a released target)" ;;
  esac
}

fetch() {
  # $1 URL, $2 destination. https only (both hops), TLS 1.2+, no more than
  # three redirects, a real failure on an HTTP error.
  curl \
    --fail --silent --show-error \
    --location --max-redirs 3 \
    --proto "$proto" --proto-redir "$proto" --tlsv1.2 \
    --connect-timeout 15 --retry 2 \
    --user-agent "omm-install.sh" \
    --output "$2" \
    "$1"
}

display_path() {
  tilde='~'
  case "$1" in
    "${HOME:-/nonexistent}"/*) printf '%s/%s\n' "$tilde" "${1#"$HOME"/}" ;;
    *) printf '%s\n' "$1" ;;
  esac
}

target="${OMM_TARGET:-$(detect_target)}"
case "$base_url" in
  https://*) ;;
  http://*) [ "$proto" = '=http,https' ] || die "OMM_RELEASE_BASE_URL is not https: $base_url" ;;
  *) die "OMM_RELEASE_BASE_URL is not an http(s) URL: $base_url" ;;
esac
base_url="${base_url%/}"
if [ "$version" = latest ]; then
  asset_base="$base_url/latest/download"
else
  asset_base="$base_url/download/v$version"
fi

tmp="$(mktemp -d "${TMPDIR:-/tmp}/omm-install.XXXXXX")"
trap 'rm -rf "$tmp"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# ---- 1. the checksum list names the exact asset --------------------------
note "release list: $asset_base/SHA256SUMS"
fetch "$asset_base/SHA256SUMS" "$tmp/SHA256SUMS" || die "could not fetch $asset_base/SHA256SUMS"
if [ "$version" = latest ]; then
  line="$(grep -E "^[0-9a-f]{64} [ *]omm-[0-9A-Za-z.-]+-$target\.tar\.gz\$" "$tmp/SHA256SUMS" | head -n 1 || true)"
else
  line="$(grep -E "^[0-9a-f]{64} [ *]omm-$version-$target\.tar\.gz\$" "$tmp/SHA256SUMS" | head -n 1 || true)"
fi
[ -n "$line" ] || die "no asset for $target in $asset_base/SHA256SUMS (released: $(sed -n 's/^[0-9a-f]\{64\} [ *]\(omm-.*\.tar\.gz\)$/\1/p' "$tmp/SHA256SUMS" | tr '\n' ' '| sed 's/ $//'))"
expected="${line%% *}"
asset="${line##* }"
asset="${asset#\*}"
if [ "$version" = latest ]; then
  version="${asset#omm-}"
  version="${version%"-$target.tar.gz"}"
fi
note "omm $version for $target: $asset"

# ---- 2. the tarball, verified ---------------------------------------------
fetch "$asset_base/$asset" "$tmp/$asset" || die "could not fetch $asset_base/$asset"
actual="$(sha256_file "$tmp/$asset")"
[ "$actual" = "$expected" ] ||
  die "$asset: sha256 mismatch — SHA256SUMS says $expected, the download hashes to $actual; nothing was installed"
note "sha256 verified: $expected"

mkdir "$tmp/unpack"
tar -xzf "$tmp/$asset" -C "$tmp/unpack" omm || die "$asset does not unpack an 'omm' member"
[ -f "$tmp/unpack/omm" ] || die "$asset carries no 'omm' file"
chmod 0755 "$tmp/unpack/omm"

# ---- 3. place it -----------------------------------------------------------
destination="$install_dir/omm"
if [ "$dry_run" = 1 ]; then
  note "dry run: would place omm $version at $(display_path "$destination"); nothing was written"
  exit 0
fi
mkdir -p "$install_dir" || die "cannot create $install_dir"
staged="$install_dir/.omm.$$.tmp"
cp "$tmp/unpack/omm" "$staged" || die "cannot write into $install_dir"
chmod 0755 "$staged"
mv -f "$staged" "$destination"
if ! reported="$("$destination" --version 2>&1)"; then
  rm -f "$destination"
  die "the placed binary does not run ($reported); removed it again — is $target the right target for this machine? (OMM_TARGET overrides)"
fi
note "placed $(display_path "$destination") ($reported)"

# ---- 4. PATH — only with --modify-path -------------------------------------
on_path=0
case ":${PATH:-}:" in
  *":$install_dir:"*) on_path=1 ;;
esac
if [ "$on_path" = 1 ]; then
  :
elif [ "$modify_path" = 1 ]; then
  [ -n "${HOME:-}" ] || die "HOME is not set; the binary is in place but no shell profile can be found for --modify-path"
  shell_name="$(basename "${SHELL:-sh}")"
  case "$shell_name" in
    zsh) profile="${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) profile="$HOME/.bashrc" ;;
    fish) profile="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/omm.fish" ;;
    *) profile="$HOME/.profile" ;;
  esac
  if [ "$shell_name" = fish ]; then
    path_line="fish_add_path --global \"$install_dir\"  # added by omm install.sh"
  else
    path_line="export PATH=\"$install_dir:\$PATH\"  # added by omm install.sh"
  fi
  if [ -f "$profile" ] && grep -Fqx -- "$path_line" "$profile"; then
    note "$(display_path "$profile") already carries the PATH line"
  else
    mkdir -p "$(dirname "$profile")"
    printf '\n%s\n' "$path_line" >>"$profile" || die "cannot append to $profile"
    note "appended to $(display_path "$profile"): $path_line"
    note "open a new shell, or run: $path_line"
  fi
else
  note "$(display_path "$install_dir") is not on your PATH; add it with"
  note "    export PATH=\"$install_dir:\$PATH\""
  note "(re-run with --modify-path to have this written to your shell profile)"
fi

note "done. Nothing but the binary was written; run \`omm install\` when ready."
