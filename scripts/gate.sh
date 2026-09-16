#!/usr/bin/env bash
# The gate check set (PLAN.md): formatting, lints and every test of every
# crate against the pinned host binary. Run from anywhere. Every step runs
# even after a red one (`cargo test --no-fail-fast`, and the steps are not
# chained), so one red crate never hides the others; the per-crate summary
# at the end names each test target with its counts, then the gate exits
# non-zero if anything was red.
#
#   OMM_MUSE_BIN=/path/to/.host/bin/muse-bin-<version> scripts/gate.sh
#
# `OMM_MUSE_BIN` defaults to the newest `.host/bin/muse-bin-*` of this checkout.
set -uo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
export PATH="$HOME/.cargo/bin:$PATH"

if [[ -z "${OMM_MUSE_BIN:-}" ]]; then
  candidate="$(ls -1 "$root"/.host/bin/muse-bin-* 2>/dev/null | sort -V | tail -n 1 || true)"
  if [[ -n "$candidate" ]]; then
    export OMM_MUSE_BIN="$candidate"
  else
    echo "gate: OMM_MUSE_BIN is unset and .host/bin/ holds no muse-bin-*; host tests will be skipped" >&2
  fi
fi

red=0
declare -a steps=()
step() {
  echo "== $*"
  if "$@"; then
    steps+=("ok    $*")
  else
    steps+=("RED   $*")
    red=1
  fi
}

step cargo fmt --all -- --check
step cargo clippy --workspace --all-targets -- -D warnings

# Tests: every crate, every target, no fail-fast; the log feeds the summary.
log="$(mktemp -t omm-gate.XXXXXX)"
trap 'rm -f "$log"' EXIT
echo "== cargo test --workspace --no-fail-fast"
if cargo test --workspace --no-fail-fast 2>&1 | tee "$log"; then
  steps+=("ok    cargo test --workspace --no-fail-fast")
else
  steps+=("RED   cargo test --workspace --no-fail-fast")
  red=1
fi

echo
echo "== gate summary"
for s in "${steps[@]}"; do
  echo "  $s"
done
echo "  per test target (crate/target: result):"
# Pair every `Running <target> (<path>)` / `Doc-tests <crate>` line with the
# `test result:` line that follows it.
awk '
  /^ +Running / {
    target = $2
    if (target == "unittests") { target = $3 }
    p = $NF
    sub(/^\(/, "", p)
    sub(/\)$/, "", p)
    n = split(p, parts, "/")
    crate = parts[n]
    sub(/-[0-9a-f]+$/, "", crate)
    name = crate ": " target
    next
  }
  /^ +Doc-tests / { name = $2 ": doc-tests"; next }
  /^test result: / {
    status = $3
    sub(/\.$/, "", status)
    printf "    %-8s %-60s %s %s %s %s\n", status, name, $4, $5, $6, $7
  }
' "$log"
if [[ "$red" -ne 0 ]]; then
  echo "gate: RED"
  exit 1
fi
echo "gate: green"
