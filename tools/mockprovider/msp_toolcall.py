#!/usr/bin/env python3
"""Drive one turn through `muse serve` (MSP over stdio) and print every frame.

    usage: msp_toolcall.py <muse-bin> <workspace> <prompt> [timeout-s] [serve args...]

The caller owns the environment (HOME / XDG_* / META_API_KEY / MUSE_NO_AUTO_UPDATE) and the
settings.json. `muse serve` has no --provider or --base-url flag: the provider comes from
settings.json -> provider, and the base URL from settings.json -> endpoint_transport.base_url
(research/musecode/model-providers.md section 5.1, plugin-mcp.md V.4).

Wire (research/musecode/msp-protocol.md, skill-routing.md V5): NDJSON JSON-RPC,
initialize -> initialized (notification) -> session/start -> turn/start; every commandId must be a
UUIDv7. Approval is chosen on the wire: approvalMode "allowAll".

Stops when a turn/* notification reports a terminal state, or at the timeout. Prints ">>>" for
frames sent, "<<<" for stdout frames, "!!!" for stderr lines, then "RESULT <state>".
"""
import json
import os
import queue
import subprocess
import sys
import threading
import time

BIN, WS, PROMPT = sys.argv[1], sys.argv[2], sys.argv[3]
TIMEOUT = float(sys.argv[4]) if len(sys.argv) > 4 else 60.0
EXTRA = sys.argv[5:]

p = subprocess.Popen([BIN, "serve"] + EXTRA, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.PIPE, cwd=WS, text=True, bufsize=1)
q = queue.Queue()


def rd(st, tag):
    for l in st:
        q.put((tag, l.rstrip("\n")))
    q.put((tag, None))


threading.Thread(target=rd, args=(p.stdout, "out"), daemon=True).start()
threading.Thread(target=rd, args=(p.stderr, "err"), daemon=True).start()
n = [0]


def send(m, params=None, notify=False):
    f = {"jsonrpc": "2.0", "method": m}
    if not notify:
        n[0] += 1
        f["id"] = n[0]
    if params is not None:
        f["params"] = params
    s = json.dumps(f)
    print(">>> " + s[:300], flush=True)
    p.stdin.write(s + "\n")
    p.stdin.flush()
    return f.get("id")


def u7():
    ms = int(time.time() * 1000)
    b = bytearray(os.urandom(16))
    b[0:6] = ms.to_bytes(6, "big")
    b[6] = (b[6] & 0x0F) | 0x70
    b[8] = (b[8] & 0x3F) | 0x80
    h = b.hex()
    return "%s-%s-%s-%s-%s" % (h[0:8], h[8:12], h[12:16], h[16:20], h[20:32])


def drain(t, stop=None):
    e = time.time() + t
    got = []
    while time.time() < e:
        try:
            k, l = q.get(timeout=0.1)
        except queue.Empty:
            continue
        if l is None:
            print("<<< EOF " + k, flush=True)
            continue
        print(("<<< " if k == "out" else "!!! ") + l[:int(os.environ.get("MSP_PRINT_WIDTH", "6000"))], flush=True)
        got.append((k, l))
        if stop and k == "out":
            try:
                o = json.loads(l)
            except Exception:
                continue
            r = stop(o)
            if r:
                return got, r
    return got, None


# clientInfo.name must match ^[a-z0-9_]+$ (SS1.4.1) — a hyphen gets -32602 invalidParams and every
# later frame is refused with -32600 "Not initialized".
send("initialize", {"clientInfo": {"name": "omm_mock_harness", "version": "0"}, "capabilities": {}})
drain(2)
send("initialized", None, True)
i = send("session/start", {"commandId": u7(), "workspaceRoot": WS, "providerId": "meta", "approvalMode": "allowAll"})
got, _ = drain(8, lambda o: "result" in o and o.get("id") == i)
sid = None
for k, l in got:
    try:
        o = json.loads(l)
    except Exception:
        continue
    if o.get("id") == i and "result" in o:
        sid = (o.get("result") or {}).get("session", {}).get("sessionId")
print("SESSION " + str(sid), flush=True)
if not sid:
    p.terminate()
    print("RESULT no-session")
    sys.exit(2)
time.sleep(2)  # let mcp.startup finish before the turn
send("turn/start", {"commandId": u7(), "sessionId": sid,
                    "input": [{"type": "text", "text": PROMPT}], "displayText": PROMPT})


def terminal(o):
    m = o.get("method") or ""
    s = json.dumps(o)
    if m.startswith("turn/") and ('"terminal"' in s or "turn/completed" in m or "turn/failed" in m):
        return m
    return None


got, r = drain(TIMEOUT, terminal)
print("RESULT " + (r or "timeout"), flush=True)
p.terminate()
drain(1)
sys.exit(0 if r and "completed" in r else 1)
