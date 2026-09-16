---
name: "omm-frontend"
description: "Build or change UI components, forms or pages (React, Vue, Svelte, plain DOM): state ownership, labels and focus, loading/empty/error states, no layout shift, bundle cost, behaviour tests; Do not use for visual styling (bundled:taste)."
---

# Frontend engineering

Contract: a view is done when all four states (loading, empty, error, populated)
exist in the code, every control is keyboard-reachable and named for a screen
reader, nothing moves after first paint, and a test asserts the behaviour through
roles and text, run green after the last edit and pasted from `bash`. Looks
(colour, type, spacing, motion) are `bundled:taste`: `read_skill` it before the
first visual edit. Language hygiene is omm-typescript. Reviewing a UI diff:
sections 1-5 are the hunt list, omm-review's format, edit nothing.

## 0. Study the project before writing a component

- Framework, test runner and a11y lint plugin from `package.json` (`read_file`).
  Per-framework idioms in `references/frameworks.md` beside this file.
- `read_file` the nearest sibling component and its test. Copy file layout,
  naming, styling method, data-fetch pattern. Never introduce a second CSS
  system, fetch layer or form library.
- `search` for existing primitives before writing one: `Button`, `Dialog`,
  `Skeleton`, `ErrorBoundary`, `EmptyState`, `useQuery`, `useForm`. Reuse or
  extend; never author a duplicate.
- Baseline with `bash` (`yield_time_ms` up to 300000): lint, typecheck, unit
  tests, production build once. Note the build's output size for step 4. Record
  pre-existing failures by name.
- `write_todos`: one item per view, sub-steps = four states + a11y + test.

## 1. Boundaries and state ownership

Decide where every piece of state lives before writing markup:

| State | Owner |
|---|---|
| server data | query/loader/cache layer, never component-local |
| filter, page, tab, selected id | the URL |
| form field values | the form (native, uncontrolled) unless validated per keystroke |
| open / hover / focus / pending | the component |
| auth, locale, theme | one store or context at the root |

- Derive, do not sync. A value computable from props or other state is a
  function call, not a second state plus an effect that copies it.
- Effects only synchronise with the outside: DOM, subscription, timer. Fetching
  in an effect is the last resort when a loader or query hook exists.
- One status field, not booleans: `status: "loading" | "empty" | "error" | "ok"`.
  `isLoading && !isError && data.length === 0` is where bugs live.
- Split a component when it renders two unrelated things; never for line count.
  Props in as data plus callbacks, never a parent's setter.

## 2. Every view has four states

- Loading: a skeleton with the populated layout's dimensions. A spinner that
  replaces content, or nothing until data arrives, shifts layout.
- Empty: name what is missing and the one action that fixes it.
- Error: what failed, the user's input kept, a retry control. An error boundary
  per route or panel so one failure does not blank the page.
- Populated: last. Then mutations: disable submit while pending, keep values on
  failure, show the server's message beside the field it concerns
  (`aria-describedby`).
- Async input (search, autocomplete): debounce ~300 ms, abort the stale request
  (`AbortController`), render only the latest result. Abort and clear timers on
  unmount.

## 3. Accessibility is mechanical, not optional

- Native element first: `<button>` for actions, `<a href>` for navigation,
  `<input>`, `<dialog>`, `<details>`. A `<div onClick>` needs role, `tabIndex`,
  Enter and Space handling and a focus style to catch up; it never does.
- Every control has a name: `<label for>`, `aria-labelledby`, or `aria-label`
  on icon-only buttons. A placeholder is not a label; it vanishes on input.
- Focus is a path you walk with the Tab key: visible ring (never `outline: none`
  without a replacement); a dialog moves focus in, traps it, closes on Escape,
  returns focus to the opener; a route change focuses the new heading; a deleted
  row hands focus to its neighbour. Nothing works only on hover.
- Announce async results: `role="status"` for counts and "saved",
  `role="alert"` for errors. Nothing announced = nothing happened.
