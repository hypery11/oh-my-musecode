# Tool map: foreign agent -> Muse

Default Muse session tools: `read_file search write_file edit_file bash bash_input
read_skill write_todos read_memory add_memory edit_memory web_search workflow` plus the
six `subagent_*` tools (`spawn status send_message wait read_result cancel`), which the
TUI hides unless `settings.json` has `run.subagent_delegation_mode: "auto"`.
Gated, may be absent (write "if available" and give a fallback): `web_fetch`,
`request_user_input`, `monitor`, `shell`, `artifact`.
Nothing named `Task`, `TodoWrite`, `Read`, `Write`, `Edit`, `Glob`, `Grep` or
capitalised `Bash` exists. The permission catalog accepts a few aliases
(`Write -> write_file`, `WebSearch -> web_search`, `search_files -> search`,
`exec_command -> bash_input`): compat plumbing, never write an alias into a body.

## Files and search

| `.claude` family | `.codex` family | `.cursor` family | Muse | notes |
|---|---|---|---|---|
| `Read(file_path, offset, limit)` | `read_file` | `read_file` | `read_file` | |
| `Write(file_path, content)` | `apply_patch` (new file) | `edit_file` (new file) | `write_file` | |
| `Edit(old_string, new_string)` `MultiEdit` `NotebookEdit` | `apply_patch` | `edit_file` `search_replace` | `edit_file(path, find, replace)` | one unique match per call; a `MultiEdit` becomes several calls |
| `Glob(pattern)` `LS` | `shell find/ls` | `list_dir` `file_search` | `search(glob: ["<pattern>"], pattern: "^", mode: "regex", output_mode: "files_with_matches")` | `pattern` never matches file names; `glob` does |
| `Grep(pattern, path, glob, -i)` | `shell rg/grep` | `grep_search` `codebase_search` | `search(pattern, mode: regex\|literal, paths, glob, case_sensitive, smart_case)` | default `mode` is literal; bounded, scope to a subtree |
| - | - | `delete_file` | `bash` `rm <path>` | |

## Shell

| foreign | Muse | notes |
|---|---|---|
| `Bash(command, timeout, run_in_background)` `run_terminal_cmd` `shell` `exec_command` `execute_command` | `bash(command, yield_time_ms <= 300000)` | a command still running after the wait returns a session id; final output arrives by itself |
| `BashOutput` / "check the background job" | nothing | do not poll; do not narrate backgrounding |
| `KillShell` / send keystrokes to a running process | `bash_input` | session id from the `bash` result |
| `` !`cmd` `` inside a command body | "run `cmd` with `bash`" | no preprocessing step exists in a skill body |

## Todos and plans

`TodoWrite`, `update_plan`, `todo_write` -> `write_todos(todos: [{text, status}])`.
Always send the FULL list, status in `pending in_progress completed cancelled`,
exactly one item `in_progress`, skip it for a task under three steps.

## Subagents

