# oh-my-pi (`omp`) — architecture teardown

Researched 2026-09-01. Source read from a real clone at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/oh-my-pi/`
(`git clone --depth 50 https://github.com/can1357/oh-my-pi.git`, 261 MB working tree).
Upstream Pi cloned to `.../scratchpad/ohmy/pi-mono/`; `ifiokjr/monopi` cloned to `.../scratchpad/ohmy/monopi/`.

---

## 0. Disambiguation: which "Pi"?

"Pi" here is **the Pi coding-agent harness by Mario Zechner (`@badlogic`)** — not Inflection's Pi
chatbot, not Raspberry Pi, not the `pi` math constant.

- Upstream repo `github.com/badlogic/pi-mono` **redirects to `github.com/earendil-works/pi`**
  (verified: `gh api repos/badlogic/pi-mono` returns `"full_name":"earendil-works/pi"`).
  100,385 stars / 12,470 forks / 166 open issues / created 2025-08-09 / pushed 2026-09-01.
  npm scope `@earendil-works/pi-coding-agent`. MIT. TypeScript.
- **`can1357/oh-my-pi`** is a hard fork of that, published as `@oh-my-pi/pi-coding-agent`,
  binary name **`omp`**, site `omp.sh`. The README states it plainly at
  `oh-my-pi/README.md:22-24`: *"Fork of Pi by @mariozechner"*.

**The key surprise for our purposes:** oh-my-pi is **NOT** an oh-my-zsh-style config layer sitting
on top of Pi. It is a *fork of the agent binary itself* — a 470k-LOC TypeScript + 220k-LOC Rust
monorepo that replaces Pi wholesale. The "oh-my-" prefix is branding for "the batteries-included
build", not a plugin-framework architecture.

