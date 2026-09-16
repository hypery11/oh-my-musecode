# Five loose ends — settled

**Subject:** Meta Muse Code `1.0.1-R2006.1`, build `e27e408b66`, `muse-aarch64-macos` (+ the shipped
`muse-aarch64-windows.exe` and `muse.pkg`, both downloaded and checksum-verified against
`scratchpad/manifest.json`).
**Work dir:** `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/loose-ends/`
**Rules honoured:** no login, no auth, no prompt ever sent to Meta. Every model call in this report went to
`127.0.0.1`. Nothing was written under `/Library`. Two shell aliases of note are recorded in §0.

| # | Question | Verdict |
|---|---|---|
| 1 | Exit code for an unknown top-level command | **RESOLVED** — the premise is wrong; there is no such thing |
| 2 | The `personal_project` memory-scope root | **RESOLVED** — exact path *and* naming algorithm recovered |
| 3 | Enterprise `system_file` on-disk path | **PARTIAL** — root proven `/Library/Application Support`; leaf name not recoverable without write access there. Policy-shipping question fully answered. |
| 4 | Which 2 of 6 agent slots survive `safe_mode` | **RESOLVED** — `managed` + `built_in`, by exhaustive elimination |
| 5 | Is Windows real? | **RESOLVED** — yes, and it is a *different product surface*, not a port |

---

## 0. Two methodology traps that bit this session

**Trap A — the shell's `IFS` is not the default.** In this environment `IFS` is `$' \t\n\0'`-ish and
`for c in "skills list"; do "$M" $c; done` passes **one** argv token `skills list`. That turns every
loop-driven CLI sweep into a sweep of *unknown top-level tokens*, which silently fall through to the
TUI. This produced a completely wrong settings-matrix on the first pass. **Every script in this report
sets `IFS=$' \t\n'` explicitly and builds argv as a bash array.**

**Trap B — a redirected `HOME` does not sandbox muse.** `muse` composes
`~/Library/Application Support/Muse/session-name-authority/` from the **real** user home
(`getpwuid`, not `$HOME`), and writes there on essentially every run:

```
$ stat -f '%Sm %z %N' -t '%Y-%m-%d %H:%M:%S' "$HOME/Library/Application Support/Muse/session-name-authority/session-names.db"
2026-09-02 02:48:44 1617920 /Users/cph/Library/Application Support/Muse/session-name-authority/session-names.db
```
That mtime is from sandboxed runs made minutes earlier with `HOME`, `XDG_CONFIG_HOME` and
`XDG_DATA_HOME` all redirected into the scratchpad. Any prior pass that assumed a fake `HOME` is a
complete sandbox was wrong about this one directory. `authority.json` + a growing
`session-names.db` (1.6 MB) accumulate there permanently.

---

## 1. Exit codes

### 1.1 The headline question dissolves: there is no "unknown top-level command"

`muse --help` says so itself:

```
Usage: muse [OPTIONS] [PROMPT]
       muse [OPTIONS] <COMMAND>
```

An unrecognised first token is not an error — it is the **`[PROMPT]` positional**, and it launches the
interactive TUI with that string already submitted. Proven under a real pty
(`loose-ends/q1-exit/ptyrun.py`, VT100-rendered with `sandbox/tui/render.py`):

```
$ MUSEBIN=…/muse-aarch64-macos SBHOME=…/q1-exit/home SBWS=…/q1-exit/ws \
  PTY_SCRIPT='wait:1|send:2|wait:0.4|send:\r|wait:2' \
  python3 …/q1-exit/ptyrun.py out.bin --provider echo zzznotacommand
$ python3 …/sandbox/tui/render.py out.bin
  Muse Code 1.0.1
⟩ zzznotacommand
◆ echo: zzznotacommand
⟩ 2
◆ echo: 2
── Voice input (⌥ + v to start) ──────────────────────────────────────────
```

`zzznotacommand` was **sent to the model as the first user turn.** On a first run in an untrusted
workspace the same invocation renders the trust prompt first:

```
Do you trust this workspace?
Workspace: …/q1-exit/ws
> 1  Trust and continue
  2  Quit
```

**Design consequence: a typo in a wrapper does not fail — it silently starts a paid session and sends
the typo to the model.** `omm` must validate the first token against the 16 known commands *before*
exec'ing muse.

### 1.2 Why the first pass saw both 0 and 1

On a non-tty the exit code depends on **workspace trust**, not on the token (fresh sandbox,
`loose-ends/q1b/`):

```
=== FRESH sandbox, NO trust.json ===
  stdin=/dev/null      rc=1   err=Device not configured (os error 6)
  stdin=/dev/zero      rc=1   err=Device not configured (os error 6)
  stdin=pipe           rc=1   err=Device not configured (os error 6)
  stdin=inherited      rc=1   err=Device not configured (os error 6)
=== now WITH trust.json (trusted) ===
  stdin=/dev/null      rc=0   err=
  stdin=pipe           rc=0   err=
  stdin=inherited      rc=0   err=
=== bare `muse` with no token at all, trusted ===
  bare, stdin=/dev/null   rc=0
```

Untrusted ⇒ the TUI must draw the trust prompt ⇒ needs a terminal ⇒ `Device not configured (os error 6)`
⇒ **1**. Trusted ⇒ terminal init exits cleanly ⇒ **0**. Bare `muse` behaves identically, which is the
proof that the token is irrelevant. Under a real pty a clean TUI quit (`/exit` or Ctrl-D) is **0**.

### 1.3 The canonical matrix

