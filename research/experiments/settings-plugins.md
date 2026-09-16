# SETTLED — what `settings.plugins` holds (Meta Muse Code 1.0.1-R2006.1)

Sandbox: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/settings-plugins/`
(`HOME=<D>/home`, `XDG_CONFIG_HOME=<D>/cfg`, `XDG_DATA_HOME=<D>/data`, `MUSE_NO_AUTO_UPDATE=1`,
`--provider echo` everywhere; no login, no network, nothing written outside the scratchpad).

---

## 0. Verdict

**RESOLVED.** `settings.plugins` is a **real, deserialized field** of `SettingsFile`
(`struct SettingsFileDocument with 29 elements`). Its Rust type is
`Option<Box<serde_json::value::RawValue>>` — an opaque JSON passthrough at load time — and its
**consumer is the plugin runtime-capability enablement resolver**, which reads it as:

```jsonc
"plugins": {
  "<plugin-id>": { "enabled": <bool> }        // unknown sibling fields are ignored
}
```

It is the **canonical** half of a live, in-progress migration. `settings.runtime_capabilities`
(keyed by the full stable id `plugin:<plugin>:<kind>:<capability>`) is the **legacy** half. In
1.0.1 the legacy half is the one that actually *decides*; the canonical half is read and
**cross-checked**, and a disagreement force-disables the capability:

```
runtime capability `plugin:tbh-reminders:reminder:skill-reminder` is inactive: legacy and canonical reminder settings conflict
runtime capability `plugin:tbh-reminders:reminder:goal-reminder`  is inactive: reminder enablement settings are invalid
```

Exactly **two plugin ids are wired in 1.0.1**: `skill-reminder` and `goal-reminder`. Both are
first-party plugins whose manifests are embedded in the binary as part of `specs/16590` — the
extraction of the six bundled `tbh-reminders` reminders into six standalone plugins.

**Nothing in the product ever writes `settings.plugins`.** `plugins install / enable / disable /
approve / reject`, the TUI plugin drawer toggle and the TUI reminder toggle all write
`$DATA/plugins/installed.json` or `settings.runtime_capabilities` only. Whatever you put in
`settings.plugins` survives every settings rewrite **byte for byte**.

---

## 1. Step 1 — the duplicate-key oracle: the field is really deserialized

```bash
D=<sandbox>; . $D/env.sh
printf '%s' '{"schema_version":1,"plugins":1,"plugins":2}' > "$CFG/settings.json"
cd $D/ws && "$M" skills list --source user --json
```

```
malformed settings file at <CFG>/settings.json: duplicate field `plugins` at line 1 column 41
```

Controls, same command, same run:

| document | result |
|---|---|
| `{"schema_version":1,"tui":{},"tui":{}}` | `duplicate field \`tui\` at line 1 column 34` |
| `{"schema_version":1,"plugins":{},"plugins":{}}` | `duplicate field \`plugins\` at line 1 column 42` |
| `{"schema_version":1,"hooks":{},"hooks":{}}` | `duplicate field \`hooks\` at line 1 column 38` |
| `{"schema_version":1,"permissions":{},"permissions":{}}` | `duplicate field \`permissions\` at line 1 column 50` |
| `{"schema_version":1,"runtime_capabilities":{},"runtime_capabilities":{}}` | `duplicate field \`runtime_capabilities\` at line 1 column 68` |
| `{"schema_version":1,"model_catalog":{},"model_catalog":{}}` | `duplicate field \`model_catalog\` at line 1 column 54` |
| `{"schema_version":1,"schema_version":1}` | `duplicate field \`schema_version\` at line 1 column 36` |
| **`{"schema_version":1,"zzzunknown":1,"zzzunknown":2}`** | **rc=0, silent** ← unknown keys are not in `FIELDS` |

So `plugins` is in the derived `FIELDS` array, exactly like every other real member, and unlike an
unknown key. **PROVEN: it is a genuine `SettingsFile` member, not a derived or ignored name.**

The serde `FIELDS` slice recovered verbatim from the binary (adjacent literal run, `SettingsFile`
marker) — note `plugins` sits between `permissions` and `managed_hooks_path`, and that
`mcp_servers` is a 30th alias entry over 29 struct fields:

```
SettingsFile schema_version agents agent_definitions provider model reasoning_effort
first_turn_minimal_effort_regex context_compaction run provider_retry tui context
local_session_messaging feature_config tools skills model_catalog mcpServers mcp_servers
presets hooks runtime_capabilities permissions plugins managed_hooks_path
managed_hooks_env_vars max_consecutive_stop_hook_continuations endpoint_transport telemetry
notifications
```

---

## 2. Step 2 — the type is `Option<Box<RawValue>>` (round-trip oracle)

Muse rewrites `settings.json` from its typed struct on any settings mutation (e.g.
`muse skills disable doctor --scope built-in`). Round-tripping deliberately non-canonical JSON
separates `RawValue` from `serde_json::Value` from a typed struct:

```bash
printf '%s' '{"schema_version":1,"plugins":{"a":1,"a":2, "z" :  3.50}}' > "$CFG/settings.json"
"$M" skills disable doctor --scope built-in
cat "$CFG/settings.json"
```

| key | input value | value after the binary rewrote the file | verdict |
|---|---|---|---|
| **`plugins`** | `{"a":1,"a":2, "z" :  3.50}` | **`{"a":1,"a":2, "z" :  3.50}`** — duplicate key kept, `3.50` kept, whitespace kept | **`RawValue`** |
| `permissions` | same | same, byte-identical | `RawValue` |
| `hooks` | same | `{\n "a": 2,\n "z": 3.5\n }` (pretty, deduped) | `serde_json::Value` |
| `runtime_capabilities` | same | `{\n "a": 2,\n "z": 3.5\n }` | `Value` |
| `model_catalog` | same | `{\n "a": 2,\n "z": 3.5\n }` | `Value` |
| `tui` | same | `{}` — unknown keys dropped | typed struct |

Extra round-trips confirming `Option<…>` and rawness:

| in | out |
|---|---|
| `"plugins":null` | **field disappears** (JSON null → `None`) |
| `"plugins":1.50000` | `1.50000` (a `Value::Number` would print `1.5`) |
| `"plugins":"é😀"` | `"é😀"` unescaped |
| `"plugins":[1,  2,   3]` | `[1,  2,   3]` |
| `"_marker_unknown":"KEEPME"` (unknown key) | **destroyed** — confirms the prior report's warning |

Consequence: **any JSON whatsoever is accepted at settings-load time**; the settings loader never
type-checks `plugins`. That is why four earlier candidate shapes produced no error — they *couldn't*.

---

## 3. Step 3 — finding the consumer (the positive control that cracked it)

`permissions` is the *other* `RawValue` member and it **does** have a consumer, which announces
itself the moment it touches the value. That gave a template for what a `plugins` consumer would
look like:

```bash
printf '%s' '{"schema_version":1,"permissions":"garbage"}' > "$CFG/settings.json"
"$M" exec --provider echo hi
# muse: Named permission profiles are unavailable: invalid type: string "garbage",
#       expected struct UserPermissionSettingsV1 at line 1 column 9.
```

The same `"garbage"` under `plugins` is silent in **every** CLI lane
(`exec`, `skills list`, `plugins list|inspect|validate`, `init`, `config status`, `sandbox check`,
`session-message list`, `trace`, `auth`, `export`), with the plugins gate on **and** off, with a
plugin installed and enabled, and with a hook+MCP plugin installed and approved.

