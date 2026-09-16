# Meta Muse Code 1.0.1-R2006.1 — Model catalog, provider router, credentials

Reverse-engineering report for the `model` crate (catalog / meta client / meta provider /
provider_stream / router) and the `config` crate credential + endpoint-transport layers.

Binary: `/private/tmp/claude-501/.../scratchpad/muse-aarch64-macos`
(`muse-build/1.0.1 (non-interactive; macos-aarch64; build e27e408b666e693900118f778bd6c2880f88e432)`)

Every claim below is either **PROVEN** (reproducible command + captured output, or an exact
string from the binary with its grep) or explicitly marked **INFERRED**.

Sandbox used for all runtime work (never the user's real config):

```
D=/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/model-providers
HOME=$D/home XDG_CONFIG_HOME=$D/home/.config XDG_DATA_HOME=$D/home/.local/share
MUSE_NO_AUTO_UPDATE=1 TBH_CREDENTIAL_BACKEND=file
```

---

## 0. Executive summary

* Muse Code ships **one** compiled provider: `meta`, plus an offline `echo` provider.
  `openai`, `anthropic` and `openrouter` exist in the source tree behind Cargo features
  `provider-openai` / `provider-anthropic` / `provider-openrouter` and are **compiled out**
  of the public build. You cannot point this binary at a non-Meta model.
* The Meta wire protocol is an **OpenAI-Responses-shaped API**: `POST {base}/responses`,
  SSE streaming, `instructions` / `input[]` / `tools` / `reasoning.effort` /
  `include: ["reasoning.encrypted_content"]` / `prompt_cache_key` / `store:false`.
* The model catalog is **not** in the binary. `bundledCatalog` is empty; the catalog is
  fetched at startup from `GET {origin}/muse-code/models` (OpenAI `/v1/models`-shaped with a
  Meta-specific `metadata` block) and cached under `$XDG_DATA_HOME/muse/model-catalog`.
  Because it is a network document, **no shipped model id (e.g. "Muse Spark 1.2") exists in
  the binary** — only the product name "Muse Spark" in the system prompt.
* Reasoning-effort tiers `none|minimal|low|medium|high|xhigh|ultra`; `none` is rejected for
  the meta provider, and **`ultra` clamps to `xhigh` on the wire** (proven by request capture).
* Credentials: `META_API_KEY` env > saved credential (macOS Keychain by default, or
  `auth.json` when `TBH_CREDENTIAL_BACKEND=file`) > OIDC device login against
  `https://auth.meta.com` with `client_id=1031625952748946`, whose access token is exchanged
  ("minted") for a Model-API key at `POST {mint_base}/muse-code/key`.
* A settings-level **`model_catalog` array is a fully supported user extension point**
  (`source: "configCatalog"`), and `endpoint_transport` lets you re-point the origin, add
  mTLS/proxy/CA, and add arbitrary request headers.

---

## 1. CLI surface

### 1.1 Root flags that touch the model layer

`muse --help` (verbatim excerpt):

```
      --provider <MODE>
          Startup provider: echo or meta (default: meta)
      --preset <NAME>
          Run a built-in preset: native-basic, miniswe
      --model <MODEL>
          Model id for non-echo providers
      --reasoning-effort <EFFORT>
          Meta reasoning effort: none|minimal|low|medium|high|xhigh|ultra
          (default: high)
      --base-url <URL>
          Override the Meta provider base URL
      --parallel-tool-calls
          Enable Meta API parallel tool calls
      --no-parallel-tool-calls
          Disable Meta API parallel tool calls
      --echo-delay-ms <MS>
          Deterministic echo reply delay (echo provider only)
```

Validation (PROVEN):

```
$ muse exec --provider bogus "hi"
unsupported provider `bogus`; expected `echo` or `meta`

$ muse exec --reasoning-effort bogus "hi"
unsupported reasoning effort `bogus`; expected none|minimal|low|medium|high|xhigh|ultra

$ muse exec --reasoning-effort none "…"     # provider meta
--reasoning-effort none is not supported with --provider meta; choose minimal|low|medium|high|xhigh|ultra

$ muse exec --preset bogus "hi"
unknown preset bogus; expected native-basic|miniswe
```

Note `--echo-delay-ms` is a **root-only** flag; `muse exec --echo-delay-ms 0` errors with
`unknown option --echo-delay-ms`.

### 1.2 auth / login / logout

```
$ muse auth --help
muse auth — store provider API credentials

Usage: muse auth set [--provider <PROVIDER>] --api-key-stdin

The API key is read from stdin (never taken as a command-line argument, so it
never lands in shell history). Run `muse auth set --help` for provider options.

$ muse auth set --help
Save provider credentials. The API key is read from stdin via --api-key-stdin; it is never passed on the command line.

Usage: muse auth set [OPTIONS]

Options:
      --provider <provider>  Which provider the credential is for. Defaults to `meta`. [default: meta] [possible values: meta]
      --api-key-stdin        Read the API key from stdin (the only accepted way to pass a secret).
  -h, --help                 Print help

$ muse login --help
usage: muse login

Log in with your Meta account: approve a code in your browser.
META_API_KEY always takes priority over the account login.

$ muse logout --help
usage: muse logout

Remove the saved Meta credential (API key or Meta-account login).
META_API_KEY in the environment is not touched.
```

`--provider` on `auth set` has exactly one possible value: `meta`. That is clap-level proof
that no other provider credential slot is reachable in this build.

---

## 2. Provider set: what is actually compiled in

### 2.1 Four provider ids exist in the source; three are feature-gated out

Exact strings (from `strings -a -n 4`, offsets in `strings4.txt`):

```
bprovider `openai` is not compiled into this build; enable the `provider-openai` feature or choose
hprovider `anthropic` is not compiled into this build; enable the `provider-anthropic` feature or choose
jprovider `openrouter` is not compiled into this build; enable the `provider-openrouter` feature or choose
```

(the leading byte is the Rust `&str` length prefix from the adjacent length table)

Provider-id vocabulary literal: `Muse Codeopenaianthropicopenrouter` and
`metaopenaianthropicopenrouter` (next to `not logged in: run /login to add an API key`,
`muse.provider`, `muse.provider_alias`, `provider_unavailable`).

Runtime confirmation (PROVEN) — `settings.json` `{"provider":"openai","model":"gpt-5"}`:

```
$ muse exec --json --no-session-log "hi"
tbh: settings provider `openai` is unavailable in this build; falling back to meta
muse: local session messaging disabled: session logging is required
missing meta credentials: run `muse login` or set META_API_KEY, or save credentials at <config>/muse/auth.json
```

MSP confirmation (PROVEN) — `session/setModel` with a foreign provider is rejected:

```
openai  gpt-5     -> {"code":-32030,"message":"session/setModel command … rejected: unsupported_route",
                      "data":{"kind":"commandRejected","retryable":false,"reason":"unsupported_route"}}
anthropic claude-x-> … "reason":"unsupported_route"
openrouter z      -> … "reason":"unsupported_route"
echo echo         -> … "reason":"unsupported_route"
(no providerId)   -> … "reason":"invalid_target"
providerId="muse", modelId="" -> … "reason":"invalid_model"
```

`ModelReconfigureFailureKind` (string table):
`invalid_model, unsupported_model, unsupported_provider, unsupported_route,
validator_unavailable, run_active, invalid_target, command_id_conflict, apply_failed`.

### 2.2 The multi-provider abstraction that is still in the binary

Even though the code paths are gated out, the shared option/credential tables were compiled
in. Per-provider **request option keys** (one contiguous literal run):

```
meta.instructions   openai.instructions   openrouter.instructions
meta.prompt_cache_key   openai.prompt_cache_key   openrouter.session_id
META_API_KEY   OPENAI_API_KEY   ANTHROPIC_API_KEY   OPENROUTER_API_KEY
meta.max_output_tokens   openai.max_output_tokens   anthropic.max_tokens   openrouter.max_tokens
```

and, elsewhere:

```
meta.model   openai.model   anthropic.model   openrouter.model
meta.reasoning.effort   openai.reasoning.effort   anthropic.output_config.effort   openrouter.reasoning.effort
anthropic.thinking   anthropic.thinking.budget_tokens
openrouter.provider.order      meta.session_id
anthropic.messages[].content:tool_result
```

Provider-option provenance tags: `explicit_prompt_cache_key`,
`session_id_manual_provider_order`, `session_sticky_route`, `configured_provider_option`,
plus the error `provider route is not served by this host`.

Base-URL env vars are present in the env-var table but there is no compiled provider to
consume them in this build (INFERRED, from 2.1):
`ANTHROPIC_API_KEY, ANTHROPIC_BASE_URL, OPENAI_API_KEY, OPENAI_BASE_URL,
OPENROUTER_API_KEY, OPENROUTER_BASE_URL`.

Other errors that betray the multi-provider design:
`provider does not support base instructions`,
`parallel_tool_calls is only supported with provider meta`,
`model catalog is not supported for this provider`.

### 2.3 The `echo` provider

`--provider echo` runs fully offline. Proven end-to-end:

```
$ muse exec --json --approval-mode never --provider echo "say hi"
… "payload_type":"run.terminal.completed" … "text":"echo: say hi"
```

Task kind for the model step is `model.unknown.response`. Echo-specific machinery lives in
`fbcode/musecode/build/src/crates/tui/src/startup/echo_runtime/response_gate.rs`
(strings: `failed to stage echo gate arrival marker`, `failed to publish echo gate arrival
marker`, `echo response gate was not released`, `failed to read echo gate release marker`,
`muse-tui-echo`). The reply is literally `echo: ` + prompt, with an optional
`--echo-delay-ms` delay.

---

## 3. The model catalog

### 3.1 Catalog sources (MSP `ModelCatalogSource`, exported schema)

`muse schema generate-json-schema --out DIR` → `$defs.ModelCatalogSource`:

```json
{
  "description": "Where a model catalog came from (tdd SS3.10). **Open**: an unrecognized\nvalue is an unknown source, not an error.",
  "enum": ["providerCatalog","fakeCatalog","unresolvedCatalog","bundledCatalog","configCatalog"],
  "type": "string", "x-msp-openness": "open"
}
```

Doc strings in the binary: `providerCatalog` "A live provider catalog.", `fakeCatalog`
"A test fake.", `unresolvedCatalog` "A production host whose catalog has not been resolved
yet.", `bundledCatalog` "The compiled-in fallback.", `configCatalog` "A user-configured
fallback."

Internal (snake_case) spellings: `provider_catalog fake_catalog unresolved_catalog
bundled_catalog config_catalog`.

### 3.2 The bundled catalog is EMPTY in this build (PROVEN)

MSP query over stdio (no auth, no network):

```
$ muse serve --no-session-log            # then, on stdin:
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"clientInfo":{"name":"probe","version":"0"}}}
{"jsonrpc":"2.0","method":"initialized","params":{}}
{"jsonrpc":"2.0","id":2,"method":"model/list","params":{}}

<< {"jsonrpc":"2.0","id":2,"result":{"providerId":"meta","profileId":"tbh","source":"bundledCatalog","models":[]}}
```

Local-tracing corroboration (`$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-*.log`):

```
event="model_catalog.normalize" outcome="failed" reason="empty_visible" source="bundled"
  examined_rows=0 output_rows=0 ignored_hidden_rows=0 conflicts=0 rejected_rows=0
event="model_catalog.compose" outcome="bundled_fallback" source="bundled"
  bundled_rows=0 network_rows=0 config_rows=0 accepted_config_rows=0
  rejected_config_rows=0 conflicts=0 output_rows=0
```

Consequence: **there is no shipped model id in the binary**, and the MSP schema explicitly
allows this — `ModelListResult`: *"`models` MAY be empty — a build shipped with no bundled
models is a supported configuration."* The default provider/profile pair is
`providerId="meta"`, `profileId="tbh"`.

The only "Muse Spark" reference is the system-prompt preamble:

```
You are Muse Code, an agentic coding CLI (command line interface) that helps users with
software engineering tasks. You are powered by Muse Spark, a large language model trained
by Meta MSL. When asked who you are, identify yourself as "Muse Code powered by Meta Muse Spark".
```

No `Muse Spark 1.2` / `spark-1.x` / `llama-*` model id string exists anywhere in the binary
(`grep -aE 'Spark ?1|spark[-_.]?1' strings4.txt` → 0 hits).

### 3.3 Provider (network) catalog wire format

Endpoint (PROVEN by request capture against a local server):

```
GET {origin}/muse-code/models
H: authorization: Bearer <api key>
H: x-client-id: tbh:exec
H: accept: */*
H: user-agent: muse-build/1.0.1 (non-interactive; macos-aarch64; build e27e408b66…)
H: accept-encoding: gzip,deflate,br
```

Response type names + field lists (adjacent serde literals in the binary):

```
Model        { id, object, created, owned_by, metadata }     ("struct Model with 5 elements")
ModelList    { data, … }                                     ("struct ModelList with 2 elements")
TbhMetadata  { family, release_date, is_hidden, attachment, reasoning, temperature,
               tool_call, modalities, limit, options, roles, description, cost }
Modalities   { … }
Limits       { context, … }
TbhOptions   { reasoningEffort, forceReasoning, include, top_p }
TbhCost      { input, output, cached }
ToolSurfaceName { namespace, name }
```

Empirically (PROVEN by mutating a local fake `/muse-code/models`):

* `data` is **required** — `{}` and `{"object":"list"}` → `Provider returned malformed
  response data.`; `{"data":[]}` → parses, then `model catalog has no visible models`.
* Unknown envelope keys are ignored (`"zzz":1` accepted).
* Row fields are all optional except that an empty `id` is caught:
  `{"data":[{"id":"","metadata":{…}}]}` → `model catalog row has an empty model id`.

Normalizer rejection messages (string table):

```
model catalog provider id is required
model catalog default model is not present in visible rows
model catalog row profile does not match request profile
model catalog contains duplicate model ids
model catalog row provider does not match request provider
model catalog row has an empty model id
model catalog has no visible models
model catalog is not supported for this provider
```

`model_catalog.normalize` failure reasons: `provider_mismatch, duplicate_model,
empty_visible, missing_default`.

### 3.4 MSP-facing catalog row (`model/list`)

```json
ModelCatalogEntry {
  "modelId": string,            // the selectable model id
  "providerId": string,         // provider routing
  "profileId": string|null,     // provider profile
  "displayLabel": string,
  "releaseDate": string|null,
  "description": string|null,
  "contextLimit": integer|null,
  "outputLimit": integer|null,
  "cost": ModelCost|null,
  "isDefault": boolean,         // "May be false for every row."
  "isActive": boolean           // set only when sessionId was supplied
}
ModelCost { "input": string, "output": string, "cached": string, "currency": string|null }
```

`ModelCost` doc: *"Per-1M-token catalog cost, carried **verbatim** for display and never
rounded or re-formatted by the host (tdd SS3.10). Cost arithmetic stays client-local view
math."* — currency is *"nullable — today's catalogs are USD."*

`model/list` doc: *"Reads the catalog of models the host will accept in `session/setModel`;
a query, not a command (SS3.10)"*; rows are *"Visible rows only, newest `releaseDate`
first."*; *"Catalog rows marked hidden never reach the wire, so a client cannot select one."*

Internal row type (`ModelCatalogRow`, 14 fields):
`provider_id, profile_id, model_id, display_label, visibility, release_date, display_order,
is_current, is_default, roles, context_limit, output_limit, description, cost`.
`ModelCatalogVisibility = visible | hidden`.

### 3.5 The config catalog — a first-class user extension point (PROVEN)

`settings.json` accepts a `model_catalog` array of `ConfigCatalogRow`:
`model_id, provider_id, profile_id, display_label, visibility, release_date, display_order,
is_default, context_limit, output_limit, description`.

```json
{
  "schema_version": 1,
  "model_catalog": [
    { "model_id": "cfg-model-1", "provider_id": "meta", "profile_id": "tbh",
      "display_label": "Config Model One", "visibility": "visible",
      "release_date": "2026-02-02", "display_order": 3, "is_default": true,
      "context_limit": 500000, "output_limit": 32000,
      "description": "row from settings.json" }
  ]
}
```

`model/list` then returns:

```json
{"providerId":"meta","profileId":"tbh","source":"configCatalog",
 "models":[{"modelId":"cfg-model-1","displayLabel":"Config Model One","providerId":"meta",
            "profileId":"tbh","releaseDate":"2026-02-02","description":"row from settings.json",
            "contextLimit":500000,"outputLimit":32000,"cost":null,
            "isActive":false,"isDefault":true}]}
```

tracing: `event="model_catalog.normalize" outcome="ready" reason="none" source="config"
examined_rows=1 output_rows=1 …`

A malformed block is tolerated: `tbh: ignoring malformed model_catalog in settings:` .

### 3.6 Catalog cache on disk

`fbcode/musecode/build/src/crates/tui/src/startup/meta_catalog_cache/fetch.rs` (lines 92, 99,
236 appear in the tracing metadata table).

* Path key `model_catalog_cache`, resolved from the **data root**:
  `event="path.resolved" kind="model_catalog_cache" source="data_root" state="absent"`.
* Directory name: `model-catalog` under `$XDG_DATA_HOME/muse` (adjacent literal run
  `memory  snapshots  model-catalog  feature-config`; the bundled doctor skill lists
  *"Data dirs: `sessions`, `memory`, `model-catalog`, and `crashes` under the data dir"*).
* Invariant string: **`model catalog cache stores only the provider (network) catalog
  (#1958 INV-007)`**.
* Cache states: `stored | failed | rejected_source | invalid | read_failed | missing`.
* Diagnostics: `model_catalog.cache_load` (`state`, `row_count`, `duration_ms`),
  `model_catalog.cache_write`, `model_catalog.compose`
  (`bundled_rows, network_rows, config_rows, accepted_config_rows, rejected_config_rows,
  conflicts, output_rows`).
* Failure strings: `tbh: failed to write model catalog cache: `,
  `failed to write model catalog cache: `, `model catalog fetch failed: `,
  `…the stored login no longer authorizes the model catalog `,
  `tbh serve: startup model-catalog fetch failed (…); no cached default — composing the
  logged-out fallback`.

Compose order (from `model_catalog.compose` fields, INFERRED ordering):
bundled rows ← network rows ← config rows, with conflicts counted and config rows split into
accepted/rejected.

### 3.7 Open question: provider rows are always classified "hidden" in this build

With a synthetic `/muse-code/models` response every provider row is counted as
`ignored_hidden_rows`, regardless of `metadata.is_hidden`:

```
event="model_catalog.normalize" outcome="failed" reason="empty_visible" source="provider"
  examined_rows=1 output_rows=0 ignored_hidden_rows=1 conflicts=0 rejected_rows=0
```

Tested and rejected as the visibility key: `metadata.is_hidden:false|true|0`, `isHidden`,
`hidden`, `visibility`, `visible`, `is_visible`, `status`, `available`, `enabled`,
`is_current`, `current`, `is_default`, `default`, `display_order`, `deprecated`,
`internal`, `profiles`, `surfaces`, `roles:{}`; the same keys at row level; `owned_by` in
{`probe`,`meta`,`tbh`}; a 40-value `roles` array (`main, chat, code, agent, tbh, default,
primary, approval_review, summarizer, compaction, reminder, subagent, assistant, completion,
responses, coding, muse, muse-code, tbh:exec, exec, tui, desktop, general, standard,
contributor, spark, …`); model ids `muse-spark-1.2`, `meta/muse-spark`, `llama-4-maverick`;
and envelope keys `default_model`, `default`, `profile`, `provider`.
The visibility predicate for `providerCatalog` rows is therefore **unknown** — the config
catalog (§3.5) is the only source I could make visible.

---

## 4. Reasoning effort

### 4.1 The tier vocabulary

MSP `$defs.ReasoningEffort` (exported schema, verbatim):

```json
{
  "description": "The reasoning-effort tier sampled at submission (tdd SS3.2, SS3.3). The\n**same closed tier vocabulary** on both the fresh-turn and steer lanes,\nspelled identically; invalid tiers are invalid params. `none` is a tier of\nthe vocabulary (ask for no reasoning), not a way to say \"unset\".",
  "enum": ["none","minimal","low","medium","high","xhigh","ultra"],
  "type": "string",
  "x-msp-openness": "closed"
}
```

Internal Rust enums with the same variants: `WireReasoningEffort`, `MetaReasoningEffort`,
`ReasoningEffortV1` (enterprise defaults). Note the string table stores the tiers as the run
`minimallowmediumxhighultra` — `high` is absent because the linker merged it as a suffix of
`xhigh`.

### 4.2 Wire mapping (PROVEN by capturing `POST /responses`)

| `--reasoning-effort` | body `reasoning` |
|---|---|
| `none`    | rejected: `--reasoning-effort none is not supported with --provider meta` |
| `minimal` | `{"effort":"minimal","summary":"auto"}` |
| `low`     | `{"effort":"low","summary":"auto"}` |
| `medium`  | `{"effort":"medium","summary":"auto"}` |
| `high`    | `{"effort":"high","summary":"auto"}` (default) |
| `xhigh`   | `{"effort":"xhigh","summary":"auto"}` |
| **`ultra`** | **`{"effort":"xhigh","summary":"auto"}`** |

Repro:

```
settings.json: {"schema_version":1,"endpoint_transport":{"base_url":"http://127.0.0.1:8099","auth":"bearer"}}
MUSE_MODEL=env-model-x META_API_KEY=FAKE TBH_CREDENTIAL_BACKEND=file \
  muse exec --json --approval-mode never --reasoning-effort <tier> "what is the capital of France"
# local server logs the POST body
```

The `include` array is always `["reasoning.encrypted_content"]`; `reasoning.summary` is
always `"auto"` (there is also a `tui.reasoning_summaries` boolean that only controls
rendering).

### 4.3 The product contract for the tiers

The bundled `manage-settings` skill body (recovered verbatim from the binary) states:

> For Meta, the persistent effort tiers are `minimal`, `low`, `medium`, `high`, `xhigh`, and
> `ultra`. `high` is the default Meta baseline; `xhigh` is the opt-in premium precision tier.
> `ultra` remains the saved client selection, clamps to `xhigh` on the Meta wire, and
> currently enables proactive workflow/delegation guidance when that tool surface is
> available. It may proactively run multi-agent workflows and increase token usage quickly.
> […] Do not invent reasoning-effort tiers, budgets, ratios, mappings, or effects […] In
> particular, do not say a higher provider tier performs more, deeper, longer, or better
> reasoning. The only current workflow delta named by this contract is `ultra`'s proactive
> guidance.

Related strings: `may proactively run multi-agent workflows and can increase token usage
quickly`, `meta.reasoning.effort.select` (telemetry event),
`active_model_effort_transition`, `MUSE_DISABLE_ULTRA_ANIMATION`, `UltraActivationTick`,
`DelegationPromptProfile { unified_proactive, reinforced_proactive }`.

Mid-session change reminders (verbatim):

```
The active model changed mid-session: <…>. Reasoning effort is unchanged. This supersedes any
earlier statement in this conversation about which model is active, including your own.

Reasoning effort changed mid-session: <…>. The active model is unchanged. This supersedes any
earlier statement in this conversation about the reasoning effort, including your own.
```

### 4.4 First-turn effort shortcut — and a hard-coded canned reply

`settings.first_turn_minimal_effort_regex` + gate `MUSE_EXPERIMENTAL_FIRST_TURN_MINIMAL_EFFORT`.

**PROVEN, and surprising:** greetings are answered from a table baked into the binary with
**zero model calls**, even with the gate off and no regex configured:

```
prompt          POSTs  terminal text
"hi"            0      'Hi! What would you like to work on?'
"hello"         0      'Hello! How can I help you today?'
"what is 2+2"   1      (real request went out)
```

With `MUSE_EXPERIMENTAL_FIRST_TURN_MINIMAL_EFFORT=1` and
`"first_turn_minimal_effort_regex": "capital"`, the prompt `capital of France` also returned
a canned `'Hi! How can I help you today?'` with 0 POSTs.

The binary contains ~8 such fixtures, each a **captured real Muse Spark response**: a message
id (`msg_019fcfea…`), an assistant text, a reasoning summary, an encrypted reasoning item id
pair (`rs_6a72aa91154d8e3e933a439e:rs_019fcfea471f7ef29a708386bb9aad3e`) and a base64
`reasoning.encrypted_content` blob. Recovered texts + summaries:

```
"Hi! What would you like to work on?"     / "Simple greeting. Reply accordingly."
"Hello! How can I help you today?"        / "User says hi. Need to respond accordingly. Simple greeting.\nWe should answer directly."
"Hi — how can I help?"                    / "Need to respond to hi trivially."
"Hello — how can I help?"                 / "User says hi. Simple greeting. Respond in one line, ask what they want."
"Hi! What can I help you with today?"     / "User says hi. Respond simply."
"Hi! How can I help you today?"           / "User says hi. Respond accordingly."
(one more)                                / "User says hi. Respond concisely."
(one more)                                / "Simple greeting, respond directly."
```

The turn-submit command payload carries the matching fields
`first_turn_canned_response`, `first_turn_canned_reasoning_item`
(literal run `…workflow_child_effort_overridefirst_turn_canned_responsefirst_turn_canned_reasoning_item…`).

---

## 5. The Meta provider wire protocol

### 5.1 Base URL and path rules (PROVEN)

Default base URL literal: `https://api.meta.ai/v1` (with `https://api.meta.ai` used
separately as the mint/auth origin).

Two different path rules, demonstrated with `base_url = http://127.0.0.1:8099/v1`:

```
GET  /muse-code/models      <- product paths are ROOT-RELATIVE to the ORIGIN (base path dropped)
POST /v1/responses          <- the model API path is RELATIVE TO THE FULL BASE URL
```

and with `base_url = https://127.0.0.1:1/x?q=1` the catalog request went to
`https://127.0.0.1:1/muse-code/models` — path *and* query discarded. Matching strings:

```
provider product path must be root-relative without query or fragment
provider endpoint override must be origin-only
provider endpoint override is invalid
provider endpoint is unavailable
invalid base url: unsupported scheme `ftp`; expected http or https
```

Product paths found in the binary:

| path | purpose |
|---|---|
| `/muse-code/models` | model catalog |
| `/muse-code/search` | hosted `web_search` |
| `/muse-code/browser_open` | hosted `web_fetch` / browser open |
| `/muse-code/config` | remote feature config (gates) |
| `/muse-code/feedback` | `/feedback` rageshake intake |
| `/muse-code/telemetry/traces`, `/muse-code/telemetry/logs` | OTLP-ish telemetry |
| `muse-code/key` | API-key mint (OAuth → Model API key) |
| `muse-code/logout` | credential revoke |
| `/responses` | the model call (relative to base incl. path) |

### 5.2 Request headers (captured verbatim)

```
=== POST /v1/responses
H: authorization: Bearer FAKE
H: x-client-id: tbh:exec
H: x-tbh-session-id: 01a05d82-769a-7f83-9de7-bd21502a83d9
H: x-meta-ai-gateway-session-id: 01a05d82-769a-7f83-9de7-bd21502a83d9
H: traceparent: 00-a18cf1420c4b4ea2a11157396896f457-20c8820a2927e02b-01
H: accept: text/event-stream
H: content-type: application/json
H: user-agent: muse-build/1.0.1 (non-interactive; macos-aarch64; build e27e408b666e693900118f778bd6c2880f88e432)
H: accept-encoding: gzip,deflate,br
```

Header/identity strings: `x-client-id`, `x-fb-routing-control`, `x-api-version` (`1.0.0`),
`x-tbh-session-id`, `x-meta-ai-gateway-session-id`, `traceparent`, plus response-side
`x-fb-request-id`, `x-request-id`, `x-fb-trace-id`.

Client surface (the `x-client-id` value) is one of `tbh:tui`, `tbh:exec`, `tbh:desktop`:

```
meta client surface not set: declare tbh:tui/tbh:exec/tbh:desktop via
ClientBuilder::surface(..) or set_process_client_surface(..) at the entrypoint
```

`default_c1` is the default client-id literal; `client id is not a valid header value` guards it.

### 5.3 Request body (captured, `--reasoning-effort ultra`)

Top-level keys, in order:

```json
{
  "model": "env-model-x",
  "input": [ {"type":"message","role":"developer","content":"<system-reminder …>"},
             {"type":"message","role":"user","content":"what is the capital of France"} ],
  "instructions": "You are Muse Code, an agentic coding CLI …",   // 36 613 bytes
  "max_output_tokens": 128000,
  "store": false,
  "tools": [ { "type":"namespace", "name":"muse", "description":"Muse Code tool set.",
               "tools":[ {"type":"function","name":"workflow", …}, … ] } ],
  "reasoning": {"effort":"xhigh","summary":"auto"},
  "include": ["reasoning.encrypted_content"],
  "prompt_cache_key": "tbh:main:01a05d7e-ac4c-7110-80b9-b220f849f9c4",
  "stream": true
}
```

* `tools` is a **single namespace object** (`{"type":"namespace","name":"muse"}`) wrapping 20
  functions: `workflow, read_file, search, write_file, edit_file, read_memory, add_memory,
  edit_memory, web_search, bash, bash_input, cron_create, cron_delete, cron_list, get_goal,
  create_goal, update_goal, report_progress, read_skill, write_todos`.
* `prompt_cache_key` = `tbh:main:<session uuid>` (matches the option key
  `meta.prompt_cache_key` and provenance `explicit_prompt_cache_key`).
* Total body for a trivial prompt in an empty workspace: **106 615 bytes**.

Wire vocabulary from the serde field tables (request + stream items):

```
model instructions max_output_tokens previous_response_id store parallel_tool_calls
tool_choice tools type message content id phase function_call_output call_id execution
arguments tool_search_output status function_call encrypted_content summary
input_text input_image image_url detail input_video file_id
roles: user assistant system developer tool
effort format schema strict
meta.include            include values: web_search_call.results, reasoning.encrypted_content
```

Stream event types (`ResponseSnapshot`, `ContentPartEvent`, `TextDeltaEvent`, `TextDoneEvent`,
`SummaryTextDeltaEvent`, `FunctionCallArgumentsDelta/Done`, `OutputItemEvent`,
`SubscriptionUsageEvent`), SSE terminator `data: [DONE]`, and:

```
stream event missing `type` field
non-utf8 SSE frame
assistant_message  tool_search_output  hosted_web_search_replay  output_text  reasoning_text
completed visible output item missing non-empty `id`
```

### 5.4 Error taxonomy

`ApiErrorBody { message, param, resets_at, error, … }` (5 elements) with kinds:

```
RateLimit  Overloaded  AuthenticationFailed  OauthOrgNotAllowed  BillingError
InvalidRequest  ModelNotFound  ServerError  MaxOutputTokens  Unknown
```
plus `permanent_client_error, error_envelope, error_code, error_type, error_detail,
provider_category, file_not_found, invalid_request_rejection, resets_at, request_id, retry_after`.

`provider_stream.terminal` categories:

```
rate_limited content_policy model_not_found invalid_request transport transport_error
protocol_error decode_error config_error context_length server_error client_error
auth_error http_error
```

Upstream gateway strings passed through verbatim:

```
ServiceException: tokens-out: upstream stream error: upstream engine terminated the response stream before completion
ServiceException: tokens-out: upstream stream error: router timed out reading the response stream from the upstream engine
ServiceException: tokens-out: upstream stream error: Requested token count exceeds the model's maximum context length of …
servicerouterexception(streaming_chunk_timeout) (connect_unknown) (streaming_server_closing_connection)
response_incomplete_no_message_output  generic_generation_failed
model hallucination / produced malformed output / tool call is malformed
```

Stream tracing (`model/src/meta/provider/status_summaries/local_tracing.rs`) fields:
`attempt, max_attempts, has_http_status, http_status, request_id, trace_id_state, bytes_read,
has_time_to_first_event, time_to_first_event_ms, eof, eof_buffer_len`.
`TBH_PROVIDER_TRACE_ROOT` selects a provider trace dump root;
`TBH_STREAM_IDLE_TIMEOUT_SECS` / `TBH_STREAM_FIRST_EVENT_TIMEOUT_SECS` bound the stream.

Secret redaction in traces/logs: `[REDACTED_URL]`, `Bearer [REDACTED]`, `[REDACTED]`, and
prefix matchers `github_pat_`, `sk-ant-`, `sk-`, `xoxb- xoxp- xoxa- xoxr- xoxs-`.

### 5.5 Retry policy (PROVEN)

`settings.provider_retry` = `{ max_retries | max_attempts, base_delay_ms, retry_after_cap_ms }`
(validation string: `provider_retry must not set both max_retries and max_attempts`,
`max_attempts must be greater than 0`).

Default is **5 attempts**; with `{"max_attempts":2}` exactly 2 POSTs went out and the run
terminal was:

```
Provider returned malformed response data. (after 2 provider attempts) (response: resp_1)
```

Enterprise `ProviderRetryDefaultsV1` has 3 elements (`max_retries, base_delay_ms,
retry_after_cap_ms`).

### 5.6 Files API (image / video input)

`Meta Files API upload cancelled|failed`, `Meta Files API returned an invalid file binding`,
`UploadResponse { id, bytes, created_at, expires_at, object, purpose, status, … }` (8),
`ModelInputFileUploaded { provider, file_id, expires_at }`,
`ModelInputFileUnavailable { reason: not_found }`,
env `TBH_META_FILE_EXPIRY_SECS`, `TBH_VIDEO_INPUT_ENABLED`.
Accepted image media types: `image/png image/jpeg image/webp` (and `image/gif` in the tool
result path); video: `video/mp4 video/quicktime`.
Input part types: `input_text`, `input_image` (`image_url`, `detail`), `input_video` (`file_id`).

---

## 6. Provider router

`fbcode/musecode/build/src/crates/model/src/router/diagnostic.rs` emits an event with fields
`route` and `model` (lines 12 and 25).

Route-selection vocabulary (`provider_route.select`, contiguous literals):

```
ready none rejected transport_build unsupported_scheme
config provider_build provider_transport.build custom default inherited bearer
completed cancelled
invalid provider_owned provider_route.select provider_unavailable unavailable
proxy_mtls_conflict proxy direct mtls injected injected_search_only
```

`provider_transport.build` event (from `model/src/meta/client/diagnostic.rs:51`), observed live:

```
event="provider_transport.build" provider="meta" outcome="ready" reason="none"
  endpoint="custom" search_endpoint="inherited" transport="direct" auth="bearer"
  api_key_present=false duration_us=87
```

Sticky-route / provider-option resolution order (INFERRED from the provenance literals):
`explicit_prompt_cache_key` → `session_id_manual_provider_order` → `session_sticky_route` →
`configured_provider_option`. Failure: `provider route is not served by this host`.

Model-selection precedence (**PROVEN**, by reading `model` out of the captured POST body):

| configuration | wire `model` |
|---|---|
| `settings.model = settings-model` | `settings-model` |
| + `MUSE_MODEL=env-model` | `env-model` |
| + `--model flag-model` | `flag-model` |

Effort precedence: `settings.reasoning_effort=low` → `low`; adding `--reasoning-effort xhigh`
→ `xhigh`. `run_preset.resolve` reports the winning source as
`provider_source`/`model_source`/`agent_profile_source` ∈ {`none`,`settings`,`runtime`}
(`MUSE_MODEL` and `--model` both report `runtime`).

The durable record of a selection, from a real session log:

```json
"payload_type":"run.model.configured",
"payload":{"kind":"run_model_configured","provider_id":"meta","profile_id":null,
           "model_id":"env-model-x","display_label":"env-model-x","source":"startup"}
```

`EffectiveModelSource = startup | replay | model_reconfigure`;
`AgentRunConfigurationSource = runtime | settings | provider_default | default | legacy_unrecorded`;
MSP `ModelChangeSource = user | default | policy` (open enum).
A fresh `session/start` with no explicit provider reports `providerId: "muse"` and
`modelId: null` — the "logged-out fallback" alias (telemetry attrs `muse.provider`,
`muse.provider_alias`).

---

## 7. Credentials

### 7.1 Resolution order

`credential.status` tracing event (`config/src/credential_provider/diagnostic.rs:13`):

```
event="credential.status" provider="meta" source="environment" available=true
  login_present=false masks_login=false duration_ms=0
```

`source` ∈ {`environment`, `unavailable`, …}. `/status` renders the credential as one of
`Meta account`, `API key (unsaved)`, `META_API_KEY (environment)`, and warns:

```
note: META_API_KEY overrides your Meta account login; unset it to use the account login,
or run `muse logout` to remove the login.
```

Missing-credential messages:

```
missing meta credentials: run `muse login` or set META_API_KEY, or save credentials at <path>
missing <provider> credentials: set <ENV> or save credentials in <path>
missing meta credential; set <ENV> or save provider credentials
```

Order (PROVEN by the messages + `login_present`/`masks_login` fields):
`META_API_KEY` → stored provider credential → Meta-account (OAuth) login, which is then
minted into an API key.

### 7.2 `auth.json`

Path: `$XDG_CONFIG_HOME/muse/auth.json`, else `$HOME/.config/muse/auth.json`
(the config root also holds `settings.json` and `trust.json`). Mode `0600`, guarded by a
sibling lock file `.auth.json.lock`.

Produced live (PROVEN):

```
$ echo "FAKE-KEY-DO-NOT-USE-000" | TBH_CREDENTIAL_BACKEND=file muse auth set --provider meta --api-key-stdin
credentials saved for provider meta

$ cat $XDG_CONFIG_HOME/muse/auth.json
{
  "schema_version": 1,
  "providers": {
    "meta": {
      "api_key": "FAKE-KEY-DO-NOT-USE-000"
    }
  }
}
```

Types: `AuthFile { schema_version, providers }`, `ProviderCredential`,
`SecretPayload { secret_schema_version, api_key, access_token, refresh_token }`,
`OAuthTokens { access_token, refresh_token, expires_at, obtained_via, api_key, api_base_url,
user_full_name, user_email }`.

The OAuth slot additionally carries `mechanism: "oauth"` and a `storage` discriminator —
proven by the launcher, which reads
`providers.meta.mechanism == "oauth"` then `providers.meta.access_token` and
`providers.meta.expires_at` (`muse-launcher.sh:read_credential`). Binary literals:
`mechanism`, `oauth`, `storage`, `oauth slot did not serialize as an object`.

Errors: `malformed auth file at <p>`, `malformed auth file: providers.<x> must be a string`,
`unsupported auth schema version <n>`, `failed to read auth file at <p>`,
`failed to write credential file at <p>`, `grant write left no meta slot behind`.

### 7.3 Credential backends — `TBH_CREDENTIAL_BACKEND`

On macOS the **default backend is the Keychain**, and the file is only the fallback.
PROVEN indirectly and decisively: with `TBH_CREDENTIAL_BACKEND` unset every `muse exec`
blocked forever in this headless sandbox with `.auth.json.lock` open and no other work;
setting `TBH_CREDENTIAL_BACKEND=file` made the same command return instantly. Value `file`
is confirmed working; other accepted values are unknown (an unrecognized value also blocked,
i.e. it fell through to the keychain path).

Keychain-related strings:

```
Your saved login is in the macOS Keychain, which is locked in this session. Run
'security unlock-keychain' and retry, or log in again.
keychain item for <x> is unreadable (os status <n>)
keychain payload for <x> is malformed
keychain payload exceeds the 16 KiB cap
keychain item for <x> holds an unreadable payload
failed creating CFString
```

Telemetry captures the backend at session open:
`SessionOpenedRecord { …, credential_backend, keychain_fallback_reason, … }`, with
`keychain_fallback_file` and the note *"Backend state AT EMISSION (spec 8053 Telemetry
Impact): a post-open latch reports on the NEXT session's row. Absent only on pre-#8053
records; off-macOS …"*.

