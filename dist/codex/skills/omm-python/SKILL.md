---
name: "omm-python"
description: "Write or review Python (.py, make this pythonic, mypy or ruff red): typed defs, checker run, dataclass/pydantic at boundaries, no bare except, pathlib, f-strings, pytest, pinned deps; Do not use for environment setup (bundled:python-env)."
---

# Python hygiene

Contract: Python you hand back passes `ruff format --check`, `ruff check`, the repo's
type checker, and `pytest`, with the four runs pasted verbatim from `bash`. Writing:
follow omm-tdd (`read_skill` `plugin:omm:omm-tdd`), failing test first. Reviewing:
read-only, findings as `path:line - claim` in the omm-review format; section 1 is the
probe list. No or broken interpreter: `read_skill` `bundled:python-env` first; never
create a venv or install into the system interpreter here. A missing tool is
reported, not installed.

## 0. Orient

- `read_file` `pyproject.toml` (else `setup.cfg`, `ruff.toml`, `mypy.ini`, `tox.ini`):
  `requires-python`, `[tool.ruff]` select/ignore, `[tool.mypy]` or `[tool.pyright]`
  strictness, `[tool.pytest.ini_options]`. The repo's config wins: never add a rule it
  disabled or widen `select` inside a review.
- Checker: the one configured; both -> both; neither -> `mypy --strict <touched
  files>` for the report only, config is a separate ask.
- Interpreter: the project's, via `uv run`, `poetry run`, or `.venv/bin/`; never a
  bare system `python`. Pin tool from the lockfile present (`uv.lock`, `poetry.lock`,
  `requirements.in`, bare `requirements.txt`). Commands per tool, config keys,
  `search` patterns for section 1: `references/tooling.md` beside this file.
- Baseline with `bash` (`yield_time_ms` up to 300000, one shot, never a watch mode):
  the section 3 gate. Record pre-existing failures by rule code and test name; fix
  them only when asked.
- Over 3 files: `write_todos`, one item per file, sub-steps types / boundary / tests.

## 1. Checklist

Types

- Every `def` you write or touch: annotated parameters and return, `-> None`
  included. `list[str]`, `str | None` when `requires-python` is 3.10+; below it,
  `from __future__ import annotations`.
- `Any` is a decision, not a default: only where the shape is unknown at a boundary,
  narrowed on the next line (`isinstance`, `model_validate`). `# type: ignore[<code>]
  # <why>` and `cast(T, x)  # <why>`: code and reason on the line, or it is a finding.
- Accept abstract (`Sequence`, `Mapping`, `Iterable`), return concrete (`list`,
  `dict`). `Protocol` for duck typing; `TypedDict` for a fixed-key dict; `Literal`
  for a closed string set; `Enum` when values need identity or behaviour.

Boundaries

- Data entering the process (JSON, argv, env, DB rows, HTTP bodies, files) is parsed
  once, at the edge, into a typed object; interior code takes the object, never the
  dict. `pydantic.BaseModel` when it needs validation or coercion and pydantic is
  already a dependency; `@dataclass(frozen=True, slots=True)` otherwise.
- `dict[str, Any]` or `**kwargs` handed more than one level down is a finding.
- An `__init__` that only assigns arguments is a dataclass. Mutable default ->
  `field(default_factory=list)`; `def f(x=[])` is a Blocker.

Errors

- No bare `except:`; no `except Exception:` that neither re-raises nor returns an
  error the caller can see. Narrowest type; `raise New(...) from e` when translating;
  `logger.exception` once, at the boundary that decides the outcome. One module-level
  exception base, a subclass per outcome callers branch on; never `None` for failure.
- `assert` states an invariant; it never validates input (`python -O` deletes it).

Idioms

- `pathlib.Path` over `os.path` and string joins; `Path.read_text(encoding="utf-8")`;
  every `open()` names its `encoding`; `with` on every file, lock, connection.
- f-strings over `%`, `.format`, `+`; the one exception is a logging call, which takes
  `%s` and the arguments lazily (`log.info("loaded %d", n)`).
- Comprehension over `map`/`filter` with a lambda; a generator when the consumer
  iterates once; `enumerate`, `zip(strict=True)`, `.items()`; `range(len(x))` is a
  finding.
