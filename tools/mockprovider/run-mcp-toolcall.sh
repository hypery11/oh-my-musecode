#!/bin/bash
# End-to-end MCP tools/call through a plugin-provided stdio MCP server, driven by the mock provider.
#
#   usage: OMM_MUSE_BIN=/path/to/muse-bin bash run-mcp-toolcall.sh [work-dir]
#   env:   LANE        exec (default) | serve  — `muse exec` or `muse serve` (MSP over stdio)
#          MOCK_PORT   (default 8731)
#          PLUGIN_ID   (default omm)
#          TIMEOUT_S   (default 90) — hard kill for the muse run
#          PLAN_FILE   a plan.json for respond-toolcall.py (default: one call per server, then text)
#          CALL_FORM   default name form for the responder (ns__fn | ns.fn | fn | fn+namespace | ns/fn)
#          MAX_STEPS   --max-model-steps for exec (default 8)
#          PROMPT      the user prompt (must be non-trivial: a bare "hi" is answered locally, 0 POSTs)
#          SKIP_APPROVE=1  leave every capability at review_needed (negative control: no MCP on the wire)
#
# What it does (each step leaves its evidence under <work-dir>):
#   1. builds a throwaway HOME / XDG_CONFIG_HOME / XDG_DATA_HOME / workspace
#   2. builds a native plugin package with THREE stdio MCP servers, all running
#      mcp_echo_server.py (tool `echo_upper`), with server ids of length 2, 15 and 16 so that
#      len(pid)+len(sid) is 5, 18 and 19 — bracketing the 18-char verbatim-name rule
#   3. validates, installs (MUSE_EXPERIMENTAL_PLUGINS=1) and approves every mcp_server capability,
#      then asserts each is PRESENT and literally trusted_enabled in `plugins inspect --json`
#   4. starts mock.py with respond-toolcall.py and a plan that calls each server's tool once
#   5. runs one `muse exec --provider meta --base-url http://127.0.0.1:$MOCK_PORT --yolo` with a
#      dummy META_API_KEY, gate UNSET (runtime composition is ungated), in a scrubbed env
#   6. runs analyze_mcp_toolcall.py and exits with its status
#
# Nothing here contacts any non-loopback host: --base-url is total (skill-routing.md V3).
set -uo pipefail
IFS=$' \t\n'
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
M="${OMM_MUSE_BIN:?set OMM_MUSE_BIN to the muse binary}"
LANE="${LANE:-exec}"
PORT="${MOCK_PORT:-8731}"
PID="${PLUGIN_ID:-omm}"
TIMEOUT_S="${TIMEOUT_S:-90}"
W="${1:-$HERE/work/mcp-toolcall}"
W="$(mkdir -p "$W" && cd "$W" && pwd)"
PY="$(command -v python3)"
PYDIR="$(dirname "$PY")"

rm -rf "$W"
mkdir -p "$W"/{home,cfg,data,ws,state,logs} "$W/pkg/.muse-plugin" "$W/pkg/mcp"
cp "$HERE/mcp_echo_server.py" "$W/pkg/mcp/server.py"

# server ids: len 2 / 15 / 16  ->  pid+sid = 5 / 18 / 19 with pid=omm
SID_SHORT="up"
SID_15="fifteencharsxxx"
SID_16="sixteencharsxxxx"
cat > "$W/pkg/.muse-plugin/plugin.json" <<JSON
{ "schemaVersion": 1, "name": "$PID", "displayName": "omm mock MCP probe", "version": "0.1.0",
  "description": "Three stdio MCP servers exposing echo_upper, to prove model->tools/call->result end to end.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "skills": [], "commands": [], "hooks": [], "reminders": [],
    "mcpServers": [
      { "id": "$SID_SHORT", "transport": "stdio", "command": ["python3", "mcp/server.py", "$SID_SHORT"] },
      { "id": "$SID_15",    "transport": "stdio", "command": ["python3", "mcp/server.py", "$SID_15"] },
      { "id": "$SID_16",    "transport": "stdio", "command": ["python3", "mcp/server.py", "$SID_16"] }
    ] } }
JSON

