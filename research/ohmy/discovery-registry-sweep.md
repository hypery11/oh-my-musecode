# Discovery — Package Registry + Marketplace Sweep
Lens: where framework-shaped agent-extension projects actually *distribute*.
Date of sweep: 2026-09-01. All numbers pulled live via npm registry API, npm downloads API,
PyPI JSON API, formulae.brew.sh API, and `gh api` / `gh search`. Repos were cloned and read.

Working dir: `/private/tmp/.../scratchpad/ohmy/registry-sweep/` (clones in `clones/`).

---

## 0. Headline conclusions for oh-my-musecode

1. **npm is the de-facto distribution channel for "oh-my-*" agent frameworks — even for
   agents that are compiled binaries and not JS.** Every major one ships an npm package with a
   global `bin`. This is the single strongest transferable pattern.
2. **Muse Code is absent from every multi-agent installer I checked.** `vercel-labs/skills`
   (77 agents), `withastro/rosie` (~60 agents), `rulesync`, `agentwheel`, `add-mcp` — none
   know about `muse`. Adding a `muse` adapter to `skills` + `rosie` is a cheap, high-leverage
   distribution win: it makes every existing skill repo installable into Muse on day one.
3. **`.claude-plugin/marketplace.json` is a de-facto ecosystem standard** with a huge installed
   base (32k+ files in GitHub code search; official Anthropic directory alone lists 2,282 plugins).
   Since Muse's loader already recognises `.claude-plugin`, oh-my-musecode should *consume*
   this format rather than invent one.
4. **`agentskills.io` is a real open standard** (`agentskills/agentskills`, 24,929 stars) with a
   published spec and an explicit "For client implementors → Adding skills support" path plus a
   Client Showcase. Muse is not listed. Registering Muse as a conformant client is the
   standards-track version of point 2.
5. **The closest single precedent to build from is `pi-claude-marketplace`** — a non-Claude agent
   (Pi) consuming Claude Code marketplaces, with a *desired-state* config file for repeatable,
   shareable installs. That is almost exactly oh-my-musecode's brief.

---

## 1. The "oh-my-*" agent framework family (direct naming precedents)

Verified live via `gh api repos/...` + npm downloads API.

| Repo | Stars | Forks | npm package | Weekly DL | Last publish | Target agent |
|---|---:|---:|---|---:|---|---|
| code-yeongyu/oh-my-openagent | 68,585 | 5,630 | `oh-my-opencode` (bins: oh-my-opencode, oh-my-openagent, lazycodex) | 19,970 | 2026-08-01 | OpenCode / multi |
| Yeachan-Heo/oh-my-claudecode | 38,933 | 3,492 | `oh-my-claude-sisyphus` (bins: omc) | 5,759 | 2026-08-31 | Claude Code |
| Yeachan-Heo/oh-my-codex | 32,945 | 2,534 | `oh-my-codex` (bin: omx) | 4,021 | 2026-09-01 | Codex CLI |
| can1357/oh-my-pi | 28,857 | 2,884 | `@oh-my-pi/pi-coding-agent` (+~20 scoped pkgs) | 113,949 | 2026-09-01 | Pi (own agent) |
| alvinunreal/oh-my-opencode-slim | 8,560 | 510 | `oh-my-opencode-slim` | 16,050 | 2026-09-01 | OpenCode |
| sangrokjung/claude-forge | 821 | 176 | — (shell) | — | — | Claude Code |
| happycastle114/oh-my-openclaw | 185 | 24 | — | — | — | OpenClaw |
| TechDufus/oh-my-claude | 176 | 7 | — | — | — | Claude Code |
| tmcfarlane/oh-my-cursor | 108 | 14 | — (shell) | — | — | Cursor |
| scalarian/oh-my-codex | 73 | 18 | `oh-my-codex-workspace` | n/a | 2.0.0 | Codex CLI |
| baekenough/oh-my-customcode | 34 | 6 | `oh-my-customcode` (bin: omcustom) | 1,001 | 2026-08-30 | Claude Code |
| vanducng/oh-my-dsh | 0 | 0 | `@vanducng/oh-my-dsh` | ~460/mo | 2026-08-27 | DeepSeek Harness |
| Y-Square-T3/oh-my-codes | 0 | 0 | `@y-square-t3/oh-my-codes-*` | ~330/mo | 2026-08-25 | OpenCode |
| dmae97/omk (was oh-my-kimi) | 142 | 13 | `@oh-my-kimi/cli` | ~360/mo | 2026-05-22 | multi |

