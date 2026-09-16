# Teardown: oh-my-claudecode (OMC)

**Verdict: CONFIRMED — exists, is the canonical "oh my claudecode", and is by a wide margin the
most architecturally serious project in this space.**

- Repo: https://github.com/Yeachan-Heo/oh-my-claudecode
- npm package: `oh-my-claude-sisyphus` (note the mismatch — repo name ≠ package name)
- Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/oh-my-claude-code/` (`git clone --depth 50`, HEAD `e9e8fa3`, 94 MB)
- Version at HEAD: **5.1.0** (`package.json:3`, `.claude-plugin/plugin.json:3`)
- License: MIT. Primary language: TypeScript.

---

## 1. Identity / disambiguation

The user said "oh my claudecode". There are ~30 repos matching that string. The canonical one is
**`Yeachan-Heo/oh-my-claudecode`** (38,933 stars). Everything else in the namespace is either a
mirror, a fork, or a port of it:

| Repo | Stars | What it is |
|---|---|---|
| **Yeachan-Heo/oh-my-claudecode** | **38,933** | **THE ONE.** Teams-first multi-agent orchestration for Claude Code |
| Yeachan-Heo/oh-my-claudecode-website | 37 | its docs site |
| witt3rd/oh-my-hermes | 302 | port of OMC to Hermes Agent |
| MeroZemory/oh-my-droid | 28 | port of OMC to Factory Droid CLI |
| dasomel/oh-my-cursor, hongvincent/oh-my-warp, namojo/oh-my-harness | <5 | ports |
| 2233admin/oh-my-claudecode-RS | 3 | **Rust rewrite of OMC's statusline** — directly relevant to us |
| 2lab-ai/oh-my-claude, stefandevo/oh-my-claude | 36 / 7 | unrelated small plugins |
| SleepyLGod/oh-my-claude-code | 2 | unrelated fork of the CC CLI |
| vyvhouse/oh-my-destructor | 29 | **a third-party uninstaller for OMC** (see §7 — this exists because OMC's own uninstall is broken) |

**Near-equivalents / rivals** (real numbers via `gh repo view`, 2026-09-01):

| Project | Stars | Forks | Last push | Shape |
|---|---|---|---|---|
| davila7/claude-code-templates | 30,482 | 3,463 | 2026-09-01 | CLI + a **content catalogue**: 435 agent files, 346 command files, 89 hooks, 101 MCP configs, 73 settings presets, 5,659 skill files (`cli-tool/components/*`). Breadth over depth; it is a *registry*, not a framework. |
| SuperClaude-Org/SuperClaude_Framework | 23,856 | 2,013 | 2026-08-21 | Python installer (`install.sh` → `~/.claude/commands/`), v4.3.0, ~46 agent markdown files, personas + commands. Pure prompt-asset shipping, no runtime. |
| sangrokjung/claude-forge | 821 | 176 | 2026-08-29 | Explicitly brands itself "oh-my-zsh for Claude Code". 16 agents / 35 commands / 135 skill files / 32 hooks. **Installs by symlinking the clone into `~/.claude/`** (`install.sh:282-317`) and has a genuine overlay dir (`cc-chips-custom/`). Interesting install model, tiny compared to OMC. |
| carlrannaberg/claudekit | 761 | 117 | 2026-03-31 | Hooks/commands toolkit. Effectively dormant (5 months since push). |
| baekenough/oh-my-customcode | 34 | 6 | 2026-08-31 | "oh-my-zsh style customization framework". 50 agents / 115 skills / 23 rules. Interesting *idea* ("agents are compiled from skills"), no traction, PolyForm-NC license. |
| huangdijia/oh-my-claude-code-plugins | 11 | 0 | 2026-03-31 | A third-party Claude plugin marketplace. Dead. |

**Canonical = Yeachan-Heo/oh-my-claudecode.** claude-code-templates is a bigger *catalogue* but is a
different product category (a content index + a CLI that copies files). OMC is the only one that has
an actual runtime, a lifecycle model, and provenance-based file ownership — which is what we need.

---

## 2. TRACTION (real numbers, `gh` / GitHub API / npm API, 2026-09-01)

- **Stars: 38,933**, forks: 3,492, watchers/subscribers: 131
- Created **2026-01-09**; last push **2026-09-01T09:34Z** (today — extremely active)
- **4,512 commits**, **150 contributors** (from `Link: rel="last"` on paginated API, `per_page=1`)
- **2,409 PRs**, 1,414 issues, **only 1 open issue** (aggressive triage, or aggressive closing)
- **100+ GitHub releases**; **252 npm versions**; latest `v5.1.0` (2026-08-31)
- npm downloads: **24,628 last month**, 5,759 last week
- npm `unpackedSize`: **41,084,384 bytes across 5,303 files** — for a "config framework"
- Repo disk usage: 91,616 KB. Working tree: 6,812 tracked files, 581 non-test `.ts` files,
  **716 test files** (test-to-source ratio > 1, which is unusual and good)
- Discord, sponsors, 12 translated READMEs, a named ambassador/maintainer table

Growth from 0 → 38.9k stars in under 8 months. This is the reference point for the category.

---

## 3. WHAT IT SHIPS (real counts, `find | wc -l`)

| Category | Count | Path |
|---|---|---|
| Subagent definitions | **19** `.md` | `agents/*.md` (analyst, architect, code-reviewer, code-simplifier, critic, debugger, designer, document-specialist, executor, explore, git-master, planner, qa-tester, scientist, security-reviewer, test-engineer, tracer, verifier, writer) — 4.4 KB–22 KB each, XML-tagged prompts with `<Role>`, `<Why_This_Matters>` sections |
| Slash commands | **21** `.md` | `commands/*.md` — but these are ~600-byte **dispatch shims** (see §9), not content |
| Skills | **35** `SKILL.md` (71 files total) | `skills/<name>/SKILL.md` — 1.1 KB to **59 KB** (`team`), plus `lib/`, `templates/`, `phases/`, `scripts/`, `tests/` subtrees |
| Hook scripts | **24** distinct `.mjs` wired across **11 hook events / 25 registrations** | `hooks/hooks.json` + `scripts/*.mjs` |
| Hook templates (standalone install) | 9 `.mjs` + 12 `lib/*.mjs` | `templates/hooks/` |
| Rule packs | **7** `.md` | `templates/rules/` (coding-style, git-workflow, testing, performance, security, karpathy-guidelines, README) |
| MCP servers | **1** declared (`t`), exposing **~56 custom tools** | `.mcp.json` → `bridge/mcp-server.cjs`; tool names in `src/tools/*.ts` (ast_grep_*, lsp_* ×12, notepad_* ×5, project_memory_* ×4, shared_memory_* ×5, state_* ×6, wiki_* ×4, trace_*, session_search, list_omc_skills, load_omc_skills_local/global) |
| Statusline | **1** (the "HUD") | `src/hud/` → installed as `~/.claude/hud/omc-hud.mjs` |
| Settings presets | **0** shipped as presets | it *mutates* `~/.claude/settings.json` in place instead |
| Output styles | **0** | `find -type d -name 'output-style*'` → 0 |
| Injected system prompt | 1, 73 lines | `docs/CLAUDE.md` — merged into `~/.claude/CLAUDE.md` |
| Docs | 48 `.md` | `docs/` incl. 3 ADRs |
| Pre-bundled runtime | ~7.5 MB of esbuild megabundles | `bridge/cli.cjs` (4.77 MB), `bridge/team.js` (885 KB), `bridge/runtime-cli.cjs` (832 KB), `bridge/team-mcp.cjs` (747 KB), `bridge/mcp-server.cjs` (1.18 MB), `bridge/claude-md-coordinator.cjs` (48 KB) |

Marketing claims "28 agent variants, 35 skills" (`.claude-plugin/marketplace.json:4`). The 35 skills
is accurate; **19** agent markdown files exist on disk and **20** agent names are typed in
`src/shared/types.ts:KNOWN_AGENT_NAMES` (the extra is `omc`, the orchestrator itself). "28" is stale.

---

## 4. INSTALL — what it actually writes

Two supported paths, and OMC goes to real trouble to make them coexist.

### Path A (recommended): Claude Code plugin marketplace
```
/plugin marketplace add https://github.com/Yeachan-Heo/oh-my-claudecode
/plugin install oh-my-claudecode
```
The repo root *is* the marketplace: `.claude-plugin/marketplace.json` declares one plugin whose
`"source": "./"`. `.claude-plugin/plugin.json` enumerates all 35 skill dirs explicitly, plus
`"commands": "./commands/"` and `"mcpServers": "./.mcp.json"`. Claude Code clones it to
`~/.claude/plugins/cache/omc/oh-my-claudecode/` (`src/lib/paths.ts:OMC_PLUGIN_CACHE_REL`) and OMC
never copies anything into `~/.claude/agents|skills` in this mode — everything resolves through
`${CLAUDE_PLUGIN_ROOT}`.

### Path B: npm global CLI
`npm i -g oh-my-claude-sisyphus@latest`, then `omc setup` (or `/omc-setup` in-session).

### The installer: `src/installer/index.ts` (2,818 lines, `install()` at :2357)

Path constants (`src/installer/index.ts:39-46`), rooted at `getClaudeConfigDir()` (`$CLAUDE_CONFIG_DIR`
or `~/.claude`):

```
~/.claude/agents/*.md                      19 agent files   (only if plugin isn't providing them)
~/.claude/skills/<name>/SKILL.md           35 skill dirs    (+ .omc-managed marker in each)
~/.claude/hooks/*.mjs  + hooks/lib/*.mjs   24 + 12 scripts  (standalone mode only)
~/.claude/hud/omc-hud.mjs                  statusline, chmod 0755
~/.claude/hud/lib/config-dir.mjs
~/.claude/hud/find-node.sh
~/.claude/hud/omc-hud-cache.sh
~/.claude/settings.json                    MUTATED: .hooks.* entries + .statusLine
~/.claude/CLAUDE.md                        MUTATED: managed block, backup written first
~/.claude/.omc-version.json                {version, installedAt, installMethod, lastCheckAt}
~/.claude/.omc-config.json                 {nodeBinary, setupVersion, hudEnabled}
~/.claude/.omc-silent-update.json          auto-update state
~/.claude/skills/omc-learned/*.md          user-authored "learned" skills live here
```
Plus, off the Claude dir entirely:
```
~/.config/omc/  (XDG) or ~/.omc/            global config root  (src/utils/paths.ts:96-107)
~/.config/omc/mcp-registry.json             unified MCP registry
~/.local/state/omc/ or ~/.omc/state/        global state root
~/.omc/skills/*.md                          global learned skills
~/.omc/rules-injector/                      rules-injector state
<repo>/.omc/                                per-worktree state: plans, notepad.md,
                                            project-memory.json, research/, drafts/, logs/,
                                            state/sessions/, autopilot/, skills/,
                                            deepinit-manifest.json, template-version.json
```
And — notably — it reaches into **other agents' configs**
(`src/installer/mcp-registry.ts:74-83`, called from `install()` via `syncUnifiedMcpRegistryTargets`):
```
~/.claude.json                              MCP servers synced in
~/.codex/config.toml                        MCP servers synced in, wrapped in
                                            "# BEGIN OMC MANAGED MCP REGISTRY" markers
```

### Notable installer behaviours
- **`~/.claude/CLAUDE.md` is written through a real transaction** (`src/installer/claude-md-transaction.ts`,
  called at `index.ts:2617`): verified `O_EXCL` backup with readback comparison
  (`exclusiveVerifiedBackup`, :156-168), atomic temp-file + `rename` (`atomicWrite`, :171-177),
  symlink/alias target validation, and full **rollback with a distinct exit code** (5 = rolled back
  cleanly, 6 = rollback itself failed). Managed content goes at the top between
  `<!-- OMC:START -->` / `<!-- OMC:END -->` with a `<!-- OMC:VERSION:5.1.0 -->` stamp; everything
  else in the file is preserved verbatim beneath a `<!-- User customizations -->` header.
- **Downgrade guard**: if the on-disk version marker is newer than the CLI package, `install()`
  returns early with "run omc update first" rather than silently regressing (`index.ts:2384-2393`).
- **No npm `postinstall`** in `package.json`. There is a hidden `omc postinstall` command
  (`src/cli/index.ts:1403`) but nothing invokes it from npm. Install is explicit. Good.
- `checkNodeVersion()` gates on Node 20+; `resolveNodeBinary()` persists the detected node path
  into `.omc-config.json` so hooks work under nvm/fnm where `node` isn't on the hook's PATH.

---

## 5. EXTENSION CONTRACT — how a user adds/overrides without forking

This is the part most relevant to us, and OMC's answer is **strong on additive extension, weak on
override, and closed for new agents.**

### Additive surfaces that work (no fork required)

1. **Learned skills — the real extension point.** Flat markdown files with YAML frontmatter, picked
   up at four search roots (`src/hooks/learner/constants.ts:11-22`, `src/hooks/learner/bridge.ts:25-34`):
   ```
   ~/.claude/skills/omc-learned/<name>.md       user-level
   ~/.omc/skills/<name>.md                      global (preferred going forward)
   <project>/.omc/skills/<name>.md              project-level
   <project>/.agents/skills/<name>.md           read-only compat source
   ```
   Frontmatter contract (`skills/skillify/SKILL.md:36-44`): `name`, `description`, `triggers: []`.
   The `skill-injector.mjs` UserPromptSubmit hook fuzzy-matches triggers (Levenshtein, cached) and
   injects matching skills — max 10/session, 4,000 chars each, min quality score 50.
   **`/skillify` is a first-class skill whose entire job is to author one of these from the current
   session.** That is the loop that makes the extension point actually get used.

2. **Rules — host-neutral, and it reads other tools' conventions.**
   `src/hooks/rules-injector/constants.ts:27-36` searches:
   ```
   .claude/rules/*.md|.mdc
   .cursor/rules/*.md|.mdc              <- Cursor's convention
   .github/instructions/*.instructions.md   <- Copilot's convention
   .github/copilot-instructions.md
   ```
   injected on `read|write|edit|multiedit`. 7 starter packs in `templates/rules/`.

3. **`~/.claude/CLAUDE.md` below the `<!-- User customizations -->` header** — preserved across
   every reinstall by the transaction described above.

4. **Config files**, JSONC, two levels (`src/config/loader.ts:1-9`):
   ```
   ~/.config/claude-omc/config.jsonc      user
   <project>/.claude/omc.jsonc            project (wins)
   ```
   plus env vars. Surface: per-agent model overrides, 5 feature toggles, model routing tiers +
   escalation, MCP enable/disable, team `roleRouting`, `autopilot.workflows` named stage profiles,
   `magicKeywords`, `keywordDetector.disabled`, `security.*`, `permissions.*`.

5. **User hooks survive.** `mergeHookGroups()` (`src/installer/index.ts` ~:960) classifies every hook
   command via `isOmcHook()` (:454-471 — matches `omc` as a path segment, `oh-my-claudecode`, or a
   known OMC hook filename under a `hooks/` dir). Non-OMC hooks on an event cause OMC to **skip that
   event entirely** and log a conflict, unless you pass `--force-hooks`.

### Where the contract breaks down

- **There is no override mechanism for shipped skills or agents.** No `custom/` overlay dir, no
  "enabled modules" list, no priority chain. `PluginConfig.agents` (`src/shared/types.ts`) is a
  **hard-coded struct of exactly 20 named keys**, and `KNOWN_AGENT_NAMES` is a `const` tuple. You
  cannot register a new agent, disable a shipped skill, or replace an agent prompt through config.
- The *de facto* override is: drop an edited copy into `~/.claude/agents/<name>.md` and rely on
  OMC's cleanup refusing to delete it (§6). That works, but it is emergent behaviour, not a
  documented contract, and precedence vs. the plugin copy is Claude Code's business, not OMC's.
- `keywordDetector.disabled: []` lets you stop a skill from *auto-triggering*. It does not unload it.
- **Compare claude-forge**, which does have an explicit overlay (`cc-chips-custom/` patched over
  `cc-chips/` at install time) — a cruder but more honest override contract.

---

## 6. UPDATE — and how it avoids clobbering you

- Plugin path: `/plugin marketplace update` (Claude Code's own mechanism).
- npm path: `omc update` (`src/cli/index.ts:734`) → `src/features/auto-update.ts`, which shells
  `npm install -g oh-my-claude-sisyphus@latest` via `execFileSync` with an
  `assertSafeNpmPackageSpec()` allowlist regex (`auto-update.ts:61-73`) and a 120 s timeout, then
  re-runs `install()`.
- There is also a **silent background auto-update** with state at `~/.claude/.omc-silent-update.json`
  (`auto-update.ts:1355`), disableable via `security.disableAutoUpdate` / `OMC_SECURITY=strict`.
- `omc update-reconcile` (`src/cli/index.ts:819`) exists specifically to repair the post-update state.

### Version/lockfile artifacts
- `~/.claude/.omc-version.json` — installed version metadata
- `<repo>/.omc/template-version.json` — per-worktree stamp, used by `session-start.mjs` for drift detection
- `<!-- OMC:VERSION:x.y.z -->` — marker inside `~/.claude/CLAUDE.md`
- `omc capabilities lock` / `omc capabilities check` (`src/cli/commands/capabilities.ts`) — writes
  `omc-capabilities.lock.json`: a schema-versioned digest of the **entire tool/agent/skill surface**
  (tool JSON-schemas, agent tool allowlists + models, per-skill SHA digests) plus a `surfaceDigest`.
  This is a genuine capability lockfile and is the closest existing analogue to Muse's
  `.muse/skills.lock`.

### The clobber-avoidance mechanism — this is the crown jewel

OMC never deletes a file it cannot *prove* it wrote:

1. **`.omc-managed` marker files.** Every skill dir OMC installs gets a `.omc-managed` sentinel
   (`markSkillAsOmcManaged`, `index.ts:2173`). `cleanupStaleSkills()` (:1142-1200) refuses to remove
   any skill dir lacking it, and hard-skips `omc-learned/`.
2. **Content-addressed historical ownership.** `src/installer/historical-agent-ownership.ts` is a
   47 KB generated table of **202 records** — every `agents/*.md` byte-image ever shipped in v4.0.0
   through v4.15.7, each as `{filename, byteLength, sha256, gitBlob, firstReleaseTag, lastReleaseTag}`.
   `cleanupStaleAgents()` / `prunePluginDuplicateAgents()` (`index.ts:1066-1135`) delete a stale
   agent **only if its bytes hash to an authenticated historical release artifact**. The file header
   says it out loud: *"Do not replace this with filename, frontmatter, or fuzzy ownership heuristics."*
   Every record is schema-validated at module load (`isValidHistoricalAgent`, :83-97).
3. **TOCTOU guard.** Between the hash check and the `unlink`, `hasUnchangedRegularAgentFile()`
   re-stats and compares `dev`/`ino`/`size`/`mtimeMs` (:110-121).
4. Duplicate pruning between plugin and standalone copies is likewise content-hash gated
   (`prunePluginDuplicateSkills`, :1211).

**If a user edited one byte of `~/.claude/agents/executor.md`, OMC will never touch it again.** That
is exactly the right guarantee and almost nobody implements it.

---

## 7. UNINSTALL — genuinely bad, and the one clear failure

`scripts/uninstall.sh` (the only uninstaller; there is **no `omc uninstall` CLI command** —
`grep -rn "uninstall" src/cli/` returns nothing) is **badly stale, roughly at v3-era**:

- Removes **10 hard-coded agent filenames** — but ships **19**. `code-reviewer.md`, `debugger.md`,
  `git-master.md`, `qa-tester.md`, `scientist.md`, `security-reviewer.md`, `test-engineer.md`,
  `tracer.md`, `verifier.md`, `code-simplifier.md` are all left behind. It also removes
  `vision.md`, an agent that no longer exists.
- Removes **3 skill dirs** (`ultrawork`, `git-master`, `frontend-ui-ux`) — none of which are in the
  current 35. All 35 shipped skills survive an "uninstall".
- Removes **`.sh` hooks** (`keyword-detector.sh`, `stop-continuation.sh`, `silent-auto-update.sh`).
  Bash hooks were deleted in v3.9.0 — the header of `src/installer/index.ts:7` says so. The 24
  current `.mjs` hooks and `hooks/lib/` are never removed.
- Its `jq` settings.json surgery filters only for those three `.sh` filenames on only the
  `UserPromptSubmit` and `Stop` events. The other 9 events, and every `.mjs` entry, remain — so
  **settings.json is left pointing at hook scripts that may or may not still exist**.
- Never touches `~/.claude/hud/`, never resets `.statusLine`, never removes `~/.config/omc/`,
  `~/.omc/`, the OMC block injected into `~/.codex/config.toml`, or `~/.claude/CLAUDE.md`.
- Requires `jq`; degrades to "please hand-edit your settings.json".
- Leaves `settings.json.bak` behind and tells you so.

The existence of a third-party **`vyvhouse/oh-my-destructor` — "Safe uninstaller for Oh My Claude
Code" (29 stars)** is the market's verdict on this file.

The irony is total: OMC has the most sophisticated *install-time* provenance system in the category
(§6) and does not use one byte of it at uninstall time. `cleanupStaleAgents()` + the 202-record
ownership table + `.omc-managed` markers is *already* a correct uninstaller — nobody wired it up.

---

## 8. REGISTRY — none

- `.claude-plugin/marketplace.json` is a **single-entry marketplace containing only OMC itself**
  (`"source": "./"`). It exists to make `/plugin marketplace add <repo-url>` work, not to index
  anything.
- There is no submission process, no third-party plugin index, no schema for community skills, no
  search. Discovery of community additions is entirely ad hoc (Discord + the README).
- The closest thing to a registry in this ecosystem is **claude-code-templates**' `components/`
  tree (5,659 skill files, 435 agents, 101 MCP configs) — but that is a monorepo catalogue, not a
  federated registry either.
- Contrast: the *host* (Claude Code) owns marketplaces, so OMC deliberately doesn't build one. It
  builds an install target for the host's registry instead. That's the right call and worth copying.

---

## 9. GOOD DESIGN / BAD DESIGN — opinionated

### Genuinely good, steal these

1. **Content-addressed ownership for destructive operations** (§6). 202 sha256+blob records so the
   installer can *prove* a file is an unmodified artifact it shipped before deleting it, plus a
   dev/ino/size/mtime TOCTOU recheck. This is the single best idea in the repo.
2. **The `.omc-managed` sentinel.** Cheap, obvious, and it makes "is this mine?" a filesystem
   question instead of a heuristic. Complements #1 for directories.
3. **Marker-delimited managed regions in shared files** (`<!-- OMC:START -->` / `<!-- OMC:END -->`
   in `CLAUDE.md`, `# BEGIN/END OMC MANAGED MCP REGISTRY` in `~/.codex/config.toml`). One file,
   two owners, no fight. The version stamp inside the marker makes drift detectable without a
   separate lockfile.
4. **A real transaction for config mutation** (`claude-md-transaction.ts`): verified exclusive
   backup with readback, atomic temp+rename, path/symlink validation against a captured root, full
   rollback, distinct exit codes for "rolled back" vs "rollback failed". Most installers in this
   category do `writeFileSync` and hope.
5. **The compact-shim pattern for context economy.** Two independent instances:
   - `commands/*.md` are ~600-byte dispatch stubs that say "read `skills/<n>/SKILL.md` and follow it
     exactly, `$ARGUMENTS` are the args" — so 21 slash commands cost ~13 KB of always-loaded
     description instead of ~450 KB.
   - `compactPluginSkillPayload()` (`index.ts:1838-1894`) rewrites every installed `SKILL.md` into a
     shim with a ≤240-char description + `omc-full-body:` frontmatter pointer, archiving the real
     body to `<plugin-root>/skill-bodies/<name>/SKILL.md`.
   Lazy-loading prompt assets is the central scaling problem for any framework that ships 35+ skills.
6. **Host-neutral rule discovery.** Reading `.cursor/rules/` and `.github/instructions/` costs
   nothing and means adopting OMC doesn't require re-authoring your existing agent instructions.
7. **A capability lockfile** (`omc capabilities lock/check`) that digests tools + agents + skills
   into one `surfaceDigest`. Turns "did my agent's abilities change?" into a CI check.
8. **Unified MCP registry projected into multiple hosts.** One `~/.config/omc/mcp-registry.json`,
   synced into both `~/.claude.json` and `~/.codex/config.toml`. Config is authored once and
   *projected*; hosts are render targets.
9. **A coherent security posture with a fail-safe master switch.** `OMC_SECURITY=strict`
   (`src/lib/security-config.ts`), where the config file can only *tighten* — booleans OR'd, numeric
   caps `Math.min`'d. It includes `disableProjectSkills`, correctly identifying repo-supplied
   `.omc/skills/*.md` as an untrusted prompt-injection surface. Skill content is sanitized
   (role-boundary tags stripped, 4 KB truncation) and `projectRoot` is boundary-validated
   (`src/tools/skills-tools.ts:16-56`).
10. **Supply-chain seriousness.** `scripts/release-boundary.mjs` verifies the published npm tarball
    against **SLSA v1 / in-toto provenance attestations**, pins the builder ID and workflow path,
    and asserts the exact bin map and entrypoint set. CI has a `generated-artifact-authorization`
    workflow and owner-signed release heads. For a project that writes to `~/.claude/settings.json`
    on 24k installs/month, this is proportionate.
11. **`/skillify`.** Shipping a skill whose purpose is to author user skills is how you make an
    extension point actually get used. Most frameworks document their extension point and wonder
    why nobody uses it.

### Bad, don't copy

1. **The uninstaller** (§7). Stale by two major versions, leaves the majority of installed files and
   all statusline/settings wiring behind, and has spawned a third-party cleanup tool.
2. **41 MB / 5,303 files unpacked for a config framework.** ~7.5 MB of that is five separate esbuild
   megabundles in `bridge/` that all overlap (`cli.cjs` 4.77 MB + `runtime-cli.cjs` 832 KB +
   `mcp-server.cjs` 1.18 MB + `team-mcp.cjs` 747 KB + `team.js` 885 KB). The npm package also ships
   the entire `docs/` tree. `better-sqlite3` drags a native addon (and a deprecation warning the
   README has to apologise for) into a global CLI install.
3. **A closed agent roster.** `KNOWN_AGENT_NAMES` is a `const` tuple and `PluginConfig.agents` is 20
   literal keys. Adding an agent means forking. For a framework explicitly named after oh-my-zsh —
   whose entire value proposition is a plugin/theme directory anyone can drop into — this is the
   central design failure. Skills are extensible; agents, the thing users most want to customise,
   are not.
4. **No override/disable chain.** You can add, you cannot replace or turn off. There is no
   `enabled: []`, no overlay dir, no precedence rule. `keywordDetector.disabled` only mutes
   auto-triggering.
5. **Sprawling state topography.** `~/.claude/` + `~/.config/omc/` + `~/.local/state/omc/` + `~/.omc/`
   (legacy) + `<repo>/.omc/` + `~/.claude.json` + `~/.codex/config.toml`, with `OMC_HOME`,
   `OMC_STATE_DIR`, `CLAUDE_CONFIG_DIR`, `XDG_*` and legacy-path fallback candidate lists layered on
   top (`src/utils/paths.ts:128-170`). Every path has 2-3 candidate locations. The complexity is
   *earned* — it's migration debt — but it is the thing to design away from, not toward.
6. **Hooks that skip rather than compose.** If any non-OMC hook exists on an event, OMC declines to
   install its own hook there and logs a conflict. Correct-but-unhelpful: coexistence should be
   ordering, not abdication.
7. **The identity mess.** Repo `oh-my-claudecode`, npm `oh-my-claude-sisyphus`, plugin
   `oh-my-claudecode`, marketplace slug `omc`, CLI `omc`. Four names for one product.
8. **Stale marketing in the manifest.** `marketplace.json` still advertises "28 agent variants"
   against 19 files. The build system verifies skill entitlements, prompt projections, and inventory
   graphs — but not its own storefront copy.
9. **Dead scaffolding in the tree.** `src/config/builtin-skill-entitlements.json` gates skills on
   `isSkininthegamebrosUser()` → `process.env.USER_TYPE === 'ant'`, with an **empty** list. An
   entitlement system with no entitlements, plus a plaintext env-var check as its authorization
   primitive.
10. **`~/.claude/CLAUDE.md` reordering.** Every install rewrites the file as `[managed block]` then
    `[<!-- User customizations -->][your content]`. Safe, but if your content was above the block
    it silently moves below it, every single upgrade.

---

## 10. TRANSFER TO A COMPILED RUST BINARY HOST (Muse Code)

### Transfers essentially unchanged — these are format/lifecycle ideas, not JS

| OMC mechanism | Muse equivalent |
|---|---|
| Content-addressed historical ownership (202 sha256 records) | Bake a generated `ownership.rs` (or a signed sidecar) into the `oh-my-musecode` binary. Refuse to remove any managed file whose bytes don't hash to a shipped release artifact. Muse's `.muse/lock.json` already carries provenance/quarantine — this is the deletion half of that story. |
| `.omc-managed` sentinel | `.omm-managed` in every managed skill/rule dir. Trivial, works identically. |
| `<!-- OMC:START -->` / `<!-- OMC:END -->` marked regions | Same technique for any shared file (`AGENTS.md`, `.muse/hooks.json`, `~/.config/muse/*`). Rust's `serde_json` + `jsonc-parser` equivalents preserve enough to do this safely; for JSON use a `_managedBy`/`_managedRange` key rather than comments. |
| Verified-backup + atomic temp+rename + rollback transaction | Strictly *easier* in Rust: `tempfile::NamedTempFile::persist`, `fs2`/`rustix` for `O_EXCL`, real error types instead of exit-code encoding. |
| Capability lockfile (`omc capabilities lock`) | Direct fit for `.muse/skills.lock`. Digest the resolved tool/agent/skill surface, one `surfaceDigest`, `omm lock` / `omm check` in CI. Muse's `allowed_tools` in `lock.json` is the same idea from the security side. |
| Unified registry projected into multiple hosts | The killer feature for us. Author once in `~/.config/omm/`, project into `.muse-plugin/`, `.claude-plugin/`, `.codex-plugin/`. **Muse's loader already reads all three manifest dirs** — so one `oh-my-musecode` install can legitimately serve Muse Code, Claude Code, and Codex from one source of truth. OMC only proved this for MCP servers; Muse's loader makes the full-plugin version possible. |
| Compact skill shims + lazy full bodies | Even more valuable with a compiled host: emit ≤240-char frontmatter descriptions at build time, keep bodies in `skill-bodies/`, load on invocation. |
| Host-neutral rule discovery (`.cursor/rules`, `.github/instructions`) | Copy the search list verbatim. Zero-cost adoption lever. |
| `OMC_SECURITY=strict` tighten-only precedence | Maps onto Muse's quarantine + `allowed_tools`; a Rust enum-typed config makes "config can only tighten" a compile-checked lattice instead of a `||`/`Math.min` convention. |
| SLSA/provenance verification of the published artifact | Better in Rust: ship a signed binary, `cargo-dist` + attestations, verify the plugin payload's manifest digest on load. |
| Skill frontmatter contract (`name` / `description` / `triggers`) + a `/skillify` authoring skill | Pure markdown. Transfers 1:1. Ship the authoring skill on day one. |
| Downgrade guard (refuse to install over a newer marker) | 1:1. |
| Marker-delimited managed block in `AGENTS.md`/`CLAUDE.md` equivalent | 1:1, and Muse already has a rules subsystem to inject into. |

### Does NOT transfer

- **Everything about the Node runtime.** `bin/oh-my-claudecode.js`, the five `bridge/*.cjs`
  megabundles, `resolveNodeBinary()`, `find-node.sh`, the nvm/fnm PATH-detection workarounds, the
  `checkNodeVersion()` gate, `better-sqlite3`'s native addon, `prebuild-install` deprecation
  warnings — all of it exists to solve "can this hook find a working `node`?". A compiled binary
  deletes this entire class of problem, and with it maybe 15% of OMC's installer complexity.
- **`hooks.json` commands shaped as `node "$CLAUDE_PLUGIN_ROOT"/scripts/run.cjs <script>.mjs`.**
  25 process spawns of a JS interpreter across 11 events, with 3–60 s timeouts each, is why OMC has
  a `templates/hooks/lib/cache-occupancy.mjs` and CI perf tests with "p50/p95 lock ceilings". For
  Muse: hooks should be `omm hook <name>` invoking the same static binary (sub-5 ms cold start —
  which is precisely what `2233admin/oh-my-claudecode-RS` was built to prove), or in-process if
  Muse's hook host allows it.
- **npm as the distribution channel** — global install, `npm install -g …@latest` for self-update,
  `~/.npm-global` PATH problems, semver ranges on 12 runtime deps. Replace with a single binary
  (curl|sh, brew, cargo-binstall) whose *content* payload is versioned separately from the binary.
- **The esbuild/tsc/`compose-docs`/`generate-prompt-projections`/`build-skill-bridge` pipeline**
  (12 build steps in `package.json:scripts.build`). In Rust, `include_dir!`/`include_str!` embeds
  the markdown corpus into the binary at compile time — one artifact, no payload-sync repair logic,
  no `validatePluginCachePayload()`, no `copyPluginSyncPayload()`, no `repair-plugin-cache.mjs`.
- **`syncInstalledPluginPayload()` / plugin-cache repair** (~600 lines across `index.ts:1400-1990`).
  This entire subsystem exists because the npm package and the Claude Code plugin cache are two
  copies of the same content that drift. A single binary with embedded assets has one copy.
- **`isRunningAsPlugin()` / `isProjectScopedPlugin()` / `--plugin-dir-mode` / `--no-plugin` and the
  resulting `shouldInstallLegacyAgents` × `shouldInstallBundledSkills` × `allowPluginHookRefresh`
  matrix** (`index.ts:2398-2430`). Four install topologies that must not duplicate each other.
  **Pick exactly one for `oh-my-musecode` and never add a second.** This is OMC's largest
  self-inflicted wound and the easiest one to avoid.

### The design lesson to carry over

OMC's install-time correctness is excellent and its uninstall-time correctness is absent, because
ownership tracking was built reactively (issue #2252: duplicate agents) rather than as the
foundation. For `oh-my-musecode`, make **the ownership ledger the primitive**: every managed path
gets an entry (path, sha256, source version, scope) in `.muse/`-adjacent state at write time, and
*install, update, reconcile, doctor, and uninstall are all the same traversal over that ledger*.
Muse already gives us `.muse/lock.json` as the place to put it. OMC needed 202 hand-generated
history records precisely because it lacked that ledger from day one.

And: **fix the two things OMC left open.** Ship an override chain (an overlay dir plus an
`enabled`/`disabled` list, so a user can replace a shipped agent prompt or turn a skill off without
forking), and make the agent roster open — data-driven, not a `const` tuple in the type system.
Those are the two places where a 38.9k-star project named after oh-my-zsh isn't actually oh-my-zsh.

---

## Verification

**Verdict: SOLID.** Independent re-clone (`git clone --depth 50` into
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-oh-my-claude-code/omc`,
HEAD `e9e8fa3`, v5.1.0 — same commit as the original teardown). Every load-bearing claim was
re-derived from the source tree, the GitHub API and the npm registry. Nothing was refuted.

### Priority 1 — repo exists, traction is real

`gh repo view Yeachan-Heo/oh-my-claudecode` returns, byte-for-byte against the report:
stars **38,933**, forks **3,492**, watchers **131**, created **2026-01-09T03:36:29Z**,
pushed **2026-09-01T09:34:10Z**, diskUsage **91,616 KB**, MIT, not archived.
Paginated API: **4,512** commits (`Link rel="last"`, `per_page=1`), **2,409** PRs,
**1,414** issues, **1** open. npm registry: **252** versions, latest **5.1.0**,
`unpackedSize` **41,084,384**, `fileCount` **5,303**; downloads **24,628**/month and
**5,759**/week (`api.npmjs.org`). Sibling repos re-checked: `vyvhouse/oh-my-destructor` **29**,
`witt3rd/oh-my-hermes` **302**, `MeroZemory/oh-my-droid` **28**. No inflation anywhere.

