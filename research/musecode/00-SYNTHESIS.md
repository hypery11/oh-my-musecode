# oh-my-musecode — Architecture & Opportunity Synthesis

**Subject:** Meta Muse Code (internal codename `tbh`), binary `muse-aarch64-macos`, version
`1.0.1-R2006.1`, build `e27e408b66`, stripped arm64 Mach-O.
**Sources:** 12 reverse-engineering passes (each independently adversarially verified) + 4
ecosystem surveys. Full reports listed in Appendix D.
**Date:** 2026-09-02.

Confidence discipline used throughout:

| Marker | Meaning |
|---|---|
| **PROVEN** | Reproduced at runtime in a sandbox, or an exact byte string extracted from the binary at a named offset, **and** survived an adversarial re-verification pass. |
| **PROVEN⁻** | Reproduced, but only by one pass; the verifier did not retest it. Treat as strong but single-sourced. |
| **INFERRED** | Deduced from adjacent literals, struct arity, or documentation text. Not executed. |
| **CONTESTED** | The original report and its verifier disagree, and the disagreement is unresolved. |

Claims that a verifier **refuted** are dead and do not appear here except in Appendix C, where
they are listed explicitly so nobody re-derives them. Claims a verifier **corrected** appear only
in their corrected form.

---

## 0. Verdict in one page

**Muse Code already has the loader. It does not have the ecosystem.**

oh-my-zsh's kernel is 236 lines; its value is 359 plugins, a three-line config, a git-ignored
override directory, and an updater that respects the user. Muse Code inverts this. It ships a
compiled Rust extension runtime that is *better than every oh-my-X loader ever written*: a
schema-versioned plugin manifest, a validator with 25 typed diagnostic codes, content-addressed
package caching, per-capability trust bound to a package digest, two lockfiles, git marketplaces,
an offline hook test harness, and native ingestion of Claude Code's and Codex's plugin formats.
And it ships that runtime with **no public content, no registry, and effectively no
documentation** — the `muse plugins` subsystem has exactly one oblique mention in the entire
public documentation corpus (skills may come from "enabled plugin bundles"), and every published
review says "no plugin ecosystem, no marketplace, no community catalogue exists." All three
exist.

So oh-my-musecode is not the OMZ kernel. It occupies OMZ's *other* slot: the curated bundle, the
convention layer above the manifest, the distribution channel, and — the thing no oh-my-X ever
built and the thing Muse most needs — **per-source cost attribution**.

Three facts set the whole design:

1. **The unit of cost is tokens, not milliseconds.** Every extension mechanism in Muse ends up as
   bytes in one `POST /responses` body. The skills catalog is capped at 32,000 bytes and **silently
   drops whole skills past that with zero diagnostics anywhere** (600 skills → 426 rendered, 189
   gone, `diagnostics: []`). That is oh-my-zsh's 400ms-with-no-attribution failure, except it
   degrades correctness rather than latency, and it is invisible. PROVEN.
2. **Trust is the master gate, and no CLI can grant it.** Project rules, project skills, project
   hooks, project workflows, project agent definitions and project plugin installs are all inert
   until the workspace is trusted. `--trust-workspace` explicitly does not persist. There is no
   `muse trust` subcommand. Only the TUI's first-run prompt writes `$CONFIG_DIR/trust.json`.
   Hand-writing that file works. PROVEN.
3. **The Claude Code format is a tri-vendor de-facto standard.** Muse reads `~/.claude/skills`
   live with no import step, installs `.claude-plugin/plugin.json` bundles, reads
   `.claude-plugin/marketplace.json`, honours `${CLAUDE_PLUGIN_ROOT}`, exports
   `CLAUDE_PLUGIN_ROOT`/`CLAUDE_PLUGIN_DATA` to hooks, and falls back to `CLAUDE.md`. Codex CLI's
   own Rust loader independently lists `.claude-plugin/plugin.json` in its manifest-candidate
   order. **Author once in the Claude shape and you ship to three agents.** PROVEN for the Muse
   half; PROVEN⁻ for the Codex half (extracted from the Codex binary by the ecosystem survey,
   not re-verified here).

The single largest *product* gap: **the status line and prompt cannot be themed.** oh-my-posh is an
entire product built on that surface. Muse renders `provider · [effort ·] cwd [· YOLO]` from
compiled code, and there is no template, format, or component key anywhere in the 14-field
`TuiSettings` struct. Any oh-my-musecode roadmap that promises prompt themes is lying. PROVEN.

---

## 1. Muse Code architecture map

### 1.1 What it is, and how it arrives

A single statically-shipped Rust binary plus a bash launcher. The launcher (`muse-launcher.sh`)
auto-updates **hourly by default** (`MUSE_UPDATE_INTERVAL_SECONDS`, default 3600) from a signed
release channel, then `exec "$binary" "$@"` with argv passed through verbatim. It adds no commands
and no flags. PROVEN.

Consequences that constrain any framework:

- `MUSE_NO_AUTO_UPDATE`, `MUSE_SYNC_UPDATE`, `MUSE_UPDATE_INTERVAL_SECONDS`, `MUSE_LAUNCHER_URL`,
  `MUSE_CHANNEL_URL`, `MUSE_DOWNLOAD_HOST`, `MUSE_RELEASE_INFO`, `MUSE_AUTH_URL`, `MUSE_AUTH_PATH`,
  `MUSE_CLIENT_ID`, `MUSE_LOGIN` are **launcher-only**. `grep -c MUSE_NO_AUTO_UPDATE` over the
  binary's strings returns **0**. A framework that wants a pinned binary must wrap the launcher,
  not the binary. PROVEN.
- A second release channel exists and is ahead: `muse-stable` → `1.0.1-R2006.1` (`state: public`),
  `muse-canary` → **`1.1.0-R2009.1`** (`state: canary`), selectable with `MUSE_CHANNEL_URL` alone.
  Neither channels nor the hourly update are documented. PROVEN⁻ (live network probe by the
  documentation survey; not re-verified).
- The published docs changelog stops at **0.2.1** while stable is 1.0.1. PROVEN⁻.
- Windows binaries ship in both release manifests (`x86_windows` 311 MB, `aarch64_windows`
  283 MB) plus an undocumented `muse.pkg`, and `muse sandbox windows check|setup` exists —
  while every doc and review says macOS/Linux + WSL2 only. PROVEN⁻.

### 1.2 Crate map (from leaked panic paths and v0 mangled symbols)

Root: `fbcode/musecode/build/src/crates/`.

| Crate | What lives there (recovered module paths) |
|---|---|
| `tbh_config` | `paths.rs` (4 path kinds), `settings.rs`, `rules/diagnostic.rs`, `skills/{frontmatter,loader,id,plugin,plugin_activation,captured,diagnostic}.rs`, `trust/`, `enterprise/`, `gate_registry/diagnostics.rs`, `feature_provider/`, `agent_definitions/composition/diagnostic.rs`, `workflows/diagnostic.rs` |
| `tbh_agent` | `runtime/workflow_work_stop/start_admission.rs`, `hooks/selected_skills/adapter`, `command_invoked::BUILTIN_SLASH_COMMAND_NAMES`, `agent_definitions::catalog` |
| `tbh_agent_definition` | `source::{DefinitionSource,DefinitionScope,…}`, `parser::{SessionOverlayFailure,session::JsonNode,yaml_core}`, `registry`, `diagnostic::vocabulary::DiagnosticReasonV1` (51 variants), `grant::audit::codec`, `canonical::scaffold::declaration`, `inventory::codec::EntryWire` |
| `tbh_skills` | `generation`, `bundled_source`, `invocation_model`, `activation_update`, `model::requests`, `store::{files,sources}`, `security::package_files`, `selected` |
| `tbh_plugins` | `capability_snapshot/diagnostic.rs`, `agent_definition_inventory::preflight` |
| `tbh_tui` | 90 recovered module paths incl. `slash.rs`, `terminal/msp_session_bridge.rs`, `terminal/background_probe/overlap`, `theme_picker::showcase`, `state/client_config/theme`, `splash`, `highlight`, `render/text/markdown`, `login/flow/activation` |
| `tbh_skill_reminder` | `plugin` |
| MSP / protocol | schema bundle exported by `muse schema`; TUI is itself becoming an in-process MSP client (`MUSE_EXPERIMENTAL_TUI_MSP_CLIENT`, `tbh_tui-inproc`) |

Vendored: `saphyr_parser` (YAML frontmatter), `syntect` (216 syntax scopes, ~200 languages —
the bat/two-face extended set), `crossterm`, V8 (workflow script engine), Bubblewrap (embedded
Linux sandbox helper).

### 1.3 The CLI is five parsers glued together

16 top-level commands. 14 advertised; `plugins` is revealed only by `MUSE_EXPERIMENTAL_PLUGINS=1`,
and `workflows` is **never advertised in any `--help` yet permanently enabled**. PROVEN.

```
resume  exec  config  export  trace  skills  plugins  sandbox  schema  serve
session-message  auth  login  logout  init  workflows
```

Five incompatible error dialects, and `--help` resolves only at depth 1–2:

| Parser | Commands | Unknown-flag text |
|---|---|---|
| clap | root, `resume`, `trace inspect`, `auth set` | `error: unexpected argument '--X' found` |
| hand-rolled A | `exec`, `skills`, `init`, `session-message` | `unknown option --X` |
| hand-rolled B | `plugins` | ``unknown plugins list option `--X` `` |
| hand-rolled C | `serve`, `schema` | `muse serve: unknown option --X` / ``muse schema generate-ts: unexpected argument `--X` `` |
| hand-rolled D | `export`, `workflows`, `login`, `logout`, `config` | `unexpected export argument: --X` / `unknown workflows save argument: --X` |

Plus two dialects the first pass missed: `muse serve zzz` → `serve takes no positional arguments`;
`muse schema zzz` → ``muse schema: unknown subcommand `zzz` ``. PROVEN.

**Exit codes are not uniform and a wrapper must not assume clap semantics.** Corrected form:
parse failure is **2** across all five parsers (including an unknown *root* flag); **1** is
reserved for runtime/gate failures — ungated `session-message list` exits 1, `init` on an existing
`AGENTS.md` exits 1, `sandbox windows check` on macOS exits 1, a hook block makes `muse exec`
exit 1. `workflows` bare, `schema zzz`, `trace` bare all exit 2. PROVEN.
The exit code for an unknown *top-level token* (as opposed to flag) is **CONTESTED** — the first
pass recorded both 0 and 1 in different places and the verifier did not settle it. Do not build
error detection on it.

Two environment variables are validated as strict enums **at process start for every subcommand**,
so a typo bricks even `muse --version`: `MUSE_ENABLE_WEB_TOOLS` (`1,true,on,yes,0,false,off,no`)
and `MUSE_WEB_SEARCH_MODE` (`client,hosted,off`). PROVEN.

### 1.4 The load order

This is the pipeline a framework plugs into. Every step is observable offline in
`$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-<uuid>.log`, which is written on most
non-TUI invocations with **no auth and no network**. It is by far the best instrument in the
binary and it is undocumented. PROVEN.

```
  launcher            hourly update check → exec binary (argv verbatim)
    ↓
  env enum validation MUSE_ENABLE_WEB_TOOLS, MUSE_WEB_SEARCH_MODE  (fatal on typo)
    ↓
  path.resolved       config_root ← XDG_CONFIG_HOME/muse | HOME/.config/muse
                      data_root   ← XDG_DATA_HOME/muse   | HOME/.local/share/muse
                      model_catalog_cache, feature_config_cache  (both under data_root)
    ↓
  settings.load       $CONFIG_DIR/settings.json  — schema_version:1 HARD-REQUIRED,
                      malformed = hard startup failure, unknown keys silently ignored
    ↓
  enterprise planes   system_file | macos_managed_preferences | windows_machine_policy
                      (all `absent` by default; every execution.* field is field_not_activated)
    ↓
  gate.resolve × 41   one trace line per gate: enabled=<bool> source=default|override
    ↓
  trust.resolve       $CONFIG_DIR/trust.json | --trust-workspace (run-only) | --yolo
    ↓
  run_preset.resolve  --model > MUSE_MODEL > --preset > settings.model
  permission commit   built-in profile | --permission-profile | launch_overrides | yolo
  credential.status   --api-key-stdin > META_API_KEY > keychain|file > OIDC device flow
    ↓
  capability compose  rules.context_load        (user 1 file + project N files)
                      skills.catalog_load       (user/project/bundled/plugin)
                      agent_definition.sources_load (6 slots)
                      plugin_capability_snapshot.compose
                      named_workflow.catalog_load
                      hooks (managed → user → project → plugin)
    ↓
  session open        <data>/muse/sessions/YYYY/MM/DD/<uuid>/session.jsonl
    ↓
  context assembly    ordered developer-role blocks (§1.5)
    ↓
  model request       POST {endpoint_transport.base_url}/responses
```

### 1.5 The context-block model — read this before designing anything

**This is Muse's `.zshrc`.** Everything a framework installs becomes an ordered, budgeted,
developer-role block in one HTTP body. Observed order in a trusted git workspace with the echo
provider (id, order, bytes): PROVEN.