### 7.4 OIDC device-code login

The **binary itself** implements the same flow as the launcher (both sets of literals are
present):

```
base:      https://auth.meta.com          (override: TBH_AUTH_BASE_URL / MUSE_AUTH_URL)
client_id: 1031625952748946               (override: MUSE_CLIENT_ID)
authorize: /oidc/device/authorization/    (POST client_id)
token:     /oidc/device/token/            (POST grant_type, device_code, client_id)
grant:     urn:ietf:params:oauth:grant-type:device_code
header:    x-api-version: 1.0.0
types:     DeviceAuthorization, TokenGrant, OAuthErrorBody, ProblemBody, RevokeBody, MintedKey
```

Launcher (`muse-launcher.sh`, verbatim):

```bash
channel_url="${MUSE_CHANNEL_URL:-https://api.meta.ai/muse-code/channels/muse-stable}"
auth_url="${MUSE_AUTH_URL:-https://auth.meta.com}"
client_id="${MUSE_CLIENT_ID:-1031625952748946}"
download_host="${MUSE_DOWNLOAD_HOST:-lookaside.facebook.com}"
authorization_endpoint="/oidc/device/authorization/"
token_endpoint="/oidc/device/token/"
device_code_grant="urn:ietf:params:oauth:grant-type:device_code"
launcher_url="${MUSE_LAUNCHER_URL:-https://api.meta.ai/muse-launcher.sh}"
launcher_version="2"
user_agent="muse-code/launcher-$launcher_version"
credential_path="${MUSE_AUTH_PATH:-$credential_default}"   # $XDG_CONFIG_HOME/muse/auth.json
```

