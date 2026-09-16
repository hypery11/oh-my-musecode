#!/usr/bin/env bash
# fetch-host.sh — download and verify the Muse binary into .host/bin/ (PLAN.md 2.5).
#
#   scripts/fetch-host.sh [version]     place .host/bin/muse-bin-<version>, print the version
#   scripts/fetch-host.sh --query       print the channel's current version, download nothing
#
# Reproduces the research download that pinned the host (docs/host-reality.md),
# the way the launcher (.host/muse-launcher.sh) does it:
#   1. GET the channel document  → {version, manifest_url}
#   2. GET the release manifest  → artifacts.<platform>.{url, checksum, size}
#   3. GET the artifact with the launcher's User-Agent; verify the size AND the sha256
#   4. place it at .host/bin/muse-bin-<version>, mode 0755
# With a [version] other than the channel's, the manifest URL is derived from the
# channel's manifest_url by substituting the version (the URL carries it as a query
# parameter); the manifest's own `version` field must then match.
#
# The binary is never executed here. Re-running with the binary in place verifies
# it against the manifest and downloads nothing; with the network down, it is
# verified against the manifest recorded under .host/fetch/ by the previous run.
# The version goes to stdout, everything else to stderr, so
# `v="$(scripts/fetch-host.sh)"` works.
#
# Environment:
#   MUSE_CHANNEL_URL       channel document (default: the muse-stable channel)
#   MUSE_MANIFEST_URL      release manifest; skips the channel (needs [version])
#   OMM_HOST_DIR           where .host lives (default: <repo>/.host)
#   OMM_FETCH_ALLOW_HTTP   1 to accept http:// URLs — loopback test servers only
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
host_dir="${OMM_HOST_DIR:-$root/.host}"
bin_dir="$host_dir/bin"
record_dir="$host_dir/fetch"
channel_url="${MUSE_CHANNEL_URL:-https://api.meta.ai/muse-code/channels/muse-stable}"
manifest_url_override="${MUSE_MANIFEST_URL:-}"
# The launcher's identity (`launcher_version="2"` in .host/muse-launcher.sh).
user_agent="muse-code/launcher-2"
# The launcher's version grammar; the version lands in a file name, so it is enforced.
version_pattern='^[0-9]+\.[0-9]+\.[0-9]+-R[0-9]+(\.[0-9]+)?$'
proto='=https'
[[ "${OMM_FETCH_ALLOW_HTTP:-0}" != 1 ]] || proto='=http,https'

query_only=0
requested=""
for arg in "$@"; do
  case "$arg" in
    --query) query_only=1 ;;
    -h | --help)
      sed -n '2,25p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    -*)
      echo "fetch-host: unknown option: $arg" >&2
      exit 2
      ;;
    *)
      if [[ -n "$requested" ]]; then
        echo "fetch-host: one version at most" >&2
        exit 2
      fi
      requested="$arg"
      ;;
  esac
done
if [[ -n "$requested" && ! "$requested" =~ $version_pattern ]]; then
  echo "fetch-host: '$requested' is not a Muse version (expected e.g. 1.0.3-R2198.1)" >&2
  exit 2
fi
if [[ -n "$manifest_url_override" && -z "$requested" ]]; then
  echo "fetch-host: MUSE_MANIFEST_URL needs the version as the argument" >&2
  exit 2
fi

work=""
part=""
cleanup() {
  [[ -z "$part" ]] || rm -f "$part"
  [[ -z "$work" ]] || rm -rf "$work"
}
trap cleanup EXIT

die() {
  echo "fetch-host: $*" >&2
  exit 1
}

note() {
  echo "fetch-host: $*" >&2
}

for tool in curl mktemp uname wc; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool is required"
done
json_tool=""
if command -v jq >/dev/null 2>&1; then
  json_tool=jq
elif command -v python3 >/dev/null 2>&1; then
  json_tool=python3
else
  die "jq or python3 is required to read the channel and manifest documents"
fi

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

file_size() {
  local size
  size="$(wc -c <"$1")" || return 1
  printf '%s\n' "${size//[[:space:]]/}"
}

detect_platform() {
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64 | Darwin:aarch64) printf '%s\n' aarch64_macos ;;
    Darwin:x86_64 | Darwin:amd64) printf '%s\n' x86_macos ;;
    Linux:arm64 | Linux:aarch64) printf '%s\n' aarch64_linux ;;
    Linux:x86_64 | Linux:amd64) printf '%s\n' x86_linux ;;
    *) die "unsupported platform: $(uname -s) $(uname -m)" ;;
  esac
}

