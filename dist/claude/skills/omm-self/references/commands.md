# omm command reference

Global flags: `--json` on every read command, `--dry-run` on every write command, `-v` prints every
host process omm spawns. Every write prints a per-category converge summary
(updated / unchanged / skipped / backed-up / removed) and appends one JSONL line to `audit.log`.
Every host call goes through `omm run`'s invoker: `MUSE_NO_AUTO_UPDATE=1` + `NO_COLOR=1` always,
`MUSE_EXPERIMENTAL_PLUGINS=1` only when the first token is `plugins`, argv allowlist first.

| Command | Does | Writes |
|---|---|---|
| `omm install [--no-plugin] [--yes] [--dry-run]` | `plugins marketplace add omm <src> --json` (already configured -> `marketplace update omm`) -> `plugins list --available --json` (assert `oh-my-musecode` `status: "available"`, keep `digest`) -> `plugins install oh-my-musecode@omm --json` (assert `manifest_family == "native"`, `package_sha256 == digest`) -> approve every capability -> verify `trusted_enabled` via `inspect --json` -> cross-check `skills list --json`. Also: `AGENTS.md`, default profile, themes, trust for cwd when it is a workspace. Prints the catalog bytes consumed. Idempotent. Refuses under `CI=true` / non-TTY without `--yes`. `--no-plugin` = `muse skills install` per skill into the managed store instead of the plugin. | ledger, audit, muse config root, muse data root (via muse) |
| `omm uninstall [--dry-run] [--force]` | Ledger traversal in reverse, deepest-first. Two-section preview: remove (on-disk sha == ours) / preserve (user edited; removed only with `--force`). Runs `muse plugins remove oh-my-musecode --delete-data`, `muse plugins marketplace remove omm`, restores every settings key to its recorded `prior`. Names kept residue (`~/Library/Application Support/Muse/session-name-authority/`). | ledger removed last |
| `omm update [--dry-run]` | `snapshots/<ts>/` of every ledgered file (keep 5) -> per-entry 3-way merge with ancestor = ledger sha: no-op / overwrite (user never touched) / adopt (user already has the new bytes) / stage to `updates/<version>/<path>` (both changed; on-disk untouched) -> `marketplace update omm` -> digest compare (equal -> no-op) -> pre-flight install in a scratch `XDG_DATA_HOME` -> `plugins remove` -> `plugins install` -> approve -> verify, as one transaction. Reports staged conflicts. | ledger, audit, snapshots, updates |
| `omm doctor [--self-test] [--report-drift] [--json]` | Checks D1-D14 below. `--self-test` re-measures the golden constants against the live binary. `--report-drift` prints the golden-constant diff table. Exit non-zero on any `critical`. | nothing |
| `omm cost [--json]` | Skills-catalog byte table by source (bundled / filesystem / plugin - plugin is starved first), the refundable built-in tax (10,060 B), memory (16,305 B / 48 files per scope), rules (256,000 B/file, 65,536 B aggregate), the 18,056 B workflow cookbook and how to drop it, tokens ~ bytes/4 (labelled as a heuristic). Runs one `--provider echo` session in a temp `XDG_DATA_HOME`. | nothing |
| `omm lint [path]` | Default path `./content`. id grammar `^[a-z0-9][a-z0-9._-]{0,79}$`, `omm-` prefix, collisions with the 9 reserved plugin ids / 15 bundled skills / 39 built-in slash commands, `len("oh-my-musecode") + len(server-id) <= 18` (server id stays <= 4 chars), no symlink, no backslash in filenames, manifest <= 131,072 B, <= 4,096 fs entries, depth <= 16, SKILL.md rules (no BOM, `name` == dir, description <= 240 chars with a negative-trigger clause), catalog estimate <= 21,542 B, then the three host checkpoints: `muse skills validate` per skill -> `muse plugins validate` -> `muse skills list --json`. | nothing |
| `omm build` | dev only: regenerate `plugins/omm/` (native package), the `.claude-plugin` / `.codex-plugin` projections under `dist/`, and the three marketplace catalog files. | repo |
| `omm trust [path]` | Merge `{"<abs>":{"decision":"trusted"}}` into `trust.json`; never rewrites the map. Default path: cwd. | ledger |
| `omm theme <name\|custom:<stem>>` | targeted `tui.theme` patch. Custom themes live in `<config root>/themes/<stem>.tmTheme` (data dir is NOT scanned). | ledger |
| `omm keymap <preset>` | targeted `tui.keymap` patch. | ledger |
| `omm profile use strict\|default\|fast\|ci` | apply a settings slice as one ledgered transaction; prior value recorded per key. | ledger |
| `omm memory seed\|list\|backup\|gc` | the `personal_project` memory root `$XDG_DATA_HOME/muse/memory/projects/<slug96>-<fnv1a64hex>/`: seed `MEMORY.md` for cwd, list roots, back up cwd's root, remove roots of vanished workspaces. | ledger |
| `omm enable\|disable skill-routing` | opt-in routing via `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1` + `_APPLY=1`; combined catalog + routing budget 31,984 B. | ledger |
| `omm run -- <muse args>` | first token must be one of `resume exec config export trace skills sandbox schema serve session-message auth login logout init plugins workflows`; sets `MUSE_NO_AUTO_UPDATE=1`, the profile gates, `MUSE_EXPERIMENTAL_PLUGINS=1` for `plugins`, then exec. | nothing |
| `omm hook <name>` | hook dispatcher: JSON event on stdin -> handler chain -> JSON on stdout; fail-open on error; sub-5 ms. Plugin hooks are declared as `omm hook <name>`, so `omm` must be on `PATH`. | nothing |
| `omm list [--json]` | resolved assets with provenance and `_shadowed`. | nothing |
| `omm mcp` | stdio MCP server exposing `omm_doctor` and `omm_cost` to the model. Creates `MUSE_PLUGIN_DATA_DIR` itself (Muse advertises but does not create it). | nothing |

