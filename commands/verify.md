---
description: Evidence-gated verification of the current claim
argument-hint: [claim]
---

# /verify

Verify: **$ARGUMENTS**

## Steps
1. Read skill `verifier` (and `qa-tester` if manual checks help).
2. Build a checklist from the claim, `.omm/plan.json`, or `$ARGUMENTS`.
3. Run checks; capture outputs.
4. Write `.omm/verify.json` and a short `.omm/verify.md`.
5. Declare pass only if evidence supports it.

Muse validate is recommended when plugin files changed.
