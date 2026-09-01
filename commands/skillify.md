---
description: Draft a new Muse skill from a repeated workflow
argument-hint: [workflow-name]
---

# /skillify

Skillify workflow: **$ARGUMENTS**

## Steps
1. Read `writer` and `document-specialist`.
2. Interview for triggers, steps, tools, and failure modes.
3. Draft `skills/<id>/SKILL.md` with YAML `name` + `description` (id must match portable grammar).
4. Show the draft; on approval, add a capabilities.skills entry guidance for `plugin.json`.
5. Save working notes under `.omm/skillify/<id>.md`.

Remind the user to run Muse plugins validate after manifest edits.
