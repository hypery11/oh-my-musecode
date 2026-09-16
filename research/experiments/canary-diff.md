# Canary vs Stable — extension-surface diff

**`muse-stable` 1.0.1-R2006.1  vs  `muse-canary` 1.1.0-R2009.1** (aarch64-macos)

Settles synthesis §7 unknown **#11**.

**Headline: a full minor-version bump moved nothing an extension framework can see.**
Across the 8 requested axes and 37 more, the only observable differences are the version
string and the build commit. Everything else — every gate, every schema, every budget,
every capability family, every discovery root — is identical, most of it byte-for-byte.

Two things nonetheless changed how I would advise building on it: the plugin validator
signals unsupported capabilities as a **warning** next to `valid:true` (§2.1), and a
**server-side feature config** can move the extension surface without any binary
update at all (§2.3). Neither is new in canary — both are simply load-bearing, and
neither is visible to a version check.

---

## 0. Provenance — reproducible from scratch

```bash
W=<scratch>/settle/canary; mkdir -p $W; cd $W

# 1. channel  (pattern from muse-launcher.sh MUSE_CHANNEL_URL, channel name swapped)
curl -sS -A "muse-code/launcher-2" \
  https://api.meta.ai/muse-code/channels/muse-canary
# -> {"channel":"muse-canary","version":"1.1.0-R2009.1",
#     "manifest_url":"https://lookaside.facebook.com/lookaside/muse/download/
#                     ?channel=muse&version=1.1.0-R2009.1&file=manifest.json",
#     "urgency":"none","notification_text":"","state":"canary","min_version":null}

# 2. release manifest (artifact URL pattern from manifest.json)
curl -sS -A "muse-code/launcher-2" -o manifest.json \
  "https://lookaside.facebook.com/lookaside/muse/download/?channel=muse&version=1.1.0-R2009.1&file=manifest.json"

# 3. binary
curl -sS -A "muse-code/launcher-2" -o muse-canary \
  "https://lookaside.facebook.com/lookaside/muse/download/?channel=muse&version=1.1.0-R2009.1&file=muse-aarch64-macos"

# 4. VERIFY BEFORE RUNNING
shasum -a 256 muse-canary   # 30573c830d4aa3d2ed27f947490cdacc42c6a07ddd997f7f16aa0b063f43a492
stat -f%z    muse-canary    # 241904432
chmod +x muse-canary
```

Both checks passed exactly. No authentication was used at any point; every run below is
`--provider echo` in a sandboxed `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME` with
`MUSE_NO_AUTO_UPDATE=1`.

**Channel enumeration.** Exactly two channels exist. `muse-dev`, `muse-beta`,
`muse-nightly`, `muse-internal`, `muse-experimental` all return
`404 {"title":"Not Found","detail":"Channel metadata is not available."}`.
The launcher served to both is byte-identical (`sha256:21c66e55…`), so `MUSE_CHANNEL_URL`
is the only channel selector and there is no third ring to watch.

### Build identity — the only real differences in the whole report

| Field | stable | canary |
|---|---|---|
| `product_version` | 1.0.1 | **1.1.0** |
| release tag | R2006.1 | **R2009.1** |
| `build_commit` | `e27e408b666e693900118f778bd6c2880f88e432` | **`36e29c15c8c300cb18da01808eb554d934505f6c`** |
| `workflow_engine` | v8 | v8 |
| `build` | developer | developer |
| binary size | 241,576,272 | 241,904,432 (**+328,160 B, +0.136 %**) |
| channel `state` | `public` | `canary` |

The binary genuinely changed (+320 KB of code). None of that change reaches the
extension surface.

---

## 1. The complete difference table

Every row below is a measured comparison, not an inference. `≡` = identical.

