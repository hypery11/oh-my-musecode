#!/usr/bin/env python3
"""Minimal stdio MCP server that exposes one tool, `echo_upper`, and logs every JSON-RPC
message it receives or sends.

    usage: mcp_echo_server.py <tag> [<log-path>]

Shipped inside a plugin package as `mcp/server.py` and declared as
    {"id": "<sid>", "transport": "stdio", "command": ["python3", "mcp/server.py", "<sid>"]}

Log location. A plugin MCP server gets a scrubbed 16-key environment (research/experiments/
plugin-mcp.md V.8) — no custom variables reach it — so the log path is derived from
MUSE_PLUGIN_DATA_DIR ($XDG_DATA_HOME/muse/plugins/data/<plugin-id>, advertised but NOT created by
the host), falling back to MCP_ECHO_LOG_DIR, then the cwd. An explicit second argv wins.

Framing. Muse's MCP client speaks line-delimited JSON (protocolVersion 2024-11-05, clientInfo
{"name":"tbh"}); Content-Length framing is also accepted and mirrored, for other hosts.

Log line: <utc-ms>\tTAG=<tag>\tEV=<spawn|recv|sent|eof|exit|crash>\tPID=<pid>\t<json>
"""
import json
import os
import sys
import time

TAG = sys.argv[1] if len(sys.argv) > 1 else "srv"
_logdir = (os.environ.get("MUSE_PLUGIN_DATA_DIR") or os.environ.get("MCP_ECHO_LOG_DIR") or os.getcwd())
LOG = sys.argv[2] if len(sys.argv) > 2 else os.path.join(_logdir, "mcp-%s.log" % TAG)


def log(ev, obj):
    try:
        os.makedirs(os.path.dirname(os.path.abspath(LOG)), exist_ok=True)
        with open(LOG, "a") as f:
            f.write("%s\tTAG=%s\tEV=%s\tPID=%d\t%s\n" % (
                time.strftime("%Y-%m-%dT%H:%M:%S", time.gmtime()) + ".%03dZ" % (int(time.time() * 1000) % 1000),
                TAG, ev, os.getpid(), json.dumps(obj, sort_keys=True)))
    except Exception:
        pass


log("spawn", {"argv": sys.argv, "cwd": os.getcwd(), "ppid": os.getppid(),
              "env_keys": sorted(os.environ.keys()),
              "MUSE_PLUGIN_ID": os.environ.get("MUSE_PLUGIN_ID"),
              "MUSE_PLUGIN_ROOT": os.environ.get("MUSE_PLUGIN_ROOT"),
              "MUSE_PLUGIN_DATA_DIR": os.environ.get("MUSE_PLUGIN_DATA_DIR")})

inb = sys.stdin.buffer
outb = sys.stdout.buffer

TOOLS = [{
    "name": "echo_upper",
    "description": "Upper-cases the given text and returns it. Test tool shipped by the omm mock harness.",
    "inputSchema": {"type": "object",
                    "properties": {"text": {"type": "string", "description": "Text to upper-case."}},
                    "required": ["text"], "additionalProperties": False},
}]


def emit(obj, framing):
    data = json.dumps(obj).encode()
    if framing == "content_length":
        outb.write(b"Content-Length: %d\r\n\r\n" % len(data))
        outb.write(data)
    else:
        outb.write(data + b"\n")
    outb.flush()
    log("sent", obj)


def readframe():
    while True:
        line = inb.readline()
        if not line:
            return None, None
        s = line.strip()
        if not s:
            continue
        if s.lower().startswith(b"content-length:"):
            n = int(s.split(b":", 1)[1])
            while True:
                l2 = inb.readline()
                if l2 in (b"\r\n", b"\n", b""):
                    break
            return inb.read(n).decode(), "content_length"
        return s.decode(), "line"


def handle(msg, framing):
    m = msg.get("method")
    i = msg.get("id")
    if m == "initialize":
        emit({"jsonrpc": "2.0", "id": i, "result": {
            "protocolVersion": (msg.get("params") or {}).get("protocolVersion", "2024-11-05"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "omm-echo-" + TAG, "version": "0.1.0"}}}, framing)
    elif m == "tools/list":
        emit({"jsonrpc": "2.0", "id": i, "result": {"tools": TOOLS}}, framing)
    elif m == "tools/call":
        p = msg.get("params") or {}
        name = p.get("name")
        args = p.get("arguments") or {}
        if name == "echo_upper":
            text = str(args.get("text", ""))
            emit({"jsonrpc": "2.0", "id": i, "result": {
                "content": [{"type": "text", "text": "%s [echo_upper via server=%s pid=%d]" % (text.upper(), TAG, os.getpid())}],
                "isError": False}}, framing)
        else:
            emit({"jsonrpc": "2.0", "id": i, "result": {
                "content": [{"type": "text", "text": "unknown tool: %s" % name}], "isError": True}}, framing)
    elif m in ("resources/list", "prompts/list", "resources/templates/list"):
        key = {"resources/list": "resources", "prompts/list": "prompts", "resources/templates/list": "resourceTemplates"}[m]
        emit({"jsonrpc": "2.0", "id": i, "result": {key: []}}, framing)
    elif m == "ping":
        emit({"jsonrpc": "2.0", "id": i, "result": {}}, framing)
    elif i is not None:
        emit({"jsonrpc": "2.0", "id": i, "error": {"code": -32601, "message": "method not found: %s" % m}}, framing)
    # notifications (no id) get no reply


def main():
    while True:
        raw, framing = readframe()
        if raw is None:
            log("eof", {})
            return
        try:
            msg = json.loads(raw)
        except Exception as e:
            log("parse-error", {"error": str(e), "raw": raw[:500]})
            continue
        log("recv", msg)
        handle(msg, framing)


try:
    main()
except Exception as e:
    log("crash", {"error": repr(e)})
finally:
    log("exit", {})
