# `.omm/` — Oh My Muse Code session state

Runtime state for a Muse workspace. Keep this directory **local**; only `.gitkeep` and this README are meant to be committed.

1.0.0 keeps state here in a new shape and does not read 0.3.0 files. Your old notes stay on disk — copy out `memory.md`, `handoff.md`, `plan.md`, or `architecture.md` if you want to keep them (see docs/MIGRATION.md).

## Layout

| Path | Purpose |
|------|---------|
| `compact.log.jsonl` | Pre-compact observations (trigger, transcript size) |
| `team/log.jsonl` | Subagent start/stop entries, with outcome when the host reports one |
| `intent-gate.json` / `plan.json` / `verify.json` | Gate state, only when you opt in |

Hook runs also land in the host's own session log as `hook_run_terminal` records. Run `/omm-doctor` to verify state health, `/omm-status` for a snapshot.
