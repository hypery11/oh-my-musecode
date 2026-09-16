# Meta Muse Code ("TBH") 1.0.1-R2006.1 — Agent definitions, subagents, worktree isolation

Reverse-engineered from the shipped stripped Mach-O arm64 binary
(`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`),
by running it offline (`--provider echo`) inside a throw-away `$HOME`, and by mining
`strings -a -n 4` of the binary.

Every claim below is tagged **PROVEN** (reproduced live) or **INFERRED** (read from an exact
binary string, but not executed).

Sandbox used for all runs:

```
BASE=/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/agents
export MUSE_NO_AUTO_UPDATE=1
export HOME=$BASE/fakehome
export XDG_CONFIG_HOME=$HOME/.config
export XDG_DATA_HOME=$HOME/.local/share
```

---

## 0. TL;DR of the findings

| Question | Answer |
|---|---|
| `--agents <JSON>` shape | A **JSON object keyed by agent name**: `{"<name>": <definition-object-or-prompt-string>, ...}`. It is the `session` definition source. |
| Agent name grammar | `[a-z]+(-[a-z]+)*`, ≤128 UTF-8 bytes. **No digits, no underscores, no leading/trailing/double dash.** |
| Where definitions live | Project: `<workspace-root>/.agents/agents/**/*.md`. User: `$CONFIG_DIR/agents/**/*.md` (= `~/.config/muse/agents`). Nothing else. |
| File format | Markdown with mandatory YAML frontmatter; `name:` is **required** and is authoritative (filename is irrelevant). |
| Composition sources | 6 slots, enum `DefinitionSource` = `managed`, `session`, `project`, `user`, `plugin`, `built_in` (plus a `fixed` marker; `project` carries `project_depth`, `plugin` carries `plugin_id`). |
| Built-in agents | `general-purpose`, `workflow-subagent`. |
| Schema version string | `agent-definition-current-v1` (+3 variants, see §3.1). Parser contract id: `agent_definition_core_v1`. |
| Native subagent tools | `subagent_spawn`, `subagent_status`, `subagent_send_message`, `subagent_wait`, `subagent_read_result`, `subagent_cancel` (+ `subagent_followup_task`). Hidden unless `run.subagent_delegation_mode = "auto"`. |
| Concurrency | `settings.agents.execution_capacity`, integer **1..64**, default 8, counting the root. |
| `--subagent-worktree-isolation` | A **no-op compatibility flag**; the capability is on by default and isolation is requested per child. |
| Worktree location | `<repo>/.muse/worktrees/<YYYYMMDD>-<4hex>` with a reservation store in `<repo>/.muse/worktrees/.session-worktree-reservations/v1/`. |
| `--preset` | Selects a *run preset* = `{provider, model, agent_profile, run}`. Built-ins `native-basic`, `miniswe`; user presets under `settings.presets.<name>`. |
| Agent profiles | `native-basic`, `miniswe`, `code-mode-v1-all-tools`, `code-mode-v2-all-tools`, `code-mode-v2-prefer-generated-bindings`, `code-mode-v2-only`, `code-mode-v2-native-libraries-disabled`, `code-mode-exp-16461-declarations-r1`. |
| Big gotcha | Agent-definition composition **runs only in TUI mode**. `muse exec` never emits `agent_definition.sources_load` and has no `--agents` flag. |

---

## 1. Surface: the CLI flags

### 1.1 `muse --help` (root / TUI) — verbatim excerpt

```
      --agents <JSON>
          Supply one ephemeral agent-definition overlay
      --provider <MODE>
          Startup provider: echo or meta (default: meta)
      --preset <NAME>
          Run a built-in preset: native-basic, miniswe
  -w, --worktree [<MODE>]
          Session Git worktree: off|create|existing; a bare -w means create
          (default: off)
      --worktree-base <REF>
          Base ref for --worktree create (default: HEAD)
      --worktree-existing <PATH>
          Existing worktree path for --worktree existing
      --subagent-worktree-isolation
          Compatibility flag; capability defaults on. Only an affirmative
          per-child request asks for isolation; omission stays shared. Requests
          may reject when capability, provider, or Git prerequisites are
          unavailable.
```

### 1.2 `muse exec --help` — the delta

`muse exec` has `--preset`, `-w/--worktree`, `--worktree-base`, `--worktree-existing`
and `--subagent-worktree-isolation`, but **not `--agents`**. PROVEN:

```
$ muse exec --help | grep -c -- --agents
0
```

And `--agents` is a *root* option, so `muse --agents '{}' exec ...` is rejected:

```
invalid TUI options: unknown argument `exec`
note: `exec` is a command, not a root option — put it first and its flags after it: `muse exec [OPTIONS]`
```

### 1.3 Hidden commands

`MUSE_EXPERIMENTAL_PLUGINS=1` reveals a whole `muse plugins` command tree (install / list /
inspect / **approve** / **reject** / hook test / marketplace / enable / disable / update /
remove / validate). This is the trust-review plane that agent definitions from plugins would
ride on (§3.6). PROVEN.

---

## 2. Observability: how everything below was measured

The binary writes structured bootstrap traces to
`$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-<uuid>.log` on every run, with no env var
needed. The line that drives most of this report is:

```
INFO tbh.local.catalog fbcode/musecode/build/src/crates/config/src/agent_definitions/composition/diagnostic.rs:53
  event="agent_definition.sources_load" outcome="ready" reason="none" safe_mode=false
  sources=6 loaded_sources=6 suppressed_sources=0 source_failed=0 candidates=2
  diagnostics=0 duration_ms=0
```

Field vocabulary recovered from strings (`fbcode/musecode/build/src/crates/config/src/agent_definitions/composition/diagnostic.rs`):

* `event` ∈ `agent_definition.sources_load`
* `outcome` ∈ `ready | not_implemented | session_overlay | project_discovery | source_load | failed`
* plus `reason`, `safe_mode`, `sources`, `loaded_sources`, `suppressed_sources`,
  `source_failed`, `candidates`, `diagnostics`, `duration_ms`.

A minimal PTY harness was needed because the TUI requires a real terminal
(it aborts with `Device not configured (os error 6)` on a pipe and
`The cursor position could not be read within a normal duration` under `script`).
The harness answers the `ESC[6n` DSR query with `ESC[1;1R`.
(`.../sandbox/agents/ptyrun.py`, driver `.../sandbox/agents/tui2.sh`.)

**Baseline** (empty workspace, no overlay): `sources=6 loaded_sources=6 candidates=2 diagnostics=0`.
`candidates=2` are exactly the two built-in definitions.

---

## 3. The agent-definition system

### 3.1 Version strings and schema ids

Contiguous string run in the binary (offset `196361694`):

```
agent-definition-current-v1
agent-definition-with-execution-request-v1
agent-definition-with-skills-v1
agent-definition-with-skills-and-execution-request-v1
permission_mode default dontAsk acceptEdits bypassPermissions
max_turns
isolation shared remote
agent-definition-review-resource-v1
plugin-agent-inventory-v1
```

* `agent-definition-current-v1` — the base definition schema id. **PROVEN** (exact string).
* The `-with-skills-*` / `-with-execution-request-*` variants are alternate schema ids selected
  by which optional blocks the definition carries. **INFERRED** from naming + adjacency.
* `agent-definition-review-resource-v1` is the *review resource* schema used by the
  runtime-capability trust plane; review-resource ids are built as
  `agent-resource:` + `sha256:` (both literals present adjacent to
  `resolved Agent Definition id exceeds its byte bound`). **INFERRED**.
* `plugin-agent-inventory-v1` is the plugin-scoped inventory schema.
* Parser contract constant: **`agent_definition_core_v1`**.

### 3.2 The full author-facing field list

Contiguous serde field run (the FieldCode enum used in diagnostics; `none` is the enum's
zero variant):

```
none
name description prompt tools disallowed_tools model effort permission_mode
max_turns background isolation skills mcp_servers hooks memory color
initial_prompt main_session_selection raw_bytes
```

So an agent definition may declare:

| Field | Notes |
|---|---|
| `name` | **Required.** `[a-z]+(-[a-z]+)*`, ≤128 bytes. PROVEN (§3.4). |
| `description` | Free text; surfaces to the parent model as the "when to use" blurb. |
| `prompt` | The child's developer prompt. In a `.md` file the Markdown body serves as the prompt; `prompt:` in frontmatter also parses. PROVEN both accepted. |
| `tools` | Work-tool allowlist. Renders to `work_tool_selector` = `ToolSelectorWire::All{all}` or `::Named{names}`. |
| `disallowed_tools` | Denylist → `disallowed_work_tools`. |
| `model` | Model id. **Inert for workflow children** (see §5.4). |
| `effort` | `none\|minimal\|low\|medium\|high\|xhigh\|ultra`. Also inert for workflow children. |
| `permission_mode` | `default \| dontAsk \| acceptEdits \| bypassPermissions` (Claude-Code-compatible vocabulary). |
| `max_turns` | Integer turn cap for the child. |
| `background` | Boolean. |
| `isolation` | `shared \| remote` in the definition wire enum. |
| `skills` | Skill ids preloaded into the child (§3.7). |
| `mcp_servers` | MCP servers granted to the child. |
| `hooks` | Hook declarations. |
| `memory` | Memory scope; `AgentDefinitionMemoryScope` = `user \| local \| project`. |
| `color` | TUI colour. |
| `initial_prompt` | Seed prompt for the child's first turn. |
| `main_session_selection` | Whether the definition is selectable for the *main* session, not only children. |
| `raw_bytes` | Internal field-code for "the document as a whole" (used in size diagnostics). |

