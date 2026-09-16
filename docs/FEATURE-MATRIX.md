> Superseded by 1.0.0. This matrix describes the 0.3.0 surface (19 role
> skills, 19 commands, Node CLI, Python hooks), which 1.0.0 replaced — see
> [MIGRATION.md](MIGRATION.md). Kept for the record; a fresh matrix lands once
> the new surface settles (ROADMAP).

> v0.3.0 ships file-based ralplan/interview/verify/autopilot engines + SKILL.md ask router. Still not a 1:1 port.

# Oh My Muse Code v0.3.0 — feature completeness matrix

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

**Still not a 1:1 OMC/OMX/OMG port.** The catalog is wide; live engines are file-based `.omm/` state machines plus hooks. Interview/verify/ralplan/autopilot/execute slash-commands remain in-session interviewers; companion CLI now writes the matching `.omm/` state files. Skipped on purpose: live tmux team dashboard, Claude statusline, Haiku/Opus routing, hashline, LSP, bundled MCP, vendored superpowers, wrapping Muse `/loop`.

What actually runs: Python hooks (keyword mode, skill-gate, fail-open intent-gate, Ralph/ulw/boulder/todo/autopilot Stop chain, subagent JSONL, compact flush, optional session-end webhook) plus `omm` file-based verbs (`setup` `doctor` `hud` `team` `ask` `wait` `mission` `wiki` `update` `ralplan` `interview` `ultragoal` `handoff` `skillify` `verify` `autopilot` `execute` `remember` `debug` `trace`). Slash-commands remain in-session interviewers. Hashline, LSP, bundled MCP, vendored superpowers, live tmux, Claude statusline, Haiku/Opus routing, and wrapping Muse `/loop` are skipped. Muse cannot host statusline or `apps`/`agents`; MCP arrays stay empty.

## Compact scoreboard

