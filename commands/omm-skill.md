---
description: Explain or open an Oh My Muse Code skill
argument-hint: [skill-id]
---

# /omm-skill

Show skill: **$ARGUMENTS**

## Steps
1. Resolve `$ARGUMENTS` to a skill id (architect … git-master).
2. Read `skills/<id>/SKILL.md` and summarize when to use it.
3. List related commands and `.omm/` files it typically touches.
4. If id unknown, list all skill ids and closest matches.

This is documentation/navigation—not skill authoring (`/skillify`).
