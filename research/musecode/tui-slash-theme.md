# Meta Muse Code ("TBH") 1.0.1-R2006.1 — TUI dimension
## Slash commands, keybindings, theming, status line, banner, reasoning display, login/payment

Crate: `fbcode/musecode/build/src/crates/tui` (Rust crate name `tbh_tui`).
Binary: `/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`

Everything below is either **PROVEN** (reproduced by running the binary in a pseudo-terminal, or read
verbatim out of the binary at a named offset) or explicitly marked **INFERRED**.

---

## 0. Method — how the TUI was driven

`muse` refuses to start without a real TTY (`Device not configured (os error 6)`) *and* it
interrogates the terminal at startup, aborting if the replies never come
(`cursor position could not be read`). A raw `pty.fork()` is not enough: you must answer its probes.

Startup probe sequence captured verbatim from a bare pty (first 260 bytes the process wrote):

```
\x1b]10;?\x07                      OSC 10  – query default foreground
\x1b]11;?\x07                      OSC 11  – query default background
\x1b]4;0;?\x07 … \x1b]4;15;?\x07   OSC 4   – query all 16 ANSI palette slots
\x1b[?2004h                        enable bracketed paste
\x1b[?1004h                        enable focus reporting
\x1b[?25l                          hide cursor
\x1b[>3u                           push kitty keyboard protocol flags
\x1b[?u                            query kitty keyboard support
\x1b[c                             DA1
\x1b[6n \x1b[6n                    two cursor-position reports
```

The harness used for every runtime capture in this report lives at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/sandbox/tui/drive.py`
(pty + a minimal terminal responder that answers OSC 10/11/4, DA1, CPR and the kitty query) and
`…/sandbox/tui/render.py` (a small VT100 screen model so the frames can be diffed as text).
All runs used `--provider echo` and a sandboxed `HOME`/`XDG_CONFIG_HOME` under the scratchpad.

Two facts fall straight out of the probe list and matter for the rest of the report:

* the TUI **reads the terminal's background colour and its whole 16-colour palette at startup** —
  that feeds both light/dark theme selection and the `Dynamic` theme;
* the TUI speaks the **kitty keyboard protocol** when the terminal advertises it (`CSI > 3 u` push,
  `CSI < u` pop on exit).

---

## 1. Slash commands — the complete list

### 1.1 The authoritative built-in table, recovered verbatim from the binary

The whole slash table (name, aliases, argument hint, description) is one concatenated literal run in
`__TEXT,__const` at file offset **`0xb806baf`** (a second copy at `0xb848486`). Verbatim:

```
/skills/skill/login
  Log in with Meta account or using an API key
/logout   Log out, forget the saved login, and exit
/clear    Start a fresh session and wipe the scrollback
/new      Start a fresh session, keep the scrollback
[<name>] /name        Show or rename this session
[--last] /resume      Resume an earlier Muse Code session
[prompt] /fork        Branch this session from the latest message
/side     Start a side conversation  /btw
/init     Explore the workspace and create or improve AGENTS.md
<question> /deep-research  Research a question across sources with cross-checking and citations
/subagents  View running and past subagents
/model    Choose model  /models
/settings Open local settings  /config
/keymap   Show keyboard shortcuts
/help     Show help
/theme    Choose color theme
[import] /rules   Show which md files govern this session  /memory
/compact  Summarize the conversation to free up context
[transcript|trajectory] /export  Save the conversation, or the full session log
/copy     Copy the last response to the clipboard
/recap    Show a recap of recent session activity
[import|reload|manage|diagnostics|use <id-or-path> [prompt]]  (/skills) Browse and use skills
[list|inspect <id>|diagnostics|install <path|plugin@marketplace>|enable <id>|disable <id>|update <id>|remove <id>|marketplace ...]
          /plugins  Manage plugins
<id-or-path> [prompt]  (/skill) Invoke a skill
[<none|minimal|low|medium|high|xhigh|ultra>] /effort  Set the model's effort level
[<objective>|edit <objective>|clear|pause|resume] /goal  Start or manage continuous work toward a goal
[note] /feedback  Send quick feedback to the team
[status|debug] /voice  Manage voice input
/exit     Quit when idle  /quit
/stop     Stop all background tasks
/status   Show current session status
/usage    Show session usage  /cost  /context
/upgrade  Show your subscription plan
/permissions  choose what Muse Code is allowed to do
/tasks    View and manage background tasks and terminals  /ts  /ps
/workflows  Browse workflow runs
```

That is **37 built-in commands** with **10 aliases**. Reconstructed as a table
(canonical name → aliases → argument hint → description):

| # | Command | Aliases | Args | Description |
|---|---------|---------|------|-------------|
| 1 | `/login` | — | — | Log in with Meta account or using an API key |
| 2 | `/logout` | — | — | Log out, forget the saved login, and exit |
| 3 | `/clear` | — | — | Start a fresh session and wipe the scrollback |
| 4 | `/new` | — | — | Start a fresh session, keep the scrollback |
| 5 | `/name` | — | `[<name>]` | Show or rename this session |
| 6 | `/resume` | — | `[--last]` | Resume an earlier Muse Code session |
| 7 | `/fork` | — | `[prompt]` | Branch this session from the latest message |
| 8 | `/side` | `/btw` | `[prompt]` | Start a side conversation |
| 9 | `/init` | — | — | Explore the workspace and create or improve AGENTS.md |
| 10 | `/deep-research` | — | `<question>` | Research a question across sources with cross-checking and citations |
| 11 | `/subagents` | — | — | View running and past subagents |
| 12 | `/model` | `/models` | — | Choose model |
| 13 | `/settings` | `/config` | — | Open local settings |
| 14 | `/keymap` | — | — | Show keyboard shortcuts |
| 15 | `/help` | — | — | Show help |
| 16 | `/theme` | — | — | Choose color theme |
| 17 | `/rules` | `/memory` | `[import]` | Show which md files govern this session |
| 18 | `/compact` | — | — | Summarize the conversation to free up context |
| 19 | `/export` | — | `[transcript\|trajectory]` | Save the conversation, or the full session log |
| 20 | `/copy` | — | — | Copy the last response to the clipboard |
| 21 | `/recap` | — | — | Show a recap of recent session activity |
| 22 | `/skills` | — | `[import\|reload\|manage\|diagnostics\|use <id-or-path> [prompt]]` | Browse and use skills |
| 23 | `/plugins` | — | `[list\|inspect <id>\|diagnostics\|install <path\|plugin@marketplace>\|enable <id>\|disable <id>\|update <id>\|remove <id>\|marketplace ...]` | Manage plugins |
| 24 | `/skill` | — | `<id-or-path> [prompt]` | Invoke a skill |
| 25 | `/effort` | — | `[<none\|minimal\|low\|medium\|high\|xhigh\|ultra>]` | Set the model's effort level |
| 26 | `/goal` | — | `[<objective>\|edit <objective>\|clear\|pause\|resume]` | Start or manage continuous work toward a goal |
| 27 | `/feedback` | — | `[note]` | Send quick feedback to the team |
| 28 | `/voice` | — | `[status\|debug]` | Manage voice input |
| 29 | `/exit` | `/quit` | — | Quit when idle |
| 30 | `/stop` | — | — | Stop all background tasks |
| 31 | `/status` | — | — | Show current session status |
| 32 | `/usage` | `/cost`, `/context` | — | Show session usage |
| 33 | `/upgrade` | — | — | Show your subscription plan |
| 34 | `/permissions` | — | — | choose what Muse Code is allowed to do |
| 35 | `/tasks` | `/ts`, `/ps` | — | View and manage background tasks and terminals |
| 36 | `/workflows` | — | — | Browse workflow runs |
| 37 | `/import` * | — | `<session-id-or-path>` | (see §1.4 — bundled *skill*, not a built-in) |

\* row 37 is not part of the built-in table; it is listed here only because `/help` shows it in the
same list. The 36 rows above `/import` plus `/login` = the 37-entry built-in vocabulary.

Runtime confirmation of aliases (the picker renders `canonical (alias)` when you type the alias):

```
$ /p   → /tasks (/ps)          View and manage background tasks and terminals
$ /con → /settings (/config)   Open local settings
        /usage (/context)      Show session usage
$ /c   → /usage (/cost)        Show session usage
$ /b   → /side (/btw)          Start a side conversation
$ /m   → /rules (/memory)      Show which md files govern this session
```

### 1.2 The telemetry vocabulary and `TELEMETRY_DROPPED_SLASH_COMMAND_NAMES`

At offset **`0xba31c63`** the binary carries a *second*, machine-readable list plus its own doc
comment (verbatim):

```
 /login
/logout/clear/new/resume/fork/side/init/deep-research/subagents/model/settings/keymap/help/theme
/rules/compact/export/copy/recap/skills/plugins/skill/effort/goal/feedback/voice/exit/quit/stop
/status/usage/upgrade/permissions/tasks/workflows
custom
Built-in slash-command names pass through verbatim from the durable record's closed vocabulary
(spec 8403 owns it; machine-readable as tbh_agent::command_invoked::BUILTIN_SLASH_COMMAND_NAMES
minus TELEMETRY_DROPPED_SLASH_COMMAND_NAMES since #13706, TUI-table parity pinned in
crates/tui/src/slash.rs); plugin-command and skill-shortcut dispatches land in the literal
`custom` bucket — never arguments, never a user-defined name.
```

The emitted list is 35 names (`/logout` … `/workflows`); `/login` sits *outside* the run, and
`/name` is absent entirely. So **`TELEMETRY_DROPPED_SLASH_COMMAND_NAMES ⊇ {"/login", "/name"}`**
(PROVEN by set difference against the TUI table; the constant's own literal is not separately
present). Every plugin command and skill shortcut is reported as the literal string `custom`.

The doc string also names the source file: **`crates/tui/src/slash.rs`**.

### 1.3 `TBH_UNKILL_SLASH_COMMANDS` — the kill-list

* The env var string exists exactly once, at file offset `0xbb9ae2f`, inside the startup env-capture
  blob between `MUSE_CUSTOM_HEADERS` and `MUSE_DISABLE_APPROVAL_JUDGE`.
* Its only code reference is at `0x105f5825c` (`adrp x0, 0x10bb9a000 ; add x0, x0, #0xe2f ; mov w1, #25`),
  in a function that reads a run of env vars in the order
  `MUSE_CUSTOM_HEADERS` → `TBH_UNKILL_SLASH_COMMANDS` → `MUSE_DISABLE_APPROVAL_JUDGE`. It is read
  through a *different* helper (`0x108fef29c`) than its neighbours (`0x108fec81c`), and the result is
  a `(tag, ptr, len, cap)` triple, i.e. an `Option<String>`/`Option<Vec>`.
