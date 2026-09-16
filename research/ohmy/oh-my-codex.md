# oh-my-codex (OMX) — Architecture Teardown

**Verdict: CONFIRMED. It exists, it is large, and it is the single most relevant prior art for
oh-my-musecode that we have found.**

Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/oh-my-codex/main`
(`git clone --depth 50 https://github.com/Yeachan-Heo/oh-my-codex.git`, HEAD `b48df50`, v0.21.2)

---

## 0. Disambiguation — which "oh my codex"?

`gh search repos` returns several. Only one matters:

| Repo | Stars | Forks | Last push | Verdict |
|---|---|---|---|---|
| **Yeachan-Heo/oh-my-codex** | **32,946** | **2,534** | 2026-09-01 | **THE project.** Teardown target. |
| scalarian/oh-my-codex | 73 | 18 | 2026-04-01 | Different, dormant. "Like oh-my-zsh but for Codex." |
| realsigridjin/oh-my-codex | 20 | 6 | 2026-03-09 | "Sigrid's personal codex plugin." Personal dotfiles. |
| materialofair/oh-my-codex | 12 | 0 | 2026-08-22 | Small skills/rules/prompts bundle. |
| junghwaYang/oh-my-codex | 5 | 2 | 2026-02-13 | Abandoned Python stub. |

The README contains an explicit anti-confusion clause (`README.md:27-31`) disowning forks that
brand themselves "OMX v2". The name is being squatted; that is itself a lesson for us.

---

## 1. Does it exist?

**CONFIRMED.** `https://github.com/Yeachan-Heo/oh-my-codex` — MIT, npm package `oh-my-codex`,
website `oh-my-codex.dev`. Not a README-ware repo: 197,916 lines of TypeScript across 393
non-test source files, plus 6 Rust crates.

Critically, it is **not** what the name suggests. It is not a curated dotfiles bundle. It is a
**full multi-agent orchestration runtime** that happens to install itself as Codex config. The
"oh-my-*" branding is marketing; the architecture is closer to a package manager plus a
supervisor daemon.

---

## 2. What it SHIPS (real counts)

```
skills/          29 dirs, 29 SKILL.md          (find skills -name SKILL.md | wc -l)
prompts/         32 role prompts (*.md)
agents            28 native agent definitions  (src/agents/definitions.ts, AGENT_DEFINITIONS)
plugins/          1 Codex plugin bundle, 24 mirrored SKILL.md
MCP servers       6 first-party (state, memory, code-intel, trace, wiki, hermes)
hook events       7 registered (SessionStart, Pre/PostToolUse, UserPromptSubmit,
                    Pre/PostCompact, Stop)
templates/        4 (AGENTS.md + catalog-manifest.json + 2 model-instruction variants)
docs/           281 files
src/            393 .ts (non-test) + 437 .test.ts  — more test files than source files
crates/           6 Rust crates, 37 .rs files
CLI              49 command modules (src/cli/*.ts)
missions/        13 benchmark/eval missions
```

Notable: **437 test files vs 393 source files.** This project is tested harder than most
commercial software. `package.json` wires 8 separate coverage/verification gates into `npm test`.

The 6 Rust crates (`omx-api`, `omx-explore`, `omx-mux`, `omx-runtime`, `omx-runtime-core`,
`omx-sparkshell`) are compiled to per-platform binaries and shipped as release assets —
**57 assets per release**. So OMX is already a hybrid: JS control plane, Rust hot paths.

### The catalog is a declared SSOT
`src/catalog/manifest.json` is a machine-readable index of every skill and agent with
`category`, `status` (`active`/`deprecated`/`internal`), `core`, and `canonical` (alias target):

```json
{ "name": "ralph",  "status": "deprecated", "core": false,
  "description": "Removed in OMX 0.21; use ultragoal. One-release sunset stub." }
```

This drives install filtering, doc generation (`generate-catalog-docs.js --check` runs in CI),
and **one-release sunset stubs** for retired skills. That is a real deprecation policy encoded
as data, not prose. Steal this.

---

## 3. INSTALL — what it writes to your machine

Install is deliberately **two-phase**, and this is the single best decision in the project.

### Phase 1: `npm install -g oh-my-codex` — writes almost nothing
`src/scripts/postinstall.ts` (153 lines). Read end to end:
- Returns `noop-local` immediately unless `npm_config_global` is set (`isGlobalInstallLifecycle`).
- Hydrates the platform `omx-runtime` Rust binary into the package dir, with a **15s abort
  timeout**, wrapped so any failure is non-fatal (`hydrateOmxRuntimeNonFatal`).
- Writes a version stamp and **prints an advisory**. It does *not* run setup.

The README states this explicitly: *"the global npm install now prints an explicit reminder
instead of launching setup automatically."* They used to auto-run setup and backed it out.

### Phase 2: `omx setup` — the real installer (`src/cli/setup.ts`, 6,469 lines)
Eight announced steps. Scope is chosen deliberately (`--scope user|project`).

`resolveScopeDirectories()` (`src/cli/setup.ts:2512`) is the whole footprint in one function:

```ts
if (scope === "project") {
  const codexHomeDir = join(projectRoot, ".codex");
  return { codexConfigFile: join(codexHomeDir, "config.toml"),
           codexHomeDir,
           codexHooksFile: join(codexHomeDir, "hooks.json"),
           nativeAgentsDir: join(codexHomeDir, "agents"),
           promptsDir:      join(codexHomeDir, "prompts"),
           skillsDir:       join(codexHomeDir, "skills") };
}
// user scope -> ~/.codex/{config.toml,hooks.json,agents,prompts,skills}
```

**Project scope is real isolation, not a flag.** `src/cli/codex-home.ts` walks upward from cwd
to find `.omx/setup-scope.json`, and if scope is `project` it sets `CODEX_HOME` to
`<projectRoot>/.codex` for the launched Codex process. So a project install cannot touch
`~/.codex` at all. An explicit `$CODEX_HOME` in the environment always wins over both.

Full write set:

