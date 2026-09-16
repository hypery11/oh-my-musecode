#!/usr/bin/env bash
set -euo pipefail

channel="muse-stable"
channel_url="${MUSE_CHANNEL_URL:-https://api.meta.ai/muse-code/channels/muse-stable}"
update_interval="${MUSE_UPDATE_INTERVAL_SECONDS:-3600}"
auth_url="${MUSE_AUTH_URL:-https://auth.meta.com}"
client_id="${MUSE_CLIENT_ID:-1031625952748946}"
download_host="${MUSE_DOWNLOAD_HOST:-lookaside.facebook.com}"
authorization_endpoint="/oidc/device/authorization/"
token_endpoint="/oidc/device/token/"
device_code_grant="urn:ietf:params:oauth:grant-type:device_code"
launcher_url="${MUSE_LAUNCHER_URL:-https://api.meta.ai/muse-launcher.sh}"
# Bump on every launcher change; rides the User-Agent.
launcher_version="2"
user_agent="muse-code/launcher-$launcher_version"
# The trailing component is optional; earlier releases carried it.
version_pattern='^[0-9]+\.[0-9]+\.[0-9]+-R[0-9]+(\.[0-9]+)?$'
credential_default=""
if [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
  credential_default="$XDG_CONFIG_HOME/muse/auth.json"
elif [[ -n "${HOME:-}" ]]; then
  credential_default="$HOME/.config/muse/auth.json"
fi
credential_path="${MUSE_AUTH_PATH:-$credential_default}"
access_token=""
work=""
lock_dir=""
state_tmp=""
notice_tmp=""
release_info_tmp=""
launcher_tmp=""
launcher_headers=""
skip_download_auth=0
request_status=""

die() {
  printf 'muse: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  [[ -z "$work" ]] || rm -rf "$work"
  [[ -z "$state_tmp" ]] || rm -f "$state_tmp"
  [[ -z "$notice_tmp" ]] || rm -f "$notice_tmp"
  [[ -z "$release_info_tmp" ]] || rm -f "$release_info_tmp"
  if [[ -n "$lock_dir" && -r "$lock_dir/pid" ]] && \
      [[ "$(<"$lock_dir/pid")" == "$$" ]]; then
    rm -rf -- "$lock_dir"
  fi
}

parent_directory() {
  local directory="${1%/*}"
  if [[ "$directory" == "$1" ]]; then
    printf '%s\n' .
  elif [[ -z "$directory" ]]; then
    printf '%s\n' /
  else
    printf '%s\n' "$directory"
  fi
}

normalize_https_url() {
  case "$1" in
    https://*) printf '%s\n' "$1" ;;
    http://*) printf 'https://%s\n' "${1#http://}" ;;
    *) return 1 ;;
  esac
}

retarget_download_url() {
  local rest path
  if [[ -z "${MUSE_DOWNLOAD_HOST:-}" ]]; then
    printf '%s\n' "$1"
    return 0
  fi
  rest="${1#https://}"
  case "$rest" in
    */*) path="/${rest#*/}" ;;
    *) path="" ;;
  esac
  printf 'https://%s%s\n' "$download_host" "$path"
}

resolve_self() {
  local self target directory
  self="${BASH_SOURCE[0]}"
  while [[ -L "$self" ]]; do
    target="$(readlink "$self")"
    if [[ "$target" == /* ]]; then
      self="$target"
    else
      self="$(parent_directory "$self")/$target"
    fi
  done
  directory="$(cd -- "$(parent_directory "$self")" && pwd)" || return 1
  printf '%s/%s\n' "$directory" "${self##*/}"
}

resolve_install_dir() {
  parent_directory "$(resolve_self)"
}

detect_platform() {
  case "$(uname -s):$(uname -m)" in
    Darwin:arm64|Darwin:aarch64) printf '%s\n' aarch64_macos ;;
    Darwin:x86_64|Darwin:amd64) printf '%s\n' x86_macos ;;
    Linux:arm64|Linux:aarch64) printf '%s\n' aarch64_linux ;;
    Linux:x86_64|Linux:amd64) printf '%s\n' x86_linux ;;
    *) die "unsupported platform: $(uname -s) $(uname -m)" ;;
  esac
}

sha256_file() {
  local output
  if command -v sha256sum >/dev/null 2>&1; then
    output="$(sha256sum "$1")" || return 1
  else
    output="$(shasum -a 256 "$1")" || return 1
  fi
  printf '%s\n' "${output%% *}"
}

human_size() {
  local bytes="$1"
  if ((bytes >= 1073741824)); then
    printf '%d GB\n' $(((bytes + 536870912) / 1073741824))
  elif ((bytes >= 1048576)); then
    printf '%d MB\n' $(((bytes + 524288) / 1048576))
  elif ((bytes >= 1024)); then
    printf '%d KB\n' $(((bytes + 512) / 1024))
  else
    printf '%d B\n' "$bytes"
  fi
}

