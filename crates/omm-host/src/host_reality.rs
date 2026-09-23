//! Golden host facts: every numeric constant of `docs/host-reality.md` as a
//! `pub const`, and the seven data lists of `docs/host-data/*.json` parsed once
//! at first use. Each constant's doc comment names the report it was measured
//! in; `hostcheck` re-measures the P0/P1 rows against the live binary.
//!
//! R8: no asset name lives in Rust. Command names, gate ids, skill ids, hook
//! events, settings keys, reserved ids and slash commands are all read from the
//! data files, which are `include_str!`-ed so nothing is hand-copied.
//! R15: none of these values is a version number to gate on.

use std::sync::OnceLock;

use serde::de::DeserializeOwned;
use serde::Deserialize;

use crate::error::{HostError, Result};

// ---------------------------------------------------------------------------
// Budgets — docs/host-reality.md "Budgets", research/experiments/00-DECISION.md appendix
// ---------------------------------------------------------------------------

/// Hard cap of the order-200 `skills_catalog` context block, in bytes
/// (`research/experiments/quotas.md`; re-proven by binary search in
/// `research/experiments/canary-diff.md` §2.2: 21,863 B of description fits,
/// 21,864 collapses).
pub const SKILLS_CATALOG_CAP_BYTES: u64 = 32_000;
/// Fixed header of the skills catalog block (host-reality.md "Budgets").
pub const SKILLS_CATALOG_HEADER_BYTES: u64 = 535;
/// Fixed footer of the skills catalog block (host-reality.md "Budgets").
pub const SKILLS_CATALOG_FOOTER_BYTES: u64 = 35;
/// What Meta's 15 built-in skills cost in the catalog, entries only; refundable with
/// `muse skills disable bundled:<id> --scope built-in` (00-DECISION.md appendix).
/// `docs/experiments/context-slimming.md` §2 re-measured the whole order-200
/// block at 10,458 B = this + the 363-B header + the 35-B footer, exactly.
pub const BUILTIN_SKILLS_CATALOG_BYTES: u64 = 14_593;
/// The whole order-200 block with only the 15 built-ins (plugins gate on):
/// header + entries + footer (context-slimming.md §2, §6).
pub const BUILTIN_SKILLS_CATALOG_BLOCK_BYTES: u64 = 15_427;
/// Same block without the gate (19 entries plus the threejs plugin skill;
/// `bundled:create-plugin` is gated) — 15,000 B on 1.3.0-R3057.1.
pub const BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF: u64 = 15_000;
/// What the second builtin plugin's skill (`plugin:threejs:threejs`) costs in
/// the catalog: it composes unconditionally since 1.3.0-R3057.1 (both gates),
/// so the whole-block pins above include it.
pub const THREEJS_PLUGIN_CATALOG_BYTES: u64 = 264;
/// The 15-entry block under `run.context_slimming.skill_catalog_descriptions:
/// "first_sentence"` (context-slimming.md §6).
pub const BUILTIN_SKILLS_FIRST_SENTENCE_BLOCK_BYTES: u64 = 7_076;
/// Same with 14 entries (gate off) (context-slimming.md §6).
pub const BUILTIN_SKILLS_FIRST_SENTENCE_BLOCK_BYTES_GATE_OFF: u64 = 6_853;
/// Room left for a bundle with the built-ins on (R18, 00-DECISION.md rule 3).
pub const BUNDLE_BUDGET_BYTES: u64 = 16_573;
/// Room left for a bundle with the built-ins disabled (00-DECISION.md appendix).
pub const BUNDLE_BUDGET_BUILTINS_DISABLED_BYTES: u64 = 31_430;
/// Room left for a bundle with the built-ins on and `first_sentence` active:
/// 32,000 − 4,832 (context-slimming.md §6, +26 % over `full`).
pub const BUNDLE_BUDGET_FIRST_SENTENCE_BYTES: u64 = 24_924;
/// Catalog entry cost = `38 + len(display_id) + len(display_path) [+ len(desc) + 36]`
/// for a plugin-scope entry (host-reality.md "Budgets", entry cost): the plugin
/// id appears twice per entry, which is the `+2` over the user-scope base.
pub const CATALOG_ENTRY_BASE_BYTES: u64 = 38;
/// Catalog entry base for `scope="user"`: `36 + len(display_id) +
/// len(display_path) + len(rendered_desc) + 36` (context-slimming.md §2;
/// XML escaping such as `&apos;` counts).
pub const CATALOG_ENTRY_BASE_BYTES_USER: u64 = 36;
/// What `scope="plugin"` adds to the user-scope entry base (context-slimming.md §2).
pub const CATALOG_ENTRY_PLUGIN_SCOPE_EXTRA_BYTES: u64 = 2;
/// The extra cost of carrying a description in a catalog entry (same row).
pub const CATALOG_ENTRY_DESCRIPTION_OVERHEAD_BYTES: u64 = 36;
/// `settings.json → run.context_slimming` — the five typed slimming keys
/// (context-slimming.md §0). `skill_catalog_descriptions: "first_sentence"`
/// cuts every `<description>` after the first run of ASCII `.`/`?`/`!` that
/// is followed by a space (§3); the first sentence becomes the whole trigger
/// surface, so negative-trigger clauses in sentence two vanish.
pub const SETTINGS_CONTEXT_SLIMMING_KEY: &str = "run.context_slimming";
/// `skill_catalog_descriptions` value that keeps only the first sentence.
pub const CONTEXT_SLIMMING_FIRST_SENTENCE: &str = "first_sentence";
/// `skill_catalog_descriptions` default value (byte-identical to no setting).
pub const CONTEXT_SLIMMING_FULL: &str = "full";
/// Built-in default of `run.context_slimming.full_skill_description_ids`. A
/// user value REPLACES it: `bundled:git` drops from 740 B to 30 B unless it
/// is listed again (context-slimming.md §2). A host skill id, not an omm asset.
pub const CONTEXT_SLIMMING_DEFAULT_FULL_IDS: [&str; 1] = ["bundled:git"];
/// `run.context_slimming.session_identity_enabled: false` removes the order-240
/// `session_identity` block; its size varies with the session-log path
/// (context-slimming.md §2).
pub const SESSION_IDENTITY_BYTES_MIN: u64 = 776;
/// Upper bound of the same block.
pub const SESSION_IDENTITY_BYTES_MAX: u64 = 786;
/// Order-180 `workflow_choice` block with workflows on (context-slimming.md §2).
pub const WORKFLOW_CHOICE_BYTES: u64 = 3_421;
/// Order-180 `workflow_availability_off` block that replaces it under
/// `run.workflow_trigger_mode: "off"` (context-slimming.md §2).
pub const WORKFLOW_AVAILABILITY_OFF_BYTES: u64 = 539;
/// The `workflow` tool definition on the wire (context-slimming.md §5).
pub const WORKFLOW_TOOL_WIRE_BYTES: u64 = 15_766;
/// Net run-context saving of `run.workflow_trigger_mode: "off"`
/// (−18,056 − 2,896 + 539; context-slimming.md §2).
pub const WORKFLOW_OFF_RUN_CONTEXT_SAVING_BYTES: u64 = 3_920;
/// Net run-context saving of `run.context_slimming.excluded_tool_names:
/// ["workflow"]`, which removes orders 180 and 181 with no replacement
/// (context-slimming.md §7.4).
pub const WORKFLOW_EXCLUDED_RUN_CONTEXT_SAVING_BYTES: u64 = 4_459;
/// Order-186 `subagent_delegation` block, present only in a trusted workspace
/// (context-slimming.md §7.2).
pub const SUBAGENT_DELEGATION_BYTES: u64 = 956;
/// Combined cap of order 200 + order 201 when hook-selected skill routing is
/// on; the third outcome is `selected_skills:rejected:combined-budget`
/// (`research/experiments/skill-routing.md`, 00-DECISION.md §1.1 row 12).
pub const ROUTING_BUDGET_BYTES: u64 = 31_984;
/// Routed skills accepted per turn across all handlers (skill-routing.md).
pub const ROUTED_SKILLS_PER_TURN: usize = 32;
/// Max bytes of one routed skill's description (skill-routing.md).
pub const ROUTED_SKILL_DESCRIPTION_MAX_BYTES: u64 = 1_024;
/// Max bytes of a hook's stdout (skill-routing.md / hooks.md).
pub const HOOK_STDOUT_MAX_BYTES: u64 = 16_384;
/// The smallest `timeout` a shipped hook may carry: an omitted one lets the
/// hook block forever and `0` is clamped to this (hook-events.json
/// `protocol.timeout`, held to say so).
pub const HOOK_TIMEOUT_MIN_SECS: u64 = 1;
/// The only hook exit code that blocks (stderr = the reason) — hook-events.json
/// `protocol.exit_codes`. `0` completes; `1`, `3`, `127` and every other
/// non-zero exit are `status failed`, non-blocking: fail-open (R16). omm's
/// own dispatcher must therefore never exit 2 by accident — a crash in it
/// must not block the host.
pub const HOOK_EXIT_BLOCK: i32 = 2;
/// The four hook tiers in composition order (hook-events.json `protocol.tiers`,
/// host-reality.md "hooks tiers"): managed (`settings.managed_hooks_path`,
/// replaced by [`ENV_MANAGED_HOOKS_PATH`]; order 0) → user (`settings.json →
/// hooks`; 1) → project (`<ws>/.muse/hooks.json`, trust-gated; 2) → plugin
/// (`capabilities.hooks[]`; 100000+).
pub const HOOK_TIERS: [&str; 4] = ["managed", "user", "project", "plugin"];

/// The marker line that opens omm's managed region in the personal rules
/// file (`content/rules/AGENTS.md.tmpl`; the same literal as
/// `omm_manifest::lint::RULES_MANAGED_START`). A structural marker, not an
/// asset name (R8): doctor D13 uses it to spot an `AGENTS.md` that carries
/// omm's block with no ledger entry — an interrupt after the write but
/// before the ledger save (Gate 1 round 5 M2).
pub const RULES_MANAGED_START_MARKER: &str = "<!-- omm:managed-start -->";
/// What a config-file hook `command` runs under when `$SHELL` is unset
/// (hook-events.json `protocol.shell`). Native plugin hooks take a `command`
/// ARRAY — no shell, no placeholder expansion — which is why R16's
/// `["omm","hook","<name>"]` needs no quoting.
pub const HOOK_SHELL_FALLBACK: &str = "/bin/sh";
/// Keys of the scrubbed environment a config-file hook command runs with
/// (hook-events.json `protocol.shell`; ARCHITECTURE.md §8).
pub const HOOK_SHELL_ENV_KEYS: usize = 16;
/// The eight decision kinds of the per-event capability matrix
/// (hook-events.json `decision_kinds`, `items[].decisions`): `true` =
/// accepted with the stated effect, `false` = `status failed: unsupported
/// <field> in … hook output`, `null` = accepted, silently no effect
/// (`matrix_legend`). A hook handler shipped for an event must only emit
/// the kinds that event accepts.
pub const HOOK_DECISION_KINDS: [&str; 8] = [
    "block",
    "stop",
    "context",
    "feedback",
    "rewrite",
    "permission_verdict",
    "skills_v1",
    "bare_stdout_context",
];
/// The events whose foreground command hooks may return
/// `hookSpecificOutput.selectedSkills` under the two routing gates
/// (`decisions.skills_v1`); only the first applies them end to end
/// (`research/musecode/skills.md` verification R7), so Phase 3's router
/// hook installs there.
pub const HOOK_SKILLS_V1_EVENTS: [&str; 2] = ["UserPromptSubmit", "PostToolUse"];
/// The events on which non-JSON hook stdout is injected as context
/// (`decisions.bare_stdout_context`; everywhere else it is accepted with no
/// effect — hooks.md verification R6).
pub const HOOK_BARE_STDOUT_CONTEXT_EVENTS: [&str; 2] = ["SessionStart", "UserPromptSubmit"];
/// The memory snapshot block truncation outcome, in bytes, per scope
/// (`research/musecode/sessions-memory-rules.md` §3.4 and its verification R4).
pub const MEMORY_SNAPSHOT_BYTES: u64 = 16_305;
/// "Other Markdown files" listed per memory scope, silent cap (sessions-memory-rules.md).
pub const MEMORY_LISTED_FILES_MAX: usize = 48;
/// Per-file rules load limit, warned on stderr (sessions-memory-rules.md).
pub const RULES_FILE_MAX_BYTES: u64 = 256_000;
/// Aggregate rules load limit, warned on stderr (sessions-memory-rules.md).
pub const RULES_AGGREGATE_MAX_BYTES: u64 = 65_536;
/// The order-181 `workflow_cookbook` block; removable with
/// `run.workflow_trigger_mode: "off"` (sessions-memory-rules.md §5, measured live).
pub const WORKFLOW_COOKBOOK_BYTES: u64 = 1_038;
/// Plugin manifest size cap, inclusive (`research/experiments/plugin-contract.md` §1.12 step 1).
pub const PLUGIN_MANIFEST_MAX_BYTES: u64 = 131_072;
/// Max filesystem entries in a plugin package (`research/experiments/quotas.md`).
pub const PACKAGE_MAX_FS_ENTRIES: usize = 4_096;
/// Max path depth inside a plugin package (quotas.md).
pub const PACKAGE_MAX_PATH_DEPTH: usize = 16;
/// Inventory budget: 1/class + 2/skill + 1 per command·hook·mcpServer·reminder
/// (quotas.md, fitted from 11 bisections).
pub const INVENTORY_BUDGET_UNITS: usize = 4_094;
/// Per-class maximum for one plugin: skills (quotas.md).
pub const PER_CLASS_MAX_SKILLS: usize = 2_046;
/// Per-class maximum for one plugin: commands (quotas.md).
pub const PER_CLASS_MAX_COMMANDS: usize = 4_093;
/// Per-class maximum for one plugin: hooks (quotas.md).
pub const PER_CLASS_MAX_HOOKS: usize = 2_153;
/// Per-class maximum for one plugin: mcpServers (quotas.md).
pub const PER_CLASS_MAX_MCP_SERVERS: usize = 2_902;
/// Per-class maximum for one plugin: reminders — bounded by the manifest cap
/// (plugin-contract.md C5: 43 compact reminders fit, 45 exceed 131,072 B).
pub const PER_CLASS_MAX_REMINDERS: usize = 43;
/// Enabled plugins before `plugin_preflight_overflow` (quotas.md: 257 overflows, soft).
pub const ENABLED_PLUGINS_MAX: usize = 256;
/// Where doctor D7 starts warning (ARCHITECTURE.md §6).
pub const ENABLED_PLUGINS_WARN_AT: usize = 200;

// ---------------------------------------------------------------------------
// Identity — docs/host-reality.md "Identity constraints"
// ---------------------------------------------------------------------------

/// Plugin and capability id grammar (plugin-contract.md §1.3; same regex, same 80-byte cap).
pub const ID_GRAMMAR: &str = "^[a-z0-9][a-z0-9._-]{0,79}$";
/// Max id length in bytes (same).
pub const ID_MAX_BYTES: usize = 80;
/// The MCP *namespace* (`mcp__plugin_<pid>_<sid>`) survives verbatim on the
/// wire up to this length; 32+ is rewritten to `first-17 + "__" + 12 hex`
/// (`research/experiments/plugin-mcp.md`; re-measured wire-level in
/// `docs/experiments/mcp-tools-call.md` §3.1: the digest is a function of the
/// namespace string only).
pub const MCP_TOOL_NAME_VERBATIM_MAX: usize = 31;
/// Consequence: `len(plugin-id) + len(server-id) <= 18` (00-DECISION.md rule 4).
/// Functional, not cosmetic: past it the `<ns>__<fn>` addressing form stops
/// dispatching and the canonical id is never on the wire (mcp-tools-call.md §3).
pub const MCP_ID_LENGTH_BUDGET: usize = 18;
/// Model-visible MCP namespace / canonical id prefix: `mcp__plugin_<pid>_<sid>`
/// is the wire `tools[]` namespace entry, `mcp__plugin_<pid>_<sid>__<tool>` the
/// canonical id in `session.jsonl` (plugin-mcp.md; mcp-tools-call.md §2.2).
pub const MCP_TOOL_NAME_PREFIX: &str = "mcp__plugin_";
/// The wire namespace that carries the built-in tools (mcp-tools-call.md §4).
pub const MCP_BUILTIN_NAMESPACE: &str = "muse";
/// `{"type":"namespace","description":…}` text of every MCP namespace group.
pub const MCP_NAMESPACE_DESCRIPTION: &str = "Tools provided by an MCP server.";
/// The `function_call.name` forms the host dispatches (mcp-tools-call.md §3.2):
/// `<ns>.<fn>` for every namespace length; `<ns>__<fn>` only while the wire
/// namespace is unrewritten; the canonical id always (but never on the wire once
/// rewritten). Bare `<fn>`, `<fn>` + a `namespace` field and `<ns>/<fn>` are
/// `tool unavailable: unknown tool …`, fed back to the model, turn continues.
pub const MCP_CALL_SEPARATOR_DOT: &str = ".";
/// See [`MCP_CALL_SEPARATOR_DOT`].
pub const MCP_CALL_SEPARATOR_DUNDER: &str = "__";
/// `muse serve` refuses an MSP `clientInfo.name` outside this pattern
/// (mcp-tools-call.md §2.3, SS1.4.1).
pub const MSP_CLIENT_NAME_PATTERN: &str = "^[a-z0-9_]+$";
/// The environment variable a scripted `meta` provider needs for an offline
/// `muse exec --provider meta --base-url http://127.0.0.1:<port>` or
/// `muse serve` (`settings.endpoint_transport.base_url`) run; any dummy value
/// (mcp-tools-call.md §1). omm never sets it to a real key.
pub const ENV_META_API_KEY: &str = "META_API_KEY";
/// Native plugin `capabilities.hooks[]` rejects these fields with
/// `unsupported-field` (`docs/experiments/marketplace-precedence.md` §7): the
/// Windows twin of `command` exists only in the hooks.json document tiers.
pub const PLUGIN_HOOK_REJECTED_FIELDS: [&str; 2] = ["commandWindows", "command_windows"];

