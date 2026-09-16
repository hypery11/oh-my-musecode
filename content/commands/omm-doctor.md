---
description: Run omm doctor and explain every finding with its exact fix command
argument-hint: [--self-test] [--report-drift]
---
Run omm's health checks and explain the result. Do not fix anything.

1. Run with the bash tool: `omm doctor --json $ARGUMENTS`. If `omm` is not on PATH try `~/.local/bin/omm`; if that fails too, stop and say omm is not installed.
2. The JSON lists checks D1..D14. Each carries an id, a severity (critical | warning | info | ok), what was observed, why it is silent at session time, and the exact fix command. A non-zero exit code means at least one critical check.
3. Report most severe first, one line per non-ok check: `<id> <severity> - <observed>`, then the fix command verbatim in a code span. Fold every ok check into one line of ids.
4. For each critical, add one sentence on what is silently broken in a live session (examples: a capability that is not literally `trusted_enabled` never spawns; an untrusted workspace loads no project skills, hooks, rules or workflows; `settings.provider` unset gives `muse serve` a one-tool session; `mcpServers` plus `mcp_servers` both present makes every settings-writing command exit 1).
5. Do not run a fix command. If the user wants a fix applied, show the command and let them approve it. The fixes omm prints are the only supported way to change its state; never edit `settings.json` or `trust.json` by hand.