Naming note: the `oh-my-` + two/three-letter-alias convention is well established
(`omc`, `omx`, `omo`, `omp`, `omcustom`, `omdsh`). `omm` / `oh-my-musecode` fits the family and
appears unclaimed on npm.

### Distribution mechanics observed
- **npm global bin** — dominant. `npm i -g <pkg>` then `<alias> init`.
- **curl | bash** — `claude-forge` ships `install.sh` + `install.ps1` (true oh-my-zsh style),
  and also supports `git pull` updates and git submodules (`.gitmodules`).
- **Native plugin command** — `claude-forge` is *also* installable via Claude Code's own
  `/plugin install claude-forge`, i.e. it registers as a plugin in the host agent's marketplace.
  Three parallel channels for one project.

### Best lifecycle model found: `oh-my-customcode`
Small (34 stars) but the most directly transferable engineering. Read at
`clones/oh-my-customcode/`.
- CLI verbs: `omcustom init`, `update`, `list [agents]`, `doctor`, `doctor --fix`.
- Ships a **lockfile** `.omcustom.lock.json`:
  ```json
  { "lockfileVersion": 1, "generatorVersion": "1.1.57", "templateVersion": "1.1.57",
    "generatedAt": "...",
    "files": { ".claude/rules/MUST-orchestrator-coordination.md":
      { "templateHash": "<sha256>", "size": 67943, "component": "rules" } } }
  ```
  Per-file `templateHash` + `size` + `component` = drift detection, safe update, and
  "did the user hand-edit this?" — maps directly onto Muse's `.muse/skills.lock`.
- npm `files: ["dist","templates"]` — templates shipped in the tarball, materialised on `init`.
- Philosophy is "compiled, not configured": skills are source, agents are build artifacts.

---

## 2. Claude Code plugin marketplaces (the format Muse already ingests)

`.claude-plugin/marketplace.json` — GitHub code search reports **32,192** matching files.
Notable public marketplaces, verified live:

| Repo | Stars | Plugins | Notes |
|---|---:|---:|---|
| anthropics/skills | 172,947 | — | Official Agent Skills repo |
| anthropics/claude-plugins-official | 35,785 | — | Official Anthropic-managed directory |
| wshobson/agents | 39,326 | 93 | **Multi-harness**: Claude Code, Codex, Cursor, OpenCode |
| anthropics/claude-plugins-community | 3,127 | **2,282** | Official community marketplace; 1.5 MB manifest |
| phuryn/pm-skills | 25,873 | 100+ | PM/product vertical |
| NanmiCoder/cc-haha | 14,259 | — | Desktop workspace + marketplace browser |
| davepoon/buildwithclaude | 3,401 | — | Discovery hub (skills/agents/commands/hooks/plugins/MCP) |
| jeremylongshore/tons-of-skills-marketplace | 2,687 | 471 | 3,069 skills, 347 agents |
| daymade/claude-code-skills | 1,367 | — | Curated |
| obra/superpowers-marketplace | 1,237 | 10 | Curated; uses `strict: true` |
| fivetaku/gptaku_plugins | 1,095 | — | — |
| numman-ali/n-skills | 1,040 | 5 | Cross-agent: Claude Code, Codex, openskills |
| ananddtyagi/cc-marketplace | 688 | — | — |
| anthropics/life-sciences | 584 | — | Official *vertical* marketplace |
| Piebald-AI/claude-code-lsps | 515 | — | LSP servers as plugins |
| trailofbits/skills-curated | 496 | 29 | Security-vendor curated + vetted |

