# content/ validation record (everything that is not a skill)

Binary: `.host/bin/muse-bin-1.0.1-R2006.1` (`Muse Code 1.0.1 (1.0.1-R2006.1)`). Every invocation ran
through a wrapper that sets `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME` to a throwaway sandbox,
`MUSE_NO_AUTO_UPDATE=1 NO_COLOR=1 MUSE_EXPERIMENTAL_PLUGINS=1`, and `cd`s into a scratch workspace
(never the repo root, never the real HOME). TUI captures used `tools/pty/drive.py` with `BIN`/`HOME`/`WS`
pointed at the sandbox and `tools/pty/render.py` for the frames. Date: 2026-09-02.

Package under test: a throwaway native package assembled from `content/` (commands, hooks,
reminders, and - for the bundle checkpoints - the 12 skills), manifest `name: "omm"`.

## 1. Commands (`commands/omm-doctor.md`, `omm-cost.md`, `omm-status.md`)

Id lint against `docs/host-data/slash-commands.json` (39 rows + 6 hidden + 9 aliases = 53 names),
the 9 reserved ids, the 15 bundled skill ids and the 26 built-in tool names: `omm-doctor`,
`omm-cost`, `omm-status` -> `ok` (no collision; `/doctor`, `/cost`, `/status` are distinct names).

`muse plugins validate <pkg> --json` (commands + hooks + reminder only):

```
valid True diagnostics [] family native
compat summary full
  hook:omm-guard supported
  hook:omm-session-start supported
  hook:omm-stop supported
  command:omm-cost supported
  command:omm-doctor supported
  command:omm-status supported
  reminder:omm-verify-nudge supported
```

TUI (pty harness, `--provider echo --yolo`, package installed with `plugins install --scope user`,
gate NOT set at run time) - typing `/omm`:

```
⟩ /omm
  /omm-cost    Run omm cost, name the biggest context sources and the exact settings that cut them
  /omm-doctor  Run omm doctor and explain every finding with its exact fix command
  /omm-status  One-screen omm status - ledger, active profile, plugin capability trust, workspace trust
```

`/help` -> Custom commands (first run; `[--json]` hints since removed, see the 2026-09-02 pass below):

```
  General  Commands  [Custom commands]
  Browse custom commands
  /omm-cost [--json]
  /omm-doctor [--self-test] [--report-drift]
  /omm-status [--json]
```

Second pass: `omm-cost.md` and `omm-status.md` no longer carry `argument-hint` (the advertised
`/omm-cost --json` expanded to `omm cost --json --json`, which clap rejects with `the argument
'--json' cannot be used multiple times`, rc 2); the bodies now run exactly `omm cost --json` /
`omm list --json` and tell the model to ignore anything typed after the command. `omm-doctor.md`
keeps `[--self-test] [--report-drift]`. Re-validated: `plugins validate` `valid True diagnostics []
manifest_family native summary full`, `command:omm-cost` / `omm-doctor` / `omm-status` all
`supported`; `plugins inspect` lists the three with `enabled_default: true`.

## 2. Hooks (`hooks/session-start.json`, `guard.json`, `stop.json`)

Closed-field check: each file carries only `id event command timeoutMs statusMessage async`
(`extra fields: none`). `command` is the argv array `["omm","hook","<name>"]`; the validator does
not treat bare argv words as package paths:

```
"command": ["omm","hook","session-start"], "shell_command": null,
"source_path": null, "source_relative_path": null, "timeout_ms": 3000, "async": false
```

Install / approve / inspect in a clean sandbox:

```
installed omm 0.1.0 family native diag [] warning third-party plugin: hooks and reminders require review before activation; commands are active without review while the plugin is enabled
 approved plugin:omm:hook:omm-guard True
 approved plugin:omm:hook:omm-session-start True
 approved plugin:omm:hook:omm-stop True
 approved plugin:omm:reminder:omm-verify-nudge True
  plugin:omm:hook:omm-guard -> trusted_enabled
  plugin:omm:hook:omm-session-start -> trusted_enabled
  plugin:omm:hook:omm-stop -> trusted_enabled
  plugin:omm:reminder:omm-verify-nudge -> trusted_enabled
```

`muse plugins hook test omm:<id> --fixture <f> --json` with a shell shim named `omm` on PATH that
prints exactly the stdout documented in `hooks/README.md` (fixtures: PreToolUse with
`tool_input.command = "git push --force origin main"`, SessionStart `source: startup`, Stop with
`stop_hook_active: false`):

