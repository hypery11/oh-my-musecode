# Experiment: `run.context_slimming` — does it work, and how much does it buy?

Status: **RESOLVED — the keys work.** Every number below was measured against
`.host/bin/muse-bin-1.0.1-R2006.1` on 2026-09-02 in a throw-away `HOME`/`XDG_CONFIG_HOME`/
`XDG_DATA_HOME`, offline, with `--provider echo` (and, for the wire-level cross-check, a local
mock provider on `127.0.0.1:8731`). Nothing here comes from documentation.

Closes PLAN.md task 0.5(a) and DECISION.md §3 row 2 ("never executed"). Bytes are UTF-8 bytes of
the `text` field of `model_request_configured.run_context_messages[]` in `session.jsonl`, unless
stated otherwise. (`jq '.text|length'` counts characters: the 14 bundled entries are 10,015 chars
but **10,031 bytes** — measure bytes.)

## 0. Verdict table

| Key (`settings.json → run.context_slimming.*`) | Works? | Effect measured |
|---|---|---|
| `skill_catalog_descriptions: "first_sentence"` | **YES** | order-200 `skills_catalog` 23,838 → **11,376 B (−12,462 B, −52.3 %)** with 14 bundled + 40 user skills; every `<description>` cut to its first sentence; `<short-description>`, header (363 B), footer (35 B) untouched |
| `skill_catalog_descriptions: "full"` | YES (default) | byte-identical to no setting (`sha a2a656048816`) |
| `full_skill_description_ids: [...]` | **YES, with a trap** | listed ids keep the full description; match is on the exact display id (`bundled:taste` works, bare `taste`/`plan` does not); unknown ids ignored; **the default value is `["bundled:git"]` and a user value replaces it** — set it and `bundled:git` loses its 740-B two-rule description unless you list it again |
| `session_identity_enabled: false` | **YES** | removes the order-240 `session_identity` block (776–786 B; size varies with the session-log path) |
| `meta_context_note_enabled: false` | **no observable effect** | run context, wire `instructions`, wire developer message and wire tools all byte-identical (modulo session id) with `--provider echo` and with the meta wire format through the mock; the literal `meta_context_note` exists in the binary next to `developer`/`session_start`, so it is a context-source id that never composes in the `exec` lane. TUI lane untested |
| `excluded_tool_names: [...]` | **YES, sharp edges** | removes named tools from `toolset.active_tools`; unknown names silently ignored; `write_todos` cannot be removed; `["bash"]` alone **fails the run** (`invalid run configuration: \`bash_input\` requires managed \`bash\` to be enabled`) — exclude `bash` and `bash_input` together; `["read_skill"]` removes the tool but the catalog (which tells the model to call `read_skill`) still renders; `["workflow"]` removes the tool **and** the order-180 + 181 blocks (−20,952 B) with no replacement block |
| control: `run.workflow_trigger_mode: "off"` | YES | order-181 `workflow_cookbook` (18,056 B) gone; order-180 `workflow_choice` (2,896 B) replaced by `workflow_availability_off` (539 B); `workflow` tool gone (20 → 19 tools); net −20,413 B of run context, −36,308 B on the wire (the `workflow` tool definition is 15,766 B of JSON) |
| `muse config validate --plane defaults` | accepts all five keys and `workflow_trigger_mode` | only inside the enterprise-defaults wrapper `{"schema_version":1,"settings":{"run":{…}}}`; a bare `settings.json` document is `unknown_member`; bad values are `semantic_invalid` / `wrong_type` at `settings.run.context_slimming` |

Combined (`first_sentence` + `session_identity_enabled:false` + `excluded_tool_names:["web_search"]`
+ `workflow_trigger_mode:"off"`): run context **46,973 → 13,318 B (−71.6 %)**; wire request body
**119,298 → 69,417 B (−41.8 %)** for a 40-skill user install.

## 1. Method

- Sandbox per run: `T_<label>/{home,cfg,data,ws}`; env is exactly
  `PATH HOME XDG_CONFIG_HOME XDG_DATA_HOME MUSE_NO_AUTO_UPDATE=1 NO_COLOR=1 META_API_KEY=dummy`
  (`env -i` equivalent). Workspace is an empty untrusted directory; no `--trust-workspace`
  (that only adds the 6 `subagent_*` tools and the 956-B order-186 block — measured: 20 tools
  untrusted, 26 trusted, catalog identical).
