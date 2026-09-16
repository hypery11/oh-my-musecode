# Teardown: `first-fluke/oh-my-agent` (oma)

**Verified: CONFIRMED / EXISTS.** `curl -o /dev/null -w %{http_code} https://github.com/first-fluke/oh-my-agent` → `200`.
Cloned `--depth 50` to `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/first-fluke-oh-my-agent/`.
HEAD at teardown time: `75b3fbc` (2026-09-01, `ci: sync prompt-manifest.json [skip ci]`), version **13.1.1**.

Everything below is read from source in that clone. Paths are repo-relative unless noted.

---

## 0. One-paragraph summary

`oma` is **not** an oh-my-zsh clone. It is a **compiler + reconciler**: it keeps one vendor-neutral
source of truth at `.agents/` (skills, workflows, rules, abstract agent definitions, hook handlers,
config), and *projects* it into ~13 host-agent-native layouts (`.claude/`, `.codex/`, `.cursor/`,
`.qwen/`, `.grok/`, `.kiro/`, `.pi/`, `.opencode/`, `.zcode/`, `.github/`, `.kimi-code/`,
`.commandcode/`, `.gemini/antigravity-cli/`). The projection is driven by **declarative per-vendor
adapter JSON** (`.agents/hooks/variants/<vendor>.json`, JSON-Schema'd), not per-vendor code branches.
Hook execution is collapsed into **one dispatcher process per event** (`oma hook --vendor X --event Y`)
instead of N script spawns. Ownership of written files is tracked by **in-band markers**, and
`oma uninstall` builds an explicit oma-owned vs. user-owned removal plan with a dry-run preview.
Its weakest joint — and the one most relevant to us — is that **content updates are a wholesale
`cpSync(force:true)` of an unverified `main`-branch tarball**, so user edits to any shipped asset
are silently clobbered and there is no real supply-chain gate despite a sha256 manifest existing
in the repo.

---

## 1. Does it exist?

**CONFIRMED.** Real repo, real code, 3,007 tracked files, 875 TypeScript files under `cli/`
(554 non-test + 321 test), 51 numbered migrations, active same-day commits.

---

## 2. What does it SHIP? (real counts)

Counted with `find … | wc -l` inside the clone.

| Category | Count | Path evidence |
|---|---|---|
| **Skills** (`SKILL.md`) | **33** (+ `_shared/`) | `.agents/skills/*/SKILL.md`; dir count 34 incl. `_shared` |
| Skill support files (`resources/`) | 203 | `find .agents/skills -path '*resources*' -type f` |
| `_shared/` assets (`core/`, `conditional/`, `runtime/`) | 32 | `.agents/skills/_shared/` |
| **Subagents** (vendor-neutral) | **12** | `.agents/agents/*.md` |
| Subagent **vendor variants** | 7 + 1 schema | `.agents/agents/variants/{claude,codex,commandcode,cursor,grok,kiro,opencode}.json`, `agent-variant.schema.json` |
| **Workflows** (slash-commands) | **21** `.md` (24 files incl. `ralph/resources`, `ultrawork/resources`) | `.agents/workflows/` |
| **Rules** (context docs) | **13** | `.agents/rules/*.md` |
| **Hook handler scripts** | **23** | `.agents/hooks/core/` (`keyword-detector.ts`, `skill-injector.ts`, `state-boundary.ts`, `scm-guard.ts`, `refactor-guard.ts`, `test-filter.ts`, `persistent-mode.ts`, `serena-primer.ts`, `hud.ts`, …) |
| **Hook vendor variants** (adapter manifests) | **10** JSON (9 vendors + schema) | `.agents/hooks/variants/{antigravity,claude,codex,commandcode,cursor,grok,kimi,kiro,qwen}.json` + `hook-variant.schema.json` |
| Extension-vendor hook bridges | 2 | `.agents/hooks/variants/opencode/oma.ts`, `.agents/hooks/variants/pi/index.ts` |
| **MCP configs** | 4 | `.mcp.json` (repo dev), `mcp.json` (Agent-Plugins emit), `.agents/mcp.json` (shipped), `.agents/mcp_config.json` |
| **Statusline** | 1 | `.agents/hooks/core/hud.ts`, wired via `statusLine`/`statusLineKey` in each variant |
| **Settings presets** (model routing) | 9 named + 10 install presets | `cli/platform/built-in-presets.ts` (antigravity/claude/codex/…); README preset table (All, Backend, Content, DevOps, Frontend, Fullstack, Fullstack Mobile, Fullstack Web, Mobile, Research) |
| **Output styles** | **0** | no `output-styles/` anywhere |
| **Prompts** | generated, not shipped | `.github/prompts/*.prompt.md` emitted by `installCopilotWorkflowPrompts`; `.pi/prompts/` by `cli/platform/pi-prompts.ts` |
| **CLI commands** | 38 top-level dirs | `cli/commands/` |
| Eval fixtures | 20 files / 3 suites | `.agents/eval/{oma-docs,oma-market,oma-scm}` (deleted at install time — see §3) |
| Distribution artifacts | — | `plugin.json`, `.claude-plugin/{plugin,marketplace}.json`, `.cursor-plugin/*`, `com.firstfluke.oma/`, `skills/` (all **generated** by `oma emit`) |
| Docs | 31 md in `web/docs` + 12 translated READMEs | `docs/`, `web/docs/` |

Total under `.agents/`: **439 files, 326 markdown.**

Note the **duplication by design**: `skills/` at repo root (277 md) and `com.firstfluke.oma/` are
*emitted copies* of `.agents/skills` and `.agents/{agents,rules,workflows,oma-config.yaml}` —
produced by `oma emit --target agent-plugin` (`cli/platform/emit/agent-plugin.ts`) so a plain
`git clone` of the repo *is* an installable Agent-Plugins 1.0.0 package. `diff -rq skills
.agents/skills` shows only frontmatter reflow differences (YAML line-wrapping), confirming it is
machine-generated, not hand-maintained.

---

## 3. INSTALL — what exactly hits the user's machine

Two layers.

### 3a. Bootstrap: `cli/install.sh` (197 lines) / `cli/install.ps1` (137)

`curl -fsSL …/cli/install.sh | bash`. Reads end-to-end:

- `pick_downloader` → curl or wget; `download_to_stdout` forces `--proto '=https' --tlsv1.2`
  (curl) / `--https-only` (wget). **Good**: explicit refusal to downgrade for a pipe-to-shell installer.
- `detect_platform`: Darwin/Linux only; MINGW/MSYS/CYGWIN → hard fail pointing at `install.ps1`;
  arch normalised to x64/arm64.
- Installs, if missing: **bun** (`curl https://bun.sh/install | bash`), **uv**
  (`https://astral.sh/uv/install.sh`), **serena** (`uv tool install -p 3.13 serena-agent@latest
  --prerelease=allow`). uv/serena failures are warn-and-continue; bun failure is fatal.
- `OMA_INSTALL_NO_RUN=1` escape hatch for CI smoke tests.
- Ends with `exec bunx oh-my-agent@latest < /dev/tty` — the real installer.

So the "installer" is a **third-party runtime provisioner**: three separate `curl | sh` chains
before oma's own code runs.

### 3b. Real installer: `cli/commands/install/run.ts` (547 lines)

Guardrails first (genuinely good, all with named edge-case IDs in comments):

- **Refuses sudo** when `geteuid()===0 && SUDO_USER` set (`run.ts:88-100`).
- **Refuses/­confirms `cwd === homedir()`** without `--global` (`run.ts:135-152`).
- **HOME-write consent prompt** for `--global`, plus a first-run scope note listing exactly what
  will be touched (`run.ts:168-203`).
- **WSL detection** with a $HOME-vs-%USERPROFILE% explanation (`run.ts:155-165`).
- **PID-based install lock** at `<root>/.agents/_install.lock` with stale-PID grace
  (`cli/utils/install-lock.ts`, `run.ts:117-124`).
- Runs 51 migrations twice — once with `vendors: []` (before vendor selection, so no vendor-owned
  file can be touched), once after with the real selection (`run.ts:209`, `run.ts:346`).

Then: `downloadAndExtract()` (`cli/io/tarball.ts`) fetches **`main`** — not a release tag —
trying `api.github.com/repos/…/tarball/main` → `codeload…/tar.gz/main` → `github.com/…/archive/main.tar.gz`,
with a final `git clone --depth 1 --branch main` fallback. Extracted with system `tar` to a mkdtemp.
**No checksum, no signature, no tag pin.**

Writes, per `cli/platform/skills-installer/ssot-install.ts` + `cli/commands/link/run.ts`:

**Into the project (or `$HOME` with `--global`):**
- `.agents/skills/<selected>/` — `cpSync(force:true)` per selected skill; `variants/<lang>` promoted
  to `stack/` with a generated `stack.yaml`, then `variants/` deleted (`ssot-install.ts:7-42`)
- `.agents/skills/_shared/`, `.agents/hooks/`, `.agents/agents/`, `.agents/workflows/`,
  `.agents/rules/` — all `cpSync(force:true)`
- `.agents/config/`, `.agents/mcp.json`, `.agents/oma-config.yaml` — **only if absent**
  (`installConfigs`, `ssot-install.ts:107-158`)
- `.agents/skills/_version.json` — `{schemaVersion:2, version, mode, installedAt}`
- `.agents/eval/` — **deleted** (`run.ts:313-316`)
- `.gitignore` — appends `.antigravitycli/`, `.agents/results/`, `.agents/state/`,
  `.agents/backup/`, `docs/plans/`, `.migration-backup/`, `.qwen/tmp/`
  (`cli/constants/paths.ts:71-79`, `cli/io/gitignore.ts`)

**Per selected vendor (the `link()` kernel, `cli/commands/link/run.ts`, 40+ imports):**
- skill **symlinks** `<vendor>/skills/<skill> → .agents/skills/<skill>` — target dirs from
  `CLI_SKILLS_DIR` in `cli/constants/vendors.ts:110-151` (`.claude/skills`, `.codex/skills`,
  `.cursor/skills`, `.qwen/skills`, `.github/skills`, `.opencode/skills`, `.kiro/skills`,
  `.commandcode/skills`, and HOME-consent ones: `~/.hermes/skills/oma`, `~/.kimi-code/skills`,
  `~/.gemini/antigravity-cli/skills`)
- workflow symlinks (`createVendorWorkflowSymlinks`), and flat `.zcode/commands/*.md`
- generated subagents: `.claude/agents/*.md`, `.codex/agents/*.toml`, `.gemini/agents/*.md`,
  `.grok/agents/` (`cli/platform/agent-composer.ts`)
- hook wiring: `<hookDir>/oma-hook.sh` (0755) + merged entries into the vendor settings file
  (`cli/platform/hooks-composer.ts`)
- `.cursor/rules/*.mdc`, `.cursor/mcp.json`, Claude `.mcp.json` seeding, Codex `config.toml`
  feature flags + telemetry, Qwen/Gemini `settings.json` privacy keys, Claude workspace trust
- **doc merging** — an `<!-- OMA:START … OMA:END -->` block spliced into `CLAUDE.md` / `AGENTS.md`
  (`cli/platform/rules.ts`)
- `.pi/extensions/oma/index.ts`, `.opencode/plugin` registration
- git hooks: `.githooks/commit-msg` co-author guard (`cli/io/git-hooks.ts`), opt-in global
  `git config` recommendations

**Outside any project root:**
- `~/.bun`, `~/.local/bin` (serena, uv)
- `~/.serena/project.yml` + project registration + `default_max_tool_answer_chars`
  (`cli/io/serena.ts`), plus an oma-specific serena *context* file
- `~/.gemini/antigravity-cli/` HUD
- `~/.cache/oma-<id>/<ref>/` for "managed" third-party skills (`cli/platform/managed-skill.ts`)

Plus: it **offers to uninstall competing tools** (`promptUninstallCompetitors`,
`cli/utils/competitors.ts`) — interactive-only, but still an aggressive move.

### 3c. Other install channels
- npm: `oh-my-agent` / bin `oma` — `files: ["bin"]` only, i.e. **the npm package ships no content**;
  all assets come from the GitHub `main` tarball at runtime.
- Homebrew formula `cli/oh-my-agent.rb` (bottled, `depends_on node`, rebuilds `better-sqlite3`),
  auto-bumped by `.github/workflows/bump-homebrew.yml`.
- Claude Code marketplace: `/plugin install oma@oh-my-agent` → `hooks/hooks.json` `SessionStart`
  runs `scripts/plugin-bootstrap.sh`, which **`curl`s `install.sh` from `main` and pipes it to `sh`
  inside the session hook**, then runs `oma install --yes` non-interactively (85 lines, idempotent
  via a `.oma-wired` version marker). Effective behaviour: installing the plugin silently
  provisions bun/uv/serena and rewrites your project.
- Microsoft APM: `apm install first-fluke/oh-my-agent` — skills only, README explicitly warns
  "pick one distribution per project to avoid drift".
- GitHub Action `action/action.yml` — runs `oma update` in CI and opens a PR (Renovate-style).

---

## 4. EXTENSION CONTRACT — can a user add/override without forking?

**Mostly no. This is the design's weakest area, and the repo says so out loud.**

Header of `.agents/oma-config.yaml` (lines 1-7), verbatim:

> The single user-owned file: `oma update` preserves it byte for byte and only appends top-level
> keys new releases introduce. **Every other file under `.agents/` is replaced wholesale on update,
> so this is the only place edits survive.**

What actually exists:

| Mechanism | Where | Verdict |
|---|---|---|
| `.agents/oma-config.yaml` (527 lines, 12 sections) | preserved verbatim; new keys appended by `appendMissingConfigKeys` | Real, but it is *settings*, not content |
| `vendors:` list in that file | `readVendorsFromConfig` → `link()` vendor filter | enable/disable **hosts**, not modules |
| `custom_presets:` / `models:` | `cli/platform/agent-config/types.ts:146` | user model routing without forking — good |
| **User-authored skills** `.agents/skills/<name>/` | survive: `cpSync` never deletes extraneous entries, and `selectSkillsToPrune` filters `name.startsWith("oma-")` (`cli/commands/update/install-state.ts:79`) | **works, but undocumented as a contract and unenforced** |
| User evals `.agents/eval/<skill>/` | documented in `web/docs/guide/skill-eval.md:33` as surviving update | the one explicit "put your stuff here" slot |
| User hook entries in vendor `settings.json` | preserved — `mergeHookGroups` strips only oma-managed groups (`cli/platform/hooks-composer/settings-merge.ts:97-109`) | **excellent** |
| User `.github/prompts/*.prompt.md`, `.zcode/commands/*.md` | preserved unless they carry `<!-- oma:generated -->` | good |
| Overriding a *shipped* skill/workflow/rule/agent | **impossible** — clobbered on next update | the gap |
| Adding a **new vendor** | requires editing `cli/constants/vendors.ts`, adding a variant JSON, and a `cli/vendors/<x>/` adapter → **fork / PR** | the variant JSON is 80% declarative; the last 20% is compiled TS |
| Third-party plugin protocol | **none** | no `oma plugin add` |

There is **no `custom/` directory, no enabled-modules list, no overlay/merge layer.** The only
sanctioned way to change a shipped skill is to upstream it — `web/docs/guide/skill-opt.md:171`
literally says: *"Skills whose ID starts with `oma-` are owned by oh-my-agent and are overwritten by
`oma update`. For these skills, `--apply` is discouraged … upstream changes to the registry."*

`oma skills opt` / `oma skills lint` / `oma skills audit` / `oma skills eval` and the
`oma-skill-creation` skill make **authoring** a first-class activity — but authoring inside a
namespace oma doesn't own.

---

## 5. UPDATE — and how badly it clobbers

`oma update` → `cli/commands/update/run.ts` (520 lines).

1. Acquire the same PID lock. Run migrations with a vendor allow-list.
2. `fetchRemoteManifest()` → GETs `raw.githubusercontent.com/…/main/prompt-manifest.json`
   (`cli/platform/manifest.ts:242`). **Used only to read `.version`** for a string compare.
3. If versions match and no reconcile flag → exit; else `downloadAndExtract()` — again, the
   **unverified `main` tarball**.
4. Save aside exactly three things: `.agents/oma-config.yaml`, `.agents/mcp.json`, and the backend
   `stack/` state (`captureBackendStackBeforeCopy`).
5. **`cpSync(repoDir/.agents → cwd/.agents, { recursive: true, force: true })`** (`run.ts:233-237`),
   filtering only `.agents/eval`. Every shipped skill/workflow/rule/agent/hook file is overwritten.
6. Restore the three saved files; `appendMissingConfigKeys` adds new template keys to the user's
   config and *reports which keys it added*.
7. `selectSkillsToPrune` removes `oma-`-prefixed skills that the release newly ships and the user
   hadn't installed — unless `--with-new-skills`. Skills referenced by a shipped agent's `skills:`
   frontmatter are never pruned (`collectAgentRequiredSkills`) — a nice dangling-dependency guard.
8. `link()` reconciles all vendor projections; symlinks rebuilt; dangling ones pruned.
9. `_version.json` stamped; **all backup dirs deleted** (`run.ts` "Clean up backups"); optional
   `uv tool upgrade serena-agent`; optional detached CLI self-update (`cli/io/self-update.ts`),
   deliberately deferred to after project writes so a stale process can't re-copy old content.

**Is there a lockfile?** Three near-misses, none of which is one:
- `_version.json` — a version stamp (`{schemaVersion, version, mode, installedAt, needsReconcile}`), no per-file data.
- `prompt-manifest.json` — 418 entries of `{path, sha256, size}` + `metadata.{skillCount, workflowCount, totalFiles}`, CI-regenerated (`.github/workflows/sync-manifest.yml`). It **is** a real integrity manifest… and it is **dead weight in production**: the only function that verifies a sha256 is `downloadFile` (`cli/platform/manifest.ts:248-302`), and `grep -rn "downloadFile" cli/` shows **the sole callers are in `manifest.test.ts`**. The tarball path never checks a hash.
- `_install.lock` — a concurrency mutex, not a content lock.

**Ownership is marker-based, not manifest-based.** `docs/plans/designs/023-ownership-manifest.md`
(336 lines, status **Draft — design only, no implementation**; `grep -rn "_ownership.json" cli/`
returns nothing) is a superb write-up of exactly this gap. It enumerates today's five heuristics,
names three real defects they cause (HOME-consent vendors leak symlinks uninstall can never find;
Windows `hardlink`/`copy` link fallbacks get misclassified as user-authored; drift is undetectable
so `link` clobbers blind), and proposes `.agents/_ownership.json` with three ownership classes —
`exclusive` / `shared` (region-scoped, region-hashed) / `seeded` — plus `base: "installRoot"|"home"`,
`writer`, `mechanism`, per-entry `sha256`. **Read this file before designing `.muse/lock.json`.**

---

## 6. REGISTRY

**NONE.** There is no index or marketplace of third-party oma additions.

- `.claude-plugin/marketplace.json` is a *publisher-side* manifest listing oma's own 33 skills and
  12 agents so Claude Code can `marketplace add` this one repo. Same for `.cursor-plugin/`.
- `cli/commands/market/` is the **market-research** skill runner (wraps the upstream `last30days`
  Python skill), not a package marketplace.
- `cli/platform/managed-skill.ts` is the closest thing to a package manager: it pins *specific
  hard-coded upstream repos* (`archify`, `last30days`, remotion skills) into
  `~/.cache/oma-<id>/<ref>/`, channel `stable` (latest release tag) or `main` (HEAD sha), with a
  throttled check, per-ref directories so an in-flight run never sees a half-written tree, prune on
  success, and network failure → reuse cache + report `status: "stale"`. **This is a genuinely good
  micro-package-manager — but its spec list is compiled in, not user-extensible.**
- Distribution is fan-out, not federation: npm, Homebrew tap, Claude Code marketplace, Cursor
  plugin, Agent-Plugins 1.0.0 package, Microsoft APM, GitHub Action. Six channels, one publisher.

---

## 7. UNINSTALL

**The best-executed part of the repo.** `cli/commands/uninstall/run.ts` (461 lines).

`buildRemovalPlan(installRoot)` returns two classified lists and the CLI **always** renders a
two-section preview (`✗ will remove` / `✓ will preserve`) before a `--yes`-gated confirm; `--dry-run`
stops there. Removal order is symlinks → files → dirs sorted deepest-first.

Ownership tests, all real:
- `symlinkTargetsInstall` — `readlink` + `realpath`, with lexical fallback for dangling links, so a
  symlink written by *another* project's install or the global install is preserved, not stolen
  (`run.ts:109-135`).
- `isWorkflowSymlinkDir` / `isWorkflowSymlinkFile` — a dir counts as oma's only if its `SKILL.md`
  is a symlink resolving inside `.agents/workflows/`.
- `hasOmaMarker` — `<!-- oma:generated -->` gates `.github/prompts/*.prompt.md`.
- `vendorSkillsDir(vendor, installRoot)` — the mode-aware resolver, so HOME-consent vendors are
  scanned under `~`.
- Explicit preserve list: `.agents/oma-config.yaml`, `.agents/mcp.json`, any real (non-symlink)
  dir in a vendor skills dir.

**Residue it leaves** (by reading what `buildRemovalPlan` does *not* enumerate):
- `.agents/hooks/`, `.agents/agents/` — never added to `omaOwned`
- **hook registrations inside vendor settings** — `.claude/settings.json` `hooks.*`, permissions,
  statusLine; `.codex/config.toml` feature flags; `.qwen`/`.gemini` privacy keys. There is
  `mergeHookGroups` machinery to strip them, but uninstall never calls it.
- the `<!-- OMA:START … OMA:END -->` block in `CLAUDE.md` / `AGENTS.md`
- `.cursor/rules/*.mdc`, `.cursor/mcp.json`, `.pi/extensions/oma/`, `.opencode` plugin registration
- the seven `.gitignore` lines
- `.githooks/commit-msg` co-author guard and any global `git config` applied
- `~/.serena/project.yml` registration + oma serena context, `~/.cache/oma-*`,
  `~/.gemini/antigravity-cli/` HUD, `~/.bun`, `~/.local/bin/serena`
- the three `.agents/backup/` dirs are only cleaned by a *successful update*, not by uninstall

So: **clean about what it claims to own, materially incomplete about what it actually wrote.**
The 023 design doc names defect #1 (HOME-consent leakage) itself.

---

## 8. TRACTION (measured 2026-09-01 via `gh api` / npm registry API)

| Metric | Value | Source |
|---|---|---|
| Stars | **1,257** | `gh api repos/first-fluke/oh-my-agent` |
| Forks | **146** | same |
| Open issues | **1** | same |
| Watchers | 6 | same |
| Contributors | 11 | `gh api …/contributors --paginate` |
| Created | 2026-01-30 | same |
| Last push | **2026-09-01** (same day) | same |
| Repo size | 73,176 KB | same |
| License / language | MIT / TypeScript | same |
| Releases total | **601** tags | `gh api …/releases --paginate` |
| — by component | `cli` 431, `web` 128, `oh-my-agent` 34, `action` 6, `oh-my-antigravity` 2 | tag prefix histogram |
| Latest release | `cli-v13.1.1`, 2026-09-01T06:55Z | `latestRelease` |
| npm versions published | **392** | `registry.npmjs.org/oh-my-agent` |
| npm first publish | 2026-03-13 | same |
| npm downloads / week | **2,033** | `api.npmjs.org/downloads/point/last-week` |
| npm downloads / month | **10,758** | `…/last-month` |
| Homebrew | formula + auto-bump workflow | `cli/oh-my-agent.rb`, `.github/workflows/bump-homebrew.yml` |

Reading: **~7 months old, 1.25k stars, ~10.7k npm downloads/month, 431 CLI releases** = roughly two
releases per working day. release-please + conventional commits + commitlint. 1 open issue against
146 forks suggests aggressive triage or low external engagement; 11 contributors, 6 watchers — this
is essentially a very fast solo/small-team project with strong CI discipline.

Engineering hygiene is unusually high for this category: 321 vitest files, Biome, `bun typecheck`,
`.husky/pre-push` runs `CI=true bun run test` **and** `check:emit-drift`, and CI gates that the
committed generated artifacts (`.claude-plugin/marketplace.json`, `skills/`, `com.firstfluke.oma/`,
`cli/{CLAUDE,AGENTS}.md`) byte-match a fresh `oma emit` into a scratch dir, with `version` normalised
so a release bump alone doesn't fail the gate (`cli/scripts/check-emit-drift.mjs`).

---

## 9. What's GOOD, what's BAD (opinionated)

### Genuinely good

1. **Declarative vendor adapters.** `.agents/hooks/variants/<vendor>.json` + a 150-line JSON Schema
   (`hook-variant.schema.json`) capture: hook dir, settings file path, project-dir env var, runtime,
   event→handler map with per-hook timeout and matcher, statusLine (with `statusLineKey` for Qwen's
   nested `ui.statusLine`), `extra` settings to merge, `featureFlags` (file/section/flags for Codex
   TOML), and three escape hatches for hosts that don't fit — `flatHookEntries` (Cursor's flat array),
   `skipSettingsMerge` (Kiro reads a dedicated agent JSON), `homeOnly` (Antigravity). Adding a host
   is mostly *data*. Every deviation is a named, documented boolean rather than an `if (vendor ===
   "cursor")` scattered through the code.

