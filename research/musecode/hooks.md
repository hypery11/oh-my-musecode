# Meta Muse Code ("TBH") 1.0.1-R2006.1 — Hooks Subsystem

Reverse-engineered from the stripped arm64 Mach-O at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
plus live execution in a sandboxed `HOME`.

Every claim below is either **PROVEN** (reproduced by running the binary, output pasted verbatim)
or **INFERRED** (string evidence only, marked inline).

Sandbox used for all runs:

```
export MUSE_NO_AUTO_UPDATE=1
export HOME=<scratch>/sandbox/hooks/home           # never the real ~
W=<scratch>/sandbox/hooks/ws                        # workspace
```

---

## 1. TL;DR

* Muse Code has a **full Claude-Code-shaped hooks system** that is live and working in 1.0.1-R2006.1.
* File: **`<workspace>/.muse/hooks.json`** (project tier) and the **`hooks` key of `$CONFIG_DIR/muse/settings.json`** (user tier).
* **There is NO `schema_version` for `hooks.json`.** Unknown top-level keys are ignored. The only
  forbidden top-level key is `managed_hooks_env_vars` (rejects the whole file).
* **17 hook events**, CamelCase on the wire (`HookEventKind`), snake_case internally.
  Muse adds 6 events Claude Code does not have: `PermissionRequest`, `PreLLMCall`, `PostLLMCall`,
  `PostCompact`, `SubagentStart`, `PostToolUseFailure`, `StopFailure`, `PostToolBatch`.
* **Four declaration tiers**, executed in this order: **managed → user → project → plugin**.
* Hooks **can block** (`decision:block`, exit 2), **can deny/allow a permission request**, and
  **can rewrite tool input** (`permissionDecision:"allow"` + `updatedInput`, effect `rewrite_selected`).
* The **enterprise `defaults` vs `policy` split is real and provable**, but the hooks-specific policy
  field `extensions.hooks` is **declared-but-not-activated in this build** — it validates the schema
  shape and then fails `field_not_activated`. Enterprise **cannot ship hooks** today; it *can* set
  `settings.max_consecutive_stop_hook_continuations` in the `defaults` plane.

---

## 2. Where hooks may be declared (discovery matrix) — PROVEN

Test method: put an identical `SessionStart` hook (`echo <TAG> > $O/<TAG>.txt`) at each candidate
location, run `muse exec --provider echo --trust-workspace "hi"`, list `$O`.

| Candidate path | Loaded? |
|---|---|
| `<workspace>/.muse/hooks.json` | **YES** (tier `project`, requires workspace trust) |
| `$CONFIG_DIR/muse/settings.json` → `"hooks"` key | **YES** (tier `user`) |
| file named by `settings.json` → `"managed_hooks_path"` | **YES** (tier `managed`) |
| env `TBH_MANAGED_HOOKS_PATH=<file>` | **YES** (tier `managed`) |
| plugin `.muse-plugin/plugin.json` → `capabilities.hooks[]` | **YES** (tier `plugin`, needs approval) |
| plugin `.claude-plugin/plugin.json` → `"hooks": "hooks/hooks.json"` | **YES** (foreign compat, needs approval) |
| `$CONFIG_DIR/muse/hooks.json` | no |
| `~/.muse/hooks.json` | no |
| `~/.claude/settings.json` | no |
| `~/.codex/hooks.json` | no |
| `<workspace>/.muse/settings.json` | no |
| `<workspace>/hooks.json` | no |
| `<workspace>/.claude/settings.json`, `.codex/`, `.agents/` | no |

`$CONFIG_DIR` = `$XDG_CONFIG_HOME/muse` if set, else `$HOME/.config/muse` (**PROVEN**: setting
`XDG_CONFIG_HOME` to a scratch dir made `<xdg>/muse/settings.json` hooks fire).

Evidence (multi-location run):

```
$ ls $O
PROJMUSEHOOKS.txt              # only .muse/hooks.json fired out of 7 candidates
$ TBH_MANAGED_HOOKS_PATH=$M/env-managed.json muse exec --provider echo --trust-workspace hi
$ ls $O
ENVMANAGED.txt  PROJMUSEHOOKS.txt
```

### 2.1 Workspace trust gates the project tier — PROVEN

```
### WITHOUT --trust-workspace
muse: warning: rules file at .../AGENTS.md exists, but the workspace is untrusted, so it is skipped
echo: hi
(no hooks fired)
### WITH --trust-workspace
PROJMUSEHOOKS.txt
```

The startup security-mode context block says it in words (from the session log):

> `Workspace trust: trusted — project-local instructions, skills, and hooks are eligible to load.`
> `Workspace trust: untrusted — project-local instructions, skills, and hooks are not loaded.`

and the TUI trust prompt string in the binary:

> `Trusting allows project-local skills, rules, hooks, and plugin config to load before the model runs.`

### 2.2 Tier ordering / precedence — PROVEN

Four `SessionStart` hooks, one per tier, each returning a distinct `additionalContext`:

```
  order=0        run=.../managed/mh.json:session_start:def-6ba2ba3d70cf31ea:1
  order=1        run=.../.config/muse/settings.json:session_start:def-d6f7fe1e7e57f5a2:2
  order=2        run=.../ws/.muse/hooks.json:session_start:def-9b467eafbd569d44:3
  order=100011   run=plugin:allev:sessionstart:4
--- context blocks ---
  block=hook:session_start:session:0 role=developer order=900000 text=FROM-MANAGED
  block=hook:session_start:session:1 role=developer order=900001 text=FROM-USER
  block=hook:session_start:session:2 role=developer order=900002 text=FROM-PROJECT
  block=hook:session_start:session:3 role=developer order=900003 text=FROM-PLUGIN
```

So: **managed(0) < user(1) < project(2) < plugin(100000+)**. Config-file handlers get a
`display_order` counted from 0; plugin capabilities are shifted into a 100000+ band.

---

## 3. `.muse/hooks.json` — exact schema

### 3.1 Top level

```jsonc
{
  // OPTIONAL, IGNORED. No schema_version is required or validated.
  // "schema_version": 1 | 2 | 99  -> all accepted, all ignored
  // any unknown top-level key     -> ignored
  // "managed_hooks_env_vars"      -> REJECTS THE WHOLE FILE (see 3.5)
  "hooks": {
    "<HookEventKind>": [ <MatcherGroup>, ... ],
    ...
  }
}
```

PROVEN (each row is one full `muse exec` run; `fired=1` means the hook executed):

```
  baseline                                 fired=1
  schema_version 1                         fired=1
  schema_version 2                         fired=1
  schema_version 99                        fired=1
  unknown top key                          fired=1
  managed_hooks_env_vars in file           fired=0     <-- whole file rejected
  managed_hooks_path in file               fired=1     <-- ignored as unknown key
```

Binary strings for the top-level errors:

```
hooks must be an object
hook event `<name>` must be an array
unsupported hook event `<name>`
malformed hook config: <err>              /  malformed hook config at <path>: <err>
failed to read hook config: <err>         /  failed to read hook config at <path>: <err>
 hook file cannot declare User-tier field `managed_hooks_env_vars`
 hook content cannot declare User-tier field `managed_hooks_env_vars`
```

### 3.2 MatcherGroup

```jsonc
{
  "matcher": "<string>",        // OPTIONAL. Absent/"" = match everything.
  "hooks": [ <Handler>, ... ]   // REQUIRED, must be an array
}
```

No other key is allowed. **`description` at group level drops the group** (PROVEN: `fired=0`),
which is the native counterpart of the strings

```
D66: unknown matcher-group field `<name>` may narrow execution, so this group is skipped
hook matcher group must declare `hooks`
hook matcher group must be an object, found <x>
hook matcher group `hooks` must be an array, found <x>
hook matcher must be a string, found <x>
`<matcher>` is unmatchable; drop this group's matcher
```

**Matcher semantics — PROVEN on `SessionStart`**: the matcher is compared against the event's
selector value (for `SessionStart` that is the `source` field, `"startup"`).

```
$ # groups: matcher "startup" | "resume" | "*" | "NoSuchThing"
$ ls $O
m-star.txt   m-startup.txt        # only "startup" and "*" fired
```

`*` matches everything. For `PreToolUse`/`PostToolUse`/`PermissionRequest` the matcher input is the
tool name — the plugin-hook fixture format has a dedicated `matcher_input` field, and my
`PreToolUse` fixture used `"matcher_input": "bash"` (**INFERRED** that tool-name matching uses the
same regex-ish string comparison Claude Code uses; only exact/`*` was directly exercised).

### 3.3 Handler

```jsonc
{
  "type": "command",             // REQUIRED. Only "command" is supported.
  "command": "<shell string>",   // REQUIRED, non-empty, run through a shell
  "commandWindows": "<string>",  // OPTIONAL, Windows-only alternative
  "timeout": <non-negative int>, // OPTIONAL, SECONDS (not ms)
  "statusMessage": "<string>",   // OPTIONAL, shown in the TUI while running
  "async": true|false,           // OPTIONAL, fire-and-forget (no terminal record, no decision)
  "silent": true|false,          // OPTIONAL, accepted and IGNORED (D65)
  "outputCapabilities": ["skills.v1"]  // OPTIONAL, exactly this value, only on
                                       // UserPromptSubmit / PostToolUse
}
```

Field-by-field behaviour, one isolated `muse exec` run per row — **PROVEN**:

