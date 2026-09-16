# AGENTS.md — working in Oh My Muse Code

This repository is a native Muse Code plugin (oh-my-musecode) plus a companion CLI (`omm`, single Rust binary, no dependencies). Agents including Muse itself should follow this file.

## What this repo is

- Plugin id: oh-my-musecode (display name: Oh My Muse Code)
- Manifest: .muse-plugin/plugin.json
- Marketplace: .claude-plugin/marketplace.json (marketplace name `omm`)
- Durable workspace state: .omm/ (gitignored except .gitkeep + README)
- Companion CLI: `omm`, installed via install.sh from the releases page

It is not a fork of oh-my-openagent, oh-my-claudecode, oh-my-codex, or oh-my-grok. Those projects inspired the feature set; all text and hook behavior here is original and Muse-native. Do not call Claude Code or Codex APIs.

## Muse plugin rules (1.3.0)

- schemaVersion is number 1. compat.source is native. compat.manifestDir is .muse-plugin.
- capabilities keys only: skills, commands, hooks, mcpServers, reminders (all arrays).
- Do not declare tools, agents, outputStyles, settings, or apps.
- Do not use plugin ids loop or muse-core.
- Portable ids: lowercase letter or digit, then lowercase letters, digits, dot, underscore, hyphen; max 80.
- Skill path must be a file (skills/<id>/SKILL.md), not a directory.
- Command path must be an existing markdown file.
- Hook command is an argv array (not a string): `["omm", "hook", "<name>"]`. Use timeoutMs (camelCase).
- No hook fields: matcher, cwd, env, timeout_ms, status_message, shell_command.
- Command ids and skill ids must be globally unique across families.
- Skills need YAML frontmatter name + description.
- plugin.json, VERSION, and .claude-plugin/marketplace.json versions agree (CI checks).

After any manifest or path change, run `muse plugins validate` with experimental plugins enabled, and `python3 scripts/check-manifest.py`. Do not install into the user Muse home unless they asked.

## How to work

1. Read the relevant skills/<id>/SKILL.md before playing that role.
2. Prefer the three slash-commands for diagnostics (`/omm-doctor`, `/omm-cost`, `/omm-status`); workflows live in skills, not commands.
3. The hooks observe and log; they never block or veto. The guard is a heuristic over literal command text, not a boundary — say so in docs, never imply otherwise.
4. English is primary. Keep README.zh-TW.md in sync with README.md.
5. Never claim a tmux dashboard, a statusline, output styles, or an ask transport unless the host ships the surface.

## Layout

- .muse-plugin/plugin.json
- skills/omm-*/SKILL.md (35 entries)
- commands/omm-*.md (3 slash-commands)
- reminders/omm-verify.md (off by default)
- docs/MIGRATION.md (0.3.0 → 1.0.0)
- install.sh, CHANGELOG.md, VERSION, LICENSE
