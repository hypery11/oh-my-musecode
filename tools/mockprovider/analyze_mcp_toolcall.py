#!/usr/bin/env python3
"""Extract the evidence for an MCP tools/call round-trip from a run-mcp-toolcall.sh work dir.

    usage: analyze_mcp_toolcall.py <work-dir> [--json]

Reads three independent instruments and cross-checks them:
  1. the MCP servers' own logs   <work>/data/muse/plugins/data/<pid>/mcp-*.log
  2. the mock provider transcript <work>/logs/mock.log   (+ <work>/state/resolved.jsonl)
  3. Muse's own session.jsonl     <work>/data/muse/sessions/YYYY/MM/DD/<uuid>/session.jsonl

Prints PASS/FAIL for the three proofs the experiment needs:
  P1  every scripted call reached a server as JSON-RPC tools/call with the scripted arguments
  P2  every tool result appeared as function_call_output in the NEXT provider request
  P3  session.jsonl carries the tool call and tool result records
plus the identity catalog (verbatim vs rewritten surface names) and the wire tool names.
"""
import glob
import json
import os
import re
import sys

W = os.path.abspath(sys.argv[1])
AS_JSON = "--json" in sys.argv[2:]
report = {"work": W}


def newest_session():
    fs = sorted(glob.glob(os.path.join(W, "data", "muse", "sessions", "*", "*", "*", "*", "session.jsonl")),
                key=os.path.getmtime)
    return fs[-1] if fs else None


# ---------- 1. MCP server logs ----------
servers = {}
for lf in sorted(glob.glob(os.path.join(W, "data", "muse", "plugins", "data", "*", "mcp-*.log"))):
    tag = os.path.basename(lf)[4:-4]
    s = {"log": lf, "spawns": 0, "recv_methods": [], "calls": [], "results": [], "spawn_env_keys": None, "cwd": None}
    for line in open(lf):
        parts = line.rstrip("\n").split("\t", 4)
        if len(parts) < 5:
            continue
        ev = parts[2][3:]
        try:
            o = json.loads(parts[4])
        except Exception:
            continue
        if ev == "spawn":
            s["spawns"] += 1
            s["spawn_env_keys"] = o.get("env_keys")
            s["cwd"] = o.get("cwd")
        elif ev == "recv":
            s["recv_methods"].append(o.get("method"))
            if o.get("method") == "tools/call":
                s["calls"].append({"id": o.get("id"), "params": o.get("params")})
        elif ev == "sent":
            r = o.get("result") or {}
            if isinstance(r, dict) and "content" in r:
                s["results"].append({"id": o.get("id"), "content": r.get("content"), "isError": r.get("isError")})
    servers[tag] = s
report["servers"] = servers

# ---------- 2. mock transcript ----------
posts = []
for line in open(os.path.join(W, "logs", "mock.log")):
    try:
        r = json.loads(line)
    except Exception:
        continue
    if r.get("m") != "POST":
        continue
    try:
        b = json.loads(r["body"])
    except Exception:
        posts.append({"parse_error": True})
        continue
    tools = [t.get("name") for t in (b.get("tools") or []) if isinstance(t, dict)]
    fcs = [it for it in (b.get("input") or []) if isinstance(it, dict) and it.get("type") == "function_call"]
    fcos = [it for it in (b.get("input") or []) if isinstance(it, dict) and it.get("type") == "function_call_output"]
    posts.append({"n_tools": len(tools), "mcp_tools": [t for t in tools if t and t.startswith("mcp__")],
                  "function_calls": [{"call_id": f.get("call_id"), "name": f.get("name"), "arguments": f.get("arguments")} for f in fcs],
                  "function_call_outputs": [{"call_id": f.get("call_id"),
                                             "output": f.get("output") if isinstance(f.get("output"), str) else json.dumps(f.get("output"))}
                                            for f in fcos]})
report["posts"] = posts
resolved = []
rp = os.path.join(W, "state", "resolved.jsonl")
if os.path.exists(rp):
    resolved = [json.loads(l) for l in open(rp) if l.strip()]
report["resolved"] = resolved
plan = []
pp = os.path.join(W, "state", "plan.json")
if os.path.exists(pp):
    plan = json.load(open(pp))