# scrubbed environment for every muse invocation
E() { env -i PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PYDIR" HOME="$W/home" XDG_CONFIG_HOME="$W/cfg" XDG_DATA_HOME="$W/data" \
        MUSE_NO_AUTO_UPDATE=1 TBH_CREDENTIAL_BACKEND=file "$@"; }
G() { E MUSE_EXPERIMENTAL_PLUGINS=1 "$M" "$@"; }      # gated plugins CLI

if [ "$LANE" = serve ]; then
  # `muse serve` has no --provider/--base-url: both come from settings.json (typed keys, so they
  # survive the rewrite `plugins approve` performs). Written BEFORE install so approve merges into it.
  mkdir -p "$W/cfg/muse"
  cat > "$W/cfg/muse/settings.json" <<JSON
{ "schema_version": 1, "provider": "meta", "model": "test-model",
  "endpoint_transport": { "base_url": "http://127.0.0.1:$PORT" } }
JSON
  echo "### 0 lane=serve: settings.json provider=meta endpoint_transport.base_url=http://127.0.0.1:$PORT"
fi

echo "### 1 validate / install / approve (plugin id=$PID)"
G plugins validate "$W/pkg" --json > "$W/logs/validate.json" 2>&1; echo "    validate rc=$?"
G plugins install "$W/pkg" --scope user --json > "$W/logs/install.json" 2>&1; echo "    install  rc=$?"
grep -o 'warning.*' "$W/logs/install.json" | head -1 | sed 's/^/    /'
if [ -n "${SKIP_APPROVE:-}" ]; then
  echo "    SKIP_APPROVE set: capabilities left at review_needed (negative control)"
else
  for sid in "$SID_SHORT" "$SID_15" "$SID_16"; do
    G plugins approve "plugin:$PID:mcp_server:$sid" --json > "$W/logs/approve-$sid.json" 2>&1; echo "    approve plugin:$PID:mcp_server:$sid rc=$?"
  done
fi
G plugins inspect "$PID" --json > "$W/logs/inspect.json" 2>&1
"$PY" - "$W/logs/inspect.json" "$PID" "$SID_SHORT" "$SID_15" "$SID_16" <<'PY'
import json,sys,re
raw=open(sys.argv[1]).read()
m=re.search(r'\{.*\}\s*$',raw,re.S); doc=json.loads(m.group(0)) if m else {}
pid=sys.argv[2]; want={"plugin:%s:mcp_server:%s"%(pid,s) for s in sys.argv[3:]}
# plugins inspect --json: runtime_capabilities[] = {"candidate":{"stable_id":...,...},"status":"trusted_enabled",...}
seen={c.get("candidate",{}).get("stable_id"):c.get("status") for c in doc.get("runtime_capabilities",[])}
ok=True
for w in sorted(want):
    st=seen.get(w); print("    runtime-capability %-45s %s"%(w,st)); ok&=(st=="trusted_enabled")
sys.exit(0 if ok else 1)
PY
echo "    all trusted_enabled: $([ $? -eq 0 ] && echo yes || echo NO)"

echo "### 2 start mock provider on 127.0.0.1:$PORT"
: > "$W/logs/mock.log"; rm -f "$W/state/requests.txt" "$W/state/resolved.jsonl"
if [ -n "${PLAN_FILE:-}" ]; then
  cp "$PLAN_FILE" "$W/state/plan.json"; echo "    plan: $PLAN_FILE"
else
cat > "$W/state/plan.json" <<JSON
[
  {"call": {"name": "mcp__plugin_${PID}_${SID_SHORT}__echo_upper", "arguments": {"text": "omm mcp probe short 7f3a"}}},
  {"call": {"name": "mcp__plugin_${PID}_${SID_15}__echo_upper",    "arguments": {"text": "omm mcp probe fifteen 18ch"}}},
  {"call": {"name": "/^mcp__plugin_${PID}_s__[0-9a-f]{12}__echo_upper\$/", "arguments": {"text": "omm mcp probe sixteen rewritten"}}},
  {"text": "MOCK-FINAL-ANSWER all three echo_upper calls done"}
]
JSON
fi
[ -n "${CALL_FORM:-}" ] && echo "$CALL_FORM" > "$W/state/form.txt"
MOCK_PORT="$PORT" MOCK_STATE="$W/state" MOCK_LOG="$W/logs/mock.log" MOCK_RESPONDER="$HERE/respond-toolcall.py" \
  "$PY" "$HERE/mock.py" 2> "$W/logs/mock.stderr" & MOCKPID=$!
for i in $(seq 1 50); do "$PY" -c "import socket,sys;s=socket.socket();s.settimeout(0.2);sys.exit(0 if s.connect_ex(('127.0.0.1',$PORT))==0 else 1)" && break; sleep 0.1; done
echo "    mock pid=$MOCKPID $(head -1 "$W/logs/mock.stderr")"

PROMPT="${PROMPT:-Use the echo_upper tools to upper-case the phrase: omm mcp probe.}"
cd "$W/ws"
if [ "$LANE" = serve ]; then
  echo "### 3 muse serve (MSP) through the mock (gate UNSET, approvalMode=allowAll, dummy META_API_KEY)"
  E META_API_KEY=dummy "$PY" "$HERE/msp_toolcall.py" "$M" "$W/ws" "$PROMPT" "$TIMEOUT_S" \
      > "$W/logs/serve.out" 2>&1; echo $? > "$W/logs/exec.rc"
  echo "    serve driver rc=$(cat "$W/logs/exec.rc")  $(grep -E '^(SESSION|RESULT) ' "$W/logs/serve.out" | tr '\n' ' ')"
  grep -E '^(>>>|!!!)' "$W/logs/serve.out" | cut -c1-160 | sed 's/^/    /'
else
  echo "### 3 muse exec through the mock (gate UNSET, --yolo, dummy META_API_KEY)"
  # perl alarm = hard timeout without a bash watchdog (bash defers signals while `sleep` is foreground)
  E META_API_KEY=dummy perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
      "$M" exec --provider meta --base-url "http://127.0.0.1:$PORT" --model test-model --yolo \
      --max-model-steps "${MAX_STEPS:-8}" "$PROMPT" \
      > "$W/logs/exec.out" 2>&1; echo $? > "$W/logs/exec.rc"
  echo "    exec rc=$(cat "$W/logs/exec.rc")  (142 = killed by the ${TIMEOUT_S}s alarm)"
  sed 's/^/    | /' "$W/logs/exec.out" | head -20
fi
kill $MOCKPID 2>/dev/null; wait $MOCKPID 2>/dev/null

echo "### 4 analysis"
"$PY" "$HERE/analyze_mcp_toolcall.py" "$W"
