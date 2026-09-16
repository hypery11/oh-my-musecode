# Meta Muse Code ("TBH") 1.0.1-R2006.1 — Complete CLI Surface

**Binary:** `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
**Version string:** `Muse Code 1.0.1 (1.0.1-R2006.1)` (`--version` / `-V`)
**Build sha (from export doc):** `e27e408b66`
**Method:** every claim below is either (a) a reproducible command against the binary, or (b) an exact literal from `strings`. All runtime probes ran with `HOME` redirected to a throwaway sandbox
(`.../scratchpad/sandbox/cli-surface/fakehome`) so the user's real `~/.config/muse` was never touched. No network, no auth, no Meta API traffic: everything below is offline or `--provider echo`.

> **zsh gotcha for reproduction.** zsh does *not* word-split unquoted parameter expansions. `$M $c --help` with `c="trace inspect"` passes a single argv token and silently falls back to the top-level help. Use `${=c}` (or bash). This produced a false "no sub-subcommand help exists" result on the first pass.

---

## 1. Top-level shape

```
$ MUSE_NO_AUTO_UPDATE=1 muse --help
muse — interactive terminal coding agent

If no subcommand is given, options run the interactive TUI; pass a prompt to start
a session, or use a command below.

Usage: muse [OPTIONS] [PROMPT]
       muse [OPTIONS] <COMMAND>

Commands:
  resume           Resume a previous session (--last or <session-uuid>)
  exec             Run one prompt non-interactively (headless)
  config           Validate enterprise configuration documents
  export           Export a session transcript to a file
  trace            Inspect a recorded session or run trace
  skills           List, inspect, enable, or disable skills
  sandbox          Check or set up the OS sandbox
  schema           Export the MSP wire schema (JSON Schema or TypeScript)
  serve            Serve an MSP session host over stdio
  session-message  List or send cross-session messages
  auth             Store provider API credentials
  login            Log in to a provider
  logout           Remove stored provider credentials
  init             Scaffold agent config in this workspace
```

The clap-derived usage (visible only in a parse error) reveals the real positional shape:

```
$ muse --zzz-nope
invalid TUI options: error: unexpected argument '--zzz-nope' found
  tip: to pass '--zzz-nope' as a value, use '-- --zzz-nope'
Usage: muse [OPTIONS] [resume <session-uuid>]...
```

i.e. the root TUI parser accepts a trailing `resume <uuid>` positional pair, which is why `muse resume` inherits the *entire* root option set (proved in §4.1).

### 1.1 Complete command inventory (16 commands, 2 hidden)

Oracle used: `muse <name> --help`. An unknown first token is treated as prompt text and the process
prints the top-level help; a real command prints its own. A candidate list of 130 plausible names
(doctor, monitor, mcp, sdk, tui, update, feedback, agents, hooks, memory, settings, …) was swept;
exactly these 16 hit.

| Command | Advertised in `--help`? | Gate | First line of its help |
|---|---|---|---|
| `resume` | yes | — | `muse resume — resume a previous session` |
| `exec` | yes | — | `muse exec — run one prompt non-interactively (headless)` |
| `config` | yes | — | `Usage: muse config validate --plane <defaults\|policy> --file <path>` |
| `export` | yes | — | `usage: muse export [--session <id\|path>] [--last] [--out <file>] [--redacted]` |
| `trace` | yes | — | `muse trace — inspect a recorded Muse Code trace` |
| `skills` | yes | — | `usage: muse skills list …` |
| `sandbox` | yes | — | `usage: muse sandbox windows check` |
| `schema` | yes | — | `muse schema — export the MSP wire schema embedded in this binary` |
| `serve` | yes | — | `muse serve — serve an MSP session host over stdio` |
| `session-message` | yes | — | `usage: muse session-message <command> [options]` |
| `auth` | yes | — | `muse auth — store provider API credentials` |
| `login` | yes | — | `usage: muse login` |
| `logout` | yes | — | `usage: muse logout` |
| `init` | yes | — | `usage: muse init [--dry-run] [--force]` |
| **`plugins`** | **only when `MUSE_EXPERIMENTAL_PLUGINS=1`** | gate | `usage: muse plugins <command>` — ungated it prints `plugins are not available in this build` and exits 2 |
| **`workflows`** | **never** (self-described as unadvertised) | none — always live | `usage: muse workflows list` |

Proof for the two hidden ones:

```
$ diff <(muse --help) <(MUSE_EXPERIMENTAL_PLUGINS=1 muse --help)
15a16
>   plugins          Validate and manage plugin bundles

$ muse plugins list; echo $?
plugins are not available in this build
2

$ muse workflows            # never listed in any --help output
usage: muse workflows list
       muse workflows save <name> --from <script.js> [--scope project|user] [--overwrite]
       muse workflows run <entry> --headless-qa [--token-budget <tokens|Nk|Nm>] [--live-auto-qa|--prompt-live-smoke]
       muse workflows recover <workflow-run-id> [--apply] (--session-log <path>|--session <session-id>)

