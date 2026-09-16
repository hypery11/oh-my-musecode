---
name: omm-reflect
description: Persist lessons (conventions, commands, traps) to memory or propose a skill for a repeated procedure; use on remember this, reflect, what did we learn, or when a multi-step task ends; Do not use for one-off facts or ones in AGENTS.md.
argument-hint: "[fact or focus]"
metadata:
  short-description: Persist this session's lessons to memory
---

# Reflect

Goal: the next session starts knowing what this one learned. Output: zero or more
memory writes, at most one proposal (a skill or an AGENTS.md line) that waits for
the user, and a report of at most six lines. Memory holds facts, never a summary.

Entry points differ:

- `remember this: X` -> X is the whole candidate list. Do steps 2-4 for it and stop.
- `what did we learn` -> steps 1-3, show the report as a plan, write only on yes.
- `reflect`, or a multi-step task just ended -> all steps. `personal` and
  `personal_project` writes need no confirmation; `project` writes wait for a yes.

## 1. Collect candidates

Source: the transcript in context. Walk it once and mark every place where a first
attempt failed and a later one worked, where you searched for something a rules file
should have told you, and where the user corrected you.

Only if context was compacted or work ran in subagents: `read_skill bundled:read-session`,
take the `session.jsonl` path from the session-identity block, then read bounded slices:

```bash
LOG=<session.jsonl path from the session-identity block>
tail -n 300 "$LOG" | grep -E 'user_prompt_display|assistant_message_committed' | cut -c1-400
ls "$(dirname "$LOG")/subagent/"    # delegated work: subagent/<id>/session.jsonl
```

`assistant_tool_calls_committed` paired with `tool_result_batch_committed` shows what
ran and what failed. Never load a whole log.

Keep only:

- convention: naming, layout, generated files the repo enforces but no file states;
  keep if a fresh session would violate it on the first try
- command: exact argv, cwd, env that worked; keep if it cost at least one failed attempt
- trap: the error text and the fix; keep if it will recur and the cause is non-obvious
- preference: something the user stated or corrected
- procedure: the same 3+ step dance run two or more times -> skill candidate (step 5)

Drop: task state (branch, open items: that is `write_todos` territory); anything
`AGENTS.md` or another rules file already says; claims no tool result confirmed;
what a tool's own `--help` states; secrets and tokens, even redacted.

## 2. Dedupe

The memory snapshot at session start is stale after any write. Before writing,
`read_memory` with `scope` and `path: "MEMORY.md"` for the target scope (a missing
file is fine). If a line already covers the fact, `edit_memory` it (sharpen, correct)
instead of appending. If `AGENTS.md` states it, skip and say so.

## 3. Choose scope and type

| fact | scope | type |
|---|---|---|
| user preference, habit, tool choice; true across repos | `personal` | `user`, or `feedback` for a correction |
| about this repo but local to this user or machine: paths, env, what was tried | `personal_project` (the tool default) | `project` |
| convention or trap every collaborator hits; safe to commit | `project` (`<ws>/.agents/memory/`) | `project` |
| URL, doc, command recipe | by the rows above | `reference` |

Tie-breakers: a repo fact a teammate would agree with is `project`; one only this
user cares about is `personal_project`. Unsure between `personal` and
`personal_project`: pick `personal_project`; a wrongly global fact pollutes every
other project. Unsure whether to keep at all: drop it.

`project` memory lands in version control and is loaded even in untrusted checkouts:
never a secret, a personal path, or a preference; never in a repo the user said is not
theirs; do not commit it yourself. A rule that must hold every turn ("always run X
before commit") is an `AGENTS.md` line: propose the exact line, do not edit the file.

## 4. Save

- `MEMORY.md` is inlined at every session start; other files are only listed by
  path. The snapshot truncates silently at 16,305 B. Keep `MEMORY.md` an index of
  one-line facts under about 12 KB; put detail in a topic file (`traps.md`,
  `commands.md`) and add one pointer line to `MEMORY.md`.
- Entry form: `- <imperative fact> -- <evidence: command, path, or error text> (YYYY-MM-DD)`.
  A fix without its symptom is useless: write `<symptom> -> <fix>`.
- New fact: `add_memory` with `scope`, `path`, `content`, `type`, `description`
  (one-line recall summary).
- Correction: `edit_memory` with `scope`, `path`, `old_str` (must match exactly once,
  keep it short), `new_str`. Never rewrite a memory file wholesale.

## 5. Skill proposal (only when a procedure repeated)

Propose; never create unasked. Give: id (`verb-noun`), a one-line description ending
in "Do not use when ...", 3 to 7 steps naming Muse tools (`read_file`, `edit_file`,
`bash`, `search`, `subagent_spawn`). On yes, `read_skill bundled:create-skill` and
follow it (`.agents/skills/<id>/`, then `muse skills validate`).

## 6. Report

At most six lines: each write as `<scope>/<path>: <line as stored>`; each skip with
its reason; the proposal if any, marked as waiting on the user. Nothing else.

Worked example: `references/examples.md` next to this file.