| # | Axis | Instrument | stable | canary | Verdict |
|---|---|---|---|---|---|
| 1 | **CLI: command set** | recursive `--help` on all 16 commands | 16 commands | 16 commands | **≡ byte-identical help text, all 16** |
| 2 | **CLI: behavioral sweep** | 40 invocations, normalized diff | — | — | **39/40 ≡; only `--version` differs** |
| 3 | **CLI: root flags** | root `--help` | 30 flags | 30 flags | **≡ byte-identical** |
| 4 | **Feature gates** | `gate.resolve` trace × 41 | 41 gates | 41 gates | **≡ names identical, ZERO default flips** |
| 5 | **Gates default-ON** | same | 14 | 14 | **≡ same 14** |
| 6 | **`MUSE_EXPERIMENTAL_*` env tokens** | direct grep, both binaries | — | — | **≡ (see §3 on the strings-diff trap)** |
| 7 | **`TBH_*`/`MUSE_*` env surface** | direct grep, 22 candidates | — | — | **≡ all present in both** |
| 8 | **settings.json `schema_version`** | wrong-value oracle | 1 required; 0, 2, absent rejected | same | **≡ still hard-required, still exactly 1** |
| 9 | **settings.json keys** | wrong-type serde oracle, 39 keys | — | — | **≡ every struct name identical, no new keys** |
| 10 | **Plugin `schemaVersion`** | manifest oracle, values 0/1/2/99 | only 1 accepted | only 1 accepted | **≡** |
| 11 | **Plugin capability families** | validator capability snapshot | 5: `skills, hooks, mcp_servers, commands, reminders` | same 5 | **≡** |
| 12 | **`agents` capability** | non-empty entry + real file | `unsupported`, dropped from snapshot | same | **≡ still not real** |
| 13 | **`tools` capability** | same | `unsupported`, dropped | same | **≡ still not real** |
| 14 | **`developerPrompts` capability** | entry with valid `text` | `unsupported`, dropped | same | **≡ still not real** |
| 15 | **`outputStyles`/`settings`/`apps`** | same | dropped | dropped | **≡** |
| 15b | **`compatibility.declarations`** | validator JSON | per-entry `supported`/`unsupported` + `summary` | same | **≡ (this is the field to check — see §2.1)** |
| 16 | **create-plugin contract (in-binary)** | sha256 of extracted tree | — | — | **≡ every bundled file byte-identical** |
| 17 | **Bundled skills** | full tree checksum | 15 skills + refs + scripts | same | **≡ byte-identical, incl. `native-plugin-contract.md`, `capability-examples.json`** |
| 18 | **Hook events** | plugin validator, 27 names tested | 17 accepted, 10 rejected | identical | **≡ exactly 17, no additions** |
| 19 | **Skills discovery roots** | planted markers at 9 candidates | 7 discovered | 7 discovered | **≡ `.muse/skills` + bare `skills/` still NOT roots** |
| 20 | **Rules discovery** | planted markers at 6 candidates | `AGENTS.md` only | `AGENTS.md` only | **≡** |
| 21 | **Context blocks** | `trace inspect --render-mode verbose` | 8 blocks, orders 85/95/96/180/181/186/200/240 | identical | **≡ same orders AND same byte sizes** |
| 22 | **`total_lane_bytes`** | same | 34,189 | 34,189 | **≡ exact** |
| 23 | **skills_catalog cap** | binary search, from scratch | **32,000** | **32,000** | **≡ still exactly 32000** |
| 24 | **Plugin manifest cap** | boundary test | 131,072 | 131,072 | **≡** |
| 25 | **Active toolset** | `trace inspect` | 26 tools | 26 tools | **≡ same names, same order** |
| 26 | **MSP stable fingerprint** | `muse schema generate-json-schema` | `sha256:03312c21…` | `sha256:03312c21…` | **≡** |
| 27 | **MSP experimental fingerprint** | `--experimental` | `sha256:577d717d…` | `sha256:577d717d…` | **≡** |
| 28 | **MSP schema bytes** | sha256 of `msp.schema.json` | `f7c77710…` | `f7c77710…` | **≡ byte-identical** |
| 29 | **MSP TypeScript export** | sha256 of `msp.d.ts` | `5108cbde…` | `5108cbde…` | **≡ byte-identical** |
| 30 | **MSP method list** | schema `methods` / `notifications` / `errors` | 31 / 23 / 29 | 31 / 23 / 29 | **≡** |
| 31 | **MSP `initialize` result** | live `muse serve` handshake | — | — | **≡ except `serverInfo.version` + `userAgent`** |
| 32 | **`tbh-curated`** | `marketplace add` | reserved, rejected | reserved, rejected | **≡ still reserved** |
| 33 | **Shipped public content** | `plugins list --available` | `[]` | `[]` | **≡ still empty** |
| 34 | **Reserved names** | `loop`,`muse-core`,`tbh-reminders` | addable as marketplace names | same | **≡** |
| 35 | **Enterprise generation hash** | `config status` | `sha256:db7c1fb6…` | `sha256:db7c1fb6…` | **≡** |
| 36 | **Enterprise planes** | `config validate` | 3 planes, `unknown_member` | same | **≡** |
| 37 | **`export_schema_version`** | `muse export` | 1 | 1 | **≡ same document keys** |
| 38 | **session.jsonl envelope** | frame keys + `frame_schema_version` | 1 | 1 | **≡** |
| 39 | **record/payload types** | full session inventory | — | — | **≡ 43 records, identical types** |
| 40 | **Trace event vocabulary** | `event="…"` set | 9 | 9 | **≡** |
| 41 | **`path.resolved` roots** | trace | 4 kinds | 4 kinds | **≡** |
| 42 | **TUI first screen** | pty harness + VT100 render | — | — | **≡ except the version banner line** |
| 43 | **TUI slash commands** | paged menu capture, pristine HOME | 39 built-in | 39 built-in | **≡ identical set** |
| 44 | **Remote feature-config surface** | binary blob compare | — | — | **≡ identical override key list** |

