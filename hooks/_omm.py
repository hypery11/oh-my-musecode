"""Shared helpers for Oh My Muse Code hooks.

Imported by hook scripts. This file is not a hook command source, so it does
not count toward duplicate-hook-source uniqueness.
"""
from __future__ import annotations

import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

HOOK_ID = ""  # set by each script after import if desired


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def read_stdin_json() -> dict[str, Any]:
    raw = sys.stdin.read()
    if not raw or not raw.strip():
        return {}
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return {"_raw": raw}
    return data if isinstance(data, dict) else {"_value": data}


def unwrap_event(event: dict[str, Any]) -> dict[str, Any]:
    """Accept live flat hook stdin or wrapper {event, stdin:{...}}.

    Muse hook-test fixtures nest fields under stdin; live hooks send the inner object at the top level. Prefer stdin for cwd and prompt when present.
    """
    if not isinstance(event, dict):
        return {}
    inner = event.get("stdin")
    if not isinstance(inner, dict):
        return event
    merged = {k: v for k, v in event.items() if k != "stdin"}
    merged.update(inner)
    return merged


def cwd_from(event: dict[str, Any]) -> Path:
    event = unwrap_event(event)
    for key in (
        "cwd",
        "Cwd",
        "working_directory",
        "workingDirectory",
        "workspace_root",
        "workspaceRoot",
        "workspace_path",
        "workspacePath",
        "workspace",
        "project_dir",
        "projectDir",
    ):
        val = event.get(key)
        if isinstance(val, str) and val.strip():
            return Path(val)
    payload = event.get("payload")
    if isinstance(payload, dict):
        record = payload.get("record")
        if isinstance(record, dict):
            nested = record.get("workspace_root")
            if isinstance(nested, str) and nested.strip():
                return Path(nested)
    omm_override = os.environ.get("OMM_DIR")
    if isinstance(omm_override, str) and omm_override.strip():
        return Path(omm_override).expanduser().resolve().parent
    for env_key in ("MUSE_WORKSPACE", "MUSE_CWD"):
        val = os.environ.get(env_key)
        if isinstance(val, str) and val.strip():
            return Path(val)
    return Path(os.getcwd())


def omm_dir(event: dict[str, Any]) -> Path:
    return cwd_from(event) / ".omm"


def emit(obj: dict[str, Any] | None = None) -> None:
    sys.stdout.write(json.dumps(obj if obj is not None else {}, ensure_ascii=False))
    sys.stdout.write("\n")
    sys.stdout.flush()


def audit(event: dict[str, Any], hook_id: str, extra: dict[str, Any] | None = None) -> None:
    """Append one JSONL line to .omm/hooks.jsonl when the directory is writable."""
    try:
        cwd = cwd_from(event)
        directory = cwd / ".omm"
        directory.mkdir(parents=True, exist_ok=True)
        rec = {
            "ts": utc_now(),
            "hook": hook_id,
            "event": event.get("hook_event_name") or event.get("event") or "",
            "session_id": event.get("session_id") or event.get("sessionId") or "",
            "tool_name": event.get("tool_name") or event.get("toolName") or "",
            "cwd": str(cwd),
        }
        if extra:
            rec.update(extra)
            rec["cwd"] = str(cwd)
        path = directory / "hooks.jsonl"
        with path.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
    except OSError:
        return


def load_json(path: Path) -> Any:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None


def write_json(path: Path, obj: Any) -> bool:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(obj, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        return True
    except OSError:
        return False


def append_jsonl(path: Path, obj: Any) -> bool:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open("a", encoding="utf-8") as fh:
            fh.write(json.dumps(obj, ensure_ascii=False) + "\n")
        return True
    except OSError:
        return False
