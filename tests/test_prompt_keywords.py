#!/usr/bin/env python3
"""Offline test: UserPromptSubmit ralph keyword writes .omm/mode.json."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOOK = ROOT / "hooks" / "user_prompt.py"
FIXTURE = ROOT / "tests" / "fixtures" / "prompt-ralph.json"


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def run_hook(payload: dict) -> None:
    proc = subprocess.run(
        [sys.executable, str(HOOK)],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        fail(f"hook exit {proc.returncode}: {proc.stderr}")


def assert_mode(cwd: Path, expected: str = "ralph") -> None:
    path = cwd / ".omm" / "mode.json"
    if not path.is_file():
        fail(f"missing {path}")
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("mode") != expected:
        fail(f"mode={data.get('mode')!r} expected {expected!r}")
    if data.get("source") != "prompt-keywords":
        fail(f"source={data.get('source')!r}")


def main() -> None:
    if not FIXTURE.is_file():
        fail(f"missing fixture {FIXTURE}")
    fixture = json.loads(FIXTURE.read_text(encoding="utf-8"))
    stdin = fixture.get("stdin") if isinstance(fixture.get("stdin"), dict) else {}
    prompt = stdin.get("prompt") or fixture.get("prompt") or ""
    if "ralph" not in str(prompt).lower():
        fail("fixture prompt must contain ralph")
    tmp = Path(tempfile.mkdtemp(prefix="omm-prompt-ralph-"))
    try:
        wrapper = json.loads(json.dumps(fixture))
        if not isinstance(wrapper.get("stdin"), dict):
            fail("fixture must have dict stdin")
        wrapper["stdin"]["cwd"] = str(tmp)
        run_hook(wrapper)
        assert_mode(tmp)
        tmp2 = tmp / "flat"
        tmp2.mkdir()
        flat = {
            "hook_event_name": "UserPromptSubmit",
            "cwd": str(tmp2),
            "prompt": "ralph please continue",
        }
        run_hook(flat)
        assert_mode(tmp2)
        print("ok  prompt-keywords wrapper+flat wrote .omm/mode.json")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
