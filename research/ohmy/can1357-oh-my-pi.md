# Teardown: can1357/oh-my-pi (`omp`)

Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/can1357-oh-my-pi/`
Read at: commit `d74c5833` ("chore: bump version to 18.1.0"), 2026-09-01.

## 1. Existence — CONFIRMED

`https://github.com/can1357/oh-my-pi` → HTTP 200. GitHub API `repos/can1357/oh-my-pi` returns
`id: 1125856365`, description "⌥ Coding agent with the IDE wired in", homepage `https://omp.sh`.
Cloned successfully, 261 MB, 6943 tracked files.

**Important framing correction.** Despite the name, this is *not* an "oh-my-zsh for an agent".
It is a **complete, from-scratch coding agent** — Meta-Muse's peer, not its config layer. The
`oh-my-` prefix is branding, not architecture. The lesson for oh-my-musecode is therefore
*not* "copy their layout" but **"copy the extensibility substrate that let 197 third parties
build on them in eight months"**.

## 2. What it ships (real counts)

Monorepo: 18 TS packages under `packages/`, 9 Rust crates under `crates/`.
File census via `git ls-files`: 4734 `.ts`, 541 `.md`, 402 `.rs`, 218 `.json`, 116 `.tsx`, 107 `.kdl`.

| Category | Count | Path evidence |
|---|---|---|
| Built-in LLM tools | 29 public + 3 hidden | `packages/coding-agent/src/tools/builtin-names.ts:1-37` (`BUILTIN_TOOL_NAMES`, `HIDDEN_TOOL_NAMES`) |
| Tool implementation files | 152 | `packages/coding-agent/src/tools/` |
| Built-in subagents (prompts) | 8 | `packages/coding-agent/src/prompts/agents/` — designer, librarian, reviewer, scout, security-reviewer, task, init, frontmatter |
| Prompt markdown assets | 176 | `packages/coding-agent/src/prompts/**/*.md` (78 in `prompts/system/`, 53 in `prompts/tools/`) |
| Built-in lint/rule packs | 27 `.md` | `packages/coding-agent/src/discovery/builtin-rules/` — 8 Go, 6 Rust, 13 TS |
| TUI themes | 98 JSON | `packages/coding-agent/src/modes/theme/defaults/*.json` |
| Personalities (system-prompt styles) | 3 | `prompts/system/personalities/{default,friendly,pragmatic}.md` |
| Bundled slash-command markdown | 2 | `extensibility/custom-commands/bundled/` (ci-green, review) |
| Built-in (code) slash commands | ~181 name literals | `src/slash-commands/builtin-*.ts` (9 registry files) |
| Settings schema entries | 495 typed keys | `src/config/settings-schema.ts` (6327 lines) |
| Model-compat rule files | 107 `.kdl` | `packages/catalog/src/compat/rules/` (64 provider, 23 taxonomy, 19 class) |
| Docs | 81 `.md` | `docs/` |
| **Capabilities** | **14** | `src/capability/*.ts` — see §4 |
| **Discovery providers** | **82 registrations across 19 modules** | `src/discovery/*.ts` |

It also dogfoods its own extension surface: `.omp/` in the repo root holds 5 project slash
commands (`.omp/commands/*.md`) and 3 skills (`.omp/skills/*/SKILL.md`).

## 3. INSTALL — what actually lands on disk

`scripts/install.sh` (334 lines) + `scripts/install.ps1` (309 lines), served from `https://omp.sh/install`.

The installer is **deliberately tiny in footprint**. It writes exactly one thing:

- `${PI_INSTALL_DIR:-$HOME/.local/bin}/omp` — a single prebuilt binary, `chmod +x`.

Or, in `--source` mode, it shells out to `bun install -g @oh-my-pi/pi-coding-agent`
(footprint owned by bun). It will install bun itself (`curl https://bun.sh/install | bash`)
if missing, and enforces `MIN_BUN_VERSION=1.3.14`.

**It writes no config, no dotfiles, no shell rc lines, no `~/.omp`.** Everything under
`~/.omp` is created lazily by the running agent. This is the single biggest structural
difference from an oh-my-zsh-style framework and, in my view, the correct choice.

Two details worth stealing:

1. **Rosetta-aware arch detection** (`host_arch()`, lines 74-104). On Darwin it reads
   `sysctl hw.optional.arm64` rather than `uname -m`, because `uname -m` reports the
   *translated* `x86_64` inside a Rosetta shell. It then compares against `bun_arch()` and
   **refuses to build from source on a mismatch**, falling back to the prebuilt native binary.
   Most installers silently produce a slow x86_64 build here.
2. **Post-install smoke test** (lines 279-300). After download it runs `omp --version` and
   fails loudly if the binary can't start — with a musl-specific remedy
   (`apk add libstdc++ libgcc`) because Bun's musl target links libstdc++/libgcc dynamically.
   The comment is explicit: *"Never claim success for a binary that cannot run."*

Other install channels (README.md:35-65): Homebrew (`can1357/tap/omp`), bun global, Nix flake
(`nix run github:can1357/oh-my-pi`), Docker.

### Runtime config footprint (created on use, not on install)

From `packages/utils/src/dirs.ts` (1114 lines) and `docs/marketplace.md`:

```
~/.omp/
  agent/config.yml            canonical settings (YAML)
  agent/settings.json         legacy, migrated once → renamed .bak
  agent/agent.db              auth store + state
  agent/{skills,commands,rules,prompts,extensions,instructions,hooks/<type>,tools}/
  agent/managed-skills/       agent-authored skills (see §9)
  agent/keybindings.*
  agent/models.yml
  profiles/<name>/agent/...   full per-profile relocation
  plugins/installed_plugins.json
  plugins/omp-plugins.lock.json
  plugins/package.json
  plugins/node_modules/<pkg>  symlinks into the cache
  plugins/cache/marketplaces/<name>/
  plugins/cache/plugins/<marketplace>___<plugin>___<version>/
  marketplaces.json
  cache/{github-cache.db,commit-inference.db,avatars,fastembed,...}
  logs/, reports/, worktrees/, remote/, security/, autoresearch/
<project>/.omp/
  config.yml, settings.json, mcp.json|.mcp.json
  skills/ commands/ rules/ prompts/ extensions/ instructions/ hooks/ tools/
  plugins/{installed_plugins.json,omp-plugins.lock.json,node_modules/}
```

