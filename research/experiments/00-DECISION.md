# 00-DECISION — oh-my-musecode

**Subject:** Meta Muse Code `1.0.1-R2006.1` (stable) and `1.1.0-R2009.1` (canary).
**Inputs:** six settling experiments (four independently re-verified from clean sandboxes) plus a
45-axis canary diff, updating `re/00-SYNTHESIS.md` §6/§7 and `ohmy/00-MATRIX.md` §6.
**Date:** 2026-09-02.

Rule applied throughout: **a verifier's CORRECTED or REFUTED result overrides the experiment**, and
the reversal is named where it happened. STILL_UNKNOWN stays unknown — where a question is open, this
document says what the framework must do to be correct under *both* answers rather than picking one.

---

## 0. The decision, in one page

**GO. Start M0 and M1 now. Do not wait on anything still open.**

Three results decide it.

1. **A single plugin bundle can ship model tools.** Plugin-provided MCP servers start in *every*
   lane — `muse exec`, the TUI, and `muse serve` — and the earlier negative was a trust-state
   artifact, not a lane restriction. The install story stays "one bundle" and does not degrade to
   writing `settings.json → mcpServers`. CONFIRMED by an independent verifier who added the control
   the original lacked (a bundle shipping a skill *and* an MCP server, proving the plugin loaded
   while only the MCP capability was withheld).
2. **`MUSE_EXPERIMENTAL_PLUGINS=1` is an install-time gate only.** An installed bundle's skills,
   commands, hooks, MCP servers and agent definitions all compose at session time with the gate
   unset. Two independent experiments prove it. The user never exports anything; `omm` sets it on
   its own subprocesses. That collapses the friction gap between Tier 0 and Tier 1 to nearly zero.
3. **The extension surface did not move across a full minor version.** 45 measured axes, three
   release trains apart, two differences — both cosmetic (`--version` string, `userAgent` build
   hash). Manifest `schemaVersion` still 1, still exactly 5 capability families, 41 gates with zero
   default flips, MSP schema byte-identical, skills-catalog cap re-proven at exactly 32,000 by
   independent binary search on each binary.

The surface is stable enough to build on and capable enough to be worth building on. What the
results *change* is not whether to build but **what `omm` has to own**: nearly every failure mode
found across all six experiments is silent at session time. Approvals that were never granted, a
capability line that was deleted rather than set to a bad value, a package byte that changed, a
missing `provider` key, a settings collision that makes every settings-writing command exit 1, a
catalog that dropped half its descriptions — none of these produce an error, a warning, or a log
record when a session runs. **`omm doctor` is not a nice-to-have milestone; it is the product's
reason to exist.** M0 was already first in the plan. It is now first by a wider margin.

Two things are blocked, permanently, and should be struck from the roadmap rather than deferred:
**an enterprise/team tier** (the policy plane accepts only documents that express nothing, and the
delivery directory cannot be written by anything Meta ships), and **"MSP for tool registration"**
(MSP has no tool-invocation and no tool-registration method, assembles no run context, and runs no
hooks). One thing is de-risked but must stay opt-in: **skills.v1 routing**, which works and is real
but rides two experimental env gates and requires a per-workspace vendored library.

---

## 1. What is settled

### 1.1 CONFIRMED — experiment plus an adversarial verifier

**Plugin MCP / the bundle's capability to ship tools**

| # | Settled fact |
|---|---|
| 1 | Plugin-provided stdio MCP servers spawn under `muse exec`, the TUI, and `muse serve`. The tool reaches the model's `active_tools` as `mcp__plugin_<pid>_<sid>__<tool>`. |
| 2 | Spawn is gated on the runtime capability being **`trusted_enabled`**. `review_needed` (the default after install), `trusted_disabled`, and `modified` are skipped **silently** — no stderr, no session record, no trace line — while `plugin_capability_snapshot.compose` still reports `mcp_servers=N`, because that field counts *declared* capabilities. |
| 3 | **Four** independent kill switches, not one: (a) capability trust state; (b) `muse plugins disable <id>`, which *removes the runtime-capability line from `plugins inspect` entirely* — a doctor that greps for a bad status sees nothing at all; (c) any byte change to the package followed by `plugins update` (`package_sha256` moves, therefore the cache path, therefore the resolved command, therefore `trusted_definition_hash`); (d) in the TUI only, the workspace-trust dialog, which blocks session open outright. |
| 4 | `muse plugins remove <id> [--delete-data]` **exists** and is the correct uninstall primitive: it deletes the cache and strips `runtime_capabilities` from `settings.json`. |
| 5 | Model-visible namespace: `len(plugin-id) + len(server-id) <= 18`, exactly. ≤31 chars survives verbatim; 32 is the first length rewritten to `first-17 + "__" + 12 hex`. |
| 6 | A plugin MCP server gets **no `env`, `cwd` or `headers`** (native manifests silently drop them). Child env is exactly 16 keys: 9 base (`HOME LANG LOGNAME PATH SHELL TERM TMPDIR USER __CF_USER_TEXT_ENCODING`) + 7 plugin vars. `PATH` is inherited verbatim. `cwd` is the **workspace**, not the plugin. |
| 7 | `transport: "http"` plugin MCP servers work (loopback, streamable HTTP, same handshake), and stdio + http coexist in one session. |
| 8 | `muse serve` (MSP) needs `settings.json → provider`. Without it the agent runtime is never constructed — `active_tools: ["write_todos"]`, no capability composition, turn fails "not logged in" before any network call. With `"provider": "echo"` the whole MSP lane is offline-testable. |
| 9 | MSP has **no** tool-invocation method and **no** `session/close`. The served surface is `session/*`, `turn/*`, `model/list`, `view/*`, `approval/*`, `userInput/*`. An MSP session also assembles **no run context at all** and runs **no hooks**. |