**It only speaks in the TUI.** Under the pty harness (`drive2.py`, adapted from
`sandbox/tui/drive.py`), `settings.plugins = "garbage"` produces at session open:

```
runtime capability `plugin:tbh-reminders:reminder:skill-reminder` is inactive: reminder enablement settings are invalid
runtime capability `plugin:tbh-reminders:reminder:goal-reminder`  is inactive: reminder enablement settings are invalid
```

Corroborating literals in the binary, adjacent inside the config crate's blob:

```
… duplicate key  remote file log  paths  enabled
legacy and canonical reminder settings conflict
reminder enablement settings are invalid
settings.tui.keymap  tui.keymap  settings.skills.activation.user.<id> …
```

and, in the plugins/TUI blob:

```
… name plugin marketplace  missing plugin id
plugins.enablement  enabled  plugin_id  stable_id  definition_hash  blocking_reminder
invalid plugin blocking reminder state
```

---

## 4. Step 4 — the exact shape (probed against the TUI oracle)

Oracle A — validity: any `runtime capability … is inactive: reminder enablement settings are invalid`.
Oracle B — recognition: pair the candidate with a *legacy* entry that disagrees; a recognized,
well-formed canonical value yields `legacy and canonical reminder settings conflict`.

### 4.1 Top level must be an object

| `plugins` value | result |
|---|---|
| `{}` , `{"anything":…}` | valid |
| `"garbage"` , `0` , `true` , `[]` , `[1,2,3]` , `null` (per key) | **invalid** → both wired capabilities go inactive |

### 4.2 Keys are **plugin ids**; only two are wired in 1.0.1

`{"<k>":"str"}` (a deliberately wrong value type) errors only when `<k>` is a wired plugin id:

| key | error raised? |
|---|---|
| `skill-reminder` | **yes** — names `plugin:tbh-reminders:reminder:skill-reminder` |
| `goal-reminder` | **yes** — names `plugin:tbh-reminders:reminder:goal-reminder` |
| `memory-reminder`, `todo-reminder`, `verify-reminder`, `scope-reminder`, `memory`, `todo`, `goal`, `verify`, `scope` | no |
| `tbh-reminders`, `ohmy`, `hookdemo`, `pre-check`, `workspace-index` | no |
| `plugin:tbh-reminders:reminder:skill-reminder` (stable id as key) | no |
| `tbh-reminders:skill-reminder`, nested `{"tbh-reminders":{"skill-reminder":…}}` | no |

The pair `{skill-reminder, goal-reminder}` is **hard-wired**: it does not move when reminder gates
are flipped (`MUSE_EXPERIMENTAL_{SKILL,GOAL,MEMORY,VERIFY,SCOPE,TODO}_REMINDER=0/1`) nor with
`MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=0`. The binary contains the literal pair verbatim:

```
… the login request was denied  goal-reminder  skill-reminder  session stream has an unresolved strict retained append …
```

**Why those two:** the binary embeds six complete first-party plugin manifests, one per bundled
reminder, all `specs/16590 identity` — this is the migration in flight:

| embedded manifest `name` | `description` |
|---|---|
| `skill-reminder` | `Advisory skill-reminder plugin (specs/16590 identity; X2 dormant skeleton).` |
| `goal-reminder` | `Blocking goal-reminder plugin (specs/16590 identity; specs/20698 class F extracted implementation).` |
| `memory-reminder` | `Advisory memory-reminder plugin (specs/16590 identity; #21385 post-X3 extracted implementation).` |
| `todo-reminder` | `Advisory todo-reminder plugin (specs/16590 identity; #21386 post-X3 extracted implementation).` |
| `verify-reminder` | `Blocking verify-reminder plugin (specs/16590 identity; #21387 post-X3 extracted implementation).` |
| `scope-reminder` | `Advisory scope-reminder plugin (specs/16590 identity; X3 dormant skeleton).` |

All six declare `"capabilities": {"skills":[],"hooks":[],"mcpServers":[],"commands":[],"reminders":[],"developerPrompts":[]}`
— i.e. dormant skeletons carrying only an identity. `skill-reminder` (X2) and `goal-reminder`
(class F) are the two whose enablement is already routed through `settings.plugins`; the other
four are staged for X3 and are not yet read.

Note `developerPrompts` — a **seventh capability family** not in the published
`native-plugin-contract.md` list (`skills, commands, hooks, mcpServers, reminders`). It also
appears in the compose trace as `developer_prompts=0`.

### 4.3 The value struct

| value for a wired key | verdict |
|---|---|
| `{}` | valid, **no opinion** (no conflict even against an explicit legacy entry) |
| `{"enabled":true}` / `{"enabled":false}` | valid, compared against legacy |
| `{"enabled":"x"}` | **invalid** → capability inactive |
| `{"enabled":true,"zzzunknown":1}` | valid — **unknown fields ignored** (no `deny_unknown_fields`) |
| `{"enabled":true,"trusted_definition_hash":"x"}` / `"definition_hash"` / `"blocking_reminder":true` | valid (all ignored today) |
| `"str"`, `1`, `[]`, `true`, `null` | **invalid** |

So: `Map<PluginId, { enabled: Option<bool>, … }>`. Only `enabled` is read in 1.0.1.

### 4.4 The canonical/legacy interaction matrix (PROVEN, all four cells)

`legacy` = `settings.runtime_capabilities["plugin:tbh-reminders:reminder:skill-reminder"].enabled`
`canonical` = `settings.plugins["skill-reminder"].enabled`

| legacy | canonical | TUI startup notice | plugin drawer → TBH Reminders → Runtime |
|---|---|---|---|
| absent | absent | — | `trusted_enabled  active next run` |
| absent | `false` | — | `trusted_enabled  active next run` ← **canonical alone does nothing** |
| absent | `true` | — | `trusted_enabled  active next run` |
| `false` | absent | — | `trusted_disabled  disabled` ← **legacy alone decides** |
| `false` | `false` | — | `trusted_disabled  disabled` |
| `false` | `true` | `legacy and canonical reminder settings conflict` → **inactive** | `trusted_disabled  disabled` |
| `true` | `false` | `legacy and canonical reminder settings conflict` → **inactive** | — |
| `true` | `true` | — | active |
| any | malformed | `reminder enablement settings are invalid` → **inactive** | unchanged |

The drawer row shows the *trust store* state; the startup notice shows the *runtime decision*. The
notice is the authoritative oracle.

Legacy key form matters: only the full stable id `plugin:<plugin>:<kind>:<capability>` is honoured.
The short alias `plugin:tbh-reminders:skill-reminder` (which exists as a literal in the binary's
alias run) is **not** honoured by `runtime_capabilities` — planting it left the reminder
`trusted_enabled active next run`.

### 4.5 Scope: reminders only

A purpose-built plugin `hookdemo` with one `PreToolUse` hook and one stdio MCP server was built,
validated, installed and approved. Neither `plugins:{"hookdemo":{"enabled":false}}` nor
`plugins:{"pre-check":…}` / `{"workspace-index":…}` nor a wholesale `plugins:"garbage"` changed the
hook's or MCP server's `review_needed`/approved status or produced any diagnostic. Likewise
`plugins:{"ohmy":{"enabled":false}}` left `ohmy` `active` in `plugins list --json` and `[x] ohmy
active` in the drawer. **In 1.0.1 `settings.plugins` reaches only the reminder-enablement resolver.**

---

## 5. Step 5 — the write side: nothing writes it

