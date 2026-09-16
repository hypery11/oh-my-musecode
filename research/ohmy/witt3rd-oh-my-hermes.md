# Teardown: witt3rd/oh-my-hermes (OMH)

**Status: CONFIRMED — repo exists and was cloned and read.**

- URL: https://github.com/witt3rd/oh-my-hermes
- Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/witt3rd-oh-my-hermes/`
- HEAD at clone: `2a98d38b43010a438b316fb48dbe68a3c8ee8fed` — "test: include triage roles in OMH role catalog (#23)", 2026-08-05
- Target agent: **Hermes Agent** (NousResearch/hermes-agent — verified real: 239,350 stars, pushed 2026-09-01). Python agent, not a compiled binary.
- Ancestor: oh-my-claudecode (Yeachan-Heo/oh-my-claudecode — verified real: 38,933 stars).

---

## 1. Existence — CONFIRMED

GitHub API returns 200 with full metadata. `git clone --depth 50` succeeded. 76 commits total (from the API `Link: rel="last"` header on `commits?per_page=1`). Everything below is read from the working tree, not from the README's claims — where the two disagree, I say so.

---

## 2. What it SHIPS (real counts from `find | wc -l`)

Everything lives under one directory, `plugins/omh/`. There is no top-level `skills/`, no `bin/`, no `install.sh`, no `.github/`.

| Category | Count | Path |
|---|---:|---|
| Skills (dirs with a `SKILL.md`) | **10** | `plugins/omh/skills/*/SKILL.md` |
| Role prompts (subagent personas) | **15** | `plugins/omh/references/role-*.md` |
| Lifecycle hooks (Python) | **3** | `plugins/omh/hooks/{llm,session,tool}_hooks.py` |
| Custom agent tools (Python) | **2** | `plugins/omh/tools/{state,evidence}_tool.py` |
| Skill reference files | **19** | `plugins/omh/skills/*/references/*` |
| Skill templates | **1** | `plugins/omh/skills/*/templates/*` |
| Skill scripts | **1** | `plugins/omh/skills/omh-ralph-task/scripts/verify-redaction-marker.sh` |
| Project-seed templates | **2** | `plugins/omh/templates/dot-omh-{readme.md,gitignore}` |
| Docs (markdown) | **13** | `docs/**.md` |
| Test functions | **201** | `grep -c 'def test_' plugins/omh/tests/*.py` |
| Python LOC | **4,766** | `find plugins -name '*.py'` |
| Markdown LOC (whole repo) | **18,951** | |

**Slash commands: 0. Subagent *definitions*: 0 (roles are prompt files, not agent manifests). MCP configs: 0. Settings presets: 0. Statusline: 0. Output styles: 0.** I grepped for all of these (`-iname "*command*" -o -iname "*agent*" -o -iname "*mcp*" -o -iname "*statusline*" -o -iname "*output-style*" -o -iname "settings*"`) and the only hit in the entire tree is `docs/research/hermes-multiagent.md`. This is a deliberately narrow product: it is **not** a "kitchen sink of 142 agents." It ships **one workflow methodology** (consensus planning → verified execution) and the infrastructure to run it.

The 10 skills, in two tiers:

| Skill | LOC | Version | Role |
|---|---:|---|---|
| `omh-deep-research` | 368 | 1.0.0 | worker: decompose → parallel search → synthesize → verify citations |
| `omh-ralplan` | 145 | 2.0.0 | worker: Planner→Architect→Critic consensus, ≤3 rounds |
| `omh-ralplan-driver` | **1,249** | 1.1.0 | **driver**: how to *run* a ralplan; 26 numbered pitfalls |
| `omh-deep-interview` | 241 | 2.0.0 | worker: Socratic requirements interview |
| `omh-ralph` | 255 | 2.0.0 | worker: one-task-per-invocation verified execution |
| `omh-ralph-driver` | 644 | 1.0.0 | **driver**: batching, evidence, strike categorization, commit hygiene |
| `omh-ralph-task` | 191 | 1.0.0 | **executor contract** for a single ralph task |
| `omh-triage` | 183 | 0.1.0 | worker: multi-role backlog triage |
| `omh-triage-driver` | 125 | 0.1.0 | **driver** |
| `omh-autopilot` | 228 | 2.0.0 | composition of the above |

The 15 roles: analyst, architect, code-reviewer, critic, debugger, executor, planner, research-synthesist, research-verifier, researcher, security-reviewer, test-engineer, triage-maintainer, triage-skeptic, verifier.

---

## 3. INSTALL — read end to end

**There is no installer script.** No `install.sh`, no `curl | bash`, no npm package, no Makefile, no CI. Install is delegated to the **host's own package manager**, plus one Python side-effect.

### Path A — the host's tap system (README lines 36-39)

```bash
hermes skills tap add witt3rd/oh-my-hermes
hermes skills install omh-deep-research omh-ralplan ...
```

`hermes skills tap` is a **real upstream Hermes feature**, not something OMH built. I verified it in `NousResearch/hermes-agent`: `hermes_cli/skills_hub.py` (2,134 lines) and `website/docs/user-guide/features/skills.md`. Taps are Homebrew-style: a GitHub repo of `SKILL.md` directories, no server, no registry signup, stored in `~/.hermes/skills/.hub/taps.json`, with trust levels (`builtin` / `trusted` / `community`, new taps default to `community`) and a security scan on install.

**This documented install command is broken.** `do_tap("add", repo)` in `skills_hub.py:1543` calls `mgr.add(repo)` with no path argument; the upstream docs state "The `hermes skills tap add` CLI defaults new taps to `path: "skills/"`". oh-my-hermes has **no top-level `skills/` directory** — its skills are at `plugins/omh/skills/`. Adding the tap therefore indexes zero skills. The user must hand-edit `~/.hermes/skills/.hub/taps.json` to `{"repo": "witt3rd/oh-my-hermes", "path": "plugins/omh/skills/"}`. This is not theoretical: issue #25 is a production user reporting exactly this — *"第一次跑 `hermes skills tap add witt3rd/oh-my-hermes` 后 `hermes skills search omh-deep-research` 返回 `No skills found`"*.

The README's own fallback line is wrong for the same reason: *"Or copy `skills/<name>/` to `~/.hermes/skills/omh/` manually"* — `skills/` does not exist (`ls -d skills` → No such file or directory).

### Path B — the plugin, which self-installs skills as a registration side effect

`plugins/omh/__init__.py:register(ctx)` calls `_install_skills()` **first**, before registering anything:

```python
def register(ctx):
    _install_skills()          # ← unguarded, runs on every session boot
    ...
    ctx.register_tool("omh_state", ...)
    ctx.register_hook("pre_llm_call", pre_llm_call)
```

`_install_skills()` (`__init__.py:19-73`) does:
1. Resolve dest: `from hermes_cli.config import get_hermes_home` → `<home>/skills/omh`, **falling back to `Path.home()/".hermes"/"skills"/"omh"` on any exception**.
2. `skills_dest_root.mkdir(parents=True, exist_ok=True)`.
3. For each bundled skill dir: `if dest.exists(): continue` — **never overwrite**; else `copytree` to `<name>._installing` then `rename()` (atomic on same fs).

### What it writes to the user's machine

| Path | Written by | Contents |
|---|---|---|
| `~/.hermes/plugins/omh/` (or `$HERMES_HOME/plugins/omh/`) | user, manually (`cp -r` or `ln -s`) | the whole Python plugin + skills + roles + config.yaml |
| `~/.hermes/skills/omh/<skill>/` | `_install_skills()`, automatically on plugin load | copies of the 10 skill dirs |
| `~/.hermes/skills/.hub/taps.json` | the host's `hermes skills tap add` | tap subscription |
| `<project>/.omh/state/` | `omh_state` tool, on first write | `{mode}-state.json`, `{mode}--{slug}.json`, `.lock` files, `dispatched/*.json` breadcrumbs |
| `<project>/.omh/README.md` | `_seed_dot_omh()` (`omh_state.py:40`) | seeded once, never overwritten |
| `<project>/.omh/.gitignore` | `_seed_dot_omh()` | seeded once, never overwritten |
| `<project>/.omh/{specs,plans,research,logs,progress}/` | skills at runtime | durable + ephemeral artifacts |

Nothing else. **No dotfile mutation, no shell rc edits, no PATH changes, no global state outside the Hermes home.** The `.omh/` convention is explicitly project-local, and the seeded README enforces it in prose: *"All paths are project-local. Never write to `~/.omh/` or any global location."*

### Real install failures (issues #8, #25 — production users, not hypotheticals)

1. **Symlink kills the whole plugin.** `mkdir(exist_ok=True)` raises `FileExistsError` when the path is a *symlink*, not a directory. `CONTRIBUTING.md` tells developers to create exactly that symlink (`ln -s "$PWD/plugins/omh/skills" ~/.hermes/skills/omh`). Because `_install_skills()` is unguarded at the top of `register()`, the exception propagates and **Hermes unloads the entire plugin — all 2 tools and 3 hooks gone**, with an errno-17 message that blames the filesystem. (Issue #8.)
2. **`HERMES_HOME` ignored.** The `Path.home()/".hermes"` fallback is wrong when the host uses `HERMES_HOME=/opt/data`; skills land where nothing looks for them. (Issue #25 §1. Issue #19 tracks migrating to the host-native `ctx.register_skill()`.)
3. **The host's own supply-chain scanner blocks `hermes plugins install`** because `subprocess`/`pytest`/`uv` keywords appear in the plugin. The user had to `cp -r` around it. (Issue #25 §2.)
4. **Toolset name not whitelisted in CLI mode**: `hermes chat -t terminal,web,omh` → `Warning: Unknown toolsets: omh`, so `omh_state`/`omh_gather_evidence` are invisible to subagents and the entire `[omh-role:]` injection mechanism silently no-ops. (Issue #25 §3 — the reporter calls this "最严重的 bug".)

That is four independent install-layer failures in a project whose *runtime methodology* the same reporter says "完全工作" (works completely). **Distribution, not design, is where this project bleeds.**

---

## 4. EXTENSION CONTRACT

Genuinely mixed. There are three real seams and one large gap.

**Seam 1 — `config.yaml` is the single source of truth, with no Python fallback.** `omh_config.py:6` states it outright: *"config.yaml is the single source of truth — there is no hardcoded Python fallback. If config.yaml is missing or unreadable, get_config() returns {}."* A user extends the evidence tool's command allowlist by adding a line to `config.yaml` — no code change:

```yaml
evidence:
  allowlist_prefixes:
    - "npm test"
    - "cargo clippy"
    ...
```

The allowlist is **token-prefix matched** (`_matches_allowlist`, `evidence_tool.py:28`), so `"npm test"` matches `npm test --verbose` but not `npm testing-malicious`. That comment is in the shipped config, teaching the user the semantics at the point of extension. This is the best-designed extension point in the repo.

**Seam 2 — the role catalog is directory-driven.** `get_role_catalog()` (`omh_roles.py:42`) is literally `glob("role-*.md")` over `plugins/omh/references/`. Drop `role-designer.md` into that directory and `[omh-role:designer]` works — no registration, no manifest edit, no index file. Name validation is a regex (`^[a-zA-Z0-9_-]+$`) that doubles as path-traversal defense.

**Seam 3 — the host's copy-never-overwrite rule.** `_install_skills()`: `if dest.exists(): continue  # already installed; never overwrite user's copy`. A user who edits `~/.hermes/skills/omh/omh-ralph/SKILL.md` keeps that edit forever. Likewise `_seed_dot_omh()` only writes files that don't exist.

**The gap: there is no `custom/` directory, no `enabled_modules` list, no plugin protocol, no override-by-layering.** Roles and skills are *replaced by editing files in the install tree*, which is the same file that an update would want to replace. Adding a role means writing into `~/.hermes/plugins/omh/references/` — inside OMH's own directory. There is no `~/.hermes/plugins/omh-custom/` that shadows the shipped one. So the extension story is "edit in place, and we promise never to touch it again" — which works exactly once and then permanently forks the user.

---

## 5. UPDATE — this is the weakest part of the design

**There is no update mechanism, and no lockfile for content.** (`uv.lock` exists but pins *Python* dependencies, not OMH assets. There are **0 releases and 0 git tags** — verified via the API.)

The failure is structural. `_install_skills()` is idempotent by *skipping*, not by *reconciling*:

```python
dest = skills_dest_root / skill_dir.name
if dest.exists():
    continue  # already installed; never overwrite user's copy
```

Once a skill directory exists in `~/.hermes/skills/omh/`, **no future version of the plugin will ever refresh it**. Upgrade the plugin from v0.1.0 to v0.9.0 and every skill file stays frozen at whatever shipped first. Skill `version:` frontmatter fields exist (1.0.0 through 2.0.0) but nothing reads or compares them. The user's only upgrade path is `rm -rf ~/.hermes/skills/omh` and restart — which silently destroys any edits they made under Seam 3 above.

The safe-update design the project *needs* already exists upstream and it doesn't use it. Hermes's bundled-skill sync (`website/docs/user-guide/features/skills.md`) keeps `~/.hermes/skills/.bundled_manifest` mapping each skill name to its **origin content hash** at last sync; on each sync it rehashes the local copy — unchanged means safe to pull the new version and re-record the hash, changed means user-modified and skipped forever. Plus `hermes skills reset <name>` as the re-baseline escape hatch. That is a proper three-way merge discriminator. OMH implements the "skip" half and none of the "detect unchanged and update" half.

Issue #19 ("Migrate `_install_skills()` from legacy copytree to `ctx.register_skill()`") is the acknowledgement that the whole self-install path was the wrong call.

---

## 6. REGISTRY

**OMH ships no registry of its own — and that is the right call.** It publishes itself *into* the host's registry.

Hermes's Skills Hub is a real multi-source index (`skills_hub.py`, `unified_search`) spanning: `official` (curated catalog), `github` (direct repo/path installs and user-added taps, with default taps for openai/skills, anthropics/skills, huggingface/skills, NVIDIA/skills), `well-known` (`/.well-known/skills/index.json` served from any website), `url` (a bare `SKILL.md` over HTTP), plus `clawhub`, `lobehub`, `browse-sh`. Results carry `source` and `trust_level`, official entries outrank community mirrors on name collision, and a security scan plus a third-party warning panel gate first install. Taps can ship a `skills.sh.json` at repo root (per the skills.sh schema) to supply real category groupings instead of a tag-derived guess.

So: **discovery is neither ad hoc nor OMH's problem.** OMH is a *tap* — which is why the broken default tap path in §3 is such an expensive bug: it's the single interface between this project and the entire distribution channel, and it doesn't work as documented.

The plugin distribution channel is separate (`hermes plugins install <owner/repo>`, `hermes plugins enable`, `hermes plugins doctor`) and is where the supply-chain scanner false-positive bites.

---

## 7. UNINSTALL

**Clean, but undocumented for real users.** `CONTRIBUTING.md` gives only the developer case:

```bash
rm ~/.hermes/plugins/omh ~/.hermes/skills/omh
```

with the honest note *"(Symlinks only — your repo is untouched.)"*. For a `cp -r` install the same two `rm -rf`s work. There is no `hermes plugins uninstall` invocation documented, no cleanup of `~/.hermes/skills/.hub/taps.json` (the tap entry survives), and — deliberately — no cleanup of per-project `.omh/` directories.

Residue after uninstall: the tap entry, and every project's `.omh/`. The `.omh/` residue is *intentional and correct*: `.omh/plans/` and `.omh/specs/` are tracked-in-git decision records (ADRs), so deleting them on uninstall would be destroying the user's own work. `.omh/state/`, `.omh/logs/`, `.omh/progress/` are gitignored ephemera. The seeded `.omh/.gitignore` encodes exactly this split, and `.omh/README.md` explains the reasoning to a human reading the repo six months later. This is the single most quietly excellent thing in the project.

---

## 8. TRACTION (real numbers)

Via `curl https://api.github.com/repos/witt3rd/oh-my-hermes` and `gh api`, on 2026-09-01:

| Metric | Value |
|---|---|
| Stars | **302** |
| Forks | **29** (network_count 29) |
| Watchers/subscribers | 4 |
| Open issues | **8** (all genuine ISSUEs, zero are PRs) |
| Created | 2026-04-07 |
| Last push | **2026-08-05** (~4 weeks stale at time of writing) |
| Last metadata update | 2026-08-31 |
| Total commits | **76** (from the `Link: rel="last"` header on `commits?per_page=1`) |
| Releases | **0** |
| Tags | **0** |
| Contributors | **5** — witt3rd 50, adkap 14, adamkaplan 9, forge-witt3rd 2, dfitz1138 1 |
| License | MIT |
| Language | Python |
| Repo size | 511 KB |
| CI | **none** (`.github/` does not exist) |
| Topics | none set |

Interpretation: 302 stars on 76 commits and 5 contributors in 5 months is real interest, but this is fundamentally **one person's system** (witt3rd = 50/76 commits, plus a `forge-witt3rd` bot account). Zero releases and zero tags mean there is no versioned artifact anyone can pin — combined with §5, a user has literally no way to say "give me OMH v2.0". The open issues are unusually high-quality (long, reproducible, source-cited) which suggests the users it does have are serious ones.

---

## 9. What is GOOD and what is BAD — opinionated

### Genuinely good

**1. The driver/worker skill split is the best idea in the repo.** Each workflow ships twice: `omh-ralplan` (145 lines: what the workflow *is*) and `omh-ralplan-driver` (1,249 lines: how to *drive* it, with 26 numbered pitfalls P1-P26 harvested from real sessions). Then `omh-ralph-task` is a third artifact: the contract for the *executor* being dispatched. Three audiences, three documents, loaded independently. The pitfalls are dated and sourced — P24 "Delegation-for-vantage works", P16 "Apply the counterfactual deference test in Round 2", P26 "Deliver decisions-first; deep review is for the archive". This is an operations manual accreting from lived failure, not a feature list. Almost every oh-my-* project writes one flat skill and loses this knowledge.

**2. Role injection that never touches the parent's context.** `[omh-role:executor]` is a ~20-character marker in the `delegate_task` goal string. The `pre_llm_call` hook fires *in the subagent's session*, sees the marker in `user_message` on `is_first_turn`, loads `role-executor.md`, and returns `{"context": ...}` to be merged into that subagent's system prompt. The role prose — potentially thousands of tokens across 15 roles — **never passes through the orchestrator's context window**. `docs/plugin.md` names the insight precisely: `delegate_task` passes `goal` as `user_message` to the child's `run_conversation()`, so `pre_llm_call` on first turn is the natural injection point, "no new Hermes primitives required." Finding a zero-cost injection point in the host's existing lifecycle instead of asking for a new hook is exactly right.

**3. `pre_tool_call` as fail-fast defense in depth.** The same marker is validated in the *parent* before dispatch, so a typo'd `[omh-role:excutor]` warns immediately rather than after a subagent burns tokens and returns unroled output. Deliberately non-blocking (`tool_hooks.py:8`: "warns but does not prevent"). Correct call — a config framework should not be able to hard-fail the host's tool dispatch.

**4. Filesystem discipline that a systems programmer would sign off on.** `_atomic_write` is `os.open(O_WRONLY|O_CREAT|O_TRUNC, 0600)` → write → `flush()` → `fsync()` → `os.replace()`, with tmp cleanup in the exception path. Advisory locks use `O_EXCL` create-or-fail with `{pid, session_id, started_at}` payloads and **stale-lock reaping via `os.kill(pid, 0)`** — including the subtlety that `PermissionError` means "alive, owned by someone else" (`omh_state.py:156`). Release checks `session_id` match unless the holder's pid is dead, with `force=True` for admin paths. State files carry a `_meta` envelope with `schema_version` and a mismatch warning. Instance IDs are slugified with an explicit `--` separator chosen *because* mode names contain hyphens, with a comment proving the separator can't collide. This is better plumbing than most production Python.

**5. The evidence tool's threat model is stated, not assumed.** Token-prefix allowlist (not substring), `shell=False` as the primary defense with a metacharacter regex as belt-and-suspenders, `workdir` forced `is_relative_to(project_root)`, hard caps that clamp LLM-supplied values (`timeout = min(timeout, 300)`, `truncate = min(truncate, 50_000)`), tail-not-head truncation because build failures are at the end. The config comment explains *why* `"npm test"` and not `"npm "` — because the latter would permit `npm publish`. It teaches the user the threat model at the exact place where they'd weaken it.

**6. `omh_delegate`'s subagent-persists contract.** Hermes returns only a subagent's *final summary* to the parent, so a long research document can't come back through the return value. OMH inverts it: precompute the absolute output path, `mkdir` it, write a `{id}.dispatched.json` breadcrumb, append a deliberately blunt contract to the goal (*"The file you write IS the deliverable. The path is the receipt."*), then after dispatch just `Path(...).is_file()` and write `{id}.completed.json`. **The file is the protocol; the return value is a receipt.** And when the v0 design turned out to be architecturally impossible — it assumed `delegate_task` was an importable Python callable when it's actually an agent-loop tool — the fix (`aa7a4a8 fix(omh-delegate): split into prepare/finalize (Bug D1)`) is documented *in the module docstring with the date it surfaced and how*. Two separate append-only breadcrumb files rather than one read-modify-write file is the right concurrency call.

**7. `docs/hermes-constraints.md` corrects itself in public.** It carries a dated status note saying several entries "previously listed as hard constraints are now configurable defaults," adds a "Defaults vs hard limits (read this first)" table, and `docs/research/hermes-multiagent.md` §4 contains a same-day correction: an earlier draft claimed `toolsets=["file"]` gave a read-only worker; the author spot-checked `hermes-agent/toolsets.py:147`, found `{"tools": ["read_file", "write_file", "patch", "search_files"]}`, and published the correction inline with the source line number. Then propagated the consequence into the README as a known gap (A5) *and* filed `docs/upstream-prs/per-tool-scoping.md`. A config framework that reads its host's source and dates its own errors is rarer than it should be.

**8. Honest self-assessment as a shipped artifact.** `docs/gaps.md` opens "OMH v1.0 replicates the core execution pipeline (~85%) but not the full OMC feature surface (~60% overall)." `docs/omc-comparison.md` has a "Deliberate Design Differences" table that separates *choices* from *gaps* and records that consensus review called OMC's three named challenge modes "cargo cult." The README publishes a **cost envelope** (5-8 `delegate_task` calls happy path, ~10-12 with a retry, 14-16 worst case before BLOCKED). Almost nobody publishes the token bill.

**9. Self-bootstrapping, with the receipts committed.** `.omh/plans/` and `.omh/research/` contain the actual multi-round consensus transcripts (`round1-planner.md`, `round2-critic.md`, `CONSENSUS.md`, `HANDOFF.md`) used to design OMH's own skills, including three plans explicitly renamed `*-superseded.md`. The tool was used to build itself and the debate is in the repo.

### Genuinely bad

**1. The headline install command does not work.** §3. `hermes skills tap add witt3rd/oh-my-hermes` indexes zero skills because the tap default path is `skills/` and the repo has none. The README's manual fallback cites the same non-existent path. Confirmed by a production user in issue #25. A project whose entire distribution strategy is "be a tap" has a broken tap.

**2. Install as a side effect of plugin registration, unguarded.** `register()` calls `_install_skills()` before registering anything, with no `try/except`. A `FileExistsError` from a *symlink* — the exact setup `CONTRIBUTING.md` instructs developers to create — takes down all 2 tools and all 3 hooks (issue #8). Registration and provisioning are different lifecycle phases and must not share a failure domain. If provisioning must happen at load, it belongs in a `try/except` that logs and continues; the plugin's tools are useful even with zero skills installed.

**3. No update path at all.** §5. `if dest.exists(): continue` means skills freeze at first install forever. Combined with 0 releases and 0 tags, there is no version anyone can pin, diff, or roll back to. The host already implements content-hash-based safe update (`.bundled_manifest` + origin hash + `hermes skills reset`); OMH implements the half that skips and none of the half that updates.

**4. `Path.home()/".hermes"` hardcoded as the fallback.** The code *tries* `get_hermes_home()` first and then guesses when the import fails — and the guess is wrong on any host using `HERMES_HOME`, silently, in a way that manifests as "skills not found" three commands later (issue #25 §1). A wrong-but-plausible fallback is worse than a hard failure with a clear message.

**5. No CI, no releases, no tags, on 201 tests.** The tests exist and are good. Nothing runs them on push. There is no `.github/` at all. `run_integration.sh` requires the user's actual Hermes venv (`~/.hermes/hermes-agent/venv/bin/python3`), so integration coverage is unreproducible off the maintainer's machine.

**6. Extension = editing files inside the install tree.** §4. No `custom/` layer, no shadowing, no enabled-modules list. Combined with the never-overwrite rule this means a user who adds one role has silently forked, permanently, with no signal that they've done so.

**7. Bus factor 1, and the docs know it.** `omh-ralph-driver` and `omh-ralplan-driver` both end with a section literally titled "Do-not-lose content (if this skill has to be regenerated from memory)." That is a maintainer writing a message to whoever inherits this. 50 of 76 commits are one person's.

**8. Documentation drift is a live pattern, not a one-off.** `docs/plugin.md` says "Nine shared role prompts" and then lists ten in the table — while the repo ships fifteen. `omh_state` is described as "8 actions" in the same doc; the schema enum has 13. The prose is written by hand and the counts are not derived from the tree.

---

## 10. What transfers to a compiled Rust binary host (Muse Code) — and what does not

### Transfers wholesale (this is markdown + JSON + filesystem semantics — the host language is irrelevant)

- **The `.muse/` project-directory convention with a tracked/ignored split.** Copy `.omh/README.md` and `.omh/.gitignore` verbatim in spirit: `plans/`, `specs/`, `research/` tracked in git because they are decision records like ADRs; `state/`, `logs/`, `progress/` gitignored because they are per-session runtime. Seed both files once on first use and never overwrite. **This is the single highest-value idea to steal and it costs nothing to implement.** It maps directly onto Muse Code's existing `.muse/` (which already holds `hooks.json`, `lock.json`, `skills.lock`).
- **Driver/worker/executor skill triples.** Ship `omc-plan` (what it is), `omc-plan-driver` (how to run it, with numbered dated pitfalls), `omc-plan-task` (the contract for a dispatched worker). Loaded independently so the orchestrator never pays for the executor's prose. This is a pure information-architecture decision — nothing about it is Python.
- **The subagent-persists contract.** Precompute an absolute output path, mkdir it, inject a blunt "the file IS the deliverable, the path is the receipt" contract into the goal, verify with a stat afterward, write two append-only breadcrumb JSON files (`{id}.dispatched.json`, `{id}.completed.json`). Rust makes this *better*: `tempfile::NamedTempFile::persist` gives you the tmp→fsync→rename atomicity for free and correctly, and you can return a typed `DelegateOutcome` enum instead of hand-rolling `ok` vs `ok_strict` to work around Python truthiness (the `AC-1` note in `omh_delegate.py:34-36` is an artifact of a dynamic language and simply evaporates).
- **Marker-based role injection.** `[muse-role:NAME]` in a delegation goal + a first-turn hook that loads `references/role-{name}.md` in the *child's* session. Muse Code has a hooks subsystem (`.muse/hooks.json`) and a rules/agent-definitions surface. Keep the parent-side fail-fast validation as a non-blocking warning.
- **Directory-as-registry for roles.** `glob("role-*.md")` with a `^[a-zA-Z0-9_-]+$` name regex that doubles as traversal defense. Trivially a `read_dir` + regex in Rust. No manifest, no index file, no registration.
- **The evidence-gathering threat model.** Token-prefix allowlist (not substring), no shell, workdir confined under project root, hard caps clamping model-supplied timeout/truncation, tail-not-head truncation. In Rust this is `std::process::Command` with explicit argv — **the `shell=False` guarantee is structural rather than a flag you can forget**, and `_SHELL_METACHAR_RE` becomes unnecessary belt-and-suspenders rather than load-bearing. Ship the allowlist in `.muse/` config with the "why `npm test` and not `npm `" comment intact.
- **Advisory locks with stale-pid reaping.** `O_EXCL` create-or-fail, `{pid, session_id, started_at}` payload, `kill(pid, 0)` liveness with `PermissionError`→alive. Rust: `OpenOptions::new().create_new(true)` and the `nix` crate. Same semantics, better types.
- **Config as the single source of truth with no hardcoded fallback.** In Rust this is `serde` with `#[derive(Default)]` on the config struct — you get the "no divergent hardcoded copy" property by construction.
- **Publishing the cost envelope and a dated, self-correcting constraints doc.** Pure discipline. Free. Do it.
- **The self-bootstrapping receipts.** Committing the consensus transcripts that designed the system is a documentation strategy, not a technology.

### Does NOT transfer

- **`_install_skills()` — the entire self-install-on-load mechanism.** It is a Python-import side effect (`register(ctx)` executing `shutil.copytree` at plugin load). A compiled Rust host has no equivalent and should not grow one. Muse Code already has `.muse/skills.lock` and `.muse/lock.json` (provenance/quarantine/allowed_tools) — **provisioning belongs in an install command that writes the lockfile, never in a load-time side effect.** Copy the *idempotence intent*, discard the mechanism, and you get issue #8 and half of issue #25 for free.
- **`pyproject.toml` / `uv.lock` / `pyyaml` as a runtime dependency.** OMH's plugin needs a Python interpreter with pyyaml *in the host's venv*, and `CONTRIBUTING.md` ships a verification command for it (`uv run python -c "import yaml"`). For a Rust binary the config parser is compiled in. This eliminates an entire class of install failure — but it also means **oh-my-musecode cannot ship executable extension logic the way OMH does**, unless Muse Code exposes a scripting/WASM surface or you ship a companion binary. Plan for declarative-only from day one.
- **Hooks as importable Python callables.** `ctx.register_hook("pre_llm_call", pre_llm_call)` passes a function object. Muse Code's `.muse/hooks.json` is declarative — hooks are presumably commands or matchers, not closures. Role injection must therefore be expressed as *data* (a marker→file mapping the binary resolves) rather than *code*, or as a subprocess hook. This is the single biggest porting question and it should be answered before anything else is designed.
- **`shutil.copytree` + `os.replace` atomic-rename install.** Correct in Python; in Rust use `tempfile` + `fs::rename`, and honor `lock.json` provenance rather than inventing a parallel install path.
- **`get_hermes_home()` with a `Path.home()` fallback.** Muse Code already has `~/.config/muse/`. Resolve it once through the binary's own config resolution and **fail loudly if it can't be resolved** — never guess.

### The strategic lesson for oh-my-musecode

OMH's actual strategy is: **build almost no infrastructure, publish into the host's registry, and put all the value in prose plus a thin state layer.** No installer, no registry, no updater — the host provides all three. That is exactly right, and Muse Code hands you an even better version of it: `.muse-plugin/plugin.json` *plus* recognition of `.claude-plugin` and `.codex-plugin`, so a single manifest reaches three ecosystems.

But OMH also demonstrates the cost of that strategy done sloppily: **when the host owns distribution, your one integration point must be exactly right, and OMH's is broken.** The tap path is wrong, the home resolution is wrong, the toolset name isn't whitelisted, and the plugin dies on a symlink — four failures at the seam, in a project whose methodology users describe as working completely. For oh-my-musecode: write an end-to-end install test that runs `muse` against a clean `$HOME` in CI *before* writing the second skill, pin the `.muse-plugin/plugin.json` path conventions against the binary's actual loader, and implement content-hash safe-update (origin hash + user-modified detection + a `reset` escape hatch) from the first release rather than shipping `if dest.exists(): continue` and discovering in month five that no user has ever received an update.

---

## Verification

**Verdict: MOSTLY_SOLID.** Independently re-derived on 2026-09-01 from a fresh clone plus live GitHub API, in `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-witt3rd-oh-my-hermes/`. The repo is real, the traction is exact, every headline count reproduces to the digit, and every code seam described below was read line-by-line rather than inferred from the README. Nine precision/attribution errors are corrected below; none of them changes a conclusion, and the report's single most consequential claim — that the documented install command is broken — is not merely confirmed but **strengthened**, because I proved the mechanism at the source level and against the live GitHub API, which the original report could not.

### 1. Existence and traction — CONFIRMED, exact

`https://github.com/witt3rd/oh-my-hermes` returns HTTP 200; repo id `1203994445`, default branch `master`. Live API values versus the report, all matching:

| Field | Report | Measured |
|---|---|---|
| stars | 302 | 302 |
| forks / network_count | 29 / 29 | 29 / 29 |
| watchers (subscribers) | 4 | 4 |
| open issues | 8 | 8 |
| created / pushed / updated | 2026-04-07 / 2026-08-05 / 2026-08-31 | 2026-04-07T15:33:04Z / 2026-08-05T12:34:16Z / 2026-08-31T17:14:55Z |
| license / language / size | MIT / Python / 511 KB | MIT / Python / 511 |
| commits | 76 | 76 (`Link: rel="last"`, and `git log \| wc -l` = 76) |
| releases / tags | 0 / 0 | `[]` / `[]` |
| contributors | witt3rd 50, adkap 14, adamkaplan 9, forge-witt3rd 2, dfitz1138 1 | identical (sums to 76) |
| CI | `.github/` does not exist | confirmed absent |

Upstream repos also verified real: `NousResearch/hermes-agent` (239,355 stars — report said 239,350, hours of drift), `Yeachan-Heo/oh-my-claudecode` (38,933 stars, exact).

### 2. Shipped-content counts — CONFIRMED, exact

Re-ran every count independently:

- **10 skill dirs** under `plugins/omh/skills/` ✅
- **3,629 lines** across the 10 `SKILL.md` files ✅ *exact*. (Scoping note: all 31 markdown files under `skills/` total 6,033 lines. The 3,629 figure is SKILL.md-only.)
- **15** `references/role-*.md` ✅ · **19** skill reference `.md` ✅ · **1** template ✅ · **1** script (`verify-redaction-marker.sh`) ✅
- **13** docs markdown files, **6** of them under `docs/research/` ✅
- **201 test functions** ✅ *exact* — across 8 `test_*.py` modules (test_state 75, test_evidence 29, test_hooks 29, test_roles 26, test_config 15, test_omh_delegate 13, test_integration 9, test_init 5)
- **18,951 markdown LOC** repo-wide ✅ *exact*
- **4,766 Python LOC** ✅ — this is `plugins/omh/` only; repo-wide git-tracked Python is 4,919 (`examples/plan-and-execute/demo.py` adds 153)
- **`omh_state` has exactly 13 actions** ✅ *exact* (`init, read, write, clear, check, list, list_instances, cancel, cancel_check, lock, unlock, lock_check, load_role`)
- **`omh-ralplan` = 145 lines, `omh-ralplan-driver` = 1,249 lines** ✅ *both exact*
- Negative sweep ✅ — no slash commands, no MCP config, no settings preset, no statusline, no output style, no subagent manifest anywhere in the tree (I found *zero* hits, even cleaner than the report's "only hit is a docs research file")

### 3. Extension contract — CONFIRMED, it is code, not aspiration

All three seams exist in the source, not just the README.

- **Seam 1 (config as sole source of truth):** `omh_config.py:5` literally reads *"config.yaml is the single source of truth — there is no hardcoded Python fallback"*, and `get_config()` returns `{}` when the file is missing. ✅ The `config.yaml` evidence-allowlist comment is verbatim as quoted: *"not `npm ` which permits npm publish, npm install"*. ✅
- **Seam 2 (directory-driven role catalog):** `omh_roles.py:42-49` is literally `{p.stem.removeprefix("role-"): p for p in sorted(_REFERENCES_DIR.glob("role-*.md"))}` with `ROLE_NAME_RE = re.compile(r"^[a-zA-Z0-9_-]+$")` and the traversal-defense comment. ✅ Dropping `role-designer.md` into `references/` genuinely makes `[omh-role:designer]` work with no registration.
- **Seam 3 (never overwrite):** `__init__.py:47-48` is `if dest.exists(): continue  # already installed; never overwrite user's copy`, and `omh_state.py:_seed_dot_omh()` skips any file that exists. ✅
- **The gap is real:** no `custom/` overlay, no shadowing, no enabled-modules list anywhere in the tree. ✅

Also verified verbatim: `_atomic_write` (`os.open(O_WRONLY|O_CREAT|O_TRUNC, 0600)` → write → flush → `fsync` → `os.replace`, tmp unlink on the exception path); advisory locks (`O_EXCL` create-or-fail, `{pid, session_id, started_at, lock_key}` payload, `os.kill(pid, 0)` liveness with `PermissionError` → alive, session_id-checked release with `force=True` override); the `_meta` envelope with `schema_version` and mismatch warning; the `'--'` instance-separator comment proving non-collision. The evidence tool's threat model is exactly as described — `shell=False` commented as "primary metacharacter defense", token-prefix `_matches_allowlist`, `is_relative_to(project_root)` workdir confinement, `_MAX_TIMEOUT = 300` / `_MAX_TRUNCATE = 50_000` clamps, and `combined[-truncate:]` tail truncation. The `omh_delegate.py` contract string is verbatim at line 165: *"The file you write IS the deliverable. The path is the receipt."*, with the AC-1 `ok`/`ok_strict` note at lines 34-36 exactly as cited.

### 4. Install and update — CONFIRMED, and the broken-tap claim is now *proven*

No installer exists: no `install.sh`, `Makefile`, `bin/`, `package.json`, or `setup.py`. ✅ README's install block is exactly `hermes skills tap add witt3rd/oh-my-hermes` + `hermes skills install …`, and the manual fallback is exactly *"Or copy `skills/<name>/` to `~/.hermes/skills/omh/` manually."* ✅

The report asserted the tap indexes zero skills and cited issue #25 as confirmation. **Issue #25 does not actually say that** (see correction 1) — but the claim is true, and I proved it three ways in upstream `NousResearch/hermes-agent`:

1. `hermes_cli/skills_hub.py:1544` — `do_tap("add")` calls `mgr.add(repo)` with **no path argument**. (File is 2,134 lines, exactly as reported.)
2. `tools/skills_hub.py:4109` — `def add(self, repo: str, path: str = "skills/")`, writing `{"repo": …, "path": "skills/"}` into `taps.json`.
3. `tools/skills_hub.py:919-946` — `_list_skills_in_repo` fetches `https://api.github.com/repos/{repo}/contents/{path}`, **single level, no recursion**, and returns `[]` on non-200.

Live check: `GET /repos/witt3rd/oh-my-hermes/contents/skills` → **404**; `…/contents/plugins/omh/skills` → **200**. The tap therefore resolves to a path that does not exist and indexes nothing. The headline install command is genuinely broken, for exactly the reason stated.

The update critique is likewise confirmed. `if dest.exists(): continue` freezes skills at first install; 0 releases and 0 tags means nothing is pinnable. The upstream design OMH doesn't use is real and precisely as described: `tools/skills_sync.py` maintains `~/.hermes/skills/.bundled_manifest` in `skill_name:origin_hash` form, rehashes the local copy each sync, updates only when `bundled_hash == origin_hash`, marks divergence `user_modified` and skips it forever, with `hermes skills reset <name>` (and `--restore`) as the escape hatch at `hermes_cli/skills_hub.py:1264`. Registry claims all check out too: `taps.json` under `_hub_dir()` = `skills/.hub/`, default taps `openai/skills` / `anthropics/skills` / `huggingface/skills` / `NVIDIA/skills` with the signed `skill.oms.sig` comment, `trust_level` of `builtin|trusted|community` defaulting to `community` for new taps, `unified_search` spanning official / skills-sh / well-known / github / clawhub / lobehub / browse-sh, and the root `skills.sh.json` grouping sidecar. Uninstall is verbatim from CONTRIBUTING.md including *"(Symlinks only — your repo is untouched.)"*.

Quotes spot-checked and exact: `docs/gaps.md` "~85% … ~60% overall"; `docs/omc-comparison.md:56` "cargo cult"; README cost envelope 5–8 / 10–12 / 14–16; `run_integration.sh` hardcoding `${HOME}/.hermes/hermes-agent/venv/bin/python3`; the `.omh/README.md` line *"It belongs in the repo for the same reason an ADR belongs in the repo"*; and the `docs/research/hermes-multiagent.md` §4 self-correction, dated **2026-04-22 day-of**, citing `hermes-agent/toolsets.py:147`, finding `write_file` and `patch` in the `file` toolset, keeping A5 as a README known gap, and filing `docs/upstream-prs/per-tool-scoping.md`. (In today's hermes-agent HEAD the `file` toolset sits at `toolsets.py:201`, and does contain `read_file, write_file, patch, search_files` — the doc's line number is stale, the substance is right.)

### Corrections

1. **"Confirmed by a production user in issue #25" (of the broken tap path) — misattributed.** Issue #25 §1 reports the *symptom* (`hermes skills search` → "No skills found" after `tap add`) but diagnoses a **different root cause**: the plugin's `Path.home()/".hermes"` fallback versus `$HERMES_HOME`. That diagnosis is itself a non-sequitur — the plugin's self-install path has nothing to do with what a tap search indexes. The report's own mechanism is the correct one; it just wasn't the reporter's. Fixed above with direct source + live-API proof.
2. **"`Path.mkdir(exist_ok=True)` raises FileExistsError on a SYMLINK" — over-general** (an inaccuracy inherited from issue #8's own wording). Empirically on CPython: a symlink pointing to an **existing directory** does *not* raise (`mkdir` catches `FileExistsError` and re-checks `is_dir()`, which follows the link). Only a **broken** symlink, or one pointing at a non-directory, raises. Issue #8's actual repro (`ln -s /nonexistent/path`) and its real-world case (a Linux-made symlink resolving nowhere on macOS) are both the broken variety. The failure is real; its trigger is narrower than "the exact setup CONTRIBUTING.md instructs".
3. **"with no try/except" — partially wrong.** `register()` itself is unguarded ✅ and the `skills_dest_root.mkdir()` at `__init__.py:41` is outside any handler ✅, so the blast radius claim holds. But the per-skill `copytree` loop **is** wrapped (`__init__.py:50-57`, logging a warning and cleaning up `tmp_dest`). Only the mkdir and the `register()` call are unguarded.
4. **The `omh` toolset whitelist bug is upstream, not OMH's.** The report lists it as an OMH weakness. Issue #25's reporter states explicitly: *"这是 Hermes 上游的 bug，不是 OMH 的设计问题"* ("this is an upstream Hermes bug, not an OMH design problem"). The *consequence* for OMH users is real and the reporter does call it the most serious bug — but the attribution was dropped.
5. **"26 numbered dated pitfalls P1–P26" — there are 25.** `omh-ralplan-driver/SKILL.md` has 25 `### P` headings: P1–P22, P24, P25, P26. P23 has no section of its own (it survives only as inline references at lines 833 and 1194).
6. **"both end with a section literally titled 'Do-not-lose content (if this skill has to be regenerated from memory)'" — only one does.** `omh-ralplan-driver:1208` carries the full parenthetical; `omh-ralph-driver:620` is plain `## Do-not-lose content`.
7. **"1 config.yaml (role catalog …)" overstates the role catalog.** `config.yaml`'s `roles:` block lists **10** roles while **15** `role-*.md` files ship — and **nothing in the codebase reads `config["roles"]`**. `get_role_catalog()` globs the directory. The block is dead and already drifted. This *strengthens* Seam 2 and adds a datapoint to the drift weakness, but it is not a functioning catalog. (Similarly, `omh_config._deep_merge()` is defined and covered by 8 unit tests but never called by `get_config()`.)
8. **"8 open issues … zero PRs" is right but incomplete.** The repo has **14 PRs** total — 12 merged, 2 closed, 0 open. Directly relevant to the update critique: **PR #4, "fix(omh): replace copytree skill install with symlinks", was closed unmerged**, so the copytree install path has now survived both a rejected PR and an open issue (#19).
9. **Minor drift the report missed, supporting its own "documentation drift is a live pattern" weakness:** README's `hermes skills install` line names only **8** skills, omitting `omh-triage` and `omh-triage-driver`, both of which appear in the table immediately above it.

None of the above is a fabrication, and no correction overturns a strength, weakness, or transfer recommendation. The `transfers_to_musecode` analysis stands as written — including its central warning, which this verification only sharpens: when the host owns distribution, the single integration point must be exactly right, and OMH's demonstrably is not.