**skills.v1 routing**

| # | Settled fact |
|---|---|
| 10 | Routing works. "Negotiation" is a one-way per-invocation advertisement: with `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1` **and** the handler declaring `"outputCapabilities":["skills.v1"]`, the host adds `supported_output_capabilities:["skills.v1"]` to the hook's stdin. Omitting the field is exactly the old `capability-not-negotiated`. `..._APPLY=1` is a second gate that makes the host consume the result. |
| 11 | **`read_skill` resolves a routed, non-discovered skill** — by id *and* by absolute path, with matched negative controls (no router → `unknown-skill`; gates off → `unknown-skill`). This retires the "programmable hint vs programmable router" risk that the experiment flagged as blocking. |
| 12 | The budget is **combined**: `order200_bytes + order201_bytes <= 31984`. Three outcomes: full block; every description silently dropped; or `selected_skills:rejected:combined-budget` with no block at all — a rejection code the original experiment never saw. Bisected to the byte and then *predicted* at a second catalog size. **This refutes the experiment's "additive, order 200 untouched" claim as far as budget is concerned.** |
| 13 | Routable skills must be **real regular files inside the workspace**, not under a discovery root, not symlinked. `read_skill` refuses any symlinked path component or file (`skill-body-path-unsafe`). A user-global library cannot be routed (`path-outside-project`). |
| 14 | Caps: 32 selected skills per turn globally across all handlers; 1024 B per description; 16384 B hook stdout; ids `[a-z0-9._-]+`; one bad entry discards the entire selection. |
| 15 | A project-tier `<ws>/.muse/hooks.json` router in an **untrusted** workspace never loads — no hook run, no terminal record, no error. The safe "failed terminal" degradation exists only in the user and plugin tiers. |
| 16 | Order 201 is `cache_class=runtime_prefix`: it invalidates the prompt-cache prefix from 201 onward every turn, whether or not the selection changed. |

**settings.json and the manifest contract**

| # | Settled fact |
|---|---|
| 17 | `settings.plugins` is a real deserialized member typed `Option<Box<RawValue>>`, but it reaches exactly two first-party ids (`skill-reminder`, `goal-reminder`) and **cannot enable or disable anything on its own** — canonical-only is a no-op; it can only invalidate or conflict. Nothing in the product ever writes it. **It is not the desired-state config the plan hoped for.** |
| 18 | `reminder enablement settings are invalid` fires for a malformed value in *either* store — `runtime_capabilities: {"a":1}`, a single bad entry under an unrelated key, takes both wired reminders down. Doctor must lint both. |
| 19 | `muse plugins approve plugin:tbh-reminders:reminder:skill-reminder` works from the CLI and, with a disagreeing `settings.plugins` entry present, **silently manufactures the conflict** (writes `enabled:true`, prints `"decision":"approve"`, no warning). |
| 20 | `mcpServers` **and** `mcp_servers` both present — even both `{}` — makes every settings-**mutating** command exit 1 with `MCP configuration error in "<CFG>/settings.json"; MCP is disabled for this runtime`, while read-only lanes stay silent. This upgrades the known collision from "degrade-only" to "the settings writer cannot write, and the message names MCP rather than the key it was setting." |
| 21 | Nine reserved plugin ids — `skill-reminder, goal-reminder, memory-reminder, todo-reminder, verify-reminder, scope-reminder, tbh-reminders, loop, muse-core` — install "successfully", report valid/active, and contribute **nothing** (`bundled_plugin_id_reserved`). |
| 22 | **`compat.manifestDir` is NOT restricted to `.muse-plugin`.** The same manifest bytes validate clean (`valid:true`, `diagnostics:[]`, `summary:"full"`) in `.muse-plugin`, `.codex-plugin` and `.claude-plugin` — with *different runtime semantics*: in `claude-compatible`, `enabledDefault` is ignored entirely. **This refutes a load-bearing sentence of the plugin-contract experiment's own answer.** A packer writing into the wrong dot-directory ships a clean, "full" plugin that behaves differently. |
| 23 | `compatibility.summary == "full"` is not a safe blanket gate: a plugin with no supported capability reports `summary:"unsupported"` with `valid:true, diagnostics:[]`. The correct predicate is `valid == true AND diagnostics == [] AND manifest_family == "native" AND every declaration "supported"`. |
| 24 | `muse plugins validate` performs **zero** inspection of `SKILL.md` — not frontmatter, not UTF-8, not the filename. Only `muse skills validate <DIRECTORY>` catches those, and even it accepts a BOM and a frontmatter `name` that differs from the capability id. `muse skills list` is a third checkpoint and the only post-install one. |
| 25 | Any symlink anywhere in the package — escaping or not, referenced or not — kills it with the opaque `Agent Definition inventory derivation failed closed`. Negative control run: a stray *regular* unreferenced file is valid, so the symlink is genuinely the cause. Same string covers three distinct hygiene failures (symlink, >4096 entries, and now depth). |
| 26 | Exact package boundaries, double-sourced: manifest **131072** B inclusive; **4096** filesystem entries; **16** path segments. |
| 27 | Per-family field strictness: `hooks` and `reminders` are CLOSED (unknown key = hard error); `skills`, `commands`, `mcpServers` are OPEN (unknown key silently accepted). An `mcpServers` entry carrying `env`/`cwd`/`headers` validates perfectly and then launches without them. |