### 3.3 The compiled/registry wire form (`EntryWire`)

```
internally tagged enum EntryWire  { valid | named_invalid | invalid_id }

struct variant EntryWire::Valid with 13 elements:
  ordinal, review_resource_id, scoped_definition_id, description, prompt,
  work_tool_selector, disallowed_work_tools, source_digest, definition_hash,
  skills, permission_mode, max_turns, isolation

struct variant EntryWire::NamedInvalid with 9 elements:
  ordinal, ..., reason_code, field_code, offending_field_count, bound_code, limit
struct variant EntryWire::InvalidId with 8 elements

internally tagged enum ToolSelectorWire { All{all} | Named{names} }

struct InventoryWire { schema_version, parser_contract, plugin_id,
                       package_integrity_digest_v1, entries, counts }
struct CountsWire     { valid, named_invalid, invalid_id }
```

Note that `model`, `effort`, `hooks`, `mcp_servers`, `memory`, `color`, `background`,
`initial_prompt`, `main_session_selection` do **not** appear in `EntryWire::Valid`.
That matches the workflow tool text: *"Definition-carried model and effort remain inert."*
INFERRED: those fields parse but are not yet plumbed into the child-launch grant in 1.0.1.

Diagnostic/telemetry side of the definition (`DefinitionGrantAuditV1`):

```
DefinitionGrantAuditV1 {
  definition_name, selection_mode, launch_lane, source, effective_grant,
  resolution_summary, capability_context, configuration_epoch
}
AuditSource { scope, kind }
AuditEffectiveGrant { work_tools, session_control_tools }
GrantResolutionSummary {
  requested_work_tool_count, parent_omitted_work_tool_count,
  definition_denied_work_tool_count, caller_narrowed_work_tool_count,
  lane_denied_tool_count
}
definition_tool_mode ∈ { all, named, empty }
AgentSpawnDefinitionSelectionModeV1 ∈ { default, explicit }
AgentSpawnLaneV1 ∈ { native, workflow }
```

### 3.4 Name grammar — PROVEN by exhaustive probe

Method: `--agents '{"<name>":{"description":"d"}}'` and read the trace counters.
A rejected name fails the **whole session source** (`source_failed=1`, all its entries dropped).

| name | result |
|---|---|
| `a` | accepted (candidates 2→3) |
| `ab`, `abc`, `aa` | accepted |
| `a-b`, `with-dash`, `code-reviewer` | accepted |
| 128×`a` | accepted |
| `a1`, `z9`, `test2`, `a1b2`, `9start` | **rejected** (`source_failed=1 diagnostics=1`) |
| `with_underscore` | **rejected** |
| `with.dot`, `with/slash`, `with:colon` | **rejected** |
| `UPPER` | **rejected** |
| `é` | **rejected** |
| `-a`, `a-`, `a--b` | **rejected** |
| 129×`a` | **rejected** |
| `{"ok":…,"BAD":…}` | **whole source rejected** — one bad name kills the overlay |

⇒ grammar is `[a-z]+(-[a-z]+)*` with a 128-byte cap. This is the same
"lowercase capability identifier grammar" the binary names elsewhere
(`context feed 'name' must use the lowercase capability identifier grammar`).

`general-purpose` and `workflow-subagent` are **accepted at load** as user names
(candidates 2→3); the collision is resolved later by the precedence fold
(`duplicate` / `higher_precedence` reason codes).

### 3.5 On-disk locations — PROVEN by bisect

One `.md` file per candidate location, then read `candidates`:

| path tried | picked up? |
|---|---|
| `<ws>/.agents/agents/alpha.md` | **YES** (candidates 2→3) |
| `$CONFIG_DIR/agents/epsilon.md` (`~/.config/muse/agents`) | **YES** (candidates 2→3) |
| `<ws>/.claude/agents/beta.md` | no |
| `<ws>/.codex/agents/gamma.md` | no |
| `<ws>/.muse/agents/delta.md` | no |
| `<ws>/agents/theta.md` | no |
| `~/.claude/agents/zeta.md` | no |
| `~/.agents/agents/eta.md` | no |
| `~/.codex/agents/iota.md` | no |

So, unlike skills (which explicitly import from `.claude`/`.codex`), **agent definitions are
Muse-native only**: project `.agents/agents/` and user `$CONFIG_DIR/agents/`.

File rules — PROVEN:

| variation | result |
|---|---|
| `alpha.md` with `name: alpha` | accepted |
| `alpha.md` with `name: beta` | **accepted** — frontmatter `name` wins, filename ignored |
| no `name:` in frontmatter | **rejected**, 1 diagnostic |
| no `description:` | accepted |
| no frontmatter at all (plain body) | **rejected**, 1 diagnostic |
| `prompt:` in frontmatter, empty body | accepted |
| unknown frontmatter key `bogus_field` | accepted at load (deferred validation) |
| `alpha.txt`, `alpha.markdown` | not scanned |
| `ALPHA.md`, `some file.md`, `.alpha.md` | scanned (filename irrelevant) |
| `nested/alpha.md`, `nested/deep/alpha.md` | scanned — **discovery is recursive** |

Project discovery does **not** walk up the tree: with `.agents/agents/rootlevel.md` at the repo
root and cwd = `<repo>/sub`, nothing is found (`candidates=2`); adding
`<repo>/sub/.agents/agents/sublevel.md` makes it `candidates=3`. The project source is rooted at
the **session workspace root** (cwd by default, or `--workspace <PATH>`).

Minimal working example (PROVEN to become a candidate):

```markdown
---
name: code-reviewer
description: Reviews diffs for correctness bugs.
---

You review the supplied diff and report correctness bugs only.
```

### 3.6 Composition: sources, precedence and safe mode

`DefinitionSource` enum (exact string run, two independent copies in the binary):

```
DefinitionSource  managed session project user plugin built_in fixed
                  project_depth  plugin_id
```

`sources=6` is constant in every run I made, and never changed with overlays or files.
INFERRED mapping of the six slots: `managed` (enterprise), `session` (`--agents`),
`project`, `user`, `plugin`, `built_in`. `fixed` is the marker for the immovable built-ins and
`project_depth` / `plugin_id` are qualifiers carried on the project/plugin variants.

**Safe mode** — PROVEN. `settings.json`:

```json
{ "schema_version": 1, "agent_definitions": { "safe_mode": true } }
```

```
safe_mode=false  sources=6 loaded_sources=6 suppressed_sources=0 candidates=3   (user .md present)
safe_mode=true   sources=6 loaded_sources=2 suppressed_sources=4 candidates=2
```

Safe mode suppresses 4 of 6 sources, leaving 2 — and it suppresses the **`--agents` session
overlay too** (`safe_mode=true` + `--agents '{"alpha":{…}}'` ⇒ `candidates=2`).
`settings.agent_definitions` accepts **only** `safe_mode` (PROVEN:
``unknown field `enabled`, expected `safe_mode` ``), and it must be a boolean.

Enterprise policy can force it: `execution.force_agent_definition_safe_mode` is a real
policy-plane member but **not yet activated** in this build — PROVEN:

```
$ MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG=1 muse config validate --plane policy --file policy.json
enterprise_document_invalid: plane=policy reason=field_not_activated location=execution.force_agent_definition_safe_mode
```

(An unknown member instead yields `reason=unknown_member`, so the field name is exact.)

**Precedence / diagnostic reason codes** — the full 51-variant enum, verbatim and in order:

```
selected, nearest_trusted_project, builtin_fallback, higher_precedence, nearer_project,
duplicate, named_invalid, safe_mode, untrusted_project, disabled_plugin,
not_trusted_enabled, plugin_scope_quota, plugin_preflight_overflow, plugin_class_overflow,
inventory_refresh_required, invalid_utf8, bom_forbidden, frontmatter_invalid,
structured_shape, duplicate_key, invalid_name, missing_field, invalid_field, unknown_field,
field_too_large, candidate_too_large, missing_source, wrong_kind, symlink_or_reparse,
path_escape, non_utf8_path, cycle, inventory_changed, identity_changed, unstable_read,
io_error, source_bound, snapshot_bound, inventory_integrity, join_mismatch,
package_digest_mismatch, not_found, lookup_expectation_mismatch, unknown_tool,
tool_classification, epoch_mismatch, context_mismatch, grant_bound, forged_dispatch,
known_field_inactive, restricted_source_field
```

Reading the precedence semantics off those names (INFERRED but tightly constrained):

* `higher_precedence` — a same-named definition from a stronger source wins.
* `nearer_project` / `nearest_trusted_project` — among project sources the nearest (and
  trusted) project root wins.
* `builtin_fallback` — nothing matched, fall back to a built-in.
* `duplicate` — two entries with the same name inside one source.
* `untrusted_project`, `disabled_plugin`, `not_trusted_enabled`, `safe_mode` — suppression
  reasons per source.
* `known_field_inactive`, `restricted_source_field` — a field is understood but not honoured
  from that source class (e.g. a `hooks` block from a project definition).

`candidates` counts **raw entries before conflict resolution** — PROVEN by an incremental
build-up, where every added entry increments the counter even when its name already exists:

| state | candidates |
|---|---|
| clean (2 built-ins only) | 2 |
| + user `alpha.md` (`name: alpha`) | 3 |
| + project `.agents/agents/alpha.md` (`name: alpha`) | 4 |
| + `--agents '{"alpha":{"description":"session"}}'` | 5 |
| + a *second* user file `alpha2.md` also named `alpha` (no overlay) | 5 = 2 + 2 user + 1 project |
| + a user file named `general-purpose` (shadowing a built-in) | 5 |

