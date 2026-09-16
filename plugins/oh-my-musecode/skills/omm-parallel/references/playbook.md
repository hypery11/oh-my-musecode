# Parallel playbook: refusals, handing over uncommitted files, wait budget, definitions

## Spawn refusals and what to do

| Reason | Meaning | Do |
|---|---|---|
| tools absent | `run.subagent_delegation_mode` is `off` | tell the user; work sequentially |
| `root_capacity_exhausted` | all slots busy (`agents.execution_capacity`, default 8 incl. root) | wait for a child; spawn again with a new `command_id` |
| `workspace_not_git` | not a Git repository | readers: respawn without isolation; writers: sequential |
| `workspace_git_probe_failed`, `no_workspace_root` | isolation impossible here | same as above |
| `no_session_event_sink` | session started with `--no-session-log` | same as above, or restart with logging |
| `isolation_not_enabled_in_profile`, `native_child_execution_not_enabled` | the run preset forbids it | tell the user; work sequentially |
| `worktree_cleanup_pending` | an earlier worktree is still being cleaned up (inferred from the name) | wait once; retry with a new `command_id` |
| wait returns `timeout` / `would_park` | child still running | do not poll; the result is delivered when idle |
| duplicate-work advisory | two children share a task | `subagent_cancel` the redundant one |

An affirmative `worktree_isolation` request that cannot be honoured is rejected; it
never falls back to the shared checkout. A rejected spawn created nothing: fix the
request and spawn again with a new `command_id`; an exact retry replays the rejection.

## Files that exist only in the parent's working tree

An isolated child checks out HEAD. Anything uncommitted in the parent is invisible.

1. The user asked for commits in this task: commit the scaffold, then spawn. Cleanest.
2. Otherwise paste the file(s) verbatim into every brief under `RECREATE FIRST`, tell
   the child to `write_file` them byte-for-byte, and exclude them at integration:
   `git apply --3way --exclude=src/lib/csv.ts "$P"` (one `--exclude` per file), or
   leave them out of the patch: `git -C "$WT" diff --cached --binary -- . ':!src/lib/csv.ts'`.
3. A single line in a shared file (an import, a route registration): make the edit in
   the main checkout, tell the child to make the same edit itself, exclude the file.

Never commit to make the scaffold visible; the bundled git rule needs an explicit
user request for every commit.

## Harvest checks

```bash
WT=/repo/.muse/worktrees/20260902-1a2b; P=$(mktemp)
git -C "$WT" status --short                     # every path inside OWN? untracked junk?
git -C "$WT" add -A                             # stage new files so the diff carries them
git -C "$WT" diff --cached --binary -- . ':!.muse' > "$P"
git apply --stat "$P"                           # paths and sizes, before touching anything
git apply --check --3way "$P" || echo CONFLICTS # dry run
git apply --3way "$P"                           # markers on conflict; resolve by hand
```

`--binary` keeps images and fixtures intact; `--3way` needs the `index` lines that
`diff --cached` produces. The result is unstaged in the main checkout, ready for the
user's review. Apply in dependency order. Do not `reset`, `clean` or `worktree remove`
the child worktree: the runtime owns it, removes a clean one, and retains one with
tracked changes, untracked files, or a changed HEAD. Report retained paths.

## Wait budget

`subagent_wait` defaults to 30000 ms; `timeout_ms` accepts 10000 through 300000. Give
one wait per child at 300000, then move on. A timed-out wait never cancels the child.
`wait_for: result_ready` returns the result envelope; `task_terminal` only a task ref.

## Result envelope

`subagent_read_result` returns `summary` (at most 512 chars), `text` (at most 32 KiB),
`artifactRefs`, `evidenceRefs`, `errorKind` when the child failed, and `structuredData`
when the spawn passed an `output_schema`. Keep the brief's REPORT short enough to fit.

## Agent definitions

`subagent_type` names a definition from `<workspace>/.agents/agents/**/*.md` or
`$XDG_CONFIG_HOME/muse/agents/**/*.md` (frontmatter `name:` is the id, grammar
`[a-z]+(-[a-z]+)*`, at most 128 bytes). Its prompt is appended to the child; its
`tools` may only narrow the parent's grant, never widen it. `general-purpose` and
`workflow-subagent` are built in; omit `subagent_type` for a general-purpose child.

## Children and permissions

A child inherits the parent's effective tools as an upper bound. An approval it needs
reaches the user labelled as coming from a delegated subagent. Keep briefs inside
operations this session already allows, or expect the child to stall on a prompt.
