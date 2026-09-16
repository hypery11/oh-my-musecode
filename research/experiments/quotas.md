# Settled: plugin capability quotas — how big can a shipped bundle be?

**Subject:** Meta Muse Code `1.0.1-R2006.1`, build `e27e408b66`, binary
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
**Sandbox:** `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/settle/quotas/`
(`HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` all under it; `MUSE_NO_AUTO_UPDATE=1`,
`MUSE_EXPERIMENTAL_PLUGINS=1`; provider `echo` only; never authenticated).
**Date:** 2026-09-02.

Verdict: **PARTIAL.** Every number that actually bounds a shipped bundle is now measured and
reproducible. One of the three named diagnostic codes (`plugin_preflight_overflow`) was
observed firing and its trigger pinned to an exact integer. The other two
(`plugin_scope_quota`, `plugin_class_overflow`) could not be made to fire by any volume of
capabilities — see §7 for the full list of what was tried.

---

## 0. Answer in one screen

| Bound | Value | Where it bites | Failure mode |
|---|---|---|---|
| **Manifest file size** | **131 072 bytes** (inclusive) | `plugin.json` | hard error `invalid-plugin-package: plugin manifest exceeds 131072 byte limit`, at `validate` and `install` |
| **Agent-definition inventory budget, per package** | **4 094 units** | one plugin | hard error `invalid-plugin-package: Agent Definition inventory derivation failed closed`, at `validate` and `install` |
| unit cost | skill = **2**, command/hook/mcpServer/reminder = **1**, plus **1 per non-empty capability class** | | |
| **Enabled installed plugins** | **256** | the whole user scope | soft: `capability_diagnostics = [{code: agent_definition_review_unavailable, message: "agent definitions are not reviewable and stay inactive: plugin_preflight_overflow"}]` — skills, commands, hooks and MCP keep working |
| **Skills catalog context block** | **32 000 bytes** | one session, all skill sources together | silent per-entry degradation, then silent truncation; a CLI warning line only in the truncation stage |

Per-class maximum for a **single** plugin, everything else empty:

| class | max in one plugin | what binds it |
|---|---|---|
| `skills` | **2 046** | the 4 094-unit inventory budget (1 + 2×2046 = 4093) |
| `commands` | **4 093** | the 4 094-unit inventory budget (1 + 4093 = 4094) |
| `hooks` | **2 153** | the 131 072-byte manifest cap (mb = 131 098 at 2154) |
| `mcpServers` | **2 902** | the 131 072-byte manifest cap (mb = 131 088 at 2903) |
| `reminders` | **43** | the 131 072-byte manifest cap — a reminder's mandatory `decision` block is ~3 049 bytes on its own |

**The practical maximum for a shippable curated bundle is not any of those numbers.**
It is the skills-catalog budget, and for a *useful* bundle (every skill still carrying a
description the model can route on) it is:

```
N_max = floor( (32000 − 363 − 35 − bundled_bytes − other_scope_bytes)
               / (id_only_line_bytes + description_bytes + 36) )
```

With Muse's 15 built-in skills left enabled and no project skills, that is
**21 542 bytes** of room, i.e. **82 bundle skills at 120-byte descriptions, 63 at 200-byte,
39 at 400-byte.** Disabling all 15 built-ins raises those to 121 / 92 / 58 (all three measured).

---

## 1. Instruments used

Two oracles, both fast and both offline.

**a. `muse plugins inspect <id> --json`** composes the real capability snapshot
(`"effective_capabilities_scope": "plugin-capability-snapshot"`) and returns
`effective_capabilities[]`, `capability_diagnostics[]`, `runtime_capabilities[]`.
This is the only CLI surface that surfaces a capability-snapshot diagnostic.

**b. `muse exec --provider echo "hi"`**, then the `model_request_configured` event in
`$XDG_DATA_HOME/muse/sessions/YYYY/MM/DD/<uuid>/session.jsonl` →
`run_context_messages[] where id == "skills_catalog"`. This is the byte-exact text the model
would receive. Helper: `settle/quotas/dump.py`.

