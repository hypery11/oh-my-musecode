---
name: "omm-rust"
description: "Use when writing or reviewing Rust (.rs, clippy is red, make it idiomatic): borrows over clones, no unwrap outside tests, thiserror in libs / anyhow in bins, clippy -D warnings; Do not use for a dependency bump alone."
---

# Rust

Contract: a Rust change is done when `cargo fmt --all -- --check`, `cargo clippy
--all-targets -- -D warnings` and `cargo test` are green and their tails are pasted from
`bash`. Writing: under omm-tdd (`read_skill` `plugin:omm:omm-tdd`), the failing test in
a `#[cfg(test)]` module beside the code; this skill shapes each green step. Reviewing:
read-only, omm-review's report format (`path:line`, Scenario), sections 1-3 as the
probe list.

## 0. Study the crate

- `read_file` `Cargo.toml`: `[lib]` or `[[bin]]` (decides section 2), `edition`,
  `rust-version` (use nothing newer), `[lints]`, deps. A dep already in the tree is the
  one you use; a new one is a decision to state, not a default.
- `read_file` the head of `lib.rs`/`main.rs`: `#![deny(..)]`, `#![warn(missing_docs)]`,
  `#![forbid(unsafe_code)]` are the crate's rules. `search` (literal) the target module
  for its error type, `pub(crate)` habit, and where `mod tests` sits. The repo's lint
  set wins over this skill; do not add lints it disabled.
- Baseline with `bash` (`yield_time_ms` up to 300000, one shot): the section 4 gate.
  Record pre-existing failures by lint or test name; fix none unless asked; never
  claim green over them. Workspace: `-p <crate>` in the loop, `--workspace` at the end.

## 1. Ownership first

Decide who owns each value before writing the signature.

- Parameters borrow: `&str`, `&[T]`, `&Path`, `&T`. By value (`String`, `Vec<T>`, `T`)
  only when the function stores it. `impl Into<String>` / `AsRef<Path>` only on public
  constructors whose callers really pass mixed types.
- Return owned. A borrowed return only when the lifetime is plainly the receiver's
  (`fn name(&self) -> &str`). Return `impl Iterator<Item = T>` over a `Vec` built for
  the caller.
- A `.clone()` that exists to satisfy the borrow checker: name the alternative (borrow,
  hand over the value, split the borrow, take the index first) or state the cost and
  keep it. `Arc`/`Rc` handles and small `Copy` types clone freely.
- Named lifetimes only when the compiler demands one; anything stored in a collection
  or held across `.await` owns.
- `Rc<RefCell<T>>` / `Arc<Mutex<T>>`: shared mutation is a design decision; write why
  the ownership tree cannot express it. Never a fix for a borrow error. No guard across
  `.await`.
- Invalid states unrepresentable: an enum over `bool` flags or paired `Option`s; a
  newtype for ids and units.

## 2. Errors

- Library (`[lib]`, or anything another crate depends on): one `thiserror` enum per
  module, one variant per outcome a caller branches on, `#[source]` kept, `#[from]`
  only when nothing is lost. Never `Box<dyn Error>` or `anyhow` in a public signature.
- Binary (`main.rs`, CLI, service): `anyhow::Result`; `.context("what this layer
  knows")` at each `?` crossing a layer (path, key, id). `main() -> anyhow::Result<()>`.
- `unwrap()` / `expect()`: none outside `#[cfg(test)]`, doctests, `build.rs`. One
  exception: `expect("invariant: <why>")` where the invariant is provable from the
  lines above. On parsed input, I/O, env, or a value from another module it is a
  finding: Scenario "crash on a normal path". External data: `.get(i)`, never `v[i]`.
- `?` over `match` on `Err`; `match` only to branch on the variant. `Option` is absence,
  `Result` is failure; `.ok()` on a `Result` only with a comment saying why.
- `panic!`, `unreachable!`, `assert!`: bugs only, with a message. `todo!` never in a
  change you call done.

## 3. Shape

- Iterators over indices: `for x in &v`, `iter().enumerate()`, `zip`, `windows(n)`,
  `chunks(n)`, `filter`/`map`/`collect`. An index loop only when the index is the output.
  Collect once, at the end; never `collect` then `iter` again.
