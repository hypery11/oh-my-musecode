# Discovery sweep: Chinese-language & non-English community

Lens: CN/JP/KR sweep for oh-my-* style frameworks and agent config managers relevant to
**oh-my-musecode** (config/extension framework for Meta Muse Code, a compiled Rust terminal agent).

Method: `gh search` / `gh api search/repositories` (URL-encoded CJK — the `gh search repos`
subcommand silently returns empty for CJK args, the REST API path works), `gh repo view` for
traction, `git clone --depth` + reading real trees/schemas, npm registry API, WebSearch/WebFetch.
All repos below were verified to exist via `gh repo view`. Numbers captured 2026-09-01.

---

## 0. Headline finding: the centre of gravity is not English

Two distinct non-English clusters dominate this space, and they are structurally different:

| Cluster | Naming | Distribution model | Scale |
|---|---|---|---|
| **Korean** | owns the literal `oh-my-*` brand | git repo + npm, curated monorepo | oh-my-claudecode ★38.9k, oh-my-codex ★32.9k, oh-my-openagent ★68.6k |
| **Chinese** | `dsh-*` around DeepSeek Harness | **GitHub topic as the registry** + npm | harness ★207.7k, topic:dsh-plugin = **13,105 repos** |

The decisive quantitative result of this sweep:

```
topic:dsh-plugin          13105     <- Chinese ecosystem, 12 months old
topic:claude-code-plugin   5847
topic:claude-code-skill    2424
topic:codex-plugin         1044
topic:opencode-plugin       690
topic:muse-plugin             0     <- greenfield
```

The Chinese DeepSeek Harness plugin ecosystem is **>2x the size of the Claude Code plugin topic**
and is essentially invisible to English-language search. `topic:muse-plugin` is empty — claiming it
is a zero-cost land-grab for oh-my-musecode.

---

## 1. DeepSeek Harness (DSH) — the Chinese "everything is a plugin" harness

`deepseek-ai/deepseek-harness` ★207,681 f24,156 · created 2026-08-13 · TypeScript · deepseek.com/harness

DeepSeek's own open-source agent harness. Both a runnable coding agent (web + headless) *and*
a framework where models, tools, sandboxes, session stores, UI, and **the agent loop itself** are
plugins. Plugins can extend the official agent or replace its core parts.

### Architectural lineage: Cordis

`cordiverse/cordis` ★7,941 f490 · created **2022-05-17** · TypeScript

The DI / plugin framework underneath DSH — a Chinese-origin project (shigma, Koishi.js lineage)
that predates the whole agent wave by three years. Its homepage now redirects to
`deepseek-harness.github.io/deepseek-harness/reference/cordis-primer`, i.e. DeepSeek adopted an
existing mature CN plugin framework rather than inventing one. Described as "a programming
paradigm for spatiotemporal composability."

**Relevance to oh-my-musecode:** this is the strongest available precedent for the exact problem —
a plugin system where lifecycle/composition is the primitive. Worth reading Cordis directly.

### The `dsh.bundle` manifest (verified, exact)

Distribution is **npm-native**, manifest inline in `package.json` — not a sidecar directory.
From `dsh-market/package.json` (real file, v1.39.0):

```json
{
  "name": "dshmarket",
  "peerDependencies": {
    "@deepseek-ai/cordis": "^4.0.1",
    "@deepseek-ai/dsh-settings": "^0.1.0-rc.7 || ^0.1.1-rc.2 || ^0.1.2-alpha.2",
    "@deepseek-ai/schemastery": "^3.18.1"
  },
  "dsh": {
    "bundle": { "patch": "./cordis.patch.yml" },
    "client": {
      "inject": ["@deepseek-ai/dsh-client-connection", "@deepseek-ai/dsh-client-runtime",
                 "@deepseek-ai/dsh-client-locale", "@deepseek-ai/dsh-client-ui-settings",
                 "@deepseek-ai/dsh-client-ui-theme"],
      "platform": "web"
    }
  }
}
```