**Canary / drift**

| # | Settled fact |
|---|---|
| 28 | `muse-canary` `1.1.0-R2009.1` vs stable: 45 axes, 2 cosmetic differences. Every extension-surface constant identical, including the 32,000 catalog cap re-proven independently on each binary. |
| 29 | The real drift risk is **server-side**: both builds carry an identical `feature_config` provider whose embedded override list includes `extensions.skills.allowed_digests`, `extensions.skills.allowed_kinds`, `extensions.hooks`, `extensions.runtime_capabilities`. Meta can allowlist which skills and extension kinds are permitted **without shipping a binary**. Pinning a checksum will not catch it. Mechanism proven; behaviour not exercised (needs auth). |
| 30 | A naive `strings` diff of two Muse builds is ~100% false positives (Rust rodata blob boundaries shift on any code change). Use behavioural oracles. Noted so no future CI job is built on the wrong instrument. |

### 1.2 SINGLE_SOURCE — measured once, no verifier ran

These are strong (exact boundaries, fitted models, out-of-sample predictions that held) but were not
adversarially re-tested. **Lock every one of them into the CI fixture set** described in §5; that is
how they get their second source.

| # | Fact |
|---|---|
| 31 | Per-package **inventory budget = 4094 units**, cost = 1 per non-empty capability class + 2 per skill + 1 per command/hook/mcpServer/reminder. Fitted from 11 mixed bisections, exact on every cross-check. |
| 32 | Per-class ceilings in one plugin: skills **2046**, commands **4093**, hooks **2153**, mcpServers **2902**, reminders **43**. |
| 33 | `plugin_preflight_overflow` counts **enabled plugins**, not capabilities: 256 clean, 257 fires. Soft advisory — skills/commands/hooks/MCP all still work; only the agent-definition review lane is disarmed. Disabling plugins refunds the count. |
| 34 | `plugin_scope_quota` and `plugin_class_overflow` are **not reachable by volume** in this build: 256 plugins × 500 skills = 128,000 capabilities, `capability_diagnostics: []`. Treat as nonexistent. |
| 35 | Skills-catalog anatomy: 363 B header + 35 B footer + **10,060 B of Meta's 15 built-ins** → **21,542 B left for a bundle**. Entry cost `38 + len(display_id) + len(display_path) [+ len(desc) + 36]`. |
| 36 | Catalog order is **bundled → project/user filesystem → plugin**. Plugin skills are rendered last and starved first. Project skills starve the bundle; never the reverse. |
| 37 | Degradation is **three-stage**: (1) intact; (2) entries listed, **descriptions dropped tail-first, completely silently** — `skills list --json` still reports every skill with `diagnostics:[]`; (3) entries vanish, and *this* stage prints on stderr naming the 32000 constant. **Corrects `00-SYNTHESIS` §0 fact 1 and the Appendix-A row: the drop is not uniformly silent — stage 3 is announced, stage 2 is not, and stage 2 is the dangerous one.** |
| 38 | `muse skills disable bundled:<id> --scope built-in` refunds the built-in tax: 21,542 → 31,602 B, **+48% usable bundle skills for one settings write**. `bundled:browser-app-delivery` alone costs 1,912 B. |
| 39 | Exit codes: **there is no unknown top-level command** — an unrecognised first token is the `[PROMPT]` positional and starts a session. `2` = argv rejected (a parse error and a missing gate are indistinguishable); `1` = parsed but the run failed; `0`/`1` for an unknown token, decided by *workspace trust*, not by the token. |
| 40 | Malformed `settings.json` is a **lazy load, not a startup gate**: `--version`, `--help`, `export`, `init`, `workflows list` all succeed. Only settings-consuming commands fail. Doctor cannot use `--version` to detect it; use `muse skills list`. |
| 41 | `personal_project` memory root = `$XDG_DATA_HOME/muse/memory/projects/<slug96>-<fnv1a64hex>/`; slug = canonical path minus leading `/`, every non-alnum → `-`, truncated to 96; hash = FNV-1a-64 of the full path as `%016x`. Confirmed by predicting a never-used path before running. The block is **trust-gated**. |
| 42 | Enterprise: the `system_file` root is `/Library/Application Support` (proven by a non-destructive seatbelt oracle); the leaf is **not recoverable on this machine** and the method is structurally blind to it. The **policy plane is entirely inert** — every field carrying content returns `field_not_activated`. The **defaults plane is largely live**. The shipped `muse.pkg` installs exactly one file and runs no scripts. |
| 43 | `agent_definitions.safe_mode` suppresses `user`, `project` **and** `plugin`; survivors are `managed` + `built_in`, neither reachable by a framework. Proven by exhaustive elimination including a purpose-built plugin agent-definition capability. |
| 44 | Windows is a **different tool surface**, not a port: the bash tool is replaced by a PowerShell tool, and a handler with only `commandWindows` is dropped on POSIX with a diagnostic while a POSIX-only `command` **is dispatched into PowerShell with no static warning**. A `.claude-plugin` bundle **cannot carry `commandWindows`** (`foreign hook handler field 'commandWindows' is unsupported`). |
| 45 | A redirected `HOME` does **not** sandbox muse: it composes `~/Library/Application Support/Muse/session-name-authority/` from the real user home via `getpwuid` and writes there on essentially every run. |