- Skills: `$XDG_CONFIG_HOME/muse/skills/cs-NN/SKILL.md` (user scope, not trust-gated), N = 40,
  descriptions 158–265 B (median 243 B, mean 233 B; the 20 plain ones are 265 B each); 20 of
  them are sentence-boundary probes (§3), one
  (`cs-20`) carries `metadata.short-description`. A second set of 7 `px-*` probes exercises real
  tab / LF / CRLF / NBSP characters.
- Run: `muse exec --provider echo hi` in `ws/`; parse the newest
  `data/muse/sessions/*/*/*/*/session.jsonl` → `payload.event.kind == "model_request_configured"`
  → `run_context_messages[]` (order, id, bytes) and `toolset.active_tools`.
- Catalog entries parsed from the `skills_catalog` text with
  `<skill (attrs)>\n<description>…</description>[\n<short-description>…</short-description>]\n</skill>`.
- Wire cross-check: the same runs with `--provider meta --base-url http://127.0.0.1:8731/v1
  --model test-model` against a copy of `tools/mockprovider/mock.py` (its log records the full
  `POST /v1/responses` body). Prompt for that lane: `Reply with the single word ok and nothing
  else. Do not call any tool.` — see §7.1 for why `hi` cannot be used there.

## 2. Bytes, run by run (order 200 unless stated)

Every row is one fresh sandbox. `sha` is the first 12 hex of sha256 of the block text.