XDG is opt-in via `omp config init-xdg` (creates roots, does **not** migrate existing data).
`PI_CONFIG_DIR`, `PI_CODING_AGENT_DIR`, `OMP_PROFILE` relocate bases.

## 4. EXTENSION CONTRACT — the actually important part

There is no `custom/` directory and no "enabled modules" list. Instead there is a
**capability × provider matrix with numeric priority and first-wins dedup**. This is the
design that made a second-order ecosystem possible, and it is the part worth stealing wholesale.

### 4a. The core abstraction

`packages/coding-agent/src/capability/types.ts:1-9` states the thesis outright:

> *"This architecture inverts control: instead of callers knowing about paths like `.claude`,
> `.codex`, `.gemini`, they simply ask for `load("mcps")` and get back a unified array."*

14 capabilities are declared with `defineCapability<T>()`:

| capability | file | dedup key | disable id (`toExtensionId`) |
|---|---|---|---|
| skills | `capability/skill.ts:58` | `skill.name` | `skill:<name>` |
| slash-commands | `capability/slash-command.ts:48` | `cmd.name` | `slash-command:<name>` |
| mcps | `capability/mcp.ts:95` | server name | yes |
| rules | `capability/rule.ts:286` | — | yes |
| hooks | `capability/hook.ts:27` | — | yes |
| tools | `capability/tool.ts:38` | tool name | yes |
| context-files | `capability/context-file.ts:27` | — | `context-file:<level>:<basename>` |
| prompts, instructions, settings, system-prompt, ssh, extension, extension-module | `capability/*.ts` | — | — |

A `Provider<T>` (`types.ts:63-89`) is `{ id, displayName, description, priority, load(ctx) }`.
The doc comment prescribes the priority bands: `100+` primary, `50-99` tool-specific,
`1-49` shared standards. The actual ladder, grepped from `src/discovery/*.ts`:

```
builtin (.omp native)   100   discovery/builtin.ts:42          14 registrations
omp-plugins              90   discovery/omp-plugins.ts:47       7
claude (.claude)         80   discovery/claude.ts:35            9
agent-plugins            75   discovery/agent-plugins.ts:41     2
codex (.codex)           70   discovery/codex.ts:43             9
claude-plugins           70   discovery/claude-plugins.ts:34    6   ("below claude.ts (80) so user .claude/ wins")
agents                   70   discovery/agents.ts:29            6
gemini (.gemini)         60   discovery/gemini.ts:41            6
opencode                 55   discovery/opencode.ts:45          6
cursor / windsurf        50   discovery/cursor.ts:37 windsurf.ts:31  3 / 2
cline                    40   discovery/cline.ts:17             1
github (.github)         30   discovery/github.ts:42            5
vscode (.vscode)         20   discovery/vscode.ts:16            1
managed-skills            5   discovery/builtin.ts:313          1
builtin-defaults          1   discovery/builtin-defaults.ts:22  1
```

`registerProvider` (`capability/index.ts:64-92`) inserts **priority-sorted at registration
time** via `findIndex(p => p.priority < provider.priority)`.

### 4b. How override-without-forking actually works

`loadImpl` (`capability/index.ts`):

1. All providers `load()` **in parallel** (`Promise.all`), each wrapped in try/catch — a
   broken provider becomes a warning string, never a crash.
2. Every item must carry `_source: { provider, providerName, path, level }`. Items without
   it are dropped with a warning. **Provenance is mandatory, not optional.**
3. `disabledExtensions` filter by stable id (`skill:foo`, `slash-command:bar`,
   `context-file:user:CLAUDE.md`) — applied *before* dedup.
4. Optional `filter()` (drop as if never existed) vs `suppress()` (excluded from results but
   **still claims its dedup key**, so a disabled project-level server keeps the same-named
   user server off). That distinction is subtle and correct.
5. First-wins dedup by `capability.key(item)`, plus optional `equivalent()` alias matching.
6. Shadowed items are **kept** in `result.all` with `_shadowed: true` for diagnostics.

So: to override a plugin's `review` skill, a user drops `.omp/skills/review/SKILL.md`
(priority 100) and it shadows the plugin's (priority 90). To delete one, they add
`disabledExtensions: [skill:review]` to `config.yml`. To turn off an entire *source*
(e.g. "stop reading `~/.claude`"), `disabledProviders` removes the provider by id.
**No forking, no copy-paste, no re-templating.** That is the whole contract.

### 4c. Three tiers of extension author

Deliberately layered by required skill:

- **Tier 1, no code.** Markdown/YAML/JSON dropped into `.omp/{skills,commands,rules,prompts,instructions}/`
  or the foreign equivalents. Slash commands are markdown with `description` / `argument-hint`
  frontmatter and `$ARGUMENTS` interpolation (`capability/slash-command.ts:34-45`;
  example `.omp/commands/release.md`).
- **Tier 2, packaged plugin.** A directory with `plugin.json` + `skills/` + `mcp.json`,
  installable from a marketplace. See §6.
- **Tier 3, TS/JS extension module.** Default-export a factory receiving `ExtensionAPI`.

### 4d. The `ExtensionAPI` surface

`src/extensibility/extensions/types.ts` (1786 lines). **41 typed lifecycle events** via
`pi.on(...)` — `session_start`, `session_before_compact`, `before_provider_request`,
`after_provider_response`, `tool_call`, `tool_result`, `user_bash`, `input`,
`tool_approval_requested`, `credential_disabled`, `mcp_notification`, and so on. Many are
**interceptors** returning a result type (e.g. `ToolCallEventResult` can `{ block: true, reason }`).