Repro: `bash …/settle/loose-ends/q1-exit/exitcodes.sh`

```
RC   CASE                                           FIRST STDERR LINE
0    muse --version
0    muse --help
0    muse zzznotacommand   (no tty, trusted ws)
0    muse                  (no tty, trusted ws)
0    muse zzznotacommand --help                     ← --help wins over the PROMPT positional
2    muse --zzznotaflag                             invalid TUI options: error: unexpected argument '--zzznotaflag'
2    muse skills zzz                                unknown skills command `zzz`
2    muse schema zzz                                muse schema: unknown subcommand `zzz`
2    muse trace zzz                                 error: unsupported trace subcommand `zzz`; expected `inspect`
2    muse sandbox zzz                               unknown sandbox command `zzz`
2    muse serve zzz                                 muse serve: unexpected argument zzz; serve takes no positional arguments
2    muse export zzz                                unexpected export argument: zzz
2    muse auth zzz                                  expected `auth set`
2    muse session-message zzz                       usage: muse session-message <command> [options]
2    muse workflows zzz                             usage: muse workflows list
2    muse config zzz                                Usage: muse config validate --plane <defaults|policy> --file <path>
2    muse plugins list        (ungated)             plugins are not available in this build
0    muse plugins list        (gated)
0    muse skills list         (ok)
1    muse sandbox windows check (macOS)             Windows elevated sandbox setup has not run
1    muse session-message list (ungated)            external agent ingress is unavailable
2    MUSE_ENABLE_WEB_TOOLS=bogus --version          MUSE_ENABLE_WEB_TOOLS must be one of 1,true,on,yes,0,false,off,no
2    MUSE_WEB_SEARCH_MODE=bogus --version           MUSE_WEB_SEARCH_MODE must be one of client,hosted,off
0    bad settings.json + --version
1    bad settings.json + skills list                malformed settings file at …
1    bad settings.json + exec                       malformed settings file at …
0    bad settings.json + workflows list
1    schema_version:2 + skills list                 unsupported settings schema version 2 at …
```

**Rules that hold:**

* **2 = a parser rejected the argv.** Uniform across all five parser dialects, for unknown *flags*
  and unknown *subcommands of a known command* alike.
* **2 = a gated command invoked without its gate** (`plugins` → `plugins are not available in this build`).
  Same code as a parse error, so a wrapper cannot distinguish "typo" from "needs a gate" by exit code.
* **1 = the argv parsed but the run failed** (missing gate at runtime, missing Windows setup,
  malformed settings, hook block, `init` over an existing `AGENTS.md`).
* **2 = an unparseable env enum**, and it is checked before everything — it kills `muse --version`.
* **0 or 1 for an unknown top-level token**, decided by workspace trust. **Never build error detection on it.**

### 1.4 Correction: malformed `settings.json` is *not* a hard startup failure

The synthesis records `settings.load` as a startup step whose failure is fatal. It is a **lazy load**.
With `settings.json` = `NOTJSON{{{`:

| command | rc | behaviour |
|---|---|---|
| `--version`, `--help` | 0 | never reads settings |
| `export`, `init` | 0 / 1 | unaffected by the bad file |
| `workflows list` | **0** | **unaffected — reads no settings** |
| `schema zzz`, `trace inspect`, `config validate`, `auth set` | 2 | parse layer, reached first |
| `sandbox windows check` | 1 | its own runtime error, not the settings error |
| `skills list`, `skills list --json`, `session-message list`, `exec`, the TUI | **1** | `malformed settings file at <path>` |

`{"schema_version":2}` → `unsupported settings schema version 2 at <path>`, also **1**, same set of
commands. `{}` and `{"schema_version":"1"}` → `malformed settings file` (the field is a hard-required
`u32`).

---

## 2. `personal_project` — RESOLVED, path and algorithm

### 2.1 How it was exercised without authenticating

The echo provider never emits tool calls, and MSP has no tool-invocation method, so the tool could not
be driven from any existing lane. The route that worked: **`endpoint_transport.base_url` pointed at a
local HTTP server**, with a fake `META_API_KEY`. No Meta endpoint was contacted.

`loose-ends/q2-memory/srv/srv.py` answers `GET …/muse-code/models` from `catalog.json` and every
`POST /responses` with a canned SSE body.

Three things had to be reverse-engineered to make muse accept a synthetic stream — none of them are in
any prior report:

1. **`TBH_PROVIDER_TRACE_ROOT=<dir>`** makes muse dump a per-stream JSON diagnostic
   (`meta-stream-<ms>-<pid>-<n>.json`) with `failure_kind`, `wire_events_seen`,
   `last_wire_event_type` and redacted `tail_frames`. This is the oracle that made the whole thing
   tractable. Failure ladder observed: `transport_eof_before_terminal_event` (unknown `type` is
   silently ignored) → `provider_stream_decode_error` → `provider_event_translation_error` → accepted.
2. **`sequence_number` is required on every SSE event.** Without it every well-typed frame fails
   `provider_stream_decode_error`. This is the single field that blocked three earlier attempts.
3. The stream is OpenAI-Responses-shaped with `data:`-only frames and a `data: [DONE]` terminator.

Minimal accepted stream (`loose-ends/q2-memory/srv/sse.txt`), four frames:

```
data: {"type":"response.output_item.added","sequence_number":1,"output_index":0,"item":<ITEM>}

data: {"type":"response.function_call_arguments.done","sequence_number":2,"item_id":"fc_1","output_index":0,"arguments":<ARGS>}

data: {"type":"response.output_item.done","sequence_number":3,"output_index":0,"item":<ITEM>}

data: {"type":"response.completed","sequence_number":4,"response":{"id":"r1","object":"response","model":"M","status":"completed","created_at":0,"completed_at":1,"output":[<ITEM>],"usage":{"input_tokens":1,"output_tokens":1,"total_tokens":2}}}

data: [DONE]
```
with
```
<ITEM> = {"type":"function_call","id":"fc_1","call_id":"call_1","name":"add_memory","arguments":<ARGS>,"status":"completed"}
<ARGS> = "{\"scope\":\"personal_project\",\"path\":\"omm-probe.md\",\"content\":\"# OMM PROBE MARKER 4f3a9c\\n\"}"
```

### 2.2 The answer

```
$XDG_DATA_HOME/muse/memory/projects/<slug>-<fnv1a64hex>/
```

Observed, first write:

```
…/q2-memory/home/.local/share/muse/memory/projects/
  private-tmp-claude-501--Volumes-OWC-Envoy-Ultra-omm-121b85f3-1a9e-4c0e-af22-0c459424aa0f-scratch-69e6e251dabfe971/
    omm-probe.md
    .muse-memory.lock
```

**Naming algorithm, fully determined** (four workspaces, `loose-ends/q2-memory`):

| workspace (canonicalised) | directory name |
|---|---|
| `/private/tmp/omm-w1` | `private-tmp-omm-w1-cb608784b444ce8a` |
| `/private/tmp/omm/w2` | `private-tmp-omm-w2-bcf99c84acf0b0a9` |
| `/private/tmp/UPPER_Case+weird#chars` | `private-tmp-UPPER-Case-weird-chars-013792409fd16a19` |
| `/private/tmp/has space.and.dots` | `private-tmp-has-space-and-dots-49eb1ace9a5112dd` |
| `/private/tmp/` + `'a'*84` | slug truncated to 96, `…-7a977e5cb86f0173` |
| `/private/tmp/` + `'a'*85` | slug truncated to 96 (identical), `…-a41238a33aafa120` |

1. Canonicalise the workspace root (`/tmp/x` → `/private/tmp/x` on macOS).
2. Drop the leading `/`; replace every character outside `[A-Za-z0-9]` with `-`. **Case is preserved.
   No run-collapsing** (`/-Volumes` → `--Volumes`).
3. **Truncate the slug to exactly 96 characters.**
4. Append `-` + **FNV-1a 64-bit of the full canonical path**, `%016x` lowercase.

The hash was identified by brute force against three independent samples; FNV-1a-64 over the raw path
bytes is the *only* candidate that matched all three (sha256/sha1/md5/blake2b/blake2s/sha3, in prefix,
suffix and byte-swapped forms, over `path`, `path+"/"` and `path.lower()`, all missed):

```
/private/tmp/omm-w1 -> want cb608784b444ce8a
  sha256   911c276cd89aa986
  sha1     858d841b3ff61f8d
  md5      95423d89327bf9a6
  blake2b  79dba9cbff2e3303
  blake2s  bd986afd22ce6898
  fnv1a64  cb608784b444ce8a     ← match, and matches the other two samples
```

The 84-char vs 85-char pair is the proof that truncation is real and that the hash is what
disambiguates two workspaces that share a 96-char prefix. Note `/private/tmp/omm-w1` and
`/private/tmp/omm/w2` both slugify to `private-tmp-omm-w?`, i.e. **the slug alone genuinely collides**
and the hash is load-bearing.

### 2.2b Algorithm confirmed by prediction

`settle/loose-ends/q2-memory/REPRO.sh` is self-contained: it builds a fresh sandbox, starts the local
`/responses` server on `127.0.0.1:8477`, drives one `add_memory` call, then *predicts* the directory
name from the algorithm and prints both. Run from scratch:

```
$ bash .../settle/loose-ends/q2-memory/REPRO.sh
--- personal_project root ---
/home/.local/share/muse/memory/projects/private-tmp-claude-501--Volumes-OWC-Envoy-Ultra-omm-121b85f3-1a9e-4c0e-af22-0c459424aa0f-scratch-8a83968bb8223d26/probe.md
/home/.local/share/muse/memory/projects/private-tmp-claude-501--Volumes-OWC-Envoy-Ultra-omm-121b85f3-1a9e-4c0e-af22-0c459424aa0f-scratch-8a83968bb8223d26/.muse-memory.lock
PREDICTED dir: private-tmp-claude-501--Volumes-OWC-Envoy-Ultra-omm-121b85f3-1a9e-4c0e-af22-0c459424aa0f-scratch-8a83968bb8223d26
```

Predicted == actual, on a workspace path never used in the derivation. The reference implementation is
20 lines:

```python
def personal_project_dir(workspace_root):
    ws = os.path.realpath(workspace_root)
    slug = "".join(c if (c.isalnum() and c.isascii()) else "-" for c in ws.lstrip("/"))[:96]
    h = 0xcbf29ce484222325
    for b in ws.encode():
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return "%s-%016x" % (slug, h)
```

Each root also carries a `.muse-memory.lock`.

### 2.3 The two hypotheses, separated

The synthesis offered: (H1) the root exists but no one guessed the path; (H2) the renderer never emits
a `personal_project` section, so path-scanning says nothing.

**H1 is correct. H2 is refuted.** Once a `MEMORY.md` exists in the real root, the section renders —
and it renders *first*, above `personal`. From the captured `POST /responses` body:

```
<system-reminder source="memory-snapshot">
Memory snapshot for this run. This snapshot is fixed for this run and may be stale after memory writes.
Use read_memory for live memory or topic details.

## Memory scope: personal_project
MEMORY.md:
# Project memory
PERSONAL_PROJECT_SENTINEL_9x7
Other Markdown files:
- extra-note.md
- omm-probe.md

## Memory scope: personal
MEMORY.md:
# Personal
PERSONAL_SENTINEL
</system-reminder>
```

The ~73 earlier candidate paths missed because they tried `basename`, `sha256[:8/16]`, base64 and
`/`→`_` slugs — never *full-path slug truncated to 96 + FNV-1a-64*. The `memory_snapshot` block is
also **trust-gated**: on an untrusted workspace it is not emitted at all, which is a second reason the
offline scans saw nothing.

---

## 3. Enterprise `system_file` — root proven, leaf not recoverable here

### 3.1 The observable nobody used: `muse config status`

`muse config --help` advertises two verbs, and prior passes only used the first:

```
Usage: muse config validate --plane <defaults|policy> --file <path>
       muse config status
```

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
rc=0. **Four rows, not six** — `windows_machine_policy` is compiled in (`EnterpriseSourceClass` has
all three) but not enumerated on macOS. There is no `--json`. `config status` writes no local-tracing
record, so this command *is* the only enterprise oracle.

### 3.2 The root is `/Library/Application Support` — proven without writing anything

A `sandbox-exec` deny-rule flips the state, which is a read-only existence oracle:

```
$ cat deny.sb
(version 1)
(allow default)
(deny file-read* (subpath "/Library/Application Support"))

$ sandbox-exec -f deny.sb muse config status
enterprise_status_source_rejected: code=acquisition_failed source_class=system_file plane=defaults \
  state=unreadable remediation=run muse config validate for the reported plane, repair the \
  administrator source, and retry
```

Decomposition (each row a separate profile):

| profile | state |
|---|---|
| deny `file-read*` subpath `/Library/Application Support` | **unreadable** |
| deny `file-read-data` literal `/Library/Application Support` | **unreadable** |
| deny `file-read-metadata` literal `/Library/Application Support` | **unreadable** |
| deny `file-read*` subpath `/Library` | **unreadable** |
| deny `file-read*` subpath `/Library/Managed Preferences` | absent (unchanged) |
| deny subpath `/Library/Application Support` **+ allow `file-read*` literal on it** | **absent** (restored) |
| deny `file-read-data` under it, allow data *on* it | **absent** |

So the read that matters is on `/Library/Application Support` **itself** — muse needs read-data (i.e.
a directory enumeration or an `openat` through it) and never needs a child successfully.

This also kills `~/Library/...`: a 25-path sweep of every `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`/cwd
candidate (including `$HOME/Library/Application Support/Muse`, `.../tbh`, `.../com.tbh.tbh`,
`$XDG_CONFIG_HOME/muse`, `$HOME/.muse`, `<ws>/.muse`) with both filenames planted produced **zero**
state changes. There is no env override either — no `MUSE_*`/`TBH_*` variable in the binary matches
`ENTERPRISE|MANAGED|POLICY|DEFAULTS|SYSTEM` except `TBH_MANAGED_HOOKS_PATH`.

### 3.3 Why the leaf component could not be recovered

**Static:** the two filenames are a bare adjacent literal pair with no directory attached —
`…a Display implementation returned an error unexpectedly` `Error` `enterprise-defaults.json`
`enterprise-policy.json` `token response missing access_token…` at offset **196741437**. The directory
components are pooled separately at offset **196721291**, two NUL-free constants back to back:

```
00000020  11 3e 02 21 00 00 58 5b  52 55 20 20 4c 69 62 72  |.>.!..X[RU  Libr|
00000030  61 72 79 41 70 70 6c 69  63 61 74 69 6f 6e 20 53  |aryApplication S|
00000040  75 70 70 6f 72 74 00 00                           |upport..|
```
i.e. `"Library"` + `"Application Support"` as *components*, in the `tbh_config` pool 20 KB from
`com.tbh.tbh` and the two filenames. The only full-path literal in the binary containing them is
`Library/Application Support/Muse/session-name-authority` (offset 197608386) — a different crate, and
the one that writes into the real user home (§0, Trap B). "Muse" is available in the literal pool as a
sub-slice of that string, so `/Library/Application Support/Muse/enterprise-{defaults,policy}.json` is
the **strongest inference** — but it is an inference, not a proof.

**Dynamic:** seatbelt cannot bisect it, because **path filters do not match nonexistent paths**.
Control:

```
$ sandbox-exec -f ctl.sb /bin/cat "/Library/Application Support/zzznotexist/f.json"
cat: /Library/Application Support/zzznotexist/f.json: No such file or directory     ← ENOENT wins
$ sandbox-exec -f ctl.sb /bin/ls  "/Library/Application Support/Apple"
ls: /Library/Application Support/Apple: Operation not permitted                     ← EPERM (exists)
```
So the eight-candidate allow-list bisection (`Muse`, `muse`, `MuseCode`, `Muse Code`, `tbh`,
`com.tbh.tbh`, `Meta`, `MetaAI`) is structurally blind and its all-miss result means nothing.

**Installer:** `muse.pkg` (sha256 `57b1e5ff…`, matches the manifest) was expanded. Its BOM installs
**exactly one file** and runs **no scripts**:

```
$ lsbom -p MUGsf pkg/muse-component.pkg/Bom
drwxrwxr-x  root wheel            .
drwxr-xr-x  root wheel            ./usr
drwxr-xr-x  root wheel            ./usr/local
drwxr-xr-x  root wheel            ./usr/local/bin
-rwxr-xr-x  root wheel 505342272  ./usr/local/bin/muse
```
`identifier="ai.meta.dev.installer"`, `version="2006.1"`, `auth="root"`, `Scripts/` empty. **Nothing in
the shipped product ever creates the enterprise directory.** (Side finding: the `.pkg` installs the
raw 505 MB universal binary at `/usr/local/bin/muse`, *not* the launcher — so a pkg install has no
auto-updater.)

The macOS managed-preferences plane (domain `com.tbh.tbh`, keys `enterprise_defaults_json` /
`enterprise_policy_json`, both confirmed as adjacent literals at 196740689/196741437) was **not**
exercised: `defaults write com.tbh.tbh …` goes through `cfprefsd`, which resolves the home directory
from the user record and ignores `$HOME`, so it would have written to the real
`~/Library/Preferences/`. That is outside the scratchpad, so it was deliberately not attempted.

### 3.4 Could a team edition ship policy today? — **No for policy, yes for defaults**

This corrects the synthesis, which says "every `execution.*` field is `field_not_activated`". Two
things are wrong with that: `semantic_invalid` fires *before* the activation gate (so a badly-shaped
value masks the real answer), and the **defaults plane is a completely different story**.

**Policy plane — every field that carries content is inactive:**

```
execution.forbid_approval_bypass            field_not_activated
execution.forbid_sandbox_bypass             field_not_activated
execution.force_agent_definition_safe_mode  field_not_activated
execution.permission_profiles               field_not_activated
execution.approval_reviewers                field_not_activated
execution.allow_project_configuration       field_not_activated
execution.allow_foreign_configuration       field_not_activated
execution.approval_modes  {"allowed":[…]}                     semantic_invalid
execution.approval_modes  {"allowed":[…],"fallback":…}        field_not_activated   ← real answer
execution.network_sandbox_modes {"allowed":[…],"fallback":…}  field_not_activated
execution.stop_hook_continuations {"maximum":N}               semantic_invalid  (0 and 10 both)
execution.tool_rules {}                                       VALID   (empty ⇒ nothing to activate)
execution.tool_rules {"bash":{"decision":"deny"}}             field_not_activated   ← real answer
model_egress.allowed_providers / allowed_models / model_fallback  semantic_invalid
model_egress.web_search                                       unknown_member
extensions.skills / hooks / runtime_capabilities              field_not_activated
privacy.telemetry / foreign_personal_rules  (bool and {"force_off":true})  wrong_type
local_session_messaging.receiver_limits {}                    VALID   (empty)
```
And every *empty* container validates: `{"schema_version":1}`, `{"execution":{}}`, `{"model_egress":{}}`,
`{"extensions":{}}`, `{"privacy":{}}` are all `valid: plane=policy schema_version=1`.

**Conclusion: the only policy document this build accepts is one that expresses nothing.**

**Defaults plane — largely live.** Real values validate:

```
settings.reasoning_effort:"low"                        valid
settings.provider:"meta"                               valid
settings.model:"x"                                     valid
settings.run.workflow_trigger_mode:"off"               valid
settings.agents.execution_capacity:4                   valid
settings.context.foreign_personal_rules:false          valid
settings.telemetry.enabled:false                       valid
settings.mcp_servers:{} / settings.presets:{}          valid
settings.tui.theme:"x"                                 field_not_activated   ← the one exception found
```

So an enterprise could ship org **defaults** — model, effort, workflow trigger mode, telemetry,
foreign-rules privacy, MCP servers, presets — but **no restrictions of any kind**. And they still
cannot deliver the file, because the leaf directory under `/Library/Application Support` is unknown
and nothing in the shipped installer creates it. **A team edition is not shippable today.**

Repro: `muse config validate --plane policy|defaults --file <doc>` in
`loose-ends/q3-enterprise/docs/`.

---

## 4. `safe_mode` keeps `managed` + `built_in` — RESOLVED

Harness: `loose-ends/q4-safemode/run.sh <label> [extra muse args]` — a pty run (composition is
TUI-only) that clears `local-tracing/bootstrap` first and then greps the one line that matters.

### 4.1 Baseline: each source's contribution, `safe_mode` off

```
safe=off clean                       sources=6 loaded_sources=6 suppressed_sources=0 candidates=2
safe=off +user(1)                    …                                                candidates=3
safe=off +user+project(2)            …                                                candidates=4
safe=off +user+project+session(3)    …                                                candidates=5
```
(user = `$CONFIG/muse/agents/u1.md`, project = `<ws>/.agents/agents/p1.md`,
session = `--agents '{"ss-gamma":{…}}'`.)

### 4.2 `safe_mode: true` — all four contributions vanish, count is structural

```
safe=ON  +user+project+session   sources=6 loaded_sources=2 suppressed_sources=4 candidates=2
safe=ON  +user+project           …                                              candidates=2
safe=ON  user only               …                                              candidates=2
safe=ON  project only            …                                              candidates=2
safe=ON  session only            …                                              candidates=2
safe=ON  nothing                 …                                              candidates=2
```
`loaded_sources=2 suppressed_sources=4` never moves, so the suppressed set is fixed at compile time,
not derived from what happens to be populated.

### 4.3 The session slot is not merely emptied — it is never loaded

A malformed overlay is the oracle. `safe_mode` off, the overlay *fails*; `safe_mode` on, there is
nothing to fail:

