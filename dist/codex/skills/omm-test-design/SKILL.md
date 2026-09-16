---
name: "omm-test-design"
description: "Plan tests (how should I test this, test plan, coverage, too many mocks): level per behaviour, one focus per test, fakes only at process boundaries, redundant tests cut, misses measured; Do not use when asked to write the tests (omm-tdd)."
---

# Test design

Contract: the output is a plan, not code: behaviours in a table, each with a level, a
sentence for a name and the one boundary that gets faked; tests to delete, each with the
test that now covers it; what the suite misses, measured. Study the existing suite before
deciding anything. Writing the tests is omm-tdd's loop; this skill ends where that starts.

## 0. Inventory

- `search` (`glob` `**/*test*`, `**/*spec*`; `output_mode` `files_with_matches`) for the
  suite; `read_file` the files nearest the code. Note runner, fixtures, factories, mock
  library. `search` for `mock`, `patch(`, `jest.fn`, `vi.fn`, `mockall`: a test with
  more mocks than assertions goes on a list.
- Behaviours: one sentence each, `<unit> <does what> when <condition>`, from the public
  surface (exports, endpoints, commands), the requirement and any bug report; not from
  the implementation's branches (step 5). One you cannot phrase in a sentence is two.
- Coverage once, if the tool is installed (`references/tools.md` beside this file).
  Keep the uncovered-lines listing, drop the percentage.
- Over five behaviours: `write_todos`, one per behaviour.

## 1. Level per behaviour

Ask: what must be broken for this test to fail, and would the failure name the cause?
Push each behaviour down a level until the answer changes.

| Behaviour | Level | Real | Faked |
|---|---|---|---|
| parsing, calculation, validation, state machine | unit | the function | nothing |
| your code against a boundary you own (schema, file layout, queue) | integration | the engine: test db, tmp dir, local broker | the outside world |
| your code against a boundary you do not own (payments, mail, vendor HTTP) | unit plus ONE contract test | the client seam | the network (contract test: recorded response) |
| wiring: DI graph, config load, route table, CLI parsing | one boot smoke test | everything | nothing |
| a flow the user performs across components | e2e: happy path plus the failure the user will hit; never the sole test of a behaviour | everything | nothing |

- Too high: slow, flaky, a failure that points nowhere. Too low: the test passes while
  the mock lies. A branch tested through http is a slow unit test.
- SQL, serialization, migrations: integration on the real engine.
- No seam at the level you want: pin at the outermost reachable level, add a seam
  (omm-refactor), then the real test. Never patch your way in.

## 2. Shape of one test

- One behaviour: one arrange, one act, one logical assertion (several asserts on the
  same result are fine). Two acts: two tests. A table of cases is a parametrized test,
  not a `for` loop.
- The name is a sentence with condition and outcome: `rejects_expired_token`,
  `it("returns 404 when the order belongs to another tenant")`. `test_process`,
  `works`: unfocused, rename or split. "and" in the name: split.
- Assert the observable result: return value, raised error, row, file, response body.
  Not that a collaborator was called, not the order of calls. At a boundary the
  outgoing message is the result: assert what the fake recorded (recipient, body),
  never the call count.
- Fixtures: realistic values (a plausible email, a date at a DST edge, a non-ASCII
  name), only the fields the behaviour reads, the rest from a builder (`search` for an
  existing factory first). No mutable fixture shared between tests. Time, randomness,
  ids, env: injected and fixed; never `sleep`.
- Cases: empty, one, many; each bound and one past it; malformed; the bug report's input.

## 3. Fakes only at process boundaries

Fake a thing only when it is outside this process AND slow, non-deterministic, paid or
absent in test: network, clock, randomness, env, vendor services, mail. Your own classes,
repositories and the database you own stay real (containerised engine, tmp dir). Prefer
a hand-written fake (`InMemoryRepo`, `FakeClock`) to a mocking library: a fake has
behaviour, a mock has expectations. One fake per boundary, shared by the suite.

