# Review probes for TypeScript diffs

Report-only companion to omm-typescript. Run omm-review's steps (read everything,
hunt, verify) with these probes added. Every finding: `path:line`, Scenario, Fix.
A regex hit is a candidate; it becomes a finding only with a runtime scenario.

## Probe regex

`search`, `mode: "regex"`, `glob` limited to the diff's files:

```
: any|as any|as unknown as|@ts-ignore|@ts-nocheck|eslint-disable|!\.|JSON\.parse|forEach\(async|Promise<any>|: Function\b|: Object\b|: \{\}
```

Then `read_file` the enclosing function for each hit; the three context lines
are not context.

## Severity map

| Severity | TypeScript shape |
|---|---|
| Blocker | external data cast or `any`-typed reaching a write, auth, or money path; a dropped rejection on a path that reports success; a `catch` that returns a default and hides the failure |
| Major | switch on a union with no `assertNever` default; a type duplicated and already drifted; `.message` read off `unknown`; `?.` on required data hiding a lying type |
| Minor | disable without reason; enum where a union fits; `!` in production code; `export default` in a repo of named exports |
| Nit | `interface` vs `type` style, import order. Omit unless asked |

## Anti-patterns, with the scenario that bites

| Pattern | Scenario | Fix |
|---|---|---|
| `function f(x: any)` | caller passes `undefined`; the error surfaces three calls later as `Cannot read properties of undefined` | `unknown` plus a guard, or `<T>` |
| `JSON.parse(s) as Config` | a missing field passes the cast; its first read is `undefined` in production | schema `.parse`, type from the schema |
| `items.forEach(async (i) => save(i))` | one `save` rejects; nothing catches it; the loop reports done | `for (const i of items) await save(i)`, or `await Promise.all(items.map(save))` |
| `catch (e) { log(e.message) }` | `e` is a string or `undefined`; the log line reads `undefined`, the cause is lost | `e instanceof Error ? e.message : String(e)`, then rethrow or return a result |
| `arr[0].id` without `noUncheckedIndexedAccess` | empty array; TypeError at runtime, compiler silent | enable the flag, or check `arr[0]` first |
| `Object.keys(o).forEach(k => o[k])` | `k` is `string`; the index needs a cast that hides a real key mismatch | a typed `keys` helper: `Object.keys as <T extends object>(o: T) => (keyof T)[]` |
| `enum Status { Active = 'active' }` compared to an API string | `'ACTIVE'` never matches; a silent no-op branch | validate the string into the union at the boundary |
| `type User` defined in two files | one gains `email?`, the other does not; a cast reconciles them | one `types.ts`, `import type` |
| `fn(true, false)` | swapped arguments compile; the reader cannot tell which is which | an options object or a union parameter |
| `map.get(k)!` | the key was removed between check and use; TypeError | `const v = map.get(k); if (!v) return ...` |
| `// eslint-disable-next-line` with no reason | the rule fires on a real bug next edit; nobody knows whether the disable still applies | reason on the line, or fix the code |
| `export default class Foo` | imported as `Foo`, `FooService`, `foo` in three files; a rename misses one | a named export |
| `async` handler passed to an event emitter | the emitter ignores the returned promise; a rejection is unhandled | `(e) => void handle(e).catch(report)` |
| config literal without `as const` | `'GET'` widens to `string`; a union parameter rejects it downstream | `satisfies` or `as const` |
| `Promise<void>` not awaited in a test | the assertion runs after the test ends; the test passes on failure | `await`, or return the promise |