| label | skills | settings (`run.*`) | order-200 bytes | entries / with description | other blocks |
|---|---:|---|---:|---|---|
| `base` | 40 | none | **23,838** (`a2a656048816`) | 54 / 54 | 85:251 95:683 96:471 180:2,896 181:18,056 240:778 |
| `sv` | 40 | `{"schema_version":1}` only | 23,838 (`a2a656048816`) | 54 / 54 | identical |
| `full` | 40 | `context_slimming.skill_catalog_descriptions:"full"` | 23,838 (`a2a656048816`) | 54 / 54 | identical |
| `fs` | 40 | `…:"first_sentence"` | **11,376** (`2f05497ef966`) | 54 / 54 | identical |
| `fs_ids` | 40 | `first_sentence` + `full_skill_description_ids:["cs-00","cs-03","bundled:taste","plan","nonexistent-id"]` | 11,436 | 54 / 54 | `cs-00`,`cs-03`,`bundled:taste` full; `bundled:plan` still cut (bare `plan` no match); `bundled:git` **cut** (default list replaced) |
| `fs_ids_empty` | 40 | `first_sentence` + `full_skill_description_ids:[]` | 10,666 | 54 / 54 | only difference from `fs`: `bundled:git` 740 → 30 B |
| `fs_ids_git` | 40 | `first_sentence` + `["bundled:git"]` | 11,376 (`2f05497ef966`) | 54 / 54 | byte-identical to `fs` ⇒ default list is exactly `["bundled:git"]` |
| `ids_only` | 40 | `"full"` + `["cs-00"]` | 23,838 (`a2a656048816`) | 54 / 54 | the id list is a no-op in `full` mode |
| `meta_off` | 40 | `context_slimming.meta_context_note_enabled:false` | 23,838 (`a2a656048816`) | 54 / 54 | all 7 blocks same sizes/shas as `base` |
| `sid_off` | 40 | `context_slimming.session_identity_enabled:false` | 23,838 | 54 / 54 | **order 240 absent**; 6 blocks |
| `excl_ws` | 40 | `excluded_tool_names:["web_search"]` | 23,838 | 54 / 54 | 19 tools (`web_search` gone) |
| `excl_rs` | 40 | `["read_skill"]` | 23,838 | 54 / 54 | 19 tools, `read_skill` gone, catalog still rendered |
| `excl_wf` | 40 | `["workflow"]` | 23,838 | 54 / 54 | 19 tools; **orders 180 and 181 absent, nothing added** (85 95 96 200 240 only) |
| `excl_bash2` | 40 | `["bash","bash_input"]` | 23,838 | 54 / 54 | 18 tools |
| `excl` | 40 | `["web_search","bash","read_skill"]` | — | — | **rc=1**, no model request: `agent loop failed: model failed: invalid run configuration: \`bash_input\` requires managed \`bash\` to be enabled` |
| `excl_todos` | 40 | `["write_todos","subagent_spawn"]` | 23,838 | 54 / 54 | 20 tools — `write_todos` not removable, unknown name ignored |
| `excl_bogus` | 40 | `["not_a_tool"]` | 23,838 | 54 / 54 | 20 tools, no diagnostic |
| `wf_off` | 40 | `workflow_trigger_mode:"off"` | 23,838 | 54 / 54 | 181 absent; 180 = `workflow_availability_off` 539 B; 19 tools |
| `all` | 40 | `wf off` + `first_sentence` + `meta off` + `sid off` + `["web_search"]` | 11,376 (`2f05497ef966`) | 54 / 54 | 85:250 95:682 96:471 180:539 200:11,376 — **total 13,318 B vs 46,973** |
| `base0` | 0 | none | **10,031** | 14 / 14 | the bundled tax (14 visible; `create-plugin` is gated off without `MUSE_EXPERIMENTAL_PLUGINS`) |
| `fs0` | 0 | `first_sentence` | **4,609** | 14 / 14 | bundled tax under slimming (−5,422 B, −54 %) |
| `fs0_ids_empty` | 0 | `first_sentence` + `[]` | 3,899 | 14 / 14 | `bundled:git` also cut |
| `big_full` | 100 | none | 31,890 | 114 / **66** | stage-2 degradation: `cs-52`…`cs-99` (48 entries) rendered `<skill …/>` with no description, no diagnostic, rc=0 |
| `big_fs` | 100 | `first_sentence` | 21,396 | 114 / **114** | every description present |
| `trust_base` | 40 | none, `--trust-workspace` | 23,838 (`a2a656048816`) | 54 / 54 | +186:956, 26 tools |
| `g_base0` | 0 | none, `MUSE_EXPERIMENTAL_PLUGINS=1` | **10,458** | 15 / 15 | = host-reality's 10,060 + 363 + 35 exactly; `bundled:create-plugin` = 426 B (282-B description) |
| `g_fs0` | 0 | `first_sentence`, gate on | **4,832** | 15 / 15 | the 15-entry bundled tax under slimming (−5,626 B, −54 %) |
| `g_base` | 40 | none, gate on | 24,265 | 55 / 55 | |
| `g_fs` | 40 | `first_sentence`, gate on | 11,599 | 55 / 55 | −12,666 B (−52.2 %) |

