# Meta Muse Code ("TBH") 1.0.1-R2006.1 — Plugin subsystem, reverse-engineered

Binary: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
Crate: `fbcode/musecode/build/src/crates/plugins/` (only leaked panic path: `plugins/src/capability_snapshot/diagnostic.rs`), plus `config/src/skills/`, `config/src/trust/`, `config/src/agent_definitions/composition/`.

Everything below was produced in a sandbox with
`HOME=<scratch>/sandbox/plugins/fakehome`, `MUSE_NO_AUTO_UPDATE=1`, `MUSE_EXPERIMENTAL_PLUGINS=1`.
Nothing outside the scratchpad was touched; no network calls to Meta; no login.

`M=<scratch>/muse-aarch64-macos`

---

## 0. TL;DR

* The whole `muse plugins` CLI is **feature-gated**. Without the gate:
  `$ M plugins --help` → `plugins are not available in this build`.
  With `MUSE_EXPERIMENTAL_PLUGINS=1` a full 16-verb command tree appears **and**
  `plugins  Validate and manage plugin bundles` is added to `muse --help`.
* A plugin package is a directory holding **exactly one** manifest, one of:
  * `.muse-plugin/plugin.json`  → family `native` (lockfile writes `"muse"`)
  * `.claude-plugin/plugin.json` → family `claude-compatible` (lockfile `"claude"`)
  * `.codex-plugin/plugin.json`  → family `codex-compatible` (lockfile `"codex"`)
  * root `plugin.json` carrying "the exact Agent Plugins 1.0.0 `$schema`" → family `agent-plugins` (**schema literal not recovered — see Open Questions**)
* Native capability families: `skills`, `commands`, `hooks`, `mcpServers`, `reminders`
  (+ `developerPrompts`, present in the schema but hard-gated off in this build).
  Rejected outright: `tools`, `agents`, `outputStyles`, `settings`, `apps`.
* **Claude Code plugins are a first-class supported input.** `commands`, `skills`,
  `hooks` (full `hooks.json` incl. `matcher`, `${CLAUDE_PLUGIN_ROOT}`) and `mcpServers`
  are translated. `agents`, `outputStyles`, `lspServers`, `userConfig`, `allowed-tools`
  enforcement, `author/homepage/repository/license/keywords` are dropped with warnings.
* **Codex plugins are much thinner**: only `skills` (a single string path) and
  `mcpServers` (a path to a `.mcp.json`) are imported; `commands`, `apps`,
  `dependencies`/`requiredPlugins` are not.
* Two lockfiles, both under `$DATA_DIR/muse/plugins/`: `installed.json` and
  `marketplaces.json`. Plus a per-marketplace `snapshot.json`.
* Trust is **per runtime capability**, keyed by a `definition_hash` that binds the
  plugin package digest, and is stored in `$CONFIG_DIR/settings.json` →
  `runtime_capabilities`. Hooks, MCP servers and Agent Definitions are inert until
  `muse plugins approve`; skills and commands are live as soon as the plugin is enabled.
* Marketplaces exist (local dir, local file, Git), including native support for
  Claude Code's `.claude-plugin/marketplace.json`. `.muse-claude-sources/` and
  `.muse-codex-sources/` are the clone targets Muse creates *inside a Git marketplace
  worktree* for per-plugin remote (`"source":"url"`/`"github"`) entries.
* Plugins are **not** on the MSP wire: `muse schema generate-json-schema --experimental`
  produces `msp.schema.json` with **zero** occurrences of `plugin`.

---

## 1. CLI surface

### 1.1 Gate

```
$ MUSE_NO_AUTO_UPDATE=1 $M plugins --help
plugins are not available in this build

$ MUSE_NO_AUTO_UPDATE=1 MUSE_EXPERIMENTAL_PLUGINS=1 $M plugins --help
usage: muse plugins <command>

Commands:
  install <path> [--scope user|project] [--json]
                               Install a local plugin bundle into the cache
  install <plugin>@<marketplace> [--json]
                               Install a plugin from a configured marketplace snapshot
  list [--available] [--json]  List installed or available plugins
  inspect <id> [--json]       Inspect one installed plugin and its runtime capabilities
  approve <plugin-id[[:kind]:capability-id] | stable-id> [--json]
                               Trust and enable current runtime capability definitions
  reject <plugin-id[[:kind]:capability-id] | stable-id> [--json]
                               Trust and disable current runtime capability definitions
  hook test <plugin-id>:<hook-id> | plugin:<plugin-id>:hook:<hook-id> --fixture <path> [--json]
                               Run one installed plugin hook against a fixture
  marketplace add <name> <source> [--json]
                               Add a local file/directory or Git marketplace source and store a snapshot
  marketplace list [--json]   List configured marketplaces
  marketplace update <name> [--json]
                               Refresh a marketplace snapshot explicitly
  marketplace remove <name> [--json]
                               Remove a marketplace source and snapshot
  enable <id> [--json]        Enable an installed plugin for future runtime loads
  disable <id> [--json]       Disable an installed plugin
  update <id> [--json]        Refresh an installed local plugin from its source
  remove <id> [--delete-data] [--json]
                               Remove an installed plugin record and cached package
  validate <path> [--json]    Validate a local plugin bundle without installing or executing it
```

The feature-gate name inside the binary is `plugins` (from the gate table
`...git_sandbox_relaxation | plugins | enterprise_config | web_fetch ...`), env var
`MUSE_EXPERIMENTAL_PLUGINS`.

### 1.2 TUI surface (from strings, not exercised)

* `/plugins` — "Use /plugins to inspect and manage installed plugins."
* `/plugins marketplace` — the composer tip reads
  "`/plugins marketplace` installs new commands and skills / user wants a capability the agent lacks".
* Sub-panels: `/plugins install`, `/plugins inspect`, `/plugins marketplace add`.
* Marketplace-add dialog prompt: `Add marketplace` / `Marketplace source` /
  **`owner/repo, git URL, file:// URL, or local path`** / `enter add - esc cancel`.
* `runtime capability changed; refresh plugin details`.

---

## 2. Package layout and manifest discovery

### 2.1 Manifest selection (verbatim error text)

```
plugin root must contain one supported plugin manifest: a root plugin.json with the
exact Agent Plugins 1.0.0 $schema, or exactly one nested
.muse-plugin/.codex-plugin/.claude-plugin plugin.json
```
and
```
plugin root contains more than one supported plugin manifest
```

Root-`plugin.json` format-selection diagnostics (`code: ignored-root-manifest`):

| condition | message |
|---|---|
| unreadable | `root plugin.json is unreadable; ignored for format selection` |
| not JSON object | `root plugin.json is not a JSON object; ignored for Agent format selection` |
| `$schema` not a string | `root plugin.json \`$schema\` is not a string; ignored for Agent format selection` |
| unparseable | `root plugin.json is unparseable; ignored for format selection` |
| unknown value | `root plugin.json declares unknown root format "<v>"; not an Agent Plugins marker; ignored for format selection` |
| Agent marker, wrong version | `root plugin.json declares unsupported Agent Plugins schema <v>; the package is rejected with no nested-parser fallback` |

Reproduced:

```
$ $M plugins validate ./ap-plugin --json      # root plugin.json, $schema=https://example.com/nope.json
{"error":{"code":"missing-manifest", ... "diagnostics":[
  {"code":"ignored-root-manifest","severity":"warning",
   "message":"root plugin.json declares unknown root format \"https://example.com/nope.json\"; not an Agent Plugins marker; ignored for format selection"},
  {"code":"missing-manifest","severity":"error","message":"plugin root must contain one supported plugin manifest: ..."}]}}
```

### 2.2 Package hygiene rules (all from binary strings, several confirmed at runtime)

* `plugin contains symlink entries and cannot be installed; replace symlinks with regular files` — **confirmed** (a single `link.txt -> /etc/hosts` in the package failed validation).
* `plugin packages may contain only files and directories`
* `plugin package contains duplicate entry paths`
* `plugin <id> directory nesting exceeds the maximum depth of <N>`
* `plugin <id> directory contains more than <N> filesystem entries; files and directories both count`
* `plugin manifest exceeds <N> byte limit`
* `plugin capability path must be relative` / `must not be empty` / `must stay inside plugin root` / `plugin capability paths must be UTF-8` / `plugin capability path escapes plugin root`
* ASCII-case shadowing checks: `ASCII-case collision`, `ASCII-case file/directory shadow`, `file/directory shadow`.
* Installation goes through a **frozen / no-follow** view of the source: a long list of
  TOCTOU guards (`frozen validation file changed during reading`, `held project plugin
  source inventory changed during snapshot`, `plugin package file identity changed while
  inventorying it`, `frozen validation path escaped its held source`, …). The stage →
  publish pipeline names are `stage`, `staged package`, `published package`, `quarantine`.

---

## 3. Native `.muse-plugin/plugin.json` — full schema

### 3.1 Top level

| field | type | required | notes |
|---|---|---|---|
| `schemaVersion` | integer | **yes** | must be exactly `1` (`plugin manifest must declare schemaVersion 1`) |
| `name` | string | **yes** | plugin id, grammar below. `plugin manifest must declare string field \`name\`` |
| `displayName` | string | no | shown in `list`/`inspect`; `null` when absent |
| `version` | string | **yes** | `plugin manifest must declare string field \`version\``, `plugin version must not be empty`. Free-form (`"0.1.0"` and `"2.3.4"` both accepted) |
| `description` | string | no | |
| `compat` | object | **yes** | `{ "source": "native", "manifestDir": ".muse-plugin" }` |
| `compat.manifestDir` | string | **yes** | must equal the actual manifest directory. `plugin manifest must declare compat.manifestDir`; mismatch → `compat.manifestDir \`.claude-plugin\` does not match actual manifest directory \`.muse-plugin\`` |
| `capabilities` | object | **yes** | `plugin manifest must declare capabilities`; `plugin capabilities must be a JSON object`. May be `{}` (validated true) |

Confirmed edge cases:

```
--- missing compat        | valid False | manifest-family-mismatch | plugin manifest must declare compat.manifestDir
--- wrong manifestDir     | valid False | manifest-family-mismatch | compat.manifestDir `.claude-plugin` does not match actual manifest directory `.muse-plugin`
--- schemaVersion 2       | valid False | invalid-manifest-schema  | plugin manifest must declare schemaVersion 1
--- no capabilities       | valid False | invalid-manifest-schema  | plugin manifest must declare capabilities
--- empty capabilities {} | valid True
```

### 3.2 Identifier grammar

From the bundled `create-plugin` reference (`references/native-plugin-contract.md`) and
`capability-examples.json`:

```
^[a-z0-9][a-z0-9._-]{0,79}$        (1..80 ASCII bytes)
```
plus an authoring rule (creator-side only, **not** enforced by the validator) that the
basename before the first dot must not case-fold to `CON PRN AUX NUL COM1..COM9 LPT1..LPT9`.

Runtime confirmation:

```
--- id=UPPER   | valid False | invalid-plugin-id | plugin id `UPPER` is invalid
--- id=<81*a>  | valid False | invalid-plugin-id
--- id=con     | valid True          <-- Windows-reserved names ARE accepted by the validator
--- id=com1    | valid True
--- id=ok.id_1-2 | valid True
```

**Reserved plugin ids: `loop`, `muse-core`, `tbh-reminders`.** They pass `validate` and
even `install`, but are then inert:

```
$ $M plugins list
loop  0.1.0  enabled=true active=true trust=user-local provenance=native-local valid=true diagnostics=1
warning  installed plugin `loop` uses a reserved bundled plugin id; the bundled plugin wins and the installed plugin contributes nothing
```
(diagnostic code `bundled_plugin_id_reserved`; full message adds "the bundled (builtin) plugin wins".)

### 3.3 `capabilities.skills[]`

`{ "id": "<portable-id>", "path": "skills/<x>/SKILL.md", "enabledDefault": bool? }`
Target must be a UTF-8 `SKILL.md` with valid frontmatter.
`enabledDefault` maps to `skills list` activation (`on`/`off`).

### 3.4 `capabilities.commands[]`

`{ "id": "...", "path": "commands/<x>.md", "enabledDefault": bool? }`
Errors: `command entry must declare id and path`,
`command capability id \`X\` duplicates a skill id in the same plugin`,
`plugin command path escapes plugin root`, `plugin_command_template_missing`,
`plugin_command_name_withheld`.

