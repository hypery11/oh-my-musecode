# Changelog

## Unreleased

## 0.3.0 — 2026-09-01 (Taipei)

### Added
- File-based CLI engines: `omm ralplan`, `interview`/`deep-interview`, `ultragoal`, `handoff`, `skillify`, `verify`, `autopilot`, `execute`, `remember`, `debug`, `trace`
- `omm ask` scores bundled SKILL.md YAML descriptions (19-role router, no remote model)
- Stop chain honors `.omm/autopilot.json` after ralph/ulw/boulder/todo

### Notes
- Slash-commands remain in-session interviewers; CLI writes `.omm/` state files
- Still not 1:1 with peer runtimes (no tmux, no remote ask)

## 0.2.0 — 2026-09-01 (Taipei)

### Added
- File-based companion CLI: `omm team`, `ask`, `wait`, `mission`, `wiki`, `update` (no tmux, no remote model)
- Stop chain also honors `.omm/ulw.json` / `ultrawork.json`, `.omm/boulder.json`, and a capped `.omm/todo.json` nudge
- PreCompact writes `.omm/compact.json` and appends `.omm/memory.md`
- SessionEnd optional http(s) webhook from `.omm/notify.json`
- Fail-open intent-gate on PreToolUse when `.omm/intent-gate.json` requires `plan`

### Fixed
- Hook `cwd_from` prefers `workspace_root` / `OMM_DIR` / `MUSE_WORKSPACE` over lying `PWD`

### Notes
- Still not 1:1 OMC. Templates remain for interview/verify/ralplan/etc. CLI+hooks now have real file-based state machines.


## 0.1.1 — 2026-09-01 (Taipei)

### Changed
- `omm hud` is a real text snapshot of `.omm/` (not a live TUI); keyword hook accepts wrapper `{event, stdin}` fixtures

### Notes
- npm package oh-my-musecode 0.1.1 and GitHub release v0.1.1

## 0.1.0 — 2026-09-01

### Added
- Ralph Stop loop: `stop-chain` emits `{"decision":"block"}` while `.omm/ralph.json` is active (confirmed via `muse plugins hook test`)
- README demo GIF (`docs/assets/omm-demo.gif`)
- Native Muse plugin `oh-my-musecode`: 19 role skills, 19 slash-commands, 8 hooks
- Companion CLI `omm`: `setup` and `doctor` are real; `team|ask|hud|wait|mission|wiki|update` are stubs
- `.omm/` workspace state conventions
- GitHub community files (issue/PR templates, CI, CoC, security)

### Notes
- Validated with Muse 1.0.1-R2006.1 `plugins validate` (valid true)
- Not a 1:1 port of every OMC 5.1 / OMX skill