| action | what it wrote |
|---|---|
| `muse plugins install <path> --json` | `$DATA/plugins/installed.json` only. `settings.json` stayed `{"schema_version":1}` |
| `muse plugins enable/disable <id>` | `installed.json` `enabled` flag only |
| `muse plugins approve/reject <id[:kind:cap]>` | `settings.runtime_capabilities["plugin:hookdemo:hook:pre-check"] = {"enabled":true,"trusted_definition_hash":"sha256:2770030d…"}` |
| TUI `/plugins` → Installed → space | `installed.json` only; `"plugins":{"KEEP":"me"}` preserved verbatim |
| TUI `/plugins` → detail → Runtime → space on a reminder | `settings.runtime_capabilities["plugin:tbh-reminders:reminder:skill-reminder"] = {"enabled":false,"trusted_definition_hash":"sha256:2801674f…"}`; `"plugins":{"KEEP":"me"}` preserved verbatim |
| `muse skills enable/disable` | `settings.skills.activation.*`; `plugins` preserved verbatim |

Enterprise cannot set it either:

```bash
MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG=1 "$M" config validate --plane defaults --file f.json
# {"schema_version":1,"settings":{"plugins":{}}}      → enterprise_document_invalid: plane=defaults reason=unknown_member
# {"schema_version":1,"settings":{"mcp_servers":{}}}  → valid: plane=defaults schema_version=1
# policy: {"schema_version":1,"extensions":{"plugins":{}}} → enterprise_document_invalid: plane=policy reason=unknown_member
```

`muse schema generate-json-schema --out DIR [--experimental]` and `generate-ts`: the MSP wire
schema contains **no** `plugins`/`plugin` identifier at all — settings are not projected over MSP.

---

## 6. Reproduce from scratch

```bash
S=<scratchpad>; D=$S/settle/settings-plugins
mkdir -p $D/{home,cfg,data,ws,pkg}
export MUSE_NO_AUTO_UPDATE=1 HOME=$D/home XDG_CONFIG_HOME=$D/cfg XDG_DATA_HOME=$D/data
export M=$S/muse-aarch64-macos CFG=$XDG_CONFIG_HOME/muse DATA=$XDG_DATA_HOME/muse
mkdir -p "$CFG"; cd $D/ws

# (1) the field is real
printf '%s' '{"schema_version":1,"plugins":1,"plugins":2}' > $CFG/settings.json
"$M" skills list --source user --json          # duplicate field `plugins` at line 1 column 41

# (2) the field is a RawValue (byte-preserving, null-collapsing)
printf '%s' '{"schema_version":1,"plugins":{"a":1,"a":2, "z" :  3.50}}' > $CFG/settings.json
"$M" skills disable doctor --scope built-in; cat $CFG/settings.json   # value unchanged verbatim

# (3) installs never touch it
export MUSE_EXPERIMENTAL_PLUGINS=1
cp -R $S/sandbox/tui/ohmy-plugin $D/pkg/ohmy
printf '%s' '{"schema_version":1}' > $CFG/settings.json
"$M" plugins install $D/pkg/ohmy --json >/dev/null; cat $CFG/settings.json   # {"schema_version":1}

# (4) the consumer — needs a pty; drive2.py = sandbox/tui/drive.py with HOME/XDG/WS repointed at $D
cat > $D/sc_short.json <<'EOF'
[["wait",4]]
EOF
printf '%s' '{"schema_version":1,"plugins":{"skill-reminder":{"enabled":true}},
 "runtime_capabilities":{"plugin:tbh-reminders:reminder:skill-reminder":{"enabled":false}}}' \
  | tr -d '\n' > $CFG/settings.json
python3 $D/drive2.py $D/sc_short.json $D/final.raw env:MUSE_EXPERIMENTAL_PLUGINS=1
python3 $D/graw.py $D/final.raw
# skill-reminder -> legacy and canonical reminder settings conflict

printf '%s' '{"schema_version":1,"plugins":"garbage"}' > $CFG/settings.json
python3 $D/drive2.py $D/sc_short.json $D/g.raw env:MUSE_EXPERIMENTAL_PLUGINS=1
python3 $D/graw.py $D/g.raw
# goal-reminder -> reminder enablement settings are invalid ; skill-reminder -> …
```

Helper scripts left in place: `env.sh`, `probe.sh`, `rw.sh`, `comp.sh`, `plist.sh`, `p.sh`, `pr.sh`,
`pre.sh`, `pd.sh`, `tui.sh`, `drive2.py`, `render.py`, `graw.py`, `gerr.py`, `gall.py`, `sum.py`,
`sc_*.json`.

---

## 7. Failed attempts / dead ends, in order

1. **Four "obvious" enablement shapes against the CLI** — `{"ohmy":{"enabled":false}}`,
   `{"ohmy":false}`, `{"enabled":{"ohmy":false}}`, `{"activation":{"ohmy":"off"}}`,
   `{"disabled":[…]}`, `{"enabled":[…]}`, `{"marketplaces":{}}`, `{"autoUpdate":false}`,
   `"garbage"`, `12345`, `[1,2,3]`. `muse plugins list --json` returned
   `ohmy en=True valid=True active=True scope=installed-plugin diag=[]` for **all** of them.
2. **The `plugin_capability_snapshot.compose` trace as an oracle.** `plugins approve ohmy` composes
   the snapshot and logs
   `outcome="ready" … installed_admitted=true bundled_plugins=3 installed_plugins=1 plugin_ids=10
   skills=15 hooks=0 mcp_servers=0 commands=2 reminders=6 … rejected=0 conflicts=0 omitted=0`.
   That line is **byte-identical across 15 different `settings.plugins` values**, including scalars.
   The composer does not consult the field. (`muse exec` composes no snapshot at all;
   `plugins list/inspect/enable/disable` write no trace file.)
3. **`muse config validate --plane defaults|policy`** — `plugins` is `unknown_member` on both
   planes, so the enterprise validator names nothing.
4. **`muse schema generate-json-schema/generate-ts` (stable and `--experimental`)** — zero hits for
   `plugin` anywhere in the MSP wire schema.
5. **Binary symbol table** — stripped: `nm` yields 462 entries, all undefined imports. No
   `PluginsSettings`/`PluginSettings`/`SettingsPlugins` struct-name literal exists (every other
   settings sub-struct does: `TuiSettings`, `McpServerSettings`, `SkillsSettings`, `ToolSettings`,
   `TelemetrySettings`, `ProviderRetrySettings`, `RuntimeCapabilityState`, `HookStateConfig`,
   `EndpointTransportConfig`…). This nearly led to a wrong "inert placeholder" verdict.
6. **The settings key registry** (`settings.tui.*`, `settings.tools.web_fetch.*`,
   `settings.skills.activation.*`, `settings.mcp_servers.<id>`, `settings.presets.<name>`) contains
   **no `settings.plugins.*` entry** — also misleading; the canonical store is deliberately outside
   the enterprise-projectable key space.
7. **`muse exec` / every headless lane** — silent for every `plugins` value. Concluding "unused"
   from headless evidence alone would have been wrong.
8. **The TUI plugin drawer's Discover / Installed / Marketplaces / Errors tabs** — all four are
   byte-identical with `plugins` set to `{"ohmy":{"enabled":false}}`, `{"ohmy":false}`,
   `{"tbh-reminders":{"enabled":false}}`, `{"enabled":{"ohmy":false}}`, `"garbage"`, `[1,2,3]`.
   The Errors tab still reads `0 issues / Plugin store OK`. The signal is only in the **startup
   notice strip**, which the rendered-screen diff clipped to two lines — grepping the raw pty byte
   stream was necessary.