### 3.5 `capabilities.hooks[]`

| field | type | required | notes |
|---|---|---|---|
| `id` | string | yes | |
| `event` | enum | yes | see §3.5.1 |
| `command` | **array of strings** | yes | structured argv. `hook capability \`h\` must declare a command array` if given a string |
| `timeoutMs` | integer | no | |
| `statusMessage` | string | no | surfaced in the hook run terminal |
| `async` | bool | no | default `false` |
| `compatibilityName` | string | no | aliases the hook to a canonical tool matcher name; **must not collide with a built-in tool name** |
| `outputCapabilities` | array | no | must be exactly `["skills.v1"]`, and only on foreground `UserPromptSubmit`/`PostToolUse` command hooks |
| `matcher` | — | **rejected** | Claude/Codex-only field |

Verbatim validator responses:

```
--- matcher on native hook       | unsupported-field       | hook capability field `matcher` is a Claude/Codex hook field; TBH plugin hooks key on `event`; matcher aliasing is tracked separately
--- compatibilityName "Bash"     | invalid-manifest-schema | hook capability `h` compatibilityName `Bash` collides with a built-in tool matcher name
--- compatibilityName "my_custom_tool" | valid True        | "compatibility_name": "my_custom_tool"
--- outputCapabilities ["context"] | invalid-manifest-schema | hook capability `h` outputCapabilities must be exactly ["skills.v1"]
--- outputCapabilities ["skills.v1"] on PreToolUse | invalid-manifest-schema | hook capability `h` skills.v1 is supported only for foreground UserPromptSubmit or PostToolUse command hooks
--- command ["sh","../x.sh"]     | unsafe-path             | plugin capability path must stay inside plugin root
--- command ["sh","/etc/passwd"] | valid True              (absolute argv is NOT treated as a package path)
--- async true                   | valid True
```

Cross-plugin `compatibilityName` ownership is enforced at install:
`compatibilityName \`X\` for plugin \`A\` canonical tool \`T\` collides with plugin \`B\` canonical tool \`T\`; installation rejected`
and `cannot verify compatibilityName ownership for installed plugin \`X\` because its descriptor is unreadable / manifest digest changed; installation rejected`.

Other hook errors: `duplicate-hook-source` (`Hook source paths cannot be shared by two hook IDs`), `missing-capability-command`.

#### 3.5.1 Hook events (`HookEventKind`, 17 values)

`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`,
`PreLLMCall`, `PostLLMCall`, `PreCompact`, `PostCompact`, `SubagentStart`, `SubagentStop`,
`Stop`, `SessionEnd`, `Notification`, `PostToolUseFailure`, `StopFailure`, `PostToolBatch`.

snake_case wire forms also exist (`session_start`, `user_prompt_submit`, …).

`Setup` (a Claude Code event) is *recognised but refused*:
`hook capability \`h\` event \`Setup\` is unsupported` (validator), and for foreign manifests
`Claude hook event \`Setup\` is recognized but is not run by Muse; complete any required plugin setup manually`.
Unknown values → `unsupported-hook-event`.

### 3.6 `capabilities.mcpServers[]`

stdio: `{ "id": "...", "transport": "stdio"?, "command": ["prog","args"...] }` (`transport` defaults to `stdio`)
http:  `{ "id": "...", "transport": "http", "url": "https://..." }` (non-empty `url` required)

Observed: extra keys `env`, `cwd`, `headers` on a **native** entry are **silently dropped**
(no diagnostic, absent from the emitted descriptor). Only `id/transport/command/url/source_path/source_relative_path` survive.

### 3.7 `capabilities.reminders[]`

This is the deepest and most Muse-specific family — a fully declarative, sandboxed
"reminder agent" DSL.

```json
{ "id":"review-policy", "path":"reminders/review-policy.md", "tools":["read_file"],
  "blocking":false,
  "decision":{ "version":1,
    "envelope":{"version":1,"template":"<system-reminder>\n{text}\n</system-reminder>"},
    "deliveryRole":"developer",
    "fields":[{"name":"text","description":"...","requirement":"remind",
               "shape":{"type":"string","minBytes":1,"maxBytes":2000}}],
    "bodyTemplate":{"text":"{text}","slots":[{"name":"text","source":{"field":"text"},
                    "escape":"xml","fallback":""}],"maxBytes":65536},
    "validators":[],
    "proposal":{ "remind":{...}, "none":{...}, "invalidPayload":{...},
                 "roundEnded":{...}, "syntheticFailure":{...}, "validatorRejections":{} },
    "lifecycle":{"seenKey":"ordinary","eotProgressGate":"notApplicable",
                 "failureFallback":"none","installBudget":"roster"},
    "limits":{ ...18 numeric caps... } } }
```

Optional policy fields: `defaultPriority`, `maxPriority`, `maxChildSteps`,
`maxInstallsPerRun`, `reasoningEffort`, `context`.

Serde structs recovered from the binary:
`ReminderDecisionDeclarationV1 (9 elements)`: `version envelope deliveryRole fields bodyTemplate validators proposal lifecycle limits`
`DecisionFieldDeclaration (4)`: `name description requirement shape`
`DecisionFieldRequirement`: `always | remind | none | optional`
`DecisionValueShape`: `string{minBytes,maxBytes,enum} | integer{minimum,maximum} | boolean | array{minItems,maxItems,items} | object`
`DecisionBranchMappings (6)`: `remind none invalidPayload roundEnded syntheticFailure validatorRejections`
`DecisionProposalMapping (10)`: `body kind subject reason requestedPriority effectivePriority visibleForSteps rejection memoryEvidence skillEvidence`
`ReminderDeliveryRole`: `developer | user`
`TemplateSlot (4)`: `name source escape fallback`; `TemplateEscape`: `xml`
value sources (untagged): `LiteralValueSource{literal}`, `FieldValueSource{field}`,
`ValidatorValueSource{validator}`, `EffectiveValueSource{effective}`,
`RenderedBodyValueSource{renderedBody}`, `InputErrorValueSource{inputError}`
`EffectiveDecisionValue`: `defaultPriority | maxPriority | clampedPriority | maxInstallsPerRun | blocking`
`ReminderDecisionLifecyclePolicy (4)`: `seenKey eotProgressGate failureFallback installBudget`
  `DecisionSeenKeyPolicy`: `ordinary | redeliverExpired`
  `DecisionEotProgressGate`: `notApplicable | requireNewToolProgress | decisionWithFiniteBudget`
  `DecisionFailureFallback`: `none | afterNewWork`
  `DecisionInstallBudget`: `roster | finiteRequired`
`DecisionLimits (18)`: `declarationBytes customFields fieldNameBytes descriptionBytes objectDepth objectFields arrayItems submittedJsonBytes scalarStringBytes templateBytes slots renderedBodyBytes memoryAdmissionReads memoryAdmissionLinesPerRead memoryAdmissionExaminedBytesPerRead memoryAdmissionReturnedBytesPerRead memoryAdmissionExaminedBytesPerDecision memoryAdmissionReturnedBytesPerDecision`

Cross-field invariants (verbatim):
```
eotProgressGate=notApplicable requires blocking=false
failureFallback=afterNewWork requires blocking=true
installBudget=finiteRequired requires blocking=true and a finite nonzero maxInstallsPerRun
eotProgressGate=decisionWithFiniteBudget requires installBudget=finiteRequired
remind proposal body must use the renderedBody source
reminder priority must be low, normal, or high
body template contains a host-reserved tag        (<system-reminder>, <continue>)
blocking plugin reminders require a positive maxInstallsPerRun before blocking approval
reminder envelope requires current elevated approval
```

There are four built-in "reminder validator contracts" a declaration can bind
(`ValidatorBinding{id, inputs, outputs, ...}`):
`reminder-validator/memory_body/v1`, `reminder-validator/goal_next_step/v1`,
`reminder-validator/verify_next_step/v1`, `reminder-validator/skill_read_ledger/v1`,
`reminder-validator/memory_source_refs/v1`.

Verified by building the reference reminder plugin verbatim from
`capability-examples.json` and running `plugins validate` → `valid True, diagnostics []`,
descriptor:
```json
[{"id":"review-policy","path":"reminders/review-policy.md",
  "source_path":".../reminders/review-policy.md","tools":["read_file"],"blocking":false,
  "default_priority":null,"max_priority":null,"max_child_steps":null,
  "max_installs_per_run":null,"reasoning_effort":null}]
```

### 3.8 `capabilities.developerPrompts[]` — present but gated OFF

```
$ $M plugins validate ./dp --json
{"error":{"code":"unsupported-capability",
  "message":"plugin developerPrompts capabilities are not supported in this phase",
  ... "declarations":[{"id":"developer-prompt:dp1","kind":"developer-prompt","classification":"unsupported"}],
  "diagnostics":[ ...,
   {"code":"invalid-manifest-schema",
    "message":"developerPrompt capability `dp1` must declare non-empty `text`"}]}}
```
So the entry shape is `{id, text, enabledDefault?}` (JSON pointer `/capabilities/developerPrompts`),
and there is an extra rule
`<x> capability \`id\` text must be bare content — delivery adds the <system-reminder> wrapper`.

### 3.9 Rejected capability keys

```
--- capabilities.tools  | unsupported-capability | plugin tools capabilities are not supported in this phase
--- capabilities.agents | unsupported-capability | plugin agents capabilities are not supported in this phase
```
Same for `outputStyles`, `settings`. `apps` is accepted-but-ignored on foreign manifests
(`plugin apps capabilities are ignored by this runtime in this phase`,
`plugin apps declaration must be a string path`, `plugin apps path escapes plugin root`).

### 3.10 Implicit capability: `agents/` (Agent Definitions) — **native family only**

Not declared in `capabilities`; a top-level `agents/` directory of Markdown files is
parsed by the `agent_definition_core_v1` parser into a signed inventory.

```
$ $M plugins install ./ad --json
  "warning": "third-party plugin: Agent Definitions require review before activation",
  "diagnostics": [{"code":"unsupported-field","severity":"warning",
    "message":"Plugin agent frontmatter key `model` was ignored","path":".../agents/reviewer.md"}]
```

`$DATA_DIR/muse/plugins/cache/local/ad/<pkgsha>/agent-definitions/<inventory-digest>.json`:
```json
{"schema_version":1,"parser_contract":"agent_definition_core_v1","plugin_id":"ad",
 "package_integrity_digest_v1":"sha256:cdf4906d...",
 "entries":[
  {"kind":"invalid_id","ordinal":0,"review_resource_id":"agent-resource:1efde37d...",
   "reason_code":"frontmatter_invalid","field_code":"none","offending_field_count":0},
  {"kind":"valid","ordinal":1,"review_resource_id":"agent-resource:471e2acd...",
   "scoped_definition_id":"ad/reviewer","description":"Reviews code.",
   "prompt":"You review code.","work_tool_selector":{"mode":"named","names":["read_file"]},
   "disallowed_work_tools":[],"source_digest":"sha256:cdf4906d...",
   "definition_hash":"sha256:4953de6c..."}],
 "counts":{"valid":1,"named_invalid":0,"invalid_id":1}}
```

It becomes an approvable runtime capability:
```
$ $M plugins approve ad --json
{"decision":"approve","runtime_capabilities":[
  {"kind":"agent_definition","plugin_id":"ad","scoped_definition_id":"ad/reviewer",
   "original_ordinal":1,"enabled":true}]}
```
persisted in settings under key
`plugin:ad:agent_definition:agent-resource:471e2acda4c1...`.

**Claude and Codex plugins get `status:"empty"` inventories even when they ship `agents/`**
(verified: `cc-plugin` has `agents/reviewer.md` and its inventory is `{"valid":0,...}` —
its agent is reported as `agent:reviewer / unsupported` instead).

Agent-definition frontmatter field vocabulary recovered from the binary:
`name description prompt tools disallowed_tools model effort permission_mode max_turns
background isolation skills mcp_servers hooks memory color initial_prompt main_session_selection`
(with `model` observed being ignored by the plugin path).
Related failure codes: `agent_name_invalid`, `named_invalid`, `invalid_id`,
`Agent Definition inventory derivation failed closed`,
`Agent Definition inventory package integrity digest mismatch`.

---

