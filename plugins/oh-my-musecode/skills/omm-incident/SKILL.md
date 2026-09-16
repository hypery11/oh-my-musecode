---
name: omm-incident
description: "Use on outage, prod is down, incident, or users report a live failure: stabilise first (rollback, flag, scale), timestamped timeline, evidence then one hypothesis, blameless postmortem; Do not use when it reproduces only in dev (omm-debug)."
---

# Incident

Stop the bleeding before understanding it. Every observation, decision and command
goes into a timestamped timeline as it happens. No production change without its
undo named first, and none run without the user's ok. Blame is never a root cause.

## 0. Declare (two minutes, no more)

- Confirm it is production and user-facing: what fails, for whom, since when.
  Reproducible only in dev, or no user impact: not an incident; omm-debug.
- Open the timeline first: `write_file` `<dir>/<yyyy-mm-dd>-<slug>.md`, `<dir>` the
  repo's incident directory (`search` for `incidents/`, `postmortems/`) or a scratch
  path outside the repo, named in the reply. `edit_file` appends one line per event,
  `HH:MMZ <TAG> <text>` (`bash` `date -u +%H:%MZ`, never from memory), tags
  `REPORTED OBS DECISION ACTION RULED-OUT MITIGATED RESOLVED`. T0 is when the
  symptom started (first alert or user report), not when you were told.
- Severity, one line: all users or a subset; data lost, degraded, or cosmetic. Data
  at risk: act now, gather later.
- `write_todos`: stabilise, gather, hypothesis, resolve, postmortem; one `in_progress`.

## 1. Stabilise before diagnosing

Ask "what changed?" and take the cheapest reversible move that matches. Do not wait
for a cause; a mitigation that works is evidence.

| signal | first move | undo |
|---|---|---|
| started minutes after a deploy, config or flag change | roll that change back via the deploy tool | redeploy the artifact |
| one new feature or endpoint failing | flag off | flag on |
| saturation: CPU, connections, queue depth, disk | scale out; shed or rate-limit load | scale in |
| a dependency down or slow | fail over, serve stale cache, circuit-break | restore the route |
| writes producing bad data | stop the writer (pause the job, drain the queue) | resume |
| leak, wedged process, exhausted pool | restart after a one-minute log or heap capture | none |
| nothing fits | freeze deploys and cron on the system; step 2 | unfreeze |

- One change at a time. Before: append `DECISION <move> because <signal>; undo:
  <command>`. After: wait one metric cycle, read the user-visible signal (error
  rate, latency), never "the process is up"; append `OBS`. Worse or unchanged:
  undo it, then the next row.
- Rollback beats fix-forward while users are down. Fix forward only when rollback is
  impossible (irreversible migration, bad state in data not code, committed writes
  would be lost); then flag, scale or fail over instead.
- A command that mutates production (rollback, flag, scale, restart, config): write
  the exact command, its effect and its undo, end the turn, run it with `bash` only
  on the user's ok. Non-mutating pulls (logs, metrics, deploy history) run at once. No
  credentials here: hand the command to the human, record who ran it and when.
  Never run `DELETE`, `DROP`, `rm`, a migration or a backfill against production.
- User wants the cause first: offer the mitigation once, with its cost; declined,
  record `DECISION mitigation deferred by <role>`.

## 2. Gather evidence, bounded

Every fact carries its source and time. Never pull a whole log into context: `bash`
with `| tail -n 200`, `| sort | uniq -c | sort -rn | head`; save excerpts beside the
timeline (logs rotate) and cite `file:line`.

- Changes: `git log --since='24 hours ago' --date=iso --format='%h %ad %s'`, deploy
  and flag history, infra and config, dependency bumps, provider status, certificate
  expiry, quota resets, cron. No deploy behind it is common; do not force one in.
- Logs: the FIRST error in the window, not the loudest or newest. `search` (literal
  mode) the repo for its text; `read_file` the frame that emits it and its caller.
  Count errors before and after each change timestamp; the jump at T0 is the lead.
