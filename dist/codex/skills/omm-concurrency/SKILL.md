---
name: "omm-concurrency"
description: "Use for a race condition, deadlock, flaky-under-load or thread/async bug, or a concurrency review: list shared state, probe each access, prove it by injected delay, fix at source; Do not use for single-threaded logic bugs (omm-debug)."
---

# Concurrency

Contract: no edit before the breaking interleaving is written in one sentence ("A reads
X, B writes X between A's check and act -> ..."). Done only when the final message
pastes the test failing WITH an injected delay, the same test green with the delay still
in place, and a stress-loop count from `bash`. "Thread-safe now" is not done.

## 0. Gate

- Fails every run, one actor, same input: omm-debug. Fails only in the test harness
  (order, fixture, port): a flaky-test investigation, which hands back here when the
  race is in the code under test. Here: two or more actors (threads, tasks, processes, requests,
  retries) reach one thing.
- Review with no reported bug: steps 1-2, findings in omm-review's format, no edit.
- `write_todos`: inventory, probes, proof, fix, stress, report.

## 1. Inventory shared state

Everything two actors can reach: globals, singletons, module-level caches, class
attributes, an object handed to more than one task, pools, files and temp paths, DB
rows, cache keys, env vars, cwd. `search` (literal, `glob`-scoped) for the language's
telltales (`Thread(`, `spawn`, `go func`, `gather`, `Promise.all`, `Mutex`, `Lock(`,
`atomic`, `chan`); per-language list and race-detector commands in
`references/tools.md` beside this file. One line per item:

```text
<item> - writers: <who, on which thread/task> - readers: <who> - guard: <lock|atomic|none>
```

"Immutable after init" is shared until you have shown every reader starts after init
ends. `read_file` every access site; the search hit is not the access.

## 2. Probe every access

| Probe | Looks like | Breaks when |
|---|---|---|
| check-then-act | `if k not in m: m[k]=v`, get-or-create, `SELECT` then `INSERT` | B acts between A's check and act |
| read-modify-write | `n += 1`, `list.append`, `m[k] = m[k] + v` unguarded | two writers, one lost |
| lock scope | lock dropped between two steps of one invariant; different locks per path | the second lock guards nothing |
| await in critical section | `await`/yield under a lock or mid-update | another task sees half a state; in async the `await` IS the interleaving |
| lock order | locks A,B taken B,A elsewhere; callback invoked under a lock it may take | deadlock |
| lifetime | closed/dropped while another actor holds it; task outlives its request | use after close, orphaned work |
| publication | field set after the object is visible; listener added after the event | missed write or event |
| retry | re-run on timeout with no idempotency key | duplicate side effect |
| unbounded | queue, channel, task count, pool with no cap | memory or back-pressure failure |
| timeout | wait with no timeout; timeout that does not cancel the work | hang, or ghost work later |

Record each hit as `path:line - <access> - <interleaving that breaks it>`. No sentence,
no finding.

## 3. Prove the interleaving

A race you cannot make fail on demand you cannot prove fixed. In order, with `bash`
(`yield_time_ms` up to 300000):

1. Race detector where one exists (`go test -race`, `-fsanitize=thread`, `loom`,
   `jcstress`; `references/tools.md`). Paste the report. Missing tool: use the loop,
   do not install.
2. Stress: N actors run the operation in a tight loop; assert the invariant at the end
   (count == N, ids unique, each message once). Loop the test:
   `for i in $(seq 30); do timeout 60 <cmd> >/dev/null 2>&1 || echo FAIL $i; done | wc -l`.
   Run it on the unfixed code first: a harness that never fails there proves nothing.
3. Injected delay: a `sleep`/yield of 10-50 ms inside the window step 2 named, behind
   an env var or the suite's test hook, never in the committed hot path. The flake goes
   to 30/30. The delay is the proof; the same test green with it still present is the
   proof of the fix.
4. Still 0/30: report the interleavings tried and stop.

## 4. Fix at the source

Take the first that applies; say why the earlier ones do not.

1. Remove the sharing: local variable, per-request instance, value passed in,
   immutable data (freeze, copy-on-write), a fresh object per task.
2. One owner: a single writer that others message (channel, queue, actor, worker
   thread). No lock, because no second writer.
3. An atomic the platform has: CAS, `sync.Once`, `computeIfAbsent`, `setdefault`,
   atomic counters, DB `UNIQUE` + `ON CONFLICT`, `SELECT ... FOR UPDATE`.
4. A lock, last: one per invariant, held across the whole invariant, never across I/O
   or `await`, always in one order; write the invariant and the order at the declaration.

Always: anything retried gets an idempotency key or a dedupe check in the same
transaction; every queue and pool gets a fixed capacity and a full policy; every wait
gets a timeout and every timeout cancels the work it abandons.

One `edit_file` at the site the interleaving names. Run: delay test (green), stress
loop (count), detector, surrounding suite. Remove the delay unless it sits behind the
suite's own hook.

## 5. Report

```text
Shared: <item> - writers <..> - readers <..> - guard <..>
Race: path:line - <A does X, B does Y between ...> -> <wrong state>
Proof: <detector output | delay test 30/30 fail>
Fix: <option 1-4> - <why not the earlier ones>
After: delay test 30/30 green; stress <n> runs, 0 fail; detector clean; suite <summary>
Not covered: <actors not exercised: other process, other host, restart mid-op>
```

Every number comes from a run in this session.

## Judgment calls

- One-threaded async (event loop): no data races, but every `await` is a yield point.
  Check-then-act across an `await` is a race; remove the `await` from between them or
  use an async lock, not a thread mutex.
- Lock or channel: short section on a counter or cache, lock or atomic; ownership
  handoff or pipeline, channel. Never a channel to guard one integer.
- More than one process or host: in-process locks guard nothing. The database or a
  lock service is the only arbiter; a unique constraint beats check-then-insert.
- User says "just add a lock": name the invariant first; a lock around the wrong
  scope moves the bug.
- Deadlock: dump every thread's stack before theorising (`kill -QUIT` for Go,
  `jstack`, `py-spy dump`, `gdb -p`; `references/tools.md`). The two blocked frames
  name the two locks; compare acquisition order across paths. The fix is one lock
  order, not a longer timeout.