`Task` / `Agent` -> the `subagent_*` family. Requires
`run.subagent_delegation_mode: "auto"` (TUI default `off`: "native subagent tools are
hidden"). If the tools are absent, do the work inline and say so once.

| foreign idiom | Muse |
|---|---|
| `Task(subagent_type: "general-purpose", prompt)` | `subagent_spawn(objective)` with `subagent_type` omitted (resolves to `general-purpose`) |
| `Task(subagent_type: "<custom>")` | `subagent_spawn(objective, subagent_type: "<custom>")` only if that agent definition exists (`.agents/agents/**/*.md` or `$CONFIG_DIR/agents/**/*.md`, name grammar `[a-z]+(-[a-z]+)*`; built-ins `general-purpose`, `workflow-subagent`); otherwise omit and inline the persona text into `objective` |
| wait for the result | `subagent_wait(timeout_ms 10000..300000, default 30000)`; expiry leaves the child running; then `subagent_read_result` |
| `run_in_background` + poll | spawn without wait; `subagent_status`; `subagent_read_result` when ready |
| "run N agents in parallel" | N `subagent_spawn` calls, then wait on each; or the same steps inline in order |
| parallel writers / "use a worktree" | per child `worktree_isolation: true`; keep read-only children in the shared checkout |
| "tell the agent to ..." mid-run | `subagent_send_message` |
| abort | `subagent_cancel` |

Capacity is 8 agents including the root by default (`agents.execution_capacity`,
1..64); a full pool rejects the spawn (`root_capacity_exhausted`): wait, then retry.

## Web, memory, user, skills

| foreign | Muse | notes |
|---|---|---|
| `WebSearch` `web_search` | `web_search` | |
| `WebFetch(url)` `fetch` | `web_fetch` | gated; fallback: `bash` `curl -sL <url>` (may be sandboxed) or ask the user to paste |
| memory files, "remember that ..." | `read_memory`, `add_memory(scope, path, content, type, description)`, `edit_memory(scope, path, old_str, new_str)` | scopes `personal`, `personal_project` (default), `project` |
| `AskUserQuestion` | `request_user_input` only if listed in the session's tools; else end the turn with the question | |
| `Skill(name)`, "invoke the X skill", `SlashCommand` | `read_skill(name)` | ids: bare `<id>` (project/user), `bundled:<id>`, `plugin:<pid>:<id>` |
| `/x` slash command in prose | `read_skill x` if it is a skill; else inline the command body or drop it | custom slash commands come only from plugin `commands/*.md`; a skill cannot carry one |

## Paths and idioms with no counterpart

| foreign | Muse |
|---|---|
| `.claude/skills/`, `.codex/skills/` (project) | `.agents/skills/` |
| `~/.claude/skills/`, `~/.codex/skills/` | `$CONFIG_DIR/skills/` (`$XDG_CONFIG_HOME/muse` else `~/.config/muse`) |
| `CLAUDE.md` (rules) | `AGENTS.md` (project: VCS root down to cwd, `CLAUDE.md` is a same-directory fallback; personal: `$CONFIG_DIR/AGENTS.md`) |
| `$ARGUMENTS`, `$1` | "the text the user typed after the skill name"; substitution is proven only for plugin `commands/*.md` |
| `@path/to/file` | `read_file path/to/file` |
| `${CLAUDE_PLUGIN_ROOT}` | a path relative to the skill directory |
| `$CLAUDE_PROJECT_DIR` | `$PWD` (the variable is empty in Muse) |
| `.claude/settings.json` hooks, frontmatter `hooks:` | delete; hooks live in `<ws>/.muse/hooks.json`, the `hooks` key of `settings.json`, or a plugin |
| `allowed-tools: Bash(git status:*)` as a guard | body rule: "run only `git status`; never write"; real narrowing is a permission profile or hook, never frontmatter |
| `model:` pin | delete; say nothing about the model |
| `context: fork`, `agent:` | delete; the skill runs in-session; note it in the report |
| `.mdc` `globs: **/*.ts` | description clause: "Use when editing `*.ts` files" |
| `.mdc` `alwaysApply: true`, `.cursorrules` | not a skill: the text belongs in `AGENTS.md` |

## Source shapes

- `.claude-plugin` / `.codex-plugin` package: `skills/<id>/SKILL.md` beside
  `commands/*.md`, `agents/*.md`, `hooks/hooks.json`. Port one `skills/<id>` at a time.
  A whole package installs into Muse unchanged (skills, commands, hooks, MCP
  translated; agents dropped): port single skills only when the user wants them
  native, not to make the package load.
- `.claude/skills`, `.codex/skills`: read live, tagged `origin-family`. Precedence in a
  workspace: `.agents/skills` > `.codex/skills` > `.claude/skills`; in home:
  `$CONFIG_DIR/skills` > `~/.agents/skills` > `~/.claude/skills` > `~/.codex/skills`;
  project > user across scopes. A losing copy is reported `skill-shadowed`. A home
  source can be scanned free with `muse skills import --from claude|codex --dry-run
  --json` (`unsupported_fields`, `unavailable_binaries`); a body naming a binary absent
  from `PATH` is classified `tool-specific` and a real import quarantines it.
- `AGENTS.md` / `CLAUDE.md` snippet: a rule (short, "always"/"never") goes into
  `AGENTS.md`; a procedure (numbered steps, invoked on demand) becomes a skill, id from
  its heading, description from its first sentence. Rules budget: 256,000 B per file,
  65,536 B aggregate, warned on stderr.
- `.cursor/rules/*.mdc`: frontmatter `description`, `globs`, `alwaysApply`. Muse has no
  glob-scoped rule or skill.

## Self-check regex

`search` the ported body, `mode: "regex"`, for:

```
Task|TodoWrite|MultiEdit|NotebookEdit|AskUserQuestion|BashOutput|KillShell|Glob|Grep|SlashCommand|apply_patch|update_plan|run_terminal_cmd|codebase_search|grep_search|allowed-tools|\.claude/|\.codex/|\.cursor/|\$CLAUDE_|CLAUDE\.md
```

Every hit is either rewritten or a deliberate mention of the source format (say so in
the report).

## Worked example

Source `.claude/skills/changelog/SKILL.md`:

```
---
name: changelog
description: Update CHANGELOG.md
allowed-tools: Read, Edit, Bash(git log:*)
model: haiku
---
Use Glob to find CHANGELOG.md, Read it, run !`git log --oneline -20`.
Track steps with TodoWrite. Use the Task tool with subagent_type=reviewer to check.
```

Ported `.agents/skills/changelog/SKILL.md`:

```
---
name: changelog
description: Use when asked to update the changelog: add entries to CHANGELOG.md from recent commits. Do not use when tagging releases or writing commit messages.
---
`search` with `glob: ["**/CHANGELOG.md"]`, `pattern: "^"`, `mode: "regex"`,
`output_mode: "files_with_matches"`, then `read_file` it. `bash`: `git log --oneline -20`.
Track steps with `write_todos`. Review the diff yourself before finishing.
```

Report: dropped `allowed-tools` (advisory) and `model` (ignored); `Glob -> search(glob)`,
`Read -> read_file`, `` !`git` -> bash ``, `TodoWrite -> write_todos`; `Task(reviewer)`
inlined because no `reviewer` agent definition exists and delegation mode is `off`.
