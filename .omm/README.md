# `.omm/` — Oh My Muse Code session state

Runtime state for a Muse workspace. Keep this directory **local**; only `.gitkeep` and this README are meant to be committed.

## Layout

| Path | Purpose |
|------|---------|
| `hooks.jsonl` | One-line JSON audit from plugin hooks |
| `mode.json` | Last detected mode (`ralph`, `ralplan`, `ultrathink`, `autopilot`, `ultragoal`) |
| `plan.md` / `plan.json` | Active plan and machine-readable steps |
| `progress.md` | Executor notes |
| `ralph.json` | `{active, goal, iterations, max}` for the Ralph loop |
| `ralph-report.md` | Loop summary when finished |
| `skill-gate.json` | `{enabled, required:[]}` — mutating tools gated until skills are marked read |
| `read-skills.json` | Skills already consumed this session |
| `verify.json` / `verify.md` | Evidence-gated completion |
| `ultragoal.md` | North-star goal + milestones |
| `handoff.md` | Next-session brief |
| `memory.md` / `memory.jsonl` | `/remember` notes |
| `architecture.md` | Durable design decisions |
| `requirements.md` | Interview brief |
| `hud.md` | Last HUD snapshot |
| `doctor-report.md` | `/omm-doctor` output |
| `setup-notes.md` | Install commands recommended in-session |
| `blockers.md` | Executor stop reasons |
| `ask-log.md` | Optional Q/A log |
| `git-notes.md` | Git-master notes |
| `testing.md` | How tests were run |
| `team/mission.md` | Current team mission |
| `team/roster.json` | Role → skill mapping |
| `team/log.jsonl` | Subagent start/stop events |
| `explore/` `analysis/` `design/` `debug/` `trace/` `critique/` `reviews/` `security/` `science/` `docs/` `writing/` `interview/` `qa/` `wiki/` `skillify/` | Role-specific notes |

| `ulw.json` / `ultrawork.json` | Ultrawork Stop loop `{active, goal, iterations, max}` |
| `boulder.json` | Boulder Stop loop until `<promise>DONE</promise>` or max |
| `todo.json` | `{items, nudge, nudge_cap}` — Stop nudges once while items are open |
| `compact.json` | Last PreCompact marker `{ts, note}` |
| `notify.json` | Optional `{url}` http(s) webhook for SessionEnd |
| `intent-gate.json` | `{required: ["plan"]}` — mutating tools denied until `plan.json` exists |
| `ask/last.json` | Last `omm ask` skill pick |
| `mission/queue.json` | `{id, text, status}` items |
| `wiki/*.md` | File wiki pages |

Hooks write here when the workspace is writable. If a write fails, hooks still print `{}` (or a deny decision) and continue.

## Skill gate

To require skills before mutating tools (`Write`, `Edit`, `StrReplace`, write-ish `Bash`):

```json
{"enabled": true, "required": ["planner"]}
```

Mark a skill as read:

```json
["planner"]
```

or `{"skills": ["planner"]}`.

| `autopilot.json` | `{active, step, max, goal}` — Stop loop after ralph/ulw/boulder/todo |
| `progress.md` | Executor notes from `omm execute` |
| `ask/last.json` | `{query, skill, score, reason, alternatives}` |