Nothing is deduplicated at this stage, so name collisions across `session` / `project` / `user`
/ `built_in` — including shadowing a reserved built-in name — are resolved strictly downstream
of this log line, by the `higher_precedence` / `nearer_project` / `duplicate` fold.

Session-overlay failure text (PROVEN, printed by the TUI before it starts):

```
$ muse --provider echo --trust-workspace --agents 'notjson'
Session Agent Definition JSON is invalid

$ muse ... --agents '[]'      → Session Agent Definition JSON is invalid
$ muse ... --agents '"x"'     → Session Agent Definition JSON is invalid
$ muse ... --agents '{}'      → accepted (a no-op overlay, candidates unchanged)
```

Related error strings in the binary (INFERRED, not triggered live):

```
Session Agent Definition overlay failed:
project definition discovery failed:
Agent Definition source loading failed for
Agent Definition filesystem source failed:
Agent Definition product startup was already composed
agent definitions are not reviewable and stay inactive:
non-reviewable runtime capabilities:
```

and, for the legacy Claude-style two-file layout:

```
legacy overlay `…` is unreadable or unparseable beside the authoritative Agent root; the overlay contributes nothing
legacy overlay `…` fails the text-safety rules; the overlay contributes nothing
legacy overlay `…` does not carry the Agent root `name`; the overlay contributes nothing
legacy overlay `…` declaration `…` is inactive beside the authoritative Agent root
legacy overlay `…` is present and inactive beside the authoritative Agent root
```

### 3.7 `--agents` is a NAME→DEFINITION MAP (PROVEN)

This was the single most useful measurement. `candidates` rises by exactly the number of
top-level keys:

| `--agents` value | candidates |
|---|---|
| *(absent)* | 2 |
| `{}` | 2 |
| `{"a":1}` | 3 |
| `{"a":"just a prompt string"}` | 3 |
| `{"a":{}}` | 3 |
| `{"alpha":{...},"beta":{...}}` | 4 |
| `{"a":1,"b":2}` | 4 |

Two consequences:

1. The overlay is a **map of agent name → definition**, not a single definition object and
   not an array. `--agents '{"name":"foo"}'` (which "looks" like a definition) actually
   declares an agent literally called `name`.
2. Values may be a **string or a map** — the binary carries the serde expectation literal
   `string or map` right next to the `EntryWire` type names, and a bare string value is
   accepted. INFERRED: the string form is the prompt shorthand.

Per-entry field validation is deferred: `{"a":{"unknown_field_xyz":1}}`,
`{"a":{"model":"m"}}`, `{"a":{"tools":["read_file"]}}` all load cleanly with
`diagnostics=0`. Only the **name** is validated at composition time.

### 3.8 Skills preloaded by a definition

When a selected definition carries `skills`, the child gets a wrapped developer block
(verbatim):

```
<agent-definition-skills definition="These skill instructions were preloaded by the selected
Agent Definition. Apply them throughout this child run.
<skill id="…
</agent-definition-skills>
```

### 3.9 Built-in agent definitions (PROVEN — exact strings, contiguous)

```
general-purpose
General-purpose agent for complex, multi-step tasks that require exploration and action.
Complete the delegated task using only the effective capabilities supplied for this agent.
Return a concise result to the parent.

workflow-subagent
Workflow child agent for one delegated Workflow task.
Complete the delegated Workflow task using only the effective capabilities supplied for this
agent. Return a concise result to the Workflow owner.
```

followed immediately by the enum literals `default dontAsk acceptEdits bypassPermissions`
and `shared remote`, i.e. these two records are stored with `permission_mode` and `isolation`
defaults. Both names are also listed in the spawn-rejection `reserved_role` neighbourhood
(`…generated_agent_name_collision workflow-subagent general-purpose…`), i.e. they are
**reserved roles**.

Canonical id budget (from the `workflow` tool description, verbatim): *"a #7546 canonical
rendered Agent Definition id of at most 385 UTF-8 bytes (plugin-scoped ids included; its
unscoped or final definition name is at most 128 UTF-8 bytes)"*. Matches the measured
128-byte name cap.

### 3.10 Plugin-supplied agent definitions are DARK in 1.0.1 (PROVEN)

A plugin bundle declaring an `agents` capability validates as an `agent`-kind declaration but
is refused:

```
$ MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins validate ./plug --json
{
  "error": { "code": "manifest-family-mismatch", ...
    "plugin": { "compatibility": { "summary": "unsupported",
      "declarations": [ { "id": "agent:a5", "kind": "agent", "classification": "unsupported" } ] } },
    "capabilities": { "skills": [], "hooks": [], "mcp_servers": [], "commands": [], "reminders": [] },
    "diagnostics": [ …,
      { "code": "unsupported-capability", "severity": "error",
        "message": "plugin agents capabilities are not supported in this phase" } ] } }
```

So the plugin manifest key is `capabilities.agents`, the capability id namespace is
`agent:<id>`, the scoped id form is `plugin:<plugin-id>:agent:<id>`
(field `scoped_definition_id`), and **the whole lane is gated off in this build**.
The plugin manifest capability list in the binary is
`agents lspServers outputStyles skills userConfig` alongside
`displayName description hooks mcpServers author repository license interface dependencies requiredPlugins`.

Related diagnostic codes on the plugin side:
`unsupported-agent-schema`, `agent-component-invalid`, `agent-skill-skipped`,
`agent-mcp-server-skipped`, `agent-overlay-inactive`, `agent_definition_review_unavailable`.

---

## 4. The native subagent runtime

### 4.1 Enabling it

Default is **off**. Exact TUI string:

```
Agent delegation: off; native subagent tools are hidden.
Set run.subagent_delegation_mode = "auto" in settings to enable delegation.
```

PROVEN setting shape:

```
$ echo '{"schema_version":1,"run":{"subagent_delegation_mode":"bogus"}}' > $CONFIG_DIR/settings.json
malformed settings file …: unknown variant `bogus`, expected `off` or `auto`
```

Related env gate: `TBH_NATIVE_SUBAGENT_DOGFOOD` (no observable effect in `exec`).
Also `TBH_SUBAGENT_RUNTIME_CONCURRENCY_CAP` (host-scaled scheduler cap, separate from the
Agent-Tree root capacity).

### 4.2 The six (seven) tools — verbatim descriptions

Contiguous string block:

```
subagent_type subagent_spawn subagent_status
subagent_send_message subagent_wait subagent_read_result subagent_cancel
```

| tool | description (verbatim) |
|---|---|
| `subagent_spawn` | "Spawn a simple child agent. The root Agent Tree can execute up to 8 agents at once by default, including the root; the configured limit may vary from 1 to 64. A spawn attempted while the root pool is full is rejected with root_capacity_exhausted; wait for an Agent to finish before retrying. An accepted child may remain queued by the host-scaled runtime scheduler and starts automatically when a scheduler slot frees. Choose worktree_isolation (true or an empty object) when the user requests subagent isolation or when parallel children may write, because concurrent writers can corrupt a shared checkout even when their intended files differ. Keep read-only children in the shared checkout. Isolation may be unavailable for the current profile or workspace." |
| `subagent_status` | "Read subagent status from the replayable owner registry." |
| `subagent_send_message` | "Queue a message for a running child through subagent input delivery." |
| `subagent_wait` | "Wait for a child result for up to 30000 milliseconds by default. Set timeout_ms between 10000 and 300000 to choose the live-wait deadline. timeout or would_park means the child keeps running. Finished results are delivered to you automatically when your session is idle. Use {{tool:subagent_cancel}} to stop the child." |
| `subagent_read_result` | "Read the bounded result envelope and artifact refs for a child." |
| `subagent_cancel` | "Request child cancellation through the subagent owner surface." |
| `subagent_followup_task` | (present in the tool-name table; MSP method `subagent/followupTask`) |

Parameter names recovered adjacent to those descriptions:
`subagent_type`, `worktree_isolation`, `schema_ref`, `required_fields`, `status_filter`,
`interrupt` (boolean), `wait_for` (`result_ready | task_terminal`), `timeout_ms` (integer),
`cancellation_token_ref`, `path_prefix`, `result_cursor`, `artifact_ref`, `command_id`,
plus `maxLength` on the objective/prompt.

Per-parameter help text, verbatim:

* `subagent_type` — "Optional Agent Definition identifier. Omit or use null to resolve the
  unscoped general-purpose identity through current registry precedence."
* schema — "Optional bounded structured-result contract. Omit or pass null to keep the native
  final-text result channel."
* `wait_for` — "Use result_ready for the child result envelope. Use task_terminal only when a
  terminal task ref is enough."
* `timeout_ms` — "Live-wait deadline in milliseconds. Defaults to 30000 when omitted; valid
  range is 10000 through 300000. Expiry returns timeout and leaves the child running."
* `worktree_isolation` — "Choose worktree_isolation (true or an empty object) … false, null,
  or omission spawns without isolation." and the validation message
  "worktree_isolation accepts true, false, or an object such as {}. Send true (or {}) to
  request an isolated worktree checkout; send false or omit the field to spawn without
  isolation."

Wait-timeout message (verbatim, with runtime slots):

```
Wait timed out after {N} ms because the child did not reach {state} before the deadline.
The wait did not cancel the child; its result will be delivered automatically when your
session is idle. Do not poll; use subagent_cancel to stop it.
```

Duplicate-work advisory: *"Other subagents have the same task. Ignore this advisory if
intentional; otherwise reuse an existing result or cancel redundant children."*