| order | block id | bytes (observed) | source |
|---:|---|---:|---|
| 85 | `workspace_identity` | 272 | runtime |
| 95 | `sandbox_policy` | 812 | runtime |
| 96 | `security_mode` | 467 | runtime |
| 100 | `rules_file` | 332 | **AGENTS.md chain** |
| 180 | `workflow_choice` | 2,896 | runtime |
| 181 | `workflow_cookbook` | **18,056** | runtime |
| 186 | `subagent_delegation` | ~1,000 | runtime |
| 200 | `skills_catalog` | 9,440 | **skills** |
| 201 | `selected_skills_catalog` | — | hook `skills.v1` (double-gated) |
| 240 | `session_identity` | 809 | runtime |
| 900000+n | `hook:<event>:<scope>:<n>` | — | **hook `additionalContext`** |
| 4294967295 | `memory_snapshot` | 16,304 | **memory** |

The three orders in bold are the ones a framework owns. `memory_snapshot` is deliberately last
(u32::MAX) — it wins on recency.

Total observed `instructions` field: 36,613 bytes. Whole `tools` array: 40,284 bytes. The
`workflow` tool description alone is 7,551 characters. So on a stock trusted session, roughly
**80 KB of the prompt is framework-shaped surface before the user types anything**, and the
single largest block Muse itself contributes (`workflow_cookbook`, 18 KB) is switched off by
`run.workflow_trigger_mode: "off"` or `"explicit"`. PROVEN.

Progressive disclosure is real and is the lever: the catalog carries only
`id / scope / path / description (+ short-description)`; bodies are fetched by the `read_skill`
tool on demand. Design every shipped item so its **catalog line is cheap and its body is paid for
only on invocation**. That is oh-my-zsh's completion-only-plugin trick translated to context.

### 1.6 Session model

Append-only JSONL event log per session, plus SQLite sidecars and a materialized view.

```
$XDG_DATA_HOME/muse/
├── sessions/YYYY/MM/DD/<session-uuid>/
│   ├── session.jsonl                     append-only durable event log
│   ├── .session.lock                     flock, body `pid=N`
│   ├── cron.db                           SQLite: cron_jobs (17 cols)
│   ├── session.peer-history.sqlite3      SQLite: source_snapshot/retained_records/stable_ids
│   ├── approval-review/                  (empty without a live judge)
│   ├── cli-<uuid>.log                    per-process bootstrap trace
│   └── subagent/<child>/session.jsonl    child sessions (nested_session_v1)
├── sessions/.msp-view-v1/<session-id>/   HEAD.json + journal-*.bin + index-*.bin + 2 snapshots
├── session-index.db                      global SQLite index, 33 cols, READER-built
├── plugins/  skills/bundled/  local-tracing/  memory/  model-catalog/  crashes/
```

Key facts a framework can build on. All PROVEN:

- Envelope: `{schema_version, id, stream{kind,id}, sequence, recorded_at(µs), record_type,
  durability, causation_id, payload_type, payload_schema_version, payload}`. `record_type` observed:
  `event` and `reconciliation`. Line 1 is a `retained_frame` transaction batching permission
  records as JSON strings with a `content_sha256`.
- MSP-created sessions get only `.session.lock` + `session.jsonl` — the SQLite sidecars are a
  CLI/TUI artifact.
- `session-index.db` is built by the first **reader** (`muse export`, `muse serve` `session/list`,
  the resume picker), not by the writer. It carries `msp_fork_source_session_id`,
  `msp_fork_cut_cursor`, `msp_fork_command_id`, and a `search_text` column holding a **lowercased
  copy of the absolute workspace path**.
- `muse export` emits one `export_schema_version: 1` document, offline, RAW by default.
  `--redacted` rewrites sha256 digests to `sha256:[redacted]` **and recomputes the retained
  frame's `content_sha256` over the redacted children** — so a redacted export is no longer
  hash-verifiable against the on-disk log.
- `muse trace inspect --session-log <f> --render-mode verbose` dumps the entire run configuration
  including every context block's text, the toolset, model input lanes, and request digests. This
  is the offline oracle for "what did we actually send."
- `--no-session-log` gives memory-only sessions and hard-blocks `--session-id`, `--worktree`,
  resume, `/permissions`, export, feedback and rageshake, each with its own error string.
- Headless continuation is `muse exec --session-id <uuid>` (appends, writes `session.resumed`).
  `muse resume` is TUI-only (`Device not configured (os error 6)` without a tty).

### 1.7 Config layering — and the load-bearing negative

**Two roots, three tiers, and no workspace settings file.** PROVEN, including the negative:
26 candidate paths (`.muse/settings.json`, `.agents/settings.json`, `muse.json`, `~/.muse.json`,
`~/.config/tbh/settings.json`, …) were planted with a poison document and **none is ever read**.

| Tier | Where | Notes |
|---|---|---|
| Enterprise `policy` | system file / `com.tbh.tbh` managed prefs / Windows machine policy | Containers `execution`, `model_egress`, `privacy`, `extensions`, `local_session_messaging`. **Every `execution.*` field is `field_not_activated` in 1.0.1** — an org cannot forbid `--yolo` or pin a permission profile today. |
| Enterprise `defaults` | same sources, `settings` container | 20 members, genuinely working. Can pin `provider`, `model`, `reasoning_effort`, `run`, `tui` (11 of 14 keys), `mcp_servers`, `presets`, `max_consecutive_stop_hook_continuations`, `endpoint_transport`, `provider_retry`. Cannot set `hooks`, `permissions`, `plugins`, `runtime_capabilities`, `agent_definitions`, `model_catalog`. |
| User | `$CONFIG_DIR/settings.json` | 29 top-level keys. `schema_version: 1` hard-required. |
| Workspace | **files only**: `AGENTS.md`, `.agents/skills/`, `.agents/agents/`, `.agents/memory/`, `.agents/workflows/`, `.muse/hooks.json`, `.muse/worktrees/` | No settings document exists at this tier. |
| Env | `MUSE_*` / `TBH_*` | **Env beats settings.json** — proven with `MUSE_ENABLE_WEB_TOOLS` vs `tools.web_search.mode` using the session log's `active_tools` as the oracle. |
| CLI flags | | Beat settings.json (`--provider` overrides a bogus saved provider). `--base-url` beats `endpoint_transport.base_url`. |

Three traps that will generate "my config does nothing" bug reports:

1. **settings.json is rewritten from the typed struct during ordinary sessions.** An echo session
   adds `tui.foreign_context_notice_shown: true`; `muse skills enable/disable` writes the
   activation tree. The rewrite **drops unknown keys** and reformats to 2-space pretty-print. A
   framework must not stash a marker key there. PROVEN (a `_omm_marker` key was destroyed by one
   ordinary run).
2. **A bad enum value anywhere under `tui.*` makes the whole file malformed** and every subcommand
   fails with `malformed settings file at <path>: <serde message>`. A *semantically* invalid
   keymap entry, by contrast, parses fine and is **silently ignored**.
3. `mcpServers` and `mcp_servers` are **both live but not aliases** — and with **both present the
   entire MCP config is silently discarded**: no server starts, no error, no diagnostic. PROVEN.

There is one free offline linter for all of this: `muse config validate --plane defaults --file X`
(no gate required). It catches unknown members, wrong types, semantic invalidity and
`field_not_activated`, with dotted locations like `location=settings.tui.keymap`. PROVEN.

### 1.8 Trust — the master gate

`$CONFIG_DIR/trust.json`, schema recovered by positional serde probing:

```json
{"schema_version": 1, "projects": {"<absolute workspace path>": {"decision": "trusted"}}}
```

`decision` ∈ `trusted | untrusted`; `schema_version` must be `1` (2 → `unsupported trust store
schema version 2`); a **malformed file aborts the session with exit 1**. PROVEN.

| Surface | Trust-gated? | Escape hatch |
|---|---|---|
| Project `AGENTS.md` | **yes** | `--trust-workspace` (run-only), `--yolo` |
| Project skills (`.agents/skills`, `.claude/skills`, `.codex/skills`) | **yes** | `--trust-workspace` |
| Project hooks (`.muse/hooks.json`) | **yes** | `--trust-workspace`, `--yolo` |
| Project workflows (`.agents/`, `.claude/`, `.codex/workflows`) | **yes** | **none** — `muse workflows` has no `--trust-workspace` and no `--workspace` flag; only `trust.json` works |
| Project agent definitions (`.agents/agents`) | **yes** | (TUI prompt; `trust.json`) |
| `plugins install --scope project` | **yes** | `trust.json` only |
| **Project memory (`.agents/memory/`)** | **NO** | — it loads even when the workspace is untrusted and its `AGENTS.md` is refused |

That last row is a genuine security finding and a genuine opportunity: repo-controlled Markdown
reaches the model through the memory channel with no trust prompt. PROVEN.

**No CLI writes `trust.json`.** `--trust-workspace` is documented "does not save trust", there is
no `muse trust` subcommand, and ~60 invocations across 20 throwaway homes never created it. The
TUI's first-run prompt ("Do you trust this workspace? 1 Trust and continue / 2 Quit") does write
it. Hand-writing it works everywhere. PROVEN.

---

## 2. Complete extension-point table

Every confirmed way to change Muse Code's behaviour from outside the binary, with the exact
file / env var / flag that activates it. Ranked by leverage for oh-my-musecode.

**Leverage** = how much of an oh-my-X-shaped product this one point can carry.
**Gate** = whether it needs an experimental env var or workspace trust.

### 2.1 HIGH leverage

