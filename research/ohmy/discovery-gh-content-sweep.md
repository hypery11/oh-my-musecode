# Discovery — GitHub content/topic sweep (for oh-my-musecode)

Method: `gh` CLI (authenticated, core API for traction to dodge the 30/min search cap),
topic sweeps + code search for manifest markers, then `git clone --depth 10..20` of the
12 highest-value candidates and direct reading of installers, manifests and adapter code.
Traction captured 2026-09-01. Clones under `scratchpad/ohmy/gh-content-sweep/src/`.

Every repo below was resolved through `GET /repos/{owner}/{repo}` — none are inferred.
Two redirects worth noting: `sst/opencode` → **anomalyco/opencode**, and
`code-yeongyu/oh-my-opencode` → **code-yeongyu/oh-my-openagent**.

---

## 1. The finding that matters most: an "oh-my-*" lineage already exists for coding agents

This is direct prior art and it is large.

| Repo | Stars | Pushed | What it actually is |
|---|---:|---|---|
| code-yeongyu/oh-my-openagent | 68,585 | 2026-09-01 | npm pkg still named `oh-my-opencode` v5.0.0-beta.31. Ships 5 bin aliases (`oh-my-opencode`, `oh-my-openagent`, `omo-agent-toolkit`, `lazycodex`, `lazycodex-ai`). Targets Codex + OpenCode. |
| Yeachan-Heo/oh-my-claudecode | 38,933 | 2026-09-01 | npm pkg `oh-my-claude-sisyphus` v5.1.0, bins `oh-my-claudecode`/`omc`. Teams-first multi-agent orchestration. |
| alvinunreal/oh-my-opencode-slim | 8,558 | 2026-09-01 | Lean fork of the omo idea. |

`oh-my-openagent` is the single most transferable precedent. Its `package.json` `files`
array publishes **`.opencode/command`, `.opencode/skills`, `.agents/command`,
`.agents/skills`, `packages/omo-codex/marketplace.json` and
`packages/omo-codex/plugin/.codex-plugin`** — i.e. one npm artifact that drops native
manifests into several different agents' config trees, with a `postinstall` script doing
the placement. That is exactly the shape oh-my-musecode needs, and because Muse Code's
loader already recognises `.codex-plugin`, **this repo's published artifact may be
partially ingestible by Muse Code today**. Worth testing early.

Also note the naming drift: the project renamed opencode→openagent while keeping the npm
name, and carries 5 bin aliases. Evidence that in this niche the "oh-my-" brand is used as
a *distribution* label across multiple host agents, not a single-host label.

Smaller/derivative "oh-my" entries (all real, all low traction): TechDufus/oh-my-claude
(176), 2lab-ai/oh-my-claude (36, self-described "Inspired by oh-my-opencode"),
vyvhouse/oh-my-destructor (29, an *uninstaller* for oh-my-claudecode — the lifecycle gap
is real enough that someone shipped a third-party remover), huangdijia/oh-my-claude-code-plugins (11),
stefandevo/oh-my-claude (7, "Port of oh-my-opencode").

---

## 2. microsoft/apm — the serious package-manager precedent

3,683 stars, 342 forks, 224 open issues, pushed 2026-09-01, Python, created 2025-09-18.

This is the most architecturally relevant repo in the whole sweep for the
`.muse/lock.json` provenance/quarantine problem. Read from the clone:

- **`apm.yml`** — human manifest: `name/version/description/includes: auto/dependencies/scripts`.
  Deps are grouped by ecosystem (`apm:` list + `mcp:` list) and support local paths.
- **`apm.lock.yaml`** — real lockfile: `lockfile_version`, `generated_at`, `apm_version`,
  and per-dep `repo_url`, `name`, `version`, `package_type`, plus an exhaustive
  **`deployed_files` array and a `deployed_file_hashes` map**. So uninstall is exact and
  tamper is detectable. This is precisely the model `.muse/lock.json` should copy.
- **`CONFORMANCE.json` / `CONFORMANCE.md`** — a generated conformance statement against a
  written spec, "OpenAPM v0.1", with four **conformance classes: Producer, Consumer,
  Registry, Governance** (12/89/1/17 active requirements). Requirements are ID'd
  (`req-lk-001` …) and bound to pytest markers, with an explicit "Honesty contract"
  admitting where drift detection does not exist.
- **`src/apm_cli/commands/`** — the lifecycle verb set worth stealing wholesale:
  `install, uninstall, update, outdated, lock, prune, audit, doctor, policy, approve,
  publish, pack, cache, marketplace, registry, targets, runtime, self_update, compile, deps, lifecycle`.
