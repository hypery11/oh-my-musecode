# Host reality — what the Muse binary actually does

Shape of every row: **fact → version observed → code constant → test that locks it.**
Nothing in this file is derived from documentation. Every value was measured against the shipped
binary. `omm doctor --self-test` and `tests/hostcheck` re-measure the P0 block on every run.

Observed builds: `1.0.1-R2006.1` (muse-stable until 2026-09-05), **`1.0.3-R2198.1` (muse-stable,
the pin)**, `1.1.0-R2009.1` and `1.1.0-R2074.1` (muse-canary), 2026-09-01/02 and 2026-09-05.
Between the two stable builds one gate, one enum value and one gated behaviour moved (the
"Release log" at the end; `docs/experiments/stable-1.0.3-diff.md`); between 1.0.1 and the first
canary, 45 measured axes changed on exactly 2 cosmetic values (`--version` string, `userAgent`
build hash). **Version numbers track the product, not the contract — never gate on them.** A fact
that appeared with a build is tagged `since: <build>` in its data file; hostcheck expects it wherever
probing finds it and reports it as `OLDER-BUILD` (a pass with a note) only where the whole group of
facts tagged with that build is absent (gates.json `since_rule`; hostcheck module docs).

Source of record: `research/musecode/*.md`, `research/experiments/*.md`, and — for everything
measured after the research phase — `docs/experiments/*.md` (the "Phase 0 experiments" section at
the end lists them). When a value here and a value there disagree, the experiment report wins.

Code constants live in `crates/omm-host/src/host_reality.rs` (`hr::` below); data lists in
`docs/host-data/*.json`.

## P0 — run on every channel poll (< 5 s)

| Fact | Constant | Test |
|---|---|---|
| MSP stable schema fingerprint | `sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7` | `muse schema generate-json-schema` → hash |
| MSP experimental fingerprint | `sha256:577d717d09bf3aae6ad43c85d3c2d8e0c37bbde350897c94808626d2f362060c` | `… --experimental` |
| `msp.schema.json` bytes | `f7c77710dbf181b309a3a12060627608dd5c91b1ec0680953ca7381b21181beb` | file hash |
| `msp.d.ts` bytes | `5108cbde447cdfe65fd1bd26f8d2910a20141668959d0da8ce6bd55bb1adcf81` | file hash |
| enterprise generation | `sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4` | `muse config …` |
| MSP surface | 31 methods · 23 notifications · 29 error codes | count from schema |
| plugin manifest `schemaVersion` | `1` (0, 2, 99 rejected) | `plugins validate` on fixtures |
| plugin capability families | exactly 5: `skills hooks mcpServers commands reminders` | validate fixture per family + one rejected |
| `settings.schema_version` | `1` (0 and 2 rejected) | `config validate --plane defaults` |
| `export_schema_version` / `frame_schema_version` | `1` / `1`; record `schema_version` values `{1}`. The trusted echo session writes 43 records on 1.0.1-R2006.1 and 44 since 1.0.3-R2198.1 (one `runtime.retained_fact` event, `top_level_turn.run_origin`; stable-1.0.3-diff.md row 23) — the count is informational, the versions are locked | `muse export` on an echo session; hostcheck `session/record-schema-versions`, `export/schema-version` |
| context blocks (`--trust-workspace`) | 8 @ orders `85 95 96 180 181 186 200 240` (`hr::CONTEXT_BLOCK_ORDERS`); untrusted: 7, no 186 (`hr::CONTEXT_BLOCK_ORDERS_UNTRUSTED`) | `muse exec --provider echo --trust-workspace hi` → `session.jsonl`; hostcheck `context/block-orders`, `context/block-orders-untrusted` |

## P1 — run per release