| Symptom | Meaning | Move |
|---|---|---|
| red on a refactor that changed no behaviour | mocks pin the implementation | fake at the boundary, assert the output |
| setup lines outnumber assertions 3:1 | wrong level or too many collaborators | move up a level, or add a seam |
| `assert_called_with` is the only assertion | the test tests the mock | assert what the call caused |
| patching a function in the same module, or a mock returning a mock | no seam | inject the dependency; fake the boundary object, once |

## 4. Delete

Delete a test when another fails for every failure it catches; when its only assertion
is implementation (call order, private state, mock counts); when it is a snapshot nobody
reads; when it duplicates a parametrized case. Prove it: `edit_file` one line to break
the behaviour, run the file with `bash`, note which tests go red, revert. Among tests red
for the same break keep the best-named one at the lowest level. Keep the only test
reaching a path and any you do not understand (`read_file`, rename it instead). Every
deletion in the report names the test that now covers it. Never delete to get green.

## 5. Measure what the suite misses

- Coverage is a list of misses, never a score. Study the uncovered lines: error branches
  first (most common, most costly), then bounds, then config paths.
- Mutation probe on the three most important branches: `edit_file` one flip (`>` to
  `>=`, `and` to `or`, delete an `if` body), run the suite with `bash`, paste the
  summary line. Green after a flip is a missing test: record it as a behaviour. Revert
  before the next flip. A mutation tool is installed: run it instead
  (`references/tools.md`).
- History: `git log --oneline --grep=fix -20`; each fix this suite would not have
  caught is a miss.

## 6. Report

```text
## Plan (<n> unit / <m> integration / <k> e2e; was <a>/<b>/<c>)
| # | behaviour | level | test name | fixture | faked | exists |
## Delete
- <file>::<test> - <reason>; covered by #N; <break> proved it
## Misses
- <path:line> <branch> - <flip> stayed green; add #N
## Seams
- <module> - <dependency to inject>; <tests to move up a level>
## Next
omm-tdd, starting with #<n> (the miss that costs most in production)
```

## Judgment calls

- In-memory stand-in or the real engine: real when dialect, constraints or transactions
  matter (SQLite hides Postgres); in-memory when only the repository contract matters.
- The user wants 100% coverage: coverage measures execution, not verification. Offer
  the misses list instead.
- Writing the tests: omm-tdd. A flaky test or a red run to sort is a debugging job
  (omm-debug), not a design one.

## Refuse

- A plan before reading the existing suite.
- Mocking the unit under test or a collaborator you own.
- A test whose only assertion is a call count or `assert True`, or named after a method.
- Deleting a test to make a run green, or without the break that proved it redundant.
- A coverage percentage as the done criterion.

## Micro-example (Python)

"How should I test `charge(order)`? 14 tests, they all mock everything." It loads a
customer from Postgres, calls the payment API, writes a `Payment` row, sends a receipt.
`read_file tests/test_billing.py`: 11 of 14 `patch` `db`, `payments`, `mailer`; 9 assert
only `assert_called_with`. Coverage: the `declined` branch uncovered.

| # | behaviour | level | faked |
|---|---|---|---|
| 1 | charges the order total in the customer currency | unit: `compute_charge`, extracted, pure | nothing |
| 2 | writes a Payment row with the provider id on success | integration, existing `pg` fixture | `FakePayments` returns a fixed id |
| 3 | raises `PaymentDeclined` and writes no row when the card is declined | integration | `FakePayments.decline()` |
| 4 | sends the receipt only after the Payment is committed | integration | `FakePayments`, `RecordingMailer` |
| 5 | request body matches the provider API | contract, recorded response | the network |

Delete: the 9 `assert_called_with` tests (covered by #2, #4); fold the 2 rounding tests
into #1 as cases. Miss: flipping `status == "declined"` to `!=` stays green; #3 goes
first. Next: omm-tdd from #3.
