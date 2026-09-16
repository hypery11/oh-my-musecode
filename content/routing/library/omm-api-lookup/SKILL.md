---
name: omm-api-lookup
description: Answer "what is the signature of / what does X return / where are the docs for" about a library API from the installed source or its docs, quoting the real definition; Do not use for questions about this repo's own code (use search) or for general programming concepts.
metadata:
  triggers: api doc, api docs, documentation for, signature of, what does it return, what does this return, how do i call, how to call, docs.rs, man page, which arguments, parameter list, type signature
---

# API lookup

Goal: the exact, current definition of an API this repo depends on, quoted
from a source of truth on this machine, with the version it came from.
Never answer a signature question from memory: versions drift and an
invented parameter costs a failed build.

## Where the truth is, in order

1. The installed dependency source. Find it with `bash`:
   - Rust: `cargo metadata --format-version 1 | jq -r '.packages[] | select(.name=="<crate>") | .manifest_path'`
     then `search` under that directory for `pub fn <name>` / `pub struct <Name>`;
     `cargo doc --open` is never needed.
   - Node: `node_modules/<pkg>/` - `package.json` for the version, `*.d.ts` for
     the types (`search` for the exported name).
   - Python: `python -c "import <mod>, inspect; print(inspect.signature(<mod>.<fn>)); print(<mod>.__file__)"`
     and `pydoc <mod>.<fn>` for the docstring.
   - Go: `go doc <pkg>.<Name>` or `go doc -all <pkg>`.
   - System commands: `man <cmd>` (`| col -b | head -120`), `<cmd> --help`.
2. The vendored or generated docs in the repo (`docs/`, `*.d.ts`, `openapi.*`).
3. Only if neither exists and network is allowed: the project's own
   documentation site via `curl`, naming the URL and the version it documents.

## Answer shape

- The definition, quoted verbatim (signature or type), with the file path and
  the version (`<pkg> <x.y.z>`).
- One minimal call example that would compile or run against that version,
  using names from the quoted definition only.
- Any deprecation note or feature flag found next to the definition.
- If the API does not exist in the installed version, say so and quote the
  nearest name found instead of guessing an alias.
