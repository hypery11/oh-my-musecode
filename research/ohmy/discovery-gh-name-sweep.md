# Discovery: GitHub `oh-my-*` name-pattern sweep for AI coding agents

Date of sweep: 2026-09-01. All rows below were verified to EXIST via `gh repo view`
(core API) — star/fork/push/created/license/release figures are live values pulled at
sweep time, not recalled. Repos I could not reach are marked NOT_FOUND (none were).

## Method

1. `gh search repos "<stem>"` across 54 stem variants (hyphenated, unhyphenated, and
   `omc`/`omx`/`omo`/`omp` abbreviations). Raw JSON in
   `gh-name-sweep/raw2.jsonl`, `gh-name-sweep/raw3.jsonl`; flattened in `parsed2.txt`, `parsed3.txt`.
2. Precision passes with the `in:name` qualifier via `gh api search/repositories`, bucketed by
   stars so the 1000-result cap never bites:
   - `oh-my in:name pushed:>2025-11-01 stars:>50` → **76 hits** (`name_sweep_top.txt`)
   - `oh-my in:name pushed:>2026-01-01 stars:5..50` → **224 hits** (`buckets.txt`)
   - `ohmy  in:name pushed:>2025-11-01 stars:>3`  → **81 hits** (`buckets.txt`)
3. Per-candidate verification with `gh repo view --json ...` (`detail.sh`).

Note on rate limits: the GitHub *search* API is 30 req/min (separate from the 5000/hr core
quota). The sweep scripts pace at 2.6s/query with a 65s pause every 25 queries.

---

## Headline conclusion

The `oh-my-*` pattern has been applied to **essentially every terminal coding agent that
exists** — Claude Code, Codex, OpenCode, Pi, Gemini CLI, Copilot CLI, Cursor, Kilo, Qwen,
Kimi, Grok Build, Antigravity, OpenClaw, Hermes, DeepSeek Harness, Droid, Kiro, Qoder,
Trae, Warp, Crush, Auggie, MiniMax Code, Devin, Gajae Code, Multica — **except Meta Muse
Code**. That is the gap oh-my-musecode occupies.

The pattern's semantics have drifted. In shell land `oh-my-*` meant "config framework". In
agent land it now means one of five distinct things, and the ambiguity matters for
positioning:
- **(a) config/orchestration framework** installed into someone else's agent (the oh-my-zsh analogue),
- **(b) the agent itself** — `can1357/oh-my-pi` (28.8k) and `code-yeongyu/oh-my-openagent`
  (68.5k) are full agent harnesses/forks, not config layers,
- **(c) a curated catalog/marketplace** of plugins for an agent,
- **(d) a single skill pack** (oh-my-resume, oh-my-mermaid, oh-my-design),
- **(e) an unrelated name collision** (oh-my-posh, oh-my-fish, oh-my-rime, OhMyMeme…).

### The DeepSeek Harness land-grab — the closest precedent for Muse Code

DSH shipped mid-August 2026. Within ~10 days at least **11 distinct `oh-my-dsh` /
`Oh-My-DSH` repos** were created by unrelated authors:

| repo | created | stars | what it actually is |
|---|---|---|---|
| wangshunnn/oh-my-dsh | 2026-08-13 | 5 | auto-updating plugin index |
| huiliyi37/oh-my-tianshu | 2026-08-13 | 39 | friendly MIT fork of the harness |
| LaplaceYoung/oh-my-dsh | 2026-08-13 | 54 | 700+ plugin ecosystem, extension-seam only |
| like-study1/Oh-My-DSH | 2026-08-14 | 78 | catalog, auto-synced every 4h from `dsh-plugin` topic |
| LiuMengxuan04/oh-my-dsh | 2026-08-14 | 10 | autopilot loop |
| NoWint/Oh-My-DSH | 2026-08-14 | 15 | catalog, hourly refresh |
| amplifthq/oh-my-dsh | 2026-08-14 | 14 | "curated distribution. Overlay, not a fork." |
| agi-fans/oh-my-dsh | 2026-08-15 | 27 | keyboard-first agent built on DSH plugin arch |
| llmpolska/oh-my-dsh | 2026-08-16 | 2 | tiered model-routing plugin |
| ninipa/oh-my-dsh-slim | 2026-08-23 | 8 | port of oh-my-opencode-slim |
| oh-my-dsh (org) | 2026-08-30 | 33 | `dsh-plugin-upgrade-skill` |

