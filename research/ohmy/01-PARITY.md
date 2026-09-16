# 01-PARITY — does `oh-my-musecode` implement the features of the other oh-my-* frameworks?

Written 2026-09-05 against omm `0.1.0` @ `60931a2` (12 shipped skills; 23 more in flight, see the
footnote under §1). Inputs: the eight verified inventories in `parity-inventories.json` (seven
rivals, 522 feature rows, plus omm's 53), classified against `docs/host-reality.md`,
`research/musecode/00-SYNTHESIS.md` §2 (extension points and dead ends), `research/ohmy/00-MATRIX.md`
§4/§6/§7 and `research/experiments/00-DECISION.md` §0/§2.5/§4.

## 0. Headline

**No.** Of the 162 distinct capabilities the seven rivals ship between them, omm ships 61 outright
(38 %), matches 39 only partly (24 % — the same capability with far less content behind it: 12
skills against 29–114, 3 commands against 7–77, 3 hooks against 6–56, no agent roster against 7–28),
leaves 36 as buildable gaps (22 %), has ruled out 23 by design because Muse ships them natively or
the research struck them (14 %), and 3 the Muse binary makes impossible (statusline, output styles,
per-skill tool grants). Where omm is ahead is lifecycle, not content: the ledger,
the three-way merge, the byte-identical uninstall, the measured cost accounting, the host-drift
self-test and the offline wire harness have no counterpart in any of the seven inventories.

## 1. Totals

| omm status | Rows | Share | Meaning |
|---|---|---|---|
| HAVE | 61 | 38 % | shipped, with the file or command cited (includes host-native capabilities only where omm adds a command, check, ledger entry or lint) |
| PARTIAL | 39 | 24 % | the capability exists but differs materially in breadth or depth; the cell says how |
| GAP | 36 | 22 % | buildable on Muse, not built; the cell says roughly what it would take |
| IMPOSSIBLE_ON_MUSE | 3 | 2 % | the host has no surface for it (`host-reality.md`, SYNTHESIS §2.4) |
| OUT_OF_SCOPE_BY_DESIGN | 23 | 14 % | ruled out by MATRIX §4/§6 or DECISION §0/§4 (framework not agent; no in-process execution; Muse ships subagents and workflows; no enterprise tier) |
| **Total distinct capability rows** | **162** | | merged from the 522 rival rows (67 OMH, 73 SLIM, 67 OMA, 77 OMO, 63 OMC, 95 OMX, 80 OMP); rival rows that record a defect, a refuted claim or an explicit "NONE" were dropped rather than counted, because an absence is not a capability |

Footnote on skills. omm's tree has 12 skills in `content/catalog.json` and an untracked 13th
(`content/skills/omm-release/`). Twenty-three more are being finalised at the time of writing
(`omm-pr perf migrate ci release incident api-design errors observability config concurrency shell
rust typescript python database frontend test-design legacy repo-map cleanup handoff containers`).
They are counted below as **PARTIAL, HAVE-pending**, never as HAVE: nothing is HAVE until it is in
`catalog.json`, passes `omm lint`, and fits the measured catalog budget (`omm cost`). At 35 skills
the bundle would sit between SLIM (8) and OMX (29) and well under OMH (114) — and a 35-skill bundle
at today's average entry cost (≈ 390 B `first_sentence`) is ≈ 13.6 KB of the 27,168 B budget, so
the count is affordable but not free.

Rows marked "host-native" are capabilities the Muse binary already provides to every user without
omm; omm is credited HAVE only where it adds a command, a check, a ledger entry or a lint on top,
and PARTIAL where it merely does not get in the way.

## 2. Method

1. Each rival inventory row was reduced to the capability it evidences; the same capability from
   several rivals became one row listing all of them with their counts. Rows whose feature text was
   a defect ("DEFECT", "REFUTED", "NOT implemented", "NONE", "no …") were dropped — an absence in a
   rival is not something omm can have or lack.
2. omm was classified per row from its own inventory (53 rows, every cell verified against the tree
   at `60931a2`) and, where the inventory was silent, from a direct read of the file named in the
   evidence column.
3. IMPOSSIBLE_ON_MUSE needs a PROVEN dead-end row; OUT_OF_SCOPE_BY_DESIGN needs a MATRIX §4 antipattern
   id, a §6 requirement, or a DECISION §0/§4 strike. Everything else buildable is GAP, however low its
   value.

Rival keys: **OMC** oh-my-claudecode 5.1.0 · **OMX** oh-my-codex 0.21.2 · **OMO** oh-my-openagent /
oh-my-opencode · **OMP** oh-my-pi 18.1.0 · **SLIM** oh-my-opencode-slim 2.2.18 · **OMA** oh-my-agent
13.1.1 · **OMH** rlaope/oh-my-hermes 2.0.0.

## 3. The matrix

Status column values: `HAVE` · `PARTIAL` · `GAP` · `IMPOSSIBLE_ON_MUSE` · `OUT_OF_SCOPE_BY_DESIGN`.

### 3.1 Content — skills

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 1 | Bundled workflow skills as SKILL.md directories | OMH 114 · OMC 35 · OMA 33 · OMX 29 (23 active) · OMO 17 shared +10 senpi +11 codex · SLIM 8 · OMP 0 | PARTIAL | `content/catalog.json` 12 `kind=skill`; `content/skills/**`; plugin manifest `capabilities.skills[12]` | 12 vs 29–114. HAVE-pending: 23 in flight (footnote §1). Budget check per skill is `omm cost`; descriptions must survive `first_sentence` |
| 2 | Skill reference sub-documents beside the body | OMH 55 · OMA 255 support files · OMC lib/templates/phases | HAVE | 18 `references/*.md` under `content/skills/*/` | bodies capped at 8,192 B by lint; overflow goes to references |
| 3 | Routed / conditional skills invisible to the always-loaded catalog | OMH `ulw-` triggers · OMC learned skills (cap 10/session) · OMP managed-skills | PARTIAL | `content/routing/library/` 7 skills, `omm enable skill-routing` | opt-in only: two experimental env gates + per-workspace copy (DECISION §4 "must stay opt-in") |
| 4 | Asset catalog with lifecycle status (active / deprecated / merged / alias), canonical forwarding, sunset date | OMX 33 rows · OMH 86 `SkillDefinition` | HAVE | `content/catalog.json` fields `lifecycle`, `core`, `canonical`, `since`, `sunset` (M10) | no rule in `lint.rs` yet fails the build when a `core: true` asset is deactivated — the field is data, the gate is missing |
| 5 | One-release sunset stubs for retired skills | OMX (`ralph` → `ultragoal`) | PARTIAL | catalog `sunset` / `canonical` fields exist; no retired asset yet, no stub generator | first deprecation will need `omm build` to emit the stub body |
| 6 | In-band provenance badge on every installed skill | OMX `[OMX] ` prefix | HAVE | host lists plugin skills as `plugin:omm:<id>` (README install transcript); `omm list` provider column | host-native id namespace; no string prefix needed |
| 7 | User-authored / learned skills discovered from user directories | OMC 4 roots · OMX 3 roots · OMP `.omp/skills` + managed-skills · OMH imported/ | PARTIAL | host reads `$CONFIG_DIR/skills`, `~/.agents/skills`, `~/.claude/skills`, `~/.codex/skills` natively (SYNTHESIS §2.1 #1–2); `$OMM/custom/skills/<id>/` overlay | overlay skills resolve in `omm list` but are **not packaged** into the installed plugin (`overlay.rs` header; README "Your own content") |
| 8 | Skill-authoring loop (`/skillify`, `/skill add`, `oma skills opt`) | OMC · OMX (prose) · OMA · SLIM `reflect` | PARTIAL | `omm-reflect` proposes a skill for repeated procedures (prose); no authoring command | a `omm skill new <id>` scaffold + lint is a day; the packaging limit in row 7 is the real blocker |
| 9 | Skill lint / validate toolchain with stable rule ids | OMA `skills lint/audit` · OMX `/skill validate` (prose only) · OMC | HAVE | `omm lint [PATH] [--no-host]`, 42 rule ids in `crates/omm-manifest/src/lint.rs` + `muse skills validate` / `plugins validate` checkpoints | stronger than any rival: host validators run as part of the lint |
| 10 | Skill eval fixture suites / benchmark missions | OMA 20 eval files, 3 suites · OMX 13 missions | GAP | none | N8; the mock-provider harness (`tools/mockprovider/`) is the substrate — a `omm eval <skill>` over scripted plans is ~1 week |
| 11 | Per-language skill variants promoted at install | OMA 27 `variants/` files | GAP | none; the in-flight `rust typescript python` skills are separate assets, not variants | low value; a `variants/<lang>` selector in `catalog.json` + `omm install --stack` |
| 12 | Skill install profiles / presets selecting which skills land | OMH `core`/`full` · OMA 10 presets · OMX catalog filter | PARTIAL | `omm install --skip <STEP>` (themes/rules/profile/trust) and `config.json → disabled[]`; no skill-set preset | a `profile.skills[]` selector honoured by `omm build`/install: 2–3 days |
| 13 | Compact shims / description caps for context economy | OMC ≤240-char stubs · S4 | HAVE | lint `skill-description-length` (≤240) + `skill-negative-trigger`; profile `default` sets `first_sentence` | measured 4,627 B for 12 skills (`omm cost`) |
| 14 | Self-configuring skill teaching the agent the framework's own schema | SLIM `oh-my-opencode-slim/SKILL.md` · S11 | HAVE | `content/skills/omm-self/` (+ `references/ledger.md`) | |
| 15 | Reflect skill mining session friction | SLIM `reflect` (reads host SQLite) · OMC `/skillify` | PARTIAL | `content/skills/omm-reflect/SKILL.md` — persists lessons via `add_memory`/`edit_memory`, proposes skills | prose only; does not parse `session.jsonl`. A `omm sessions grep` MCP tool would make it real |
| 16 | Vendoring third-party skill repos (submodules) with a provenance table | OMO 4 submodules · OMH `SKILL-SOURCES.md` 13 rows | GAP | `omm-skill-port` + `content/translation/muse.md` port one skill at a time | N8: `docs/SKILL-SOURCES.md` with `reviewed_ref` + a drift diff in CI: 2 days |
| 17 | Skill frontmatter carrying per-skill tool / MCP config | OMO 4 frontmatter fields (`tools`, `mcpConfig`) | IMPOSSIBLE_ON_MUSE | — | `allowed-tools` in SKILL.md is parsed and explicitly **not enforced** (SYNTHESIS §2.4) |

### 3.2 Content — agents and prompts

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 18 | Bundled subagent roster (personas) | OMX 28 TOML · OMH 21 role files · OMC 19 · OMA 12 · OMO 10 · SLIM 9 · OMP 7 | GAP | none — `omm list --kind agent` accepts the kind, catalog has no agent asset | buildable only as `$CONFIG_DIR/agents/**/*.md` or a plugin top-level `agents/` dir; composes in the TUI only, never under `muse exec`; native manifest `capabilities.agents` is a hard error (SYNTHESIS §2.2 #15, §2.4). A 6-persona pack + lint on the `[a-z]+(-[a-z]+)*` name grammar (one bad name kills the whole overlay): 3–4 days |
| 19 | Agent role prompts shipped as editable files | OMX 32 `prompts/*.md` · OMH 9 `roles/*.md` | GAP | none | same delivery path as row 18 |
| 20 | User-defined agents without forking | SLIM `agents.*` keys · OMO 4 roots · OMP `.omp/agents/` | PARTIAL | host-native: `$CONFIG_DIR/agents/`, `<ws>/.agents/agents/` (trust-gated) | omm adds no lint, list or ledger for them |
| 21 | Vendor-neutral agent definitions compiled to host-native formats | OMA `agent-compose` 7 vendors · OMO Codex TOMLs | OUT_OF_SCOPE_BY_DESIGN | — | single host (MATRIX §6 M14, DECISION §2.5: "author once" holds for content, breaks for hooks) |
| 22 | External agent binaries as subagents (ACP) | SLIM `acpAgents` | OUT_OF_SCOPE_BY_DESIGN | — | Muse ships `subagent_*` natively; no in-process plugin execution (M17) |
| 23 | Operating models / team profile packs | OMH 4 models, 4 packs, 17 roles | OUT_OF_SCOPE_BY_DESIGN | — | no multi-agent orchestration product (DECISION §0; MATRIX A18) |
| 24 | Disabling the host's own default agents so the framework owns delegation | SLIM install step 5 | OUT_OF_SCOPE_BY_DESIGN | — | never patches or disables host behaviour (README "deliberately does not do") |
| 25 | Output styles / selectable personalities | OMP 3 personalities · OMC 0 · OMA 0 | IMPOSSIBLE_ON_MUSE | — | `outputStyles` is a hard `unsupported-capability`; no personality surface in the 29 settings keys (SYNTHESIS §2.4). Nearest: text in the AGENTS.md managed block |
| 26 | Declarative model-side reminder agent | none of the seven | HAVE | `content/reminders/omm-verify.json` (`enabledDefault:false`) | Muse-only surface (SYNTHESIS §2.1 #13); see §4 |

### 3.3 Content — slash commands and CLI

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 27 | Slash commands shipped as prompt files | OMP 77 built-in + user `.md` · OMC 21 · OMA 21 workflows · OMO 7+10 · OMX 7 shortcut skills · SLIM 4 | PARTIAL | `content/commands/omm-{doctor,cost,status}.md` — 3 | 3 vs 7–77. Muse invokes skills directly via `read_skill`/selectors, so shim-per-skill is unnecessary; but planning/handoff/PR commands are missing (row 60) |
| 28 | Command → skill dispatch shims for context economy | OMC 21 shims ≈ 13 KB | PARTIAL | not needed for bundled skills (host selectors); omm's 3 commands shell out to `omm … --json` | only bundled skills with an invocation marker appear as slash commands (SYNTHESIS §2.3 #27); plugin skills do not — a shim per skill would cost catalog bytes the budget cannot spare |
| 29 | Project-local user markdown commands with `$ARGUMENTS` | OMP `.omp/commands/*.md` · OMO ingests `.claude/commands` | PARTIAL | `$OMM/custom/commands/<id>.md` overlay resolves in `omm list`; `$ARGUMENTS` supported by the host in plugin commands | not packaged into the plugin (row 7); host has no project commands dir (commands only via plugin `capabilities.commands[]`) |
| 30 | Broad framework CLI verb surface | OMH 168 · OMX 49 · OMA 38 groups · OMC 26 · SLIM 2 | HAVE | 21 top-level subcommands (`omm --help`; `crates/omm/src/cli.rs`) | `--json` on every read verb, `--dry-run` on every write verb |
| 31 | Ingesting another host's slash commands | OMO `claude-code-command-loader` | GAP | none | a build-time `omm import commands --from claude` into `custom/commands/`: 1–2 days, blocked on row 7 packaging |

### 3.4 Content — hooks

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 32 | Hook fleet breadth (events covered) | OMO 56 hooks · OMP 41 events · OMC 24 scripts / 11 events · OMA 23 handlers · SLIM 14 · OMX 1 script × 7 events · OMH 6 | PARTIAL | 3 plugin hooks on 3 of Muse's 17 events (`SessionStart`, `PreToolUse`, `Stop`) + project-tier `route` on `UserPromptSubmit` | 14 events unused (`PostToolUse`, `PreCompact`, `SubagentStop`, `PostToolUseFailure`, …). Each handler is a Rust fn in `hook.rs`; the dispatcher is done |
| 33 | One dispatcher process per event, fail-open, CI latency budget | OMA (`oma-hook.sh`, p50 624 ms, SLO 1500 ms) · OMX · OMC `run.cjs` | HAVE | `omm hook <NAME>`: JSON in / JSON out, exit 0 on any failure, 5 ms cold (`tests/hook_fail_open.rs`, e2e s09) | M15 met; two orders of magnitude under OMA |
| 34 | Foreign-hook coexistence when writing shared hook config | OMC `mergeHookGroups` · OMA marker groups · OMX shared-ownership | HAVE | plugin-tier hooks touch no shared file; `<ws>/.muse/hooks.json` merged with pre-omm bytes + mode kept (`routing_enable.rs`, `tests/routing.rs`) | |
| 35 | Destructive-command guard | OMA `scm-guard` · OMC `pre-tool-enforcer` | HAVE | `omm-guard`: 5 rules, 16 wrapper words stripped (`content/hooks/README.md`) | documented as heuristic, not a boundary (KNOWN_ISSUES #4) |
| 36 | Continuation / persistence enforcement around the loop | OMC 9 hooks (`persistent-mode`, `workflow-drift-guard`, `verify-deliverables`…) · OMA `persistent-mode` · OMO `stop-continuation` | PARTIAL | `omm-stop` one `decision: block` per turn + `omm-verify-nudge` reminder | no drift guard, no artifact verifier (row 61); by design the nudge is single-shot |
| 37 | Keyword / trigger detector on the user prompt | OMC `keyword-detector` · OMX 4,426-line module · OMA | PARTIAL | `omm hook route` scores `metadata.triggers`, top 3 (`routing.rs`) | opt-in, two experimental gates, `skills.v1` only — cannot trigger arbitrary behaviour, only route skills |
| 38 | Glob-matched rules injector (path-conditional guidance) | OMO 10 roots · OMC 3 subdirs · OMA `skill-injector` · OMP 27 built-in rules | GAP | none — Muse rules are whole `AGENTS.md` files that accumulate, no globs | a `PreToolUse`/`UserPromptSubmit` handler emitting `additionalContext` from `custom/rules/*.md` frontmatter globs: 3 days |
| 39 | Post-edit diagnostics (LSP / linter) hook | OMO LSP `PostToolUse` · OMA `test-filter` | GAP | none | `PostToolUse` handler running a configured linter command and returning its first lines: 2 days; no LSP (row 50) |
| 40 | User drop-in hook plugins with an SDK | OMX `.omx/hooks/*.mjs` + `onHookEvent` SDK · OMP `pi.on(event)` | PARTIAL | host-native tiers: `settings.json → hooks`, `<ws>/.muse/hooks.json` (any executable); `$OMM/custom/hooks/*.json` overlay unpackaged | omm offers no SDK; the host's JSON stdin/stdout contract is the SDK (M17) |
| 41 | Hook integrity / trust digests | OMH `hook_integrity.py` · OMO canonical SHA-256 · OMX trust state in `config.toml` | HAVE | host binds every capability to `package_sha256` (`trusted_definition_hash`); doctor D1 asserts literal `trusted_enabled` | host-native trust + omm's verify (R14) |
| 42 | Hook kill switch | OMC `DISABLE_OMC`, `OMC_SKIP_HOOKS` (9/24 honoured) · OMX `OMX_HOOK_PLUGINS=0` (refuted) | PARTIAL | `content/hooks/README.md:126` documents `config.json → "disabled": ["hook:omm-guard"]` | **not enforced on the dispatch path** — `hook.rs` never reads `OmmConfig.disabled` (grep 2026-09-05); same shape as OMX's refuted switch. Fix: one config read in the dispatcher, ≤ 1 day |
| 43 | Compaction-lifecycle hooks | OMC `pre-compact`, `context-guard-stop` · OMX `PreCompact`/`PostCompact` | GAP | none; events exist (`docs/host-data/hook-events.json`) | a `PreCompact` handler injecting the memory pointer: 1 day |
| 44 | Notification hook | OMX setup step 7 | GAP | none | host `Notification` event + `settings.notifications` typed keys; 1 day, low value |
| 45 | Git hooks (commit-msg guard) | OMA `.githooks/commit-msg` | GAP | none; `omm-commit` skill is prose | out of the ledger's bases today (repo `.git/hooks` is not an allowlisted base); 1 day plus a containment decision |
| 46 | Versioned hook event envelope, derived heuristic events | OMX `schema_version "1"`, `DERIVED_EVENTS` | OUT_OF_SCOPE_BY_DESIGN | — | the host's 17 PascalCase events are the envelope; omm re-measures them in hostcheck rather than inventing a second one (M16) |

### 3.5 Content — rules, prompts, reminders

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 47 | Starter rule packs | OMP 27 built-in (Go/Rust/TS) · OMA 13 · OMC 7 templates · OMX managed AGENTS.md 229 lines | PARTIAL | one asset, `content/rules/AGENTS.md.tmpl`, 10 bullets | Muse loads exactly one personal rules file (SYNTHESIS §2.1 #4) so packs must be composed into that block; `custom/rules/<id>_append.md` exists. Language packs: 2 days each |
| 48 | Marker-delimited managed block in the user's rules file with a preserved user region | OMC `OMC:START/END` · OMX 6 marker pairs · OMA `OMA:START/END` | HAVE | `<!-- omm:managed-start/end -->` + `<!-- omm:user-start/end -->`; e2e s14, s23 | omm owns only the block (Gate 1) |
| 49 | Foreign rules import (`.cursorrules`, `.clinerules`, `copilot-instructions`) | OMH 5 formats · OMC/OMO/OMP ingest Cursor/Copilot | GAP | host reads `~/.claude/CLAUDE.md`, `~/.codex/AGENTS.md` natively; nothing reads Cursor/Cline/Windsurf | S1: `omm import rules --from cursor` into the managed block or `custom/rules/`: 2 days, plus the injection/credential refusal of row 138 |
| 50 | Model-instruction template variants | OMX 2 variants | PARTIAL | one template; profiles change settings, not the rules text | a `rules.variant` per profile was in PLAN 2.4 and did not ship (`profile.rs` header) |

### 3.6 MCP servers and model tools

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 51 | First-party MCP server(s) exposing tools to the model | OMC 1 server ≈ 56 tools · OMX 6 servers (all disabled) · OMO 5 in-plugin + 3 standalone · OMH 1 + 14 plugin tools · SLIM 2 built-in MCPs | PARTIAL | `omm mcp` — 1 server (`doctor`), 2 tools (`omm_doctor`, `omm_cost`); proven end to end (`tests/mcp_server.rs`) | the server is done; the tool count is 2 vs 56 |
| 52 | LSP / AST-grep / DAP tooling for the agent | OMP 54 LSP + 14 DAP presets · OMC `lsp_*` ×12 + `ast_grep_*` · OMO `ast-grep-mcp`, `lsp-tools-mcp` · SLIM `ast_grep_*` | GAP | none; host `lspServers` manifest key is warn-and-ignore | an `ast_grep_search/replace` pair inside `omm mcp` shelling to a user-installed `sg` (never provisioned, M15): 1 week. LSP is a month and belongs to the host |
| 53 | Memory / notepad / wiki tools | OMC 20 tools · OMX `omx_memory`, `omx_wiki` (disabled) · OMH provider | PARTIAL | host-native `read_memory`/`add_memory`/`edit_memory`; `omm memory path/seed/list/backup/gc` CLI | no notepad, wiki or shared-memory store; omm keeps no store of its own (row 118) |
| 54 | Web fetch / web search tools | SLIM `webfetch` (jsdom) · OMO `websearch` · OMP built-ins | HAVE | host-native `web_search`, `web_fetch` in `run.toolset` (SYNTHESIS §2.3 #34) | nothing for omm to add |
| 55 | Background-task control tools (`task_status`, `task_cancel`, `wait_for_user`) | SLIM 5 + 1 · OMP hidden tools | OUT_OF_SCOPE_BY_DESIGN | — | Muse `subagent_*` (6 tools, trusted workspace) covers it (DECISION §0) |
| 56 | Session search / merge-readiness tools | OMC `session_search`, `merge_readiness_*` ×5 | GAP | none | `session.jsonl` is on disk (`host-reality.md` Paths); a `omm_sessions_grep` tool: 3 days |
| 57 | Python REPL / trace tools | OMC `python_repl`, `trace_*` | GAP | none | low value; a REPL is a runtime the binary must not provision (M15) |
| 58 | Provisioning third-party runtimes and binaries on install (`sg`, node, serena) | OMO 2 runtimes · SLIM `sg` (unverified) · OMA bun/uv/serena | OUT_OF_SCOPE_BY_DESIGN | — | M15 "no runtime provisioning"; MATRIX §7.2 |
| 59 | Unified MCP registry seeded / projected into host configs | OMC `mcp-registry.json` → 2 hosts · OMA 4 mcp.json · OMO ingests `.claude` MCP config | PARTIAL | `omm settings set mcpServers.<id> <json>` (typed, validated), `fix-mcp-collision` | no registry file, no `omm mcp add`; host destroys the legacy `mcp_servers` key so a writer must be typed (R9) |
| 60 | In-session preset / profile switch tool | SLIM `/preset` tool | GAP | `omm profile use` is CLI-only | expose as a third `omm mcp` tool: 1 day |

### 3.7 Orchestration and autonomy

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 61 | Multi-agent team / orchestrator runtime | OMC `team` (1.6 MB of bundles) · OMX `team/runtime.ts` 7,127 lines · SLIM orchestrator + 8 · OMO sisyphus/atlas · OMH packs | OUT_OF_SCOPE_BY_DESIGN | `omm-parallel` is prose over native `subagent_spawn`/`wait` | DECISION §0: Muse ships `subagent_*` + `workflow`; MATRIX A18 (framework absorbs the host) |
| 62 | Autonomy loops (ralph / ultragoal / autopilot / deepwork / loop) | OMX 4 core skills · OMC autopilot 4 stage sequences · OMO ULW · OMH 9 `ulw-` · OMA ralph/ultrawork · SLIM deepwork/loop | OUT_OF_SCOPE_BY_DESIGN | — | same; the host's `workflow` tool + cookbook (18 KB) is the loop controller. A prose "long-running work mode" skill would be a GAP of 1 day if wanted |
| 63 | Planning / handoff / interview commands | OMO `hyperplan goal handoff` · OMX `ralplan` · SLIM `/interview` · OMH `ulw-plan/interview` | GAP | none shipped; `handoff` is among the 23 in flight | prose skills; 1 day each |
| 64 | Workflow stop-gate with artifact verification and durable event log | OMA `artifact-verifier`, 13 event kinds | PARTIAL | `omm-stop` checks the *message* for a cited run, not the *artifacts*; `audit.log` records omm's writes, not the agent's | an artifact check (`git status`, test log present) in the Stop handler: 2 days |
| 65 | Terminal-multiplexer adapters / `tmux send-keys` injection | SLIM 5 adapters · OMX/OMO tmux SDK | OUT_OF_SCOPE_BY_DESIGN | — | workaround for hosts without an input channel; Muse has MSP and session messaging (MATRIX §7.3) |
| 66 | Desktop companion / menubar / TUI sidebar / VS Code extension / HTTP dashboard | SLIM Rust GUI + sidebar · OMH LaunchAgent · OMX VS Code pkg · SLIM interview dashboard | OUT_OF_SCOPE_BY_DESIGN | — | framework, not agent (MATRIX §1.2, A18) |
| 67 | Council / multi-model deliberation | SLIM `council` + `councillor` | OUT_OF_SCOPE_BY_DESIGN | — | same |
| 68 | Per-agent orchestration knobs in config (`max_depth`, `allowed_subagents`, fallback chain) | OMO 14 schema fields | OUT_OF_SCOPE_BY_DESIGN | — | no agent roster; host `agent_definitions` frontmatter owns it |
| 69 | Delegation routing to external coders (codex/claude) | OMH `category-maestro.json` | OUT_OF_SCOPE_BY_DESIGN | — | same |
| 70 | Benchmark / eval mission suites | OMX 13 missions · OMA harness | GAP | none | see row 10 |

### 3.8 Statusline, themes, keymaps

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 71 | Statusline / HUD | OMC `omc-hud.mjs` · OMX `[tui].status_line` + presets · OMA `hud.ts` · OMH TUI widget · SLIM TUI sidebar | IMPOSSIBLE_ON_MUSE | substitutes: `/omm-status`, `omm doctor`, SessionStart context line (model-visible only) | no `statusline`/`prompt`/`format` key in 29 settings keys or 14 `tui` fields (SYNTHESIS §2.4, PROVEN) |
| 72 | TUI colour themes / skins | OMP 98 JSON · OMH 4 skins · others 0 | HAVE | 3 `.tmTheme` (`omm-carbon`, `omm-slate`, `omm-paper`), `omm theme <NAME>`, prior restored on uninstall (e2e s07) | 3 vs 98 is a content count, not a capability gap |
| 73 | Keybinding presets | OMP `keybindings.*` file | PARTIAL | `omm keymap <PRESET>` validates and ledgers `tui.keymap`; **0 bundled presets** beyond `default` | one or two presets over the 37 host actions: 1 day |

### 3.9 Profiles and configuration

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 74 | Settings profiles / presets switchable by command | OMO profiles · SLIM presets · OMP `--profile` · N5 | HAVE | `omm profile use \| list \| show` — 4 profiles as ledgered typed slices (e2e s19) | |
| 75 | Layered framework config with a documented precedence chain (user ← project) | SLIM 6 layers · OMC 2 + env · OMO walk-up · OMP | GAP | `$XDG_CONFIG_HOME/omm/config.json` only; per-workspace `.omm/routing.json` for routing | a `<ws>/.omm/config.json` layer (disabled, profile) merged under the user file: 2 days. The host itself has no workspace settings file (SYNTHESIS §2.4) — this is omm's own layer |
| 76 | Strict config schema (unknown keys rejected) and an editor JSON Schema | OMO 55 `.strict()` · SLIM 1,435-line schema · OMP typed | PARTIAL | ledger and profile documents are `deny_unknown_fields` (`schema.rs`); `config.json` is a free `Value` map (`OmmConfig { doc }`) | typing `OmmConfig` + emitting a `$schema`: 1 day |
| 77 | Model presets / mixing / routing categories | OMH 9 categories + chains · SLIM 5 mappings · OMA 7 presets · OMC 3 tiers · OMO per-agent `models[]` | GAP | none — no profile sets provider or model (`content/profiles/README.md`) | host `settings.presets.<name>` is native (SYNTHESIS §2.2 #17); a `presets/` asset kind is 2 days, but every preset is untestable offline (echo provider only) |
| 78 | Relocatable roots via env (`*_HOME`, XDG) | OMC 4 families · OMH 2 anchors · OMX · OMP `init-xdg` | HAVE | XDG roots only (host reads exactly two candidates); `OMM_SOURCE`, `OMM_MUSE_BIN`, `OMM_THEMES_DIR` | |
| 79 | `--scope user \| project` with an upward-resolved scope marker | OMX `resolveNearestPersistedSetupScope` · OMP per-scope plugins · OMA | GAP | user scope only; workspace is trust + routing | S9: host `plugins install --scope project` exists; `.omm/` marker walk-up + a second ledger base: 1 week |
| 80 | Subtract-only capability policy / `disabled_*` lists / per-hook toggles | OMH 6 families · SLIM 4 lists · OMO 56 toggles · OMP `disabledExtensions` | HAVE | `config.json → disabled: ["skill:…","hook:…"]` capability-qualified (M8) | enforcement caveat in row 42 |
| 81 | Fail-closed / quarantine on invalid config instead of silent repair | OMP `.broken-<ts>` · OMO `.strict()` | HAVE | ledger → `omm.lock.json.bad-<ts>`; malformed host `settings.json`/`trust.json` refused by the plan (e2e s27) | S10 |
| 82 | Typed per-plugin settings schema, optional plugin features | OMP `settings`/`features` in manifest | OUT_OF_SCOPE_BY_DESIGN | — | no third-party plugin protocol inside omm (M17); Muse's plugin manifest has no `settings` (hard error) |
| 83 | `.gitignore` management for project state (ignore runtime, un-ignore declarative) | OMX 6 patterns · OMA 7 patterns | GAP | none — `omm enable skill-routing` writes `<ws>/.omm/` and `<ws>/.muse/hooks.json` and ignores nothing (grep `gitignore` in `crates/` → 0) | S13: two ledgered lines, 1 day |
| 84 | Per-skill settings overrides in the config file | OMA 8 skills | GAP | none | low value; skills are prose |
| 85 | Interactive install presets | OMA 10 presets | PARTIAL | `--profile`, `--skip`, `--workspace`; non-interactive by design (R6) | |
| 86 | Config-migration engine (applied ids, journal, lease lock, resume) | OMO 6 modules · OMA 24 migrations | GAP | none needed at 0.1.0; `schema_version` fields exist in ledger docs | S12: needed before the first breaking ledger change; 1 week |
| 87 | Persisted setup preferences replayed on update | OMX `.omx/setup-scope.json` | PARTIAL | `config.json → profile` persists; `--skip`/`--workspace` choices are not recorded | S5: add them to the ledger's install record: 1 day |

### 3.10 Install

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 88 | `curl \| sh` installer placing exactly one binary, checksum-verified | OMP (1 file, smoke `--version`) · OMH (venv + symlink) · OMA (bun/uv/serena first) | HAVE | `install.sh`: SHA256SUMS verify, one file, `--dry-run`, `--modify-path` opt-in (e2e s32) | release URL is a placeholder until the first tag — today it is "build from clone" |
| 89 | Multi-channel distribution (brew, npm/bun, nix, mise, docker, apm, GitHub Action) | OMP 6 · OMA 6 · OMH 5 · OMO 3 · OMC 2 | PARTIAL | 1 channel (tarball via `install.sh`) + the git marketplace `ohmy` for the plugin | M19 namespace claim (npm `oh-my-musecode`, brew tap) is unmet; N6. Tap + npm launcher: 3 days after the first tag |
| 90 | Install ≠ setup: two phases, setup refuses under CI / non-TTY | OMH `OMH_RUN_SETUP=0` · OMX near-inert postinstall | HAVE | phase 1 `install.sh`, phase 2 `omm install` with R6 gate (e2e s10) | M7 |
| 91 | Plan-then-execute with preflight (containment, writability, host config validity), `--dry-run` | OMX 8-step + compensating transaction · SLIM 10 guarded steps · OMA incident guardrails | HAVE | plan validated before the first host mutation (Gate 1); e2e s15/s15b/s15c (interrupted install undone) | |
| 92 | Atomic write + verified backup on every host-config mutation | SLIM `.bak` · OMO `.bak.<ISO>` · OMC O_EXCL transaction · OMX backups mirror | HAVE | realpath-resolved temp + rename, verified backup, rolling snapshots (`omm-host` patcher; Gate 0) | pre-omm bytes **and mode** restored (e2e s21) |
| 93 | Non-destructive by default; explicit `--reset`/`--force` | SLIM · OMX `--force` | HAVE | second install is a no-op; edits staged; `--force` only on uninstall/profile | |
| 94 | Capability negotiation against the installed host binary before writing | OMX `codex features list` probe · M16 | HAVE | gate probe from the bootstrap trace, `hostcheck` 62 rows, host config validity in the plan | |
| 95 | Post-install register smoke test | OMH plugin import smoke · OMX step 5.5 | HAVE | every capability verified literally `trusted_enabled`; `skills list` cross-check | R14 |
| 96 | Shell-profile edit as an idempotent marker block | SLIM `>>> … <<<` | PARTIAL | `--modify-path` appends one PATH line, opt-in only | no marker block, no idempotency check on re-run (install.sh §4) |
| 97 | Windows support | OMA `install.ps1` + junction fallback · OMP `install.ps1` · OMO PowerShell dispatch | GAP | none: release targets are 2 darwin + 2 linux-musl (`scripts/release.sh`) | the binary would cross-compile, but native plugin hooks **reject `commandWindows`** (host-reality "Windows"; ARCHITECTURE R16 open) so plugin-tier hooks cannot run there. Partially IMPOSSIBLE until the host accepts the field |
| 98 | Name-collision avoidance with host-native commands | OMC `toSafeStandaloneSkillName` (rename) | HAVE | lint refuses collisions with 39 slash commands, 15 bundled skills, 9 reserved ids (`id-slash-collision`, …) | refuse at build time rather than rename at install |
| 99 | Distribution through another host's plugin marketplace | OMA `/plugin install oma@oh-my-agent` bootstrap · OMC marketplace | PARTIAL | `dist/claude/.claude-plugin/` and `dist/codex/.codex-plugin/` generated and drift-gated | no test installs them into Claude Code or Codex (inventory row 40) |
| 100 | Agent-runnable install page | OMO README ("let an LLM install it") | HAVE | `docs/INSTALL_FOR_AGENTS.md` (flags, `--json` error shape, verification) | S11 |
| 101 | Interpreter discovery / node pinning / runtime version gates | OMC `resolveNodeBinary`, Node 20 gate · OMA bun · OMO node runtime | OUT_OF_SCOPE_BY_DESIGN | — | one static binary (MATRIX §7.2) |
| 102 | Prompt to star the repo with the user's `gh` credential | SLIM · OMO | OUT_OF_SCOPE_BY_DESIGN | — | A21 |
| 103 | Telemetry on by default | OMO PostHog | OUT_OF_SCOPE_BY_DESIGN | `ci` profile turns the *host's* telemetry off | A20 spirit; omm emits none |
| 104 | Offer to uninstall competing tools | OMA `promptUninstallCompetitors` | OUT_OF_SCOPE_BY_DESIGN | — | A20 (touching things that are not yours) |
| 105 | Injecting an autonomy directive into the user's rules by default | OMX `templates/AGENTS.md` | OUT_OF_SCOPE_BY_DESIGN | managed block is 10 conservative bullets | A19 |

### 3.11 Update

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 106 | Self-update of the binary detecting the owning installer | OMP 6 methods · OMH · OMX · OMC · OMA | GAP | `omm update` refreshes the *bundle*; the binary is re-installed by re-running `install.sh` | S6: `current_exe()` + `install-provenance.json` (named in `omm-self/references/ledger.md` and `uninstall.rs:394` but never written): 3 days |
| 107 | Launch-time / background update check, deferred install | SLIM (on by default) · OMX 12 h throttle, deferred worker · OMC opt-in | GAP | none | must be opt-in and deferred (A23, S6); a `SessionStart` check against the release manifest: 2 days |
| 108 | Three-way merge with recorded ancestor; conflicts staged, never clobbered | SLIM `skill-sync.ts` (the only rival) | HAVE | ancestor = ledger sha; no-op / overwrite / adopt / stage under `$OMM/updates/<version>/` (e2e s03, s03b) | M3, M4 |
| 109 | Repair command for post-update state | OMC `update-reconcile` | HAVE | `omm reconcile` (doctor D13's fix; e2e s20) | |
| 110 | Downgrade guard | OMC version marker check | GAP | none | compare ledger `source_version` with the incoming bundle: ½ day |
| 111 | Conservative pruning: delete only what the manifest proves you wrote | OMH retain-and-report · OMA `oma-` prefix filter | HAVE | reconcile drops vanished entries, adopts identical; nothing outside the ledger is ever deleted except via `--reconcile-host` | |
| 112 | Upgrade path exercised in CI against a real host | OMC `upgrade-test.yml` | HAVE | e2e update scenarios run in `scripts/gate.sh` against the pinned host on every push (`ci.yml` `gate`) | |
| 113 | Release gate: versions must agree across every artifact; generated mirrors drift-checked | OMH 4 workflows · OMX `prepack` chain · OMA drift gate | HAVE | `scripts/release.sh` bump → regenerate → gate; `omm build --check` in CI | never tags or pushes |
| 114 | Historical-hash adoption for users who installed before ancestors were recorded | SLIM `LEGACY_MANAGED_SKILL_HASHES` (empty) · OMC 202 records | OUT_OF_SCOPE_BY_DESIGN | — | unnecessary when the ancestor is recorded from the first write (M1, M3) |

### 3.12 Uninstall

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 115 | Uninstall as a manifest traversal with a two-section preview and `--dry-run` | OMH (best rival) · OMX 6-step transaction · OMA `buildRemovalPlan` · OMO Codex only · OMC/OMP/SLIM none | HAVE | `omm uninstall [--force] [--reconcile-host]`: remove / preserve / "beyond the ledger", deepest-first, contained | M5 |
| 116 | Deliberate residue named per channel | OMH removal table · OMX residue list | HAVE | preview names kept residue; ARCHITECTURE §2 footprint; `session-name-authority` named | |
| 117 | Byte-identical filesystem after install → uninstall, asserted in CI | none of the seven | HAVE | e2e s01/s02 (both install paths), s04, s16, s21 | M6; see §4 |
| 118 | Maintenance sweeper for caches and dead state | OMP `gc` | HAVE | `omm memory gc`; runtime-dir residue sweep (`residue::sweep_dead`) | scoped narrower than OMP's |

### 3.13 Ledger and lockfile

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 119 | Per-file SHA-256 install manifest recording what the installer wrote | OMH `manifest.json` · OMX receipts · SLIM skills-manifest · OMC `.omc-managed` sentinels · OMA `_version.json` (no per-file) | HAVE | `$OMM/omm.lock.json` (`crates/omm-ledger`), entry saved before the write (e2e s26) | M1, M2; every verb consumes it (contrast A4) |
| 120 | Separate manifest so user-imported content can never be pruned | OMH `skills-import-manifest.json` | HAVE | user content under `custom/` is never ledgered, so never a removal candidate | structural rather than a second file |
| 121 | Digest of the whole capability surface as a drift lock | OMC `capabilities lock` (no consumers) · OMX `omx-capabilities.lock.json` (CI) | HAVE | `omm build --check` + `catalog.json` + marketplace `integrity.digest` = `package_sha256` | consumed by CI and by `omm install` (digest asserted) |
| 122 | Installer concurrency lock with stale-owner handling | OMA PID lock · OMO PID+lease · SLIM mkdir lock · OMH | HAVE | `$OMM/locks/muse-config.lock` held from stale check through rename (Gate 0 round 2) | |
| 123 | Dependency lockfile for third-party plugins | OMP 3-file split | GAP | none — omm manages no third-party content | arrives only with a registry (row 128) |
| 124 | Install-time decision records read back by uninstall | OMO `.installed-agents.json`, `.installed-bin-dir.json` | HAVE | ledger registrations (marketplace, plugin id + `package_sha256`), settings priors, trust prior | S5 |

### 3.14 Override and overlay

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 125 | Overlay directory searched before bundled content (shadow by name) | OMP 16-constant ladder · OMO 7 roots · SLIM 4-level prompt search | PARTIAL | `$OMM/custom/<kind>/<id>` replaces; `_shadowed` retained; `omm list` columns (`overlay.rs`) | resolution is complete; **delivery is not** — skills/commands/hooks in `custom/` are not packaged into the installed plugin. Closing it: `omm build --user` producing a second local plugin `omm-custom`, ~1 week |
| 126 | Append channel alongside replace | SLIM `<agent>_append.md` | PARTIAL | `custom/<kind>/<id>_append.md` for skills, commands, rules | same delivery limit; rules append does land (AGENTS.md block is written by omm) |
| 127 | Capability-qualified disable ids | OMP `disabledExtensions` · OMO per-hook · SLIM `disabled_*` | HAVE | `disabled: ["skill:omm-x", "hook:omm-guard"]` | runtime enforcement gap for hooks: row 42 |
| 128 | Disable an entire content source / provider | OMP `disabledProviders` | PARTIAL | host-native `context.foreign_personal_skills: false` for foreign roots; no omm switch | 1 day if wanted |
| 129 | Mandatory provenance on every loaded item; shadowed items retained and visible | OMP `_source` + `_shadowed` | HAVE | `omm list --json` provider/level/shadowed/append/path | M11 |
| 130 | Format-preserving marker-region edit of a host config file you do not own | OMX TOML fence · OMO JSONC AST · OMC CLAUDE.md transaction | HAVE | AGENTS.md: markers; `settings.json`: typed-key patch with prior recorded (the host rewrites the file from a struct, so comments/markers cannot survive — R9) | M13, adapted to the host's rewrite behaviour |
| 131 | Porting / ingesting foreign skills into the native format | OMO 7-root shadowing · OMP 9 foreign roots · SLIM — | HAVE | host reads `.claude/skills` and `.codex/skills` live; `omm-skill-port` + `translation/muse.md` + lint `foreign-tool-vocabulary` | |
| 132 | Project-level override file that disables plugins/features for everyone on the repo | OMP `plugin-overrides.json` | GAP | none | same as row 75 |

### 3.15 Registry and marketplace

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 133 | Self-marketplace as the delivery vehicle | OMC · OMX · OMO · OMA · OMH tap | HAVE | root `marketplace.json` (native) + `.claude-plugin/` + `.agents/plugins/` catalogs, digest asserted at install (e2e s13) | |
| 134 | Third-party plugin manager / marketplace with install, upgrade, doctor | OMP full subsystem · OMA `managed-skill.ts` (3 pinned specs) | GAP | none; host `muse plugins marketplace add <git>` is native for any repo | N1 (become the index): a `muse-plugin`-topic harvester + trust levels, 2 weeks; omm adds nothing today |
| 135 | Vendored read-only ecosystem discovery index | OMH 216 items with claim boundary | GAP | none | N4: 2 days |
| 136 | Agent Plugins 1.0.0 / Claude-marketplace schema compatibility | OMP `agent-plugin-format.ts` · OMA `emit` | PARTIAL | the two foreign catalogs and `dist/` projections conform enough for Muse's own probe order | untested on the foreign hosts (row 99) |

### 3.16 Doctor and diagnostics

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 137 | Doctor with remediation commands | OMH 20 checks · OMX 4,172-line doctor · SLIM `doctor --json` · OMC skill · OMP plugin-scoped | HAVE | 16 checks D1–D16, each with `observed`, `why_silent`, `fix` (`crates/omm-doctor/src/checks/`) | compares a **live** echo session against what was installed (D8, D11) |
| 138 | Identity-conflict detection (another source claiming your name) | OMH `identity_conflicts.py` | PARTIAL | build-time: `id-bundled-skill`, `id-slash-collision`, `id-reserved`; runtime: D3 (mcp key), D5 (reminder) | no runtime check that a user/project skill shadows `plugin:omm:*` by bare id (SYNTHESIS §2.3 #27); 1 day |
| 139 | Load-time health assertion (minimum tools/agents composed) | SLIM `health-check.ts` | HAVE | D1 capabilities, D7 plugin count, D8 catalog pressure in a live session | |
| 140 | Hook latency benchmark with an enforced SLO | OMA bench, `P95_SLO_MS=1500` | HAVE | `tests/routing.rs` and e2e s09 assert < 5 ms cold | |
| 141 | Hook scaffold / validate / dry-fire subcommands | OMX `hooks init\|status\|validate\|test` | PARTIAL | `omm hook <NAME> < event.json` dry-fires; lint `hook-argv`; no scaffold | `omm hook new`: ½ day |
| 142 | Structured logs | SLIM log dir · OMO rotating log · OMX JSONL | HAVE | `$OMM/audit.log` JSONL; `omm-mcp.log` bounded at 1 MiB (KNOWN_ISSUES #11 path not contained) | |
| 143 | Fault isolation: a bad item is a warning, never a crash | OMP per-provider capture | HAVE | hooks fail open; overlay symlinks reported and skipped; corrupt ledger quarantined | |
| 144 | `--dry-run` producing the same summary as a real run | OMX setup + uninstall · SLIM · OMA | HAVE | every write verb; per-category converge table | S15 |

### 3.17 Cost and context

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 145 | Measured context cost of the installed content | OMH `skill-context-cost` (char limit 856 KB) · OMC compact shims · SLIM/OMO/OMX/OMP none | HAVE | `omm cost`: real `muse exec --provider echo` session, per-source catalog bytes, built-in tax, memory/rules/cookbook caps, cuts with savings | measured, not estimated — no rival measures |
| 146 | Context slimming applied by default with a guard against the replaced-default trap | none | HAVE | profile `default`: `first_sentence` + `bundled:git` re-listed (R18) | Muse-only lever (SYNTHESIS §2.3 #35) |

### 3.18 Memory and project state

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 147 | Memory store tooling for the model | OMC 20 tools + 6 hooks · OMX `project-memory.json` + wiki · OMH provider | PARTIAL | host-native memory tools + `omm memory seed/list/backup/gc` + `omm-reflect` | no wiki, notepad or shared store of omm's own (row 53) |
| 148 | Per-worktree runtime state directory with a tracked/ignored split | OMC `.omc/` 11 entries · SLIM `.slim/` · OMX `.omx/` · OMH `~/.omh/runtime` | PARTIAL | `<ws>/.omm/{skills,routing.json}` only when routing is enabled; no split, no seeded README | S13: 1 day (with row 83) |

### 3.19 Multi-host and i18n

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 149 | Projection of one content tree into several hosts | OMA 14 runtimes · OMO 4 hosts · OMC MCP registry → Codex · OMH MCP bridge → 4 hosts | PARTIAL | `dist/claude`, `dist/codex` generated from `content/`; routed library, themes, profiles, reminder not projected; no foreign-host test | by design single-host (M14); the projection exists because Muse's marketplace reader probes those schemas |
| 150 | Localized READMEs / trigger packs | OMX 13 · OMC 12 · OMA 12 · SLIM 4 · OMH 4 + 3 i18n trigger packs | GAP | English only | docs: 1 day per language; trigger packs need the router to accept locale word lists: 2 days |
| 151 | Framework as a standalone agent edition | OMO `omo-ai` · OMP (is the agent) | OUT_OF_SCOPE_BY_DESIGN | — | framework, not agent (MATRIX §1.2; DECISION §0) |

### 3.20 Security and trust

| # | Capability | Who ships it | omm | Evidence | Reason / what it would take |
|---|---|---|---|---|---|
| 152 | Strict posture that can only tighten and vetoes `--force` | OMC `OMC_SECURITY=strict` · OMH `OMH_SECURITY=strict` | GAP | permission profiles `omm-strict`/`omm-ci` exist; R22 tighten-only lattice **not implemented** (`profile.rs` header); `--force` has no veto | M18: a compile-checked lattice over the profile leaves + a `strict` mode refusing `--force`: 1 week |
| 153 | Symlink-escape refusal and path containment on every read/write | OMO `isSymlinkedProjectPath` · OMA `assertContainedRelPath` · SLIM path-confined delete | HAVE | `canonicalize` + `strip_prefix` under 4 bases; symlinked store dirs never handed to the host (e2e s18, s22, s25) | |
| 154 | Prompt-injection and credential-shape refusal on imported content | OMH 2 refusal classes · OMC 4 KB truncation + boundary tags | GAP | lint caps body size and foreign vocabulary; no injection regex, no secret-shape check | M18: apply to `omm-skill-port` output and any future `import`: 2 days |
| 155 | Supply-chain verification of the published artifact | OMC SLSA/in-toto verifier · OMX/OMP/SLIM SHA-256 sidecars | PARTIAL | `install.sh` verifies the tarball against `SHA256SUMS`; `fetch-host.sh` verifies size + SHA-256 | no attestation, no signature; M12 partially met (digest yes, signed provenance no) |
| 156 | Fail-closed on manifest corruption | SLIM `validateManifest` | HAVE | corrupt ledger quarantined and rebuilt through printed fixes (e2e s20); malformed host config refused | |
| 157 | Approval / effect receipts journaled | OMH 3 `.jsonl` journals | HAVE | `$OMM/audit.log` JSONL; capability approvals recorded in the ledger | |
| 158 | Human-in-the-loop gate on every write | OMH `OMH_RUN_SETUP=0` · M7 | HAVE | R6: refuses under `CI=true` / non-TTY without `-y` | |
| 159 | Permission-profile presets for the host | OMC `permissions.*` (undetailed) | HAVE | `omm-strict`, `omm-ci` as `settings.permissions.profiles` entries with `schema_version` kept | |
| 160 | Prototype-pollution / JS-object hardening | OMO, OMX `Object.create(null)` | OUT_OF_SCOPE_BY_DESIGN | — | typed serde structs (MATRIX §7.2) |
| 161 | Version-pinned self-identifying entry in the host's config | SLIM `oh-my-opencode-slim@<ver>` + marker · OMO | HAVE | plugin installed as `omm@ohmy` with `package_sha256` recorded in the ledger and asserted against the catalog digest | |
| 162 | Security review of the executable surface completed | OMC SLSA CI · others unstated | GAP | Gate 2 (hook dispatcher + MCP server) and Gate 3 (clean-clone R1–R22) are **open** (`docs/PLAN.md`); 18 known issues logged | run the gates before the first tag |

## 4. What omm has that no rival in the seven inventories has

Each item was checked against all 522 rival rows; "no rival" means no inventory row evidences it,
not that no project on earth has it.

1. **A CI-asserted byte- and mode-identical install → uninstall round trip** (e2e s01/s02/s04/s16/s21).
   MATRIX M6 noted its absence across all fourteen teardowns; still absent in these seven.
2. **Measured, not estimated, context cost** — `omm cost` runs a real host session and reports the
   catalog per source against the 32,000 B cap, the refundable built-in tax and the silent stage-2
   description drops. OMH warns from a character count; nobody else has a number at all.
3. **A host-drift self-test compiled from a fact → constant → test ledger** (`docs/host-reality.md`,
   62 rows, `hostcheck`, daily channel poll in CI). No rival re-measures its host; OMX probes one
   feature list at setup time.
4. **Doctor rows that compare what composed in a live session against what was installed**, each
   with `why_silent` and an exact fix (D1, D8, D11). Rival doctors validate disk against disk.
5. **A declarative model-side reminder** (`omm-verify-nudge`) — a Muse-only capability family; no
   rival host has the surface.
6. **A launcher shim that refuses prompt smuggling** (`omm run`): argv checked against the host's
   16 commands and root-flag walk so a stray token cannot become a billed `[PROMPT]`.
7. **An offline scripted-model harness proving the wire** (`tools/mockprovider/`): model →
   `tools/call` → tool result → next request, and `read_skill` of a routed skill, with no network.
   OMH's post-install smoke imports the plugin; nobody drives the model side.
8. **Skill authoring lint that runs the host's own validators as checkpoints and enforces the
   18-character MCP namespace rule** (`mcp-id-length`) — a rule derived from a measured host
   rewrite, not from documentation.
9. **Ledger entry written before the file** (e2e s26), so an interrupted install is always
   undoable; OMX's claim journal is the nearest rival design and it commits after.

Not unique, but worth naming because only one rival has each: the three-way merge with a recorded
ancestor (SLIM), the two-section uninstall preview (OMH, OMA), the append channel (SLIM), and the
capability-qualified disable ids (OMP).

## 5. What to build next if content breadth is the goal

Ordered by user value per unit of effort, against the gaps and partials above. Effort is a rough
engineering estimate for one person who knows the tree.

| Rank | Item | Closes rows | Effort | Why this order |
|---|---|---|---|---|
| 1 | **Land the 23 in-flight skills through `omm lint` and `omm cost`**, then re-measure the catalog | 1 | in progress | it is the single largest visible count gap (12 → 35), and it is already paid for |
| 2 | **Enforce `disabled: ["hook:…"]` in the dispatcher** | 42, 80, 127 | ≤ 1 day | a documented switch that does nothing is A5 in our own tree; fix before anyone relies on it |
| 3 | **Package the `custom/` overlay into a second local plugin (`omm-custom`)** | 7, 8, 29, 31, 40, 125, 126 | ~1 week | every "user content" row is PARTIAL for one reason; this turns seven partials into HAVE and makes learned/authored skills real |
| 4 | **Six-persona agent pack under `$CONFIG_DIR/agents/` with a name-grammar lint** | 18, 19, 20 | 3–4 days | the most-asked-for content class after skills (7 of 7 rivals ship one); TUI-only is an honest caveat, not a blocker |
| 5 | **Planning / handoff / PR-description commands as prose skills** | 27, 63 | 1 day each | cheap, high-frequency workflows every rival ships; `handoff` and `omm-pr` are already in flight |
| 6 | **Glob-conditional rules via a `UserPromptSubmit` handler + `custom/rules/*.md` frontmatter** | 38, 47 | 3 days | closes the biggest "rules" gap without touching the one-file AGENTS.md constraint |
| 7 | **`PostToolUse` linter hook and `PreCompact` memory-pointer hook** | 39, 43 | 3 days | 14 of 17 host events are idle; these two are the ones users feel |
| 8 | **Skill-set presets (`profile.skills[]`) honoured by install** | 12 | 2–3 days | lets a 35-skill bundle ship without forcing 35 catalog entries on every user (budget row: ≈ 13.6 KB) |
| 9 | **Keymap presets (2) and a light/dark theme pair** | 72, 73 | 1–2 days | the surfaces exist; they are empty |
| 10 | **`.gitignore` lines + `<ws>/.omm/` tracked/ignored split + workspace-level `config.json`** | 75, 83, 132, 148 | 3 days | S13 and the project config layer together |
| 11 | **`ast_grep_search/replace` tools in `omm mcp` (shelling to a user-installed `sg`)** | 52 | ~1 week | the one model-tool class every rival with an MCP server ships; must not provision the binary |
| 12 | **`SKILL-SOURCES.md` provenance table + upstream drift check** | 16 | 2 days | needed the moment a ported skill lands (rank 3 and `omm-skill-port` make that imminent) |
| 13 | **Rules import from `.cursor/rules`, `.clinerules`, Copilot instructions, with injection/secret refusal** | 49, 154 | 4 days | S1 + M18 together; the refusal must ship with the importer, not after |
| 14 | **Model/run presets as an asset kind** | 77 | 2 days + untestable offline | last, because nothing here can be proven with the echo provider |

Not on this list, on purpose: orchestration runtimes, autonomy loops, a statusline, output styles,
multi-host projection beyond the generated manifests, an enterprise tier. The first two are the
host's job (DECISION §0), the next two are impossible on Muse (SYNTHESIS §2.4), the last two are
struck (M14, DECISION §4). Before any of the above ships to users: close Gate 2 and Gate 3 (row 162)
and cut the first tag so `install.sh` stops pointing at a placeholder (row 88).
