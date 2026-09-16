# Muse facts omm depends on (measured on 1.0.1-R2006.1, re-verified on 1.1.0-R2009.1)

Never gate on the version string: 45 measured axes differed between those builds on exactly 2
cosmetic values. `omm doctor --self-test` re-measures; `--report-drift` prints the diff table.

## Skills catalog budget (context block order 200)

- Hard cap 32,000 B: 363 B header + entries + 35 B footer. UTF-8 bytes, XML-escaped.
- Meta's 15 bundled skills cost 10,060 B of entries (10,458 B block); 14 of 15 render without
  `MUSE_EXPERIMENTAL_PLUGINS=1` (`bundled:create-plugin` is gated). Room for everything else:
  21,542 B; 31,602 B with the built-ins disabled (`muse skills disable bundled:<id> --scope built-in`;
  `bundled:browser-app-delivery` alone refunds 1,912 B). omm CI refuses a bundle over 21,542 B.
- Entry cost, plugin scope: `38 + len(id) + len(path) + len(description) + 36`; the plugin id appears
  twice per entry. `metadata.short-description` renders as an extra `<short-description>` element.
- Render order: bundled -> project/user filesystem -> plugin. Plugin entries are starved FIRST.
- Degradation: stage 1 intact -> stage 2 descriptions dropped tail-first, SILENT (`skills list --json`
  still reports all, `diagnostics: []`) -> stage 3 entries dropped, stderr names `32000`.
- `run.context_slimming.skill_catalog_descriptions: "first_sentence"` keeps the first sentence of every
  description (cut at the first `.`/`?`/`!` + space; -52 % on a 40-skill install; room becomes
  24,924 B on 1.3.0 (was 27,168 B on 1.0.x); `skills list --json` still shows full text).
  `full_skill_description_ids` is an exact
  display-id list (`bundled:git`, never `git`); its default is `["bundled:git"]` and a user value
  REPLACES it - always re-list `bundled:git`.
- `run.context_slimming.excluded_tool_names: ["workflow"]` drops the tool and orders 180+181
  (-20,952 B). `["bash"]` without `bash_input` fails the run (exit 1). `write_todos` is not removable.
- `run.workflow_trigger_mode: "off"` removes the 18,056 B cookbook (-20,413 B net, 539 B less than
  excluding the tool).
- Routing (orders 200 + 201): 31,984 B combined; 32 routed skills per turn; description <= 1,024 B.
- Memory snapshot: 16,305 B and 48 files per scope, silent. Rules: 256,000 B/file, 65,536 B aggregate,
  warned on stderr.
- SKILL.md loader cap 262,144 B; a smaller `read_skill` read guard truncates the body
  (`skill-file-too-large`, "returned a truncated prefix").
- Plugin package: manifest <= 131,072 B; <= 4,096 fs entries; path depth <= 16; any symlink anywhere
  kills the package with an opaque error. Enabled plugins: 256 (warn at 200).

## Plugin capability lifecycle

- Spawn gate for MCP / hook / reminder: state literally `trusted_enabled`. `review_needed` (default after
  install), `trusted_disabled`, `modified`, line absent -> silently skipped. Skills and commands in a
  plugin carry no such state; they load once the plugin is installed.
- Model-side symptom: no `mcp__plugin_omm_<sid>` namespace in the tool list; any call ->
  `tool unavailable: unknown tool`, the turn continues.
- `muse plugins approve plugin:omm:<hook|mcp_server|reminder|agent_definition>:<id> --json` - no TTY.
- `muse plugins inspect omm --json` - verify presence AND state. `muse plugins disable <id>` DELETES the
  line. Any byte change to the package + `plugins update` -> every capability `modified`.
- `muse plugins remove omm --delete-data` strips `runtime_capabilities`.
- `MUSE_EXPERIMENTAL_PLUGINS=1` is required for every `muse plugins ...` verb and for `/plugins` in the
  TUI. Runtime composition (skills, commands, hooks, MCP from an installed plugin) is ungated.
- MCP: `len(plugin id) + len(server id) <= 18`, else the namespace is rewritten and the `<ns>__<tool>`
  form stops dispatching. The model calls `<ns>.<tool>`. `env`, `cwd`, `headers` on an `mcpServers`
  entry validate and are then DROPPED at launch.
- Reserved plugin ids that install "successfully" and do nothing: `skill-reminder goal-reminder
  memory-reminder todo-reminder verify-reminder scope-reminder tbh-reminders loop muse-core`.

## Marketplace and update

- Probe order in a marketplace dir: `marketplace.json` (native) -> `.agents/plugins/marketplace.json`
  -> `.claude-plugin/marketplace.json`; first existing file wins, no merge. The package's manifest dir
  decides the family; omm asserts `manifest_family == "native"`.
- `marketplace update` always creates a new generation (keep current + previous). An installed plugin
  stays pinned to the generation it came from; `plugins update oh-my-musecode` re-reads that pinned path and after
  two rotations (or `marketplace remove`) fails `plugin-source-unavailable`.
- Update signal: none. Compare `plugins list --available --json -> digest` with the installed
  `package_sha256`; equal means no-op.
- Only path after a rotation: `plugins remove oh-my-musecode` -> `plugins install oh-my-musecode@omm` -> approve each
  capability -> verify. That is what `omm update` runs.
- A package is good only when `rc == 0 && valid == true && diagnostics == [] &&
  manifest_family == "native"` and every compatibility declaration is `supported`. `valid` alone and
  `compatibility.summary == "full"` are both unsafe gates.