### Priority 2 — shipped-content counts, re-run

| Claim | Re-measured | |
|---|---|---|
| 19 agents | `ls agents/*.md` → **19** (same 19 names) | ✅ |
| 21 commands, ~600 B shims | **21** files, **13,895 B** total, **avg 661 B** | ✅ |
| 35 skills / 71 files | `find skills -name SKILL.md` → **35**; `-type f` → **71** | ✅ |
| skill size 1.1 KB–59 KB (`team`) | min `skills/verify` **1,109 B**, max `skills/team` **59,105 B** | ✅ |
| 24 hooks / 11 events / 25 registrations | parsed `hooks/hooks.json`: **11** events, **25** registrations, **24** distinct `.mjs` | ✅ exact |
| 9 hook templates + 12 lib | **9** + **12** | ✅ |
| 7 rule packs | **7** | ✅ |
| 1 MCP server named `t` | `.mcp.json` → single server `t` | ✅ |
| 48 docs, 103 scripts files, 6,812 tracked | **48**, **103**, **6,812** | ✅ |
| 581 non-test `.ts` | `find src -name '*.ts' ! -name '*.test.ts'` → **581** | ✅ |
| bridge megabundles | cli **4,770,311**; mcp-server **1,179,351**; team **884,964**; runtime-cli **832,144**; team-mcp **747,400** | ✅ |
| `docs/CLAUDE.md` 73 lines | **73** | ✅ |
| marketplace.json "28 agent variants" vs 19 | confirmed verbatim in `.claude-plugin/marketplace.json:11` | ✅ |