Device response fields consumed: `device_code, user_code, verification_uri,
verification_uri_complete, expires_in, interval` (interval defaults 5 s, lifetime clamped to
60–1800 s). Polling error handling: `authorization_pending` (keep polling), `slow_down`
(+5 s), `access_denied`, `expired_token`.

**`MUSE_AUTH_PATH` is a launcher-only variable** — it does not appear in the binary at all
(`grep -c MUSE_AUTH_PATH strings4.txt` → 0). The binary uses XDG resolution only.

The launcher only *reads* the credential store, and says so:

> Read-only view of the CLI's credential store. The launcher never writes it: the token from
> its own device login stays in memory for the run, so a caller who signs in here gains
> nothing on disk that the CLI did not put there.

It attaches `Authorization: Bearer <access_token>` only to `https://lookaside.facebook.com/…`
downloads.

TUI login flow (`tui/src/login/flow/activation.rs`, `tui/src/app/login/payment.rs`) strings:

```
Opening your browser / Waiting for approval / Esc cancel
Model API access verified.
You are logged in.
device login isn't available yet
device authorization endpoint not found (unlanded or gated off)
login request was denied / login request expired before it was approved
your saved login is no longer valid. Log in again or use a different account.
still unauthorized after a token refresh; run `muse login` again
outcomes: success mint_failed unavailable access_denied expired cancelled flow_failed store_failed
credential.refresh outcomes: success terminal mint_failed no_stored_login masked_by_environment
```

