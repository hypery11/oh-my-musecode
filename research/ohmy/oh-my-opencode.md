# oh-my-opencode — architecture teardown

**Verdict: CONFIRMED, but not under that name.**

`code-yeongyu/oh-my-opencode` was **renamed to `code-yeongyu/oh-my-openagent`**. Verified by HTTP 301:

```
$ curl -s https://api.github.com/repos/code-yeongyu/oh-my-opencode
{"message": "Moved Permanently", ...}
```

- Canonical repo: https://github.com/code-yeongyu/oh-my-openagent
- Homepage: https://omo.dev
- Clone: `/private/tmp/.../scratchpad/ohmy/oh-my-opencode/` (default branch `dev`, HEAD `b0658ba`, 2026-09-01)
- The npm package is **still named `oh-my-opencode`** (root `package.json:2`), dual-published as `oh-my-openagent`.

Do not confuse with `alvinunreal/oh-my-opencode-slim` (8,560 stars) — a separate, much smaller
declarative-config fork. This teardown covers the original.

---

## 1. Traction (real numbers, `gh` + npm registry API, fetched 2026-09-01)

| Metric | Value | Source |
|---|---|---|
| Stars | **68,585** | `gh repo view --json stargazerCount` |
| Forks | **5,630** | same |
| Open issues | **917** | `gh api repos/.../ --jq .open_issues_count` |
| Watchers | 231 | same |
| Contributors | **312** | `gh api .../contributors` Link header `page=312` |
| Releases | **255** | `gh api --paginate .../releases \| wc -l` |
| Created | 2025-12-03 | `createdAt` |
| Last push | **2026-09-01T14:39Z** (same day) | `pushedAt` |
| Repo size | 166 MB | `diskUsage` |
| npm `oh-my-opencode` | 98,055 / month | `api.npmjs.org/downloads/point/last-month` |
| npm `oh-my-openagent` | 148,185 / month | same |
| **Combined npm** | **~246k downloads/month** | |
| dist-tags | `latest` 4.19.4, `beta` 5.0.0-beta.31, `next` 4.5.12 | `registry.npmjs.org/-/package/.../dist-tags` |

255 releases in 9 months is roughly one release per day. This is a hyperactive project.

**Caveat on the star count**: the installer prompts `Star the repos on GitHub?` and shells out to
`gh api --method PUT /user/starred/...` (`packages/omo-opencode/src/cli/star-request.ts:31`).
It is genuinely opt-in — `initialValue: false` in the TUI (`tui-installer.ts:178`), `[y/N]` in the
CLI, and skipped entirely when not a TTY (`cli-installer.ts:212`) — but the stars are solicited at
install time, so treat 68.5k as a *solicited* number, not organic signal. Downloads are the honest metric.

