> v0.1.1 ships only hud snapshot + keyword wrapper, not a full port.

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

The catalog is wide: 19 role skills, 19 slash-commands, 8 hook ids, and an `omm` CLI. What actually runs is a thin Python hook set (keyword to `.omm/mode.json`, optional skill-gate deny, Ralph `systemMessage` stop nudge, subagent JSONL, session greeting) plus `omm setup` / `omm doctor` / `omm hud` text snapshot, and a real Ralph Stop `decision:block`. Everything else named after OMC live engines — team tmux, ask providers, live HUD statusline, ralph/ulw completion-promise loops, boulder/todo continuation, autopilot stage machine, wiki/memory/verify ledgers with session hooks — is either a prompt template, a CLI stub, or absent. OMG hashline / intent-gate / LSP / vendored superpowers are absent. Muse cannot host Claude statusline, Haiku/Opus routing, or plugin `apps`/`agents`; MCP and reminders are legal and still empty.

Calling this a port of OMC 5.x / OMX 0.21 / OMG 0.2 would overclaim. It is a Muse-native nameplate and prompt catalog with a few real gates.

## Compact scoreboard

| Feature | OMC | OMX | OMG | OMM v0.1 | Status |
|---------|-----|-----|-----|----------|--------|
| 19 role catalog | live agents + routing | role skills / workers | different 3-agent set | 19 SKILL.md files | TEMPLATE |
| `/ralph` + stop continuation | live persistent-mode | live Stop dispatcher | Go ralph/ulw + promise tags | command md + stop-chain `decision:block` (hook test confirmed) | SHIPPED (adapted) |
| `/ulw` / ultrawork | ultrathink keyword + loops | ultrawork skill | `/ulw-loop` + oracle | keyword writes mode.json only | MISSING |
| `/ralplan` | live skill | live skill | prometheus `/plan` | command md | TEMPLATE |
| Skill-gate | pre-tool enforcer + Read tracking | PreToolUse dispatcher | catalog Read, fail-open | opt-in `.omm/skill-gate.json` deny | ADAPTED |
| Unified Stop chain | persistent-mode + drift + simplifier | Stop in native hook | ralph then boulder then todo then LSP then plan.md | ralph iterations only | ADAPTED |
| `/team` + worktrees | native team + `omc team` tmux | team runtime | n/a | command md + subagent log; CLI stub | TEMPLATE + STUB |
| `ask` providers | live `omc ask` / `/ask` | live ask skill | n/a | command md; CLI stub | TEMPLATE + STUB |
| HUD / statusline | live `omc hud` + Claude statusline | live HUD | n/a | CLI text snapshot of `.omm/`; slash `/hud` template; no live statusline | CLI snapshot SHIPPED (adapted); live statusline CANNOT; slash `/hud` TEMPLATE |
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
| `/ralph` | OMC persistent-mode Stop hooks; OMG writes `.omg/ralph-loop.local.md`, blocks Stop until a completion promise, max 100 | `commands/ralph.md` arms `.omm/ralph.json`; Stop hook blocks until DONE or budget | TEMPLATE + SHIPPED hook |
| Stop continuation | OMC `persistent-mode.mjs`; OMG Stop block JSON; OMX Stop in `codex-native-hook.mjs` | `hooks/stop_chain.py` emits `{decision:"block",reason}` while active and under budget; increments iterations; `<promise>DONE</promise>` or abort/cancel allows exit. Confirmed `should_block: true` via `muse plugins hook test` on Stop | SHIPPED (adapted) |
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
| Stop: ralph | yes | `decision:block` + DONE promise | SHIPPED (adapted) |
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
| Live HUD / statusline | OMC `omc hud` + Claude statusline preset; OMX HUD | `omm hud` text snapshot of `.omm/`; `/hud` still prints an in-session markdown table; no Muse statusline | CLI snapshot SHIPPED (adapted); slash TEMPLATE; live bar CANNOT |
| Claude/Codex statusline injection | OMC/OMX | Muse plugin capabilities: skills, commands, hooks, mcpServers, reminders only. No statusline / apps | CANNOT |

Muse spec: live remote ask transport is not ready. Even a faithful OMC-style provider advisor would be a companion-CLI feature, not a plugin capability.


## 5. Interview, goals, handoff, skillify, wiki, mission

| Feature | OMM path | Status |
|---------|----------|--------|
| `/deep-interview` | `commands/deep-interview.md` to `.omm/requirements.md` | TEMPLATE |
| `/ultragoal` | `commands/ultragoal.md` to `.omm/ultragoal.md` plus milestone-1 plan | TEMPLATE (no `omm ultragoal` CLI; OMC has `omc ultragoal create-goals`) |
| `/handoff` | `commands/handoff.md` to `.omm/handoff.md` | TEMPLATE (OMG injects phases on `/handoff` via UserPromptSubmit) |
| `/skillify` | `commands/skillify.md` draft SKILL.md | TEMPLATE (no OMC quality gates / auto-inject) |
| `/wiki` | `commands/wiki.md` to `.omm/wiki/` | TEMPLATE; `omm wiki` STUB; no OMC wiki session/compact engines |
| `mission` | CLI only | STUB (no `commands/mission.md`; OMC missions/, OMX mission queue) |
| `/remember` | append `.omm/memory.md` | TEMPLATE (OMC project-memory hooks on SessionStart/PostToolUse/PreCompact) |
| `/omm-trace` | write `.omm/trace/` | TEMPLATE |
| `/debug` | `.omm/debug/` | TEMPLATE |
| `/verify` | `.omm/verify.json` | TEMPLATE (no evidence engine, no Stop re-entry) |