### 7.5 API-key mint

After the OAuth grant, the access token is exchanged for a Model-API key:

```
POST {TBH_MINT_BASE_URL:-https://api.meta.ai}/muse-code/key   -> MintedKey { api_key, base_url, … }
mint response missing api_key or base_url
mint returned an empty api key   /   refresh produced an empty api key
unrecognized Model API base URL from login; using the default.   (default https://api.meta.ai/v1)
mint flight interrupted at shutdown
no login to refresh a key from; run `muse login` or set META_API_KEY
your API key from META_API_KEY was rejected — update or unset it
your saved API key was rejected — run `muse login` or add a new key
```

`muse-code/logout` + `RevokeBody` back the `muse logout` path
(`logged out: removed the stored Meta credential`).

### 7.6 Pricing / entitlement tiers

There is **no "Standard vs Contributor" pair** in the binary (the only `Contributor` hits are
Apache-2.0 licence text). The real tiers are:

* `/status` "Billing" row values: **`Pay-as-you-go`** and **`Subscription`**.
* Tier ids in the payment/entitlement code: `payg`, `cli`, `no_payg`, `xgrade`.
* Entitlement errors:

```
your Meta account isn't set up for Model API yet — finish onboarding at https://dev.meta.ai, then log in again.
Model API requires a payment method on your account — add one at https://dev.meta.ai, then log in again.
Muse Code requires a payment method — subscribe at https://accountscenter.meta.com/muse_code/?ep=no_payg, then log in again.
```