| Path | Content |
|---|---|
| `~/.codex/config.toml` *or* `<proj>/.codex/config.toml` | marker-delimited OMX block |
| `~/.codex/hooks.json` *or* `<proj>/.codex/hooks.json` | OMX-managed wrapper hook entries only |
| `<scope>/prompts/*.md` | 32 role prompts |
| `<scope>/skills/<name>/SKILL.md` | 29 skills (filtered by catalog status) |
| `<scope>/agents/*.toml` | 28 native agent configs |
| `AGENTS.md` | project root (project scope) or `~/.codex/AGENTS.md` (user scope) |
| `.omx/state/`, `.omx/plans/`, `.omx/logs/` | runtime state |
| `.omx/setup-scope.json` | persisted scope + mcpMode + teamMode + mergeAgents policy |
| `.omx/backups/setup/<ISO-timestamp>/` | pre-write backups, mirrored dir structure |
| `.omx/state/setup/installed-skills.json` | **per-file SHA-256 install receipt** |
| `~/.codex/.omx/install-state.json` | update stamp |
| `.gitignore` | appended entries (project scope only) |

The `.gitignore` entries (`src/cli/setup.ts:272`) are unusually well thought out:

```
.omx/  .omx-state-locks/  .codex/*
!.codex/agents/   !.codex/agents/**
!.codex/skills/   !.codex/skills/**
!.codex/prompts/  !.codex/prompts/**
.codex/skills/.system/**
```

Ignore volatile runtime state; **un-ignore the declarative assets** so agents/skills/prompts
stay commitable and reviewable in the repo. This is exactly the right split and we should copy
the shape verbatim.

### config.toml is edited, never owned
OMX never rewrites `config.toml`. It maintains a fenced region:

```toml
# oh-my-codex (OMX) Configuration
# Managed by omx setup
...
# End oh-my-codex
```

`src/config/generator.ts` (4,300 lines) is a hand-written TOML surgery layer that:
- strips orphaned OMX sections that escaped the fence (`stripOrphanedOmxSections`)
- removes retired tables (`stripLegacyOmxTeamRunTable`) while **refusing to consume past the
  end marker** — there is a literal guard for that, tied to issue #3447
- preserves every user key and table outside the fence (tests assert `model = "o3"` and
  `[user.custom]` survive)
- asserts a **single** marker block after a rebuild (`assertSingleOmxBlock`)

**The best detail in the whole codebase** is `extractCustomizedTuiSectionsFromOmxBlocks`
(`src/config/generator.ts:3390`). Inside its *own* managed block, OMX distinguishes
"value I wrote" from "value the user edited":

- `# omx:managed-status-line` marker present AND value is a known OMX preset → managed, rebuild it
- marker present but value is NOT a preset → **user edited it, preserve**
- no marker but value byte-matches the legacy default → managed (pre-marker installs)
- anything else inside the fence → **treated as user customization and preserved**

That is a three-way merge implemented against a config file it nominally owns. Almost nobody
does this. Most tools treat their fence as a clobber zone.

### Capability negotiation against the host binary
`src/config/codex-feature-flags.ts` parses `codex features list` output and picks which key to
write: current Codex uses `[features].hooks`, older used `[features].codex_hooks`.
`supportsCodexPluginScopedHooks()` gates plugin-scoped hooks on a probed minimum version.
So OMX **negotiates with the compiled host rather than assuming a schema.** Directly applicable
to Muse's ~45 `MUSE_EXPERIMENTAL_*` gates.

---

## 4. EXTENSION CONTRACT — how a user extends without forking

Three genuinely distinct mechanisms, in increasing power.

### (a) Drop-in hook plugins — the real extension point
`src/hooks/extensibility/` + `docs/hooks-extension.md`.

**Contract: drop a `.mjs` file in `.omx/hooks/` that exports `onHookEvent`.** That is the entire
registration protocol. No manifest, no enable-list, no rebuild.

- Discovery (`loader.ts:discoverHookPlugins`): `readdir('.omx/hooks')`, take `*.mjs`, id =
  sanitized basename, **hash-suffixed on id collision**, sorted for determinism.
- Validation: source is regex-scanned for an `onHookEvent` export *before* import
  (`ON_HOOK_EVENT_EXPORT_PATTERN`) — cheap static check, no side effects from a bad file.
- Execution (`plugin-runner.ts`): each plugin runs in a **separate subprocess**, fed a JSON
  envelope on stdin, result returned on stdout behind a `__OMX_PLUGIN_RESULT__` sentinel.
  Default timeout 1500ms (`OMX_HOOK_PLUGIN_TIMEOUT_MS`, clamped 100–60,000), SIGKILL grace 250ms.
- **Enabled by default**; opt out with `OMX_HOOK_PLUGINS=0`.
- Versioned event envelope: `schema_version: "1"`, `event`, `timestamp`, `source`
  (`native`|`derived`), `context`, optional `session_id`/`thread_id`/`turn_id`/`mode`.
- Speculative "derived" events (`needs-input`, `pre/post-tool-use`) are **opt-in**
  (`OMX_HOOK_DERIVED_SIGNALS=1`) and carry a `confidence` float plus `parser_reason`. Honest
  about heuristics — the envelope tells you the signal is guessed.
- Plugins get an SDK (`sdk.ts`): `log` (JSONL to `.omx/logs/hooks-YYYY-MM-DD.jsonl`),
  `state` (namespaced KV under `.omx/state/hooks/plugins/<name>/data.json`, key traversal
  rejected), `tmux` (send-keys with cooldown + dedupe window), `omx` (read-only runtime state).
- **Team safety**: when `OMX_TEAM_WORKER` is set, plugin side effects are suppressed so N
  parallel workers don't each fire the same notification.

Scaffolding: `omx hooks init | status | validate | test`.

This is a genuinely good plugin protocol: subprocess isolation, timeout, static pre-validation,
versioned envelope, namespaced state, deterministic ordering, on-by-default with an env kill switch.

### (b) User policy blocks inside the managed AGENTS.md
`src/utils/agents-md.ts` defines a marker vocabulary:

```
<!-- omx:generated:agents-md -->        provenance stamp
<!-- OMX:AGENTS:START --> / END         OMX-managed region
<!-- USER:OMX:POLICY:START --> / END    USER-OWNED region, carried across regeneration
<!-- OMX:RUNTIME:START --> / END        session-scoped runtime overlay
<!-- OMX:TEAM:WORKER:START --> / END    worker overlay
```

`preserveUserOmxPolicyBlocks(existing, next)` re-injects any `USER:OMX:POLICY` block that the
newly generated content lost. **A reserved user region inside a generated file** — the cheapest
possible escape hatch, and the one most tools forget. Merge policy is itself persisted
(`--merge-agents` / `--no-merge-agents` / `--clear-merge-agents-policy` → `mergeAgents` in
`.omx/setup-scope.json`) and replayed on later `omx update`.

There is also a tamper warning: if an existing `AGENTS.md` lacks OMX contract markers, setup
warns *"it may have been overwritten by another tool"* rather than silently reasserting.

### (c) User skills / prompts — namespace coexistence
`listInstalledSkillDirectories()` (`src/utils/paths.ts:233`) scans `.codex/skills/` then
`~/.codex/skills/`, dedupes by basename, **project wins over user**. A skill is anything with a
`SKILL.md`. Your own skill sitting next to OMX's is a first-class citizen — Codex loads them
identically. `omx skill add|edit|remove|list|search|validate` is a full authoring CLI
(`skills/skill/SKILL.md`).

OMX-installed skills get a badge prefix on their frontmatter description
(`INSTALLED_SKILL_BADGE_PREFIX`) so provenance is visible in-band.

### What is NOT offered
No `custom/` overlay directory that shadows a shipped skill, and no config file listing enabled
modules. To change a *shipped* skill you edit the installed copy — which then trips the receipt
check (section 5) and is preserved, but you have diverged with no rebase path. **This is the
main extension gap.**

---

## 5. UPDATE — how it avoids clobbering you

### Two lockfile-shaped artifacts (they are not the same thing)

**(a) `omx-capabilities.lock.json`** (48KB, repo root) — a **build-time integrity lock over the
project's own assets**, not a user dependency lock. Structure: `surfaces` → `agents`, `skills`,
`configured_tools`, `fixtures`; each has a rollup `digest` plus per-file `{path, digest}`
SHA-256. `npm run verify:capabilities-lock` is a hard gate in `npm test`. Purpose: no one lands
a prompt/skill edit without the change being explicit and reviewed. This is supply-chain
hygiene for prompt assets — a category most agent frameworks have no answer for at all.

**(b) `.omx/state/setup/installed-skills.json`** — the **real user-side lockfile**. A per-file
SHA-256 manifest of exactly what the installer wrote, keyed by skill.

`isUnmodifiedRecordedInstall()` (`src/cli/setup.ts:2386`) re-digests the installed directory and
requires an exact set-and-hash match before OMX will retire or overwrite it. The comment on
`writeInstalledSkillReceipt` is worth quoting because it shows the failure mode they designed
against:

> *"Record digests for EXACTLY the files OMX wrote, keyed by skill. Digesting the destination
> tree instead would poison the receipt: a note a user drops into an active skill directory
> would be recorded as OMX-owned and then deleted when that skill is later retired. The receipt
> therefore describes the installer's own output, and any extra entry present at retirement
> time makes the comparison fail, which retains the directory."*

Fail-safe by construction: unknown state → retain, never delete. They even use
`Object.create(null)` throughout so a file literally named `__proto__` cannot slip the
comparison — a genuinely paranoid detail.

### Backups
`ensureBackup()` copies any file about to change into
`.omx/backups/setup/<ISO-timestamp>/<relative-path>`, mirroring the original tree, before the
write. Every setup run is a restorable snapshot. Backup root is `.omx/` in project scope,
`~/.omx/` in user scope.

### Update flow (`src/cli/update.ts`, 991 lines)
- Throttled npm version check at launch; `OMX_AUTO_UPDATE=0` disables, `=defer` schedules silently.
- **Deferred updates**: an update is scheduled to run *after the current session exits*
  (`update-worker.ts`) — you cannot swap the binary under a running agent. Logged to a file.
- Package-manager ownership detection (`package-manager-ownership.ts`) — npm vs bun, so it
  doesn't `npm install -g` over a bun-owned install.
- `omx update` then replays the persisted setup preferences (scope, mcpMode, teamMode,
  mergeAgents) from `.omx/setup-scope.json`, so a refresh reproduces your choices.
- Shared-ownership refresh of `hooks.json`: only OMX-managed wrapper entries that invoke
  `dist/scripts/codex-native-hook.js` are rewritten; foreign hook entries are preserved.

### Verification
`omx doctor` (`src/cli/doctor.ts`, ~3,900 lines) checks install shape, config coherence, plugin
cache manifest presence, hook trust state, and legacy-path overlap (e.g. it warns when both
`~/.codex/skills` and legacy `~/.agents/skills` hold same-named skills with *different hashes*).
Error messages name the exact remediation command. `omx exec` separately proves the runtime can
actually authenticate and complete a model call.

---

## 6. REGISTRY — there isn't one (and this is the biggest weakness)

`.agents/plugins/marketplace.json` looks like a marketplace. It is not. It is a
**self-registration manifest with exactly one entry — OMX itself**:

```json
{ "name": "oh-my-codex-local",
  "plugins": [{ "name": "oh-my-codex",
                "source": { "source": "local", "path": "./plugins/oh-my-codex" },
                "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" } }] }
```

`src/cli/plugin-marketplace.ts` (2,723 lines) exists solely to register OMX's own npm-installed
directory into Codex's plugin store as a `local` source. There is **no index of third-party OMX
skills, hooks, or agents.** `grep -rn "third-party|community plugin|add your own plugin" docs/
README.md` → zero hits.

Discovery of community content is entirely ad hoc: Discord, the `awesome-ai-plugins` list, and
name-squatted forks. Given 32.9k stars and 2.5k forks, the absence of a registry is the clearest
unmet need in this ecosystem — and the clearest opening for oh-my-musecode.

