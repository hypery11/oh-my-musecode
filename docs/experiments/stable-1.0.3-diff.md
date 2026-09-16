# Experiment: muse-stable 1.0.1-R2006.1 → 1.0.3-R2198.1 (and canary 1.1.0-R2074.1)

Status: **RESOLVED — one new gate, one new enum value, one gated behaviour; everything else on
the stable line is byte-identical.** Measured 2026-09-05 against the three sha256-verified binaries
in `.host/bin/` (`muse-bin-1.0.1-R2006.1`, `muse-bin-1.0.3-R2198.1`, `muse-bin-1.1.0-R2074.1`), each
in its own throw-away `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`, `MUSE_NO_AUTO_UPDATE=1`,
`TBH_DISABLE_FEATURE_CONFIG=1`, a temp cwd, `--provider echo` only, never logged in. Nothing here
comes from documentation or from a `strings` diff (canary-diff.md §3 explains why that method is
~100 % false positives); every row is a behavioural oracle, most of them the ones
`research/experiments/canary-diff.md` used.

First instrument: `OMM_MUSE_BIN=<bin> omm doctor --self-test` (the hostcheck P0/P1 set as it stood
before this fold — 62 rows). Then the manual probes below.

| build | `--version` | `process.identity` | size | sha256 |
|---|---|---|---|---|
| 1.0.1-R2006.1 (muse-stable, previous) | `Muse Code 1.0.1 (1.0.1-R2006.1)` | `build_commit="e27e408b66"` `workflow_engine="v8"` | 241,576,272 B | `b9c7f9ba…54a4` |
| **1.0.3-R2198.1 (muse-stable, current)** | `Muse Code 1.0.3 (1.0.3-R2198.1)` | `build_commit="238bb03ff3"` `workflow_engine="v8"` | 241,984,144 B (+407,872) | `4c0f9600…` |
| 1.1.0-R2074.1 (muse-canary) | `Muse Code 1.1.0 (1.1.0-R2074.1)` | `build_commit="ea67f229ca"` `workflow_engine="v8"` | 249,325,152 B | `ca941549…` |

## 0. `omm doctor --self-test` before the fold (62 rows, the pre-fold data files)

| build | result | non-PASS rows |
|---|---|---|
| 1.0.1-R2006.1 | 62 checks, 0 failed | `DATA slash/39-rows` only |
| 1.0.3-R2198.1 | 62 checks, **2 failed** | `gates/total` 41 → **42**; `gates/id-set` differs: `["ultra_reasoning_effort"]` |
| 1.1.0-R2074.1 | 62 checks, **13 failed** | `msp/stable-fingerprint`, `msp/schema-json-sha256`, `msp/experimental-fingerprint`, `msp/dts-sha256`, `tools/active-count` 26 → 27, `tools/active-set` extra `snooze_reminder`, `tools/active-count-untrusted` 20 → 21, `gates/default-on` 14 → 15, `gates/default-on-set` differs `["todo_reminder"]`, `bundled/visible-without-gate` 14 → 16, `bundled/with-gate` 15 → 17, `bundled/id-set` differs `["bundled:durable-test-collateral", "bundled:requirements-clarification"]`, `bundled/package-files` 21 → 28 |

The two stable failures are one fact (§3). The canary is a different line (§5): it does **not**
carry the 1.0.3 changes and 1.0.3 does not carry the canary's.

## 1. The difference table — 1.0.1-R2006.1 vs 1.0.3-R2198.1

`≡` = identical. The last column names what in omm the difference touches (data file / `hr::`
constant / host-reality.md row / test); `—` = nothing.