```
safe=off badoverlay   --agents '{"ok":{},"BAD":{}}'   source_failed=1 candidates=4 diagnostics=1
safe=ON  badoverlay   --agents '{"ok":{},"BAD":{}}'   source_failed=0 candidates=2 diagnostics=0
```

### 4.4 The plugin slot — the last one needed for the elimination

Built a native plugin `adplug` with a top-level `agents/pl-delta.md` (the implicit, undeclared
Agent-Definition capability):

```
$ MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins install ./adplug --json
  "warning": "third-party plugin: Agent Definitions require review before activation"
$ MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins approve adplug --json
  {"decision":"approve","runtime_capabilities":[
     {"kind":"agent_definition","plugin_id":"adplug","scoped_definition_id":"adplug/pl-delta",
      "original_ordinal":0,"enabled":true}]}
```
(persisted as `plugin:adplug:agent_definition:agent-resource:b4dbdf11…`). Then, with user and project
sources removed so the delta is unambiguous:

```
gate=ON  safe=off plugin approved   loaded=6 suppressed=0 candidates=3   ← plugin contributes +1
gate=OFF safe=off plugin approved   loaded=6 suppressed=0 candidates=3   ← see §4.6
gate=ON  safe=off plugin DISABLED   loaded=6 suppressed=0 candidates=2   ← control
gate=ON  safe=ON  plugin approved   loaded=2 suppressed=4 candidates=2   ← suppressed
```

### 4.5 The conclusion

`DefinitionSource` = `managed session project user plugin built_in` (+ the `fixed` marker and the
`project_depth`/`plugin_id` qualifiers, which are not slots).

* `built_in` **survives** — the 2 surviving candidates under safe mode are exactly the two built-ins.
* `session`, `project`, `user`, `plugin` are **each proven suppressed** by loss of their candidate
  contribution (and, for `session`, by loss of its failure too).
* That is 4 of 4. `suppressed_sources=4` is exhausted.

**⇒ `loaded_sources = 2 = { managed, built_in }`.** Enterprise-managed definitions and the built-ins
are the only agent sources that survive `safe_mode`. This is semantically coherent: safe mode is
itself intended to be enterprise-forced (`execution.force_agent_definition_safe_mode`), so it would be
incoherent for it to suppress the enterprise's own definitions.

**Design consequence for oh-my-musecode: an agent pack ships as `user`, `project`, or `plugin`
definitions, and all three are dead on a `safe_mode` fleet.** There is no way for a third party to
reach the two surviving slots — `built_in` is compiled in and `managed` needs `/Library`. `omm doctor`
should read `agent_definitions.safe_mode` and say plainly that the agent pack is inert.

### 4.6 Side finding: the plugin *runtime* is not gated, only the plugin *CLI* is

Row 2 of §4.4: with `MUSE_EXPERIMENTAL_PLUGINS` **unset**, the approved plugin's agent definition
still composed (`candidates=3`). The gate covers `muse plugins …` as a command surface; installed
plugins participate in session composition regardless. A framework can therefore install once with the
gate and ship a runtime that never sets it.

---

## 5. Windows — real, maintained, and a *different tool surface*

### 5.1 The binary is genuine

```
$ curl -sSL -o muse-aarch64-windows.exe "https://lookaside.facebook.com/lookaside/muse/download/?channel=muse&version=1.0.1-R2006.1&file=muse-aarch64-windows.exe"
$ shasum -a 256 muse-aarch64-windows.exe
81715374e9f721c721a36c7bd24b513a6f3fde3fbd970e06e39ff277d499f180   ← matches manifest.json exactly
$ file muse-aarch64-windows.exe
PE32+ executable (console) Aarch64, for MS Windows
```
Built from the same tree and version: `1.0.1-R2006.1` and `e27e408b66` are both present. Rust
toolchain `1.97.1`, source paths `xplat\rust\toolchain\sysroot\…\std\src\sys\paths\windows.rs`.

It could **not** be executed — no `wine`, `wine64`, `qemu-aarch64` or `box64` on this host, and no
Windows machine. Everything below is from strings plus differential comparison against the macOS build.

### 5.2 Feature parity is complete

| literal | macOS | Windows |
|---|---|---|
| `agent_definition.sources_load` | 1 | 1 |
| `plugins are not available in this build` | 2 | 2 |
| `MUSE_EXPERIMENTAL_PLUGINS` | 1 | 1 |
| `enterprise-defaults.json` | 1 | 1 |
| `system_file` / `windows_machine_policy` / `macos_managed_preferences` | 5/5/5 | 2/2/2 |
| `commandWindows` | 3 | 3 |
| `personal_project` | 18 | 9 |
| `Bubblewrap` | 17 | 18 |

### 5.3 It is not a stub — it is a serious native port

`muse sandbox windows check` on the Windows build reports a status line the macOS build has no
equivalent for:

```
usage: muse sandbox windows check
       muse sandbox windows setup
…
backend=… status=… reason=… runner_path=… runner_trusted=… runner_version_matches=…
sandbox_users_ready=… capabilities_ready=… private_desktop_ready=… restricte[d…]
```

That implies **dedicated sandbox user accounts, a private desktop, and restricted tokens**. Supporting
strings, all Windows-only:

* `SHGetKnownFolderPath(FOLDERID_ProgramData) failed with HRESULT 0x…`
* `SHGetKnownFolderPath(FOLDERID_LocalAppDataLow) returned a null path`
* `Windows private sandbox temp root is not a direct child of LocalAppDataLow: …`
* `protected setup path has a null DACL`, `%ProgramData root is a reparse point: …`,
  `create protected setup directory … failed: …`, `default setup root has no ProgramData ancestor`,
  `read existing integrity label failed`, `restore integrity label failed`,
  `refusing to change integrity labels for unexpected sandbox prof…`, prefix `tbh-sbx-`
