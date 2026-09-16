# SETTLED: Do plugin-provided MCP servers ever start?

**Answer: YES. In every lane — `muse exec`, the TUI, and `muse serve` — an installed plugin's
stdio MCP server is spawned, completes a full MCP handshake, and its tools reach the model
surface as `mcp__plugin_<plugin-id>_<server-id>__<tool>`.**

The prior report's negative (open question #1 in `00-SYNTHESIS.md` §7, claim 11b) is **refuted**.
It was not a lane problem. It was a **trust-state** problem: a plugin MCP server only starts when
its runtime capability is `trusted_enabled`. In `review_needed`, `trusted_disabled` or `modified`
it is silently skipped — no `mcp.*` records, no diagnostic, no stderr — *while the plugin
capability snapshot still reports `mcp_servers=N`*, because that number counts **declared**
capabilities, not **active** ones. That is exactly the fingerprint the earlier pass observed.

Target: `muse` `1.0.1-R2006.1`, build `e27e408b66`, arm64 macOS.
Binary: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
Sandbox: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/plugin-mcp/`
No login, no credential, no request ever left the machine. Every session used `--provider echo`
or `settings.json → "provider": "echo"`.

---

## 0. One-command reproduction

```bash
bash /private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/plugin-mcp/repro.sh
```

Builds a clean `HOME`, writes a self-logging stdio MCP server, and runs four lanes. Verbatim
output of the last run:

```
### LANE A  settings.json mcpServers + muse exec
    spawned: 1 ; tools: mcp__settingsprobe mcp__settingsprobe__probe_ping
### LANE B1 plugin installed, capability NOT approved (default state)
    runtime-capability	plugin:ommprobe:mcp_server:pluginprobe	status=review_needed
    spawned: 0   <-- expect 0
### LANE B2 same plugin, capability APPROVED
    runtime-capability	plugin:ommprobe:mcp_server:pluginprobe	status=trusted_enabled
    spawned: 1   <-- expect 1
    "canonical_id":"mcp__plugin_ommprobe_pluginprobe__probe_ping"
    "canonical_id":"mcp__settingsprobe__probe_ping"
### LANE C muse serve + session/start (settings.provider=echo)
    spawned: 1   <-- expect 1