// ---------------------------------------------------------------------------
// Marketplaces — docs/host-reality.md "Marketplaces", docs/experiments/marketplace-precedence.md
// ---------------------------------------------------------------------------

/// The catalog files `muse plugins marketplace add <dir|git>` probes, in order;
/// the FIRST existing file wins, nothing is merged and a parse error does not
/// fall through to the next file (marketplace-precedence.md §1). The root
/// `marketplace.json` is the native Muse catalog; the two foreign files serve
/// the `.agents/plugins` (Codex schema) and `.claude-plugin` (Claude schema)
/// ecosystems and are never opened while the root file exists.
pub const MARKETPLACE_PROBE_ORDER: [&str; 3] = [
    "marketplace.json",
    ".agents/plugins/marketplace.json",
    ".claude-plugin/marketplace.json",
];
/// Native root catalog `schemaVersion` (marketplace-precedence.md §2).
pub const MARKETPLACE_NATIVE_SCHEMA_VERSION: u64 = 1;
/// Native root catalog `source` — the only accepted value (`only local
/// marketplace sources are supported`; marketplace-precedence.md §2).
pub const MARKETPLACE_NATIVE_SOURCE: &str = "local";
/// Native catalog entry `install.transport` (marketplace-precedence.md §2).
pub const MARKETPLACE_NATIVE_TRANSPORT: &str = "local-path";
/// Native catalog entry `integrity.digest` prefix; the value is Muse's
/// content-addressed `package_sha256`, verified at INSTALL, not at add
/// (marketplace-precedence.md §2). Obtain it from the binary, never compute it.
pub const MARKETPLACE_DIGEST_PREFIX: &str = "sha256:";
/// `marketplace update` keeps this many generations (current + previous) under
/// `plugins/marketplaces/<name>/generations/<epoch-ns>/` (marketplace-precedence.md §5.2).
pub const MARKETPLACE_GENERATIONS_KEPT: usize = 2;
/// The install-time failure code once the generation a plugin was installed
/// from is rotated out (marketplace-precedence.md §5.3).
pub const MARKETPLACE_SOURCE_UNAVAILABLE_CODE: &str = "plugin-source-unavailable";
/// Skill description ceiling omm lints for (ARCHITECTURE.md §5.3).
pub const SKILL_DESCRIPTION_MAX_CHARS: usize = 240;
/// The only manifest family omm accepts (plugin-contract.md C1: the same bytes
/// validate in all three dot-dirs with different runtime semantics).
pub const MANIFEST_FAMILY_NATIVE: &str = "native";
/// Native manifest directory (plugin-contract.md §1.1).
pub const MANIFEST_DIR_NATIVE: &str = ".muse-plugin";
/// The five capability families as spelled in a manifest (`research/musecode/plugins.md` §3).
pub const CAPABILITY_FAMILIES_MANIFEST: [&str; 5] =
    ["skills", "commands", "hooks", "mcpServers", "reminders"];
/// The same five as `plugins validate --json` reports them under
/// `plugin.capabilities`, in the host's order (canary-diff.md §1 row 11).
pub const CAPABILITY_FAMILIES_VALIDATOR: [&str; 5] =
    ["skills", "hooks", "mcp_servers", "commands", "reminders"];
/// Families that are hard errors in a native manifest when nothing supported
/// remains, and warnings otherwise (plugins.md §3.9, canary-diff.md §2.1).
pub const CAPABILITY_FAMILIES_REJECTED: [&str; 5] =
    ["tools", "agents", "outputStyles", "settings", "apps"];

// ---------------------------------------------------------------------------
// Trust lifecycle — docs/host-reality.md "Trust lifecycle"
// ---------------------------------------------------------------------------

/// The only runtime-capability state in which MCP servers, hooks and reminders
/// spawn (`research/experiments/plugin-mcp.md`, 00-DECISION.md rule 14).
pub const RUNTIME_CAPABILITY_ACTIVE_STATE: &str = "trusted_enabled";
/// `RuntimeCapabilityTrustStatus` vocabulary (plugins.md §9.3).
pub const RUNTIME_CAPABILITY_STATES: [&str; 6] = [
    "review_needed",
    "trusted_enabled",
    "trusted_disabled",
    "modified",
    "invalid",
    "blocked",
];

// ---------------------------------------------------------------------------
// Schema versions and fingerprints — docs/host-reality.md P0
// ---------------------------------------------------------------------------

/// `settings.json` → `schema_version` (u32, hard-required, 0 and 2 rejected;
/// config-paths.md verification R4).
pub const SETTINGS_SCHEMA_VERSION: u64 = 1;
/// Plugin manifest `schemaVersion` (0, 2, 99 and `"1"` rejected; plugin-contract.md §1.2).
pub const PLUGIN_MANIFEST_SCHEMA_VERSION: u64 = 1;
/// `trust.json` → `schema_version` (`unsupported trust store schema version 2`;
/// security-permissions.md verification 11).
pub const TRUST_SCHEMA_VERSION: u64 = 1;
/// `muse export` document `export_schema_version` (cli-surface.md §3.9).
pub const EXPORT_SCHEMA_VERSION: u64 = 1;
/// `session.jsonl` retained-frame `frame_schema_version` (canary-diff.md §1 row 38).
pub const FRAME_SCHEMA_VERSION: u64 = 1;
/// MSP wire envelope `schemaVersion` in `manifest.json` (msp-protocol.md §2.3).
pub const MSP_ENVELOPE_SCHEMA_VERSION: u64 = 1;
/// `muse schema generate-json-schema` → `manifest.json` → `fingerprint` (stable surface).
/// 1.3.0-R3401.1 adds `session/listChanged` to notifications.
pub const MSP_STABLE_FINGERPRINT: &str =
    "sha256:7469c9e352e67def4a59df7e439984d7194fa351e1c8b7abb34060fd977ced81";
/// Same with `--experimental` (byte-identical schema, different fingerprint; msp-protocol.md §2.4).
pub const MSP_EXPERIMENTAL_FINGERPRINT: &str =
    "sha256:2db88d9ee93131257757ab3cf74026eab9079813a4cb82c45cc231901a170fc6";
/// SHA-256 of the emitted `msp.schema.json` (canary-diff.md §5).
pub const MSP_SCHEMA_JSON_SHA256: &str =
    "ed442d494cf8cdb85f0b1487f8d0f54b7570548561c6c92d6def09ce107e1664";
/// SHA-256 of the emitted `msp.d.ts` (canary-diff.md §5).
pub const MSP_DTS_SHA256: &str = "2c2a7b481db1fe1e659789cc687cf47f4c7d1a96fec09eb7dc78f7b13c626c39";
/// `muse config status` → `Generation:` (config-paths.md §7.1). The hash covers
/// the source list, which is OS-specific: macOS reports `macos_managed_preferences`
/// planes (four lines, all absent) while Linux reports only `system_file` (two
/// lines, all absent) — measured live on 1.3.0-R3057.1, macOS locally and Linux in
/// a clean container (the same a0cf value the ubuntu CI runners observe).
#[cfg(target_os = "macos")]
pub const ENTERPRISE_GENERATION: &str =
    "sha256:db7c1fb6263c2ca1483bcaae0cce50d323b491f600c88f38069012a1b008b5e4";
#[cfg(not(target_os = "macos"))]
pub const ENTERPRISE_GENERATION: &str =
    "sha256:a0cf253e6e09d7739aecef4f30be4e5f6e5671baafd912ad123fdbb8315b5cdf";
/// Source lines in `muse config status` output: four on macOS (system_file
/// plus macos_managed_preferences, defaults and policy each), two elsewhere.
#[cfg(target_os = "macos")]
pub const ENTERPRISE_SOURCES: usize = 4;
/// See the macOS definition.
#[cfg(not(target_os = "macos"))]
pub const ENTERPRISE_SOURCES: usize = 2;
/// MSP client→server methods, counted from `msp.schema.json` → `methods` (msp-protocol.md §1).
pub const MSP_METHODS: usize = 47;
/// MSP server→client notifications, `msp.schema.json` → `notifications` (msp-protocol.md §1).
/// 31 on 1.3.0-R3401.1 (`session/listChanged` joined).
pub const MSP_NOTIFICATIONS: usize = 31;
/// MSP error registry rows, `msp.schema.json` → `errors` (msp-protocol.md §1).
pub const MSP_ERROR_CODES: usize = 31;

// ---------------------------------------------------------------------------
// Counts — docs/host-reality.md P1
// ---------------------------------------------------------------------------

/// `MUSE_EXPERIMENTAL_*` gates listed in gates.json — 41 on 1.0.1-R2006.1
/// (cli-surface.md §4.1; config-paths.md verification C3) plus
/// `ultra_reasoning_effort`, first observed on 1.0.3-R2198.1 (gates.json
/// `since`; `docs/experiments/stable-1.0.3-diff.md`), 47 on 1.3.0-R3057.1,
/// 45 on 1.3.0-R3401.1 (`subscription_launch` and `context_meter` removed).
/// The count a given build is expected to show is derived per build by
/// [`Gates::partition_for_build`] from what probing finds, never from this
/// constant alone (R15).
pub const GATES_TOTAL: usize = 45;
/// Gates resolving `enabled=true source="default"` (canary-diff.md §1 rows 4–5;
/// unchanged on 1.0.3-R2198.1).
pub const GATES_DEFAULT_ON: usize = 17;
/// Hook events, PascalCase only (`research/musecode/hooks.md`; canary-diff.md row 18).
pub const HOOK_EVENTS: usize = 17;
/// Bundled skills, `muse skills list --source built-in --json` with the plugins gate
/// (bundled-skills.json `counts.skills`).
pub const BUNDLED_SKILLS: usize = 20;
/// Bundled skills visible without `MUSE_EXPERIMENTAL_PLUGINS` (`create-plugin` is gated;
/// bundled-skills.json `counts.visible_by_default`).
pub const BUNDLED_SKILLS_VISIBLE_DEFAULT: usize = 19;
/// Files materialised under `skills/bundled/muse-core/` (bundled-skills.json `package_files`).
/// 42 on 1.3.0-R3401.1 (`CREDITS.md` plus `skills/slack-connector/references/slack-ui.md`).
pub const BUNDLED_SKILL_PACKAGE_FILES: usize = 42;
/// Active tools of an echo session in a **trusted** workspace (canary-diff.md row 25;
/// the six `subagent_*` tools are absent when the workspace is untrusted — measured
/// 2026-09-02, `docs/experiments/context-slimming.md` §7.2, see `probe::echo_session`).
pub const ACTIVE_TOOLS: usize = 29;
/// Active tools of an echo session in an **untrusted** workspace: no
/// `subagent_*` tools (context-slimming.md §7.2).
pub const ACTIVE_TOOLS_UNTRUSTED: usize = 23;
/// Composer picker rows on a pristine HOME (slash-commands.json `counts.items`).
pub const BUILTIN_SLASH_COMMANDS: usize = 44;
/// Top-level CLI commands: 14 advertised + `plugins` (gated) + `workflows` (hidden)
/// (cli-surface.md §1.1).
pub const TOP_LEVEL_COMMANDS: usize = 17;
/// Commands listed by a bare `muse --help` (cli-surface.md §1.1).
pub const ADVERTISED_COMMANDS: usize = 15;
/// Context blocks of an echo session in a trusted workspace, by order
/// (canary-diff.md row 21; sessions-memory-rules.md §5). Order 185
/// `agent_definition_catalog` joined on 1.3.0-R3401.1.
pub const CONTEXT_BLOCK_ORDERS: [u32; 10] = [85, 95, 96, 180, 181, 185, 186, 200, 240, u32::MAX];
/// The same session in an **untrusted** workspace: no order-186
/// `subagent_delegation` (context-slimming.md §7.2, measured 2026-09-02).
pub const CONTEXT_BLOCK_ORDERS_UNTRUSTED: [u32; 9] =
    [85, 95, 96, 180, 181, 185, 200, 240, u32::MAX];
/// Context order of the `agent_definition_catalog` block (new on
/// 1.3.0-R3401.1, trusted and untrusted).
pub const CONTEXT_ORDER_AGENT_DEFINITION_CATALOG: u32 = 185;
/// Context order of the trust-gated `subagent_delegation` block.
pub const CONTEXT_ORDER_SUBAGENT_DELEGATION: u32 = 186;
/// Context order of `workflow_availability_proactive` (1,355 B,
/// [`WORKFLOW_AVAILABILITY_PROACTIVE_BYTES`]) — composes only under
/// `settings.reasoning_effort: "ultra"`: unconditionally on 1.0.1-R2006.1 and
/// canary 1.1.0-R2074.1, behind the `ultra_reasoning_effort` gate since
/// 1.0.3-R2198.1 (gates.json n=42 `effect_probe`; stable-1.0.3-diff.md §3).
pub const CONTEXT_ORDER_WORKFLOW_AVAILABILITY_PROACTIVE: u32 = 183;
/// Context order of `subagent_delegation_proactive` (528 B,
/// [`SUBAGENT_DELEGATION_PROACTIVE_BYTES`]); same conditions as order 183.
pub const CONTEXT_ORDER_SUBAGENT_DELEGATION_PROACTIVE: u32 = 187;
/// Bytes of the order-183 block under ultra effort (2026-09-05, all three builds).
pub const WORKFLOW_AVAILABILITY_PROACTIVE_BYTES: u64 = 1_355;
/// Bytes of the order-187 block under ultra effort (2026-09-05, all three builds).
pub const SUBAGENT_DELEGATION_PROACTIVE_BYTES: u64 = 528;
/// Context order of the `session_identity` block.
pub const CONTEXT_ORDER_SESSION_IDENTITY: u32 = 240;
/// Context order of the rules block (present only when a rules file loads).
pub const CONTEXT_ORDER_RULES_FILE: u32 = 100;
/// Context order of the skills catalog.
pub const CONTEXT_ORDER_SKILLS_CATALOG: u32 = 200;
/// Context order of the hook-selected skills catalog (routing).
pub const CONTEXT_ORDER_SELECTED_SKILLS: u32 = 201;
/// Context order of the workflow cookbook.
pub const CONTEXT_ORDER_WORKFLOW_COOKBOOK: u32 = 181;
/// Context order of the memory snapshot (`u32::MAX`; sessions-memory-rules.md §5).
pub const CONTEXT_ORDER_MEMORY_SNAPSHOT: u32 = u32::MAX;

// ---------------------------------------------------------------------------
// Exit codes and environment — docs/host-reality.md "Exit codes", loose-ends.md §1.3
// ---------------------------------------------------------------------------

/// Ran and succeeded.
pub const EXIT_OK: i32 = 0;
/// Parsed, run failed (missing gate at runtime, malformed settings, hook block, …).
pub const EXIT_RUN_FAILED: i32 = 1;
/// argv rejected — parse error OR missing gate, indistinguishable; uniform across
/// all five parser dialects.
pub const EXIT_ARGV_REJECTED: i32 = 2;
/// The host's own timeout kill of its hidden `__tbh_internal_process_owner_pty_gate_v1`
/// mode (muse-cli.json `exit_codes`, `hidden_argv_modes`) — not a product
/// surface; `OutcomeKind::Other(125)` when ever seen.
pub const EXIT_PTY_GATE_TIMEOUT: i32 = 125;
/// Launcher-only variable (0 occurrences in the binary; cli-surface.md §2) —
/// harmless on the raw binary, mandatory on the launcher.
pub const ENV_NO_AUTO_UPDATE: &str = "MUSE_NO_AUTO_UPDATE";
/// Gates the `muse plugins` verbs and the `create-plugin` skill; runtime
/// composition is ungated (loose-ends.md §4.6).
pub const ENV_PLUGINS_GATE: &str = "MUSE_EXPERIMENTAL_PLUGINS";
/// Prefix of every gate variable (gates.json `env_prefix`).
pub const ENV_GATE_PREFIX: &str = "MUSE_EXPERIMENTAL_";
/// Routing gate 1 of 2 (skill-routing.md).
pub const ENV_ROUTING_GATE: &str = "MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS";
/// Routing gate 2 of 2 (skill-routing.md).
pub const ENV_ROUTING_APPLY_GATE: &str = "MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY";
/// Replaces `settings.managed_hooks_path` when set (settings-keys.json row 24).
pub const ENV_MANAGED_HOOKS_PATH: &str = "TBH_MANAGED_HOOKS_PATH";
/// Kills the remote feature-config fetch (gates.json `remote_override`). Set
/// by `Invoker` in its clean environment for every probe, never for `omm run`,
/// so a gate measurement is the binary's and not the server's; the trace line
/// [`FEATURE_CONFIG_CACHE_EVENT`] reports whether a cache was consulted.
pub const ENV_DISABLE_FEATURE_CONFIG: &str = "TBH_DISABLE_FEATURE_CONFIG";
/// The value omm sets a gate variable to (`MUSE_EXPERIMENTAL_PLUGINS=1`);
/// gates.json `accepted_values.on` is held to contain it.
pub const GATE_ON_VALUE: &str = "1";