### marketplace.json schema (read from real manifests)
Top level: `name`, `owner{name,email,url}`, `metadata{description,version,generated_at,...}`,
`plugins[]`, and optionally `renames{old:new}` (Anthropic's community manifest uses this for
stable renaming — a real lifecycle concern worth copying).

`plugins[]` entries are polymorphic on `source`:
```jsonc
// (a) relative path inside the marketplace repo
{ "name":"code-documentation", "source":"./plugins/code-documentation",
  "description":"...", "version":"1.2.1",
  "author":{"name":"...","email":"..."}, "homepage":"...", "license":"MIT",
  "category":"documentation" }

// (b) external git URL, optionally SHA-PINNED
{ "name":"0x", "description":"...",
  "source":{"source":"url","url":"https://github.com/0xProject/0x-ai.git",
            "sha":"0167bbb411cc972b966127d23c23de801061fa99"},
  "homepage":"..." }

// (c) strict mode
{ "name":"superpowers", "source":{"source":"url","url":"..."}, "strict":true }
```
**The SHA pin in the official Anthropic manifest is the key supply-chain detail** and lines up
exactly with Muse's `.muse/lock.json` provenance/quarantine/allowed_tools design.

---

## 3. Cross-agent installers / package managers (the "one source, every agent" layer)

| Tool | npm weekly | Stars | Last publish | Shape |
|---|---:|---:|---|---|
| `skills` (vercel-labs/skills) | **9,363,514** | 30,166 | 2026-08-18 | `npx skills add owner/repo` |
| `rosie-skills` (withastro/rosie) | 352,456 | 156 | 2026-06-29 | Rust binary, "npm but for skills" |
| `rulesync` (dyoshikawa/rulesync) | 289,270 | 1,371 | 2026-09-01 | rules/config compiler |
| `add-mcp` (neon-solutions) | 148,747 | 291 | 2026-08-24 | MCP config tool, bundles 1,380-server registry |
| `pi-claude-marketplace` | 614 | 21 | 2026-08-30 | **Claude marketplaces → Pi** |
| `claudepluginhub` | 429 | — | 2026-08-02 | repo NOT_FOUND on GH (site 308) |
| `contextmux` | 405 | 1 | 2026-08-31 | one rules source → CLAUDE.md/AGENTS.md/.cursor/rules |
| `@gobing-ai/superskill` | 381 | 5 | 2026-08-27 | skill/command/subagent/hook/MCP manager |
| `opencode-agent-skills` | 372 | 271 | 2026-05-09 | skills plugin for OpenCode |
| `agentwheel` (NestDevLab) | 319 | 8 | 2026-08-31 | one source → every agent |
| `claude-code-marketplace` | 146 | 2 | 2026-08-22 | web dashboard over marketplaces |

### `vercel-labs/skills` — the reach leader
- 9.36M weekly downloads; supports **77 agents** via a table of project + global paths.
- Source resolution: `owner/repo` shorthand, full GitHub URL, deep tree URL, GitLab, any git URL,
  local path, private repos via git credential helper → `gh repo clone` → SSH fallback.
- Install modes: symlink by default, `--copy` opt-out; `-g/--global`; `-a/--agent`; `-s/--skill`.
- Auto-detects installed agents.
- **No `muse` entry.** Adapter would be a table row + path pair, e.g.
  `muse` → `.muse/skills/` (project) and `~/.config/muse/skills/` (global).
- Note it already maps Codex → `.agents/skills/` global `~/.codex/skills/`, and several agents
  share the neutral `.agents/skills/` convention.

### `withastro/rosie` — closest *architectural* analogue to oh-my-musecode
- **Rust binary** (like Muse itself), cross-platform, "npm, but for skills".
- Multi-channel distribution: `npx rosie-skills`, `brew tap withastro/rosie && brew install rosie`,
  plus apt / AUR / FreeBSD pkg / build-from-source. Source tree has `npm/`, `aur/`, `debian/`,
  `freebsd-package/` directories — a ready-made packaging blueprint.
