# Public API migration recipes

Use when a step touches a public name. Pattern is always the same: add the new
surface, make the old one delegate, move callers, delete the old one in a later green
step (or keep it when consumers outside the repo exist; removing it is their decision).

## Rename a function or class

- Python: `def old(*a, **k): warnings.warn("old is deprecated, use new", DeprecationWarning, stacklevel=2); return new(*a, **k)`.
  Class: `Old = New` plus a comment.
- TypeScript/JavaScript: `/** @deprecated use newName */ export const oldName = newName;`.
- Rust: `#[deprecated(note = "use new_name")] pub fn old_name(..) { new_name(..) }`.
- Go: keep the old symbol with a `// Deprecated: use NewName.` doc comment delegating to the new one.
- Java/Kotlin: `@Deprecated` on the old method delegating to the new one.
- Swift: `@available(*, deprecated, renamed: "newName")`.

## Change a signature

- Add the new parameter with a default that reproduces today's behaviour.
- Removing a parameter: introduce `new_fn` without it, `old_fn` delegates, migrate, delete.
- Reordering parameters: never in place. New name, delegate, migrate.

## Move a module or file

- Python: old module keeps `from new.location import *` plus the explicit names it exported; `__all__` unchanged.
- TypeScript: old file becomes `export * from "./new/location"` (and `export { default } from ...` if it had one).
- Rust: `pub use new::path::Item;` at the old path.
- Go: package moves are breaking; a type alias `type Old = newpkg.New` covers types, functions need wrappers.
- Drop the shim in a later step, after `search` shows zero imports of the old path (tests, scripts, docs, config, and lazy or string-based imports included).

## Language rename tools (prefer over text replace)

- Python: `rope` (`rope.refactor.rename`), or `pyright --outputjson` to list references.
- TypeScript: `tsc --noEmit` after the edit catches every missed typed reference; `ts-morph` for scripted renames.
- Rust: `cargo check` after the edit is the reference list; `rust-analyzer` rename via editor.
- Go: `gopls rename -w <file>:<line>:<col> NewName`, or `gofmt -r 'old -> new' -w <dir>` for simple expressions.
- Java: IDE rename or OpenRewrite recipes.
- Dynamic references survive none of these: `search` for the old name as a string literal before deleting it.

## Not a refactor (stop and report, SKILL.md section 3)

- JSON/YAML keys, database columns, message schemas, cache keys.
- CLI flag names, environment variable names, config file keys.
- URL paths, event names, metric names, log strings that dashboards match on.
- Error codes or exception types callers catch by name.
- Anything a consumer outside this repo compiles or parses against.
