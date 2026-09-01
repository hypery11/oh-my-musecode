---
description: Stand up a multi-skill Muse team for a mission
argument-hint: [mission]
---

# /team

Coordinate a small Muse team for: **$ARGUMENTS**

## Steps
1. Read skills `architect`, `planner`, and `executor`.
2. Write `.omm/team/mission.md` with the mission text and success criteria.
3. Propose roles (3–5) mapped to skill ids. Persist `.omm/team/roster.json`.
4. For each parallelizable research/implement slice, call `subagent_spawn` with a crisp prompt and the matching skill context. Prefer worktrees under `.muse/worktrees/` for isolated edits.
5. Append events to `.omm/team/log.jsonl` as work progresses.
6. When finished, run a `/verify`-style evidence pass and summarize.

## Notes
CLI `omm team [mission...]` is live (writes `.omm/team/mission.md` + `roster.json`; no args lists roster + log). It is file-based, not a tmux dashboard. This slash-command still coordinates in-session `subagent_spawn`. Do not call foreign agent CLIs.
