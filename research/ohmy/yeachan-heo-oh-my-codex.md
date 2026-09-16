# Teardown: Yeachan-Heo/oh-my-codex (OMX)

Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/yeachan-heo-oh-my-codex/`
HEAD: `b48df502d566430be768d3c60f2f51fa91630396` — Tue Sep 1 13:17:53 2026 +0900, "Merge pull request #3603 from Yeachan-Heo/release/0.21.2"

---

## 1. EXISTS: CONFIRMED

`gh repo view Yeachan-Heo/oh-my-codex` returns a live repo, `npm view oh-my-codex` returns 131
published versions. Both verified. Homepage `https://oh-my-codex.dev`, docs site
`https://yeachan-heo.github.io/oh-my-codex-website/`.

Package name `oh-my-codex`, binary name `omx`. Self-described as "Multi-agent orchestration layer
for OpenAI Codex CLI" (`package.json:4`).

Important framing correction vs. the sweep's assumption: **this is not a port of the same asset set
to a second host.** It is a very large standalone product — 197,916 lines of TypeScript across 393
non-test source files, plus 6 Rust crates (13,417 lines) that ship as prebuilt native binaries.
The "oh-my-*" name is branding; the architecture has almost nothing in common with a dotfiles
framework.

---

## 2. WHAT IT SHIPS (real counts)

Counted with `find | wc -l` inside the clone.

| Category | Count | Evidence |
|---|---|---|
| Skills (canonical root) | **29** `SKILL.md` | `find skills -name SKILL.md \| wc -l` → 29; 30 files total (one `references/agent-tiers.md`) |
| Skills (plugin mirror) | **24** `SKILL.md` | `find plugins -name SKILL.md \| wc -l` → 24 (mirror carries only catalog-installable ones) |
| Agent role prompts | **32** `.md` | `prompts/*.md` — analyst, architect, code-reviewer, critic, debugger, designer, executor, planner, qa-tester, researcher, security-reviewer, team-orchestrator, verifier, vision, writer, … |
| Native agent definitions | **30 catalog rows / 16 active** | `src/catalog/manifest.json` → agents: 30 total {active 16, internal 2, merged 10, deprecated 2}; categories {build 8, review 6, domain 10, product 4, coordination 2} |
| Catalog skill rows | **33 total / 23 active** | same manifest → {active 23, deprecated 4, alias 1, merged 4, internal 1}; categories {execution 10, planning 4, shortcut 7, utility 12} |
| Core skills (pinned, cannot be deactivated) | **4** | `autopilot, team, ultragoal, ralplan` — enforced by `REQUIRED_CORE_SKILLS` in `src/catalog/schema.ts:31` |
| Hooks | **1 script, 7 lifecycle events** | `plugins/oh-my-codex/hooks/hooks.json` registers SessionStart, PreToolUse, PostToolUse, UserPromptSubmit, PreCompact, PostCompact, Stop — all pointing at the *same* `codex-native-hook.mjs`. Matches `MANAGED_HOOK_EVENTS` in `src/config/codex-hooks.ts:5` |
| MCP servers | **6, all `enabled: false`** | `plugins/oh-my-codex/.mcp.json` — `omx_state`, `omx_memory`, `omx_code_intel`, `omx_trace`, `omx_wiki`, `omx_hermes`, each `omx mcp-serve <name>` |
| Apps | **0** | `plugins/oh-my-codex/.app.json` is literally `{"apps":{}}` |
| Settings presets | **1 template + HUD preset** | `templates/AGENTS.md` (229 lines / 22.4 KB), `.omx/hud-config.json` default `{"preset":"focused"}` (`src/cli/setup.ts:5092`) |
| Statusline | **1**, config-driven | written into `~/.codex/config.toml` `[tui]` with marker `# omx:managed-status-line` |
| Rust crates | **6** (37 `.rs` files, 13,417 lines) | `omx-api` 3007, `omx-explore-harness` 2972, `omx-runtime-core` 3344, `omx-sparkshell` 2877, `omx-mux` 679, `omx-runtime` 538 |
| Docs | **266** `.md` under `docs/` | includes 14 translations of `openclaw-integration.*` |
| TS source / tests | **393** src files / **437** test files | tests outnumber sources |

Notable file-size outliers (this matters, see §9):

```
23,077  src/scripts/codex-native-hook.ts     <- the hook entrypoint
10,725  src/cli/index.ts
 7,127  src/team/runtime.ts
 6,469  src/cli/setup.ts   (208 KB)
 4,426  src/hooks/keyword-detector.ts
 2,723  src/cli/plugin-marketplace.ts (112 KB)
 2,076  src/cli/uninstall.ts (69 KB)
```

---

## 3. INSTALL — what it actually writes

**Mechanism: `npm install -g oh-my-codex`.** No curl|bash, no git clone, no brew.
`package.json:bin` → `omx: dist/cli/omx.js`.

### 3a. postinstall (`src/scripts/postinstall.ts`)

Deliberately near-inert. `runPostinstall()`:
- returns `noop-local` unless `npm_config_global` is truthy — a local install does nothing.
- calls `hydrateNativeBinary("omx-runtime")` with a 15s abort timeout; failures are non-fatal.
- writes an install stamp and **prints a hint instead of running setup**:
  `"OMX setup is explicit opt-in; run `omx setup` or `omx update` when you're ready."`

This is the right call and rare in this category. Nothing touches the user's Codex config until they
type a command.

