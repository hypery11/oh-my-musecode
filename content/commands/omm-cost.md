---
description: Run omm cost, name the biggest context sources and the exact settings that cut them
---
Measure where the session context budget goes and recommend the cheapest cuts.

1. Run with the bash tool exactly `omm cost --json`, whatever was typed after the command (it takes no arguments; a repeated `--json` is rejected by the parser). It runs one `muse exec --provider echo` in a temporary data dir and writes nothing to the user's config.
2. Read the table: skills-catalog bytes by source (bundled, filesystem, plugin; the render order is bundled -> filesystem -> plugin, so plugin entries are starved first when the 32,000-byte cap is hit), the refundable built-in tax, memory (cap 16,305 B / 48 files, silent), rules (256,000 B per file, 65,536 B aggregate), the workflow cookbook block, and estimated tokens (bytes/4, a heuristic; label it as one).
3. Report the three largest sources with bytes and percent of the 32,000-byte catalog cap. State whether stage-2 degradation is active (descriptions silently dropped tail-first while `muse skills list` still reports them all).
4. Recommend, largest measured saving first, the exact setting for each source:
   - `run.context_slimming.excluded_tool_names: ["workflow"]` removes the `workflow` tool and both workflow context blocks (-20,952 B run context, -15,766 B of tool JSON on the wire). Use `run.workflow_trigger_mode: "off"` instead when the model should be told workflows are off (-20,413 B).
   - `run.context_slimming.skill_catalog_descriptions: "first_sentence"` (about -52% on a 40-skill install). Always pair it with `full_skill_description_ids: ["bundled:git"]`: a user value replaces the default list and `bundled:git` would lose its 740-byte rules.
   - `run.context_slimming.session_identity_enabled: false` (-776 to -786 B).
   - `muse skills disable bundled:<id> --scope built-in` for unused built-ins (`bundled:browser-app-delivery` alone is 1,912 B; all 15 are 10,060 B).
   - Memory or rules over their caps: name the files to trim.
5. Give the one-command way to apply: `omm profile use fast` (first_sentence + workflow excluded + session identity off) or `omm profile use default` (first_sentence only). Do not edit `settings.json` yourself: Muse rewrites it from a typed struct and drops unknown keys; omm patches it validated, with the prior value recorded for undo.
