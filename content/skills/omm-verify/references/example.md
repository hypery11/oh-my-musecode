# omm-verify: worked example

Request: "Fix the off-by-one in `paginate()` that drops the last page."

Claims to be made: (1) bug fixed, (2) tests pass, (3) lint clean, (4) only
the asked-for files changed.

## Runs (all after the last edit)

```
$ pytest tests/test_paginate.py -q; echo exit=$?
5 passed in 0.31s
exit=0
$ pytest -q; echo exit=$?
131 passed, 2 skipped in 9.8s
exit=0
$ ruff check . && ruff format --check .; echo exit=$?
All checks passed!
41 files already formatted
exit=0
$ git status --porcelain --untracked-files=all
 M src/pager.py
 M tests/test_paginate.py
?? scratch_repro.py
```

`scratch_repro.py` is mine: `rm scratch_repro.py`; status now shows two lines.

Old-code half of the bug-fix claim:

```
$ W=$(mktemp -d); git worktree add -q "$W" HEAD
$ cp tests/test_paginate.py "$W/tests/"
$ (cd "$W" && pytest tests/test_paginate.py -q); echo exit=$?
1 failed, 4 passed in 0.29s
FAILED tests/test_paginate.py::test_last_page_kept
exit=1
$ git worktree remove --force "$W"
```

The 2 skipped tests in the full run were checked: both are marked
`@pytest.mark.skipif(no_postgres)`, unrelated to `paginate()`.

## Report block

```
Verified
- bug fixed: `pytest tests/test_paginate.py -q` -> 5 passed, exit 0;
  same test on old code (worktree at HEAD) -> 1 failed (test_last_page_kept), exit 1
- tests pass: `pytest -q` -> 131 passed, 2 skipped (postgres-gated, unrelated), exit 0
- lint clean: `ruff check . && ruff format --check .` -> All checks passed!, exit 0
Diff: 2 files, in scope: src/pager.py tests/test_paginate.py
Not verified: the database-backed pagination path needs credentials; did
  in-memory list path only
```

Then, and only then: "Fixed."