**License is not OSS**: `LICENSE.md` is the **Sustainable Use License v1.0** (n8n's license) —
internal-business/personal use only, no paid redistribution. Not OSI-approved. Relevant if
oh-my-musecode wants to borrow code rather than ideas.

---

## 2. Scale and what it ships

It is **not** a bag of markdown files. It is a **31-package TypeScript monorepo** that compiles to a
plugin bundle plus per-platform binaries.

```
find packages -name '*.ts' -not -path '*/node_modules/*' | wc -l   →  5710
find packages -name '*.test.ts' -not -path '*/node_modules/*' | wc -l → 2275
```

**40% of source files are tests.** 13 CI workflows (`.github/workflows/`).

### Shipped content — real counts

| Category | Count | Evidence |
|---|---|---|
| Built-in agents | **11** | `config/schema/agent-names.ts:3` `BuiltinAgentNameSchema` — sisyphus, hephaestus, prometheus, oracle, librarian, explore, multimodal-looker, metis, momus, atlas, sisyphus-junior |
| Built-in slash commands | **7** | `features/builtin-commands/types.ts:3` — goal, refactor, ulw-execute, stop-continuation, handoff, remove-ai-slops, hyperplan |
| Shipped skill bundles | **17** | `find packages/shared-skills/skills -name SKILL.md \| wc -l` |
| Built-in skill names (schema) | 13 | `config/schema/agent-names.ts:17` `BuiltinSkillNameSchema` |
| **Individually toggleable hooks** | **57** | `config/schema/hooks.ts:3` `HookNameSchema` enum |
| Hook implementation dirs | 63 | `find packages/omo-opencode/src/hooks -maxdepth 1 -type d` |
| Custom tools | 16 | `packages/omo-opencode/src/tools/` |
| In-plugin MCP servers | 5 | `src/mcp/` — codegraph, context7, grep-app, lsp, websearch |
| Standalone MCP server packages | 3 | `ast-grep-mcp`, `git-bash-mcp`, `lsp-tools-mcp` |
| Runtime feature modules | 23 | `packages/omo-opencode/src/features/` |
| Platform binary packages | **12** | `packages/oh-my-opencode-{darwin,linux,windows}-*` incl. musl + AVX2-baseline variants |
| Codex plugin hooks | **23** JSON | `packages/omo-codex/plugin/hooks/*.json` |
| Codex plugin components | 14 | `packages/omo-codex/plugin/components/` |
| Vendored upstream skill repos | 4 git submodules | `.gitmodules` — open-design, taste-skill, ui-ux-pro-max, designpowers |

The 17 shared skills: ast-grep, coding-agent-sessions, data-scientist, debugging, frontend,
git-master, init-deep, lsp-setup, programming, refactor, remove-ai-slops, review-work,
ultimate-browsing, ulw-execute, ulw-plan, ulw-research, visual-qa.

**Note**: `.opencode/` and `.agents/` at repo root are the project's *own dogfooding config*
(5 commands, 13 skills), not the shipped product — though `package.json:files` does ship
`.opencode/command`, `.opencode/skills`, `.agents/command`, `.agents/skills`.

### It targets four hosts from one codebase
`packages/omo-opencode` (OpenCode), `packages/omo-codex` (Codex CLI), `packages/omo-senpi` +
`packages/omo-native` (senpi native), `packages/openclaw-core` (OpenClaw). This multi-host
posture is the single most transferable thing in the repo.

---

## 3. INSTALL — what it actually writes

Two distribution shapes.

### 3a. "Ultimate" (OpenCode) — `bunx oh-my-openagent install`

npm package + **platform-specific optionalDependency binaries**, the esbuild/swc pattern:
`bin/oh-my-opencode.js` resolves `oh-my-opencode-<os>-<arch>[-musl][-baseline]` and spawns it
(`bin/platform.js:31`). It probes **AVX2** at runtime (`/proc/cpuinfo` on Linux,
`sysctl machdep.cpu.leaf7_features` on macOS) and falls back to a baseline build; it even retries
the baseline binary on **SIGILL** (`bin/oh-my-opencode.js:203`). That is unusually careful.

`postinstall.mjs` is deliberately non-destructive: it verifies the platform binary resolves, warns
on OpenCode `< 1.4.0`, warns on a main/platform version mismatch, and **never fails the install**
(`postinstall.mjs:181`). Its one mutation is clearing the OpenCode plugin cache
(`invalidateOpenCodePluginCache`, `postinstall.mjs:100`).

The actual install (`src/cli/cli-installer.ts:33`) does exactly two writes:

1. **One line in the host's config.** `addPluginToOpenCodeConfig()`
   (`src/cli/config-manager/add-plugin-to-opencode-config.ts:198`) appends `oh-my-opencode@<version>`
   to the `plugin` array in `~/.config/opencode/opencode.json[c]`. For JSONC it uses
   `jsonc-parser`'s `modify()` + `applyEdits()` (line 176) — a **surgical edit that preserves the
   user's comments and formatting**. It also enumerates and patches every profile under
   `profiles/*/` (line 44), and it recognises a `file://.../src/index.ts` dev entry and *keeps* it
   rather than stomping it (`choosePluginEntry`, line 107). Before overwriting an existing entry it
   writes `<config>.backup-<ISO timestamp>` (`backup-config.ts:12`).
2. **Its own config, in its own namespace.** `writeOmoConfig()` writes `~/.omo/omo.jsonc`,
   deep-merged under an `[opencode]` key (`write-omo-config.ts:23`).

Plus: `ensureTuiPluginEntry()` for the TUI config, and `installAstGrepForOpenCode()` which provisions
the `sg` binary.

### 3b. "Light" (Codex CLI) — `npx lazycodex-ai install`

Documented at `README.md:127` and `packages/omo-codex/MARKETPLACE.md:24`. Writes:

```
~/.codex/plugins/cache/sisyphuslabs/omo/<version>/   # version-addressed plugin payload
~/.codex/.tmp/marketplaces/sisyphuslabs/plugins/omo/ # local marketplace snapshot
~/.codex/agents/*.toml                               # bundled agent definitions
~/.codex/config.toml                                 # [plugins."omo@sisyphuslabs"], marketplace,
                                                     #   [features] plugins=true, plugin_hooks=true
~/.codex/runtime/{node,ast-grep}/                    # provisioned runtimes
~/.local/bin/<component CLIs>                        # or ~/.codex/bin
```

The **version-addressed cache** (`packages/omo-codex/src/install/codex-cache-paths.ts:18`) is the
key move: each version lands in its own directory, so an upgrade never overwrites a live install;
old versions are pruned separately (`codex-cache-prune.ts`).

### 3c. Native edition — `npm i -g omo-ai@beta`

`packages/omo-native` uses `bun build --compile` to emit a **single native binary that embeds the
entire agent plus plugin payload**, then materializes the embedded runtime to disk on first run
(`compile-entry.ts:6`, `provisionEmbeddedRuntime`, `materializeProvisionedExecutable`,
`shouldReexecAfterProvisioning`). This is the closest analogue to a compiled Rust host.

---

## 4. EXTENSION CONTRACT — genuinely good, and the main lesson

**You never fork.** There are three independent, composable extension mechanisms.

### (a) Filesystem shadowing by name, with explicit precedence

`packages/skills-loader-core/src/features/opencode-skill-loader/loader.ts:96` searches **seven
skill roots** in a fixed order:

```
.opencode/skills/      (project, walked up)      ← highest
~/.config/opencode/skills/
.claude/skills/        (project)
.agents/skills/        (project)
~/.claude/skills/
~/.agents/skills/
<bundled shared-skills>                          ← lowest
```

`deduplicateSkillsByName()` (`skill-deduplication.ts:3`) is **first-wins**, and
`getAllSkills()` filters bundled skills whose names were already claimed
(`skill-discovery.ts:81`). So **dropping `~/.claude/skills/debugging/SKILL.md` on disk silently
replaces the shipped `debugging` skill.** No config, no registration, no fork. There is also an
explicit numeric ladder for merge conflicts (`merger/scope-priority.ts:3`):

```ts
builtin: 1, shared: 1, config: 2, user: 3, opencode: 4, project: 5, "opencode-project": 6
```

Agents work identically: markdown-with-frontmatter in `~/.claude/agents/`, `.claude/agents/`,
`~/.config/opencode/agents/`, `.opencode/agents/`
(`packages/claude-code-compat-core/src/features/claude-code-agent-loader/loader.ts:31-79`).

**It deliberately ingests a competitor's format.** `.claude/` paths are first-class
(`discoverSkills({ includeClaudeCodePaths: true })`), and there are three dedicated loaders:
`claude-code-agent-loader`, `claude-code-command-loader`, `claude-code-mcp-loader`. Every Claude Code
skill/agent/command in the wild is free inventory. This is exactly the bet Muse Code makes by
recognising `.claude-plugin` and `.codex-plugin`.

### (b) Declarative override in one namespaced config

`~/.omo/omo.jsonc` (`packages/omo-config-core/src/schema/config.ts:49`). Every built-in agent is
overridable by key, and new agents are declared the same way
(`schema/agent.ts:17` `OmoAgentDefSchema`): `description`, `prompt`, `model`, `models[]` (fallback
chain), `reasoning`, `tools`, `execution_mode`, `background`, `max_depth`, `allowed_subagents`,
`disallowed_tools`, `max_turns`, `temperature`, and **`disable: true`**. All 57 hooks are
individually disableable (`isHookEnabled`, `create-hooks.ts:45`).

Three things make this config design worth stealing wholesale:

- **Layered discovery.** `resolveOmoConfigPaths()` (`loader/paths.ts:104`) takes `~/.omo/omo.jsonc`
  as the user layer, then walks *up* from cwd collecting every `.omo/omo.jsonc`, **farthest-first**,
  stopping at `$HOME` so the home `.omo` is never double-counted as a project layer
  (`findProjectConfigPathsFarthestFirst`, line 76, depth-capped at 256).
- **Harness-scoped folding.** One file, with `[opencode]`, `[codex]`, `[senpi]` blocks that fold
  over the shared base (`schema/config.ts:60`). A single user config drives four different agent
  hosts. Plus named `profiles`.
- **Hardened merge.** `mergeOmoConfigRecords()` (`loader/merge.ts:27`) refuses
  `__proto__`/`constructor`/`prototype` keys at every depth; the loader rejects symlinked `.omo`
  dirs and config files (`paths.ts:42`) so a checked-out repo cannot redirect config reads.

### (c) Host-native plugin protocol

For Codex, `.codex-plugin/plugin.json` (`packages/omo-codex/plugin/.codex-plugin/plugin.json`) is a
declarative manifest: `skills` dir, a `hooks[]` array of JSON files, `mcpServers` pointing at
`.mcp.json`. Each hook JSON is a matcher + a command:

```json
{"hooks": {"PostToolUse": [{"matcher": "^(apply_patch|write|Write|edit|Edit|multi_edit|...)$",
 "hooks": [{"type": "command",
            "command": "node \"${PLUGIN_ROOT}/components/lsp/dist/cli.js\" hook post-tool-use",
            "timeout": 60,
            "statusMessage": "(OmO 5.0.0-beta.31) Checking LSP Diagnostics",
            "commandWindows": "powershell -NoProfile ... node-dispatch.ps1 ..."}]}]}}
```

Note `${PLUGIN_ROOT}` interpolation, a per-hook `timeout`, a user-facing `statusMessage`, and a
separate `commandWindows` for the Windows dispatch path. **This is very close to what
`.muse-plugin/plugin.json` + `.muse/hooks.json` need to be.**

---

## 5. UPDATE — good idea, weakest code

The plugin version is **pinned in the host config** as `oh-my-opencode@5.0.0-beta.31`, so the
installed version is user-visible and reproducible. A background hook
(`hooks/auto-update-checker/`) polls `registry.npmjs.org/-/package/oh-my-opencode/dist-tags`
with a 5s timeout (`constants.ts:20`), and **respects the user's release channel**: it parses the
current pin and stays on `beta`/`alpha`/`rc`/`canary`/`next` rather than yanking a beta user to
`latest` (`version-channel.ts:24`). Nice.

Two real strengths:
- **Rollback exists.** `revertPinnedVersion()` restores the prior entry when the new version fails
  to load (`checker/pinned-version-updater.ts:66`).
- **Compatibility gate.** `checkVersionCompatibility()` refuses illegal upgrades before writing, and
  a timestamped backup is taken first (`add-plugin-to-opencode-config.ts:156-171`).

But the auto-updater **rewrites the config with a hand-rolled regex/bracket-counter**
(`pinned-version-updater.ts:5-56`) — locate `"plugin"\s*:\s*\[`, count brackets to find the end,
`String.replace` the entry. This is a straight regression from the `jsonc-parser` AST edit the
installer uses 200 lines away, and it will mangle a config where a bracket appears inside a string.
**Two different config writers with different fidelity is the clearest bug-in-waiting in the repo.**

### Config migrations — the genuinely impressive part

`packages/omo-config-core/src/migration/` is a **crash-safe, journaled, locked schema-migration
engine** for a *config file* — the kind of thing normally reserved for databases:

- `_migrations: string[]` in the config records applied migration ids (`schema/config.ts:65`).
- A **write-ahead journal** at `~/.omo/.migration-journal.json` (`journal.ts:26`), versioned and
  strictly validated, written via exclusive-create-then-rename with collision retry (`journal.ts:96`).
  An interrupted migration **resumes** (`journalResumed`).
- A **PID + lease lock** at `~/.omo/.migration.lock` (`lock.ts:24`) with a 30s lease, renewal, and
  a `LIVE_OWNER_STALE_LEASE_MULTIPLIER` so a live owner must miss two renewal windows before
  takeover, while dead owners are reclaimable at expiry.

Two agents starting in two terminals will not corrupt shared config. That is the correct level of
paranoia for a tool that edits files users also hand-edit.

---

## 6. REGISTRY — none. This is the real gap.

There is **no index, marketplace, or registry of third-party additions** for omo itself. The only
`marketplace.json` in the tree is single-vendor and Codex-specific:

```json
{"name": "sisyphuslabs", "interface": {"displayName": "Sisyphus Labs"},
 "plugins": [{"name": "omo", "source": "./plugins/omo", "category": "Developer Tools",
              "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"}}]}
```

One marketplace, one plugin, both authored by the repo owner. It exists to satisfy Codex's plugin
loader, not to host anyone else's work. `find . -name 'registry*.json' -o -name 'plugins.json'`
returns nothing else.

**Yet an ecosystem formed anyway.** `gh search repos` surfaces dozens of real third-party projects:
`oh-my-opencode-slim` (8,560★), `OpenCode-Config-Manager` (396★), `oh-my-opencode-dashboard` (290★),
`oh-my-openclaw` (185★), config switchers, patch kits, Docker images, ports to Cursor/Copilot/MiMo/
Grok/pi. Discovery is entirely ad hoc — GitHub search and word of mouth.

This is the clearest opportunity for oh-my-musecode: the shadowing extension model *begs* for a
registry, and 68k stars of demand proved it, but nobody built one. Note also the pattern the repo
does use for third-party content — **git submodules** pinning four upstream skill repos
(`.gitmodules`), materialized at build time. That is curation-by-vendoring, not a registry, and it
does not scale past the maintainer's attention.

---

## 7. UNINSTALL — clean for Codex, absent for OpenCode

**Codex: genuinely well engineered.** `omo-agent-toolkit uninstall --platform=codex`:
- Strips only its own TOML sections. `removeTomlSections()` (`codex-config-toml-sections.ts:9`)
  splits `config.toml` into sections and drops only `[plugins."omo@sisyphuslabs"]` /
  `[hooks.state.*]` blocks — the rest of the user's file is untouched. Backup written first.
- **Deletion is allowlisted.** `validateManagedCleanupTarget()` (`codex-cleanup-safety.ts:16`)
  refuses any path that is not absolute, not inside `$CODEX_HOME`, equal to `$CODEX_HOME`, or not in
  an explicit set of five managed roots — and it explicitly refuses to operate when `codexHome`
  resolves to a filesystem root (`codexHomeResolvesToFilesystemRoot`, line 12). Anything else is
  reported as `skipped`, not deleted.
- Project-local `.codex` artifacts are **reported, never deleted** (`README.md:435`).

**OpenCode: there is no uninstall.** `cleanup.ts:23` hard-fails:

```ts
if (options.platform !== "codex") {
  console.error("Error: cleanup currently supports only --platform=codex")
  return 1
}
```

The README's OpenCode removal procedure is "edit the config by hand, then check `opencode --version`"
(`README.md:415`). Residue left behind on the flagship platform: the `plugin` entry, every
`opencode.json.backup-<timestamp>` the installer ever wrote, `~/.omo/` entirely
(config + `.migration-journal.json` + `.migration.lock` + project state), `~/.cache/oh-my-opencode/`,
`~/.cache/opencode/packages/oh-my-opencode@*`, and the provisioned `sg` binary. The asymmetry is
stark: the *secondary* host got the careful allowlisted uninstaller, the primary one got a doc paragraph.

---

## 8. What is genuinely GOOD

1. **One line in the host's config; everything else in your own namespace.** The entire footprint in
   `opencode.json` is a single `plugin` array entry. All settings live in `~/.omo/omo.jsonc`. Small
   blast radius, trivially auditable, trivially reversible.
2. **Surgical config edits via a JSONC AST** (`modify`/`applyEdits`) instead of parse-and-reserialize.
   User comments and formatting survive. Non-negotiable for a file humans hand-edit.
3. **Extension by filesystem shadowing with a published precedence ladder.** Drop a same-named file
   in a higher-priority dir and you have overridden a built-in. No registration, no fork, no
   `enabled_modules` list to keep in sync. Removing the file restores the default.
4. **Harness-scoped config folding** (`[opencode]` / `[codex]` / `[senpi]`) — one user config, four
   hosts, plus named `profiles`. This is the design that makes multi-host support tractable.
5. **Journaled + locked config migrations** with resume-after-crash and lease-based multi-process
   locking. Enormously over-built for a config file, and exactly right.
6. **Version-addressed install cache** (`.../omo/<version>/`) so upgrades cannot corrupt a live install.
7. **Ingesting the competitor's format** (`.claude/skills`, `.claude/agents`, `.claude/commands`).
   Every Claude Code asset becomes free inventory. Directly validates Muse Code's `.claude-plugin`
   ingestion bet.
8. **Allowlisted deletion in the uninstaller.** Nothing outside five named roots is ever removed.
9. **Prototype-pollution and symlink hardening in the config loader** — a checked-out repo cannot
   redirect config reads or poison an object prototype.
10. **57 individually named, individually disableable hooks.** Fine-grained opt-out beats an
    all-or-nothing plugin.
11. **Platform binaries with AVX2 detection and SIGILL fallback.** Handles old CPUs and musl.
12. **2,275 test files against 5,710 sources.** The paranoia is backed by tests.

## What is BAD

1. **No OpenCode uninstall.** The flagship platform's removal path is a README paragraph. Inexcusable
   for a tool that writes to six locations.
2. **Two config writers with different fidelity.** The installer uses a JSONC AST; the auto-updater
   uses regex + manual bracket counting (`pinned-version-updater.ts:5`). The second will eventually
   corrupt someone's config.
3. **Unbounded backup litter.** Every upgrade writes `opencode.json.backup-<ISO>` and nothing ever
   prunes them or offers a restore command. Backups nobody can find are not backups.
4. **Telemetry on by default**, opt-out only via `OMO_SEND_ANONYMOUS_TELEMETRY=0` /
   `OMO_DISABLE_POSTHOG=1`, disclosed in a post-install console line (`cli-installer.ts:238`) that
   npm hides without `--foreground-scripts`. For a config framework touching a user's whole dev
   environment this should be opt-in.
5. **Not open source.** Sustainable Use License. The "oh-my-*" name sets an OSS expectation the
   license does not honor.
6. **Naming chaos as shipped surface area.** One binary answers to five names
   (`oh-my-opencode`, `oh-my-openagent`, `omo-agent-toolkit`, `lazycodex`, `lazycodex-ai`), the
   package is dual-published, the repo was renamed, and the README needs a full paragraph warning
   that `bunx omo` resolves to an unrelated author's package (`README.md:178`). Real user harm.
7. **No registry**, despite an ecosystem that visibly formed without one.
8. **Prompting for GitHub stars during install.** Opt-in and TTY-gated, so not malicious — but it
   contaminates the project's own headline metric.
9. **917 open issues** against 312 contributors and daily releases. Velocity is outrunning triage.
10. **166 MB repo with vendored submodules and 12 platform packages** in-tree. Heavy to fork,
    heavy to audit.

---

## 9. Transfer analysis for oh-my-musecode (compiled Rust binary host)

### Transfers directly — steal these

| Pattern | Where | Muse mapping |
|---|---|---|
| One-line host registration, own namespace for everything else | `add-plugin-to-opencode-config.ts` | one entry in `.muse-plugin/plugin.json`; all settings in `~/.omo-muse/config.toml` |
| Format-preserving surgical config edit | `jsonc-parser` `modify`/`applyEdits` | `toml_edit` crate — the exact Rust equivalent, already format-preserving |
| Layered config: user + walk-up project layers, farthest-first, `$HOME`-bounded | `loader/paths.ts:76,104` | pure path logic, trivial in Rust |
| Harness-scoped folding `[opencode]`/`[codex]` + named profiles | `schema/config.ts:49` | `[muse]` / `[claude]` / `[codex]` blocks; native TOML table syntax makes this *nicer* in Rust than in JSONC |
| Extension by filesystem shadowing, first-wins, explicit numeric precedence | `loader.ts:96`, `skill-deduplication.ts:3`, `scope-priority.ts:3` | Muse already has skills/rules/agent-definitions dirs — publish the ladder |
| Journaled + PID/lease-locked config migration with resume | `migration/journal.ts`, `migration/lock.ts` | maps cleanly onto `.muse/lock.json`; `fs2`/`fd-lock` + atomic rename. Rust makes this *easier* |
| `_migrations: []` applied-id list | `schema/config.ts:65` | same idea in `.muse/lock.json` |
| Version-addressed install cache `.../<plugin>/<version>/` | `codex-cache-paths.ts:18` | plugin cache under `~/.muse/plugins/cache/<ns>/<name>/<ver>/` |
| Allowlisted deletion in uninstall; refuse filesystem root | `codex-cleanup-safety.ts:12,16` | port literally; Rust `Path::strip_prefix` is the containment check |
| Section-scoped config removal (drop only your own tables) | `codex-config-toml-sections.ts:9` | **already TOML** — `toml_edit` does this natively |
| Declarative hook manifest: event → matcher regex → command + timeout + statusMessage + `commandWindows` | `.codex-plugin/plugin.json` | this is the shape `.muse/hooks.json` should have. `${PLUGIN_ROOT}` interpolation included |
| Ingesting foreign formats (`.claude/`, `.agents/`) | 3 dedicated loader packages | Muse already reads `.claude-plugin`/`.codex-plugin` — extend to `.claude/skills`, `.claude/agents`, `.claude/commands` |
| Prototype-pollution + symlink rejection in config load | `merge.ts:27`, `paths.ts:42` | serde is immune to prototype pollution; **keep the symlink rejection**, it is a real attack |
| Version pinned in host config + channel-aware updates + rollback on failure | `version-channel.ts:24`, `pinned-version-updater.ts:66` | pin in `.muse/lock.json`; provenance/quarantine fields make this stronger than omo's |
| Named, individually toggleable hooks | `config/schema/hooks.ts` (57 entries) | pairs with `.muse/hooks.json` + `allowed_tools` |
| Embedded payload materialized to disk on first run | `omo-native/compile-entry.ts` | **the single most relevant precedent** — `include_dir!`/`rust-embed` for bundled skills/rules, extract to `~/.muse/` on first run, integrity-check thereafter |

### Does NOT transfer

- **The plugin *is* JavaScript.** `omoPlugin` is a JS module the host `import()`s
  (`omo-opencode/src/index.ts:6`), with in-process hooks holding live closures, disposers
  (`disposeCreatedHooks`), and shared mutable manager objects. A compiled Rust host cannot load
  that. Muse's equivalent is out-of-process: hooks as **subprocess commands** (the Codex model), or
  **MSP/stdio servers** over `muse serve`. Design for IPC from day one, not in-process callbacks.
- **npm as the distribution and update channel.** `registry.npmjs.org/.../dist-tags`, the
  `pkg@version` pin, `bunx`/`npx` bootstrap, and the 12 optionalDependency platform packages all
  assume a Node toolchain on the user's machine. A Rust binary needs its own channel: GitHub
  Releases + checksums, `cargo-binstall`, Homebrew, or a `muse plugin install` verb. **Keep** the
  ideas (version pinning, dist-tag channels, rollback); **drop** the npm dependency.
- **Bun/Node runtime provisioning.** `~/.codex/runtime/node`, `node-dispatch.ps1`,
  `commandWindows` PowerShell shims, `stage-lsp-daemon-runtime.mjs` — a large fraction of this
  codebase exists purely to guarantee a JS runtime is present. A Rust binary deletes this entire
  category of complexity. Do not recreate it.
- **AVX2 probing / SIGILL retry / musl+baseline matrix.** Rust's target triples and static musl
  builds handle this at compile time. 12 platform packages collapse to a normal release matrix.
- **The TS type system as the config contract.** zod schemas at runtime → serde + a generated
  JSON Schema. Strictly better, but it is a rewrite, not a port.
- **`.opencode`/`.claude` markdown-with-frontmatter agent definitions** transfer as a *format* but
  the loader is trivial; do not port the loader, port the precedence table.
- **The monorepo shape itself.** 31 packages with cross-package `export *` re-export shims (e.g.
  `omo-opencode/src/cli/install-codex/codex-cache-paths.ts` is one line re-exporting from
  `omo-codex`) is workspace-resolution scaffolding. A Rust workspace with real modules needs none
  of it.

### The three decisions to copy first

1. **One host-config line + `~/.omo-muse/config.toml` with `[muse]`/`[claude]`/`[codex]` folding and
   walk-up project layers.** This is the architecture. Everything else is detail.
2. **Publish the shadowing precedence ladder as the extension contract**, and support `.claude/` and
   `.codex/` paths in it. Zero-fork customization plus free inventory from two competitors' ecosystems.
3. **Build the registry omo never built.** A signed index of third-party skills/agents/hooks, backed
   by `.muse/lock.json` provenance and quarantine — which Muse already has and OpenCode does not.
   68k stars of proven demand and a visible third-party ecosystem, with discovery still stuck at
   GitHub search. That is the opening.

And copy the uninstaller **including** its allowlist — but unlike omo, ship it for the primary host.

---

## Verification

Adversarial re-verification performed 2026-09-01 against an independent clone at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/oh-my-opencode`
(origin `https://github.com/code-yeongyu/oh-my-openagent.git`, branch `dev`, HEAD `b0658ba` — the
same tree the teardown claims), plus live `gh` / GitHub API / npm registry calls.

