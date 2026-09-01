# Oh My Muse Code

[![License: MIT](https://img.shields.io/github/license/hypery11/oh-my-musecode)](LICENSE)
[![CI](https://img.shields.io/github/actions/workflow/status/hypery11/oh-my-musecode/ci.yml?branch=main)](https://github.com/hypery11/oh-my-musecode/actions)
[![Release](https://img.shields.io/github/v/release/hypery11/oh-my-musecode)](https://github.com/hypery11/oh-my-musecode/releases)

**The missing productivity layer for Meta Muse Code.**

![Oh My Muse Code demo](docs/assets/omm-demo.gif)

Agents: skip this README and read [AGENTS.md](AGENTS.md).

Oh My Muse Code (plugin id `oh-my-musecode`, CLI `omm`) adds a role catalog, slash-commands, session hooks, and durable `.omm/` state on top of stock Muse.

It is **inspired by** [oh-my-openagent](https://github.com/code-yeongyu/oh-my-openagent), [oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode), [oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex), and [oh-my-grok](https://github.com/mihazs/oh-my-grok). It is **not a fork**. Skills, commands, and hooks are original Muse-native text.

License: MIT. Copyright 2026 hypery11.

## Status

| Surface | Ready? |
|---------|--------|
| Native plugin (19 skills, 19 commands, 8 hooks) | **Plugin-ready** — validate with Muse 1.0.1-R2006.1 |
| Ralph Stop loop (`decision: block`) | **Shipped** — `muse plugins hook test` confirms `should_block: true` |
| `omm setup` / `omm doctor` | **Implemented** (local CLI, no deps) |
| `omm team` `ask` `hud` `wait` `mission` `wiki` `update` | **CLI stubs** — print `planned: ...` |
| Slash-commands `/team` `/ask` `/hud` etc. | **Plugin-ready** (in-session templates, not a live tmux HUD) |

There is **no** live tmux team dashboard or remote ask transport in this release. Do not assume those work.

Muse 1.0.1 plugin APIs are **experimental**. You must set `MUSE_EXPERIMENTAL_PLUGINS=1`.

## Install

Requires Muse Code with experimental plugins enabled.

One-liner (local tree):

    MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins install /path/to/oh-my-musecode && MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins approve oh-my-musecode

Marketplace (when the git remote is listed):

    export MUSE_EXPERIMENTAL_PLUGINS=1
    muse plugins marketplace add omm https://github.com/hypery11/oh-my-musecode
    muse plugins install oh-my-musecode@omm
    muse plugins approve oh-my-musecode

Validate without installing:

    MUSE_NO_AUTO_UPDATE=1 MUSE_LOGIN=0 MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins validate . --json

Companion CLI (optional):

    node bin/omm.mjs setup
    node bin/omm.mjs doctor

## Vanilla Muse vs Oh My Muse Code

| | Vanilla Muse Code | With oh-my-musecode |
|---|-------------------|---------------------|
| Roles | Bring your own prompts | 19 bundled skills (architect through git-master) |
| Slash-commands | Built-ins only | 19 workflow commands (`/ralph`, `/team`, `/verify`, ...) |
| Session state | Chat + transcripts | Durable `.omm/` plans, memory, traces, verify reports |
| Hooks | You write them | 8 hooks: session, prompt keywords, skill gate, Ralph stop-chain, subagent log, compact placeholder |
| Multi-agent | `subagent_spawn` + worktrees | Same Muse tools, plus team roster/log conventions |
| Companion CLI | `muse` | `omm setup` / `omm doctor` (other `omm` verbs are stubs) |
| Experimental flag | Needed for plugins | Documented; required on 1.0.1-R2006.1 |

## Features

### Skills (19)

architect, planner, executor, explore, analyst, designer, debugger, tracer, critic, code-reviewer, security-reviewer, code-simplifier, test-engineer, qa-tester, verifier, scientist, document-specialist, writer, git-master

Each skill is a Muse role recipe: when to activate, how to use `subagent_spawn` and `.muse/worktrees/`, and which `.omm/` files to persist. They do not mention Claude Code or Codex APIs.

### Commands (19)

| Command | Job |
|---------|-----|
| `/team` | Multi-skill mission + roster under `.omm/team/` |
| `/autopilot` | Plan then execute with light supervision |
| `/execute` | One plan step or concrete task |
| `/ralph` | Work-until-done loop with iteration budget |
| `/ralplan` | Ralph-oriented planning interview |
| `/deep-interview` | Requirements interview |
| `/ask` | Route a question to the best skill (in-session; CLI stub) |
| `/verify` | Evidence-gated completion |
| `/ultragoal` | North-star goal + first milestone |
| `/handoff` | Next-session brief |
| `/skillify` | Draft a new skill from a workflow |
| `/omm-skill` | Explain a bundled skill |
| `/hud` | Text snapshot of `.omm/` state (not a live TUI) |
| `/omm-setup` | Print install/approve steps |
| `/omm-doctor` | Diagnose Muse binary + plugin tree |
| `/remember` | Durable notes |
| `/omm-trace` | End-to-end flow trace |
| `/wiki` | Lightweight `.omm/wiki/` pages |
| `/debug` | Structured debugging session |

### Hooks (8 unique scripts)

| Event | Id | Behavior |
|-------|----|----------|
| SessionStart | session-start | Optional system message listing `/commands` |
| UserPromptSubmit | prompt-keywords | If prompt mentions ralph/ralplan/ultrathink/autopilot, write `.omm/mode.json` |
| PreToolUse | skill-gate | Optional deny of mutating tools until required skills are marked read |
| Stop | stop-chain | If `.omm/ralph.json` is active and under max, keep going |
| SubagentStart | subagent-start | Append `.omm/team/log.jsonl` |
| SubagentStop | subagent-stop | Same log |
| PreCompact | pre-compact | Placeholder (`.omm/` reminder) |
| SessionEnd | session-end | Quiet audit |

Each hook is a distinct Python file (argv uniqueness). Shared helpers live in `hooks/_omm.py` and are **not** hook sources. Hooks never emit a bare `permissionDecision=allow`.

### State

Workspace state lives under `.omm/` (see `.omm/README.md`). Plugin source is this repo. Muse worktrees: `.muse/worktrees/`.

### Companion CLI

`package.json` bin `omm` -> `bin/omm.mjs` (Node, no dependencies).

- `omm setup` — print Muse install/approve commands with the experimental flag
- `omm doctor` — locate `muse`, check this tree, run validate if found
- `omm team|ask|hud|wait|mission|wiki|update` — stubs (`planned: ...`)
- `-h` / `--help` and `-V`

## Layout

    .muse-plugin/plugin.json
    skills/<id>/SKILL.md
    commands/<id>.md
    hooks/*.py
    bin/omm.mjs
    .omm/                 (runtime; mostly gitignored)

## 繁體中文（摘要）

**Oh My Muse Code** 是 Meta Muse Code 缺少的生產力層：19 個技能、19 個斜線指令、8 個 hooks，以及工作區狀態目錄 `.omm/`。靈感來自 oh-my-openagent、oh-my-claudecode、oh-my-codex、oh-my-grok，**不是 fork**，內容皆為原創。

Muse 1.0.1 的 plugin API 仍是實驗功能，請設定 `MUSE_EXPERIMENTAL_PLUGINS=1`，再用 `muse plugins install` / `marketplace` / `approve`。

本版 **沒有** 實作即時 tmux 團隊儀表板或遠端 ask 通道。`omm team|ask|hud|...` 僅是 CLI 占位；真正可用的是 plugin 內的斜線指令與 hooks。完整中文說明見 [README.zh-TW.md](README.zh-TW.md)。

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](.github/CODE_OF_CONDUCT.md).
Security: [SECURITY.md](SECURITY.md). Roadmap: [ROADMAP.md](ROADMAP.md).
Honest feature matrix vs OMC/OMX/OMG: [docs/FEATURE-MATRIX.md](docs/FEATURE-MATRIX.md).

## License

MIT. Copyright (c) 2026 [hypery11](https://github.com/hypery11).
