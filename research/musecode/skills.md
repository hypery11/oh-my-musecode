# Meta Muse Code ("TBH") 1.0.1-R2006.1 — Skills subsystem

Reverse-engineered from the stripped arm64 Mach-O at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
(build sha `e27e408b66`, semver `1.0.1`, reported by the session-log
`runtime.session.metadata` record).

Everything below is either **PROVEN** (reproducible command in a sandbox, or an
exact byte string extracted from the binary at a named offset) or explicitly
marked **INFERRED** / **UNKNOWN**.

Standard preamble used for every command in this report:

```sh
export MUSE_NO_AUTO_UPDATE=1
export HOME=<a throwaway dir under the scratchpad>     # never the real ~
MUSE=/private/tmp/.../scratchpad/muse-aarch64-macos
```

Extracted artifacts written alongside this report:

- `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/skills-assets/muse-core/` — the complete bundled skill package (21 files), byte-identical to what the binary materializes on disk
- `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/skills-assets/muse-core-plugin.json` — the `muse-core` `.muse-plugin/plugin.json` manifest (carved from rodata; not materialized on disk)
- `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/skills-assets/tbh-reminders-plugin.json` — the second bundled plugin, `tbh-reminders`, incl. the full `skill-reminder` agent declaration

---

## 0. TL;DR / architecture

Muse's skills system is a **superset of Claude Code's SKILL.md convention**, with a
hard-edged security model bolted on:

- Same on-disk shape: `<dir>/SKILL.md` with YAML frontmatter + Markdown body.
- Muse's *native* project root is `.agents/skills/` (not `.claude/skills/`), and its
  personal root is `$CONFIG_DIR/skills/` (`$XDG_CONFIG_HOME/muse` else `$HOME/.config/muse`).
- It **also reads Claude Code's and Codex's skill roots live**, both at workspace scope
  (`.claude/skills`, `.codex/skills`) and at personal scope (`~/.claude/skills`,
  `~/.codex/skills`), and tags those entries in the model catalog with
  `origin-family` / `written-for` provenance attributes so the model discounts
  harness-specific instructions.
- Project-scope skills are **gated on workspace trust** — untrusted workspaces load no
  project skills at all.
- A managed personal store (`muse skills install/import/update/uninstall`) keeps a
  lockfile with per-file SHA-256, a trust tag, a malware-scan slot, an append-only
  audit log, and an import quarantine.
- Skills reach the model as a **catalog only** (id + path + description) injected as a
  `developer`-role context block at session start; the body is loaded on demand by the
  `read_skill` tool.
- 15 first-party skills ship inside the binary as a bundled plugin named `muse-core`
  (14 visible by default, 1 gated behind `MUSE_EXPERIMENTAL_PLUGINS`).

Crates involved (from leaked panic paths + the symbol-name table at `0xe4053xx`+):

- `fbcode/musecode/build/src/crates/config/src/skills/` — `frontmatter.rs`, `loader.rs`,
  `id.rs` (`derive_directory_skill_id`), `plugin.rs` (`unknown_keys`),
  `plugin_activation.rs` (`load_codex_plugin_activation_default`), `captured.rs`,
  `diagnostic.rs` (the only path that leaked into panic metadata:
  `fbcode/musecode/build/src/crates/config/src/skills/diagnostic.rs`)
- a separate `tbh_skills` crate: modules `generation`, `bundled_source`, `invocation_model`,
  `activation_update`, `error`, `model::requests`, `store::{files,sources}`, `stores`,
  `security::package_files`, `selected`
- `tbh_agent::hooks::selected_skills::adapter`
- `tbh_tui::skills_runtime` (+ `generation_registry`, `scheduled_prompt_resolver`)
- `tbh_skill_reminder` (+ `tbh_skill_reminder::plugin`)

---

## 1. `muse skills` CLI surface (PROVEN)

`muse skills --help` prints every usage line at once (there is no clap subcommand tree;
per-subcommand `--help` prints only that one line):

```
$ muse skills --help
usage: muse skills list [--source all|user|project|built-in|plugin] [--enabled-only] [--workspace <path>] [--trust-workspace] [--json]

usage: muse skills inspect <skill-id-or-path> [--source all|user|project|built-in|plugin] [--workspace <path>] [--trust-workspace] [--json]

usage: muse skills enable <skill-id-or-path> --scope user|project|built-in|plugin [--workspace <path>] [--trust-workspace] [--json]

usage: muse skills user-only <skill-id-or-path> --scope user|project|built-in|plugin [--workspace <path>] [--trust-workspace] [--json]

usage: muse skills disable <skill-id-or-path> --scope user|project|built-in|plugin [--workspace <path>] [--trust-workspace] [--json]

usage: muse skills validate <path> [--json]

usage: muse skills install <path> [--scope user] [--name NAME] [--force] [--json]

usage: muse skills import --from claude|codex [--scope user] [--dry-run] [--force] [--json]

usage: muse skills update <skill-id> [--json]

usage: muse skills uninstall <skill-id> [--keep-files] [--json]
```

Note the undocumented-in-the-top-level-help `user-only` verb (third activation state)
and that `skills import --scope project is not supported in Phase 8`
(exact string at `0xb6de7f0`-ish, in the argv parser blob).

Errors are a small closed set (`missing skills command`, `invalid skills command`,
`missing skill selector`, `missing required --scope user|project|built-in|plugin`,
`missing --from claude|codex`, `missing skill package path`).

### `muse skills list`

```
$ muse skills list
NAME	SCOPE	ACTIVATION	DESCRIPTION	PATH
browser-app-delivery	built-in	on	Deliver a runnable local browser app …	built-in
…
```

`--json` shape (PROVEN, `muse skills list --source all --trust-workspace --json`):

```json
{
  "skills": [
    {
      "id": "bundled:browser-app-delivery",
      "name": "browser-app-delivery",
      "display_name": "browser-app-delivery",
      "description": "…",
      "short_description": null,
      "scope": "bundled",
      "source": { "type": "local" },
      "path": "bundled://muse-core/skills/browser-app-delivery/SKILL.md",
      "activation": "on",
      "diagnostics": [],
      "provenance": null,
      "context_cost": {
        "startup_bytes": 1884,
        "startup_estimated_tokens": 471,
        "invoke_bytes": null,
        "invoke_estimated_tokens": null
      }
    }
  ],
  "diagnostics": [
    { "code": "skill-shadowed",
      "message": "skill `hello` skipped because a higher-priority source defines the same id",
      "scope": "user",
      "path": "$CONFIG_DIR/skills/hello/SKILL.md" }
  ]
}
```

- `startup_estimated_tokens == ceil(startup_bytes / 4)` (PROVEN by inspection: 1884→471, 477→120, 793→199).
- `invoke_bytes` / `invoke_estimated_tokens` were **always `null`** in every CLI run,
  including for a 200 000-byte body. INFERRED: they are only populated by the TUI
  `/skills` drawer or after a `read_skill`.
- `--enabled-only` filters out `activation: "off"` but **keeps** `user-invocable-only`.

### Selector grammar (PROVEN)

`<skill-id-or-path>` accepts:

| form | example | resolves to |
|---|---|---|
| bare id | `grill` | project → user → bundled, in that precedence |
| qualified bundled | `bundled:plan` | always the built-in, even when a project `plan` exists |
| qualified plugin | `plugin:demo-plugin:plug-skill` | plugin skill (bare alias does **not** exist) |
| workspace-relative path | `.agents/skills/plan/SKILL.md` | that exact file |
| absolute path | `/abs/.../SKILL.md` | that exact file |

Proofs:

```
$ muse skills inspect plan --trust-workspace     # with .agents/skills/plan present
id: plan
scope: project
path: .agents/skills/plan/SKILL.md

$ muse skills inspect bundled:plan --trust-workspace
id: plan
scope: built-in
path: built-in

$ muse skills inspect plug-skill --trust-workspace   # plugin skill only
skill not found: plug-skill

$ muse skills inspect nope --json
{ "error": { "code": "unknown-skill", "message": "skill not found: nope", "details": {} } }
```

`--scope` on `enable/disable/user-only` is a **filter**, not just a label — the selector
must resolve inside that scope:

```
$ muse skills disable grill --scope project --trust-workspace --json
{ "error": { "code": "unknown-skill", "message": "skill not found: grill", "details": {} } }
$ muse skills disable grill --scope built-in --json
{ "selector": "grill", "scope": "bundled", "activation": "off",
  "settings_path": "$CONFIG_DIR/settings.json" }
```

CLI scope name `built-in` maps to settings scope `bundled`.

---

## 2. On-disk format of a skill

### 2.1 Directory layout (PROVEN)

A skill is a **directory** containing exactly one required file, `SKILL.md`, at its root.
Sibling files (scripts, references, assets) are optional and are carried through by
`install`/`import` verbatim, at arbitrary depth.

```
<skills-root>/<skill-id>/
  SKILL.md                # required
  scripts/…               # optional (bundled skills use scripts/*.py, *.sh)
  references/…            # optional (bundled create-plugin uses references/*.md,*.json)
```

The skill's id defaults to the **directory name** (`derive_directory_skill_id` in
`config/src/skills/id.rs`), and is overridden by the frontmatter `name` when present:

```
$ mkdir -p .agents/skills/name-mismatch
$ cat .agents/skills/name-mismatch/SKILL.md
---
name: totally-different
description: name does not match dir.
---
$ muse skills validate .agents/skills/name-mismatch --json | head -3
{ "valid": true, "id": "totally-different", … }
$ muse skills list --source project --trust-workspace
totally-different	project	on	name does not match dir.	.agents/skills/name-mismatch/SKILL.md
```

Symlink behaviour (PROVEN):

- A **symlinked skill directory** under `.agents/skills/` IS loaded and IS validatable.
- A **symlinked `SKILL.md`** inside a real directory is NOT loaded (binary string at
  `0xba3c...`: `direct SKILL.md symlinks are not local skills`).
- `muse skills install` refuses a symlinked source directory:
  `{"error":{"code":"invalid-skill-package","message":"No such file or directory (os error 2)"}}`.

Package-safety strings present in the binary (format strings, thresholds not reached
in testing at 3 001 entries / 40 levels of nesting — see §12):

```
skill package path escapes source root
duplicate normalized package path
skill package entry must resolve to a regular file
skill package contains unsafe relative path
skill package paths must be UTF-8
skill packages must not contain .muse files
foreign skill file name
skill package exceeds the maximum of <N> entries
skill package directory nesting exceeds the maximum depth of <N>
skill body path escapes skill root
SKILL.md must be a file
```

### 2.2 SKILL.md frontmatter

Parser rules (PROVEN by error injection):

| condition | error |
|---|---|
| file does not start with `---` | `SKILL.md must start with YAML frontmatter` |
| frontmatter never closed | `SKILL.md frontmatter must end with ---` |
| no `description:` key | `SKILL.md frontmatter must include description` |
| duplicate YAML key | `duplicate_key` diagnostic (string present, not exercised) |
| BOM present | `bom_forbidden` (string present, not exercised) |
| non-UTF-8 | `skill file at <p> is not valid UTF-8` |

```
$ printf -- '---\nname: no-desc\n---\n\nbody\n' > .agents/skills/no-desc/SKILL.md
$ muse skills validate .agents/skills/no-desc --json ; echo exit=$?
{
  "error": {
    "code": "invalid-skill-package",
    "message": "SKILL.md frontmatter must include description",
    "details": { "path": ".agents/skills/no-desc" }
  }
}
exit=1
```

**`description` is the only required key. `name` is optional** (falls back to the
directory name) — this differs from Claude Code, which requires `name`.

#### Recognized frontmatter keys

`muse skills validate` reports compatibility against a profile named
`agent-skills-common-subset`. Feeding it a kitchen-sink SKILL.md partitions keys into
`known_fields` / `unknown_fields` / `unsupported_fields` (PROVEN):

```
$ muse skills validate .agents/skills/kitchen-sink --json
{
  "valid": true,
  "id": "kitchen-sink",
  "source_path": ".agents/skills/kitchen-sink",
  "files": [ { "relative_path": "SKILL.md", "sha256": "sha256:31ad46b…", "bytes": 424 } ],
  "diagnostics": [
    { "code": "unsupported-skill-field", "severity": "warning",
      "message": "`allowed-tools` is recorded as advisory metadata but is not enforced and grants no tool permissions",
      "path": ".agents/skills/kitchen-sink/SKILL.md" }
  ],
  "compatibility": {
    "profile": "agent-skills-common-subset",
    "result": "compatible",
    "known_fields":  ["allowed-tools","argument-hint","description","disable-model-invocation",
                      "license","metadata","name","user-invocable"],
    "unknown_fields":["display-name","experimental-gate","model","short-description",
                      "totally-unknown-field","version"],
    "unsupported_fields": [],
    "allowed_tools": ["Bash","Read(*)"]
  }
}
```

So the **common-subset (Claude-compatible) key set** is exactly:

```
name  description  allowed-tools  argument-hint  disable-model-invocation
license  metadata  user-invocable
```

Claude Code keys Muse recognizes as **Claude-specific and explicitly unsupported**
(they land in `unsupported_fields` and raise a warning). Message format string at
`0xcbb6aea`: `` Claude field `<k>` is not implemented by <product> and is treated as metadata ``.
Confirmed members (PROVEN via `muse skills import --from claude --dry-run`):

```
agent      → "Claude field `agent` is not implemented by Muse Code and is treated as metadata"
context    → "Claude field `context` is not implemented by Muse Code and is treated as metadata"
```

