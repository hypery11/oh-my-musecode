#!/usr/bin/env python3
"""Static checks for plugins/oh-my-musecode/.muse-plugin/plugin.json (no Muse binary required).

Usage: scripts/check-manifest.py [--frontmatter-only]
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PUB = ROOT / "plugins" / "oh-my-musecode"
MANIFEST = PUB / ".muse-plugin" / "plugin.json"
MARKETPLACE = ROOT / ".claude-plugin" / "marketplace.json"
ID_RE = re.compile(r"^[a-z0-9][a-z0-9._-]{0,79}$")
REQUIRED = ("schemaVersion", "name", "version", "description", "compat", "capabilities")
FORBIDDEN = ("tools", "agents", "outputStyles", "settings", "apps")
HOOK_BAD_FIELDS = ("matcher", "cwd", "env", "timeout_ms", "shell_command", "status_message")


def fail(msg: str) -> None:
    print(f"FAIL  {msg}", file=sys.stderr)
    raise SystemExit(1)


def check_frontmatter() -> tuple[int, int]:
    """Every skill/command markdown carries YAML name + description. Returns counts."""
    data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    caps = data.get("capabilities") or {}
    n = 0
    for fam in ("skills", "commands"):
        for item in caps.get(fam) or []:
            text = (PUB / item["path"]).read_text(encoding="utf-8")
            if not text.startswith("---"):
                fail(f"{fam} {item['id']}: missing frontmatter")
            head = text.split("---", 2)[1]
            name = re.search(r"^name:\s*(\S+)", head, re.M)
            desc = re.search(r"^description:", head, re.M)
            if fam == "skills":
                if not name or not desc:
                    fail(f"skill {item['id']}: frontmatter needs name + description")
                if name.group(1) != item["id"]:
                    fail(f"skill {item['id']}: frontmatter name {name.group(1)!r} mismatch")
            elif not desc:
                fail(f"command {item['id']}: frontmatter needs description")
            n += 1
    return n, len(caps.get("reminders") or [])


def main() -> None:
    frontmatter_only = "--frontmatter-only" in sys.argv
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
    version = str(data.get("version"))
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'\[workspace\.package\][^[]*?^version\s*=\s*"([^"]+)"', cargo, re.M | re.S)
    if m and m.group(1) != version:
        fail(f"Cargo workspace {m.group(1)!r} != manifest {version!r}")
    if MARKETPLACE.is_file():
        mk = json.loads(MARKETPLACE.read_text(encoding="utf-8"))
        plugs = mk.get("plugins") or []
        if not plugs or plugs[0].get("version") != version:
            fail("marketplace version != manifest version")
        if plugs and plugs[0].get("name") != data.get("name"):
            fail("marketplace plugin name != manifest name")
    caps = data.get("capabilities") or {}
    for fam in FORBIDDEN:
        if fam in caps and caps[fam] not in (None, [], {}):
            fail(f"forbidden capability family {fam}")
    ids: dict[str, str] = {}
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
            if not (PUB / path).is_file():
                fail(f"{fam} {i} path missing: {path}")
    if not frontmatter_only:
        seen: dict[str, str] = {}
        for hook in caps.get("hooks") or []:
            hid = hook.get("id")
            cmd = hook.get("command")
            if not hid or not ID_RE.match(hid):
                fail(f"bad hook id {hid!r}")
            if not isinstance(cmd, list) or len(cmd) != 3 or cmd[:2] != ["omm", "hook"]:
                fail(f"hook {hid} command must be [omm, hook, <name>]")
            if cmd[2] in seen:
                fail(f"duplicate hook dispatch {cmd[2]} ({seen[cmd[2]]} vs {hid})")
            seen[cmd[2]] = hid
            for bad in HOOK_BAD_FIELDS:
                if bad in hook:
                    fail(f"hook {hid} has forbidden field {bad}")
        for rem in caps.get("reminders") or []:
            if not (PUB / rem.get("path", "")).is_file():
                fail(f"reminder {rem.get('id')} path missing")
    n_skills, n_rem = check_frontmatter()
    print("ok  manifest")
    print(f"ok  {n_skills} skills+commands with frontmatter")
    print(f"ok  {n_rem} reminders")


if __name__ == "__main__":
    main()
