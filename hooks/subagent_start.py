#!/usr/bin/env python3
"""SubagentStart — append a team log line."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "subagent-start"


def main() -> None:
    event = omm.read_stdin_json()
    omm.audit(event, HOOK_ID)
    rec = {
        "ts": omm.utc_now(),
        "kind": "start",
        "hook": HOOK_ID,
        "agent_id": event.get("agent_id") or event.get("agentId") or "",
        "agent_type": event.get("agent_type") or event.get("agentType") or "",
        "session_id": event.get("session_id") or event.get("sessionId") or "",
    }
    omm.append_jsonl(omm.omm_dir(event) / "team" / "log.jsonl", rec)
    omm.emit({})


if __name__ == "__main__":
    main()