### 1.3 The seven reversals — where a verifier overrode an experiment

Recorded so nobody rebuilds on the superseded claim.

1. **`compat.manifestDir` is not pinned to `.muse-plugin`** (verifier C1). The experiment's answer
   said "must be literally `.muse-plugin`". It is a *consistency* rule, not a value rule, and the
   three families differ semantically. → `omm` must assert `manifest_family == "native"`.
2. **`muse plugins remove` exists** (verifier V.7). The experiment concluded you must hand-edit
   `settings.json` to reset a plugin. → this is `omm uninstall`'s primitive.
3. **The "~21.25 KB rendered-block cliff" is a shared 31,984-byte budget** with a third rejection
   outcome (verifier CORRECTION 1). → the CI gate is `A + B <= 31984`, not a per-block number.
4. **The symlink "containment hole" is not exploitable** (verifier CORRECTION 2) — `read_skill`
   refuses links. → downgrade the security wart, **and** the design guidance that a symlink could
   pull in an out-of-workspace library is dead: the routed library must be **copied**.
5. **`read_skill` resolves routed paths** (verifier, new result). → the experiment's own
   "do not build past step 3 without this answer" blocker is retired.
6. **Failure is not silent at *install* time** (verifier V.6) — install and every later
   `plugins inspect` print the MCP-review warning. It is silent at *session* time. → `omm install`
   can quote the binary's own warning, but nothing may depend on a session-time signal.
7. **The TUI evidence was misread** (verifier V.5) — the "swallowed keystroke" was the workspace-trust
   dialog. The server starts at session open within ~1 s with no input on an already-trusted
   workspace. `exec` and `serve` have no such gate.

Two smaller ones worth carrying: `hook-configured-order` for user vs project is 0 and 1
(positional), not both 0; and `mcpServers`/`mcp_servers` are **not** members of the struct that
raises `duplicate field` — they are read by a separate pass, so the "30 fields over 29" count in the
old config table is unsupported.

---

## 2. What changes in the plan

### 2.1 Install tiers — Tier 1 becomes the default path

`00-SYNTHESIS` §6.3 made Tier 0 (skills only, no gate) the README path "because it is the only one
that cannot be turned off by a flag Meta owns", and Tier 1 (the plugin bundle) the richer opt-in.
**Invert it.**

| | Old | New |
|---|---|---|
| **Default** | Tier 0: `muse skills install` × N + settings merge | **Tier 1: one bundle.** `omm install` sets `MUSE_EXPERIMENTAL_PLUGINS=1` **on its own subprocess only**, runs `plugins install` → `plugins approve` per capability → verifies with `plugins inspect`. Ships skills + commands + hooks + MCP tools in one digest-pinned, approvable artifact. |
| **Fallback** | — | **Tier 0 becomes `omm install --no-plugin`**, kept working and CI-tested, so a gate removal is a config change and not a rewrite. |
| **Tier 2** | repo scaffold | unchanged, **plus** it now owns the routed-skill library (§2.4, M4). |

Why: the gate is install-time only, proven twice; the runtime is ungated; a bundle can ship model
tools; and the canary shows 41 gates with zero default flips across a full minor version. The user
never exports anything. The one honest cost is that `omm doctor`, `omm update` and `omm uninstall`
must all set the gate themselves — trivial, and it is exactly the "negotiate capabilities against
the actual binary" requirement (MATRIX M16) applied at every entry point.

### 2.2 Command surface — the concrete deltas

