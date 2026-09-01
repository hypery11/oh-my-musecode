# Contributing to Oh My Muse Code

Thanks for helping. This is a native Muse plugin plus a tiny Node CLI.

## Ground rules

- Write original prose. Do not paste oh-my-claudecode / oh-my-codex / oh-my-openagent / oh-my-grok skill text.
- Stay inside Muse 1.0.1-R2006.1 constraints (see AGENTS.md).
- Skill path in the manifest is a file (skills/<id>/SKILL.md).
- Hook command is an argv array. Each hook id must point at a unique source file.
- Command ids and skill ids must not collide with each other.
- Do not add capabilities.tools, agents, outputStyles, settings, or apps.

## Validate

Run Muse plugins validate on this directory. Experimental plugins must be on. Expect valid true. Root-key unsupported-field warnings are ok; capability errors are not.

## CLI

The companion binary has no package dependencies. Keep it that way. All documented `omm` verbs are file-based engines (not tmux, not a remote model). Slash-commands remain in-session interviewers.

## Docs

English is primary. Keep the Traditional Chinese README in sync. Never claim live tmux team HUD or ask transport unless the code exists.

Please follow the [Code of Conduct](.github/CODE_OF_CONDUCT.md).

## License

MIT, copyright 2026 hypery11.
