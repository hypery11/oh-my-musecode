# Installing omm without a human

This page is for a coding agent that has to install, verify and (if asked) remove `omm` on a
machine with no one to answer prompts. Every command below exists with exactly these flags
(`omm <cmd> --help` is the source of truth); every number was measured against Muse Code
`1.0.3-R2198.1` and re-verified on `1.0.1-R2006.1` (the previous stable build; doctor D11 reports the
rows that build predates as `OLDER-BUILD`, a pass).

## 0. Rules

- Never edit `settings.json`, `trust.json` or `omm.lock.json` by hand. The commands below are the
  only supported way to change omm's state, and every one of them records what it did.
- Every write command refuses under `CI=true` or a non-TTY without `--yes` (exit 1, nothing
  written). Pass `--yes` deliberately; preview with `--dry-run` first when the action is not the one
  you were asked for.
- Read `--json` output, not prose. Under `--json` an error is one object on **stderr**,
  `{"error": "<message>", "exit_code": <n>}`, and stdout is empty.
- Run omm from the workspace you want configured: `trust`, `memory`, `enable`/`disable` and doctor
  D12 all take the current directory as the workspace.

## 1. Prerequisites

1. Muse Code is installed and resolvable: `$OMM_MUSE_BIN` (a `muse-bin-<version>` path), else the
   launcher's `~/.local/bin/.muse-version` → `muse-bin-<version>`, else `muse` on `PATH`.
   `omm doctor --fast` reports the binary it found on its first line.
2. The `omm` binary is on the `PATH` the host will inherit (the plugin declares `omm hook <name>` and
   `omm mcp` as bare words). Put it in `~/.local/bin` and make sure that directory is on `PATH` for
   login shells; doctor D15 is critical otherwise.
3. A content checkout. A shipped binary does not know where the repository is: pass
   `--source <checkout>` or export `OMM_SOURCE=<checkout>` on the first `omm install`; afterwards the
   ledger remembers it. The checkout must hold `content/`, `marketplace.json` and `plugins/omm/`.
4. The working directory is the workspace to trust — a git checkout. `omm install` trusts it
   (`--workspace <DIR>` picks another; `--skip trust` leaves it out).

Placing the binary:

```sh
curl -fsSL https://github.com/hypery11/oh-my-musecode/releases/latest/download/install.sh | sh -s -- --modify-path
# or from a clone: cargo build --release -p omm && install -m 0755 target/release/omm ~/.local/bin/omm
omm --version            # omm 0.1.0
```

`install.sh` writes `$OMM_INSTALL_DIR/omm` (default `~/.local/bin/omm`) and, with `--modify-path`,
one line in the shell profile; nothing else. Its last line is always "run `omm install` when ready".

## 2. Install

```sh
cd <workspace>
omm install --source <checkout> --dry-run          # the plan; nothing written; exit 0
omm install --source <checkout> --yes --json        # exit 0 = installed and verified
omm doctor --json                                   # exit 0 = no critical row
```

What a successful install does, in order: `muse plugins marketplace add omm <checkout>` → `plugins
install oh-my-musecode@omm` (asserting `manifest_family == "native"` and the package digest) → `plugins approve`
for every declared capability → `plugins inspect --json` verifying each is literally
`trusted_enabled` → `AGENTS.md` managed block, three themes, the `default` profile's three settings
keys, the trust entry. The ledger `$XDG_CONFIG_HOME/omm/omm.lock.json` is saved after every host
mutation, so an interrupted run is always undoable.

Reading the `--json` document:

| field | meaning |
|---|---|
| `mode` | `plugin` or `no-plugin` |
| `plan.files[].contained` / `.refused` | every file write with its resolved path; a `refused` entry means nothing was written at all (the message names the way out: `--skip themes`, `OMM_THEMES_DIR`) |
| `bundle.trusted` | the capability ids verified `trusted_enabled`; `bundle.unapproved_by_design` lists reminders shipped `enabledDefault: false` (expected: `plugin:omm:reminder:omm-verify-nudge`) |
| `bundle.skills_listed` / `skills_missing` | 12 / `[]` on this release |
| `settings.set` / `settings.unchanged`, `trust.action` | the settings keys and the trust merge |
| `budget.within_budget` | the catalog estimate is under the 21,542 B bundle budget |
| `converge.noop` | `true` on a rerun that changed nothing |