* The counterpart is a **settings/feature-config field named `killed_slash_commands`**, found in two
  serde field-name runs:
  * `0xbb97bd9`: `… agents  ttl_seconds  gates  killed_slash_commands  secret_schema_version …`
  * `0xbb9fc28`: `… decision  ttl_seconds  gates  killed_slash_commands   OAuthTokens …`
  It sits next to `ttl_seconds` and `gates`, i.e. it belongs to the **cached remote feature/gate
  document** (compare `crates/tui/src/startup/meta_catalog_cache/fetch.rs` in the leaked paths) —
  not to `settings.json` (`FeatureConfigSettings` only carries `enabled`).

**Conclusion (partly INFERRED):** the server-driven catalog can ship a `killed_slash_commands` list
that removes commands from the TUI; `TBH_UNKILL_SLASH_COMMANDS` is the local escape hatch that
re-enables them. Empirically the killable subset is **empty in this offline build**: setting the
variable to `1`, `all`, `*`, `permissions`, `/permissions`, `permissions,skill,exit` and `login` all
produced exactly the same 39-row menu (`↓ 32 more` in every case). The exact value grammar could not
be proven without a live catalog response.

### 1.4 What the composer actually offers — measured, not guessed

With `--provider echo`, workspace trusted, no gates set, typing `/` gives **39 rows**, in this order
(verified by stepping the cursor with ↓ and screenshotting at offsets 0, 7, 8, 14, 15, 21, 22, 28, 29, 35, 39):

```
 0 /clear          Start a fresh session and wipe the scrollback
 1 /compact        Summarize the conversation to free up context
 2 /copy           Copy the last response to the clipboard
 3 /deep-research  Research a question across sources with cross-checking and citations
 4 /effort         Set the model's effort level
 5 /export         Save the conversation, or the full session log
 6 /feedback       Send quick feedback to the team
 7 /fork           Branch this session from the latest message
 8 /goal           Start or manage continuous work toward a goal
 9 /help           Show help
10 /init           Explore the workspace and create or improve AGENTS.md
11 /keymap         Show keyboard shortcuts
12 /logout         Log out, forget the saved login, and exit
13 /model          Choose model
14 /name           Show or rename this session
15 /new            Start a fresh session, keep the scrollback
16 /quit           Quit when idle
17 /recap          Show a recap of recent session activity
18 /resume         Resume an earlier Muse Code session
19 /rules          Show which md files govern this session
20 /settings       Open local settings
21 /side           Start a side conversation
22 /skills         Browse and use skills
23 /status         Show current session status
24 /stop           Stop all background tasks
25 /subagents      View running and past subagents
26 /tasks          View and manage background tasks and terminals
27 /theme          Choose color theme
28 /usage          Show session usage
29 /voice          Manage voice input
30 /workflows      Browse workflow runs
31 /create-skill       · built-in · default skill
32 /doctor             · built-in · default skill
33 /grill              · built-in · default skill
34 /grill-and-record   · built-in · default skill
35 /import             · built-in · default skill
36 /manage-settings    · built-in · default skill
37 /plan               · built-in · default skill
38 /loop           Schedule a recurring prompt     (bundled plugin command)
```

Not in the default browse list but **resolvable by typing a prefix**:

| Command | Why it is hidden |
|---|---|
| `/permissions` | appears as soon as you type `/p`; not listed in the unfiltered browse list or in `/help → Commands` |
| `/skill` | appears on `/s`; `/skills` is the browsable one |
| `/exit` | appears on `/e`; the unfiltered list shows the alias `/quit` in its alphabetical slot |
| `/login` | hidden for `--provider echo`. **PROVEN present** with `--provider meta` (the whole TUI is replaced by the login picker) |
| `/upgrade` | hidden unless `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH=1`. With that set: `/u` → `/upgrade  Show your subscription plan` + `/usage` |
| `/plugins` | hidden unless `MUSE_EXPERIMENTAL_PLUGINS=1`. With that set: `/p` → `/permissions`, `/plugins`, `/tasks (/ps)`, `/plan` |

Gate proof (same session, same filter, one env var different):

```
$ /p                                   → /permissions, /tasks (/ps), /plan
$ MUSE_EXPERIMENTAL_PLUGINS=1  /p      → /permissions, /plugins, /tasks (/ps), /plan
$ /u                                   → /usage
$ MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH=1  /u → /upgrade, /usage
```

### 1.5 Bundled skill shortcuts (`· built-in · default skill`)

Seven bundled skills register as slash commands. From `/help → Commands` (descriptions truncated by
the panel, ellipsis preserved):

| Command | Args | Description |
|---|---|---|
| `/create-skill` | — | Create and validate a new Muse skill — project-local in the current workspace by default, or a personal skill staged for `muse skills install` into the managed personal root. … |
| `/doctor` | — | Diagnose Muse Code product/runtime issues from installed binary evidence. Use ONLY when the user explicitly invokes the doctor skill, asks to debug/troubleshoot Muse Code itse… |
| `/grill` | — | Run a decision-forcing interview only when the user explicitly asks to be grilled, pressure-tested, or stress-tested. |
| `/grill-and-record` | — | Run an explicitly requested decision interview and record each settled decision in durable project documentation. |
| `/import` | `<session-id-or-path>` | Import a Claude Code, Codex, or Grok session |
| `/manage-settings` | — | Any explicit Muse Code setting question or change (model, reasoning effort, /settings) requires a silent read_skill call for bundled:manage-settings as FIRST ACTION—no assista… |
| `/plan` | — | Create a grounded, decision-complete plan, then stop for approval. Use ONLY when the user explicitly asks to plan — when they request a plan, design, approach, rollout/migrati… |

### 1.6 `/loop` — the bundled *plugin* command, and the plugin-command format

`/loop` is not a built-in and not a skill: it is a **bundled plugin**. Its manifest and command file
are embedded verbatim at `0xb6df8a8`:

```json
{
  "schemaVersion": 1,
  "name": "loop",
  "displayName": "Loop",
  "version": "1.0.0",
  "description": "Built-in recurring-prompt command.",
  "compat": {"source": "native", "manifestDir": ".muse-plugin"},
  "capabilities": {"skills": [], "hooks": [], "mcpServers": [], "commands": [
    {"id": "loop", "path": "commands/loop.md", "enabledDefault": true}
  ]}
}
```

`commands/loop.md`:

```markdown
---
description: Schedule a recurring prompt
argument-hint: [interval] <prompt>
---
Understand what the user wants to run and on what cadence, then call `cron_create` exactly once. …
```

A second embedded fixture (`0xb6e862b`) shows the same shape for a user plugin and proves
`$ARGUMENTS` substitution:

```json
"files": { "commands/summarize.md":
  "---\ndescription: Summarize a requested change\nargument-hint: <path>\n---\nSummarize $ARGUMENTS with file references.\n" }
```

Plugin IDs `loop` and `muse-core` are reserved by the product bundle.

### 1.7 PROVEN: shipping your own slash commands

Built and installed a plugin from scratch in the sandbox and watched the command appear:

```
$ mkdir -p ohmy-plugin/.muse-plugin ohmy-plugin/commands
# .muse-plugin/plugin.json  → capabilities.commands = [{id:"ohmy-hello", path:"commands/ohmy-hello.md", enabledDefault:true}]
# commands/ohmy-hello.md    → ---\ndescription: Say hello the oh-my way\nargument-hint: [name]\n---\nGreet $ARGUMENTS ...

$ MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins validate ohmy-plugin
valid	ohmy	native	skills=0 commands=1 hooks=0 mcp=0 reminders=0 diagnostics=0

$ MUSE_EXPERIMENTAL_PLUGINS=1 muse plugins install ohmy-plugin --scope user
installed	ohmy	0.1.0	enabled=true	trust=user-local	provenance=native-local
        cache=$XDG_DATA_HOME/muse/plugins/cache/local/ohmy/<sha256>/package
```

Then, in the TUI (**with the plugins gate NOT set** — the gate only hides the `/plugins` management
command, not plugin-provided commands):

```
⟩ /ohmy
  /ohmy-hello  Say hello the oh-my way
```

and `/help → Custom commands`:

```
  General  Commands  [Custom commands]
  Browse custom commands
  /ohmy-hello [name]
    Say hello the oh-my way
```

`muse plugins` sub-commands (verbatim `muse plugins --help`, gate on):

```
usage: muse plugins <command>
Commands:
  install <path> [--scope user|project] [--json]
  install <plugin>@<marketplace> [--json]
  list [--available] [--json]
  inspect <id> [--json]
  approve <plugin-id[[:kind]:capability-id] | stable-id> [--json]
  reject  <plugin-id[[:kind]:capability-id] | stable-id> [--json]
  hook test <plugin-id>:<hook-id> | plugin:<plugin-id>:hook:<hook-id> --fixture <path> [--json]
  marketplace add <name> <source> [--json]
  marketplace list [--json]
  marketplace update <name> [--json]
  marketplace remove <name> [--json]
  enable <id> [--json] / disable <id> [--json] / update <id> [--json]
  remove <id> [--delete-data] [--json]
  validate <path> [--json]
```

### 1.8 Slash-menu / help UI details

* Menu footer counts: `↓ N more` / `↑ N more`; the drawer shows 6 rows at a time regardless of
  terminal height (verified at 50 and 90 rows).
* `/help` has three tabs: **General**, **Commands**, **Custom commands**
  (`Browse commands` / `No commands found` / `Browse custom commands` / `No custom commands found` /
  `Browse built-in commands` / `No built-in commands found` at `0xb830d61`).
* Unknown command: `No slash command named ` (string at `0xb8306xx`), with a
  `↵ Send as prompt` affordance.
* `/skills` drawer footer: `enter use - space toggle - /skills diagnostics - esc close - type filter`.
* Command usage errors are literal, e.g. `usage: /skills use <id-or-path> [prompt]`,
  `usage: /skill <id-or-path> [prompt]`, `usage: /name [<name>]`,
  `usage: /deep-research <question>`, `usage: /plugins inspect <plugin-id>`.
* `/deep-research` refuses without its tools:
  `web_fetch_unavailable: /deep-research requires WebFetch; enable WebFetch in /settings and try again`
  and `workflow_unavailable: /deep-research requires Workflow; enable Workflow in /settings and try again`.
* `/effort` refuses on echo: `/effort is not available for the echo provider`.
* Background-command dispatch has an internal invariant string:
  `internal error: entered unreachable code: background slash command <x> has no dispatch arm`.

---

## 2. Keybindings

### 2.1 The keymap model

