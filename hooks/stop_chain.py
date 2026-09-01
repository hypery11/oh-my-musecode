#!/usr/bin/env python3
"""Stop — keep Ralph loops alive while iterations remain."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "stop-chain"


def main() -> None:
    event = omm.read_stdin_json()
    ralph = omm.load_json(omm.omm_dir(event) / "ralph.json")
    active = False
    iterations = 0
    maximum = 0
    if isinstance(ralph, dict):
        active = bool(ralph.get("active"))
        try:
            iterations = int(ralph.get("iterations") or 0)
        except (TypeError, ValueError):
            iterations = 0
        try:
            maximum = int(ralph.get("max") or ralph.get("maxIterations") or 0)
        except (TypeError, ValueError):
            maximum = 0
    omm.audit(event, HOOK_ID, {"active": active, "iterations": iterations, "max": maximum})
    if active and iterations < maximum:
        omm.emit({"systemMessage": "Ralph loop still active. Continue the task. Do not stop."})
        return
    omm.emit({})


if __name__ == "__main__":
    main()
