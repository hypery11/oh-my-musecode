---
description: Execute the next plan step or a concrete task
argument-hint: [step-or-task]
---

# /execute

Execute: **$ARGUMENTS**

## Steps
1. If `.omm/plan.json` exists, select the named or next pending step; otherwise treat `$ARGUMENTS` as the task.
2. Read skill `executor` (and `explore` if paths are unknown).
3. Implement with minimal diff; record progress in `.omm/progress.md`.
4. Run the step’s verification; on failure invoke `debugger` guidance.
5. Update plan statuses. Stop after one step unless the user asked for more.

Muse-native tools only. State in `.omm/`.
