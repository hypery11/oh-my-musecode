# SETTLED: end-to-end MCP `tools/call` from the model through a plugin-provided server

**Verdict: RESOLVED_YES.** The last open row of `research/experiments/00-DECISION.md` §3 (row 1) is
closed. With a plugin-provided stdio MCP server in `trusted_enabled` state, a `function_call` emitted
by the model is dispatched as JSON-RPC `tools/call` to that server with the model's arguments, the
server's `content[0].text` is returned to the model verbatim as a `function_call_output` in the very
next provider request, and `session.jsonl` records the whole chain. It works identically in the
`muse exec` lane and the `muse serve` (MSP) lane. No login, no `muse auth`, no request left
`127.0.0.1`.

Two things came out that nobody had measured:

1. **The wire tool shape is a namespace group, and the 18-char rule is functional, not cosmetic.**
   Muse sends `tools[]` to the provider as
   `{"type":"namespace","name":"mcp__plugin_<pid>_<sid>","tools":[{"type":"function","name":"<tool>"}]}`.
   Once `len(pid)+len(sid) > 18` the namespace the model sees is the rewritten
   `first-17 + "__" + 12hex`, and a call addressed as `<namespace>__<tool>` — the form that works for
   every shorter namespace — is refused as `unknown tool`. Only `<namespace>.<tool>` still dispatches.
2. **Which `function_call.name` shapes Muse accepts:** `<ns>.<fn>` (every length), `<ns>__<fn>`
   (only while the namespace is unrewritten), and the canonical id. Bare `<fn>`, `<fn>` plus a
   `namespace` field, and `<ns>/<fn>` are all rejected — and the rejection is fed back to the model as
   a `function_call_output` that lists every session tool id.

Binary: `.host/bin/muse-bin-1.0.1-R2006.1`, sha256
`b9c7f9badb6b2af1b362d30202b366e7bdc13b3c4048e9002caa236cc56c54a4` (byte-identical to the
`muse-aarch64-macos` every earlier report used). macOS 26.6.2, python 3.14.6. Date 2026-09-02.

---

## 0. One-command reproduction

```bash
export OMM_MUSE_BIN="$PWD/.host/bin/muse-bin-1.0.1-R2006.1"
bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/exec                       # 2.6 s
LANE=serve MOCK_PORT=8732 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/serve   # 6.8 s
```

Both end with

```
== verdicts ==
  [PASS] P1 server received tools/call with the scripted argument (or none, where expect=reject)
         ok   step 0 expect=ok     args={"text": "omm mcp probe short 7f3a"} -> server=up jsonrpc_id=3
         ok   step 1 expect=ok     args={"text": "omm mcp probe fifteen 18ch"} -> server=fifteencharsxxx jsonrpc_id=3
         ok   step 2 expect=ok     args={"text": "omm mcp probe sixteen rewritten"} -> server=sixteencharsxxxx jsonrpc_id=3
  [PASS] P2 tool result appeared as function_call_output in the NEXT provider request (or `tool unavailable`, where expect=reject)
         ok   call_0 emitted in request #1 as mcp__plugin_omm_up.echo_upper (expect=ok) -> output in request #2: OMM MCP PROBE SHORT 7F3A [echo_upper via server=up pid=63409]
         ok   call_1 emitted in request #2 as mcp__plugin_omm_fifteencharsxxx.echo_upper (expect=ok) -> output in request #3: OMM MCP PROBE FIFTEEN 18CH [echo_upper via server=fifteencharsxxx pid=63406]
         ok   call_2 emitted in request #3 as mcp__plugin_omm_s__a279115b87d6.echo_upper (expect=ok) -> output in request #4: OMM MCP PROBE SIXTEEN REWRITTEN [echo_upper via server=sixteencharsxxxx pid=63408]
  [PASS] P3 session.jsonl carries tool call and tool result records
```

The probe plans and the negative control (all under `tools/mockprovider/plans/`, all PASS):

