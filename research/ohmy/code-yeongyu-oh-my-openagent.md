# Teardown: code-yeongyu/oh-my-openagent (omo / lazycodex)

**Status: CONFIRMED — repo exists, cloned and read.**
Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/code-yeongyu-oh-my-openagent/`
URL: https://github.com/code-yeongyu/oh-my-openagent · Homepage: https://omo.dev
Default branch: `dev` · HEAD at read time: `b0658ba` (2026-09-01T23:22:03+09:00)

---

## 1. Existence & identity

Real. 179 MB working tree, 9,408 files (excluding `.git`), 5,848 TypeScript files.
npm package name is still `oh-my-opencode` (`package.json:name`) at version `5.0.0-beta.31`,
publishing **five** bin aliases to one entry point (`package.json:bin`):

```
oh-my-opencode | oh-my-openagent | omo-agent-toolkit | lazycodex | lazycodex-ai
   -> bin/oh-my-opencode.js
```

The name is a moving target: `oh-my-opencode` → `oh-my-openagent` → marketed as "omo/lazycodex".
The repo description now reads *"The coding agent for tokenmaxxers; the one and only agent harness
for complex codebases."* — note **"agent harness"**, not "config for an agent".

**This is the drift the sweep flagged, and it is real and complete.** It began as a config layer for
OpenCode and has become (a) a plugin for OpenCode, (b) a plugin for Codex CLI, and (c) `omo-ai`, a
*standalone agent* that vendors its own engine (`packages/omo-native/package.json` depends on
`@code-yeongyu/senpi` pinned to `2026.8.31` and ships its own `omo` bin). The config framework ate
the host.

---

## 2. What it ships (real counts)

| Category | Count | Evidence path |
|---|---|---|
| Workspace packages | 45 | `packages/*/` |
| Built-in agents | 10 | `packages/omo-opencode/src/agents/builtin-agents.ts` (`agentSources` map) |
| Skills — shared catalog | 17 | `packages/shared-skills/skills/*/SKILL.md` |
| Skills — senpi edition | 10 | `packages/omo-senpi/skills/*/SKILL.md` |
| Skills — codex plugin | 11 | `packages/omo-codex/plugin/**/SKILL.md` |
| Skills — repo's own dogfood | 13 | `.agents/skills/*/SKILL.md` |
| `SKILL.md` total in tree | 63 | `find . -name SKILL.md` |
| Hooks — Codex (declarative JSON) | 23 | `packages/omo-codex/plugin/hooks/*.json` |
| Hooks — OpenCode (TS modules) | 62 | `packages/omo-opencode/src/hooks/index.ts` exports |
| Agent tools | 15 | `packages/omo-opencode/src/tools/*/` |
| Codex plugin components | 14 | `packages/omo-codex/plugin/components/` |
| Bundled MCP servers | 5 | `packages/omo-codex/plugin/.mcp.json` |
| Slash commands (dogfood) | 5 | `.agents/command/*.md`, `.claude/commands/*.md` |
| Prebuilt platform binaries | 12 | `packages/oh-my-opencode-{darwin,linux,windows}-*` |
| Tests | 2,353 | `*.test.ts` |
| Per-directory design docs | 149 | `AGENTS.md` files scattered through the tree |
| User docs | 32 | `docs/**/*.md` |

The 10 built-in agents are Greek-mythology-named and defined **in TypeScript, not markdown**:
`sisyphus`, `sisyphus-junior`, `hephaestus`, `oracle`, `librarian`, `explore`, `multimodal-looker`,
`metis`, `momus`, `atlas` (`packages/omo-opencode/src/agents/builtin-agents.ts:33-46`).

No statusline and no output styles ship. There is no "settings preset" concept — presets are
expressed as `profiles` inside one config file instead.

**Prompt assembly is dynamic.** `dynamic-agent-prompt-builder.ts`, `dynamic-agent-core-sections.ts`,
`dynamic-agent-policy-sections.ts`, and `dynamic-agent-category-skills-guide.ts` build each agent's
system prompt at runtime from the *currently loaded* skill set and available models. Agent prompts
are computed, not authored. That's a real architectural choice with real consequences (see §9).

---

## 3. Install — what actually lands on disk

Three editions, three installers.

### Light edition (Codex CLI) — `npx lazycodex-ai install`
Entry: `packages/omo-codex/src/install/install-codex.ts` (290 LOC orchestrator over a **50-file**
install module — `packages/omo-codex/src/install/`).

Flow, read end to end:
1. Resolve `codexHome` = `$CODEX_HOME` or `~/.codex` (`install-codex.ts:32`).
2. Read the vendor manifest `packages/omo-codex/marketplace.json`, then each plugin's
   `.codex-plugin/plugin.json` (`codex-marketplace.ts:59`).
3. Build + copy the plugin into `~/.codex/plugins/cache/sisyphuslabs/omo/<version>/`.
4. Symlink component CLIs into a bin dir (`~/.local/bin` by default), and record *where it chose*
   into `.installed-bin-dir.json` so uninstall can find them later even if the env var that
   selected the dir was a one-shot (`codex-installed-bin-dir.ts`).
5. Link agent TOMLs into `~/.codex/agents/`, writing an `.installed-agents.json` manifest
   (`link-cached-plugin-agents.ts:9`), while **preserving** the user's per-agent `reasoning` and
   `service_tier` across reinstalls (`capturePreservedAgentReasoning` / `capturePreservedAgentServiceTier`).
6. Compute SHA-256 **trust hashes** for every hook handler and write them into config as trusted
   state (`codex-hook-trust.ts`, detailed in §5).
7. Surgically edit `~/.codex/config.toml` (`codex-config-toml.ts`, detailed below).
8. Install `ast-grep` and, on Windows, resolve/require Git Bash — install **aborts** if absent
   (`install-codex.ts:46`).
9. Fire install telemetry (`codex-install-telemetry.ts`).

Resulting footprint:
```
~/.codex/plugins/cache/sisyphuslabs/omo/
~/.codex/.tmp/marketplaces/sisyphuslabs/
~/.codex/plugins/data/omo-sisyphuslabs/bootstrap/
~/.codex/runtime/{ast-grep,node}/
~/.codex/agents/*.toml               (+ .installed-agents.json)
~/.codex/config.toml                 (managed blocks only)
~/.local/bin/<component CLIs>        (+ .installed-bin-dir.json record)
~/.omo/omo.jsonc                     (unified config, seeded/migrated)
```

### Ultimate edition (OpenCode) — `bunx oh-my-openagent install`
Registers the plugin in `opencode.json`, writes agent/model config, prompts for provider auth.
`postinstall.mjs` additionally **deletes OpenCode's plugin cache** on every npm install
(`invalidateOpenCodePluginCache()`, `postinstall.mjs:107-121`) — it walks
`$XDG_CACHE_HOME/opencode` and `.../packages` and `rmSync(recursive, force)` any directory starting
`oh-my-opencode@` or `oh-my-openagent@`. Best-effort and try/caught, but it is an unprompted
recursive delete in a shared cache directory owned by another product.

### Senpi/native edition — `npm i -g omo-ai@beta`
Ships the `omo` bin plus a pinned senpi engine. A bare `npm i -g omo-ai` **fails by design** —
every version is published as a prerelease under `--tag beta` so `latest` never resolves.

**Notable:** the README tells humans not to install it themselves. *"Strongly recommended: let an LLM
agent install this for you"* (`README.md:145`), then supplies a prompt to paste into another coding
agent pointing at a 1,000+ line install guide. That is an install UX confession.

---

## 4. Extension contract — how a user adds their own without forking

This is the strongest part of the project, and the part most worth stealing.

### 4a. Skills: multi-root discovery with explicit scope precedence
`packages/skills-loader-core/src/features/opencode-skill-loader/` (55 files, ~6.2k LOC per its own
`AGENTS.md`). Seven discovery sources across five scope families, deduplicated by skill `name`,
highest scope wins (`merger/scope-priority.ts`):

```ts
export const SCOPE_PRIORITY: Record<SkillScope, number> = {
  builtin: 1, shared: 1, config: 2, user: 3, opencode: 4, project: 5, "opencode-project": 6,
}
```

So `.opencode/skills/` in the project beats `.claude/skills/` in the project beats the user dirs
beats anything the framework ships. **A user overrides a built-in skill by creating a file with the
same `name` in a higher scope.** No fork, no patch, no registry entry. Disabled names never load.
Skills are `SKILL.md` with YAML frontmatter (`name`, `description`, `tools`, `mcp`) and support
`{{directory}}` / `{{agent}}` template variables resolved at load.

### 4b. Config: layered JSONC with harness blocks and profiles
`packages/omo-config-core/src/loader/paths.ts` + `resolution.ts`.

- User layer: `~/.omo/omo.jsonc`
- Project layers: every `.omo/omo.jsonc` from cwd **up to** the home boundary, applied
  farthest-first so nested workspaces layer correctly (`findProjectConfigPathsFarthestFirst`,
  `paths.ts:76-105`, depth-capped at 256).
- VSCode-style per-harness override blocks: `[opencode]`, `[codex]`, `[senpi]` (`resolution.ts:21`).
- Named `profiles`, activated by `OMO_PROFILE` > `OCX_PROFILE` > trailing dir of
  `OPENCODE_CONFIG_DIR` (`resolveOmoProfileName`, `resolution.ts:35-41`).

The home `.omo` is deliberately *not* also claimed as a project layer — there's an explicit guard
and comment for it (`paths.ts:96`). Someone hit that bug and fixed it properly.

### 4c. Agents: override or disable built-ins by key
`packages/omo-config-core/src/schema/agent.ts` — `agents: { <name>: {...} }` accepts
`description`, `prompt`, `model`/`models`, `reasoning`, `tools`, `execution_mode`, `max_depth`,
`allowed_subagents`, `disallowed_tools`, `max_turns`, `temperature`, and **`disable: true`**.
The schema is `.strict()`, so typos are rejected rather than silently ignored.
Fully custom agents come from `.claude/agents/*.md` or `.opencode/agents/`
(`claude-code-compat-core/src/features/claude-code-agent-loader/loader.ts:32-72`).

### 4d. Rules: a universal glob-matched markdown injector
`packages/rules-engine/src/constants.ts` — reads Cursor-style rule files from **nine** locations
with explicit priority:

```
.omo/rules(0) .claude/rules(1) .cursor/rules(2) .github/instructions(3)
.github/copilot-instructions.md(4) .sisyphus/rules(5)
~/.omo/rules(100) ~/.opencode/rules(101) ~/.claude/rules(102) ~/.sisyphus/rules(103)
```

Rules are `.md`/`.mdc` with frontmatter `description` + `globs`, injected when a matching file is
touched. It reads *its competitors' formats natively*. And `security-boundary.test.ts` proves
symlinked rule dirs that escape the workspace are rejected — a rules file is a prompt-injection
vector and they treated it as one.

### 4e. Plugins: it parasitises Claude Code's registry
`packages/claude-code-compat-core/src/features/claude-code-plugin-loader/` loads
`.claude-plugin/plugin.json` (falling back to bare `plugin.json`) and reads Claude Code's own
installed-plugin database at `~/.claude/plugins/installed_plugins.json`
(`discovery-paths.ts:12`). Sub-loaders exist for agents, commands, skills, hooks and MCP servers.
**Every plugin a user already installed for Claude Code is available here for free, with zero
registry of its own.**

**Verdict: this is a genuine extension contract, not copy-paste.** Four independent override
surfaces (skills / config / rules / plugins), all precedence-ordered, all documented, none
requiring a fork.

---

## 5. Update & clobber-avoidance

There is **no lockfile** in the `.muse/lock.json` sense. What exists instead:

### Managed-block surgical config editing (the good pattern)
`packages/omo-codex/src/install/codex-config-toml.ts` never parses-and-reserialises the user's
`config.toml`. It loads it **as a string** and applies namespaced `ensure*` / `remove*` operations
via a hand-written section editor (`toml-section-editor.ts`, `codex-config-toml-sections.ts`):

```
removeStaleMarketplacePluginBlocks(config, marketplaceName, keepSet)
ensureMarketplaceBlock(config, "sisyphuslabs", source)
ensurePluginEnabled(config, "omo@sisyphuslabs")
ensureHookTrusted(config, state)
ensureAgentConfig(config, agentConfig)
```

Every operation is keyed to `marketplaces.sisyphuslabs` / `<plugin>@sisyphuslabs` headers.
User comments, formatting, and unrelated blocks survive untouched. Stale blocks from *previous
versions of itself* are pruned by set-difference, including two legacy marketplace names
(`lazycodex`, `code-yeongyu-codex-plugins`, `codex-config-marketplaces.ts:5`).

For JSONC config it uses `jsonc-parser`'s `modify` + `applyEdits`
(`packages/omo-config-core/src/writer/writer.ts:4`) — same principle, comment-preserving edits.

### Atomic, symlink-aware writes
`codex-config-atomic-write.ts`: resolves symlinks to their real target *before* writing (so it
edits the file the user linked to, not the link), writes to a pid+timestamp temp file, renames,
and retries `EPERM`/`EBUSY` on a 10/25/50 ms backoff for Windows AV interference. The JSONC writer
additionally uses exclusive-create temp files and writes `.bak.<ISO-timestamp>` backups with
collision-numbered fallbacks (`writer.ts:37-51`).

### Hook trust by content hash (the closest thing to a lockfile)
`codex-hook-trust.ts` canonicalises each hook handler (sorted keys, normalized timeout,
platform-selected command) and emits `sha256:<hex>` keyed by
`<plugin>@<marketplace>:<hooksFile>:<event>:<groupIdx>:<handlerIdx>`. Those hashes are written into
config as trusted state at install. A hook whose command changes afterwards no longer matches its
trusted hash. **This is functionally Muse's `.muse/lock.json` provenance/quarantine model, arrived
at independently.**

### Migration engine
`packages/omo-config-core/src/migration/` — `lock.ts`, `journal.ts`, `commit.ts`, `recovery.ts`,
`backup-move.ts`, `merge.ts`. Imports legacy config files into the unified `omo.jsonc` using
`mergeWithoutClobber` (existing user values always win; skipped values become **diagnostics**, not
silent drops), stamps `_migrations: [id]` markers for idempotency, validates the result against the
Zod schema *before* commit, and leaves resumable backups in
`~/.omo/migration-backup-<UTC>-opencode-config/`. Lock + journal means an interrupted migration
resumes rather than corrupts.

### The weak spot
`packages/omo-opencode/src/hooks/auto-update-checker/checker/pinned-version-updater.ts` updates the
version pin in `opencode.json` by **bracket-counting and regex-replacing** inside the `"plugin"`
array. That is exactly the fragile approach the Codex side correctly avoids. It has a
`revertPinnedVersion` path for failed upgrades, which is thoughtful — but the technique is
inconsistent with the rest of the codebase's standards.

---

## 6. Registry

**NONE, in any meaningful third-party sense.**

`packages/omo-codex/marketplace.json` looks like a registry and is not one:

```json
{ "name": "sisyphuslabs",
  "plugins": [ { "name": "omo", "source": "./plugins/omo", ... } ] }
```

One vendor, one plugin, source `./plugin` — a **local** path, installed as
`sourceType: "local"` (`install-codex.ts:290`). This is the vendor wearing Codex's marketplace
mechanism as a costume so its own plugin installs through a supported code path. There is no index
of third-party omo extensions, no submission process, no discovery surface.

Discovery of *other people's* content is entirely delegated: whatever is in the user's
`~/.claude/plugins/installed_plugins.json`, `.cursor/rules`, `.github/instructions`, or
`.claude/skills` is picked up automatically. **Consumption without curation.** Strategically clever
— zero registry to operate, instant catalogue — but it means the project has no way to feature,
version, or vouch for community content, and the ~30 downstream ports have nowhere to land.

---

## 7. Uninstall

**Genuinely clean, and the best-engineered uninstall I have read in this space.**
`packages/omo-codex/src/install/codex-cleanup.ts` + `codex-cleanup-safety.ts` + `codex-cleanup-config.ts`.

- `validateManagedCleanupTarget()` allowlists **five exact paths** and refuses everything else:
  `plugins/cache/sisyphuslabs`, `.tmp/marketplaces/sisyphuslabs`, `runtime/ast-grep`,
  `runtime/node`, `plugins/data/omo-sisyphuslabs/bootstrap`. Anything outside is returned as a
  `SkippedCleanupPath` with a reason rather than deleted.
- Refuses to operate at all if `codexHome` resolves to a filesystem root
  (`codexHomeResolvesToFilesystemRoot`) — the `rm -rf /` guard, written down.
- Containment is checked with `relative()` + `isAbsolute()`, not string prefixes, so
  `../` escapes fail.
- Agent TOMLs are removed **from the `.installed-agents.json` manifest**, including orphans whose
  manifest is gone — it doesn't guess by filename.
- `config.toml` is restored by removing only managed blocks, after writing
  `config.toml.backup-<timestamp>` (`codex-cleanup-config.ts:46-48`).
- A user-owned file at a path the installer would have generated is **left untouched** (per
  CHANGELOG, for the `~/.local/bin/omo` wrapper).
- Bin links are removed from the *recorded* install-time bin dir, not a recomputed default.
- Project-local artifacts are **reported, not deleted** (`repairProjectLocalCodexArtifactsBestEffort`).

Residue after uninstall: `~/.omo/omo.jsonc` and any `.omo/omo.jsonc` (documented as a manual `rm` in
`docs/guide/installation.md:1007-1015`), the timestamped config backups, and migration backup dirs.
Leaving user config behind is arguably correct, and it is documented. OpenCode-edition uninstall is
manual `jq` surgery in the docs — noticeably weaker than the Codex path.

---

## 8. Traction (measured 2026-09-01 via `gh api` and the npm registry API)

| Metric | Value |
|---|---|
| Stars | **68,585** |
| Forks | **5,630** |
| Open issues | **917** |
| Watchers | 231 |
| Created | 2025-12-03 |
| Last push | 2026-09-01T14:39:32Z (same day) |
| Releases | ≥100 (API page cap hit) |
| Latest | `v5.0.0-beta.31`, 2026-08-31 — prerelease |
| Contributors | 100+ (API cap); `code-yeongyu` = **11,497** commits, next human = 396 |
| Repo size | 166 MB |
| License | Sustainable Use License (non-OSI, `NOASSERTION`) |

npm downloads, last 7 days (2026-08-23 → 2026-08-29):

| Package | Downloads |
|---|---|
| `oh-my-openagent` | 32,253 |
| `oh-my-opencode` | 19,970 |
| `omo-ai` | 4,654 |
| `lazycodex-ai` | 4,105 |

~61k weekly installs across names. Release cadence is roughly daily (beta.21 → beta.31 in six days).
**Every one of the last 10 releases is a prerelease.** There has been no stable release of 5.x.

Two numbers to hold together: 11,497 commits from the author vs. 396 from the next human. And 917
open issues against 68.5k stars. This is a one-person project at enormous scale, and the issue
tracker shows the strain.

---

## 9. What's genuinely good, and what's bad

### Good — steal these

1. **Scope-precedence override, expressed as a table.** `SCOPE_PRIORITY` is seven integers in one
   file. Every "how do I override this?" question has one answer, and it's greppable. Most config
   frameworks answer that question with prose.
2. **Surgical managed-block config editing.** Editing the user's file as *text*, touching only
   namespaced blocks, is the single highest-leverage decision in the codebase. It is why reinstall
   and uninstall are non-destructive, and why users can keep comments in their config.
3. **Symlink-aware atomic write with Windows rename retry.** 60 lines
   (`codex-config-atomic-write.ts`) that eliminate a whole class of corruption and support tickets.
4. **Allowlisted uninstall with a filesystem-root guard.** Uninstall is where frameworks destroy
   trust. This one enumerates exactly five deletable paths and reports skips with reasons.
5. **Install-time recording of install-time decisions.** `.installed-agents.json`,
   `.installed-bin-dir.json` — uninstall reads what install *did*, instead of recomputing what
   install *would do now*. Env vars change; manifests don't.
6. **`mergeWithoutClobber` + diagnostics.** Skipped migrations become visible warnings rather than
   silent data loss. Almost everyone gets this wrong.
7. **Reading competitors' formats natively.** `.cursor/rules`, `.github/instructions`,
   `.claude/skills`, `~/.claude/plugins/installed_plugins.json`. Zero-cost catalogue, zero registry
   to run.
8. **`.strict()` config schemas.** A typo in `omo.jsonc` is an error, not a silently ignored key.
9. **149 in-tree `AGENTS.md` files.** Per-directory design docs with LOC counts and file tables,
   written for the agent that will edit that directory. Genuinely novel, and it's how I navigated
   9,400 files quickly.
10. **Hook trust by canonical content hash.** Independent reinvention of a provenance lock.

### Bad — don't repeat these

1. **The identity crisis is a real cost, not a cosmetic one.** Package name `oh-my-opencode`, repo
   `oh-my-openagent`, product "omo", CLI `omo-agent-toolkit`, alt-CLI `lazycodex-ai`, npm `omo-ai`,
   marketplace `sisyphuslabs`, legacy `.sisyphus/rules`. Five bin aliases to one script. The
   CHANGELOG contains a paragraph explaining that a running agent may see exactly one failed `omo`
   call during relink. Users cannot form a stable mental model of what this thing is called.
2. **`postinstall.mjs` recursively deletes another product's cache** without asking
   (`invalidateOpenCodePluginCache`). Best-effort and wrapped, but it is an unprompted `rmSync`
   in `~/.cache/opencode` owned by OpenCode. If the reason is a caching bug in the host, the fix
   belongs in the host or behind a flag.
3. **Sustainable Use License on a framework built to be extended.** Internal-business or
   non-commercial use only; redistribution must be free and non-commercial. 5,630 forks are
   operating under a license most of them have not read. For an "oh-my-*" project — a genre defined
   by community contribution — this is a structural contradiction. It also means **omo's code
   cannot be vendored into oh-my-musecode.** Read it for ideas; do not copy lines.
4. **Perpetual beta.** Every one of the last 10 releases is a prerelease, and `omo-ai` is
   *engineered* so `latest` can never resolve. Daily prereleases against 917 open issues is a
   project that ships faster than it stabilises.
5. **Agents defined in TypeScript with runtime-computed prompts.** `dynamic-agent-prompt-builder`
   assembles system prompts from the live skill set. Powerful, but it means a user cannot read the
   prompt their agent is running, cannot diff it across versions, and cannot fully override it
   without the `prompt` escape hatch. The declarative surface stops at the agent boundary.
6. **Inconsistent engineering standard between editions.** The Codex installer is a 50-file,
   safety-audited, manifest-driven system. The OpenCode auto-updater regex-edits a JSON array by
   counting brackets, and OpenCode uninstall is `jq` in a docs code block. Same repo, two very
   different bars.
7. **The install UX is documented as unfit for humans.** "Strongly recommended: let an LLM agent
   install this for you" — for a *plugin manager*. If the TUI needs an LLM to drive it, the
   configuration surface is too large.
8. **Bus factor 1.** 11,497 commits vs. 396 for the next human contributor.

---

## 10. Transfer to Muse Code (compiled Rust binary)

### Transfers directly — this is the shortlist for oh-my-musecode

| omo mechanism | Muse Code equivalent |
|---|---|
| `SCOPE_PRIORITY` integer table | Precedence across `.muse/` project → `~/.config/muse/` user → shipped defaults. Publish it as a table. |
| Managed-block surgical editing of `config.toml` | `.muse/hooks.json` and settings. Use `toml_edit` / `serde_json` with a preserving formatter — Rust's `toml_edit` does format-preserving edits *better* than omo's hand-rolled string surgery. Namespace every block `[muse.omm.*]`. |
| Symlink-resolving atomic write + rename retry | `tempfile::NamedTempFile::persist()` on the resolved realpath. Same 60 lines, safer in Rust. |
| Hook trust by canonical SHA-256 | **Already native to Muse** as `.muse/lock.json` provenance/quarantine/`allowed_tools`. omo validates the design; adopt the canonical-JSON-then-hash detail (sorted keys, normalized defaults) so hashes are stable across writers. |
| Allowlisted uninstall + filesystem-root guard | Enumerate deletable paths under `~/.config/muse` and `.muse/`. Port `validateManagedCleanupTarget` almost line for line. |
| `.installed-*.json` manifests | Record what install did; uninstall reads the manifest. Trivial in Rust, high payoff. |
| `mergeWithoutClobber` + diagnostics | Merge into `.muse/` config with existing values winning and skips surfaced as warnings. |
| Lock + journal migration engine | Muse's config crate will need schema migrations. `fs2`/`fd-lock` + a journal file. |
| Glob-matched markdown rules from many roots | Muse has a rules subsystem. Read `.muse/rules`, `.claude/rules`, `.cursor/rules`, `.github/instructions` — the format is plain frontmatter + globs, host-agnostic. |
| Symlink-escape rejection for rule/skill dirs | A rules file is a prompt-injection vector. `std::fs::canonicalize` + containment check. Non-negotiable. |
| `SKILL.md` frontmatter format | Already the de-facto standard; Muse's skills subsystem should read it unmodified. |
| **Reading foreign manifests** | Muse's loader *already* accepts `.claude-plugin` and `.codex-plugin`. omo proves the payoff is large and the cost is a few hundred lines of adapter. Lean into it: oh-my-musecode should install Claude Code and Codex plugins as first-class citizens. |
| Per-directory `AGENTS.md` design docs | Pure markdown convention, zero runtime. Adopt immediately. |
| Strict schema rejection of unknown keys | `#[serde(deny_unknown_fields)]`. Free. |

### Does not transfer

- **The 45-package TypeScript monorepo and its 12 prebuilt platform binaries.** Muse is one Rust
  binary; the npm-postinstall-fetches-a-platform-package dance (`bin/platform.js`,
  `postinstall.mjs`, `detect-libc`) is solving a problem Muse doesn't have. Distribute via
  `cargo install` / brew / a signed tarball.
- **TypeScript-defined agents and runtime prompt assembly.** `builtin-agents.ts` composes prompts
  from live objects. Muse's agent-definitions subsystem is declarative; oh-my-musecode should ship
  agents as markdown-with-frontmatter and let a user read and diff the prompt. Muse's
  `MUSE_EXPERIMENTAL_*` gates are the right lever for conditional behaviour, not code branching in
  a prompt builder.
- **In-process JS hooks.** omo's 62 OpenCode hooks are TS modules loaded into the host process.
  Muse can only take the *Codex-shaped* 23: declarative JSON with `type: "command"`, a `matcher`
  regex, a `timeout`, and `${PLUGIN_ROOT}` interpolation — which is precisely the shape
  `.muse/hooks.json` wants. **Design oh-my-musecode's hooks as subprocess-command hooks only.**
  Copy the `commandWindows` sibling-key idea for cross-platform dispatch.
- **`bunx`/`npx` as the install channel and npm as the update channel.** Replace with a self-update
  path that reads a signed manifest.
- **The Sustainable Use License.** Legally cannot be carried over, and shouldn't be — an "oh-my-*"
  project needs MIT/Apache-2.0 or contributors won't come.
- **The parasitic single-vendor `marketplace.json`.** It exists to satisfy Codex's installer. If
  oh-my-musecode wants a registry, build a real index; if not, do what omo actually does — consume
  the ecosystems that already exist and skip the registry entirely.
- **The `omo-native` move (vendoring an engine to become a standalone agent).** This is the drift
  endgame. For oh-my-musecode the lesson is a boundary to *hold*, not a path to follow: Muse Code
  is the host, omm configures it. The moment omm ships its own engine it inherits every
  maintenance burden Meta is already carrying.

### The one strategic read

omo's most valuable and most transferable idea is not any single file — it's that **the framework
never built a registry and won foreign ecosystems instead.** It reads Claude Code plugins, Cursor
rules, GitHub Copilot instructions and Claude skills as native inputs. Muse Code's loader already
accepts `.claude-plugin` and `.codex-plugin` manifests, which means oh-my-musecode starts with the
same advantage *on day one, by design rather than by adapter*. Ship the precedence table, the
managed-block writer, the hash-pinned lock, and the allowlisted uninstall — then let Muse's existing
compatibility surface supply the catalogue.

The cautionary half is equally clear: omo is what a config framework becomes when nobody draws a
boundary. It absorbed the host, forked into three editions, renamed itself four times, and now
carries 917 open issues on one maintainer. Steal the file-level engineering, which is excellent.
Refuse the scope.

## Verification

**Verdict: MOSTLY_SOLID.** Independently re-cloned and re-measured on 2026-09-01 into
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-code-yeongyu-oh-my-openagent/repo`
(fresh `git clone --depth 50`, HEAD `185a90b`, 2026-09-01T23:52:27+09:00 — three commits ahead of the
report's `b0658ba`, which is present in history at exactly the claimed timestamp 2026-09-01T23:22:03+09:00).
The repo is real, the traction is real, and the engineering claims are not README paraphrase — nearly
every code assertion reproduced verbatim from the files named. One claim is refuted and seven need correction.

### Repo and traction — CONFIRMED, exact

`gh api repos/code-yeongyu/oh-my-openagent`: 68,585 stars / 5,630 forks / 231 watchers / created
2025-12-03T01:40:05Z / size 166,478 KB / license NOASSERTION ("Other"), `LICENSE` opens "Sustainable Use
License, Version 1.0". Open issues 916 (report said 917 — 30 minutes of drift). Releases API returns
exactly 100 (per_page cap), all of the last 10 are `prerelease=true`, v5.0.0-beta.31 published
2026-08-31T16:20:10Z, beta.21→beta.31 spans 2026-08-26→08-31. Contributors: code-yeongyu 11,497,
github-actions[bot] 731, MoerAI 396 — bus-factor-1 claim holds (next *human* is 396). npm downloads for
2026-08-23..29 returned to the digit: oh-my-openagent 32,253 / oh-my-opencode 19,970 / omo-ai 4,654 /
lazycodex-ai 4,105. Working tree 179 MB.

### Shipped-content counts — CONFIRMED

Re-ran every count. 9,408 files, 5,848 `.ts`, 63 `SKILL.md`, 149 `AGENTS.md`, 45 dirs with a
`package.json` under `packages/` (of which 12 are `oh-my-opencode-<platform>` binary packages), 17
shared-catalog skills, 10 senpi skills, 11 codex-plugin skills, 13 `.agents/skills`, 23 hook JSONs, 15
tool dirs, 14 plugin components, 5 MCP servers in `.mcp.json` (grep_app, context7, codegraph, git_bash,
lsp), 32 `docs/**/*.md`, 5 + 5 slash commands, zero statusline/output-style files anywhere. Test files
2,356 vs 2,353 claimed (drift from the three newer commits). `builtin-agents.ts` `agentSources` contains
exactly the 10 named agents. `hooks/index.ts` has 62 export statements. The 23 hook JSONs cover exactly
the 7 claimed events: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, PostCompact, Stop,
SubagentStop.

### Extension contract — CONFIRMED, implemented not aspirational

All four surfaces exist in the code paths named:

- **Skills.** `merger/scope-priority.ts` is the seven-integer table verbatim
  (`builtin:1, shared:1, config:2, user:3, opencode:4, project:5, "opencode-project":6`), and
  `merger.ts:86` actually consumes it: `if (!existing || SCOPE_PRIORITY[skill.scope] > SCOPE_PRIORITY[existing.scope])`.
  Loader dir is 55 files / 6,148 LOC (report said ~6.2k). `deduplicateSkillsByName` and per-skill
  `mcpConfig` both exist.
- **Config.** `loader/paths.ts` confirmed: `MAX_PROJECT_CONFIG_DIRECTORY_DEPTH = 256`,
  `findProjectConfigPathsFarthestFirst`, and the home-boundary guard with the verbatim comment
  *"A home `.omo` is a user layer, so the walk must not also claim it as a project layer."*
  `schema/config.ts` carries literal `"[opencode]"` / `"[senpi]"` / `"[codex]"` keys and `profiles`;
  `loader/resolution.ts:36-38` resolves `OMO_PROFILE` → `OCX_PROFILE` → `OPENCODE_CONFIG_DIR` in that order.
- **Agents.** `schema/agent.ts` accepts exactly the listed fields and ends in `.strict()`; 55 `.strict()`
  calls across the schema dir.
- **Rules.** `rules-engine/src/constants.ts` `SOURCE_PRIORITY` matches the claimed 10-entry ordering;
  `security-boundary.test.ts` exists.
- **Plugins.** `claude-code-compat-core/.../discovery-paths.ts:12` joins `installed_plugins.json`;
  `plugin-manifest.ts:8` joins `.claude-plugin/plugin.json`. The parasitic-ingestion thesis is real.

### Install / update / uninstall — CONFIRMED, near-verbatim

`install-codex.ts` is 290 lines and orchestrates exactly as described: reads `marketplace.json`, validates
manifest-vs-marketplace name, `writeInstalledCodexBinDir` (with the in-source comment explaining
`CODEX_LOCAL_BIN_DIR` is "often a one-shot override, so uninstall cannot recompute this location"),
`installAstGrepForCodex`, `trackCodexInstallTelemetry`, and it throws `gitBashResolution.installHint`
on Windows without Git Bash. `codex-config-toml.ts` loads config as a **string** and applies the named
`ensure*`/`remove*` ops including legacy pruning for `lazycodex` and `code-yeongyu-codex-plugins`, then
enables `plugins`/`plugin_hooks`/`multi_agent`. `codex-config-atomic-write.ts` matches line for line:
symlink→realpath, pid+timestamp temp, `RENAME_RETRY_DELAYS_MS = [10, 25, 50]`, `EPERM`/`EBUSY` only.
`codex-hook-trust.ts` produces `sha256:<hex>` over sorted-key canonical JSON with normalized timeout and
`commandWindows` platform selection. `cleanup-safety.ts` contains the five exact managed roots and
`codexHomeResolvesToFilesystemRoot`, using `relative()`/`isAbsolute()` containment. `writer.ts` uses
jsonc-parser `modify`+`applyEdits` and `.bak.<suffix>`. `pinned-version-updater.ts` really does
bracket-count and regex-replace inside the `"plugin"` array, and really does export `revertPinnedVersion`.
The migration dir has `lock.ts`, `journal.ts`, `commit.ts`, `recovery.ts`, `backup-move.ts`, `merge.ts`
with `mergeWithoutClobber`, and `_migrations` markers live in `schema/config.ts:65`.

The OpenCode-has-no-uninstall criticism is **stronger** than reported: `cli/cleanup.ts` types
`CleanupPlatform = "codex"` and hard-fails any other target with
`"Error: cleanup currently supports only --platform=codex"`. Docs confirm the jq snippet at
`docs/guide/installation.md:1003`. `README.md:145` matches verbatim, and `installation.md` is 1,043 lines
(the "1000+ line install guide"). The CHANGELOG relink caveat is verbatim, including "a single failed
`omo ulw-loop` call". `package.json` `bin` maps all five aliases to `bin/oh-my-opencode.js`.
`omo-ai` (packages/omo-native) depends on pinned `@code-yeongyu/senpi": "2026.8.31"`.
Log rotation confirmed in `packages/utils/src/logging/logger.ts`:
`DEFAULT_MAX_LOG_FILE_SIZE_BYTES = 50 * 1024 * 1024` with `.1`/`.2`-style backup renaming.

