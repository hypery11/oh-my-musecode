# Per-framework idioms for omm-frontend

Use the row for the framework `package.json` names. When the sibling component
does it differently, the sibling wins.

## Where each concern lives

| Concern | React | Vue 3 | Svelte 5 | Angular |
|---|---|---|---|---|
| derived value | plain expression or `useMemo` | `computed()` | `$derived` | `computed()` signal |
| outside sync (DOM, timer, subscription) | `useEffect` with cleanup | `watchEffect` / `onMounted` + `onUnmounted` | `$effect` with return cleanup | `effect()` or lifecycle hooks with `DestroyRef` |
| server data | TanStack Query, SWR, router loader, RSC | TanStack Query, Pinia Colada, Nuxt `useFetch` | SvelteKit `load`, TanStack Query | `HttpClient` + signals or `resource()` |
| URL state | router `useSearchParams` | `useRoute().query` + `router.replace` | `$page.url.searchParams` + `goto` | `ActivatedRoute.queryParamMap` |
| uncontrolled form | `<form action>` / `FormData` on submit | native form + `FormData` | native form + `FormData`, or `enhance` | template-driven form |
| controlled form | `useState` per field or react-hook-form | `v-model` | `bind:value` | reactive forms |
| lazy route | `React.lazy` + `Suspense` | `defineAsyncComponent` / route `component: () => import()` | `+page` split by SvelteKit | `loadComponent: () => import()` |
| error boundary | `react-error-boundary` or class component | `onErrorCaptured` in a wrapper | `<svelte:boundary>` | `ErrorHandler` provider per feature |

## Testing

| | Library | Render | Interaction |
|---|---|---|---|
| React | `@testing-library/react` | `render(<X />)` | `@testing-library/user-event` |
| Vue | `@testing-library/vue` or `@vue/test-utils` | `render(X, { props })` | `user-event` |
| Svelte | `@testing-library/svelte` | `render(X, { props })` | `user-event` |
| Angular | `@testing-library/angular` | `await render(X, { componentInputs })` | `user-event` |

Network edge fake: `msw` (`setupServer` in node, `setupWorker` in browser
tests). Fake the request, not the hook or store that issues it.

## Lint rules that catch the Refuse list

- React: `eslint-plugin-jsx-a11y` (`recommended`), `react-hooks/exhaustive-deps`.
- Vue: `eslint-plugin-vuejs-accessibility`.
- Svelte: compiler a11y warnings are on by default; never `svelte-ignore a11y-*`
  without a comment saying why.
- Angular: `@angular-eslint/template/accessibility`.
- Any: `axe-core` via `jest-axe` / `vitest-axe` in the test for each view when
  the project already has it; do not add it just for this change.

## Bundle checks

- Vite: `vite build` prints per-chunk gzip sizes; `rollup-plugin-visualizer`
  when installed. Next: `next build` route table; `@next/bundle-analyzer` when
  installed. Webpack: `--json > stats.json` then `webpack-bundle-analyzer`.
- Dependency cost before install: `npm view <pkg> dist.unpackedSize`, then
  check the package's `sideEffects` and `exports` fields for tree-shakeability.