- Public surface: `pub(crate)` until a second crate or the docs need it; `pub use`
  re-exports from `lib.rs`. Every `pub` item carries a `///` doc: one sentence,
  `# Errors` when it returns `Result`, `# Panics` when it can. `#[non_exhaustive]` on
  public enums and config structs; `#[must_use]` on builders and pure functions. No
  `pub` field on a type with an invariant.
- Derive `Debug` always; `Clone, PartialEq, Eq, Default` when meaningful; `Copy` only
  on small plain data. `Display` for user text, never `Debug`.
- Exhaustive `match` on enums you own (no `_ =>`). Integer widths via `try_from`, not
  `as`. `unsafe`: smallest block, `// SAFETY:` naming the invariant, a test exercising
  it; `#![forbid(unsafe_code)]` when the crate has none.
- Tests: `#[cfg(test)] mod tests { use super::*; }` at the bottom of the same file;
  `tests/` only for the public API; `#[should_panic(expected = "..")]`, never bare.
- Naming, derive table, async bounds, lint names, Cargo hygiene: `references/idioms.md`
  beside this file.

## 4. Gate

`bash`, in this order, tails pasted:

```
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps 2>&1 | tail -5      # when pub items changed
```

Red clippy: fix the code, not the lint. `#[allow(clippy::<lint>)]` only on the item,
with a `// reason:` comment, each listed in the final message. Never at crate level,
never `-A warnings`. Review severity:
Blocker = panic or UB reachable from input; Major = cause dropped, `pub` leak of a
dependency type, borrow bug hidden by a clone; Minor = index loop, missing `# Errors`,
item-level allow; Nit = fmt (omit unless asked).

## Judgment calls

- `Box<dyn Trait>` vs generic: generic when one concrete type per call site or the path
  is hot; `dyn` when stored in a collection or behind a plugin boundary.
- Existing code breaks these rules: inside your change follow the file; propose the
  migration in one sentence. Fixing it drive-by is omm-refactor.
- A dep bump needed to get there: its own change, never folded in.

## Refuse

- `unwrap()` outside tests "for now"; `.clone()` in a loop with no cost stated.
- `#![allow(clippy::all)]`, `#[allow(warnings)]`, or clippy skipped "because it builds".
- `unsafe` without `// SAFETY:`; `pub` on every item, or on a field so a test compiles.
- `std::process::exit` or `panic!` on an expected error inside library code.
- A new error, async, or serialization crate when the tree already has one.
- "Clippy is clean" without the pasted run.

## Micro-example

Request: "return the ids of active users from the config file". `Cargo.toml`: `[lib]`,
`thiserror` and `toml` already deps. Draft under review:

```rust
pub fn active_ids(path: String) -> Vec<u64> {
    let text = std::fs::read_to_string(path).unwrap();
    let cfg: Config = toml::from_str(&text).unwrap();
    let mut out = Vec::new();
    for i in 0..cfg.users.len() {
        if cfg.users[i].active { out.push(cfg.users[i].id.clone()); }
    }
    out
}
```

Findings: `unwrap()` on I/O and parse (Scenario: mistyped path -> panic); `String`
taken but only read; index loop; `clone()` on a `Copy`; no doc; no error type. Rewrite:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read {path}")]
    Io { path: PathBuf, #[source] source: std::io::Error },
    #[error("invalid config {path}")]
    Parse { path: PathBuf, #[source] source: toml::de::Error },
}

/// Ids of users marked `active` in the TOML config at `path`.
///
/// # Errors
/// `Io` when the file cannot be read; `Parse` when it is not a valid config.
pub fn active_ids(path: &Path) -> Result<Vec<u64>, ConfigError> {
    let text = fs::read_to_string(path)
        .map_err(|source| ConfigError::Io { path: path.to_owned(), source })?;
    let cfg: Config = toml::from_str(&text)
        .map_err(|source| ConfigError::Parse { path: path.to_owned(), source })?;
    Ok(cfg.users.iter().filter(|u| u.active).map(|u| u.id).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_ids_missing_file_is_io_error() {
        let err = active_ids(Path::new("/no/such/file")).unwrap_err();
        assert!(matches!(err, ConfigError::Io { .. }));
    }
}
```

Gate: `fmt --check` no output; clippy `Finished`; `test result: ok. 3 passed`. Not
touched: the caller's `unwrap()` in `src/main.rs`; reported as a finding, its own change.
