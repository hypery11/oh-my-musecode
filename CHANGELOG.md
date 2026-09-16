# Changelog

All notable changes to oh-my-musecode. The format follows Keep a Changelog; versions follow SemVer.
Every host fact named here was measured against Meta Muse Code `1.0.1-R2006.1` (muse-stable),
`1.1.0-R2009.1` (muse-canary) and, since 1.0.0, `1.3.0-R3057.1`; nothing gates on a version number
(ARCHITECTURE R15).

## [1.0.0] — 2026-09-16

The 1.0.0 release. Host pins refreshed to Muse `1.3.0-R3057.1`, and this release
replaces the 0.x plugin surface (Node CLI, Python hooks, role skills) instead of
extending it — upgraders read `docs/MIGRATION.md` first.

### Added

**Host re-probe (1.3.0)** — every pin re-measured against `1.3.0-R3057.1`: PreCompact
payload proven (`trigger` soft/hard-style, no `reason`), SubagentStart payload proven
(nine keys, no `agent_id`/`agent_type`), SubagentStop identity keys plausible by
symmetry, unconfirmed. A live crash probe (PreCompact handler exits 1) records
`hook_run_terminal` `status: failed` while the host still evaluates the compaction
candidate and ends the session clean — fail-open proven, not assumed.

**Hooks: eight, all observed** — `pre-compact` observer (logs trigger and transcript
size, never vetoes), `subagent-start`/`subagent-stop` team log (ids plus outcome when
the host reports one), `skill-gate`/`intent-gate` absorbed as in-binary fail-open
handlers, and a literal-text write jail around the guard's `cd` handling. Locked by
`crates/omm/tests/hook_gates.rs` and the hook suites.

**Community files** — `CONTRIBUTING.md`, `SECURITY.md`, `SUPPORT.md`, `AGENTS.md`,
`ROADMAP.md`, `README.zh-TW.md`, issue/PR templates, CoC, `docs/MIGRATION.md`,
`scripts/check-manifest.py` (manifest, frontmatter, and Cargo-version agreement
without a Muse binary).

**Skills** — 23 skills join the 12 of 0.1.0 (35 in the bundle, every one with a `; Do not use …`
clause that survives `first_sentence`): `omm-pr`, `omm-perf`, `omm-migrate`, `omm-ci-fix`,
`omm-release`, `omm-incident`, `omm-api-design`, `omm-errors`, `omm-observability`, `omm-config`,
`omm-concurrency`, `omm-shell`, `omm-rust`, `omm-typescript`, `omm-python`, `omm-database`,
`omm-frontend`, `omm-test-design`, `omm-legacy`, `omm-repo-map`, `omm-cleanup`, `omm-handoff`,
`omm-containers`. Measured on `1.0.3-R2198.1`: the order-200 `skills_catalog` block is 23,722 B in
`full` (plugin share 13,264 B of the 21,542 B room) and 18,028 B under the default profile
(13,196 B of 27,168 B), 35/35 entries with their description in both modes
(`content/VALIDATION.md` §10).