| handler JSON fragment | result |
|---|---|
| *(baseline)* | runs, `status=completed` |
| `"description":"d"` | **dropped** (unknown handler field) |
| `"once":true` | **dropped** — `D96: `once: true` one-shot handlers are unsupported` |
| `"async":true` | **runs**, but emits **no `hook_run_terminal` record** and contributes no decision |
| `"silent":true` | runs (D65: display-only compatibility metadata, ignored) |
| `"if":"true"` | **dropped** — `D66: a conditional handler is unsupported; TBH cannot prove the condition` |
| `"shell":"bash"` | **dropped** — `D98: a per-handler `shell` selector is unsupported` |
| `"asyncRewake":true` | **dropped** — `D97: `asyncRewake: true` handlers are unsupported` |
| `"outputCapabilities":["skills.v1"]` on `SessionStart` | **dropped** (`skills.v1` is only valid on foreground `UserPromptSubmit`/`PostToolUse` command hooks) |
| `"outputCapabilities":["skills.v1"]` on `UserPromptSubmit` | runs |
| `"outputCapabilities":["bogus"]` / `[]` | **dropped** — `outputCapabilities must be exactly ["skills.v1"]` |
| `"timeout":0` | runs |
| `"timeout":-1` | **dropped** (`a non-negative integer`) |
| `"timeout":100000` | runs (no ceiling found on the native path) |
| unknown field `"zzz":1` | **dropped** |
| `"commandWindows"` only (on macOS) | **dropped** — `D63: this handler declares only a Windows command form (commandWindows), which is selected on Windows only, so it has no command to run on this platform` |
| `"command":""` | **dropped** — `command hook command must not be empty` |
| no `"type"` | **dropped** — `hook handler must declare a `type`` |
| `"command":["sh","-c","..."]` (argv array) | **dropped** — `D99: Claude `command` plus `args` argv execution is not implemented yet (#10133)` |
| `"type":"webhook"` | **dropped** — `D101: hook handler type `<t>` is unsupported` |

**`timeout` is in seconds** — PROVEN:

```json
{"kind":"hook_run_terminal", ... "status":"timed_out","duration_ms":1002,
 "exit_code":null,"error":"hook timed out after 1s"}      // handler had "timeout": 1, cmd slept 2s
```

**stdout ceiling is 16 KiB** — PROVEN:

```json
{"kind":"hook_run_terminal", ... "status":"failed","exit_code":null,
 "error":"output_too_large: hook stdout exceeded its 16384-byte ceiling; the process tree was terminated and no stream was parsed"}
```

`statusMessage` shows up in the record as `status_message`:

```json
{"kind":"hook_run_terminal", ... "display_order":2,"status_message":"msg","status":"completed", ...}
```

### 3.4 A complete, working `.muse/hooks.json` (verbatim, verified to run)

```json
{
  "hooks": {
    "SessionStart": [
      { "matcher": "startup",
        "hooks": [ { "type": "command", "command": "echo hi > /tmp/ss.txt" } ] }
    ],
    "UserPromptSubmit": [
      { "hooks": [ { "type": "command",
                     "command": "printf '%s' '{\"hookSpecificOutput\":{\"hookEventName\":\"UserPromptSubmit\",\"additionalContext\":\"EXTRA-CTX\"}}'",
                     "statusMessage": "adding context",
                     "timeout": 5 } ] }
    ],
    "PreToolUse": [
      { "matcher": "bash",
        "hooks": [ { "type": "command", "command": "/path/to/gate.sh", "timeout": 10 } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command",
                     "command": "printf '%s' '{\"decision\":\"block\",\"reason\":\"keep going\"}'" } ] }
    ]
  }
}
```

### 3.5 `managed_hooks_path` / `managed_hooks_env_vars` — PROVEN

In `$CONFIG_DIR/muse/settings.json` (a typed `SettingsFile`, `schema_version: 1`):

```json
{
  "schema_version": 1,
  "managed_hooks_path": "/abs/path/to/managed-hooks.json",
  "managed_hooks_env_vars": ["FOO_MANAGED"],
  "hooks": { "SessionStart": [ ... ] }
}
```

* `managed_hooks_env_vars` is an **array of environment-variable NAMES** (not a map, not `NAME=VAL`):

```
managed_hooks_env_vars = ["MYMANAGED"]      -> accepted
managed_hooks_env_vars = ["MYMANAGED=yes"]  -> invalid managed_hooks_env_vars[0]: name must match [A-Za-z_][A-Za-z0-9_]*
managed_hooks_env_vars = {"MYMANAGED":"y"}  -> malformed settings file ...: invalid type: map, expected a sequence
managed_hooks_env_vars = [{"name":...}]     -> malformed settings file ...: invalid type: map, expected a string
```

* Those variables are forwarded **only to managed-tier hooks**. PROVEN by diffing the `env` dump of a
  managed hook against a user hook in the same run:

```
=== diff user-env vs managed-env ===
4a5
> MYMANAGED=hello-managed
```

* A **hook file may never declare `managed_hooks_env_vars`** — not even the managed file itself.
  Adding it to `managed-hooks.json` silently dropped that whole file
  (`fired: p.txt` only, the managed `m.txt` was gone).

### 3.6 Hook process environment — PROVEN

Hooks get a **scrubbed allow-list environment**, cwd = workspace root. Full `env` seen by a
project-tier hook:

```
_=/usr/bin/env
HOME=<sandbox home>
LANG=en_US.UTF-8
LOGNAME=cph
OLDPWD=<workspace>
PATH=<inherited PATH>
PWD=<workspace>
SHELL=/bin/zsh
SHLVL=1
TERM=xterm-ghostty
TMPDIR=/var/folders/.../T/
USER=cph
```

No `MUSE_*`/`TBH_*` variables are exported to config-file hooks. **Plugin** hooks additionally get:

```
MUSE_PLUGIN_ID=hooktest
MUSE_PLUGIN_ROOT=<data>/muse/plugins/cache/local/hooktest/<pkgsha>/package
MUSE_PLUGIN_DATA_DIR=<data>/muse/plugins/data/hooktest
CLAUDE_PLUGIN_ROOT=<same as MUSE_PLUGIN_ROOT>       # Claude compat alias
CLAUDE_PLUGIN_DATA=<same as MUSE_PLUGIN_DATA_DIR>   # Claude compat alias
```

(`MUSE_TOOL_USE_ID` exists in the binary but sits next to shell-tool strings, not hook strings —
**unknown** whether it is exported to `PreToolUse` hooks.)

---

## 4. The event model

### 4.1 The 17 events — PROVEN (enum literal in the binary + accepted by the plugin validator)

Rust enum name and variants, recovered verbatim from adjacent serde literals:

```
HookEventKind
  SessionStart UserPromptSubmit PreToolUse PermissionRequest PostToolUse
  PreLLMCall PostLLMCall PreCompact PostCompact SubagentStart SubagentStop
  Stop SessionEnd Notification PostToolUseFailure StopFailure PostToolBatch
```

snake_case serialization (used for `hook_key`, context-block ids and the enterprise policy map):

```
session_start user_prompt_submit pre_tool_use permission_request post_tool_use
pre_llm_call post_llm_call pre_compact post_compact subagent_start subagent_stop
stop session_end notification post_tool_use_failure stop_failure post_tool_batch
```

`muse plugins validate` accepted all 17 CamelCase names in a manifest and rejected
`PreSubmit`/`Error` with `foreign hook event `X` is unsupported`.
`.muse/hooks.json` keys must be **CamelCase**: a `session_start` key silently loads nothing.

Extra Claude compat: event `Setup` parses but is a warning —
`Claude hook event `Setup` is recognized but is not run by Muse; complete any required plugin setup manually`.

### 4.2 Which events actually fire in a trivial echo session — PROVEN

Registering all 17 and running `muse exec --provider echo "hi"`:

```
=== FIRED ===
SessionStart.json  UserPromptSubmit.json  PreLLMCall.json
PostLLMCall.json   Stop.json              SessionEnd.json
```

(The other 11 need tool calls / compaction / subagents / notifications, which the `echo`
provider never produces.)

### 4.3 Hook input payloads (stdin, one JSON object) — PROVEN verbatim

Every payload carries a common tail: `session_id`, `cwd`, `transcript_path`, `model`,
`permission_mode`; turn-scoped events also carry `turn_id`.

```json
// SessionStart
{"hook_event_name":"SessionStart","source":"startup","session_id":"01a05d74-…",
 "cwd":"…/ws","transcript_path":null,"model":"unknown","permission_mode":"default"}

// UserPromptSubmit
{"hook_event_name":"UserPromptSubmit","prompt":"hello world","session_id":"01a05d74-…",
 "turn_id":"d7627110-…","cwd":"…/ws","transcript_path":null,"model":"unknown",
 "permission_mode":"default"}

// PreLLMCall
{"hook_event_name":"PreLLMCall","provider":"model.unknown.response",
 "request_id":"d7627110-…:0:1","attempt":1,"step":0,
 "messages":[…],"message_count":1,
 "tools":[{"name":"read_file","description":"…","has_parameters":true,"strict":false}, …],
 "tool_count":25,"options":{},"session_id":"…","turn_id":"…","cwd":"…","transcript_path":null,
 "model":"unknown","permission_mode":"default"}

// PostLLMCall
{"hook_event_name":"PostLLMCall","provider":"model.unknown.response",
 "request_id":"…:0:1","attempt":1,"step":0,"status":"success","response_id":"muse-tui-echo",
 "usage":{"input_tokens":0,"output_tokens":0,"cached_tokens":0,"reasoning_tokens":0},
 "finish_reason":null,"error":null,"output_text_preview":"echo: hello world","tool_call_count":0,
 "messages":[…],"message_count":2,"tools":[…],"tool_count":25,
 "options":{"meta.session_id":"…","meta.traceparent":"00-…-01"},
 "session_id":"…","turn_id":"…","cwd":"…","transcript_path":null,"model":"unknown",
 "permission_mode":"default"}

// Stop
{"hook_event_name":"Stop","stop_hook_active":false,"last_assistant_message":"echo: hello world",
 "session_id":"…","turn_id":"…","cwd":"…","transcript_path":null,"model":"unknown",
 "permission_mode":"default"}

// SessionEnd
{"hook_event_name":"SessionEnd","reason":"other","session_id":"…","cwd":"…",
 "transcript_path":null,"model":"unknown","permission_mode":"default"}
```