```
### omm-guard
decision: {"should_block": true, "block_reason": "omm guard: force-push-main - `git push --force origin main`", "permission_decision": "deny", ...}
terminal: blocked effects ['blocked', 'permission_denied'] exit 0

### omm-session-start
decision: {"should_block": false, "additional_contexts": ["omm 0.1.0 · profile default · catalog 18,420/32,000 B (27 entries)"], ...}
terminal: completed effects ['context'] exit 0

### omm-stop
decision: {"should_block": true, "block_reason": "omm: before calling this done, run the check that proves it (the tests, build or lint you touched) and report its exit status.", ...}
terminal: blocked effects ['blocked'] exit 0
```

Fail-open when `omm` is not on PATH (R16):

```
### omm-guard  (PATH=/usr/bin:/bin)
decision: {"should_block": false, ...}
terminal: failed effects [] exit None error No such file or directory (os error 2)
```

**Closed (2026-09-02, second pass).** The Phase-0 `omm hook <name>` stub printed `not implemented:
hook` and exited 2 - the one BLOCKING exit code - so with the binary on PATH every guarded tool call
and every Stop was blocked. `crates/omm/src/main.rs` now dispatches `Command::Hook` to a fail-open
handler: drain stdin (skipped on a TTY, bounded at 1 MiB), print `{}`, exit 0, for every name; exit 2
is reserved for an intentional deny once the guard/session-start/stop decisions land in Phase 1.
`crates/omm/tests/hook_fail_open.rs` (cargo test, 3 tests) pins it: `omm hook guard|session-start|stop`
with `{}` on stdin, `omm hook no-such-hook`, empty stdin and non-JSON stdin all give rc 0, stdout
exactly `{}`, empty stderr, while `omm cost` still exits 2 with `not implemented: cost`. Re-measured
with the rebuilt `target/debug/omm` on PATH, same package and fixtures as above:

```
### omm-guard          decision should_block False permission_decision None
                       terminal status completed exit 0 effects [] stdout '{}\n' stderr '' duration 3 ms
### omm-session-start  should_block False | completed exit 0 effects [] stdout '{}\n'
### omm-stop           should_block False | completed exit 0 effects [] stdout '{}\n'
### omm-guard, PATH=/usr/bin:/bin  should_block False | failed exit None error 'No such file or directory (os error 2)'
```

## 3. Reminder (`reminders/omm-verify.json` + duty file `reminders/omm-verify.md`)

Capability id is `omm-verify-nudge`, not `omm-verify`, because a reminder id collides with skill
and command ids in the same plugin (measured):

```
duplicate-capability-id: reminder capability id `omm-verify` duplicates another capability id in the same plugin
```

Envelope tag: the reference example's `<system-reminder>` validates and approves but is then
INACTIVE headlessly; `<omm-reminder>` is `trusted_enabled` (both measured through
validate -> install -> approve -> inspect):

```
<system-reminder>{text}</system-reminder>  -> status blocked, reminder_envelope_elevated_approval:
   runtime capability `plugin:omm:reminder:omm-verify-nudge` is inactive: reminder envelope requires current elevated approval
<omm-reminder>{text}</omm-reminder>        -> status trusted_enabled, diagnostic null
```

Shipped declaration (validated `valid True []` as part of the package, all 9 `decision` members
present, `validators: []`): `enabledDefault: false`, `tools: ["read_file"]`, `blocking: false`,
`defaultPriority`/`maxPriority: normal`, `reasoningEffort: low`,
`context.conversation: {mode: bounded, maxTokens: 6000}`, one field `text` (requirement `remind`,
string 1..600 B). Also measured valid: `tools: []`, `maxChildSteps: 8`.

Validator contract discovered on the way (not bound, recorded for later): `verify_next_step` wants
`inputs ["next_step"]` and `outputs {"normalized_next_step": "string", "used_fallback": "boolean"}`;
binding with `outputs: {}` fails validation with exactly that message.

## 4. Themes (`themes/omm-carbon.tmTheme`, `omm-slate.tmTheme`, `omm-paper.tmTheme`)