- `src/apm_cli/adapters/` + `agent_plugins/` + `copilot_plugins/` — multi-host targeting.
- Deploys into a neutral **`.agents/skills/…`** tree, not a vendor-specific one.

Takeaway: `approve` + `policy` + `audit` map almost 1:1 onto Muse's
provenance/quarantine/allowed_tools fields in `.muse/lock.json`.

---

## 3. wshobson/agents — the multi-harness adapter precedent

39,326 stars, 4,192 forks, only 9 open issues, pushed 2026-09-01. 91 local plugins.
Description already says "Multi-harness agentic plugin marketplace for Claude Code, Codex,
Cursor, OpenCode, GitHub Copilot, and Google Antigravity".

`ARCHITECTURE.md` states five invariants that read like a design doc oh-my-musecode could adopt:

1. **Single source of truth** — all authoring under `plugins/<name>/`; per-harness artifacts
   (`.codex/`, `.opencode/`, `.copilot/`, `.antigravity/`) are generated and gitignored.
   Exception: small native-install registries *are* committed (`.agents/plugins/marketplace.json`,
   `plugins/*/.codex-plugin/plugin.json`, `.cursor-plugin/`) because they only point at source.
2. **One canonical context file** — `AGENTS.md` is authored; `CLAUDE.md` is a *symlink* to it.
3. **Adapters own per-harness mechanics** — `tools/adapters/{base,capabilities,codex,cursor,opencode,antigravity,copilot}.py`,
   with a shared `capabilities.py` capability matrix. Source content never carries host conditionals.
4. **Mechanical enforcement with remediation hints** — every lint finding ships a fix string
   (`make validate`, `make garden`, `plugin-eval harness_portability`).
