---
name: omm-skill-port
description: Use when asked to port, convert, or make a foreign skill (.claude/.codex skill dir, AGENTS.md snippet, .cursor rule) work in Muse: rename tools, fix frontmatter, validate, place; Do not use when authoring a new skill (bundled:create-skill).
argument-hint: "<path-to-source> [project|personal]"
---

# Port a foreign skill to Muse

Output: `<id>/SKILL.md` that validates with `diagnostics: []`, names only Muse
tools, in the right root. Steps in order; never skip 5. `muse skills import` copies
verbatim and rewrites nothing. New skill: `read_skill bundled:create-skill` instead.

## 0. Port, or leave in place?

Muse reads `.claude/skills` and `.codex/skills` (workspace and home) live, tagged
`written-for` and down-weighted. Port when the body names a foreign tool (`Task`,
`TodoWrite`, `Glob` ...) or command idiom (`$ARGUMENTS`, `` !`cmd` ``, `@file`),
relies on `allowed-tools` to narrow itself, or the user wants it first-class (no
foreign tag, top precedence). Plain prose with none of these: say so and stop.

## 1. Read the source; pick id and scope

`read_file` the source and its siblings (`references/`, `scripts/`).

| source | port as |
|---|---|
| `skills/<id>/SKILL.md` under `.claude`, `.codex`, `.claude-plugin`, `.codex-plugin` | skill, same id |
| `.claude/commands/<name>.md` | skill; keep `argument-hint`; `$ARGUMENTS` -> "the text after the skill name" (substitution is proven only for plugin commands) |
| `AGENTS.md` / `CLAUDE.md` snippet | always-on rule: a paragraph in `AGENTS.md`, not a skill; on-demand procedure: skill, id from its heading |
| `.cursor/rules/*.mdc` | `alwaysApply: true`: `AGENTS.md`; else skill; fold `globs` into the description ("when editing `*.ts`") |

Id: `[a-z0-9_-]+`, not a Windows reserved stem (`con prn aux nul com1-9 lpt1-9`),
not a bundled id (`muse skills list --source built-in`; a bare `plan` shadows the
alias of `bundled:plan`). Frontmatter `name` MUST equal the directory name.
Scope: project (default) = `<ws>/.agents/skills/<id>/`; personal (user said
personal / user / cross-project) = draft in `<ws>/.agents/skill-drafts/<id>/`.

## 2. Frontmatter

Keep: `name description argument-hint disable-model-invocation user-invocable
license metadata`. Drop everything else and list each in the report:

- `allowed-tools`: parsed, never enforced, grants and narrows nothing; the one key
  that keeps `validate` from being clean. If the source used it to NARROW itself,
  write the limit as a body rule ("run only `git status`; never write files").
- `agent`, `context`, `hooks`: warned `unsupported-skill-field`; the skill now runs
  in-session, say so. `model`, `tools`, `mcp-servers`, `version`, `globs`,
  `alwaysApply`: silently ignored. Do not add `experimental-gate` unless asked.

## 3. Description, at most 240 chars

One line, ASCII. First sentence = WHEN, standing alone (`first_sentence` slimming
cuts at the first `. `); last sentence "Do not use when ...". Cut trigger lists:
every char (and `metadata.short-description`) is charged to the 32,000 B catalog
of every session; the body costs nothing until `read_skill`.

## 4. Body: rewrite the calls, keep the prose

Core table. Full map with parameters, `.codex` / `.cursor` names, path idioms and
a worked example: `read_file references/tool-map.md` beside this file.

