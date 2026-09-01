#!/usr/bin/env python3
"""Stop — block session end while a file-based loop still has budget.

Confirmed Muse hook stdout (PreToolUse hook test, same JSON decoder):
  {"decision":"block","reason":"..."}  → should_block true
  {}                                   → allow stop
  systemMessage is shown on the terminal

Live Stop extra stdin fields (stop_reason, last_assistant_message) are
UNKNOWN on 1.0.1; we read them if present and ignore if absent.

Order: ralph, then ulw/ultrawork, then boulder, then a capped todo nudge.
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


def handle_loop(event: dict, path: Path, label: str) -> dict | None:
    """Ralph-style iteration loop. None = skip; dict = emit (block or allow)."""
    state = omm.load_json(path)
    if not isinstance(state, dict) or not state.get("active"):
        return None
    iterations = _int(state.get("iterations"), 0)
    maximum = _int(state.get("max") or state.get("maxIterations"), 0)
    goal = str(state.get("goal") or "")
    text = last_text(event)
    done = bool(text and DONE_RE.search(text))
    if done or (maximum > 0 and iterations >= maximum):
        state["active"] = False
        state["status"] = "done" if done else "budget"
        state["iterations"] = iterations
        omm.write_json(path, state)
        return None
    if maximum <= 0:
        return None
    state["iterations"] = iterations + 1
    state["active"] = True
    omm.write_json(path, state)
    nxt = state["iterations"]
    reason = (
        f"{label} loop {nxt}/{maximum} still active"
        + (f" for: {goal}" if goal else "")
        + ". Continue. End with <promise>DONE</promise> when the goal is met."
    )
    return {"decision": "block", "reason": reason, "systemMessage": reason}


def _open_todos(items: list) -> list:
    open_items = []
    for it in items:
        if isinstance(it, dict):
            st = str(it.get("status") or "pending").lower()
            if st not in ("done", "completed", "cancelled", "canceled"):
                open_items.append(it)
        elif isinstance(it, str) and it.strip():
            open_items.append(it)
    return open_items


def handle_todo(event: dict, directory: Path) -> dict | None:
    path = directory / "todo.json"
    data = omm.load_json(path)
    if isinstance(data, list):
        items = data
        wrapper: dict = {"items": items, "nudge": 0, "nudge_cap": 1}
    elif isinstance(data, dict):
        items = data.get("items") or data.get("todos") or []
        wrapper = data
    else:
        return None
    if not isinstance(items, list):
        return None
    open_items = _open_todos(items)
    if not open_items:
        return None
    nudge = _int(wrapper.get("nudge") or wrapper.get("nudge_count"), 0)
    cap = _int(wrapper.get("nudge_cap") or wrapper.get("max_nudges"), 1)
    if cap < 1:
        cap = 1
    if nudge >= cap:
        return None
    wrapper["items"] = items
    wrapper["nudge"] = nudge + 1
    wrapper["nudge_cap"] = cap
    omm.write_json(path, wrapper)
    reason = (
        f"Todo list has {len(open_items)} open item(s). Continue, "
        "then mark items done in .omm/todo.json."
    )
    return {"decision": "block", "reason": reason, "systemMessage": reason}


def main() -> None:
    event = omm.unwrap_event(omm.read_stdin_json())
    directory = omm.omm_dir(event)
    path = directory / "ralph.json"
    ralph = omm.load_json(path)
    if not isinstance(ralph, dict):
        ralph = {}
    active = bool(ralph.get("active"))
    iterations = _int(ralph.get("iterations"), 0)
    maximum = _int(ralph.get("max") or ralph.get("maxIterations"), 0)
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

    if abort:
        omm.emit({})
        return

    for candidate, label in (
        (path, "Ralph"),
        (directory / "ulw.json", "Ultrawork"),
        (directory / "ultrawork.json", "Ultrawork"),
        (directory / "boulder.json", "Boulder"),
    ):
        out = handle_loop(event, candidate, label)
        if out is not None:
            omm.emit(out)
            return

    out = handle_todo(event, directory)
    if out is not None:
        omm.emit(out)
        return
    omm.emit({})


if __name__ == "__main__":
    main()
