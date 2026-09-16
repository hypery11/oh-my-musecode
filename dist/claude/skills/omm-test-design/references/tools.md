# Test tooling by ecosystem

Used by omm-test-design steps 0 and 5. Runner detection and single-test commands
live in omm-tdd's `references/runners.md`. Run everything with `bash`, one-shot,
`yield_time_ms` raised for slow suites. A tool is missing: say so, one install
attempt if permitted, otherwise the manual flip probe from step 5.

## Coverage with an uncovered-lines listing, and mutation

| Ecosystem | Coverage (uncovered lines) | Mutation |
|---|---|---|
| Python, pytest | `pytest --cov=<pkg> --cov-report=term-missing -q` (pytest-cov) | `mutmut run && mutmut results` |
| Node, vitest | `npx vitest run --coverage --coverage.reporter=text` | `npx stryker run` |
| Node, jest | `npx jest --coverage --coverageReporters=text` | `npx stryker run` |
| Rust | `cargo llvm-cov --text` (cargo-llvm-cov) or `cargo tarpaulin` | `cargo mutants` |
| Go | `go test -coverprofile=c.out ./... && go tool cover -func=c.out`; `-html=c.out` for lines | `gremlins unleash` |
| Java, Kotlin | `./gradlew jacocoTestReport` or `mvn jacoco:report` | PIT: `mvn org.pitest:pitest-maven:mutationCoverage` |
| .NET | `dotnet test --collect:"XPlat Code Coverage"`, then `reportgenerator` | `dotnet stryker` |
| Swift | `swift test --enable-code-coverage`, then `llvm-cov report` on the `.profdata` | `muter` |
| Ruby, RSpec | SimpleCov (`COVERAGE=1 bundle exec rspec`) | `mutant run` |

Study the listing, not the number. A line executed by a test with no assertion on its
effect still counts as covered; the flip probe (step 5) is what catches that.

## Boundary fakes worth reaching for

| Boundary | Python | Node | Rust | Go | JVM / .NET |
|---|---|---|---|---|---|
| HTTP out | `responses`, `respx` | `msw`, `nock` | `wiremock` | `httptest.NewServer` | WireMock, WireMock.Net |
| Clock | injected `now()`; `freezegun` as last resort | `vi.useFakeTimers` for timers only | a `Clock` trait | `func() time.Time` field | `java.time.Clock`, `TimeProvider` |
| Filesystem | `tmp_path` | `memfs`, `os.tmpdir()` | `tempfile::tempdir()` | `t.TempDir()` | `@TempDir`, `Path.GetTempPath()` |
| Database | `testcontainers`, `pytest-postgresql` | `testcontainers` | `sqlx::test`, `testcontainers` | `testcontainers-go` | Testcontainers |
| Mail, queue | recording fake (`sent: list`) | recording fake | recording fake behind a trait | interface plus hand fake | recording fake |

Rules: one fake per boundary, shared by the suite, hand-written when under fifty
lines. A recording fake exposes what it received (`sent`, `published`) so tests
assert on content, never on call counts. Mocking libraries (`unittest.mock`,
`jest.fn`, `mockall`, Mockito) only at a boundary trait or interface, never on your
own modules.

## Vocabulary

- Stub: canned answers, no behaviour, no assertions.
- Fake: a working implementation with a shortcut (in-memory repository, recording
  mailer). The default at a boundary.
- Mock: records calls to assert on them. Avoid; where the outgoing call is the
  behaviour, use a recording fake and assert on its record.

## Reading uncovered lines, in order of cost

1. Error branches: `except`, `catch`, `Err(_)`, `if err != nil`, `default:` in a match.
2. Early returns and guards: nil, empty, permission denied, not found.
3. Cleanup paths: `finally`, `defer`, `Drop`, unlock after failure.
4. Retry and timeout paths.
5. Configuration branches: feature flags, env-dependent defaults.

Each uncovered line becomes a behaviour sentence in the plan, or an explicit "not
worth a test" with the reason.
