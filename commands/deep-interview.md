---
description: Structured interview to sharpen requirements
argument-hint: [subject]
---

# /deep-interview

Deep interview about: **$ARGUMENTS**

## Steps
1. Read `analyst` and `writer`.
2. Ask batched, high-leverage questions (goals, constraints, non-goals, success metrics).
3. Reflect back a requirements brief.
4. Save `.omm/interview/<stamp>.md` and a condensed `.omm/requirements.md`.
5. Offer next command: `/ultragoal`, `/ralplan`, or `/team`.

Do not start coding until the brief is accepted.

## Notes
CLI `omm interview` and `omm deep-interview` are aliases and live for state files (`.omm/interview/<utc-stamp>.md` + `.omm/requirements.md`). In-session Muse still follows this markdown for the Socratic questions and brief.