**Verdict: MOSTLY_SOLID.** The repo is real, the traction is real, and every load-bearing
architectural claim was located in the actual source at or near the cited line. The errors are
confined to small counts.

### 1. Existence and traction — CONFIRMED

`gh api repos/code-yeongyu/oh-my-openagent` returns, live:

| Metric | Claimed | Measured |
|---|---|---|
| Stars | 68,585 | **68,585** ✅ |
| Forks | 5,630 | **5,630** ✅ |
| Watchers | 231 | **231** ✅ |
| Open issues | 917 | 916 (drifted by 1) |
| Contributors | 312 | **312** ✅ (`Link` header `page=312`) |
| Releases | 255 | **255** ✅ (`gh api --paginate \| wc -l`) |
| Created | 2025-12-03 | **2025-12-03T01:40:05Z** ✅ |
| Repo size | 166 MB | **166,478 KB** ✅ |
| Default branch | `dev` | **`dev`** ✅ |
| License | Sustainable Use v1.0 | **`NOASSERTION`**; `LICENSE` reads "Sustainable Use License / Version 1.0" ✅ |

Rename confirmed: `api.github.com/repos/code-yeongyu/oh-my-opencode` → **HTTP 301** to
`/repositories/1108837393`. npm last-month downloads: `oh-my-opencode` **98,055** +
`oh-my-openagent` **148,185** = **246,240** ✅ exact. The star-solicitation caveat is real and
correctly stated — see §5.