## Every settings write, in order

load `settings.json` -> patch only typed keys -> serialize -> `muse config validate --plane defaults --file <tmp>`
(the candidate wrapped as `{"schema_version":1,"settings":{...}}`; that plane refuses `hooks plugins
runtime_capabilities mcpServers model_catalog permissions` and `tui.theme`, so those are stripped before
validation and re-added on write) -> verified backup -> atomic rename on the resolved realpath. Refused
outright when `mcpServers` and `mcp_servers` are both present (report it, do not add to it).

## Doctor checks

Always copy the fix from the live `omm doctor --json` output (`fix` field); this table is the map, not
the command source.

| Id | Detects | Fix printed |
|---|---|---|
| D1 | a declared `mcp_server` / `hook` / `reminder` line missing from `plugins inspect omm --json` or not literally `trusted_enabled` | `muse plugins approve plugin:omm:<kind>:<id>` or `omm install` |
| D2 | `settings.provider` unset -> `muse serve` / SDK get a one-tool session, no MCP | `omm settings set provider meta` |
| D3 | `mcpServers` + `mcp_servers` both present -> every settings-writing verb exits 1 | `omm settings fix-mcp-collision` |
| D4 | malformed `settings.plugins` / `settings.runtime_capabilities` - one bad entry disables both wired reminders | `omm settings lint --fix` |
| D5 | canonical vs legacy reminder enablement disagree | `omm settings reconcile-reminders` |
| D6 | `agent_definitions.safe_mode: true` -> user/project/plugin agent packs never load | `omm settings set agent_definitions.safe_mode false` |
| D7 | enabled plugins vs 256 (warn at 200) | `muse plugins disable <id>` |
| D8 | catalog pressure from a real echo session: entries / entries-with-description / bytes of 32,000; stage-2 silent description drops | `muse skills disable bundled:<id> --scope built-in`, `omm cost` |
| D9 | enterprise config rows (`muse config status`) | informational |
| D10 | installed `omm` is not `manifest_family == "native"` | `omm install --reinstall` |
| D11 | host drift: golden constants vs live probes | `omm doctor --report-drift` |
| D12 | workspace not in `trust.json` -> project skills/hooks/rules/workflows silently inert | `omm trust .` |
| D13 | ledger corrupt/missing, ledgered file vanished, unlisted file under an omm root | `omm reconcile` |
| D14 | exit-code matrix + argv allowlist sanity | - |

A corrupt ledger is renamed `omm.lock.json.bad` and treated as absent, with a loud warning.
Doctor compares what composed in a LIVE session against what was installed - never disk vs disk
(Meta can allowlist extension kinds server-side without shipping a binary).