Registration methods: `registerTool`, `registerCommand`, `registerShortcut`, `registerFlag`,
`registerMessageRenderer`, `registerAssistantThinkingRenderer`, `registerComposerShape`,
`registerProvider` (an LLM *model* provider!), `registerFileWriteFallback`.
Runtime methods: `sendMessage`, `sendUserMessage`, `appendEntry`, `exec`, `setModel`,
`setThinkingLevel`, `setActiveTools`, `setServiceTier`, `getSessionName`.

Batteries injected so extensions need **zero dependencies**: `pi.zod`, `pi.typebox`,
`pi.arktype`, `pi.logger`, `pi.pi` (the whole host package). This matters more than it
looks — it is why 40+ tiny plugin repos have no build step.

Loading (`extensions/loader.ts`, 762 lines; `docs/extension-loading.md`) resolves a directory
by: `package.json#omp.extensions` (legacy `pi.extensions`) → `index.ts` → `index.js` → one-level
scan. Modules are imported with an `?mtime` cache-buster propagated **graph-wide** since 16.3.7,
so editing any file in an extension's dependency tree hot-reloads on re-import.

### 4e. Plugin manifest: features + typed settings

`src/extensibility/plugins/types.ts:26-95`. A plugin's `package.json#omp` may declare:

- `tools` / `hooks` / `extensions[]` / `commands[]` entry points
- **`features: Record<string, PluginFeature>`** — Cargo-style optional feature flags, each
  contributing its own extra `extensions`/`tools`/`hooks`/`commands`, with `default?: boolean`.
  Selective sub-installation of one plugin.
- **`settings: Record<string, PluginSettingSchema>`** — typed config (`string|number|boolean|enum`)
  with `default`, `min`/`max`/`step`, `values`, `description`, **`secret: true`** (masked in UI
  and logs) and **`env`** (environment-variable fallback).

Third-party plugins therefore get first-class typed settings UI and secret handling for free.
That is a large part of why the ecosystem repos look polished.

## 5. UPDATE — and how user edits survive

### Binary/self-update
`src/cli/update-cli.ts` (2073 lines) — disproportionately large, and deservedly so. It detects
**six** install methods at runtime (`UpdateMethod`, line 541): `brew | mise | nix | bun | npm | binary`,
by inspecting `argv[0]`, whether it's a symlink, whether it sits in a bun/npm bin dir, and
whether it's a Windows script launcher. Then it updates *through the manager you actually used*.

It also handles **package renames** mid-flight: a release manifest may carry
`"omp": { "rename": { "package": ..., "natives": ... }, "dist": "binary" }`; older deployed
updaters that don't understand `rename` fall through to the `dist: "binary"` escape hatch
(lines 118-142). That is real, hard-won forward-compat engineering — the updater is designed
so that *previously shipped versions of itself* degrade gracefully.

Post-update it prunes the bun install cache (`pruneBunInstallCache`, line 951).

### Plugin update
`omp plugin upgrade [--scope user|project] [name@marketplace]`; `/marketplace update [name]`
refreshes *catalogs only*, never reinstalls. Upgrade-all compares only entries that declare
`version` (semver must be strictly newer; non-semver = changed-if-unequal), and per-plugin
failures are skipped so a bulk upgrade can partially succeed.
`marketplace.autoUpdate`: `off | notify | auto`, default `notify`; catalogs older than 24h
refresh best-effort. **Documented wart:** "Despite its name, current `notify` mode writes
update availability only to the debug log; it does not show a user-facing notification."

### Lockfile
`~/.omp/plugins/omp-plugins.lock.json` (`getPluginsLockfile`, `dirs.ts:624`). Shape verified
from `test/plugin-install-local.test.ts:138`:

```json
{ "plugins": { "kimi-datasource": { "version": "1.0.0", "enabledFeatures": null, "enabled": true } } }
```

Plus `installed_plugins.json` (version 2) for marketplace provenance, per scope.
There is `omp plugin doctor [--fix]` (`plugins/manager.ts:932`) which detects orphaned
`node_modules` symlinks, lockfile/disk drift and repairs them.

### Why user edits are never clobbered
This is the elegant bit: **the updater never touches user content at all.** Shipped content
lives inside the binary/package; user content lives in `~/.omp` and `.omp/` and is composed
at *load* time by priority, not merged at *install* time by a templating step. There is no
"we overwrote your `.zshrc`" failure mode because nothing is ever written into a user file
by an upgrade. Config migration is one-way and backs up (`settings.json` → `config.yml`,
original renamed `.bak`; an invalid YAML settings file is moved to `.broken-*` and the process
exits with the backup path).

## 6. REGISTRY — protocol yes, index no

There *is* a full marketplace subsystem (`src/extensibility/plugins/marketplace/`, 1932 lines
across 7 files; `docs/marketplace.md`, 252 lines) with `/marketplace` TUI browser and CLI:

```
omp plugin marketplace add|remove|update|list
omp plugin discover|install|uninstall|upgrade|enable|disable|list|link|doctor
```

A marketplace is a git repo/local dir/JSON URL containing
`.omp-plugin/marketplace.json`, falling back to **`.claude-plugin/marketplace.json`** —
and the catalog `$schema` is literally
`https://anthropic.com/claude-code/marketplace.schema.json`. Plugin sources: relative path,
GitHub shorthand, git URL, git-subdir (monorepo). `npm` sources parse but are explicitly
rejected: *"npm plugin sources are not yet supported"*.

Beyond Claude compat, it implements a genuinely portable third-party standard:
`src/discovery/agent-plugin-format.ts` (551 lines) implements **Agent Plugins 1.0.0**
(`https://agent-plugins.org`) — a closed `plugin.json` schema (§5), closed `mcp.json` (§7.2),
`${PLUGIN_ROOT}`/`${PLUGIN_DATA}` placeholder expansion (§9.2), and package-boundary
containment (§4.1) — plus **Agent Skills** frontmatter validation
(`https://agentskills.io/specification`, closed 6-field schema, NFKC name normalization).
`classifyAgentPluginRoot()` prevents a standard-conformant plugin from *also* being loaded
through legacy Claude conventions — no double-loading.