The literal "oh-my-zsh for pi" config framework does exist separately and is much smaller:
**`ifiokjr/monopi`** (149★, "One-click setup for pi-coding-agent — extensions, themes, prompts,
skills, and ant-colony swarm. Like oh-my-zsh for pi."). It ships only 3 `SKILL.md` files; it is a
TS monorepo of installer packages, not a large content library. Noted for completeness; the real
architecture lesson is in oh-my-pi.

---

## 1. Does it exist? — **CONFIRMED**

`https://github.com/can1357/oh-my-pi` · MIT · TypeScript+Rust · default branch `main`.

---

## 2. What it SHIPS (real counts)

Everything below is counted from the clone, not from the README.

### The agent itself
| Thing | Count | Evidence |
|---|---|---|
| TypeScript files (packages + crates) | 4,667 | `find packages crates -name '*.ts'` |
| Rust files | 402 | `find crates packages -name '*.rs'` |
| Rust LOC in `crates/` | 222,773 | `find crates -name '*.rs' -exec cat {} + \| wc -l` |
| TS LOC in `packages/coding-agent/src` | 472,811 | same method |
| Workspace packages | 17 | `packages/` (agent, ai, catalog, coding-agent, tui, natives, wire, …) |
| Rust crates | 9 + vendor | `crates/` (pi-ast, pi-builtins, pi-iso, pi-natives, pi-shell, pi-vcs, pi-voice, pi-walker) |
| Internal design docs | 130 `.md` | `find docs -name '*.md'` |

### Content categories a user actually consumes
| Category | Count | Path |
|---|---|---|
| Built-in LLM tools | **29** (+3 hidden: `yield`, `goal`, `think`) | `packages/coding-agent/src/tools/builtin-names.ts` |
| Tool implementation files | 99 | `packages/coding-agent/src/tools/` |
| Built-in slash commands (top-level) | **77** (17 modes + 10 collab + 19 session + 24 lifecycle + 3 marketplace + 4 control), plus aliases and nested subcommands (~84 distinct `name:` literals) | `packages/coding-agent/src/slash-commands/builtin-*.ts` |
| Bundled subagents | **7** — `scout`, `designer`, `reviewer`, `security-reviewer`, `librarian`, `task`, `sonic` | `src/task/agents.ts` + `src/prompts/agents/*.md` (8 md files incl. `frontmatter.md`) |
| Built-in **rules** (glob/condition-triggered guidance) | **27** `.md` (8 Go, 6 Rust, 13 TS) | `packages/coding-agent/src/discovery/builtin-rules/` |
| Prompt markdown files (tool prompts, system, advisor, memories, security, steering, skills…) | **176** | `packages/coding-agent/src/prompts/**` |
| TUI themes | **98** JSON | `packages/coding-agent/src/modes/theme/defaults/*.json` |
| Model catalog | **66 providers / 4,705 models** | `packages/catalog/src/models.json` |
| LSP server presets | **54** | `packages/coding-agent/src/lsp/defaults.json` |
| DAP debug adapters | **14** | `packages/coding-agent/src/dap/defaults.json` |
| Settings schema | 6,327 lines | `packages/coding-agent/src/config/settings-schema.ts` |
| **Bundled `SKILL.md` skills for end users** | **0** | `find packages -name SKILL.md` → 0 |

### Repo-local dogfood config (NOT shipped to users)
`oh-my-pi/.omp/` — 14 files: 5 slash commands (`review-prs`, `fix-issues`, `triage`, `release`,
`cleanup`), 3 skills (`semantic-compression`, `system-prompts`, `tool-prompt-optimization`), 1
custom tool (`tools/tui.ts`). This is the project's own `.omp` directory, the same surface any user
repo gets — a nice self-demonstration, but it is not distributed content.

**Bottom line on "what it ships":** oh-my-pi ships *capability*, not a *content library*. There is
no curated pack of 100 subagents or 50 slash commands. Its 27 built-in rules and 176 prompt files
are the only real curated-content surface, and they are compiled into the binary rather than
installed as user-editable files.

---

## 3. INSTALL — read end to end

Primary installer: `scripts/install.sh` (334 lines) and `scripts/install.ps1`.
Served from `https://omp.sh/install`.

### What it does
1. Parses `--source | --binary | --ref <tag>` (`install.sh:20-60`).
2. `host_arch()` (`:74`) — on macOS uses `sysctl hw.optional.arm64` instead of `uname -m`, so a
   Rosetta shell can't be fooled into an x86_64 install. Genuinely thoughtful.
3. `bun_arch_matches_host()` (`:97`) — if bun exists but reports a different arch than the host,
   it **refuses the source install** and falls back to the prebuilt binary (`:307-331`), with an
   explanatory message. This is the kind of failure most installers ship broken.
4. Source mode: `bun install -g @oh-my-pi/pi-coding-agent` (or clone at `--ref` + `git lfs pull` +
   `bun install -g <tmpdir>/packages/coding-agent`).
5. Binary mode: detects `linux | linux-musl | darwin` × `x64 | arm64`, resolves the tag from the
   GitHub releases API, downloads `omp-<platform>-<arch>` → `${PI_INSTALL_DIR:-$HOME/.local/bin}/omp`,
   `chmod +x`.
6. **Smoke test before claiming success** (`:270-290`): runs `omp --version` and, if it fails,
   prints the actual error and (on musl) the exact `apk add libstdc++ libgcc` remedy, then exits 1.
   Refusing to print a green checkmark for a binary that can't start is rare and correct.
7. Warns if `$INSTALL_DIR` is not on `$PATH`. Does not edit shell rc files.

### What it writes to the user's machine at install time
**Exactly one file**: `~/.local/bin/omp` (binary mode) — or a bun global package + shim (source
mode). No dotfiles, no shell rc edits, no config seeded. Everything else is created lazily at
first run. Also available via Homebrew tap (`can1357/tap/omp`), Nix flake (with
`nixosModules.default` and `homeManagerModules.default` for declarative settings), and `mise`.

---

## 4. EXTENSION CONTRACT — how a user adds their own thing

This is the strongest part of the design. There is no forking, no copy-paste, and **five distinct
mechanisms** layered by escalating power.

### 4.1 Drop a file in a scanned directory (zero config)
Native discovery (`packages/coding-agent/src/discovery/builtin.ts`, priority 100) scans:

- **Project:** `<cwd>/.omp/{commands,skills,hooks,tools,rules,prompts,agents,extensions,instructions}/`
  plus `<cwd>/.omp/{settings.json,config.yml,mcp.json,SYSTEM.md,RULES.md,AGENTS.md}`
- **User:** the active profile's agent dir (default `~/.omp/agent/`) with the same subdir set

No registration step. A `SKILL.md` under `.omp/skills/<name>/` is live on next start.

### 4.2 Point at a path from settings
```yaml
# ~/.omp/agent/config.yml
extensions:
  - ~/my-exts/safety.ts
  - ./local/ext-pack
```
plus CLI `--extension/-e <path>`, `--hook <path>`, `--plugin-dir <dir>`, `--trusted-extension <abs>`.
Directory resolution order (`extensibility/extensions/loader.ts`, documented in
`docs/extension-loading.md:200-230`): `package.json#omp.extensions` → `index.ts` → `index.js` →
one-level scan. Never recursive beyond one subdir level. TS preferred over JS. Symlinks honored.

### 4.3 Write a TypeScript extension module (full runtime API)
Default-export a factory taking `ExtensionAPI` (`docs/extensions.md`, 747 lines). Surfaces:
`pi.on(event)`, `registerTool`, `registerCommand`, `registerShortcut`, `registerFlag`,
`registerMessageRenderer`, `registerComposerShape`, `registerProvider` (incl. custom usage/billing
reporting and `fetchDynamicModels`), `registerFileWriteFallback`, `setModel`, `setThinkingLevel`,
`setServiceTier`, `setActiveTools`, `sendMessage` with four delivery semantics
(`steer` / `followUp` / `nextTurn` / `triggerTurn`), plus injected `pi.zod`, `pi.arktype`,
`pi.typebox` schema builders.

Two-phase lifecycle enforced by the runtime: during load only *registration* is legal; calling an
action method throws `ExtensionRuntimeNotInitializedError` until `ExtensionRunner.initialize()`
wires the live session. Load errors are captured per path (`{path, error}`) and never abort the
other extensions; handler exceptions are caught and surfaced as extension errors rather than
crashing the loop.

Hook events (`docs/hooks.md`): ~30 typed events — `session_start`, `session_before_compact`
(cancellable), `session.compacting` (can rewrite the compaction context), `context` (can rewrite
messages), `tool_call` (can `{block, reason}` **or rewrite `input`**), `tool_result` (can rewrite
`content`/`isError`), `turn_start/end`, `auto_retry_*`, `ttsr_triggered`, `todo_reminder`, …

### 4.4 Package it (`package.json#omp`)
```json
{ "omp": { "extensions": ["./src/a.ts"], "tools": "./tools.ts", "hooks": "./hooks.ts",
           "commands": ["./cmds/x.md"],
           "features": { "beta": { "description": "...", "default": false, "extensions": ["./beta.ts"] } },
           "settings": { "apiKey": { "type": "string", "secret": true, "env": "MY_KEY" } } } }
```
`packages/coding-agent/src/extensibility/plugins/types.ts` — a plugin can declare **optional
features** (installed with bracket syntax `pkg[a,b]`, `pkg[*]`, `pkg[]`) and a **typed settings
schema** (`string|number|boolean|enum`, with `secret: true` masking and `env:` fallback). The
legacy `pi` key is still accepted. A plugin root is also convention-scanned for
`skills/ hooks/ tools/ commands/ rules/ prompts/ agents/ mcp.json`.

### 4.5 Publish to a marketplace (see §6)

### 4.6 Overriding without forking — the real answer
Three orthogonal mechanisms, all in `docs/extension-loading.md:110-150` and `docs/skills.md`:

- **Shadowing by precedence.** Every capability is deduped by a stable key (skills by `name`, rules
  by `name`, extensions by absolute path, agents by `name`) with providers sorted by priority:
  `native` 100 > `omp-plugins` 90 > `claude` 80 > `agent-plugins` 75 > `claude-plugins`/`agents`/`codex` 70
  > `opencode` 55 > `github` 30 > `omp-managed` (auto-learned) 5. Dropping `~/.omp/agent/skills/pdf/SKILL.md`
  silently overrides a marketplace plugin's `pdf` skill. **First-wins, highest-priority-first** — no merge, no conflict file.
- **`disabledExtensions`** — one unified deny-list keyed by capability-qualified id:
  `extension-module:foo`, `skill:pdf`, `context-file:user:CLAUDE.md`. Every capability that defines
  `toExtensionId` contributes to the same list. Elegant: one setting turns off *anything*.
- **`plugin-overrides.json`** at `<project>/.omp/` — read-only from the manager's perspective, lets
  a repo disable plugins or override features/settings for everyone who clones it without touching
  the user's global lockfile.

### 4.7 Cross-vendor ingestion — the standout design
`packages/coding-agent/src/discovery/` is 8,907 LOC of *other people's config formats*:
`claude.ts` (592), `claude-plugins.ts` (675), `codex.ts` (553), `opencode.ts` (538), `gemini.ts` (386),
`github.ts` (337), `cursor.ts` (223), `windsurf.ts` (149), `vscode.ts` (106), `cline.ts` (83),
`agent-plugins.ts` (341) + `agent-plugin-format.ts` (551).

`SOURCE_PATHS` (`discovery/helpers.ts:31-84`) is a single declarative table mapping each vendor to
`{userBase, userAgent, projectDir}`:
`.omp` / `.claude` / `.codex` / `.gemini` / `.config/opencode`+`.opencode` / `.cursor` /
`.codeium/windsurf`+`.windsurf` / `.cline` / `.github` / `.vscode`.

And critically it implements **Agent Plugins 1.0.0** (`https://agent-plugins.org`) — a real
cross-vendor portable-plugin standard with a *closed* `plugin.json` manifest schema, `mcp.json`,
`${PLUGIN_ROOT}` / `${PLUGIN_DATA}` placeholder expansion, and package-boundary containment
(`agent-plugin-format.ts:1-16`). `classifyAgentPluginRoot()` is consulted by the Claude and OMP
legacy loaders so a standard-conformant root is never double-loaded through legacy conventions.
The manifest is validated strictly: unknown top-level fields → warning, closed-schema violation →
**the whole plugin is rejected and none of its components load** (spec §5.2).

Marketplace catalogs read `.omp-plugin/marketplace.json` first and fall back to
`.claude-plugin/marketplace.json` — **omp installs Claude Code plugins unmodified**. Same trick
Muse Code is doing with `.claude-plugin`/`.codex-plugin`.

---

## 5. UPDATE — and how it avoids clobbering

### The agent
`omp update [--check] [--force] [--plugins] [--canary|--stable]`
(`packages/coding-agent/src/commands/update.ts`, `src/cli/update-cli.ts`, 2,073 lines).

Real engineering here, not a `curl | sh` re-run:
- Detects the install method (nix store path, Homebrew, mise, bun/npm global, raw binary) and
  refuses to self-mutate a Nix-store or brew-managed install.
- Release channel selection: `stable` / `canary`.
- `omp.dist: "npm" | "binary"` in the published manifest lets the maintainer *push* installs from
  npm to prebuilt binary when a runtime like bun breaks; unknown values map to `"binary"` as the
  safe escape hatch.
- `omp.rename: { package, natives }` is a **forward pointer**: an updater that understands it
  follows the rename to a new npm package name; an older deployed updater ignores it and takes the
  `dist: "binary"` escape hatch. A migration protocol designed for updaters *already in the field*.
- Downloads verify GitHub-reported **asset size and SHA-256 digest**, reject an asset with
  duplicate names, an unfinished upload, an unsupported digest, or an unexpected download URL
  (`update-cli.ts:183-346`). 15-minute hard timeout.

### Plugins
- Marketplace: `/marketplace upgrade [name@marketplace]`, `marketplace.autoUpdate: off|notify|auto`
  with a 24h catalog staleness check. Semver-aware; non-semver treated as changed-when-unequal.
  Per-plugin failure is skipped so an all-upgrade can partially succeed.
  (Documented wart: `notify` mode "writes update availability only to the debug log; it does not
  show a user-facing notification" — `docs/marketplace.md:236`. The default mode is a no-op.)
- npm/git plugins: **there is no update action.** `PluginManager` has no `update`; you re-run
  `omp plugin install pkg@newVersion` (`docs/plugin-manager-installer-plumbing.md:60`).

### Lockfiles and clobber-avoidance
Three separate state files, deliberately split by concern:
- `~/.omp/plugins/package.json` — the dependency manifest bun operates on
- `~/.omp/plugins/omp-plugins.lock.json` — **runtime state**: per-plugin `{version, enabledFeatures, enabled}` + a separate `settings` map
- `~/.omp/plugins/installed_plugins.json` (v2) — marketplace-scoped installs (user), mirrored at `<project>/.omp/plugins/installed_plugins.json`

Clobber-avoidance is genuinely good:
- **User settings never live in the lockfile's plugin-state entry.** Uninstall/reinstall replaces
  `config.plugins[name]` but the `config.settings[name]` map is a distinct key; an upgrade replaces
  version/features and leaves settings intact.
- **Install is transactional with rollback.** Before `bun install`, the manager snapshots the
  package tree, `plugins/package.json`, and `bun.lock`. Then it *validates every declared extension
  entry* — each must resolve on disk, import to a factory function, **and initialize successfully
  against a throwaway registration surface**. Any failure (feature validation, extension
  validation, runtime-config save) restores all three snapshots and aborts
  (`docs/plugin-manager-installer-plumbing.md:120-140`). Very few agent plugin managers do a
  smoke-import before committing.
- **User config is never written by the installer.** `settings.set()` writes only the *global*
  layer; project settings and `--config` overlays are read-only from the settings API
  (`docs/config-usage.md:150-160`). So installing a plugin cannot rewrite a repo's `.omp/config.yml`.
- **Broken config is quarantined, not overwritten.** An invalid global/native-project YAML is moved
  under a file lock to `.broken-<timestamp>-<pid>-<uuid>` and startup fails loudly with both paths.
  An *unreadable* file fails without being moved. Overlay files from `--config`/`PI_CONFIG_FILES`
  are strict: missing file, invalid YAML, or non-mapping root are hard errors and are never quarantined.

---

## 6. REGISTRY

Yes — a real one, and it is **deliberately Claude Code-compatible**.

- `/marketplace add <source>` where source is `owner/repo`, an https git URL, `git@`/`ssh://`, a
  local `./path`, or a direct `*.json` catalog URL.
- Catalog file: `.omp-plugin/marketplace.json` preferred, `.claude-plugin/marketplace.json` as
  fallback — and the docs explicitly recommend publishing **both** so one repo serves omp and
  Claude Code from separate files with the same schema (`docs/marketplace.md:60-70`). The
  `$schema` in the example is literally `https://anthropic.com/claude-code/marketplace.schema.json`.
- Plugin sources: relative path in-repo, `github` shorthand with `ref`+`sha`, `url`+`sha`,
  `git-subdir` (monorepo path), and `npm` (parsed but **rejected at install**: "npm plugin sources
  are not yet supported").
- Two install scopes, `user` (default, `~/.omp/plugins/`) and `project` (`<repo>/.omp/plugins/`),
  where an **enabled project install shadows an enabled user install** of the same plugin but a
  *disabled* project install does not shadow.
- Full CLI parity with the slash commands: `omp plugin marketplace add|remove|update|list`,
  `omp plugin discover|install|uninstall|upgrade|enable|disable|list|link|doctor|features|config`.
- Naming rules are enforced: `^[a-z0-9][a-z0-9.-]*[a-z0-9]$`, ≤64 chars, plugin id
  `name@marketplace` ≤128.
- Content cache: `~/.omp/plugins/cache/marketplaces/<name>/` and
  `cache/plugins/<marketplace>___<plugin>___<version>/`, symlinked into `plugins/node_modules/`.

There is no first-party curated index of oh-my-pi plugins — discovery is "add someone's repo".
The practical registry is Anthropic's `anthropics/claude-plugins-official`, which omp consumes
directly. That is a deliberate free-ride on a competitor's ecosystem, and it works.

---

## 7. UNINSTALL — **not clean**

- **There is no `omp uninstall`.** `grep -rn uninstall docs/*.md` returns only plugin-uninstall
  hits; the README has zero occurrences of "uninstall". Confirmed by reading
  `packages/coding-agent/src/commands/` (42 commands, none of them uninstall).
- Removal is manual: delete `~/.local/bin/omp` (or `bun uninstall -g @oh-my-pi/pi-coding-agent`,
  or `brew uninstall`), then `rm -rf ~/.omp`.
- The residue is substantial. Enumerated from `packages/utils/src/dirs.ts` — under `~/.omp/`:
  `agent/` (config.yml, settings.json, sessions/, blobs/, agent.db, history.db, models.db,
  memories/, managed-skills/, themes/, commands/, prompts/, tools/, modules/, keybindings,
  terminal-sessions/, python-gateway/, secret-placeholder.key, omp-crash.log, omp-debug.log,
  cache/{composer,document-conversions,tiny-models}), plus root-level `plugins/`, `marketplaces.json`,
  `logs/`, `reports/`, `security/`, `autoresearch/`, `wt/` (git worktrees!), `remote/`,
  `remote-host/`, `ssh-control/`, `python-env/` (a managed venv), `puppeteer/`, `browser-relay/`,
  `natives/`, `webcache/`, `cache/{avatars,fastembed,fastembed-runtime,github-cache.db,
  commit-inference.db,legacy-pi-extension-cache.db,auth-broker-snapshot.enc}`, `run/{daemons,
  provider-inflight}`, `autoqa.db`, `stats.db`, `gpu_cache.json`, `profiles/<name>/…`.
- Plus credentials at the auth store and, on XDG systems, split state under
  `$XDG_DATA_HOME/omp`, `$XDG_STATE_HOME/omp`, `$XDG_CACHE_HOME/omp` (created by
  `omp config init-xdg`, which "does not move existing data" — so you can end up with *both*).
- `omp gc` exists but is a maintenance sweeper (blobs, cold sessions, WAL checkpoints), not an
  uninstaller.

Upstream Pi is marginally better: `docs/quickstart.md` has an explicit Uninstall section that
names the package-manager command per manager — and then honestly states "Uninstalling pi leaves
settings, credentials, sessions, and installed pi packages in `~/.pi/agent/`."

---

## 8. TRACTION (measured 2026-09-01 via `gh api` and the npm registry)

| Metric | Value | How |
|---|---|---|
| Stars | **28,857** | `gh api repos/can1357/oh-my-pi` |
| Forks | **2,884** | same |
| Watchers (subscribers) | 90 | same |
| Open issues+PRs | 2,042 | same |
| Open issues only | **1,261** | `gh api search/issues?q=…type:issue+state:open` |
| Closed issues | **3,386** | search API |
| Merged PRs | **3,129** | search API |
| Contributors | **408** | `Link:` rel=last header on `/contributors?per_page=1` |
| Releases | **583** | `gh api releases --paginate` |
| Latest release | **v18.1.0**, 2026-09-01T14:00Z | releases API |
| Release cadence | v18.0.8 → v18.1.0 in 5 days (18.0.8 Aug 27, .9 Aug 28, .10 Aug 28, .11 Aug 29, 18.1.0 Sep 1) | releases API |
| Created / last push | 2025-12-31 / **2026-09-01T13:33Z (same day)** | repo API |
| Repo size | 578,969 KB | repo API |
| npm downloads, last week | **113,949** | `api.npmjs.org/downloads/point/last-week/@oh-my-pi/pi-coding-agent` |
| npm downloads, last month | **447,565** | same, last-month |
| npm dist-tag latest | 18.1.0 (605 published revisions) | `registry.npmjs.org` |

For calibration: upstream `earendil-works/pi` has 100,385★ / 12,470 forks. oh-my-pi is a ~29%-of-upstream
fork that has become its own ecosystem — GitHub search returns 25+ third-party repos in its orbit
(desktop GUIs `gooey-pi` 847★, `pi-desktop` 71★; sandboxing `omp-sbx`; Nix flake `omp-nix`;
orchestrators `oh-my-singularity`, `omp-best-of` 63★; web cockpits `omp-deck`, `omp-web`;
config packs `omp-config` 44★, `omp-agent` 38★; an SDK `omp-workshop-sdk`; forks like `veyyon`).
That third-party orbit is the strongest single signal that the extension contract is real.

---

## 9. What is GOOD, and what is BAD

### Genuinely good

1. **One capability registry, N providers, priority + first-wins dedup.** Every extensible thing —
   skills, rules, hooks, tools, commands, prompts, MCP servers, context files, settings, system
   prompts, SSH hosts, extension modules — goes through `defineCapability<T>()` with a `key`
   function and a `toExtensionId` function (`packages/coding-agent/src/capability/*.ts`, 14
   capabilities). One mechanism produces: discovery, dedup, precedence, *and* the unified
   `disabledExtensions` deny-list, for free, for every new capability. This is the single best
   idea in the repo.

2. **Adopting a competitor's config formats as an install strategy.** Reading `.claude/`,
   `.claude-plugin/marketplace.json`, `.codex/`, `.gemini/`, `.cursor/`, `.windsurf/`,
   `.opencode/`, `.github/skills/` means a new user's existing investment works on day one and the
   plugin ecosystem is non-empty at launch. 8,907 LOC is a lot to spend on this; it is clearly
   deliberate and clearly the right call.

3. **Implementing a real portable standard (Agent Plugins 1.0.0) with a *closed* schema.** Closed
   schemas — reject unknown fields fatally, warn on unknown top-level keys — are how you keep a
   plugin format from rotting into "whatever the biggest vendor emits". Contrast with the legacy
   `package.json#omp` path where the docs admit "There is no strict schema validation in
   manager/loader" (`plugin-manager-installer-plumbing.md:95`).

4. **Transactional install with a smoke-import.** Snapshot package tree + `package.json` +
   `bun.lock`, then import every declared extension entry and initialize it against a throwaway
   registration surface before committing. Rollback on any failure. Most plugin managers discover
   a broken plugin at the *user's* next startup.

5. **Separating "what is installed" from "how it is configured" from "is it on".**
   `plugins/package.json` (deps) / `omp-plugins.lock.json` (`{version, enabledFeatures, enabled}` +
   a *separate* `settings` map) / `plugin-overrides.json` (project-level, read-only). Upgrades
   can't eat your settings; a repo can disable a plugin for its contributors without touching
   their global state.

6. **Two-phase extension lifecycle enforced by throwing.** `pi.sendMessage()` during load throws
   `ExtensionRuntimeNotInitializedError` rather than half-working. Errors are per-path and
   non-fatal to the whole load.

7. **Typed plugin settings schemas with `secret: true` and `env:` fallback.** A plugin declares
   `{type: "string", secret: true, env: "MY_KEY"}` and gets masking in UI/logs and env-var
   resolution without writing any code.

8. **Config quarantine over silent repair.** Broken YAML is moved to
   `.broken-<ts>-<pid>-<uuid>` under a file lock and startup *fails* with both paths named.

9. **The installer refuses to lie.** Arch mismatch detection, post-install `--version` smoke test,
   musl-specific remediation text, no green checkmark for a binary that can't start.

10. **Update protocol designed for updaters already deployed.** `omp.rename` + `omp.dist` escape
    hatch means today's shipped updater can be steered by tomorrow's release metadata.

11. **Documentation as an artifact.** 130 design docs that cite specific file paths and line-level
    behavior including known warts ("Despite its name, current `notify` mode writes update
    availability only to the debug log"). This is what made this teardown possible in an afternoon.

### Genuinely bad

1. **No project-trust gate — a real security regression from upstream.** Verified:
   `grep -rn "trust.json\|projectTrust\|project_trust" packages/coding-agent/src` → **zero hits**.
   Upstream Pi has an explicit trust model (`~/.pi/agent/trust.json`, `defaultProjectTrust:
   ask|always|never`, a `project_trust` event, and a documented split where "Project-local
   extensions, project package-managed extensions, and project settings are loaded only after the
   project is trusted" — `pi-mono/packages/coding-agent/README.md:298-308`). oh-my-pi dropped it.
   So: `git clone` a hostile repo, `cd` into it, run `omp`, and `<repo>/.omp/extensions/*.ts` is
   auto-discovered and executed **unsandboxed, in-process, before you type anything**. The docs
   even say so plainly: "Extensions are **not sandboxed** (same process/runtime)"
   (`docs/extension-loading.md:255`). `--no-extensions` and `--trusted-extension` exist, but the
   default is unsafe and the safe path is opt-in. This is the one thing in the design I would not
   copy under any circumstances.

2. **Two plugin-manager implementations, one of them documented but dead.** `manager.ts` is live;
   `installer.ts` "still documents important safety checks and filesystem behavior, but it is not
   the path used by `src/commands/plugin.ts`" (`plugin-manager-installer-plumbing.md:14`). A
   documented-but-unused safety layer is worse than none.

3. **Precedence overrides are invisible.** Skills, rules, and agents dedup silently by name with
   first-wins across 8 providers. `docs/skills.md` mentions collision *warnings*, but there's no
   `omp doctor`-style "these 6 items are being shadowed" report at the point a user is confused.
   A user who drops `~/.omp/agent/skills/pdf/` and forgets will spend an hour debugging why a
   marketplace plugin's `pdf` skill stopped working.

4. **`marketplace.autoUpdate: notify` is a lie by default.** The default mode writes to the debug
   log and shows nothing.

5. **No npm-plugin update path.** `omp plugin install pkg@newVersion` *is* the update. No
   `check-updates`, no migration hook, no changelog surfacing.

6. **Inconsistent extension-suffix rules by source.** Native/configured-directory auto-scan takes
   `.ts`/`.js` only; installed-plugin manifests additionally take `.mjs`/`.cjs`. Native
   auto-discovery applies `gitignore: true, hidden: false`; explicit configured-directory scanning
   does not apply gitignore at all (`docs/extension-loading.md:238-243`). Two scanners, two rule
   sets, one concept.

7. **1,261 open issues against 408 contributors and 583 releases.** Shipping 5 releases in 5 days
   at v18.1.0 with a four-digit issue backlog is velocity purchased with debt. The repo is 8 months
   old and has already burned 18 major versions.

8. **Uninstall is unowned.** ~30 directories and 8 SQLite databases under `~/.omp`, a managed
   Python venv, a Puppeteer sandbox, a browser-relay extension install, git worktrees under
   `~/.omp/wt`, an encrypted auth snapshot — and no command to remove any of it. `omp config
   init-xdg` explicitly "does not move existing data", so a user who adopts XDG mid-life ends up
   with two live state roots.

9. **The `.pi` → `.omp` legacy compat surface is load-bearing and messy.** A Bun `onLoad` hook
   rewrites `@mariozechner/*` and `@earendil-works/*` specifiers onto host-bundled copies at import
   time, plus shim modules for moved catalog symbols
   (`src/extensibility/legacy-pi-ai-shim.ts`, `legacy-pi-coding-agent-shim.ts`). It works, and it
   is exactly the kind of thing that will be load-bearing forever.

---

## 10. What transfers to a compiled Rust binary host (Muse Code)

### Transfers directly — copy these

1. **The capability-provider registry.** `defineCapability<T>{ key, toExtensionId }` + N providers
   with integer priorities + first-wins dedup is pure data-structure design. In Rust it is a
   `trait CapabilityProvider<T>` with `priority() -> u8` and a `BTreeMap` merge. Muse already has
   skills, rules, workflows, agent-definitions and settings subsystems in its config crate — this
   is the missing unifying abstraction, and adopting it gives `disabled_extensions` for every
   subsystem for free.

2. **The `SOURCE_PATHS` table.** A single const table mapping vendor → `{user_base, user_agent,
   project_dir}` is *more* natural in Rust than in TS. Muse's loader already recognises
   `.claude-plugin` and `.codex-plugin`; formalize it into this table and add `.gemini`, `.cursor`,
   `.opencode`, `.github/skills`, `.windsurf` for near-zero marginal cost per vendor.

3. **Agent Plugins 1.0.0 (`agent-plugins.org`) with a closed manifest schema.** `.muse-plugin/plugin.json`
   should *be* an Agent Plugins manifest with a `muse` namespace under `extensions`, not a
   Muse-specific invention. Closed-schema validation — reject unknown fields fatally, warn on
   unknown top-level keys — is straightforward with `serde(deny_unknown_fields)`. This is the single
   highest-leverage borrowing: it makes oh-my-musecode packages portable to Claude Code, Codex,
   *and* omp without a shim.

4. **The three-file state split.** deps-manifest / runtime-lock (`{version, enabled_features,
   enabled}` + a *separate* settings map) / project overrides. Muse already has `.muse/lock.json`
   with provenance/quarantine/allowed_tools; the lesson is to keep **user settings in a different
   key from install state** so upgrade can replace one without touching the other, and to add a
   read-only project-level `plugin-overrides.json` equivalent.

5. **Unified `disabled_extensions` with capability-qualified ids.** `skill:pdf`,
   `context-file:user:CLAUDE.md`, `extension-module:foo`. One deny-list, every subsystem, one
   setting a user has to learn.

6. **Transactional install with rollback + pre-commit validation.** Snapshot before mutate, restore
   on any failure. In Rust this is *easier* (a temp dir + atomic rename). The smoke-import step has
   a native analogue: for declarative assets, parse and schema-validate every shipped file before
   committing the install.

7. **Config quarantine.** Move broken config to `.broken-<ts>-<pid>-<uuid>` under a file lock and
   fail loudly with both paths. Trivial in Rust; Muse's `lock.json` already has a quarantine concept
   to hang this on.

8. **Update protocol with a forward-rename pointer and a `dist` escape hatch,** plus SHA-256 +
   size verification of every downloaded asset. Muse ships a single binary — this is *more*
   applicable, not less. Also: self-update must detect and refuse to mutate a package-manager-owned
   install (brew, nix, mise).

9. **The installer's honesty discipline.** Arch detection that doesn't trust `uname -m` under
   Rosetta; a post-install `--version` smoke test; no success message for a binary that can't run;
   no shell-rc edits.

10. **Docs-that-cite-line-behavior as a deliverable.** 130 docs naming implementation files and
    admitting warts. Directly copyable working practice.

11. **Skill/rule/prompt content as markdown-with-frontmatter, discovered non-recursively at
    `<root>/skills/<name>/SKILL.md`,** addressed by an internal URL scheme (`skill://name/path`)
    with containment guards (reject absolute paths, reject `..`, reject any resolved path escaping
    the base). This is format-only and host-language-independent — and the containment guards are
    exactly what a Rust `Path::canonicalize` + `starts_with` check does natively.

12. **The rule shape.** `{name, globs, alwaysApply, condition, astCondition, scope,
    interruptMode}` with `scope: "tool:edit(*.ts)"` — glob- and tool-scoped guidance that fires at
    the moment of an edit rather than bloating the system prompt. 27 built-in rules is a small,
    high-value content library, and it is exactly the kind of thing oh-my-musecode should *ship*
    where oh-my-pi merely *enables*.

### Does NOT transfer

1. **The entire TypeScript extension-module runtime.** `ExtensionAPI`, `pi.on()`, `registerTool`,
   `registerMessageRenderer`, `registerProvider`, dynamic `import()` with `?mtime` cache-busting,
   the Bun `onLoad` specifier-rewriting hook, the graph-wide re-import for hot reload, the
   Zod/arktype/TypeBox schema-builder injection — all of it presupposes an in-process JS runtime
   that a compiled Rust binary does not have. **This is the single biggest architectural fork in
   the road for oh-my-musecode.**
   The Rust-host substitutes, in ascending order of power:
   - **declarative** (what Muse already has): hooks.json, skills, rules, workflows, settings — no
     code, no runtime;
   - **subprocess hooks** (Claude Code's model): a hook is a command line; the host passes JSON on
     stdin and reads JSON on stdout. Cross-language, sandboxable, no embedded runtime. This should
     be oh-my-musecode's default extension mechanism.
   - **MSP / `muse serve`**: Muse's stdio session protocol is the real analogue of omp's in-process
     extension API. A long-lived out-of-process extension speaking MSP gets event subscription and
     tool registration *with* a process boundary — strictly better isolation than omp has, at the
     cost of latency and a serialization contract.
   - embedding a scripting runtime (rhai/wasm) only if a real need survives the above.

2. **`bun install` as the package manager.** omp shells out to bun for npm/git installs and
   symlinks into `node_modules/`. A Rust host has no reason to inherit npm. The transferable
   *shape* is: a content-addressed cache (`cache/plugins/<marketplace>___<plugin>___<version>/`)
   plus symlinks/junctions from a scope root — implement that natively over git and tarballs.
   Note omp's own marketplace already **rejects npm plugin sources**, so its git/`git-subdir`/
   `url`+`sha` source model is the part worth copying, and it is runtime-neutral.

3. **`package.json#omp` as the manifest.** Only makes sense in an npm world. Use the Agent Plugins
   `plugin.json` for portable content, and a Muse-namespaced key inside it for anything native.

4. **Hot-reload by re-import.** The `?mtime` cache-buster trick has no Rust analogue for compiled
   code. Declarative assets can and should be watch-reloaded (`/reload-plugins` equivalent); code
   extensions in a Rust host reload by restarting the subprocess.

5. **Nothing here is shell-specific** — that is the good news relative to actual oh-my-zsh. Neither
   oh-my-pi nor Pi sources shell code; both are "a binary reads declarative config + markdown from
   a precedence-ordered set of directories". That core is exactly the model Muse Code already has,
   and it is why this teardown is worth more to us than an oh-my-zsh teardown would be.

### The strategic read for oh-my-musecode

oh-my-pi is the wrong *name-analogue* and the right *architecture-analogue*. It is not a config
pack; it is proof that (a) a capability-provider registry with priority dedup scales to 14
capabilities and 11 vendor formats, (b) ingesting Claude Code's plugin format verbatim is a viable
cold-start strategy worth ~9k LOC, and (c) the Agent Plugins 1.0.0 standard is real and already
implemented by a 29k-star project.

But note what oh-my-pi does *not* do: it ships almost no content (0 user-facing skills, 27 rules,
7 subagents). The oh-my-* slot for Pi — a curated content library with an update path — is still
occupied only by 149-star `monopi`. If oh-my-musecode wants to be the oh-my-zsh of Muse Code, the
distribution machinery should be borrowed from oh-my-pi and the *content* is the part nobody in
this ecosystem has built yet.

---

## Verification

**Verdict: MOSTLY_SOLID** — verified independently on 2026-09-01 by an adversarial second pass.
Working dir: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-oh-my-pi/`
(fresh `git clone --depth 1` of `can1357/oh-my-pi` + `ifiokjr/monopi`, plus `gh api` and the npm
registry API). Not a single load-bearing claim was refuted. The only defects are three
slightly-off file counts and a set of stale line-number anchors in doc citations.

### 1. Repo existence and traction — CONFIRMED, exact

`gh api repos/can1357/oh-my-pi` returns a real, non-fork, unarchived MIT repo,
description "⌥ Coding agent with the IDE wired in". Every number in the report reproduced:

| Claim | Independent measurement | |
|---|---|---|
| stars 28,857 | 28,859 | ✓ (drift of 2 in ~1h) |
| forks 2,884 | 2,886 | ✓ |
| watchers 90 | 90 | ✓ |
| open issues+PRs 2,042 | 2,044 | ✓ |
| open issues only 1,261 | 1,261 (search API) | ✓ exact |
| closed issues 3,386 | 3,386 | ✓ exact |
| merged PRs 3,129 | 3,129 | ✓ exact |
| contributors 408 | `Link rel=last` → page=408 | ✓ exact |
| releases 583 | 583 (`--paginate`) | ✓ exact |
| latest v18.1.0 @ 2026-09-01T14:00Z | identical | ✓ exact |
| created 2025-12-31, pushed 2026-09-01T13:33:21Z | identical | ✓ exact |
| size 578,969 KB | 578,969 | ✓ exact |
| npm last-week 113,949 / last-month 447,565 | identical | ✓ exact |
| npm 605 published revisions | 605 versions, dist-tag latest 18.1.0 | ✓ exact |

Disambiguation confirmed: `gh api repos/badlogic/pi-mono` resolves to `earendil-works/pi`
(100,392★ / 12,470 forks / MIT / TypeScript). The fork claim is in the README verbatim —
`README.md:22` "Fork of Pi by @mariozechner", restated at `:566` and `:582`.
`ifiokjr/monopi` is real at 149★ with the literal description "Like oh-my-zsh for pi", and
`find monopi -name SKILL.md` returns exactly **3**. Third-party orbit spot-checked:
`am-will/gooey-pi` 847★, `wolfiesch/omp-best-of` 63★, `sakuradairong/omp-config` 44★,
`bparlan/omp-agent` 38★, `apoc/omp-desktop` 38★ — all exact.

### 2. Shipped-content counts — CONFIRMED, near-perfect

Re-ran every `find`/`wc` from scratch against the clone:

- **29 built-in tools + 3 hidden** — `builtin-names.ts` `BUILTIN_TOOL_NAMES` has exactly 29
  entries; `HIDDEN_TOOL_NAMES = ["yield","goal","think"]`. ✓ exact
- **77 top-level slash commands, 17+10+19+24+3+4** — counting top-level `name:` entries in each
  of the six spec arrays gives modes 17, collaboration 10, session 19, lifecycle 24,
  marketplace 3, control 4 = **77**. Every sub-figure exact. ✓
- **7 subagents + 8 md files** — `src/task/agents.ts` names scout, designer, reviewer,
  security-reviewer, librarian, task, sonic; `src/prompts/agents/` holds 8 `.md`
  (the 7 + `frontmatter.md` + `init.md`, minus one). ✓
- **27 built-in rules, 8 Go / 6 Rust / 13 TS** — 27 files, split 8/6/13. ✓ exact
- **176 prompt markdown files** — 176. ✓ exact
- **98 TUI themes** — 98 JSON. ✓ exact
- **66 providers / 4,705 models** — `json.load` on `packages/catalog/src/models.json`:
  66 keys, 4,705 nested model entries. ✓ exact
- **54 LSP presets / 14 DAP adapters** — 54 and 14. ✓ exact
- **0 bundled user-facing skills** — `find packages/*/src -name SKILL.md` → **0**; repo-wide
  15 = 3 dogfood in `./.omp/skills/` + 12 test fixtures. ✓ exact, and the strongest single
  finding in the teardown.
- **Repo-local `.omp/` dogfood = 14 files** — `find .omp -type f` returns exactly 14:
  5 commands, 3 SKILL.md (+2 supporting files), 3 files under `tools/`. ✓ exact
- **222,773 Rust LOC in `crates/`** — 222,773. ✓ **exact to the line**
- **472,811 TS LOC in `packages/coding-agent/src`** — 472,811. ✓ **exact to the line**
- **6,327-line settings schema** — `wc -l src/config/settings-schema.ts` → 6,327. ✓ exact
- **402 Rust files** — 402. ✓ exact
- **130 internal design docs** — `find docs -name '*.md'` → 130. ✓ exact
- **17 workspace packages / 9 Rust crates** — `packages/*` = 17 dirs; `Cargo.toml` members = 9
  (8 first-party `pi-*` + vendored `brush-core`). ✓
- **8,907 LOC in `src/discovery/`** — 8,907 at maxdepth 1. ✓ **exact**, and the per-vendor
  breakdown is exact for all ten: claude 592, claude-plugins 675, codex 553, opencode 538,
  gemini 386, github 337, cursor 223, windsurf 149, vscode 106, cline 83.

**Corrections (immaterial):**
- "99 tool implementation files" is wrong. Actual: **94** at `src/tools/*.ts`, or **131**
  recursively including `browser/`, `puppeteer/`, `hub/`, `computer/`, `eval-format/`.
  99 matches neither.
- "4,667 TS files" is off by ~8. Actual: 4,659 `.ts` under `packages/`, 4,734 repo-wide,
  4,850 including `.tsx`.
- "~84 distinct `name:` literals incl. nested subcommands" could not be reproduced
  (145 unique across the six files). The load-bearing 77 is exact, so this is noise.

### 3. Extension contract — CONFIRMED in code, not aspiration

This was the highest-risk section and it survives intact. Every mechanism was traced to source.

- **Capability registry.** `src/capability/index.ts:52` defines `defineCapability<T>`;
  `registerProvider` inserts in priority order (`providers.findIndex(p => p.priority < provider.priority)`).
  Exactly **14** capabilities call it: context-file, extension, extension-module, hook,
  instruction, mcp, prompt, rule, settings, skill, slash-command, ssh, system-prompt, tool.
  ✓ matches the claimed list item-for-item.
- **`toExtensionId` + unified deny-list.** Defined on 10 of the 14. The three ids the report
  quotes are literal source: `extension-module:${ext.name}`, `skill:${skill.name}`,
  and `context-file:${file.level}:${path.basename(file.path)}` — so
  `context-file:user:CLAUDE.md` is exactly right. `capability/index.ts:112-155` reads
  `settings.get("disabledExtensions")` into a Set and skips any item whose `toExtensionId`
  is present, before filtering and before dedup. ✓
- **Provider priorities.** Every constant checked: `builtin.ts:42` = 100, `omp-plugins.ts:47` = 90,
  `claude.ts:35` = 80, `agent-plugins.ts:41` = 75, `claude-plugins.ts:34` = 70 (with the
  comment "Below claude.ts (80) so user .claude/ overrides win"), `agents.ts:29` = 70,
  `codex.ts:43` = 70, `opencode.ts:45` = 55, `github.ts:42` = 30, `builtin.ts:313`
  `MANAGED_SKILLS_PRIORITY` = 5. ✓ **every number the report cited is exact.**
  (The report omitted gemini 60, cursor/windsurf 50, cline 40, vscode 20, builtin-defaults 1 —
  an omission, not an error.)
- **`SOURCE_PATHS`.** At `discovery/helpers.ts:31-84` exactly as cited, with all ten vendors and
  the exact `{userBase, userAgent, projectDir}` mappings claimed — including the two non-obvious
  ones (`opencode` → `.config/opencode` user / `.opencode` project; `windsurf` →
  `.codeium/windsurf` user / `.windsurf` project) and the two nulls (`cline.projectDir: null`,
  `github.userBase: null`). ✓ exact
- **Drop-a-file discovery.** `builtin.ts` scans all nine claimed subdirs
  (commands, skills, hooks, tools, rules, prompts, agents, extensions, instructions) plus
  settings.json, config.yml, mcp.json, SYSTEM.md, RULES.md, AGENTS.md, over
  `getConfigDirs()` = `{cwd}/.omp` (project) + `getAgentDir()` (profile-scoped user). ✓
- **TS extension module API.** `docs/extensions.md` is **747 lines** — exact. `pi.on()` has 45
  overloads across **41 unique typed events** (report said "30+" — conservative and correct).
  All eight claimed `register*` methods exist verbatim on `ExtensionAPI`: registerTool, Command,
  Shortcut, Flag, MessageRenderer, ComposerShape, Provider, FileWriteFallback (plus
  registerFileDeleteFallback and registerAssistantThinkingRenderer). `arktype` and `TypeBox` are
  injected at `loader.ts:156-157`. `sendMessage` takes
  `{ triggerTurn?: boolean; deliverAs?: "steer" | "followUp" | "nextTurn" }` — the claimed four
  delivery semantics. `ExtensionRuntimeNotInitializedError` is a real class at `loader.ts:65`,
  thrown at `:89` and `:93`. ✓
- **Hook rewrite semantics — verified verbatim.** `shared-events.ts`:
  `ToolCallEventResult { block?: boolean; reason?: string; input?: Record<string, unknown> }`
  with the doc comment "Replacement input the tool executes with… Ignored when `block` is true";
  `ToolResultEventResult { content?; details?; isError? }`. `session.compacting` carries a
  `SessionCompactingResult`. ✓ The report's "`{block, reason}` OR rewrite input" is precisely right.
- **Agent Plugins 1.0.0, closed schema.** `discovery/agent-plugin-format.ts` header:
  "Agent Plugins 1.0.0 format support (https://agent-plugins.org)… the closed `plugin.json`
  manifest (spec §5)"; pins
  `https://agent-plugins.org/schemas/1.0.0/plugin.schema.json`, has an `invalid` state for
  "targets Agent Plugins but violates the closed schema", and a closed frontmatter field set. ✓
- **`plugin-overrides.json`.** Real: `plugins/types.ts:153`, `plugins/loader.ts:61`
  (`getConfigDirPaths("plugin-overrides.json", { user: false, cwd })` — project-only, as claimed),
  documented read-only at `plugin-manager-installer-plumbing.md:56`. ✓
- **Dead installer.ts.** `plugin-manager-installer-plumbing.md:14` verbatim: "installer.ts still
  documents important safety checks and filesystem behavior, but it is not the path used by
  `src/commands/plugin.ts`". ✓
- **Transactional install with rollback.** `plugins/manager.ts:325` `#snapshotInstalledPackage`,
  `:345` `#cleanupSnapshot`, `:356` `#rollbackFailedInstall`. ✓
- **Config quarantine.** `config/settings.ts:1730`:
  `` `${filePath}.broken-${Date.now()}-${process.pid}-${randomUUID()}` `` — the claimed
  `.broken-<ts>-<pid>-<uuid>` shape, exactly. ✓

**The security weakness is CONFIRMED, and it is the most important verified finding.**
`grep -rnE 'trust\.json|projectTrust|project_trust' packages/coding-agent/src` returns **0 hits**.
`grep -i trust` on `settings-schema.ts` returns **0 hits**. The only "trust" in the codebase is
`--trusted-extension` (an explicit CLI allowlist, `main.ts:1346-1363`) and `--no-extensions`
(`main.ts:1371`), i.e. opt-in. Meanwhile `builtin.ts:480-492` discovers
`<cwd>/.omp/extensions/` unconditionally via `discoverExtensionModulePaths` with no gate of any
kind, and `docs/extension-loading.md:251` states "Extensions are **not sandboxed**
(same process/runtime)". The "clone a hostile repo, `cd`, run `omp`" attack described in the
report is real as written. This claim was the one most likely to be an over-read; it is not.

### 4. Install / update — CONFIRMED, faithful

- `scripts/install.sh` is **334 lines** — exact. Read end to end.
- `INSTALL_DIR="${PI_INSTALL_DIR:-$HOME/.local/bin}"` at `:15`; `BINARY="omp-${PLATFORM}-${ARCH}"`
  at `:241`. ✓
- `host_arch()` uses `sysctl -in hw.optional.arm64` on Darwin with the comment "so it stays
  correct inside a Rosetta session, where `uname -m` reports the translated x86_64". ✓ exactly
  as described.
- `bun_arch_matches_host()` exists and returns 0 (match) when bun's arch can't be read. ✓
- Post-install smoke test verified verbatim, including the source comment **"Never claim success
  for a binary that cannot run"**, the `exit 1` on failure, and the musl-conditional
  `apk add libstdc++ libgcc` remedy. ✓
- **No shell-rc edits**: `grep -nE 'bashrc|zshrc|profile|\.config/fish' scripts/install.sh`
  returns nothing. ✓ The "writes exactly one file" claim holds.
- All six install paths are in the README at `:40` (curl), `:48` (brew), `:54` (bun),
  `:61`/`:64` (nix), `:85` (PowerShell), `:91` (mise). ✓
- `cli/update-cli.ts` is **2,073 lines** — exact. `commands/update.ts` is a 43-line shim. ✓
- Flags `--check/-c`, `--force/-f`, `--plugins/-l`, `--canary`, `--stable` all parsed at `:380-387`. ✓
- **`omp.dist` / `omp.rename` are real.** `:114-127` parses `omp.dist` and — exactly as claimed —
  maps any non-`"npm"` value to `"binary"`: `return dist === "npm" ? "npm" : "binary";`.
  `:131-154` parses the `omp.rename` `{package, natives}` forward pointer, with the doc comment
  showing the paired `"dist": "binary"` escape hatch. ✓
- **Asset verification is real.** `:221-240` rejects non-integer/non-positive `size`, a missing
  digest, and anything not matching `/^sha256:([0-9a-f]{64})$/i`; `:292-340` streams into a
  `createHash("sha256")` and throws on digest mismatch, printing `Verified sha256:…`.
  (Report cited `:183-346`; the checks sit inside that span.) ✓
- **No npm/git plugin update action** — `plugin-manager-installer-plumbing.md:41`: "No explicit
  npm-plugin `update` action exists; update is done by re-running `install` with a new
  package/version spec." ✓ (report cited `:60`; content exact)
- **`notify` is a no-op** — `docs/marketplace.md:218`: "Despite its name, current `notify` mode
  writes update availability only to the debug log; it does not show a user-facing notification."
  ✓ (report cited `:236`)
- **Marketplace / Claude Code compat** — `marketplace/fetcher.ts:200`:
  `CATALOG_RELATIVE_PATHS = [".omp-plugin/marketplace.json", ".claude-plugin/marketplace.json"]`,
  in that preference order. `docs/marketplace.md:96` recommends publishing both. The example
  `$schema` at `docs/marketplace.md:100` is literally
  `"https://anthropic.com/claude-code/marketplace.schema.json"`. ✓ The "deliberate free-ride on a
  competitor's ecosystem" reading is accurate.
- **npm sources rejected** — `marketplace/source-resolver.ts:139` throws the exact string
  "npm plugin sources are not yet supported. Use git-based sources instead." ✓

### 5. Config footprint / uninstall — CONFIRMED

`ls src/commands/*.ts` → **42 files**, no `uninstall.ts`. `grep -c uninstall README.md` → **0**.
The only `uninstall` in `src/commands/` is `plugin.ts` (plugin-scoped). `gc.ts` exists and is a
sweeper, not an uninstaller. `omp config init-xdg` is a real action
(`cli/config-cli.ts:30`, `VALID_ACTIONS` at `:81`) and `docs/settings.md:68` confirms verbatim
"It does not move existing files or set the XDG environment variables." All eight named SQLite
databases resolve to real source references (agent.db, history.db, models.db, autoqa.db,
stats.db, github-cache.db, commit-inference.db, legacy-pi-extension-cache.db), as do
`python-env`, `auth-broker-snapshot.enc`, `browser-relay` and `secret-placeholder.key`. ✓

### 6. Defects found

Nothing refuted. Six corrections, all cosmetic:

1. **"99 tool implementation files"** → actual **94** (`src/tools/*.ts`) or **131** (recursive).
2. **"4,667 TS files"** → actual **4,659** under `packages/` (4,734 repo-wide `.ts`).
3. **"~84 distinct `name:` literals"** → not reproducible (145 unique); the exact 77 stands.
4. **Stale doc line anchors** — content correct everywhere, anchors drifted by 10-40 lines
   because the repo was pushed the same day the teardown was written:
   `extension-loading.md:255` → **:251**; `marketplace.md:236` → **:218**;
   `marketplace.md:60-70` → **:96**; `plugin-manager-installer-plumbing.md:60` → **:41/:119**;
   `install.sh:97` → **:99**; `install.sh:270-290` → **:276-291**.
5. **Release cadence understated.** "5 releases in 5 days" is true but there were actually
   **8 releases in 8 days** (v18.0.5 Aug 25 → v18.1.0 Sep 1). Errs conservative.
6. **`pi-desktop 71★`** → the repo I find by that name (`DLYZZT/pi-desktop`) is at **173★**;
   possibly a different repo or growth since measurement. Every other orbit number was exact.

### 7. Bottom line

This teardown was written from the source, not from the README. Three independent LOC counts land
**exactly to the line** (222,773 / 472,811 / 6,327), the discovery-directory total lands exactly
(8,907), all ten per-vendor ingestion LOC figures land exactly, every provider priority constant
is right, and the two claims most likely to be hallucinated — "0 bundled user-facing skills" and
"no project-trust gate" — are both verified true against the code. The strategic read for
oh-my-musecode (borrow the capability-provider registry, the `SOURCE_PATHS` table, Agent Plugins
1.0.0 with `serde(deny_unknown_fields)`, the three-file state split, and the installer's honesty
discipline; do **not** copy the missing trust gate; do **not** try to port the in-process TS
extension runtime to a Rust binary) rests on claims that all hold up. Use it.

*Verified 2026-09-01 against `can1357/oh-my-pi` @ v18.1.0, pushed 2026-09-01T13:33:21Z.*