| Feature | OMC | OMX | OMG | OMM v0.3 | Status |
|---------|-----|-----|-----|----------|--------|
| 19 role catalog | live agents + routing | role skills / workers | different 3-agent set | 19 SKILL.md files + `omm ask` YAML router | TEMPLATE skills; SHIPPED (adapted, file-based) router |
| `/ralph` + stop continuation | live persistent-mode | live Stop dispatcher | Go ralph/ulw + promise tags | command md + stop-chain `decision:block` (hook test confirmed) | SHIPPED (adapted) |
| `/ulw` / ultrawork | ultrathink keyword + loops | ultrawork skill | `/ulw-loop` + oracle | keyword writes mode.json; Stop blocks on `.omm/ulw.json` or `ultrawork.json` | SHIPPED (adapted, no oracle) |
| `/ralplan` | live skill | live skill | prometheus `/plan` | CLI writes mode/plan.md/ralph.json (inactive); slash interviews | SHIPPED (adapted, file-based); slash interviewer |
| Skill-gate | pre-tool enforcer + Read tracking | PreToolUse dispatcher | catalog Read, fail-open | opt-in `.omm/skill-gate.json` deny | ADAPTED |
| Unified Stop chain | persistent-mode + drift + simplifier | Stop in native hook | ralph then boulder then todo then LSP then plan.md | ralph then ulw then boulder then capped todo nudge then autopilot | ADAPTED |
| `/team` + worktrees | native team + `omc team` tmux | team runtime | n/a | command md + subagent log; CLI writes mission/roster (not tmux) | TEMPLATE + SHIPPED (adapted) |
| `ask` providers | live `omc ask` / `/ask` | live ask skill | n/a | command md; CLI scores SKILL.md description+id (no remote model) | TEMPLATE + SHIPPED (adapted, file-based) |
| HUD / statusline | live `omc hud` + Claude statusline | live HUD | n/a | CLI text snapshot of `.omm/` (ralph/plan/verify/team/memory/autopilot/interview/debug/trace/handoff when present); slash `/hud` template; no live statusline | CLI snapshot SHIPPED (adapted); live statusline CANNOT; slash `/hud` TEMPLATE |
| `/deep-interview` | live Socratic skill | live skill | n/a | CLI `interview`/`deep-interview` writes stamp + requirements.md; slash interviews | SHIPPED (adapted, file-based); slash interviewer |
| `/ultragoal` | artifacts + CLI | live skill | n/a | CLI writes mode/ultragoal.md/milestone-1 plan.json; slash fills prose | SHIPPED (adapted, file-based); slash interviewer |
| `/handoff` | session artifacts | session | skill + prompt collector | CLI writes handoff.md from mode/plan/verify/team (no secrets) | SHIPPED (adapted, file-based); slash interviewer |
| `/skillify` | quality-gated extractor | n/a | writing-skills via superpowers | CLI drafts `.omm/skillify/`; `--apply` only for portable slugs | SHIPPED (adapted, file-based); slash interviewer |
| `/wiki` | session start/end + compact hooks | wiki skill | n/a | command md; CLI files under `.omm/wiki/` | TEMPLATE + SHIPPED |
| `mission` queue | missions dir + CLI | mission runner | n/a | CLI `.omm/mission/queue.json` | SHIPPED (adapted) |
| Notifications | Telegram/Discord/Slack/OpenClaw | configure-notifications | n/a | optional SessionEnd POST from `.omm/notify.json` http(s) URL | SHIPPED (adapted) |
| Compact persistence | pre-compact + wiki + memory | Pre/PostCompact | n/a | writes `.omm/compact.json` + appends `memory.md`; still emits `{}` | SHIPPED (adapted) |
| `setup` / `doctor` | live CLI + skills | live | n/a | CLI real; slash md | SHIPPED + TEMPLATE |
| `/remember` | project-memory hooks | n/a | n/a | CLI appends memory.md + memory.jsonl; refuses secrets | SHIPPED (adapted, file-based); slash interviewer |
| `/trace` | live skill | n/a | n/a | CLI static plugin outline under `.omm/trace/` | SHIPPED (adapted, file-based); slash interviewer |
| `/debug` | live skill | n/a | n/a | CLI writes `.omm/debug/<stamp>.md` + mode debug | SHIPPED (adapted, file-based); slash interviewer |
| `/verify` | evidence loop engine | verification src | Stop LSP/plan checks | CLI verify.json pending/pass/fail; slash still gathers evidence | SHIPPED (adapted, file-based); slash interviewer |
| `/autopilot` | stage machine + Stop + HUD | live skill | n/a | CLI autopilot.json + Stop after todo; slash walks steps | SHIPPED (adapted, file-based); slash interviewer |
| `/execute` | verify/fix engine | n/a | boulder `/start-work` | CLI marks plan.json in-progress/done + progress.md | SHIPPED (adapted, file-based); slash interviewer |
| Boulder / todo continuation | boulder-state + todo-continuation | state model | Go boulder + todo enforcer | Stop reads `.omm/boulder.json` + capped `.omm/todo.json` nudge | SHIPPED (adapted) |
| Intent-gate | prompt prerequisites / keywords | planning gate | INTENT_GATE collector | fail-open `.omm/intent-gate.json`; PreToolUse deny if plan.json missing | SHIPPED (adapted) |
| Hashline | n/a (different edit model) | n/a | xxhash line tags + PreTool deny | none | CANNOT / intentional |
| LSP diagnostics | n/a | n/a | post-tool LSP + Stop | none | CANNOT / intentional |
| Bundled MCP | `.mcp.json` | plugin `.mcp.json` | ast-grep + lsp | `mcpServers: []` | CANNOT / intentional (empty) |
| tmux team | `omc team N:codex|...` | team runtime / sparkshell | n/a | skipped (file-based `omm team` only) | CANNOT / intentional |
| Haiku / Opus routing | model x agent matrix | model instructions | inherit | n/a | CANNOT |
| obra/superpowers vendor | not the OMC model | not OMX model | 14 skills vendored | none (intentional) | CANNOT / intentional |
| `/loop` wrap | wraps Claude `/goal` in docs | `/goal` guidance | n/a | none; Muse `/loop` is builtin; plugin id `loop` forbidden | CANNOT / intentional |
| Muse builtins wrap (`plan`/`grill`/`taste`) | wraps Claude builtins | wraps Codex `/goal` | wraps grok inspect | none | MISSING |

---

## 1. Nineteen roles

OMC ships `agents/` markdown for architect, planner, executor, explore, analyst, designer, debugger, tracer, critic, code-reviewer, security-reviewer, code-simplifier, test-engineer, qa-tester, verifier, scientist, document-specialist, writer, git-master, plus TypeScript routing (`src/agents`, model matrix, delegation enforcer). OMX maps an overlapping but smaller live skill set (analyze, design, git-master, worker, code-review). OMG does not use this 19-role catalog; it uses prometheus/metis/momus plus loop skills.