Python `plistlib` parse: `omm Carbon 34 rules`, `omm Slate 34 rules`, `omm Paper 34 rules`. Files
copied into the sandbox `$XDG_CONFIG_HOME/muse/themes/` and `/theme` driven with the pty harness.

Dark terminal (OSC 11 answered `rgb:1010/1010/1010`) - the picker appends the two dark themes after
the 17 bundled/synthetic rows and hides the light one:

```
  Synthwave '84
  Tokyo Night
  omm Carbon
  omm Slate
```

Round trips (cursor moved with `ESC [ B`, saved with Enter; the list wraps, so 17 downs = omm Carbon,
18 = omm Slate):

```
⟩ omm Carbon        -> settings.json  "tui": { "theme": "custom:omm-carbon" }
                       settings.json  "tui": { "theme": "custom:omm-slate" }
```

Light terminal (`rgb:f8f8/f8f8/f8f8`) - the picker shows the 11 light rows plus `omm Paper` only
(custom themes are background-filtered exactly like bundled ones):

```
  Snazzy Light
  omm Paper
⟩ omm Paper          -> settings.json  "tui": { "theme": "custom:omm-paper" }
```

## 5. Profiles (`profiles/strict.json`, `default.json`, `fast.json`, `ci.json`)

Validator key matrix (`muse config validate --plane defaults --file` on
`{"schema_version":1,"settings":{<key>}}`): accepted - `reasoning_effort`,
`max_consecutive_stop_hook_continuations`, `telemetry`, `feature_config`, `local_session_messaging`,
`notifications`, `tools.web_fetch/web_search`, `context`, `tui.verbose_output`, `agents`,
`context_compaction`, `provider_retry`, `first_turn_minimal_effort_regex`, every `run.*` key used;
rejected - `permissions` (`unknown_member`), `tui.prompt_hint_enabled` (`unknown_member`),
`agent_definitions` (`unknown_member`), `tui.color_depth` and `presets` (`field_not_activated`).

Per profile, wrapped with `permissions` stripped:

```
strict   validator: valid: plane=defaults schema_version=1  rc=0
default  validator: valid: plane=defaults schema_version=1  rc=0
fast     validator: valid: plane=defaults schema_version=1  rc=0
ci       validator: valid: plane=defaults schema_version=1  rc=0
```

Runtime oracle (full slice + `schema_version: 1` written as the sandbox `settings.json`, then
`muse skills list --json` and `muse exec --provider echo hi`, then `session.jsonl`):

```
strict   skills list diagnostics=[] skills 15 | exec -> echo: hi
         committed profile {'kind': 'user_named', 'id': 'omm-strict', 'filesystem': 'managed', 'approval': 'prompt_unmatched', 'reviewer': 'human'}
         tools 20, workflow present | blocks skills_catalog 10442, session_identity 785, workflow_choice 2896, workflow_cookbook 18056
default  skills list diagnostics=[] skills 15 | exec -> echo: hi
         committed profile {'kind': 'built_in', 'id': ':ask-me', 'approval': 'on_request', 'reviewer': 'human'}
         tools 20, workflow present | blocks skills_catalog 4824, session_identity 786, workflow_choice 2896, workflow_cookbook 18056
fast     skills list diagnostics=[] skills 15 | exec -> echo: hi
         committed profile :ask-me (built_in)
         tools 19, workflow ABSENT   | blocks skills_catalog 4824 (no session_identity, no workflow blocks)
ci       skills list diagnostics=[] skills 15 | exec -> echo: hi
         committed profile {'kind': 'user_named', 'id': 'omm-ci', 'filesystem': 'managed', 'approval': 'allow_all', 'reviewer': 'none'}
         tools 19, workflow ABSENT   | blocks skills_catalog 4824
```

`permissions.default_profile` is honoured without `--permission-profile` (source kind
`user_named`). Second pass: the undocumented `description` member was removed from both profile
objects (`PermissionProfileDefinitionInputV1` documents only `approval reviewer filesystem network
extends`; one rejected member voids the whole map) and the strict/ci prose moved to
`profiles/README.md`; the banned two-letter initialism is gone from the profile text. Re-run of the oracle without it:
`config validate --plane defaults` on the stripped slice `valid` rc 0 for both; `skills list`
`diagnostics [] skills 15`; `exec --provider echo hi` -> `echo: hi` rc 0;
`permission_profile_committed.source` = `{kind: user_named, id: omm-strict}` / `{kind: user_named,
id: omm-ci}`, `resolved_snapshot.filesystem.mode: managed`. Two profile shapes that FAIL and were therefore not shipped:
`extends ":ask-me"` + `approval: allow_all` + `reviewer: none` without a `network` override ->
`contradictory authority: proxy_only requires prompting approval with human fallback`, and that
one bad profile voided the whole map (`Permission profile 'omm-strict' is unavailable ... Using
':ask-me'`); `extends ":unrestricted"` -> `invalid parent profile id`. `network.mode: restricted`
and `enabled` both resolve with `allow_all`; `restricted` shipped.

