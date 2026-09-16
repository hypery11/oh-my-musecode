# oh-my-musecode — Architecture

`omm` is a curated content bundle, a thin CLI, and a git marketplace for **Meta Muse Code**.
It is not a loader and it never touches the Muse binary. Everything it does goes through Muse's
own config surface and its own CLI verbs.

This document is the contract every crate is built against. When it disagrees with a research
report under `research/`, the research report wins and this document gets fixed.

## 0. Non-negotiables (from the research — each traceable)

| # | Rule | Source |
|---|---|---|
| R1 | **The ownership ledger is the primitive.** `install`, `update`, `doctor`, `uninstall` are the same traversal over `omm.lock.json`. | MATRIX M1 |
| R2 | **Record what the installer wrote, never a scan of the destination.** | MATRIX M2 |
| R3 | **Three-way merge with a recorded ancestor from the first release.** Outcomes: no-op / overwrite / adopt / stage. Never overwrite a user edit; never freeze. | MATRIX M3, M4 |
| R4 | **Uninstall is a ledger traversal with a two-section preview, `--dry-run`, allowlisted roots, `canonicalize()`+`strip_prefix()` containment, deepest-first.** Ship it before install. | MATRIX M5 |
| R5 | **CI test: install into a clean `$HOME`, uninstall, assert byte-identical.** | MATRIX M6 |
| R6 | **Two-phase.** Placing the binary writes nothing. Only `omm install` touches Muse config. Refuse under `CI=true`/non-TTY without `--yes`. | MATRIX M7 |
| R7 | **Overlay dir searched before bundled content + capability-qualified disable ids + an APPEND channel** (`<name>.md` replaces, `<name>_append.md` appends). | MATRIX M8, M9 |
| R8 | **No asset name in a Rust enum or const array.** The catalog is data. | MATRIX M10 |
| R9 | **Never write framework state into `settings.json`.** Muse rewrites it from a typed struct and destroys unknown keys. Only the 29 typed keys may be written, each recorded in the ledger with its prior value. | SYNTHESIS §1.7 |
| R10 | **Every write to a file we do not own is a targeted, validated, atomic merge on the resolved realpath, with a verified backup first.** Validate `settings.json` candidates with `muse config validate --plane defaults --file <tmp>` before landing them. | MATRIX M13 |
| R11 | **Validate plugin packages on four predicates:** `rc==0 && valid==true && diagnostics==[] && manifest_family=="native" && every compatibility declaration "supported"`. Never on `valid` alone. | DECISION rule 11 |
| R12 | **The packer never emits a symlink** anywhere in a package, referenced or not; scan first so doctor can print the real reason. Reject backslashes in filenames. | DECISION rule 12 |
| R13 | **Three checkpoints in order:** `muse skills validate <skill-dir>` per skill → `muse plugins validate <pkg>` → post-install `muse skills list --json`. | DECISION rule 13 |
| R14 | **The trust lifecycle is `omm`'s.** Install approves every capability and then verifies via `muse plugins inspect --json` that the line is PRESENT and literally `trusted_enabled`. Update = `marketplace update` → digest compare (equal → no-op) → pre-flight install in a scratch `XDG_DATA_HOME` → `plugins remove` → `plugins install` → approve each → verify, as one transaction. **Never `plugins update omm`** — it refreshes the pinned generation and fails with `plugin-source-unavailable` after two rotations. Doctor asserts presence AND state. There is no session-time signal for any of this. | DECISION rule 14, experiments/marketplace-precedence §6.4 |
| R15 | **Never gate on the version number.** Probe behaviour. Run the golden fixture set on every channel poll. | DECISION rule 15 |
| R16 | **Hooks are `omm hook <name>` invoking this one static binary.** The command string is shell-neutral (`omm hook <name>` runs unchanged under `$SHELL -c` and under PowerShell), which is how the Windows problem is sidestepped: plugin `capabilities.hooks` is a CLOSED family that **rejects `commandWindows`** (`unsupported-field`, docs/experiments/marketplace-precedence.md §7), so plugin hooks carry `command` only. User/project-tier `hooks.json` documents may carry both forms. Fail-open on non-zero exit. Sub-5 ms dispatch. | MATRIX M15, DECISION rule 9, experiments/marketplace-precedence §7 |
| R17 | **Executable extensions run out-of-process only.** MSP is NOT a tool-registration path (no such method exists; MSP sessions run no hooks). | DECISION §4 |
| R18 | **Budget every shipped item.** Catalog payload ≤ 21,542 B with Meta's built-ins on, ≤ 31,602 B with them disabled, ≤ 27,168 B with `run.context_slimming.skill_catalog_descriptions: "first_sentence"` (PROVEN, −52% on a 40-skill install); `order200 + order201 ≤ 31,984 B` when routing is on. CI refuses a bundle over budget. Trap: `full_skill_description_ids` defaults to `["bundled:git"]` and a user value REPLACES it — always re-list `bundled:git`. Cheapest single saving: `excluded_tool_names: ["workflow"]` (−20,952 B, drops the cookbook and the 15,766 B tool definition). | DECISION rule 3, experiments/context-slimming |
| R19 | **Plugin id is `omm`.** `len(plugin-id) + len(server-id) ≤ 18`. Every skill/command/capability id is `omm-` prefixed and linted against the 9 reserved ids, the 15 bundled skills and the 39 built-in slash commands. | DECISION rule 4 |
| R20 | **Argv allowlist.** `omm run` validates the first token against the 16 known Muse top-level commands — an unknown token is a *prompt* and silently starts a billed session. | DECISION §2.2 |
| R21 | **No enterprise tier. No MSP control plane.** Struck from the roadmap, not deferred. | DECISION §4 |
| R22 | **Config can only tighten** — a compile-checked lattice, not a `min()` convention. A project-scope config cannot raise a permission the user scope lowered. | MATRIX M18 |