OMM copies the names into `skills/<id>/SKILL.md` (19 files, all declared in `plugin.json`). Each file is an original Muse role recipe: when to activate, `.omm/` paths, `subagent_spawn`, `.muse/worktrees/`. `omm ask` scores those YAML descriptions (no remote model, no model tier, no entitlement graph).

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
| `/ulw-loop` / ultrawork | OMG max 500 + oracle VERIFIED; OMX ultrawork skill | ultrathink keyword + Stop loop on `.omm/ulw.json` or `ultrawork.json`; no oracle, no `/ulw` command | SHIPPED (adapted) |
| `/ralplan` | OMC/OMX live iterative planning skills | CLI writes mode/plan.md stub/inactive `ralph.json`; slash interviews | SHIPPED (adapted, file-based); slash interviewer |
| `/autopilot` | OMC named stage profiles, Stop/HUD lifecycle, team execution config | CLI `autopilot.json` + Stop after todo; no named stage profiles | SHIPPED (adapted, file-based); slash interviewer |
| `/execute` | OMC verify/fix loop from plan to code | CLI next pending `plan.json` step + `progress.md`; slash implements | SHIPPED (adapted, file-based); slash interviewer |
| Cancel / resume | OMC cancelomc; OMG `/cancel-ralph`, `/stop-continuation` | no cancel command; no pause marker | MISSING |

## 3. Skill-gate and Stop chain

| Piece | Peers | OMM | Status |
|-------|-------|-----|--------|
| Catalog discovery | OMG grok inspect / on-disk; OMC skill-injector | none | MISSING |
| Mark skill loaded on Read | OMG PostToolUse Read to skills.loaded | no PostToolUse hook at all | MISSING |
| Deny mutating tools | OMG always-on if catalog nonempty; OMC `pre-tool-enforcer.mjs` | `hooks/skill_gate.py` opt-in via `.omm/skill-gate.json` `{enabled, required}`. Compares to `.omm/read-skills.json` (must be written by the model). Denies Write/Edit/StrReplace and write-ish Bash. Emits Muse `permissionDecision=deny`; never bare `allow` | SHIPPED / ADAPTED (weaker) |
| Plan-mode write jail | OMG prometheus deny outside `.omg/**/*.md` | none | MISSING |
| Stop: ralph | yes | `decision:block` + DONE promise | SHIPPED (adapted) |
| Stop: boulder plan checkboxes | OMG/OMC boulder-state | `.omm/boulder.json` Ralph-style loop until DONE or max | SHIPPED (adapted) |
| Stop: todo enforcer | OMG cooldown + OMC todo-continuation | `.omm/todo.json` open items: block once (nudge cap) | SHIPPED (adapted) |
| Stop: LSP errors | OMG | none | MISSING |
| Stop: plan.md unchecked | OMG cap 8 | none | MISSING |
| Stop: workflow-drift / code-simplifier | OMC extra Stop hooks | none | MISSING |
| SessionStart greeting | OMC setup + memory + wiki | `session_start.py` lists /commands via systemMessage | SHIPPED |
| SessionEnd cleanup | OMC/OMG clear skill-gate/LSP/boulder | audit JSONL; optional webhook POST | SHIPPED (adapted) |
| Subagent start/stop | OMC tracker + verify-deliverables | append `.omm/team/log.jsonl` | SHIPPED (log only) |

`plugin.json` `mcpServers` and `reminders` are empty arrays. No PostToolUse, PermissionRequest, PostToolUseFailure, or Notification hooks.

## 4. Team, worktrees, ask, HUD

| Piece | Peers | OMM | Status |
|-------|-------|-----|--------|
| In-session `/team` | OMC staged pipeline team-plan to prd to exec to verify to fix; native Claude teams flag | `commands/team.md`: write mission/roster, spawn subagents | TEMPLATE |
| CLI `team` tmux workers | `omc team N:codex|gemini|antigravity|grok|cursor|claude`; OMX team runtime | skipped; `omm team` writes `.omm/team/` files only | CANNOT / intentional |
| Worktrees | OMC native team worktree mode; Muse already has `.muse/worktrees/` | skills/commands mention worktrees; no creator CLI | ADAPTED (docs only) |
| `/ask` multi-provider | OMC/OMX live advisor (claude/codex/gemini/antigravity/grok/cursor) | `commands/ask.md` routes to a skill, not a provider CLI | TEMPLATE / ADAPTED |
| CLI `ask` | live providers | scores 19 SKILL.md YAML descriptions + first heading + id; writes `.omm/ask/last.json` | SHIPPED (adapted, file-based) |
| Live HUD / statusline | OMC `omc hud` + Claude statusline preset; OMX HUD | `omm hud` text snapshot of `.omm/` (including autopilot/interview/debug/trace/handoff when present); `/hud` still prints an in-session markdown table; no Muse statusline | CLI snapshot SHIPPED (adapted); slash TEMPLATE; live bar CANNOT |
| Claude/Codex statusline injection | OMC/OMX | Muse plugin capabilities: skills, commands, hooks, mcpServers, reminders only. No statusline / apps | CANNOT |

