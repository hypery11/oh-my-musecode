# Runner cheat sheet

Detect the framework, run one test, run one file, run the package. Always
one-shot (no watch mode). Prefer the project's own script when it exists
(`npm test`, `make test`, `just test`, `scripts/test.sh`); read it with
`read_file` and reuse its flags. Detect with `search`, not a recursive grep.

| Framework | Detect (search for) | One test | One file | Package |
|---|---|---|---|---|
| pytest | `pytest.ini`, `pyproject.toml [tool.pytest]`, `conftest.py` | `pytest path/test_x.py::test_name -q` | `pytest path/test_x.py -q` | `pytest tests -q` |
| unittest | `import unittest`, no pytest config | `python -m unittest mod.Class.test_name` | `python -m unittest mod` | `python -m unittest discover` |
| jest | `jest.config.*`, `"jest"` in package.json | `npx jest path -t "name"` | `npx jest path` | `npx jest` |
| vitest | `vitest.config.*`, `"vitest"` in package.json | `npx vitest run path -t "name"` | `npx vitest run path` | `npx vitest run` |
| mocha | `.mocharc.*`, `"mocha"` in package.json | `npx mocha path -g "name"` | `npx mocha path` | `npx mocha` |
| node --test | no config, `node:test` imports | `node --test --test-name-pattern="name" path` | `node --test path` | `node --test` |
| cargo test | `Cargo.toml` | `cargo test mod::name -- --exact` | `cargo test --test file` | `cargo test -p crate` |
| go test | `go.mod`, `*_test.go` | `go test ./pkg -run '^TestName$'` | `go test ./pkg` | `go test ./...` |
| rspec | `.rspec`, `spec/` | `bundle exec rspec path:LINE` | `bundle exec rspec path` | `bundle exec rspec` |
| minitest | `test/test_helper.rb` | `ruby -Itest path -n test_name` | `ruby -Itest path` | `bundle exec rake test` |
| dotnet test | `*.csproj` with test sdk | `dotnet test --filter "FullyQualifiedName~Name"` | same with class filter | `dotnet test` |
| swift test | `Package.swift` with `testTarget` | `swift test --filter Module.Class/testName` | `swift test --filter Module.Class` | `swift test` |
| XCTest (xcodebuild) | `*.xcodeproj`, `*.xcworkspace` | `xcodebuild test -scheme S -only-testing:Target/Class/testName` | `-only-testing:Target/Class` | `xcodebuild test -scheme S` |
| gradle | `build.gradle*` | `./gradlew test --tests 'pkg.Class.method'` | `./gradlew test --tests 'pkg.Class'` | `./gradlew test` |
| maven | `pom.xml` | `mvn -q test -Dtest=Class#method` | `mvn -q test -Dtest=Class` | `mvn -q test` |
| phpunit | `phpunit.xml*` | `vendor/bin/phpunit --filter name path` | `vendor/bin/phpunit path` | `vendor/bin/phpunit` |
| elixir | `mix.exs` | `mix test path:LINE` | `mix test path` | `mix test` |
| dart / flutter | `pubspec.yaml`, `test/` | `dart test path -n "name"` | `dart test path` | `dart test` (or `flutter test`) |

## Naming by language

- Python: `test_<unit>_<condition>_<expected>` in `tests/test_<module>.py`.
- JS/TS: `describe("<unit>")` + `it("<verb phrase> when <condition>")`.
- Rust: `fn <unit>_<condition>_<expected>()` under `#[cfg(test)] mod tests`.
- Go: `TestUnit_Condition` with table-driven subtests named by condition.
- Swift: `func test<Unit><Condition><Expected>()`.
- Java/Kotlin: `<unit>_<condition>_<expected>()` or `should<Expected>When<Condition>()`.

## Reading a red run

- Paste from the first `E`, `FAIL`, `panicked`, `--- FAIL`, or `Expected`
  line through the assertion diff. Skip stack frames inside the framework.
- A red for the wrong reason: `ImportError`, `NameError`, `ModuleNotFoundError`,
  `cannot find`, `undefined is not a function`, `no such file`, compile
  errors, fixture errors. Fix wiring with a stub, then get the behavioural red.
- `0 tests ran`, `no tests found`, `filtered out`: the selector did not match.
  Fix the `-k`/`-t`/`-run` pattern; do not assume green.
- Non-zero exit with no failing assertion anywhere: the environment is broken,
  not the test. Fix the environment first.

## Slow runs

`bash` waits about 10 s by default. Pass `yield_time_ms` (up to 300000) for a
suite that needs longer, so the result lands in one call. If it still runs
past the wait, the final output is delivered to you automatically; do not
poll with `bash_input`, and do not guess the result.
