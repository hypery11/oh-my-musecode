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

## Notes
CLI `omm debug [symptom...]` is live for state files (`.omm/debug/<stamp>.md` with symptom + empty Hypotheses section, `mode.json` debug). In-session Muse still follows this markdown for hypotheses and probes.