9. **Space on the built-in "TBH Reminders" row** in the Installed tab — that row has no checkbox;
   the key falls through to the filter box.
10. **`plugins inspect tbh-reminders`** — `{"error":{"code":"unknown-plugin","message":"plugin
    \`tbh-reminders\` is not installed"}}`. Bundled plugins are not CLI-inspectable, so the CLI
    cannot be used as a reminder-state oracle.
11. **`session.jsonl` as an oracle** — an echo-provider TUI turn writes only
    `{'retained_frame': 1, 'schema_version': 43}`; no reminder records, so no signal.
12. **Guessing the key namespace** — nested `{"<plugin>":{"<capability>":…}}`, stable ids
    (`plugin:tbh-reminders:reminder:skill-reminder`), colon short ids
    (`tbh-reminders:skill-reminder`), and reminder capability ids that differ from the plugin id
    (`memory` — the *capability* is `memory`, the *plugin* is `memory-reminder`) all silently do
    nothing.
13. **Gate flipping to widen the wired set** — `MUSE_EXPERIMENTAL_TODO_REMINDER=1`,
    `MEMORY_REMINDER=1`, `VERIFY_REMINDER=1`, `SCOPE_REMINDER=1`, `SKILL_REMINDER=0`,
    `GOAL_REMINDER=0`, `HOOK_SELECTED_SKILLS=0`, `HOOK_SELECTED_SKILLS_APPLY=0`: the affected pair
    stays exactly `{skill-reminder, goal-reminder}` in every combination.

Still open (small): why `verify-reminder`, whose embedded manifest is likewise a
`post-X3 extracted implementation`, is not yet wired while `goal-reminder` (class F) is. The
`X2 / class F / post-X3 / X3` staging language in the manifests is the likely answer but was not
executed.

---

## 8. Corrected `settings.json` key table (29 members, `schema_version` must be `1`)

Method: `printf '{"schema_version":1,"<key>":12345}'` then `muse skills list --source user --json`
(load-time errors) **and** `muse exec --provider echo hi` + a pty TUI open (consumer-time errors).

| # | key | load-time type | notes / defaults |
|---|---|---|---|
| 1 | `schema_version` | `u32`, **must be 1** | mandatory; `2` → `unsupported settings schema version 2` |
| 2 | `agents` | `an agents settings object` | `execution_capacity`: integer 1–64 |
| 3 | `agent_definitions` | `an Agent Definition settings object` | |
| 4 | `provider` | string | `echo` \| `meta`; default `meta` |
| 5 | `model` | string | |
| 6 | `reasoning_effort` | string | `none\|minimal\|low\|medium\|high\|xhigh\|ultra`; default `high` |
| 7 | `first_turn_minimal_effort_regex` | string | invalid → `tbh: ignoring invalid first_turn_minimal_effort_regex … in settings` |
| 8 | `context_compaction` | `struct ContextCompactionSettings` | 5 fields |
| 9 | `run` | `struct RunConfigurationSettings` | 13 fields |
| 10 | `provider_retry` | `struct ProviderRetrySettings` | 4 fields |
| 11 | `tui` | `struct TuiSettings` | 15 fields; **unknown sub-keys are dropped on rewrite** |
| 12 | `context` | `struct ContextSettings` | `foreign_personal_rules`, `foreign_personal_skills` |
| 13 | `local_session_messaging` | `struct LocalSessionMessagingSettings` | `enabled` |
| 14 | `feature_config` | `struct FeatureConfigSettings` | `enabled` |
| 15 | `tools` | `struct ToolSettings` | `artifact`, `web_search`, `web_fetch` |
| 16 | `skills` | `struct SkillsSettings` | `{ activation: { user\|projects\|bundled\|plugin } }` |
| 17 | **`model_catalog`** | **lazy → `a sequence`** | *corrects the prior report*: `"x"` → `tbh: ignoring malformed model_catalog in settings: invalid type: string "x", expected a sequence`. Array of `ModelCatalogRow` (14 fields). Malformed is **ignored**, not fatal |
| 18 | `mcpServers` | lazy | map of `McpServerSettings` (13 fields) |
| 19 | `mcp_servers` | lazy, **also a real `FIELDS` entry** | present in the user file too; per the prior report, having *both* silently discards all MCP config |
| 20 | `presets` | `a map` → `struct RunPresetSettings` | typed at load |
| 21 | **`hooks`** | lazy → **must be an object** | `settings.json: MalformedConfig: hooks must be an object`; TUI status `Hooks: 0 runnable · 1 warning`. Unknown top-level hook keys ignored |
| 22 | **`runtime_capabilities`** | lazy → object; values `struct RuntimeCapabilityState` | root non-object → `runtime capability state \`<root>\` is malformed; capability is inactive: runtime_capabilities must be an object`. Value non-object → `invalid type: integer \`1\`, expected struct RuntimeCapabilityState`. Fields: `enabled`, `trusted_definition_hash`, `trusted_blocking_definition_hash`. Keys: **full stable id only** (`plugin:<plugin>:<kind>:<capability>`) |
| 23 | **`permissions`** | lazy → `struct UserPermissionSettingsV1` | `deny_unknown_fields`: `unknown user permissions field \`zzz\``. `schema_version` **required**, must be 1 (`unsupported permissions.schema_version 2`); `profiles`: map → `PermissionProfileDefinitionInputV1` (`unknown field \`profiles.p.zzz\``); `default_profile`: string. Failure is non-fatal: `muse: Named permission profiles are unavailable: …` |
| 24 | **`plugins`** | **lazy → `Map<PluginId, {enabled: bool, …}>`** | **this report.** Wired plugin ids in 1.0.1: `skill-reminder`, `goal-reminder`. Non-object root, or a non-object / non-bool-`enabled` entry for a wired id → that capability goes inactive with `reminder enablement settings are invalid`. Disagreeing with the legacy `runtime_capabilities` entry → `legacy and canonical reminder settings conflict`. Unknown sibling fields ignored. Never written by the product; preserved byte-for-byte |
| 25 | `managed_hooks_path` | string | superseded by env `TBH_MANAGED_HOOKS_PATH` |
| 26 | `managed_hooks_env_vars` | `a sequence` | |
| 27 | `max_consecutive_stop_hook_continuations` | `usize` (typed, fatal on wrong type) | |
| 28 | `endpoint_transport` | `struct EndpointTransportConfig` | 9 fields incl. `env_http_headers` |
| 29 | `telemetry` | `struct TelemetrySettings` | 9 fields |
| 30 | `notifications` | `struct NotificationSettings` | `events`, `method`, `condition` |

(29 struct fields + the `mcp_servers` alias = 30 `FIELDS` entries.)

Two rules that bite:
* A wrong **type** on any *typed* key makes the **whole file** malformed and every subcommand exits 1.
* A wrong type on a *lazy* key (`model_catalog`, `mcpServers`, `mcp_servers`, `hooks`,
  `runtime_capabilities`, `permissions`, `plugins`) loads fine and degrades only that subsystem.

---

## 9. What this changes for oh-my-musecode

1. **There is no free desired-state config.** `settings.plugins` is *not* a general
   "which plugins are enabled" array in 1.0.1 — it reaches only two first-party reminder plugin ids
   and it cannot even disable them on its own. A framework still has to synthesise desired state and
   drive it through `muse plugins install/enable/disable` (→ `$DATA/plugins/installed.json`) and
   `muse plugins approve/reject` (→ `settings.runtime_capabilities`).