(`model` landed in `unknown_fields`, not `unsupported_fields`, so the Claude-specific
list is narrow. The complete list is UNKNOWN beyond `agent` and `context`.)

#### Semantics of each key

| key | effect | proof |
|---|---|---|
| `name` | overrides the directory-derived id | §2.1 |
| `description` | **required**; goes into the model catalog verbatim | §2.2 |
| `metadata.short-description` | emitted as `<short-description>` in the catalog | bundled `import`/`read-session`; see §5 |
| `user-invocable: false` | hides the skill from the `/skill` picker; does **not** change CLI `activation` and does **not** remove it from the model catalog | §2.3 |
| `disable-model-invocation: true` | CLI `activation` becomes `user-invocable-only`; the skill is still listed in the catalog but tagged `model-invocable="false"` | §2.3, §5 |
| `allowed-tools` | parsed (string or list; `Bash(git status:*)` selectors preserved) and echoed as `compatibility.allowed_tools`, but **advisory only — grants nothing** | warning above |
| `argument-hint` | accepted; used by the TUI `/skill` shortcut (string `argument_hint` at `0xb...`) | bundled `import` uses `argument-hint: "<session-id-or-path>"` |
| `license`, `metadata` | accepted, carried as metadata | kitchen-sink |
| `experimental-gate: <gate>` | Muse-only. Skill is loaded only when that experimental gate is on; an unrecognized gate excludes the skill | §2.4 |

`allowed-tools` sub-validation strings (present, not all exercised):

```
`allowed-tools` must declare at least one selector
`allowed-tools` entries must not be empty
`allowed-tools` contains duplicate canonical selectors
```

### 2.3 `user-invocable` vs `disable-model-invocation` (PROVEN)

Four project skills, identical except for frontmatter:

```
$ muse skills list --source project --trust-workspace
NAME	SCOPE	ACTIVATION	DESCRIPTION	PATH
both	project	user-invocable-only	test skill both.	.agents/skills/both/SKILL.md
dmi-true	project	user-invocable-only	test skill dmi-true.	.agents/skills/dmi-true/SKILL.md
hello	project	on	A test skill that says hello.	.agents/skills/hello/SKILL.md
ui-false	project	on	test skill ui-false.	.agents/skills/ui-false/SKILL.md
```

- `disable-model-invocation: true` ⇒ CLI activation `user-invocable-only`.
- `user-invocable: false` ⇒ CLI activation stays `on` (it only removes the skill from the
  human-facing `/skill` picker). 7 of the 14 default built-ins use it.

**Important asymmetry (PROVEN, and probably a bug):** `user-invocable-only` reached via
*frontmatter* keeps the skill in the model catalog with `model-invocable="false"`, while
`user-invocable-only` reached via *settings activation* removes it from the catalog
entirely.

```
# gamma has `disable-model-invocation: true` in frontmatter
# beta was set with: muse skills user-only beta --scope project
<skill id="delta" scope="project" path=".agents/skills/delta/SKILL.md">
<skill id="gamma" scope="project" path=".agents/skills/gamma/SKILL.md" model-invocable="false">
# alpha (activation off) and beta (activation user-invocable-only) are absent
```

### 2.4 `experimental-gate` (PROVEN)

```
$ cat .agents/skills/bogus-gate/SKILL.md
---
name: bogus-gate
description: A skill with a bogus gate.
experimental-gate: no-such-gate
---
$ muse skills list --source project --trust-workspace
…
Diagnostics
- skill-unknown-experimental-gate: skill file at …/bogus-gate/SKILL.md names unknown
  experimental-gate `no-such-gate`; the skill is excluded (.agents/skills/bogus-gate/SKILL.md)
```

With a *valid* gate the skill is hidden until the gate is on:

```
$ muse skills list --source project --trust-workspace --json          # gate off
hello
$ MUSE_EXPERIMENTAL_PLUGINS=1 muse skills list --source project --trust-workspace --json
hello, kitchen-sink        # kitchen-sink had `experimental-gate: plugins`
```

The gate vocabulary is the snake_case list embedded at `0x...` next to the
`MUSE_EXPERIMENTAL_*` env-var table (exact string run, split for readability):

```
workflow_tool artifact_tool local_session_messaging external_agent_ingress code_mode
prefix_compaction monitor foreign_personal_context_kill voice voice_default_on
voice_native_capture reasoning_display model_effort_context bash_titles
bash_sandbox_escalation git_sandbox_relaxation plugins enterprise_config web_fetch
server_web_fetch curl_web_fetch web_fetch_preflight_hard_cap first_turn_minimal_effort
todo_reminder memory_reminder skill_reminder goal_reminder verify_reminder scope_reminder
session_runtime session_recovery_shadow tui_msp_client sdk_enabled
meta_context_workspace_only non_strict_tool_params hook_selected_skills
hook_selected_skills_apply workflow_api_v2_rollout subscription_launch
provider_tool_switch tag
```

### 2.5 Size caps (PROVEN)

The loader hard cap on `SKILL.md` is **262 144 bytes (256 KiB)**:

```
$ muse skills list --source project --trust-workspace
No skills found.

Diagnostics
- skill-file-too-large: skill file at …/big/SKILL.md is too large:
  262145 bytes exceeds 262144 bytes (.agents/skills/big/SKILL.md)
```

(Note the reported "actual" size is clamped to `limit+1` — Muse stops reading at the cap.)

Format strings: `SKILL.md exceeds <N>`, `skill file at <p> is too large: <a> bytes exceeds <b> bytes`.

A separate, smaller **read guard** exists for `read_skill` (`Oversized skill body;
read_skill returns a truncated prefix.` and `warning: skill file exceeded the <N> byte
read guard; returned a truncated prefix`). Its numeric value is **UNKNOWN** — it did not
trigger for a 200 000-byte body at catalog-render time, and could not be observed
without an actual model turn.

---

## 3. Discovery roots and precedence

### 3.1 The roots (PROVEN)

Four scopes: `bundled` (a.k.a. `built-in`), `user`, `project`, `plugin`.

**bundled** — one embedded plugin package, `muse-core`. Materialized on first use to:

```
$XDG_DATA_HOME/muse/skills/bundled/muse-core/        (default $HOME/.local/share/muse/…)
```

Display locator in the catalog is `bundled://muse-core/skills/<id>/SKILL.md`; ids are
namespaced `bundled:<id>`.

**user** — four roots, highest precedence first (PROVEN by additive collision test, §3.2):

1. `$CONFIG_DIR/skills/` — the *managed* personal root
   (`$XDG_CONFIG_HOME/muse` else `$HOME/.config/muse`); displayed as `$CONFIG_DIR/skills/…`
2. `$HOME/.agents/skills/` — displayed as `$HOME/.agents/skills/…`
3. `$HOME/.claude/skills/` — foreign (Claude Code)
4. `$HOME/.codex/skills/` (and `$CODEX_HOME/skills` — literal
   `$HOME/.claude/skills$HOME/.codex/skills$CODEX_HOME/skills` at `0xbb9cd8b`)

**project** — three roots under the workspace root, highest precedence first:

1. `<ws>/.agents/skills/`
2. `<ws>/.codex/skills/`
3. `<ws>/.claude/skills/`

Not discovered (PROVEN negative): `<ws>/.muse/skills/`, `<ws>/skills/`.

**plugin** — installed plugin packages that declare a `skills` capability; ids are
`plugin:<plugin-id>:<skill-id>`, locators `plugin://<plugin-id>/<path>`.

Not a discovery root but relevant: `.agents/skill-drafts/<id>/` is the sanctioned
*staging* directory for a personal-scope draft, deliberately outside `.agents/skills/`
so the draft is not loaded as a project skill (from the bundled `create-skill` body).

### 3.2 Precedence proof — user scope

```
$ mk(){ mkdir -p "$1/dup"; printf -- '---\nname: dup\ndescription: dup from %s.\n---\nbody\n' "$1" > "$1/dup/SKILL.md"; }
$ w(){ muse skills list --source user --json | jq -r '.skills[]|select(.id=="dup")|.path'; }

codex only:  $HOME/.codex/skills/dup/SKILL.md
+claude:     $HOME/.claude/skills/dup/SKILL.md
+agents:     $HOME/.agents/skills/dup/SKILL.md
+config:     $CONFIG_DIR/skills/dup/SKILL.md
```

Losers are reported as `skill-shadowed`.

### 3.3 Precedence proof — project scope

```
claude only:   .claude/skills/pdup/SKILL.md
claude+codex:  .codex/skills/pdup/SKILL.md
+agents:       .agents/skills/pdup/SKILL.md
codex only:    .codex/skills/pdup/SKILL.md
```

Note the **inversion vs. user scope**: at project scope `.codex` outranks `.claude`,
at user scope `.claude` outranks `.codex`. Reproducible in both directions.

### 3.4 Cross-scope precedence (PROVEN)

`project > user`:

```
$ muse skills list --source all --trust-workspace
hello	project	on	A test skill that says hello.	.agents/skills/hello/SKILL.md
…
Diagnostics
- skill-shadowed: skill `hello` skipped because a higher-priority source defines the same id ($CONFIG_DIR/skills/hello/SKILL.md)
```

`bundled` and `plugin` **never collide** with project/user, because their ids are
namespaced (`bundled:plan` vs `plan`). A project skill named `plan` coexists with
`bundled:plan`; the bare selector `plan` then resolves to the project one, i.e. the
project skill "shadows the alias" (diagnostic codes `skill-alias-shadowed` /
`skill-alias-unsafe` exist for this, though neither fired in the CLI).

### 3.5 Duplicate ids inside one scope (PROVEN)

Two directories in the same root whose frontmatter `name` collides:

```
$ muse skills list --source project --trust-workspace
samename	project	on	from aaa.	.agents/skills/aaa/SKILL.md
samename	project	on	from bbb.	.agents/skills/bbb/SKILL.md
```

The CLI lists both, but the **session catalog drops both** and emits:

```xml
<diagnostic code="duplicate-active-skill-id" path="samename">duplicate active skill id
`samename` from .agents/skills/aaa/SKILL.md, .agents/skills/bbb/SKILL.md;
disable one entry or select by path</diagnostic>
```

### 3.6 Turning off foreign roots (PROVEN)

`settings.json`:

```json
{ "schema_version": 1, "context": { "foreign_personal_skills": false } }
```

removes `~/.claude/skills` and `~/.codex/skills` from user-scope discovery, while
`$HOME/.agents/skills` and `$CONFIG_DIR/skills` remain:

```
$ muse skills list --source user
dup	user	on	…	$CONFIG_DIR/skills/dup/SKILL.md
ha-skill	user	on	A home .agents skill.	$HOME/.agents/skills/ha-skill/SKILL.md
hello	user	off	…	$CONFIG_DIR/skills/hello/SKILL.md
taste	user	on	…	$CONFIG_DIR/skills/taste/SKILL.md
# cc-skill and cx-skill are gone
```

There is a sibling `context.foreign_personal_rules`, and a kill-switch gate
`MUSE_EXPERIMENTAL_FOREIGN_PERSONAL_CONTEXT_KILL`.

---

## 4. Workspace trust gating (PROVEN)

Project-scope skills load only from a **trusted** workspace.

```
$ muse skills list --source project
No skills found.

Diagnostics
- project-skills-untrusted: project skills skipped because workspace is untrusted (.agents/skills)

$ muse skills list --source project --trust-workspace
NAME	SCOPE	ACTIVATION	DESCRIPTION	PATH
hello	project	on	A test skill that says hello.	.agents/skills/hello/SKILL.md
```

- Diagnostic code `project-skills-untrusted`; message format
  `project skills skipped because workspace is untrusted: <root>`.
- `--trust-workspace` grants trust for the run only ("does not save trust", per
  `muse --help`). `--yolo` implies it plus the approval/sandbox bypasses.
- Persisted trust lives in `$CONFIG_DIR/trust.json` (named in the bundled `doctor`
  skill body: `` Local files: `settings.json`, `auth.json`, and `trust.json` ``; error
  strings `malformed trust store at`, `unsupported trust store schema version`,
  `project root cannot be stored in trust store at`). Symbols `load_trust_store`,
  `write_trust_store`, `resolve_run_flag_trust_gate_from_held_root`.
- A run reports its trust decision on stderr:
  `muse: workspace trust: trusted source=run-flag`
- The session's `security_mode` developer block states it to the model:
  `Workspace trust: trusted — project-local instructions, skills, and hooks are eligible to load.`
- Related loader reason codes: `untrusted_project`, `nearest_trusted_project`,
  `nearer_project`, `not_trusted_enabled`, `safe_mode`.

User-scope, bundled, and plugin skills are **not** trust-gated (they load with no flag).

---

## 5. How skills reach the model

### 5.1 The `skills_catalog` context block (PROVEN end-to-end)

Recovered by running `muse exec --provider echo --trust-workspace "hi"` and reading the
`model_request_configured` event out of `~/.local/share/muse/sessions/<Y>/<M>/<D>/<uuid>/session.jsonl`.

Block metadata:

```json
{ "id": "skills_catalog", "role": "developer", "source": "skills",
  "lifecycle": "session_start", "order": 200 }
```

and its diagnostic record:

```
context block `skills_catalog` from source `skills` matched catalog source `skills_catalog`
lane=context_block lifecycle=session_start order=200 cache_class=stable_prefix status=supported
```

Verbatim rendered text (real capture, abridged in the middle):