```bash
PLAN_FILE=tools/mockprovider/plans/name-forms.json          MAX_STEPS=12 MOCK_PORT=8733 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/forms
PLAN_FILE=tools/mockprovider/plans/rewritten-namespace.json MAX_STEPS=12 MOCK_PORT=8734 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/rewritten
SKIP_APPROVE=1 PLAN_FILE=tools/mockprovider/plans/unapproved-control.json MOCK_PORT=8735 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/unapproved
```

`tools/mockprovider/README.md` documents every file and the wire contract.

---

## 1. Instrument

Three independent instruments, cross-checked by `analyze_mcp_toolcall.py`:

| instrument | what it proves |
|---|---|
| the MCP servers' own logs (`data/muse/plugins/data/omm/mcp-<sid>.log`) — every JSON-RPC frame in and out | (1) the server received `tools/call` with the model's argument |
| the mock provider transcript (`logs/mock.log`) — every `POST /responses` body and every SSE we returned | (2) the tool result reached the *next* request as `function_call_output` |
| Muse's `session.jsonl` | (3) the host's own record of the call and the result |

**The bundle** (`run-mcp-toolcall.sh` writes it): plugin id `omm` — our real id — with three stdio
`mcpServers`, all running `tools/mockprovider/mcp_echo_server.py` (tool `echo_upper`, input
`{"text": string}`, output `TEXT.upper() + " [echo_upper via server=<sid> pid=<pid>]"`):

| server id | `len(pid)+len(sid)` | namespace `mcp__plugin_omm_<sid>` | on the wire |
|---|---|---|---|
| `up` | 5 | 18 chars | verbatim |
| `fifteencharsxxx` | **18** | **31 chars** | verbatim — the boundary |
| `sixteencharsxxxx` | 19 | 32 chars | **rewritten** `mcp__plugin_omm_s__a279115b87d6` |

The server derives its log path from `MUSE_PLUGIN_DATA_DIR` (the child env is the scrubbed 16-key
allowlist; no custom variable reaches it — `research/experiments/plugin-mcp.md` V.8), and the
directory is not created by the host, so the server `mkdir -p`s it.

**The model** is `tools/mockprovider/respond-toolcall.py` behind `mock.py`, playing a plan: one
`echo_upper` call per server, each with a distinct argument, then a final message. The plan step is
chosen by counting `function_call_output` items in the request, so a provider retry can never run the
script ahead. Muse is pointed at it with
`exec --provider meta --base-url http://127.0.0.1:8731 --model test-model --yolo` and
`META_API_KEY=dummy`; the gate `MUSE_EXPERIMENTAL_PLUGINS` is set on the `plugins` verbs only and
**unset** for every session.

Trust is verified, not assumed — `plugins inspect omm --json` after approve:

```
runtime_capabilities[].candidate.stable_id            .status
plugin:omm:mcp_server:fifteencharsxxx                  trusted_enabled
plugin:omm:mcp_server:sixteencharsxxxx                 trusted_enabled
plugin:omm:mcp_server:up                               trusted_enabled
```

(`inspect --json` nests it as `{"candidate":{"stable_id":…},"status":…}` — the text form's
`runtime-capability <id> status=<s>` line is a rendering of that.)

---

## 2. Results

### 2.1 The chain, `muse exec` lane, verbatim

**Request #1 → provider.** `tools[]` has four entries: the built-ins in one namespace and one
namespace per MCP server. The MCP tool spec is the server's `tools/list` entry, passed through
unchanged (`inputSchema` → `parameters`):

```json
{"type":"namespace","name":"muse","description":"Muse Code tool set.","tools":[{"type":"function","name":"workflow",…},…]}
{"type":"namespace","name":"mcp__plugin_omm_up","description":"Tools provided by an MCP server.",
 "tools":[{"type":"function","name":"echo_upper",
           "description":"Upper-cases the given text and returns it. Test tool shipped by the omm mock harness.",
           "parameters":{"type":"object","properties":{"text":{"type":"string","description":"Text to upper-case."}},
                         "required":["text"],"additionalProperties":false}}]}
{"type":"namespace","name":"mcp__plugin_omm_fifteencharsxxx", …same…}
{"type":"namespace","name":"mcp__plugin_omm_s__a279115b87d6", …same…}
```