### The host's registry is the real one
Codex itself has the registry OMX lacks. `openai/codex` `codex-rs/core-plugins/` implements
`marketplace.rs`, `store.rs`, `loader.rs`, `manifest.rs`, `remote_bundle.rs`, `npm_source.rs`,
with `MarketplacePluginSource = Local { path } | Git { .. } | Npm { .. }` plus
`MarketplacePluginInstallPolicy` and `MarketplacePluginAuthPolicy`. OMX uses only the `Local`
arm. **The compiled host already solved distribution; the framework declined to use it.**

---

## 7. UNINSTALL — clean, and unusually rigorous

`src/cli/uninstall.ts`, 2,076 lines. Six steps, and it is a **compensating transaction** with
named failure-injection stages for testing:
`before-hooks-commit`, `before-shim-removal`, `before-config-commit`, `before-rename`,
`before-rollback`, `after-staged-cleanup`, … (17 stages). Hooks and shim are committed first;
the config mutation is committed **last** so hooks never reference unwritten trust state.
There is a durable claim journal (`native-hook-claim-journal.ts`) so a crash mid-uninstall is
recoverable, plus explicit fsync via `utils/file-durability.ts`.

Removal is **name-scoped against what shipped**, so user content survives:
- `removeInstalledSkills` iterates `pkgRoot/skills/*` and removes only those names from the
  install dir. Your `~/.codex/skills/my-thing/` is untouched.
- `removeAgentConfigs` removes only `<name>.toml` for names in `AGENT_DEFINITIONS`.
- `removeInstalledPrompts` likewise diffed against the shipped `prompts/`.
- `config.toml`: strips the OMX fence, env settings, top-level keys, feature flags and managed
  hook trust state — but **preserves `multi_agent` and `hooks` feature flags when user-owned**
  (`preserveHooksFeatureFlag` is set when foreign hooks are detected).
- `hooks.json`: removes only OMX wrapper entries; the file survives if user hooks remain.
- `AGENTS.md`: excises the marked region and **keeps surrounding user guidance**
  (`uninstall.ts:824`), only deleting the file when nothing else remains.

Residue by default: `.omx/` (plans, logs, memory, backups) is intentionally kept — only
`setup-scope.json` and `hud-config.json` are removed. `--purge` removes the whole cache dir.
`--keep-config` skips config cleanup. `--dry-run` is supported throughout.

**This is the cleanest uninstall I have seen in an agent framework.** Most have none at all.

---

## 8. TRACTION (measured 2026-09-01 via `gh` / GitHub + npm APIs)

| Metric | Value |
|---|---|
| Stars | **32,946** |
| Forks | **2,534** |
| Watchers | 83 |
| Created | 2026-02-02 |
| Last push | **2026-09-01** (same day) |
| Latest release | **v0.21.2**, 2026-09-01 |
| Total releases | **134** (100 + 34 across 2 API pages) |
| Release cadence | v0.1.3 (2026-02-13) → v0.21.2 (2026-09-01) = **134 releases / 200 days ≈ 1 per 1.5 days** |
| Release assets | 57 per release (cross-platform Rust binaries) |
| Open issues | **0** |
| Total issues ever | 1,267 |
| Total PRs ever | 2,298 |
| Contributors | **88** |
| npm versions | 131 |
| npm downloads | **4,021/week · 22,247/month** |
| License | MIT |
| Repo size | 32 MB |

**Read the numbers carefully.** 32.9k stars but only 4k weekly npm downloads and 83 watchers —
a ~0.12 downloads-per-star ratio. Compare: a genuinely adopted dev tool runs 1–10 weekly
downloads per star. This is a **star-magnet with modest real usage**. The 2,534 forks against
88 contributors says most forks are bookmarks or rebrands, not contributions.

Also: 1,267 issues closed with **0 open** is not normal maintenance. Combined with 1 release
every 1.5 days and 2,298 PRs in 200 days, this is a repo being driven at machine cadence by its
own agent tooling. Impressive, and a caution: velocity here is not the same as ecosystem health.

For scale, the host: `openai/codex` has 120,644 stars, release `rust-v0.152.0`.

---

## 9. What is GOOD and what is BAD

### Genuinely good — steal these

1. **Two-phase install.** Package manager installs the binary; a separate explicit `setup`
   writes config. They tried auto-setup-on-install and reverted it. Non-negotiable for us.
2. **Marker-fenced config editing with intra-fence user detection.** Not just "own a block" but
   "distinguish my value from your edit *inside* my own block." (`generator.ts:3390`)
3. **The install receipt keyed to installer output, not the destination tree.** Fail-safe
   retention: an unknown file makes the comparison fail, which *keeps* the directory. The
   comment explaining why is better documentation than most projects' architecture docs.
4. **Reserved user region in a generated file** — `<!-- USER:OMX:POLICY:START -->`. Costs ~40
   lines, removes the most common reason to fork.