// ---------------------------------------------------------------------------
// Literals omm keys on — the exact strings, so message matching is in one place
// ---------------------------------------------------------------------------

/// `--version` output prefix: `Muse Code 1.0.1 (1.0.1-R2006.1)` (cli-surface.md header).
pub const VERSION_STRING_PRODUCT: &str = "Muse Code";
/// Prefix of the settings-collision error every settings-mutating command prints
/// (settings-plugins.md verification C5).
pub const MCP_COLLISION_MESSAGE: &str = "MCP configuration error";
/// Prefix of the malformed-settings error (loose-ends.md §1.4).
pub const SETTINGS_MALFORMED_MESSAGE: &str = "malformed settings file";
/// Prefix of the settings schema-version error (loose-ends.md §1.4).
pub const SETTINGS_UNSUPPORTED_VERSION_MESSAGE: &str = "unsupported settings schema version";
/// The trace line every gate resolution writes (gates.json `probe.line_pattern`).
pub const GATE_RESOLVE_EVENT: &str = "event=\"gate.resolve\"";
/// First line of every complete bootstrap trace (`event="startup" host="cli"`).
/// The writer is lossy under concurrent bootstraps (gates.json `probe.note`), so
/// a trace without this head is not authoritative.
pub const BOOTSTRAP_TRACE_HEAD_EVENT: &str = "event=\"startup\"";
/// Last line of every complete bootstrap trace, written after the gate table
/// (`event="trust.resolve" outcome=… source=…`); absent from a tail-truncated log.
pub const BOOTSTRAP_TRACE_TAIL_EVENT: &str = "event=\"trust.resolve\"";
/// The bootstrap line naming the remote feature-config cache
/// (`event="feature_config.cache" state="missing" gate_count=0` on every
/// offline run; the cache lives under the data root — `path.resolved
/// kind="feature_config_cache" source="data_root"`). A `gate_count` above 0
/// means server-side overrides are in play (host-reality.md "Server-side risk").
pub const FEATURE_CONFIG_CACHE_EVENT: &str = "event=\"feature_config.cache\"";
/// Lines of a complete `plugins enable` bootstrap trace that are not gate
/// resolutions: startup + process.identity + 4× path.resolved +
/// feature_config.cache + trust.resolve. A complete trace has this many lines
/// plus one `gate.resolve` per gate the build knows (49 on a 41-gate build,
/// 50 on 1.0.3-R2198.1); the completeness test is the head/tail pair, not
/// the count.
pub const BOOTSTRAP_TRACE_FIXED_LINES: usize = 8;
/// Lines of a complete bootstrap trace on the newest observed stable build
/// (1.0.3-R2198.1, 42 gates). Informational — see [`BOOTSTRAP_TRACE_FIXED_LINES`].
pub const BOOTSTRAP_TRACE_LINES_OBSERVED: usize = BOOTSTRAP_TRACE_FIXED_LINES + GATES_TOTAL;
/// The host refusing a `--reasoning-effort` value outside its list
/// (`unsupported reasoning effort `<v>`; expected none|…`, exit 2) — the
/// value check runs before the provider check, so with `--provider echo` an
/// accepted value fails later with [`REASONING_EFFORT_ECHO_REFUSED_MESSAGE`]
/// instead (muse-cli.json `--reasoning-effort` notes; measured 2026-09-05 on
/// 1.0.1-R2006.1 and 1.0.3-R2198.1).
pub const REASONING_EFFORT_UNSUPPORTED_MESSAGE: &str = "unsupported reasoning effort";
/// The host refusing an accepted `--reasoning-effort` value under the echo
/// provider (exit 2). Seeing this line proves the value passed the list.
pub const REASONING_EFFORT_ECHO_REFUSED_MESSAGE: &str =
    "--reasoning-effort is not supported with --provider echo";
/// The session event carrying the context blocks and the toolset
/// (sessions-memory-rules.md §5, verified live).
pub const MODEL_REQUEST_CONFIGURED_EVENT: &str = "model_request_configured";

// ---------------------------------------------------------------------------
// Data lists — docs/host-data/*.json, compiled in
// ---------------------------------------------------------------------------

/// Raw `docs/host-data/bundled-skills.json`.
pub const RAW_BUNDLED_SKILLS: &str = include_str!("../../../docs/host-data/bundled-skills.json");
/// Raw `docs/host-data/gates.json`.
pub const RAW_GATES: &str = include_str!("../../../docs/host-data/gates.json");
/// Raw `docs/host-data/hook-events.json`.
pub const RAW_HOOK_EVENTS: &str = include_str!("../../../docs/host-data/hook-events.json");
/// Raw `docs/host-data/muse-cli.json`.
pub const RAW_MUSE_CLI: &str = include_str!("../../../docs/host-data/muse-cli.json");
/// Raw `docs/host-data/reserved-ids.json`.
pub const RAW_RESERVED_IDS: &str = include_str!("../../../docs/host-data/reserved-ids.json");
/// Raw `docs/host-data/settings-keys.json`.
pub const RAW_SETTINGS_KEYS: &str = include_str!("../../../docs/host-data/settings-keys.json");
/// Raw `docs/host-data/slash-commands.json`.
pub const RAW_SLASH_COMMANDS: &str = include_str!("../../../docs/host-data/slash-commands.json");
/// Raw `crates/omm-host/data/enterprise-defaults-plane.json` — what the offline
/// settings validator (`muse config validate --plane defaults`) can see.
pub const RAW_ENTERPRISE_DEFAULTS_PLANE: &str =
    include_str!("../data/enterprise-defaults-plane.json");
/// Raw `crates/omm-host/data/fixtures/reminder-decision.json` — a reminder
/// `decision` block derived from the bundled `capability-examples.json`.
pub const RAW_REMINDER_DECISION_FIXTURE: &str =
    include_str!("../data/fixtures/reminder-decision.json");

fn parse_once<T: DeserializeOwned + Send + Sync + 'static>(
    cell: &'static OnceLock<std::result::Result<T, String>>,
    name: &'static str,
    raw: &'static str,
) -> Result<&'static T> {
    cell.get_or_init(|| serde_json::from_str::<T>(raw).map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|detail| HostError::Data {
            name,
            detail: detail.clone(),
        })
}

macro_rules! data_accessor {
    ($(#[$meta:meta])* $fn_name:ident, $ty:ty, $raw:expr, $name:literal) => {
        $(#[$meta])*
        pub fn $fn_name() -> Result<&'static $ty> {
            static CELL: OnceLock<std::result::Result<$ty, String>> = OnceLock::new();
            parse_once(&CELL, $name, $raw)
        }
    };
}

/// One bundled skill (bundled-skills.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct BundledSkill {
    /// Position in the carved `muse-core` manifest (= position in `items`).
    pub manifest_index: u32,
    pub id: String,
    pub qualified_id: String,
    pub path: String,
    #[serde(rename = "enabledDefault")]
    pub enabled_default: bool,
    pub user_invocable: bool,
    pub gated_by: Option<String>,
    /// `true` for the seven `/<id>` composer rows (user-invocable and
    /// ungated); `null` for the gated skill, whose row was never observed.
    #[serde(default)]
    pub slash_shortcut: Option<bool>,
    #[serde(default)]
    pub argument_hint: Option<String>,
    /// `metadata.short-description`, shown by the picker instead of `description`.
    #[serde(default)]
    pub short_description: Option<String>,
    pub skill_md_bytes: u64,
    pub description: String,
}

/// bundled-skills.json.
#[derive(Clone, Debug, Deserialize)]
pub struct BundledSkills {
    pub plugin_id: String,
    pub id_prefix: String,
    pub counts: BundledSkillCounts,
    pub catalog_cost: BundledCatalogCost,
    pub items: Vec<BundledSkill>,
    pub package_files: Vec<PackageFile>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BundledSkillCounts {
    pub skills: usize,
    pub visible_by_default: usize,
    pub gated: usize,
    pub package_files: usize,
    pub slash_shortcuts: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BundledCatalogCost {
    pub all_20_rendered_bytes: u64,
    pub refund_command: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PackageFile {
    pub path: String,
    pub bytes: u64,
}

data_accessor!(
    /// The 20 bundled skills of plugin `muse-core`.
    bundled_skills, BundledSkills, RAW_BUNDLED_SKILLS, "bundled-skills.json"
);

/// One gate (gates.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct Gate {
    pub n: u32,
    pub env: String,
    pub id: String,
    #[serde(rename = "default")]
    pub default_state: String,
    pub cli_visible: bool,
    pub evidence: String,
    /// The build the gate was first observed on (gates.json `since_rule`):
    /// absent from the trace of a build that predates it, the row is
    /// OLDER-BUILD rather than drift. `None` for the rows every observed
    /// build carries.
    #[serde(default)]
    pub since: Option<String>,
    /// How the gate's effect is re-measured (gates.json `effect_probe`), when
    /// it can be seen offline.
    #[serde(default)]
    pub effect_probe: Option<GateEffectProbe>,
}

impl Gate {
    /// True when the gate resolves `enabled=true source="default"`.
    pub fn default_on(&self) -> bool {
        self.default_state == "on"
    }
}

/// gates.json `items[].effect_probe` — a behavioural oracle for one gate.
/// `kind` is `settings_echo_session`: write `settings` as the sandbox's
/// `settings.json`, run an echo session in a trusted workspace, compare the
/// context-block orders and the first stderr line with `closed` (gate present
/// in the trace, variable unset), `open` (variable set to [`GATE_ON_VALUE`])
/// or `absent` (no `gate.resolve` line for it — a build predating `since`).
#[derive(Clone, Debug, Deserialize)]
pub struct GateEffectProbe {
    pub kind: String,
    pub settings: serde_json::Value,
    pub closed: GateEffectExpectation,
    pub open: GateEffectExpectation,
    pub absent: GateEffectExpectation,
    #[serde(default)]
    pub note: String,
}

/// The only `effect_probe.kind` hostcheck knows how to run.
pub const GATE_EFFECT_PROBE_SETTINGS_ECHO_SESSION: &str = "settings_echo_session";

/// One expected outcome of a [`GateEffectProbe`].
#[derive(Clone, Debug, Deserialize)]
pub struct GateEffectExpectation {
    /// A stderr line the host must print (`Some`) or must not print (`None` —
    /// the `closed` prefix must then be absent).
    #[serde(default)]
    pub stderr_prefix: Option<String>,
    /// The context-block orders of the resulting session, ascending.
    pub context_orders: Vec<u64>,
}

/// How the gates a build is expected to resolve are derived from what its
/// trace shows ([`Gates::partition_for_build`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GatePartition {
    /// Every listed gate the build must resolve: all rows without `since`,
    /// plus every `since`-tagged row the trace shows.
    pub expected: std::collections::BTreeSet<String>,
    /// `since`-tagged rows the trace does not show — allowed to be absent on
    /// a build that predates their tag (OLDER-BUILD), never drift by
    /// themselves. Whether the build predates the tag is decided by the whole
    /// since group behaving that way (hostcheck), not by the version string.
    pub older_build: Vec<String>,
}

impl Gates {
    /// Split the listed gates for a build whose trace resolved `observed`
    /// (gates.json `since_rule`). An observed id that is not listed is not
    /// in either set — the caller reports it as drift.
    pub fn partition_for_build<S: AsRef<str>>(&self, observed: &[S]) -> GatePartition {
        let seen: std::collections::BTreeSet<&str> = observed.iter().map(AsRef::as_ref).collect();
        let mut part = GatePartition::default();
        for g in &self.items {
            if g.since.is_some() && !seen.contains(g.id.as_str()) {
                part.older_build.push(g.id.clone());
            } else {
                part.expected.insert(g.id.clone());
            }
        }
        part
    }
    /// The default-ON ids a build is expected to show given the partition.
    pub fn default_on_expected(&self, part: &GatePartition) -> std::collections::BTreeSet<String> {
        self.default_on_ids
            .iter()
            .filter(|id| !part.older_build.contains(id))
            .cloned()
            .collect()
    }
    /// The `since`-tagged rows: `(since build, gate id)`.
    pub fn since_tagged(&self) -> Vec<(&str, &str)> {
        self.items
            .iter()
            .filter_map(|g| g.since.as_deref().map(|s| (s, g.id.as_str())))
            .collect()
    }
}

/// The launcher's version grammar (`1.0.3-R2198.1`), the shape every `since`
/// tag must have (scripts/fetch-host.sh `version_pattern`).
pub fn is_build_version(s: &str) -> bool {
    let Some((semver, release)) = s.split_once("-R") else {
        return false;
    };
    let parts: Vec<&str> = semver.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && !release.is_empty()
        && release
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        && release.split('.').count() <= 2
}

/// gates.json `probe` — how the 41 resolutions are observed. The probe verb
/// and the trace literal live here, not in Rust (R8).
#[derive(Clone, Debug, Deserialize)]
pub struct GateProbe {
    /// `MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins enable __omm_gate_probe__`.
    pub command: String,
    /// `$XDG_DATA_HOME/muse/local-tracing/bootstrap/cli-*.log`.
    pub trace_glob: String,
    /// `event="gate.resolve" gate="<id>" enabled=<bool> source="default|override"`.
    pub line_pattern: String,
    #[serde(default)]
    pub note: String,
}

impl GateProbe {
    /// The argv of the probe verb: `command` minus its `KEY=VALUE` prefixes
    /// and the program name (`["plugins", "enable", "__omm_gate_probe__"]`).
    pub fn argv(&self) -> Vec<String> {
        self.command
            .split_whitespace()
            .skip_while(|t| t.contains('='))
            .skip(1)
            .map(str::to_string)
            .collect()
    }
    /// The trace directory under the data root, from `trace_glob`
    /// (`local-tracing/bootstrap`).
    pub fn trace_subdir(&self) -> Option<String> {
        let rest = self.trace_glob.split("/muse/").nth(1)?;
        let (dir, _file) = rest.rsplit_once('/')?;
        Some(dir.to_string())
    }
}

/// gates.json `accepted_values` — what a gate variable parses as on / off
/// (measured for two gates, inferred for the rest).
#[derive(Clone, Debug, Deserialize)]
pub struct GateValues {
    pub on: Vec<String>,
    pub off: Vec<String>,
    #[serde(default)]
    pub note: String,
}

/// gates.json.
#[derive(Clone, Debug, Deserialize)]
pub struct Gates {
    pub counts: GateCounts,
    pub env_prefix: String,
    pub accepted_values: GateValues,
    pub probe: GateProbe,
    pub items: Vec<Gate>,
    pub default_on_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GateCounts {
    pub gates: usize,
    pub default_on: usize,
    pub default_off: usize,
    /// Gates that change an argv/`--help` surface (`plugins`, `external_agent_ingress`).
    pub cli_visible: usize,
}

data_accessor!(
    /// The 41 `MUSE_EXPERIMENTAL_*` gates with their defaults.
    gates, Gates, RAW_GATES, "gates.json"
);

/// One hook event (hook-events.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct HookEvent {
    pub name: String,
    pub snake: String,
    pub fires_in_echo_session: bool,
    /// What the group `matcher` selects on (`source`, `reason`, a tool name,
    /// `null` for none — any matcher but `*`/absent then disables the group).
    #[serde(default)]
    pub matcher_selector: Option<String>,
    /// The event's row of the capability matrix, keyed by
    /// [`HOOK_DECISION_KINDS`]: `Some(true)` accepted, `Some(false)` fails
    /// the hook, `None` (JSON `null`) accepted with no effect.
    pub decisions: std::collections::BTreeMap<String, Option<bool>>,
    pub payload: HookPayload,
}

/// hook-events.json `items[].payload` — what the stdin document carries
/// beyond `common_payload`, by evidence level.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct HookPayload {
    /// Fields observed on a live fire.
    #[serde(default)]
    pub proven: Vec<String>,
    /// Fields recovered from the binary's strings, not yet observed.
    #[serde(default)]
    pub plausible: Vec<String>,
    /// Nothing is known about the payload.
    #[serde(default)]
    pub unknown: bool,
    /// Enumerations of a payload field's values (`source`, `reason`, …).
    #[serde(default)]
    pub vocab: std::collections::BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub note: Option<String>,
}

impl HookEvent {
    /// The matrix cell for a decision kind: `None` when the row lacks it.
    pub fn decision(&self, kind: &str) -> Option<Option<bool>> {
        self.decisions.get(kind).copied()
    }
    /// True when the event accepts a decision kind with its stated effect.
    pub fn accepts(&self, kind: &str) -> bool {
        self.decision(kind) == Some(Some(true))
    }
    /// `proven` / `plausible` / `unknown` — the evidence class of the payload
    /// (`counts.payload_*`).
    pub fn payload_class(&self) -> &'static str {
        if !self.payload.proven.is_empty() {
            "proven"
        } else if !self.payload.plausible.is_empty() {
            "plausible"
        } else {
            "unknown"
        }
    }
}