```xml
<system-reminder source="skills">
Muse Code loaded available skills at session open. These are summaries only. To load one skill's full instructions, call the read_skill tool with the skill's id or path from this catalog — do not read the path with read_file. A plugin:// or bundled:// path is a display locator, not a file read_file can open.

Some listed skills carry provenance attributes: written-for means the skill was discovered in another agent harness's directory (origin-family) and was written for that harness, not this agent — weigh its applicability instead of following harness-specific instructions literally.

<skill-catalog>
<skill id="bundled:import" scope="bundled" path="bundled://muse-core/skills/import/SKILL.md">
<description>Resume a third-party coding-agent session from a local transcript, path, or session id.</description>
<short-description>Import a Claude Code, Codex, or Grok session</short-description>
</skill>
<skill id="hello" scope="project" path=".agents/skills/hello/SKILL.md">
<description>A test skill that says hello.</description>
</skill>
<skill id="pdup" scope="project" path=".codex/skills/pdup/SKILL.md" origin-family=".codex" written-for="Codex">
<description>pdup from .codex/skills.</description>
</skill>
<skill id="proj-cc" scope="project" path=".claude/skills/proj-cc/SKILL.md" origin-family=".claude" written-for="Claude Code">
<description>workspace claude skill.</description>
</skill>
<skill id="gamma" scope="project" path=".agents/skills/gamma/SKILL.md" model-invocable="false">
<description>Gamma skill, model invocation disabled.</description>
</skill>
</skill-catalog>
<skill-diagnostics>
<diagnostic code="skill-shadowed" path="$CONFIG_DIR/skills/hello/SKILL.md" scope="user">skill `hello` skipped because a higher-priority source defines the same id</diagnostic>
<diagnostic code="duplicate-active-skill-id" path="samename">duplicate active skill id `samename` from .agents/skills/aaa/SKILL.md, .agents/skills/bbb/SKILL.md; disable one entry or select by path</diagnostic>
</skill-diagnostics>
</system-reminder>
```

Observations:

- Entries are sorted by id (`bundled:` sorts among the bare ids).
- `&apos;` XML-escaping is applied to descriptions.
- Paths are **display locators** with `$HOME` / `$CONFIG_DIR` abbreviated; the reminder
  explicitly tells the model not to `read_file` a `bundled://` or `plugin://` path.
- `origin-family` ∈ {`.claude`, `.codex`}; `written-for` ∈ {`Claude Code`, `Codex`}
  (both literals live at `0xba895..`).
- Attribute set observed: `id`, `scope`, `path`, `origin-family`, `written-for`,
  `model-invocable="false"`. When the byte budget is exceeded, entries collapse to
  self-closing `<skill … />` with no `<description>` at all.

### 5.2 Catalog byte budget (PROVEN)

Growing a single description and re-running the echo session:

| description bytes | rendered block bytes | descriptions present |
|---|---|---|
| 500 | 10 620 | yes |
| 20 000 | 30 120 | yes |
| 24 000 | 31 628 | yes (trimmed) |
| 30 000 | 31 969 | yes (trimmed) |
| 40 000 | 1 933 | **no — all entries collapse to `<skill … />`** |

The block is trimmed to stay under **32 768 bytes (32 KiB)**; beyond a point the renderer
falls back to an id-only catalog. Diagnostic code `skill-catalog-entry-too-large` exists
for the per-entry case.

### 5.3 Description slimming (PROVEN)

`settings.json`:

```json
{ "run": { "context_slimming": {
    "skill_catalog_descriptions": "first_sentence",
    "full_skill_description_ids": ["bundled:taste"] } } }
```

`SkillCatalogDescriptionMode` is `full | first_sentence`. In `first_sentence` mode every
`<description>` is truncated to its first sentence, except ids listed in
`full_skill_description_ids`:

```xml
<skill id="bundled:plan" …><description>Create a grounded, decision-complete plan, then stop for approval.</description></skill>
<skill id="bundled:taste" …><description>Mandatory preflight for web frontend visual design. If available, call read_skill for bundled:taste before … still win.</description></skill>
```

Sibling settings in the same struct (`ContextSlimmingSettings` / `ContextSlimmingDefaultsV1`):
`meta_context_note_enabled`, `session_identity_enabled`, `excluded_tool_names`.

### 5.4 `read_skill` tool

Registered in the default toolset (PROVEN, from `model_request_configured.toolset.active_tools`):

```
workflow, read_file, search, write_file, edit_file, read_memory, add_memory, edit_memory,
web_search, bash, bash_input, cron_create, cron_delete, cron_list, get_goal, create_goal,
update_goal, report_progress, subagent_spawn, subagent_status, subagent_send_message,
subagent_wait, subagent_read_result, read_skill, write_todos
```

Argument schema (from exact validation strings at `0xb808fd8`): a JSON object with a
**single string field `name`**:

```
invalid_arguments: expected JSON object with string field `name`
invalid_arguments: expected only string field `name`
invalid_arguments: expected non-empty string field `name`
```

Result envelope (reassembled from the adjacent format-string fragments — the individual
fragments are exact; their assembly is INFERRED):

```xml
<read-skill-result name="<id>" status="ok">
<metadata>
locator: file://<abs path>
(written for <family>; weigh applicability)
bytes_total: <n>
bytes_returned: <n>
truncated: <bool>
warning: skill file exceeded the <N> byte read guard; returned a truncated prefix
hint: this read_skill result is truncated; call read_file with path "<p>" if you need more of the truncated content
</metadata>

<system-reminder source="skill-body">
Muse Code loaded the full instructions for an explicitly invoked skill. Apply these instructions only to the current user turn.
<skill-body id="<id>" scope="<scope>" path="<p>" sha256="<h>" byte-count="<n>" returned-byte-count="<n>" truncated="true">
…body…
</skill-body>
</system-reminder>
</read-skill-result>
```

Error form (exact):

```xml
<read-skill-result name="" status="error">
<metadata>
message: <msg>
</metadata>
</read-skill-result>
```

Truncated variant lead sentence (exact):

> Muse Code loaded a truncated prefix for an explicitly invoked skill because the SKILL.md file exceeded the read guard. Apply only the returned instructions to the current user turn.

Selected-skill path-safety checks (exact strings, all in the read path):

```
selected skill descendant is not a regular non-link directory
selected skill opened descendant is unsafe
selected skill path is empty
selected skill final handle is not a regular non-link file
selected skill final handle path proof is unavailable
selected skill aliases a reserved base skill
selected skill held root identity changed
selected skill trusted project root is unavailable
selected skill containment proof is unavailable
selected skill final path is not a regular non-link file
selected skill held root identity unavailable: <e>
selected skill final metadata unavailable: <e>
selected skill final identity unavailable: <e>
selected skill read failed: <e>
```

### 5.5 Agent-definition preloaded skills (PROVEN by string; runtime not exercised)

An agent definition (`--agents <JSON>`, `.agents/agents/*`, or a plugin `agents`
capability) carries a `skills` array — the agent-definition field list is the exact
string run:

```
name description prompt tools disallowed_tools model effort permission_mode max_turns
background isolation skills mcp_servers hooks memory color initial_prompt
main_session_selection
```

Those skills' **bodies** are preloaded into the child run:

```xml
<agent-definition-skills definition="<name>">
These skill instructions were preloaded by the selected Agent Definition. Apply them throughout this child run.
<skill id="…">
…
</skill>
</agent-definition-skills>
```

Diagnostic `agent-skill-skipped` covers rejected entries. Prompt-cache key variants
`agent-definition-with-skills-v…` and `agent-definition-with-skills-and-execution-request-v…`
confirm skills participate in the child's cached prefix.

### 5.6 `MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS` / `_APPLY`

Two gates (`hook_selected_skills`, `hook_selected_skills_apply`). A hook returns, inside
`hookSpecificOutput`, a `selectedSkills` array. Validation strings (exact, at `0xbc10bae`):

```
selectedSkills must be an array
selectedSkills entries must be objects
selectedSkills entries must contain exactly id, path, and description
```

The host then renders a **second, separate** context block (exact strings at `0xba895a1`):

```xml
<system-reminder source="selected-skills">
<product> loaded authenticated Project-scope skills selected for this run. These are summaries only; use read_skill with the exact id or absolute path.

<skill-catalog source="selected_skills_catalog">
<skill id="<id>" scope="project" path="<abs path>" hook-source="<src>" hook-source-family="<fam>" hook-plugin-id="<pid>" hook-source-digest="<sha>" hook-configured-order="<n>">
<description>…</description>
</skill>
</skill-catalog>
</system-reminder>
```

The `hook-source-digest` attribute is why the reminder says *authenticated*: each entry
carries the digest of the hook source that produced it. Symbols:
`tbh_agent::hooks::selected_skills::adapter`, `with_selected_skills_source`,
`selected_skills::declares_skills`, `selected_skills::provenance`,
`discard_superseded_skill_catalog_updates`, `selected_skills_generation`.

**Not runtime-verified**: I could not get any hook to fire in this sandbox. `.muse/hooks.json`
and `$CONFIG_DIR/hooks.json` containing deliberate garbage produced no error, so hook loading
is evidently gated on something else (settings `managed_hooks_path` / `runtime_capabilities`
trust, or a plugin-provided hook). The `selectedSkills` contract itself is PROVEN by the
exact validation strings and reminder template; the wiring is INFERRED.

### 5.7 `MUSE_EXPERIMENTAL_SKILL_REMINDER` — the skill-reminder agent

Gate `skill_reminder`. It enables a first-party **reminder agent** shipped as a second
bundled plugin, `tbh-reminders` (manifest carved from `0xb714527`, full JSON saved to
`re/skills-assets/tbh-reminders-plugin.json`). Its six reminders are
`memory`, `skill-reminder`, `todo-reminder`, `goal-reminder`, `verify-reminder`,
`scope-reminder` — matching the `*_REMINDER` env gates.

The `skill-reminder` declaration (verbatim excerpts):

```json
{
  "id": "skill-reminder",
  "path": "reminders/skill-reminder.md",
  "enabledDefault": true,
  "tools": [],
  "blocking": false,
  "defaultPriority": "normal",
  "maxPriority": "high",
  "maxChildSteps": 1000000,
  "reasoningEffort": "low",
  "context": {
    "conversation": { "mode": "bounded", "maxTokens": 4000 },
    "feeds": [
      { "name": "skill_catalog",     "kind": "host", "source": "skill_catalog",     "maxBytes": 128000, "refresh": "run_start" },
      { "name": "skill_read_ledger", "kind": "host", "source": "skill_read_ledger", "maxBytes": 24000,  "refresh": "boundary" }
    ]
  },
  "decision": {
    "deliveryRole": "developer",
    "envelope": { "version": 1,
      "template": "<system-reminder>\n{text}\n\nTreat this reminder as internal guidance. Act on it directly without\nacknowledging, quoting, paraphrasing, or referring to the reminder.\n</system-reminder>" },
    "bodyTemplate": { "maxBytes": 65536, "text": "{advisory_text}",
      "slots": [ { "escape": "xml", "name": "advisory_text", "source": { "validator": "skill.skill_decision" } } ] },
    "fields": [
      { "name": "skill_id",          "requirement": "optional", "shape": { "type": "string", "minBytes": 1, "maxBytes": 256 } },
      { "name": "reason",            "requirement": "always",   "shape": { "type": "string", "minBytes": 1, "maxBytes": 2000 } },
      { "name": "advisory_text",     "requirement": "optional", "shape": { "type": "string", "minBytes": 1, "maxBytes": 2000 } },
      { "name": "confidence",        "requirement": "optional", "shape": { "type": "string", "enum": ["low","medium","high"] } },
      { "name": "priority",          "requirement": "optional", "shape": { "type": "string", "enum": ["low","normal","high"] } },
      { "name": "visible_for_steps", "requirement": "optional", "shape": { "type": "integer", "minimum": 1, "maximum": 8 } }
    ],
    "validators": [ { "id": "skill_read_ledger", "name": "skill",
        "inputs": { "skill_id": {"field":"skill_id"}, "reason": {"field":"reason"},
                    "advisory_text": {"field":"advisory_text"}, "confidence": {"field":"confidence"} },
        "outputs": { "normalized_skill": "skill", "rejection": "optionalString",
                     "skill_decision": "skillDecisionEvidence" }, "version": 1 } ],
    "proposal": { "…": "branches: remind / none / invalidPayload / roundEnded / syntheticFailure",
      "validatorRejections": ["duplicate_skill_task","invalid_skill","missing_skill_id","skill_already_read"] },
    "lifecycle": { "seenKey": "ordinary", "eotProgressGate": "notApplicable",
                   "failureFallback": "none", "installBudget": "roster" }
  }
}
```

`reminders/skill-reminder.md` (the whole duty file, exact — one line):

```
Decide whether the main agent should be reminded to load a relevant skill.
```

The host builds the reminder agent's prompt from these exact fragments (`0xbc10002`,
`0xbc1f99d`):

```xml
<available-skills>
<skill id="<id>" scope="<s>" origin-family="<f>" written-for="<w>" model-invocable="false"/>
<description>…</description>
<short-description>…</short-description>
</skill>
</available-skills>
  <truncated source="skill_catalog" />

<skill-reminder-conversation-anchor>
…
</skill-reminder-conversation-anchor>
<skill-reminder-recent-tail>
…
</skill-reminder-recent-tail>

<skill-reminder-state>
<already-read-skills><skill-ref id="…"/></already-read-skills>
  <truncated source="skill_read_ledger" />
</skill-reminder-state>

Decide whether one skill reminder is needed now.
```

Supporting event payloads in the session log schema:

