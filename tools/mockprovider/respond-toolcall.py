# Responder for mock.py: scripts the "model" to emit tool calls, then a final answer.
#
# Executed by mock.py for every POST /responses with globals:
#     body, path, headers, state, log      (see mock.py)
# and sets: out, ctype, code.
#
# Plan file: $state/plan.json — a JSON list of steps, consumed in order. A step is either
#     {"call": {"name": "<spec>", "arguments": {...}, "form": "<form>", "raw_name": "...", "extra": {...},
#               "expect": "ok" | "reject"}}        (expect is read by analyze_mcp_toolcall.py only)
#     {"text": "<assistant message>"}
# When the plan is exhausted the responder answers with the text "MOCK-FINAL-ANSWER".
#
# Which step applies is derived from the REQUEST, not from a counter: step index =
# the number of `function_call_output` items in the request's `input`. That makes the
# responder idempotent under provider retries (Muse retries a stream it cannot decode
# up to ten times; a counter would run ahead).
#
# Tool specs on the wire. Muse sends `tools[]` in the Meta Responses dialect as NAMESPACE groups:
#     {"type":"namespace","name":"mcp__plugin_<pid>_<sid>","description":"Tools provided by an MCP server.",
#      "tools":[{"type":"function","name":"echo_upper","parameters":{...}}]}
# (the built-ins sit in a namespace called "muse"). The responder flattens every namespace into
# candidates (ns, fn) and resolves `name` against `ns__fn` (or bare `fn` for un-namespaced tools):
#     exact                    -> used verbatim
#     "/regex/"                -> must match exactly one candidate
#     anything else            -> `<spec>` or `*__<spec>` (exactly one hit)
# The function_call item's `name` is then rendered per `form` (step-level, else $state/form.txt,
# else the default below):
#     ns__fn        "mcp__plugin_omm_up__echo_upper"                     (the canonical id)
#     fn            "echo_upper"
#     fn+namespace  "echo_upper" plus a sibling field "namespace": ns
#     ns.fn         "mcp__plugin_omm_up.echo_upper"
#     ns/fn         "mcp__plugin_omm_up/echo_upper"
# `raw_name` bypasses resolution entirely; `extra` is merged into the function_call item.
# Every decision is appended to $state/resolved.jsonl for the analyzer.
#
# Wire format (research/experiments/skill-routing.md V3, loose-ends.md section 2.1):
# The Responses-API SSE stream: `event:` + `data:` frames, `sequence_number` on EVERY frame, and only
# these event types: response.created, response.function_call_arguments.done,
# response.output_item.done, response.completed. Anything else (output_item.added,
# in_progress, content_part.*) makes the client fail the stream with error_kind "decode".
import json
import os
import re
import time

# ns.fn is the only form that dispatches for EVERY namespace length (docs/experiments/mcp-tools-call.md);
# ns__fn works only while the wire namespace equals the canonical one (len(pid)+len(sid) <= 18).
DEFAULT_FORM = "ns.fn"

req = json.loads(body)
inp = req.get("input") or []
n_done = sum(1 for it in inp if isinstance(it, dict) and it.get("type") == "function_call_output")

# flatten namespaces -> [(ns, fn)]
cands = []
for t in (req.get("tools") or []):
    if not isinstance(t, dict):
        continue
    if t.get("type") == "namespace":
        for f in (t.get("tools") or []):
            if isinstance(f, dict) and f.get("name"):
                cands.append((t.get("name"), f["name"]))
    elif t.get("name"):
        cands.append((None, t["name"]))
display = [(ns + "__" + fn) if ns else fn for ns, fn in cands]

# request ordinal, for the transcript only
_cp = os.path.join(state, "requests.txt")
try:
    _k = int(open(_cp).read().strip())
except Exception:
    _k = 0
_k += 1
open(_cp, "w").write(str(_k))