* Subscription usage telemetry: `SubscriptionUsageEvent`,
  `SubscriptionUsageSnapshot { window, weekly, … }`,
  `SubscriptionWeeklySnapshot { used_percent, resets_at }`,
  `SubscriptionWindowSnapshot { used_percent, resets_at, window_duration_mins }`,
  `SubscriptionEntrypointImpressionRecord`,
  `PaymentBannerShownRecord { model, payment_tier, has_action_url }`,
  `PaymentUnresolvedAtPollCapRecord { remint_attempts }`,
  states `absent_store no_current_row non_active paused_active active paused usage_limited budget_limited`,
  gate `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH`, flag `show_subs_upsell`,
  credential kinds `env_api_key stored_api_key unsaved_api_key subscription payment_banner`.

---

## 8. `endpoint_transport` (settings)

Struct fields (serde literals): `base_url, proxy, auth, http_headers, client_cert,
client_key, ca_bundle`. Enterprise-defaults counterpart `EndpointTransportDefaultsV1` has 3
elements (`base_url, proxy, auth`); enterprise key paths are
`settings.endpoint_transport.base_url|proxy|auth`.

`auth` enum (**PROVEN**):

```
$ settings.json endpoint_transport.auth = "bogus"
malformed settings file at …/settings.json: unknown variant `bogus`, expected `bearer` or `none` at line 1 column 83
```
(`EndpointAuthV1 = bearer | none`)

Runtime validation (**PROVEN**, each via `muse exec`):

| config | message |
|---|---|
| `{"base_url":"ftp://x/"}` | `failed to fetch model catalog: invalid base url: unsupported scheme `ftp`; expected http or https` |
| `proxy` + `client_cert` + `client_key` | `invalid endpoint transport config: a proxy and an mTLS identity are mutually exclusive; set only one` |
| `ca_bundle` alone | `invalid endpoint transport config: mTLS requires both client_cert and client_key` |
| `base_url` with path+query | path/query silently dropped for product paths |