check_url() {
  case "$1" in
    https://*) return 0 ;;
    http://*) [[ "$proto" == '=http,https' ]] || die "$2 is not https: $1 (OMM_FETCH_ALLOW_HTTP=1 accepts http for a loopback test server)" ;;
    *) die "$2 is not an http(s) URL: $1" ;;
  esac
}

# GET a small JSON document. The launcher's flags: follow at most three
# redirects, https only (both hops), TLS 1.2+, its User-Agent.
fetch_json() {
  local url="$1" destination="$2"
  curl \
    --fail --silent --show-error \
    --location --max-redirs 3 \
    --proto "$proto" --proto-redir "$proto" --tlsv1.2 \
    --connect-timeout 15 --max-time 60 --retry 2 \
    --user-agent "$user_agent" \
    --header 'Accept: application/json' \
    --output "$destination" \
    "$url"
}

# GET the artifact (hundreds of MB): a progress bar on a terminal, silence otherwise.
fetch_artifact() {
  local url="$1" destination="$2"
  local -a verbosity=(--silent)
  [[ ! -t 2 ]] || verbosity=(--progress-bar)
  curl \
    --fail "${verbosity[@]}" --show-error \
    --location --max-redirs 3 \
    --proto "$proto" --proto-redir "$proto" --tlsv1.2 \
    --connect-timeout 15 --retry 2 \
    --user-agent "$user_agent" \
    --output "$destination" \
    "$url"
}

# `version<TAB>manifest_url` from a channel document.
channel_fields() {
  case "$json_tool" in
    jq) jq -r '[.version // "", .manifest_url // ""] | @tsv' "$1" ;;
    python3)
      python3 - "$1" <<'PY'
import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
v, u = d.get("version", ""), d.get("manifest_url", "")
if not isinstance(v, str) or not isinstance(u, str):
    sys.exit(1)
print(f"{v}\t{u}")
PY
      ;;
  esac
}

# `version<TAB>checksum_algorithm<TAB>url<TAB>checksum<TAB>size` for one platform.
artifact_fields() {
  case "$json_tool" in
    jq)
      jq -r --arg p "$2" \
        '[.version // "", .checksum_algorithm // "", .artifacts[$p].url // "", .artifacts[$p].checksum // "", (.artifacts[$p].size // "" | tostring)] | @tsv' "$1"
      ;;
    python3)
      python3 - "$1" "$2" <<'PY'
import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
a = (d.get("artifacts") or {}).get(sys.argv[2]) or {}
size = a.get("size", "")
if not isinstance(size, int) or isinstance(size, bool):
    size = ""
row = [d.get("version", ""), d.get("checksum_algorithm", ""), a.get("url", ""), a.get("checksum", ""), str(size)]
if not all(isinstance(x, str) for x in row):
    sys.exit(1)
print("\t".join(row))
PY
      ;;
  esac
}

# Verify a file's size and sha256 against the manifest values; prints nothing.
verify_file() {
  local file="$1" expected_size="$2" expected_checksum="$3" actual
  actual="$(file_size "$file")"
  [[ "$actual" == "$expected_size" ]] ||
    die "$file: size mismatch — the manifest says $expected_size bytes, the file has $actual"
  actual="$(sha256_file "$file")"
  [[ "$actual" == "$expected_checksum" ]] ||
    die "$file: sha256 mismatch — the manifest says $expected_checksum, the file hashes to $actual"
}

# Read the manifest document at $1 for $platform into the artifact_* globals.
artifact_url=""
artifact_checksum=""
artifact_size=""
load_manifest() {
  local manifest="$1" expected_version="$2" fields m_version algorithm
  fields="$(artifact_fields "$manifest" "$platform")" || die "$manifest is not a release manifest"
  IFS=$'\t' read -r m_version algorithm artifact_url artifact_checksum artifact_size <<<"$fields"
  [[ "$m_version" == "$expected_version" ]] ||
    die "the manifest is for version '${m_version:-?}', not $expected_version"
  [[ "$algorithm" == sha256 ]] || die "the manifest's checksum_algorithm is '${algorithm:-?}', not sha256"
  [[ -n "$artifact_url" ]] || die "the manifest has no artifact for $platform"
  check_url "$artifact_url" "the $platform artifact URL"
  [[ "$artifact_checksum" =~ ^[0-9a-f]{64}$ ]] || die "the $platform checksum is not a sha256 hex digest: '$artifact_checksum'"
  [[ "$artifact_size" =~ ^[1-9][0-9]*$ ]] || die "the $platform size is not a positive integer: '$artifact_size'"
}