2. **One dispatcher per event, not N processes.** `installHooksFromVariant`
   (`cli/platform/hooks-composer.ts:73-200`) collapses a whole handler chain into a single settings
   entry: `oma-hook.sh --vendor claude --event UserPromptSubmit`, timeout = Σ(handler timeouts) + 5s.
   Handler `.ts` files aren't even copied to the vendor dir — they run in-process inside `oma hook`.
   They measured it: `cli/__bench__/oma-hook-latency.bench.ts` documents p50 ~624 ms / p95 ~821 ms
   against a 1,500 ms SLO, names the dominant cost (node startup + a 6.5 MB bundle), and the bench
   **exits non-zero if p95 regresses**.

3. **The `oma-hook.sh` wrapper is machine-independent by construction.** No install-time path is
   baked in: `$OMA_BIN` → `command -v oma` → a literal list of well-known install dirs (`~/.bun/bin`,
   `~/.local/bin`, mise shims + a glob for mise node installs, volta, npm-global, both homebrew
   prefixes) → **`exit 0` fail-open**. The file is byte-identical on every machine, so a team can
   commit `.claude/hooks/` without carrying one developer's `$HOME` and without churn as teammates
   with different install methods re-link. `"$@"` passed verbatim; a non-zero `oma` exit is swallowed
   (`|| true`) so a stale CLI can never wedge the agent. This is the single most copyable idea here.