| # | Extension point | Exact mechanism | Gate | Conf. |
|---|---|---|---|---|
| 1 | **Personal skills, zero-install** | `$CONFIG_DIR/skills/<id>/SKILL.md` · **`~/.agents/skills/`** · **`~/.claude/skills/`** · **`~/.codex/skills/`** (or `$CODEX_HOME/skills`, which *replaces* `~/.codex`). Precedence in that order, first-hit-wins per id. Foreign roots tagged `origin-family` / `written-for` in the catalog and killable with `context.foreign_personal_skills: false`. | none | PROVEN |
| 2 | **Project skills, zero-install** | `<ws>/.agents/skills/` > `<ws>/.codex/skills/` > `<ws>/.claude/skills/` (note: foreign order is **inverted** vs personal scope). Exactly one directory level deep; nested dirs are silently not discovered. | **trust** | PROVEN |
| 3 | **Managed skill store (a real package manager)** | `muse skills install <path> [--scope user] [--name N] [--force] [--json]`, `update <id>`, `uninstall <id> [--keep-files]`, `validate <path> --json`, `import --from claude\|codex [--dry-run] [--json]`. Writes `$CONFIG_DIR/skills/.muse/lock.json` (per-file sha256, `trust`, `scan{status,warnings}`, `source{type,…}`), `audit.log` (JSONL), `.skills.lock`, `quarantine/`, `import-quarantine/<id>/QUARANTINE.txt`. | none | PROVEN |
| 4 | **Personal rules — the one-file `.zshrc` slot** | `$CONFIG_DIR/AGENTS.md` → `$CONFIG_DIR/CLAUDE.md` → `~/.claude/CLAUDE.md` → `$CODEX_HOME\|~/.codex/AGENTS.md`. **Exactly one file loads, first hit wins.** Rungs 3–4 killed by `context.foreign_personal_rules: false`, `--no-foreign-personal-context`, or `MUSE_EXPERIMENTAL_FOREIGN_PERSONAL_CONTEXT_KILL`. Loading a foreign one prints `muse: Including your Claude Code personal rules — manage with /settings.` | none | PROVEN |
| 5 | **Project rules, accumulating** | `AGENTS.md` in **every directory from the VCS root down to cwd**, rendered shallow→deep — they **accumulate**, not nearest-wins. `CLAUDE.md` is a same-directory fallback only where no sibling `AGENTS.md` exists (and shadowing prints a warning). Without a `.git` marker the walk collapses to cwd alone. Caps: **256,000 B/file** and **65,536 B aggregate**, both with explicit stderr warnings. | **trust** | PROVEN |
| 6 | **Hooks — four tiers, 17 events** | managed (`settings.managed_hooks_path` **or** `TBH_MANAGED_HOOKS_PATH`, which *replaces* it) → user (`settings.json` → `"hooks"`) → project (`<ws>/.muse/hooks.json`) → plugin (`capabilities.hooks[]`, 100000+ band). No `schema_version`; unknown top-level keys ignored; `managed_hooks_env_vars` inside a hook file rejects the whole file. Event keys are **PascalCase only**. | project tier: **trust**; plugin tier: **`MUSE_EXPERIMENTAL_PLUGINS` + approve** | PROVEN |
| 7 | **Plugin bundle — the one-artifact distribution unit** | `.muse-plugin/plugin.json` (`schemaVersion:1`, `name`, `version`, **`description` required non-empty**, `compat.manifestDir`, `capabilities`). Families: `skills`, `commands`, `hooks`, `mcpServers`, `reminders` + implicit `agents/`. Install: `muse plugins install <path>\|<plugin>@<marketplace> [--scope user\|project]`. | **`MUSE_EXPERIMENTAL_PLUGINS=1`** | PROVEN |
| 8 | **Git marketplace (the registry Muse already reads)** | `muse plugins marketplace add <name> <owner/repo\|git-url\|file://\|path>`. Catalog files probed inside the root: **`.claude-plugin/marketplace.json`** (Claude Code's exact schema; the binary carries `json.schemastore.org/claude-code-marketplace.json`) and `.agents/plugins/marketplace.json` (Codex). Remote per-plugin sources clone into `.muse-claude-sources/<i>/` inside the worktree and normalise to `local-path`. `tbh-curated` is reserved. | **`MUSE_EXPERIMENTAL_PLUGINS=1`** | PROVEN |
| 9 | **User settings document** | `$CONFIG_DIR/settings.json`, 29 top-level keys, `schema_version: 1` required. Governs provider/model/effort, `run.*`, `tui.*`, `tools.*`, `context_compaction.*`, `mcpServers`, `presets`, `skills.activation`, `runtime_capabilities`, `permissions`, `hooks`, `telemetry`, `notifications`, `endpoint_transport`. **Rewritten by the binary; unknown keys destroyed.** | none | PROVEN |
| 10 | **Skill activation tri-state** | `settings.skills.activation.{user\|bundled\|plugin}["<display locator>"] = on\|user-invocable-only\|off`, plus `projects["<abs ws root>"]["<ws-relative SKILL.md path>"]`. Keys are **SKILL.md paths / `bundled://` / `plugin://` URIs, not ids**. CLI writers: `muse skills enable\|disable\|user-only <sel> --scope user\|project\|built-in\|plugin`. | none | PROVEN |
| 11 | **MCP servers — the only third-party model-tool path** | `settings.json → mcpServers.<id>` (13 fields: `enabled mode transport command args env framing cwd url headers startup_timeout_sec enabled_tools disabled_tools`; `transport` ∈ `stdio\|streamable_http`, `mode` ∈ `required\|optional`, `framing` ∈ `auto\|content_length\|line_delimited_json`). **A settings-declared stdio server does launch under `muse exec`** (observed via its own startup-timeout error). `capabilities.tools` is rejected outright: *"a custom model tool is exposed by an MCP server, not by a direct `tools` capability."* | none | PROVEN |
| 11b | **MCP servers via a plugin bundle** | plugin `capabilities.mcpServers[]`, native transports `stdio\|http` only (`sse` rejected); native entries silently drop `env`, `cwd`, `headers`. **An installed and approved plugin MCP server did NOT start under `muse exec` OR `muse serve`** — zero `mcp.*` records — while the capability snapshot *was* composed (`mcp_servers=1`). TUI-only is INFERRED from `hooks/MCP still need restart`. | gate + **approve** | PROVEN (validation & install) / **INFERRED (does it ever start)** |
| 12 | **Permission profiles** | Four built-ins: **`:read-only`, `:ask-me` (default), `:auto-review`, `:unrestricted`** — the leading `:` is why user ids exclude it. User-defined: `settings.json → permissions.profiles.<id>` with four dimensions (`approval`, `reviewer`, `filesystem`, `network`) + `extends` (≤16 deep, cycle-checked). Selected by `--permission-profile <id>` or `permissions.default_profile`. | none | PROVEN |
| 13 | **Reminder agents — the deepest Muse-only surface** | plugin `capabilities.reminders[]`: a fully declarative sandboxed DSL — `decision{envelope, deliveryRole, fields[], bodyTemplate{text,slots[]}, validators[], proposal{6 branches}, lifecycle{4 policies}, limits{18 caps}}` plus `tools`, `blocking`, `defaultPriority`, `maxPriority`, `maxChildSteps`, `maxInstallsPerRun`, `reasoningEffort`, `context{conversation, feeds[]}`. Five first-party validator contracts bindable. No code executes. | **`MUSE_EXPERIMENTAL_PLUGINS`** to install; per-reminder `MUSE_EXPERIMENTAL_*_REMINDER` to run first-party ones | PROVEN (validator round-trip of the reference manifest) |

### 2.2 MEDIUM leverage

| # | Extension point | Exact mechanism | Gate | Conf. |
|---|---|---|---|---|
| 14 | **Named workflow scripts (JavaScript)** | `<ws>/.agents/workflows/<name>.js` · `<ws>/.claude/workflows/` · `<ws>/.codex/workflows/` (all three read; `.agents` shadows) and `$CONFIG_DIR/workflows/<name>.js`. CLI: `muse workflows save <name> --from <f.js> [--scope project\|user] [--overwrite]`, `list`, `run <entry> --headless-qa [--token-budget N\|Nk\|Nm]`, `recover <run-id>`. `save --scope project` writes **only** to `.agents/workflows/`. Name grammar `[a-z0-9][a-z0-9._-]*` ≤64. Callable as `workflow({"name":"..."})` and from `/workflows`. | project scope: **trust.json only** (no flag exists) | PROVEN |
| 15 | **Agent definitions (subagent personas)** | `<ws>/.agents/agents/**/*.md` (recursive, rooted at the workspace, no walk-up) and `$CONFIG_DIR/agents/**/*.md`. Markdown + YAML frontmatter; `name:` required and authoritative (filename ignored). Ephemeral overlay: **`--agents '<JSON map of name→definition>'`** (root/TUI only, not `exec`). Name grammar `[a-z]+(-[a-z]+)*` ≤128 B — **one bad name kills the entire session overlay**; filesystem sources drop per-entry instead. Six composition slots; `settings.agent_definitions.safe_mode: true` suppresses 4 of 6 including the CLI overlay. | project: **trust** | PROVEN |
| 16 | **Memory (personal + project)** | `$XDG_DATA_HOME/muse/memory/personal/` and `<ws>/.agents/memory/`. Both recursive. `MEMORY.md` inlined; other `.md` listed by relative path. Tools `read_memory` / `add_memory` / `edit_memory` (scopes `personal \| personal_project \| project`, default `personal_project`). Block ordered **last** at u32::MAX. Caps: **16,305 B total** and **48 listed files per scope**, both silent. | **none, even for project scope** | PROVEN |
| 17 | **Run presets** | `settings.presets.<name> = {provider, model, agent_profile, run}`; selected with `--preset <name>`. Built-ins `native-basic`, `miniswe`. Eight agent profiles exist (`native-basic`, `miniswe`, six `code-mode-*`). Precedence: `--model` > `MUSE_MODEL` > `--preset` > `settings.model`. | none | PROVEN |
| 18 | **Themes — a real theme system** | `$XDG_CONFIG_HOME/muse/themes/*.tmTheme` (case-insensitive extension; `.json` ignored; the **data** dir is not scanned), selected as `tui.theme = "custom:<file-stem>"`. 24 bundled themes (15 dark / 9 light) + `Default` + `Dynamic` (derived from an OSC-10/11/4 terminal probe). `/theme` filters by detected background — so `tui.terminal_background: auto\|light\|dark` matters. Downgrade via `tui.color_depth: auto\|truecolor\|256\|16\|none`; `NO_COLOR=1` disables colour. | none | PROVEN |
| 19 | **Keymap** | `settings.tui.keymap = {context: {action: [keyspec]}}` over 4 contexts (`app`, `composer`, `editor`, `navigation`) and 37 actions; `[]` unbinds. Interactive `/keymap` overlay (8 tabs) writes the same file. Validated **offline** by `muse config validate --plane defaults`. `app.commands` / `app.shell` / `app.files` are route-prefix actions constrained to **one printable character**; `/ ! @ ?` are reserved. | none | PROVEN |
| 20 | **Headless SDK via `muse exec --json`** | Emits the raw MSP durable-record stream as JSONL on stdout (13 records for a trivial echo turn). Plus `--session-id <uuid>` (resumable), `--max-model-steps`, `--max-tool-output-bytes`, `--prompt-file`, `--user-input-auto-resolve`, `--allow-workspace-switch`, and three undocumented flags (`--agents`, `--eval-context-compaction-strategy`, `--eval-workflow-api-version`). | none | PROVEN |
| 21 | **MSP host + the lease-free read plane** | `muse serve` — line-delimited JSON-RPC 2.0 over stdio. **40 client→server methods** (31 published + 9 routed-but-undocumented: `goal/{edit,clear,pause,resume}`, `workflow/{cancel,childControl}`, `task/{background,stop,stopAll}`). `session/list`, `session/read` and `view/page` take **no writer lease**, so a second host can observe every session on the machine while the user's TUI holds the lease — **runtime-verified**. Schema exported offline by `muse schema generate-json-schema\|generate-ts [--experimental]` with a fingerprint the server echoes at `initialize`. | **`MUSE_EXPERIMENTAL_SDK_ENABLED` is DEFAULT-ON and gates both `serve` and `schema`** — setting it to `0`/`false`/`yes`/`""` disables both | PROVEN |
| 22 | **Approval interception** | `approval/requested` → `approval/decide` over MSP, with argv-stage decomposition, a `requirementId` race guard, `suggestedPrefix` rule previews, and `ApprovalChoiceScope ∈ once\|session\|localPersistent`. Locally: hook events `PermissionRequest` (`{"behavior":"allow"\|"deny"}`) and `PreToolUse` (`permissionDecision` + `updatedInput` → effect `rewrite_selected`). | MSP path: SDK gate (default-on) | PROVEN |
| 23 | **Feature gates as a capability profile** | 41 `MUSE_EXPERIMENTAL_*` vars mapping 1:1 onto internal snake_case gate ids. **14 are default-ON**: `workflow_tool, local_session_messaging, bash_titles, bash_sandbox_escalation, git_sandbox_relaxation, first_turn_minimal_effort, memory_reminder, skill_reminder, goal_reminder, verify_reminder, scope_reminder, non_strict_tool_params, sdk_enabled, voice_native_capture`. Only 2 change the CLI surface (`PLUGINS`, `EXTERNAL_AGENT_INGRESS`); the other 39 gate in-session tools and TUI panels. Every resolution is logged as `gate.resolve gate=<id> enabled=<bool> source=default\|override`. | — | PROVEN |
| 24 | **Enterprise `defaults` plane** | `enterprise-defaults.json` via system file or macOS managed-preferences domain `com.tbh.tbh` (keys `enterprise_defaults_json` / `enterprise_policy_json`). Validate offline with `muse config validate --plane defaults --file X` (**no gate**). 20 settable members incl. `provider`, `model`, `reasoning_effort`, `run`, `tui` (11 of 14), `presets`, `mcp_servers`, `endpoint_transport`, `max_consecutive_stop_hook_continuations`. | none for validation; deployment needs admin write | PROVEN |
| 25 | **Cross-session messaging** | `muse session-message list\|send` behind `MUSE_EXPERIMENTAL_EXTERNAL_AGENT_INGRESS` (accepts `1\|on\|true`). Discovery is **machine-wide per-uid** via `/private/tmp/tbh-<uid>-rt/muse/ms-<12hex>.sock` + `.sock.lease` — **not** scoped to `XDG_DATA_HOME`, so it crosses profiles. `list` works; **`send` is stubbed** (`not implemented in this build`). | gate | PROVEN |

### 2.3 LOW leverage / situational

| # | Extension point | Exact mechanism | Gate | Conf. |
|---|---|---|---|---|
| 26 | **Slash commands via plugin `capabilities.commands[]`** | `commands/<id>.md` with YAML front matter (`description`, `argument-hint`) and `$ARGUMENTS` substitution. Appear in the composer picker and `/help → Custom commands`. **Does NOT require the plugins gate to *run*** — only to install/approve. | install: gate | PROVEN |
| 27 | **Bundled-skill shadowing** | A project/user skill named `plan` captures the bare `plan` selector; `bundled:plan` still resolves. Bundled skills with an invocation marker also appear as slash commands. | trust (project) | PROVEN |
| 28 | **`experimental-gate:` SKILL.md frontmatter** | Muse-only key (Claude Code ignores it as unknown) that gates a skill on a `MUSE_EXPERIMENTAL_*` name. An **unknown** gate excludes the skill entirely — and `muse skills validate` returns `valid: true, diagnostics: []` for it, so a lint gate does not catch it. | — | PROVEN |
| 29 | **Eval prompt-injection hooks** | `TBH_EVAL_APPEND_SYSTEM_PROMPT[_FILE]`, `TBH_EVAL_APPEND_DEVELOPER_PROMPT[_FILE]`, `TBH_EVAL_USER_STEER_FILE`. Real, undocumented, unsupported. Useful for benchmarking a framework; must never be shipped as a product API. | — | PROVEN (strings) / INFERRED (behaviour) |
| 30 | **Custom request headers** | `MUSE_CUSTOM_HEADERS`, **newline-separated** `Name: value` (commas are not separators). `authorization` is stripped. `MUSE_WWW_ROUTING` accepts exactly one value: `default_c1`. | — | PROVEN |
| 31 | **`muse init`** | Scaffolds exactly one file, `AGENTS.md`, adapting its "Project Layout" section to detected directories. `--dry-run` prints without writing; re-run on an existing file exits 1. Scaffolds nothing for hooks, skills, workflows or plugins. | none | PROVEN |
| 32 | **Sandbox posture** | `--sandbox-network restricted\|enabled\|proxy-only` (default proxy-only), `--disable-sandbox`, `--disable-write`, `--disable-shell`. macOS = Apple Seatbelt via `/usr/bin/sandbox-exec -p` with a `(deny default)` profile; Linux = Bubblewrap (system or embedded); Windows = `windows_elevated`, needs `muse sandbox windows setup`. Per-command escalation exists as a bash-tool param `sandbox_permissions: use_default \| require_escalated`. | escalation: `MUSE_EXPERIMENTAL_BASH_SANDBOX_ESCALATION` (association INFERRED) | PROVEN |
| 33 | **`workflow_cookbook` / `workflow_choice` suppression** | `settings.run.workflow_trigger_mode: "off"` removes the `workflow` tool **and** the 18 KB cookbook block; `"explicit"` keeps the tool and drops the cookbook. The cheapest single token saving available. | none | PROVEN |
| 34 | **`run.toolset`** | A settings array of tool names selecting the wire surface directly. Accepted vocabulary (28): `read_file edit_file write_file search bash bash_input shell monitor read_memory add_memory edit_memory create_goal update_goal get_goal report_progress cron_create cron_delete cron_list web_search web_fetch subagent_* artifact workflow`. `read_skill`, `write_todos`, `subagent_*` are **not removable**. | none | PROVEN |
| 35 | **`run.context_slimming`** | `skill_catalog_descriptions: full \| first_sentence`, `full_skill_description_ids: [...]`, `meta_context_note_enabled`, `session_identity_enabled`, `excluded_tool_names`. The direct lever on the 32,000-byte catalog budget. | none | PROVEN |
| 36 | **Claude-channel interop bridge** | `muse --internal-claude-channels-sidecar-v1` is a **live JSON-RPC/MCP server on stdio** (`serverInfo: tbh-session-messaging`, capability `experimental.claude/channel`), and `--internal-claude-channels-live-smoke-host-v1` execs the real `claude` binary and version-gates it (pinned to `2.1.220`). Muse ships a complete Claude Code plugin bundle in Claude Code's own on-disk format and drives Claude Code's own `plugin` CLI verbs to install it. | argv only | PROVEN |

### 2.4 Confirmed dead ends — do not build on these

| Non-point | Evidence |
|---|---|
| A workspace settings file | 26 candidate paths planted with poison; none read. PROVEN. |
| Auto-discovery of project plugin directories | Valid packages in `<ws>/.agents/plugins`, `.claude/plugins`, `.codex/plugins`, `.muse/plugins`, `plugins/` — none discovered, in **both** trusted and untrusted workspaces. Plugins must be explicitly installed. PROVEN. |
| A status-line / prompt template | No `statusline`/`prompt`/`format` key exists in any of the 29 settings keys or the 14 `tui` fields. PROVEN. |
| `allowed-tools` in SKILL.md or command frontmatter | Parsed, canonicalised, recorded — and explicitly **not enforced**, with a warning-severity diagnostic saying so. PROVEN. |
| Enterprise-forced execution policy | Every `execution.*` field returns `field_not_activated`, including `forbid_approval_bypass`, `forbid_sandbox_bypass`, `permission_profiles`, `approval_modes`, `tool_rules` config. PROVEN. |
| `capabilities.tools` / `agents` / `outputStyles` / `settings` / `apps` in a native manifest | Hard `unsupported-capability` errors. `lspServers` / `userConfig` are warn-and-ignore. PROVEN. |
| `.muse/settings.json`, `$CONFIG_DIR/hooks.json`, `~/.muse/hooks.json` | Never read. PROVEN. |
| `MUSE_SESSIONS`, `MUSE_NO_SESSION_LOG`, `MUSE_HOME`, `MUSE_CONFIG_DIR` | Not env vars the binary reads. `MUSE_SESSIONS` is documentation prose inside a bundled skill; `NO_SESSION_LOG` is a clap ID. PROVEN. |
| `plugins/data/<plugin-id>` existing | Advertised in the hook env (`MUSE_PLUGIN_DATA_DIR`) but **not created**. A plugin must `mkdir -p` it. PROVEN. |
| Plugins on the MSP wire | `grep -io plugin msp.schema.json \| wc -l` → **0**. Also 0 in the TS export. PROVEN. |
| `developerPrompts` capability | Hard-gated off with **all 41** experimental gates exported =1. PROVEN. |

---

## 3. Claude Code / Codex compatibility verdict

**Verdict: compatibility is real, first-class, deliberately engineered, and far stronger than
anything Meta documents. Author once in the Claude Code shape and you ship to three agents.
But the tool *vocabulary* does not port, and that is where naive reuse breaks.**

### 3.1 Six independent compatibility layers, all PROVEN

**Layer 1 — skills, live, no import step.** This is the biggest single finding for content reuse.
`~/.claude/skills` and `~/.codex/skills` are scanned at **user scope**, and `<ws>/.claude/skills`
and `<ws>/.codex/skills` at **project scope**. No `muse skills import` needed; no plugin needed;
no gate. Foreign entries are tagged in the model catalog:

```xml
<skill id="proj-cc" scope="project" path=".claude/skills/proj-cc/SKILL.md"
       origin-family=".claude" written-for="Claude Code">
<skill id="pdup"    scope="project" path=".codex/skills/pdup/SKILL.md"
       origin-family=".codex"  written-for="Codex">
```

so the model is told to discount harness-specific instructions. **A skill pack installed into
`~/.claude/skills` is already live in Muse without touching Muse at all.**

**Layer 2 — plugins.** `.claude-plugin/plugin.json` installs as `manifest_family:
claude-compatible` (lockfile value `claude`). Translated: `skills`, `commands`, `hooks` (the whole
`hooks.json` shape including `matcher`, `timeout`→`timeout_ms`×1000, `statusMessage`, `async`, and
inline-object form), `mcpServers` (`command`+`args` flattened). `${CLAUDE_PLUGIN_ROOT}` is expanded
to the content-addressed cache root and Claude hooks are executed **through the user's shell**
(native hooks are exec'd directly with no shell and no expansion).
`.codex-plugin/plugin.json` is thinner: only `skills` (a single string dir path) and `mcpServers`
(a `.mcp.json` path); `interface.displayName` is honoured.

**Layer 3 — marketplaces.** Both catalog files are probed inside a marketplace root:
`.claude-plugin/marketplace.json` (Claude Code's exact schema — the binary even carries
`https://json.schemastore.org/claude-code-marketplace.json`) and `.agents/plugins/marketplace.json`
(Codex). A Claude marketplace file works unmodified. Remote per-plugin sources
(`{"source":"url"}` / `{"source":"github","repo":…}`) require the marketplace itself to be a git
source and are cloned into `.muse-claude-sources/<i>/` inside the worktree.

**Layer 4 — rules.** `CLAUDE.md` is a same-directory fallback for `AGENTS.md` at project scope,
and `~/.claude/CLAUDE.md` / `~/.codex/AGENTS.md` are read **in place** as user rules (not
imported), tagged `written-for="Claude Code"` / `"Codex"`.

**Layer 5 — hook wire format.** The envelope is Claude Code's: snake_case in
(`hook_event_name`, `tool_name`, `tool_input`, `session_id`, `turn_id`, `cwd`, `transcript_path`,
`model`, `permission_mode`), camelCase out (`hookSpecificOutput.permissionDecision`,
`additionalContext`, `updatedInput`, `decision:"block"`, `systemMessage`, `suppressOutput`),
exit 0 = ok / exit 2 = blocking with stderr as the reason. Env aliases `CLAUDE_PLUGIN_ROOT` /
`CLAUDE_PLUGIN_DATA` / bare `PLUGIN_ROOT` / `PLUGIN_DATA` are exported alongside the `MUSE_*` names.

**Layer 6 — the reverse direction.** Codex CLI's own Rust marketplace loader carries an ordered
manifest-candidate list including `.claude-plugin/plugin.json` and `.claude-plugin/marketplace.json`.
So the `.claude-plugin/` shape is implemented by **three** vendors. PROVEN⁻ (extracted from the
Codex binary by the ecosystem survey; not re-verified by this synthesis).

Bonus, and stranger than the rest: Muse ships a **complete Claude Code plugin bundle in Claude
Code's own on-disk format** (`plugins/tbh-session-messaging/.claude-plugin/plugin.json` +
`.mcp.json`), drives Claude Code's own `plugin install|enable|disable|update|uninstall` CLI verbs,
and `muse --internal-claude-channels-sidecar-v1` is a live MCP server on stdio advertising
`experimental.claude/channel`. Meta built a two-way bridge. PROVEN.

### 3.2 What does not port — the exact loss list

| Claude Code / Codex feature | Muse outcome | Diagnostic |
|---|---|---|
| `agents/` in a Claude or Codex plugin | **dropped**; inventory comes back `status: "empty"` | ``Claude manifest field `agents` declares unsupported behavior``; declaration `agent:<id> / unsupported` |
| `outputStyles`, `lspServers`, `userConfig` | dropped | `unsupported-capability` / `unsupported-field` |
| `author`, `homepage`, `repository`, `license`, `keywords` | dropped | ``…is presentation-only and is not imported`` |
| `allowed-tools` (command frontmatter **and** SKILL.md) | **parsed, recorded, not enforced** — grants nothing | ``…is not enforced yet; the command runs under ordinary approval and no tool permission is granted`` / ``recorded as advisory metadata but is not enforced`` |
| SKILL.md `agent:`, `context:`, `hooks:` | `unsupported_fields` | ``Claude field `hooks` is not implemented by Muse Code and is treated as metadata`` |
| SKILL.md `model:`, `tools:`, `mcp-servers:` | `unknown_fields` (ignored) | — |
| Claude MCP entry with `env` or non-stdio transport | **rejected** | ``Claude MCP server `example` rejected: non-empty-env`` / `non-stdio-transport` |
| Claude hook event `Setup` | recognised, refused | ``…is recognized but is not run by Muse; complete any required plugin setup manually`` |
| Claude hook handler `once`, `shell`, `asyncRewake`, argv `command`+`args`, `commandWindows`-only | dropped, whole handler silently skipped | `D96/D98/D99/D101` |
| Claude hook handler `silent` | runs, display suppression ignored | ``foreign hook handler field `silent` is recognized`` |
| Codex `commands`, `hooks`, `dependencies`/`requiredPlugins`, `apps` | not imported | ``plugin manifest field `commands` is not used by this runtime`` |
| `$CLAUDE_PROJECT_DIR` idiom in a config-file hook | **empty string** — no equivalent exists; only `cwd`/`$PWD` | — |
| Unknown top-level keys in hook **stdout** | **hard failure**, not ignored (Claude Code's superset does not apply) | `unsupported \`continue\` in output of PreToolUse hook output` |

**Honoured, and worth knowing:** Claude's `disable-model-invocation` **is** respected
(`honored-declaration`: `true` → user-invocable only; `false` → model invocation retained).

### 3.3 The vocabulary problem — the real porting cost

Muse's tool names are its own. A trusted echo session emits 19 tools; the full default set across
configurations is:

```
workflow read_file search write_file edit_file read_memory add_memory edit_memory
web_search bash bash_input read_skill write_todos
subagent_spawn subagent_status subagent_send_message subagent_wait subagent_read_result subagent_cancel
(+ monitor, web_fetch, request_user_input, shell, artifact under gates/flags)
```

Notably: there is no `Task`, no `TodoWrite`, no `Read`/`Write`/`Edit` in Claude's casing, no
`Glob`/`Grep`. A ported Claude skill that says *"use the Task tool with subagent_type"* or
*"call TodoWrite"* is naming things that do not exist. Muse **does** accept a small foreign alias
table in the permission catalog (`search_files`→`search`, `WebSearch`→`web_search`,
`Write`/`write`→`write_file`, `exec_command`/`execute_command`→`bash_input`) but this is
compat plumbing, not tool resolution — `run.toolset` rejects every alias as an unknown tool name.

The ecosystem precedent for this exact problem is `obra/superpowers`, which ships a
`skillInstructions` prose translation table per harness ("when a skill says `TodoWrite`, use X").
**oh-my-musecode must ship a `muse` translation block.** Its content is entirely determinable from
the tool list above plus the `subagent_*` family's semantics (spawn/status/send_message/wait/
read_result/cancel, `subagent_type` selects a registered agent definition, per-child
`worktree_isolation`).

### 3.4 Practical conclusion for content reuse

1. `skills/` is the product; `.muse-plugin/`, `.claude-plugin/`, `.codex-plugin/` are **build
   artifacts generated by script** from it (superpowers ships nine such adapters; each manifest is
   20–40 lines).
2. Publish the marketplace repo with **both** `.claude-plugin/marketplace.json` and
   `.agents/plugins/marketplace.json` at the root. One repo, three agents.
3. Never rely on `allowed-tools` for safety on Muse. Use permission profiles or a
   `PermissionRequest` hook.
4. Keep Claude's trigger-phrase-stuffed descriptions **short** — on Muse they are pure catalog
   cost against a 32,000-byte budget, and Muse skills are explicitly invoked rather than
   description-matched.

---

## 4. What does not transfer from oh-my-X — the honest negatives

Muse Code is a compiled Rust binary. Five of oh-my-zsh's load-bearing mechanisms have **no analogue
whatsoever**, and pretending otherwise is how a framework ends up promising things it cannot ship.

**1. There is no late binding. Nothing can be monkey-patched.**
OMZ works because zsh functions are late-bound and `$fpath` is a search path a user can prepend to,
so a plugin can redefine `git_prompt_info` and win. Muse's tools, system prompt, compaction
strategies, reminder envelopes, slash-command table, status line and approval-judge policy are
compiled Rust. The approval judge's 15,092-byte policy is embedded, SHA-256 self-verified
(`03152dbb0bc78ca0d79ee3591ba55abe6153c3bde11a6e459e449aa681949eff`, independently recomputed and
matched) and says in its own text: *"There is no tenant, provider, project, local, or managed
policy overlay."* The 36 built-in slash commands are a closed, spec-owned vocabulary; plugin
commands and skill shortcuts fall into a literal `custom` bucket.

**2. There is no `$ZSH` to clone and no `custom/` overlay.**
OMZ's whole update model rests on `git pull --rebase` over a repo whose `custom/` subtree is
`.gitignore`d, so user data can never conflict. Muse installs itself from a signed release channel
with per-arch sha256 manifests and auto-updates hourly. A framework must never touch the binary or
its tree, and must record which binary version it was tested against — the plugin validator already
has an `incompatible_package` error code waiting for exactly that.

**3. Lazy loading is not a startup-time lever, because time is not the resource.**
There is no `autoload`, no `compinit`, no deferred sourcing to exploit. The genuine analogue is
**progressive disclosure of context**: a cheap catalog line vs. a body fetched by `read_skill`,
and `run.context_slimming.skill_catalog_descriptions: first_sentence`. The budget is bytes in one
HTTP body, and it is enforced by silent truncation rather than by latency.

**4. The prompt cannot be themed.** oh-my-posh's 123 themes and 118 compiled segment types have no
counterpart. Muse's status line is `provider · [effort ·] cwd [· YOLO]` with a width-aware path
shortener, rendered from compiled code. The only adjacent levers are: the theme colour palette,
`/name` (the session codename shown in `/status`), and `tui.verbose_output`. There is no
`extends`, no `palette`, no `segments`, no `cache`, no `version` field — `tui.theme` is one string.

**5. Plugins have no drop-in directory.** Skills, agent definitions, workflows, memory and rules
all have convention-scanned workspace directories. Plugins do not: five candidate locations were
tested in both trusted and untrusted workspaces and none is discovered. `muse plugins install` is
mandatory, which is why `trust: project-trusted` is a lockfile field rather than a discovery scope.

**Two more that transfer only in mutated form:**

- **Aliases.** OMZ's 200+ aliases are its most-regretted feature (its `CONTRIBUTING.md` now gates
  new ones behind a five-point test) and Muse has no alias system at all. The equivalent hazards
  are **always-on reminders** (which consume context every turn) and **skill-id collisions**
  (two same-named skills in one scope are *both* silently dropped from the model catalog while
  `skills list` shows both). Set policy before the bundle grows, not after.