list/save manage saved named workflows (#6007): project scope lives in .agents/.codex/.claude workflows
directories, user scope lives in the user config workflows directory. run/recover are a QA-only lane:
not advertised in `muse --help`; kept for headless QA seeding and release smokes.
```

The dispatcher's own name table is a single concatenated literal in the binary
(`strings -a -n 5 muse-aarch64-macos | grep -n session-message`, line 111930):

```
execconfigexporttraceloginlogoutsession-messageskillssandboxschemaservepluginsworkflows
```

`resume`, `init` and `auth` are handled on other paths (`resume` is a root positional; `init`/`auth`
appear in adjacent literals).

### 1.2 Hidden internal argv modes (not commands)

Extracted with `grep -oE '__tbh[a-z_]*'`:

| argv[1] | Paired env | Evidence |
|---|---|---|
| `__tbh_internal_bwrap` | `TBH_INTERNAL_MODE=bwrap` | literal `bwrap/TBH_INTERNAL_MODE__tbh_internal_bwrap` next to `--perms --ro-bind-symlink --ro-bind-data`, `embedded Bubblewrap`, `Linux sandbox helper` |
| `__tbh_internal_linux_sandbox` | `TBH_INTERNAL_MODE=linux-sandbox` | literal `…--env-jsonlinux_helper command wrapped with embedded Linux sandbox helper__tbh_internal_linux_sandboxTBH_INTERNAL_MODElinux-sandbox` |
| `__tbh_internal_process_owner_pty_gate_v1` | `TBH_INTERNAL_PROCESS_OWNER_TOKEN` | literal `process-owner PTY admission cancelled__tbh_internal_process_owner_pty_gate_v1` |

Runtime confirmation that the PTY-gate token is a real mode and not prompt text:

```
$ timeout 8 muse zzzrandomprompt </dev/null >/dev/null 2>&1; echo $?
1                       # ordinary prompt: TUI aborts immediately on a non-tty
$ timeout 8 muse __tbh_internal_process_owner_pty_gate_v1 </dev/null >/dev/null 2>&1; echo $?
125                     # hangs waiting for its admission channel; killed by timeout
```

On macOS the two Linux-sandbox argv modes fall through to the ordinary top-level help (they are
Linux-only helper re-execs); the associated flags `--tbh-bwrap-selection-v1`, `--tbh-bwrap-path-hex`,
`--tbh-bwrap-capabilities`, `--tbh-bwrap-kind`, `--tbh-embedded-image-v1`,
`--tbh-embedded-image-device`, `--tbh-embedded-image-inode`, `--stale-fuse-remedy`, `--profile-json`,
`--env-json` exist only in that helper's parser.

---

## 2. The launcher is not part of the binary's CLI

`muse` on a user's PATH is `muse-launcher.sh` (1138 lines of bash), which `exec "$binary" "$@"` at the
end of `main()` — it adds **no** subcommands and consumes **no** flags. Its env vars are launcher-only:

`MUSE_NO_AUTO_UPDATE`, `MUSE_SYNC_UPDATE`, `MUSE_UPDATE_INTERVAL_SECONDS` (default 3600),
`MUSE_LAUNCHER_INSTALL`, `MUSE_LAUNCHER_URL`, `MUSE_CHANNEL_URL`, `MUSE_DOWNLOAD_HOST`,
`MUSE_RELEASE_INFO` (exported *into* the binary), `MUSE_AUTH_URL`, `MUSE_AUTH_PATH`,
`MUSE_CLIENT_ID`, `MUSE_LOGIN`.

`install.sh` adds: `MUSE_INSTALL_DIR`, `MUSE_NO_MODIFY_PATH`, `MUSE_UPGRADE_MODE`,
`MUSE_LAUNCHER_INSTALL`, `MUSE_LAUNCHER_URL`.

**`MUSE_NO_AUTO_UPDATE` does not exist inside the binary** — `grep -c MUSE_NO_AUTO_UPDATE strings5.txt` → `0`, while `grep -n MUSE_NO_AUTO_UPDATE muse-launcher.sh` → lines 1103, 1110. Setting it when invoking the raw binary is harmless but a no-op.

---

## 3. Exhaustive flag reference

Short flags exist only at the root/TUI parser and on `exec`; a full a–z/A–Z sweep found:
root → `-h`, `-V`, `-w`; `exec` → `-h`, `-w`. No others.

### 3.1 Root / TUI (also fully accepted by `resume`)

| Flag | Value | Default | Enum (verbatim from the binary's own error) |
|---|---|---|---|
| `-h, --help` | — | — | |
| `-V, --version` | — | — | prints `Muse Code 1.0.1 (1.0.1-R2006.1)` |
| `--agents <JSON>` | JSON string | — | one ephemeral agent-definition overlay; bad JSON → `Session Agent Definition JSON is invalid` |
| `--provider <MODE>` | enum | `meta` | `unsupported provider `X`; expected `echo` or `meta`` |
| `--preset <NAME>` | name | — | `native-basic`, `miniswe`; bad chars → ``preset name `X` must use lowercase ASCII letters, digits, '-' or '_'`` |
| `--model <MODEL>` | string | — | non-echo providers only |
| `--reasoning-effort <EFFORT>` | enum | `high` | `none\|minimal\|low\|medium\|high\|xhigh\|ultra` |
| `--base-url <URL>` | URL | `https://api.meta.ai/v1` | overrides the Meta provider base URL |
| `--image <PATH>` | path | — | attach a local image to the next prompt |
| `--workspace <PATH>` | path | cwd | must exist and be a directory |
| `-w, --worktree [<MODE>]` | optional enum | `off` | `off\|create\|existing`; bare `-w` ⇒ `create`. A non-mode value is *not* an error: `muse: note: --worktree given no mode, using 'create'; 'X' kept as prompt text` |
| `--worktree-base <REF>` | git ref | `HEAD` | requires `--worktree create` |
| `--worktree-existing <PATH>` | path | — | requires `--worktree existing`; must differ from `--workspace` |
| `--parallel-tool-calls` | bool | — | Meta provider only (`parallel tool call flags are only supported with --provider meta`) |
| `--no-parallel-tool-calls` | bool | — | same constraint; the pair may be given only once |
| `--subagent-worktree-isolation` | bool | on | "Compatibility flag; capability defaults on." |
| `--approval-mode <MODE>` | enum | `on-request` | `untrusted\|on-request\|never` |
| `--permission-profile <ID>` | id | — | resolved from the enterprise config; unknown ⇒ `Permission profile 'X' is unavailable: profile does not exist.` |
| `--approval-judge <off\|on>` | enum | `on` | `unknown approval judge value 'X' (use: off, on)` |
| `--no-session-log` | bool | — | cannot be combined with `--worktree` or `--session-id` |
| `--echo-delay-ms <MS>` | int | — | echo provider only |
| `--yolo` | bool | — | disable approval **and** sandbox + trust workspace, this run |
| `--trust-workspace` | bool | — | load this workspace's skills/rules for the run; does **not** persist trust |
| `--disable-approval` | bool | — | |
| `--disable-sandbox` | bool | — | |
| `--sandbox-network <MODE>` | enum | `proxy-only` | `restricted\|enabled\|proxy-only` |
| `--disable-write` | bool | — | non-shell workspace FS writes |
| `--disable-shell` | bool | — | |
| `--enable-shell-tool` | bool | — | legacy shell tool instead of the managed platform shell |
| `--last` | bool | — | only meaningful with `resume` (`--last requires resume`) |

A full sweep of 566 candidate long flags (every hyphen-boundary prefix of every `--…` literal in the
binary) against the root parser found **no undocumented root flags** — every hit is in the table above.

### 3.2 `muse exec` — 3 undocumented flags

Documented in `muse exec --help`: `--json`, `--prompt-file`, `--api-key-stdin`, `--provider`,
`--preset`, `--permission-profile`, `--model`, `--reasoning-effort`, `--parallel-tool-calls`,
`--no-parallel-tool-calls`, `--base-url`, `--image` (repeatable), `--workspace`, `-w/--worktree`,
`--worktree-base`, `--worktree-existing`, `--context-compaction-strategy`,
`--context-compaction-soft-threshold`, `--context-compaction-hard-threshold`, `--max-model-steps`,
`--max-tool-output-bytes`, `--session-id`, `--allow-workspace-switch`, `--user-input-auto-resolve`,
`--subagent-worktree-isolation`, `--disable-web-tools`, `--no-foreign-personal-context`,
`--no-session-log`, `--approval-mode`, `--approval-judge`, `-h/--help`, plus the Safety block
(`--yolo`, `--trust-workspace`, `--disable-approval`, `--disable-sandbox`, `--sandbox-network`,
`--disable-write`, `--disable-shell`, `--enable-shell-tool`).

**Accepted but absent from every `--help`:**

| Hidden flag | Behaviour (verbatim) |
|---|---|
| `--agents <JSON>` | `missing value for --agents`; bad JSON → `Session Agent Definition JSON is invalid`. Same ephemeral agent-definition overlay as the root flag, just undocumented on `exec`. |
| `--eval-context-compaction-strategy <ID>` | ``unknown eval context compaction strategy `X` `` (no enum printed) |
| `--eval-workflow-api-version <v1\|v2>` | ``unknown Workflow API version `X`; expected v1 or v2`` |
| `--api-key <…>` | recognized *only* to refuse it: `use --api-key-stdin instead of passing secrets as flag values` |

Verbatim enum/format errors for the documented value-taking flags:

```
--provider                          unsupported provider `X`; expected `echo` or `meta`
--reasoning-effort                  unsupported reasoning effort `X`; expected none|minimal|low|medium|high|xhigh|ultra
--approval-mode                     unsupported approval mode `X`; expected untrusted|on-request|never
--approval-judge                    unknown approval judge value 'X' (use: off, on)
--sandbox-network                   unsupported sandbox network mode `X`; expected restricted|enabled|proxy-only
--context-compaction-strategy       unknown context compaction strategy `X`; expected
                                      summary-preserved-suffix/v1|prefix-extension-summary/v1|prefix-extension-inventory-summary/v1
--session-id                        invalid --session-id: X (expected a UUID, e.g. 123e4567-e89b-12d3-a456-426614174000)
--workspace                         workspace root does not exist: X
--image                             failed to read --image X: No such file or directory (os error 2)
```

Cross-flag validation literals (single concatenated blob at strings5.txt:111944):

```
use --api-key-stdin instead of passing secrets as flag values
--prompt-file may only be provided once
--max-model-steps must be greater than 0
--eval-workflow-api-version may only be provided once
--prompt-file cannot be used with inline prompt text
missing prompt
--worktree-base requires --worktree create
--worktree-existing requires --worktree existing
--worktree existing requires --worktree-existing
--worktree-existing must differ from --workspace
--worktree requires session logging; remove --no-session-log
a session id needs retained logging; remove --no-session-log
--allow-workspace-switch requires --session-id
--last may only be provided once
parallel tool call flags may only be provided once
--preset may only be provided once
--session-id requires session logging
```

`--json` emits MSP durable records as JSONL on stdout; a live sample with `--provider echo`:

```
{"schema_version":1,"id":"018f0000-0000-7000-8000-00000000c350","stream":{"kind":"session","id":"…"},
 "sequence":1,"recorded_at":1780531400000000,"record_type":"reconciliation","durability":"durable",
 "causation_id":"…","payload_type":"runtime.command.accepted","payload_schema_version":1,
 "payload":{"kind":"command_accepted","command_id":"…","client_id":null,"command_kind":"turn.submit"}}
```
payload_type sequence for one echo turn: `runtime.command.accepted` → `session.run.linked` →
`turn.input.user` → `run.lifecycle.started` → `task.stream.linked` → `task.lifecycle.proposed` →
`accepted` → `scheduled` → `side_effect_intent` → `started` → `run.output.delta` →
`task.lifecycle.completed` → `run.terminal.completed`.

### 3.3 `muse resume`

```
muse resume — resume a previous session

Usage: muse resume
       muse resume --last
       muse resume <session-uuid>

With no argument it opens the session picker for this workspace.

Options:
      --last    Resume the most recent session in this workspace

Root options (`--provider`, `--workspace`, …) may appear on either side of
`resume`. Run `muse --help` for the full list.
```

Confirmed by sweep: `resume` accepts every root flag, with the same clap validators
(`--provider`, `--reasoning-effort`, `--approval-mode`, `--approval-judge`, `--sandbox-network`,
`--echo-delay-ms`, `--model`, `--preset`, `--permission-profile`, `--image`, `--agents`, `--base-url`,
`--workspace`, `--worktree*`, `--yolo`, `--trust-workspace`, `--disable-*`, `--no-session-log`,
`--parallel-tool-calls`/`--no-…`, `--subagent-worktree-isolation`, `--enable-shell-tool`, `--last`,
`--version`, `--help`). Non-interactive without a target: `muse resume requires an interactive
terminal; pass --last or a session uuid for non-interactive resume`. `--last` + uuid →
`resume session uuid cannot be combined with --last`.

### 3.4 `muse exec` and `muse serve` — the serve refusal set

```
muse serve — serve an MSP session host over stdio

The client owns this process's stdin and stdout and is its only
connection. Sandbox posture and session durability are constructed
here and apply to every session the host loads; neither is negotiable
over the wire. Approval mode is the other way round — it is selected
on the wire, so there is no approval flag here.

Options:
  -h, --help
      --no-session-log            Use memory-only sessions
Sandbox posture (fixed for the host's lifetime):
      --disable-sandbox
      --sandbox-network <MODE>    (default: proxy-only)
      --disable-write
      --disable-shell
      --trust-workspace           Load each session workspace's skills and rules
```

Sweeping all 566 candidates against `serve` surfaces a **reserved, not-yet-implemented flag** plus a
bespoke refusal family:

| Flag | Response (verbatim) |
|---|---|
| `--listen` | `muse serve: --listen is not available in v1; the unix-socket and websocket transports are deferred post-v1 (#13929)` |
| `--yolo` | `muse serve: --yolo is not a serve option; approval mode is selected over the wire (session/start.approvalMode, session/setApprovalMode) and constructed nowhere` |
| `--approval-mode` | same sentence with `--approval-mode` |
| `--approval-judge` | same sentence with `--approval-judge` |
| `--disable-approval` | same sentence with `--disable-approval` |
| `--sandbox-network` | `muse serve: --sandbox-network expects one of restricted\|enabled\|proxy-only` |
| anything else | `muse serve: unknown option --X` |

`--listen` is the single most interesting hidden CLI affordance in the binary: a unix-socket /
websocket MSP transport that is parsed and explicitly deferred.

### 3.5 `muse skills` (10 sub-subcommands)

```
usage: muse skills list      [--source all|user|project|built-in|plugin] [--enabled-only] [--workspace <path>] [--trust-workspace] [--json]
usage: muse skills inspect   <skill-id-or-path> [--source all|user|project|built-in|plugin] [--workspace <path>] [--trust-workspace] [--json]
usage: muse skills enable    <skill-id-or-path> --scope user|project|built-in|plugin [--workspace <path>] [--trust-workspace] [--json]
usage: muse skills user-only <skill-id-or-path> --scope user|project|built-in|plugin [--workspace <path>] [--trust-workspace] [--json]
usage: muse skills disable   <skill-id-or-path> --scope user|project|built-in|plugin [--workspace <path>] [--trust-workspace] [--json]
usage: muse skills validate  <path> [--json]
usage: muse skills install   <path> [--scope user] [--name NAME] [--force] [--json]
usage: muse skills import    --from claude|codex [--scope user] [--dry-run] [--force] [--json]
usage: muse skills update    <skill-id> [--json]
usage: muse skills uninstall <skill-id> [--keep-files] [--json]
```

A 40-name brute force found no eleventh: `create`, `new`, `remove`, `show`, `search`, `export`,
`sync`, `doctor` all return `` unknown skills command `X` ``.
`list` output is TSV: `NAME<TAB>SCOPE<TAB>ACTIVATION<TAB>DESCRIPTION<TAB>PATH`; ACTIVATION values
observed `on` / `user-invocable-only` (`off` also exists as a literal). Paths are rendered with
`$HOME` / `$CONFIG_DIR` placeholders. Skill IDs for the bundled set are `bundled:<name>`
(literals `bundled:` and `bundled://` are both present).

**`muse skills import --from claude|codex` reads competitor agents' skill directories.** The literal
`HOME is required for skills import<unset>claudecodex` sits directly beside the plugin verb table.

### 3.6 `muse plugins` (gated by `MUSE_EXPERIMENTAL_PLUGINS=1`)

```
usage: muse plugins <command>

Commands:
  install <path> [--scope user|project] [--json]      Install a local plugin bundle into the cache
  install <plugin>@<marketplace> [--json]             Install a plugin from a configured marketplace snapshot
  list [--available] [--json]                         List installed or available plugins
  inspect <id> [--json]                               Inspect one installed plugin and its runtime capabilities
  approve <plugin-id[[:kind]:capability-id] | stable-id> [--json]   Trust and enable current runtime capability definitions
  reject  <plugin-id[[:kind]:capability-id] | stable-id> [--json]   Trust and disable current runtime capability definitions
  hook test <plugin-id>:<hook-id> | plugin:<plugin-id>:hook:<hook-id> --fixture <path> [--json]
                                                      Run one installed plugin hook against a fixture
  marketplace add <name> <source> [--json]            Add a local file/directory or Git marketplace source and store a snapshot
  marketplace list [--json]
  marketplace update <name> [--json]
  marketplace remove <name> [--json]
  enable  <id> [--json]
  disable <id> [--json]
  update  <id> [--json]
  remove  <id> [--delete-data] [--json]
  validate <path> [--json]                            Validate a local plugin bundle without installing or executing it
```

Side effect of the gate beyond the command itself:

```
$ diff <(muse skills list --source built-in | cut -f1) \
       <(MUSE_EXPERIMENTAL_PLUGINS=1 muse skills list --source built-in | cut -f1)
2a3
> create-plugin
```

Runtime-capability kinds a plugin can declare (literal, next to `plugins inspect`):
`hook`, `reminder`, `mcp_server`, `agent_definition`; trust states
`review_needed`, `trusted_enabled`, `invalid`, `blocked`; per-plugin env handed to hooks:
`MUSE_PLUGIN_ROOT`, `MUSE_PLUGIN_ID`, `MUSE_PLUGIN_DATA_DIR`, plus `hooks/hooks.json`.
Hook fixture format: `fixture must contain \`stdin\`` with fields `event`, `stdin`, `matcher_input`, `cwd`.

### 3.7 `muse workflows` (hidden, always on)

```
usage: muse workflows list
       muse workflows save <name> --from <script.js> [--scope project|user] [--overwrite]
       muse workflows run <entry> --headless-qa [--token-budget <tokens|Nk|Nm>] [--live-auto-qa|--prompt-live-smoke]
       muse workflows recover <workflow-run-id> [--apply] (--session-log <path>|--session <session-id>)
```

Verified end-to-end offline:

```
$ echo 'export async function main(ctx){ return "hi"; }' > /tmp/wf.js
$ muse workflows save demo --from /tmp/wf.js --scope user
Saved workflow "demo" to …/fakehome/.config/muse/workflows/demo.js
Run it with: muse workflows run demo --headless-qa, or workflow({"name": "demo"}) from the model tool.
$ muse workflows list
demo	user	$CONFIG_DIR/workflows/demo.js
```

`save` flags: `--from` (`--from requires a script path`), `--scope`
(`--scope must be project or user`), `--overwrite`. Project scope resolves against
`.agents` / `.codex` / `.claude` workflow directories — i.e. **Muse reads Claude Code's and Codex's
project directories for workflows**. Related literals: `__tbh_workflow_live_auto_qa`,
`TBH_WORKFLOW_PRODUCT_LIVE_AUTO_QA`, `TBH_WORKFLOW_PROMPT_LIVE_SMOKE`,
`Release gate: default-on`, `Workflow status: launched`,
`Owner path: runtime command admission -> production owner loop`.
`workflows` is completely independent of `MUSE_EXPERIMENTAL_WORKFLOW_TOOL` (setting it changes nothing).

### 3.8 `muse trace inspect`

```
Inspect a recorded session trace (golden fixture, session log, or runtime window)

Usage: muse trace inspect [OPTIONS]

Options:
      --fixture <path-or-name>              Golden-trace fixture path or bare name
      --session-log <jsonl>                 Session-log JSONL file to inspect
      --run-id <uuid>                       Project only this run stream of --session-log (#6408; includes hashed
                                            workflow-child run streams mirrored into the parent log)
      --all-runs                            Project every run stream of --session-log as one section per run
      --run-log <jsonl>                     Run-stream runtime log JSONL file
      --task-log <jsonl>                    Task-stream runtime log JSONL file
      --render-mode <compact|default|verbose>   [possible values: compact, default, verbose]
      --include-positions                   Include source positions in the report
      --max-text-bytes <N>                  Truncate rendered text values to N bytes
      --format <text|json>                  [possible values: text, json]
  -h, --help
```

`muse trace` with no subcommand: `error: a trace subcommand is required; expected \`inspect\``;
any other name: ``error: unsupported trace subcommand `X`; expected `inspect` ``.
With no source: `error: a trace source is required: pass --fixture, --session-log, --run-log, or --task-log`.
No golden fixtures are embedded — `--fixture __BOGUS__` renders an empty report rather than failing.
Live run against a session log produced by `muse exec --provider echo`:

```
Trace: session log / Scope: session_file / Schema: 1 / Result: completed / Records: run=18 task=6
```

### 3.9 `muse export`

Full help text (verbatim):

```
usage: muse export [--session <id|path>] [--last] [--out <file>] [--redacted]

Exports one session's durable log as a single self-contained JSON document
(export_schema_version 1): timestamps, messages, verbatim encrypted
reasoning, tool calls/results, approvals, question outcomes, model ids,
ses_/trajectory_ ids, and fork/subagent lineage.

Without --out, the document is written to trajectory-<timestamp>.json in
the current directory (falling back to the temp dir when the cwd is not
writable; an existing file is never overwritten — the name is uniquified)
and stdout carries exactly one line: the absolute path written.

With no --session on an interactive terminal, export opens the SAME session
picker `muse resume` uses so you can see and choose which session to export
(#10408) — unless --out is given (a file destination exports the most
recent session, never the picker). A piped/redirected export stays
non-interactive and exports the most recent session; pass --last to be
explicit or to skip the picker interactively.

Options:
  --session <id|path>  Session UUID (resolved in the local session store) or
                       a path to a session.jsonl file. An existing file path
                       wins over an id parse. Bypasses the picker.
  --last               Skip the picker and export the most recent retained
                       session for the current workspace. Also the default
                       when the terminal is non-interactive.
  --out <file>         Write the document to <file> instead of the default
                       timestamped file. Like --last, this
                       skips the picker (most recent session).
  --redacted           Share-safe variant: payload strings pass through the
                       telemetry redaction rules. Default output is RAW —
                       the export is a local file you already own. Encrypted
                       reasoning blobs stay verbatim in both modes.

Offline: reads only local files; no network access. Schema and details:
docs/session-export.md
```

Sweep confirms exactly four flags (`--session`, `--last`, `--out`, `--redacted`, `--help`);
anything else → `unexpected export argument: --X`. Live document top level:

```json
{"export_schema_version":1,"redaction":"raw",
 "exporter_version":{"display":"Muse Code 1.0.1 (e27e408b66)","sha":"e27e408b66","semver":"1.0.1"},
 "session_terminated_abnormally":false,
 "sessions":[…],"events":[…],
 "diagnostics":{"unparseable_lines":0,"unknown_payload_kinds":1,"gaps":0,
                "omitted_live_only":0,"duplicate_records":0},
 "session_build":{"display":"Muse Code 1.0.1 (e27e408b66)","sha":"e27e408b66","semver":"1.0.1"}}
```

### 3.10 `muse schema`

```
muse schema — export the MSP wire schema embedded in this binary

Usage: muse schema generate-json-schema --out DIR [--experimental]
       muse schema generate-ts --out DIR [--experimental]

Both exports are offline and instant: the bundles are precomputed at build
time, so the output is exact for this binary. The error table describes every
error this binary can emit, and the method index describes the served command
plane (session/*, turn/*, model/list, view/*, approval/*, userInput/*) — with
one exception: workflow/* (spec 14410) carries no row yet. tdd.md SS3.19/SS3.20
ratify its params and ack, so what is outstanding is the protocol-crate types
and bundle rows, not the shapes; tracked at
https://github.com/mslsrc/tbh/issues/14410.

Options:
      --out DIR       Directory to write the export into (created if absent)
      --experimental  Export the experimental surface instead of the stable one
```

Produced artefacts (offline):

```
$ muse schema generate-json-schema --out sch/stable
wrote manifest.json, msp.schema.json to sch/stable (stable surface)
$ muse schema generate-json-schema --out sch/exp --experimental
wrote manifest.json, msp.schema.json to sch/exp (experimental surface)
$ muse schema generate-ts --out sch/ts
wrote msp.d.ts to sch/ts (stable surface)
```

`sch/stable/manifest.json` → `{"experimental":false,"fingerprint":"sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7","schemaVersion":1}`
`sch/exp/manifest.json` → `{"experimental":true,"fingerprint":"sha256:577d717d09bf3aae6ad43c85d3c2d8e0c37bbde350897c94808626d2f362060c","schemaVersion":1}`
Sizes: 192K stable, 192K experimental, 104K `msp.d.ts`. The fingerprints are build-time constants and
do **not** change under any `MUSE_EXPERIMENTAL_*` gate.

### 3.11 `muse config` (enterprise configuration)

```
Usage: muse config validate --plane <defaults|policy> --file <path>
       muse config status
```

`config` has an unusual dispatcher: **any** token other than `status` prints the usage line
(no "unknown command" branch), so `validate` is the only real verb besides `status`.

```
$ muse config status
Enterprise configuration status
Generation: sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4
Sources:
  plane=defaults source_class=system_file state=absent
  plane=policy   source_class=system_file state=absent
  plane=defaults source_class=macos_managed_preferences state=absent
  plane=policy   source_class=macos_managed_preferences state=absent
```

Validator behaviour (exit 1 on invalid, 0 on valid):

```
$ echo '{}' > c.json;                        muse config validate --plane defaults --file c.json
enterprise_document_invalid: plane=defaults reason=missing_schema_version location=schema_version
$ echo '{"schema_version":1}' > c.json;      muse config validate --plane defaults --file c.json
valid: plane=defaults schema_version=1
$ echo '{"schema_version":1,"zzz":1}' …      enterprise_document_invalid: plane=defaults reason=unknown_member
$ echo '{"schema_version":1,"settings":{"model":{}}}' …
                                             enterprise_document_invalid: plane=defaults reason=wrong_type location=settings.model
```

Rejection reason vocabulary (single literal): `invalid_json`, `missing_schema_version`,
`unsupported_schema_version`, `unknown_member`, `wrong_type`, `normalized_duplicate_key`,
`field_not_activated`. Source classes: `system_file`, `macos_managed_preferences`,
`windows_machine_policy`. Source states: `absent`, `valid`, `unreadable`, `untrusted`, `malformed`,
`unsupported`, `invalid`, `stale`.

Accepted top-level container on the `defaults` plane is `settings`; accepted `settings.*` members
probed live: `tools`, `skills`, `mcp_servers`, `endpoint_transport`, `telemetry`, `notifications`,
`model` (scalar), `provider` (scalar). Rejected: `enabled`, `artifact`, `web_search`, `web_fetch`,
`skill_activation`, `preset`, `agent_profile`, `permission_profiles`. (The full policy-plane member
list lives in the `config` crate's blob — `permission_profiles`, `approval_modes`,
`approval_reviewers`, `network_sandbox_modes`, `tool_rules`, `allowed_providers`, `allowed_models`,
`model_fallback`, `forbid_approval_bypass`, `forbid_sandbox_bypass`,
`force_agent_definition_safe_mode`, `allow_project_configuration`, `allow_foreign_configuration`,
`stop_hook_continuations`, `voice_tap_enabled`, `execution_capacity`, `mcp_servers`, `privacy`,
`extensions`, `model_egress`, … — the config dimension owns the detail.)
`--permission-profile <ID>` on `exec`/root resolves against `permission_profiles` from this plane,
which is why every guessed id (`ask-me`, `default`, `yolo`, `plan`, `read-only`, …) reports
`profile does not exist` on a machine with no enterprise document.
Profile source labels in the binary: `managed_default`, `user_default`; refusal literal
`not allowed by managed policy`.

### 3.12 `muse session-message` (gated behaviour)

```
usage: muse session-message <command> [options]

commands:
  muse session-message list [--json]
  muse session-message send --target <session-uuid-or-name> [--in-reply-to <reply-token>] [--json]
```

`send` is parsed but stubbed: `session-message send is not implemented in this build`
(literal `not_attempted` / `not_implemented` adjacent). `list` is gated:

```
$ muse session-message list --json
{"schema_version":1,"status":"unavailable","error_code":"external_agent_ingress_closed"}
external agent ingress is unavailable

$ MUSE_EXPERIMENTAL_EXTERNAL_AGENT_INGRESS=1 muse session-message list --json
{"schema_version":1,"sessions":[{"session_id":"01a05d64-…","session_name":"tawny-radiation","workspace_label":"ws"}]}
```

Note the gate is `EXTERNAL_AGENT_INGRESS`, **not** the similarly named
`MUSE_EXPERIMENTAL_LOCAL_SESSION_MESSAGING` (which changes nothing here).
Other constraints: `session-message list accepts no positional args`,
`--json may be provided only once`, `--target is required`,
`session-message send accepts no positional body`. Transport env:
`TBH_SESSION_MESSAGE_SOCKET`; wire handle `tbh.session-message.target-handle.v1` with fields
`message.text`, `delivery.queue_next_turn`, `delivery.steer_active_turn`, `delivery.notify_only`,
`wake.when_idle`, `wake.at_safe_point`, `request.causal_context`, `request.expiry`,
`receipt.transport_accepted`, `receipt.target_admission`, `receipt.durable_delivery`,
`receipt.wake_requested`, `receipt.processing_observed`.

### 3.13 `muse auth` / `login` / `logout`

```
muse auth — store provider API credentials
Usage: muse auth set [--provider <PROVIDER>] --api-key-stdin
The API key is read from stdin (never taken as a command-line argument, so it
never lands in shell history).

  --provider <provider>  Which provider the credential is for. Defaults to `meta`.
                         [default: meta] [possible values: meta]
  --api-key-stdin        Read the API key from stdin (the only accepted way to pass a secret).
```

Any `auth` verb other than `set` → `` expected `auth set` ``. Positional secrets are refused:
`secret input rejected: secrets are not accepted as positional arguments`; a flag value is refused with
`secret input rejected: use --api-key-stdin instead of passing secrets as flag values`; empty stdin →
`secret input rejected: empty API key`; multi-line → `API key must be a single line`.

```
usage: muse login
Log in with your Meta account: approve a code in your browser.
META_API_KEY always takes priority over the account login.

usage: muse logout
Remove the saved Meta credential (API key or Meta-account login).
META_API_KEY in the environment is not touched.
```

Both take **no** arguments: `muse login takes no arguments (got \`X\`); see muse login --help`.
Credential store: `$CONFIG_DIR/auth.json` (`…/.config/muse/auth.json`, with an `.auth.json.lock`),
macOS Keychain when available — `Your saved login is in the macOS Keychain, which is locked in this
session. Run 'security unlock-keychain' and retry, or log in again.` Backend selector:
`TBH_CREDENTIAL_BACKEND` (`keychain_fallback_file` literal). Endpoints: `TBH_AUTH_BASE_URL`,
`TBH_MINT_BASE_URL`, default model base `https://api.meta.ai/v1`.
Other provider keys recognized in the env registry: `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`,
`OPENAI_API_KEY`, `OPENAI_BASE_URL`, `OPENROUTER_API_KEY`, `OPENROUTER_BASE_URL` — although the CLI
`--provider` enum is only `echo|meta` and `auth set --provider` only `meta`.

### 3.14 `muse init`

```
usage: muse init [--dry-run] [--force]
```

Exactly three flags (`--dry-run`, `--force`, `--help`). It scaffolds a single file, `AGENTS.md`
(not `MUSE.md`, not `CLAUDE.md`):

```
$ muse init --dry-run          # prints the file to stdout, writes nothing
# AGENTS.md

Muse Code reads this file as project rules when it runs in this directory.

## Project

- Name: ws
- Generated by `muse init`.

## Common Commands

- No standard build or test commands detected yet.

## Project Layout

- No common source, test, docs, or spec directories detected yet.

$ muse init
Wrote AGENTS.md
```

The Commands/Layout sections are populated by repo detection (literal `TBH_SOURCE_AWARE_INIT_V1`).
An untrusted workspace with an AGENTS.md warns at session start:
`muse: warning: rules file at …/AGENTS.md exists, but the workspace is untrusted, so it is skipped for
this session; restart with --trust-workspace to load project rules`.

### 3.15 `muse sandbox`

```
usage: muse sandbox windows check
       muse sandbox windows setup
```

`windows` is the **only** subcommand — `check`, `setup`, `macos`, `linux`, `status` at the `sandbox`
level all return `` unknown sandbox command `X` ``. macOS uses `/usr/bin/sandbox-exec`
(`macos_seatbelt`, `command wrapped with fixed macOS sandbox-exec path`, `(version 1)` profile
preamble); Linux uses embedded Bubblewrap; neither needs a setup command. Sandbox network modes
`restricted | enabled | proxy-only`; internal names `diagnostic`, `proxy`, `restricted`, `enabled`,
`proxy_only`, `linux_broker_socket`.

---

## 4. Environment gates

### 4.1 `MUSE_EXPERIMENTAL_*` — 41 gates, with their internal ids

Two adjacent literals in the binary give the env-var list and the gate-id list in **identical order**,
which pins the mapping exactly (strings5.txt line 177520, ending in `gate.resolve`):

| # | Env var | Gate id |
|---|---|---|
| 1 | `MUSE_EXPERIMENTAL_WORKFLOW_TOOL` | `workflow_tool` |
| 2 | `MUSE_EXPERIMENTAL_ARTIFACT_TOOL` | `artifact_tool` |
| 3 | `MUSE_EXPERIMENTAL_LOCAL_SESSION_MESSAGING` | `local_session_messaging` |
| 4 | `MUSE_EXPERIMENTAL_EXTERNAL_AGENT_INGRESS` | `external_agent_ingress` |
| 5 | `MUSE_EXPERIMENTAL_CODE_MODE` | `code_mode` |
| 6 | `MUSE_EXPERIMENTAL_PREFIX_COMPACTION` | `prefix_compaction` |
| 7 | `MUSE_EXPERIMENTAL_MONITOR` | `monitor` |
| 8 | `MUSE_EXPERIMENTAL_FOREIGN_PERSONAL_CONTEXT_KILL` | `foreign_personal_context_kill` |
| 9 | `MUSE_EXPERIMENTAL_VOICE` | `voice` |
| 10 | `MUSE_EXPERIMENTAL_VOICE_DEFAULT_ON` | `voice_default_on` |
| 11 | `MUSE_EXPERIMENTAL_VOICE_NATIVE_CAPTURE` | `voice_native_capture` |
| 12 | `MUSE_EXPERIMENTAL_REASONING_DISPLAY` | `reasoning_display` |
| 13 | `MUSE_EXPERIMENTAL_MODEL_EFFORT_CONTEXT` | `model_effort_context` |
| 14 | `MUSE_EXPERIMENTAL_BASH_TITLES` | `bash_titles` |
| 15 | `MUSE_EXPERIMENTAL_BASH_SANDBOX_ESCALATION` | `bash_sandbox_escalation` |
| 16 | `MUSE_EXPERIMENTAL_GIT_SANDBOX_RELAXATION` | `git_sandbox_relaxation` |
| 17 | `MUSE_EXPERIMENTAL_PLUGINS` | `plugins` |
| 18 | `MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG` | `enterprise_config` |
| 19 | `MUSE_EXPERIMENTAL_WEB_FETCH` | `web_fetch` |
| 20 | `MUSE_EXPERIMENTAL_SERVER_WEB_FETCH` | `server_web_fetch` |
| 21 | `MUSE_EXPERIMENTAL_CURL_WEB_FETCH` | `curl_web_fetch` |
| 22 | `MUSE_EXPERIMENTAL_WEB_FETCH_PREFLIGHT_HARD_CAP` | `web_fetch_preflight_hard_cap` |
| 23 | `MUSE_EXPERIMENTAL_FIRST_TURN_MINIMAL_EFFORT` | `first_turn_minimal_effort` |
| 24 | `MUSE_EXPERIMENTAL_TODO_REMINDER` | `todo_reminder` |
| 25 | `MUSE_EXPERIMENTAL_MEMORY_REMINDER` | `memory_reminder` |
| 26 | `MUSE_EXPERIMENTAL_SKILL_REMINDER` | `skill_reminder` |
| 27 | `MUSE_EXPERIMENTAL_GOAL_REMINDER` | `goal_reminder` |
| 28 | `MUSE_EXPERIMENTAL_VERIFY_REMINDER` | `verify_reminder` |
| 29 | `MUSE_EXPERIMENTAL_SCOPE_REMINDER` | `scope_reminder` |
| 30 | `MUSE_EXPERIMENTAL_SESSION_RUNTIME` | `session_runtime` |
| 31 | `MUSE_EXPERIMENTAL_SESSION_RECOVERY_SHADOW` | `session_recovery_shadow` |
| 32 | `MUSE_EXPERIMENTAL_TUI_MSP_CLIENT` | `tui_msp_client` |
| 33 | `MUSE_EXPERIMENTAL_SDK_ENABLED` | `sdk_enabled` |
| 34 | `MUSE_EXPERIMENTAL_META_CONTEXT_WORKSPACE_ONLY` | `meta_context_workspace_only` |
| 35 | `MUSE_EXPERIMENTAL_NON_STRICT_TOOL_PARAMS` | `non_strict_tool_params` |
| 36 | `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS` | `hook_selected_skills` |
| 37 | `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY` | `hook_selected_skills_apply` |
| 38 | `MUSE_EXPERIMENTAL_WORKFLOW_API_V2_ROLLOUT` | `workflow_api_v2_rollout` |
| 39 | `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH` | `subscription_launch` |
| 40 | `MUSE_EXPERIMENTAL_PROVIDER_TOOL_SWITCH` | `provider_tool_switch` |
| 41 | `MUSE_EXPERIMENTAL_TAG` | `tag` |

**Observable CLI effect of each gate.** Every one of the 41 was set to `1` and diffed against
`muse --help`, `muse exec --help`, `muse skills list --source built-in`, `muse config status`,
`muse session-message list --json`, `muse workflows list`:

| Gate | CLI-visible change |
|---|---|
| `MUSE_EXPERIMENTAL_PLUGINS` | adds the `plugins` top-level command to `muse --help`; adds the `create-plugin` built-in skill |
| `MUSE_EXPERIMENTAL_EXTERNAL_AGENT_INGRESS` | `session-message list` goes from `error_code:"external_agent_ingress_closed"` to a real session roster |
| **all 39 others** | **no change to any CLI surface** — they gate in-session tools, TUI panels and prompt behaviour, not argv |

Notably `MUSE_EXPERIMENTAL_WORKFLOW_TOOL` does *not* gate `muse workflows` (that lane is
unconditional); it gates the model-facing `workflow(...)` tool.

Two gates also have **remote** feature-config keys (server-pushed, `remote` / `default` resolution):
`ARTIFACT_TOOL_ENABLED` and `LOCAL_SESSION_MESSAGING_ENABLED`. Remote fetch can be killed with
`TBH_DISABLE_FEATURE_CONFIG`.

### 4.2 Non-experimental `MUSE_*` env vars read by the binary

| Var | Values / effect |
|---|---|
| `MUSE_MODEL` | default model id for non-echo providers |
| `MUSE_ENABLE_WEB_TOOLS` | strict enum — `MUSE_ENABLE_WEB_TOOLS must be one of 1,true,on,yes,0,false,off,no`; a bad value aborts *every* subcommand. Status line: `off (MUSE_ENABLE_WEB_TOOLS)` |
| `MUSE_WEB_SEARCH_MODE` | strict enum — `MUSE_WEB_SEARCH_MODE must be one of client,hosted,off`; aborts every subcommand on a bad value. `Set by MUSE_WEB_SEARCH_MODE. Applies to new sessions.` |
| `MUSE_DISABLE_APPROVAL_JUDGE` | INFERRED from the name: the env equivalent of `--approval-judge off`. Only the bare name is in the binary, with no adjacent message. |
| `MUSE_CUSTOM_HEADERS` | extra provider headers; `MUSE_CUSTOM_HEADERS contains invalid provider header input; affected headers will not be sent` |
| `MUSE_WWW_ROUTING` | sets the `X-FB-Routing-Control` header; `MUSE_WWW_ROUTING is set to an unrecognized value; the X-FB-Routing-Control header will not be sent` |
| `MUSE_HUMAN_CONFIRMATION` | appears beside the `monitor` tool's runtime text |
| `MUSE_SESSIONS` | documented in the `read-session` skill as `${XDG_DATA_HOME:-$HOME/.local/share}/muse/sessions` |
| `MUSE_CURRENT_SESSION_LOG` | path to the live session log, handed to helper scripts |
| `MUSE_TOOL_USE_ID` | current tool-call id exposed to helpers |
| `MUSE_PLUGIN_ROOT`, `MUSE_PLUGIN_ID`, `MUSE_PLUGIN_DATA_DIR` | per-plugin hook environment |
| `MUSE_DISABLE_ULTRA_ANIMATION` | TUI "ultra" effort animation off |
| `MUSE_DISABLE_ACTIVE_BANG_SHELL_QUEUE` | disables the `!`-prefix shell queue in the composer |
| `MUSE_DISABLE_VOICE_WAVE` | disables the voice waveform widget |
| `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `CODEX_HOME` | path resolution; `CODEX_HOME` is read for Codex interop |

### 4.3 `TBH_*` env vars (≈110). Product-relevant subset:

| Var | Effect |
|---|---|
| `TBH_INTERNAL_MODE` | selects an internal re-exec mode: `bwrap`, `linux-sandbox` |
| `TBH_INTERNAL_PROCESS_OWNER_TOKEN` | paired with `__tbh_internal_process_owner_pty_gate_v1` |
| `TBH_CREDENTIAL_BACKEND` | keychain vs file credential store |
| `TBH_AUTH_BASE_URL`, `TBH_MINT_BASE_URL` | auth / token-mint endpoints |
| `TBH_UNKILL_SLASH_COMMANDS` | re-enables slash commands otherwise suppressed |
| `TBH_DISABLE_TELEMETRY` | telemetry kill switch |
| `TBH_DISABLE_FEATURE_CONFIG` | disables the remote feature-config fetch |
| `TBH_DISABLE_MACOS_NETWORK_SUPPORT` | drops macOS sandbox network support |
| `TBH_DISABLE_TOOL_SCALAR_COMPAT` | strict tool-parameter typing |
| `TBH_DISABLE_CHILD_SESSION_LOG_ROUTING` | subagent log routing off |
| `TBH_MANAGED_HOOKS_PATH` | managed (enterprise) hooks file |
| `TBH_EVAL_APPEND_SYSTEM_PROMPT[_FILE]`, `TBH_EVAL_APPEND_DEVELOPER_PROMPT[_FILE]` | **prompt injection points for evals** |
| `TBH_EVAL_USER_STEER_FILE`, `TBH_EVAL_STEER_AFTER_REMINDER_AGENT_ID` | scripted user steering |
| `TBH_NATIVE_SUBAGENT_DOGFOOD` | native subagent dogfood path |
| `TBH_SUBAGENT_RUNTIME_CONCURRENCY_CAP` | subagent fan-out cap |
| `TBH_STREAM_FIRST_EVENT_TIMEOUT_SECS`, `TBH_STREAM_IDLE_TIMEOUT_SECS` | model stream timeouts |
| `TBH_META_FILE_EXPIRY_SECS` | uploaded-file TTL |
| `TBH_VIDEO_INPUT_ENABLED` | video input |
| `TBH_VOICE_FAKE`, `TBH_VOICE_FAKE_AUDIO`, `TBH_VOICE_FAKE_FINAL_DELAY_MS`, `TBH_VOICE_CAPTURE_WAV_FILE`, `TBH_VOICE_ASR_PROTOCOL/_ENDPOINT/_MODEL/_PROXY` | voice/ASR plumbing |
| `TBH_SESSION_MESSAGE_SOCKET` | cross-session messaging socket |
| `TBH_PROVIDER_TRACE_ROOT` | dumps provider traces |
| `TBH_CONTEXT_COMPACTION_EVIDENCE_PATH` | compaction evidence dump |
| `TBH_FEEDBACK_ROUTE`, `TBH_FEEDBACK_INTAKE` | feedback routing |
| `TBH_COPY_TRANSPORT` | clipboard transport (`os-clipboard`/`tmux`/`osc52` literals nearby) |
| `TBH_TUI_HYPERLINKS`, `TBH_TUI_CRASH_FIXTURE` | TUI |
| `TBH_SOURCE_AWARE_INIT_V1` | `muse init` repo detection |
| `TBH_OVERDUE_NOTICE_AGE_SECS`, `TBH_OVERDUE_BUSY_RETRY_FLOOR_SECS` | background-task nagging |
| `TBH_WORKFLOW_PRODUCT_LIVE_AUTO_QA`, `TBH_WORKFLOW_PROMPT_LIVE_SMOKE` | back the `workflows run --live-auto-qa` / `--prompt-live-smoke` flags |
| `TBH_BWRAP_EXE` | override the bubblewrap binary (literal is immediately followed by `Bubblewrap was not found on PATH…`) |
| `TBH_CLAUDE_CHANNELS_LIVE_SMOKE`, `_SMOKE_ROOT`, `_SMOKE_STATE`, `_SMOKE_DEADLINE_SECONDS` | "Claude channels" smoke harness (see `--internal-claude-channels-sidecar-v1`) |
| `TBH_IS_E2E_TEST`, `TBH_E2E_STARTUP_READ_TARGET/_TRACE` | e2e markers |
| ~55 `TBH_TMUX_*` | tmux-driven TUI test fixtures/clocks/release-files (not product surface) |

Two more internal flag literals with no `--help` home:
`--internal-claude-channels-sidecar-v1` and `--internal-claude-channels-live-smoke-host-v1`.
Also present: `OTEL_EXPORTER_OTLP_ENDPOINT`, `RUST_MIN_STACK`, and the terminal-detection set
`SSH_TTY`, `SSH_CONNECTION`, `TMUX_PANE`, `WAYLAND_DISPLAY`, `DISPLAY`.

---

## 5. Adjacent surface: built-in slash commands

Not argv, but the same "command surface". The closed vocabulary is one literal
(strings5.txt lines 150316–150317), annotated in the binary itself:

> *Built-in slash-command names pass through verbatim from the durable record's closed vocabulary
> (spec 8403 owns it; machine-readable as `tbh_agent::command_invoked::BUILTIN_SLASH_COMMAND_NAMES`
> minus `TELEMETRY_DROPPED_SLASH_COMMAND_NAMES` since #13706, TUI-table parity pinned in
> `crates/tui/src/slash.rs`); plugin-command and skill-shortcut dispatches land in the literal
> `custom` bucket*

```
/login /logout /clear /new /resume /fork /side /init /deep-research /subagents /model /settings
/keymap /help /theme /rules /compact /export /copy /recap /skills /plugins /skill /effort /goal
/feedback /voice /exit /quit /stop /status /usage /upgrade /permissions /tasks /workflows
```

(36 names, plus the `custom` bucket.) `TBH_UNKILL_SLASH_COMMANDS` restores ones the build suppresses.

---

## 6. Parser families and error signatures (useful for tooling)

Muse has **five distinct argument parsers**, each with its own error dialect. This matters for anyone
wrapping the CLI:

| Family | Commands | Unknown-flag error | Unknown-verb error |
|---|---|---|---|
| clap (derive) | root/TUI, `resume`, `trace inspect`, `auth set` | `invalid TUI options: error: unexpected argument '--X' found` / `error: unexpected argument '--X' found` | — |
| hand-rolled A | `exec`, `skills`, `init`, `session-message` | `unknown option --X` | `` unknown skills command `X` `` |
| hand-rolled B | `plugins` | `` unknown plugins list option `--X` `` | `` unsupported plugins command `X` `` |
| hand-rolled C | `serve`, `schema` | `muse serve: unknown option --X` / `` muse schema generate-ts: unexpected argument `--X` `` | — |
| hand-rolled D | `export`, `workflows`, `config`, `trace`, `login`, `logout` | `unexpected export argument: --X` / `unknown workflows save argument: --X` / usage reprint / `muse login takes no arguments (got \`--X\`)` | `` error: unsupported trace subcommand `X`; expected `inspect` `` |

**`--help` only works at depth 1 and 2** — and at depth 2 only for commands that have their own
parser. `muse trace inspect --help` works; `muse skills list --help` works; but an *unrecognized*
first token silently prints the top-level help with exit 0, which is the oracle used in §1.1.

### Exit codes observed

| Situation | Code |
|---|---|
| `muse exec --provider echo hi` (success) | 0 |
| `muse --help`, `muse --version` | 0 |
| unknown flag on `exec` | 2 |
| `muse plugins …` without the gate | 2 |
| `muse config validate` on an invalid document | 1 |
| unknown top-level token (falls to TUI, non-tty) | 1 |
| internal PTY-gate mode (blocks) | n/a (hangs) |

---

## 7. Things that surprised me

1. **`muse workflows` is a fully working, permanently-enabled command that appears in no help text**, and its
   own usage string admits it: *"not advertised in `muse --help`; kept for headless QA seeding and release smokes."*
   It reads/writes workflow scripts under `.agents/`, `.codex/` **and `.claude/`** project directories.
2. **`muse serve --listen` is parsed and refused with a roadmap note** (`deferred post-v1 (#13929)`) — a
   unix-socket/websocket MSP transport is designed but not shipped.
3. **`muse skills import --from claude|codex`** — first-class import of Claude Code and Codex skills,
   plus `CODEX_HOME` in the path resolver.
4. Only **2 of 41** experimental gates change the CLI surface. The other 39 are in-session.
5. `MUSE_NO_AUTO_UPDATE`, which every doc tells you to set, is **not read by the binary at all** — it is
   purely a launcher variable.
6. `MUSE_ENABLE_WEB_TOOLS` and `MUSE_WEB_SEARCH_MODE` are validated **at process start for every
   subcommand**, so a typo in either bricks `muse --version`.
7. Provider env vars for Anthropic, OpenAI and OpenRouter are in the registry even though
   `--provider` only accepts `echo|meta`.
8. `--subagent-worktree-isolation`'s help text is a 3-sentence apology for being a no-op compatibility flag.

## 8. Open questions

- What are the accepted values of `--eval-context-compaction-strategy`? The error prints no enum and
  the value set is not adjacent to the message in the string table.
- Are `--internal-claude-channels-sidecar-v1` / `--internal-claude-channels-live-smoke-host-v1` argv
  flags of the main binary or of a spawned sidecar? Neither is accepted by any parser I could reach.
- Which enterprise `permission_profiles` ids ship by default? Every guess reports "profile does not
  exist" on a machine with no managed document, and writing to `/Library/Application Support` was out
  of bounds for this pass.
- Does `MUSE_EXPERIMENTAL_SDK_ENABLED` expose anything at all outside a session? Nothing in argv.
- `TBH_INTERNAL_MODE` had no observable effect on macOS; needs a Linux host to exercise.

---

# Verification

Adversarial re-verification pass, run independently in
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/verify-cli-surface/`
with `HOME` redirected to a throwaway `fakehome`, no network, no auth, `--provider echo` only.
Flag candidates were re-mined from scratch (`strings -a -n 6` → 379 `--flag` literals → 570
hyphen-boundary prefixes, vs. the original 566) and re-swept against every parser.

**Verdict: MOSTLY_SOLID.** ~90% of the report reproduced byte-for-byte. Two claims are refuted, one
headline "oh-my" hook does not work as advertised, and a whole reachable subsystem was missed.

## Refuted

### R1. "No undocumented root flags" is false — two internal root flags are live

§3.1: *"A full sweep of 566 candidate long flags … against the root parser found **no undocumented
root flags** — every hit is in the table above."* and open question 2: *"Neither is accepted by any of
the five parsers I could reach."*

Both are wrong. An unknown root flag exits **2** with a clap error. These two exit 0 and 1 silently:

```
$ muse --internal-claude-channels-sidecar-v1 </dev/null; echo $?
0                                    # no stdout, no stderr
$ muse --internal-claude-channels-live-smoke-host-v1 </dev/null; echo $?
1                                    # no stdout, no stderr
$ muse --internal-bogus-v1 </dev/null; echo $?
2   invalid TUI options: error: unexpected argument '--internal-bogus-v1' found
```

The report's own sweep should have caught the sidecar flag; it is in `flags_prefix.txt` verbatim. The
live-smoke flag was missed for a different reason: the raw literal is
`--internal-claude-channels-sidecar-v1--internal-claude-channels-live-smoke-host-v1a known agent-run
payload type`, so a naive `grep -oE '--[a-z0-9-]+'` yields `…-host-v1**a**` and the real name is never
tested.

### R2. "Claude channels" is a working Claude Code interop bridge, not an open question

The sidecar flag is a **live JSON-RPC/MCP server on stdio**:

```
$ printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"claude-code","version":"2.1.220"}}}' \
  | muse --internal-claude-channels-sidecar-v1
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"experimental":{"claude/channel":{}},"tools":{}},"serverInfo":{"name":"tbh-session-messaging","version":"0.1.0"}}}
```

(`tools/list`, `prompts/list`, `hello`, `channel.send` all return `{"code":-32600,"message":"invalid_request"}`
without a live host; `TBH_SESSION_MESSAGE_SOCKET` is the missing piece.)

And the live-smoke host **shells out to the real `claude` binary on the machine**:

```
$ TBH_CLAUDE_CHANNELS_LIVE_SMOKE=1 TBH_CLAUDE_CHANNELS_SMOKE_ROOT=$PWD/smokeroot \
  TBH_CLAUDE_CHANNELS_SMOKE_DEADLINE_SECONDS=3 muse --internal-claude-channels-live-smoke-host-v1
