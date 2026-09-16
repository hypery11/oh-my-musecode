# Skill routing (opt-in)

`omm enable skill-routing` turns one Muse Code feature on for one workspace: a **per-turn skill
router**. Every prompt is matched against a small library of sharply-triggered skills that live
outside the ordinary skills catalog, and the best matches (at most three) are handed to the host,
which renders them as a second, per-turn catalog block the model can `read_skill`. Nothing else
in omm depends on it; it is off by default and stays off until you ask for it, for the reasons
in [Why it is opt-in](#why-it-is-opt-in).

Everything below was measured on `1.0.1-R2006.1` (`research/experiments/skill-routing.md`,
`docs/host-reality.md` "Budgets" / "Trust lifecycle" / "Paths"; the block-size formula was
bisected on 2026-09-03, see `content/routing/README.md`).

## What it is

Muse composes the model's context from ordered blocks. Order 200 is the `skills_catalog`: every
discovered skill (built-in, user, project, plugin) with its description, capped at 32,000 bytes.
Order 201 is `selected_skills_catalog`: a block that exists only when a `UserPromptSubmit` hook
that declared `"outputCapabilities":["skills.v1"]` returns

```json
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit",
  "selectedSkills":[{"id":"omm-commit-message","path":"/abs/ws/.omm/skills/omm-commit-message/SKILL.md","description":"…"}]}}
```

The host writes `"supported_output_capabilities":["skills.v1"]` on such a hook's stdin — that
field is the whole negotiation — and, under both gates, renders the entries at order 201:

```
<system-reminder source="selected-skills">
Muse Code loaded authenticated Project-scope skills selected for this run. These are summaries only; use read_skill with the exact id or absolute path.
<skill-catalog source="selected_skills_catalog">
<skill id="omm-commit-message" scope="project" path="…/.omm/skills/omm-commit-message/SKILL.md" hook-source="project" …>
<description>Write or reword the message for an already-shaped commit …</description>
</skill>
</skill-catalog>
</system-reminder>
```

`read_skill omm-commit-message` (or the absolute path) then returns the body. This is proven end to
end offline: `tools/mockprovider/run-skill-routing.sh` runs `omm enable skill-routing`, a real
`muse exec` through the local mock provider whose scripted model calls `read_skill` on the routed
id, and reads back `<read-skill-result name="omm-commit-message" status="ok">` from `session.jsonl`;
`GATES=off` is the negative control (no block, `unknown-skill`).

The two things that make this worth having:

- **Routed skills are invisible at order 200.** The library lives at `<ws>/.omm/skills/`, which is
  not a discovery root, so it costs nothing on turns that do not need it, and it can grow past what
  the 32 KB catalog could carry.
- **It is a real router, not a hint.** The selection is replaced every turn; `read_skill`
  resolves a routed id or path the ordinary catalog knows nothing about (skill-routing.md V3).

## What `omm enable skill-routing` does

Run it at the root of a **trusted git workspace** (`omm trust .` first — a project hooks file in
an untrusted workspace is never loaded, silently; there is no error to see). It is one ledgered
transaction (ARCHITECTURE.md §4), `--dry-run` previews it, and every path is contained
(`canonicalize` + `strip_prefix`, never through a symlink):

1. **Measure order 200 for this workspace.** One `muse exec --provider echo --trust-workspace hi`
   in a throwaway copy of the workspace's project skill roots (`.agents/skills`, `.codex/skills`,
   `.claude/skills` — never the workspace itself, so none of your own project hooks run) against a
   throwaway data root seeded with the host's plugin store. The catalog estimate stands in when the
   host cannot be run, and the report says which.
2. **Copy the library** from `content/routing/library/<id>/` to `<ws>/.omm/skills/<id>/SKILL.md`
   — real regular files (`read_skill` refuses a symlink or anything behind one, V2), each a
   `workspace` ledger entry (`copy`, `exclusive`) reconciled with R3's four outcomes: a file you
   edited is left alone and named, a second run is a no-op.
3. **Write the router handler** into `<ws>/.muse/hooks.json` (the project hook tier):

   ```json
   {"hooks":{"UserPromptSubmit":[{"hooks":[
     {"type":"command","command":"omm hook route","timeout":5,
      "statusMessage":"omm: routing skills","outputCapabilities":["skills.v1"]}]}]}}
   ```

   `command` is `omm hook route` when `omm` on `PATH` is the binary that ran `enable`, else that
   binary's absolute path, shell-quoted (`$SHELL -c` runs it; `/bin/sh` when `SHELL` is unset).
   A file omm creates is ledgered `copy`/`exclusive`; a file that already existed is parsed (never
   overwritten when it does not parse), the handler is merged into its own matcher-less group after
   yours, and the entry is `hooks-merge`/`shared-key` with the pre-omm bytes and mode in
   `prior.original`. The pre-write bytes also go to `$OMM/snapshots/<ts>/workspace/`.