## 4. Claude Code plugin compatibility (`.claude-plugin/plugin.json`)

### 4.1 Test package used

`.claude-plugin/plugin.json`:
```json
{
  "name": "cc-plugin",
  "description": "A Claude Code style plugin.",
  "version": "2.3.4",
  "author": { "name": "Someone", "email": "a@b.c", "url": "https://example.com" },
  "homepage": "https://example.com/home",
  "repository": "https://github.com/example/cc-plugin",
  "license": "MIT",
  "keywords": ["lint", "review"],
  "commands": ["./commands/hello.md"],
  "agents": ["./agents/reviewer.md"],
  "skills": ["./skills/lint"],
  "hooks": "./hooks/hooks.json",
  "mcpServers": { "example": { "command": "node", "args": ["scripts/mcp.js"] } },
  "outputStyles": ["./styles/x.md"],
  "someUnknownField": 42
}
```
`hooks/hooks.json` is Claude Code's exact shape (`{"hooks":{"<Event>":[{"matcher":..,"hooks":[{"type":"command",...}]}]}}`).

### 4.2 Result

```
$ $M plugins validate ./cc-plugin --json
"valid": true, "manifest_family": "claude-compatible", "display_name": null
compatibility.summary: "partial"
```

Translated → live capabilities:

| Claude field | outcome |
|---|---|
| `skills: ["./skills/lint"]` | → skill `lint`, `path: skills/lint/SKILL.md`, `enabled_default: null` |
| `commands: ["./commands/hello.md"]` | → command `hello`, `enabled_default: true` |
| `hooks: "./hooks/hooks.json"` | → 3 hooks, ids `hook-<16 hex>` derived from the entry content, keeping `event`, `matcher`, `timeout`→`timeout_ms` (×1000), `statusMessage`, `async`; `command` becomes both `command:[str]` and `shell_command:str` |
| `mcpServers: {...}` | → mcp `example`, `transport:"stdio"`, `command:["node","scripts/mcp.js"]` (i.e. `command`+`args` are flattened) |

Dropped, each with a diagnostic:

| code | message |
|---|---|
| `unsupported-field` | ``Claude manifest field `author` is presentation-only and is not imported`` |
| `unsupported-field` | ``Claude manifest field `homepage` is presentation-only and is not imported`` |
| `unsupported-field` | ``Claude manifest field `repository` is presentation-only and is not imported`` |
| `unsupported-field` | ``Claude manifest field `license` is presentation-only and is not imported`` |
| `unsupported-field` | ``Claude manifest field `keywords` is presentation-only and is not imported`` |
| `unsupported-capability` | ``Claude manifest field `agents` declares unsupported behavior`` |
| `unsupported-capability` | ``Claude manifest field `outputStyles` declares unsupported behavior`` |
| `unsupported-field` | ``Claude manifest field `someUnknownField` is not modelled and is ignored`` |
| `unsupported-field` | ``foreign hook handler field `silent` is recognized; Muse ignores Claude display suppression`` |
| `unsupported-capability` | ``Claude command declares `allowed-tools`, which is not enforced yet; the command runs under ordinary approval and no tool permission is granted`` |

`compatibility.declarations` records every declaration with a `classification`:
```json
[{"id":"skill:lint","kind":"skill","classification":"supported"},
 {"id":"hook:hook-af6225756e4873d1","kind":"hook","classification":"supported"},
 {"id":"hook:hook-d28c888164c4a14f","kind":"hook","classification":"supported"},
 {"id":"hook:hook-a1b4be5746808228","kind":"hook","classification":"supported"},
 {"id":"mcp:example","kind":"mcp","classification":"supported"},
 {"id":"command:hello","kind":"command","classification":"supported"},
 {"id":"invocation-preapproval:command:hello","kind":"invocation-preapproval","classification":"unsupported"},
 {"id":"agent:reviewer","kind":"agent","classification":"unsupported"},
 {"id":"output-style:x","kind":"output-style","classification":"unsupported"}]
```
`compatibility.summary` ∈ `full | partial | unsupported`
(`plugin compatibility is unsupported: no supported behavior capabilities`).

### 4.3 `${CLAUDE_PLUGIN_ROOT}` and hook execution

Claude hooks are **shell** commands (the whole string is handed to the user shell) and
`${CLAUDE_PLUGIN_ROOT}` is expanded to the cached package root at run time:

```
$ $M plugins hook test cc-plugin:hook-af6225756e4873d1 --fixture ./fixture.json --json
"stderr": "zsh:1: permission denied: <DATA>/muse/plugins/cache/local/cc-plugin/<sha>/package/scripts/check.sh"
"exit_code": 126
```
(the script was not chmod +x — which itself proves both the substitution and that the
command went through `zsh`, unlike native `command:[argv]` hooks which are exec'd directly).

Internally the placeholder set is `${CLAUDE_PLUGIN_ROOT}`, `CLAUDE_PLUGIN_ROOT`,
`PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, `PLUGIN_DATA`; misuse yields
`unpermitted-key` / `unpermitted-placeholder`.

### 4.4 `allowed-tools` handling (commands and skills)

* command frontmatter `allowed-tools` → parsed, canonicalised, recorded, **not enforced**:
  ``Claude command declares `allowed-tools`, which is not enforced yet; the command runs under
  ordinary approval and no tool permission is granted``, declaration kind `invocation-preapproval`.
* Selector grammar is still validated:
  ``` `allowed-tools` selector syntax is malformed / has unbalanced parentheses /
  Bash rule has a misplaced `:*` / Bash rule must not be empty / Bash prefix rule must not be empty /
  must declare at least one selector / entries must not be empty /
  contains duplicate canonical selectors / list entries must be strings /
  must be a string or a list of strings /
  `allowed-tools` may use `${CLAUDE_PLUGIN_ROOT}` only in Bash rules ```

### 4.5 `disable-model-invocation` — honoured

```
--- disable-model-invocation: true
   honored-declaration | Claude command frontmatter field `disable-model-invocation` is honoured conservatively; an installed command is user-invocable only
--- disable-model-invocation: false
   honored-declaration | Claude command frontmatter field `disable-model-invocation` is honoured as permissive; an installed command keeps model invocation allowed
```

### 4.6 Other Claude parser messages (from strings)

```
Claude manifest field `skills` must be an array
Claude manifest `skills` array must contain only string paths
Claude skill source must be a SKILL.md file or directory containing one
Claude skill source `X` contains no SKILL.md files
Claude skill sources must contain only regular files and directories
Claude SKILL.md must be valid UTF-8
Claude manifest field `commands` must be a path array; a bare string is unsupported
Claude manifest `commands` must be a path or path array
Claude manifest `commands` array must contain only string paths
Claude command source `X` contains no Markdown commands
Claude command source must be a Markdown file
Claude command path must have a valid UTF-8 command slug
Claude command frontmatter must be a mapping / keys must be strings / is invalid YAML
Claude command frontmatter field `name` must be a string / must be a valid command id
Claude command frontmatter field `argument-hint` must be a string
Claude command frontmatter field `argument-hint` has an unsupported complex or multiline value and is ignored
Claude command frontmatter field `X` is restrictive but unsupported
Claude command frontmatter field `X` is not modelled and is ignored
Claude skill frontmatter field `X` is not used by this runtime
Claude commands path is not a frozen directory
duplicate foreign command capability id `X`
```
Foreign hook-handler parser (shared by Claude and Codex; known keys
`type description command timeout statusMessage async outputCapabilities if silent`):
```
foreign hook handler must declare string field `type`
foreign hook handler type `X` is unsupported
foreign hook handler must be an object
foreign hook handler field `description` must be a string
foreign hook command must be a string / must not be empty
foreign hook timeout must be a non-negative integer / is too large
foreign hook statusMessage must be a string
foreign hook if must be a string
foreign hook async must be a boolean
foreign hook group must contain a `hooks` array
foreign hook group must be an object / field `description` must be a string
foreign hook matcher must be a string
plugin hooks must be an object
plugin hook source must contain an object field `hooks`
plugin hook source escapes plugin root
```
The Claude manifest's own known-field vocabulary (concatenated serde literal in the binary):
`plugin.json | agents | lspServers | outputStyles | skills | userConfig | Claude |
displayName | description | hooks | mcpServers | author | repository | license`
— i.e. `lspServers` and `userConfig` are modelled-and-dropped too.

Claude MCP entries are rejected for `non-stdio-transport` and `non-empty-env`:
```
"Claude MCP server `example` rejected: non-empty-env"      (when env:{FOO:"bar"} present)
```

---

## 5. Codex plugin compatibility (`.codex-plugin/plugin.json`)

### 5.1 Test package

```json
{ "name":"cx-plugin","version":"0.2.0","description":"A Codex style plugin.",
  "interface":{"displayName":"CX Plugin"},
  "author":{"name":"x"},"repository":"https://example.com/r","license":"MIT",
  "skills":"skills",
  "commands":["commands/go.md"],
  "mcpServers":".mcp.json",
  "apps":"apps.json",
  "dependencies":{"requiredPlugins":["other"]},
  "weirdField":true }
```
`.mcp.json` = `{"mcpServers":{"cx":{"type":"stdio","command":"node","args":["x.js"]}}}`

### 5.2 Result

```
"valid": true, "manifest_family": "codex-compatible", "display_name": "CX Plugin"
compatibility.summary: "partial"
capabilities: skills=[audit], mcp_servers=[cx], hooks=[], commands=[], reminders=[]
declarations: skill:audit supported | mcp:cx supported | app:apps unsupported
diagnostics:
  unsupported-field       | plugin manifest field `commands` is not used by this runtime
  unsupported-capability  | plugin apps capabilities are ignored by this runtime in this phase
  unsupported-field       | plugin manifest field `dependencies` is not used by this runtime
  unsupported-field       | plugin manifest field `weirdField` is not used by this runtime
```

Key differences vs Claude:

* `interface.displayName` **is** honoured (JSON pointer `/interface/displayName`); `author`,
  `repository`, `license` are *known* (no diagnostic) but not surfaced.
* `skills` is a **single string path to a directory** of skill directories
  (`public Codex manifest \`skills\` must be a string path`,
  `public Codex skills directory must contain at least one SKILL.md`,
  `public Codex skill path must have a UTF-8 parent directory`,
  `public Codex skills path is not a frozen directory`).
* `mcpServers` may be a path to a conventional `.mcp.json`
  (`foreign manifest \`mcpServers\` must be an object, path, or path list`,
  `foreign manifest \`mcpServers\` source list must contain only strings`,
  `foreign conventional MCP source`). The Codex `.mcp.json` needs **no** `$schema`
  (unlike the Agent-Plugins `.mcp.json`).
* `commands` and `hooks` are *not* imported: the binary carries the explicit note
  `this Codex plugin declares no importable skills yet (hooks/MCP/commands are not imported in this phase)`
  (emitted when a Codex manifest yields nothing at all).
* Codex `apps` is a string path to a **file** (`plugin path must be a file` when a dir is given).
* If `name` is absent: `Codex plugin manifest has no \`name\` and no directory name could be
  derived from the source path`; default version literal is `0.0.0`.
* `Codex skill frontmatter field \`X\` is not used by this runtime`.

---

## 6. "Agent Plugins 1.0.0" family (third foreign format)

A root-level `plugin.json` bearing the exact Agent Plugins 1.0.0 `$schema` selects a
third parser (`manifest_family` enum value `agent-plugins`). Recovered rules:

```
Agent plugin manifest root must be a JSON object
Agent plugin manifest must declare string field `name`
Agent plugin `name` <v> does not match the Agent Plugins 1.0.0 name rule
Agent plugin manifest field `keywords` must be an array of strings
Agent plugin manifest field `author` must be an object
Agent plugin manifest `author` member `X` is not part of the Agent Plugins 1.0.0 schema
Agent plugin manifest `author.X` must be a string
Agent plugin manifest field `extensions` is not an object; reported and ignored
Agent plugin manifest extension `X` must be an object
Agent plugin manifest field `X` is not part of the Agent Plugins 1.0.0 core; reported and ignored
```
Known members near the parser: `name`, `author{name,url,...}`, `repository`, `license`,
`keywords`, `extensions`, `commands`, `reminders`, `skills`, `mcp`.

