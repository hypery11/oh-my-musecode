# Fold note: muse-stable 1.0.3-R2198.1 → 1.3.0-R3057.1

Status: **RESOLVED — pins folded; see the data files for the evidence.**

Caveat, read first: this note was reconstructed after the fold from the
committed pins (muse-cli.json, gates.json, bundled-skills.json, hook-events.json
and the `hr::` constants that cross-check them). It is NOT a fresh behavioural
run in the style of stable-1.0.3-diff.md — it names what changed and where the
proof lives, nothing more. Every claim below resolves to a committed data row;
anything not in the data files is not claimed here.

## What moved (1.0.3 → R3057.1)

| # | Axis | 1.0.3 | R3057.1 | Where the proof lives |
|---|---|---|---|---|
| 1 | CLI surface | 16 commands, no top-level `mcp` | new top-level `mcp`: OAuth login/logout for streamable-HTTP MCP servers | muse-cli.json `also_verified_on` |
| 2 | Feature gates | 42 | **47**: `official_plugin_marketplace`, `memory_repository_sync`, `native_connector_delivery`, `vim`, `context_meter` join (all `since: 1.3.0-R3057.1`); `voice`, `voice_default_on`, `todo_reminder` flip to default ON (17) | gates.json items/`counts`/`also_verified_on`; `hr::GATES_TOTAL` |
| 3 | Ultra effect | gate absent (10 blocks unconditional) | closed: 8 blocks + downgrade stderr; open: 10 blocks, a single order-187 `delegation_proactive_combined` (the separate order-183 block no longer composes) | gates.json ultra `gates` + `effect_probe` |
| 4 | Bundled skills | 15 ids | `grill-and-record` renamed to `grill`; `durable-test-collateral`, `migrate`, `requirements-clarification`, `resume-claude`, `resume-codex`, `workflow-authoring` added; `plan`, `manage-settings`, `import`, `read-session` bodies change | bundled-skills.json `also_verified_on` |
| 5 | Hook payloads | PreCompact/SubagentStart unknown | PreCompact proven (`trigger`, no `reason`), SubagentStart proven (nine keys, no `agent_id`/`agent_type`) — measured live on R3057.1 | hook-events.json notes |
| 6 | Enterprise generation | `sha256:db7c…` (macOS) | same value on macOS; the hash covers the OS-specific source list (four lines macOS, two Linux — pinned per OS after the R3401.1 refresh) | `hr::ENTERPRISE_GENERATION`, host-reality.md Release log |

## What did not move

MSP surface (47 methods / 30 notifications / 31 errors / fingerprints), hook
event ids (17), context-block orders (9/8), `config status` source states (all
absent), `settings.json` schema — all byte-identical between the two builds to
the resolution of the hostcheck P0/P1 tables, which pass on both.