The four cited ecosystem repos were checked individually and **all four star counts are exact**:
`alvinunreal/oh-my-opencode-slim` 8,560 ✅, `icysaintdx/OpenCode-Config-Manager` 396 ✅,
`WilliamJudge94/oh-my-opencode-dashboard` 290 ✅, `happycastle114/oh-my-openclaw` 185 ✅.

### 2. Shipped-content counts — MOSTLY CONFIRMED, several off-by-one

Re-run independently at the same HEAD:

| Item | Claimed | Measured | |
|---|---|---|---|
| Built-in agents | 11 | **11** | ✅ exact — `BuiltinAgentNameSchema` list matches name-for-name |
| Built-in skill names | 13 | **13** | ✅ exact |
| Built-in slash commands | 7 | **7** | ✅ exact — `BuiltinCommandName` union matches name-for-name |
| Shipped skill bundles | 17 | **17** | ✅ `find packages/shared-skills/skills -name SKILL.md` |
| Hooks in `HookNameSchema` | 57 | **56** | ❌ off by one |
| Hook impl dirs | 63 | **62** | ❌ off by one |
| Custom tools | 16 | **15 dirs** (14 real; `shared/` is not a tool) | ❌ overcount |
| Runtime feature modules | 23 | **23** | ✅ exact |
| In-plugin MCP servers | 5 | **5** | ✅ codegraph, context7, grep-app, lsp, websearch — exact |
| Standalone MCP packages | 3 | **3** | ✅ ast-grep-mcp, git-bash-mcp, lsp-tools-mcp |
| Platform binary packages | 12 | **12** | ✅ exact `optionalDependencies` list |
| Codex hook JSON files | 23 | **23** | ✅ |
| Codex components | 14 | **14** | ✅ |
| Codex skills | 2 | **2** | ✅ |
| Vendored submodules | 4 | **4** | ✅ exact URLs in `.gitmodules` |
| Workspace packages | 31 | **30** | ❌ `package.json` `workspaces` has 30 entries (46 dirs exist under `packages/`) |
| `.ts` files | 5,710 | **5,848** | ❌ undercount ~2.4% |
| `.test.ts` files | 2,275 | **2,353** | ❌ undercount ~3.4% |

