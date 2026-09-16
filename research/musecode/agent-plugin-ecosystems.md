# Agent Plugin Ecosystems — Field Survey (Sept 2026)

**Purpose:** establish the exact extension formats that today's CLI coding agents use, so an
"oh-my-musecode" framework can be built to interoperate rather than invent.

**Method note.** Where possible this report cites *primary artifacts on disk*, not blog posts:

- Claude Code `2.1.252` — Mach-O arm64 binary at `/Users/cph/.local/share/claude/versions/2.1.252`.
  Zod schemas extracted from the embedded bun bundle (`strings` + offset extraction). These are
  the loader's actual validators, and they are *ahead of* the published docs.
- Codex CLI `0.148.0` — Rust binary at `/Users/cph/.local/bin/codex` (214 MB). Serde struct field
  lists and embedded JSON Schemas extracted the same way.
- Locally installed plugins/marketplaces under `~/.claude/plugins/` and `~/.codex/`.
- Muse Code — no local install; sourced from vendor docs + third-party reverse-engineering, flagged
  inline with confidence.

---

## 1. Claude Code — the reference implementation

### 1.1 Plugin layout

```
my-plugin/
├── .claude-plugin/
│   └── plugin.json          # THE manifest. Must be in .claude-plugin/, not the root.
├── skills/                  # <name>/SKILL.md directories
│   └── my-skill/SKILL.md
├── commands/                # flat *.md slash commands
├── agents/                  # *.md subagent definitions
├── hooks/
│   └── hooks.json
├── output-styles/
├── themes/
├── workflows/               # *.js workflow scripts
├── monitors/
│   └── monitors.json
├── bin/                     # binaries added to PATH
├── scripts/
├── .mcp.json                # MCP servers
├── .lsp.json                # LSP servers
└── LICENSE
```

Everything except `plugin.json` lives at the **plugin root**, *not* inside `.claude-plugin/`.
The loader's own error string confirms the sniff set:

> `has no plugin content at its root (expected .claude-plugin/ or a commands/, skills/, agents/,
> hooks/, themes/, output-styles/, monitors/, workflows/, SKILL.md, .mcp.json, or .lsp.json at the
> top level, optionally inside a single wrapper directory)`

