# Teardown: rlaope/oh-my-hermes

**Verdict: CONFIRMED — repo exists, cloned, source read.**
Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/rlaope-oh-my-hermes/`
URL: https://github.com/rlaope/oh-my-hermes (HTTP 200, `gh repo view` OK)
Target agent: **NousResearch Hermes Agent** (`hermes` CLI + Hermes Desktop + Hermes messenger app). NOT Claude Code.

---

## 1. Existence + traction (real numbers, `gh api`, fetched 2026-09-01)

| Metric | Value | Source |
| --- | --- | --- |
| Stars | **1,299** | `gh api repos/rlaope/oh-my-hermes` |
| Forks | **123** | same |
| Open issues+PRs | **6** | same |
| Watchers (subscribers) | 6 | same |
| Repo size | 92,803 KB (~91 MB) | same |
| Created | 2026-06-03 | same |
| Last push | 2026-09-01T11:35:05Z (today) | same |
| Lifetime issues | **153** | `search/issues type:issue` |
| Lifetime PRs | **1,097** | `search/issues type:pr` |
| Releases | **11** (v1.0.0 → v2.0.0) | `gh release list` |
| Latest release | v2.0.0, 2026-08-29 | same |
| License | MIT | same |
| Language | Python (requires-python >= 3.11) | `pyproject.toml` |

**Contributors (`gh api .../contributors`):**
`rlaope` 1245 · `sionic-khope` 868 · `frirenai` 577 · dependabot 13 · then 10 one-commit drive-bys.

That third-place contributor matters: **`frirenai` and `sionic-khope` are AI agents**, named as such in the README ("Built with AI agents Friren and Killua"). The HEAD commit's author is literally `Killua AI`. So ~1,690 of ~2,700 commits are agent-authored. 1,097 PRs in 90 days is ~12/day. **This repo is an agent-built codebase.** That explains both its greatest strength (relentless, uniform, comment-heavy engineering discipline) and its greatest weakness (scale far beyond what its user contract can absorb).

**Codebase size:**
- `src/`: **289,331 lines** of Python across 24 packages
- `tests/`: **195,752 lines** across **427 test files**
- 1,578 non-git files total; 1,161 `.py`, 253 `.md`, 73 `.json`

### Name contention (`gh search repos oh-my-hermes`)
**Seven** repos are literally named `oh-my-hermes`:

| Repo | Stars | Lang |
| --- | --- | --- |
| **rlaope/oh-my-hermes** | **1,299** | Python |
| Salomondiei08/oh-my-hermes | 857 | Shell |
| witt3rd/oh-my-hermes | 302 | Python |
| HERMESquant/oh-my-hermes | 9 | TypeScript |
| shuangxipop1/oh-my-hermes | 2 | Python |
| codenote-net/oh-my-hermes | 2 | Python |
| relover1986/oh-my-hermes | 0 | Shell |

Plus `oh-my-hermes-agent`, `oh-my-hermes-web`, `oh-my-hermes-memory`, `oh-my-hermes-for-legal-researcher`. And witt3rd's description says "Inspired by oh-my-claudecode, rebuilt natively for Hermes primitives" — a fourth lineage.

**~2,460 stars are split across three near-identical products with the same name.** No one owns the namespace, so no one can own the ecosystem: a third-party "Hermes skill pack" author cannot target "oh-my-hermes" without picking a side.

Crucially, rlaope **knows** this. `src/install/identity_conflicts.py` (493 lines) is a doctor subsystem whose entire job is detecting when a *second visible local source* claims an OMH-facing name — a skill directory, a registered tool name, or a hook subscription — and reporting who owns which side by reading install manifests rather than guessing from path shape. Name contention forced a runtime conflict detector into the product.

---

## 2. What it SHIPS (real counts from `find | wc -l` and source greps)

| Category | Count | Evidence |
| --- | --- | --- |
| **Workflow skills (SKILL.md)** | **114** | `find skills -name SKILL.md \| wc -l` = 114; `ls skills \| wc -l` = 114 |
| — of which `ulw-` ultra-workflows | 9 | `ulw-context/interview/research/plan/work/maestro/loop/qa/perf` |
| — of which `omh-` skills | 105 | `ls skills \| grep -c '^omh-'` |
| Skill reference `.md` (sub-docs) | **55** | `find skills -name '*.md' ! -name SKILL.md` |
| `SkillDefinition` entries in catalog | **86** | `grep -c 'SkillDefinition(' src/skills/catalog_definitions.py` (6,914 lines) |
| **Plugin tools** (host-registered) | **14** | `src/plugin_bundle/omh/plugin.yaml` `provides_tools` |
| **Hooks** (lifecycle events) | **6** | `plugin.yaml` `provides_hooks`: `pre_llm_call`, `pre_tool_call`, `post_tool_call`, `transform_tool_result`, `pre_verify`, `on_session_end` |
| Memory provider | 1 | `provides_memory_provider: omh` |
| **Role prompts** | **9** | `roles/*.md` — builder, guide, handoff-guide, memory-keeper, operator, planner, researcher, reviewer, tracker; mirrored in `src/plugin_bundle/omh/references/role-*.md` |
| **Team profile packs (subagents)** | **4 packs / 17 roles** | `grep -c 'TeamProfilePack(' src/profiles/team.py` = 4; `TeamRole(` = 17. Installed as `~/.hermes/agents/omh-<pack>-<role>.md` |
| Operating models | 4 | solo-operator, small-team, research-ops, coding-runtime-team |
| **Statusline / TUI widget** | **1** | `src/omh/tui_widgets/omh-status.mjs` → `$HERMES_HOME/tui-widgets/` |
| **Output styles (skins)** | **4** | `src/omh/skins/omh.yaml` (sky), `omh-amber`, `omh-crimson`, `omh-mono` → `$HERMES_HOME/skins/` |
| **CLI subcommands** | **165 unique** | `grep -oE 'sub\.add_parser\("[a-z0-9-]+"'` across 67 files in `src/commands/` |
| **MCP** | server + 4 host installers | `omh mcp serve`; `src/mcp/host_config.py` writes configs for claude-code, codex, opencode, cursor |
| Doctor checks | **20** | `grep -oE 'Check\("[a-z_0-9]+"' src/maintenance/doctor.py` |
| Model routing categories | 9 | ultrabrain, deep, architect, unspecified-high/low, quick, writing, visual-engineering, artistry |
| i18n trigger packs | 3 | `src/routing/trigger_packs/{ko,ja,zh}.json` |
| Localized READMEs | 4 | en/ko/ja/zh |
| Vendored ecosystem index | **216 items** | `src/catalogs/awesome_hermes_agent_catalog.json` |
| Docs | 38 files | `ls docs` |
| CI workflows | 4 | ci.yml (14.7 KB), release.yml (11.1 KB), auto-release.yml, pages.yml |

**Slash commands: ZERO as shipped assets.** Hermes has no slash-command asset format here. Invocation is (a) trigger phrases in `SKILL.md` frontmatter `description` matched by `src/routing/`, (b) `./<name>` / `$ultrawork` style in chat, (c) the 165-command `omh` CLI. This is a meaningful architectural difference from Claude-Code-targeted oh-my-* repos.

---

## 3. INSTALL — reading `install.sh` (531 lines) end to end

Five install channels, all landing on **one wheel**:

```
brew install rlaope/tap/omh
bun install -g oh-my-hermes
npm install -g oh-my-hermes
curl -fsSL .../install.sh | sh
irm .../install.ps1 | iex          # install.ps1, 34 KB — a real native installer, not a shim
```

### What `install.sh` actually does

**Step 0 — resolve the artifact** (`install.sh:344-416`). `OMH_CHANNEL` defaults to `stable`. Stable resolves the newest tag by reading only the `Location:` header of a `/releases/latest` 302 — **one header read, no API token, no rate limit**:
```sh
curl -sSI "$OMH_REPO_LATEST_URL" | tr -d '\r' | awk 'tolower($1)=="location:"{print $2}' | tail -1
```
It then constructs a predictable wheel URL `oh_my_hermes-X.Y.Z-py3-none-any.whl` (~2.7 MB) rather than the tag archive (~44 MB), with a documented rationale in-file. `preview` tracks `main.zip`. `local` requires an explicit `OMH_PACKAGE_URL`.

**Step 1-2 — isolated venv** (`install_into_venv`, `:308-323`):
```sh
python3 -m venv "$OMH_VENV_DIR"                    # ~/.local/share/omh/venv (XDG-aware)
"$VENV/bin/python" -m pip install -q --no-cache-dir --force-reinstall --upgrade "$WHEEL"
```
`OMH_INSTALL_MODE=python` is an escape hatch that does `pip install --user` instead.

**Step 3 — expose the command** (`link_omh_command`, `:274-306`): symlink `~/.local/bin/omh` → venv. **If the target already exists and is not our symlink, it refuses**, prints the existing path, and tells you `OMH_FORCE_LINK=1`. It never clobbers a stranger's `omh`.

**Step 4/5 — setup + doctor: OPT-IN, DEFAULT OFF.** `OMH_RUN_SETUP="${OMH_RUN_SETUP:-0}"` (`:38`). The curl-pipe-sh path **installs a binary and touches nothing else**. There is even a branch (`:518`) that detects setup-flavoured env vars set without `OMH_RUN_SETUP=1` and prints *"Setup options were not applied because install.sh now installs the command only by default."*

**This is the single best decision in the whole repo.** `curl | sh` writes exactly two things: a venv and a symlink. Every mutation of the agent's own config is a separate, named, user-typed `omh setup`.

### What `omh setup` writes (the real footprint)

Anchors: `OMH_HOME` (default `~/.omh`), `HERMES_HOME` (default `~/.hermes`), both env-overridable, both with a `--scope project` variant (`src/system/paths.py:433-439, 670-688`).

**Under `~/.omh/` (OMH-owned, 20 top-level entries):**
`skills/` · `manifest.json` · `runtime/` (state.json, runs/, journal/{events,external_effect_receipts,approval_receipts,blocked_work_records,workspace_bindings,run_lineage_checkpoints}.jsonl, output-spills/, wrapper_sessions/, worktrees.jsonl, plan-context/, release-evidence/, efficiency-reports/) · `memory/` · `operations/` · `hermes-ops/` · `coding/` · `goals/` · `learning/` · `loops/` · `materials/` · `research-department/` · `state/` · `use-cases/` · `visual/` · `web-visual-qa/` · `agent-ops/` · `team-profile-packs/` · `setup-profile.json` · `targets.json` · `routing/{model-chains,model-providers,category-maestro,dispatch-models}.json`

**Under `~/.hermes/` (host-owned, touched surgically):**
- `config.yaml` — **line-level edits only** (see §4)
- `plugins/omh/` — the plugin bundle + `.omh-plugin-manifest.json`
- `skins/` — 4 yaml files + manifest (`src/omh/skin_pack.py:276`)
- `tui-widgets/omh-status.mjs` + manifest
- `agents/omh-<pack>-<role>.md` — team role subagents (`src/profiles/team.py:437`)
- `profiles/<bot>/` — the same registration replicated into every Hermes bot profile

**Optional / opt-in only:** MCP host configs (`~/.claude.json`, `~/.codex/config.toml`, opencode, cursor — via `omh mcp install --host`), macOS menubar `~/Library/LaunchAgents/com.rlaope.omh.menubar.plist`, npm/bun wheel cache (`~/Library/Caches/oh-my-hermes/npm`).

### The npm launcher is not a shim
`packaging/npm/lib/{launcher,cache,python}.js` vendors the exact wheel, **verifies its SHA-256** (`cache.js:44-64`), **refuses symlinks anywhere in the wheel path** (`launcher.js:34-56`), unpacks into a content-addressed cache keyed by `wheel_sha256` + `cache_tree_sha256`, requires a pre-existing Python ≥3.11, and **never downloads anything**. It stamps provenance into the child env: `OMH_COMMAND_PACKAGE_MANAGER` / `_ROOT` / `_RUNTIME` / `_ENTRYPOINT` (`launcher.js:125-128`).

---

## 4. EXTENSION CONTRACT — the weakest link

There are **five** ways content reaches the host, and only one of them is open to a user.

### (a) Registration by pointer — the core mechanism
`src/install/config_adapter.py:643` `ensure_external_dir()` appends **one line** to `~/.hermes/config.yaml`:
```yaml
skills:
  external_dirs:
    - /Users/me/.omh/skills
```
That's it. Hermes then reads `~/.omh/skills/<category>/<label>/SKILL.md`, an entire tree OMH owns outright. **OMH mutates 1 line of host config to gain a whole namespace.**

The YAML editing is deliberately a **hand-rolled line-based editor**, not a parse-and-redump — so comments, ordering and formatting in the user's config survive. It handles block lists, inline lists, `~`/`null`, and mixed indentation. When it sees a shape it does not understand it **raises rather than guesses**:
```python
_UNSUPPORTED_EXTERNAL_DIRS_SHAPE = "unsupported skills.external_dirs shape; use a YAML block list or inline list"
_DUPLICATE_EXTERNAL_DIRS_SHAPE  = "duplicate skills.external_dirs entries are unsupported; ..."
```
Readers stay non-throwing (doctor must not crash); **mutators are strict**. That split is exactly right. Five keys are edited this way: `skills.external_dirs`, plugin enablement, display skin, display interface, memory provider.

### (b) Foreign-rules import — the ONE real addition path
`src/workflows/external_rule_import.py` (477 lines) — `omh ops rules-import`. It ingests **other tools'** rule files:
`.cursorrules`, `.cursor/rules/*.mdc`, `.clinerules`, `.windsurfrules`, `.github/copilot-instructions.md`

Each becomes `~/.omh/skills/imported/<slug>/SKILL.md`. Guarantees:
- **explicit-root only** — the caller names the repo root; no filesystem walking; no symlinked sources; ≤40 sources, ≤128 KB each
- **repo containment enforced** — resolved source must stay under resolved root, and no path component below the root may be a symlink (ancestor symlinks like macOS `/var → private/var` stay legal, with a comment saying why)
- **prompt-injection + credential-shape refusal** — `/\b(?:ignore|disregard)\s+(?:all\s+)?(?:previous|prior)\s+instructions\b/i` and `is_secret_value_shaped()`; refused with reasons, never silently imported
- **a SEPARATE manifest** (`skills-import-manifest.json`) so `omh update`'s orphan pruning, which only touches names in the managed manifest, **can never delete an import** (`src/install/manifest.py:44-52` spells this out)

This is the correct shape for an extension namespace: *a directory the manager registers but does not own.*

### (c) Config-file overrides — genuinely open, well-designed
Four routing files under `~/.omh/routing/`, all documented and CLI-editable:
- `model-chains.json` (`mixture_chain_overrides/v1`) — empty `categories` = all shipped defaults live; a category you write **replaces** that chain for routing, fallback, and HUD labels alike. `omh model-chains show|set|interview`. **Seeded create-only**, so `omh update` never overwrites your edits (`src/commands/setup.py`, `_seed_model_chains_result`: *"Seeding is create-only, so user edits are never touched."*)
- `model-providers.json` (`model_provider_routes/v1`) — alias → wire-model. Stores provider IDs, **never credentials**.
- `category-maestro.json`, `dispatch-models.json` — coding-delegation routing with a documented precedence order.

Plus `~/.omh/setup-profile.json` `capability_policy` — six capability families you can disable (`omh capability-policy disable memory`). Every value is a scalar or list-of-scalars **on purpose**, because the setup-profile repair path drops nested dicts and floats and a policy using them would silently lose its disable list.

### (d) The tap path — the host's registry, not OMH's
```sh
hermes skills tap add rlaope/oh-my-hermes
hermes skills install rlaope/oh-my-hermes/skills/omh-routing --yes
```
This is why `skills/` in the repo is **flat** while installs are **nested**. Documented in `src/install/manifest.py:66-83`: Hermes' tap lister reads exactly one directory level, so nesting the tap tree would hide every OMH skill from `hermes skills search`. But Hermes reads a skill's dashboard *category* off its path and only when the path has ≥3 parts — so managed installs must be `<skills_dir>/<category>/<label>/SKILL.md`. **Two distribution surfaces with mutually incompatible layout constraints, both satisfied.**

### (e) Adding an OMH skill — FORK ONLY
`docs/ADDING-A-SKILL.md` is the damning document. To add one skill you must edit, in the same commit:

1. `src/skills/catalog_definitions.py` — the `SkillDefinition`
2. `src/routing/recommend.py` — `_SKILL_POLICIES`
3. `src/routing/trigger_packs/{ko,ja,zh}.json` — trigger language packs
4. `src/plugin_bundle/omh/awareness.py` — awareness-lane membership **and** `_WORKFLOW_CONTEXT_CARD_BY_WORKFLOW`
5. `src/wrapper/contract.py` — `VISIBLE_ACTIONS` + `_ACK_PRIMARY_ACTIONS_BY_NEXT_ACTION`
6. `src/routing/action_copy.py` — `NEXT_ACTION_LABELS`
7. `src/quality/chat_card_coverage.py` **or** `src/quality/routing_precision.py` — a coverage case
8. **Exact-count fixtures** in `tests/test_routing_precision.py`, `test_cli.py`, `test_hermes_ux_quality.py`, `test_release_smoke.py` — *"Grep those four for the old count."*
9. Regenerate four artifact families and pass a **skill-density gate** (`tests/test_skill_density.py`: zero filler phrases, <5% repeated sentences, >9.0 payload markers per 1k chars)

> **There is no `custom/` directory. There is no `enabled_modules` list. There is no plugin protocol for third-party OMH skills. A user who wants their own workflow in the OMH catalog must fork a 289k-line Python monolith and keep rebasing.**

The capability policy lets you **subtract** six coarse families. Nothing lets you **add** one skill.

---

## 5. UPDATE — best-in-class, and the reason to study this repo

`omh update` is a **two-phase self-update**.

**Phase 1: upgrade the command package through its own owner.** `_command_package_self_update_plan()` (`src/commands/setup.py:716`) detects the owning installer by *evidence*, never by guess:
- npm/bun → the launcher stamped `OMH_COMMAND_PACKAGE_MANAGER` into the env
- Homebrew → `_homebrew_prefix_root()` walks `sys.prefix` for a `Cellar/omh` component
- **PEP 610 `direct_url.json`** → `_direct_url_update_guidance()` reads the dist-info to distinguish `uv tool install` (→ `uv tool upgrade`), `pip install git+…` (→ `pip install --upgrade "git+<url>"`), `pip install <clone>` (→ `git -C <path> pull && pip install --upgrade <path>`), and an editable dev checkout (→ leave alone, `git pull` owns it)

The rationale comment is exactly right: *"An agent handed only the repository link often installs the command with `pip install git+...`. Those installs carry no manager provenance state, and the generic curl fallback would create a second, conflicting install next to the one that already owns the command."*

It then **re-enters the upgraded command** guarded by `OMH_UPDATE_COMMAND_PACKAGE_REENTERED` so phase 2 runs on the new code.

**Phase 2: refresh managed assets, without clobbering.** Three layered protections:

**(i) Per-file SHA-256 manifest.** `~/.omh/manifest.json` records `{name, path, sha256, source}` per skill plus `catalog_revision` — *"A version string cannot distinguish two builds of the same release with different catalog data, and per-file checksums detect drift without naming what drifted."*

**(ii) Modification detection → refusal.** `local_modifications()` re-hashes every recorded file; `install_skill_pack` raises before writing anything:
```python
if modified and not _overwrite_allowed(force):
    raise OmhError("local modifications detected; rerun with --force or resolve: " + ", ".join(modified))
```
And `--force` is not the last word. `_overwrite_allowed()` routes through `resolve_approval_tier()` + `resolve_security_posture()` — under `OMH_SECURITY=strict`, **`--force` stops overriding a local modification at all**.

**(iii) Four-condition prune.** `_prune_orphaned_skills` / `_prune_flat_layout_skill_directories` remove a directory only if it is (a) a catalog skill name, (b) recorded in the prior manifest at that path, (c) already replaced by a live categorized directory holding a SKILL.md, and (d) byte-identical to what the manifest recorded. Anything edited is **kept and reported** in `flat_layout_skills_retained`.

The comment on that function is the best incident post-mortem in the repo: two successive layout migrations (canonical→label, then label→category/label) plus non-destructive installs meant *"an observed machine went from 92 skills to 184 after the relabel, doubling the pack's per-turn context weight, and the manifest then reported every vanished old path as a local modification."*

**(iv) Idempotent manifest bytes.** `_carry_installed_at_when_nothing_moved()` — `installed_at` is the only field not derived from disk, so two identical installs produced identical manifest *bytes* only when both landed in the same wall-clock second. Since `omh update` answers "did the pack move?" by hashing the manifest, a timestamp made it report phantom changes. Now the timestamp records **when content last changed**, not when the installer last ran.

**(v) Plugin bundle: install + smoke test.** `.omh-plugin-manifest.json` in `~/.hermes/plugins/omh/` mirrors the same per-file-hash scheme, plus the installer **imports the freshly-written plugin and asserts every declared tool and hook actually registers** (`_register_smoke`, `_SmokeContext`). Not "files copied" — "the host can load it."

**(vi) Hook integrity ledger.** `src/install/hook_integrity.py` (571 lines) keeps tamper-evident per-hook digests plus a **revocation ledger**. A changed or revoked hook is dropped from `managed_hooks` **and** listed in `excluded_hooks` with the capability it takes down and the command that restores it. *"Revocation is data, never absence."* Every record carries `observed_in_this_environment: False` — a reviewed digest proves a file, not an invocation.

**Lockfile: NO.** There is no dependency lockfile, because there are no third-party content dependencies to resolve. `manifest.json` is an *install* manifest (what we wrote, and its hash), not a *resolution* lockfile (which upstream versions we chose). This is the correct answer given (4) — but it also means the moment a third-party content ecosystem exists, a real lockfile has to be designed from scratch.

---

## 6. REGISTRY — discovery is vendored and read-only

`src/catalogs/awesome_hermes_agent_catalog.json` — **216 items**, pinned to a specific upstream:
```json
"source": {"repo": "0xNyk/awesome-hermes-agent",
           "commit": "27389ad544f923ee67b455457c214c679f26ad8a",
           "readme_sha256": "33d58901...", "retrieved_at": "2026-07-27",
           "claim_boundary": "Static catalog parsed from upstream README. This is not plugin installation, runtime load, safety review, or endorsement evidence."}
```
Sections: Skills & Plugins 110 · Integrations & Bridges 30 · Tools & Utilities 28 · Memory Providers 18 · Domain Applications 16 · Multi-Agent & Swarms 5 · Forks 4 · Guides 4 · Media Forensics 1.

Surfaced via `omh ecosystem {summary,list,inspect,outcomes}`. It is **discovery only — you cannot install from it.**

Second registry-adjacent artifact: `docs/SKILL-SOURCES.md`, a hand-maintained **provenance table** of the 13 external repos whose skills were *reconstructed* (not copied) into OMH, each with paths studied, license, `reviewed_on` and `reviewed_ref` commit — with automation that diffs upstream HEAD against `reviewed_ref` and files an `upstream-skill-update` issue when the studied paths move. Plus a "candidate rows" table of researched-but-unshipped ideas with issue numbers.

**No marketplace. No `omh install <third-party-pack>`. Discovery is a pinned snapshot of somebody else's awesome-list.** Combined with §4(e), the ecosystem story is: *rlaope curates everything; you consume or you fork.*

---

## 7. UNINSTALL — clean, and honest about what it cannot clean

```sh
omh uninstall            # = --all = --purge
omh uninstall --dry-run
omh uninstall --registration-only   # only unhook from config.yaml
omh uninstall --remove-files        # legacy: registration + ~/.omh
omh uninstall --keep-command        # keep venv/link
```
Full uninstall removes: the `external_dirs` line from `~/.hermes/config.yaml`; `~/.omh`; `~/.hermes/plugins/omh` **only when it carries an OMH manifest** (else `--force`); generated team role files **recorded in the team-profile manifests**; the same registration from **every** `~/.hermes/profiles/<bot>/` through the same manifest-checked refusals; and the install.sh-managed venv + symlink **only when the running command is inside that managed venv**.

It explicitly declines to touch unrelated Hermes files, unrelated plugins, unrelated agents, or pipx/dev environments it cannot identify as installer-managed — and tells you so: *"If `omh` still runs after uninstall, that means the command package is still on PATH."*

**Documented residue** (deliberate, all disclosed in `docs/INSTALLATION.md:2255-2330`): the npm/bun wheel cache (`~/Library/Caches/oh-my-hermes/npm`, keeps the current wheel + 2 recent, GCs abandoned staging after 24h) and, for package-manager installs, the CLI package itself, with a per-manager removal table. Everything an uninstall leaves behind, it names.

**Verdict: cleanest uninstall in this whole research sweep.** The rule "remove only what a manifest proves we wrote" is the entire design, and it is applied uniformly across five surfaces.

---

## 8. What is genuinely GOOD

1. **Install ≠ setup.** `curl | sh` writes a venv and a symlink and nothing else; every mutation of the agent's config needs a second, typed command. `OMH_RUN_SETUP` defaults to `0` and there is a branch that *warns you* when you set setup flags without it. Copy this verbatim.
2. **Registration by pointer.** One line in host config buys a whole namespace OMH owns. Minimal blast radius; trivially reversible; the uninstall is a one-line delete.
3. **Manifest-with-hashes as the ownership boundary.** Ownership is never inferred from path shape — always read from a manifest. `identity_conflicts.py` says it best: *"A directory named `omh` that OMH did not install is precisely the case the attribution exists to catch, and a path-shaped guess would call it OMH's own."*
4. **Four-condition pruning + retain-and-report.** Only delete what we recorded, that is byte-identical, and that has a live replacement. Anything the user edited survives and gets named.
5. **`--force` is not sovereign.** `OMH_SECURITY=strict` makes `--force` stop overriding local modifications. A confirmation flag that a security posture can veto is a rare and correct design.
6. **Install-provenance detection via PEP 610.** Detecting `uv tool` vs `pip git+` vs editable-checkout vs Homebrew Cellar vs npm-stamped-env, and routing the upgrade to the real owner instead of creating a second conflicting install.
7. **One artifact, five channels.** The npm package *vendors the exact wheel*, SHA-256-verifies it, refuses symlinks in its path, and downloads nothing. Homebrew formula is rendered from the verified release URL+hash. The release workflow refuses to publish unless pyproject, `version.py`, wheel metadata, npm metadata, tag, and formula all agree.
8. **Post-install smoke test.** Import the plugin, register into a fake context, assert every declared tool and hook appears. "Files copied" is not "it works."
9. **Refuse rather than guess.** The YAML editor raises on shapes it does not understand. `capability-policy` refuses an unrecognized family name and prints all six. Rule import refuses injection-shaped and credential-shaped content with reasons.
10. **Separate manifest for user-namespace content.** Imports live under `skills/imported/` with their own manifest so managed pruning structurally cannot reach them.
11. **Foreign-rules ingestion as an on-ramp.** Reading `.cursorrules` / `.clinerules` / `.windsurfrules` / copilot-instructions removes a migration tax and steals users from four competitors at once.
12. **`omh doctor` with 20 checks**, including `identity_conflicts`, `plugin_hook_integrity`, `guidance_projection`, `security_posture`, and a `plugin_bundle_current` staleness check.
13. **Comments that explain the incident, not the code.** Nearly every non-obvious decision carries a paragraph naming the observed failure ("an observed machine went from 92 skills to 184", "Windows CI hit it first only because the suite runs ~2.4x slower there"). This is a rare and valuable house style.
14. **Explicit claim boundaries everywhere.** A recurring string: *"Prepared handoffs are not execution, review, CI, merge-readiness, or merge evidence."* An agent framework that structurally refuses to let preparation be reported as execution is an unusually honest design.
15. **The evidence allowlist is a token-prefix allowlist run with `shell=False`** (`plugin_bundle/omh/config.yaml`), scoped to `project_root`. Correct shape for a probe tool.

---

## 9. What is genuinely BAD

1. **No third-party extension contract at all.** This is the disqualifying flaw for an "oh-my-*" framework. `docs/ADDING-A-SKILL.md` requires touching 6+ Python modules and editing exact-count fixtures in 4 test files. With 123 forks and 216 catalogued ecosystem projects, **not one of them can plug into OMH.** oh-my-zsh's entire value was `custom/` + `plugins=(...)`. OMH has neither.
2. **289k lines of Python to ship 114 markdown files.** src:content is roughly 2,500:1. The workflow-skill payload is maybe 400 KB of markdown; the machinery around it is a small operating system. Most of it — `wrapper/`, `surfaces/`, `codegraph/`, `conformance/`, `evidence/`, `quality/` (60 files), `workflows/` (144 files) — is product, not packaging, and it is fused to the packaging.
3. **165 CLI subcommands.** `accept-plan`, `adapter-quality`, `agent-review-list`, `batch-stage`, `blueprint-show`, `domain-retire`, `goal-driver-observe`, `observe-codex`, `qa-ladder`, `recurring-intent-show`, `sticky-rule`… No user holds this. The README papers over it with "just say the trigger in chat", which is an admission the CLI is not the product surface.
4. **114 always-loaded skills is a context-budget problem the repo openly concedes.** `DEFAULT_SKILL_PROFILE = "full"` — every install pays for all 114. The code carries `FULL_PROFILE_SKILL_BODY_CHAR_LIMIT`, a `context_cost_warning` in the manifest, `omh docs skill-context-cost`, a density gate, and `RECONCILE_CONTEXT_COST_NOTE` (*"Every installed skill adds per-turn context weight to every Hermes request"*). All of that machinery exists because the default is wrong. The honest fix is lazy/JIT loading, not a warning.
5. **The name is unowned.** 1,299 + 857 + 302 stars across three same-named products. rlaope had to ship a 493-line runtime conflict detector because of it. A framework whose identity is contested cannot host an ecosystem.
6. **Hand-rolled line-based YAML editing.** The right *goal* (preserve comments and formatting), but it will meet a config it cannot parse. The mitigation — raise instead of guess — is correct, and it means some users will hit a hard `omh setup` failure on a legal YAML file.
7. **Configuration sprawl.** A user's OMH behaviour lives in `~/.omh/setup-profile.json` + four separate files under `~/.omh/routing/` + five keys inside `~/.hermes/config.yaml`. There is no single "here is my configuration" file and no `omh config show` that unifies them.
8. **Documentation asymmetry.** 38 docs, 4 localized READMEs, a Pages site — and the *only* page telling a user how to add their own content (`ADDING-A-SKILL.md`) is written for a repo maintainer, opens with "Every surface below is enforced by a test", and ends with a section about a schema-overlap probe.
9. **Agent-authored velocity is a governance risk.** 1,097 PRs in 90 days with 2 of the top 3 committers being AI agents. The test suite (195k lines, exact-count fixtures) is what holds it together — and those exact-count fixtures are themselves a symptom: a contract so wide it must be pinned by literal integers in four files.
10. **README overclaims relative to code.** The "Recommended models" table lists nine categories of specific frontier models as "editable recommendation chains", with a careful disclaimer that this is "prepared routing configuration, not provider availability, credential, dispatch, or execution evidence." The disclaimer is honest; the table still reads as a benchmark. The gap between what the code guarantees and what the README's imagery implies is the widest in this repo.

---

## 10. Transfer to oh-my-musecode (compiled Rust binary host)

### Transfers directly — steal these

| Pattern | OMH source | Musecode mapping |
| --- | --- | --- |
| **Install ≠ setup, default off** | `install.sh:38` `OMH_RUN_SETUP=0` | `curl \| sh` installs the `omm` binary only. `omm setup` is what ever touches `~/.muse/` or `.muse-plugin/`. Language-agnostic. |
| **Registration by pointer** | `config_adapter.py:643` `ensure_external_dir` | Add **one** entry to Muse's skills/rules search path pointing at `~/.omm/skills`. Muse's config crate already has skills/rules/workflows/agents/settings subsystems — find the equivalent of `skills.external_dirs` and write exactly one line into it. |
| **Per-file SHA-256 install manifest + refuse-on-drift** | `install/manifest.py`, `installer.py:181` | `~/.omm/manifest.json`. This is *directly* what Muse's `.muse/lock.json` (provenance/quarantine/allowed_tools) and `.muse/skills.lock` are for. Write the lock, verify before every refresh, refuse when a hash moved. |
| **Four-condition prune, retain-and-report** | `installer._prune_orphaned_skills` | Only delete a file that is (a) a catalog name, (b) manifest-recorded at that path, (c) already replaced, (d) byte-identical. Everything else is retained and named in the output. |
| **`--force` vetoed by security posture** | `installer._overwrite_allowed` + `system/security_posture.py` | Maps onto Muse's quarantine/allowed_tools model exactly: `.muse/lock.json` quarantine should be able to override a user's `--force`. |
| **Separate manifest for the user namespace** | `manifest.py:44-52` `IMPORTED_SKILLS_DIR_NAME` | Reserve `~/.omm/skills/custom/` (or `local/`), record it in a **second** lock, and make managed pruning structurally unable to see it. This is the cheapest way to buy a real extension contract. |
| **Post-install register smoke test** | `plugin_pack._register_smoke` | After writing `.muse-plugin/plugin.json`, actually invoke `muse` (or the MSP host) to confirm the plugin loads and the declared tools/hooks register. Rust makes this easier: shell out to the binary you just registered against. |
| **Hook integrity + revocation ledger** | `install/hook_integrity.py` | Direct match for `.muse/hooks.json` + `.muse/lock.json`. Per-hook digest, revocation as *data* not absence, exclusion list naming the capability lost and the command that restores it. |
| **Install-provenance-driven self-update** | `commands/setup.py:162-235` | Rust equivalent: read your own `std::env::current_exe()`, plus a provenance stamp written at install time (`~/.omm/install-provenance.json`: `{manager: "homebrew"\|"cargo"\|"curl"\|"npm", root, entrypoint}`). Then upgrade through the owner and re-exec, guarded by a `OMM_UPDATE_REENTERED` env var. |
| **One artifact, many channels** | `docs/DISTRIBUTION.md` | For Rust: **one set of signed per-target binaries** is the artifact. Homebrew formula, npm launcher, and cargo-binstall metadata all point at the *same* release asset with the same SHA-256. Refuse to release unless every version surface agrees. |
| **Uninstall = "remove only what a manifest proves we wrote"** | `commands/setup.py:1657` | Same rule, plus the same honesty: name the residue you deliberately leave (caches, the binary itself). |
| **Foreign-config ingestion as an on-ramp** | `workflows/external_rule_import.py` | **Huge for Musecode.** Muse's loader already recognises `.claude-plugin` and `.codex-plugin`. Extend the same idea to `.cursorrules`, `.clinerules`, `.windsurfrules`, `CLAUDE.md`, `AGENTS.md` → `~/.omm/skills/imported/<slug>/`. Keep OMH's guardrails verbatim: explicit-root only, no symlinked sources below the root, bounded count and size, injection- and credential-shape refusal with reasons, separate manifest. |
| **Refuse rather than guess on config mutation** | `config_adapter._validate_*_mutation_shape` | Muse config is JSON (`hooks.json`, `plugin.json`, `lock.json`) — **strictly easier than YAML**. Use `serde_json` with a preserve-order map, and still refuse on shapes you did not expect rather than normalizing them. |
| **Vendored, commit-pinned discovery index with a claim boundary** | `catalogs/awesome_hermes_agent_catalog.json` | Ship `awesome-musecode.json` pinned to an upstream commit + README sha256, surfaced by `omm ecosystem`. Cheap, honest, no server. |
| **Provenance table for reconstructed content** | `docs/SKILL-SOURCES.md` | If you reconstruct skills from Claude Code / Codex ecosystems, keep the `reviewed_ref` table and the upstream-drift automation. It is also your license defence. |
| **Doctor as a first-class product** | `maintenance/doctor.py`, 20 checks | Rust suits this well. Include an `identity_conflicts` equivalent from day one — Muse ingesting `.claude-plugin` and `.codex-plugin` means name collisions across three ecosystems are the *default*, not the exception. |

### Does NOT transfer

- **Anything Python.** The venv, `pip install --force-reinstall`, `python3 -m venv`, PEP 610 `direct_url.json`, `importlib.resources`, `setuptools` package-dir mapping — all gone. A Rust binary has no runtime to isolate: you ship a static binary, and the "isolated environment" problem disappears entirely. **This deletes `install_into_venv`, `install_into_python`, `find_omh_command`'s `sysconfig` probe, the whole npm Python-bridge (`packaging/npm/bin/python_bridge.py`, `lib/python.js`, `lib/cache.js`) — roughly the entire install substrate.** Musecode's installer should be ~150 lines, not 531 + 34 KB of PowerShell.
- **The npm/bun channel *as designed*.** OMH's npm package exists to vendor a wheel and find a Python. For Rust, npm is only viable as a thin per-platform-binary fetcher (the `optionalDependencies` pattern esbuild/swc use). The SHA-256 verification and symlink refusal transfer; the cache-tree machinery does not.
- **`OMH_PIP_ARGS` / `OMH_INSTALL_MODE`** and every escape hatch that exists because Python installs are ambiguous. Delete them.
- **The line-based YAML editor.** Muse config is JSON. Use a real parser with order preservation. Keep the *policy* (refuse unknown shapes, preserve what you did not write), drop the 744-line hand-rolled implementation.
- **`.mjs` TUI widget + YAML skins.** These are Hermes' Ink-TUI extension points. Muse Code is a Rust TUI; whatever its statusline/theme surface is, it will be a different format. The *pattern* — ship theme files into the host's own theme dir with a manifest so refresh is safe — transfers; the artifacts do not.
- **165 CLI subcommands.** Do not port the surface area. In Rust with `clap`, a wide command tree is *cheaper* to write and just as expensive to learn. Target ~15.
- **Full-profile-by-default with 114 skills.** Muse Code's `MUSE_EXPERIMENTAL_*` gates plus its skills subsystem should let oh-my-musecode ship a small core and load the rest on demand. Do not inherit the context-cost problem and then build a warning system for it.
- **The agent-authored PR velocity model** and its consequence, exact-count test fixtures across four files. That is a symptom of an over-wide contract, not a practice to copy.

### The one thing to do differently from day one

OMH's *packaging* is close to state of the art and its *extension contract does not exist*. For oh-my-musecode, invert the priority: **design `~/.omm/skills/custom/` + a second lockfile + an `enabled` list in `~/.omm/config.json` before writing the first bundled skill.** OMH already proves the mechanism works — `skills/imported/` with its own manifest is exactly that shape, built for foreign rule files. Point it at the user instead, and the framework becomes extensible for essentially no additional engineering.

And Muse Code hands you something Hermes never gave OMH: a loader that already ingests `.claude-plugin` and `.codex-plugin` manifests. **Third-party content for oh-my-musecode can be Claude Code plugins.** Do not invent a fourth plugin format — adopt the two the host already reads, add your lockfile and provenance layer on top, and inherit two existing ecosystems instead of curating a private catalog of 114.

---

## Verification

Adversarial re-verification run 2026-09-01 in an independent clone
(`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-rlaope-oh-my-hermes/repo`,
`git clone --depth 50`), with all counts re-run from scratch rather than read back from the report.

**Verdict: MOSTLY_SOLID.** The repo is real, the traction is real, and essentially every
load-bearing structural claim reproduced exactly — including line numbers and verbatim in-source
quotations. Four numeric slips and one mis-attribution, all listed below. Nothing was fabricated.

### Priority 1 — Existence and traction: CONFIRMED exactly

`gh api repos/rlaope/oh-my-hermes` returned, field for field:

| Claim | Observed | |
| --- | --- | --- |
| 1,299 stars | `stargazers_count: 1299` | ✅ |
| 123 forks | `forks_count: 123`, `network_count: 123` | ✅ |
| 6 open issues | `open_issues_count: 6` | ✅ |
| 6 watchers | `subscribers_count: 6` | ✅ (`watchers_count` is 1299, GitHub's stars alias — the report used the right field) |
| 92,803 KB | `size: 92803` | ✅ |
| Created 2026-06-03 | `2026-06-03T13:00:34Z` | ✅ |
| Last push 2026-09-01T11:35:05Z | identical | ✅ |
| MIT | `license.spdx_id: MIT` | ✅ |
| 11 releases, v1.0.0 2026-06-09 → v2.0.0 2026-08-29 | `gh release list` returns exactly 11; v1.0.0 `2026-06-09T03:18:36Z`, v2.0.0 `2026-08-29T04:03:23Z` | ✅ |
| 153 issues / 1,097 PRs | `search/issues` total_count 153 and 1097 | ✅ |

**Name contention: CONFIRMED exactly.** `gh search repos oh-my-hermes` returns precisely seven
repos with that exact name, at the exact star counts claimed: rlaope 1299 (Python),
Salomondiei08 857 (Shell), witt3rd 302 (Python), HERMESquant 9 (TypeScript), shuangxipop1 2
(Python), codenote-net 2 (Python), relover1986 0 (Shell). Top three sum to 2,458 — the report's
"~2,460 split three ways" is right.

**Agent authorship: CONFIRMED, with one arithmetic error.** `gh api .../contributors`:
rlaope 1245, sionic-khope 868, frirenai 577, dependabot 13. `git log -5` shows three of the last
five commits authored by `Killua AI <khope@sionic.ai>` — the report's "the HEAD commit author is
literally 'Killua AI'" is literally true. README.md:108 names Friren and Killua as AI-agent
collaborators, exactly as described.

### Priority 2 — Shipped counts: re-run independently

Every headline count reproduced to the digit:

```
find skills -name SKILL.md | wc -l                  → 114   ✅
find skills -maxdepth 1 -type d -name 'ulw-*'       →   9   ✅
find skills -maxdepth 1 -type d -name 'omh-*'       → 105   ✅
find skills -name '*.md' ! -name SKILL.md | wc -l   →  55   ✅
grep -c 'SkillDefinition(' catalog_definitions.py   →  86   ✅
wc -l src/skills/catalog_definitions.py             → 6914  ✅
find src -name '*.py' | xargs wc -l                 → 289,331 ✅
ls src/commands/*.py | wc -l                        →  67   ✅
find . -path ./.git -prune -o -type f -print | wc -l→ 1578  ✅
```

Module sizes cited in the report are all exact: `config_adapter.py` 744, `hook_integrity.py` 571,
`identity_conflicts.py` 493, `external_rule_import.py` 477, `install.sh` 531, `install.ps1`
34,118 bytes.

Asset inventory confirmed from source, not README: `plugin.yaml` declares exactly **14**
`provides_tools`, exactly **6** `provides_hooks` (`on_session_end`, `post_tool_call`,
`pre_llm_call`, `pre_tool_call`, `pre_verify`, `transform_tool_result`), and
`provides_memory_provider: omh`. `roles/` holds exactly 9 `.md` files. `src/omh/skins/` holds
exactly 4 (`omh`, `omh-amber`, `omh-crimson`, `omh-mono`). `src/omh/tui_widgets/omh-status.mjs`
exists. `src/routing/trigger_packs/` holds exactly ja/ko/zh. Four localized READMEs present.
`HERMES_MIXTURE_CATEGORY_CHAINS` has exactly **9** keys (ultrabrain, deep, architect,
unspecified-high, unspecified-low, quick, writing, visual-engineering, artistry). `TEAM_PROFILE_PACKS`
contains exactly **4** `TeamProfilePack(` constructions and **21** ids total → 4 packs + **17**
roles, matching "4 packs / 17 subagent role files". `MCP_HOST_CONFIG_INSTALL_HOSTS =
("claude-code", "codex", "opencode", "cursor")` — exactly the 4 claimed. The vendored catalog's
`item_count` is **216** with `len(items) == 216`, and the section histogram matches every figure
(110 / 30 / 28 / 18 / 16 / 5 / 4 / 4 / 1). The pinned provenance is verbatim: repo
`0xNyk/awesome-hermes-agent`, commit `27389ad544f923ee67b455457c214c679f26ad8a`, readme_sha256
`33d58901d6f8…`, `retrieved_at 2026-07-27`, and the claim boundary string reproduced word for word.

The "ZERO slash commands" claim is supported by the source: there is no slash-command asset dir,
and `docs/INSTALLATION.md:797` states "OMH workflows are skill triggers, not Hermes slash commands".

**Corrections (all minor, all in the direction of over-precision, none material):**

1. **Test-suite figure conflates two measurements.** "195,752 lines across 427 test files" mixes
   denominators. `find tests -name 'test_*.py'` → **427 files / 192,080 lines**;
   `find tests -name '*.py'` → **458 files / 195,752 lines**. Both numbers are real; they are not
   the same set. Correct phrasing: ~192k lines across 427 `test_*.py` files (195,752 across all 458).
2. **CLI subcommand count is 168, not 165.** Unique `add_parser("…")` names across
   `src/commands/*.py` → **168** (416 total registrations). All nine distinctive examples the
   report quotes to make the point — `accept-plan`, `adapter-quality`, `batch-stage`,
   `domain-retire`, `goal-driver-observe`, `observe-codex`, `qa-ladder`, `recurring-intent-show`,
   `sticky-rule` — are present in the real list. The argument is unaffected; the integer is off by 3.
3. **Docs count is 36, not 38.** `find docs -type f` → 36, all markdown. The "4 CI workflows"
   half of that claim is exact (`auto-release.yml`, `ci.yml`, `pages.yml`, `release.yml`).
4. **Agent-authored commit share is ~1,445, not ~1,690.** sionic-khope 868 + frirenai 577 = **1,445**
   of ~2,719 total contributor commits (53%). The report's "~1,690 of ~2,700" overstates by ~245.
   The qualitative claim — two of the top three committers are AI agents, and agent-authored commits
   are a majority — survives.
5. **`docs/SKILL-SOURCES.md` has 13 shipped rows covering 11 unique upstream repos**, not "13
   external repos" (14 distinct GitHub URLs appear across the whole file including the candidate
   section). The table structure is exactly as described: OMH skill | category | upstream repo |
   paths studied | license | `reviewed_on` | `reviewed_ref`, and the header documents the
   `upstream-skill-update` issue automation verbatim.

### Priority 3 — Extension contract: CONFIRMED, and the report understated nothing

This was the claim most at risk of being an aspiration read off a README. It is not. I read
`docs/ADDING-A-SKILL.md` (171 lines) in full and the code paths behind it.

- The doc opens, verbatim: *"Every surface below is enforced by a test; skipping one fails CI with
  an actionable message naming the file and structure to edit."* ✅
- The 6+ modules a new skill must touch are named exactly as the report lists them:
  `src/skills/catalog_definitions.py`, `src/routing/recommend.py`,
  `src/routing/trigger_packs/<lang>.json`, `src/plugin_bundle/omh/awareness.py`,
  `src/wrapper/contract.py`, `src/routing/action_copy.py`, `src/quality/chat_card_coverage.py`. ✅
- §3 "Exact-count fixtures" names exactly four test files — `test_routing_precision.py`,
  `test_cli.py`, `test_hermes_ux_quality.py`, `test_release_smoke.py` — and ends with the
  literal sentence *"Grep those four for the old count."* ✅
- §4 lists four doc-regeneration commands ✅; §6 is the skill-density gate ✅; §7 is the
  schema-overlap probe, so "ends with a schema-overlap probe" is literally accurate ✅.
- Searches for a third-party surface (`enabled_modules`, `custom_skills`, `user_skills`,
  `third_party`) return nothing. `omh install` exists but its help string is *"Refresh the managed
  OMH skill pack without changing Hermes registration"* — it is a self-refresh, not a package
  installer. **The "no marketplace, no `omh install <third-party-pack>`" claim holds.** ✅

The four *partial* extension surfaces are all real code:

- **Registration by pointer**: `ensure_external_dir()` is at `config_adapter.py:643` exactly as
  cited, calls `_validate_external_dirs_mutation_shape()` on entry, and `raise ValueError(...)`
  on an inline shape it cannot handle. ✅
- **Foreign-rules import**: `external_rule_import.py` handles `.cursorrules`, `.cursor/rules/*.mdc`,
  `.clinerules`, `.windsurfrules`, `.github/copilot-instructions.md`; writes to
  `<skills_dir>/imported/<slug>/SKILL.md`; `IMPORT_MANIFEST_FILE = "skills-import-manifest.json"`
  is a genuinely separate manifest; `MAX_IMPORT_SOURCES = 40` and `MAX_SOURCE_BYTES = 131_072`
  (= 128 KB) confirm the "≤40 sources ≤128 KB each" bound to the byte; symlink refusal
  (`not clinerules.is_symlink()`) is in the discovery walk. Every guardrail claimed is in the code. ✅
- **Routing overrides**: `mixture_chain_overrides_path()` returns `~/.omh/routing/model-chains.json`
  and `MIXTURE_CHAIN_OVERRIDES_SCHEMA_VERSION = "mixture_chain_overrides/v1"` — exact. ✅
- **Capability policy**: `src/capabilities/toggles.py` defines
  `CAPABILITY_POLICY_SCHEMA_VERSION = "omh_capability_policy/v1"`, and the alias map resolves to
  exactly **six** canonical families (`retain_knowledge`, `delegate_coding_and_ship`,
  `learn_and_gather`, `plan_and_decide`, `create_materials_and_visuals`, `operate_and_observe`),
  with `disabled_families` / `enabled_families` — subtraction only, as claimed. ✅
- **Host tap**: `hermes skills tap add rlaope/oh-my-hermes` appears at README.md:164 and in
  `docs/ARCHITECTURE.md:201`, which states the flat repo layout exists so the tap lister can see it. ✅

### Priority 4 — Install / update: faithful, including line numbers

`install.sh` verified line by line against the report:

- `OMH_RUN_SETUP="${OMH_RUN_SETUP:-0}"` is on **line 38**, exactly as cited. ✅
- The "setup flags without opt-in" warning branch is the `elif` on **line 518**, exactly as cited,
  printing *"Setup options were not applied because install.sh now installs the command only by
  default."* ✅
- Line 359 reads the release tag from **only** the `Location:` header of a `/releases/latest` 302
  (`curl -sSI … | awk 'tolower($1) == "location:"'`), with a `wget --max-redirect=0` fallback —
  no API token, one header read. ✅
- Line 383 builds the predictable wheel URL
  `oh_my_hermes-$OMH_RELEASE_VERSION-py3-none-any.whl`; line 7's comment states the
  ~2.7 MB wheel vs ~44 MB branch archive tradeoff. ✅
- Venv at `$XDG_DATA_HOME/omh/venv` else `~/.local/share/omh/venv` (lines 21-24) ✅;
  `OMH_FORCE_LINK` default 0 with the replace-refusal at lines 294-300 ✅.
- All five install channels appear verbatim in README.md at lines 126, 132, 138, 144, 150. ✅
  `rlaope/homebrew-tap` exists (last push 2026-08-29). npm `oh-my-hermes` exists, latest **2.0.0**,
  6 published versions. ✅
- npm launcher SHA-256 verification is real: `packaging/npm/lib/cache.js` has `sha256File()` and
  `if (sha256File(path, allowHardlinks) !== wheelSha256)`. ✅

Update mechanism verified in code:

- `_command_package_self_update_plan` is at `src/commands/setup.py:716` — exact line. ✅
- `SELF_UPDATE_REENTRY_ENV = "OMH_UPDATE_COMMAND_PACKAGE_REENTERED"` (setup.py:231),
  `COMMAND_PACKAGE_MANAGER_ENV = "OMH_COMMAND_PACKAGE_MANAGER"` (121),
  `_homebrew_prefix_root()` (217), `_direct_url_update_guidance()` (164) reading
  `direct_url.json` via `importlib.metadata` (182). Every mechanism named in the report exists
  under the name given. ✅
- `local_modifications()` at `manifest.py:84`; `installer.py:184` raises the literal string
  *"local modifications detected; rerun with --force or resolve: "*. ✅
- `--force` vetoed by posture is real: `installer.py:121` routes
  `"installer_overwrite_local_modification"` through `resolve_approval_tier(confirmed=force,
  posture=resolve_security_posture())`, and the docstring at line 117 says the override stops
  working *"once `OMH_SECURITY=strict` is set"*. ✅
- `_carry_installed_at_when_nothing_moved` (installer.py:234) and both prune functions exist. ✅
- Hook integrity: `hook_integrity.py:42` reads *"Revocation is data, never absence."* ✅
- Register smoke test: `install/plugin_pack.py:69-73` surfaces `plugin_import_smoke` and
  `plugin_register_smoke`. ✅
- The distinctive incident comments are all real, verbatim: *"an observed machine went from 92
  skills to 184 after the relabel"* (installer.py:286), *"only because the suite runs ~2.4x slower
  there"* (installer.py:251), and identity_conflicts.py:18 *"…is precisely the case the attribution
  exists to catch, and a path-shaped [guess]…"*. ✅

**One mis-attribution.** The report says *"`_prune_orphaned_skills` / `_prune_flat_layout_skill_directories`
delete a directory only when it is (a) a catalog skill name, (b) manifest-recorded at that path,
(c) already replaced by a live categorized directory, and (d) byte-identical."* Those four
conditions belong to **`_prune_flat_layout_skill_directories` alone** (docstring, installer.py:292-297,
where they are enumerated (a)-(d) in exactly that wording). `_prune_orphaned_skills` has **three**
conditions, and its first is the *inverse*: prune only when the name is **absent** from the full
catalog (installer.py:404-407 — "(a) recorded in the prior manifest, (b) absent from the full
catalog, and (c) sha-unmodified"). Both are genuinely conservative and both retain user-modified
directories, so the report's conclusion is right; it merged two different safety rules into one
and attributed the stricter one to both.

### Weaknesses spot-check

The disqualifying-flaw framing survives scrutiny, and so does the context-cost criticism:

- `DEFAULT_SKILL_PROFILE = "full"` at `installer.py:36`, `SKILL_PROFILES = ("core", "full")` at
  line 30. `_resolved_skill_profile()` (setup.py:1174-1192) falls through to `DEFAULT_SKILL_PROFILE`
  when no manifest exists — i.e. **a first `omh setup` with no flags installs the full 114-skill
  profile**. The report's weakness #4 is confirmed at the code level, not just from docs. The
  compensating warning apparatus is also real: `_context_cost_warning()` (installer.py:186) emits a
  message telling the user to *"prefer core unless this workspace genuinely needs the complete
  catalog"*, and `FULL_PROFILE_SKILL_BODY_CHAR_LIMIT = 856342` sits in `maintenance/release.py:558`.
  An 856 KB always-loaded body budget is a fair thing to call a design smell.
- Uninstall flags all exist as argparse arguments in `setup.py:4294-4298`: `--registration-only`,
  `--remove-files` (help text literally says "Legacy mode"), `--purge` ("Alias for --all"),
  `--keep-command`. The quoted honesty line is real, in `commands/language.py:259`:
  *"If `omh` still runs after uninstall, the command package is still on PATH…"* — the report
  paraphrases it slightly ("that means the command package is still on PATH") but the substance
  is exact. `docs/INSTALLATION.md` is 2,329 lines and its `## Uninstall` section sits at ~2,258,
  matching the cited 2255-2330 range.

### Bottom line for the oh-my-musecode design

Nothing in the transfer analysis rests on a claim that failed verification. The two structural
findings the design depends on — **(a)** OMH's packaging/manifest/prune/uninstall discipline is
real, implemented, and worth stealing, and **(b)** OMH has no third-party extension contract, with
`docs/ADDING-A-SKILL.md` as the proof rather than the rebuttal — both hold under adversarial
re-reading. The "one inversion" recommendation stands unchanged.