`git ls-files` and `find` agree, so the source-scale numbers were simply measured sloppily. The
**~40% test ratio conclusion survives** (2,353/5,848 = 40.2%).

### 3. Extension contract — CONFIRMED IN CODE, not aspirational

This was the priority check, and it holds up completely.

**(a) Filesystem shadowing.** `packages/skills-loader-core/src/features/opencode-skill-loader/loader.ts`,
`discoverAllSkills()` composes exactly **seven** roots in exactly the claimed order:

```
opencodeProject(.opencode/skills) > opencodeGlobal(~/.config/opencode/skills)
  > projectClaude(.claude/skills) > agentsProject(.agents/skills)
  > userClaude(~/.claude/skills) > agentsGlobal(~/.agents/skills) > shared(bundled)
```

`deduplicateSkillsByName()` (`skill-deduplication.ts`) is verbatim first-wins over a `Set`, so the
"drop a same-named SKILL.md in a higher root and the shipped one disappears" mechanic is real.
The numeric ladder in `merger/scope-priority.ts` is **exactly** as quoted:
`builtin:1, shared:1, config:2, user:3, opencode:4, project:5, "opencode-project":6`. ✅

**(b) Declarative override.** `OmoAgentDefInputSchema` (`packages/omo-config-core/src/schema/agent.ts`)
contains **every field claimed, in order**: `description, prompt, model, models[], reasoning, tools,
execution_mode, background, max_depth, allowed_subagents, disallowed_tools, max_turns, temperature,
disable` — plus two deprecated aliases the report omitted. `.strict()`. ✅