_json_text=""
_json_i=0
json_document=""
json_value=""
json_type=""
json_start=0
json_end=0

json_max_bytes=8192

_json_ws() {
  local length=${#_json_text}
  while ((_json_i < length)); do
    case "${_json_text:_json_i:1}" in
      ' '|$'\t'|$'\n'|$'\r') _json_i=$((_json_i + 1)) ;;
      *) break ;;
    esac
  done
  return 0
}

_json_string() {
  local length=${#_json_text} character escape decoded="" rest chunk
  [[ "${_json_text:_json_i:1}" == '"' ]] || return 1
  _json_i=$((_json_i + 1))

  rest="${_json_text:_json_i}"
  chunk="${rest%%\"*}"
  if [[ "$chunk" != "$rest" && "$chunk" != *\\* ]]; then
    _json_i=$((_json_i + ${#chunk} + 1))
    json_value="$chunk"
    return 0
  fi

  while ((_json_i < length)); do
    character="${_json_text:_json_i:1}"
    case "$character" in
      '"')
        _json_i=$((_json_i + 1))
        json_value="$decoded"
        return 0
        ;;
      \\)
        _json_i=$((_json_i + 1))
        character="${_json_text:_json_i:1}"
        case "$character" in
          '"'|\\|/) decoded="$decoded$character" ;;
          b) decoded="$decoded"$'\b' ;;
          f) decoded="$decoded"$'\f' ;;
          n) decoded="$decoded"$'\n' ;;
          r) decoded="$decoded"$'\r' ;;
          t) decoded="$decoded"$'\t' ;;
          u)
            escape="${_json_text:_json_i + 1:4}"
            [[ "$escape" =~ ^00[2-7][0-9a-fA-F]$ ]] || return 1
            printf -v character '%b' "\\x${escape:2:2}"
            [[ "$character" != $'\x7f' ]] || return 1
            decoded="$decoded$character"
            _json_i=$((_json_i + 4))
            ;;
          *) return 1 ;;
        esac
        _json_i=$((_json_i + 1))
        ;;
      *)
        decoded="$decoded$character"
        _json_i=$((_json_i + 1))
        ;;
    esac
  done
  return 1
}

_json_skip() {
  local length=${#_json_text} closers=""
  _json_ws
  case "${_json_text:_json_i:1}" in
    '"')
      _json_string || return 1
      return 0
      ;;
    '{'|'[')
      while ((_json_i < length)); do
        case "${_json_text:_json_i:1}" in
          '"')
            _json_string || return 1
            continue
            ;;
          '{') closers="}$closers" ;;
          '[') closers="]$closers" ;;
          '}'|']')
            [[ "${closers:0:1}" == "${_json_text:_json_i:1}" ]] || return 1
            closers="${closers:1}"
            ;;
        esac
        _json_i=$((_json_i + 1))
        [[ -n "$closers" ]] || return 0
      done
      return 1
      ;;
    *)
      while ((_json_i < length)); do
        case "${_json_text:_json_i:1}" in
          ,|'}'|']'|' '|$'\t'|$'\n'|$'\r') break ;;
          *) _json_i=$((_json_i + 1)) ;;
        esac
      done
      return 0
      ;;
  esac
}

json_member() {
  local key="$3" name start
  _json_text="$1"
  _json_i="$2"
  _json_ws
  [[ "${_json_text:_json_i:1}" == '{' ]] || return 1
  _json_i=$((_json_i + 1))
  while :; do
    _json_ws
    [[ "${_json_text:_json_i:1}" == '"' ]] || return 1
    _json_string || return 1
    name="$json_value"
    _json_ws
    [[ "${_json_text:_json_i:1}" == ':' ]] || return 1
    _json_i=$((_json_i + 1))
    _json_ws
    start=$_json_i
    if [[ "$name" == "$key" ]]; then
      case "${_json_text:_json_i:1}" in
        '"')
          _json_string || return 1
          json_type=string
          ;;
        '{')
          _json_skip || return 1
          json_type=object
          json_value=""
          ;;
        '[')
          _json_skip || return 1
          json_type=array
          json_value=""
          ;;
        *)
          _json_skip || return 1
          json_value="${_json_text:start:_json_i - start}"
          if [[ "$json_value" =~ ^-?[0-9]+$ ]]; then
            json_type=number
          else
            json_type=literal
          fi
          ;;
      esac
      json_start=$start
      json_end=$_json_i
      return 0
    fi
    _json_skip || return 1
    _json_ws
    [[ "${_json_text:_json_i:1}" == ',' ]] || return 1
    _json_i=$((_json_i + 1))
  done
}

json_is_object() {
  _json_text="$1"
  _json_i=0
  _json_ws
  [[ "${_json_text:_json_i:1}" == '{' ]]
}