### 3b. Native binary hydration (`src/cli/native-assets.ts`)

Downloads prebuilt Rust binaries from GitHub Releases (`{repo}/releases/download/v{version}`, line
99) into a **cache outside the package**:

```
~/.cache/oh-my-codex/native/<version>/<platform>/<product>/<binary>       (XDG_CACHE_HOME honored)
%LOCALAPPDATA%\oh-my-codex\native\...                                     (Windows)
```

Each binary gets a `.sha256` sidecar; the loader distinguishes `orphan-checksum`,
`checksum-unsafe`, `checksum-malformed`, `checksum-mismatch` (lines 295–298), verifies the digest
against a release manifest before use, writes via temp+rename, and holds a lock file. Env overrides:
`OMX_NATIVE_AUTO_FETCH`, `OMX_NATIVE_MANIFEST_URL`, `OMX_NATIVE_RELEASE_BASE_URL`,
`OMX_NATIVE_CACHE_DIR`, `OMX_NATIVE_LOCK_WAIT_MS`.

### 3c. `omx setup` — the real installer, 8 numbered steps (`src/cli/setup.ts:4375+`)

Scope is explicit: `--scope user` (writes to `$CODEX_HOME` / `~/.codex`) or `--scope project`
(writes to `./.codex`). Install mode is `legacy` (copy assets) or `plugin` (register with Codex's
own plugin system). MCP mode `none` (default) or `compat`.

```
[1/8] mkdir            <codexHome>/, prompts/, skills/, agents/, .omx/{state,plans,logs}/
[2/8] prompts          prompts/*.md          -> <codexHome>/prompts/<name>.md
[3/8] skills           skills/<n>/SKILL.md   -> <codexHome>/skills/<n>/SKILL.md
[4/8] native agents    generated TOML        -> <codexHome>/agents/<name>.toml
[5/8] config.toml      [features], [tui], [mcp_servers.*], [plugins.*], [marketplaces.*],
                       [projects."<path>"], notify, model_reasoning_effort,
                       developer_instructions, model
                       + <codexHome>/hooks.json (shared-ownership merge)
[5.5]  verify team CLI interop
[6/8] AGENTS.md        ./AGENTS.md (project) or <codexHome>/AGENTS.md (user)
[7/8] notify hook
[8/8] HUD              .omx/hud-config.json
```

Also: `ensureProjectGitignore()` appends `.omx/` entries to the project `.gitignore`
(`setup.ts:3130`).

### Full config footprint

```
~/.codex/config.toml                              (marker-delimited block, merged)
~/.codex/hooks.json                               (shared-ownership: only OMX wrappers)
~/.codex/prompts/<32 files>.md
~/.codex/skills/<~24 dirs>/SKILL.md
~/.codex/agents/<16 active>.toml
~/.codex/AGENTS.md                                (user scope) or ./AGENTS.md (project scope)
~/.codex/.omx/native-agents.json                  <- sha256 install manifest
~/.codex/.omx/install-state.json                  <- version stamp
~/.codex/plugins/... cache                        (plugin install mode only)
~/.omx/state/setup/installed-skills.json          <- sha256 receipt (user scope)
~/.cache/oh-my-codex/native/<ver>/<plat>/...      <- Rust binaries + .sha256 sidecars
./.omx/                                           <- setup-scope.json, hud-config.json, state/,
                                                     plans/, logs/, ultragoal/, adapters/,
                                                     specs/, reviews/, hooks/, tmp/
./omx_wiki/                                       (repo-tracked, if wiki used)
./project-memory.json                             (repo-visible canonical memory)
./.gitignore                                      (appended)
./.codex/{prompts,skills,agents,hooks.json}       (project scope)
```

Paths are centralised in `src/utils/paths.ts` — a single 400-line module that is the only place
that knows where anything lives. This is the cleanest file in the repo and the piece most worth
copying.

---

## 4. EXTENSION CONTRACT

Three real, non-forking mechanisms — this is the strongest part of the design:

**(a) Hook plugins — `.omx/hooks/*.mjs`** (`src/hooks/extensibility/loader.ts`, `docs/hooks-extension.md`)

The genuine plugin protocol. Drop any `.mjs` file in `.omx/hooks/` that exports `onHookEvent`;
`discoverHookPlugins()` picks it up by convention — no registration, no manifest, no list of enabled
modules. Notable properties:
- **Enabled by default** (`isHookPluginsEnabled` returns true unless `OMX_HOOK_PLUGINS=0`).
- Export is validated by *source regex* before import (`ON_HOOK_EVENT_EXPORT_PATTERN`,
  loader.ts:59) so a syntactically broken plugin cannot crash the hook.
- Each plugin runs in a subprocess with a clamped timeout (default 1500 ms, `OMX_HOOK_PLUGIN_TIMEOUT_MS`,
  clamped 100–60,000).
- Event vocabulary: `session-start`, `keyword-detector`, `pre-tool-use`, `post-tool-use`, `stop`,
  `session-end`, `turn-complete`.
- Scaffolded with `omx hooks init`; there is `omx hooks status|validate|test`.

**(b) User skills — `~/.codex/skills/<name>/` and `.codex/skills/<name>/`**

`skills/skill/SKILL.md` documents `/skill add|edit|remove|search|list|sync|validate`. User skills
live in the *same* directory as OMX's, with project-over-user precedence
(`listInstalledSkillDirectories`, `paths.ts`). The ownership-receipt machinery (§5) is what makes
co-tenancy safe.