2. **`omm doctor` gains two new checks, both cheap and both real footguns:**
   * `settings.plugins` present but not an object, or a wired id whose `enabled` is not a bool →
     `skill-reminder` and `goal-reminder` silently go inactive **in the TUI only**, with no CLI
     symptom at all. This is exactly the "my reminders stopped working and nothing says why" bug.
   * `settings.plugins["skill-reminder"].enabled` disagreeing with
     `settings.runtime_capabilities["plugin:tbh-reminders:reminder:skill-reminder"].enabled` (same
     for `goal-reminder`) → `legacy and canonical reminder settings conflict`, capability inactive.
     A framework that writes only one of the two stores creates this state.
3. **If oh-my-musecode ever wants to control reminders declaratively, it must write BOTH stores in
   agreement** — the legacy stable-id entry (which decides) and the canonical plugin-id entry (which
   is cross-checked). Writing canonical-only is a no-op; writing legacy-only works today but will
   break when X3 lands.
4. **`settings.plugins` is the one settings key that survives the rewrite verbatim** — the only
   `RawValue` member besides `permissions`. It is nevertheless **not** a safe place for framework
   state: it is a live product key with two reserved ids today and four more (`memory-reminder`,
   `todo-reminder`, `verify-reminder`, `scope-reminder`) reserved for X3. Framework state stays in
   `~/.config/omm/`, as the synthesis already concluded.
5. **The install tier is unchanged**: `omm install` writes `installed.json` via the CLI and
   `runtime_capabilities` via `plugins approve`; it must **not** invent a `settings.plugins` block.
   `omm remove`/`omm uninstall` must leave any user-authored `settings.plugins` untouched.
6. **Manifest generator note:** the embedded first-party manifests declare a seventh capability
   family, `developerPrompts`, absent from `native-plugin-contract.md`'s supported list and visible
   in the compose trace as `developer_prompts=N`. Worth a follow-up before freezing the generator.
7. **Version pinning matters.** This is a migration in flight (`specs/16590`, `specs/20698`,
   `#21385-21387`). `muse-canary` 1.1.0 is very likely to wire the remaining four ids and may flip
   which store decides. The `omm doctor` binary-version check should treat 1.0.1-R2006.1 as the
   tested version for these two rules.

---

# Verification

**Adversarial re-run by a second agent, from a clean sandbox**
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/verify-settings-plugins/`
(fresh `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`, empty `installed.json`, no reuse of the original
sandbox; `MUSE_NO_AUTO_UPDATE=1`, `--provider echo`, no login, no network).
Binary re-checked: `muse --version` → `Muse Code 1.0.1 (1.0.1-R2006.1)`.

**Verdict: CONFIRMED in substance, CORRECTED in five supporting claims.**
The central answer — *what `settings.plugins` holds, its type, its key space, its semantics, and
that nothing in the product writes it* — reproduced **exactly**, including error strings and column
numbers, and survived four attacks the original run did not make. Five auxiliary claims are
overstated or wrong, and three of them change the `omm doctor` design.

---

## V1. What reproduced exactly (clean room)

| claim | result |
|---|---|
| `plugins` is a real deserialized `SettingsFile` field | ✅ `duplicate field \`plugins\` at line 1 column 41`, same column |
| unknown-key control silent | ✅ plus **three new near-miss controls** `plugin`, `pluginz`, `Plugins` — all rc=0 silent |
| type is an opaque `RawValue` | ✅ `{"a":1,"a":2, "z" :  3.50}` survives the rewrite byte-for-byte; `null` → field gone; `1.50000` → `1.50000`; `[1,  2,   3]` keeps its whitespace |
| `permissions` also `RawValue`; `hooks`/`runtime_capabilities`/`model_catalog` are `Value`; `tui` is a typed struct; unknown keys destroyed | ✅ all six round-trips identical to the report |
| `muse plugins install` writes nothing to `settings.json` | ✅ `{"schema_version":1}` unchanged; only `$DATA/plugins/installed.json` |
| `plugins enable/disable` write nothing to `settings.json` | ✅ |
| CLI is silent for every `plugins` value | ✅ `plugins list --json` row byte-identical across `{"ohmy":{"enabled":false}}`, `{"ohmy":false}`, `"garbage"`, `[1,2,3]`, `12345` |
| positive control passes | ✅ `permissions:"garbage"` → `muse: Named permission profiles are unavailable: invalid type: string "garbage", expected struct UserPermissionSettingsV1` in the *same* `exec` lane where `plugins:"garbage"` is silent |
| TUI baseline control | ✅ `{"schema_version":1}` → **no** `runtime capability … inactive` notice at all |
| `plugins:"garbage"` → both wired ids invalid | ✅ verbatim |
| `plugins:{}` → nothing | ✅ |
| `{"skill-reminder":"str"}` → names `plugin:tbh-reminders:reminder:skill-reminder` | ✅ |
| `{"goal-reminder":"str"}` → names `plugin:tbh-reminders:reminder:goal-reminder` | ✅ |
| every other candidate id inert | ✅ one document carrying `todo-reminder, scope-reminder, memory, todo, verify, scope, goal, skill, loop, muse-core, zzuser, zzcap, zzskill` → `(none)` |
| the 12-row shape table (§4.1–4.3) | ✅ **all 12 cells** reproduced: `{}` / `{"skill-reminder":{}}` no-opinion, `{"enabled":true}` conflict, `{"enabled":"x"}`/`true`/`null` invalid, `{"enabled":true,"zzzunknown":1}` conflict (unknown fields ignored), nested and stable-id keys inert, `[1,2,3]`/`0` invalid |
| the 9-cell canonical/legacy matrix (§4.4) | ✅ **all 9 cells** reproduced, including the asymmetry that `legacy=absent, canonical=false` is silent while `legacy=true, canonical=false` conflicts |
| canonical alone cannot disable | ✅ drawer stays `trusted_enabled active next run`; legacy alone flips it to `trusted_disabled disabled` |
| `plugins approve` writes only `runtime_capabilities` | ✅ |
| TUI reminder toggle writes only `runtime_capabilities` and preserves `plugins` verbatim | ✅ `"plugins": {"KEEP":"me","x":[1, 2,  3.50]}` came back byte-identical |
| enterprise `unknown_member` on both planes, positive control `mcp_servers` valid | ✅ (`runtime_capabilities` is **also** `unknown_member` — neither store is enterprise-settable) |
| MSP schema has zero `plugin` hits (stable and `--experimental`) | ✅ |

---

## V2. Attacks the original run did not make

### Attack A — build a real third-party reminder plugin (the decisive one)

The original run concluded "scope is reminders only" from a **hook + MCP** plugin. That does not test
the interesting case. `references/capability-examples.json` contains a complete executable
`reminders` example, so I built one.

```
zzuser/.muse-plugin/plugin.json   name=zzuser, reminders:[{id:review-policy, decision:{…}}]
muse plugins validate  -> valid:true, diagnostics:[]
muse plugins install   -> settings.json unchanged
muse plugins approve   -> settings.runtime_capabilities["plugin:zzuser:reminder:review-policy"]
                          = {enabled:true, trusted_definition_hash:"sha256:d246a5d1…"}
```

It landed on `status: blocked / reminder_envelope_elevated_approval`, so I cleared that in the TUI
(`/plugins` → Installed → zzuser → Runtime → `enter approve elevated`), reaching
`Reminder review-policy  trusted_enabled  active next run`. **Then** I poisoned the canonical store:

```
plugins value                                      inspect zzuser status   TUI notice
(absent)                                           trusted_enabled         (none)
"garbage"                                          trusted_enabled         only skill-reminder + goal-reminder go invalid
{"zzuser":"str"}                                   trusted_enabled         (none)
{"zzuser":{"enabled":false}}  (legacy = true)      trusted_enabled         (none)   <- no conflict
{"zzuser":{"enabled":true}}   (legacy = true)      trusted_enabled         (none)
```

A fully activated, fully trusted third-party reminder capability is **completely untouched** by any
`settings.plugins` value, including a `"garbage"` root that simultaneously kills both wired ids in
the same process. **The report's scope claim is not an artifact of picking the wrong capability
family — it is correct, and now proven on the family that matters.**

Two more families checked the same way: a skill-only plugin (`zzskill`, one bundled `SKILL.md`)
stayed `plugin:zzskill:review  on` under `{"zzskill":{"enabled":false}}`, `{"zzskill":"str"}` and
`"garbage"`; the command-only `ohmy` row was unchanged (matching the report).

### Attack B — is the key space really the *plugin id*?

The report inferred this from six manifests embedded in the binary. It is now a **product-observable
fact**. Installing a plugin under each candidate name and reading `plugins inspect <id> --json`:

```
skill-reminder   warning=installed plugin `skill-reminder` uses a reserved bundled plugin id;
                 the bundled plugin wins and the installed plugin contributes nothing
goal-reminder    (same)      memory-reminder (same)     todo-reminder (same)
verify-reminder  (same)      scope-reminder  (same)     tbh-reminders (same)
loop             (same)      muse-core       (same)
memory           third-party plugin: reminders require review before activation   <- NOT reserved
zzcontrol        third-party plugin: reminders require review before activation   <- control
```

diagnostic code `bundled_plugin_id_reserved`, `effective_capabilities: []`, `runtime_capabilities: []`.

So all six extracted reminder ids **are already registered bundled plugin ids in 1.0.1**, and
`settings.plugins`'s keys are drawn from that namespace. This also kills the competing hypothesis
that the key is the *reminder capability id*: the bundled capability ids are
`memory · skill-reminder · todo-reminder · goal-reminder · verify-reminder · scope-reminder`
(read off the drawer), and `memory` — a real capability id that is **not** a bundled plugin id — is
inert, while `memory-reminder` — a bundled plugin id that is not a capability id — is also inert
(reserved, not yet wired). Two further discriminators were built and installed:
a plugin literally *named* `skill-reminder` (contributes nothing, reserved) and a plugin `zzcap`
whose reminder capability id is `skill-reminder` (`plugin:zzcap:reminder:skill-reminder`, approved) —
neither is reachable through `settings.plugins`.

**New omm constraint:** nine plugin ids are reserved (`skill-reminder`, `goal-reminder`,
`memory-reminder`, `todo-reminder`, `verify-reminder`, `scope-reminder`, `tbh-reminders`, `loop`,
`muse-core`). A plugin generated with one of them installs successfully, reports `valid:true
active:true`, and **contributes nothing** — a warning, not an error.

### Attack C — two new ways to break "canonical alone does nothing"

1. **Legacy store non-empty but with no entry for the id.**
   `plugins:{"skill-reminder":{"enabled":false}}` +
   `runtime_capabilities:{"plugin:tbh-reminders:reminder:goal-reminder":{"enabled":false}}`
   → `skill-reminder` stays `trusted_enabled active next run`, no notice. So the comparison is not
   "canonical vs the legacy store", it is "canonical vs *this capability's* legacy entry", and an
   absent entry means the canonical value is never read. That is why canonical alone is inert.
2. **A default-off capability.** The drawer shows `scope-reminder` is `default off` /
   `trusted_disabled disabled` out of the box. `plugins:{"scope-reminder":{"enabled":true}}` does not
   enable it (it is not a wired id), and no wired id is default-off, so there is no configuration in
   which the canonical store alone can change an outcome.
3. **The trace lane.** TUI runs under `plugins` absent / canonical-false / legacy-false produce
   **byte-identical** `local-tracing` output (17 lines, no reminder records at all), so the trace
   cannot be used to claim a hidden runtime effect either way.

**"Canonical alone is a no-op" holds.**

### Attack D — new write lanes

`muse init`, `muse plugins marketplace add`, `muse plugins remove --delete-data`,
`muse skills enable`, `muse plugins install --scope project`, the TUI **elevated** approval, and
`muse plugins approve|reject <bundled stable id>` were all run against a settings file containing
`"plugins":{"KEEP":"me"}`. Every one of them left the value byte-identical. Elevated approval writes
`trusted_blocking_definition_hash` — **into `runtime_capabilities`, not `plugins`**:

```
"runtime_capabilities": { "plugin:zzuser:reminder:review-policy": {
    "enabled": true,
    "trusted_definition_hash":          "sha256:d246a5d11076ebc3f4cc5b03f5a053a7a159bcf792a198ea89536d5f5bbcb200",
    "trusted_blocking_definition_hash": "sha256:d246a5d11076ebc3f4cc5b03f5a053a7a159bcf792a198ea89536d5f5bbcb200" } }
```

**"Nothing in the product writes `settings.plugins`" holds**, now over 12 write lanes instead of 6.

---

## V3. Corrections

### C1 — "surfaces ONLY in the TUI; every CLI lane is completely silent" is too strong

`muse plugins inspect <id> --json` exposes the **full trust diagnostic** for installed plugins:

```json
"runtime_capabilities": [{ "candidate": {…}, "status": "blocked",
  "diagnostic": { "code": "reminder_envelope_elevated_approval",
                  "stable_id": "plugin:zzuser:reminder:review-policy",
                  "message": "runtime capability `plugin:zzuser:reminder:review-policy` is inactive: reminder envelope requires current elevated approval" }}]
```

Same struct, same `is inactive:` message shape as the TUI notice. The wired pair is invisible in the
CLI **only** because bundled `tbh-reminders` is not `plugins inspect`-able — not because the CLI
lacks the surface. The complete `RuntimeCapabilityTrustDiagnosticCode` enum, recovered from the
binary as one literal run, gives omm stable machine codes to key on instead of message text:

```
malformed_state · empty_stable_id · empty_definition_hash · modified_definition_hash ·
duplicate_stable_id · malformed_reminder_decision · malformed_reminder_settings ·
reminder_setting_conflict · explicit_reminder_roster · blocking_plugin_reminder ·
blocking_plugin_reminder_missing_install_limit · reminder_envelope_elevated_approval
```

`malformed_reminder_settings` = "reminder enablement settings are invalid";
`reminder_setting_conflict` = "legacy and canonical reminder settings conflict".

### C2 — "the CLI cannot serve as a reminder-state oracle at all" is wrong, and the footgun is worse

`plugins inspect tbh-reminders` does fail, but **`muse plugins approve|reject
plugin:tbh-reminders:reminder:<id>` works from the CLI** on the bundled reminders and echoes the
resulting state:

```
$ muse plugins reject plugin:tbh-reminders:reminder:skill-reminder --json
{"decision":"reject","runtime_capabilities":[{"stable_id":"plugin:tbh-reminders:reminder:skill-reminder",
  "trusted_definition_hash":"sha256:2801674fad2b0b2c453a5d1c3c88c9fc4ddf36a467558e75783267465fd0c4d2","enabled":false}]}
```

