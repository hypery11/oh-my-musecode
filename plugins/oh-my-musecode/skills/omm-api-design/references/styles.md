# Per-style specifics

Companion to `omm-api-design`. The decisions in SKILL.md apply to every style; this
file holds what differs. Take the block for the surface at hand. Where the repo's
inventory (SKILL.md section 1) disagrees with a row here, the inventory wins.

## REST (HTTP + JSON)

- Default spec format when none exists: OpenAPI 3.x, one file or one file per tag,
  `components/schemas` for every body, `components/responses` for the error shape.
- Naming: plural nouns in paths, one case throughout (`snake_case` or `camelCase`
  per inventory), no verbs except a `:action` suffix for non-resource operations.
- Status is a category, not the contract: 200 read/update, 201 create with
  `Location`, 202 accepted (async), 204 delete; 400 malformed, 401 unauthenticated,
  403 forbidden, 404 not found (also when existence itself is confidential), 409
  conflict or idempotency key reused with a different body, 422 semantically
  invalid, 429 with `Retry-After`. 5xx is never a client mistake.
- Error body, one shape for all: `{"error": {"code", "message", "details": [{"field",
  "code"}], "request_id"}}`; or RFC 9457 `application/problem+json` if the inventory
  already uses it. `code` values enumerated in the spec.
- Pagination: `?limit=&cursor=` -> `{"data": [...], "next_cursor": null}`. The cursor
  is opaque (encoded sort key), sort order stable and documented, `limit` capped.
- Idempotency: `Idempotency-Key` header on POST; store key + response ~24 h; same key
  with a different body -> 422 or 409.
- Long-running: 202 + an operation resource (`id`, `status`, `error`, result link);
  never hold a request past the client's timeout.
- Versioning: `/v1/` path major, or a date header if the inventory does so.
  Deprecation: `Deprecation: true`, `Sunset: <http-date>`, `Link: <...>;
  rel="successor-version"`, plus the changelog entry.
- Also breaking: content type change, URL structure change, auth scheme change,
  removing a status code clients handle, renaming a query parameter.
- Tools: `oasdiff breaking <base.yaml> <head.yaml>`, `spectral lint openapi.yaml`,
  `redocly lint`, `openapi-generator validate -i openapi.yaml`.

## gRPC (protobuf)

- Default spec format: `.proto` files, one package per major (`acme.billing.v1`),
  `<Verb><Noun>Request` / `<Verb><Noun>Response` per method, never a shared or
  empty request type (you cannot add fields later without a new method).
- Naming: `PascalCase` services and messages, `snake_case` fields, `UPPER_SNAKE`
  enum values prefixed with the enum name, first value `<ENUM>_UNSPECIFIED = 0`.
- Errors: canonical status (`INVALID_ARGUMENT`, `NOT_FOUND`, `ALREADY_EXISTS`,
  `FAILED_PRECONDITION`, `RESOURCE_EXHAUSTED`, `PERMISSION_DENIED`,
  `UNAUTHENTICATED`, `UNAVAILABLE`) plus `google.rpc.Status` details:
  `ErrorInfo.reason` carries the machine code, `BadRequest.field_violations` the
  per-field list. `UNKNOWN` and `INTERNAL` are never client mistakes.
- Pagination: `page_size`, `page_token` -> `next_page_token` (empty = end).
- Idempotency: `request_id` field on every mutating request.
- Long-running: return an `Operation` message (`name`, `done`, `error`, `response`)
  and a `GetOperation` method.
- Versioning: new major = new package and new service; v1 messages are frozen.
  Deprecation: `[deprecated = true]` on the field or method, comment naming the
  replacement and the removal release.
- Breaking: change a field number or type; remove a field without `reserved`;
  rename a field (JSON mapping and generated code change); change an enum number;
  rename a service or method (wire path); switch unary and streaming; move a
  message between packages. New enum values reach old clients as unknown: check
  generated `switch` statements.
- Tools: `buf breaking --against '.git#branch=main'`, `buf lint`, `protolock`.

## CLI

- Default spec format: the `--help` text of each command, plus a documented exit
  code table and an example invocation per command, in the repo's docs or the
  command's own long help.
- Naming: `noun verb` subcommands matching the existing tree (`issues export`);
  long flags `--kebab-case`; the same flag means the same thing in every command;
  short flags only for the handful used constantly.
- Output contract: stdout is machine output (`--json` or `--format`, NDJSON for
  streams); stderr is diagnostics and progress. Exit codes documented: 0 ok, 1
  failure, 2 usage, others per repo. The default human output is not a contract
  unless the repo says it is; `--json` always is.
- Errors: one line on stderr, `error: <code>: <message>`; with `--json`, an error
  object on stdout carrying the same `code`.
- Pagination: `--limit`, `--cursor`, or `--all` that streams pages.
- Idempotency: `--dry-run` on every mutating command; `--if-not-exists` for create.
- Versioning: semver of the binary; flag tiers stable / `--experimental-*`.
  Deprecation: warning on stderr when the old name is used, alias kept for one
  major, marked in `--help`.
- Breaking: remove or rename a command or flag; change positional order; change the
  `--json` shape or default output that docs call stable; change an exit code's
  meaning; rename a config key or environment variable.
- Tools: golden `--help` snapshot tests; `bash`: run `<cli> <cmd> --help` on base
  and head and `diff` them.

## Library (public package API)

- Default spec format: the public signatures with doc comments, plus a `## Errors`
  and a `## Examples` section per entry point (doc tests where the language has
  them).
- Naming: the language's own convention; verbs for functions, nouns for types;
  request and result types named symmetrically; no abbreviation the package does
  not already use.
- Errors: typed values or exceptions with a stable variant or code; `#[non_exhaustive]`
  error enums in Rust; an exception hierarchy in Python and TypeScript; sentinel or
  typed errors in Go. Never a message string clients must match.
- Extensibility: options struct or builder past three parameters;
  `#[non_exhaustive]` on public structs and enums; sealed traits or interfaces when
  user implementation is not intended.
- Versioning: semver of the package. Deprecation: `#[deprecated(since, note)]`,
  `@deprecated` JSDoc, `warnings.warn(..., DeprecationWarning)`, `// Deprecated:`
  in Go; kept for one major with the replacement named.
- Breaking: remove or rename an export; change a signature or return type; add a
  required method to a trait or interface others implement; add a field to a public
  struct without `non_exhaustive`; tighten a generic bound; change an exception
  type; change a default.
- Tools: `cargo semver-checks check-release`, `api-extractor run`, `apidiff` or
  `gorelease` (Go), `griffe check` (Python), `japicmp` (Java).
