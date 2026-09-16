# oh-my-* for AI Coding Agents — Comparative Analysis
### What `oh-my-musecode` must be, derived from 14 verified teardowns + 4 discovery sweeps

**Date:** 2026-09-01 · **Target:** Meta Muse Code v1.0.1-R2006.1 (compiled Rust binary, `dev.meta.ai`)
**Method:** every project below was cloned and read at source level. Traction is live (`gh api`,
npm registry API, `api.npmjs.org/downloads`). Every claim here survived an independent
verification pass; refuted claims have been deleted and corrected claims appear in corrected form.
Sources: `ohmy/*.md` (14 teardowns), `ohmy/discovery-*.md` (4 sweeps).

---

## 0. The one-paragraph answer

The `oh-my-*` niche for coding agents is **saturated by name and empty by substance**. The pattern has
been applied to essentially every terminal coding agent that exists — Claude Code, Codex, OpenCode, Pi,
Gemini CLI, Copilot, Cursor, Kilo, Qwen, Kimi, Grok Build, Antigravity, OpenClaw, Hermes, DeepSeek
Harness, Droid, Kiro, Qoder, Trae, Warp, Crush, Auggie, MiniMax Code, Devin, Gajae Code, Multica —
**except Meta Muse Code**. Across ~300 repos matching the name and 14 deep teardowns, **not one project
has all four of: a working uninstall, a content lockfile something actually reads, an override/overlay
mechanism, and a third-party registry.** The closest (`can1357/oh-my-pi`) has three of four and no
uninstaller, no sandbox, and no integrity checking on the plugins it installs. The projects with the
most stars ship the most content and have the worst extension contracts; the project with the best
extension substrate ships zero user-facing skills. Nobody has shipped both. That gap — *curated content
on top of a correct lifecycle* — is the entire opportunity, and Muse Code hands you the two primitives
(`.muse/lock.json`, `.muse/skills.lock`) that every project in this list hand-rolled badly.

---

## 1. Honest state of the landscape

### 1.1 "oh-my-*" now means five different things

The shell-era semantics ("a config framework you install into someone else's tool") no longer hold.
Sorting by what the artifact actually *is*:

| Kind | Meaning | Examples (verified) |
|---|---|---|
| **(a) Framework** | Installs/manages/updates declarative assets in a host's config tree | `oh-my-claudecode` 38.9k, `oh-my-codex` 32.9k, `oh-my-opencode-slim` 8.6k, `oh-my-agent` 1.3k, `rlaope/oh-my-hermes` 1.3k |
| **(b) The agent itself** | A full harness or a fork of one, wearing the name as branding | `can1357/oh-my-pi` 28.9k (fork of Pi), `code-yeongyu/oh-my-openagent` 68.6k (vendors its own engine in the native edition) |
| **(c) Catalog / marketplace** | A pile of markdown someone copies | `Salomondiei08/oh-my-hermes` 857, `witt3rd/oh-my-hermes` 302, most of the long tail |
| **(d) Single skill pack** | One capability, one name | `oh-my-mermaid` 2.2k, `oh-my-design` 474 |
| **(e) Name collision** | Unrelated | `oh-my-posh` 23k, `oh-my-fish` 11k, `oh-my-rime` 4.9k, ~40 more |

**Positioning consequence:** the name signals nothing about the artifact. Two of the four projects the
brief named — `oh-my-pi` and `oh-my-opencode`/`oh-my-openagent` — are **agents, not config layers**.
`oh-my-pi` is explicitly "Fork of Pi by @mariozechner" (472k LOC TS + 222k LOC Rust). The literal
"oh-my-zsh for pi" config framework is a separate 149-star project, `ifiokjr/monopi`, which ships three
SKILL.md files. So the star leaders in this space are not evidence that the framework model works;
they are evidence that the *name* attracts stars.

### 1.2 Framework vs catalog — the ruthless classification

| Project | Real classification | Why |
|---|---|---|
| Yeachan-Heo/oh-my-claudecode | **FRAMEWORK** (install half) + product | Content-addressed ownership at install, no ownership at uninstall |
| Yeachan-Heo/oh-my-codex | **FRAMEWORK** buried in a 198k-LOC product | ~8k of 198k lines is the framework part |
| code-yeongyu/oh-my-openagent | **AGENT** with the best extension contract | 30 workspace packages; native edition vendors its own engine |
| can1357/oh-my-pi | **AGENT** with the best extension substrate | Not a config layer at all; ships 0 user-facing skills |
| alvinunreal/oh-my-opencode-slim | **FRAMEWORK** (in-process plugin) | 53k LOC TS running inside the host; "slim" is positioning, not a property |
| first-fluke/oh-my-agent | **FRAMEWORK** (multi-host compiler) | Declarative vendor adapters over a neutral SSOT |
| rlaope/oh-my-hermes | **FRAMEWORK** (best packaging) wrapped in a product | 289k LOC Python to ship 114 markdown files |
| haoruilee/oh-my-mcode | **PRODUCT with a plugin wrapper** | 654 lines of assets : 8,130 lines of TypeScript |
| witt3rd/oh-my-hermes | **CATALOG + prose + a thin state layer** | 3,629 lines of markdown; no installer, no updater |
| Salomondiei08/oh-my-hermes | **CATALOG** | 132 lines of flat `cp`; documented extension path is "edit your clone" |

**Roughly two of ten are what the label promises.** Everything else is either a product that absorbed
its host or a catalog with an install script.

### 1.3 Stars are not adoption — and two projects rig their own metric

Weekly npm downloads per star, computed from live figures:

| Project | Stars | Weekly DL | **DL / star** | Read |
|---|---:|---:|---:|---|
| can1357/oh-my-pi | 28,857 | 113,949 | **3.95** | An agent people run daily |
| first-fluke/oh-my-agent | 1,257 | 2,033 | **1.62** | Small, genuinely used |
| alvinunreal/oh-my-opencode-slim | 8,560 | ~13,700 | **1.60** | Converts 10× better than the star leaders |
| code-yeongyu/oh-my-openagent | 68,585 | ~61,000 | **0.89** | Star magnet, real but thinner usage |
| Yeachan-Heo/oh-my-claudecode | 38,933 | 5,759 | **0.15** | 26× the stars of slim, 0.4× the downloads |
| Yeachan-Heo/oh-my-codex | 32,946 | 4,021 | **0.12** | 83 watchers on 32.9k stars |

Genuinely adopted dev tools run 1–10. **The two biggest "frameworks" in the space run at 0.12–0.15.**
Both also have 0–1 open issues against 1,267–1,414 lifetime issues at ~1 release/day from one primary
author — aggressive closing at machine cadence, not absence of problems.

Two projects solicit stars from the installer: `oh-my-openagent`
(`gh api --method PUT /user/starred/...` for **two** repos) and `oh-my-opencode-slim`
(`gh api --method PUT /user/starred/alvinunreal/oh-my-opencode-slim`, default-no `[y/N]`). Both are
opt-in and TTY-gated; both contaminate the primary traction signal of their own project.

**Do not treat any design choice in this space as validated by usage.** Treat it as validated by
reading the code.

### 1.4 The scarce good is lifecycle, not content

Catalogs saturate instantly: `anthropics/skills` 173k★, `awesome-claude-code` 53k★,
`agentic-awesome-skills` 46k★, `wshobson/agents` 39k★, `claude-plugins-community` 2,282 plugins.
Content is free. What nobody ships is **install → pin → update-without-clobbering → verify →
uninstall**. The market has stated this out loud three separate times:

- `vyvhouse/oh-my-destructor` (29★) — a third-party product whose *only purpose* is safely
  uninstalling `oh-my-claudecode`.
- `oh-my-dsh/dsh-plugin-upgrade-skill` (33★ / 19 forks) — a skill whose only job is migrating plugins
  across host versions.
- `AnPod/Switch-Omo-Config` (24★) and `Poorgramer-Zack/omo-switch` (26★) — profile switching invented
  twice, independently, for the same host.

Three independent skill install/enable/disable managers (`nextcaicai/oh-my-skills` 26★ Rust,
`AIjunja/oh-my-skills` 20★, `duzhenxun/oh-my-skills` 20★) exist for the same reason. Skill lifecycle is
an unsolved, repeatedly-attempted problem.

### 1.5 The nobody-has-all-four table

