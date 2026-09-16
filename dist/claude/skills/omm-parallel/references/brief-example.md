# Worked example: one task, two children, parent keeps the third part

Request: "Add CSV export to the reports page: backend endpoint, frontend button, docs."
Repo: Python/FastAPI backend (`uv`, `pytest`) plus a TypeScript frontend (`pnpm`,
`vitest`).

## Step 0 and 1 (main checkout, before spawning)

Three parts. Backend and frontend are independent once the endpoint contract is
fixed. Docs is under ten minutes: the parent writes it while the children run, so it
is not a child.

| part | OWN | VERIFY |
|---|---|---|
| backend | `api/reports/export.py`, `tests/api/test_export.py` | `uv run pytest tests/api/test_export.py -q` |
| frontend | `web/src/reports/ExportButton.tsx`, `web/src/reports/ExportButton.test.tsx` | `cd web && pnpm vitest run src/reports/ExportButton` |
| docs (parent) | `docs/reports.md` | none |

Contract fixed by the parent and pasted verbatim into both briefs:

```
GET /api/reports/{id}/export?format=csv
  200 text/csv, header row = column names from ReportSchema.columns
  404 when the report id is unknown
Frontend calls it through the existing api.get(path) helper and triggers a download
named report-{id}.csv.
```

`api/reports/__init__.py` (route registration) is a shared file. The parent edits it
in the main checkout. That edit is uncommitted, so the child will not see it; the
brief tells the child to add the one import line itself, and step 4 excludes that
file from the child's patch.

## The backend brief (the `objective` string)

```
GOAL: GET /api/reports/{id}/export?format=csv returns the report as CSV, with tests.
CONTEXT: FastAPI app in api/. Report model: api/reports/models.py
  (ReportSchema.columns is the ordered column list). Contract:
    GET /api/reports/{id}/export?format=csv
      200 text/csv, header row = column names from ReportSchema.columns
      404 when the report id is unknown
  Route registration: add exactly this line to api/reports/__init__.py so tests run:
    from .export import router as export_router; app.include_router(export_router)
CONVENTIONS: copy structure, naming and test style from api/reports/list.py and
  tests/api/test_list.py
OWN (write only these): api/reports/export.py, tests/api/test_export.py,
  api/reports/__init__.py (the one line above, nothing else)
DO NOT TOUCH: api/reports/models.py, web/, docs/, pyproject.toml, uv.lock
STEPS:
  1. read_file api/reports/models.py and api/reports/list.py.
  2. write_file api/reports/export.py exposing `router` with the endpoint above.
  3. write_file tests/api/test_export.py: 200 with the right header row, 404 for an
     unknown id.
  4. Run INSTALL once, then VERIFY until green.
INSTALL (once, before tests): uv sync
VERIFY: uv run pytest tests/api/test_export.py -q
RULES: no git commit/branch/push/stash/reset; no full suite; no new dependencies;
  a change needed outside OWN, or a contract that does not fit: stop and report it
REPORT (last message, under 40 lines): worktree root (`pwd`); files changed, one per
  line; VERIFY command and its summary line verbatim; undone or uncertain, or "none"
```

Spawn call: `command_id: par-backend-1`, `role: implementer`,
`task_name: csv export backend`, `worktree_isolation: true`, `objective: <the brief>`.

The frontend brief has the same shape: OWN is the two `web/src/reports/` files,
INSTALL is `cd web && pnpm install --offline`, VERIFY is the vitest command, the
contract block is identical, DO NOT TOUCH lists `web/package.json`, `web/pnpm-lock.yaml`,
`api/`. Spawn with `command_id: par-frontend-1`.

## While they run

`subagent_wait` twice, each with its own `command_id` and `timeout_ms: 300000`. The
parent writes `docs/reports.md` in the main checkout meanwhile. The frontend child
returns first; the backend wait returns `timeout`: leave it, integrate the frontend,
end the turn if nothing else is pending; the result arrives when idle.

## Integrate

Each report ends with a root such as `/work/app/.muse/worktrees/20260902-4f1a`; that
is `WT` in the step-4 recipe.

- Frontend patch: no excludes.
- Backend patch: `--exclude=api/reports/__init__.py`, because the parent already made
  that edit in the main checkout.
- `search` (literal) for `export_router` and `/export` to confirm both sides match.

Full suite once: `uv run pytest -q && (cd web && pnpm vitest run)`. Report: files per
child, both VERIFY lines, the two suite summary lines, the two worktree paths still
under `.muse/worktrees/`.