_pp = os.path.join(state, "plan.json")
plan = json.load(open(_pp)) if os.path.exists(_pp) else []
step = plan[n_done] if n_done < len(plan) else {"text": "MOCK-FINAL-ANSWER"}
_fp = os.path.join(state, "form.txt")
form_default = open(_fp).read().strip() if os.path.exists(_fp) else DEFAULT_FORM


def _resolve(spec):
    if spec in display:
        return cands[display.index(spec)]
    if len(spec) > 2 and spec.startswith("/") and spec.endswith("/"):
        rx = re.compile(spec[1:-1])
        hits = [c for c, d in zip(cands, display) if rx.search(d)]
    else:
        hits = [c for c, d in zip(cands, display) if d == spec or d.endswith("__" + spec)]
    if len(hits) == 1:
        return hits[0]
    raise LookupError("tool %r resolved to %d candidates among %d: %s" % (spec, len(hits), len(display), hits))


def _render(ns, fn, form):
    if ns is None or form == "fn":
        return fn, {}
    if form == "ns__fn":
        return ns + "__" + fn, {}
    if form == "fn+namespace":
        return fn, {"namespace": ns}
    if form == "ns.fn":
        return ns + "." + fn, {}
    if form == "ns/fn":
        return ns + "/" + fn, {}
    raise ValueError("unknown form %r" % form)


_seq = [0]


def _ev(t, o):
    _seq[0] += 1
    o["type"] = t
    o["sequence_number"] = _seq[0]
    return "event: %s\ndata: %s\n\n" % (t, json.dumps(o))


def _resp(rid, status, output):
    now = int(time.time())
    return {"id": rid, "object": "response", "model": req.get("model", "test-model"), "status": status,
            "created_at": now, "completed_at": now if status == "completed" else None,
            "output": output,
            "usage": {"input_tokens": 10, "output_tokens": 5, "total_tokens": 15,
                      "input_tokens_details": {"cached_tokens": 0},
                      "output_tokens_details": {"reasoning_tokens": 0}},
            "error": None, "previous_response_id": None, "metadata": {},
            "incomplete_details": None, "status_details": None}


rid = "resp_%d" % _k
record = {"request": _k, "n_done": n_done, "step_index": n_done, "step": step, "tools": display}
if "call" in step:
    c = step["call"]
    try:
        form = c.get("form") or form_default
        if c.get("raw_name"):
            wire, extra = c["raw_name"], {}
            record["resolved"] = None
        else:
            ns, fn = _resolve(c["name"])
            wire, extra = _render(ns, fn, form)
            record["resolved"] = {"namespace": ns, "function": fn}
        extra.update(c.get("extra") or {})
        args = json.dumps(c.get("arguments", {}))
        item = {"type": "function_call", "id": "fc_%d" % n_done, "call_id": "call_%d" % n_done,
                "name": wire, "arguments": args, "status": "completed"}
        item.update(extra)
        record.update({"wire_name": wire, "form": form, "extra": extra, "call_id": item["call_id"]})
    except (LookupError, ValueError) as e:
        item = {"type": "message", "id": "msg_%d" % _k, "role": "assistant", "status": "completed",
                "content": [{"type": "output_text", "text": "MOCK-UNRESOLVED: %s" % e, "annotations": []}]}
        record["error"] = str(e)
else:
    item = {"type": "message", "id": "msg_%d" % _k, "role": "assistant", "status": "completed",
            "content": [{"type": "output_text", "text": step.get("text", "MOCK-FINAL-ANSWER"), "annotations": []}]}

with open(os.path.join(state, "resolved.jsonl"), "a") as f:
    f.write(json.dumps(record) + "\n")

parts = [_ev("response.created", {"response": _resp(rid, "in_progress", [])})]
if item["type"] == "function_call":
    parts.append(_ev("response.function_call_arguments.done",
                     {"item_id": item["id"], "output_index": 0, "arguments": item["arguments"]}))
parts.append(_ev("response.output_item.done", {"output_index": 0, "item": item}))
parts.append(_ev("response.completed", {"response": _resp(rid, "completed", [item])}))
out = "".join(parts)
ctype = "text/event-stream"
code = 200