Lesson for oh-my-musecode: **the name is contested within days of a host's launch, and the
winner is decided by which repo becomes the catalog/registry the others point at**, not by
which shipped first. Two of the eleven went straight for the auto-synced catalog position.

---

## Tier A — the anchors (>8k stars)

| repo | stars / forks | pushed | lang | kind | note |
|---|---|---|---|---|---|
| [code-yeongyu/oh-my-openagent](https://github.com/code-yeongyu/oh-my-openagent) | 68585 / 5630 | 2026-09-01 | TS | **AGENT** (b) | "omo/lazycodex". 507 open issues, v5.0.0-beta.19, non-OSI license. Renamed from oh-my-opencode. Targets Codex+OpenCode. |
| [Yeachan-Heo/oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) | 38933 / 3492 | 2026-09-01 | TS | FRAMEWORK (a) | Teams-first multi-agent orchestration for Claude Code. MIT, v5.1.0, created 2026-01-09. The canonical "oh-my-* for a coding agent". |
| [Yeachan-Heo/oh-my-codex](https://github.com/Yeachan-Heo/oh-my-codex) | 32945 / 2534 | 2026-09-01 | TS | FRAMEWORK (a) | "OmX". Same author as above — the strongest evidence that one author can hold the name across multiple hosts. v0.21.2, no license file. |
| [can1357/oh-my-pi](https://github.com/can1357/oh-my-pi) | 28856 / 2884 | 2026-09-01 | TS | **AGENT** (b) | "Coding agent with the IDE wired in". 1261 open issues, v18.1.0, MIT. Spawned its own plugin ecosystem (`omp-*`, 40+ repos). |
| [alvinunreal/oh-my-opencode-slim](https://github.com/alvinunreal/oh-my-opencode-slim) | 8559 / 510 | 2026-09-01 | TS | FRAMEWORK (a) | The "slim" counter-positioning against a bloated incumbent. MIT, v2.2.17. Itself ported to DSH and Antigravity. |

## Tier B — substantial per-agent ports (100–1500 stars)

| repo | stars | pushed | target agent | kind |
|---|---|---|---|---|
| [oh-my-mermaid/oh-my-mermaid](https://github.com/oh-my-mermaid/oh-my-mermaid) | 2217 | 2026-04-07 | Claude Code | SKILL (d) — codebase→diagrams |
| [rlaope/oh-my-hermes](https://github.com/rlaope/oh-my-hermes) | 1299 | 2026-09-01 | Hermes Agent | FRAMEWORK — harness + memory + model routing |
| [first-fluke/oh-my-agent](https://github.com/first-fluke/oh-my-agent) | 1257 | 2026-09-01 | **multi** (CC, Codex, Cursor, 10+) | FRAMEWORK — verification-first: stop-hook gates, independent judges, append-only event logs |
| [Dong90/oh-my-taiyiforge](https://github.com/Dong90/oh-my-taiyiforge) | 887 | 2026-08-31 | Claude/Codex | FRAMEWORK |
| [Salomondiei08/oh-my-hermes](https://github.com/Salomondiei08/oh-my-hermes) | 857 | 2026-07-28 | Hermes Agent | FRAMEWORK — name collision with rlaope's |
| [sangrokjung/claude-forge](https://github.com/sangrokjung/claude-forge) | 821 | 2026-08-29 | Claude Code | FRAMEWORK — self-describes as "oh-my-zsh for Claude Code"; 16 agents/35 cmds/32 skills/21 hooks |
| [qwen-code-dev-bot/oh-my-cli](https://github.com/qwen-code-dev-bot/oh-my-cli) | 802 | 2026-08-12 | Qwen Code | AGENT (b) |
| [LigphiDonk/Oh-my--paper](https://github.com/LigphiDonk/Oh-my--paper) | 720 | 2026-04-15 | Claude Code | SKILL (d) — research pipeline |
| [kwakseongjae/oh-my-design](https://github.com/kwakseongjae/oh-my-design) | 474 | 2026-08-28 | **multi** (CC/Codex/Cursor/OpenCode) | CATALOG (c) — 400+ graded DESIGN.md refs, "zero AI calls" |
| [witt3rd/oh-my-hermes](https://github.com/witt3rd/oh-my-hermes) | 302 | 2026-08-05 | Hermes Agent | FRAMEWORK — third oh-my-hermes |
| [WilliamJudge94/oh-my-opencode-dashboard](https://github.com/WilliamJudge94/oh-my-opencode-dashboard) | 290 | 2026-03-14 | OpenCode+OMO | TOOL |
| [Joonghyun-Lee-Frieren/oh-my-antigravity](https://github.com/Joonghyun-Lee-Frieren/oh-my-antigravity) | 211 | 2026-06-22 | Antigravity CLI | FRAMEWORK |
| [Four-JJJJ/oh-myusage](https://github.com/Four-JJJJ/oh-myusage) | 186 | 2026-08-30 | multi | TOOL — macOS menubar quota monitor |
| [happycastle114/oh-my-openclaw](https://github.com/happycastle114/oh-my-openclaw) | 185 | 2026-04-09 | OpenClaw | FRAMEWORK — "oh-my-opencode patterns ported" |
| [VOBC/oh-my-coder](https://github.com/VOBC/oh-my-coder) | 185 | 2026-08-16 | own | AGENT (b) |
| [3x-haust/oh-my-design](https://github.com/3x-haust/oh-my-design) | 185 | 2026-08-24 | — | SKILL — collides with kwakseongjae's |
| [TechDufus/oh-my-claude](https://github.com/TechDufus/oh-my-claude) | 176 | 2026-07-13 | Claude Code | FRAMEWORK — single-feature ("ultrawork") |
| [NakanoSanku/OhMySkills](https://github.com/NakanoSanku/OhMySkills) | 165 | 2026-01-09 | Claude Code | CATALOG |
| [jmstar85/oh-my-githubcopilot](https://github.com/jmstar85/oh-my-githubcopilot) | 153 | 2026-05-20 | Copilot/VS Code | FRAMEWORK — 28 agents, 30 skills, hooks, guardrails |
| [ifiokjr/monopi](https://github.com/ifiokjr/monopi) | 149 | 2026-08-28 | pi-coding-agent | FRAMEWORK — "Like oh-my-zsh for pi" without the name |
| [AlphaLab-USTC/OhMyCode](https://github.com/AlphaLab-USTC/OhMyCode) | 131 | 2026-04-02 | own | AGENT (b) |
| [tmcfarlane/oh-my-cursor](https://github.com/tmcfarlane/oh-my-cursor) | 108 | 2026-07-02 | Cursor IDE | FRAMEWORK — "nothing but a few config files" (closest structural analogue to a declarative-asset framework) |
| [KaimingWan/oh-my-kiro](https://github.com/KaimingWan/oh-my-kiro) | 103 | 2026-04-02 | Kiro | FRAMEWORK |

## Tier C — long tail per-agent ports (5–100 stars), grouped by host

**Claude Code**: baekenough/oh-my-customcode (34, "oh-my-zsh style customization framework
for Claude Code", v1.1.57), 2lab-ai/oh-my-claude (36), vyvhouse/oh-my-destructor (29, *safe
uninstaller* for OMC — evidence the ecosystem needed an uninstall story),
854771076/oh-my-claude-roles (22), erkandogan/oh-my-team (17), kyu1204/oh-my-harness (16,
generates CLAUDE.md/hooks/settings from NL), ssenart/oh-my-claude (15, statusline),
huangdijia/oh-my-claude-code-plugins (11, marketplace), suparerk9x/Oh-My-Claude (7),
stefandevo/oh-my-claude (7, ARCHIVED), ZDragon17/oh-my-claude (5),
hey-pals/Oh-My-Claude (5), alexj/oh-my-claude-duo (4), SleepyLGod/oh-my-claude-code (2),
Boulea7/ohmyclaude (0), Yeachan-Heo/oh-my-claudecode-website (37),
youngeun1209/oh-my-claudecode-research (31), mazenyassergithub/oh-my-claudecode (6)

**Codex**: scalarian/oh-my-codex (73, **ARCHIVED** 2026-04-01), realsigridjin/oh-my-codex
(20), materialofair/oh-my-codex (12), junghwaYang/oh-my-codex (5),
baekenough/oh-my-customcodex (3, 49 open issues), YanzuoLu/oh-my-codex-slim (2),
Yeachan-Heo/oh-my-codex-website (30), Meredith2328/nano-omx (4),
j-lag/OhMyCodexPortable (0, Windows no-admin installer), windyslime/ohmycodex (1)

**OpenCode / OpenAgent**: HanTechnology/oh-my-openagent-toolkit (38), L4ntern0/oh-my-tang
(28, Tang-dynasty governance metaphor), srod/oh-my-opencode-config (20),
SoraYama/omo-configurator (17, GUI), HaiNinh1/oh-my-opencode (13),
junlin-233/oh-my-lite-openagent (8), huchi996/oh-my-opencode-toggle (9),
hellosunghyun/oh-my-opencode-docs (3), + ~25 near-empty forks

**Pi / OMP**: ayu-exorcist/oh-my-pi (15), DeprecatedLuke/oh-my-singularity (24),
nornzach/oh-my-pi-gui (18), unkeyn/oh-my-pi-gui (10), BRCOO/ohmypi-craft (7),
bparlan/omp-agent (38), sakuradairong/omp-config (44), wolfiesch/omp-best-of (63),
mikeatlas/omp-sbx (26), 77zane/oh-my-agent-platform (18), + ~40 `omp-*` extensions

**DeepSeek Harness**: see land-grab table above, plus lizhiyao/oh-my-knowledge (18, eval
harness w/ DSH support), amplifthq/oh-my-dsh (14)

**OpenClaw**: minpeter/oh-my-openclaw (66, **ARCHIVED**), jkf87/ohmyclaw (61),
hqwuzhaoyi/oh-my-acpx (3), TeFuirnever/oh-my-matrix (3, "omm"),
Xingyu-Romantic/oh-my-opencode-openclaw (11), hanjiayuan2025-coder/oh-my-openclaw (1),
clawflint/oh-my-openclaw (0), + ~10 empties

**Grok Build**: ImL1s/oh-my-grok (16), metaphorics/oh-my-pi-plugin-grok-build (14),
mihazs/oh-my-grok (9, Go), duarbdhks/oh-my-grok-build (8), ART1KZ/omp-grok-build (5),
Kyou12138/oh-my-grok (3), + ~10 empties incl. an `oh-my-grokbuild` org

**Antigravity**: materialofair/oh-my-antigravity (14), TurnaboutHero/oh-my-antigravity (13),
shayne-snap/oh-my-antigravity (3), + ~8 empties

**Gemini CLI**: richardcb/oh-my-gemini (16), oneforce/oh-my-openagent-gemini (2),
pangelini777/oh-my-gem (2), vishalrajv/oh-my-gemini-slim (0), FCAR2025/oh-my-gemini-cli (0),
+ ~10 empties (a notably *dead* stem — most last pushed Mar–May 2026)

**Copilot CLI**: Lee-SiHyeon/oh-my-copilot (10), eugenejahn/oh-my-openagent-copilot (7),
damian87x/oh-my-copilot (5), RobinNorberg/oh-my-copilot (5), odpilot/oh-my-copilot (5),
r3dlex/oh-my-githubcopilot (0)

**Kimi**: wang-h/oh-my-kimi (6), Goblin1024/oh-my-kimi (6), Yorha9e/oh-my-kimi-code (4),
dorname/oh-my-kimi (3), + ~10 empties

**Kilo**: PanPanFR/oh-my-kilo (11), emngny/oh-my-kilocode-slim (7), CrowWizard/oh-my-kilocode (1)

**Qwen**: E4crypt3d/oh-my-qwen (7), chrisxue90/oh-my-qwencode (2), + ~6 empties

**Others, one per host**: MeroZemory/oh-my-droid (28, Factory AI Droid),
1nhann/oh-my-kiro (36), NachoFLizaur/oh-my-kiro (12), qoder-plugins/oh-my-qoder (13),
AndrewWayne/oh-my-warp (12, Rust), hongvincent/oh-my-warp (4),
jiangmuran/oh-my-crush (7, Crush), r3dlex/oh-my-auggie (2, Augment auggie),
luw2007/oh-my-trae (1, Trae), adrianmjim/oh-my-devin (1, Devin CLI),
miketako3/Oh-My-Junie (0), tim-hub/oh-my-zed (1),
haoruilee/oh-my-mcode (51, **MiniMax Code**), devswha/oh-my-gjc (34, **Gajae Code**),
xiaohei-info/oh-my-multica (22, Multica)

**Host-agnostic / multi-agent**: TNG/oh-my-agentic-coder (25, Go, Apache-2.0, sandbox on
seatbelt/bubblewrap/landlock — a corporate-authored one), AbyssCN/oh-my-dag (38, DAG
orchestration + MCP server), cskwork/oh-my-symphony (24), ringlochid/oh-my-subagents (92),
sean2077/oh-my-agents (5), sswym/oh-my-agent (76)

## Catalogs, marketplaces and registries (kind = CATALOG/MARKETPLACE)

| repo | stars | what |
|---|---|---|
| [hashgraph-online/awesome-ai-plugins](https://github.com/hashgraph-online/awesome-ai-plugins) | 129 (142 forks) | Cross-agent plugin list: CC, Codex, Gemini, Antigravity, Pi, Grok, OpenCode. Apache-2.0. Forks > stars = people are submitting entries. |
| [like-study1/Oh-My-DSH](https://github.com/like-study1/Oh-My-DSH) | 78 | Auto-syncs the `dsh-plugin` GitHub topic every 4h into a catalog. **The mechanism to copy.** |
| [NoWint/Oh-My-DSH](https://github.com/NoWint/Oh-My-DSH) | 15 | Same idea, hourly. |
| [wangshunnn/oh-my-dsh](https://github.com/wangshunnn/oh-my-dsh) | 5 | Same idea, auto-updated index. |
| [devswha/oh-my-gjc](https://github.com/devswha/oh-my-gjc) | 34 | "Community plugin marketplace for Gajae Code, Claude Code / Codex compatible" — closest analogue to the cross-format ingestion Muse Code's loader implies. |
| [aws-samples/sample-oh-my-aidlcops](https://github.com/aws-samples/sample-oh-my-aidlcops) | 19 | AWS-authored plugin marketplace extending Claude Code + Kiro. MIT-0. |
| [huangdijia/oh-my-claude-code-plugins](https://github.com/huangdijia/oh-my-claude-code-plugins) | 11 | Claude plugin marketplace |
| [NeuZhou/awesome-ai-anatomy](https://github.com/NeuZhou/awesome-ai-anatomy) | 239 | Source-code teardowns of 15 agents incl. oh-my-codex. Prior art for the reverse-engineering framing. |
| [GulajavaMinistudio/awesome-copilot-id](https://github.com/GulajavaMinistudio/awesome-copilot-id) | 69 | Cross-agent skills/rules/prompts collection |

## Ecosystem tooling worth studying (kind = TOOL)

- [AnPod/Switch-Omo-Config](https://github.com/AnPod/Switch-Omo-Config) (24) and
  [Poorgramer-Zack/omo-switch](https://github.com/Poorgramer-Zack/omo-switch) (26) — *config
  profile switching* emerged independently twice for the same host. Muse Code's
  `.muse/lock.json` + settings subsystem will need this.
- [vyvhouse/oh-my-destructor](https://github.com/vyvhouse/oh-my-destructor) (29) — a
  third-party **uninstaller** for oh-my-claudecode. Ship a clean `uninstall` yourself.
- [oh-my-dsh/dsh-plugin-upgrade-skill](https://github.com/oh-my-dsh/dsh-plugin-upgrade-skill)
  (33, 19 forks) — a skill whose only job is migrating plugins across host versions. Directly
  relevant given Muse Code is at v1.0.1-R2006.1 and moving.
- [hoosin/oh-my-ccenv](https://github.com/hoosin/oh-my-ccenv) (4) — "manage Claude Code
  profiles, like pyenv for Python".
- [netil/oh-my-hi](https://github.com/netil/oh-my-hi) (56) — harness insight dashboard
  cataloguing skills/agents/plugins/hooks across CC + Codex.
- [nextcaicai/oh-my-skills](https://github.com/nextcaicai/oh-my-skills) (26, Rust),
  [AIjunja/oh-my-skills](https://github.com/AIjunja/oh-my-skills) (20),
  [duzhenxun/oh-my-skills](https://github.com/duzhenxun/oh-my-skills) (20) — three
  independent skill install/enable/disable managers. Skill lifecycle is an unsolved,
  repeatedly-attempted problem.
- [haoruilee/oh-my-mcode](https://github.com/haoruilee/oh-my-mcode) (51) — **the single
  closest structural precedent.** An `oh-my-*` npm package that installs a plugin into a
  *separate, third-party host CLI* (`@minimax-ai/code`), explicitly disclaims owning the
  host ("We do not own `mcode`. We are not a bundled host."), auto-installs the host if
  absent, and ships `doctor`, `doctor --smoke`, `doctor --tps` health checks. Read its README
  before designing oh-my-musecode's installer.

## The Muse Code gap

There is **no** `oh-my-*` framework for Meta Muse Code other than the user's own
[hypery11/oh-my-musecode](https://github.com/hypery11/oh-my-musecode) (0 stars, created
2026-09-01, Python, MIT, v0.3.0, topics: muse/muse-code/plugin/hooks/agentic).
`sawasawasawa/oh-my-muse` (0 stars, 2026-05-19, empty) is a dead name-squat, not a competitor.

The entire third-party Muse Code ecosystem found by this sweep is **six repos, max 2 stars**:

| repo | stars | created | what |
|---|---|---|---|
| [agentic-control-plane/muse-code-acp-plugin](https://github.com/agentic-control-plane/muse-code-acp-plugin) | 2 | 2026-08-18 | Policy-checks every tool call before it runs |
| [xhluca/muse-code-openrouter](https://github.com/xhluca/muse-code-openrouter) | 1 | 2026-08-25 | OpenRouter keys/models inside Muse Code |
| [luckeyfaraday/muse-shim](https://github.com/luckeyfaraday/muse-shim) | 1 | 2026-08-06 | Codex OAuth / Anthropic / OpenRouter models inside Muse Code |
| [pinta-ai/pinta-musecode](https://github.com/pinta-ai/pinta-musecode) | 0 | 2026-08-09 | OTLP forwarder for Muse Code **hook events** — confirms `.muse/hooks.json` is externally usable |
| [troioi-vn/muse-skill](https://github.com/troioi-vn/muse-skill) | 0 | 2026-08-23 | Delegate repo work *to* Muse Code from another agent |
| [dttdrv/musecodeapp](https://github.com/dttdrv/musecodeapp) | 0 | 2026-08-06 | Electron GUI for the Muse Code CLI |

Three of six are model/provider shims — the same first-move every other agent ecosystem
made. Nobody has taken the config-framework or catalog position.

## Name collisions to exclude (kind = UNRELATED)

ohmyzsh/ohmyzsh (189k), JanDeDobbeleer/oh-my-posh (23k), oh-my-fish/oh-my-fish (11k),
ohmybash/oh-my-bash (7.6k), xlucn/oh-my-foss-android (5k), Mintimate/oh-my-rime (4.9k),
ipcjs/oh-my-userscripts (4k), git-learning-game/oh-my-git (2.9k), oh-my-ocr/text_renderer
(915), shenhao-stu/ohmycaptcha (868), sirius1024/iterm2-with-oh-my-zsh (1.9k),
shinshin86/oh-my-logo (1.6k), chclt/oh-my-wechat (663), qwq233/OhMyKeymint (313),
OhMyMeme/* (270), ttys3/oh-my-kitty (294), ohmygit-hub/ohmygithub (222),
JuliaFolds2/OhMyThreads.jl (202), kvokov/oh-my-fullstack (174), renatoworks/oh-my-reddit
(142), sonnyp/OhMySVG (125), dougburks/ohmydebn (138), monshunter/ohmykube (35),
kapeka0/OhMyBounty (51), arcsin1/oh-my-ppt (1.9k — AI, but slide generation, not agent config),
hongfamonvAI/oh-my-cover-design (224), TFboy1/oh-my-minimaxh3-director (89),
plus ~40 more dotfiles/theme repos.

**Trap**: `oh-my-droid` is two different projects — MeroZemory/oh-my-droid (28, Factory AI
Droid CLI orchestration) vs tsirysndr/oh-my-droid (32, Android/Termux Linux setup). Same for
`oh-my-design` (two agent design-system packs), `oh-my-hermes` (three), `oh-my-kiro` (four),
`oh-my-skills` (three), `oh-my-warp` (two), `oh-my-cc` (three), `oh-my-dsh` (seven).

## Signals for oh-my-musecode positioning

1. **Nobody is competing yet.** Six repos, ≤2 stars, all narrow shims. The window is open now
   and, per the DSH precedent, closes in days-to-weeks once Muse Code's user base crosses
   some threshold.
2. **The catalog position is the defensible one.** In DSH, the two repos that took the
   auto-synced-catalog role (78 and 15 stars) beat the ones that shipped orchestration
   features. A registry that auto-syncs a `muse-plugin` GitHub topic is a concrete first move.
3. **The `.claude-plugin` / `.codex-plugin` ingestion is the killer feature and it is
   unexploited.** `devswha/oh-my-gjc` (34) is the only repo positioned as "marketplace for
   host X, Claude Code / Codex compatible". Muse Code's loader does this natively — a catalog
   that indexes existing Claude Code and Codex plugins *as installable Muse plugins* inherits
   two mature ecosystems on day one. No competitor can copy this without the same loader.
4. **Follow `oh-my-mcode`'s installer contract**, not oh-my-zsh's. Compiled/third-party host,
   explicit non-ownership disclaimer, optional host bootstrap, and a `doctor` subcommand.
   Muse Code being a single Rust binary makes `doctor` (binary version, `~/.config/muse/auth.json`
   presence, `.muse/lock.json` quarantine state, MSP reachability via `muse serve`) more
   valuable than in any JS-hosted ecosystem.
5. **Plan for lifecycle from v0.** Three of the strongest secondary repos across all
   ecosystems are an uninstaller (29), a version-migration skill (33/19 forks), and profile
   switchers (24+26). `.muse/lock.json` (provenance/quarantine/allowed_tools) and
   `.muse/skills.lock` are exactly the surfaces those tools would need.
6. **Avoid `oh-my-muse`** (already squatted, dead) and prefer `oh-my-musecode`, which is
   already held and matches the host's own product name.

## Files

- `gh-name-sweep/raw2.jsonl`, `raw3.jsonl` — raw JSON per query
- `gh-name-sweep/parsed2.txt`, `parsed3.txt` — flattened per-stem results
- `gh-name-sweep/name_sweep_top.txt` — 76 `oh-my in:name` repos >50 stars
- `gh-name-sweep/buckets.txt` — 224 in 5..50 bucket + 81 `ohmy` bucket
- `gh-name-sweep/detail.sh` — per-repo verification helper