json_is_valid() {
  _json_text="$1"
  _json_i=0
  _json_skip || return 1
  _json_ws
  ((_json_i >= ${#_json_text}))
}

json_load() {
  [[ -r "$1" ]] || return 1
  json_document="$(<"$1")" || return 1
  ((${#json_document} <= json_max_bytes)) || return 1
  json_is_valid "$json_document" || return 1
  json_is_object "$json_document"
}

json_field() {
  json_member "$1" 0 "$2" || return 1
  case "$json_type" in
    string|number) return 0 ;;
  esac
  return 1
}

json_is_clean() {
  case "$1" in
    *$'\t'*|*$'\r'*|*$'\n'*|*$'\x1f'*) return 1 ;;
  esac
  return 0
}

parse_channel_manifest() {
  local document version url urgency
  json_load "$1" || return 1
  document="$json_document"
  json_field "$document" channel || return 1
  [[ "$json_value" == "$channel" ]] || return 1
  json_field "$document" version || return 1
  [[ "$json_type" == string ]] || return 1
  version="$json_value"
  [[ "$version" =~ $version_pattern ]] || return 1
  json_field "$document" manifest_url || return 1
  [[ "$json_type" == string ]] || return 1
  url="$json_value"
  [[ -n "$url" ]] || return 1
  json_is_clean "$url" || return 1
  json_field "$document" urgency || return 1
  [[ "$json_type" == string ]] || return 1
  urgency="$json_value"
  case "$urgency" in
    none|encouraged|must_download) ;;
    *) return 1 ;;
  esac
  json_field "$document" notification_text || return 1
  [[ "$json_type" == string ]] || return 1
  printf '%s\t%s\n' "$version" "$url"
}

parse_release_manifest() {
  local manifest="$1"
  local expected_version="$2"
  local platform="$3"
  local document artifacts artifact url checksum size
  json_load "$manifest" || return 1
  document="$json_document"
  json_field "$document" version || return 1
  [[ "$json_value" == "$expected_version" ]] || return 1
  json_field "$document" checksum_algorithm || return 1
  [[ "$json_value" == sha256 ]] || return 1

  json_member "$document" 0 artifacts || return 1
  [[ "$json_type" == object ]] || return 1
  artifacts=$json_start
  json_member "$document" "$artifacts" "$platform" || return 1
  [[ "$json_type" == object ]] || return 1
  artifact=$json_start

  json_member "$document" "$artifact" url || return 1
  [[ "$json_type" == string && -n "$json_value" ]] || return 1
  url="$json_value"
  json_is_clean "$url" || return 1
  json_member "$document" "$artifact" checksum || return 1
  [[ "$json_type" == string ]] || return 1
  checksum="$json_value"
  [[ "$checksum" =~ ^[0-9a-f]{64}$ ]] || return 1
  json_member "$document" "$artifact" size || return 1
  [[ "$json_type" == number ]] || return 1
  size="$json_value"
  [[ "$size" =~ ^[1-9][0-9]*$ ]] || return 1

  printf '%s\t%s\t%s\n' "$url" "$checksum" "$size"
}

json_fields() {
  local source="$1"
  shift
  local document name value fields="" first=1
  json_load "$source" || return 1
  document="$json_document"
  for name in "$@"; do
    value=""
    if json_member "$document" 0 "$name"; then
      case "$json_type" in
        string|number)
          json_is_clean "$json_value" || return 1
          value="$json_value"
          ;;
      esac
    fi
    if ((first)); then
      fields="$value"
      first=0
    else
      fields="$fields"$'\x1f'"$value"
    fi
  done
  printf '%s\n' "$fields"
}

auth_post() {
  local endpoint="$1"
  local destination="$2"
  shift 2
  local field
  local form=()
  for field in "$@"; do
    form+=(--data-urlencode "$field")
  done
  curl \
    --silent \
    --show-error \
    --max-redirs 0 \
    --proto '=https' \
    --tlsv1.2 \
    --user-agent "$user_agent" \
    --header 'Accept: application/json' \
    "${form[@]+"${form[@]}"}" \
    --output "$destination" \
    --write-out '%{http_code}' \
    "$auth_url$endpoint"
}

open_verification_page() {
  local url="$1"
  if [[ "${OSTYPE:-}" == darwin* ]] && command -v open >/dev/null 2>&1; then
    open "$url" >/dev/null 2>&1
  elif command -v xdg-open >/dev/null 2>&1; then
    xdg-open "$url" >/dev/null 2>&1
  else
    return 1
  fi
}

# Waits up to 30 seconds for an Enter, then moves on so an unattended terminal
# still reaches the poll loop. The answer is read from /dev/tty, not stdin:
# under `curl | bash` stdin is the script being piped in, so reading it would
# consume the installer instead of waiting for a key.
confirm_open_verification_page() {
  local url="$1" answer
  [[ -t 2 && -r /dev/tty ]] || return 0
  printf 'Press Enter to open it in your browser: ' >&2
  if ! IFS= read -r -t 30 answer </dev/tty; then
    printf '\n' >&2
    return 0
  fi
  if open_verification_page "$url"; then
    printf 'Opening it now; if nothing happens, use the link above.\n' >&2
  else
    printf 'No browser could be opened; use the link above.\n' >&2
  fi
}

# Read-only view of the CLI's credential store. The launcher never writes it:
# the token from its own device login stays in memory for the run, so a caller
# who signs in here gains nothing on disk that the CLI did not put there.
read_credential() {
  local document slot access expires=""
  [[ -n "$credential_path" ]] || return 1
  json_load "$credential_path" || return 1
  document="$json_document"
  json_member "$document" 0 providers || return 1
  [[ "$json_type" == object ]] || return 1
  slot=$json_start
  json_member "$document" "$slot" meta || return 1
  [[ "$json_type" == object ]] || return 1
  slot=$json_start
  json_member "$document" "$slot" mechanism || return 1
  [[ "$json_type" == string && "$json_value" == oauth ]] || return 1
  json_member "$document" "$slot" access_token || return 1
  [[ "$json_type" == string && -n "$json_value" ]] || return 1
  access="$json_value"
  json_is_clean "$access" || return 1
  if json_member "$document" "$slot" expires_at && \
      [[ "$json_type" == number && "$json_value" =~ ^[0-9]+$ ]]; then
    expires="$json_value"
  fi
  printf '%s\x1f%s\n' "$access" "$expires"
}

grant_access_token() {
  local access
  access="$(json_fields "$1" access_token)" || return 1
  [[ -n "$access" ]] || return 1
  printf '%s\n' "$access"
}

ensure_access_token() {
  local fields access expires_at
  if [[ -n "$access_token" ]]; then
    printf '%s\n' "$access_token"
    return 0
  fi
  fields="$(read_credential)" || return 1
  IFS=$'\x1f' read -r access expires_at <<<"$fields"
  [[ -n "$access" ]] || return 1
  printf '%s\n' "$access"
}

download_auth_header() {
  local token
  [[ "$skip_download_auth" == 0 ]] || return 0
  case "$1" in
    "https://$download_host/"*) ;;
    *) return 0 ;;
  esac
  token="$(ensure_access_token 2>/dev/null)" || return 0
  [[ -n "$token" ]] || return 0
  printf 'Authorization: Bearer %s\n' "$token"
}

