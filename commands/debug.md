---
description: Start a structured debugging session
argument-hint: [symptom]
---

# /debug

Debug symptom: **$ARGUMENTS**

## Steps
1. Read skills `debugger` and `tracer`.
2. Capture current failure evidence into `.omm/debug/<stamp>.md`.
3. Form hypotheses; test the cheapest ones first.
4. If useful, `subagent_spawn` parallel probes (one hypothesis each) using worktrees under `.muse/worktrees/` when edits are risky.
5. When resolved, note root cause and a regression check; suggest `/verify`.

Muse-native tools only.