Loader hardening all confirmed in `packages/omo-config-core/src/loader/`:
`MAX_PROJECT_CONFIG_DIRECTORY_DEPTH = 256` (`paths.ts:18`) ✅; `DANGEROUS_KEYS = new Set(["__proto__",
"constructor", "prototype"])` in **both** `internal/plain-object.ts:1` and `loader/merge.ts:1` ✅;
symlink rejection is real and is applied to the `.omo` **directory itself**
(`detectOmoJsonPath` → `isSymlinkedProjectPath(omoDir)`), exactly as described ✅.

Harness folding: `OMO_CONFIG_HARNESS_IDS = ["opencode", "senpi", "codex"]`
(`schema/harness.ts`) — the three claimed keys, exact ✅.

**(c) Codex hook manifest.** `packages/omo-codex/plugin/.codex-plugin/plugin.json` declares
`"skills": "./skills/"`, a `hooks[]` array of JSON paths, and `mcpServers`. A sample hook file is
shape-for-shape what the report describes — event → `matcher` regex → `command` with
`${PLUGIN_ROOT}` interpolation, `timeout`, `statusMessage`, and a separate `commandWindows`
PowerShell dispatch:

```json
"PostCompact": [{ "hooks": [{ "type": "command",
  "command": "node \"${PLUGIN_ROOT}/components/git-bash/dist/cli.js\" hook post-compact",
  "timeout": 5, "statusMessage": "(OmO 5.0.0-beta.31) Resetting Git Bash MCP Reminder",
  "commandWindows": "powershell -NoProfile -ExecutionPolicy Bypass -File \"${PLUGIN_ROOT}\\...\\node-dispatch.ps1\" ..." }],
  "matcher": "manual|auto" }]
```