# Emits the granted access token on stdout; every human-facing line goes to
# stderr, so the caller can capture the token without swallowing the prompts.
device_login() (
  local auth_work="" response status fields
  local device_code user_code verification_uri verification_uri_complete
  local lifetime interval deadline error
  local minutes expiry bold="" normal=""

  trap '[[ -z "$auth_work" ]] || rm -rf "$auth_work"' EXIT
  auth_work="$(mktemp -d "${TMPDIR:-/tmp}/muse-login.XXXXXX")" || return 1
  response="$auth_work/device.json"

  status="$(auth_post "$authorization_endpoint" "$response" \
    "client_id=$client_id")" || return 1
  case "$status" in
    2??) ;;
    3??)
      printf 'muse: the sign-in service redirected the request; not following it\n' >&2
      return 1
      ;;
    404)
      printf 'muse: sign-in is not available yet\n' >&2
      return 1
      ;;
    *)
      printf 'muse: sign-in could not be started (HTTP %s)\n' "$status" >&2
      return 1
      ;;
  esac

  fields="$(json_fields "$response" \
    device_code \
    user_code \
    verification_uri \
    verification_uri_complete \
    expires_in \
    interval)" || return 1
  IFS=$'\x1f' read -r device_code user_code verification_uri \
    verification_uri_complete lifetime interval <<<"$fields"
  [[ -n "$device_code" && -n "$user_code" ]] || return 1
  verification_uri="$(normalize_https_url "$verification_uri")" || return 1
  if [[ -n "$verification_uri_complete" ]]; then
    verification_uri_complete="$(
      normalize_https_url "$verification_uri_complete"
    )" || verification_uri_complete=""
  fi
  [[ "$interval" =~ ^[0-9]+$ ]] && ((interval > 0)) || interval=5
  [[ "$lifetime" =~ ^[0-9]+$ ]] || lifetime=900
  ((lifetime >= 60)) || lifetime=60
  ((lifetime <= 1800)) || lifetime=1800
  deadline=$(($(date +%s) + lifetime))

  if [[ -t 2 && -z "${NO_COLOR:-}" ]]; then
    if ! { bold="$(tput bold 2>/dev/null)" && \
        normal="$(tput sgr0 2>/dev/null)"; }; then
      bold=""
      normal=""
    fi
  fi

  printf '\n' >&2
  printf 'Open this page to sign in:\n' >&2
  printf '  %s\n' "${verification_uri_complete:-$verification_uri}" >&2
  if [[ -n "$verification_uri_complete" ]]; then
    printf 'Confirm this code matches:\n' >&2
  else
    printf 'Enter this code:\n' >&2
  fi
  printf '  %s%s%s\n' "$bold" "$user_code" "$normal" >&2
  printf '\n' >&2
  confirm_open_verification_page \
    "${verification_uri_complete:-$verification_uri}"
  minutes=$(((lifetime + 59) / 60))
  expiry="$minutes minutes"
  ((minutes != 1)) || expiry='1 minute'
  printf 'Waiting for approval (link expires in %s)... ' "$expiry" >&2

  response="$auth_work/token.json"
  while :; do
    if (($(date +%s) >= deadline)); then
      printf '\nmuse: the sign-in request expired before it was approved\n' >&2
      return 1
    fi
    status="$(auth_post "$token_endpoint" "$response" \
      "grant_type=$device_code_grant" \
      "device_code=$device_code" \
      "client_id=$client_id")" || { printf '\n' >&2; return 1; }
    case "$status" in
      2??)
        grant_access_token "$response" || {
          printf '\nmuse: the sign-in response carried no usable token\n' >&2
          return 1
        }
        printf 'Signed in.\n' >&2
        return 0
        ;;
      3??)
        printf '\nmuse: the sign-in service redirected the request; not following it\n' >&2
        return 1
        ;;
    esac
    error="$(json_fields "$response" error 2>/dev/null || true)"
    case "$error" in
      authorization_pending) ;;
      slow_down) interval=$((interval + 5)) ;;
      access_denied)
        printf '\nmuse: the sign-in request was denied\n' >&2
        return 1
        ;;
      expired_token)
        printf '\nmuse: the sign-in request expired before it was approved\n' >&2
        return 1
        ;;
      *)
        printf '\nmuse: sign-in failed (HTTP %s)\n' "$status" >&2
        return 1
        ;;
    esac
    sleep "$interval"
  done
)