**Model → Muse** (our SSE, `response.output_item.done`):

```json
{"type":"function_call","id":"fc_0","call_id":"call_0","name":"mcp__plugin_omm_up.echo_upper",
 "arguments":"{\"text\": \"omm mcp probe short 7f3a\"}","status":"completed"}
```

**Muse → server** (`mcp-up.log`, line-delimited JSON-RPC, after the usual `initialize` →
`notifications/initialized` → `tools/list`):

```
EV=recv {"id": 3, "jsonrpc": "2.0", "method": "tools/call", "params": {"arguments": {"text": "omm mcp probe short 7f3a"}, "name": "echo_upper"}}
EV=sent {"id": 3, "jsonrpc": "2.0", "result": {"content": [{"text": "OMM MCP PROBE SHORT 7F3A [echo_upper via server=up pid=63409]", "type": "text"}], "isError": false}}
```

The argument object arrives as parsed JSON (not a string), and `params.name` is the raw tool name
`echo_upper` — the namespace is stripped before the server sees it.

**Request #2 → provider**, `input[]` tail:

```json
{"type":"function_call","id":"fc_0","call_id":"call_0","name":"mcp__plugin_omm_up.echo_upper","arguments":"{\"text\": \"omm mcp probe short 7f3a\"}"}
{"type":"function_call_output","call_id":"call_0","output":"OMM MCP PROBE SHORT 7F3A [echo_upper via server=up pid=63409]"}
```

`output` is exactly `result.content[0].text`. No wrapper, no metadata, no JSON encoding.