`$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-<uuid>.log` was read on every run and is
**useless for this question**: under `mode="exec"` it stops at `credential.status` (13 lines)
and never emits a `tbh.local.catalog` plugin-composition record. Under
`mode="plugin_mutation"` (i.e. `plugins install`) it emits 49 lines of `gate.resolve` and
`path.resolved` and one `trust.resolve`, and nothing about plugin capability counts. Grepping
every file under `$XDG_DATA_HOME/muse/` for `plugin_scope_quota|disabled_plugin|
higher_precedence|nearest_trusted_project|not_trusted_enabled|inventory_refresh_required`
returns **nothing**, ever — including with a plugin deliberately disabled.

---

## 2. The 131 072-byte manifest cap  (PROVEN, exact)

`settle/quotas/bytecap.py` pads the manifest `description` to hit an exact byte length:

```
bytes=131070   ok=True  valid diags=0
bytes=131071   ok=True  valid diags=0
bytes=131072   ok=True  valid diags=0
bytes=131073   ok=False plugin manifest exceeds 131072 byte limit
bytes=131074   ok=False plugin manifest exceeds 131072 byte limit
```

This closes plugins.md open question #4's byte half: the format string
`plugin manifest exceeds \n byte limit` renders **131072**. (The sibling
`plugin \n directory contains more than \n filesystem entries` and
`plugin \n directory nesting exceeds the maximum depth of \n` were not reached — 4 093 files
in one plugin dir installs cleanly.)

---

## 3. The per-package inventory budget  (PROVEN, exact, with a fitted cost model)

Overflow is a **hard error at `plugins validate`**, before install, before any snapshot:

```json
{"error":{"code":"invalid-plugin-package",
          "message":"Agent Definition inventory derivation failed closed",
          "details":{"valid":false,"plugin":null,
                     "capabilities":{"skills":[],"hooks":[],"mcp_servers":[],"commands":[],"reminders":[]},
                     "diagnostics":[{"code":"invalid-plugin-package","severity":"error",
                                     "message":"Agent Definition inventory derivation failed closed"}]}}}
```

Nothing is partially loaded; `plugin` is `null` and every capability array comes back empty.
`plugins install` refuses with the same payload and exit 1.

### 3.1 Single-class bisections (dense manifests, `settle/quotas/gen2.py` + `bisect.py`)

```
skills      LAST_GOOD=2046  FIRST_BAD=2047   (mb 73252 → 73289; nowhere near the byte cap)
commands    LAST_GOOD=4093  FIRST_BAD=4094   (mb 124433 → 124464)
hooks       LAST_GOOD=2153  FIRST_BAD=2154   (mb 131011 → 131098; BYTE cap, not the inventory)
mcpServers  LAST_GOOD=2902  FIRST_BAD=2903   (mb 131042 → 131088; BYTE cap)
reminders   LAST_GOOD=43    FIRST_BAD=44     (mb 130948 → 133990; BYTE cap)
```

### 3.2 Fitting the unit cost (`settle/quotas/genmix2.py`)

Method: hold a small base of one class, bisect the `commands` axis, and read the number of
units the base consumed as `4093 − max_commands`.

```
base={}                                 -> commands 4093    (0 units consumed)
base={'skills':1}                       -> commands 4090    (3)
base={'skills':2}                       -> commands 4088    (5)
base={'skills':4}                       -> commands 4084    (9)
base={'hooks':1}                        -> commands 4091    (2)
base={'hooks':2}                        -> commands 4090    (3)
base={'hooks':4}                        -> commands 4088    (5)
base={'mcpServers':1}                   -> commands 4091    (2)
base={'mcpServers':2}                   -> commands 4090    (3)
base={'mcpServers':4}                   -> commands 4088    (5)
base={'skills':1,'hooks':1,'mcpServers':1} -> commands 4086 (7 = 3 + 2 + 2)
```

and against the `skills` axis, for reminders:

```
base={'reminders':1} -> skills 2045
base={'reminders':2} -> skills 2045
base={'reminders':4} -> skills 2044
```

The unique consistent model:

```
units = Σ_class (1 if the class array is non-empty)
      + 2 × n_skills
      + 1 × (n_commands + n_hooks + n_mcpServers + n_reminders)
budget: units ≤ 4094
```

Cross-checks, all exact:
`skills only`  1 + 2×2046 = 4093 ≤ 4094 ✔, 1 + 2×2047 = 4095 ✘.
`commands only` 1 + 4093 = 4094 ✔, 1 + 4094 = 4095 ✘.
`skills=2046 + commands=1` = 1+4092+1+1 = 4095 ✘ — and it does fail, which is how the
naive "cap = 4093 flat" model was killed.
`skills=1 + hooks=1 + mcp=1 + commands=4086` = 3+2+2+(1+4086) = 4094 ✔.

The `skills` weight of 2 is the reason `create-plugin`'s own family is asymmetric: a skill
carries both a capability declaration and a materialised agent-resource entry (the
`cache/local/<id>/<sha>/agent-definitions/<digest>.json` files).

Interpretation of the code name: this is `tbh_plugins::agent_definition_inventory::preflight`
failing closed. `plugin_preflight_overflow` is its snapshot-side reason (see §5).

---

## 4. Scope-level volume: there is no capability quota  (PROVEN negative)

Everything below composed cleanly with `capability_diagnostics: []`:

| configuration | capabilities admitted | result |
|---|---|---|
| 3 plugins × 2 000 skills | 6 000 | `eff=2000` on each, capdiag 0 |
| 16 plugins × 2 046 skills | **32 736** | `skills list --json` returns **32 751** rows (32 736 plugin + 15 bundled), capdiag 0 |
| 4 plugins × 4 093 commands | 16 372 | capdiag 0 |
| 16 plugins × 4 093 commands | 65 488 | capdiag 0 (deliberately straddling 65 536) |
| 17 plugins × 4 093 commands | 69 581 | capdiag 0 |
| 256 plugins × 500 skills | **128 000** | capdiag 0 |
| one local marketplace with 3 000 plugins | — | `plugin_count: 3000, skipped: [], warnings: []` |

So the loader will admit at least **128 000** plugin capabilities and at least **32 736**
plugin skills into one session's inventory without a single diagnostic. There is no
per-scope and no per-class *volume* quota reachable in this build.

---

## 5. `plugin_preflight_overflow` — reproduced, and it counts **plugins**, not capabilities

The one named code that fires. At 257 enabled plugins, every plugin's snapshot carries:

```json
"capability_diagnostics": [
  {"code": "agent_definition_review_unavailable",
   "message": "agent definitions are not reviewable and stay inactive: plugin_preflight_overflow",
   "path": null}
]
```

Bisection (`settle/quotas/pcount.py`, one 1-skill plugin per install):

```
plugins=17 32 64 128 256 -> (0, [])          # clean
  k=268 (1, [... plugin_preflight_overflow])
  k=262 (1, [...])
  k=259 (1, [...])
  k=257 (1, [...])
LAST_CLEAN=256 FIRST_OVERFLOW=257
```

Three controls (`settle/quotas/pcount2.py`) prove it is a **plugin count**, and specifically
a count of **enabled** plugins:

```
A 256 plugins × 500 skills  (128 000 capabilities)  -> capdiag=0     # volume is irrelevant
D 300 plugins installed, 50 of them `plugins disable`d (250 enabled) -> capdiag=0
B/C a plugin with all-empty capability arrays is rejected outright:
    invalid-plugin-package: "plugin compatibility is unsupported: no supported behavior capabilities"
```

### 5.1 What actually degrades (`settle/quotas/impact.py`)

A hook-bearing plugin was installed alongside 255 and then 256 filler plugins:

```
=== 256 enabled plugins ===
 inspect capdiag=[]
 approve   rc=0   -> {"decision":"approve", ... "enabled": true}
 hook test rc=0   -> {"decision":{"should_block":false,...}}
 skills total=271 plugin=256

=== 257 enabled plugins ===
 inspect capdiag=[{"code":"agent_definition_review_unavailable",
                   "message":"agent definitions are not reviewable and stay inactive: plugin_preflight_overflow"}]
 runtime_capabilities=[{... "stable_id":"plugin:im-hook:hook:pc", "status":"review_needed", "diagnostic":null}]
 approve   rc=0   -> approve succeeded, enabled true
 hook test rc=0   -> hook still runs
 skills total=272 plugin=257
```

