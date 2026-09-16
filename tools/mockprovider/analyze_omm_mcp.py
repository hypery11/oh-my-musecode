#!/usr/bin/env python3
"""Extract the evidence for the `omm mcp` round-trip from a run-omm-mcp.sh work dir.

    usage: analyze_omm_mcp.py <work-dir> [--json]

Three independent instruments, cross-checked:
  1. the server's own trace      <work>/home/.local/share/muse/plugins/data/oh-my-musecode/omm-mcp.log
                                 (one line per frame: <epoch-ms>\\tEV=<ev>\\tPID=<pid>\\t<json>)
  2. the mock provider transcript <work>/logs/mock.log  (+ <work>/state/resolved.jsonl)
  3. Muse's own session.jsonl     <work>/home/.local/share/muse/sessions/YYYY/MM/DD/<uuid>/session.jsonl

Proofs:
  P1  the server was spawned by the host (spawn line, cwd = the workspace) and received tools/call
      for every scripted step with the scripted arguments
  P2  every tool result appeared as function_call_output in the NEXT provider request and parses as
      the document it claims: omm_doctor → {"host":{"plugin_id":"oh-my-musecode"},"checks":[D1..D15]},
      omm_cost → {"catalog":…,"tokens":…}
  P3  session.jsonl carries the canonical ids (mcp__plugin_oh_my_musecode_doc__omm_doctor / __omm_cost) on
      the tool call and the tool result records
"""
import glob
import json
import os
import sys

W = os.path.abspath(sys.argv[1])
AS_JSON = "--json" in sys.argv[2:]
DATA = os.path.join(W, "home", ".local", "share", "muse")
report = {"work": W}


def newest_session():
    fs = sorted(glob.glob(os.path.join(DATA, "sessions", "*", "*", "*", "*", "session.jsonl")), key=os.path.getmtime)
    return fs[-1] if fs else None


# ---------- 1. the server trace ----------
trace_path = os.path.join(DATA, "plugins", "data", "oh-my-musecode", "omm-mcp.log")
trace = {"path": trace_path, "spawns": [], "recv": [], "calls": [], "tool": [], "sent": 0}
if os.path.exists(trace_path):
    for line in open(trace_path):
        parts = line.rstrip("\n").split("\t", 3)
        if len(parts) < 4:
            continue
        ev = parts[1][3:]
        try:
            o = json.loads(parts[3])
        except Exception:
            continue
        if ev == "spawn":
            trace["spawns"].append(o)
        elif ev == "recv":
            trace["recv"].append(o.get("method"))
            if o.get("method") == "tools/call":
                trace["calls"].append({"id": o.get("id"), "name": o.get("name"), "arguments": o.get("arguments")})
        elif ev == "tool":
            trace["tool"].append(o)
        elif ev == "sent":
            trace["sent"] += 1
report["trace"] = trace

# ---------- 2. the mock transcript ----------
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
    mcp_ns = [t for t in (b.get("tools") or []) if isinstance(t, dict) and str(t.get("name", "")).startswith("mcp__")]
    fcs = [it for it in (b.get("input") or []) if isinstance(it, dict) and it.get("type") == "function_call"]
    fcos = [it for it in (b.get("input") or []) if isinstance(it, dict) and it.get("type") == "function_call_output"]
    posts.append({
        "n_tools": len(tools),
        "mcp_namespaces": [{"name": t.get("name"), "tools": [f.get("name") for f in (t.get("tools") or [])]} for t in mcp_ns],
        "function_calls": [{"call_id": f.get("call_id"), "name": f.get("name"), "arguments": f.get("arguments")} for f in fcs],
        "function_call_outputs": [{"call_id": f.get("call_id"),
                                   "output": f.get("output") if isinstance(f.get("output"), str) else json.dumps(f.get("output"))}
                                  for f in fcos],
    })
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
call_records = []
result_records = []
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
        k = (ev or {}).get("kind") if isinstance(ev, dict) else p.get("kind") if isinstance(p, dict) else None
        s = json.dumps(o)
        if k == "assistant_tool_calls_committed":
            names = []
            for tc in (ev.get("tool_calls") or []):
                if isinstance(tc, dict):
                    names.append(tc.get("name"))
            call_records.append({"seq": o.get("sequence"), "names": names})
        if k == "tool_result_batch_committed":
            texts = []
            for r in (ev.get("results") or []):
                if isinstance(r, dict):
                    texts.append(r.get("text") or "")
            result_records.append({"seq": o.get("sequence"), "texts": texts})
report["identity_catalog"] = catalog
report["call_records"] = call_records
report["result_records"] = [{"seq": r["seq"], "lengths": [len(t) for t in r["texts"]]} for r in result_records]

# ---------- cross-check ----------
checks = []
scripted = [(i, st["call"]) for i, st in enumerate(plan) if "call" in st]


def tool_of(spec):
    return spec.rsplit("__", 1)[-1]


# P1
p1 = []
p1_ok = len(trace["spawns"]) >= 1
ws = os.path.join(W, "ws")
spawn_cwd_ok = any(os.path.realpath(s.get("cwd") or "") == os.path.realpath(ws) for s in trace["spawns"])
p1_ok &= spawn_cwd_ok
for i, c in scripted:
    want_name = tool_of(c["name"])
    want_args = c.get("arguments", {})
    hit = next((call for call in trace["calls"] if call["name"] == want_name and (call["arguments"] or {}) == want_args), None)
    p1.append({"step": i, "tool": want_name, "arguments": want_args, "ok": hit is not None, "jsonrpc_id": hit["id"] if hit else None})
    p1_ok &= hit is not None
