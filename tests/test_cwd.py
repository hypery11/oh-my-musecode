#!/usr/bin/env python3
"""Offline test: cwd_from prefers workspace_root over PWD/getcwd; cwd still wins."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HOOKS = ROOT / "hooks"
sys.path.insert(0, str(HOOKS))
import _omm as omm  # noqa: E402

HOOK = ROOT / "hooks" / "user_prompt.py"


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    decoy = Path(tempfile.mkdtemp(prefix="omm-cwd-decoy-"))
    workspace = Path(tempfile.mkdtemp(prefix="omm-cwd-ws-"))
    other = Path(tempfile.mkdtemp(prefix="omm-cwd-other-"))
    saved_cwd = os.getcwd()
    saved_pwd = os.environ.get("PWD")
    saved_omm = os.environ.get("OMM_DIR")
    saved_mw = os.environ.get("MUSE_WORKSPACE")
    saved_mc = os.environ.get("MUSE_CWD")
    try:
        os.chdir(decoy)
        os.environ["PWD"] = str(decoy)
        os.environ.pop("OMM_DIR", None)
        os.environ.pop("MUSE_WORKSPACE", None)
        os.environ.pop("MUSE_CWD", None)

        nested = {
            "hook_event_name": "UserPromptSubmit",
            "payload": {"record": {"workspace_root": str(workspace)}},
            "prompt": "hello",
        }
        got = omm.cwd_from(nested)
        if got != Path(str(workspace)):
            fail(f"workspace_root nested: {got} != {workspace}")
        odir = omm.omm_dir(nested)
        if odir != workspace / ".omm":
            fail(f"omm_dir nested: {odir}")
        if odir == decoy / ".omm":
            fail("omm_dir used decoy/PWD")

        cwd_wins = {
            "cwd": str(other),
            "workspace_root": str(workspace),
            "payload": {"record": {"workspace_root": str(workspace)}},
        }
        got = omm.cwd_from(cwd_wins)
        if got != Path(str(other)):
            fail(f"cwd should win: {got} != {other}")

        top_root = {"workspace_root": str(workspace)}
        got = omm.cwd_from(top_root)
        if got != Path(str(workspace)):
            fail(f"top workspace_root: {got}")

        # Last resort is getcwd, not PWD env. Point PWD at decoy while getcwd is other.
        os.chdir(other)
        os.environ["PWD"] = str(decoy)
        last = omm.cwd_from({})
        if last != Path(os.getcwd()):
            fail(f"last resort should be getcwd {os.getcwd()} not {last}")
        if last == Path(str(decoy)):
            fail("must not use PWD env as last resort")

        # Live-style: run prompt hook with nested workspace_root, PWD elsewhere
        os.chdir(decoy)
        os.environ["PWD"] = str(decoy)
        payload = {
            "hook_event_name": "UserPromptSubmit",
            "payload": {"record": {"workspace_root": str(workspace)}},
            "prompt": "ralph continue",
        }
        proc = subprocess.run(
            [sys.executable, str(HOOK)],
            input=json.dumps(payload),
            text=True,
            capture_output=True,
            check=False,
            cwd=str(decoy),
            env={**os.environ, "PWD": str(decoy)},
        )
        if proc.returncode != 0:
            fail(f"hook exit {proc.returncode}: {proc.stderr}")
        mode = workspace / ".omm" / "mode.json"
        if not mode.is_file():
            fail(f"hook wrote elsewhere; missing {mode} (decoy has {(decoy / '.omm').exists()})")
        audit_path = workspace / ".omm" / "hooks.jsonl"
        if not audit_path.is_file():
            fail("missing hooks.jsonl under workspace_root")
        last_line = audit_path.read_text(encoding="utf-8").strip().splitlines()[-1]
        rec = json.loads(last_line)
        if rec.get("cwd") != str(workspace):
            fail(f"audit cwd={rec.get('cwd')!r} expected {workspace}")
        if (decoy / ".omm" / "mode.json").is_file():
            fail("hook wrote mode.json under PWD decoy")
        print("ok  cwd_from prefers workspace_root; cwd key still wins; audit has cwd")
    finally:
        os.chdir(saved_cwd)
        if saved_pwd is None:
            os.environ.pop("PWD", None)
        else:
            os.environ["PWD"] = saved_pwd
        for key, val in (
            ("OMM_DIR", saved_omm),
            ("MUSE_WORKSPACE", saved_mw),
            ("MUSE_CWD", saved_mc),
        ):
            if val is None:
                os.environ.pop(key, None)
            else:
                os.environ[key] = val
        shutil.rmtree(decoy, ignore_errors=True)
        shutil.rmtree(workspace, ignore_errors=True)
        shutil.rmtree(other, ignore_errors=True)


if __name__ == "__main__":
    main()