The same chain then repeats for `fifteencharsxxx` (request #2 → #3) and for the rewritten
`mcp__plugin_omm_s__a279115b87d6` (request #3 → #4, dispatched to the `sixteencharsxxxx` process),
and the final message `MOCK-FINAL-ANSWER all three echo_upper calls done` is what `muse exec` prints.

### 2.2 `session.jsonl`, the host's own record

Accepted call (`seq` numbers from the default run; `run_id`/`task_id` elided):

```
seq 67  runtime.session   event.kind=assistant_tool_calls_committed
        {"response_id":"resp_1","tool_calls":[{"id":"fc_0","call_id":"call_0",
          "name":"mcp__plugin_omm_up__echo_upper","args":"{\"text\": \"omm mcp probe short 7f3a\"}"}]}
seq 69  runtime.session   event.kind=proposed            task_kind="tool.mcp__plugin_omm_up__echo_upper"
seq 72  runtime.session   event.kind=side_effect_intent  operation="tool:mcp__plugin_omm_up__echo_upper"
                                                         idempotency_key="tool:call_0" policy_decision="allow:policy"
seq 74  tool_batch.effect.started   record.kind=started  call_id="call_0" tool_name="mcp__plugin_omm_up__echo_upper"
                                                         parallel_profile={"kind":"ineligible"}
seq 75  tool_batch.effect.terminal  record.kind=terminal outcome={"kind":"completed","task_completion":{"kind":"complete"},"output_ref_count":0}
seq 76  runtime.session   event.kind=output              chunk="OMM MCP PROBE SHORT 7F3A [echo_upper via server=up pid=63409]"
seq 79  runtime.session   event.kind=tool_result_batch_committed
        {"results":[{"tool_call_index":0,"tool_call_id":"call_0","text":"OMM MCP PROBE SHORT 7F3A [echo_upper via server=up pid=63409]"}]}
```

Note the normalisation: the model said `mcp__plugin_omm_up.echo_upper`; every session record
carries the **canonical id** `mcp__plugin_omm_up__echo_upper`. For the rewritten namespace the
model said `mcp__plugin_omm_s__a279115b87d6.echo_upper` and the records carry
`mcp__plugin_omm_sixteencharsxxxx__echo_upper`. `parallel_profile: ineligible` — MCP tool calls are
not eligible for parallel batching in this build.

Rejected call (from the name-forms probe, bare `echo_upper`):

```
seq 94  assistant_tool_calls_committed  tool_calls[0].name="echo_upper"
seq 96  proposed                        task_kind="tool.echo_upper"
seq 97  rejected                        reason="unknown tool `echo_upper`"
                                        failure_kind={"kind":"bad_arguments","bad_args_kind":"unknown_tool"} fed_back=true
seq 99  tool_result_batch_committed     results[0].text="tool unavailable: unknown tool `echo_upper`. Session tool ids: add_memory, bash, bash_input,
        create_goal, cron_create, cron_delete, cron_list, edit_file, edit_memory, get_goal, mcp__plugin_omm_fifteencharsxxx__echo_upper,
        mcp__plugin_omm_sixteencharsxxxx__echo_upper, mcp__plugin_omm_up__echo_upper, read_file, read_memory, read_skill, report_progress,
        search, subagent_cancel, subagent_read_result, subagent_send_message, subagent_spawn, subagent_status, subagent_wait, update_goal,
        web_search, workflow, write_file, write_todos"
```

That `Session tool ids` text is the one place the un-rewritten canonical id of a 32+ char namespace
ever reaches the model — as the feedback to a failed call.

### 2.3 `muse serve` (MSP) lane — same result

`serve` has no `--provider`/`--base-url`. The runner writes, **before** install so that
`plugins approve` merges into it:

```json
{"schema_version":1,"provider":"meta","model":"test-model","endpoint_transport":{"base_url":"http://127.0.0.1:8732"}}
```

and drives `initialize` → `initialized` → `session/start{providerId:"meta",approvalMode:"allowAll"}`
→ `turn/start` over NDJSON with `META_API_KEY=dummy` in the environment. The bootstrap trace confirms
`mode="serve" provider_source="settings"`. Result:

```
<<< {"jsonrpc":"2.0","id":3,"result":{"commandId":"…","status":"accepted","turnId":"…","startedNewTurn":true,"disposition":"started"}}
<<< turn/started
<<< item/completed {"kind":"userMessage","status":"completed","text":"Use the echo_upper tools to upper-case the phrase: omm mcp probe."}
<<< item/started   {"kind":"toolCall","status":"inProgress","tool":"mcp__plugin_omm_up__echo_upper","callId":"call_0","args":"{\"text\": \"omm mcp probe short 7f3a\"}"}
<<< item/completed {"kind":"toolCall","status":"completed", "tool":"mcp__plugin_omm_up__echo_upper","callId":"call_0",…}
<<< item/started / item/completed  {"kind":"toolCall","tool":"mcp__plugin_omm_fifteencharsxxx__echo_upper","callId":"call_1",…}
<<< item/started / item/completed  {"kind":"toolCall","tool":"mcp__plugin_omm_sixteencharsxxxx__echo_upper","callId":"call_2",…}
<<< item/completed {"kind":"agentMessage","status":"completed","text":"MOCK-FINAL-ANSWER all three echo_upper calls done"}
<<< turn/completed {"terminal":"completed","durationMs":884}
RESULT turn/completed
```

So an MSP host *sees* every MCP call as an `item/started` → `item/completed` pair of
`kind:"toolCall"` carrying the canonical id, the `callId` and the argument string (the result text is
not on the item; it is in `session.jsonl`). All three servers received `tools/call` with the
scripted arguments, all three results came back as `function_call_output` in the next request, and
`session.jsonl` carries the identical `assistant_tool_calls_committed` → `tool_batch.effect.*` →
`tool_result_batch_committed` chain (seq 46–112) with `side_effect_intent.policy_decision =
"allow:policy"` for each call under `approvalMode:"allowAll"` (the `mcp.startup.batch` intent is
`allow:mcp_configuration_admitted`, as in `plugin-mcp.md` §2.3). The MSP lane is fully
offline-testable *with a scripted model*, which extends
`plugin-mcp.md` §3.3 (echo provider) to real tool traffic. `msp-protocol.md` §1's "a `serve` host
always uses the real provider" is true of the wire and false of the settings file.

Two MSP facts that cost a run each: `clientInfo.name` must match `^[a-z0-9_]+$` (SS1.4.1) — a hyphen
gets `-32602 invalidParams` and every later frame `-32600 Not initialized`; and the `initialized`
notification sent before the `initialize` result has arrived is dropped with
`tbh-session-wire: dropping pre-handshake notification initialized (FR-008)`.

### 2.4 Negative control — same bundle, never approved

`SKIP_APPROVE=1`: all three capabilities at `review_needed`. Then in the session:
`tools[]` has **one** entry (the `muse` namespace), no `runtime.mcp_tool_identity_catalog`, no
server process spawned (no log file), and both addressing forms are refused:

```
call_0  mcp__plugin_omm_up.echo_upper   -> tool unavailable: unknown tool `mcp__plugin_omm_up.echo_upper`. Session tool ids: …
call_1  mcp__plugin_omm_up__echo_upper  -> tool unavailable: unknown tool `mcp__plugin_omm_up__echo_upper`. Session tool ids: …
```

So dispatch is credited to the trust state and nothing else, and the model-side symptom of an
unapproved bundle is exactly "the tool does not exist" — consistent with `plugin-mcp.md`'s
"silent at session time".

---

## 3. The 18-char rule, re-measured, and what it actually breaks

### 3.1 Namespace rewrite (identity catalog, verbatim, same in all six runs)

```json
{"canonical_id":"mcp__plugin_omm_up__echo_upper",               "surface":{"namespace":"mcp__plugin_omm_up",              "name":"echo_upper"},"server_name":"plugin:omm:up"}
{"canonical_id":"mcp__plugin_omm_fifteencharsxxx__echo_upper",  "surface":{"namespace":"mcp__plugin_omm_fifteencharsxxx", "name":"echo_upper"},"server_name":"plugin:omm:fifteencharsxxx"}
{"canonical_id":"mcp__plugin_omm_sixteencharsxxxx__echo_upper", "surface":{"namespace":"mcp__plugin_omm_s__a279115b87d6", "name":"echo_upper"},"server_name":"plugin:omm:sixteencharsxxxx"}
```

31 chars (`len(pid)+len(sid)=18`) survives verbatim; 32 is rewritten to `first-17 + "__" + 12 hex`
— `plugin-mcp.md` V.9 confirmed at the exact boundary with our real plugin id. The 12-hex digest was
`a279115b87d6` in every sandbox and run (six different `package_sha256`/cache paths), so it is a
function of the namespace string alone, not of the package. The `surface.namespace` is what goes on
the wire as the `namespace` group name; `canonical_id` is what every session record and the
`Session tool ids` feedback use.

### 3.2 Which `function_call.name` dispatches (`plans/name-forms.json`, server `up`)

| form the model emits | example | result |
|---|---|---|
| `<ns>__<fn>` | `mcp__plugin_omm_up__echo_upper` | **dispatched** (server got `tools/call`, result fed back) |
| `<ns>.<fn>` | `mcp__plugin_omm_up.echo_upper` | **dispatched** |
| `<fn>` + `"namespace":"<ns>"` on the item | `echo_upper` | rejected `unknown tool \`echo_upper\`` (the field is ignored) |
| `<ns>/<fn>` | `mcp__plugin_omm_up/echo_upper` | rejected `unknown tool \`mcp__plugin_omm_up/echo_upper\`` |
| bare `<fn>` | `echo_upper` | rejected `unknown tool \`echo_upper\`` |

### 3.3 The same forms on the boundary and past it (`plans/rewritten-namespace.json`)

| server (pid+sid) | form | name emitted | result |
|---|---|---|---|
| `fifteencharsxxx` (18) | `<ns>__<fn>` | `mcp__plugin_omm_fifteencharsxxx__echo_upper` | dispatched |
| `fifteencharsxxx` (18) | `<ns>.<fn>` | `mcp__plugin_omm_fifteencharsxxx.echo_upper` | dispatched |
| `sixteencharsxxxx` (19) | `<ns>__<fn>` from the **wire** namespace | `mcp__plugin_omm_s__a279115b87d6__echo_upper` | **rejected** `unknown tool` |
| `sixteencharsxxxx` (19) | `<ns>.<fn>` from the wire namespace | `mcp__plugin_omm_s__a279115b87d6.echo_upper` | dispatched → `server=sixteencharsxxxx` |
| `sixteencharsxxxx` (19) | canonical id (never on the wire) | `mcp__plugin_omm_sixteencharsxxxx__echo_upper` | dispatched |

Reading: dispatch resolves a name either **literally against the session tool ids** (the canonical
`mcp__plugin_<pid>_<sid>__<tool>` set) or **as `<surface-namespace>.<tool>` through the identity
catalog**. While the surface namespace equals the canonical one, `<ns>__<fn>` happens to *be* a
session tool id and both readings agree. Past 18 chars they diverge: the concatenation the model can
build from what it sees is not a session tool id, and only the dot form survives. Whether the real
Meta model emits the dot form or the concatenation for a `namespace`-typed tool cannot be measured
offline (§5.1) — so the safe engineering position is the one `00-DECISION` already took: **keep
`len(pid)+len(sid) ≤ 18` and both readings coincide.** With `omm` that leaves 15 characters for a
server id.

---

## 4. Wire and host facts learned

| fact | evidence |
|---|---|
| Tool specs go to the provider as `{"type":"namespace","name":…,"tools":[{"type":"function",…}]}` groups; built-ins are the namespace `muse` ("Muse Code tool set."); each MCP server is one namespace ("Tools provided by an MCP server.") | request #1 `tools[]`, §2.1 |
| The MCP tool's `inputSchema` is passed through verbatim as `parameters`; `description` verbatim | §2.1 |
| `tools/call.params.arguments` reaches the server as a JSON object; `params.name` is the raw tool name | `mcp-up.log` |
| The tool result is `content[0].text`, returned to the model as the `function_call_output.output` string with no wrapping | request #2, §2.1 |
| `model_input_trace_recorded.aggregates[tool_specs].byte_count` is `0` even when four namespace groups were sent | exec session seq 43 — do not use that lane as a tool-spec oracle |
| MCP calls run with `parallel_profile: {"kind":"ineligible"}` | `tool_batch.effect.started` |
| The policy decision on an MCP call is `allow:policy` both under `exec --yolo` and under MSP `approvalMode:"allowAll"`; no approval round-trip in either lane | `side_effect_intent` in both sessions |
| An MSP host observes each MCP call as `item/started` → `item/completed` with `kind:"toolCall"`, `tool:<canonical id>`, `callId`, `args`; the result text is not carried on the item | serve transcript, §2.3 |
| A rejected name yields `proposed` → `rejected {failure_kind:{kind:"bad_arguments",bad_args_kind:"unknown_tool"},fed_back:true}` and a `tool unavailable: unknown tool … Session tool ids: …` result; the turn continues | §2.2 |
| `settings.json → endpoint_transport.base_url` + `provider:"meta"` + `META_API_KEY` routes `muse serve` to a loopback provider (`provider_source="settings"`) | §2.3 |
| MSP `clientInfo.name` grammar `^[a-z0-9_]+$`; pre-handshake `initialized` is dropped (FR-008) | §2.3 |
| `plugins inspect --json` shape: `runtime_capabilities[] = {"candidate":{"kind","plugin_id","capability_id","stable_id","display_path","definition_hash","source_digest"},"status","diagnostic"}` | `logs/inspect.json` |
| The whole exec lane — build, validate, install, three approves, inspect, a four-request session — runs in 2.6 s; the serve lane in 6.8 s | `time` of the runner |

---

## 5. What is still NOT established

1. **The name form the real Meta model emits for a `namespace`-typed tool.** Both accepted forms
   were exercised, but only a scripted model was used. If the production model emits `<ns>__<fn>`,
   every MCP tool past the 18-char boundary is uncallable; if it emits `<ns>.<fn>`, the rule is
   cosmetic. Needs one authenticated turn; until then the rule is load-bearing.
2. Approval prompting for MCP calls without `--yolo` (`--approval-mode on-request`) — whether an MCP
   tool is `Prompt`-bound and what the `PermissionRequest` hook sees. Not exercised.
3. `isError: true` results, multi-part `content`, non-text content, and outputs over
   `--max-tool-output-bytes` — the shape of a *failed* or *large* tool result on the wire.
4. `transport: "http"` servers under `tools/call` (only spawn/handshake was proven, `plugin-mcp.md`
   V.10). The harness can do it; not run.
5. The TUI lane was not driven (pty harness not rebuilt); nothing suggests it differs — the runtime
   is shared and `plugin-mcp.md` L3 proved spawn and catalogue there.
6. `parallel_profile: ineligible` was observed, not explained; whether `--parallel-tool-calls`
   changes it is untested.

---

## 6. Design consequences for oh-my-musecode

1. **`00-DECISION` §4 "blocked, conditionally" is lifted.** A bundle whose primary value is an MCP
   tool may ship: the model's own emission, the dispatch, the server call and the result path are
   all proven, in both headless lanes. ARCHITECTURE §7 `omm install` and PLAN 2.2 (`omm mcp`) can
   proceed as written.
2. **R19's `len(pid)+len(sid) ≤ 18` is a correctness rule, not a readability rule** — past it, one of
   the two accepted addressing forms stops working and the model may never see the canonical id.
   The lint must fail the build, not warn. With `omm`, server ids ≤ 15 chars: `doctor` (6) and
   `cost` (4) are fine.
3. **Doctor D1 gains a wire-level oracle.** The symptom of any non-`trusted_enabled` state is now
   known from the model's side: the namespace is absent from `tools[]`, and a call is answered
   `tool unavailable: unknown tool`. `omm doctor --live` could run the mock lane and assert the
   namespace `mcp__plugin_omm_<sid>` is present in request #1 — that is a measurement of what
   *composed*, which host-reality.md's server-side-risk section says doctor must prefer over
   disk-vs-disk.
4. **The mock lane belongs in CI** (PLAN 2.2 acceptance: "`tools/call` round-trip through the mock
   provider"). `run-mcp-toolcall.sh` is that test; it needs only the binary and a loopback port.
   Add the P1 row below to `hostcheck`.
5. **Tool-result contract for `omm mcp`:** return exactly one `{"type":"text"}` content part; it is
   forwarded byte-for-byte. Keep it small (a tool result is context every subsequent turn) and make
   errors explicit in the text — an `isError` flag's rendering is untested (§5.3).
6. **`muse serve` hosts are testable offline** with `settings.provider`, `settings.model` and
   `settings.endpoint_transport.base_url`. Doctor D2's "provider must be set for `serve`" now has a
   positive fixture, and any future SDK-host test can reuse `msp_toolcall.py`.
7. The scrubbed child environment stands: the server found its state directory only via
   `MUSE_PLUGIN_DATA_DIR`, which it had to create. `omm mcp` must `mkdir -p` its data dir.

---

## 7. Failed attempts and dead ends, in order

1. **Assumed the wire tool name was the canonical id.** The first run's responder found "0 wire
   names among 4 tools": the request carries namespace groups, not flat `mcp__…__tool` functions.
   Reading request #1's `tools[]` in full was the fix and produced §3.
2. **Assumed `<ns>__<fn>` was the only addressing form.** It fails past 18 chars; the five-form
   probe (`plans/name-forms.json`) found the dot form.
3. **`plugins inspect --json` trust lines.** The first parser looked for `stable_id` and `status` in
   the same object; they live in `candidate.stable_id` / sibling `status`.
4. **Bash watchdog.** `( sleep N; kill $P ) & K=$!; …; kill $K; wait $K` blocked for the full N
   seconds — bash defers a signal while `sleep` is its foreground child. Replaced by
   `perl -e 'alarm shift @ARGV; exec @ARGV'`.
5. **MSP `clientInfo.name` with a hyphen** → `-32602`, then `Not initialized` for everything after.
6. **`grep` on the binary's `strings.txt`** for the call-name grammar: the file is rodata blobs;
   `ugrep` refused the bounded regexes ("exceeds complexity limits") and the unbounded ones returned
   41 KB of noise. The empirical probe answered in 2.8 s.
7. The MSP driver truncated frames at 400 chars, which hid the `item/*` payloads; widened to 6000.

---

## 8. Files

| path | what |
|---|---|
| `tools/mockprovider/run-mcp-toolcall.sh` | the experiment, both lanes (`LANE=exec|serve`), `SKIP_APPROVE`, `PLAN_FILE`, `CALL_FORM` |
| `tools/mockprovider/mock.py` | loopback provider, env-configured, JSONL transcript |
| `tools/mockprovider/respond-toolcall.py` | the scripted model (plans, name forms, retry-safe step selection) |
| `tools/mockprovider/mcp_echo_server.py` | the logging `echo_upper` stdio server |
| `tools/mockprovider/analyze_mcp_toolcall.py` | cross-checks the three instruments, prints P1/P2/P3 |
| `tools/mockprovider/msp_toolcall.py` | MSP client for the serve lane |
| `tools/mockprovider/plans/{name-forms,rewritten-namespace,unapproved-control}.json` | the probes of §3 and the control of §2.4 |
| `tools/mockprovider/README.md` | harness documentation and the wire contract |

---

## host-reality.md patch

Rows to add or replace, in that file's table format.

**Identity constraints** — replace the "MCP tool name as seen by the model" row and add two:

```
| MCP tool name as seen by the model | wire `tools[]` entry `{"type":"namespace","name":"mcp__plugin_<pid>_<sid>","tools":[{"type":"function","name":"<tool>",…}]}`; namespace survives verbatim to 31 chars; 32+ rewritten to `first-17 + "__" + 12 hex` (digest deterministic per namespace string) → **`len(pid) + len(sid) ≤ 18`**; `canonical_id` (`mcp__plugin_<pid>_<sid>__<tool>`) is never on the wire once rewritten |
| MCP call addressing the host dispatches | `<ns>.<fn>` for every length · `<ns>__<fn>` only while the wire namespace is unrewritten (pid+sid ≤ 18) · the canonical id; bare `<fn>`, `<fn>`+`"namespace"` field, `<ns>/<fn>` → `tool unavailable: unknown tool …` fed back to the model with the full `Session tool ids` list |
| MCP call in `session.jsonl` | `assistant_tool_calls_committed.tool_calls[].name`, `task_kind tool.<id>`, `side_effect_intent.operation tool:<id>`, `tool_batch.effect.started.tool_name` all carry the canonical id whatever form the model used; `parallel_profile` is `ineligible` |
```

**Trust lifecycle** — add:

```
| MCP `tools/call` end-to-end | PROVEN offline (mock provider, `tools/mockprovider/run-mcp-toolcall.sh`): model `function_call` → line-delimited JSON-RPC `tools/call {"name":"<tool>","arguments":{…}}` on the plugin server → `result.content[0].text` → verbatim `function_call_output.output` in the next `/responses` request → `tool_result_batch_committed.results[].text`; identical in `muse exec` and `muse serve` |
| model-side symptom of a non-`trusted_enabled` capability | no `mcp__plugin_…` namespace in `tools[]`, no `mcp_tool_identity_catalog`, no spawn; any call → `tool unavailable: unknown tool` (the turn continues) |
| `muse serve` offline with a scripted model | `settings.json → provider:"meta", model:"<id>", endpoint_transport.base_url:"http://127.0.0.1:<port>"` + `META_API_KEY` in env (`provider_source="settings"`); `session/start.approvalMode:"allowAll"`; `clientInfo.name` must match `^[a-z0-9_]+$` (SS1.4.1) |
```

**P1 — run per release** — add:

```
| MCP `tools/call` round-trip | 3 plugin servers (sid length 2 / 15 / 16) → 3 `tools/call` received, 3 `function_call_output` returned, `tool_result_batch_committed` ×3, in `exec` and `serve` | `tools/mockprovider/run-mcp-toolcall.sh` (+ `LANE=serve`), P1–P3 PASS |
```