Components:
```
Agent `skills/` is not a contained package directory; the skills component is disabled
Agent skill `X` is skipped: <reason>; sibling skills continue
   reasons: its directory name is not a valid skill id / it is not a skill directory /
            it has no contained regular SKILL.md / its SKILL.md is not valid UTF-8 /
            its SKILL.md frontmatter is invalid / its SKILL.md is unreadable or oversized /
            its path fails the text-safety rules / its SKILL.md fails the text-safety rules
Agent skill frontmatter key `X` is not recognized; ignored
Agent `mcp.json` <reason>; the MCP component is disabled
   reasons: has no `mcpServers` object / has an invalid top-level shape /
            does not declare the exact Agent Plugins 1.0.0 MCP `$schema` /
            is not a JSON object / fails the text-safety rules / is not valid JSON /
            is not valid UTF-8 / is unreadable or oversized / is not a contained regular file
Agent MCP server `X` is skipped: <reason>; sibling entries continue
   reasons: the entry `type` is not a known transport / the entry `env` values must be strings /
            the entry `headers` must be an object of strings / the entry `cwd` is not plugin-contained /
            the entry `env` declares a reserved variable / the entry `env` must be an object /
            the entry `args` must be an array of strings / the entry is missing its required
            non-empty string field / the entry carries a field outside the bundled schema /
            the entry has no string `type` / the entry is not an object
Agent Plugins MCP server <X> is not supported in this release; it stays visible and inactive
```
Diagnostic codes reserved for it: `unsupported-agent-schema`, `agent-component-invalid`,
`agent-skill-skipped`, `agent-mcp-server-skipped`, `agent-overlay-inactive`.

**The literal `$schema` value could not be recovered** (see §14).

---

## 7. Diagnostics vocabulary (complete, from the binary's code table)

```
invalid-plugin-package   bundled_plugin_id_reserved  missing-manifest      multiple-manifests
invalid-manifest-json    invalid-manifest-schema     manifest-family-mismatch
invalid-plugin-id        invalid-version             invalid-capability-id
missing-capability-path  missing-capability-command  unsupported-hook-event
duplicate-capability-id  duplicate-hook-source       unsafe-path
unsupported-capability   unsupported-field           honored-declaration
unsupported-agent-schema ignored-root-manifest       agent-component-invalid
agent-skill-skipped      agent-mcp-server-skipped    agent-overlay-inactive
```
Severities observed: `warning`, `error`. Each diagnostic carries `{code, severity, message, path}`
(plus `kind`/`id` in the `install` text/JSON renderer).

Store/CLI error codes: `ready store_failed invalid_package incompatible_package unknown_plugin
unknown_marketplace source_unavailable store_read store_write store_format cache_invalid`.

Capability-kind vocabulary (used by `compatibility.declarations[].kind`):
`tool skill setting reminder output-style mcp lsp hook developer-prompt dependency
command app agent invocation-preapproval user-config`.

Runtime capability-snapshot diagnostic codes (crate `plugins::capability_snapshot`):
```
nearest_trusted_project higher_precedence nearer_project duplicate named_invalid safe_mode
untrusted_project disabled_plugin not_trusted_enabled plugin_scope_quota
plugin_preflight_overflow plugin_class_overflow inventory_refresh_required
invalid_utf8 bom_forbidden frontmatter_invalid duplicate_key invalid_name missing_field
invalid_field unknown_field field_too_large candidate_too_large missing_source wrong_kind
symlink_or_reparse path_escape non_utf8_path cycle inventory_changed unstable_read
source_bound snapshot_bound inventory_integrity join_mismatch package_digest_mismatch
not_found lookup_expectation_mismatch unknown_tool tool_classification epoch_mismatch
grant_bound forged_dispatch known_field_inactive restricted_source_field
plugin_command_template_missing plugin_command_name_withheld plugin_reminder_duty_missing
plugin_reminder_duty_too_large agent_definition_review_unavailable bundled_plugin_invalid
bundled_plugin_id_reserved foreign-plugin-artifact-observed plugin-cache-invalid
```
Its structured telemetry event fields (`tbh.local.catalog`):
`installed_admitted bundled_plugins installed_plugins marketplace_plugins plugin_ids
reminders developer_prompts conflicts omitted foreign_artifacts duration_ms`.

---

## 8. On-disk layout

```
$CONFIG_DIR = $XDG_CONFIG_HOME/muse   (macOS default ~/.config/muse)
$DATA_DIR   = $XDG_DATA_HOME/muse     (macOS default ~/.local/share/muse)

$CONFIG_DIR/settings.json                        <- plugin skill activation + capability trust
$CONFIG_DIR/trust.json                           <- workspace (project) trust store
$CONFIG_DIR/skills/                              <- managed personal skills (import target)
$CONFIG_DIR/skills/.muse/lock.json               <- skills lockfile
$CONFIG_DIR/skills/.muse/audit.log               <- JSONL audit
$CONFIG_DIR/skills/.muse/.skills.lock            <- advisory lock
$CONFIG_DIR/skills/.muse/quarantine/             <- corrupt-package quarantine
$CONFIG_DIR/skills/.muse/import-quarantine/<id>/ <- foreign-import quarantine

$DATA_DIR/plugins/installed.json                 <- plugin lockfile  (a.k.a. "installed pointer")
$DATA_DIR/plugins/.installed.lock
$DATA_DIR/plugins/.tmp/
$DATA_DIR/plugins/marketplaces.json              <- marketplace lockfile
$DATA_DIR/plugins/marketplaces/<name>/snapshot.json
$DATA_DIR/plugins/marketplaces/<name>/source/            (git worktree)
$DATA_DIR/plugins/marketplaces/<name>/source/.muse-claude-sources/<i>/   (cloned remote plugin)
$DATA_DIR/plugins/marketplaces/<name>/source/.muse-codex-sources/<i>/
$DATA_DIR/plugins/marketplaces/<name>/generations/
$DATA_DIR/plugins/cache/local/<plugin-id>/<package-sha256>/package/
$DATA_DIR/plugins/cache/local/<plugin-id>/<package-sha256>/agent-definitions/<digest>.json
$DATA_DIR/plugins/cache/builtin/muse-core/<skill-id>/<sha256>/...
$DATA_DIR/plugins/data/<plugin-id>/                       (per-plugin writable data dir)
```
Verified paths from `plugins install` output and `find`. String evidence for the ones not
exercised: `plugins/marketplaces/<n>/source`, `/snapshot.json`, `/generations/<g>`, `.tmp-<x>`,
`plugins/data`, `plugins/installed.json`, `cache/local`, `.installed.lock`.

### 8.1 `installed.json` (plugin lockfile) — verbatim

```json
{
  "version": 1,
  "plugins": {
    "demo-plugin": {
      "id": "demo-plugin",
      "display_name": "Demo Plugin",
      "version": "0.1.0",
      "description": "Reverse engineering probe plugin.",
      "manifest_family": "muse",
      "enabled": true,
      "trust": "user-local",
      "source": {
        "provenance": "native-local",
        "path": "/.../ws/demo-plugin"
      },
      "installed_at": "2026-09-01T14:27:22.64582Z",
      "updated_at": null,
      "manifest_sha256": "sha256:c87be30f...",
      "package_sha256": "sha256:8a510b46...",
      "cache_path": "plugins/cache/local/demo-plugin/8a510b46.../package",
      "agent_definition_inventory": {"schema_version":1,"parser_contract":"agent_definition_core_v1",
        "status":"empty","package_sha256":"sha256:8a510b46...",
        "package_integrity_digest_v1":"sha256:10f4d3b0...","inventory_digest":"sha256:7c9a34de...",
        "serialized_size_bytes":262,"counts":{"valid":0,"named_invalid":0,"invalid_id":0}}
    }
  }
}
```

Serde shapes from the binary:
* `Lockfile` (2): `version`, `plugins`
* `LockPluginRecord` (15): `id display_name description version manifest_family enabled trust
  source installed_at updated_at manifest_sha256 package_sha256 cache_path
  source_format_version agent_definition_inventory`
* `LockSourceRecord` (3): `provenance`, `path` (+ one more, `sha`/`ref` for remote sources)
* `manifest_family` on the wire: `muse | claude | codex | agent`
  (the CLI/JSON renders `native | claude-compatible | codex-compatible | agent-plugins`)
* `trust`: `user-local | project-trusted`
* `provenance`: `native-local | foreign-import | marketplace-user-added | curated | local | marketplace`
* `agent_definition_inventory.status`: `ready | empty | omitted` (`MetadataWire` 9 fields:
  `schema_version parser_contract status omission package_sha256 package_integrity_digest_v1
  inventory_digest serialized_size_bytes counts`; `OmissionWire` 6: `reason_code field_code
  skills hooks offending_field_count bound_code observed limit`)
* Version guard: `unsupported lockfile version`, `installed pointer must be a regular non-symlink
  file`, `installed pointer exceeds <N> byte limit before parsing`.

### 8.2 `marketplaces.json` — verbatim

```json
{
  "version": 1,
  "marketplaces": {
    "g1": {
      "name": "g1",
      "source": {
        "kind": "git",
        "path": "file:///.../gitmkt",
        "worktree_path": "plugins/marketplaces/g1/source"
      },
      "snapshot_path": "plugins/marketplaces/g1/snapshot.json",
      "last_updated_at": "2026-09-01T14:35:54.683817Z"
    },
    "local1": {
      "name": "local1",
      "source": { "kind": "local-dir", "path": "/.../ws/mkt" },
      "snapshot_path": "plugins/marketplaces/local1/snapshot.json",
      "last_updated_at": "2026-09-01T14:35:20.085767Z"
    }
  }
}
```
`MarketplaceLockfile` (2): `version marketplaces`;
`LockMarketplaceRecord` (4): `name source snapshot_path last_updated_at`;
`LockMarketplaceSource` (3): `kind path worktree_path`.
`kind` ∈ `local-dir | local-file | git` (`local-path` is the *install transport*).
Guard: `unsupported marketplace lockfile version`.

### 8.3 `snapshot.json` — verbatim

```json
{
  "schemaVersion": 1,
  "source": "local",
  "plugins": [
    { "name": "demo-plugin", "version": "0.1.0",
      "install": { "transport": "local-path",
                   "source": "/.../marketplaces/g1/source/.muse-claude-sources/0/demo-plugin" },
      "integrity": { "digest": "sha256:8a510b46..." },
      "availability": { "status": "available" } }
  ],
  "diagnostics": [
    { "severity":"warning","name":"demo-plugin","message":"Claude marketplace source `git` is not supported" }
  ]
}
```
`MarketplacePlugin` (5): `name version install integrity availability`.
JSON pointers used in errors: `/install/transport`, `/install/source`, `/integrity/digest`,
`/availability/status`, `/policy/installation`.
`availability.status` ∈ `available | blocked | deprecated` (`AVAILABLE` literal also present).
Digest rules: `marketplace digest must start with sha256:`, `marketplace digest must be a
sha256 hex digest`. `marketplace must declare schemaVersion 1`, `marketplace root must be a
JSON object`, `marketplace plugins must be an array`.

---

## 9. Trust, review and audit

### 9.1 Three layers

1. **Workspace trust** — `$CONFIG_DIR/trust.json`.
   Format recovered by probing (`ProjectTrustStore` 2 fields, `ProjectTrustEntry` 1 field):
   ```json
   {"schema_version":1,"projects":{"/abs/path":{"decision":"trusted"}}}
   ```
   (`{"trusted":true}` → `missing field \`decision\``.)
   Without it, `plugins install --scope project` fails:
   `project-local plugin source is blocked because the workspace is untrusted`.

2. **Plugin enable/disable** — `enabled` in `installed.json`, toggled by
   `muse plugins enable|disable <id>`. `enabled=false` → `active=false`.

3. **Per-capability trust** — `$CONFIG_DIR/settings.json` → `runtime_capabilities`.

### 9.2 What needs review

Install-time warnings (exact strings), which show the split precisely:

| package contents | warning |
|---|---|
| hooks + mcp | `third-party plugin: hooks and MCP servers require review before activation; skills and commands are active without review while the plugin is enabled` |
| mcp only | `third-party plugin: MCP servers require review before activation; skills are active without review while the plugin is enabled` |
| hooks only | `third-party plugin: hooks require review before activation` |
| agents/ only | `third-party plugin: Agent Definitions require review before activation` |