| # | Axis | Instrument | 1.0.1 | 1.0.3 | Verdict | omm impact |
|---|---|---|---|---|---|---|
| 1 | CLI: command set | `<cmd> --help` for the 16 allow-listed commands | 16 | 16 | ≡ byte-identical, all 16 | — |
| 2 | CLI: depth-2 help | `--help` on 10 `skills`, 12 `plugins`, `schema`/`trace`/`session-message`/`auth`/`workflows` verbs (55 help texts in all) | — | — | ≡ every text byte-identical **except** `root`, `root` ungated and `exec`, each on exactly one line: `Meta reasoning effort: none\|minimal\|low\|medium\|high\|xhigh\|ultra` → `…\|xhigh\|**max**\|ultra` | muse-cli.json `root_flags[--reasoning-effort].values` + `values_since`; hostcheck `cli/reasoning-effort-values`, `cli/reasoning-effort/max`; `hr::REASONING_EFFORT_*` |
| 3 | CLI: root flags | root `--help` | 30 | 30 | ≡ (only the enum line above) | — |
| 4 | `--reasoning-effort` value list | `exec --provider echo --reasoning-effort <v> hi`, one run per value; the host checks the value before the provider, so an accepted value dies on `--reasoning-effort is not supported with --provider echo` (exit 2) and an unlisted one on `unsupported reasoning effort \`<v>\`; expected <list>` (exit 2) | 7 accepted; `max` → `unsupported reasoning effort \`max\`` | **8 accepted; `max` passes the value check** | **`max` is new** | as row 2; `hr::REASONING_EFFORT_UNSUPPORTED_MESSAGE`, `hr::REASONING_EFFORT_ECHO_REFUSED_MESSAGE` |
| 5 | Feature gates | `plugins enable __omm_gate_probe__` → `gate.resolve` lines | 41 | **42** | **`ultra_reasoning_effort` appended (position 42, `enabled=false source="default"`)**; the other 41 ids and their order identical | gates.json item 42 (`since: 1.0.3-R2198.1`), `counts.gates` 42; `hr::GATES_TOTAL`; hostcheck `gates/total`, `gates/id-set`, `gates/since/*`; `tests/probes.rs` |
| 6 | Gates default-ON | same | 14 | 14 | ≡ same 14 | — |
| 7 | New gate's env | `MUSE_EXPERIMENTAL_ULTRA_REASONING_EFFORT=1` / `=0` | (no line) | `enabled=true source="override"` / `enabled=false source="override"` | flips as the other 41 do | gates.json `accepted_values` note unchanged (1 accepted) |
| 8 | Bootstrap trace | complete-trace line count; `event=` vocabulary; `path.resolved` kinds; `feature_config.cache` | 49 lines; 6 events; 4 kinds; `state="missing" gate_count=0` | **50 lines**; 6; 4; same | one more `gate.resolve` line, nothing else | `hr::BOOTSTRAP_TRACE_FIXED_LINES` (8) + one per gate replaces the literal 49; `tests/probes.rs` |
| 9 | settings.json `schema_version` | loader oracle via `skills list --source user --json` | 1 required; 0, 2, `"1"`, absent rejected | same | ≡ | — |
| 10 | settings.json keys | wrong-type (`true`) oracle on all 29 keys | 29 messages | 29 identical messages | ≡ no new key, no renamed struct | — |
| 11 | `settings.reasoning_effort` values | the same file with `high xhigh ultra max bogus`, then an echo session | `max`/`bogus`: `tbh: ignoring invalid reasoning_effort "max" in settings` (exit 0, default used) | **`max` accepted silently**, run context = the high/xhigh one (8 blocks) | `max` is a real setting now | settings-keys.json row 6 (`values_since`); host-reality Budgets row |
| 12 | **`ultra` effort** | `settings.reasoning_effort: "ultra"`, echo session, `--trust-workspace` | **10 context blocks**: the 8 usual ones plus order **183 `workflow_availability_proactive` (1,355 B)** and order **187 `subagent_delegation_proactive` (528 B)** — unconditionally | **8 blocks** and stderr `tbh: reasoning effort ultra is not available (gate ultra_reasoning_effort is closed); using xhigh`; with the env var set: the 10 blocks of 1.0.1, byte for byte | **this is what the new gate gates** — §3 | gates.json item 42 `effect_probe`; `hr::CONTEXT_ORDER_WORKFLOW_AVAILABILITY_PROACTIVE`, `hr::CONTEXT_ORDER_SUBAGENT_DELEGATION_PROACTIVE`, `hr::*_PROACTIVE_BYTES`; hostcheck `gates/effect/ultra_reasoning_effort/{closed,open,absent}` |
| 13 | Enterprise defaults plane | `config validate --plane defaults` on `{"schema_version":1,"settings":{<key>:true}}` × 29, `schema_version` 0/2, `settings.reasoning_effort` ∈ high/ultra/max/bogus | 29 identical verdicts; `max` → `semantic_invalid location=settings.reasoning_effort` | same, **except** the `max` message is `enterprise_document_invalid: plane=defaults reason=semantic_invalid` (no `location=`) | `max` still refused on this plane; message shortened | muse-cli.json `--reasoning-effort` notes (informational; no test keys on the suffix) |
| 14 | Enterprise generation | `config status` | `sha256:db7c1fb6…` | same | ≡ | — |
| 15 | Plugin `schemaVersion` | `plugins validate --json` with 0/1/2/99/`"1"` | only 1 | only 1 | ≡ | — |
| 16 | Plugin capability families | five-family fixture, one fixture per family | 5: `skills hooks mcp_servers commands reminders`, summary `full` | same | ≡ | — |
| 17 | Rejected capabilities | `tools` only / alongside, `agents` only / alongside / empty array, `developerPrompts` with and without `text`, `outputStyles`, `settings`, `apps`, a bogus kind | 22 fixtures, verdicts as canary-diff §2.1 (`agents` alongside: `valid:true` + `unsupported-capability` warning, summary `partial`; a bogus kind: `unsupported-field` warning, summary `full`; `developerPrompts` without `text`: `invalid-manifest-schema`) | 22 identical verdicts | ≡ | — |
| 18 | Hook events | one hook fixture per name, 17 listed + 10 foreign | 17 accepted, 10 `unsupported-hook-event` | identical | ≡ | — |
| 19 | Context blocks, trusted | `exec --provider echo --trust-workspace hi` → `model_request_configured.run_context_messages` | 8 @ `85 95 96 180 181 186 200 240`, 34,254 B, `skills_catalog` 10,031 B | identical orders **and bytes** | ≡ | — |
| 20 | Context blocks, untrusted | same without `--trust-workspace` | 7 (no 186), 20 tools | identical | ≡ | — |
| 21 | Active toolset | `toolset.active_tools` | 26 / 20 untrusted, same names, same order | identical | ≡ | — |
| 22 | `skills_catalog` cap | binary search on one planted personal skill's description (`$XDG_CONFIG_HOME/muse/skills/omm-cap/SKILL.md`), 15 echo sessions per build | block **32,000 B** at 21,855 desc bytes, collapse to 10,109 B one byte later | identical to the byte | ≡ cap still exactly 32,000 | — |
| 23 | session.jsonl | record inventory of the trusted echo session | 43 records; `frame_schema_version` 1; record `schema_version` {1} | **44 records**: one new `runtime.retained_fact` event (`payload.record.payload_type: "top_level_turn.run_origin"`, empty payload) right after `runtime.command_intake.settled`; frame/record versions unchanged | one new informational record | host-reality P0 row note; hostcheck `session/record-schema-versions` still `{1}` (no test keys on the count) |
| 24 | `muse export --last` | document keys, `export_schema_version` | 1; 8 top-level keys | identical | ≡ | — |
| 25 | MSP stable fingerprint | `schema generate-json-schema` | `sha256:03312c21…` | same | ≡ | — |
| 26 | MSP experimental fingerprint | `--experimental` | `sha256:577d717d…` | same | ≡ | — |
| 27 | `msp.schema.json` / `msp.d.ts` bytes | sha256 | `f7c77710…` / `5108cbde…` | same | ≡ | — |
| 28 | MSP surface | `methods` / `notifications` / `errors` | 31 / 23 / 29 | same | ≡ | — |
| 29 | Bundled skills | `skills list --source built-in --json`, gate off / on; materialised tree | 14 / 15, 21 files | identical | ≡ | — |
| 30 | Shipped content / reserved names | `plugins list --available --json`; `marketplace add tbh-curated <dir>` | `[]`; `marketplace \`tbh-curated\` is reserved and cannot be added` | identical | ≡ | — |
| 31 | Exit codes | unknown root flag · unknown `skills` verb · `--version` · `plugins list` gated/ungated · `session-message list` gated/ungated · unknown top-level token on a non-tty · `init` twice · `serve --listen` · `schema` with `SDK_ENABLED=0` | 2 · 2 · 0 · 0/2 · 0/1 · 1 · 0 then 1 · 2 · 5 | identical, identical messages | ≡ | — |
| 32 | TUI first screen | pty harness (`.host/harness/drive.py` shape), 70×180 | banner `Muse Code 1.0.1` | `Muse Code 1.0.3` | version banner only | — |
| 33 | TUI slash menu | `/` in the pty | 7 rows + `↓ 32 more` = 39 | 39 | ≡ | — |
| 34 | `/effort` argument hint | `/effort` in the pty | `[<none\|minimal\|low\|medium\|high\|xhigh\|ultra>]` | `[<…\|xhigh\|**max**\|ultra>]` | `max` again | slash-commands.json `/effort` row (`argument_hint_since`) |
| 35 | Version string / build id | `--version`, `process.identity` | `1.0.1` / `e27e408b66` | `1.0.3` / `238bb03ff3` | cosmetic (R15) | muse-cli.json `version_string`; pins in README, INSTALL_FOR_AGENTS, ci.yml, fetch-host.sh, `.host/channel.json` |

