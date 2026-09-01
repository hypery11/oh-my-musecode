#!/usr/bin/env python3
"""Static checks for .muse-plugin/plugin.json (no Muse binary required)."""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / ".muse-plugin" / "plugin.json"
ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,79}$")
REQUIRED = ("schemaVersion", "name", "version", "description", "compat", "capabilities")
FAMILIES = ("skills", "commands", "hooks", "mcpServers", "reminders")
FORBIDDEN = ("tools", "agents", "outputStyles", "settings", "apps")


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def main() -> None:
    if not MANIFEST.is_file():
        fail(f"missing {MANIFEST}")
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    for key in REQUIRED:
        if key not in data:
            fail(f"missing {key}")
    if data.get("schemaVersion") != 1:
        fail("schemaVersion must be number 1")
    if not ID_RE.match(str(data.get("name") or "")):
        fail("invalid plugin name")
    compat = data.get("compat") or {}
    if compat.get("manifestDir") != ".muse-plugin":
        fail("compat.manifestDir must be .muse-plugin")
    caps = data.get("capabilities") or {}
    for fam in FORBIDDEN:
        if fam in caps and caps[fam] not in (None, [], {}):
            fail(f"forbidden capability family {fam}")
    ids: dict[str, str] = {}
    hook_sources: dict[str, str] = {}
    for fam in ("skills", "commands"):
        items = caps.get(fam) or []
        if not isinstance(items, list):
            fail(f"{fam} must be an array")
        for item in items:
            i = item.get("id")
            path = item.get("path")
            if not i or not ID_RE.match(i):
                fail(f"bad {fam} id {i!r}")
            if i in ids:
                fail(f"id collision {i} ({ids[i]} vs {fam})")
            ids[i] = fam
            target = ROOT / path
            if not target.is_file():
                fail(f"{fam} {i} path missing: {path}")
    for hook in caps.get("hooks") or []:
        hid = hook.get("id")
        cmd = hook.get("command")
        if not hid or not ID_RE.match(hid):
            fail(f"bad hook id {hid!r}")
        if not isinstance(cmd, list) or len(cmd) < 2:
            fail(f"hook {hid} command must be argv")
        src = cmd[-1]
        if src in hook_sources:
            fail(f"duplicate-hook-source {src} ({hook_sources[src]} vs {hid})")
        hook_sources[src] = hid
        if not (ROOT / src).is_file():
            fail(f"hook {hid} source missing: {src}")
        for bad in ("matcher", "cwd", "env", "timeout_ms", "shell_command"):
            if bad in hook:
                fail(f"hook {hid} has forbidden field {bad}")
    print("ok  manifest")
    print(f"ok  {sum(1 for v in ids.values() if v=='skills')} skills")
    print(f"ok  {sum(1 for v in ids.values() if v=='commands')} commands")
    print(f"ok  {len(hook_sources)} unique hook sources")


if __name__ == "__main__":
    main()