This is the single most transferable artifact in the teardown and it is **accurately reported** ✅.
(Nit: there are **two** `.codex-plugin/plugin.json` files, not one — a second under
`plugin/components/rules/`.)

### 4. Install / update — FAITHFUL, including the flagged weakness

- `bin` maps **five** names to one script — `oh-my-opencode, oh-my-openagent, omo-agent-toolkit,
  lazycodex, lazycodex-ai` ✅ exactly the five the "naming chaos" weakness names.
- AVX2 probing is real and exactly as described: `bin/oh-my-opencode.js:48` reads `/proc/cpuinfo`,
  `:56` shells `sysctl -n machdep.cpu.leaf7_features`, `:221` retries the baseline binary on
  `result.signal === "SIGILL"` ✅.
- `postinstall.mjs`: the only `throw` (line 173) sits inside a `try/catch` whose handler downgrades
  to `console.warn` under the comment `// Don't fail installation - let user try anyway` ✅. Its one
  mutation is `invalidateOpenCodePluginCache()` → `rmSync` on `~/.cache/opencode/packages/…` ✅.
- Installer writes via JSONC AST: `cli/config-manager/add-plugin-to-opencode-config.ts:3` imports
  `{ applyEdits, modify }`, writes at `:195`, calls `backupConfigFile()` at `:174` before
  overwriting, and enumerates `profiles/*/` at `:47-52` ✅.