Four **contexts** — `app`, `composer`, `editor`, `navigation` — each holding named **actions**, each
action holding a list of key specs. 37 actions total (the `/keymap` header literally says
`All configurable shortcuts - 37 rows`).

The complete default table is one literal run at **`0xb7f7828`**. Reconstructed
(action id → default bindings → label → description), all confirmed against the running `/keymap`
overlay:

**`app` (12 actions)**

| Action id | Default binding(s) | Label | Description |
|---|---|---|---|
| `commands` | `/` | Commands | Open slash commands. |
| `shell` | `!` | Shell command | Start a shell command from the composer. |
| `files` | `@` | File paths | Search and insert file paths. |
| `keymap-short` | `?` | Shortcut overlay | Open or close the compact shortcut overlay when the composer is empty. |
| `paste-images` | `ctrl+v`, `super+v` | Paste images | Paste image data or paths into the composer. |
| `interrupt` | `ctrl+c` | Interrupt or exit | Cancel the active operation or request exit. |
| `suspend` | `ctrl+z` | Suspend | Suspend the terminal process when supported. |
| `external-editor` | `ctrl+g` | External editor | Edit the composer draft in `$VISUAL` or `$EDITOR`. |
| `clear-terminal` | `ctrl+l` | Clear terminal | Clear the terminal viewport. |
| `expand-tool-output` | `ctrl+o` | Expand tool output | Toggle expanded tool-output display. |
| `voice` | `alt+v`, `√` | Voice input | Start or stop voice input when available. |
| `process-background` | `ctrl+b` | Process background | Send the selected task to the background queue. |

**`composer` (8 actions)**

| Action id | Default binding(s) | Label | Description |
|---|---|---|---|
| `submit` | `enter` | Submit | Submit the composer message. |
| `newline` | `shift+enter`, `ctrl+j`, `ctrl+m` | New line | Insert a newline without submitting. |
| `queue` | `alt+enter` | Queue while running | Queue or steer input while a run is active. |
| `complete` | `tab` | Complete suggestion | Accept the active composer suggestion. |
| `cancel` | `esc` | Cancel or close | Close focused UI, cancel completion, or return to the composer. |
| `history-search` | `ctrl+r` | History search | Search composer history newest first. |
| `history-previous` | `ctrl+p` | Previous history | Recall the previous composer history item. |
| `history-next` | `ctrl+n` | Next history | Move forward through composer history. |

**`editor` (14 actions)**

| Action id | Default binding(s) | Label | Description |
|---|---|---|---|
| `move-left` | `left` | Move cursor left | Move left in the composer draft. |
| `move-right` | `right`, `ctrl+f` | Move cursor right | Move right in the composer draft. |
| `move-word-left` | `alt+b`, `∫`, `alt+left`, `super+left`, `ctrl+left` | Move word left | Move left by one word in the composer draft. |
| `move-word-right` | `alt+f`, `ƒ`, `alt+right`, `super+right`, `ctrl+right` | Move word right | Move right by one word in the composer draft. |
| `line-start` | `home`, `ctrl+a` | Line start | Move to the start of the current line. |
| `line-end` | `end`, `ctrl+e` | Line end | Move to the end of the current line. |
| `backspace` | `backspace`, `shift+backspace`, `ctrl+h` | Backspace | Delete text before the cursor. |
| `delete-forward` | `delete` | Delete forward | Delete text after the cursor. |
| `delete-word` | `alt+backspace`, `super+backspace`, `ctrl+backspace`, `ctrl+w` | Delete previous word | Delete text before the cursor by word. |
| `delete-next-word` | `alt+delete`, `super+delete`, `ctrl+delete`, `alt+d`, `∂` | Delete next word | Delete text after the cursor by word. |
| `delete-start` | `ctrl+u` | Delete to line start | Delete from cursor to line start. |
| `delete-end` | `ctrl+k` | Delete to line end | Delete from cursor to line end. |
| `yank` | `ctrl+y` | Yank deleted text | Paste the last killed composer text. |
| `transpose` | `ctrl+t` | Transpose characters | Swap the two characters around the cursor. |

**`navigation` (3 actions)**

| Action id | Default binding(s) | Label | Description |
|---|---|---|---|
| `move-row-up` | `up` | Move row up | Move up through drawer, picker, list, or history rows. |
| `move-row-down` | `down` | Move row down | Move down through drawer, picker, list, or history rows. |
| `focus-plan` | `alt+up` | Review plan | Review every item in an overflowing plan. |

The macOS Option-key literals (`√` = ⌥v, `∫` = ⌥b, `ƒ` = ⌥f, `∂` = ⌥d) are bound alongside the
`alt+` names — verified live: `Move word left  alt+b / ∫ / alt+left / super+left / c…`.

### 2.2 The `/keymap` overlay

```
  [All]  Common  App  Composer  Editor  Navigation  Customized  Unbound  |  All configurable shortcuts - 37 rows
  filter

⟩ Commands              /  App  Open slash commands.
  Shell command         !  App  Start a shell command from the composer.
  File paths            @  App  Search and insert file paths.
  Shortcut overlay      ?  App  Open or close the compact shortcut overlay when the composer is empty.
  Paste images          ctrl+v / super+v  App  Paste image data or paths into the composer.
  Interrupt or exit     ctrl+c  App  Cancel the active operation or request exit.
  ↓ 31 more
  App - Open slash commands.
  up/down move - tab switch tab - type filter - enter details - esc close
```

Tabs and their header text (from `0xb7f8401` and confirmed live):

| Tab id | Label | Header |
|---|---|---|
| `all` | All | All configurable shortcuts — 37 rows |
| `common` | Common | Most-used shortcuts — 16 rows |
| (app) | App | App-level shortcuts |
| (composer) | Composer | Composer shortcuts |
| (editor) | Editor | Text editing shortcuts |
| (navigation) | Navigation | Drawers, pickers, and transcript movement |
| `customized` | Customized | User overrides |
| (unbound) | Unbound | Shortcuts explicitly disabled |

The **Common** 16 (measured in order): Commands, Shell command, File paths, Shortcut overlay,
Paste images, Interrupt or exit, External editor, Voice input, Submit, New line, Queue while running,
Complete suggestion, Cancel or close, Previous history, Move row up, Move row down.

Row detail view (`enter` on a row):

```
  Keymap  |  Start a shell command from the composer.
⟩ Replace binding       !  Enter a key spec such as ctrl+x, shift+enter, or ;
  Unbind action         Disable this action until it is reset or replaced.
  Reset to default      !  Restore the default binding.
  Comma-separated bindings are supported.
  enter choose - esc close - /keymap reopen all shortcuts
```

Binding editor: `Keymap | Replace Shell command` / `Binding` / `enter save - esc cancel`.
Internal ids: `keymap.shortcut.replace`, `keymap.shortcut.unbind`, `keymap.shortcut.reset`,
`keymap.shortcut.detail`, `keymap.shortcut.saveBinding`.

### 2.3 Keymap is fully configurable from `settings.json` — PROVEN

Schema: **`tui.keymap : { <context> : { <action> : [ "<key-spec>", … ] } }`**, all leaves strings.
Recovered by feeding deliberately wrong types and reading serde's message:

```
tui.keymap = 5                         → invalid type: integer `5`, expected a map
tui.keymap.app = 5                     → invalid type: integer `5`, expected a map
tui.keymap.app.commands = 5            → invalid type: integer `5`, expected a sequence
tui.keymap.app["clear-terminal"] = [5] → invalid type: integer `5`, expected a string
tui.keymap.app["clear-terminal"] = [{}]→ invalid type: map, expected a string
```

Working example, written to `$XDG_CONFIG_HOME/muse/settings.json`:

```json
{"schema_version":1,
 "tui":{"keymap":{"app":{"clear-terminal":["ctrl+q","alt+j"]},
                  "editor":{"transpose":[]}}}}
```

`/keymap → Customized` then shows:

```
  All  Common  App  Composer  Editor  Navigation  [Customized]  Unbound  |  User overrides - 2 rows
⟩ Clear terminal        ctrl+q / alt+j  App  Clear the terminal viewport.
  Transpose characters  unbound         Editor  Swap the two characters around the cursor.
```

An empty array `[]` is how you **unbind**.

**Semantic validation** (bad entries are silently dropped by the TUI, but the enterprise validator
reports them). `muse config validate --plane defaults --file X` is a perfect oracle:

```
{"tui":{"keymap":{}}}                                       → valid
{"tui":{"keymap":{"app":{}}}}                               → valid
{"tui":{"keymap":{"editor":{"transpose":["ctrl+q"]}}}}      → valid
{"tui":{"keymap":{"composer":{"submit":["enter"]}}}}        → valid
{"tui":{"keymap":{"app":{"voice":[]}}}}                     → valid
{"tui":{"keymap":{"app":{"clear-terminal":["ctrl+q","alt+j"]}}}} → valid
{"tui":{"keymap":{"app":{"clear-terminal":["ctrl+y"]}}}}    → semantic_invalid   (ctrl+y already = editor.yank → duplicate binding)
{"tui":{"keymap":{"app":{"clear_terminal":["ctrl+y"]}}}}    → semantic_invalid   (underscore is not the action id)
{"tui":{"keymap":{"zzz":{"transpose":["ctrl+q"]}}}}         → semantic_invalid   (unknown context)
{"tui":{"keymap":{"app":{"zzz":["ctrl+q"]}}}}               → semantic_invalid   (unknown action)
{"tui":{"keymap":{"app":{"clear-terminal":["nope+q"]}}}}    → semantic_invalid   (bad key spec)
{"tui":{"keymap":{"app":{"keymap-short":["f1"]}}}}          → semantic_invalid   (function keys not in the key grammar)
{"tui":{"keymap":{"app":{"commands":["ctrl+q"]}}}}          → semantic_invalid   (app.commands appears to be reserved)
```

Error strings that back this up (offsets `0xcbadeed`, `0xcbbd0c8`):

```
unknown keymap action: <ctx>.<action>
unknown keymap context: <ctx>
route-reserved binding `<k>` is invalid for navigation.focus-plan
invalid key binding `<k>` for <ctx>.<action>
duplicate binding `<k>` in <ctx>.<action>
keymap ignored: <reason>
keymap shortcut not found
missing key binding
```

Enterprise policy can also pin/retire keymap entries:
`enterprise_status_composition_failed: code=retired_explicit_keymap_action field_id=…` and
`code=invalid_explicit_keymap field_id=…` (`0xcb9c855`). `settings.tui.keymap` is listed in the
enterprise `defaults` plane surface.

### 2.4 The compact `?` overlay

Pressing `?` on an empty composer:

```
  /               for commands                    !               for shell commands
  shift + enter   for newline                     enter           to submit message
  @               for file paths                  ctrl + v        to paste images
  ?               to show or close shortcuts      esc             to cancel or close
  ctrl + p/n      browse history                  ctrl + c        to interrupt or exit
  alt + enter     to queue while running          alt + v         to start voice input
  ctrl + o        to expand tool output           ctrl + g        to edit in $EDITOR
  open shortcuts with /keymap
```

Other footer hints seen in drawers: `ctrl+o comfortable view` / `ctrl+o dense view` /
`esc clear  ctrl+t transcript  ctrl+e expand  ↑/↓ browse  pgup/pgdn page` (resume picker),
`q close preview` (transcript preview), `j/k scroll`, `r restart`, `c cancel workflow`.

---

## 3. Theming — Muse Code **does** have a real theme system

This is the headline finding: unlike Claude Code's handful of hardcoded palettes, Muse Code ships a
**compiled 24-theme catalog plus a user-extensible custom-theme directory** and a live preview picker.

### 3.1 `/theme` — the picker with a live showcase

```
  Theme

⟩ Default  (active)     ⟩ Make the selected Settings row easier to see.
  Dynamic               ◆ I'll update selected_label_style and run the focused tests.
  Ayu Dark
  Catppuccin Mocha      ◆ Ran cargo test -p tbh-tui settings_overlay (exit 0)
  Dracula Theme           └ 37 passed
  Everforest Dark
  GitHub Dark Default   ◆ Edited crates/tui/src/render/panels/settings_overlay.rs (+1 −1)
  Gruvbox Dark Medium      fn selected_label_style(
  Houston                      selected: bool,
  Kanagawa Wave                semantic: &SemanticStyles<'_>,
  Monokai                  ) -> Style {
  Nord                         if selected {
  One Dark Pro            -        semantic.style(SemanticRole::PrimaryText).add_modifier(Modifier::BOLD)
  Poimandres              +        semantic.style(SemanticRole::PrimaryAccent).add_modifier(Modifier::BOLD)
  Rosé Pine Moon               } else {
  Synthwave '84                    semantic.style(SemanticRole::PrimaryText)
  Tokyo Night                  }
                           }
                        ◆ Working
                        ⟩
                          Meta · high · /workspace/tbh
  ↑↓ move · enter save · esc go back
```

The right pane is a canned session ("theme showcase") rendered in the highlighted theme so you can
see the transcript colours before committing. It happens to leak an internal type vocabulary:
`SemanticStyles<'_>`, `SemanticRole::PrimaryText`, `SemanticRole::PrimaryAccent`,
`crates/tui/src/render/panels/settings_overlay.rs`.

**The list is filtered by the detected terminal background.** With the harness answering
OSC 11 `rgb:1010/1010/1010` (dark) the picker offers Default, Dynamic + the 15 dark themes above.
Answering `rgb:f8f8/f8f8/f8f8` (light) instead gives:

```
⟩ Default  (active)
  Dynamic
  Ayu Light
  Catppuccin Latte
  Everforest Light
  GitHub Light Default
  Gruvbox Light Medium
  Kanagawa Lotus
  One Light
  Rosé Pine Dawn
  Snazzy Light
```

15 dark + 9 light = the 24 catalogue entries exactly.

### 3.2 The compiled theme catalog (`MCTHM001`) — fully parsed

The bundled themes live in a binary blob starting with magic `MCTHM001` at file offset
`0xb9d0bc7`, followed by a 100-byte header (a u32 + three 32-byte digests), a `u64` entry count of
`24`, and then 24 records. Serde field names for the record are in the binary at `0xb9cd623`:

```
struct BundledThemeSource with 10 elements
  id  display_name  source_file  source_mode  license  source_digest
  foreground  background  accent  scopes
struct SourceScopeStyle with 3 elements
  scopes  foreground  font_style
struct CatalogV1 with 3 elements
  manifest_digest  source_digest  entries
Theme source kinds:  Default | Dynamic | Bundled | Custom
```

Parsed in full (script: length-prefixed strings, then an 8-byte mode/license word, a 32-byte digest,
three optional RGBA colours, then a scope-style list):

| # | id | Display name | source file | mode | fg | bg | accent | #scopes |
|---|---|---|---|---|---|---|---|---|
| 0 | `ayu-dark` | Ayu Dark | ayu-dark.json | dark | `#BFBDB6` | `#10141C` | `#E6B450` | 65 |
| 1 | `ayu-light` | Ayu Light | ayu-light.json | light | `#5C6166` | `#FCFCFC` | `#F29718` | 65 |
| 2 | `catppuccin-latte` | Catppuccin Latte | catppuccin-latte.json | light | `#4C4F69` | `#EFF1F5` | `#8839EF` | 179 |
| 3 | `catppuccin-mocha` | Catppuccin Mocha | catppuccin-mocha.json | dark | `#CDD6F4` | `#1E1E2E` | `#CBA6F7` | 179 |
| 4 | `dracula` | Dracula Theme | dracula.json | dark | `#F8F8F2` | `#282A36` | `#6272A4` | 85 |
| 5 | `everforest-dark` | Everforest Dark | everforest-dark.json | dark | `#D3C6AA` | `#2D353B` | `#A7C080` | 276 |
| 6 | `everforest-light` | Everforest Light | everforest-light.json | light | `#5C6A72` | `#FDF6E3` | `#93B259` | 276 |
| 7 | `github-dark-default` | GitHub Dark Default | github-dark-default.json | dark | `#E6EDF3` | `#0D1117` | `#1F6FEB` | 49 |
| 8 | `github-light-default` | GitHub Light Default | github-light-default.json | light | `#1F2328` | `#FFFFFF` | `#0969DA` | 49 |
| 9 | `gruvbox-dark-medium` | Gruvbox Dark Medium | gruvbox-dark-medium.json | dark | `#EBDBB2` | `#282828` | `#3C3836` | 126 |
| 10 | `gruvbox-light-medium` | Gruvbox Light Medium | gruvbox-light-medium.json | light | `#3C3836` | `#FBF1C7` | `#EBDBB2` | 126 |
| 11 | `houston` | Houston | houston.json | dark | `#EEF0F9` | `#17191E` | `#00DAEF` | 244 |
| 12 | `kanagawa-lotus` | Kanagawa Lotus | kanagawa-lotus.json | light | `#545464` | `#F2ECBC` | `#C7D7E0` | 76 |
| 13 | `kanagawa-wave` | Kanagawa Wave | kanagawa-wave.json | dark | `#DCD7BA` | `#1F1F28` | `#223249` | 76 |
| 14 | `monokai` | Monokai | monokai.json | dark | `#F8F8F2` | `#272822` | `#99947C` | 51 |
| 15 | `nord` | Nord | nord.json | dark | `#D8DEE9` | `#2E3440` | `#3B4252` | 139 |
| 16 | `one-dark-pro` | One Dark Pro | one-dark-pro.json | dark | `#ABB2BF` | `#282C34` | `#3E4452` | 275 |
| 17 | `one-light` | One Light | one-light.json | light | `#383A42` | `#FAFAFA` | `#526FFF` | 211 |
| 18 | `poimandres` | Poimandres | poimandres.json | dark | `#A6ACCD` | `#1B1E28` | `#303340` | 101 |
| 19 | `rose-pine-dawn` | Rosé Pine Dawn | rose-pine-dawn.json | light | `#575279` | `#FAF4ED` | `#6E6A86` (α 20) | 33 |
| 20 | `rose-pine-moon` | Rosé Pine Moon | rose-pine-moon.json | dark | `#E0DEF4` | `#232136` | `#817C9C` (α 38) | 33 |
| 21 | `snazzy-light` | Snazzy Light | snazzy-light.json | light | `#565869` | `#FAFBFC` | `#09A1ED` | 165 |
| 22 | `synthwave-84` | Synthwave '84 | synthwave-84.json | dark | `#FFFFFF` | `#262335` | `#1F212B` | 88 |
| 23 | `tokyo-night` | Tokyo Night | tokyo-night.json | dark | `#A9B1D6` | `#1A1B26` | `#545C7E` (α 51) | 114 |

`manifest_digest = be3304008af86c24b5c458bd734a17984196d1f778020b9ab77f8f480afa16a1…` (first 32 B
of the header). The `source_file` names are VS Code-style theme JSON files (the build ingests them;
they are not present on disk at runtime), and each theme carries a full TextMate scope→style list
(the `scopes` column) used for **syntax highlighting of code blocks in the transcript**.

### 3.3 Two synthetic themes

* **Default** — the product palette. It must always resolve; the binary carries the assertions
  `the fallback Default theme must resolve` (`0xb806850`) and
  `the fixed Default theme must resolve` (`0xb807580`).
* **Dynamic** — derived from the terminal's own colours. This is what the OSC 10/11/`4;0..15`
  probes are for (`tui/src/terminal/background_probe/overlap.rs`,
  `TerminalColorProbeOutcome`, `spawn_probe_thread`). Under Dynamic the TUI emits colours built from
  the palette the terminal reported (e.g. our fake bright-red `#ff0000` came straight back as
  `38;2;255;0;0`).

### 3.4 Custom themes — `~/.config/muse/themes/*.tmTheme` — PROVEN

Discovery lives in `tbh_tui::…::discovery` (`OsTheme`), symbol fragments recovered from the binary:

```
load_custom_themes            discover_custom_theme_candidates
CustomThemeSource             CustomThemeCandidate            RejectedThemeEntry
custom::CustomThemeSnapshot   theme_picker::showcase::PreparedThemeShowcase
state::client_config::theme::PickerThemeRow
state::client_config::theme::snapshot::ThemeStartupSnapshot
```

Experiment (all in the sandbox `HOME`):

| File dropped | Result in `/theme` |
|---|---|
| `$XDG_CONFIG_HOME/muse/themes/ohmy.tmTheme` | **appears** as `OhMyMuse Neon` |
| `$XDG_DATA_HOME/muse/themes/ohmy.tmTheme` | ignored |
| `…/themes/lower.tmtheme` | **appears** (`Lower Case Ext`) |
| `…/themes/upper.TMTHEME` | **appears** |
| `…/themes/jsonform.json` | ignored |

So: **the custom-theme directory is `$XDG_CONFIG_HOME/muse/themes` (else `$HOME/.config/muse/themes`),
the accepted extension is `.tmTheme` case-insensitively, and JSON is not accepted.** The display name
comes from the plist's `name` key; custom themes are sorted by display name and appended after the
bundled ones.

Selecting one and pressing `enter` writes:

```json
{
  "schema_version": 1,
  "tui": {
    "theme": "custom:ohmy"
  }
}
```

