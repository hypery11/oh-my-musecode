---
description: Maintain a lightweight project wiki under .omm/wiki
argument-hint: [page|search]
---

# /wiki

Wiki action: **$ARGUMENTS**

## Steps
1. Ensure `.omm/wiki/` exists.
2. If `$ARGUMENTS` looks like a title, create/update `.omm/wiki/<slug>.md`.
3. If `search …`, list matching pages.
4. Keep an index at `.omm/wiki/README.md`.
5. Read `document-specialist` for structure; keep entries short.

CLI `omm wiki [list|show <page>|write <page>]` is live against `.omm/wiki/*.md`. This slash-command still edits wiki pages in-session.