4. **Marker-scoped settings merge.** `isOmaManagedHookGroup` matches two generations —
   current (`name` starts `oma-hook-`, or `command` contains `oma-hook.sh`) and legacy
   (`bun …/<known-core-script>.{ts,js}`, from a hard-coded set of 8 names) — so a re-install replaces
   oma's own entries *including ones written by older versions*, and user hook groups on the same
   event are preserved **in their original order** (`mergeHookGroups`). `extra` is shallow-merged one
   level so `permissions` / `ui` augment rather than clobber.

5. **Ownership-classified uninstall with a mandatory preview.** See §7. The three-way class model in
   the 023 draft (`exclusive` / `shared`+region / `seeded`) is the right abstraction and I have not
   seen it stated this cleanly anywhere else in this ecosystem.

6. **Emit-drift CI gate.** Generated distribution artifacts are committed *and* verified against a
   fresh emit on every push. This is how you ship to six package ecosystems without the manifests
   rotting. The `stripVersion` normaliser shows they hit the obvious false-positive and fixed it
   rather than disabling the gate.

7. **Install guardrails written from real incident reports.** sudo refusal, HOME refusal, WSL
   explanation, PID lock with stale grace, migrations run twice with an empty vendor list first so
   pre-consent code cannot touch vendor files, and `assertContainedRelPath` path containment on every
   manifest-driven write (`cli/platform/path-containment.ts`). The comments carry edge-case IDs
   (EC-5, EC-12, EC-15, T2.13) — these came from bug reports, not from a checklist.