| Command | Change |
|---|---|
| `omm install` | Must run `muse plugins approve plugin:<id>:<kind>:<cap>` for every declared capability and then **verify with `muse plugins inspect` that the runtime-capability line is PRESENT and literally `trusted_enabled`**. Exit 0 from approve is not sufficient, and `plugins disable` deletes the line rather than marking it — so a doctor that only greps for a bad status sees nothing. Approval is fully non-interactive; no TTY needed. |
| `omm update` | **Atomic `plugins update` + re-approve, always.** Any byte change to the bundle flips every capability to `modified` and silently kills the MCP server. There is no such thing as a safe partial upgrade. |
| `omm uninstall` | Use `muse plugins remove <id> [--delete-data]`. **Never restore `settings.json.pre-omm` wholesale** — approvals live in `settings.runtime_capabilities` inside that same file, and a user-authored `settings.plugins` must survive byte-for-byte. Restore by targeted merge. Name `~/Library/Application Support/Muse/session-name-authority/` as deliberately-kept residue (it is outside our ownership, grows on every run, and nothing else cleans it up). |
| `omm doctor` | **Eleven checks**, all of them detecting something with no session-time symptom: (1) every declared `mcp_server` capability line present and `trusted_enabled`, with the exact fix command; (2) `settings.json → provider` set — without it any `muse serve` / SDK host gets a one-tool session and no MCP; (3) `mcpServers` + `mcp_servers` collision, now reported as "every settings-writing muse command will exit 1"; (4) lint `settings.plugins` **and** `settings.runtime_capabilities` for malformed shapes — a bad entry under an unrelated key takes both wired reminders down, TUI-only; (5) canonical/legacy reminder conflict; (6) `agent_definitions.safe_mode` → "the agent pack will not load"; (7) enabled-plugin count, warn at 200 of 256; (8) catalog pressure: entries / entries-with-description / bytes-of-32000, from `muse exec --provider echo hi` + `session.jsonl`; (9) `muse config status` as the enterprise probe (4 rows on macOS); (10) `manifest_family == "native"`; (11) binary-version drift against the golden-constant block. |
| `omm cost` | Now has the exact per-entry formula and the refundable built-in tax; it can name `bundled:browser-app-delivery` as 8.9% of the bundle's budget. |
| `omm memory` | **New, and now buildable** — the `personal_project` root and its slug+FNV-1a-64 algorithm are known. Seed, back up, list, GC, and warn about the slug collisions the hash is covering. Note the block is trust-gated, so `omm trust` is a prerequisite. |
| `omm enable skill-routing` | **New.** Prints/exports the two gates, vendors the routed library into the workspace, installs the router hook. Opt-in, never default. |
| `omm run` | The shim carries the routing gates when enabled, alongside `MUSE_NO_AUTO_UPDATE=1`. |
| `omm doctor --self-test` | Carries the exit-code matrix, and `omm` must **validate argv itself against a hard allowlist of the 16 top-level commands** — an unknown token is a prompt, so a wrapper typo silently starts a session and bills a turn. |

### 2.3 The ten design rules — three replaced, five added

**Replaced**

- **Rule 3 (budget).** "Refuse to ship a bundle whose catalog exceeds ~24,000 bytes (75% of the hard
  cap)" is **wrong** — 24,000 B exceeds the 21,542 B that actually exists once Meta's 15 built-ins
  are counted. Replace with: *catalog payload ≤ 21,542 B with built-ins on, ≤ 31,602 B with them
  disabled; and if routing is enabled, `order200 + order201 ≤ 31,984 B`.* The rule's stated mechanism
  (`run.context_slimming.skill_catalog_descriptions: "first_sentence"`) is **still untested** — see
  §3.
- **Rule 4 (prefix everything).** Extend the reserved-id list from four to **nine** ids, and add the
  hard constraint `len(plugin-id) + len(server-id) <= 18`. Together with the catalog entry cost
  (the plugin id appears **twice** per entry, in the id and the path) this settles the naming:
  **the plugin id is `omm`, not `oh-my-musecode`.** Shortening it buys 22 B per catalog entry —
  about three extra skills at 50 entries — and it is *required* anyway for a verbatim MCP namespace.
- **Rule 9 (hooks).** Add: every hook ships **both** `command` (POSIX) and `commandWindows`
  (PowerShell), in a **native** Muse hooks document — a `.claude-plugin` bundle cannot carry the
  Windows form, and a POSIX-only handler is dispatched into PowerShell with no static warning.

**Added**

- **Rule 11 — validate on four predicates, never one.** `rc == 0 AND valid == true AND
  diagnostics == [] AND manifest_family == "native" AND every compatibility declaration
  "supported"`. `valid:true` hides dropped capability families; `summary == "full"` is not a safe
  blanket gate either (a plugin with no supported capability reports `"unsupported"` with
  `valid:true, diagnostics:[]`).
- **Rule 12 — the packer never emits a symlink,** anywhere in the tree, referenced or not.
  Dereference on copy; scan first so `omm doctor` can print the real reason instead of the binary's
  opaque one. Same for any filename containing a backslash.
- **Rule 13 — three checkpoints, in order.** `muse skills validate <dirname(path)>` per skill →
  `muse plugins validate <pkg>` → post-install `muse skills list --json`. The plugin validator never
  opens `SKILL.md`; the skills validator accepts a BOM and a `name`≠id mismatch; only the generator
  can catch those two.
- **Rule 14 — the trust lifecycle is `omm`'s, not the user's.** Install approves and verifies;
  update re-approves atomically; doctor asserts presence *and* state. Never depend on a session-time
  signal, because there is none.
- **Rule 15 — never gate on the version number.** A full minor bump, three release trains apart,
  changed nothing on the extension surface; a future patch bump therefore guarantees nothing either.
  Muse's semver tracks the product, not the contract. Probe behaviour (MATRIX M16), and run the P0
  fixture set on every channel poll rather than every release.

### 2.4 Milestones — M3 splits, M4 is new