### REFUTED

1. **"Supports `{{directory}}`/`{{agent}}` template vars"** (extension_contract, surface 1) — no
   implementation exists. Grepping `{{` across all of `packages/skills-loader-core/src/` (excluding tests)
   returns **zero** hits. The claim traces to exactly two documentation lines
   (`packages/skills-loader-core/src/features/opencode-skill-loader/AGENTS.md:72` and its
   `omo-opencode` mirror) asserting "Variables like `{{directory}}`, `{{agent}}` in skill content get
   resolved at load time". The only real template injection in the loader is
   `git-master-template-injection.ts`, which rewrites git commands inside bash code blocks — not variable
   substitution. This is a doc claim the code does not honour, and the report repeated it unverified.

### CORRECTIONS

2. **marketplace.json source string.** The file declares `"source": "./plugins/omo"` and contains **no**
   `sourceType` field. `sourceType:"local"` appears only in type definitions and test fixtures. The
   installer supplies `resolvePluginSource(..., { pathOverride: "./plugin" })`, which is where `./plugin`
   comes from. The substantive claim — one plugin, one vendor, local path, registry-as-costume — is correct.
3. **Install module size.** 67 non-test files (117 including tests), not "a 50-file install module".
4. **Uninstall allowlist is five roots plus one pattern.** `validateManagedCleanupTarget` also permits
   `isManagedBootstrapDriftPath`: any `plugins/**/<owner>/bootstrap` where owner starts with `omo` and
   contains `sisyphuslabs`. "Exactly five deletable paths" understates it slightly.