Install: `dsh plugin add <name>` / `dsh plugin --profile web add <name>`.
`dsh.bundle` is **required** for installability; `dsh.client` alone fails.
The bundle's payload is a **Cordis config patch YAML** (`cordis.patch.yml`) — i.e. a plugin is
literally a declarative patch against the host's composition graph. Peer-dep version ranges do
host-compatibility gating.

**Contrast worth stealing:** Claude Code distributes via *git repo* + `.claude-plugin/marketplace.json`;
DSH distributes via *npm* + a `package.json` field. Muse's `.muse-plugin/plugin.json` is the former
shape, but npm-style semver peer-dep gating is what makes DSH's 13k-plugin ecosystem survive host churn.

### Notable DSH ecosystem projects (all verified)

| Repo | ★ | What |
|---|---|---|
| `anywhere-labs/dsh-desktop` | 22,714 | Desktop for the DSH plugin ecosystem; "the desktop itself is a plugin" |
| `awesome-dsh-plugin/awesome-dsh-plugin` | 14,029 | The curated list; ships a **`skills-lock.json`** |
| `zhu1090093659/dsh-web` | 6,649 | Web plugin aggregation ecosystem ("Creative Workshop" distribution) |
| `Devin-AXIS/iPolloWork` | 5,226 | Multi-engine workbench: Codex Harness + DSH + OpenCode, **unified plugins & Skills** |
| `zhukunpenglinyutong/desktop-cc-gui` | 4,126 | Tauri multi-engine GUI (CC, Codex, Gemini, OpenCode, DSH) |
| `liustack/modlens` | 3,824 | Vision bridge plugin for text-only agents |
| `dsh-market/dsh-market` | 2,994 | In-agent plugin market, one-click install, theme switching |
| `AdamPlatin123/dsh-plugin-radar` | 1,439 | **Automated curation pipeline** (see §2) |
| `bowenliang123/dsh-context` | 1,228 | Context composition/evolution dashboard |
| `0xsline/awesome-deepseek-harness` | 967 | Second curated catalog |
| `Electricitysheep/dsh-handbook` | 724 | 0→1 handbook: install / plugin dev / tuning (CN + EN PDF) |
| `csyangwen/dsh-memory-evolve` | 266 | Skill self-evolution + skill manager, "zero core modification, uninstall-clean" |
| `bradeGithub/DSH-Plugins-Marketplace` | 152 | Installs **directly from `topic:dsh-plugin`** |
| `awesome-dsh-plugin/dsh-find-plugin` | 111 | In-session conversational plugin discovery |
| `pingfanfan/hello-dsh` | 87 | Zero-basis plugin dev tutorial, 22 Chinese skill examples |
| `like-study1/Oh-My-DSH` | 78 | **Literal oh-my-* for DSH**; auto-syncs the topic every 4h |
| `AwesomeHou/dsh-plugin-marketplace` | 27 | Live-syncs the topic (1800+ repos) into a settings tab |
| `sulfide2085/dsh-skill-manager` | 11 | Manages **DSH / Codex / Claude** skills in one surface |

---

## 2. The three curation architectures worth copying

### 2a. GitHub topic AS the registry (zero infrastructure)

DSH's registry is not a server — it is the public `topic:dsh-plugin` label. At least four
independent clients (`dsh-market`, `DSH-Plugins-Marketplace`, `dsh-plugin-marketplace`,
`dsh-find-plugin`) live-sync from it, and `Oh-My-DSH` re-syncs every 4 hours. The official repo's
only instruction to plugin authors is "add the `dsh-plugin` topic for discoverability."

This scaled to 13,105 repos with no central registry, no accounts, no hosting bill. It is the single
most transferable idea in this sweep. **`topic:muse-plugin` is currently 0.**

### 2b. `dsh-plugin-radar` — facts/verdicts separation (★1,439)