platform="$(detect_platform)"
work="$(mktemp -d "${TMPDIR:-/tmp}/omm-fetch-host.XXXXXX")"

# ---- 1. the channel -------------------------------------------------------
version=""
manifest_url=""
channel_ok=0
if [[ -z "$manifest_url_override" ]]; then
  check_url "$channel_url" "MUSE_CHANNEL_URL"
  if fetch_json "$channel_url" "$work/channel.json"; then
    channel_ok=1
    fields="$(channel_fields "$work/channel.json")" || die "$channel_url did not return a channel document"
    IFS=$'\t' read -r channel_version manifest_url <<<"$fields"
    [[ "$channel_version" =~ $version_pattern ]] ||
      die "the channel's version '${channel_version:-?}' does not match the version grammar"
    [[ -n "$manifest_url" ]] || die "the channel document has no manifest_url"
    check_url "$manifest_url" "the channel's manifest_url"
    note "channel: $channel_url → $channel_version"
    if [[ "$query_only" == 1 ]]; then
      printf '%s\n' "$channel_version"
      exit 0
    fi
    if [[ -z "$requested" || "$requested" == "$channel_version" ]]; then
      version="$channel_version"
    else
      version="$requested"
      # The manifest URL carries the version as a query parameter
      # (`…/download/?channel=muse&version=<v>&file=manifest.json`).
      case "$manifest_url" in
        *"version=$channel_version"*) manifest_url="${manifest_url//version=$channel_version/version=$version}" ;;
        *) die "cannot derive the manifest URL for $version from the channel's manifest_url ($manifest_url); set MUSE_MANIFEST_URL" ;;
      esac
      note "requested $version (the channel is at $channel_version): manifest $manifest_url"
    fi
  else
    [[ "$query_only" == 0 ]] || die "could not reach $channel_url"
    [[ -n "$requested" ]] || die "could not reach $channel_url and no version was given to fall back on"
    version="$requested"
    note "could not reach $channel_url; continuing offline with $version"
  fi
else
  version="$requested"
  manifest_url="$manifest_url_override"
  check_url "$manifest_url" "MUSE_MANIFEST_URL"
fi

target="$bin_dir/muse-bin-$version"
record="$record_dir/manifest-$version.json"

# ---- 2. the manifest ------------------------------------------------------
manifest_file="$work/manifest.json"
if [[ -n "$manifest_url" ]] && fetch_json "$manifest_url" "$manifest_file"; then
  load_manifest "$manifest_file" "$version"
  manifest_source="$manifest_url"
elif [[ -r "$record" ]]; then
  note "could not reach ${manifest_url:-the manifest}; using the recorded $record"
  load_manifest "$record" "$version"
  manifest_file="$record"
  manifest_source="$record"
else
  die "could not reach ${manifest_url:-the manifest} and no recorded manifest for $version under $record_dir"
fi

# ---- 3. already there? ----------------------------------------------------
if [[ -e "$target" ]]; then
  [[ -f "$target" && ! -L "$target" ]] || die "$target exists and is not a regular file; remove it"
  verify_file "$target" "$artifact_size" "$artifact_checksum"
  note "already present and verified against $manifest_source: $target"
else
  # ---- 4. download, verify, place ------------------------------------------
  mkdir -p "$bin_dir"
  part="$bin_dir/.muse-bin-$version.part.$$"
  note "downloading $platform $version ($artifact_size bytes) from $artifact_url"
  fetch_artifact "$artifact_url" "$part" || die "download failed: $artifact_url"
  verify_file "$part" "$artifact_size" "$artifact_checksum"
  chmod 0755 "$part"
  mv -f -- "$part" "$target"
  part=""
  note "placed $target (sha256 $artifact_checksum)"
fi

# ---- 5. record what was used ---------------------------------------------
mkdir -p "$record_dir"
if [[ "$manifest_file" != "$record" ]]; then
  cp -f -- "$manifest_file" "$record.tmp.$$"
  mv -f -- "$record.tmp.$$" "$record"
fi
if [[ "$channel_ok" == 1 ]]; then
  cp -f -- "$work/channel.json" "$record_dir/channel.json.tmp.$$"
  mv -f -- "$record_dir/channel.json.tmp.$$" "$record_dir/channel.json"
fi

printf '%s\n' "$version"
