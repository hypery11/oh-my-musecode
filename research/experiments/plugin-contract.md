# SETTLED — The authoritative native plugin manifest spec, contract vs. binary

Question: read Meta's own in-binary authoring contract, produce the definitive `plugin.json`
specification a generator should target, and validate every contract claim against
`muse plugins validate`. List every place the contract and the observed validator disagree.

**Verdict: RESOLVED_YES.** The contract is broadly accurate but has **13 material disagreements**
with the shipped validator, five of which will silently produce a broken-but-"valid" plugin.
A five-capability reference package that validates clean on both layers is saved as an artifact.

Binary: `.../scratchpad/muse-aarch64-macos` — `Muse Code 1.0.1 (1.0.1-R2006.1)`
Sandbox: `.../scratchpad/settle/plugin-contract/` (`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` all
under it; `MUSE_NO_AUTO_UPDATE=1`, `MUSE_EXPERIMENTAL_PLUGINS=1`). No login, no network, no
`--provider` needed — `plugins validate` and `skills validate` are offline verbs.

Sources read in full:
- `.../re/artifacts/create-plugin/references/native-plugin-contract.md` (166 lines)
- `.../re/artifacts/create-plugin/references/capability-examples.json` (785 lines)
- `.../re/artifacts/create-plugin/SKILL.md` (184 lines)
- `.../re/plugins.md` §§0–4, §7, §17, and the whole Verification section

Harness: `.../settle/plugin-contract/probe.py` (builds a package from a manifest dict + file map,
runs `plugins validate --json`, summarises). ~200 packages built under `.../settle/plugin-contract/pkgs/`.

---

## 0. Reproduce from scratch

```sh
S=/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad
D=$S/settle/plugin-contract
mkdir -p $D/home $D/xdgconfig $D/xdgdata
export HOME=$D/home XDG_CONFIG_HOME=$D/xdgconfig XDG_DATA_HOME=$D/xdgdata
export MUSE_NO_AUTO_UPDATE=1 MUSE_EXPERIMENTAL_PLUGINS=1

# both validation layers on the reference package
$S/muse-aarch64-macos skills  validate $D/artifacts/ohmy-reference/skills/review --json
$S/muse-aarch64-macos plugins validate $D/artifacts/ohmy-reference               --json
# -> both: exit 0, "valid": true, "diagnostics": [], compatibility.summary "full"

# rerun the whole probe battery
cd $D && python3 -c "import sys;sys.path.insert(0,'.');import probe;probe.run('c-minimal')"
```

Every `--- LABEL` line quoted below comes from that harness; the message text is the
validator's verbatim `error.message` or `diagnostics[].message`.

---

## 1. THE DEFINITIVE SPEC (what a generator must target)

### 1.1 Package layout

The native manifest is **only ever** `<pluginRoot>/.muse-plugin/plugin.json`. The directory name
is fixed by manifest *discovery*, not by `compat.manifestDir`:

```
--- MD-custom-dir      (dir .muse, compat.manifestDir ".muse")
rc=1 missing-manifest :: plugin root must contain one supported plugin manifest: a root
plugin.json with the exact Agent Plugins 1.0.0 $schema, or exactly one nested
.muse-plugin/.codex-plugin/.claude-plugin plugin.json
--- MD-root-only       (plugin.json at package root)     -> same missing-manifest
--- MD-two-manifests   (.muse-plugin + .claude-plugin)   -> multiple-manifests
```

So `compat.manifestDir` has exactly one legal value for the native family: `".muse-plugin"`.

### 1.2 Top-level fields — measured requiredness

| field | type | required | empty allowed | notes |
|---|---|---|---|---|
| `schemaVersion` | integer | **yes** | – | must be integer `1`. `0`, `2`, `"1"` → `invalid-manifest-schema :: plugin manifest must declare schemaVersion 1` |
| `name` | string | **yes** | **no** | the plugin id; grammar §1.3 |
| `displayName` | string | **no** | yes | the ONLY optional top-level field. `""` and `null` both accepted |
| `version` | string | **yes** | **no** | free-form. `"0.1.0"`, `"v1.2.3"`, `"not-a-semver at all"` all pass. `""` → `invalid-version :: plugin version must not be empty`; integer → `must declare string field \`version\`` |
| `description` | string | **yes** | **no** | `""` and `null` both → `plugin manifest must declare string field \`description\`` |
| `compat` | object | **yes** | – | only `manifestDir` is read |
| `compat.manifestDir` | string | **yes** | no | must equal `".muse-plugin"` |
| `compat.source` | string | **no** | – | **not validated at all** — see D2 |
| `capabilities` | object | **yes** | `{}` ok | `null`/array/string → `plugin capabilities must be a JSON object` |

Verbatim, dropping each field from the contract's own minimal manifest:

```
--- top-drop-schemaVersion  rc=1 invalid-manifest-schema  :: plugin manifest must declare schemaVersion 1
--- top-drop-name           rc=1 invalid-manifest-schema  :: plugin manifest must declare string field `name`
--- top-drop-displayName    rc=0 VALID=True diagnostics=0        <-- only optional one
--- top-drop-version        rc=1 invalid-manifest-schema  :: plugin manifest must declare string field `version`
--- top-drop-description    rc=1 invalid-manifest-schema  :: plugin manifest must declare string field `description`
--- top-drop-compat         rc=1 manifest-family-mismatch :: plugin manifest must declare compat.manifestDir
--- top-drop-capabilities   rc=1 invalid-manifest-schema  :: plugin manifest must declare capabilities
```

**Unknown top-level keys are warnings, and warnings are failures for creation:**

```
--- E-unknown-top-keys  rc=0 VALID=True diagnostics=4
    [warning] unsupported-field :: plugin manifest field `author` is not used by this runtime
    [warning] unsupported-field :: plugin manifest field `homepage` is not used by this runtime
    [warning] unsupported-field :: plugin manifest field `license` is not used by this runtime
    [warning] unsupported-field :: plugin manifest field `keywords` is not used by this runtime
```

So the top-level key set a generator may emit is EXACTLY:
`schemaVersion, name, displayName, version, description, compat, capabilities`.

### 1.3 Identifier grammar — `^[a-z0-9][a-z0-9._-]{0,79}$`

Reproduces exactly, for the plugin id AND for every capability id (same regex, same 80-byte cap):

```
a            VALID    | 0             VALID    | ok.id_1-2  VALID
"a"*80       VALID    | "a"*81        invalid-plugin-id :: plugin id `aaa…` is invalid
Uppercase    invalid  | aBc  invalid  | -lead invalid | .lead invalid | _lead invalid
trail-       VALID    | trail.        VALID              <-- trailing punct IS legal
"has space" "has/slash" "has\back" "has:colon" "has@at" "has+plus" "café"   all invalid
```
Capability ids use the identical rule with a per-family code:
`skill capability id \`UPPER\` is invalid` / `invalid-capability-id`, 80 ok / 81 rejected.