So: **skills and commands are live on install; hooks, MCP servers and Agent Definitions are not.**

### 9.3 `inspect` / `approve` / `reject`

```
$ $M plugins inspect demo-plugin
demo-plugin  0.1.0  enabled=true active=true trust=user-local valid=true skills=1 commands=1 hooks=1 mcp=1 reminders=0 cache=...
warning  third-party plugin: hooks and MCP servers require review before activation; ...
runtime-capability  plugin:demo-plugin:hook:pre-check              status=review_needed
runtime-capability  plugin:demo-plugin:mcp_server:workspace-index  status=review_needed
```
JSON adds:
```json
"effective_capabilities_scope": "plugin-capability-snapshot",
"effective_capabilities": [{"kind":"command","stable_id":"plugin:demo-plugin:summarize"},
                           {"kind":"skill","stable_id":"plugin:demo-plugin:review"}],
"capability_diagnostics": [],
"active_scope": "installed-plugin",
"runtime_capabilities": [
  {"candidate":{"kind":"hook","plugin_id":"demo-plugin","capability_id":"pre-check",
     "stable_id":"plugin:demo-plugin:hook:pre-check",
     "display_path":"plugin://demo-plugin/hook/pre-check",
     "definition_hash":"sha256:c2fb65bf...","source_digest":"sha256:8a510b46..."},
   "status":"review_needed","diagnostic":null}, ...]
```

Selector forms accepted by `approve`/`reject`:
* `<plugin-id>` (all reviewable capabilities of that plugin)
* `<plugin-id>:<kind>:<capability-id>` e.g. `demo-plugin:hook:pre-check`
* full stable id `plugin:demo-plugin:mcp_server:workspace-index`
Errors: `runtime capability selector \`X\` is ambiguous; qualify by kind, e.g. \`...\``,
`no runtime capabilities match \`X\``, `<N> non-reviewable runtime capabilities: ...`,
`unknown runtime capability kind \`X\`; expected one of \`...\``,
`runtime capability trust load/save failed`, `runtime capabilities changed or disappeared
before trust could be persisted`.

`RuntimeCapabilityTrustStatus`: `review_needed | trusted_enabled | trusted_disabled |
modified | invalid | blocked`.

Persisted result (`$CONFIG_DIR/settings.json`):
```json
{
  "schema_version": 1,
  "skills": { "activation": { "plugin": {
      "plugin://cc-plugin/skills/lint/SKILL.md": "off",
      "plugin://cx-plugin/skills/audit/SKILL.md": "user-invocable-only",
      "plugin://demo-plugin/skills/review/SKILL.md": "on" } } },
  "runtime_capabilities": {
    "plugin:demo-plugin:hook:pre-check": {
      "enabled": true,  "trusted_definition_hash": "sha256:c2fb65bf..." },
    "plugin:demo-plugin:mcp_server:workspace-index": {
      "enabled": false, "trusted_definition_hash": "sha256:88f6c2d8..." },
    "plugin:ad:agent_definition:agent-resource:471e2acd...": {
      "enabled": true,  "trusted_definition_hash": "sha256:4953de6c..." }
  }
}
```
(`RuntimeCapabilityState` also has a third member `trusted_blocking_definition_hash`, used
for blocking reminders.)

### 9.4 Trust is bound to the package bytes — verified

```
A(approved):                [hook-8298 trusted_enabled dh=95bda828 sd=6a4586de,
                             hook-a1b4 trusted_enabled dh=0077ca37 sd=6a4586de,
                             hook-d28c trusted_enabled dh=07641a9c sd=6a4586de,
                             mcp:example trusted_enabled dh=087fcbe7 sd=68149d90]
B(edited scripts/post.sh):  ALL FOUR -> "modified"   (dh and sd all changed)
C(after re-approve, edited one hook entry in hooks.json):
                            hook-5677 review_needed (new content-derived id),
                            hook-a1b4 trusted_enabled, hook-d28c trusted_enabled,
                            mcp:example modified
```
and for a native plugin, merely **adding an unrelated file** flips everything:
```
before: hook:pre-check trusted_enabled dh=cc4c68cc sd=ead36a43
        mcp_server:workspace-index trusted_enabled dh=1e0c21f6 sd=ead36a43
after `echo unrelated > README.md; plugins update hp`:
        both -> "modified", dh and sd changed
diagnostic: {"code":"modified_definition_hash",
  "message":"runtime capability `plugin:hp:mcp_server:workspace-index` is inactive: definition hash changed and needs review"}
```
INFERRED model: `definition_hash` = H(capability declaration ‖ `source_digest`), where
`source_digest` covers the plugin package content *other than* that capability's own
declaration file (this explains why editing `hooks/hooks.json` left sibling hooks' digests
untouched in C but changed the MCP server's, while editing `scripts/post.sh` or adding a
root file changed everything).

Related messages: `runtime capability is inactive: stable id must not be empty`,
`stable id is duplicated in the review snapshot`, `definition hash must not be empty`,
`definition hash changed and needs review`, `plugin cache is invalid; capabilities are blocked`.

### 9.5 `plugins hook test`

```
$ cat fixture.json
{ "event": "PreToolUse", "stdin": {"tool_name":"Bash","tool_input":{"command":"ls"}},
  "matcher_input": "Bash", "cwd": "." }

$ $M plugins hook test demo-plugin:pre-check --fixture ./fixture.json --json
{ "decision": { "should_block": false, "block_reason": null, "additional_contexts": [],
                "updated_input": null, "permission_decision": null, "feedback_message": null },
  "terminals": [ { "run_id": "plugin:demo-plugin:pre-check:1",
                   "hook_key": "plugin:demo-plugin:pre-check", "event": "pre_tool_use",
                   "status_message": "Checking plugin policy", "system_message": null,
                   "status": "completed", "duration_ms": 5, "exit_code": 0, "effects": [],
                   "stdout": "ok\n", "stderr": "", "error": null } ],
  "records": 2 }
```
Fixture schema: `{event, stdin, matcher_input, cwd}` (`fixture must contain \`stdin\``,
`fixture event must be \`PostToolUse\` or \`post_tool_use\``).
Hook must be installed **and enabled/approved**: `plugin hook \`X\` is not installed and enabled`.
`HookRunStatus`: `completed | blocked | failed | timed_out | cancelled | stopped`.
`HookEffectCategory`: `context | rewrite_selected | rewrite_ignored | permission_allowed |
permission_denied | feedback`.

### 9.6 Hook process environment (measured)

A native hook (`command: ["sh","hooks/probe.sh"]`) sees:

```
PWD=<workspace cwd>
CLAUDE_PLUGIN_DATA=<DATA>/muse/plugins/data/envprobe
CLAUDE_PLUGIN_ROOT=<DATA>/muse/plugins/cache/local/envprobe/<pkgsha>/package
MUSE_PLUGIN_DATA_DIR=<DATA>/muse/plugins/data/envprobe
MUSE_PLUGIN_ID=envprobe
MUSE_PLUGIN_ROOT=<DATA>/muse/plugins/cache/local/envprobe/<pkgsha>/package
PLUGIN_DATA=<DATA>/muse/plugins/data/envprobe
PLUGIN_ROOT=<DATA>/muse/plugins/cache/local/envprobe/<pkgsha>/package
+ inherited HOME LANG LOGNAME PATH SHELL SHLVL TERM TMPDIR USER
STDIN: {"tool_name":"Bash","tool_input":{"command":"ls"}}
```
i.e. the hook JSON payload arrives on **stdin**, the cwd is the workspace, and the plugin
runs out of the **content-addressed cache**, never the source tree.
Additional related env: `TBH_MANAGED_HOOKS_PATH`, settings `managed_hooks_path`,
`managed_hooks_env_vars`.

---

## 10. Marketplaces / registry

### 10.1 Sources

`muse plugins marketplace add <name> <source>`; `<source>` may be
`owner/repo`, a git URL, a `file://` URL, or a local path (TUI hint), producing
`kind: local-dir | local-file | git`.

Verified:
```
$ $M plugins marketplace add local1 ./mkt --json
{"marketplace":{"name":"local1","source":{"kind":"local-dir","path":"/.../mkt"},
 "plugin_count":1,"skipped":[],"snapshot_path":".../marketplaces/local1/snapshot.json",
 "last_updated_at":"2026-09-01T14:35:20.085767Z"}}

$ $M plugins marketplace add g1 "file:///.../gitmkt" --json
{"marketplace":{"name":"g1","source":{"kind":"git","path":"file:///.../gitmkt"}, ... }}
    -> shallow clone into $DATA_DIR/plugins/marketplaces/g1/source (git --quiet --single-branch,
       `timeout --kill-after=5s`, `git -C`, `git checkout failed`)

$ $M plugins marketplace add g2 "https://example.invalid/x.git" --json
{"error":{"code":"invalid-plugin-package","message":"failed to clone marketplace source
  `https://example.invalid/x.git`: fatal: unable to access ... SSL_ERROR_SYSCALL ..."}}
```
`tbh-curated` is a reserved marketplace name:
```
$ $M plugins marketplace add tbh-curated ./mkt --json
{"error":{"code":"invalid-plugin-package","message":"marketplace `tbh-curated` is reserved and cannot be added"}}
```
(also `marketplace \`X\` is reserved and cannot be removed`, `marketplace name \`X\` is invalid`,
`marketplace \`X\` is already configured`). Provenance `curated` exists in the enum, so a
first-party curated registry is wired but not populated in this build.

### 10.2 Catalog file discovery