**(c) Shared-ownership config files**

`~/.codex/hooks.json` and `config.toml` are merged, not replaced. `AGENTS.md` uses
`<!-- OMX:AGENTS:START -->` / `<!-- OMX:AGENTS:END -->` markers plus nested overlay markers
(`<!-- OMX:RUNTIME:START -->`, `<!-- OMX:TEAM:WORKER:START -->`, `<!-- OMX:GUIDANCE:OPERATING:START -->`)
so OMX rewrites only its own block. `--merge-agents` / `--no-merge-agents` /
`--clear-merge-agents-policy` persist the choice per-root in `.omx/setup-scope.json`.

**What is NOT extensible:** the shipped catalog. `templates/catalog-manifest.json` +
`src/catalog/manifest.json` are compiled-in SSOT. There is no user-facing "enabled modules" list —
`.omx/setup-scope.json` only holds `{scope, installMode, mcpMode, teamMode, mergeAgents}`
(`src/cli/setup-preferences.ts:22`). You cannot disable `ultragoal` or add a curated skill to the
official bundle without a PR. `docs/plugin-bundle-ssot.md` spells out the 4-step contributor
workflow: edit `skills/<n>/SKILL.md` → add a catalog row → `npm run sync:plugin` →
`npm run verify:plugin-bundle`.

---

## 5. UPDATE — and how it avoids clobbering

**Update:** `omx update` (`src/cli/update.ts`). Channels: `stable` = `oh-my-codex@latest` via
`npm install -g` (or `bun add -g` — it detects the owning package manager via
`src/cli/package-manager-ownership.ts`); `dev` = clone `github:Yeachan-Heo/oh-my-codex#dev`,
`npm install --include=dev`, `npm run prepack`, `npm pack`, then global-install the tarball
(300 s timeout). Launch-time throttled check every 12 h (`CHECK_INTERVAL_MS`), prompts before
scheduling, and defers the actual install until after the current session exits via a detached
worker process (`runDeferredGlobalUpdate` → `update-worker.ts` with a payload fingerprint).
`OMX_AUTO_UPDATE=0` disables; `=defer` schedules without prompting.

**Anti-clobber: two sha256 ownership ledgers.** This is the genuinely good idea in the repo.

*Native agent TOMLs* — `<codexHome>/.omx/native-agents.json`, `{version:1, files:{"<name>.toml":{sha256}}}`
(`setup.ts:5320–5450`). On refresh (`syncNativeAgentToml`):

```
priorHash = manifest.files[fileName]?.sha256
safeToOverwrite = options.force || priorHash === existingHash
if (!safeToOverwrite) { summary.skipped++; "local modifications preserved; use --force" }
```

i.e. OMX overwrites only files whose current bytes still match what OMX last wrote. Any user edit
freezes the file until `--force`.

*Skills* — `~/.omx/state/setup/installed-skills.json` (user) or `<projectRoot>/.omx/...` (project),
recording a per-file sha256 map per skill (`setup.ts:2328–2440`). Three details show unusual care:

- The receipt records **the installer's own output**, not a scan of the destination tree. The
  comment is explicit: digesting the destination "would poison the receipt: a note a user drops into
  an active skill directory would be recorded as OMX-owned and then deleted when that skill is later
  retired."
- Records are built on `Object.create(null)` because a file literally named `__proto__` would
  otherwise vanish from `Object.keys()` and let a user file be deleted while the comparison still
  reported equality. That is a prototype-pollution defense in an *installer*.
- Non-regular entries (symlinks, sockets) are recorded as a `"non-regular-entry"` sentinel rather
  than skipped, so a directory containing one can never compare equal and is therefore retained.

Every managed write also goes through `ensureBackup()` into a timestamped backup directory, and the
run prints a per-category summary (`updated / unchanged / skipped / backedUp / removed`) for
prompts, skills, native_agents, agents_md, config.

The hooks/config/shim mutation is a **compensating transaction** with pre-flight snapshots,
`COPYFILE_EXCL` claim files, an fsync'd claim journal, post-write readback verification, ancestor
topology preconditions, and rollback (`setup.ts:1232–2148`). ~900 lines exist purely to make
"write two files atomically" survive a crash.

---

## 6. REGISTRY: **NONE**

There is no index or marketplace of third-party additions. `.agents/plugins/marketplace.json`
declares exactly one marketplace (`oh-my-codex-local`) containing exactly one plugin
(`oh-my-codex`), sourced `local` from `./plugins/oh-my-codex`. All 2,723 lines of
`src/cli/plugin-marketplace.ts` are hard-wired to those two constants
(`OMX_LOCAL_MARKETPLACE_NAME`, `OMX_PLUGIN_NAME`, lines 16–18); there is no add/remove/search of
other marketplaces anywhere in the file.

So OMX **uses Codex's plugin/marketplace system as a delivery vehicle for itself**, and does not
operate a registry. Discovery of third-party OMX hook plugins is entirely ad hoc — Discord + the
docs page. There is no `omx search`, no `omx install <thing>`.

---

## 7. UNINSTALL: clean, and unusually careful — with two real gaps

`omx uninstall` (`src/cli/uninstall.ts`, 2,076 lines), flags `--dry-run`, `--purge`,
`--keep-config`, `--scope`. Six steps. The hooks + shim + config removal is the same compensating
transaction as setup, with the config mutation deliberately committed last "so hooks never reference
unwritten trust."