## 1. Repository layout

```
oh-my-musecode/                       ← ALSO the marketplace repo root
├── Cargo.toml                        ← workspace
├── crates/
│   ├── omm-host/                     ← everything about the Muse binary (§3)
│   ├── omm-ledger/                   ← omm.lock.json, reconcile, snapshot, audit (§4)
│   ├── omm-manifest/                 ← content model, generators, lint, budget (§5)
│   ├── omm-doctor/                   ← the checks + cost model (§6)
│   └── omm/                          ← the CLI binary (§7)
├── content/                          ← CANONICAL SOURCE, hand-authored (§5.1)
│   ├── skills/<id>/SKILL.md
│   ├── commands/<id>.md
│   ├── hooks/<id>.json               ← one hook definition per file
│   ├── reminders/<id>.json
│   ├── mcp/<id>.json                 ← one MCP server declaration per file (`doc` → `["omm","mcp"]`)
│   ├── agents/<name>.md
│   ├── themes/<name>.tmTheme
│   ├── profiles/<name>.json          ← settings slices (strict / default / fast / ci)
│   ├── rules/AGENTS.md.tmpl
│   ├── translation/muse.md           ← tool-vocabulary block for skills ported from the foreign plugin formats
│   ├── routing/library/<id>/SKILL.md ← the routed library (opt-in M4; claimed, never packaged — docs/ROUTING.md)
│   └── catalog.json                  ← THE asset catalog: id, kind, lifecycle, core flag (R8)
├── plugins/omm/                      ← GENERATED native package (committed, drift-gated)
│   ├── .muse-plugin/plugin.json
│   ├── skills/ commands/ hooks/ reminders/ agents/
├── dist/                             ← GENERATED projections (COMMITTED — a catalog `source` must resolve inside the repo)
│   ├── claude/  (.claude-plugin/plugin.json + content)
│   └── codex/   (.codex-plugin/plugin.json + content)
├── marketplace.json                  ← GENERATED, NATIVE Muse catalog — the ONLY one Muse reads (probed first; first-found-wins)
├── .agents/plugins/marketplace.json  ← GENERATED, Codex schema → dist/codex (for hosts of that format; Muse never opens it while marketplace.json exists)
├── .claude-plugin/marketplace.json   ← GENERATED, Claude schema → dist/claude (for hosts of that format; same)
├── docs/
│   ├── ARCHITECTURE.md               ← this file
│   ├── PLAN.md                       ← milestones + acceptance criteria
│   ├── host-reality.md               ← fact → version → constant → test
│   └── INSTALL_FOR_AGENTS.md         ← the installer's user is another agent
├── tools/pty/                        ← drive.py / render.py, the TUI harness
├── tools/mockprovider/               ← the scripted-model harness (MCP tools/call, omm mcp, skill routing)
├── crates/omm/tests/e2e.rs           ← clean-HOME install/uninstall/doctor scenarios (30)
├── scripts/
│   ├── gate.sh                       ← fmt, clippy, every test against the pinned host
│   ├── fetch-host.sh                 ← download + sha256-verify the Muse binary into .host/
│   └── release.sh                    ← per-platform binaries + checksum manifest
├── install.sh                        ← `curl … | sh`: places the binary, writes nothing else (R6)
├── .github/workflows/ci.yml          ← the gate on push/PR; a daily channel poll (R15)
└── .host/                            ← gitignored: muse binary, launcher, extracted contract
```

## 2. On-disk footprint on the user's machine

Everything `omm` owns lives in **one** directory. Everything it writes into Muse's directories is a
ledger entry.

```
$XDG_CONFIG_HOME/omm/  (else ~/.config/omm/)
├── omm.lock.json                     ← THE LEDGER (§4)
├── audit.log                         ← JSONL, append-only, same shape as Muse's skills/.muse/audit.log
├── config.json                       ← user config: profile, disabled ids, overlay opts (small!)
├── custom/                           ← OVERLAY: searched before bundled content (R7)
│   ├── skills/ commands/ hooks/ agents/ themes/
├── updates/<version>/<asset>         ← STAGED conflicts from `omm update` (R3)
├── snapshots/<timestamp>/            ← rolling pre-update snapshots
├── locks/                            ← advisory flock(2) files, never beside the host's files:
│   ├── muse-config.lock              ←   held by every writer of settings.json / trust.json from stale check through rename
│   └── ledger.lock                   ←   held by every writer of omm.lock.json
└── install-provenance.json           ← how omm itself was installed (brew/curl/cargo)

Written into Muse's config root ($XDG_CONFIG_HOME/muse, else ~/.config/muse) — ALL ledgered:
├── skills/<id>/SKILL.md              ← via `muse skills install` (managed store, has its own lock)
├── AGENTS.md                         ← personal rules (exactly one file loads — first-hit-wins)
├── themes/<name>.tmTheme
├── trust.json                        ← merged, never overwritten
└── settings.json                     ← TARGETED keys only, each with prior value in ledger

Written into Muse's data root ($XDG_DATA_HOME/muse, else ~/.local/share/muse) — by Muse itself:
└── plugins/                          ← `muse plugins install` owns this; omm records the registration

Deliberately kept residue (named in uninstall preview, outside our ownership):
└── ~/Library/Application Support/Muse/session-name-authority/   (Muse writes via getpwuid on every run)
```

Muse's real paths (from `research/musecode/config-paths.md`): exactly two candidates each, XDG first,
no `MUSE_HOME`, no `MUSE_CONFIG_DIR`. Redirecting `HOME` does **not** fully sandbox Muse.

## 3. `omm-host` — the Muse binary as a dependency

Responsibilities:

