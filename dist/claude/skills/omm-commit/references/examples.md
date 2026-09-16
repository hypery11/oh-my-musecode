# Commit messages that pass, and one worked split

Companion to omm-commit. Load with `read_file` when unsure what a subject or
body should look like, or how a mixed tree becomes several commits.

## Message shapes that pass

```
Reject empty usernames at signup

The form allowed a whitespace-only name, which the mailer later turned
into "Hi ," in every message. Validate at the API boundary rather than
in the mailer so the rule holds for imports too.
```

```
Bump tokio to 1.40 for cooperative task budgets

Needed by the scheduler change that follows; 1.40 is the first release
with Runtime::budget. No other dependency moves.
```

```
Revert "Cache user lookups in middleware"

This reverts commit 8c1f2d3. The cache keyed on user id but ignored the
tenant, so a request could read another tenant's profile. Reintroduce
with a tenant-scoped key once the session carries it.
```

```
Fix typo in install instructions
```
(no body: the subject leaves no question)

## Worked split: one tree, three commits

Tree after fixing `parse_date`: a rename `utils.py` to `dateutil.py` that the
fix needed, the fix plus its test, an unrelated README typo, and a stray
`.env`. Partitions: (1) rename, (2) fix + test, (3) typo. `.env`: never
staged; mention it, and offer a `.gitignore` line only if the user wants one.

1. `git add -p src/dateutil.py` keeping only import-path hunks;
   `git add src/utils.py` (the deletion). Screen: rename plus import updates
   only.
   ```
   Rename utils.py to dateutil.py

   Every remaining function in the module handles dates; the generic name
   hid that. No behaviour change. The parse_date fix follows separately.
   ```
2. `git add src/dateutil.py tests/test_dateutil.py`. Screen: one function
   body, one test.
   ```
   Fix parse_date rejecting ISO timestamps with a trailing Z

   RFC 3339 allows "Z" for UTC but the regex accepted only a numeric
   offset, so every timestamp from the events API raised ValueError.
   Map "Z" to +00:00 before parsing instead of widening the regex, so
   offset arithmetic stays in one place.

   Fixes #142
   ```
3. `git add README.md`, subject `Fix typo in install instructions`, no body.

`git log --oneline -3` now reads as three steps. Report: three commits,
`.env` left untracked and flagged.
