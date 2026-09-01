#!/usr/bin/env python3
"""PreCompact — persist a compact marker and a memory line, then emit {}."""
from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "pre-compact"


def main() -> None:
    event = omm.unwrap_event(omm.read_stdin_json())
    directory = omm.omm_dir(event)
    note = "flush .omm/ before compact"
    ts = omm.utc_now()
    omm.write_json(directory / "compact.json", {"ts": ts, "note": note})
    try:
        mem = directory / "memory.md"
        mem.parent.mkdir(parents=True, exist_ok=True)
        with mem.open("a", encoding="utf-8") as fh:
            fh.write(f"- compact {ts}: {note}\n")
    except OSError:
        pass
    omm.audit(event, HOOK_ID, {"note": note})
    omm.emit({})


if __name__ == "__main__":
    main()