```

---

## 1. Instrument

`settle/plugin-mcp/mcp_server.py` — a stdio MCP server that appends one line to an absolute log
file the instant it is executed (before any protocol), then answers `initialize`, `tools/list`,
`tools/call`, `resources/list`, `prompts/list`. It accepts **both** line-delimited-JSON and
`Content-Length` framing and mirrors whichever it received. `settle/plugin-mcp/probe.sh` is a
`/bin/sh` wrapper that logs `sh-exec` *before* handing over to python, so a python failure would
still prove the spawn.

Each log line carries: UTC ms timestamp, LANE tag, TAG (`sh-exec|spawn|recv|sent|stdin-eof|exit`),
PID, PPID, CWD, the resolved script path, argv and the child's environment.

---

## 2. Results, lane by lane

| # | Lane | Trust state | log written? | `mcp.startup` task | `runtime.mcp_tool_identity_catalog` | `mcp__*` in `active_tools` |
|---|---|---|---|---|---|---|
| B0 | `exec`, no MCP anywhere | — | — | absent | absent | — (20 tools) |
| L1 | `exec`, **settings.json `mcpServers`** | n/a | **YES** | proposed→…→completed | `mcp__settingsprobe__probe_ping` | **yes** (21 tools) |
| L2 | `exec`, **plugin** `capabilities.mcpServers[]` | `trusted_enabled` | **YES** | proposed→…→completed | `mcp__plugin_ommprobe_pluginprobe__probe_ping` | **yes** (21 tools) |
| L2b | `exec`, plugin, **no `MUSE_EXPERIMENTAL_PLUGINS`** | `trusted_enabled` | **YES** | present | present | **yes** |
| L2c | `exec`, plugin | `trusted_disabled` (rejected) | no | **absent** | absent | no (20 tools) |
| L2e | `exec`, plugin | `review_needed` (fresh install) | no | **absent** | absent | no (20 tools) |
| L2f | `exec`, plugin | `modified` (after `plugins update`) | no | **absent** | absent | no (20 tools) |
| L3 | **TUI** under `drive.py` pty, plugin | `trusted_enabled` | **YES** | present | present | **yes** (30 tools) |
| L4 | `serve` MSP `session/start`, plugin, **no `settings.provider`** | `trusted_enabled` | no | absent | absent | n/a |
| L4b | `serve` + `session/userShell`, no `settings.provider` | `trusted_enabled` | no | absent | absent | n/a |
| L4d | `serve` MSP `session/start` + `turn/start`, **`settings.provider="echo"`** | `trusted_enabled` | **YES** | present | present | **yes** (23 tools, all 3 servers) |
| L5 | `exec`, settings **and** plugin together | `trusted_enabled` | **YES, both** | present | **both entries** | **both** (28 tools) |
| L6 | `exec`, settings + **two** plugins | `trusted_enabled` | **YES, all three** | present | three entries | all three |

### 2.1 Positive control (L1) — verbatim child log

```
LANE=settings-exec TAG=sh-exec PID=3888 CWD=…/settle/plugin-mcp/ws SELF=…/probe.sh ARGS=settings-exec
LANE=settings-exec TAG=spawn   PID=3888 PPID=3884 ENV={"HOME":"…/settle/plugin-mcp/home","PWD":"…/ws"}
LANE=settings-exec TAG=recv    FRAMING=line BODY={"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"tbh","version":"0.1.0"}}}
LANE=settings-exec TAG=sent    FRAMING=line BODY={"jsonrpc":"2.0","id":1,"result":{…"serverInfo":{"name":"ommprobe-settings-exec"…}}}
LANE=settings-exec TAG=recv    FRAMING=line BODY={"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
LANE=settings-exec TAG=recv    FRAMING=line BODY={"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
LANE=settings-exec TAG=sent    FRAMING=line BODY={"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"probe_ping"…}]}}
```

Muse's MCP **client** identifies as `{"name":"tbh","version":"0.1.0"}`, speaks
`protocolVersion 2024-11-05`, uses **line-delimited JSON** (not `Content-Length`) when
`framing` is left at its default, and issues exactly `initialize` → `notifications/initialized`
→ `tools/list`.

### 2.2 The plugin lane (L2) — same handshake, richer environment

```
LANE=plugin TAG=sh-exec PID=5334 CWD=…/settle/plugin-mcp/ws
  SELF=…/home/.local/share/muse/plugins/cache/local/ommprobe/<pkg-sha256>/package/mcp/probe.sh
LANE=plugin TAG=spawn PPID=5329 ENV={
  "CLAUDE_PLUGIN_ROOT":  "…/plugins/cache/local/ommprobe/<sha>/package",
  "MUSE_PLUGIN_ROOT":    "…/plugins/cache/local/ommprobe/<sha>/package",
  "MUSE_PLUGIN_ID":      "ommprobe",
  "MUSE_PLUGIN_DATA_DIR":"…/plugins/data/ommprobe",
  "HOME": "…", "PWD": "…/ws" }
```

Then the identical `initialize` / `notifications/initialized` / `tools/list` exchange.

Session record (`session.jsonl`, seq 30):

```json
{"schema_version":1,"kind":"mcp_tool_identity_catalog","entries":[
  {"canonical_id":"mcp__plugin_ommprobe_pluginprobe__probe_ping",
   "surface":{"namespace":"mcp__plugin_ommpr__b56b5d65b6ce","name":"probe_ping"},
   "server_name":"plugin:ommprobe:pluginprobe","raw_tool_name":"probe_ping"}]}
```

and the tool surface handed to the model (seq 42):

```json
"active_tools":["workflow","read_file",…,"read_skill",
                "mcp__plugin_ommprobe_pluginprobe__probe_ping","write_todos"]
```

### 2.3 The full `mcp.startup` task lifecycle (identical for both sources)

```
runtime.session.task  {"event":{"kind":"proposed","task_kind":"mcp.startup"}}
runtime.session.task  {"event":{"kind":"accepted"}}
runtime.session.task  {"event":{"kind":"scheduled","idempotency_key":"mcp.startup:<uuid>"}}
runtime.session.task  {"event":{"kind":"side_effect_intent","operation":"mcp.startup.batch",
                        "policy_decision":"allow:mcp_configuration_admitted",
                        "cancellation_handle":"runtime-task:<id>"}}
runtime.session.task  {"event":{"kind":"started"}}
runtime.mcp_tool_identity_catalog  { … }
runtime.session.task  {"event":{"kind":"completed"}}
```

Note there is **one** `mcp.startup` batch task for the whole session, whatever the number or
source of servers. `policy_decision` is the literal `allow:mcp_configuration_admitted`.

---

## 3. Why the earlier pass measured zero — proven, not guessed

### 3.1 The snapshot count is the *declared* count, not the *active* count

With **both** plugin MCP capabilities set to `trusted_disabled`:

```
$ muse plugins inspect ommprobe  | tail -1
runtime-capability	plugin:ommprobe:mcp_server:pluginprobe	status=trusted_disabled
$ muse plugins list --json >/dev/null      # composes the snapshot
$ grep plugin_capability_snapshot.compose <newest local-tracing/bootstrap/cli-*.log>
event="plugin_capability_snapshot.compose" outcome="ready" reason="none" installed_admitted=true
  bundled_plugins=3 installed_plugins=2 marketplace_plugins=0 plugin_ids=11 skills=16 hooks=0
  mcp_servers=2 commands=1 reminders=6 … duration_ms=6
$ muse exec --provider echo "both rejected"
# child log: only LANE=settings-combined spawned. Neither plugin server ran.
# session.jsonl catalog: only mcp__settingsprobe__probe_ping.
```

`mcp_servers=2` with **zero** servers started. This is the exact signature the previous report
called "the capability snapshot WAS composed (`mcp_servers=1`) but zero `mcp.*` records".

### 3.2 Three distinct states all produce the silent-skip

| state | how you get there | observable |
|---|---|---|
| `review_needed` | **the default immediately after `muse plugins install`** | nothing, ever |
| `trusted_disabled` | `muse plugins reject …` | nothing, ever |
| `modified` | **any** byte changes in the package, then `muse plugins update` | nothing, ever |

The `modified` trap is the nastiest: I appended one unrelated file (`NOTES.md`) to the plugin
source and ran `muse plugins update ommprobe`. The install warning came back, the status flipped
to `modified`, and the next `muse exec` silently ran with 20 tools and no MCP process. The binary
literal for this path is `plugin trust reverts to review_needed`.

### 3.3 `muse serve` needs a resolvable provider before it composes anything

`serve` was never MCP-blind. Without a provider it never builds the agent runtime at all:

```
# settings.json WITHOUT "provider"
serve → session/start → session.jsonl has 5 records, no agent_tree_initialized, no mcp.startup
serve → turn/start   → active_tools:["write_todos"]  (one tool!)
                     → task failed: "not logged in: run /login to add an API key"
                     → terminal: failed
# and no plugin_capability_snapshot.compose anywhere in that process's trace.
```

Add one line to `$XDG_CONFIG_HOME/muse/settings.json`:

```json
{ "schema_version": 1, "provider": "echo" }
```

…and the very same `serve` + `session/start` composes the snapshot, fires `mcp.startup`, spawns
the plugin server and emits the tool identity catalog. A `turn/start` on that session then reports

```json
"active_tools":["workflow",…,"read_skill","mcp__settingsprobe__probe_ping",
                "mcp__plugin_ommprobe2_probe2__probe_ping",
                "mcp__plugin_ommprobe_pluginprobe__probe_ping","write_todos"]   // 23 tools
```

with `turn/completed` and `"delta":"echo: serve turn probe"` — entirely offline. The correlation is perfect across six
`serve` runs (traces at 02:34:35, 02:35:23, 02:37:10 → no provider, no
`plugin_capability_snapshot.compose`; 02:36:03, 02:36:39, 02:37:32 → provider set, compose
present).

**Side finding that matters beyond this question:** `msp-protocol.md` §1 says "`muse serve` cannot
select the provider on the wire" and concludes a `serve` host always dies with `modelError`.
True on the wire — but **`settings.json → provider` does route it**, and a full offline
`turn/start` → `turn/completed` with `"delta":"echo: serve turn probe"` was observed. The MSP
lane is fully exercisable offline; that report's §1 limitation should be amended.

### 3.4 The "needs restart" strings are about *mid-session mutation*, not about `exec`

Byte-exact neighbourhood in the binary (`strings -a`, ~line 608 960):

```
applied plugin skills from ⟨name⟩; hooks/MCP still need restart
removed plugin ⟨name⟩; apply plugin skills for this session, restart for hooks/MCP
updated plugin ⟨name⟩; apply plugin skills for this session, restart for hooks/MCP
plugin trust reverts to review_needed
```

They sit among TUI drawer-action strings (`unsupported drawer action:`, `Approval decision failed:`).
They mean: install/update/remove a plugin **from inside a running session** and its skills hot-apply
but hooks and MCP servers require a *new session*. They never meant "MCP is TUI-only".

---

## 4. Mechanics worth writing down for oh-my-musecode

### 4.1 The child environment is scrubbed to an allowlist

Full `os.environ.keys()` of the spawned server:

```
settings-declared: ["HOME","LANG","LOGNAME","PATH","PWD","SHELL","SHLVL","TERM","TMPDIR","USER",
                    "__CF_USER_TEXT_ENCODING"]
plugin-declared:   the same 11, PLUS
                   ["CLAUDE_PLUGIN_DATA","CLAUDE_PLUGIN_ROOT","MUSE_PLUGIN_DATA_DIR",
                    "MUSE_PLUGIN_ID","MUSE_PLUGIN_ROOT","PLUGIN_DATA","PLUGIN_ROOT"]
```

`PATH` is inherited **verbatim** from the muse process. Nothing else is — no `MUSE_*` gates, no
custom vars from the parent shell. A settings-declared server can add more via `mcpServers.<id>.env`;
a **plugin** server cannot (native manifests silently drop `env`/`cwd`/`headers`), so a plugin MCP
server must take its configuration from `MUSE_PLUGIN_DATA_DIR` / files under `MUSE_PLUGIN_ROOT`,
or from the `command` argv.

`MUSE_PLUGIN_DATA_DIR` = `$XDG_DATA_HOME/muse/plugins/data/<plugin-id>` — a per-plugin writable
directory. This is the natural home for an oh-my-musecode server's state.

### 4.2 Command resolution

* `command[0]` is resolved through `PATH` — a bare `python3` works (verified with a second plugin
  whose manifest is verbatim Meta's own `create-plugin` example
  `{"id":"probe2","transport":"stdio","command":["python3","mcp/server.py"]}`; it spawned and
  registered `mcp__plugin_ommprobe2_probe2__probe_ping`).
* `command[1]` is taken as **package-relative** and rewritten to the absolute path inside the
  content-addressed cache: `…/plugins/cache/local/<id>/<package_sha256>/package/<rel>`. The
  validator surfaces it as `source_relative_path`.
* The child's **cwd is the workspace root**, *not* the plugin root. Anything the server needs from
  its own package must be reached via `$MUSE_PLUGIN_ROOT`.
* Absolute `command[0]` also works (`/bin/sh mcp/probe.sh plugin`).

### 4.3 Tool naming, and a namespace budget

```
settings server  → canonical_id  mcp__settingsprobe__probe_ping
                   namespace     mcp__settingsprobe                       (18 chars, verbatim)
plugin server    → canonical_id  mcp__plugin_<plugin-id>_<server-id>__<tool>
                   server_name   plugin:<plugin-id>:<server-id>
short  namespace   mcp__plugin_ommprobe2_probe2                           (28 chars, verbatim)
long   namespace   mcp__plugin_ommpr__b56b5d65b6ce                        (31 chars, TRUNCATED+HASHED)
                   …from the 32-char mcp__plugin_ommprobe_pluginprobe
```

Observed: a namespace of 28 chars survives intact; one of 32 is rewritten to a 17-char prefix +
`__` + a 12-hex-digit digest. The exact cut-off was not measured (it is somewhere in 29–31).
**Design consequence:** keep `<plugin-id>` + `<server-id>` short or the model sees an opaque hash
in the tool namespace. `canonical_id` is not truncated.

### 4.4 Approval is fully non-interactive and scriptable

```bash
export MUSE_EXPERIMENTAL_PLUGINS=1                       # required for the `plugins` CLI only
muse plugins install ./bundle --scope user --json
muse plugins approve plugin:<id>:mcp_server:<server> --json
#   → {"decision":"approve","runtime_capabilities":[
#        {"stable_id":"plugin:<id>:mcp_server:<server>",
#         "trusted_definition_hash":"sha256:…","enabled":true}]}
```

No TTY needed. The result is written to `$XDG_CONFIG_HOME/muse/settings.json → runtime_capabilities`.
`MUSE_EXPERIMENTAL_PLUGINS` is **not** needed at session time — L2b ran `muse exec` with the gate
unset and the plugin server still started. The gate only guards the `plugins` sub-command
(`plugins are not available in this build`, exit 0).

### 4.5 Settings and plugin servers coexist

Three servers ran simultaneously in one `muse exec` (one from `settings.mcpServers`, two from two
plugins) and all three tools landed in one catalog and one `active_tools` list. No conflict, no
precedence rule triggered.

---

## 5. What is still NOT established

1. **End-to-end `tools/call`.** The `echo` provider never emits tool calls, and the `tool_specs`
   lane in `model_input_trace_recorded` is `byte_count: 0` under `echo` — so the tool *schema*
   was never serialised to a provider request in any of these runs. What is proven is that the
   server is spawned, handshakes, its tools are catalogued, and its canonical id is in the
   runtime's own `model_request_configured.toolset.active_tools`. Proving an actual invocation
   needs a provider that emits tool calls, which needs credentials.
2. **The exact namespace truncation threshold** (29, 30 or 31 chars) and the hash input.
3. **`transport: "http"` plugin servers** — untested; only `stdio` was exercised.
4. **Startup failure semantics for a plugin server** (a plugin manifest has no `mode:
   required|optional` and no `startup_timeout_sec`; what happens when a plugin server hangs or
   exits was not measured).

---

## 6. Failed attempts / dead ends, in order

1. `session/close` — **not an MSP method.** `{"code":-32601,"message":"method not found",
   "data":{"kind":"methodNotFound"}}`. Kill the process instead.
2. `muse plugins uninstall <id>` — **does not exist**: `unsupported plugins command \`uninstall\``,
   followed by the root help text. To return a plugin to `review_needed` you must delete the
   `runtime_capabilities` entry from `settings.json` and re-install.
3. `muse serve` + `session/start` alone → no MCP. Wasted two runs before realising the
   agent runtime is not built at all without a provider (no `agent_tree_initialized` record).
4. `muse serve` + `session/userShell` → still no MCP. `userShell` runs without the model *and*
   without the runtime, so it is useless as a runtime-composition trigger.
5. First TUI script sent `"hello tui\r"` after 12 s; the keystroke was swallowed and no turn ever
   ran (`session.jsonl` had 28 records, zero `runtime.user_intent.*`). The MCP server had already
   started, so the lane still answered the question, but for a *turn* you must type the text and
   send `\r` as two separate writes with a wait between them (see `tui_script2.json`).
6. Bootstrap traces (`$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-*.log`) carry
   `plugin_capability_snapshot.compose` for `plugin_mutation`- and `serve`-mode processes but
   **not** for `exec`- or `tui`-mode processes, and never carry any `mcp.*` line. `session.jsonl`
   is the instrument for MCP; the bootstrap trace is the instrument for gates, path resolution and
   snapshot composition.
7. Trying to read the tool *schema* byte budget to prove schema delivery: `tool_specs` →
   `request.tools` shows `byte_count: 0` under `--provider echo`. Dead end without credentials.

---

## 7. Files

| Path | What |
|---|---|
| `…/settle/plugin-mcp/repro.sh` | Clean-room reproduction, 4 lanes, ~20 s |
| `…/settle/plugin-mcp/mcp_server.py` | Self-logging stdio MCP server (dual framing) |
| `…/settle/plugin-mcp/probe.sh` | `sh` wrapper that logs before exec'ing python |
| `…/settle/plugin-mcp/env.sh` | Sandbox env (`HOME`, `XDG_*`, `MUSE_NO_AUTO_UPDATE`, `M`) |
| `…/settle/plugin-mcp/summarize.py` | Extracts `mcp.*`, identity catalog and `active_tools` from the newest `session.jsonl` |
| `…/settle/plugin-mcp/drive_settle.py` | pty TUI driver (fork of `sandbox/tui/drive.py`) that polls the MCP log between steps |
| `…/settle/plugin-mcp/srv/t_serve2.py` | MSP client: `initialize` → `session/start` → `turn/start`, prints MCP-log size before and after the turn |
| `…/settle/plugin-mcp/plug/`, `plug2/` | The two probe plugin bundles |
| `…/settle/plugin-mcp/logs/L*.log` | Per-lane child-process invocation logs |
| `…/settle/plugin-mcp/logs/tui_out*.bin` | Raw TUI pty captures (render with `sandbox/tui/render.py`) |

---

## 8. What this changes for oh-my-musecode

**A bundle CAN ship tools.** The install story stays "one plugin bundle", it does **not** have to
degrade to writing `settings.json → mcpServers`. But the framework must own the trust lifecycle:

1. `omm install` must run `muse plugins approve plugin:<id>:mcp_server:<server> --json` after
   `muse plugins install`, and must **verify** the result — check `plugins inspect` reports
   `trusted_enabled`, not just that approve exited 0. An unapproved MCP server is invisible: no
   error, no warning at session time, just a missing tool.
2. `omm upgrade` is the dangerous verb. Any change to the bundle flips every runtime capability to
   `modified` and silently kills the MCP server. Upgrade must be `plugins update` **followed by**
   re-approve, and `omm doctor` must report any capability whose status is not `trusted_enabled`
   as a hard failure with the fix command.
3. `omm doctor` gets three new checks: (a) every declared `mcp_server` is `trusted_enabled`;
   (b) `settings.json → provider` is set if the user runs `muse serve`/SDK hosts, otherwise their
   sessions have one tool and no MCP; (c) the `mcpServers` + `mcp_servers` collision (already
   known) still voids the *settings* lane but not the plugin lane.
4. Keep plugin id + server id under ~28 characters combined so the model-visible namespace is not
   replaced by a hash.
5. Plugin MCP servers get no `env`. Put configuration in files under `$MUSE_PLUGIN_ROOT`, state in
   `$MUSE_PLUGIN_DATA_DIR`, and remember the process cwd is the **workspace**, not the plugin.
6. `MUSE_EXPERIMENTAL_PLUGINS=1` is an *install-time* requirement only. Users do not need it in
   their shell rc for an installed bundle's MCP tools to work.

---

# Verification

**Independent adversarial re-run, clean sandbox, 2026-09-02.**
Verdict: **CONFIRMED.** The headline answer holds, the causal mechanism holds, and every lane
reproduces. Five corrections and three extensions are recorded below; none of them touch the
top-line answer.

Sandbox: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/verify-plugin-mcp/`
(fresh `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME` per run — `run1`…`run4`, `verify-run` — never a reuse
of `settle/plugin-mcp/home`). Binary `muse-aarch64-macos`, `sha256
b9c7f9badb6b2af1b362d30202b366e7bdc13b3c4048e9002caa236cc56c54a4`. macOS 26.6.2, python 3.14.6.
No login, no `muse auth`, no credential, no request to any non-loopback host.

## V.0 Their script, run verbatim from a clean HOME

```
$ bash .../settle/plugin-mcp/repro.sh .../settle/verify-plugin-mcp/run1     # 8.0 s
### LANE A  settings.json mcpServers + muse exec
    spawned: 1 ; tools: mcp__settingsprobe mcp__settingsprobe__probe_ping
### LANE B1 plugin installed, capability NOT approved (default state)
    runtime-capability	plugin:ommprobe:mcp_server:pluginprobe	status=review_needed
    spawned: 0   <-- expect 0
### LANE B2 same plugin, capability APPROVED
    runtime-capability	plugin:ommprobe:mcp_server:pluginprobe	status=trusted_enabled
    spawned: 1   <-- expect 1
### LANE C muse serve + session/start (settings.provider=echo)
    spawned: 1   <-- expect 1
```

Reproduces exactly. The four `session.jsonl` files it leaves behind carry the discriminating
evidence the script itself does not print:

| lane | `mcp.startup` | `mcp_tool_identity_catalog` | `active_tools` |
|---|---|---|---|
| A (settings) | present | `mcp__settingsprobe__probe_ping`, `server_name:"settingsprobe"` | 21 |
| B1 (`review_needed`) | **absent** | absent | 20 |
| B2 (`trusted_enabled`) | present | `mcp__plugin_ommprobe_pluginprobe__probe_ping`, `server_name:"plugin:ommprobe:pluginprobe"` | 21 |
| C (`serve`) | present | same plugin entry | (no turn) |

**Positive control passed** (lane A produced a `settingsprobe` catalog entry) and, critically,
**the plugin lane cannot be the settings lane in disguise**: `repro.sh` deletes `mcpServers` from
`settings.json` before installing the plugin, the child-process log line for lanes B2/C reads
`LANE=plugin`, the spawned path is inside `.../plugins/cache/local/ommprobe/<pkg_sha256>/package/`,
and the catalog's `server_name` is the literal `plugin:ommprobe:pluginprobe`. There is no
mechanism-confusion here.

## V.1 The control their design was missing, and it strengthens the result

Their bundle shipped **only** an MCP server, so "0 spawns at `review_needed`" was equally consistent
with a boring alternative: *the plugin never loaded at all*. I rebuilt the experiment (`run2`) with a
bundle that ships **one skill + one command + one MCP server** (`omv` / `srv`), which separates the two:

```
### 1  install -> DEFAULT trust state
    warning  third-party plugin: MCP servers require review before activation;
             skills are active without review while the plugin is enabled
    runtime-capability  plugin:omv:mcp_server:srv  status=review_needed
    spawned=0                              tools=20  mcp_tools=NONE
    DISCRIMINATOR: plugin skill marker in that same session = 1   <-- the skill IS there
```

The plugin's `SKILL.md` marker string appears in the very session where the MCP server did not run.
**The plugin is loaded and active; only the MCP capability is withheld.** Their conclusion is not
just reproduced, it is now properly controlled.

## V.2 Causality, not correlation: the gate is reversible, and there are *four* of them

`verify.sh` (below) drives the state machine both directions and finds **four independent gates**,
where the report names one and a half:

```
### 2  approve            spawned=1   tools=21  mcp_tools=['mcp__plugin_omv_srv__vping']   lane tag: LANE=PLUGIN
### 3  reject             spawned=0
     re-approve           spawned=1                      <-- reversible; not an ordering artifact
### 4  plugins disable    spawned=0                      <-- SECOND GATE (new)
     plugins enable       spawned=1
### 5  touch a file + plugins update -> status=modified
                          spawned=0                      <-- THIRD GATE (as reported)
     re-approve           spawned=1
### 6  all capabilities rejected:
     trace says mcp_servers=1 commands=1
     actually spawned=0                                  <-- declared != active, confirmed
```

* **Gate 2 is new and matters for `omm doctor`:** `muse plugins disable <id>` yields
  `enabled=false active=false`, the MCP server does not start, **and the `runtime-capability` line
  disappears from `plugins inspect` entirely** — so a doctor check that greps for
  `status=trusted_enabled` will see *nothing at all* rather than a bad status. It must assert the
  line is present *and* `trusted_enabled`. `plugins enable` restores the server with the trust
  decision intact.
* **Gate 4 is new and is TUI-only:** see V.5.

## V.3 §3.1 confirmed at scale, and the mechanism behind `modified` identified

Scaled the "declared ≠ active" test to seven servers in one bundle (`run3`), all
`trusted_disabled`:

```
event="plugin_capability_snapshot.compose" outcome="ready" reason="none" installed_admitted=true
  bundled_plugins=3 installed_plugins=1 marketplace_plugins=0 plugin_ids=10 skills=15 hooks=0
  mcp_servers=7 commands=1 reminders=6 developer_prompts=0 diagnostics=0
  rejected=0 conflicts=0 omitted=0 foreign_artifacts=0 duration_ms=3
$ muse exec --provider echo "all rejected"   ->  spawns: 0
```

`mcp_servers=7`, `rejected=0`, **seven servers, none running.** Note `rejected=0` too — the compose
record's `rejected` counter does *not* track capability trust either. Nothing in that trace line can
be used as a health signal.

Why a stray file revokes trust (`run2`, step 7): adding an unrelated `EXTRA.md` left
`manifest_sha256` **unchanged** (`sha256:7cdcbf39…` before and after) but changed
`package_sha256` (`969606d4…` → `1fbe68fd…`), which changes the content-addressed cache path, which
is baked into the capability's resolved `command`, which changes `trusted_definition_hash`
(`sha256:a5f47e36…` → `sha256:761af3f4…`). That is why `plugins update` reports the same plugin hash
yet flips the capability to `modified`. **Any byte anywhere in the package** — a README, a
`.DS_Store` — revokes MCP trust. Their warning is right and the mechanism is now pinned.

## V.4 `serve`: the provider finding is right, and there is a better instrument than timestamps

Reproduced both halves in `run2` and found the binary's own signal, which beats the report's
six-run timestamp correlation:

```
mode="serve", settings has no "provider":   provider/cache.rs:47 provider_source="none"      compose=0
mode="serve", settings.provider="echo":     provider/cache.rs:47 provider_source="settings"  compose=2
```

`provider_source` in the bootstrap trace is the direct read-out. Confirmed downstream:

* **No `provider` in settings** (with `providerId:"echo"` present on the wire and recorded in
  `runtime.session.metadata` as `provider_id:"echo"`): 21 records, `active_tools:["write_todos"]`,
  task `model.unknown.response` → `failed`, `reason: "not logged in: run /login to add an API key"`,
  0 MCP spawns. The task kind is literally `model.**unknown**.response` — the wire `providerId` is
  stored but never resolves a provider. This is a clean confirmation of `msp-protocol.md` §1's
  "cannot select the provider on the wire", and of this report's amendment to it.
* **`provider:"echo"` in settings**: plugin MCP server spawns during `session/start` (mcp log 0 →
  2947 bytes before any `turn/start`), `active_tools` = 15 including
  `mcp__plugin_omv_srv__vping`, and `turn/start` → `assistant_message_committed
  {"response_id":"muse-tui-echo","text":"echo: serve turn probe"}` → `terminal: completed`,
  fully offline.

Also confirmed their failed-attempt #6 by census of `run2`'s 18 bootstrap traces:
`plugin_capability_snapshot.compose` appears in `mode="plugin_mutation"` (2×) and `mode="serve"`
(1×, only with a provider), and **never** in the 7 `mode="exec"` or 3 `mode="tui"` traces.

## V.5 CORRECTION — the TUI lane has a fourth gate the report missed, and it explains their own anomaly

The report's failed-attempt #5 says a first TUI keystroke was "swallowed" and no turn ran, and its
evidence block says the server "spawned at TUI session open" after an 8 s wait. Both are artifacts of
something else. A TUI launched in an **untrusted workspace** never opens a session at all:

```
Do you trust this workspace?
Workspace: .../verify-run/ws
Trusting allows project-local skills, rules, hooks, and plugin config to load before the model runs.
Only trust this workspace when you trust its contents.
>  1  Trust and continue
   2  Quit
```

Three consecutive 12 s pty runs with no input: `mcp-log bytes=0` throughout, capture only 1107 bytes.
Answer the prompt and the server is up immediately:

```
TUI A (untrusted ws):  MARK at-trust-prompt bytes=0 ; send "1" then "\r" ; MARK t+1 bytes=912 ...
                       spawns=1  lane=LANE=PLUGIN
TUI B (ws now trusted, zero keystrokes):  t+1 bytes=912   t+2 912   t+4 912   t+8 912
```

Their "swallowed keystroke" was `h` being rejected by the dialog and their `\r` selecting
*"1 Trust and continue"* — which is why a turn never ran and why the server appeared only after they
typed. **Corrected statement:** in the TUI the plugin MCP server starts at session open, within ~1 s,
with no input — but session open is itself blocked by the workspace-trust dialog on first use of a
workspace. `muse exec` and `muse serve` have no such gate (every `exec` above ran to completion, with
a spawned plugin MCP server, in a workspace that was never trusted).

A full TUI turn was independently reproduced (`run2`, workspace already trusted): 30 `active_tools`
including `mcp__plugin_omv_srv__vping`, `agent_tree_initialized: true`.

## V.6 CORRECTION — "every failure mode here is silent" is too strong

It is silent **at session time** — no stderr, no session record, no trace line, confirmed. But the
plugin CLI is explicit, at install *and* on every later inspect:

```
$ muse plugins install ./pkg --scope user --json     # (on stdout, alongside the JSON)
warning  third-party plugin: MCP servers require review before activation;
         skills and commands are active without review while the plugin is enabled
$ muse plugins inspect omv | tail -2
warning  third-party plugin: MCP servers require review before activation; ...
runtime-capability  plugin:omv:mcp_server:srv  status=review_needed
```

The prior run never saw this because `repro.sh` discards install output (`>/dev/null 2>&1`).
For `omm install` this is good news: the binary tells you, in words, exactly what to do.

## V.7 CORRECTION — `muse plugins remove` exists; `uninstall` is just the wrong verb

Their failed-attempt #2 concludes you must hand-edit `settings.json` to reset a plugin. Not so —
`muse plugins --help` lists a verb they did not try:

```
remove <id> [--delete-data] [--json]   Remove an installed plugin record and cached package
```

Verified: `plugins remove omv` deletes the cache **and** strips `runtime_capabilities` from
`settings.json` (it went back to `{ "schema_version": 1, "provider": "echo" }`); a re-install then
lands at `review_needed`. That is the correct `omm uninstall` primitive. The full verb list is
`install / list / inspect / approve / reject / hook test / marketplace {add,list,update,remove} /
enable / disable / update / remove / validate`.

## V.8 CORRECTION — the child-environment allowlist is 9 base keys, not 11

Their §4.1 lists `PWD` and `SHLVL` as inherited. Those are artifacts of their `probe.sh`
`/bin/sh` wrapper, which sets them itself. Executing the server directly (`command: ["python3",
"mcp/server.py", "PLUGIN"]`) gives exactly 16 keys:

```
["CLAUDE_PLUGIN_DATA","CLAUDE_PLUGIN_ROOT","HOME","LANG","LOGNAME","MUSE_PLUGIN_DATA_DIR",
 "MUSE_PLUGIN_ID","MUSE_PLUGIN_ROOT","PATH","PLUGIN_DATA","PLUGIN_ROOT","SHELL","TERM","TMPDIR",
 "USER","__CF_USER_TEXT_ENCODING"]
```

i.e. base = `HOME LANG LOGNAME PATH SHELL TERM TMPDIR USER __CF_USER_TEXT_ENCODING` (9) plus the 7
plugin vars. No `PWD`, no `SHLVL`. `PATH` **is** verbatim — byte-compared against the parent shell's
value, identical across all 36 entries. A control variable (`MYMARKER=…`) set on the `muse` command
line did not reach the child, confirming the scrub. Everything else in §4.1/§4.2 (cwd = workspace,
`command[1]` rewritten to the absolute cache path, `argv[0]` handed over absolute) reproduced.

## V.9 EXTENSION — open item #2 settled: the namespace budget is exactly 31 characters

One bundle, seven `mcpServers` with server ids of increasing length, all approved, one `muse exec`
(`run3`). All seven spawned:

```
declared mcp__plugin_omv_sxxxxxxxxxxx      len=28 -> mcp__plugin_omv_sxxxxxxxxxxx      28  VERBATIM
declared mcp__plugin_omv_sxxxxxxxxxxxx     len=29 -> mcp__plugin_omv_sxxxxxxxxxxxx     29  VERBATIM
declared mcp__plugin_omv_sxxxxxxxxxxxxx    len=30 -> mcp__plugin_omv_sxxxxxxxxxxxxx    30  VERBATIM
declared mcp__plugin_omv_sxxxxxxxxxxxxxx   len=31 -> mcp__plugin_omv_sxxxxxxxxxxxxxx   31  VERBATIM
declared mcp__plugin_omv_sxxxxxxxxxxxxxxx  len=32 -> mcp__plugin_omv_s__03c7eebb8a94    31  REWRITTEN
declared mcp__plugin_omv_sxxxxxxxxxxxxxxxx len=33 -> mcp__plugin_omv_s__53ffc8a6cef0    31  REWRITTEN
declared ...xxxxxxxxxxxxxxxxx              len=34 -> mcp__plugin_omv_s__82c92a615c99    31  REWRITTEN
```

**≤ 31 chars survives verbatim; 32 is the first length that is rewritten**, to `first-17-chars` +
`__` + 12 hex = 31 chars. Since the prefix `mcp__plugin_` costs 12 and the joining `_` costs 1,
the budget is **`len(plugin-id) + len(server-id) ≤ 18`**. `omv`+`srv` = 6, fine;
`oh-my-musecode`+`workspace-index` = 29, hashed. `canonical_id` is never truncated (confirmed at
length 34).

## V.10 EXTENSION — open item #3 settled: `transport: "http"` plugin MCP servers work

Untested in the original. A bundle declaring
`{"id":"hsrv","transport":"http","url":"http://127.0.0.1:38471/mcp"}` validates clean
(`diagnostics: []`), installs, lands at `review_needed`, and once approved muse connects on
loopback:

```
POST /mcp HTTP/1.1
accept: application/json, text/event-stream
mcp-protocol-version: 2024-11-05
content-type: application/json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{...,"clientInfo":{"name":"tbh","version":"0.1.0"}}}
POST /mcp  {"jsonrpc":"2.0","method":"notifications/initialized","params":{}}
POST /mcp  {"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
```

Streamable HTTP, one POST per message, same three-step handshake and same client identity as stdio.
With a loopback server that answers correctly, the tool registers exactly like a stdio one:
`canonical_id: mcp__plugin_omh_hsrv__hping`, `server_name: plugin:omh:hsrv`, 21 `active_tools`.
Both transports coexist in one session (22 tools, both entries in one catalog). **A bundle may ship
either transport.** (No traffic left the machine; the listener bound `127.0.0.1` only.)

## V.11 The one open item I could NOT close, and exactly why

Open item #1, end-to-end `tools/call`, stands unproven, and I believe it is unprovable under this
task's hard rules. What I tried:

1. **A direct invocation verb on the MSP wire.** Exported the binary's own method index —
   `muse schema generate-json-schema --out DIR --experimental` — and enumerated it. The complete
   served surface is `session/*`, `turn/*`, `model/list`, `view/*`, `approval/*`, `userInput/*`.
   There is **no `tool/call`, no `mcp/*`, no invocation method of any kind**. A host cannot call a
   tool; only the model can.
2. **A CLI verb.** `muse --help` has no `mcp` subcommand (`muse mcp` prints root help, exit 0), and
   `muse plugins --help`'s only execution verb is `hook test … --fixture` — hooks, not MCP tools.
3. **An echo-provider scripting hook.** Enumerated every `MUSE_*` name in the binary (44 of them);
   none scripts the provider's output. `strings` around the echo provider yields only
   `echo provider` / `echo: ` / `muse-tui-echo`. The echo provider emits one text message and never
   a tool call.

The remaining route is `muse auth set --api-key-stdin` with a dummy key plus
`--provider meta --base-url http://127.0.0.1:PORT` against a local mock that returns a
`tool_calls` response. That is fully offline **by construction**, but it is `muse auth` — the hard
rule says never authenticate — and I cannot rule out a token-exchange to a hardcoded host that
`--base-url` does not cover. **I did not run it.** It is the right next experiment for a pass that
is explicitly authorised for it, and it is the *only* thing standing between this report and a
complete end-to-end proof. Everything short of the model's own tool-call emission is already proven:
process spawned, handshake completed, `tools/list` answered, tool catalogued with a canonical id,
canonical id present in `model_request_configured.toolset.active_tools`.

## V.12 Reproduce the verification

```bash
bash /private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/verify-plugin-mcp/verify.sh
```

~9 s, clean `HOME` each run, no login, `--provider echo` / `settings.provider="echo"` only, and — unlike
the original — `settings.json` **never** declares an `mcpServers` entry, so nothing can be miscredited
to the settings lane. Verbatim output of the run above:

```
### 0  baseline: no plugin at all
    spawned=0  <-- expect 0
      tools=20  mcp_tools=NONE
### 1  install -> DEFAULT trust state
    warning	third-party plugin: MCP servers require review before activation; skills are active without review while the plugin is enabled
    runtime-capability	plugin:omv:mcp_server:srv	status=review_needed
    spawned=0  <-- expect 0
      tools=20  mcp_tools=NONE
    DISCRIMINATOR: plugin skill marker in that same session = 1 (expect 1)
                   => the plugin IS loaded; only its MCP capability is withheld.
### 2  approve  (MUSE_EXPERIMENTAL_PLUGINS deliberately UNSET for every exec below)
    runtime-capability	plugin:omv:mcp_server:srv	status=trusted_enabled
    spawned=1  <-- expect 1
      tools=21  mcp_tools=['mcp__plugin_omv_srv__vping']
    lane tag: LANE=PLUGIN
### 3  reject -> approve  (reversibility)
    rejected  spawned=0  <-- expect 0
    approved  spawned=1  <-- expect 1
### 4  second gate: plugins disable / enable (capability stays trusted_enabled)
    disabled  spawned=0  <-- expect 0
    enabled   spawned=1  <-- expect 1
### 5  third gate: any byte change to the package + plugins update -> modified
    runtime-capability	plugin:omv:mcp_server:srv	status=modified
    spawned=0  <-- expect 0
    re-approved spawned=1  <-- expect 1
### 6  declared != active: compose still counts a capability nobody may run
    trace says mcp_servers=1 commands=1
    actually spawned=0  <-- expect 0
### 7  http transport, loopback only
    stdio spawned=1 ; http requests=3  <-- expect 1 and 3
      tools=22  mcp_tools=['mcp__plugin_omh_hsrv__hping', 'mcp__plugin_omv_srv__vping']
```

Supporting files, all under `.../scratchpad/settle/verify-plugin-mcp/`:

| Path | What |
|---|---|
| `verify.sh` | The controlled reproduction above (7 stages, ~9 s) |
| `tools/an.py` | Per-session extractor: `mcp.startup`, identity catalog, `active_tools`, `agent_tree_initialized` |
| `tools/serve.py` | Independent MSP client (`initialize` → `session/start` → `turn/start`), samples the MCP log around each step |
| `tools/drive_verify.py` | pty TUI driver taking HOME/WS/LOG/script on argv; strips `MUSE_EXPERIMENTAL*` from the child env |
| `tools/http_mcp.py` | Loopback streamable-HTTP MCP server used for V.10 |
| `run1/` | Their `repro.sh`, clean HOME |
| `run2/` | Skill+command+MCP bundle: discriminator, reversibility, disable/enable, `modified`, serve ×2, TUI ×3 |
| `run3/` | Seven-server namespace sweep and the 7-way declared-vs-active test |
| `run4/` | `transport: "http"` bundle and listeners |
| `schema-stable/msp.schema.json` | The binary's own MSP method index, used to rule out a wire-level tool-call verb |

## V.13 Net effect on §8 / design impact

The design guidance survives intact, with four amendments:

1. `omm doctor`'s MCP check must assert the `runtime-capability` line is **present and**
   `trusted_enabled`. `plugins disable` removes the line entirely, so "no bad status found" is not
   the same as healthy.
2. `omm uninstall` should call `muse plugins remove <id> [--delete-data]`, which exists and also
   clears `settings.json → runtime_capabilities`. No hand-editing.
3. The naming rule is exact: **`len(plugin-id) + len(server-id) ≤ 18`**, else the model-visible
   namespace becomes a hash. `omm`/`idx` = 6. Fine.
4. `omm install` may present the binary's own words — the install warning is explicit and quotable —
   but must not rely on any *session-time* signal, because there is none, in any lane, at any log level.

And two capabilities the framework can now count on: **`transport:"http"` plugin MCP servers work**,
and a bundle may mix stdio and http servers in one session.