And with `plugins:{"skill-reminder":{"enabled":false}}` already in the file,
`muse plugins approve plugin:tbh-reminders:reminder:skill-reminder` **silently manufactures the
conflict state** — it writes `enabled:true`, prints `"decision":"approve"`, and says nothing. The
resulting file disables the capability at the next TUI start. This is a stronger version of the
report's design point 2(b): the conflict is not only reachable by a framework writing one store, it
is reachable by a **single documented product command**.

### C3 — `reminder enablement settings are invalid` does not identify which store is broken

The report attributes this message to a malformed **canonical** value. A malformed **legacy** store
raises the identical message for the identical two ids:

```
{"schema_version":1,"runtime_capabilities":"x"}
  runtime capability state `<root>` is malformed; capability is inactive: runtime_capabilities must be an object
  runtime capability `plugin:tbh-reminders:reminder:skill-reminder` is inactive: reminder enablement settings are invalid
  runtime capability `plugin:tbh-reminders:reminder:goal-reminder`  is inactive: reminder enablement settings are invalid

{"schema_version":1,"runtime_capabilities":{"a":1}}          <- one unrelated malformed entry
  runtime capability state `a` is malformed; capability is inactive: invalid type: integer `1`, expected struct RuntimeCapabilityState
  runtime capability `plugin:tbh-reminders:reminder:skill-reminder` is inactive: reminder enablement settings are invalid
  runtime capability `plugin:tbh-reminders:reminder:goal-reminder`  is inactive: reminder enablement settings are invalid
```

Note the second case: **a single malformed entry anywhere in `runtime_capabilities` — under a key
that has nothing to do with reminders — takes both wired reminders down.**
`omm doctor` check 2(a) must therefore lint **both** stores, and must not report "your
`settings.plugins` is malformed" on the strength of the message alone.

### C4 — `mcpServers` / `mcp_servers` are **not** `SettingsFile` members

Running the duplicate-key oracle over all 30 claimed names with an object value
(`{"schema_version":1,"<k>":{},"<k>":{}}`), every claimed key answers — 23 with
`duplicate field \`<k>\``, 6 with `invalid type: map` (typed scalars: `provider`, `model`,
`reasoning_effort`, `first_turn_minimal_effort_regex`, `managed_hooks_path`,
`managed_hooks_env_vars`, `max_consecutive_stop_hook_continuations`) — **except**:

```
mcpServers    <silent, rc=0>
mcp_servers   <silent, rc=0>
zzz_control   <silent, rc=0>      <- unknown-key control, same answer
```

They behave exactly like an unknown key in this deserializer, so §1's "30 `FIELDS` entries over 29
struct fields, `mcp_servers` is the alias" is not supported. They are nevertheless real settings,
read by a **separate pass** over the file with its own error channel (see C5). The
`SettingsFile`-proper member count is 28, and `plugins` is one of them.

### C5 — malformed `mcpServers` is **fatal to every settings-mutating command**, not "degrade only"

§8's rule "a wrong type on a lazy key loads fine and degrades only that subsystem" fails here:

```
{"schema_version":1,"mcpServers":"x"}            skills list -> silent rc=0 ;  exec -> "echo: hi" rc=0
                                                 skills disable doctor --scope built-in ->
   MCP configuration error in "<CFG>/settings.json"; MCP is disabled for this runtime      rc=1
{"schema_version":1,"mcpServers":{"a":1}}        same
{"schema_version":1,"mcp_servers":{"a":1}}       same
{"schema_version":1,"mcpServers":{},"mcp_servers":{}}   same   <- both keys present, both empty
```