p1_ok &= len(p1) >= 1
checks.append(("P1 the host spawned `omm mcp` in the workspace and the server received tools/call with the scripted arguments",
               p1_ok, {"spawns": len(trace["spawns"]), "spawn_cwd_is_workspace": spawn_cwd_ok, "steps": p1}))


# P2
def looks_like(tool, text):
    try:
        d = json.loads(text)
    except Exception:
        return False, "not JSON"
    if not isinstance(d, dict):
        return False, "not an object"
    if tool == "omm_doctor":
        ids = [c.get("id") for c in d.get("checks", []) if isinstance(c, dict)]
        ok = d.get("host", {}).get("plugin_id") == "oh-my-musecode" and ids[:1] == ["D1"] and "D15" in ids and "exit_code" in d
        return ok, "checks=%s exit_code=%s" % (len(ids), d.get("exit_code"))
    if tool == "omm_cost":
        ok = isinstance(d.get("catalog"), dict) and isinstance(d.get("tokens"), dict) and d.get("host", {}).get("plugin_id") == "oh-my-musecode"
        return ok, "catalog.total_bytes=%s" % d.get("catalog", {}).get("total_bytes")
    return False, "unknown tool"


p2 = []
p2_ok = True
for r in resolved:
    if "call_id" not in r:
        continue
    k = r["request"]
    # 1.3.0 interleaves subagent requests (no mcp namespace) between the
    # call and its output: search forward from the emitting request (posts
    # is 0-based, request numbers 1-based).
    got = None
    found = None
    for idx in range(k - 1, len(posts)):
        for fco in posts[idx]["function_call_outputs"]:
            if fco["call_id"] == r["call_id"]:
                got = fco["output"]
                found = idx + 1
                break
        if got is not None:
            break
    tool = (r.get("resolved") or {}).get("function") or tool_of(r.get("wire_name", ""))
    ok, why = (False, "no function_call_output in this or any later request") if got is None else looks_like(tool, got)
    p2.append({"emitted_in_request": k, "call_id": r["call_id"], "wire_name": r.get("wire_name"), "tool": tool,
               "ok": ok, "found_in_request": found, "detail": why, "bytes": len(got or "")})
    p2_ok &= ok
p2_ok &= len(p2) >= 1
checks.append(("P2 each tool result is the function_call_output of its own or a later provider request and parses as the doctor / cost JSON", p2_ok, p2))

# P3
canon = {"mcp__plugin_oh_my_musecode_doc__" + tool_of(c["name"]) for _, c in scripted}
seen_calls = {n for r in call_records for n in r["names"]}
p3_ok = canon <= seen_calls and any(any(('"checks"' in t or '"catalog"' in t) for t in r["texts"]) for r in result_records)
checks.append(("P3 session.jsonl carries the canonical ids on the tool calls and the JSON documents on the results", p3_ok,
               {"expected_canonical": sorted(canon), "seen": sorted(seen_calls), "result_records": report["result_records"]}))
report["checks"] = [{"name": n, "ok": ok, "detail": d} for n, ok, d in checks]

if AS_JSON:
    print(json.dumps(report, indent=1))
    sys.exit(0 if all(ok for _, ok, _ in checks) else 1)

print("work dir : %s" % W)
print("session  : %s" % sf)
print("trace    : %s (%s)" % (trace_path, "present" if os.path.exists(trace_path) else "ABSENT — the server never started"))
print()
print("== identity catalog ==")
for e in (catalog or {}).get("entries", []):
    ns = e.get("surface", {}).get("namespace", "")
    print("  canonical_id=%-44s surface=%s.%s  server=%s" % (e.get("canonical_id"), ns, e.get("surface", {}).get("name"), e.get("server_name")))
print("== provider requests ==")
for i, p in enumerate(posts):
    if p.get("parse_error"):
        print("  #%d  (unparseable body)" % (i + 1))
        continue
    print("  #%d  tools=%d  mcp namespaces=%s" % (i + 1, p["n_tools"], json.dumps(p["mcp_namespaces"])))
    for f in p["function_calls"]:
        print("       input function_call        %s %s %s" % (f["call_id"], f["name"], f["arguments"]))
    for f in p["function_call_outputs"]:
        print("       input function_call_output %s -> %d B: %s…" % (f["call_id"], len(f["output"] or ""), (f["output"] or "")[:100]))
print("== server trace ==")
for s in trace["spawns"]:
    print("  spawn cwd=%s argv=%s omm_version=%s" % (s.get("cwd"), s.get("argv"), s.get("omm_version")))
print("  recv=%s sent=%d" % (trace["recv"], trace["sent"]))
for c in trace["calls"]:
    print("  tools/call id=%s name=%s arguments=%s" % (c["id"], c["name"], json.dumps(c["arguments"])))
for t in trace["tool"]:
    print("  tool id=%s name=%s isError=%s bytes=%s" % (t.get("id"), t.get("name"), t.get("isError"), t.get("bytes")))
print()
print("== verdicts ==")
for n, ok, d in checks:
    print("  [%s] %s" % ("PASS" if ok else "FAIL", n))
    if n.startswith("P1"):
        for x in d["steps"]:
            print("         %s step %d %s %s -> jsonrpc_id=%s" % ("ok  " if x["ok"] else "BAD ", x["step"], x["tool"], json.dumps(x["arguments"]), x["jsonrpc_id"]))
    if n.startswith("P2"):
        for x in d:
            print("         %s %s emitted in request #%d as %s -> output in request #%s: %s (%d B)" % (
                "ok  " if x["ok"] else "BAD ", x["call_id"], x["emitted_in_request"], x["wire_name"], x["found_in_request"], x["detail"], x["bytes"]))
    if n.startswith("P3"):
        print("         expected %s seen %s" % (d["expected_canonical"], d["seen"]))
sys.exit(0 if all(ok for _, ok, _ in checks) else 1)
