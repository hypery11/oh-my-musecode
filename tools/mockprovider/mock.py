#!/usr/bin/env python3
"""Local stand-in for the Meta model API, bound to 127.0.0.1 only.

Muse is pointed at it with
    muse exec --provider meta --base-url http://127.0.0.1:8731 --model test-model ...
or, for lanes without a --base-url flag (muse serve), with
    settings.json -> {"provider":"meta","endpoint_transport":{"base_url":"http://127.0.0.1:8731"}}
plus META_API_KEY=<anything> in the environment. With that override the binary's only network
destination is this socket (containment proven in research/experiments/skill-routing.md V3).

Routes (research/musecode/model-providers.md section 5.1):
    GET  <base>/muse-code/models   -> models.json          (the model catalog)
    POST <base>/responses          -> the responder script (the Responses-API SSE stream)

Configuration, all via environment:
    MOCK_PORT       listen port                      (default 8731)
    MOCK_STATE      directory the responder may use  (default: this file's directory)
    MOCK_LOG        JSONL transcript path            (default: $MOCK_STATE/mock.log)
    MOCK_RESPONDER  responder script path            (default: respond.py next to this file)
    MOCK_MODELS     models catalog JSON path         (default: models.json next to this file)

The responder is exec'd for every POST with these globals:
    body (str), path (str), headers (dict), state (str: MOCK_STATE), log (str: MOCK_LOG)
and must set:
    out (str or bytes), ctype (str, default application/json), code (int, default 200)

Every request and every response is appended to MOCK_LOG as one JSON object per line:
    {"m":"GET"|"POST","path":...,"h":{...},"body":...}
    {"m":"RESP","path":...,"code":...,"ctype":...,"out":...}
"""
import http.server
import json
import os
import socketserver
import sys
import time
import traceback

D = os.path.dirname(os.path.abspath(__file__))
PORT = int(os.environ.get("MOCK_PORT", "8731"))
STATE = os.environ.get("MOCK_STATE", D)
LOG = os.environ.get("MOCK_LOG", os.path.join(STATE, "mock.log"))
RESPONDER = os.environ.get("MOCK_RESPONDER", os.path.join(D, "respond.py"))
MODELS = os.environ.get("MOCK_MODELS", os.path.join(D, "models.json"))


def _append(o):
    os.makedirs(os.path.dirname(os.path.abspath(LOG)), exist_ok=True)
    with open(LOG, "a") as f:
        f.write(json.dumps(o) + "\n")


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json"):
        b = body.encode() if isinstance(body, str) else body
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(b)))
        self.end_headers()
        self.wfile.write(b)
        _append({"m": "RESP", "path": self.path, "code": code, "ctype": ctype,
                 "out": b.decode(errors="replace"), "t": time.time()})

    def do_GET(self):
        _append({"m": "GET", "path": self.path, "h": dict(self.headers), "t": time.time()})
        if self.path.endswith("/models"):
            if os.path.exists(MODELS):
                return self._send(200, open(MODELS).read())
            return self._send(200, '{"data":[{"id":"test-model","object":"model"}]}')
        self._send(404, '{"error":"not found"}')

    def do_POST(self):
        n = int(self.headers.get("Content-Length", "0"))
        body = self.rfile.read(n).decode(errors="replace")
        _append({"m": "POST", "path": self.path, "h": dict(self.headers), "body": body, "t": time.time()})
        if not os.path.exists(RESPONDER):
            return self._send(400, '{"error":{"message":"no responder at %s"}}' % RESPONDER)
        g = {"body": body, "path": self.path, "headers": dict(self.headers),
             "state": STATE, "log": LOG,
             "out": None, "ctype": "application/json", "code": 200}
        try:
            exec(compile(open(RESPONDER).read(), RESPONDER, "exec"), g)
        except Exception:
            tb = traceback.format_exc()
            _append({"m": "RESPONDER-ERROR", "path": self.path, "trace": tb})
            sys.stderr.write(tb)
            return self._send(500, '{"error":{"message":"responder raised"}}')
        if g["out"] is None:
            return self._send(500, '{"error":{"message":"responder set no out"}}')
        self._send(g["code"], g["out"], g["ctype"])


class S(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


if __name__ == "__main__":
    os.makedirs(STATE, exist_ok=True)
    sys.stderr.write("mock: listening on 127.0.0.1:%d state=%s responder=%s log=%s\n"
                     % (PORT, STATE, RESPONDER, LOG))
    sys.stderr.flush()
    S(("127.0.0.1", PORT), H).serve_forever()
