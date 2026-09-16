# Meta Muse Code ("TBH") 1.0.1-R2006.1 — Configuration System

Reverse-engineering report for the `config` crate (`paths.rs`, `settings.rs`, `enterprise/`,
`trust/`, `skills/`, `gate_registry/`, `feature_provider/`).

Binary: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
All commands below were run with `MUSE_NO_AUTO_UPDATE=1` and `HOME` redirected into a sandbox
(`.../scratchpad/sandbox/config/home`). Nothing outside the scratchpad was read or written.

Confidence markers: **PROVEN** = reproduced at runtime or exact literal in the binary.
**INFERRED** = deduced from adjacent literals / struct arity, not executed.

---

## 1. Executive summary

Muse Code has a **two-root, three-tier** configuration model:

* **Two roots on disk** — a *config root* (`$XDG_CONFIG_HOME/muse`, else `$HOME/.config/muse`)
  holding `settings.json`, `auth.json`, `trust.json` and the managed personal `skills/` store;
  and a *data root* (`$XDG_DATA_HOME/muse`, else `$HOME/.local/share/muse`) holding `sessions/`,
  `plugins/`, `skills/bundled/`, `local-tracing/`, `memory/`, `model-catalog/`, `crashes/`.
* **Three tiers** — enterprise (system file + OS managed preferences), user (`settings.json`),
  and workspace. Critically, **there is no workspace-level settings file at all** (§6.1); the
  workspace tier consists only of rules markdown, skill directories, plugin bundles and
  worktree scratch space.
* The enterprise tier is split into two mutually exclusive **planes**: `defaults` (seeds values
  the user may still override) and `policy` (a runtime floor the user cannot override).

The single most surprising finding is that `.muse/` is **two unrelated things**: inside the
config root's skill store it is the install-lock directory (`lock.json`, `audit.log`,
`.skills.lock`, `quarantine/`); inside a workspace it is *only* `worktrees/`. There is no
workspace `.muse/settings.json`, and `.muse/hooks.json` is not discovered by convention.

---

## 2. Path resolution (`config/src/paths.rs`)

### 2.1 The resolver and its telemetry

`paths.rs:453` emits one structured event per resolved path. Recovered verbatim from a
bootstrap trace log written by the binary:

```
2026-09-01T14:25:38.144523Z INFO tbh.local.bootstrap fbcode/musecode/build/src/crates/config/src/paths.rs:453 event="path.resolved" kind="config_root" source="home" state="present"
2026-09-01T14:25:38.144528Z INFO tbh.local.bootstrap fbcode/musecode/build/src/crates/config/src/paths.rs:453 event="path.resolved" kind="data_root"   source="home" state="present"
2026-09-01T14:25:38.144532Z INFO tbh.local.bootstrap fbcode/musecode/build/src/crates/config/src/paths.rs:453 event="path.resolved" kind="model_catalog_cache"  source="data_root" state="absent"
2026-09-01T14:25:38.144537Z INFO tbh.local.bootstrap fbcode/musecode/build/src/crates/config/src/paths.rs:453 event="path.resolved" kind="feature_config_cache" source="data_root" state="absent"
```

The enums, recovered as one adjacent literal run in `strings.txt`:

```
absent present unreadable   path.resolved   xdg   config_root data_root model_catalog_cache feature_config_cache
```

* `PathKind` = `config_root` | `data_root` | `model_catalog_cache` | `feature_config_cache`
* `PathSource` = `xdg` | `home` | `data_root`
* `PathState` = `absent` | `present` | `unreadable`

**Reproduce:**
```bash
export MUSE_NO_AUTO_UPDATE=1 HOME=<sandbox>
muse init --dry-run >/dev/null
cat "$HOME/.local/share/muse/local-tracing/bootstrap/"*.log
```

### 2.2 XDG handling — PROVEN

Setting `XDG_CONFIG_HOME` / `XDG_DATA_HOME` flips `source` from `home` to `xdg` and relocates
both roots:

```
2026-09-01T14:25:59.350931Z ... kind="config_root" source="xdg" state="present"
2026-09-01T14:25:59.350937Z ... kind="data_root"   source="xdg" state="present"
```

with the tree actually created at `$XDG_CONFIG_HOME/muse/` and `$XDG_DATA_HOME/muse/`.

Failure literals in the binary:

```
no usable config directory from XDG_CONFIG_HOME or HOME
no usable data directory from XDG_DATA_HOME or HOME
```

This proves the resolution order is **exactly two candidates, XDG first, HOME second** — there
is no third fallback and no `MUSE_HOME`/`MUSE_CONFIG_DIR` override (neither string exists in the
binary; the full `MUSE_*` env inventory is in §8).

The bundled `manage-settings` skill makes the "exactly one root" rule explicit:

> Resolve exactly one root. When `XDG_CONFIG_HOME` is set, `$HOME/.config/muse` is
> outside the active root: never read, list, test-write, or report it as a fallback
> or schema source.

The launcher shell script agrees:

```bash
# muse-launcher.sh:20-23
if [[ -n "${XDG_CONFIG_HOME:-}" ]]; then
  credential_default="$XDG_CONFIG_HOME/muse/auth.json"
else
  credential_default="$HOME/.config/muse/auth.json"
```

`XDG_CACHE_HOME` is **not** referenced anywhere in the binary — caches live under the data root.

---

## 3. The config root — `$XDG_CONFIG_HOME/muse` | `$HOME/.config/muse`

Observed tree after `muse init` + `muse skills install`:

```
~/.config/muse/
├── settings.json              # user settings (§5)
├── auth.json                  # provider credentials (struct AuthFile, 2 fields)
├── .auth.json.lock            # advisory lock, created on every startup
├── trust.json                 # workspace trust store (struct ProjectTrustStore, 2 fields)
└── skills/                    # managed *personal* skill store
    ├── <skill-id>/SKILL.md
    └── .muse/                 # ← store metadata, NOT a workspace dir
        ├── lock.json          # install lockfile
        ├── audit.log          # JSONL install/uninstall audit trail
        ├── .skills.lock       # 0-byte advisory lock
        └── quarantine/        # rejected packages
```

`muse skills install` reports these paths using a literal `$CONFIG_DIR` placeholder:

```json
{ "installed": { "id": "myskill", "path": "$CONFIG_DIR/skills/myskill", ... },
  "lockfile_path": "$CONFIG_DIR/skills/.muse/lock.json" }
```

### 3.1 `skills/.muse/lock.json` — verbatim

```json
{
  "version": 1,
  "skills": {
    "myskill": {
      "id": "myskill",
      "source": { "type": "local", "path": "/.../pkg/myskill" },
      "version": null,
      "revision": null,
      "installed_at": "2026-09-01T14:31:33.635797Z",
      "updated_at": null,
      "content_sha256": "sha256:a4aeab94d6bc66c938b671e459815c6168b9401eb70333619feb1fce02ba0243",
      "trust": "local",
      "scan": { "status": "passed", "warnings": [] },
      "files": [
        { "relative_path": "SKILL.md",
          "sha256": "sha256:99467dba62ae3a1690fdba4d47384aae7d97ab2351c781a3eef5d8f5180fda7e",
          "bytes": 117 }
      ]
    }
  }
}
```

Additional lock-record fields present in the binary but not exercised by a local install:
`provenance uninstalled removed_files kept_files compatibility known_fields unknown_fields
unsupported_fields allowed_tools`, and source kinds `marketplace-static marketplace-git
repository requested_ref resolved_revision manifest_path manifest_hash previous_content_sha256`.

### 3.2 `skills/.muse/audit.log` — verbatim (JSONL)

```json
{"time":"2026-09-01T14:31:33.643375Z","action":"install","skill":"myskill","source":"local","result":"ok"}
```

### 3.3 `trust.json`

`struct ProjectTrustStore with 2 elements` / `struct ProjectTrustEntry with 1 element`.
Trust is keyed to a canonical workspace root; failure literals:

```
workspace is not trusted
workspace trust has no canonical root binding
workspace root is unavailable
workspace root does not match its trusted binding
```