## 6. Catalog (`catalog.json`) and the three host checkpoints on the full bundle

All 28 asset ids pass grammar `^[a-z0-9][a-z0-9._-]{0,79}$`, the `omm-` prefix, and the collision
sets (slash names, reserved ids, bundled skill ids, tool names) -> `ok` for every id.

Checkpoint 1 - `muse skills validate <dir> --json` per skill: all 12 `valid True diag []
compat compatible`.

Checkpoint 2 - `muse plugins validate <full pkg> --json`:
`valid True diagnostics [] family native summary full, declarations 19, unsupported []` (rc 0).

Checkpoint 3 - install + approve + `muse skills list --json`:

```
installed omm family native diag []
package_sha256 sha256:85262cd02f611d870d98e7b304fb7bb7bd432e4471413b22d7583e50ffc22be8   (throwaway assembly, not the release digest)
approved 4 capabilities
diagnostics []  plugin skills listed 12   (plugin:omm:omm-commit ... plugin:omm:omm-verify, all `on`)
```

Budget: one `muse exec --provider echo hi` with the bundle installed rendered a 15,151-B
`skills_catalog` block (10,060 B bundled + 4,693 B plugin + 363 + 35 header/footer). Per-entry
measured vs `catalog.json` estimate (formula `38 + len(plugin:omm:<id>) + len(plugin://omm/<path>)
+ len(desc) + 36`):

```
omm-self 364/364  omm-reflect 455/455  omm-tdd 347/347  omm-debug 453/453  omm-review 373/373
omm-commit 372/372  omm-refactor 374/374  omm-security 374/374  omm-verify 366/366
omm-parallel 466/466  omm-skill-port 382/382  omm-docs 367/367      -> plugin total 4,693 B measured = 4,693 B catalog
```

4,693 B of 21,542 B (22 %); under `first_sentence` 4,625 B (of 27,168 B) after the second pass
(was 4,119 B when only omm-debug and omm-parallel joined their negative clause with `;`). Every
description now joins `; Do not use …` to its first sentence, so the negative trigger survives the
default/fast/ci profiles: measured `first_sentence` block 9,457 B = 4,832 B bundled + 4,625 B plugin,
12/12 entries carry `Do not use`, per entry identical to `budget_bytes` except omm-parallel
(466 -> 398 B: its trailing sentence after the clause is still cut, by design). `full` block
re-measured 15,151 B, per-entry bytes unchanged (a `. ` -> `; ` swap is length-neutral). Non-skill assets cost 0
catalog bytes (`budget_bytes: 0`); the reminder costs per-turn context only while enabled
(`enabledDefault: false`). `budget_bytes` were computed from the SKILL.md files present at
2026-09-02 12:0x local; the skill descriptions were still being edited by the skills task during
this run (omm-skill-port went 253 -> 240 chars), so `omm build`/`omm lint` must recompute them.

## 7. Other files

- `rules/AGENTS.md.tmpl`: 1,646 B (<= 2 KB), markers `<!-- omm:managed-start/end -->` and
  `<!-- omm:user-start -->…<!-- omm:user-end -->`.
- `translation/muse.md`: 8,798 B; every Muse tool name and parameter in the table was read from the
  tool schemas in `research/musecode/workflows-tools.md` §B5. Second pass: heading is now
  `# Tool vocabulary: foreign harness names -> Muse Code` and row 3 says `(foreign \`isolation:
  worktree\`)`; the only remaining vendor-shaped strings under `content/` are path/format/CLI
  literals (`.claude/skills`, `CLAUDE.md`, `$CLAUDE_PROJECT_DIR`, `.claude-plugin`, `muse skills
  import --from claude|codex`).