5. **Scope isolation via the host's own env var.** Project scope sets `CODEX_HOME` to
   `<proj>/.codex`, walking upward from cwd so subdirectories work (issue #3447). A project
   install provably cannot touch `~/.codex`.
6. **The `.gitignore` split**: ignore runtime state, un-ignore declarative assets. Skills,
   agents and prompts stay reviewable in git; logs and locks don't.
7. **Capability negotiation against the compiled host** (`codex features list` → choose
   `hooks` vs `codex_hooks`; version-gate plugin hooks). Never assume the binary's schema.
8. **Hook plugin protocol**: subprocess isolation, static pre-validation before import, bounded
   timeout, versioned envelope, namespaced state, deterministic ordering, on-by-default with an
   env kill switch, side effects suppressed in worker sessions.
9. **Confidence-tagged derived events.** Heuristic signals are labeled `source: "derived"` with
   a `confidence` float. Honest telemetry.
10. **Catalog-as-data with sunset stubs.** `status: "deprecated"` + a one-release stub that
    tells the user what replaced it. Deprecation as a first-class lifecycle state.
11. **Transactional uninstall with a durable claim journal**, and removal scoped to shipped
    names so user content survives.
12. **Asset integrity lock in CI.** Prompt/skill changes cannot land unreviewed.

### Genuinely bad — do not repeat

1. **No third-party registry.** The single biggest gap. 2,534 forks and no way to publish a
   skill. Meanwhile the host (Codex) ships `Local | Git | Npm` marketplace sources that OMX
   uses only the `Local` arm of. Inexcusable and directly exploitable by us.
2. **No override/overlay for shipped content.** To tweak a bundled skill you edit the installed
   copy and diverge permanently. There is no `custom/` shadow dir, no per-skill "extends", no
   rebase path on update. This is what forces the forks.
3. **`setup.ts` is 6,469 lines and `generator.ts` is 4,300.** The TOML surgery is hand-rolled
   line-scanning with regexes — `@iarna/toml` is a dependency but round-tripping is avoided to
   preserve formatting. Correct trade-off, terrifying implementation. The 437 test files are
   load-bearing; without them this would be unmaintainable.
4. **Config surface sprawl.** Env vars (`OMX_HOOK_PLUGINS`, `OMX_ROOT`, `OMX_STATE_ROOT`,
   `OMX_AUTO_UPDATE`, `OMX_TEAM_WORKER`, `OMX_SKIP_NATIVE_AGENT_REFRESH`, …), `setup-scope.json`,
   CLI flags, and `config.toml` all steer behavior with non-obvious precedence.
5. **The `--merge-agents` policy documentation is impenetrable.** README spends ~500 words on
   one tri-state flag, hedging every clause. When docs read like a legal settlement, the design
   is too complicated. A feature needing that much prose should be redesigned.
6. **Prompt-layer over-reach.** `templates/AGENTS.md` opens with shouted all-caps
   ("YOU ARE AN AUTONOMOUS CODING AGENT… DO NOT STOP TO ASK"). It ships an autonomy override
   into every workspace by default. That is a safety posture chosen by the framework, not the
   user, and it is exactly the kind of thing that should be opt-in.
7. **Name squatting went unmanaged.** Needing an anti-confusion clause in your own README
   (`README.md:27-31`) means the namespace was lost. Claim `oh-my-musecode` on npm and the
   plugin registry on day one.
8. **Platform honesty gap.** A "CAUTION" table admits Windows and the Codex App "may break or
   behave inconsistently" — yet the tmux-dependent SDK is central to the hook story.

---

## 10. Transfer to Muse Code (compiled Rust binary host)

### Transfers directly — the architecture is host-agnostic

The essential insight: **OMX never links against Codex.** It is an out-of-process config
generator plus subprocess hooks. Everything it does is (a) write declarative files the host
reads, and (b) get invoked as a subprocess by the host. Both work identically against a Rust
binary. Nothing here depends on Codex being JavaScript.

| OMX mechanism | Muse Code equivalent |
|---|---|
| `.codex-plugin/plugin.json` | **`.muse-plugin/plugin.json` — the loader already reads `.codex-plugin`** |
| `~/.codex/config.toml` fenced block | fenced block in Muse's settings file |
| `.codex/hooks.json` shared-ownership | `.muse/hooks.json` — same wrapper-entry ownership model |
| `CODEX_HOME` project scoping | `MUSE_HOME` (or equivalent) → `<proj>/.muse` |
| `codex features list` probing | probe the ~45 `MUSE_EXPERIMENTAL_*` gates before writing config |
| `.omx/state/setup/installed-skills.json` | per-file SHA-256 receipt, unchanged |
| `omx-capabilities.lock.json` | asset integrity lock in CI, unchanged |
| `AGENTS.md` marker vocabulary | Muse's rules/agent-definitions files, same markers |
| catalog manifest + sunset stubs | unchanged, pure data |
| `.gitignore` ignore/un-ignore split | unchanged |
| two-phase install | unchanged |
| transactional uninstall + claim journal | unchanged (arguably easier in Rust) |

**The `.codex-plugin` finding is the headline.** Muse Code's loader recognising `.claude-plugin`
and `.codex-plugin` is not incidental — the upstream Codex `plugin.json` schema is documented at
`codex-rs/skills/src/assets/samples/plugin-creator/references/plugin-json-spec.md` and OMX's
bundle at `plugins/oh-my-codex/.codex-plugin/plugin.json` conforms to it exactly:

```json
{ "name", "version", "description", "author": {...}, "homepage", "repository",
  "license", "keywords",
  "skills": "./skills/", "hooks": "./hooks/hooks.json",
  "mcpServers": "./.mcp.json", "apps": "./.app.json",
  "interface": { "displayName", "shortDescription", "longDescription",
                 "developerName", "category" } }
```

Per the spec, `skills`/`hooks`/`mcpServers` are **supplemental to default discovery, not
replacements** — so a plugin adds surfaces without suppressing the host's own. If oh-my-musecode
emits a manifest of this exact shape, **one bundle installs into Muse Code, Codex CLI, and
Claude Code.** That is a distribution multiplier no competitor currently has, and it costs us
nothing but schema discipline.

Muse also has two things Codex lacks that make this *easier*:
`.muse/lock.json` (provenance/quarantine/allowed_tools) is a first-class host-side trust
primitive — OMX had to hand-roll "managed hook trust state" inside `config.toml` comments.
And `.muse/skills.lock` means the host may do receipt-keeping for us.

### Does NOT transfer

1. **The Node runtime assumption.** Every hook is `node "${PLUGIN_ROOT}/hooks/codex-native-hook.mjs"`
   and every plugin is a `.mjs` module loaded by dynamic `import()`. Against a Rust host with no
   bundled Node, requiring Node for hooks is a hard dependency users won't have. **We need a
   process-level contract, not a module-level one**: hooks as executables speaking the JSON
   envelope over stdin/stdout. Keep OMX's envelope schema and sentinel-prefixed result protocol
   verbatim — those are language-neutral. Drop `onHookEvent` as an ESM export; the equivalent
   static pre-validation becomes a manifest declaration or a `--describe` probe call.
2. **`ON_HOOK_EVENT_EXPORT_PATTERN` regex validation.** Only meaningful for JS source. Replace
   with manifest-declared event subscriptions — better anyway, since it avoids spawning plugins
   that don't handle the event.
3. **The npm distribution spine.** `npm install -g`, `postinstall`, package-manager ownership
   detection, deferred `npm install -g` after session exit — all npm-shaped. A Rust binary host
   wants a versioned archive plus checksum, or Muse's own plugin store (`Git`/`Npm`/`Local`
   sources exist upstream). The *deferred update* idea transfers and is important: never swap
   assets under a running agent.
