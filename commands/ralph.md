---
description: Work-until-done loop with iteration budget
argument-hint: [goal] [--max N]
---

# /ralph

Ralph loop for: **$ARGUMENTS**

## Steps
1. Parse goal and optional max iterations (default 10).
2. Write `.omm/ralph.json` as `{"active":true,"goal":"...","iterations":0,"max":N}`.
3. Also set `.omm/mode.json` mode `ralph`.
4. Read `planner` → plan, then `executor` → implement, then `verifier`.
5. Each Stop while `active` and `iterations < max` should continue (hook `stop-chain` reinforces this). Increment `iterations` when a cycle completes.
6. Clear `active` when verified or budget exhausted; write `.omm/ralph-report.md`.

Use `subagent_spawn` for parallel checks. No foreign loops/CLIs.
