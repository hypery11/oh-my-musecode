# Rust idioms: reference tables

Overflow from `SKILL.md`. Load only the section you need.

## Naming (RFC 430)

| Item | Case | Example |
|---|---|---|
| crate, module, function, method, local, field | `snake_case` | `parse_config`, `max_retries` |
| type, trait, enum variant | `UpperCamelCase` | `ConfigError::Parse` |
| const, static | `SCREAMING_SNAKE_CASE` | `DEFAULT_TIMEOUT` |
| lifetime, generic | short lowercase / single upper | `'a`, `T`, `E` |
| feature flag | `kebab-case` | `serde-support` |

- Conversions: `as_x` (free, borrow to borrow), `to_x` (costly, borrow to owned),
  `into_x` (consumes self). Getters have no `get_` prefix; the setter is `set_x`.
- Constructors: `new` for the obvious one, `with_x` / `from_x` for alternatives,
  `Default` when there is a no-argument case. Builders end in `build()`.
- Iterator adaptors: `iter()` borrows, `iter_mut()` borrows mutably, `into_iter()`
  consumes; a custom iterator type is named after its producer (`Lines`, `Chars`).
- Error types end in `Error`; the crate's `Result<T>` alias is fine only in a crate
  with one error type.

## Derive table

| Derive | When | When not |
|---|---|---|
| `Debug` | every type | never skip; use `#[derive(Debug)]` with a manual impl to redact secrets |
| `Clone` | value can be duplicated meaningfully | type owns a handle that must be unique (file, lock) |
| `Copy` | small plain data, all fields `Copy`, no `Drop` | anything with heap data or that may grow |
| `PartialEq`, `Eq` | equality is meaningful | floats (`Eq` impossible), types with identity semantics |
| `Hash` | used as a map key; always with `Eq` | `Hash` without `Eq`, or float fields |
| `PartialOrd`, `Ord` | there is one natural order | order is a view (sort by field instead) |
| `Default` | there is an obvious empty value | the "default" would be invalid (a required id) |
| `Serialize`, `Deserialize` | the type IS the wire shape and is versioned with it | the wire shape differs or drifts: write a DTO |

`#[non_exhaustive]` on every public enum and public config struct that may gain a
variant or field; without it, adding one is a breaking change.

## Clippy lints worth knowing by name

Default (`clippy::all`) already covers most of section 3. Lints to raise per crate in
`[lints.clippy]` when the crate wants them, never per change:

- `unwrap_used`, `expect_used` (deny in lib crates; allow in `#[cfg(test)]`),
- `panic`, `todo`, `unimplemented`, `dbg_macro`, `print_stdout` (library code),
- `cast_possible_truncation`, `cast_sign_loss`, `as_conversions`,
- `needless_pass_by_value`, `large_enum_variant`, `trivially_copy_pass_by_ref`,
- `missing_errors_doc`, `missing_panics_doc`, `must_use_candidate`,
- `module_name_repetitions`, `wildcard_imports` (style; only if already enabled).

Silencing: `#[allow(clippy::<lint>)]` on the smallest item, with `// reason:` on the
line above; `#[expect(clippy::<lint>)]` (1.81+) is better because it fails when the
lint stops firing.

## Async

- One runtime per binary; the one already in `Cargo.toml`. A library stays
  runtime-agnostic: no `tokio::spawn` inside a lib, take a `Future` or a trait instead.
- `Send + 'static` bounds appear where a future is spawned, not on every trait method.
  A `Rc`, `RefCell`, or `MutexGuard` held across `.await` is the usual cause of a
  `future is not Send` error; scope the borrow to end before the await.
- Blocking I/O or CPU work inside async: `spawn_blocking` (or the runtime's equivalent),
  and say why in a comment.
- `async fn` in traits: fine on 1.75+ for same-crate use; `dyn`-compatible traits still
  need `Box<Pin<..>>` or the `async-trait` crate if already present.
- Cancellation: a dropped future stops at its last `.await`. Anything that must
  complete (a write, a lock release) goes in a `Drop` or a spawned task.

## Cargo.toml hygiene

- `edition` current for new crates; never bump the edition of an existing crate as a
  side effect (`cargo fix --edition` is its own change).
- `rust-version` set when the crate is published; CI checks it with that toolchain.
- `[dependencies]` sorted, versions as `"1"` or `"1.2"` (caret, not `=`), `default-features
  = false` plus the features used when the dep is heavy.
- `[lints]` table (1.74+) for crate-wide lint levels, replacing `#![deny(..)]` clusters.
- `[profile.release]`: `lto`, `codegen-units`, `panic = "abort"` only when measured
  (omm-perf), never as a habit.
- Feature flags additive only; a feature that removes an item breaks `--all-features`.

## Tests

- Unit: `#[cfg(test)] mod tests { use super::*; }` at the bottom of the file under test.
  Fixture repeated three times: a `fn` in the test module, not a fixture crate.
- Integration: `tests/*.rs`, public API only; each file is its own crate, so shared
  helpers go in `tests/common/mod.rs`.
- Doc tests: every `///` example compiles and runs under `cargo test`; mark
  `no_run` for network or filesystem, `ignore` never without a reason.
- `#[should_panic(expected = "...")]` always with `expected`; a bare `#[should_panic]`
  passes on the wrong panic.
- Property tests only when `proptest` or `quickcheck` is already a dev-dep.
- Temp files: `tempfile` if present, else `std::env::temp_dir()` with a unique name;
  never a fixed path.