```
SkillReadObservedPayload  { skill_id, observed_at_sequence, evidence_kind, evidence_hash }
SkillReadCheckpointState  { observed_skill_ids }
SkillReminderDecisionPayload { decision_id, reminder_agent_id, needs_reminder, skill_id,
                               reason, foreground_task_key, advisory_text, confidence }
```

So the ledger is durable and survives compaction/resume: the reminder agent will not
re-nag about a skill the main agent already read (`skill_already_read` rejection).

I could not observe a live reminder because reminder agents need a real model and this
build's `--provider echo` never invokes them.

### 5.8 TUI surfaces (PROVEN by exact strings)

```
usage: /skills [import|reload|manage|diagnostics|use <id-or-path> [prompt]]
usage: /skill <id-or-path> [prompt]
usage: /skills use <id-or-path> [prompt]
Skills
enter use - space toggle - /skills diagnostics - esc close - type filter
Skills diagnostics
esc close - type filter
Run `muse skills list --source all` for full diagnostics.
```

Slash-command roster (exact run): `…/rules /compact /export /copy /recap /skills /plugins
/skill /effort /goal …`.

A `/skill` invocation becomes a **skill chip** in the composer, backed by a typed binding
recorded in the session log (`UserIntentBindingV1::skill`):

```
refill_block_index, utf8_start, utf8_end, scope, skill_id, body_sha256
UserIntentSkillScopeV1 = user | project | bundled | plugin
```

i.e. the exact body the user selected is pinned by hash. Related symbols:
`skill_chip`, `reanchor_unique_skill_chips`, `replace_visible_range_with_skill_chip`,
`parse_slash_skill_invocation`, `decode_scheduled_skill_shortcut`,
`skill_shortcut_name_claim`, `load_shadowed_skill_slash_shortcuts`,
`bundled_skill_search_terms`, `user_skill_display_priority`, `project_skill_display_priority`.
Failure strings: `selected skill is unavailable; choose another source`,
`inconsistent skill chips; press Ctrl+U to clear`,
`skill-user-invocation-marker-invalid`.

---

## 6. The enable/disable state store (PROVEN)

All activation state lives in **`$CONFIG_DIR/settings.json`** under `skills.activation`
— there is no per-workspace state file.

```
$ muse skills disable hello --scope user --json
{ "selector": "hello", "scope": "user", "activation": "off",
  "settings_path": "$CONFIG_DIR/settings.json" }
$ muse skills user-only bundled:git --scope built-in --json
{ "selector": "bundled:git", "scope": "bundled", "activation": "user-invocable-only",
  "settings_path": "$CONFIG_DIR/settings.json" }
$ muse skills disable alpha --scope project --trust-workspace --json
{ "selector": "alpha", "scope": "project", "activation": "off", … }
$ muse skills disable plugin:demo-plugin:plug-skill --scope plugin --json
{ "selector": "plugin:demo-plugin:plug-skill", "scope": "plugin", "activation": "off", … }
```

Resulting file (verbatim, merged from the four runs above):

```json
{
  "schema_version": 1,
  "skills": {
    "activation": {
      "user": {
        "$CONFIG_DIR/skills/hello/SKILL.md": "off"
      },
      "bundled": {
        "bundled://muse-core/skills/git/SKILL.md": "user-invocable-only"
      },
      "plugin": {
        "plugin://demo-plugin/skills/plug-skill/SKILL.md": "off"
      },
      "projects": {
        "/abs/path/to/workspace": {
          ".agents/skills/alpha/SKILL.md": "off",
          ".agents/skills/beta/SKILL.md": "user-invocable-only"
        }
      }
    }
  }
}
```

Key facts:

- Four sub-maps: `user`, `bundled`, `plugin` are **flat** maps keyed by the *display
  locator*; `projects` (note: plural) is **nested**, keyed by absolute workspace root,
  then by workspace-relative `SKILL.md` path.
- Values are the tri-state `"on" | "user-invocable-only" | "off"` (Rust enum literal run:
  `onuser-invocable-onlyoff`).
- The key is the file path, not the id — so renaming a skill silently resets its state.
- Settings write failures are hard-guarded:
  `settings could not be read, so they must not be overwritten; fix the settings file or the config directory`.
- Symbols: `user_skill_activation_override`, `project_skill_activation_override`,
  `bundled_skill_activation_override`, `plugin_skill_activation_override`,
  `clear_user_skill_activation`, `prune_plugin_skill_activation_overrides`,
  `sealed_with_skill_activation_patch`, `skill_activation_patched_read`.
- Effect on the catalog (PROVEN, §2.3): `off` ⇒ excluded; `user-invocable-only` ⇒ excluded
  from the model catalog (but still `/skill`-invocable).

---

## 7. The managed skills store and lockfile (PROVEN)

`muse skills install|import|update|uninstall` all operate on the managed personal root
`$CONFIG_DIR/skills/`. Its bookkeeping lives in `$CONFIG_DIR/skills/.muse/`:

```
$CONFIG_DIR/skills/
  <skill-id>/…                       # the installed package
  .muse/
    lock.json                        # the lockfile
    audit.log                        # append-only JSONL
    .skills.lock                     # 0-byte advisory file lock
    quarantine/                      # (created empty)
    import-quarantine/<id>/          # rejected imports land here
```

### 7.1 `lock.json` — verbatim

```json
{
  "version": 1,
  "skills": {
    "hello": {
      "id": "hello",
      "source": { "type": "local", "path": "/abs/src/.agents/skills/hello" },
      "version": null,
      "revision": null,
      "installed_at": "2026-09-01T14:28:11.310116Z",
      "updated_at": null,
      "content_sha256": "sha256:2082cf7524baaedfe44f1692a4ed625f28926f89b23b0e9c20b354c479dcf766",
      "trust": "local",
      "scan": { "status": "passed", "warnings": [] },
      "files": [
        { "relative_path": "SKILL.md",
          "sha256": "sha256:072dcf8d44f35884a456f2fd827ec196242716f23859d50d108c19cf70d118c4",
          "bytes": 84 }
      ]
    },
    "cc-one": {
      "id": "cc-one",
      "source": { "type": "imported",
                  "path": "/abs/home/.claude/skills/cc-one",
                  "ecosystem": "claude",
                  "source_path": "/abs/home/.claude/skills/cc-one" },
      "version": null, "revision": null,
      "installed_at": "2026-09-01T14:36:54.781378Z", "updated_at": null,
      "content_sha256": "sha256:aff9e472…",
      "trust": "imported",
      "scan": { "status": "passed", "warnings": [] },
      "files": [
        { "relative_path": "SKILL.md",       "sha256": "sha256:2b901db1…", "bytes": 173 },
        { "relative_path": "scripts/run.sh", "sha256": "sha256:ab08508f…", "bytes": 8 }
      ]
    }
  }
}
```

Rust struct names appear in the serde metadata: `Lockfile`, `LockSkillRecord`,
`LockSource`, `LockScan`, `LockFileRecord`.

The `LockSource` enum is internally tagged on `type` with variants and fields:

```
local            { path }
imported         { path, ecosystem, source_path }
marketplace      { marketplace, ecosystem, source_path, skill, index, package_hash }
marketplace-static | marketplace-git
                 { repository, requested_ref, resolved_revision, sparse_path,
                   manifest_path, manifest_hash }
```

(field-name run at `0x...`: `typepathmarketplaceecosystemsource_pathskillindexpackage_hash
repositoryrequested_refresolved_revisionsparse_pathmanifest_pathmanifest_hash`
`marketplace-staticmarketplace-gitlocal`)

`trust` values seen: `local`, `imported`. Plugin-side trust vocabulary (adjacent enum):
`user-local`, `project-trusted`, `curated`, `marketplace-user-added`, `foreign-import`.

Lockfile guards: `lockfile must be a regular non-symlink file`,
`lockfile exceeds <N> bytes`, `malformed skills lockfile at <p>: <e>`,
`unsupported skills lockfile version at <p>: expected version <a>, found <b>`.

### 7.2 `audit.log`

Newline-delimited JSON, one record per store mutation, exact template
(format string `{"time":"<t>","action":"<a>","skill":"<id>","source":"local","result":"ok"}\n`):

```json
{"time":"2026-09-01T14:36:54.790155Z","action":"install","skill":"cc-one","source":"local","result":"ok"}
{"time":"2026-09-01T14:36:54.812063Z","action":"install","skill":"cx-one","source":"local","result":"ok"}
```

Action vocabulary from the adjacent literal run: `update`, `install`, `uninstall`.

### 7.3 install / update / uninstall outputs (verbatim)

```json
// muse skills install .agents/skills/hello --scope user --json
{ "installed": { "id": "hello", "path": "$CONFIG_DIR/skills/hello",
                 "version": null, "revision": null,
                 "provenance": { "source": { "type": "local" },
                                 "content_sha256": "sha256:2082cf75…",
                                 "files": [ { "relative_path": "SKILL.md",
                                              "sha256": "sha256:072dcf8d…", "bytes": 84 } ] } },
  "lockfile_path": "$CONFIG_DIR/skills/.muse/lock.json",
  "diagnostics": [] }

// muse skills update badfm --json
{ "updated": { "id": "badfm", "path": "$CONFIG_DIR/skills/badfm",
               "previous_content_sha256": "sha256:2f24352b…",
               "content_sha256": "sha256:97f699ee…" },
  "lockfile_path": "$CONFIG_DIR/skills/.muse/lock.json", "diagnostics": [] }

// muse skills uninstall withreq --json
{ "uninstalled": { "id": "withreq", "removed_files": ["SKILL.md"], "kept_files": [] },
  "lockfile_path": "$CONFIG_DIR/skills/.muse/lock.json" }

// muse skills uninstall badfm --keep-files --json
{ "uninstalled": { "id": "badfm", "removed_files": [], "kept_files": ["SKILL.md"] },
  "lockfile_path": "$CONFIG_DIR/skills/.muse/lock.json" }
```

`update` re-reads the **recorded source path** from the lockfile (no argument beyond the
id) — proven by editing `~/.claude/skills/badfm/SKILL.md` and running
`muse skills update badfm`, which picked up the new content hash.

Store error codes: `skill-already-installed`, `skill-not-installed`,
`install-would-overwrite-unmanaged-files`, `install-path-unsafe`,
`update-source-unavailable`, `provenance-missing`, `skills-store-error`,
`failed to read/write skills store at <p>: <e>`.
Temp/rename scheme: `.<x>.<y>.tmp`, `<id>-uninstall-<n>`, `.tmp-<n>`.

---

## 8. `muse skills import --from claude|codex` (PROVEN)

Scans the foreign personal root and copies portable skills into the managed root.

```
$ muse skills import --from claude --dry-run --json
{
  "source": { "type": "claude", "path": "/…/home/.claude/skills" },
  "dry_run": true,
  "candidates": [
    { "id": "cc-one",
      "source_path": "/…/.claude/skills/cc-one/SKILL.md",
      "target_path": "/…/.config/muse/skills/cc-one/SKILL.md",
      "action": "copy",
      "valid": true,
      "classification": "portable",
      "compatibility": { "profile": "agent-skills-common-subset", "result": "compatible",
                         "known_fields": ["allowed-tools","description","license","metadata","name"],
                         "unknown_fields": [], "unsupported_fields": [],
                         "allowed_tools": ["Bash(git status:*)","Read"] },
      "unavailable_binaries": [],
      "missing_requires": [],
      "diagnostics": [ { "code": "unsupported-skill-field", "severity": "warning",
                         "message": "`allowed-tools` is recorded as advisory metadata but is not enforced and grants no tool permissions", … } ] }
  ],
  "installed": [], "quarantined": [], "skipped": [], "failed": []
}
```

`classification` ∈ `portable | tool-specific` (a third literal `<unknown>` sits beside them).
A skill is classified **tool-specific** when the importer's body scan names a binary that
does not exist on `PATH`:

```
# body contains: Run `zzz-nonexistent-binary --go` and use the Task tool.
"classification": "tool-specific",
"unavailable_binaries": ["zzz-nonexistent-binary"]
```

Tool-specific candidates are **quarantined**, not installed:

```json
"quarantined": [
  { "id": "nonport",
    "path": "/…/.config/muse/skills/.muse/import-quarantine/nonport",
    "reasons": ["unavailable binaries: zzz-nonexistent-binary"] }
]
```

and the quarantine directory gets a `QUARANTINE.txt` (verbatim):

```
quarantined by skills import on 2026-09-01
source: /…/home4/.claude/skills/nonport (written for Claude Code)
reason: unavailable binaries: zzz-nonexistent-binary
review this skill and move its directory up to $CONFIG_DIR/skills/ to enable it
```

Other quarantine reason format string: `missing frontmatter requires: <…>` (field
`missing_requires`; a `requires:` frontmatter key is otherwise treated as an unknown field).

Foreign roots looked at (exact user-facing string):

```
No personal skills found to import (looked for ~/.claude/skills and ~/.codex/skills).
```

`--scope project` is rejected: `skills import --scope project is not supported in Phase 8`.

Import-side safety strings: `Claude skill sources must contain only regular files and
directories`, `public Codex skills path is not a frozen directory`,
`Claude commands path is not a frozen directory`, `foreign skill file name`,
`skill packages must not contain .muse files`.

---

## 9. Plugin-provided skills (PROVEN)

Requires `MUSE_EXPERIMENTAL_PLUGINS=1`.