4. **Record the state.** `<ws>/.omm/routing.json` (ledgered) and `$OMM/config.json` →
   `"skill_routing": true` plus a `routing` record: the workspace, the handler command, the
   measured `order200_bytes`, the library ids and the worst-case `order201_max_bytes`. `omm run`
   reads `skill_routing` and exports both gates.

The report ends with the budget line and the two gates:

```
budget: order 200 10,031 B (measured) + order 201 at most 2,939 B (three of 7 skills) = 12,970 of 31,984 B; headroom 19,014 B
gates: `omm run` exports MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1 and MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY=1; a plain `muse` needs them exported in the shell
```

## The two gates

Routing is double-gated by **environment variables the host reads at startup**; neither can be set
from `settings.json`:

| gate | effect |
|---|---|
| `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1` | the host advertises `skills.v1` to a handler that declares it (the `supported_output_capabilities` field on stdin) |
| `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY=1` | the host consumes the returned `selectedSkills` and renders order 201 |

With one or neither: the handler still runs and completes, the router sees no negotiation and
answers `{}`, and no block composes — nothing breaks, nothing routes. `omm run -- …` exports both
while `skill_routing` is on (`omm run --dry-run -- --version` shows the plan); a `muse` started any
other way needs them in its environment.

## The router: `omm hook route`

Runs in the host's 16-key scrubbed hook environment (`HOME` and `PATH` pass, `XDG_*` does not),
never spawns, decides in memory, prints once, exits 0 (R16). The decision itself — read the
library, parse, score, budget, validate, serialise — takes well under a millisecond (asserted by a
unit test); the rest of the sub-5 ms budget is process start, which `crates/omm/tests/routing.rs`
times over 20 cold runs (5 ms for the release binary, 12 ms for the unoptimised test profile).
Per turn:

1. `{}` unless the event is `UserPromptSubmit` with `skills.v1` in `supported_output_capabilities`,
   a `prompt` and an absolute `cwd` (the workspace).
2. Reads `<cwd>/.omm/skills/*/SKILL.md` (at most 64, leniently: a broken one is skipped) and
   scores each against the prompt with its `metadata.triggers`: both are lower-cased and split on
   non-alphanumerics; a trigger occurring as whole words scores `1 + its word count`, so a
   three-word phrase outranks three loose words; the sum decides. Top three with a positive score,
   by score then id — deterministic across turns.
3. Budgets the selection (next section): descriptions are trimmed at a word boundary (the last
   entry first, never below 48 bytes), then entries are dropped from the end.
4. Validates every entry the way the host will (absolute path under the root, no `..`, basename
   `SKILL.md`, a regular file, lowercase id, description 1–1,024 bytes, unique paths, ≤ 32 entries,
   stdout ≤ 16,384 bytes) and drops what would fail — one bad entry discards the whole selection
   on the host side, so none is ever sent.

## The caps and the shared budget

| cap | value |
|---|---|
| routed skills per turn, all handlers together | 32 (33 → `aggregate-limit`, nothing rendered) |
| one description | 1,024 bytes |
| hook stdout | 16,384 bytes (`output_too_large`, the process tree killed) |
| **order 200 + order 201** | **31,984 bytes** |

The last one is the important one, and it has three outcomes (skill-routing.md V1, bisected to the
byte): with the descriptions the block fits → rendered whole; only the description-less block fits
→ **every routed description is dropped silently** (`status: completed`, no diagnostic); neither →
`selected_skills:rejected:combined-budget`, no block. The rendered order-201 block costs

```
280 + Σ over entries ( 301 + len(id) + len(<ws>/.omm/skills/<id>/SKILL.md)
                       + len(<ws>/.muse/hooks.json) + len(xml-escaped description) )   bytes
```

so the router budgets every turn against `31,984 − order200 − 256` (a margin: `hook-configured-
order` grows a digit per ten handlers ahead of ours, and a measurement is a snapshot). `order200`
is the larger of the size `omm enable` measured for this workspace (`$OMM/config.json` when its
`routing.workspace` is this one, `<ws>/.omm/routing.json` otherwise), falling back to the catalog
estimate the session-start hook uses, then to the built-in block alone. Slimming order 200 buys the
router room byte for byte — `omm settings set run.context_slimming.skill_catalog_descriptions
first_sentence` is the lever (R18) — and `omm enable skill-routing` again re-measures.