- `sleep(0.1)` "to let the other thread finish" in a test is a race in the test. Await
  the condition (join, event, channel receive) with a timeout. Assert the invariant
  (count, uniqueness, order), never "lock was acquired".

## Refuse

- Any edit before the interleaving sentence; a finding with no written interleaving.
- A lock, `synchronized`, `retry`, or `volatile` without the invariant it protects.
- `sleep` as a fix or as synchronization; a wider timeout to hide a hang.
- Catching the exception the race produces (duplicate key) and continuing.
- Rerunning until it passes; two fixes measured in one run; "thread-safe now" without
  the pasted delay-injected run and stress count.

## Micro-example (Python)

Bug: duplicate welcome emails under load. `search` `send_welcome`; `read_file`
`app/signup.py`:

```python
user = User.get(email)
if user is None:
    user = User.create(email)
    send_welcome(user)
```

1. Inventory: `users` table - writers: every signup request, 4 processes x 8 threads -
   readers: same - guard: none. Probe: check-then-act across a DB round trip; two
   requests for one email both see `None`, both create, both send.
2. Proof: 20 threads call `signup("a@x.io")`, assert `sent == 1`. Loop: 3/30 fail.
   `time.sleep(0.05)` after `User.get` when `SIGNUP_TEST_DELAY` is set: 30/30 fail.
3. Fix: 1, no (the table is the point); 2, no single owner across 4 processes; 3, yes:
   `email` already has `UNIQUE`.
   ```python
   created = User.insert_if_absent(email)   # INSERT ... ON CONFLICT DO NOTHING RETURNING id
   if created:
       send_welcome(created)
   ```
4. After: with delay 30/30 green; without, 50 runs 0 failures; `pytest tests -q`:
   `88 passed`. Not covered: the mail client retries `send_welcome` with no
   idempotency key; follow-up filed.