### Priority 3 — the extension contract is implemented, not aspirational

Every mechanism was read at the code path that honours it:

- **Learned-skill search roots** — `src/hooks/learner/constants.ts:11-21` defines exactly the four
  claimed roots (`~/.claude/skills/omc-learned`, `~/.omc/skills`, `<project>/.omc/skills`,
  `<project>/.agents/skills`), mirrored in `bridge.ts:25-35`. Limits confirmed literally:
  `MAX_SKILLS_PER_SESSION = 10`, `MAX_SKILL_CONTENT_LENGTH = 4000`, `MIN_QUALITY_SCORE = 50`
  (`constants.ts:35,38,44`), with an LRU Levenshtein cache (`bridge.ts:56-73`).
- **Host-neutral rules** — `rules-injector/constants.ts:27-36`: `PROJECT_RULE_SUBDIRS` is
  `[.github/instructions, .cursor/rules, .claude/rules]` plus `.github/copilot-instructions.md`. Real.
- **Config** — `src/config/loader.ts:1-9` documents `~/.config/claude-omc/config.jsonc` and
  `.claude/omc.jsonc`. Real.
- **Closed roster** — `src/shared/types.ts:485-506` `KNOWN_AGENT_NAMES` is a 20-element
  `as const` tuple; `PluginConfig.agents` at `:72-93` is exactly 20 literal `{ model?: string }`
  keys. The report's central criticism is verified at the type level.
