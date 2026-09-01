#!/usr/bin/env python3
"""Stop — block session end while a Ralph loop still has budget.

Confirmed Muse hook stdout (PreToolUse hook test, same JSON decoder):
  {"decision":"block","reason":"..."}  → should_block true
  {}                                   → allow stop
  systemMessage is shown on the terminal

Live Stop extra stdin fields (stop_reason, last_assistant_message) are
UNKNOWN on 1.0.1; we read them if present and ignore if absent.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "stop-chain"
DONE_RE = re.compile(r"<promise>\s*DONE\s*</promise>", re.IGNORECASE)
ABORT_RE = re.compile(r"^(cancel|cancelled|canceled|abort|aborted|error|failed)$", re.IGNORECASE)


def _int(val, default: int = 0) -> int:
    try:
        return int(val)
    except (TypeError, ValueError):
        return default


def last_text(event: dict) -> str:
    for key in (
        "last_assistant_message",
        "lastAssistantMessage",
        "assistant_message",
        "transcript",
        "text",
    ):
        val = event.get(key)
        if isinstance(val, str) and val.strip():
            return val
    return ""


def stop_reason(event: dict) -> str:
    for key in ("stop_reason", "stopReason", "reason"):
        val = event.get(key)
        if isinstance(val, str) and val.strip():
            return val.strip()
    return ""


def should_abort(event: dict) -> bool:
    if event.get("stop_hook_active") or event.get("stopHookActive"):
        return True
    tasks = event.get("background_tasks") or event.get("backgroundTasks") or []
    if isinstance(tasks, list) and tasks:
        return True
    reason = stop_reason(event)
    return bool(reason and ABORT_RE.match(reason))


def main() -> None:
    event = omm.read_stdin_json()
    path = omm.omm_dir(event) / "ralph.json"
    ralph = omm.load_json(path)
    if not isinstance(ralph, dict):
        ralph = {}
    active = bool(ralph.get("active"))
    iterations = _int(ralph.get("iterations"), 0)
    maximum = _int(ralph.get("max") or ralph.get("maxIterations"), 0)
    goal = str(ralph.get("goal") or "")
    text = last_text(event)
    done = bool(text and DONE_RE.search(text))
    abort = should_abort(event)
    extra = {
        "active": active,
        "iterations": iterations,
        "max": maximum,
        "done": done,
        "abort": abort,
    }
    omm.audit(event, HOOK_ID, extra)

    if abort or not active:
        omm.emit({})
        return

    if done or (maximum > 0 and iterations >= maximum):
        ralph["active"] = False
        ralph["status"] = "done" if done else "budget"
        ralph["iterations"] = iterations
        omm.write_json(path, ralph)
        omm.emit({})
        return

    if maximum <= 0:
        omm.emit({})
        return

    ralph["iterations"] = iterations + 1
    ralph["active"] = True
    omm.write_json(path, ralph)
    nxt = ralph["iterations"]
    reason = (
        f"Ralph loop {nxt}/{maximum} still active"
        + (f" for: {goal}" if goal else "")
        + ". Continue. End with <promise>DONE</promise> when the goal is met."
    )
    omm.emit(
        {
            "decision": "block",
            "reason": reason,
            "systemMessage": reason,
        }
    )


if __name__ == "__main__":
    main()