`stop_hook_active` flips to `true` on every continuation after the first — PROVEN with a
Stop hook that always blocks and `max_consecutive_stop_hook_continuations: 2`:

```
stop-0.json {'hook_event_name': 'Stop', 'stop_hook_active': False, …}
stop-1.json {'hook_event_name': 'Stop', 'stop_hook_active': True,  …}
stop-2.json {'hook_event_name': 'Stop', 'stop_hook_active': True,  …}
```

Payload fields for the events that could not be fired live (recovered as a contiguous serde
literal run in `.rodata`, high confidence, **PROVEN as strings / INFERRED as payload shape**):

```
hook_event_name
  PreLLMCall        request_id  message_count  tool_count
  PostLLMCall       response_id finish_reason  output_text_preview  tool_call_count
  PermissionRequest tool_name   tool_input
  PreToolUse        tool_name   tool_input     tool_use_id
  PostToolUse       tool_use_id tool_response
  Stop              last_assistant_message
  PreCompact        reason
  PostCompact
  SessionStart      source
  SessionEnd        reason
  UserPromptSubmit  prompt
  SubagentStart     subagent_id
  SubagentStop
```

`SessionStart.source` vocabulary is the `SessionStartActivationSourceV1` enum:
`startup | resume | clear | fork`.
`Notification` carries `notification_type` (observed value `permission_prompt`, message
`Muse Code needs approval`).

---

## 5. Hook output protocol

### 5.1 Exit codes — PROVEN

| exit | meaning |
|---|---|
| `0` | success; stdout is parsed (see below) |
| `1` | non-blocking error; run continues normally |
| `2` | **blocking**; stderr is the block reason. Run terminated (`run ended with Cancelled` on `UserPromptSubmit`) |

```
########## UserPromptSubmit exit 2 with stderr
run ended with Cancelled
########## UserPromptSubmit exit 1 with stderr
echo: hi
```

### 5.2 stdout — PROVEN

* Non-JSON stdout on `UserPromptSubmit` (and any event whose `additionalContext` is allowed) is
  injected verbatim as additional context. On other events it is ignored.
* A JSON **array** → `hook output must be a JSON object` (`status: failed`).
* Empty stdout → `completed`, no effect.
* Ceiling 16384 bytes (see §3.3).
* Malformed-JSON-looking output: `malformed_output: hook stdout started as JSON but did not parse: …`

The JSON object envelope:

```jsonc
{
  "continue": false,               // stop the whole run (only on some events, see 5.4)
  "stopReason": "<string>",         // paired with continue:false
  "systemMessage": "<string>",      // surfaced to the user; lands in the terminal record
  "suppressOutput": true,           // accepted, no observable effect
  "decision": "block",              // legacy Claude-style verdict
  "reason": "<string>",             // required when decision:block
  "hookSpecificOutput": {
    "hookEventName": "<must equal the firing event>",
    "additionalContext": "<string>",
    "permissionDecision": "allow" | "deny" | "ask",   // PreToolUse
    "permissionDecisionReason": "<string>",
    "updatedInput": { … },                             // PreToolUse allow
    "decision": "…" | { "behavior": "allow" | "deny", "message": "…" },  // PermissionRequest
    "selectedSkills": [ {"id":"…","path":"…","description":"…"} ]        // skills.v1 capability
  }
}
```

### 5.3 PreToolUse verdict matrix — PROVEN via `muse plugins hook test`

Harness: a plugin hook whose script `cat`s a file I rewrite between runs.

```
### allow without updatedInput
   TER: failed  err=PreToolUse `permissionDecision: allow` requires `updatedInput`; a bare allow is rejected
### allow with updatedInput
   DEC: {"updated_input": {"command": "echo rewritten"}}
   TER: completed eff=['rewrite_selected']
### deny without reason
   TER: failed  err=PreToolUse `permissionDecision: deny` requires a non-empty `permissionDecisionReason`
### deny with reason           (earlier run)
   DEC: {"should_block": true, "block_reason": "denied by plugin hook", "permission_decision": "deny"}
   TER: blocked eff=['blocked','permission_denied']
### ask
   TER: completed eff=[]
### bogus decision
   TER: failed  err=unsupported `permissionDecision` value in PreToolUse output
### reason without decision
   TER: failed  err=PreToolUse output set `permissionDecisionReason` without a `permissionDecision`
### wrong hookEventName
   TER: failed  err=`hookSpecificOutput.hookEventName` must be `PreToolUse`
### missing hookEventName
   TER: failed  err=`hookSpecificOutput` is missing the required `hookEventName` (expected `PreToolUse`)
### legacy decision approve
   TER: failed  err=unsupported legacy PreToolUse output; use `hookSpecificOutput.permissionDecision`
### legacy decision block
   DEC: {"should_block": true, "block_reason": "nope"}   TER: blocked eff=['blocked']
### decision block no reason
   TER: failed  err=hook output with `decision: block` requires a non-empty `reason`
### continue false
   TER: failed  err=unsupported `continue` in output of PreToolUse hook output
### systemMessage
   TER: completed sysmsg='hello operator'
### additionalContext
   DEC: {"additional_contexts": ["ctx here"]}   TER: completed eff=['context']
### not json          -> completed, no effect
### json array        -> failed  err=hook output must be a JSON object
### empty output      -> completed, no effect
```

**Yes: a PreToolUse hook can rewrite a tool call.** `permissionDecision:"allow"` is *only* legal
when accompanied by `updatedInput`; the result is decision `updated_input` with effect
`rewrite_selected`. A bare "allow" is deliberately rejected so a hook cannot silently
rubber-stamp a call. There is also an effect `rewrite_ignored` and the message
`supersedes the ignored earlier rewrite from hook `<key>`` — later rewrites win, earlier ones are
recorded as ignored.

### 5.4 Per-event capability matrix — PROVEN

One plugin with one hook per event; each row is three real `muse plugins hook test` invocations.

| event | `decision:"block"` | `continue:false` | `hookSpecificOutput.additionalContext` |
|---|---|---|---|
| SessionStart | ✗ `unsupported decision` | ✓ `blocked/['stopped']` | ✓ `['context']` |
| UserPromptSubmit | ✓ `blocked/['blocked']` | — (accepted, no effect) | ✓ `['context']` |
| PreToolUse | ✓ `blocked/['blocked']` | ✗ unsupported | ✓ `['context']` |
| PermissionRequest | ✓ `blocked/['blocked']` | ✗ unsupported | ✗ unsupported |
| PostToolUse | ✓ `blocked/['blocked','feedback']` | ✓ `blocked/['stopped','feedback']` | ✓ `['context']` |
| PreLLMCall | ✓ `blocked/['blocked']` | — | ✓ `['context']` |
| PostLLMCall | ✓ `blocked/['blocked']` | — | ✓ `['context']` |
| PreCompact | ✗ unsupported | ✓ `blocked/['stopped']` | ✗ unsupported |
| PostCompact | ✗ unsupported | — | ✗ unsupported |
| SubagentStart | ✗ unsupported | — | ✓ `['context']` |
| SubagentStop | ✓ `blocked/['blocked']` | — | ✗ unsupported |
| Stop | ✓ `blocked/['blocked']` | ✓ `blocked/['stopped']` | ✗ unsupported |
| SessionEnd | ✗ unsupported | — | ✗ unsupported |
| Notification | ✗ unsupported | ✗ unsupported | ✗ unsupported |
| PostToolUseFailure | `completed/['feedback']` | ✓ `blocked/['stopped']` | ✓ `['context']` |
| StopFailure | ✗ unsupported | — | ✗ unsupported |
| PostToolBatch | ✓ `blocked/['blocked']` | ✓ `blocked/['stopped']` | ✓ `['context']` |

("—" = accepted and silently produced no effect. "✗ unsupported" = `status:failed` with
`unsupported <field> in ... of <Event> hook output`.)

Runtime block messages found in the binary confirm the wiring:

```
pre-tool hook blocked tool use
permission hook denied tool use
post-tool hook stopped this turn
post-tool hook stopped before background compaction installed
pre-LLM hook blocked model call
post-LLM hook blocked model result
pre-compact hook vetoed compaction
session start hook stopped this turn
user prompt hook blocked submission
prompt rejected by hook before background compaction installed
run stopped by session start hook before background compaction installed
stop hook requested continuation
subagent stop hook requested continuation
tool blocked by hook: <reason>
tool denied by hook: <reason>
allow:hook
```

### 5.5 PermissionRequest verdict shape — PROVEN

```
  decision="allow" (bare string)        failed  err=unsupported `decision.behavior` value in PermissionRequest output
  decision={"behavior":"allow"}         completed eff=['permission_allowed'] dec={"permission_decision":"allow"}
  decision={"behavior":"deny","message":"no"}
                                        blocked   eff=['blocked','permission_denied']
                                        dec={"should_block":true,"block_reason":"no","permission_decision":"deny"}
  decision={"behavior":"ask"}           failed  err=unsupported `decision.behavior` value in PermissionRequest output
  decision={"behavior":"allow","updatedInput":{…}}
                                        failed  err=unsupported `hookSpecificOutput.decision.updatedInput` in PermissionRequest hook output
  decision={"behavior":"allow","zzz":1} failed  err=unsupported `hookSpecificOutput.decision.zzz` in PermissionRequest hook output
```

So `PermissionRequest` accepts **only** `{"behavior":"allow"}` or `{"behavior":"deny","message":…}`.
The session-start security context block advertises this:
`Same access as Ask me; AI reviews eligible actions; trusted permission hooks may decide approval requests first.`

### 5.6 PostToolUse — PROVEN

```
  updatedMCPToolOutput   failed  err=unsupported `updatedMCPToolOutput` in hookSpecificOutput of PostToolUse hook output
  additionalContext      completed eff=['context']
  reason w/o decision    failed  err=PostToolUse output set `reason` without `decision: block`
  decision block         blocked eff=['blocked','feedback']
                         dec={"should_block":true,"block_reason":"bad output","feedback_message":"bad output"}
```