- **Install-time hooks.** oh-my-fish runs `hooks/install.fish`; fisher and antidote do not. Muse
  deliberately has none — a plugin declares capabilities and executes nothing at install time.
  Given the binary holds `auth.json` and `trust.json`, **this is correct and oh-my-musecode should
  not campaign for it.** Declare, diff, then apply.

**What does transfer, cleanly:**

| oh-my-X mechanism | Muse analogue |
|---|---|
| `plugins=(...)` in `.zshrc` | `settings.skills.activation.*` map + `permissions.default_profile` + `presets` |
| oh-my-zsh's vendored 359-plugin bundle | a curated skills tree installed at first run, `enabledDefault` per item |
| `$ZSH_CUSTOM` override directory | `$CONFIG_DIR/skills/` + `$CONFIG_DIR/AGENTS.md` + `$CONFIG_DIR/themes/` |
| fisher's `fish_plugins` ⟷ receipt reconcile | `installed.json` + `skills/.muse/lock.json` `files[]` receipts (mechanism present; **no reconcile verb exists**) |
| antidote's `pin:` + `snapshot` | `content_sha256`, `package_sha256`, `resolved_revision` fields (**present and unused**) |
| prezto's error-on-conflicting-modules | `duplicate-capability-id`, `compatibilityName` cross-plugin ownership (present for plugins; **absent for filesystem skills**) |
| oh-my-posh's `themes/*.omp.json` | `$CONFIG_DIR/themes/*.tmTheme` (colours only; no segments) |
| sheldon's generated `plugins.lock` + staleness gate | content-addressed plugin cache + the resolved capability snapshot |