**It is an advisory degradation, not a functional one, in 1.0.1.** Skills still load,
commands still load, hooks still approve and still execute, MCP servers still compose. What
stops is the *agent-definition review* lane — and plugin-declared `agents` is already an
unsupported capability family in this build, so nothing observable is lost today. Treat it as
a forward-compatibility tripwire: a bundle that pushes a user past 256 enabled plugins
silently disarms a subsystem Meta is clearly still building.

---

## 6. The skills catalog — the budget that actually decides bundle size

### 6.1 Anatomy (all bytes measured, not inferred)

Rendered block `skills_catalog`, `role: developer`, `order: 200`,
`cache_class: stable_prefix`. Its `context_block_diagnostic` reports `max_bytes: 0`, i.e. the
32 000 cap is enforced in the skills renderer, not the context-block lane.

```
header (fixed)                                   363 bytes
footer  </skill-catalog></system-reminder>        35 bytes
15 bundled entries, with descriptions          10 060 bytes   ← Meta's own built-ins
                                               -------------
room left for everything else                  21 542 bytes   (of 32 000)
```

Entry line costs, measured:

```
id-only    <skill id="ID" scope="plugin" path="PATH"/>\n                     = 38 + len(ID) + len(PATH)
full       same, plus <description>…</description>                           = id_only + len(desc) + 36
example    plugin id "ohmy", 12-char skill id, 120-byte description          = 104 id-only, 260 full
example    project skill `.agents/skills/<12-char id>/SKILL.md`, same desc   = 243 full
bundled    bundled:browser-app-delivery                                      = 1 912 (!)
```

### 6.2 Order, and who gets starved

```
order: [('bundled', 15), ('fs', 50), ('plugin', 50)]
```

**bundled → project/user filesystem → plugin.** Plugin skills are rendered last, so they are
the first to lose descriptions and the first to be dropped entirely. Measured at
200 project + 200 plugin skills: all 200 filesystem entries survive, only **121** of the 200
plugin entries are listed at all.

### 6.3 Degradation is three-stage

Sweep at 120-byte descriptions, plugin id `ohmy`, 12-char skill ids, no project skills:

```
n=  0  bytes=10458 entries= 15 desc= 15
n= 10  bytes=13058 entries= 25 desc= 25
n= 50  bytes=23458 entries= 65 desc= 65      <- stage 1: everything intact
n=100  bytes=31934 entries=115 desc= 86
n=150  bytes=31986 entries=165 desc= 53
n=200  bytes=31882 entries=215 desc= 19      <- stage 2: entries listed, descriptions dropped
n=250  bytes=31752 entries=265 desc=  5         in catalog order, tail first
n=270  bytes=31844 entries=285 desc=  1
n=288  bytes=31927 entries=303 desc=  0      <- last n where nothing is dropped
n=289  bytes=31927 entries=303 desc=  0  warn="muse: Skills: 303 loaded · 1 warning"
n=290  bytes=31927 entries=303 desc=  0  warn="muse: Skills: 303 loaded · 2 warnings"
                                             <- stage 3: entries silently vanish from the block
```

Stage 2 is **completely silent** — no warning, no diagnostic, nothing in `session.jsonl`, and
`muse skills list --json` still reports every skill with `diagnostics: []`. A skill that
reaches the model with no `<description>` is effectively dead: the catalog's own preamble
tells the model these are "summaries only", and with no summary there is nothing to route on.

Stage 3 does print, on stderr, from `muse exec`:

```
muse: Skills: 377 loaded · 5638 warnings · 5635 details hidden
muse:   19q: skill omitted to keep the startup catalog within its aggregate budget of 32000 bytes
muse:   Run `muse skills list --source all` for full diagnostics.
```

