---
description: Store a durable memory note under .omm
argument-hint: [note]
---

# /remember

Remember: **$ARGUMENTS**

## Steps
1. Append a dated note to `.omm/memory.md`.
2. If the note is structured (decision/preference/fact), also upsert `.omm/memory.jsonl` with `{ts,type,text}`.
3. Confirm what was stored and how to recall it later (`/ask` or read `.omm/memory.md`).

Never store secrets (tokens, private keys). State stays under `.omm/`.