i.e. **`custom:<file stem>`**. Selecting a bundled theme writes its bare id
(`"tui":{"theme":"ayu-dark"}`, confirmed by the same round-trip).

The custom-theme loader's own error/diagnostic strings (offsets `0xcbb59cd`, `0xcbaddb9`, `0xb9cc9a0`):

```
custom theme <name> is not a regular file
custom theme <name> exceeds the <N>-byte limit
custom theme byte limit overflowed
theme <name> cannot satisfy its contrast matrix <m>
Skipped custom theme files: <a> (+<n> more)
.TMTHEME    .tmtheme    TextMateRule    MCCUSTOM1
```

(`MCCUSTOM1` is the magic for the *compiled custom-theme snapshot* cache, mirroring `MCTHM001`.)

Theme-resolution notices, printed above the composer at startup (all three reproduced live):

```
Saved theme "bogus" was not found; using Default.
Saved theme <x> is unavailable; using Default.
Saved custom theme "custom:nope" is unavailable; using Default.
```

### 3.5 Theme hydration is asynchronous and bounded

Worker thread name `tbh-theme-hydration`; event kind `ThemeHydration`. Bound/diagnostic strings:

```
theme hydration starts at most once
theme hydration exceeded the 192-row bound
theme hydration returned unaligned showcase counts
theme hydration returned a mismatched showcase identity
theme hydration worker stopped before delivering a result
theme hydration worker could not start: <e>
theme hydration worker panicked: <e>
Loading theme showcase…
Theme showcase unavailable; theme selection still works.
theme save failed: <e>
theme save failed: no selectable theme · draft restored
No alternate themes are available
Theme background differs from terminal
```

### 3.6 Colour depth and `NO_COLOR` — PROVEN

`tui.color_depth` accepts `auto | truecolor | 256 | 16 | none` (error message quoted verbatim in §4).
Counting distinct SGR colour sequences emitted for the same scripted session:

| setting | emitted colours |
|---|---|
| `theme: nord` | 11 distinct truecolor pairs incl. `38;2;129;161;193` (Nord blue `#81A1C1`), `38;2;191;97;106` (`#BF616A`) |
| `theme: synthwave-84` | 11 distinct, incl. `38;2;254;222;93` (`#FEDE5D`), `38;2;254;68;80` (`#FE4450`) |
| `theme: default` | 10 distinct, incl. `38;2;90;160;255`, `38;2;243;139;168` |
| `theme: dynamic` | 11 distinct, derived from the probed palette |
| `color_depth: 256` | only `38;5;NNN` form (`38;5;75`, `38;5;211`, `38;5;252`, …) |
| `color_depth: 16` | 1 colour |
| `color_depth: none` | 0 colours |
| `NO_COLOR=1` (env) | 0 colours |

`NO_COLOR` and `256color` are both literals in the binary near the theme code.

### 3.7 Syntax highlighting

The bundled themes double as **syntect** themes for transcript code blocks. The binary carries the
full TextMate scope vocabulary (`comment`, `string`, `keyword.operator`, `entity.name.function`,
`markup.heading.*`, per-language scopes for Rust/Python/TS/CSS/YAML/TOML/diff/…), a
`.tmTheme`/`.sublime-syntax` loader (`sublime-settings`, `sublime-menu`, `sublime-keymap`,
`sublime-theme`, `sublime-build`, `sublime-syntax` extension list at `0xb88efb5`), a
`highlight::warm_up` entry point, and role names `PrimaryText`, `PaletteFallback`, `Corrected`,
`FixedTextMate`.

---

## 4. `tui.*` settings — complete schema

`TuiSettings` field list, verbatim from `0xbba117b`:

```
TuiSettings
  show_reasoning  reasoning_summaries  away_recap_enabled  prompt_hint_enabled
  voice_enabled  voice_shortcut_mode  keymap  resize_reflow_cap  color_depth
  terminal_background  theme  rules_import_offer_dismissed
  foreign_context_notice_shown  verbose_output
```

(`voice_tap_enabled` also exists in the enterprise `TuiDefaultsV1` variant, 11 elements.)

Value domains, recovered by feeding `"zzz"` and reading the rejection verbatim:

| Key | Type / domain | Default | Error text proving the domain |
|---|---|---|---|
| `tui.theme` | string: `default`, `dynamic`, a bundled id, or `custom:<stem>` | `default` | `invalid type: integer 123, expected a string` |
| `tui.color_depth` | `auto` \| `truecolor` \| `256` \| `16` \| `none` | `auto` | `invalid color_depth "zzz": expected "auto", "truecolor", "256", "16", or "none"` |
| `tui.terminal_background` | `auto` \| `light` \| `dark` | `auto` | `invalid terminal_background "zzz": expected "auto", "light", or "dark"` |
| `tui.verbose_output` | `less` \| `edits` \| `more` | `edits` (shown as `edits & writes`) | ``unknown variant `zzz`, expected one of `less`, `edits`, `more` `` |
| `tui.voice_shortcut_mode` | `toggle` \| `hold_to_talk` | `toggle` | ``unknown variant `zzz`, expected `toggle` or `hold_to_talk` `` |
| `tui.resize_reflow_cap` | `auto` \| `disabled` \| positive integer rows | `auto` | `invalid resize_reflow_cap "zzz": expected "auto", "disabled", or a positive integer row…` |
| `tui.keymap` | `{ctx:{action:[keyspec]}}` | `{}` | see §2.3 |
| `tui.show_reasoning` | bool | `false` (gated, §7) | — |
| `tui.reasoning_summaries` | bool | `true` | — |
| `tui.away_recap_enabled` | bool | `true` | — |
| `tui.prompt_hint_enabled` | bool | `true` | — |
| `tui.voice_enabled` | bool | `true` | — |
| `tui.voice_tap_enabled` | bool | — | — |
| `tui.rules_import_offer_dismissed` | bool | `false` | — |
| `tui.foreign_context_notice_shown` | bool | `false` | — |

Unknown keys under `tui` are **ignored** (`{"tui":{"zzz":1}}` starts fine) — the settings doc
embedded in the binary explicitly says "Preserve unrelated keys … and unknown fields."

Config root, verbatim from the embedded `manage-settings` skill (`0xb70…`):

```
Config root: `$XDG_CONFIG_HOME/muse`, else `$HOME/.config/muse` — holds
`settings.json` (saved settings and defaults), `auth.json`, and `trust.json`
…
`tui.terminal_background` is one of `auto`, `light`, or `dark`; absent defaults to
`auto` (detect from the startup probe). `light`/`dark` override the detected terminal
background for theme resolution at the next startup.
```

Also observed on disk after a run: `$XDG_DATA_HOME/muse/{sessions,skills,plugins,local-tracing,
tui-history.jsonl,session-index.db}` and lockfiles `.settings.json.lock`, `.auth.json.lock`.

### 4.1 `/settings` overlay

```
  Settings
  ⟩ Search settings
  ─────────────────────────────────────────────────────────────────────────────
  Model
  ⟩ Model  Open the model picker.                                          echo
  Appearance
    Reasoning summaries  Live summary line while the model works.            on
    Away recap  Display a short local recap after you return.                on
    Prompt hint  Show one contextual feature tip in the idle composer.       on
  Context
    Personal rules fallback  Fall back to compatible personal rules.         on
    Personal skills fallback  Include compatible personal skills.            on
  TUI
    Verbose output  Transcript detail, from less to more.          edits & writes
  Input
    Voice input  Enable or disable voice.                                    on
    Voice mode  Choose hold or toggle.                                   Toggle
  Tools
    Workflows  Control model-chosen workflow proposals.                    auto
    Agent delegation  Control simple model-chosen helper agents.           auto
    Web search  Applies to new sessions.                  client (new sessions)
    Session messaging  Unverified peers require approval.                    on
    Skills  Use /skills to browse and activate skill packs.          14 sources
```

Footer: `↑↓ move · tab/enter change · type to search · esc go back · filter:`.
**Theme is deliberately *not* in `/settings`** — it has its own `/theme` picker.

---

## 5. Status line (`tui/src/app/session_status.rs`)

### 5.1 The one-line footer

Format, measured across flag combinations:

```
  <provider/model> · [<effort> ·] <cwd> [· YOLO]
```

| Invocation | Footer |
|---|---|
| `--provider echo --trust-workspace` | `echo · /…/ws` |
| `--provider echo --yolo` | `echo · /…/ws · YOLO` |
| `--approval-mode never --trust-workspace` | `echo · /…/ws` |
| `--disable-sandbox --trust-workspace` | `echo · /…/ws` |
| (theme-showcase fixture, Meta provider) | `Meta · high · /workspace/tbh` |
| inside `/side` | `echo · /…/ws    Side chat: 01a05d8c · Ctrl+C to return · source idle` |

So the only extra badge in the plain footer is `YOLO`; the effort tier appears for real providers.
`SIDE`, `Away`, `busy`, `idle` are also 4-char literals in the same pool (`0xcb96044`).

### 5.2 `/status` — the full panel

```
┌──────────────────────────────────────────────────────────────────────────┐
│  MUSE CODE 1.0.1 / glass-acrux                                      IDLE │
│                                                                          │
│  MODEL          echo                                                     │
│                 echo · native-basic                                      │
│                                                                          │
│  WORKSPACE      /…/sandbox/tui/ws                                        │
│                 trusted · not found                                      │
│  ACCESS         Unrestricted                                             │
│                 none                                                     │
│                                                                          │
│  USAGE          0 tokens · 0 turns · 0 subagents                         │
│  CONTEXT        not projected                                            │
│                                                                          │
│  SESSION        01a05d73-41a7-7ef3-a398-9b6ec57a51d1                     │
│  ACTIVITY       no tasks                                                 │
│                 0 terminals · inbox clear                                │
└──────────────────────────────────────────────────────────────────────────┘
```

`glass-acrux` is an auto-generated session codename (renameable with `/name`).

The complete set of section labels is a single literal run at `0xb830364`:

```
MODEL  USAGE  CONTEXT  SESSION  WORKSPACE  ACCESS  ACCOUNT  NOTICE
Scheduled jobs   No scheduled jobs   Next:
```

`ACCOUNT` and `NOTICE` (and the `Scheduled jobs` block) render only when relevant — ACCOUNT is the
logged-in Meta account / subscription row (see §8).

### 5.3 Is the status line configurable?

**No.** There is no `tui.statusline`, `prompt`, `format`, or template key anywhere in `TuiSettings`
(§4 lists the complete 14-field struct), no format-string setting in the enterprise `defaults` plane,
and no theme field that touches it. The only user-visible levers are indirect: the theme (colours),
`/name` (session codename in `/status`), `tui.verbose_output`, and the flags that add the `YOLO`
badge. An "oh-my" framework cannot re-template the status line without patching the binary.