- `is None`; `isinstance`, never `type(x) ==`.
- Import-time side effects behind `if __name__ == "__main__": raise SystemExit(main())`,
  `main() -> int`. No `import *`, no `sys.path.insert`; absolute imports at the top.
  Libraries log; a CLI prints results to stdout, diagnostics to stderr.

Tests

- `pytest`, plain `assert`, functions (classes only where the suite already has
  them); `tmp_path`, `monkeypatch`, `capsys`, `caplog` over hand-rolled fixtures;
  `@pytest.mark.parametrize` for the input table; `pytest.raises(X, match=...)`.
- Fake the boundary (`monkeypatch.setattr(module_under_test, "fetch", fake)`), never
  the unit under test. No `time.sleep`, inject a clock. No network.

Format, lint, dependencies

- `ruff format <files>` then `ruff check --fix <files>` on the touched files, never
  hand-format. Fix the lint, not the warning: `# noqa: <CODE>  # <why>` on the line;
  never a file-level `# ruff: noqa`, never a rule dropped from `select`.
- New dependency: add it with a constraint to the manifest, then regenerate the lock
  with the repo's tool so every version is `==`. Never a bare `pip install`, never a
  hand-edited lock. A bump is its own change, never folded in.

## 2. Judgment calls

- Dataclass or pydantic: external data -> pydantic if present; built by your own
  code -> dataclass; must stay a dict (JSON passthrough, third-party kwargs) ->
  `TypedDict`. Adding pydantic to a repo without it is a dependency decision: ask.
- Gradually typed repo: annotate what you touch and its direct callers; a `--strict`
  sweep is its own task, say so instead of starting it.
- Async: only when the caller is already async; never `asyncio.run` inside a library;
  no blocking I/O (`open`, `requests`, `time.sleep`) in a coroutine.
- Target version: `requires-python`, not the local interpreter; a 3.11 feature in a
  3.9 repo is a finding.

## 3. Gate

```text
ruff format --check . && ruff check .
mypy <paths>          # or: pyright <paths>
pytest -q
```

Each as `<cmd>; echo exit=$?`, through the project interpreter, from the repo root.
Paste each final line verbatim. Review severities: Blocker = swallowed exception,
mutable default, `assert` on input, `sys.path` hack in shipped code; Major = untyped
public `def`, dict crossing two layers, unpinned dependency, `type: ignore` without a
code, test that mocks the unit under test; Minor = `os.path`, `%` formatting,
`range(len)`, missing `encoding`; Nit = format (omit unless asked).

## Refuse

- `Any`, `cast`, or `# type: ignore` to make the checker pass.
- `except Exception: pass`, or a default returned on failure.
- A ruff rule disabled, a test skipped, or a tolerance widened to get green.
- `pip install` with nothing recorded in the manifest and lockfile.
- "Types check" or "ruff is clean" without the pasted run.

## Micro-example

```python
import os, json

def load_users(path):
    try:
        f = open(path)
        data = json.load(f)
    except:
        return []
    users = []
    for i in range(len(data)):
        users.append({"name": data[i]["name"], "age": int(data[i]["age"])})
    return users
```

Findings: `:7` [Blocker] bare except returns `[]` for a missing file, bad JSON and a
bad row alike, so empty and failed are indistinguishable; `:5` handle leaked; `:3`
untyped; `:10-11` `range(len)`, dict leaves the boundary; `:1` unused `os` (F401).
Decision: no pydantic in this repo -> frozen dataclass, one error type.

```python
import json
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True, slots=True)
class User:
    name: str
    age: int


class UserFileError(Exception):
    """The users file is missing, unreadable, or malformed."""


def load_users(path: Path) -> list[User]:
    try:
        rows = json.loads(path.read_text(encoding="utf-8"))
        return [User(name=str(r["name"]), age=int(r["age"])) for r in rows]
    except (OSError, ValueError, KeyError, TypeError) as e:
        raise UserFileError(f"cannot load {path}: {e}") from e
```

With pydantic, `TypeAdapter(list[User]).validate_python(rows)` replaces the
comprehension. Test: `test_load_users_missing_file_raises(tmp_path)`,
`pytest.raises(UserFileError, match="cannot load")`. Gate: `ruff check` -> `All
checks passed!`; `mypy src` -> `Success: no issues found`; `pytest -q` -> `2 passed`.
