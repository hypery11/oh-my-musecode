#!/usr/bin/env python3
"""SessionStart — greet with available Oh My Muse Code slash-commands."""
from __future__ import annotations

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "session-start"

COMMANDS = [
    "team", "autopilot", "execute", "ralph", "ralplan", "deep-interview",
    "ask", "verify", "ultragoal", "handoff", "skillify", "omm-skill",
    "hud", "omm-setup", "omm-doctor", "remember", "omm-trace", "wiki", "debug",
]


def main() -> None:
    event = omm.read_stdin_json()
    omm.audit(event, HOOK_ID)
    listing = " ".join("/" + c for c in COMMANDS)
    msg = (
        "Oh My Muse Code is loaded. Durable session state lives under .omm/. "
        f"Slash-commands: {listing}. "
        "Read skills via the plugin skill catalog; use subagent_spawn and "
        ".muse/worktrees/ for isolated work."
    )
    omm.emit({"systemMessage": msg})


if __name__ == "__main__":
    main()