| | Status | Change |
|---|---|---|
| **M0 — `omm doctor` + `omm cost`** | **GO now** | Fully instrumented and strictly larger than planned: 11 checks, all detecting silent failures. Instruments: `session.jsonl` (the only MCP-startup oracle — the structured trace carries **no** `mcp.*` records in any lane), `plugins inspect --json`, `muse config status`, and `muse exec --provider echo hi` for catalog pressure. |
| **M1 — Tier 0/1 install + lint + uninstall + 8–12 skills** | **GO now** | Ship `omm uninstall` first, on `plugins remove`. The default-tier skill ceiling is ~50–55 at 250-byte descriptions with built-ins on, ~74–81 with them disabled. `omm lint` implements Rules 11–13. |
| **M2 — `omm.plugins.json` + lock + reconcile + snapshot** | **GO**, scope grew | The ledger must now also record **runtime-capability approval state** per capability and the per-file `sha256` as a *trust* input, not just an update input — because any byte change silently revokes trust. |
| **M3a — marketplace + `.muse-plugin` bundle + hooks + MCP servers** | **Unblocked; pull forward into M1/M2** | This was gated behind "does plugin MCP work". It does. The gate is install-time only. There is no longer a reason to hold the bundle to last. |
| **M3b — reminder agents + themes + keymaps** | **GO**, deprioritised | Reminders remain the scarce class (43 per plugin, ~3 KB of mandatory `decision` each) and cost context every turn. Ship at most one enabled by default. A third-party reminder using the reserved `<\system-reminder>` tag **cannot be activated headlessly at all** — only the TUI's elevated-approval path clears it. `omm` must not generate one. |
| **M4 — skill routing (NEW, advanced tier)** | **Buildable, opt-in only** | Behind `omm enable skill-routing`. Requires both gates; ship the router as a **plugin** hook capability with `outputCapabilities:["skills.v1"]` (or user-tier `settings.json → hooks`) — **not** project tier, which fails silently in an untrusted workspace. The routed library must be **copied** into each workspace under a non-discovered directory (`<ws>/.omm/skills/`), disjoint from `.agents/skills/` by construction (`base-id-collision`). The router self-validates every entry before emitting; one bad entry discards the whole selection. |

The architectural consequence of M4 is worth stating plainly: **routing lifts the skill ceiling only
for the advanced tier, and only per-workspace.** Because a routed library cannot be user-global and
cannot be a plugin `skills` capability, `omm update` must walk the registered workspaces and refresh
their copies — which makes the M1 ledger (MATRIX M1) the mechanism that makes M4 possible at all.
Build the ledger correctly and M4 is cheap later; build M4 on top of ad-hoc copies and it is
unmaintainable.

### 2.5 `ohmy/00-MATRIX.md` §6 requirement changes

- **M17 — partially REFUTED.** "MSP over `muse serve` for event subscription and tool registration"
  is wrong on both halves: MSP has no tool-registration and no tool-invocation method, assembles no
  run context, and runs no hooks. **Tool registration is plugin `mcpServers`.** MSP stays a
  host-embedding protocol only. The rest of M17 (subprocess hooks, JSON stdin/stdout, SHA-pinned
  third-party content) stands and is strengthened — the host already binds trust to the package
  digest.
- **M1 (ledger)** — add the runtime-capability approval state and treat the per-file digest as a
  trust input. **M5 (uninstall)** — the primitive is `plugins remove --delete-data`; name the
  `session-name-authority` residue. **M12 (pinned transport)** — unchanged and vindicated: the
  package digest *is* the trust key, so a mutable-branch tarball would silently revoke every
  capability on every fetch.
- **M15 (hooks = one static binary)** — add the `commandWindows` twin and note that hooks may carry
  `outputCapabilities`, which is a first-class native field silently omitted from the normalized
  manifest that `plugins validate` echoes back (which is why nobody found it).
- **M16 (capability negotiation)** — promote from SHOULD-shaped to load-bearing, and add the P0
  fixture set from the canary report. **S2 (`omm doctor` first-class)** — promote to MUST in
  practice; it is M0.
- **S7 (registration by pointer)** — refine: `plugins install` writes `$DATA/plugins/installed.json`,
  approvals write `settings.runtime_capabilities`. Both are muse-owned. `omm` writes **nothing of its
  own** into `settings.json` — which now also means the pre-omm restore must merge, not overwrite.
- **§7.4 (the enterprise policy layer nobody has)** — **strike it.** The policy plane accepts only
  documents that express nothing; the defaults plane is live but undeliverable. This was listed as
  "the largest genuinely unclaimed gap in the space"; it is unclaimed because it does not exist.
  The auditability half of §7.4 (M11 provenance + M1 ledger + `omm doctor --json`) survives intact
  and is now *more* valuable, because it is the only way to see any of the silent failures.
- **§7.1 (tri-manifest projection)** — survives, with a sharp new caveat: the three dot-directories
  all validate clean but do **not** behave identically (`enabledDefault` ignored under
  `claude-compatible`), and a Claude-format bundle cannot carry a Windows hook command. The
  "author once, ship to three agents" thesis holds for content and breaks for hooks.

---

## 3. What is still open — and how to be correct under both answers

