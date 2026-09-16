---
name: omm-parallel
description: "Use when a task splits into 2+ independent parts or the user says do these in parallel; Do not use when the work is one linear task or its parts share files or state. Spawn isolated subagents with file ownership, integrate, test once."
metadata:
  short-description: Fan out independent parts to subagents, integrate once
---

# Parallel work

Fan out only parts that are genuinely independent; integrate in one place; run the
suite once. Record the plan with `write_todos`: one item per child, then `integrate`,
then `full suite`; keep exactly one item `in_progress`.

## 0. Gate: work inline, in order, without this skill, when

- The user said "don't use subagents" or "single agent". Always wins.
- `subagent_spawn` is not in your tool list: delegation is off
  (`run.subagent_delegation_mode` is not `auto`). Say so; do not edit settings.
- Fewer than 2 parts, the whole job fits one normal turn, or a part is under ten
  minutes of work (a child costs a startup plus a harvest).
- Part B needs A's output, or an interface that is not fixed yet: sequential.
  Delegate only parts that depend on nothing still in flight.
- Two parts write the same file, migration, lockfile or manifest, or share a live
  resource (dev database, port, fixture dir): one owner writes, the other reads it
  and reports the edits it needs.
- All but one part are lookups: answer those with `search` and `read_file` inline.
  Inspection-only parts (find callers, audit a dir, compare options) may fan out in the
  shared checkout, batched into one child per theme, never one per file or keyword.

## 1. Split and fix the contracts

- Name each part. Give it a disjoint OWN set (paths it may write) and a VERIFY
  command that runs only its tests. Fan-out = number of independent parts, at most
  capacity minus one (default 7; `agents.execution_capacity` counts you). Extra
  spawns queue and start on their own; never re-issue a queued spawn.
- Fix every shared thing in the main checkout BEFORE spawning: new types,
  signatures, module and file names, config keys, dependency additions, migration
  numbers, route registration. Children implement against these and never edit them.
- A child worktree is a checkout of HEAD; your uncommitted edits are invisible to it.
  If the user asked for commits in this task, commit the scaffold first. Otherwise
  paste the contract into every brief and let each child recreate it (excluded again
  in step 4). Never commit on your own.
- A fresh worktree has no ignored state (`node_modules`, `.venv`, `target`). Put the
  install command in the brief (`pnpm install --offline`, `uv sync`) or tell the child
  to skip tests and rely on your single full run. Never point a child at the
  parent's dependency dir.

## 2. Spawn

One `subagent_spawn` per part, all in one turn:

- `command_id`: fresh unique string (`par-<slug>-1`). Every `subagent_*` call that
  takes one gets its own; an exact retry with a used id replays the earlier outcome.
- `role`: short label (`implementer`, `researcher`). `task_name`: at most 80 chars.
- `worktree_isolation: true` for every child that writes. Two writers never share
  the checkout, even on different files (index, lockfiles, build caches). Readers:
  omit it.
- `subagent_type`: omit (general-purpose) unless the workspace ships a fitting
  definition under `.agents/agents/`. A definition only narrows the tool grant.
- `objective`: the whole brief. The child sees nothing of this conversation.

```
GOAL: <one sentence: what is true when you are done>
CONTEXT: <repo, language, key paths; the step-1 contract verbatim, not described>
CONVENTIONS: copy structure, naming and test style from <one existing file>
OWN (write only these): <paths or globs>
DO NOT TOUCH: <contract files, other parts' files, manifests, lockfiles>
STEPS: <numbered, concrete>
INSTALL (once, before tests): <command>   # or: skip tests, the parent runs the suite
VERIFY: <exact command for this part's tests only>
RULES: no git commit/branch/push/stash/reset; no full suite; no new dependencies;
  a change needed outside OWN, or a contract that does not fit: stop and report it
REPORT (last message, under 40 lines): worktree root (`pwd`); files changed, one per
  line; VERIFY command and its summary line verbatim; undone or uncertain, or "none"
```

Worked example: `references/brief-example.md`.

## 3. Wait and collect

- `subagent_wait` per child: fresh `command_id`, the `subagent_id` from the spawn
  result, `wait_for: result_ready`, `timeout_ms` up to 300000.
- `timeout` or `would_park` means the child is still running. Do not poll
  `subagent_status`, do not redo its work. Integrate finished parts, do unrelated
  work, or end the turn: results arrive when you are idle.
- `subagent_read_result` with the `subagent_id`: `summary`, `text`, artifact refs,
  `errorKind`. Judge it: did the child stay inside OWN? Is the quoted test output
  a real run?
- Contract changed under a running child: `subagent_send_message` (fresh
  `command_id`, `subagent_id`, `message`). Children cannot talk to each other.
- A child failed or reported a change needed outside OWN: fix the contract yourself,
  respawn that ONE part with a new `command_id`. `subagent_cancel` (fresh
  `command_id`, `subagent_id`, `reason`) a child made obsolete or duplicating another.

## 4. Integrate

Isolated edits live in the child's worktree (`<repo>/.muse/worktrees/<date>-<hex>`,
the root it reported; fallback `git worktree list`). Bring each into the main
checkout in dependency order, one `bash` call per part:

```sh
WT=<reported root>; P=$(mktemp)
git -C "$WT" status --short                                  # only OWN paths?
git -C "$WT" add -A && git -C "$WT" diff --cached --binary -- . ':!.muse' > "$P"
git apply --stat "$P"                                        # eyeball the paths
git apply --3way --exclude='<contract path>' "$P"   # one --exclude per recreated file
```

- A conflict is an ownership bug: resolve by hand here, the step-1 contract wins;
  never respawn to fix it. Then `search` for the contract names to confirm every part
  used the same signatures.
- Non-isolated children changed nothing on disk; skip them.
- Never `reset`, `clean` or `worktree remove` a child worktree; the runtime owns it,
  deletes a clean one and keeps one with changes. List kept paths in the report.

## 5. Verify once

- Run the FULL suite (plus the repo's lint and typecheck) exactly once, after the
  last patch, with `bash` (`yield_time_ms` up to 300000). Quote the summary line.
- Red: fix it in the main checkout yourself. A "fix" child restarts the loop.
- Report: per child its files and VERIFY line; what you changed at integration; the
  suite summary; kept worktree paths; anything a child flagged.

## Spawn refused

- `workspace_not_git`, `workspace_git_probe_failed`, `no_workspace_root`,
  `no_session_event_sink` (session started with `--no-session-log`),
  `isolation_not_enabled_in_profile`: isolation never degrades silently. Writers: do
  them yourself, sequentially. Readers: respawn without `worktree_isolation`.
- `root_capacity_exhausted`: wait for a child to finish, then spawn with a new
  `command_id`; an exact retry replays the rejection.
- A child's permission prompt reaches the user labelled as delegated; keep briefs
  inside operations this session already allows.
- Full table, wait budget, agent definitions: `references/playbook.md`.

## Stop signals

One child per file, keyword or directory. "Improve X" briefs with no VERIFY.
Working a child's task while waiting. Two writers, one file, "they will not
collide". Suite after each child. Children running the full suite, committing, or
adding dependencies. Respawning to fix a conflict or a red test. Looping on
`subagent_status`.
