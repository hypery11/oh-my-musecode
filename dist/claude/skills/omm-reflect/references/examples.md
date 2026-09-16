# Reflect: worked example

Session: fixed a flaky integration test in a Rust workspace; three attempts before
the fix; the user corrected the agent once; context was not compacted, so the
transcript alone was the source.

## 1. Candidates

1. `cargo test -p api` hangs without `-- --test-threads=1`; the tests share a port.
   Trap + command; cost two failed attempts.
2. Fixtures live in `crates/api/tests/fixtures/`, not `tests/`. Convention; the first
   `search` looked in the wrong place.
3. User said: "stop asking, just run the tests". Preference / correction.
4. Ran the same reproduce-then-bisect loop twice (stash, rerun, diff). Procedure.
5. Today's failing test name. Transient task state -> drop.
6. `cargo build` is the build command. Already in `AGENTS.md` -> drop.
7. "The runner is probably slow on Fridays". Never confirmed by a tool result -> drop.

## 2. Dedupe

`read_memory {scope: "personal_project", path: "MEMORY.md"}` returns an older line
`- api tests are flaky, rerun on failure -- seen 2x`. Candidate 1 supersedes it:
edit, do not append. `read_memory {scope: "project", path: "MEMORY.md"}` fails
(no file yet): fine, the first `add_memory` creates it.

## 3. Scope and type

| candidate | scope | type | why |
|---|---|---|---|
| 1 | `project` | `project` | every collaborator hits it; nothing private |
| 2 | `project` | `project` | same |
| 3 | `personal` | `feedback` | about this user, true in every repo |
| 4 | skill proposal | - | a procedure, not a fact |

The repo had no `project`-scope memory yet, so the agent asked before the first
write; the user agreed.

## 4. Writes

```
edit_memory {scope: "personal_project", path: "MEMORY.md",
  old_str: "- api tests are flaky, rerun on failure -- seen 2x",
  new_str: "- api tests share a port: `cargo test -p api` hangs -> run with `-- --test-threads=1` (2026-09-02)"}

add_memory {scope: "project", path: "MEMORY.md", type: "project",
  description: "api tests need a single-threaded run",
  content: "- api tests share a port: `cargo test -p api` hangs -> run with `-- --test-threads=1` -- crates/api/tests/common.rs (2026-09-02)"}

add_memory {scope: "project", path: "MEMORY.md", type: "project",
  description: "fixture location",
  content: "- Test fixtures live in crates/api/tests/fixtures/, not tests/ (2026-09-02)"}

add_memory {scope: "personal", path: "MEMORY.md", type: "feedback",
  description: "run tests without asking",
  content: "- Run the test suite without asking for confirmation -- user correction (2026-09-02)"}
```

## 5. Skill proposal

- id: `bisect-flaky-test`
- description: Reproduce a flaky test, then bisect recent commits with stash, rerun
  and diff until the introducing change is found. Do not use when the test fails
  deterministically.
- steps: (1) `bash` run the test 5x, record pass/fail; (2) `bash git log --oneline -20`;
  (3) per candidate commit: `bash git stash && git checkout <sha>`, rerun, restore;
  (4) `search` the diff of the first bad commit; (5) report sha + diff hunk.

## 6. Report

```
project/MEMORY.md: - api tests share a port: `cargo test -p api` hangs -> run with `-- --test-threads=1` -- crates/api/tests/common.rs (2026-09-02)
project/MEMORY.md: - Test fixtures live in crates/api/tests/fixtures/, not tests/ (2026-09-02)
personal/MEMORY.md: - Run the test suite without asking for confirmation -- user correction (2026-09-02)
personal_project/MEMORY.md: replaced the stale "flaky, rerun" line
skipped: cargo build (in AGENTS.md); today's failing test (task state); Friday runner theory (unconfirmed)
proposed skill: bisect-flaky-test -- waiting on you
```