## 6. Setup, doctor, compact, notifications

| Feature | OMM | Status |
|---------|-----|--------|
| `omm setup` | prints muse plugins install/approve with experimental flag; does not mutate Muse home | SHIPPED |
| `/omm-setup` | same text via the model | TEMPLATE |
| `omm doctor` | tree counts 19/19/8, parses manifest, optional muse plugins validate --json | SHIPPED |
| `/omm-doctor` | model-run checklist | TEMPLATE |
| omm update | stub | STUB |
| PreCompact | pre_compact.py emits empty JSON | STUB |
| Notifications | no hook | MISSING |
| omm wait | stub | STUB |

## 7. Engines OMM does not have

| Feature | Who ships it | Muse 1.0.1 feasible? | OMM status |
|---------|--------------|----------------------|------------|
| Boulder / todo continuation | OMG + OMC | Yes via Stop + files | MISSING |
| Intent-gate banners | OMG + OMC | Yes via UserPromptSubmit | MISSING |
| Hashline LINE#ID edits | OMG only | Partial without Read rewrite | MISSING (likely CANNOT faithful) |
| LSP post-tool + Stop | OMG | Yes via mcpServers + PostToolUse | MISSING |
| ast-grep MCP | OMG | Yes | MISSING |
| Bundled MCP | OMC OMX OMG | Yes | MISSING (mcpServers empty) |
| tmux multi-CLI team | OMC/OMX | Yes as companion CLI | STUB |
| Haiku / Opus routing | OMC | CANNOT: Muse Spark; agents capability rejected | CANNOT |
| obra/superpowers vendor | OMG 14 skills | Yes as extra skills | MISSING (deliberate: not a fork) |
| /loop wrap | OMC wraps Claude /goal; Muse has builtin /loop | plugin id loop reserved | CANNOT (id) / MISSING (wrapper) |
| Wrap Muse builtins plan/grill/taste | OMC/OMX wrap host builtins | prose only | MISSING (no wrapper commands) |
| Autoresearch visual-verdict deepinit graph release self-improve PSM | OMC | Yes | MISSING |
| Named autopilot workflow profiles | OMC v5 | Yes | MISSING |
| Skill auto-inject / learner | OMC | Partial UserPromptSubmit | MISSING |
| OpenClaw / Discord gateway | OMC/OMX | Companion CLI | MISSING |

## 8. CLI surface (bin/omm.mjs)

| Verb | Behavior | Status |
|------|----------|--------|
| setup | print install/approve recipe | SHIPPED |
| doctor | manifest + counts + optional validate | SHIPPED |
| hud | text snapshot of `.omm/` (not a TUI) | SHIPPED (adapted) |
| team ask wait mission wiki update | prints planned stub | STUB |
| ralph autopilot execute ultragoal verify | unknown command | MISSING (slash only) |

## 9. What Muse cannot host (so a full port is impossible)

Native plugin capabilities keys: skills, commands, hooks, mcpServers, reminders. Rejected in this phase: tools, agents, outputStyles, settings, apps. Hook matcher is a Claude/Codex field. No statusline. No PreLLMCall. Live in-process ask is documented not-ready. Reserved plugin ids include loop and muse-core.

Therefore even a complete OMM cannot be a 1:1 OMC/OMX: those products are runtimes (tmux, statusline, model routing, MCP apps). Honest ceiling for Muse 1.0.1 is richer hooks (PostToolUse, Stop block, compact flush), real omm verbs, optional MCP, and better prompt templates - still ADAPTED.

## 10. Inventories consulted

| Source | Result |
|--------|--------|
| /workspace/oh-my-musecode | plugin.json, 19 skills, 19 commands, 8 hooks, bin/omm.mjs |
| /workspace/ohmy-research/oh-my-grok-dissection.md | full OMG v0.2 surface |
| /workspace/ohmy-research/part9.md | empty (line-one); no omm plan |
| omc-inventory / omx-inventory / omm-plan | not present under /workspace |
| GitHub API Yeachan-Heo/oh-my-claudecode | plugin.json 5.1.0, 35 skills, 21 commands, hooks.json, omc CLI |
| GitHub API Yeachan-Heo/oh-my-codex | plugin.json 0.21.1, plugin skills + native hook dispatcher |

Text in this file is original. Skill bodies from OMC/OMX/OMG were not copied.