Ownership-aware removals:
- `AGENTS.md`: strips the `<!-- OMX:AGENTS:START/END -->` block and **preserves surrounding user
  guidance**; deletes the whole file only if `isOmxGeneratedAgentsMd(content)`.
- `config.toml`: removes the marker block; `[features].multi_agent` and `hooks` are **preserved when
  user-owned** (`preserveHooksFeatureFlag = hooksRemoval.plan?.hasForeignHooks`).
- `hooks.json`: removes only OMX wrapper entries; leaves the file if user hooks remain.
- `.omx/`: removed only with `--purge`; otherwise it deletes just `setup-scope.json` and
  `hud-config.json`.
- Emits a warning about the historical `~/.agents/skills` root, which it explicitly refuses to touch.

**Gap 1 — uninstall is name-matched, setup is receipt-matched.** `removeInstalledSkills`
(uninstall.ts:744) and `removeInstalledPrompts` (uninstall.ts:715) iterate the *package's* skill and
prompt names and `rm -rf` any destination directory/file with a matching name. They never read
`installed-skills.json`. So a user who edited `~/.codex/skills/team/SKILL.md` — which `omx setup`
carefully refuses to overwrite — loses it silently on `omx uninstall`. And a user who had their own
`~/.codex/prompts/planner.md` before ever installing OMX loses it too. The conservative machinery
exists; uninstall just doesn't call it.

**Gap 2 — no backups on uninstall.** `ensureBackup` is a setup-only path. Uninstall snapshots
config/hooks for rollback purposes but does not archive removed prompts or skills.

Residue after a non-`--purge` uninstall: `~/.cache/oh-my-codex/native/` (Rust binaries, never
cleaned), `~/.codex/.omx/install-state.json`, `~/.codex/.omx/native-agents.json`, most of `./.omx/`,
`./omx_wiki/`, `./project-memory.json`, and the `.gitignore` lines.

---

## 8. TRACTION (measured 2026-09-01)

Via `gh api repos/Yeachan-Heo/oh-my-codex` and the npm registry API:

| Metric | Value |
|---|---|
| Stars | **32,946** |
| Forks | 2,534 |
| Watchers (subscribers) | 83 |
| Open issues | **0** (issues enabled) |
| Closed issues | 1,267 |
| Closed PRs | 2,298 |
| Created | 2026-02-02 |
| Last push | 2026-09-01T06:50:52Z (same day) |
| GitHub releases | **134** |
| npm published versions | **131**, latest `0.21.2` |
| npm first publish | 2026-02-13 |
| npm downloads, last week | **4,021** |
| npm downloads, last month | **22,247** |
| Contributors | 30 |
| Commits: Yeachan-Heo / HaD0Yun / iqdoctor | 2,702 / 139 / 88 |
| Repo size | 31,965 KB |
| License | **`package.json` says MIT; there is no LICENSE file in the repo** (`licenseInfo: null`) |

Read on those numbers: 32.9k stars in 7 months with only 83 watchers and 4k weekly npm downloads is
a badly skewed ratio — roughly 8 stars per weekly download. Real usage is in the low thousands.
134 releases in 210 days (~1 every 1.6 days) and 2,702 commits from one author (~13/day) point at a
heavily agent-driven development loop; the repo contains `missions/`, `.gjc/ultragoal/`, and
`artifacts/release-0.21.*/` consistent with that. 0 open issues alongside 1,267 closed is a
maintainer who closes aggressively, not an absence of problems.

---

## 9. GOOD vs BAD

### Genuinely good — steal these

1. **sha256 ownership ledgers.** The single most transferable idea. "Overwrite only what I wrote,
   byte-identical" turns a config installer from a hazard into a safe, repeatable convergence step,
   and it needs no lockfile format, no diff3, no user prompt.
2. **Ownership receipts describe the installer's output, not the destination.** The comment
   explaining why is worth reading verbatim (`setup.ts:2404–2412`). Nearly every framework in this
   space gets this backwards and eats user files on retirement.
3. **Marker-delimited managed blocks in shared files.** `<!-- OMX:AGENTS:START/END -->`,
   `# oh-my-codex (OMX) Configuration` … `# End oh-my-codex`, `# omx:managed-status-line`. Turns
   `config.toml`, `hooks.json` and `AGENTS.md` into co-tenant files instead of owned files.
4. **Explicit opt-in setup.** postinstall prints a hint and stops. No surprise mutation of
   `~/.codex` from an `npm install -g`.
5. **Explicit scope with upward resolution.** `--scope user|project`, persisted in
   `.omx/setup-scope.json`, and `resolveNearestPersistedSetupScopeSync()` walks *up* from cwd so
   launching from a subdirectory still finds the project's config (issue #3447). Small, correct,
   and the kind of thing everyone gets wrong.
6. **Convention-discovered hook plugins with subprocess isolation + clamped timeout + pre-import
   source validation.** A user extension cannot hang or crash the host's hook path.
7. **The catalog as curation SSOT with a status lifecycle.** `active | alias | merged | deprecated |
   internal` plus `canonical` forwarding lets skills be renamed, merged and retired without breaking
   users, and `REQUIRED_CORE_SKILLS` fails the build if a load-bearing skill is deactivated.
8. **`omx-capabilities.lock.json`** — a 48 KB digest lock over four asset surfaces, verified in
   `npm test` via `verify:capabilities-lock`. Asset drift fails CI, not production.
