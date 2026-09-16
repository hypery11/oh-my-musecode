# tools/mockprovider — a local stand-in for the Meta model API

Everything in this directory exists so that a real Muse session can be driven through a model that
**we** script, with zero contact with Meta: no login, no `muse auth`, no real credential, and no
network destination other than `127.0.0.1`. Muse has a documented `--base-url` override (and the
settings-side twin `endpoint_transport.base_url`); the override is total — every request the binary
makes (`GET /muse-code/models`, `POST /responses`) lands on the local socket
(`research/experiments/skill-routing.md`, Verification V3, and `probe_server.py` below).

It is how three offline results were obtained: `read_skill` resolving a routed skill
(`skill-routing.md` V3), the `personal_project` memory root (`loose-ends.md` section 2), and the
end-to-end MCP `tools/call` proof (`docs/experiments/mcp-tools-call.md`).

## Files

| File | Role |
|---|---|
| `mock.py` | The HTTP server. `GET */models` → `models.json`; `POST /responses` → exec's a responder script. Logs every request and response to a JSONL transcript. Configured entirely by `MOCK_PORT`, `MOCK_STATE`, `MOCK_LOG`, `MOCK_RESPONDER`, `MOCK_MODELS`. |
| `respond-toolcall.py` | Responder that plays a scripted "model": a plan of tool calls, then a final message. Resolves tool names against the request's `tools[]`, renders the call in a chosen name form, emits the SSE stream Muse accepts. |
| `mcp_echo_server.py` | A stdio MCP server exposing one tool, `echo_upper`, that logs every JSON-RPC message it sees. Shipped inside the probe plugin as `mcp/server.py`. |
| `run-mcp-toolcall.sh` | One-command experiment: sandbox → 3-server plugin → validate/install/approve → verify `trusted_enabled` → mock → `muse exec` (or `muse serve` with `LANE=serve`) → analysis. |
| `analyze_mcp_toolcall.py` | Cross-checks the three instruments (server logs, mock transcript, `session.jsonl`) and prints PASS/FAIL for the three proofs. |
| `run-omm-mcp.sh` | The `omm mcp` proof (PLAN.md 2.2): the REAL bundle installed by `omm install` into a throwaway HOME, the Muse binary reachable only through `~/.local/bin/.muse-version` (no `OMM_MUSE_BIN` anywhere — the host's child environment has none), `omm` on PATH, `plugin:oh-my-musecode:mcp_server:doc` asserted `trusted_enabled`, `omm doctor --fast` asserted green (D15), then the scripted model calls `mcp__plugin_oh-my-musecode_doc.omm_doctor` and `.omm_cost` through `muse exec`. Needs `OMM_BIN` (default `target/release/omm`). Run by `crates/omm/tests/mcp_server.rs`. |
| `analyze_omm_mcp.py` | The analyzer of that run: the server's own trace (`home/.local/share/muse/plugins/data/oh-my-musecode/omm-mcp.log`), the mock transcript and `session.jsonl`; P2 parses each `function_call_output` as the doctor / cost JSON. |
| `msp_toolcall.py` | MSP client for the `muse serve` lane: `initialize` → `initialized` → `session/start` → `turn/start`, prints every frame, stops at `turn/completed`. |
| `plans/*.json` | Ready-made plans: `name-forms.json` (which `function_call.name` shapes dispatch), `rewritten-namespace.json` (the 18-char rule), `unapproved-control.json` (negative control). |
| `harness.sh` | The `skills.v1` routing harness from `skill-routing.md` (project router hook + `--provider echo`). Unrelated to the mock; kept here because PLAN.md 0.5(c) asked for the whole harness to be copied. |
| `probe_server.py` | Containment probe: records every request and answers 400. Use it first when in doubt that `--base-url` still captures all traffic. |
| `models.json` | The model catalog served to `GET /muse-code/models` (one model, `test-model`). |

## Quick start

```bash
export OMM_MUSE_BIN=/path/to/.host/bin/muse-bin-1.0.1-R2006.1

# exec lane: three servers, one call each (2.6 s)
bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/exec

# muse serve (MSP) lane, same bundle, same plan (6.8 s)
LANE=serve MOCK_PORT=8732 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/serve

# probe plans (each step may declare "expect": "reject")
PLAN_FILE=tools/mockprovider/plans/name-forms.json          MAX_STEPS=12 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/forms
PLAN_FILE=tools/mockprovider/plans/rewritten-namespace.json MAX_STEPS=12 bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/rewritten

# negative control: bundle installed but never approved
SKIP_APPROVE=1 PLAN_FILE=tools/mockprovider/plans/unapproved-control.json bash tools/mockprovider/run-mcp-toolcall.sh /tmp/mcpcall/unapproved

# omm's own server: the real bundle, `omm install`, then omm_doctor {fast:true} and omm_cost (~25 s)
cargo build -p omm --release && bash tools/mockprovider/run-omm-mcp.sh /tmp/mcpcall/omm
```

Every run builds its own `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` and workspace under the work dir
and leaves the evidence there: `logs/mock.log` (full provider transcript), `logs/*.json` (validate /
install / approve / inspect output), `state/resolved.jsonl` (what the responder decided per
request), `data/muse/plugins/data/<pid>/mcp-<sid>.log` (each server's JSON-RPC log) and
`data/muse/sessions/…/session.jsonl`. Use a different `MOCK_PORT` per concurrent run.

## The wire contract the responder implements

Learned by probing; nothing here is documented by Meta. Numbers refer to `1.0.1-R2006.1`.

**Request** (`POST <base>/responses`, JSON): `model`, `instructions` (~36 KB system base),
`input[]`, `tools[]`, `max_output_tokens`, `store:false`, `reasoning`, `include`,
`prompt_cache_key`, `stream:true`. `input[]` items are `{"type":"message","role":"developer"|"user",…}`,
`{"type":"function_call","id","call_id","name","arguments"}` and
`{"type":"function_call_output","call_id","output"}`. **A tool result comes back as the `output` string
of a `function_call_output` item in the next request**, verbatim (`content[0].text` of the MCP
result).

**`tools[]` are namespace groups**, not flat functions:

```json
{"type":"namespace","name":"mcp__plugin_omm_up","description":"Tools provided by an MCP server.",
 "tools":[{"type":"function","name":"echo_upper","description":"…","parameters":{…}}]}
```

The built-ins sit in a namespace called `muse` ("Muse Code tool set."). The MCP namespace on the wire
is the *surface* namespace from `runtime.mcp_tool_identity_catalog`: verbatim while
`len("mcp__plugin_<pid>_<sid>") <= 31`, i.e. `len(pid)+len(sid) <= 18`; otherwise rewritten to
`first-17 + "__" + 12 hex`.

**Response**: `text/event-stream`, `event:`+`data:` frames, `sequence_number` on **every** frame
(omit it and every frame is `provider_stream_decode_error`), and only these event types:
`response.created`, `response.function_call_arguments.done`, `response.output_item.done`,
`response.completed`. Emitting `response.output_item.added`, `response.in_progress` or
`response.content_part.*` makes Muse fail the stream with `error_kind: "decode"` and retry ten
times, after which it sends a request with **no tools** and a developer message "no tools are
available for this request". A bare greeting prompt (`hi`) never reaches the provider at all — it is
answered locally.

**How a namespaced call must be named** (`function_call.name`), measured in
`docs/experiments/mcp-tools-call.md`:

| form | example | dispatches? |
|---|---|---|
| `<ns>.<fn>` | `mcp__plugin_omm_up.echo_upper` | **yes, for every namespace length** (default here) |
| `<ns>__<fn>` | `mcp__plugin_omm_up__echo_upper` | yes **only** while the wire namespace equals the canonical one (pid+sid ≤ 18); for a rewritten namespace it is `unknown tool` |
| canonical id | `mcp__plugin_omm_sixteencharsxxxx__echo_upper` | yes (it is the session tool id) — but a 32+ char namespace never appears on the wire, so the model cannot know it |
| bare `fn`, `fn` + `namespace` field, `<ns>/<fn>` | | no — `tool unavailable: unknown tool …` fed back as a `function_call_output` |

**Step selection**: the responder picks plan step *n* where *n* = number of `function_call_output`
items in the request. This is idempotent under retries and needs no counter.

## Environment rules baked into the runner

- `env -i` with `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` under the work dir, `MUSE_NO_AUTO_UPDATE=1`,
  `TBH_CREDENTIAL_BACKEND=file`, `META_API_KEY=dummy` (env beats every stored credential, so the
  keychain is never consulted), `PATH` containing a `python3`.
- `MUSE_EXPERIMENTAL_PLUGINS=1` only on `plugins` verbs. Every `muse exec` / `muse serve` runs with
  the gate **unset** — runtime composition is ungated.
- `--yolo` on `exec` (no approval prompts, sandbox off, workspace trusted for the run). On `serve`
  approval is chosen on the wire: `session/start.approvalMode = "allowAll"`.
- `serve` has no `--provider`/`--base-url`: the runner writes `settings.json →
  {"provider":"meta","model":"test-model","endpoint_transport":{"base_url":"http://127.0.0.1:<port>"}}`
  *before* install so that `plugins approve` (which rewrites the file from a typed struct) merges into
  it. MSP `clientInfo.name` must match `^[a-z0-9_]+$` (SS1.4.1); every `commandId` must be a UUIDv7.
- A redirected `HOME` does not sandbox everything: Muse writes
  `~/Library/Application Support/Muse/session-name-authority/` under the **real** home on every run
  (`loose-ends.md` section 0, Trap B).

## Writing another responder

`mock.py` exec's the responder with globals `body` (request JSON as text), `path`, `headers`,
`state` (a scratch directory), `log` (the transcript path) and expects it to set `out`
(str/bytes), `ctype` (default `application/json`) and `code` (default 200). Point `MOCK_RESPONDER` at
it. Keep the four event types and `sequence_number`; copy `_ev`/`_resp` from `respond-toolcall.py`.
