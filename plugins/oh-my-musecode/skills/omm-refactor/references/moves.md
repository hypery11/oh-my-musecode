# Move recipes

Each recipe is one step of the loop in SKILL.md: apply, migrate callers, run SUITE.
Public names: the alias, deprecation, and shim forms per language are in
`api-migration.md`.

## Extract function / method

1. `read_file` the source range. Pick the exact lines; note every variable they read
   (these become parameters) and every variable they write that is used afterwards
   (these become return values).
2. `edit_file`: insert the new function next to the source, body copied verbatim,
   parameters = reads, return = writes.
3. `edit_file`: replace the original lines with one call.
4. Run SUITE.

More than ~4 parameters needed -> extract a smaller piece first.

## Move to another module

1. `write_file` (new module) or `edit_file` (existing): append the definition
   verbatim, plus the imports it needs.
2. In the old location: replace the definition with a re-export from the new place
   (`from new import X`, `export { X } from "./new"`, `pub use new::X`). Run SUITE.
3. Next step: migrate callers to the new path (`search` the old import path;
   `edit_file` each), delete the re-export. Run SUITE.

Two steps, not one; step 2 alone is already a green checkpoint.

## Rename across files

1. `search` the old name: mode `literal`, `word` true, `output_mode`
   `files_with_matches`. Repeat with `hidden` true for dotfiles and configs. Record
   the list. CamelCase and snake_case variants are separate names; list each.
2. Language rename tool if one is on PATH (`gopls rename`, `rope`, `ts-morph`, an
   IDE's CLI; list in `api-migration.md`). Else per-site `edit_file` from the list.
   Over ~20 sites: a word-bounded replace on exactly the listed files, never on the
   tree: `perl -pi -e 's/\bOLD\b/NEW/g' <files>`.
3. `bash` -> `git diff`; read every hunk. Strings, docs, and unrelated symbols that
   share the name are the usual damage; put those hunks back with `edit_file`.
4. `search` the old name again with `word` true: zero hits outside changelog,
   history, and a kept alias.
5. Run SUITE.

Renaming a file: `git mv` when the repo is git; update imports in the same step.

## Inline

1. Confirm exactly one caller (`search`, `word` true). More than one -> not inline.
2. Copy the body into the caller, substituting parameters. Delete the definition.
3. Run SUITE.

## Dedupe

1. `bash` -> `diff` the two blocks (paste each to a temp file, or compare with
   `read_file` side by side). Any difference beyond names and whitespace -> not a
   duplicate; stop and report (SKILL.md section 3).
2. Identical: extract one copy (recipe above), one green step.
3. Next step, per remaining copy: replace it with a call. Run SUITE after each.

## Characterisation test (when nothing covers the code)

1. Pick 3-8 representative inputs, the odd edges included (empty, max, negative,
   unicode, the case the comment says "should not happen").
2. Call the code as it is; capture the actual output verbatim with `bash`.
3. `write_file` tests next to the existing ones, same naming, asserting exactly that
   output. Do not "correct" an output that looks wrong: assert it, comment it
   `# current behaviour`, add a `later:` todo.
4. Run them green. They are the baseline for this area; tighten or replace them after
   the refactor, as their own step.

## Undo a red step without commits

The step is small: reverse each `edit_file` call (swap `find` and `replace`) in
reverse order. A file created with `write_file`: `bash` -> `rm <path>`. A file that
was clean at baseline: `git restore -- <path>`. Confirm with `git diff` that only
earlier, green steps remain, then run SUITE.