9. **Generated-and-verified plugin mirror.** `plugins/oh-my-codex/` is derived from `skills/` by
   `sync:plugin` and checked by `verify:plugin-bundle` in `prepack`. One canonical authoring root,
   host-shaped output generated from it — exactly the right shape for multi-host.
10. **`--dry-run` on both setup and uninstall,** with the same summary output as the real run.

### Bad — do not copy

1. **`src/scripts/codex-native-hook.ts` is 23,077 lines.** A single file on the hook hot path, run
   on 7 lifecycle events per turn. `src/cli/index.ts` is 10,725; `setup.ts` 6,469;
   `plugin-marketplace.ts` 2,723; `uninstall.ts` 2,076. This is not modular software; it is a
   monolith with a manifest.
2. **The transaction machinery is disproportionate.** ~900 lines of claim journals, ancestor
   topology preconditions, readback verification and rollback to write two config files. Correct,
   but the surface it protects (`hooks.json` + `config.toml`) does not justify the maintenance
   burden, and it is now itself a large untestable-by-inspection artifact.
3. **Uninstall ignores the receipts setup maintains** (§7 Gap 1). The most dangerous single defect
   here: OMX teaches the user that their edits are safe, then deletes them by name on the way out.
4. **No LICENSE file** despite MIT in `package.json` and an MIT badge in the README. GitHub reports
   `license: null`. For anything anyone wants to fork or vendor, this is a real problem.
5. **`templates/AGENTS.md` opens with a shouting all-caps autonomy directive** injected into the
   user's `AGENTS.md` at user scope by default. "EXECUTE TASKS TO COMPLETION WITHOUT ASKING FOR
   PERMISSION… ONLY ASK WHEN TRULY AMBIGUOUS OR DESTRUCTIVE." Rewriting the global safety posture of
   someone's agent as an install side-effect is a bad default even with a merge policy.
6. **No registry, but registry-shaped complexity.** 2,723 lines of marketplace code that can only
   ever install one plugin from one local marketplace.
7. **README as governance document.** Core Maintainers / Ambassadors / Top Collaborators tables, and
   a section asserting which fork is "official" — signals name-squatting pressure and community
   friction, not architecture.
8. **Two parallel install modes (`legacy` vs `plugin`) that must be kept behaviourally equivalent.**
   Every step in setup branches on `isPluginInstallMode`. This doubles the state space of the single
   riskiest code path in the product.
9. **14 translations of one integration doc** while the extension contract has one page. Effort
   allocated by what is easy to generate rather than what is load-bearing.
10. **Skills are copied, not linked.** Every `omx setup` re-copies 24 directories to converge; the
    receipts exist only because the copy is lossy. A content-addressed store plus symlinks would
    have made most of §5 unnecessary.

---

## 10. TRANSFER TO MUSE CODE (compiled Rust host)

### Transfers directly — this is the core of what oh-my-musecode should take

- **The sha256 ownership ledger.** Muse already has `.muse/lock.json` (provenance/quarantine/
  allowed_tools) and `.muse/skills.lock`. Extend that model with an *installed-asset* ledger keyed by
  file with the digest of what the installer wrote, and adopt the rule verbatim: overwrite iff
  `on_disk_digest == recorded_digest`, else skip with a `--force` escape. In Rust this is ~80 lines
  with `sha2` + `serde_json`, versus the ~1,500 TS lines here.
- **Receipt records the writer's output, not the destination scan** — plus the sentinel for
  non-regular entries. Language-independent, and the exact bug class it prevents (deleting a user's
  note dropped into a managed skill dir) will hit oh-my-musecode on day one.
- **Marker-delimited managed blocks** in `.muse/hooks.json`, `AGENTS.md`/`MUSE.md`, and settings.
  JSON needs a sibling key (`"_omm_managed": [...]`) rather than comments, but the pattern holds.
- **Explicit `--scope user|project` with upward-walking resolution** from a `.omm/scope.json`
  marker. Directly portable; Muse's user/project config split is the same shape.
- **The catalog manifest with a status lifecycle** (`active|alias|merged|deprecated|internal` +
  `canonical` forwarding + required-core validation). This is pure declarative data. In Rust it
  becomes a `serde`-derived struct with a validating `TryFrom` — strictly better than the hand-rolled
  validator in `schema.ts`.
- **The generated-and-verified mirror pattern.** One canonical `skills/<n>/SKILL.md` root, with
  `.muse-plugin/`, `.claude-plugin/` and `.codex-plugin/` manifest trees *generated* from it and
  drift-checked in CI. Given that Muse ingests all three manifest kinds, this is the single highest-
  leverage structural idea here for a multi-host play — and it is the thing the sweep hypothesis was
  actually reaching for, done properly.
- **A capabilities lockfile over asset surfaces**, verified in CI. Maps onto `.muse/skills.lock`.
- **Explicit opt-in setup, `--dry-run`, and a per-category converge summary.**
- **Convention-discovered hook plugins run as subprocesses with a clamped timeout and validated
  entrypoint.** Muse's `.muse/hooks.json` already spawns commands; the addition is a conventional
  drop-in directory (`.omm/hooks/`) plus the timeout/validation discipline. Note the Rust host makes
  this *better*: it can enforce the timeout and sandbox in-process rather than trusting Node.
- **Checksummed, cached, atomically-installed release assets** (`native-assets.ts`'s temp+rename +
  `.sha256` sidecar + lock + digest-state taxonomy) — if oh-my-musecode ever ships binaries or large
  bundles. Rust gets this nearly free with `sha2` + `fs2` (which OMX's own `omx-runtime-core` already
  uses).