- Metrics: where the curve bends is T0. A change more than a few minutes before T0
  needs a delay story (cache TTL, rollout percentage, batch job) or it is not the
  trigger. Split by endpoint, region, tenant, version; what still works is a clue.
- Three or more independent sources: `subagent_spawn` one read-only child per
  source, returning `HH:MMZ OBS <fact> (<command>)` lines only; merge them yourself.

## 3. One hypothesis, verified

- State it as a prediction: "If X, then <observable> shows Y." Pick what changed
  nearest T0, or what the first error names literally.
- Name the cheapest observation that could refute it, then get it: `git show <sha>`
  of the implicated change, `read_file` the function the error names, a log line
  only X produces, the failing input replayed in staging. A probe that can only
  agree with you is noise. Refuted: `RULED-OUT <hypothesis> because <fact>`, back
  to step 2.
- Two refutations: widen. The change may be upstream (provider, DNS, certificate),
  or two changes landed in the window: revert one at a time in staging, never in
  production. Still stuck: reproduce outside prod; omm-debug.
- Root cause is proven only when the chain change -> mechanism -> symptom has
  evidence per link, the mitigation removed the symptom for the reason the
  mechanism predicts (not "it went away"), and no timeline entry contradicts it.
  Separate trigger (the deploy) from cause (the missing validation, test, alert or
  limit that let it reach users) and detection gap (why nothing caught it).

## 4. Fix and resolve

- The permanent fix ships through the normal path once stable: regression test
  first (omm-tdd or omm-debug), review, deploy. Never hand-edit production files;
  never skip the test because it feels urgent; no fix during the incident.
- Resolved means: user-visible signal at baseline for one full alert window (at
  least 15 minutes), queues and retries drained, the mitigation made permanent or
  reverted deliberately. Append `RESOLVED`.

## 5. Postmortem skeleton

Put it into the incident file while the timeline is fresh. Full template, blameless
wording and a filled example: `references/postmortem.md` beside this skill (directory
from the `locator:` line of the read_skill result). Gaps stay `TODO`, never filled
from memory.

```
# <yyyy-mm-dd> <one-line title>
Severity | Detected HH:MMZ | Mitigated HH:MMZ | Resolved HH:MMZ | Status: draft
## Impact          who, what failed, how many, how long, data lost or not
## Timeline (UTC)  the HH:MMZ lines, verbatim
## Root cause      cause (what let it happen), trigger, detection gap
## Action items    | # | action | prevent / detect / mitigate | owner | due |
```

- Systems and decisions, not people: "the deploy went out without a contract
  test", never "<name> deployed without testing". "Human error" names a missing
  guard; name the guard.
- Every action item changes a system, test, alert, limit or runbook; "be more
  careful" is not one. Users noticing before the alert is always its own `detect`
  item. Owner and due stay `TBD` unless the user assigns them.
- A trap others will hit (deploy tool quirk, misleading dashboard): `add_memory`.
- Final message: incident file path; mitigation state with its timestamp; root
  cause status (proven | hypothesis | unknown); open action items.

## Refuse

Diagnosing while users are down when a reversible move exists. Two production
changes at once. A restart before the capture. Resolving on the first green minute.
A timeline written from memory.

## Micro-example

`14:02Z REPORTED checkout 500s for everyone since about 13:50`. `git log` shows
`a1b2c3d 13:47Z payments: switch to v2 client`, rolled out 13:48Z. `14:04Z DECISION
rollback checkout because deploy 2 min before T0; undo: redeploy a1b2c3d`; user ok,
run. `14:07Z OBS 5xx 41% -> 0.3% (baseline 0.2%)`, `MITIGATED`. `14:08Z OBS first
error in window: payments/client_v2.py:88 KeyError 'currency'; count equals 5xx
count`. Hypothesis: v2 requires `currency`, old carts lack it. `search` `currency`:
required in the v2 client, optional in the cart serializer since 2023; every failing
order carries a legacy cart. Confirmed: schema mismatch the cause, deploy the
trigger, 50% alert threshold the detection gap. `14:25Z RESOLVED baseline 15 min`.
Actions: contract test (prevent); 5xx alert at 5% (detect); staged rollout (mitigate).