# ---------- 3. session.jsonl ----------
sf = newest_session()
report["session_file"] = sf
catalog = None
active = None
tool_records = []
if sf:
    for line in open(sf):
        try:
            o = json.loads(line)
        except Exception:
            continue
        pt = o.get("payload_type")
        p = o.get("payload") or {}
        if pt == "runtime.mcp_tool_identity_catalog":
            catalog = p
        ev = p.get("event") if isinstance(p, dict) else None
        if isinstance(ev, dict) and ev.get("kind") == "model_request_configured":
            active = ev.get("toolset", {}).get("active_tools")
        s = json.dumps(o)
        if "echo_upper" in s or "mcp__plugin_" in s:
            kind = (ev or {}).get("kind") if isinstance(ev, dict) else p.get("kind") if isinstance(p, dict) else None
            tool_records.append({"seq": o.get("sequence"), "payload_type": pt, "kind": kind, "bytes": len(s)})
report["identity_catalog"] = catalog
report["active_mcp_tools"] = [t for t in (active or []) if t.startswith("mcp__")]
report["tool_record_kinds"] = tool_records

# pull the concrete call / result records
call_records = []
result_records = []
if sf:
    for line in open(sf):
        try:
            o = json.loads(line)
        except Exception:
            continue
        p = o.get("payload") or {}
        ev = p.get("event") if isinstance(p, dict) else None
        k = (ev or {}).get("kind") if isinstance(ev, dict) else p.get("kind") if isinstance(p, dict) else None
        s = json.dumps(o)
        if k == "assistant_tool_calls_committed":
            call_records.append({"seq": o.get("sequence"), "kind": k, "names": re.findall(r'"name":\s*"(mcp__[^"]+)"', s),
                                 "arguments": re.findall(r'"arguments":\s*"((?:[^"\\]|\\.)*)"', s)[:4]})
        if k == "tool_result_batch_committed" or o.get("payload_type") in ("tool_batch.effect.started", "tool_batch.effect.terminal"):
            m = re.findall(r'ECHO_UPPER|echo_upper via server=([a-z0-9]+)', s)
            result_records.append({"seq": o.get("sequence"), "payload_type": o.get("payload_type"), "kind": k,
                                   "mentions_result_marker": bool(m), "servers": [x for x in m if x]})
report["call_records"] = call_records
report["result_records"] = result_records

# ---------- cross-check ----------
# A plan step may carry "expect": "ok" (default) or "reject" (the call must be refused by Muse,
# never reach a server, and come back to the model as a `tool unavailable` function_call_output).
checks = []
scripted = [(i, st["call"]) for i, st in enumerate(plan) if "call" in st]
# P1
p1_ok = True
p1 = []
for i, c in scripted:
    want_args = c.get("arguments", {})
    expect = c.get("expect", "ok")
    hit = None
    for tag, s in servers.items():
        for call in s["calls"]:
            if (call["params"] or {}).get("arguments") == want_args and (call["params"] or {}).get("name") == "echo_upper":
                hit = (tag, call)
    ok = (hit is not None) if expect == "ok" else (hit is None)
    p1.append({"step": i, "scripted": c, "expect": expect, "ok": ok,
               "server": hit[0] if hit else None, "jsonrpc_id": hit[1]["id"] if hit else None})
    p1_ok &= ok
p1_ok &= len(p1) >= 1
checks.append(("P1 server received tools/call with the scripted argument (or none, where expect=reject)", p1_ok, p1))
# P2
p2_ok = True
p2 = []
for r in resolved:
    if "call_id" not in r:
        continue
    expect = (r.get("step", {}).get("call") or {}).get("expect", "ok")
    k = r["request"]  # 1-based; the NEXT request is index k (0-based k)
    nxt = posts[k] if k < len(posts) else None
    got = None
    if nxt:
        for fco in nxt["function_call_outputs"]:
            if fco["call_id"] == r["call_id"]:
                got = fco["output"]
    if expect == "ok":
        ok = got is not None and "echo_upper via server=" in got
    else:
        ok = got is not None and got.startswith("tool unavailable:")
    p2.append({"emitted_in_request": k, "call_id": r["call_id"], "wire_name": r.get("wire_name"), "expect": expect, "ok": ok,
               "found_in_request": k + 1 if got is not None else None, "output": got})
    p2_ok &= ok
p2_ok &= len(p2) >= 1
checks.append(("P2 tool result appeared as function_call_output in the NEXT provider request (or `tool unavailable`, where expect=reject)", p2_ok, p2))
# P3 — for a plan whose every call is expect=reject (negative control) the correct observation is
# that NO tool result carrying the server marker exists.
any_ok_step = any(c.get("expect", "ok") == "ok" for _, c in scripted)
if any_ok_step:
    p3_ok = len(call_records) >= 1 and any(r["mentions_result_marker"] for r in result_records)