### Does NOT transfer

- **npm as the distribution and update channel.** `npm install -g` / `bun add -g`, the
  `package-manager-ownership.ts` detection, `postinstall`, `prepack`, `npm pack`, the deferred
  detached update worker — all of it is JS-ecosystem plumbing. For a Rust host the analogue is a
  versioned asset bundle (a tarball or an OCI artifact) plus a digest manifest, resolved by the
  binary itself.
- **The Node hook runtime.** `node "${PLUGIN_ROOT}/hooks/codex-native-hook.mjs"` invoked on 7 events
  per turn pays ~50–100 ms of Node startup each time. A Rust host should either resolve hooks
  in-process or spawn a long-lived hook daemon over its stdio protocol. Muse's `muse serve` / MSP is
  the natural place for this and is a strict improvement over OMX's architecture, not a port of it.
- **The 900-line compensating transaction.** Not because the goal is wrong, but because in Rust the
  correct answer is a small `write_atomic()` helper over `tempfile::NamedTempFile::persist` plus an
  `fs2` advisory lock — 50 lines, already proven inside this repo's own `omx-runtime-core`.
- **The entire TypeScript orchestration layer** (`src/team/`, `src/ultragoal/`, `src/ralph/`,
  `src/hud/`, MCP servers, tmux integration — ~170k lines). That is a *product built on top of*
  Codex, not a configuration framework. oh-my-musecode should explicitly not follow OMX across this
  line; the framework part of this repo is roughly `src/cli/setup.ts` + `src/catalog/` +
  `src/utils/paths.ts` + `src/hooks/extensibility/` — maybe 8k of the 198k lines.
- **`.mcp.json` with `enabled:false` defaults and MCP-as-compat-mode.** OMX has visibly retreated
  from MCP toward CLI-first (`--mcp none` is the default, with setup offering to *remove* prior OMX
  MCP registrations). Worth noting as a negative result: don't build oh-my-musecode's primary
  integration surface on MCP servers.
- **The `legacy` vs `plugin` dual install mode.** Muse ingests plugin manifests natively; pick the
  manifest path and only that path.

### One strategic read

The sweep's premise — "same author holds the oh-my-* name across hosts by porting the same asset
set" — is **not** what the source shows. `oh-my-codex` shares branding with `oh-my-claudecode` but is
a from-scratch 198k-line orchestration product wearing a framework's name. The transferable asset is
not the content and not the port; it is the **five-file discipline**: `paths.ts` (one module knows
every path), `catalog/manifest.json` (curation with a lifecycle), the sha256 ownership ledgers
(convergence without clobbering), the marker blocks (co-tenancy in shared files), and
`sync-plugin-mirror.ts` (one canonical root, N generated host manifests, drift-checked in CI). Those
five, in Rust, are perhaps 1,500 lines and are the whole of oh-my-musecode's install/update/uninstall
story.

---

## Verification

