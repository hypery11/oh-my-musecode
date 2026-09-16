---
name: omm-self
description: Use when the user asks about omm (oh-my-musecode), why an omm-installed skill, hook, command, theme or MCP tool is not loading in Muse, or wants Muse configured through omm; Do not use for Muse settings or faults with no omm involved.
---

# omm (oh-my-musecode)

omm = curated content bundle + thin CLI + git marketplace for Muse Code. It never patches the
binary: it installs ONE native plugin `oh-my-musecode` (skills, commands, hooks, MCP, reminders)
from marketplace `omm` through `muse plugins ...`, records every file it writes in a ledger, and undoes
exactly that on uninstall. Every asset id is `omm-<name>`.

Rule zero: `omm doctor` before touching any file. Each finding = what was observed, why Muse is
silent about it, the exact fix command. Exit != 0 = critical.

## Layout

`$XDG_CONFIG_HOME/omm/` (else `~/.config/omm/`) - everything omm owns:
- `omm.lock.json` - THE ledger: every path omm wrote, its sha256, prior value. Truth, not a scan.
- `config.json` - `{"profile":"default","disabled":["skill:omm-x","hook:omm-y"]}`; kind-qualified ids.
- `custom/<kind>/<id>` - overlay, searched before bundled content (REPLACES); `custom/<kind>/<id>_append.md`
  APPENDS. kinds: `skills commands hooks agents themes`.
- `updates/<version>/` - conflicts staged by `omm update`; `snapshots/<ts>/` - pre-update copies; `audit.log`.

Written into Muse, every line ledgered: config root `$XDG_CONFIG_HOME/muse` (else `~/.config/muse`) -
`skills/<id>/`, `AGENTS.md`, `themes/*.tmTheme`, `trust.json` (merged), `settings.json` (typed keys only).
Data root `$XDG_DATA_HOME/muse` (else `~/.local/share/muse`) - `plugins/`: Muse writes, omm records the registration.

## Commands - run with `bash`; `--json` on reads, `--dry-run` on writes

- `omm doctor [--self-test] [--report-drift]` - anything not loading / firing / listed. Prints check id + fix.
- `omm cost` - "context feels bloated": per-source byte table of the 32,000 B skills catalog.
- `omm install [--no-plugin] [--yes]` - first setup, repair, re-approve. Idempotent. `--yes` required under `CI=true` / non-TTY.
- `omm update` - new omm release: snapshot -> 3-way merge -> plugin remove/install/re-approve -> lists staged conflicts.
- `omm uninstall [--force]` - two-section preview: remove / preserve (user-edited files kept unless `--force`).
- `omm trust [path]` - project skills/hooks/rules/workflows inert -> merge the workspace into `trust.json`.
- `omm theme <name>` / `omm keymap <preset>` / `omm profile use strict|default|fast|ci` - settings changes:
  targeted patch, validated first, prior value ledgered.
- `omm lint [path]` - user-authored content: ids, collisions, symlinks, budget.
- `omm list` - what resolves, from where, `_shadowed`.
- `omm run -- <muse args>` - invoke muse safely: argv allowlist, `MUSE_NO_AUTO_UPDATE=1`, gate for `plugins`
  verbs. Never diagnose through the `~/.local/bin/muse` launcher (it auto-updates hourly).
- `omm memory {seed,list,backup,gc}` / `omm enable|disable skill-routing` / `omm hook <name>` /
  `omm mcp` (stdio MCP server) / `omm build` (dev) - see `references/commands.md`.

## Checklist: an omm skill / hook / command is not loading

1. `omm doctor --json`. Run the `fix` of every failing check, verbatim. Re-run. Stop if green.
2. `omm list --json`. Id absent -> `read_file` `config.json`: remove it from `disabled`, or `omm install`.
   `_shadowed:true` -> a `custom/` overlay or another skill root hides it; name the shadowing path.
3. Skill: `omm run -- skills list --json`. Id must be listed with `activation: "on"`. Project scope -> 4.
   Listed but the model never sees it -> 6.
4. Trust: `read_file` `<config root>/trust.json`. Workspace absent or `untrusted` -> `omm trust .`.
   An untrusted workspace loads NO project skills, hooks, rules, workflows or agents, with no error.
