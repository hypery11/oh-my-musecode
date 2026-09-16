---
name: omm-api-design
description: "Design or review an API contract (new endpoint; REST, gRPC, CLI, library): error model, versioning, pagination, idempotency, compat with existing surface and clients; Do not use when implementing handlers or looking up a library API."
---

# API design

Contract: the output is a written contract in the repo's own format (OpenAPI, proto,
`--help` text, typed signatures): one full example per operation, one error model, a
stated versioning and deprecation rule, a compatibility verdict for every client found.
Prose in the message is not done. Consistency with the existing surface beats a better
idea in isolation. Never write the handlers here; hand the contract to omm-tdd. Run
every command with `bash`.

## 0. Before deciding anything

- Review mode (a contract or diff is handed over): sections 1, 3 and 5 only, edit
  nothing. Design mode: all sections. Both start with the inventory.
- Surface with `search`: `openapi.*`, `swagger.*`, `*.proto`, `schema.graphql`, route
  tables (`router.`, `@app.route`, `http.Handle`), CLI parsers (`clap`, `argparse`,
  `cobra`), public exports (`pub fn`, `export`, `__all__`). No spec file: the routes
  and their tests are the spec; say so.
- Clients with `search` (literal) for every path, rpc, flag or symbol you will touch,
  across src, tests, SDKs, fixtures, docs. Anything published (SDK, public URL,
  released package) has clients you cannot see: treat it so.
- `write_todos`: inventory, one item per operation, compat, examples, lint. Skip it
  for one additive operation.

## 1. Inventory: the existing surface is the style guide

`read_file` the spec whole under 500 lines, else the three operations nearest the one
in hand; one line per row:

| Dimension | Record |
|---|---|
| naming | case, plural or singular nouns, verbs in paths (yes/no) |
| ids | format (`usr_...`, uuid, int), where they appear |
| time, money | RFC 3339 UTC? minor units + currency? |
| error body | exact shape, code vocabulary, status mapping |
| pagination | cursor vs offset, parameter names, page-size cap |
| versioning, auth | URL, header, package major, none; header names; scopes |
| nullability | is absent distinct from null? |

Match every row, the ugly ones included; one convention badly kept beats two. Two
already present: report it, follow the majority, never add a third. A broken
convention (ambiguous, lossy, unsafe) changes across the whole surface in a new
major, never in one endpoint.

## 2. Decide, per operation

| Question | Default | Otherwise when |
|---|---|---|
| resource or action | noun resource, standard verb | no resource state exists (`POST .../quotes:compute`) |
| sync or async | sync under ~2 s p95 | longer: 202 + operation resource with `status`; poll or webhook |
| list shape | cursor: `limit`, opaque `cursor`, `next_cursor` null at end, hard cap | offset only for small fixed sets, and the inventory already does |
| create, send, charge | `Idempotency-Key` (or `request_id`); same key = same response, other body = 409 | never; retries happen |
| batch | per-item result with its own code | all-or-nothing only when atomic and stated |
| enums | strings; state how clients treat unknown values | ints only in proto, 0 = unspecified |

Error model, one shape for every failure: stable machine `code` (enumerated in the
spec), human `message`, optional per-field `details`, `request_id`. Status or gRPC code
is the category; clients branch on `code`, never `message`. List every error each
operation returns, with its trigger; an undeclared code is a defect. Never a bare
string, a 200 carrying an error, or a 500 for a client mistake. Decide once whether a
forbidden id answers 404 or 403.

Versioning: the mechanism in the inventory; none -> URL major for REST, package major
for proto and libraries, stability tiers for CLI flags. Additive changes never bump.
Deprecation = marker (`Deprecation`/`Sunset` header, `deprecated = true`,
`@deprecated`, stderr warning) + replacement named + removal release; removal lands in
the announced version, never the release that deprecates.

## 3. Compatibility check

