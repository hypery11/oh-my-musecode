#!/usr/bin/env python3
"""Containment probe: a loopback listener that records every request Muse makes and answers 400.
    usage: probe_server.py <log-path>      (binds 127.0.0.1:8731)
Point muse at it with --base-url http://127.0.0.1:8731 (or endpoint_transport.base_url) and confirm the
log holds GET /muse-code/models and POST /responses and nothing else reached any other host.
Proof that --base-url is total: research/experiments/skill-routing.md, Verification V3."""
import http.server,json,sys,threading,socketserver
LOG=sys.argv[1]
class H(http.server.BaseHTTPRequestHandler):
    def log_message(self,*a): pass
    def do_POST(self):
        n=int(self.headers.get("Content-Length","0"))
        body=self.rfile.read(n)
        with open(LOG,"a") as f:
            f.write(json.dumps({"path":self.path,"headers":dict(self.headers),"body":body.decode(errors="replace")[:4000]})+"\n")
        self.send_response(400); self.send_header("Content-Type","application/json"); self.end_headers()
        self.wfile.write(b'{"error":{"message":"probe"}}')
    def do_GET(self):
        with open(LOG,"a") as f: f.write(json.dumps({"path":self.path,"method":"GET"})+"\n")
        self.send_response(404); self.end_headers()
class S(socketserver.ThreadingTCPServer): allow_reuse_address=True
S(("127.0.0.1",8731),H).serve_forever()
