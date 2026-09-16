#!/bin/bash
# End-to-end skill routing (PLAN.md 3.1) through the mock provider: a prompt that matches a routed
# skill -> `omm hook route` selects it -> the host renders the order-201 block -> the scripted
# "model" calls `read_skill` on the routed id -> the body comes back as the tool result.
#
#   usage: OMM_MUSE_BIN=/path/to/muse-bin bash run-skill-routing.sh [work-dir]
#   env:   OMM_BIN      the omm binary (default: target/debug/omm of this checkout)
#          MOCK_PORT    (default 8733)
#          GATES        on (default) | off  — off is the negative control: no block, `unknown-skill`
#          PROMPT       (default: a prompt that routes omm-commit-message)
#          SKILL        the id the model reads (default omm-commit-message)
#          CALL_FORM    read_skill call spelling for the responder (default ns.fn; the built-ins sit
#                       in the `muse` namespace)
#          TIMEOUT_S    hard kill for the muse run (default 90)
#
# What it does (evidence under <work-dir>):
#   1. a throwaway HOME / XDG_CONFIG_HOME / XDG_DATA_HOME, a git workspace, trust.json written by
#      hand (host-reality "Paths": hand-writing works) so the project hook tier loads
#   2. `omm enable skill-routing` in the workspace (copies the library, writes .muse/hooks.json,
#      measures order 200 with an echo session)
#   3. mock provider + a plan that calls read_skill(<SKILL>) then answers
#   4. one `muse exec --provider meta --base-url http://127.0.0.1:$MOCK_PORT --yolo` with BOTH
#      routing gates (GATES=on) in a scrubbed env, dummy META_API_KEY
#   5. reads session.jsonl: the hook terminal, the order-201 block, the tool result
#
# Nothing here contacts any non-loopback host (--base-url is total, skill-routing.md V3).
set -uo pipefail
IFS=$' \t\n'
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
M="${OMM_MUSE_BIN:?set OMM_MUSE_BIN to the muse binary}"
OMM="${OMM_BIN:-$ROOT/target/debug/omm}"
PORT="${MOCK_PORT:-8733}"
GATES="${GATES:-on}"
SKILL="${SKILL:-omm-commit-message}"
PROMPT="${PROMPT:-Write the commit message for the staged changes, then read the routed skill.}"
FORM="${CALL_FORM:-ns.fn}"
TIMEOUT_S="${TIMEOUT_S:-90}"
W="${1:-$HERE/work/skill-routing}"
W="$(mkdir -p "$W" && cd "$W" && pwd)"
PY="$(command -v python3)"
PYDIR="$(dirname "$PY")"
[ -x "$OMM" ] || { echo "omm binary not found at $OMM (cargo build -p omm)"; exit 2; }

rm -rf "$W"
mkdir -p "$W"/{home,cfg/muse,data,ws,state,logs}
( cd "$W/ws" && git init -q . )
WS="$(cd "$W/ws" && pwd -P)"
cat > "$W/cfg/muse/trust.json" <<JSON
{"schema_version":1,"projects":{"$WS":{"decision":"trusted"}}}
JSON

# scrubbed environment for omm and every muse invocation
E() { env -i PATH="/usr/bin:/bin:/usr/sbin:/sbin:$PYDIR" HOME="$W/home" XDG_CONFIG_HOME="$W/cfg" XDG_DATA_HOME="$W/data" \
        MUSE_NO_AUTO_UPDATE=1 TBH_CREDENTIAL_BACKEND=file OMM_MUSE_BIN="$M" "$@"; }

echo "### 1 omm enable skill-routing (workspace $WS)"
( cd "$WS" && E "$OMM" --json --yes enable skill-routing ) > "$W/logs/enable.json" 2> "$W/logs/enable.stderr"; echo "    enable rc=$?"
"$PY" - "$W/logs/enable.json" <<'PY'
import json,sys
d=json.load(open(sys.argv[1]))
print("    skills=%s" % d.get("skills"))
print("    handler=%s" % d.get("handler_command"))
print("    order200=%s (%s) order201_max=%s headroom=%s" % (d.get("order200_bytes"), d.get("order200_source"), d.get("order201_max_bytes"), d.get("headroom_bytes")))
PY
[ -f "$WS/.muse/hooks.json" ] && echo "    hooks.json: $(tr -d '\n ' < "$WS/.muse/hooks.json" | cut -c1-200)"