**Total: 45 measured axes. 2 differences, both cosmetic (`--version` string, `userAgent`
build hash). Zero semantic differences.**

### The 41 gates, with defaults (identical in both builds)

Default-**ON** (14, plus `plugins` when overridden):
`workflow_tool`, `local_session_messaging`, `bash_titles`, `bash_sandbox_escalation`,
`git_sandbox_relaxation`, `first_turn_minimal_effort`, `memory_reminder`,
`skill_reminder`, `goal_reminder`, `verify_reminder`, `scope_reminder`,
`non_strict_tool_params`, `sdk_enabled`, `voice_native_capture`.

Default-**OFF** (27): `artifact_tool`, `external_agent_ingress`, `code_mode`,
`prefix_compaction`, `monitor`, `foreign_personal_context_kill`, `voice`,
`voice_default_on`, `reasoning_display`, `model_effort_context`, `plugins`,
`enterprise_config`, `web_fetch`, `server_web_fetch`, `curl_web_fetch`,
`web_fetch_preflight_hard_cap`, `todo_reminder`, `session_runtime`,
`session_recovery_shadow`, `tui_msp_client`, `meta_context_workspace_only`,
`hook_selected_skills`, `hook_selected_skills_apply`, `subscription_launch`,
`provider_tool_switch`, `tag`, `workflow_api_v2_rollout`.

Reproduce:
```bash
$BIN plugins enable __probe__            # mode="plugin_mutation" emits all 41
grep -o 'gate="[a-z0-9_]*" enabled=[a-z]*' \
  $XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-*.log
```
Note `muse plugins list` does **not** emit the gate table — only a *mutation*
subcommand (`enable`/`disable`/`install`) does. That cost a false-negative on the
first attempt.

### The 39 built-in slash commands (identical in both builds)

```
/clear /compact /copy /create-skill /deep-research /doctor /effort /export /feedback
/fork /goal /grill /grill-and-record /help /import /init /keymap /logout /loop
/manage-settings /model /name /new /plan /quit /recap /resume /rules /settings /side
/skills /status /stop /subagents /tasks /theme /usage /voice /workflows
```

Captured through the pty harness with a **pristine** `HOME`. A first pass against the
working sandbox reported 41 — `/x` and `/mk` were the probe plugin's own command and a
planted marker skill leaking into the menu. Worth noting as a method caution: the slash
menu reflects installed plugins, so any slash-surface baseline must be taken against a
clean profile.

---

## 2. The three findings worth acting on

### 2.1 `agents`, `tools`, `developerPrompts` did NOT become real

This was the question that could have changed the product, and the answer is a clean no
— **identically on both builds**. But the validator's behaviour is more nuanced than
"rejected", and getting it right matters for a manifest generator.

The phrase the binary uses is `"plugin <kind> capabilities are not supported in this
phase"` — Meta's own wording implies a later phase, but nothing has arrived by 1.1.0.

| manifest shape | result |
|---|---|
| `"agents":[]` (empty array) | ignored entirely — install succeeds, no diagnostic |
| `"__bogus_kind__":[]` | **also** ignored — an empty array proves nothing either way |
| `"agents":[…]` **as the only capability** | hard error `unsupported-capability`, `valid:false` |
| `"agents":[…]` **alongside a supported capability** | `valid:true` + a **`severity:"warning"`** diagnostic; entry dropped from the snapshot |
| `"developerPrompts":[{"id":…}]` (no `text`) | hard error — `developerPrompt capability \`c1\` must declare non-empty \`text\`` |
| `"developerPrompts":[{"id":…,"text":"…"}]` | `valid:true` + `unsupported-capability` warning; dropped |
| `"tools"` / `"outputStyles"` / `"settings"` / `"apps"` / unknown kinds | `valid:true`, dropped, warning for the *known* unsupported kinds only |