* ConPTY: `ConPTY process handle lock poisoned`, `ConPTY process remained active after its handle was
  signaled`, `.COM;.EXE;.BAT;.CMD`, `PATHEXT`
* Config resolution: `no usable config directory from XDG_CONFIG_HOME, HOME, or USERPROFILE`
  (and the same for the data dir) — **`XDG_CONFIG_HOME`/`XDG_DATA_HOME` work on Windows too**.

Also recovered from the Windows build, and useful well beyond Windows — the **settings-origin
precedence chain**, which does not appear as a clean run in the macOS binary:

```
built_in . enterprise_defaults : user  project  environment  command_line  client_request
          runtime_mutation . enterprise_policy : runtime_safety_floor
```

### 5.4 The decisive answer: yes, hook scripts need a Windows path — for two reasons

**Reason 1 — the shell is different.** The Windows build **replaces the `bash` tool with a PowerShell
tool**. `powershell_input` appears **19×** in the Windows binary and **0×** in the macOS one;
`Run a Windows PowerShell command` 2× vs 0×. Tool description text, verbatim:

> By default commands are executed by `powershell.exe`, so use PowerShell syntax and environment
> variables such as `$env:NAME`, not Bash or cmd.exe syntax.

and the memory-reminder agent gets its own: *"Run a Windows PowerShell command in the memory reminder
read-only workspace. Do not use Bash or cmd.exe syntax."*

**Reason 2 — the hook contract selects per-platform, and there is no fallback diagnostic.** Diagnostic
**D63**, present *verbatim in both binaries*:

> `D63: this handler declares only a Windows command form (`commandWindows`), which is selected on
> Windows only, so it has no command to run on this platform`

Reconfirmed at runtime on macOS (`loose-ends/q5-windows/hooktest`, a `SessionStart` hook whose command
touches a marker file):

```
  command only                        marker=plain.txt        ← runs
  commandWindows only                 marker=(none)           ← dropped
  command + commandWindows            marker=both-posix.txt   ← POSIX form selected on macOS
  command + command_windows (snake)   marker=snake.txt        ← snake_case alias also accepted
```

There is **no mirror-image diagnostic** — nothing like "declares only a POSIX command form". So a bare
`command` *is* dispatched on Windows, into PowerShell, where POSIX shell syntax fails at runtime with
no static warning. `commandWindows` exists precisely to carry the PowerShell variant.

**Third constraint:** foreign (Claude-format) plugin hooks **reject** the field —
`foreign hook handler field 'commandWindows' is unsupported`. A Windows command can only be carried by
a native Muse hooks document, never by a `.claude-plugin` bundle.

**Design consequence:** every `omm` hook must ship both `command` (POSIX) and `commandWindows`
(PowerShell), authored as a native Muse hooks document. A single-form hook is either dead on Windows
(if `commandWindows`-only) or silently broken there (if `command`-only with POSIX syntax). And a
Claude-format bundle cannot express this at all — which is a real limit on the "author once in the
Claude shape, ship to three agents" thesis.

---

## 6. Failed attempts, in order

**Q1**
1. Loop-driven sweep `for c in "skills list"; do muse $c; done` — invalid, non-default `IFS` (§0).
   Produced a wrong settings matrix that made `skills list` look like it needed a terminal.
2. First `ptyrun.py`: `os.write` on a dead pty raised `OSError: [Errno 5] Input/output error` and the
   run aborted before reporting the exit status. Patched to catch it.
3. `os.waitpid` after SIGTERM hung when the TUI outlived the signal → `NO_STATUS`. Patched to a
   bounded WNOHANG poll then SIGKILL.

**Q2**
4. Looked for an env var to script the echo provider (`MUSE_*`/`TBH_*` matching
   `ECHO|FIXTURE|REPLAY|SCRIPT|MOCK|STUB|CANNED`) — only `TBH_TMUX_*_FIXTURE` exist, all TUI.
5. Looked for an MSP tool-invocation method — the 40 routed methods are session/turn/agent/goal/task
   only. `turn/start` sends a prompt; with echo the model never emits a tool call.
6. Searched the binary for standard OpenAI SSE tags (`response.output_item.added`,
   `response.completed`, `response.function_call_arguments.done`, …) — **all zero occurrences**.
   The tag values are not recoverable statically; they had to be guessed and probed.
7. First working stream attempt failed `provider_stream_decode_error` on `response.created`; dropping
   that frame moved the failure to `response.output_item.added`; the actual cause was the missing
   **`sequence_number`** on every frame. Three separate frame-shape hypotheses were burned before
   that.
8. `muse exec … "hi"` never POSTs at all — a trivial greeting is answered by a canned
   `Hi — how can I help?`. Needed a substantive prompt to get a request on the wire.
9. First memory-snapshot check showed no block: the workspace was untrusted. The `memory_snapshot`
   context block is trust-gated.

**Q3**
10. 25-path sweep of every `HOME`/`XDG_*`/cwd-relative candidate with both enterprise filenames
    planted — zero state changes. The path is not user-derived.
11. Searched the binary for absolute prefixes: `/Library/Application Support` **0**,
    `/Library/Managed Preferences` **0**, `/etc/muse` **0**, `ProgramData` **0**, `MuseCode` **0** in
    the macOS build. Only `/System/Library/Frameworks/*` and TLS/zoneinfo paths exist.