- `skills/omm-security/SKILL.md`: body trimmed 8,026 -> 7,936 B (file 8,215 B; wording tightened,
  no content moved), so it passes the 8,192 B lint constant now written into ARCHITECTURE §5.3 and a
  decimal 8,000 B reading alike; next largest body is omm-self at 7,989 B.
- No BOM, no symlink, no backslash in any authored filename.

## 8. Notes for the orchestrator

1. ~~`omm hook` stub exits 2 -> blocks~~ closed in the second pass (section 2); keep
   `crates/omm/tests/hook_fail_open.rs` in CI.
2. Reminder capability id is `omm-verify-nudge` (file still `reminders/omm-verify.json`); catalog
   entry `omm-verify-nudge`, kind `reminder`, `duty: reminders/omm-verify.md`.
3. Reminder envelopes must not use `<system-reminder>`/`<continue>`; the generator should lint this.
4. `permissions` cannot be validated by `muse config validate`; the settings writer must strip it
   (and `agent_definitions`, `tui.prompt_hint_enabled`, `tui.color_depth`, `presets`) before the
   validator call and rely on the runtime oracle for those keys.
5. Custom themes are filtered by terminal background like bundled ones: `omm theme omm-paper` on a
   dark terminal will be accepted by settings but the picker will not list it.
6. `omm lint` (Phase 1, `omm-manifest`) must implement the two rules now in ARCHITECTURE §5.3: the
   foreign-product-name grep over `content/` (the two names `repo_lint.rs` assembles plus their bare
   first words; path/format literals and `muse skills import --from claude|codex` exempt) and the code-span-aware foreign-tool
   check (only a backticked name or `the <Name> tool`; `skills/omm-skill-port`, `translation/muse.md`,
   `rules/AGENTS.md.tmpl` allowlisted as translation content). The prose hits a naive regex finds
   (`Read-only`, `## 2. Read everything`, `Write each candidate`, `## 4. Write`, `Write it before`)
   are English verbs, not tool references; no skill instructs the model to call a foreign tool.
   Today's `crates/omm-host/tests/repo_lint.rs` scans `crates docs tools scripts` only.
7. `omm doctor`: after `profile use strict|ci`, assert `permission_profile_committed.source.kind ==
   user_named` and `id == omm-<name>` in the probe session (the one-bad-profile-voids-the-map
   failure is otherwise silent apart from a stderr line).
8. Generator: copy every catalog `duty` path into the package (the reminder declaration's `path`
   points at it) and do not package `hooks/README.md`, `profiles/README.md` or `VALIDATION.md`;
   those three are the only files under `content/` that are neither cataloged nor a `duty`.
9. Skill descriptions: every `Do not use …` clause is joined with `;` (never `. `) so it survives
   `first_sentence`; a new skill must follow suit or its negative trigger vanishes under the
   default profile. Cost of the rule: +506 B under `first_sentence` (4,119 -> 4,625 B), zero in `full`.

## 9. Second pass, 2026-09-02 - findings closed and the overlap record

Closed: hook stub (high), `[--json]` double flag (medium), vendor names in `translation/muse.md`
(medium), the banned initialism in profile text (low), undocumented `description` profile member (low), omm-security
body size (low), `commandWindows` sentence in ARCHITECTURE §5.2 (low; now says `command` only, native
rejects `commandWindows`), negative triggers under `first_sentence` (low; applied to all 12).
Recorded, no content change: foreign-vocabulary false positives (note 6), catalog consistency
(note 8; recomputed budgets match the files and the measured catalog byte-for-byte, 28 asset paths
present, every file cataloged or `duty`, no duplicate ids).

Re-validation after the edits (same sandbox wrapper as the header): checkpoint 1 `muse skills
validate` 12/12 `valid True diagnostics [] compatibility.result compatible`; checkpoint 2 `plugins
validate` on the assembled package `valid True diagnostics [] manifest_family native summary full`,
19 declarations `supported`; checkpoint 3 install (`family native diag []`, `package_sha256
sha256:c5932b67…` - throwaway assembly) + approve 4 -> every `plugin:omm:*` capability
`trusted_enabled` -> `skills list --json` `diagnostics [] plugin skills 12`. Catalog 15,151 B full /
9,457 B first_sentence; plugin 4,693 / 4,625 B; 12/12 descriptions present with their negative
clause in both modes. Hygiene: no BOM, symlink, backslash or non-ASCII filename under `content/`; all
12 descriptions <= 240 chars (longest omm-skill-port at 240), one line, ASCII, `name` == dirname.
`cargo test -p omm` 5 passed (2 unit + 3 hook), `cargo clippy -p omm --all-targets -D warnings`
clean, `cargo fmt --check` clean. Not re-captured: the `/help` TUI frame (hint removal only).