**The correction that matters:** these are *not* dropped silently. The validator emits
a warning diagnostic **and** classifies every declaration in a
`plugin.compatibility.declarations` array:

```json
"compatibility": {
  "summary": "partial",
  "declarations": [
    {"id": "command:x",           "kind": "command",          "classification": "supported"},
    {"id": "developer-prompt:c1", "kind": "developer-prompt",  "classification": "unsupported"}
  ]
}
```

`summary` ∈ `supported | partial | unsupported`. **This is the field a bundle linter
should assert on** — `valid:true` alone is not enough, because a manifest declaring
`agents` passes it while doing nothing. Meta's own contract already says a result is
clean only with an **empty `diagnostics` array**, and that "warnings are failures for
creation" — so the contract and the validator do agree; it is the naive
`jq .valid` check that is wrong.

Note also that `developerPrompts` has real per-field validation (`text` required
non-empty) even though the capability is unsupported — the parser is further along than
the runtime. That is the one place a future phase looks most likely to land first.

**Tooling gotcha (both builds):** on some error paths `plugins validate --json` writes a
JSON document **followed by a bare plain-text line** repeating the message, so the
stream is not a single JSON value. Use `json.JSONDecoder().raw_decode()` (or read only
the first document) rather than `json.load()`, which fails with `Extra data`.

Reproduce:
```bash
# capabilities: {"commands":[…], "agents":[{"id":"c1","path":"blob/f.md"}]}
$BIN plugins validate ./pp --json | python3 -c '
import sys,json; d=json.JSONDecoder().raw_decode(sys.stdin.read())[0]
print(d["valid"], [x["code"] for x in d.get("diagnostics",[])],
      list(d["plugin"]["capabilities"]), d["plugin"]["compatibility"]["summary"])'
# -> True ['unsupported-capability'] \
#    ['skills','hooks','mcp_servers','commands','reminders'] partial      # both builds
```

### 2.2 The 32,000-byte skills catalog cap is unchanged — re-proven from scratch

Binary search on a single growing description, run independently against each binary:

| desc bytes | stable block | canary block |
|---|---|---|
| 21,863 | **32,000** | **32,000** |
| 21,864 | 10,101 (collapse) | 10,101 (collapse) |

Identical boundary to the byte. The cap is still `32000` (not 32768), descriptions are
still never truncated — entries past the running total lose their `<description>`
wholesale — and there is still **zero diagnostic**. `omm doctor` must keep this check.

### 2.3 The real risk is not the binary — it is the server-side feature config

This is the one thing that should change how the framework is designed, and it is
**identical in both builds**, which is precisely why the binary diff being empty is not
sufficient reassurance.

Both binaries carry a remote configuration provider:

```
event="path.resolved" kind="feature_config_cache" source="data_root" state="absent"
event="feature_config.cache" state="missing" gate_count=0
       crates/config/src/feature_provider/cache.rs:47
```

`gate_count=0` only because this session never authenticated. The override key list
embedded in the binary (byte-identical across builds) includes, alongside the gates:

```
extensions.skills            extensions.skills.allowed_digests
extensions.hooks             extensions.skills.allowed_kinds
extensions.runtime_capabilities
settings.run.system_prompt   settings.run.developer_prompt   settings.run.toolset
settings.feature_config
```

**Meta can remotely allowlist which skills and extension kinds are permitted
(`allowed_digests`, `allowed_kinds`), and can replace the toolset and prompts, without
shipping a binary.** A framework that verifies compatibility by pinning a binary
checksum will not notice any of it.

I could not exercise this — it requires authentication, which is out of scope. State
it as a known unknown, not a proven risk: what is proven is that the *mechanism* exists,
is wired to the gate registry, and names the extension surface explicitly.

---

## 3. Methodology warning: the naive strings-diff is ~100 % false positives

Worth recording because it is the obvious way to do this job and it does not work.

```
strings -n 6 stable | sort -u  -> 72,817 lines
strings -n 6 canary | sort -u  -> 72,849 lines
only-stable: 2,304    only-canary: 2,336
```

