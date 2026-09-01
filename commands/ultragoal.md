---
description: Define a north-star goal with measurable milestones
argument-hint: [goal]
---

# /ultragoal

Ultra goal: **$ARGUMENTS**

## Steps
1. Set `.omm/mode.json` to `{"mode":"ultragoal","goal":"$ARGUMENTS"}`.
2. Read `architect`, `planner`, and `critic`.
3. Write `.omm/ultragoal.md` with vision, milestones, metrics, and anti-goals.
4. Create an initial `.omm/plan.json` covering milestone 1 only.
5. Recommend `/autopilot` or `/ralph` for execution.

Keep state in `.omm/`. Use Muse `subagent_spawn` for divergent option sketches.

## Notes
CLI `omm ultragoal [goal...]` is live for state files (`.omm/mode.json` ultragoal, `ultragoal.md`, `plan.json` milestone-1 pending steps). In-session Muse still follows this markdown for vision/metrics prose.