### 4.3 Delegation posture (system reminders, verbatim)

`<system-reminder source="subagent-delegation">`:

> Delegation posture for the native subagent tools: spawning children is optional. By default,
> prefer working inline — a lookup that a single `search` or `read_file` call can answer stays
> inline, and work one normal turn can handle stays one turn. When you do delegate, size the
> fan-out to the number of genuinely independent subtasks: batch related small queries into one
> child rather than one child per query, keyword, or directory. Queued spawns start
> automatically as slots free; never re-issue or duplicate a queued spawn. After spawning, wait
> for or read the child result instead of repeating the same work yourself. A wait that returns
> timeout or would_park means the child keeps running; do not poll — finished results are
> delivered to you automatically when your session is idle, so keep working or end your turn.
> User opt-outs ("don't use subagents", "single agent") always win.

`<system-reminder source="subagent-delegation-proactive">`:

> Proactive subagent delegation is active (reasoning effort is ultra): this replaces the default
> inline-first posture. Use `subagent_spawn` when independent subtasks could run in parallel and
> parallel work would materially improve speed or quality, without waiting for the user to ask
> for subagents. The over-trigger guards still apply: work one normal turn can handle stays one
> turn, and user opt-outs ("don't use subagents", "single agent") always win.

`DelegationPostureWire` = `{ inactive{state} | proactive{profile, lanes, renderer_revision} }`.

### 4.4 Agent Tree, capacity and lineage

```
AgentTreeInitializedV1 { writer_protocol, root_agent_id, root_session_id,
                         execution_capacity, derived_root_admission }
AgentLineageAdmissionV1 { agent_instance_id, immediate_parent_agent_id }
CommandKindV1  = spawn | queue_message | subtree_close | retry_source_observation
CommandKeyV1   { root_agent_id, caller_agent_id, command_kind, command_id_digest }
CapacityScopeV1 = root
CapacitySnapshotV1 { occupied, limit, available }
SpawnDecisionV1 { command_key, request_fingerprint, durability_disposition,
                  admission_disposition, result_rendering_version, guidance_version, capacity }
SpawnRejectionReasonV1 = agent_name_invalid | agent_name_conflict | agent_path_too_long |
   caller_not_executing | parent_closed | policy_denied | reserved_role |
   lineage_integrity_failed | root_capacity_exhausted | generated_agent_name_collision
```

Capacity is `settings.agents.execution_capacity` — **PROVEN**:

```
$ echo '{"schema_version":1,"agents":{"execution_capacity":0}}'  → invalid value: integer `0`, expected an integer from 1 through 64
$ echo '{"schema_version":1,"agents":{"execution_capacity":65}}' → invalid value: integer `65`, expected an integer from 1 through 64
$ echo '{"schema_version":1,"agents":{"bogus":1}}'               → unknown field `bogus`, expected `execution_capacity`
```

Capacity-exhaustion messages (verbatim):

```
Root execution capacity is full: the root Agent occupies the only execution slot (1 occupied of 1 total).
No agent was created or queued. The root Agent occupies the only execution slot, so waiting
cannot help or create a child slot. Start a new root configured with agents.execution_capacity
of at least 2.
No agent was created or queued. This root has {occupied} of {limit} execution slots occupied.
Wait for an agent to stop executing, or message an existing agent. To try this spawn again
after capacity is available, use a new command_id. An exact retry with this command_id will
replay this rejection; changing the request while reusing …
```

Spawn request fingerprint (the identity a retry must match):

```
AgentSpawnFingerprintEvidenceV1 {
  immediate_parent_session_id, immediate_parent_run_id, tool_surface_version,
  name_input_evidence, role, objective, resolved_agent_definition_id,
  definition_selection_mode, definition_grant_digest, definition_context_digest,
  definition_toolset_digest, context_policy_ref_digest, worktree_isolation,
  lane, workflow_provenance, fork_seed_admission_digest, child_completion_contract }
RequestFingerprintDomainV1 = agent.spawn.v1 | agent.message.v1
```

### 4.5 Durable control records

```
SubagentControlRecord (internally tagged) variants:
  spawn_accepted { parent_run_id, agent_path, description, definition_grant_audit,
                   inherited_web_search_mode, worktree_isolation_request }
  spawn_rejected | wait_parked | status_updated { control_status }
  start_attested { subagent_session_id, dispatched_via_tool_call_id, run_stream,
                   parent_trace_id, parent_span_id }
  child_session_bound { child_session_id, storage_layout }
  attempt_admitted | resume_context_recorded | owner_state_changed |
  owner_command_recorded | result_ready | cancel_requested | cancel_outcome |
  close_outcome | closed | redirect_pending | redirect_reconciled |
  recovery_available { recovery_decision } | runtime_observed |
  lease_parked | lease_active | cleanup_outcome | cleanup_effect_started |
  cleanup_effect_settled | setup_failed | workspace_scope_activated |
  parent_dirty_excluded | operation_requested

SubagentResumeContextRecord { spawn_command_id, depth, role, objective,
  context_policy_ref, allowed_child_tool_names, worktree_isolation }
ChildSessionStorageLayout = nested_session_v1
SubagentControlStatus = starting | running | result_ready | closing | closed |
                        recovery_pending | manual_reconciliation
SubagentTaskTerminalRef { task_stream, task_terminal_ref, summary,
                          evidence_refs, artifact_refs, error_kind, structured_data }
```

MSP wire surface (from `muse schema generate-ts`, `msp.d.ts`, identical for the stable and
experimental exports):

```ts
export type MspMethod = "initialize" | "subagent/sendMessage" | "subagent/followupTask"
  | "subagent/interrupt" | "subagent/stop" | "subagent/resume" | "subagent/reopen"
  | "subagent/close" | "subagent/readResult" | "session/start" | ...

export type SubagentControlStatus = "accepted" | "starting" | "running" | "resultReady"
  | "closing" | "closed" | "recoveryPending" | "manualReconciliation" | (string & {});

export interface SubagentResult {
  artifactRefs: string[];
  errorKind?: string;          // verbatim durable vocabulary, when the child failed
  evidenceRefs: string[];
  structuredData?: Record<string, unknown>;
  summary: string;             // <=512 chars, runtime-enforced
  text?: string;               // <=32 KiB
}
```

