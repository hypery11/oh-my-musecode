#!/usr/bin/env python3
"""Offline tests: pre-compact, session-end webhook, intent-gate fail-open."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PRE = ROOT / "hooks" / "pre_compact.py"
END = ROOT / "hooks" / "session_end.py"
GATE = ROOT / "hooks" / "skill_gate.py"
PROMPT = ROOT / "hooks" / "user_prompt.py"


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def run_hook(script: Path, cwd: Path, payload: dict) -> dict:
    body = dict(payload)
    body["cwd"] = str(cwd)
    proc = subprocess.run(
        [sys.executable, str(script)],
        input=json.dumps(body),
        text=True,
        capture_output=True,
        check=False,
        cwd=str(cwd),
    )
    if proc.returncode != 0:
        fail(f"{script.name} exit {proc.returncode}: {proc.stderr}")
    line = (proc.stdout or "").strip().splitlines()[-1]
    return json.loads(line)


def main() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="omm-hooks-extra-"))
    try:
        out = run_hook(PRE, tmp, {"hook_event_name": "PreCompact"})
        if out != {}:
            fail(f"pre_compact should emit {{}}, got {out}")
        compact = json.loads((tmp / ".omm" / "compact.json").read_text(encoding="utf-8"))
        if "ts" not in compact or "note" not in compact:
            fail(f"compact.json {compact}")
        mem = (tmp / ".omm" / "memory.md").read_text(encoding="utf-8")
        if "compact" not in mem:
            fail("memory.md missing compact line")

        out = run_hook(END, tmp, {"hook_event_name": "SessionEnd", "session_id": "abc"})
        if out != {}:
            fail(f"session_end no-op should emit {{}}, got {out}")

        received: list[bytes] = []

        class H(BaseHTTPRequestHandler):
            def do_POST(self):
                n = int(self.headers.get("Content-Length") or 0)
                received.append(self.rfile.read(n))
                self.send_response(200)
                self.end_headers()

            def log_message(self, *_a):
                return

        httpd = HTTPServer(("127.0.0.1", 0), H)
        port = httpd.server_address[1]
        thread = threading.Thread(target=httpd.handle_request, daemon=True)
        thread.start()
        (tmp / ".omm" / "notify.json").write_text(
            json.dumps({"url": f"http://127.0.0.1:{port}/hook"}) + "\n",
            encoding="utf-8",
        )
        out = run_hook(END, tmp, {"hook_event_name": "SessionEnd", "session_id": "sess-1"})
        thread.join(timeout=4)
        if out != {}:
            fail(f"session_end webhook should still emit {{}}, got {out}")
        if not received:
            fail("webhook POST not received")
        body = json.loads(received[0].decode("utf-8"))
        if body.get("event") != "session-end" or body.get("session_id") != "sess-1":
            fail(f"webhook body {body}")

        # malformed notify must not crash
        (tmp / ".omm" / "notify.json").write_text("{not-json", encoding="utf-8")
        out = run_hook(END, tmp, {"hook_event_name": "SessionEnd"})
        if out != {}:
            fail("malformed notify should no-op")

        # intent-gate fail-open when absent
        out = run_hook(
            GATE,
            tmp,
            {"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"path": "x"}},
        )
        if out != {}:
            fail(f"intent-gate absent should allow, got {out}")

        ig_dir = tmp / "ig"
        ig_dir.mkdir()
        (ig_dir / ".omm").mkdir()
        (ig_dir / ".omm" / "intent-gate.json").write_text(
            json.dumps({"required": ["plan"]}) + "\n", encoding="utf-8"
        )
        out = run_hook(
            GATE,
            ig_dir,
            {"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"path": "x"}},
        )
        perm = (out.get("hookSpecificOutput") or {}).get("permissionDecision")
        if perm != "deny":
            fail(f"intent-gate missing plan should deny Write, got {out}")

        (ig_dir / ".omm" / "plan.json").write_text('{"steps": []}\n', encoding="utf-8")
        out = run_hook(
            GATE,
            ig_dir,
            {"hook_event_name": "PreToolUse", "tool_name": "Write", "tool_input": {"path": "x"}},
        )
        if out != {}:
            fail(f"intent-gate with plan.json should allow, got {out}")

        # UserPromptSubmit with gate present must not crash
        out = run_hook(
            PROMPT,
            ig_dir,
            {"hook_event_name": "UserPromptSubmit", "prompt": "hello there"},
        )
        if out != {}:
            fail(f"prompt with intent-gate should emit {{}}, got {out}")

        print("ok  pre-compact / session-end webhook / intent-gate")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