Adversarial re-verification, 2026-09-01. Method: fresh independent `git clone --depth 5` into
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-yeachan-heo-oh-my-codex/omx`
(HEAD `b48df502` — "Merge pull request #3603 from Yeachan-Heo/release/0.21.2", 2026-09-01), plus
`gh api repos/Yeachan-Heo/oh-my-codex`, the npm registry API, and `api.npmjs.org/downloads`. All
counts re-run from scratch; every claimed code path opened and read rather than inferred.

**Verdict: MOSTLY_SOLID.** Nothing material is refuted. The repo is real, the traction figures are
exact to the digit, every shipped-content count reproduces, the extension contract is implemented in
code (not aspirational README text), and the install/update/uninstall description is faithful —
including the two uninstall defects, which I confirmed by reading the functions. Corrections below
are all peripheral: numeric or attributional, none changes a conclusion.

### Priority 1 — existence and traction: CONFIRMED, exact

| Claim | Measured |
|---|---|
| 32,946 stars | 32,946 |
| 2,534 forks | 2,534 |
| 83 watchers | `subscribers_count` = 83 |
| 0 open issues, issues enabled | 0, `has_issues: true` |
| 1,267 closed issues / 2,298 closed PRs | 1,267 / 2,298 |
| created 2026-02-02 | 2026-02-02T14:21:18Z |
| last push 2026-09-01T06:50:52Z | identical |
| repo 31,965 KB | 31,965 |
| 134 GitHub releases | 134 (`--paginate`) |
| 131 npm versions, latest 0.21.2 | 131, `dist-tags.latest = 0.21.2` |
| first npm publish 2026-02-13 | `time.created` = 2026-02-13T11:12:28Z |
| 4,021 weekly / 22,247 monthly downloads | identical |
| GitHub `licenseInfo: null`, MIT in package.json | `license: None`; `"license": "MIT"`; `ls LICENSE*` → no matches |

The skew observation stands on the measured numbers. Commit split is exact: Yeachan-Heo 2,702 /
HaD0Yun 139 / iqdoctor 88 / dependabot[bot] 56.

### Priority 2 — shipped-content counts: CONFIRMED

`find skills -name SKILL.md | wc -l` = **29**; `find skills -type f` = **30**. Plugin mirror
`plugins/oh-my-codex/skills/**/SKILL.md` = **24**. `prompts/*.md` = **32**. `docs/*.md` = **266**.
`src/catalog/manifest.json`: **33 skills** (status: 23 active / 4 deprecated / 4 merged / 1 alias /
1 internal; category: utility 12, execution 10, shortcut 7, planning 4) and **30 agents** (16 active
/ 10 merged / 2 internal / 2 deprecated; domain 10, build 8, review 6, product 4, coordination 2) —
every bucket matches. `.mcp.json` has exactly 6 servers, all `"enabled": false`. `.app.json` is
literally `{"apps": {}}`. `hooks/hooks.json` registers exactly 7 events (SessionStart, PreToolUse,
PostToolUse, UserPromptSubmit, PreCompact, PostCompact, Stop) with **one** distinct command string,
`node "${PLUGIN_ROOT}/hooks/codex-native-hook.mjs"`. `templates/` holds `AGENTS.md` (229 lines),
`catalog-manifest.json`, and 2 model-instruction variants. HUD default `{ preset: "focused" }` at
setup.ts:5092. Statusline marker `# omx:managed-status-line` at config/generator.ts:229. Every
monolith line count is exact: codex-native-hook.ts **23,077**, index.ts **10,725**, setup.ts
**6,469**, plugin-marketplace.ts **2,723**, uninstall.ts **2,076**.

### Priority 3 — extension contract: REAL, implemented in code

All three mechanisms verified against source, not docs.

1. **Hook plugins.** `src/hooks/extensibility/loader.ts` (130 lines): `hooksDir()` = `<cwd>/.omx/hooks`;
   discovery globs `*.mjs`; `isHookPluginsEnabled()` returns true unless the value is `0`/`false`/`no`
   (comment: "Plugins are ON by default"); `resolveHookPluginTimeoutMs` defaults **1500** and
   `readTimeout` clamps to **[100, 60000]**; `ON_HOOK_EVENT_EXPORT_PATTERN` is a source regex tested
   against the file **before** any import. `dispatcher.ts` `spawn(process.execPath, [runnerPath])` —
   a genuine subprocess, with SIGTERM→SIGKILL escalation on timeout (slightly better than the report
   describes). `plugin-runner.ts` imports the module and calls `onHookEvent(event, sdk)`.
   `omx hooks` is a real CLI verb (index.ts:463/500/5026).
2. **User skills.** `src/utils/paths.ts:176/181/186` — `codexHome()/skills`, `<root>/.codex/skills`,
   and the legacy `~/.agents/skills`. A `skill` skill ships in the catalog for add/edit/remove.
3. **Shared-ownership config.** Markers confirmed verbatim: `<!-- OMX:AGENTS:START -->`
   (utils/agents-md.ts:7), `<!-- OMX:RUNTIME:START/END -->` (hooks/agents-overlay.ts:50), 
   `<!-- OMX:TEAM:WORKER:START/END -->` (team/worker-bootstrap.ts:24), and config.toml's
   `oh-my-codex (OMX) Configuration` / `# End oh-my-codex` (config/generator.ts:251/253).

**Closed catalog: confirmed by negative search.** `src/catalog/reader.ts` resolves only
`templates/catalog-manifest.json`; greps for `.omx/catalog`, `userCatalog`, `CATALOG_OVERRIDE`,
`OMX_CATALOG` return nothing. `PersistedSetupScope` (setup-preferences.ts:19-25) is exactly
`{scope, installMode?, mcpMode?, teamMode?, mergeAgents?}` — no enabled-modules list, as claimed.
`REQUIRED_CORE_SKILLS` (catalog/schema.ts:31) is exactly `{autopilot, ralplan, team, ultragoal}`.

**Registry claim confirmed.** `OMX_LOCAL_MARKETPLACE_NAME`/`OMX_PLUGIN_NAME` are hard constants at
plugin-marketplace.ts:16-18; `.agents/plugins/marketplace.json` declares one marketplace with one
local-sourced plugin.

### Priority 4 — install / update / uninstall: FAITHFUL

`src/scripts/postinstall.ts` (153 lines) reads exactly as described — `noop-local` unless
`npm_config_global`/`npm_config_location=global`, 15,000 ms `AbortController` on
`hydrateNativeBinary("omx-runtime")` with a non-fatal catch, version stamp, then the literal hint
*"OMX setup is explicit opt-in; run `omx setup` or `omx update` when you're ready."* No mutation of
`~/.codex` from `npm install -g`.

Setup's eight steps print as `[1/8] Creating directories` … `[8/8] Configuring HUD`. All flags exist
(`--scope`, `--plugin`/`--legacy`/`--install-mode`, `--mcp none|compat`, `--dry-run`, `--force`,
`--merge-agents`/`--no-merge-agents`/`--clear-merge-agents-policy`).

`update.ts`: `CHECK_INTERVAL_MS = 12*60*60*1000`, `DEV_INSTALL_SOURCE =
'github:Yeachan-Heo/oh-my-codex#dev'`, `DEV_UPDATE_TIMEOUT_MS = 300000`, `npm run prepack` → `npm
pack` → global install, `runDeferredGlobalUpdate` → `update-worker.js`, `OMX_AUTO_UPDATE`.
`native-assets.ts`: `SIDECAR_SUFFIX = '.sha256'`, `LOCK_SUFFIX = '.hydrate.lock'`, `XDG_CACHE_HOME`
and `%LOCALAPPDATA%` branches, `rename`, and a digest-state taxonomy richer than described
(`orphan-checksum`, `publication-in-progress`, `legacy-unverified`, `inspection-race`).

**Ownership ledgers confirmed.** setup.ts:5423-5429 is literally
`const priorHash = manifest.files[fileName]?.sha256; const safeToOverwrite = options.force ||
priorHash === existingHash;` with the skip message *"local modifications preserved; use --force to
overwrite"*. The "poison the receipt" doc comment exists verbatim at setup.ts:2407-2412.
`Object.create(null)` appears five times in the receipt code (2341, 2343, 2352, 2366, 2424); the
`"non-regular-entry"` sentinel is at setup.ts:2376.
`resolveNearestPersistedSetupScopeSync` (setup-preferences.ts:201-216) really does walk `dirname`
upward to the filesystem root.

**Both uninstall gaps are real — independently confirmed.**
- Gap 1: `removeInstalledPrompts` (uninstall.ts:715) iterates `readdir(pkgRoot/prompts)` and
  `rm(join(promptsDir, file))` for every name match; `removeInstalledSkills` (uninstall.ts:744)
  iterates `readdir(pkgRoot/skills)` and `rm(..., {recursive: true, force: true})` on every
  name-matching destination directory. Neither consults a receipt. Grepping uninstall.ts for
  `installed-skills`, `InstalledSkillReceipt`, or `native-agents.json` returns **nothing** — the
  ledger setup maintains is never read on the way out.
- Gap 2: `ensureBackup` is defined in setup.ts:2149 and agents-init.ts:193 and called only from
  those two files. Zero call sites in uninstall.ts.
- The counterweight is also real: uninstall.ts:1956 carries the comment *"config mutation is
  intentionally last so hooks never reference unwritten trust"*, and the `~/.agents/skills` refusal
  (warn-only) is at uninstall.ts:1738-1752.

Build discipline confirmed: `prepack` = `build && verify:native-agents && sync:plugin &&
verify:plugin-bundle && clean:native-package-assets`; `verify:plugin-bundle` is
`sync-plugin-mirror.js --check`; `npm test` includes `verify:capabilities-lock`.
`omx-capabilities.lock.json` is 48,404 bytes.

### Corrections

1. **Contributors: 30 → 88.** `gh api .../contributors --paginate` yields 88 logins. "30" is the
   GitHub API's default `per_page` — an unpaginated call. The commit split quoted for the top four is
   nonetheless exact.
2. **Translations: 14 → 13.** `ls docs | grep -c '^openclaw-integration'` = 14, but one of those is
   the English canonical `openclaw-integration.md`. The languages are de/es/fr/it/ja/ko/pt/ru/tr/uk/
   vi/zh/zh-TW = 13. The point being made (effort allocated to what is easy to generate) is unharmed.
3. **TypeScript counts are slightly low.** Test files = 437 exactly. Non-test `src/**/*.ts` = **399
   files / 199,710 lines**, not 393 / 197,916. A definitional difference (probably `.d.ts` or
   `__tests__` helper exclusion), not an error of kind.
4. **Rust line accounting needs a footnote.** 37 `.rs` files and 6 crates are exact, and the claimed
   13,417 total is the exact sum of the per-crate figures given — but those are *src-minus-test-
   helpers*. Measured src-only per crate: omx-api 3,007 ✓, omx-explore-harness 2,972 ✓,
   omx-runtime-core 3,344 ✓, omx-mux 679 ✓, omx-runtime 538 ✓, **omx-sparkshell 3,066** (report said
   2,877; the 189-line delta is `src/test_support.rs`). Including `tests/` dirs the workspace is
   **15,951** lines. Note also the directory is `crates/omx-explore/` while the *crate* is named
   `omx-explore-harness`.
5. **Hook event vocabulary is understated and partly doc-sourced.** The seven names listed
   (session-start, keyword-detector, pre-tool-use, post-tool-use, stop, session-end, turn-complete)
   are the list in `docs/hooks-extension.md`, not the code. The actual `HookEventName` union
   (`src/hooks/extensibility/types.ts:4-26`) has **23** named events — adding session-idle, blocked,
   run.heartbeat, run.blocked_on_user, run.blocked_on_system, finished, failed, worker.assigned,
   worker.stalled, worker.recovered, retry-needed, pr-created, test-started, test-finished,
   test-failed, handoff-needed, needs-input — plus an open `(string & {})` escape. `keyword-detector`
   is **not** in the union; it appears in the doc and as the module name `src/hooks/keyword-detector.ts`.
   This makes the hook-plugin surface *broader* than the report credits, which strengthens transfer
   point (9) rather than weakening it.
6. **Setup step ordering.** The report folds "merge config.toml + hooks.json" into one step. The
   banners are `[5/8] Updating config.toml` and `[7/8] Configuring notification hook` — hooks wiring
   is step 7, with `[6/8] Generating AGENTS.md` between them.
7. Trivia: the "poison the receipt" comment is at setup.ts:2407-2412 (report cited 2404);
   `paths.ts` is 411 lines (report said ~400); `templates/AGENTS.md` is 22,439 bytes.

### Refuted

Nothing. No fabricated repo, no invented file, no code path that exists only in the README. Spot
checks of the report's most falsifiable assertions — the `priorHash === existingHash` rule, the
`Object.create(null)` prototype defence, the `"non-regular-entry"` sentinel, the two hard-wired
marketplace constants at lines 16-18, the all-caps autonomy directive at the top of
`templates/AGENTS.md`, the missing LICENSE file, and both uninstall gaps — all reproduced verbatim.
The transfer analysis and the strategic read rest on facts that hold.
