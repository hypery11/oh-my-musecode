# oh-my-musecode

`omm` is a curated content bundle, a thin CLI and a git marketplace for **Meta Muse Code**: thirty-five
skills, three slash commands, eight hooks, a reminder, an in-binary MCP server, three themes and four
settings profiles, installed through Muse's own plugin and config surface. Every byte it writes into
Muse's directories is recorded in an ownership ledger, so `omm uninstall` puts the machine back
byte for byte and `omm doctor` can tell you exactly which silent failure a live session would hit.
It never touches the Muse binary, never writes framework state into `settings.json`, and refuses to
write anything at all without a human (or an explicit `--yes`) in the loop.

## Install

Phase one places one static binary and writes nothing else. Phase two (`omm install`) is the only
thing that touches Muse config (ARCHITECTURE R6).

```sh
curl -fsSL https://github.com/hypery11/oh-my-musecode/releases/latest/download/install.sh | sh
#   options: --version <v>  --dir <path>  --modify-path  --dry-run   (OMM_VERSION, OMM_INSTALL_DIR)
```

Releases live on the [releases page](https://github.com/hypery11/oh-my-musecode/releases) (`scripts/release.sh`
cuts them; the base is one variable at the top of `install.sh`, `OMM_RELEASE_BASE_URL`). Alternatively,
build from a clone:

```sh
git clone https://github.com/hypery11/oh-my-musecode && cd oh-my-musecode
cargo build --release -p omm && install -m 0755 target/release/omm ~/.local/bin/omm
omm install --source "$PWD"        # a shipped binary must be told the checkout: --source or $OMM_SOURCE
```

`omm` must be on the `PATH` the host inherits: plugin hooks are declared as `omm hook <name>` and the
MCP server as `omm mcp`, and a command word that does not resolve never spawns, silently (doctor D15
is critical when that happens).

Upgrading from the 0.x plugin (Node CLI, Python hooks, role skills)? Read `docs/MIGRATION.md` first —
1.0.0 replaces that surface instead of extending it.

### What `omm install` writes

The complete plan is built and validated before the first host mutation, then executed; a second run
is a no-op. The shape of the output (an earlier sandbox run; byte counts shift release to release):

```
$ omm install --source ~/src/omm
plan: 4 file(s) (32404 B), 3 settings key(s), 3 host mutation(s), trust /home/me/proj
marketplace omm: added (local-dir ~/src/omm)
plugin oh-my-musecode@omm: installed e62ca6db8915… (~/src/omm/plugins/oh-my-musecode)
capabilities: 4 trusted_enabled (4 approved now); 1 left review_needed by design (enabledDefault false): plugin:oh-my-musecode:reminder:omm-verify-nudge — `muse plugins approve <id>` enables one
skills: 35 listed by the host as plugin:oh-my-musecode:*
rules ~/.config/muse/AGENTS.md: updated — written from the template
themes: 3 written, 0 unchanged, 0 skipped, 0 staged
settings: profile omm-default — 3 key(s) set (prior recorded), 0 already current; settings.json created
trust: /home/me/proj trusted (prior recorded); trust.json created
category      updated  unchanged  skipped  backed-up  removed
capabilities        4          0        1          0        0
marketplace         1          0        0          0        0
plugin              1          0        0          0        0
rules               1          0        0          0        0
settings            3          0        0          1        0
themes              3          0        0          0        0
trust               1          0        0          0        0
total              14          0        1          1        0
catalog budget: 35 skill entries, 13264 B full / 13196 B first_sentence of 21542 / 27168 (≈ 3316 / 3299 tokens, bytes/4 heuristic)
```