### 5.4 Other footer / transient lines

* Idle composer shows one rotating **prompt hint** above the divider, e.g.
  `── Voice input (⌥ + v to start) ───────` or
  `⟩ Type @ to search and insert workspace file paths`.
* Toasts appear above the composer: `Copied last response to clipboard`,
  `Press Ctrl-C again to quit`, `Press Ctrl-D again to quit`, `Model set to <m>`,
  `Permissions changed to <p>`, `Goal set — …`, `cleared goal`.
* Context pressure: `context <a>/<b>` and `context blocked <a>/<b>`.

### 5.5 The prompt-hint catalog (`tui.prompt_hint_enabled`)

Recovered in full at `0xb833bca` — pairs of *(hint text, "when it helps" note)* keyed by id. An LLM
picks one per idle turn (`You are picking one optional feature tip for a user who just finished a
turn and is idle. … Reply with exactly that tip's id … If no tip is clearly helpful, reply exactly NONE.`):

| id | hint | when |
|---|---|---|
| `file-mentions` | Type @ to search and insert workspace file paths | user types or pastes file paths by hand |
| `shell-mode` | Start a message with ! to run a shell command yourself | user asks the agent to run trivial one-off shell commands |
| `image-paste` | Paste an image with Ctrl+V — file paths and URLs work too | conversation involves UI, screenshots, or diagrams |
| `voice-input` | Press Alt+V to dictate instead of typing | user writes long prose prompts |
| `queue-steer` | Alt+Enter queues or steers while a run is active | user waits for turns to finish |
| `shortcut-overlay` | Press ? on an empty composer to see keyboard shortcuts | user seems new to the TUI's keyboard controls |
| `expand-tool-output` | Press Ctrl+O to expand collapsed tool output | user asks what a command printed |
| `side-chat` | /side asks a quick question without touching this thread | user goes off-topic mid-task |
| `fork` | /fork branches the conversation from the latest point | user weighs alternative approaches |
| `compact` | /compact frees context in long sessions | session is long or context pressure is discussed |
| `goal` | /goal pins a session objective with a progress bar | long multi-step effort underway |
| `resume` | /resume reopens a past session (--last for the latest) | user mentions prior or lost work |
| (workflow) | Ask to 'use a workflow' to fan out parallel agents on big tasks | large parallelizable task |
| `subagent-center` | /agent opens the subagent command center | subagents are running |
| `work-inventory` | /tasks shows workflows, subagents and terminals in one place | several concurrent activities in flight |
| `loop` | /loop 10m <prompt> schedules a recurring prompt | user manually re-checks or polls something |
| `background-queue` | Ctrl+B sends the selected task to the background queue | long builds or tests block the conversation |
| `skills` | /skills browses ready-made skills for repeatable procedures | user repeats a procedure by hand |
| `plugins` | /plugins marketplace installs new commands and skills | user wants a capability the agent lacks |
| `effort` | /effort adjusts reasoning depth vs speed | user complains responses are slow or shallow |
| `usage` | /usage shows this session's token usage | user asks about cost |
| `export` | /export saves the conversation to a text file | user wants to share or archive |

(Note `/agent` in the `subagent-center` hint is stale copy — the shipped command is `/subagents`.)

---

## 6. Startup banner

There is **no ASCII-art splash in the default path**. A fresh `HOME`, 180×50 terminal, first launch:

```

  Muse Code 1.0.1

── Voice input (⌥ + v to start) ───────────────────────────────────────────────
⟩
───────────────────────────────────────────────────────────────────────────────
  echo · /…/sandbox/tui/ws · YOLO
```

That is the whole banner: one blank line, two spaces, `Muse Code <version>`. Notices
(theme fallback, rules-import offer, foreign-context notice) print under it.

Splash machinery does exist in the binary but was never triggered in any offline run:
`tbh_tui::splash::SplashWorker::spawn`, literals `splash-grid`, `spawn splash worker`, app event
`SplashTick`, plus a classic ASCII density ramp at `0xb807… `:

```
.'`^,:;Il!i><~+_-?][}{1)(|/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$
```

There is also an `UltraActivationTick` event and `MUSE_DISABLE_ULTRA_ANIMATION` (an animation when
the `ultra` effort tier is engaged). **INFERRED:** the splash grid is the animated
image/logo the product plays for authenticated Meta sessions and/or the ultra activation; it is not
reachable with `--provider echo`.

Untrusted-workspace gate (shown *before* the banner when the workspace is not trusted):

```
Do you trust this workspace?
Workspace: /…/sandbox/tui/ws
Trusting allows project-local skills, rules, hooks, and plugin config to load before the model runs.
Only trust this workspace when you trust its contents.
> 1  Trust and continue
  2  Quit
Use Up/Down or 1/2, then Enter. Esc quits.
```

---

## 7. Reasoning display (`MUSE_EXPERIMENTAL_REASONING_DISPLAY`)

PROVEN, by diffing `/settings` with and without the gate. Without it, the **Appearance** section is:

```
  Appearance
    Reasoning summaries  Live summary line while the model works.        on
    Away recap …
```

With `MUSE_EXPERIMENTAL_REASONING_DISPLAY=1` a new first row appears:

```
  Appearance
    Show reasoning  Show model reasoning in the transcript.             off
    Reasoning summaries  Live summary line while the model works.        on
```

So the gate reveals the **`tui.show_reasoning`** boolean (default `off`), distinct from
`tui.reasoning_summaries` (default `on`, the one-line "Working / Thinking" summary). The transcript
renderer already has `reasoning summary` and `reasoning` item kinds
(`tool_search returned` … `reasoning summary` … `reasoning` … `assistant message`, `0xb833…`).

Full `MUSE_EXPERIMENTAL_*` list (single literal run at `0xbb9…`, 43 gates):

```
WORKFLOW_TOOL  ARTIFACT_TOOL  LOCAL_SESSION_MESSAGING  EXTERNAL_AGENT_INGRESS  CODE_MODE
PREFIX_COMPACTION  MONITOR  FOREIGN_PERSONAL_CONTEXT_KILL  VOICE  VOICE_DEFAULT_ON
VOICE_NATIVE_CAPTURE  REASONING_DISPLAY  MODEL_EFFORT_CONTEXT  BASH_TITLES
BASH_SANDBOX_ESCALATION  GIT_SANDBOX_RELAXATION  PLUGINS  ENTERPRISE_CONFIG  WEB_FETCH
SERVER_WEB_FETCH  CURL_WEB_FETCH  WEB_FETCH_PREFLIGHT_HARD_CAP  FIRST_TURN_MINIMAL_EFFORT
TODO_REMINDER  MEMORY_REMINDER  SKILL_REMINDER  GOAL_REMINDER  VERIFY_REMINDER
SCOPE_REMINDER  SESSION_RUNTIME  SESSION_RECOVERY_SHADOW  TUI_MSP_CLIENT  SDK_ENABLED
META_CONTEXT_WORKSPACE_ONLY  NON_STRICT_TOOL_PARAMS  HOOK_SELECTED_SKILLS
HOOK_SELECTED_SKILLS_APPLY  WORKFLOW_API_V2_ROLLOUT  SUBSCRIPTION_LAUNCH
PROVIDER_TOOL_SWITCH  TAG
```

Other TUI-relevant env vars: `TBH_TUI_HYPERLINKS` (OSC-8 hyperlinks; no OSC 8 emitted in the simple
sessions tested, so its effect is unconfirmed), `TBH_TUI_CRASH_FIXTURE`, `MUSE_DISABLE_VOICE_WAVE`,
`MUSE_DISABLE_ULTRA_ANIMATION`, `MUSE_DISABLE_ACTIVE_BANG_SHELL_QUEUE`, `NO_COLOR`, and a large
`TBH_TMUX_*` family used by the tmux-driven e2e harness (`TBH_TMUX_WORKFLOW_OVERVIEW_FIXTURE`,
`TBH_TMUX_TRANSIENT_HINT_CLOCK_DIR`, `TBH_TMUX_EXPORT_FIXED_CLOCK`, …).

---

## 8. Login and payment in the TUI

Source files (from panic metadata): `tui/src/app/login/payment.rs`, `tui/src/login/flow/activation.rs`,
`tui/src/login/payment_poll.rs`, `tui/src/runtime_command_bridge/login/disposition.rs`.

### 8.1 The picker (reproduced offline with `--provider meta`, auth base URL pointed at a dead port)

```
  Muse Code 1.0.1
  Log in with browser · Enter to choose
  Set an API key
  ↓↑ to select · Esc to quit
```

Choosing *Set an API key*:

```
  Muse Code 1.0.1
  Paste your Model API key.
  ⟩  (0 chars)
  Enter validate & save · Esc back
```

### 8.2 The device-code flow, verbatim (string run at `0xb80ab46`)

```
confirm this code matches:
and enter this code:
Couldn't open a browser — copy the URL below.
Sign in at this page:
Waiting for approval…
Esc cancel
Payment required to finish setting up your account.
Press Ctrl+Enter to open billing:
Enter validate & save · Esc back
Validating key…
Enter retry · Esc back
Requesting a device login code…
Enter try again · Esc back
```

Structs: `struct DeviceAuthorization with 6 elements`, `struct TokenGrant with 3 elements`,
`struct OAuthErrorBody`, `struct RevokeBody`, `OAuthTokens{access_token, refresh_token, expires_at,
obtained_via}`, `ProviderCredential{api_key, api_base_url, user_full_name, user_email}`.
App events: `OpenBillingUrlDispatch`, `StartupMintRetryDispatch`, `PaymentOpenBrowserFailed`,
`ResumeDiscovery`. Telemetry events: `payment_banner_shown`, `payment_unresolved_at_poll_cap`,
`subscription_entrypoint_impression`.

### 8.3 Payment / onboarding error copy (`0xbbb7900`)

```
device login isn't available yet
your saved login is no longer valid. Log in again or use a different account.
your Meta account isn't set up for Model API yet — finish onboarding at https://dev.meta.ai, then log in again.
Model API requires a payment method on your account — add one at https://dev.meta.ai, then log in again.
Muse Code requires a payment method — subscribe at https://accountscenter.meta.com/muse_code/?ep=no_payg, then log in again.
stored login is no longer valid
```

and (`0xb81c700`):

```
your login is no longer saved. Try /login again.
startup activation skipped: the stored login vanished before the mint; the action that removed it already spoke
the server couldn't verify this login. Try again or with a different account.
```

### 8.4 Subscription surface (`/upgrade`, gated behind `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH`)

```
upgrade_command
Log in with your Meta account to use a subscription (subscriptions don't apply to manually entered API keys).
No active subscription — you're on pay-as-you-go.
Subscriptions aren't currently available for your account.
You are currently subscribed to the <plan> usage plan.
You are currently subscribed to a usage plan.
<n>% used · Resets at <t>     <n>% used · Resets <t>
Currently unavailable / Usage currently unavailable / Current / Weekly
```

Structs `SubscriptionWeeklySnapshot{used_percent, resets_at}` and
`SubscriptionWindowSnapshot{window_duration_mins}`. Base URL literal: `https://api.meta.ai`;
credential keychain service `muse-code/key`. Voice needs auth too:
`voice authentication unavailable: set META_API_KEY, save a Meta API key, or use /login`.

### 8.5 `/permissions` drawer

```
  Permissions
  1. Read only          Reads files only.
› 2. Ask me (current)   Edits workspace and temporary files; requires approval for protected writes,
                        new network targets, and other restricted actions.
  3. Auto-review        Same access as Ask me; AI reviews eligible actions; trusted permission hooks
                        may decide approval requests first.
  4. Unrestricted       No filesystem sandbox or approval prompts; local commands have direct network
                        access; project trust is unchanged.
```

Internal ids: `:read-only`, `:ask-me`, `:auto-review`, `:unrestricted`.
The YOLO confirmation dialog reads: `Enable unrestricted permissions? / Muse Code can read and edit
any file, access the network from local commands, and run actions without approval prompts. /
Project trust will not change. Processes already running keep the permissions they started with.`

---

## 9. TUI module map (recovered Rust v0 symbol fragments)

90 distinct `tbh_tui::` paths were reconstructed from mangled-symbol fragments still present in the
binary (script scans for `7tbh_tui` then parses the length-prefixed path segments). Full list saved
at `…/scratchpad/tui_symbols.txt`. Highlights relevant here:

```
splash                                          state::client_config::theme::PickerThemeRow
theme_picker::showcase::PreparedThemeShowcase   state::client_config::theme::snapshot::ThemeStartupSnapshot
terminal::background_probe::overlap::TerminalColorProbeOutcome
terminal::background_probe::overlap::spawn_probe_thread
highlight::warm_up                              render::text::markdown::MarkdownBlock
render::tool::preview::file                     render::tool::preview::file::command_diff::CommandDiffSource
render::panels::in_progress::peer_batch::lines  render::mcp_marker::args
file_mentions::WorkspaceFileIndex               subagent_command_center::model::SubagentThreadEntry
subagent_tree::SubagentTreeRowId                usage_summary::main_accumulator::MainRunUsage
workflow_agent_status::WorkflowAgentStatusRow   hook_status::active_group_summary
feedback::submit                                reporting::submit::RageshakeSubmitter
login::flow::activation::mint_with_activation_retry
startup::rules_import_offer::resolve_foreign_context_notice_from_skill_rows
terminal::frame_burst::BurstCore                terminal::write_offload::spawn
terminal::resume_startup_input                  flight_recorder::FlightEntry
```

---

## 10. Answers to the dimension's questions, condensed

1. **Complete slash-command list** — §1.1 (37 built-ins + 10 aliases, verbatim from the binary),
   §1.4 (the 39 rows actually offered, plus the six hidden/gated ones), §1.5 (7 bundled skill
   shortcuts), §1.6–1.7 (`/loop` and the plugin-command format).
2. **Kill-list** — §1.3. `TBH_UNKILL_SLASH_COMMANDS` pairs with a remote `killed_slash_commands`
   field in the gate/catalog document. Nothing is killed in this build, so the killable subset is
   empirically empty here.
3. **Keybindings** — §2. 4 contexts, 37 actions, full default table, fully user-configurable via
   `tui.keymap` and via the `/keymap` overlay's Replace/Unbind/Reset flow.
4. **Status line** — §5. Shows `provider · [effort ·] cwd [· YOLO]`; `/status` adds
   MODEL/WORKSPACE/ACCESS/USAGE/CONTEXT/SESSION/ACTIVITY (+ ACCOUNT/NOTICE/Scheduled jobs).
   **Not configurable** — no template, format, or component setting exists.
5. **Theming** — §3. **Muse Code has a first-class theme system**: 24 compiled themes, light/dark
   auto-selection from an OSC-11 probe, a `Dynamic` theme derived from the terminal palette, a live
   preview picker, colour-depth downgrade, and a **user theme directory
   `~/.config/muse/themes/*.tmTheme`** selected as `tui.theme = "custom:<stem>"`. This is the single
   most oh-my-shaped surface in the product.
6. **Startup banner** — §6. Minimal (`  Muse Code 1.0.1`); splash machinery exists but is not
   reachable offline; not configurable.
7. **Reasoning display** — §7. `MUSE_EXPERIMENTAL_REASONING_DISPLAY=1` reveals the
   `tui.show_reasoning` toggle in `/settings → Appearance`.
8. **Login / payment** — §8. Device-code + API-key picker, payment-required banner with
   `Ctrl+Enter to open billing`, `/upgrade` behind `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH`.

---

## 11. Reproduction cheat-sheet

```bash
S=/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad
export MUSE_NO_AUTO_UPDATE=1 HOME=$S/sandbox/tui/home XDG_CONFIG_HOME=$HOME/.config

# static
python3 - <<'EOF'   # dump the slash table
d=open('/…/muse-aarch64-macos','rb').read(); print(d[0xb806baf:0xb806baf+2400].decode('utf8','replace'))
EOF
python3 - <<'EOF'   # parse the theme catalog (see §3.2)
# magic MCTHM001 @ 0xb9d0bc7, +8 magic, +100 header, u64 count=24, then records
EOF

# runtime (needs the pty responder)
python3 $S/sandbox/tui/drive.py <script.json> out.bin && python3 $S/sandbox/tui/render.py out.bin

# schema oracle
muse config validate --plane defaults --file doc.json
```

---

# Verification

Adversarial re-run by a second agent, independent harness
(`sandbox/verify-tui-slash-theme/{drv.py,scr.py}` — pty + OSC 10/11/4 + DA1 + CPR + kitty responder,
screen modelled with `pyte`, all runs `--provider echo` under sandboxed `HOME`/`XDG_*`).
Every claim below was re-derived from the binary or re-driven in a fresh pty; nothing was taken on
trust. **Verdict: MOSTLY_SOLID.** The theme catalog, the keymap model, the settings schema, the
status line and the runtime menus all reproduce byte-for-byte. Five things do not.

## V.1 Refutations

### R1 — "37 built-in commands with 10 aliases" is a double miscount (§1.1, claim 1)

The literal run at `0xb806baf` contains **45 slash tokens and 36 descriptions**:

```
python3: run = d[0xb806baf : d.find(b'TBH_TMUX_TRANSIENT_HINT_CLOCK_DIR', 0xb806baf)]
         re.findall(r'/[a-z][a-z-]*', run)  ->  45 tokens, 45 unique
```

36 canonical commands + **9** aliases (`/btw /models /config /memory /quit /cost /context /ts /ps`).
The report's own table lists those same 9 aliases while the prose says 10, and it reaches "37" by
counting `/login` twice (row 1 *and* "the 36 rows above `/import` plus `/login`"). Correct figure:
**36 built-ins, 9 aliases**.

### R2 — `TELEMETRY_DROPPED_SLASH_COMMAND_NAMES ⊇ {"/login","/name"}` is REFUTED (§1.2, claim 2)

The inference was drawn from string adjacency, but the actual `&[&str]` lives in `__DATA_CONST` and
can be read directly. Pointer-array walk from `0xe160300` (16-byte `(ptr,len)` entries, `vmaddr =
fileoff + 0x100000000`) yields, at indices 15–51, **37 names**:

```
/login /logout /clear /new /name /resume /fork /side /init /deep-research /subagents /model
/settings /keymap /help /theme /rules /compact /export /copy /recap /skills /plugins /skill
/effort /goal /feedback /voice /exit /quit /stop /status /usage /upgrade /permissions /tasks
/workflows
```

`/name` is entry **19**, whose pointer resolves to `0xba30ed6` (`…cted stageevent.name/name`) — it is
simply pooled into a *different* literal run, which is why it looked absent from the `/logout…/workflows`
blob. `/login` is entry 15 at `0xba31bfe`, four NUL alignment bytes before the blob, i.e. inside the
array. **Nothing is demonstrably dropped**; the emitted vocabulary is 37 names and it includes the
alias `/quit` (which the TUI table treats as an alias of `/exit`).

### R3 — `tui.keymap.app.commands` is NOT unmodifiable (ohmy_hooks #2; open question 2)

The rejection has nothing to do with reservation or a retired-action list. The binary states the rule
verbatim at `0xcbbd1a3`:

```
`…`.`…`) must use one printable character binding
```

`app.commands`, `app.shell`, `app.files` are *route-prefix* actions and accept exactly one printable
character. Validator grid (`muse config validate --plane defaults --file X`, `schema_version:1` required):

```
app.commands = [";"] [ "#" ] [ "%" ] [ "^" ] [ "/" ] [ "[" ]   -> valid
app.commands = ["ctrl+q"] ["alt+k"] ["esc"] ["tab"] ["enter"]  -> semantic_invalid
app.shell / app.files = [";"]                                  -> valid
app.keymap-short = ["ctrl+q"]                                  -> valid   (NOT route-reserved)
```

And it works end to end. With `~/.config/muse/settings.json` =
`{"schema_version":1,"tui":{"keymap":{"app":{"commands":[";"]}}}}`:

```
⟩ ;            -> slash picker opens (31 rows)
⟩ /            -> nothing; "/" is just a character now
```

(Reproduced on three consecutive launches of the same HOME.) The report's open question about
`route-reserved` is separately answerable too: `navigation.focus-plan` rejects exactly the four route
characters `/ ! @ ?` and accepts `;`, `alt+up`, `ctrl+q`.

### R4 — Interactive rebinding DOES commit (open question 3)

Driven in a pty: `/keymap` ⏎ → type `clear` (filter) → ⏎ (row detail) → ⏎ on **Replace binding** →
type `ctrl+q` → ⏎. `settings.json` afterwards:

```json
{ "schema_version": 1, "tui": { "keymap": { "app": { "clear-terminal": ["ctrl+q"] } } } }
```

and the detail pane redraws as `Replace binding  ctrl+q` / `Reset to default  ctrl+l`. The earlier
harness most likely sent ⏎ while the *filter* still had focus. The UI-driven path is proven.

### R5 — The `0xb88efb5` extension table is misread (§3.7, claim 26)

`json sublime-settings sublime-menu … sublime-macro sublime-color-scheme ipynb Pipfile.lock` is not a
list of asset formats the highlighter accepts. Read with its length prefixes it is the **file-extension
list of the bundled JSON syntax definition inside a syntect `SyntaxSet` dump**:

```
"JSON"  count=14  ["json","sublime-settings",…,"sublime-color-scheme","ipynb","Pipfile.lock"]
"source.json"  <zlib blob 0x78 0xda …>
```

The claim that "the highlighter also accepts sublime-syntax/tmTheme assets" is unsupported by that
evidence. (tmTheme ingestion *is* independently proven by §3.4.)

### R6 — The `Dynamic` derivation uses palette slot **1** and slot **12** (open question 8)

Answering the OSC-4 probes with a deliberately unusual palette (slot 1 `#ff0000`, slot 9 `#ff3366`,
slot 4 `#0066ff`, slot 12 `#3399ff`) and running `tui.theme = "dynamic"`:

```
emitted: 38;2;255;0;0   (= slot 1, normal red)
         38;2;51;153;255 (= slot 12, bright blue)
         38;2;208;208;208 (= the OSC-10 foreground)
```

Slot 9 and slot 4 never appear. So Dynamic keys off **fg + palette[1] + palette[12]**, not "blue 4/12,
red 9".

### R7 — Smaller factual corrections

| Report says | Actually |
|---|---|
| "a second copy [of the slash table] at `0xb848486`" | `/skills/skill/login` occurs **once**. `0xb848486` is a *partial* duplicate covering only `…theme…/export…` |
| §1.8 "the drawer shows 6 rows at a time" | The **slash** drawer shows **7** rows (7 + `↓ 32 more` = 39) at 50 *and* 90 terminal rows. Only the `/keymap` overlay shows 6 |
| §5.2 "the complete set of section labels is a single literal run" | `ACTIVITY`, which renders live, is **not** in that run — it sits in an 8-byte-aligned pool at `0xcb92b28` |
| §3.2 `manifest_digest = be3304008af86c24…` | `be330400` is a separate u32 (`275390`) *before* the three 32-byte digests; digest #1 is `8af86c24b5c458bd…`. Whole blob = 275434 B from the magic |
| claim 21 evidence `muse plugins validate ohmy-plugin` | Requires the gate: without it every `muse plugins` subcommand prints `plugins are not available in this build`. (The *conclusion* — plugin commands work without the gate — is correct and reproduced.) |
| §4 `…expected "auto", "disabled", or a positive integer row…` | Full text is `…or a positive integer row count` |
| §3.5 offsets (`0xb81be9e`) etc. | Several quoted offsets are a few hundred bytes off (`theme hydration starts at most once` is at `0xb81b77b`). Strings are all present; offsets are unreliable |

## V.2 Confirmed byte-for-byte / run-for-run

* Slash table literal at `0xb806baf`, verbatim (modulo R1's counts). `/side [prompt]` is real —
  confirmed in `/help → Commands`, so the pooled `[prompt]` is genuinely shared with `/fork`.
* The 39-row picker and its exact order (stepped ↓ at N=0/6/12/18/24/30/36/38); `/permissions`,
  `/skill`, `/exit` prefix-only; `/login` unreachable on echo; `MUSE_EXPERIMENTAL_PLUGINS=1` → `/plugins`,
  `MUSE_EXPERIMENTAL_SUBSCRIPTION_LAUNCH=1` → `/upgrade`; aliases rendered `canonical (alias)`.
* `TBH_UNKILL_SLASH_COMMANDS`: **exactly one** adrp+add xref in `__text`, found by scanning all
  0x2b23f8 text words — `adrp x0, 0x10bb9a000` at `0x105f58258`, `add x0, x0, #0xe2f` at `0x105f5825c`,
  `mov w1, #25`. (The report anchored on the `add`, one instruction late.) All five values tried
  leave the menu at `↓ 32 more`.
* Keymap literal at `0xb7f7828` verbatim; 12 + 8 + 14 + 3 = **37** actions; `/keymap` header
  `All configurable shortcuts - 37 rows`, `Common` = `Most-used shortcuts - 16 rows`, 8 tabs,
  row-detail Replace/Unbind/Reset.
* `tui.keymap` map schema, `[]` = unbind, and the `Customized` tab showing
  `User overrides - 2 rows` / `Clear terminal  ctrl+q / alt+j` / `Transpose characters  unbound`.
* All twelve `config validate` results reproduce exactly.
* **The 24-theme `MCTHM001` catalog parses byte-exactly** — same ids, display names, source files,
  15 dark / 9 light, same fg/bg/accent *including* the three alpha values (`rose-pine-dawn` α20,
  `rose-pine-moon` α38, `tokyo-night` α51) and same scope counts (33 … 276). The parse terminates
  precisely at the end of the blob, which is the strongest possible check on the layout.
* `/theme` filtered by OSC-11 (17 dark rows vs 11 light rows); custom-theme discovery rules
  (`$XDG_CONFIG_HOME/muse/themes`, `.tmTheme`/`.tmtheme`/`.TMTHEME`, `.json` ignored, data dir ignored)
  and `"tui":{"theme":"custom:ohmy"}` written on save.
* Colour plumbing: `nord` → `38;2;129;161;193` + `38;2;191;97;106`; `synthwave-84` → `38;2;254;222;93`
  + `38;2;254;68;80`; `256` → only `38;5;N`; `16` → one colour; `none` and `NO_COLOR=1` → zero.
* Both theme-fallback notices; `TuiSettings` 14-field list; all six domain-error strings.
* Status line `echo · <cwd>[· YOLO]`, `/status` panel, one-line banner, workspace-trust gate,
  `?` overlay, `/help` three tabs, `/permissions` four profiles, prompt-hint catalog (22 tips,
  stale `/agent`), login picker + API-key screen, startup probe byte sequence, 90 `tbh_tui::` paths,
  theme-hydration strings, splash literals with no offline render.

## V.3 Ground the report missed

1. **The enterprise `defaults` plane can pin 11 TUI keys.** `struct TuiDefaultsV1 with 11 elements`
   (`0xbb99e9d`) and the surface list in the same region name them:
   `settings.tui.{show_reasoning, reasoning_summaries, away_recap_enabled, voice_enabled,
   voice_tap_enabled, voice_shortcut_mode, keymap, resize_reflow_cap, color_depth, theme,
   verbose_output}`. **`terminal_background` and `prompt_hint_enabled` are NOT pinnable.**
   `muse config status` names the two source classes — `system_file` and
   `macos_managed_preferences`, both `absent` here. This is a real oh-my/MDM lever (a machine-wide
   theme + keymap) that §2.3/§4 only treated as a validator.
2. **The key-spec grammar, enumerated.** Accepted: any single printable char; `ctrl|alt|shift|super|
   cmd|meta` (case-insensitive, stackable — `ctrl+alt+q`, `ctrl+shift+q` both valid) plus a key;
   named `enter esc tab backtab home end pageup pagedown delete backspace up down left right`.
   Rejected: **all function keys** (`f1`, `shift+f5`), `insert`, `space`, `ctrl+space`, `win+q`,
   media keys, and a spec repeated inside one action's own list. Backing literals at `0xcbbd0c0`:
   `unknown keymap context:`, `route-reserved binding …`, `invalid key binding … for ….…`,
   `duplicate binding … in ….…`, `key binding … is used by both … and …`,
   `unsupported key binding …`, `missing key in binding …`, `….…) must use one printable character binding`.
3. **Rebinding `app.commands` silently costs you the skill and plugin rows.** With
   `app.commands=[";"]`, the picker opened with `;` lists **31 rows** — the 7 bundled skill shortcuts
   and `/loop` are gone (39 → 31). Reproduced on three launches, and on a long 18 s warm-up, so it is
   not hydration timing. Any oh-my keymap preset that moves the command prefix breaks skill/plugin
   discovery in the picker.
4. **The `YOLO` badge tracks the *unrestricted profile*, not the `--yolo` flag.**
   `--disable-approval --disable-sandbox --trust-workspace` → `echo · <ws> · YOLO`; either flag alone
   → no badge. (`--permission-profile unrestricted` is rejected: `Permission profile 'unrestricted'
   is unavailable: profile does not exist.`)
5. **The status line abbreviates the cwd when it needs room** — inside `/side` it renders
   `echo · /p/t/c/-/1/s/s/verify-tui-slash-theme/ws` — and the composer divider gains a prefix
   (`── Side Chat · Voice input (⌥ + v to start) ──`).
6. **No gate other than PLUGINS and SUBSCRIPTION_LAUNCH adds a browsable command.** With 24
   `MUSE_EXPERIMENTAL_*` gates exported at once the menu is 41 rows = 39 + `/plugins` + `/upgrade`.
   Useful negative result for the "gate profile" hook.
7. **The catalog's serialization is bincode-with-fixint** (u64 lengths, 1-byte `Option` tags, u32 enum
   discriminants — `font_style` is a single trailing byte per scope, values seen `{0,1,2,3,4,6,8}`).
   That resolves open question 6: the 8 bytes after `source_file` are **two u32s** — `source_mode`
   (0 = dark, 1 = light) and `license`, which is **0 for all 24 entries**. Not "mode plus 7
   unexplained zero bytes".
8. **`color_depth: 16` still emits `38;5;N` (N < 16)**, not legacy 30-37/90-97 SGR — relevant to any
   theme pack that tries to degrade gracefully.
9. **The embedded syntect `SyntaxSet` carries 216 length-prefixed syntax scopes** (`source.rust`,
   `source.zig`, `source.typst`, `text.html.markdown`, `text.git.commit`, … ≈200 languages — the
   bat/two-face extended set). That, not the JSON extension list, is the real evidence for transcript
   code-block highlighting.
10. **`tui.terminal_background` really does override the probe** — the report asserted it as an
    ohmy hook without a runtime test. Proven: `{"tui":{"terminal_background":"light"}}` with the pty
    answering OSC 11 `rgb:1010/1010/1010` still shows only the 9 light themes in `/theme`.
11. **`/help → Commands` carries the argument hints** for every browsable row
    (`/loop [interval] <prompt>`, `/import <session-id-or-path>`, `/side [prompt]`, …) and, like the
    composer picker, omits `/permissions`, `/skill`, `/exit`, `/login`, `/plugins`, `/upgrade`.
12. **No CLI flag touches the TUI's appearance.** `muse --help` has no `--theme`, `--color`,
    `--no-color` or keymap flag; the only TUI-visible flags are the safety ones
    (`--yolo/--trust-workspace/--disable-approval/--disable-sandbox/--approval-mode`).