login_after_denial() {
  if [[ "${MUSE_LOGIN:-}" == 0 ]]; then
    return 1
  fi
  if [[ "${MUSE_LOGIN:-}" != 1 && ! -t 2 ]]; then
    return 1
  fi
  access_token="$(device_login)" || {
    access_token=""
    return 1
  }
}

fetch_json() {
  local url="$1"
  local destination="$2"
  local header
  local extra=()
  header="$(download_auth_header "$url")"
  [[ -z "$header" ]] || extra=(--header "$header")
  curl \
    --silent \
    --show-error \
    --location \
    --max-redirs 3 \
    --proto '=https' \
    --proto-redir '=https' \
    --tlsv1.2 \
    --user-agent "$user_agent" \
    --header 'Accept: application/json' \
    "${extra[@]+"${extra[@]}"}" \
    --output "$destination" \
    --write-out '%{http_code}' \
    "$url"
}

fetch_launcher() {
  curl \
    --silent \
    --show-error \
    --location \
    --max-redirs 3 \
    --proto '=https' \
    --proto-redir '=https' \
    --tlsv1.2 \
    --connect-timeout 10 \
    --max-time 60 \
    --user-agent "$user_agent" \
    --dump-header "$3" \
    --output "$2" \
    --write-out '%{http_code}' \
    "$1"
}

fetch_artifact() {
  local url="$1"
  local destination="$2"
  local header
  local extra=()
  header="$(download_auth_header "$url")"
  [[ -z "$header" ]] || extra=(--header "$header")
  if [[ -t 2 ]]; then
    curl \
      --show-error \
      --location \
      --max-redirs 3 \
      --proto '=https' \
      --proto-redir '=https' \
      --tlsv1.2 \
      --user-agent "$user_agent" \
      --progress-bar \
      "${extra[@]+"${extra[@]}"}" \
      --output "$destination" \
      --write-out '%{http_code}' \
      "$url"
  else
    curl \
      --silent \
      --show-error \
      --location \
      --max-redirs 3 \
      --proto '=https' \
      --proto-redir '=https' \
      --tlsv1.2 \
      --user-agent "$user_agent" \
      "${extra[@]+"${extra[@]}"}" \
      --output "$destination" \
      --write-out '%{http_code}' \
      "$url"
  fi
}

is_auth_denial() {
  case "$1" in
    401|403|404) return 0 ;;
    *) return 1 ;;
  esac
}

fetch_json_with_login() {
  local url="$1" destination="$2" had_auth=0
  [[ -z "$(download_auth_header "$url")" ]] || had_auth=1
  skip_download_auth=1
  request_status="$(fetch_json "$url" "$destination")" || {
    skip_download_auth=0
    return 1
  }
  skip_download_auth=0
  if is_auth_denial "$request_status" && ((had_auth != 0)); then
    request_status="$(fetch_json "$url" "$destination")" || {
      return 1
    }
  fi
  if is_auth_denial "$request_status" && login_after_denial; then
    request_status="$(fetch_json "$url" "$destination")" || return 1
  fi
}