Breaking on every style: remove or rename a field; change a type, format, default, or
required-ness; tighten validation; change an existing status, code or exit code; narrow
an enum; change pagination semantics; reorder positional arguments. Not breaking: new
optional field, operation, or flag; a new enum value only if clients tolerate unknowns.
Per-style lists, formats, and tools: `references/styles.md` beside this file.

- Repo has a compat tool (`oasdiff breaking`, `buf breaking`, `cargo semver-checks`,
  `apidiff`): run it against the base; paste the output. Missing: classify by hand
  from `git diff <base>...HEAD -- <contract paths>`; do not install silently.
- Each client from step 0: `read_file` the call site; what it sends and expects;
  compatible or breaks, with `path:line`. The one question: an existing client built
  against the old contract, unchanged, sent to the new one; what happens? Anything
  but "works" is breaking.
- Breaking and unavoidable: new version beside the old, deprecation entry, migration
  note. Never an in-place edit of a published shape.

## 4. Draft the contract

- `edit_file` the existing spec; `write_file` only when none exists, in the style's
  default format from the reference. Match the file's order and conventions.
- Per operation: one-line purpose; every request field with type, required,
  constraints; response; every error with code and trigger; one complete example with
  realistic values (`"prj_8f3k"`, not `"string"`), the same ids across the file; for
  lists a second page. Runnable form (`curl`, `grpcurl`, the CLI line); a local
  server exists: run it, paste the output; none: mark it `(not run)`.
- Lint the spec with the repo's tool (`spectral lint`, `buf lint`, or compile); paste
  the result.

## 5. Report

```text
## Decisions
- <question> -> <choice>; because <reason>; rejected <alternative>
## Breaking
1. <client path:line> - <operation>: <what changed>
   Scenario: <client sends / expects> -> <fails how>
   Fix: <additive alternative | new version + deprecation>
## Inconsistencies
- <operation>.<field> - deviates from inventory row <name>; change to <x>
## Missing
- <operation> - <no error list | no example | unbounded list | no idempotency>
## Verdict
<Ship | Ship after fixing #N | Redesign> - <one sentence>. Compat tool: <ran | none>.
```

Order by severity; omit empty sections, never the Verdict. No praise.

## Judgment calls

- Existing convention is wrong (errors as 200, offset pages on a growing table): match
  it in this major; file the fix as the next major with a migration.
- All clients in this repo: a break is fine when every caller moves in the same
  change; paste the `search` count. One client you cannot see: not fine.
- "Just a quick endpoint": the error list and one example are ten lines; skipping
  them is the expensive option. Say so once, then comply and label it.
- Proto: never reuse a field number; `reserved` it. Library: past three parameters,
  an options struct. CLI: stdout is the contract, stderr is for humans.
- In doubt whether a change breaks: it breaks.

## Refuse

- A contract with no examples, or with placeholder values.
- Errors as free text, as 200, or as a different shape per operation; an unpaginated
  list "for now"; sequential or guessable ids on a public surface.
- Renaming a published field in place; removing in the release that deprecates.
- Handler code. "Backward compatible" without the per-client check.

## Micro-example (REST)

"Export a project's issues as CSV." Inventory from `openapi.yaml`:
`/v1/projects/{project_id}/...`, snake_case, `prj_`/`iss_` ids, cursor lists, errors
`{code, message, request_id}`. Clients: `web/src/api/`, `cli/cmd/issues.go`, a
published SDK. Large exports run for minutes: not a sync body. Rejected
`GET .../issues?format=csv` (content type switches on a query parameter; the SDK's
typed decoder breaks). Chosen: `POST /v1/projects/{project_id}/exports` with
`Idempotency-Key`, 202, `{"id":"exp_4k2p","status":"pending"}`; `GET
/v1/exports/{id}` returns `status` and, once `complete`, `download_url`. Errors:
`project_not_found` 404, `export_rate_limited` 429, `invalid_format` 400. Examples
in `openapi.yaml`; `spectral lint`: 0 problems; `oasdiff breaking` against main:
none. Verdict: Ship. Handlers: omm-tdd.
