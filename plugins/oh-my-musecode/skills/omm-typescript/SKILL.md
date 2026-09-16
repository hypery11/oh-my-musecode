---
name: omm-typescript
description: "Use when writing or reviewing TypeScript or JavaScript (.ts, tsc or eslint red): strict tsconfig, no any, unions over enums, validated boundaries, promises handled, tests beside code; Do not use for framework UI work (omm-frontend)."
---

# TypeScript hygiene

Contract: a TS or JS change is done only when typecheck, lint, format check and the
tests have run after the last edit and their summary lines are pasted from `bash`.
Reviewing ("look over my TS"): read-only, omm-review's format; probe regex, severity
map and anti-pattern table in `references/review.md` beside this file. Components,
hooks, routing, styling: omm-frontend. This is the type and tooling layer.

## 0. Study the project's contract first

- `read_file` `tsconfig.json` and its `extends` chain. Note `strict`,
  `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `verbatimModuleSyntax`,
  `isolatedModules`. No `strict: true`: say so once; never flip it inside a feature
  change (ratchet, section 2). Never loosen a flag.
- `read_file` `package.json`: scripts, runner (vitest, jest, `node --test`, bun),
  linter config (`eslint.config.*`, `.eslintrc*`, `biome.json*`), formatter config.
  Package manager from the lockfile (`pnpm-lock.yaml`, `yarn.lock`, `bun.lockb`, else
  npm). Match all of it; never add a second linter, formatter, or runner.
- CHECK = those scripts, else `npx tsc --noEmit -p .`, `npx eslint <files>`,
  `npx prettier --check <files>`, the runner on the touched test file. One-shot
  `bash`, `yield_time_ms` up to 300000, never watch mode. Run CHECK at baseline;
  name pre-existing errors by file; never claim clean over them.
- Plain JS, no tsconfig: same rules via `// @ts-check` plus JSDoc types or
  `checkJs` in `jsconfig.json`. Do not rename files to `.ts` unless asked.
- Over 3 files: `write_todos`, one item per file, sub-steps type / test / CHECK.
  `edit_file` per change; `write_file` only for a new file.

## 1. Rules

- Strict on, escape hatches justified. `// @ts-ignore` never; `// @ts-expect-error`
  only with the reason on the same line. `// @ts-nocheck` is a finding.
- No `any`. Unknown shape is `unknown`; narrow it, then it has a type. "Works on
  anything" is a generic `<T>`. `as X` only after a check the compiler cannot see,
  reason on the line; `as unknown as X` and `as any` are findings. `satisfies T`
  checks a literal without widening it. Untyped dependency: a `declare module` shim
  in `types/`, typed to the members you call.
- Discriminated unions over enums. `{ kind: 'circle'; r: number } | { kind: 'square';
  side: number }`, switched on `kind`, `default: assertNever(x)` so a new variant fails
  to compile. A value set that needs a runtime object: `const X = {...} as const` and
  `type X = (typeof X)[keyof typeof X]`. `const enum` never (breaks
  `isolatedModules`). Two booleans that cannot both be true is a union in disguise.
- Narrow at the boundary with a validator. Anything from outside (request body,
  `JSON.parse`, env, argv, DB row, storage, message, third-party response) is
  `unknown` until a schema parses it; the type comes from the schema
  (`z.infer<typeof S>`), never written twice. Use the schema library the repo has
  (`search` for `zod`, `valibot`, `arktype`, `ajv`). Validate once at the entry;
  interior code trusts its parameters: no `?.` on a field the type says is present.
- Every promise handled. Awaited, returned, or collected into `Promise.all`; a
  deliberate fire-and-forget is `void p.catch(report)`, never bare. `async` inside
  `forEach`, `setTimeout`, an event handler, or a constructor drops rejections:
  `for...of` with `await`, or `Promise.all`. `catch (e)` binds `unknown`: narrow
  before `.message`; rethrow with `{ cause: e }`, never swallow. Exported async
  functions declare `Promise<T>`.
- Nullability: `?.` and `??`; `||` only for booleans (`0`, `""` are values). Exported
  functions: explicit return type. Data you do not mutate: `readonly`.
- Lint and format clean, no silent disables. `eslint-disable` only with the reason on
  the line. Formatter on touched files only; no format-only hunks in a behaviour
  change.