5. **The postinstall deletion is narrower than the footprint line says.** `config_footprint` claims
   `$XDG_CACHE_HOME/opencode/**` is "DELETED recursively on every npm install". Actual behaviour
   (`postinstall.mjs:108-124`): it iterates only `opencode/` and `opencode/packages/`, and `rmSync`s only
   direct children whose names start with `oh-my-opencode@` or `oh-my-openagent@`
   (`OPENCODE_PLUGIN_PACKAGES` at line 18). The weaknesses bullet describes this correctly; the footprint
   line overstates it.
6. **Path drift (three items).** `pinned-version-updater.ts` lives under
   `packages/omo-opencode/src/hooks/auto-update-checker/checker/`, not `src/features/...`. The Codex
   manifest is at `packages/omo-codex/plugin/.codex-plugin/plugin.json`, not `packages/omo-codex/.codex-plugin/`.
   Rules `SOURCE_PRIORITY` user-scope keys are `~/.omo/rules`, `~/.opencode/rules`, `~/.claude/rules`,
   `~/.sisyphus/rules` (the report abbreviated the `/rules` suffix away).
7. **Hook-trust key component.** The event segment is a snake_case *label* (`pre_tool_use`), not the raw
   event name (`PreToolUse`) — mapped through `EVENT_LABELS`. Matters if the design is copied.
8. **Counts at a moving HEAD.** 2,356 test files and 916 open issues at verification time vs 2,353/917
   reported. Drift, not error — but any figure from this repo needs a date stamp.

### Addition the report missed (strengthens its own thesis)

`loader/paths.ts` also rejects **symlinked project `.omo` directories and config files**
(`isSymlinkedProjectPath`, applied in `detectOmoJsonPath`/`isLoadableProjectConfigFile`, and failing
closed — an `lstat` error returns `true`/untrusted). The report credited symlink-escape rejection only to
the rules engine; the same defense is independently implemented in the config loader. For
oh-my-musecode this reinforces transfer item (10): treat *every* project-supplied config and rules path as
an injection vector, not just `rules/`.

### Bottom line for oh-my-musecode

The transfer shortlist survives verification. The five items most worth stealing —
`scope-priority.ts`, `codex-config-toml.ts` managed-block string editing, `codex-config-atomic-write.ts`,
`codex-hook-trust.ts` canonical hashing, and `codex-cleanup-safety.ts` — were all read in full and are
exactly what the report says they are. The licensing blocker (Sustainable Use License, non-OSI) is
confirmed from `LICENSE` and means these files can be *studied and reimplemented*, never vendored.
