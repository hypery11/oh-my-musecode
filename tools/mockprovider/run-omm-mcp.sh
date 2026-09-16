#!/bin/bash
# End-to-end proof of `omm mcp` (PLAN.md 2.2): the REAL bundle, installed by the REAL installer,
# its stdio MCP server spawned by the host through PATH, and the scripted model calling
# `mcp__plugin_oh_my_musecode_doc.omm_doctor` — the doctor JSON must come back in the next provider request.
#
#   usage: OMM_MUSE_BIN=/path/to/muse-bin bash run-omm-mcp.sh [work-dir]
#   env:   OMM_BIN      the omm binary (default: <repo>/target/release/omm; must exist)
#          MOCK_PORT    (default 8741)
#          TIMEOUT_S    (default 120) — hard kill for the muse run
#          PLAN_FILE    a plan.json for respond-toolcall.py (default: omm_doctor {fast:true}, omm_cost, text)
#          MAX_STEPS    --max-model-steps for exec (default 8)
#          PROMPT       the user prompt (non-trivial: a bare "hi" is answered locally)
#
# What it does (each step leaves its evidence under <work-dir>):
#   1. a throwaway HOME whose XDG roots are HOME's defaults (~/.config, ~/.local/share): the server the
#      host spawns sees only the 16-key scrubbed environment — HOME and PATH, no XDG_*, no OMM_MUSE_BIN —
#      and must resolve the same roots as the installer did
#   2. the Muse binary reachable ONLY the way a real install is: ~/.local/bin/.muse-version naming a
#      muse-bin-<version> beside it (omm_host::locate's launcher-dir rung); OMM_MUSE_BIN is never set
#   3. `omm` on PATH through one symlink dir (what the host inherits; doctor D15)
#   4. `omm install --source <repo> --yes` — marketplace add → plugins install → approve every
#      capability → verify — then asserts plugin:omm:mcp_server:doctor is literally trusted_enabled in
#      `plugins inspect --json` and that `omm doctor --fast` is green (D15 included)
#   5. mock.py + respond-toolcall.py with the plan; `muse exec --provider meta --base-url … --yolo` in the
#      scrubbed env, gate UNSET, dummy META_API_KEY
#   6. analyze_omm_mcp.py: P1 the server's trace shows tools/call omm_doctor {fast:true}; P2 the doctor
#      JSON (host.plugin_id == omm, checks D1..D15) is the function_call_output of the NEXT request, and
#      the cost JSON likewise; P3 session.jsonl carries the canonical ids and the results
#
# Nothing here contacts any non-loopback host: --base-url is total (skill-routing.md V3).
set -uo pipefail
IFS=$' \t\n'
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
M="${OMM_MUSE_BIN:?set OMM_MUSE_BIN to the muse binary}"
OMM="${OMM_BIN:-$ROOT/target/release/omm}"
[ -x "$OMM" ] || { echo "run-omm-mcp: no omm binary at $OMM (build with cargo build -p omm --release, or set OMM_BIN)" >&2; exit 2; }
PORT="${MOCK_PORT:-8741}"
TIMEOUT_S="${TIMEOUT_S:-120}"
W="${1:-$(mktemp -d "${TMPDIR:-/tmp}/omm-mcp.XXXXXX")}"
W="$(mkdir -p "$W" && cd "$W" && pwd)"
PY="$(command -v python3)"
PYDIR="$(dirname "$PY")"

rm -rf "$W"
mkdir -p "$W"/{home/.config,home/.local/share,home/.local/bin,ws,bin,state,logs}

# 2. the launcher-dir rung: ~/.local/bin/.muse-version + muse-bin-<version>
MBASE="$(basename "$M")"
case "$MBASE" in
  muse-bin-*) MVER="${MBASE#muse-bin-}" ;;
  *)          MVER="local" ;;
esac
ln -s "$M" "$W/home/.local/bin/muse-bin-$MVER"
printf '%s\n' "$MVER" > "$W/home/.local/bin/.muse-version"
# 3. omm on PATH
ln -s "$OMM" "$W/bin/omm"

