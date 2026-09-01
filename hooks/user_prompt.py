#!/usr/bin/env python3
"""UserPromptSubmit — detect ralph/ralplan/ultrathink/autopilot keywords."""
from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "prompt-keywords"

# Longer tokens first so ralplan wins over ralph.
KEYWORDS = ("ralplan", "ralph", "ultrathink", "autopilot")


def extract_prompt(event: dict) -> str:
    for key in ("prompt", "user_prompt", "userPrompt", "text", "message"):
        val = event.get(key)
        if isinstance(val, str):
            return val
    return ""


def main() -> None:
    event = omm.read_stdin_json()
    prompt = extract_prompt(event)
    lower = prompt.lower()
    matched = None
    for kw in KEYWORDS:
        if re.search(rf"(?<![a-z0-9]){kw}(?![a-z0-9])", lower) or kw in lower:
            matched = kw
            break
    extra = {"matched": matched} if matched else {}
    omm.audit(event, HOOK_ID, extra)
    if matched:
        payload = {
            "mode": matched,
            "source": "prompt-keywords",
            "ts": omm.utc_now(),
        }
        omm.write_json(omm.omm_dir(event) / "mode.json", payload)
    omm.emit({})


if __name__ == "__main__":
    main()