Other strings: `auth=bearer requires an api key, but none was provided`,
`ca bundle contains no certificate`, `client cert is unreadable`, `client key is unreadable`,
`ca bundle is unreadable`, `provider authentication is unavailable`,
`provider transport is unavailable`, `provider no-redirect transport is unavailable`.

Live diagnostic:

```
event="endpoint_transport.resolve" outcome="selected" transport="direct" auth="bearer"
  api_key_present=true reason="none" duration_ms=0
```

### 8.1 The "sanctioned front door" rule

```
Meta bearer withheld: base_url is off the sanctioned front door. To opt in, set the base_url
in settings `endpoint_transport` AND pin its `auth = "bearer"` (a settings pin does not vouch
for a --base-url flag).
```

Observed nuance (PROVEN): with `META_API_KEY` set, `--base-url http://127.0.0.1:8099` and
**no** settings pin, the `Authorization: Bearer` header *was still sent*. INFERRED: the
withholding rule protects the **minted/OAuth-derived** bearer, not an operator-supplied
`META_API_KEY`.

### 8.2 `MUSE_CUSTOM_HEADERS`

Format is **newline-separated `Name: value`** (PROVEN):

```
MUSE_CUSTOM_HEADERS=$'x-probe-one: 1\nx-probe-two: two'
=> H: x-probe-one: 1
   H: x-probe-two: two

MUSE_CUSTOM_HEADERS='x-probe-one: 1, x-probe-two: two'
=> H: x-probe-one: 1, x-probe-two: two          # one header, comma is not a separator

MUSE_CUSTOM_HEADERS='notaheader'
=> muse: MUSE_CUSTOM_HEADERS contains invalid provider header input; affected headers will not be sent
```

`authorization` cannot be injected (`provider request headers must not include
authorization`); supplying `MUSE_CUSTOM_HEADERS='authorization: Bearer evil'` left the real
`Authorization: Bearer FAKE` header intact in the captured request. The settings equivalent
is `endpoint_transport.http_headers` (and `env_http_headers` internally).

`MUSE_WWW_ROUTING` feeds the `X-FB-Routing-Control` header; every value I tried
(`latest prod www intern canary dev`) produced
`muse: MUSE_WWW_ROUTING is set to an unrecognized value; the X-FB-Routing-Control header will
not be sent` — the accepted vocabulary is unknown.

---

## 9. Enterprise policy: `model_egress`

`PolicyEnvelopeV1 { schema_version, execution, model_egress, extensions, privacy,
local_session_messaging }`.

```
PolicyModelEgressV1 { allowed_providers, allowed_models, model_fallback, web_search, web_fetch }
ModelFallbackV1     { provider, model }
ChoiceV1            { allowed, fallback }
```

Enterprise key paths: `model_egress.allowed_providers`, `model_egress.allowed_models`,
`model_egress.model_fallback`, `model_egress.web_search`, `model_egress.web_fetch`.

Runtime validation surface (PROVEN):

```
$ muse config validate --plane policy --file p.json
{"schema_version":1}                                            -> valid: plane=policy schema_version=1
{"model_egress":{"allowed_providers":{"allowed":["meta"]}}}     -> enterprise_document_invalid: reason=wrong_type location=model_egress.allowed_providers
{"model_egress":{"allowed_providers":["meta"]}}                 -> enterprise_document_invalid: reason=semantic_invalid location=model_egress.allowed_providers
{"model_egress":{"allowed_models":["m1"]}}                      -> enterprise_document_invalid: reason=wrong_type location=model_egress.allowed_models
{"execution":{"forbid_approval_bypass":true,…}}                 -> enterprise_document_invalid: reason=field_not_activated location=execution.forbid_approval_bypass
```

`field_not_activated` shows enterprise fields are individually gated by a field-activation
registry. The exact accepted shape for `allowed_providers` was not recovered (open question).

`muse config status` works offline:

```
Enterprise configuration status
Generation: sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4
Sources:
  plane=defaults source_class=system_file state=absent
  plane=policy   source_class=system_file state=absent
  plane=defaults source_class=macos_managed_preferences state=absent
  plane=policy   source_class=macos_managed_preferences state=absent
```

Source classes: `system_file`, `macos_managed_preferences`, `windows_machine_policy`.

`EnterpriseDefaultsSettingsV1` can pin `reasoning_effort`, `first_turn_minimal_effort_regex`,
`context_compaction`, `run`, `provider_retry`, `tui`, `context`, `tools`, `mcp_servers`,
`presets`, `max_consecutive_stop_hook_continuations`, `endpoint_transport`, `notifications`
— note it does **not** include `provider`/`model`/`model_catalog`. Validated live:

```json
{"schema_version":1,"settings":{"reasoning_effort":"xhigh",
 "first_turn_minimal_effort_regex":"^/?(hi|hello)$",
 "endpoint_transport":{"base_url":"https://example.invalid/v1","auth":"bearer"},
 "provider_retry":{"max_retries":4,"base_delay_ms":500,"retry_after_cap_ms":30000}}}
-> valid: plane=defaults schema_version=1
```

---

## 10. MSP model methods (exported schema, verbatim)

```
model/list        params ModelListParams   result ModelListResult
                  "Reads the catalog of models the host will accept in `session/setModel`;
                   a query, not a command (SS3.10)."
session/setModel  params SessionSetModelParams result SessionSetModelResult
                  "Reconfigures the session's model; the selection is durable and applies to
                   subsequent model calls (SS3.8)."
session/modelChanged (notification)  SessionModelChangedParams
```

```json
ModelSelection { "modelId": string (required), "providerId": string, "profileId": string|null,
                 "displayLabel": string }        // "Empty strings are normalized to absent."
SessionSetModelParams { "commandId": UUIDv7, "sessionId": string, "model": ModelSelection }
SessionSetModelResult { "commandId": string, "status": CommandStatus }
                 // "If a turn is running the selection is admitted now and applied at the
                 //  next model-call boundary; the ack does not wait for that boundary."
EffectiveModel { "modelId": string, "providerId": string|null, "source": ModelChangeSource }
```

`session/start` takes `providerId` + `modelId` ("server default when omitted");
`turn/start` takes `reasoningEffort`. `turn/start.providerRequestOptions` is deliberately
**absent from the published schema**: *"RULED (#22785 E3, owner, 2026-08-26) to stay off the
published schema."*

Schema fingerprint (stable and experimental surfaces are byte-identical in this build):
`sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7`.

---

## 11. Environment variables that touch this layer

| var | effect |
|---|---|
| `META_API_KEY` | Model API key; **overrides** any saved login (PROVEN by message + `masks_login`) |
| `MUSE_MODEL` | model id; beaten by `--model`, beats `settings.model` (PROVEN) |
| `MUSE_CUSTOM_HEADERS` | newline-separated extra provider headers; `authorization` stripped (PROVEN) |
| `TBH_CREDENTIAL_BACKEND` | `file` selects the plaintext `auth.json` backend instead of the macOS Keychain (PROVEN) |
| `TBH_AUTH_BASE_URL` | OIDC device-flow base (default `https://auth.meta.com`) |
| `TBH_MINT_BASE_URL` | API-key mint base (default `https://api.meta.ai`) |
| `MUSE_WWW_ROUTING` | value for `X-FB-Routing-Control`; unrecognized values are dropped with a warning |
| `TBH_PROVIDER_TRACE_ROOT` | provider request/stream trace dump root |
| `TBH_STREAM_IDLE_TIMEOUT_SECS`, `TBH_STREAM_FIRST_EVENT_TIMEOUT_SECS` | stream watchdogs |
| `TBH_META_FILE_EXPIRY_SECS`, `TBH_VIDEO_INPUT_ENABLED` | Files-API / video input |
| `TBH_DISABLE_FEATURE_CONFIG`, `TBH_DISABLE_TELEMETRY` | kill the `/muse-code/config` and telemetry calls |
| `MUSE_EXPERIMENTAL_FIRST_TURN_MINIMAL_EFFORT` | activates `first_turn_minimal_effort_regex` (PROVEN: canned reply, 0 model calls) |
| `MUSE_EXPERIMENTAL_MODEL_EFFORT_CONTEXT`, `MUSE_EXPERIMENTAL_REASONING_DISPLAY`, `MUSE_EXPERIMENTAL_PROVIDER_TOOL_SWITCH`, `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH` | model/effort-adjacent gates |
| `ANTHROPIC_API_KEY`, `ANTHROPIC_BASE_URL`, `OPENAI_API_KEY`, `OPENAI_BASE_URL`, `OPENROUTER_API_KEY`, `OPENROUTER_BASE_URL` | present in the env table; **no compiled consumer in this build** |
| `MUSE_NO_AUTO_UPDATE` | launcher/update suppression |
| `MUSE_AUTH_PATH` | **launcher only** — absent from the binary |

---

## 12. Reproduction harness used

Local logging server that answers `GET …/muse-code/models` from `catalog.json` and any other
path with an SSE body, writing every request (headers + full body) to `req.log` /
`body-*.json`: `$D/fs2.py`. Driver: `$D/try.sh`, MSP probes `$D/msp*.py`, catalog oracle
`$D/oracle.sh` (mutates `catalog.json`, runs `muse serve` + `model/list`, then greps
`model_catalog.normalize` out of `$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-*.log`).