**But there is no first-party index.** `can1357` publishes no marketplace catalog repo
(their only other omp repo is `homebrew-tap`, 2 stars). GitHub code search finds only 71 files
matching `.omp-plugin/marketplace.json`. Discovery of the real ecosystem is **ad hoc**:
the `oh-my-pi` GitHub *topic* (197 repos) and word of mouth.

### The second-order ecosystem is real
`gh api search/repositories?q=topic:oh-my-pi` → **197 repos**. Top by stars:
`am-will/gooey-pi` (847, desktop workspace), `AVIDS2/memorix` (710, cross-agent memory via MCP),
`Signet-AI/signetai` (264), `pulseaiclub/phi` (226), `czottmann/pi-automode` (106),
`FaqFirebase/pi-desktop` (71), `wolfiesch/omp-best-of` (63, best-of-N with LLM verifier),
`sakuradairong/omp-config` (44), `apoc/omp-desktop` (38), `bparlan/omp-agent` (38),
`kartikkabadi/omp-advisors` (28), `mikeatlas/omp-sbx` (26, Docker sandbox),
`pasky/pi-omplike-advisor` (109), `metaphorics/omp-plugin-dynamic-system-prompt`,
`rauls-kjarners/omp.nvim`, `yeet-src/agent-jail` (Landlock confinement).

Note the *shape* of that list: GUIs, sandboxes, memory layers, model routers, provider
adapters, editor bridges. Third parties did not just add prompts — they replaced whole
subsystems. That is only possible because `registerProvider` (model providers),
`before_provider_request`, and `tool_call` interception are public API.

## 7. UNINSTALL — leaves residue

**There is no `omp uninstall` command.** Confirmed: `docs/cli-reference.md` lists `uninstall`
only under `plugin`; no uninstall section in `README.md`; no uninstall handler in
`src/commands/` or `src/cli/`. The only `npm uninstall -g` call in the tree
(`update-cli.ts:1458`) is part of the package-rename migration path, not user-facing.

Removing the agent means manually:
1. `rm ~/.local/bin/omp` (or `brew uninstall` / `bun remove -g` / `npm uninstall -g` / `nix profile remove`)
2. `rm -rf ~/.omp` — and that directory is large and heterogeneous: sessions, `agent.db`
   (which holds **auth credentials**), caches, fastembed model weights, plugin `node_modules`,
   worktrees, per-project autoresearch and security dirs.
3. `rm -rf <each-project>/.omp`

Partial mitigation: `omp gc` exists (`src/commands/gc.ts`), and the residue is at least
*contained* — one dotdir per scope, no shell-rc edits, no PATH mutation, no files scattered
into `~/.config` unless XDG was explicitly opted into. But "clean" it is not.

## 8. TRACTION (fetched 2026-09-01 via `gh api`)

| Metric | Value | Source |
|---|---|---|
| Stars | **28,857** | `gh api repos/can1357/oh-my-pi` |
| Forks | 2,884 | same |
| Watchers (subscribers) | 90 | same |
| Open issues+PRs | 2,042 | same |
| Total issues (all states) | 4,647 | `search/issues?q=repo:...+is:issue` |
| Total PRs (all states) | 5,547 | `search/issues?q=repo:...+is:pr` |
| Releases | **583** | `releases?per_page=1` Link rel=last |
| Git tags | 830 | `tags?per_page=1` Link rel=last |
| Contributors | ~592 | `contributors?per_page=1&anon=1` Link rel=last |
| Commits on main | ~20,831 | `commits?per_page=1` Link rel=last |
| Created | 2025-12-31 | API |
| Last push | 2026-09-01 (same day) | API |
| Latest release | v18.1.0, 2026-09-01, 11 assets | `releases?per_page=1` |
| npm weekly downloads | **113,949** | `api.npmjs.org/downloads/point/last-week/@oh-my-pi/pi-coding-agent` |
| npm versions published | 605 | registry |
| License / language | MIT / TypeScript | API |
| Second-order ecosystem | 197 repos, topic `oh-my-pi` | `search/repositories` |

583 releases and ~20.8k commits in **8 months** (~85 commits/day). 592 contributors on a repo
that shipped its first commit on New Year's Eve. The README even mentions a retired "vouch
system" for gating contributions (README.md:32-33) — they had to *throttle* inbound help.

## 9. What's genuinely GOOD, and what's BAD

### Good

1. **Capability × provider matrix with numeric priority.** The single best idea in the repo.
   It converts "which config file wins?" from scattered per-subsystem `if` chains into one
   sorted list and one dedup pass. `capability/index.ts` is ~468 lines and governs skills,
   commands, MCP, rules, hooks, tools, prompts, settings, context files. Adding a new config
   *source* costs one file; adding a new config *kind* costs one `defineCapability` call.
2. **Mandatory provenance.** Every item carries `_source {provider, providerName, path, level}`
   or is dropped with a warning. Shadowed duplicates are retained in `result.all` with
   `_shadowed: true`. This makes "why is this skill active / where did it come from?" a
   query rather than an investigation. Most config systems make provenance an afterthought
   and can never answer that question.
3. **Stable disable-ids as a first-class concept.** `toExtensionId` gives every item a name
   (`skill:foo`, `context-file:user:CLAUDE.md`) that a user can list in `disabledExtensions`.
   Combined with priority-shadowing this yields *both* verbs — override and delete — with
   no forking and no template regeneration.
4. **Agent-authored content is a separate, lowest-priority provider.** `MANAGED_SKILLS_PRIORITY = 5`
   (`discovery/builtin.ts:313`) with the comment: *"so an authored skill of the same name from
   ANY other provider wins."* The agent may write skills into `~/.omp/agent/managed-skills`,
   and a human file of the same name always beats it. This is the cleanest answer I have seen
   to "let the agent self-modify without letting it stomp the user."
5. **`suppress()` vs `filter()`.** A suppressed item still claims its dedup key (so a disabled
   project MCP server keeps the same-named user server off) but cannot equivalence-shadow a
   differently-keyed survivor. Someone hit both bugs and encoded the fix in the type system.
