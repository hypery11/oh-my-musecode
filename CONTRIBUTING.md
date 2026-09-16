# Contributing to Oh My Muse Code

Thanks for helping. This is a native Muse plugin plus a single dependency-free Rust binary (`omm`).

## Ground rules

- Write original prose. Do not paste oh-my-claudecode / oh-my-codex / oh-my-openagent / oh-my-grok skill text.
- Stay inside Muse 1.3.0 constraints (see AGENTS.md).
- Skill path in the manifest is a file (skills/<id>/SKILL.md).
- Hook command is an argv array of the form `["omm", "hook", "<name>"]`. Hook behavior lives in the `omm` binary, not in this repo.
- Command ids and skill ids must not collide with each other.
- Do not add capabilities.tools, agents, outputStyles, settings, or apps.

## Validate

Run `muse plugins validate dist/claude` on the built projection (run `omm build` first). Experimental plugins must be on. Expect valid true. Root-key unsupported-field warnings are ok; capability errors are not.

## CLI

The companion binary has no dependencies. Keep it that way. Documented `omm` verbs are `doctor`, `cost`, `status`, `mcp` (plus `hook` for plugin dispatch). Workflows live in skills; slash-commands stay at three.

## Docs

English is primary. Keep the Traditional Chinese README in sync. Never claim a tmux dashboard, a statusline, output styles, or an ask transport unless the host ships the surface.

Please follow the [Code of Conduct](.github/CODE_OF_CONDUCT.md).

## License

MIT, copyright 2026 hypery11.