- Contrast 4.5:1 text, 3:1 large text and UI edges; measure, do not eyeball.
  `alt` on every image, `alt=""` when decorative. Colour never carries state
  alone. `prefers-reduced-motion` honoured by anything that moves.

## 4. Nothing moves, nothing bloats

- Images, video, iframes: `width`/`height` or `aspect-ratio`. Late content gets
  a `min-height`. Never insert above existing content after load. Fonts:
  `font-display: swap` with a metric-matched fallback.
- Before adding a dependency, `search` for one already in the repo that does it,
  get the cost with `bash` (`npm view <pkg> dist.unpackedSize`) and name the
  platform alternative you rejected (`Intl`, `fetch`, `URLSearchParams`,
  `<dialog>`).
- Import the module, not the barrel. Lazy-load by route for heavy screens; never
  anything above the fold. Rebuild; paste the size delta against the baseline.

## 5. Tests assert behaviour, not markup

- Query by role and name: `getByRole("button", { name: "Save" })`,
  `getByLabelText`, `findByText`. `data-testid` last; class names never. A role
  query that fails IS an accessibility finding: fix the markup, not the query.
- Assert what the user sees or what left the component: text, a callback's
  argument, the request body, the URL. Not internal state, not a snapshot.
- One test per state plus one per user action, boundary faked (msw, a fake
  loader, a mock at the network edge), the component real. `user-event`, not
  `fireEvent`.
- omm-tdd loop: red first. End-to-end only for a flow that crosses routes.

## 6. Report

Per view: state owners and URL keys; the four states and how many are tested;
the focus path walked (`Tab -> Save -> Enter; Escape closes`); a11y lint result;
test summary line and exit code; bundle delta; what was not verified and why.

## Judgment calls

- Controlled input: only when you validate per keystroke or derive UI from the
  value; otherwise native form plus `FormData`.
- Optimistic update: reversible and cheap to roll back (toggle, like). Never for
  delete, payment or a server-assigned id.
- Infinite scroll: ship a "Load more" button as well, so keyboard users and the
  footer both remain reachable.
- Framework instinct conflicts with the sibling component's pattern: follow
  the sibling and say so.

## Refuse

- `<div onClick>` as a button; `outline: none` with no replacement.
- Placeholder as the only label; icon button with no name.
- A view with only the happy path; a spinner where a skeleton belongs.
- `useEffect` (or `watch`) that copies props into state.
- Snapshot as the only test; querying by class when a role exists.
- A new dependency without a size number and a rejected alternative.
- Disabling an a11y lint rule to get green; a silent `catch` as error handling.

## Micro-example (React, Testing Library)

Ask: "add a search box that filters the user list." Decisions: `q` lives in
the URL; 300 ms debounce with abort; one status field; skeleton has the list's
height. Red first (`edit_file` into the sibling test file):

```tsx
it("offers to clear the search when nothing matches", async () => {
  server.use(http.get("/api/users", () => HttpResponse.json([])));
  render(<UserSearch />, { wrapper: Router });
  await user.type(screen.getByRole("searchbox", { name: "Search users" }), "zz");
  expect(await screen.findByText(/no users match/i)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Clear search" }));
  expect(screen.getByRole("searchbox")).toHaveValue("");
});
```

Green, the state block only:

```tsx
<label htmlFor="q">Search users</label>
<input id="q" type="search" value={q} onChange={(e) => setQ(e.target.value)} />
<p role="status">{status === "ok" ? `${users.length} users` : null}</p>
{status === "loading" && <ListSkeleton rows={10} />}
{status === "error" && <p role="alert">Could not load users. <button onClick={retry}>Retry</button></p>}
{status === "empty" && <p>No users match "{q}". <button onClick={() => setQ("")}>Clear search</button></p>}
{status === "ok" && <UserList users={users} />}
```

Then loading and error tests the same way; Tab through it once; rebuild and
paste the size delta.