`PostToolUse output cannot replace tool output` and `updatedMCPToolOutput` exist as strings but the
field is rejected: **a PostToolUse hook cannot rewrite a tool result in 1.0.1**, it can only block
with feedback or add context.

### 5.7 `additionalContext` delivery — PROVEN

Injected as a **developer-role context block** in the `runtime_hook` lane:

```json
{"kind":"context_block_updated","id":"hook:user_prompt_submit:prompt:0","role":"developer",
 "source":"runtime_hook","lifecycle":"user_prompt_submit","order":900000,
 "text":"EXTRA-CTX-MARKER","reason":"hook:user_prompt_submit"}
```

with the catalog diagnostic

```
context block `hook:user_prompt_submit:prompt:0` from source `runtime_hook` matched catalog source
`runtime_hook` lane=context_block lifecycle=hook_event order=900000 cache_class=dynamic_turn status=supported
```

Block-id grammar: `hook:<snake_event>:<scope>:<index>` where scope ∈ {`session`, `prompt`, `stop`, …}.

### 5.8 `skills.v1` output capability (feature-gated) — PROVEN

Gate: `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1` (+ `..._APPLY=1`); gate keys in the registry are
`hook_selected_skills` / `hook_selected_skills_apply`.

Handler must declare `"outputCapabilities": ["skills.v1"]` and be a foreground
`UserPromptSubmit` or `PostToolUse` command hook. Output:

```json
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit",
 "selectedSkills":[{"id":"mytest","path":"<abs path inside project>","description":"…"}]}}
```

Validation errors observed:

```
path="…/ws/.muse/skills/mytest/SKILL.md"  -> completed, skill injected
path=".muse/skills/mytest/SKILL.md"       -> failed: selected_skills:rejected:invalid-path
path="<bundled skills dir>/plan/SKILL.md" -> failed: selected_skills:rejected:path-outside-project
(no gate)                                 -> completed, output ignored
```

Other strings: `selectedSkills must be an array`, `selectedSkills entries must be objects`,
`selectedSkills entries must contain exactly id, path, and description`.

The injected block (verbatim from the session log):

```
<system-reminder source="selected-skills">
Muse Code loaded authenticated Project-scope skills selected for this run. These are summaries only; use read_skill with the exact id or absolute path.

<skill-catalog source="selected_skills_catalog">
<skill id="mytest" scope="project" path="…/ws/.muse/skills/mytest/SKILL.md"
       hook-source="project"
       hook-handler="…/ws/.muse/hooks.json:user_prompt_submit:def-b3f05f2c6e5845b9"
       hook-source-family="project"
       hook-source-digest="sha256:b3f05f2c6e5845b97e7261d0d90cea690c04202e1cac011a85ee20f570cbbb57"
       hook-configured-order="0">
<description>Do the test thing</description>
</skill>
</skill-catalog>
</system-reminder>
```

Note `def-b3f05f2c6e5845b9` = `def-` + the first 16 hex chars of the handler's source digest.

---

## 6. Stop-hook continuation cap — PROVEN

`Stop` (and `SubagentStop`) blocking makes the agent continue. The cap is
`settings.max_consecutive_stop_hook_continuations`.

```
  max_consecutive_stop_hook_continuations=default -> assistant turns=9   => default cap is 8
  max_consecutive_stop_hook_continuations=0       -> assistant turns=1
  max_consecutive_stop_hook_continuations=1       -> assistant turns=2
  max_consecutive_stop_hook_continuations=2       -> assistant turns=3
  max_consecutive_stop_hook_continuations=5       -> assistant turns=6
```

When the cap is hit:

```json
{"kind":"context_block_diagnostic","block_id":"hook:stop:continuation-cap","source":"runtime_hook",
 "lifecycle":"stop","action":"observed","reason":"stop_hook_continuation_cap_reached",
 "message":"Stop hook continuation limit reached; ignoring the continuation and completing normally."}
```

---

## 7. Observability: hook records in the session log

`~/.local/share/muse/sessions/YYYY/MM/DD/<session-uuid>/session.jsonl`

Three record kinds (`AgentRunEvent` payload types `agent.hook.run_started`,
`agent.hook.run_terminal`, `agent.hook.decision_applied`):

```json
{"kind":"hook_run_started","run_id":"<hook_key>:<n>","hook_key":"<config-path|plugin:id>:<snake_event>:<handler-id>",
 "event":"UserPromptSubmit","display_order":0,"status_message":null,
 "origin":{"plugin_id":"allev","capability_id":"sessionstart","source_digest":"sha256:…"}}

{"kind":"hook_run_terminal","run_id":"…","hook_key":"…","event":"UserPromptSubmit","display_order":0,
 "status":"completed","duration_ms":6,"exit_code":0,"stdout":"","stderr":"","error":null,
 "system_message":null,"effects":["context"]}

{"kind":"hook_decision_applied", …}   // "hook decision applied (legacy)" in the UI
```

Enums recovered from the binary:

```
HookRunStatus       = completed | blocked | failed | timed_out | cancelled
HookEffectCategory  = blocked | stopped | context | rewrite_selected | rewrite_ignored
                    | permission_allowed | permission_denied | feedback
HookRunOrigin       = { plugin_id, capability_id, source_digest }
```

`hook_key` grammar:

* config-file hook: `<absolute path of the declaring file>:<snake_event>:def-<16 hex of source digest>`
  e.g. `…/ws/.muse/hooks.json:session_start:def-9b467eafbd569d44`
* plugin hook: `plugin:<plugin-id>:<capability-id>`

Telemetry note found in the binary (`INV-014`):

> `HookRunTerminal.origin.plugin_id` — `Omitted when the hook has no plugin origin. Hook rows never
> carry stdin/stdout/stderr content (INV-014). User-installed code in the loop: how often hooks
> block, fail, stall, or are cancelled`
> `run AgentRunEvent::HookRunTerminal (session mirror)` … `hook.event` … `PostToolUseFailure, STAGED …
> production emits no hook_run row for this value until downstream acceptance is recorded on #7221,
> INV-012. `StopFailure` and `PostToolBatch` are …`

(The local **session log** does carry stdout — I read it. The redaction rule applies to the
telemetry mirror.)

Owner ticket references in the binary: `runtime_hook … hook_context … hooks owner (#3057/#3177)`.

**Diagnostics for malformed hook configs are TUI-only.** A syntactically broken
`.muse/hooks.json`, an unsupported event name, a bad handler — all are silently dropped in
`muse exec`, `muse skills list`, `muse config status`. No CLI surface prints `malformed hook
config at …` or the `D63/D65/D66/D96–D101` texts. (Those texts *are* emitted through
`muse plugins validate --json` for plugin hooks; see §8.)

---

## 8. Plugin hooks

### 8.1 Native plugin manifest — PROVEN

`<plugin>/.muse-plugin/plugin.json`:

```json
{
  "schemaVersion": 1,
  "name": "hooktest",
  "displayName": "Hook Test",
  "version": "0.1.0",
  "description": "Native hook capability.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "skills": [], "commands": [],
    "hooks": [
      { "id": "pre-check",
        "event": "PreToolUse",
        "command": ["sh", "hooks/pre-check.sh"],
        "timeoutMs": 5000,
        "statusMessage": "Checking plugin policy" }
    ],
    "mcpServers": [], "reminders": []
  }
}
```

Native plugin hooks key on **`event`**, not `matcher`. From the binary:

> `hook capability field `matcher` is a Claude/Codex hook field; TBH plugin hooks key on `event`;
> matcher aliasing is tracked separately`

Contract text shipped inside the binary (bundled skill
`skills/create-plugin/references/native-plugin-contract.md`):

> ### Hooks
> ```json
> { "id":"pre-check", "event":"PreToolUse", "command":["sh","hooks/pre-check.sh"],
>   "timeoutMs":1000, "statusMessage":"Checking plugin policy" }
> ```
> The command is structured argv, not a shell string. If an argv element names a relative source
> path, that regular file must exist beneath the plugin root. Hook source paths cannot be shared by
> two hook IDs.

Normalized capability record from `muse plugins validate --json`:

```json
{"id":"pre-check","event":"PreToolUse","matcher":null,
 "command":["sh","hooks/pre-check.sh"],"shell_command":null,
 "source_path":"…/hooks/pre-check.sh","source_relative_path":"hooks/pre-check.sh",
 "timeout_ms":5000,"async":false,"compatibility_name":null}
```

### 8.2 Plugin hooks require explicit review — PROVEN

```
$ muse plugins install <path> --scope user --json
"warning": "third-party plugin: hooks require review before activation"
```

Before approval the hooks do **not** run in a live session (0 `hook_run_terminal` rows).
After `muse plugins approve allev`:

```json
{"decision":"approve","runtime_capabilities":[
  {"stable_id":"plugin:allev:hook:sessionstart",
   "trusted_definition_hash":"sha256:2977ea4a320d4a328844a4f1c334cd831c3794b9cc4630219fb43f6090c1c0b2",
   "enabled":true}, …]}
```

which is persisted into `$CONFIG_DIR/muse/settings.json`:

```json
{"schema_version":1,
 "runtime_capabilities":{
   "plugin:allev:hook:sessionstart":{"enabled":true,
     "trusted_definition_hash":"sha256:2977ea4a…"}, …}}
```

`HookStateConfig` in the binary is `{ enabled, trusted_hash }`; the settings-file variant is
`RuntimeCapabilityState { enabled, trusted_definition_hash, trusted_blocking_definition_hash }`.
Stable-id grammar: `plugin:<plugin-id>:hook:<capability-id>`
(`muse plugins approve` also accepts `<plugin-id>:hook:<cap>`, `<plugin-id>:<cap>`, or the whole
`<plugin-id>`). `muse plugins reject` writes `enabled:false`.

After approval, plugin hooks fire and carry `origin`:

```json
{"kind":"hook_run_terminal","run_id":"plugin:allev:sessionstart:1","hook_key":"plugin:allev:sessionstart",
 "event":"SessionStart","display_order":100011,
 "origin":{"plugin_id":"allev","capability_id":"sessionstart","source_digest":"sha256:704ca81a…"},
 "status":"completed","duration_ms":10,"exit_code":0,"stdout":"","stderr":"","error":null}
```

Editing the cached package after install invalidates the digest and disables the hook:
`plugin hook `hooktest:pre-check` is not installed and enabled`.

### 8.3 `muse plugins hook test` — the offline hook harness — PROVEN

```
usage: muse plugins hook test <plugin-id>:<hook-id> | plugin:<plugin-id>:hook:<hook-id>
                              --fixture <path> [--json]
```

Fixture schema (keys `event`, `stdin`, `matcher_input`, `cwd`; `stdin` required):

```json
{ "event": "PreToolUse",
  "stdin": "{\"hook_event_name\":\"PreToolUse\",\"tool_name\":\"bash\",\"tool_input\":{\"command\":\"ls\"}}",
  "matcher_input": "bash",
  "cwd": "/tmp" }
```

Errors: `fixture must contain \`stdin\``, `fixture event must be \`PreToolUse\` or \`pre_tool_use\``.

Result — this is the **internal hook decision struct**:

```json
{
  "decision": {
    "should_block": true,
    "block_reason": "denied by plugin hook",
    "additional_contexts": [],
    "updated_input": null,
    "permission_decision": "deny",
    "feedback_message": null
  },
  "terminals": [ { "run_id":"plugin:hooktest:pre-check:1", "hook_key":"plugin:hooktest:pre-check",
                   "event":"pre_tool_use", "status_message":"Checking plugin policy",
                   "system_message":null, "status":"blocked", "duration_ms":11, "exit_code":0,
                   "effects":["blocked","permission_denied"], "stdout":"…", "stderr":"", "error":null } ],
  "records": 2
}
```

(The internal field names also appear as a literal run in the binary:
`should_blockblock_reasonadditional_contextsupdated_inputpermission_decision`.)

### 8.4 Foreign (Claude / Codex) plugin hooks — PROVEN

`<plugin>/.claude-plugin/plugin.json` with `"hooks": "hooks/hooks.json"`
(manifest `hooks` must be an object, a path, or a path array). The referenced file uses the
Claude Code shape (`{"hooks": {"<Event>": [{"matcher": …, "hooks": [...]}]}}`).

`muse plugins validate --json` emits the full compat diagnostic catalogue:

```json
{"code":"unsupported-field","severity":"warning",
 "message":"foreign hook handler field `silent` is recognized; Muse ignores Claude display suppression"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `once` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `asyncRewake` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `rewakeMessage` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `rewakeSummary` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `shell` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `commandWindows` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `args` is unsupported"}
{"code":"unsupported-field","severity":"error","message":"foreign hook handler field `url` is unsupported"}
{"code":"unsupported-capability","severity":"error","message":"foreign hook handler type `webhook` is unsupported"}
{"code":"invalid-manifest-schema","severity":"error","message":"foreign hook command must be a string"}
{"code":"unsupported-capability","severity":"warning",
 "message":"conditional hook `hook-68f290d92d175244` is inactive: selector_unsupported_tool"}
{"code":"unsupported-capability","severity":"warning",
 "message":"Claude hook event `Setup` is recognized but is not run by Muse; complete any required plugin setup manually"}
{"code":"unsupported-hook-event","severity":"error","message":"foreign hook event `Bogus` is unsupported"}
```

Foreign hooks **do** keep `matcher` (native ones do not) and get a synthetic id `hook-<16 hex>`:

```json
{"id":"hook-e2f5cb275ef2afbf","event":"PreToolUse","matcher":"Bash",
 "command":["echo ok"],"shell_command":"echo ok","timeout_ms":10000,"async":false,
 "compatibility_name":null}
```

Note `timeout: 10` (Claude seconds) → `timeout_ms: 10000`. Matchers are stored verbatim
(`"Bash"`, `"Read|Write"`, `"*"`, `"mcp__server__tool"`, even unknown names).

The `if` conditional selector is supported for **exactly one selector shape**: the Claude tool
name `Bash`, optionally with an argument glob. Everything else deactivates the hook —

```
'Bash'                 valid=True
'Bash(git*)'           valid=True
'Bash(*)'              valid=True
'bash' / 'read_file' / 'true' / '$FOO' / 'tool == "bash"' / 'Bash|Read' / 'mcp__x__y' / ' Bash '
                       -> conditional hook `hook-…` is inactive: selector_unsupported_tool
''                     -> conditional hook `hook-…` is inactive: selector_empty
```

Selector reason vocabulary in the binary:
`selector_empty | selector_malformed | selector_unsupported_tool | selector_unsupported_event`,
plus `conditional hook selector is unsupported or malformed`.

Other foreign strings:

```
foreign hook group must contain a `hooks` array
foreign hook group must be an object
foreign hook group field `description` must be a string
foreign hook matcher must be a string       /  foreign hook matcher is invalid: <x>
foreign hook handler must be an object      /  foreign hook handler must declare string field `type`
foreign hook handler field `description` must be a string
foreign hook command must not be empty      /  foreign hook command must be a string
foreign hook timeout must be a non-negative integer   /  foreign hook timeout is too large
foreign hook async must be a boolean
plugin hook source must contain an object field `hooks`
plugin hook source escapes plugin root      /  plugin hook source is not valid JSON: <e>
duplicate foreign hook identity `<id>`
plugin hooks must be an object
`<name>` collides with a built-in tool matcher name; conflicting plugin hook omitted
`<name>`; conflicting alias hook omitted    /   `<name>`; conflicting plugin hook omitted
```

Plugin capability families (from the bundled `create-plugin` skill):
`skills`, `commands`, `hooks`, `mcpServers`, `reminders` are supported;
`tools`, `agents`, `outputStyles`, `settings`, `apps` are rejected.
Reserved plugin ids: `loop`, `muse-core`, `tbh-reminders`.

---

## 9. The enterprise `defaults` vs `policy` split — PROVEN

`muse config` is the only enterprise surface:

```
Usage: muse config validate --plane <defaults|policy> --file <path>
       muse config status
```

`muse config status` on a clean machine:

```
Enterprise configuration status
Generation: sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4
Sources:
  plane=defaults source_class=system_file state=absent
  plane=policy source_class=system_file state=absent
  plane=defaults source_class=macos_managed_preferences state=absent
  plane=policy source_class=macos_managed_preferences state=absent
```

Source classes: `system_file`, `macos_managed_preferences`, `windows_machine_policy`.
Source states: `absent | valid | unreadable | untrusted | malformed | unsupported | invalid | stale`.
Document filenames: `enterprise-defaults.json`, `enterprise-policy.json`
(macOS managed-preferences domain `com.tbh.tbh`, keys `enterprise_defaults_json` /
`enterprise_policy_json`). The system root is under `Library/Application Support/Muse/` —
**INFERRED**, not written to.

### 9.1 Schema version

```
{}                     -> enterprise_document_invalid: plane=defaults reason=missing_schema_version location=schema_version
{"schema_version":1}   -> valid: plane=defaults schema_version=1
{"schema_version":1}   -> valid: plane=policy   schema_version=1
{"schema_version":2}   -> enterprise_schema_unsupported: plane=defaults reason=unsupported_schema_version location=schema_version
```

**Enterprise document schema_version = 1** for both planes.

### 9.2 Top-level members of each plane — PROVEN by brute force

```
===== plane=defaults
  settings -> valid
===== plane=policy
  extensions              -> valid
  execution               -> valid
  privacy                 -> valid
  model_egress            -> valid
  local_session_messaging -> valid
```

So the split is: **`defaults` = "what the product's settings start as"** (a lower-priority
`SettingsFile`), **`policy` = "hard ceilings and prohibitions"** (a separate vocabulary).

### 9.3 What `defaults` can say about hooks

`settings.hooks` is **NOT** a member of the defaults plane:

```
{"schema_version":1,"settings":{"hooks":{}}} -> enterprise_document_invalid: plane=defaults reason=unknown_member
```

**Enterprise defaults cannot ship hook definitions.** What it *can* set (accepted members found):

```
settings.agents  settings.run  settings.tui  settings.context  settings.context_compaction
settings.provider_retry  settings.model  settings.provider  settings.reasoning_effort
settings.skills  settings.telemetry  settings.notifications  settings.mcp_servers  settings.presets
settings.endpoint_transport  settings.feature_config  settings.local_session_messaging
settings.first_turn_minimal_effort_regex
settings.max_consecutive_stop_hook_continuations      <-- the one hook-relevant knob
```

```
settings.max_consecutive_stop_hook_continuations = 1  -> valid: plane=defaults schema_version=1
settings.max_consecutive_stop_hook_continuations = "x"-> wrong_type
```

### 9.4 What `policy` says about hooks — and why it is inert today

The policy plane *does* declare hook-relevant fields, but they are **declared-but-not-activated**
in this build. `field_not_activated` is a distinct rejection reason from `unknown_member`:

```
extensions.hooks                  -> field_not_activated  (member names allowed:
                                       allowed_identities, denied_identities,
                                       allowed_sources, allowed_digests — all arrays;
                                       anything else -> unknown_member)
extensions.skills                 -> field_not_activated
extensions.runtime_capabilities   -> field_not_activated  (member: allowed_kinds)
extensions.mcp_servers/plugins/…  -> unknown_member
execution.forbid_approval_bypass  -> field_not_activated
execution.forbid_sandbox_bypass   -> field_not_activated
execution.permission_profiles     -> field_not_activated
execution.approval_reviewers      -> field_not_activated
execution.allow_project_configuration  -> field_not_activated
execution.allow_foreign_configuration  -> field_not_activated
execution.force_agent_definition_safe_mode -> field_not_activated
privacy.telemetry / privacy.feature_config / privacy.local_session_messaging /
privacy.foreign_personal_rules    -> field_not_activated
execution.tool_rules              -> {} is VALID (only activated container found)
execution.stop_hook_continuations -> shape-valid (member: maximum:int) but EVERY value
                                     0,1,2,5,8,9,10,16,32,64,100,1000 -> semantic_invalid
execution.approval_modes / network_sandbox_modes -> {"allowed":[…]} -> semantic_invalid
```

