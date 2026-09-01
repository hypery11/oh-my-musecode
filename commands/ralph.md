---
description: Work-until-done loop with a Stop-hook iteration budget
argument-hint: "[goal] [--max N]"
---

# /ralph

Ralph loop for: **$ARGUMENTS**

The `stop-chain` hook **blocks** session end (`{"decision":"block"}`) while `.omm/ralph.json` is active and `iterations < max`. That is the live engine. This command only arms the state file.

## Steps
1. Parse the goal and optional `--max N` (default 10).
2. Write `.omm/ralph.json`:
   `{"active":true,"goal":"...","iterations":0,"max":N}`
3. Write `.omm/mode.json` with `"mode":"ralph"`.
4. Work the goal: planner → executor → verifier.
5. When the goal is actually met, print `<promise>DONE</promise>` in the assistant turn so the next Stop allows exit.
6. If you stop early without DONE, Stop is blocked and you get another turn (iteration increments). Budget exhaustion or an abort/cancel reason allows exit.
7. On finish, set `active` false and write `.omm/ralph-report.md`.

Do not call Muse `/loop` (different builtin). Do not call foreign CLIs.