- Types from one place. Each package exports its public types from one file
  (`types.ts` or its `index.ts`); consumers `import type { X }` from there. `search`
  (literal) for a second `interface X` or `type X =` before adding one. Export what is
  public, not `export *` from a root barrel.
- Tests beside the code. `foo.ts` gets `foo.test.ts` in the same directory unless the
  repo already uses `__tests__/` or `test/`: match what exists. A type that IS the
  product (mapped type, guard) gets a type test: `expectTypeOf` (vitest) or
  `// @ts-expect-error` lines. Red first, green pasted: omm-tdd.

## 2. Judgment calls

| Situation | Decision |
|---|---|
| `interface` or `type` | interface for shapes to extend or implement; type for unions, tuples, mapped and function types. Repo convention wins |
| Repo already has `enum`s | leave them; new value sets as `as const` objects; never both styles in one module |
| `?` or `\| undefined` | under `exactOptionalPropertyTypes` they differ: `?` = may be absent, `\| undefined` = present, empty. One meaning per field |
| `null` or `undefined` for absent | the repo's choice; one function never returns both |
| `!` non-null | tests only, or right after a check the compiler lost; better: bind once and check it |
| No validator library | one boundary: a hand `is` guard, a test per rejected shape; three or more: propose one, never add it unasked |
| `any` leaks from a library | one typed adapter wraps the call; the `any` never crosses the module line |
| Legacy repo, strict off | ratchet: one flag on, its errors fixed, commit; or a stricter `tsconfig.strict.json` for new directories |
| Clever type or union plus runtime check | a type that needs a comment to read loses to the union |
| `Partial<Domain>` in a signature | define the shape the function needs; `Pick` it from the source type |
| `export default` | named exports; default names drift per importer. A framework that requires it: follow it |

## 3. Report

```
Types:  <cmd> -> <summary line>, exit <n>
Lint:   <cmd> -> <summary line>, exit <n>
Format: <cmd> -> <summary line>, exit <n>
Tests:  <cmd> -> <summary line>, exit <n>
Boundaries: <entry point> -> <schema or guard>   (or: none)
tsconfig gaps: <flags missing>   (or: none)
Pre-existing: <errors left untouched, by file>   (or: none)
```

Red is a result: paste it and stop. Never weaken a check to pass it.

## Refuse

- `any`, `as`, `!`, or a disable comment to make CHECK green.
- Loosening tsconfig or an ESLint rule inside a feature or fix change.
- A validator, runner, or formatter the repo lacks, unasked.
- Format-only hunks mixed into a behaviour change.
- "Types look fine" without the pasted `tsc --noEmit` run.

## Micro-example

Report: "grant() sometimes receives `undefined` as level."

```ts
export function handle(req: any) {
  const body = JSON.parse(req.body);
  if (body.type === 'admin') grant(body.id, body.level);
  else grant(body.id);
}
```

One cause, three defects: `any` in, `JSON.parse` out as `any`, `grant`'s promise
dropped. `package.json` has `zod`: schema at the boundary, type from the schema,
exported once, exhaustive switch, result out.

```ts
// role.ts
export const Role = z.discriminatedUnion('type', [
  z.object({ type: z.literal('admin'), id: z.string(), level: z.number().int().min(1) }),
  z.object({ type: z.literal('member'), id: z.string() }),
]);
export type Role = z.infer<typeof Role>;

// handle.ts
export async function handle(raw: unknown): Promise<Result<void, 'invalid'>> {
  const parsed = Role.safeParse(raw);
  if (!parsed.success) return { ok: false, error: 'invalid' };
  const role = parsed.data;
  switch (role.type) {
    case 'admin': await grant(role.id, role.level); break;
    case 'member': await grant(role.id); break;
    default: assertNever(role);
  }
  return { ok: true, value: undefined };
}
```

`handle.test.ts` beside it: `rejects a body without type` (red first: the old code
reaches `grant` with `undefined`), `awaits grant with level for admin`,
`expectTypeOf<Role['type']>().toEqualTypeOf<'admin' | 'member'>()`. CHECK pasted:
`tsc` 0 errors, eslint clean, `2 passed`. The existing `enum Status` stays: out of
scope, reported as a follow-up.