## settings.json

- `<config root>/settings.json`, 29 typed top-level keys, `schema_version: 1` hard-required
  (missing, null, 0, 2 all fail).
- Rewritten by Muse from its typed struct on every settings-mutating verb (even `muse skills disable`):
  unknown top-level keys, unknown `tui.*` members and legacy `mcp_servers` are destroyed; the file is
  re-pretty-printed with 2 spaces.
- Wrong type on a typed key or a bad enum under `tui.*` -> `malformed settings file at <path>`; every
  settings-consuming verb fails (`--version --help export init workflows list` still work - probe with
  `muse skills list`). Wrong type on a lazy key (`hooks plugins permissions runtime_capabilities
  model_catalog`): loads, that subsystem degrades.
- `mcpServers` and `mcp_servers` both present (even both `{}`) -> the whole MCP config is silently
  discarded; every settings-mutating verb exits 1 `MCP configuration error ...`; read-only lanes silent.
- Env beats settings (`MUSE_*` / `TBH_*`); CLI flags beat both.
- There is NO workspace settings file (26 candidate paths probed, none read). Workspace tier is files
  only: `AGENTS.md`, `.agents/skills/`, `.agents/agents/`, `.agents/memory/`, `.agents/workflows/`,
  `.muse/hooks.json`. No project plugin auto-discovery either.
- Offline linter: `muse config validate --plane defaults --file <tmp>` - needs the wrapper
  `{"schema_version":1,"settings":{...}}`; refuses `hooks plugins runtime_capabilities mcpServers
  model_catalog permissions` (`unknown_member`) and `tui.theme` (`field_not_activated`).

## trust.json

- `<config root>/trust.json` = `{"schema_version":1,"projects":{"<abs workspace>":{"decision":"trusted"}}}`;
  `decision` in `trusted | untrusted`; `projects` may be absent (everything untrusted, exit 0).
- Malformed file (typo'd key, missing `decision`, `schema_version` != 1) -> every session exits 1.
- Gated on trust: project `AGENTS.md`, project skills, project hooks (`.muse/hooks.json`), project
  workflows (no CLI escape at all), project agent definitions, project memory root,
  `plugins install --scope project`. User-scope, bundled and plugin skills are NOT gated.
- No muse CLI verb writes it. `--trust-workspace` is per-run and unsaved (`--yolo` implies it). Only the
  TUI prompt and omm write it. A run reports `muse: workspace trust: trusted source=...` on stderr.

## Paths

| What | Where |
|---|---|
| config root | `$XDG_CONFIG_HOME/muse` else `~/.config/muse` - nothing else (`MUSE_HOME`, `MUSE_CONFIG_DIR` are not read) |
| data root | `$XDG_DATA_HOME/muse` else `~/.local/share/muse` |
| personal skills | `<config root>/skills/` first, then `~/.agents/skills/`, then foreign agent roots (`context.foreign_personal_skills: false` turns those off); first hit per id |
| project skills | `<ws>/.agents/skills/` first, then foreign roots; one level deep; trust-gated |
| personal rules | exactly ONE file loads: `<config root>/AGENTS.md`, else fallbacks |
| project rules | `AGENTS.md` root -> cwd, accumulate; trust-gated |
| hooks tiers | managed -> user (`settings.json -> hooks`) -> project (`.muse/hooks.json`) -> plugin; command runs via `$SHELL -c` with a 16-key scrubbed env, cwd = workspace |
| themes | `<config root>/themes/*.tmTheme`; `tui.theme = "custom:<stem>"`; data dir NOT scanned |
| memory | `<data root>/memory/personal/` and `<data root>/memory/projects/<slug96>-<fnv1a64hex>/` |
| sessions | `<data root>/sessions/YYYY/MM/DD/<uuid>/session.jsonl` |
| marketplace generations | `<data root>/plugins/marketplaces/<name>/generations/<epoch-ns>/` |
| bootstrap trace | `<data root>/local-tracing/bootstrap/cli-<uuid>.log` (`gate.resolve` lines; writer is lossy) |
| plugin data dir | `MUSE_PLUGIN_DATA_DIR` is advertised but not created - `mkdir -p` it |
| residue | `~/Library/Application Support/Muse/session-name-authority/` on every run, real HOME; uninstall names it, never removes it |

Redirecting `HOME` does not fully sandbox Muse. Launcher `~/.local/bin/muse` auto-updates hourly;
`MUSE_NO_AUTO_UPDATE=1` is read by the launcher only - omm calls `muse-bin-<version>` directly.

## Exit codes

| Situation | Code |
|---|---|
| argv rejected - parse error OR missing `MUSE_EXPERIMENTAL_PLUGINS` gate, indistinguishable | 2 |
| parsed, run failed | 1 |
| unknown first token | not an error: it is the `[PROMPT]` positional and starts a billed session |
| flag-only argv (`muse --provider echo`, `muse -w`) | starts the TUI |
| malformed `settings.json` | lazy: only settings-consuming verbs fail |
| malformed `trust.json` | every session aborts, exit 1 |
| `mcpServers` + `mcp_servers` both present, settings-mutating verb | 1 |

Top-level verbs (the `omm run` allowlist): `resume exec config export trace skills sandbox schema
serve session-message auth login logout init plugins workflows` (`plugins` gated, `workflows` hidden).
`muse exec --provider meta ... "hi"` never POSTs (replays a fixture) - doctor and cost use `--provider echo`.