8. **A real A/B harness for prompt changes.** `cli/commands/harness/` runs baseline vs. candidate
   arms over a task suite with deterministic file/output checks, computes lift against
   `HARNESS_PASS_LIFT = 0.05` and `HARNESS_MIN_TASKS = 5`, and hashes both the suite
   (`computeSuiteHash`, including a Merkle-ish `hashTree` of each task workspace) and the baseline
   (`computeBaselineHash` over `.agents/{agents,config,rules,skills,workflows}` + `oma-config.yaml`).
   `hashTree` **refuses to hash a symlink escaping the root** rather than following it. Plus an
   append-only event log with 14 typed semantic kinds and `fsync`+`rename` atomicity
   (`cli/state/events.ts`), and `cli/state/artifact-verifier.ts` — a deterministic stop-gate that
   checks the durable artifacts a workflow *should* have left, with the explicit rationale that
   "prose instructions can be rationalized away". That framing is correct and rare.

### Genuinely bad

1. **Unverified `main` tarball is the content transport.** `TARBALL_URLS` in `cli/io/tarball.ts:19-23`
   all point at `main`. Every `oma install` and `oma update` on every machine pulls whatever is on the
   default branch *right now*. There is no tag pin, no checksum, no signature — and `prompt-manifest.json`
   with its 418 sha256 entries exists but is **only used for a version string compare**; the verifying
   function `downloadFile` has zero production callers. Two users installing an hour apart get
   different bytes with the same reported version. A repo compromise or a bad merge is instantly live
   for everyone. This is the single most serious defect.