The most sophisticated curation design found anywhere in this sweep. Real schemas read from
`/scratchpad/ohmy/cn-community/radar/schema/`:

- **`event-contract-v4.md`** — core principle, quoted: *"事件是不可变事实，判定是派生视图"*
  ("**events are immutable facts, verdicts are derived views**"). The schema constrains fact
  structure only; it never encodes the verdict. Corrections are new events pointing at old ones via
  a `supersedes: event_id` chain — **the original event is never rewritten**. Stated rationale: when
  the ecosystem's criteria change, you rewrite the *view layer* and never version the contract.
  Three event types: `install_attempt` | `boot_attempt` | `function_probe`.
- **`plugin.schema.json`** — catalog entry keyed by **stable GitHub numeric repo id**
  (`"pattern": "^github:[0-9]+$"`) explicitly because it *"survives rename/transfer"*, plus a
  `previous_names[]` array. Two orthogonal state machines:
  - `curation.state`: `candidate | listed | rejected | removed | blocked`
  - `lifecycle.state`: `active | deprecated | archived | deleted | unknown`
- **`observation.schema.json`** — per-run collected facts, `additionalProperties: false` throughout.
- **`summary.schema.json`** — aggregate counts the README and reports are *generated* from.
- `engine/` split into `discovery | aggregation | distribution | maintenance | ops | rendering`.

Scale claim from its own description: auto-discovers 15,900+ candidates, runtime-tests 10,000+ in
k8s, 15-minute snapshot pipeline; **the plugin directory is an auto-generated artifact**, not a
hand-edited file.

**Relevance:** Muse has `.muse/lock.json` with provenance/quarantine/allowed_tools. Radar's
curation-vs-lifecycle split and immutable-event model is a proven design for exactly that, and its
"never key a catalog on owner/name" lesson is a cheap bug to avoid.

### 2c. `skills-lock.json` — content-hash pinning (from awesome-dsh-plugin, ★14,029)

Real file, verbatim shape — directly analogous to Muse's `.muse/skills.lock`:

```json
{
  "version": 1,
  "skills": {
    "ui-ux-pro-max": {
      "source": "nextlevelbuilder/ui-ux-pro-max-skill",
      "sourceType": "github",
      "skillPath": ".claude/skills/ui-ux-pro-max/SKILL.md",
      "computedHash": "523b8063c07eedd2b8a1ca94d7a0be9ad340026bd9f21a1eda0b6be2a855298f"
    }
  }
}
```

Note it pins `skillPath` **into** another project's `.claude/skills/` tree — i.e. a catalog can
vendor a single skill out of an unrelated repo without forking it, and the hash detects upstream drift.

### 2d. Human-review policy worth reading (awesome-dsh `contributing.md`)

Unusually rigorous, and quotable for oh-my-musecode's own governance:
- *"CI 通过是**前置条件**，不是结论"* — a green CI run is the **precondition, not the decision**.
  CI checks manifest shape, repo age, formatting, README regeneration; it cannot judge whether a
  plugin does what its entry claims. A maintainer reads the target repo before merging.
- **"List the plugins, not the bundle."** A meta-package whose only content is a dependency list
  gets no row — it double-counts the same work in every category it touches.
- A bundle's dependencies **must resolve to the original author's** repo/npm package. Re-uploading
  others' plugins under your own account and depending on the copies is refused: the copies carry
  no fork relationship, no attribution, no upstream, so users get "a silent snapshot of someone
  else's work with the author's name kept only in the package name."
- Explicit anti-ranking stance: *"本列表不给插件排名，也不评判优劣"*.
- Explicit security disclaimer: listing ≠ security review; plugins run with full user privileges and
  **tool-approval does not cover the plugin's own code**. (Directly relevant to Muse's
  `allowed_tools`/quarantine design — the CN ecosystem has already learned this the hard way.)

---

## 3. Korean cluster — owns the `oh-my-*` brand

