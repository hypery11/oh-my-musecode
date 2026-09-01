#!/usr/bin/env python3
"""SessionEnd — audit and exit quietly."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "session-end"


def main() -> None:
    event = omm.read_stdin_json()
    omm.audit(event, HOOK_ID)
    omm.emit({})


if __name__ == "__main__":
    main()
