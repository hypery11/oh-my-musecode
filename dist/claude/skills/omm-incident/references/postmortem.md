# omm-incident: postmortem template and worked example

Put the postmortem into the incident file, under the timeline, before the
session ends. Every sentence in Root cause and Action items must trace to a
`HH:MMZ` line in the timeline; a claim with no line behind it is a guess and
gets a `?` prefix until someone confirms it.

## Template

```
# <yyyy-mm-dd> <one-line title: what users saw, not what broke>
Severity: <S1 all users / data | S2 many users | S3 subset or degraded | S4 cosmetic>
Detected: HH:MMZ (<alert | user report>) | Mitigated: HH:MMZ | Resolved: HH:MMZ
Duration to mitigate: <m> min | Author: <role> | Status: draft

## Impact
- Who: <all users | region | tenant | plan | client version>
- What: <the user-visible failure, in the user's words>
- How many: <requests, orders, users; a count or a percentage, with its source>
- Data: <none lost | N records corrupted, repaired at HH:MMZ | unknown, check pending>

## Timeline (UTC)
<the HH:MMZ lines, verbatim from the incident file; do not tidy them>

## Root cause
- Cause: <the condition that let the failure reach users: missing validation,
  test, alert, limit, fallback>
- Trigger: <the event that set it off: deploy sha, config change, traffic
  pattern, expiry, upstream change>
- Detection: <alert fired at HH:MMZ | users reported at HH:MMZ, alert did not fire
  because <threshold, missing metric, muted>>
- Ruled out: <each RULED-OUT line and the fact that killed it>

## What went well / what went badly
- Well: <a decision or tool that shortened the incident>
- Badly: <a decision, gap or tool that lengthened it>

## Action items
| # | action | type | owner | due |
|---|---|---|---|---|
| 1 | <change to a system, test, alert, limit or runbook> | prevent | TBD | TBD |
| 2 | <...> | detect | TBD | TBD |
| 3 | <...> | mitigate | TBD | TBD |
```

Types: `prevent` stops the cause recurring; `detect` shortens time-to-detect;
`mitigate` shortens time-to-mitigate (rollout percentage, kill switch, runbook,
faster rollback). A postmortem with only `prevent` items has not looked at the
detection and mitigation gaps; ask why users saw it before the alert did, and
why mitigation took as long as it did.

## Blameless wording

| write this | not this |
|---|---|
| the deploy shipped without a contract test for the cart schema | the developer forgot to test |
| the alert threshold (50% 5xx) was above the failure rate for 12 minutes | on-call did not notice |
| the runbook had no rollback command for this service | the responder did not know how to roll back |
| the migration was irreversible, so rollback was not an option | we should not have run the migration |

"Human error" names a person and stops the analysis. Name the guard that was
missing and the item that adds it.

## Worked example (the micro-example from SKILL.md, filled in)

```
# 2026-09-05 Checkout returned 500 for all users for 20 minutes
Severity: S1 | Detected: 14:02Z (user report) | Mitigated: 14:07Z | Resolved: 14:25Z
Duration to mitigate: 17 min from T0 (13:50Z), 5 min from report | Author: on-call | Status: draft

## Impact
- Who: all web and mobile users placing an order
- What: checkout page returned HTTP 500 after "Pay"
- How many: 41% of checkout requests 13:50Z-14:07Z (~1,900 requests, dashboard "checkout 5xx")
- Data: none lost; no order was created for a failed request

## Timeline (UTC)
14:02Z REPORTED checkout 500s for everyone since about 13:50 (support channel)
14:03Z OBS git log: a1b2c3d 13:47Z "payments: switch to v2 client"; deploy log: rolled out 13:48Z
14:04Z DECISION rollback checkout to previous artifact because deploy 2 min before T0; undo: redeploy a1b2c3d
14:05Z ACTION deploy rollback checkout --to 9f8e7d6 (run by on-call)
14:07Z OBS checkout 5xx 41% -> 0.3% (baseline 0.2%)
14:07Z MITIGATED
14:08Z OBS first error in window: payments/client_v2.py:88 KeyError 'currency'; count equals 5xx count
14:11Z OBS v2 client: currency required (client_v2.py:80); cart serializer: currency optional since 2023 (cart/serializers.py:44)
14:12Z OBS all 5xx requests carry carts created before 2023-04 (log field cart_created)
14:25Z RESOLVED 5xx at baseline for 15 min; no retry backlog (checkout has no queue)

## Root cause
- Cause: the cart schema allows a missing `currency`; the v2 payments client assumes
  it is present; no contract test covers legacy carts against the client
- Trigger: deploy a1b2c3d at 13:48Z
- Detection: users reported at 14:02Z; the 5xx alert (threshold 50%) never fired at 41%
- Ruled out: payment provider outage (provider status green 13:40Z-14:10Z; errors are
  local KeyError, not upstream 5xx)

## What went well / what went badly
- Well: rollback took 2 minutes because the previous artifact was still cached
- Badly: 12 minutes between T0 and the first human noticing; alert threshold too high

## Action items
| # | action | type | owner | due |
|---|---|---|---|---|
| 1 | default `currency` in the cart serializer; contract test: legacy cart through v2 client | prevent | TBD | TBD |
| 2 | checkout 5xx alert threshold 50% -> 5% over 2 min | detect | TBD | TBD |
| 3 | payments deploys go out at 5% for 10 min before 100% | mitigate | TBD | TBD |
| 4 | add `deploy rollback checkout` to the checkout runbook | mitigate | TBD | TBD |
```

Not in the report: who wrote a1b2c3d, who was on call, how long the fix took to
write. The fix itself is a separate change (omm-tdd: the contract test red on the
old serializer, green after the default), reviewed and deployed through the normal
path.
