---
description: Route a focused question to the best skill
argument-hint: [question]
---

# /ask

Question: **$ARGUMENTS**

## Steps
1. Classify the question (design / debug / git / security / docs / other).
2. Read the best-fit skill (`architect`, `debugger`, `git-master`, `security-reviewer`, `document-specialist`, …).
3. Answer with evidence and paths. If research is large, `subagent_spawn` an `explore` worker.
4. Optionally append Q/A to `.omm/ask-log.md`.

Note: CLI `omm ask` is live as an in-process keyword router over the 19 bundled skills (writes `.omm/ask/last.json`, no remote model). This slash-command still answers in-session.