**This corrects 00-SYNTHESIS §0 fact 1 and the Appendix-A budget table.** The drop is not
"invisible with zero diagnostics anywhere": stage 3 is announced on stderr by name, and the
32 000 constant is printed verbatim by the binary. What *is* invisible is stage 2, which is
the more dangerous one, because the skill is still listed and still counted as "loaded".
(The follow-up the message suggests, `muse skills list --source all`, prints all 6 015 rows
and **no** diagnostics — the hint is wrong.)

### 6.4 The formula, validated out of sample

```
budget       = 32000 − 363 (header) − 35 (footer) − Σ bytes of higher-priority entries
N_max        = floor( budget / (id_only_bytes + description_bytes + 36) )
id_only      = 38 + len("plugin:<plugin-id>:<skill-id>") + len("plugin://<plugin-id>/<rel-path>")
```

`settle/quotas/formula.py` predicted, then measured, three fresh configurations:

```
pid=oh-my-musecode idlen=20 dlen=250  id_only=140 full=426 predicted 50   n=50 all-desc TRUE, n=51 FALSE
pid=ohmy           idlen=12 dlen=120  id_only=104 full=260 predicted 82   n=82 all-desc TRUE, n=83 FALSE
pid=omm            idlen= 8 dlen= 60  id_only= 94 full=190 predicted 113  n=113 all-desc TRUE, n=114 FALSE
```

### 6.5 One budget, not two  (PROVEN)

Plugin-provided skills and filesystem skills share a single budget with each other **and**
with the bundled built-ins:

```
50 plugin,   0 fs  -> 115 entries, 65 with descriptions
 0 plugin,  50 fs  -> 115 entries, 65 with descriptions
50 plugin,  50 fs  -> 115 entries, 91 with descriptions   (not 130; one pool)
40 plugin,  40 fs  ->  95 entries, 95 with descriptions   (30 578 bytes, just under)
100 plugin,100 fs  -> 215 entries, 30 with descriptions
200 plugin,200 fs  -> 336 entries,  0 with descriptions, 79 plugin skills dropped entirely
```

### 6.6 The built-ins are a 10 060-byte tax, and it is refundable

`muse skills disable bundled:<id> --scope built-in` removes an entry from the catalog
entirely. Disabling all 15 raises the room from 21 542 to 31 602 bytes:

```
baseline (15 bundled on), 1 plugin skill : bytes=10718  entries=16
all bundled off,          1 plugin skill : bytes=  658  entries= 1
all bundled off, n=121                   : entries=121 desc=121   all descriptions kept
all bundled off, n=122                   : entries=122 desc=121   first drop
```

At other description sizes (measured the same way):

```
dlen=200  n=92 entries=92 desc=92 bytes=31678 all-desc TRUE   |  n=93 desc=92 FALSE
dlen=400  n=58 entries=58 desc=58 bytes=31718 all-desc TRUE   |  n=59 desc=58 FALSE
```

**82 → 121 usable bundle skills, a +48 % gain, for one settings write.** The single skill
`bundled:browser-app-delivery` alone costs 1 912 bytes — 8.9 % of a bundle's entire budget,
and more than seven 250-byte bundle skills.

---

## 7. What could NOT be settled, and exactly what was tried

`plugin_scope_quota` and `plugin_class_overflow` were **never observed firing**. They exist in
two sibling reason enums extracted from the binary (the capability-snapshot one at
`strings4.txt` offset 5 305 371 and the skills-loader one at 7 570 036, the latter with four
extra variants: `structured_shape`, `identity_changed`, `io_error`, `context_mismatch`). No
numeric constant and no rendered message for either exists anywhere in the printable strings —
the exhaustive `quota` search returned only `Disc quota exceeded` (libc) and
`ModelFailedquota` (a provider error enum).

Attempts, in order, all negative:

1. **Per-package volume, every class** — bisected all five classes to their exact ceiling
   (§3.1). Every ceiling is either the 4 094-unit inventory budget or the 131 072-byte manifest
   cap, and both report `invalid-plugin-package`, never a snapshot reason code.