So, to answer the brief precisely:

* **The split is real and provable.** `defaults` and `policy` are two different documents with two
  different schemas, validated by two different code paths (`muse config validate --plane`), each
  reported separately by `muse config status`
  (`source.defaults.document.present`, `source.defaults.document.schema_version`,
  `source.defaults.settings`, `source.policy.document.present`,
  `source.policy.document.schema_version`, `source.policy.document.empty`).
* **Enterprise policy is *designed* to constrain hooks** — `extensions.hooks` with
  `allowed_identities` / `denied_identities` / `allowed_sources` / `allowed_digests`, i.e. an
  allow/deny list over hook stable-ids, source tiers and content digests, matching the
  `runtime_capabilities` trust records. `execution.stop_hook_continuations.maximum` is the policy
  ceiling for the Stop-hook loop.
* **But in 1.0.1-R2006.1 none of it is switched on.** Enterprise policy **cannot force or forbid
  hooks today**; enterprise **defaults** can only pre-set
  `max_consecutive_stop_hook_continuations`. The forcing story is forward-declared.

`MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG=1` does not change any of the above (retested).

---

## 10. Complete string catalogue (verbatim, for cross-reference)

Config-file parsing:

```
hooks must be an object
hook matcher group must declare `hooks`
hook matcher group must be an object, found
hook matcher group `hooks` must be an array, found
hook matcher must be a string, found
`<m>` is unmatchable; drop this group's matcher
unsupported hook event `
hook event `<e>` must be an array
hook handler must be an object, found
hook handler must declare a `type`
hook handler field `<f>`
command hook command must not be emptya non-negative integer
D63: this handler declares only a Windows command form (`commandWindows`), which is selected on
     Windows only, so it has no command to run on this platform