**Total: 35 axes (the 45 of canary-diff.md collapse into these; the discovery-root and TUI
plugin-MCP axes were not re-run — nothing above suggests they moved). Differences: 3 semantic
(rows 4/5/12 — one feature), 2 informational (rows 13, 23), 2 cosmetic (rows 32, 35).**

## 2. What did NOT change (so nobody re-measures it)

Every P0 row of host-reality.md: both MSP fingerprints and both file hashes, the enterprise
generation, plugin `schemaVersion`, the five capability families and every rejected kind, settings
`schema_version`, export/frame schema versions, the eight context-block orders **and their byte
sizes**, the 26/20 toolsets, the 32,000-B catalog cap, 17 hook events, 15 bundled skills / 21 files,
16 commands / 30 root flags, 39 slash entries, the exit-code matrix, the 29 settings keys'
loader messages. All 22 plugin-validator fixtures return identical JSON.

## 3. The new gate: `ultra_reasoning_effort`

| Fact | Value |
|---|---|
| id / env | `ultra_reasoning_effort` / `MUSE_EXPERIMENTAL_ULTRA_REASONING_EFFORT` |
| first seen | 1.0.3-R2198.1 (`since`); absent on 1.0.1-R2006.1 and on canary 1.1.0-R2074.1 |
| position | 42nd and last `gate.resolve` line (trace line 49 of 50) |
| default | **OFF** — `event="gate.resolve" gate="ultra_reasoning_effort" enabled=false source="default"` |
| env flip | `=1` → `enabled=true source="override"`; `=0` → `enabled=false source="override"` |
| what it gates | the `ultra` reasoning effort. Closed: `settings.reasoning_effort: "ultra"` prints `tbh: reasoning effort ultra is not available (gate ultra_reasoning_effort is closed); using xhigh` on stderr (exit 0) and composes the xhigh run context — 8 blocks. Open: ultra composes **two extra context blocks**, order 183 `workflow_availability_proactive` (1,355 B) and order 187 `subagent_delegation_proactive` (528 B), 10 blocks, +1,883 B of run context, and no notice |
| older builds | 1.0.1-R2006.1 and canary 1.1.0-R2074.1 compose the 10 ultra blocks **unconditionally** — the gate withholds a behaviour those builds ship, it does not add one |
| toolset | unchanged in every state (26 trusted) |
| `--reasoning-effort ultra` on the CLI | accepted on every build (value check), then refused under `--provider echo` — not observable offline; the settings key is the oracle |
| `max` | **not** behind this gate: with the gate closed or open, `settings.reasoning_effort: "max"` on 1.0.3 composes the plain 8 blocks; on 1.0.1 it is `tbh: ignoring invalid reasoning_effort "max" in settings` |