fetch_artifact_with_login() {
  local url="$1" destination="$2" had_auth=0
  [[ -z "$(download_auth_header "$url")" ]] || had_auth=1
  skip_download_auth=1
  request_status="$(fetch_artifact "$url" "$destination")" || {
    skip_download_auth=0
    return 1
  }
  skip_download_auth=0
  if is_auth_denial "$request_status" && ((had_auth != 0)); then
    request_status="$(fetch_artifact "$url" "$destination")" || {
      return 1
    }
  fi
  if is_auth_denial "$request_status" && login_after_denial; then
    request_status="$(fetch_artifact "$url" "$destination")" || return 1
  fi
}

die_http() {
  local status="$1"
  local url="$2"
  case "$status" in
    401)
      die "the download was rejected (HTTP 401); rerun the installer to sign in again"
      ;;
    403)
      die "the download was refused (HTTP 403); muse is available to Meta employees"
      ;;
    *) die "request failed (HTTP $status): $url" ;;
  esac
}

acquire_lock() {
  local dir="$1"
  local lock_pid
  lock_dir="$dir/.muse-update-lock"
  if [[ -d "$lock_dir" ]]; then
    lock_pid=""
    [[ ! -r "$lock_dir/pid" ]] || lock_pid="$(<"$lock_dir/pid")"
    if [[ ! "$lock_pid" =~ ^[0-9]+$ ]] || \
        ! kill -0 "$lock_pid" 2>/dev/null; then
      rm -rf -- "$lock_dir"
    fi
  fi
  mkdir -- "$lock_dir" 2>/dev/null || return 1
  printf '%s\n' "$$" >"$lock_dir/pid"
}