Per-entry cost, user scope, id `cs-00`, path `$CONFIG_DIR/skills/cs-00/SKILL.md`:
`full` = 316 B + 1 newline with a 207-B description; `first_sentence` = 149 B + 1 with the 40-B
first sentence. Overhead is therefore `36 + len(id) + len(path) + len(desc) + 36` for
`scope="user"` (host-reality's `38 + …` is the `scope="plugin"` form, 2 B longer). The 32,000-B
cap, the 363-B header and the 35-B footer are unchanged by every setting.

## 3. What "first sentence" means, byte-exactly

Rule recovered from 27 probes (§2 `fs`, `p2_fs`): **the description is cut after the first run of
ASCII `.`, `?` or `!` characters that is immediately followed by a space** (after the frontmatter
loader has collapsed every whitespace run — space, tab, LF, CRLF, U+00A0 — to one space). If no
such boundary exists the whole description is kept. Nothing is appended (no ellipsis). Only
`<description>` is affected; `<short-description>` renders in full.

| probe (frontmatter description, abridged) | rendered under `first_sentence` |
|---|---|
| `Zero probe with a plain period boundary. Second …` | `Zero probe with a plain period boundary.` |
| `Does this probe end with a question mark? Second …` | `Does this probe end with a question mark?` |
| `Exclaim the first sentence loudly! Second …` | `Exclaim the first sentence loudly!` |
| `Hey!? Mixed terminators here. Second …` | `Hey!?` (the whole punctuation run is kept) |
| `Wait for it... then act on the ellipsis. Second …` | `Wait for it...` |
| `Use e.g. this probe for abbreviation testing. Second …` | `Use e.g.` — **abbreviations are boundaries** |
| `Mr. Smith's probe uses a title abbreviation. Second …` | `Mr.` |
| `Version 2. Then a digit-period boundary. Second …` | `Version 2.` |
| `Handles v1.2 releases and 3.5 builds cleanly. Second …` | `Handles v1.2 releases and 3.5 builds cleanly.` (no space after `.` ⇒ no boundary) |
| `No space after this period.Second sentence follows here. Third …` | `No space after this period.Second sentence follows here.` |
| `Does the thing (mostly). Second …` | `Does the thing (mostly).` |
| `Ends in paren (see above.) Second sentence follows here. Third …` | `Ends in paren (see above.) Second sentence follows here.` (`.)` is not a boundary) |
| `Say "stop." Then go on. Second …` | `Say &quot;stop.&quot; Then go on.` (`."` is not a boundary) |
| `Colon: not a boundary; semicolon neither. Second …` | `Colon: not a boundary; semicolon neither.` |
| `第一句以全角句号结束。第二句在这里。 Third …` | unchanged (191 B) — `。` is not a terminator |
| `Only one sentence … under the mode?` (nothing after) | unchanged (158 B) |
| `No terminator at all …, and a very long tail …` | unchanged (204 B) |
| `Two spaces follow this period.  Second …` | `Two spaces follow this period.` (double space already collapsed by the loader) |
| `Tab<TAB>after period.<TAB>Second …` / `…period.\nSecond …` / `…CRLF.\r\nSecond …` / `…space.<NBSP>Second …` | cut at that period — all four whitespace kinds are normalised to a space first |
| `First sentence is exactly this. Second. Third. Fourth. …` | `First sentence is exactly this.` |
| bundled `bundled:git` (740 B, "Two rules apply whether or not you read the body…") | kept in full **only** through the default `full_skill_description_ids = ["bundled:git"]` |
| bundled `bundled:taste` (450 B) | `Mandatory preflight for web frontend visual design.` (51 B) |
| bundled `bundled:browser-app-delivery` (1,753 B) | 85 B |

Side facts from the same probes (relevant to `omm lint`, not to slimming):

- `muse skills list --json` still reports the full description under `first_sentence`
  (`cs-00` → 207 B) with `diagnostics: []`. The cut is render-time only; there is no oracle for it
  except the session log.
- The SKILL.md frontmatter loader strips surrounding double quotes but does **not** process
  escapes: `"Tab\tafter"` renders as the four characters `Tab\tafter`, `\"` renders as `\"`
  (then `\&quot;`). Authors must not JSON-escape descriptions.
- XML escaping (`&apos; &quot; &amp; &lt; &gt;`) is applied to the rendered text and counts
  toward the budget (`cs-07`: 248 B rendered from a 216-B source).

## 4. Validation and parsing oracles

`muse config validate --plane defaults --file <doc>` (rc 0 = valid, 1 = invalid):

| document | result |
|---|---|
| `{"schema_version":1,"settings":{"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence"}}}}` | `valid: plane=defaults schema_version=1` |
| same with `"full"`, `full_skill_description_ids:[…]`, `meta_context_note_enabled:false`, `session_identity_enabled:false`, `excluded_tool_names:[…]`, all five at once, `workflow_trigger_mode:"off"` | valid |
| `skill_catalog_descriptions:"bogus"` | `enterprise_document_invalid: plane=defaults reason=semantic_invalid location=settings.run.context_slimming` |
| `full_skill_description_ids:"cs-00"` / `meta_context_note_enabled:"no"` / `session_identity_enabled:0` / `excluded_tool_names:"web_search"` | `reason=wrong_type location=settings.run.context_slimming` |
| `context_slimming.bogus_key` | `reason=unknown_member` |
| `workflow_trigger_mode:"sometimes"` | `reason=semantic_invalid location=settings.run.workflow_trigger_mode` |
| **bare user shape** `{"schema_version":1,"run":{…}}` | **`reason=unknown_member`** — the defaults plane only accepts the `settings` container |
| policy plane, any of the above | `unknown_member` |
| wrapper with `hooks` / `plugins` / `runtime_capabilities` / `mcpServers` / `model_catalog` / `permissions` | `unknown_member` (`mcp_servers`, `skills`, `provider`, `model`, `tui.keymap` are accepted; `tui.theme` → `field_not_activated`) |

Consequence for ARCHITECTURE R10 / §3: the settings writer must validate
`{"schema_version":1,"settings":<patched-subset>}` and must first strip the nine
`SettingsFileDocument` members the enterprise validator does not know (29 user members vs 20
`EnterpriseDefaultsSettingsV1` members). Validating the whole user file fails on a pristine
install that has `hooks` or `plugins`.

Real loader (`$XDG_CONFIG_HOME/muse/settings.json` then `muse skills list --json`), which proves
each field is a typed serde field rather than an ignored key:

| document | result |
|---|---|
| any valid combination | rc 0, `diagnostics: []` |
| `skill_catalog_descriptions:"bogus_mode"` | rc 1 `malformed settings file at …: unknown variant \`bogus_mode\`, expected \`full\` or \`first_sentence\`` |
| `full_skill_description_ids:"cs-00"` | `invalid type: string "cs-00", expected a sequence`; `[1]` → `invalid type: integer \`1\`, expected a string` |
| `meta_context_note_enabled:"no"` / `session_identity_enabled:0` | `expected a boolean` |
| `excluded_tool_names:"web_search"` | `expected a sequence` |
| duplicated `skill_catalog_descriptions` / `full_skill_description_ids` / `meta_context_note_enabled` / `session_identity_enabled` / `excluded_tool_names` / `context_slimming` / `workflow_trigger_mode` | `duplicate field \`<name>\`` (all 7 real) |
| `context_slimming.bogus_key`, duplicated or not | rc 0 — unknown keys inside the struct are silently ignored |

String-table confirmation (`strings -n 6 muse-bin | grep`): `SkillCatalogDescriptionMode` →
`full`,`first_sentence`; `struct ContextSlimmingSettings` / `struct ContextSlimmingDefaultsV1 with
5 elements`; the five field names appear in `SettingsFileDocument with 29 elements` and in
`RunDefaultsV1 with 11 elements`; a standalone `bundled:git` literal sits in the skills rodata
(the default exemption).

## 5. Wire-level cross-check (mock provider, `POST /v1/responses`)

Same 40-skill install, prompt `Reply with the single word ok and nothing else. Do not call any
tool.`, `--provider meta --base-url http://127.0.0.1:8731/v1 --model test-model`,
`META_API_KEY=dummy`. Only network destination: the mock (with nothing listening the run dies at
`failed to fetch model catalog: transport error … http://127.0.0.1:8731/muse-code/models`).

| variant | body | `instructions` | developer message (all context blocks joined by `\n\n`) | `tools` (one `namespace` tool `muse`, 20 functions) |
|---|---:|---:|---:|---:|
| base | 119,298 | 36,709 | 46,997 | 35,136 |
| `meta_context_note_enabled:false` | 119,310 (+12 = longer sandbox path) | 36,709 | 47,009 (identical after path/session-id normalisation) | 35,136 |
| `session_identity_enabled:false` | 118,511 | 36,709 | 46,219 (−778: the `session-identity` reminder) | 35,136 |
| `first_sentence` | 106,830 | 36,709 | 34,529 (−12,468) | 35,136 |
| `excluded_tool_names:["web_search"]` | 118,988 | 36,709 | 46,997 | 34,805 (−331) |
| `workflow_trigger_mode:"off"` | 82,990 | 36,709 | 26,588 (−20,409) | 19,368 (−15,768: the `workflow` function is 15,766 B) |
| all four + `["web_search"]` | **69,417** | 36,709 | 13,334 | 19,037 |

`instructions` (the system prompt, 36,709 B) is untouched by every key. The session-log block
sizes and the wire developer message agree to the byte (blocks + `\n\n` separators).

## 6. What it buys — the budget arithmetic

- **Bundled tax** (15 entries, plugins gate on, header+footer included): 10,458 B → **4,832 B**
  under `first_sentence` (14 entries, gate off: 10,031 → 4,609 B; → 3,899 B if `bundled:git` is
  not re-listed). Room under the 32,000-B cap for everything else: **21,542 B → 27,168 B (+26 %)**
  with the built-ins left on, before touching `muse skills disable bundled:*`. R18's 21,542 is
  reproduced exactly.
- **Per shipped skill** (user scope, 5-char id, 33-char path): 110 B + rendered description. A
  240-char description costs 350 B in `full`; a 60-char first sentence costs 170 B. For plugin
  scope add the two plugin-id occurrences per host-reality's entry formula.
- **Silent-drop rescue**: at 100 user skills (mean 233-B descriptions), `full` renders 31,890 B and drops the
  descriptions of 48 skills with zero diagnostics; `first_sentence` renders 21,396 B with all 114
  descriptions. Past ~70 such skills the setting is the difference between a skill being routable
  and not.
- **Per session** (40-skill install): −12,462 B of catalog, −778 B session identity, −20,413 B by
  `workflow_trigger_mode:"off"` (or −20,952 B by `excluded_tool_names:["workflow"]`), plus
  −15,766 B of `workflow` tool JSON on the wire. Total wire body −41.8 %.
- `first_sentence` changes the design contract for skill authors: **the first sentence is the
  entire trigger surface** the model sees. Negative-trigger clauses ("Do not use for …") that
  live in sentence two or later disappear unless the id is in `full_skill_description_ids`.
  Every bundled skill except `git` (default-exempt) and the single-sentence ones (`grill`,
  `grill-and-record`, `import`) loses its "Use ONLY when… Do NOT use for…" clauses.

## 7. Other facts found on the way

### 7.1 `muse exec --provider meta … "hi"` never calls the model

With the meta provider the prompt `hi` produced `Hi! How can I help you today?` (rc 0) after a
single `GET /muse-code/models` — no `POST /v1/responses`, and no `model_request_configured` record
in `session.jsonl`. The session log carries `reasoning_committed` text
`User says hi. Respond simply.` with `provider_item_id rs_6a72aaa7…:rs_019fcfea…` and an
`encrypted_content` starting `Q-PaDg…`; the same `rs_6a72aa…` / `Q-PaDg…` prefixes are literals in
the binary (`strings` lines ~21066–21068: `how can I help?Need to respond to hi trivially.…`,
`User says hi. Simple greeting. Respond in one line…`). It is an embedded replay fixture for
trivial greetings. No request left the machine. Any doctor/cost probe must use `--provider echo`
(which does assemble and record the run context for `hi`) or a non-trivial prompt.

### 7.2 Tool count depends on trust

Untrusted workspace, echo: 20 tools (no `subagent_*`), no order-186 block. `--trust-workspace`:
26 tools, +`subagent_delegation` 956 B. host-reality's "26" is the trusted number.

### 7.3 `excluded_tool_names` vs `run.toolset`

SYNTHESIS row 34 says `read_skill`/`write_todos`/`subagent_*` are not removable through
`run.toolset`. Through `excluded_tool_names`, `read_skill` **is** removable (catalog still
rendered, so the model is told to call a tool that does not exist); `write_todos` is not.

### 7.4 `excluded_tool_names:["workflow"]` is the cheapest cookbook kill

It removes the `workflow` tool and both order-180 and order-181 blocks and adds nothing
(−20,952 B), whereas `workflow_trigger_mode:"off"` adds the 539-B `workflow_availability_off`
reminder (−20,413 B). The two differ in what the model is told: with `"off"` it is told workflows
are disabled; with the exclusion it is told nothing.

## 8. Repro

All commands assume `M=/Volumes/OWC\ Envoy\ Ultra/omm/.host/bin/muse-bin-1.0.1-R2006.1` and a
scratch root `R`. Each variant is one fresh `R`.

```bash
R=$(mktemp -d)/cs; mkdir -p "$R"/{home,cfg/muse/skills,data,ws}
PAD=" Third sentence pads this description out to roughly two hundred bytes so that the slimming effect is measurable in the rendered catalog."
for i in $(seq -w 0 39); do
  mkdir -p "$R/cfg/muse/skills/cs-$i"
  printf -- '---\nname: cs-%s\ndescription: Standard probe number %s for the omm slimming experiment. It exists only to measure how many bytes the catalog spends per entry.%s\n---\n# cs-%s\n' \
    "$i" "$i" "$PAD" "$i" > "$R/cfg/muse/skills/cs-$i/SKILL.md"
done
# variant: none | first_sentence | ids | sid_off | wf_off | all
echo '{"schema_version":1,"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence"}}}' > "$R/cfg/muse/settings.json"
( cd "$R/ws" && env -i PATH=/usr/bin:/bin HOME="$R/home" XDG_CONFIG_HOME="$R/cfg" XDG_DATA_HOME="$R/data" \
    MUSE_NO_AUTO_UPDATE=1 NO_COLOR=1 "$M" exec --provider echo hi )
S=$(ls -t "$R"/data/muse/sessions/*/*/*/*/session.jsonl | head -1)
python3 - "$S" <<'PY'
import json,sys,re
for line in open(sys.argv[1]):
    try: ev=json.loads(line)["payload"]["event"]
    except Exception: continue
    if ev.get("kind")!="model_request_configured": continue
    for m in ev["run_context_messages"]:
        t=m["text"]; print(m["order"], m["id"], len(t.encode()), "B")
        if m["id"]=="skills_catalog":
            n=len(re.findall(r"<skill ",t)); d=len(re.findall(r"<description>",t))
            print("   entries",n,"with_description",d)
    print("tools", len(ev["toolset"]["active_tools"]), ev["toolset"]["active_tools"])
PY
```

Validator / loader oracles:

```bash
echo '{"schema_version":1,"settings":{"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence"}}}}' > /tmp/d.json
"$M" config validate --plane defaults --file /tmp/d.json      # valid: plane=defaults schema_version=1
echo '{"schema_version":1,"run":{"context_slimming":{"skill_catalog_descriptions":"first_sentence"}}}' > /tmp/d.json
"$M" config validate --plane defaults --file /tmp/d.json      # enterprise_document_invalid: … reason=unknown_member
echo '{"schema_version":1,"run":{"context_slimming":{"skill_catalog_descriptions":"bogus"}}}' > "$R/cfg/muse/settings.json"
( env -i PATH=/usr/bin:/bin HOME="$R/home" XDG_CONFIG_HOME="$R/cfg" XDG_DATA_HOME="$R/data" MUSE_NO_AUTO_UPDATE=1 "$M" skills list --json )
# malformed settings file at …: unknown variant `bogus`, expected `full` or `first_sentence`
```

Wire capture: run `python3 tools/mockprovider/mock.py` (copy it somewhere writable first — it logs
next to itself) with a `respond.py` that returns the Responses-API SSE stream
(`response.created`, `response.output_item.done`, `response.completed`, `data: [DONE]`, each frame
with `sequence_number`), then the same `exec` with
`--provider meta --base-url http://127.0.0.1:8731/v1 --model test-model` and
`META_API_KEY=dummy`, prompt as in §5; read `mock.log` → the `POST` row's `body`.

## 9. Recommended follow-ups (for the orchestrator, not done here)

1. ARCHITECTURE R18 / DECISION rule 3: the budget can be stated with `first_sentence` on —
   room 27,168 B with built-ins on (measured 4,832-B tax, 15 entries), and the packer/lint must budget the
   **first sentence** of each shipped description, not its full length, and must keep every
   negative-trigger clause in sentence one or list the id in `full_skill_description_ids`.
2. Any profile that sets `full_skill_description_ids` must include `"bundled:git"` or knowingly
   drop Meta's two-rule git guard from the catalog.
3. ARCHITECTURE §3 settings writer: wrap in `{"schema_version":1,"settings":{…}}` and strip
   `hooks plugins runtime_capabilities mcpServers model_catalog permissions` before
   `config validate`; a `tui.theme` patch cannot be validated (`field_not_activated`).
4. `omm cost` / D8: never `--provider meta` with `hi` (§7.1); prefer `excluded_tool_names:
   ["workflow"]` over `workflow_trigger_mode:"off"` when the goal is bytes, `"off"` when the goal is
   telling the model why.
5. `excluded_tool_names` in a profile must never list `bash` without `bash_input` (hard run
   failure) and should not list `read_skill` while any skill is installed.