5. Hook / MCP / reminder: `omm run -- plugins inspect omm --json` (omm sets the `plugins`
   gate; a bare `muse plugins ...` needs `MUSE_EXPERIMENTAL_PLUGINS=1`). Every capability line
   must be PRESENT and literally `trusted_enabled`. `review_needed` (default after install),
   `trusted_disabled`, `modified`, missing -> silently skipped. One bad line ->
   `omm run -- plugins approve plugin:omm:<hook|mcp_server|reminder>:<id> --json`; several -> `omm install`.
   Hook still silent -> `which omm` (the hook command is `omm hook <name>`).
6. Catalog: `omm cost`. Past 32,000 B Muse drops descriptions tail-first, SILENTLY - `skills list` still
   shows every skill, `diagnostics: []`. Plugin entries starve first. Apply the D8 fix doctor prints.
7. Still wrong -> `omm doctor --report-drift`; paste the drift table to the user.

## Muse facts that bite

- `trust.json` = `{"schema_version":1,"projects":{"<abs path>":{"decision":"trusted"}}}` is the master
  gate. No muse CLI verb writes it; `--trust-workspace` is per-run only; a malformed file exits
  every session with 1.
- Skills catalog cap 32,000 B, render order bundled -> filesystem -> plugin. Relief, cheapest first:
  `run.context_slimming.excluded_tool_names: ["workflow"]` (-20,952 B), `muse skills disable bundled:<id>
  --scope built-in` (built-ins: 10,060 B), `skill_catalog_descriptions: "first_sentence"`
  (-52 %; re-list `bundled:git` in `full_skill_description_ids` - a user value REPLACES the default).
- `settings.json` is rewritten from a typed struct by every settings-mutating verb (even `muse skills
  disable`): unknown top-level keys, unknown `tui.*` members and legacy `mcp_servers` are DESTROYED.
  `schema_version: 1` required; one bad `tui.*` enum makes every settings-consuming verb fail.
  `mcpServers` + `mcp_servers` both present -> MCP silently off, every settings-mutating verb exits 1 (D3).
- `MUSE_EXPERIMENTAL_PLUGINS=1` gates only `muse plugins ...` verbs (install time). Runtime composition is
  ungated - exporting it in a shell profile loads nothing. Without it `muse plugins ...` exits 2 (same as an argv typo).
- Never `muse plugins update omm`: an installed plugin pins a marketplace generation; after two rotations
  it fails `plugin-source-unavailable`. `omm update` is the only path.
- `muse plugins disable <id>` DELETES capability lines; any byte change to the package flips every
  capability to `modified` - re-approve.
- `muse <unknown word>` is not an error: a PROMPT that starts a billed session. Use `omm run --`.
- Exactly ONE personal rules file loads, `<config root>/AGENTS.md` first. `settings.provider` unset ->
  `muse serve` gets a one-tool session, no MCP (D2). `agent_definitions.safe_mode: true` -> no agent
  packs load (D6).

## Judgment calls

- Overlay, never edit in place. To change a bundled asset write `custom/<kind>/<id>` or `<id>_append.md`.
  An edited installed copy is not merged by `omm update` (staged under `updates/<ver>/`, on-disk
  untouched) and `omm uninstall` preserves it unless `--force`. Say which before the user chooses.
- Disable, do not delete. omm assets: `config.json` -> `disabled`. Built-ins: `muse skills disable
  bundled:<id> --scope built-in`. A deleted ledgered file is D13 and uninstall can no longer undo it.
- Not omm's question: a plain Muse setting (model, effort, /settings) -> `read_skill` with name
  `bundled:manage-settings`; a Muse runtime fault with no omm involved -> `read_skill` `bundled:doctor`.
- Never gate on the Muse version; `omm doctor --self-test` probes behaviour.

## Rules for you

- `bash` for omm/muse commands; `read_file` for ledger and config; `edit_file`/`write_file` only under
  `custom/` and `config.json`. Never write `settings.json`, `trust.json` or `omm.lock.json` yourself;
  never hand-copy into `<config root>/skills/` (the managed store has its own lock).
- A settings key no omm command covers: validate the candidate first with
  `muse config validate --plane defaults --file <tmp>` (wrapped `{"schema_version":1,"settings":{...}}`).
- `--dry-run` before install / update / uninstall; show the converge summary. Quote doctor output
  verbatim; its `fix` field is the answer. Report observed, not expected, state.
- Depth: `references/commands.md` (flags, writes, doctor D1-D14), `references/muse-facts.md` (budgets,
  capability states, paths, exit codes), `references/ledger.md` (ledger, reconcile, uninstall, overlay).