2. **`cpSync(force:true)` over the whole `.agents` tree on update.** One line
   (`cli/commands/update/run.ts:233`) silently destroys every edit to every shipped asset. The config
   header is honest about it, which is better than hiding it, but "document the data loss" is not a
   fix. There is no `--dry-run`, no diff, no drift warning, no backup retained (backups are *deleted*
   at the end of a successful update). The 023 doc's §7.2 (compare hash → skip/write/warn, `--force`
   to override) is the fix, unimplemented.

3. **No extension contract worth the name.** No `custom/`, no overlay, no enabled-modules list, no
   third-party plugin protocol. User-authored skills survive by accident of `cpSync` semantics plus a
   `startsWith("oma-")` filter — not by design. The docs' own answer to "I want to change a skill" is
   "upstream it". For a project positioned as a framework, this makes every serious user a forker.

4. **`.oma-hook` bootstrap is a `curl | sh` inside a SessionStart hook.** `scripts/plugin-bootstrap.sh`
   fetches `install.sh` from `main` and pipes it to `sh`, which then installs bun, uv, and serena and
   runs `oma install --yes` against the project. Installing a Claude Code plugin should not silently
   provision three runtimes and rewrite the workspace. It is idempotent and fail-open, which limits
   the blast radius, but not the trust radius.

5. **`promptUninstallCompetitors`.** Offering to remove other people's tools during your own install
   is a category error, even behind a prompt.

6. **Ownership is heuristic and the coverage is admittedly partial.** Five different in-band markers
   (`oma-hook.sh` in a command string, `oma-hook-` name prefix, `<!-- OMA:START -->`,
   `<!-- oma:generated -->`, "is it a symlink"), each covering a different file type, none covering
   JSON settings on the uninstall path. Their own design doc: *"Marker coverage is not total… relies
   on structural inference that gets weaker with every new vendor."* And the "is it a symlink" test
   is **wrong on Windows**, where `createLink` legitimately falls back to junction/hardlink/copy.

7. **Duplication as a build artifact, in git.** `skills/`, `com.firstfluke.oma/`, `plugin.json`,
   `mcp.json`, `.claude-plugin/`, `.cursor-plugin/` are all emitted copies of `.agents/`. The drift
   gate makes this *safe*, but the repo is ~3× larger than its information content and every reader
   has to learn which of four copies of `oma-debug/SKILL.md` is canonical.