Muse spec: live remote ask transport is not ready. Even a faithful OMC-style provider advisor would be a companion-CLI feature, not a plugin capability.


## 5. Interview, goals, handoff, skillify, wiki, mission

| Feature | OMM path | Status |
|---------|----------|--------|
| `/deep-interview` | CLI `omm interview`/`deep-interview` → `.omm/interview/<stamp>.md` + `requirements.md` | SHIPPED (adapted, file-based); slash interviewer |
| `/ultragoal` | CLI writes `.omm/ultragoal.md` + milestone-1 `plan.json` | SHIPPED (adapted, file-based); slash interviewer |
| `/handoff` | CLI summarizes mode/plan/verify/team into `.omm/handoff.md` (no secrets) | SHIPPED (adapted, file-based); slash interviewer |
| `/skillify` | CLI drafts `.omm/skillify/<slug>.md`; `--apply` only if portable slug | SHIPPED (adapted, file-based); slash interviewer |
| `/wiki` | `commands/wiki.md` to `.omm/wiki/` | TEMPLATE + `omm wiki` SHIPPED (files only; no OMC session/compact wiki engine) |
| `mission` | CLI only | SHIPPED `.omm/mission/queue.json` (no `commands/mission.md`) |
| `/remember` | CLI appends `memory.md` + `memory.jsonl`; refuses secrets | SHIPPED (adapted, file-based); slash interviewer |
| `/omm-trace` | CLI static plugin outline under `.omm/trace/<slug>.md` | SHIPPED (adapted, file-based); slash interviewer |
| `/debug` | CLI `.omm/debug/<stamp>.md` + mode.json debug | SHIPPED (adapted, file-based); slash interviewer |
| `/verify` | CLI `verify.json` pending/pass/fail + `verify.md` (no Stop re-entry) | SHIPPED (adapted, file-based); slash interviewer |

## 6. Setup, doctor, compact, notifications

| Feature | OMM | Status |
|---------|-----|--------|
| `omm setup` | prints muse plugins install/approve with experimental flag; does not mutate Muse home | SHIPPED |
| `/omm-setup` | same text via the model | TEMPLATE |
| `omm doctor` | tree counts 19/19/8, parses manifest, optional muse plugins validate --json | SHIPPED |
| `/omm-doctor` | model-run checklist | TEMPLATE |
| omm update | print muse plugins update/approve; optional registry check | SHIPPED |
| PreCompact | compact.json + memory.md line; emit `{}` | SHIPPED (adapted) |
| Notifications | optional SessionEnd webhook via notify.json | SHIPPED (adapted) |
| omm wait | poll `.omm/team/log.jsonl` mtime | SHIPPED |

## 7. Engines OMM does not have

Skipped on purpose this release (CANNOT / intentional): live tmux team dashboard, Claude statusline, Haiku/Opus routing, hashline, LSP, bundled MCP servers, vendored superpowers, wrapping Muse `/loop` or other Muse builtins as colliding command ids.

| Feature | Who ships it | Muse 1.0.1 feasible? | OMM status |
|---------|--------------|----------------------|------------|
| Boulder / todo continuation | OMG + OMC | Yes via Stop + files | SHIPPED (adapted) |
| Intent-gate banners | OMG + OMC | Yes via PreToolUse + fail-open file | SHIPPED (adapted; UserPromptSubmit does not crash) |
| Hashline LINE#ID edits | OMG only | Partial without Read rewrite | CANNOT / intentional |
| LSP post-tool + Stop | OMG | Yes via mcpServers + PostToolUse | CANNOT / intentional |
| ast-grep MCP | OMG | Yes | MISSING |
| Bundled MCP | OMC OMX OMG | Yes | CANNOT / intentional (mcpServers empty) |
| tmux multi-CLI team | OMC/OMX | Yes as companion CLI | CANNOT / intentional (no fake tmux TUI) |
| Haiku / Opus routing | OMC | CANNOT: Muse Spark; agents capability rejected | CANNOT |
| obra/superpowers vendor | OMG 14 skills | Yes as extra skills | CANNOT / intentional (not a fork) |
| /loop wrap | OMC wraps Claude /goal; Muse has builtin /loop | plugin id loop reserved | CANNOT / intentional |
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
| team ask wait mission wiki update | file-based `.omm/` engines (not tmux / not remote) | SHIPPED (adapted) |
| ralplan interview ultragoal handoff skillify verify autopilot execute remember debug trace | file-based `.omm/` engines; slash remains in-session interviewer | SHIPPED (adapted, file-based) |

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

