---
description: Diagnose Muse binary + plugin tree health
---

# /omm-doctor

Doctor the OMM + Muse environment.

## Steps
1. Check for `muse` on PATH and common binary locations.
2. Confirm this plugin tree has `.muse-plugin/plugin.json`, skills, commands, hooks.
3. If a Muse binary is available, run plugins validate with experimental flag and summarize JSON.
4. Write `.omm/doctor-report.md` with pass/fail checks.
5. Suggest fixes for any failures (missing paths, hook argv, id collisions).

Prefer diagnosis over silent repair.