8. **Heavy, opinionated runtime dependency chain.** bun **and** uv **and** a Python `serena-agent`
   language server, spawned per concurrent agent session, plus `better-sqlite3` (native), puppeteer-core,
   pptxgenjs. The code is full of comments about the memory cost of that ("each open agent session
   spawns its own serena + LSP tree"), and there is a whole `serena-reaper` subsystem
   (`cli/io/serena-reaper.ts`, 25 KB + 36 KB of tests) whose job is to kill leaked daemons. That is
   a subsystem that exists to clean up after an architectural choice.

9. **Scope sprawl.** 38 command groups including video (Remotion), slides (pptx), images, voice, PDF,
   HWP (Korean word processor), academic writing, diagrams, scholar search, and a "market" research
   engine. `oma-hwp` and `oma-video` do not belong in the same binary as a hook dispatcher. The npm
   bundle is ~6.5 MB and, per their own bench, node startup on it is the dominant hook latency cost.

10. **Latency the architecture can't fix in JS.** ~620 ms p50 per hook invocation, on **every user
    prompt**. Their stated mitigations are "a leaner entrypoint" or "a daemon phase". Both are
    workarounds for the runtime.

---

## 10. Transfer to Muse Code (compiled Rust binary host)

### Transfers directly — take these

1. **The vendor-variant JSON schema, generalised into a host-adapter format.** `.agents/hooks/variants/*.json`
   is 90% of what `oh-my-musecode` needs to write `.muse/hooks.json` *and* `.claude/settings.json`
   *and* `.codex/…` from one source. Copy the field set wholesale: `hookDir`, `settingsFile`,
   `projectDirEnv`, `runtime`, `events{event → [{hook, matcher, timeout}]}`, `statusLine` +
   `statusLineKey`, `extra`, `featureFlags{file,section,flags}`, and — critically — the three named
   escape-hatch booleans (`flatHookEntries`, `skipSettingsMerge`, `homeOnly`). Ship it with a JSON
   Schema and validate at load. In Rust this is a `serde` struct with `#[serde(default)]`, and you
   get compile-time exhaustiveness over the host enum that TypeScript can't give them.

2. **One dispatcher process per event.** Register a single `musecode-hook` (or `omm hook --host muse
   --event PreToolUse`) per event and run the whole handler chain in-process. This is the biggest
   structural win in the repo, and **Rust converts it from "acceptable" to "free"**: their 620 ms
   node-startup tax becomes ~2 ms. Everything they wrote about needing a future daemon phase is
   simply not a problem for us — do not port the daemon design.

3. **The runtime-resolving, machine-independent wrapper script.** `$OMM_BIN` → `command -v` →
   well-known install dirs → `exit 0` fail-open, `"$@"` verbatim, non-zero swallowed. Byte-identical
   on every machine so a team can commit its hook dir. Steal this file almost verbatim; only the
   candidate paths change. **Fail-open is non-negotiable** — a config framework must never be able
   to wedge the agent it configures.

4. **Marker-scoped settings merge with generation awareness.** `mergeHookGroups` +
   `isOmaManagedHookGroup` is exactly the algorithm `.muse/hooks.json` needs: strip only entries
   whose `name` carries your prefix or whose `command` contains your wrapper filename, preserve user
   groups in original order, shallow-merge `extra` one level. Include the legacy-pattern arm from day
   one — you will change your marker format and need to reclaim old entries.

5. **The 023 ownership manifest — but *implemented*, as `.muse/lock.json`.** This is the highest-value
   artifact in the repo and it is a design doc they never shipped. Muse Code already gives us the
   file (`.muse/lock.json` with provenance/quarantine/allowed_tools), so we start where they stopped.
   Take: three ownership classes (`exclusive` / `shared` + region descriptor + `regionSha256` /
   `seeded`), `base: "installRoot" | "home"` relative paths (never absolute — survives dotfiles sync
   and fixes their HOME-consent leak), `writer` (the subsystem that produced the entry, for traceable
   repair), `mechanism` (symlink/junction/hardlink/copy — fixes their Windows misclassification),
   per-entry `sha256`, entries sorted by `(base, path)` for byte-stable rerun, atomic
   temp→fsync→rename, corrupt manifest renamed `.bad` and treated as absent, **markers demoted to a
   secondary sweep that reports `unmanifested` rather than deleting.** Their §9 failure-mode table is
   a ready-made test matrix. And take their honesty: this buys *convergence* (rerun the reconciler),
   not rollback — don't claim rollback.

6. **Ownership-classified uninstall with a mandatory two-section preview and `--dry-run`.** Port the
   structure (`buildRemovalPlan` → preview → confirm → ordered removal: symlinks, files, dirs
   deepest-first). Then **fix the residue**: enumerate hook registrations inside host settings,
   delimited doc regions, gitignore lines, git hooks, and cache dirs as `shared`/`exclusive` manifest
   entries so they are actually removable. Their gap is our checklist.

7. **Emit-drift CI gate.** If `oh-my-musecode` ships generated artifacts for multiple hosts —
   `.muse-plugin/plugin.json`, `.claude-plugin/`, `.codex-plugin/` (Muse's loader reads all three) —
   commit them *and* verify on every push that a fresh emit reproduces them byte-for-byte, with
   `version` normalised. Cheap, and it is the only thing that keeps six manifests honest.

8. **`managed-skill.ts` as the third-party fetch primitive.** Channel `stable` (latest release tag)
   vs `main` (HEAD sha); per-ref cache dirs so an in-flight run never sees a partial tree; throttled
   remote check with `--force`; prune-other-refs after a successful switch; network failure →
   reuse cache and *report* `status: "stale"` rather than failing. This is the right shape for
   `omm add <repo>`. Generalise `ManagedSkillSpec` from a compiled-in constant to a registry file —
   that one change is the difference between their dead end and a real ecosystem.

9. **Guardrails.** sudo refusal, HOME-write consent, `cwd == $HOME` refusal, PID lock with stale
   grace, path containment on every write, symlink-escape refusal when hashing. All host-agnostic,
   all cheap, all learned from incidents.

10. **`hashTree` / `computeBaselineHash`.** Deterministic Merkle hash over the config tree, sorted,
    skipping `node_modules`/`.venv`, refusing escaping symlinks. In Rust with `sha2` + `walkdir` this
    is ~40 lines and gives us: drift detection, "is my config what the lockfile says", and A/B
    provenance for prompt changes.

### Does NOT transfer

- **Anything shell-sourcing.** Already understood; nothing here depends on it (oma is not a shell
  framework either — this is the one real similarity in framing).
- **The bun/uv/serena bootstrap chain.** Three `curl | sh` provisioners exist only because the CLI is
  TypeScript and the code-intelligence layer is Python. A Rust binary is the install: one artifact,
  `cargo install` / brew / a signed release binary. Delete this entire layer, and with it
  `install.sh`, `install.ps1`, `plugin-bootstrap.sh`, and the `serena-reaper` subsystem.
- **`runtime: "bun"` in the variant schema and `.ts` hook handlers.** Our handlers are Rust functions
  behind an enum, or (if we want user-authored handlers) a declared subprocess contract. The variant
  file should name a *handler id*, not a script filename — that also removes their `copyHookScripts` /
  `requiredVariantScripts` machinery entirely.
- **Node-startup mitigations.** The latency bench, the SLO, the lazy `Promise.all` command
  registration in `cli.ts`, the "fast path so `oma hook` never pays module-evaluation cost" comment,
  and the deferred daemon phase are all artifacts of a JS runtime. Skip them; keep only the *idea* of
  a measured, CI-enforced hook latency budget (ours should be ~5 ms, not 1,500).
- **npm/Homebrew/APM/Agent-Plugins six-channel fan-out.** Wrong shape for a compiled binary. One
  signed release per platform + a checksum file, plus Muse's own `.muse-plugin` manifest.
- **`cpSync(force:true)` as the update mechanism.** Replace with lockfile-driven per-file
  reconciliation: hash matches → skip; absent → write; drifted → preserve + warn (`--force` to
  override); in-manifest-but-not-in-plan → orphan, prune under the same drift rules. Their §7.2,
  implemented.
- **The unverified `main`-branch tarball.** Muse Code ships `.muse/lock.json` with
  provenance/quarantine — pin a release tag, verify a detached signature or at minimum the
  per-file sha256 set, and record the resolved ref + digest in the lockfile. Do not repeat this.
- **Scope sprawl.** Video/slides/HWP/scholar/market do not belong in a config framework. If we want
  them, they are third-party packages resolved through the registry, which is exactly the pressure
  that forces us to build a real extension contract instead of an "upstream it" policy.
- **`promptUninstallCompetitors`.** Don't.

### The one-line lesson

oma proves the **SSOT + declarative host-adapter + single-dispatcher** architecture works across 13
runtimes, and then loses most of the benefit to two implementation choices — an unverified mutable-branch
tarball as the transport, and a force-copy as the update. `oh-my-musecode` should take the architecture,
implement the ownership manifest they only designed, and replace the transport with a pinned, verified,
lockfile-recorded fetch. In a Rust binary, the parts that cost them the most (hook latency, runtime
bootstrap, daemon-lifecycle cleanup) cost us nothing.

---

## Key file map (for follow-up reading)

| Concern | File |
|---|---|
| Host adapter schema | `.agents/hooks/variants/hook-variant.schema.json`, `.agents/hooks/variants/claude.json` |
| Hook composition / dispatcher | `cli/platform/hooks-composer.ts`, `cli/platform/hooks-composer/oma-hook-wrapper.ts` |
| Settings merge / marker ownership | `cli/platform/hooks-composer/settings-merge.ts` |
| Install | `cli/install.sh`, `cli/commands/install/run.ts`, `cli/platform/skills-installer/ssot-install.ts` |
| Vendor reconciliation kernel | `cli/commands/link/run.ts` |
| Update (the clobber) | `cli/commands/update/run.ts:233-246`, `cli/commands/update/install-state.ts` |
| Transport (the gap) | `cli/io/tarball.ts`, `cli/platform/manifest.ts:242-310` |
| Uninstall | `cli/commands/uninstall/run.ts` |
| **Ownership manifest design (unimplemented)** | `docs/plans/designs/023-ownership-manifest.md` |
| Vendor/target registry | `cli/constants/vendors.ts`, `cli/constants/paths.ts` |
| SSOT → host emit + drift gate | `cli/platform/emit/*.ts`, `cli/scripts/check-emit-drift.mjs` |
| Third-party fetch primitive | `cli/platform/managed-skill.ts` |
| Event log / verification | `cli/state/events.ts`, `cli/state/artifact-verifier.ts`, `cli/commands/harness/` |
| Latency budget | `cli/__bench__/oma-hook-latency.bench.ts` |
| Spec + support matrix | `docs/AGENTS_SPEC.md`, `docs/SUPPORTED_AGENTS.md` |
| The extension contract, stated | `.agents/oma-config.yaml` lines 1-7 |

---

## Verification

**Verdict: MOSTLY_SOLID.** Independently re-cloned (`git clone --depth 50`) into
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-first-fluke-oh-my-agent/repo`
and re-derived every number from the working tree and the GitHub/npm APIs on 2026-09-01. The repo is real,
the traction is real, and every load-bearing architectural and security claim survives direct code reading.
Four numeric overcounts and one doc-vs-code inversion are corrected below. Nothing was refuted.

### 1. Existence and traction — CONFIRMED, exact

`gh api repos/first-fluke/oh-my-agent` returns, field for field: stars **1257**, forks **146**, open issues **1**,
subscribers **6**, size **73176** KB, created **2026-01-30T11:29:46Z**, pushed **2026-09-01T06:55:38Z**, MIT,
TypeScript, default branch `main`, 17 topics. `contributors?per_page=100` → **11**. Homepage
`https://firstfluke.com/oh-my-agent/`.

Releases: `gh api .../releases --paginate` → **601** tags, prefix breakdown re-derived independently:
**431 `cli-`, 128 `web-`, 34 `oh-my-agent-`, 6 `action-`, 2 `oh-my-antigravity-`** — matches the report exactly.
Latest `cli-v13.1.1`, published `2026-09-01T06:55:30Z`.

npm registry: **392** versions, `time.created` **2026-03-13T05:43:51Z**, `dist-tags.latest` 13.1.1, bins
`oh-my-agent` and `oma` both → `bin/cli.js`. `api.npmjs.org/downloads/point`: **2033** last week
(2026-08-23→29), **10758** last month (2026-07-31→08-29). `cli/package.json` `files: ["bin"]` — confirming the
report's sharpest packaging observation: the npm package ships no content, assets always come from the GitHub
`main` tarball at runtime.

### 2. Shipped-content counts — CONFIRMED, with one label caveat

Re-ran every find/wc. Matches: **3007** tracked files; **33** `SKILL.md` (34 dirs incl. `_shared`);
`_shared` **32** files; **12** agents in `.agents/agents/*.md`; **7** per-vendor agent variant JSONs +
`agent-variant.schema.json`; **21** workflow `.md` / **24** files; **13** rules; **23** entries in
`.agents/hooks/core/`; **9** vendor variant JSONs (antigravity, claude, codex, commandcode, cursor, grok, kimi,
kiro, qwen) + `hook-variant.schema.json` + the `opencode/` and `pi/` bridge dirs; **4** MCP configs
(`.mcp.json`, `mcp.json`, `.agents/mcp.json`, `.agents/mcp_config.json`); **20** eval files across **3** suites;
**439** files / **326** markdown under `.agents/`; **875** `.ts` under `cli/` = **554** non-test + **321**
`.test.ts` (+1 `.bench.ts`); **38** command groups; **277** `.md` under the generated `skills/`; `cli/oh-my-agent.rb`
Homebrew formula present; `.github/prompts` and `.pi` are **not** committed, confirming "prompts are generated,
not shipped"; **0** output styles.

*Caveat on "203 skill resource files":* correct only under a narrow definition — files living under a skill's
`resources/` subdir. Total non-`SKILL.md`, non-`_shared` files under `.agents/skills/` is **255**
(203 `resources/` + 27 `variants/` + 9 `reference/` + 7 `config/` + 6 `templates/` + 3 `scripts/`). Not wrong,
but narrower than it reads.

### 3. Extension contract — CONFIRMED as described, not an aspiration

Read the code paths that would honour a contract; there is none for content.

- `.agents/oma-config.yaml` is **527** lines with **12** documented sections (`Contents` block, lines 11-24).
  The header quote is verbatim at lines 1-6 (report says 1-7): *"The single user-owned file: `oma update`
  preserves it byte for byte and only appends top-level keys new releases introduce. Every other file under
  `.agents/` is replaced wholesale on update, so this is the only place edits survive."* `vendors:`,
  `models:`, `custom_presets:` keys all present. `appendMissingConfigKeys` lives at
  `cli/platform/agent-config/config-merge.ts:31` and is documented as top-level-only.
- `selectSkillsToPrune` confirmed at `cli/commands/update/install-state.ts:74-84`; the
  `name.startsWith("oma-")` filter is on **line 80** (report says 79). User skills do survive by this accident,
  and nothing in the code or docs states it as a contract.
- `isOmaManagedHookGroup` / `mergeHookGroups` in `cli/platform/hooks-composer/settings-merge.ts` are real and
  match both generations, with the legacy arm keyed on exactly **8** core script names
  (`OMA_CORE_SCRIPT_NAMES`, lines 16-25). User hook groups genuinely survive.
- No `custom/` dir, no overlay, no enabled-modules list, no third-party plugin protocol: `grep -rn
  '\.agents/custom\|customDir\|overlay' cli/ --include='*.ts'` returns only unrelated hits (Remotion video
  overlays, harness candidate overlays). The "upstream it" policy is real —
  `web/docs/guide/skill-opt.md` §"SSOT caveat for `oma-*` skills": *"Skills whose ID starts with `oma-` are
  owned by oh-my-agent and are **overwritten by `oma update`** … upstream changes to the registry."*
- Vendor extension really does require a fork: `cli/constants/vendors.ts` is a compiled-in const with four
  arms, and adding a host means editing it plus a variant JSON plus a `cli/vendors/<x>/` adapter.

### 4. Install / update — CONFIRMED, faithful to the scripts

- `cli/install.sh` is **197** lines, forces `curl --proto '=https' --tlsv1.2`, chains `bun.sh/install`,
  `astral.sh/uv/install.sh`, and `uv tool install -p 3.13 serena-agent@latest --prerelease=allow`, and ends
  `exec bunx oh-my-agent@latest < /dev/tty` (line 192). `cli/commands/install/run.ts` is **547** lines.
- `cli/io/tarball.ts`: `TARBALL_URLS` are all `/main` (api.github.com tarball → codeload → archive/main.tar.gz),
  falling back to `git clone --depth 1 --branch main`, extracted into `mkdtempSync`. **No tag pin, no checksum,
  no signature anywhere in the file.**
- `cli/commands/update/run.ts` is **522** lines (report says 520). The single
  `cpSync(join(repoDir,".agents"), join(cwd,".agents"), {recursive:true, force:true, filter: … ".agents/eval"})`
  is at **line 233**, exactly as claimed. Exactly three things are saved aside: `oma-config.yaml`, `mcp.json`,
  and the backend stack state.
- **The report's sharpest claim is true.** `fetchRemoteManifest()` (`cli/platform/manifest.ts:242`) is called
  once, at `run.ts:193`, and `remoteManifest` is thereafter used only for `.version` string compares
  (195, 207, 209, 452, 464, 488) plus `metadata.totalFiles` in a success message (489). `prompt-manifest.json`
  really does carry **418** entries of shape `{path, sha256, size}`. And `grep -rn downloadFile cli/` returns
  exactly three hits: the definition at `manifest.ts:248` and two in `manifest.test.ts`. The only function that
  verifies a sha256 has **zero production callers**.
- `docs/plans/designs/023-ownership-manifest.md` exists, is **336** lines, and reads
  `Status: **Draft** (design only — no implementation)`. `grep -rn '_ownership.json' cli/` returns nothing.
- Uninstall residue confirmed by reading `buildRemovalPlan` (`cli/commands/uninstall/run.ts:156-320`, file is
  **461** lines): it enumerates `.agents/skills/*`, `.agents/workflows/`, `.agents/rules/`, `.agents/config/`,
  vendor skills dirs, `.github/prompts/*` (marker-gated), `.zcode/commands/*` — and **not** `.agents/hooks/`
  or `.agents/agents/`. `grep -nE 'settings\.json|OMA:START|gitignore|config\.toml|githooks|serena|cache'`
  over the whole uninstall file returns **zero** hits, confirming every item on the report's residue list.

### 5. Strengths spot-checked — all real

`generateOmaHookWrapper` (`cli/platform/hooks-composer/oma-hook-wrapper.ts`) matches the report almost
word for word: `$OMA_BIN` → `command -v oma` → the 8 literal candidates (bun, `~/.local/bin`, mise shims,
a glob for mise node installs, volta, npm-global, both Homebrew prefixes) → `exit 0` fail-open, `"$@"` passed
verbatim, `|| true` swallowing a non-zero exit. `hook-command.ts:33` confirms the single-dispatcher emission
`<hookDir>/oma-hook.sh --vendor <vendor> --event <nativeEvent> [--matcher <m>]`. The bench file documents
`p50 ~624 ms | p95 ~821 ms | max ~848 ms` against `P95_SLO_MS = 1500` and exits non-zero on breach.
`cli/scripts/check-emit-drift.mjs` re-emits `oma emit --target all` into an `mkdtempSync` scratch dir and
byte-compares with `key === "version" ? "<version-normalized>"`, wired into both `.github/workflows/test.yml`
and `.husky/pre-push` (alongside `CI=true bun run test`). `managed-skill.ts` is 347 lines with
`ManagedChannel = "stable" | "main"` and exactly three compiled-in specs — `ARCHIFY_SPEC`
(`cli/commands/diagram/managed.ts`), `LAST30DAYS_SPEC` (`cli/commands/market/resolve.ts`),
`REMOTION_SKILLS_SPEC` (`cli/commands/video/internal/remotion-workspace.ts`) — none user-extensible.
`HARNESS_MIN_TASKS = 5` / `HARNESS_PASS_LIFT = 0.05` at `cli/commands/harness/types.ts:1-2`, and
`hashTree` throws `Cannot hash escaping symbolic link` rather than following it. The serena-reaper subsystem
measures 25,512 bytes of impl against 36,294 bytes of tests — the report's "25KB impl + 36KB tests" is exact.
`promptUninstallCompetitors` is real, at `cli/utils/competitors.ts:268`. The 7 appended `.gitignore` patterns
are enumerated verbatim in `OMA_PROJECT_GITIGNORE_PATTERNS` (`cli/constants/paths.ts:71-79`).
`.claude-plugin/marketplace.json` contains exactly **1** plugin entry (`oma`) listing the repo's own skills —
publisher-side, not a registry, as claimed.

### 6. Corrections

1. **"51 numbered migrations" is wrong — it is 24.** `cli/commands/migrations/` holds **51 files**, but only
   **24** are migrations (IDs `001`–`025` with `006` absent). The rest are 24 matching `.test.ts` files plus
   `index.ts`, `README.md`, `vendor-scope.ts`. The error appears twice (in `ships` and in `strengths`); a file
   count was read as a migration count.
2. **"9 built-in model-routing presets" is wrong — it is 7.** `BUILT_IN_PRESETS`
   (`cli/platform/built-in-presets.ts:18-165`) has exactly seven keys: `antigravity`, `claude`, `codex`, `qwen`,
   `kiro`, `cursor`, `mixed`. There are 5 further entries in `BUILT_IN_PRESET_ALIASES`, so neither 7 nor 12
   yields 9.
3. **"13 runtimes" is wrong — it is 14**, and the report's own list names all 14. `ALL_CLI_VENDORS` =
   9 `VENDORS` (antigravity, claude, codex, commandcode, cursor, grok, kimi, kiro, qwen) + 2 `EXTENSION_VENDORS`
   (pi, opencode) + 2 `INSTALL_ONLY_VENDORS` (copilot, hermes) + 1 `WORKFLOW_ONLY_VENDORS` (zcode). Only the
   headline number is off.
4. **"14 typed semantic kinds" is wrong — `SEMANTIC_EVENT_KINDS` has 13** (`cli/state/events.ts:24-38`). The
   broader `EventKind` union has 15, since it adds `"boundary"` and `"session.created"`. 14 is neither.
5. **`.agents/eval/` does not survive update — the report has this backwards, in its own favour.** The report
   lists `.agents/eval/<skill>/` as "the one documented user-writable slot", citing
   `web/docs/guide/skill-eval.md:33`, which does claim the path *"survives `oma update` without overwriting
   user-authored evals"*. The code contradicts the doc. `cli/commands/update/run.ts:239-242` runs
   `rmSync(join(cwd, ".agents", "eval"), {recursive: true, force: true})` unconditionally after the copy —
   identical to the install path at `cli/commands/install/run.ts:313-315`. The `cpSync` filter only prevents
   oma's *own* eval fixtures from being written; it does not protect the user's. So oma has **zero** durable
   user-writable slots under `.agents/` other than `oma-config.yaml`, `mcp.json`, the backend `stack/` state,
   and accidentally-surviving non-`oma-`-prefixed skills. This strengthens the report's central thesis, but the
   report states the fact wrongly and cites a doc the code does not honour.
6. **Minor drift, all immaterial:** `update/run.ts` is 522 lines (claimed 520); the `startsWith("oma-")` filter
   is at `install-state.ts:80` (claimed 79); `hook-variant.schema.json` is 160 lines (claimed "150-line"); the
   config header quote spans lines 1-6 (claimed 1-7).
7. **`homeOnly` is used by two vendors, not one.** The report attributes the escape hatch to Antigravity alone;
   `.agents/hooks/variants/kimi.json:8` also sets `"homeOnly": true`. The `transfers_to_musecode` advice is
   unaffected.
8. **Nuance, not an error:** the report's summary of `oma-config.yaml` ("enables/disables HOSTS … and model
   routing") undersells §11 "Skill overrides", which gives eight skills (video, image, voice, hwp, pdf, scholar,
   diagram, market) real, sparse, precedence-ordered settings overrides — *"shipped defaults < this file < env
   < flags"* — introduced by migration 022 precisely because the per-skill `.agents/skills/*/config/*.yaml`
   files were being clobbered by update. The report's verdict that there is no *content* extension contract
   remains correct; the *settings* surface is more substantive than stated.

### 7. Refuted

Nothing. Every claim about the transport defect, the force-copy update, the unimplemented ownership manifest,
the uninstall residue, the single-dispatcher architecture, the declarative variant schema, and the absence of a
registry was verified against the source and holds.