/// hook-events.json `protocol.plugin_hook_fields` — the closed field set of
/// a native plugin `capabilities.hooks[]` entry and the fields it rejects.
#[derive(Clone, Debug, Deserialize)]
pub struct PluginHookFields {
    pub closed_set: bool,
    pub fields: Vec<String>,
    pub rejected: Vec<String>,
    #[serde(default)]
    pub notes: String,
}

/// hook-events.json `protocol` — the parts omm keys on.
#[derive(Clone, Debug, Deserialize)]
pub struct HookProtocol {
    /// The exit-code rule: only [`HOOK_EXIT_BLOCK`] blocks, every other
    /// non-zero exit is fail-open (`verify_hook_events` holds it to say so).
    pub exit_codes: String,
    /// Inclusive stdout ceiling (16,384 → completed; 16,385 → `output_too_large`).
    pub stdout_ceiling_bytes: u64,
    /// The timeout rule: no default, `0` clamped to [`HOOK_TIMEOUT_MIN_SECS`],
    /// every shipped hook must set one.
    pub timeout: String,
    /// How a `command` is run: `$SHELL -c` (fallback [`HOOK_SHELL_FALLBACK`])
    /// with a [`HOOK_SHELL_ENV_KEYS`]-key scrubbed env for the config tiers,
    /// an argv array for native plugin hooks, PowerShell on Windows.
    pub shell: String,
    #[serde(default)]
    pub plugin_env: Vec<String>,
    /// The four tiers in composition order, one line each, first word =
    /// [`HOOK_TIERS`].
    pub tiers: Vec<String>,
    pub plugin_hook_fields: PluginHookFields,
    /// The fields a hooks.json-tier (managed/user/project) handler accepts —
    /// the only place `commandWindows` exists.
    pub config_hook_fields: Vec<String>,
}

/// hook-events.json.
#[derive(Clone, Debug, Deserialize)]
pub struct HookEvents {
    pub counts: HookEventCounts,
    pub case_rule: String,
    /// The eight kinds of [`HOOK_DECISION_KINDS`], each with its wire shape.
    pub decision_kinds: std::collections::BTreeMap<String, String>,
    pub items: Vec<HookEvent>,
    pub protocol: HookProtocol,
}

impl HookEvents {
    /// The row of an event by its PascalCase name.
    pub fn event(&self, name: &str) -> Option<&HookEvent> {
        self.items.iter().find(|e| e.name == name)
    }
    /// The events that accept a decision kind (`Some(true)` in the matrix),
    /// in trace order.
    pub fn events_accepting(&self, kind: &str) -> Vec<&str> {
        self.items
            .iter()
            .filter(|e| e.accepts(kind))
            .map(|e| e.name.as_str())
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct HookEventCounts {
    pub events: usize,
    /// Events observed firing in a `muse exec --provider echo hi` session.
    pub fire_in_echo_session: usize,
    /// Events by payload evidence class ([`HookEvent::payload_class`]).
    pub payload_proven: usize,
    pub payload_plausible: usize,
    pub payload_unknown: usize,
}

data_accessor!(
    /// The 17 PascalCase hook events.
    hook_events, HookEvents, RAW_HOOK_EVENTS, "hook-events.json"
);

/// One top-level command (muse-cli.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct CliCommand {
    pub name: String,
    pub advertised: bool,
    /// The gate the command mentions, if any — four commands carry one.
    pub gate: Option<String>,
    /// True when the argv is REJECTED (exit 2) without the gate — `plugins`
    /// only; `schema`/`serve` sit behind a default-ON gate and
    /// `session-message` only gates its `list` verb (exit 1). `counts.gated`
    /// counts these.
    #[serde(default)]
    pub gate_hard: bool,
    pub hidden: bool,
    pub summary: String,
}

/// muse-cli.json.
#[derive(Clone, Debug, Deserialize)]
pub struct MuseCli {
    pub version_string: String,
    pub counts: CliCounts,
    /// What `counts.gated` means (see [`CliCommand::gate_hard`]).
    #[serde(default)]
    pub gated_rule: String,
    /// `{"0": …, "1": …, "2": …, "125": …}` — cross-checked against the
    /// `EXIT_*` constants.
    pub exit_codes: std::collections::BTreeMap<String, String>,
    pub argv_allowlist: Vec<String>,
    /// Commands that are root-parser positionals and may therefore follow root
    /// flags (`muse [OPTIONS] [resume <session-uuid>]`); every other command
    /// after a root flag is refused by the host (`root_flag_walk_rule`).
    #[serde(default)]
    pub root_positional_commands: Vec<String>,
    pub items: Vec<CliCommand>,
    /// The 30 root/TUI flags with their value arity.
    pub root_flags: RootFlags,
    /// The verb tables of `skills` and `plugins` (the two families omm
    /// drives) and the `exec` flag list, cross-checked against
    /// `counts.skills_verbs` / `counts.plugins_verbs` and the verbs the
    /// probes call ([`HOST_VERBS_OMM_CALLS`]).
    pub verbs: CliVerbs,
    /// The `__tbh_internal_*` argv modes: never a product surface; one of
    /// them hangs and is killed with [`EXIT_PTY_GATE_TIMEOUT`].
    pub hidden_argv_modes: Vec<String>,
    /// `<ENV_VAR> must be one of …` — the env enums validated at process
    /// start for every subcommand; a bad value bricks even `--version`
    /// (cli-surface.md §4.2). Never passed through by omm's clean invocation
    /// environment; [`MuseCli::process_start_validated_env`] names them.
    pub process_start_validators: Vec<String>,
}

impl MuseCli {
    /// The env variables validated at process start, from
    /// `process_start_validators` (the first token of each rule).
    pub fn process_start_validated_env(&self) -> Vec<&str> {
        self.process_start_validators
            .iter()
            .filter_map(|rule| rule.split_whitespace().next())
            .collect()
    }
    /// The root-flag row for a spelling (`-w`, `--worktree`), if any.
    pub fn root_flag(&self, spelling: &str) -> Option<&RootFlagItem> {
        self.root_flags
            .items
            .iter()
            .find(|f| f.spellings().contains(&spelling))
    }
    /// Whether `token` may follow root flags as the root positional command.
    pub fn is_root_positional_command(&self, token: &str) -> bool {
        self.root_positional_commands.iter().any(|c| c == token)
    }
    /// The spellings of the host-internal root flags
    /// (`root_flags.internal_root_flags`, first token of each entry).
    pub fn internal_root_flags(&self) -> Vec<&str> {
        self.root_flags
            .items_internal()
            .into_iter()
            .filter(|f| f.starts_with("--"))
            .collect()
    }
    /// True for a host-internal root flag (`--internal-…`): a bare switch to
    /// the argv walker, never listed by `--help`, not a product surface.
    pub fn is_internal_root_flag(&self, spelling: &str) -> bool {
        self.internal_root_flags().contains(&spelling)
    }
    /// The arity of any root flag spelling the walker may meet: a listed
    /// flag's own, [`RootFlagArity::None`] for an internal one, `None` for a
    /// spelling the host does not know (its parse error, exit 2).
    pub fn root_flag_arity(&self, spelling: &str) -> Option<RootFlagArity> {
        self.root_flag(spelling)
            .map(RootFlagItem::arity)
            .or_else(|| {
                self.is_internal_root_flag(spelling)
                    .then_some(RootFlagArity::None)
            })
    }
    /// The verb table of a top-level command, for the two families listed.
    pub fn verb_family(&self, command: &str) -> Option<&VerbFamily> {
        match command {
            "skills" => Some(&self.verbs.skills),
            "plugins" => Some(&self.verbs.plugins),
            _ => None,
        }
    }
    /// The row of `<command> <verb>` in the verb tables.
    pub fn verb(&self, command: &str, verb: &str) -> Option<&VerbItem> {
        self.verb_family(command)?
            .items
            .iter()
            .find(|v| v.verb == verb)
    }
}

/// muse-cli.json `verbs` — the second-level surfaces omm drives.
#[derive(Clone, Debug, Deserialize)]
pub struct CliVerbs {
    pub skills: VerbFamily,
    pub plugins: VerbFamily,
    pub exec: ExecVerb,
}

/// One verb family (`verbs.skills`, `verbs.plugins`).
#[derive(Clone, Debug, Deserialize)]
pub struct VerbFamily {
    /// Cross-checked against `items.len()` and `counts.<family>_verbs`.
    pub count: usize,
    /// `plugins` only: the install-time gate ([`ENV_PLUGINS_GATE`]).
    #[serde(default)]
    pub gate: Option<String>,
    /// One `muse <family> <verb> …` synopsis per form (`install` has two,
    /// `marketplace` one per subverb).
    pub usage: Vec<String>,
    pub items: Vec<VerbItem>,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub selector_grammar: std::collections::BTreeMap<String, String>,
}

impl VerbFamily {
    /// How many `usage` lines the items account for: one per item, or one
    /// per `forms` entry / `subverbs` key where present.
    pub fn usage_lines_expected(&self) -> usize {
        self.items.iter().map(VerbItem::usage_lines).sum()
    }
}

/// One verb row (`verbs.<family>.items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct VerbItem {
    pub verb: String,
    #[serde(default)]
    pub subverb: Option<String>,
    #[serde(default)]
    pub subverbs: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub forms: Vec<String>,
    #[serde(default)]
    pub positional: Option<String>,
    #[serde(default)]
    pub flags: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

impl VerbItem {
    /// See [`VerbFamily::usage_lines_expected`].
    pub fn usage_lines(&self) -> usize {
        if !self.forms.is_empty() {
            self.forms.len()
        } else if !self.subverbs.is_empty() {
            self.subverbs.len()
        } else {
            1
        }
    }
}

/// muse-cli.json `verbs.exec` — the flag lists of the lane every probe
/// session runs through.
#[derive(Clone, Debug, Deserialize)]
pub struct ExecVerb {
    pub documented_flags: Vec<String>,
    #[serde(default)]
    pub undocumented_flags: Vec<String>,
    /// The probe invocation, `muse exec --provider echo [--trust-workspace] hi`.
    pub omm_probe_invocation: String,
}

impl ExecVerb {
    /// True when `flag` (e.g. `--provider`) heads a documented `exec` flag row
    /// (`--image (repeatable)`, `-w/--worktree` count by any spelling).
    pub fn documents_flag(&self, flag: &str) -> bool {
        self.documented_flags.iter().any(|row| {
            row.split_whitespace()
                .next()
                .map(|head| head.split('/').any(|s| s == flag))
                .unwrap_or(false)
        })
    }
}

/// `(<command>, <verb>)` pairs omm's probes and installer run
/// (`probe::*`, ARCHITECTURE.md §7 install/update/uninstall, R13); each must
/// be a row of muse-cli.json `verbs` so a verb the host drops is caught by
/// the data cross-check before a probe meets `unknown … command`.
pub const HOST_VERBS_OMM_CALLS: [(&str, &str); 12] = [
    ("skills", "list"),
    ("skills", "validate"),
    ("skills", "install"),
    ("skills", "uninstall"),
    ("plugins", "validate"),
    ("plugins", "install"),
    ("plugins", "inspect"),
    ("plugins", "approve"),
    ("plugins", "remove"),
    ("plugins", "enable"),
    ("plugins", "marketplace"),
    ("plugins", "list"),
];

/// muse-cli.json `root_flags`.
#[derive(Clone, Debug, Deserialize)]
pub struct RootFlags {
    pub count: usize,
    pub items: Vec<RootFlagItem>,
    /// `--internal-…` root flags the binary parses but never advertises
    /// (one starts a stdio JSON-RPC sidecar, one execs a foreign binary);
    /// each entry is `<spelling> (<what it does>)`. Bare switches to the
    /// argv walker ([`MuseCli::root_flag_arity`]).
    #[serde(default)]
    pub internal_root_flags: Vec<String>,
}

impl RootFlags {
    /// The spellings of `internal_root_flags` (first token of each entry).
    pub fn items_internal(&self) -> Vec<&str> {
        self.internal_root_flags
            .iter()
            .filter_map(|row| row.split_whitespace().next())
            .collect()
    }
}

/// One root flag (muse-cli.json `root_flags.items[]`): `flag` is the help
/// spelling, e.g. `-w, --worktree [<MODE>]`, whose `<VALUE>` / `[<VALUE>]`
/// suffix names the arity.
#[derive(Clone, Debug, Deserialize)]
pub struct RootFlagItem {
    pub flag: String,
    #[serde(default)]
    pub values: Option<String>,
    #[serde(default, rename = "default")]
    pub default_value: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// `value → build` for values that appeared after the first observed
    /// build (`--reasoning-effort max` since 1.0.3-R2198.1); the same
    /// OLDER-BUILD rule as gates.json `since` (hostcheck
    /// `cli/reasoning-effort/<value>`).
    #[serde(default)]
    pub values_since: std::collections::BTreeMap<String, String>,
}

/// How many argv tokens a root flag consumes after itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootFlagArity {
    /// A bare switch (`--yolo`, `-V`).
    None,
    /// Exactly one value (`--provider <MODE>`); `--flag=value` also works.
    One,
    /// An optional value (`-w, --worktree [<MODE>]`): the next token is taken
    /// only when it is one of the flag's listed `values` (`-w off`; a bare
    /// `-w` means `create`). Any other next token is the `[PROMPT]`: `muse -w
    /// hi` starts the TUI and submits `hi` (measured 2026-09-02 in a pty),
    /// while an attached `-w=hi` is exit 2 (`invalid value 'hi' for
    /// '--worktree'`) with no session.
    Optional,
}

