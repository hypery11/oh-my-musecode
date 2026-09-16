# Meta Muse Code — MSP (Muse Session Protocol) wire protocol and the `serve` / SDK surface

Target: `muse` 1.0.1 (`1.0.1-R2006.1`), build `e27e408b666e693900118f778bd6c2880f88e432`, arm64 macOS.
Binary under test: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`

Every claim below is either (a) reproduced by a command in this document, or (b) an exact string
recovered from the binary. Anything I could not prove is marked **INFERRED** or **UNKNOWN**.

## Artifacts written by this investigation

| Path | What |
|---|---|
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/msp/msp.schema.json` | Full JSON Schema bundle (190,535 bytes, 185 `$defs`) |
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/msp/msp.d.ts` | Full TypeScript declarations (103,265 bytes, 1,892 lines) |
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/msp/manifest.stable.json` | Stable-surface export manifest |
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/msp/manifest.experimental.json` | Experimental-surface export manifest |
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/msp/drive.py` | Reusable ~50-line Python MSP stdio client (handshake + UUIDv7 minting) |
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/msp/t*.py`, `t*.log` | Individual probe scripts and their raw transcripts |
| `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/msp/{json,ts}-{stable,exp}/` | The four raw exports as emitted by `muse schema` |

Sandbox conventions used everywhere below: `HOME` and `XDG_CONFIG_HOME` were redirected into
`…/scratchpad/sandbox/msp/fakehome`, `cwd` was `…/scratchpad/sandbox/msp/ws`, and
`MUSE_NO_AUTO_UPDATE=1` was always set. No credentials were ever supplied and no request ever
reached a Meta endpoint.

---

## 1. Executive summary

**MSP is JSON-RPC 2.0 over newline-delimited stdio, one connection per host process, with a
CQRS/event-sourced core.** It is not MCP-shaped and it is not Claude Code's SDK-shaped. The mental
model is closer to *"a durable, replayable, multi-writer-safe session database with a subscription
protocol bolted on"*:

- **Commands** (`session/*`, `turn/*`, `approval/*`, `userInput/*`, `subagent/*`) carry a
  client-minted **UUIDv7 `commandId`** and are answered with an **admission ack only**
  (`{"commandId", "status":"accepted"}`). The ack is never the outcome.
- **Outcomes** arrive on a separate **view stream** of notifications, each stamped with an opaque
  monotonic `viewCursor` and a `sourceRange` provenance token pointing at the raw durable records
  it folded from.
- **Replay/idempotency is first class**: re-sending the same `commandId` returns the same result
  byte-for-byte, and every command's intake is durably logged *before* the ack.
- Everything a client renders is a **fold** of an append-only `session.jsonl`. `view/page`,
  `session/read`, `session/resume` and the snapshot rung are all different projections of the same
  log.

There are exactly **31 client→server methods**, **23 server→client notifications**, and a
**29-row error registry**. The wire envelope schema version is **`1`**, and the stable surface
carries a content fingerprint,
`sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7`, which the server
echoes in `initialize` so a client can detect drift.

The single most important limitation for anyone building on this: **`muse serve` cannot select the
provider on the wire.** There is no `--provider` flag on `serve`, and `session/start.providerId` is
recorded as metadata but does not route the model. A `serve` host therefore always attempts the
real Meta provider, and without credentials every turn dies with
`turn/completed { terminal: "failed", error.kind: "modelError" }`. The offline-testable surface is
`session/userShell` (which needs no model) plus the whole lifecycle/approval/view plane.

---

## 2. The `muse schema` exporter

### 2.1 Help text (verbatim)

```
$ MUSE_NO_AUTO_UPDATE=1 muse schema --help
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
  -h, --help          Print this help
```

Error strings for the argument parser (from `strings.txt`): `--out DIR is required`,
`--out needs a DIR`.

### 2.2 Running all four exports

```
$ muse schema generate-json-schema --out $S/json-stable
wrote manifest.json, msp.schema.json to …/json-stable (stable surface)
$ muse schema generate-ts          --out $S/ts-stable
wrote msp.d.ts to …/ts-stable (stable surface)
$ muse schema generate-json-schema --out $S/json-exp  --experimental
wrote manifest.json, msp.schema.json to …/json-exp (experimental surface)
$ muse schema generate-ts          --out $S/ts-exp    --experimental
wrote msp.d.ts to …/ts-exp (experimental surface)
```

Sizes: `msp.schema.json` 190,535 B; `msp.d.ts` 103,265 B (1,892 lines).

### 2.3 The manifests, and the schema version string

```json
// json-stable/manifest.json
{
  "experimental": false,
  "fingerprint": "sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7",
  "schemaVersion": 1
}
```
```json
// json-exp/manifest.json
{
  "experimental": true,
  "fingerprint": "sha256:577d717d09bf3aae6ad43c85d3c2d8e0c37bbde350897c94808626d2f362060c",
  "schemaVersion": 1
}
```

**Schema version string: `schemaVersion: 1`** — this is the *envelope* version, and the TS doc is
explicit that it is not the only version number in the system:

> `SchemaInfo` — "`version` is the wire **envelope** schema version — distinct from the
> session-view `schemaVersion`, the raw-log `schema_version`, and the 0.x/1.0 stability posture,
> which is not on the wire at all (INV-006a)." Constraint on `fingerprint`:
> `"pattern": "^sha256:[0-9a-f]{64}$"`.

So there are **four** independent version numbers in play:

| Version | Where | Observed value |
|---|---|---|
| MSP envelope `schema.version` | `initialize` result, export manifest | `1` |
| `ViewSnapshot.schemaVersion` | `history.snapshot.schemaVersion` | `1` |
| raw-log `schema_version` | every `session.jsonl` record | `1` |
| per-payload `payload_schema_version` | raw-log records | `1`, and `2` for `runtime.command_intake.received` |

### 2.4 A real finding: `--experimental` changes the fingerprint but not the bytes

```
$ diff json-stable/msp.schema.json json-exp/msp.schema.json && echo IDENTICAL
IDENTICAL
$ md5 json-stable/msp.schema.json json-exp/msp.schema.json
MD5 (json-stable/msp.schema.json) = 11f6adfad4e0e6b34b467445f113fb43
MD5 (json-exp/msp.schema.json)    = 11f6adfad4e0e6b34b467445f113fb43
$ diff ts-stable/msp.d.ts ts-exp/msp.d.ts && echo IDENTICAL
IDENTICAL
```

The two surfaces are **byte-identical** in this build, yet the manifests advertise different
fingerprints. Neither fingerprint is the SHA-256 of the emitted file
(`shasum -a 256 json-stable/msp.schema.json` = `f7c77710dbf1…`), so the fingerprint is computed over
the internal bundle model, not the rendered artifact. Practical reading: **in 1.0.1 the
experimental surface adds nothing to the published schema.** The experimental delta lives entirely
in fields/variants that the model deliberately refuses to publish — see §8.

Setting `MUSE_EXPERIMENTAL_SDK_ENABLED=1` does not change the export either:

```
$ MUSE_EXPERIMENTAL_SDK_ENABLED=1 muse schema generate-json-schema --out $S/json-exp-env --experimental
$ diff json-exp/msp.schema.json json-exp-env/msp.schema.json && echo "SCHEMA IDENTICAL under SDK gate"
SCHEMA IDENTICAL under SDK gate
```

### 2.5 Bundle shape

The JSON Schema bundle is **not** a plain draft-2020-12 document — it is a schema-plus-registry
bundle with four sibling sections next to `$defs`:

```
TOP KEYS: ['$defs', '$schema', 'capabilities', 'description', 'errors', 'methods', 'notifications', 'reserved']
$defs         -> 185 entries
$schema       = https://json-schema.org/draft/2020-12/schema
description   = "Muse Session Protocol (MSP) v1 wire schema bundle, rendered from the tbh-protocol
                 schema model (spec 206). Generated; do not edit."
methods       -> 31 entries
notifications -> 23 entries
errors        -> 29 entries
capabilities  -> {grantable, reserved}
reserved      -> 3 entries
```

The TS header is:

```ts
// Muse Session Protocol (MSP) v1 TypeScript declarations.
// Generated by tbh-protocol (spec 206) from the same schema model as
// the JSON Schema bundle (INV-017). Do not edit by hand.
```

The generating crate is `tbh-protocol` (Rust path fragments in the binary confirm the module tree:
`tbh_protocol::method::lifecycle::PendingRequestKind`,
`tbh_protocol::method::lifecycle::SessionResumeResult`,
`tbh_protocol::method::lifecycle::SessionForkParams`).

The schema model has its own validator, whose error kinds leak into the binary:
`MissingOpenness`, `OpennessOnNonEnum`, `FieldsOnNonObject`, `VariantsOnNonEnum`,
`DependentRequiredUnknownMember`, `EmptyEnum`, `AliasTargetMismatch`, `DanglingRef`,
`UnknownRoot`, `NameMismatch`, plus `` `required` is not an array — refuse rather than default``.
Model-level extension keys `x-msp-openness`, `x-msp-minimum`, `x-msp-maximum` also appear;
only `x-msp-openness` survives into the rendered bundle.

### 2.6 `x-msp-openness` — the open/closed enum discipline

Every enum in the bundle is tagged `"x-msp-openness": "open" | "closed"`. This is the single most
important thing to internalise before writing a client. **Closed (11)** — a value outside the list
is a protocol violation and you may model it as a discriminated union:

```
ApprovalMode, CommandAckStatus, HistoryPreference, IfBusy, JsonRpcVersion,
PlatformFamily, PlatformOs, ReasoningEffort, TurnInputPartType,
UserInputSelectionMode, ViewPageDirection
```

**Open (34)** — the server may add values additively and your client must degrade gracefully:

```
ApprovalAmendmentDurability, ApprovalChoiceScope, ApprovalDecision, ApprovalModeApplyOutcome,
ApprovalModeSource, ApprovalPersistenceStatus, ApprovalPolicyResult, ApprovalResolvedBy,
BackgroundInitiator, CapabilityName, CommandStatus, CompactStatus, CompactionOutcome,
CompactionTrigger, ContextPressureLevel, ErrorKind, HistoryMode, HistoryNoneReason, ItemKind,
ItemStatus, ModelCatalogSource, ModelChangeSource, OutputRefAvailability, PendingRequestKind,
SessionDurability, SessionStatus, SubagentControlStatus, TodoStatus, TurnErrorKind,
TurnStartDisposition, TurnTerminal, UserInputOutcome, Vcs, ViewPageAnchor
```

In TS the openness is encoded as `"a" | "b" | (string & {})` for open enums and a plain union for
closed ones. The rationale is spelled out in the doc comments, e.g. for `ItemKind`:
*"Open: a new kind is additive evolution, and clients MUST render unknown kinds generically
(tdd SS4.10)"*; and for `ApprovalMode`: *"**Closed** by design (D-006, select-never-create): a
client selects a preconfigured mode and can never construct one."*

---

## 3. Transport, framing and the handshake

### 3.1 Transport

`muse serve --help`, verbatim:

```
muse serve — serve an MSP session host over stdio

Usage: muse serve [OPTIONS]

The client owns this process's stdin and stdout and is its only
connection. Sandbox posture and session durability are constructed
here and apply to every session the host loads; neither is negotiable
over the wire. Approval mode is the other way round — it is selected
on the wire, so there is no approval flag here.

Options:
  -h, --help
          Print this help
      --no-session-log
          Use memory-only sessions

Sandbox posture (fixed for the host's lifetime):
      --disable-sandbox
          Disable shell filesystem/network sandboxing for this host
      --sandbox-network <MODE>
          Sandbox network mode (default: proxy-only)
      --disable-write
          Disable non-shell workspace filesystem writes
      --disable-shell
          Disable workspace shell execution
      --trust-workspace
          Load each session workspace's skills and rules
```

**Only stdio exists in v1.** `--listen` is a *recognised and deliberately refused* flag:

```
$ muse serve --listen
muse serve: --listen is not available in v1; the unix-socket and websocket transports are deferred post-v1 (#13929)
usage: muse serve [OPTIONS] (run `muse serve --help` for options)

$ muse serve --listen=unix:/tmp/x.sock      # same message
$ MUSE_EXPERIMENTAL_SDK_ENABLED=1 muse serve --listen   # same message — the SDK gate does not unlock it
$ muse serve --http | --websocket | --stdio | --port 8080
muse serve: unknown option --http           # etc.
$ muse serve extra-positional
muse serve: unexpected argument extra-positional; serve takes no positional arguments
```

So the transport roadmap is on the record: **unix socket and websocket are planned, deferred
post-v1 under issue #13929.**

Failure strings for the transport layer: `MSP stdio connection failed: `,
`serve: MSP stdio connection failed before a degraded drain: `, `build MSP serve runtime: `,
`compose product startup for serve: `, `resolve serve paths: `, `resolve the MSP sessions root: `.

### 3.2 Framing

Newline-delimited JSON, one frame per line, JSON-RPC 2.0. **Batches are rejected**:

```
>>> [{"jsonrpc":"2.0","id":1,"method":"session/list","params":{"limit":1}}]
<<< {"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"frame is not a JSON-RPC object","data":{"kind":"invalidRequest"}}}
```

Malformed JSON produces the canonical `id: null` parse error:

```
>>> {"jsonrpc":"2.0","id":99,"method":"session/list"          <- truncated on purpose
<<< {"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse error","data":{"kind":"parseError"}}}
```

`jsonrpc` is enforced strictly (missing *or* wrong value is the same error, and the id is echoed
because it was recoverable):

```
>>> {"jsonrpc":"1.0","id":5,"method":"session/list","params":{}}
<<< {"jsonrpc":"2.0","id":5,"error":{"code":-32600,"message":"missing or invalid jsonrpc member","data":{"kind":"invalidRequest"}}}
>>> {"id":6,"method":"session/list","params":{}}
<<< {"jsonrpc":"2.0","id":6,"error":{"code":-32600,"message":"missing or invalid jsonrpc member","data":{"kind":"invalidRequest"}}}
```

Frame types in the schema — `Request`, `Notification`, `SuccessResponse`, `ErrorResponse`:

```jsonc
Request        { id: RequestId, jsonrpc: "2.0", method: string, params?: object, trace?: TraceContext }
Notification   { jsonrpc: "2.0", method: string, params?: object, emittedAtMs?: integer }
SuccessResponse{ id: RequestId, jsonrpc: "2.0", result: object }   // always an object, never a scalar
ErrorResponse  { id: RequestId|null, jsonrpc: "2.0", error: ErrorObject }
```

Two details worth noting:

- `SuccessResponse.result` — *"Always a JSON object, possibly `{}`, never a bare scalar — so every
  result can grow additive-optional members (SS1.2)."* Verified: `view/unsubscribe` returns `{}`.
- `Notification.emittedAtMs` — *"Emission time in Unix milliseconds, recorded once at emission;
  server→client notifications only (SS1.2.5)."* Verified present on every server notification I
  observed, e.g. `"emittedAtMs":1788273093176`.
- `Request.trace` — optional W3C trace context (`traceparent` / `tracestate`), *"on requests in both
  directions only — never on responses or notifications (SS1.8)."*

### 3.3 Request ids — a documented rule the server does not enforce

`RequestId` doc string, recovered verbatim from the binary:

> `Integer` — "Integer form; server-initiated requests always use this form, from a monotonically
> increasing per-process counter (SS1.3)."
> `String` — "String form (client-initiated only)."
> "A request id (SS1.3): client-chosen string or integer. `1` and `"1"` do not compare equal; each
> direction owns its own id space."

The intent is a clean split: **server→client requests use integer ids, client→server requests use
string ids.** In practice the host accepted my integer client ids throughout this whole
investigation without complaint. **INFERRED**: the split is advisory/convention in 1.0.1, not
enforced. Do not rely on it to demultiplex.

### 3.4 The handshake is two steps and both are mandatory

```
>>> {"jsonrpc":"2.0","method":"session/list","id":1,"params":{"limit":5}}
<<< {"jsonrpc":"2.0","id":1,"error":{"code":-32600,"message":"Not initialized","data":{"kind":"notInitialized"}}}

>>> {"jsonrpc":"2.0","method":"initialize","id":2,"params":{"clientInfo":{"name":"omm_probe","title":"OMM Probe","version":"0.0.1"},"capabilities":{"experimentalApi":false,"requestedCapabilities":["userShell","rawLog","bogus"]}}}
<<< {"jsonrpc":"2.0","id":2,"result":{
      "serverInfo":{"name":"muse","version":"1.0.1"},
      "userAgent":"muse-build/1.0.1 (non-interactive; macos-aarch64; build e27e408b666e693900118f778bd6c2880f88e432)",
      "museHome":"…/fakehome/.local/share/muse",
      "platformFamily":"unix","platformOs":"macos",
      "schema":{"version":1,"fingerprint":"sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7"},
      "grantedCapabilities":["userShell"],
      "experimentalApi":false,
      "sessionDurability":"durable"}}

>>> {"jsonrpc":"2.0","method":"initialized"}      // notification, no id, no params
```

Skipping the `initialized` notification leaves the connection unusable — proven directly:

```
>>> initialize                       -> 200 OK
>>> {"jsonrpc":"2.0","method":"session/list","id":2,"params":{"limit":1}}
<<< {"jsonrpc":"2.0","id":2,"error":{"code":-32600,"message":"Not initialized","data":{"kind":"notInitialized"}}}
>>> {"jsonrpc":"2.0","method":"initialized"}
>>> {"jsonrpc":"2.0","method":"session/list","id":3,"params":{"limit":1}}
<<< {"jsonrpc":"2.0","id":3,"result":{"sessions":[…]}}
```

Re-initialising is refused:

```
<<< {"jsonrpc":"2.0","id":3,"error":{"code":-32600,"message":"already initialized","data":{"kind":"alreadyInitialized"}}}
```

`clientInfo.name` is validated against a hard pattern (`^[a-z0-9_]+$`, "machine identifier"):

```
>>> initialize {"clientInfo":{"name":"Omm-Probe!","version":"0.0.1"}}
<<< {"code":-32602,"message":"invalid initialize params: clientInfo.name must be a machine identifier matching ^[a-z0-9_]+$ (SS1.4.1)","data":{"kind":"invalidParams"}}
```

**Auditability note that matters for any control plane:** your `clientInfo` is written to the
durable session log. From `…/sessions/2026/09/01/<sid>/session.jsonl`:

```json
{"payload_type": "session.opened.observed", "payload": {"kind":"session_opened","record":{
  "schema_version":1,"session_id":"01a05d5d-e9e7-7b62-852e-9ccba4234d25","resume":false,
  "security_mode":"normal","credential_backend":"file","keychain_fallback_reason":"none",
  "client_info_name":"omm_probe","client_info_version":"0.0.1"}}}
```

The binary's telemetry docs name this: `session.opened.observed.client_info_name (#23124, MSP serve
open boundary)` with a normalisation rule `[A-Za-z0-9…]` and *"empty wire values — absent, never
\"\""*. The same telemetry block also defines
`command_admission.refusals` = *"process-local MSP -32031 refusal count, keyed by targeted loaded
session (#207)"*.

### 3.5 Capabilities

```jsonc
"capabilities": {
  "grantable": ["userShell"],
  "reserved":  [{ "name": "rawLog", "reference": "#13929" }]
}
```

Exactly **one grantable capability in v1: `userShell`**, and one reserved name, `rawLog`, held for
the deferred SS6 raw-log altitude. Requesting unknown names is silently ignored — I asked for
`["userShell","rawLog","bogus"]` and got back `"grantedCapabilities":["userShell"]`.
`InitializeResult.grantedCapabilities` is *"Fixed for the connection lifetime (INV-009)."*

Ungranted use is a clean typed error:

```
>>> session/userShell (connection did NOT request userShell)
<<< {"code":-32010,"message":"session/userShell requires the userShell capability",
     "data":{"kind":"capabilityRequired","retryable":false,"capability":"userShell"}}
```

### 3.6 Notification opt-out, and the protected set (SS1.7)

`ClientCapabilities.optOutNotificationMethods` — *"Exact notification method names the client
declines; no wildcards; unknown names accepted and ignored; the protected set (SS1.7) cannot be
opted out."*

I opted out of **all 23 documented notifications plus `session/started` and `session/closed`**,
then drove a `session/userShell` through a full approval cycle. The transcript shows exactly one
notification survived:

```
(no approval/requested delivered -> recovered the approval via approval/listPending instead)
POST-DECIDE NOTIF: approval/resolved
```

**Proven: `approval/resolved` is in the protected set and cannot be suppressed.** The schema
descriptions name the same property for two others — `approval/resolved` *"protected delivery
(SS5.5)"*, `userInput/settled` *"protected delivery (SS5.10.3)"* — and `view/gap` *"rides the
protected reserved queue"*. **INFERRED**: the protected set is
`{approval/resolved, userInput/settled, view/gap}`; I only proved the first member because the
other two could not be triggered without a live model.

A useful side-proof: `session/started` (see §5.1) *was* suppressed by the opt-out list, which means
it is a genuinely registered notification method routed through the same filter, not stray output.

---

## 4. The full method index (31)

`MspMethod` from `msp.d.ts` is the normative list. Every one of these was confirmed present at
runtime (I probed 17 plausible non-members and every one returned `-32601 methodNotFound`).

| Method | Params | Result | One-line contract (from the bundle) |
|---|---|---|---|
| `initialize` | `InitializeParams` | `InitializeResult` | Opens the connection: client id, capability requests, experimental opt-in; reply carries envelope schema version + stable-surface fingerprint (SS1.4.1) |
| `session/start` | `SessionStartParams` | `SessionStartResult` | Creates a brand-new session, loads it, durably records the start, auto-subscribes this connection (SS2.5.1) |
| `session/resume` | `SessionResumeParams` | `SessionResumeResult` | Loads a stored session on this host, auto-subscribes, returns history needed to render it (SS2.5.2) |
| `session/fork` | `SessionForkParams` | `SessionForkResult` | Branches a session into a new id whose log copies the source through a cut point, with durable provenance (SS2.5.3) |
| `session/list` | `SessionListParams` | `SessionListResult` | Pages stored sessions; read-only, never touches leases (SS2.5.4) |
| `session/read` | `SessionReadParams` | `SessionReadResult` | Reads one stored session **without attaching**: no lease, no load, no subscription, no resume record (SS2.5.5) |
| `session/compact` | `SessionCompactParams` | `SessionCompactResult` | Compacts conversation context; async, ack is admission only (SS3.7). The only method that may answer `"noop"` |
| `session/setModel` | `SessionSetModelParams` | `SessionSetModelResult` | Reconfigures the session's model; durable, applies to subsequent model calls (SS3.8) |
| `session/setApprovalMode` | `SessionSetApprovalModeParams` | `SessionSetApprovalModeResult` | Selects a **preconfigured** approval mode mid-session; select, never create (SS5.12) |
| `session/userShell` | `SessionUserShellParams` | `SessionUserShellResult` | User-initiated shell in the session workspace; capability-gated on `userShell` (SS3.9) — the TUI's `!` escape hatch |
| `turn/start` | `TurnStartParams` | `TurnStartResult` | Submits user input; optional `ifBusy` disposition controls queue/steer/replace (SS3.2) |
| `turn/steer` | `TurnSteerParams` | `TurnSteerResult` | Injects input into the turn the caller names; fails if that turn is no longer active (SS3.3) |
| `turn/interrupt` | `TurnInterruptParams` | `TurnInterruptResult` | Stops the running turn on the **priority lane**, optionally pairing a durable retract intent (SS3.4) |
| `turn/cancel` | `TurnCancelParams` | `TurnCancelResult` | Cancels a turn on the **normal command lane**, recording no priority interrupt intent (SS3.5) |
| `turn/unqueue` | `TurnUnqueueParams` | `TurnUnqueueResult` | Reclaims a queued submit before it launches; never upgrades into a cancel (SS3.6) |
| `model/list` | `ModelListParams` | `ModelListResult` | Reads the catalog the host will accept in `session/setModel`; **a query, not a command** (SS3.10) |
| `view/page` | `ViewPageParams` | `ViewPageResult` | Cursor-paged reads of the session view, forward/backward; each element is an unframed view notification (SS4.7.3) |
| `view/unsubscribe` | `ViewUnsubscribeParams` | `ViewUnsubscribeResult` | Removes this connection from the subscription set; does not unload the session (SS4.7.2) |
| `approval/decide` | `ApprovalDecideParams` | `ApprovalDecideResult` | Decides a pending approval, guarded by `requirementId` against the multi-stage race (SS5.4) |
| `approval/listPending` | `ApprovalListPendingParams` | `ApprovalListPendingResult` | Pull dual of the re-issued requests; a lease-free fold read (SS5.7) |
| `userInput/answer` | `UserInputAnswerParams` | `UserInputAnswerResult` | Answers every question of an open prompt (SS5.10.2) |
| `userInput/cancel` | `UserInputCancelParams` | `UserInputCancelResult` | Declines a prompt; the tool call resolves cancelled and the model sees it (SS5.10.2) |
| `userInput/clarify` | `UserInputClarifyParams` | `UserInputClarifyResult` | Free-form clarification the model re-decides against (SS5.10.2) |
| `subagent/sendMessage` | `SubagentInputParams` | `CommandAcceptedResult` | Queue a user note into a running child (SS3.16) |
| `subagent/followupTask` | `SubagentInputParams` | `CommandAcceptedResult` | Queue a follow-up task for a child (SS3.16) |
| `subagent/interrupt` | `SubagentOwnerReasonParams` | `CommandAcceptedResult` | Ask a child to yield at its next boundary (SS3.16, SS4.5.7) |
| `subagent/stop` | `SubagentOwnerReasonParams` | `CommandAcceptedResult` | Stop a running child (SS3.16, SS4.5.7) |
| `subagent/close` | `SubagentOwnerReasonParams` | `CommandAcceptedResult` | Owner-close a child; terminal mapping folds per SS4.5.7 (SS3.16) |
| `subagent/resume` | `SubagentTargetParams` | `CommandAcceptedResult` | Resume a paused/recoverable child as a durable later attempt (SS3.16) |
| `subagent/reopen` | `SubagentTargetParams` | `CommandAcceptedResult` | Reopen a stopped/closed child as a durable later attempt (SS3.16) |
| `subagent/readResult` | `SubagentTargetParams` | `CommandAcceptedResult` | Consume a ready child result (state-changing) (SS3.16) |

### 4.1 The command discipline (SS3.1)

Every method above except `initialize`, `session/list`, `session/read`, `model/list`,
`approval/listPending`, `view/page` and `view/unsubscribe` is a **command**, which means:

1. **`commandId` is required and must be a client-minted UUIDv7.** Proven — a UUIDv4 is rejected:
   ```
   <<< {"code":-32602,"message":"invalid session/start commandId: expected UUIDv7","data":{"kind":"invalidParams"}}
   ```
   *"Required; the server never mints one."*
2. **The ack is admission only.** Shape is uniform (`CommandAcceptedResult`):
   `{"commandId":"…","status":"accepted"}`, with each method adding its own identity field
   (`turnId`, `approvalId`, `userInputId`, `effectiveMode`, …).
   `CommandStatus` is the open `"accepted"` enum; `CompactStatus` adds `"noop"`.
3. **Durable intake before ack**, and **value-identical replay**. Proven — replaying a
   `session/start` with the same `commandId` returned the byte-identical result including the same
   minted `sessionId`:
   ```
   >>> session/start commandId=01a05d5d-e9e7-79fa-886a-af8153b62c45   -> sessionId 01a05d5d-e9e7-7b62-852e-9ccba4234d25
   >>> session/start SAME commandId                                    -> sessionId 01a05d5d-e9e7-7b62-852e-9ccba4234d25 (identical result object)
   ```
   The raw log shows the paired records `runtime.command_intake.received` (payload_schema_version 2)
   and `runtime.command_intake.settled` for every command.
4. **`turnId` derives from `commandId`.** *"the fresh turn's `turnId` derives from it (tdd SS3.1.4)"*.
   Confirmed in every transcript: `turn/start` acked
   `{"commandId":"01a05d5e-3fc4-…","turnId":"01a05d5e-3fc4-…"}` — identical strings.
5. **Command-id conflict is not its own code.** The reserved list says
   `-32012` is *"deliberately never assigned; command_id_conflict is commandRejected"*, and the
   binary carries the reason string `command_id_conflict` alongside `runtime_busy`.

### 4.2 Command admission capacity is 4

Flooding 200 `session/userShell` commands at one host:

```
FLOOD result codes: {-32031: 390, -32001: 6, 'ok': 1, -32030: 3}
SAMPLE {"code": -32031, "message": "host command admission capacity 4 is exhausted",
        "data": {"kind":"backpressured","retryable":true,"capacity":4}}
SAMPLE {"code": -32001, "message": "Server overloaded; retry later.",
        "data": {"kind":"overloaded","retryable":true}}
SAMPLE {"code": -32030, "message": "session/userShell command 01a05d68-… rejected: runtime_busy",
        "data": {"kind":"commandRejected","retryable":false,"commandId":"01a05d68-…","reason":"runtime_busy"}}
```

**The MSP host admits at most 4 in-flight commands.** Anything above that is `-32031 backpressured`
with `data.capacity` naming the bound. A driver must implement a semaphore of 4 plus
retry-with-backoff on `-32031`/`-32001`. This is the single hardest operational constraint in the
protocol and it is not mentioned in `--help`.

---

## 5. The full notification index (23 + 2 undocumented)

| Notification | Params | Contract |
|---|---|---|
| `initialized` | *(none)* | Client→server, closes the handshake (SS1.4.2) |
| `turn/started` | `TurnStartedParams` | A foreground turn began running (SS4.5.1) |
| `turn/completed` | `TurnCompletedParams` | Turn terminal: `completed \| failed \| cancelled`, with usage and settled error (SS4.5.1) |
| `turn/retracted` | `TurnRetractedParams` | An interrupt-paired retract was durably accepted (SS4.5.1) |
| `turn/retryScheduled` | `TurnRetryScheduledParams` | A failing attempt's retry is scheduled and waiting out backoff; non-terminal (D-026) |
| `turn/unqueued` | `TurnUnqueuedParams` | A queued submit's reclaim won; its pre-minted turn never runs (SS3.6) |
| `item/started` | `ItemStartedParams` | An item opened on the transcript (SS4.3) |
| `item/updated` | `ItemUpdatedParams` | Open item changed non-terminally: full re-emission at higher `revision` (SS4.4.2) |
| `item/delta` | `ItemDeltaParams` | Streaming append to an open item's field path; **no `sourceRange`**; opt-out-able (SS4.3.1) |
| `item/completed` | `ItemCompletedParams` | Item reached terminal state: the authoritative final object (SS4.3) |
| `view/gap` | `ViewGapParams` | Push delivery dropped events; `(after, next)` brackets the hole (SS4.8, FM-001) |
| `approval/requested` | `ApprovalRequestParams` | An approval opened (SS5.5.1) |
| `approval/updated` | `ApprovalUpdatedParams` | A non-terminal approval record changed the pending view (SS5.5.1) |
| `approval/resolved` | `ApprovalResolvedParams` | First durable approval terminal landed; **protected delivery** (SS5.5) |
| `userInput/requested` | `UserInputRequestParams` | A user-input prompt opened (SS5.10.1) |
| `userInput/settled` | `UserInputSettledParams` | First durable settlement; **protected delivery** (SS5.10.3) |
| `session/modelChanged` | `SessionModelChangedParams` | A durable model selection took effect (SS4.6.1) |
| `session/goalChanged` | `SessionGoalChangedParams` | Goal block changed; replace wholesale, explicit `null` clears (SS4.6.2) |
| `session/todoListChanged` | `SessionTodoListChangedParams` | Todo list replaced wholesale; empty `items` is a cleared list (SS4.6.3) |
| `session/branchChanged` | `SessionBranchChangedParams` | Durable workspace-branch observation; `null` branch is a detached-HEAD fact (SS4.6.4) |
| `session/tokenUsage` | `SessionTokenUsageParams` | A model call completed with usage: raw + counted-once + session cumulative (SS4.6.5) |
| `session/contextUsage` | `SessionContextUsageParams` | Provider-reported context occupancy folded to a changed `(windowTokens, usedTokens, pressure)` triple (SS4.6.6) |
| `session/approvalModeChanged` | `SessionApprovalModeChangedParams` | Approval mode changed: the fold of the durable reconfigure audit fact (SS5.12) |

### 5.1 Two notifications the schema does not publish

The published `MspNotification` union omits two method names that the host actually emits and that
the opt-out filter actually knows about.

**`session/started`** — observed on every `session/start` *and* `session/fork`, delivered *before*
the request's own response:

```
>>> {"jsonrpc":"2.0","method":"session/start","id":2,"params":{…}}
<<< {"jsonrpc":"2.0","method":"session/started","params":{"session":{
      "sessionId":"01a05d5d-e9e7-7b62-852e-9ccba4234d25",
      "path":"…/sessions/2026/09/01/01a05d5d-e9e7-7b62-852e-9ccba4234d25/session.jsonl",
      "status":"idle","activeTurnId":null,
      "createdAt":"2026-09-01T14:27:10.959597Z","updatedAt":"2026-09-01T14:27:10.9596Z",
      "workspaceRoot":"…/ws","providerId":"echo","modelId":null,
      "turnCount":0,"forkedFrom":null}},"emittedAtMs":1788272831005}
<<< {"jsonrpc":"2.0","id":2,"result":{"session":{…same…},"viewCursor":""}}
```

Its params are a bare `{ "session": Session }` — no `viewCursor`, no `sourceRange`. It is a
**connection-level** notification, not a view event. It is suppressible via
`optOutNotificationMethods` (proven in §3.6), which is what makes it a registered method rather
than stray output.

**`session/closed`** — present in the binary's notification-name literal block immediately after
the published 23 (`…session/approvalModeChanged` `viewCursor` `session/closed`). I never triggered
it. **UNKNOWN**: exact params and trigger. **INFERRED**: the dual of `session/started`, emitted
when the host unloads a session.

A schema-validating client that rejects unknown notification methods **will break on
`session/started` on its very first `session/start`.** This is the concrete practical hazard of the
published bundle.

### 5.2 The view stream contract

Every *view* notification (as opposed to the connection-level `session/started`) carries three base
members, formalised as `UnframedViewNotificationParams`:

```ts
{ sessionId: string, viewCursor: string, sourceRange: SourceRange }
```

Observed `viewCursor` format: **`v:<sessionId>:<n>`** with `n` a 1-based monotone integer, e.g.
`"v:01a05d61-0ac9-7d21-95c0-5094f168257b:4"`. The schema insists it is opaque
(*"Opaque, strictly monotonic view cursor (tdd SS4.1)"*, *"clients relay it, never parse it"*), and
you should honour that — but the structure is useful for debugging. Two non-`v:` cursor values I
observed: the empty string `""` (fold head with no events yet), and the sentinel
`"pending:seam-c-session-view-fold"` under `--no-session-log` (an internal seam name that leaked
onto the wire; also present as a literal in the binary).

`SourceRange` is the provenance token:

```json
"sourceRange": {
  "stream": {"kind":"session","id":"01a05d61-0ac9-7d21-95c0-5094f168257b"},
  "first":  {"id":"bf8602d7-be6f-4319-8511-4e6999e7b589","sequence":8},
  "last":   {"id":"bf8602d7-be6f-4319-8511-4e6999e7b589","sequence":8}
}
```

Its doc is the clearest statement of the whole architecture:

> "The inclusive raw-record range a durable-sourced view event folded from (tdd SS4.2) — the
> reconciliation token: re-folding the cited records from the canonical fold state immediately
> before `first` … reproduces the event. In v1 it is an **opaque provenance token**: no wire method
> reads what it points at (the SS6 raw-log altitude is deferred, #13929). Ephemeral-sourced events
> (`item/delta`, and an `item/started` that opens on an ephemeral record) carry no `sourceRange`."

`stream.kind` values observed: `"session"`. The exec JSONL stream also shows `"run"` and `"task"`.

**Delivery-gap recovery (`view/gap`).** The doc gives two sanctioned strategies (D-030, FR-013) and
says the server holds no partial-fill state: (1) *splice-fill* — buffer live events at cursors
`>= next`, page `(after, next)` forward, discard the overlap, splice; or (2) *re-anchor* through the
anchored read surface. Coalescing rule: a run of consecutive undelivered events collapses into one
bracket keeping the **first** range's `after` (D-16487-1); the bracket flushes at the next
*accepted* delivery, not at enqueue (D-16487-2).

---

## 6. The nine item kinds and the transcript model

`ItemKind` (open): `userMessage`, `agentMessage`, `reasoning`, `toolCall`, `userShell`, `subagent`,
`workflow`, `reminderChild`, `compaction`.

`Item` is a **single flat struct with ~60 optional fields**, each documented with the kind that owns
it. That is unusual and deliberate — it is what makes snapshot+suffix "a pure splice", because the
notification item object and the snapshot item object are the same schema.

Universal members: `itemId` (bare UUIDv7), `kind`, `status`, `revision` (integer ≥1, strictly
monotonic per item, **apply rule: replace-iff-higher**; `item/delta` never bumps it),
`turnId` (`null` only for `userShell`, the one kind outside a turn), `recordedAt`, and the optional
`fallbackText` (*"Server-provided one-line human summary any kind MAY carry, for generic rendering
of kinds a client does not recognize"*).

`ItemStatus` (open): `inProgress`, `completed`, `failed`, `cancelled`, `rejected`, `timedOut` —
with the rule **terminal = anything other than `inProgress`**, and unknown values are
"terminal-unknown, rendered generically".

Per-kind highlights recovered from the doc comments:

- **`userMessage`** — `text` (as submitted), `displayText` (presentation form, never model-visible),
  `commandId` (so multi-client UIs de-duplicate their local echo), `attachments` (image *metadata*
  only, base64 never echoed back), `steered: true` when injected mid-turn, `retracted: true` after
  an accepted retract.
- **`agentMessage`** — `text`, streamed via `item/delta` on field `"text"`.
- **`reasoning`** — `summary: string[]`, one entry per part; part *n* streams via `item/delta` field
  `"summary.n"` (part boundary = index change). `providerItemId` (e.g. `rs_…`). Raw `text` is
  *never streamed in v1*.
- **`toolCall`** — `tool`, `callId` (`call_…`), `args` (*"the model-authored argument JSON,
  **verbatim**; clients parse"* — kept verbatim so the fold stays byte-deterministic and survives
  model-emitted almost-JSON), `approvalId` (join key to the approval events), `visibleOutput`
  (streams on field `"output"`), `outputRef`, `modelVisibleContent`, `background` +
  `backgroundInitiator`, `failureKind` (snake_case verbatim — the SS1.6 casing exemption),
  `failureReason`.
- **`userShell`** — `commandText`, `visibleOutput`, `exitCode`, `exitSignal` (*"the terminating
  signal NUMBER … nothing maps numbers to names, and the fold never invents one"*), `durationMs`.
- **`subagent`** — `subagentId`, `childSessionId` (*"readable via `session/read`/`view/page` — child
  transcript drill-down without a second protocol"*), `depth`, `objective`, `role`, `agentPath`,
  `controlStatus` (`SubagentControlStatus`), `result` (`SubagentResult`), `usage` (**transitive**:
  the child and its descendants; *never* folded into `session/tokenUsage.cumulative`).
- **`workflow`** — `workflowRunId`, `entryId`, `scriptId`, `resumeFromRunId`, `triggerSource`,
  `children: WorkflowChild[]` keyed by `(childId, attempt)`, re-emitted whole on every change.
- **`reminderChild`** — `reminderAgentId`, `generationId`, `taskId`, `childSessionLogPath`.
- **`compaction`** — `outcome`, `trigger`, `strategyId`, `tokensBefore`/`tokensAfter`, `reason`
  (snake_case verbatim, e.g. `"no_compactable_history"`), and `summarizedThrough` — an opaque
  compaction anchor that **is** accepted as a read anchor by `session/resume` and `view/page`
  (D-029, 2026-08-12).
- Any streamed surface may set `truncated: true` when the per-surface text budget saturates; the
  full text stays in the durable log.

`SubagentResult` shape: `{summary (≤512 chars), text? (≤32 KiB), structuredData?, artifactRefs[],
evidenceRefs[], errorKind?}`.

`OutputRef` shape: `{id, uri, kind, byteLen, availability, mediaType?, digest?, path?}` with
`availability ∈ {available, missing, unsupported, accessFailed}` and the fetch path named as
`item/readOutput` — **which does not exist in 1.0.1**:

```
>>> {"jsonrpc":"2.0","method":"item/readOutput","id":4,"params":{"sessionId":"x","itemId":"y"}}
<<< {"jsonrpc":"2.0","id":4,"error":{"code":-32601,"message":"method not found","data":{"kind":"methodNotFound"}}}
```

(Tested both with and without `experimentalApi: true`.) Its error machinery is nevertheless already
compiled in: `-32603` override `outputResultTooLarge`, `data.alignedNextOffset` for a misaligned
UTF-8 read, and the strings `offsetBytes does not start on a UTF-8 boundary`, `outputUnavailable`,
`stored output is unavailable`, `output result cannot fit the configured frame budget`,
`item or attached output ref was not found`. **So `outputRef` is a dangling pointer in 1.0.1:
you can see that bytes exist, and you cannot fetch them over MSP.**

---

## 7. Verified behaviour, method by method

Everything in this section is a live transcript from `muse serve`.

### 7.1 Session lifecycle

`session/start` — minimal call and result:

```
>>> {"method":"session/start","id":2,"params":{"commandId":"01a05d5f-f2eb-78dd-…","workspaceRoot":"…/ws"}}
<<< {"id":2,"result":{"session":{"sessionId":"01a05d5f-f2eb-7c40-…","path":"…/session.jsonl",
      "status":"idle","activeTurnId":null,"createdAt":"…","updatedAt":"…","workspaceRoot":"…/ws",
      "providerId":"muse","modelId":null,"turnCount":0,"forkedFrom":null},
      "viewCursor":"v:…:1"}}
```

Note `providerId` defaults to the string **`"muse"`**, not `"meta"`.

Optional params: `sessionId` (*"This field never selects an existing session: a retained or
reserved id is rejected `commandRejected` with reason `session_id_conflict`"*), `providerId`,
`modelId`, `approvalMode`, `workspaceRoot`, and `config: SessionConfig` — which is **an empty
struct with no members in v1**, reserved for a future Configuration section.

`session/start.approvalMode` is *"the only surface that declares a non-interactive run's policy
(tdd SS5.11, D-008)"*. Setting it fires a view event immediately:

```
<<< {"method":"session/approvalModeChanged","params":{"sessionId":"…","viewCursor":"v:…:1",
     "sourceRange":{…},"mode":"allowAll","source":"approvalReconfigure",
     "commandId":"01a05d5e-33e4-…","clientName":"omm_probe"}}
```

`session/list` — pagination bound is published and enforced:

```
>>> {"method":"session/list","params":{"limit":201}}
<<< {"code":-32602,"message":"invalid session/list limit: expected 1..=200","data":{"kind":"invalidParams"}}
```

Result `{sessions: Session[], nextCursor: string|null}`, ordered `updatedAt` descending.

`session/read` — no attach, no lease, no subscription. Verified from a second host process against
a session the first host had never loaded: it returned `"status":"notLoaded"` and
`"pendingRequests":[]` and produced no `session.resumed` record.

`session/fork`:

```
>>> {"method":"session/fork","id":16,"params":{"commandId":"01a05d60-6bd8-…","sessionId":"01a05d5f-f2eb-…"}}
<<< {"method":"session/started","params":{"session":{"sessionId":"01a05d60-6bda-…", …}}}
<<< {"id":16,"result":{"session":{…fork…},"history":{…},"pendingRequests":[],"viewCursor":"…"}}
```

Bad cut point:

```
>>> cutPoint {"lastTurnId":"01a05d5e-0000-7000-8000-000000000009"}
<<< {"code":-32023,"message":"invalid fork boundary for session …: InvalidCut",
     "data":{"kind":"forkBoundaryInvalid","retryable":false,"sessionId":"…","lastTurnId":"…"}}
```

The durable fork record shows the provenance format (`ForkProvenance.cutCursor`, which the schema
insists is display-only and unparseable):

```json
{"payload_type":"session.fork.created","payload":{
  "fork_session_id":"01a05d60-6bda-7862-93be-1a80e021e999",
  "source_session_id":"01a05d5f-f2eb-7c40-b5fc-1ea15256164b",
  "source_cut_cursor":"session:01a05d5f-f2eb-7c40-b5fc-1ea15256164b:turn:0",
  "created_by_command_id":"01a05d60-6bd8-791b-abcc-6b3ea1a5f266","schema_version":1}}
```

`session/resume` — **all five history rungs verified**:

| Request | `history.mode` | `noneReason` | Payload |
|---|---|---|---|
| `{"excludeItems":true}` | `none` | `excluded` | `items:null, snapshot:null` |
| `{"cursor":"v:…:2"}` | `none` | `cursorSuffix` | `items:null, snapshot:null` |
| `{"history":"anchored"}` | **`inline`** (downgraded — no compaction boundary exists) | — | `items:[…]` |
| `{"history":"inline"}` | `inline` | — | `items:[…]` |
| `{"history":"snapshot"}` | `snapshot` | — | `snapshot:{schemaVersion:1, viewCursor:"v:…:7", state:{…}}` |

This directly demonstrates the *"report what was served, never what was asked for"* rule.
`HistoryPreference` is **closed** (`auto`, `inline`, `snapshot`, `anchored`); `HistoryMode` is
**open** (`anchoredSnapshot`, `inline`, `snapshot`, `none`) — asymmetric on purpose.
Under `auto` the full rung order is documented as
`anchoredSnapshot → inline → snapshot → elided snapshot → none`.

Full `SnapshotState` observed:

```json
{"schemaVersion":1,"viewCursor":"v:…:7","state":{
  "items":[…], "activeTurn":null, "queuedTurns":[],
  "pendingApprovals":[], "pendingUserInputs":[],
  "approvalMode":{"mode":"promptUnmatched","source":"startup","lastCommandId":null},
  "effectiveModel":null, "goal":null, "todoList":null, "branch":null,
  "tokenUsage":{"promptTokens":0,"outputTokens":0,"totalTokens":0},
  "turnCount":0}}
```

Note `contextUsage` is **omitted**, while the four siblings are present-null. The schema doc calls
this out as a live escalation: *"SS4.9.1 says '`null`/absent' and SS4.9.2's worked example OMITS the
member; the binary omits it too … INV-017 admits one or the other, never both; escalated under
#22785."* The binary's behaviour matches the doc's admission.

### 7.2 Cross-host writer lease

Two `muse serve` processes, same `MUSE`/`HOME`, same session:

```
host A: session/start                 -> sessionId 01a05d63-106a-7eb3-a263-194ae68bb747 (loaded, subscribed)
host B: session/resume same sessionId ->
  {"code":-32021,"message":"session 01a05d63-… is already in use",
   "data":{"kind":"sessionInUse","retryable":false,"sessionId":"01a05d63-…"}}
host B: session/list                  -> the session appears with "status":"notLoaded"
```

Exactly as documented: *"`session/list` reports `notLoaded` for sessions loaded by **other** hosts."*
**One writer per session, enforced across processes.** Any control plane must treat
`-32021` as "someone else owns this" and route to `session/read` instead.

### 7.3 Crash recovery / reconciliation

I killed a host with an approval still pending, then resumed the same session from a fresh host.
The reconciliation is fully observable on the view stream:

```
<<< approval/resolved   decision:"abort", policyResult:"deny", resolvedBy:"user",
                        (no decidedByCommandId), stageEvidence[0].resolution.kind:"unresolved"
<<< approval/updated    change:{"kind":"restartRecovered","reparsed":false,"executable":false}
<<< turn/completed      terminal:"cancelled", reason:"resume_reconcile:orphaned_by_process_loss"
<<< item/completed      userShell revision 2, status:"cancelled", visibleOutput:"", durationMs:0
<<< approval/requested  NEW approvalId 550e826d-7f0c-5877-bb4b-01109a2a1d1a  (re-opened for the user)
```

Two things to notice. First, the re-opened approval id is **deterministic** — its version nibble is
`5` (`…-5877-…`), i.e. a UUIDv5 derived from the recovery inputs, not a random v4 like the original
`58d496cf-a0ac-4ce1-…`. Second, **the pending request is re-issued as a plain `approval/requested`
notification**, not as a server→client JSON-RPC *request*, even though `ApprovalRequestParams` is
documented as *"Full params shared by `approval/request` and `approval/requested`"*. The
`approval/request` and `userInput/request` method names appear in doc text but are not routed:

```
>>> {"method":"approval/request","id":14,"params":{}}   -> -32601 methodNotFound
>>> {"method":"userInput/request","id":15,"params":{}}  -> -32601 methodNotFound
```

**INFERRED**: the server→client *request* form is a designed-but-unshipped delivery mode. Supporting
strings exist on the client side (`the TUI serves no server-initiated request (\`…\`)`,
`the MSP host delivered unhandled server-initiated request \`…\``, `the MSP host delivered a
non-notification outside a request`), so the client library is built to receive them.

### 7.4 The approval plane — end to end

`session/userShell` is the offline path into the approval machinery. Full `approval/requested`
payload for `echo hi-from-msp; pwd`, showing **multi-stage** decomposition:

```json
{
  "sessionId": "01a05d61-0ac9-7d21-95c0-5094f168257b",
  "approvalId": "5e699664-9c3d-4ef6-96c8-4fd3f4ad4de5",
  "turnId": "01a05d61-16ff-719a-a9e4-721e60aa4531",
  "taskId": "5e699664-9c3d-4ef6-96c8-4fd3f4ad4de5",
  "itemId": "5e699664-9c3d-4ef6-96c8-4fd3f4ad4de5",
  "toolCallId": "user_shell_01a05d61-16ff-719a-a9e4-721e60aa4531",
  "toolName": "shell",
  "rawArgs": "{\"command\":\"echo hi-from-msp; pwd\"}",
  "viewCursor": "v:01a05d61-0ac9-7d21-95c0-5094f168257b:2",
  "sourceRange": { "stream": {"kind":"session","id":"…"}, "first": {"id":"bf8602d7-…","sequence":8}, "last": {…} },
  "subject": {
    "kind": "shell",
    "command": "echo hi-from-msp; pwd",
    "workspaceRoot": "…/ws",
    "stages": [
      { "requirementId": {"approvalId":"5e69…","sourceIndex":0}, "position": 1, "totalStages": 2,
        "argv": ["echo","hi-from-msp"], "argvComplete": true,
        "resolution": {"kind":"unresolved"},
        "suggestedPrefix": {"argvPrefix":["echo"],"label":"Always allow in this workspace: echo ..."} },
      { "requirementId": {"approvalId":"5e69…","sourceIndex":1}, "position": 2, "totalStages": 2,
        "argv": ["pwd"], "argvComplete": true,
        "resolution": {"kind":"unresolved"},
        "suggestedPrefix": {"argvPrefix":["pwd"],"label":"Always allow in this workspace: pwd ..."} }
    ]
  },
  "currentRequirementId": {"approvalId":"5e69…","sourceIndex":0},
  "availableChoices": [
    {"choiceId":"allow_once","label":"Allow once","decision":"approved","scope":"once"},
    {"choiceId":"abort","label":"Reject","decision":"abort","scope":"once","acceptsFeedback":true}
  ],
  "protectedWrite": false,
  "judgeEscalated": false
}
```

The shell string is **parsed into argv stages** and each stage is separately approvable. The
`requirementId: {approvalId, sourceIndex}` pair is the race guard. All three failure arms verified:

```
stale requirementId (sourceIndex 99):
  {"code":-32053,"message":"approval 5e69… requirement is stale",
   "data":{"kind":"approvalRequirementStale","retryable":false,"approvalId":"5e69…",
           "currentRequirementId":{"approvalId":"5e69…","sourceIndex":0}}}

bogus choiceId:
  {"code":-32052,"message":"choice \"not-a-choice\" is invalid for approval 5e69…",
   "data":{"kind":"approvalChoiceInvalid","retryable":false,"approvalId":"5e69…","choiceId":"not-a-choice"}}

valid decision on stage 1 of 2:
  {"id":6,"result":{"commandId":"01a05d61-3e3c-…","status":"accepted","approvalId":"5e69…","terminal":false}}
  <<< approval/updated  currentRequirementId.sourceIndex 0 -> 1
                        stages[0].resolution.kind: "unresolved" -> "allowOnce"
                        change: {"kind":"stageResolved","requirementId":{…,"sourceIndex":0},
                                 "choiceId":"allow_once","decision":"approved"}

re-deciding stage 0 after it advanced:  -32053 approvalRequirementStale  (sourceIndex now 1)
```

`terminal: false` on the ack means *"the choice satisfied a stage but further requirements remain"*.
For a single-stage command the same decision yields `terminal: true` followed by the durable
resolution:

```json
{"method":"approval/resolved","params":{
  "sessionId":"…","viewCursor":"v:…:4","sourceRange":{…},
  "approvalId":"6dab1248-98d7-41cb-83f5-2e41635a5f16",
  "itemId":"6dab1248-…","turnId":"01a05d61-d2b0-…",
  "decision":"approved","policyResult":"allow","resolvedBy":"user",
  "decidedByCommandId":"01a05d61-ea29-706b-93fe-8bdb7fa28258",
  "stageEvidence":[{"requirementId":{"approvalId":"6dab…","sourceIndex":0},
                    "position":1,"totalStages":1,"argv":["echo","hi-from-msp"],
                    "resolution":{"kind":"allowOnce"}}]}}
```

…then the tool item terminates:

```json
{"method":"item/completed","params":{"item":{
  "itemId":"4c57d42c-…","kind":"userShell","turnId":null,"revision":2,"status":"completed",
  "commandId":"01a05d61-d2b0-…","visibleOutput":"hi-from-msp\n",
  "commandText":"echo hi-from-msp","exitCode":0,"durationMs":6048}}}
```

`ApprovalDecideParams.feedback` is documented as *"Free-text guidance delivered to the model with
the denial … **Never persisted in the durable audit record** (live steer only), so it is absent from
`approval/resolved`."* Only choices with `acceptsFeedback: true` accept it.

`approval/listPending` is the pull dual and returns *"Exactly the `approval/request` params of tdd
SS5.3, one per pending approval"* plus the `userInput/request` params. Verified: after killing the
notification via opt-out, `approval/listPending` still returned the full approval object and I
could decide it. This is the recovery path for a client that opts out of push.

`session/setApprovalMode` (select-never-create; `ApprovalMode` is closed):

```
allowAll -> denyUnmatched:  {"status":"accepted","applyOutcome":"completed",
                             "effectiveMode":{"mode":"denyUnmatched","source":"approvalReconfigure",
                                              "lastCommandId":"01a05d60-16b1-…"}}
denyUnmatched -> denyUnmatched:
                            {"status":"accepted","applyOutcome":"noop",
                             "effectiveMode":{"mode":"denyUnmatched","source":"startup","lastCommandId":null}}
"bogusMode":                {"code":-32602,"message":"Invalid params: session/setApprovalMode params",
                             "data":{"kind":"invalidParams","retryable":false}}
```

**Anomaly worth flagging:** the `noop` arm reports `source:"startup"` and `lastCommandId:null`,
discarding the fact that a prior command set the mode. On the non-noop arm it correctly reports
`source:"approvalReconfigure"` with the command id. A client that reads `effectiveMode` from a noop
ack will therefore mis-attribute the mode. **INFERRED** defect; the `EffectiveApprovalModeState`
doc says `lastCommandId` is *"The command that last set it; `null` when no command did"*, and a
command did.

Internal approval vocabulary recovered from the binary (the durable side of
`ApprovalMode`): `ApprovalBackendMode` = `allow_all`, `prompt_unmatched`, `on_request`,
`deny_unmatched`; plus `approval_mode_ceiling` and *"approval mode exceeds or is incomparable with
the sealed start…"* — i.e. the wire mode is clamped by a host-side ceiling.

### 7.5 Turns

`turn/start` ack and the resulting stream (echo-less host, so the model leg fails):

```
>>> {"method":"turn/start","id":3,"params":{"commandId":"01a05d5e-3fc4-788f-…","sessionId":"…",
      "input":[{"type":"text","text":"hello muse"}],"displayText":"hello muse"}}
<<< {"id":3,"result":{"commandId":"01a05d5e-3fc4-788f-…","status":"accepted",
      "turnId":"01a05d5e-3fc4-788f-…","startedNewTurn":true,"disposition":"started"}}
<<< turn/started      {"turnId":"01a05d5e-3fc4-…","commandId":"01a05d5e-3fc4-…","viewCursor":"v:…:2"}
<<< item/completed    {"item":{"kind":"userMessage","revision":1,"status":"completed","text":"hello muse", …}}
<<< turn/completed    {"turnId":"…","terminal":"failed","durationMs":94,
                       "reason":"not logged in: run /login to add an API key",
                       "error":{"kind":"modelError","message":"not logged in: run /login to add an API key",
                                "retryable":true}}
```

This is the load-bearing demonstration that **mid-turn failures never surface as JSON-RPC errors** —
`TurnCompletedParams.error` is *"Present iff `terminal` is `\"failed\"` … mid-turn failures reach the
client here — never as a JSON-RPC error."*

Input validation:

```
input: []                       -> -32602 "Invalid params: turn/start input must be non-empty"
{"type":"zzz"}                  -> -32602 "unknown variant `zzz`, expected one of `text`, `image`, `mention`"
{"type":"mention"} (no expApi)  -> -32602 kind:"experimentalRequired", descriptor:"turn/start.input.mention"
{"type":"mention"} (expApi=true)-> -32602 "turn/start mention input is reserved and not implemented"
{"type":"image","mediaType":"image/png","base64Data":"iVBORw0KGgo="}          -> accepted
{"type":"image", …, "width":10}  (no height)                                  -> -32602 "image width and height must be provided together"
```

The `dependentRequired: {"height":["width"],"width":["height"]}` in the JSON Schema is therefore a
faithful description of runtime behaviour. Note the deserializer's variant list is
`text | image | mention` while the published closed `TurnInputPartType` is only `text | image` —
`mention` is the experimental third arm, gated *and* unimplemented. `turn/steer` refuses it outright
(binary string: `turn/steer does not accept mention parts`).

`IfBusy` (closed): `queue` (the wire default), `steer`, `replace`. The doc rationale:
*"an SDK caller who has not looked at session state should not silently mutate an in-flight turn."*
`TurnStartDisposition` (open): `started`, `queued`, `steered`; `startedNewTurn` is retained only as
a boolean shorthand and `disposition` is authoritative.

`turn/cancel` with no `turnId` resolves the current foreground turn at admission:

```
<<< {"id":22,"result":{"commandId":"01a05d60-9828-…","status":"accepted","turnId":"01a05d5f-feaf-776e-…"}}
```

`turn/unqueue` for a turn that never existed:

```
<<< {"code":-32030,"message":"turn/unqueue command 01a05d60-a029-… rejected: missing_run",
     "data":{"kind":"commandRejected","retryable":false,"commandId":"…","reason":"missing_run"}}
```

`turn/interrupt` vs `turn/cancel`: interrupt runs on the *priority lane* and may pair
`retract: true` (*"if the turn is cancelled before any assistant output committed, the submission is
durably retracted (view event `turn/retracted`) so the client may restore the prompt text"*).
Pairing a retract with a plain `turn/cancel` is durably rejected with reason `not_paired_interrupt`.
**Not runtime-verified** — I could not keep a turn running without a model.

`turn/steer` requires `expectedTurnId`, which *"closes the race where the turn completes or is
replaced between your read and your steer: input meant for turn A can never leak into turn B."*
**Not runtime-verified** for the same reason.

### 7.6 Models

```
>>> {"method":"model/list","id":6,"params":{}}
<<< {"id":6,"result":{"providerId":"meta","profileId":"tbh","source":"bundledCatalog","models":[]}}
```

The catalog is **empty** in this build — *"`models` MAY be empty — a build shipped with no bundled
models is a supported configuration."* Consequently every `session/setModel` fails:

```
{"modelId":"fake-model-1","providerId":"meta"} -> -32030 commandRejected reason:"unsupported_route"
{"modelId":"echo","providerId":"echo"}         -> -32030 commandRejected reason:"unsupported_route"
{"modelId":""}                                 -> -32030 commandRejected reason:"invalid_target"
```

`ModelCatalogSource` (open): `providerCatalog`, `fakeCatalog`, `unresolvedCatalog`, `bundledCatalog`,
`configCatalog`. `ModelCost` is carried **verbatim as decimal strings** and *"never rounded or
re-formatted by the host … Cost arithmetic stays client-local view math."*
Other model-reconfigure reasons in the binary: `profile_unavailable`, `named_catalog_unavailable`,
`session_blocked`, `stale_cat…`. The rejection is durably logged as
`runtime.model_reconfigure.rejected`.

### 7.7 `view/page`

```
limit 0     -> -32602 "invalid limit 0: expected 1-1000"
limit 1001  -> -32602 "invalid limit 1001: expected 1-1000"
cursor+anchor together -> -32602 "anchor is mutually exclusive with cursor"
                          data:{"kind":"invalidParams","sessionId":"…"}
cursor "bogus-cursor"  -> -32011 "unknown cursor anchor"
                          data:{"kind":"notFound","reason":"missingAnchor","sessionId":"…","anchor":"bogus-cursor"}
anchor "latestCompaction" (no boundary installed)
                       -> -32042 "session has no installed compaction boundary"
                          data:{"kind":"noBoundary","sessionId":"…","anchor":null,"latestBoundaryCursor":null}
```

Note the explicit `null`s in the `-32042` payload — the schema is emphatic that these are
*"present on the wire, never a dropped key (spec 208 FM-005)"*, and the binary honours it.

A forward page returns exactly the shape the schema promises — an array of *unframed* notifications,
i.e. `{method, params}` with `params` verbatim and `viewCursor` still nested inside it:

```json
{"events":[
  {"method":"session/approvalModeChanged","params":{"sessionId":"…","viewCursor":"v:…:1","sourceRange":{…},"mode":"allowAll",…}},
  {"method":"turn/started","params":{…}},
  {"method":"item/completed","params":{…}}],
 "nextCursor":"v:…:3"}
```

Backward paging returns events **still ascending by `viewCursor`** (verified), with `nextCursor`
being the *first* event's cursor and `null` at the end of the view in that direction.

Related errors compiled in but not triggered here: `-32040 viewTruncated`
(`requested range predates the retained view`, `data.earliestCursor`), `-32603` override
`pageEventTooLarge` (`view event exceeds the page result budget`, `data.viewCursor` +
`data.limitBytes`), `-32042` arms `boundaryPruned` (`compaction boundary has been pruned`) and
`boundaryUnusable` (`compaction boundary has no servable checkpoint`), plus
`session is not loaded on this host` and `view range read failed`.

### 7.8 `view/unsubscribe`

Idempotent, returns `{}` both times, and does **not** unload the session:

```
>>> view/unsubscribe -> {"id":17,"result":{}}
>>> view/unsubscribe -> {"id":18,"result":{}}     (still not subscribed, still loaded)
```

### 7.9 Host posture flags are real and non-negotiable

`serve --disable-shell` (plus `--disable-sandbox --disable-write --sandbox-network restricted
--trust-workspace`). The command is still *admitted* — the ack is about admission, not policy —
and the item terminates `rejected`:

```
<<< {"id":3,"result":{"commandId":"01a05d69-b23c-…","status":"accepted"}}
<<< item/completed {"item":{"kind":"userShell","revision":2,"status":"rejected",
                    "visibleOutput":"tool unavailable: unknown tool `shell`", …}}
```

`serve --no-session-log` flips the whole host into the ephemeral profile:

```
initialize -> "sessionDurability":"ephemeral", "grantedCapabilities":[]
session/start ->
  {"session":{"sessionId":"01a05d66-44de-…","path":"", …},
   "viewCursor":"pending:seam-c-session-view-fold"}
```

`Session.path` is `""` — exactly as documented: *"Under the ephemeral session profile it is the
**empty string**, meaning 'no durable log exists' (tdd SS2.13.2) — the one value a client must not
hand to a filesystem call."*

---

## 8. The experimental surface, and what the gates actually do

### 8.1 `experimentalApi` (wire-level, per connection)

`ClientCapabilities.experimentalApi` (default `false`) is echoed back in `InitializeResult`. Its
enforcement is the `-32601`/`-32602` override kind `experimentalRequired` carrying an SS1.5.1
`descriptor`. The **only** live instance I found in 1.0.1:

```
>>> turn/start input:[{"type":"mention","text":"@README.md"}]     (experimentalApi absent)
<<< {"code":-32602,"message":"turn/start.input.mention requires experimentalApi capability",
     "data":{"kind":"experimentalRequired","retryable":false,"descriptor":"turn/start.input.mention"}}
```

With `experimentalApi: true` it becomes `"turn/start mention input is reserved and not implemented"`.
No method became available under `experimentalApi: true` (all 17 probes still `-32601`).

There is also one **deliberately unpublished** experimental field that the host silently accepts:

```
>>> turn/start {... , "providerRequestOptions":{"foo":1}}   -> accepted, turn ran
```

The schema comment is explicit about why you will not find it in the bundle:

> "`turn/start.providerRequestOptions` is deliberately absent: **RULED (#22785 E3, owner,
> 2026-08-26) to stay off the published schema.** The schema is the contract, and a free-form-object
> node kind is revisited only if the experimental field graduates."

**So the published schema is not a complete description of what `turn/start` accepts.** That is the
answer to "why are the stable and experimental bundles identical": the experimental delta is
deliberately excluded from the model, not merely absent from this build.

### 8.2 `MUSE_EXPERIMENTAL_SDK_ENABLED`

Feature-config key `sdk_enabled` (it appears in the binary's gate-name list between
`session_recovery_shadow`/`tui_msp_client` and `meta_context_workspace_only`). Verified:

- **Does not** change `muse --help`, `muse serve --help`, or `muse schema --help` (byte-identical diffs).
- **Does not** change any schema export (byte-identical bundle, identical fingerprint).
- **Does not** unlock `serve --listen`.
- **Does not** gate `muse serve` at all — `serve` works with the gate unset.

By contrast, `MUSE_EXPERIMENTAL_PLUGINS=1` *does* visibly change `muse --help` (adds
`plugins   Validate and manage plugin bundles`), so the diff methodology is sound and
`SDK_ENABLED`'s null result is meaningful.

**INFERRED**: `sdk_enabled` gates a *distribution* concern — whether this build advertises/permits
SDK usage — rather than any wire behaviour. **UNKNOWN**: whether it is read anywhere other than the
feature-config table. I found no string that ties it to a code path.

### 8.3 `MUSE_EXPERIMENTAL_TUI_MSP_CLIENT`

This is the interesting one. The evidence says the TUI is being migrated to be *an MSP client of an
in-process MSP host*, i.e. the same protocol dogfooded internally:

- Leaked Rust source path: `fbcode/musecode/build/src/crates/tui/src/terminal/msp_session_bridge.rs`
- Client identity strings: **`tbh_tui-inproc`** and **`TBH TUI in-process MSP client`**
  (note `tbh_tui-inproc` does not match `^[a-z0-9_]+$` — the hyphen would be rejected by the wire
  validator, so the in-process path presumably bypasses `initialize` validation, or the two strings
  are used in different places. **UNKNOWN**.)
- Startup error strings, in order:
  `could not assemble the TUI MSP host: `, `could not initialize the TUI MSP host: `,
  `could not bind the TUI MSP notification edge: `, `could not attach the TUI MSP client: `,
  plus `assemble MSP host: `, `join the selected MSP bridge owner: `, `gate-on MSP startup failed: `.
- Runtime/shutdown strings: `embedded MSP host drain degraded during owner close`,
  `selected MSP bridge has no successor inputs`,
  `selected MSP bridge owner panicked during exit shutdown`,
  `the TUI MSP owner generation is exhausted`, `MSP session client failed: `,
  `selected MSP session client request failed`.
- Client-side protocol strictness strings:
  `the MSP host delivered unknown notification \`…\``,
  `the TUI serves no server-initiated request (\`…\`)`,
  `the MSP host delivered unhandled server-initiated request \`…\``,
  `\`…\` response id did not match request`,
  `the MSP host delivered a non-notification outside a request`.
- Migration strings showing the two rendering lanes coexisting:
  `TUI materialized session view attachment skipped: the resume replay source is incomplete;
  leaving the sidecar for the MSP lane`, and
  `first touch deferred: the log tail is not checkpoint-quiescent; MSP history stays bounded-none
  until a load settles it`.
- User-facing failure copy: `Source view could not be reloaded. Retry resume, or exit and start a
  new chat. Details: `.

Also present: `TBH_TMUX_MSP_PATH_ATTESTATION_FILE` / `TBH_TMUX_MSP_PATH_ATTESTATION_NONCE`,
suggesting a tmux-based harness that attests the MSP binary path.

I could not exercise it — the TUI needs a real TTY:

```
$ MUSE_EXPERIMENTAL_TUI_MSP_CLIENT=1 muse --provider echo </dev/null
Device not configured (os error 6)
```

(identical with and without the gate).

**Relationship to `serve`, stated plainly:** `MUSE_EXPERIMENTAL_TUI_MSP_CLIENT` makes the TUI speak
the *same protocol* an external client speaks over `muse serve`, but over an in-process channel
instead of stdio. That is the strongest possible signal that MSP is the intended universal
interface: Meta is eating its own dog food by rewriting their own TUI on top of it. If that
migration lands, every capability the TUI has becomes reachable from an external client.

### 8.4 Deferred and reserved surface

From the bundle's `reserved` block:

```json
[
  {"identifier":"-32060..-32069",
   "reference":"#13929 — SS6 raw log & projections error block, deferred whole; a v1 host never emits it"},
  {"identifier":"-32012",
   "reference":"SS1.6: deliberately never assigned; command_id_conflict is commandRejected"},
  {"identifier":"deprecationNotice",
   "reference":"#13929 — reserved notification, no producer before 1.0"}
]
```

Deferred lanes, with issue numbers:

| Lane | Status | Issue |
|---|---|---|
| `workflow/*` methods | params + ack ratified (tdd SS3.19/SS3.20); protocol types and bundle rows outstanding | #14410 |
| SS6 raw-log altitude + `rawLog` capability + `-32060..-32069` | deferred whole | #13929 |
| unix-socket and websocket transports (`serve --listen`) | deferred post-v1 | #13929 |
| `deprecationNotice` notification | reserved, no producer before 1.0 | #13929 |
| `item/readOutput` | error machinery compiled in, method not routed | #208 |
| `turn/unqueued` notification row | ratified deferral (mentioned in `UnframedViewNotificationParams`) | #207 / #14407 |
| `turn/start.input.mention` | gated *and* "reserved and not implemented" | — |
| `userInput` image `attachments` | "reserved field in v1, rejected if sent" | — |
| `SessionConfig` members | none in v1 | #23456 |

The binary also carries the **runtime** command-kind vocabulary, which is a strict superset of the
MSP method set. Recovered verbatim as one adjacent literal run:

```
session/start session/resume session/fork turn/start turn/steer turn/interrupt turn/cancel
turn/unqueue session/compact session/userShell model/list
goal/edit goal/clear goal/pause goal/resume
subagent/sendMessage subagent/followupTask subagent/interrupt subagent/stop subagent/resume
subagent/reopen subagent/close subagent/readResult
workflow/cancel workflow/childControl
task/background task/stop task/stopAll
```

So `goal/*` (4), `workflow/*` (2) and `task/*` (3) exist as runtime commands with no MSP door. Note
`session/goalChanged` and the `workflow`/`toolCall.background` item fields are *observable* over MSP
— you can watch goals and background tasks change, you just cannot drive them.

---

## 9. The error registry (29 rows, verbatim)

| Code | `kind` | `retryable` | Override kinds |
|---:|---|---|---|
| -32700 | `parseError` | false | — |
| -32600 | `invalidRequest` | false | `notInitialized`, `alreadyInitialized` |
| -32601 | `methodNotFound` | false | `experimentalRequired` |
| -32602 | `invalidParams` | false | `experimentalRequired` |
| -32603 | `internal` | false | `pageEventTooLarge`, `outputResultTooLarge` |
| -32001 | `overloaded` | **true** | — |
| -32002 | `inputTooLarge` | false | — |
| -32010 | `capabilityRequired` | false | — |
| -32011 | `notFound` | false | — |
| -32013 | `interrupted` | *(unset)* | — |
| -32014 | `cancelled` | *(unset)* | — |
| -32020 | `sessionNotFound` | false | — |
| -32021 | `sessionInUse` | false | — |
| -32022 | `sessionAmbiguous` | false | — |
| -32023 | `forkBoundaryInvalid` | false | — |
| -32024 | `sessionNotLoaded` | false | — |
| -32025 | `sessionStreamMismatch` | false | — |
| -32030 | `commandRejected` | false | — |
| -32031 | `backpressured` | **true** | — |
| -32040 | `viewTruncated` | false | — |
| -32042 | `boundaryPruned` | false | `boundaryUnusable`, `noBoundary` |
| -32050 | `approvalNotFound` | false | — |
| -32051 | `approvalAlreadyResolved` | false | — |
| -32052 | `approvalChoiceInvalid` | false | — |
| -32053 | `approvalRequirementStale` | false | — |
| -32054 | `approvalReviewerUnavailable` | false | — |
| -32055 | `userInputNotFound` | false | — |
| -32056 | `userInputAlreadySettled` | false | — |
| -32057 | `userInputAnswerInvalid` | false | — |

Design notes: `code` is a **plain integer** on the wire; the value domain is the registry
(INV-018). `message` is *"Never a branch point (SS1.6)"* — always branch on `data.kind`.
`ErrorKind` is **open**, so a new kind is additive. `data.retryable`, when present,
*"overrides the error table row's `retryable` default for this code."*
`-32012` is permanently unassigned. `-32060..-32069` is reserved for the deferred SS6 block.

`ErrorData` has 23 members: `kind` (always present when `data` is), `retryable`, `descriptor`,
`capability`, `commandId`, `reason`, `limitBytes`, `alignedNextOffset`, `capacity`, `details`,
`sessionId`, `earliestCursor`, `anchor`, `latestBoundaryCursor`, `lastTurnId`, `paths`,
`resolution`, `currentRequirementId`, `choiceId`, `approvalId`, `userInputId`, `settlement`,
`viewCursor`.

`commandRejected` reasons observed live: `missing_run`, `unsupported_route`, `invalid_target`,
`runtime_busy`. Additional reasons found as strings: `session_id_conflict`, `command_id_conflict`,
`not_paired_interrupt`, `missing_actor`, `apply_failed`, `no_compactable_history`,
`invalid_approval_id`, `invalid_command_id`, `invalid_goal_state`,
`resume_reconcile:orphaned_by_process_loss`.

---

## 10. What MSP is *not*: comparisons

### 10.1 MSP vs MCP

Both are JSON-RPC 2.0 with an `initialize` handshake + `initialized` notification + capability
negotiation. That's where it ends.

| | MCP | MSP |
|---|---|---|
| Direction of control | Host **calls into** a server to borrow tools/resources/prompts | Client **drives** an agent host; the host owns the model, tools, sandbox |
| Primitives | `tools/*`, `resources/*`, `prompts/*`, `sampling/*` | sessions, turns, transcript items, approvals, view cursors |
| State | Mostly stateless per call; server exposes capabilities | Event-sourced; every command durably logged, every event has a cursor and a provenance range |
| Versioning | `protocolVersion` date string negotiated | `schema.version: 1` **plus** a content fingerprint of the whole surface |
| Streaming | notifications + progress tokens | a first-class **view stream** with `item/delta`, `revision` replace-iff-higher, `view/gap` holes and `view/page` backfill |
| Reconnect | reconnect and re-list | `session/resume` with a cursor → suffix-only, or a snapshot rung, or a compaction anchor |
| Idempotency | none | mandatory client-minted UUIDv7 `commandId` with value-identical replay |
| Backpressure | none specified | `-32031` with `data.capacity` (=4) and `-32001` |
| Server→client requests | yes (`sampling/createMessage`, `roots/list`, `elicitation`) | *designed* (`approval/request`, `userInput/request`), *shipped as notifications* |
| Multi-writer | n/a | explicit writer lease, `-32021 sessionInUse` |
| Transports | stdio + streamable HTTP | **stdio only**; unix/ws deferred |

The `elicitation`/`sampling` analogy is worth pulling out: MSP's `userInput/*` plane is
structurally MCP elicitation (questions with `single`/`multiple` selection modes, options with
previews, min/max selections, free text ≤500 chars, plus a `clarify` escape hatch), but it is
*server-driven and durable* — the prompt is a durable record, has a `userInputId`, survives
process death, and is recoverable via `approval/listPending`.

The muse binary is *also* an MCP **client** (strings: `MCP stdio header ended before newline`,
`MCP stdio server closed stdout`, `failed to build MCP HTTP client`, `MCP is disabled for this
runtime`, `remap MCP after plugin installs`). So MCP is how muse consumes external tools; MSP is
how something consumes muse.

### 10.2 MSP vs Claude Code's SDK

Claude Code's SDK is a **library** (TypeScript/Python) with a streaming-message iterator, hooks,
permission callbacks and subagent definitions. MSP is a **protocol** with a *generated* type
package and no client library shipped in the binary.

| | Claude Code SDK | MSP |
|---|---|---|
| Artifact | npm/pip package, ergonomic async iterator | 190 KB JSON Schema + 103 KB `.d.ts` you generate yourself; you write the client |
| Session identity | session id, `--resume`, `--fork-session` | `session/start` / `session/resume` / `session/fork` with durable provenance + cut points |
| Permissions | `canUseTool` callback / permission modes / hooks | `approval/requested` → `approval/decide`, with **argv-stage decomposition**, `requirementId` race guards, durable `stageEvidence` |
| Streaming | assistant/user/result message objects | `item/started` → `item/delta` (field-path append) → `item/updated` (revision bump) → `item/completed` |
| Interruption | `interrupt()` | `turn/interrupt` (priority lane, optional durable retract) vs `turn/cancel` (normal lane) vs `turn/unqueue` (pre-launch reclaim) — three genuinely different operations |
| Compaction | automatic, mostly invisible | `session/compact` command + `compaction` item + `summarizedThrough` anchor you can resume from |
| Backfill after disconnect | replay the transcript | `view/page` with cursors, `view/gap` brackets, snapshot rungs, `session/read` without attaching |
| Concurrency | one process per session | writer lease + `-32021`, `session/list` shows other hosts' sessions as `notLoaded` |
| Model selection | `--model` | `model/list` + `session/setModel` (empty catalog in this build) |
| Extensibility contract | semver on the package | `x-msp-openness` per enum + additive-optional evolution + a fingerprint mismatch warning |

MSP is the more rigorous design on every axis that matters for a long-running control plane —
durability, replay, multi-client, gap recovery, race guards. It is the less pleasant design for
"just run a prompt and give me the text", because there is no library and the ack/outcome split
means every operation needs a correlator.

### 10.3 MSP vs `muse exec --json` (the *other* machine surface)

This is the sharpest contrast in the product, and it is easy to pick the wrong one.

```
$ muse exec --json --provider echo --disable-approval --no-session-log "hello from exec"
{"schema_version":1,"id":"018f0000-0000-7000-8000-00000000c350",
 "stream":{"kind":"session","id":"01a05d66-b19a-7433-9396-29a40269b0e6"},"sequence":1,
 "recorded_at":1780531400000000,"record_type":"reconciliation","durability":"durable",
 "causation_id":"7fbcc61e-26db-407b-b98c-950b93bb7f80",
 "payload_type":"runtime.command.accepted","payload_schema_version":1,
 "payload":{"kind":"command_accepted","command_id":"7fbcc61e-…","client_id":null,"command_kind":"turn.submit"}}
{"…","payload_type":"session.run.linked","payload":{"kind":"session_run_linked",…}}
{"…","payload_type":"turn.input.user","durability":"ephemeral","payload":{"prompt":"hello from exec"}}
{"…","payload_type":"run.lifecycle.started",…}
{"…","payload_type":"task.stream.linked",…}
{"…","payload_type":"task.lifecycle.proposed","payload":{"event":{"task_kind":"model.unknown.response"}}}
{"…","payload_type":"task.lifecycle.accepted",…}
{"…","payload_type":"task.lifecycle.scheduled","payload":{"event":{"idempotency_key":"model:7fbcc61e-…:4e24138d-…"}}}
{"…","payload_type":"task.lifecycle.side_effect_intent","payload":{"event":{"operation":"model.unknown.response","policy_decision":"not_applicable",…}}}
{"…","payload_type":"task.lifecycle.started",…}
{"…","payload_type":"run.output.delta","durability":"ephemeral","payload":{"text":"echo: hello from exec"}}
{"…","payload_type":"task.lifecycle.completed",…}
{"…","payload_type":"run.terminal.completed","payload":{"terminal":"completed","text":"echo: hello from exec","reason":null}}
```

`muse exec --json` emits **the raw durable log records** — snake_case, `payload_type`-tagged,
`schema_version`/`payload_schema_version`-versioned, with `causation_id` chains and
`durability: durable|ephemeral`. This is precisely the SS6 "raw altitude" that MSP treats as an
opaque `sourceRange` and refuses to expose. It has **no published schema**, no fingerprint, no
openness discipline, and every doc comment in the MSP bundle warns you off parsing it.

The same records land on disk. `payload_type` inventory across my session logs:

```
runtime.command_intake.received / .settled     (paired, per command; received is payload v2)
runtime.approval_command_intake.received / .settled
runtime.approval_reconfigure
runtime.model_reconfigure.rejected
runtime.session                (151 — the workhorse, both durable events and ephemeral status)
runtime.session.metadata
runtime.user_intent.accepted / .materialized
session.opened.observed
session.resumed
session.fork.created
user_shell.command / user_shell.result
```

**Rule of thumb: `exec --json` is a firehose of implementation detail with no compatibility
promise; `serve` is the contract.** If you are building anything you intend to keep working across
muse releases, use MSP.

---

## 11. Should you build an "oh-my-musecode" on MSP? — assessment

**Yes for a control plane; with three real caveats.**

### What MSP gives you for free that nothing else in the binary does

1. **A supervisor can watch without owning.** `session/read` and `session/list` take no lease, and
   `view/page` needs no subscription. You can build a dashboard, a policy auditor, or a
   cost tracker that observes every session on a machine while the human's TUI keeps the writer
   lease. This is genuinely rare in agent CLIs.
2. **Approvals are a clean, guarded, remotable decision point.** `approval/requested` →
   `approval/decide` with `requirementId` race guards and argv-stage decomposition is exactly the
   shape you need to put a policy engine, an LLM judge, or a Slack approval button in front of
   tool calls — and the `suggestedPrefix` (`{"argvPrefix":["echo"],"label":"Always allow in this
   workspace: echo ..."}`) is the host handing you a pre-computed policy rule to persist.
3. **Reconnect is solved.** Cursor-suffix resume, snapshot rungs, compaction anchors, `view/gap`
   brackets and durable command replay mean a control plane can crash and rejoin without losing or
   duplicating state. Very few agent protocols get this right.
4. **Idempotency is mandatory, not optional.** Because `commandId` is a UUIDv7 you mint, a retry
   loop is safe by construction and `turnId == commandId` for fresh turns means you can correlate
   without a lookup table.
5. **The type surface is machine-generated and self-describing.** `muse schema generate-ts` gives
   you a typed client in one command, and the fingerprint in `initialize` lets you detect that the
   binary drifted from your generated types.

### Caveats

1. **No provider selection on the wire.** A `serve` host always uses the real provider. Any
   integration test harness must therefore either authenticate (money) or confine itself to
   `session/userShell` + the lifecycle plane. `--provider echo` exists on `muse` and `muse exec`
   but **not** on `muse serve`. This is the single biggest gap for an "oh-my" project that wants
   CI.
2. **Admission capacity is 4.** Not documented in `--help`; discovered only by flooding. A naive
   driver that fires 10 commands will see 6 of them fail with `-32031`.
3. **The published schema is neither complete nor exactly true.** `session/started` is emitted but
   unpublished; `turn/start.providerRequestOptions` is accepted but deliberately unpublished; the
   `mention` variant exists in the deserializer but not in the closed `TurnInputPartType`; and
   `SnapshotState.contextUsage` is omitted where four siblings are present-null. **Write a
   permissive decoder.** The schema's own doc comments admit most of these
   (`#22785` E3/E4/E8, `#19923`), which is admirable, but it means "validate strictly against the
   bundle" will break you.
4. **Stdio only, one connection per process.** A multi-tenant control plane must spawn and supervise
   one `muse serve` per concurrent writer, and route reads through separate lease-free processes.
   Unix/ws transports are deferred (#13929).
5. **`outputRef` is a dangling pointer.** `item/readOutput` is not routed, so large tool outputs are
   visible-but-unfetchable over MSP in 1.0.1. You must read the session log directly (which means
   parsing the unversioned raw altitude) to get the bytes.

### Sketch: how I would build it

- **One `muse serve` per driven session** (writer), supervised, with a 4-permit semaphore and
  exponential backoff on `-32031`/`-32001`.
- **A separate read-only `muse serve`** for the observer plane, using only `session/list`,
  `session/read`, `view/page` — never `session/resume`, so it never takes a lease.
- **Generate the client from `muse schema generate-ts`** at build time, pin the fingerprint, and
  warn (not fail) on mismatch, exactly as SS1.4.1 prescribes.
- **Decode permissively**: unknown notification methods → log-and-ignore; unknown `ItemKind` →
  render `kind` + `status` + `fallbackText`; unknown `ItemStatus` → treat as terminal.
- **Correlate on `commandId`**, wait on the *view stream* for outcomes (`turn/completed`,
  `turn/unqueued`, `approval/resolved`, `item/completed`), never on the ack.
- **Persist your own cursor** per session so a control-plane restart resumes with
  `{"cursor": lastSeen}` and gets a pure suffix.
- **Set `optOutNotificationMethods`** aggressively (e.g. drop `item/delta` if you do not render
  streaming) — it measurably reduces wire volume, and `approval/listPending` covers you for
  anything you dropped except the protected set.

---

## 12. Open questions

1. `session/closed` — exact params and trigger. Present as a literal in the notification-name block
   and suppressible by name, but never observed.
2. Whether the server ever issues a true server→client JSON-RPC *request* (`approval/request` /
   `userInput/request`). Client-side handling strings exist; the host used notifications on every
   path I could reach. Possibly a `MUSE_EXPERIMENTAL_*` or config-gated delivery mode.
3. What `MUSE_EXPERIMENTAL_SDK_ENABLED` actually does. No observable effect on help, schema,
   `serve`, or `--listen`.
4. Whether `MUSE_EXPERIMENTAL_TUI_MSP_CLIENT=1` in a real TTY produces a working
   TUI-over-MSP session, and whether it exposes an inspectable channel (the tmux path-attestation
   env vars hint at a harness).
5. Whether any config file or env var can make a `serve` host use the echo provider. If yes, MSP
   becomes fully CI-testable; if no, that is a genuine product gap.
6. Exact membership of the SS1.7 protected set. `approval/resolved` proven; `userInput/settled` and
   `view/gap` inferred from doc text.
7. The `-32002 inputTooLarge` threshold. A 6 MB text part was accepted without error.
8. Whether `session/setApprovalMode`'s `noop` arm reporting `source:"startup"` /
   `lastCommandId:null` is a defect or an intentional "no command applied *this time*" reading.
9. `-32013 interrupted` / `-32014 cancelled` are the only two rows with no `retryable` field at all.
   Unclear whether that is deliberate (caller decides) or an omission.
10. The `workflow/*` MSP methods (#14410): params and ack are described as ratified, so the shapes
    exist somewhere in `tdd.md` SS3.19/SS3.20 but not in this binary's bundle.

---

## Verification

Adversarial re-verification, independent sandbox
`…/scratchpad/sandbox/verify-msp-protocol/` (own `drive.py`, own `HOME`, `MUSE_NO_AUTO_UPDATE=1`,
no credentials, no network). Every claim was re-driven from scratch rather than re-read.

**Headline: the report is strong on the published surface and wrong in one structural way — it
undercounts the method surface by 9 and mis-identifies the one env gate that actually matters.**

### Refuted

**R1 — `MUSE_EXPERIMENTAL_SDK_ENABLED` is the gate for the entire MSP surface.** (Refutes finding
22 and answers open question 3.) The report concludes it "does not gate `muse serve` at all —
`serve` works with the gate unset". The gate is **default-on**, so unset and `=1` are the same
state; the report only ever diffed those two. Setting it to a *falsy* value removes both `serve`
and `schema` from `muse --help` and makes them refuse:

```
$ MUSE_EXPERIMENTAL_SDK_ENABLED=0 muse serve --help
muse serve is not available in this build            # exit code 5
$ MUSE_EXPERIMENTAL_SDK_ENABLED=0 muse schema generate-json-schema --out X
muse schema is not available in this build
$ diff <(muse --help) <(MUSE_EXPERIMENTAL_SDK_ENABLED=0 muse --help)
17,18d16
<   schema           Export the MSP wire schema (JSON Schema or TypeScript)
<   serve            Serve an MSP session host over stdio
```

Accepted-truthy: `1`, `true`. Falsy/unparseable (`0`, `false`, `yes`, empty) all disable. The
report's `MUSE_EXPERIMENTAL_PLUGINS=1` "control" was blind here because PLUGINS defaults **off**
and SDK_ENABLED defaults **on** — the same diff direction cannot see a default-on gate. Practical
consequence for an oh-my project: the whole MSP integration can be switched off by one env var
(and plausibly, though unproven, by the `feature_config` / `macos_managed_preferences` /
`windows_machine_policy` enterprise planes — **INFERRED**).

**R2 — the client→server method surface is 40, not 31.** (Refutes finding 27.) The report says
`goal/*` (4), `workflow/*` (2) and `task/*` (3) "exist as runtime commands with no MSP door". All
nine are **routed MSP methods**. The report quoted the correct literal run and then probed the
wrong names (`workflow/list`, `workflow/start`, `session/setGoal` — none of which exist). Probing
with `{}` separates routing from validation: `-32601` = absent, `-32602` with a *method-specific*
deserializer message = routed.

```
goal/edit             -32602 Invalid params: goal/edit invalid params: missing field `sessionId`
goal/clear            -32602 …  goal/pause -32602 …  goal/resume -32602 …
workflow/cancel       -32602 Invalid params: workflow/cancel invalid params: missing field `sessionId`
workflow/childControl -32602 …
task/background       -32602 …  task/stop  -32602 …  task/stopAll -32602 …
workflow/list         -32601 method not found        (genuinely absent)
```

They reach the command layer, not just the parser:

```
goal/edit      {sessionId, commandId, objective}      -> -32030 rejected: missing_goal
workflow/cancel{sessionId, commandId, workflowRunId}  -> -32030 rejected: missing_run
task/stopAll   {sessionId, commandId}                 -> {"commandId":"01a05d7c-528e-7e90-8aa1-ff87a36b400f",
                                                          "status":"accepted"}      <-- SUCCESS
```

`task/stopAll` returns a clean `CommandAcceptedResult`. Confirmed programmatically: all 31
published methods route, **and** all 9 undocumented ones route. Discovered param fields:
`goal/edit{sessionId,commandId,objective}`, `task/{background,stop}{sessionId,commandId,taskId}`,
`workflow/cancel{sessionId,commandId,workflowRunId}`,
`workflow/childControl{sessionId,commandId,workflowRunId,childId,attempt:u32,action}`. The serde
field run in the binary confirms `taskId`, `workflowRunId`, `childId`, `attempt`, `action`,
`subagentId`, `objective` sitting immediately after `struct UserShellParams with 3 elements`.

**R3 — `turn/start.providerRequestOptions` is NOT silently accepted; it is gated.** (Refutes
finding 21 and the summary's "the host silently accepts off-schema fields like
`turn/start.providerRequestOptions`".)

```
turn/start {... "providerRequestOptions":{"foo":1}}          (experimentalApi absent)
  -> {"code":-32602,"message":"turn/start.providerRequestOptions requires experimentalApi capability",
      "data":{"kind":"experimentalRequired","retryable":false,"descriptor":"turn/start.providerRequestOptions"}}
same call with experimentalApi:true
  -> {"commandId":"01a05d7d-3289-…","status":"accepted","turnId":"…","disposition":"started"}
     followed by turn/started, item/completed, turn/completed
```

It is a real serde member of `TurnStartParams` (binary field run:
`TurnStartParams commandId ifBusy displayText reasoningEffort providerRequestOptions`), not an
ignored extra. What *is* silently ignored is a genuinely unknown member — `{"zzzUnknownMember":
{"foo":1}}` was accepted and the turn ran. So the report's *conclusion* ("decode permissively",
"the schema is not a complete description of the wire") survives, but its stated mechanism is
wrong in both directions: the named field is enforced, and arbitrary fields are ignored.

**R4 — `experimentalApi` gates at least two live things, not one.** (Refutes finding 20 and §8.1's
"The **only** live instance I found in 1.0.1".) The two descriptors sit adjacent in the binary:
`turn/start.input.mention` and `turn/start.providerRequestOptions`. They behave differently —
`mention` is gated *and* unimplemented ("reserved and not implemented"), while
`providerRequestOptions` is gated *and fully functional*. So the opt-in does unlock real
capability in 1.0.1, contrary to the report's "no method becomes available under the opt-in"
framing (true for methods, false for the surface as a whole).

### Corrections

**C1 — finding 33's evidence is not reproducible as written.** `SessionSetModelParams` requires a
**nested** `model: ModelSelection`, not flat `modelId`/`providerId`. The report's literal request
returns `-32602 "session/setModel invalid params: missing field \`model\`"`. With the correct shape
the report's *outcomes* do reproduce exactly, plus a distinction it missed:

```
{"model":{"modelId":"fake-model-1","providerId":"meta"}} -> -32030 reason "unsupported_route"
{"model":{"modelId":"echo","providerId":"echo"}}         -> -32030 reason "unsupported_route"
{"model":{"modelId":"llama-4"}}   (no providerId)        -> -32030 reason "invalid_target"
{"model":{"modelId":""}}                                 -> -32030 reason "invalid_target"
```

`invalid_target` is about a missing/empty routing target; `unsupported_route` is about a target the
empty catalog cannot satisfy.

**C2 — finding 6's tally is arithmetically impossible.** "Fired 200 `session/userShell` commands …
{-32031: 390, -32001: 6, 'ok': 1, -32030: 3}" sums to 400 responses for 200 requests. My run:
exactly 200 responses for 200 commands — `{-32031: 194, -32001: 2, 'ok': 1, -32030: 3}`. **The
substantive claim is solid**: `{"code":-32031,"message":"host command admission capacity 4 is
exhausted","data":{"kind":"backpressured","retryable":true,"capacity":4}}`.

**C3 — finding 18 conflates two things.** `--no-session-log` does **not** revoke `userShell`. The
report's `"grantedCapabilities":[]` is an artifact of its probe not requesting the capability:

```
ephemeral  requestedCapabilities=[]           -> granted=[]            durability=ephemeral
ephemeral  requestedCapabilities=[userShell]  -> granted=[userShell]   durability=ephemeral
durable    requestedCapabilities=[]           -> granted=[]            durability=durable
```

`sessionDurability:"ephemeral"`, `path:""` and `viewCursor:"pending:seam-c-session-view-fold"` all
reproduce exactly.

**C4 — `session/closed` is not "emitted by the host".** The executive summary says "two unpublished
ones the host actually emits — `session/started` and `session/closed`". Only `session/started` is
proven emitted. I could not trigger `session/closed` under 12 concurrent loaded sessions,
self-resume of a loaded session, or `view/unsubscribe`. Finding 5 and open question 1 state this
correctly; the summary overclaims. Note also that opting a name out proves nothing about
registration on its own — unknown names are accepted and ignored (I opted out `totally/bogus`
without error); the argument works for `session/started` only because it was *observed, then
suppressed*.

**C5 — "23 server→client notifications" is 22.** One of the 23 published rows, `initialized`, is
client→server ("Client-to-server notification closing the handshake"). The report's own §5 table
lists it correctly; the summary and finding 3 miscount the direction.

**C6 — finding 28 understates enforcement.** String client ids work (`"str-id-1"` answered
normally), confirming the integer/string split is unenforced. But the host *does* validate the id
type: `id: 3.5` and `id: null` both return
`{"code":-32600,"message":"request id must be a string or integer","data":{"kind":"invalidRequest"}}`.

### Open questions answered

**OQ7 — the `-32002 inputTooLarge` bound is 10 MiB on the whole inbound frame.** The report says
"a 6 MB text part … was accepted without error, so the bound is above that or is enforced somewhere
I did not reach."

```
 1 MB text part -> accepted
 6 MB text part -> accepted
24 MB text part -> {"code":-32002,"message":"frame exceeds the inbound limit",
                    "data":{"kind":"inputTooLarge","retryable":false,"limitBytes":10485760}}
64 MB text part -> same
```

**10,485,760 bytes, applied to the frame, with the bound published in `data.limitBytes`.** Any
driver batching attachments or large prompts must chunk below this.

**OQ3 — see R1.**

### Ground the report missed

1. **The 9 undocumented routed methods (R2)** — the single biggest omission, and directly relevant
   to the report's own oh-my hook #15, which calls `goal/*` "a natural feature-request target"
   when it is in fact already callable.
2. **The 10 MiB frame limit (OQ7).**
3. **The on-disk materialized view projection.** `view/page`, the snapshot rung and the
   `pending:seam-c-session-view-fold` sentinel are all backed by a real on-disk fold at
   `~/.local/share/muse/sessions/.msp-view-v1/<sessionId>/`:
   `HEAD.json` + `journal-00000000.bin` + `index-00000000.bin` + `snapshot-<uuid5>.json`.
   `HEAD.json` carries `{schema_version, session_id, projection_id, generation, status:"healthy",
   latest_snapshot_id, latest_snapshot_sha256[32], source_through:{id,sequence},
   source_record_sha256[32]}`. This is the mechanism behind every cursor claim in §5.2.
4. **`session-index.db` — a SQLite session index with a dedicated MSP projection.** 33 columns
   including ten `msp_*` ones (`msp_created_at_us`, `msp_updated_at_us`, `msp_turn_count`,
   `msp_fork_source_session_id`, `msp_fork_cut_cursor`, `msp_fork_cut_explicit`,
   `msp_fork_command_id`, `msp_provider_id`, `msp_model_id`, `msp_source_fingerprint`) plus
   `idx_sessions_msp_updated`. `session/list` ordering is the SQL
   `ORDER BY msp_updated_at_us DESC, session_id ASC`, and the binary enforces
   "msp fact columns must be all present or all absent". A read-plane daemon could query this
   directly instead of spawning a second host.
5. **The lease-free read plane is now runtime-proven, not just asserted.** The report offers it as
   its highest-value hook on documentation alone. Verified: host B ran `session/list`
   (`status:"notLoaded"`), `session/read` (full result, keys
   `['history','pendingRequests','session','viewCursor']`) **and `view/page` returning 3 events**
   against a session host A held the lease on, while A kept writing successfully.
6. **`view/page` requires `limit`** (the report's §7.7 table omits it), and two validators it
   missed: `anchor:"bogusAnchor"` → `-32602 "unknown anchor \"bogusAnchor\": the v1 anchor value is
   \"latestCompaction\""`; `direction:"sideways"` → `-32602 "invalid direction \"sideways\":
   expected \"forward\" or \"backward\""`.
7. **The complete `CommandRejectionReason` enum** (20 variants, one literal run), versus the
   report's ad-hoc list: `already_terminal, command_id_conflict, invalid_target, missing_run,
   run_active, runtime_busy, deferred_start_failed, cancelled_before_completion,
   assistant_output_committed, no_restorable_submission, not_paired_interrupt, already_applied,
   subagent_state_ineligible, not_ready, stale_attempt, not_cancellable, policy_rejected,
   post_run_start_backlog_overflow, missing_goal, invalid_goal_state`.
8. **`turn/start` accepts `reasoningEffort`** (real serde member, closed `ReasoningEffort` enum) —
   absent from the report's §7.5.
9. **`session/resume` accepts `history:"auto"` explicitly** (→ `inline` here), and a bogus rung is
   a clean `-32602 "invalid session/resume history: bogusRung"`.
10. **`-32020 sessionNotFound`** shape, never exercised in the report:
    `{"code":-32020,"message":"session <id> was not found","data":{"kind":"sessionNotFound",
    "retryable":false,"sessionId":"<id>"}}`.

### Reproduced exactly (no change needed)

Findings 1, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 19, 23, 24, 25, 29, 30, 31, 32, 34,
35 all reproduced verbatim, several with tighter evidence than the report gives:

- Fingerprints, byte-identical stable/experimental exports, 185/31/23/29 counts, 11 closed / 34
  open enums, `x-msp-openness` as the only surviving `x-` key, the 3-row `reserved` block, and
  `-32013`/`-32014` as the only rows with no `retryable` — all exact.
- **Protected set (finding 11)**: opting out all 25 known names *plus* `totally/bogus` yielded
  **zero** notifications through `session/start` and a full `session/userShell` approval, then
  exactly one on decide — `approval/resolved`. `approval/listPending` returned the pending approval
  (`keys: ['approvals','userInputs']`) as the pull fallback.
- **Crash recovery (finding 13)**: reproduced to the letter, including the UUID version transition
  — original approval `0282fb74-dfe1-4e86-…` (nibble `4`) → re-opened `091f57a6-25c2-526c-…`
  (nibble `5`), with `turn/completed reason:"resume_reconcile:orphaned_by_process_loss"`.
- **Multi-stage approvals (finding 14)**: 2 stages, `suggestedPrefix` labels, `-32053`/`-32052`,
  `terminal:false` → `stageResolved` → `terminal:true` — all exact.
- **`session/setApprovalMode` noop defect (finding 30)**: reproduced across three modes. Not a
  one-off; every repeat of an already-active mode reports `source:"startup", lastCommandId:null`.
  Given `EffectiveApprovalModeState.lastCommandId` is documented as "the command that last set it",
  this is a **defect**, not an intentional reading.
- Backward `view/page` really does return ascending events with `nextCursor` = the *first* event's
  cursor; forward/backward paging round-trips cleanly over 9 events.
- `Item` has **58** properties (report: "~60"), required `[itemId, kind, revision, status]`.

### Verdict

**MOSTLY_SOLID.** Everything the report drove at runtime against the *published* surface is
accurate and reproducible, and the crash-recovery, approval-stage and protected-set work is
genuinely excellent. The failures cluster in exactly the two places the report was least
empirical: it inferred a negative from a probe of names it made up (R2), and it tested an env gate
only in the direction that could not reveal it (R1). Both are load-bearing for an oh-my project —
one changes the callable surface by 29%, the other is a single env var that can turn the whole
integration off.