# Final response only: --location dumps one header block per response.
read_response_header() {
  local file="$1" wanted="$2" line name value=""
  [[ -r "$file" ]] || return 0
  while IFS= read -r line; do
    line="${line%$'\r'}"
    if [[ "$line" == HTTP/* ]]; then
      value=""
      continue
    fi
    [[ "$line" == *:* ]] || continue
    name="$(printf '%s' "${line%%:*}" | tr '[:upper:]' '[:lower:]')"
    [[ "$name" == "$wanted" ]] || continue
    value="${line#*:}"
  done <"$file"
  while [[ "$value" == [[:space:]]* ]]; do value="${value#?}"; done
  while [[ "$value" == *[[:space:]] ]]; do value="${value%?}"; done
  printf '%s\n' "$value"
}

# Replaces this script when a newer one is published. Rename, never write:
# bash reads a script as it runs, so overwriting a live launcher corrupts it.
# Best effort -- any failure leaves the current launcher in place.
_update_launcher() {
  local self="$1" dir published current advertised status
  dir="$(parent_directory "$self")"
  [[ -w "$self" && -w "$dir" ]] || return 1
  launcher_tmp="$(mktemp "$dir/.muse-launcher.XXXXXX")" || return 1
  launcher_headers="$launcher_tmp.headers"
  status="$(fetch_launcher "$launcher_url" "$launcher_tmp" \
    "$launcher_headers")" || return 1
  [[ "$status" == 2?? ]] || return 1

  # A body that cannot parse must never reach the launcher; there is no recovery.
  bash -n "$launcher_tmp" 2>/dev/null || return 1

  published="$(sha256_file "$launcher_tmp")" || return 1
  published="$(printf '%s' "$published" | tr '[:upper:]' '[:lower:]')"

  # Verified when advertised; older serving code sends none.
  advertised="$(read_response_header "$launcher_headers" x-content-sha256)"
  advertised="$(printf '%s' "$advertised" | tr '[:upper:]' '[:lower:]')"
  if [[ "$advertised" =~ ^[0-9a-f]{64}$ ]]; then
    [[ "$published" == "$advertised" ]] || return 1
  fi

  current="$(sha256_file "$self")" || return 1
  current="$(printf '%s' "$current" | tr '[:upper:]' '[:lower:]')"
  [[ "$published" != "$current" ]] || return 1

  cp -f -- "$self" "$dir/.muse-launcher.previous" 2>/dev/null || true
  # Checked explicitly: set -e is off here, this runs as the left side of `||`.
  chmod -- 0755 "$launcher_tmp" || return 1
  mv -f -- "$launcher_tmp" "$self" || return 1
  launcher_tmp=""
  return 0
}

# Subshell with traps, like update_binary: an interrupted download must not
# strand temp files in the install dir, and nothing in here may stop the run --
# `|| true` alone does not contain a set -u abort.
discard_launcher_download() {
  [[ -z "$launcher_tmp" ]] || rm -f -- "$launcher_tmp"
  launcher_tmp=""
  [[ -z "$launcher_headers" ]] || rm -f -- "$launcher_headers"
  launcher_headers=""
}

update_launcher() (
  trap discard_launcher_download EXIT
  trap 'exit 129' HUP
  trap 'exit 130' INT
  trap 'exit 143' TERM
  _update_launcher "$1" || true
)

# Background updates finish after the handoff, so leave a note for the next run.
# An undelivered note keeps its original starting version.
record_update_notice() {
  local dir="$1" from="$2" to="$3"
  local notice pending
  [[ -n "$from" && "$from" != "$to" ]] || return 0
  notice="$dir/.muse-update-notice"
  pending=""
  [[ ! -r "$notice" ]] || pending="$(<"$notice")"
  [[ "$pending" != *$'\t'* ]] || from="${pending%%$'\t'*}"
  if [[ "$from" == "$to" ]]; then
    rm -f -- "$notice"
    return
  fi
  notice_tmp="$notice.tmp.$$"
  if ! printf '%s\t%s\n' "$from" "$to" >"$notice_tmp"; then
    rm -f -- "$notice_tmp"
    notice_tmp=""
    return 1
  fi
  if ! mv -f -- "$notice_tmp" "$notice"; then
    rm -f -- "$notice_tmp"
    notice_tmp=""
    return 1
  fi
  notice_tmp=""
}

publish_release_info() {
  local dir="$1" source="$2" release_info
  release_info="$dir/.muse-release-info.json"
  release_info_tmp="$release_info.tmp.$$"
  mv -f -- "$source" "$release_info_tmp"
  mv -f -- "$release_info_tmp" "$release_info"
  release_info_tmp=""
}

update_binary() (
  local dir="$1"
  local state channel_manifest release_manifest candidate status
  local manifest_values version manifest_url installed_version target
  local platform artifact_url expected_checksum expected_size
  local actual_size actual_checksum old_binary

  trap cleanup EXIT
  trap 'exit 129' HUP
  trap 'exit 130' INT
  trap 'exit 143' TERM
  acquire_lock "$dir" || return 0

  for required_command in curl date mktemp uname wc; do
    command -v "$required_command" >/dev/null 2>&1 || \
      die "required command not found: $required_command"
  done
  if ! command -v sha256sum >/dev/null 2>&1 && \
      ! command -v shasum >/dev/null 2>&1; then
    die "sha256sum or shasum is required"
  fi

  state="$dir/.muse-version"
  work="$(mktemp -d "$dir/.muse-update.XXXXXX")"
  channel_manifest="$work/channel.json"
  release_manifest="$work/release.json"
  candidate="$work/muse-bin"

  status="$(fetch_json "$channel_url" "$channel_manifest")" || \
    die "could not reach $channel_url"
  [[ "$status" == 2?? ]] || die_http "$status" "$channel_url"
  manifest_values="$(parse_channel_manifest "$channel_manifest")" || \
    die "invalid channel manifest"
  IFS=$'\t' read -r version manifest_url <<<"$manifest_values"
  target="$dir/muse-bin-$version"
  installed_version=""
  [[ ! -r "$state" ]] || installed_version="$(<"$state")"
  if [[ "$version" == "$installed_version" && -x "$target" ]]; then
    publish_release_info "$dir" "$channel_manifest"
    return 0
  fi

  manifest_url="$(retarget_download_url "$manifest_url")"
  fetch_json_with_login "$manifest_url" "$release_manifest" || \
    die "could not reach $manifest_url"
  status="$request_status"
  [[ "$status" == 2?? ]] || die_http "$status" "$manifest_url"
  platform="$(detect_platform)"
  manifest_values="$(parse_release_manifest \
    "$release_manifest" "$version" "$platform")" || \
    die "invalid release manifest for $version and $platform"
  IFS=$'\t' read -r artifact_url expected_checksum expected_size \
    <<<"$manifest_values"
  artifact_url="$(retarget_download_url "$artifact_url")"
  if [[ -t 2 ]]; then
    printf '\nDownloading muse %s (%s)\n' \
      "$version" "$(human_size "$expected_size")" >&2
  fi
  fetch_artifact_with_login "$artifact_url" "$candidate" || \
    die "could not reach $artifact_url"
  status="$request_status"
  [[ "$status" == 2?? ]] || die_http "$status" "$artifact_url"
  actual_size="$(wc -c <"$candidate")"
  actual_size="${actual_size//[[:space:]]/}"
  [[ "$actual_size" == "$expected_size" ]] || \
    die "artifact size mismatch: expected $expected_size, got $actual_size"
  actual_checksum="$(sha256_file "$candidate")"
  [[ "$actual_checksum" == "$expected_checksum" ]] || \
    die "artifact checksum mismatch"

  chmod -- 0755 "$candidate"
  mv -f -- "$candidate" "$target"
  publish_release_info "$dir" "$channel_manifest"
  state_tmp="$state.tmp.$$"
  printf '%s\n' "$version" >"$state_tmp"
  mv -f -- "$state_tmp" "$state"
  state_tmp=""
  record_update_notice "$dir" "$installed_version" "$version"

  shopt -s nullglob
  for old_binary in "$dir"/muse-bin-*; do
    [[ "$old_binary" == "$target" ]] || rm -f "$old_binary"
  done
  rm -f -- "$dir/muse-bin"
)

active_binary() {
  local dir="$1"
  local version
  version=""
  [[ ! -r "$dir/.muse-version" ]] || version="$(<"$dir/.muse-version")"
  [[ "$version" =~ $version_pattern ]] || return 1
  printf '%s/muse-bin-%s\n' "$dir" "$version"
}

release_info_for_active_binary() {
  local dir="$1" state release_info installed_version
  local manifest_values version manifest_url
  state="$dir/.muse-version"
  release_info="$dir/.muse-release-info.json"
  [[ -r "$state" && -r "$release_info" ]] || return 1
  installed_version="$(<"$state")"
  manifest_values="$(parse_channel_manifest "$release_info")" || return 1
  IFS=$'\t' read -r version manifest_url <<<"$manifest_values"
  [[ "$version" == "$installed_version" ]] || return 1
  printf '%s\n' "$(<"$release_info")"
}

# Claims a note atomically. Malformed notes are consumed so they cannot wedge.
pending_update_notice() {
  local dir="$1" notice claimed pending from to
  notice="$dir/.muse-update-notice"
  [[ -r "$notice" ]] || return 1
  claimed="$notice.take.$$"
  mv -f -- "$notice" "$claimed" 2>/dev/null || return 1
  pending="$(<"$claimed")"
  rm -f -- "$claimed" || return 1
  [[ "$pending" == *$'\t'* ]] || return 1
  from="${pending%%$'\t'*}"
  to="${pending#*$'\t'}"
  [[ -n "$from" && -n "$to" && "$from" != "$to" ]] || return 1
  printf 'Muse Code updated %s -> %s\n' "$from" "$to"
}

report_update_notice() {
  local message
  # Only claim it when it can be seen; otherwise leave it for the next run.
  [[ -t 2 ]] || return 0
  message="$(pending_update_notice "$1")" || return 0
  printf '%s\n' "$message" >&2
}

should_check_for_update() {
  local checked_at="$1"
  local now previous
  [[ "${MUSE_SYNC_UPDATE:-0}" == 1 ]] && return 0
  [[ "$update_interval" =~ ^[0-9]+$ ]] || return 1
  now="$(date +%s)"
  previous=""
  [[ ! -r "$checked_at" ]] || previous="$(<"$checked_at")"
  [[ "$previous" =~ ^[0-9]+$ ]] || return 0
  ((now - previous >= update_interval))
}

main() {
  local dir binary checked_at checked_tmp now release_info
  dir="$(resolve_install_dir)"
  checked_at="$dir/.muse-update-checked-at"

  if [[ "${MUSE_LAUNCHER_INSTALL:-0}" == 1 ]]; then
    update_binary "$dir"
    binary="$(active_binary "$dir")" || \
      die "installation did not produce an active binary"
    [[ -x "$binary" ]] || die "installed binary is missing: $binary"
    report_update_notice "$dir"
    exit 0
  fi

  binary="$(active_binary "$dir" 2>/dev/null || true)"
  if [[ ! -x "$binary" ]]; then
    if [[ "${MUSE_NO_AUTO_UPDATE:-0}" == 1 ]]; then
      die "installed binary is missing; rerun the installer"
    fi
    update_launcher "$(resolve_self)" || true
    update_binary "$dir"
    binary="$(active_binary "$dir")" || \
      die "installed binary is missing; rerun the installer"
  elif [[ "${MUSE_NO_AUTO_UPDATE:-0}" != 1 ]] && \
      should_check_for_update "$checked_at"; then
    now="$(date +%s)"
    checked_tmp="$checked_at.tmp.$$"
    if printf '%s\n' "$now" >"$checked_tmp" && \
        mv -f -- "$checked_tmp" "$checked_at"; then
      if [[ "${MUSE_SYNC_UPDATE:-0}" == 1 ]]; then
        update_launcher "$(resolve_self)" || true
        update_binary "$dir"
        binary="$(active_binary "$dir")" || \
          die "installed binary is missing; rerun the installer"
      else
        ({ update_launcher "$(resolve_self)" || true; update_binary "$dir"; } \
          </dev/null >/dev/null 2>&1 &)
      fi
    fi
  fi

  report_update_notice "$dir"
  release_info="$(release_info_for_active_binary "$dir" 2>/dev/null || true)"
  if [[ -n "$release_info" ]]; then
    export MUSE_RELEASE_INFO="$release_info"
  else
    unset MUSE_RELEASE_INFO
  fi
  exec "$binary" "$@"
}

main "$@"