| Fact | Constant | Test |
|---|---|---|
| experimental gates | **42 total on 1.0.3-R2198.1, 41 on 1.0.1-R2006.1** (`ultra_reasoning_effort`, gates.json n=42, `since: 1.0.3-R2198.1`, default OFF — OLDER-BUILD on 1.0.1), 14 default-ON on both; canary 1.1.0-R2074.1: 41 and 15 default-ON (`todo_reminder`), not folded. `hr::GATES_TOTAL` is the data file's 42; the count a build must show is `Gates::partition_for_build` (every untagged row + every tagged row the trace shows) | bootstrap trace `gate.resolve` lines — **the trace writer is lossy under concurrent bootstraps** (0/14/22/48-line logs instead of 49, measured 2026-09-02); a trace counts only with its `event="startup"` head AND its `event="trust.resolve"` tail (`hr::BOOTSTRAP_TRACE_HEAD_EVENT` / `_TAIL_EVENT`), otherwise `probe::gates` re-runs the verb with an exponential jittered backoff (100 ms → 2 s) until `probe::GATE_PROBE_DEADLINE` (20 s, under `RUN_TIMEOUT_DEFAULT`; serialized per process) — a fixed budget of 8 attempts (≈ 2–4 s) lost 2 of 12 suite runs under an 8-wide storm of host sessions, a burst outlives a count but not a clock — and the error names the attempts and the seconds tried; the verb and the trace location are read from gates.json `probe` (`GateProbe::argv`), never hand-copied; hostcheck `gates/trace-complete` |
| remote feature-config cache | `event="feature_config.cache" state="missing" gate_count=0` (`hr::FEATURE_CONFIG_CACHE_EVENT`); the cache lives under the data root (`path.resolved kind="feature_config_cache" source="data_root"`), so a probe with a redirected data root never sees one — but only by luck of the cache. Every probe therefore runs with `TBH_DISABLE_FEATURE_CONFIG=1` (`hr::ENV_DISABLE_FEATURE_CONFIG`, set by `Invoker`'s clean environment, never for `omm run`; the trace is byte-identical with and without it, 49 lines) and the line is asserted, so a populated cache is REPORTED rather than silently moving `gates/default-on` and `bundled/*` | hostcheck `gates/feature-config-cache`; `probe::parse_feature_config_line` |
| default-ON gates | `workflow_tool local_session_messaging bash_titles bash_sandbox_escalation git_sandbox_relaxation first_turn_minimal_effort memory_reminder skill_reminder goal_reminder verify_reminder scope_reminder non_strict_tool_params sdk_enabled voice_native_capture` | same |
| complete bootstrap trace | 8 fixed lines (`hr::BOOTSTRAP_TRACE_FIXED_LINES`: `startup`, `process.identity`, 4× `path.resolved`, `feature_config.cache`, `trust.resolve`) + one `gate.resolve` per gate — 49 on 1.0.1-R2006.1 and canary, 50 on 1.0.3-R2198.1 (`hr::BOOTSTRAP_TRACE_LINES_OBSERVED`, informational) | `tests/probes.rs::gates_probe_and_override_detection`, `…survives_concurrent_host_load` |
| `ultra_reasoning_effort` effect | with the gate present and closed, `settings.reasoning_effort: "ultra"` prints `tbh: reasoning effort ultra is not available (gate ultra_reasoning_effort is closed); using xhigh` and composes the 8 xhigh blocks; open (`MUSE_EXPERIMENTAL_ULTRA_REASONING_EFFORT=1`) — and unconditionally on builds without the gate (1.0.1-R2006.1, canary 1.1.0-R2074.1) — ultra composes **10 blocks**: orders 183 `workflow_availability_proactive` (1,355 B, `hr::CONTEXT_ORDER_WORKFLOW_AVAILABILITY_PROACTIVE`) and 187 `subagent_delegation_proactive` (528 B, `hr::CONTEXT_ORDER_SUBAGENT_DELEGATION_PROACTIVE`) join the 8; toolset unchanged (gates.json n=42 `effect_probe`; stable-1.0.3-diff.md §3) | hostcheck `gates/effect/ultra_reasoning_effort/{closed,open}` (gate present) or `…/absent` (OLDER-BUILD) |
| `--reasoning-effort` values | `none\|minimal\|low\|medium\|high\|xhigh\|max\|ultra` — **`max` since 1.0.3-R2198.1** (muse-cli.json `values_since`; 1.0.1 and canary: `unsupported reasoning effort \`max\``). Observable offline because the host checks the value before the provider: an accepted value under `--provider echo` fails with `--reasoning-effort is not supported with --provider echo` (`hr::REASONING_EFFORT_ECHO_REFUSED_MESSAGE`), an unlisted one with `unsupported reasoning effort …` (`hr::REASONING_EFFORT_UNSUPPORTED_MESSAGE`), both exit 2. The same list is the `settings.reasoning_effort` enum (settings-keys.json row 6: 1.0.1 prints `tbh: ignoring invalid reasoning_effort "max" in settings` and uses the default; 1.0.3 accepts `max` with the plain 8-block run context) and the `/effort` hint (slash-commands.json). The enterprise defaults plane refuses `settings.reasoning_effort: "max"` as `semantic_invalid` on every build | hostcheck `cli/reasoning-effort-values`, `cli/reasoning-effort/max` (OLDER-BUILD on 1.0.1), `cli/reasoning-effort-bogus-rejected` |
| hook events | 17, PascalCase only | `hooks.json` with each name → validate |
| bundled skills | 15 `SKILL.md`, 21 files, plugin id `muse-core` | `muse skills list --json` |
| bundled skills in catalog | **14 of 15 without `MUSE_EXPERIMENTAL_PLUGINS=1`** (`bundled:create-plugin`, 426 B, is gated; `hr::BUNDLED_SKILLS_VISIBLE_DEFAULT`) | `docs/experiments/context-slimming.md` §2; hostcheck `bundled/visible-without-gate` |
| active tools (echo session, `--trust-workspace`) | **26** (`hr::ACTIVE_TOOLS`) — **20 untrusted** (`hr::ACTIVE_TOOLS_UNTRUSTED`): the six `subagent_*` tools and the order-186 `subagent_delegation` block (956 B, `hr::SUBAGENT_DELEGATION_BYTES`) compose only when the workspace is trusted | `session.jsonl` toolset; context-slimming.md §7.2; hostcheck `tools/active-count`, `tools/active-count-untrusted` |
| built-in slash commands | 39 | string table / composer picker |
| top-level CLI commands | 16 (14 advertised; `plugins` gated, `workflows` hidden) | `--help` + gate |
| MCP `tools/call` round-trip | 3 plugin servers (sid length 2 / 15 / 16) → 3 `tools/call` received, 3 `function_call_output` returned, `tool_result_batch_committed` ×3, in `exec` and `serve`; unapproved control → 0 namespaces on the wire | `tools/mockprovider/run-mcp-toolcall.sh` (+ `LANE=serve`, + `SKIP_APPROVE=1 PLAN_FILE=plans/unapproved-control.json`), P1–P3 PASS |

## Budgets

| Fact | Constant | Notes |
|---|---|---|
| `skills_catalog` (order 200) hard cap | **32,000 B** | header 363 B + footer 35 B |
| Meta's 15 built-ins | **10,060 B** of entries (`hr::BUILTIN_SKILLS_CATALOG_BYTES`); the whole block is **10,458 B** = 10,060 + 363 + 35, gate on (`hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES`), 10,031 B with 14 entries, gate off | refundable: `muse skills disable bundled:<id> --scope built-in`; `bundled:browser-app-delivery` alone = 1,912 B |
| Meta's 15 built-ins under `first_sentence` | **4,832 B** block (10,458 B full, gate on) / 4,609 B (10,031 B, 14 entries, gate off) / 3,899 B with `full_skill_description_ids: []` (`hr::BUILTIN_SKILLS_FIRST_SENTENCE_BLOCK_BYTES`) | `docs/experiments/context-slimming.md` §6 |
| room for a bundle | **21,542 B** → **31,602 B** with built-ins disabled | +48% |
| room for a bundle with `first_sentence` on | **27,168 B** with built-ins on (32,000 − 4,832; `hr::BUNDLE_BUDGET_FIRST_SENTENCE_BYTES`) vs 21,542 B in `full` | context-slimming.md §6 |
| entry cost | `38 + len(display_id) + len(display_path) [+ len(desc) + 36]` for `scope="plugin"` (`hr::CATALOG_ENTRY_BASE_BYTES`); `scope="user"` is `36 + len(display_id) + len(display_path) + len(rendered_desc) + 36` (`hr::CATALOG_ENTRY_BASE_BYTES_USER`, +2 B for `scope="plugin"`); XML escaping (`&apos;` etc.) counts | plugin id appears twice per entry; context-slimming.md §2 |
| `run.context_slimming.skill_catalog_descriptions: "first_sentence"` | **PROVEN, live.** Cuts every `<description>` after the first run of ASCII `.`/`?`/`!` followed by a space (whitespace runs incl. tab/LF/CRLF/NBSP collapsed first; abbreviations ARE boundaries; `.)` `."` `1.2` `。` are not; no boundary → kept whole). `<short-description>`, 363-B header, 35-B footer, 32,000-B cap unchanged; `skills list --json` still reports the full text. 40 user skills + bundled: 23,838 → 11,376 B (−52 %) | context-slimming.md §2–3; `hr::CONTEXT_SLIMMING_FIRST_SENTENCE` |
| `run.context_slimming.full_skill_description_ids` | exact display-id match (`bundled:taste`, never bare `taste`); unknown ids ignored; no-op in `full`. **Default is `["bundled:git"]`; a user value REPLACES it** (bundled:git 740 → 30 B unless re-listed; `hr::CONTEXT_SLIMMING_DEFAULT_FULL_IDS`) | same §2 |
| `run.context_slimming.session_identity_enabled: false` | removes order-240 `session_identity` (776–786 B; `hr::SESSION_IDENTITY_BYTES_MIN/MAX`) | same §2 |
| `run.context_slimming.meta_context_note_enabled` | **no observable effect** in the `exec` lane (echo and meta wire bodies identical modulo session id); `meta_context_note` is a context-source literal that never composes; TUI untested | same §0, §5 |
| `run.context_slimming.excluded_tool_names` | removes named tools from `active_tools`; unknown names ignored; `write_todos` NOT removable; `read_skill` removable (catalog still rendered); `["bash"]` without `bash_input` → run fails rc=1 `invalid run configuration: \`bash_input\` requires managed \`bash\` to be enabled`; `["workflow"]` removes the tool AND orders 180+181 with no replacement block (−20,952 B; `hr::WORKFLOW_EXCLUDED_RUN_CONTEXT_SAVING_BYTES`) | same §2, §7.4 |
| render order | bundled → project/user filesystem → plugin | plugin is starved FIRST |
| degradation | stage 1 intact → **stage 2 descriptions dropped tail-first, SILENT** (`skills list --json` still reports all, `diagnostics: []`) → stage 3 entries dropped, printed on stderr naming `32000` | doctor must measure stage 2; `first_sentence` is the stage-2 rescue: 100 skills → `full` 31,890 B with 48 descriptions dropped, `first_sentence` 21,396 B with all 114 present (context-slimming.md §6) |
| routing budget (200 + 201) | **31,984 B** combined; third outcome `selected_skills:rejected:combined-budget` | budget to 31,984 and you are right under both readings |
| routed skills per turn | 32 globally; description ≤ 1,024 B; hook stdout ≤ 16,384 B | |
| memory snapshot (order u32::MAX) | 16,305 B and 48 listed files per scope, both silent | |
| rules | 256,000 B/file, 65,536 B aggregate, both warned on stderr | |
| `workflow_cookbook` (181) | 18,056 B; `run.workflow_trigger_mode: "off"` removes it, swaps order-180 `workflow_choice` (2,896 B, `hr::WORKFLOW_CHOICE_BYTES`) for `workflow_availability_off` (539 B, `hr::WORKFLOW_AVAILABILITY_OFF_BYTES`) and drops the `workflow` tool (15,766 B of wire JSON, `hr::WORKFLOW_TOOL_WIRE_BYTES`) — net −20,413 B run context (`hr::WORKFLOW_OFF_RUN_CONTEXT_SAVING_BYTES`) / −36,308 B wire; `excluded_tool_names:["workflow"]` is 539 B cheaper | cheapest single saving; context-slimming.md §2, §5 |
| wire body, 40-skill install | 119,298 B (instructions 36,709 B constant; developer message = run-context blocks joined by `\n\n`; tools namespace 35,136 B) → 69,417 B with all four levers (−41.8 %) | context-slimming.md §5 |
| plugin manifest | 131,072 B inclusive | |
| package | 4,096 fs entries; path depth 16 | any symlink anywhere kills the package (opaque error) |
| inventory budget | 4,094 units = 1/class + 2/skill + 1/command·hook·mcpServer·reminder | single-source, fitted from 11 bisections |
| per-class max (one plugin) | skills 2,046 · commands 4,093 · hooks 2,153 · mcpServers 2,902 · reminders 43 | single-source |
| enabled plugins | 256 (257 → `plugin_preflight_overflow`, soft: only the agent-definition review lane is disarmed) | |
| `plugin_scope_quota` / `plugin_class_overflow` | never observed (128,000 caps admitted clean) | treat as nonexistent |

## Identity constraints

| Fact | Constant |
|---|---|
| id grammar | `^[a-z0-9][a-z0-9._-]{0,79}$` |
| MCP tool name as seen by the model | wire `tools[]` entry `{"type":"namespace","name":"mcp__plugin_<pid>_<sid>","description":"Tools provided by an MCP server.","tools":[{"type":"function","name":"<tool>","parameters":<inputSchema>}]}` (built-ins are the namespace `muse`, `hr::MCP_BUILTIN_NAMESPACE`); namespace survives verbatim to 31 chars (`hr::MCP_TOOL_NAME_VERBATIM_MAX`); 32+ rewritten to `first-17 + "__" + 12 hex` (digest deterministic per namespace string) → **`len(pid) + len(sid) ≤ 18`** (`hr::MCP_ID_LENGTH_BUDGET`); the `canonical_id` `mcp__plugin_<pid>_<sid>__<tool>` is never on the wire once rewritten (`docs/experiments/mcp-tools-call.md` §3.1) |
| MCP call addressing the host dispatches | `<ns>.<fn>` for every length · `<ns>__<fn>` only while the wire namespace is unrewritten (pid+sid ≤ 18) · the canonical id; bare `<fn>`, `<fn>`+`"namespace"` field, `<ns>/<fn>` → `tool unavailable: unknown tool …` fed back to the model with the full `Session tool ids` list, turn continues (mcp-tools-call.md §3.2–3.3) |
| MCP call in `session.jsonl` | `assistant_tool_calls_committed.tool_calls[].name`, `task_kind tool.<id>`, `side_effect_intent.operation tool:<id>` (`policy_decision allow:policy` under `--yolo` and under MSP `allowAll`), `tool_batch.effect.started.tool_name` all carry the canonical id whatever form the model used; `parallel_profile` is `ineligible`; result text in `tool_result_batch_committed.results[].text` (mcp-tools-call.md §2.2) |
| reserved plugin ids (install "successfully", report valid/active, do nothing) | `skill-reminder goal-reminder memory-reminder todo-reminder verify-reminder scope-reminder tbh-reminders loop muse-core` |
| our plugin id | **`omm`** |
| `compat.manifestDir` | a *consistency* rule, not a value rule — the same bytes validate in all three dot-dirs with different runtime semantics; assert `manifest_family == "native"` |
| `compatibility.summary` | `"full"` is NOT a safe gate (a plugin with zero supported caps reports `"unsupported"` with `valid:true, diagnostics:[]`) |
| per-family strictness | `hooks`, `reminders` CLOSED (unknown key = error); `skills`, `commands`, `mcpServers` OPEN (silently accepted) |
| native plugin hook fields | `commandWindows` / `command_windows` **rejected** in `capabilities.hooks[]` (`unsupported-field`, `valid:false`; `hr::PLUGIN_HOOK_REJECTED_FIELDS`); the Windows form exists only in the hooks.json tiers (`docs/experiments/marketplace-precedence.md` §7) — `plugins validate` fixture |
| `mcpServers` entries | `env`, `cwd`, `headers` validate and are then DROPPED at launch; transports `stdio\|http` only |
| `enabledDefault` | defaults `true`; only literal `false` disables |
| `muse config validate --plane defaults` on a settings candidate | accepts `run.context_slimming.*` and `run.workflow_trigger_mode` ONLY inside `{"schema_version":1,"settings":{…}}`; bare settings.json shape → `unknown_member`; refuses `hooks plugins runtime_capabilities mcpServers model_catalog permissions` (`unknown_member`) and `tui.theme` (`field_not_activated`); bad values → `semantic_invalid`/`wrong_type location=settings.run.context_slimming` — R10's writer must wrap and strip those keys (`crates/omm-host/data/enterprise-defaults-plane.json`; context-slimming.md §4) |
| settings loader, `run.context_slimming` | all 5 fields typed (unknown variant / `expected a boolean` / `expected a sequence` / `duplicate field` errors); unknown keys inside the struct silently ignored — `muse skills list --json` after writing the doc (context-slimming.md §4) |
| `permissions` object | `UserPermissionSettingsV1`, deny_unknown_fields with a REQUIRED `schema_version`: `{"permissions":{"profiles":{…}}}` (member present, `schema_version` absent) → `muse exec --provider echo hi` prints `muse: Named permission profiles are unavailable: missing field schema_version at line N column M` on stderr, exit 0, and every named profile in it is inert; with `schema_version: 1` the same profiles load (measured 2026-09-02, Gate 1 round 4 K). settings-keys.json `items[].structural` records it; a leaf restore under such an object keeps the structural member while other members remain (`settings::structural_keep_reason`), doctor D4 names the missing member |

## Trust lifecycle

| Fact | Detail |
|---|---|
| MCP/hook/reminder spawn gate | runtime capability state must be literally **`trusted_enabled`** |
| states that are silently skipped | `review_needed` (default after install) · `trusted_disabled` · `modified` · line absent |
| `muse plugins disable <id>` | **DELETES** the capability line (does not set a bad state) |
| any byte change to the package + `plugins update` | every capability → `modified` (package_sha256 → cache path → resolved command → `trusted_definition_hash`) |
| approve | `muse plugins approve plugin:<pid>:<kind>:<cap-id> [--json]` — non-interactive, no TTY |
| verify | `muse plugins inspect <id> --json` — line PRESENT and `trusted_enabled` |
| remove | `muse plugins remove <id> [--delete-data]` — strips `runtime_capabilities` |
| install-time gate | `MUSE_EXPERIMENTAL_PLUGINS=1` for `plugins` verbs; **runtime composition is ungated** |
| routing gates | `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1` + `…_APPLY=1`; handler must declare `"outputCapabilities":["skills.v1"]` |
| `muse serve` | requires `settings.json → provider`; without it: one-tool session, no MCP, no capability composition |
| MSP | NO tool invocation, NO tool registration, NO `session/close`; MSP sessions assemble no run context and run no hooks |
| MCP `tools/call` end-to-end | PROVEN offline (`tools/mockprovider/run-mcp-toolcall.sh`): model `function_call` → line-delimited JSON-RPC `tools/call {"name":"<tool>","arguments":{…}}` on the plugin server (arguments as an object, namespace stripped) → `result.content[0].text` → verbatim `function_call_output.output` in the next `/responses` request; identical in `muse exec` and `muse serve` (mcp-tools-call.md §2) |
| model-side symptom of a non-`trusted_enabled` capability | no `mcp__plugin_…` namespace in `tools[]`, no `mcp_tool_identity_catalog`, no spawn; any call → `tool unavailable: unknown tool` (the turn continues) (mcp-tools-call.md §2.4) |
| `muse serve` offline with a scripted model | `settings.json → provider:"meta", model:"<id>", endpoint_transport.base_url:"http://127.0.0.1:<port>"` + `META_API_KEY` in env (`provider_source="settings"`; `hr::ENV_META_API_KEY`); `session/start.approvalMode:"allowAll"`; `clientInfo.name` must match `^[a-z0-9_]+$` (SS1.4.1; `hr::MSP_CLIENT_NAME_PATTERN`); each MCP call surfaces as `item/started`→`item/completed` `kind:"toolCall"` with `tool:<canonical id>`, `callId`, `args` (mcp-tools-call.md §2.3) |
| workspace trust store without `projects` | `{"schema_version":1}` is accepted by the host (every workspace untrusted, no trust line, exit 0); an entry without `decision` or with a typo (`"trust"`) is `malformed trust store at …`, exit 1 — `trust::check_shape` mirrors exactly this (measured 2026-09-02; `tests/probes.rs::trust_merge_is_read_by_the_host`); a repeated key is malformed too — `{"schema_version":1,"projects":{…},"projects":{}}` → `muse exec --provider echo hi` exit 1 `malformed trust store at …: duplicate field \`projects\` at line 1 column 181` (2026-09-02) — and `TrustStore::load` refuses it up front like `SettingsDoc::load` (`settings::reject_duplicate_keys`) |

## Marketplaces (P1)

| Fact | Constant | Test |
|---|---|---|
| catalog probe order (dir or git source) | `marketplace.json` → `.agents/plugins/marketplace.json` → `.claude-plugin/marketplace.json` (`hr::MARKETPLACE_PROBE_ORDER`); **first existing file wins**, no merge, no fallback on parse error. The root file is a third, undocumented NATIVE catalog; the two foreign files serve the Codex-schema and Claude-schema ecosystems and are never opened while the root file exists | `marketplace add` on a dir with only the two foreign files → only Codex ids listed; on an empty dir → error text names the order (`docs/experiments/marketplace-precedence.md` §1) |
| `local-file` source | always the native parser, whatever the file name | `marketplace add m <dir>/.claude-plugin/marketplace.json` → ``must declare `/install/transport` `` |
| native catalog (root `marketplace.json`) | `schemaVersion:1` · `source:"local"` · per entry `name` `version` `install{transport:"local-path",source:<rel>}` `integrity{digest:"sha256:<64hex>"}` `availability{status:available\|blocked\|deprecated}` — all required; unknown keys ignored; `..`/absolute `install.source` accepted (no containment) (`hr::MARKETPLACE_NATIVE_*`) | one fixture per missing field → ``must declare `/<pointer>` `` |
| native `integrity.digest` | = Muse `package_sha256` (content-addressed, path- and git-independent); **verified at install, not at add**; mismatch → ``plugin `<id>` in marketplace `<m>` failed integrity check``; algorithm not reproduced — read it from `plugins list --available --json → digest` after a temp Codex-catalog add (`plugins validate --json` does not emit it) | wrong-digest fixture: add rc 0, install rc 1 |
| cross-family install | the package manifest dir decides the family, never the catalog schema: any catalog → `.muse-plugin` package → `manifest_family "native"`, provenance `marketplace-user-added` | Claude catalog → `.muse-plugin` pkg → `inspect --json → plugin.manifest_family == "native"` |
| catalog `name` | must equal manifest `name`; add succeeds, install fails ``resolves to a bundle whose manifest id `<x>` does not match`` | fixture |
| catalog `version` | informational, no diagnostic on mismatch; Claude/Codex parsers list the manifest version, native lists the catalog version; install records the manifest version | fixture |
| duplicate id in one catalog | all entries kept (`plugin_count` counts them, `skipped: []`, no diagnostic); `install <id>@<m>` takes the **first** in file order (all three parsers) | fixture |
| same id in two marketplaces | second install → ``plugin `<id>` is already installed from a different source``; reinstall from the same source is idempotent | |
| foreign entry minimum | Codex: `name` + `source:{"source":"local","path":<rel>}` (object required); Claude: `name` + `source:"<rel>"` (an object source, even `{"source":"local"}`, is remote → skipped under local-dir); everything else optional; bad entries → `skipped[]` + `severity:"warning"` in `snapshot.json`, rc 0; both foreign parsers reject `..`/absolute sources | |
| git source clone | shallow (`.git/shallow`, depth 1), single-branch of remote HEAD; worktree `plugins/marketplaces/<n>/source`; snapshot `install.source` = absolute path inside the worktree, `transport:"local-path"` | `marketplaces.json` record `{kind:"git",path,worktree_path}` |
| `marketplace update` (git AND local-dir) | **always** a new generation `plugins/marketplaces/<n>/generations/<epoch-ns>/{source,snapshot.json}` (fresh clone for git), lockfile repointed; retention = current + previous (`hr::MARKETPLACE_GENERATIONS_KEPT`; root `source/` deleted on the 2nd update); failed clone → rc 1, lockfile untouched | tree + inode before/after |
| installed plugin ↔ generation | `installed.json → source.path` pinned to the generation installed from; `plugins update <id>` re-reads that path (never the new generation); after 2 rotations or `marketplace remove` → rc 1 `plugin-source-unavailable` (`hr::MARKETPLACE_SOURCE_UNAVAILABLE_CODE`); `install <id>@<m>` while installed → `already installed from a different source`; **only path forward = `remove` → `install <id>@<m>` → re-approve** | |
| update signal | none (`plugins list --json → warning: null`); compare `list --available` digest with installed `package_sha256` | |
| remote per-plugin sources | `{"source":"url","url","ref"}` (Claude or Codex) → export without `.git` into `<worktree>/.muse-claude-sources/<ordinal>/<name>` resp. `.muse-codex-sources/…`, ordinal = index in `plugins[]`; re-cloned on every update; `ref` branch/tag/sha/absent(=HEAD); unknown ref → entry skipped; git marketplace only; Codex `github` unsupported | |
| `marketplace remove` | deletes `plugins/marketplaces/<n>/` entirely + lockfile record; installed plugin keeps running from cache | |
| `tbh-curated` | reserved marketplace name (add/remove refused) | muse-cli.json |

## Paths

| What | Where |
|---|---|
| config root | `$XDG_CONFIG_HOME/muse` else `~/.config/muse` — exactly two candidates |
| data root | `$XDG_DATA_HOME/muse` else `~/.local/share/muse` |
| NOT read by the binary | `MUSE_HOME MUSE_CONFIG_DIR MUSE_SESSIONS MUSE_NO_SESSION_LOG` |
| `MUSE_NO_AUTO_UPDATE` | launcher-only (0 occurrences in the binary) |
| personal skills | `$CONFIG_DIR/skills/` › `~/.agents/skills/` › `~/.claude/skills/` › `~/.codex/skills/` (or `$CODEX_HOME/skills`, replaces) — first-hit per id |
| project skills | `<ws>/.agents/skills/` › `.codex/skills/` › `.claude/skills/` — INVERTED foreign order; one level deep; trust-gated |
| personal rules | `$CONFIG_DIR/AGENTS.md` → `$CONFIG_DIR/CLAUDE.md` → `~/.claude/CLAUDE.md` → `~/.codex/AGENTS.md` — exactly ONE loads |
| project rules | `AGENTS.md` root→cwd, accumulate; `CLAUDE.md` same-dir fallback; trust-gated |
| hooks tiers | managed (`settings.managed_hooks_path` or `TBH_MANAGED_HOOKS_PATH`) → user (`settings.json → hooks`) → project (`<ws>/.muse/hooks.json`, trust-gated) → plugin |
| themes | `$XDG_CONFIG_HOME/muse/themes/*.tmTheme` (case-insensitive ext; data dir NOT scanned); `tui.theme = "custom:<stem>"` |
| trust | `$CONFIG_DIR/trust.json` = `{"schema_version":1,"projects":{"<abs>":{"decision":"trusted"}}}` (`projects` may be absent) |
| memory personal | `$XDG_DATA_HOME/muse/memory/personal/` |
| memory personal_project | `$XDG_DATA_HOME/muse/memory/projects/<slug96>-<fnv1a64hex>/` — slug = canonical path minus leading `/`, non-alnum → `-`, truncate 96; hash = FNV-1a-64 of full path, `%016x`; trust-gated |
| bootstrap trace | `$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-<uuid>.log` — written by a lossy writer (see P1 "experimental gates") |
| sessions | `$XDG_DATA_HOME/muse/sessions/YYYY/MM/DD/<uuid>/session.jsonl` — created even by a TUI start that fails on a missing tty |
| marketplace generations | `$XDG_DATA_HOME/muse/plugins/marketplaces/<name>/generations/<epoch-ns>/{source,snapshot.json}` — current + previous kept; `marketplaces.json → worktree_path/snapshot_path` name the live one |
| plugin data dir | `MUSE_PLUGIN_DATA_DIR` advertised to hooks and MCP servers but NOT created — `mkdir -p` it (`omm mcp` must) |
| residue outside XDG | `~/Library/Application Support/Muse/session-name-authority/` via `getpwuid` — real HOME, every run (`Roots::session_name_authority_residue`, rooted at the account home from the **passwd entry** — `paths::account_home()`: `id -un` + `dscl /Search -read /Users/<name> NFSHomeDirectory` on macOS, `getent passwd <uid>` on other unixes, cached per process, `$HOME` only when the entry cannot be read. `std::env::home_dir()` follows `$HOME` first, so under a redirected `HOME` it named `<redirected>/Library/…` while the host wrote the real account home; measured 2026-09-02, `paths::tests::account_home_ignores_a_redirected_home`. A `Sandbox` therefore names the real account home as its residue root while its XDG roots stay redirected. The lookup is two tool spawns, ~20 ms once per process, so the sub-5 ms hook dispatcher uses `Roots::from_env_fast()` — same XDG roots, `$HOME` as the residue root, never named there) |
| runtime dir / session-messaging registry | `/private/tmp/tbh-<uid>-rt/muse/` (`Roots::runtime_dir`, `residue::runtime_dir_for_uid`; keyed by the uid, ignores `HOME`/`XDG_*`/`TMPDIR`; binary fallback literals `$XDG_DATA_HOME/muse/runtime/muse`, `~/.local/share/muse/runtime/muse`; measured on macOS). Every session — **sandboxed probes and hostcheck included** — writes `sessions/<uuid>.json` there (`workspace_label`, `target_eligibility: launch_excluded\|message_capable`, `process_generation_hint: "pid=<pid>"`), a message-capable one also `ms-<id>.sock` + `ms-<id>.sock.lease` (`{"pid":…}`), and a persistent empty `.session-registry.mutation.lock`. A running probe is thus visible to the user's live sessions (`local_session_messaging` is default-ON). Removed on normal exit, **left behind by a kill** (716 stale socket/lease pairs and a registry entry per killed session found on the measuring machine). `Invoker::run` sweeps the entries of a host it killed on timeout (`residue::sweep_killed_pid`); `residue::sweep_dead` removes every entry whose pid is dead (doctor's lane); the mutation lock is never touched |
| shell sandbox | `$TMPDIR/muse-shell-sandbox-<uuid>/` per session (`residue::SHELL_SANDBOX_PREFIX`, `Roots::shell_sandbox_parent`); empty dir, removed on normal exit, left behind by a kill (742 found); carries no pid, so it cannot be attributed and is only named in the uninstall preview |
| workspace settings file | **does not exist** (26 candidates poisoned, none read) |
| project plugin auto-discovery | **does not exist** — plugins must be explicitly installed |
| enterprise | defaults plane live, policy plane inert (`field_not_activated`), delivery dir unknown → do not build on it |
| `muse exec --provider meta … "hi"` | never POSTs: reply `Hi! How can I help you today?` is replayed from an embedded fixture (`rs_6a72aa…`/`Q-PaDg…` literals) after `GET <base>/muse-code/models` only; no `model_request_configured` record. Doctor/cost must use `--provider echo` or a non-trivial prompt (context-slimming.md §7.1) |
| `skills_catalog` measurement unit | UTF-8 bytes of `text` (jq `length` counts chars: 10,015 chars = 10,031 B) (context-slimming.md §0) |
| SKILL.md frontmatter loader | strips surrounding double quotes but does NOT process escapes (`\t`, `\"` kept literally); whitespace runs collapsed to one space (context-slimming.md §3) |
| settings rewrite | the host rewrites `settings.json` from its typed struct: unknown top-level keys, unknown `tui.*` members AND the legacy `mcp_servers` key are all destroyed by the next settings-mutating verb (`muse skills disable bundled:doctor --scope built-in` rewrote `{schema_version, tui, skills}` only, 2026-09-01); `SettingsDoc::validate` warns on all three |