2. **Aggregate capability volume** — 6 000, then 16 372, 32 736, 65 488, 69 581, and finally
   **128 000** capabilities across up to 256 plugins. `capability_diagnostics` stayed `[]` in
   every case. Deliberately straddled 65 536 (16 vs 17 × 4 093 commands) on the theory of a
   16-bit bound: nothing.
3. **Plugin count** — this is what found `plugin_preflight_overflow` at 257. Pushed to 400
   plugins: still only that one code, never `scope_quota` or `class_overflow`.
4. **Marketplace scope** — a local `.claude-plugin/marketplace.json` with 300 and then 3 000
   plugins. `plugins marketplace add` returns `plugin_count: 3000, skipped: []`;
   `plugins list --available --json` returns `available=3000 skipped=0 warnings=[]`.
5. **Duplicate capability ids across two plugins** (`dupa` and `dupb` both declaring skill
   `shared`) — both load, both appear as `plugin:dupa:shared` and `plugin:dupb:shared`,
   `capability_diagnostics: []`, `skills list` `diagnostics: []`. The `duplicate` reason code
   does not surface either.
6. **A disabled plugin** — `plugins disable dupb` then `plugins inspect dupb --json` gives
   `eff=0 active=false capability_diagnostics=[]`. Even `disabled_plugin`, a reason that
   demonstrably applied, is not rendered.
7. **Log sweep** — after every one of the above, `grep -rl` over the whole
   `$XDG_DATA_HOME/muse/` tree for all 40+ reason names: zero hits, in session logs, in
   local-tracing, and in the plugin store.

**Conclusion (INFERRED, but well-supported):** in 1.0.1 the reason enum is internal
bookkeeping. Exactly one member is projected to a user-visible surface, and only through the
`agent_definition_review_unavailable` diagnostic that carries it as a message suffix. The
other two are either dead in this build or gated behind the unshipped plugin `agents`
capability family. Settling them needs either the `muse-canary` 1.1.0 build or a decompiler
on `tbh_plugins::capability_snapshot::diagnostic`, neither of which is available here.

Two smaller things also not settled: the numeric N in
`plugin \n directory contains more than \n filesystem entries` (not reached at 4 093 files in
one directory) and in `plugin \n directory nesting exceeds the maximum depth of \n`.

---

## 8. What this changes for oh-my-musecode

1. **CI must enforce a catalog-bytes budget, not a skill count.** Add
   `omm lint --catalog-budget`: compute
   `Σ (38 + len(display_id) + len(display_path) + len(description) + 36)` over the bundle's
   skills and fail the build above **21 542 bytes** (the room left with Muse's built-ins on
   and no project skills). At a 250-byte description budget that is ~50 skills; the
   default-safe headline number to publish is **"a bundle ships at most 50 skills"**.
2. **Cap descriptions in the authoring lint.** Description bytes are the dominant term.
   A 250-byte cap per skill, enforced at author time, is what makes the 50-skill number hold.
3. **`omm doctor` must report catalog pressure**, because stage-2 degradation is silent. Run
   `muse exec --provider echo hi`, read `skills_catalog` out of `session.jsonl`, and report
   `entries`, `entries with <description>`, and `bytes / 32000`. Any bundle skill without a
   description is a defect, not a warning.
4. **Ship a `muse skills disable bundled:*` recipe as a first-class install option.** It is
   the single highest-leverage lever in the whole system: +48 % catalog room, one settings
   write, fully reversible. `bundled:browser-app-delivery` (1 912 B),
   `bundled:read-session` (938 B), `bundled:git` (865 B), `bundled:greenfield-project-scaffolding`
   (830 B), `bundled:table-fit` (807 B) and `bundled:plan` (794 B) are worth ~6 KB between them.
5. **Project skills starve the bundle, never the reverse.** Ordering is bundled → filesystem →
   plugin. A user with 60 project skills can silently strip the descriptions off an entire
   bundle. `omm doctor` must account for project/user skill bytes before claiming a bundle fits.