- **Locate** the binary: `$OMM_MUSE_BIN` → `~/.local/bin/muse` launcher's active `muse-bin-<version>`
  (read `.muse-version` next to it) → `which muse`. Never the launcher itself for subprocess calls
  (it auto-updates hourly; we call `muse-bin-*` directly with `MUSE_NO_AUTO_UPDATE=1` set anyway).
- **Invoke** with a controlled environment: `MUSE_NO_AUTO_UPDATE=1`, `MUSE_EXPERIMENTAL_PLUGINS=1`
  only for `plugins` verbs (install-time gate), pass-through `XDG_*`/`HOME`, `NO_COLOR=1`,
  `--json` everywhere it exists. Parse exit codes: `2` = argv rejected (parse error OR missing gate —
  indistinguishable), `1` = ran and failed, `0` = ok. **No unknown-command exit exists**: an
  unrecognised first token is the `[PROMPT]` positional.
- **Argv allowlist** (R20): `resume exec config export trace skills sandbox schema serve
  session-message auth login logout init plugins workflows`.
- **Probe** capabilities from behaviour, never from the version string (R15): `--version` parse
  (informational only), `config validate` availability, `plugins --help` under the gate, gate
  resolution from the bootstrap trace `$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-<uuid>.log`
  (`gate.resolve gate=<id> enabled=<bool> source=default|override`), `skills list --json`,
  `plugins inspect <id> --json`.
- **Paths**: config root, data root, memory roots incl. the `personal_project` formula
  `memory/projects/<slug96>-<fnv1a64hex>/` (slug = canonical path minus leading `/`, non-alnum → `-`,
  truncate 96; hash = FNV-1a-64 of the full path as `%016x`).
