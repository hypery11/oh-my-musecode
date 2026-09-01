#!/usr/bin/env python3
"""PreToolUse skill gate — deny mutating tools until required skills are marked read.

Expects optional `.omm/skill-gate.json`:
  {"enabled": true, "required": ["planner", "executor"]}

And `.omm/read-skills.json` as a list or {"skills": [...]} / {"read": [...]}.
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import _omm as omm  # noqa: E402

HOOK_ID = "skill-gate"

MUTATING_TOOLS = {"write", "edit", "strreplace", "str_replace"}
BASHISH = {"bash", "shell", "run"}
WRITEISH = re.compile(
    r"(?:^|\s)(?:tee|rm|mv|cp|mkdir|touch|chmod|chown|dd|install|sed\s+-i|awk\s+-i)\b"
    r"|(?:^|\s)(?:python3?|node|ruby|perl)\s+\S+\s+.*>"
    r"|>>|>\s|tee\s",
    re.IGNORECASE,
)


def tool_name(event: dict) -> str:
    val = event.get("tool_name") or event.get("toolName") or ""
    return str(val)


def tool_input(event: dict) -> dict:
    val = event.get("tool_input") or event.get("toolInput") or {}
    return val if isinstance(val, dict) else {}


def looks_mutating(name: str, inp: dict) -> bool:
    n = name.lower().replace("-", "").replace("_", "")
    if n in {"write", "edit", "strreplace"} or name in ("Write", "Edit", "StrReplace"):
        return True
    if name.lower() in MUTATING_TOOLS:
        return True
    if name.lower() in BASHISH or n in {"bash", "shell"}:
        cmd = inp.get("command") or inp.get("cmd") or ""
        if isinstance(cmd, list):
            cmd = " ".join(str(x) for x in cmd)
        if isinstance(cmd, str) and WRITEISH.search(cmd):
            return True
    return False


def required_skills(gate: dict) -> list[str]:
    raw = gate.get("required") or gate.get("requiredSkills") or gate.get("skills") or []
    if isinstance(raw, str):
        return [raw]
    if isinstance(raw, list):
        return [str(x) for x in raw if x]
    return []


def read_set(payload) -> set[str]:
    if payload is None:
        return set()
    if isinstance(payload, list):
        return {str(x) for x in payload}
    if isinstance(payload, dict):
        for key in ("skills", "read", "ids"):
            val = payload.get(key)
            if isinstance(val, list):
                return {str(x) for x in val}
        return {str(k) for k, v in payload.items() if v}
    return set()


def deny(reason: str) -> dict:
    return {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }


def main() -> None:
    event = omm.read_stdin_json()
    directory = omm.omm_dir(event)
    gate_path = directory / "skill-gate.json"
    gate = omm.load_json(gate_path)
    name = tool_name(event)
    inp = tool_input(event)
    mutating = looks_mutating(name, inp)
    omm.audit(event, HOOK_ID, {"mutating": mutating})

    if isinstance(gate, dict) and gate.get("enabled") and mutating:
        needed = required_skills(gate)
        if needed:
            already = read_set(omm.load_json(directory / "read-skills.json"))
            missing = [s for s in needed if s not in already]
            if missing:
                reason = (
                    "Oh My Muse Code skill gate: mutating tool "
                    f"{name or '(unknown)'} blocked until these skills are marked read "
                    f"in .omm/read-skills.json: {', '.join(missing)}"
                )
                omm.emit(deny(reason))
                return

    intent = omm.load_json(directory / "intent-gate.json")
    if isinstance(intent, dict) and mutating:
        raw = intent.get("required") or intent.get("requiredSkills") or []
        if isinstance(raw, str):
            raw = [raw]
        needed_i = [str(x) for x in raw] if isinstance(raw, list) else []
        if "plan" in needed_i or "plan.json" in needed_i:
            if not (directory / "plan.json").is_file():
                reason = (
                    "Oh My Muse Code intent-gate: mutating tool "
                    f"{name or '(unknown)'} blocked until .omm/plan.json exists"
                )
                omm.emit(deny(reason))
                return

    if not mutating:
        omm.emit({})
        return
    if not isinstance(gate, dict) or not gate.get("enabled"):
        omm.emit({})
        return
    # Do not emit permissionDecision=allow — empty object lets Muse proceed.
    omm.emit({})


if __name__ == "__main__":
    main()
