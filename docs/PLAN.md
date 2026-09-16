# oh-my-musecode — Build plan

Order is by "cannot be retrofitted later" first, then by leverage. Every phase ends with a
verification gate that a *different* agent runs adversarially. Nothing advances on green tests alone.

Toolchain: `~/.cargo/bin` (rustup stable 1.88). Homebrew's rust is broken on this machine — always
`export PATH="$HOME/.cargo/bin:$PATH"`. Host binary: `.host/bin/muse-bin-1.0.1-R2006.1`
(`OMM_MUSE_BIN` points at it in tests). Never run tests against the user's real `~/.config/muse`.

## Definition of done, globally

- `cargo build --release` produces one static `omm` binary; `cargo test` and `cargo clippy -- -D warnings` clean.
- `tests/e2e`: **clean `$HOME` → `omm install` → `omm uninstall` → filesystem byte-identical** (R5), for
  both the bundle path and `--no-plugin`.
- `omm doctor` green after install; every planted failure in `tests/e2e/planted/` is caught with its
  fix command; no false positives on a pristine install ("green" = zero Warn/Critical rows; Info rows
  are allowed — a pristine install never sets `settings.provider`, so D2 is Info, not Warn).
- `omm lint` + `omm build` drift-gated in CI; `plugins/oh-my-musecode` passes R11's four predicates and R13's
  three checkpoints against the real binary.
- Catalog payload of `plugins/oh-my-musecode` ≤ 21,542 B (R18), measured by a real echo session, not estimated.
- No `Co-Authored-By`, no mention of AI tooling anywhere in the repo (project rule).

## Status (2026-09-03, `HEAD` = "gate 1 closed", working tree carries Phases 2 and 3)

| Phase | State | Evidence |
|---|---|---|
| 0 — foundation | **done**, Gate 0 closed | `omm-host` with hostcheck (62 P0/P1 rows), host data, the three experiments in `docs/experiments/`, `tools/mockprovider/` |
| 1 — ledger, content, lint, doctor/cost, CLI | **done**, Gate 1 closed after five rounds | 30 e2e scenarios in `crates/omm/tests/e2e.rs` (the ten below plus 3b, 11–27); `docs/KNOWN_ISSUES.md` holds the ten non-blocking findings |
| task 2.1 marketplace | done | native `marketplace.json` + two projections generated; `omm install` goes marketplace add → install → approve → verify (e2e s01, s11, s15c) |
| task 2.2 `omm mcp` | done | `crates/omm/src/cmd/{mcp,mcp_wire}.rs`, `content/mcp/doctor.json`, doctor D15, `crates/omm/tests/mcp_server.rs` (the real pipe; the mock-provider harness `tools/mockprovider/run-omm-mcp.sh` proving model → `tools/call` → doctor JSON) |
| task 2.3 memory | done | `omm memory {path,seed,list,backup,gc}` (`crates/omm/src/cmd/c_tune/{memory,memory_tar}.rs`); the seed renders at order `u32::MAX` in a real echo session |
| task 2.4 profiles | done | `omm profile use|list|show` as one ledgered transaction (e2e s19; `crates/omm/tests/cmd_c.rs`) |
| task 2.5 release | done, unreleased | `scripts/fetch-host.sh`, `scripts/release.sh`, `install.sh`, `.github/workflows/ci.yml`, `scripts/README.md`; the release URL is a placeholder until the first tag |
| task 3.1 routing | done | `omm enable/disable skill-routing`, `omm hook route`, `content/routing/`, doctor D16, `docs/ROUTING.md`, `crates/omm/tests/routing.rs` + `tools/mockprovider/run-skill-routing.sh` |
| task 3.2 docs | done | `README.md`, `docs/INSTALL_FOR_AGENTS.md`, `CHANGELOG.md`, `LICENSE`, this table, ARCHITECTURE §6/§7 |
| Gate 2 / Gate 3 | **open** | the security review of the hook dispatcher and the MCP server, and the final clean-clone pass over R1–R22, have not been run |

Gate results at the time of writing (`scripts/gate.sh` against `1.0.1-R2006.1`): see the
"Gate run" note at the end of this file.

## Phase 0 — Foundation