Two catalog filenames are probed inside a marketplace root:
* `.agents/plugins/marketplace.json` → parsed by the **Codex** marketplace parser
* `.claude-plugin/marketplace.json` → parsed by the **Claude** marketplace parser
(the binary also contains `https://json.schemastore.org/claude-code-marketplace.json`,
i.e. Claude's published marketplace JSON-Schema id.)

### 10.3 Codex marketplace (`.agents/plugins/marketplace.json`)

Probed shape:
```json
{ "schemaVersion": 1,
  "plugins": [
    { "name": "demo-plugin", "version": "0.1.0", "description": "d",
      "source": { "source": "local", "path": "demo-plugin" } }
  ] }
```
Errors observed while probing:
```
Codex marketplace plugins must be an array
Codex marketplace `<file>` must declare `name`
Codex marketplace plugin `p1` must declare source
Codex marketplace `<file>` must declare `source`        (when source obj lacks the inner `source` key)
Codex marketplace source `local-path|path|dir|file|git|relative|bundled` is not supported
Codex marketplace local source `../demo-plugin` is invalid: plugin capability path must stay inside plugin root
Codex marketplace url sources require a Git marketplace source
Codex marketplace `X` installation policy `Y` is unsupported
Codex marketplace plugin name `X` is invalid
```
So the inner discriminator is `source.source ∈ {local, url}`; `local` takes `path`
(must stay inside the marketplace root), `url` takes `url` (+ `ref`) and requires the
marketplace itself to be a Git source.

### 10.4 Claude marketplace (`.claude-plugin/marketplace.json`)

Claude Code's own file works unmodified:
```json
{ "name":"cc1","owner":{"name":"me"},
  "plugins":[{"name":"cc-plugin","source":"./cc-plugin","description":"d"}] }
```
→ `plugin_count: 1, skipped: []`.

Remote entries:
```
{"source":{"source":"url","url":"file:///.../gitsrc","ref":"main"}}   -> accepted (git marketplace only)
{"source":{"source":"github","repo":"a/b"}}                            -> accepted shape ("must declare `repo`" if missing)
{"source":{"source":"git", ...}}                                       -> "Claude marketplace source `git` is not supported"
{"source":{"source":"git+https|remote|http|gitlab"}}                   -> "... is not supported"
any remote source under a local-dir marketplace                        -> "Claude marketplace remote sources require a Git marketplace source"
```
Other messages: `Claude marketplace plugin must declare \`source\``,
`Claude marketplace entry \`X\` must be a non-empty string`,
`Claude marketplace entry must declare \`X\``,
`Claude marketplace source must be a path or object`,
`Claude marketplace remote source \`X\` is not a supported Git source`.

### 10.5 `.muse-claude-sources` / `.muse-codex-sources` — answered

They are directories Muse creates **inside the Git marketplace worktree** to clone each
remote per-plugin source into, indexed by the entry's ordinal:

```
$ cat .../marketplaces/g1/snapshot.json
"install": { "transport": "local-path",
             "source": ".../marketplaces/g1/source/.muse-claude-sources/0/demo-plugin" }

$ find .../marketplaces/g1/source -maxdepth 2 -name ".muse-*"
.../marketplaces/g1/source/.muse-claude-sources
.../marketplaces/g1/source/.muse-claude-sources/0
```
and the identical thing for the Codex catalog:
```
".../marketplaces/g2/source/.muse-codex-sources/0/demo-plugin"
```
So every plugin, however remote, is normalised to a `local-path` install transport before
`plugins install <name>@<marketplace>` ever runs.

### 10.6 Install from marketplace

```
$ $M plugins list --available --json
{"available":[{"marketplace":"local1","name":"demo-plugin","version":"0.1.0","status":"available",
   "install":{"transport":"local-path","source":"/.../mkt/demo-plugin"},
   "digest":"sha256:8a510b46..."}],"skipped":[],"warnings":[]}

$ $M plugins install demo-plugin@local1 --json
"source": {"provenance":"marketplace-user-added","path":"/.../mkt/demo-plugin"}

$ $M plugins install nope@local1 --json
{"error":{"code":"invalid-plugin-package","message":"plugin `nope` is not available in marketplace `local1`"}}
$ $M plugins install x@nomarket --json
{"error":{"code":"unknown-marketplace","message":"marketplace `nomarket` is not configured"}}
```
Post-install staleness messages:
```
plugin `X` was installed from marketplace `M` and its cached source is no longer available;
  run `muse plugins remove X`, then `muse plugins install ...`
plugin `X` source `S` is no longer available; run `muse plugins remove X`, then reinstall the
  plugin from an available source
plugin `X` in marketplace `M` failed integrity check
marketplace plugin `X` catalog version `V1` differs from manifest version `V2`
```

---

## 11. Lifecycle verbs — observed behaviour

```
$ $M plugins install ./demo-plugin --scope user --json     # default scope is user
{"installed":{...,"trust":"user-local","source":{"provenance":"native-local",...}},
 "warning":"third-party plugin: hooks and MCP servers require review before activation; ...",
 "diagnostics":[], "plugin":{...full descriptor...},
 "lockfile_path":"<DATA>/muse/plugins/installed.json"}

$ $M plugins install ./pp --scope project --json           # requires trust.json entry
{"installed":{... "trust":"project-trusted" ...}}          # still recorded in the USER lockfile

$ $M plugins disable envprobe --json   -> {"disable":{... "enabled": false ...}}
$ $M plugins enable  envprobe --json   -> {"enable":{... "enabled": true ...}}
$ $M plugins update  envprobe --json   -> {"updated":{... "updated_at":"...", new package_sha256 ...}, "warning": ...}
$ $M plugins remove  envprobe --delete-data --json
{"removed":"envprobe","cache_path":"<DATA>/.../package",
 "data_path":"<DATA>/muse/plugins/data/envprobe","warning":null,
 "lockfile_path":"<DATA>/muse/plugins/installed.json"}
```
Guards:
* `plugin \`demo-plugin\` is already installed from a different source`
* `cannot update plugin \`X\`: the source manifest now declares name \`Y\`; remove and reinstall to change a plugin's id`
* `manifest family changed from <A> to <B>; capabilities were revalidated`
* `plugins install --scope project requires a local plugin path`
* `invalid plugins install --scope \`X\` (expected user or project)`
* `data directory <d> may remain after requested deletion: <err>`

**There is no auto-discovery of project-local plugin directories.** Dropping a valid package
into `<ws>/.agents/plugins/`, `<ws>/.claude/plugins/`, `<ws>/.codex/plugins/`,
`<ws>/.muse/plugins/` or `<ws>/plugins/` produced no entries in `plugins list` or
`skills list --source plugin`. Project plugins must be explicitly
`plugins install <path> --scope project` (which is why `trust: project-trusted` exists as a
lockfile field rather than a discovery scope).

---

## 12. How plugin capabilities surface in the session

### 12.1 Skills

```
$ $M skills list --source plugin
plugin:cc-plugin:lint      plugin  on   Lint the workspace.            plugin://cc-plugin/skills/lint/SKILL.md
plugin:cx-plugin:audit     plugin  on   Audit stuff.                   plugin://cx-plugin/skills/audit/SKILL.md
plugin:demo-plugin:review  plugin  off  Review the current workspace.  plugin://demo-plugin/skills/review/SKILL.md
```
`muse skills enable|disable|user-only <sel> --scope plugin` writes
`settings.skills.activation.plugin["plugin://<id>/skills/<x>/SKILL.md"] = on|off|user-invocable-only`.

Skill scopes: `user | project | bundled | plugin` (also used by `UserIntentSkillScopeV1`).

### 12.2 Commands

Commands become slash commands; the user-intent wire carries
`semantic_kind: "plugin_command"` with a binding
`{scope:"plugin", plugin_id, plugin_version, template_sha256, body_sha256}`.

### 12.3 Display ids

* stable id: `plugin:<plugin>:<kind>:<capability>` (`kind` ∈ `hook | mcp_server | agent_definition`),
  and `plugin:<plugin>:<capability>` for skills/commands in `effective_capabilities`
* display path: `plugin://<plugin>/hook/<id>`, `plugin://<plugin>/mcp/<id>`,
  `plugin://<plugin>/skills/<id>/SKILL.md`

### 12.4 Not on the MSP wire

```
$ $M schema generate-json-schema --out ./schema --experimental
$ grep -io plugin ./schema/msp.schema.json | wc -l
0
```
Plugins are a purely local/CLI/TUI concern in 1.0.1; the MSP host protocol has no plugin
methods, types or events.

---

## 13. The adjacent skills-import pipeline (`muse skills import`)

This is the other half of "Claude/Codex compatibility" and owns the `.muse/lock.json`,
`quarantine` and `audit.log` strings.

```
usage: muse skills import --from claude|codex [--scope user] [--dry-run] [--force] [--json]
       (`skills import --scope project is not supported in Phase 8`; `HOME is required for skills import`)
```
Sources: `~/.claude/skills` and `$CODEX_HOME/skills` (fallback `~/.codex/skills`).

Dry-run output carries the exact compatibility record that ends up in the lockfile:
```json
{ "id":"good","source_path":"~/.claude/skills/good/SKILL.md",
  "target_path":"$CONFIG_DIR/skills/good/SKILL.md","action":"copy","valid":true,
  "classification":"portable",
  "compatibility":{"profile":"agent-skills-common-subset","result":"compatible",
     "known_fields":["allowed-tools","description","name"],
     "unknown_fields":["model"],"unsupported_fields":[],
     "allowed_tools":["Bash","Read"]},
  "unavailable_binaries":[],"missing_requires":[],
  "diagnostics":[{"code":"unsupported-skill-field","severity":"warning",
    "message":"`allowed-tools` is recorded as advisory metadata but is not enforced and grants no tool permissions"}] }
```
Observed field classification:
* `hooks:` in a Claude SKILL.md → `unsupported_fields:["hooks"]` +
  ``Claude field `hooks` is not implemented by Muse Code and is treated as metadata``
* `model:`, `tools:`, `mcp-servers:` → `unknown_fields`
* `requires: [bin]` with a missing binary → `missing_requires`, `classification: "tool-specific"`
  → **quarantined**.

`$CONFIG_DIR/skills/.muse/lock.json` (verbatim):
```json
{ "version": 1,
  "skills": {
    "good": { "id":"good",
      "source":{"type":"imported","path":"~/.claude/skills/good","ecosystem":"claude",
                "source_path":"~/.claude/skills/good"},
      "version":null,"revision":null,
      "installed_at":"2026-09-01T14:38:42.141566Z","updated_at":null,
      "content_sha256":"sha256:c1b8d504...","trust":"imported",
      "scan":{"status":"passed","warnings":[]},
      "files":[{"relative_path":"SKILL.md","sha256":"sha256:2335682c...","bytes":105}] } } }
```
Serde structs: `Lockfile`, `LockFileRecord`, `LockSkillRecord`, `LockScan`, `LockSource`.
`LockSource.type`: `imported | local | git | bundled | plugin | marketplace-static | marketplace-git`;
extra remote fields exist (`repository requested_ref resolved_revision sparse_path
manifest_path manifest_hash package_hash previous_content_sha256 content_sha256`).
The lockfile also models `uninstalled`, `removed_files`, `kept_files`, `provenance`
(the `--keep-files` path of `skills uninstall`).

`$CONFIG_DIR/skills/.muse/audit.log` (JSONL, verbatim):
```
{"time":"2026-09-01T14:38:42.148649Z","action":"install","skill":"good","source":"local","result":"ok"}
{"time":"2026-09-01T14:38:42.176612Z","action":"install","skill":"cxskill","source":"local","result":"ok"}
```
Action vocabulary in the binary: `install update uninstall enable disable
marketplace_add marketplace_update marketplace_remove`.

Quarantine:
```
$ $M skills import --from claude --force
candidates:1  installed:0  quarantined:1  skipped:0  failed:0
quarantined:t1  reasons:missing frontmatter requires: definitely-not-a-binary-xyz
  path:$CONFIG_DIR/skills/.muse/import-quarantine/t1
compat:t1  missing-requires:definitely-not-a-binary-xyz

$ cat $CONFIG_DIR/skills/.muse/import-quarantine/t1/QUARANTINE.txt
quarantined by skills import on 2026-09-01
source: /.../.claude/skills/t1 (written for Claude Code)
reason: missing frontmatter requires: definitely-not-a-binary-xyz
review this skill and move its directory up to $CONFIG_DIR/skills/ to enable it
```

---

## 14. Bundled ("builtin") plugins

The binary embeds two first-party plugins, materialised on first use into
`$DATA_DIR/muse/plugins/cache/builtin/<plugin>/<skill>/<sha256>/`.

`loop` (manifest verbatim from the binary):
```json
{
  "schemaVersion": 1,
  "name": "loop",
  "displayName": "Loop",
  "version": "1.0.0",
  "description": "Built-in recurring-prompt command.",
  "compat": {"source": "native", "manifestDir": ".muse-plugin"},
  "capabilities": {"skills": [], "hooks": [], "mcpServers": [], "commands": [
    {"id": "loop", "path": "commands/loop.md", "enabledDefault": true}
  ]}
}
```
`muse-core` — `"First-party Muse skills."`, version 1.0.0, 15 skills, all
`enabledDefault: true` (materialised names confirmed on disk):
`browser-app-delivery create-plugin create-skill doctor git greenfield-project-scaffolding
grill grill-and-record import manage-settings plan python-env read-session table-fit taste`

`tbh-reminders` — the third bundled plugin, `"First-party reminder agents."`, version 1.0.0,
**6 reminder capabilities and nothing else**. Its manifest is embedded verbatim in the binary
(`LC_ALL=C grep -abo '"name": "tbh-reminders"' muse-aarch64-macos` → offsets 191972673 and
192792205; `strings.txt:5317` shows `"displayName": "TBH Reminders"`). It is not materialised
into `plugins/cache/builtin/` unless the corresponding reminder gates
(`MUSE_EXPERIMENTAL_TODO_REMINDER`, `_MEMORY_REMINDER`, `_SKILL_REMINDER`, `_GOAL_REMINDER`,
`_VERIFY_REMINDER`, `_SCOPE_REMINDER`) are on. Its reminder ids:
`memory`, `skill-reminder`, `todo-reminder`, `goal-reminder`, `verify-reminder`, `scope-reminder`.

Its `memory` entry shows two reminder manifest fields my own probes did not exercise —
`enabledDefault` on a reminder, and the `context` block:
```json
{ "id":"memory","path":"reminders/memory.md","enabledDefault":true,
  "tools":["bash"],"blocking":false,
  "defaultPriority":"normal","maxPriority":"high",
  "maxChildSteps":1000000,"reasoningEffort":"minimal",
  "context":{
    "conversation":{"mode":"bounded","maxTokens":4000},
    "feeds":[{"name":"memory_pack","kind":"host","source":"memory_pack",
              "maxBytes":24000,"refresh":"run_start"},
             {"name":"memory_read_ledger","kind":"host","source":"memory_read_ledger",
              "maxBytes":24000,"refresh":"..."}]}}
```
matching the `ReminderContextDeclaration` structs in §3.7
(`conversation feeds digest_renderer conversation_renderer conversation_estimator`;
`ReminderContextFeedDeclaration`: `name kind digest roots index_file total_max_bytes
placement source max_bytes refresh`; `FirstPartyReminderContextSource`:
`memory_pack | skill_catalog | memory_read_ledger | skill_read_ledger | todo_snapshot`;
`ReminderContextRefreshPolicy`: `run_start | boundary`;
`ReminderContextFeedPlacement`: `before_conversation | after_conversation`).
The full recovered manifest is at
`<scratch>/re/tbh-reminders-manifest.json` (72 KB; recovered by the reminders-dimension pass).

Bundled-plugin errors: `bundled plugin file is not embedded`, `bundled_plugin_invalid`,
`invalid bundled package digest`, `failed to load bundled skills: skill entry missing id/path`,
`bundled skill package must contain exactly one root \`SKILL.md\``,
`builtin://compiled-registry` (the compiled bundled registry), `compiled portable package
validation produced no descriptor`.

Recovered verbatim to
`<scratch>/re/artifacts/create-plugin/{SKILL.md,references/native-plugin-contract.md,references/capability-examples.json}`
— the authoritative first-party plugin-authoring contract, 23.6 KB of executable examples.

---

## 15. Enterprise policy hooks (partially gated)

`muse config validate --plane defaults|policy --file <path>` (gate
`MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG`). The policy envelope has an `extensions` block:

```
PolicyExtensionsV1: skills | hooks | runtime_capabilities
SkillPolicyV1:            allowed_identities denied_identities allowed_sources fallback
SkillPolicyFallbackV1:    off | user-invocable-only
HookPolicyV1:             ... allowed_digests
RuntimeCapabilityPolicyV1: ... allowed_kinds
```
Runtime probe (all three are parsed but inert in 1.0.1):
```
--- clean      : valid: plane=policy schema_version=1
--- skills.*   : enterprise_document_invalid: plane=policy reason=field_not_activated location=extensions.skills
--- hooks.*    : enterprise_document_invalid: plane=policy reason=field_not_activated location=extensions.hooks
--- runtime_capabilities.allowed_kinds : ... reason=field_not_activated location=extensions.runtime_capabilities
--- runtime_capabilities + allowed_identities/... : ... reason=unknown_member
```
There is also a top-level `SettingsFileDocument` field `plugins` (29-field struct:
`... mcpServers presets hooks runtime_capabilities permissions plugins managed_hooks_path
managed_hooks_env_vars endpoint_transport telemetry notifications`), whose value shape I
could not determine — four candidate shapes were written and none changed observable
behaviour and none produced an error.

---

## 16. Reproduction cheat-sheet

```bash
export MUSE_NO_AUTO_UPDATE=1 MUSE_EXPERIMENTAL_PLUGINS=1
export HOME=<scratch>/sandbox/plugins/fakehome
M=<scratch>/muse-aarch64-macos

mkdir -p demo/.muse-plugin demo/skills/review demo/commands demo/hooks demo/mcp
cat > demo/.muse-plugin/plugin.json <<'JSON'
{ "schemaVersion":1,"name":"demo-plugin","displayName":"Demo Plugin","version":"0.1.0",
  "description":"...","compat":{"source":"native","manifestDir":".muse-plugin"},
  "capabilities":{
    "skills":[{"id":"review","path":"skills/review/SKILL.md","enabledDefault":false}],
    "commands":[{"id":"summarize","path":"commands/summarize.md","enabledDefault":true}],
    "hooks":[{"id":"pre-check","event":"PreToolUse","command":["sh","hooks/pre-check.sh"],
              "timeoutMs":1000,"statusMessage":"Checking plugin policy"}],
    "mcpServers":[{"id":"workspace-index","transport":"stdio","command":["python3","mcp/server.py"]}],
    "reminders":[] } }
JSON
printf -- '---\nname: review\ndescription: Review the current workspace.\n---\n\nReview.\n' > demo/skills/review/SKILL.md
printf -- '---\ndescription: Summarize\nargument-hint: <path>\n---\nSummarize $ARGUMENTS\n' > demo/commands/summarize.md
printf '#!/bin/sh\nprintf %%s\\n ok\n' > demo/hooks/pre-check.sh
printf 'import sys\nfor l in sys.stdin: sys.stdout.write(l); sys.stdout.flush()\n' > demo/mcp/server.py

$M plugins validate ./demo --json          # valid demo-plugin native skills=1 commands=1 hooks=1 mcp=1
$M plugins install  ./demo --json
$M plugins inspect  demo-plugin            # hooks/mcp: status=review_needed
$M plugins approve  demo-plugin:hook:pre-check --json
echo '{"event":"PreToolUse","stdin":{"tool_name":"Bash"},"matcher_input":"Bash","cwd":"."}' > fx.json
$M plugins hook test demo-plugin:pre-check --fixture ./fx.json --json
```

---

## 17. Open questions / not established

1. **The exact "Agent Plugins 1.0.0" `$schema` literal.** It is not an `https://` string in the
   binary (an exhaustive URL extraction found only `api.meta.ai`, `auth.meta.com`,
   `dev.meta.ai`, `accountscenter.meta.com`, `github.com/mslsrc/tbh`,
   `json.schemastore.org/claude-code-marketplace.json`, `json-schema.org/draft/2020-12/schema`).
   11 guessed markers all returned `declares unknown root format`. The second constant, the
   Agent-Plugins **MCP** `$schema` for `.mcp.json`, is likewise unrecovered. Without it the
   fourth manifest family cannot be exercised.
2. `settings.plugins` (a real `SettingsFileDocument` member) — value shape and semantics unknown.
3. The precise composition of `definition_hash` / `source_digest` (§9.4) is inferred from four
   experiments, not decompiled.
4. Package depth / entry-count / manifest-byte limits exist as messages
   (`exceeds the maximum depth of <N>`, `more than <N> filesystem entries`,
   `exceeds <N> byte limit`) but the numeric N values were not measured.
5. `plugin_scope_quota`, `plugin_preflight_overflow`, `plugin_class_overflow` imply
   per-scope/per-class caps on how many plugin capabilities enter a session snapshot;
   the numbers are unknown.
6. `curated` / `tbh-curated` marketplace: reserved and wired, but empty in 1.0.1 — no
   first-party registry endpoint was found.
7. The `frozen` / `compiled` / `held-source` machinery (dozens of TOCTOU messages,
   `builtin://compiled-registry`, `file is not frozen`) was only exercised on the happy path.
8. `muse plugins install --scope project` writes to the **user** lockfile; whether a
   project-scoped lockfile exists in a later phase is unknown.

---

# Verification

Adversarial re-verification pass, independent sandbox
`<scratch>/sandbox/verify-plugins/` (`HOME=<that>/fakehome`, `MUSE_NO_AUTO_UPDATE=1`,
`MUSE_EXPERIMENTAL_PLUGINS=1`). Every claim below was re-run from scratch against the
same binary; test packages were rebuilt from the report's own descriptions.

**Overall: MOSTLY_SOLID.** ~90% of the report reproduces exactly — often byte-identical
(`hook-af6225756e4873d1`, `agent-resource:471e2acda4c1…`, `agent-resource:1efde37d95cd…`,
`definition_hash sha256:4953de6c…` all reproduced verbatim from independently rebuilt
packages). Six things are wrong, and one of them (§3.1) would break every plugin
generated from this report's schema table.

## V.1 REFUTED

### R1 — `description` is REQUIRED on a native manifest, not optional  (§3.1, claim 3)

The report's §3.1 table marks `description` "required: no", and structured claim 3 lists it
as optional. It is required, and must be a **non-empty** string:

```
$ ./probe.sh <<'J'
{"schemaVersion":1,"name":"p","version":"0.1.0","compat":{"source":"native","manifestDir":".muse-plugin"},"capabilities":{}}
J
  valid=False code=invalid-manifest-schema msg=plugin manifest must declare string field `description`
  rc=1

# description:"" -> identical error.  description:"d" -> valid=True
```
The check fires **before** the `compat` and `capabilities` checks, which is why the
report's own probes (whose manifests evidently carried a description) never saw it.
This is family-specific: a `.claude-plugin/plugin.json` and a `.codex-plugin/plugin.json`
with no `description` both validate `true`.

### R2 — `compat.source` is neither required nor validated  (§3.1, claim 3)

Claim 3 says required `compat{source,manifestDir}`. Only `manifestDir` is enforced:

```
compat:{"manifestDir":".muse-plugin"}                  -> valid=True
compat:{"source":"claude","manifestDir":".muse-plugin"} -> valid=True   # bogus source accepted
compat:{"source":"native"}                              -> manifest-family-mismatch: must declare compat.manifestDir
```
`"source":"native"` is a convention from `references/native-plugin-contract.md`, not a
validator rule.

### R3 — the INFERRED `definition_hash`/`source_digest` model is wrong for `mcp_server`, and is family-dependent  (§9.4, claim 13, open question 3)

§9.4 proposes `source_digest` = "package content *other than* that capability's own
declaration file". Two new controlled experiments refute the general form.

**Native family — `source_digest` is literally `package_sha256`, with no exclusion:**
```
$ muse plugins inspect demo-plugin --json
  pkg_sha= 3f252026
  plugin:demo-plugin:hook:pre-check              dh=a0ad4ee8 sd=3f252026
  plugin:demo-plugin:mcp_server:workspace-index  dh=8ac10595 sd=3f252026
# approve, then edit ONLY .muse-plugin/plugin.json (description text) -> update
  pkg_sha= 1d0ad819
  both capabilities -> "modified", sd=1d0ad819   # the manifest is NOT excluded
```

**Claude family — `mcp_server` sd == package_sha256; hooks get a separate digest that
excludes BOTH the manifest and the hook source:**
```
E0 (approved)      pkg_sha=1454604d  3 hooks sd=e7b8168a trusted_enabled | mcp sd=1454604d trusted_enabled
edit .claude-plugin/plugin.json  ("license": MIT -> Apache-2.0)
E1                 pkg_sha=ffaadf49  3 hooks sd=e7b8168a TRUSTED_ENABLED  | mcp sd=ffaadf49 MODIFIED
```
So the mcp server's declaration file (the manifest) **is** covered by its own digest —
exactly the case the report's model says should be excluded. The reproducible rule is:

* every runtime capability's `source_digest` is the plugin's `package_sha256`, **except**
* a foreign-family **hook**, whose `source_digest` is a distinct digest, shared by every
  hook in the package, that is invariant under edits to the manifest and to the hook
  source (whether `hooks/hooks.json` or an inline `hooks` object) and changes on any
  other package byte change.

The report's experiments B and C reproduce exactly (`scripts/post.sh` edit → all four
`modified`; `hooks.json` timeout edit → edited hook gets a new content-derived id
`hook-82be25fec5ad52be` and goes `review_needed`, siblings stay `trusted_enabled` with
unchanged sd, mcp goes `modified`). Only the *explanation* is wrong.