Byte-level: the trusted echo session under `settings.reasoning_effort: "ultra"` differs from the
`high` one only in those two blocks (plus the usual 1–5 B `session_identity` jitter from the
session-log path length); no other block, no tool, no record kind moves.

Reproduce (any build):

```bash
export HOME=$T/home XDG_CONFIG_HOME=$T/cfg XDG_DATA_HOME=$T/data MUSE_NO_AUTO_UPDATE=1 TBH_DISABLE_FEATURE_CONFIG=1
mkdir -p $T/cfg/muse $T/ws && echo '{"schema_version":1,"reasoning_effort":"ultra"}' > $T/cfg/muse/settings.json
(cd $T/ws && $BIN exec --provider echo --trust-workspace hi)          # 1.0.3: the `using xhigh` notice
MUSE_EXPERIMENTAL_ULTRA_REASONING_EFFORT=1 $BIN exec --provider echo --trust-workspace hi   # no notice
jq -r 'select(.payload.event.kind=="model_request_configured") | .payload.event.run_context_messages[] | "\(.order) \(.id) \(.text|utf8bytelength)"' \
  $XDG_DATA_HOME/muse/sessions/*/*/*/*/session.jsonl                  # 183 + 187 present ⇔ ultra live
```

## 4. How omm folds it (the `since` rule)