That looks like a substantial diff. It is not. Rust packs string literals into
concatenated rodata blobs, and a *single* code change shifts blob boundaries, so the
same atomic token appears inside a different neighbour-run in each binary.

Concrete case: `TBH_MANAGED_HOOKS_PATH` appeared in the "only in stable" list. Direct
grep finds it **once in each binary**. Every one of 22 candidate tokens I chased this
way existed in both. The apparent 4,640-line difference contains, as far as I could
determine, **zero** real vocabulary changes.

**Use behavioural oracles instead** — they are decisive and cheap:

| Want | Oracle |
|---|---|
| gate list + defaults | `plugins enable X` → `gate.resolve` in the bootstrap trace |
| settings key set | wrong-*type* value → serde names the expected struct |
| settings unknown keys | silently ignored — the type oracle is the only way |
| hook event set | plugin manifest with that `event` → `unsupported-hook-event` |
| capability families | read `plugin.capabilities` back out of `plugins validate --json` |
| context blocks + budgets | `trace inspect --session-log F --render-mode verbose --format json` |
| MSP surface | `schema generate-json-schema` → compare `fingerprint` |
| discovery roots | plant a marker skill/rules file at each candidate, `skills list` |

---

## 4. Verdict

### Is the extension surface stable enough to build on?

**Yes — but the version number is not what makes it so, and must not be used as the
compatibility signal.**

A full minor bump (1.0.1 → 1.1.0), three release trains apart (R2006 → R2009), with a
new build commit and +320 KB of binary, produced **zero** change across 44 measured
extension-relevant axes. Every finding in the 12-dimension teardown remains valid
against canary. Nothing in `oh-my-musecode`'s design needs revision.

The correct reading is not "Meta moves slowly." It is:

> **Muse's semantic version tracks the product, not the extension contract.**
> `1.0.1 → 1.1.0` told you nothing — in *either* direction. It did not signal breakage
> here, and a future `1.1.1 → 1.1.2` will not signal safety.

So: build on it, and **verify by fingerprint, never by version comparison.** Three
concrete consequences:

1. **Do not gate on `--version`.** A version check is both a false alarm and a false
   reassurance. Gate on the specific fingerprints in §5.
2. **Pin the binary checksum, but do not trust it alone** — the server-side
   `feature_config` (§2.3) can move `extensions.skills.allowed_kinds`,
   `settings.run.toolset` and the gate defaults underneath a pinned binary.
3. **`omm doctor` is the compatibility layer**, not a nice-to-have. It is the only
   mechanism that can detect either kind of drift.

### What must be re-verified per release

Ordered by (catastrophic if changed) × (cheap to check). The first six are a
sub-five-second CI job and should run on **every** channel poll, not every release —
because the channel can move without the version changing meaningfully, and the
feature-config can move without the channel changing at all.

| Pri | Check | Command | Breaks what, if it moves |
|---|---|---|---|
| **P0** | MSP stable fingerprint | `schema generate-json-schema --out D; jq -r .fingerprint D/manifest.json` | every generated MSP client |
| **P0** | Plugin `schemaVersion` still 1 | `plugins validate` a fixture with `schemaVersion:2` → must reject | every shipped bundle fails to install |
| **P0** | Capability families still the 5 | read `plugin.capabilities` keys from `plugins validate --json` | bundles lose capabilities behind a mere *warning* |
| **P0** | `compatibility.summary` == `supported` | assert on `plugin.compatibility.summary` + empty `diagnostics`, **not** on `valid` | `valid:true` hides every unsupported declaration |
| **P0** | `settings.schema_version` still 1 | write `{"schema_version":2}` → must print `unsupported settings schema version 2` | every managed settings file |
| **P1** | 41 gates + their defaults | `plugins enable __probe__`, diff `gate.resolve` lines against a golden file | a default-ON→OFF flip silently disables a whole feature |
| **P1** | 17 hook events | validate a fixture per event, expect 17 accept / N reject | hooks stop firing, no diagnostic |
| **P1** | skills_catalog cap = 32000 | binary-search fixture (≈40 s) | **silent** skill dropping — the worst failure mode in the product |
| **P1** | Context block orders + sizes | `trace inspect --render-mode verbose`, diff the 8 (order,id,bytes) tuples | context budget maths, injection ordering |
| **P2** | Discovery roots (7 skills, rules) | planted-marker probe | user content silently stops loading |
| **P2** | Bundled-skill tree checksum | sha256 the extracted `skills/bundled` tree | the in-binary authoring contract changed |
| **P2** | 26-tool active set | `trace inspect` toolset | tool-name assumptions in prompts/hooks |
| **P2** | `tbh-curated` still reserved | `marketplace add tbh-curated` → must reject | Meta launched a registry; re-plan G3 |
| **P2** | `plugins list --available` empty | — | Meta started shipping content; re-plan G4 |
| **P3** | Channel enumeration | probe `muse-{dev,beta,nightly,…}` | a new ring appeared |
| **P3** | Binary caps (131072 / 262144 / 4096 / 16) | boundary fixtures | bundle size limits |

