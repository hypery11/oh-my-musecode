# Teardown: haoruilee/oh-my-mcode

**Verdict: CONFIRMED — real repo, real code, cloned and executed locally.**

- URL: https://github.com/haoruilee/oh-my-mcode
- Clone: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/haoruilee-oh-my-mcode/`
- Target host: **MiniMax Code CLI** = `mcode` = npm `@minimax-ai/code` (registry latest **0.2.7**, 15 versions, modified 2026-08-28 — verified via `registry.npmjs.org`)
- License MIT, TypeScript, Node >= 22.
- I ran `npm ci`, `tsc`, `npm test` (**142/142 pass**), `doctor --package-only` (**PASS**), and a real sandboxed `install --skip-host --yes` against a fake `MINIMAX_HOME`.

This is NOT a prompt pack that happens to be called "oh-my-*". It is an **8,130-LOC TypeScript
orchestrator** that shells out to a third-party agent binary, with a thin (556-line) Skill layer
on top. That asymmetry is the single most important fact about it.

---

## 1. Does it exist?

Yes. CONFIRMED. Repo id `1339894168`. 147 tracked files, 6.0 MB working tree.

---

## 2. What does it SHIP? (real counts)

| Category | Count | Path evidence |
| --- | --- | --- |
| **Skills** (host-visible) | **10** `SKILL.md` | `skills/{max,plan,verify,resume,doctor,review,ship,research,team,interview}/SKILL.md` — 556 lines total, avg 56 lines each |
| Skill reference docs | 1 | `skills/max/references/run-store.md` |
| **Subagents / "agents"** | **5** markdown **role contracts** | `agents/{explorer,planner,builder,verifier,release}.md` — only **98 lines total**. Explicitly NOT registered host agents (see `docs/host-reality.md`) |
| **Slash commands** | **0** | `docs/host-reality.md`: "Commands / slash commands" listed as *not* a public plugin capability |
| **Hooks** | **0** | same; `src/inspect.ts:312` `we_do_not_send: ["hooks", …]` |
| **MCP servers** | **1** stdio, **7 tools** | `mcp/server.mjs` (220 lines, zero deps): `omm_run_create`, `omm_run_show`, `omm_run_list`, `omm_status`, `omm_verify`, `omm_interview`, `omm_inspect` |
| **Workflow definitions** | **8** YAML | `workflows/{max,plan,verify,review,ship,research,team,interview}.yaml`, parsed by `src/workflows.ts` (its own 130-line mini-YAML parser) |
| **JSON schemas** | **6** | `schemas/{evidence,finding,planner-output,run-event,task-contract,worker-yield}.schema.json` |
| **Settings presets / statusline / output styles** | **0** | none exist |
| **CLI commands** | **15** | `src/cli.ts` HELP: max plan verify resume review ship research attach status cancel inspect team interview doctor install |
| **TypeScript source** | **34 files / 8,130 LOC** | `src/*.ts` |
| **Tests** | **15 files / 4,660 LOC / 142 assertions** | `test/*.test.mjs`, all hermetic (fake host at `test/fixtures/fake-mcode.mjs`) |
| **Eval fixtures** | **4** + checked-in baseline | `evals/tasks/{pass,fail-then-repair,plan-only,follow-goal}`, `evals/baselines/report.json` |
| **Standalone no-build store** | 904 LOC | `scripts/run-store.mjs` — a *second implementation* of the run store in plain JS |
| Docs | 8 files / 915 lines | `docs/{architecture,host-reality,harness,roadmap,marketplace,design-check}.md` + 2 zh-CN |
| Manifests | 3 | `plugin.json` (portable "Agent Plugins 1.0"), `.minimax-plugin/plugin.json` (MiniMax native, schemaVersion 1), `mcp.json` |

**Content-to-machinery ratio: 654 lines of markdown assets vs 8,130 lines of TypeScript.**
This is a *harness that ships some skills*, not a *skill pack*. That inverts the oh-my-zsh model
entirely and is the thing to decide about before copying it.

### The dual-manifest trick (worth stealing)

It ships **two** plugin manifests at once for the same content:

- `plugin.json` — `$schema: https://agent-plugins.org/schemas/1.0.0/plugin.schema.json`, vendor-neutral.
- `.minimax-plugin/plugin.json` — schemaVersion 1, MiniMax-specific: `displayName`, `displayName_zhHans`, `category`, `exampleQueries[6]`, `apps: []`, `mcpServers: ["mcp.json"]`, `skills: [10 explicit paths]`.

`src/util.ts:15-27` `packageRoot()` walks up from `import.meta.url` (never `process.cwd()`)
requiring **both** files to be present as the root marker. `src/install.ts:122` refuses to install
if either manifest is missing. Directly relevant to Muse Code, which reads `.muse-plugin/`,
`.claude-plugin/` *and* `.codex-plugin/` — one directory tree, N manifests, one install.

---

## 3. INSTALL — what exactly lands on the machine

Three entry paths, all converging on `src/install.ts`:

1. `npx oh-my-mcode install --yes` (README's TL;DR — **but the package is not on npm; see §8**)
2. `npx github:haoruilee/oh-my-mcode install --yes` (the actually-working one-liner)
3. `scripts/install.sh` / `scripts/install.ps1` (rsync/tar/Copy-Item equivalents, no Node)

### `install()` control flow (`src/install.ts:165-249`), read end to end

```
presentBefore = mcodeExists()          # which("mcode") || $OMM_MCODE
if !presentBefore && !skipHost:
    if !shouldAttemptHostInstall(...): # non-TTY stdin without --yes
        log "stdin is not a TTY; skipping host install (plugin-only)"
        → installPlugin(); return
    if !yes: promptYesNo("Install official @minimax-ai/code and the oh-my-mcode plugin?")
        declined → installPlugin(); return
    spawnSync("npm", ["install","-g","@minimax-ai/code"])
    on success → prependNpmGlobalBin()  # `npm prefix -g` + /bin prepended to PATH
                 recheck mcodeExists()
    on failure → record host_error, KEEP GOING (plugin still drops)
installPlugin()                         # always runs
```

`installPlugin()` (`src/install.ts:120-158`):

```ts
const dest = path.join(MINIMAX_HOME||~/.minimax, "plugins", "oh-my-mcode");
if (existsSync(dest)) rmSync(dest, {recursive:true, force:true});   // ← nukes prior install
cpSync(root, dest, {recursive:true, dereference:true,
  filter: rel => top !== ".git" && top !== "node_modules" && top !== ".minimax"});
if (lstatSync(dest).isSymbolicLink()) throw CliError("refused to install a symlink as the plugin root");
```

### Measured footprint (I actually ran this)

```
$MINIMAX_HOME/plugins/oh-my-mcode/   →  250 files, 6.0 MB
```

and the copied top level is **the whole git checkout**:

```
.github/  .gitignore  .minimax-plugin/  agents/  AGENTS.md  bin/  CLAUDE.md  dist/
docs/  evals/  examples/  icon.png  LICENSE  mcp/  mcp.json  package-lock.json
package.json  plugin.json  README.md  README.zh-CN.md  schemas/  scripts/  skills/
src/  test/  tsconfig.json  workflows/
```

**That is a real flaw.** `package.json` has a correct `files:` allowlist for npm, but
`installPlugin` ignores it and uses a 3-entry denylist instead. Installing from a git clone
therefore ships `test/` (33 files), `.github/workflows/hermetic.yml`, `tsconfig.json`,
`package-lock.json`, and the contributor `AGENTS.md`/`CLAUDE.md` into the host's plugin
directory. Only `dist/`, `skills/`, `agents/`, `schemas/`, `workflows/`, `mcp/`, `scripts/`,
`bin/` and the manifests are load-bearing.

### Other things it writes

- `<workspace>/.minimax/runs/<run_id>/` — the run store, created lazily at first `max`/`plan`:
  `run.json`, `plan.md`, `tasks.json`, `events.jsonl`, `evidence/index.json`, `.lock`,
  later `findings.json`, `summary.md`, `file-hashes.json`. Atomic temp+rename (`writeAtomic`).
- `<workspace>/.minimax/worktrees/` — only with `--worktree`.
- `~/.minimax/oh-my-mcode.json` and `<workspace>/.minimax/oh-my-mcode.json` — config; **never
  created by the installer**, user-authored only.
- Global npm bin: `mcode` (only if missing and consented).
- **Does NOT touch** the project's `AGENTS.md` — `src/install.ts:154-155` logs this explicitly.
  `examples/AGENTS.max-mode.md` is a template the user copies by hand.

### Install honesty properties I verified in code + tests

- `npmGlobalHostInstaller()` **refuses to run in CI** (`process.env.CI==="true" || OMM_HERMETIC==="1"`), returns an error object rather than hitting the registry (`src/install.ts:96-102`).
- Non-TTY stdin without `--yes` degrades to plugin-only rather than silently installing a global npm package (`shouldAttemptHostInstall`, tested at `test/security.test.mjs:451`, `test/install.test.mjs:110`).
- A failed host install is reported, the plugin still drops, and the **CLI exits 2** (`src/cli.ts:296`, `test/install.test.mjs:163`). Partial success is not laundered into exit 0.
- "Never curl a script. Never install MiniMax desktop." — comment at `src/install.ts:93`, and the README/`host-reality.md` name-check the competitors that do (`curl omp.sh/install`, `npm i -g omo-ai@beta`) to say *we are not that*.

---

## 4. EXTENSION CONTRACT — how a user adds/overrides without forking

**This is the weakest part of the project.** There is no `custom/`, no enabled-modules list,
no plugin protocol, no `skills.d/`. What actually exists:

| Mechanism | Surface | File |
| --- | --- | --- |
| Config file (2 layers) | Exactly **4 knobs**: `permission`, `maxRepairs`, `llmVerify`, `team{concurrency,worktree}` | `src/config.ts:11-23`; `~/.minimax/oh-my-mcode.json` then `<ws>/.minimax/oh-my-mcode.json`, then CLI flags |
| Privilege ceiling on the workspace layer | A repo-local config **cannot** raise `permission` to `"full"`; only the home file can | `src/config.ts:46-52`, tested `test/security.test.mjs:396` |
| Env overrides | `OMM_MCODE`, `OMM_WORKSPACE`, `OMM_PACKAGE_ROOT`, `OMM_RUN_ID`, `OMM_HOST_OUTPUT_SCHEMA`, `OMM_HERMETIC`, `OMM_MCODE_STUB`, `MINIMAX_HOME` | `grep OMM_ src/` |
| Opt-in project template | Copy `examples/AGENTS.max-mode.md` → your repo's `AGENTS.md` | manual, documented |
| Workflow YAML | `workflows/*.yaml` is read from `packageRoot()` at runtime, so editing the installed copy *does* change behavior (`stop_after`, phase list) | `src/workflows.ts:81-126` |

**Everything else — the 10 skills, 5 role contracts, 6 schemas, all prompts (`src/prompts.ts`) —
is fork-or-edit-in-place.** And editing in place is unsafe, because §5.

There is no third-party extension story at all. You cannot write `oh-my-mcode-plugin-foo` and have
it discovered. The design deliberately says so: `docs/roadmap.md` "What we will not do" and
`src/inspect.ts:185` "We do not register custom plugin agents."

Being fair: this is partly the *host's* fault. MiniMax's public plugin surface is Skills + MCP only.
But the project also made no attempt at a userland overlay it could have built itself
(e.g. merging `~/.minimax/oh-my-mcode/skills/*` over the packaged ones).

---

## 5. UPDATE — and the clobbering problem

**There is no update command.** `oh-my-mcode --help` lists 15 commands; `update`/`upgrade`/`sync`
is not among them. `grep -rniE "uninstall|upgrade|update" src/ scripts/` returns only
`hash.update()` and `store.ts` comments.

Updating = re-running install, which is:

```ts
if (existsSync(dest)) rmSync(dest, { recursive: true, force: true });
cpSync(root, dest, …);
```

**Destroy-and-recopy. There is no lockfile, no installed-file manifest, no checksum, no backup,
no diff, no merge, no `--force` gate, and no warning.** Any edit a user makes under
`~/.minimax/plugins/oh-my-mcode/` — a tweaked `SKILL.md` trigger phrase, a modified
`workflows/max.yaml` `max_repairs`, an added role contract — is silently deleted on the next
`install`. Since editing those files is the *only* way to customize (§4), the extension story and
the update story actively contradict each other.

Note the irony: this project is obsessive about content hashing **inside** the run store —
`src/hash.ts` sha256s evidence files and a stale hash **refuses Accept**
(`docs/architecture.md` "content-hash evidence; stale hash refuses Accept"). It applies zero of
that rigor to its own installed artifacts.

What *is* well-handled around versioning is the **host**, not itself: `src/host-version.ts` parses
`mcode --version` into `{major,minor,patch}`, derives `HostCapabilities`
(`structuredExec` ≥0.2.4, `outputSchemaDocumented`, `legacyOutputSchemaCrash` ==0.2.1), and
`doctor` / `inspect model-policy` print those flags. Unparsed version → **all capabilities false**,
never optimistic defaults. That is a genuinely good pattern.

---

## 6. REGISTRY — none of its own; honest about someone else's

`docs/marketplace.md` is titled **"Marketplace (not listed)"** and opens: *"This repo is **not** on
the official MiniMax marketplace."* It then documents, for a **human** (explicitly not CI):

- Official submit = ZIP or public GitHub repo whose root contains `.minimax-plugin/plugin.json`;
  fields it would submit (URL / branch `main` / directory = repo root); progress tracked via
  Feishu + `submission_id` + email.
- Warns that official checklists *"sometimes reject install scripts, secrets, and symlinks"* and
  that `scripts/install.*` should be omitted from a submission ZIP if a reviewer requires it.
- Points at a **community** registry — `hetaoBackend/MiniMax-Code-Plugins`, layout
  `plugins/<you>/<plugin>/` — and immediately says *"It is not the official MiniMax catalog…
  we are not opening a PR there."*
- Explicitly steers users **away** from `MiniMax-AI/skills`, which is a Claude/Cursor/Codex pack,
  not an mcode plugin registry.

So: **discovery is ad hoc (GitHub + npx-from-git). It publishes no index of third-party
additions and there is no mechanism for anyone to add one.** But it is the most honest
registry section I have read in this whole space — it documents the registry it is *not* in
rather than implying membership.

---

## 7. UNINSTALL — documented, manual, incomplete

README, verbatim:

```bash
npm unlink -g oh-my-mcode
rm -rf ~/.minimax/plugins/oh-my-mcode
```

No `uninstall` subcommand exists. Residue left behind after those two lines:

- `~/.minimax/oh-my-mcode.json` (if the user made one)
- every `<workspace>/.minimax/runs/**` in every project ever touched — 6 files + evidence per run
- `<workspace>/.minimax/oh-my-mcode.json`, `<workspace>/.minimax/worktrees/`
- the globally-installed `@minimax-ai/code` host, if `install` put it there — **the uninstall
  instructions never mention the thing the install may have installed**. That is an asymmetry
  worth fixing in any design that copies the bootstrap-the-host idea.

Mitigating: the plugin directory is a single self-contained folder, so `rm -rf` genuinely does
remove the plugin. Run stores are arguably user data and *should* survive. But an install that
can `npm i -g` a second product owes the user a symmetric teardown.

---

## 8. TRACTION — real numbers

Fetched 2026-09-01 via `gh api repos/haoruilee/oh-my-mcode`:

| Metric | Value |
| --- | --- |
| Stars | **51** |
| Forks | **0** |
| Watchers (subscribers) | **0** |
| Open issues (incl. PRs) | **0** |
| Created | **2026-08-19T18:16:11Z** |
| Last push | **2026-08-29T02:33:15Z** |
| Repo size | 6,061 KB |
| **Total commits** | **38** (from `Link:` rel=last header, `per_page=1`) |
| **Pull requests (all states)** | **24** — all merged/closed, 0 open |
| Non-PR issues | **0** (the 24 "issues" the API returns are the 24 PRs) |
| **Releases** | **0** |
| **Tags** | **0** |
| Contributors | `haoruilee` 24, `cursoragent` 13, `imgbot[bot]` 1 |
| npm | **404 — `oh-my-mcode` is NOT published** (`registry.npmjs.org/oh-my-mcode` → `{"error":"Not found"}`) |

**Ten days old, 38 commits, 51 stars, zero forks, zero external contributors, zero releases,
zero npm publish.** Roughly a third of the commits are authored by `cursoragent` — this was
built fast with heavy agent assistance, and the AGENTS.md/design-check.md discipline is visible
as the counterweight to that.

Two live-honesty problems follow from the numbers:

1. The README's headline install `npx oh-my-mcode install --yes` **does not work today** —
   the package 404s on npm. The README does caveat this twice ("Interim one-liner while this
   package is not on the public npm registry") but still leads with the broken command.
2. The README carries a `GitHub Release` shields badge that resolves to nothing (0 releases),
   and an `mcode-0.1.6` badge while the code targets 0.2.7.

---

## 9. What is GOOD, and what is BAD

### Genuinely good — steal these

1. **Ownership disclaimer as a load-bearing design constraint, not a footer.**
   `src/install.ts:8` `/** Official MiniMax Code CLI. We do not own this package. We are not a bundled host. */`;
   `install.ts:198` prints *"We do not own mcode. We are not a bundled host (not Senpi / omo-ai, not curl omp.sh/install)"*
   at install time. `docs/host-reality.md` has a whole section "We do not become Senpi / OMP" that
   reasons about *why* the competitor's one-command install is legitimate for them (they **are** the
   host) and not for this project. The disclaimer changed the architecture, which is the only way
   a disclaimer ever means anything.

2. **`docs/host-reality.md` — an observed-behavior ledger of the third-party binary.**
   This is the best artifact in the repo and I have not seen its equal elsewhere. It records, with
   dates and version numbers, things you can only learn by running the host and getting burned:
   - `mcode` `--timeout` regex is `/^(\d+)(ms|s|m|h)?$/i`; **a bare `180` is 180 *milliseconds***,
     which caused a real exit-6 timeout after PR #9 — so `formatHostTimeout()` (`src/mcode.ts:281`)
     now always emits a unit suffix.
   - `--session` and `--continue` are **mutually exclusive** (host throws → invocation exit 2);
     `sessionXorContinue()` enforces XOR in argv (`src/mcode.ts:259`).
   - `--output-schema` takes a **JSON object string, not a path**, and returned **exit 70**
     (host internal) on live 0.2.1 — so the default argv **omits the flag entirely** and yield is
     validated in TypeScript instead; `OMM_HOST_OUTPUT_SCHEMA=1` remains the probe.
   - The full documented exit table (`0/1/2/3/4/5/6/7/70/130`) → `HOST_EXIT` +
     `classifyHostExit()` (`src/mcode.ts:133-160`).
   - Node 24 + better-sqlite3 GC abort (`Statement::~Statement`, `RemoveEnvironmentCleanupHook`)
     → `HOST_NATIVE_CRASH_RE` and exactly **one** crash retry (`src/mcode.ts:184`).
   - Host token floor: a ~20-word exec still costs **17–20k input tokens** (fixture: 16,816 in /
     261 out), because the host's own system prompt dominates. Named as a host ceiling, not spun.
   Each fact is paired with the code that encodes it and the test that locks it
   (`test/host-contract.test.mjs`). **This is what "reverse-engineer the host, then write it down"
   should look like.**

3. **`finalizeHostExit()` — refusing to conflate three orthogonal signals.**
   `src/mcode.ts:167-177`: after *our* SIGTERM, Node's `close` gives `code=null, signal=SIGTERM`.
   The naive `code ?? 1` *lies that a timeout was a crash*. So `exitCode`, `timedOut`, and
   `signal` are kept as three independent facts, and "the host trapped our signal and exited 0"
   is still `timedOut`. That is an unusually careful piece of subprocess plumbing.

4. **`doctor` refuses to prove things it cannot prove.**
   `src/doctor.ts` emits a permanent `note`-level check:
   *"No public host API lists indexed Skills. If plugin list shows installed+enabled, files exist;
   triggering is not proven."* And `doctor --tps` prints **`unmeasured`** and exits non-zero rather
   than fabricating tok/s when the host is stubbed or omits `message.usage`
   (`src/tps.ts`, test at `test/*: "doctor --tps against fake-mcode is unmeasured and exits non-zero"`).
   Compare with every dashboard that shows a plausible number when it has no data.

5. **The "configured but invisible" check.**
   `src/doctor.ts:150-185` and `src/inspect.ts:86-130` cross-check the manifest's `skills[]` list
   against the `skills/` directory **in both directions** — a listed-but-missing file is an error,
   AND a directory-present-but-unlisted skill is an error (`extra`). It further validates each
   `SKILL.md` frontmatter `name` equals its directory name, and requires each description to
   contain a `do not` clause (negative triggers). Silent skill drop is the classic failure mode of
   markdown-asset frameworks, and this is a cheap, effective guard.

6. **Only the verifier may Accept, and Accept requires on-disk evidence.**
   `agents/verifier.md`: *"Only this role may set Accepted or Rejected… Accept without on-disk
   evidence files"* is in the Must-not list. Deterministic verification runs first, in TypeScript,
   with an **allowlist** of runnable commands (named check ∪ `detectProjectCommands`) so a
   model-authored `tasks.json` cannot smuggle a shell command into a spawn
   (`test/security.test.mjs:73,104,120`). The LLM judge is optional and read-only.

7. **Real, hostile security tests.** 31 assertions in `test/security.test.mjs` covering path
   traversal in run ids, evidence writes through symlinks, `../` escape in task ids, worktree
   path escape, workspace-config privilege escalation, `cleanSpawnEnv` dropping secrets/`npm_*`
   while keeping PATH, and lock-stealing only when the holder PID is dead
   (`canStealLock`, `src/store.ts:70-76`). `src/safe-path.ts` does `assertUnder` **plus**
   `realpathSync` re-checks on the nearest existing ancestor.

8. **`docs/design-check.md` — eight fixed questions applied to every change.**
   *"After each cut, eight questions. A failing check is a change, not a footnote."*
   Q3 "One core, many surfaces?" and Q4 "Subagents are workers, not trees?" are enforced in code:
   `src/harness.ts` `submit()` is the single core that both the CLI and the MCP server call, and
   `src/subagent.ts` uses `AsyncLocalStorage` so **depth ≥ 1 throws** — a worker cannot spawn a
   grandchild, with a test that calls spawn from inside a worker and expects failure.

9. **Hermetic CI by construction.** `.github/workflows/hermetic.yml` runs `npm test` +
   `npm run eval` with no live host and no secrets; `OMM_HERMETIC=1`/`CI=true` makes the host
   installer refuse to touch the network. Eval baselines are checked in
   (`evals/baselines/report.json`) and labeled *"Fixture harness only. Not a production ΔY statistic."*

### Bad — do not copy these

1. **Install = `rm -rf` + full-tree copy, with no manifest and no update path.** (§3, §5)
   Ships `test/`, `.github/`, `tsconfig.json`, `package-lock.json` into the host plugin dir —
   250 files / 6.0 MB where ~40 files are load-bearing. `package.json`'s `files:` allowlist already
   encodes the right answer and the installer ignores it.

2. **Extension contract and update mechanism are mutually exclusive.** The only way to customize
   is to edit installed files; the only way to update deletes them. Both silently.

3. **Two implementations of the same on-disk contract, with no parity test.**
   `src/store.ts` (762 LOC, used by CLI + MCP + tests) and `scripts/run-store.mjs`
   (904 LOC, plain JS, **zero imports from `dist/`**, used by the Skills). `grep -rn "run-store.mjs" test/`
   returns **nothing** — there is no test asserting the two produce identical `run.json`/`events.jsonl`.
   The rationale ("skills need a no-build tool") is legitimate; the missing conformance test is not.
   This will drift.

4. **`mcp.json` uses a relative command path.**
   `{"command":"node","args":["./mcp/server.mjs"]}` — `./` resolves against whatever cwd the host
   uses, not the plugin root. No `${PLUGIN_ROOT}`-style variable is available or used. If the host
   does not chdir to the plugin directory the MCP server silently fails to start, and there is no
   doctor check that the server actually *launches* (doctor only checks the file exists,
   `src/doctor.ts:243-256`). Muse Code must define an explicit plugin-root variable for this.

5. **Uninstall does not undo the host install.** The install may `npm i -g @minimax-ai/code`;
   the uninstall instructions never mention it.

6. **README leads with a command that 404s.** `npx oh-my-mcode install --yes` against an
   unpublished package, plus a dead release badge and a stale `mcode-0.1.6` badge on a repo
   targeting 0.2.7. For a project whose entire brand is honesty, the top of the README is the
   least honest part of it.

7. **A hand-rolled 130-line YAML subset parser** (`parseSimpleYaml`, `src/workflows.ts:16-79`)
   handling exactly one level of nesting. It is the right call for a zero-dependency package, but
   it means `workflows/*.yaml` is not really YAML, and a user editing it can get silent
   misparses (nested maps beyond depth 1 are dropped, not rejected).

8. **The workflow YAML is barely load-bearing.** Eight files, and `loadWorkflow` extracts
   essentially `phases[]` and `rules.stop_after`. Everything else in those files
   (`single_host_agent`, `only_verify_sets_accepted`, `max_repairs`, `deterministic_verify_first`)
   is **documentation that reads like configuration** — `rules` is typed
   `Record<string, unknown>` and no consumer reads those keys. That is a trap for anyone who
   edits `max.yaml` expecting `max_repairs: 3` to take effect (the real one is in
   `src/config.ts` `DEFAULT_CONFIG.maxRepairs`).

---

## 10. Transfer to oh-my-musecode (compiled Rust host)

### Transfers directly — this is the structural precedent we were looking for

| Pattern | Muse Code equivalent |
| --- | --- |
| **Dual/triple manifest, one tree** | Ship `.muse-plugin/plugin.json` **and** `.claude-plugin/` **and** `.codex-plugin/` from one directory, exactly as this ships portable `plugin.json` + `.minimax-plugin/plugin.json`. Muse's loader already recognises all three — this proves the pattern works and that a root-marker function (`packageRoot()` requiring both manifests) is how you anchor paths. |
| **Copy-a-folder install into `$HOME/<agent>/plugins/<name>`** | `~/.muse/plugins/oh-my-musecode/`. `MINIMAX_HOME` env override → `MUSE_HOME`. Host-agnostic, language-agnostic: it is `cp -r` plus a symlink refusal. |
| **Bootstrap-the-host-if-missing, with consent + honesty** | `mcodeExists()` → `which("muse")`. But Muse is a **compiled binary from dev.meta.ai**, not an npm package, so the "install the official package" branch has to be *"here is the official download URL"*, not an automated fetch. `install --skip-host` stays. The `CI=true` refusal to network-install is mandatory. |
| **`doctor` / `doctor --smoke` / `doctor --tps` triad** | package checks (manifests, skills-on-disk, frontmatter, bins) → host presence + version → one tiny real `muse exec`-equivalent → measured throughput or literal `unmeasured` + non-zero exit. All three transfer verbatim. |
| **"Configured but invisible" bidirectional manifest↔disk check** | Muse has `.muse/skills.lock`, so this becomes *stronger*: verify manifest ⊆ disk, disk ⊆ manifest, **and** lockfile ⊆ both. |
| **Host-version → capability-flag table** | `muse --version` → `{major,minor,patch}` → capability struct; **unparsed ⇒ all-false**, never optimistic. Given ~45 `MUSE_EXPERIMENTAL_*` gates, this generalizes to a *feature-gate probe table*: which gates exist on this build, which are on, which we refuse to depend on. This is the highest-value idea to port. |
| **`docs/host-reality.md` as a dated observed-behavior ledger** | Given we reverse-engineered the Muse binary, we already have the raw material. Write it in this shape: fact → date/version observed → the code constant that encodes it → the test that locks it. |
| **Exit-code table + orthogonal timeout/signal handling** | Muse's Rust binary has its own exit codes; the `finalizeHostExit` discipline (exitCode ⟂ timedOut ⟂ signal) is process-management wisdom that is completely language-independent. |
| **Verifier-only-Accepts + evidence-on-disk + content hashing** | The run-store design (`<ws>/.muse/runs/<id>/`) is pure filesystem convention. Nothing about it needs Node. |
| **Path-safety kit** (`assertUnder` + realpath re-check, id regexes, PID-aware lock stealing) | Maps 1:1 onto Rust (`std::fs::canonicalize`, `nix::sys::signal::kill(pid, None)`), and Rust makes it *easier*. |
| **Privilege ceiling on repo-local config** | A checked-in `.muse/oh-my-musecode.json` must not be able to raise permissions to `full`. Given Muse ships `lock.json` with `provenance`/`quarantine`/`allowed_tools`, this principle is already native to the host — align with it rather than reinventing. |
| **`design-check.md` eight-questions ritual + AGENTS.md as single source of truth** | Process, not code. Free to adopt. |
| **Honest marketplace doc** | Document the Meta plugin catalog we are *not* in, and how a human would submit. |

### Does NOT transfer

| Thing | Why |
| --- | --- |
| **`npx <pkg> install` / `npx github:owner/repo` as the install channel** | This is npm-specific and is precisely what makes their headline command broken today. A Rust-host framework needs its own channel: `curl \| sh` with a checksum, Homebrew, `cargo install`, or a `muse plugin add <git-url>` if the host provides one. |
| **`npm unlink -g` uninstall** | Ditto. |
| **`bin/*.mjs` shim with `--experimental-strip-types` src fallback** | Node-only cleverness (`bin/oh-my-mcode.mjs:21-30`). A Rust binary has no dist/src duality. |
| **`scripts/run-store.mjs` "no-build tool the skills call"** | This exists *only* because Node needs `tsc` before `dist/` is usable. A compiled Rust binary is already the no-build tool — the entire duplicate-implementation problem (§9 bad #3) **disappears** if `oh-my-musecode` is one binary that both the CLI and the skills invoke. This is a strict win for the Rust design. |
| **`stream-json` line parsing, `delta.content` stitching, `message.usage` extraction** | Structure transfers; every field name is MiniMax-specific and must be re-derived from Muse's MSP/`muse serve` stdio protocol. Muse exposes an **MSP host** — which is a *better* integration point than scraping stdout, and may make `subagent.ts`-style scraping unnecessary. |
| **`hostOutputSchemaEnabled()` / exit-70 workarounds** | Bug-for-bug host workarounds. The *practice* (default to omitting an unproven flag, gate it behind an env probe) transfers; the specifics do not. |
| **Zero-dependency stdio MCP server in 220 lines of `.mjs`** | Rust needs a real MCP crate (or to speak MSP directly). The design point that survives: **one core (`submit`), two surfaces (CLI + protocol server), no duplicated logic.** |
| **Hand-rolled mini-YAML parser** | In Rust, `serde_yaml` exists. Do not reproduce this. |
| **"No hooks / no slash commands / no custom agents" ceiling** | This is a *MiniMax* limitation, and it is the reason their `agents/*.md` are 98 lines of inert prose. **Muse Code has `.muse/hooks.json`, an agent-definitions subsystem, a rules subsystem, and workflows in its config crate** — so oh-my-musecode can actually ship real hooks and real agent definitions. The correct lesson is inverted: their honesty about *not* shipping hooks is the model; our capability set is genuinely larger, so ship them, and be equally precise about which ones are proven on which binary version. |

### Three concrete decisions this teardown forces

1. **Fix what they got wrong, in the design, on day one:** ship a **lockfile** —
   `~/.muse/plugins/oh-my-musecode/.omm-lock.json` listing every installed file + sha256 + source
   version. Update = diff, preserve user-modified files, report conflicts, never blind `rm -rf`.
   Muse already has `.muse/lock.json` and `.muse/skills.lock` as precedent for lockfiles being
   idiomatic on this host.

2. **Give users an overlay directory** (`~/.muse/oh-my-musecode/custom/{skills,agents,hooks}/`)
   that is merged over the packaged assets at load time and is **never** touched by install/update.
   This is the single missing piece that makes it an "oh-my-*" framework instead of a product.
   Combined with (1), it also resolves their extension/update contradiction.

3. **Keep their content-to-machinery ratio in mind and decide deliberately.** 654 lines of markdown
   assets to 8,130 lines of TypeScript is a *product with a plugin wrapper*, not a curation
   framework. oh-my-zsh's value is the 300 plugins and 150 themes; oh-my-mcode's value is the
   verifier and the run store. Both are valid; they are different products, and the install/update/
   registry design differs accordingly. If oh-my-musecode is to be the curation framework, the
   registry and overlay questions (§4, §6 — where this project is weakest) are the *primary*
   design problems, not afterthoughts.

---

## File-path index for follow-up

| Question | Read |
| --- | --- |
| Installer | `src/install.ts` (249 L), `scripts/install.sh`, `scripts/install.ps1` |
| Config layering + privilege ceiling | `src/config.ts` |
| Host contract / argv / exit codes | `src/mcode.ts`, `src/host-version.ts` |
| Observed host behavior ledger | `docs/host-reality.md` |
| Health checks | `src/doctor.ts`, `src/tps.ts`, `src/inspect.ts` |
| Run store + locking | `src/store.ts`, `scripts/run-store.mjs` (the duplicate) |
| Path safety | `src/safe-path.ts`, `test/security.test.mjs` |
| Single core, two surfaces | `src/harness.ts`, `src/cli.ts`, `mcp/server.mjs` |
| Depth-limited workers | `src/subagent.ts` |
| Design ritual | `docs/design-check.md`, `AGENTS.md` |
| Registry honesty | `docs/marketplace.md` |
| Manifests | `plugin.json`, `.minimax-plugin/plugin.json`, `mcp.json` |

---

## Verification

**Verdict: MOSTLY_SOLID.** Independent re-verification on 2026-09-01 from a fresh
`git clone --depth 50` into
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/ohmy/verify-haoruilee-oh-my-mcode/repo`.
The repo is real, the traction numbers are exact, every shipped-content count reproduces, and the
install/update and extension-contract descriptions are faithful to the code. Four small overcounts
or attribution slips are corrected below; none of them changes a conclusion.

### 1. Existence and traction — CONFIRMED, exact

`gh api repos/haoruilee/oh-my-mcode` returns, verbatim:

| Field | Claimed | Measured |
| --- | --- | --- |
| stars | 51 | `stargazers_count: 51` |
| forks | 0 | `forks_count: 0` |
| subscribers (watchers) | 0 | `subscribers_count: 0` |
| open issues | 0 | `open_issues_count: 0` |
| created | 2026-08-19T18:16:11Z | identical |
| pushed | 2026-08-29T02:33:15Z | identical |
| size | 6,061 KB | `size: 6061` |
| tracked files | 147 | `git ls-files \| wc -l` → 147 |
| commits | 38 | `Link rel="last"` → `page=38` on `commits?per_page=1` |
| releases / tags | 0 / 0 | both `[]` |
| contributors | haoruilee 24, cursoragent 13, imgbot[bot] 1 | identical |
| PRs (all states) | 24 | 24 |
| non-PR issues | 0 | 0 |

npm status confirmed: `registry.npmjs.org/oh-my-mcode` → **HTTP 404** `{"error":"Not found"}`.
Host package `@minimax-ai/code` → latest **0.2.7**, **15** versions, modified **2026-08-28T11:30:57Z**.
All three match the report exactly. Not fabricated.

### 2. Shipped-content counts — CONFIRMED, every one

Re-ran the finds and `wc -l` myself:

- 10 `skills/*/SKILL.md`, **556** lines total, plus `skills/max/references/run-store.md` — exact.
- 5 `agents/*.md`, **98** lines total — exact.
- 8 `workflows/*.yaml`, 6 `schemas/*.schema.json` — exact.
- `mcp/server.mjs` **220** lines, 7 tools (`omm_run_create`, `omm_run_show`, `omm_run_list`,
  `omm_status`, `omm_verify`, `omm_interview`, `omm_inspect`) — exact.
- **34** `src/*.ts`, **8,130** LOC; `src/store.ts` 762 LOC — exact.
- **15** `test/*.test.mjs`, **4,660** LOC — exact.
- 8 `docs/*`, **915** lines — exact.
- `scripts/run-store.mjs` **904** LOC, zero imports from `dist/` (only `node:crypto`, `node:fs`,
  `node:path`, `node:url`) — exact.
- 4 eval fixtures (`pass`, `fail-then-repair`, `plan-only`, `follow-goal`) + checked-in
  `evals/baselines/report.json` — exact.
- 15 CLI commands from `--help`: max plan verify resume review ship research attach status cancel
  inspect team interview doctor install — exact.

**I ran the suite.** `npm ci && npm test` on Node v24.15.0 → `tests 142 / pass 142 / fail 0`.
`doctor --package-only` → `oh-my-mcode doctor PASS`, exit 0. Both claims reproduce.

### 3. Install and update — CONFIRMED verbatim, including line numbers

`src/install.ts:127-128` is exactly as quoted:

```ts
if (existsSync(dest)) rmSync(dest, { recursive: true, force: true });
cpSync(root, dest, { recursive: true, dereference: true, filter: (src) => { ... } });
```

The filter is precisely the claimed 3-entry top-level denylist (`.git`, `node_modules`, `.minimax`)
and ignores `package.json`'s `files:` allowlist. `src/install.ts:8` carries the ownership comment
verbatim; `src/install.ts:198` prints
`"We do not own mcode. We are not a bundled host (not Senpi / omo-ai, not curl omp.sh/install)."`
at install time. `src/install.ts:155` logs "install does not write or overwrite a project AGENTS.md."

**No update command exists.** `grep -rniE "uninstall|upgrade|\bupdate\b" src/ scripts/` returns only
`hash.update()`, `createHash(...).update()` and a `store.ts` doc comment. Confirmed.

**I measured the footprint myself** with `MINIMAX_HOME` pointed at a sandbox and
`install --skip-host --yes`: **250 files, 6.0 MB** — matching the report to the file. The installed
top level does contain `.github/`, `test/`, `tsconfig.json`, `package-lock.json`, `.gitignore`,
`AGENTS.md`, `CLAUDE.md`, `evals/` and `src/`, exactly as described.

`scripts/install.sh` confirmed: `rm -rf "$DEST"` then rsync `-a --copy-links` with a tar fallback,
no Node required. `install.ps1` uses `Remove-Item -Recurse -Force` + `Copy-Item`. README uninstall is
literally the two lines quoted, at README.md:110-114.

### 4. Extension contract — CONFIRMED, and the report is right that it is nearly absent

- No `custom/`, no `skills.d/`, no overlay merge anywhere. Confirmed by directory listing and grep.
- `src/config.ts` has exactly the 4 claimed knobs (`permission`, `maxRepairs`, `llmVerify`,
  `team{concurrency,worktree}`), and the privilege ceiling is real: `readConfigFile` refuses
  `permission: "full"` when `source === "workspace"`, with the comment
  "Workspace file cannot silently raise permission to full." Locked by
  `test/security.test.mjs:396` ("workspace config cannot silently raise permission to full") and
  :411 ("workspace config may set ask/smart/off; home file may set full"). Confirmed.
- **The dead-workflow-keys criticism is correct and is the sharpest finding in the report.**
  `loadWorkflow` extracts only `phases[]` and `rules.stop_after`; `rules` is returned as
  `Record<string, unknown>` and `grep` for `only_verify_sets_accepted` / `single_host_agent` /
  `deterministic_verify_first` finds **zero consumers outside the YAML files themselves**.
  `workflows/max.yaml` really does declare `max_repairs: 3` while the effective value comes from
  `DEFAULT_CONFIG.maxRepairs = 3` in `src/config.ts` / `orchestrator.ts:392`. A user editing the
  YAML is indeed trapped.
- `mcp.json` relative-path defect confirmed: `{"command":"node","args":["./mcp/server.mjs"]}`, no
  `${PLUGIN_ROOT}` anywhere, and `src/doctor.ts` only `existsSync`-checks the file.
- Missing parity test confirmed: `grep -rn "run-store" test/` returns **nothing**.

### 5. Host-contract claims — CONFIRMED verbatim

Every specific fact quoted from `docs/host-reality.md` is present with the stated detail:
the `chm` timeout regex `/^(\d+)(ms|s|m|h)?$/i` and the exit-6 incident after PR #9 (line 108);
`--session` and `--continue` mutually exclusive → exit 2 after PR #12 (line 118); `--output-schema`
exit 70 on live 0.2.1 with the no-flag run succeeding in 19.1s (line 104); the
`message.usage` fixture **16816 in / 261 out** and the 17–20k input-token floor named as a host
ceiling (line 123). `HOST_EXIT` in `src/mcode.ts:133-144` is the full table
`0/1/2/3/4/5/6/7/70/130`. `finalizeHostExit` (`src/mcode.ts:167-177`) keeps `exitCode`, `timedOut`
and `signal` orthogonal exactly as described. `HOST_NATIVE_CRASH_RE` matches
`better-sqlite3|RemoveEnvironmentCleanupHook|Statement::~Statement|SIGABRT`. `host-version.ts`
capability table and unparsed-⇒-all-false confirmed. `src/subagent.ts:106` throws on
`parent.depth >= 1` via `AsyncLocalStorage`. `harness.submit()` is genuinely the single core called
by both `src/cli.ts` and `mcp/server.mjs`. `doctor.ts:294` and `tps.ts` `TPS_UNMEASURED` confirmed.
`docs/design-check.md:5` — "After each cut, eight questions. A failing check is a change, not a
footnote." — verbatim. `docs/marketplace.md` opens exactly as quoted, including the Feishu /
`submission_id` submission path and the "we are not opening a PR there" line about
hetaoBackend/MiniMax-Code-Plugins.

### 6. Corrections

Four items are wrong or over-attributed. All are small.

1. **"31 genuinely hostile security tests" → 21.** `test/security.test.mjs` contains **21**
   top-level `test("…")` calls. A naive `grep -c "test("` yields 30 because nine of those lines are
   `/…/.test(error.message)` assertion callbacks inside `assert.rejects`/`assert.throws`. The suite
   is still genuinely hostile and covers every named attack (traversal run ids, symlinked evidence
   dests, `../` task-id escape, worktree escape, workspace privilege escalation, `cleanSpawnEnv`,
   dead-pid lock stealing) — the count is inflated by ~50%.

2. **"a hand-rolled 130-line YAML subset parser (parseSimpleYaml, src/workflows.ts:16-79)" is
   self-inconsistent.** `src/workflows.ts` is **130 lines total**; `parseSimpleYaml` spans lines
   16–67 and its helper `coerce` 69–79, i.e. the parser is **~64 lines**, not 130. The substantive
   criticism (one level of nesting, silent drops rather than rejects) is correct as written.

3. **The bidirectional manifest↔disk check is implemented in `src/inspect.ts` only, not in
   `src/doctor.ts`.** `inspectSkills` (`src/inspect.ts:104-112`) does `readdirSync(skills/)` and
   computes `extra` = on-disk-but-unlisted, which is the real both-directions check. `src/doctor.ts`
   compares the manifest against a **hardcoded 10-entry `expected` array**, so a *new* skill
   directory added without a manifest entry would pass `doctor` and only be caught by `inspect`.
   The frontmatter validation (`name` == directory, description must contain a "do not" clause) is
   genuinely in `doctor.ts` and works — I saw all 10 `frontmatter:*` checks pass. The idea is worth
   stealing as the report says; the attribution to doctor.ts is half wrong.

4. **"the top of the README is its least honest part" overstates it.** The 404 is real and the hero
   command genuinely does not work. But the same Installation section discloses this twice: a TL;DR
   row labelled "**From git (until npm publish)**" and the line "Interim one-liner while this
   package is not on the public npm registry." So the broken hero is *documented*, not concealed.
   The genuinely undisclosed inaccuracies at the top of the README are the two badges — a GitHub
   Release badge against **0 releases / 0 tags**, and a `mcode-0.1.6` badge on code that targets
   0.2.7. Both confirmed.

Minor, not worth counting as errors: the env-override list omits an eighth variable,
`OMM_MCODE_STUB` (the full set is `OMM_MCODE`, `OMM_MCODE_STUB`, `OMM_WORKSPACE`,
`OMM_PACKAGE_ROOT`, `OMM_RUN_ID`, `OMM_HOST_OUTPUT_SCHEMA`, `OMM_HERMETIC`, `MINIMAX_HOME`). And the
"~40 load-bearing files" figure is the report's own editorial judgment rather than a measurement —
`package.json`'s `files:` allowlist is in fact broader (it also ships `src`, `docs`, `examples`,
`evals`, `AGENTS.md`, `CLAUDE.md`). The contrast still holds, because `files:` excludes `test/`,
`.github/`, `tsconfig.json`, `package-lock.json` and `.gitignore`, all of which the installer copies.

### 7. Nothing refuted

No claim was found to be fabricated. The repo exists, the numbers are real, the code does what the
report says it does, and the two headline design criticisms — the extension contract and the update
mechanism being mutually exclusive, and the un-parity-tested duplicate run store — both reproduce
directly from the source. The transfer analysis to oh-my-musecode rests on verified facts.
