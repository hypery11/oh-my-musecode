# content/routing — the routed skill library (opt-in, PLAN.md 3.1)

Skills here are NOT part of the plugin package, the projections, the marketplace catalogs or the
order-200 `skills_catalog`. They are the library `omm enable skill-routing` copies into a trusted git
workspace at `<ws>/.omm/skills/<id>/SKILL.md` — real regular files, never symlinks (`read_skill`
refuses a link; `research/experiments/skill-routing.md` V2) and never under a discovery root
(`.agents/skills/` would be auto-catalogued and then `base-id-collision`-rejected). The router
(`omm hook route`, a `UserPromptSubmit` handler declaring `"outputCapabilities":["skills.v1"]`)
picks at most three per turn by matching the prompt against each skill's `metadata.triggers` and
returns them as `hookSpecificOutput.selectedSkills`; the host renders them at order 201 and
`read_skill <id>` resolves them by id or absolute path. `docs/ROUTING.md` has the whole story.

The generator treats this tree as claimed and non-packaged (`omm_manifest::content::ROUTING_DIR`);
`omm lint`'s symlink / backslash / UTF-8 scan still covers it (R12 applies to what omm copies).

## Format

One directory per skill, `SKILL.md` only (a `references/` sibling is copied too when present):

```
---
name: <id>                       # == the directory name; omm- prefix; ^[a-z0-9][a-z0-9._-]{0,79}$
description: <one line>          # what the model sees at order 201; ≤ 1,024 bytes (the host cap),
                                 # aim ≤ 300; end with a "Do not use …" clause
metadata:
  triggers: <phrase>, <phrase>   # comma-separated, case-insensitive; a multi-word phrase matches as a
                                 # substring on word boundaries, a single word as a whole word; every
                                 # matched trigger scores 1 + its word count, the top three (score > 0)
                                 # are routed
---
<body ≤ 8,192 bytes, Muse tool vocabulary only>
```

`metadata.triggers` is used because the host's `skills validate` reports a top-level `triggers` key
under `unknown_fields` (still `valid`), while `metadata` members are a known field (measured
2026-09-03 on 1.0.1-R2006.1). Ids must not collide with a catalog skill id (`content/catalog.json`)
or a bundled skill id: the bundle's skills are at order 200 already and the router's job is to add
what the catalog does not carry.

## Budget

`order200 + order201 ≤ 31,984 B` (host-reality.md "Budgets"; above it the host first drops every
routed description silently, then rejects the whole selection). The rendered order-201 block costs
`280 + Σ (301 + len(id) + len(<ws>/.omm/skills/<id>/SKILL.md) + len(<ws>/.muse/hooks.json) +
len(xml-escaped description))` bytes (measured 2026-09-03; `c_tune::routing::BLOCK_FRAME_BYTES` /
`ENTRY_BASE_BYTES` in `crates/omm`). The router budgets every turn against the last measured order-200 size and
trims or drops entries to stay under the cap with a margin; keep descriptions short so three fit
in any workspace.