```
$ cat demo/.muse-plugin/plugin.json
{ "schemaVersion": 1, "name": "demo-plugin", "displayName": "Demo Plugin",
  "version": "0.1.0", "description": "Demo plugin with one skill.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": { "skills": [ { "id": "plug-skill",
                                  "path": "skills/plug-skill/SKILL.md",
                                  "enabledDefault": true } ],
                    "commands": [], "hooks": [], "mcpServers": [], "reminders": [] } }

$ muse plugins install demo --json
{ "installed": { "id": "demo-plugin", …, "trust": "user-local",
                 "source": { "provenance": "native-local", "path": "…/demo" },
                 "manifest_sha256": "sha256:432a40ed…", "package_sha256": "sha256:4b36bdf9…",
                 "cache_path": "$XDG_DATA_HOME/muse/plugins/cache/local/demo-plugin/<pkgsha>/package" }, … }

$ muse skills list --source plugin --trust-workspace
NAME	SCOPE	ACTIVATION	DESCRIPTION	PATH
plugin:demo-plugin:plug-skill	plugin	on	A skill shipped by a plugin.	plugin://demo-plugin/skills/plug-skill/SKILL.md
```

Plugin ids `loop` and `muse-core` are reserved (`bundled_plugin_id_reserved`). Capability
id grammar `^[a-z0-9][a-z0-9._-]{0,79}$`, Windows reserved stems rejected. Plugin skill
paths are containment-checked (`plugin capability path escapes plugin root`,
`plugin contains symlink entries and cannot be installed`).

---

## 10. Built-in ("bundled") skills — complete dump

The binary embeds two plugin packages in rodata as a flat `<path><contents>` concatenation:

- `muse-core` — "First-party Muse skills.", schemaVersion 1, version 1.0.0, manifest at
  file offset `0xb6e0116` (6 identical copies exist in the image at
  `0xb6e02a6`, `0xb7a83f2`, `0xba3d9b6`, `0xbaabd2e`, `0xbaf8531`, `0xbb45858`)
- `tbh-reminders` — "First-party reminder agents.", at `0xb714527`
- (`loop` — "Built-in recurring-prompt command.", a command-only plugin)

The whole `muse-core` tree is materialized on first use to
`$XDG_DATA_HOME/muse/skills/bundled/muse-core/`, and my rodata carve matches it byte for
byte (`diff -r` clean except for the manifest, which is not materialized).

### 10.1 `muse-core` manifest (verbatim)

```json
{
  "schemaVersion": 1,
  "name": "muse-core",
  "displayName": "Muse Core",
  "version": "1.0.0",
  "description": "First-party Muse skills.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": {
    "commands": [], "hooks": [], "mcpServers": [], "reminders": [],
    "skills": [
      { "id": "browser-app-delivery",           "path": "skills/browser-app-delivery/SKILL.md",           "enabledDefault": true },
      { "id": "create-plugin",                  "path": "skills/create-plugin/SKILL.md",                  "enabledDefault": true },
      { "id": "plan",                           "path": "skills/plan/SKILL.md",                           "enabledDefault": true },
      { "id": "create-skill",                   "path": "skills/create-skill/SKILL.md",                   "enabledDefault": true },
      { "id": "doctor",                         "path": "skills/doctor/SKILL.md",                         "enabledDefault": true },
      { "id": "manage-settings",                "path": "skills/manage-settings/SKILL.md",                "enabledDefault": true },
      { "id": "import",                         "path": "skills/import/SKILL.md",                         "enabledDefault": true },
      { "id": "read-session",                   "path": "skills/read-session/SKILL.md",                   "enabledDefault": true },
      { "id": "greenfield-project-scaffolding", "path": "skills/greenfield-project-scaffolding/SKILL.md", "enabledDefault": true },
      { "id": "grill-and-record",               "path": "skills/grill-and-record/SKILL.md",               "enabledDefault": true },
      { "id": "grill",                          "path": "skills/grill/SKILL.md",                          "enabledDefault": true },
      { "id": "taste",                          "path": "skills/taste/SKILL.md",                          "enabledDefault": true },
      { "id": "git",                            "path": "skills/git/SKILL.md",                            "enabledDefault": true },
      { "id": "python-env",                     "path": "skills/python-env/SKILL.md",                     "enabledDefault": true },
      { "id": "table-fit",                      "path": "skills/table-fit/SKILL.md",                      "enabledDefault": true }
    ]
  }
}
```

### 10.2 File inventory (from the materialized tree)

```
   10847  LICENSE                                              (Apache-2.0)
   10282  skills/browser-app-delivery/SKILL.md
    7251  skills/create-plugin/SKILL.md
   23616  skills/create-plugin/references/capability-examples.json
    4640  skills/create-plugin/references/native-plugin-contract.md
    5697  skills/create-skill/SKILL.md
   14079  skills/doctor/SKILL.md
   31029  skills/doctor/scripts/session-evidence.py
    4110  skills/git/SKILL.md
    7974  skills/git/scripts/workspace-recovery.sh
    6211  skills/greenfield-project-scaffolding/SKILL.md
    3081  skills/grill-and-record/SKILL.md
    1552  skills/grill/SKILL.md
   13465  skills/import/SKILL.md
   18946  skills/import/scripts/find-session.py
   12361  skills/manage-settings/SKILL.md
   19739  skills/plan/SKILL.md
    2712  skills/python-env/SKILL.md
    8449  skills/read-session/SKILL.md
    3105  skills/table-fit/SKILL.md
    2033  skills/taste/SKILL.md
```

Bundled skills carry executable helpers, which is why `read_skill` bothers to expose a
"physical path": `doctor/scripts/session-evidence.py`, `import/scripts/find-session.py`,
`git/scripts/workspace-recovery.sh`.

### 10.3 Every built-in: id, frontmatter, description

`create-plugin` is present in the package but **hidden unless `MUSE_EXPERIMENTAL_PLUGINS`
is set** (PROVEN: it is absent from `muse skills list --source built-in` by default and
appears with the gate on). So the default roster is **14**, the total **15**.

| id | extra frontmatter | gated |
|---|---|---|
| `browser-app-delivery` | `user-invocable: false` | — |
| `create-plugin` | — | `plugins` |
| `create-skill` | — | — |
| `doctor` | — | — |
| `git` | `user-invocable: false` | — |
| `greenfield-project-scaffolding` | `user-invocable: false` | — |
| `grill` | — | — |
| `grill-and-record` | — | — |
| `import` | `argument-hint: "<session-id-or-path>"`, `metadata.short-description: Import a Claude Code, Codex, or Grok session` | — |
| `manage-settings` | — | — |
| `plan` | — | — |
| `python-env` | `user-invocable: false` | — |
| `read-session` | `user-invocable: false`, `metadata.short-description: Read this session's or a past session's logs` | — |
| `table-fit` | `user-invocable: false` | — |
| `taste` | `user-invocable: false` | — |

Full descriptions (verbatim from `muse skills list --source built-in --json`, plus
`create-plugin` from the carved package):

**browser-app-delivery** — *Deliver a runnable local browser app so the user can operate it on the first handoff. This delivery skill does not choose whether a requested local system should be a browser app. On a current turn that explicitly says to implement a brand-new browser project now, load bundled:greenfield-project-scaffolding before project work only after no-peek or permitted top-level placement facts prove that no named or suitable current-folder target resolves the root; never hand off from planning or answer-only turns, existing work, a named or suitable target, or standalone or single-file delivery. Resolve user-blocking product or interface choices with an available interaction surface; resolving those choices alone never authorizes implementation. Always call read_skill for bundled:browser-app-delivery before creating or materially changing a local browser app that owns its start path or a playable browser game. Also load it when the user requests a paste-ready, single-file, offline, or immediately playable browser artifact, even if no local start path should remain, unless the user says they will open or check it themselves or explicitly declines verification. On completed local-server delivery, include exact start commands, a concrete local URL, and explicit browser-open wording. On completed standalone delivery, hand off the exact artifact path or paste URL and smoke result without inventing a server. Do not load it for a self-contained static file that has no owned start path if the user either says they will open or check it themselves or explicitly declines verification. Do not load it for component/style-only edits, deployed sites, backend/API-only work, browser QA, review, explanation, plan-only, or explicit stop/no-tool turns.*

**create-plugin** — *Create and validate a new native Muse plugin package in the current workspace. Use ONLY when the user explicitly asks to create a Muse plugin or invokes the create-plugin skill. Do NOT use for application/library plugin classes, third-party plugin systems, or ordinary code changes.*

**create-skill** — *Create and validate a new Muse skill — project-local in the current workspace by default, or a personal skill staged for `muse skills install` into the managed personal root. Use ONLY when the user explicitly asks to create a Muse skill or invokes the create-skill skill. Do NOT use for ordinary skill usage, code changes, benchmark tasks, or third-party skill/plugin systems.*

**doctor** — *Diagnose Muse Code product/runtime issues from installed binary evidence. Use ONLY when the user explicitly invokes the doctor skill, asks to debug/troubleshoot Muse Code itself, asks what happened earlier in the current Muse Code session, or explicitly selects an earlier Muse Code session. Do NOT use for ordinary repository code failures or history, benchmark tasks, implementation debugging, build/test hangs, or third-party project issues.*

**git** — *Source-control safety for Git. Two rules apply whether or not you read the body. First, never commit, amend, push, tag, rebase, cherry-pick, revert, or reset --hard unless the user asked for that exact write in this session. An explicit request to create a new commit counts anywhere in the user's own task text, but it is not a request to amend, push, or tag. Finishing a task without that request is not authorization, so leave your work uncommitted for review. Second, when a Git lock file or corrupt index blocks you, never kill the holding process or bulldoze through with deletions - wait, retry, use the holder's own stop mechanism, or stop and report. Load the body only before writing Git history or recovering Git state.*

**greenfield-project-scaffolding** — *Use only when all three gates already hold. (1) This same user turn explicitly says to start, implement, build, scaffold, or go ahead with a new project now; prior answers, planning, decisions, and reminders never authorize. (2) No named path or confirmed suitable here/current-folder target resolves placement. (3) Inspection is forbidden, or permitted top-level inspection has already proved the current root home-like, general-purpose, or falsely empty. Do not load this skill merely to check eligibility. Exclude existing work, bug fixes, component edits, server or verification work, and standalone, paste-ready, single-file, or snippet delivery.*

**grill** — *Run a decision-forcing interview only when the user explicitly asks to be grilled, pressure-tested, or stress-tested.*

**grill-and-record** — *Run an explicitly requested decision interview and record each settled decision in durable project documentation.*

**import** — *Resume a third-party coding-agent session from a local transcript, path, or session id.* (short-description: *Import a Claude Code, Codex, or Grok session*)

**manage-settings** — *Any explicit Muse Code setting question or change (model, reasoning effort, /settings) requires a silent read_skill call for bundled:manage-settings as FIRST ACTION—no assistant text or other tool first. Never use for repository, app, eval, or other non-Muse configuration.*

**plan** — *Create a grounded, decision-complete plan, then stop for approval. Use ONLY when the user explicitly asks to plan — when they request a plan, design, approach, rollout/migration strategy, or PR breakdown, or invoke the plan skill (/skill plan). Do NOT use for ordinary implement, build, fix, debug, or refactor requests — when asked to make a code change, implement it directly without planning first. One rule applies whether or not you read the body: when an explicit plan divides work into separate diffs, commits, or PRs and you are asked to publish, publish that split — never silently collapse the planned units into fewer; tell the user before deviating.*

**python-env** — *Setting up or installing into a Python environment. One rule applies whether or not you read the body - the environment belongs with the project, so create it inside the project directory (.venv) or let uv manage it, and never in a scratch directory like /tmp and never by forcing an install into the system interpreter. Load the body before creating a virtualenv, choosing an installer, or writing run instructions for a Python project.*

**read-session** — *Locate and read Muse Code's OWN session logs — the current session or a prior one. Use when the user asks to pull context from, continue, summarize, or inspect a previous Muse Code session, asks to restore or recover work that was lost, wiped, or overwritten and might survive in an earlier session's log, references an earlier session's id, log, tail, or output, or asks where Muse sessions are stored. Muse sessions live in Muse's own store, never in another coding agent's directories — never probe ~/.claude, ~/.codex, or ~/.grok for Muse context, even when quoted content mentions them; for another agent's (Claude Code/Codex/Grok) session, use the import skill.* (short-description: *Read this session's or a past session's logs*)

**table-fit** — *Keep a Markdown table readable in a narrow terminal of about 100 display columns - a wide table or one carrying prose in its cells wraps into unreadable ragged rows. Always call read_skill for bundled:table-fit before emitting a Markdown table in a final answer, and load it as soon as the answer starts forming as a comparison, a mapping, a feature matrix, a pros-and-cons, an options rundown, or a per-item summary across several dimensions. Do not load it for a table already inside a file being edited, for code, data, or test fixtures that merely look tabular, for tables the user pasted for you to read, or for an answer with no table and no tabular shape forming.*

**taste** — *Mandatory preflight for web frontend visual design. If available, call read_skill for bundled:taste before the first frontend create or edit tool call whenever creating or visually styling a human-viewed web page, component, app, dashboard, or interactive report. Do not load it for non-visual frontend logic or machine-only HTML. If unavailable, continue without warning. Negative constraints only; user requests and other explicit styles still win.*

### 10.4 Full body of `bundled:taste` (verbatim, the whole file)