Concretely, with the 35-skill bundle on 1.3.0-R3057.1 (measured: bundle `full` 14,034 B,
`first_sentence` 13,966 B per `content/catalog.json` budget; built-ins 15,427 / 7,076 B): under
the default profile order 200 is 21,042 B with no user skills, so the router has ~10.7 KB of room and
fits its worst case (2,657 B) even beside 8 KB of user skills. Under `full` descriptions order 200 is
29,461 B — room ~267 B, the worst case no longer fits whole — so the router emits fewer entries or
none, and reports the shortfall in the block's diagnostics rather than overrunning the shared budget.
That is the host's ceiling, not a defect; the default profile exists so that a user never meets it
by accident.

Doctor **D16** reads it all back: off is a pass; on, it checks the workspace's trust, the handler in
`.muse/hooks.json` and that its program word still resolves, the library files, and the headroom
against the **live** order-200 size of the doctor's echo session (the recorded size under `--fast`),
warning with the exact fix when the router is budgeting against a stale number or the cap is within
the margin.

## The cache-prefix cost

Order 200 is `cache_class=stable_prefix`. Order 201 is `cache_class=runtime_prefix`: it is rebuilt
every turn, so the prompt-cache prefix is invalidated from order 201 onward on every turn that
routes something different from the last — the cached tokens up to and including the catalog are
kept, everything after (rules and blocks composed later, the conversation) is re-sent. On a
provider that bills cached input at a discount, a routed turn costs more than an un-routed one;
on a turn where nothing matches the router prints `{}` and there is no block at all. That, and the
two experimental gates, is why this is not part of `omm install`.

## Why it is opt-in

- **Two experimental environment gates**, settable only in the environment: an install that
  silently depended on them would look broken to anyone who starts `muse` without `omm run`.
- **The cache-prefix cost above**, paid on every turn that routes.
- **A trust dependency**: the project hook tier is silent when the workspace is untrusted.
- **A shared budget with the catalog**: a fat order 200 steals the router's room, and the failure
  just under the cap is silent.

## Turning it off

`omm disable skill-routing`, in the workspace, reverses the transaction byte for byte: every
library file still holding omm's bytes is removed (an edited one is kept and named, and stays in
the ledger for `omm uninstall` to show); a hooks file omm created is removed when unedited, else
omm's handler is taken out and the pre-omm bytes and mode written back when nothing else changed
(otherwise the merged document is kept with only the handler gone, and the report says so); the
directories omm created go when empty; `skill_routing` and `routing` leave `$OMM/config.json`
(the file itself goes when `enable` created it). `omm uninstall` covers the same entries by the
general rules (§4) — a merged hooks file that still carries the handler is kept and named there,
so run `disable` first. Re-running `enable` converges: unchanged files are no-ops, a shrunken
library removes the files it no longer ships (while they are omm's bytes), the measurement is
refreshed.

## The library

`content/routing/library/<id>/SKILL.md` — seven skills for common follow-ups, each with a
`metadata.triggers` list and a `Do not use …` clause: `omm-commit-message`, `omm-test-triage`,
`omm-dep-upgrade`, `omm-api-lookup`, `omm-release-notes`, `omm-flaky-test`,
`omm-pr-description`. Ids carry the `omm-` prefix and may not collide with a catalog or bundled
skill (those are at order 200 already). The format, the trigger semantics and the byte formula are
in `content/routing/README.md`; every file validates with `muse skills validate` (`unknown_fields:
[]` — triggers sit under `metadata`, which the validator knows). The tree is claimed as a whole by
the generator and never packaged (`omm_manifest::content::ROUTING_DIR`); `omm lint`'s
symlink/backslash scan covers it.

## Files and commands

| what | where |
|---|---|
| enable / disable | `omm enable skill-routing`, `omm disable skill-routing` (cwd = the workspace; `--dry-run`, `--json`, `--yes`) |
| the router | `omm hook route` (project-tier handler, `crates/omm/src/cmd/c_tune/routing.rs`) |
| the transactions | `crates/omm/src/cmd/c_tune/routing_enable.rs` |
| doctor D16 | `crates/omm/src/cmd/c_tune/routing_doctor.rs` (appended by `omm doctor`) |
| workspace state | `<ws>/.omm/skills/<id>/SKILL.md`, `<ws>/.omm/routing.json`, `<ws>/.muse/hooks.json` — all `workspace` ledger entries |
| global state | `$OMM/config.json` → `skill_routing`, `routing` |
| the gates | `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS`, `…_APPLY` (exported by `omm run`) |
| proof | `crates/omm/tests/routing.rs` (real host, echo), `tools/mockprovider/run-skill-routing.sh` (scripted model, `read_skill`) |