Read lanes stay quiet; **any command that rewrites `settings.json` refuses to run**. Any omm
settings writer must handle this: it will not be able to write, and the message names MCP, not the
key it was trying to set. (It also sharpens the prior report's "having both silently discards all
MCP config" — having both is an outright configuration error, even when both are `{}`.)

### C6 — lane attribution in the §8 table

`model_catalog`, `hooks` and `runtime_capabilities` malformed-type messages are **TUI-only**, exactly
like the `plugins` ones — they do not appear in `muse exec` or `muse skills list`:

```
TUI: tbh: ignoring malformed model_catalog in settings: invalid type: string "x", expected a sequence
TUI: settings.json: MalformedConfig: hooks must be an object   ·   Hooks: 0 runnable · 1 warning
exec/skills list: silent, rc=0
```

Confirmed corrections that stand as written: `model_catalog` is a **sequence** (`{"a":1}` →
`invalid type: map, expected a sequence`), `permissions` messages (`unsupported
permissions.schema_version 2`, `unknown user permissions field \`zzz\``, `expected struct
PermissionProfileDefinitionInputV1`) all reproduce **in `exec`**, and
`max_consecutive_stop_hook_continuations` / `presets` are fatal at load.

### C7 — the "canonical vs legacy" labelling is an inference, not a measurement

The binary says only `legacy and canonical reminder settings conflict`. Which store is which is
inferred from the `plugins.enablement · enabled · plugin_id · stable_id · definition_hash ·
blocking_reminder` literal run and the six embedded manifests. **No behaviour distinguishes the two
assignments**, so the labelling cannot be proved from this binary. The operationally safe statement,
which *is* proved: *the plugin-id store never decides and is only cross-checked; the stable-id store
decides.* (Attack B's reserved-id evidence makes the report's reading the strongly favoured one.)

### C8 — small factual notes

* Bundled reminder **capability** ids are `memory`, `skill-reminder`, `todo-reminder`,
  `goal-reminder`, `verify-reminder`, `scope-reminder` — `memory`, not `memory-reminder`.
* `scope-reminder` ships **`default off` / `trusted_disabled disabled`**; the other five are
  `default on` / `trusted_enabled active next run`. The report's matrix never mentions a default-off
  reminder exists.
* A third-party reminder whose envelope uses a **reserved tag** (`<system-reminder>`) gets
  `requiresElevatedApproval=true` / `elevatedApprovalState=required` and **cannot be activated from
  the CLI at all** — `muse plugins approve` leaves it `blocked`; only the TUI's
  `enter approve elevated` clears it. This is a hard limit on any headless omm install tier that
  ships reminder capabilities.

---

## V4. Net effect on the design impact section

* Points **1, 3, 4, 5, 7** stand unchanged (and 1 is now much better evidenced).
* Point **2(a)** must widen: lint **both** stores. A malformed `runtime_capabilities` — including a
  single bad entry under an unrelated key — produces the *same* `reminder enablement settings are
  invalid` outcome. Prefer the machine codes `malformed_reminder_settings` /
  `reminder_setting_conflict` over message matching.
* Point **2(b)** must widen: `muse plugins approve plugin:tbh-reminders:reminder:<id>` creates the
  conflict on its own, silently. Doctor should also flag `runtime_capabilities` entries whose
  `plugin:` prefix is a reserved bundled id but whose canonical partner disagrees.
* **New:** doctor should refuse to generate, and warn on, plugin ids in the reserved set
  {`skill-reminder`, `goal-reminder`, `memory-reminder`, `todo-reminder`, `verify-reminder`,
  `scope-reminder`, `tbh-reminders`, `loop`, `muse-core`} — such a plugin installs "successfully"
  and contributes nothing.
* **New:** omm's settings writer must detect the `MCP configuration error … MCP is disabled for
  this runtime` state, because in it every `muse` command that would write `settings.json` exits 1.
* **New:** omm cannot ship an auto-activating reminder capability that uses reserved tags; that
  requires a manual TUI elevated approval.

---

## V5. Reproduce this verification

```bash
S=/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad
V=$S/settle/verify-settings-plugins
rm -rf $V; mkdir -p $V/{home,cfg,data,ws,pkg,out}
export MUSE_NO_AUTO_UPDATE=1 HOME=$V/home XDG_CONFIG_HOME=$V/cfg XDG_DATA_HOME=$V/data
export M=$S/muse-aarch64-macos CFG=$XDG_CONFIG_HOME/muse DATA=$XDG_DATA_HOME/muse
mkdir -p "$CFG"; cd $V/ws
"$M" --version                                   # Muse Code 1.0.1 (1.0.1-R2006.1)

# --- V1: the field, its type, near-miss controls -----------------------------
for k in plugins plugin pluginz Plugins zzzunknown; do
  printf '%s' "{\"schema_version\":1,\"$k\":1,\"$k\":2}" > $CFG/settings.json
  echo "$k -> $("$M" skills list --source user --json 2>&1 | head -1)"
done
printf '%s' '{"schema_version":1,"plugins":{"a":1,"a":2, "z" :  3.50},"_marker_unknown":"KEEPME"}' > $CFG/settings.json
"$M" skills disable doctor --scope built-in >/dev/null; cat $CFG/settings.json

# --- V1: FIELDS sweep (C4) ---------------------------------------------------
for k in agents tui skills model_catalog mcpServers mcp_servers hooks runtime_capabilities \
         permissions plugins endpoint_transport telemetry notifications zzz_control; do
  printf '%s' "{\"schema_version\":1,\"$k\":{},\"$k\":{}}" > $CFG/settings.json
  printf '%-24s %s\n' "$k" "$("$M" skills list --source user --json 2>&1 \
      | grep -o 'duplicate field `[^`]*`\|invalid type: map' | head -1)"
done                                            # mcpServers / mcp_servers print nothing

# --- C5: malformed mcpServers blocks every settings write --------------------
printf '%s' '{"schema_version":1,"mcpServers":{},"mcp_servers":{}}' > $CFG/settings.json
"$M" skills disable doctor --scope built-in; echo "rc=$?"   # MCP configuration error …  rc=1

# --- Attack A: a real third-party reminder plugin ----------------------------
export MUSE_EXPERIMENTAL_PLUGINS=1
python3 - <<'PY'
import json
S="/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad"
V=S+"/settle/verify-settings-plugins"; import os
ex=json.load(open(S+"/re/artifacts/create-plugin/references/capability-examples.json"))
r=[e for e in ex["examples"] if e["family"]=="reminders"][0]
m=json.loads(json.dumps(r["manifest"])); m["name"]="zzuser"; m["displayName"]="ZZ User Reminder"
m["capabilities"]["reminders"][0]["decision"]["deliveryRole"]="user"
os.makedirs(V+"/pkg/zzuser/.muse-plugin",exist_ok=True); os.makedirs(V+"/pkg/zzuser/reminders",exist_ok=True)
open(V+"/pkg/zzuser/.muse-plugin/plugin.json","w").write(json.dumps(m,indent=2))
for p,c in r["files"].items(): open(V+"/pkg/zzuser/"+p,"w").write(c)
PY
printf '%s' '{"schema_version":1}' > $CFG/settings.json
"$M" plugins validate $V/pkg/zzuser --json | head -2
"$M" plugins install  $V/pkg/zzuser --json > /dev/null
cat $CFG/settings.json                          # unchanged
"$M" plugins approve  zzuser --json             # writes runtime_capabilities only
"$M" plugins inspect  zzuser --json | python3 -c \
  "import sys,json;[print(c['status'],(c.get('diagnostic') or {}).get('code')) for c in json.load(sys.stdin)['runtime_capabilities']]"
# -> blocked reminder_envelope_elevated_approval   (C1: the CLI *does* carry the diagnostic)

# --- Attack B: reserved bundled plugin ids -----------------------------------
for n in skill-reminder goal-reminder memory-reminder todo-reminder verify-reminder \
         scope-reminder tbh-reminders loop muse-core memory zzcontrol; do
  rm -rf $V/pkg/probe; cp -R $V/pkg/zzuser $V/pkg/probe
  python3 -c "import json,sys;p='$V/pkg/probe/.muse-plugin/plugin.json';m=json.load(open(p));m['name']='$n';json.dump(m,open(p,'w'))"
  "$M" plugins install $V/pkg/probe --json > /dev/null 2>&1
  printf '%-16s %s\n' "$n" "$("$M" plugins inspect $n --json 2>/dev/null | python3 -c \
      "import sys,json;print((json.load(sys.stdin).get('warning') or 'no-warning')[:70])")"
  "$M" plugins remove $n --json > /dev/null 2>&1
done

# --- C2: the CLI manufactures the conflict silently --------------------------
K=plugin:tbh-reminders:reminder:skill-reminder
printf '%s' "{\"schema_version\":1,\"plugins\":{\"skill-reminder\":{\"enabled\":false}}}" > $CFG/settings.json
"$M" plugins approve $K --json                  # "decision":"approve", no warning
cat $CFG/settings.json                          # legacy true vs canonical false  ->  conflict at next TUI start

# --- C3: a malformed LEGACY store raises the same message --------------------
# (TUI lane; drive3.py = sandbox/tui/drive.py with HOME/XDG_*/WS repointed at $V)
printf '%s' '{"schema_version":1,"runtime_capabilities":{"a":1}}' > $CFG/settings.json
printf '[["wait",5]]' > $V/sc_short.json
python3 $V/drive3.py $V/sc_short.json $V/out/c3.raw env:MUSE_EXPERIMENTAL_PLUGINS=1
python3 $V/graw.py $V/out/c3.raw
# skill-reminder -> reminder enablement settings are invalid ; goal-reminder -> …
```

Helper scripts left in `$V`: `env.sh`, `drive3.py`, `render.py`, `graw.py`, `gall.py`,
`gnotice.py`, `sc_short.json`, `sc_tbh2.json`, `sc_toggle.json`, `sc_p{1..5}.json`,
`stringsa.txt` (`strings -a -n 6` of the binary), `REPRO.sh`; raw pty captures in `$V/out/`.

## V6. Attempts that produced nothing (for the record)

1. **`local-tracing` as a reminder oracle.** A TUI session writes 17 bootstrap lines and no reminder
   records at all; `plugins` absent / canonical-false / legacy-false are byte-identical after
   stripping timestamps. Confirms the original run's dead end from a clean sandbox.
2. **Escaping the elevated-approval gate from the CLI.** `deliveryRole:"user"` validates (the
   validator accepts only `developer` or `user` — `assistant`/`system`/`tool`/`none` are rejected
   with `reminder delivery role must be developer or user`) but does **not** clear it; the gate is on
   the envelope's reserved `<system-reminder>` tag, and only the TUI can clear it.
3. **`muse plugins approve skill-reminder`** (bare reserved id) → `runtime-capability-not-found: no
   runtime capabilities match \`skill-reminder\`` — a reserved-id plugin contributes no capabilities
   to approve.
4. **A project-scope settings plane.** `muse plugins install --scope project` writes nothing into the
   workspace, and the trace shows exactly one plane loaded (`settings.load source="user_settings"`).
   There is no second file that could hold a different `plugins` key.
5. **Embedded documentation.** The binary carries the bundled `settings` skill verbatim, which cites
   `runtime_capabilities["plugin:tbh-reminders:reminder:skill-reminder"].enabled` as *the* example key
   path — and never mentions `settings.plugins`. No embedded doc describes the canonical store.