- Lockfile at `.agents/rosie.lock`; typed JS API (`import * as rosie from 'rosie-skills'`).
- Source modules of interest: `src/lockfile.rs`, `src/resolve.rs`, `src/audit.rs`,
  `src/sanitize.rs`, `src/link.rs`, `src/agent.rs`, `src/agentsmd.rs`.
- ~60 agents in `src/agent.rs`; includes `omp` (oh-my-pi) and `pi`. **No `muse`.**

### `pi-claude-marketplace` — the single best template for the brief
Read at `clones/pi-claude-marketplace/`.
- Lets **Pi** install plugins from **Claude** marketplaces. Supports commands, skills, agents,
  hooks (partial), MCP servers — and is explicit that "plugins containing unsupported components
  can be partially installed", which is the honest UX for cross-ecosystem ingestion.
- `/claude:plugin` command mirrors Claude Code's `/plugin` (bootstrap, list --available,
  install `<plugin>@<marketplace>`, marketplace add `<owner/repo>`).
- **Desired-state config** at `[~/].pi/agent/claude-plugins[.local].json` makes installs
  automatic, repeatable and shareable across machines/teams — the `.local` split is the
  personal-vs-team override pattern worth copying.
- Capability dependencies are declared and delegated: agents need `pi-subagents`,
  MCP needs `pi-mcp-adapter`.

---

## 4. How each agent ecosystem distributes extensions today

| Agent | Official registry? | Manifest | Distribution |
|---|---|---|---|
| **Claude Code** | Yes — `anthropics/claude-plugins-official` (35,785★) + `claude-plugins-community` (3,127★, 2,282 plugins) | `.claude-plugin/marketplace.json`, `plugin.json` | Git repos added as marketplaces; `/plugin install <name>@<marketplace>`; SHA-pinnable sources |
| **Gemini CLI** | Yes — **`gemini-cli-extensions` GitHub org, 66 repos** (conductor 3,719★, nanobanana 1,124★, security 790★) | `gemini-extension.json` | Git repo per extension; manifest has `name`, `version`, `contextFileName`, `mcpServers`, `${extensionPath}` expansion |
| **OpenCode** | No formal registry — curated markdown table in docs (`ecosystem.mdx`) + `awesome-opencode` + opencode.cafe | npm package | **npm names listed in `opencode.json` `plugin: []` array**, auto-installed with Bun at startup, cached in `~/.cache/opencode/node_modules/`; or drop JS/TS in `.opencode/plugins/` or `~/.config/opencode/plugins/` |
| **Codex CLI** | No plugin marketplace found in-repo. Has `docs/skills.md` (points to developers.openai.com/codex/skills) and `docs/slash_commands.md` | AGENTS.md / skills | Skills at `~/.codex/skills/` & `.agents/skills/`; third-party "codex plugins" on npm are ad-hoc, not a first-party registry |
| **Crush** (charmbracelet, 27,848★) | No registry | `crush.json` + `schema.json` | **Implements the agentskills.io open standard**; discovers skills from `$CRUSH_SKILLS_DIR`, `~/.config/agents/skills/`, `~/.config/crush/skills/`, `~/.agents/skills/`, **and `~/.claude/skills/`** — i.e. it deliberately ingests Claude's directory, same trick as Muse. Not in Homebrew core (own tap). |
| **Amp** | No plugin registry found | — | Homebrew formula `amp` v0.7.1; uses neutral `.agents/skills/` |
| **Cursor** | No CLI plugin registry | `.cursor/rules` | Rules files; skills at `.agents/skills/` + `~/.cursor/skills/` |
| **MCP (cross-cutting)** | Yes — `modelcontextprotocol/registry` (7,209★); schema `https://static.modelcontextprotocol.io/schemas/2025-12-11/server.schema.json` (HTTP 200) | server.schema.json | Registry API endpoints returned 404/000 from this environment — **traction numbers UNKNOWN, not verified**. `add-mcp` vendors a 1,380-server snapshot of it. |
| **Agent Skills standard** | `agentskills.io` + `agentskills/agentskills` (24,929★) | `SKILL.md` | Spec + Client Showcase + "Adding skills support" implementor guide. **Muse not listed.** |

