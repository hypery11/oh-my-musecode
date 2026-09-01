---
description: Interview and planning pass tuned for Ralph
argument-hint: [topic]
---

# /ralplan

Ralph-oriented planning for: **$ARGUMENTS**

## Steps
1. Set `.omm/mode.json` to `{"mode":"ralplan","topic":"$ARGUMENTS"}`.
2. Read `planner` and `critic`.
3. Ask only the missing questions; then write a Ralph-ready `.omm/plan.md` with explicit verify commands.
4. Initialize `.omm/ralph.json` with `active:false`, goal, `iterations:0`, and a sensible `max`.
5. Tell the user to run `/ralph` when ready.

Persist everything under `.omm/`.
