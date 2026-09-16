# Teardown: Salomondiei08/oh-my-hermes

**Verdict: CONFIRMED — exists, cloned, read.**
Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/salomondiei08-oh-my-hermes/`
Commit read: `7ce4e3c3aca643e68409ec7a794bdd85c2b6d1bd` (main HEAD, 2026-07-04)
URL: https://github.com/Salomondiei08/oh-my-hermes

Target host: **Hermes Agent** (Nous Research) — a Python CLI/daemon with `profile`, `cron`,
`kanban`, `memory`, `gateway`, `skills`, `computer-use` subcommands, home at `~/.hermes/`.
Not a shell. Same structural class as Muse Code: a host binary with a filesystem config tree.

---

## 1. Existence and traction (real numbers)

`gh repo view Salomondiei08/oh-my-hermes --json ...` (2026-09-01):

| Metric | Value |
|---|---|
| Stars | **857** |
| Forks | **81** |
| Created | 2026-05-08 |
| `pushedAt` | 2026-07-28 — **but that is the `develop` branch** |
| **main HEAD** | **2026-07-04** (`7ce4e3c`) — main is 2 months stale |
| Commits (main) | **47** (`repos/.../commits?per_page=1` Link header → `page=47`) |
| Contributors | **3** — Salomondiei08 (39), jaythehardcoder (6), zbqshen (1) |
| Releases | **1** — `v0.0.1`, 2026-06-19 |
| Issues ever opened | **1** (`#8`, "How close is this to Oh-My-Openagent (OMO)?") |
| PRs ever | 10 (4 merged, 3 closed, **2 open**) |
| License | **NONE** — `licenseInfo: null`, no `LICENSE` file on disk |
| Languages | Shell 55282 B, TS 396 B, JS 393 B |
| Disk | 2038 KB (1.4 MB of that is `banner.png`) |

**857 stars : 1 issue : 47 commits : 3 contributors.** A healthy repo of this star count
normally carries dozens of issues. This ratio is the single strongest quantitative signal
that the stars are *name recognition*, not usage.

### The name is contested — five repos share it
`gh search repos "oh-my-hermes"`:

| Stars | Repo | Created | Pushed |
|---|---|---|---|
| **1299** | `rlaope/oh-my-hermes` | 2026-06-03 | **2026-09-01 (today)** |
| 857 | `Salomondiei08/oh-my-hermes` | 2026-05-08 | 2026-07-28 (develop) |
| 302 | `witt3rd/oh-my-hermes` | 2026-04-07 | 2026-08-05 |
| 9 | `HERMESquant/oh-my-hermes` | 2026-03-27 | 2026-03-28 |
| 8 | `eloklam/oh-my-hermes-agent` | 2026-05-10 | 2026-05-28 |

Our target is **second, not first**. It was created *before* the 1299-star one and lost.
`witt3rd`'s description reads "Inspired by oh-my-claudecode" — the lineage is explicit.
And the only issue ever filed is a user confusing this project with *another* "oh-my-*"
project (OMO). **The name attracts stars and simultaneously destroys identity.**

---

## 2. What it ships (real counts via `find`)

| Category | Count | Path |
|---|---|---|
| Skills | **36** `.md` | `skills/` |
| Agent role definitions | **7** `.md` | `agents/` (cto, pm, designer, dev, qa, ops, security) |
| Workflows | **6** `.md` | `workflows/` |
| Shell scripts | **13** `.sh` | `scripts/` (1524 lines total) |
| Docs | 8 `.md` | `docs/` |
| Templates | 4 | `templates/` (AGENTS.md.template, .env.example, 2 healthcheck stubs) |
| CI workflows | 2 | `.github/workflows/{test,release}.yml` |
| **Total files (non-.git)** | **85** | |

**Content volume:** skills 13,372 words / 2,601 lines; agents 2,402 words;
workflows 1,740 words; docs 6,127 words.

**Ships ZERO of:** slash commands, hooks, MCP server configs, settings presets,
statusline, output styles, plugin manifests. There is no MCP wiring at all —
`docs/improvements-to-hermes.md` §9 *asks Hermes upstream* for MCP examples.

Everything shipped is **prose markdown**. The "skills" are LLM procedure documents,
not executable units. That is the entire product.

### Skill schema (`skills/*.md`)
```yaml
---
name: create-skill
description: Use when ...   # MUST start "Use when" — this is the host's matcher input
version: 1.0.0              # per-skill semver
tags: [meta, skills, creation]
---
## Overview / ## When to Use / ## Prerequisites / ## Procedure / ## Pitfalls / ## Verification
```
Enforced (nominally) by `scripts/validate-skills.sh:19` `REQUIRED_FIELDS=(name description version tags)`
plus required sections at `:68`.

**Schema is not uniformly applied.** `agents/*.md` use a *different* frontmatter
(`name/role/persona/version`), and **2 of 6 workflows have no frontmatter at all** —
`workflows/deploy-and-monitor.md` and `workflows/ship-this-idea.md` start with `# Workflow: ...`.
The validator only lints `skills/`; agents and workflows are unvalidated.

---

## 3. INSTALL — read end to end (`install.sh`, 132 lines)

Entry: `git clone … /tmp/oh-my-hermes && bash /tmp/oh-my-hermes/install.sh`
(`curl | bash` supported: `install.sh:42-62` self-clones to `mktemp -d` when the sibling
`skills/`, `workflows/`, `agents/` dirs are absent, with a `trap cleanup EXIT`).

