#!/usr/bin/env python3
"""Offline tests for stop-chain ralph/ulw/boulder/todo (must not break ralph)."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOOK = ROOT / "hooks" / "stop_chain.py"
FIXTURE = ROOT / "tests" / "fixtures" / "stop-ralph.json"


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def run_hook(cwd: Path, extra: dict | None = None) -> dict:
    payload = json.loads(FIXTURE.read_text(encoding="utf-8"))
    if extra:
        if isinstance(payload.get("stdin"), dict):
            payload["stdin"].update(extra)
        payload.update({k: v for k, v in extra.items() if k != "cwd"})
    if isinstance(payload.get("stdin"), dict):
        payload["stdin"]["cwd"] = str(cwd)
    payload["cwd"] = str(cwd)
    proc = subprocess.run(
        [sys.executable, str(HOOK)],
        input=json.dumps(payload),
        text=True,
        capture_output=True,
        check=False,
        cwd=str(cwd),
    )
    if proc.returncode != 0:
        fail(f"hook exit {proc.returncode}: {proc.stderr}")
    line = (proc.stdout or "").strip().splitlines()[-1]
    return json.loads(line)


def write_json(path: Path, obj) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(obj, indent=2) + "\n", encoding="utf-8")


def main() -> None:
    if not FIXTURE.is_file():
        fail(f"missing {FIXTURE}")
    tmp = Path(tempfile.mkdtemp(prefix="omm-stop-"))
    try:
        # no ralph → allow
        out = run_hook(tmp)
        if out != {}:
            fail(f"empty state should allow, got {out}")

        # ralph active under max → block (existing behavior)
        write_json(tmp / ".omm" / "ralph.json", {"active": True, "iterations": 0, "max": 3, "goal": "ship"})
        out = run_hook(tmp)
        if out.get("decision") != "block":
            fail(f"ralph should block, got {out}")
        if "Ralph loop 1/3" not in str(out.get("reason")):
            fail(f"ralph reason {out.get('reason')!r}")
        ralph = json.loads((tmp / ".omm" / "ralph.json").read_text(encoding="utf-8"))
        if ralph.get("iterations") != 1:
            fail(f"ralph iterations {ralph.get('iterations')}")

        # DONE promise allows
        tmp2 = tmp / "done"
        tmp2.mkdir()
        write_json(tmp2 / ".omm" / "ralph.json", {"active": True, "iterations": 1, "max": 5})
        out = run_hook(tmp2, {"last_assistant_message": "all good <promise>DONE</promise>"})
        if out != {}:
            fail(f"DONE should allow, got {out}")
        ralph = json.loads((tmp2 / ".omm" / "ralph.json").read_text(encoding="utf-8"))
        if ralph.get("active") is not False:
            fail("ralph should deactivate on DONE")

        # ulw after inactive ralph
        tmp3 = tmp / "ulw"
        tmp3.mkdir()
        write_json(tmp3 / ".omm" / "ulw.json", {"active": True, "iterations": 0, "max": 4, "goal": "deep"})
        out = run_hook(tmp3)
        if out.get("decision") != "block" or "Ultrawork" not in str(out.get("reason")):
            fail(f"ulw should block, got {out}")

        # ultrawork.json alias
        tmp3b = tmp / "ultrawork"
        tmp3b.mkdir()
        write_json(tmp3b / ".omm" / "ultrawork.json", {"active": True, "iterations": 0, "max": 2})
        out = run_hook(tmp3b)
        if out.get("decision") != "block":
            fail(f"ultrawork.json should block, got {out}")

        # boulder
        tmp4 = tmp / "boulder"
        tmp4.mkdir()
        write_json(tmp4 / ".omm" / "boulder.json", {"active": True, "iterations": 0, "max": 6})
        out = run_hook(tmp4)
        if out.get("decision") != "block" or "Boulder" not in str(out.get("reason")):
            fail(f"boulder should block, got {out}")

        # todo nudge once then cap
        tmp5 = tmp / "todo"
        tmp5.mkdir()
        write_json(
            tmp5 / ".omm" / "todo.json",
            {"items": [{"id": "a", "text": "land", "status": "pending"}], "nudge": 0, "nudge_cap": 1},
        )
        out = run_hook(tmp5)
        if out.get("decision") != "block" or "Todo" not in str(out.get("reason")):
            fail(f"todo should block once, got {out}")
        todo = json.loads((tmp5 / ".omm" / "todo.json").read_text(encoding="utf-8"))
        if todo.get("nudge") != 1:
            fail(f"todo nudge {todo.get('nudge')}")
        out = run_hook(tmp5)
        if out != {}:
            fail(f"todo cap should allow, got {out}")

        # ralph still takes precedence over todo
        tmp6 = tmp / "both"
        tmp6.mkdir()
        write_json(tmp6 / ".omm" / "ralph.json", {"active": True, "iterations": 0, "max": 2})
        write_json(tmp6 / ".omm" / "todo.json", {"items": [{"id": "a", "status": "pending"}]})
        out = run_hook(tmp6)
        if "Ralph" not in str(out.get("reason")):
            fail(f"ralph should win over todo, got {out}")

        print("ok  stop-chain ralph/ulw/boulder/todo")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
