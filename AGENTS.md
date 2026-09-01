# AGENTS.md — working in Oh My Muse Code

This repository is a native Muse Code plugin (oh-my-musecode) plus a zero-dependency companion CLI (omm). Agents including Muse itself should follow this file.

## What this repo is

- Plugin id: oh-my-musecode (display name: Oh My Muse Code)
- Manifest: .muse-plugin/plugin.json
- Durable workspace state: .omm/ (gitignored except .gitkeep + README)
- Isolated checkouts: Muse worktrees under .muse/worktrees/
- Parallel helpers: Muse subagent_spawn (and status / wait / read_result)

It is not a fork of oh-my-openagent, oh-my-claudecode, oh-my-codex, or oh-my-grok. Those projects inspired the feature set; all text and hook code here is original and Muse-native. Do not call Claude Code or Codex APIs.

## Muse plugin rules (1.0.1-R2006.1)

- schemaVersion is number 1. compat.source is native. compat.manifestDir is .muse-plugin.
- capabilities keys only: skills, commands, hooks, mcpServers, reminders (all arrays).
- Do not declare tools, agents, outputStyles, settings, or apps.
- Do not use plugin ids loop or muse-core.
- Portable ids: lowercase letter or digit, then lowercase letters, digits, dot, underscore, hyphen; max 80.
- Skill path must be a file (skills/<id>/SKILL.md), not a directory.
- Command path must be an existing markdown file.
- Hook command is an argv array (not a string). Use timeoutMs (camelCase).
- Each hook id needs a unique source file (duplicate relative paths fail validate).
- No hook fields: matcher, cwd, env, timeout_ms, status_message, shell_command.
- Command ids and skill ids must be globally unique across families (verify command + verifier skill is fine).
- Skills need YAML frontmatter name + description.

After any manifest or path change, run Muse plugins validate with experimental plugins enabled. Do not install into the user Muse home unless they asked.

## How to work

1. Read the relevant skills/<id>/SKILL.md before playing that role.
2. Prefer slash-command templates in commands/*.md for user-facing workflows.
3. Persist decisions, plans, traces, and verify evidence under .omm/.
4. Use subagent_spawn with a bounded prompt; one owner per file cluster.
5. Hook scripts: read stdin JSON, audit .omm/hooks.jsonl if writable, print {} or a real decision. Never emit a bare permissionDecision=allow.
6. CLI verbs `setup` `doctor` `hud` `team` `ask` `wait` `mission` `wiki` `update` are live file-based engines. `omm hud` is a text snapshot, not a live TUI. Do not invent a tmux dashboard or remote ask transport.

## Layout

- .muse-plugin/plugin.json
- skills/<id>/SKILL.md (19 roles)
- commands/<id>.md (19 slash-commands)
- hooks/*.py (8 unique sources plus _omm.py helper)
- bin/omm.mjs (companion CLI)