echo "### 2 mock provider on 127.0.0.1:$PORT (plan: read_skill($SKILL) as $FORM, then text)"
: > "$W/logs/mock.log"; rm -f "$W/state/requests.txt" "$W/state/resolved.jsonl"
cat > "$W/state/plan.json" <<JSON
[
  {"call": {"name": "read_skill", "form": "$FORM", "arguments": {"name": "$SKILL"}}},
  {"text": "MOCK-FINAL-ANSWER read_skill done"}
]
JSON
# stdout and stderr to files: a mock holding this script's stdout would keep a
# downstream pipe open past the script's end.
MOCK_PORT="$PORT" MOCK_STATE="$W/state" MOCK_LOG="$W/logs/mock.log" MOCK_RESPONDER="$HERE/respond-toolcall.py" \
  "$PY" "$HERE/mock.py" > "$W/logs/mock.stdout" 2> "$W/logs/mock.stderr" < /dev/null & MOCKPID=$!
for i in $(seq 1 50); do "$PY" -c "import socket,sys;s=socket.socket();s.settimeout(0.2);sys.exit(0 if s.connect_ex(('127.0.0.1',$PORT))==0 else 1)" && break; sleep 0.1; done
echo "    mock pid=$MOCKPID $(head -1 "$W/logs/mock.stderr")"

echo "### 3 muse exec through the mock (gates=$GATES, --yolo, dummy META_API_KEY)"
# (bash 3.2 + `set -u`: an empty array expansion is "unbound", hence the two spellings)
GATE_ENV=(MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1 MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY=1)
if [ "$GATES" != on ]; then GATE_ENV=(MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=0 MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY=0); fi
cd "$WS"
E META_API_KEY=dummy "${GATE_ENV[@]}" perl -e 'alarm shift @ARGV; exec @ARGV' "$TIMEOUT_S" \
    "$M" exec --provider meta --base-url "http://127.0.0.1:$PORT" --model test-model --yolo \
    --max-model-steps 4 "$PROMPT" > "$W/logs/exec.out" 2>&1; echo $? > "$W/logs/exec.rc"
echo "    exec rc=$(cat "$W/logs/exec.rc")  (142 = killed by the ${TIMEOUT_S}s alarm)"
sed 's/^/    | /' "$W/logs/exec.out" | head -12
kill $MOCKPID 2>/dev/null; wait $MOCKPID 2>/dev/null

echo "### 4 analysis"
"$PY" - "$W" "$SKILL" "$GATES" <<'PY'
import glob, json, os, sys
W, skill, gates = sys.argv[1], sys.argv[2], sys.argv[3]
fs = sorted(glob.glob(os.path.join(W, "data", "muse", "sessions", "*", "*", "*", "*", "session.jsonl")), key=os.path.getmtime)
if not fs:
    print("FAIL no session.jsonl"); sys.exit(1)
terms, block, calls, results = [], None, [], []
def walk(o):
    if isinstance(o, dict):
        k = o.get("kind")
        if k == "hook_run_terminal":
            terms.append((o.get("status"), o.get("error"), o.get("hook_key") or o.get("key")))
        if k == "model_request_configured":
            for m in o.get("run_context_messages", []) or []:
                if m.get("id") == "selected_skills_catalog":
                    global block; block = m.get("text")
        if k == "assistant_tool_calls_committed":
            for c in o.get("tool_calls", []) or []:
                calls.append((c.get("name"), c.get("arguments")))
        if k == "tool_result_batch_committed":
            for r in o.get("results", []) or []:
                results.append(r.get("text") or json.dumps(r)[:400])
        for v in o.values(): walk(v)
    elif isinstance(o, list):
        for v in o: walk(v)
for line in open(fs[-1]):
    try: r = json.loads(line)
    except Exception: continue
    walk(r)
    # retained-frame wrapper: stringified child records
    ch = r.get("children") if isinstance(r, dict) else None
    if ch:
        for c in ch:
            try: walk(json.loads(c.get("record_json", "null")))
            except Exception: pass
print("    hook terminals: %s" % terms)
print("    order-201 block: %s" % ("%d B" % len(block.encode()) if block else "NONE"))
print("    tool calls: %s" % calls)
for t in results:
    print("    tool result: %s" % t.replace("\n", "\\n")[:300])
ok = True
def check(cond, label):
    global ok
    print(("    PASS " if cond else "    FAIL ") + label); ok = ok and cond
routed = bool(block) and ('id="%s"' % skill) in block
read_ok = any(('<read-skill-result name="%s" status="ok">' % skill) in t for t in results)
unknown = any("unknown-skill" in t for t in results)
called = any(c and c[0] and c[0].endswith("read_skill") for c in calls)
if gates == "on":
    check(any(t[0] == "completed" for t in terms), "the router hook completed")
    check(routed, "order-201 block names %s" % skill)
    check(called, "the model called read_skill")
    check(read_ok, "read_skill(%s) returned the routed body (status ok)" % skill)
else:
    check(block is None, "no order-201 block with the gates off")
    check(called, "the model called read_skill")
    check(unknown and not read_ok, "read_skill(%s) is unknown-skill with the gates off" % skill)
sys.exit(0 if ok else 1)
PY