The naive fold — bump 41 to 42 — turns the gate green on 1.0.3 and red on 1.0.1, and R15 forbids
picking by version string. The fold instead tags the fact with the build it was first observed on:

- gates.json item 42 carries `"since": "1.0.3-R2198.1"` (`since_rule` at the top of the file);
  muse-cli.json `--reasoning-effort` carries `"values_since": {"max": "1.0.3-R2198.1"}`;
  settings-keys.json and slash-commands.json record the same tag on their rows.
- `hostcheck` treats a tagged fact as EXPECTED wherever probing finds it. Where probing finds it
  absent, the row is `OLDER-BUILD` — a pass with a note — **only if every other fact tagged with
  the same build is absent too**: the build as a whole behaves like one that predates the tag.
  On 1.0.1 the group `1.0.3-R2198.1` has two facts (the gate, the `max` value) and both are
  absent → OLDER-BUILD; on 1.0.3 both are present → PASS. A mixed group, a missing untagged fact,
  or an observed gate that gates.json does not list is FAIL. The version string is quoted in the
  OLDER-BUILD note for the reader and used for nothing else.
- gates.json item 42 also carries an `effect_probe` (`settings_echo_session`): hostcheck writes the
  settings, runs the echo session and checks §3's orders and notice in the `closed`, `open` and
  `absent` states — so "what the gate gates" is re-measured on every build, not remembered.
- Rows: `gates/total` (expected 41 on 1.0.1 = 42 listed − 1 OLDER-BUILD; 42 on 1.0.3),
  `gates/id-set`, `gates/since/ultra_reasoning_effort`, `gates/effect/ultra_reasoning_effort/*`,
  `cli/reasoning-effort-values`, `cli/reasoning-effort/max`, `cli/reasoning-effort-bogus-rejected`,
  `since/1.0.3-R2198.1`. The trace-length assertion is `hr::BOOTSTRAP_TRACE_FIXED_LINES` (8) + one
  line per gate resolved.