6. **Ingest the competition.** 10 foreign config roots (`discovery/helpers.ts:31-87`):
   `.claude .codex .gemini .opencode .cursor .codeium/windsurf .cline .github .vscode`.
   Zero-friction onboarding: point `omp` at an existing repo and its Claude skills, Cursor
   rules, and `AGENTS.md` all light up. Cost: one ~150-500 line adapter each.
7. **Plugin `features` + typed `settings` schema** with `secret: true` masking and `env`
   fallback (`plugins/types.ts:26-95`). Third-party plugins get a settings UI and safe secret
   handling for free — a major reason ecosystem plugins look first-party.
8. **Six-way install-method detection in the updater**, plus forward-compatible package-rename
   handling designed so *already-deployed older updaters* degrade safely (`update-cli.ts:118-178`).
9. **Installer honesty.** Rosetta-aware arch detection and a post-install `--version` smoke
   test with a musl-specific remedy. "Never claim success for a binary that cannot run."
10. **Standards over conventions.** Implementing Agent Plugins 1.0.0 and Agent Skills as
    closed, validated schemas — with explicit classification so a standards-conformant plugin
    is not double-loaded through legacy paths — is real interop work, not a compat shim.
11. **`omp plugin doctor --fix`.** Reconciles lockfile against disk, detects orphan symlinks.
    Every package manager needs this and most bolt it on years late.
12. **Docs written against the source.** 81 docs that cite implementation files by path and
    line-level behavior, and that record their own warts (the `notify` mode admission, the
    hooks-are-legacy note). This is documentation as engineering artifact.

### Bad

1. **No sandbox. This is the big one.** `docs/extension-loading.md`: *"Extensions are **not
   sandboxed** (same process/runtime). They share one `EventBus` and one `ExtensionRuntime`."*
   A marketplace install is `git clone` → symlink into `node_modules` → `import()` → arbitrary
   code in-process with the agent's credentials (`~/.omp/agent/agent.db`), its network, and
   `tool_call` interception on every tool the model runs. **There is no signature, no checksum,
   no integrity pinning, no capability manifest, no trust prompt** — `marketplace/fetcher.ts`
   grep for `sha|verify|checksum|integrity` returns nothing but `clonePath`. The one `trust`
   hit in the whole plugin subsystem is a comment at `manager.ts:688`. A malicious plugin in
   a marketplace someone `/marketplace add`ed is a total compromise. The ecosystem's own
   response is telling: third parties shipped `mikeatlas/omp-sbx` (Docker) and
   `yeet-src/agent-jail` (Landlock) to add the isolation the host doesn't provide.
2. **No uninstall.** See §7. `~/.omp` holds credentials and model weights and nothing offers
   to remove it.
3. **Priority is a magic integer with no conflict detection.** cursor and windsurf are both
   `50`; codex, claude-plugins and agents are all `70`. Ties resolve by registration order,
   which is import order in `discovery/index.ts`. A third-party provider cannot slot itself
   between two built-ins without guessing a number, and nothing warns on collision.
4. **Two-and-a-half overlapping extension mechanisms.** "Extensions", "hooks", and "custom
   tools" are three loaders, three factory contracts, three docs — and `docs/hooks.md` opens by
   admitting hooks are effectively legacy (`--hook` is now an alias for `--extension`, and
   `HookToolWrapper` is dead code superseded by `ExtensionToolWrapper`). `docs/custom-tools.md`
   needs a "what this is and is not" table to disambiguate four concepts. Legacy carrying cost
   is visible: `legacy-pi-compat.ts` is **2897 lines**, plus three separate shim modules
   (`legacy-pi-ai-shim.ts`, `legacy-pi-coding-agent-shim.ts`, `legacy-pi-tui-shim.ts`) rewriting
   `@mariozechner/*` and `@earendil-works/*` specifiers at `onLoad` time. Eight months old.
5. **Marketplace mutations don't fully hot-apply.** Per `docs/marketplace.md`: TUI installs
   update disk and invalidate caches but need `/reload-plugins` for skills/commands/MCP, and a
   **full session restart** for new tools, hooks, or extension modules. ACP/RPC handlers behave
   differently again. Three inconsistent refresh semantics for one operation.
6. **Discovery asymmetries that will bite someone.** Native auto-discovery globs with
   `gitignore: true, hidden: false`; explicitly-configured directory scanning uses `readdir`
   and applies **no gitignore filtering**. Native scanning accepts only `.ts`/`.js`; installed
   plugin manifests also accept `.mjs`/`.cjs`. Native project discovery is **cwd-only and does
   not walk ancestors**, but `findAllNearestProjectConfigDirs` does — so monorepo behavior
   differs by subsystem.
7. **495 settings keys** in a 6327-line schema file. `config.yml` has become a second product
   surface. There is no profile/preset layer above it beyond whole-directory `--profile`.
8. **Scope shadowing has a sharp edge**: an *enabled* project install shadows an enabled user
   install, but a *disabled* project install does not — so toggling a project plugin off
   silently changes which code the user-scope plugin runs.
9. **2,042 open issues** against 4,647 total. Roughly 44% of all issues ever filed are still
   open. Velocity is outrunning triage.

## 10. Transfer analysis for oh-my-musecode (compiled Rust host)

### Transfers cleanly — this is the payload