**Also worth a standing watch, though it cannot be checked offline:** the authenticated
`feature_config.fetch` payload. If a logged-in machine is ever available, dump
`gate_count` and any `extensions.*` overrides from the bootstrap trace and diff those
too — that is the channel most likely to move first, and the only one invisible to
every check above.

### What this experiment did NOT cover

Stated plainly so nobody over-reads the result:

- **No authenticated run.** No real model turn, so `base_instructions` /
  `developer_prompt` were `null` and the ~36 KB instruction block and 40 KB tools array
  were never composed. Their *sizes* are therefore unverified on canary; their
  *ordering slots* are verified.
- **`feature_config` never populated** (`gate_count=0`) — §2.3 is a mechanism finding,
  not a behaviour finding.
- **TUI beyond the opening screen + slash menu.** No authenticated session, so the
  plugin-MCP-in-TUI question (synthesis unknown #1) is untouched here.
- **Windows and enterprise managed-preferences planes** — same blockers as the original
  teardown.
- **aarch64-macos only.** The other six artifacts were checksum-listed, not run.

### Bonus: synthesis unknown #9, partially settled

`muse zzznotacommand` does **not** exit 2 as a parse failure — an unknown top-level
token is treated as a **prompt** and starts the TUI (here: `Device not configured
(os error 6)`, exit **1**, identical on both builds). A wrapper cannot distinguish
"unknown command" from "runtime failure" by exit code; both are 1.

---

## 5. Golden fingerprints for CI

```
MSP stable fingerprint     sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7
MSP experimental           sha256:577d717d09bf3aae6ad43c85d3c2d8e0c37bbde350897c94808626d2f362060c
msp.schema.json bytes      f7c77710dbf181b309a3a12060627608dd5c91b1ec0680953ca7381b21181beb
msp.d.ts bytes             5108cbde447cdfe65fd1bd26f8d2910a20141668959d0da8ce6bd55bb1adcf81
enterprise generation      sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4
MSP surface                31 methods, 23 notifications, 29 error codes
gates                      41 total, 14 default-ON (list in §1)
hook events                17
bundled skills             15 SKILL.md, 21 files total
plugin capability families 5  (skills, hooks, mcp_servers, commands, reminders)
context blocks             8  @ orders 85,95,96,180,181,186,200,240
skills_catalog cap         32000
plugin manifest cap        131072
active tools               26
slash commands             39 built-in (plugin commands add to this)
settings schema_version    1 (0 and 2 rejected)
plugin schemaVersion       1 (0, 2, 99 rejected)
export_schema_version      1
frame_schema_version       1
```

All values above hold for **both** 1.0.1-R2006.1 and 1.1.0-R2009.1.

---

## 6. Artifacts on disk

```
settle/canary/muse-canary            verified canary binary (241,904,432 B)
settle/canary/manifest.json          canary release manifest
settle/canary/chan.json              canary channel document
settle/canary/envs.sh                sandboxed run_stable / run_canary helpers
settle/canary/dual.sh                run-both-and-normalized-diff helper
settle/canary/sweep.sh               40-command behavioral sweep
settle/canary/harvest_help.py        recursive --help harvester
settle/canary/tui/drive2.py          pty harness, parameterized by binary + HOME
settle/canary/out/help-{stable,canary}.json    16 help texts each
settle/canary/out/gates-{stable,canary}.tsv    41 gates + defaults
settle/canary/out/insp-{stable,canary}.json    verbose trace inspect
settle/canary/out/sch-*/                       MSP schema exports
settle/canary/out/slashC-{stable,canary}.txt   39 built-in slash commands (pristine HOME)
settle/canary/clean-{stable,canary}/           pristine HOMEs used for the slash count
settle/canary/env-{stable,canary}/             the two sandboxed HOMEs
```