| # | Open question | Stay-correct rule | Cost to settle |
|---|---|---|---|
| 1 | **End-to-end MCP `tools/call` from the model.** Proven: server spawned, handshake completed, `tools/list` answered, catalogued, canonical id present in `model_request_configured.toolset.active_tools`. Unproven: the model actually invoking it. | Assume it works (everything short of the model's own emission is proven) but do not ship a bundle whose *only* value is an MCP tool until it is demonstrated. Skills and commands carry M1 on their own. | **Cheap, and now unblocked.** The plugin-MCP verifier believed this needed `muse auth` and declined. The skill-routing verifier and the loose-ends experiment both **independently** drove real tool calls through a local mock provider at `127.0.0.1` with a dummy key and a containment probe proving `--base-url` is total — no `muse login`, no `muse auth`, no Meta contact. **That harness already exists** at `settle/verify-skill-routing/`. Point it at the plugin MCP bundle. |
| 2 | **`run.context_slimming.skill_catalog_descriptions` / `full_skill_description_ids`.** Found in the settings string table; never executed. Synthesis design rule 3 depends on it, and it is the mechanism that would buy the most order-200 headroom. | Budget as if it does not exist: ≤21,542 B with built-ins on. Ship the `muse skills disable bundled:*` lever, which **is** proven, as the headroom story. | One sandbox run: set the key, run `muse exec --provider echo hi`, diff `skills_catalog` bytes. |
| 3 | **`plugin_scope_quota` / `plugin_class_overflow`.** Never observed firing; 128,000 capabilities admitted with `capability_diagnostics: []`. | Treat as nonexistent. The binding constraint is the 32,000-byte catalog, which is measured. Keep the code enum in `docs/host-reality.md` so a future sighting is recognised. | Not worth more effort. |
| 4 | **Server-side `feature_config`** — `extensions.skills.allowed_digests`, `allowed_kinds`, `extensions.hooks`, `extensions.runtime_capabilities`. Mechanism proven; behaviour never exercised (`gate_count=0` without auth). | **This is the single largest unquantified risk to the whole product** and no offline check can see it. Design so a rejected extension is *detectable*: `omm doctor` must compare what it installed against what actually composed in a live session, not against what it wrote to disk. | Needs one authenticated machine. Dump `gate_count` and any `extensions.*` overrides from the bootstrap trace. |
| 5 | **Enterprise `system_file` leaf directory.** Root proven (`/Library/Application Support`); leaf not recoverable — seatbelt path filters do not match nonexistent paths, so the bisection is structurally blind and its eight negatives mean nothing. | **Do not ship an enterprise tier.** Keep `muse config status` in doctor as the probe. If the path is ever published, only *defaults* could ship — never one restriction. | Needs a machine where an admin can write under `/Library`, or a sanctioned managed-preferences fixture. |
| 6 | **Agent Plugins 1.0.0 `$schema` literal** (synthesis unknown #4). Untouched by these experiments; no `$schema`-shaped literal in the binary. | Generate the three manifests we can verify. Do not speculate a fourth. | Watch Codex / Claude Code release notes; feed a real package to `plugins validate` if one appears. |
| 7 | **`PostToolUse` skill routing**, per-turn TUI replacement of the 201 block, plugin re-approval after a package edit, `--preset miniswe`. | Not load-bearing for M0–M2. | Cheap now that the mock-provider harness exists — fold into the M4 spike. |
| 8 | **The 16-byte discrepancy**: the catalog alone is capped at 32,000 (binary-searched on both builds), the combined 200+201 budget at 31,984. | Budget to **31,984** and you are correct under both readings. | Not worth settling. |
| 9 | Everything in §1.2 is **single-sourced**. | Convert to CI fixtures (§5). A fixture that runs on every channel poll *is* the second source. | Bundled into M0. |

---

## 4. Go / no-go

**GO — start M0 and M1 now.**

The extension surface is stable (45 axes unchanged across a full minor version and three release
trains), capable (one bundle ships skills, commands, hooks and working model tools, all headlessly,
with the experimental gate needed only at install time), and every remaining unknown either changes
a number rather than a shape, or belongs to something we have decided not to build.

**Blocked, permanently — strike from the roadmap:**

- **The enterprise / team tier.** The policy plane is inert and the file cannot be delivered.
  Unblocked only by Meta activating policy fields *and* publishing the directory. (MATRIX §7.4.)
- **"MSP for tool registration / event subscription with hooks."** No such method exists; MSP
  sessions assemble no context and run no hooks. Unblocked only by a new MSP method appearing in
  `schema generate-json-schema`. (MATRIX M17.)

**Blocked, conditionally:**

- **A bundle whose primary value is an MCP tool** — until the mock-provider `tools/call` run
  (§3.1). Unblocked by one afternoon with an existing harness. Everything short of the model's own
  emission is already proven, so this is a confidence gate, not a design gate.
- **M4 (skill routing) as anything other than opt-in** — two experimental gates plus a per-workspace
  vendored library. Unblocked only if Meta promotes the gates to default-on, which the canary gives
  no evidence for. Build the M1 ledger such that M4 is cheap later; do not make anything depend on it.

**Not blocked and often assumed to be:** the plugin bundle itself, the hook surface, the manifest
generator, cost accounting, the uninstaller, memory tooling, and the Windows path — all now have
either an exact contract or a working instrument.

---

## 5. Next actions, in order

1. **Convert §1.2 into a CI fixture set.** The canary report's P0 block runs in under five seconds:
   MSP stable fingerprint; plugin `schemaVersion` still 1; still exactly 5 capability families;
   assert on `compatibility.summary` + empty `diagnostics` (never on `valid`);
   `settings.schema_version` still 1. Add P1: 41 gates and their defaults, 17 hook events, the
   32,000 catalog cap by binary search, the 8 context-block orders and sizes. Run on **every channel
   poll**, not every release. Golden constants are in `settle/canary-diff.md` §5.
2. **Build `omm doctor` (M0)** against the 11 checks in §2.2. Every check exists because something
   fails silently at session time; each must ship its exact fix command.
3. **Run the plugin-MCP `tools/call` proof** using `settle/verify-skill-routing/`'s mock provider
   against the `settle/verify-plugin-mcp/` bundle. Closes the last real gap in the tools story.
4. **Fix the naming now, before anything is published.** Plugin id `omm`; capability ids short
   enough that `len(pid) + len(sid) <= 18`; every skill/command/capability id `omm-` prefixed and
   linted against the nine reserved ids, the 15 bundled skills and the 39 built-in slash commands.
   This is a schema decision and it cannot be changed later.
5. **Write the manifest generator against Rules 11–13** — four-predicate validation, no symlinks,
   three checkpoints, per-family strictness (`hooks`/`reminders` closed, `skills`/`commands`/
   `mcpServers` open, so the generator must reject `env`/`cwd`/`headers` itself), `enabledDefault`
   as a real JSON boolean, and both `command` and `commandWindows` on every hook.
6. **Ship `omm uninstall` before `omm install`** — `plugins remove --delete-data`, a merge-not-restore
   settings path, and the CI test that installs into a clean `$HOME`, uninstalls, and asserts the
   filesystem is byte-identical (MATRIX M6, which nobody in the surveyed ecosystem does).
7. **Settle `context_slimming`** (one sandbox run) before finalising the published skill ceiling.
8. **Amend the source reports** so nobody re-derives dead claims: `00-SYNTHESIS.md` claim 11b and §7
   unknowns 1/3/5/7/8/9/10/11/12; §0 fact 1 and the Appendix-A catalog row (three-stage
   degradation); §6.3 (tier inversion); §6.5 rules 3/4/9; §6.6 (M3 split); Appendix A (add the
   combined 31,984 budget, the 4094 inventory budget, the 256-plugin ceiling);
   `msp-protocol.md` §1 (`settings.json → provider` routes `serve`); `ohmy/00-MATRIX.md` M17 and §7.4.

---

## Appendix — the corrected numbers a builder needs on day one

```
IDENTITY
  plugin id                      omm            (len(pid)+len(sid) <= 18 for MCP; also 22 B/entry cheaper)
  manifest dir                   .muse-plugin   (assert manifest_family == "native")
  reserved ids (9)               skill-reminder goal-reminder memory-reminder todo-reminder
                                 verify-reminder scope-reminder tbh-reminders loop muse-core

CATALOG BUDGET   (order 200, skills_catalog)
  hard cap                       32000 B     (header 363 + footer 35)
  Meta's 15 built-ins            10060 B     refundable via `muse skills disable bundled:<id> --scope built-in`
  room for a bundle              21542 B     -> 31602 B with built-ins disabled  (+48%)
  entry cost                     38 + len(display_id) + len(display_path) [+ len(desc) + 36]
  order                          bundled -> project/user filesystem -> plugin   (plugin starved first)
  ~skills at 250 B desc, pid=omm  ~55 with built-ins on, ~81 with them off
  degradation                    stage 2 (descriptions dropped) is SILENT; stage 3 (entries dropped) prints

ROUTING BUDGET   (order 200 + order 201)
  combined cap                   31984 B     third outcome: selected_skills:rejected:combined-budget
  entries per turn               32 globally across all handlers
  description                    1024 B      hook stdout 16384 B
  library location               <ws>/.omm/skills/  — in-workspace, non-discovered, real files, copied

PACKAGE
  manifest                       131072 B inclusive
  filesystem entries             4096        path depth 16
  inventory budget               4094 units  (1/class + 2/skill + 1/command|hook|mcpServer|reminder)
  per-class max                  skills 2046 · commands 4093 · hooks 2153 · mcpServers 2902 · reminders 43
  enabled plugins                256         (257 -> plugin_preflight_overflow, soft)

TRUST
  MCP spawns only at             trusted_enabled
  killed by                      review_needed | trusted_disabled | modified | plugins disable | TUI trust dialog
  install-time gate              MUSE_EXPERIMENTAL_PLUGINS=1   (runtime is NOT gated)
  routing gates                  MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1 + ..._APPLY=1

PATHS
  personal_project memory        $XDG_DATA_HOME/muse/memory/projects/<slug96>-<fnv1a64hex>/
  undocumented residue           ~/Library/Application Support/Muse/session-name-authority/   (real HOME, always)
  enterprise root                /Library/Application Support/<leaf unknown>   — do not build on it
```

**Source reports:** `settle/plugin-mcp.md` · `settle/skill-routing.md` · `settle/settings-plugins.md` ·
`settle/plugin-contract.md` · `settle/quotas.md` · `settle/loose-ends.md` · `settle/canary-diff.md`,
each with its verification section appended in place.