### 1.4 Path rules

- relative only: `/etc/passwd` → `unsafe-path :: plugin capability path must be relative`
- non-empty: `""` → `unsafe-path :: plugin capability path must not be empty`
- **no `..` segment at all, even one that resolves back inside**:
  `skills/x/../review/SKILL.md` → `unsafe-path :: plugin capability path must stay inside plugin root`
  (the check is lexical, not canonical — stricter than the contract's "canonicalize containment")
- `./` and `//` and `/./` ARE normalised and accepted (`./skills/review/SKILL.md` → VALID)
- must resolve to an existing **regular file**: missing → `missing-capability-path :: plugin file is
  not readable: No such file or directory (os error 2)`; a directory → `plugin path must be a file`
- **any symlink anywhere in the package is fatal**, including one that stays inside:
  ```
  --- MD-internal-symlink (skills/r/SKILL.md -> ../../real/SKILL.md)
  rc=1 invalid-plugin-package :: Agent Definition inventory derivation failed closed
  --- MD-stray-symlink    (alias.txt -> real.txt, referenced by nothing)   -> same
  ```
- **a backslash is not diagnosed as a backslash.** A manifest path with `\` is read as a literal
  filename → `plugin file is not readable`. If a file with that literal name actually exists, the
  whole package dies with the same opaque `Agent Definition inventory derivation failed closed`.

Package limits (re-measured / cited): manifest ≤ **131072 bytes** exact
(`131325 B -> plugin manifest exceeds 131072 byte limit`; `123205 B -> VALID`);
≤ 4096 filesystem entries; ≤ 16 directory levels (both from `re/plugins.md` M2).

### 1.5 `capabilities` container

Supported families: `skills, commands, hooks, mcpServers, reminders`.
Every family key is optional and every array may be empty; `"capabilities": {}` validates true.
Partial omission (`{"skills": []}`) validates true. A family whose value is not an array →
`skill capabilities must be an array`.

Unknown / inactive keys inside `capabilities` are **warnings**:
```
--- D-caps-unknown-key  [warning] unsupported-field :: plugin capability field `zzzUnknown` is not used by this runtime
--- D-caps-lspServers   [warning] unsupported-field :: plugin capability field `lspServers` is not used by this runtime
--- D-caps-userConfig   [warning] unsupported-field :: plugin capability field `userConfig` is not used by this runtime
```

### 1.6 `skills[]`

`{ "id": <id>, "path": <relpath>, "enabledDefault": <bool>? }`

- `id` required → `invalid-capability-id :: skill capability must declare an id`
- `path` required → `missing-capability-path :: skill capability \`review\` must declare a path`
- `enabledDefault` **defaults to `true`**, and only a literal JSON `false` turns it off:
  ```
  enabledDefault=None -> enabled_default=True     enabledDefault=False -> False
  enabledDefault='yes'-> enabled_default=True     enabledDefault=0     -> True
  ```
- unknown keys on a skill entry are **silently accepted, no diagnostic** (`{"zzz":1}` → VALID, 0 diags)
- two skills may share the same `path` (unlike hooks)
- duplicate skill id → `duplicate-capability-id :: duplicate skill capability id \`review\``

### 1.7 `commands[]`

`{ "id": <id>, "path": <relpath>, "enabledDefault": <bool>? }` — identical rules to `skills`,
plus a cross-family collision:
`command capability id \`review\` duplicates a skill id in the same plugin`.
Errors: `command capability \`summarize\` must declare a path`, `command capability must declare an id`.
Any extension works (`.txt` validates); the file's content is never inspected (an empty file passes).

### 1.8 `hooks[]` — the only family with a CLOSED field set

Allowed keys, enumerated by probing 32 candidate names (unknown key → hard error
`unsupported-field :: hook capability field \`X\` is not supported`):

```
ALLOWED:  id  event  command  timeoutMs  statusMessage  async  compatibilityName  outputCapabilities
REJECTED: args background blocking compatibility_name condition cwd description displayName
          enabledDefault env filter name once output_capabilities path priority shell source
          status_message timeout timeout_ms tools type when
SPECIAL:  matcher -> unsupported-field :: hook capability field `matcher` is a Claude/Codex hook
          field; TBH plugin hooks key on `event`; matcher aliasing is tracked separately
```

| field | type | required | notes |
|---|---|---|---|
| `id` | string | yes | `hook capability must declare an id` |
| `event` | enum | yes | `hook capability \`h\` must declare an event`; 17 PascalCase values, §1.8.1 |
| `command` | string[] | yes | argv. String → `must declare a command array`; `[]` → `command must not be empty` |
| `timeoutMs` | integer | no | **not range-checked** — `0`, `-1`, `"1000"`, `99999999` all VALID |
| `statusMessage` | string | no | `""` accepted |
| `async` | bool | no | default `false` |
| `compatibilityName` | string | no | must not equal a built-in tool matcher: `compatibilityName \`Bash\` collides with a built-in tool matcher name`; `"my_custom_tool"` ok |
| `outputCapabilities` | array | no | must be **exactly** `["skills.v1"]`, and only on **foreground** `UserPromptSubmit` or `PostToolUse` |

argv path handling: an argv element that looks like a relative path must exist as a regular file
under the plugin root; an **absolute** argv element is a literal, not a package path
(`["sh","/etc/passwd"]` → VALID). Traversal in argv → `unsafe-path`.
Two hooks may not share a relative source:
`duplicate-hook-source :: hook capability \`h2\` reuses source \`hooks/pre-check.sh\` already used by \`h1\``
(two hooks sharing an *absolute* argv is fine).

#### 1.8.1 The 17 events (all confirmed accepted, PascalCase only)

`SessionStart UserPromptSubmit PreToolUse PermissionRequest PostToolUse PreLLMCall PostLLMCall
PreCompact PostCompact SubagentStart SubagentStop Stop SessionEnd Notification
PostToolUseFailure StopFailure PostToolBatch`

`Setup` → `unsupported-hook-event :: hook capability \`h\` event \`Setup\` is unsupported`.
snake_case (`pre_tool_use`) → same rejection. `outputCapabilities` accepted on exactly
`UserPromptSubmit` and `PostToolUse`; adding `"async": true` re-triggers the rejection.

### 1.9 `mcpServers[]` — OPEN field set (silent drops)

```
stdio: { "id": <id>, "transport": "stdio"?, "command": [<argv>...] }
http:  { "id": <id>, "transport": "http",   "url": "<non-empty>" }
```
- `transport` defaults to `stdio`; whitelist is stdio|http:
  `mcp server \`m\` transport \`sse\` is unsupported`
- stdio without command → `missing-capability-command :: mcp_server capability \`m\` must declare a
  command array`; `[]` → `command must not be empty`; string → `must declare a command array`
- http without url or `""` → `mcp server \`m\` with http transport must declare url`.
  **The url is not parsed** — `"not-a-url"` VALID.
- `id` required → `mcp_server capability must declare an id`; duplicate id →
  `duplicate mcp_server capability id \`m\``
- **`env`, `cwd`, `headers`, `args`, `timeoutMs`, `enabledDefault` and any other key are accepted
  with zero diagnostics and dropped from the descriptor.** A generator that emits `env` produces a
  plugin that validates perfectly and launches the server without that environment.

### 1.10 `reminders[]` — CLOSED field set, the deepest family

```
{ "id", "path", "decision", "tools"?, "blocking"?, "enabledDefault"?,
  "defaultPriority"?, "maxPriority"?, "maxChildSteps"?, "maxInstallsPerRun"?,
  "reasoningEffort"?, "context"? }
```
Unknown key → `unsupported-field :: reminder capability field \`zzz\` is not supported`
(measured rejected: `command description event name priority statusMessage`).

| field | required | domain |
|---|---|---|
| `id` | yes | id grammar; collides with **skill and command** ids (`reminder capability id \`x\` duplicates another capability id in the same plugin`) but NOT with hook or mcp ids |
| `path` | yes | duty file, must exist |
| `decision` | yes | `reminder capability \`r\` decision must declare a decision` |
| `tools` | no | must be an array; **contents unvalidated** (`["not_a_tool"]` VALID) |
| `blocking` | no | any non-`false` value reads as true; `true` needs `eotProgressGate != notApplicable` |
| `enabledDefault` | no | boolean — `enabledDefault must be a boolean`. **Undocumented in the contract** |
| `defaultPriority` / `maxPriority` | no | `low`\|`normal`\|`high` — `must be low, normal, or high` |
| `reasoningEffort` | no | `none`\|`minimal`\|`low`\|`medium`\|`high`\|`xhigh`\|`ultra` |
| `maxChildSteps` | no | integer 1..1000000 |
| `maxInstallsPerRun` | no | integer 1..1024 |
| `context` | no | object, §1.10.2 |

#### 1.10.1 `decision` — all nine members are REQUIRED

The contract's `"...":"copy the remaining executable fields"` hides that six of them are
non-optional. The serde error enumerates the closed set:

```
--- R-decision-extra-key
invalid-manifest-schema :: reminder capability `r` decision declaration is invalid: unknown field
`zzz`, expected one of `version`, `envelope`, `deliveryRole`, `fields`, `bodyTemplate`,
`validators`, `proposal`, `lifecycle`, `limits`
```
Measured:
```
drop validators   -> decision declaration is invalid: missing field `validators`
drop lifecycle    -> ... missing field `lifecycle`
drop limits       -> ... missing field `limits`
drop fields       -> ... missing field `fields`
drop bodyTemplate -> ... missing field `bodyTemplate`
drop proposal     -> ... missing field `proposal`
drop envelope     -> decision reminder envelope is required
drop deliveryRole -> VALID          <-- OPTIONAL, contract says required (D6)
version: 2        -> decision version must be 1
```
Envelope rules: `version` must be 1; the template must contain **exactly one literal `{text}` slot**
(`reminder envelope template must contain exactly one literal {text} slot`); host-reserved tags are
refused (`<continue>` → `envelope template uses retired reserved tag continue`).
`bodyTemplate.slots[].escape` accepts only `xml`.
`fields[].requirement` ∈ `always|remind|none|optional`.
`proposal.remind.body` must be `{"renderedBody": true}` →
`remind proposal body must use the renderedBody source`.
`validators[]` entries need `{name, ...}`, not `{id}` (`missing field \`name\``).

Cross-field invariants (none of which the contract states):
```
blocking=true + eotProgressGate=notApplicable      -> eotProgressGate=notApplicable requires blocking=false
installBudget=finiteRequired, blocking=false       -> installBudget=finiteRequired requires blocking=true and a finite nonzero maxInstallsPerRun
eotProgressGate=decisionWithFiniteBudget, roster   -> requires installBudget=finiteRequired
failureFallback=afterNewWork (any blocking)        -> decision syntheticFailure rejection must use literal null
lifecycle.seenKey ∈ {ordinary, redeliverExpired}   -> unknown variant `bogus`, expected `ordinary` or `redeliverExpired`
```

#### 1.10.2 `context` — an entire undocumented sub-schema

The contract names `context` as an "optional policy field" and says nothing more. It is a closed
object with exactly two members, `feeds` and `conversation`:

```
context.feeds        OK      | context.hostFeeds/host/budget/maxTokens/sources
context.conversation OK      |   -> context field `X` is not supported
```

`feeds[]` (≤ 8 entries; aggregate `totalMaxBytes` ≤ 24000):

| field | required | rule |
|---|---|---|
| `name` | yes | lowercase capability identifier grammar |
| `kind` | yes | must be `"digest"` |
| `roots` | yes | 1..8 canonical project-relative paths (no absolute, no `..`) |
| `indexFile` | yes | canonical relative path ending in lowercase `.md`; **need not exist on disk** |
| `inlineMaxBytes` | yes | integer 1..8000, ≤ `totalMaxBytes` |
| `totalMaxBytes` | yes | integer 512..24000 |
| `placement` | no | `before_conversation` \| `after_conversation` |

`conversation`: `{"mode": "full"|"bounded"}`; `bounded` additionally requires an integer `maxTokens`
(`bounded context conversation must declare integer field \`maxTokens\``).

### 1.11 Unsupported families — fatal ONLY when nothing supported survives

This is the single largest correction to the contract.

```
--- tools ONLY                 rc=1 ERROR unsupported-capability :: plugin tools capabilities are not supported in this phase
--- skill + tools              rc=0 valid=True summary=partial
      [warning] unsupported-capability :: plugin tools capabilities are not supported in this phase
--- skill + agents             rc=0 valid=True summary=partial  [warning] …
--- skill + outputStyles       rc=0 valid=True summary=partial  [warning] …
--- skill + settings           rc=0 valid=True summary=partial  [warning] …
--- skill + apps               rc=0 valid=True summary=partial  [warning] …
--- skill + developerPrompts   rc=0 valid=True summary=partial  [warning] …
--- empty capabilities {}      rc=0 valid=True summary=unsupported
```
And an **empty** unsupported array is invisible: `{"tools": []}` → VALID, **zero** diagnostics.

`developerPrompts` behaves identically to the five named unsupported families (still hard-gated:
`plugin developerPrompts capabilities are not supported in this phase`), which the contract omits.
`lspServers` and `userConfig` are a third class: always warn-and-ignore, never fatal.

### 1.12 Validation order (measured by peeling one violation at a time)

```
as-authored                  -> invalid-manifest-schema  :: plugin manifest must declare schemaVersion 1
fix schemaVersion            -> invalid-plugin-id        :: plugin id `BAD ID` is invalid
fix name                     -> invalid-version          :: plugin version must not be empty
fix version                  -> invalid-manifest-schema  :: plugin manifest must declare string field `description`
fix description              -> manifest-family-mismatch :: compat.manifestDir `.claude-plugin` does not match …
fix compat.manifestDir       -> unsupported-capability   :: plugin tools capabilities are not supported in this phase
```
The full order, with the interleavings pinned by targeted pairs:

1. manifest bytes ≤ 131072 → manifest is valid JSON → root is a JSON object
2. `schemaVersion == 1`
3. `name` present-and-string → **plugin id grammar** (grammar beats every later check:
   `bad id + version MISSING -> invalid-plugin-id`)
4. `version` present-and-string → non-empty
5. `description` present-and-string-and-non-empty
6. `compat.manifestDir` present → matches the actual directory
7. `capabilities` is an object → unsupported-family gate (fatal iff nothing supported remains)
8. per family, per entry: **id grammar → path safety (`unsafe-path`) → file readability
   (`missing-capability-path`)**, then family-specific rules
   (`cap: bad id + bad path -> invalid-capability-id`; `cap: bad path -> unsafe-path`;
   `cap: safe path, missing file -> missing-capability-path`)
9. hook entries: unknown-field and event checks precede argv path checks
   (`hook: bad event + missing file -> unsupported-hook-event`;
   `hook: unknown field + missing file -> unsupported-field`)
10. duplicate-id / duplicate-hook-source across the whole manifest
11. package hygiene last (symlinks, backslash filenames) — surfaces as
    `invalid-plugin-package :: Agent Definition inventory derivation failed closed`

Only the **first** fatal error is reported; `error.diagnostics` may carry siblings. Exit code is
0 on `valid:true` (even with warnings) and 1 on any fatal error.

### 1.13 The two-layer contract, and why it exists

The contract mandates `muse skills validate <dir> --json` before
`muse plugins validate <pkg> --json`. This is **load-bearing**, because `plugins validate` performs
**no inspection of SKILL.md whatsoever**:

```
--- S-no-frontmatter          (SKILL.md = "just body\n")            rc=0 VALID=True diagnostics=0
--- S-fm-missing-description  (frontmatter without description)     rc=0 VALID=True diagnostics=0
--- S-fm-name-mismatch        (frontmatter name != capability id)   rc=0 VALID=True diagnostics=0
--- S-invalid-utf8            (0xff 0xfe in the file)               rc=0 VALID=True diagnostics=0
--- S-bom                     (UTF-8 BOM before ---)                rc=0 VALID=True diagnostics=0
--- S-not-named-SKILL.md      (path -> skills/review/OTHER.md)      rc=0 VALID=True diagnostics=0
```
whereas layer 1 catches them:
```
$ muse skills validate pkgs/S-no-frontmatter/skills/review --json
{"error":{"code":"invalid-skill-package","message":"SKILL.md must start with YAML frontmatter", …}}
$ muse skills validate pkgs/S-not-named-SKILL.md/skills/review --json
{"error":{"code":"invalid-skill-package","message":"skill package must contain SKILL.md", …}}
$ muse skills validate .../skills/review/SKILL.md --json          # a FILE, not a dir
{"error":{"code":"invalid-skill-package","message":"skill package must be a directory", …}}
```
Note the argument is the skill **directory** — a generator must pass `dirname(entry.path)`.

The cost of skipping layer 1 is silent degradation, not failure. The broken package installs
clean and the skill shows up with a placeholder description:

```
$ muse plugins install pkgs/S-no-frontmatter --json    -> warning: null, diagnostics: []
$ muse skills list | grep probe-plugin
plugin:probe-plugin:review  plugin  on   Plugin skill from probe-plugin   plugin://probe-plugin/skills/review/SKILL.md
# vs. the well-formed one:
plugin:probe-plugin:review  plugin  off  Review the current workspace.    plugin://probe-plugin/skills/review/SKILL.md
```

### 1.14 Correction-round limits

`maxCorrectionRounds: 3`, `terminalFailedResult: 4`, `validation.order: [nestedSkills, plugin]`,
`warningsAccepted: false` are **authoring policy in `capability-examples.json`**, enforced by the
skill's prose, not by the binary. The binary has no notion of a correction round, and it exits 0
on a warning. A generator that treats `rc == 0` as success will happily ship a
`summary: "partial"` plugin.

---

## 2. CONTRACT vs. VALIDATOR — every disagreement

Ordered by how expensive the surprise is.

### D1 — "The validator rejects direct capability keys `tools`, `agents`, `outputStyles`, `settings`, `apps`" — only when the plugin would have zero supported capabilities

Contract §Unsupported Families states this flatly; `capability-examples.json` lists them under
`unsupportedFamilies`. Reality: with ≥1 supported capability they are **warnings** and the package
is `valid: true`, `rc = 0`, `compatibility.summary: "partial"`. With an empty array they produce
**no diagnostic at all**. Generator impact: a naive "did it exit 0" check ships a plugin whose
`agents`/`tools` block was silently dropped. Evidence: §1.11.

### D2 — `compat.source` is never validated

Contract's required manifest base pins `"source": "native"`. Measured:
```
compat:{"manifestDir":".muse-plugin"}                       -> VALID   (no source at all)
compat:{"source":"claude","manifestDir":".muse-plugin"}     -> VALID   (wrong source)
compat:{"source":123,"manifestDir":".muse-plugin"}          -> VALID   (not even a string)
compat:{"source":"native"}                                  -> manifest-family-mismatch: must declare compat.manifestDir
```
Emit it anyway for forward compatibility, but never branch on it.

### D3 — `plugins validate` does not check SKILL.md at all

Contract §Skills: "The target is a UTF-8 `SKILL.md` with valid frontmatter." The plugin validator
enforces none of it — not the filename, not the frontmatter, not UTF-8, not the BOM. Only
`muse skills validate` does, and only when handed the skill **directory**. Evidence: §1.13.
This is the reason the contract's `validation.order` is `[nestedSkills, plugin]`; the contract
never says *why*, and a generator that "optimises away" layer 1 loses all skill checking.

### D4 — Windows-reserved basenames are an authoring rule the validator does not enforce

Contract §Identifiers and `idRules.windowsReservedBasenames` list `CON PRN AUX NUL COM1–9 LPT1–9`
and give `con`, `CON.txt`, `lpt1.log` as `rejectedExamples`. Measured: `con`, `prn`, `aux`, `nul`,
`com1`, `com9`, `lpt1`, `lpt9`, `con.txt`, `lpt1.log` **all VALID**. (`CON` is rejected only
because it is uppercase.) Keep the generator's own check — it is a portability rule with no
enforcement behind it.

### D5 — reserved plugin ids are not rejected by `validate`

`loop`, `muse-core`, `tbh-reminders` all pass `plugins validate` with zero diagnostics
(and pass `install` too, per `re/plugins.md` §3.2, becoming inert with
`bundled_plugin_id_reserved`). Also: `native-plugin-contract.md` names only `loop` and `muse-core`;
`capability-examples.json` names three. Trust the JSON.

### D6 — `decision.deliveryRole` is optional, not required

Contract §Reminders: "`decision` is required, including its V1 `envelope` and `deliveryRole`
authority." Measured: dropping `envelope` → `decision reminder envelope is required`; dropping
`deliveryRole` → **VALID, 0 diagnostics**.

### D7 — six more `decision` members ARE required, and the contract elides them

The contract's example replaces them with `"...":"copy the remaining executable fields from
capability-examples.json"`. `fields`, `bodyTemplate`, `validators`, `proposal`, `lifecycle`,
`limits` are each a hard `missing field` error. A generator cannot author a reminder without
copying the whole ~7 KB decision block; the reference manifest below is 7666 bytes almost entirely
because of it.

### D8 — the reminder `context` object is a full sub-schema the contract does not describe

Contract lists `context` among "optional policy fields … add them only when requested". It is a
closed two-key object with a feed schema carrying seven fields, six numeric/enum constraints and
two aggregate budgets. Evidence: §1.10.2. Anything not in that shape is a hard error.

### D9 — the reminder cross-field invariants are undocumented

Five invariants tie `blocking`, `maxInstallsPerRun`, `lifecycle.eotProgressGate`,
`lifecycle.failureFallback` and `lifecycle.installBudget` together (§1.10.1). Copying the
example verbatim and merely flipping `"blocking": true` fails:
`decision eotProgressGate=notApplicable requires blocking=false`.

### D10 — the contract's hook field list is a strict subset of a CLOSED field set

Contract §Hooks shows `{id, event, command, timeoutMs, statusMessage}`. The validator also accepts
`async`, `compatibilityName`, `outputCapabilities` — and rejects **everything else as a hard
error**. Hooks are the only family with a closed field set; skills, commands and mcpServers all
swallow unknown keys. A generator must special-case hooks.

### D11 — `enabledDefault` defaults to `true`, and only literal `false` disables

Contract: "`enabledDefault` is optional; omit it only when the requested activation behavior is
clear," and its skills example uses `false` while its commands example uses `true`, implying they
differ. They do not: omitted → `true` for both families. Worse, `0`, `"yes"` and `"false"` all
read as `true` (no type error). Only JSON `false` works. Evidence: §1.6.

### D12 — "Reject … backslashes" produces the wrong error, or kills the package

Contract §Paths. A backslash in a manifest path is treated as a literal filename character, so the
user gets `plugin file is not readable`. If the file really exists with that name, the whole
package fails with `invalid-plugin-package :: Agent Definition inventory derivation failed closed`
— a message that names neither the path nor the backslash. Same opaque message for **any** symlink
in the package, referenced or not.

### D13 — "Canonicalize containment" understates the path rule

Contract §Paths says to canonicalize. The validator refuses any `..` segment even when it
canonicalizes back inside the root (`skills/x/../review/SKILL.md` → `must stay inside plugin root`).
Generators must emit already-normalised paths, not rely on canonicalization.

### Contract claims that DO hold, verified

- `.muse-plugin/plugin.json`, exactly one manifest per package (§1.1)
- id grammar `^[a-z0-9][a-z0-9._-]{0,79}$`, 1..80 bytes, for plugin AND capability ids (§1.3)
- relative-only, non-empty, no-traversal, symlink-escape rejection, files must pre-exist (§1.4)
- skills/commands `{id, path, enabledDefault?}`; hooks `command` is structured argv, not a shell
  string; hook source paths cannot be shared by two hook ids (§1.6–1.8)
- mcpServers `transport` defaults to `stdio`; http requires a non-empty `url` (§1.9)
- reminders require `decision` including a V1 `envelope` (§1.10)
- "A result is clean only when the process starts, exits zero, returns parseable JSON, sets
  top-level `valid` to `true`, and returns an empty `diagnostics` array" — exactly right, and
  §1.11/§1.14 show why the `diagnostics == []` clause is the one that matters
- "Use only fields required by the request" — correct, because unknown top-level and
  unknown `capabilities` keys both emit `unsupported-field` warnings

---

## 3. THE REFERENCE PACKAGE

Saved at `.../scratchpad/settle/plugin-contract/artifacts/ohmy-reference/`
(manifest also copied standalone to `.../artifacts/reference-plugin.json`, 7666 bytes).

```
artifacts/ohmy-reference/
  .muse-plugin/plugin.json
  skills/review/SKILL.md
  commands/summarize.md
  hooks/pre-check.sh          (chmod 755)
  mcp/server.py
  reminders/review-policy.md
```

Manifest shape (the `decision` block is `capability-examples.json`'s `mixedExample` verbatim):

```json
{
  "schemaVersion": 1,
  "name": "ohmy-reference",
  "displayName": "Oh My Reference",
  "version": "0.1.0",
  "description": "Reference native plugin exercising every supported capability family.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "skills":     [ { "id": "review",    "path": "skills/review/SKILL.md",  "enabledDefault": false } ],
    "commands":   [ { "id": "summarize", "path": "commands/summarize.md",   "enabledDefault": true  } ],
    "hooks":      [ { "id": "pre-check", "event": "PreToolUse",
                      "command": ["sh", "hooks/pre-check.sh"],
                      "timeoutMs": 1000, "statusMessage": "Checking plugin policy" } ],
    "mcpServers": [ { "id": "workspace-index", "transport": "stdio",
                      "command": ["python3", "mcp/server.py"] } ],
    "reminders":  [ { "id": "review-policy", "path": "reminders/review-policy.md",
                      "tools": ["read_file"], "blocking": false,
                      "decision": { …the 9-member V1 declaration… } } ]
  }
}
```

Results:

```
### LAYER 1: muse skills validate .../skills/review --json
{"valid": true, "id": "review", "files":[{"relative_path":"SKILL.md",
 "sha256":"sha256:450b328d1ef1ce028dcb9d545bbd371d30d4e318bd667ee1ace177ced15783f8","bytes":151}],
 "diagnostics": [],
 "compatibility": {"profile":"agent-skills-common-subset","result":"compatible",
                   "known_fields":["description","name"],"unknown_fields":[],
                   "unsupported_fields":[],"allowed_tools":[]}}
rc=0

### LAYER 2: muse plugins validate .../ohmy-reference --json
valid= True  diagnostics= []  compatibility.summary= full
rc=0

### declarations
skill:review supported | hook:pre-check supported | mcp:workspace-index supported
command:summarize supported | reminder:review-policy supported

### install (executed once for evidence, in a throwaway HOME)
installed: ohmy-reference  family=native  diagnostics=[]
warning: "third-party plugin: hooks, reminders, and MCP servers require review before activation;
          skills and commands are active without review while the plugin is enabled"
effective:   command plugin:ohmy-reference:summarize
             skill   plugin:ohmy-reference:review
runtime:     3 x review_needed   (hook, mcp_server, reminder)
```

---

## 4. FAILED ATTEMPTS / DEAD ENDS

Enumerated so nobody repeats them.

1. **`{"tools": []}` as an unsupported-family probe.** Returned `valid: true, diagnostics: []`,
   which reads as "tools is supported". It is not — the family gate only fires on a non-empty
   array. Had to re-probe with entries.
2. **Probing `agents`/`outputStyles` with a path to a file that did not exist.** Both returned
   `missing-capability-path :: plugin file is not readable`, masking the real
   `unsupported-capability` verdict. The path check runs first; the files must exist before the
   family gate is reached.
3. **Probing `apps` with a string array** (`"apps": ["apps/x"]`) →
   `plugin apps capability must be an object`; with an object map →
   `plugin apps capabilities must be an array`; with `{"id","path"}` pointing at a directory →
   `plugin path must be a file`. Three dead ends before `{"id":"a","path":"apps/a.json"}` reached
   the real `unsupported-capability`.
4. **Reading a "backslash rejected" diagnostic.** There is none. `skills\review\SKILL.md` yields
   `plugin file is not readable`; creating the literal file yields
   `invalid-plugin-package :: Agent Definition inventory derivation failed closed`, whose
   `diagnostics` array repeats only that one line with the package root as `path`. No message
   anywhere in the binary's strings mentions a rejected backslash in a manifest value.
5. **Looking for the reminder `context` schema by probing plausible key names**
   (`hostFeeds`, `host`, `budget`, `maxTokens`, `sources`, `memory`, `files`, `skills`, `paths`,
   `memories`) — all ten returned `context field \`X\` is not supported` with no hint of the real
   names. The schema only fell out after `strings` on the binary surfaced
   `context feed root`, `context feed \`placement\` must be …`, `placementindexFileinlineMaxBytes
   totalMaxBytes`, and `reminder context \`feeds\` must be an array`.
6. **`validators: [{"id": "reminder-validator/memory_body/v1"}]`** — the four/five built-in
   validator contract ids in `re/plugins.md` §3.7 are NOT bound by an `id` key.
   `ValidatorBinding` wants `name` first: `decision declaration is invalid: missing field \`name\``.
   The full binding shape was not pursued further; `"validators": []` is what the reference uses.
7. **`muse skills validate <path-to-SKILL.md>`** → `skill package must be a directory`. The verb
   takes the containing directory.
8. **Peeling script bug.** The first ordering script used `m["capabilities"].pop("tools") or m`,
   which returns the popped list; it crashed after six steps. The order for steps 1–6 in that run
   is still valid and is what §1.12 quotes; the remaining steps were re-derived with targeted
   pairs instead of a peel.
9. **Assuming `enabledDefault` is type-checked.** `"yes"` and `0` are accepted and both mean
   `true`. There is no diagnostic. Only discovered by reading `enabled_default` back out of the
   validator's descriptor rather than trusting `valid: true`.
10. **`timeoutMs` range probing.** `0`, `-1`, `"1000"`, `99999999` are all accepted at validate
    time. Whatever clamps them lives at hook-run time, not in the manifest validator; not pursued.

## 5. NOT SETTLED

- The exact `ValidatorBinding` shape for `decision.validators[]` (only that it needs `name`, and
  from the binary's serde table `id`, `inputs`, `outputs`). Not needed for a generator that emits
  `"validators": []`.
- Whether `context.feeds[].indexFile` is required to exist at **install** time (it is not required
  at validate time).
- The `honored-declaration` diagnostic code was never observed on a native manifest.

---

## Verification

Adversarial re-run by a second agent, from a **clean sandbox**
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/verify-plugin-contract/`
(fresh `home/`, `xdgconfig/`, `xdgdata/` — no reuse of the original sandbox, no reuse of its
`probe.py`; an independently written harness of the same shape lives at
`.../verify-plugin-contract/probe.py`). Binary sha256
`b9c7f9badb6b2af1b362d30202b366e7bdc13b3c4048e9002caa236cc56c54a4`, `Muse Code 1.0.1 (1.0.1-R2006.1)`.
~230 packages built under `.../verify-plugin-contract/pkgs/`. No login, no network, no `--provider`.

**Verdict: CONFIRMED WITH CORRECTIONS.** The specification is reproducible end to end and every one of
the 13 disagreements re-measured true. Seven claims needed correction — one of them (C1) refutes a
load-bearing sentence of the summary answer and adds a **14th** disagreement and a **sixth**
silent-breakage mode. Four items the report left open or merely cited are now settled.

### Positive control — it really passes, on a fresh HOME

```
$ muse skills  validate .../verify-plugin-contract/artifacts/ohmy-reference/skills/review --json
{"valid": true, "id": "review", "diagnostics": [],
 "files":[{"relative_path":"SKILL.md","sha256":"sha256:450b328d1ef1ce028dcb9d545bbd371d30d4e318bd667ee1ace177ced15783f8","bytes":151}],
 "compatibility":{"profile":"agent-skills-common-subset","result":"compatible", ...}}   rc=0
$ muse plugins validate .../verify-plugin-contract/artifacts/ohmy-reference --json
valid=True  diagnostics=[]  plugin.compatibility.summary="full"                          rc=0
  declarations: skill:review / hook:pre-check / mcp:workspace-index / command:summarize /
                reminder:review-policy   — all "supported"
$ muse plugins install ... --json          (throwaway HOME)
  installed.manifest_family "native", diagnostics [], warning exactly as quoted in §3
$ muse plugins inspect ohmy-reference --json
  valid true, active true, diagnostics [], capability_diagnostics []
  effective_capabilities: command plugin:ohmy-reference:summarize, skill plugin:ohmy-reference:review
  runtime_capabilities:   hook / reminder / mcp_server — 3 x "review_needed"
```
The sha256 of `SKILL.md` matches the report byte for byte. Note also that the whole thing is gated on
`MUSE_EXPERIMENTAL_PLUGINS=1`: without it, `muse plugins --help` prints
`plugins are not available in this build` and there is no validator at all.

### Reproduced verbatim (no change needed)

Top-level requiredness table incl. `displayName` as the sole optional field, `""`/`null` handling and
`schemaVersion` rejecting `"1"`, `0`, `2` **and `1.0`**; the four `unsupported-field` top-level
warnings · the ID grammar including the 80/81 boundary and legal trailing `.`/`-`/`_` · every path
rule, incl. `..` being **lexical** (`skills/x/../review/SKILL.md` rejected) and `./`, `//`, `/./`
normalising · D1's three states, re-measured across all six unsupported families · D2 (`compat.source`
accepts `claude`, `123`, `null`, or absence) · D4 (all ten Windows-reserved basenames VALID) ·
D5 (`loop`, `muse-core`, `tbh-reminders` VALID) · D6/D7 (`deliveryRole` optional; the other **eight**
`decision` members each a hard `missing field`, `version` included) · D8, every boundary of the
`context` sub-schema (1..8000, 512..24000, ≤8 feeds, 24000 aggregate, `digest`, placement enum,
canonical roots, lowercase `.md`) · D9's five invariants · D10's closed hook field set — I re-probed
33 candidate names and got the identical 8 allowed / 24 rejected split plus the `matcher` special
message · D11 (`0`, `1`, `"yes"`, `"false"`, `null` all mean `true`; only literal `false` disables;
identical for commands) · D12/D13 · mcpServers silently dropping `env`/`cwd`/`headers`/`args`/
`timeoutMs`/`enabledDefault`/unknown keys, read back out of the descriptor · the whole duplicate /
cross-family collision matrix · manifest discovery (`.muse` custom dir and root-only both
`missing-manifest`; two manifests → `multiple-manifests`) · the validation order peel, steps 1–5
identical · `timeoutMs` unchecked (`0`, `-1`, `"1000"`, `1.5`, `99999999`) · `outputCapabilities`
matrix · argv rules and `duplicate-hook-source` (absolute argv shared is fine) · all 17 events.

The symlink claim got the **negative control the original run lacked**: a stray *regular* unreferenced
file (`alias.txt` + `real.txt`, copies not links) is `VALID, diagnostics 0`, while the same layout with
`alias.txt` as a symlink dies. So the symlink genuinely is the cause, in every position tested —
referenced-and-internal, stray, escaping, **broken**, **directory symlink**, and **inside
`.muse-plugin`** — all six with the identical `invalid-plugin-package :: Agent Definition inventory
derivation failed closed`.

### Corrections

**C1 — `compat.manifestDir` is NOT restricted to `.muse-plugin`; the directory silently selects the
family.** This refutes "compat.manifestDir (must be literally `.muse-plugin`)" in the answer and
§1.1's "exactly one legal value". The rule the validator actually enforces is only
*`compat.manifestDir` must equal the directory the manifest was found in*, and all three discovery
directories are legal:

```
manifest bytes identical except compat.manifestDir, package otherwise the reference package:
  .muse-plugin/plugin.json    -> valid true, diagnostics [], summary full, manifest_family "native"
  .codex-plugin/plugin.json   -> valid true, diagnostics [], summary full, manifest_family "codex-compatible"
  .claude-plugin/plugin.json  -> valid true, diagnostics [], summary full, manifest_family "claude-compatible"
all three: 5/5 declarations "supported" (skill, hook, mcp, command, reminder)
```
and the families are **not** semantically identical:
```
enabledDefault   .muse-plugin      .codex-plugin      .claude-plugin
absent        -> enabled_default True   True    None
true          -> True                   True    None
false         -> False                  False   None
```
So a packer that writes the manifest into `.claude-plugin` (trivial when porting a Claude plugin, and
`compat.source` is unvalidated so nothing else objects) ships a plugin that validates **clean and
"full"** and then ignores every `enabledDefault` in it. Call this **D14**, and a sixth
silently-broken-but-valid mode. The report's failed-attempt #11 was right that `.muse` fails — but the
reason is discovery, and the generalisation to "single legal value" does not hold.
`omm doctor` must assert `plugin.manifest_family == "native"`, not just `valid && diagnostics == []`.

**C2 — layer 1 does not catch two of the six D3 cases; those two are caught by *neither* layer.**
§1.13's "whereas layer 1 catches them" over-reaches. Re-measured, `muse skills validate <dir>`:
```
S-no-frontmatter          plugins: VALID   skills: invalid-skill-package :: SKILL.md must start with YAML frontmatter
S-fm-missing-description  plugins: VALID   skills: invalid-skill-package :: SKILL.md frontmatter must include description
S-invalid-utf8            plugins: VALID   skills: invalid-skill-package :: SKILL.md must be valid UTF-8
S-not-named-SKILL.md      plugins: VALID   skills: invalid-skill-package :: skill package must contain SKILL.md
S-bom (UTF-8 BOM)         plugins: VALID   skills: valid true, diagnostics []      <-- NOT caught
S-fm-name-mismatch        plugins: VALID   skills: valid true, diagnostics []      <-- NOT caught
(new) S-empty-file        plugins: VALID   skills: SKILL.md must start with YAML frontmatter
(new) S-binary-file       plugins: VALID   skills: SKILL.md must start with YAML frontmatter
```
Both uncaught cases then install and run *correctly* (`muse skills list` shows description `d` for
each), so the BOM is harmless and the frontmatter `name` is simply ignored — the capability id wins.
Design consequence: name↔id consistency is a **generator-only** check; no Muse verb will ever flag it.

**C3 — the degradation is not silent; `muse skills list` reports it.** After installing
`S-no-frontmatter`, the malformed skill surfaces as a structured diagnostic on **stdout**, both
per-skill and at the top level of `skills list --json`:
```json
{"id":"plugin:probe-plugin:review","description":"Plugin skill from probe-plugin","activation":"on",
 "diagnostics":[{"code":"invalid-skill-package",
   "message":"skill file at <cache>/package/skills/review/SKILL.md is malformed: SKILL.md must start with YAML frontmatter",
   "scope":"plugin","path":"plugin://probe-plugin/skills/review/SKILL.md"}]}
```
`muse plugins inspect probe-plugin --json` does **not** show it (`diagnostics: []`,
`capability_diagnostics: []`). So there is a third checkpoint the report missed, and it is the *only*
one that works on an already-installed plugin: `omm doctor` should read
`muse skills list --json | .diagnostics` post-install, not `plugins inspect`.

**C4 — sibling diagnostics are at `error.details.diagnostics`, not `error.diagnostics`.**
§1.12's "`error.diagnostics` may carry siblings" names a key that does not exist; the original
`probe.py` reads `e.get("diagnostics")` on the error object, which is why no run ever printed one.
Two bad capability ids:
```json
{"error":{"code":"invalid-capability-id","message":"skill capability id `BAD` is invalid",
  "details":{"valid":false,"diagnostics":[
     {"code":"invalid-capability-id","severity":"error","message":"skill capability id `BAD` is invalid", ...},
     {"code":"invalid-capability-id","severity":"error","message":"skill capability id `ALSOBAD` is invalid", ...}]}}}
```

**C5 — the reminder `decision` block is ~3 KB, not ~7 KB, and ~43 reminders fit, not ~15.**
The 7666 bytes is the *whole reference manifest*. Measured on the reference `decision`:
compact 2962 B, `indent=2` 4649 B; the whole reminder entry compact is 3071 B. Empirically:
```
40 reminders -> manifest 120630 B  VALID
42 reminders -> manifest 126652 B  VALID
43 reminders -> manifest 129663 B  VALID
45 reminders -> manifest 135685 B  plugin manifest exceeds 131072 byte limit
```
so the ceiling is **43 compact / ~25 pretty-printed**. The design-impact byte-counting advice stands;
the number in it was ~3x pessimistic.

**C6 — `compatibility.summary == "full"` is not a safe blanket gate.** A plugin that is valid and
declares no supported capability reports `summary: "unsupported"` with `valid: true, diagnostics: []`:
`capabilities: {}`, `{"tools": []}`, `{"skills": []}` all do. Conversely an *empty* unsupported array
alongside a real capability is invisible in every field — `{"skills":[G],"tools":[]}` →
`valid true, diagnostics 0, summary "full"`. So the assertion is `summary == "full"` **only for a
plugin that declares ≥1 supported capability**; a capability-less package needs a different rule.

**C7 — two smaller additions to the "emittable key set" rule.**
`$schema` inside `.muse-plugin/plugin.json` is an `unsupported-field` warning even when set to the
Agent Plugins 1.0.0 URL — worth naming explicitly because every ported Claude manifest carries it.
And unknown keys inside `compat` are **silently accepted with no warning at all**
(`compat: {"source","manifestDir","zzz"}` → `valid true, diagnostics 0`), unlike unknown keys at top
level and inside `capabilities`. The `compat` object is a third, open, undiagnosed key space.

### Items the report left open, now settled

**§5.1 — the `ValidatorBinding` shape is RESOLVED.** The serde-enumeration trick that cracked
`decision` works here too (`validators: [{"name":"x","zzz":1}]`):
```
unknown field `zzz`, expected one of `name`, `id`, `version`, `inputs`, `outputs`   (all five REQUIRED)
name    : decision identifier grammar  ("Bad Name" -> validator name `Bad Name` must use the decision identifier grammar)
id      : the BARE contract name — "memory_body", NOT "reminder-validator/memory_body/v1"
          (the slashy form -> validator `mb` must declare a valid id and positive version)
version : positive integer; 2 -> references unavailable host contract `memory_body` version 2
inputs  : a MAP (a list -> invalid type: sequence, expected a map) whose key set must equal the host contract's
outputs : a MAP whose keys AND type strings must equal the host contract's
```
The binary prints the host contracts when they mismatch, so all five are now known:
```
memory_body        inputs ["advisory_text","refs"]                      outputs {"advisory_block":"string","proposal_advisory":"string","rendered_refs":"string"}
goal_next_step     inputs ["next_step"]
verify_next_step   inputs ["next_step"]
skill_read_ledger  inputs ["advisory_text","confidence","reason","skill_id"]
memory_source_refs (reached only after `decision none rejection must use literal `no_remind``)
```
A binding that **validates clean** (rc=0, `diagnostics: []`) — the report's `"validators": []` is not
the only option:
```json
"validators": [{ "name": "mb", "id": "memory_body", "version": 1,
  "inputs":  {"advisory_text": {"field": "advisory_text"}, "refs": {"literal": []}},
  "outputs": {"advisory_block":"string","proposal_advisory":"string","rendered_refs":"string"} }]
```
`refs` is typed `memoryEvidence` on the host side and no plugin field can carry it — the declared
field shapes are exactly `string|integer|boolean|array|object` — so it must be bound to a literal.

**§5.2 — `context.feeds[].indexFile` is not required to exist at install time either.** The
`feed-index-missing-on-disk` package installs with `diagnostics: []`, inspects `valid true`, and its
reminder can be `plugins approve`d, all with a feed pointing at `nope/does-not-exist.md`.

**Package limits, previously cited from `re/plugins.md`, now measured.**
```
manifest bytes   131072 -> VALID      131073 -> plugin manifest exceeds 131072 byte limit   (exact boundary)
fs entries        4096  -> VALID       4097  -> invalid-plugin-package :: Agent Definition inventory derivation failed closed
path depth      16 segs -> VALID     17 segs -> invalid-plugin-package :: Agent Definition inventory depth limit exceeded
```
Depth gets its **own** message; the entry cap reuses the same opaque `derivation failed closed` line
as symlinks and backslashes, so that one string now covers *three* distinct hygiene failures.

**Working `blocking: true` reminders exist** (the report stopped at the failure). The gate lattice:
```
eotProgressGate=notApplicable            <=> blocking=false                                     (reference package)
eotProgressGate=requireNewToolProgress   <=> blocking=true                                      VALID
eotProgressGate=decisionWithFiniteBudget  => blocking=true AND installBudget=finiteRequired
                                            AND a finite nonzero maxInstallsPerRun              VALID
```
verbatim: `requireNewToolProgress` + `blocking:false` → `eotProgressGate=RequireNewToolProgress
requires blocking=true`; `decisionWithFiniteBudget` + `roster` + `blocking:true` →
`requires installBudget=finiteRequired`; + `finiteRequired` without a budget →
`requires blocking=true and a finite nonzero maxInstallsPerRun`; with `maxInstallsPerRun: 3` → VALID.

**Package hygiene really is last** — proven by pairing a symlink against each earlier stage; the
symlink loses every time:
```
symlink + schemaVersion 2      -> invalid-manifest-schema
symlink + missing capability   -> missing-capability-path
symlink + unsupported family   -> unsupported-capability
symlink + duplicate ids        -> duplicate-capability-id
```

Smaller additions: a **backslash in the name of a file nothing references** also kills the package
(`weird\name.txt` present, manifest clean → `Agent Definition inventory derivation failed closed`), so
D12 is a filename rule, not a manifest-value rule · inside a hook entry the unknown-field check
precedes the event check (`{event:"Setup", zzz:1}` → `unsupported-field`) · `developerPrompts` entries
are `{id, text}`, not `{id, path}` (`developerPrompt capability `x` must declare non-empty `text``) ·
malformed JSON has its own code, `invalid-manifest-json` (`plugin manifest is not valid JSON: key must
be a string at line 1 column 2`) · a skill `path` may point at any file at all — `hooks/h.sh`
validates as a skill.

### What the verifier tried that did not pay off

1. Probing the unsupported families with `{"id","path"}` entries pointed at real files — `developerPrompts`
   errored `must declare non-empty `text`` before the family gate, masking the verdict, exactly as the
   original run was masked by nonexistent files. Re-probed with `{"id","text"}` to get `partial`.
2. Guessing the root-`plugin.json` discovery path with an invented
   `https://schemas.tbh.meta.com/agent-plugins/1.0.0/plugin.json` `$schema` → `missing-manifest`. No
   `agent-plugins` URL appears anywhere in the binary's printable strings, so the "exact Agent Plugins
   1.0.0 $schema" branch of the discovery message is still unexercised.
3. Binding a validator with `inputs`/`outputs` as **arrays** — `invalid type: sequence, expected a map`.
   Then with `outputs` keyed to guessed value strings (`{"advisory_block":"advisory_block"}`) — the
   value must be the literal type name `"string"`. Then `{"effective":"memoryEvidence"}` as an input
   source → `data did not match any variant of untagged enum DecisionValueSource`; and a field of shape
   `{"type":"memoryEvidence"}` → `unknown variant, expected one of string, integer, boolean, array,
   object`. Only `{"literal": []}` satisfies a `memoryEvidence` input.
4. Trying `array` and `object` field shapes on the way there: both are closed sub-schemas of their own
   (`missing field `minItems``, `missing field `fields``). Not pursued — no generator needs them yet.
