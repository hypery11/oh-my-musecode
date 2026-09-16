---
description: One-screen omm status - ledger, active profile, plugin capability trust, workspace trust
---
Summarize the omm install state. Read only; change nothing.

1. Run with the bash tool exactly `omm list --json` (resolved assets with provenance and `_shadowed`) and `omm doctor --json` (D1 plugin capability states, D12 workspace trust, D13 ledger integrity). Neither takes arguments; ignore anything typed after the command.
2. Report four short sections:
   - Ledger: omm version, host version and sha recorded at install, entry counts by kind (skill, command, hook, theme, rules, settings-key, trust, plugin), registrations (marketplace, plugin), and any staged conflicts under `updates/<version>/`.
   - Profile: which of strict / default / fast / ci is applied (the ledger's settings-key entries) and the effective `run.context_slimming` values.
   - Plugin trust: every `plugin:omm:*` capability with its state. Anything not literally `trusted_enabled` (`review_needed`, `trusted_disabled`, `modified`, or a missing line) is inert at runtime; print `muse plugins approve <stable-id>` for each.
   - Workspace trust: whether the current directory is in `trust.json`. If not, project skills, hooks, rules and workflows are silently inert; print `omm trust .`.
3. List shadowed or disabled assets by id (overlay REPLACE or APPEND under `custom/`, `config.json` disabled ids).
4. End with the single next command if anything is off, otherwise the line `omm: all green`.