Overlap against the 15 bundled skills - closest description and why there is no collision:
omm-self vs bundled:doctor (`Diagnose Muse Code product/runtime issues... Do NOT use for ordinary
repository code failures`) and bundled:manage-settings (`Any explicit Muse Code setting question...
Never use for ... non-Muse configuration`) - omm-self triggers only when omm is involved and routes
plain settings/faults to those two; omm-reflect vs bundled:read-session (`Locate and read Muse
Code's OWN session logs`) - reflect writes memory and calls read-session only as a sub-step; omm-tdd
and omm-refactor vs bundled:plan (`Do NOT use for ordinary implement, build, fix, debug, or refactor
requests`) - plan is explicit-only, these cover the lane it excludes; omm-debug vs bundled:doctor
(`Do NOT use for ... implementation debugging, build/test hangs`) - exact complement; omm-commit vs
bundled:git (`never commit... unless the user asked for that exact write... Load the body only
before writing Git history`) - same trigger word, different concern (git = WHETHER, omm-commit =
WHAT/message), body defers explicitly; omm-review / omm-security vs bundled:git - read-only, no
history writes, and their two negative clauses partition review vs security; omm-verify vs the
reserved reminder id `verify-reminder` - different namespace, no bundled skill; omm-parallel - none;
omm-skill-port vs bundled:create-skill (`Do NOT use for ... third-party skill/plugin systems`) and
bundled:import (`Resume a third-party coding-agent session`) - create-skill excludes exactly
skill-port's domain and hands new-skill authoring back; omm-docs vs bundled:table-fit (`Do not load
it for a table already inside a file being edited`) - no overlap. Commands omm-doctor / omm-cost /
omm-status: no hit in the 53-name slash set. Hooks: command arrays exactly `["omm","hook",
"guard|session-start|stop"]`, no `commandWindows`, events SessionStart / PreToolUse / Stop
(PascalCase, in hook-events.json), timeouts 3000 / 2000 / 2000; the README deny shape
`{hookSpecificOutput:{hookEventName:PreToolUse, permissionDecision:deny,
permissionDecisionReason:<non-empty>}}` matches hooks.md §5.3 and produced effects
`['blocked','permission_denied']` with the shim. Themes: 3 distinct (bg #0A0A0B / #2A2E33 / #FBF7EF,
34 rules each); picker rows and settings round trips as in section 4. `rules/AGENTS.md.tmpl` copied
to `$XDG_CONFIG_HOME/muse/AGENTS.md` loads as an order-100 rules_file (1,969 B) with the
`omm:managed` markers. `read_skill` output carries a `locator: file://<abs path>` line
(research/musecode/skills.md:746), so omm-review / omm-security's `references/` addressing is valid.

## Note — plugin skills also surface as slash shortcuts

The composer picker lists every plugin skill twice (`/omm-<id> · omm · default skill` and
`/omm:omm-<id> · omm skill`), 27 rows for `/omm-` with this bundle. A future command id must therefore
never equal a skill id. `/help → Custom commands` shows `/omm-doctor [--self-test] [--report-drift]`,
`/omm-cost` and `/omm-status`.

## 10. Skill fill 0.2.0 (2026-09-05) - 23 skills added, 35 in the bundle