| omp design | Muse Code mapping |
|---|---|
| **Capability × provider matrix**, priority-sorted, first-wins dedup | Port directly into Muse's config crate. A `trait Provider<T> { fn id(&self) -> &str; fn priority(&self) -> u32; async fn load(&self, ctx: &LoadContext) -> LoadResult<T>; }` plus a `Capability<T>` registry is *more* natural in Rust than in TS — the dedup pass is a `HashSet<String>` and a stable sort. This is the whole ballgame. |
| **Mandatory `_source` provenance on every item** | Muse already has `.muse/lock.json` with provenance/quarantine. Make provenance a required struct field (`SourceMeta { provider, path, level }`) so it cannot be forgotten, and surface it in a `musecode config list --json`. |
| **Stable disable-ids + `disabled_extensions` list** | `skill:<name>`, `command:<name>`, `context-file:<level>:<basename>`. Cheap, and gives users a delete verb that survives updates. |
| **Priority ladder as override contract** | Muse already reads `.muse-plugin`, `.claude-plugin`, `.codex-plugin`. Assign them explicit numeric priorities (native 100 > muse-plugins 90 > claude 80 > codex 70 …) and publish the table. Right now that precedence is implicit in a binary; making it a documented number is the difference between a config layer and a lottery. |
| **Agent-authored content as a separate lowest-priority provider** | Direct port. If Muse's agent can write skills, they belong in `~/.config/muse/managed-skills` at priority 5, permanently losing to human-authored files. |
| **Markdown/YAML-first content tiers** | Skills, rules, prompts, slash commands (frontmatter + `$ARGUMENTS`) are inert text. A Rust host parses YAML frontmatter + Markdown with `serde_yaml` + `pulldown-cmark` and needs no JS at all. **This is where oh-my-musecode should live.** |
| **Declarative JSON hooks** (`.muse/hooks.json`) | omp's *declarative* hook shape (event → matcher → command) transfers; its *TS factory* shape does not. Spawn a subprocess with JSON on stdin, read a JSON verdict on stdout. That is language-agnostic and sandboxable. |
| **Plugin `features` + typed `settings` schema** with `secret`/`env` | Pure data. Serde structs. Gives third-party Muse plugins a typed settings UI for free. |
| **Lockfile + `doctor --fix`** | `.muse/lock.json` already exists. Add reconciliation: orphan detection, lockfile↔disk drift, `--fix`. |
| **Multi-method updater** | Muse ships one Rust binary — but users will get it via brew/nix/curl/cargo. Detect the channel from `argv[0]` and update through it. Steal the "older deployed updaters must degrade gracefully" release-manifest discipline. |
| **Installer that writes exactly one file** | Keep oh-my-musecode's installer to `~/.local/bin` + one config dir. No shell-rc edits. This is what makes uninstall tractable — and unlike omp, **ship the uninstaller**. |
| **Agent Plugins 1.0.0 / Agent Skills conformance** | Highest-leverage interop move available. Muse already ingests `.claude-plugin` and `.codex-plugin`; implementing agent-plugins.org 1.0.0 (closed `plugin.json`, `mcp.json`, `${PLUGIN_ROOT}` expansion, containment) means oh-my-musecode content is portable *out* as well as in — and closed-schema validation is exactly what Rust's type system is for. |
| **Ingest competitors' config roots** | omp's 10 adapters × ~200 lines each. Muse's confirmed `.claude-plugin`/`.codex-plugin` ingestion is the seed; extend to `.cursor`, `.gemini`, `AGENTS.md`. Cheapest possible onboarding. |

### Does NOT transfer

