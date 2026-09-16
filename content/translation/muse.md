# Tool vocabulary: foreign harness names -> Muse Code

Muse's tool names are its own (research/musecode/00-SYNTHESIS.md §3.3). A skill body that says
"use the Task tool" or "call TodoWrite" names something that does not exist, and `run.toolset`
rejects every foreign alias. This block is what `omm skill-port` prepends to a ported skill and what
authors paste into anything written for another harness.

## Preamble (prepend to a ported skill body, verbatim)

> This skill was written for another coding harness. On Muse Code use Muse's tool names only: read_file, write_file, edit_file, search, bash and bash_input, write_todos, read_skill, read_memory / add_memory / edit_memory, web_search, web_fetch (gated), workflow, and subagent_spawn / subagent_status / subagent_send_message / subagent_wait / subagent_read_result / subagent_cancel.
> There is no Task, TodoWrite, Read, Write, Edit, MultiEdit, Glob, Grep, WebSearch, WebFetch or AskUserQuestion tool; where this text names one, use the Muse tool it maps to in omm's translation table (content/translation/muse.md).
> `allowed-tools` in frontmatter is advisory on Muse and neither grants nor restricts anything; what a session may do comes from its permission profile.
> Skill bodies load on demand through read_skill by id; a `plugin://` or `bundled://` path is a display locator, not a file, so never pass it to read_file.

## Table