D65: `silent` is display-only compatibility metadata and is ignored
D66: unknown matcher-group field `<f>` may narrow execution, so this group is skipped
D66: unknown handler field `<f>` may narrow execution, so this handler is skipped
D66: a conditional handler is unsupported; TBH cannot prove the condition
D96: `once: true` one-shot handlers are unsupported
D97: `asyncRewake: true` handlers are unsupported
D97: model reawakening is unsupported
D98: a per-handler `shell` selector is unsupported
D99: Claude `command` plus `args` argv execution is not implemented yet (#10133)
D101: hook handler type `<t>` is unsupported
 hook content cannot declare User-tier field `managed_hooks_env_vars`
 hook file cannot declare User-tier field `managed_hooks_env_vars`
malformed hook config: / malformed hook config at
failed to read hook config: / failed to read hook config at
invalid managed_hooks_env_vars[<i>]: name must match [A-Za-z_][A-Za-z0-9_]*
```

Output parsing:

```
hook output must be a JSON object
`hookSpecificOutput` must be a JSON object
`hookSpecificOutput.decision` must be a string or a JSON object
`hookSpecificOutput` is missing the required `hookEventName` (expected `<E>`)
`hookSpecificOutput.hookEventName` must be `<E>`
hook output with `decision: block` requires a non-empty `reason`
`decision: block` is unsupported for this hook event
unsupported `<field>` in <E> hook output
unsupported `<field>` in hookSpecificOutput of <E> hook output
unsupported `hookSpecificOutput.decision.<k>` in PermissionRequest hook output
unsupported `decision.behavior` value in PermissionRequest output
PreToolUse `permissionDecision: deny` requires a non-empty `permissionDecisionReason`
PreToolUse `permissionDecision: allow` requires `updatedInput`; a bare allow is rejected
PreToolUse output set `permissionDecisionReason` without a `permissionDecision`
unsupported legacy PreToolUse output; use `hookSpecificOutput.permissionDecision`
unsupported `permissionDecision` value in PreToolUse output
PostToolUse output cannot replace tool output
PostToolUse `decision: block` requires a non-empty `reason`
PostToolUse output set `reason` without `decision: block`
PostToolUse hook stopped execution
PermissionRequest hook denied approval
selectedSkills must be an array
selectedSkills entries must be objects
selectedSkills entries must contain exactly id, path, and description
selectedSkills.<f> must be a string / must not be empty
skills.v1 is supported only for foreground UserPromptSubmit or PostToolUse command hooks
outputCapabilities must be exactly ["skills.v1"]
output_too_large: hook <...>
malformed_output: hook stdout started as JSON but did not parse:
hook timed out after
hook selector skipped / muse: hook selector skipped: handler=
supersedes the ignored earlier rewrite from hook `
tool blocked by hook: / tool denied by hook:
see hook logs
```

Handler serde field-name run (one struct, adjacent literals):

```
commandcommandWindowscommand_windowstimeoutstatusMessageasyncasyncRewakerewakeMessage
rewakeSummaryshellconditionifsilentoutputCapabilities
```

Normalized (internal) handler struct:

```
matcher output_capabilities command shell_command timeout_ms status_message env source_digest duty_sha
```

`SettingsFile` field-name run (note the three hook members):

```
schema_version agent_definitions first_turn_minimal_effort_regex context_compaction provider_retry
tui context local_session_messaging feature_config skills model_catalog mcpServers mcp_servers
presets hooks runtime_capabilities permissions plugins managed_hooks_path managed_hooks_env_vars
max_consecutive_stop_hook_continuations endpoint_transport telemetry notifications
```

Enterprise policy field-name run (note `extensions.hooks` and `stop_hook_continuations`):

```
… extensions hooks runtime_capabilities allowed_identities denied_identities allowed_sources
allowed_digests allowed_kinds … execution forbid_approval_bypass forbid_sandbox_bypass
force_agent_definition_safe_mode permission_profiles approval_modes approval_reviewers
network_sandbox_modes tool_rules allow_project_configuration allow_foreign_configuration
stop_hook_continuations allowed maximum …
```

Feature gates relevant to hooks (`gate_registry`):

```
hook_selected_skills  hook_selected_skills_apply  plugins  enterprise_config
```

---

## 11. Open questions

1. Live `PreToolUse`/`PostToolUse`/`PermissionRequest`/`Notification`/`PreCompact` payloads could not
   be captured end-to-end: the offline `echo` provider never emits tool calls, and authenticating to
   Meta's API was out of scope. Their field lists come from the `.rodata` literal run and from the
   `plugins hook test` harness.
2. `matcher` semantics for tool events: is it a regex (Claude-style `Read|Write`) or a literal?
   Foreign manifests store `"Read|Write"` verbatim, and the native path matched `startup` and `*`
   on `SessionStart`, but the regex path was never exercised against a real tool name.
3. `SessionEnd.reason` vocabulary — only `"other"` observed (headless exit).
4. `Notification` trigger conditions and full payload (`notification_type` values beyond
   `permission_prompt`).
5. Whether `MUSE_TOOL_USE_ID` is exported into `PreToolUse`/`PostToolUse` hook processes.
6. Why `execution.stop_hook_continuations.maximum` returns `semantic_invalid` for every integer —
   whether an accompanying activation/declaration record (`EnterpriseCompositionDeclarations`,
   `EnabledDeclaration`) exists that `muse config validate` cannot synthesize locally.
7. The exact filesystem path of the enterprise system documents (`Library/Application Support/Muse/`
   is the only matching prefix in the binary; not verified because writing there is out of scope).
8. `hook_decision_applied` / `LegacyHookDecisionApplied` record shape — never emitted in my runs.
9. Whether the TUI exposes any hook-management surface (no `/hooks` command string was found; the
   plugin manager shows `skills=0 commands=0 hooks=0 mcp=0 reminders=0` and
   `Hooks and MCP servers load only at session startup.`).

---

# Verification

Adversarial re-run of every claim in a fresh sandbox
(`<scratch>/sandbox/verify-hooks`, `HOME=$V/home`, `W=$V/ws`, `MUSE_NO_AUTO_UPDATE=1`).
Verdict: **MOSTLY_SOLID**. The runtime behaviour of the hooks system reproduced almost
perfectly — the discovery matrix, the 4 tiers and their ordering, the 17-variant enum, the
live payloads, exit codes, the 16 KiB ceiling, the PreToolUse rewrite seam, the
PermissionRequest verdict shape, the full 17×3 capability matrix, the Stop continuation cap,
`managed_hooks_env_vars`, plugin approval/digest, and every enterprise `config validate`
result all came out byte-identical. Six claims are refuted or materially wrong, and the
INFERRED payload table (§4.3 tail) rests on a string-adjacency method that this pass proved
unreliable.

## R1 — REFUTED: `PreCompact{reason}` (and the method behind the whole inferred field table)

The report attributes `reason` to **both** `PreCompact` and `SessionEnd` from a single
occurrence of one literal. The actual `.rodata` run (`strings -a -n 6`, line 58493-58495) is:

```
…stop hook requested continuationPreCompact
SessionEnd
reasonPostCompactSessionStartpromptSubagentStartsubagent_idSubagentStop…
```

`reason` follows **`SessionEnd`**, not `PreCompact`. The second, more-deduplicated occurrence
(line 93859) is `…PreCompactSessionEndPostCompact` with **no `reason` at all**. And
`SessionEnd.reason` is independently PROVEN live (`{"hook_event_name":"SessionEnd","reason":"other",…}`).

Worse, the same run contains `PostCompactSessionStartprompt`. If "name follows its event" held,
`SessionStart` would carry `prompt` — but the live capture proves `SessionStart` carries
`source` and `prompt` belongs to `UserPromptSubmit`. **The adjacency heuristic is demonstrably
wrong at least once inside the very run used to build the table**, so every row of the
"events that could not be fired live" list is unproven, not merely unverified:

| report claim | status after verification |
|---|---|
| `PreCompact{reason}` | **REFUTED** — `reason` belongs to `SessionEnd`; no `trigger` or `custom_instructions` string exists anywhere in the binary, so `PreCompact`'s payload is simply **unknown** |
| `PreToolUse{tool_name,tool_input,tool_use_id}` | plausible — supported by a *separate* run `hook_event_nametool_inputtool_use_idPreToolUse`, but `tool_name` is only adjacency |
| `PostToolUse{tool_use_id,tool_response}` | plausible — `PostToolUsetool_use_idtool_response` is tightly adjacent |
| `PermissionRequest{tool_name,tool_input}` | plausible but same heuristic |
| `SubagentStart{subagent_id}` | plausible; `subagent_id` sits between `SubagentStart` and `SubagentStop` and could belong to either |
| `Notification{notification_type}` | plausible — `permission_promptnotification_typeMuse Code needs approvalsystemMessage` (line 94620) |

Independent cross-check attempted and failed: `muse schema generate-json-schema --out DIR`
(and `--experimental`) emits `msp.schema.json` + `manifest.json` containing **zero** hook
types — `grep -oh '"[A-Za-z_]*[Hh]ook[A-Za-z_]*"'` over both bundles returns nothing. Hooks
are not on the MSP wire, so there is no second source for these shapes offline. A local mock
provider was attempted to fire tool events for real; it fails at `GET /muse-code/models`
(`Provider returned malformed response data`) and the `ModelCatalogResponse` 5-field shape is
not recoverable from strings, so live tool-event payloads remain uncaptured here too.

## R2 — REFUTED: `"timeout": 0` "runs", and there is no default timeout

The report's handler table says `"timeout":0 | runs`. It does not run normally — **0 is
clamped to a 1-second timeout**:

```
handler {"command":"sleep 3; echo done","timeout":0}
 -> {"status":"timed_out","duration_ms":1005,"exit_code":null,"error":"hook timed out after 1s"}
```

And the report never establishes what happens with `timeout` omitted. There is **no default
timeout at all** — an omitted `timeout` lets a hook block the turn indefinitely:

```
handler {"command":"sleep 3; echo done"}   -> {"status":"completed","duration_ms":3020}
handler {"command":"sleep 15; echo done"}  -> {"status":"completed","duration_ms":15007}
handler {"command":"sleep 3","timeout":60} -> {"status":"completed","duration_ms":3014}
```

This is the single most operationally important correction in this pass: every hook a
framework ships must set an explicit `timeout ≥ 1`, or one hung script hangs the agent forever.

## R3 — REFUTED: "The local session log does carry stdout — I read it" (§7)

Hook stdout is **elided from `hook_run_terminal` unless the run failed**. Same command, two
exit codes:

```
command "echo MYSTDOUT; echo MYSTDERR >&2"          exit 0
 -> {"status":"completed","exit_code":0,"stdout":"","stderr":"MYSTDERR\n","effects":["context"]}
command "echo MYSTDOUT; echo MYSTDERR >&2; exit 1"  exit 1
 -> {"status":"failed","exit_code":1,"stdout":"MYSTDOUT\n","stderr":"MYSTDERR\n"}
```

Blocked runs also elide it (`exit 2` with stdout -> `"stdout":""`, `"stderr":"REASON\n"`,
`effects:["blocked"]`). A 16384-byte stdout that was accepted and turned into a context block
still logged `"stdout":""`. So `session.jsonl` is **not** a place to read what a successful
hook printed — only `stderr` and `error` survive. (`INV-014` in the binary is about the
telemetry mirror; the local rule observed here is status-dependent, not "everything is kept".)

## R4 — REFUTED: "Nothing appeared in local-tracing (grep -ril hook returned nothing)" and "no CLI surface prints them"

`grep -il hook ~/.local/share/muse/local-tracing/bootstrap/*.log` matches many files:

```
INFO tbh.local.catalog …/plugins/src/capability_snapshot/diagnostic.rs:88
  event="plugin_capability_snapshot.compose" outcome="ready" … skills=15 hooks=19 … diagnostics=0
INFO tbh.local.config …/config/src/gate_registry/diagnostics.rs:69
  event="gate.resolve" gate="hook_selected_skills" enabled=false source="default"
INFO tbh.local.config …/gate_registry/diagnostics.rs:69 event="gate.resolve" gate="hook_selected_skills_apply" …
```

And `muse skills list` **does** print a hook-relevant diagnostics block (for the plugin tier):

```
Diagnostics
- plugin-cache-invalid: plugin cache is invalid; capabilities are blocked (plugin://allev)
```

The *narrow* claim survives and was re-confirmed: with `printf '{not json' > .muse/hooks.json`,
neither `muse exec`, `muse exec --json`, `muse skills list`, `muse config status`, nor the
freshly written local-tracing log (dumped in full, 14 lines, zero hook rows) mentions the
config-file failure. **Config-file hook diagnostics (D63/D65/D66/D96–D101, `malformed hook
config at …`) are invisible headlessly; plugin-tier hook diagnostics are not.**

## R5 — REFUTED as presented: the whole `muse plugins` surface is feature-gated

Claims 15, 16, 20, 26 and all of §8 are built on `muse plugins install|approve|validate|hook test`.
In a default build that surface does not exist:

```
$ muse plugins --help
plugins are not available in this build
$ MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins --help
usage: muse plugins <command>  …
```

The bundled `create-plugin` skill (quoted in §8.1) is likewise hidden — it appears in
`muse skills list` only with the gate set. The report lists `plugins` among the gates in §10
but never states that its own primary evidence harness, and the entire plugin declaration
tier's tooling, require `MUSE_EXPERIMENTAL_PLUGINS=1`.

Mitigating (verified): **runtime execution of an already-approved plugin hook needs no gate.**
After `MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins approve plugin:vh:hook:ss`, a plain
`muse exec --provider echo --trust-workspace hi` ran the plugin hook and logged
`display_order: 100000` with `origin:{plugin_id,capability_id,source_digest}`. So the four-tier
model is real at runtime; only its management CLI is gated.

## R6 — REFUTED: "Non-JSON stdout … (and any event whose `additionalContext` is allowed) is injected verbatim"

One handler, `printf '%s' PLAINSTDOUT-MARK`, registered on one event at a time:

```
SessionStart      -> completed effects=["context"]
UserPromptSubmit  -> completed effects=["context"]
Stop              -> completed effects=null
PreLLMCall        -> completed effects=null
PostLLMCall       -> completed effects=null
```

`PreLLMCall` and `PostLLMCall` both *accept* `hookSpecificOutput.additionalContext` (confirmed
in the matrix), yet plain stdout on them produces no context. **Bare-stdout context injection
is limited to `SessionStart` and `UserPromptSubmit`.**

---

## Corrections and sharpened facts (claims that stand, but not as stated)

**Exit codes.** Only `2` blocks. `1`, `3` and `127` are all equally non-blocking and all record
`"status":"failed"`. The block reason is not printed anywhere in `muse exec` — it lands in the
run record as `{"kind":"terminal","terminal":"cancelled","reason":"BLOCKME"}` while stderr
shows only `run ended with Cancelled`. `muse exec` itself exits **rc=1** on a hook block.

**16 KiB ceiling is inclusive.** 16384 bytes -> `completed effects=["context"]`; 16385 bytes ->
`failed / output_too_large: hook stdout exceeded its 16384-byte ceiling…`.

**`TBH_MANAGED_HOOKS_PATH` replaces, not augments, `managed_hooks_path`.** With both set, only
the env-named file fired (`ENVMANAGED.txt` present, `MANAGEDPATH.txt` absent). The report's
table lists them as two independent rows and never states the precedence.

**Which shell runs `command`.** The handler string is executed by **`$SHELL -c`**, not `/bin/sh`:
`argv0=/bin/zsh` by default here; `SHELL=/bin/bash muse exec …` -> `argv0=/bin/bash`;
`SHELL=/bin/sh` -> `/bin/sh`; `SHELL=/nonexistent/shell` -> the hook fails with
`"error":"No such file or directory (os error 2)"`; with `SHELL` unset the interpreter falls
back to `/bin/sh` (while the forwarded `SHELL` var is still repopulated to `/bin/zsh`).
Combined with `D98: a per-handler \`shell\` selector is unsupported`, a distributed hook's
`command` is interpreted by whatever login shell the user happens to have. Corroborating
literal: `SHELLhook_event_nameduplicate_ofsubmitted_surface`.

**No project-directory variable.** `echo "CPD=[$CLAUDE_PROJECT_DIR] MPD=[$MUSE_PROJECT_DIR]
WS=[$MUSE_WORKSPACE_ROOT]"` from a project hook printed `CPD=[] MPD=[] WS=[]`. Claude Code's
`$CLAUDE_PROJECT_DIR` idiom does not port; the only handle on the workspace is `cwd`/`$PWD`.

**Plugin hook environment has 7 injected vars, not 5.** In addition to `MUSE_PLUGIN_ID`,
`MUSE_PLUGIN_ROOT`, `MUSE_PLUGIN_DATA_DIR`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, an
`env|sort` from inside a plugin hook also shows bare **`PLUGIN_ROOT`** and **`PLUGIN_DATA`**.

**`display_order` arithmetic (report open question #10) is fully pinned down.** It is a
**session-wide monotonic counter over admitted handlers in declaration order**, not per-event.
Two events, five handlers -> `SessionStart 0, SessionStart 1, UserPromptSubmit 2, 3, 4, Stop 5`.
A *dropped* handler consumes no slot; an **`async:true` handler consumes a slot but emits no
records**. Handlers `a, bad(unknown field), b(async), c` produced exactly
`ord=0 (run …:1)` and `ord=2 (run …:2)` — index 1 was silently eaten by the async handler.
The `run_id` `:N` suffix is a separate counter over *recorded* runs only.

**`SessionEnd` hooks fire but emit no `hook_run_terminal` row.** A run registering all six
firable events wrote all six marker files, but the session log contained terminals only for
SessionStart/UserPromptSubmit/PreLLMCall/PostLLMCall/Stop (`ord=0..4`). SessionEnd hook
execution is invisible to the session log.

**No deduplication of identical handlers, and `hook_key` is not unique.** Three byte-identical
handlers in one file all ran and all logged the *same*
`…/hooks.json:session_start:def-ef6a2122668037fc`, distinguished only by the `run_id` suffix.
The same handler text in the user tier and the project tier produced two runs with the same
`def-` digest but different file prefixes.

**`skills.v1` needs BOTH gates.** With only `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1`, or only
`…_APPLY=1`, or neither, the session log contains zero `selected-skills` text. Both together
produce the block, which lands as
`id=selected_skills_catalog source=selected_skills_catalog lifecycle=runtime_invocation order=201`
— **not** in the 900000 `runtime_hook` lane — carrying `hook-source`, `hook-handler`,
`hook-source-family`, `hook-source-digest="sha256:28be228d53a42b165…"`, `hook-configured-order`,
and confirming `def-<first 16 hex of the digest>`.

**Handler field list additions.** `"command_windows"` (snake_case) is accepted alongside
`"commandWindows"` (both keep the handler alive on macOS as long as a `command` exists);
`"env"` is **not** a handler field (it drops the handler) despite appearing in the *normalized*
struct run; `"timeout":"5"` (string) drops the handler.

**An unsupported event key drops only that key, not the file.** With
`{"hooks":{"SessionStart":[…canary…],"Bogus":[…]}}` the canary still fired. Same for
`session_start` (snake) — confirming CamelCase-only keys without losing the rest of the file.

**PermissionRequest deny without `message`** still blocks, with a default reason:
`block_reason: "PermissionRequest hook denied approval"`. Also newly confirmed:
`hookSpecificOutput.permissionDecision` is rejected on PermissionRequest
(`unsupported \`permissionDecision\` in hookSpecificOutput of PermissionRequest hook output`) —
the two events use disjoint verdict vocabularies.

**Enterprise plane, extra exclusions.** Beyond `settings.hooks`, the defaults plane also rejects
`settings.managed_hooks_path` and `settings.managed_hooks_env_vars` as `unknown_member`. So
`max_consecutive_stop_hook_continuations` really is the *only* hook-adjacent defaults knob.
`execution.stop_hook_continuations` returns `semantic_invalid` even for `{}` (empty), and
`allowed` is **not** one of its members (`unknown_member`) — §10's field run pairs
`stop_hook_continuations allowed maximum`, but `allowed` belongs to
`approval_modes`/`network_sandbox_modes`.

**Native plugin hook capability shape.** `{}` valid; `matcher` -> the quoted Claude/Codex
rejection; `silent` -> `hook capability field \`silent\` is not supported`; `description` ->
same; `command:"sh hooks/ss.sh"` (string) -> `hook capability \`ss\` must declare a command array`;
`event:"session_start"` -> `event \`session_start\` is unsupported`; **`async:true` is accepted**;
`outputCapabilities:["skills.v1"]` on SessionStart -> `skills.v1 is supported only for foreground
UserPromptSubmit or PostToolUse command hooks`. Bundled contract literal confirmed:
`` - `hooks`: `{id, event, command, timeoutMs?, statusMessage?}` and any relative ``.

**`source_digest` ≠ `trusted_definition_hash`.** The `hook_run_*` `origin.source_digest`
(`sha256:5997a83f…`) is the plugin **package** sha256 from `plugins install`; the
`runtime_capabilities` entry stores a different `trusted_definition_hash` (`sha256:596faad7…`).
The report shows both without distinguishing them.

**Foreign `if` selector: only `Bash`.** Re-swept — `Bash`, `Bash(git*)`, `Bash(*)` valid;
`bash`, `read_file`, `true`, `Bash|Read`, `mcp__x__y` and — newly — **`Read`, `Write`, `Edit`**
all produce `selector_unsupported_tool`; `""` produces `selector_empty`. Every other foreign
diagnostic in §8.4 reproduced verbatim.

---

## Missed ground

### 1. Matcher semantics are fully answerable offline (closes report open question #2)

The report leaves "regex or literal?" open. It is a **hybrid**, and the rule is decisive.
Against `SessionStart` (selector = `source` = `"startup"`), one `muse exec` per matcher:

```
MATCH  : startup  start.*  ^startup$  startu.  .  ..  .......  s.*p  x*  a**
         ^star  tup$  star.*  tar.  (tar)  sta[r]  sta[r]tup  [s]tartup  (?:star)
         star{1}  st(a)r  (?i)startup  (?i)STARTUP  .*  *  ""(absent)
         startup|resume  resume|startup  startup|zzz  zzz|startup|www  startup|  |startup
         (star|zzz)  st.*|zzz  zzz|st.*  .*|zzz
NO MATCH: star  tar  tartup  artup  startupX  STARTUP  "startup "  " startup"
         star$  zzz.  star\b  ........  star|zzz  zzz|star  x|star  star|
         "startup |zzz"  " startup|zzz"  [  (  **  startup(  xyz
```

Consistent model: a `|`-separated list of **word-only** tokens is an exact, case-sensitive
alternation list (`star|zzz` misses, `startup|zzz` hits); any token carrying a regex
metacharacter is compiled and matched **unanchored** (`sta[r]`, `tup$`, `.`, `x*` all hit);
an invalid regex makes the group unmatchable (`[`, `(`, `**`, `startup(`); and the bare string
`*` (plus absent/`""`) is a special-cased wildcard. Note the consequences: `star` does *not*
match `startup` (so it is not substring matching), but `.` *does* (so it is not full-match
either), and `Bash|Read` in a **native** `.muse/hooks.json` will only ever match the exact
strings `Bash` or `Read`.

`SessionEnd`'s selector is `reason`: `other` and `other|clear` hit, `oth` and `clear` miss,
`oth.*` hits.

### 2. Four of the six offline-firable events have NO matcher selector — a silent-drop trap

```
UserPromptSubmit  matcher '*' fires;  'hi' 'hi there' 'there' '.*' 'h.*' 'prompt'
                  'UserPromptSubmit' 'user_prompt_submit'  -> group skipped
Stop / PreLLMCall / PostLLMCall   '*' fires; 'zzz' and 'other' -> group skipped
```

So on these events **any** matcher other than `*`/absent/`""` silently disables the whole group
— including the innocuous-looking `"matcher": "Bash"` a user copies from a Claude Code
`PreToolUse` example. This is the most likely real-world footgun in the format and the report
does not mention it.

### 3. `muse schema` is a dead end for hooks

`muse schema generate-json-schema --out DIR` and `--experimental` both produce
`msp.schema.json` + `manifest.json` with **zero** hook types. Worth recording as a negative
result: the hook contract is not part of the MSP wire protocol, so an MSP client cannot
discover it and there is no machine-readable schema to validate `.muse/hooks.json` against.

### 4. `muse init` scaffolds nothing for hooks

`muse init` in a clean directory writes exactly one file, `AGENTS.md`. No `.muse/`, no
`hooks.json`, no example. Combined with the silent-drop failure mode this makes the format
undiscoverable from the product itself.

### 5. Plugin-cache tamper detection is a usable supply-chain signal

Appending one line to a cached plugin hook script produced, on the next session, **zero**
plugin hook rows, plus `muse plugins hook test` -> `plugin hook \`allev:sessionstart\` is not
installed and enabled` and a `muse skills list` diagnostic
`plugin-cache-invalid: plugin cache is invalid; capabilities are blocked (plugin://allev)`.
That last line is the one headless hook-health signal the product actually prints.

### 6. `SessionStartActivationSourceV1` verified as a clean literal run

`…LegacyHookDecisionAppliedSessionStartActivationSourceV1startupresumeclearforkSessionStartCommandPayloadV2…`
— an unambiguous four-variant enum, unlike the payload-field run in R1. `muse resume <uuid>`
could not be exercised headlessly (`Device not configured (os error 6)`; it needs a TTY), so
`source` values other than `startup` remain unobserved, as does the `SessionEnd.reason`
vocabulary (no enum literal for it exists in the binary at all).

### 7. Everything else reproduced exactly

For the record, these all came out identical to the report and are re-confirmed: the 7-path
discovery matrix and its negatives; `managed_hooks_path` / `TBH_MANAGED_HOOKS_PATH` /
user-`settings.json`-`hooks` / project `.muse/hooks.json`; `XDG_CONFIG_HOME` override;
trust-gating of the project tier only; the versionless top level (`schema_version` 1/2/99/"x"
and unknown keys all ignored, `managed_hooks_env_vars` rejecting the file); tier order
managed(0) → user(1) → project(2) → plugin(100000+) with context blocks at 900000+n; the
17-variant `HookEventKind` (all 17 accepted by `plugins validate` with zero diagnostics); the
6 events that fire in an echo session and their verbatim payloads; the full handler triage
table; the 17×3 output-capability matrix (every cell); the PreToolUse verdict matrix including
`rewrite_selected` and the bare-allow rejection; the PermissionRequest `{behavior}` shape; the
PostToolUse `updatedMCPToolOutput` rejection; `additionalContext` as
`hook:user_prompt_submit:prompt:0 role=developer source=runtime_hook order=900000
reason=hook:user_prompt_submit`; the Stop cap (default 8 → 9 turns; 0/1/2/5/8/20 →
1/2/3/6/9/21) and `stop_hook_active` flipping; the scrubbed env allow-list and
`managed_hooks_env_vars` forwarding; `HookRunStatus` / `HookEffectCategory` / `HookRunOrigin`
literals; and every single `muse config validate --plane` result including
`field_not_activated` on `extensions.hooks` and `semantic_invalid` on
`execution.stop_hook_continuations`.