`--trust-workspace` grants trust **for one run only** and does not write `trust.json`
(top-level `--help`: "Trust this workspace for this run (load its skills and rules); does not
save trust"). PROVEN effect: with the flag, project skills load; without it:

```json
{"skills": [], "diagnostics": [
  {"code":"project-skills-untrusted",
   "message":"project skills skipped because workspace is untrusted",
   "scope":"project","path":".agents/skills"}]}
```

---

## 4. The data root — `$XDG_DATA_HOME/muse` | `$HOME/.local/share/muse`

Observed top level: `local-tracing/  plugins/  sessions/  skills/`.
The bundled `doctor` skill names the full set:

> 4. Data dirs: `sessions`, `memory`, `model-catalog`, and `crashes` under the data dir.

```
~/.local/share/muse/
├── sessions/
│   ├── YYYY/MM/DD/<session-id>/session.jsonl     # durable event log
│   │                          .session.lock      # inode-backed kernel lease (flock)
│   └── .msp-view-v1/<session-id>/                # materialized projection cache
│       ├── snapshot-<uuid>.json
│       └── index-00000000.bin
├── skills/bundled/muse-core/                     # bundled skills unpacked at runtime
│   ├── LICENSE
│   └── skills/{browser-app-delivery,create-plugin,create-skill,doctor,git,
│               greenfield-project-scaffolding,grill,grill-and-record,import,
│               manage-settings,plan,python-env,read-session,table-fit,taste}/SKILL.md
├── plugins/cache/<class>/<plugin>/<skill>/<sha256>/…   # content-addressed plugin cache
│                                        .<sha256>.lock
├── local-tracing/bootstrap/cli-<uuid>.log        # startup path-resolution traces
├── memory/                                       # agent memory (.md files)
├── model-catalog/                                # provider catalog cache
└── crashes/                                      # crash reports
```

The session path shape is confirmed by a literal in the binary:

```
${XDG_DATA_HOME:-$HOME/.local/share}/muse/sessions/YYYY/MM/DD/SESSION_ID/session.jsonl
```

and `MUSE_SESSIONS="${XDG_DATA_HOME:-$HOME/.local/share}/muse/sessions"`.

Bundled skills are **extracted from the binary into the data root on first run** — this is why
`muse-core/skills/manage-settings/SKILL.md` and `doctor/SKILL.md` are readable on disk and are
the authoritative source for §5's key contracts.

A second macOS-only data path exists as a literal: `Library/Application Support/Muse/session-name-authority`
(cross-process session-name lock). INFERRED to be joined onto `$HOME`.

---

## 5. `settings.json` — the user settings schema

### 5.1 File identity and error behaviour — PROVEN

Path: `$XDG_CONFIG_HOME/muse/settings.json`, else `$HOME/.config/muse/settings.json`.

| Input | Runtime result |
|---|---|
| `{"schema_version":1}` | accepted |
| `{"schema_version":2}` | `unsupported settings schema version 2 at <path>` |
| unknown top-level key | **silently ignored** (forward-compatible) |
| bad enum value | `malformed settings file at <path>: unknown variant \`nope\`, expected one of \`less\`, \`edits\`, \`more\`` |

`schema_version` is mandatory-by-convention and must be `1`. The `manage-settings` skill
instructs the agent to create `{"schema_version": 1, …}` when the file is missing.

### 5.2 Discovery method

Because a *parsed* key with a wrong-typed value produces a serde error naming the expected
Rust type, while an *unknown* key is ignored, feeding sentinel values is a complete schema
oracle. Every row below was produced by:

```bash
printf '{"schema_version":1,"<key>":12345}' > ~/.config/muse/settings.json
muse skills list --source user --json 2>&1 | head -c 400
```

### 5.3 Top-level keys (`struct SettingsFileDocument with 29 elements` — PROVEN count)

| Key | Type (from serde error) |
|---|---|
| `schema_version` | integer, must be `1` |
| `agents` | `an agents settings object` |
| `agent_definitions` | `an Agent Definition settings object` |
| `provider` | string — `echo` \| `meta` (`unsupported provider \`x\`; expected \`echo\` or \`meta\``) |
| `model` | string |
| `reasoning_effort` | string — `none\|minimal\|low\|medium\|high\|xhigh\|ultra` |
| `first_turn_minimal_effort_regex` | string |
| `context_compaction` | `struct ContextCompactionSettings` |
| `run` | `struct RunConfigurationSettings` |
| `provider_retry` | `struct ProviderRetrySettings` |
| `tui` | `struct TuiSettings` |
| `context` | `struct ContextSettings` |
| `local_session_messaging` | `struct LocalSessionMessagingSettings` |
| `feature_config` | `struct FeatureConfigSettings` |
| `tools` | `struct ToolSettings` |
| `skills` | `struct SkillsSettings` (lazily parsed) |
| `model_catalog` | lazily parsed |
| `mcpServers` | lazily parsed (`mcp_servers` also accepted — alias) |
| `presets` | `a map` of name → `struct RunPresetSettings` |
| `hooks` | lazily parsed |
| `runtime_capabilities` | lazily parsed — `runtime_capabilities must be an object` |
| `permissions` | lazily parsed |
| `plugins` | lazily parsed |
| `managed_hooks_path` | string |
| `managed_hooks_env_vars` | `a sequence` |
| `max_consecutive_stop_hook_continuations` | `usize` |
| `endpoint_transport` | `struct EndpointTransportConfig` |
| `telemetry` | `struct TelemetrySettings` |
| `notifications` | `struct NotificationSettings` |

"Lazily parsed" keys hold `RawValue` and are validated by a later subsystem, so a wrong type
there does not fail the settings load.

### 5.4 `tui` — `struct TuiSettings` (15 fields, all PROVEN)

| Key | Type / variants | Default |
|---|---|---|
| `show_reasoning` | bool | — |
| `reasoning_summaries` | bool | `true` |
| `away_recap_enabled` | bool | — |
| `prompt_hint_enabled` | bool | — |
| `voice_enabled` | bool | — |
| `voice_tap_enabled` | bool-ish (untagged) | — |
| `voice_shortcut_mode` | `toggle` \| `hold_to_talk` | — |
| `keymap` | map → map (sections `app`, `files`) | — |
| `resize_reflow_cap` | `"auto"` \| `"disabled"` \| positive integer row count | — |
| `color_depth` | `"auto"` \| `"truecolor"` \| `"256"` \| `"16"` \| `"none"` | `auto` |
| `terminal_background` | `auto` \| `light` \| `dark` | `auto` |
| `theme` | string | — |
| `rules_import_offer_dismissed` | bool | — |
| `foreign_context_notice_shown` | bool | — |
| `verbose_output` | `less` \| `edits` \| `more` | `edits` (shown "edits & writes") |

Exact error text for the two most useful:

```
invalid color_depth 12345: expected "auto", "truecolor", "256", "16", or "none"
invalid resize_reflow_cap "z": expected "auto", "disabled", or a positive integer row count
```

Defaults for `reasoning_summaries`, `verbose_output`, `terminal_background` are quoted verbatim
from the bundled `manage-settings/SKILL.md`.

### 5.5 `run` — `struct RunConfigurationSettings` (13 fields, PROVEN)

| Key | Type / variants |
|---|---|
| `system_prompt` | string (lazily parsed) |
| `developer_prompt` | string (lazily parsed) |
| `toolset` | `a sequence` |
| `parallel_tool_calls` | bool |
| `workflow_trigger_mode` | `auto` \| `explicit` \| `off` |
| `workflow_api_version` | `v1` \| `v2` |
| `subagent_delegation_mode` | `off` \| `auto` |
| `code_mode` | `enabled` \| `off` |
| `search_literal_fallback` | bool |
| `context_usage_message_enabled` | bool |
| `reminder_observers` | `a map` (→ `struct ReminderObserverSettings { every_n_steps }`) |
| `context_slimming` | `struct ContextSlimmingSettings` |
| `reminder_roster` | `struct ReminderRosterSettings { skip_if_running, max_in_flight_per_agent, agents }` |

`ReminderAgentSettings` (14 fields, from the binary's field list):
`id duty model preset permission_boundary default_priority max_priority tools max_child_steps
max_installs_per_run blocking reasoning_effort interval_steps decision`.
Validation literals: `reminder agent max_child_steps must be greater than zero`,
`reminder agent interval_steps must be greater than zero`.

### 5.6 `context_compaction` — `struct ContextCompactionSettings` (5 fields, PROVEN)

| Key | Type |
|---|---|
| `strategy` | `summary-preserved-suffix/v1` \| `prefix-extension-summary/v1` \| `prefix-extension-inventory-summary/v1` |
| `soft_threshold` | `f64` |
| `hard_threshold` | `f64` |
| `provider_context_limit_tokens` | `u64` |
| `tool_result_clearing_enabled` | bool |

### 5.7 `provider_retry` — `struct ProviderRetrySettings` (PROVEN)

`max_retries` `u32` · `base_delay_ms` `u64` · `retry_after_cap_ms` `u64` · `max_attempts` `u32`.
Cross-field rules (literals): `provider_retry must not set both max_retries and max_attempts`,
`max_attempts must be greater than 0`.

### 5.8 `tools` — `struct ToolSettings` (PROVEN)

```
tools.artifact.enabled                 bool
tools.web_search.mode                  client | hosted | off
tools.web_fetch.enabled                bool
tools.web_fetch.timeout_seconds        u64
tools.web_fetch.redirect_limit         u8
tools.web_fetch.max_fetch_bytes        usize
tools.web_fetch.max_processed_chars    usize
tools.web_fetch.max_output_chars       usize
tools.web_fetch.cache_ttl_seconds      u64
tools.web_fetch.cache_max_bytes        usize
```

### 5.9 `telemetry`, `notifications`, `endpoint_transport` (PROVEN)

```
telemetry.enabled        bool
telemetry.endpoint       string
telemetry.proxy          string
telemetry.client_cert    path string
telemetry.client_key     path string
telemetry.ca_bundle      path string
telemetry.destination    legacy | consolidated | edge | external
telemetry.auth           bearer | none
telemetry.artillery      bool

notifications.events     sequence of turn-complete | approval-waiting | turn-failed
notifications.method     osc9 | bell
notifications.condition  unfocused | always | off

endpoint_transport.base_url      string        (default https://api.meta.ai/v1)
endpoint_transport.proxy         string
endpoint_transport.auth          bearer | none
endpoint_transport.client_cert   path string
endpoint_transport.client_key    path string
endpoint_transport.ca_bundle     path string
endpoint_transport.http_headers  map
endpoint_transport.allow_openai  bool
```

Cross-field rules for `telemetry.destination=external` (verbatim):

> destination=external is direct-only: proxy/mTLS fields do not apply (the api.meta.ai front
> door authenticates with the Model API key; spec 9697 FR-E03)
> destination=external carries an implied credential (the Model API key); remove the auth field

Transport resolution modes: `direct`, `proxy`, `proxy_mtls`; failure kinds
`missing_api_key`, `proxy_mtls_conflict`, `invalid_mtls_path`, `incomplete_mtls`.

### 5.10 Scalars and small structs (PROVEN)

```
context.foreign_personal_rules        bool
context.foreign_personal_skills       bool
agents.execution_capacity             integer from 1 through 64
local_session_messaging.enabled       bool
feature_config.enabled                bool
```

`agents.execution_capacity` error text: `expected an integer from 1 through 64`. The
`manage-settings` skill notes the 64-slot unconfigured-root default is a *target*, not current.

### 5.11 `skills.activation` (key paths from the enterprise registry blob)

```
settings.skills.activation.user.<id>
settings.skills.activation.projects.<project>.<id>
settings.skills.activation.bundled.<id>
settings.skills.activation.plugin.<id>
```

Activation values seen in `muse skills list --json`: `"activation": "on"`.
CLI writers: `muse skills enable|disable|user-only <id> --scope user|project|built-in|plugin`.

### 5.12 `mcpServers.<id>` — `struct McpServerSettings`

`enabled mode transport command args env framing cwd url headers startup_timeout_sec
enabled_tools disabled_tools`

* `mode` — `required` \| `optional`
* `transport` — `stdio` \| `streamable_http`
* `framing` — `auto` \| `content_length` \| `line_delimited_json`

Validation literals:
```
transport is ambiguous with both command and url
transport requires command or url
stdio transport requires command
streamable_http transport requires url
stdio transport must not set url
streamable_http transport must not set command
streamable_http transport must not set args or env
streamable_http transport must not set stdio framing
server name must not be blank
server name must not contain surrounding whitespace
```

### 5.13 `runtime_capabilities`

`runtime_capabilities must be an object`; value is `RuntimeCapabilityState`
= `{ enabled, trusted_definition_hash, trusted_blocking_definition_hash }`.
Key shape quoted by the bundled skill:
`runtime_capabilities["plugin:tbh-reminders:reminder:skill-reminder"].enabled`.

### 5.14 `hooks`

Hook event kinds (`HookEventKind`, verbatim enum run):
`session_start user_prompt_submit pre_tool_use permission_request post_tool_use pre_llm_call
post_llm_call pre_compact post_compact subagent_start subagent_stop stop session_end
notification post_tool_use_failure stop_failure post_tool_batch`

Hook entry fields: `command commandWindows command_windows timeout statusMessage async
asyncRewake rewake rewakeMessage rewakeSummary shell condition if silent outputCapabilities`.
Companion keys: `managed_hooks_path` (string) / `managed_hooks_env_vars` (array), overridable
by env `TBH_MANAGED_HOOKS_PATH`, and `max_consecutive_stop_hook_continuations` (usize).

---

## 6. Workspace-level configuration

### 6.1 There is NO workspace settings file — PROVEN (negative result)

Nine candidate paths were planted with a **poison** document
(`{"schema_version":1,"tui":{"verbose_output":"POISON"}}`) that would produce a loud
`malformed settings file` error if parsed:

```
.muse/settings.json   .agents/settings.json   settings.json   .muse/config.json
muse.json   .musecode.json   .muse/muse.json   .agents/config.json   .muse.json
```

`muse skills list --source all --json --trust-workspace` produced **no error and no mention of
POISON**. None of these files is read. The bundled `manage-settings` skill states the rule
directly:

> That config root is the whole answer surface for this skill. A configuration file outside it
> is not Muse Code configuration — do not hunt for substitute configuration files.

Consequence: the classic "workspace overrides user" tier **does not exist for settings**.
Workspace influence is limited to rules, skills, plugins, and worktrees.

### 6.2 Rules files — `AGENTS.md` and `CLAUDE.md`

`muse init` scaffolds `AGENTS.md` (never `.muse/`). Verbatim output for a workspace named
`ws2` containing `src/`, `tests/`, `docs/`:

```markdown
# AGENTS.md

Muse Code reads this file as project rules when it runs in this directory.

## Project

- Name: ws2
- Generated by `muse init`.

## Common Commands

- No standard build or test commands detected yet.

## Project Layout

- `src/`: Source code.
- `tests/`: Tests.
- `docs/`: Project docs.
```

`muse init --help` → `usage: muse init [--dry-run] [--force]`. `--dry-run` prints the document
to stdout without writing.

**Precedence — PROVEN.** Running `muse init` in a workspace that already has `CLAUDE.md`:

```
Wrote AGENTS.md
muse: warning: wrote AGENTS.md, which takes precedence over existing project rules at CLAUDE.md;
merge still-applicable guidance into AGENTS.md or remove one of the two files
```

So **`AGENTS.md` > `CLAUDE.md`** in the same directory.

Nested-directory precedence, verbatim from the session preamble literal:

> Muse Code loaded standing rules at session open. Follow higher-priority instructions first.
> If user and project rules conflict, **project rules win**. If project rules files conflict,
> the **deeper file wins over the shallower one**.

Rules-file guards (literals): `rules file target must be a regular file`,
`rules file is not valid UTF-8`, plus size guards
`warning: rules file at … bytes, over the … byte load limit; it is skipped for this session`
and a truncation variant.

Personal (user-level) rules are **imported**, not read in place:

```
No personal rules found to import (looked for ~/.claude/CLAUDE.md and $CODEX_HOME/AGENTS.md,
default ~/.codex/AGENTS.md).
Rules loaded this session (in precedence order): … Codex … Muse Code
```

Gated by `settings.context.foreign_personal_rules` (bool) and the `foreign_personal_context_kill`
feature gate.

### 6.3 Workspace skill roots — PROVEN

Probing eight candidate directories with valid `SKILL.md` files, only three were discovered:

```
project | probe--agents-skills | .agents/skills/probe--agents-skills/SKILL.md
project | probe--claude-skills | .claude/skills/probe--claude-skills/SKILL.md
project | probe--codex-skills  | .codex/skills/probe--codex-skills/SKILL.md
```

`.muse/skills`, `skills/`, `agents/skills`, `.muse-plugin/skills` were **not** discovered.
The canonical root is `.agents/skills/`; `.claude/skills/` and `.codex/skills/` are compat roots.
All three require workspace trust (§3.3). The diagnostic names `.agents/skills` as *the*
project path.

Personal skill import roots (PROVEN via `muse skills import --from … --dry-run --json`):
`$HOME/.claude/skills` and `$CODEX_HOME/skills` (default `$HOME/.codex/skills`).

### 6.4 `.muse/` in a workspace = worktrees only — PROVEN

`muse exec -w create` produced:

```
muse: workspace root: …/ws8/.muse/worktrees/20260901-dbeb (cwd default)
```

```
ws8/.muse/worktrees/
└── .session-worktree-reservations/
    ├── capability-probe
    └── v1/
        ├── plans/<session-uuid>.json
        ├── by-leaf/<leaf>.json
        ├── by-session/<session-uuid>.json
        └── tmp/
```

Leaf naming: `<YYYYMMDD>-<4 hex>`. Reservation record verbatim:

```json
{"schema_version":1,"leaf":"20260901-dbeb","session_id":"01a05d66-9911-7c42-923e-fde143a98d0c","backend":"git","source_binding":"git-storage-root"}
```

Muse **appends `/.muse/worktrees/` to `.git/info/exclude`** (verified: the line is present at the
end of the file after the run). Backends: `git`, `sapling` (`sapling-shared-root`,
`git-storage-root`). Guards: `reservation record exceeds 16384 bytes`,
`non-canonical reservation JSON`, `unsupported reservation schema version`.

CLI: `-w/--worktree off|create|existing`, `--worktree-base <REF>` (default HEAD),
`--worktree-existing <PATH>`. Constraint literals: `--worktree requires session logging;
remove --no-session-log`, `--worktree-existing must differ from --workspace`.

### 6.5 `.muse/hooks.json` — not discovered by convention

`.muse` and `hooks.json` appear as adjacent literals, and `hooks/hooks.json` exists as a plugin
path. But planting a poison `.muse/hooks.json` in a workspace, and separately pointing both
`TBH_MANAGED_HOOKS_PATH` and `settings.managed_hooks_path` at it, produced **no hook error** in
an echo session. Workspace hook auto-discovery at `.muse/hooks.json` is **unconfirmed**; hooks
reach the runtime via `settings.hooks`, plugin capability manifests, and `managed_hooks_path`.

### 6.6 `.muse-plugin/` — plugin bundle manifest dir

`muse plugins` is hidden until `MUSE_EXPERIMENTAL_PLUGINS=1` is set (PROVEN: the subcommand
appears in `muse --help` only with the gate). From the bundled
`create-plugin/references/native-plugin-contract.md`:

> A native plugin is a new directory in the current workspace. Its manifest is:
> ```
> .muse-plugin/plugin.json
> ```

Minimal manifest (verbatim from the same file):

```json
{
  "schemaVersion": 1,
  "name": "example-plugin",
  "displayName": "Example Plugin",
  "version": "0.1.0",
  "description": "One plain-language sentence.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": { "skills": [], "commands": [], "hooks": [], "mcpServers": [], "reminders": [] }
}
```

Validator errors (PROVEN by installing a manifest lacking them):
`plugin manifest must declare schemaVersion 1`, `plugin manifest must declare compat.manifestDir`.

Compat manifest dirs: `.muse-plugin` / `.codex-plugin` / `.claude-plugin`.
Plugin ID grammar `^[a-z0-9][a-z0-9._-]{0,79}$`; `loop` and `muse-core` are reserved.
Install scopes: `--scope user|project`; source classes
`user-local project-trusted curated marketplace-user-added foreign-import native-local`.

### 6.7 `.muse-claude-sources` / `.muse-codex-sources`

Marketplace source-list files for the foreign plugin ecosystems. Literals:

```
Claude marketplace remote sources require a Git marketplace source   .muse-claude-sources
Claude marketplace source must be a path or object
Codex marketplace url sources require a Git marketplace source       .muse-codex-sources
```

Managed by `muse plugins marketplace add|list|update`; snapshots stored as `marketplace.json`.

### 6.8 `.muse-memory*`

Agent memory is Markdown (`memory path must be a Markdown .md file`, `memory path must include a
file name`, canonical `memory.md`), stored under the data root's `memory/` dir. The dotfiles are
concurrency/atomicity artifacts: `.muse-memory.lock` (advisory lock) and `.muse-memory-` (temp
prefix for atomic replace). Memory scopes: `personal`, `personal_project`, `project`; section
heading literal `## Memory scope: `.

---

## 7. Enterprise configuration

### 7.1 Gate and CLI

Gated by `MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG`. CLI:

```
Usage: muse config validate --plane <defaults|policy> --file <path>
       muse config status
```

`muse config status` on a clean machine (PROVEN):

```
Enterprise configuration status
Generation: sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4
Sources:
  plane=defaults source_class=system_file state=absent
  plane=policy   source_class=system_file state=absent
  plane=defaults source_class=macos_managed_preferences state=absent
  plane=policy   source_class=macos_managed_preferences state=absent
```

There is no `--json` flag on either subcommand.

### 7.2 Source classes, states, planes

```
EnterprisePlane        = defaults | policy
EnterpriseSourceClass  = system_file | macos_managed_preferences | windows_machine_policy
EnterpriseSourceState  = absent | valid | unreadable | untrusted | malformed | unsupported | invalid | stale
EnterpriseOpenKind     = new | resume | replacement
```

File names: `enterprise-defaults.json`, `enterprise-policy.json`.
macOS managed-preferences domain: **`com.tbh.tbh`** (the codesign identifier is `com.tbh.tbh.cli`,
Team OU `4W5TH4RKQ2`).

**System file location — INFERRED.** `Library` and `Application Support` exist as two separate
literals immediately adjacent to the `paths.rs` symbols and the "no usable config directory"
errors, and a full literal `Library/Application Support/Muse/session-name-authority` exists, so
the macOS system file is almost certainly
`/Library/Application Support/Muse/enterprise-{defaults,policy}.json`. This could not be proven
because writing under `/Library` is outside the permitted sandbox. Planting the files in
`~/.config/muse/`, `~/.config/muse/enterprise/`, and `<workspace>/.muse/` left `state=absent`,
so **the enterprise system file is NOT read from the user config root or the workspace** (PROVEN
negative). No env var overrides the enterprise path.

### 7.3 Envelope schemas — PROVEN by validator probing

The two planes are **strictly disjoint documents**:

| Plane | Accepted members | Rejected |
|---|---|---|
| `defaults` (`DefaultsEnvelopeV1`, 2 elements) | `schema_version`, `settings` | `execution` → `unknown_member` |
| `policy` (`PolicyEnvelopeV1`, 6 elements) | `schema_version`, `execution`, `model_egress`, `privacy`, `extensions`, `local_session_messaging` | `settings` → `unknown_member` |

`schema_version` must be `1`:
```
{"schema_version":2} → enterprise_schema_unsupported: reason=unsupported_schema_version location=schema_version
{}                   → enterprise_document_invalid:   reason=missing_schema_version  location=schema_version
```

Rejection reasons observed: `missing_schema_version`, `unsupported_schema_version`,
`unknown_member`, `wrong_type`, `semantic_invalid`, `field_not_activated`, `invalid_json`.
`field_not_activated` means the field exists in the schema but its feature gate is off in this
build — a distinct and very useful signal.

### 7.4 `defaults` plane — `EnterpriseDefaultsSettingsV1` (exactly 20 members, PROVEN)

Probing each candidate as `{"settings":{"<k>":{}}}` gave `valid` / `wrong_type` for real members
and `unknown_member` otherwise:

```
valid/wrong_type (real):  provider  model  reasoning_effort  first_turn_minimal_effort_regex
                          max_consecutive_stop_hook_continuations  run  provider_retry  tui
                          context  endpoint_transport  telemetry  notifications  mcp_servers
                          feature_config  local_session_messaging  agents  context_compaction
                          tools  skills  presets                                    ← 20 ✓
unknown_member:           mcpServers  hooks  permissions  plugins  runtime_capabilities
                          agent_definitions  model_catalog
```

Note the **naming divergence**: enterprise uses `mcp_servers` (snake_case); the user
`settings.json` uses `mcpServers` (camelCase). Enterprise cannot set hooks, permissions,
plugins, runtime capabilities, agent definitions, or the model catalog through this plane.

Sub-struct arities recovered from the binary, all matching the key registry:
`RunDefaultsV1` 11 · `TuiDefaultsV1` 11 · `TelemetryDefaultsV1` 6 · `ProviderRetryDefaultsV1` 3 ·
`ContextDefaultsV1` 2 · `EndpointTransportDefaultsV1` 3 · `NotificationsDefaultsV1` 3 ·
`ContextCompactionDefaultsV1` 5 · `AgentsDefaultsV1` 1 · `ToolsDefaultsV1` 3 ·
`WebFetchDefaultsV1` 8 · `SkillsDefaultsV1` 1 · `SkillActivationDefaultsV1` 4 ·
`McpServerDefaultsV1` 6 · `PresetDefaultsV1` 4 · `ReminderRosterDefaultsV1` 5 ·
`ReminderAgentDefaultsV1` 13 · `ContextSlimmingDefaultsV1` 5.

Activation status of `settings.tui.*` in this build (PROVEN):

```
show_reasoning  reasoning_summaries  away_recap_enabled  voice_tap_enabled
resize_reflow_cap  verbose_output                                  → activated (valid)
voice_enabled  voice_shortcut_mode  color_depth  theme             → field_not_activated
terminal_background  prompt_hint_enabled                           → unknown_member (user-only)
```

`TuiDefaultsV1` therefore has 11 members — `terminal_background` and `prompt_hint_enabled` are
deliberately **not** enterprise-settable.

### 7.5 `policy` plane

`PolicyExecutionV1` (11 elements — all 11 confirmed present):

```
execution.forbid_approval_bypass              field_not_activated (bool)
execution.forbid_sandbox_bypass               field_not_activated (bool)
execution.force_agent_definition_safe_mode    field_not_activated (bool)
execution.permission_profiles                 field_not_activated
execution.approval_modes                      ChoiceV1 (2 elements)
execution.approval_reviewers                  field_not_activated
execution.network_sandbox_modes               ChoiceV1
execution.allow_project_configuration         field_not_activated (bool)
execution.allow_foreign_configuration         field_not_activated (bool)
execution.stop_hook_continuations             StopHookContinuationsV1 (2 elements)
execution.tool_rules.<id>                     ToolRuleV1 (1 element)   ← activated
```

`PolicyModelEgressV1` (5): `allowed_providers allowed_models model_fallback web_search web_fetch`
(`web_fetch` = `field_not_activated`; `ModelFallbackV1` 2 elements; `WebFetchPolicyV1` 7).

`PolicyPrivacyV1` (5): `telemetry foreign_personal_rules foreign_personal_skills
local_session_messaging feature_config` — each rejects objects/booleans with `wrong_type` and
strings with `semantic_invalid`, i.e. a constrained string decision type.

`PolicyExtensionsV1` (3, PROVEN): `skills`, `runtime_capabilities`, `hooks`
(all `field_not_activated`; `plugins`/`mcp_servers` → `unknown_member`).
`SkillPolicyV1` 4 · `HookPolicyV1` 5 · `RuntimeCapabilityPolicyV1` 4.

Plus `local_session_messaging` (`LocalSessionMessagingPolicyV1`) at envelope level.

### 7.6 The runtime floor

The enterprise snapshot is projected into a "runtime floor" whose field list is:

```
snapshot.schema_version  sources  fields  permissions
runtime_floor.permissions  runtime_floor.opt_outs  runtime_floor.opt_out.field
runtime_floor.name  runtime_floor.decision (allow | ask)
permission.name  field.name  field.kind (atomic | map)  field.value  field.origin
field.entries  field.entry.value  field.entry.origin
source.class  source.defaults.state  source.policy.state
source.defaults.document.present / .schema_version / .settings
source.policy.document.present   / .schema_version / .empty
```

Opt-out-able floor fields: `telemetry.enabled`, `context.foreign_personal_rules`,
`context.foreign_personal_skills`, `local_session_messaging.enabled`, `feature_config.enabled`.
Every merged field carries a `field.origin`, so the effective config is fully attributable.

---

## 8. Environment variables

### 8.1 Path / config-relevant (the config crate's env allowlist, verbatim adjacent run)

```
MUSE_CUSTOM_HEADERS  TBH_UNKILL_SLASH_COMMANDS  MUSE_DISABLE_APPROVAL_JUDGE
XDG_CONFIG_HOME  XDG_DATA_HOME  CODEX_HOME  TBH_CREDENTIAL_BACKEND  META_API_KEY
TBH_VIDEO_INPUT_ENABLED  TBH_META_FILE_EXPIRY_SECS
ANTHROPIC_API_KEY  ANTHROPIC_BASE_URL  OPENAI_API_KEY  OPENAI_BASE_URL
OPENROUTER_API_KEY  OPENROUTER_BASE_URL  MUSE_MODEL
TBH_EVAL_APPEND_SYSTEM_PROMPT[_FILE]  TBH_EVAL_APPEND_DEVELOPER_PROMPT[_FILE]
TBH_NATIVE_SUBAGENT_DOGFOOD  TBH_VOICE_*  MUSE_DISABLE_VOICE_WAVE
OTEL_EXPORTER_OTLP_ENDPOINT  TBH_IS_E2E_TEST  TBH_COPY_TRANSPORT
TBH_FEEDBACK_ROUTE  TBH_FEEDBACK_INTAKE
SSH_TTY  SSH_CONNECTION  TMUX_PANE  WAYLAND_DISPLAY  DISPLAY
```

Others of note: `MUSE_CURRENT_SESSION_LOG`, `MUSE_SESSIONS`, `MUSE_TOOL_USE_ID`,
`MUSE_HUMAN_CONFIRMATION`, `MUSE_ENABLE_WEB_TOOLS`, `MUSE_WEB_SEARCH_MODE`, `MUSE_WWW_ROUTING`,
`MUSE_PLUGIN_ROOT`, `TBH_MANAGED_HOOKS_PATH`, `TBH_DISABLE_FEATURE_CONFIG`,
`TBH_DISABLE_TELEMETRY`, `TBH_SESSION_MESSAGE_SOCKET`, `TBH_MINT_BASE_URL`, `RUST_MIN_STACK`.

### 8.2 `CODEX_HOME` — honoured, PROVEN

```bash
CODEX_HOME=<dir> muse skills import --from codex --dry-run --json
→ {"source":{"type":"codex","path":"<dir>/skills"}, ...}
```

vs. default `$HOME/.codex/skills`. `CODEX_HOME` is used **only for third-party import/compat**:
* `$CODEX_HOME/skills` — personal skill import root
* `$CODEX_HOME/AGENTS.md` — personal rules import (default `~/.codex/AGENTS.md`)
* `$CODEX_HOME/sessions` — read-only transcript recovery for `rollout-*.jsonl`

The bundled `doctor` skill states the boundary explicitly:

> Variables such as `CODEX_HOME` matter only for explicitly requested import/compat evidence and
> stay attributed to the product that owns them.

`CLAUDE_CONFIG_DIR` is **NOT** honoured by the skill importer (PROVEN — it still resolved
`$HOME/.claude/skills`), even though the bundled `import` skill's prose mentions
`$CLAUDE_CONFIG_DIR/projects` for session search.

### 8.3 Feature gates

`MUSE_EXPERIMENTAL_*` (41 gates) map 1:1 onto a gate-registry enum:

```
workflow_tool artifact_tool local_session_messaging external_agent_ingress code_mode
prefix_compaction monitor foreign_personal_context_kill voice voice_native_capture
reasoning_display model_effort_context bash_titles bash_sandbox_escalation
git_sandbox_relaxation plugins enterprise_config web_fetch curl_web_fetch
web_fetch_preflight_hard_cap first_turn_minimal_effort todo_reminder memory_reminder
skill_reminder goal_reminder verify_reminder scope_reminder session_runtime
session_recovery_shadow tui_msp_clients sdk_enabled meta_context_workspace_only
non_strict_tool_params hook_selected_skills hook_selected_skills_apply
workflow_api_v2_rollout subscription_launch provider_tool_switch tag
```

PROVEN: `MUSE_EXPERIMENTAL_PLUGINS=1` reveals the `plugins` subcommand in `muse --help`;
`MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG=1` activates enterprise validation.

---

## 9. Precedence order

### 9.1 What is proven

1. **CLI flag > `settings.json`** — PROVEN.
   With `{"schema_version":1,"provider":"bogus_provider"}`, a bare `muse exec "hi"` fails with
   `unsupported provider \`bogus_provider\`; expected \`echo\` or \`meta\``, proving the file is
   read and applied. Adding `--provider echo` to the *same* config runs the session
   successfully — the flag wins.
2. **`AGENTS.md` > `CLAUDE.md`** in the same directory — PROVEN by the `muse init` warning.
3. **Deeper project rules file > shallower** — verbatim product string.
4. **Project rules > user rules** — verbatim product string
   ("If user and project rules conflict, project rules win").
5. **`XDG_*` > `HOME`**, exactly one root, no third fallback — PROVEN (§2.2).
6. **env `META_API_KEY` > stored `auth.json` login** — verbatim:
   `note: META_API_KEY overrides your Meta account login; unset it to use the account login, or run \`muse logout\``.
7. **Enterprise `policy` is a floor** — the merged model is literally named `runtime_floor`, with
   `runtime_floor.decision ∈ {allow, ask}` and an explicit `runtime_floor.opt_outs` list, i.e.
   the user may only opt out of the five fields the policy marks opt-out-able.
8. **Enterprise `defaults` is a seed** — the plane is named `defaults` and carries the same
   `settings.*` key space as the user file (`DefaultsEnvelopeV1 { schema_version, settings }`).

### 9.2 Resulting order (lowest → highest effective priority)

```
built-in defaults
  < enterprise DEFAULTS plane        (system_file | macos_managed_preferences | windows_machine_policy)
    < user settings.json             ($CONFIG_DIR/settings.json)
      < environment variables        (META_API_KEY, MUSE_MODEL, MUSE_EXPERIMENTAL_*, TBH_*)
        < CLI flags                  (PROVEN above CLI > settings.json)
          < enterprise POLICY plane  (runtime floor — clamps everything below it)
```

Notes and caveats:
* The **workspace tier is absent from this chain for settings** (§6.1). Workspace files
  participate only in the rules chain (user rules < project rules, shallower < deeper) and in
  skill/plugin discovery, both gated by workspace trust.
* env-vs-`settings.json` ordering is **INFERRED**. It is not directly proven for a key that
  exists in both surfaces; the only proven env-over-persisted case is `META_API_KEY` over the
  saved login. The placement is consistent with `MUSE_MODEL` being documented as a startup
  input ("set MUSE_MODEL for Meta startup") and with `MUSE_EXPERIMENTAL_*` gates overriding
  `field_not_activated` states.
* `execution.allow_project_configuration` and `execution.allow_foreign_configuration` are the
  policy switches that let an administrator disable the workspace and foreign-import tiers
  entirely; both are `field_not_activated` in this build.
* The `manage-settings` skill's completion template confirms the flag-over-file rule from the
  product side: *"For a setting such as `reasoning_effort` with higher-precedence startup input,
  append `unless a launch flag overrides the saved value`."*

---

## 10. Complete file/directory inventory

| Path | Tier | Purpose | Evidence |
|---|---|---|---|
| `$XDG_CONFIG_HOME/muse` \| `$HOME/.config/muse` | user | config root | PROVEN |
| `…/settings.json` | user | all user settings | PROVEN |
| `…/auth.json` | user | provider credentials (`AuthFile`, 2 fields) | PROVEN (launcher + literal) |
| `…/.auth.json.lock` | user | advisory lock, created every startup | PROVEN |
| `…/trust.json` | user | workspace trust store | literal + skill doc |
| `…/skills/<id>/` | user | managed personal skills | PROVEN |
| `…/skills/.muse/lock.json` | user | install lockfile | PROVEN (verbatim) |
| `…/skills/.muse/audit.log` | user | install audit JSONL | PROVEN (verbatim) |
| `…/skills/.muse/.skills.lock` | user | 0-byte advisory lock | PROVEN |
| `…/skills/.muse/quarantine/` | user | rejected packages | PROVEN |
| `$XDG_DATA_HOME/muse` \| `$HOME/.local/share/muse` | user | data root | PROVEN |
| `…/sessions/YYYY/MM/DD/<id>/session.jsonl` | data | durable event log | PROVEN + literal |
| `…/sessions/…/.session.lock` | data | inode-backed kernel lease | literal |
| `…/sessions/.msp-view-v1/<id>/` | data | materialized projection cache | PROVEN |
| `…/skills/bundled/muse-core/` | data | bundled skills unpacked from binary | PROVEN |
| `…/plugins/cache/<class>/…/<sha256>/` | data | content-addressed plugin cache | PROVEN |
| `…/local-tracing/bootstrap/cli-<uuid>.log` | data | startup traces | PROVEN |
| `…/memory/`, `…/model-catalog/`, `…/crashes/` | data | per `doctor` skill | doc |
| `~/Library/Application Support/Muse/session-name-authority` | data | session-name lock (macOS) | literal, INFERRED join |
| `/Library/Application Support/Muse/enterprise-defaults.json` | enterprise | defaults plane | **INFERRED** |
| `/Library/Application Support/Muse/enterprise-policy.json` | enterprise | policy plane | **INFERRED** |
| macOS managed prefs domain `com.tbh.tbh` | enterprise | MDM-delivered planes | PROVEN literal |
| Windows machine policy | enterprise | `windows_machine_policy` source class | literal |
| `<ws>/AGENTS.md` | workspace | project rules (scaffolded by `muse init`) | PROVEN |
| `<ws>/CLAUDE.md` | workspace | compat project rules (lower precedence) | PROVEN |
| `<ws>/.agents/skills/<id>/SKILL.md` | workspace | project skills (canonical) | PROVEN |
| `<ws>/.claude/skills/`, `<ws>/.codex/skills/` | workspace | compat project skills | PROVEN |
| `<ws>/.muse/worktrees/<YYYYMMDD>-<hex4>/` | workspace | session git worktrees | PROVEN |
| `<ws>/.muse/worktrees/.session-worktree-reservations/v1/{plans,by-leaf,by-session,tmp}/` | workspace | worktree reservation store | PROVEN (verbatim) |
| `<ws>/.git/info/exclude` | workspace | gets `/.muse/worktrees/` appended | PROVEN |
| `<ws>/.muse-plugin/plugin.json` | workspace | native plugin manifest | PROVEN |
| `<ws>/.codex-plugin/`, `<ws>/.claude-plugin/` | workspace | compat plugin manifests | literal |
| `.muse-claude-sources`, `.muse-codex-sources` | workspace | foreign marketplace source lists | literal |
| `.muse-memory.lock`, `.muse-memory-*` | — | memory lock / atomic-write temp prefix | literal |
| `~/.claude/skills`, `$CODEX_HOME/skills` | foreign | personal skill import roots | PROVEN |
| `~/.claude/CLAUDE.md`, `$CODEX_HOME/AGENTS.md` | foreign | personal rules import | PROVEN literal |
| `$HOME/.local/bin/muse` | install | launcher (`MUSE_INSTALL_DIR` overrides) | `install.sh:5` |
| `$XDG_CONFIG_HOME/fish/conf.d/muse.fish` | install | PATH shim | `install.sh:194` |

---

## 11. Open questions

1. The exact enterprise `system_file` directory could not be proven (writing under `/Library`
   is outside the sandbox). `Library` + `Application Support` are adjacent literals in
   `paths.rs`'s constant pool, but the middle component (`Muse` vs `muse` vs `TBH`) and the
   Linux/Windows equivalents are unconfirmed.
2. Whether `<workspace>/.muse/hooks.json` is ever auto-discovered. Poison tests through both
   `TBH_MANAGED_HOOKS_PATH` and `settings.managed_hooks_path` produced no diagnostic; hook
   loading may be gated behind `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS` or require a plugin.
3. Whether env vars sit above or below `settings.json` for a key present in both. Only
   `META_API_KEY` over the saved login is proven.
4. The `ChoiceV1` (2 elements) and `PolicyPrivacyV1` decision wire shapes — every probed shape
   returned `semantic_invalid`, suggesting a constrained string/tagged form not yet guessed.
5. Default *values* for `provider_retry.*`, `tools.web_fetch.*`, `context_compaction.*` and
   `agents.execution_capacity`. Types and ranges are proven; the built-in defaults are compiled
   constants and were not recovered.
6. `trust.json` on-disk shape (`ProjectTrustStore` 2 fields, `ProjectTrustEntry` 1 field) — no
   CLI writes it, so it was never materialized in the sandbox.
7. Whether `mcp_servers` in the user `settings.json` is a true serde alias for `mcpServers` or a
   separate deprecated field; both were accepted simultaneously, and unknown-key tolerance makes
   the two cases indistinguishable from outside.

---

# Verification

Adversarial re-run by a second agent in
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/verify-config-paths/`
(sandboxed `HOME`, `MUSE_NO_AUTO_UPDATE=1`, `--provider echo` only, no network, no auth).
Every line below is either a command I re-ran or a byte offset in the binary.

**Verdict: MOSTLY_SOLID.** The path model, the negative "no workspace settings file" result, the
type/enum oracle, and the whole enterprise two-plane schema reproduce exactly. Five claims are
wrong, one of them badly (`.muse/hooks.json`), and one "oh-my" hook is actively dangerous advice.

## V.1 Refuted

### R1 — `<workspace>/.muse/hooks.json` **IS** auto-discovered (refutes §6.5 / finding 22)

The original poison test failed for two reasons the report did not control for: workspace hooks are
**trust-gated**, and a malformed hook file is **silent** in headless `exec`. With a well-formed
document and trust granted, the hook runs:

```bash
mkdir -p .muse .agents .claude .codex .muse-plugin
# same doc planted at .muse/hooks.json, .agents/hooks.json, hooks.json,
# .claude/hooks.json, .codex/hooks.json, .muse-plugin/hooks.json, .muse/settings.json
printf '{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"touch %s/HIT-muse"}]}]}}' "$PWD" > .muse/hooks.json
muse exec "hi" --trust-workspace   # -> HIT-muse created
muse exec "hi"                     # -> nothing   (untrusted)
muse exec "hi" --yolo              # -> HIT-muse created
```
Only `.muse/hooks.json` fires; every other candidate path is inert. So the workspace tier **does**
have an executable configuration file, and the report's headline ("workspace influence is limited
to rules, skills, plugins and worktrees") needs `.muse/hooks.json` added to it.

Corroborating literals (present but mis-read by the report): `malformed hook config at `,
`failed to read hook config at `, and the adjacency `local_session_messaging` + `.muse` +
`hooks.json` in `strings.txt`.

### R2 — hook event keys are **PascalCase**, not snake_case (refutes §5.14)

```
SessionStart      FIRED        session_start   no
UserPromptSubmit  FIRED        sessionStart    no
Stop              FIRED        SESSION_START   no
SessionEnd        FIRED
PreLLMCall        FIRED
PostLLMCall       FIRED
```
(`.muse/hooks.json` with `{"hooks":{"<Event>":[{"hooks":[{"type":"command","command":"touch HIT-<Event>"}]}]}}`,
run with `--trust-workspace`.) The report's own finding 24 quotes the correct PascalCase blob
`HookEventKindSessionStartUserPromptSubmit…PostToolBatch`; §5.14 of the prose then renders it in
snake_case, which is the one shape that provably does **not** work. `settings.hooks` uses the same
PascalCase keys (verified: `hooks.SessionStart` fires from `settings.json`, `hooks.session_start`
does not).

### R3 — `tui.voice_tap_enabled` is not a user settings key (refutes §5.4, 15 fields → 14)

The `TuiSettings` serde FIELDS run in the binary is exactly 14 names and does not contain it:
```
TuiSettings show_reasoning reasoning_summaries away_recap_enabled prompt_hint_enabled
voice_enabled voice_shortcut_mode keymap resize_reflow_cap color_depth terminal_background
theme rules_import_offer_dismissed foreign_context_notice_shown verbose_output
```
Duplicate-field oracle (below) says REAL for all 14 and `not-a-field` for `voice_tap_enabled`
across 14 sentinel values (`null true false 1 0 "on" "tap" "auto" {} [] "enabled" "hold_to_talk"
0.5 "true"`). The only place the token exists is the **enterprise** key registry
(`strings.txt:55915`): `settings.tui.voice_enabled` `settings.tui.voice_tap_enabled` … i.e. it is a
`TuiDefaultsV1` member with no user-settings counterpart. This is exactly the "field name lifted
from an adjacent unrelated blob" failure mode.

### R4 — `schema_version` is hard-required, not "mandatory-by-convention" (refutes §5.1)

```
{"provider":"bogus_provider"}   -> malformed settings file …: missing field `schema_version`
{"schema_version":"1", …}       -> invalid type: string "1", expected u32
{"schema_version":null, …}      -> invalid type: null, expected u32
{"schema_version":0}            -> unsupported settings schema version 0 at <path>
{"schema_version":2}            -> unsupported settings schema version 2 at <path>
```
The report's earlier probe only appeared to pass because serde reports the *first* bad field and
the missing-field check runs last. Also new: top-level type name is `a settings JSON object`
(`[]` → `invalid type: sequence, expected a settings JSON object`), and JSON is strict
(`trailing comma at line 1 column 49`).

### R5 — personal rules are read **in place**, not imported (refutes §6.2)

```bash
echo USER_CLAUDE_MARKER > $HOME/.claude/CLAUDE.md
muse exec --provider echo --trust-workspace "hi"
# stderr: muse: Including your Claude Code personal rules — manage with /settings.
```
and the session log's context block contains, verbatim:
```
<rules-file scope="user" path="~/.claude/CLAUDE.md" written-for="Claude Code">
USER_CLAUDE_MARKER
</rules-file>
```
`muse skills import --from claude` is a *separate, optional* copy-into-the-store command; it is not
how personal rules reach a session. Full ladder proven in V.3.1.

### R6 — DANGEROUS: a framework must **not** stash marker keys in `settings.json`
(refutes ohmy_hook #1)

Unknown keys are ignored on **read** but destroyed on **write**. Muse rewrites the file during an
ordinary run:
```bash
printf '{"schema_version":1, "_omm_marker": "KEEPME", "provider":"echo"}\n' > $CONFIG/settings.json
muse exec "hi"
cat $CONFIG/settings.json
{ "schema_version": 1, "provider": "echo", "tui": { "foreign_context_notice_shown": true } }
```
`_omm_marker` is gone. The file is re-serialised from the typed struct (2-space pretty-print), so
**any** unknown key a framework writes is silently lost on the user's next session. The report's
"the framework may add its own namespaced marker key for ownership tracking" would lose state.

### R7 — `muse config` is **not** gated (refutes §7.1)

`muse config status` and `muse config validate --plane … --file …` both work with no
`MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG`, and `config` is listed in a bare `muse --help`. Setting the
gate changes nothing — including `field_not_activated`, which is a compiled-in build property that
`MUSE_EXPERIMENTAL_VOICE=1` does **not** flip:
```
{"schema_version":1,"settings":{"tui":{"voice_enabled":true}}}
  no gate / VOICE=1 / ENTERPRISE_CONFIG=1 / both  -> field_not_activated: plane=defaults
```
So ohmy_hook #7's "programmatic capability probe" via `field_not_activated` reports what the
**binary** was built with, not what the user's gates enable.

### R8 — `settings.skills.activation` keys are SKILL.md **paths**, not ids (refutes §5.11)

`muse skills disable … --json` reports `"settings_path": "$CONFIG_DIR/settings.json"` and writes:
```json
"skills": { "activation": {
  "user":     { "$CONFIG_DIR/skills/myskill/SKILL.md": "off" },
  "projects": { "/abs/path/to/workspace": {
        ".agents/skills/probe/SKILL.md": "off",
        ".claude/skills/probe2/SKILL.md": "user-invocable-only" } },
  "bundled":  { "bundled://muse-core/skills/git/SKILL.md": "off" } } }
```
Project keys nest under the **absolute workspace root**; bundled keys are `bundled://` URIs.
Activation values are `on | off | user-invocable-only` (the report only observed `"on"`).
Note `--scope built-in` writes into the `bundled` bucket.

## V.2 Corrections

### C1 — `mcpServers` / `mcp_servers`: not an alias, and a collision silently voids both

A duplicate-field oracle separates real derived fields from ignored keys far better than the
type oracle (a duplicated real field errors regardless of value type):
```
tui, provider, schema_version, agents, agent_definitions, model, reasoning_effort,
first_turn_minimal_effort_regex, context_compaction, run, provider_retry, context,
local_session_messaging, feature_config, tools, skills, model_catalog, presets, hooks,
runtime_capabilities, permissions, plugins, managed_hooks_path, managed_hooks_env_vars,
max_consecutive_stop_hook_continuations, endpoint_transport, telemetry, notifications
                                                          -> "duplicate field `<k>`"  (28 real)
mcpServers, mcp_servers, totally_bogus_key, mcpservers    -> no error at all
```
If `mcp_servers` were a serde `alias` of `mcpServers` the pair would still raise
`duplicate field`. It does not — so they are handled outside the derive path (flatten or a custom
deserializer). Both are nevertheless **functionally live** (proven by launching a real stdio server
from each):
```
mcpServers only   -> "Required MCP server `camel` failed during startup: the startup operation timed out."
mcp_servers only  -> "Required MCP server `snake` failed during startup: …"
BOTH present      -> no server started, no error, no diagnostic   <-- silent total loss of MCP config
```
Also silent: `"mcpServers":12345`, an entry with neither `command` nor `url`, an entry with
**both**, and a blank server name — none produce any of the validation literals the report quotes
in §5.12 (`transport requires command or url`, `transport is ambiguous…`, `server name must not be
blank`). Those strings exist but are not reachable from a user `settings.json` load. The report's
open question 7 should be restated: it is not "alias vs deprecated field", it is "neither key goes
through the derived struct, and setting both is a silent config wipe".

`cwd` (§5.12) is correct — the binary has `…framing` `cwd` `url` `headers` `startup_timeout_sec`
`enabled_tools` `disabled_tools`, so `McpServerSettings` has 13 fields.

### C2 — `struct SettingsFileDocument with 29 elements` arithmetic

Union of the two FIELDS blobs (`strings.txt` around lines 55957 and 55865) is 30 distinct names;
28 are confirmed real by the duplicate oracle; the struct is 29. `skills` is **not** lazily parsed
(it types as `struct SkillsSettings`); `max_consecutive_stop_hook_continuations` types as `usize`
only with a non-integer sentinel. The genuinely lazy (RawValue) keys are exactly five:
`model_catalog hooks runtime_capabilities permissions plugins`.

### C3 — feature-gate registry is 41, and the report's list has three errors (§8.3)

There are **41** `MUSE_EXPERIMENTAL_*` env names and **41** registry variants — the report's
enumerated list has only 39. Missing: **`voice_default_on`** (`MUSE_EXPERIMENTAL_VOICE_DEFAULT_ON`)
and **`server_web_fetch`** (`MUSE_EXPERIMENTAL_SERVER_WEB_FETCH`); and `tui_msp_clients` is really
`tui_msp_client` (the following token is `sdk_enabled`). Both missing names appear verbatim in the
full registry run and again in a second blob next to `field.entry.name`.
`MUSE_EXPERIMENTAL_PLUGINS=1` adding `plugins  Validate and manage plugin bundles` to `muse --help`
reproduces exactly; no other gate changes the top-level command list.

### C4 — enterprise `tui` activation table (§7.4): reproduces, plus one omission

The report's split is correct **only when the probe value is type-correct** — `{}` short-circuits
to `wrong_type` before the activation check. With correct types:
```
show_reasoning reasoning_summaries away_recap_enabled voice_tap_enabled
resize_reflow_cap verbose_output keymap                     -> valid
voice_enabled voice_shortcut_mode color_depth theme         -> field_not_activated
terminal_background prompt_hint_enabled
rules_import_offer_dismissed foreign_context_notice_shown   -> unknown_member
```
The report omits **`keymap`** (a real, activated `TuiDefaultsV1` member) from its table, which is
why its 11-member accounting only works by accident.

### C5 — enterprise error taxonomy is wider than §7.3 reports

Four error **classes**, not one: `enterprise_document_invalid`, `enterprise_document_malformed`,
`enterprise_schema_unsupported`, `enterprise_source_unreadable` (the last carries
`remediation=run muse config validate for the reported plane, repair the administrator source, and retry`).
Reasons beyond the seven listed: `duplicate_member`, `trailing_content`, `null_not_allowed`,
`skipped_oversize`, `runtime_mutation`.
```
{"schema_version":1,"settings":{},"settings":{}}  -> enterprise_document_malformed: reason=duplicate_member
{"schema_version":1,"settings":{}} trailing       -> enterprise_document_malformed: reason=trailing_content
{"schema_version":1,"settings":{"provider":null}} -> enterprise_document_invalid:   reason=null_not_allowed
```

### C6 — the `/Library/Application Support/Muse/enterprise-*.json` inference is weaker than stated

The offsets in the report are right (`LibraryApplication Support` @ `0xbb9ba84` = 196721284,
`no usable config directory` @ 196720386) but the two are separated by ~800 bytes of non-string
relocation data, and the nearest *named* consumer in that neighbourhood is
`CredentialProvider` / `auth_path`, not the enterprise reader. There is **no** literal
`/Library/Application Support/Muse`, no `/etc/…` path, no `SOFTWARE\…` or `Policies` string for
`windows_machine_policy`, and none of the bundled skills mentions an enterprise path at all
(`grep -ri enterprise` over `skills/bundled/muse-core` returns only an unrelated `create-plugin`
line). Planting `enterprise-defaults.json` under `$HOME/Library/Application Support/Muse/`,
`$HOME/Library/Preferences/com.tbh.tbh.plist`, `$HOME/Library/Managed Preferences/com.tbh.tbh.plist`,
`$CONFIG_DIR/`, `$CONFIG_DIR/enterprise/`, `$DATA_ROOT/` and `$DATA_ROOT/enterprise/` all left
`state=absent`. Keep this as *unsupported inference*, not "almost certainly".

### C7 — smaller fixes

* `muse config status` and `validate` work with **no** gate (R7).
* `XDG_CONFIG_HOME`/`XDG_DATA_HOME` are honoured even when **relative** (`XDG_CONFIG_HOME=relc`
  creates `./relc/muse` relative to cwd) — a spec deviation worth knowing before a framework sets them.
  Empty-string values fall back to `HOME`; with neither, `muse skills list` dies with
  `no usable config directory from XDG_CONFIG_HOME or HOME` while `muse init` still works.
* `terminal_background` variant list is provable at runtime, not just from docs:
  `invalid terminal_background "nope": expected "auto", "light", or "dark"`.
* `reasoning_effort` is **not** validated at settings-load (`"nope"` is accepted there); the
  `none|minimal|…|ultra` list in §5.3 comes from the CLI flag help, not the file schema.
* `~/.local/share/muse/sessions/.msp-view-v1/<id>/` also contains `HEAD.json` and
  `journal-00000000.bin` (§4 lists only the snapshot and index).
* `memory/`, `model-catalog/`, `crashes/` were never materialised in any run — doc-only, as the
  report's evidence column admits, but §4's tree presents them as observed.
* `muse schema generate-json-schema --out DIR --experimental` emits `msp.schema.json` +
  `manifest.json` with **zero** occurrences of `settings`, `trust` or `enterprise` — the MSP wire
  schema is not a config source. Worth recording as checked-and-empty.
* `install.sh` claims verify: `install_dir="${MUSE_INSTALL_DIR:-$HOME/.local/bin}"` (line 5),
  `fish_file="${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/muse.fish"` (line 151).

## V.3 Missed ground

### M1 — `$CONFIG_DIR/AGENTS.md` is Muse's own **personal rules file**, and §3's tree omits it

This is the single biggest omission. The config root holds a native user-scope rules file, and
there is a four-rung ladder in which **exactly one** user rules file loads (first hit wins):

| rung | path rendered in the context block | `written-for` |
|---|---|---|
| 1 | `$CONFIG_DIR/AGENTS.md` | — (native) |
| 2 | `$CONFIG_DIR/CLAUDE.md` | — (native compat) |
| 3 | `~/.claude/CLAUDE.md` | `Claude Code` |
| 4 | `~/.codex/AGENTS.md` / `$CODEX_HOME/AGENTS.md` | `Codex` |

Proven by deleting one rung at a time and reading the session log. Rungs 3–4 are killed outright by
`{"schema_version":1,"context":{"foreign_personal_rules":false}}` (no user rules block at all);
rungs 1–2 are not. `$HOME/AGENTS.md`, `$HOME/.agents/AGENTS.md`, `$HOME/.muse/AGENTS.md` and
`$CONFIG_DIR/rules.md` are all inert. For an oh-my-musecode framework this is *the* place to ship
house rules — and it does not exist in the report.

### M2 — the whole rules chain is provable at runtime, not just from a `muse init` warning

Marker files + the `session.jsonl` context block give the real thing (git repo, cwd `pkg/api`):
```
<system-reminder source="rules-file">
Muse Code loaded standing rules at session open. Follow higher-priority instructions first.
If user and project rules conflict, project rules win. If project rules files conflict, the
deeper file wins over the shallower one.
<rules-file scope="user" path="~/.claude/CLAUDE.md" written-for="Claude Code">USER_CLAUDE_MARKER
<rules-file scope="project" path="AGENTS.md">ROOT_AGENTS_MARKER
<rules-file scope="project" path="pkg/AGENTS.md">PKG_AGENTS_MARKER
<rules-file scope="project" path="pkg/api/AGENTS.md">API_AGENTS_MARKER
```
Facts the report could not state: project rules are **accumulated** from the git root down to cwd
(not "nearest wins"); the "deeper file wins" sentence is emitted only when ≥2 project files load;
`CLAUDE.md` is loaded as `scope="project"` only where no sibling `AGENTS.md` exists; and the
per-directory shadowing has its own runtime warning:
```
muse: warning: rules file at <ws>/pkg/api/CLAUDE.md is ignored this session because
pkg/api/AGENTS.md takes precedence in that directory; merge still-applicable guidance into
pkg/api/AGENTS.md or remove one of the two files
```
Outside a repo the scan does **not** walk above the workspace root: from `sub/`, only
`sub/AGENTS.md` loads.

### M3 — env **does** beat `settings.json` (closes open question 3)

```
settings.tools.web_search.mode = "client",  no env            -> web_search active
settings.tools.web_search.mode = "client",  MUSE_ENABLE_WEB_TOOLS=0 -> web_search ABSENT
settings.tools.web_search.mode = "off",     MUSE_ENABLE_WEB_TOOLS=1 -> web_search + web_fetch active
```
read off `"active_tools"` in `model_request_configured` in `session.jsonl`. Note also that
`tools.web_search.mode:"off"` alone does **not** remove the tool — only the env var does.
The same event carries a **provenance field** the report never found:
`"toolset":{"source":"settings","mode":"named","active_tools":[…]}` vs `"source":"default"` —
a ready-made oracle for testing any future precedence question offline.

### M4 — `ChoiceV1` and the privacy decision type are solved (closes open question 4)

* `ChoiceV1` = `{"allowed":[<variant>,…],"fallback":<variant>}`, and `fallback` **must be a member
  of `allowed`** — `{"allowed":["on_request"],"fallback":"allow_all"}` → `semantic_invalid`,
  `{"allowed":["on_request"],"fallback":"on_request"}` → `field_not_activated` (shape accepted).
  `ApprovalModeV1` = `on_request | prompt_unmatched | allow_all`;
  `NetworkSandboxModeV1` = `restricted | proxy_only | enabled`.
* `PolicyPrivacyV1` fields take a **bare string** `"force_off"` (`ForceOffV1`, one variant):
  `"force_off"` → `field_not_activated`; any other string → `semantic_invalid`; any non-string
  (bool/int/object/array, including `{"force_off":true}`) → `wrong_type`.
* `StopHookContinuationsV1` = `{"maximum":<int>,"fallback":<int>}` (both integers).
* `ToolRuleV1` = `{"decision":"deny"|"ask"|"allow"}`; the validator renders the location as the
  literal `execution.tool_rules.<id>`.
* `SkillPolicyV1` = `{allowed_identities, denied_identities, allowed_sources, fallback}` with
  `SkillPolicyFallbackV1` = `off | user-invocable-only`.
* `extensions.hooks` **is** a member (`field_not_activated`), so `PolicyExtensionsV1` = skills,
  hooks, runtime_capabilities.
* `local_session_messaging.receiver_limits` is the one policy field that is `valid` **and**
  activated.
* `model_egress.allowed_providers` returns `semantic_invalid` for every array I tried, including
  `["meta"]` and `[]`, and any other `model_egress` member re-reports the error at
  `location=model_egress.allowed_providers` — an undocumented cross-field requirement.

### M5 — the enterprise **key registry** exists as one literal blob; probing was unnecessary

`strings.txt` ~line 55868 holds the authoritative activated-key list, which independently confirms
every arity in §7.4/§7.5 without a single `config validate` call:
```
model_egress.{allowed_providers,allowed_models,model_fallback,web_search,web_fetch}
privacy.{telemetry,foreign_personal_rules,foreign_personal_skills,local_session_messaging,feature_config}
extensions.{skills,runtime_capabilities}          ( + extensions.hooks in a second blob )
settings.run.{system_prompt,developer_prompt,toolset,parallel_tool_calls,workflow_trigger_mode,
              workflow_api_version,subagent_delegation_mode,code_mode,context_usage_message_enabled,
              reminder_roster,context_slimming}                                        = 11 = RunDefaultsV1
execution.{forbid_approval_bypass,forbid_sandbox_bypass,force_agent_definition_safe_mode,
           permission_profiles,approval_modes,approval_reviewers,network_sandbox_modes,
           allow_project_configuration,allow_foreign_configuration,stop_hook_continuations,
           tool_rules.<id>}                                                            = 11 = PolicyExecutionV1
settings.provider_retry.{max_retries,base_delay_ms,retry_after_cap_ms}                 = 3  (no max_attempts!)
settings.tui.{show_reasoning,reasoning_summaries,away_recap_enabled,voice_enabled,voice_tap_enabled,
              voice_shortcut_mode,keymap,resize_reflow_cap,color_depth,theme,verbose_output} = 11
settings.telemetry.{enabled,endpoint,proxy,destination,auth,artillery}                 = 6
settings.endpoint_transport.{base_url,proxy,auth}                                      = 3
settings.context.{foreign_personal_rules,foreign_personal_skills}                      = 2
settings.notifications.{events,method,condition}                                       = 3
settings.context_compaction.{strategy,soft_threshold,hard_threshold,
                             provider_context_limit_tokens,tool_result_clearing_enabled} = 5
settings.{provider,model,reasoning_effort,first_turn_minimal_effort_regex,
          max_consecutive_stop_hook_continuations}
settings.mcp_servers.<id> · settings.presets · settings.feature_config.enabled
settings.local_session_messaging.enabled · settings.agents.execution_capacity
```
Note `provider_retry` exposes only 3 of its 4 user fields to enterprise (`max_attempts` is
user-only), which the report's `ProviderRetryDefaultsV1 3` arity hints at but never explains.

### M6 — the shell sandbox hard-protects the config dirs

The `sandbox_policy` context block (verbatim from `session.jsonl`) ends:
> Nested or unrelated .git paths and **.muse, .agents**, .sl, .hg, and .eden stay read-only;
> system temp roots remain full read/write except sandbox-owned enforcer files.

So an agent-authored change to `.muse/hooks.json` or `.agents/skills/` cannot be made by the model
through the sandboxed shell — a framework has to write those before/outside a session. Directly
relevant to ohmy_hook #3 and #6 and not mentioned anywhere in the report.

### M7 — smaller gaps

* `ContextSlimmingSettings` = `skill_catalog_descriptions full_skill_description_ids
  meta_context_note_enabled session_identity_enabled excluded_tool_names` (§5.5 names the struct
  but never its fields); `RunPresetSettings` = `provider agent_profile run`;
  `ReminderRosterSettings` = `skip_if_running max_in_flight_per_agent agents`.
* Unknown keys are ignored at **every** nesting level (`tui.bogus_tui_key` is silent), so the
  type-name oracle cannot confirm any nested field either — only the duplicate-field oracle can.
  Any nested field list in §5.4–5.9 that was not duplicate-tested should be treated as unverified.
  I duplicate-tested and confirmed: all 9 `telemetry.*` (so `telemetry.enabled` **is** real despite
  being absent from the adjacent literal run — short literals like `enabled` are interned
  elsewhere), all 3 `tools.*`, all 8 `tools.web_fetch.*` (`enabled` included), `tools.artifact.enabled`
  (but **not** `tools.artifact.mode`), `tools.web_search.mode` (but **not** `tools.web_search.enabled`).
* `muse plugins` also has `approve`, `reject`, `inspect`, and `hook test <plugin-id>:<hook-id>
  --fixture <path>`; the installer's capability buckets are reported snake_case
  (`skills hooks mcp_servers commands reminders`) while the manifest uses camelCase `mcpServers`.
  A non-empty `capabilities.skills` entry must be an **object**, not a path string
  (`skill capability must be an object`).
* `PathState` `unreadable` was not reachable via the bootstrap trace; a `chmod 000` config parent
  surfaces instead as `failed to read skill file at …: Permission denied (os error 13)`, and a
  config root that is a regular file gives `failed to read settings file at …: Not a directory`.
* `trust.json` still never materialises — no CLI path writes it, `--yolo` included. Open question 6
  stands.