The local-tracing bootstrap log is the single most useful offline oracle in this binary:
it is written unconditionally to `$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-<uuid>.log`
and contains `path.resolved`, `settings.load`, `run_preset.resolve`, `credential.status`,
`endpoint_transport.resolve`, `provider_transport.build`, `model_catalog.cache_load`,
`model_catalog.normalize`, `model_catalog.compose`, `trust.resolve`,
`permission_profile.catalog/resolve`, `feature_config.cache`.

---

# Verification

Adversarial re-run by a second agent, sandbox
`/private/tmp/claude-501/.../scratchpad/sandbox/verify-model-providers`
(`HOME=$D/home`, `XDG_CONFIG_HOME=$D/home/.config`, `XDG_DATA_HOME=$D/home/.local/share`,
`MUSE_NO_AUTO_UPDATE=1`, capture server `srv.py` on `127.0.0.1:8199` logging every request
head + body). Everything below is a re-run, not a re-read of the report.

**Verdict: MOSTLY_SOLID.** The provider set, the wire shape, the empty bundled catalog, the
catalog fetch/validation, the effort ladder incl. the `ultra→xhigh` clamp, `auth.json`,
the OIDC/mint constants, `MUSE_CUSTOM_HEADERS`, `session/setModel`, the billing tiers and the
local-tracing oracle all reproduce exactly. Five findings are wrong, and several
"reconstructed" numbers are one-sample artefacts rather than properties of the build.

## V1. Refuted

### V1.1 `provider_retry` default is **10**, not 5 — and the cap is per model step
```
settings: {"endpoint_transport":{...8199...},"model":"M","provider_retry":{"base_delay_ms":5}}
$ muse exec --approval-mode never --no-session-log "what is the capital of France"
muse: retrying meta model stream in 5ms (attempt 2/10)
… attempt 3/10 … 4/10 … 5/10 … 6/10 … 7/10 … 8/10 … 9/10 … attempt 10/10
```
With no `provider_retry` at all the backoff ladder is `1000, 2000, 4000, 8000, 16000, 32000,
60000, 60000, 60000 ms` — i.e. base 1000 ms, doubling, capped at 60 000 ms — and the counter
still reads `/10`. `{"max_retries":3}` → `attempt 2/4 … 4/4`, so `max_attempts = max_retries+1`
and the shipped default is `max_attempts=10` (`max_retries=9`).

The report's "exactly 5 POSTs / exactly 2 POSTs" is also wrong about scope: the bound is
**per model step**, not per run. With `{"max_attempts":2,"base_delay_ms":10}` a single
`muse exec` emitted **18** `POST /responses` in 60 s (the run loop re-enters after each
exhausted step); the default run emitted **16** in 300 s. `--max-model-steps 1` did not stop it.

### V1.2 The namespace tool does not hold 20 functions, and never held the ones listed
Parsed from the captured body, same command shape as the report (`x-client-id: tbh:exec`):

| configuration | n | tools |
|---|---|---|
| default (untrusted workspace) | **13** | workflow, read_file, search, write_file, edit_file, read_memory, add_memory, edit_memory, web_search, bash, bash_input, read_skill, write_todos |
| `--trust-workspace` or `--yolo` | **19** | + subagent_spawn/status/send_message/wait/read_result/cancel |
| all `MUSE_EXPERIMENTAL_*` on + trusted | **21** | + web_fetch, monitor |
| `--preset native-basic` | 13 | same as default |

