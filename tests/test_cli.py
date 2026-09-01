#!/usr/bin/env python3
"""Offline tests for file-based omm CLI verbs (OMM_DIR / cwd)."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "bin" / "omm.mjs"


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def run_omm(args, env, cwd) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["node", str(CLI), *args],
        text=True,
        capture_output=True,
        check=False,
        cwd=str(cwd),
        env=env,
    )


def main() -> None:
    tmp = Path(tempfile.mkdtemp(prefix="omm-cli-"))
    omm = tmp / "state"
    decoy = tmp / "decoy"
    decoy.mkdir()
    env = {**os.environ, "OMM_DIR": str(omm)}
    try:
        r = run_omm(["team", "ship", "wiki", "cli"], env, decoy)
        if r.returncode != 0:
            fail(f"team init exit {r.returncode}: {r.stderr}")
        if not (omm / "team" / "mission.md").is_file():
            fail("team did not write mission.md under OMM_DIR")
        roster = json.loads((omm / "team" / "roster.json").read_text(encoding="utf-8"))
        if "architect" not in json.dumps(roster):
            fail("roster missing roles")
        r = run_omm(["team"], env, decoy)
        if r.returncode != 0 or "roster:" not in r.stdout:
            fail(f"team list: {r.stdout!r}")

        r = run_omm(["ask", "git", "rebase", "conflict"], env, decoy)
        if r.returncode != 0:
            fail(f"ask exit {r.returncode}: {r.stderr}")
        if "git-master" not in r.stdout:
            fail(f"ask expected git-master, got {r.stdout!r}")
        last = json.loads((omm / "ask" / "last.json").read_text(encoding="utf-8"))
        if last.get("skill") != "git-master":
            fail(f"ask last.json skill={last.get('skill')}")

        r = run_omm(["mission", "land", "phase-2"], env, decoy)
        if r.returncode != 0 or "queued 1" not in r.stdout:
            fail(f"mission append: {r.stdout!r}")
        r = run_omm(["mission"], env, decoy)
        if "pending" not in r.stdout:
            fail(f"mission list: {r.stdout!r}")
        r = run_omm(["mission", "done", "1"], env, decoy)
        if r.returncode != 0:
            fail("mission done failed")
        queue = json.loads((omm / "mission" / "queue.json").read_text(encoding="utf-8"))
        if queue[0].get("status") != "done":
            fail("mission not marked done")

        r = run_omm(["wiki", "write", "home", "hello wiki"], env, decoy)
        if r.returncode != 0:
            fail(f"wiki write: {r.stderr}")
        r = run_omm(["wiki", "list"], env, decoy)
        if "home" not in r.stdout:
            fail(f"wiki list: {r.stdout!r}")
        r = run_omm(["wiki", "show", "home"], env, decoy)
        if "hello wiki" not in r.stdout:
            fail(f"wiki show: {r.stdout!r}")

        r = run_omm(["wait", "0"], env, decoy)
        if r.returncode != 1 or "timeout" not in r.stdout:
            fail(f"wait empty: code={r.returncode} out={r.stdout!r}")
        log = omm / "team" / "log.jsonl"
        log.parent.mkdir(parents=True, exist_ok=True)
        log.write_text('{"event":"subagent-stop","id":"a"}\n', encoding="utf-8")
        r = run_omm(["wait", "0"], env, decoy)
        if r.returncode != 0 or "subagent-stop" not in r.stdout:
            fail(f"wait existing log: {r.stdout!r}")

        r = run_omm(["update"], env, decoy)
        if r.returncode != 0:
            fail(f"update exit {r.returncode}")
        if "muse plugins update" not in r.stdout or "muse plugins approve" not in r.stdout:
            fail(f"update help: {r.stdout!r}")

        r = run_omm(["not-a-command"], env, decoy)
        if r.returncode != 1:
            fail("unknown command should exit 1")

        help_out = run_omm(["--help"], env, decoy).stdout
        for verb in ("team", "ask", "wait", "mission", "wiki", "update", "setup", "doctor", "hud"):
            if verb not in help_out:
                fail(f"HELP missing {verb}")

        print("ok  cli team/ask/wait/mission/wiki/update against OMM_DIR")
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    main()
