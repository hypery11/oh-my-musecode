# Python tooling reference

Loaded on demand from `omm-python`; the decisions are in SKILL.md. Every command is
one-shot for `bash`. When a tool is missing (`command not found`), say so and use the
next row that exists; never install into the system interpreter (bundled:python-env
owns environment creation).

## Pinning, by lockfile present

| Lockfile | Add or bump a dependency | Refresh the lock | Install from lock |
|---|---|---|---|
| `uv.lock` | `uv add <pkg>==<ver>` (`--dev` for test-only) | `uv lock` | `uv sync` |
| `poetry.lock` | `poetry add <pkg>@<ver>` (`--group dev`) | `poetry lock --no-update` | `poetry install` |
| `pdm.lock` | `pdm add <pkg>==<ver>` | `pdm lock` | `pdm sync` |
| `requirements.in` + `requirements.txt` | edit `requirements.in` | `pip-compile requirements.in` | `pip-sync` |
| bare `requirements.txt` | add `<pkg>==<ver>` by hand (`pip index versions <pkg>` for the current) | none | `pip install -r requirements.txt` |
| `pyproject.toml` only, no lock | add to `[project] dependencies` with a `>=,<` range and say the repo has no lock | none | `pip install -e .` |

`pip install <pkg>` with nothing written to a manifest is never done. One dependency
per change; the lockfile diff is part of the report.

## Type checker config keys

| Checker | Config table | Strict switch | Per-line escape |
|---|---|---|---|
| mypy | `[tool.mypy]` in `pyproject.toml`, or `mypy.ini` / `setup.cfg` | `strict = true` (or the individual flags `disallow_untyped_defs`, `no_implicit_optional`, `warn_return_any`) | `# type: ignore[<code>]`; run with `--show-error-codes` (default since 0.990) |
| pyright | `[tool.pyright]` or `pyrightconfig.json` | `typeCheckingMode = "strict"` | `# pyright: ignore[<rule>]` |
| ty / pyrefly (if the repo uses one) | `[tool.ty]` / `[tool.pyrefly]` | tool default | its own comment syntax; read the config |

Missing third-party stubs: `mypy --install-types --non-interactive` only inside the
project env; otherwise `[[tool.mypy.overrides]] module = "<pkg>.*" ignore_missing_imports = true`
with the package named, never a global `ignore_missing_imports`.

## Ruff

- `ruff check --statistics .` shows which rules fire before you touch anything.
- A repo with no `[tool.ruff]`: run with defaults (`E`, `F`) and say so; propose
  `select = ["E", "F", "I", "B", "UP", "SIM", "PTH", "RUF"]` as a separate change.
  `PTH` is the pathlib family, `UP` the modern-syntax family, `B` bugbear (mutable
  defaults `B006`, bare except `E722`, f-string in logging `G004` when `G` is on).
- `ruff check --fix` applies safe fixes only; `--unsafe-fixes` changes behaviour and
  needs a green test run after.
- `ruff format --diff <file>` to preview; never hand-align code.

## `search` patterns for the checklist (mode `regex`, `glob: ["**/*.py"]`)

| Finding | Pattern |
|---|---|
| bare except | `except\s*:` |
| broad except | `except (Exception|BaseException)\s*(as \w+)?:` |
| mutable default | `def \w+\([^)]*=\s*(\[\]|\{\}|set\(\))` |
| os.path | `os\.path\.|os\.getcwd|os\.listdir` |
| old formatting | `%\s*\(|\.format\(` |
| range(len) | `range\(len\(` |
| type: ignore without code | `type:\s*ignore\s*$` and `type:\s*ignore\s*#` |
| untyped def (rough) | `def \w+\([^)]*\)\s*:` (no `->`) |
| sys.path hack | `sys\.path\.(insert|append)` |
| print in library | `^\s*print\(` under `src/` |
| sleep in tests | `time\.sleep` under `tests/` |
| open without encoding | `open\([^)]*\)` then inspect for `encoding=` |

## Dataclass, pydantic, TypedDict, NamedTuple

| Need | Use |
|---|---|
| your own code builds it, no validation | `@dataclass(frozen=True, slots=True)` (`kw_only=True` past 3 fields) |
| external data, must validate or coerce, pydantic present | `pydantic.BaseModel` at the edge; `TypeAdapter(list[Model])` for collections |
| external data, pydantic absent, small shape | dataclass plus one `from_mapping` classmethod that raises the module's error type |
| must stay a dict (JSON passthrough, third-party kwargs) | `TypedDict`, `total=False` for optional keys |
| tiny immutable tuple with names, unpacked by callers | `NamedTuple` |
| settings from env | `pydantic-settings` if present, else a dataclass built in one `load_settings()` (omm-config) |

## Pytest fixtures over hand-rolled setup

| Instead of | Use |
|---|---|
| `tempfile.mkdtemp()` + cleanup | `tmp_path` |
| `os.environ["X"] = ...` and restore | `monkeypatch.setenv` / `monkeypatch.delenv` |
| patching a module attribute by hand | `monkeypatch.setattr(mod, "name", fake)` |
| capturing `print` | `capsys.readouterr()` |
| asserting a log line | `caplog.records` with the level set via `caplog.set_level` |
| a loop of inputs in one test | `@pytest.mark.parametrize("given, expected", [...])` |
| `try: ... except X: pass else: assert False` | `with pytest.raises(X, match="...")` |
| `unittest.mock.patch` on the unit under test | fake the boundary the unit calls instead |

Run shapes: one test `pytest path/test_x.py::test_name -q`, one file
`pytest path/test_x.py -q`, package `pytest -q`; `-x` to stop at the first failure,
`--lf` to rerun last failures, `-p no:randomly` when order dependence is suspected.