6. **One bundle = one plugin, and split only on the 4 094-unit budget.** A single plugin holds
   2 046 skills or 4 093 commands — thousands of times more than the catalog can carry. There
   is no capability-count reason to split a bundle into many plugins, and a strong reason not
   to: the **256-enabled-plugin** ceiling is the only aggregate limit in the system. Design the
   installer so a full oh-my-musecode install costs the user a handful of plugin records, not
   one per topic.
7. **Reminders are the scarce class: 43 per plugin**, because the mandatory `decision`
   declaration is ~3 049 bytes of manifest and the manifest cap is 128 KiB. A bundle that wants
   both many reminders and many skills must watch `plugin.json` size directly; add it to CI.
8. **Hooks and MCP servers are effectively unbounded** (2 153 / 2 902 per plugin). Neither will
   ever be the constraint.

---

## 9. Reproduction — from a clean sandbox

```bash
S=/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad
Q="$S/settle/quotas"
export HOME="$Q/home" XDG_CONFIG_HOME="$Q/home/.config" XDG_DATA_HOME="$Q/home/.local/share"
export MUSE_NO_AUTO_UPDATE=1 MUSE_EXPERIMENTAL_PLUGINS=1
M="$S/muse-aarch64-macos"
mkdir -p "$Q"/{home,work,gen,out} "$XDG_CONFIG_HOME" "$XDG_DATA_HOME"
cd "$Q"

# 1. manifest byte cap  -> 131072 inclusive
python3 bytecap.py 131071 131072 131073

# 2. per-class ceilings  (each prints LAST_GOOD / FIRST_BAD)
python3 bisect.py skills      1024 2048      # 2046 / 2047   inventory budget
python3 bisect.py commands    2048 4096      # 4093 / 4094   inventory budget
python3 bisect.py hooks       2000 2400      # 2153 / 2154   manifest bytes
python3 bisect.py mcpServers  2500 3200      # 2902 / 2903   manifest bytes
python3 rem2.py   1    80                    #   43 /   44   manifest bytes

# 3. the unit-cost model
python3 genmix2.py commands 1000 4200                       # 4093, base cost 0
python3 genmix2.py commands 1000 4200 skills=1              # 4090  -> skill = 2 units + 1 class
python3 genmix2.py commands 1000 4200 hooks=1               # 4091  -> hook  = 1 unit  + 1 class
python3 genmix2.py commands 1000 4200 skills=1 hooks=1 mcpServers=1   # 4086

# 4. plugin_preflight_overflow  -> 256 clean / 257 overflow   (~2 min)
python3 pcount.py 256 280
python3 impact.py                # shows hooks still approve+run past the bound

# 5. no scope/class volume quota                             (~6 min for the 128k case)
python3 scope.py 16 2046 skills  # 32 736 plugin skills, capdiag 0
python3 mkt.py 3000              # 3 000-plugin marketplace, skipped 0

# 6. the catalog budget
python3 catlimit.py 80 120 200 300 400     # descriptions-kept ceiling per description size
python3 formula.py                          # out-of-sample formula validation
python3 bundledtax.py                       # 82 -> 121 by disabling the 15 built-ins

# 7. read any catalog by hand
"$M" exec --provider echo --trust-workspace hi
python3 dump.py "$XDG_DATA_HOME/muse"        # summary
python3 dump.py "$XDG_DATA_HOME/muse" raw    # the exact block text the model sees
```

Scripts, all under `$Q`:
`gen.py` (fat generator, Meta's canonical reminder decision block) ·
`gen2.py` (dense single-class generator) · `genmix2.py` (mixed-class, weight fitting) ·
`bisect.py` `bmix.py` `sweep.py` (bisections) · `bytecap.py` · `rem.py` `rem2.py` (reminders) ·
`probe.py` `probe2.py` (install + snapshot) · `scope.py` `scope2.py` `pcount.py` `pcount2.py`
`impact.py` (scope/plugin-count) · `mkt.py` (marketplace) · `catsweep.py` `catlimit.py`
`formula.py` `bundledtax.py` `dump.py` (catalog) · `diagshape.py` (diagnostic shape) ·
`env.sh` (the sandbox environment).