Preflight: `install.sh:30` requires `$HOME/.hermes` to already exist, else hard-exits with
the Hermes install command. `HERMES_HOME` env override is honoured throughout.

**Exactly what it writes:**

| Path | How | Mode |
|---|---|---|
| `~/.hermes/skills/*.md` | `cp` of 36 files (`install_md_dir`, `:73-87`) | default |
| `~/.hermes/workflows/*.md` | `cp` of 6 files | default |
| `~/.hermes/agents/*.md` | `cp` of 7 files | default |
| `~/.hermes/scripts/*.sh` | `install -m 700` of 13 files (`:104-108`) | **700** |

That is all `install.sh` does. **Flat `cp` into flat directories. No manifest, no backup,
no lockfile, no checksum, no namespacing, no dry-run.**

Good bit worth stealing (`install.sh:93-99`):
```bash
if [ "$SKILLS_INSTALLED" -eq 0 ] || [ "$WORKFLOWS_INSTALLED" -eq 0 ] || [ "$AGENTS_INSTALLED" -eq 0 ]; then
  echo "[ERROR] Refusing to report success after installing zero items."
```
The installer refuses to print success after a no-op install. This exists because the
`curl | bash` path previously installed nothing and reported OK. **Directly worth copying.**

Bug: `WORKFLOWS_INSTALLED` is computed at `:90`, checked at `:93`, then **reset to 0 and
recomputed by a duplicate loop at `:111-118`**. Dead duplicated code; the guard at `:93`
silently reads a value the summary later discards.

### The rest of the footprint (other scripts, not `install.sh`)
- `~/.hermes/profiles/<role>/agent-role.md` × 7 — `setup-cto.sh:202`
- `~/.hermes/.env` chmod 600 — `setup-integrations.sh:27`
- `~/.hermes/projects/<slug>.env` chmod 600 + `~/.hermes/projects/current` — `project.sh:48-52`
- `~/.hermes/oh-my-hermes/dead-letter/<project>/*.log` chmod 600 — `run-cron-safe.sh:29-55`
- `~/.hermes-backups/hermes-runtime-*.tar.gz` + `~/.hermes/reset-<ts>/` — `reset-runtime.sh:28-33`
- **Host-side state, not files:** `hermes profile create` × 7, `hermes kanban init`,
  4–5 `hermes cron add` jobs named `oh-my-hermes-*`, and `hermes memory set github-repo/github-username`
- **`~/.config/gh/hosts.yml` mutated** — `setup-cto.sh:244` runs `gh auth login --with-token`
- **In your project dir** (`bootstrap.sh`): creates `AGENTS.md`, `.env.example`,
  `src/app/api/health/route.ts` (only if Next.js detected, `:123-129`), and **appends
  `.env.local` to your `.gitignore`** (`:156-158`)

---

## 4. THE HEADLINE FINDING: `setup-cto.sh` is syntactically broken on main

`scripts/setup-cto.sh:380` contains a **committed, unresolved git merge conflict marker**:

```
>>>>>>> 9e3e68f (feat: add first-run server operating layer)
```

