#!/usr/bin/env python3
"""SessionEnd — audit, optional webhook POST, then emit {}."""
from __future__ import annotations

import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "session-end"


def _webhook_url(payload) -> str | None:
    if not isinstance(payload, dict):
        return None
    raw = payload.get("url") or payload.get("webhook") or payload.get("webhook_url")
    if not isinstance(raw, str):
        return None
    url = raw.strip()
    if url.lower().startswith("http://") or url.lower().startswith("https://"):
        return url
    return None


def main() -> None:
    event = omm.unwrap_event(omm.read_stdin_json())
    omm.audit(event, HOOK_ID)
    notify = omm.load_json(omm.omm_dir(event) / "notify.json")
    url = _webhook_url(notify)
    if url:
        body = json.dumps(
            {
                "event": "session-end",
                "session_id": event.get("session_id") or event.get("sessionId") or "",
            }
        ).encode("utf-8")
        req = urllib.request.Request(
            url,
            data=body,
            method="POST",
            headers={"Content-Type": "application/json"},
        )
        try:
            urllib.request.urlopen(req, timeout=3)
        except (urllib.error.URLError, TimeoutError, OSError, ValueError):
            pass
    omm.emit({})


if __name__ == "__main__":
    main()