```markdown
---
name: taste
description: Mandatory preflight for web frontend visual design. If available, call read_skill for bundled:taste before the first frontend create or edit tool call whenever creating or visually styling a human-viewed web page, component, app, dashboard, or interactive report. Do not load it for non-visual frontend logic or machine-only HTML. If unavailable, continue without warning. Negative constraints only; user requests and other explicit styles still win.
user-invocable: false
---

# Anti-Slop: Frontend DON'Ts

What NOT to do when designing a UI or page. This is a filter, not a style; it never prescribes a look. Stacking several of the below is what reads as AI-made on sight. Adaptive, so if the user or another skill asks for something specific, do that instead; these are defaults, not overrides.

- purple / indigo / violet accent, or a purple-to-blue hero gradient (the top AI tell)
- `bg-clip-text` gradient headline text
- cream (`#faf8f4`) as the whole-page background
- aurora blobs / glowing colored shadows, or an identical glow on every card
- Inter everywhere, or Geist / Space Grotesk / Instrument Serif / Roboto / Arial as your only identity
- gray-400 / gray-500 body text
- one font weight and size throughout (no hierarchy)
- tracked all-caps micro-kickers above every section
- centered hero of eyebrow pill + headline + two CTAs
- the identical 3-card feature grid with a lucide icon in a rounded square
- an 8+ word, 48px+ headline that says nothing
- bento grid for everything
- the same eyebrow-headline-3cards section repeated back to back
- `rounded-2xl`, or any single radius, on every element
- glassmorphism blur + border + shadow on every surface
- `hover:scale-105` lift on cards
- emoji as feature icons (🚀 ⚡ ✨ 🎯 💡)
- left-border accent cards, a colored border on a rounded element, or cards nested inside cards
- fade-up / slide-up on everything as it scrolls in
- motion that ignores `prefers-reduced-motion`
- scribbly hand-drawn SVG mascots with no purpose
```

### 10.5 Full body of `bundled:create-skill` (verbatim) — the authoring contract

```markdown
---
name: create-skill
description: Create and validate a new Muse skill — project-local in the current workspace by default, or a personal skill staged for `muse skills install` into the managed personal root. Use ONLY when the user explicitly asks to create a Muse skill or invokes the create-skill skill. Do NOT use for ordinary skill usage, code changes, benchmark tasks, or third-party skill/plugin systems.
---

# Create Skill

Create one new Muse skill. Use this skill only for explicit Muse skill creation
requests, not for ordinary skill usage, code changes, benchmark tasks, or
third-party skill/plugin systems.

Two scopes exist (ADR 8975):

- **Project scope (the default)**: the skill lives in the current workspace at
  `.agents/skills/<skill-id>/`.
- **Personal scope** (the user asked for a "personal", "user", or
  "cross-project" skill): the skill belongs in the managed personal root
  `$CONFIG_DIR/skills/<skill-id>` (`$XDG_CONFIG_HOME/muse`, else
  `$HOME/.config/muse`). You stage and validate the draft in the workspace,
  then hand the user one `muse skills install` command — the store performs the
  managed install (files + provenance), so `skills update` and
  `skills uninstall` keep working on it.

Never write to a foreign harness root (`~/.codex/skills`, `~/.claude/skills`)
or to `$HOME/.agents/skills` — those are import-only sources, never write
targets. Never write directly into `$CONFIG_DIR/skills` either: the store owns
that write, through the install command below.

## Scope

- Create exactly one directory: `.agents/skills/<skill-id>/` (project scope) or
  the staging directory `.agents/skill-drafts/<skill-id>/` (personal scope —
  deliberately OUTSIDE `.agents/skills/`, so the draft is not loaded as a
  project skill).
- Create exactly one required file: `<that directory>/SKILL.md`.
- Do not create a plugin, install a plugin, enable a skill, trust a plugin, execute
  the generated skill, fetch remote content, or write outside the current workspace.
- Do not add scripts, assets, references, or extra files unless the user explicitly
  asks for them and the target remains inside the new skill directory.

## Inputs

Before writing files, identify:

- `scope`: project (default) or personal — personal only when the user asked for
  a personal/user/cross-project skill.
- `skill-id`: a portable lowercase identifier for the directory and frontmatter
  `name`.
- `description`: one clear sentence for the frontmatter.
- `body`: concise instructions that make the generated skill useful on its own.

Ask a short clarification question if the user did not provide enough information
to choose a safe `skill-id` and useful behavior.

## Safety Checks

Reject the request before writing when:

- the destination is not under `.agents/skills/` (project scope) or
  `.agents/skill-drafts/` (personal staging) in the current workspace;
- the requested final destination is a foreign harness root (`~/.codex/skills`,
  `~/.claude/skills`), `$HOME/.agents/skills`, or any absolute path outside the
  two sanctioned roots and the personal staging destination (the workspace
  `.agents/skills/` tree, `$CONFIG_DIR/skills/<skill-id>`, and
  `.agents/skill-drafts/<skill-id>`) — explain the sanctioned path instead;
- the ID is empty, `.`, `..`, contains `/` or `\`, starts with `-`, or contains
  anything except ASCII lowercase letters, digits, hyphen, or underscore;
- the ID is a Windows reserved stem such as `con`, `prn`, `aux`, `nul`, `com1`,
  `com2`, `com3`, `com4`, `com5`, `com6`, `com7`, `com8`, `com9`, `lpt1`,
  `lpt2`, `lpt3`, `lpt4`, `lpt5`, `lpt6`, `lpt7`, `lpt8`, or `lpt9`;
- the destination already exists, is a symlink, or any parent resolves outside the
  current workspace.

## Creation Steps

Use normal file and shell tools with the current workspace as the base. `<dir>`
below is `.agents/skills/<skill-id>` (project scope) or
`.agents/skill-drafts/<skill-id>` (personal scope).

1. Check that `<dir>` does not exist.
2. Create its parent (`.agents/skills` or `.agents/skill-drafts`) if needed.
3. Reserve the leaf directory with a no-replace operation. If another process wins
   the race, stop and report incomplete.
4. Recheck that the reserved directory resolves inside the current workspace and is
   not a symlink.
5. Write `<dir>/SKILL.md`.
6. Run:

   ```sh
   muse skills validate <dir> --json
   ```

7. Treat the draft as complete only when the validator succeeds, returns
   `valid: true`, and reports zero diagnostics or warnings.
8. If validation fails, correct the same draft and revalidate. Stop after three
   correction rounds and report the remaining validator output as incomplete.

## Generated `SKILL.md`

Use this shape:

```markdown
---
name: <skill-id>
description: <one sentence>
---

# <Readable Skill Name>

<Instructions for when and how to use the skill.>
```

Keep the generated instructions direct and self-contained. Include only behavior the
user asked for or that is necessary for the skill to work.

## Completion Report

On success, report:

- the canonical path to the created directory;
- the validator command and clean result;
- that no install, enable, trust, or execution step was run;
- **personal scope only**: the one command that finishes the managed install
  into the personal root — run by the user, so the skills store records the
  install provenance itself:

  ```sh
  muse skills install .agents/skill-drafts/<skill-id>
  ```

  and that the staging directory can be deleted after the install succeeds.

On failure, report:

- what operation failed;
- the destination if it was reserved;
- the remaining diagnostics or tool error;
- which checks were not completed.
```

**Note the id grammar it enforces** (this is the *authoring* rule, stricter than the
loader — a skill dir named `Kitchen_Sink` still loads; the create-skill skill just
refuses to author one): non-empty, not `.`/`..`, no `/` or `\`, no leading `-`, only
`[a-z0-9_-]`, not a Windows reserved stem.

The other bodies (`plan`, `doctor`, `import`, `manage-settings`, `read-session`,
`browser-app-delivery`, `greenfield-project-scaffolding`, `grill`, `grill-and-record`,
`git`, `python-env`, `table-fit`, `create-plugin` + its two reference files, and the three
helper scripts) are saved verbatim under
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/skills-assets/muse-core/`.

---

## 11. Complete diagnostic-code catalogue (exact, from the Rust enum literal run)

Skill-scoped codes, in binary order:

```
unknown-skill
disabled-skill
ambiguous-skill
project-skills-untrusted
invalid-skill-file
duplicate-active-skill-id
skill-file-missing
skill-file-too-large
skill-catalog-entry-too-large
skill-body-invalid-utf8
skill-body-path-unsafe
skill-alias-shadowed
skill-shadowed
skill-alias-unsafe
invalid-skill-package
install-path-unsafe
skill-already-installed
skill-not-installed
install-would-overwrite-unmanaged-files
update-source-unavailable
provenance-missing
unsupported-skill-field
skills-store-error
foreign-plugin-artifact-observed
plugin-cache-invalid
bundled_plugin_id_reserved
skill-user-invocation-marker-invalid
skill-unknown-experimental-gate
```

Loader-internal reason codes (a separate enum, used for shadowing/exclusion bookkeeping):

```
nearest_trusted_project  higher_precedence  nearer_project  duplicate  named_invalid
safe_mode  untrusted_project  disabled_plugin  not_trusted_enabled  plugin_scope_quota
plugin_preflight_overflow  plugin_class_overflow  inventory_refresh_required
invalid_utf8  bom_forbidden  frontmatter_invalid  duplicate_key  invalid_name
missing_field  invalid_field  unknown_field  field_too_large  candidate_too_large
missing_source  wrong_kind  symlink_or_reparse  path_escape  non_utf8_path  cycle
inventory_changed  unstable_read  source_bound  snapshot_bound  inventory_integrity
join_mismatch  package_digest_mismatch  not_found  lookup_expectation_mismatch
unknown_tool  tool_classification  epoch_mismatch  grant_bound  forged_dispatch
known_field_inactive  restricted_source_field
```

Bundled-loading errors:

```
failed to load bundled skills: muse-core package is missing
failed to load bundled skills: skill entry missing id
failed to load bundled skills: skill entry missing path
failed to load bundled skills: unsafe package path `<p>`
failed to materialize bundled skill files: <e>
bundled skill package must contain exactly one root `SKILL.md`
bundled skill `SKILL.md` must be valid UTF-8
bundled skill entry, no_qualified_form
```

---

## 12. Limits: measured vs. declared

| limit | value | status |
|---|---|---|
| `SKILL.md` loader cap | **262 144 bytes** | PROVEN (`skill-file-too-large`) |
| `skills_catalog` context-block budget | **~32 768 bytes**, with per-entry trimming then an id-only fallback | PROVEN (30 000-byte description → 31 969 B block; 40 000 → 1 933 B, no descriptions) |
| `read_skill` read guard | exists (`warning: skill file exceeded the <N> byte read guard`) | **UNKNOWN value** |
| skill package max entries | format string exists; **not reached at 3 001 files** | UNKNOWN |
| skill package max nesting depth | format string exists; **not reached at 40 levels** | UNKNOWN |
| lockfile max bytes | `lockfile exceeds <N> bytes` | UNKNOWN |
| skill-reminder body template | `maxBytes: 65536` | PROVEN (manifest) |
| skill-reminder `skill_catalog` feed | `maxBytes: 128000`, `refresh: run_start` | PROVEN |
| skill-reminder `skill_read_ledger` feed | `maxBytes: 24000`, `refresh: boundary` | PROVEN |
| skill-reminder `visible_for_steps` | 1..8 | PROVEN |

---

## 13. Compatibility with Claude Code's SKILL.md convention

### What is identical

- Directory-per-skill, one `SKILL.md` at its root, sibling `scripts/` and `references/`.
- YAML frontmatter delimited by `---` … `---`, Markdown body after it.
- Frontmatter keys `name`, `description`, `allowed-tools`, `argument-hint`,
  `disable-model-invocation`, `license`, `metadata` are all recognized and are exactly
  Muse's declared "common subset" (`agent-skills-common-subset`).
- `metadata.short-description` is honoured (rendered as `<short-description>` in the catalog).
- `allowed-tools` accepts Claude's selector syntax including `Bash(git status:*)` and
  `mcp__server__tool` forms — they parse and round-trip.
- The progressive-disclosure model is the same: descriptions at session start, body on demand.

### What differs — a Claude skill dropped into Muse

| aspect | Claude Code | Muse Code | consequence |
|---|---|---|---|
| native project root | `.claude/skills/` | `.agents/skills/` | Muse **also reads** `.claude/skills/` (lowest project precedence) — a Claude project skill works unmodified |
| native personal root | `~/.claude/skills/` | `$CONFIG_DIR/skills/` | Muse **also reads** `~/.claude/skills/` (3rd of 4) — a Claude personal skill works unmodified |
| `name` required? | yes | **no** — falls back to directory name | Muse is more permissive |
| `allowed-tools` enforced? | yes (permission gate) | **no** — "recorded as advisory metadata and is not enforced and grants no tool permissions"; raises a `unsupported-skill-field` **warning** | **Security-relevant**: a Claude skill relying on `allowed-tools` to *narrow* its own permissions gets no narrowing in Muse |
| `agent:` / `context:` | Claude features | `unsupported_fields`, warned, treated as metadata | subagent-scoped Claude skills silently run in-session |
| `model:` | supported | `unknown_fields` (ignored) | model pinning lost |
| trust gate | Claude trusts the folder | project skills need explicit workspace trust | a Claude skill silently does not load in an untrusted workspace (with a `project-skills-untrusted` diagnostic) |
| provenance labelling | n/a | catalog entries get `origin-family=".claude" written-for="Claude Code"` and the reminder tells the model to discount harness-specific instructions | Claude skills are *deliberately* down-weighted |
| slash commands | `.claude/commands/*.md` | Muse plugin `commands` capability; `Claude hook event 'Setup' is recognized but is not run by Muse` | commands do not come along with skills |
| Muse-only key | — | `experimental-gate:` | ignored by Claude Code (unknown key) — safe to add |
| id namespace | flat | `bundled:<id>`, `plugin:<pid>:<sid>`, bare for user/project | a project skill can shadow a built-in's bare alias but never its qualified id |
| import path | — | `muse skills import --from claude` copies+hashes+quarantines | one-shot migration with a body scan for missing binaries |