`cron_create`, `cron_delete`, `cron_list`, `get_goal`, `create_goal`, `update_goal`,
`report_progress` — 7 of the report's 20 — appeared in **none** of these, and are dropped even
when named explicitly:
```
settings.run.toolset = ["read_file","bash","cron_create","cron_delete","cron_list",
                        "get_goal","create_goal","update_goal","report_progress"]
→ tools = ['read_file','bash','read_skill','write_todos']
```
Those names exist only in the agent-profile *vocabulary* string run
(`…bash_inputmonitorcron_createcron_deletecron_listget_goalcreate_goalupdate_goalreport_progresssubagent_spawn…`)
and in the bundled `muse-core` plugin's `commands/loop.md`. The tool list is a function of
workspace trust + gates + `settings.run.toolset`; "20" is not a property of the build.
(`instructions` length 36 613 B does reproduce exactly; total body 100 978 B here vs the
report's 106 615 B, consistent with the different tool count.)

### V1.3 Enterprise defaults **do** pin `provider` and `model`
```
$ muse config validate --plane defaults --file <doc>
{"schema_version":1,"settings":{"provider":"meta"}}      -> valid: plane=defaults schema_version=1
{"schema_version":1,"settings":{"model":"m1"}}           -> valid: plane=defaults schema_version=1
{"schema_version":1,"settings":{"model_catalog":[]}}     -> enterprise_document_invalid: reason=unknown_member
```
and the enterprise key-path table in the binary reads
`…settings.notifications.condition` **`settings.provider`** **`settings.model`** `settings.reasoning_effort…`.
Only `model_catalog` is genuinely absent. (Activation is checked at validate time —
`execution.forbid_approval_bypass` returns `field_not_activated` — so "valid" here means the
field is both known *and* activated.)

### V1.4 The `model_egress` error codes in §9 do not reproduce
```
{"model_egress":{}}                                        -> valid: plane=policy schema_version=1
{"model_egress":{"allowed_providers":{"allowed":["meta"]}}} -> semantic_invalid  location=model_egress.allowed_providers   (report said wrong_type)
{"model_egress":{"allowed_models":["m1"]}}                  -> semantic_invalid  location=model_egress.allowed_providers   (report said wrong_type @ allowed_models)
{"model_egress":{"allowed_providers":null}}                 -> null_not_allowed
{"model_egress":{"web_search":"off"}}                       -> wrong_type        location=model_egress.web_search
{"model_egress":{"web_fetch":{}}}                           -> field_not_activated location=model_egress.web_fetch
```
An empty `model_egress` block is **valid**, so "any `model_egress` block without a valid
`allowed_providers` fails" is false. Every non-null `allowed_providers` value I tried
(string, number, `[]`, `{}`, `{allowed:[…]}`, `{values:[…]}`, `{allowed,fallback}` with
fallback ∈ {"meta","deny","allow","none",object,true}) returns the same `semantic_invalid` —
the field is recognised (null is a distinct error) but not satisfiable in this build.
Any `allowed_models` also reports at `allowed_providers`, i.e. it requires it.

### V1.5 The "sanctioned front door" inference is wrong
The report inferred the withholding "protects the minted/OAuth-derived bearer, not an
operator-supplied `META_API_KEY`". With a synthetic **OAuth slot** in `auth.json`
(`{"providers":{"meta":{"mechanism":"oauth","access_token":…,"api_key":"FAKE-MINTED-KEY",
"api_base_url":"https://api.meta.ai/v1",…}}}`), no `META_API_KEY`, and only
`--base-url http://127.0.0.1:8199`:
```
=== GET /muse-code/models
H: authorization: Bearer FAKE-MINTED-KEY
```
The minted key went to an arbitrary host. A stored plain `api_key` behaves the same. The
`Meta bearer withheld:` literal sits inside the **TUI action table**
(`…muse.provider_alias` `Meta bearer withheld: …` `navigation focus-plan expand-tool-output`
`FlushSessionLog … OpenBillingUrlDispatch ModelSelectionDispatch StartupMintRetryDispatch…`),
so it is a TUI-surface guard; I could not trigger it from `exec` with any credential kind.
Downgrade to: **the guard is not observable outside the TUI, and its stated scope is unverified.**

### V1.6 `MUSE_AUTH_URL` and `MUSE_CLIENT_ID` are launcher-only, like `MUSE_AUTH_PATH`
§7.4's table lists them as binary overrides. They are not in the binary:
```
grep -ac 'MUSE_AUTH_URL'  strings4.txt -> 0
grep -ac 'MUSE_CLIENT_ID' strings4.txt -> 0
grep -ac 'MUSE_AUTH_PATH' strings4.txt -> 0        (report got this one right)
grep -ac 'TBH_AUTH_BASE_URL' strings4.txt -> 4
```
The binary honours **only** `TBH_AUTH_BASE_URL`; it has no client-id override at all
(`1031625952748946` is a bare literal next to `struct MintedKey` / `muse-code/logout`).

### V1.7 `first_turn_minimal_effort_regex` needs no experimental gate
```
settings: {…,"first_turn_minimal_effort_regex":"quantum"}          # gate NOT set
$ muse exec --approval-mode never --no-session-log "explain quantum tunneling"
Hello! How can I help you today?          POSTs = 0
```
Same prompt with the regex removed → 5 POSTs, `reasoning.effort = "high"`. So the setting
alone short-circuits arbitrary prompts to a canned reply with **zero** model calls;
`MUSE_EXPERIMENTAL_FIRST_TURN_MINIMAL_EFFORT=1` changed nothing. (Note the setting is named
"minimal effort" but the observed effect is *no model call*, not a lowered tier.)

### V1.8 The canned-greeting table is a sample, not a mapping
`hi` → `Hi! How can I help you today?` here (report: `Hi! What would you like to work on?`);
`hello` → `Hello! How can I help you today?`; `HI` → `Hi — how can I help?`;
`hi!` → `Hello! How can I help you today?`. All 0 POSTs. `hey` and `what is 2+2` → real POSTs.
The fixture is chosen non-deterministically from the pool, so §4.4's prompt→text pairs must
not be read as a mapping. The pool itself checks out: 6 `msg_019fcfea…` ids, 8 `rs_019fcf…`
ids, the listed summaries, and `first_turn_canned_responsefirst_turn_canned_reasoning_item`.

### V1.9 The model-catalog cache is **not** seedable (ohmy hook #8 unverified)
`$XDG_DATA_HOME/muse/model-catalog` as a regular file → `model_catalog.cache_load
state="read_failed"` for *every* payload tried, including the exact 5-field
`ModelCatalogResponse` shape (`schema_version, provider_id, profile_id, source, rows`),
camelCase, bare array, wrapper objects, empty file and invalid JSON — the state never varies,
so the file is not being parsed. As a **directory** it reports `state="missing"` for all 22
candidate member names tried (`meta`, `tbh`, `catalog`, `cache`, `index`, `models`,
`default`, `current`, `latest`, each ± `.json`, `meta-tbh*`, `meta/tbh.json`, `meta/catalog.json`).
Leaving junk in it yields `state="invalid"`. Nothing I could write made a cached row reach
`model/list`. The invariant string (INV-007) and the diagnostics are real; the *hook* is not
demonstrated.

## V2. Corrections and sharpenings

* **`--api-key-stdin` is a credential source the report missed, and it outranks `META_API_KEY`.**
  `echo STDIN-KEY | META_API_KEY=ENVKEY muse exec --api-key-stdin …` → `H: authorization: Bearer STDIN-KEY`.
  Real order: `--api-key-stdin` > `META_API_KEY` > stored credential > OIDC login/mint.
* **Model precedence has a fourth layer: `--preset`.** `--preset` is *not* limited to the two
  built-ins. With `settings.presets.mypreset = {"provider":"meta","model":"preset-model"}`:
  `--preset mypreset` → wire `model=preset-model`; `+ MUSE_MODEL=env-model` → `env-model`;
  `+ --model flag-model` → `flag-model`; no preset → `settings.model`. So
  **`--model` > `MUSE_MODEL` > `--preset` > `settings.model`**, and `run_preset.resolve` gains a
  fourth source value the report's list omits:
  `outcome="ready" selected=true provider_source="preset" model_source="preset" agent_profile_source="preset"`.
* **`endpoint_transport.auth = "none"` really does suppress the bearer** (not just an enum
  variant): with `META_API_KEY=FAKE` and `auth:"none"` the captured `GET /muse-code/models`
  and `POST /responses` carry **no** `authorization` header at all.
* **`--base-url` beats `settings.endpoint_transport.base_url`.** Settings pinned to `:8199`,
  flag to `:8299` → 0 requests on 8199, 5 on 8299, and `Bearer FAKE` was still sent.
* **`/responses` path join has a query/fragment footgun the report missed.** Product paths
  always hit the origin root, as claimed, but the model path is resolved *relatively*:
  | `base_url` | `POST` path |
  |---|---|
  | `http://h:8199` or `…/` | `/responses` |
  | `…/v1` or `…/v1/` | `/v1/responses` |
  | `…/foo/bar` or `…/foo/bar/` | `/foo/bar/responses` |
  | `…/v1?q=1` **or** `…/v1#frag` | `/responses` ← `/v1` silently dropped |
  | `…/deep/path?q=1` | `/deep/responses` ← last segment dropped |
  A gateway URL carrying a query string silently loses its last path segment.
* **`parallel_tool_calls` does reach the wire** (report open question #7): it is omitted by
  default and present when the flag is given — `--parallel-tool-calls` → `"parallel_tool_calls":true`,
  `--no-parallel-tool-calls` → `false`, inserted between `store` and `tools`.
  `tool_choice` / `previous_response_id` stayed absent, as reported.
* **`MUSE_WWW_ROUTING` open question SOLVED: the only accepted value is `default_c1`.**
  ```
  MUSE_WWW_ROUTING=default_c1 → H: x-fb-routing-control: default_c1
  c1 / www / latest / prod / intern / canary / dev / tier / default / on / true / 1 /
  sandbox / beta / staging / master / default_c2 / latest_c1 / canary_c1 / intern_c1 /
  www_c1 / default_c1_test / DEFAULT_C1 → warning, header not sent
  ```
  So `default_c1` is a routing-control value, not (as §5.2 says) "the default client-id literal".
* **The Keychain claim is right, and provable directly.** `/usr/bin/sample` of the hung
  process (`TBH_CREDENTIAL_BACKEND` unset, plain `muse exec`):
  ```
  SecItemAdd (Security) → SecItemAdd_osx → SecKeychainItemCreateFromContent →
  StorageManager::defaultKeychainUI → makeLoginAuthUI → AuthorizationCopyRights →
  _AuthorizationCopyRights_send_message      [blocked, 1553/1553 samples]
  ```
  Two upgrades to the report: the evidence is a stack, not an inference; and it is
  `SecItemAdd` — a **write** — on an ordinary read-path `muse exec`, which is why a headless
  or fresh-`HOME` session blocks on a keychain-creation authorization prompt.
* **`settings.provider` fallback is not silent, and `echo` is settable.**
  `openai|anthropic|openrouter` print `tbh: settings provider X is unavailable in this build;
  falling back to meta`; `"provider":"echo"` starts a working echo session; any other value is
  a hard error (`unsupported provider …; expected echo or meta`), not a fallback.
* `ModelCost.currency` is a **required** (nullable) property in the exported schema, not optional.
* `struct ConfigCatalogRow with 11 elements` confirms §3.5's 11-field list; `struct
  ModelCatalogRow with 14 elements` confirms §3.4's; and a fifth type the report missed sits
  beside them: `struct ModelCatalogResponse with 5 elements`
  (`schema_version, provider_id, profile_id, source, rows`) — the internal/cache envelope.

## V3. Reproduced exactly (no change)

Providers (`--provider bogus` → ``expected `echo` or `meta` ``; three `provider X is not
compiled into this build` literals; `auth set --provider` `[possible values: meta]`;
`auth set --provider openai` → `unsupported provider openai`) · empty bundled catalog over MSP
(`{"providerId":"meta","profileId":"tbh","source":"bundledCatalog","models":[]}`, schema
fingerprint `sha256:03312c21…`) · no `Muse Spark 1.x` id anywhere · catalog envelope
validation (`{}`/`{"object":"list"}`/`{"data":5}` → `Provider returned malformed response
data.`; `{"data":[]}` → `model catalog has no visible models`; `{"data":[{"id":""}]}` →
`model catalog row has an empty model id`; unknown keys ignored) · `configCatalog` source with
the exact row echoed back, `visibility:"hidden"` suppressed · every effort tier's wire value
including `ultra → {"effort":"xhigh","summary":"auto"}` and the `none` rejection ·
`include:["reasoning.encrypted_content"]`, `store:false`, `max_output_tokens:128000`,
`prompt_cache_key:"tbh:main:<session uuid>"`, `x-client-id`/`x-tbh-session-id`/
`x-meta-ai-gateway-session-id`/`traceparent`/`accept: text/event-stream` ·
`endpoint_transport` validation (all four messages, verbatim) · `MUSE_CUSTOM_HEADERS`
newline-vs-comma, `notaheader` warning, `authorization` injection dropped ·
`auth.json` content + `-rw-------` + `.auth.json.lock` · all OIDC/mint literals ·
`session/setModel` → `unsupported_route` for openai/anthropic/openrouter/echo/meta/muse,
`invalid_target` with no providerId, `invalid_model` with empty modelId;
`session/start` → `providerId:"muse", modelId:null` · billing tiers (`Pay-as-you-go` /
`Subscription`, `paygclino_paygxgrade`, `Contributor` = Apache-2.0 text only) · the whole
per-provider option-key run · the local-tracing bootstrap log and every event named ·
`--echo-delay-ms` root-only · `muse logout` · §3.7's open question reproduced verbatim
(`ignored_hidden_rows=1` for a provider row even with a **fully populated** 13-field
`TbhMetadata` including `roles`, `limit`, `cost`, `modalities`, `options`).

## V4. Ground this dimension still misses

1. **`--api-key-stdin`** as a credential layer (see V2) — absent from §7.1 and §11.
2. **`settings.presets`** as a live provider/model selector and the `preset` source value —
   §6 lists only `{none, settings, runtime}`.
3. **`settings.run.toolset`** — a working per-run tool filter that shapes the `tools`
   namespace object the report spends §5.3 on.
4. **`endpoint_transport.auth:"none"`** as a bearer kill-switch (only listed as an enum).
5. **base_url query/fragment segment-eating** (V2) — a live misconfiguration hazard for the
   "point it at your own gateway" hook.
6. **`MUSE_WWW_ROUTING=default_c1`** (V2) — was listed as an open question.
7. **`--base-url` vs settings precedence** — untested in the report despite both being
   presented as the egress seam.
8. `/muse-code/config` is still never contacted, even with `settings.feature_config.enabled
   = true` and `TBH_DISABLE_FEATURE_CONFIG` unset (`feature_config.cache state="missing"
   gate_count=0`) — the report's open question stands, now with the "just enable it" answer
   ruled out.
9. The catalog-cache hook was never exercised end-to-end (V1.9); an `oh-my` design should not
   depend on it until someone captures a real `model_catalog.cache_write`.