## Exit codes

| Situation | Code |
|---|---|
| argv rejected (parse error OR missing gate — indistinguishable) | 2 |
| parsed, run failed | 1 |
| hidden PTY-gate mode killed by its own timeout | 125 (`hr::EXIT_PTY_GATE_TIMEOUT`; not a product surface). omm never waits for a wedged host: every captured run has a budget (`invoke::RUN_TIMEOUT_DEFAULT` 30 s, `RUN_TIMEOUT_LONG` 120 s for `exec`/`plugins`/`serve`), after which it is killed **together with everything it spawned** — the process tree is snapshotted while the host is alive (`ps -A -o pid=,ppid=,pgid=,stat=`, `residue::ProcessTree`), descendants are `SIGKILL`ed leaves-first, then the host, then one re-walk kills what appeared in between and records what is still alive (`residue::kill_tree`; a plain `SIGKILL` of the host pid left a `SessionStart` hook's `sleep 30` running as an orphan in its own process group, ppid 1, measured 2026-09-02) — reported as `HostError::Timeout` with the partial output and the `survivors`, and its registry entries swept; a grandchild holding the pipes cannot extend a call past `RUN_OUTPUT_GRACE` on a normal exit or `RUN_KILL_OUTPUT_GRACE` after a kill (with every writer dead the EOF is immediate and the partial output complete); each pipe is retained up to `invoke::CAPTURE_CAP_BYTES` (16 MiB) and drained past it (`Outcome::dropped`) — an `exec yes` fake host under a 2 s budget had driven omm to a 12 GB resident set and a 5 s kill when every chunk was kept |
| killed by a signal | no exit code — rendered `killed by signal` (`HostError::Command`), `OutcomeKind::Signal` |
| unknown first token | **not an error** — it is the `[PROMPT]` positional and starts a session; 0/1 decided by workspace trust |
| bare token after root flags | also the `[PROMPT]`: `muse --provider echo hi`, `muse -- hi`, `muse --provider echo -- hi`, a lone `muse -`, and a flag-only argv (`muse --provider echo`, `muse -w`) all start the TUI (escape sequences + exit 1 `Device not configured` on a missing tty, a `session.jsonl` written); root flags may precede ONLY `resume` — `muse --provider echo exec hi` is exit 2 `unknown argument \`exec\``; a `<VALUE>` flag consumes the next token; a `[<VALUE>]` flag consumes it **only when it is exactly one of its listed values** — in a pty (git workspace, `--provider echo --yolo`, 2026-09-02) `muse -w off zebra` / `muse -w create zebra` take the mode and submit `zebra`, `muse -w zebra` creates the worktree (bare `-w` = create) and submits `zebra` as the prompt (8 screen mentions, 5 in `session.jsonl`; `-w` alone 0), `muse -w OFF` leaves `OFF` to the prompt, and an attached `-w=hi` / `--worktree=hi` is exit 2 `invalid value 'hi' for '--worktree'` with no session; `--version` anywhere pre-empts the parse (`muse -w hi --version` prints the version), so it proves nothing about consumption (`muse-cli.json → root_flag_walk_rule`, `RootFlagItem::accepts_value`; `allowlist::validate_argv` refuses `-w hi`, passes `-w off` and `-w=off`) |
| malformed `settings.json` | lazy: `--version --help export init workflows list` succeed; settings-consuming commands fail → probe with `muse skills list` |
| duplicate field in `settings.json` | malformed too, at any depth (typed serde structs): `{"schema_version":1,"tui":{…},"tui":{…}}` → `muse skills list --source user --json` exit 1 `malformed settings file at …: duplicate field \`tui\` at line 1 column 45` (2026-09-02); a plain JSON `Value` parse keeps the LAST occurrence silently, so `SettingsDoc::load` refuses duplicates up front (`settings::reject_duplicate_keys`) instead of committing a de-duplicated document with the first value lost and the last recorded as the ledger prior |
| malformed `trust.json` | the same laziness: `skills list --json` exit 1 `malformed trust store at …: unknown variant \`typo\`, expected \`trusted\` or \`untrusted\`` / `missing field \`schema_version\``, while `--version`, `plugins list --json`, `plugins marketplace list --json`, `plugins remove`, `plugins marketplace remove` still answer (2026-09-02; e2e scenario 27) — so `omm install`'s plan probes with `skills list --json` after mirroring `TrustStore::load` / `SettingsDoc::load` (`lifecycle::refuse_host_config_shape`, `refuse_unloadable_host_config`), and `omm uninstall` degrades to a blind undo when that listing fails instead of stopping |
| typed error in `settings.json` | `{"schema_version":1,"tui":5}` → `skills list --json` exit 1 `malformed settings file at …: invalid type: integer \`5\`, expected struct TuiSettings` — a shape a plain JSON parse cannot see; only the host's loader can, hence the probe (2026-09-02; e2e scenario 27) |
| managed-store directory the lockfile does not list | `skills list --source user --json` still lists it (`provenance: null`, scope `user`); `skills uninstall <id>` exit 1 `{"error":{"code":"provenance-missing","message":"provenance missing for skill \`<id>\`"}}` and the directory stays — the state an install killed inside `skills install` leaves (files copied, `skills/.muse/lock.json` not yet written); omm removes such a directory itself when it is byte-identical to the source (`skills::classify_orphan_store_dir`; 2026-09-02; e2e scenario 24) |
| managed-store directory that is a symlink | `skills list --source user --json` lists it like a directory (the link is followed); `skills uninstall <id>` would act on the target — omm never hands one to the host (`lifecycle::unsafe_store_dir`, the R2 sentinel; 2026-09-02; e2e scenarios 22 and 25) |
| `mcpServers` + `mcp_servers` both present | every settings-**mutating** command exits 1 with `MCP configuration error …; MCP is disabled for this runtime`; read-only lanes silent |
| `run.context_slimming.excluded_tool_names: ["bash"]` without `bash_input` | 1 — `invalid run configuration: \`bash_input\` requires managed \`bash\` to be enabled` |

## Windows

Real but a different tool surface: the bash tool is replaced by a PowerShell tool; a POSIX-only
`command` is dispatched into PowerShell with no static warning; a `.claude-plugin` bundle cannot carry
`commandWindows`. → every native hook ships both `command` and `commandWindows` — **but**
`commandWindows` is NOT accepted inside a native plugin manifest's `capabilities.hooks[]`
(`unsupported-field`, `docs/experiments/marketplace-precedence.md` §7); the Windows twin can only be
carried by the user/project hooks.json tiers. How R16 is met inside `plugins/omm` is an open
design decision (ARCHITECTURE R16 note).

## Server-side risk (cannot be seen offline)

Both builds carry a `feature_config` provider whose override list includes
`extensions.skills.allowed_digests`, `extensions.skills.allowed_kinds`, `extensions.hooks`,
`extensions.runtime_capabilities`. Meta can allowlist extension kinds without shipping a binary.
Doctor must compare what **composed in a live session** against what was installed — never disk vs disk.

Probes and hostcheck measure the *binary*: they run with `TBH_DISABLE_FEATURE_CONFIG=1` and assert the
`feature_config.cache … gate_count=0` trace line (P1 "remote feature-config cache"). The rows that would
move under a populated cache are `gates/default-on`, `gates/default-on-set` and every `bundled/*` row. A
live-session measurement that *wants* the overrides lifts the variable with
`Invoker::env_remove(hr::ENV_DISABLE_FEATURE_CONFIG)`.

## Release log

One entry per observed build; each names only what moved against the previous stable build, and
where it is locked. Everything not named was re-measured and found identical
(`docs/experiments/stable-1.0.3-diff.md` §1–§2 for the axes).

| build | channel | measured | what moved | locked in |
|---|---|---|---|---|
| `1.0.1-R2006.1` | muse-stable | 2026-09-01/02 | baseline of every row above (`research/musecode/*`, `research/experiments/*`, `docs/experiments/*`) | every data file; `hr::*` |
| `1.1.0-R2009.1` | muse-canary | 2026-09-02 | nothing but the version string and the `userAgent` build hash (45 axes; `research/experiments/canary-diff.md`) | — |
| **`1.0.3-R2198.1`** | **muse-stable (pin since 2026-09-05)** | 2026-09-05 | (1) gate `ultra_reasoning_effort` appended, default OFF — 42 gates, 50-line trace; (2) `--reasoning-effort` / `settings.reasoning_effort` / `/effort` gain `max` (plain 8-block run context; 1.0.1 ignored it with a notice); (3) `ultra` now gated: closed → xhigh + notice, open → the 10 blocks 1.0.1 composed unconditionally (orders 183, 187); (4) one more session record (`runtime.retained_fact`, 43 → 44); (5) the enterprise plane's `semantic_invalid` message for `max` drops its `location=` suffix. Build commit `238bb03ff3`, +407,872 B | gates.json n=42 (`since`, `effect_probe`), `counts.gates`; muse-cli.json `values_since`, `version_string`; settings-keys.json row 6; slash-commands.json `/effort`; `hr::GATES_TOTAL`, `BOOTSTRAP_TRACE_FIXED_LINES`, `REASONING_EFFORT_*`, `CONTEXT_ORDER_*_PROACTIVE`; hostcheck `gates/since/*`, `gates/effect/*`, `cli/reasoning-effort*`, `since/*`; `tests/probes.rs`; ci.yml runs the gate on both stable builds |
| `1.1.0-R2074.1` | muse-canary | 2026-09-05 | a different line: no `ultra_reasoning_effort`, no `max`; `todo_reminder` default ON (15); all four MSP hashes; 27/21 tools (`snooze_reminder`, `reminder_roster`); 17 bundled skills / 28 files (`durable-test-collateral`, `requirements-clarification`; `daemon`, `slack-connector` materialised, unlisted), catalog 11,926 B; `exec --max-tool-output-bytes` help `(0 or >=9)`; the same 44th record. **Not folded** — `omm doctor --self-test` reports 13 drifted rows + 3 OLDER-BUILD on it (stable-1.0.3-diff.md §5) | — (watched by the ci.yml `hostcheck` job) |
| **`1.3.0-R3401.1`** | **muse-stable (pin since 2026-09-23)** | 2026-09-23 | (1) gates `subscription_launch` and `context_meter` removed from the trace — 45 gates, trace line 50 for ultra at position 43 (absent even with their env forced on, so removed, not lossy); (2) new order-185 `agent_definition_catalog` context block, trusted and untrusted (10/9 blocks; ultra open is 11, +187 only); (3) MSP `session/listChanged` notification (30 → 31) and four new fingerprints; (4) bundled `CREDITS.md` + `slack-connector/references/slack-ui.md` (40 → 42 files) and five SKILL.md byte changes; (5) `config status` Generation is OS-specific (four source lines macOS, two Linux) — pinned per OS after reproducing the ubuntu value in a clean container | gates.json items/`counts`/ultra note, `hr::GATES_TOTAL`, `CONTEXT_BLOCK_ORDERS*`, `CONTEXT_ORDER_AGENT_DEFINITION_CATALOG`, MSP `*_FINGERPRINT`/`*_SHA256`/`MSP_NOTIFICATIONS`, `BUNDLED_SKILL_PACKAGE_FILES`, `ENTERPRISE_GENERATION` per-OS, bundled-skills.json files/bytes, muse-cli.json pins; hostcheck green |

## Phase 0 experiments

PLAN.md task 0.5. All three ran on `.host/bin/muse-bin-1.0.1-R2006.1`
(sha256 `b9c7f9badb6b2af1b362d30202b366e7bdc13b3c4048e9002caa236cc56c54a4`), 2026-09-02, in a
throw-away `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`, offline; nothing left `127.0.0.1`.

| # | Question (PLAN 0.5) | Verdict | Report | Folded into this file / code |
|---|---|---|---|---|
| (a) | Does `run.context_slimming.skill_catalog_descriptions: "first_sentence"` work, how many bytes does it buy, do the sibling keys do anything, does `config validate` accept them, does the method detect the `workflow_trigger_mode:"off"` control? | **RESOLVED_YES** — all five keys live and typed; first_sentence −52 % on a 40-skill install, built-in tax 10,458 → 4,832 B, room 21,542 → 27,168 B; `full_skill_description_ids` default `["bundled:git"]` is replaced by a user value; `meta_context_note_enabled` has no observable effect in `exec`; `excluded_tool_names:["bash"]` alone fails the run; control detected (−20,413 B). Side findings: `--provider meta "hi"` never POSTs; 20 tools untrusted / 26 trusted; 14 of 15 bundled without the gate | `docs/experiments/context-slimming.md` | Budgets rows (first_sentence, full ids, session identity, meta note, excluded tools, workflow, wire body), P1 tools/bundled rows, Paths rows (meta fixture, byte unit, frontmatter, settings rewrite), Identity rows (`config validate` wrapper, loader), Exit codes row; `hr::BUILTIN_SKILLS_*`, `BUNDLE_BUDGET_FIRST_SENTENCE_BYTES`, `CATALOG_ENTRY_BASE_BYTES_USER`, `CONTEXT_SLIMMING_*`, `SESSION_IDENTITY_BYTES_*`, `WORKFLOW_*`, `ACTIVE_TOOLS_UNTRUSTED`, `CONTEXT_BLOCK_ORDERS_UNTRUSTED`, `SUBAGENT_DELEGATION_BYTES`; hostcheck `tools/active-count-untrusted`, `context/block-orders-untrusted` |
| (b) | Marketplace precedence: both files or first-found, in which order; does a Claude-schema entry pointing at a `.muse-plugin` package install as `native`; duplicate ids; git vs local, does `update` re-clone; what the repo-root files must contain | **RESOLVED_YES** — first-found-wins in the order `marketplace.json` → `.agents/plugins/marketplace.json` → `.claude-plugin/marketplace.json`, no merge; the package's manifest dir decides the family; duplicates keep both, install takes the first; every `update` is a fresh generation (keep 2), installed plugins stay pinned and `plugins update` never follows — `remove` → `install` → re-approve is the only path. Side finding: native plugin hooks reject `commandWindows`. Refutes ARCHITECTURE §1/§5.2 "Muse reads both" | `docs/experiments/marketplace-precedence.md` | "Marketplaces (P1)" section, Paths row (generations), Identity row (hook fields), Windows paragraph; `hr::MARKETPLACE_*`, `PLUGIN_HOOK_REJECTED_FIELDS`; ARCHITECTURE §1/§2/§5.2 corrected; `omm-manifest::generate::marketplace` contract |
| (c) | Does an installed, approved plugin stdio MCP server receive `tools/call` when the model invokes its tool, does the result flow back, in `muse exec` and `muse serve`, and does the 18-char pid+sid rule hold? | **RESOLVED_YES** — proven end to end offline through `tools/mockprovider/`; the wire shape is a namespace group; the 18-char rule is functional (past it `<ns>__<fn>` stops dispatching); unapproved control shows no namespace. Lifts 00-DECISION §4's conditional block on MCP-primary bundles; R19's ≤18 lint must FAIL the build; doctor D1 gains a wire-level oracle | `docs/experiments/mcp-tools-call.md` | Identity rows (tool name, addressing, session.jsonl), Trust lifecycle rows (round-trip, symptom, serve offline), P1 row (round-trip); `hr::MCP_BUILTIN_NAMESPACE`, `MCP_NAMESPACE_DESCRIPTION`, `MCP_CALL_SEPARATOR_*`, `MSP_CLIENT_NAME_PATTERN`, `ENV_META_API_KEY` |

Adversarial review of Gate 0 (2026-09-02) additionally measured and folded in: the lossy bootstrap
trace writer (P1 "experimental gates", gates.json `probe.note`), the root-flag prompt smuggling
(Exit codes "bare token after root flags", muse-cli.json `root_flag_walk_rule`), the projects-less
trust store (Trust lifecycle), and the settings rewrite dropping `tui.*` unknowns and `mcp_servers`
(Paths "settings rewrite"). Its second round added: the per-uid runtime dir and shell sandbox that
every session — sandboxed probes included — writes and a kill leaves behind (Paths "runtime dir",
"shell sandbox"; `crate::residue`), the feature-config trace line being identical with and without
`TBH_DISABLE_FEATURE_CONFIG` (P1 "remote feature-config cache"), the wedged-host budget (Exit codes
125 row; `Invoker::run`), and the lock every writer of `settings.json`/`trust.json` now holds from its
stale check through its rename (`$OMM/locks/muse-config.lock` — two unlocked writers both landed, 4 of 4
rounds, and one update vanished with no backup). Its third round re-measured the `[<VALUE>]` root-flag
rule in a pty — a detached token is consumed only when it is one of the flag's values, so `-w hi`
smuggles a prompt where `-w=hi` is exit 2 (Exit codes "bare token after root flags", muse-cli.json
`root_flag_walk_rule`, `RootFlagItem::accepts_value`) — the host's `duplicate field` verdict on a
repeated settings key (Exit codes "duplicate field"; `SettingsDoc::load`), the unbounded capture that let
a flooding host grow omm to 12 GB (Exit codes 125 row; `invoke::CAPTURE_CAP_BYTES`), and typed the
contract-bearing host-data fields nothing parsed (`hook-events.json protocol.timeout /
plugin_hook_fields / config_hook_fields`, `gates.json accepted_values`, `reserved-ids.json id_grammar.* /
reserved_key_bindings / reserved_reminder_envelope_tags`, `settings-keys.json items[].default /
tui_fields.items[].type`, `muse-cli.json hidden_argv_modes / process_start_validators` — the last now
read through `MuseCli::process_start_validated_env` instead of a Rust literal; `hr::verify_data`).