- **Hooks abdicate** — `mergeHookGroups()` (`index.ts:955`) ends with
  `"Warning: ${eventType} hook has non-OMC hook. Skipping. Use --force-hooks to override."` and
  returns `existingGroups` unchanged. Exactly as described.
- **Ownership** — `historical-agent-ownership.ts` is **47,868 bytes** with **202** records
  (202 `filename:` keys, 202 `sha256` fields) and carries the quoted header at line 8:
  *"Do not replace this with filename, frontmatter, or fuzzy ownership heuristics."*
  `cleanupStaleAgents()` (`:1066-1098`) deletes only when
  `hasAuthenticatedHistoricalAgentBytes(file, content)` passes **and** the
  `dev/ino/size/mtimeMs` recheck (`:110-125`) still matches. `cleanupStaleSkills()` (`:1142+`)
  contains a literal `if (entry.name === 'omc-learned') continue;` and an
  `if (!isOmcManagedSkillDir(skillDir)) continue;`. All four claims verified.
- **Security lattice** — `src/lib/security-config.ts:95-108`: under `OMC_SECURITY === "strict"`,
  every boolean is `base.X || (fileOverrides.X ?? false)` and `hardMaxIterations` is `Math.min(...)`.
  "Config can only tighten" is literally implemented, `disableProjectSkills` included.