Everything omm owns lives in `$XDG_CONFIG_HOME/omm/` (the ledger `omm.lock.json`, `audit.log`,
`config.json`, `custom/`, `snapshots/`, `updates/`); into Muse's config root it writes `AGENTS.md`
(only a marked block of it), `themes/*.tmTheme`, three typed `settings.json` keys and one `trust.json`
entry; the plugin itself is installed by `muse plugins install` from the marketplace `omm`.
`--no-plugin` installs the skills into Muse's managed store with `muse skills install` instead. The
full footprint is [ARCHITECTURE.md §2](docs/ARCHITECTURE.md#2-on-disk-footprint-on-the-users-machine).

## Commands

Global flags: `--json` (every read command), `--dry-run` (every write command), `-y/--yes`
(required under `CI=true` or a non-TTY for every write command), `-v/--verbose` (print every host
process omm spawns). Exit codes: `0` ok, `1` ran and failed or refused (a critical doctor row, a
missing `--yes`), `2` rejected before anything ran (bad argv, an unknown feature or profile).

| Command | Does |
|---|---|
| `omm install [--no-plugin] [--source <DIR>] [--workspace <DIR>] [--profile <ID>] [--skip <STEP>…] [--drop-unknown-settings-keys] [--reinstall]` | Install the bundle: marketplace add → plugins install → approve every capability → verify |
| `omm uninstall [--force] [--reconcile-host]` | Remove what the ledger records: two-section preview, deepest-first (R4) |
| `omm update [--source <DIR>] [--reinstall]` | Snapshot → reconcile (R3) → reinstall + re-approve → report staged conflicts |
| `omm reconcile` | Re-check every ledger entry against the disk and the host (doctor D13's fix) |
| `omm trust [PATH]` | Merge a workspace into trust.json (default: cwd) |
| `omm list [--kind <KIND>] [--source <DIR>]` | Resolved assets with provenance and `_shadowed` |
| `omm doctor [--fast] [--self-test] [--report-drift]` | Run the D1–D15 checks with exact fix commands (plus D16, skill routing) |
| `omm cost [--no-cuts]` | Per-source byte table of the skills catalog and the other context budgets |
| `omm lint [PATH] [--no-host]` | Lint content: ids, collisions, symlinks, budgets, host checkpoints |
| `omm build [PATH] [--check]` | Regenerate the native package, projections and marketplace indexes (dev) |
| `omm mcp` | The stdio MCP server the bundle declares (omm_doctor, omm_cost); the host spawns it |
| `omm theme <NAME>` / `omm theme list` | Set tui.theme — validated first, prior value recorded |
| `omm keymap <PRESET>` / `omm keymap list` | Apply a keymap preset |
| `omm profile use <NAME> [--force]` / `list` / `show <NAME>` | Settings profiles: strict / default / fast / ci |
| `omm settings set <KEY> <VALUE>` / `fix-mcp-collision` / `lint [--fix]` / `reconcile-reminders` | Targeted settings.json patches — the fixes doctor D2–D6 print |
| `omm memory path` / `seed` / `list` / `backup [--to <DIR>]` / `gc` | The personal_project memory root |
| `omm enable skill-routing` / `omm disable skill-routing` | Enable / disable an opt-in feature |
| `omm run -- <muse args>` | The launcher shim: allowlist check (R20), controlled env, then exec muse |
| `omm hook <NAME>` | In-binary hook dispatcher: JSON event on stdin, JSON out, fail-open (R16) |

## Profiles

`omm profile use <name>` applies a settings slice as one ledgered transaction (every key's prior
value recorded, validated by the host before landing) and restores the outgoing profile's keys first.

| profile | keys | for |
|---|---|---|
| `default` | `run.context_slimming.*` (3 leaves): `first_sentence` descriptions with `bundled:git` re-listed | what `omm install` applies |
| `strict` | `permissions` + `run` (8 leaves): managed sandbox, `prompt_unmatched`, human reviewer, full descriptions | when nothing may run unasked |
| `fast` | `reasoning_effort: medium` + `run` (5 leaves) | short interactive loops |
| `ci` | `permissions` `allow_all`/no reviewer, one Stop-hook continuation, telemetry, feature_config, session messaging and notifications off (16 leaves) | headless runs |

## `omm doctor` and `omm cost`

Doctor compares what composed in a **live** echo session against what was installed, never disk
against disk; every non-ok row carries the exact fix. Excerpt of a green run (3.7 s; `--fast` skips
the live session and the host self-test, 0.25 s):

```
ok   D1  plugin capabilities  4/4 trusted_enabled (ledger): plugin:omm:hook:omm-guard trusted_enabled; … plugin:omm:mcp_server:doctor trusted_enabled
ok   D8  catalog pressure     26 entries / 26 with description / 9,236 B of 32,000 (22,764 B headroom; bundled 14/14 4,211 B, plugin 12/12 4,627 B)
ok   D11 host drift           69 P0/P1 rows match host-reality.md (3191 ms; Muse Code 1.0.3 (1.0.3-R2198.1))
ok   D12 workspace trust      /home/me/proj is trusted
ok   D15 omm on PATH          `omm` → /home/me/.local/bin/omm (3 hook(s), 1 MCP server(s))
ok   D16 skill routing        off (opt-in): `omm enable skill-routing` in a trusted git workspace turns it on; nothing is routed and nothing is at stake
16 checks: 0 critical, 0 warn — ok (3663 ms, exit 0)
```

and of a machine where the plugin is gone and the workspace untrusted (exit 1):

```
CRIT D1  plugin capabilities  plugin `oh-my-musecode` is not installed (`muse plugins inspect oh-my-musecode` → plugin `oh-my-musecode` is not installed)
        fix: omm install
WARN D12 workspace trust      no trust.json at ~/.config/muse/trust.json: every workspace is untrusted
        fix: omm trust .
```

`omm cost` measures one echo session and prints where the bytes go and what each cut buys:

```
skills catalog (order 200): 9,236 B of 32,000 (22,764 B headroom), 26 entries / 26 with description, header 363 B, footer 35 B, stage Intact, ≈ 2,309 tokens
  source (render order)  entries  with-desc     bytes
  bundled                     14         14     4,211 B
  plugin                      12         12     4,627 B
what to cut (bytes measured, not guessed)
  −20,952 B  exclude the workflow tool
             omm settings set run.context_slimming.excluded_tool_names '["workflow"]'
     −760 B  session_identity off
             omm settings set run.context_slimming.session_identity_enabled false
     −865 B  disable bundled:git
             muse skills disable bundled:git --scope built-in
```

The model can ask for both itself: the bundle declares `omm mcp` as the MCP server `doctor`, whose
tools `omm_doctor {fast?}` and `omm_cost` return the same `--json` documents
(`mcp__plugin_omm_doctor.omm_doctor`; proven end to end with a scripted model in
`tools/mockprovider/run-omm-mcp.sh`).

## Guarantees, each with the test that proves it

All scenarios live in `crates/omm/tests/e2e.rs` and run the release binary in a fresh `HOME` against
the pinned host (`scripts/gate.sh`).

- **The ledger is the primitive** (R1, R2): install, update, doctor and uninstall are one traversal
  over `omm.lock.json`, which records what omm wrote, never a scan. `s11` (a second install is a
  no-op with a byte-identical ledger), `s26` (entries exist before the files do), `s20` (a corrupt
  ledger heals through the printed fixes).
- **Three-way merge, never a clobbered edit** (R3): ancestor = the ledger sha; outcomes no-op /
  overwrite / adopt / stage. `s03` (an edited skill is staged under `updates/<version>/`, disk
  untouched), `s03b` (a theme edit, a changed package), `s14`/`s23` (your text around the managed
  `AGENTS.md` block survives install, update and uninstall).
- **Byte-identical uninstall** (R4, R5): deepest-first, contained (`canonicalize` + `strip_prefix`),
  edits preserved unless `--force`, shared files restored to their pre-omm bytes and mode. `s01`
  (bundle), `s02` (`--no-plugin`), `s04`, `s16`, `s21`, `s15`/`s15b`/`s15c` (an interrupted install
  is undone), `s22`/`s25` (a symlinked store directory is never handed to the host).
- **Cost attribution**: every number is measured in a real session, not estimated. `s12` (a pristine
  install has no Warn/Critical row and the JSON parses), `crates/omm-doctor/tests/cost.rs`.
- **Doctor names the silent failure and its fix**: `s05` (`muse plugins disable omm` → D1 critical,
  the fix heals), `s06` (`mcp_servers` collision → D3), `s07` (theme prior restored), `s27`
  (malformed host config is refused by the plan).
- **Hooks fail open under 5 ms** (R16), **`omm run` refuses prompts** (R20): `s09`, `s08`; the MCP
  server and skill routing have their own suites (`tests/mcp_server.rs`, `tests/routing.rs`).

## What omm deliberately does not do

No enterprise tier and no MSP control plane (struck, not deferred — R21). It never patches, wraps or
replaces the Muse binary; `omm run` only `exec`s it with a controlled environment. It never writes a
key Muse does not type into `settings.json` (R9), never writes `trust.json` beyond one merged entry,
and never installs a skill by copying into the managed store by hand.

## Muse facts you must know

- **`trust.json` gates everything project-scoped.** In a workspace absent from it, project skills,
  hooks, rules, workflows and the `personal_project` memory scope are silently inert. Nothing but
  the TUI writes it; `omm trust .` merges one entry.
- **The skills catalog is capped at 32,000 bytes** and degrades silently: descriptions are dropped
  tail-first (plugin skills first) with no diagnostic. `omm cost` measures it; `first_sentence`
  slimming is the rescue (R18).
- **Muse rewrites `settings.json` from a typed struct** on every settings-mutating verb and destroys
  unknown keys; omm patches typed keys only and puts your untyped keys back after every host rewrite.
- **`MUSE_EXPERIMENTAL_PLUGINS=1` gates the `plugins` verbs at install time only**; runtime
  composition is ungated. A capability runs only in the literal `trusted_enabled` state — `omm
  install` approves and verifies every one, and doctor D1 re-checks.

Open findings with reproductions: [docs/KNOWN_ISSUES.md](docs/KNOWN_ISSUES.md). The host facts
everything is built on: [docs/host-reality.md](docs/host-reality.md).

## Your own content

The overlay lives in `$XDG_CONFIG_HOME/omm/custom/` and is searched before the bundled content
(R7): `custom/skills/<id>/SKILL.md` or `custom/commands/<id>.md` replaces an asset of that id, a
custom id with no bundled counterpart is an addition, `custom/<kind>/<id>_append.md` appends to a
skill, command or rules asset, and `config.json` → `"disabled": ["skill:omm-pdf"]` hides one.
`omm list` shows the resolution with `shadowed` and `append` columns; `omm theme`, `omm profile use`
and `omm keymap` take `custom/themes/`, `custom/profiles/` and `custom/keymaps/` files. The plugin
package is generated from `content/`, so a custom skill is not yet packaged into the installed
plugin — change shipped skills through the repository (below), or edit the installed copy: `omm
update` stages a conflicting upstream instead of overwriting, and `omm uninstall` preserves it.

## Contributing

`content/` is the canonical source and `content/catalog.json` the only list of assets (R8). Edit
there, then `omm build` regenerates `plugins/omm/`, `dist/` and the three marketplace catalogs (all
committed; `omm build --check` is the CI drift gate), and `omm lint` runs every rule and the host's
own validators. `scripts/gate.sh` is the whole gate (fmt, clippy, every test against the pinned host
fetched by `scripts/fetch-host.sh`); `scripts/README.md` documents the scripts and
`docs/INSTALL_FOR_AGENTS.md` is the page written for a coding agent that has to install omm alone.