claude_channels_live_smoke_host code=unsupported_claude_version stage=version observed_version=2.1.252
```

`observed_version=2.1.252` is this machine's installed Claude Code. Muse pins `2.1.220`.

Muse ships an entire **Claude Code plugin bundle** it installs through Claude Code's own CLI:

```
.claude-plugin
.claude-plugin/marketplace.json
plugins/tbh-session-messaging
plugins/tbh-session-messaging/.claude-plugin/plugin.json
plugins/tbh-session-messaging/.mcp.json
tbh_reply_session.jsonl
```
plus the driver argv `plugin` `install|enable|disable|update|uninstall` `--scope` `--yes` `--json`,
the marketplace ref `tbh-session-messaging@tbh-muse`, MCP display name `Muse Session Messaging`,
manifest keys `$schema version plugins category author mcpServers server displayName command args`,
wire roles `claude_sidecar` / `muse_host` / `transport_accepted` / `channel.send` / `hello`,
generation counters `connection_generation sidecar_generation mcp_generation callback_id`,
fixture `mcp-initialize.json`, and smoke codes
`unsupported_platform claude_unavailable unsupported_claude_version owned_path_conflict not_installed command_failed io`.
Env: `TBH_CHANNEL_ACTIVE_QUEUE_MARKER`, `TBH_CHANNEL_IDLE_WAKE_MARKER` (both absent from §4.3).

### R3. Project-scope workflows are never listed — the `.claude`/`.codex` hook does not work

Finding 2 and the `workflows` oh-my hook claim a framework can ship project workflows into
`.agents/.codex/.claude`. `save` writes them; `list` refuses to show them:

```
$ muse workflows save proj --from wf.js --scope project
Saved workflow "proj" to <ws>/.agents/workflows/proj.js     # note: .agents only, never .codex/.claude
$ muse workflows list
demo	user	$CONFIG_DIR/workflows/demo.js
(0 shadowed entries, 1 diagnostic)                           # proj is NOT listed
$ mkdir -p .claude/workflows .codex/workflows && cp .agents/workflows/proj.js .claude/workflows/a.js && cp … .codex/workflows/b.js
$ muse workflows list
demo	user	$CONFIG_DIR/workflows/demo.js
(0 shadowed entries, 3 diagnostics)                          # all three scanned, all three suppressed
```

Cause is a binary literal the report never found:
`project workflows skipped because the workspace is untrusted`
(beside `named workflows use the .js extension` / `named workflow entries must be regular files`).
`muse workflows` has **no** `--trust-workspace` and **no** `--workspace` flag (swept: `list` takes no
arguments at all; `save` takes exactly `--from`, `--scope`, `--overwrite`, `--help`), so there is no
argv route to make project workflows visible. Ship them at `--scope user` or not at all.

## Corrections

- **`MUSE_RELEASE_INFO` is not "exported *into* the binary"** in any meaningful sense: `grep -c` for
  `MUSE_RELEASE_INFO`, `RELEASE_INFO` and `release_info` over `strings -a -n 4` all return **0**. The
  launcher exports it; the binary never reads it — exactly the `MUSE_NO_AUTO_UPDATE` situation.
  (`MUSE_NO_AUTO_UPDATE` = 0 hits reconfirmed, including `grep -a` on raw bytes.)
- **`muse skills list` has an undocumented flag: `--scope`.** Accepted, validated, and inert:
  `muse skills list --scope all` → ``invalid value for --scope: all; expected user|project|built-in|plugin``;
  `--scope built-in` returns the identical 15 lines as no flag. It appears in no usage line. So the
  claim that only `exec` and `serve` carry undocumented flags is wrong.
- **The enterprise reason vocabulary is larger than the quoted literal**, and there are three outcome
  codes rather than one. Live: `notjson` → `enterprise_document_malformed: … reason=invalid_json`;
  `{"schema_version":99}` → `enterprise_schema_unsupported: … reason=unsupported_schema_version`;
  `{"schema_version":1,"execution":{"approval_modes":{}}}` on the policy plane →
  `enterprise_document_invalid: plane=policy reason=semantic_invalid location=execution.approval_modes`.
  `semantic_invalid` is not in §3.11's list; it lives in a separate 16-char blob with
  `skipped_oversize duplicate_member trailing_content null_not_allowed`. Fourth code in the binary:
  `enterprise_diagnostic_redaction_failed`.
- **The export flag sweep could not have proved what it claims.** `muse export --session` and
  `muse export --out` (no value) return the *identical* `unexpected export argument: --X` as a bogus
  flag, so presence is unobservable by that oracle. Re-swept with a value (`muse export <flag> __BOGUS__`)
  the conclusion does hold: exactly `--session`, `--last`, `--out`, `--redacted`, `--help`.
- **"36 built-in slash commands form a closed vocabulary" overstates the binary's own annotation**,
  which the report quotes: the list is `BUILTIN_SLASH_COMMAND_NAMES` **minus**
  `TELEMETRY_DROPPED_SLASH_COMMAND_NAMES`. It is the telemetry-reported subset, so 36 is a floor. A
  regex for runs of ≥6 `/token`s over the whole binary finds no second slash table, so the dropped
  names are not recoverable from strings.
- **Env-var counts.** The report's own command (`grep -oE 'TBH_[A-Z0-9_]+' strings5.txt | sort -u`)
  yields **82**, not "~110". After splitting concatenated literals the real figure is ~111 `TBH_*`
  (a handful are glue artifacts), of which **46**, not ~55, are `TBH_TMUX_*`. The §4.2 `MUSE_*`
  non-experimental table is exactly right: 16 real names, all present.
- **Exit codes.** Parse failure is uniformly **2** across all five parsers (including an unknown
  *root* flag, which §6's table omits). Code **1** is reserved for runtime/gate failures:
  ungated `session-message list` = 1 (not 2), `config validate` invalid = 1, `init` on an existing
  AGENTS.md = 1, `sandbox windows check` = 1, unknown top-level token on a non-tty = 1.
- `trace inspect --fixture __BOGUS__` does not merely "render an empty report": `--format json` shows
  `"load_status": "failed"`. Exit is 0.
- Payload types in §3.2 are abbreviated; the wire values are fully qualified
  (`task.lifecycle.accepted`, `task.lifecycle.scheduled`, `task.lifecycle.side_effect_intent`,
  `task.lifecycle.started`). The 13-record sequence, the fixed `id 018f0000-0000-7000-8000-00000000c350`
  and `recorded_at 1780531400000000` all reproduce exactly.

## Open questions now answered

1. **`--eval-context-compaction-strategy` values** (open q 1). Exactly three, and it needs an env var:
   ```
   prefix-extension-freeform-checkpoint/v1              → --eval-context-compaction-strategy requires TBH_CONTEXT_COMPACTION_EVIDENCE_PATH
   prefix-extension-freeform-checkpoint-with-suffix/v1  → (same)
   prefix-extension-freeform-checkpoint-with-suffix/v2  → (same)
   everything else (incl. all three production strategies, reminder-observation/v1) → unknown eval context compaction strategy `X`
   ```
   With the env set: `TBH_CONTEXT_COMPACTION_EVIDENCE_PATH requires a model provider` (so it is
   meta-only). Full strategy literal in the binary is 7 long: the 3 production ones, `reminder-observation/v1`,
   and these 3 eval-only ones.
2. **The `policy` plane's top-level containers** (open q 7). Not `settings` — four containers:
   `execution`, `model_egress`, `privacy`, `extensions`. Verified live (each `{"schema_version":1,"<c>":{}}`
   → `valid: plane=policy`; `settings` → `unknown_member` on policy and vice-versa on defaults).
   Dotted members from the binary: `execution.{permission_profiles,approval_modes,approval_reviewers,
   network_sandbox_modes,tool_rules.<id>,forbid_approval_bypass,forbid_sandbox_bypass,
   force_agent_definition_safe_mode,allow_project_configuration,allow_foreign_configuration,
   stop_hook_continuations}`, `model_egress.{allowed_providers,allowed_models,model_fallback,web_search,web_fetch}`,
   `privacy.{telemetry,foreign_personal_rules,foreign_personal_skills,local_session_messaging,feature_config}`,
   `extensions.{skills,runtime_capabilities,hooks}`.
3. **Which `permission_profiles` ship** (open q 3). None can: on the policy plane
   `execution.permission_profiles` returns `reason=field_not_activated`. The field is compiled in but
   switched off in 1.0.1, so `--permission-profile` is unsatisfiable in this build regardless of
   managed preferences. (`execution.approval_reviewers` and `execution.forbid_approval_bypass` are also
   `field_not_activated`; `execution.tool_rules` **is** activated.)
4. **`workflows run` / `recover`** (open q 6). Both run offline:
   `muse workflows run __nope__ --headless-qa` → `workflow launch failed for entry __nope__: workflow entry was not found` /
   `Recovery: fix the reported entry id, then rerun the command`.
   `--token-budget zzz` → `workflow token budget must be a positive integer, optionally suffixed with k or m`.
   `--live-auto-qa` → `workflow live auto-QA requires TBH_WORKFLOW_PRODUCT_LIVE_AUTO_QA=1`;
   `--prompt-live-smoke` → `… requires TBH_WORKFLOW_PROMPT_LIVE_SMOKE=1`.
   `recover` needs a log containing `workflow.execution.committed_parked_record`, and resolves
   `--session <uuid>` to `$XDG_DATA_HOME/muse/sessions/YYYY/MM/DD/<uuid>/session.jsonl`.

## Missed ground

- **Muse loads `~/.claude/skills` and `~/.codex/skills` natively, at user scope, with no import.**
  This is the single biggest omission — §3.5 frames Claude/Codex interop as `skills import` only.
  ```
  $ muse skills list --source user
  homeclaudeskill user on user scope probe … $HOME/.claude/skills/homeclaudeskill/SKILL.md
  homecodexskill  user on user scope probe … $HOME/.codex/skills/homecodexskill/SKILL.md
  homemuseskill   user on user scope probe … $CONFIG_DIR/skills/homemuseskill/SKILL.md
  ```
  Project scope likewise reads `.claude/skills` and `.codex/skills` beside `.agents/skills`
  (verified with `--trust-workspace`). Importing then produces a `skill-shadowed` diagnostic:
  `` skill `X` skipped because a higher-priority source defines the same id ($HOME/.claude/skills/X) ``.
  An oh-my-musecode that installs into `~/.claude/skills` is already live in Muse.
- **`skills import` artefacts and JSON schema** — never shown. `--json` emits
  `{source:{type,path}, dry_run, candidates:[{id,source_path,target_path,action,valid,classification,
  compatibility:{profile,result,known_fields,unknown_fields,unsupported_fields,allowed_tools},
  unavailable_binaries,missing_requires,diagnostics}], installed:[{installed:{id,path,version,revision,
  provenance:{source:{type,ecosystem,source_path,trust},content_sha256,files:[{relative_path,sha256,bytes}]}},
  lockfile_path,diagnostics}], quarantined, skipped, failed}`.
  Compat profile is `agent-skills-common-subset`, classification `portable`. It writes
  `$CONFIG_DIR/skills/.muse/lock.json`, `.muse/audit.log`, `.muse/.skills.lock`, `.muse/quarantine/`.
  Bundled skills live at `$XDG_DATA_HOME/muse/skills/bundled/muse-core/` (with a LICENSE).
  Text mode prints `candidates:N installed:N quarantined:N skipped:N failed:N`.
  `--scope project` is refused: `skills import --scope project is not supported in Phase 8`.
- **`skills list --json` schema** (only the TSV was documented): per skill
  `{id,name,display_name,description,short_description,scope,source:{type},path,activation,diagnostics,
  provenance,context_cost:{startup_bytes,startup_estimated_tokens,invoke_bytes,invoke_estimated_tokens}}`
  plus a top-level `diagnostics[]` with `{code,message,scope,path}` (e.g. `project-skills-untrusted`).
- **Two more parser dialects.** `muse serve zzz` → `muse serve: unexpected argument zzz; serve takes no
  positional arguments`; `muse schema zzz` → ``muse schema: unknown subcommand `zzz` ``. §6 lists neither.
- **`muse sandbox windows check` actually runs on macOS** and emits a structured report — §3.15
  implies the family is inert here:
  ```
  backend=windows_elevated
  status=setup_required
  reason=Windows elevated sandbox setup has not run
  diagnostic=windows_host_required:windows_elevated readiness can only pass on Windows
  ```
- **`muse init` re-run guard**: `AGENTS.md already exists. Pass --force to replace it, or --dry-run to preview.` (exit 1).
- **MSP method index is recoverable from strings** — relevant to both `serve` and `schema`:
  `session/start`, `session/resume`, `session/fork`, `session/compact`, `turn/start`, `turn/steer`,
  `turn/interrupt`, `turn/cancel`, `turn/unqueue`, plus event families
  `item/{started,updated,delta,completed,retracted,unqueued}`, `approval/{requested,updated,resolved}`,
  and control families `subagent/{send,interrupt,stop,resume,reopen,close,read}` and
  `goal/{list,edit,clear,pause,resume}`.
- `resume` accepts the whole root flag set **except** `--internal-claude-channels-sidecar-v1`, which is
  intercepted only as argv[1] (diffed sweep: one line of difference).

## Independently reproduced (unchanged)

16 top-level commands from a fresh 179-name sweep (identical set); the dispatcher literal at
strings5.txt:111930; the 41 gates and their 1:1 id mapping (re-extracted from the binary directly,
identical); only `PLUGINS` and `EXTERNAL_AGENT_INGRESS` move any surface (all 41 gates × root-help,
exec-help, built-in skills, and 163 non-command names re-swept: no other hit); the three hidden `exec`
flags plus `--api-key`'s refusal (43 accepted `exec` flags, exactly 3 absent from `--help`);
`serve --listen` and the full serve refusal family; every enum error message verbatim;
`MUSE_ENABLE_WEB_TOOLS`/`MUSE_WEB_SEARCH_MODE` as the *only* two env vars that abort process start
(all 107 `MUSE_`/`TBH_` names swept with both `1` and a bogus value); short flags `-h -V -w` / `-h -w`;
schema fingerprints `sha256:03312c21…` and `sha256:577d717d…`, unchanged under gates; the export
document shape and `e27e408b66` build sha; `muse init` writing only AGENTS.md; the 10 `skills`
subcommands; the 12 `plugins` verbs; the 36-name slash blob; `session-message send` stub;
`__tbh_internal_process_owner_pty_gate_v1` exit 125 vs 1; the launcher's 12 vars and `exec "$binary" "$@"`.