Verified:
```
$ bash -n scripts/setup-cto.sh
scripts/setup-cto.sh: line 380: syntax error near unexpected token `>>'
```
All 13 other scripts pass `bash -n`. This one does not. It is the **only** conflict marker
in the repo (`grep -rn '^>>>>>>> '`), and it is on `main` at HEAD.

Two further latent bugs in the same file: under `set -euo pipefail` (`:6`),
`[ -n "$PRODUCTION_URL" ]` (`:358`) and `[ -n "$GITHUB_REPO" ]` (`:369`) dereference
**unquoted-default variables** → `unbound variable` crash whenever the user hasn't exported them.

### Why this matters more than the bug itself

1. `setup-cto.sh` is **Step 4 of the advertised install flow** (`INSTALL_FOR_AGENTS.md`,
   `install.sh:131`, `docs/installation.md`). It is not a corner. It is *the* configuration step —
   the thing that creates profiles, kanban, and crons. The product does not work without it.
2. `install.sh` clones `--depth 1` from the **default branch (`main`)**. Every user who follows
   the documented install gets the broken file.
3. **The fix has been sitting in an open PR for five weeks.** PR **#9**,
   `docxology:fix/setup-cto-unbound-vars-and-merge-conflict`, "fix: resolve merge conflict and
   unbound variable crashes in setup-cto.sh", opened **2026-07-27**, still **OPEN**.
   An outside contributor independently found exactly this and was ignored.
4. The maintainer had already fixed **the identical class of bug elsewhere** —
   commit `fe296a4 "fix: resolve install.sh merge artifacts (syntax error at line 51)"` —
   and did not check the neighbouring file.

### CI structurally cannot catch it
`.github/workflows/test.yml` runs exactly two things: `bash install.sh` into a faked
`~/.hermes`, then `bash scripts/test.sh` with `TEST_MODE=1`.

- `scripts/test.sh` only does `[ -f ... ]` existence checks and greps `description:` lines.
- **No `bash -n`. No `shellcheck`. No lint job.**
- **`scripts/validate-skills.sh` is not invoked by CI at all.**
- `install.sh` never touches `setup-cto.sh` beyond `install -m 700`, so the file is copied,
  never parsed.

And `validate-skills.sh` — the one real schema linter — is **itself broken on macOS**:
`:31` uses `declare -A` (bash 4+), and macOS ships bash 3.2.57:
```
scripts/validate-skills.sh: line 31: declare: -A: invalid option
```
So the only schema enforcement in the project runs neither in CI nor on the most common dev laptop.

**857 stars on a framework whose central setup script does not parse.** That is the finding.

---

## 5. EXTENSION CONTRACT: there isn't one — it's fork-or-copy-paste

Exhaustive grep for `custom/`, `lock.json`, `lockfile`, `registry`, `marketplace`, `plugin`,
`manifest`, `namespace`, `override`, `enabled` across all `.md`/`.sh`/`.yml` returns **zero**
architectural hits. Every match is incidental prose ("lockfiles" inside `security-review.md`,
"enabled" in feature sentences).

There is:
- **no `custom/` or `local/` directory**
- **no config file listing enabled modules** — no opt-in/opt-out, you get all 36 or none
- **no plugin protocol, no manifest**
- **no namespacing** — `~/.hermes/skills/` is one flat pile; a user skill and a shipped skill
  are byte-indistinguishable and collide on filename

The documented extension path is the `create-skill` meta-skill, `skills/create-skill.md:63-65`:
```
Write to:
1. `~/.hermes/skills/[skill-name].md`      — immediate load
2. `[oh-my-hermes-repo]/skills/[skill-name].md` — repo persistence (if accessible)
```
**That is the fork model, stated in plain text.** "Repo persistence" means: edit your clone of
someone else's repository and hope. There is no upstream contribution path, no overlay, no
precedence rule.

The author *knows* this. `docs/improvements-to-hermes.md` §10 requests, from the host:
> **Per-project skill scoping** — Allow skills to be scoped to a specific project via AGENTS.md…
> As the Skills Hub grows, Hermes will have many skills loaded globally. Per-project scoping
> would prevent skill name collisions and reduce noise in skill matching.
> **Classification:** Hermes core — requires changes to the skill discovery mechanism.

That is the framework author documenting that his own framework has no scoping and cannot
build it, because the host doesn't expose the seam.

---

## 6. UPDATE: `git pull && bash install.sh` — silently clobbers user edits

`docs/installation.md:119-122`:
```bash
cd /path/to/oh-my-hermes && git pull && bash install.sh
```
`install.sh` is an unconditional `cp`. **There is no lockfile, no version comparison, no
checksum, no conflict detection, no backup, no `.bak`, no prompt.** Any edit a user made to
`~/.hermes/skills/health-check.md` is destroyed on the next update with no warning and no
recovery path.

The docs actively obscure this. `docs/installation.md:34` says "It is idempotent — running
again updates existing files," and `:124` reassures "Project `AGENTS.md` and `.env.local` are
never modified by the installer." Both true — and both about *project* files, silently changing
the subject away from the `~/.hermes/` files that **are** overwritten. `bootstrap.sh` genuinely
skips existing project files (`:22`, `:87`, `:133` all guard with `[ ! -f ]`). `install.sh` does not.

**Wasted signal:** every skill carries `version: 1.0.0`/`2.0.0` frontmatter, and
`validate-skills.sh:19` *requires* the field. Nothing anywhere reads it. `grep -rn version scripts/*.sh`
returns only `hermes --version` calls and the literal in the required-fields array. Per-asset
semver was authored and then never wired to an upgrade decision — the exact metadata a lockfile
would need, sitting unused.

**Version identity is incoherent three ways:** `VERSION.md` says `2.0.0`; the only git release
tag is `v0.0.1`; individual skills claim `1.0.0`–`3.0.0`. And `README.md:6` renders a
`license-MIT` badge linking to `LICENSE` — **a file that does not exist**, in a repo GitHub
reports as `licenseInfo: null`. The badge is false. For anyone in a company, an unlicensed
repo is legally un-adoptable.

---

## 7. REGISTRY: none. Discovery is a README table.

No index, no marketplace, no manifest feed, no search, no `omh install <skill>`, no third-party
namespace, no versioned bundles. The 36 skills are enumerated as a markdown table at
`README.md:117-154`. There is no mechanism by which a third party publishes anything.

The **only** discovery affordance is the copy-paste block at `README.md:19-32` — a prompt you
paste into another coding agent telling it to clone and run the installer. Distribution *is* the README.

Notably, the host has a registry (`hermes skills search`, "Skills Hub", referenced at
`docs/installation.md:20` and `skills/product-marketing.md:50`) and oh-my-hermes **does not
publish to it**. `docs/improvements-to-hermes.md` §1 instead asks the Hermes team to build
official app-building skills — i.e. asks to be made redundant.

---

## 8. UNINSTALL: cleanest part of the project, and still leaks

`scripts/uninstall.sh` (85 lines) is a **hardcoded name manifest** — arrays of 36 skills,
6 workflows, 7 agents, 13 scripts, 7 profile dirs (`:18-36`) — with `read -p` confirm at `:44`,
then deletes only exact known filenames.

This is the right instinct: **it deletes only what it shipped, never globs.** A user's own
`~/.hermes/skills/my-thing.md` survives. It correctly does not touch `~/.hermes/memory/`,
gateway config, or Hermes itself. I verified the manifest is currently accurate — 36 entries,
and `comm` against the on-disk skill list shows **zero drift**.

**But it is residue-leaving, and it knows it:**
- Cron jobs are **not removed**. `:83-85` just prints "To clean up cron jobs: `hermes cron list` /
  `hermes cron remove [id]`" — manual homework. `setup-cto.sh` created 4–5 named
  `oh-my-hermes-*` jobs; they keep firing after uninstall, invoking skills that no longer exist.
- `hermes memory` keys `github-repo`/`github-username` (`setup-cto.sh:307,316`) — left.
- `~/.hermes/.env` (`setup-integrations.sh`) — left, with live API keys.
- `~/.hermes/projects/*.env` + `current` — left.
- `~/.hermes/oh-my-hermes/dead-letter/` — left.
- `~/.hermes-backups/*.tar.gz` — left.
- `~/.hermes/{skills,workflows,agents,scripts}/` empty dirs — left.
- Project-side `AGENTS.md`, `.env.example`, `src/app/api/health/route.ts`, the `.gitignore` line — left.
- `gh` auth state in `~/.config/gh/hosts.yml` — left.

**Root cause is structural:** because install writes no manifest, uninstall must *duplicate* the
inventory by hand. Adding one skill means editing hardcoded arrays in **three** places —
`uninstall.sh:18`, `test.sh:15`, `verify.sh:41-76`. That is why the leak list is long: only
`cp`-installed *files* are tracked; nothing that mutates host state is.

---

## 9. What is genuinely GOOD, and what is BAD

### Good — and I would steal these

1. **`install.sh:93-99` — refuse to report success after installing zero items.** Born from a
   real `curl | bash` failure. Every installer should have this.
2. **`scripts/run-cron-safe.sh` (63 lines) — the best file in the repo.** A dead-letter queue for
   agent jobs: run the command, discard output on success (`:40`), and only on failure write a
   `chmod 600` log with project/job/timestamp/exit_code plus **regex-redacted secrets**
   (`:32`, `:52` — `s/(token|secret|password|api[_-]?key)=([^\s]+)/\1=[REDACTED]/Ig`), then notify
   via `hermes send`. Non-obvious, operationally correct, and host-agnostic.
3. **`scripts/setup-integrations.sh` — correct secret handling.** `umask 077` at file scope (`:5`),
   `read -r -s` so keys never echo or hit argv/shell history (`:36`), atomic upsert via
   `mktemp` + `mv` (`:19-26`), `chmod 600`, single-quote escaping (`:23`), and just-in-time
   prompting (never asks for a key until the action needs it). This is better than most
   production tooling.
4. **`scripts/project.sh` — multi-project context as a primitive.** `~/.hermes/projects/<slug>.env`
   (chmod 600) plus a `current` pointer file, with real slug validation (`:20-27`) rejecting
   anything outside `[a-zA-Z0-9._-]`. Clean, tiny, composable — `status.sh` consumes it by
   `set -a; . env_file; set +a`. This is the one piece of genuine *architecture*.
5. **`scripts/reset-runtime.sh` — backup before destruction.** Refuses without `--yes` (`:19-23`),
   tars the whole home first (`:33`), then **moves rather than deletes** stale state into
   `reset-<ts>/` (`:41-45`), explicitly preserving config/skills/.env.
6. **`INSTALL_FOR_AGENTS.md` — the install doc written for an LLM to execute, not a human to read.**
   Numbered, copy-pasteable, with a "Common failures" section and an explicit side-effects
   warning before the destructive step. Paired with the `README.md:19-32` paste-into-your-agent
   block, this is a genuinely modern distribution insight: **the installer's user is another agent.**
7. **`description:` must start "Use when…"** and describe *triggering conditions only, never the
   workflow* (`create-skill.md:73`), machine-checked at `test.sh:57`. Correct understanding that
   the description field is a *matcher input*, not documentation.
8. **`docs/improvements-to-hermes.md`** — 10 proposals each classified *Hermes core / Hermes docs /
   Extension layer / Deferred*. Honest engineering about where a seam belongs. Rare and valuable.
9. **Uninstall by explicit manifest, never by glob.**

### Bad

1. **A committed merge conflict in the primary setup script, shipped on `main`, with the
   community fix ignored in PR #9 for five weeks.** Everything else is a footnote.
2. **CI that cannot fail.** Existence checks only; no `bash -n`, no shellcheck; the one real
   linter (`validate-skills.sh`) isn't wired to CI *and* crashes on macOS bash 3.2.
3. **No extension contract.** Fork-or-copy-paste, stated outright in `create-skill.md:65`.
4. **No lockfile, no provenance, no update safety.** `cp` over the user's edits, silently.
5. **Inventory duplicated in 3 hardcoded arrays** (`uninstall.sh`, `test.sh`, `verify.sh`) because
   install writes no manifest. Guarantees future drift.
6. **`version:` frontmatter on every asset that nothing reads.** The lockfile that wasn't built.
7. **Uninstall leaves live cron jobs and live API keys** and calls itself clean.
8. **False MIT badge over a repo with no LICENSE file.** Blocks any commercial adoption.
9. **Schema inconsistency inside the shipped content** — 2 of 6 workflows lack frontmatter;
   agents use a different schema; neither is validated.
10. **Everything is global.** No per-project scoping, no enable/disable — you take all 36 skills
    into every project's matcher, or none.
11. **`main` is 2 months stale while work sits unmerged on `develop`** (whose last two CI runs
    both failed). Users install the stale broken branch.
12. **Deep opinionation hardcoded into a "framework"** — Vercel + Supabase + Sentry + Telegram +
    Buffer + Next.js are baked into skills and templates with no abstraction. Fine for a
    *starter kit*; fatal for something claiming the oh-my-* extensibility mantle.

### The strategic read for oh-my-musecode

The sweep framed this repo as evidence that "name recognition drives adoption more than release
maturity." **The evidence is stronger and more double-edged than that.**

Confirmed: 857 stars on a v0.0.1 whose flagship script does not parse, with 1 issue and
3 contributors. Name recognition absolutely converts to stars.

But: **five repos are named `oh-my-hermes`, and this one is second.** `rlaope/oh-my-hermes` has
1299 stars and shipped code *today*; this one's `main` hasn't moved since July. The single issue
ever filed is someone asking how it differs from a *different* oh-my-* project. So the honest
lesson is:

> **The "oh-my-" name buys a star spike and nothing else. It is not defensible — it invites
> immediate collision, and the winner is decided by who keeps shipping.** Claim `oh-my-musecode`
> early *and* loudly, yes — but treat the name as a starting gun, not a moat. Ship a working
> installer and a real extension contract, because that is what the second-place repo lacked.

---

## 10. Transfer to Muse Code (compiled Rust binary host)

### Transfers directly — the whole delivery model is host-agnostic

The core insight holds perfectly: **oh-my-hermes never executes inside its host.** It writes
markdown and JSON into a config tree and exits. The host — a Python daemon here, a Rust binary
for us — reads that tree on its own schedule. That is *exactly* our situation and the reason
this repo is a better reference than any oh-my-zsh descendant. Nothing here depends on being
sourced into a shell, on Python, or on Node.

Concretely portable:
- **Assets-as-markdown-with-frontmatter.** `name`/`description`/`version`/`tags` maps cleanly onto
  Muse's skills and agent-definitions subsystems.
- **"Use when…" descriptions as matcher input, machine-linted.** Adopt verbatim, and make it a
  hard CI gate rather than an advisory one.
- **`install.sh:93-99` zero-install guard.** Port as-is.
- **`run-cron-safe.sh`'s dead-letter + redaction pattern.** Port the *concept* into a Rust
  subcommand — it becomes far better as a compiled feature with structured logging than as sed regex.
- **`setup-integrations.sh`'s just-in-time secret prompting** (`umask 077`, no-echo read, atomic
  `mktemp`+`mv`, 0600). Muse already has `~/.config/muse/auth.json`; this is the right UX to wrap it.
- **`project.sh`'s per-project env + `current` pointer.** Directly relevant to the per-project
  scoping the Hermes author had to beg for.
- **`INSTALL_FOR_AGENTS.md`.** Ship one. Muse Code's own users are agents.
- **Uninstall-by-manifest, never by glob.**
- **`docs/improvements-to-hermes.md`'s core/docs/extension/deferred triage.** Keep this file for
  Muse Code; it is how you stay a good citizen of a host you don't control.

### Does NOT transfer

- **Bash as the distribution runtime.** 1524 lines of shell with a bash-4 dependency that breaks
  on macOS, and a class of bug (`bash -n` failure, `set -u` unbound vars) that a compiled binary
  makes structurally impossible. `oh-my-musecode` should be a **Rust binary or a `muse` subcommand**,
  not `install.sh`. The single highest-leverage lesson of this teardown is that the framework's
  worst defect is one the type system would have eaten.
- **Hardcoded Vercel/Supabase/Sentry/Telegram/Buffer/Next.js opinions.** Starter-kit content, not
  framework architecture.
- **Host-CLI shelling** (`hermes profile create`, `hermes cron add`, `hermes memory set`) with
  its comedy of compatibility probes — `setup-cto.sh:22-28` greps `--help` output to
  feature-detect, tries `cron create` then falls back to `cron add` (`:51-59`), tries
  `profile create` then `profile new` (`:92-96`). Muse's **MSP stdio host (`muse serve`)** is the
  supported seam; use a protocol, never `--help` scraping.
- **The flat `cp` install and `git pull && install.sh` update.** Replace outright.

### What Muse Code hands us for free — and this repo proves is essential

Read `docs/improvements-to-hermes.md` as a **requirements document written by someone who didn't
have these**, then check it against Muse's confirmed surface:

| oh-my-hermes had to ask the host for… | Muse Code already ships |
|---|---|
| §10 per-project skill scoping / collision avoidance | `.muse-plugin/plugin.json` manifests + project-local config |
| (unasked, unbuilt) provenance & update safety | **`.muse/lock.json`** — provenance / quarantine / allowed_tools |
| (unasked, unbuilt) asset version pinning | **`.muse/skills.lock`** |
| §3 deployment lifecycle events / event bus | **`.muse/hooks.json`** |
| §1 curated official registry | plugin manifests + `.claude-plugin`/`.codex-plugin` ingestion |
| §9 MCP integration patterns | MSP stdio host via `muse serve` |

**Every structural failure in this teardown — no lockfile, no scoping, no provenance, no events,
no registry — is a hole Muse Code has already filled.** oh-my-hermes flat-`cp`s markdown and
loses user edits *because Hermes gave it nowhere else to go*. We have somewhere to go.

So the design directive is: **do not reimplement oh-my-hermes's file copying on top of a host that
already has a lockfile.** Write `.muse/skills.lock` and `.muse/lock.json` as the install record;
make uninstall read that record instead of a hand-maintained array; use `.muse/hooks.json` for the
event bus Hermes never got; and expose an overlay + precedence rule (`custom/` beats shipped) so
the extension contract is not "edit your clone."

And because the loader also reads `.claude-plugin` and `.codex-plugin`: **ship the pack in a
manifest format all three ingest.** That is a distribution advantage oh-my-hermes never had —
one artifact, three hosts — and it is worth far more than the name.

---

## Verification

**Independent adversarial re-verification, 2026-09-01.** Fresh clone + fresh API pulls in
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-salomondiei08-oh-my-hermes/`
(repo at `.../verify-salomondiei08-oh-my-hermes/repo`, sandbox install at `.../fakehome/.hermes`).
Nothing below was taken from the teardown above; every number was re-derived.

**Verdict: MOSTLY_SOLID.** The repo is real, every load-bearing structural claim reproduces, and
the headline finding is not only confirmed but *understated*. Four small factual errors and one
refuted sub-claim, listed at the end. No fabrication anywhere.

### 1. Existence and traction — CONFIRMED, exact

`gh repo view Salomondiei08/oh-my-hermes --json ...`, re-run 2026-09-01:

| Field | Report | Verified |
|---|---|---|
| stars | 857 | **857** |
| forks | 81 | **81** |
| createdAt | 2026-05-08 | **2026-05-08T16:20:54Z** |
| pushedAt | 2026-07-28 | **2026-07-28T05:38:06Z** |
| licenseInfo | null | **null** |
| releases | 1 (`v0.0.1`, 2026-06-19) | **1, `v0.0.1`, 2026-06-19T07:02:50Z** |
| diskUsage | 2038 KB | **2038 KB** |
| issues, all-time | 1 (#8) | **1** — `#8 "How close is this to Oh-My-Openagent (OMO)?"`, still OPEN |
| commits on main | 47 | **47** (`Link: ...page=47; rel="last"` at `per_page=1`) |
| contributors | 39 / 6 / 1 | **Salomondiei08 39, jaythehardcoder 6, zbqshen 1** |

The `pushedAt`-vs-`main` nuance is real and correctly called: `main` = `7ce4e3c`,
committed **2026-07-03T17:02:16Z** (= 2026-07-04 in the author's +0900 tz — the report's date is the
local rendering, not an error). `develop` = `1356c37`, 2026-07-28T05:37:26Z. `main...develop`
compares **diverged, 19 ahead / 20 behind, 78 files** — so `develop` is not a fast-forward and the
stale-main story holds.

### 2. Shipped-content counts — CONFIRMED, every single number

Re-ran `find`/`ls`/`wc` on my own clone. All eleven counts match to the digit:

```
skills/*.md      36    cat skills/*.md    | wc -wl → 2601 lines / 13372 words
agents/*.md       7    cat agents/*.md    | wc -w  → 2402 words
workflows/*.md    6    cat workflows/*.md | wc -w  → 1740 words
scripts/*.sh     13    cat scripts/*.sh   | wc -l  → 1524 lines
docs/*.md         8    cat docs/*.md      | wc -w  → 6127 words
templates/        4    (AGENTS.md.template, .env.example, healthcheck/{express-health.js,nextjs-health-route.ts})
.github/workflows 2    (test.yml, release.yml)
non-.git files   85
banner.png     1400 KB of 2038 KB disk
```

The two frontmatter-less workflows are exactly the two named: `deploy-and-monitor.md` and
`ship-this-idea.md` both begin `# Workflow: ...` with no `---`.

The five ZERO-claims hold under exhaustive grep: no `commands/`, no hooks (zero occurrences),
no `mcpServers`/`.mcp.json`, no `settings.json`/statusline/output-styles, **no `.json` file of any
kind in the repo** (`find . -name '*.json' -not -path './.git/*'` → empty), therefore no plugin
manifests.

### 3. Extension contract — CONFIRMED as NONE, and the code path was checked

The teardown's central architectural claim is the one most likely to be an aspiration read off a
README. It is not. My own exhaustive grep across every `.md`/`.sh`/`.yml`/`.json` for
`lock.json|lockfile|registry|marketplace|plugin|manifest|namespace|precedence|custom/|enabled_modules`
returns **exactly one hit**, and it is about the *host*:

```
skills/product-marketing.md:50: search the current Hermes registry for the specific capability:
```

No `custom/` directory exists. The documented extension path is verbatim as quoted, at
`skills/create-skill.md:64-65`:

```
1. `~/.hermes/skills/[skill-name].md` — immediate load
2. `[oh-my-hermes-repo]/skills/[skill-name].md` — repo persistence (if accessible)
```

i.e. edit your clone of someone else's repo. `docs/improvements-to-hermes.md:103-109` confirms the
author knows: **"## 10. Per-project skill scoping"**, classified
**"Hermes core — requires changes to the skill discovery mechanism."** The 4-way triage
(`Hermes core` / `Hermes docs` / `Extension layer` / `Deferred`) is real and is stated at lines 6-9.

### 4. Install / update — CONFIRMED, and I reproduced the clobbering empirically

`install.sh` read end to end (**133 lines**, not 132 — see corrections). Every cited line is right:
`:30` hard preflight on `$HERMES_DIR`; `HERMES_HOME` override at `:5`; `:42-62` self-clone to
`mktemp -d` with `trap cleanup EXIT` at `:24`; `--depth 1` from the default branch (`main`) at `:54`;
the zero-install guard at **`:93-99`** verbatim (`"[ERROR] Refusing to report success after
installing zero items."`); and the duplicate-loop bug is exactly as described — `WORKFLOWS_INSTALLED`
assigned at `:90`, gated at `:93`, then **reset to 0 at `:111` and recomputed by a redundant loop at
`:112-118`**.

I then ran the installer twice against a sandbox `HERMES_HOME`:

```
$ echo "MY LOCAL EDIT — DO NOT LOSE" >> $HERMES_HOME/skills/health-check.md
$ bash install.sh          # the documented update step
health-check.md md5 before=3ef7cd71... after=806f0c9b...
edit survived?            NO — CLOBBERED SILENTLY
user's own new skill?     YES (survives; flat pile, collides only on filename)
any .bak/backup created?  none
```

No prompt, no diff, no backup, no lockfile, no version check — confirmed by execution, not by
reading. The doc-obscuring claim is also exact: `docs/installation.md:34` says
*"It is idempotent — running again updates existing files"*; `:125` says *"Project `AGENTS.md` and
`.env.local` are never modified by the installer"* — both about project files, while `~/.hermes/`
is what gets destroyed. `bootstrap.sh` genuinely does guard with `[ ! -f ]` (project `AGENTS.md`,
`.env.example`, health route) and appends `.env.local` to `.gitignore` only when absent.

The wasted-`version:` finding holds: `validate-skills.sh:19` is
`REQUIRED_FIELDS=(name description version tags)`, and `grep -rn version scripts/*.sh` returns only
`hermes --version` calls and an unrelated `npm_package_version` in a generated health route.
Nothing reads `version:` for an upgrade decision.

### 5. The headline (`setup-cto.sh`) — CONFIRMED, and worse than reported

```
$ for f in install.sh scripts/*.sh; do bash -n $f; done
scripts/setup-cto.sh   FAIL: line 380: syntax error near unexpected token `>>'
                             line 380: `>>>>>>> 9e3e68f (feat: add first-run server operating layer)'
all 13 others          OK
$ grep -rn '^\(<<<<<<<\|>>>>>>>\)' --include='*.sh' --include='*.md' --include='*.yml' .
scripts/setup-cto.sh:380:>>>>>>> 9e3e68f (feat: add first-run server operating layer)
```

Confirmed on main at HEAD, exactly one marker, exactly that line, exactly one broken script.
`INSTALL_FOR_AGENTS.md` Step 4 is indeed `bash /tmp/oh-my-hermes/scripts/setup-cto.sh`, carrying its
own warning *"This step has side effects: it creates Hermes profiles, touches GitHub auth, writes
Hermes memory, and schedules cron jobs."* The unbound-var pair is confirmed at the exact lines:
`:358 [ -n "$PRODUCTION_URL" ]` and `:369 [ -n "$GITHUB_REPO" ]`, with `set -euo pipefail` at `:6`
and **no assignment or `.env` sourcing anywhere in the file** for either variable.

PR #9 is real and still open: `docxology`, branch `fix/setup-cto-unbound-vars-and-merge-conflict`,
title *"fix: resolve merge conflict and unbound variable crashes in setup-cto.sh"*, created
2026-07-27T18:01:04Z, **zero comments**, no maintainer response in 5 weeks. The prior-art commit is
verbatim: `fe296a4 fix: resolve install.sh merge artifacts (syntax error at line 51)`, 2026-07-01 —
the neighbouring file, same bug class, never re-checked.

CI genuinely cannot catch it: `test.yml` runs only `bash install.sh` then `bash scripts/test.sh`
with `TEST_MODE=1`; `grep -rn 'shellcheck\|bash -n' .github/ scripts/` → nothing; `test.sh` is
`[ -f ]` existence checks (`:15-50`) plus a `grep -q 'Use when'` on `description:` at **`:57`**.
`install.sh` only `install -m 700`-copies `setup-cto.sh` — it never parses it.

`validate-skills.sh` reproduces broken on macOS. Running it on this machine (GNU bash 3.2.57):

```
scripts/validate-skills.sh: line 31: declare: -A: invalid option
```

**NEW — a third crash the teardown missed.** I stubbed `hermes`/`gh` and ran `setup-cto.sh` in a
sandboxed `HOME` to see what a real user actually hits. It never reaches line 380 or 358. It dies
much earlier, in **Section 2 "Creating Hermes profiles"**:

```
setup-cto.sh: line 181: CREATE_ARGS[@]: unbound variable
```

`:176 CREATE_ARGS=()` then `:181 "${CREATE_ARGS[@]}"` — an empty array expanded under `set -u`, which
is an error on bash ≤4.3 and fine on bash 4.4+. So this is a **second macOS-only bug**, same family
as the `declare -A` one, and it aborts the flagship script on the most common dev laptop *before*
the merge marker is ever parsed. The teardown's outcome claim ("the product does not work without
it") is therefore right, but its enumeration of failure modes is incomplete and its ordering is
wrong: on macOS the run dies at `:181` during profile creation; the `:380` marker is what a Linux
user would eventually hit, after `gh auth login`, memory writes and several crons have already
fired. Half-configured host state, then abort — worse than a clean early failure.

`develop` **already fixes the marker** (raw fetch of `develop/scripts/setup-cto.sh`: 374 lines,
zero markers, `bash -n` clean) — but still ships `"${CREATE_ARGS[@]}"` at `:135`. The fix exists,
on a diverged branch, unmerged, while `install.sh --depth 1` keeps handing users `main`.

### 6. Uninstall — CONFIRMED, including the zero-drift check

`scripts/uninstall.sh` is **85 lines**, hardcoded arrays at `:18-36`, `read -r -p` confirm at `:44`,
exact-filename `rm` with no globbing, and the cron residue is verbatim at `:82-85`:

```
echo "Hermes itself is untouched. Memory, gateway, and cron jobs are intact."
echo "To clean up cron jobs:"
echo "  hermes cron list"
echo "  hermes cron remove [id]"
```

I re-ran the drift check independently (`comm` of the array against `ls skills/*.md`):
**36 vs 36, zero difference.** The triplicate-inventory claim is correct and the line numbers are
exact: `uninstall.sh:18` (array), `test.sh:15-24` (inline `for` list), `verify.sh:41-76`
(36 hand-written `check` lines).

### 7. Strengths — spot-checked, all real, line numbers near-exact

- `run-cron-safe.sh` — **63 lines** as claimed. Discards output on success `:39-42`; on failure
  writes project/job/timestamp/exit_code + regex-redacted command `:32` and redacted output `:52`;
  `chmod 600` at `:55`; notifies via `hermes send` at `:59`. Real, non-obvious, host-agnostic.
- `setup-integrations.sh` — `umask 077` at `:5`, `read -r -s` at `:36`, atomic `mktemp`+`mv` upsert
  `:19-26`, `chmod 600` `:27`, single-quote escaping `:23`, just-in-time prompt guard `:32-35`.
  All confirmed.
- `project.sh` — `validate_slug` rejecting anything outside `[a-zA-Z0-9._-]` at **`:20-27`** exactly;
  `chmod 600` on the per-project env and the `current` pointer write at `:51-53`.
- `reset-runtime.sh` — refuses without `--yes` at **`:19-23`**, tars at **`:33`**, `mv`s rather than
  deletes into `reset-<ts>/` at **`:41-45`**. All three exact.
- `create-skill.md:73` — *"It MUST start "Use when..." and describe ONLY triggering conditions —
  never the skill's workflow."* Exact line, machine-checked at `test.sh:57`. Confirmed.
- `README.md:19-32` paste-into-your-agent block and `README.md:117-154` skills table: both exact.

### 8. Name thesis — CONFIRMED, and the collision is far worse than reported

`gh search repos "oh-my-hermes" --limit 40`, filtered to **exact** repo name `oh-my-hermes`:

```
1299  rlaope/oh-my-hermes        (created 2026-06-03, pushed 2026-09-01T11:35Z, MIT)
 857  Salomondiei08/oh-my-hermes (created 2026-05-08, main last touched 2026-07-03, no license)
 302  witt3rd/oh-my-hermes
   9  HERMESquant/oh-my-hermes
   2  shuangxipop1/oh-my-hermes
   2  codenote-net/oh-my-hermes
   1  tconn93/oh-my-hermes
   + 8 more at 0 stars
exact-name count: 15
```

The thesis holds and then some: this repo is **second**, it was created **first** (2026-05-08 vs
rlaope's 2026-06-03), the winner shipped **today** while this `main` has not moved since 2026-07-03,
and the only issue ever filed is someone confusing it with a different oh-my-\* project. The
"starting gun, not a moat" conclusion is if anything strengthened — the report said five namesakes;
there are fifteen.

### 9. Refuted / corrected

**REFUTED (1):**

- *"main is 2 months stale while work sits unmerged on develop (whose last two CI runs both
  FAILED)"* — the **CI half is wrong**. `gh run list --branch develop` returns, newest first:
  `success 2026-07-28T05:38:08Z (1356c37)`, `success 2026-07-28T05:29:44Z`, `failure 05:23:25Z`,
  `failure 05:16:15Z`. The last two runs on `develop` both **succeeded**; the two before them
  failed. The stale-main half is correct (diverged 19/20).

**CORRECTED (5 minor factual slips, none load-bearing):**

1. **PR count is 9, not 10.** `gh pr list --state all` → 9 total, and the breakdown given
   (4 merged / 3 closed / 2 open) sums to 9 and is itself correct. The "10" is the highest PR
   *number*; #8 is an issue, sharing the same sequence.
2. **`install.sh` is 133 lines**, not 132.
3. **Skill `version:` values span 1.0.0–2.0.0, not 1.0.0–3.0.0.** Distribution: 1.0.0 ×24,
   1.1.0 ×3, 2.0.0 ×10 (37 matches over 36 files because `create-skill.md` contains a second
   `version:` inside its template body). The lone `3.0.0` is on `workflows/cto-loop.md:4`, not a
   skill. The three-way version incoherence (VERSION.md 2.0.0 / only tag v0.0.1 / per-asset semver)
   is otherwise confirmed exactly.
4. **"Skills Hub" is not in `docs/installation.md:20`.** `docs/installation.md:19` has
   `hermes skills search --help`; the phrase "Skills Hub" appears only in
   `docs/improvements-to-hermes.md` (`:15`, `:17`, `:19`, `:49`, `:107`). The substance — the host
   has a registry and oh-my-hermes does not publish to it — is confirmed.
5. Two off-by-ones: `docs/installation.md:125` (not `:124`) for *"never modified"*, and
   `project.sh:51-53` (not `:48-52`) for the chmod/current-pointer pair.

**Net:** one wrong CI observation and five citation slips against a teardown whose ~60 other
file:line citations I checked and found exact, whose eleven content counts all reproduce to the
digit, whose traction numbers match the API exactly, and whose headline defect I reproduced by
execution — plus one additional crash (`setup-cto.sh:181`) the teardown missed that makes its own
argument stronger. The strategic conclusion (do not port the `cp` installer; write
`.muse/skills.lock` + `.muse/lock.json` as the install record; uninstall by manifest, not by
hand-maintained array; ship one manifest that `.muse-plugin`/`.claude-plugin`/`.codex-plugin` all
ingest) rests on findings that all survive verification.