5. **Progressive disclosure** — context files cap ~150 lines, skill bodies cap ~8 KB
   (stated as Codex's hard limit), overflow to `references/details.md`.

Concrete detail relevant to Muse: the codex adapter does a **Markdown→TOML transform, an
8 KB body cap with `references/` overflow, a sandbox_mode heuristic and collision detection**.
Anyone writing a Muse adapter will hit the same class of per-host transforms.

This repo is the strongest argument that oh-my-musecode should be an **adapter/compiler**
over portable source, not a pile of Muse-specific markdown.

---

## 4. Official / normative anchors

- **anthropics/claude-plugins-official** — 35,785★, 3,992 forks, 1,069 open issues, pushed 2026-09-01.
  `.claude-plugin/marketplace.json` carries **291 plugins**, plus `plugins/` and a separate
  `external_plugins/` tree. This is the canonical marketplace.json shape to conform to —
  and since Muse ingests `.claude-plugin`, it is a candidate upstream source.
- **anthropics/knowledge-work-plugins** — 23,795★, pushed 2026-09-01. Non-engineering plugin set for Cowork.
- **anthropics/skills** — 172,943★, 20,533 forks, pushed 2026-08-21. The official Agent Skills content repo.
- **agentskills/agentskills** — 24,928★, created 2025-12-16, pushed 2026-08-09. The
  *specification* for Agent Skills (`specification.mdx`, `docs/client-implementation/`,
  `skills-ref/src` reference implementation + tests). Normative `SKILL.md` layout:
  `SKILL.md` required (name + description minimum) plus optional `scripts/ references/ assets/`.
  If oh-my-musecode wants portability, this is the contract to implement — and
  `docs/client-implementation/` is written for exactly the case of a new host agent.

---

## 5. Framework-vs-catalog classification of the named targets

Verified traction, all confirmed to exist:

| Repo | Stars | Forks | Pushed | Kind | Note |
|---|---:|---:|---|---|---|
| hesreallyhim/awesome-claude-code | 53,328 | 4,642 | 2026-09-01 | CATALOG | 966 open issues; the canonical awesome-list |
| musistudio/claude-code-router | 37,013 | 3,109 | 2026-09-01 | TOOL | repositioned as "local control plane for every AI agent"; routing, not config packaging |
| davila7/claude-code-templates | 30,482 | 3,463 | 2026-09-01 | FRAMEWORK | npm `claude-code-templates` v1.29.4, **8 bin aliases**; `components/{agents,commands,hooks,loops,mcps,sandbox,settings,skills}`; also ships analytics + plugin/skill/teams dashboards, `security-audit.js`, `health-check.js`, and a `cli-rust/` port |
| VoltAgent/awesome-claude-code-subagents | 24,777 | 2,867 | 2026-09-01 | CATALOG | 100+ subagents |
| SuperClaude_Framework | 23,856 | 2,013 | 2026-08-21 | FRAMEWORK | Python/uv `install.sh`, installs 30 slash commands to `~/.claude/commands/`; single `plugins/superclaude/` plugin w/ `core/ modes/ agents/ skills/ commands/ hooks/ mcp/`; PLUGIN_INSTALL.md is partly Japanese and hardcodes an author-local path |
| contains-studio/agents | 12,406 | 2,511 | **2025-07-28** | CATALOG | high stars, **stale >1yr** |
| sirmalloc/ccstatusline | 12,704 | 557 | 2026-08-26 | TOOL | statusline only |
| steipete/agent-rules | 5,695 | 508 | 2026-05-03 | CATALOG | **ARCHIVED** |
| wshobson/commands | 2,621 | 290 | **2025-10-12** | CATALOG | superseded by wshobson/agents |
| carlrannaberg/claudekit | 761 | 117 | 2026-03-31 | FRAMEWORK | npm v0.9.5, two bins (`claudekit`, `claudekit-hooks`); `cli/{commands,hooks,lib}`; ships `.cursorrules`/`.windsurfrules`/`.clinerules` too. Quietest of the frameworks |
| GowayLee/cchooks | 130 | 11 | 2026-04-08 | TOOL | Python SDK for hooks (this is the real `cchooks`) |
| disler/claude-code-hooks-mastery | 3,906 | — | 2026-03-04 | CATALOG | disler's top hook repo; reference impl, not an installer |

disler's wider portfolio (verified via `users/disler/repos`): `claude-code-hooks-multi-agent-observability`
(1,529), `super-simple-software-factory` (781, "packaged as one skill, stamped into any repo"),
`the-library` (417, "Meta-Skill for Private-First Distribution of Agentics across your Agents"),
`agent-sandbox-skill` (382), `claude-code-damage-control` (479). `the-library` and
`super-simple-software-factory` are the two conceptually closest to a distribution framework.

---

## 6. Marketplaces / registries with real install tooling

- **jeremylongshore/tons-of-skills-marketplace** — 2,687★, pushed 2026-09-01. Claims 471
  plugins / 3,069 skills / 347 agents behind a **`ccpi` CLI package manager**. Largest
  third-party registry found with its own PM.
- **karanb192/claude-code-hooks** — 493★, pushed 2026-08-23. Hooks + *installable plugin
  marketplace*; `plugins/` includes `block-dangerous-commands`, `git-safety`,
  `notify-permission`, `config-watch`, `dead-rules-audit`. Closest to Muse's hooks.json + safety story.
- **davepoon/buildwithclaude** — 3,401★, pushed 2026-08-31. Aggregation hub across skills/agents/commands/hooks/plugins/marketplaces.
- **composio-community/awesome-claude-plugins** (1,916★), **ccplugins/awesome-claude-code-plugins** (927★),
  **quemsah/awesome-claude-plugins** (1,259★ — notable because it *automatically* harvests
  plugin adoption metrics across GitHub via n8n; a ready-made source of ecosystem telemetry).
- **netresearch/claude-code-marketplace** — 54★. Small, but explicitly aligns to the
  agentskills.io open standard and claims portability across Claude Code/Cursor/Copilot/Codex/Gemini.
- **microsoft/apm** — see §2; has `marketplace/` and `registry/` modules.

---

## 7. Cross-host config managers (the "one config, many agents" category)

This category is the real competitive set for oh-my-musecode.

- **farion1231/cc-switch** — 130,528★, 8,963 forks, 2,575 open issues, Rust, pushed 2026-09-01.
  Desktop all-in-one config switcher explicitly covering "Claude Code, Codex, OpenCode,
  OpenClaw, Grok Build & Hermes Agent". Enormous traction. A Muse Code target here would
  reach users immediately — and it shows config-switching is a mainstream need.
- **hoangnb24/repository-harness** — 1,209★, 431 forks, **Rust**, pushed 2026-08-13.
  "Turn any repo into an agent-ready workspace for Claude Code, Codex, Cursor, and other agents."
  Rust + repo-scaffolding is the nearest technical analogue to a Muse-native tool.
- **yzhao062/anywhere-agents** — 241★, pushed 2026-08-29. "One config to rule all your AI
  agents: portable, effective, safer (destructive-command guards)."
- **alex-feel/claude-code-toolbox** — 16★. Explicitly "automated installers and environment
  configuration framework … one-line setup across Windows, macOS, Linux". Tiny but exactly on-brief.
- **harnessprotocol/harness-kit** — 10★, created 2026-03-07. "Your plugins, skills, MCP
  servers, hooks, conventions, and governance packaged into a single config." Newest and
  most literally identical framing to oh-my-musecode. Watch, don't copy.
- **ReflexioAI/claude-smart** — 772★. Turns corrections into preferences/skills across
  Claude Code, Codex, OpenCode — the *learning* angle on config.
- **boshu2/agentops** — 432★. "Portable skills and contracts" operations layer.
- **mksglu/context-mode** — 20,291★, pushed 2026-09-01. Context-window optimisation +
  session memory + routing across 17 platforms. TOOL, but a heavyweight neighbour.
- **sickn33/agentic-awesome-skills** — 45,808★, 6,698 forks. "AAS Core … local, agent-first
  control plane for catalog discovery, agent-owned selection, stack validation" over ~2,000 skills.
  Catalog scale with framework ambitions.

---

## 8. OpenCode plugin ecosystem (host with a real plugin API)

**anomalyco/opencode** (ex-`sst/opencode`) — 202,996★, 26,420 forks, 5,623 open issues,
TypeScript, pushed 2026-09-01. The largest OSS coding agent found. Its plugin ecosystem is
healthy and single-purpose, which is a useful contrast to the mega-framework pattern:

kdcokenny/opencode-worktree (718), shekohex/opencode-pty (566), kdcokenny/opencode-workspace
(581, "bundled multi-agent orchestration harness … one install"), ZaxbyHub/opencode-swarm (463),
kdcokenny/opencode-background-agents (377), griffinmartin/opencode-claude-auth (1,250),
slkiser/opencode-quota (931), ianjwhite99/opencode-with-claude (547).

Charm ecosystem: **charmbracelet/crush** — 27,847★, Go, pushed 2026-09-01, active.
**charmbracelet/mods** — 4,523★, **ARCHIVED**. No oh-my-style config framework has formed
around Crush yet; that is an open niche and a signal about timing for Muse.

---

## 9. `.claude-plugin/marketplace.json` in the wild (code search)

Real repos shipping the manifest — note how many are *ordinary products* embedding a
marketplace rather than dedicated plugin repos. Evidence the manifest is becoming a
default artifact, which is good news for a Muse loader that ingests it:

redpanda-data/connect, grafana/gcx, storybookjs/mcp, DefangLabs/defang,
datalayer/jupyter-mcp-server (`extensions/.claude-plugin/`), featurevisor/featurevisor,
Ivy-Interactive/Ivy-Framework (`src/.claude-plugin/`), ludo-technologies/pyscn,
Pimzino/spec-workflow-mcp, JasonXuDeveloper/JEngine, FamilySearch/gedcomx(+-java),
basnijholt/agent-cli, spencerc99/playhtml, brendan-duncan/webgpu_inspector,
CharlesWiltgen/Axiom, EtienneLescot/n8n-as-code, vshulcz/deja-vu, mem9-ai/mem9,
MemPalace/mempalace, agentic-box/memora, Eigenwise/atomic-agents, Fergana-Labs/stash,
Owl-Listener/{ai-design-skills,designer-skills}, ViryaZheng/recomby-geo, wanteddev/montage-web.

---

## 10. What this means for oh-my-musecode

1. **The "oh-my-" name is taken but not for Muse.** oh-my-openagent (68k) and
   oh-my-claudecode (39k) establish the brand pattern and prove demand. `oh-my-musecode`
   is unclaimed and consistent with the lineage.
2. **Don't build a catalog.** Catalogs saturate fast (awesome-claude-code 53k;
   VoltAgent 33k/24k; anthropics/skills 173k). The scarce good is *lifecycle*:
   install, pin, update, verify, uninstall. Only apm, davila7, omo, omc and claudekit do it.