| source says | write |
|---|---|
| `Read` `Write` `Edit` `MultiEdit` `NotebookEdit` `apply_patch` | `read_file` `write_file` `edit_file` (one `find` / `replace` per call) |
| `Glob` `LS` `list_dir` `file_search` | `search` with `glob: ["**/<name>"]`, `pattern: "^"`, `mode: "regex"`, `output_mode: "files_with_matches"` |
| `Grep` `codebase_search` `grep_search`, "grep -r" | `search` (`pattern`, `mode`, `paths`, `glob`), scoped to a subtree |
| `Bash` `shell` `run_terminal_cmd` `` !`cmd` `` | `bash`; slow: larger `yield_time_ms`; stdin or kill: `bash_input`; never poll |
| `TodoWrite` `update_plan` `todo_write` | `write_todos` (full list, exactly one `in_progress`) |
| `Task` / `Agent` (`subagent_type`) | `subagent_spawn` (`objective`, `subagent_type`, `worktree_isolation`), `subagent_wait`, `subagent_read_result` |
| `Skill` `SlashCommand` `/name` | `read_skill <id>` (`bundled:<id>` for a built-in); slash commands do not port: inline or drop |
| `WebSearch` / `WebFetch` | `web_search` / `web_fetch` (gated: write "if available", else `bash` `curl`) |
| `AskUserQuestion` | ask in the reply and end the turn |
| memory notes, `CLAUDE.md` facts | `read_memory` `add_memory` `edit_memory`; the rules file is `AGENTS.md` |

Rules:

- `subagent_*` is hidden in the TUI unless `run.subagent_delegation_mode` is `auto`:
  `read_file` `$CONFIG_DIR/settings.json`; if not `auto`, write "do this inline".
  A named `subagent_type` must exist under `.agents/agents/` or `$CONFIG_DIR/agents/`;
  else omit it and inline the persona text into `objective`.
- Paths: `.claude/skills` -> `.agents/skills`; `~/.claude/skills` -> `$CONFIG_DIR/skills`;
  `$CLAUDE_PROJECT_DIR` -> `$PWD` (empty in Muse); "the X hook runs" -> delete.
- All other prose stays byte-for-byte. Improving the skill is a second change, after
  the port validates, only if asked.
- Self-check: `search` the new body, `mode: "regex"`, for
  `Task|TodoWrite|Glob|Grep|MultiEdit|allowed-tools|\.claude/|\$CLAUDE_`. Every hit
  is rewritten or a deliberate mention of the source format, named in the report.
  Body <= 8 KB; overflow to `references/*.md`.

## 5. Files and validate

No BOM, no symlinks (a symlinked `SKILL.md` is never loaded), ASCII file names, no
`.muse` files. Copy `references/` and `scripts/` verbatim; a script that needs a
binary absent from `PATH` makes the skill tool-specific: state it in the body.

```
muse skills validate <dir> --json
```

Done means `valid: true` AND `diagnostics: []`; a warning (usually a leftover
`allowed-tools`) is not done. Fix, rerun; stop after three rounds and report.

## 6. Place

- Project: `muse skills list --source project --trust-workspace --json` lists the id
  with `diagnostics: []`. Without trust the workspace loads NO project skills, with
  no error at session time: hand the user `omm trust .`.
- Personal: hand the user `muse skills install .agents/skill-drafts/<id>`; the store
  writes `$CONFIG_DIR/skills/<id>/` with lockfile provenance. Never write into
  `$CONFIG_DIR/skills`, `~/.agents/skills`, `~/.claude/skills` or `~/.codex/skills`.
  Delete the draft after install.
- The original: `.agents` > `.codex` > `.claude` in one scope, and project > user,
  so a workspace `.claude/skills` copy OUTRANKS a personal port: port at project
  scope or delete the copy. A shadowed original (`skill-shadowed`) is harmless;
  keep it while the other agent still runs on this repo. Two dirs with one `name`
  in one root drop BOTH (`duplicate-active-skill-id`).

## 7. Report

```
Ported:   <src> -> <dst>
Validate: muse skills validate <dst> --json -> valid: true, diagnostics: []
Dropped:  allowed-tools (advisory), model (ignored), ...
Rewrites: Glob -> search(glob), Read -> read_file, Task(x) -> inline, ...
Lost:     <permission narrowing | agent scoping | model pin | none>
Next:     omm trust . | muse skills install .agents/skill-drafts/<id> | original kept (shadowed)
```

Never say "works in Muse" without the validate output in the reply.