| | Working uninstall | Lockfile with real consumers | Override / overlay | 3rd-party registry |
|---|---|---|---|---|
| oh-my-claudecode | ✗ broken (143/154 files survive, prints success) | ✗ `capabilities lock` has **zero** consumers | ✗ closed const roster | ✗ |
| oh-my-codex | ~ clean but **name-matched**, deletes edits setup protected | ✓ `installed-skills.json` | ✗ | ✗ (host has one; uses only the Local arm) |
| oh-my-openagent | ~ Codex only; **none** for OpenCode | ✗ hash-pinned hook trust only | ✓ best in space | ✗ |
| oh-my-pi | ✗ none at all (leaves credentials + model weights) | ✓ plugins only, not content | ✓ best substrate | ~ **protocol** yes, index no |
| opencode-slim | ✗ no command; manual doc already drifted | ✓ ancestor-hash manifest | ~ agents/prompts only | ✗ |
| oh-my-agent | ~ best preview, materially incomplete | ✗ 418 hashes, verifier has **zero** callers | ✗ | ✗ |
| rlaope/oh-my-hermes | ✓ **best in sweep** | ✓ `manifest.json` refuse-on-drift | ✗ | ✗ (is a tap in the host's registry) |
| oh-my-mcode | ~ manual; never undoes the host install it performed | ✗ | ✗ | ✗ (honestly says so) |
| witt3rd/oh-my-hermes | ✓ trivially (two `rm -rf`) | ✗ | ✗ | ~ publishes into host registry — **broken path** |
| Salomondiei08/oh-my-hermes | ~ hardcoded manifest; leaves live crons + API keys | ✗ | ✗ | ✗ |

### 1.6 Where the ecosystem's centre of gravity actually is

- **`.claude-plugin/marketplace.json` is a de-facto standard.** 32,192 matching files in GitHub code
  search; `anthropics/claude-plugins-official` 291 plugins, `claude-plugins-community` 2,282.
  `can1357/oh-my-pi` reads it directly — its own catalog `$schema` is literally
  `https://anthropic.com/claude-code/marketplace.schema.json`. `charmbracelet/crush` ingests
  `~/.claude/skills/`. Ingesting Claude's format is now a normal competitive move.
- **The Chinese DSH ecosystem is larger and invisible to English search.** `topic:dsh-plugin` = 13,105
  repos vs `topic:claude-code-plugin` 5,847. **`topic:muse-plugin` = 0.**
- **The DSH land-grab is the precedent that matters for timing.** DeepSeek Harness shipped mid-August
  2026; within ~10 days **11 distinct `oh-my-dsh` repos** existed from unrelated authors. The two that
  took the **auto-synced-catalog** position (78★, 15★) beat the ones that shipped orchestration
  features. The name is contested within days, and the winner is whoever becomes the index the others
  point at.
- **Muse Code is greenfield and the window is open now.** The entire third-party Muse ecosystem is six
  repos, max 2 stars, and three of six are model/provider shims. `sawasawasawa/oh-my-muse` is a dead
  empty squat. `oh-my-musecode` and npm `omm` appear unclaimed.

---

## 2. The comparison matrix

### Table 1 — the ten torn-down projects

| Project | Host agent | Install | Ships | Extension contract | Update | Registry | Traction (2026-09-01) |
|---|---|---|---|---|---|---|---|
| **Yeachan-Heo/oh-my-claudecode** (OMC, npm `oh-my-claude-sisyphus`) | Claude Code; also syncs MCP into `~/.codex/config.toml` | Claude Code plugin marketplace (repo root **is** the marketplace) **or** `npm i -g` + explicit `omc setup`. No postinstall side effect | 19 agents · 35 skills · 21 command dispatch shims (18-line) · 25 hook registrations / 11 events · 7 rule packs · 1 MCP server (~56 tools) · 1 HUD | **ADDITIVE-ONLY.** Learned skills (flat `.md` + `triggers` frontmatter, 4 roots) · rules auto-discovery incl. `.cursor/rules` + `.github/instructions` · user region in CLAUDE.md · jsonc config. **No overlay, no disable list, closed `KNOWN_AGENT_NAMES` const tuple** — a new agent requires a fork | `omc update` → npm `@latest` → re-run install. **202-record sha256+gitBlob historical ownership table**; `.omc-managed` sentinels; TOCTOU re-lstat before unlink; marker regions with embedded version; downgrade guard. `capabilities lock` exists with **zero consumers** | **NONE.** Single-entry self-marketplace | 38,933★ / 3,492f / **1 open issue** / 248 releases / 135 contributors / npm 5,759 wk (**0.15 DL/star**) |
| **Yeachan-Heo/oh-my-codex** (OMX) | Codex CLI | **TWO-PHASE.** postinstall is near-inert (`noop-local` unless global, hydrates a Rust binary, prints an advisory) — they shipped auto-setup and **reverted** it. Then explicit `omx setup --scope user\|project`, 8 numbered steps | 29 skills · 32 prompts · 28 agent TOMLs · 7 hook events · 6 MCP servers **all `enabled:false`** · catalog manifest with a lifecycle · 6 Rust crates | Hook plugins (`.omx/hooks/*.mjs`, `onHookEvent`, subprocess + clamped timeout) · user skills by namespace · reserved `USER:OMX:POLICY` region. **Curated catalog is closed** — no enabled-modules list, no overlay. Kill switch `OMX_HOOK_PLUGINS=0` is **inert on the dispatch path** | **sha256 receipt keyed to the installer's own output**, not a destination scan; `Object.create(null)`; non-regular-entry sentinel; every write backed up; **deferred update after session exit**; package-manager ownership detection; capabilities lock verified in CI | **NONE.** 2,723 lines of marketplace code hard-wired to one local plugin — while the host ships `Local\|Git\|Npm` sources | 32,946★ / 2,534f / **0 open** / 1,267 closed / 134 releases / 88 contributors / npm 4,021 wk (**0.12 DL/star**) |
| **code-yeongyu/oh-my-openagent** (omo / lazycodex) | OpenCode (primary) + Codex + own "senpi" native edition; ingests Claude Code plugins/skills/agents, Cursor rules, Copilot instructions | Three installers. `bunx oh-my-openagent install` writes **one line** into `opencode.json` `plugin[]` + `~/.omo/omo.jsonc`. `npx lazycodex-ai install` for Codex. `npm i -g omo-ai@beta` native | 30 workspace entries (46 pkg dirs) · 10 agents defined **in TypeScript** · 17 shared skills · 56 individually-disableable hooks · 14 tools · 5 MCP servers · 12 platform binaries | **BEST IN SPACE.** 7-root skill ladder with an explicit integer `SCOPE_PRIORITY` table + first-wins dedup · declarative per-agent override incl. `disable:true` · 56 named hook toggles · rules from 9 roots with `SOURCE_PRIORITY` · reads `~/.claude/plugins/installed_plugins.json` | Version pinned in host config; channel-aware; `revertPinnedVersion` rollback; journaled + PID/lease-locked migration engine with resume. **Two config writers**: installer uses a JSONC AST, updater uses regex + a bracket counter with no string-literal awareness | **NONE.** Single-vendor `marketplace.json`, one plugin. Ecosystem formed anyway (slim 8,560★, config-manager 396★, dashboard 290★) | 68,585★ / 5,630f / **916 open issues** / 312 contributors / 255 releases / ~61k wk npm (**0.89**). **Non-OSI Sustainable Use License.** Bus factor 1 (11,497 vs 396 commits) |
| **can1357/oh-my-pi** (omp) | **Its own agent** (fork of Pi). Ingests 9 foreign config roots | `curl \| sh` writes **exactly one file** — the binary. Also brew / bun / nix / mise / Docker. No dotfiles, no shell-rc edits | 29+3 tools · 8 subagents · 27 built-in rules · 176 prompts · 98 themes · 77 top-level slash commands · 481 settings keys · **0 user-facing skills** | **BEST SUBSTRATE.** 14 capabilities × N providers, integer priority + first-wins dedup, **mandatory `_source` provenance or the item is dropped**, `_shadowed:true` retained for diagnostics, **capability-qualified disable ids** (`skill:pdf`, `context-file:user:CLAUDE.md`), `suppress()` vs `filter()`. Implements **Agent Plugins 1.0.0** with a closed schema | 6-way install-method detection (brew/mise/nix/bun/npm/binary); updates *through the real owner*; **`omp.rename` forward pointer + `dist:"binary"` escape hatch for updaters already in the field**; SHA-256 + size verification of every asset; `plugin doctor --fix` | **REAL PROTOCOL, no index.** `.omp-plugin/marketplace.json` with `.claude-plugin` **fallback** and Anthropic's `$schema`. git / git-subdir / url+sha sources; npm rejected. **No signature, no checksum, no trust prompt, no sandbox** | 28,857★ / 2,884f / 1,261 open issues + 783 open PRs / **583 releases**, 830 tags / ~592 contributors / npm 113,949 wk (**3.95 DL/star**) / 197 repos on its topic |
| **alvinunreal/oh-my-opencode-slim** | OpenCode (in-process TS plugin, 53k LOC) | `bunx oh-my-opencode-slim@latest install`, 10 guarded steps, `--dry-run` everywhere. **Atomic write + automatic `.bak` on every host-config mutation.** Plugin entry carries `{__ohMyOpencodeSlimManagedByInstaller:true}` | 9 agents · 8 skills installed · 4 slash commands **registered in memory, never written to disk** · 14 hook factories · 2 MCP servers · Rust companion binary | Layered config (built-in → user → project → preset → `agents.*`) · **custom agents as unknown config keys** with auto-injected routing · **`<agent>.md` REPLACE + `<agent>_append.md` APPEND** over a 4-level search path. Everything else is **subtract-only** | **THE ONLY REAL THREE-WAY MERGE.** `lastManagedHash` = ancestor, `sourceHash` = theirs, on-disk = yours → no-op / overwrite / **adopt** / **stage-as-customized**. mkdir lock w/ staleness, fail-closed manifest validation, path-confined delete, orphan recovery | **NONE.** A GitHub issue form feeding a hand-curated marketing page | 8,560★ / 510f / 62 releases / 117 contributors / npm 59,530 mo (**~1.60 DL/star** — best of the frameworks) |
| **first-fluke/oh-my-agent** (oma) | **14 runtimes** from a neutral `.agents/` SSOT + declarative vendor adapter JSON | `curl \| bash` bootstrapper that runs **three more `curl\|sh` provisioners** (bun, uv, serena), then an interactive installer that downloads an **unverified `main`-branch tarball** | 33 skills · 12 agents · 21 workflows · 13 rules · 23 hook handlers · 9 vendor variant manifests · 24 migrations · Homebrew formula + GitHub Action | **NONE for content.** One user-owned file (`oma-config.yaml`, preserved byte-for-byte, new keys appended). Everything else replaced wholesale — including the `.agents/eval/` dir the docs *promise* survives but the code `rm -rf`s | `cpSync(force:true)` over the whole `.agents` tree — **destroys every edit, silently, no diff, no dry-run, backups deleted at the end**. `prompt-manifest.json` ships **418 real `{path,sha256,size}` entries** and its verifier has **zero production callers**. Ownership manifest **designed** (336-line doc) and **unimplemented** | **NONE.** Six distribution channels, one publisher — fan-out, not federation | 1,257★ / 146f / **1 open issue** / 601 tags / 11 contributors / npm 2,033 wk (**1.62**) |
| **rlaope/oh-my-hermes** (OMH) | Hermes Agent + MCP bridge auto-config for claude-code / codex / opencode / cursor | Five channels onto **one wheel**. **`OMH_RUN_SETUP` defaults to 0** — `curl\|sh` writes a venv + a symlink and nothing else; a second, user-typed `omh setup` does all config mutation. npm channel vendors the exact wheel, SHA-256-verifies it, refuses symlinks, downloads nothing | 114 skills · 9 roles · 4 team packs · 17 subagent files · **168 CLI subcommands** · 20 doctor checks · 216-item vendored ecosystem index · 4 skins · 1 TUI widget | **EFFECTIVELY NONE.** Adding one skill = 6+ Python modules + exact-count fixtures in 4 test files. What exists: **registration by pointer** (one `skills.external_dirs` line), foreign-rules import (`.cursorrules`/`.clinerules`/`.windsurfrules`/copilot) into a **separately-manifested** namespace, routing overrides, capability policy (subtract only) | `manifest.json` per-file sha256 + `local_modifications()` **raises before writing anything**; **`--force` is VETOED by `OMH_SECURITY=strict`**; four-condition prune with retain-and-report; **install-provenance-driven self-update** (PEP 610 `direct_url.json`, brew Cellar, launcher env stamp); hook integrity ledger with **revocation-as-data** | **NONE of its own — it *is* a tap** in the host's Skills Hub. Discovery = a vendored, commit-pinned, read-only index with an explicit claim boundary | 1,299★ / 123f / 6 open / 11 releases / 289k LOC Python · 192k LOC tests / **~53% of commits agent-authored** |
| **haoruilee/oh-my-mcode** | MiniMax Code (`mcode`) — a third-party binary it **explicitly disclaims owning** | `npx github:haoruilee/oh-my-mcode install --yes` (the README's npm hero command **404s**). Optionally bootstraps the host with consent; **refuses under `CI=true`/`OMM_HERMETIC=1`**; `--skip-host` | 10 skills (556 lines) · 5 role contracts (98 lines) · 8 workflow YAMLs · 6 JSON schemas · 1 MCP server (7 tools) · 15 CLI commands · **8,130 LOC TS** | **NONE.** 4 config knobs with a genuine privilege ceiling (a repo-local config cannot raise `permission` to `full`) + env overrides + a hand-copied AGENTS.md template | **NO UPDATE COMMAND.** Install is `rm -rf` + full-tree copy — 250 files / 6.0 MB including `test/`, `.github/`, `tsconfig.json`. No manifest, no checksum, no backup, no warning. Per-asset `version:` frontmatter that nothing reads | **NONE**, and unusually honest — `docs/marketplace.md` is titled "Marketplace (not listed)" | 51★ / 0f / 0 issues / **38 commits, 10 days old** / 0 releases / **not on npm** |
| **witt3rd/oh-my-hermes** | Hermes Agent. Explicit port of `oh-my-claudecode` | **NO INSTALLER.** Delegated to the host's tap system — **which is broken**: `tap add` defaults to `skills/`, the repo has none (skills live at `plugins/omh/skills/`), so the tap indexes zero skills. Real path is `cp -r`/`ln -s`, whereupon plugin `register()` triggers an **unguarded** copytree side effect | 10 skills (3,629 md lines) · 15 role prompts · 3 hooks · 2 tools · 13 docs incl. 6 host teardowns | Three seams: `config.yaml` as SSOT with **no hardcoded fallback**; **directory-as-registry** for roles (`glob("role-*.md")`, name regex doubles as traversal defense); **never-overwrite**. **No shadowing, no overlay** — extension means editing files inside the install tree | **NONE.** `if dest.exists(): continue` freezes skills at first install **forever**. The host already implements content-hash safe update (`.bundled_manifest` origin-hash + user-modified detection + `skills reset`); this uses only the skip half | Publishes **into** the host's registry (correct call, broken execution) | 302★ / 29f / 8 open / **76 commits** / **0 releases, 0 tags, no CI** / bus factor 1 |
| **Salomondiei08/oh-my-hermes** | Hermes Agent | `git clone` + `install.sh` — 132 lines of flat `cp`, no manifest, no backup, no checksum, no dry-run | 36 skills · 7 agents · 6 workflows · 13 shell scripts · **0 hooks, 0 MCP, 0 plugin manifests, 0 slash commands** | **NONE.** The documented path (`create-skill.md:63-65`) literally says to write the new skill into *your clone of someone else's repo* | `git pull && install.sh`. Unconditional `cp`. **Empirically verified: silently destroys user edits, no prompt/diff/backup.** Per-asset `version:` frontmatter required by a linter and read by nothing | **NONE.** A hand-maintained markdown table | 857★ / 81f / **1 issue ever** / 47 commits on main / 3 contributors / **no LICENSE** (badge is false) / **main HEAD 2 months stale** |

**Headline defect, Salomondiei08:** `scripts/setup-cto.sh` contains a **committed unresolved git merge
conflict marker on `main` at HEAD**; `bash -n` fails. That is step 4 of the advertised install flow.
A community fix PR has been open and ignored for five weeks. On macOS a *different* unbound-variable
crash fires first, aborting after `gh auth login`, memory writes and several crons have already run.

### Table 2 — adjacent precedents (discovery-verified, not torn down at source depth)

| Project | Stars | What it proves |
|---|---:|---|
| **microsoft/apm** | 3,683 | The only real lockfile in the space: `apm.lock.yaml` with a **`deployed_files` array and a `deployed_file_hashes` map** → exact uninstall + tamper detection. Verb set worth stealing wholesale: `install uninstall update outdated lock prune audit doctor policy approve publish pack cache marketplace registry targets runtime self_update`. Ships `CONFORMANCE.json` against a written spec with four conformance classes and an explicit "Honesty contract" admitting where drift detection does not exist |
| **wshobson/agents** | 39,326 | Multi-harness adapter architecture: SSOT under `plugins/<name>/`, per-harness artifacts **generated and gitignored**, adapters own all host mechanics, mechanical lint findings ship a remediation string, progressive disclosure with an 8 KB skill-body cap and `references/` overflow |
| **baekenough/oh-my-customcode** | 34 | `.omcustom.lock.json` with per-file `templateHash` + `size` + `component`, and the verb set `init / update / list / doctor / doctor --fix`. Tiny, and the most directly transferable lifecycle model found |
| **withastro/rosie** | 156 | "npm, but for skills" as a **Rust binary** — lockfile `.agents/rosie.lock`, `src/{lockfile,resolve,audit,sanitize,link,agent}.rs`, and a ready-made multi-channel packaging layout (`npm/ debian/ aur/ freebsd-package/`). ~60 agents supported. **No `muse`** |
| **vercel-labs/skills** | 30,166 | 9.36M weekly downloads across **77 agents**; symlink-by-default with `--copy`. **No `muse` entry** — an adapter is one table row |
| **anthropics/claude-plugins-official / -community** | 35,785 / 3,127 | The manifest shape to conform to: polymorphic `source` (relative path \| `{source:"url", url, sha}` \| `strict:true`) plus a **`renames{old:new}` map** for stable renaming. **SHA-pinned sources** in the official manifest |
| **pi-claude-marketplace** | 21 | A non-Claude agent consuming Claude marketplaces, with a **desired-state config** (`claude-plugins.json` + a `.local` personal/team split) and honest partial-install UX for unsupported components |

---

## 3. Design patterns that recur across the winners

Each pattern is named with the project that proves it and *why* it works. These are the parts worth
copying; §5 sorts them into what survives the move to a compiled Rust host.

### 3.1 Lifecycle & ownership

**P1 — Two-phase install: package installation ≠ configuration.**
`oh-my-codex` shipped auto-setup-on-install and **deliberately reverted it** ("the global npm install
now prints an explicit reminder instead of launching setup automatically"). `rlaope/oh-my-hermes`
defaults `OMH_RUN_SETUP=0` so `curl|sh` writes only a venv and a symlink, and even prints a warning
when setup-flavoured env vars are passed without opting in. `oh-my-mcode` refuses host installation
under `CI=true`. *Why:* the moment a package manager can mutate a user's agent config, every CI runner,
every Docker layer and every transitive install becomes an unattended config write.

**P2 — Content-addressed ownership recorded at write time.**
`oh-my-codex`'s `installed-skills.json` (per-file sha256), `rlaope`'s `manifest.json`,
`oh-my-claudecode`'s 202-record `{filename, byteLength, sha256, gitBlob, firstReleaseTag,
lastReleaseTag}` table, `microsoft/apm`'s `deployed_file_hashes`, `oh-my-customcode`'s
`templateHash`. Deletion or overwrite requires an exact byte match against what the installer wrote.
*Why:* it turns "is this file mine?" from a heuristic into arithmetic. OMC even re-`lstat`s
dev/ino/size/mtime between hash and `unlink` to defeat TOCTOU.

**P3 — The receipt records the INSTALLER'S OUTPUT, never a scan of the destination.**
`oh-my-codex`'s own source comment is the best documentation in this space: digesting the destination
tree "would poison the receipt" by recording a user's dropped note as framework-owned and later
deleting it. Corollaries it also gets right: `Object.create(null)` so a file literally named
`__proto__` cannot vanish from the comparison, and a `non-regular-entry` sentinel so a directory
containing a symlink or socket can never compare equal and is therefore **retained**.
*Why:* every failure mode here is fail-safe by construction — an unexpected file makes the comparison
fail, which means "keep", not "delete".

**P4 — Three-way merge with a recorded common ancestor.**
`oh-my-opencode-slim` is the only project in the space that does this. `lastManagedHash` = ancestor,
`sourceHash` = theirs, on-disk hash = yours. Four outcomes: **no-op / overwrite / adopt / stage**.
`adopt` (user independently converged on the new version) and `stage` (user diverged) are the two cases
everyone else gets wrong. *Why:* "I edited a file and now I can never update again" is the actual
failure mode of every oh-my-* framework, and it is a solved problem from version control.
**Note the warning:** slim's `LEGACY_MANAGED_SKILL_HASHES` table — the retrofit path for users who
installed before ancestors were recorded — is documented with a full recipe and is **still empty**.
Retrofitting an ancestor hash after users have edited files is the hard case. Build it on day one.

**P5 — Stage the update next door; never overwrite, never silently skip forever.**
slim writes conflicting updates to `.oh-my-opencode-slim/skill-updates/<version>/<skill>/` and toasts
exactly which skills need review. Contrast the two failure modes it avoids: **clobber**
(`oh-my-agent`'s `cpSync(force:true)`, `Salomondiei08`'s unconditional `cp`, `oh-my-mcode`'s
`rm -rf` + recopy) and **freeze** (`witt3rd`'s `if dest.exists(): continue`, which means no user has
ever received a skill update).

**P6 — Uninstall reads the same ledger install wrote.**
`rlaope/oh-my-hermes` is the reference: remove only what a manifest proves it wrote, with `--dry-run`,
`--registration-only`, `--purge`, `--keep-config`, manifest-checked refusals for the plugin dir, and a
**per-package-manager residue table** for what it deliberately leaves. `oh-my-openagent` adds
**allowlisted deletion**: five named managed roots, containment via `relative()+isAbsolute()` (not
string prefixes), and a `codexHomeResolvesToFilesystemRoot()` guard — the `rm -rf /` check, written
down. `oh-my-agent` adds the **mandatory two-section preview** (✗ remove / ✓ preserve) before any
`--yes`-gated confirm.
*Counterexample that defines the pattern:* `oh-my-codex` is **receipt-matched on setup and
name-matched on uninstall**, so it deletes by name exactly the user edits its installer carefully
refused to overwrite. The conservative machinery exists; uninstall just doesn't call it.

**P7 — Record install-time decisions at install time.**
`oh-my-openagent` writes `.installed-agents.json` and `.installed-bin-dir.json` so uninstall reads what
install *did*, not what install *would do now*. *Why:* env vars, `$PATH` and defaults change between
install and uninstall; manifests don't.

**P8 — `--force` is not sovereign.**
`rlaope`'s `--force` is routed through `resolve_security_posture`, and under `OMH_SECURITY=strict` it
**stops overriding local modifications entirely**. `oh-my-claudecode`'s `OMC_SECURITY=strict` is a
tighten-only lattice (booleans OR'd, numerics `Math.min`'d). *Why:* a confirmation flag that a security
posture can veto is the difference between a config framework and a foot-gun in a managed environment.

**P9 — Deferred updates; never swap assets under a running agent.**
`oh-my-codex` schedules the install to a detached worker that runs only after the current session
exits. `oh-my-pi` detects six install methods and **refuses to self-mutate a package-manager-owned
install**, updating through brew/nix/mise/bun/npm instead.

**P10 — Design the update protocol for updaters already in the field.**
`oh-my-pi`'s release manifest carries `omp.rename{package,natives}` — a forward pointer new updaters
follow to a renamed package — plus `omp.dist:"npm"|"binary"` where *any unknown value maps to
`"binary"`*, so older deployed updaters that don't understand `rename` fall through a safe escape
hatch. Every downloaded asset is verified by GitHub-reported **size and SHA-256 digest**, rejecting
duplicate-named assets, unfinished uploads and unexpected URLs.

### 3.2 Coexistence in files you don't own

**P11 — Registration by pointer.**
`rlaope` appends **one line** to `~/.hermes/config.yaml` (`skills.external_dirs: - ~/.omh/skills`) and
owns a whole namespace behind it. `oh-my-openagent` and slim each add **one entry** to the host's
`plugin[]` array and keep every setting in their own file. *Why:* minimal blast radius, auditable,
one-line uninstall, and it survives host config schema churn.

**P12 — Marker-delimited managed regions + a reserved USER region.**
`<!-- OMC:START -->` / `<!-- OMC:VERSION:5.1.0 -->` / `<!-- OMC:END -->`;
`# oh-my-codex (OMX) Configuration` … `# End oh-my-codex`; `<!-- oma:generated -->`;
`# BEGIN/END OMC MANAGED MCP REGISTRY`. The refinement: `oh-my-codex`'s
`<!-- USER:OMX:POLICY:START/END -->` — a region *inside a generated file* that the generator carries
across every regeneration. ~40 lines of code that removes the most common reason to fork.
*Caveat found in verification:* OMX does **not** apply this uniformly — the legacy non-merge refresh
path drops those blocks. Port the idea; apply it on every path.

**P13 — Intra-fence user-edit detection (the rarest good idea in the space).**
Inside its *own* managed block, `oh-my-codex` distinguishes "a value I wrote" from "a value you
edited": marker + known preset ⇒ rebuild; marker + non-preset value ⇒ **user edited, preserve**;
no marker + byte-match to a legacy default ⇒ managed; anything else inside the fence ⇒ preserve.
A three-way merge against a file it nominally owns. (Verification note: scoped to one key,
`[tui].status_line` — the mechanism is right, the coverage is narrower than advertised.)

**P14 — Format-preserving surgical editing, never parse-and-reserialize.**
`oh-my-openagent` uses `jsonc-parser` `modify`/`applyEdits` so comments and formatting survive.
`oh-my-codex` hand-rolls 4,300 lines of line-based TOML surgery for the same reason. Both treat the
user's config as **text with named regions**, not as an object to round-trip.

**P15 — The installer-managed marker key.**
slim writes `["oh-my-opencode-slim@2.2.18", {"__ohMyOpencodeSlimManagedByInstaller": true}]` into the
host's own array. One boolean distinguishes "my line, I may rewrite it" from "the user pinned this,
leave it alone", and it makes both auto-update and uninstall precise. (Caveat: it's conditional —
omitted when the version resolves to `latest`.)

**P16 — Atomic write on the resolved realpath, with a verified backup first.**
`oh-my-openagent` resolves symlinks to realpath before writing, writes a pid+timestamp temp, renames,
and retries `EPERM`/`EBUSY` on Windows. `oh-my-claudecode`'s CLAUDE.md transaction does `O_EXCL` backup
with readback comparison, symlink refusal, root canonicalisation with dev/ino pinning, temp+rename and
full rollback. slim takes a `.bak` on **every** host-config mutation.

**P17 — The tracked/ignored split for project state.**
`witt3rd` seeds `.omh/` with a README and a `.gitignore` encoding the split: `plans/ specs/ research/`
are **git-tracked decision artifacts** ("a consensus plan belongs in the repo for the same reason an
ADR does"); `state/ logs/ progress/` are gitignored per-session runtime. `oh-my-codex` does the same
at the host level — ignore `.omx/`, explicitly **un-ignore** `.codex/{agents,skills,prompts}/**` so
declarative assets stay commitable and reviewable.

### 3.3 Extension & precedence

**P18 — A published integer precedence ladder with first-wins dedup.**
`oh-my-openagent`'s `SCOPE_PRIORITY` is a seven-integer table in one file
(`builtin:1, shared:1, config:2, user:3, opencode:4, project:5, opencode-project:6`) — every "how do I
override this?" question has one greppable answer. `oh-my-pi` generalises it:
`native 100 > omp-plugins 90 > claude 80 > agent-plugins 75 > codex 70 > gemini 60 > opencode 55 >
cursor/windsurf 50 > cline 40 > github 30 > vscode 20 > managed-skills 5 > builtin-defaults 1`.
*Why:* precedence expressed as prose is a lottery; expressed as a table it is a contract.

**P19 — The capability × provider matrix (the single best structural idea found).**
`oh-my-pi` declares 14 capabilities via `defineCapability<T>{key, equivalent, validate, toExtensionId}`
and N providers `{id, priority, load(ctx)}`. Its own source states the thesis: "instead of callers
knowing about paths like `.claude`, `.codex`, `.gemini`, they simply ask for `load('mcps')`."
One mechanism yields discovery, dedup, precedence **and** a unified deny-list, for every capability,
forever. Adding a config *source* costs one file; adding a config *kind* costs one `defineCapability`.

**P20 — Mandatory provenance, and keep the shadowed items.**
Every loaded item must carry `_source{provider, providerName, path, level}` or it is **dropped with a
warning**; shadowed duplicates are retained in `result.all` with `_shadowed:true`. *Why:* "why is this
skill active and where did it come from?" becomes a query rather than an investigation. Most config
systems treat provenance as an afterthought and can never answer it.

**P21 — Stable, capability-qualified disable ids.**
`skill:pdf`, `extension-module:foo`, `context-file:user:CLAUDE.md`. One `disabledExtensions` list a
user learns once and that works on everything. Combined with P18 shadowing you get **both verbs** —
override and delete — with no forking and no template regeneration.
The subtlety `oh-my-pi` encodes in its type system: `suppress()` (excluded but still **claims** its
dedup key, so a disabled project MCP server keeps the same-named user server off) vs `filter()`
(dropped as if it never existed). Someone hit both bugs and wrote the fix into the types.

**P22 — The APPEND channel alongside the REPLACE channel.**
slim resolves `<agent>.md` (replace) and `<agent>_append.md` (append) **independently** across the same
4-level search path: `effectiveBase = inline ?? file ?? builtIn`, then `+ "\n\n" + append`.
*Why this is the highest-leverage single idea in the whole sweep:* the most common customisation is
"add one house rule". With only a replace channel, that user forks a 1,200-line prompt and never
receives another upstream improvement. With an append channel they **never enter a merge state at
all**. It removes the majority of P4's work by removing the majority of conflicts.

**P23 — Directory-as-registry.**
`witt3rd`'s role catalog is literally `glob("role-*.md")` — drop a file, it works; no manifest, no
index, no registration. The name regex `^[a-zA-Z0-9_-]+$` doubles as path-traversal defense.
`oh-my-codex`'s hook plugins are the same shape: drop a file in `.omx/hooks/`, that *is* the entire
registration protocol.

**P24 — Agent-authored content as a separate, lowest-priority provider.**
`oh-my-pi` gives `managed-skills` priority 5 with the source comment "so an authored skill of the same
name from ANY other provider wins". The cleanest answer found to letting the agent self-modify without
ever stomping a human.

**P25 — Ingest the competition's formats.**
`oh-my-pi` spends 8,907 LOC reading 9 foreign config roots. `oh-my-openagent` reads
`~/.claude/plugins/installed_plugins.json`, `.claude/skills`, `.cursor/rules`,
`.github/instructions`, `.github/copilot-instructions.md`. `rlaope` ships a guarded
`rules-import`. `oh-my-claudecode` auto-discovers `.cursor/rules/` and
`.github/instructions/*.instructions.md`. Even the hosts do it — `charmbracelet/crush` reads
`~/.claude/skills/`. *Why:* a new user's existing investment works on day one and your catalogue is
non-empty at launch, at a cost of ~150–500 lines per vendor.

**P26 — Catalog-as-data with a real lifecycle.**
`oh-my-codex`'s `src/catalog/manifest.json`: per-item `status` ∈ `active | merged | deprecated |
internal | alias` (39/14/6/3/1 across 63 entries), a `canonical` forwarding field, a `core` flag,
one-release **sunset stubs** naming the replacement, and a `REQUIRED_CORE_SKILLS` build gate that fails
if a load-bearing skill is deactivated. Deprecation as a first-class lifecycle state, CI-checked.

### 3.4 Host contract & honesty

**P27 — Capability negotiation against the actual binary.**
`oh-my-codex` parses `codex features list` to choose `[features].hooks` vs legacy
`[features].codex_hooks` and version-gates plugin-scoped hooks. `oh-my-mcode` parses
`mcode --version` into a `HostCapabilities` struct — and an **unparsed version yields ALL-FALSE
capabilities**, not optimistic defaults. *Why:* schema assumption against a moving third-party binary
is how you ship a config file that bricks a user's agent.

**P28 — A dated observed-behavior ledger of the host.**
`oh-my-mcode`'s `docs/host-reality.md` is the best single artifact found in this sweep. Its shape:
**fact → date/version observed → the code constant that encodes it → the test that locks it.** Sample
entries: the host's `--timeout` regex means a bare `180` is 180 *milliseconds* (caused a real exit-6);
`--session` and `--continue` are mutually exclusive so XOR is enforced in argv; `--output-schema`
returned exit 70 on 0.2.1, so the flag is omitted by default and validation moved into TypeScript; a
full documented exit table `0/1/2/3/4/5/6/7/70/130`; a Node 24 + better-sqlite3 GC abort matched by a
regex with exactly one crash retry.

**P29 — Orthogonal process outcomes.**
`oh-my-mcode` refuses to conflate `exitCode`, `timedOut` and `signal`. After its own SIGTERM, Node
reports `code=null, signal=SIGTERM`; the naive `code ?? 1` **lies** that a timeout was a crash. A child
that traps the signal and exits 0 is still `timedOut`.

**P30 — Post-install smoke test; never claim success for something that can't run.**
`oh-my-pi`'s installer runs `omp --version` after download and fails loudly with a musl-specific
remedy (`apk add libstdc++ libgcc`) — source comment: *"Never claim success for a binary that cannot
run."* It also reads `sysctl hw.optional.arm64` instead of `uname -m` so a Rosetta shell cannot force
a slow x86_64 install. `oh-my-codex` and `rlaope` both run a **register smoke test**: import the
freshly written plugin and assert every declared tool and hook actually registers. "Files copied" is
not "it works". `Salomondiei08`, for all its faults, refuses to print success after installing zero
items.

**P31 — Refuse rather than guess; fail closed; quarantine rather than repair.**
`rlaope`'s YAML mutator **raises** on shapes it cannot parse (while readers stay non-throwing so
`doctor` never crashes). slim's manifest validation is fully structural and **skips rather than
overwrites** on corruption. `oh-my-pi` moves invalid config under a file lock to
`.broken-<ts>-<pid>-<uuid>` and fails startup **naming both paths**.

**P32 — `doctor` as a first-class product.**
`rlaope` ships 20 checks including `identity_conflicts`, `plugin_hook_integrity`, `guidance_projection`
and `security_posture`. `oh-my-mcode` ships a triad — `doctor` (package) → `doctor --smoke` (one real
host exec) → `doctor --tps` — and **prints the literal string `unmeasured` and exits non-zero** rather
than fabricating a throughput number. It also emits a permanent note-level check admitting what it
cannot prove: *"If plugin list shows installed+enabled, files exist; triggering is not proven."*

**P33 — The bidirectional manifest↔disk check.**
`oh-my-mcode` cross-checks manifest `skills[]` against the `skills/` directory in **both** directions:
listed-but-missing is an error **and** present-but-unlisted is an error. Plus per-skill frontmatter
validation (`name == dirname`, and the description must contain a "do not" clause). *Why:* silent skill
drop is the classic failure mode of markdown-asset frameworks, and this is a ~30-line guard.

**P34 — Ship a skill that configures the framework, and a skill that mines sessions for what to add.**
`oh-my-claudecode` ships `/skillify` (a skill whose only job is authoring user skills). slim ships
`oh-my-opencode-slim/SKILL.md` (teaches the agent the config schema) **and** `reflect/SKILL.md`, which
reads the host's session SQLite DB to mine repeated friction and recommend "a skill, custom agent,
command, configuration change, prompt rule, playbook, or **no change**". `oh-my-mcode` ships
`INSTALL_FOR_AGENTS.md` — an install doc written for an LLM to execute. *Why:* every user of this
product has a coding agent in the terminal. **The installer's user is another agent.** An
agent-executable config manual beats a plugin API for the 90% case.

### 3.5 Performance & context economy

**P35 — One dispatcher process per event, not N.**
`oh-my-agent` collapses an entire handler chain into **one** settings entry
(`oma-hook.sh --vendor X --event Y`) with `timeout = sum(handler timeouts) + 5s`; handler files are not
even copied to the vendor dir, they run in-process. And they **measured it**: a committed benchmark
documents p50 ~624 ms / p95 ~821 ms against a 1500 ms SLO and exits non-zero on regression.
Its wrapper script is also machine-independent by construction — resolve `$OMA_BIN` via `command -v`
then a literal list of well-known install dirs, pass `"$@"` verbatim, swallow non-zero with `|| true`,
and **exit 0 fail-open** so a stale CLI can never wedge the agent.

**P36 — Compact shims + lazy bodies.**
`oh-my-claudecode` does this twice: 21 slash commands are ~600-byte dispatch stubs pointing at
`skills/<n>/SKILL.md` (≈13 KB of always-loaded description instead of ≈431 KB), and
`compactPluginSkillPayload()` rewrites each installed `SKILL.md` into a ≤240-char frontmatter shim with
an `omc-full-body:` pointer, archiving real bodies to `skill-bodies/`. `wshobson/agents` enforces the
same discipline with an 8 KB body cap and `references/` overflow.
*Counterexample:* `rlaope`'s `DEFAULT_SKILL_PROFILE = 'full'` loads all **114** skills, and the repo
then builds an entire warning apparatus (a body char limit, a manifest `context_cost_warning`, an
`omh docs skill-context-cost` command, a density gate) to manage a cost the default created.

**P37 — Embedded payload materialised on first run.**
`oh-my-openagent`'s native edition compiles the whole agent + plugin into one binary and materialises
the payload to disk on first run (`provisionEmbeddedRuntime`, `materializeProvisionedExecutable`,
`shouldReexecAfterProvisioning`). The single most relevant precedent for a compiled host: in Rust this
is `include_dir!`/`rust-embed` + extract-once + integrity-check thereafter.

### 3.6 Multi-host projection

**P38 — One canonical source, N generated host manifests, CI drift gate.**
`oh-my-codex` authors skills once at `skills/<n>/SKILL.md` and **generates** `plugins/oh-my-codex/`
via `npm run sync:plugin`, drift-checked by `verify:plugin-bundle` in `prepack`.
`oh-my-agent` commits its generated distribution artifacts (`.claude-plugin/marketplace.json`,
`skills/`, `mcp.json`) **and** re-emits them into a scratch dir on every push for a byte comparison,
with `version` normalised so a release bump alone doesn't false-positive — running in `.husky/pre-push`
as well as CI. `wshobson/agents` states the invariant: per-harness artifacts are generated and
gitignored; only small native-install registries that merely *point at source* are committed.

**P39 — Declarative host adapters.**
`oh-my-agent`'s `.agents/hooks/variants/<vendor>.json` + a 160-line JSON Schema captures `hookDir`,
`settingsFile`, `projectDirEnv`, `runtime`, `events{event → [{hook,matcher,timeout}]}`, `statusLine` +
`statusLineKey` (Qwen nests it under `ui`), `extra` settings, and `featureFlags{file,section,flags}`
for Codex TOML — plus **three named escape hatches** for hosts that don't fit
(`flatHookEntries`, `skipSettingsMerge`, `homeOnly`). *Why:* adding a host is mostly data, and every
deviation is a documented boolean rather than a scattered `if (vendor === …)`.

**P40 — Conform to an existing standard rather than inventing a format.**
`oh-my-pi` implements **Agent Plugins 1.0.0** (agent-plugins.org) with a **closed** `plugin.json` schema
— an unknown top-level field warns, a closed-schema violation **fatally rejects the whole plugin so no
component loads** — plus Agent Skills frontmatter validation (agentskills.io, closed 6-field schema,
NFKC name normalisation) and a `classifyAgentPluginRoot()` that prevents a conformant plugin from
*also* being loaded through legacy conventions. It also publishes its marketplace catalog at
`.omp-plugin/marketplace.json` with `.claude-plugin/marketplace.json` as a **documented fallback**, so
one repo serves both ecosystems from the identical schema.

---

## 4. Antipatterns — recurring mistakes `oh-my-musecode` must not repeat

Ordered by damage. Every one was verified at source level; several were verified by execution.

**A1 — Clobbering user edits on update.** `oh-my-agent` runs `cpSync(force:true)` over the whole
`.agents` tree and **deletes its backups at the end of a successful update**; no `--dry-run`, no diff,
no drift warning. `Salomondiei08` runs unconditional `cp` — *verified by execution*: appending a line
to an installed skill and re-running `install.sh` changed the file's md5 with no prompt, diff, backup
or `.bak`. `oh-my-mcode` `rm -rf`s the plugin dir and re-copies 250 files. `oh-my-claudecode`
force-overwrites all 35 bundled skills on **every plain `setup`**, silently. The docs sometimes even
admit it — oma's own config header says so — which is honest, and is not a fix.

**A2 — The opposite failure: freezing forever.** `witt3rd`'s `if dest.exists(): continue` means a user
who installed v0.1.0 is still running v0.1.0's skills at v0.9.0. **Skip is not reconcile.** Made worse
because the host it targets *already implements* content-hash safe update (`.bundled_manifest`
origin-hash + user-modified detection + `skills reset`) and the project uses only the skip half.

**A3 — Uninstall that ignores the manifest install wrote.** `oh-my-codex` teaches the user their edits
are safe (setup skips modified files), then deletes them by **name** on the way out — along with any
pre-existing user prompt whose filename collides — with **no backups on the uninstall path**.
`oh-my-claudecode` carries a 202-record cryptographic inventory of every byte it ever wrote and its
uninstaller **doesn't read it**; measured, 143 of 154 files and all hook registrations survive a
"complete uninstallation" that prints a green success checkmark. Its stale hardcoded list targets `.sh`
hooks removed two major versions earlier. `oh-my-agent`'s ownership manifest is a 336-line design doc
with `status: Draft`.

**A4 — A lockfile with no consumers.** `oh-my-claudecode`'s `omc capabilities lock` produces a real
27 KB lockfile with a `surfaceDigest` and **zero references** from the installer, the updater or any CI
workflow. `oh-my-agent`'s `prompt-manifest.json` ships **418 real `{path,sha256,size}` entries**,
is CI-synced, and the only function that verifies against it has **zero production callers** — the
manifest is used solely for a version string comparison. *Shipping the artifact is not shipping the
mechanism.*

**A5 — Announcing a safety property that doesn't hold on the hot path.** `oh-my-codex` documents
`OMX_HOOK_PLUGINS=0` as a kill switch; on the dispatch path the expression evaluates to
`true || isEnabled(env)` and the switch only changes what `omx hooks status` prints — it never stops a
plugin from being spawned. Its "source is regex-scanned before import" guarantee lives in a CLI lint
command; at dispatch the runtime `await import()`s **first** and checks for the export **after**, so a
malformed plugin's top-level side effects run. **A guard people rely on and that doesn't fire is worse
than no guard.**

**A6 — Fuzzy ownership heuristics.** `oh-my-claudecode` matches hooks for replacement with a regex on
the command string, `/(?:^|[/\\_-])omc(?:$|[/\\_-])/` — precisely the "filename, frontmatter, or fuzzy
ownership heuristic" that its own provenance-table header forbids. Any user hook whose path contains
`/omc/` becomes eligible for replacement. `oh-my-agent` runs five different in-band markers, none
covering JSON settings on the uninstall path, and its "is it a symlink?" ownership test is wrong on
Windows where its own `createLink` legitimately falls back to junction/hardlink/copy.

**A7 — Unverified mutable-branch transport.** `oh-my-agent` pulls a tarball of the **`main` branch** on
every install *and every update* — no tag pin, no checksum, no signature. Two users installing an hour
apart get different bytes under the same version number, and a bad merge is instantly live for
everyone. It ships 418 real per-file hashes and doesn't check them.

**A8 — Two standards for integrity in one repo.** slim's companion-binary updater does full SHA-256
verification against a pinned per-platform manifest; its `ast-grep` downloader, 30 lines away, fetches
a binary from GitHub releases, extracts it and `chmod 0755`s it with **zero checksum**. Same repo,
same threat model.

**A9 — No sandbox for third-party executable extensions.** `oh-my-pi`'s marketplace install is
`git clone` → symlink into `node_modules` → `import()` → **arbitrary code in-process with the agent's
credentials** (`~/.omp/agent/agent.db`), its network, and `tool_call` interception on every tool the
model runs. No signature, no checksum, no integrity pinning, no capability manifest, no trust prompt.
Its own docs say "Extensions are not sandboxed (same process/runtime)". The ecosystem's answer was
third-party jails — `omp-sbx` (Docker) and `agent-jail` (Landlock).
Related: `oh-my-pi` also **dropped upstream Pi's project-trust gate** (`trust.json`,
`defaultProjectTrust`), so `cd` into a hostile clone and `<repo>/.omp/extensions/*.ts` executes before
you type anything.

**A10 — Closed rosters in the type system.** `oh-my-claudecode`'s `KNOWN_AGENT_NAMES` is a `const`
tuple and `PluginConfig.agents` is 20 literal keys, so registering a new agent **requires a fork**. For
a project named after oh-my-zsh — whose entire value proposition was a plugin directory anyone could
drop into — this is the central design failure. (`agentOverrides` exists and is an open record, but
sets only a routing tier; it cannot register an agent, replace a prompt, or disable a skill.)

**A11 — Additive-only extension, with "upstream it" as the documented answer.** No overlay dir, no
enabled/disabled list, no precedence rule. `oh-my-claudecode`, `oh-my-codex`, `oh-my-agent`,
`rlaope/oh-my-hermes`, `oh-my-mcode`, `witt3rd`, `Salomondiei08` — seven of ten. `oh-my-agent`'s docs
answer "I want to change a shipped skill" with *upstream it*. `Salomondiei08`'s answer is *edit your
clone of my repo*. `rlaope`'s is *edit 6+ Python modules plus exact-count fixtures in 4 test files*.
This is what produces **5,630 forks against 312 contributors** (omo) and **2,884 forks** (omp): every
serious user becomes a forker, and the ecosystem fragments instead of composing.

**A12 — A "user-writable" slot the updater deletes.** `oh-my-agent`'s docs state that
`.agents/eval/<skill>/` "survives `oma update` without overwriting user-authored evals"; the code runs
`rmSync(.agents/eval, {recursive:true, force:true})` unconditionally. Verified. Net: oma has **zero**
durable user-writable slots beyond one config file.

**A13 — Two config writers with different fidelity.** `oh-my-openagent` writes the host config with a
JSONC AST in the installer and with **regex + a manual bracket counter with no string-literal
awareness** in the auto-updater. It will eventually corrupt a config where a bracket appears inside a
string. slim has the mirror-image bug: a working JSONC tokenizer on the *read* path and a
`JSON.stringify` on the *write* path that prints its own warning — *"comments will not be preserved"* —
and then destroys them anyway.

**A14 — Multiple install topologies.** `oh-my-claudecode` supports plugin / project-scoped plugin /
`--plugin-dir` dev mode / standalone npm, producing conditions like
`shouldInstallLegacyAgents && !pluginDirMode && (noPlugin || !enabledOmcPlugin || …)` and ~600 lines of
plugin-cache validate/repair/compact/sync logic that exists *only* because the npm package and the
plugin cache are two drifting copies of the same content. `oh-my-codex` runs a legacy-vs-plugin dual
mode where every setup step branches on `isPluginInstallMode`. **Pick exactly one and never add a
second.**

**A15 — Identity sprawl and losing your own namespace.** Four names for OMC (repo `oh-my-claudecode`,
npm `oh-my-claude-sisyphus`, plugin `oh-my-claudecode`, marketplace slug `omc`, CLI `omc`) — and the
obvious npm name **is squatted by an unrelated project** (`oh-my-claudecode` on npm is a
"Cthulhu-themed agentic harness", 19 versions). Five bin aliases for omo across two npm packages plus a
repo rename that kept the old npm name. `oh-my-codex`'s README needs an anti-confusion clause disowning
forks branding themselves "OMX v2". Three unrelated repos are named `oh-my-hermes` (1,299 / 857 / 302
stars, ~2,460 stars split three ways), and one of them had to ship a 493-line runtime
identity-conflict detector because of it. **Needing a disclaimer in your own README means the namespace
was already lost.**

**A16 — Stale self-description.** `oh-my-claudecode`'s shipped `marketplace.json` advertises "28 agent
variants" against 19 files on disk; a `sync-metadata` script exists to prevent exactly this and doesn't
cover the manifest. `oh-my-agent`'s README/docs counts are hand-written. slim's uninstall doc lists 7
skills where the code installs 8, and tells the user to remove a bare string from an array where the
installer writes a tuple. `witt3rd`'s `docs/plugin.md` says "Nine shared role prompts" above a ten-row
table while fifteen ship, and calls an 13-action tool an "8 actions" tool.

**A17 — Documentation that reads like configuration.** `oh-my-mcode`'s workflow YAML declares
`max_repairs`, `only_verify_sets_accepted`, `deterministic_verify_first` — all typed
`Record<string, unknown>` with **no consumer**; a user editing `max_repairs: 3` is silently trapped
(the real value lives in `DEFAULT_CONFIG`). `rlaope`'s `config.yaml` has a `roles:` block listing 10
roles while 15 ship, and **nothing reads it** (the catalog is a directory glob). `Salomondiei08` and
`witt3rd` both require per-asset `version:` frontmatter in a linter and never read it — the lockfile
that wasn't built.

**A18 — Scope drift: the framework absorbs the host.** `oh-my-codex` is 198k lines of TypeScript of
which roughly 8k is the framework; the rest is team mode, a HUD, MCP servers, ralph/ultragoal loops and
a Rust sparkshell. `rlaope` is 289k lines of Python to ship 114 markdown files (~2,500:1
machinery-to-content) and 168 CLI subcommands, papered over with "just say the trigger in chat" —
which admits the CLI is not the product surface. `oh-my-mcode` is 654 lines of assets to 8,130 lines of
TypeScript. `oh-my-openagent`'s native edition vendors its own engine; `oh-my-pi` forked the agent
outright. **The packaging/curation layer everyone actually wants is buried inside a product.**

**A19 — Injecting a safety posture as an install side effect.** `oh-my-codex`'s `templates/AGENTS.md`
opens with a shouted all-caps autonomy directive — *"YOU ARE AN AUTONOMOUS CODING AGENT. EXECUTE TASKS
TO COMPLETION WITHOUT ASKING FOR PERMISSION. DO NOT STOP TO ASK…"* — written into every workspace by
default. Rewriting the global safety posture of someone's agent is a choice for the user, not the
framework.

**A20 — Touching things that are not yours.** `oh-my-openagent`'s npm `postinstall` recursively
`rmSync`s entries under **another product's** cache (`$XDG_CACHE_HOME/opencode`) on every install,
unprompted. `oh-my-agent` ships `promptUninstallCompetitors` — an installer that offers to remove
competing tools. `oh-my-claudecode` writes into `~/.codex/config.toml` (a different vendor's config),
undocumented in its README.

**A21 — Contaminating your own traction metric.** Two projects prompt to star their own repo from the
installer using the user's authenticated `gh` credential. Opt-in and TTY-gated, and still the reason
neither project's star count can be cited as evidence of anything.

**A22 — Building core value on a host experimental flag.** slim's entire default orchestration story
requires `OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS=true` — which is the *only* reason its installer
edits shell rc files at all, and the reason its docs carry a troubleshooting section for when the flag
doesn't take. Muse ships ~45 `MUSE_EXPERIMENTAL_*` gates; this is a live risk, not a hypothetical.

**A23 — Auto-update as silent remote code installation.** slim's auto-update is **on by default** and
installs code from npm into a process that already has filesystem and shell access. It is unusually
well-guarded (channel-aware, major-version-blocked, pin-respecting, staged→verified→published,
timeout-killed) and still the default is silent remote code execution. `oh-my-claudecode`'s silent
background auto-update is **opt-in and off by default** — that is the correct polarity.

**A24 — Shipping a broken hero command.** `oh-my-mcode`'s README leads with `npx oh-my-mcode install`
against a package that is not on npm (**404**), under a GitHub Release badge on a repo with zero
releases. `witt3rd`'s documented `hermes skills tap add witt3rd/oh-my-hermes` indexes **zero** skills
because the tap defaults to a `skills/` path the repo does not have. `Salomondiei08`'s advertised
install step 4 is a script with a committed merge conflict marker that fails `bash -n`. **When the host
owns distribution, your one integration point must be exactly right — and none of these three ran an
end-to-end install test in CI against a clean `$HOME`.**

**A25 — Impenetrable policy prose as a substitute for design.** `oh-my-codex`'s README hedges the
tri-state `--merge-agents` flag across set/clear/force interactions, scope inheritance, and what
`false` explicitly does *not* promise. When documentation reads like a legal settlement, the design is
too complicated and should be redesigned.

---

## 5. What transfers to a compiled Rust host — and what is a JS/shell artefact

Muse Code is a single Rust binary with a JSON/markdown config surface. That deletes whole categories of
complexity these projects carry, and it forecloses one of them entirely.

### 5.1 Transfers unchanged (format, filesystem and lifecycle semantics — language-irrelevant)

| What | From | Rust note |
|---|---|---|
| Content-hash ownership ledger | OMX, OMC, rlaope, apm | `sha2` + `serde_json`; ~80 lines vs OMX's ~1,500 TS |
| Receipt = installer output, non-regular-entry sentinel | OMX | `serde` structs; the `__proto__` defense is unnecessary (no prototype chain) |
| Three-way merge with ancestor | slim | Directory SHA-256 over sorted `(relpath, kind, mode)` with symlinks skipped ≈ 40 lines |
| Marker-delimited managed regions + reserved USER region | OMC, OMX, oma | For JSON use a sibling key (`"_omm_managed": [...]`), not comments |
| Format-preserving config edits | omo (`jsonc-parser`), OMX (4,300 lines of hand-rolled TOML) | **`toml_edit` is format-preserving with real spans** — OMX's hardest-won correctness comes nearly free |
| Atomic write + verified backup + rollback | OMC's transaction, omo's symlink-aware write | `tempfile::NamedTempFile::persist` + `rustix` `O_EXCL`; real error types instead of exit-code encoding (OMC encodes rollback state as exit 5/6) |
| Advisory locks with PID liveness + staleness | rlaope, slim, omo, omp | `fs2`/`fd-lock` + `nix::kill(pid, None)`; `create_new(true)` is the same primitive |
| Allowlisted delete + filesystem-root guard + containment | omo | `Path::canonicalize` + `strip_prefix`; strictly better than string prefixes |
| Integer precedence ladder + first-wins dedup | omo, omp | `BTreeMap` merge + a stable sort |
| Capability × provider matrix | omp | `trait Provider<T> { fn priority(&self) -> u32; }` — **more natural in Rust than TS** |
| Mandatory `SourceMeta` provenance | omp | A required struct field makes "you can forget it" impossible |
| Capability-qualified disable ids, `suppress` vs `filter` | omp | Enum-typed ids |
| REPLACE + APPEND channels over a search path | slim | Pure path logic |
| Catalog-as-data with a status lifecycle | OMX | `serde` enum + validating `TryFrom` — better than the hand-rolled validator |
| Closed-schema manifest validation | omp (Agent Plugins 1.0.0) | `#[serde(deny_unknown_fields)]` gives it natively |
| Asset verification (size + sha256 + expected URL) | omp, rlaope | `sha2` + `reqwest` |
| Forward-rename pointer + dist escape hatch | omp | Pure protocol design |
| Install-provenance-driven self-update | rlaope (PEP 610, brew Cellar, env stamp) | `std::env::current_exe()` + an `install-provenance.json` written at install time |
| Deferred update after session exit | OMX | Same |
| Host capability probe; unparsed ⇒ all-false | mcode, OMX | Ideal use for a Rust enum + `Option` |
| Dated host-reality ledger | mcode | Pure documentation practice |
| Bidirectional manifest↔disk check | mcode | Trivial |
| Tracked/ignored project-state split | witt3rd, OMX | Pure convention |
| Generated multi-host mirror + CI drift gate | OMX, oma, wshobson | Same |
| Declarative host-adapter JSON | oma | `serde` struct with `#[serde(default)]` — plus **compile-time exhaustiveness over the host enum**, which TS cannot give them |
| Foreign-root ingestion table | omp's `SOURCE_PATHS` | A `const` table; more natural in Rust |
| Skill/rule frontmatter contracts, `/skillify`, INSTALL_FOR_AGENTS.md | OMC, slim, mcode | Pure markdown, 1:1 |
| Compact shims + lazy bodies | OMC, wshobson | Even more valuable with a compiled host: emit ≤240-char descriptions at build time, load bodies on invocation |
| Embedded payload materialised on first run | omo native | `include_dir!` / `rust-embed` |

### 5.2 Does NOT transfer — artefacts of a JS/Python/shell host

- **The entire in-process plugin model.** omp's `ExtensionAPI` (44 typed lifecycle events, `registerTool`,
  `registerMessageRenderer`, `registerProvider`, injected `pi.zod`/`pi.typebox`/`pi.arktype`, dynamic
  `import()` with `?mtime` cache-busting, a Bun `onLoad` specifier-rewriting hook, graph-wide hot
  reload) and slim's 53k-LOC TS plugin that mutates the host's in-memory config object all presuppose
  a shared JS heap a Rust binary does not have. **This is the biggest fork in the road.**
  Muse's substitutes, in ascending power: (a) declarative only — `.muse/hooks.json`, skills, rules,
  workflows, settings; (b) **subprocess hooks** — a hook is a command line, JSON on stdin, JSON on
  stdout: cross-language, sandboxable, and this should be the **default**; (c) **MSP over
  `muse serve`** — event subscription and tool registration *with* a process boundary, strictly better
  isolation than omp has; (d) WASM only if a real need survives (a)–(c).
  A plugin becomes *a program with a protocol*, not a function receiving a god-object.
- **Every interpreter-discovery workaround.** `resolveNodeBinary()`, `find-node.sh`,
  `~/.claude/.omc-config.json` `{nodeBinary}`, `hud/lib/config-dir.mjs`, the nvm/fnm `$PATH` hacks,
  `checkNodeVersion()`, `better-sqlite3`'s native addon and its prebuild warnings — all of it exists to
  answer "can this hook find a working node?" (~15% of OMC's installer complexity). A static binary
  deletes the class.
- **Runtime provisioning chains.** oma's three `curl|sh` provisioners (bun, uv, serena) plus the entire
  `serena-reaper` subsystem (25 KB impl + 36 KB tests) whose only job is killing leaked daemons — a
  subsystem that exists to clean up after an architectural choice. omo's `~/.codex/runtime/{node,ast-grep}`
  and `node-dispatch.ps1`. rlaope's venv, `pip --force-reinstall` and `importlib.resources`.
- **The 25-hook-spawns-per-turn interpreter tax.** OMC pays ~50–100 ms of Node startup per hook across
  11 events, which is why it needs a cache-occupancy hook and CI p50/p95 lock ceilings; oma measured
  ~620 ms p50 and its stated mitigations are "a leaner entrypoint" or "a future daemon phase". In Rust
  a hook is `omm hook <name>` on a static binary: **sub-5 ms cold start** — exactly what the
  third-party `oh-my-claudecode-RS` fork was built to prove. Do not port their daemon design; there is
  nothing to daemonise.
- **npm as distribution and update channel.** `npm install -g @latest`, dist-tag polling,
  `postinstall`/`prepack`/`npm pack`, package-manager ownership detection, semver ranges on 12 runtime
  deps, and 12 `optionalDependencies` platform packages with AVX2 probing / SIGILL retry / musl+baseline
  matrices. Rust target triples and static musl builds handle that at compile time; 12 platform
  packages collapse to a normal release matrix. *(Keep the **ideas**: version pinning, channels,
  rollback, deferred update, verified assets.)*
- **`bun install` + `node_modules` symlinking as the plugin package manager.** The transferable
  *shape* is the content-addressed cache (`cache/plugins/<marketplace>___<plugin>___<version>/`) plus
  links from a scope root, implemented over git + tarballs. Note omp's own marketplace **rejects npm
  sources**, so its git / git-subdir / `url+sha` model is the runtime-neutral part worth copying —
  and **make the SHA mandatory**, fixing omp's missing-integrity flaw at the same time.
- **Hand-rolled config parsers.** OMX's 4,300 lines of regex TOML line surgery, rlaope's 744-line
  line-based YAML editor, omo's bracket-counting JSON patcher, mcode's 64-line mini-YAML parser.
  `toml_edit`, `serde_yaml`, `serde_json` — keep the *policy* (preserve comments, refuse unknown
  shapes), drop every implementation.
- **900-line compensating transactions.** OMX's claim journals, ancestor topology preconditions and
  readback verification to write two config files. The goal is right; in Rust the answer is a ~50-line
  `write_atomic()` over `NamedTempFile::persist` plus an `fs2` advisory lock — as OMX's own
  `omx-runtime-core` Rust crate already does.
- **The plugin-cache repair subsystem.** OMC's ~600 lines of cache validate/repair/compact/sync exists
  *only* because the npm package and the plugin cache are two drifting copies of the same content.
  `include_dir!` makes them one copy.
- **Terminal scraping as an input channel.** omo's `tmux send-keys` SDK exists because Codex exposes no
  input channel. Muse has `muse serve`/MSP. Do not port a terminal-scraping hack into a
  protocol-having host.
- **JS runtime introspection.** `fileURLToPath(new URL('../..'))` for package root,
  `process.argv[1]` to distinguish a dev checkout from a global install, `import.meta.main`. A Rust
  binary knows where it is.
- **Bundled DOM/rendering stacks.** slim's `jsdom` + `@mozilla/readability` + `turndown` webfetch and
  `@opentui/solid` sidebar; rlaope's `.mjs` TUI widget. If Muse needs enriched fetch it's a crate or an
  MSP server.
- **Duplicate no-build implementations.** mcode ships `scripts/run-store.mjs` — 904 lines of plain JS
  re-implementing `src/store.ts` (762 lines) with **no parity test** — purely because skills need a
  tool that runs without `tsc`. **A compiled Rust binary IS the no-build tool.** The entire
  duplicate-implementation problem disappears by construction. Strict architectural win.
- **Non-OSI and absent licensing.** omo's Sustainable Use License means its code **cannot legally be
  vendored** into oh-my-musecode. OMX has no LICENSE file despite an MIT badge; `Salomondiei08` renders
  a false MIT badge linking to a file that does not exist. Ship MIT or Apache-2.0 with an actual file.

### 5.3 The three architectural decisions that follow

1. **Distribution:** signed per-platform release binaries + a checksum manifest (Homebrew tap,
   `cargo-binstall`, `curl|sh` with verification), with the **content payload versioned separately from
   the binary**. Not npm. Decide this first — every update and uninstall decision follows from it.
2. **Extension execution:** declarative first, subprocess hooks as the default executable contract,
   MSP over `muse serve` for anything needing event subscription or tool registration. Never in-process.
3. **Content authoring:** one canonical source tree, three generated manifests
   (`.muse-plugin/`, `.claude-plugin/`, `.codex-plugin/`), CI drift-gated.

---

## 6. Requirements for `oh-my-musecode`

Priority key: **MUST** = if it is missing at v0.1 it is either unfixable later or it destroys trust.
**SHOULD** = high leverage, schedulable. **NICE** = strategic, cheap, not blocking.

### 6.1 MUST

| # | Requirement | Derived from |
|---|---|---|
| M1 | **Make the ownership ledger the primitive, not a feature.** Every managed path gets an entry at write time: `{path, base: installRoot\|home, sha256, source_version, writer, mechanism (copy\|symlink\|junction\|region), class (exclusive\|shared+region+region_sha256\|seeded), scope}`. `install`, `update`, `reconcile`, `doctor` and `uninstall` become **the same traversal** over that ledger. Put it in `.muse/lock.json`. Entries sorted by `(base, path)` for byte-stable rerun; base-relative paths only (never absolute — survives dotfile sync); corrupt ledger renamed `.bad` and treated as absent. | OMC (built reactively after a duplicate-agents issue, needing 202 hand-generated history records to compensate); oma's design doc 023, specified and never implemented; apm's `deployed_files` + `deployed_file_hashes` |
| M2 | **The receipt records what the installer wrote, never a scan of the destination.** Include a non-regular-entry sentinel so a directory containing a symlink/socket can never compare equal and is therefore retained. | OMX (`"Digesting the destination tree instead would poison the receipt"`) |
| M3 | **Three-way merge with a recorded common ancestor, from the first release.** `ancestor = last_managed_hash`, `theirs = source_hash`, `mine = on_disk`. Four outcomes: no-op / overwrite / **adopt** / **stage**. | slim — the only implementation in the space, and its empty `LEGACY_MANAGED_SKILL_HASHES` proves retrofitting an ancestor after users have edited files is the hard case |
| M4 | **On conflict, stage next door and report; never overwrite, never freeze.** `~/.config/omm/updates/<version>/<asset>/` + a message naming exactly which assets need review, plus a deliberate `--force`/`--reset` discard escape hatch. | slim (stage); against oma/Salomondiei08/mcode (clobber) and witt3rd (freeze forever) |
| M5 | **Uninstall is a ledger traversal with a mandatory two-section preview (✗ remove / ✓ preserve), `--dry-run`, allowlisted roots, a filesystem-root refusal, containment via `canonicalize()` + `strip_prefix()`, and deepest-first ordering.** Hook registrations, marker regions, `.gitignore` lines, git hooks and cache dirs must be **ledger entries**, not afterthoughts. Name the residue you deliberately keep, per install channel. | rlaope (best uninstall found); omo (allowlist + root guard); oma (preview); against OMX (name-matched), OMC (broken, prints success), omp/slim (none at all) |
| M6 | **Ship a CI test that installs into a clean `$HOME`, uninstalls, and asserts the filesystem is byte-identical to its pre-install state.** Nobody in this sweep does this, and it is the cheapest possible guarantee. | The absence of it across all 14 teardowns; OMC's uninstaller was stale by two major versions with no test to catch it |
| M7 | **Two-phase install. Phase 1 places the binary and writes nothing else; phase 2 (`omm setup`) is the only thing that touches Muse config.** Refuse under `CI=true`/non-TTY without `--yes`; warn when setup-flavoured env vars are passed without opting in. | OMX shipped auto-setup and reverted it; rlaope's `OMH_RUN_SETUP=0`; mcode's CI refusal |
| M8 | **Ship a real override chain at v0.1: an overlay dir searched BEFORE bundled content, a published integer precedence ladder, and capability-qualified disable ids.** Both verbs — shadow and delete — with no forking. `~/.config/omm/custom/{skills,agents,rules,hooks}/` beats bundled; `disabled = ["skill:pdf", "rule:testing", "context-file:user:AGENTS.md"]`. | omo's `SCOPE_PRIORITY`; omp's capability matrix + disable ids; **against** A11 — its absence is what produced 5,630 forks / 312 contributors (omo) and "upstream it" as a documented answer (oma) |
| M9 | **Ship an APPEND channel alongside REPLACE:** `<name>.md` replaces, `<name>_append.md` appends, resolved independently over the same search path. | slim — the single highest-leverage idea in the sweep; it prevents most merges from ever existing |
| M10 | **No asset name may live in a Rust enum or const array.** The catalog is data with a lifecycle: `active \| alias \| merged \| deprecated \| internal`, a `canonical` forwarding field, a `core` flag, one-release sunset stubs, and a build gate that fails if a required-core item is deactivated. | OMX's catalog manifest (the model); **against** OMC's `KNOWN_AGENT_NAMES` const tuple, the central design failure of the biggest project in the space |
| M11 | **Mandatory provenance on every loaded item** — `SourceMeta{provider, path, level}` as a required struct field, item dropped with a warning if absent — and **retain shadowed items** so `omm doctor` / `omm list --json` can answer "why is this active and what is it hiding?". | omp; **against** omp's own weakness that shadowing is otherwise invisible to a confused user |
| M12 | **Pin and verify the transport.** A release tag or commit SHA, per-file sha256 (or a signature), and the **resolved ref + digest recorded in the lockfile**. Never a mutable-branch tarball. Verify size + digest + expected URL of every downloaded asset. | oma's central defect (unverified `main` tarball on every install AND update, with 418 unused hashes on disk); omp's asset verification as the model; the official Anthropic manifest's SHA-pinned sources |
| M13 | **Every write to a file you do not own is a format-preserving marker-delimited region edit** (`toml_edit` / JSONC AST; for JSON a sibling `"_omm_managed"` key, not comments), with an embedded version stamp, an atomic write on the **resolved realpath**, and a verified backup taken first. Preserve a reserved USER region on **every** regeneration path. | omo, OMX, OMC's transaction; **against** OMX's own non-uniform application and the two-config-writer bug (A13) |
| M14 | **Exactly one install topology. Ever.** | OMC's four-topology matrix (~600 lines of cache repair) and OMX's legacy/plugin dual mode — the two largest self-inflicted wounds found |
| M15 | **Hooks are `omm hook <name>` invoking the single static binary** — no interpreter discovery, no runtime provisioning, no `${VAR:-$HOME/...}` shell expansion. One dispatcher invocation per event running the whole handler chain, with a **CI-enforced latency budget** (target sub-5 ms, versus oma's measured 624 ms p50 against a 1500 ms SLO). Fail-open on a non-zero exit so a stale CLI can never wedge the agent. | oma (dispatcher + measured budget + fail-open wrapper); OMC/OMX/omo's Node-startup tax as the counterexample |
| M16 | **Negotiate capabilities against the actual binary before writing anything.** Parse `muse --version` into a capability table; probe which `MUSE_EXPERIMENTAL_*` gates exist and which are on; **an unparsed version yields ALL capabilities false.** Never make core value depend on an experimental gate. | mcode's `HostCapabilities`; OMX's `codex features list` probe; slim's `OPENCODE_EXPERIMENTAL_BACKGROUND_SUBAGENTS` dependency as the warning (A22) |
| M17 | **Executable extensions run out-of-process only** — subprocess hooks (JSON stdin → JSON stdout) as the default contract, MSP over `muse serve` for event subscription and tool registration. Third-party content is SHA-pinned and integrity-checked before it is loaded. | **against** omp: `git clone` → symlink → in-process `import()` with the agent's credentials, no signature/checksum/trust prompt, no sandbox, and a dropped project-trust gate — answered by the community with Docker and Landlock jails |
| M18 | **A strict security posture that can only tighten and that can VETO `--force`.** Symlink-escape rejection (`canonicalize` + containment) on every read of user- or repo-supplied content; treat repo-local skills and rules as an untrusted prompt-injection surface with size/count caps and injection/credential-shape refusal *with reasons*. In Rust make "config can only tighten" a **compile-checked lattice**, not a `\|\|`/`min()` convention. | rlaope's `--force` veto; OMC's tighten-only lattice and `disableProjectSkills`; omo's symlink-boundary tests; rlaope's rules-import guardrails |
| M19 | **Claim the namespace before the first release:** npm `oh-my-musecode` + bin `omm`, the GitHub topic `muse-plugin` (currently **0 repos**), a Homebrew tap, and one canonical name used by repo, package, plugin id and CLI. | The npm `oh-my-claudecode` squat by an unrelated project; three unrelated `oh-my-hermes` repos splitting ~2,460 stars; OMX's README anti-confusion clause; the DSH land-grab (11 repos in 10 days) |
| M20 | **One canonical source tree; `.muse-plugin/`, `.claude-plugin/` and `.codex-plugin/` manifests are GENERATED from it and byte-verified in CI** (with `version` normalised so a release bump alone does not false-positive). | OMX's `sync:plugin` + `verify:plugin-bundle`; oma's emit-drift gate; wshobson/agents' SSOT invariant |

### 6.2 SHOULD

| # | Requirement | Derived from |
|---|---|---|
| S1 | Ingest foreign roots inside the same precedence ladder: `.claude/`, `.codex/`, `.cursor/rules`, `.github/instructions/*.instructions.md`, `.github/copilot-instructions.md`, `.clinerules`, `.windsurfrules`, `.opencode/`, `.gemini/`. Publish the ladder as a table. | omp's `SOURCE_PATHS` (9 roots, 8,907 LOC); omo's 9-root `SOURCE_PRIORITY`; OMC's rules discovery; Crush reading `~/.claude/skills` |
| S2 | `omm doctor` as a first-class product: package checks → host presence/version → one real smoke exec → measured throughput or the literal string `unmeasured` **with a non-zero exit**. Include an `identity_conflicts` check from day one. Every finding ships a remediation command. | mcode's doctor triad; rlaope's 20 checks; wshobson's "every lint finding ships a fix string" |
| S3 | Bidirectional manifest↔disk validation (listed-but-missing **and** present-but-unlisted), plus frontmatter validation (`name == dirname`, description contains a negative-trigger clause). | mcode |
| S4 | Compact shims and lazy bodies: ≤240-char frontmatter descriptions emitted at build time, bodies in `skill-bodies/` loaded on invocation, with a hard body cap and `references/` overflow. | OMC's `compactPluginSkillPayload` + command dispatch stubs; wshobson's 8 KB cap; **against** rlaope's full-profile default and its warning apparatus |
| S5 | Record install-time decisions at install time inside the ledger (chosen bin dir, chosen scope, chosen manifest set), so uninstall reads what install *did*. | omo's `.installed-agents.json` / `.installed-bin-dir.json` |
| S6 | Deferred update after session exit; refuse to self-mutate a package-manager-owned install; detect the install channel from `current_exe()` + an `install-provenance.json` stamp. | OMX's update worker; omp's 6-way detection; rlaope's PEP 610 / brew Cellar introspection |
| S7 | Registration by pointer: exactly one entry written into Muse's own config; everything else lives in `~/.config/omm/`. | rlaope's single `skills.external_dirs` line; omo's and slim's single `plugin[]` entry |
| S8 | Maintain `docs/host-reality.md` — a dated observed-behaviour ledger of the Muse binary in the shape **fact → version observed → code constant → test that locks it**. The binary teardown already supplies the raw material. | mcode's `docs/host-reality.md` |
| S9 | `--scope user\|project`, persisted in a scope marker, resolved by walking **upward** from cwd so subdirectories work; a project scope must be provably unable to touch the user scope. | OMX's `resolveNearestPersistedSetupScopeSync` (issue #3447); omo's `CODEX_HOME` repointing |
| S10 | Refuse-rather-than-guess on config: fail closed on unknown shapes, quarantine invalid config to `.broken-<ts>-<pid>-<uuid>` under a lock and fail loudly naming **both** paths; `serde(deny_unknown_fields)` so a typo errors instead of being ignored. | rlaope's raising YAML mutator; omp's quarantine; omo's `.strict()` schemas |
| S11 | Ship `skills/oh-my-musecode/SKILL.md` (teaches the agent the `.muse/` + omm config schema), a `reflect`-style skill that mines session history for what to add, and an `INSTALL_FOR_AGENTS.md`. The installer's user is another agent. | slim's self-configuring skill pair; OMC's `/skillify`; mcode's `INSTALL_FOR_AGENTS.md` |
| S12 | Config-migration engine: applied-id list in the config, a write-ahead journal written by exclusive-create-then-rename, a PID+lease lock with renewal and a stale-owner multiplier, resume-after-crash, and `merge_without_clobber` where skipped values surface as **diagnostics, not silent drops**. | omo's migration engine; omo/rlaope's `mergeWithoutClobber` |
| S13 | Project-state convention with a tracked/ignored split, seeded once and never overwritten: `.muse/omm/{plans,specs,research}` tracked as decision records, `{state,logs,progress}` gitignored, with a seeded README explaining the split. | witt3rd's `.omh/`; OMX's `.gitignore` ignore/un-ignore split |
| S14 | Declarative host-adapter files (one JSON per host, schema-validated) with **named escape-hatch booleans** rather than scattered per-host conditionals. | oma's `variants/<vendor>.json` + `flatHookEntries` / `skipSettingsMerge` / `homeOnly` |
| S15 | `--dry-run` honoured on every write path, and a per-category converge summary (updated / unchanged / skipped / backed-up / removed) on both setup and uninstall. | OMX, slim, oma |

### 6.3 NICE

| # | Requirement | Derived from |
|---|---|---|
| N1 | **Become the index the others point at:** an auto-synced registry harvesting the `muse-plugin` GitHub topic on a schedule, published as a machine-readable catalog with trust levels. | The DSH land-grab — the two repos that took the auto-synced-catalog position (78★, 15★) beat the ones that shipped features |
| N2 | Adapter PRs adding `muse` to `vercel-labs/skills` (9.36M weekly downloads, 77 agents) and `withastro/rosie` (~60 agents). Two table rows unlock the entire existing skills corpus for Muse. | Registry sweep — neither knows about `muse` |
| N3 | Register Muse in the `agentskills.io` Client Showcase via its published "Adding skills support" implementor guide, and run `skills-ref/` conformance tests against `.muse/skills.lock`. | agentskills/agentskills (24,929★) has a spec, a reference impl and tests; Muse is not listed |
| N4 | A vendored, commit-pinned, read-only ecosystem index with an explicit claim boundary ("static catalog parsed from upstream README — not installation, runtime load, safety review, or endorsement evidence"). | rlaope's 216-item catalog |
| N5 | Profile switching (`omm profile use <name>`), since it was invented independently twice for one host. | `Switch-Omo-Config` (24★), `omo-switch` (26★), `oh-my-ccenv` |
| N6 | Multi-channel packaging layout (`npm/`, `debian/`, `aur/`, `freebsd-package/`, brew tap) once the binary is stable. | withastro/rosie's ready-made blueprint |
| N7 | `omm hud` as a subcommand, not a wrapper-script stack. | OMC's `omc-hud.mjs` + `omc-hud-cache.sh` + `find-node.sh` exists only because Node cold start is too slow for a statusline; the `oh-my-claudecode-RS` fork exists specifically to prove sub-5 ms |
| N8 | A/B prompt harness with a baseline content hash and a minimum-lift gate, plus a provenance table for reconstructed third-party content with upstream-drift automation. | oma's harness (`HARNESS_PASS_LIFT=0.05`, Merkle-ish `computeSuiteHash`); rlaope's `SKILL-SOURCES.md` with `reviewed_ref` diffing |

### 6.4 The v0.1 cut line

If only five things ship, ship these — they are the ones that cannot be retrofitted:
**M1** (ledger), **M3** (ancestor hash), **M8 + M9** (overlay + append channel), **M12** (pinned
verified transport), **M19** (namespace). Everything else can be added on top of a correct ledger;
none of these five can be added underneath one that was never written.

---

## 7. Differentiation — what `oh-my-musecode` can do that none of these can

Four structural advantages, none of which any competitor can copy without the same host.

### 7.1 The tri-manifest projection (the killer feature, and it is unexploited)

Muse Code's loader recognises `.muse-plugin/`, **`.claude-plugin/` and `.codex-plugin/`**. That is not
incidental — it is a distribution multiplier no other framework in this sweep has.

- **Inbound.** oh-my-musecode can install the *existing* Claude Code plugin ecosystem into Muse with
  zero re-authoring: 2,282 plugins in `anthropics/claude-plugins-community`, 291 in the official
  directory, **32,192** `.claude-plugin/marketplace.json` files in GitHub code search, plus
  wshobson/agents' 91 multi-harness plugins. Your catalogue is non-empty on day one and you author
  none of it. `oh-my-pi` proves the ingestion is worth ~9k LOC and buys a real ecosystem;
  `code-yeongyu/oh-my-openagent` proves you can win a foreign ecosystem *instead of* building a
  registry. Muse gives you both for free, natively.
- **Outbound.** A single generated bundle conforming to the documented `plugin.json` shape
  (`{name, version, description, author, homepage, repository, license, keywords, skills, hooks,
  mcpServers, apps, interface}` — spec published at
  `openai/codex codex-rs/skills/src/assets/samples/plugin-creator/references/plugin-json-spec.md`,
  where §"skills, hooks and string-valued mcpServers are supplemented on top of default component
  discovery") installs into **Muse Code, Claude Code and Codex CLI from one artifact**. Nobody else has
  this. `oh-my-claudecode` proved a fragment of it — it projects only its *MCP registry* into
  `~/.codex/config.toml` behind `# BEGIN/END OMC MANAGED MCP REGISTRY` markers. Muse's tri-manifest
  loader makes the full-plugin version possible.
- **Nobody has taken this position.** The only repo positioned as "marketplace for host X, Claude Code
  / Codex compatible" is `devswha/oh-my-gjc` at 34 stars.

### 7.2 The Rust binary deletes ~15–25% of every competitor's codebase by construction

Not an optimisation — a category deletion. Concretely, from the teardowns:

| Cost they pay | What it costs them | Muse cost |
|---|---|---|
| Interpreter discovery for hooks | `resolveNodeBinary()`, `find-node.sh`, `.omc-config.json{nodeBinary}`, nvm/fnm `$PATH` hacks — ~15% of OMC's installer | **zero** |
| Runtime provisioning | oma's 3 `curl\|sh` provisioners + a 61 KB `serena-reaper` subsystem; omo's `~/.codex/runtime/{node,ast-grep}` + PowerShell dispatch shims | **zero** |
| Hook latency | 25 spawns × ~50–100 ms (OMC); ~624 ms p50 with a CI SLO and a planned daemon (oma) | **sub-5 ms**, one static binary |
| Platform matrix | 12 npm `optionalDependencies` + AVX2 probing + SIGILL retry + musl/baseline variants (omo) | a normal Rust release matrix |
| Package/cache drift | ~600 lines of plugin-cache validate/repair/compact/sync (OMC) | `include_dir!` — one copy |
| Config parsing | 4,300 lines of hand-rolled TOML surgery (OMX), 744-line YAML editor (rlaope), bracket-counting JSON patcher (omo) | `toml_edit`, `serde_yaml`, `serde_json` |
| Atomic-write transaction | ~900 lines of claim journals + rollback + exit-code-encoded state (OMX) | ~50 lines: `NamedTempFile::persist` + `fs2` |
| Duplicate no-build tooling | mcode's 904-line JS re-implementation of its own 762-line store, with no parity test | **zero** — the binary *is* the no-build tool |
| Type-level guarantees | "config can only tighten" as a `\|\|`/`Math.min` convention (OMC); a `--force` veto as a runtime check (rlaope) | a compile-checked lattice; `deny_unknown_fields`; exhaustive host enums |

The competitors' most expensive problems are our non-problems. That budget should be spent on the
things none of them built: the ledger, the overlay, the registry.

### 7.3 The host ships the primitives everyone else faked

- **`.muse/lock.json` (provenance / quarantine / allowed_tools)** is the ownership ledger OMC needed
  **202 hand-generated historical hash records** to approximate, that oma only *designed*, and that
  `microsoft/apm` had to invent from scratch as `deployed_file_hashes`. We start where they stopped.
- **`.muse/skills.lock`** is the install receipt OMX built by hand as
  `.omx/state/setup/installed-skills.json` — and it means the *host* may do receipt-keeping for us.
- **MSP over `muse serve`** is a real protocol replacing omo's `tmux send-keys` terminal scraping (a
  workaround for Codex exposing no input channel) and omp's in-process `import()` (a workaround for
  having no process boundary at all). We get event subscription and tool registration **with**
  isolation — strictly better than omp's architecture, not a port of it.
- **`.muse/hooks.json`** is the declarative event bus witt3rd's host never gave it and that OMC/OMX
  emulate with 25 interpreter spawns.
- **~45 `MUSE_EXPERIMENTAL_*` gates** turn mcode's host-version→capability table into a *probeable*
  feature matrix rather than a guessed one.

### 7.4 The enterprise policy layer nobody has

This is the largest genuinely unclaimed gap in the space, and Muse's primitives make it cheap:

- **Integrity for third-party content.** `oh-my-pi` — the only project with a real marketplace protocol
  — has **no signature, no checksum, no integrity pinning, no capability manifest and no trust prompt**
  on plugin install, and no sandbox on plugin load. Its `git`/`git-subdir`/`url+sha` source model
  already supports a SHA; make it **mandatory**, record the resolved digest in `.muse/lock.json`, and
  gate load on `quarantine`.
- **Tool-level policy as a type.** `allowed_tools` in `.muse/lock.json` + a tighten-only Rust enum
  lattice makes "a repo-local config can never widen permissions" a compile-time property.
  `oh-my-mcode` implements the same idea as a runtime check (a workspace config cannot raise
  `permission` to `full`) and rlaope as a `--force` veto; in Rust it is the type system.
- **Auditability.** M11's mandatory provenance plus M1's ledger makes `omm doctor --json` answer
  "what is installed, from where, at what digest, shadowing what, with which tools allowed" — the
  question a security team asks and that **no project in this sweep can answer**.
- **Precedent exists and is a validation, not competition:** `microsoft/apm` ships `policy`, `approve`
  and `audit` verbs plus a `CONFORMANCE.json` with a Governance class; `trailofbits/skills-curated`
  (496★) is a security vendor shipping a *vetted* catalog. The demand is real and unserved for Muse.

### 7.5 Timing

`topic:muse-plugin` = **0 repos**. The entire third-party Muse ecosystem is six repos at ≤2 stars,
three of which are model/provider shims — the same first move every ecosystem makes. `oh-my-muse` is a
dead empty squat; `oh-my-musecode` and npm `omm` are free. The DSH precedent says the window closes in
days-to-weeks once the host's user base crosses a threshold, and that **the winner is whoever becomes
the catalog the others point at**, not whoever ships first or ships most.

---

## 8. Open risks and things nobody has solved

1. **Nobody has shipped content *and* a correct lifecycle.** `oh-my-pi` has the best substrate and
   **zero** user-facing skills. OMC/OMX/omo ship hundreds of assets and have no override chain. The
   synthesis is unproven; assume it is harder than either half.
2. **Registry governance is unsolved everywhere.** Every "marketplace.json" in this sweep is a
   single-vendor self-registration file. The only working models are the *hosts'* own registries
   (Claude Code's marketplaces, Hermes' Skills Hub, Codex's `Local|Git|Npm` sources) — and two projects
   declined to use the registry their host already provided. Prefer being a well-behaved entry in
   Muse's registry over operating a competing one; add a curated index only if Muse has none.
3. **Trust levels and vetting have no accepted answer.** Hermes' Skills Hub has
   `builtin | trusted | community` with signed `skill.oms.sig`; Anthropic's manifest has SHA pins and
   `strict: true`; omp has nothing. Pick a model early — it is a schema decision, and schema decisions
   are the ones you cannot change later.
4. **Muse's own extension surface is reverse-engineered, not documented.** M16's capability probe and
   S8's host-reality ledger are the mitigations. Assume `.muse/hooks.json`'s schema, the
   `MUSE_EXPERIMENTAL_*` set and MSP's wire format will all move under you.
5. **`agentOverrides`-shaped traps.** Several projects have a field that *looks* like an override and
   is not (OMC's `agentOverrides` sets only a routing tier). Whatever omm's override field is called,
   its capability must be obvious from its name.
6. **Agent-authored velocity is a governance risk.** rlaope is ~53% agent-authored commits with
   1,097 PRs in 90 days; omo has 916 open issues on one maintainer; omp has 1,261 open issues and
   ~85 commits/day. The thing holding these together is enormous test suites — omo 2,353 test files,
   omp 40% test ratio, slim 1.25:1 test:source with the riskiest module carrying more test than source.
   Budget for that ratio from the start, especially around M1/M3/M5.
7. **The "slim" counter-position is not available.** slim won 8,560 stars and the best download
   conversion in the space by being *slim against a 68k-star maximalist incumbent*. Muse has no
   incumbent oh-my-* framework, so there is nothing to counter-position against — take slim's
   engineering, not its positioning.

---

## 9. Source index

Full teardowns (all cloned and read at source level, all independently verified):

| Report | Project | Verdict |
|---|---|---|
| `oh-my-claude-code.md`, `yeachan-heo-oh-my-claudecode.md` | Yeachan-Heo/oh-my-claudecode | SOLID / MOSTLY_SOLID |
| `oh-my-codex.md`, `yeachan-heo-oh-my-codex.md` | Yeachan-Heo/oh-my-codex | MOSTLY_SOLID |
| `oh-my-opencode.md`, `code-yeongyu-oh-my-openagent.md` | code-yeongyu/oh-my-openagent | MOSTLY_SOLID |
| `oh-my-pi.md`, `can1357-oh-my-pi.md` | can1357/oh-my-pi | MOSTLY_SOLID |
| `alvinunreal-oh-my-opencode-slim.md` | alvinunreal/oh-my-opencode-slim | MOSTLY_SOLID |
| `first-fluke-oh-my-agent.md` | first-fluke/oh-my-agent | MOSTLY_SOLID |
| `rlaope-oh-my-hermes.md` | rlaope/oh-my-hermes | MOSTLY_SOLID |
| `haoruilee-oh-my-mcode.md` | haoruilee/oh-my-mcode | MOSTLY_SOLID |
| `witt3rd-oh-my-hermes.md` | witt3rd/oh-my-hermes | MOSTLY_SOLID |
| `salomondiei08-oh-my-hermes.md` | Salomondiei08/oh-my-hermes | MOSTLY_SOLID |

Discovery sweeps: `discovery-gh-name-sweep.md` (54 stem variants, 381 verified repos),
`discovery-gh-content-sweep.md` (topic + manifest-marker code search, 12 clones),
`discovery-registry-sweep.md` (npm / PyPI / Homebrew / marketplace manifests),
`discovery-cn-community-sweep.md` (CN/JP/KR cluster, DSH ecosystem, Cordis lineage).

**Nothing in the brief was refuted as non-existent.** Two of the four projects the brief named by
implication are not what their names suggest: `oh-my-pi` is a fork of the Pi agent, not a config layer
(the literal "oh-my-zsh for pi" is `ifiokjr/monopi`, 149★, three SKILL.md files), and
`oh-my-opencode` has been renamed to `oh-my-openagent` while keeping its npm name — it is an agent
harness spanning four hosts, not an OpenCode config layer.
