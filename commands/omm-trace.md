---
description: Trace a Muse/OMM flow end-to-end
argument-hint: [target]
---

# /omm-trace

Trace: **$ARGUMENTS**

## Steps
1. Read skill `tracer` (optional `explore`).
2. Follow the target (hook event, command, function, file writer).
3. Write `.omm/trace/<slug>.md` with the flow.
4. Call out `.omm/` side effects and hook scripts involved.

Use `subagent_spawn` if multiple subsystems must be scanned in parallel.

## Notes
CLI `omm trace [target...]` is live: writes `.omm/trace/<slug>.md` with a **static** outline of this plugin's hooks/commands (not a live tracer of user code). In-session Muse still follows this markdown when walking a specific flow.