---

## 5. Gap analysis — where oh-my-X precedent exists and Muse has nothing native

**This is the product.** Each gap names the precedent, states exactly what Muse lacks, and gives
the fill. Ordered by value.

---

### G1. No cost attribution — and the catalog silently drops content

**Precedent.** oh-my-zsh's real ergonomic failure was never 400 ms; it was 400 ms *with no
attribution*: 21 `lib/*.zsh` files load unconditionally with no opt-out array, `compinit` is ~42%
of startup and scales with plugin count, and there is no `omz profile`, no per-plugin timing.
Users experienced "slow" with no path to a culprit, and the whole zinit/turbo-mode subculture
exists because of it.

**What Muse lacks.** Muse gives you `context_cost.startup_bytes` and `startup_estimated_tokens`
(= ⌈bytes/4⌉) **per skill** in `skills list --json`. Nothing aggregates it, and nothing measures
the other five sources at all: rules files, memory, always-on reminders, hook `additionalContext`,
and MCP tool schemas. And the failure mode is worse than slowness:

- **`skills_catalog` is capped at exactly 32,000 bytes.** Past it, entries lose their
  `<description>` in catalog order; past *that*, **whole skills vanish**. Measured: 600 project
  skills → 426 rendered, 189 gone, and `skills list --json` reports `diagnostics: []` with all
  600 present. The string `skill omitted to keep the startup catalog within its aggregate budget
  of <N> bytes` exists in the binary and **never fired**. PROVEN.
- `memory_snapshot` caps at 16,305 bytes and at **48 listed files per scope**, silently.
- Rules cap at 256,000 B/file and 65,536 B aggregate — these two *do* warn on stderr.
- `run.toolset` cannot remove `read_skill`, `write_todos` or the six `subagent_*` tools.
- MCP tool schemas are the `compinit` of agent startup: cost scales with server count, is
  invisible, and nobody attributes it correctly.

**Fill.** `omm doctor` / `omm cost` as the framework's flagship command. Everything needed is
already offline and unauthenticated:
`muse trace inspect --session-log <f> --render-mode verbose --format json` (dumps every context
block with byte counts, the toolset, and per-lane `bytes=`), plus
`$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-*.log` (`rules.context_load` with
`rendered_bytes`, `skills.catalog_load`, `plugin_capability_snapshot.compose`,
`named_workflow.catalog_load`, `gate.resolve` × 41), plus `skills list --json`
`context_cost`. Report bytes and estimated tokens per source, flag anything within 10% of a
budget, and **explicitly detect the silent catalog drop** by comparing the count in
`skills list --json` against the `<skill ` count in the rendered block. That last check is
something Muse itself does not do, cannot be derived from the docs, and turns an invisible
correctness bug into a one-line warning.

---

### G2. No desired-state reconcile, and no lockfile for anything but the managed store

**Precedent.** fisher's 251 lines contain the single most valuable behaviour in the family:
`fisher update` is a **three-way reconcile** between `fish_plugins` (desired) and
`$_fisher_plugins` (installed) — deleting a line uninstalls — backed by a per-plugin **file
receipt** (`_fisher_<plugin>_files`) that makes uninstall exact, and a **hard error on file
collision**. antidote's `pin:` requires a full 40-char SHA (a short SHA is an error) and
`antidote snapshot` auto-saves after every update into a restorable file in the same grammar as
the source. oh-my-fish's `bundle` is the counter-example: it records `package <name>` with no
version and is additive-only, so removals never uninstall — a wish list, not a lock.

**What Muse lacks.** Muse *has all the columns and none of the verbs.*
`$CONFIG_DIR/skills/.muse/lock.json` already carries `content_sha256`, per-file
`{relative_path, sha256, bytes}`, `trust`, `scan{status,warnings}`, and — unused — `repository`,
`requested_ref`, `resolved_revision`, `sparse_path`, `manifest_path`, `manifest_hash`,
`previous_content_sha256`, plus `uninstalled`/`removed_files`/`kept_files`.
`plugins/installed.json` carries `manifest_sha256`, `package_sha256`, `cache_path`.
But: `muse skills update <id>` only re-reads the source path already recorded; `muse plugins
update <id>` likewise; **neither has `--dry-run`**; and there is **no verb anywhere that takes a
desired set and makes it so**. Worse, skills dropped into `.agents/skills/`, `~/.claude/skills/`
or `~/.codex/skills/` — the zero-friction paths that make the whole thing pleasant — have **no
version, no hash, and no update path at all**.

That is precisely oh-my-zsh's worst practical failure reproduced: `zsh-autosuggestions` and
`zsh-syntax-highlighting`, the two most-installed plugins in the world, are frozen at clone time
on millions of machines and OMZ cannot know or tell them.