4. **The tmux SDK.** `sdk/tmux.ts` shells out to `tmux send-keys` to inject text into the host's
   pane. This is a workaround for Codex not exposing an input channel. If Muse's MSP stdio host
   (`muse serve`) exposes session input properly, use MSP and delete this entire category.
   Do not port a terminal-scraping hack into a protocol-having host.
5. **Hand-rolled TOML line surgery.** In Rust, `toml_edit` is format-preserving and gives us
   fenced-region editing with real spans instead of 4,300 lines of regex. This is a case where
   the Rust ecosystem is strictly better — we get OMX's hardest-won correctness properties for
   free.
6. **The 197k-line orchestration runtime itself.** Team mode, HUD, MCP servers, ralph/ultragoal
   loops, the Rust sparkshell — that is a *product*, not a config framework. oh-my-musecode
   should be the packaging/curation/lifecycle layer OMX buried inside a supervisor. Ship the
   installer, the receipt, the marker contract, the plugin protocol, and the registry. Do not
   ship an agent runtime.

### The strategic read

OMX proves the model works — 33k stars for "config framework wrapped around a coding agent" —
and simultaneously shows the hole: **no registry, no override mechanism.** Those are the two
things that made oh-my-zsh actually oh-my-zsh, and OMX has neither. It won attention on bundled
content and lost the ecosystem to forks.

For oh-my-musecode the differentiated position is clear: **be the smaller, registry-first,
override-first framework**, emit a `plugin.json` that installs into all three agents, and use
Muse's `lock.json` provenance primitive as the trust layer OMX had to fake. Copy OMX's
lifecycle engineering — the receipt, the fences, the backups, the transactional uninstall,
the deferred update — and refuse to copy its scope.

---

## Verification

**Adversarial re-verification, 2026-09-01.** Independent fresh clone at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-oh-my-codex/repo`
(`git clone --depth 50`, HEAD `b48df502d566430be768d3c60f2f51fa91630396`, "Merge pull request #3603 from
Yeachan-Heo/release/0.21.2", 2026-09-01 13:17:53 +0900). Traction re-measured via `gh api` against the
GitHub REST API, the npm registry, and `api.npmjs.org/downloads`. Upstream `openai/codex` claims checked
against the live repo via the GitHub contents API.

**Verdict: MOSTLY_SOLID.** The repo is real and the teardown is unusually accurate — dozens of
line-number, byte-count and verbatim-string claims reproduce exactly. Four claims do not survive:
one invented invocation surface, two overstated safety properties inside the headline extension
contract, and one fabricated grep result (whose conclusion nonetheless holds).

### Priority 1 — repo exists, traction numbers (ALL EXACT)

Every number in section 8 reproduces to the digit. `https://github.com/Yeachan-Heo/oh-my-codex`
resolves; `fork: false`.

| Claim | Measured | |
|---|---|---|
| 32,946 stars | 32,946 | exact |
| 2,534 forks | 2,534 | exact |
| 83 watchers/subscribers | `subscribers_count` 83 | exact |
| 88 contributors | 88 | exact |
| created 2026-02-02 | 2026-02-02T14:21:18Z | exact |
| last push 2026-09-01 | 2026-09-01T06:50:52Z | exact |
| latest release v0.21.2, 2026-09-01 | v0.21.2, 2026-09-01T05:16:49Z | exact |
| 134 releases (100 + 34) | page1=100, page2=34, page3=0 | exact |
| oldest v0.1.3 on 2026-02-13 | v0.1.3, 2026-02-13T11:50:43Z | exact |
| 57 assets per release | 57 on each of the last 5 releases | exact |
| 0 open issues, 1,267 total | 0 open, 1,267 | exact |
| 2,298 PRs total | 2,298 | exact |
| diskUsage 31,965 KB | 31,965 | exact |
| MIT in package.json, `licenseInfo` null | `license: None` on the API; `"license":"MIT"` in package.json | exact |
| npm 131 versions, latest 0.21.2, first 2026-02-13 | 131, 0.21.2, created 2026-02-13T11:12:28Z | exact |
| 4,021 weekly / 22,247 monthly downloads | 4,021 / 22,247 | exact |
| openai/codex 120,644 stars, rust-v0.152.0 | 120,644, rust-v0.152.0 | exact |
| scalarian 73 (dormant 2026-04-01), realsigridjin 20, materialofair 12 | 73/2026-04-01, 20, 12 | exact |

The 0.12-downloads-per-star ratio is arithmetic on verified inputs and stands.

### Priority 2 — shipped-content counts (ALL CONFIRMED)

Re-run independently: `find skills -name SKILL.md | wc -l` = **29**; `prompts/*.md` = **32**;
`find plugins -name SKILL.md` = **24**; `AGENT_DEFINITIONS` keys = **28**; `templates/` files = **4**;
`src/cli/*.ts` non-test = **49**; crates = **6**, `.rs` files = **37**; `docs/` files = **281**;
`missions/` = 13 dirs + README = **13**; `omx-capabilities.lock.json` = 48,404 B with
`kind:"omx_capabilities_lock", version:1, surfaces:{agents(34), configured_tools, fixtures, skills(29)}`.

The LOC/file counts are reproducible only with the exact filter the report used, and they then match
to the line: `find src -name '*.ts' ! -name '*.test.ts' ! -path '*__tests__*'` = **393 files /
197,916 lines** (exact), and `find src -name '*.test.ts'` = **437** (442 repo-wide; the extra 5 are
`packages/vscode-extension`). `plugins/oh-my-codex/.mcp.json` = 6 servers, all `"enabled": false`;
`hooks/hooks.json` = 7 events (SessionStart/PreToolUse/PostToolUse/UserPromptSubmit/PreCompact/
PostCompact/Stop), every one invoking `node "${PLUGIN_ROOT}/hooks/codex-native-hook.mjs"`.