---

## 5. Registry-by-registry notes

### npm
The centre of gravity. Everything ships here, including Rust binaries (rosie) and
frameworks for compiled agents. Relevant scale markers: `skills` 9.36M/wk,
`@anthropic-ai/claude-code` 21.4M/wk, `@openai/codex` 20.4M/wk,
`@oh-my-pi/pi-coding-agent` 114k/wk. Platform-specific binary subpackages
(`@anthropic-ai/claude-code-darwin-arm64` etc.) are the standard way to ship a compiled
binary through npm — directly applicable if oh-my-musecode ever ships a Rust helper.

### PyPI
Thin and secondary for this space. Real entries: `superclaude` 4.3.0 (2026-03-22,
SuperClaude-Org/SuperClaude_Framework), `claude-code-sdk` 0.0.25, `specify-cli` 1.0.2
(GitHub Spec Kit), `claude-monitor` 4.0.0, `cchooks` 0.1.5, `claude-skills` 0.0.1,
`ai-rules` 0.3.0, `rulesync` 1.0.0 (different project from the npm one — obielin/agentsync).
Caveat: PyPI's JSON API emits unescaped control characters that break `jq`; parse with
Python `json.loads(..., strict=False)`.
`simonw/llm` (PyPI `llm` 0.33) is worth noting as a *design* precedent — its plugin system
is literally "pip install into the tool's env", the cleanest registry-as-plugin-host model.

### Homebrew
Agents themselves are here, extensions are not. Verified: `claude-code` **cask** v2.1.236,
`codex` **cask** v0.152.0, `opencode` formula v1.18.20, `gemini-cli` formula v0.46.0,
`amp` formula v0.7.1, `aider` formula v0.86.2. `crush` is NOT in core (own tap).
Precedent for a framework: `brew tap withastro/rosie && brew install rosie`.
So the pattern for oh-my-musecode is a **personal tap**, not core.

---

## 6. Concrete opportunities identified

1. Publish `oh-my-musecode` to npm with bin alias `omm`; the name and alias appear free.
2. Send adapter PRs to `vercel-labs/skills` (9.36M wk) and `withastro/rosie` adding `muse` —
   two small table/enum additions that unlock the entire existing skills corpus for Muse.
3. Register Muse in the `agentskills.io` Client Showcase via the "Adding skills support" guide.
4. Consume `.claude-plugin/marketplace.json` directly (Muse already recognises the dir),
   following `pi-claude-marketplace`'s partial-install honesty and desired-state config file.
5. Copy `oh-my-customcode`'s lockfile shape (per-file `templateHash`/`size`/`component`) for
   `.muse/skills.lock` drift detection, and the official manifest's `sha` pinning + `renames`
   map for `.muse/lock.json` provenance.
6. Adopt rosie's multi-channel packaging layout (`npm/`, `debian/`, `aur/`, `freebsd-package/`)
   if a compiled helper is ever needed.

## 7. Not verified / open
- MCP Registry live API unreachable from this environment (404 on documented paths, 000 on
  retries). Repo and schema URL confirmed; server counts UNKNOWN.
- `ClaudePluginHub/claudepluginhub` GitHub repo returns 404 although the npm package
  (`claudepluginhub`, 429/wk) and claudepluginhub.com (HTTP 308) exist — repo is private,
  renamed, or deleted.
- agentskills.io Client Showcase list is client-side rendered; could not enumerate members.
- `sourcegraph/amp` is not the right repo path for Amp (404); Amp's source location unconfirmed.