**Net:** Claude Code skills are ~95 % drop-in. The two real behavioural gaps are
(1) `allowed-tools` is decorative, and (2) `agent:`/`context:` are dropped. Both are
surfaced as warnings by `muse skills validate` and by
`muse skills import --from claude --dry-run`, so the incompatibility is at least honest.

Codex skills (`~/.codex/skills`, `<ws>/.codex/skills`) are treated identically, tagged
`written-for="Codex"`.

---

## 14. Reproduction recipes

```sh
export MUSE_NO_AUTO_UPDATE=1
export HOME=/tmp/scratch/home ; mkdir -p "$HOME"
MUSE=/path/to/muse-aarch64-macos
mkdir -p /tmp/scratch/ws && cd /tmp/scratch/ws

# 1. built-ins
$MUSE skills list --source built-in --json
MUSE_EXPERIMENTAL_PLUGINS=1 $MUSE skills list --source built-in     # reveals create-plugin

# 2. materialize the bundled package to disk (any store write does it)
mkdir -p .agents/skills/x && printf -- '---\nname: x\ndescription: d.\n---\nb\n' > .agents/skills/x/SKILL.md
$MUSE skills install .agents/skills/x --scope user --json
ls -R "$HOME/.local/share/muse/skills/bundled/muse-core"
cat "$HOME/.config/muse/skills/.muse/lock.json"
cat "$HOME/.config/muse/skills/.muse/audit.log"

# 3. trust gate
$MUSE skills list --source project                 # project-skills-untrusted
$MUSE skills list --source project --trust-workspace

# 4. activation state store
$MUSE skills disable x --scope project --trust-workspace --json
$MUSE skills user-only bundled:git --scope built-in --json
cat "$HOME/.config/muse/settings.json"

# 5. capture the exact injected catalog
$MUSE exec --provider echo --trust-workspace "hi"
f=$(find "$HOME/.local/share/muse/sessions" -name session.jsonl | head -1)
python3 - "$f" <<'PY'
import json,sys
for line in open(sys.argv[1]):
    try: d=json.loads(line)
    except: continue
    ev=d.get('payload',{}).get('event',{})
    if ev.get('kind')=='model_request_configured':
        for m in ev.get('run_context_messages',[]):
            if m['id']=='skills_catalog': print(m['text'])
        break
PY

# 6. carve the bundled package straight out of rodata (no run needed)
python3 - <<'PY'
data=open('muse-aarch64-macos','rb').read()
i=data.find(b'{\n  "schemaVersion": 1,\n  "name": "muse-core"')
print(hex(i))         # 0xb6e0116 on 1.0.1-R2006.1
PY
```

---

## 15. Open questions

1. The numeric `read_skill` read guard (`skill file exceeded the <N> byte read guard`) —
   not observable without a real model turn.
2. Actual thresholds for `skill package exceeds the maximum of <N> entries` and
   `skill package directory nesting exceeds the maximum depth of <N>` (not hit at 3 001
   entries / 40 levels).
3. How hooks are enabled in this build — neither `.muse/hooks.json` nor
   `$CONFIG_DIR/hooks.json` with deliberate garbage produced any error, so the
   `selectedSkills` path could not be exercised end-to-end (the contract itself is proven
   by exact validation strings and the reminder template).
4. Purpose of the `.muselock.json` literal that sits in the same string run as
   `$CODEX_HOME/skills` / `~/.codex/skills` / `quarantine` / `audit.log` / `.skills.lock`
   — never written by any command I ran. INFERRED: a marker Muse would drop into a foreign
   root to record what it imported.
5. Complete list of Claude-specific keys that map to `unsupported_fields` — only `agent`
   and `context` were confirmed.
6. Why project-scope root precedence (`.agents > .codex > .claude`) is the inverse of
   user-scope (`$CONFIG_DIR > .agents > .claude > .codex`). Reproducible, but looks
   unintentional.
7. Whether `skill-alias-shadowed` / `skill-alias-unsafe` ever fire (a project skill named
   `plan` did not produce them).
8. `context_cost.invoke_bytes` is always `null` from the CLI; presumably a TUI-only field.
9. Marketplace-sourced skills (`marketplace-static` / `marketplace-git` `LockSource`
   variants with `repository`/`requested_ref`/`resolved_revision`/`sparse_path`) — the
   `muse skills` CLI has no marketplace verb, only `muse plugins marketplace *`. INFERRED:
   skills reach a marketplace only inside a plugin package, but the lockfile is already
   shaped for direct skill marketplaces.

---

## 16. Extension points for an "oh-my-musecode" framework

See the structured summary accompanying this report; briefly, the highest-leverage seams
are `.agents/skills/` (zero-config drop-in, but trust-gated), the managed store +
`lock.json` (a real package manager already exists, with hashes, provenance, update and
audit), `muse skills install` / `import` as the distribution verbs, plugin packages as the
bundle format (skills + commands + hooks + MCP servers + reminder agents in one manifest),
`experimental-gate:` for staged rollout, `skills.activation` in `settings.json` as the
declarative on/off manifest, `context_slimming.full_skill_description_ids` as the
token-budget knob, and the `selectedSkills` hook + skill-reminder agent as the two
programmable routing layers.

---

## Verification

Adversarial re-verification performed independently in
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/verify-skills/`
(fresh throwaway `$HOME` per experiment: `home`, `homeE`, `homeM`, `homeB`, `homeN`, `homeS`,
`homeF`, `homeL`, `homeI`, `homeD`, `homeG`, `homeH`, `homeY`, `homeZ`, `homeQ`, `homeR`,
`homeW`, `homeA`, `homeC`, `homeInit`), binary `1.0.1 (1.0.1-R2006.1)`.

**Verdict: MOSTLY_SOLID.** ~26 of 32 claims reproduced exactly. Six are wrong, one of them
the report's own headline "probably a bug" finding.

### Refuted

**R1 — §2.3 / claim 14: the "important asymmetry (PROVEN, and probably a bug)" is not real.**
Settings-driven `user-invocable-only` does **not** remove a skill from the model catalog. It
renders it with `model-invocable="false"`, byte-identically to frontmatter
`disable-model-invocation: true`.

```
# wsM: p-uio set with `muse skills user-only p-uio --scope project`
#      p-dmi has `disable-model-invocation: true`
#      u-uio set with `muse skills user-only u-uio --scope user`
$ muse exec --provider echo --trust-workspace "hi"   # then read skills_catalog
<skill id="p-dmi" scope="project" path=".agents/skills/p-dmi/SKILL.md" model-invocable="false">
<skill id="p-on"  scope="project" path=".agents/skills/p-on/SKILL.md">
<skill id="p-uio" scope="project" path=".agents/skills/p-uio/SKILL.md" model-invocable="false">
<skill id="u-on"  scope="user"    path="$HOME/.agents/skills/u-on/SKILL.md">
<skill id="u-uio" scope="user"    path="$HOME/.agents/skills/u-uio/SKILL.md" model-invocable="false">
```

The same holds for bundled scope: `muse skills user-only bundled:plan --scope built-in`
yields `<skill id="bundled:plan" ... model-invocable="false">`.

The report's observation was a confound. The **actual** rule is:

| frontmatter `user-invocable` | activation | catalog |
|---|---|---|
| (default true) | `on` | listed, no attribute |
| (default true) | `user-invocable-only` | listed, `model-invocable="false"` |
| `false` | `on` | listed, no attribute |
| `false` | `user-invocable-only` | **absent** (neither model- nor user-invocable) |
| any | `off` | absent |

Proof of the last interesting row: `p-uif` (`user-invocable: false` + `muse skills user-only
p-uif --scope project`) and `bundled:table-fit` (ships `user-invocable: false`, set to
`user-invocable-only`) are both absent from the catalog while every other bundled entry is
present. §6's bullet "`user-invocable-only` ⇒ excluded from the model catalog" is wrong for
the same reason.

**R2 — §2.1 / claim 26: `muse skills install` does *not* refuse a symlinked source.**

```
$ ln -s "$PWD/real/symtarget" .agents/skills/symlinked
$ muse skills install .agents/skills/symlinked --scope user --json
{"installed":{"id":"symtarget","path":"$CONFIG_DIR/skills/symtarget", ...}}     # SUCCESS
$ ln -s ../../real/rel .agents/skills/rel        # relative symlink
$ muse skills install .agents/skills/rel --scope user --json  -> installed OK
$ ln -s ./nonexistent .agents/skills/broken
$ muse skills install .agents/skills/broken --scope user --json
{"error":{"code":"invalid-skill-package","message":"No such file or directory (os error 2)"}}
```

The error the report quotes is what a **dangling** symlink produces. Its symlink target was
broken. (The symlinked-*directory*-loads / symlinked-`SKILL.md`-does-not halves both
reproduce; the symlinked `SKILL.md` case fails validate with
`{"code":"install-path-unsafe","message":"skill package path escapes source root"}`, not the
`direct SKILL.md symlinks are not local skills` string the report attributed to it.)

**R3 — §2.4 / §10.3 / claim 22: `create-plugin` does *not* use `experimental-gate`.**

```
$ head -4 $BUNDLED/skills/create-plugin/SKILL.md
---
name: create-plugin
description: Create and validate a new native Muse plugin package in the current workspace. ...
---
$ grep -rn "experimental-gate" $BUNDLED   ->  (no matches anywhere in the 21-file package)
```

Its frontmatter carries only `name` and `description`; the `muse-core` manifest entry is a
plain `{"id":"create-plugin","path":...,"enabledDefault":true}`. The gate is enforced in code,
not declared in the skill. The rest of claim 22 (unknown gate ⇒
`skill-unknown-experimental-gate` and exclusion; valid gate hides until set) reproduces.

**R4 — §5.2 / claim 15: budget is exactly 32 000 bytes, and nothing is "trimmed".**
Binary search on a single growing description in a 15-skill catalog:

```
desc bytes  block bytes  <description> count
     20000        30136   15
     24000        31640   11   (4 entries now id-only self-closing)
     30000        31971    1   (14 entries id-only)
     30029        32000    1   <-- maximum block ever observed
     30030         1935    0   (whole catalog collapses to id-only)
```

The cap is **32000**, not 32768/32 KiB, and descriptions are never truncated — entries are
rendered in catalog order and every entry *after* the running total is exhausted loses its
`<description>` wholesale. A single fat description therefore starves everything sorted after
it. (The `context_block_diagnostic` for the block reports `max_bytes: 0`, so the 32000 cap
lives in the skills renderer, not the context-block lane.)

**R5 — §10.2 / claim 11: the bundled helper scripts are *not* executable.**

```
$ ls -l $BUNDLED/skills/git/scripts/
-rw-r--r--  1 ... 7974 workspace-recovery.sh
$ find $BUNDLED -type f -perm -u+x   ->  (empty)
```

All 21 materialized files are mode 0644. The file inventory and byte sizes in §10.2 are
otherwise byte-for-byte correct.

**R6 — §2.2 parser-rules table: `duplicate_key` and `bom_forbidden` do not fire.**
Both rows sit under "Parser rules (PROVEN by error injection)" but were never injected.

```
$ printf -- '---\nname: dupkey\ndescription: one.\ndescription: two.\n---\nb\n' > .agents/skills/dupkey/SKILL.md
$ printf '\xef\xbb\xbf---\nname: bomtest\ndescription: bom.\n---\nb\n'          > .agents/skills/bomtest/SKILL.md
$ muse skills list --source project --trust-workspace
bomtest  project  on  bom.   .agents/skills/bomtest/SKILL.md
dupkey   project  on  two.   .agents/skills/dupkey/SKILL.md
$ muse skills validate .agents/skills/dupkey  --json  -> valid:true, diagnostics:[]
$ muse skills validate .agents/skills/bomtest --json  -> valid:true, diagnostics:[]
```

A duplicate YAML key is last-wins; a UTF-8 BOM is accepted silently.

**R7 — §5.6 / claim 19: `selectedSkills` is not reachable, and it is not a SessionStart hook.**
Hooks *do* run in this build (see M6), so the contract could finally be exercised end to end —
and it fails. A plugin hook returning the exact documented payload:

```
$ muse plugins hook test hookplug:sel --fixture fixture.json --json     # event SessionStart
"status": "failed",
"error": "unsupported `selectedSkills` in hookSpecificOutput of SessionStart hook output"
```

Sweeping every event (same payload, `hookEventName` matched to the event):

```
UserPromptSubmit  -> completed, error null       <-- the ONLY accepting event
PostToolUse       -> completed, error null       (accepted, no observable effect)
SessionStart / PreToolUse / PreCompact / PostCompact / SubagentStart /
SubagentStop / Stop / SessionEnd / PermissionRequest
                  -> failed, "unsupported `selectedSkills` in hookSpecificOutput of <E> hook output"
```

And with the hook wired to `UserPromptSubmit`, both gates set
(`MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS=1 MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS_APPLY=1`),
the plugin installed *and* `muse plugins approve`d, a real run rejects it:

```
$ muse exec --provider echo --trust-workspace "hi"
# session.jsonl, hook_run_terminal:
"status": "failed", "exit_code": 0,
"error": "selected_skills:rejected:capability-not-negotiated"
# run_context_messages: no selected_skills_catalog block is produced
```

Rejection-reason enum at `0xbbfcf43`: `ignored_off`, `capability-not-negotiated`,
`provenance-unavailable`, `invalid-adapter-report`, `invalid-adapter-prepared-state`,
`adapter-failed`, `adapter-unavailable`. So the hook-selected-skills path requires a
*negotiated client capability* (MSP host), and is inert in the CLI. The strings the report
quotes are all real; the feature is not usable, and its event binding is `UserPromptSubmit`,
not `SessionStart`.

### Corrections

**C1 — `unsupported_fields` is `{agent, context, hooks}`, three keys, not two.** This closes
open question #5. A 47-key probe SKILL.md plus a 40-key second round found exactly three:

```
$ muse skills validate .agents/skills/probe --json
"unsupported_fields": ["agent","context","hooks"]
$ muse skills validate .agents/skills/kitchen-sink --json
  diagnostics: `allowed-tools` ... / Claude field `agent` ... / `context` ... / `hooks` ...