impl RootFlagItem {
    /// Every spelling of the flag: `-w, --worktree [<MODE>]` → `["-w", "--worktree"]`.
    pub fn spellings(&self) -> Vec<&str> {
        self.flag
            .split(", ")
            .filter_map(|part| part.split_whitespace().next())
            .filter(|s| s.starts_with('-'))
            .collect()
    }
    /// The values the host accepts for this flag (`values: "off|create|existing"`),
    /// empty when the row lists none.
    pub fn accepted_values(&self) -> Vec<&str> {
        self.values
            .as_deref()
            .map(|v| {
                v.split('|')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Whether the host would take `token` as this flag's detached value: a
    /// `<VALUE>` flag takes anything, a `[<VALUE>]` flag only one of its
    /// listed values (case-sensitive), a switch nothing. The measured rule of
    /// `crate::allowlist::validate_argv`.
    pub fn accepts_value(&self, token: &str) -> bool {
        match self.arity() {
            RootFlagArity::None => false,
            RootFlagArity::One => true,
            RootFlagArity::Optional => self.accepted_values().contains(&token),
        }
    }
    /// The arity named by the `<VALUE>` / `[<VALUE>]` suffix.
    pub fn arity(&self) -> RootFlagArity {
        if self.flag.contains("[<") {
            RootFlagArity::Optional
        } else if self.flag.contains('<') {
            RootFlagArity::One
        } else {
            RootFlagArity::None
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct CliCounts {
    pub top_level_commands: usize,
    pub advertised: usize,
    pub gated: usize,
    pub hidden: usize,
    pub root_flags: usize,
    pub skills_verbs: usize,
    pub plugins_verbs: usize,
}

data_accessor!(
    /// The CLI surface: the 16-command allowlist and the exit-code rules.
    muse_cli, MuseCli, RAW_MUSE_CLI, "muse-cli.json"
);

/// One reserved plugin id (reserved-ids.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct ReservedId {
    pub id: String,
    pub kind: String,
}

/// One reserved marketplace name (reserved-ids.json `reserved_marketplace_names[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct ReservedMarketplaceName {
    pub name: String,
    #[serde(default)]
    pub behaviour: String,
}

/// reserved-ids.json.
#[derive(Clone, Debug, Deserialize)]
pub struct ReservedIds {
    pub counts: ReservedCounts,
    pub id_grammar: IdGrammar,
    pub items: Vec<ReservedId>,
    /// `tbh-curated`: `marketplace add`/`remove` refused (the lint contract).
    pub reserved_marketplace_names: Vec<ReservedMarketplaceName>,
    pub bundled_skill_ids: Vec<String>,
    pub windows_reserved_stems: IdList,
    pub builtin_tool_names: IdList,
    /// `tui.keymap` route characters, rejected for `navigation.focus-plan`.
    pub reserved_key_bindings: ReservedKeyBindings,
    /// Envelope tags a third-party reminder cannot use headlessly.
    pub reserved_reminder_envelope_tags: ReservedReminderTags,
    pub omm_identity: OmmIdentity,
}

impl ReservedIds {
    /// True when `name` cannot be used as a marketplace name.
    pub fn is_reserved_marketplace_name(&self, name: &str) -> bool {
        self.reserved_marketplace_names
            .iter()
            .any(|m| m.name == name)
    }
    /// True when `id` is one of the reserved plugin ids.
    pub fn is_reserved_plugin_id(&self, id: &str) -> bool {
        self.items.iter().any(|r| r.id == id)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReservedCounts {
    pub reserved_plugin_ids: usize,
    pub reserved_marketplace_names: usize,
    pub windows_reserved_stems: usize,
    pub builtin_tool_names: usize,
}

/// reserved-ids.json `id_grammar` — one grammar per id kind the lint checks.
#[derive(Clone, Debug, Deserialize)]
pub struct IdGrammar {
    pub plugin_and_capability_ids: String,
    pub skill_authoring_id: String,
    pub agent_definition_name: String,
    pub workflow_name: String,
    pub marketplace_name: String,
    #[serde(default)]
    pub selected_skill_ids: String,
}

/// reserved-ids.json `reserved_key_bindings`.
#[derive(Clone, Debug, Deserialize)]
pub struct ReservedKeyBindings {
    #[serde(default)]
    pub note: String,
    pub chars: Vec<String>,
}

/// reserved-ids.json `reserved_reminder_envelope_tags`.
#[derive(Clone, Debug, Deserialize)]
pub struct ReservedReminderTags {
    #[serde(default)]
    pub note: String,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct IdList {
    pub ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OmmIdentity {
    pub plugin_id: String,
    pub constraint: String,
}

data_accessor!(
    /// Reserved plugin ids, bundled skill ids, built-in tool names, omm's identity.
    reserved_ids, ReservedIds, RAW_RESERVED_IDS, "reserved-ids.json"
);

/// One typed settings key (settings-keys.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct SettingsKey {
    pub n: u32,
    pub key: String,
    #[serde(rename = "type")]
    pub type_desc: String,
    pub required: bool,
    /// The host's default (`null` when it has none) — required on every row.
    #[serde(rename = "default")]
    pub default_value: serde_json::Value,
    /// Not typed at load: a `lazy_raw_value_keys` member (RawValue, degrades
    /// only its subsystem) or the `separate_pass_keys` member (`mcpServers`).
    /// `verify_data` holds this equal to membership in those two lists.
    pub lazy: bool,
    /// Nested typed fields where the key is a struct (`run`, `tui`, …), as
    /// `field → type` prose.
    #[serde(default)]
    pub fields: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// The key's object is a `deny_unknown_fields` struct with required
    /// structural members (`permissions.schema_version`): a leaf restore
    /// that would leave the object without one while other members remain
    /// must keep it, or the host refuses the whole object (Gate 1 decision
    /// D). `None` for every other key.
    #[serde(default)]
    pub structural: Option<Structural>,
}

/// settings-keys.json `items[].structural`.
#[derive(Clone, Debug, Deserialize)]
pub struct Structural {
    pub deny_unknown_fields: bool,
    /// Member names the host requires whenever the object is present.
    pub required: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// settings-keys.json.
#[derive(Clone, Debug, Deserialize)]
pub struct SettingsKeys {
    pub counts: SettingsCounts,
    pub lazy_raw_value_keys: Vec<String>,
    pub separate_pass_keys: Vec<String>,
    pub items: Vec<SettingsKey>,
    pub legacy_spellings: Vec<LegacySpelling>,
    pub tui_fields: FieldList,
    pub mcp_server_fields: FieldList,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SettingsCounts {
    pub top_level_keys: usize,
    /// `top_level_keys − lazy_raw_value − separate_pass`.
    pub typed_at_load: usize,
    pub lazy_raw_value: usize,
    pub separate_pass: usize,
    pub tui_fields: usize,
    pub mcp_server_fields: usize,
}

#[derive(Clone, Debug, Deserialize)]
pub struct LegacySpelling {
    pub key: String,
    pub same_as: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct FieldList {
    pub count: usize,
    pub items: Vec<Field>,
    /// `mcp_server_fields` only: the validator literals that exist in the
    /// binary but are unreachable from a user settings load.
    #[serde(default)]
    pub validation_literals: Vec<String>,
}

/// One nested field row (`tui_fields.items[]`, `mcp_server_fields.items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct Field {
    pub key: String,
    #[serde(rename = "type")]
    pub type_desc: String,
    /// Present on every `tui` row (`null` when the host has no default),
    /// absent on the `mcpServers` rows; `verify_data` holds the former.
    #[serde(default, rename = "default", deserialize_with = "present")]
    pub default_value: Option<serde_json::Value>,
}

/// `Option<Value>` that is `Some(Null)` for an explicit `null` and `None`
/// only when the key is absent.
fn present<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> std::result::Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(d).map(Some)
}

impl SettingsKeys {
    /// True when `key` is one of the 29 typed top-level keys.
    pub fn is_known(&self, key: &str) -> bool {
        self.items.iter().any(|k| k.key == key)
    }
    /// True when a wrong type under `key` degrades only its subsystem instead
    /// of failing every settings-consuming command.
    pub fn is_lazy(&self, key: &str) -> bool {
        self.lazy_raw_value_keys.iter().any(|k| k == key)
            || self.separate_pass_keys.iter().any(|k| k == key)
    }
    /// The canonical spelling of a legacy key (`mcp_servers` → `mcpServers`).
    pub fn canonical_spelling(&self, key: &str) -> Option<&str> {
        self.legacy_spellings
            .iter()
            .find(|l| l.key == key)
            .map(|l| l.same_as.as_str())
    }
    /// The required structural members of the object under top-level
    /// `key` (`items[].structural.required`); empty for a key whose object
    /// has none. See [`Structural`].
    pub fn structural_required(&self, key: &str) -> &[String] {
        self.items
            .iter()
            .find(|k| k.key == key)
            .and_then(|k| k.structural.as_ref())
            .map(|s| s.required.as_slice())
            .unwrap_or(&[])
    }
}

data_accessor!(
    /// The 29 typed `settings.json` keys (R9).
    settings_keys, SettingsKeys, RAW_SETTINGS_KEYS, "settings-keys.json"
);

/// One slash command row (slash-commands.json `items[]`).
#[derive(Clone, Debug, Deserialize)]
pub struct SlashCommand {
    pub name: String,
    pub canonical: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    /// `builtin | bundled-skill-shortcut | bundled-plugin-command`; absent on `hidden_builtins[]`.
    #[serde(default)]
    pub kind: String,
    /// `bundled-skill-shortcut` rows: the `bundled:<id>` the row invokes.
    #[serde(default)]
    pub skill_id: Option<String>,
    /// `bundled-plugin-command` rows: the (reserved) plugin id that ships it.
    #[serde(default)]
    pub plugin_id: Option<String>,
}

/// slash-commands.json.
#[derive(Clone, Debug, Deserialize)]
pub struct SlashCommands {
    pub counts: SlashCounts,
    pub items: Vec<SlashCommand>,
    pub hidden_builtins: Vec<SlashCommand>,
    pub builtin_aliases: Vec<String>,
}

/// slash-commands.json `counts` — the 39 picker rows by kind, the binary's
/// own table (36 canonical + 9 aliases, 6 hidden) and the collision set.
#[derive(Clone, Debug, Deserialize)]
pub struct SlashCounts {
    pub items: usize,
    pub items_builtin: usize,
    pub items_bundled_skill_shortcut: usize,
    pub items_bundled_plugin_command: usize,
    pub items_builtin_plugin_skill: usize,
    pub builtin_canonical_total: usize,
    pub hidden_builtins: usize,
    pub builtin_aliases_total: usize,
    pub collision_names_total: usize,
}

/// The `kind` of a bundled-skill shortcut row (slash-commands.json `field_semantics.kind`).
pub const SLASH_KIND_BUILTIN: &str = "builtin";
/// See [`SLASH_KIND_BUILTIN`].
pub const SLASH_KIND_SKILL_SHORTCUT: &str = "bundled-skill-shortcut";
/// See [`SLASH_KIND_BUILTIN`].
pub const SLASH_KIND_PLUGIN_COMMAND: &str = "bundled-plugin-command";
/// See [`SLASH_KIND_BUILTIN`].
pub const SLASH_KIND_PLUGIN_SKILL: &str = "builtin-plugin-skill";

impl SlashCommands {
    /// Every name a plugin command or skill shortcut could collide with:
    /// the 39 rows, the hidden built-ins and the aliases (without the `/`).
    pub fn collision_names(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .items
            .iter()
            .chain(self.hidden_builtins.iter())
            .flat_map(|c| {
                std::iter::once(c.name.clone())
                    .chain(std::iter::once(c.canonical.clone()))
                    .chain(c.aliases.iter().cloned())
            })
            .chain(self.builtin_aliases.iter().cloned())
            .map(|n| n.trim_start_matches('/').to_string())
            .collect();
        out.sort();
        out.dedup();
        out
    }
}

data_accessor!(
    /// The 39 composer rows plus hidden built-ins and aliases.
    slash_commands, SlashCommands, RAW_SLASH_COMMANDS, "slash-commands.json"
);

/// crates/omm-host/data/enterprise-defaults-plane.json.
#[derive(Clone, Debug, Deserialize)]
pub struct EnterpriseDefaultsPlane {
    pub plane: String,
    pub accepted_members: Vec<String>,
    pub renames: std::collections::BTreeMap<String, String>,
    pub user_only_subkeys: std::collections::BTreeMap<String, Vec<String>>,
    pub accept_outcomes: Vec<String>,
    pub reject_outcomes: Vec<String>,
    pub blind_outcomes: Vec<String>,
}

data_accessor!(
    /// What `muse config validate --plane defaults` can see of a user settings document.
    enterprise_defaults_plane, EnterpriseDefaultsPlane, RAW_ENTERPRISE_DEFAULTS_PLANE,
    "enterprise-defaults-plane.json"
);

/// crates/omm-host/data/fixtures/reminder-decision.json.
#[derive(Clone, Debug, Deserialize)]
pub struct ReminderDecisionFixture {
    pub decision: serde_json::Value,
}

data_accessor!(
    /// A reminder `decision` block that validates `full` and approves headlessly.
    reminder_decision_fixture, ReminderDecisionFixture, RAW_REMINDER_DECISION_FIXTURE,
    "fixtures/reminder-decision.json"
);

/// Parse every compiled-in data file and cross-check its counts against the
/// constants above. Returns the list of discrepancies (empty = consistent).
pub fn verify_data() -> Result<Vec<String>> {
    let mut problems = Vec::new();
    fn check(problems: &mut Vec<String>, what: &str, expected: usize, observed: usize) {
        if expected != observed {
            problems.push(format!("{what}: constant {expected} vs data {observed}"));
        }
    }
    let b = bundled_skills()?;
    check(
        &mut problems,
        "bundled skills",
        BUNDLED_SKILLS,
        b.items.len(),
    );
    check(
        &mut problems,
        "bundled skills (counts)",
        BUNDLED_SKILLS,
        b.counts.skills,
    );
    check(
        &mut problems,
        "bundled skills visible by default",
        BUNDLED_SKILLS_VISIBLE_DEFAULT,
        b.counts.visible_by_default,
    );
    check(
        &mut problems,
        "bundled package files",
        BUNDLED_SKILL_PACKAGE_FILES,
        b.package_files.len(),
    );
    if b.catalog_cost.all_20_rendered_bytes != BUILTIN_SKILLS_CATALOG_BYTES {
        problems.push("built-in catalog bytes differ".to_string());
    }
    let g = gates()?;
    problems.extend(verify_gates(g));
    check(&mut problems, "gates", GATES_TOTAL, g.items.len());
    check(
        &mut problems,
        "gates default-on",
        GATES_DEFAULT_ON,
        g.default_on_ids.len(),
    );
    check(
        &mut problems,
        "gates default-on (items)",
        GATES_DEFAULT_ON,
        g.items.iter().filter(|x| x.default_on()).count(),
    );
    let h = hook_events()?;
    check(&mut problems, "hook events", HOOK_EVENTS, h.items.len());
    problems.extend(verify_hook_events(h));
    let c = muse_cli()?;
    problems.extend(verify_muse_cli(c));
    check(
        &mut problems,
        "top-level commands",
        TOP_LEVEL_COMMANDS,
        c.argv_allowlist.len(),
    );
    check(
        &mut problems,
        "top-level commands (items)",
        TOP_LEVEL_COMMANDS,
        c.items.len(),
    );
    check(
        &mut problems,
        "advertised commands",
        ADVERTISED_COMMANDS,
        c.items.iter().filter(|x| x.advertised).count(),
    );
    check(
        &mut problems,
        "root flags",
        c.counts.root_flags,
        c.root_flags.items.len(),
    );
    check(
        &mut problems,
        "root flags (count)",
        c.root_flags.count,
        c.root_flags.items.len(),
    );
    if c.root_positional_commands.is_empty()
        || !c
            .root_positional_commands
            .iter()
            .all(|p| c.argv_allowlist.contains(p))
    {
        problems.push("root positional commands missing or not in the allowlist".to_string());
    }
    let r = reserved_ids()?;
    check(
        &mut problems,
        "reserved plugin ids",
        r.counts.reserved_plugin_ids,
        r.items.len(),
    );
    problems.extend(verify_reserved_ids(r));
    check(
        &mut problems,
        "bundled skill ids",
        BUNDLED_SKILLS,
        r.bundled_skill_ids.len(),
    );
    check(
        &mut problems,
        "built-in tool names",
        ACTIVE_TOOLS,
        r.builtin_tool_names.ids.len(),
    );
    if r.id_grammar
        .plugin_and_capability_ids
        .split_whitespace()
        .next()
        != Some(ID_GRAMMAR)
    {
        problems.push("id grammar differs".to_string());
    }
    let s = settings_keys()?;
    check(
        &mut problems,
        "settings keys",
        s.counts.top_level_keys,
        s.items.len(),
    );
    check(
        &mut problems,
        "tui fields",
        s.tui_fields.count,
        s.tui_fields.items.len(),
    );
    problems.extend(verify_settings_keys(s));
    let sl = slash_commands()?;
    check(
        &mut problems,
        "slash commands",
        BUILTIN_SLASH_COMMANDS,
        sl.items.len(),
    );
    problems.extend(verify_slash_commands(sl, b, r));
    problems.extend(verify_bundled_skills(b, r));
    let e = enterprise_defaults_plane()?;
    if e.accepted_members.len() != 20 {
        problems.push(format!(
            "enterprise defaults plane: 20 members expected, data has {}",
            e.accepted_members.len()
        ));
    }
    reminder_decision_fixture()?;
    Ok(problems)
}

/// gates.json `probe` against the literals the probe code keys on.
pub fn verify_gates(g: &Gates) -> Vec<String> {
    let mut problems = Vec::new();
    let argv = g.probe.argv();
    match argv.as_slice() {
        [verb, sub, id] if verb == "plugins" && sub == "enable" && id.starts_with("__") => {}
        other => problems.push(format!(
            "gates.json probe.command must be `plugins enable __<probe-id>__` after its env prefix and program, parsed {other:?}"
        )),
    }
    if !g.probe.line_pattern.starts_with(GATE_RESOLVE_EVENT) {
        problems.push(format!(
            "gates.json probe.line_pattern {:?} does not start with GATE_RESOLVE_EVENT {GATE_RESOLVE_EVENT:?}",
            g.probe.line_pattern
        ));
    }
    if g.probe.trace_subdir().as_deref() != Some(BOOTSTRAP_TRACE_SUBDIR) {
        problems.push(format!(
            "gates.json probe.trace_glob {:?} does not name the {BOOTSTRAP_TRACE_SUBDIR:?} dir under the data root",
            g.probe.trace_glob
        ));
    }
    if g.env_prefix != ENV_GATE_PREFIX {
        problems.push(format!(
            "gates.json env_prefix {:?} vs ENV_GATE_PREFIX {ENV_GATE_PREFIX:?}",
            g.env_prefix
        ));
    }
    if !g.accepted_values.on.iter().any(|v| v == GATE_ON_VALUE) || g.accepted_values.off.is_empty()
    {
        problems.push(format!(
            "gates.json accepted_values.on {:?} must contain GATE_ON_VALUE {GATE_ON_VALUE:?} and .off must not be empty",
            g.accepted_values.on
        ));
    }
    // counts vs items: default off / on + off = gates / cli_visible.
    let off = g.items.iter().filter(|x| !x.default_on()).count();
    if g.counts.default_off != off || g.counts.default_on + g.counts.default_off != g.counts.gates {
        problems.push(format!(
            "gates.json counts default_on {} + default_off {} vs gates {} (items default off: {off})",
            g.counts.default_on, g.counts.default_off, g.counts.gates
        ));
    }
    let visible = g.items.iter().filter(|x| x.cli_visible).count();
    if g.counts.cli_visible != visible {
        problems.push(format!(
            "gates.json counts.cli_visible {} vs items {visible}",
            g.counts.cli_visible
        ));
    }
    // default_on_ids is exactly the set of items with default "on".
    let on_items: std::collections::BTreeSet<&str> = g
        .items
        .iter()
        .filter(|x| x.default_on())
        .map(|x| x.id.as_str())
        .collect();
    let on_list: std::collections::BTreeSet<&str> =
        g.default_on_ids.iter().map(String::as_str).collect();
    if on_items != on_list {
        problems.push(format!(
            "gates.json default_on_ids differ from items with default on: {:?}",
            on_items.symmetric_difference(&on_list).collect::<Vec<_>>()
        ));
    }
    // env = prefix + ID, in registry order 1..=N, no duplicate ids.
    let mut ids = std::collections::BTreeSet::new();
    for (i, gate) in g.items.iter().enumerate() {
        let expected_env = format!("{}{}", g.env_prefix, gate.id.to_ascii_uppercase());
        if gate.env != expected_env {
            problems.push(format!(
                "gates.json `{}` env {:?} is not {expected_env:?}",
                gate.id, gate.env
            ));
        }
        if gate.n as usize != i + 1 {
            problems.push(format!(
                "gates.json `{}` is n={} at position {}",
                gate.id,
                gate.n,
                i + 1
            ));
        }
        if !ids.insert(gate.id.as_str()) {
            problems.push(format!("gates.json duplicate gate id `{}`", gate.id));
        }
        if let Some(since) = &gate.since {
            if !is_build_version(since) {
                problems.push(format!(
                    "gates.json `{}` since {since:?} is not a Muse build version (e.g. 1.0.3-R2198.1)",
                    gate.id
                ));
            }
        }
        if let Some(probe) = &gate.effect_probe {
            if probe.kind != GATE_EFFECT_PROBE_SETTINGS_ECHO_SESSION {
                problems.push(format!(
                    "gates.json `{}` effect_probe.kind {:?} is not {GATE_EFFECT_PROBE_SETTINGS_ECHO_SESSION:?}",
                    gate.id, probe.kind
                ));
            }
            if !probe.settings.is_object() {
                problems.push(format!(
                    "gates.json `{}` effect_probe.settings is not an object",
                    gate.id
                ));
            }
            for (name, exp) in [
                ("closed", &probe.closed),
                ("open", &probe.open),
                ("absent", &probe.absent),
            ] {
                if exp.context_orders.is_empty()
                    || exp.context_orders.windows(2).any(|w| w[0] >= w[1])
                {
                    problems.push(format!(
                        "gates.json `{}` effect_probe.{name}.context_orders must be ascending and non-empty",
                        gate.id
                    ));
                }
            }
            if probe.closed.stderr_prefix.is_none() {
                problems.push(format!(
                    "gates.json `{}` effect_probe.closed needs a stderr_prefix (the line that proves the gate is what withheld the effect)",
                    gate.id
                ));
            }
        }
    }
    // The gate variables omm sets are gates the data knows.
    for env in [ENV_PLUGINS_GATE, ENV_ROUTING_GATE, ENV_ROUTING_APPLY_GATE] {
        if !g.items.iter().any(|x| x.env == env) {
            problems.push(format!(
                "gates.json lists no gate for {env} (an ENV_* constant)"
            ));
        }
    }
    problems
}

/// bundled-skills.json against itself and reserved-ids.json: ids, paths,
/// package bytes, the gate, the slash shortcuts and the `muse-core` id.
pub fn verify_bundled_skills(b: &BundledSkills, r: &ReservedIds) -> Vec<String> {
    let mut problems = Vec::new();
    let gated = b.items.iter().filter(|s| s.gated_by.is_some()).count();
    if b.counts.gated != gated || b.counts.visible_by_default + gated != b.counts.skills {
        problems.push(format!(
            "bundled-skills.json counts gated {} / visible_by_default {} vs skills {} (items gated: {gated})",
            b.counts.gated, b.counts.visible_by_default, b.counts.skills
        ));
    }
    if b.counts.package_files != b.package_files.len() {
        problems.push(format!(
            "bundled-skills.json counts.package_files {} vs package_files {}",
            b.counts.package_files,
            b.package_files.len()
        ));
    }
    for (i, s) in b.items.iter().enumerate() {
        if s.manifest_index as usize != i {
            problems.push(format!(
                "bundled-skills.json `{}` manifest_index {} at position {i}",
                s.id, s.manifest_index
            ));
        }
        if s.qualified_id != format!("{}{}", b.id_prefix, s.id) {
            problems.push(format!(
                "bundled-skills.json `{}` qualified_id {:?} is not id_prefix + id",
                s.id, s.qualified_id
            ));
        }
        let path = format!("skills/{}/SKILL.md", s.id);
        if s.path != path {
            problems.push(format!(
                "bundled-skills.json `{}` path {:?} is not {path:?}",
                s.id, s.path
            ));
        }
        match b.package_files.iter().find(|f| f.path == s.path) {
            Some(f) if f.bytes == s.skill_md_bytes => {}
            Some(f) => problems.push(format!(
                "bundled-skills.json `{}` skill_md_bytes {} vs package_files {}",
                s.id, s.skill_md_bytes, f.bytes
            )),
            None => problems.push(format!(
                "bundled-skills.json package_files lacks {}",
                s.path
            )),
        }
        if let Some(gate) = &s.gated_by {
            if gate != ENV_PLUGINS_GATE {
                problems.push(format!(
                    "bundled-skills.json `{}` gated_by {gate:?} is not ENV_PLUGINS_GATE",
                    s.id
                ));
            }
        }
        if s.slash_shortcut == Some(true) && !(s.user_invocable && s.gated_by.is_none()) {
            problems.push(format!(
                "bundled-skills.json `{}` has a slash shortcut but is not user-invocable and ungated",
                s.id
            ));
        }
        if s.slash_shortcut == Some(false) && s.user_invocable && s.gated_by.is_none() {
            problems.push(format!(
                "bundled-skills.json `{}` is user-invocable and ungated but has no slash shortcut",
                s.id
            ));
        }
    }
    let shortcuts = b
        .items
        .iter()
        .filter(|s| s.slash_shortcut == Some(true))
        .count();
    if b.counts.slash_shortcuts != shortcuts {
        problems.push(format!(
            "bundled-skills.json counts.slash_shortcuts {} vs items {shortcuts}",
            b.counts.slash_shortcuts
        ));
    }
    if !r.is_reserved_plugin_id(&b.plugin_id) {
        problems.push(format!(
            "bundled-skills.json plugin_id {:?} is not a reserved plugin id",
            b.plugin_id
        ));
    }
    let ids: std::collections::BTreeSet<&str> = b.items.iter().map(|s| s.id.as_str()).collect();
    let reserved: std::collections::BTreeSet<&str> =
        r.bundled_skill_ids.iter().map(String::as_str).collect();
    if ids != reserved {
        problems.push(format!(
            "reserved-ids.json bundled_skill_ids differ from bundled-skills.json items: {:?}",
            ids.symmetric_difference(&reserved).collect::<Vec<_>>()
        ));
    }
    problems
}

/// slash-commands.json against its counts, bundled-skills.json (the seven
/// `/<id>` shortcut rows) and reserved-ids.json (the plugin command's id).
pub fn verify_slash_commands(
    sl: &SlashCommands,
    b: &BundledSkills,
    r: &ReservedIds,
) -> Vec<String> {
    let mut problems = Vec::new();
    let c = &sl.counts;
    let by_kind = |kind: &str| sl.items.iter().filter(|i| i.kind == kind).count();
    let (builtin, shortcut, plugin_cmd, plugin_skill) = (
        by_kind(SLASH_KIND_BUILTIN),
        by_kind(SLASH_KIND_SKILL_SHORTCUT),
        by_kind(SLASH_KIND_PLUGIN_COMMAND),
        by_kind(SLASH_KIND_PLUGIN_SKILL),
    );
    if c.items_builtin
        + c.items_bundled_skill_shortcut
        + c.items_bundled_plugin_command
        + c.items_builtin_plugin_skill
        != c.items
        || builtin != c.items_builtin
        || shortcut != c.items_bundled_skill_shortcut
        || plugin_cmd != c.items_bundled_plugin_command
        || plugin_skill != c.items_builtin_plugin_skill
        || builtin + shortcut + plugin_cmd + plugin_skill != sl.items.len()
    {
        problems.push(format!(
            "slash-commands.json counts by kind {}/{}/{}/{} = {} vs items {builtin}/{shortcut}/{plugin_cmd}/{plugin_skill} of {}",
            c.items_builtin,
            c.items_bundled_skill_shortcut,
            c.items_bundled_plugin_command,
            c.items_builtin_plugin_skill,
            c.items,
            sl.items.len()
        ));
    }
    if c.hidden_builtins != sl.hidden_builtins.len() {
        problems.push(format!(
            "slash-commands.json counts.hidden_builtins {} vs list {}",
            c.hidden_builtins,
            sl.hidden_builtins.len()
        ));
    }
    let alias_union: std::collections::BTreeSet<&str> = sl
        .items
        .iter()
        .chain(sl.hidden_builtins.iter())
        .flat_map(|i| i.aliases.iter().map(String::as_str))
        .collect();
    let alias_list: std::collections::BTreeSet<&str> =
        sl.builtin_aliases.iter().map(String::as_str).collect();
    if c.builtin_aliases_total != sl.builtin_aliases.len() || alias_union != alias_list {
        problems.push(format!(
            "slash-commands.json builtin_aliases ({}, counts {}) differ from the aliases of items + hidden_builtins: {:?}",
            sl.builtin_aliases.len(),
            c.builtin_aliases_total,
            alias_union.symmetric_difference(&alias_list).collect::<Vec<_>>()
        ));
    }
    let canonicals: std::collections::BTreeSet<&str> = sl
        .items
        .iter()
        .filter(|i| i.kind == SLASH_KIND_BUILTIN)
        .chain(sl.hidden_builtins.iter())
        .map(|i| i.canonical.as_str())
        .collect();
    if c.builtin_canonical_total != canonicals.len() {
        problems.push(format!(
            "slash-commands.json counts.builtin_canonical_total {} vs distinct canonicals of builtin rows + hidden {}",
            c.builtin_canonical_total,
            canonicals.len()
        ));
    }
    let collisions = sl.collision_names().len();
    if c.collision_names_total != collisions {
        problems.push(format!(
            "slash-commands.json counts.collision_names_total {} vs collision_names() {collisions}",
            c.collision_names_total
        ));
    }
    // Shortcut rows ↔ bundled skills with a shortcut, by name and skill_id.
    for row in sl
        .items
        .iter()
        .filter(|i| i.kind == SLASH_KIND_SKILL_SHORTCUT)
    {
        let id = row.name.trim_start_matches('/');
        let skill = b.items.iter().find(|s| s.id == id);
        match skill {
            Some(s) if s.slash_shortcut == Some(true) => {}
            Some(_) => problems.push(format!(
                "slash-commands.json {} is a shortcut row but bundled-skills.json `{id}` has no shortcut",
                row.name
            )),
            None => problems.push(format!(
                "slash-commands.json {} names no bundled skill",
                row.name
            )),
        }
        let expected = Some(format!("{}{id}", b.id_prefix));
        if row.skill_id != expected {
            problems.push(format!(
                "slash-commands.json {} skill_id {:?} is not {expected:?}",
                row.name, row.skill_id
            ));
        }
    }
    for s in b.items.iter().filter(|s| s.slash_shortcut == Some(true)) {
        let name = format!("/{}", s.id);
        if !sl
            .items
            .iter()
            .any(|i| i.kind == SLASH_KIND_SKILL_SHORTCUT && i.name == name)
        {
            problems.push(format!(
                "bundled-skills.json `{}` has a shortcut but slash-commands.json has no {name} row",
                s.id
            ));
        }
    }
    if shortcut != b.counts.slash_shortcuts {
        problems.push(format!(
            "slash-commands.json shortcut rows {shortcut} vs bundled-skills.json counts.slash_shortcuts {}",
            b.counts.slash_shortcuts
        ));
    }
    for row in sl
        .items
        .iter()
        .filter(|i| i.kind == SLASH_KIND_PLUGIN_COMMAND)
    {
        match &row.plugin_id {
            Some(pid) if r.is_reserved_plugin_id(pid) => {}
            other => problems.push(format!(
                "slash-commands.json {} plugin_id {other:?} is not a reserved plugin id",
                row.name
            )),
        }
    }
    problems
}

/// The bootstrap trace directory under the data root
/// (`Roots::bootstrap_trace_dir`, gates.json `probe.trace_glob`).
pub const BOOTSTRAP_TRACE_SUBDIR: &str = "local-tracing/bootstrap";

/// hook-events.json `protocol` against the constants.
pub fn verify_hook_events(h: &HookEvents) -> Vec<String> {
    let mut problems = Vec::new();
    if h.protocol.stdout_ceiling_bytes != HOOK_STDOUT_MAX_BYTES {
        problems.push(format!(
            "hook stdout ceiling: constant {HOOK_STDOUT_MAX_BYTES} vs data {}",
            h.protocol.stdout_ceiling_bytes
        ));
    }
    if h.counts.events != h.items.len() {
        problems.push(format!(
            "hook events (counts): {} vs items {}",
            h.counts.events,
            h.items.len()
        ));
    }
    let needle = format!("timeout ≥ {HOOK_TIMEOUT_MIN_SECS}");
    if !h.protocol.timeout.contains(&needle) {
        problems.push(format!(
            "hook-events.json protocol.timeout {:?} does not state `{needle}` (HOOK_TIMEOUT_MIN_SECS)",
            h.protocol.timeout
        ));
    }
    let phf = &h.protocol.plugin_hook_fields;
    if !phf.closed_set {
        problems.push(
            "hook-events.json plugin_hook_fields.closed_set must be true (the hooks family is CLOSED)"
                .to_string(),
        );
    }
    let leaked: Vec<&String> = phf
        .fields
        .iter()
        .filter(|f| phf.rejected.contains(f) || PLUGIN_HOOK_REJECTED_FIELDS.contains(&f.as_str()))
        .collect();
    if !leaked.is_empty() {
        problems.push(format!(
            "hook-events.json plugin_hook_fields.fields lists rejected fields {leaked:?} (PLUGIN_HOOK_REJECTED_FIELDS / .rejected)"
        ));
    }
    let required: Vec<&str> = ["command", "timeout"]
        .into_iter()
        .chain(PLUGIN_HOOK_REJECTED_FIELDS)
        .collect();
    let missing: Vec<&str> = required
        .into_iter()
        .filter(|r| !h.protocol.config_hook_fields.iter().any(|f| f == r))
        .collect();
    if !missing.is_empty() {
        problems.push(format!(
            "hook-events.json config_hook_fields lacks {missing:?} (the hooks.json tiers carry `command`, `timeout` and the Windows twin)"
        ));
    }
    // counts: echo-session fires and the payload evidence classes.
    let fires = h.items.iter().filter(|e| e.fires_in_echo_session).count();
    if h.counts.fire_in_echo_session != fires {
        problems.push(format!(
            "hook-events.json counts.fire_in_echo_session {} vs items {fires}",
            h.counts.fire_in_echo_session
        ));
    }
    let class = |c: &str| h.items.iter().filter(|e| e.payload_class() == c).count();
    let (proven, plausible, unknown) = (class("proven"), class("plausible"), class("unknown"));
    if (
        h.counts.payload_proven,
        h.counts.payload_plausible,
        h.counts.payload_unknown,
    ) != (proven, plausible, unknown)
        || proven + plausible + unknown != h.items.len()
    {
        problems.push(format!(
            "hook-events.json counts.payload_* {}/{}/{} vs items {proven}/{plausible}/{unknown}",
            h.counts.payload_proven, h.counts.payload_plausible, h.counts.payload_unknown
        ));
    }
    for e in h.items.iter().filter(|e| e.payload.unknown) {
        if !e.payload.proven.is_empty() || !e.payload.plausible.is_empty() {
            problems.push(format!(
                "hook-events.json `{}` payload.unknown with proven/plausible fields",
                e.name
            ));
        }
    }
    // The decision matrix: every row carries exactly the eight kinds.
    let kinds: std::collections::BTreeSet<&str> = HOOK_DECISION_KINDS.into_iter().collect();
    let declared: std::collections::BTreeSet<&str> =
        h.decision_kinds.keys().map(String::as_str).collect();
    if declared != kinds {
        problems.push(format!(
            "hook-events.json decision_kinds {declared:?} vs HOOK_DECISION_KINDS {kinds:?}"
        ));
    }
    for e in &h.items {
        let row: std::collections::BTreeSet<&str> =
            e.decisions.keys().map(String::as_str).collect();
        if row != kinds {
            problems.push(format!(
                "hook-events.json `{}` decisions {:?} are not the eight HOOK_DECISION_KINDS",
                e.name,
                row.symmetric_difference(&kinds).collect::<Vec<_>>()
            ));
        }
    }
    // The routing and bare-stdout events the code keys on.
    for (kind, expected) in [
        ("skills_v1", &HOOK_SKILLS_V1_EVENTS[..]),
        ("bare_stdout_context", &HOOK_BARE_STDOUT_CONTEXT_EVENTS[..]),
    ] {
        let observed = h.events_accepting(kind);
        if observed != expected {
            problems.push(format!(
                "hook-events.json events accepting `{kind}` are {observed:?}, constant says {expected:?}"
            ));
        }
    }
    // protocol.exit_codes / tiers / shell against the constants.
    let block = format!("exit {HOOK_EXIT_BLOCK} blocks");
    if !h.protocol.exit_codes.contains(&block) || !h.protocol.exit_codes.contains("fail-open") {
        problems.push(format!(
            "hook-events.json protocol.exit_codes {:?} must state `{block}` (HOOK_EXIT_BLOCK) and `fail-open`",
            h.protocol.exit_codes
        ));
    }
    let tier_words: Vec<&str> = h
        .protocol
        .tiers
        .iter()
        .filter_map(|t| t.split_whitespace().next())
        .collect();
    if tier_words != HOOK_TIERS {
        problems.push(format!(
            "hook-events.json protocol.tiers {tier_words:?} vs HOOK_TIERS {HOOK_TIERS:?} (in composition order)"
        ));
    }
    if !h
        .protocol
        .tiers
        .first()
        .map(|t| t.contains(ENV_MANAGED_HOOKS_PATH))
        .unwrap_or(false)
    {
        problems.push(format!(
            "hook-events.json protocol.tiers[0] (managed) does not name {ENV_MANAGED_HOOKS_PATH}"
        ));
    }
    let shell = &h.protocol.shell;
    let env_keys = format!("{HOOK_SHELL_ENV_KEYS}-key");
    let shell_needles: Vec<&str> = ["$SHELL -c", HOOK_SHELL_FALLBACK, env_keys.as_str()]
        .into_iter()
        .chain(PLUGIN_HOOK_REJECTED_FIELDS)
        .filter(|n| !shell.contains(n))
        .collect();
    if !shell_needles.is_empty() {
        problems.push(format!(
            "hook-events.json protocol.shell lacks {shell_needles:?} (HOOK_SHELL_FALLBACK, HOOK_SHELL_ENV_KEYS, the Windows twins)"
        ));
    }
    problems
}

/// muse-cli.json `counts.gated`, `gate_hard` and `exit_codes` against the constants.
pub fn verify_muse_cli(c: &MuseCli) -> Vec<String> {
    let mut problems = Vec::new();
    let hard = c.items.iter().filter(|i| i.gate_hard).count();
    if hard != c.counts.gated {
        problems.push(format!(
            "muse-cli.json counts.gated {} vs items with gate_hard {hard}",
            c.counts.gated
        ));
    }
    for i in c.items.iter().filter(|i| i.gate_hard && i.gate.is_none()) {
        problems.push(format!(
            "muse-cli.json `{}` is gate_hard but names no gate",
            i.name
        ));
    }
    if c.gated_rule.is_empty() {
        problems.push("muse-cli.json gated_rule missing".to_string());
    }
    let expected: std::collections::BTreeSet<String> = [
        EXIT_OK,
        EXIT_RUN_FAILED,
        EXIT_ARGV_REJECTED,
        EXIT_PTY_GATE_TIMEOUT,
    ]
    .iter()
    .map(i32::to_string)
    .collect();
    let observed: std::collections::BTreeSet<String> = c.exit_codes.keys().cloned().collect();
    if expected != observed {
        problems.push(format!(
            "muse-cli.json exit_codes {observed:?} vs EXIT_* constants {expected:?}"
        ));
    }
    if c.counts.hidden != c.items.iter().filter(|i| i.hidden).count() {
        problems.push("muse-cli.json counts.hidden vs items".to_string());
    }
    let needle = format!("exit {EXIT_PTY_GATE_TIMEOUT}");
    if !c.hidden_argv_modes.iter().any(|m| m.contains(&needle)) {
        problems.push(format!(
            "muse-cli.json hidden_argv_modes names no mode killed with `{needle}` (EXIT_PTY_GATE_TIMEOUT)"
        ));
    }
    let env = c.process_start_validated_env();
    if env.is_empty()
        || env.len() != c.process_start_validators.len()
        || env.iter().any(|v| !v.starts_with("MUSE_"))
    {
        problems.push(format!(
            "muse-cli.json process_start_validators must each start with the `MUSE_*` variable they validate, parsed {env:?}"
        ));
    }
    // `values_since`: every tagged value is one of the flag's listed values and
    // the tag is a build version (the OLDER-BUILD rule needs both).
    for f in &c.root_flags.items {
        let listed = f.accepted_values();
        for (value, since) in &f.values_since {
            if !listed.contains(&value.as_str()) {
                problems.push(format!(
                    "muse-cli.json root flag `{}` values_since names {value:?}, which is not in values {listed:?}",
                    f.flag
                ));
            }
            if !is_build_version(since) {
                problems.push(format!(
                    "muse-cli.json root flag `{}` values_since[{value:?}] = {since:?} is not a Muse build version",
                    f.flag
                ));
            }
        }
    }
    // Internal root flags: `--internal-…` spellings, none colliding with a listed flag.
    let internal = c.internal_root_flags();
    if internal.is_empty() || internal.len() != c.root_flags.internal_root_flags.len() {
        problems.push(format!(
            "muse-cli.json root_flags.internal_root_flags must each start with their `--internal-…` spelling, parsed {internal:?}"
        ));
    }
    for f in &internal {
        if !f.starts_with("--internal-") {
            problems.push(format!(
                "muse-cli.json internal root flag {f:?} is not spelled `--internal-…`"
            ));
        }
        if c.root_flag(f).is_some() {
            problems.push(format!(
                "muse-cli.json internal root flag {f:?} is also a listed root flag"
            ));
        }
    }
    // Verb tables: counts, usage lines, the plugins gate, the verbs omm calls.
    for (family, name, count) in [
        (&c.verbs.skills, "skills", c.counts.skills_verbs),
        (&c.verbs.plugins, "plugins", c.counts.plugins_verbs),
    ] {
        if family.count != family.items.len() || count != family.items.len() {
            problems.push(format!(
                "muse-cli.json verbs.{name}: count {} / counts.{name}_verbs {count} vs items {}",
                family.count,
                family.items.len()
            ));
        }
        let expected = family.usage_lines_expected();
        if family.usage.len() != expected {
            problems.push(format!(
                "muse-cli.json verbs.{name}.usage has {} lines, items account for {expected}",
                family.usage.len()
            ));
        }
        for line in &family.usage {
            let mut words = line.split_whitespace();
            let ok = words.next() == Some("muse")
                && words.next() == Some(name)
                && words
                    .next()
                    .map(|v| family.items.iter().any(|i| i.verb == v))
                    .unwrap_or(false);
            if !ok {
                problems.push(format!(
                    "muse-cli.json verbs.{name}.usage line {line:?} is not `muse {name} <listed verb> …`"
                ));
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for item in &family.items {
            if !seen.insert(item.verb.as_str()) {
                problems.push(format!(
                    "muse-cli.json verbs.{name} lists `{}` twice",
                    item.verb
                ));
            }
        }
    }
    if !c
        .verbs
        .plugins
        .gate
        .as_deref()
        .map(|g| g.contains(ENV_PLUGINS_GATE))
        .unwrap_or(false)
    {
        problems.push(format!(
            "muse-cli.json verbs.plugins.gate does not name {ENV_PLUGINS_GATE}"
        ));
    }
    if c.verbs.skills.gate.is_some() {
        problems.push("muse-cli.json verbs.skills carries a gate; `skills` is ungated".to_string());
    }
    for (command, verb) in HOST_VERBS_OMM_CALLS {
        if c.verb(command, verb).is_none() {
            problems.push(format!(
                "muse-cli.json verbs.{command} has no `{verb}` row (HOST_VERBS_OMM_CALLS: omm runs it)"
            ));
        }
    }
    for flag in ["--provider", "--trust-workspace", "--json", "--workspace"] {
        if !c.verbs.exec.documents_flag(flag) {
            problems.push(format!(
                "muse-cli.json verbs.exec.documented_flags lacks {flag} (probe::echo_session passes it)"
            ));
        }
    }
    if !c
        .verbs
        .exec
        .omm_probe_invocation
        .contains("--provider echo")
    {
        problems.push(
            "muse-cli.json verbs.exec.omm_probe_invocation is not the echo-provider session"
                .to_string(),
        );
    }
    problems
}

/// reserved-ids.json counts against its lists.
pub fn verify_reserved_ids(r: &ReservedIds) -> Vec<String> {
    let mut problems = Vec::new();
    if r.reserved_marketplace_names.len() != r.counts.reserved_marketplace_names {
        problems.push(format!(
            "reserved marketplace names: counts {} vs list {}",
            r.counts.reserved_marketplace_names,
            r.reserved_marketplace_names.len()
        ));
    }
    if r.windows_reserved_stems.ids.len() != r.counts.windows_reserved_stems {
        problems.push("windows reserved stems: counts vs list".to_string());
    }
    if r.builtin_tool_names.ids.len() != r.counts.builtin_tool_names {
        problems.push("built-in tool names: counts vs list".to_string());
    }
    let grammars = [
        ("skill_authoring_id", &r.id_grammar.skill_authoring_id),
        ("agent_definition_name", &r.id_grammar.agent_definition_name),
        ("workflow_name", &r.id_grammar.workflow_name),
        ("marketplace_name", &r.id_grammar.marketplace_name),
    ];
    let empty: Vec<&str> = grammars
        .iter()
        .filter(|(_, g)| g.trim().is_empty())
        .map(|(n, _)| *n)
        .collect();
    if !empty.is_empty() {
        problems.push(format!("reserved-ids.json id_grammar.{empty:?} empty"));
    }
    let bad_chars: Vec<&String> = r
        .reserved_key_bindings
        .chars
        .iter()
        .filter(|c| c.chars().count() != 1)
        .collect();
    if r.reserved_key_bindings.chars.is_empty() || !bad_chars.is_empty() {
        problems.push(format!(
            "reserved-ids.json reserved_key_bindings.chars must be single characters, found {bad_chars:?}"
        ));
    }
    if r.reserved_reminder_envelope_tags.tags.is_empty() {
        problems.push("reserved-ids.json reserved_reminder_envelope_tags.tags empty".to_string());
    }
    problems
}

/// settings-keys.json: `lazy` flags against the two lists, the load counts,
/// and the `mcpServers` field table.
pub fn verify_settings_keys(s: &SettingsKeys) -> Vec<String> {
    let mut problems = Vec::new();
    for k in &s.items {
        if k.lazy != s.is_lazy(&k.key) {
            problems.push(format!(
                "settings key `{}`: lazy={} but lazy_raw_value_keys/separate_pass_keys say {}",
                k.key,
                k.lazy,
                s.is_lazy(&k.key)
            ));
        }
    }
    for k in s
        .lazy_raw_value_keys
        .iter()
        .chain(s.separate_pass_keys.iter())
    {
        if !s.is_known(k) {
            problems.push(format!("settings lazy key `{k}` is not one of the items"));
        }
    }
    if s.counts.lazy_raw_value != s.lazy_raw_value_keys.len() {
        problems.push("settings counts.lazy_raw_value vs list".to_string());
    }
    if s.counts.separate_pass != s.separate_pass_keys.len() {
        problems.push("settings counts.separate_pass vs list".to_string());
    }
    let typed = s
        .counts
        .top_level_keys
        .saturating_sub(s.counts.lazy_raw_value + s.counts.separate_pass);
    if s.counts.typed_at_load != typed {
        problems.push(format!(
            "settings counts.typed_at_load {} vs {} (top-level − lazy − separate pass)",
            s.counts.typed_at_load, typed
        ));
    }
    if s.mcp_server_fields.count != s.mcp_server_fields.items.len()
        || s.counts.mcp_server_fields != s.mcp_server_fields.items.len()
    {
        problems.push("settings mcp_server_fields count vs items".to_string());
    }
    if s.mcp_server_fields.validation_literals.is_empty() {
        problems.push("settings mcp_server_fields.validation_literals empty".to_string());
    }
    if s.counts.tui_fields != s.tui_fields.items.len() {
        problems.push("settings counts.tui_fields vs items".to_string());
    }
    // A structural member must be one of the key's documented fields, and
    // the type prose must say the struct denies unknown fields.
    for k in s.items.iter().filter(|k| k.structural.is_some()) {
        let st = k
            .structural
            .as_ref()
            .map(|s| s.required.clone())
            .unwrap_or_default();
        if st.is_empty() {
            problems.push(format!(
                "settings key `{}`: structural.required is empty",
                k.key
            ));
        }
        for member in &st {
            if !k.fields.contains_key(member) {
                problems.push(format!(
                    "settings key `{}`: structural member `{member}` is not among its fields",
                    k.key
                ));
            }
        }
        if !k.type_desc.contains("deny_unknown_fields") {
            problems.push(format!(
                "settings key `{}`: structural but the type does not say deny_unknown_fields",
                k.key
            ));
        }
    }
    let untyped: Vec<&String> = s
        .tui_fields
        .items
        .iter()
        .chain(s.mcp_server_fields.items.iter())
        .filter(|f| f.type_desc.trim().is_empty())
        .map(|f| &f.key)
        .collect();
    if !untyped.is_empty() {
        problems.push(format!("settings field rows without a type: {untyped:?}"));
    }
    let no_default: Vec<&String> = s
        .tui_fields
        .items
        .iter()
        .filter(|f| f.default_value.is_none())
        .map(|f| &f.key)
        .collect();
    if !no_default.is_empty() {
        problems.push(format!(
            "settings tui_fields rows without a `default` (use null for none): {no_default:?}"
        ));
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_data_parses_and_matches_constants() {
        let problems = verify_data().unwrap();
        assert!(problems.is_empty(), "{problems:#?}");
    }

    #[test]
    fn gate_probe_verb_and_trace_literal_come_from_the_data_file() {
        let g = gates().unwrap();
        assert_eq!(
            g.probe.argv(),
            vec!["plugins", "enable", "__omm_gate_probe__"]
        );
        assert_eq!(
            g.probe.trace_subdir().as_deref(),
            Some(BOOTSTRAP_TRACE_SUBDIR)
        );
        assert!(verify_gates(g).is_empty());
        // A data edit that breaks the contract is caught.
        let mut bad: Gates = serde_json::from_str(RAW_GATES).unwrap();
        bad.probe.command = "muse plugins list".into();
        bad.probe.line_pattern = "event=\"gate.resolved\"".into();
        bad.probe.trace_glob = "$XDG_DATA_HOME/muse/traces/x.log".into();
        let problems = verify_gates(&bad);
        assert_eq!(problems.len(), 3, "{problems:#?}");
    }

    #[test]
    fn hook_stdout_ceiling_is_cross_checked() {
        let mut bad: HookEvents = serde_json::from_str(RAW_HOOK_EVENTS).unwrap();
        assert!(verify_hook_events(&bad).is_empty());
        bad.protocol.stdout_ceiling_bytes += 1;
        assert_eq!(verify_hook_events(&bad).len(), 1);
    }

    #[test]
    fn cli_gated_count_and_exit_codes_are_cross_checked() {
        let c = muse_cli().unwrap();
        assert!(verify_muse_cli(c).is_empty(), "{:#?}", verify_muse_cli(c));
        assert_eq!(c.items.iter().filter(|i| i.gate.is_some()).count(), 4);
        assert_eq!(c.items.iter().filter(|i| i.gate_hard).count(), 1);
        assert!(c.exit_codes.contains_key("125"));
        let mut bad: MuseCli = serde_json::from_str(RAW_MUSE_CLI).unwrap();
        bad.items
            .iter_mut()
            .for_each(|i| i.gate_hard = i.gate.is_some());
        bad.exit_codes.remove("125");
        assert_eq!(
            verify_muse_cli(&bad).len(),
            2,
            "{:#?}",
            verify_muse_cli(&bad)
        );
    }

    #[test]
    fn reserved_marketplace_names_are_parsed_and_counted() {
        let r = reserved_ids().unwrap();
        assert!(r.is_reserved_marketplace_name("tbh-curated"));
        assert!(!r.is_reserved_marketplace_name("ohmy"));
        assert!(r.is_reserved_plugin_id("muse-core"));
        assert!(verify_reserved_ids(r).is_empty());
        let mut bad: ReservedIds = serde_json::from_str(RAW_RESERVED_IDS).unwrap();
        bad.reserved_marketplace_names.clear();
        assert_eq!(verify_reserved_ids(&bad).len(), 1);
    }

    #[test]
    fn settings_lazy_flags_follow_the_two_lists() {
        let s = settings_keys().unwrap();
        assert!(
            verify_settings_keys(s).is_empty(),
            "{:#?}",
            verify_settings_keys(s)
        );
        assert!(s.is_lazy("mcpServers") && s.is_lazy("plugins") && !s.is_lazy("tui"));
        assert_eq!(s.counts.typed_at_load, 23);
        assert!(s
            .items
            .iter()
            .any(|k| k.key == "run" && k.fields.contains_key("context_slimming")));
        assert!(!s.mcp_server_fields.validation_literals.is_empty());
        let mut bad: SettingsKeys = serde_json::from_str(RAW_SETTINGS_KEYS).unwrap();
        bad.items.iter_mut().find(|k| k.key == "tui").unwrap().lazy = true;
        bad.counts.typed_at_load = 22;
        assert_eq!(
            verify_settings_keys(&bad).len(),
            2,
            "{:#?}",
            verify_settings_keys(&bad)
        );
    }

    #[test]
    fn contract_bearing_data_fields_are_typed_and_cross_checked() {
        // Every one of these was parsed by nothing (a data edit there was
        // never cross-checked) and `process_start_validators` was hand-copied
        // into a Rust literal against R8.
        let h = hook_events().unwrap();
        assert!(h.protocol.plugin_hook_fields.closed_set);
        assert!(h
            .protocol
            .plugin_hook_fields
            .fields
            .contains(&"command".to_string()));
        assert_eq!(
            h.protocol.plugin_hook_fields.rejected,
            vec!["matcher", "silent", "description"]
        );
        assert!(h
            .protocol
            .config_hook_fields
            .contains(&"commandWindows".to_string()));
        assert!(
            h.protocol.timeout.contains("timeout ≥ 1"),
            "{}",
            h.protocol.timeout
        );
        let g = gates().unwrap();
        assert!(g.accepted_values.on.contains(&"1".to_string()));
        assert!(g.accepted_values.off.contains(&"0".to_string()));
        let r = reserved_ids().unwrap();
        assert!(r.id_grammar.workflow_name.starts_with("[a-z0-9]"));
        assert!(r.id_grammar.agent_definition_name.starts_with("[a-z]+"));
        assert!(!r.id_grammar.marketplace_name.is_empty());
        assert!(!r.id_grammar.skill_authoring_id.is_empty());
        assert_eq!(r.reserved_key_bindings.chars, vec!["/", "!", "@", "?"]);
        assert!(r
            .reserved_reminder_envelope_tags
            .tags
            .iter()
            .any(|t| t.starts_with("<system-reminder>")));
        let s = settings_keys().unwrap();
        assert_eq!(
            s.items
                .iter()
                .find(|k| k.key == "provider")
                .unwrap()
                .default_value,
            serde_json::Value::from("meta")
        );
        assert!(s.tui_fields.items.iter().all(|f| !f.type_desc.is_empty()));
        assert!(s.tui_fields.items.iter().all(|f| f.default_value.is_some()));
        assert!(s
            .mcp_server_fields
            .items
            .iter()
            .all(|f| f.default_value.is_none()));
        assert_eq!(
            s.tui_fields
                .items
                .iter()
                .find(|f| f.key == "reasoning_summaries")
                .unwrap()
                .default_value,
            Some(serde_json::Value::Bool(true))
        );
        let c = muse_cli().unwrap();
        assert_eq!(c.hidden_argv_modes.len(), 3);
        assert!(c
            .hidden_argv_modes
            .iter()
            .any(|m| m.contains(&format!("exit {EXIT_PTY_GATE_TIMEOUT}"))));
        assert_eq!(
            c.process_start_validated_env(),
            vec!["MUSE_ENABLE_WEB_TOOLS", "MUSE_WEB_SEARCH_MODE"]
        );
        // The cross-checks catch a data edit.
        let mut bad: HookEvents = serde_json::from_str(RAW_HOOK_EVENTS).unwrap();
        bad.protocol.plugin_hook_fields.closed_set = false;
        bad.protocol
            .plugin_hook_fields
            .fields
            .push("commandWindows".into());
        bad.protocol.config_hook_fields.retain(|f| f != "timeout");
        assert_eq!(
            verify_hook_events(&bad).len(),
            3,
            "{:#?}",
            verify_hook_events(&bad)
        );
        let mut bad: Gates = serde_json::from_str(RAW_GATES).unwrap();
        bad.accepted_values.on.retain(|v| v != "1");
        assert_eq!(verify_gates(&bad).len(), 1, "{:#?}", verify_gates(&bad));
        let mut bad: ReservedIds = serde_json::from_str(RAW_RESERVED_IDS).unwrap();
        bad.reserved_key_bindings.chars.push("ab".into());
        bad.reserved_reminder_envelope_tags.tags.clear();
        bad.id_grammar.workflow_name.clear();
        assert_eq!(
            verify_reserved_ids(&bad).len(),
            3,
            "{:#?}",
            verify_reserved_ids(&bad)
        );
        let mut bad: MuseCli = serde_json::from_str(RAW_MUSE_CLI).unwrap();
        bad.process_start_validators.push("not an env var".into());
        bad.hidden_argv_modes.retain(|m| !m.contains("125"));
        assert_eq!(
            verify_muse_cli(&bad).len(),
            2,
            "{:#?}",
            verify_muse_cli(&bad)
        );
        let mut bad: SettingsKeys = serde_json::from_str(RAW_SETTINGS_KEYS).unwrap();
        bad.tui_fields.items[0].type_desc.clear();
        bad.tui_fields.items[1].default_value = None;
        assert_eq!(
            verify_settings_keys(&bad).len(),
            2,
            "{:#?}",
            verify_settings_keys(&bad)
        );
    }

    #[test]
    fn slash_collision_names_cover_rows_hidden_and_aliases() {
        let sl = slash_commands().unwrap();
        let names = sl.collision_names();
        assert!(names.contains(&"plan".to_string()));
        assert!(names.contains(&"login".to_string()), "hidden built-in");
        assert!(names.iter().all(|n| !n.starts_with('/')));
        assert_eq!(names.len(), sl.counts.collision_names_total);
    }

    /// The problems a verifier reports that mention `needle`.
    fn mentioning<'a>(problems: &'a [String], needle: &str) -> Vec<&'a String> {
        problems.iter().filter(|p| p.contains(needle)).collect()
    }

    #[test]
    fn hook_decision_matrix_tiers_shell_and_exit_codes_are_typed_and_cross_checked() {
        // Gate 0 residual: `items[].decisions`, `protocol.{exit_codes,tiers,shell}`
        // were parsed by nothing, so a data edit there could never fail.
        let h = hook_events().unwrap();
        assert!(
            verify_hook_events(h).is_empty(),
            "{:#?}",
            verify_hook_events(h)
        );
        let ups = h.event("UserPromptSubmit").unwrap();
        assert!(
            ups.accepts("block") && ups.accepts("skills_v1") && ups.accepts("bare_stdout_context")
        );
        assert_eq!(
            ups.decision("stop"),
            Some(None),
            "null = accepted, no effect"
        );
        assert_eq!(ups.decision("rewrite"), Some(Some(false)));
        assert_eq!(ups.decision("nonsense"), None);
        assert_eq!(ups.payload_class(), "proven");
        assert_eq!(h.event("PreToolUse").unwrap().payload_class(), "plausible");
        assert_eq!(h.event("PreCompact").unwrap().payload_class(), "proven");
        assert_eq!(h.events_accepting("skills_v1"), HOOK_SKILLS_V1_EVENTS);
        assert_eq!(
            h.events_accepting("bare_stdout_context"),
            HOOK_BARE_STDOUT_CONTEXT_EVENTS
        );
        assert_eq!(h.events_accepting("rewrite"), vec!["PreToolUse"]);
        assert_eq!(
            h.events_accepting("permission_verdict"),
            vec!["PermissionRequest"]
        );
        assert_eq!(
            h.event("SessionStart").unwrap().matcher_selector.as_deref(),
            Some("source")
        );
        assert_eq!(
            h.event("SessionStart")
                .unwrap()
                .payload
                .vocab
                .get("source")
                .map(Vec::len),
            Some(4)
        );
        assert_eq!(h.protocol.tiers.len(), HOOK_TIERS.len());
        assert!(h.protocol.exit_codes.contains("Only exit 2 blocks"));
        assert!(h.protocol.shell.contains("$SHELL -c"));
        // Each edit is caught, one row at a time.
        let mut bad: HookEvents = serde_json::from_str(RAW_HOOK_EVENTS).unwrap();
        bad.items[1].decisions.remove("skills_v1");
        let p = verify_hook_events(&bad);
        assert_eq!(mentioning(&p, "not the eight").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "accepting `skills_v1`").len(), 1, "{p:#?}");
        let mut bad: HookEvents = serde_json::from_str(RAW_HOOK_EVENTS).unwrap();
        bad.items[0].fires_in_echo_session = false;
        bad.items[0].payload.proven.clear();
        let p = verify_hook_events(&bad);
        assert_eq!(mentioning(&p, "fire_in_echo_session").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "payload_*").len(), 1, "{p:#?}");
        let mut bad: HookEvents = serde_json::from_str(RAW_HOOK_EVENTS).unwrap();
        bad.protocol.exit_codes = "exit 1 blocks".into();
        bad.protocol.tiers.swap(0, 1);
        bad.protocol.shell = "runs the command".into();
        bad.decision_kinds.remove("stop");
        let p = verify_hook_events(&bad);
        assert_eq!(mentioning(&p, "exit_codes").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "HOOK_TIERS").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "tiers[0]").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "protocol.shell").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "decision_kinds").len(), 1, "{p:#?}");
        assert_eq!(p.len(), 5, "{p:#?}");
    }

    #[test]
    fn cli_verb_tables_and_internal_root_flags_are_typed_and_cross_checked() {
        let c = muse_cli().unwrap();
        assert!(verify_muse_cli(c).is_empty(), "{:#?}", verify_muse_cli(c));
        assert_eq!(c.verbs.skills.items.len(), c.counts.skills_verbs);
        assert_eq!(c.verbs.plugins.items.len(), c.counts.plugins_verbs);
        assert_eq!(
            c.verbs.plugins.usage.len(),
            16,
            "install ×2 + marketplace ×4 + 10"
        );
        assert_eq!(c.verb("plugins", "marketplace").unwrap().subverbs.len(), 4);
        assert_eq!(c.verb("plugins", "install").unwrap().forms.len(), 2);
        assert_eq!(
            c.verb("plugins", "hook").unwrap().subverb.as_deref(),
            Some("test")
        );
        assert!(c.verb("skills", "list").is_some());
        assert!(c.verb("skills", "zzz").is_none() && c.verb("config", "validate").is_none());
        assert!(c.verbs.exec.documents_flag("--worktree") && c.verbs.exec.documents_flag("-w"));
        assert!(c.verbs.exec.documents_flag("--image"));
        assert!(!c.verbs.exec.documents_flag("--agents"), "undocumented");
        assert_eq!(
            c.internal_root_flags(),
            vec![
                "--internal-claude-channels-sidecar-v1",
                "--internal-claude-channels-live-smoke-host-v1"
            ]
        );
        assert!(c.is_internal_root_flag("--internal-claude-channels-sidecar-v1"));
        assert_eq!(
            c.root_flag_arity("--internal-claude-channels-sidecar-v1"),
            Some(RootFlagArity::None)
        );
        assert_eq!(c.root_flag_arity("--provider"), Some(RootFlagArity::One));
        assert_eq!(c.root_flag_arity("--zzznotaflag"), None);
        // Each edit is caught.
        let mut bad: MuseCli = serde_json::from_str(RAW_MUSE_CLI).unwrap();
        bad.verbs.skills.items.pop();
        bad.verbs.plugins.usage.push("muse plugins zzz".into());
        bad.verbs.plugins.gate = Some("none".into());
        bad.root_flags
            .internal_root_flags
            .push("--provider (again)".into());
        bad.verbs
            .exec
            .documented_flags
            .retain(|f| !f.starts_with("--provider"));
        let p = verify_muse_cli(&bad);
        assert_eq!(mentioning(&p, "verbs.skills: count").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "verbs.skills.usage has").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "verbs.plugins.usage has").len(), 1, "{p:#?}");
        assert_eq!(
            mentioning(&p, "verbs.plugins.usage line").len(),
            1,
            "{p:#?}"
        );
        assert_eq!(mentioning(&p, "verbs.plugins.gate").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "not spelled").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "also a listed root flag").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "HOST_VERBS_OMM_CALLS").len(), 1, "{p:#?}");
        assert_eq!(
            mentioning(&p, "documented_flags lacks --provider").len(),
            1,
            "{p:#?}"
        );
    }

    #[test]
    fn gate_bundled_and_slash_counts_are_cross_checked_across_files() {
        let (g, b, r, sl) = (
            gates().unwrap(),
            bundled_skills().unwrap(),
            reserved_ids().unwrap(),
            slash_commands().unwrap(),
        );
        assert!(verify_gates(g).is_empty(), "{:#?}", verify_gates(g));
        assert!(
            verify_bundled_skills(b, r).is_empty(),
            "{:#?}",
            verify_bundled_skills(b, r)
        );
        assert!(
            verify_slash_commands(sl, b, r).is_empty(),
            "{:#?}",
            verify_slash_commands(sl, b, r)
        );
        assert_eq!(g.counts.cli_visible, 2);
        // Nine on 1.3.0-R3057.1 (was seven): `grill-and-record` is gone
        // (renamed to `grill`), `migrate`, `resume-claude` and `resume-codex`
        // are new — each proven with a pty picker filter probe
        // (`/taste`, `/read-session` resolve to nothing).
        assert_eq!(
            b.items
                .iter()
                .filter(|s| s.slash_shortcut == Some(true))
                .count(),
            9
        );
        assert_eq!(
            b.items[1].slash_shortcut, None,
            "the gated skill's row was never observed"
        );
        assert_eq!(
            sl.items
                .iter()
                .filter(|i| i.kind == SLASH_KIND_PLUGIN_COMMAND)
                .count(),
            1
        );
        // gates.json edits.
        let mut bad: Gates = serde_json::from_str(RAW_GATES).unwrap();
        bad.items[0].default_state = "off".into();
        bad.items[1].env = "MUSE_EXPERIMENTAL_ARTIFACTS".into();
        bad.items[2].n = 9;
        bad.counts.cli_visible = 3;
        let p = verify_gates(&bad);
        assert_eq!(mentioning(&p, "default_on (items)").len(), 0);
        assert_eq!(mentioning(&p, "counts default_on").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "default_on_ids differ").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "env").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "n=9").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "cli_visible").len(), 1, "{p:#?}");
        let mut bad: Gates = serde_json::from_str(RAW_GATES).unwrap();
        bad.items.retain(|x| x.id != "plugins");
        assert!(!mentioning(&verify_gates(&bad), ENV_PLUGINS_GATE).is_empty());
        // bundled-skills.json edits.
        let mut bad: BundledSkills = serde_json::from_str(RAW_BUNDLED_SKILLS).unwrap();
        bad.items[2].slash_shortcut = Some(false);
        bad.items[3].skill_md_bytes += 1;
        bad.items[4].path = "skills/x/SKILL.md".into();
        bad.plugin_id = "not-reserved".into();
        let p = verify_bundled_skills(&bad, r);
        assert_eq!(mentioning(&p, "no slash shortcut").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "counts.slash_shortcuts").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "skill_md_bytes").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "path").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "package_files lacks").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "plugin_id").len(), 1, "{p:#?}");
        // A bundled id the reserved list does not know, and the cross-file
        // shortcut check from the slash side.
        let mut bad: BundledSkills = serde_json::from_str(RAW_BUNDLED_SKILLS).unwrap();
        bad.items[2].id = "plan2".into();
        bad.items[2].qualified_id = "bundled:plan2".into();
        bad.items[2].path = "skills/plan2/SKILL.md".into();
        bad.package_files[16].path = "skills/plan2/SKILL.md".into();
        let p = verify_bundled_skills(&bad, r);
        assert_eq!(
            mentioning(&p, "bundled_skill_ids differ").len(),
            1,
            "{p:#?}"
        );
        let p = verify_slash_commands(sl, &bad, r);
        assert_eq!(mentioning(&p, "names no bundled skill").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "has no /plan2 row").len(), 1, "{p:#?}");
        // slash-commands.json edits.
        let mut bad: SlashCommands = serde_json::from_str(RAW_SLASH_COMMANDS).unwrap();
        bad.items[0].kind = SLASH_KIND_SKILL_SHORTCUT.into();
        bad.hidden_builtins.pop();
        bad.builtin_aliases.push("/zz".into());
        bad.items[43].plugin_id = Some("omm".into());
        let p = verify_slash_commands(&bad, b, r);
        assert_eq!(mentioning(&p, "counts by kind").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "counts.hidden_builtins").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "builtin_aliases").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "builtin_canonical_total").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "/clear").len(), 2, "{p:#?}");
        assert_eq!(mentioning(&p, "plugin_id").len(), 1, "{p:#?}");
        assert_eq!(mentioning(&p, "shortcut rows").len(), 1, "{p:#?}");
    }

    #[test]
    fn root_flags_carry_spellings_and_arity() {
        let cli = muse_cli().unwrap();
        assert_eq!(cli.root_flags.items.len(), 30);
        let w = cli.root_flag("-w").unwrap();
        assert_eq!(w.spellings(), vec!["-w", "--worktree"]);
        assert_eq!(w.arity(), RootFlagArity::Optional);
        assert_eq!(w.accepted_values(), vec!["off", "create", "existing"]);
        assert!(w.accepts_value("off") && w.accepts_value("create"));
        assert!(!w.accepts_value("hi") && !w.accepts_value("OFF") && !w.accepts_value("--version"));
        assert!(cli.root_flag("--provider").unwrap().accepts_value("hi"));
        assert!(!cli.root_flag("--yolo").unwrap().accepts_value("hi"));
        assert_eq!(
            cli.root_flag("--provider").unwrap().arity(),
            RootFlagArity::One
        );
        assert_eq!(cli.root_flag("-V").unwrap().arity(), RootFlagArity::None);
        assert_eq!(
            cli.root_flag("--yolo").unwrap().arity(),
            RootFlagArity::None
        );
        assert!(cli.root_flag("--zzznotaflag").is_none());
        assert!(cli.is_root_positional_command("resume"));
        assert!(!cli.is_root_positional_command("exec"));
    }

    #[test]
    fn settings_keys_name_their_structural_members() {
        let s = settings_keys().unwrap();
        assert_eq!(s.structural_required("permissions"), ["schema_version"]);
        assert!(s.structural_required("tui").is_empty());
        assert!(s.structural_required("no-such-key").is_empty());
        let st = s
            .items
            .iter()
            .find(|k| k.key == "permissions")
            .and_then(|k| k.structural.as_ref())
            .unwrap();
        assert!(st.deny_unknown_fields);
        // A structural member outside the key's fields is a data error.
        let mut bad: SettingsKeys = serde_json::from_str(RAW_SETTINGS_KEYS).unwrap();
        for k in &mut bad.items {
            if k.key == "permissions" {
                k.structural = Some(Structural {
                    deny_unknown_fields: true,
                    required: vec!["nope".into()],
                    notes: None,
                });
            }
        }
        assert!(verify_settings_keys(&bad)
            .iter()
            .any(|p| p.contains("structural member `nope`")));
    }

    #[test]
    fn settings_keys_know_legacy_spelling() {
        let s = settings_keys().unwrap();
        assert!(s.is_known("mcpServers"));
        assert!(!s.is_known("mcp_servers"));
        assert_eq!(s.canonical_spelling("mcp_servers"), Some("mcpServers"));
    }
}
