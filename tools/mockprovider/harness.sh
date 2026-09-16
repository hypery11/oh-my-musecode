#!/bin/bash
# harness.sh <label> <python-file-producing-selectedSkills-json> [extra muse args]
# skills.v1 routing harness (research/experiments/skill-routing.md, Verification V0): builds a fresh
# sandbox at $V/T_<label>, installs a project UserPromptSubmit router hook whose stdout is the JSON the
# generator script prints, runs one `muse exec --provider echo`, prints the hook terminal + order-201 block.
#   env: OMM_MUSE_BIN (required), MOCK_WORK (default: ./work/skill-routing next to this file)
set -euo pipefail
MUSE="${OMM_MUSE_BIN:?set OMM_MUSE_BIN to the muse binary}"
V="${MOCK_WORK:-$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/work/skill-routing}"
mkdir -p "$V"
label="$1"; gen="$2"; shift 2
R="$V/T_$label"
rm -rf "$R"; mkdir -p "$R"/{home,cfg,data,ws/.muse}
export R
# gen script builds files under $R/ws and prints the selectedSkills JSON array
PAYLOAD=$(python3 "$gen" "$R")
cat > "$R/router.sh" <<EOF
#!/bin/sh
cat > "$R/hook-stdin.json"
cat "$R/payload.json"
EOF
printf '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","selectedSkills":%s}}' "$PAYLOAD" > "$R/payload.json"
chmod +x "$R/router.sh"
cat > "$R/ws/.muse/hooks.json" <<EOF
{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"$R/router.sh","outputCapabilities":["skills.v1"]}]}]}}
EOF
cd "$R/ws"
env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin HOME="$R/home" XDG_CONFIG_HOME="$R/cfg" XDG_DATA_HOME="$R/data" \
  MUSE_NO_AUTO_UPDATE=1 MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1 MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY=1 \
  "$MUSE" exec --provider echo "$@" "hi" >"$R/out.txt" 2>&1 || true
python3 - "$R" "$label" <<'PY'
import json,sys,os,glob,re
R,label=sys.argv[1],sys.argv[2]
fs=sorted(glob.glob(os.path.join(R,"data","muse","sessions","*","*","*","*","session.jsonl")),key=os.path.getmtime)
term=None; blk=None; n200=None
if fs:
    for line in open(fs[-1]):
        try: r=json.loads(line)
        except Exception: continue
        ev=r.get("payload",{}).get("event",{})
        if ev.get("kind")=="hook_run_terminal": term=(ev.get("status"),ev.get("error"))
        if ev.get("kind")=="model_request_configured":
            for m in ev.get("run_context_messages",[]):
                if m.get("id")=="selected_skills_catalog": blk=m["text"]
                if m.get("id")=="skills_catalog": n200=len(m["text"])
nsk=blk.count("<skill ") if blk else 0
ndesc=blk.count("<description>") if blk else 0
print("%-26s term=%-12s err=%-52s block=%-9s skills=%-3d descs=%-3d order200=%s"%(
  label, term[0] if term else "?", (term[1] if term and term[1] else "-"),
  ("%dB"%len(blk)) if blk else "NONE", nsk, ndesc, n200))
open(os.path.join(R,"block.txt"),"w").write(blk or "")
PY