- **Sanitization** — `src/tools/skills-tools.ts` validates `projectRoot` against allowed boundary
  dirs, strips role-boundary tags, and truncates at `MAX_SKILL_CONTENT_LENGTH`. Real.

### Priority 4 — install/update/uninstall are faithful

- `src/installer/index.ts` is **2,818 lines**; `export function install()` is at **:2357**;
  downgrade guard at **:2383-2393** (message verified verbatim). Node gate confirmed from the
  published manifest: `engines.node = "20.x || 22.x || …"`.
- **No npm `postinstall`** — confirmed against the *published* package.json from the registry, not
  just the repo. The hidden `omc postinstall` really is at `src/cli/index.ts:1403` and is invoked by
  nothing else in the tree.
- `omc update` at `:734`, `omc update-reconcile` at `:819`;
  `assertSafeNpmPackageSpec()` allowlist at `auto-update.ts:61-65`, `timeout: 120000` at `:56`,
  `SILENT_UPDATE_STATE_FILE` at `:1355`.
- Managed-region markers verified: `OMC_START_MARKER`/`OMC_END_MARKER`
  (`claude-md-analysis.ts:13-14`), `<!-- OMC:VERSION: -->` and `<!-- User customizations -->`
  (`claude-md-transaction.ts:113-152`), and `# BEGIN/END OMC MANAGED MCP REGISTRY`
  (`mcp-registry.ts:48-49`) with `getCodexConfigPath()` at `:82-85`. Transaction exit codes
  **5 / 6** confirmed at `claude-md-transaction.ts:265`.