- **`settings.json` editing** (R9, R10): load → typed-key patch → serialize → validate with
  `muse config validate --plane defaults --file <tmp>` → atomic rename on the realpath → verified
  backup first. Refuse if `mcpServers` and `mcp_servers` both present (every settings-writing Muse
  command exits 1 in that state — report it, don't add to it).
- **`trust.json`**: `{"schema_version":1,"projects":{"<abs>":{"decision":"trusted"}}}` — merge one
  key; never rewrite the map.
- **Golden constants** (`host_reality.rs`): the table in `docs/host-reality.md`, each constant with
  the test that locks it. `omm doctor --self-test` and `tests/hostcheck` run them.

## 4. `omm-ledger` — the ownership ledger

```jsonc
// $XDG_CONFIG_HOME/omm/omm.lock.json
{
  "schema_version": 1,
  "omm_version": "0.1.0",
  "host": { "version": "1.0.1-R2006.1", "sha256": "b9c7…" },   // observed at install
  "scope": "user",
  "entries": [                                                    // sorted by (base, path)
    {
      "base": "muse-config",            // muse-config | muse-data | omm | workspace
      "path": "skills/omm-plan/SKILL.md", // ALWAYS base-relative, never absolute
      "kind": "skill",                  // skill|command|hook|agent|theme|rules|settings-key|trust|plugin
      "sha256": "…",                    // what WE wrote (R2) — this is the ANCESTOR for the next reconcile
      "source_version": "0.1.0",
      "writer": "omm install",
      "mechanism": "copy",              // copy | muse-skills-install | muse-plugins-install | settings-patch | trust-merge
      "class": "exclusive",             // exclusive | shared-key | seeded
      "prior": null                     // for settings-key / trust: the value before we touched it (for exact undo)
    }
  ],
  "registrations": [                    // things that are not files but must be undone
    { "kind": "muse-plugin", "id": "oh-my-musecode", "approved": ["plugin:oh-my-musecode:mcp_server:doc", "…"] },
    { "kind": "muse-marketplace", "name": "omm", "source": "…" }
  ]
}
```

Rules: entries sorted `(base, path)` for byte-stable reruns; a corrupt ledger is renamed `.bad` and
treated as absent (with a loud warning); a **non-regular-entry sentinel** — a directory containing a
symlink/socket can never hash equal and is therefore retained (R2).

**Reconcile** (R3), per entry, with `ancestor = entry.sha256`, `theirs = new source sha`,
`mine = on-disk sha`:

| ancestor vs theirs | mine vs ancestor | outcome |
|---|---|---|
| equal | any | **no-op** |
| differ | equal | **overwrite** (user never touched it) |
| differ | equal to theirs | **adopt** (user already has the new content) |
| differ | differ | **stage** → `updates/<version>/<path>` + report; on-disk untouched |

**Snapshot**: before any update, copy every ledgered file to `snapshots/<ts>/`, rolling (keep 5).
**Audit**: every install/update/uninstall/approve appends one JSONL line.

**Uninstall** (R4): traversal in reverse, deepest-first; two-section preview (✗ remove / ✓ preserve —
preserve = anything whose on-disk sha ≠ our sha, i.e. the user edited it, unless `--force`);
allowlisted roots = exactly the four bases; `canonicalize()`+`strip_prefix()` on every path; refuse
`/`, `$HOME`, and any base root itself (an entry whose ancestor became a symlink out of its base is
preserved with the reason and the rest of the plan continues — and so is one whose ancestor became a
symlink **inside** the base, `Resolved::via_symlink`: omm never wrote a symlink (R12), so what lies
behind one is the user's even when byte-identical; the R2 sentinel, Gate 1 decision C; `omm install`
skips a theme behind such an ancestor for the same reason). Registrations undone: `muse plugins
remove oh-my-musecode --delete-data`, `muse plugins marketplace remove omm`, settings keys and trust entries
restored to `prior` by targeted patch — each registration records `value` (what omm wrote) beside
`prior` and is restored only while the key still holds `value`; a hand edit since is preserved and
reported under ✓. `AGENTS.md` (Gate 1 round 5, decision H2): the file is `[anything]` + the managed
block (`omm:managed-start` .. `omm:managed-end`, marker lines included) + `[anything]`, and omm owns
ONLY the block. Install and update replace the block in place — inserted at the top of a
pre-existing file that has none, the file's own bytes untouched — and keep every other byte;
uninstall removes the block and its markers plus the template's recorded frame (`prior.frame`: the
title and blank lines the seed contributed, each piece only while it still equals the template's)
and keeps the rest byte for byte, restores a pre-existing file from the `prior.replaced` bytes kept
in its entry when stripping the block leaves exactly that file (never only from a snapshot uninstall
deletes), and deletes the file only when omm created it and nothing else remains. The legacy
`omm:user-start` / `omm:user-end` pair stays supported inside the file (its marker lines are omm's,
its text the user's). For themes and `AGENTS.md` the ledger entry is upserted and saved BEFORE the
file is written (a `Missing` entry is harmless; an unledgered file carrying omm's block is not —
doctor D13 and the uninstall preview name one). The ledger is saved after every host mutation
during install (after `marketplace add`, after `plugins install` with `approved: []` — the host's
`--version` is asked for before the first mutation, so nothing sits between a mutation and its save
— after every `muse skills install` of a `--no-plugin` install, and after every file omm itself
writes, ahead of the audit line), so an
interrupted install always leaves something to traverse; with no ledger and a host-registered
`oh-my-musecode`/`omm` or catalog skills in the managed store (told by the store lockfile's source path or the
resolvable source catalog), `omm uninstall --reconcile-host` removes them and says so; with a ledger
the host outgrew (killed between a host verb and its save: the plugin or marketplace registered,
a managed-store skill installed, none of it in the ledger) `omm uninstall` names them under `!` and
refuses before applying anything, and `--reconcile-host` undoes them with the rest. A
`settings.json` the host creates during the approvals is ledgered as omm's (`seeded`) the moment the
package is installed. Shared files omm found before it wrote (`settings.json`, `trust.json`) keep
their exact bytes and mode in their entry (`prior.original`), recorded at the start of `omm install`
before any host verb can rewrite them; uninstall, after every host step and key restore, merges the
user's untyped keys back (the host's rewrites destroy every key it does not type) and writes the
recorded bytes and mode when the document then equals the pre-omm one, else keeps the merged
document at the recorded mode and says so. A bundle install refuses up front when `settings.json`
holds untyped keys, naming them, unless `--drop-unknown-settings-keys` accepts the host's rewrite;
**after every host mutation that can rewrite `settings.json`** (`plugins install / approve / remove`,
`skills install / uninstall`) omm puts such keys back and re-applies the recorded pre-install mode —
at the end of `omm install`'s and `omm update`'s host steps, and in the uninstall right after its
host steps and again with the bytes — so a private 0600 file is 0600 for the install's whole
lifetime, and doctor D4 keeps naming the doomed keys until the user moves them (Gate 1 decision F).
A registration whose shared file the user removed since install is dropped and named (nothing to
restore into), and a file a host step of the uninstall recreates empty is removed again.

**The host undo is idempotent** (Gate 1 decision B). `omm uninstall` always asks the host what it
holds — `plugins list`, `plugins marketplace list`, `skills list --source user` — before planning. A
registered plugin, marketplace or managed skill the host no longer has (removed by hand, or by an
uninstall interrupted before its ledger save) is dropped as *already gone* with the host's reason,
audited, and the plan continues; the host's `not installed` / `not configured` / `skill not installed`
answers at apply time count as done for the same reason, never as a failure that wedges every rerun.
What the host holds of omm's that the ledger does not list is shown under a third preview section,
`! beyond the ledger (host has it, ledger does not)`, and removed only with `--reconcile-host`; the
refusal names that flag. A `--no-plugin` install saves the ledger after every `muse skills install`,
so an interrupted one is at most one skill ahead of its ledger. Two ownership rules apply to every
managed-store skill the host would be asked to uninstall — ledgered, beyond the ledger, or with no
ledger at all (Gate 1 round 5): a store directory that is a symlink, is reached through one, or
resolves outside the base (`Bases::resolve`) is never handed to the host (`muse skills uninstall`
empties whatever the link points at) and is kept and named under ✓ with the R2-sentinel reason;
and a directory the host lists with no provenance (killed inside `muse skills install`: the files
copied, the lockfile not yet written — `muse skills uninstall` refuses it, `provenance-missing`) is
removed by omm itself, contained and deepest-first, only when the lockfile does not list it, it is a
regular directory inside the base, every file present is byte-identical to the source and nothing
beyond the source's files is in it; otherwise it is kept and named. Both `omm install --no-plugin`
and `omm uninstall --reconcile-host` converge over such a directory. A malformed `settings.json`
or `trust.json` is caught by the install plan, not mid-install: the plan mirrors
`SettingsDoc::load` / `TrustStore::load` on both files and then probes the host with `muse skills
list --json` (which loads its config; `--version` does not), refusing up front with the host's
reason and a repair hint. An uninstall whose host view fails (`skills list` exits 1 over such a
file) degrades to a blind undo — every registration planned, the host's already-gone answers
tolerated, a warning printed — and completes; a host step that then fails leaves the shared files
and their entries for the rerun.

**Structural keys** (Gate 1 decision D). `docs/host-data/settings-keys.json` marks the typed keys whose
object is a `deny_unknown_fields` struct with required members (`items[].structural`; `permissions`
requires `schema_version`). Restoring such a member to its prior (absent) — at uninstall, or when a
profile switch puts the outgoing slice's leaves back — never removes it while the object still holds
members omm did not write: the key is kept and named (`kept: …`), because the host refuses the whole
object without it (`Named permission profiles are unavailable: missing field schema_version`) and the
user's own named profiles would silently stop working. A structural leaf is restored last among the
settings keys, so it goes only with the last other member (the emptied object with it). Doctor D4
names a present, non-empty `permissions` object lacking `schema_version`, with the fix
`omm settings set permissions.schema_version 1`. Settings keys and trust entries
that are current but never registered (a ledger rebuilt from the host knows no prior) are named by
`omm reconcile`, doctor D13 and the uninstall preview, and left. `omm update --source <checkout>`
re-registers the marketplace from that checkout when it differs from the one registered at install,
so the plugin never stays behind the rules, themes and skills. Host-owned residue is named
unconditionally under "kept (host-owned)":
`$XDG_DATA_HOME/muse/{sessions,local-tracing,plugins}/`, `$XDG_CONFIG_HOME/muse/{.settings.json.lock,
.auth.json.lock,skills/.muse/}` and the session-name-authority dir; the R5 e2e excludes exactly those
paths and asserts everything else under both roots byte- AND mode-identical (`fsx::write_atomic`
preserves an existing file's mode; a new file gets `0666 & !umask`). A profile switch restores every
leaf the outgoing profile introduced (settings-key registrations carry a `profile` tag) before the
incoming slice lands, in one transaction.

## 5. `omm-manifest` — content → packages

### 5.1 Canonical source and the catalog (R8)

`content/catalog.json` is the single list of assets:

```jsonc
{ "schema_version": 1,
  "assets": [
    { "id": "omm-plan", "kind": "skill", "lifecycle": "active", "core": true,
      "canonical": null, "since": "0.1.0", "sunset": null, "budget_bytes": 240 }
  ] }
```
lifecycle ∈ `active | alias | merged | deprecated | internal`; `canonical` forwards an alias; `core`
items cannot be deactivated (build gate). Nothing in Rust knows an asset name.

### 5.2 Generators

`omm build` (dev-only subcommand) reads `content/` + `catalog.json` and writes:

- `plugins/oh-my-musecode/.muse-plugin/plugin.json` — **native** family. `schemaVersion:1`, `name:"oh-my-musecode"`,
  `version`, `description` (required non-empty), `capabilities:{skills,commands,hooks,mcpServers,
  reminders}`. Per-family strictness: `hooks`/`reminders` are CLOSED (unknown key = hard error);
  `skills`/`commands`/`mcpServers` are OPEN (silently accepted) — so the generator itself must
  reject `env`/`cwd`/`headers` on `mcpServers` (they validate and are then dropped at launch).
  `enabledDefault` as a real JSON boolean. Every hook carries `command` only (the argv array
  `["omm","hook","<name>"]`, R16); `commandWindows` is rejected by the native family
  (`unsupported-field`) and may appear only in user/project-tier `hooks.json` documents.
- `dist/claude/.claude-plugin/plugin.json` and `dist/codex/.codex-plugin/plugin.json` — projections.
  **A package holds exactly ONE manifest dir**, so these are separate package trees, not extra
  dirs in `plugins/omm/`. Both are committed.
- Three catalogs at the repo root (docs/experiments/marketplace-precedence.md §6 — measured):
  - `marketplace.json` — the undocumented NATIVE Muse catalog, probed FIRST; Muse reads exactly one
    file and stops. Shape: `{"schemaVersion":1,"source":"local","plugins":[{"name":"oh-my-musecode","version":…,
    "install":{"transport":"local-path","source":"plugins/omm"},"integrity":{"digest":"<package_sha256>"},
    "availability":{"status":"available"}}]}`. The digest is **obtained from the binary** after the package
    is final (`plugins validate --json` / `list --available --json`) — never hand-computed, never copied
    from a previous release.
  - `.agents/plugins/marketplace.json` — Codex schema; entry `source` is an OBJECT
    `{"source":"local","path":"dist/codex"}`; a string source is skipped.
  - `.claude-plugin/marketplace.json` — Claude schema; marketplace `name` must be `omm` (so
    `oh-my-musecode@omm` works in every tool); entry `source` is a relative STRING `"./dist/claude"`; an object
    source is treated as remote and skipped under a local-dir marketplace.
  - Family is decided by the PACKAGE's manifest dir, not by the catalog; `name` in every entry must equal
    the manifest `name` (a mismatch only fails at install time).

CI: regenerate and `git diff --exit-code` (version normalised so a bump alone does not fail); then in a
temp `XDG_DATA_HOME`: `marketplace add ci <repo-root> --json` → `plugin_count 1, skipped []` →
`install omm@ci --json` → `manifest_family == "native"` and `package_sha256 == digest`.

### 5.3 Lint (R11, R12, R13, R19)

- id grammar `^[a-z0-9][a-z0-9._-]{0,79}$`; `omm-` prefix; collision against the reserved ids
  (`skill-reminder goal-reminder memory-reminder todo-reminder verify-reminder scope-reminder
  tbh-reminders loop muse-core`), the 15 bundled skill ids, the 39 built-in slash commands (all
  loaded from `host-reality.md` data, not code);
- `len("oh-my-musecode") + len(server-id) ≤ 18` for every MCP server (server id stays ≤ 4 chars; the host sanitizes `-` to `_` on the wire);
- no symlink anywhere; no backslash in any filename; manifest ≤ 131,072 B; ≤ 4,096 fs entries;
  path depth ≤ 16; inventory units ≤ 4,094 (1/class + 2/skill + 1/each other);
- SKILL.md: no BOM, `name` == directory id, description ≤ 240 chars, one line, with a negative-trigger
  clause (`Do not use …`) joined to the first sentence with `;` so it survives `first_sentence`;
  body (bytes after the front matter) ≤ **8,192 B** (8 KiB — the lint constant; overflow goes to
  `references/*.md` beside the skill);
- foreign tool vocabulary: a backticked foreign tool name (`Task` `TodoWrite` `Read` `Write` `Edit`
  `Glob` `Grep` `AskUserQuestion`) or the phrase `the <Name> tool` in a skill/command/rules body fails,
  except in assets that map those names to Muse's: the `translation` and `rules` kinds by kind, and
  any asset the catalog flags `"foreign_vocab": true` (the skill-port skill) — the allowlist is
  catalog data, never a path in Rust (R8); bare prose verbs (`Read everything`, `Write it`) are
  not matched; the foreign-product-name grep of `crates/omm-host/tests/repo_lint.rs` (the two
  product names it assembles, plus their bare first words as whole words) runs over `content/` too,
  with `.claude-plugin` / `.codex-plugin` / `CLAUDE.md` / `.claude/` path literals and
  `muse skills import --from claude|codex` exempt;
- catalog byte estimate ≤ 21,542 B (R18) using entry cost
  `38 + len(display_id) + len(display_path) [+ len(desc) + 36]`;
- then the three host checkpoints (R13) against the real binary.

### 5.4 Overlay resolution (R7)

Precedence, first-hit-wins per id, published as a table:

```
1  $XDG_CONFIG_HOME/omm/custom/<kind>/<id>       (user overlay — REPLACE)
2  bundled content
+  $XDG_CONFIG_HOME/omm/custom/<kind>/<id>_append.md  (APPEND, resolved independently)
−  config.json → "disabled": ["skill:omm-pdf", "hook:omm-verify"]   (capability-qualified ids)
```
Shadowed items are retained with `_shadowed:true` so `omm list --json` can explain what is hiding what.

## 6. `omm-doctor` — checks and cost

Every check: `id`, `severity`, `what was observed`, `why it is silent at session time`, and the
**exact fix command**. Exit non-zero on any `critical`.

| # | Check | Detects | Fix it prints |
|---|---|---|---|
| D1 | plugin capabilities | every declared `mcp_server`/`hook`/`reminder` line PRESENT in `plugins inspect --json` and literally `trusted_enabled` (`review_needed`/`trusted_disabled`/`modified`/missing are all silent at runtime) | `muse plugins approve plugin:omm:<kind>:<id>` or `omm install` |
| D2 | `settings.provider` | unset → `muse serve`/SDK gets a one-tool session and no MCP. A pristine install never sets it (the host's login flow does), so unset is **Info** naming what breaks and the command; **Warn** only when the ledger shows omm wrote the key and it is gone. "Green" = zero Warn/Critical; Info rows are allowed | `omm settings set provider meta` |
| D3 | mcp key collision | `mcpServers` + `mcp_servers` both present → every settings-writing Muse command exits 1 | `omm settings fix-mcp-collision` |
| D4 | shape lint | malformed `settings.plugins` / `settings.runtime_capabilities` — one bad entry takes both wired reminders down; a `permissions` object lacking its required `schema_version` (settings-keys.json `structural`) — the host refuses the whole object, every named profile silently inert | `omm settings lint --fix`; `omm settings set permissions.schema_version 1` |
| D5 | reminder conflict | canonical vs legacy reminder enablement disagree | `omm settings reconcile-reminders` |
| D6 | safe_mode | `agent_definitions.safe_mode: true` → user/project/plugin agent packs never load | `omm settings set agent_definitions.safe_mode false` |
| D7 | plugin count | enabled plugins vs 256 (warn at 200) | `muse plugins disable <id>` |
| D8 | catalog pressure | entries / entries-with-description / bytes-of-32000 from a real `muse exec --provider echo hi` + `session.jsonl`; stage-2 silent description drops; under `--fast` an Info row saying the session was skipped, never a Warn | `muse skills disable bundled:<id> --scope built-in` / `omm cost` |
| D9 | enterprise probe | `muse config status` rows | informational |
| D10 | manifest family | installed `omm` is `manifest_family == "native"` | `omm install --reinstall` |
| D11 | host drift | golden constants vs live probes (`--self-test` = the P0 fixture set) | `omm doctor --report-drift` |
| D12 | trust | workspace not in `trust.json` → project skills/hooks/rules/workflows silently inert | `omm trust .` |
| D13 | ledger integrity | corrupt/missing ledger, entries whose file vanished, files present-but-unlisted under our roots; names settings keys / trust entries that are current but unregistered. With the ledger corrupt or absent the install mode is read from the host (plugin `omm` installed → bundle; `omm-*` skills in the managed store and no plugin → `--no-plugin`), and D1/D10/D13 spell every fix for that mode (Gate 1 decision E) | `omm reconcile` (drops / adopts — managed-store skills identical to the source included), then `omm install [--no-plugin]` when a rebuilt ledger would still miss files; `omm install [--no-plugin]` alone for unlisted files |
| D14 | argv self-test | exit-code matrix + allowlist sanity | — |
| D15 | omm on PATH | `command[0]` of every plugin hook and MCP server (read from `plugins inspect --json`, never the catalog) resolves on the `PATH` the host inherits; a missing one silently never spawns — no stderr, no session record, the namespace simply absent from `tools[]`. Critical when missing; Warn when a different `omm` is first on `PATH` than the one running doctor; Info in `--no-plugin` mode | `export PATH="<dir of current omm>:$PATH"   # then add that line to your shell profile` (`install <word> or add its directory to PATH` for a non-omm word) |
| D16 | skill routing | appended by the CLI (`c_tune/routing_doctor.rs`). Off: Info. On: the workspace's trust, the handler present in `<ws>/.muse/hooks.json` and its program word resolving, the library files regular, and the combined `order200 + order201` headroom against the live order-200 size (the recorded size under `--fast`) — a stale measurement or a cap within the margin is a Warn | `omm enable skill-routing` · `omm trust <ws>` · `omm settings set run.context_slimming.skill_catalog_descriptions first_sentence` |

Doctor runs D1–D15 in `omm-doctor` and prints "16 checks: …" with D16 appended; `--fast` skips D8 and
D11 (0.25 s measured against 3.7 s).

**Cost** (`omm cost`): the per-source byte table for the skills catalog (bundled / filesystem /
plugin, in that render order — plugin is starved first), the refundable built-in tax (10,060 B,
e.g. `bundled:browser-app-delivery` = 1,912 B), memory (16,305 B / 48 files), rules
(256,000 B/file, 65,536 B aggregate), the 18,056 B `workflow_cookbook` and how to drop it
(`run.workflow_trigger_mode: "off"`), and estimated tokens (bytes/4 as the documented heuristic,
labelled as such).

## 7. `omm` CLI

| Command | Does | Writes |
|---|---|---|
| `omm install [--no-plugin] [--skip <step>…] [--drop-unknown-settings-keys] [--yes] [--dry-run]` | **Plan, then execute** (Gate 1 decision A): the COMPLETE plan is built and validated before the first host mutation — every file write with its resolved realpath and containment verdict (R4), every settings key, every host mutation, the trust entry, the host reachable (`muse --version`), the paths writable, the free space — and a containment failure (a `themes/` symlinked out of the config root by a dotfile manager) is refused up front with the exact path and the way out, nothing written: `--skip themes` (or `rules`, `profile`, `trust`) leaves the step out, `OMM_THEMES_DIR=<dir>` (inside the config root) places the themes elsewhere; the same for a `--no-plugin` store that escapes. Then the default sequence (measured, experiments/marketplace-precedence §6.4): `plugins marketplace add omm <src> --json` (on "already configured" → `marketplace update omm`) → `plugins list --available --json` (assert `oh-my-musecode` `status: "available"`, keep `digest`) → `plugins install oh-my-musecode@omm --json` (assert `manifest_family == "native"` and `package_sha256 == digest`) → approve every capability → verify `trusted_enabled` via `inspect --json` → skills cross-check via `skills list --json` → untyped keys and the recorded mode of `settings.json` back. Ledger registrations: marketplace `{name, source}` and plugin `{id, package_sha256, generation path}`. `--no-plugin`: `muse skills install` per skill (ledger saved after each). Both: `AGENTS.md`, default profile, themes, trust for cwd if a workspace. The `--json` document carries the plan (`plan`, `skipped_steps`). Prints the token budget consumed. Idempotent. Refuses under `CI=true`/non-TTY without `--yes` (R6). | ledger + audit |
| `omm uninstall [--dry-run] [--force] [--reconcile-host]` | R4. Two-section preview plus `! beyond the ledger (host has it, ledger does not)`. Names kept residue. Restores settings keys to `prior` (while they still hold what omm wrote; a structural member the user's data needs is kept and named); shared files back to their pre-omm bytes and mode (§4). Always probes the host first; a registration the host no longer holds is dropped as already gone (idempotent undo, §4). `--reconcile-host`: remove the host's `oh-my-musecode`/`omm` registrations and managed-store catalog skills the ledger does not list (an interrupted install; with no ledger, all of them); without it those are named and the uninstall refuses, naming the flag. | ledger removed last |
| `omm update [--dry-run]` | snapshot → reconcile (R3) → `plugins update` + re-approve atomically → report staged conflicts | ledger + audit + snapshots |
| `omm doctor [--fast] [--self-test] [--report-drift] [--json]` | §6: D1–D15 plus D16; `--fast` skips the live echo session (D8) and the host self-test (D11); `--report-drift` prints the golden-constant diff table and exits 1 on drift; exit 1 on any critical row | nothing |
| `omm cost [--no-cuts] [--json]` | §6; `--no-cuts` skips the four extra echo sessions that measure the "what to cut" savings | nothing (runs one echo session in a temp XDG_DATA_HOME) |
| `omm reconcile` | D13's fix: re-check every ledger entry against the disk and the host, drop what vanished, adopt what the host holds identical to the source | ledger + audit |
| `omm lint [path] [--no-host]` | §5.3 | nothing |
| `omm build [path] [--check]` | §5.2 (dev); `--check` regenerates in memory and exits 1 on drift | repo files |
| `omm mcp` | the stdio MCP server the bundle declares (`content/mcp/doc.json` → `capabilities.mcpServers[{id:"doc",command:["omm","mcp"]}]`; wire namespace `mcp__plugin_oh_my_musecode_doc`, the host sanitizes `-` to `_`): tools `omm_doctor {fast?}` and `omm_cost` return the `--json` documents as one `{type:"text"}` part; line-delimited JSON-RPC 2.0 with `Content-Length` framing mirrored, protocol `2024-11-05`, frames bounded at 1 MiB, tool-level problems as `isError:true`, protocol-level as JSON-RPC errors, EOF → exit 0. Spawned by the host with the 16-key scrubbed env and cwd = workspace: roots come from `HOME`, the binary from the launcher dir (`~/.local/bin/.muse-version`) or `muse` on `PATH` — never `OMM_MUSE_BIN`. Creates `$MUSE_PLUGIN_DATA_DIR` itself | `$MUSE_PLUGIN_DATA_DIR/omm-mcp.log` only (one line per frame, never a result body, truncated past 1 MiB; removed by `plugins remove oh-my-musecode --delete-data`) |
| `omm trust [path]` | merge into `trust.json` | ledger |
| `omm theme <name>` / `omm keymap <preset>` / `omm profile use <name> [--force]` | targeted settings patches, validated first; a key the user edited since a profile set it is left as is and named (`left_edited`), on the way in as on the way out, unless `--force`; `list` on each reads only, `profile show <name>` diffs a slice against `settings.json` | ledger |
| `omm settings set <key> <value>` / `fix-mcp-collision` / `lint [--fix]` / `reconcile-reminders` | the D2–D6 fixes as targeted, validated patches; `lint` without `--fix` reads only and exits 1 on problems | ledger |
| `omm memory {path,seed,list,backup,gc}` | the host's `personal_project` root `$XDG_DATA_HOME/muse/memory/projects/<slug96>-<fnv1a64hex>/` of the cwd. `path` prints root / workspace / exists / trust; `seed` creates `MEMORY.md` from `SEED_TEMPLATE` only when missing (identical bytes adopted, any other content skipped, never overwritten) plus the sidecar `omm-workspace.json` `{schema_version:1, workspace, writer}`, warns when the workspace is not `trusted` and prints `omm trust <ws>`, refuses a symlinked root; `list` maps every root back to a workspace via the sidecar / `trust.json` / the cwd ("slug only" otherwise — never a guess), counts files and bytes against the silent 48 / 16,305 B caps and warns on slug collisions; `backup [--to <DIR>]` writes a SHA-256-verified ustar of the root (built in memory, atomic) under `$OMM/snapshots/memory/<ts>/` (rolling 5) or into `<DIR>`; `gc` removes a root only when its sidecar names an absolute workspace whose formula reproduces the directory name AND the workspace is `NotFound`, after a tar of it. Two `muse-data` ledger entries per seeded root (`MEMORY.md` seeded, the sidecar exclusive); uninstall removes `MEMORY.md` only while byte-identical to the seed | ledger + audit + `$OMM/snapshots/memory/` |
| `omm enable skill-routing` / `omm disable skill-routing` | M4, opt-in only; cwd = the root of a git workspace `trusted` in `trust.json` (exit 2 and `omm trust .` otherwise). `enable` measures order 200 for the workspace, copies `content/routing/library/<id>/SKILL.md` to `<ws>/.omm/skills/<id>/` (R3-reconciled `workspace` entries), writes `<ws>/.omm/routing.json` and the `omm hook route` handler (`outputCapabilities:["skills.v1"]`) into `<ws>/.muse/hooks.json` (created → exclusive; pre-existing → `hooks-merge` with the pre-omm bytes and mode in `prior.original`), and records `skill_routing: true` + `routing{…}` in `$OMM/config.json`; `omm run` then exports `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1` and `…_APPLY=1`. `disable` reverses it byte for byte (an edited library file is kept and named). Budget `order200 + order201 ≤ 31,984 B`; docs/ROUTING.md | ledger + audit + snapshots |
| `omm run -- <muse args>` | the launcher shim: allowlist check (R20), `MUSE_NO_AUTO_UPDATE=1`, profile gate bundle, routing gates if enabled, then `exec` | nothing |
| `omm hook <name>` | in-binary hook dispatcher: reads the JSON event on stdin (4 MiB bound), runs the handler chain, writes JSON, sub-5 ms, fail-open (R16). The guard alone denies a payload it cannot evaluate — cut at the bound, malformed, empty — with the reason `event too large or malformed to evaluate`, still exit 0 (Gate 1 decision G); its rule table is `content/hooks/README.md`, a heuristic, never a security boundary | nothing (unless the hook does) |
| `omm list [--json]` | resolved assets with provenance and `_shadowed` | nothing |

Global: `--json` on every read command; `--dry-run` on every write command; every write prints a
per-category converge summary (updated / unchanged / skipped / backed-up / removed).

## 8. Muse facts the code depends on (summary — full table in `docs/host-reality.md`)

- Plugin capability families: exactly `skills, commands, hooks, mcpServers, reminders`.
  `tools/agents/outputStyles/settings/apps` are hard errors in a native manifest.
- Approval is non-interactive: `muse plugins approve plugin:<pid>:<kind>:<cap-id> --json`.
- `muse plugins remove <id> [--delete-data]` exists and strips `runtime_capabilities`.
- MCP tools reach the model as a namespace group `{"type":"namespace","name":"mcp__plugin_<pid>_<sid>",
  "tools":[…]}`; the model calls `<ns>.<tool>` (every length) or `<ns>__<tool>` (only while the
  namespace is unrewritten). `len(pid)+len(sid) > 18` rewrites the namespace and breaks the `__` form —
  the 18-char rule is functional. Proven end to end in `exec` and `serve` lanes with
  `tools/mockprovider/run-mcp-toolcall.sh` (docs/experiments/mcp-tools-call.md).
- Marketplace probing is first-found-wins: root `marketplace.json` (native) → `.agents/plugins/
  marketplace.json` → `.claude-plugin/marketplace.json`. `marketplace update` creates a new generation
  (rolling keep-2); an installed plugin is pinned to a generation.
- `MUSE_EXPERIMENTAL_PLUGINS=1` is needed for `plugins` verbs only; runtime composition is ungated.
- Hook events (17, PascalCase only): `SessionStart UserPromptSubmit PreToolUse PermissionRequest
  PostToolUse PostToolUseFailure PreCompact SubagentStop Stop StopFailure Notification SessionEnd …`
  (full list + per-event decision support in `research/musecode/hooks.md`).
- Hook `command` runs via `$SHELL -c` with a 16-key scrubbed env; `cwd` is the workspace.
- Skills catalog: 32,000 B cap; 363 B header + 35 B footer; render order bundled → filesystem →
  plugin; three-stage degradation, stage 2 (descriptions dropped) is silent.
- Personal rules: exactly one file loads — `$CONFIG_DIR/AGENTS.md` first.
- `settings.json`: 29 typed keys, `schema_version:1` required, rewritten by Muse, unknown keys destroyed.
- `trust.json` gates project skills/hooks/rules/workflows/agents/plugin installs; nothing but the TUI
  writes it; hand-writing works.