Manifest discovery is a two-candidate probe (from the binary's `Nt()` resolver):

1. `<path>/.claude-plugin/plugin.json` (plugin root = `<path>`)
2. `<path>/plugin.json` (plugin root = `dirname(<path>)`) — i.e. you may point directly at a
   manifest file.

Failure message: `No plugin manifest found. Expected <path>/.claude-plugin/plugin.json.`

### 1.2 `plugin.json` — full field set

Reconstructed from the composed schema `jpe = { ...Cs, ...zs, ...Rs, ...Us, ...Es, ...ct, ...pt,
...Ls, ...Bs, ...js, ...Ws, ...mt, ...Ys, ...Fs, ...qs, ...Vs }`. `name` is the only required field.

```jsonc
{
  "$schema": "https://anthropic.com/claude-code/plugin.schema.json",  // ignored at load
  "name": "my-plugin",              // REQUIRED. no spaces, no control/bidi chars. kebab-case by convention
  "displayName": "My Plugin",       // UI only; may contain spaces/casing. Not used for namespacing
  "version": "1.2.3",               // semver string (not hard-validated in plugin.json)
  "description": "…",
  "author": { "name": "…", "email": "…", "url": "…" },   // name required if author present
  "homepage": "https://…",          // must parse as URL
  "repository": "https://…",
  "license": "MIT",                 // SPDX
  "keywords": ["a", "b"],

  "defaultEnabled": true,           // starts enabled when user has no explicit setting
  "dependencies": ["other-plugin", "other@marketplace", {"name":"x","marketplace":"y"}],
  "metadata": { "anything": "…" },  // preserved, never read by Claude Code

  // ---- component paths. ALL must be relative and start with "./" ----
  "commands": "./commands" | ["./a.md", "./b/"] | { "about": {…} },  // REPLACES commands/ scan
  "agents":   "./agents/x.md" | ["./a.md"],                          // REPLACES agents/ scan
  "skills":   "." | "./skills/x" | ["./skills/x"],                   // ADDS to skills/ scan
  "hooks":    "./hooks.json" | {…inline…} | [ … ],                   // ADDS to hooks/hooks.json
  "mcpServers": "./.mcp.json" | {…} | "./x.mcpb" | [ … ],
  "lspServers": "./.lsp.json" | {…} | [ … ],
  "outputStyles": "./output-styles" | [ … ],                         // REPLACES output-styles/ scan
  "themes": "./themes" | [ … ],
  "workflows": "./workflows" | [ … ],
  "monitors": "./monitors.json" | [ {name, command, description, when} ],

  // ---- advanced ----
  "userConfig": {                   // prompted at enable time
    "API_KEY": {
      "type": "string|number|boolean|directory|file",
      "title": "API key",
      "description": "…",
      "required": true,
      "default": "…",
      "multiple": false,
      "sensitive": true,            // → OS keychain, not settings.json
      "min": 0, "max": 100
    }
  },
  "channels": [ { "server": "telegram", "displayName": "Telegram", "userConfig": {…} } ],
  "settings": { "…": "…" },         // merged into user settings while enabled (allowlisted keys only)
  "binaries": { "mytool-aarch64-apple-darwin": { "sha256": "<64 hex>" } },  // fetched into bin/ at install
  "experimental": {
    "themes": …, "monitors": …, "outputStyles": …,
    "syntaxHighlighting": { "hljsLanguages": [ {"id":"…","remote":"npm:pkg@1.0.0","integrity":"sha384-…"} ] },
    "evals": "./evals"
  }
}
```

**Path rules (enforced):** `K() = string().startsWith("./")`. Command/agent paths additionally must
end `.md` (`ye()`); hooks/mcp/lsp file paths must end `.json` (`V()`). Skill paths may be `"."`.
No `../`, no absolute paths, forward slashes only.

**The critical merge asymmetry** — this trips everyone:
- `skills` **adds to** the `skills/` directory scan.
- `commands`, `agents`, `outputStyles`, `themes`, `workflows`, `monitors` **replace** their
  directory scan. Declare one and the default directory stops being auto-loaded.

**Env-var substitution** (`${…}` inside manifests, hooks, monitors, MCP/LSP config):
| Variable | Value |
|---|---|
| `${CLAUDE_PLUGIN_ROOT}` | absolute path to the installed plugin dir |
| `${CLAUDE_PLUGIN_DATA}` | `~/.claude/plugins/data/<id>/` — survives updates |
| `${CLAUDE_PROJECT_DIR}` | project root |
| `${CLAUDE_ENV_FILE}` | SessionStart hooks only; append `export K=V` to persist env |
| `${user_config.KEY}` | from `userConfig`; also `CLAUDE_PLUGIN_OPTION_<KEY>` env in hooks |

Minimal valid manifest is genuinely one line: `{"name":"hello-world"}`.

### 1.3 `marketplace.json`

Path: `<marketplace-root>/.claude-plugin/marketplace.json`.

```jsonc
{
  "$schema": "https://json.schemastore.org/claude-code-marketplace.json",
  "name": "my-marketplace",   // REQUIRED. no spaces, no "/" "\" ".." ; reserved: inline,
                              // builtin, skills-dir, synced; anti-impersonation regex blocks
                              // names matching /official.*(anthropic|claude)/i etc.
  "version": "1.0.0",
  "description": "…",
  "owner": { "name": "…", "email": "…", "url": "…" },   // REQUIRED
  "metadata": {
    "pluginRoot": "./plugins",   // base dir for BARE source names ("formatter" → ./plugins/formatter)
    "version": "…", "description": "…"
  },
  "forceRemoveDeletedPlugins": false,
  "allowCrossMarketplaceDependenciesOn": ["other-marketplace"],
  "renames": { "old-name": "new-name", "dead-plugin": null },  // append-only migration map
  "plugins": [ /* entries, see below */ ]
}
```

A **plugin entry** is `plugin.json`'s entire schema (all fields `.partial()`) *plus*:

```jsonc
{
  "name": "my-plugin",          // REQUIRED
  "source": "./plugins/my-plugin",   // REQUIRED — string path, or one of the objects below
  "description": "…",
  "version": "1.0.0",
  "category": "productivity",
  "tags": ["…"],
  "strict": true,               // default true = manifest must exist in the plugin folder.
                                // false = this entry IS the manifest (inline it here)
  "headers": { "Authorization": "Bearer …" },   // for archive sources
  "headersHelper": "/abs/path/to/mint-token.sh",
  "relevance": {                // drives "Working with {topic}?" spinner tips + auto-suggest
    "topic": "Stripe",
    "signals": {
      "cli": ["stripe"],
      "hosts": ["api.stripe.com"],
      "filesRead": ["**/*.tf"],
      "manifestDeps": [{ "file": "package\\.json", "pattern": "\"stripe\"" }],
      "cwd": ["services/payments/**"]
    }
  }
}
```

**Plugin source types** (discriminated on `source.source`):

| `source` | Fields | Notes |
|---|---|---|
| *(bare string)* | `"./plugins/x"` | relative to marketplace root; `"."` normalises to `"./"` |
| `github` | `repo` (`owner/repo`), `ref?`, `sha?` | |
| `git` / `url` | `url`, `ref?`, `sha?` | `sha` must be full 40-char lowercase |
| `git-subdir` | `url`, `path`, `ref?`, `sha?` | partial clone `--filter=tree:0`, sparse cone; for monorepos |
| `npm` | `package`, `version?`, `registry?` | |
| `archive` | `url` (https only, no loopback/link-local/metadata hosts), `sha256?` | zip; one wrapper dir stripped |
| `command` | `command`, `timeout?` (≤600 s), `mode?` (`copy`\|`link`) | prints an absolute dir path on stdout |
| `unsupported` | `error?` | **parse-time placeholder only** — never author it |

**Marketplace source types** (how you register the *marketplace itself*):
`url` (direct marketplace.json URL + `headers`/`headersHelper`), `github` (`repo`, `ref`, `path`,
`sparsePaths`, `skipLfs`), `git` (same, with `url`), `npm` (`package`), `file` (`path`),
`directory` (`path`), plus three policy-only sentinels usable *only* in managed settings:
`skills-dir`, `hostPattern`, `pathPattern`, and `settings` (inline marketplace declared in
settings.json).

Real example — `anthropics/claude-plugins-official` `.claude-plugin/marketplace.json`:

```json
{
  "name": "42crunch-api-security-testing",
  "author": { "name": "42Crunch" },
  "category": "security",
  "source": {
    "source": "git-subdir",
    "url": "https://github.com/42Crunch-AI/claude-plugins.git",
    "path": "plugins/api-security-testing",
    "ref": "v1.5.5",
    "sha": "30287f5e3f122a646d1ac5ca3ab96e130c52a3ad"
  },
  "homepage": "https://42crunch.com"
}
```

### 1.4 Marketplace / install machinery on disk

```
~/.claude/plugins/
├── known_marketplaces.json     # { "<name>": { source, installLocation, lastUpdated, autoUpdate? } }
├── installed_plugins.json      # v2: { version:2, plugins: { "p@m": [ {scope, installPath, version,
│                               #        installedAt, lastUpdated, gitCommitSha, resolvedVersion, auto} ] } }
├── config.json                 # { "repositories": {} }
├── blocklist.json              # fetched kill-switch: [{ plugin, added_at, reason, text }]
├── marketplaces/<name>/        # git checkout of each marketplace
├── cache/<marketplace>/<plugin>/<version>/   # the installed plugin tree
├── data/<id>/                  # ${CLAUDE_PLUGIN_DATA}, survives updates
└── plugin-catalog-cache.json
```

Install scopes: `managed` > `user` (`~/.claude/settings.json`) > `project`
(`.claude/settings.json`) > `local` (`.claude/settings.local.json`).

Enable/disable is a settings key, not a file move:

```json
{
  "enabledPlugins": { "superpowers@claude-plugins-official": true },
  "extraKnownMarketplaces": {
    "openai-codex": { "source": { "source": "github", "repo": "openai/codex-plugin-cc" } }
  }
}
```

CLI/TUI: `/plugin`, `/plugin marketplace add <repo|url|path>`, `/plugin install <name>@<market>`,
`claude plugin validate <dir>`, `claude --plugin-dir <dir>` (session-scoped side-load).

### 1.5 Hooks — the actual schema

Claude Code 2.1.252 registers **33** hook events (binary symbol `_y`). The public docs list 9.

```
PreToolUse            PostToolUse           PostToolUseFailure    PostToolBatch
Notification          UserPromptSubmit      UserPromptExpansion
SessionStart          SessionEnd            Stop                  StopFailure
SubagentStart         SubagentStop
PreCompact            PostCompact
PreModelSwitch        PostModelSwitch
PermissionRequest     PermissionDenied
Setup                 TeammateIdle
TaskCreated           TaskCompleted
Elicitation           ElicitationResult
ConfigChange          WorktreeCreate        WorktreeRemove
InstructionsLoaded    CwdChanged            FileChanged
DirectoryAdded        MessageDisplay
```

**Two container shapes.** Plugin `hooks/hooks.json` uses a wrapper; settings files do not.

```jsonc
// plugin hooks/hooks.json
{
  "description": "optional",
  "hooks": { "PreToolUse": [ … ] },
  "modules": ["./hooks/mod.js"]   // max 1; a module exporting register(on)
}
```
(the schema `refine`s that at least one of `hooks` / `modules` is present)

```jsonc
// ~/.claude/settings.json — no wrapper
{ "hooks": { "PreToolUse": [ … ] } }
```

A matcher group is `{ "matcher": "<regex-ish string>", "hooks": [ <handler>, … ] }`.
Matchers are case-sensitive; `"*"` = all; `"Read|Write|Edit"` = alternation;
`"mcp__.*__delete.*"` = regex over MCP tool names.

**Five handler types** (discriminated on `type`):

```jsonc
// 1. command
{ "type": "command",
  "command": "bash ${CLAUDE_PLUGIN_ROOT}/scripts/validate.sh",
  "args": ["--strict"],          // present ⇒ exec form, NO shell, per-element substitution
  "shell": "bash" | "powershell",
  "if": "Bash(git *)",           // permission-rule prefilter; skips spawn on non-match
  "timeout": 60,                 // seconds
  "statusMessage": "Validating…",
  "once": false,
  "async": false,
  "asyncRewake": false }         // background + wake the model on exit 2

// 2. prompt — LLM-evaluated
{ "type": "prompt", "prompt": "Evaluate: $ARGUMENTS", "model": "claude-sonnet-5",
  "timeout": 30, "continueOnBlock": false, "if": …, "statusMessage": …, "once": … }

// 3. agent — agentic verifier
{ "type": "agent", "prompt": "Verify tests ran and passed.", "model": …, "timeout": 60, … }

// 4. http
{ "type": "http", "url": "https://…", "headers": {"Authorization":"Bearer $TOK"},
  "allowedEnvVars": ["TOK"],     // REQUIRED for $VAR interpolation to resolve
  "timeout": …, "if": …, "once": … }

// 5. mcp_tool
{ "type": "mcp_tool", "server": "acp", "tool": "govern",
  "input": { "path": "${tool_input.file_path}" },   // ${…} from hook input JSON
  "timeout": …, "if": …, "once": … }
```

Defaults: command 60 s, prompt 30 s.

**Wire contract.** Input is snake_case JSON on stdin:

```json
{ "session_id": "…", "transcript_path": "…", "cwd": "…",
  "permission_mode": "default|acceptEdits|plan|dontAsk|bypassPermissions",
  "hook_event_name": "PreToolUse",
  "tool_name": "Write", "tool_input": {…}, "tool_use_id": "…" }
```

Output is camelCase JSON on stdout:

```json
{ "continue": true, "suppressOutput": false, "systemMessage": "…",
  "decision": "approve|block", "reason": "…", "stopReason": "…",
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow|deny|ask",
    "permissionDecisionReason": "…",
    "updatedInput": {…},
    "additionalContext": "…"
  } }
```

Exit codes: `0` success (stdout → transcript), `2` blocking error (stderr → fed back to model),
anything else non-blocking error. All matching hooks run **in parallel**; they cannot see each
other's output. Hooks load at session start — editing `hooks.json` requires a restart.

### 1.6 `SKILL.md` frontmatter (authoritative, `.strict()`)

Base fields shared with commands/output-styles, plus skill-only fields:

```yaml
---
# base (also valid on slash-command .md files)
name: my-skill                 # defaults to filename/dirname
description: One-line summary shown in listings and the Skill tool
model: haiku|sonnet|opus|fable|inherit|<full-id>
allowed-tools: Read, Grep      # comma string or YAML list
disallowed-tools: Bash         # cleared on the user's next message
argument-hint: "[file] [mode]"
disable-model-invocation: false  # true ⇒ user-typed slash command only
user-invocable: true             # false ⇒ model-only, hidden from the slash menu
effort: low|medium|high|max|<int>
shell: bash|powershell           # for `!`-command blocks
version: 0.1.0                   # @internal bookkeeping

# skill-only
when_to_use: "Reach for this when …"   # appended to the tool description
paths: ["src/**/*.rs"]                 # skill loads only when the model touches matching files
hooks: { PreToolUse: [ … ] }           # hooks registered WHILE THIS SKILL IS ACTIVE
context: inline | fork                 # fork ⇒ spawns a subagent
agent: my-agent                        # agent type for context: fork
background: true                       # fork runs as a background task
metadata: { any: "author's own use" }
---
```

Anything not in that list is rejected by `.strict()` and telemetered as
`tengu_frontmatter_shadow_unknown_key`. Note `mcpServers`, `lspServers`, `agents`,
`outputStyles`, `themes`, `workflows`, `channels`, `monitors`, `settings`, `userConfig`,
`dependencies`, `displayName`, `author`, `homepage`, `repository`, `license`, `keywords` are
*accepted but marked `@internal`* — a SKILL.md can therefore act as a one-file plugin.

**Silent-failure trap** (from the binary's own doctor text): if the YAML fails to parse, the skill
still loads but with **every field dropped** — name falls back to the directory name, description
to the first body line, and `allowed-tools` / `model` / `disable-model-invocation` silently stop
applying. Nothing warns at normal verbosity.

Progressive disclosure is the design contract: metadata always resident (~100 words) → SKILL.md
body on trigger (target 1 500–2 000 words, hard ceiling ~5 k) → `references/`, `scripts/`,
`assets/` on demand.

```
skill-name/
├── SKILL.md          # required
├── references/       # loaded into context as needed
├── scripts/          # executed WITHOUT being read into context — the token-efficiency lever
└── assets/           # used in output, never loaded
```

### 1.7 Subagents — `agents/*.md` frontmatter (`.strict()`)

```yaml
---
name: code-reviewer            # REQUIRED — how the Agent tool and --agent address it
description: Use this agent when …   # REQUIRED — the dispatch decision is made from this text
model: inherit|haiku|sonnet|opus|<full-id>
tools: [Read, Grep, Bash]      # REPLACES the default set
disallowedTools: [Write]       # subtractive; ignored when `tools` is set
color: blue                    # @internal display colour
effort: low|medium|high|max|<int>
permissionMode: …
mcpServers: {…}
hooks: {…}                     # active only while this agent runs
maxTurns: 20
skills: [tdd, systematic-debugging]   # preloaded
initialPrompt: "…"             # auto-submitted first message when run as main session via --agent
memory: user|project|local
background: true
isolation: worktree            # runs in a temporary git worktree
observer: watchdog-agent       # background observer auto-spawned alongside
observerMessage: "…"
observeSubagents: true
experimental: { cacheTtl: "1h" }
---
System prompt body…
```

Search order: `.claude/agents/*.md` (project) then `~/.claude/agents/*.md` (user), plus plugin
`agents/`.

### 1.8 Slash commands — `commands/*.md`

All frontmatter optional; a bare `.md` file works.

```yaml
---
description: Review Git changes         # ≤60 chars for /help
allowed-tools: Bash(git:*), Read        # Bash takes a command filter
model: haiku|sonnet|opus
argument-hint: "[pr-number]"
disable-model-invocation: true          # blocks the SlashCommand tool; user-typed only
---
Current changes: !`git diff --name-only`
Fix issue #$1 …
```

`$1..$N` positional args, `$ARGUMENTS` for the whole string, `` !`cmd` `` for inline bash output.
Plugin commands namespace as `/<plugin>:<command>`. Locations: `.claude/commands/` (project),
`~/.claude/commands/` (user), plugin `commands/`. Note the loader treats `commands/` and
`skills/` as near-synonyms now — a `skills/commit/SKILL.md` is invocable as `/commit`.

### 1.9 `settings.json` precedence

Highest wins:

1. **Managed** — `managed-settings.json`, MDM policy, or claude.ai server-managed settings
   (macOS: `/Library/Application Support/ClaudeCode/managed-settings.json`)
2. **Command line** — `claude --settings <file>`
3. **Project local** — `.claude/settings.local.json` (gitignored)
4. **Shared project** — `.claude/settings.json` (committed)
5. **User** — `~/.claude/settings.json`

Files are strict JSON — a `//` comment or trailing comma makes the whole file silently ignored.

Plugin-relevant keys: `enabledPlugins`, `extraKnownMarketplaces`, `strictKnownMarketplaces`
(managed only), `blockedMarketplaces` (managed only), `allowManagedHooksOnly`,
`allowManagedPermissionRulesOnly`, `permissions.{allow,ask,deny,defaultMode}`, `hooks`, `env`,
`statusLine`, `sandbox`, `syncClaudeAiSkills`.

A hardening example from `anthropics/claude-code/examples/settings/settings-strict.json`:

```json
{
  "permissions": { "disableBypassPermissionsMode": "disable", "ask": ["Bash"],
                   "deny": ["WebSearch", "WebFetch"] },
  "allowManagedPermissionRulesOnly": true,
  "allowManagedHooksOnly": true,
  "strictKnownMarketplaces": []
}
```

### 1.10 MCP server config

`.mcp.json` (project scope) / plugin `.mcp.json` / `mcpServers` inline. Schema `KY` is a
discriminated union on `type`:

```jsonc
{ "mcpServers": {
  "local":  { "type": "stdio", "command": "node", "args": ["…"], "env": {…},
              "timeout": 300000, "alwaysLoad": false },
  "remote": { "type": "http",  // or "streamable-http", normalised to "http"
              "url": "https://…", "headers": {…}, "headersHelper": "…",
              "oauth": { "clientId": "…", "callbackPort": 1234,
                         "authServerMetadataUrl": "https://…", "scopes": "…" },
              "tools": [ { "name": "x", "permission_policy": "always_allow|always_ask|always_deny" } ],
              "toolPermissions": { "x": "allow|ask|blocked" },
              "discoveryCache": true },
  "sse":    { "type": "sse", "url": "…" },
  "ws":     { "type": "ws",  "url": "…" }
} }
```

Also supported: `sse-ide`, `ws-ide`, `sdk`, `claudeai-proxy` (internal). Scopes:
`local|user|project|dynamic|enterprise|claudeai|managed|agent`. User-scope servers live in
`~/.claude.json` under top-level `mcpServers`; local scope under `projects["<cwd>"].mcpServers`.

---

## 2. OpenAI Codex CLI — and what `.codex-plugin` actually is

Codex CLI `0.148.0` is **a Rust binary**, which makes it the closest structural analogue to Muse
Code and the best model for what a compiled agent's plugin system looks like.

### 2.1 `$CODEX_HOME` layout (default `~/.codex`)

```
~/.codex/
├── config.toml               # the single config file
├── AGENTS.md                 # global instructions
├── AGENTS.override.md        # highest-precedence global override
├── hooks.json                # { "hooks": { "<Event>": [ … ] } }
├── auth.json
├── skills/
│   ├── <name>/SKILL.md
│   └── .system/              # preinstalled: skill-creator, plugin-creator, skill-installer,
│                             #   review-agent, openai-docs, imagegen
├── plugins/
│   └── cache/<marketplace>/<plugin>/<version>/
├── rules/default.rules
├── automations/<name>/automation.toml
├── worktrees/
├── sessions/, archived_sessions/, thread_history_1.sqlite, state_5.sqlite, …
└── log/
```

Cross-agent (vendor-neutral) home:

```
~/.agents/
├── skills/<name>/SKILL.md
├── .skill-lock.json          # v3 lockfile, see below
└── plugins/marketplace.json  # the "personal marketplace"
```

### 2.2 `.codex-plugin/plugin.json`

This is the real thing, and I have it from the shipped `plugin-creator` system skill's
`references/plugin-json-spec.md` plus its `scripts/validate_plugin.py`.

**Allowed key set (validator, exhaustive):**
`id`, `name`, `version`, `description`, `skills`, `apps`, `mcpServers`, `interface`, `author`,
`homepage`, `repository`, `license`, `keywords`.

**Required:** `name`, `version` (strict semver), `description`, `author.name`, and the required
`interface` fields. **`hooks` is explicitly rejected by the manifest validator** — see §2.4.

```jsonc
{
  "name": "plugin-name",
  "version": "1.2.0",
  "description": "Brief plugin description",
  "author": { "name": "…", "email": "…", "url": "https://…" },
  "homepage": "https://…", "repository": "https://…", "license": "MIT",
  "keywords": ["…"],

  "skills": "./skills/",       // ADDS to default discovery
  "mcpServers": "./.mcp.json", // or an inline object
  "apps": "./.app.json",       // only when the file exists

  "interface": {               // the App-Store block; Codex has a UI surface Claude Code doesn't
    "displayName": "Plugin Display Name",
    "shortDescription": "…", "longDescription": "…",
    "developerName": "OpenAI",
    "category": "Productivity",
    "capabilities": ["Interactive", "Read", "Write"],
    "websiteURL": "https://…",           // absolute https:// enforced
    "privacyPolicyURL": "https://…",
    "termsOfServiceURL": "https://…",
    "defaultPrompt": ["…", "…", "…"],    // max 3, each ≤128 chars, ~50 recommended
    "brandColor": "#3B82F6",             // ^#[0-9A-F]{6}$
    "composerIcon": "./assets/icon.png",
    "logo": "./assets/logo.png",
    "logoDark": "./assets/logo-dark.png",
    "screenshots": ["./assets/s1.png"]   // PNG, must live under ./assets/
  }
}
```

Real shipped example — `~/.codex/plugins/cache/openai-curated-remote/github/0.1.11-…/.codex-plugin/plugin.json`
matches this exactly, including `"version": "0.1.11-5f7cd798dc99"` (semver prerelease used as a
content hash).

### 2.3 The interop discovery chain — the single most important finding

Codex's Rust marketplace loader (`core-plugins/src/marketplace.rs`) contains these adjacent
string literals, which are the ordered candidate lists:

```
.codex-plugin/plugin.json    .claude-plugin/plugin.json    .cursor-plugin/plugin.json
```
```
.agents/plugins/marketplace.json   .agents/plugins/api_marketplace.json
.claude-plugin/marketplace.json    .cursor-plugin/marketplace.json
```

**Codex reads Claude Code's and Cursor's plugin manifests natively.** This is the convention Muse
Code follows with `.muse-plugin/`, and it is the convention an oh-my-musecode framework should ship
against.

Codex also carries a whole `codex-external-agent-migration` crate (`external-agent-migration/src/source_cla.rs`)
with telemetry events `codex.external_agent_config.detect` / `.import`. It reads, from a Claude
Code install: `CLAUDE.md` → `AGENTS.md`, `.claude/settings.json` + `settings.local.json` (hooks,
`permissionMode`, `sandbox`/`sandbox_mode`→`workspace-write`, `shell_environment_policy`),
`~/.claude/plugins/known_marketplaces.json`, `extraKnownMarketplaces`, `enabledPlugins`,
`.claude/skills`, `.claude/agents`, `.mcp.json`, `~/.claude.json` `mcpServers`
(+ `enabledMcpjsonServers` / `disabledMcpjsonServers`), memory, and even session transcripts
(`<EXTERNAL SESSION IMPORTED>`). Cursor sources cover `.cursorrules`, `.cursor/`,
`Library/Application Support/Claude`, `cli-config.json`.

### 2.4 Codex hooks

Events (Rust enum, verbatim ordering from the binary — **11**):

```
PreToolUse  PermissionRequest  PostToolUse  PreCompact  PostCompact
SessionStart  SessionEnd  UserPromptSubmit  SubagentStart  SubagentStop  Stop
```

Handler types: `command`, `prompt`, `agent`, `mcp_tool` (Rust module
`codex_hooks::engine::mcp_runner`, span `codex.hooks.mcp_tool`). Handler fields:
`if`, `command`, `async`, `asyncRewake`, `shell`, `timeout`, `timeoutSec`, `statusMessage`,
`matcher`. **Byte-for-byte the same vocabulary as Claude Code**, minus `http`.

The binary embeds JSON Schemas titled `pre-tool-use.command.input`, `pre-tool-use.command.output`,
`permission-request.command.output`, `pre-compact.command.input`, `post-compact.command.input`, …
The input schema:

```json
{ "properties": {
    "agent_id": {"type":"string"}, "agent_type": {"type":"string"},
    "cwd": {"type":"string"}, "hook_event_name": {"const":"PreToolUse"},
    "model": {"type":"string"},
    "permission_mode": {"enum":["default","acceptEdits","plan","dontAsk","bypassPermissions"]},
    "session_id": {"type":"string"}, "tool_input": true, "tool_name": {"type":"string"},
    "tool_use_id": {"type":"string"}, "transcript_path": {"type":["string","null"]},
    "turn_id": {"description":"Codex extension: expose the active turn id to internal turn-scoped hooks."} },
  "required": ["cwd","hook_event_name","model","permission_mode","session_id",
               "tool_input","tool_name","tool_use_id","transcript_path","turn_id"] }
```

The output schema reproduces Claude Code's `hookSpecificOutput` exactly:
`PreToolUseDecisionWire = approve|block`, `PreToolUsePermissionDecisionWire = allow|deny|ask`,
plus `continue`, `reason`, `stopReason`, `suppressOutput`, `systemMessage`, `additionalContext`,
`updatedInput`. The **only** documented divergence is the `turn_id` extension field.

**Where Codex hooks live:** `$CODEX_HOME/hooks.json` (wrapper form), project hooks config, and —
crucially — Codex also parses `settings.json` / `settings.local.json` with a `disableAllHooks`
key, i.e. it can consume Claude Code's settings-shaped hook block directly.

`config.toml` gates and trusts them:

```toml
[features]
hooks = true

[hooks.state."/Users/cph/.codex/hooks.json:session_start:0:0"]
trusted_hash = "sha256:438cadc56a56fadd4759e37f350a46eb2be7a9b662952db087fea000d9e8261c"
```

That is **trust-on-first-use with content pinning** — a hook whose script content changes must be
re-approved. Claude Code has no direct equivalent; Muse Code's `muse plugins approve` is the same
idea.

### 2.5 Codex marketplace

`~/.agents/plugins/marketplace.json` (personal) or `<repo>/.agents/plugins/marketplace.json`
(team). Discovered implicitly at the personal path; every other path needs
`codex plugin marketplace add <path-or-repo>`.

```json
{
  "name": "personal",
  "interface": { "displayName": "Personal" },
  "plugins": [
    {
      "name": "plugin-name",
      "source": { "source": "local", "path": "./plugins/plugin-name" },
      "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
      "category": "Productivity"
    }
  ]
}
```

- `source.source`: `local` | `git` (with `url`, `ref`, `sparse_paths`) — enum in the binary is
  `MarketplaceSourceType { git, local }`.
- `policy.installation`: `NOT_AVAILABLE` | `AVAILABLE` | `INSTALLED_BY_DEFAULT`
- `policy.authentication`: `ON_INSTALL` | `ON_USE`
- `policy.products`: optional product gating override
- Array **order is render order** in the Codex UI.
- `displayName` belongs in the top-level `interface`, never in a plugin entry.

CLI:
```bash
codex plugin add <name>            # install from a configured marketplace snapshot
codex plugin list
codex plugin remove <name>
codex plugin marketplace add <org/repo | url | ./path>  [--ref <r>] [--sparse <path>]
codex plugin marketplace list | upgrade [<name>] | remove <name>
```
Enablement lands in `config.toml`:
```toml
[plugins."github@openai-curated"]
enabled = true
```

Enterprise policy via `requirements.toml`:
```toml
[plugins]
allowed_marketplaces = ["platform-team-plugins", "openai-official"]
[mcp_servers]
allowlist = [{ name = "github-mcp", identity = "openai/github-mcp-server" }]
```

### 2.6 Codex skills

Discovery: `$CODEX_HOME/skills/`, `~/.agents/skills/`, repo `.agents/skills/`, plus
`.claude/skills` and `.codex/skills` for migration. Config struct
`SkillsConfig { bundled, include_instructions, config }` and `SkillConfig { path, name }`.

Frontmatter is deliberately minimal — `name`, `description`, optional `metadata`:

```yaml
---
name: herdr
description: "Control Herdr, a terminal multiplexer for coding agents. Use only when …"
metadata:
  short-description: Install curated skills from openai/skills or other repos
---
```

UI/invocation metadata is a **sidecar**, not frontmatter — `<skill>/agents/openai.yaml`:

```yaml
interface:
  display_name: "Playwright CLI Skill"
  short_description: "Automate real browsers from the terminal"
  icon_small: "./assets/playwright-small.svg"
  icon_large: "./assets/playwright.png"
  default_prompt: "Automate this browser workflow with Playwright …"
```

That sidecar pattern is worth stealing: it keeps the portable SKILL.md vendor-neutral while
letting each harness carry its own presentation layer in a file the others ignore.

`~/.agents/.skill-lock.json` (v3) is a real lockfile:

```json
{ "version": 3,
  "skills": {
    "grill-me": {
      "source": "mattpocock/skills", "sourceType": "github",
      "sourceUrl": "https://github.com/mattpocock/skills.git",
      "skillPath": "skills/productivity/grill-me/SKILL.md",
      "skillFolderHash": "8320e7b87f7b208f50ce165b1dd43d1e93c8e801",
      "pluginName": "mattpocock-skills",
      "installedAt": "2026-04-30T14:26:18.119Z",
      "updatedAt": "2026-06-18T17:02:40.814Z"
    } },
  "dismissed": [], "lastSelectedAgents": [] }
```

### 2.7 `AGENTS.md` and `config.toml`

Resolution order (highest first):
`~/.codex/AGENTS.override.md` → `~/.codex/AGENTS.md` → `<git-root>/AGENTS.override.md` →
`<git-root>/AGENTS.md` → nested per-directory `AGENTS.md`. Fallbacks configurable via
`project_doc_fallback_filenames`; size capped by `project_doc_max_bytes`.

`config.toml` precedence: CLI `-c key=value` → `--profile` → `.codex/config.toml` nearest cwd →
`~/.codex/config.toml` → `/etc/codex/config.toml` → defaults.

Notable tables seen in the binary: `[features]`, `[hooks.state.…]`, `[plugins."<p>@<m>"]`,
`[projects."<path>"] trust_level`, `[mcp_servers.…]`, `[sandbox_workspace_write]`
(`writable_roots`, `network_access`, `exclude_tmpdir_env_var`, `exclude_slash_tmp`),
`[shell_environment_policy]` (`inherit`, `exclude`, `include_only`, `filters`), `[memories]`,
`[history]`, `[otel]`, `[tools]`, `[goals]`, `[notifications]`, `[windows]`.

---

## 3. Muse Code (Meta) — what it accepts

Released **2026-08-05**, backed by Muse Spark 1.2; a compiled terminal agent installed by
`curl -fsSL https://dev.meta.ai/install.sh | bash`. Confidence flags below: **[V]** = confirmed by
vendor docs, **[R]** = third-party reverse-engineering, **[?]** = single-source.

### 3.1 Configuration

| Thing | Path | Conf |
|---|---|---|
| User settings | `~/.config/muse/settings.json`, requires `"schema_version": 1` | [R] |
| Runtime toggles | `runtime_capabilities` block inside that settings.json (skill recall, goal tracking, verification, background observers) | [R] |
| MCP servers | inline in `settings.json` — `stdio` and `streamable_http`. **No `.mcp.json` at repo root.** | [R] |
| Project instructions | `AGENTS.md`, seeded by `muse init`; **falls back to `CLAUDE.md`**. No `MUSE.md`. | [V] |
| Plans | `.agents/plans/` | [V] |
| Worktrees | `.muse/worktrees/` (one per subagent, detached HEAD) | [V] |
| Sessions | `~/.local/share/muse/sessions/` (XDG) | [V] |
| Sandbox read-only set | `.git`, `.muse`, `.agents` | [R] |

### 3.2 Skills

- Primary: `.agents/skills/` (repo), plus a user-level skills dir. [R]
- **Scans `.claude/skills` and `.codex/skills` automatically**, repo-local and user-level. [V]
- One-command importer: `muse skills import --from claude` / `--from codex`. [V]
- Built-in skill ids: `create-skill`, `doctor`, `git`, `grill`, `grill-and-record`, `import`,
  `manage-settings`, `plan`, `read-session`, `taste`. Surfaced as `/plan`, `/grilling`,
  `/grill-with-docs`, `/taste`, `/goal`, `/model`, `/effort`. [V]
- Skills are **explicit-invocation only** — the body loads only for the turn you invoke it on.
  This is a meaningful divergence from Claude Code's description-matched auto-trigger. [V]

### 3.3 Plugins and hooks

- Manifest: **`.muse-plugin/plugin.json`**. [R]
- Gated: `MUSE_EXPERIMENTAL_PLUGINS=1` on the *management CLI only* — once approved, hooks fire in
  ordinary unflagged sessions. [R]
- Install/trust:
  ```bash
  MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins install ./my-plugin --scope user
  MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins approve my-plugin
  ```
- **Hooks are declared as capabilities of a plugin**, not as a standalone file. The documented
  `.muse/hooks.json` + `muse hooks` CLI path is *silently ignored* / errors. [R]
- Meta ships a built-in plugin-authoring skill containing `native-plugin-contract.md` — the
  actual wired spec. Read that file before building anything. [R]
- Working events observed: `PreToolUse`, `PermissionRequest`, `PostToolUse`, `Stop`. [R]
  Broader claimed coverage: session start, prompt submission, tool use, permission requests,
  model calls, context compaction, subagent start/stop, session stop. [?]
- **Wire contract: "Claude Code's hook schema — not similar, the same."** snake_case input
  (`hook_event_name`, `tool_name`, `tool_input`, `session_id`, `permission_mode`, `cwd`),
  camelCase `hookSpecificOutput` with `permissionDecision: deny|ask|allow` +
  `permissionDecisionReason` back. [R]
- Hooks run with a **cleared environment** (deliberate credential-leak defence) — so a plugin
  must carry its own credential path rather than read `$FOO` from the session. [R]

### 3.4 Subagents and audit

Native tools: `subagent_spawn`, `subagent_status`, `subagent_send_message`, `subagent_cancel`,
`subagent_wait`, `subagent_read_result`. Nesting is one level deep. Steering slash commands:
`/agent`, `/subagents`, `/agent-note`, `/agent-followup`, `/agent-interrupt`, `/agent-stop`,
`/agent-resume`, `/agent-reopen`, `/agent-close`. [V]

Event log: "intent before effect" — seven-record chain per side effect (proposed, accepted,
approval review requested, decision applied, side-effect intent with policy decision, effect
started, terminal). `muse replay` walks it; `muse export --redacted` produces a byte-deterministic
JSON document with `export_schema_version: 1`. [V] **This is the surface no other agent has**, and
it is the natural place for a framework to hang verification and receipts.

---

## 4. Compatibility table

### 4.1 Format-by-format

| Concern | Claude Code 2.1.252 | Codex CLI 0.148.0 | Muse Code (Aug 2026) |
|---|---|---|---|
| Plugin manifest | `.claude-plugin/plugin.json` | `.codex-plugin/plugin.json`, falls back to `.claude-plugin/`, `.cursor-plugin/` | `.muse-plugin/plugin.json` |
| Required manifest fields | `name` | `name`, `version` (strict semver), `description`, `author.name`, `interface.*` | unknown — read `native-plugin-contract.md` |
| Marketplace manifest | `.claude-plugin/marketplace.json` | `.agents/plugins/marketplace.json` → `api_marketplace.json` → `.claude-plugin/` → `.cursor-plugin/` | none published |
| Marketplace root keys | `name`, `owner`, `plugins[]`, `metadata.pluginRoot`, `renames` | `name`, `interface.displayName`, `plugins[]` | — |
| Plugin entry policy | `strict` (manifest required) | `policy.installation` / `policy.authentication` / `category` | approval via `muse plugins approve` |
| Plugin source types | 8 (`github`, `git`, `url`, `git-subdir`, `npm`, `archive`, `command`, bare path) | 2 (`local`, `git`) | local dir |
| Skills | `skills/<n>/SKILL.md`; auto-triggered on `description` | same file format; `.agents/skills`, `$CODEX_HOME/skills` | same file format; `.agents/skills`; **explicit invocation only** |
| Reads *others'* skills | `.claude/skills` only | `.claude/skills`, `.codex/skills`, `.agents/skills` | `.claude/skills`, `.codex/skills` + `muse skills import` |
| Skill frontmatter | ~25 fields, `.strict()` | `name`, `description`, `metadata` | `name`, `description` (assumed) |
| Skill UI metadata | in frontmatter (`displayName` @internal) | sidecar `agents/openai.yaml` | unknown |
| Subagents | `agents/*.md`, ~22 frontmatter fields | skill-embedded / `agents/` | native `subagent_*` tools, no `.md` format published |
| Slash commands | `commands/*.md` + skills-as-commands | skills only | skills only (`/plan`, `/taste`, …) |
| Hook events | **33** | **11** | 4 confirmed, ~9 claimed |
| Hook handler types | `command`, `prompt`, `agent`, `http`, `mcp_tool` | `command`, `prompt`, `agent`, `mcp_tool` | `command` (confirmed) |
| Hook wire format | snake_case in / camelCase `hookSpecificOutput` out | **identical**, + `turn_id` | **identical** |
| Hook container | plugin `hooks/hooks.json` = `{hooks:{…}}`; settings = `{hooks:{…}}` | `$CODEX_HOME/hooks.json` = `{hooks:{…}}`; also reads `settings.json` | plugin capability declaration |
| Hook trust model | settings precedence + `allowManagedHooksOnly` | **`trusted_hash` content pinning in config.toml** | **`muse plugins approve`** |
| Hook env | inherits + `${CLAUDE_PLUGIN_ROOT}` etc. | inherits, `[shell_environment_policy]` | **cleared env** |
| Settings file | `~/.claude/settings.json` (JSON) + 4-level cascade | `~/.codex/config.toml` (TOML) + 5-level cascade | `~/.config/muse/settings.json`, `schema_version: 1` |
| Project instructions | `CLAUDE.md` | `AGENTS.md` (+ `AGENTS.override.md`) | `AGENTS.md`, falls back to `CLAUDE.md` |
| MCP config | `.mcp.json` / `~/.claude.json` / plugin manifest | `[mcp_servers]` in config.toml / `.mcp.json` in plugin | inline in `settings.json` only |
| Vendor-neutral dir | — | `~/.agents/`, `<repo>/.agents/` | `<repo>/.agents/`, `.muse/` |

### 4.2 What Muse Code appears to accept, ranked by confidence

| Artifact | Muse accepts? | Evidence |
|---|---|---|
| `SKILL.md` (name + description + body) | **Yes** | scans `.claude/skills`, `.codex/skills`; `muse skills import` |
| `AGENTS.md` | **Yes** | vendor docs, `muse init` |
| `CLAUDE.md` | **Yes, as fallback** | vendor docs |
| Claude-Code hook **wire contract** | **Yes, exactly** | ACP integration writeup: "not similar, the same" |
| `.agents/` layout (skills, plans) | **Yes** | vendor docs; sandbox treats `.agents` as read-only |
| `.muse-plugin/plugin.json` | **Yes** (its own) | reverse-engineered; gated by `MUSE_EXPERIMENTAL_PLUGINS` |
| `.claude-plugin/plugin.json` | **Unconfirmed** | Codex does this; no evidence Muse does. **Ship both dirs.** |
| `.claude-plugin/marketplace.json` | **No evidence** | no Muse marketplace exists yet |
| `commands/*.md` slash commands | **No** | Muse's slash commands are skills |
| `agents/*.md` subagent files | **No** | Muse spawns via `subagent_spawn` tools |
| `.mcp.json` at repo root | **No** | MCP lives in `settings.json` |
| Claude `settings.json` | **No** | different path *and* different schema (`schema_version`, `runtime_capabilities`) |

---

## 5. The community layer: what "oh-my-*" projects actually ship

### 5.1 obra/superpowers — the multi-harness gold standard

Locally installed at `~/.claude/plugins/cache/claude-plugins-official/superpowers/6.3.0`. ~94 k
stars; accepted into Anthropic's official marketplace. It ships **fourteen skills and nine harness
adapters from one tree**:

```
superpowers/
├── skills/                     # 14 skills — the ONLY substantive content
├── hooks/
│   ├── hooks.json              # Claude Code shape:  { hooks: { SessionStart: [{matcher, hooks:[…]}] } }
│   ├── hooks-cursor.json       # Cursor shape:       { version:1, hooks: { sessionStart: [{command}] } }
│   └── run-hook.cmd            # one entry point, dispatched by argv
├── .claude-plugin/plugin.json
├── .codex-plugin/plugin.json   # + skills:"./skills/", hooks:{}, full interface{} block
├── .cursor-plugin/plugin.json  # + skills:"./skills/", hooks:"./hooks/hooks-cursor.json"
├── .kimi-plugin/plugin.json    # + sessionStart:{skill:"using-superpowers"}, skillInstructions:"…"
├── .devin-plugin/plugin.json   # metadata only
├── .hermes-plugin/{plugin.yaml, __init__.py}   # Python bootstrap module
├── .opencode/plugins/superpowers.js
├── .pi/extensions/superpowers.ts
├── .agents/plugins/marketplace.json            # Codex personal-marketplace entry
├── gemini-extension.json                       # { name, description, version, contextFileName:"GEMINI.md" }
├── package.json                                # npm: main = .opencode plugin; "pi": {extensions, skills}
├── AGENTS.md  CLAUDE.md  GEMINI.md
└── scripts/{sync-to-codex-plugin.sh, package-codex-plugin.sh, bump-version.sh, lint-shell.sh}
```

Three details worth copying outright:

1. **One skills tree, N thin manifests.** The manifests are 20–40 lines each. Everything real lives
   once, in `skills/`.
2. **`sync-to-codex-plugin.sh` / `bump-version.sh`.** The adapters are *generated and version-bumped
   by script*, not hand-maintained. Nine harnesses is only tractable because drift is impossible.
3. **`.kimi-plugin/plugin.json` carries a `skillInstructions` string** — a ~1 500-char tool-name
   translation table ("when a Superpowers skill says `TodoWrite`, use Kimi's `TodoList`"; "`Task
   tool (general-purpose)` → Kimi's `Agent` with `subagent_type: coder`"). That is the mechanism
   for making one prose skill portable across harnesses with different tool vocabularies. It is the
   single most transferable idea in the whole survey.

### 5.2 oh-my-claudecode (Yeachan-Heo) — the traction case study

First commit 2026-01-09; **36.2 k stars / 3.3 k forks by June 2026**, ~38.9 k by the time of this
survey; 232 releases in five months.

Ships: 19–32 specialized agents with tier variants, 40+ skills, slash commands (`/team`,
`/autopilot`, `/execute`, `/ralph`, `/deep-interview`), and an `omc` npm binary.

Dual install path — this is the pattern:
```bash
/plugin marketplace add https://github.com/Yeachan-Heo/oh-my-claudecode
/plugin install oh-my-claudecode
# or
npm i -g oh-my-claude-sisyphus@latest && omc setup
```

Layout:
```
agents/  skills/  commands/  src/
.omc/                       # runtime state: sessions/*.json, state/agent-replay-*.jsonl,
                            #   artifacts/ask/, plans/  — gitignored EXCEPT .omc/skills/**
.claude/omc.jsonc           # project config (named autopilot workflows)
~/.config/claude-omc/config.jsonc   # user defaults
```

Architecture: a TypeScript layer over Claude Code's native hooks plus the experimental
`CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` flag; two runtimes — in-session agent teams, and **tmux
panes running real `claude`, `codex`, `gemini`, `antigravity`, `grok`, `cursor-agent` processes**.
Model routing (Haiku for simple, Opus for complex) is the headline cost claim.

Why it worked: a single zero-config entry point (`/autopilot "describe the task"`), pre-built
specialization, and relentless absorption of community patterns.

Why it's fragile — and these are the lessons: single maintainer, 232 breaking-ish releases in five
months (`swarm` removed at v4.1.7, MCP servers removed at v4.4.0), brand/npm-package name mismatch
("oh-my-claudecode" vs `oh-my-claude-sisyphus`), and **total dependence on one experimental vendor
flag**. If Anthropic ships native teams, the framework's core value evaporates.

### 5.3 The rest of the "oh-my-*" field

| Project | What it is | Signal |
|---|---|---|
| `Yeachan-Heo/oh-my-claudecode` | multi-agent orchestration; plugin + npm | 36–39 k ★ — the winner |
| `zephyrpersonal/oh-my-claude-code` | agent delegation, explicitly "inspired by oh-my-opencode" | small |
| `huangdijia/oh-my-claude-code-plugins` | a marketplace repo, not a framework | small |
| `npow/oh-my-claude` | statusline framework (context, spend, CI signals) | narrow but useful niche |
| `LigphiDonk/Oh-my--paper` | research-lab pipeline plugin | niche |
| `ClementATH/oh-my-claudecode` | fork/clone | — |

**Nobody owns the name.** No `oh-my-musecode`, no `awesome-muse-code`, no Muse marketplace of any
kind existed at the time of this survey. The name is unclaimed and the category is empty.

### 5.4 Directories and aggregators

| Repo | Claim |
|---|---|
| `affaan-m/everything-claude-code` | 141 k ★ aggregator firehose |
| `hesreallyhim/awesome-claude-code` | 36.8 k ★ canonical hand-curated list |
| `quemsah/awesome-claude-plugins` | automated adoption metrics; **15 134 plugin repos indexed by 2026-05-01**, up from ~4 000 a year earlier |
| `rohitg00/awesome-claude-code-toolkit` | 135 agents / 35 skills / 42 commands / 176+ plugins / 20 hooks |
| `ComposioHQ/awesome-claude-plugins`, `Chat2AnyLLM/awesome-claude-plugins`, `jmanhype/awesome-claude-code` | curated lists |
| `hashgraph-online/awesome-codex-plugins` | the Codex-side equivalent |
| `claudemarketplaces.com`, `claude-plugins.dev`, `aitmpl.com/plugins` | web directories |

Codex's own marketplace launched 2026-03-26 and reached 12 official + 40 community plugins within
a month — two orders of magnitude smaller than Claude Code's. **Being early in a compiled-agent
ecosystem is cheap; being early in Claude Code's is impossible.**

---

## 6. Sources

Primary artifacts (this machine):
- `/Users/cph/.local/share/claude/versions/2.1.252` — Claude Code binary, embedded Zod schemas
- `/Users/cph/.local/bin/codex` — Codex CLI 0.148.0 binary, Serde fields + embedded JSON Schemas
- `~/.claude/plugins/{known_marketplaces,installed_plugins,config,blocklist}.json`
- `~/.claude/plugins/marketplaces/claude-plugins-official/.claude-plugin/marketplace.json`
- `~/.claude/plugins/marketplaces/claude-plugins-official/plugins/plugin-dev/skills/**`
- `~/.claude/plugins/cache/claude-plugins-official/superpowers/6.3.0/**`
- `~/.codex/skills/.system/plugin-creator/{SKILL.md,references/plugin-json-spec.md,scripts/validate_plugin.py}`
- `~/.codex/plugins/cache/openai-curated-remote/github/0.1.11-5f7cd798dc99/.codex-plugin/plugin.json`
- `~/.agents/.skill-lock.json`, `~/.codex/{config.toml,hooks.json}`

Documentation:
- https://code.claude.com/docs/en/plugins-reference
- https://code.claude.com/docs/en/settings
- https://code.claude.com/docs/en/hooks
- https://github.com/anthropics/claude-code/blob/main/plugins/README.md
- https://developers.openai.com/codex/guides/agents-md
- https://developers.openai.com/codex/config-reference
- https://developer.meta.com/ai/products/muse-code/
- https://developer.meta.com/ai/resources/blog/build-with-muse-code/
- https://musecodes.io/docs/

Third-party analysis:
- https://agenticcontrolplane.com/blog/muse-code-acp-integration — Muse `.muse-plugin/plugin.json`, hook wire contract
- https://agenticcontrolplane.com/integrations/muse-code — install/approve flow, hook→endpoint map
- https://www.digitalapplied.com/blog/muse-code-deep-dive-fan-out-event-log-skills — event log, worktrees, `.agents/skills`
- https://codersera.com/blog/muse-code-complete-guide-2026/ — `~/.config/muse/settings.json`, `schema_version`, importer
- https://codex.danielvaughan.com/2026/04/24/codex-cli-plugin-marketplace-building-distributing-extending/
- https://codex.danielvaughan.com/2026/05/13/codex-cli-agent-migration-system-import-claude-code-sessions-skills-config/
- https://rywalker.com/research/oh-my-claudecode — adoption numbers, criticisms
- https://github.com/Yeachan-Heo/oh-my-claudecode
- https://github.com/quemsah/awesome-claude-plugins — 15 134 indexed plugin repos