| Foreign name | Muse tool | Semantics on Muse (measured on 1.0.1-R2006.1) |
|---|---|---|
| `Task` / `Agent` (spawn a subagent) | `subagent_spawn` | Required `command_id`, `role`, `objective`; optional `task_name` (<= 80), `subagent_type`, `worktree_isolation`, `output_schema`. Pool = 8 agents by default incl. the root (`agents.execution_capacity` 1..64); `root_capacity_exhausted` means wait for a child to finish, not retry. Children may queue and start when a slot frees. An untrusted workspace has no `subagent_*` tools at all. |
| `subagent_type` (`general-purpose`, `Explore`, `Plan`, a custom agent name) | `subagent_type` | Optional. Names an Agent Definition (a Markdown file under `agents/` of a trusted native plugin or the workspace; id grammar `[a-z]+(-[a-z]+)*`), not a built-in persona. Omit or `null` for the unscoped general-purpose identity. `agent_definitions.safe_mode: true` disables every user/project/plugin definition, so a named type silently falls back or fails: prefer omitting it unless omm ships that agent. |
| `worktree_isolation` (foreign `isolation: worktree`) | `worktree_isolation: true` or `{}` | Per child. Use it when children may write in parallel (concurrent writers corrupt a shared checkout); keep read-only children in the shared checkout. May be unavailable for the profile or workspace. |
| `TaskOutput` / waiting on a subagent | `subagent_wait`, `subagent_read_result` | `subagent_wait` blocks 30 s by default (`timeout_ms` 10000..300000); `timeout` / `would_park` = still running, and finished results are delivered automatically when the session is idle. `subagent_read_result` collects a finished child's result. |
| messaging / polling / killing a subagent | `subagent_send_message`, `subagent_status`, `subagent_cancel` | Send follow-up text; read state; abort. |
| `TodoWrite` / `TodoRead` / `update_plan` | `write_todos` | One call submits the FULL list `[{text, status}]`, status in pending / in_progress / completed / cancelled, exactly one `in_progress`. No read tool (the user sees the list live). Skip it for tasks under three steps. Not removable from the toolset. |
| `Read` | `read_file` | ONE regular file as a line-numbered window (`offset` 1-based, `limit` default 500); images and MP4/MOV attach as model-visible output. A directory path fails: list directories with `bash`. Relative paths resolve from the Active Workspace Root; a shell `cd` never moves it. |
| `Write` | `write_file` | `path` + complete UTF-8 `content`, create or overwrite. Large file: write a small first chunk, then grow it with `edit_file` (one huge write can fail to send). |
| `Edit` / `MultiEdit` / `NotebookEdit` / `apply_patch` | `edit_file` | `path`, `find` (exact, must be unique), `replace`. No `replace_all`, no batched edits (issue several calls); notebooks are edited as JSON text; there is no patch tool. |
| `Glob` | `search` with `glob: ["**/<name-pattern>"]`, `output_mode: "files_with_matches"`, `pattern: "^"`, `mode: "regex"` | There is no file-name tool. `pattern` never matches paths; names are matched only through the `glob` include/exclude list (`!` prefix excludes). |
| `Grep` | `search` | Ripgrep semantics, policy- and output-bounded. `pattern` is LITERAL by default (`mode: "regex"` to opt in); `paths`, `glob`, `case_sensitive` / `smart_case`, word and whole-line flags, before/after context, `max_matches`, `hidden`, `no_ignore`, `binary`, `output_mode` text / json / files_with_matches. Prefer it over `rg` / `grep -r` / `find | xargs grep` in bash: a recursive scan at the workspace root is refused by the bash policy text. |
| `Bash` | `bash` | `command` plus optional `workdir`, `shell`, `login`, `tty`, `yield_time_ms` (<= 300000; default foreground wait is 10 s, so pass it for slow builds and tests), `timeout_ms`, `max_output_tokens`, `description`, `sandbox_permissions`. A command still running after the wait returns a `session_id`; its final output arrives later as runtime context. Do not poll for it and do not narrate backgrounding. Runs inside the session's filesystem sandbox. |
| `KillShell` / interactive stdin | `bash_input` | Send input to, or terminate, a live `bash` session by `session_id`; only for interactive processes and overdue-notice cleanup. Cannot be excluded while `bash` is enabled. |
| `BashOutput` | none | Output of a backgrounded `bash` call is delivered automatically; there is no poll tool. |
| `WebSearch` | `web_search` | Returns title, URL, snippet. Governed by `tools.web_search.mode` client / hosted / off; when the tool is absent say so, do not emulate it with bash. |
| `WebFetch` | `web_fetch` (gated) | Present only when `tools.web_fetch.enabled` and its gate are on; limits in `tools.web_fetch.*`. When absent, tell the user; do not `curl` around it. |
| `AskUserQuestion` | `request_user_input` (gated); otherwise plain text | Exists only under `--user-input-auto-resolve` / its gate: 1-3 structured questions, headers <= 10 ASCII chars, answers never grant filesystem, shell, network or approval authority. In every other session there is no question tool: ask in one short message and end the turn. |
| `Skill` / `SlashCommand` / "invoke skill X" | `read_skill` | `name` = a skill id (`plugin:omm:<id>`, `bundled:<id>`, or the bare id for user and project skills) or its display path from the catalog. Slash commands are user-typed prompt templates; the model cannot invoke them. |
| "remember this" / editing CLAUDE.md as memory | `add_memory`, `edit_memory`, `read_memory` | Local Markdown memory under `$XDG_DATA_HOME/muse/memory/{personal, projects/<slug>-<fnv1a64>}/`. `add_memory` creates or appends, never overwrites; `edit_memory` replaces exactly; `read_memory` reads a bounded window. The per-session memory snapshot is capped at 16,305 B / 48 files, silently. |
| `LS` | `bash` with `ls` | No directory tool. |
| `Workflow` / `/deep-research` | `workflow` | Muse's workflow runner. Absent when `run.workflow_trigger_mode: "off"` or `run.context_slimming.excluded_tool_names` contains `workflow` (omm's fast and ci profiles): then say workflows are unavailable. |
| `EnterPlanMode` / `ExitPlanMode` | none | No plan mode. Load `bundled:plan` with `read_skill`, write the plan, and stop for approval in text. |
| `mcp__<server>__<tool>` | `mcp__plugin_<pid>_<sid>.<tool>` | MCP tools are a namespace group; call `<ns>.<tool>` (works at every length). The `<ns>__<tool>` spelling only works while `len(pid) + len(sid) <= 18`. |

## Porting rules that follow

1. Rewrite tool names in the body; never leave a foreign name as an "alias" hint, because the model
   will try it and get `tool unavailable: unknown tool`.
2. Drop `allowed-tools`, `agent:`, `context:` and `model:` from frontmatter, or keep them knowing
   Muse records them as advisory metadata (`unsupported-skill-field` warning; no enforcement).
3. Cut the description to one sentence that states WHEN to use the skill plus a "Do not use when ..."
   clause; under `first_sentence` slimming the first sentence is the entire trigger surface.
4. Anything that relied on `allowed-tools` for safety must be re-expressed as a permission profile
   (`content/profiles/`) or a `PreToolUse` deny hook (`content/hooks/guard.json`).
5. A skill that shells out to a binary must say what to do when it is missing; Muse's import lane
   flags missing binaries (`muse skills import --dry-run`) but does not install them.