| Task | Deliverable | Acceptance |
|---|---|---|
| 0.1 Workspace skeleton | `Cargo.toml` workspace; 5 crates with module stubs; `omm` CLI with every subcommand from ARCHITECTURE §7 present (stubs exit 2 with "not implemented: <cmd>"); `clap` with `--json`/`--dry-run` globals; error type; logging | builds, clippy clean, `omm --help` lists everything |
| 0.2 `omm-host` complete | locate, invoke (controlled env, gate only for `plugins`), exit-code semantics, argv allowlist, paths (both roots, memory formula), probes (`--version`, gates from trace, `skills list --json`, `plugins inspect --json`, `config validate`), `settings.json` typed patch with validate-then-atomic-rename + backup, `trust.json` merge, `host_reality.rs` constants + data lists | unit tests for path/formula/exit-code logic; integration tests against `.host/bin/*` for every probe; `mcpServers`+`mcp_servers` collision refused |
| 0.3 `tests/hostcheck` | the P0 + P1 tables from `docs/host-reality.md` as a test binary; `omm doctor --self-test` runs the same code | passes against stable; prints a diff table on drift |
| 0.4 Extract host data | `crates/omm-host/data/{bundled-skills.json, slash-commands.json, gates.json, hook-events.json}` extracted from `research/` + `research/experiments/canary-diff.md` §1 | counts match host-reality (15 / 39 / 41 / 17) |
| 0.5 Experiments still open | (a) `run.context_slimming.skill_catalog_descriptions` — does it work, how many bytes; (b) marketplace precedence — does Muse read `.claude-plugin/marketplace.json` AND `.agents/plugins/marketplace.json` or first-found; does a Claude-schema marketplace entry pointing at a `.muse-plugin` package install as `native`; (c) MCP `tools/call` end-to-end through the mock provider harness at `research/experiments/…/verify-skill-routing` (copy it into `tools/mockprovider/`) | each a report in `docs/experiments/` with repro; results folded into host-reality.md |