After the fold, `omm doctor --self-test`: **1.0.1-R2006.1 — 68 checks, 0 failed, 3 OLDER-BUILD**
(`gates/since/ultra_reasoning_effort`, `gates/effect/ultra_reasoning_effort/absent`,
`cli/reasoning-effort/max`); **1.0.3-R2198.1 — 69 checks, 0 failed**.

## 5. muse-canary 1.1.0-R2074.1 — a different line, reported, not folded

Against 1.0.1-R2006.1 (the same 35 axes):

| Axis | canary 1.1.0-R2074.1 |
|---|---|
| gates | 41 — **no `ultra_reasoning_effort`**; `todo_reminder` resolves `enabled=true source="default"` → **15 default-ON** |
| `--reasoning-effort` | 7 values, **no `max`** (help, value check and `/effort` hint all as 1.0.1); `settings.reasoning_effort: "ultra"` composes the 10 blocks unconditionally |
| `exec --help` | `--max-tool-output-bytes` help gains `(0 or >=9)`; every other help text identical to 1.0.1 |
| MSP | stable fingerprint `sha256:911e1c4a…`, experimental `sha256:e1fb4114…`, `msp.schema.json` `2823b8cf…`, `msp.d.ts` `de6eb19f…`; counts still 31 / 23 / 29 |
| toolset | **27 trusted / 21 untrusted**: `snooze_reminder` added before `write_todos`; `model_request_configured` gains a `reminder_roster` key |
| bundled skills | **17 / 16 without the gate, 28 files**: `bundled:durable-test-collateral`, `bundled:requirements-clarification` listed; `skills/daemon/` (+`scripts/daemon_registry.py`) and `skills/slack-connector/` (+`resources/slack-app-manifest.json`, `scripts/slack_connector.py`) materialised but not listed; `skills_catalog` 11,926 B (from 10,031) |
| catalog cap | still 32,000 B (boundary at 19,960 desc bytes because the built-in entries grew; collapse to 12,004 B) |
| session.jsonl | the same 44th record as 1.0.3 (`runtime.retained_fact`); the log now contains the literal `reasoning_effort` |
| unchanged | settings keys and their messages, enterprise generation, plugin validator (22 fixtures), 17 hook events, exit codes, `plugins list --available` `[]`, `tbh-curated` reserved, 39 slash entries, 30 root flags, 16 commands |

`omm doctor --self-test` after the fold: 68 checks, **13 failed**, 3 OLDER-BUILD — the 13 are the
canary's own moves above (four MSP hashes, three toolset rows, two default-ON rows, four bundled
rows); the 3 OLDER-BUILD rows are the 1.0.3 facts it does not have. None of it is folded: the
canary channel is watched, not pinned (ci.yml `hostcheck` job).

## 6. Method notes

- `--reasoning-effort` cannot be exercised end to end offline (refused with `--provider echo`),
  but the host checks the value **before** the provider, so the two refusal messages tell the
  list apart; the settings key is the behavioural oracle for what a value does.
- The settings-key oracle is the loader (`skills list --source user --json` after writing the
  file), not `config validate` — the latter validates the enterprise defaults plane and reports
  every bare settings key as `unknown_member`.
- Plugin fixture ids must be lower-case (`invalid-plugin-id` otherwise); the first run of this
  probe lost every hook-event row to that.
- The bootstrap-trace writer is lossy under concurrent bootstraps (host-reality.md P1); the probe
  re-runs the verb until a trace has both its `startup` head and its `trust.resolve` tail.
- Artifacts (scratch, not committed): `probe.py` (35-axis harvester, one JSON per build),
  `compare.py`, `tui.py` (pty driver), `out-<build>/result.json`, `out-<build>/help/*.txt`,
  `out-<build>/bootstrap-trace.log`, `out-<build>/session-trusted.jsonl`.