Surfaced via English names, so easy to mistake for Western projects. All Korean-authored.

| Repo | ★ | Note |
|---|---|---|
| `code-yeongyu/oh-my-openagent` (OmO/lazycodex) | 68,585 | READMEs in **en/ko/ja/zh-cn/ru**. Explicitly mid-**"multi-harness agent OS refactor"** to support OpenCode, Codex, Pi, etc. |
| `Yeachan-Heo/oh-my-claudecode` | 38,933 | Teams-first multi-agent orchestration |
| `Yeachan-Heo/oh-my-codex` (OmX) | 32,945 | hooks, agent teams, HUDs |
| `alvinunreal/oh-my-opencode-slim` | 8,558 | Lean fine-tuned variant — the "slim fork" pattern |
| `sangrokjung/claude-forge` | 821 | Self-describes as "**oh-my-zsh for Claude Code**" |
| `witt3rd/oh-my-hermes` | 302 | oh-my-claudecode patterns ported to Hermes |
| `happycastle114/oh-my-openclaw` | 185 | oh-my-opencode patterns ported to OpenClaw |

`oh-my-openagent`'s ROADMAP-driven multi-harness refactor is the closest living analogue to what
oh-my-musecode faces: one asset library, many host agents. Its README states the thesis plainly —
*"未来不是选一个赢家，而是把所有赢家编排到一起"* ("the future isn't picking one winner, it's
orchestrating all of them together").

---

## 4. "Oh My Pi" — definitive disambiguation

Four readings exist; only one is the real referent.

1. **✅ `can1357/oh-my-pi` — ★28,856 f2,884, TypeScript, home `omp.sh`.** CLI name **`omp`**.
   Coding agent "with the IDE wired in." Forked from Mario Zechner's pi-mono. npm
   `@oh-my-pi/pi-coding-agent` **v18.1.0, 447,565 downloads/month** (verified via registry.npmjs.org;
   `repository.url` = `git+https://github.com/can1357/oh-my-pi.git`). Author Can Bölük.
   Claims 60+ providers, 31 tools, 14 LSP ops, 28 DAP ops, ~80k lines of Rust core.
   **This is what "oh my pi" means.**
2. **The upstream it forks:** `badlogic/pi-mono` → now redirects to **`earendil-works/pi` ★100,375**.
   "Pi" alone = this base agent toolkit. npm `@mariozechner/pi` = only 2,608 downloads/month, i.e.
   the fork dwarfs the upstream in CLI usage.
3. **`oh-my-pi/omp` — NOT_FOUND.** GraphQL: *"Could not resolve to a Repository."* This URL is cited
   as the project home by `sakuradairong/omp-config`'s README — a **stale/incorrect link in a real
   repo**. Flagging because it is exactly the kind of plausible-looking dead reference that becomes
   a hallucinated citation downstream.
4. **`oh-my-open-pi`** (pi.dev registry, author `takltc`, v0.1.4, 121 downloads/mo) — a *different,
   much smaller* thing: an extension package **for** Pi, not the harness. Confusingly near-identical name.
5. Raspberry Pi: no meaningful collision in this space.

**Bonus discovery — `pi.dev` is a first-party package registry for Pi.** Install is
`pi install npm:oh-my-open-pi` (npm-backed), manifest declares asset *arrays*:

```json
{ "extensions": ["./src/index.ts"], "skills": ["./skills"], "prompts": ["./prompts"] }
```

Config split global/project: `~/.pi/agent/<name>.jsonc` and `.pi/<name>.jsonc` — **JSONC, with
comments**, a nicety Muse's strict `.json` files lack.

Satellite ecosystem confirms omp is the referent: `ifiokjr/monopi` ★149 ("Like oh-my-zsh for pi"),
`mcbarlowe/omp-deck` ★21, `mikeatlas/omp-sbx` ★26, `DeprecatedLuke/oh-my-singularity` ★24,
`czottmann/pi-automode` ★106.

**CN link:** `sakuradairong/omp-config` ★44 — "Oh My Pi (OMP) 配置合集", a de-identified reusable
dump of one developer's entire `~/.omp/`: 18 global skills (one containing **249** ECC skills),
**70 sub-agent** definitions, `config.yml`/`models.yml`/`mcp.json`, and pre/post hooks in
TypeScript. Notable for model-compat shimming as *declarative config* — `models.yml` carries
`maxTokensField: max_tokens`, `requiresAssistantContentForToolCalls: true`,
`supportsDeveloperRole: false`, `extraBody: thinking:{type:enabled}` to make DeepSeek V4 work
through an OpenCode proxy. This "config as compatibility shim" pattern is worth noting.

---

## 5. Chinese Claude Code config/lifecycle managers

### Directly relevant to packaging/lifecycle (read the source)

- **`Garretqaq/skill-hub` ★2** — *"git 仓库形态的 Claude Code 插件市场"*. Low stars, high design
  value. Thesis: **the git repo is the single source of truth, zero database** — explicitly to avoid
  "database vs files" desync. Uploads a zip → parses frontmatter → normalises into
  `plugins/<name>/skills/<skill>/` → rewrites `.claude-plugin/marketplace.json` → commit & push.
  Reads the manifest via `git show HEAD:.claude-plugin/marketplace.json` (never the working tree),
  and every mutation goes through a `withWorkTree()` helper — clean concurrency model.
  `src/lib/` has dedicated `semver.ts`, `watched.ts` (upstream repo watching + update detection),
  `githubProxy.ts` (**China-access mirror acceleration**), `ingest.ts`, `worktree.ts`.
  Its `PluginEntry` carries `origin`, `exclude[]` (top-level entries stripped at import, remembered
  for later updates) and `sourceHash` (fallback drift detection for unversioned packages).
- **`yizhiyanhua-ai/skills-updater` ★173** — the lifecycle manager. Scans
  `~/.claude/plugins/installed_plugins.json`, compares **local vs remote git commit SHAs** (not
  semver — most skills have no version), reports `up_to_date | update_available | unknown_version |
  error`, then auto-reinstalls affected skills after a marketplace update. Ships
  `references/marketplaces.md` as a **plain-markdown registry of registries** (anthropics/skills,
  claude-plugins-official, daymade, obra/superpowers-marketplace, skills.sh, skillsmp.com).
  Bilingual runtime (`i18n.py` auto-detects locale) and explicit Windows console UTF-8 fixes.
- **`shetengteng/skillix-hub` ★5** — an **npm-like package manager for skills** ("自带类 npm 的包
  管理器 Skill Store") with natural-language install, **dependency resolution**, and session
  auto-update. Tiny, but the only project found that treats skill *dependencies* as first-class.
- **`huangrichao2020/pretty-skills` ★54** — cross-agent skill manager + skill promotion tooling.
- **`icysaintdx/OpenCode-Config-Manager` (OCCM) ★396 f27** — Fluent-Design GUI + NiceGUI web (17
  pages) for OpenCode **and Oh My OpenCode**. Manages Provider/Model/MCP/Agent/permissions, JSONC
  with comments, startup config validation with one-click repair, and **one-click import from
  Claude Code / Codex / Gemini configs**. Bilingual via `locales/{zh_CN,en_US}.json`. Cross-platform
  PyInstaller builds. The "config importer" angle matters: Muse already ingests `.claude-plugin` and
  `.codex-plugin`, so a migration surface is expected by users in this ecosystem.
- **`ronghuaxueleng/claude-code-config-manage-gui` ★252** — Tauri/**Rust** visual config manager,
  Windows-first. Multi-account/endpoint switching, WebDAV sync, directory management.
  Relevant precedent: a Rust GUI companion to a coding agent.
- **`ZDragon17/oh-my-claude` ★5** — *"基于中国传统文化的 Claude Code 智能编排插件."* Only 5 stars but
  **architecturally the single closest match to oh-my-musecode's core problem.** It ships a
  provider-abstraction layer that emits *multiple host manifests from one codebase* — a
  `codex-plugin.json` (`"provider": "codex"`, hooks keyed `before_tool`/`after_tool`/`on_message`/
  `on_start`/`on_stop`) alongside Claude-Code-shaped `hooks.json` and `skill.json`. `provider/`
  contains `interface.ts`, `registry.ts`, `claude-code.ts`, `codex.ts`, `generic.ts`. The registry
  resolves in a strict order: **explicit config override → auto-detect (first match; generic never
  auto-detects) → generic fallback**. It normalises hook event names across hosts
  (`preToolUse | postToolUse | userPromptSubmit | sessionStart | stop`) and abstracts model tiers
  (`flagship | balanced | fast | cheap`). Given Muse ingests `.claude-plugin` *and* `.codex-plugin`,
  this exact pattern — one asset source, N manifest emitters, runtime host detection — is the shape
  oh-my-musecode should adopt.

### Catalogs / distribution (CN)

- `xu-xiang/everything-claude-code-zh` ★1,923 f313 — CN translation of everything-claude-code
- `lhfer/claude-howto-zh-cn` ★2,293 — CN onboarding guide with localisation guardrails
- `cfrs2005/claude-init` ★1,364 f124 — CN dev kit, one-click install, MCP + "免翻墙" (no-VPN) access
- `laolaoshiren/claude-code-skills-zh` ★810 — curated + original CN skills, "复制即装"
- `xianyu110/awesome-claudcode-tutorial` ★598, `Raymondhou0917/claude-code-resources` ★291 (TW)
- `zrt-ai-lab/opencode-skills` ★276 f72 — OpenCode/Claude Code skill library
- `KimYx0207/findskill` ★109 — **Windows-compatible** skill search (fixes `npx skills` empty output)
- `Xueheng-Li/sysu-awesome-cc` ★59 — Sun Yat-sen University, faculty+student maintained
- `codelably/harmony-claude-code` ★42, `Gdenian/claude-code-cn-plus` ★23
- Long tail of `<person>的 Claude Code 插件市场` personal marketplaces (★1–5) — the git-repo
  marketplace format is cheap enough that individuals run their own.

### Domain skill packs (evidence of demand shape)

`vivy-yi/xiaohongshu-skills` ★411 (139 Xiaohongshu ops skills), `Cuimao777/cuimao-translator` ★476
(EN→CN PDF), `lc2panda/claude-plugin-wechat` ★60 (WeChat/Lark bridge with **remote permission
approval**), `linhut/gongwen-skill` ★30 (GB/T 9704 Chinese government document standard),
`dhicoc/dsh-chinese-traditional-wisdom-skill` ★31. CN demand skews to non-coding
domain workflows far more than the English ecosystem — worth knowing for curation defaults.

---

## 6. Muse Code is already on this ecosystem's radar

- **`xhluca/session-migrate` ★76** — "Migrate coding agent sessions across 18 harnesses." README
  line 113 links **`https://dev.meta.ai/`** with a Muse Code logo, and it ships
  `docs/muse-qwen-kimi-formats.md`. Independent third-party confirmation that Muse Code is real,
  shipping, and already treated as a first-class harness by cross-agent tooling.
- CN coverage already exists: `muse-code.dev` (independent guide), sshmac.com install walkthrough,
  ai-indeed.com "Muse Code 中国能用吗？" (access/billing analysis — notes it is cloud-API dependent,
  needs Meta dev account or OpenRouter, and is *not* offline-deployable). Install is documented as
  `curl -fsSL https://dev.meta.ai/install.sh | bash`.
- Cross-harness aggregators already enumerate Muse alongside Pi/OMP/OpenClaw/Hermes:
  `AVIDS2/memorix` ★710, `magnitudedev/magnitude` ★1,542, `Anionex/agent-vision-toolkit` ★1,133 (CN),
  `NeuZhou/awesome-ai-anatomy` ★239 (CN, source teardowns of 15 agents).

**Implication:** the "one asset library, many harnesses" integration slot is where CN/KR projects
are converging. oh-my-musecode should assume it will be *consumed by* these aggregators, and ship a
machine-readable capability manifest accordingly.

---

## 7. Transferable conclusions for oh-my-musecode

1. **Claim `topic:muse-plugin` now** (currently 0 repos). GitHub-topic-as-registry scaled DSH to
   13,105 plugins with zero infrastructure and multiple independent client implementations.
2. **Adopt radar's fact/verdict split** for `.muse/lock.json`: immutable observation events with a
   `supersedes` chain, verdicts as derived views, and two orthogonal state machines
   (`curation`: candidate/listed/rejected/removed/blocked; `lifecycle`:
   active/deprecated/archived/deleted/unknown).
3. **Key catalog entries on stable numeric repo ids**, never `owner/name` — with `previous_names[]`.
   Renames and transfers are the common failure the CN catalogs designed around.
4. **`.muse/skills.lock` should carry `{source, sourceType, skillPath, computedHash}`** per skill.
   `skillPath` pointing *into* a foreign repo lets you vendor one skill without forking; the hash
   catches upstream drift. SHA-comparison beats semver here — most skills are unversioned.
5. **Emit multiple host manifests from one asset source** (ZDragon17's provider registry). Muse
   reads `.muse-plugin` + `.claude-plugin` + `.codex-plugin`; resolution order should be
   explicit-override → auto-detect → generic fallback, with normalised hook-event names.
6. **Use semver peer-dep ranges for host-compat gating**, and make the plugin *self-disable with a
   stated reason* on an unsupported host rather than rendering against missing primitives
   (dsh-market does exactly this, and its README shows it is the #1 support question).
7. **Governance text is a feature.** "CI is the precondition, not the decision"; "list the plugins,
   not the bundle"; dependencies must resolve to the original author; listing ≠ security review;
   plugin code is *not* covered by tool-approval. Muse's `allowed_tools`/quarantine should be
   documented against that last point explicitly.
8. **Generate the catalog, don't hand-edit it** (radar: directory is an auto-generated artifact;
   Oh-My-DSH: 4-hour auto-sync; awesome-dsh: `generate-readme.mjs` + probe-* scripts for
   stars/downloads/screenshots/readmes/tarballs/updates/decay).
9. **China-access is a real design constraint**: skill-hub ships `githubProxy.ts` mirror
   acceleration, claude-init advertises "免翻墙". Any installer that assumes raw GitHub reachability
   will silently fail for a large share of this audience.
10. **JSONC over JSON** for user-facing config (Pi and OCCM both do it); and budget for Windows
    console UTF-8 handling — multiple CN projects exist *solely* to fix Windows/encoding breakage
    (`findskill`, skills-updater's stdout rewrapping).

---

## Working files

Clones and read sources under
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/cn-community/`:
`radar/` (schemas), `awesome-dsh/` (skills-lock.json, contributing.md), `dsh-market/`,
`skill-hub/`, `skills-updater/`, `omo/`, `omp-config/`, `OpenCode-Config-Manager/`,
`oh-my-claude/` (ZDragon17 provider layer), `claude-init/`, `findskill/`,
`claude-code-config-manage-gui/`.

## NOT_FOUND / corrections

- `oh-my-pi/omp` — does not exist (stale link inside `sakuradairong/omp-config`'s README).
- `shigma/cordis` — redirects to `cordiverse/cordis`.
- `badlogic/pi-mono` — redirects to `earendil-works/pi`.
- `gh search repos` with CJK arguments returns empty rather than erroring; use
  `gh api search/repositories?q=<urlencoded>` instead. Cost me one false "no results" round.
