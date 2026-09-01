---
description: Write a session handoff for the next Muse chat
argument-hint: [focus]
---

# /handoff

Handoff focus: **$ARGUMENTS**

## Steps
1. Read `writer` and `document-specialist`.
2. Summarize goal, done, in-progress, blockers, and key paths.
3. List relevant `.omm/` files (`plan.json`, `mode.json`, `verify.json`, team logs).
4. Write `.omm/handoff.md` ready to paste into a new session.
5. Suggest the first command the next session should run.

Do not include secrets or raw credentials.

## Notes
CLI `omm handoff [focus...]` is live: writes `.omm/handoff.md` from existing mode/plan/verify/team files (no secrets) and prints the path. In-session Muse still follows this markdown for the narrative brief.