File landmarks all land: `setup.ts` 6,469 · `generator.ts` 4,300 · `uninstall.ts` 2,076 ·
`update.ts` 991 · `plugin-marketplace.ts` 2,723 · `postinstall.ts` 153 ·
`resolveScopeDirectories` at setup.ts:2512 · `extractCustomizedTuiSectionsFromOmxBlocks` at
generator.ts:3390 · anti-confusion clause at README.md:27 · AGENTS.md excision at uninstall.ts:824.

### Priority 3 — extension contract: REAL, but two safety properties are overstated

Confirmed verbatim in `src/hooks/extensibility/`: `.omx/hooks/` readdir + `.mjs` filter, sanitized
basename id with `sha256(fileName).slice(0,8)` collision suffix, deterministic `localeCompare` sort
(loader.ts:79); separate subprocess per plugin (dispatcher.ts spawns `dist/hooks/extensibility/
plugin-runner.js`); `__OMX_PLUGIN_RESULT__ ` sentinel; 1500 ms default clamped to 100–60,000 via
`OMX_HOOK_PLUGIN_TIMEOUT_MS`; `RUNNER_SIGKILL_GRACE_MS = 250`; envelope `{schema_version:"1", event,
timestamp, source, context}` + optional `session_id/thread_id/turn_id/mode`; `DERIVED_EVENTS = {needs-input,
pre-tool-use, post-tool-use}` with clamped `confidence` (default 0.5) and `parser_reason`;
`OMX_HOOK_DERIVED_SIGNALS !== '1'` gates the derived watcher; `OMX_TEAM_WORKER` suppresses side
effects; SDK = `logging/paths/plugin-state/runtime-state/tmux` with `if (trimmed.includes('..') ||
trimmed.startsWith('/'))` traversal rejection; `omx hooks init|status|validate|test` all present.
Claims (2) and (3) also check out: all five AGENTS.md markers plus `OMX:RUNTIME:*` and
`OMX:TEAM:WORKER:*` overlays exist; setup.ts:4948 warns when an existing AGENTS.md lacks OMX contract
markers; `listInstalledSkillDirectories` is at paths.ts:233 with project-wins-over-user dedupe by
basename; `INSTALLED_SKILL_BADGE_PREFIX = "[OMX] "`.

**REFUTED — "regex-scanned for the export BEFORE import ... so a bad file causes no side effects."**
Not on the runtime path. `dispatchHookEvent` calls `discoverHookPlugins` (dispatcher.ts:382), which does
**no** validation. The `ON_HOOK_EVENT_EXPORT_PATTERN` scan lives in `validatePluginExport`, reached only
through `loadHookPluginDescriptors` / `validateHookPluginExport`, whose sole non-test caller is
`src/cli/hooks.ts` (the `status`/`validate` subcommands). At dispatch, `plugin-runner.ts` does
`await import(moduleUrl)` **first** and checks `typeof loaded.onHookEvent !== 'function'` **after** — so
a malformed plugin's top-level side effects do run. Static pre-validation is an operator-facing lint,
not a runtime guard.

**REFUTED — "kill switch `OMX_HOOK_PLUGINS=0`."** Inert on the dispatch path.
`shouldForceEnableRuntimeHookDispatch(event)` returns true when `event.source` is `native` or `derived`,
and `HookEventSource` is exactly `'native' | 'derived'` (types.ts:2) — so `enabled = options.enabled ??
(true || isHookPluginsEnabled(env))` is unconditionally true. `dispatchHookEventRuntime` hardcodes the
same short-circuit. The env var only changes what `omx hooks status` prints; it does not stop plugins
from being spawned. For oh-my-musecode this inverts the lesson: OMX's kill switch is the *counter*example.

**CORRECTED — "`omx skill add|edit|remove|list|search|validate|sync|scan`."** There is no such CLI.
No `src/cli/skill.ts`; no `"skill"`/`"skills"` member in the `CliCommand` union (index.ts:435–476);
`grep -rn "omx skill"` over `*.ts`/`*.md`/`*.json` returns zero hits. The real surface is an in-agent
markdown skill invoked as `/skill list|add|edit|remove|search|info|sync|setup|scan|validate`, and
`skills/skill/` contains **only** a 4,624-byte `SKILL.md` — no scripts, no code. The capability is
LLM-followed prose, not a validated command. `info`, not `list`, is the metadata subcommand. This
matters for the design: OMX's skill authoring has no deterministic validation behind it.

**CORRECTED — "`preserveUserOmxPolicyBlocks` re-injected on every regeneration."** Preserved on the
plugin-mode default path (setup.ts:4845) and inside `upsertManagedAgentsBlock`'s generated-file fallback
(agents-md.ts:112). But the legacy non-merge refresh branch assigns `managedRefreshContent = rewritten`
directly when the existing file is OMX-generated (setup.ts:4966–4968), and `rewritten` is built purely
from `templates/AGENTS.md` (setup.ts:4924) — it carries no user content, so `USER:OMX:POLICY` blocks are
dropped on that path. `ensureBackup` makes this recoverable, not preserved. The reserved-region idea is
still the right one to port; OMX does not yet apply it uniformly.

**PRECISION — `extractCustomizedTuiSectionsFromOmxBlocks`.** The four-way marker/preset logic and its
source comment are verbatim as quoted. But it is scoped to exactly one key, `[tui].status_line`, not a
general three-way merge over everything inside the fence. Still worth copying; smaller than billed.

### Priority 4 — install / update / uninstall (FAITHFUL)

`postinstall.ts` read end to end and matches line for line: `noop-local` unless
`isGlobalInstallLifecycle` (`npm_config_global` truthy or `npm_config_location === "global"`); runtime
hydration behind a 15,000 ms `AbortController`, wrapped non-fatal; version stamp; advisory reading
`"OMX setup is explicit opt-in; run \`omx setup\` or \`omx update\` when you're ready."` No setup call
anywhere in the file. README.md:89 carries the revert quote verbatim.