**Fill.** `omm.plugins.json` (declared) + `omm.lock.json` (resolved), same grammar, both
committable to a dotfiles repo. `omm update` performs the three-way reconcile with `--dry-run`
first, driven by the `files[]` receipts Muse already computes. Pin by resolved digest and
**reject abbreviated refs** (antidote's rule). Auto-snapshot after every successful update,
rolling and restorable — an agent framework mutates the model's own behaviour, so "put it back
how it was Tuesday" must be one command. Extend, don't duplicate, the sha256 scheme Muse already
computes per file.

---

### G3. There is no registry, and the reserved slot for one is empty

**Precedent.** OMZ never had a registry — the discovery layer is a hand-edited wiki page and
third-party install is `git clone … $ZSH_CUSTOM/plugins/<n>`. That single absence created the
entire antigen/zplug/zinit/znap/sheldon/antidote category, all of which *kept OMZ's file
convention and replaced its distribution*. It also routed every curation impulse into monorepo
PRs → 420 open PRs and a formal theme freeze. Conversely, oh-my-fish built a registry
(`packages-main`, 254 four-line files in a git repo) and after a decade it has fewer packages
than fisher, which has none — **a registry is a later index over identifiers that already work.**

**What Muse lacks.** Muse has the *machinery*: `marketplace add|list|update|remove`, git and
local sources, snapshots with per-plugin `integrity.digest`, an `availability.status` enum, a
`curated` provenance value, and a reserved marketplace name `tbh-curated`. All of it is **empty**.
No first-party registry endpoint exists in the binary. Meanwhile `quemsah/awesome-claude-plugins`
had indexed 15,134 Claude Code plugin repos by 2026-05-01, and Codex's marketplace hit 12 official
+ 40 community plugins within a month of launch. **No `oh-my-musecode`, no `awesome-muse-code`,
and no Muse marketplace of any kind exists.** PROVEN⁻ (ecosystem survey).

**Fill.** One git repo carrying **both** `.claude-plugin/marketplace.json` and
`.agents/plugins/marketplace.json` at its root, so `muse plugins marketplace add ohmy <git-url>`
and Claude Code's own `/plugin marketplace add` both work against the same repo. Do **not** make
the registry a precondition: identifiers must work before the index exists (fisher's lesson), so
`omm install <owner/repo>` and `omm install <path>` come first and the catalogue is an index over
them. Avoid the reserved names `loop`, `muse-core`, `tbh-reminders`, `tbh-curated`. Assume Meta
ships a first-party curated marketplace and pick a non-colliding namespace now.

---

### G4. Curation is custody, and it should be a badge

**Precedent.** OMZ's only quality signal is "merged into the monorepo," which produced a 420-PR
backlog, a formal *"We have enough themes for the time being"* freeze, and a `CONTRIBUTING.md`
that now gates **new aliases** behind a five-point justification test. Its one genuinely good
mechanism is `omz pr test <N>`: a distributed test channel built from a git fetch plus a
`testers needed` label, with an explicit warning that the code *"has not been reviewed by a
maintainer and may contain malicious code."*

**What Muse lacks.** Nothing — this is a gap in *how a framework should be built*, not in the
binary. Muse already has `provenance ∈ native-local | foreign-import | marketplace-user-added |
curated | local | marketplace` and `trust ∈ user-local | project-trusted`. `curated` is a distinct
value nobody uses.

**Fill.** Apply `curated` to **externally-hosted** packages: the marketplace entry carries the
badge and the digest; the code stays in the author's repo. That is the single biggest available
improvement over every oh-my-X, and it is the difference between a project that scales past one
maintainer and one that does not. Keep OMZ's preview channel: `omm try <pr-or-repo>` installing
into a throwaway `XDG_CONFIG_HOME`/`XDG_DATA_HOME` pair, with the same blunt warning.

---

### G5. No authoring lint that catches what actually breaks

**Precedent.** OMZ's authoring bar was "make a file" and its review bar was "a maintainer reads
it" — which does not scale and produced the compound-interest problem above. antidote and sheldon
push validation into a generated artifact; oh-my-posh pushes it into a 120-branch discriminated
JSON Schema with `unevaluatedProperties: false`, so the validator, the editor autocomplete and the
docs cannot drift.

**What Muse lacks.** `muse plugins validate --json` is excellent — 25 typed diagnostic codes,
`compatibility.summary ∈ full|partial|unsupported`, per-declaration `classification`, exit 0/1,
fully offline. But it does not catch the things that actually break a *skill pack*:

- `muse skills validate` returns **`valid: true, diagnostics: []`** for a skill with an unknown
  `experimental-gate:` — which is then **silently excluded at load time**. PROVEN.
- Nothing warns about the 32,000-byte catalog budget or duplicate skill ids across sources.
- Nothing checks that a ported Claude skill's tool vocabulary exists on Muse.
- A UTF-8 BOM and a duplicate YAML frontmatter key are both accepted with zero diagnostics
  (last-wins), so neither validator catches them.
- `muse config validate --plane defaults` catches semantic keymap errors that the TUI silently
  ignores — but only if the document carries `schema_version`, and nobody knows it exists.

**Fill.** `omm lint` as the authoring contract: run `muse skills validate` + `muse plugins
validate` + `muse config validate` and add the six checks above. "The validator passes with zero
diagnostics" is a bar that scales without maintainer attention — that is how you get OMZ's
authoring volume without OMZ's compound interest.

---

### G6. No profile switching across the surfaces that matter

**Precedent.** oh-my-posh has `palette`/`palettes` switchable at runtime from a template plus
`extends` for inheritance; sheldon has `profiles`; OMZ has `$ZSH_THEME` + the `plugins=()` array;
prezto has a real hierarchical config namespace (`zstyle ':prezto:module:git:status:ignore'`).

**What Muse lacks.** Muse has two disjoint profile mechanisms and neither is complete:
`settings.presets.<name>` covers `{provider, model, agent_profile, run}`; permission profiles
cover `{approval, reviewer, filesystem, network}` and **`--permission-profile` refuses to coexist
with `--yolo`, `--disable-sandbox`, `--disable-approval`, `--approval-mode`, `--sandbox-network`
or `--approval-judge`**. Neither touches skills activation, rules, hooks, memory or theme. And
approval/reviewer are hard-coupled — only 2 of 9 combinations are legal
(`{on_request|prompt_unmatched} × {human|auto_review}` and `allow_all × none`), so a naive
"fast profile with reviewer:none" is rejected.

Worse, **passing a documented default explicitly changes the posture**: `--sandbox-network
proxy-only` or `--approval-mode on-request` (both the stated defaults) move the session off the
built-in `:ask-me` profile onto `launch_overrides` and silently flip reviewer `human` →
`auto_review`, enabling the LLM approval judge. Any wrapper that "helpfully" passes defaults
alters the security model. PROVEN.

**Fill.** `omm profile use <name>` writing one coherent slice: a permission profile (composed via
`extends` from a built-in), a preset, a skills-activation map, a `$CONFIG_DIR/AGENTS.md` variant,
a theme, and a gate env bundle for the launcher. Ship `strict` / `default` / `fast` / `ci`. Never
pass a flag whose value equals the documented default.

---

### G7. No namespace or collision policy

**Precedent.** OMZ's aliases silently shadow real binaries; prezto errors by default on
conflicting module locations and unwinds transactionally; fisher hard-errors on file collision
rather than overwriting.

**What Muse lacks.** Plugin capabilities are namespaced and collision-checked
(`duplicate-capability-id`, cross-plugin `compatibilityName` ownership rejected at install).
Filesystem skills are not:

- Two skills with the same frontmatter `name` in the same scope are **both dropped** from the
  model catalog (`duplicate-active-skill-id`) while `muse skills list` shows both and `inspect`
  silently picks the first.
- Bundled skills get a bare alias *and* `bundled:<id>`; plugin skills get only
  `plugin:<p>:<s>`; project/user skills get only the bare name — so a user skill named `plan`
  captures the bare selector from `bundled:plan`.
- Slash commands: 36 built-ins are a closed vocabulary; everything else is `custom`.

**Fill.** `omm lint` enforces a prefix (`omm-`) on every shipped skill and command id, and checks
for collisions against the 15 bundled skill names, the 36 built-in slash commands, and whatever is
already installed. Publish the source-precedence table as documented policy rather than letting
users discover it by having a skill vanish.

---

### G8. No trust bootstrap

**Precedent.** Not an oh-my-X gap — no shell framework needs it. It is a Muse gap that will
dominate first-run support load.

**What Muse lacks.** Everything project-scoped is inert until `$CONFIG_DIR/trust.json` says so;
`--trust-workspace` explicitly does not persist; **no CLI writes the file**; only the TUI's
first-run prompt does. A user who installs a repo-scoped framework via a headless command and
never opens the TUI sees literally nothing happen, with one stderr warning about `AGENTS.md` and
silence about skills, hooks and workflows.

**Fill.** `omm trust [path]` writes the entry (it is three keys), `omm doctor` reports trust state
prominently, and `omm init` writes it as part of scaffolding. Document loudly that
`muse workflows` has **no** trust flag at all — `trust.json` is the only route.

---

### G9. No settings merge tool, and the file eats unknown keys

**Precedent.** Every oh-my-X edits the user's config file. OMZ's `lib/cli.zsh` (944 lines, 4× the
kernel) awk-rewrites `~/.zshrc`, validates with `zsh -n`, and rolls back on parse error — and OMZ
also leaves a named restore file for every mutation (`.zshrc.pre-oh-my-zsh`,
`.zshrc.omz-uninstalled-<ts>`). A 41-line uninstaller that works is what earns permission for a
603-line installer that runs `chsh`.

**What Muse lacks.** There is one settings file, it is typed, `schema_version` is hard-required, a
malformed value is a **hard startup failure for every subcommand**, and the binary **rewrites the
file from its typed struct during ordinary sessions, destroying unknown keys**. There is no merge
helper, no backup convention, and no rollback.

**Fill.** `omm settings merge` doing a typed deep-merge, validating with
`muse config validate --plane defaults` *before* writing, writing atomically, and leaving
`settings.json.pre-omm` + a timestamped uninstall backup. Never stash framework state in
`settings.json` — keep it in `~/.config/omm/`. Ship the uninstaller first.

---

### G10. Skills are explicitly invoked, and there is no user-extensible router

**Precedent.** Claude Code's entire skill-authoring doctrine is third-person descriptions stuffed
with trigger phrases so the model auto-selects. Muse loads a skill "only for the turn you invoke
it on."

**What Muse lacks.** Muse's answer is `MUSE_EXPERIMENTAL_SKILL_REMINDER` — a first-party reminder
sub-agent with a `skill_catalog` feed (128,000 B) and a `skill_read_ledger` so it does not
re-nag. It is not user-extensible. The one programmable routing surface,
`hookSpecificOutput.selectedSkills` with `outputCapabilities: ["skills.v1"]`, is
**double-gated and does not work in the CLI lane**: with both
`MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS` and `..._APPLY` set and the plugin approved, a real run
rejects the payload with `selected_skills:rejected:capability-not-negotiated` and emits no
`selected_skills_catalog` block. It is also accepted only on `UserPromptSubmit`/`PostToolUse`
(rejected on 9 other events). PROVEN.

**Fill.** Two moves. (a) Ship a small, memorable **command** surface (`/omm-plan`, `/omm-ship`)
via plugin `capabilities.commands[]` plus one router skill — this is the reliable path today.
(b) Ship your **own** reminder agent in the plugin bundle using `capabilities.reminders[]`; the
DSL is fully declarative, executes no code, and Meta's own six are the reference implementation
(the complete 72 KB `tbh-reminders` manifest was recovered verbatim). Treat `skills.v1` as a
roadmap item, not a dependency.

---

### G11. No cross-session ledger

**Precedent.** None in oh-my-X — this is a differentiator, not a gap to copy.

**What Muse lacks.** The raw material is unusually good and completely unassembled: an
append-only durable event log per session, a global `session-index.db`, `muse export
--redacted` with a stable `export_schema_version: 1`, `muse trace inspect --format json`, and a
**lease-free MSP read plane** (`session/list` + `session/read` + `view/page` take no writer lease
— verified with two live hosts while the first held the lease and kept accepting commands).
Nothing aggregates any of it. There is no cost dashboard, no transcript search, no "what did I do
last week."

**Fill.** `omm cost` / `omm log` reading `session-index.db` directly (33 columns incl. ten
`msp_*`), which is materially cheaper than spawning a second `muse serve`. Note the constraints:
MSP admission capacity is **4** concurrent commands (`-32031` with `data.capacity: 4`), the
inbound frame limit is **10 MiB** (`-32002`), and `muse serve` has **no way to select the echo
provider**, so end-to-end MSP tests cost real tokens.

---

### G12. No binary pin, and the update channel is invisible

**Precedent.** OMZ's updater is the best code in its repo and its **refusal list** is the part to
copy: it declines when `$ZSH` is not writable *or not owned by you*, when stdout is not a tty,
when git is absent, when `$ZSH` is not a git repo. Plus a 13-day (prime) cadence, an atomic
`mkdir` lock with a 24 h stale reaper, an exit-status-preserving trap, a 2-second SHA probe
instead of a fetch, and `has_typed_input()` which **downgrades the blocking prompt to a one-line
reminder if you have already started typing**.

**What Muse lacks.** Muse auto-updates hourly and silently; `MUSE_NO_AUTO_UPDATE` is
launcher-only; `muse-canary` is already a full minor ahead at `1.1.0-R2009.1`; and the docs
changelog stops at 0.2.1. A framework built on undocumented internals will break without warning
and without a version to blame.

**Fill.** The `omm` launcher sets `MUSE_NO_AUTO_UPDATE=1` for its own invocations, records the
binary version it was validated against, and version-detects at startup. Copy OMZ's refusal list
verbatim for `omm update`: never mutate a store you do not own (Homebrew/Nix/enterprise-managed),
never prompt in a non-interactive or piped session, never prompt mid-turn, hold a lock so ten
concurrent sessions produce one update, keep staleness in a cheap parseable file. And **refuse to
touch the Muse binary at all** — content only.

---

### G13. Fragile single points of failure worth guarding

Three one-line footguns that will produce "everything broke" reports and that a framework should
detect in `omm doctor`:

1. **`MUSE_EXPERIMENTAL_SDK_ENABLED` is default-ON and gates BOTH `serve` and `schema`.** Setting
   it to `0`, `false`, `yes` or `""` removes both from `muse --help` and disables the entire MSP
   integration *and* its build-time type generation. An enterprise `settings.feature_config` path
   to the same switch plausibly exists (INFERRED).
2. **`MUSE_ENABLE_WEB_TOOLS` / `MUSE_WEB_SEARCH_MODE` typos brick every subcommand**, including
   `muse --version`.
3. **`mcpServers` + `mcp_servers` both present silently voids the entire MCP config** — no
   server, no error, no diagnostic.

Also worth surfacing: `runtime_capabilities` in `settings.json` is **never garbage-collected**
(a retired hook's trust entry persists forever), and remote server-pushed gates
(`ARTIFACT_TOOL_ENABLED`, `LOCAL_SESSION_MESSAGING_ENABLED`) can flip under a user —
`TBH_DISABLE_FEATURE_CONFIG` pins them for reproducibility.

---

## 6. Recommended shape of oh-my-musecode

> **oh-my-musecode is a curated content bundle, a thin CLI, and a git marketplace.
> It is not a loader, and it must never touch the binary.**

### 6.1 The one-sentence positioning

oh-my-zsh won on five things — zero-friction authoring, a vendored curated bundle that made the
default config already good, a three-line user config, a git-ignored override directory, and an
updater that respected the user — and lost on four: no manifest, no registry, no laziness, and an
unmanaged global namespace. **Muse's runtime already fixes all four losses.** oh-my-musecode's job
is to reproduce the five wins on top of it and add the two things no oh-my-X ever built:
**per-source cost attribution** and **curation as a badge on a pointer rather than custody of the
code**.

### 6.2 Repository shape — one source tree, N generated manifests

```
oh-my-musecode/
├── skills/                          ← THE PRODUCT. Plain SKILL.md dirs, harness-neutral.
│   ├── omm-plan/SKILL.md
│   ├── omm-review/SKILL.md
│   └── …
├── commands/                        ← slash commands (Claude-shaped .md + front matter)
├── hooks/                           ← hook scripts (POSIX sh, absolute paths, no inherited env)
├── reminders/                       ← declarative reminder-agent duty files
├── agents/                          ← agent definitions (native family only)
├── workflows/                       ← named .js orchestrations
├── themes/                          ← *.tmTheme
├── profiles/                        ← settings slices: strict / default / fast / ci
├── rules/AGENTS.md.tmpl             ← the personal-rules template
├── translation/muse.md              ← THE TOOL-VOCABULARY BLOCK (see §3.3)
│
├── .muse-plugin/plugin.json         ← GENERATED
├── .claude-plugin/plugin.json       ← GENERATED
├── .codex-plugin/plugin.json        ← GENERATED
├── .claude-plugin/marketplace.json  ← GENERATED (marketplace repo root)
├── .agents/plugins/marketplace.json ← GENERATED (marketplace repo root)
├── scripts/sync-manifests.sh        ← the thing that makes the above tractable
└── omm/                             ← the CLI (Rust or TS; must ship a single binary/npx entry)
```

`obra/superpowers` proves this works: 14 skills, **nine** harness adapters, kept in sync by
`sync-to-codex-plugin.sh` + `bump-version.sh`. Each manifest is 20–40 lines. Generate them; never
hand-maintain them.

### 6.3 Three install tiers

**Tier 0 — default, works today, no experimental gate, no network at runtime.**
This must be the path in the README, because it is the only one that cannot be turned off by a
flag Meta owns.

```
omm install
  ├─ muse skills install <each curated skill> --scope user --json   → $CONFIG_DIR/skills/ + lock.json
  ├─ write $CONFIG_DIR/AGENTS.md                                     (backing up any existing file)
  ├─ omm settings merge profiles/default.json                        (validated first)
  ├─ copy themes/*.tmTheme → $CONFIG_DIR/themes/
  ├─ write ~/.config/omm/omm.lock.json
  └─ print the token budget it just consumed
```

Every artifact is simultaneously valid for Claude Code and Codex: the same `SKILL.md` files also
resolve from `~/.claude/skills` and `~/.codex/skills` if the user prefers, and the same
`AGENTS.md` is read by Codex.

**Tier 1 — the plugin bundle, gated, richer.**
`MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins marketplace add ohmy <git-url>` then
`muse plugins install oh-my-musecode@ohmy`. Adds hooks, MCP servers, reminder agents, slash
commands and agent definitions in one approvable artifact, with digest-pinned trust. `omm install
--full` drives this and then walks the user through `muse plugins approve` per capability,
**showing what each one does before approving it**.

**Tier 2 — the repo.**
`omm init` scaffolds `AGENTS.md` (wrapping `muse init`), `.agents/skills/`, `.muse/hooks.json`,
`.agents/workflows/`, `.agents/memory/`, and — critically — writes the `trust.json` entry.

### 6.4 Command surface

| Command | Does |
|---|---|
| `omm install [--full]` | Tier 0, or Tier 0 + Tier 1. Idempotent. Prints the token delta. |
| `omm update [--dry-run]` | **Three-way reconcile** `omm.plugins.json` ⟷ `omm.lock.json` ⟷ disk. Auto-snapshots first. |
| `omm remove <id>` / `omm uninstall` | Exact removal via `files[]` receipts. Restores `settings.json.pre-omm`. **Ship this first.** |
| `omm doctor` | Trust state, gate matrix, budget report, catalog-drop detection, orphaned `runtime_capabilities`, plugin cache tamper (`plugin-cache-invalid`), binary version vs. tested version, `mcpServers`/`mcp_servers` collision, malformed `settings.json`. |
| `omm cost` | Bytes + estimated tokens per context source, per skill, per reminder, per MCP server. |
| `omm profile use <name>` | Writes a coherent settings slice + permission profile + activation map + rules variant + theme + gate env. |
| `omm trust [path]` | Writes `$CONFIG_DIR/trust.json`. |
| `omm lint [path]` | `skills validate` + `plugins validate` + `config validate` + the six checks Muse misses (§G5). |
| `omm theme <name>` | Copies a `.tmTheme` and sets `tui.theme = "custom:<stem>"` + `tui.terminal_background`. |
| `omm keymap <preset>` | Writes `tui.keymap`, validated offline first. |
| `omm try <repo\|pr>` | Installs into a throwaway `XDG_CONFIG_HOME`/`XDG_DATA_HOME` pair. Prints the unreviewed-code warning verbatim. |
| `omm run …` | The launcher shim: sets `MUSE_NO_AUTO_UPDATE=1`, the profile's gate bundle, and `--permission-profile`, then execs `muse`. |

### 6.5 Ten design rules, each traceable to a specific finding

1. **Rich package metadata, poor user config.** OMZ's whole API was three lines. Keep the user's
   `settings.json` footprint to an activation map + a profile name; push richness into the plugin
   manifest and into `~/.config/omm/`. The moment users hand-edit capability blocks you lose the
   copy-paste-a-dotfile channel that made OMZ spread.
2. **Never write framework state into `settings.json`.** The binary rewrites it and destroys
   unknown keys. (§1.7, PROVEN with a destroyed `_omm_marker`.)
3. **Budget every shipped item.** Catalog line cheap, body on demand. Set
   `run.context_slimming.skill_catalog_descriptions: "first_sentence"` and allowlist only the few
   skills whose full trigger prose matters. Refuse to ship a bundle whose catalog exceeds ~24,000
   bytes (75% of the hard cap).
4. **Prefix everything.** `omm-` on every skill id, command id and plugin capability id. Lint for
   collisions against the 15 bundled skills, the 36 built-in slash commands, and the reserved ids
   `loop` / `muse-core` / `tbh-reminders` / `tbh-curated`.
5. **Reminders are opt-in, budgeted and attributed.** They cost context every turn. Ship at most
   one enabled by default; make `omm cost` name it.
6. **Declare, never execute, at install time.** Reject oh-my-fish's `hooks/install.fish` model
   outright — the binary holds `auth.json` and `trust.json`. Show a diff, then apply.
7. **Pin by digest; reject abbreviated refs.** Extend Muse's existing per-file sha256 rather than
   inventing a second scheme.
8. **Auto-snapshot after every update**, rolling and restorable, and append to a JSONL audit log
   in the same shape as `skills/.muse/audit.log`.
9. **Hooks get a scrubbed environment.** No `$GITHUB_TOKEN`, no `$PATH` assumptions, no
   `$CLAUDE_PROJECT_DIR`, and the `command` string is interpreted by **`$SHELL -c`** — whatever
   the user's login shell happens to be. Every hook script must resolve absolute paths and carry
   its own credential source. Plan the credential story before the hook story.
10. **Ship the uninstaller before the installer.** OMZ's 41-line uninstaller and its
    `.pre-oh-my-zsh` restore files are what earned it permission to run `chsh` from a
    `curl | sh`.

### 6.6 What to build first (a 4-milestone sequence)

- **M0 — `omm doctor` + `omm cost`, standalone.** Zero install footprint, immediate value, and it
  makes every subsequent decision measurable. It also single-handedly surfaces the silent
  catalog-drop bug, which nothing else in the ecosystem can do.
- **M1 — Tier 0 install + `omm lint` + `omm uninstall` + 8–12 skills.** Ship the smallest curated
  set that is obviously good, plus the tool-vocabulary translation block. Cross-publish to
  `~/.claude/skills` so it lands for Claude Code users too.
- **M2 — `omm.plugins.json` + `omm.lock.json` + reconcile + snapshot.** The thing every oh-my-X
  except fisher and antidote got wrong.
- **M3 — marketplace repo + `.muse-plugin` bundle + hooks + one reminder agent + themes/keymaps.**
  Only now does the experimental gate enter the story.

### 6.7 Timing

Being early in Claude Code is impossible — 15,134 indexed plugin repos. Being early in Muse is
free: no `oh-my-musecode`, no `awesome-muse-code`, no marketplace of any kind exists, the binary's
own curated-registry slot is reserved and empty, and `muse-canary` is already a minor version
ahead. That window is measured in quarters, not years.

---

## 7. Unknowns that must be settled before building

Ordered by how much they change the design.

| # | Unknown | Why it matters | How to settle it |
|---|---|---|---|
| 1 | **Do *plugin-provided* MCP servers ever start?** A settings-declared stdio server **does** launch under `muse exec`. An installed **and approved plugin** MCP server produced zero `mcp.*` records under both `exec` and `serve`, while the capability snapshot *was* composed (`mcp_servers=1`). TUI-only is INFERRED from `hooks/MCP still need restart`. | If plugin MCP is TUI-only, a bundle cannot ship tools headlessly and the framework must fall back to writing `settings.json → mcpServers` — a very different install story. | Run a real TUI session under a pty with an installed+approved stdio MCP server that logs its own invocation; check `session.jsonl` for `mcp.*` and the tool list for `mcp__*`-prefixed names. Compare against the settings-declared path in the same session. |
| 2 | **The `native-plugin-contract.md` ground truth.** Meta ships the authoritative authoring contract *inside* the binary as `create-plugin/references/{native-plugin-contract.md, capability-examples.json}` (23.6 KB of worked examples). Every manifest field name here is validator-derived. | It will disagree with third-party derivation somewhere, and it defines the id grammar, validation order, and correction-round limits Meta's own generator targets. | It has already been extracted to `re/artifacts/create-plugin/`. **Read it before writing a manifest generator.** |
| 3 | **Does `skills.v1` hook skill-routing ever work?** With both gates on and the plugin approved, a real run rejects it: `selected_skills:rejected:capability-not-negotiated`. Rejection enum: `ignored_off, capability-not-negotiated, provenance-unavailable, invalid-adapter-report, invalid-adapter-prepared-state, adapter-failed, adapter-unavailable`. | It is the only programmable skill router. If dead, G10's fill is commands + a reminder agent, permanently. | Try in the TUI (the CLI lane may simply not negotiate the capability); try `--preset native-basic`; watch for a `selected_skills_catalog` block at order 201. |
| 4 | **The Agent Plugins 1.0.0 `$schema` literal.** The fourth manifest family exists and cannot be exercised — 24 guessed markers all returned `declares unknown root format`, and no `$schema`-shaped literal exists in the binary's printable strings. | If a fourth cross-vendor standard is coming, the manifest generator should target it. | Watch the Codex/Claude Code release notes and `agents.md`; or find a real Agent-Plugins package in the wild and feed it to `muse plugins validate`. |
| 5 | **`settings.plugins`** — a real `SettingsFileDocument` member whose value shape is unknown; four candidate shapes produced neither an error nor any behaviour change. | Could be the declarative "which plugins are enabled" array a framework would otherwise have to synthesise. | Probe with a duplicate-key oracle (`{"plugins":X,"plugins":X}` → `duplicate field` proves it is a derived field) and with `muse config validate --plane defaults` for member names. |
| 6 | **Enterprise `system_file` on-disk path.** `enterprise-defaults.json` / `enterprise-policy.json` are the filenames; the directory is composed at runtime and never appears as a single literal. `/Library/Application Support/Muse/` is a weak inference — planting files at seven candidate paths all left `state=absent`. | Determines whether a team edition can ship policy at all. | Needs a sanctioned managed-preferences fixture (domain `com.tbh.tbh`, keys `enterprise_defaults_json` / `enterprise_policy_json`) or a machine where an admin can write under `/Library`. |
| 7 | **Which two of the six agent-definition composition slots survive `safe_mode`?** `sources=6` is constant; `loaded_sources=2` under safe mode; the `DefinitionSource` enum has 7 names across three inconsistent string runs. | Determines whether a framework's agent pack survives an enterprise safe-mode fleet. | Diff `agent_definition.sources_load` with one source populated at a time under `safe_mode: true`. |
| 8 | **The `personal_project` memory scope root.** It is the *default* scope for `add_memory`, created lazily on first write; ~73 candidate paths produced no third `## Memory scope:` section. The renderer may simply never emit that section, in which case path-scanning is uninformative. | A framework that writes memory needs to know where it lands. | One authenticated `add_memory` call with `scope: "personal_project"`, then `find`. State both hypotheses until then. |
| 9 | **Does an unknown top-level CLI token exit 0 or 1?** The first pass recorded both; the verifier settled parse-failure=2 and runtime-failure=1 but not this case. | A wrapper's error detection depends on it. | `muse zzznotacommand; echo $?` — 30 seconds, and it should be in `omm doctor`'s self-test. |
| 10 | **Windows.** Binaries ship, `muse sandbox windows setup` exists, hook entries have `commandWindows`, and every doc denies it. | Decides whether the framework's hook scripts need a Windows path at all. | Run the shipped `.exe` on Windows; check `muse sandbox windows check`. |
| 11 | **`muse-canary` 1.1.0 delta.** A full minor ahead, selectable with `MUSE_CHANNEL_URL`. | Anything built on undocumented internals may already be stale. | Fetch the canary build into a throwaway prefix and re-run the extension-point table as a diff. This should be a recurring CI job, not a one-off. |
| 12 | **Numeric caps not yet measured:** `plugin_scope_quota` / `plugin_preflight_overflow` / `plugin_class_overflow` (per-scope and per-class limits on how many plugin capabilities enter one session snapshot). | Determines the maximum size of a shipped bundle. | Binary-search the capability count in a synthetic plugin until the snapshot reports one of those codes. |

---

## 8. Appendix A — the budget table (everything that silently truncates)

The single most useful artefact for a framework author. All PROVEN.

| Surface | Hard limit | Failure mode | Warned? |
|---|---|---|---|
| `skills_catalog` context block | **32,000 B** | descriptions drop first, then **whole skills vanish** | **NO — zero diagnostics anywhere** |
| `skills_catalog` per-scope file listing | — | — | — |
| `memory_snapshot` context block | **16,305 B** | `[MEMORY.md truncated]` + `[memory snapshot truncated]` | in-band markers only |
| `memory_snapshot` "Other Markdown files" | **48 entries per scope** | silently listed to 48 | **NO** |
| Rules file, per file | **256,000 B** | file skipped entirely | **YES**, stderr, names the size |
| Rules, aggregate | **65,536 B** | truncated with `[Muse Code truncated rules-file context …]` | **YES**, stderr |
| `SKILL.md` loader | **262,144 B** | skill excluded | `skill-file-too-large` diagnostic |
| Hook stdout | **16,384 B inclusive** | process tree killed, stream discarded | `output_too_large` in the terminal record |
| Hook timeout | **no default** — unbounded; `timeout: 0` clamps to 1 s | hangs the turn | — |
| Plugin manifest | **131,072 B** | install rejected | `plugin manifest exceeds 131072 byte limit` |
| Plugin package entries | **4,096** files+dirs | install rejected | (Agent-Definition inventory error masks the package walker's own message) |
| Plugin package depth | **16** levels | install rejected | same |
| Agent-definition candidate | **262,144 B** | entry dropped | `candidate_too_large` |
| Agent-definition entries per source | **1,024** | whole source fails | `source_failed=1` |
| Agent-definition field | ~**16,384 B** | entry dropped | diagnostic |
| MSP inbound frame | **10,485,760 B** | `-32002 inputTooLarge` with `limitBytes` | yes, on the wire |
| MSP concurrent commands | **4** | `-32031 backpressured`, `data.capacity: 4` | yes, on the wire |
| Permission profiles | **64** total, id ≤64 chars, description **≤240**, `extends` ≤16 deep | rejected | yes |
| `agents.execution_capacity` | **1–64** | rejected | yes |
| Workflow: concurrent children **16**, lifetime calls **1,000**, inline schemas 4 KiB / depth 16 / 16 entries | | hard run failure past 1,000 | documented in the tool description |

Observed prompt composition on a stock trusted session: `instructions` 36,613 B; `tools` array
40,284 B; `workflow_cookbook` 18,056 B; `skills_catalog` 9,440 B; `memory_snapshot` up to
16,305 B. **`run.workflow_trigger_mode: "off"` is the cheapest single saving available (~21 KB).**

## 9. Appendix B — the five silent-failure modes to detect in `omm doctor`

1. Skills past the 32,000-byte catalog budget are **dropped with no diagnostic** — detect by
   comparing `skills list --json` count against the `<skill ` count in the rendered block.
2. Two skills sharing a frontmatter `name` in one scope are **both dropped** from the model
   catalog while `skills list` shows both.
3. An unknown `experimental-gate:` excludes a skill at load time while `muse skills validate`
   reports `valid: true, diagnostics: []`.
4. `mcpServers` **and** `mcp_servers` both present voids the entire MCP config — no server, no
   error, no diagnostic.
5. A semantically invalid `tui.keymap` entry (unknown action, bad key spec, duplicate binding,
   route-char violation) is silently ignored by the TUI; only
   `muse config validate --plane defaults` catches it.

Plus two non-silent but easily missed: a matcher copied onto an event that has no selector
(`UserPromptSubmit`, `Stop`, `PreLLMCall`, `PostLLMCall` — only `*` or absent fires) disables the
whole hook group; and rebinding `tui.keymap.app.commands` away from `/` drops the 7 bundled skill
shortcuts and `/loop` from the picker (39 rows → 31).

---

## 10. Appendix C — dead claims and resolved contradictions

Recorded so nobody re-derives them. Each was asserted by a first-pass report and **refuted** by
its verifier, or was contradicted across reports and is resolved here.

### 10.1 Refuted — do not build on these

| Dead claim | Reality |
|---|---|
| "No built-in permission profile ships." | **Four ship**: `:read-only`, `:ask-me` (the default for every session), `:auto-review`, `:unrestricted`. The leading `:` is exactly why user ids exclude it; the earlier pass only guessed bare names. |
| "`description` is optional in a native plugin manifest." | **Required and non-empty**, and the check fires before `compat` and `capabilities`. Every plugin generated from the old schema table is invalid. |
| "`compat.source` is required." | Only `compat.manifestDir` is validated; `source` is a documentation convention. |
| "`muse workflows` reads `.claude/` and `.codex/` — so an existing Claude repo layout works unchanged." | It reads all three roots, **but only when the workspace is trusted**, and `muse workflows` has no trust flag — `trust.json` is the only route. `save --scope project` writes only to `.agents/workflows/`. |
| "`muse skills install` refuses a symlinked source." | It **succeeds** for both absolute and relative symlinks. The quoted error is what a *dangling* symlink produces. |
| "The `create-plugin` skill is gated by an `experimental-gate:` frontmatter key." | Its frontmatter has only `name` and `description`; no `experimental-gate` exists anywhere in the bundled package. The gating is hard-coded. |
| "`MUSE_EXPERIMENTAL_SDK_ENABLED` has no observable effect." | It is **default-ON** and gates both `muse serve` and `muse schema`. The original diff method (unset vs `=1`) is structurally blind to a default-on gate. |
| "goal/*, workflow/*, task/* exist as runtime commands with no MSP door." | All nine are **live, routed MSP methods**. The original probe used made-up names (`workflow/list`, `session/setGoal`). Real surface: 40 methods. |
| "`turn/start.providerRequestOptions` is silently accepted off-schema." | It is an `experimentalRequired`-gated field, rejected without the opt-in — and *functional* with it. Genuinely unknown members are the ones silently ignored. |
| "`.muse/hooks.json` is not auto-discovered." | It **is** — the original poison test failed because workspace hooks are trust-gated and a malformed hook file emits no diagnostic in headless `exec`. |
| "Hook event keys are snake_case." | **PascalCase only** in a hooks document. snake_case appears only in the run-terminal record. |
| "`tui.voice_tap_enabled` is a user settings key." | It exists only in the *enterprise* key registry. `TuiSettings` has 14 fields, not 15. |
| "`schema_version` in `settings.json` is mandatory-by-convention." | Hard-required by serde, typed `u32`; 0 and 2 are both rejected. |
| "Personal Claude/Codex rules are *imported*, not read." | Read **in place** at session open as `scope="user"` with `written-for=` provenance. |
| "A framework may stash a namespaced marker key in `settings.json` because unknown keys are ignored." | Actively harmful: unknown keys are ignored **on read** and **destroyed on write**. |
| "`muse config` is gated by `MUSE_EXPERIMENTAL_ENTERPRISE_CONFIG`." | Not gated. `config` is in a bare `muse --help`, and `field_not_activated` is a compiled-in build property no env var flips. |
| "`skills.activation` keys are skill ids." | They are SKILL.md **paths**, `bundled://` and `plugin://` URIs, with project keys nested under the absolute workspace root. |
| "`plugins hook test` requires an approved capability." | Only that the **plugin** is enabled. It runs `review_needed` and even explicitly `reject`ed hooks — which makes it a *better* CI harness than claimed. |
| "Settings-driven `user-invocable-only` behaves differently from frontmatter `disable-model-invocation`." | Identical: both keep the skill in the catalog with `model-invocable="false"`. The apparent asymmetry was a confound (`bundled:git` ships `user-invocable: false`). |
| "`--approval-mode never` differs functionally from `--disable-approval`." | Both commit a **byte-identical** resolved permission snapshot. Only the `security_mode` label differs (`normal` vs `approval_disabled`). |
| "The provider `openai`/`anthropic`/`openrouter` can be selected." | Compiled out behind Cargo features; a settings-level request silently falls back to `meta` with a warning. Only `echo` and `meta` exist. |
| "`--internal-claude-channels-sidecar-v1` is not accepted by any parser." | It is accepted by the root parser and short-circuits before clap: a live JSON-RPC/MCP server on stdio. |
| "The `native-basic` profile is a fixed 25-tool set including the subagent tools." | The literal run holds 26 names, and a contiguous string run cannot prove membership either way — literal pooling makes it unsound. `--preset native-basic` emits the same 13/19 tools as the default. |
| "`provider_retry` default is 5 attempts, bounding a run." | Default is **10**, and the bound is **per model step** — a single `exec` re-enters the step after each exhausted ladder, so the POST count is unbounded by `max_attempts`. |
| "V8 is disabled by default in the shipped build." | `Default V8: disabled` echoes the *selected entry's* `default_build.v8_dependency`; only the one checked-in QA fixture says `disabled`. Every saved workflow reports `enabled`, and the process-identity trace reports `workflow_engine="v8"`. |

### 10.2 Cross-report contradictions, resolved

**Workflow project roots.** `cli-surface` refuted the "reads `.claude/` and `.codex/`" claim on the
grounds that `workflows list` never surfaced project entries. `workflows-tools` then recovered the
`trust.json` schema and proved all three roots list correctly from a trusted workspace
(`.agents` shadows `.codex` for a duplicate name; `untrusted=3` is the diagnostic otherwise).
**Resolution:** all three roots are read; trust is the gate; there is no flag; `save --scope
project` still writes only `.agents/workflows/`.

**Trust persistence.** `sessions-memory-rules` reported "no CLI path writes `trust.json`; it never
materialised." `plugins` and `workflows-tools` both hand-wrote it and it worked.
`tui-slash-theme` observed the TUI prompt writing it. **Resolution:** the TUI writes it, no CLI
writes it, hand-writing works — which is precisely why `omm trust` is worth shipping (G8).

**Hook discovery.** `config-paths` §6.5 said `.muse/hooks.json` is not discovered; its own
verifier refuted that, and `hooks` proved it from the start with a trusted workspace.
**Resolution:** discovered, trust-gated, and a malformed hook file emits no diagnostic in
headless `exec` — which is what made the original negative look real.

**Hook event count.** The ecosystem survey reported "Muse ~4 confirmed" from third-party sources.
The RE pass proved **17** from the enum literal, the plugin validator accepting all 17
PascalCase names, and 6 firing in an echo session. **Resolution: 17.** (Claude Code's 33 and
Codex's 11 are third-party figures this synthesis did not verify.)

**`definition_hash` composition.** The proposed model (source digest excludes the capability's own
declaration file) was refuted. **Resolution:** every runtime capability's `source_digest` is the
plugin's `package_sha256`, **except** a foreign-family *hook*, which gets a distinct digest shared
by every hook in the package, invariant under edits to the manifest and to the hook source.
Practical consequence: for a **native** plugin, any byte change to the package flips every
approved capability to `modified` and re-arms review. An updater must re-approve, and should diff
and explain what changed.

**Slash-command count.** `cli-surface` recorded 36 as "a floor, not the count," reasoning that
the telemetry vocabulary is `BUILTIN_SLASH_COMMAND_NAMES` *minus* `TELEMETRY_DROPPED_…` and that
`/login` and `/name` were therefore dropped. `tui-slash-theme` refuted the inference by walking the
actual `&[&str]` pointer array in `__DATA_CONST`, which contains 37 entries **including** both
`/login` and `/name` — the apparent absence was literal-pool deduplication, not array membership.
**Resolution:** the TUI table holds **36 canonical commands + 9 aliases**; the telemetry array holds
37 because it carries the alias `/quit` as its own entry. Nothing is demonstrably dropped. Six
commands (`/permissions`, `/skill`, `/exit`, `/login`, `/upgrade`, `/plugins`) are hidden from
browsing but resolvable by prefix or unlocked by a provider/gate.

**Numbers corrected in passing.** `TBH_*` env vars ≈111 (not ~110), of which 46 are `TBH_TMUX_*`
(not ~55). Plugin diagnostics: 25 codes (not 23). MSP notifications: 22 server→client (the 23rd,
`initialized`, is client→server). MSP methods: 40 routed (31 published + 9 undocumented).
Skill diagnostics: 28 codes. Agent-definition diagnostic reasons: 51. Feature gates: 41.

---

## 11. Appendix D — source reports

Reverse engineering (each independently adversarially verified; all verdicts `MOSTLY_SOLID`):

- `re/cli-surface.md` — 16 commands, 5 parsers, 41 gates, hidden `workflows`/`plugins`
- `re/msp-protocol.md` — MSP wire protocol, 40 methods, serve/SDK surface
- `re/config-paths.md` — two roots, three tiers, 29 settings keys, no workspace settings file
- `re/plugins.md` — manifest, capabilities, lockfiles, Claude/Codex compatibility
- `re/skills.md` — SKILL.md format, discovery precedence, bundled `muse-core`, lockfile
- `re/hooks.md` — four tiers, 17 events, stdout protocol, per-event capability matrix
- `re/agents-subagents.md` — agent definitions, `--agents` overlay, worktree isolation
- `re/workflows-tools.md` — V8 workflow host API, the live 26-tool surface, MCP
- `re/model-providers.md` — provider router, `/responses` wire shape, credentials
- `re/security-permissions.md` — approval modes, permission profiles, trust, sandbox, judge
- `re/sessions-memory-rules.md` — event log, memory, rules chain, context management
- `re/tui-slash-theme.md` — 36 slash commands, keymap, the 24-theme catalog, status line

Ecosystem:

- `eco/ohmyzsh-arch.md` — oh-my-zsh teardown (236-line kernel, 359 plugins, updater refusal list)
- `eco/ohmy-family.md` — oh-my-bash / oh-my-fish / oh-my-posh / prezto / fisher / zinit / antidote / sheldon
- `eco/agent-plugin-ecosystems.md` — Claude Code, Codex, superpowers, oh-my-claudecode
- `eco/musecode-docs.md` — official documentation, release channels, public reporting

Extracted artefacts worth reading before implementation:

- `re/artifacts/create-plugin/references/native-plugin-contract.md` + `capability-examples.json`
  — **Meta's own authoritative plugin-authoring contract**, 23.6 KB of worked examples
- `re/tbh-reminders-manifest.json` — the complete 72 KB first-party reminder-agent manifest
- `re/skills-assets/muse-core/` — all 15 bundled skills, byte-identical to what ships
- `re/workflows-tools-artifacts/builtin-deep-research.workflow.FULL.js` — 441 lines, Meta's
  reference workflow (note: the truncated `.js` sibling in that directory is not parseable)
