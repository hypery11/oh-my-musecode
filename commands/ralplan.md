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

## Notes
CLI `omm ralplan [topic...]` is live for state files (`.omm/mode.json` `{mode:ralplan,topic}`, `plan.md` stub if missing, `ralph.json` `{active:false, goal, iterations:0, max:20}`). In-session Muse still follows this markdown for questions and planning prose.
