# Changelog

## Unreleased

### Changed
- `omm hud` is a real text snapshot of `.omm/` (not a live TUI); keyword hook accepts wrapper `{event, stdin}` fixtures

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