else:
    p3_ok = not any(r["mentions_result_marker"] for r in result_records)
checks.append(("P3 session.jsonl carries tool call and tool result records (or no server result, where every step is expect=reject)", p3_ok,
               {"call_records": call_records, "result_records": result_records}))
report["checks"] = [{"name": n, "ok": ok, "detail": d} for n, ok, d in checks]

if AS_JSON:
    print(json.dumps(report, indent=1))
    sys.exit(0 if all(ok for _, ok, _ in checks) else 1)

print("work dir : %s" % W)
print("session  : %s" % sf)
print()
print("== identity catalog (session.jsonl runtime.mcp_tool_identity_catalog) ==")
for e in (catalog or {}).get("entries", []):
    ns = e.get("surface", {}).get("namespace", "")
    print("  canonical_id=%-48s surface=%s__%s  (%d chars ns, %s)  server=%s" % (
        e.get("canonical_id"), ns, e.get("surface", {}).get("name"), len(ns),
        "REWRITTEN" if "__" in ns[12:] else "verbatim", e.get("server_name")))
print("== active_tools (mcp__*) ==")
for t in report["active_mcp_tools"]:
    print("  " + t)
print()
print("== provider requests ==")
for i, p in enumerate(posts):
    if p.get("parse_error"):
        print("  #%d  (unparseable body)" % (i + 1))
        continue
    print("  #%d  tools=%d  mcp_tools=%s" % (i + 1, p["n_tools"], p["mcp_tools"]))
    for f in p["function_calls"]:
        print("       input function_call        %s %s %s" % (f["call_id"], f["name"], f["arguments"]))
    for f in p["function_call_outputs"]:
        print("       input function_call_output %s -> %s" % (f["call_id"], (f["output"] or "")[:160]))
print("== responder decisions ==")
for r in resolved:
    if "error" in r:
        print("  request #%d step=%d -> ERROR %s" % (r["request"], r["step_index"], r["error"]))
    elif "call" in r["step"]:
        print("  request #%d step=%d -> CALL name=%s form=%s extra=%s args=%s" % (
            r["request"], r["step_index"], r.get("wire_name"), r.get("form"), json.dumps(r.get("extra")),
            json.dumps(r["step"]["call"].get("arguments"))))
    else:
        print("  request #%d step=%d -> TEXT %r" % (r["request"], r["step_index"], r["step"].get("text")))
print()
print("== MCP server logs ==")
for tag, s in servers.items():
    print("  server=%s spawns=%d cwd=%s recv=%s" % (tag, s["spawns"], s["cwd"], s["recv_methods"]))
    for c in s["calls"]:
        print("       tools/call id=%s params=%s" % (c["id"], json.dumps(c["params"])))
    for r in s["results"]:
        print("       -> result id=%s isError=%s content=%s" % (r["id"], r["isError"], json.dumps(r["content"])[:200]))
print()
print("== session.jsonl records mentioning the tool ==")
for r in tool_records:
    print("  seq=%-4s %-40s kind=%s (%d B)" % (r["seq"], r["payload_type"], r["kind"], r["bytes"]))
for r in call_records:
    print("  CALL  seq=%s %s names=%s args=%s" % (r["seq"], r["kind"], r["names"], r["arguments"]))
for r in result_records:
    print("  RESULT seq=%s %s %s marker=%s servers=%s" % (r["seq"], r["payload_type"], r["kind"], r["mentions_result_marker"], r["servers"]))
print()
print("== verdicts ==")
for n, ok, d in checks:
    print("  [%s] %s" % ("PASS" if ok else "FAIL", n))
    if n.startswith("P1"):
        for x in d:
            print("         %s step %d expect=%-6s args=%s -> server=%s jsonrpc_id=%s" % (
                "ok  " if x["ok"] else "BAD ", x["step"], x["expect"], json.dumps(x["scripted"].get("arguments")), x["server"], x["jsonrpc_id"]))
    if n.startswith("P2"):
        for x in d:
            print("         %s %s emitted in request #%d as %s (expect=%s) -> output in request #%s: %s" % (
                "ok  " if x["ok"] else "BAD ", x["call_id"], x["emitted_in_request"], x["wire_name"], x["expect"],
                x["found_in_request"], (x["output"] or "")[:120]))
sys.exit(0 if all(ok for _, ok, _ in checks) else 1)