- **Uninstall — the report's harshest claim is its most precisely correct one.**
  `grep -rn "uninstall" src/cli/` returns nothing. A script diff of `scripts/uninstall.sh` against
  the shipped tree reproduces the report's leftover list *exactly*:
  removes 10 agent filenames, of which `vision.md` is not shipped, leaving precisely
  `code-reviewer, code-simplifier, debugger, git-master, qa-tester, scientist, security-reviewer,
  test-engineer, tracer, verifier` (10 files). All three skill dirs it removes
  (`ultrawork`, `git-master`, `frontend-ui-ux`) are **not** among the current 35, so all 35 survive.
  Its `jq` block touches only `.hooks.UserPromptSubmit` and `.hooks.Stop` and only the three
  `.sh` filenames. No mention of `hud`, `statusLine`, `~/.config/omc`, or `~/.codex`; `CLAUDE.md`
  appears only as a printed manual instruction at line 168.

### Corrections (all minor; none change a conclusion)

1. **Contributors: 135, not 150.** `contributors?per_page=1` (default `anon=false`) gives
   `rel="last" page=135`, and a full `--paginate` enumeration yields **135** logins. The 150 figure
   is the `anon=true` count, which includes unmatched commit-email identities. Still a large
   contributor base; the report should have carried the caveat.
