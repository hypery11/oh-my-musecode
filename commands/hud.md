---
description: Render a text HUD of OMM session state
---

# /hud

Render a text heads-up display for the current OMM state.

## Steps
1. Read `.omm/mode.json`, `.omm/plan.json`, `.omm/ralph.json`, `.omm/verify.json` if present.
2. Summarize: mode, plan progress, ralph iterations, last verify, team mission.
3. Print a compact markdown HUD (tables ok).
4. Optionally write `.omm/hud.md` snapshot.

CLI `omm hud` prints a live text snapshot of `.omm/` (not a TUI). This slash-command still renders in-session.