3. **Copy apm's lockfile, not just its idea.** `deployed_files` + `deployed_file_hashes`
   gives exact uninstall and tamper detection — the natural fit for `.muse/lock.json`'s
   provenance/quarantine fields. Copy the verb set too (`audit`, `policy`, `approve`,
   `doctor`, `outdated`, `prune`).
4. **Be an adapter, not a fork.** wshobson/agents shows portable source + per-host adapters
   + a capability matrix. Muse's acceptance of `.claude-plugin` and `.codex-plugin` means
   oh-my-musecode's cheapest v1 is an *importer/compiler* that makes the existing 291-plugin
   official marketplace and the 91-plugin wshobson set work under Muse — instant catalog,
   zero content authoring.
5. **Conform to agentskills/agentskills.** It has a written spec and a
   `docs/client-implementation/` guide aimed at new host agents. Conforming buys portability
   and credibility cheaply, and `skills-ref/` gives conformance tests to run against `.muse/skills.lock`.
6. **Uninstall is a real, unmet need.** Someone shipped `oh-my-destructor` purely to remove
   oh-my-claudecode. Ship clean uninstall on day one.
7. **Watch harness-kit and repository-harness.** harness-kit's one-line pitch is nearly
   identical to oh-my-musecode's; repository-harness is Rust and already multi-host.