```

Nothing else in {model, tools, mcpServers, memory, color, effort, permission-mode, max-turns,
background, isolation, initial-prompt, outputStyle, output-style, skills, commands, settings,
reminder, user-config, plugin, plugins, env, cwd, timeout, when, triggers, keywords, category,
tags, icon, emoji, title, author, homepage, repository, requires, prompt, system-prompt,
subagent, subagents, agents, context-files, scripts, references, assets, allowedTools,
disallowed-tools, additionalDirectories, statusLine, permissions, sandbox, network, …} lands
there — all are `unknown_fields`. The rest of the key partition in §2.2 reproduces exactly.

**C2 — id resolution has a fourth, higher rule the report missed: the lockfile id wins in the
managed store.**

```
$ muse skills install .agents/skills/ok --scope user --name renamed --json   # SKILL.md says `name: ok`
$ cat $CONFIG_DIR/skills/renamed/SKILL.md | head -2   ->  name: ok
$ muse skills list --source user --json               ->  id "renamed"
$ # blank the lockfile's skills map, re-list:
$ muse skills list --source user --json               ->  id "ok"
```

So: **lockfile record id (managed store only) > frontmatter `name` > directory name.**
Outside the managed store the report's rule is right (verified in `$CONFIG_DIR/skills`,
`$HOME/.agents/skills`, `$HOME/.claude/skills` and `<ws>/.agents/skills`, all four honour
frontmatter `name`).

**C3 — `CODEX_HOME` *replaces* `~/.codex`, it does not add to it.** §3.1 lists both as if
additive.

```
$ muse skills list --source user                   ->  cxdefault  $HOME/.codex/skills/...
$ CODEX_HOME=/tmp/cxhome muse skills list --source user  ->  cxh  $CODEX_HOME/skills/...
                                                             (cxdefault gone)
```

**C4 — bundled materialization is triggered by skill *loading*, not by store writes.** §14
step 2 says "any store write does it". `homeL` (only `muse skills install`) and `homeI` (only
`muse skills import`) have **no** `~/.local/share/muse/skills/bundled/` tree at all. `muse
skills list` and `muse exec` do materialize it.

**C5 — `--force` does not override `install-would-overwrite-unmanaged-files`.**

```
$ muse skills install .agents/skills/w1 --scope user --force --json
{"error":{"code":"install-would-overwrite-unmanaged-files", ...}}   # same with and without --force
$ # --force DOES override skill-already-installed:
$ muse skills install .agents/skills/w1 --scope user --json         -> skill-already-installed
$ muse skills install .agents/skills/w1 --scope user --force --json -> installed
```

**C6 — `audit.log`'s `source` field is a hard-coded literal `"local"`.** An *imported* skill
(lock `source.type == "imported"`, `trust == "imported"`) still logs `"source":"local"`,
matching the format string `{"time":"…","action":"…","skill":"…","source":"local","result":"ok"}`.
The ohmy hook that pitches audit.log as provenance should say so.

**C7 — the `LockSource` variant→field mapping in §7.1 is INFERRED, not proven, and two of the
five variants were never observed.** What the binary actually proves is a *flat* field list
plus a serde arity:

```
0xba3bbec: …idsourcemarketplaceecosystemsource_pathskillindexpackage_hash
           repositoryrequested_refresolved_revisionsparse_pathmanifest_pathmanifest_hash…
           struct LockSkillRecord with 10 elements
           struct LockScan with 2 elements
           struct LockFileRecord with 3 elements
           struct Lockfile with 2 elements
           struct LockSource with 14 elements
0xba3c471: …marketplace-staticmarketplace-gitlocal…
```

`struct LockSource with 14 elements` = type + the 13 field names, so the names *are* serde
fields (good), but nothing in the image says which variant owns which field. Empirically only
`local` and `imported` were ever written; `marketplace`, `marketplace-static`,
`marketplace-git` were never produced by any command. Also note the observed `imported` record
carries **both** `path` and `source_path` with the same value, plus `ecosystem`.

**C8 — `muse skills validate` does *not* flag an unknown `experimental-gate`.** The ohmy hook
"validate as a CI/lint gate … fail on any diagnostic" is weaker than advertised:

```
$ muse skills validate .agents/skills/bogus-gate --json
{"valid": true, ..., "diagnostics": [], "compatibility": {..., "unknown_fields":["experimental-gate"]}}
```

The skill is nevertheless silently excluded at load time. Only `muse skills list` surfaces
`skill-unknown-experimental-gate`.

**C9 — small factual drift.** `context.foreign_personal_skills:false` affects **personal
scope only**; `<ws>/.claude/skills` and `<ws>/.codex/skills` keep loading (verified). The
`muse-core` manifest occurs **6×** at `0xb6e0116, 0xb7a8262, 0xba3d826, 0xbaabb9e, 0xbaf83a1,
0xbb456c8` — §10's second offset list is 0x190 off. The discovery-roots literal is at
`0xbb9cd49`, not `0xbb9cd8b`. The `active_tools` list in §5.4 omits `subagent_cancel`. One of
the four copies of the diagnostic-code run (`0xb6c0660`) has only 27 codes — it omits
`plugin-cache-invalid`; the other three (`0xb738bd2`, `0xb8076dc`, `0xba3c538`) have all 28,
so §11's list stands. `skill install --scope project` errors with
`local installs only support --scope user, got project`. The reminder declaration's
`validatorRejections` is a **map** of branch name → proposal template, not a set.

### Missed ground

**M1 — the catalog silently *drops whole skills*, not just descriptions, past 32 000 bytes.**
This is the single most important omission for an oh-my-musecode framework and the report
never tested it.

```
$ # 600 project skills, each with a ~40-byte description (+ 15 bundled = 615 total)
$ muse exec --provider echo --trust-workspace "hi"
block bytes 31942  skills 426  descs 0
first ids: bundled:browser-app-delivery …   last ids: … s0409 s0410 s0411
$ muse skills list --source project --trust-workspace --json  -> 600 skills, diagnostics: []
$ grep -c "aggregate budget" session.jsonl  ->  0
```

189 skills vanish from the model's view with **zero** diagnostics anywhere — not in the block's
`<skill-diagnostics>`, not in `muse skills list`, not in the session log. The string
`skill omitted to keep the startup catalog within its aggregate budget of <N> bytes`
(`0xcbb6d25`) exists but never fired in any run. Combined with R4 this means
`run.context_slimming.skill_catalog_descriptions: first_sentence` is not merely a token
optimisation — past ~400 skills it is the difference between a skill existing and not.

**M2 — discovery is exactly one directory level deep, and violations are silent.**
`.agents/skills/nested/inner/SKILL.md` is not discovered and produces no diagnostic
(`muse skills list --source project --trust-workspace --json` → `diagnostics: []`). Likewise
`.agents/skill-drafts/<id>/SKILL.md` is not loaded (which confirms the create-skill body's
claim), and `muse init` writes only `AGENTS.md` — it scaffolds nothing skill-related.

**M3 — `muse skills inspect --json` has its own schema, never documented.** It is *not* the
`list` element shape: it wraps everything in `{"skill": {...}}` and adds a `frontmatter`
object that is **always empty** in this build:

```json
{"skill":{"id":"bundled:import","name":"import","display_name":"import","description":"…",
 "short_description":"Import a Claude Code, Codex, or Grok session","scope":"bundled",
 "source":{"type":"local"},"path":"bundled://muse-core/skills/import/SKILL.md",
 "activation":"on","diagnostics":[],"provenance":null,
 "frontmatter":{"known":{},"unknown":{}}}}
```

`frontmatter.known` / `frontmatter.unknown` were `{}` for every skill tried (bundled, project,
kitchen-sink with 22 keys) — a dead field. Also: `inspect` accepts a `SKILL.md` *file* path but
**not** a skill *directory* path (`skill not found: .agents/skills/plan`).

**M4 — `metadata.short-description` beats a top-level `short-description`.** A kitchen-sink
carrying both yields `"short_description": "short desc here"` (the `metadata.` one); the
top-level key is an `unknown_field` and is discarded. Worth stating since Claude authors may
reach for the flat key.

**M5 — there is no CLI path to persist workspace trust.** `muse --help` has no `trust`
subcommand, `--trust-workspace` is documented as "does not save trust", and `$CONFIG_DIR/trust.json`
was **never written** by any of the ~60 invocations in this verification. (The filename is a
genuine code constant — `auth.jsonsettings.json…trust.json` at `0xbba0138` and a standalone
copy at `0xbba262c` — the other 12 occurrences are prose inside the bundled `doctor` and
`manage-settings` bodies.) So the ohmy hook "tell users about `--trust-workspace` / persisting
trust" has no scriptable persistence path: trust must be granted interactively in the TUI, or
passed per-run. A framework shipping project skills must plan for that.

**M6 — how hooks are enabled (open question #3, answered).** Hooks are a **plugin capability**,
not a config file. `.muse/hooks.json` and `$CONFIG_DIR/hooks.json` are ignored because they are
not a thing in this build.

```
$ export MUSE_EXPERIMENTAL_PLUGINS=1
$ cat hp/.muse-plugin/plugin.json
{ "schemaVersion":1, "name":"hookplug", "version":"0.1.0", "description":"…",
  "compat":{"source":"native","manifestDir":".muse-plugin"},
  "capabilities":{ "skills":[], "commands":[], "mcpServers":[], "reminders":[],
    "hooks":[{"id":"sel","event":"UserPromptSubmit","command":["sh","hooks/sel.sh"],
              "timeoutMs":3000,"statusMessage":"selecting"}] } }
$ muse plugins install hp --json     # enabled:true, trust:"user-local"
$ muse plugins approve hookplug --json
{"decision":"approve","runtime_capabilities":[
  {"stable_id":"plugin:hookplug:hook:sel","trusted_definition_hash":"sha256:…","enabled":true}]}
$ muse exec --provider echo --trust-workspace "hi"
# session.jsonl now carries hook_run_started / hook_run_terminal records with
# origin {plugin_id, capability_id, source_digest}
```

Offline single-hook testing is `muse plugins hook test <plugin>:<hook> --fixture f.json --json`,
where the fixture must be `{"event":"<Event>","stdin":{…}}` (both keys required; the error
messages are `fixture event must be \`SessionStart\` or \`session_start\`` and
`fixture must contain \`stdin\``). This is a strictly better extension seam than anything in
§16 and belongs in the ohmy hooks list.

**M7 — the serde arity strings are the real proof that the lockfile field names are fields.**
`struct LockSkillRecord with 10 elements` / `LockScan with 2` / `LockFileRecord with 3` /
`Lockfile with 2` / `LockSource with 14` at `0xba3bbec` are deserializer `expecting` messages
and pin the exact cardinality of each struct. A neighbouring pool at `0xbaaad13` names four
more lock structs the report never mentions: `LockMarketplaceRecord`, `LockMarketplaceSource`,
`MarketplaceLockfile`, `LockPluginRecord`, `LockSourceRecord` — i.e. the *marketplace* lockfile
is a separate file/type family from `$CONFIG_DIR/skills/.muse/lock.json`, which weakens the
"a git-backed skill registry is pre-wired in the skills lockfile" reading in the ohmy hooks.

**M8 — `context_slimming` has a self-documenting failure mode** worth citing instead of the
symbol-name run:

```
$ echo '{"schema_version":1,"run":{"context_slimming":{"skill_catalog_descriptions":"bogus_mode"}}}' > $CONFIG_DIR/settings.json
$ muse skills list --source built-in
malformed settings file at …/settings.json: unknown variant `bogus_mode`,
expected `full` or `first_sentence` at line 1 column 88
```

**M9 — caveat the report should carry: this is a case-insensitive filesystem.** A skill file
named `skill.md` loaded fine here, but that is APFS folding `SKILL.md`, not a Muse behaviour.
Any claim about `SKILL.md` casing is untested on a case-sensitive volume.

### Reproduced exactly (no change)

Claims 1, 2, 4, 5, 6, 7, 8, 9, 10, 12, 13, 16, 17, 18, 20, 21, 23, 24, 25, 27, 28, 29, 30, 31
all reproduced. Spot-checks worth recording: the loader cap boundary is exact (262144 loads,
262145 → `skill-file-too-large` reporting `262145 bytes exceeds 262144 bytes`); the settings
`skills.activation` file shape is byte-identical to §6; `lock.json`, `audit.log`,
`.skills.lock`, `quarantine/` and `import-quarantine/<id>/QUARANTINE.txt` all appear exactly as
described; `muse schema generate-ts|generate-json-schema [--experimental]` produce four files
containing zero case-insensitive matches for "skill"; `context_block_diagnostic` confirms
`lane=context_block lifecycle=session_start order=200 cache_class=stable_prefix status=supported`;
project-root precedence really is `.agents > .codex > .claude` while user-root precedence is
`$CONFIG_DIR > .agents > .claude > .codex`, reproduced additively in both directions.
