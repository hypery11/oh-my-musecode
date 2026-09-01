# Oh My Muse Code v0.1.0 — feature completeness matrix

Compared against public surfaces of:

- **OMC** [Yeachan-Heo/oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) v5.1.0 (plugin + live `omc` CLI + TypeScript engines)
- **OMX** [Yeachan-Heo/oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) v0.21.1 (plugin + live `omx` runtime)
- **OMG** [mihazs/oh-my-grok](https://github.com/mihazs/oh-my-grok) v0.2.x (Go hook binary + skill/rule loops)

Method: local tree at `/workspace/oh-my-musecode` plus GitHub contents API (no clones). `part9.md` is a 9-byte stub (`line-one`) and contains no omm plan. No `omc-inventory` / `omx-inventory` files were found under `/workspace`.

## Status legend

| Status | Meaning |
|--------|---------|
| **SHIPPED** | Hook or companion CLI actually executes (Python/Node), not just markdown |
| **TEMPLATE** | Slash-command or skill is markdown the model is asked to follow |
| **STUB** | CLI prints `planned: ...` and exits |
| **ADAPTED** | Muse-native equivalent exists; not a 1:1 port of OMC/OMX/OMG machinery |
| **CANNOT** | Muse 1.0.1-R2006.1 plugin contract has no API for this |
| **MISSING** | Public peers ship it; OMM v0.1 does not, and Muse would allow a port |

Evidence classes for OMM rows: hook scripts under `hooks/`, `bin/omm.mjs`, `.muse-plugin/plugin.json`, `commands/*.md`, `skills/*/SKILL.md`.

## Verdict

**v0.1.0 is a declared-surface plus stubs, not a full port of the public feature surface.**

The catalog is wide: 19 role skills, 19 slash-commands, 8 hook ids, and an `omm` CLI. What actually runs is a thin Python hook set (keyword to `.omm/mode.json`, optional skill-gate deny, Ralph `systemMessage` stop nudge, subagent JSONL, session greeting) plus `omm setup` / `omm doctor`. Everything named after OMC live engines — team tmux, ask providers, HUD statusline, ralph/ulw completion-promise loops, boulder/todo continuation, autopilot stage machine, wiki/memory/verify ledgers with session hooks — is either a prompt template, a CLI stub, or absent. OMG hashline / intent-gate / LSP / vendored superpowers are absent. Muse cannot host Claude statusline, Haiku/Opus routing, or plugin `apps`/`agents`; MCP and reminders are legal and still empty.

Calling this a port of OMC 5.x / OMX 0.21 / OMG 0.2 would overclaim. It is a Muse-native nameplate and prompt catalog with a few real gates.

## Compact scoreboard

| Feature | OMC | OMX | OMG | OMM v0.1 | Status |
|---------|-----|-----|-----|----------|--------|
| 19 role catalog | live agents + routing | role skills / workers | different 3-agent set | 19 SKILL.md files | TEMPLATE |
| `/ralph` + stop continuation | live persistent-mode | live Stop dispatcher | Go ralph/ulw + promise tags | command md + stop-chain systemMessage | ADAPTED |
| `/ulw` / ultrawork | ultrathink keyword + loops | ultrawork skill | `/ulw-loop` + oracle | keyword writes mode.json only | MISSING |
| `/ralplan` | live skill | live skill | prometheus `/plan` | command md | TEMPLATE |
| Skill-gate | pre-tool enforcer + Read tracking | PreToolUse dispatcher | catalog Read, fail-open | opt-in `.omm/skill-gate.json` deny | ADAPTED |
| Unified Stop chain | persistent-mode + drift + simplifier | Stop in native hook | ralph then boulder then todo then LSP then plan.md | ralph iterations only | ADAPTED |
| `/team` + worktrees | native team + `omc team` tmux | team runtime | n/a | command md + subagent log; CLI stub | TEMPLATE + STUB |
| `ask` providers | live `omc ask` / `/ask` | live ask skill | n/a | command md; CLI stub | TEMPLATE + STUB |
| HUD / statusline | live `omc hud` + Claude statusline | live HUD | n/a | command md snapshot; CLI stub | TEMPLATE + STUB / CANNOT live bar |
| `/deep-interview` | live Socratic skill | live skill | n/a | command md | TEMPLATE |
| `/ultragoal` | artifacts + CLI | live skill | n/a | command md | TEMPLATE |
| `/handoff` | session artifacts | session | skill + prompt collector | command md | TEMPLATE |
| `/skillify` | quality-gated extractor | n/a | writing-skills via superpowers | command md | TEMPLATE |
| `/wiki` | session start/end + compact hooks | wiki skill | n/a | command md; CLI stub | TEMPLATE + STUB |
| `mission` queue | missions dir + CLI | mission runner | n/a | CLI stub only | STUB |
| Notifications | Telegram/Discord/Slack/OpenClaw | configure-notifications | n/a | none | MISSING |
| Compact persistence | pre-compact + wiki + memory | Pre/PostCompact | n/a | hook emits `{}` | STUB |
| `setup` / `doctor` | live CLI + skills | live | n/a | CLI real; slash md | SHIPPED + TEMPLATE |
| `/remember` | project-memory hooks | n/a | n/a | command md | TEMPLATE |
| `/trace` | live skill | n/a | n/a | `/omm-trace` md | TEMPLATE |
| `/debug` | live skill | n/a | n/a | command md | TEMPLATE |
| `/verify` | evidence loop engine | verification src | Stop LSP/plan checks | command md | TEMPLATE |
| `/autopilot` | stage machine + Stop + HUD | live skill | n/a | command md + keyword | TEMPLATE |
| `/execute` | verify/fix engine | n/a | boulder `/start-work` | command md | TEMPLATE |
| Boulder / todo continuation | boulder-state + todo-continuation | state model | Go boulder + todo enforcer | none | MISSING |
| Intent-gate | prompt prerequisites / keywords | planning gate | INTENT_GATE collector | none | MISSING |
| Hashline | n/a (different edit model) | n/a | xxhash line tags + PreTool deny | none | MISSING |
| LSP diagnostics | n/a | n/a | post-tool LSP + Stop | none | MISSING |
| Bundled MCP | `.mcp.json` | plugin `.mcp.json` | ast-grep + lsp | `mcpServers: []` | MISSING |
| tmux team | `omc team N:codex|...` | team runtime / sparkshell | n/a | CLI stub | STUB |
| Haiku / Opus routing | model x agent matrix | model instructions | inherit | n/a | CANNOT |
| obra/superpowers vendor | not the OMC model | not OMX model | 14 skills vendored | none (intentional) | MISSING |
| `/loop` wrap | wraps Claude `/goal` in docs | `/goal` guidance | n/a | none; Muse `/loop` is builtin | MISSING / CANNOT as plugin id |
| Muse builtins wrap (`plan`/`grill`/`taste`) | wraps Claude builtins | wraps Codex `/goal` | wraps grok inspect | none | MISSING |

---

## 1. Nineteen roles

OMC ships `agents/` markdown for architect, planner, executor, explore, analyst, designer, debugger, tracer, critic, code-reviewer, security-reviewer, code-simplifier, test-engineer, qa-tester, verifier, scientist, document-specialist, writer, git-master, plus TypeScript routing (`src/agents`, model matrix, delegation enforcer). OMX maps an overlapping but smaller live skill set (analyze, design, git-master, worker, code-review). OMG does not use this 19-role catalog; it uses prometheus/metis/momus plus loop skills.

OMM copies the names into `skills/<id>/SKILL.md` (19 files, all declared in `plugin.json`). Each file is an original Muse role recipe: when to activate, `.omm/` paths, `subagent_spawn`, `.muse/worktrees/`. There is no router, no model tier, no entitlement graph.

| Role | OMM artifact | Status vs OMC live agent |
|------|----------------|-------------------------|
| architect through git-master (all 19) | `skills/<id>/SKILL.md` | TEMPLATE |

Related slash `/omm-skill` is TEMPLATE (tells the model to summarize a skill file).

## 2. Loops: ralph / ulw / ralplan / autopilot / execute

| Piece | What peers do | What OMM does | Status |
|-------|---------------|---------------|--------|
| `/ralph` | OMC persistent-mode Stop hooks; OMG writes `.omg/ralph-loop.local.md`, blocks Stop until a completion promise, max 100 | `commands/ralph.md` tells the model to write `.omm/ralph.json`; hook only nudges | TEMPLATE + ADAPTED hook |
| Stop continuation | OMC `persistent-mode.mjs`; OMG Stop block JSON; OMX Stop in `codex-native-hook.mjs` | `hooks/stop_chain.py`: if `ralph.json.active` and `iterations < max`, emit `{systemMessage: "Ralph loop still active..."}`. Does not increment iterations, does not emit a Stop block decision, no abort-reason matrix | SHIPPED (nudge only) |
| Keyword arming | OMC `keyword-detector.mjs` (ralph, ralplan, ultrathink, autopilot, cancelomc, ...) | `hooks/user_prompt.py` writes `.omm/mode.json` for ralplan/ralph/ultrathink/autopilot. No cancel token, no additionalContext injection | SHIPPED (mode file only) |
| `/ulw-loop` / ultrawork | OMG max 500 + oracle VERIFIED; OMX ultrawork skill | ultrathink is a keyword alias only; no ulw skill, no oracle, no `/ulw` command | MISSING |
| `/ralplan` | OMC/OMX live iterative planning skills | `commands/ralplan.md` writes plan + inactive `ralph.json` | TEMPLATE |
| `/autopilot` | OMC named stage profiles, Stop/HUD lifecycle, team execution config | `commands/autopilot.md` + keyword; no stage machine | TEMPLATE |
| `/execute` | OMC verify/fix loop from plan to code | `commands/execute.md` one-step recipe | TEMPLATE |
| Cancel / resume | OMC cancelomc; OMG `/cancel-ralph`, `/stop-continuation` | no cancel command; no pause marker | MISSING |

## 3. Skill-gate and Stop chain

| Piece | Peers | OMM | Status |
|-------|-------|-----|--------|
| Catalog discovery | OMG grok inspect / on-disk; OMC skill-injector | none | MISSING |
| Mark skill loaded on Read | OMG PostToolUse Read to skills.loaded | no PostToolUse hook at all | MISSING |
| Deny mutating tools | OMG always-on if catalog nonempty; OMC `pre-tool-enforcer.mjs` | `hooks/skill_gate.py` opt-in via `.omm/skill-gate.json` `{enabled, required}`. Compares to `.omm/read-skills.json` (must be written by the model). Denies Write/Edit/StrReplace and write-ish Bash. Emits Muse `permissionDecision=deny`; never bare `allow` | SHIPPED / ADAPTED (weaker) |
| Plan-mode write jail | OMG prometheus deny outside `.omg/**/*.md` | none | MISSING |
| Stop: ralph | yes | yes, systemMessage | ADAPTED |
| Stop: boulder plan checkboxes | OMG/OMC boulder-state | none | MISSING |
| Stop: todo enforcer | OMG cooldown + OMC todo-continuation | none | MISSING |
| Stop: LSP errors | OMG | none | MISSING |
| Stop: plan.md unchecked | OMG cap 8 | none | MISSING |
| Stop: workflow-drift / code-simplifier | OMC extra Stop hooks | none | MISSING |
| SessionStart greeting | OMC setup + memory + wiki | `session_start.py` lists /commands via systemMessage | SHIPPED |
| SessionEnd cleanup | OMC/OMG clear skill-gate/LSP/boulder | audit JSONL only | SHIPPED (noop cleanup) |
| Subagent start/stop | OMC tracker + verify-deliverables | append `.omm/team/log.jsonl` | SHIPPED (log only) |

`plugin.json` `mcpServers` and `reminders` are empty arrays. No PostToolUse, PermissionRequest, PostToolUseFailure, or Notification hooks.

## 4. Team, worktrees, ask, HUD

| Piece | Peers | OMM | Status |
|-------|-------|-----|--------|
| In-session `/team` | OMC staged pipeline team-plan to prd to exec to verify to fix; native Claude teams flag | `commands/team.md`: write mission/roster, spawn subagents | TEMPLATE |
| CLI `team` tmux workers | `omc team N:codex|gemini|antigravity|grok|cursor|claude`; OMX team runtime | `omm team` prints planned | STUB |
| Worktrees | OMC native team worktree mode; Muse already has `.muse/worktrees/` | skills/commands mention worktrees; no creator CLI | ADAPTED (docs only) |
| `/ask` multi-provider | OMC/OMX live advisor (claude/codex/gemini/antigravity/grok/cursor) | `commands/ask.md` routes to a skill, not a provider CLI | TEMPLATE / ADAPTED |
| CLI `ask` | live | stub | STUB |
| Live HUD / statusline | OMC `omc hud` + Claude statusline preset; OMX HUD | `/hud` tells the model to print a markdown table from `.omm/*`; `omm hud` stub | TEMPLATE + STUB |
| Claude/Codex statusline injection | OMC/OMX | Muse plugin capabilities: skills, commands, hooks, mcpServers, reminders only. No statusline / apps | CANNOT |

Muse spec: live remote ask transport is not ready. Even a faithful OMC-style provider advisor would be a companion-CLI feature, not a plugin capability.