**Host tracking** — Muse stable `1.0.3-R2198.1` is the pinned build (`1.0.1-R2006.1` stays a CI
fixture). The release added exactly one experimental gate, `ultra_reasoning_effort` (default off; it
now gates the `ultra` effort's two extra context blocks, which `1.0.1` composed unconditionally), and
a `max` value for `--reasoning-effort`; the other 33 measured axes are byte-identical
(`docs/experiments/stable-1.0.3-diff.md`). Host facts in `docs/host-data/*.json` can now carry a
`since: <build>` tag: `omm doctor --self-test` reports a tagged fact that probing finds absent on an
older build as `OLDER-BUILD` (a pass with a note) instead of a failure, while an unlisted gate or a
missing untagged fact still fails — planting a fake gate and removing the tag were both verified to
turn the self-test red. `gates.json` also carries an `effect_probe` for the new gate, measured live
in all three states (closed, open, absent).

**Hook kill switch** — `config.json → "disabled": ["hook:omm-guard"]` is now read on the dispatch
path (`hook:guard` is accepted too), so a documented switch actually switches (previously documented
but inert — the anti-pattern the ecosystem survey found in a rival). Locked by
`crates/omm/tests/hook_fail_open.rs`.

### Changed

- `omm-refactor` no longer triggers on "clean up" (that is `omm-cleanup`'s) and `omm-commit` no
  longer triggers on "make a PR" (that is `omm-pr`'s); both name the new owner in their negative
  clause.
- No description names a routed skill any more: `omm-test-triage`, `omm-flaky-test`,
  `omm-dep-upgrade` and `omm-pr-description` exist only after `omm enable skill-routing`, so the
  skills that pointed at them describe the action instead.
- The CI skill ships as `omm-ci-fix`, not `omm-ci`: catalog ids are unique across kinds and
  `omm-ci` is the CI permission profile (`profiles/ci.json`, `permissions.profiles.omm-ci`).

## [0.1.0] — 2026-09-03

The first release: five crates, one static `omm` binary, the content bundle, the marketplace, the
install scripts and the documentation.

### Added

**Host layer (`omm-host`)** — the Muse binary as a dependency: locate (`$OMM_MUSE_BIN` → the
launcher's `.muse-version` → `muse` on `PATH`), a controlled invoker (`MUSE_NO_AUTO_UPDATE=1`,
`NO_COLOR=1`, `MUSE_EXPERIMENTAL_PLUGINS=1` for `plugins` verbs only, per-call timeouts that kill the
whole process tree and sweep the host's runtime-dir residue), the R20 argv allowlist with the
root-flag walk, both config/data roots and the `personal_project` memory formula, behavioural probes
(`skills list`, `plugins inspect`, gate resolution from the bootstrap trace), the typed
`settings.json` patcher (validate with the host, atomic rename on the realpath, verified backup),
the `trust.json` merge, and the golden constants of `docs/host-reality.md` with the `hostcheck`
self-test (62 P0/P1 rows).

**Ledger (`omm-ledger`)** — `omm.lock.json` with base-relative paths, `.bad-<ts>` quarantine of a
corrupt file, sorted entries, the non-regular sentinel, the R3 three-way reconcile (no-op /
overwrite / adopt / stage), rolling snapshots (keep 5), the JSONL audit log, the marked-region
logic of `AGENTS.md` (omm owns only the managed block), pre-omm bytes and mode of the shared files,
and the R4 uninstall planner: two-section preview, deepest-first, `canonicalize` + `strip_prefix`
containment on four allowlisted bases, refusal of `/`, `$HOME` and every base root.

**Content model (`omm-manifest`)** — `content/catalog.json` as the only asset list; generators for
the native package (`plugins/omm/.muse-plugin/plugin.json`), the `.claude-plugin` / `.codex-plugin`
projections under `dist/` and the three marketplace catalogs (the root native `marketplace.json`
with the digest obtained from the binary, plus the two foreign projections); the lint (id grammar,
`omm-` prefix, collisions with the 9 reserved ids / 15 bundled skills / 39 slash commands, the
18-character MCP rule, symlinks, backslashes, manifest and inventory budgets, SKILL.md rules,
foreign tool vocabulary, the catalog byte estimate) and the three host checkpoints; the R7 overlay
resolution with `_shadowed`; the `omm build --check` drift gate.

**Doctor and cost (`omm-doctor`)** — checks D1–D15, each with what was observed, why it is silent
at session time and the exact fix; D8 and `omm cost` measure a real `muse exec --provider echo`
session in a throwaway data root seeded with the host's plugin store; `omm cost` prints the
per-source catalog table, the built-in tax, memory, rules, the workflow cookbook and the measured
savings of every cut.

**CLI (`omm`)** — `install` (plan-then-execute, bundle or `--no-plugin`, R6 consent, R14
approve-and-verify, containment refusals up front, `--skip`, `--drop-unknown-settings-keys`,
`--reinstall`), `uninstall` (`--force`, `--reconcile-host`, idempotent host undo, blind undo over a
malformed host config), `update` (snapshot → reconcile → reinstall → re-approve), `reconcile`,
`trust`, `list`, `doctor` (`--fast`, `--self-test`, `--report-drift`), `cost` (`--no-cuts`), `lint`,
`build`, `theme`, `keymap`, `profile use|list|show`, `settings set|fix-mcp-collision|lint|
reconcile-reminders`, `run` (the launcher shim), `hook` (the in-binary dispatcher: sub-5 ms,
fail-open, a 4 MiB stdin bound, the destructive-command guard).

**`omm mcp`** — the stdio MCP server inside the binary: line-delimited JSON-RPC 2.0
(`Content-Length` framing mirrored), protocol `2024-11-05`, tools `omm_doctor {fast?}` and
`omm_cost` returning the `--json` documents, frames bounded at 1 MiB, a bounded trace in
`$MUSE_PLUGIN_DATA_DIR/omm-mcp.log`, exit 0 at EOF. Declared by the bundle as the server `doctor`
(namespace `mcp__plugin_omm_doctor`); doctor D15 checks that `omm` resolves on the host's `PATH`;
the mock-provider harness `tools/mockprovider/run-omm-mcp.sh` proves model → `tools/call` → doctor
JSON end to end.

**`omm memory path|seed|list|backup|gc`** — the host's `personal_project` root: a seeded
`MEMORY.md` (only when missing) and an `omm-workspace.json` sidecar mapping the root back to its
workspace, both ledgered; a list with the silent caps (48 files / 16,305 B); ustar backups under
`$OMM/snapshots/memory/` (SHA-256-verified, rolling 5); a gc that removes only sidecar-mapped roots
whose workspace is gone, after a backup.

**Skill routing (opt-in)** — `omm enable skill-routing` / `omm disable skill-routing` in a trusted
git workspace: the routed library (seven skills under `content/routing/library/`, invisible at order
200) copied to `<ws>/.omm/skills/`, the `omm hook route` router installed as a `UserPromptSubmit`
handler declaring `skills.v1` in `<ws>/.muse/hooks.json` (merged into an existing file, restored
byte for byte on disable), the measured `order200 + order201 ≤ 31,984 B` budget, doctor D16, the two
experimental gates exported by `omm run`. `docs/ROUTING.md` documents it; a real echo session and
the mock-provider `read_skill` lane prove it.

**Content** — 12 skills (`omm-self`, `omm-reflect`, `omm-tdd`, `omm-debug`, `omm-review`,
`omm-commit`, `omm-refactor`, `omm-security`, `omm-verify`, `omm-parallel`, `omm-skill-port`,
`omm-docs`), 3 commands (`/omm-doctor`, `/omm-cost`, `/omm-status`), 3 hooks (`omm-session-start`,
`omm-guard`, `omm-stop`), the `omm-verify-nudge` reminder (shipped disabled), the `doctor` MCP
server, 3 themes (`omm-carbon`, `omm-slate`, `omm-paper`), 4 profiles (`strict`, `default`, `fast`,
`ci`), the `AGENTS.md` rules template and the tool-vocabulary translation block. Catalog cost of the
bundle: 4,695 B full / 4,627 B `first_sentence` of the 21,542 B budget, measured.

**Scripts and CI** — `scripts/fetch-host.sh` (channel → manifest → size- and SHA-256-verified
binary under `.host/bin/`, idempotent, offline re-verification), `scripts/release.sh` (bump →
regenerate → gate → per-target tarballs → `SHA256SUMS`; never tags or pushes), `install.sh` (POSIX
`sh`, verifies against `SHA256SUMS`, places the binary, `--modify-path`), `scripts/gate.sh`, and
`.github/workflows/ci.yml` (the gate on macOS and Linux; a daily channel poll that re-measures the
host tables, R15).

**Documentation** — `docs/ARCHITECTURE.md` (R1–R22 and the crate contracts), `docs/PLAN.md`,
`docs/host-reality.md` (fact → constant → test), `docs/experiments/` (context slimming, marketplace
precedence, MCP `tools/call`), `docs/ROUTING.md`, `docs/KNOWN_ISSUES.md`, `docs/INSTALL_FOR_AGENTS.md`,
`scripts/README.md`, `tools/mockprovider/README.md`.

### Verified

- Gate 0: adversarial review of `omm-host` (allowlist walk, atomic temp files, snapshot backups,
  dangling symlinks, trust shape, residue root, gate probe retry) closed.
- Gate 1 (five rounds): plan-then-execute install; idempotent host undo with a host probe; in-base
  ancestor symlink sentinel; structural settings keys; install mode detection without a ledger;
  settings mode preserved across every host rewrite; managed-block-only `AGENTS.md` ownership;
  linked store directories never handed to the host; provenance-missing recovery; ledger entry
  saved before the write; host config validity in the install plan. 30 end-to-end scenarios in
  `crates/omm/tests/e2e.rs`, each in a fresh `HOME` against the pinned host, including R5
  (install → uninstall → byte- and mode-identical) for both install paths.
- Phase 2/3 proofs: `crates/omm/tests/mcp_server.rs` (the real pipe, the mock-provider harness),
  `crates/omm/tests/routing.rs` (a routed prompt in a real session; a merged hooks file restored
  byte for byte; the router under 5 ms), the memory seed rendered by the host at order `u32::MAX`.

### Known issues

Ten findings that do not block the gate, each with a reproduction and the owning file:
`docs/KNOWN_ISSUES.md`.
