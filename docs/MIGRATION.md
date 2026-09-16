# Migrating 0.3.0 → 1.0.0

1.0.0 is a rewrite, not an upgrade. The Node CLI (`bin/omm.mjs`), the Python hooks (`hooks/*.py`), the 19 role skills, and the 19 slash-commands are all replaced. Read this before updating.

## What changed

| 0.3.0 | 1.0.0 | Notes |
|---|---|---|
| `npm i -g oh-my-musecode`, `bin/omm.mjs` | `install.sh` from releases, one Rust binary | No runtime, no dependencies. `npm` package is retired. |
| `hooks/*.py` (8 scripts + helper) | 8 hooks dispatched by `omm hook <name>` | Same events. Install the binary or the hooks do nothing. |
| 19 role skills (`analyst` … `writer`) | 35 `omm-*` skills | Rough mapping below. Old ids are gone. |
| 19 slash-commands (`/ralph`, `/team`, …) | 3 (`/omm-doctor`, `/omm-cost`, `/omm-status`) | Workflows moved into skills; the agent picks them up without a command. |
| Loose `.omm/*.md` state | Ledger + `session.jsonl` under `.omm/` | Your old notes stay on disk; nothing imports them. |
| `MUSE_EXPERIMENTAL_PLUGINS=1` required | Still required | Host plugins are still experimental. |

## Skill mapping

Old id → closest new skill. Approximate — read the new SKILL.md, the shape changed from "role you play" to "job to do":

| 0.3.0 | 1.0.0 |
|---|---|
| analyst | omm-errors / omm-observability |
| architect | omm-api-design |
| code-reviewer | omm-review |
| code-simplifier | omm-refactor / omm-cleanup |
| critic | omm-review |
| debugger | omm-debug |
| designer | omm-frontend / omm-api-design |
| document-specialist | omm-docs |
| executor | omm-tdd |
| explore | omm-repo-map |
| git-master | omm-commit / omm-pr |
| planner | omm-pr (breakdown) / omm-handoff |
| qa-tester | omm-test-design |
| scientist | omm-debug (bisect) / omm-perf |
| security-reviewer | omm-security |
| test-engineer | omm-tdd / omm-test-design |
| tracer | omm-observability |
| verifier | omm-verify |
| writer | omm-docs |

New with no predecessor: omm-ci-fix, omm-release, omm-migrate, omm-incident, omm-legacy, omm-config, omm-concurrency, omm-shell, omm-rust, omm-typescript, omm-python, omm-database, omm-containers, omm-parallel, omm-reflect, omm-handoff, omm-self, omm-skill-port.

Gone with no successor: `/ralph`, `/team`, `/ask`, `/hud`, `/autopilot`, `/ultragoal`, `/ralplan` and the rest of the 19 commands. The Ralph stop-loop, the team files, and the HUD snapshot do not exist in 1.0.0. If you depended on them, stay on 0.3.0 — the old tag is still there.

## `.omm/` state

1.0.0 keeps workspace state under the same `.omm/` directory but in a new shape (a ledger plus `session.jsonl` and a team log). It does not read 0.3.0 files (`ralph.json`, `mode.json`, `ultragoal.md`, …) and does not delete them either. What to do:

1. Finish or abandon any active Ralph loop first (`ralph.json` will simply be ignored).
2. Copy out anything you want to keep — `memory.md`, `handoff.md`, `plan.md`, `architecture.md` are plain text and stay readable.
3. Update the plugin, run `/omm-doctor`, and let 1.0.0 initialise fresh state.

## Update steps

    muse plugins marketplace add omm https://github.com/hypery11/oh-my-musecode
    muse plugins install oh-my-musecode@omm
    muse plugins approve oh-my-musecode
    curl -fsSL https://github.com/hypery11/oh-my-musecode/releases/latest/download/install.sh | sh

Then `/omm-doctor`. If hooks were approved for 0.3.0, approve again — the hook sources changed, which resets approval.