Variants: `--no-plugin` installs the thirty-five skills into Muse's managed store with `muse skills
install` instead of the plugin (no hooks, no MCP server, no reminder); `--profile strict|fast|ci`
applies another slice; `--reinstall` removes and installs the plugin again even when unchanged (the
fix doctor D1/D10 print); `--drop-unknown-settings-keys` proceeds when `settings.json` holds keys
the host does not type (the install refuses up front otherwise and names them).

## 3. Exit codes

| code | meaning | what to do |
|---|---|---|
| `0` | ran; for `doctor`, no critical row | continue |
| `1` | ran and failed or refused: missing `--yes`, a host verb failed, a containment refusal, a critical doctor row, `settings lint` with problems | read stderr (or the `error` object); fix the named cause; rerun — every command is idempotent |
| `2` | rejected before anything ran: bad argv, unknown feature/profile/theme, `omm run` refusing a prompt | fix the invocation |

The host's own codes, seen through `-v`: `2` = argv rejected (parse error or a missing gate, indistinguishable), `1` = ran and failed.

## 4. Doctor findings and what to do

`omm doctor --json` → `{host, ok, exit_code, elapsed_ms, checks[]}`; each check is
`{id, title, severity: "info"|"warn"|"critical", observed, why_silent, fix}`. Act on `fix` verbatim;
`fix` is `null` on a pass. `--fast` skips D8 (the live echo session) and D11 (the host self-test),
0.2 s instead of 3.7 s. Info rows are allowed in a green run (a pristine install never sets
`settings.provider`, so D2 is Info).

| id | finding | fix it prints |
|---|---|---|
| D1 | a declared hook / MCP server / reminder line missing or not `trusted_enabled`; plugin not installed | `muse plugins approve plugin:omm:<kind>:<id>` · `omm install` · `omm install --reinstall` |
| D2 | `settings.provider` unset (Info; Warn only when omm set it and it is gone) | `omm settings set provider meta` |
| D3 | `mcpServers` and `mcp_servers` both present — every settings-writing verb exits 1 | `omm settings fix-mcp-collision` |
| D4 | malformed `plugins` / `runtime_capabilities`; `permissions` without `schema_version` | `omm settings lint --fix` · `omm settings set permissions.schema_version 1` |
| D5 | canonical and legacy reminder enablement disagree | `omm settings reconcile-reminders` |
| D6 | `agent_definitions.safe_mode: true` | `omm settings set agent_definitions.safe_mode false` |
| D7 | enabled plugins near the 256 ceiling | `muse plugins disable <id>` |
| D8 | catalog over 32,000 B or descriptions silently dropped | `omm settings set run.context_slimming.skill_catalog_descriptions first_sentence` · `muse skills disable bundled:<id> --scope built-in` · `omm cost` |
| D9 | enterprise config rows | informational |
| D10 | installed `omm` is not `manifest_family == "native"` | `omm install --reinstall` |
| D11 | a golden constant drifted from the live host | `omm doctor --report-drift` (report it; do not install over drift) |
| D12 | workspace not in `trust.json` | `omm trust .` |
| D13 | ledger corrupt / missing / entries whose file vanished / unlisted files | `omm reconcile`, then `omm install [--no-plugin]` when it still misses files |
| D14 | exit-code matrix or allowlist sanity failed | report it |
| D15 | `omm` (or another hook/MCP command word) does not resolve on `PATH` | `export PATH="<dir of omm>:$PATH"` and add that line to the shell profile |
| D16 | skill routing on but stale: untrusted workspace, handler missing, budget within the margin | `omm enable skill-routing` · `omm trust <ws>` · `omm settings set run.context_slimming.skill_catalog_descriptions first_sentence` |

Loop: run the fix, rerun `omm doctor --json`, stop when `ok` is `true`. Never gate on the Muse
version string; D11 probes behaviour.

## 5. Verify (R5)

The e2e gate asserts that install followed by uninstall leaves the config and data roots byte- and
mode-identical, excluding only the host's own residue. To verify on a machine:

```sh
snap() { (cd ~/.config && find . -type f -not -path './muse/.*.lock' -not -path './muse/skills/.muse/*' -exec shasum -a 256 {} + | sort); }
before="$(snap)"
omm install --source <checkout> --yes --json >/dev/null && omm doctor --fast --json | jq -e .ok
omm uninstall --dry-run           # ✗ remove / ✓ preserve / · residue kept — read it
omm uninstall --yes --json >/dev/null
[ "$before" = "$(snap)" ] && echo identical
```

(The excluded paths are the host's own: its two startup locks and the managed store's metadata,
which `muse skills install` writes on a `--no-plugin` install.)

Host-owned residue that is named and kept on purpose: `$XDG_DATA_HOME/muse/{sessions,local-tracing,
plugins}/`, `$XDG_CONFIG_HOME/muse/{.settings.json.lock,.auth.json.lock,skills/.muse/}`,
`~/Library/Application Support/Muse/session-name-authority/` and `/private/tmp/tbh-<uid>-rt/muse/`.

Other checks: `omm list --json` (every asset with its provenance), `omm cost --json` (the catalog
bytes per source; `catalog.stage` must be `intact`), `omm run --dry-run -- --version` (the exact
binary and environment `omm run` would exec).

## 6. Optional steps

```sh
omm profile use strict --yes           # or fast / ci; `omm profile show <name>` diffs it first
omm theme omm-carbon --yes             # `omm theme list`
omm memory seed --yes                  # MEMORY.md + sidecar in the personal_project root; needs trust
omm enable skill-routing --yes         # trusted git workspace only; `omm run` then exports the two gates
omm disable skill-routing --yes        # byte-for-byte reversal; run it before `omm uninstall`
```

Start Muse through `omm run -- <muse args>`: the first token must be a Muse command (an unknown word
would be a prompt and start a billed session; refused with exit 2), `MUSE_NO_AUTO_UPDATE=1` and the
profile's gates are exported, and the routing gates when routing is on.

## 7. Update

```sh
omm update --dry-run                   # snapshot → three-way reconcile → what would be staged
omm update --yes --json
```

A file you edited that upstream also changed is staged under `$XDG_CONFIG_HOME/omm/updates/<version>/`
and reported; the disk copy is untouched. `--source <DIR>` re-registers the marketplace from another
checkout.

## 8. Uninstall

```sh
omm uninstall --dry-run                # the preview: ✗ remove, ✓ preserve, ! beyond the ledger, · residue
omm uninstall --yes --json
```

Preserved (listed under ✓, kept): any ledgered file whose bytes differ from what omm wrote —
`--force` removes those too. `!` names what the host holds of omm's that the ledger does not list (an
interrupted install); the uninstall refuses until `--reconcile-host` is given. Settings keys go back
to their recorded prior, `settings.json`/`trust.json` to their pre-omm bytes and mode, and the
ledger is removed last. `omm doctor --fast` afterwards reports D1 critical with the fix `omm install`
— that is the expected state of a machine without omm.