`setup.ts` announces exactly `[1/8]`…`[8/8]` in the claimed order (lines 4375–5088).
`resolveScopeDirectories` (2512) is the whole footprint in one function and returns `<projectRoot>/.codex`
for project scope. `src/cli/codex-home.ts` is 43 lines and confirms the isolation claim including the
comment *"Walk upward so a project-scoped setup is honored from any directory inside its tree — launching
from a subdirectory must not fall through to the user config (issue #3447)"*, with `env.CODEX_HOME`
winning over both. Config markers `OMX_CONFIG_MARKER = "oh-my-codex (OMX) Configuration"` /
`OMX_CONFIG_END_MARKER = "# End oh-my-codex"` confirmed at generator.ts:251/253. `.gitignore` split
confirmed byte-for-byte (`.omx/`, `.omx-state-locks/`, `.codex/*` with `!.codex/{agents,skills,prompts}/**`).

The install receipt is the report's strongest verified claim: `InstalledSkillReceipt`,
`isUnmodifiedRecordedInstall` (setup.ts:2387 — report said 2386), `Object.create(null)` in all three
places with the `__proto__` rationale spelled out, non-regular entries recorded as a `"non-regular-entry"`
sentinel so a symlinked dir can never compare equal, and the design comment *"Digesting the destination
tree instead would poison the receipt"* verbatim. Backup roots `<projectRoot>/.omx/backups/setup/<ts>` and
`~/.omx/backups/setup/<ts>` confirmed at setup.ts:449/454. `setup-scope.json` carries
`scope/mcpMode/teamMode/mergeAgents`; `update-worker.ts` exists; `OMX_AUTO_UPDATE` parsed by
`resolveAutoUpdateMode`; bun-vs-npm ownership detection present at update.ts:180/364/541/897.

Uninstall: `UninstallTransactionFailureStage` is a union of exactly **17** named stages, six announced
`[n/6]` steps with config committed last, `preserveHooksFeatureFlag = hooksRemoval.plan?.hasForeignHooks`
(uninstall.ts:1871), AGENTS.md excision preserving surrounding guidance (824), `--purge`/`--keep-config`/
`--dry-run` all wired, closing string verbatim. The claim journal is at `src/cli/native-hook-claim-journal.ts`
alongside `src/utils/file-durability.ts`.

### Registry claim

`.agents/plugins/marketplace.json` confirmed: one plugin, `{"source":{"source":"local","path":
"./plugins/oh-my-codex"},"policy":{"installation":"AVAILABLE","authentication":"ON_INSTALL"}}`. The
upstream irony is real — `openai/codex` `codex-rs/core-plugins/src/` contains `marketplace.rs`,
`store.rs`, `loader.rs`, `manifest.rs`, `remote_bundle.rs`, `npm_source.rs`, and marketplace.rs:127
declares `pub enum MarketplacePluginSource { Local { path }, Git { url, path, ref_name, sha },
Npm { package, version, registry } }`. The spec at
`codex-rs/skills/src/assets/samples/plugin-creator/references/plugin-json-spec.md` exists (9,179 B) and
line 117 reads *"`skills`, `hooks`, and string-valued `mcpServers` are supplemented on top of default
component discovery; they do not replace defaults"* — the headline transfer claim holds, and OMX's
`plugin.json` conforms to the documented field list exactly.

**REFUTED (evidence, not conclusion) — "`grep -rn "third-party|community plugin|add your own plugin"`
returns ZERO hits."** As written that is a literal-string grep with no `-E`, so of course it matches
nothing. `grep -rniE` returns three hits: README.md:27, docs/index.html:26, docs/getting-started.html:33
— all fork-disowning disclaimers, none a publishing path. The conclusion (no third-party registry)
stands; the cited proof does not. Do not reuse that command.

### Other corrections

- `doctor.ts` is **4,172** lines, not "~3,900".
- `codex features list` is parsed in **`src/cli/codex-feature-probe.ts:96`**, not
  `src/config/codex-feature-flags.ts` (that file holds `CODEX_HOOK_FEATURE_FLAGS = ["hooks","codex_hooks"]`
  and `supportsCodexPluginScopedHooks`). The capability-negotiation claim itself is correct.
- Catalog manifest statuses are **`active | merged | deprecated | internal | alias`** (39/14/6/3/1 across
  63 entries), not just active/deprecated/internal. `core` flag and `canonical` field confirmed; the
  25-skill sunset-stub migration is documented in `docs/release-notes-0.21.0.md`.
- "8 verification gates wired into `npm test`": the script chains **7** verify/check steps
  (`verify:native-agents`, `verify:plugin-bundle`, `verify:capabilities-lock`, `verify:prompt-guidance`,
  `test:node`, `generate-catalog-docs --check`, `prompt-inventory --check`) after `npm run build` — 8
  only if `build` counts.
- "README spends ~500 words hedging one tri-state flag": actual is **173 words** across 4 sentences, the
  bulk in a single 110-word bullet at README.md:373. The substantive criticism (legalistic, unreadable
  policy prose) is fair; the magnitude is ~3x inflated.
- Star count read 32,945 via the search API and 32,946 via REST minutes apart — live drift, not an error.

### Unchanged assessments

The two-phase install, marker-fenced config editing, install receipt, scope isolation via `CODEX_HOME`,
`.gitignore` ignore/un-ignore split, transactional uninstall, deferred update, asset-integrity lock,
Node-runtime non-transferability, and the "no registry / no override overlay" diagnosis all verified.
No `custom/`, `overlays/`, per-skill `extends`, or enabled-modules config file exists anywhere in the
tree — the override gap is real. The strategic read for oh-my-musecode is unaffected, with two
amendments: OMX's hook kill switch and its pre-import validation are **not** patterns to copy (both are
non-functional as documented), and its skill-authoring "CLI" is a markdown prompt, so a real, code-backed
authoring and validation command remains an open opportunity rather than table stakes to match.