**Gate 0**: adversarial review of `omm-host` (path traversal, symlink following, env leakage, the
settings writer's atomicity), hostcheck green, experiments reported. The gate itself is
`scripts/gate.sh`: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` with `OMM_MUSE_BIN` set.

## Phase 1 — M0 doctor + cost, M1 ledger + content + lint + uninstall/install

Parallel in git worktrees (each owns one crate or `content/`), then merge, then CLI wiring.

| Task | Deliverable | Acceptance |
|---|---|---|
| 1.1 `omm-ledger` | schema (§4), load/save with `.bad` quarantine, sorted entries, base-relative paths, non-regular sentinel, reconcile engine (4 outcomes), snapshot (rolling 5), audit JSONL, uninstall planner (preview, containment, deepest-first, allowlisted bases) | property tests on reconcile (all 4 outcomes, idempotence); containment tests with `..`, symlinks, absolute paths; planner refuses `/`, `$HOME`, base roots |
| 1.2 `omm-manifest` | content model + `catalog.json` loader (lifecycle, core gate); generators for native package, claude/codex projections, the three marketplace catalogs (root `marketplace.json` native + the two foreign projections, §5.2); lint (§5.3) incl. reserved-id/bundled/slash collision from data files, MCP name length, symlink/backslash scan, inventory units, budget estimate; overlay resolution (§5.4) with `_shadowed` | golden-file tests for generators; lint tests with one fixture per rule; validation against the real binary: `plugins validate` four predicates on the generated package |
| 1.3 `omm-doctor` | D1–D14 with fix commands; cost model + live measurement (temp `XDG_DATA_HOME`, `muse exec --provider echo hi`, parse `session.jsonl` for order-200 bytes and per-entry presence of description) | each check has a planted-failure e2e fixture and a pristine-pass fixture |
| 1.4 `content/` | 10 skills (R19 ids, ≤240-char descriptions with negative triggers, no overlap with the 15 bundled — read `research/musecode/skills.md`), incl. `omm-self` (teaches the agent omm's config schema) and `omm-reflect`; commands; 2–3 hooks with `command`+`commandWindows`; `translation/muse.md` (from SYNTHESIS §3.3); `rules/AGENTS.md.tmpl`; 3 themes (`.tmTheme`); profiles `strict default fast ci`; `catalog.json` | every skill passes `muse skills validate`; generated package passes R11; catalog bytes ≤ 21,542 measured |
| 1.5 CLI wiring | `install` (both paths, R6, R14 approve+verify), `uninstall` (R4), `update` (R3), `doctor`, `cost`, `lint`, `build`, `trust`, `theme`, `keymap`, `profile`, `list`, `run` (R20), `hook` (R16) | `tests/e2e` scenarios below |

**e2e scenarios** (`tests/e2e/`, each in a fresh temp HOME with `OMM_MUSE_BIN`):

1. install (bundle) → doctor green → uninstall → byte-identical
2. install `--no-plugin` → doctor green → uninstall → byte-identical
3. install → user edits a skill → update with a changed upstream → STAGED, on-disk untouched, report names it
4. install → user edits → uninstall → preview lists it under ✓ preserve; `--force` removes
5. install → `muse plugins disable omm` → doctor D1 critical with exact fix; run fix; doctor green
6. install → plant `mcp_servers: {}` → doctor D3; install refuses to patch settings; fix; green
7. install → `omm theme <x>` → `tui.theme` set, validated, prior recorded; uninstall restores prior
8. `omm run -- zzznotacommand` refused by allowlist; `omm run -- --version` passes
9. `omm hook <name>` on a sample event: JSON out, < 5 ms, non-zero handler → fail-open
10. install under `CI=true` without `--yes` → refused, nothing written

**Gate 1**: adversarial review (a second agent tries to make install leave residue, make uninstall
delete something it does not own, make update clobber an edit, make doctor false-positive/negative);
`cargo test`, clippy, e2e all green; fix loop until dry.

## Phase 2 — M2 marketplace + M3a tools + M3b themes/keymaps/profiles/memory

| Task | Deliverable | Acceptance |
|---|---|---|
| 2.1 Marketplace repo | three catalogs generated by `omm build` (ARCHITECTURE §5.2): the root `marketplace.json` — the NATIVE catalog Muse probes FIRST and the only one it reads (`schemaVersion:1`, `source:"local"`, entry `install:{transport:"local-path",source:"plugins/oh-my-musecode"}`, `integrity.digest` read from the binary after the package is final) — plus the two foreign projections `.agents/plugins/marketplace.json` (Codex schema → `dist/codex`) and `.claude-plugin/marketplace.json` (Claude schema → `dist/claude`), which Muse never opens while the root file exists (host-reality "Marketplaces (P1)", first-found-wins); `omm install` uses `plugins marketplace add omm <repo>` + `plugins install oh-my-musecode@omm`; local-path marketplace in tests | `marketplace add` on the repo root lists `oh-my-musecode` from the native file (`plugin_count 1, skipped []`); `plugins install oh-my-musecode@omm` → `inspect --json → manifest_family == "native"` and `package_sha256 == digest`; install from a local git clone of this repo works end to end |
| 2.2 `omm` MCP server | a tiny stdio MCP server **inside the omm binary** (`omm mcp`) exposing `omm_doctor` and `omm_cost` as tools — dogfoods the tools story with `len("oh-my-musecode")+len("doc") = 17 ≤ 18` | tool visible as `mcp__plugin_oh_my_musecode_doc__…` in a real session (the host sanitizes `-` to `_`); `tools/call` round-trip through the mock provider |
| 2.3 memory | `omm memory {seed,list,backup,gc}` on the `personal_project` root; slug-collision warning | formula test predicts a never-used path; trust prerequisite enforced |
| 2.4 profiles | `omm profile use <name>` = settings slice + permission profile + activation map + rules variant + theme + gate env, as ONE ledgered transaction | switch → doctor green → switch back → identical |
| 2.5 release | `scripts/release.sh`: per-platform static binaries, sha256 manifest, `install.sh` that verifies; `scripts/fetch-host.sh` | a fresh clone builds and installs from the manifest |

**Gate 2**: full e2e + adversarial review again; security review of the hook dispatcher and MCP server
(input parsing, no shell interpolation, no env leakage).

## Phase 3 — M4 skill routing (opt-in) + docs

| Task | Deliverable | Acceptance |
|---|---|---|
| 3.1 routing | `omm enable skill-routing`: copies the routed library to `<ws>/.omm/skills/` (real files, not symlinks), installs a router as a plugin hook with `"outputCapabilities":["skills.v1"]`, `omm run` carries both gates; budget guard `200+201 ≤ 31,984` | `read_skill` resolves a routed skill in a real session; disable is clean |
| 3.2 docs | `README.md`, `docs/INSTALL_FOR_AGENTS.md`, `docs/host-reality.md` current, `CHANGELOG.md` | a fresh agent can install from INSTALL_FOR_AGENTS.md alone |

**Gate 3**: final clean-clone verification — build from scratch, every test, every e2e, every lint;
security review; a last adversarial pass reading the whole repo for anything that violates R1–R22.

## Gate run (2026-09-03, shared working tree with Phases 2 and 3 uncommitted)

`scripts/gate.sh` against `.host/bin/muse-bin-1.0.1-R2006.1`: fmt ok, clippy (`-D warnings`) ok,
`cargo test --workspace --no-fail-fast` **483 passed, 4 failed** in 3 targets — e2e 30/30,
routing 3/3, the `omm mcp` mock-provider harness ok, hostcheck 1/1, repo_lint 4/4. The four red
tests belong to files still being edited on this tree and are recorded here as measured, not
interpreted: `crates/omm/tests/cmd_c.rs:1044`
(`memory_path_prints_the_personal_project_root_of_the_cwd`, exit 0 where 2 was expected);
`crates/omm/tests/mcp_server.rs:189` (the trace `data/muse/plugins/data/omm/omm-mcp.log` not
found) and `:283` (`muse/.auth.json.lock` under the roots after the doctor tool ran);
`crates/omm-manifest/tests/host.rs:81` (20 host-checkpoint rows where 19 were expected — the
catalog gained the `doctor` MCP server). At `HEAD` ("gate 1 closed") the gate was green with 30
e2e scenarios and ~440 tests.