- `@opencode-ai/plugin` pinned at **1.18.22** ✅ exact.
- Update poll: `NPM_REGISTRY_URL = https://registry.npmjs.org/-/package/${PACKAGE_NAME}/dist-tags`
  and `NPM_FETCH_TIMEOUT = 5000` ✅ — 5s timeout, exact.
- Channel awareness: `version-channel.ts` `extractChannel()` matches `/^(alpha|beta|rc|canary|next)/`
  ✅ exactly the five channels claimed.

**The headline weakness is real and correctly diagnosed.**
`hooks/auto-update-checker/checker/pinned-version-updater.ts` — `replacePluginEntry()` starts at
**line 5** — locates the plugin array with `content.match(/"plugin"\s*:\s*\[/)`, then walks
characters counting `[` / `]` **with no string-literal awareness**:

```js
for (let i = startIndex; i < content.length && bracketCount > 0; i++) {
  if (content[i] === "[") bracketCount++
  else if (content[i] === "]") bracketCount--
```

A `[` or `]` inside any string in that array miscounts the span. `revertPinnedVersion()` is at line
**65** (claimed 66). So: two config writers, different fidelity, the regex one corruptible — ✅
confirmed. (Rhetorical nit: they are not "200 lines away" — they live in different packages.)

**Migration engine — every specific verified.** `packages/omo-config-core/src/migration/` contains
`journal.ts`, `lock.ts`, `recovery.ts`, `commit.ts`, `transaction`-level tests.
`lock.ts:6` `DEFAULT_LEASE_DURATION_MS = 30_000` ✅ 30s; `lock.ts:9`
`LIVE_OWNER_STALE_LEASE_MULTIPLIER = 2` with the comment "A live owner must miss two extra renewal
windows before takeover" ✅ — the report's phrasing is nearly verbatim; `.migration.lock` +
`.guard` path ✅; `.migration-journal.json` ✅; `_migrations: z.array(z.string())` in the config
schema ✅.

**Uninstall asymmetry — CONFIRMED verbatim.** `packages/omo-opencode/src/cli/cleanup.ts:21-24`:

```ts
if (options.platform !== "codex") {
  console.error("Error: cleanup currently supports only --platform=codex")
  return 1
}
```

And `codex-cleanup-safety.ts:16` `validateManagedCleanupTarget()` is exactly the allowlist
described — absolute-path check, `codexHomeResolvesToFilesystemRoot()` refusal, containment check,
`target === codexHome` refusal, then a `Set` of **five** exact managed roots ✅. README §385
"Uninstallation" is indeed a manual-edit procedure ending in `opencode --version` ✅.

### 5. Refuted / corrected specifics

Nothing architectural was refuted. The following details are wrong:

1. **57 hooks → 56.** `HookNameSchema` has 56 entries; 62 impl dirs, not 63.
2. **16 custom tools → 15 subdirectories**, one of which (`shared/`) is not a tool — 14 real tools.
3. **31 workspace packages → 30** in the `workspaces` array.
4. **5,710 / 2,275 source files → 5,848 / 2,353** at the very HEAD the report cites.
5. **`initialValue:false` on the star prompt is wrong.** `maybePromptForGitHubStars()`
   (`cli-installer.ts:211`) is a plain `readline` question with a `[y/N]` default-no, not a clack
   prompt. *The substance survives and is arguably worse than reported:* the TTY gate
   (`if (!process.stdin.isTTY || !process.stdout.isTTY) return`) is real, it is genuinely opt-in,
   but it stars **two** repos (`code-yeongyu/oh-my-openagent` **and** `code-yeongyu/lazycodex`),
   not one. Path is `packages/omo-opencode/src/cli/star-request.ts`, not `src/cli/star-request.ts`.
6. **"three dedicated loader packages" for Claude Code ingestion → four loader modules inside one
   package.** `claude-code-compat-core/src/features/` holds `claude-code-agent-loader`,
   `claude-code-command-loader`, `claude-code-mcp-loader` **and** `claude-code-plugin-loader`;
   skills are handled separately in `skills-loader-core`. The ingestion claim itself is correct and
   if anything understated — it also reads `~/.claude/plugins` and `~/.claude/settings.json`.
7. **dist-tag `next` 4.5.12 exists on `oh-my-openagent` only**, not on `oh-my-opencode`
   (which carries only `latest` + `beta`).
8. **One `.codex-plugin/plugin.json` → two.**
9. Open issues 917 → 916 (live drift, not an error).

### 6. Items confirmed that most invited skepticism

Spot-checked because they read as too good to be true; all real:
telemetry on by default with the exact two opt-out env vars and the exact disclosure string
(`cli-installer.ts:180`, `tui-installer.ts:166`) ✅; `marketplace.json` is single-vendor with
exactly one plugin `omo` under namespace `sisyphuslabs` ✅; `find` for `registry*.json` /
`plugins.json` returns **nothing else** ✅; and `packages/omo-native/compile-entry.ts` really does
import all three of `provisionEmbeddedRuntime`, `materializeProvisionedExecutable`,
`shouldReexecAfterProvisioning` (used at `:198-199`) ✅ — the embedded-payload precedent the report
nominates as most relevant to a compiled Rust binary is genuine.

**Bottom line for oh-my-musecode:** the ten "transfers directly" recommendations rest on code that
exists and behaves as described. Correct the counts before quoting them; the architecture section
can be relied on as written.