12. `sandbox-exec` allow-list bisection over 8 candidate leaf names — all "still unreadable", but the
    control proved the method blind on nonexistent paths (§3.3). **Discard that negative.**
13. `sandbox-exec` `(trace "out.sb")` — exited **71** and wrote nothing.
14. `log stream` for sandbox denials with three predicates — captured 1 line, no violation records.
15. `regex #"^/Library/Application Support/"` deny probes — all missed, because traversal is checked
    on the ancestor *without* the trailing slash. Misleading; superseded by the literal/subpath
    decomposition.
16. Public docs (`scratchpad/docs/`, `devmeta_docs.html`), `install.sh` and `muse-launcher.sh`:
    **zero** mentions of enterprise config, `/Library`, managed preferences, or either filename.
17. `defaults write com.tbh.tbh …` — **deliberately not attempted**; `cfprefsd` ignores `$HOME` and
    would write to the real `~/Library/Preferences/`.

**Q4**
18. Tried to get a per-source trace by dumping every `local-tracing` record — the whole bootstrap log
    is 16 lines and `agent_definition.sources_load` appears exactly once, with the field vocabulary
    `reason safe_mode sources loaded_sources suppressed_sources source_failed candidates diagnostics
    duration_ms`. **The trace will never name a slot.**
19. Tried to observe `untrusted_project` suppression independently (trust.json `decision: "denied"`,
    no `--trust-workspace`) → **no trace at all**; the TUI stops before composition.
20. Searched for a compiled list of safe-mode-exempt sources — the only `safe_mode` prose in the
    binary is the diagnostic-reason enum and the enterprise key registry.

**Q5**
21. No emulator on this host, so the `.exe` could not be run. Everything is static + differential.
22. `strings -a -n 6` on the PE yields 4.0 MB / 114 012 lines, and `fbcode/musecode` is **0** —
    the Windows build strips panic paths differently, so crate-path mining does not transfer.

---

## 7. What each answer changes for oh-my-musecode

1. **`omm` must validate argv itself.** An unknown first token is a *prompt*: a typo starts a session
   and bills a turn. Exit codes: **2** = argv rejected (parse error *or* missing gate — indistinguishable),
   **1** = parsed but the run failed, **0/1** = unknown token, decided by workspace trust. Put the
   §1.3 matrix in `omm doctor --self-test`.
2. **`omm memory` has a real target.**
   `$XDG_DATA_HOME/muse/memory/projects/<path-slug-96>-<fnv1a64hex>/MEMORY.md`, keyed on the
   canonical workspace path. `omm` can seed, back up, list, and garbage-collect per-project memory,
   and can warn about the slug collision that the hash is silently covering. The 48-entry
   "Other Markdown files" cap and the 16,305-byte snapshot budget both apply to this directory.
3. **Do not ship an enterprise tier.** Policy is inert (only empty documents validate), the delivery
   directory under `/Library/Application Support` has no documented leaf and no installer creates it.
   Defaults *are* live, so if Meta ever ships the path, `omm` could ship org defaults — but not one
   restriction. Meanwhile `muse config status` belongs in `omm doctor` as the enterprise probe.
4. **Agent packs die under `safe_mode`.** All three third-party slots (`user`, `project`, `plugin`)
   are suppressed; only `managed` and `built_in` survive, and neither is reachable by a framework.
   `omm doctor` must read `agent_definitions.safe_mode` and say so. Conversely, §4.6 is a gift: the
   plugin *runtime* needs no gate, so `omm` can install once with `MUSE_EXPERIMENTAL_PLUGINS=1` and
   never set it again.
5. **Every hook ships two commands.** `command` (POSIX) **and** `commandWindows` (PowerShell), in a
   native Muse hooks document — because on Windows the shell tool *is* PowerShell, and a
   Claude-format bundle cannot carry the Windows form at all. Windows is not vapour: it has its own
   sandbox (sandbox users, private desktop, integrity labels under `LocalAppDataLow`), its own
   enterprise plane, and ConPTY.
6. **Stop assuming a fake `HOME` sandboxes muse** (§0, Trap B) — and note for `omm uninstall` that
   `~/Library/Application Support/Muse/session-name-authority/` is a real, growing, undocumented
   artifact that nothing else cleans up.

---

## 8. Artefacts

```
settle/loose-ends/q1-exit/exitcodes.sh          canonical exit-code matrix (self-contained)
settle/loose-ends/q1-exit/ptyrun.py             pty runner that reports the child's real exit status
settle/loose-ends/q1b/                          trust-vs-exit-code isolation
settle/loose-ends/q2-memory/srv/{srv.py,sse.txt,catalog.json,lastbody.json}
                                                local /responses server + the accepted SSE stream
settle/loose-ends/q2-memory/home/.local/share/muse/memory/projects/…
                                                the recovered personal_project roots
settle/loose-ends/q3-enterprise/{p.sb,ctl.sb,deny-appsupport.sb}   seatbelt oracles
settle/loose-ends/q3-enterprise/{muse.pkg,pkg/}                    expanded installer + BOM
settle/loose-ends/q3-enterprise/docs/d.json                        enterprise validation fixtures
settle/loose-ends/q4-safemode/{run.sh,run_nt.sh,adplug/}           composition harness + probe plugin
settle/loose-ends/q5-windows/muse-aarch64-windows.exe              verified PE32+ arm64
settle/loose-ends/q5-windows/win.strings.txt                       4.0 MB extraction
settle/loose-ends/q5-windows/hooktest/                             commandWindows selection test
```