### R4 — the summary's "any byte change to the package flips them all to `modified`" is false for foreign plugins

Claim 13's specific experiment (add an unrelated `README.md` → everything `modified`)
reproduces. The generalisation in the summary does not: per R3, editing
`.claude-plugin/plugin.json` or `hooks/hooks.json` leaves sibling Claude hooks
`trusted_enabled`. It IS true without exception for the native family.

### R5 — `plugins hook test` does NOT require approval  (§9.5)

§9.5: "Hook must be installed **and enabled/approved**". Only *enabled* is required.
A freshly installed, never-approved hook runs; so does one that has been `reject`ed:

```
$ muse plugins inspect hp2 --json   ->  plugin:hp2:hook:probe review_needed
$ muse plugins hook test hp2:probe --fixture ./fx.json --json
  terminals: 1  status completed  exit 0  stdout '{}\n'
$ muse plugins reject hp2 && muse plugins hook test hp2:probe ...   -> still runs
$ muse plugins disable hp2 && muse plugins hook test hp2:probe --json
  {"error":{"code":"plugin-hook-test-failed","message":"plugin hook `hp2:probe` is not installed and enabled"}}
```
(`plugin-hook-test-failed` is a CLI error code absent from the report's §7 code lists.)
This makes hook-test *better* as a CI harness than the report claims — no approval step
needed — but the stated precondition is wrong.

### R6 — the diagnostic vocabulary is 25 codes, not 23  (structured claim 30)

The report body §7 correctly lists 25; the structured finding says 23. The binary's table:
```
invalid-plugin-package bundled_plugin_id_reserved missing-manifest multiple-manifests
invalid-manifest-json invalid-manifest-schema manifest-family-mismatch invalid-plugin-id
invalid-version invalid-capability-id missing-capability-path missing-capability-command
unsupported-hook-event duplicate-capability-id duplicate-hook-source unsafe-path
unsupported-capability unsupported-field honored-declaration unsupported-agent-schema
ignored-root-manifest agent-component-invalid agent-skill-skipped agent-mcp-server-skipped
agent-overlay-inactive
```

## V.2 CORRECTIONS (claim broadly right, detail wrong)

* **§3.5.1 "snake_case wire forms also exist".** Manifest input accepts **PascalCase only**:
  `"event":"pre_tool_use"` → `unsupported-hook-event`. snake_case appears only in the
  hook-run terminal (`"event":"pre_tool_use"`). All 17 PascalCase names validate; the
  three probes `PreSubmit`/`PreResponse`/`SubagentComplete` are rejected, so 17 is exact.
* **§15 enterprise policy.** `extensions.runtime_capabilities.allowed_identities` returns
  `field_not_activated`, **not** `unknown_member` as the report's table says. Measured
  member map (`field_not_activated` = known, `unknown_member` = not a member):
  `extensions.skills` = {allowed_identities, denied_identities, allowed_sources, fallback}
  (4 = `struct SkillPolicyV1 with 4 elements` ✓);
  `extensions.hooks` = {allowed_identities, denied_identities, allowed_sources,
  allowed_digests} + 1 unnamed (`HookPolicyV1 with 5 elements`);
  `extensions.runtime_capabilities` = {allowed_identities, denied_identities,
  allowed_kinds} + 1 unnamed (`RuntimeCapabilityPolicyV1 with 4 elements`).
  `extensions.plugins` → `unknown_member` (no plugin-level policy block exists).
* **§2.2 symlinks.** `validate` returns top-level
  `invalid-plugin-package | Agent Definition inventory derivation failed closed`;
  the `plugin contains symlink entries…` string appears only as a *secondary* diagnostic.
  `install` surfaces it as the top-level message. The report attributes it to `validate`.
* **§3.2 reserved ids.** `references/capability-examples.json` contains
  `"reservedPluginIds": ["loop","muse-core","tbh-reminders"]` — one array, all three.
  The report's "(skill text also names `loop`)" is backwards: it is
  `references/native-plugin-contract.md` prose that names only `loop` and `muse-core`.
  Runtime behaviour reproduces for all three (validate ✓, install ✓,
  `effective_capabilities: []`, `bundled_plugin_id_reserved`).
* **§3.9 `apps`.** On a **native** manifest `capabilities.apps` is a hard error
  (`plugin apps capabilities are not supported in this phase`), matching
  `unsupportedFamilies: ["tools","agents","outputStyles","settings","apps"]` in
  capability-examples.json. Only the *foreign* parsers say "ignored by this runtime".
* **ohmy hook #14 (`plugins/data/<id>`).** The directory is **advertised but not created**:
  after a hook run whose env carried `MUSE_PLUGIN_DATA_DIR=<DATA>/plugins/data/envprobe`,
  `ls <DATA>/plugins/data/` → no matches. A plugin must `mkdir -p` it itself.

## V.3 CONFIRMED (re-run, reproduced)

Claims 1, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
25, 26, 27, 28, 29 all reproduce. Highlights:

* Gate: ungated `plugins are not available in this build`; gated adds
  `plugins  Validate and manage plugin bundles` to `muse --help` (line 16).
* All 17 `HookEventKind` values validate; `Setup`/`NotARealEvent` → `unsupported-hook-event`.
* `matcher` rejected on native hooks; `compatibilityName "Bash"` collides; `"my_custom_tool"`
  accepted; `outputCapabilities` must be exactly `["skills.v1"]` and only on **foreground**
  `UserPromptSubmit`/`PostToolUse` (adding `"async":true` re-triggers the rejection).
* Claude cc-plugin: `manifest_family: claude-compatible`, `summary: partial`, all ten
  diagnostics verbatim, all nine `compatibility.declarations` verbatim,
  `mcpServers` command+args flattened, `timeout:5 → timeout_ms:5000`.
* `${CLAUDE_PLUGIN_ROOT}` expansion + shell execution proven twice: a non-executable
  script gives `zsh:1: permission denied: <cache>/…/scripts/check.sh`, exit 126; and a
  command with `&&` runs both halves (`stdout: "okscript\nSHELLPIPE\n"`).
* Seven hook env vars + JSON on stdin + cwd=workspace reproduced exactly.
* Lockfile / marketplaces.json / snapshot.json shapes verbatim;
  `struct LockPluginRecord with 15 elements`, `LockSourceRecord with 3`, `Lockfile with 2`
  confirmed in the binary, `source_format_version` is a real serde field (absent from
  serialized output).
* Git marketplace → `.muse-claude-sources/0/demo-plugin` and (Codex catalog)
  `.muse-codex-sources/0/demo-plugin`, both normalised to `transport: local-path`.
* `tbh-curated` reserved; `demo-plugin@cc1` → `provenance: marketplace-user-added`.
* No auto-discovery — retested in **both** an untrusted **and** a `trust.json`-trusted
  workspace with valid packages in `.agents/plugins`, `.claude/plugins`, `.codex/plugins`,
  `.muse/plugins`, `plugins`. Nothing appears in `plugins list` or `skills list`.
* trust.json shape exact, incl. `missing field \`decision\` at line 1 column 184`.
  `--trust-workspace` is a TUI-only flag and cannot substitute.
* Agent Definitions: byte-identical inventory to the report's, `plugins approve ad` →
  `plugin:ad:agent_definition:agent-resource:471e2acda4c1…`.
* `skills import --from claude`: lock.json / audit.log / import-quarantine/t1/QUARANTINE.txt
  reproduced verbatim, including the four-line QUARANTINE.txt text.
* MSP: `msp.schema.json` 190535 bytes, `grep -io plugin | wc -l` = **0**; the TS export
  (`schemats/msp.d.ts`) is likewise 0.
* `developerPrompts` is genuinely hard-gated: still
  `plugin developerPrompts capabilities are not supported in this phase` with **all 41**
  `MUSE_EXPERIMENTAL_*` gates exported =1. (Gate table has 41 names, not "~40 siblings"
  plus `plugins`: workflow_tool … tag.)
* Open question 1 stands: 13 further `$schema` guesses (agentplugins.org/.dev/.io,
  agents.md, schemastore, openai.com, raw.githubusercontent, bare `agent-plugins/1.0.0`, …)
  all returned `declares unknown root format`. No `$schema`-shaped literal exists in the
  binary's printable strings.

## V.4 MISSED GROUND

### M1 — the hook **stdout protocol** is entirely undocumented (biggest gap)

The report documents the parsed `decision` object but never how a hook produces it. It is
a strict, Claude-Code-compatible JSON contract, enforced per event:

```
stdout '{}'                                              -> completed, empty decision
stdout '{"decision":"block","reason":"nope"}'            -> status=blocked, should_block=true, block_reason="nope"
stdout '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"policy"}}'
                                                          -> status=blocked, permission_decision="deny", block_reason="policy"
stdout '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"allow",
         "permissionDecisionReason":"ok","updatedInput":{"command":"ls -la"}}}'
                                                          -> completed, updated_input={"command":"ls -la"}, effects=["rewrite_selected"]
stdout '{"systemMessage":"hello"}' / '{"suppressOutput":true}'  -> accepted, ignored
stdout '{"continue":false,"stopReason":"halt"}'          -> status=FAILED, error="unsupported `continue` in output of PreToolUse hook output"
stdout '{"additionalContext":"x"}' (top level)           -> status=FAILED, error="unsupported `additionalContext` in output of PreToolUse hook output"
```
On `UserPromptSubmit`:
```
stdout '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"CTX!"}}'
                                                          -> additional_contexts: ["CTX!"]
stdout 'just text'   (non-JSON)                           -> additional_contexts: ["just text"]
malformed JSON                                            -> error="malformed_output: hook stdout started as JSON but did not parse: …"
```
So: unknown top-level keys are a hard failure (not ignored), non-JSON stdout is treated as
raw context on `UserPromptSubmit`, and `effects` really is populated (`rewrite_selected`).
Any oh-my-musecode hook library must target this contract, not Claude Code's superset.

### M2 — the numeric limits are measurable (closes open question 4)

```
manifest bytes : "plugin manifest exceeds 131072 byte limit"      (64 KiB ok, 128 KiB fails)  -> 131072 exact
package entries: 4096 total files+dirs ok, 4097 fails
                 ("Agent Definition inventory derivation failed closed")
directory depth: 16 levels below the plugin root ok, 17 fails
                 ("Agent Definition inventory depth limit exceeded")
```
Note the *Agent Definition inventory* limits fire first and mask the package-level
messages the report quotes (`plugin <id> directory nesting exceeds the maximum depth of
<N>`, `… more than <N> filesystem entries`), so 16/4096 are the **effective** caps at
install time regardless of what the package walker's own caps are.

### M3 — Claude `hooks` may be an inline object, not only a path

The report only shows `"hooks": "./hooks/hooks.json"`. An inline map works and is
attributed to the manifest:
```json
{"name":"ih","description":"…","version":"1.0.0",
 "hooks": {"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"echo A"}]}]}}
```
→ `hook-bd88a7b3e8a97c38`, `"source_relative_path": ".claude-plugin/plugin.json"`.
(Double-nesting `{"hooks":{"hooks":{…}}}` fails with `foreign hook event \`hooks\` is unsupported`.)

### M4 — native hooks: no shell, no placeholder expansion

Proven, and worth stating explicitly because the report only implies it:
```
command ["/bin/echo","A && echo B"] -> stdout "A && echo B\n"     (no shell)
command ["/bin/echo","$MUSE_PLUGIN_ID"] -> stdout "$MUSE_PLUGIN_ID\n" (no expansion)
command ["/bin/echo","${CLAUDE_PLUGIN_ROOT}/x/y.txt"]
   -> install fails: missing-capability-path | plugin file is not readable
      path=<root>/${CLAUDE_PLUGIN_ROOT}/x/y.txt
```
i.e. `${CLAUDE_PLUGIN_ROOT}` in a native argv is taken as a *literal path component*.
Use `MUSE_PLUGIN_ROOT` from the environment inside the script instead.

### M5 — two capability keys are warn-and-ignore on a native manifest

Unlike `tools/agents/outputStyles/settings/apps` (hard errors), these validate `true`:
```
capabilities.lspServers -> unsupported-field | plugin capability field `lspServers` is not used by this runtime
capabilities.userConfig -> unsupported-field | plugin capability field `userConfig` is not used by this runtime
```

### M6 — smaller facts

* Native `mcpServers.transport` whitelist: `stdio`/`http` only —
  `mcp server \`m\` transport \`sse\` is unsupported`. HTTP entries emit `"command": []`.
* An `agent_definition` runtime-capability *candidate* has a different shape from
  hook/mcp candidates: `{plugin_id, scoped_definition_id, original_ordinal}` only — no
  `stable_id`, `display_path`, `definition_hash` or `source_digest`.
* `settings.json → runtime_capabilities` is **never garbage-collected**: after the
  hooks.json edit that retired `hook-af6225756e4873d1`, its trust entry is still present.
  An oh-my-musecode `doctor` should reconcile it.
* `plugins list --json` shape (never shown in the report):
  `{"plugins":[{record, warning, plugin, valid, active, active_scope, diagnostics}]}`.
* Duplicate rules reproduce: `duplicate hook capability id \`h\``,
  `hook capability \`h2\` reuses source \`hooks/h.sh\` already used by \`h1\``,
  `command capability id \`review\` duplicates a skill id in the same plugin`.
* A root `plugin.json` with a non-Agent `$schema` does **not** block a nested manifest —
  the nested `.muse-plugin/plugin.json` still wins (`ignored-root-manifest` warning only).
* Package cap probe artefacts confirm `plugins validate` exit codes are 0/1 as claimed.