The transcript `Item` kind `"subagent"` carries:
`agentPath` ("agent definition path"), `childSessionId`, `controlStatus`, `depth`,
`durationMs`, `failureReason`, `objective`, `result`, `role`, `subagentId`, `usage`
("**transitive** observed usage — the child and its own descendants … never folded into
`session/tokenUsage.cumulative`"), `workflowRunId`.

Hook events include `SubagentStart` and `SubagentStop` (in `HookEventKind` alongside
`SessionStart`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreLLMCall`, `PostLLMCall`,
`PreCompact`, `PostCompact`, `Stop`, `SessionEnd`, `Notification`, `PostToolUseFailure`,
`StopFailure`, `PostToolBatch`).

Child result protocol: a child that must return structured data is told

> Your last reply did not call `submit_result`. The child result is recorded only through that
> tool. Call `submit_result` now and do not call another tool in the same response. If your
> answer contains information the schema fields do not capture, preserve it losslessly in the
> optional `notes` field.

and a plain-text child is told

> Your last reply contained no final answer. Reply now with the complete result as normal
> assistant text. Do not call a tool unless more work is required.

Default child-result contract id: `muse:child-result/default-v1`;
`WorkflowChildCompletionContractKind = default_text | declared_schema`.

Approval plumbing distinguishes `ApprovalRequestSource = main_turn | delegated_subagent`, and
`DeclaredToolSecurity` has an `OpeningCommandAncestor`/`DelegatedSubagent` attribution — i.e.
approvals raised inside a child are labelled as coming from a delegated subagent.

---

## 5. Worktree isolation

### 5.1 `--subagent-worktree-isolation` is a no-op (PROVEN)

Its own help text says so: *"Compatibility flag; capability defaults on. Only an affirmative
per-child request asks for isolation; omission stays shared."* Passing it in a non-Git
directory does **not** error:

```
$ cd /…/nogit && muse exec --provider echo --trust-workspace --subagent-worktree-isolation "hi"
muse: workspace root: /…/nogit (cwd default)
echo: hi
```

whereas the *session* worktree flag does hard-fail there:

```
$ cd /…/nogit && muse exec --provider echo --trust-workspace -w create "hi"
session worktree requires a Git source repository: /…/nogit
```

### 5.2 Where worktrees go (PROVEN)

```
$ cd $BASE/ws && git commit -qm init && muse exec --provider echo --trust-workspace -w create "hi"
muse: workspace root: /…/ws/.muse/worktrees/20260901-3c75 (cwd default)
echo: hi
```

Layout created:

```
ws/.muse/worktrees/<YYYYMMDD>-<4hex>/                        # the checkout (removed when clean)
ws/.muse/worktrees/.session-worktree-reservations/capability-probe
ws/.muse/worktrees/.session-worktree-reservations/v1/plans/<session-uuid>.json
ws/.muse/worktrees/.session-worktree-reservations/v1/by-leaf/<leaf>.json
ws/.muse/worktrees/.session-worktree-reservations/v1/by-session/<session-uuid>.json
ws/.muse/worktrees/.session-worktree-reservations/v1/tmp/
```

Reservation record (verbatim file contents):

```json
{"schema_version":1,"leaf":"20260901-3c75","session_id":"01a05d73-b85b-79a0-a16c-b2e52cc5e983","backend":"git","source_binding":"git-storage-root"}
```

`.muse/worktrees/` is also on the search-exclude list (`exclude` … `/.muse/worktrees/`).

### 5.3 Git prerequisites and rejection reasons (verbatim strings)

```
native_child_execution_not_enabled
  This profile cannot run native children; this child will not start. Enable native child
  execution or continue in the parent.
isolation_not_enabled_in_profile
  Read-only children do not need isolation. Retry without worktree_isolation, or enable
  native_subagent_worktree_isolation.
no_session_event_sink
  Read-only children do not need isolation. Retry without worktree_isolation, or start the
  session with durable event logging enabled.
workspace_not_git
  This workspace is not a Git repository, so worktree isolation is unavailable. Read-only
  children do not need isolation. Retry without worktree_isolation, or run from a Git
  workspace.
workspace_git_probe_failed
  The runtime could not confirm that this workspace is a Git repository, so worktree isolation
  is unavailable. …
no_workspace_root
  Read-only children do not need isolation. Retry without worktree_isolation, or run from a
  workspace that supports isolation.
```

So the prerequisites for a child worktree are: **(1)** the agent profile permits native child
execution, **(2)** the profile enables `native_subagent_worktree_isolation`, **(3)** the session
has a **durable event sink** (i.e. **NOT** `--no-session-log`), **(4)** a workspace root exists,
and **(5)** it is a Git repository the runtime can probe. Additional refusals:
`worktree_isolation_unavailable`, `worktree_cleanup_pending`, `agent_definition_policy_denied`,
`task_identity_preparation_failed`, and the low-level
`NativeSubagentWorktreeSetup` error variant.

Session-worktree failure vocabulary (`WorktreeFailureReason`):

```
source_not_git_repo, repo_mismatch, worktree_path_outside_storage_root, worktree_collision,
worktree_setup_failed, dirty_worktree, head_changed, caller_owned_worktree,
worktree_base_missing, source_no_commits, worktree_base_invalid
```

Lease/lifecycle records:

```
SubagentWorktreeLeaseStateRecord { subagent_id, lease_generation, attempt_ref, admission,
  parent_session_id, source_repo_identity, source_repo_root, worktree_root, base_ref,
  base_commit, recorded_branch, head_commit, owner_lock, cleanup_policy, ownership,
  operation_timeout_ms }
SubagentWorktreeIsolationRequest { runtime_owned }
SessionWorktreeMode = off | create | existing
SessionWorktreeCleanupPolicy = none | remove_if_clean
SessionWorktreeOwnership = runtime_owned | caller_owned
WorktreeCleanupState = cleaned | retained | cleanup_failed
SessionWorkspaceScopeKind = git_worktree
```

Cleanup rule, stated verbatim in the workflow guidance:

> After an isolated child reaches its terminal and becomes quiescent, the runtime automatically
> removes a clean or ignored-only worktree and retains one with tracked changes, non-ignored
> untracked files, or a changed HEAD.

Observed live: a clean `-w create` worktree was removed at the end of the run
(`git worktree list` afterwards shows only the main checkout) while the reservation records
were retained.

### 5.4 Workflow-lane isolation (verbatim tool guidance)

The `workflow` tool's "PER-CHILD ISOLATION" block:

> The session/profile worktree-isolation capability defaults on, but capability availability
> alone never isolates a child. `isolation` is an additional optional field on each
> `host.agent`/`host.pipeline` request object, every `host.parallel([...])` request-array entry,
> and the positional `agent("prompt", { agentType, schema, isolation, label })` options object.
> Only `true`, a case-insensitive `"true"` string, or a non-array, non-function object
> (including an empty object) affirmatively requests an isolated worktree. `false`, a
> case-insensitive `"false"` string, `null`, `undefined`, or omission uses the shared parent
> workspace and emits no isolation request; every other shape rejects. An affirmative request
> may reject when capability, provider, or Git prerequisites are unavailable; it never silently
> falls back to shared placement. Choose isolation (`true` or an empty object) when the user
> requests subagent isolation or when parallel children may write, because concurrent writers
> can corrupt a shared checkout even when their intended files differ. Keep read-only children
> in the shared checkout.

and the definition-selection contract for workflow children:

> `agentType` is optional: omit it or pass null/undefined to use the built-in
> `workflow-subagent` identity and current default launch; if supplied, use a #7546 canonical
> rendered Agent Definition id of at most 385 UTF-8 bytes (plugin-scoped ids included; its
> unscoped or final definition name is at most 128 UTF-8 bytes). An explicit `agentType`
> selects that registered Agent Definition; **its prompt is appended as one developer context
> block, and its `tools`/`disallowedTools` may only narrow the inherited Work-tool grant.
> Definition-carried model and effort remain inert; per-call options or parent inheritance
> control execution. Every child inherits the parent session's current effective Work tools as
> its upper bound** (write tools included when the session has them). Per-call `tools` is
> unsupported and must be omitted.

Workflow host validation errors that name the same fields:

```
workflow script host.<call> agentType must be a string
workflow script host.<call> agentType must be a canonical Agent Definition id: …
workflow script host.<call> tools is unsupported; omit tools (agentType is optional)
workflow script host.<call> isolation must be true, false, null, a "true"/"false" string, or a non-array object
Workflow agentType selection failed: …
```

`WorkflowJournalStableOptions { agentType, effort, isolation, model, outputSchemaRef,
outputSchemaRequiredFields, inlineSchema, tools }`.

---

## 6. `--preset` and agent profiles

### 6.1 What a preset is

`RunPresetSettings { provider, model, agent_profile, run }` (also `PresetDefaultsV1` on the
enterprise-defaults plane). A preset is a **named bundle of startup choices**, resolved by
`fbcode/musecode/build/src/crates/config/src/settings/run_preset/diagnostic.rs` and traced as:

```
event="run_preset.resolve" outcome=<ready|failed> reason=<none|invalid_configuration|unknown_preset|resolution_failed>
  selected=<bool> provider_source=<runtime|preset|none> model_source=… agent_profile_source=…
```

PROVEN behaviour:

```
--preset native-basic → outcome="ready" selected=true agent_profile_source="preset"
--preset miniswe      → outcome="ready" selected=true agent_profile_source="preset"
--preset bogus        → outcome="failed" reason="unknown_preset"
                        CLI: "unknown preset zzz; expected native-basic|miniswe"
```

User-defined presets live under `settings.presets.<name>` and are selectable by
`--preset <name>` — PROVEN with
`{"schema_version":1,"presets":{"p1":{"agent_profile":"native-basic"}}}` +
`muse exec --preset p1` → `run_preset.resolve outcome="ready" selected=true
agent_profile_source="preset"`. Preset names must match
``preset name `…` must use lowercase ASCII letters, digits, '-' or '_' ``.

### 6.2 The built-in agent-profile registry (PROVEN)

```
$ settings.presets.p1.agent_profile = "zzz"; muse exec --preset p1 …
unsupported agent profile `zzz`; expected `native-basic`, `miniswe`,
`code-mode-v1-all-tools`, `code-mode-v2-all-tools`,
`code-mode-v2-prefer-generated-bindings`, `code-mode-v2-only`,
`code-mode-v2-native-libraries-disabled`, or `code-mode-exp-16461-declarations-r1`
```

The binary also carries `agent profile id comes from the built-in registry` and
`built-in agent profile preset is valid`.

### 6.3 What `native-basic` and `miniswe` contain

Contiguous string run immediately after `agent profile id comes from the built-in registry`:

```
native-basic
  read_file search write_file edit_file artifact read_memory add_memory edit_memory
  list_peer_sessions send_session_message shell bash_input monitor
  cron_create cron_delete cron_list get_goal create_goal update_goal report_progress
  subagent_spawn subagent_status subagent_send_message subagent_wait
  subagent_read_result subagent_cancel
miniswe
  You are a helpful assistant that can interact with a computer.
```

⇒ `native-basic` = a fixed 25-tool "native" toolset **that includes the subagent tools**;
`miniswe` = a mini-SWE-agent profile whose whole system prompt is that one sentence.
The `code-mode-*` profiles swap the toolset for the `muse.code_exec` V8 sandbox with generated
bindings.

---

## 7. Everything else worth writing down

### 7.1 Settings keys relevant to this dimension

```
SettingsFile { schema_version, agents, agent_definitions, provider, model, reasoning_effort,
  first_turn_minimal_effort_regex, context_compaction, run, provider_retry, tui, context,
  local_session_messaging, feature_config, tools, skills, model_catalog, mcpServers,
  mcp_servers, presets, hooks, runtime_capabilities, permissions, plugins,
  managed_hooks_path, managed_hooks_env_vars, max_consecutive_stop_hook_continuations,
  endpoint_transport, telemetry, notifications }

AgentsSettings           { execution_capacity }              # 1..64, default 8
AgentDefinitionsSettings { safe_mode }                       # bool
RunConfigurationSettings { system_prompt, developer_prompt, toolset, parallel_tool_calls,
  workflow_trigger_mode, workflow_api_version, subagent_delegation_mode, code_mode,
  search_literal_fallback, context_usage_message_enabled, reminder_observers,
  context_slimming, reminder_roster }
```

Documented setting paths (verbatim from the binary's settings-key table):
`settings.agents.execution_capacity`, `settings.run.subagent_delegation_mode`,
`settings.presets.<name>`, `settings.presets.<name>.run.reminder_rosterts`.

### 7.2 Enterprise policy hooks

```
PolicyExecutionV1 { forbid_approval_bypass, forbid_sandbox_bypass,
                    force_agent_definition_safe_mode, permission_profiles, approval_modes,
                    approval_reviewers, network_sandbox_modes, tool_rules,
                    allow_project_configuration, allow_foreign_configuration,
                    stop_hook_continuations }
PolicyExtensionsV1 { skills, hooks, runtime_capabilities }
RuntimeCapabilityPolicyV1 { allowed_kinds }
```

`force_agent_definition_safe_mode` validates as `field_not_activated` today (§3.6).

### 7.3 Environment variables in this area

```
TBH_NATIVE_SUBAGENT_DOGFOOD
TBH_SUBAGENT_RUNTIME_CONCURRENCY_CAP
TBH_DISABLE_CHILD_SESSION_LOG_ROUTING
TBH_TMUX_SUBAGENT_RUNNING_FEED_FIXTURE
MUSE_EXPERIMENTAL_EXTERNAL_AGENT_INGRESS   (external_agent_ingress gate)
MUSE_EXPERIMENTAL_PLUGINS                  (reveals `muse plugins`)
MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG
```

### 7.4 TUI surface

`/subagents` — "View running and past subagents". It appears in the built-in slash-command
vocabulary and in the tab strip `workflows | subagents | monitor | terminals`, with a status
line like `… 0 subagents, +1 terminal`. Subagent status labels rendered in the view:

```
Spawn accepted. / Child starting. / Child running. / Result envelope ready. /
Child closing. / Child closed. / Recovery pending after a restart. /
Parked for manual reconciliation.
```

### 7.5 Tracing / telemetry span

`invoke_agent {subagent_type}`; `gen_ai.operation.name = invoke_agent`;
`gen_ai.agent.name`; opens on `subagent.control.start_attested`, closes on
`subagent.control.result_ready`; `subagent_type` is derived from
`subagent.control.spawn_accepted.role` "(falling back to the `agent_path` file stem)".
`final_status ∈ clean | failed | timed_out | cancelled`; events
`subagent_dispatch` / `subagent_complete`.

---

## 8. Reproduction recipes

```bash
BASE=…/scratchpad/sandbox/agents
M=…/scratchpad/muse-aarch64-macos
export MUSE_NO_AUTO_UPDATE=1 HOME=$BASE/fakehome \
       XDG_CONFIG_HOME=$BASE/fakehome/.config XDG_DATA_HOME=$BASE/fakehome/.local/share

# 1. Baseline composition counters (TUI only; needs the PTY harness)
$BASE/tui2.sh $BASE/ws
#   → event="agent_definition.sources_load" … sources=6 loaded_sources=6 candidates=2 diagnostics=0

# 2. Session overlay is a name→definition map
$BASE/tui2.sh $BASE/ws --agents '{"code-reviewer":{"description":"reviews diffs"}}'
#   → candidates=3

# 3. Bad name kills the whole overlay
$BASE/tui2.sh $BASE/ws --agents '{"ok":{},"BAD":{}}'
#   → source_failed=1 candidates=2 diagnostics=1

# 4. Project-scope file
mkdir -p $BASE/ws/.agents/agents
printf -- '---\nname: code-reviewer\ndescription: d\n---\nbody\n' > $BASE/ws/.agents/agents/x.md
$BASE/tui2.sh $BASE/ws            # → candidates=3

# 5. User-scope file
mkdir -p $HOME/.config/muse/agents
printf -- '---\nname: planner\ndescription: d\n---\nbody\n' > $HOME/.config/muse/agents/p.md

# 6. Safe mode suppresses 4 of 6 sources
echo '{"schema_version":1,"agent_definitions":{"safe_mode":true}}' > $HOME/.config/muse/settings.json
$BASE/tui2.sh $BASE/ws            # → loaded_sources=2 suppressed_sources=4 candidates=2

# 7. Capacity bounds
echo '{"schema_version":1,"agents":{"execution_capacity":65}}' > $HOME/.config/muse/settings.json
$M exec --provider echo "x"       # → expected an integer from 1 through 64

# 8. Delegation mode enum
echo '{"schema_version":1,"run":{"subagent_delegation_mode":"bogus"}}' > $HOME/.config/muse/settings.json
$M exec --provider echo "x"       # → unknown variant `bogus`, expected `off` or `auto`

# 9. Agent-profile registry
echo '{"schema_version":1,"presets":{"p1":{"agent_profile":"zzz"}}}' > $HOME/.config/muse/settings.json
$M exec --preset p1 --provider echo "x"   # → the 8-profile list

# 10. Worktree layout
cd $BASE/ws && git init -q . && git commit -q --allow-empty -m init
$M exec --provider echo --trust-workspace -w create "hi"
#   → workspace root: …/ws/.muse/worktrees/20260901-3c75

# 11. Plugin agents lane is dark
MUSE_EXPERIMENTAL_PLUGINS=1 $M plugins validate $BASE/plug --json
#   → "plugin agents capabilities are not supported in this phase"
```

---

## 9. Open questions

1. What exactly are the 6 source slots? `sources=6` never moved. `DefinitionSource` has 7
   names (`managed session project user plugin built_in fixed`); which two collapse, and which
   2 survive safe mode, is unproven.
2. Where does a `managed` (enterprise) agent-definition directory live on disk? No path was
   found by probe or by string.
3. The exact JSON Schema behind `agent-definition-current-v1` and its three siblings — the ids
   are present but I found no embedded schema document, so the field list in §3.2 comes from
   the serde field table, not from a schema.
4. Whether a definition's `hooks` / `mcp_servers` / `memory` / `color` / `background` /
   `initial_prompt` / `main_session_selection` fields are honoured at all in 1.0.1 — they are
   absent from `EntryWire::Valid`, and `known_field_inactive` / `restricted_source_field`
   exist as reasons, but I could not force the diagnostic.
5. How `main_session_selection` interacts with the TUI model/agent picker.
6. What the second built-in "review resource" plane does: `agent-definition-review-resource-v1`
   plus `agent_definition_review_unavailable` plus
   `agent definitions are not reviewable and stay inactive:` suggests a per-definition trust
   review UI (`muse plugins approve`-style) that is not yet wired for non-plugin definitions.
7. Whether `--agents` values as bare strings really mean "prompt" (the `string or map` serde
   literal says a string is legal, but nothing observable distinguished it).
8. Live spawn behaviour (queueing, `would_park`, worktree lease lifecycle) was not exercised —
   it needs a real model, which the task forbids.
9. The `muse-code/models` catalog wire shape, which blocked the attempt to capture the real
   tool-schema JSON by pointing `--base-url` at a local sink.

---

# Verification

Adversarial re-verification pass, independent sandbox
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/verify-agents-subagents/`
(own PTY harness `ptyharness.py` / `ptyfeed.py`, own per-run isolated `$HOME` driver `tp.sh`,
so every run starts from a clean config root). Every runtime claim below was re-executed from
scratch; every string claim was re-extracted with `grep -abo` + `dd` on the raw binary rather
than from a pre-made `strings` dump.

**Verdict: MOSTLY_SOLID.** The runtime spine of the report reproduces exactly — the
`--agents` map semantics, the name grammar, the two on-disk locations, the file-format rules,
no walk-up, the composition diagnostic, TUI-only composition, `safe_mode`, `candidates`
counting, `execution_capacity` 1..64, `subagent_delegation_mode` off|auto, the 8 agent
profiles, the preset machinery, the worktree layout, the plugin refusal and the enterprise
`field_not_activated`. Three claims are wrong (one enumeration off by one, one count wrong,
one INFERRED claim built on a misattributed string), several PROVEN labels are over-claimed
on adjacency evidence, and a substantial amount of ground is missing.

## Refuted

**R1. "MSP exposes *nine* subagent methods" — there are eight.**
`muse schema generate-ts --out ts-s` then
`grep -o '"subagent/[A-Za-z]*"' ts-s/msp.d.ts | sort -u | wc -l` → **8**:
`subagent/close`, `subagent/followupTask`, `subagent/interrupt`, `subagent/readResult`,
`subagent/reopen`, `subagent/resume`, `subagent/sendMessage`, `subagent/stop`.
The report's own evidence line lists exactly those eight; the number "nine" is unsupported.
(The rest of that finding is confirmed: `diff -r ts-stable ts-exp` is empty, `ItemKind`
contains `"subagent"`, and `SubagentResult` has the six documented fields.)

**R2. "`native-basic` is a fixed *25*-tool set" — the string run contains 26 names.**
`read_file, search, write_file, edit_file, artifact, read_memory, add_memory, edit_memory,
list_peer_sessions, send_session_message, shell, bash_input, monitor, cron_create,
cron_delete, cron_list, get_goal, create_goal, update_goal, report_progress, subagent_spawn,
subagent_status, subagent_send_message, subagent_wait, subagent_read_result, subagent_cancel`
= 26. (Re-extracted at offset 193047900.) The membership claim itself is also weaker than
"PROVEN": a contiguous literal run cannot prove *absence* — any tool name that the linker
pooled with an earlier copy elsewhere in the binary would simply not appear in this run.

**R3. INFERRED "In `--agents`, a string value is the prompt shorthand (the serde expectation
is `string or map`)" — the cited literal belongs to a different type, and the behaviour
contradicts the reading.**
The `string or map` literal sits inside the *inventory codec* serde block, immediately after
`struct variant EntryWire::Valid with 13 elements` / `struct variant ToolSelectorWire::Named
with 1 element` (offset 199140237: `...EntryWire::InvalidId with 8 elementsmap with a single
keystring or mapMapAccess::next_value called before next_key`). That is serde's stock
expectation string for the internally-tagged `EntryWire`/`ToolSelectorWire` enums, not the
`--agents` parser. Live, the overlay accepts **every** JSON value type for an entry with
`diagnostics=0`:

```
--agents '{"a":1}'            -> candidates=3 diag=0
--agents '{"a":"p"}'          -> candidates=3 diag=0
--agents '{"a":null}'         -> candidates=3 diag=0
--agents '{"a":true}'         -> candidates=3 diag=0
--agents '{"a":[1,2]}'        -> candidates=3 diag=0
--agents '{"a":{}}'           -> candidates=3 diag=0
--agents '{"a":{"bogus_field_xyz":1}}' -> candidates=3 diag=0
```

At composition time only the **key** is validated; the value shape is not inspected at all,
so nothing observable distinguishes a string from a number, let alone establishes "prompt
shorthand". Treat this as unknown.

## Corrections

**C1. An invalid agent name in `--agents` does not just drop the overlay — the TUI refuses to
start, with a *different* error string than the one the report documents.**
The report has two distinct failure messages collapsed into one. Reproduced:

```
$ muse --provider echo --trust-workspace --agents 'notjson'       -> Session Agent Definition JSON is invalid
$ ... --agents '[]' / '"x"' / '123' / 'null' / 'true' / '[{"a":1}]' -> Session Agent Definition JSON is invalid
$ ... --agents '{"a1":{}}'                                        -> Session Agent Definition input is invalid
$ ... --agents '{"ok":{},"BAD":{}}'                               -> Session Agent Definition input is invalid
```

`…input is invalid` is the *valid JSON object, bad content* message. The process emits the
trace line (`source_failed=1 candidates=2 diagnostics=1`) and then **never renders the TUI**
(191 bytes of PTY output and no frame, versus ~2.9 KB and a rendered frame for a valid
overlay). So a single malformed name is a hard startup failure, not a silent drop.

**C2. That whole-source-failure rule is specific to the *session overlay*. Filesystem sources
behave the opposite way.** A `.md` file whose frontmatter `name:` is invalid produces
`source_failed=0 candidates=2 diagnostics=1` — only that one entry is dropped, the source and
the session survive. 60 bad-named files in one project source → `diagnostics=60`,
`source_failed=0`.

**C3. `DefinitionSource` is not proven to be a 7-variant enum.** There are **three** distinct
contiguous runs prefixed by that type name, with different variant tails:

```
@191605987  DefinitionSourcemanagedsessionprojectuserpluginbuilt_in            (6, no `fixed`)
@193193471  DefinitionSourcemanagedsessionprojectuserpluginbuilt_infixedproject_depthplugin_id  (7)
@194535203  DefinitionSourcemanagedsessionprojectuserpluginbuilt_infixedproject_depthplugin_id  (7)
@197675636  DefinitionSourcesessionprojectpluginbuilt_infixedproject_depthplugin_id            (5)
```

The symbol table shows the real type is `tbh_agent_definition::source::DefinitionSource`
(paired with a `DefinitionScope`). Because the linker pools identical short literals, a name
missing from one run is not absent from that enum — and equally, `fixed` sitting next to
`built_in` in one run is not proof it is a variant of *that* array. `AgentDefinitionMemoryScope`
demonstrates the same hazard: it appears as `userlocalproject`, `localproject` and bare
`local` at four different offsets. Downgrade this finding from PROVEN to INFERRED.

**C4. The subagent tool *parameter list* is partly adjacency-guessing.** `subagent_type`,
`worktree_isolation`, `schema_ref`, `required_fields`, `status_filter`, `interrupt`,
`wait_for`, `timeout_ms`, `cancellation_token_ref` all sit inside the genuine tool-spec block
at offset 197017198 with their own help text, so those are solid. But `path_prefix`,
`result_cursor`, `artifact_ref` and `command_id` come from a *different* pool at offset
197299809 (`...missing_parent..#1338cancellation_token_refpath_prefixresult_cursorartifact_ref
#1199spawnworkerrejected legacy spawn cannot allocate a path...`) that also contains
`unsupported_real_execution`, `invalid_input` and unrelated legacy-spawn error text. Their
attachment to the subagent tool schema is INFERRED, not PROVEN. No live tool schema was ever
captured (`--provider echo` never sends one), so the schema remains unrecovered.

**C5. Claim 35 ("a per-definition trust-review plane … which is why filesystem definitions can
be discovered yet stay inactive") mixes two things.** The quoted strings are real, but they
live in the **plugin** runtime-capability trust plane: `agent definitions are not reviewable
and stay inactive: ` sits amid `bundled skill …` / `foreign hook …` plugin diagnostics
(offset 213610873), and `agent_definition_review_unavailable` sits in the `plugins
inspect`/`approve`/`reject` vocabulary next to `plugin_command_template_missing`,
`bundled_plugin_invalid`, `plugin-capability-snapshot` (offset 191637909). There is no
evidence tying that plane to project/user filesystem definitions. Whether a filesystem
definition is actually usable at spawn time remains unknown in both directions.

**C6. `--subagent-worktree-isolation` is "a compatibility flag", not demonstrated to be a
no-op.** The help text ("Compatibility flag; capability defaults on") is the only evidence;
"does not error in a non-Git dir" does not establish that it changes nothing. Its clap ident
`SUBAGENT_WORKTREE_ISOLATION` / `subagent_worktree_isolation` is in the root-arg table, and
no trace line differs with or without it — so: *no observable effect in `exec` mode*, which
is weaker than "no-op".

**C7. `general-purpose` / `workflow-subagent` as `reserved_role` is adjacency evidence.** The
run `generated_agent_name_collisionworkflow-subagentgeneral-purpose` exists (offsets 192514360,
194412392, 197077208) but is immediately followed by unrelated authority-fold error text; the
`SpawnRejectionReasonV1` array itself (offset 191675367, with `variant index 0 <= i < 10`
nearby) ends at `generated_agent_name_collision`. Plausible, not proven.

## Confirmed exactly (re-run independently)

* `--agents` is a name→definition **map**; `candidates` rises by the number of top-level keys
  (`{}`→2, `{"a":1}`→3, `{"a":1,"b":2}`→4).
* Name grammar `[a-z]+(-[a-z]+)*`, ≤128 bytes. Boundary re-bisected: **127 ok, 128 ok, 129
  fails**. `a-b-c`, `ab-cd-ef-gh`, `x-y-z-w-v-u` all accepted; `a1 z9 test2 9start
  with_underscore with.dot with/slash with:colon UPPER é -a a- a--b`, empty string and `a b`
  all rejected.
* On-disk locations: only `<ws>/.agents/agents/**/*.md` and `$CONFIG_DIR/agents/**/*.md`.
  Re-bisected and **extended** — also NOT scanned: `<ws>/.agents/*.md` (needs the second
  `agents/`), `<ws>/.config/muse/agents`, `<ws>/.muse/agents/agents`, `<ws>/.agent/agents`,
  `<ws>/.agents/subagents`, `$HOME/.config/agents`, `$HOME/.muse/agents`.
* File rules: frontmatter `name:` required and authoritative, filename irrelevant, recursion
  works, `description` optional, unknown keys pass at load, `.txt`/`.markdown` not scanned.
* No walk-up; `--workspace <PATH>` moves the project root.
* `event="agent_definition.sources_load"` at
  `config/src/agent_definitions/composition/diagnostic.rs:53`, `sources=6` invariant.
  `outcome` vocabulary `ready|not_implemented|session_overlay|project_discovery|source_load|failed`
  confirmed as a contiguous run at offset 196726880.
* `muse exec` emits no `agent_definition.*` event at all (only `startup, process.identity,
  path.resolved, feature_config.cache, settings.load, run_preset.resolve,
  permission_profile.catalog, permission_profile.resolve, credential.status`), and has no
  `--agents`.
* `agent_definitions` accepts only `safe_mode`, boolean (`invalid type: string "yes", expected
  a boolean`); `safe_mode=true` → `loaded_sources=2 suppressed_sources=4`, overlay suppressed.
* `candidates` counts pre-dedup: 2 → 3 → 4 → 5 exactly as tabulated.
* `execution_capacity` 1..64 (0 and 65 rejected, 1 and 64 accepted); `agents` accepts no other key.
* `subagent_delegation_mode` `off|auto`.
* All 8 agent profiles; `--preset zzz` → `unknown preset zzz; expected native-basic|miniswe`;
  user preset resolves with `agent_profile_source="preset"`; preset name grammar allows digits
  and `_` (`p_1`, `p-1`, `1p` ok; `P1`, `p.1` rejected) — i.e. **different** from the agent-name
  grammar.
* Worktree: `-w create` → `<repo>/.muse/worktrees/20260901-8cd5`, reservation JSON identical in
  shape to the report's (`{"schema_version":1,"leaf":…,"session_id":…,"backend":"git",
  "source_binding":"git-storage-root"}`) in all three of `plans/`, `by-leaf/`, `by-session/`,
  plus `capability-probe` and `tmp/`; clean worktree removed, reservations retained.
* Plugin `agents` capability: with a well-formed manifest (`compat.manifestDir` present) the
  *only* error left is `{"code":"unsupported-capability","message":"plugin agents capabilities
  are not supported in this phase"}` with `{"id":"agent:a5","kind":"agent",
  "classification":"unsupported"}` — stronger than the report's evidence, which still carried a
  manifest-family error.
* `MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG=1 muse config validate --plane policy` →
  `field_not_activated location=execution.force_agent_definition_safe_mode`, unknown key →
  `unknown_member`.
* 51-variant reason enum (`variant index 0 <= i < 51` directly follows it), 20-variant field-code
  enum (`0 <= i < 20`, i.e. `none` + 19 fields), `EntryWire::Valid with 13 elements`, the four
  `agent-definition-*-v1` ids, `agent_definition_core_v1`, both built-in definitions verbatim,
  the `<agent-definition-skills>` wrapper, both delegation system-reminders, the five worktree
  refusal reasons verbatim, `SubagentControlRecord` variants, `SpawnRejectionReasonV1`,
  the workflow "prompt is appended as one developer context block / model and effort remain
  inert" paragraph, and `TBH_NATIVE_SUBAGENT_DOGFOOD`,
  `TBH_SUBAGENT_RUNTIME_CONCURRENCY_CAP`, `TBH_DISABLE_CHILD_SESSION_LOG_ROUTING`,
  `MUSE_EXPERIMENTAL_EXTERNAL_AGENT_INGRESS`.
* `AgentDefinitionMemoryScope = user | local | project` confirmed (offsets 191661086, 197729583).

## Missed ground

**M1. The project source requires workspace trust — and this IS testable.** The report listed
it as an untestable open question. Pre-seed `$CONFIG_DIR/trust.json`:

```json
{"schema_version":1,"projects":{"<abs workspace path>":{"decision":"untrusted"}}}
```

then run the TUI with no `--trust-workspace` (no prompt appears):

| trust decision | user `.md` | project `.md` | candidates |
|---|---|---|---|
| untrusted | – | yes | **2** |
| untrusted | yes | yes | **3** |
| trusted | – | yes | 3 |
| trusted | yes | yes | 4 |

The project source is silently dropped when the workspace is untrusted (`suppressed_sources=0`,
`diagnostics=0` — the drop happens below this counter, presumably via the `untrusted_project`
reason). The user source is unaffected. Answering the interactive prompt with "1 Trust and
continue" writes `{"decision":"trusted"}` and composition then runs with the project source.

**M2. Hard, exact bounds on composition — none of them in the report.** All re-measured live
against `<ws>/.agents/agents/`:

| bound | value | evidence |
|---|---|---|
| candidate byte cap | **262144 (256 KiB) exactly** | 262144 → `candidates=3`; 262145 → `candidates=2 diagnostics=1` (`candidate_too_large`) |
| entries per source | **1024** | 1024 files → `candidates=1026`; 1025 → `source_failed=1 candidates=2` |
| directory nesting depth | **16** | depth 16 → ok; depth 17 → `source_failed=1` |
| single field cap | **≈16 KiB** | `description` of 16000 bytes ok; 16383 → `diagnostics=1` (`field_too_large`) |

The matching `BoundCode` enum is a contiguous run at offset 195715861 with `variant index
0 <= i < 15`: `entries, depth, candidate_files, candidate_bytes, fields, scopes,
serialized_bytes, registry_entries, registry_bytes, diagnostics, ids, audit_bytes`.

**M3. Symlinks kill the *entire* project source.** Any symlink on the path — the `.md` file
itself, the `agents/` directory, or the `.agents/` directory — gives
`source_failed=1 candidates=2 diagnostics=1`. This is the `symlink_or_reparse` reason and it is
a whole-source failure, not a per-entry drop. A framework that installs an agent pack by
symlinking a shared directory into `.agents/agents` will silently lose every project agent.

**M4. Two more file-level rules.** The `.md` extension is **case-sensitive**: `alpha.MD` is not
scanned (`candidates=2`) while `ALPHA.md` is. A **UTF-8 BOM** before the frontmatter is
rejected (`diagnostics=1`), matching the `bom_forbidden` reason.

**M5. Duplicate keys inside one `--agents` object fail the whole overlay.**
`--agents '{"a":{"description":"1"},"a":{"description":"2"}}'` → `source_failed=1 candidates=2
diagnostics=1` (`duplicate_key`). JSON duplicate keys are not last-wins here.

**M6. There is no `/agents` TUI surface, and `muse init` scaffolds nothing for agents.** The
complete built-in slash-command vocabulary (offset 195238973, described in the binary as
`tbh_agent::command_invoked::BUILTIN_SLASH_COMMAND_NAMES`) is:
`/login /logout /clear /new /resume /fork /side /init /deep-research /subagents /model
/settings /keymap /help /theme /rules /compact /export /copy /recap /skills /plugins /skill
/effort /goal /feedback /voice /exit /quit /stop /status /usage /upgrade /permissions /tasks
/workflows` + `custom`. No agent-definition browser, picker or manager exists — which also
bears on the unanswered `main_session_selection` question. `muse init` in an empty workspace
writes **only `AGENTS.md`**; it creates no `.agents/agents/` tree.

**M7. Sapling is a first-class worktree backend; the report treats Git as the only one.**
`ReservationWire { schema_version, leaf, session_id, backend, source_binding }` with
`backend ∈ {git, sapling}` and `source_binding ∈ {git-storage-root, sapling-shared-root}`
(offset near `git-storage-root`), plus a full Sapling lane: `create Sapling worktree`,
`restore Sapling worktree base`, `verify TBH session worktree node - sl help whereami`,
`Sapling worktree registry exceeds 1 MiB`, `Sapling storage root must be external to source and
shared roots`, `.hg`/`.sl`/`.eden` probes, `SourceProofSaplingUnsupported`, and
`Sapling session worktree setup is unsupported in this environment`. MSP exports
`export type Vcs = "git" | "sapling" | (string & {});` (msp.d.ts:1795). Note the asymmetry the
report should have drawn: **session** worktrees have a Sapling backend, but the **per-child**
isolation refusals are Git-only (`workspace_not_git`, `workspace_git_probe_failed`). Other
reservation-store rules recovered: `reservation record exceeds 16384 bytes`,
`non-canonical reservation JSON`, `reservation backend and source binding disagree`,
`reservation key and body disagree`, `unsupported reservation schema version`.

**M8. There is a dedicated crate, `tbh_agent_definition`, absent from the report's crate list.**
Recovered from v0-mangled symbols: `tbh_agent_definition::{source::{DefinitionSource,
DefinitionScope, DefinitionSourceInput, DefinitionCandidateEnvelope, ClosedFailureFacts},
parser::{SessionOverlayFailure, session::JsonNode, yaml_core}, registry::{SnapshotFailure,
LookupFailure}, diagnostic::{DiagnosticFailure, vocabulary::DiagnosticReasonV1},
grant::{GrantFailure, audit::codec::{DefinitionGrantAuditV1, AuditSourceWire},
epoch::tool::ToolClass}, canonical::{DefinitionValidationError, AgentDefinitionMemoryScope,
scaffold::{ScaffoldValue, ScaffoldFailure, declaration::{HookMatcherDeclaration,
DeclarationFields}}}, inventory::{PluginAgentDefinitionInventoryV1, codec::EntryWire},
built_in_definition_source}`, wired up by
`tbh_config::agent_definitions::composition::{AgentDefinitionSourceLoader::load_source,
ProductAgentDefinitionSourceLoader, AgentDefinitionSourceLoad::into_inputs,
DefinitionSourceInput::with_unusable_config_root}`,
`tbh_agent::agent_definitions::catalog::build_logical_parent_capability_surface`, and
`tbh_plugins::agent_definition_inventory::preflight::preflight_plugin_agent_definition_scopes`.
Two consequences worth noting: the YAML frontmatter is parsed by a vendored `saphyr_parser`
inside this crate, and `canonical::scaffold::declaration::HookMatcherDeclaration` +
`validate_hooks` show that a definition's `hooks:` block **is** parsed and validated, which
sharpens the report's "hooks parse but are not plumbed" guess.

**M9. Child sessions have a concrete on-disk layout.** The binary embeds a Python trace
projector (offset ~191879468) that reads them:
`root = session_log.parent / "subagent"`, then any `<dir>/session.jsonl` under it, keyed as
`subagent/<relative path>`. That is the concrete form of `ChildSessionStorageLayout =
nested_session_v1`. `muse trace inspect` (`--session-log/--run-log/--task-log/--run-id/
--all-runs/--render-mode/--format`) is the shipped reader for it.

**M10. Two more concurrency numbers the report missed**, both verbatim in the workflow tool
guidance: *"The runtime local concurrency cap is CPU-derived (maximum 16); the runtime queues
wider batches and re-runs the module as child results arrive"* and *"the workflow lifetime cap
is 1000 total host.agent/host.pipeline/host.parallel item calls. Exceeding 1000 lifetime calls
is a hard run failure, not an error_kind."* These sit alongside `settings.agents.execution_capacity`
(1..64) and `TBH_SUBAGENT_RUNTIME_CONCURRENCY_CAP` as a **third** distinct limit.

**M11. A tool-name alias table exists**: `subagent_send_message`→`send_message`,
`subagent_followup_task`→`followup_task` (offset 193078040, adjacent to `native-basic`).
`subagent_followup_task` never appears in the six-name tool run; it appears only here and as
the MSP method `subagent/followupTask`.

**M12. Minor extras.** `agent_definition.prepare_child` is a further event name in the durable
vocabulary. Enterprise config has exactly two planes (`defaults`, `policy`) and three source
classes (`SystemFile`, `MacosManagedPreferences`, `WindowsMachinePolicy`) — `muse config status`
prints all four rows as `state=absent`, and none of them is an agent-definition directory, so
the `managed` slot's on-disk home is still unlocated. `--agents` with `123`, `null`, `true` or
`[{"a":1}]` all give `Session Agent Definition JSON is invalid`, extending the report's three
cases.