# the scrubbed environment: HOME + PATH (+ the XDG twins of HOME's defaults for the CLI runs); never OMM_MUSE_BIN
E() { env -i PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PYDIR:$W/bin" HOME="$W/home" \
        XDG_CONFIG_HOME="$W/home/.config" XDG_DATA_HOME="$W/home/.local/share" \
        MUSE_NO_AUTO_UPDATE=1 TBH_CREDENTIAL_BACKEND=file TMPDIR="${TMPDIR:-/tmp}" "$@"; }
G() { E MUSE_EXPERIMENTAL_PLUGINS=1 "$M" "$@"; }      # gated plugins CLI

cd "$W/ws" && git init -q . 2>/dev/null

echo "### 1 omm install --source $ROOT (launcher-dir rung, omm on PATH via $W/bin)"
E omm install --source "$ROOT" --yes --json > "$W/logs/install.json" 2> "$W/logs/install.err"; rc=$?
echo "    install rc=$rc  $(head -c 200 "$W/logs/install.err")"
[ "$rc" -eq 0 ] || { echo "run-omm-mcp: install failed"; sed 's/^/    | /' "$W/logs/install.err" | head -20; exit 1; }
"$PY" - "$W/logs/install.json" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
b=d.get("bundle") or {}
print("    mode=%s plugin=%s approved=%s" % (d.get("mode"), b.get("plugin"), b.get("approved")))
PY

echo "### 2 plugins inspect oh-my-musecode --json: plugin:oh-my-musecode:mcp_server:doc must be trusted_enabled"
G plugins inspect oh-my-musecode --json > "$W/logs/inspect.json" 2>&1
"$PY" - "$W/logs/inspect.json" <<'PY'
import json,sys,re
raw=open(sys.argv[1]).read()
m=re.search(r'\{.*\}\s*$',raw,re.S); doc=json.loads(m.group(0)) if m else {}
seen={c.get("candidate",{}).get("stable_id"):c.get("status") for c in doc.get("runtime_capabilities",[])}
mcp=[s for s in doc.get("plugin",{}).get("capabilities",{}).get("mcp_servers",[])]
print("    mcp_servers:", json.dumps(mcp))
for k in sorted(seen): print("    runtime-capability %-45s %s" % (k, seen[k]))
ok = seen.get("plugin:oh-my-musecode:mcp_server:doc") == "trusted_enabled" and any(s.get("command")==["omm","mcp"] for s in mcp)
print("    doc server trusted_enabled and command [omm, mcp]: %s" % ("yes" if ok else "NO"))
sys.exit(0 if ok else 1)
PY
[ $? -eq 0 ] || { echo "run-omm-mcp: the doctor server is not trusted_enabled"; exit 1; }

echo "### 3 omm doctor --fast (D15: omm on PATH)"
E omm doctor --fast --json > "$W/logs/doctor.json" 2> "$W/logs/doctor.err"; rc=$?
"$PY" - "$W/logs/doctor.json" "$rc" <<'PY'
import json,sys
d=json.load(open(sys.argv[1])); rc=int(sys.argv[2])
rows=[c for c in d["checks"] if c["severity"]!="info"]
d15=[c for c in d["checks"] if c["id"]=="D15"][0]
print("    rc=%d checks=%d non-info=%s" % (rc, len(d["checks"]), [(c["id"],c["severity"]) for c in rows]))
print("    D15 %s: %s" % (d15["severity"], d15["observed"][:160]))
sys.exit(0 if rc==0 and not rows else 1)
PY
[ $? -eq 0 ] || { echo "run-omm-mcp: doctor is not green after install"; exit 1; }

echo "### 4 start mock provider on 127.0.0.1:$PORT"
: > "$W/logs/mock.log"; rm -f "$W/state/requests.txt" "$W/state/resolved.jsonl"
if [ -n "${PLAN_FILE:-}" ]; then
  cp "$PLAN_FILE" "$W/state/plan.json"; echo "    plan: $PLAN_FILE"
else
cat > "$W/state/plan.json" <<JSON
[
  {"call": {"name": "mcp__plugin_oh_my_musecode_doc__omm_doctor", "arguments": {"fast": true}}},
  {"call": {"name": "mcp__plugin_oh_my_musecode_doc__omm_cost", "arguments": {}}},
  {"text": "MOCK-FINAL-ANSWER omm_doctor and omm_cost done"}
]
JSON
fi
MOCK_PORT="$PORT" MOCK_STATE="$W/state" MOCK_LOG="$W/logs/mock.log" MOCK_RESPONDER="$HERE/respond-toolcall.py" \
  "$PY" "$HERE/mock.py" 2> "$W/logs/mock.stderr" & MOCKPID=$!
for i in $(seq 1 50); do "$PY" -c "import socket,sys;s=socket.socket();s.settimeout(0.2);sys.exit(0 if s.connect_ex(('127.0.0.1',$PORT))==0 else 1)" && break; sleep 0.1; done
echo "    mock pid=$MOCKPID $(head -1 "$W/logs/mock.stderr")"

# (no apostrophe in the default: bash 3.2, macOS's /bin/bash, mis-parses one inside "${x:-…}")
PROMPT="${PROMPT:-Run the omm doctor and cost tools and report what they say.}"
echo "### 5 muse exec through the mock (gate UNSET, --yolo, dummy META_API_KEY; the host spawns omm mcp via PATH)"
E META_API_KEY=dummy perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
    "$M" exec --provider meta --base-url "http://127.0.0.1:$PORT" --model test-model --yolo \
    --max-model-steps "${MAX_STEPS:-8}" "$PROMPT" \
    > "$W/logs/exec.out" 2>&1; echo $? > "$W/logs/exec.rc"
echo "    exec rc=$(cat "$W/logs/exec.rc")  (142 = killed by the ${TIMEOUT_S}s alarm)"
sed 's/^/    | /' "$W/logs/exec.out" | head -12
kill $MOCKPID 2>/dev/null; wait $MOCKPID 2>/dev/null

echo "### 6 analysis"
"$PY" "$HERE/analyze_omm_mcp.py" "$W"
