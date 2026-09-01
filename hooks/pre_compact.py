#!/usr/bin/env python3
"""PreCompact — placeholder. Mention .omm/ so compaction-aware roles flush state.

This hook currently emits an empty decision. Skills should already have written
durable notes under .omm/ before context is compacted.
"""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "pre-compact"


def main() -> None:
    event = omm.read_stdin_json()
    omm.audit(event, HOOK_ID, {"note": "flush .omm/ before compact"})
    # Placeholder: keep .omm/ mentioned so operators remember to persist state.
    omm.emit({})


if __name__ == "__main__":
    main()