Binary: `.host/bin/muse-bin-1.0.3-R2198.1`. Same sandbox discipline as above (throwaway `HOME`,
`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `MUSE_NO_AUTO_UPDATE=1 NO_COLOR=1 MUSE_EXPERIMENTAL_PLUGINS=1`,
`cd` into an empty scratch workspace, `--provider echo`, `--trust-workspace`). Package under test:
the committed `plugins/oh-my-musecode` as regenerated by `omm build` after the 23 rows joined `catalog.json`.

New skill ids (`since: "0.2.0"`, `core: false`): omm-pr, omm-perf, omm-migrate, omm-ci-fix,
omm-release, omm-incident, omm-api-design, omm-errors, omm-observability, omm-config,
omm-concurrency, omm-shell, omm-rust, omm-typescript, omm-python, omm-database, omm-frontend,
omm-test-design, omm-legacy, omm-repo-map, omm-cleanup, omm-handoff, omm-containers. The skill
drafted as `omm-ci` ships as `omm-ci-fix`: catalog ids are unique across kinds
(`catalog.rs` `validate`, "duplicate asset id") and `omm-ci` is already the `profiles/ci.json`
asset, which is also the live `permissions.profiles.omm-ci` name written into `settings.json`.

Static gates (`target/release/omm`, 0.1.0):

```
omm build --check .   no drift — committed version 0.1.0, crate version 0.1.0; digest re-obtained from the binary
omm lint .            0 error(s), 0 warning(s); catalog estimate 13264 / 21542 B (first_sentence 13196 / 27168 B;
                      31602 B with the built-ins disabled); package files 80; host checkpoints ran
```

Host checkpoints in the sandbox: `plugins marketplace add ci <repo-root> --json` -> `plugin_count 1,
skipped []`; `plugins install omm@ci --json` -> `installed.manifest_family native`,
`package_sha256 sha256:61872fd8…358e47` == the root `marketplace.json` digest; four approvals
(`plugin:omm:hook:omm-session-start`, `hook:omm-guard`, `hook:omm-stop`, `mcp_server:doctor`) ->
`decision` each; `skills list --json` -> 50 skills, 35 `plugin:omm:*`, all `activation: "on"`,
`diagnostics []`.

Catalog pressure, one `muse exec --provider echo --trust-workspace hi` per mode, bytes of the
order-200 `skills_catalog` text in `session.jsonl` (`model_request_configured.run_context_messages`):

```
mode            block bytes  entries  with <description>  plugin entries  plugin with <description>  plugin bytes  room       share
full            23,722       50       50                  35              35 (35 carry "Do not use")  13,264        21,542 B   61.6 %
first_sentence  18,028       50       50                  35              35 (35 carry "Do not use")  13,196        27,168 B   48.6 %
```

`first_sentence` = the default profile (`run.context_slimming.skill_catalog_descriptions:
"first_sentence"`, `full_skill_description_ids: ["bundled:git"]`). Both totals reconcile with the
constants: full 23,722 = 10,060 (15 built-ins, gate on) + 13,264 + 398 header/footer;
first_sentence 18,028 = 4,434 + 13,196 + 398. Stage 2 (silent description drop) did not trigger in
either mode: 35/35 plugin entries carry their `<description>`, every one with its negative clause
intact under `first_sentence` (the `; Do not use …` join). Per-entry bytes measured in `full` equal
`catalog.json → budget_bytes` for all 35 rows (measured without the inter-entry newline the
formula's base of 38 includes: 13,229 + 35 = 13,264). 26 tools active (trusted workspace).

Headroom after the fill: 8,278 B in `full` (about 22 more entries at the current ~375 B average),
13,972 B under the default profile. `omm cost` reads the same numbers from a live session.

Review findings closed in this pass (all in `content/`): omm-release body 10,032 -> 8,175 B (the
decisions table, the refusal list and the registry rules moved to `references/decisions.md`; the
two files it already cited, `references/ecosystems.md` and `references/example.md`, now exist);
omm-refactor no longer claims "clean up" (omm-cleanup owns it, named in its negative clause) and
omm-commit no longer claims "make a PR" (omm-pr owns it, named in its negative clause); the
negative clauses of omm-ci-fix and omm-rust no longer name a routed skill (`omm-test-triage`,
`omm-dep-upgrade` exist only when `omm enable skill-routing` is on); every body mention of a
routed skill (omm-flaky-test, omm-test-triage, omm-dep-upgrade, omm-pr-description) was reworded to
the action it stood for; capitalised foreign-tool words used as prose verbs (`Read the log`, `Write
the note`, `Edit to the plan`, `Glob`) were reworded in all 35 bodies so the only remaining hits are
the descriptions' own first words (`Write or fix a Dockerfile`, `Write or review Python`, `Write,
fix, or harden a shell script`) and the C# identifier `Task.Run` in
omm-concurrency/references/tools.md, none of which the lint matches (backticked name or `the <Name>
tool` only). Every `bundled:<id>` reference in a description or body resolves to one of the 15 ids
in `docs/host-data/bundled-skills.json`; every `(omm-<id>)` reference resolves to a catalog skill.