| omp mechanism | Why it dies on a Rust host |
|---|---|
| **`ExtensionAPI` TS factories + Bun `import()`** | The entire tier-3 story is dynamic module loading in a JS runtime. A compiled Rust binary has no `import()`, no `?mtime` cache-buster hot-reload, no `onLoad` specifier rewriting. Muse's equivalent must be **out-of-process**: MSP (`muse serve` stdio host) or MCP subprocesses, or WASM (wasmtime + component model) for in-process. Either way the *ergonomics* change completely — an MSP/MCP plugin is a program with a protocol, not a function receiving a god-object. |
| **`pi.zod` / `pi.typebox` / `pi.arktype` / `pi.pi` dependency injection** | Only possible because host and plugin share one JS heap. A Rust host cannot hand a plugin its schema library. The Rust analogue is a **stable wire schema** (JSON Schema in the manifest) rather than an injected builder. |
| **41 in-process interceptor events with mutable return values** | `tool_call → {block, reason}` requires synchronous in-heap interception. Over a stdio protocol this becomes a request/response round-trip with latency and timeout semantics on *every tool call*. Muse should expose a **small** set of blocking interception points (pre-tool, pre-request) and make the rest fire-and-forget notifications. Porting all 41 as blocking IPC would be a performance disaster. |
| **`registerProvider` for LLM model providers** | omp lets a plugin add a model provider in-process. In Rust this is either a compiled-in trait impl (requires forking — exactly what we're avoiding) or a proxy process. Realistically: support an OpenAI-compatible **base-URL + headers** provider declaration in config, which covers ~90% of what those ecosystem plugins actually do, without code. |
| **npm/bun as the plugin package manager** | omp gets `node_modules` symlinking, semver resolution, and a registry for free. Muse has no such substrate. Options: git-clone-into-cache with a lockfile pinning **commit SHAs** (omp already supports `git-subdir` + `sha` sources — take that path and make SHA *mandatory*, fixing omp's missing-integrity flaw at the same time), or OCI artifacts. Do not build a package manager. |
| **`legacy-pi-compat.ts` (2897 lines) specifier rewriting** | Wholly an artifact of JS module resolution. Irrelevant. It *is* a warning though: omp accreted 3 shim modules and ~3k lines of compat in 8 months. Version the oh-my-musecode manifest schema from day one (`schema_version` field, refuse unknown majors) so this never starts. |
| **Bun-compiled single binary + Rust natives via addon loader** | Inverted for Muse: Rust is the host, so there is no natives-loading problem — and equally no free scripting runtime. That absence is precisely why tiers 1 and 2 (markdown/JSON) must carry the weight that tier 3 carries in omp. |

### The strategic lesson

The brief asked how omp "made itself extensible enough to be extended." The honest answer is
**two** things, and only one of them is the plugin API.

The plugin API (tier 3) is what produced the flashy repos — GUIs, model routers, memory layers.
It is also the part that does not transfer to Rust.

But the *substrate* — capability × provider × priority, with mandatory provenance and stable
disable-ids — is what made those plugins **composable instead of conflicting**. Forty plugins
can each contribute skills, commands, rules and MCP servers, from four different vendors'
config directories, and the resolution is one sorted list with a documented tiebreak. That is
the part that transfers completely, is more natural in Rust than in TypeScript, and is what
oh-my-musecode should be built on.

Concretely, oh-my-musecode should be: **a curated content pack (markdown skills/rules/commands/
prompts) + a priority-aware loader contract + a SHA-pinned lockfile + a real uninstaller**,
targeting Muse's existing `.muse-plugin`/`.claude-plugin`/`.codex-plugin` ingestion, with
executable extensions deferred to out-of-process MSP/MCP — sandboxed, which is the one thing
omp got materially wrong.

---

## Verification

**Verdict: MOSTLY_SOLID.** Independently re-derived on 2026-09-01 from a fresh
`git clone --depth 50` into `verify-can1357-oh-my-pi/repo/` plus live `gh api` calls.
The repo is real, the traction is real, and — the part that actually matters — every
load-bearing architectural claim was located in source and read. Nothing was fabricated:
no invented repo, no invented subsystem, no aspiration mistaken for implementation.
The defects are counting drift plus one wrong derived statistic.

### 1. Existence and traction — CONFIRMED, several figures exact

`gh api repos/can1357/oh-my-pi` returns 28,859 stars / 2,886 forks / 90 watchers /
2,044 open, created 2025-12-31, pushed 2026-09-01, MIT, TypeScript, homepage omp.sh.
The report's 28,857 / 2,884 / 2,042 are the same numbers a few hours earlier.

Re-derived independently via `Link rel=last`, all **exact**: 583 releases, 830 tags,
592 contributors, 20,831 commits. Also exact: 4,647 total issues, npm
`@oh-my-pi/pi-coding-agent` at 113,949 weekly downloads across 605 published versions,
latest release v18.1.0 with 11 assets, 197 repos under the `oh-my-pi` topic,
`can1357/homebrew-tap` at 2 stars. Total PRs is 5,548 (report said 5,547 — one PR opened
in between). 20,831 commits over 244 days is 85.4/day, matching the report's ~85.

### 2. Shipped-content counts — mostly exact, seven small overcounts

Verified exact by re-running the counts: repo totals (6,943 files / 4,734 `.ts` /
541 `.md` / 402 `.rs` / 218 `.json` / 116 `.tsx` / 107 `.kdl`), 29 `BUILTIN_TOOL_NAMES`
+ 3 `HIDDEN_TOOL_NAMES` (`yield`, `goal`, `think`), 152 files under `src/tools/`,
all 8 subagent prompts by name, 176 prompt markdown assets, 53 in `prompts/tools/`,
27 rule packs splitting exactly 8 Go / 6 Rust / 13 TypeScript, 98 theme JSONs,
3 personalities (`default.md`, `friendly.md`, `pragmatic.md`), a 6,327-line
`settings-schema.ts`, 14 `defineCapability` calls, 107 KDL files, and `.omp/`
dogfooding 5 project commands + 3 skills.

Corrections:

| Claim | Actual |
|---|---|
| 18 packages | **17** package directories (`tsconfig.workspace.json` is a file, not a package) |
| 9 Rust crates | **8** first-party `pi-*` crates + 1 vendored (`crates/vendor/brush-core`) |
| 78 in `prompts/system/` | **77** |
| 81 docs markdown | 81 **at `docs/` top level**; **130** including `skills/`, `tools/`, `toolconv/` |
| ~181 slash commands across 9 `builtin-*.ts` | **8** registry files; **77 top-level commands + 104 subcommands** = 181. "181 slash commands" overstates by conflating the two. |
| 2 bundled **markdown** commands | `ci-green` and `review` exist but are `index.ts` **TypeScript modules**, not markdown |
| 495 typed settings keys | **481** top-level keys in `SETTINGS_SCHEMA` (441 quoted + 40 bare, lines 473–6026) |
| 41 typed lifecycle events | **44** distinct `on(event: "…")` overloads (the report's regex missed line-wrapped signatures) |
| 107 KDL = 64 provider + 23 taxonomy + 19 class | those sum to 106; there is also **1 `runtime/`** rule |

### 3. Extension contract — CONFIRMED IN CODE, not aspiration

This was the highest-risk claim and it holds up line by line.

- `capability/types.ts:1-7` carries the thesis verbatim: *"instead of callers knowing about
  paths like `.claude`, `.codex`, `.gemini`, they simply ask for `load("mcps")`"*.
  `capability/index.ts` is 468 lines, as stated.
- `registerProvider` (index.ts:64-91) really does splice priority-sorted at registration.
- **The priority ladder is exact — all 16 constants verified**: builtin 100, omp-plugins 90,
  claude 80, agent-plugins 75, codex/claude-plugins/agents 70, gemini 60, opencode 55,
  cursor/windsurf 50, cline 40, github 30, vscode 20, managed-skills 5, builtin-defaults 1.
  The claimed tie collisions are real, and `claude-plugins.ts:34` even carries the comment
  `// Below claude.ts (80) so user .claude/ overrides win`.
- `loadImpl` confirmed: `Promise.all` over providers with per-provider try/catch turning a
  throw into a warning (lines 115-131); `_source` mandatory or the item is dropped with
  *"Item missing _source metadata, skipping"* (148-150); `disabledExtensions` resolved at
  line 113, **before** the dedup pass at 184; first-wins dedup setting `_shadowed = true`
  (207); and the `suppress()` vs `filter()` distinction is real, with the comment
  *"Claim key ownership … without surviving or equivalence-shadowing survivors"* (191-194).
- **82 `registerProvider` calls across 19 discovery modules — exact.** (A naive
  `registerProvider(` grep returns only 20 because most calls are generic,
  `registerProvider<MCPServer>(`; the report's figure is right.)
- `SOURCE_PATHS` (helpers.ts:31-84, not :31-87) confirmed.
- Tier-3 confirmed: `extensions/types.ts` is 1,786 lines; `pi.logger/typebox/arktype/zod/pi`
  are injected at 1218-1230; `registerTool`, `registerCommand`, `registerShortcut`,
  `registerFlag`, `registerMessageRenderer`, `registerComposerShape` and `registerProvider`
  all exist.
- `MANAGED_SKILLS_PRIORITY = 5` at `builtin.ts:313` with the quoted comment, **verbatim**.

**One correction, repeated throughout the report:** `SOURCE_PATHS` holds **10 roots of
which 9 are foreign** — `native` is the tenth. The report's own enumerated list
(`.claude .codex .gemini .opencode .cursor .codeium/windsurf .cline .github .vscode`)
has nine entries while the prose says "10 foreign config roots". This matters for the
Muse transfer section, which repeats the framing.

### 4. Install and update — FAITHFUL, line numbers exact

`scripts/install.sh` is 334 lines and `install.ps1` 309, as claimed. Confirmed:
`INSTALL_DIR="${PI_INSTALL_DIR:-$HOME/.local/bin}"` (line 15), `MIN_BUN_VERSION="1.3.14"`
(16), `chmod +x` on a single written binary (269). The Rosetta detail is real —
`host_arch()` at line 77 reads `sysctl -in hw.optional.arm64` with the comment
*"so it stays correct inside a Rosetta session, where `uname -m` reports the translated
x86_64"*, and `bun_arch_matches_host()` refuses a mismatched source build (309-313).
The smoke test is real: line 276 runs `"${INSTALL_DIR}/omp" --version`, preceded by the
comment *"Never claim success for a binary that cannot run"* (274) and followed by the
musl remedy `apk add libstdc++ libgcc` (284). **The "no shell-rc edits, no PATH mutation"
claim survives audit** — the only `export PATH` (165) is in-process during `--source`
mode, written to no file.

Updater: `update-cli.ts` is 2,073 lines; `type UpdateMethod = "brew" | "mise" | "nix" |
"bun" | "npm" | "binary"` is at **line 541 exactly**; the forward-compat rename/`dist`
escape hatch is at 115-135 with the documented contract that unknown values map to
`"binary"` *"so already-deployed updaters never run a package-manager install against a
release that no longer supports it"*; `pruneBunInstallCache` is at **line 951 exactly**.

### 5. Registry, uninstall, weaknesses — all confirmed

- **No uninstaller: confirmed.** `uninstall` appears in `docs/cli-reference.md` only at
  **line 232** as a `plugin` subcommand, exactly as claimed; README has zero matches; every
  hit in `src/commands/` and `src/cli/` is plugin-scoped; the lone `npm uninstall -g` is at
  **`update-cli.ts:1458` exactly**, inside the rename path. `src/commands/gc.ts` exists.
- **No sandbox: confirmed verbatim** at `docs/extension-loading.md:251-252` —
  *"Extensions are **not sandboxed** (same process/runtime). They share one `EventBus` and
  one `ExtensionRuntime` instance."* Grepping `fetcher.ts` for
  `sha|verify|checksum|integrity|signature` returns only an unrelated UNC-path comment:
  **there is genuinely no integrity verification.**
- Marketplace: 7 files totalling **1,931** lines (report said 1,932); `docs/marketplace.md`
  is 252 lines; `$schema` is literally `https://anthropic.com/claude-code/marketplace.schema.json`;
  `CATALOG_RELATIVE_PATHS = [".omp-plugin/marketplace.json", ".claude-plugin/marketplace.json"]`
  confirms the Claude-compat fallback; npm sources rejected with the exact string
  *"npm plugin sources are not yet supported. Use git-based sources instead."*
- `agent-plugin-format.ts` is 551 lines and really does implement Agent Plugins 1.0.0
  (`https://agent-plugins.org/schemas/1.0.0/plugin.schema.json`), agentskills.io skill
  validation, NFKC normalization and `classifyAgentPluginRoot`.
- `legacy-pi-compat.ts` 2,897 lines, `dirs.ts` 1,114 lines with `getPluginsLockfile` at
  **line 624**, lockfile shape `{version, enabledFeatures, enabled}` confirmed in the test,
  `doctor` at **`manager.ts:932`**, the `notify`-mode wart quoted verbatim at
  `docs/marketplace.md:218`, the three-way reload semantics at `docs/marketplace.md:78`,
  the hooks-are-legacy admission in `docs/hooks.md`, the `gitignore:true, hidden:false`
  asymmetry, and the retired vouch system (`README.md:31`, `CONTRIBUTING.md:8`).

### 6. The one genuinely wrong statistic

> "2,042 open issues against 4,647 ever filed — ~44% of all issues remain open."

This conflates issues with PRs. The 2,042/2,044 figure is GitHub's `open_issues_count`,
which **includes pull requests**. Search API breaks it down: **1,261 open issues + 783 open
PRs**. Against 4,647 issues ever filed, open issues are **27%**, not 44%. The underlying
point (triage lagging velocity) stands, but the number should be corrected.

### 7. Ecosystem list — real but mislabelled as "top"

Every ecosystem repo named was checked and **all exist with exactly the claimed star
counts** (pasky/pi-omplike-advisor 109, sakuradairong/omp-config 44, bparlan/omp-agent 38,
mikeatlas/omp-sbx 26, yeet-src/agent-jail 5, czottmann/pi-automode 106,
wolfiesch/omp-best-of 63). But sorting the `oh-my-pi` topic by stars shows the list is a
curated subset presented as "top ones" — it omits five higher-starred topic repos:
makoMakoGo/fish-claude 168, icoretech/codex-pooler 166,
khankamraan2006-crypto/fabric-router-core 116, FaqFirebase/pi-desktop 71,
GulajavaMinistudio/awesome-copilot-id 69. The top four (gooey-pi 847, memorix 710,
signetai 264, phi 226) are correct and correctly ordered.

### Bottom line

Nothing refuted. The transfer payload — capability × provider matrix, numeric priority
ladder, mandatory provenance, stable disable-ids, `suppress` vs `filter`, priority-5
managed skills, the absent sandbox and absent uninstaller — is all real, and I read the
code that implements each. Fix the nine counts above, the 44% statistic, and the
"10 foreign roots" (it is 9 foreign + native) before this report is used as a design input.