2. **Releases: 248, not "100+".** `gh api .../releases --paginate` counts **248**. The report
   understates by ~2.5x — conservative rather than wrong, but the real number is more impressive.
3. `markSkillAsOmcManaged` is at `index.ts:2172`, not `:2173` (off by one; the function body is
   `:2172-2180`).
4. **MCP tool breakdown is slightly off in the details, right in the headline.** ~56 is defensible
   (63 `name:` literals in `src/tools/`, of which ~54–56 are real tools once LSP server names like
   `clangd`/`gopls`/`OmniSharp` and test fixtures are excluded). `lsp_* ×12`,
   `project_memory_* ×4`, `shared_memory_* ×5`, `state_* ×6` are exact — but `notepad_*` is **6**
   (not 5), `wiki_*` is **5** (not 4), and the list omits `merge_readiness_* ×5` and `python_repl`.
5. **`execFileSync` is platform-conditional.** `npmInstallGlobalPackage()`
   (`auto-update.ts:67-75`) uses `execFileSync` on POSIX but falls back to
   `execSync(\`npm install -g ${packageSpec}\`)` on `win32`. The `assertSafeNpmPackageSpec()`
   allowlist runs first either way, so the security conclusion holds; the mechanism description
   doesn't.
6. Test-file count: **718** `*.test.ts` repo-wide (698 under `src/`) vs the reported 716 — rounding.
   Total `SKILL.md` payload is **431 KB**, not "~450 KB" (the §9 context-economy comparison).

### One nuance the report should have pre-empted

`PluginConfig` *does* contain an `agentOverrides` field (`src/shared/types.ts:161-167`, defaults at
`config/loader.ts:104`), and unlike `agents` it is an open `Record<string, …>`. A reader grepping for
"override" will find it and think the report missed something. It did not: `agentOverrides` sets only
a **routing tier** (`{tier: "LOW"|"MEDIUM"|"HIGH", reason: string}`) for an existing agent. It cannot
register an agent, replace a prompt, or disable a skill. The report's core finding —
*additive-only, no override chain, closed roster* — survives intact.

### Bottom line

This is an unusually well-sourced teardown: line numbers, byte counts, API figures and shell-command
outputs all reproduce on an independent clone. The four headline judgements — **provenance-gated
deletion is the crown jewel**, **the uninstaller is broken**, **the agent roster is closed**, and
**there is no registry** — are each verified at the code that implements (or fails to implement)
them. The `transfers_to_musecode` section rests on facts that hold; act on it.
