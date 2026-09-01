---
description: Plan then execute with light supervision
argument-hint: [goal]
---

# /autopilot

Autopilot goal: **$ARGUMENTS**

## Steps
1. Set `.omm/mode.json` to `{"mode":"autopilot","goal":"$ARGUMENTS"}`.
2. Load `planner` and produce `.omm/plan.md` + `.omm/plan.json`.
3. Load `executor` and walk steps, updating statuses.
4. After each step, briefly self-check; on failure switch to `debugger`.
5. End with `verifier` and write `.omm/verify.json`.

Keep durable state under `.omm/`. Use `subagent_spawn` sparingly for parallel research only.

## Notes
CLI `omm autopilot [goal...]` is live for state files (`.omm/mode.json` autopilot, `plan.json` if missing, `autopilot.json` `{active:true,step:0,max:20}`). Stop-chain honors autopilot **after** ralph/ulw/boulder/todo. In-session Muse still follows this markdown for walking steps and code.
