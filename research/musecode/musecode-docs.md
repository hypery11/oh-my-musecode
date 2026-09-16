# Meta Muse Code — the OFFICIAL and press-level picture

**Purpose:** establish what Meta *tells* users Muse Code is and can be customised into, so the
reverse-engineering of `muse-aarch64-macos` 1.0.1-R2006.1 can be cross-checked against it, and so
the gaps between "documented" and "shipped" can be turned into an `oh-my-musecode` roadmap.

**Date of research:** 2026-09-01. All URLs fetched live on that date.

**Method note.** `dev.meta.ai/docs/*` is served behind Facebook's edge, which 403s plain `curl`
(returns a 1,542-byte `Sorry, something went wrong` page). Every `dev.meta.ai` quote below came
through a browser-shaped fetch. `musecodes.io` is a static Astro site and was scraped directly.
Where a claim is a cross-check against the local binary/RE artefacts rather than a published
source, it is labelled **[RE]** and points at the file in
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/re/`.

---

## 0. Source inventory

### First-party (Meta)

| Source | URL |
|---|---|
| Product page | https://developer.meta.com/ai/products/muse-code/ |
| Launch blog (model + agent) | https://developer.meta.com/ai/resources/blog/build-with-muse-code/ |
| GA blog ("New plans and features") | https://developer.meta.com/ai/resources/blog/muse-code-new-plans-and-features/ |
| Research blog | https://research.meta.ai/blog/introducing-muse-code-and-muse-spark-1-2 |
| Model API docs overview | https://dev.meta.ai/docs/overview/ |
| Muse Code doc hub | https://dev.meta.ai/docs/muse-code/ |
| Auth & billing | https://dev.meta.ai/docs/muse-code/auth/ |
| Permissions & safety | https://dev.meta.ai/docs/muse-code/permissions/ |
| Working with the agent | https://dev.meta.ai/docs/muse-code/interactive/ |
| Workflows | https://dev.meta.ai/docs/muse-code/workflows/ |
| Session messaging | https://dev.meta.ai/docs/muse-code/session-messaging/ |
| Rewind | https://dev.meta.ai/docs/muse-code/rewind/ |
| **Configuration and context** | https://dev.meta.ai/docs/muse-code/configuration/ |
| **Extending and automating** | https://dev.meta.ai/docs/muse-code/extending/ |
| Changelog | https://dev.meta.ai/docs/muse-code/changelog/ |
| Subscriptions | https://dev.meta.ai/docs/muse-code/subscriptions/ |
| Pricing & rate limits | https://dev.meta.ai/docs/pricing-rate-limits/ |
| Models | https://dev.meta.ai/docs/models/ |
| Third-party agent setup | https://dev.meta.ai/docs/coding-agents/ |
| Muse Glimmer (open weights) | https://dev.meta.ai/docs/muse-glimmer/ |
| Release channel endpoint | https://api.meta.ai/muse-code/channels/muse-stable |
| Installer | https://dev.meta.ai/install.sh |
| Launcher | https://api.meta.ai/muse-launcher.sh |
| AI at Meta announcement | https://x.com/AIatMeta/status/2085084709277565213 |
| Zuckerberg GA post | https://x.com/finkd/status/2094500475710099945 |

### Community / third-party

| Source | URL |
|---|---|
| musecodes.io docs (unofficial mirror/guide) | https://musecodes.io/docs/ |
| musecodes.io pricing | https://musecodes.io/pricing/ |
| musecodes.io compare | https://musecodes.io/compare/ |
| musecodes.io changelog | https://musecodes.io/changelog/ |
| musecodes.io — bundled skills | https://musecodes.io/blog/bundled-skills/ |
| musecodes.io — event log | https://musecodes.io/blog/event-log-replay/ |
| musecodes.io — fan-out & worktrees | https://musecodes.io/blog/agent-fanout-worktrees/ |
| musecodes.io — contributor vs standard | https://musecodes.io/blog/contributor-vs-standard/ |
| TechCrunch launch | https://techcrunch.com/2026/08/05/meta-launches-muse-code-an-ai-agent-for-large-code-bases/ |
| CNBC | https://www.cnbc.com/2026/08/05/meta-debuts-muse-code-to-take-on-anthropic-and-openai-.html |
| Engadget | https://www.engadget.com/2231285/meta-introduces-muse-code-its-take-on-a-coding-agent/ |
| 9to5Mac | https://9to5mac.com/2026/08/05/meta-launches-muse-code-ai-coding-agent-for-macos-and-linux/ |
| The New Stack | https://thenewstack.io/meta-muse-code/ |
| Neowin (GA) | https://www.neowin.net/news/meta-graduates-muse-code-out-of-beta-with-new-features/ |
| Codersera — complete guide | https://codersera.com/blog/muse-code-complete-guide-2026/ |
| Codersera — install guide | https://codersera.com/blog/how-to-install-muse-code-cli-2026/ |
| SitePoint — getting started | https://www.sitepoint.com/meta-muse-code-getting-started/ |
| SitePoint — setup/pricing/first look | https://www.sitepoint.com/muse-code-meta-terminal-coding-agent/ |
| Layer3Labs — explained | https://www.layer3labs.io/guides/muse-code-explained |
| EveryDev.ai | https://www.everydev.ai/tools/muse-code |
| Composio — vs Claude Code | https://composio.dev/content/muse-code-vs-claude-code |
| Verdent | https://www.verdent.ai/guides/agents/what-is-muse-code |
| QAInsights — CLI reference | https://qainsights.com/getting-started-with-muse-code-cli-commands-syntax-and-purpose/ |
| explainX | https://www.explainx.ai/blog/meta-muse-code-coding-agent-muse-spark-1-2-launch-august-2026 |
| MindStudio (skills/plugins/hooks framing) | https://www.mindstudio.ai/blog/how-to-use-ai-agent-skills-plugins-claude-code-codex |

> SitePoint and Neowin are behind Cloudflare / 403 for automated fetches; their content below is
> sourced from search-result extracts, which is noted inline.

---

## 1. The official product picture

### 1.1 Timeline

- **2026-08-05** — Muse Code ships **in beta**, macOS + Linux, alongside **Muse Spark 1.2**
  (https://techcrunch.com/2026/08/05/meta-launches-muse-code-an-ai-agent-for-large-code-bases/,
  https://research.meta.ai/blog/introducing-muse-code-and-muse-spark-1-2,
  https://musecodes.io/changelog/).
- **2026-08-07** — comparison snapshots dated by musecodes.io "as of August 7, 2026"
  (https://musecodes.io/compare/).
- **2026-08-10** — Codersera reports Meta announced intent to open-source Muse Spark weights,
  date TBD (https://codersera.com/blog/muse-code-complete-guide-2026/).
- **GA** — "Muse Code is out of beta and now built to handle bigger, more complex engineering
  tasks" (https://x.com/finkd/status/2094500475710099945); GA blog adds session messaging,
  Workflow, Rewind, and an **SDK in developer preview**
  (https://developer.meta.com/ai/resources/blog/muse-code-new-plans-and-features/,
  https://www.neowin.net/news/meta-graduates-muse-code-out-of-beta-with-new-features/).

### 1.2 What Meta says it is

> "Muse Code is Meta's coding agent for the terminal and CI" that can "plan, edit, and run
> commands to do a task, with approvals and an OS sandbox."
> — https://dev.meta.ai/docs/muse-code/

> "A coding agent for your most complex coding workstreams. Build, debug and ship with Muse Code."
> — https://developer.meta.com/ai/products/muse-code/

Zuckerberg, quoted by TechCrunch: it completes "complete software engineering tasks across large
repos," including "planning changes, writing code, validating the results"; "When a job is big
enough, it fans out to separate sub-agents working in parallel in isolated worktrees… Your working
copy is never touched. In testing we had it build six features for a game simultaneously with no
collisions."
(https://techcrunch.com/2026/08/05/meta-launches-muse-code-an-ai-agent-for-large-code-bases/)

Alexandr Wang (Meta Superintelligence Labs): "We think that for a lot of workflows and a lot of use
cases, this can be an incredibly good option, especially from a cost perspective." (same source)

### 1.3 The official feature list

Consolidated from https://developer.meta.com/ai/products/muse-code/,
https://research.meta.ai/blog/introducing-muse-code-and-muse-spark-1-2, and
https://musecodes.io/docs/:

1. **Multi-agent by default.** "Multiple agents coordinate on every task. Workers in parallel,
   reviewers in the background."
2. **Async background agents.** "These specialized agents remain active throughout each session
   rather than being spawned per task, which avoids redundant information gathering. They carry out
   next steps on their own and choose when to communicate back to the main agent."
   (https://musecodes.io/blog/async-background-agents/)
3. **Agent fan-out into git worktrees.** "the parent spawns a write-capable child per task, and each
   child gets its own git worktree." Worktrees are created under **`.muse/worktrees/`** in
   detached-HEAD state (https://developer.meta.com/ai/resources/blog/build-with-muse-code/,
   https://musecodes.io/blog/agent-fanout-worktrees/).
4. **Append-only local event log.** "every model call, tool run, approval, and edit is appended…
   replay-exact and restart-safe." Stored as "plain JSONL on your disk." Worktree lifecycle stages
   logged as `operation_requested → prepared → lease_active → workspace_scope_activated`
   (https://developer.meta.com/ai/resources/blog/build-with-muse-code/,
   https://musecodes.io/blog/event-log-replay/).
5. **Bundled skills / slash commands.**
6. **OS sandbox + approvals on by default.**
7. **Headless / CI mode** (`muse exec`).
8. **Multimodal**: image and video upload, voice mode, web search
   (https://developer.meta.com/ai/products/muse-code/).
9. **Session messaging** — "Your sessions can now deliver messages to each other" over **Unix
   sockets** (https://developer.meta.com/ai/resources/blog/muse-code-new-plans-and-features/).
10. **Workflow** — "orchestrates large teams of subagents to solve complex engineering tasks
    quickly" (same).
11. **Rewind** — double-`Esc` conversation rollback (same;
    https://dev.meta.ai/docs/muse-code/rewind/).
12. **SDK (developer preview)** — "Muse Code is now programmable" as a TypeScript library over the
    **Muse Session Protocol (MSP)**, "a JSON-over-stdio contract with a generated schema. There is
    no server, no network call: everything runs locally." (same blog + search extract).

### 1.4 Platform support — as documented

> "a native binary on your path for **macOS and Linux**."
> — https://dev.meta.ai/docs/muse-code/

> "Muse Code is in beta for **macOS and Linux**."
> — https://musecodes.io/docs/

Codersera: "macOS and Linux on x86_64 and arm64… Windows requires WSL2"; the installer "hard-fails
with `unsupported platform`" (https://codersera.com/blog/how-to-install-muse-code-cli-2026/).
9to5Mac's headline is literally "for macOS and Linux"
(https://9to5mac.com/2026/08/05/meta-launches-muse-code-ai-coding-agent-for-macos-and-linux/).

**This is contradicted by the shipping release manifests — see §6.2.**

### 1.5 Muse Spark 1.2 — model details

From https://research.meta.ai/blog/introducing-muse-code-and-muse-spark-1-2 and
https://musecodes.io/docs/:

- "a coding-focused update to Muse Spark 1.1, with improvements in code generation, complex
  debugging, codebase understanding, and end-to-end developer workflows"; Meta "significantly
  scaled up training compute on coding tasks while expanding training environment diversity."
- **Co-trained with the harness**: "rejection sampled harness trajectories and recipe optimizations
  for goals, compaction, and subagents." The launch blog adds "Muse Code was in the training loop
  from day one, so tool calls succeed and plans execute cleanly," and that the model was "trained
  across multiple harnesses"
  (https://developer.meta.com/ai/resources/blog/build-with-muse-code/).
- **Long-horizon**: "whole-repository generation, large end-to-end projects, and auto-research."
- **Self-improvement loop**: 1.1 generated environments and graded candidate solutions to train 1.2.
- **Context window 1,048,576 tokens** (https://dev.meta.ai/docs/models/, https://musecodes.io/docs/).
- **Max output 131,072 tokens** — not on the models page; published on the OpenCode config example
  (`"limit": {"context": 1048576, "output": 131072}`,
  https://dev.meta.ai/docs/coding-agents/) and repeated by
  https://codersera.com/blog/muse-code-complete-guide-2026/.
- **Modalities**: text, image, video, PDF in; text out (https://dev.meta.ai/docs/models/).
- **No published knowledge cutoff, parameter count, or architecture**
  (https://codersera.com/blog/muse-code-complete-guide-2026/).

Model IDs (https://dev.meta.ai/docs/models/):

| Model ID | Tier | In | Out | Context |
|---|---|---|---|---|
| `muse-spark-1.1` | Standard | text, image, video, PDF | text | 1,048,576 |
| `muse-spark-1.2` | Standard | text, image, video, PDF | text | 1,048,576 |
| `muse-spark-1.2-contributor` | Contributor | text, image, video, PDF | text | 1,048,576 |
| `muse-image-1.0` | — | text, image | image | — |

`muse-spark-1.2` is "an updated checkpoint with slightly higher performance" vs 1.1; the
contributor variant is *the same checkpoint* at discounted rates — **a privacy tier, not a
capability tier** (https://dev.meta.ai/docs/models/, https://musecodes.io/blog/contributor-vs-standard/).

**Muse Glimmer** — a separate 30B **open-weight, Apache-2.0** model, on HuggingFace as
`meta-models/Muse-Glimmer-30B`, runnable on vLLM, SGLang, llama.cpp and ExecuTorch
(https://dev.meta.ai/docs/overview/, https://dev.meta.ai/docs/muse-glimmer/).

**Benchmarks** (Meta's own numbers, via https://musecodes.io/docs/ and
https://musecodes.io/compare/):

| Agent + model | Terminal-Bench 2.1 | DeepSWE 1.1 | Meta Internal Bench |
|---|---|---|---|
| Claude Code + Opus 5 (max) | 86.7% | 65.0% | 79.4% |
| **Muse Code + Muse Spark 1.2** | **82.9%** | **59.3%** | **70.6%** |
| Codex + GPT-5.6 Terra (max) | 81.8% | 64.8% | — |
| Grok Build + Grok 4.5 (high) | 81.6% | — | — |
| Antigravity CLI + Gemini 3.6 Flash | 78.9% | — | — |
| Muse Spark 1.1 + mini-swe-agent | 76.2% | — | — |

Meta published its own second-place result. Internal-bench numbers via
https://codersera.com/blog/muse-code-complete-guide-2026/. GDPVAL is named as a fourth suite in the
launch blog (https://developer.meta.com/ai/resources/blog/build-with-muse-code/).

**Kernel case study**: "1,000+ tool calls — up to 24 hours," optimising Triton KDA/MLA kernels for
NVIDIA Hopper, barred from wrapping third-party kernel libraries (https://musecodes.io/docs/).

### 1.6 Pricing — two independent pricing systems

**(a) Token pricing, Meta Model API** — https://dev.meta.ai/docs/pricing-rate-limits/,
mirrored at https://musecodes.io/pricing/:

| Per 1M tokens | Contributor (`muse-spark-1.2-contributor`) | Standard (`muse-spark-1.2`) |
|---|---|---|
| Cached input | **$0.002** | **$0.15** |
| Input | **$0.10** | **$1.25** |
| Output | **$0.20** | **$4.25** |
| Data used for training | **May be used** | **Never** |
| Per-repo sharing opt-out | Available | n/a |
| Spend caps & usage export | Included | Included |

Muse Image: **$0.01 per generated image**, flat, web/image search included.

**The verbatim tier definitions** (https://dev.meta.ai/docs/pricing-rate-limits/):

> Standard: "Standard pricing; your prompts and completions are not used to train Meta models."
>
> Contributor: "Heavily discounted token pricing **in exchange for permission to use your prompts
> and completions to train future Meta models**."

Zero data retention: "Meta is beginning to accept requests for zero data retention. Contact Meta
sales." (https://musecodes.io/pricing/, echoed in the launch blog).

**(b) Flat monthly subscriptions** — https://developer.meta.com/ai/products/muse-code/ and
https://dev.meta.ai/docs/muse-code/subscriptions/:

| Plan | Price | Notes |
|---|---|---|
| Everyday Usage | **$5/mo** | "Send 10-50 requests every 5 hours, including image and video uploads"; Muse Spark 1.2 |
| High Usage | **$15/mo** | "3x more usage than the Everyday Usage plan"; latest models |
| Power Usage | **$50/mo** | "10x more usage than the Everyday Usage plan"; early access to new features |

Upgrades prorate immediately; downgrades take effect next cycle
(https://dev.meta.ai/docs/muse-code/subscriptions/).

> **Doc conflict:** https://musecodes.io/pricing/ still says "No seats, no subscriptions, no
> minimums," and Codersera's launch-window guide lists "No subscription tier" under *Missing
> features* (https://codersera.com/blog/muse-code-complete-guide-2026/). Subscriptions landed at
> GA. The `dev.meta.ai` subscriptions page names the three plans but **prints no prices** — only
> the marketing product page does.

### 1.7 Rate limits

https://dev.meta.ai/docs/pricing-rate-limits/:

| Tier | RPM | TPM |
|---|---|---|
| Standard | 3,000 | 4,000,000 |
| Contributor | **100** | 3,000,000 |
| Muse Image | 150 | (no TPM limit) |

> "Limits apply **per team, not per API key**."

Codersera's read is the important one: "Rate limits, not price, are the binding constraint on the
cheap tier. 100 RPM against 3,000 RPM is a **30x throughput cut**."
(https://codersera.com/blog/muse-code-complete-guide-2026/)

The launch blog frames Contributor differently again — "rate-limited by tokens in a rolling 5-hour
window, not by request count"
(https://developer.meta.com/ai/resources/blog/build-with-muse-code/) — which does **not** match
the docs' 100 RPM figure.

### 1.8 The privacy trade-off, stated plainly

- Contributor data "may be used to improve Meta's products"; standard "is never used to improve
  Meta's products" (https://musecodes.io/pricing/).
- The gap "is not a capability tier. You aren't buying a smarter model with standard — you're
  buying a privacy posture." (https://musecodes.io/blog/contributor-vs-standard/)
- Contributor is available in **select countries only**, and — per Verdent —
  "Muse Code **defaults to the Contributor Tier after install**, meaning you must actively switch
  to Standard to opt out of data-for-training"
  (https://www.verdent.ai/guides/agents/what-is-muse-code). *This default is not stated on any
  Meta page I could find — treat it as a third-party claim worth verifying.*
- Codersera's warning: avoid Contributor for proprietary code — "once code is in the weights, no
  deletion request unwinds it" (https://codersera.com/blog/muse-code-complete-guide-2026/).
- `/model` switches tier mid-session; "usage bills at whichever model served the tokens"
  (https://musecodes.io/pricing/).

---

## 2. The DOCUMENTED configuration surface

Everything in this section is what Meta publishes. §6 lists what it omits.

### 2.1 Files and directories Meta names

| Path | Role | Source |
|---|---|---|
| `~/.config/muse/settings.json` | the **only** user settings file | https://dev.meta.ai/docs/muse-code/configuration/ |
| `AGENTS.md` | project instructions, scaffolded by `muse init` | same |
| `CLAUDE.md` | fallback project instructions | same |
| `.agents/AGENTS.md` | third in precedence | same |
| `.claude/CLAUDE.md` | fourth in precedence | same |
| `.agents/memory/MEMORY.md` + topic `.md` files | project memory index (≤48 files load at session start) | same |
| `$XDG_CONFIG_HOME/muse/skills` | user skills (managed root) | https://dev.meta.ai/docs/muse-code/extending/ |
| `~/.agents/skills` | user skills | same |
| `~/.claude/skills` | **foreign** user skills (Claude Code) | same |
| `$CODEX_HOME/skills` → `~/.codex/skills` | **foreign** user skills (Codex) | same |
| `<repo>/.agents/skills/<skill-id>/SKILL.md` | project skills | same |
| `<repo>/.codex/skills`, `<repo>/.claude/skills` | foreign project skills | same |
| `<project-root>/.muse/hooks.json` | **claimed** project hooks file | same |
| `.agents/workflows/<name>.js` | project-scope saved workflow | https://dev.meta.ai/docs/muse-code/workflows/ |
| `.muse/worktrees/` | subagent worktrees | https://developer.meta.com/ai/resources/blog/build-with-muse-code/ |
| `~/.local/bin/muse` | self-updating launcher | https://codersera.com/blog/how-to-install-muse-code-cli-2026/ |
| `~/.local/bin/muse-bin-<version>` | the real binary | same |
| `~/.local/share/muse/sessions/YYYY/MM/DD/` | session store | https://qainsights.com/getting-started-with-muse-code-cli-commands-syntax-and-purpose/ |

### 2.2 `settings.json`

Requirement, stated bluntly:

> The file must include `"schema_version": 1` or startup will fail with a malformed settings error.
> A missing file is acceptable since defaults apply automatically.
> — https://dev.meta.ai/docs/muse-code/configuration/

Documented top-level sections (the docs describe them, they do not print a full schema):

- model defaults
- terminal UI preferences (voice, reasoning display)
- tool configuration (workflow selection, session messaging)
- `mcp_servers` (and, per changelog 0.2.1, the alias `mcpServers`)
- `hooks` block and `managed_hooks_path`
- `runtime_capabilities` map for optional features
- telemetry options
- `agents.execution_capacity`
- (per Codersera) the four background observer agents are toggled inside `runtime_capabilities`

MCP snippet, verbatim from https://dev.meta.ai/docs/muse-code/extending/:

```json
{ "mcp_servers": {
    "my-tools": { "transport": "stdio", "command": "my-mcp-server", "args": [] }
} }
```

- Transports: `stdio` (`command`, `args`, `env`, optional `framing`) and `streamable_http`
  (`url`, `headers`).
- Every server takes `enabled` and `mode` (`required` | `optional`).
- **Warning, verbatim:** "MCP tools are not sandboxed… an MCP server runs as an ordinary child
  process… outside the filesystem and network sandbox."

### 2.3 Instruction-file precedence — documented

Muse searches **upward from workspace root to the nearest `.git` boundary**, and at each level
checks, in order:

1. `AGENTS.md`
2. `CLAUDE.md`
3. `.agents/AGENTS.md`
4. `.claude/CLAUDE.md`

> "Project rules win over user rules. Among project files, the deeper file wins over a shallower
> one." Project files load only after **workspace trust** is granted.
> — https://dev.meta.ai/docs/muse-code/configuration/

```bash
muse init            # write AGENTS.md in the current directory
muse init --dry-run  # show what it would write, change nothing
```

### 2.4 Skills — the documented extension point

Four sources, verbatim from https://dev.meta.ai/docs/muse-code/extending/:

- **Built-in** — shipped with Muse Code
- **User** — `$XDG_CONFIG_HOME/muse/skills`, `~/.agents/skills`, `~/.claude/skills`,
  `$CODEX_HOME/skills` (fallback `~/.codex/skills`)
- **Project** — `<repo>/.agents/skills/<skill-id>/SKILL.md`, also scanning `.codex/skills` and
  `.claude/skills`
- **Plugin** — "contributed by **enabled plugin bundles**"  ← *the single public mention of plugins*

CLI, verbatim:

```bash
muse skills list                    # every skill, all sources
muse skills inspect <skill-id>
muse skills enable <skill-id> --scope project
muse skills install ./my-skill --scope user
muse skills validate ./my-skill
muse skills import --from claude    # or: --from codex
```

**Built-in skills — the docs and the blogs disagree with each other:**

| Source | Roster |
|---|---|
| https://dev.meta.ai/docs/muse-code/extending/ | `/plan`, `/grill`, `/taste`, `/grill-and-record` |
| https://developer.meta.com/ai/resources/blog/build-with-muse-code/ | `/taste`, `/grilling`, `/grill-with-docs`, `/plan` |
| https://research.meta.ai/blog/introducing-muse-code-and-muse-spark-1-2 and https://musecodes.io/docs/ | `/plan`, `/grill`, `/goal` |
| **[RE]** `re/skills.md` §10.3 | **15 bundled skills** (14 visible by default) |

Composio calls this out explicitly: "Meta's developer docs and Meta's launch post disagree on the
skill roster." (https://composio.dev/content/muse-code-vs-claude-code)

Descriptions Meta does publish: `/plan` "grounds a plan in your real files" and "stops for
approval"; `/taste` is "an anti-slop filter: a flat checklist of visual defaults not to use";
`/grilling` "interviews you one decision-forcing question at a time"; `/grill-with-docs` "writes
the settled decisions into your project docs." Skills are "explicit-invocation only" and load
instructions only for their designated turn
(https://developer.meta.com/ai/resources/blog/build-with-muse-code/).

Changelog 0.2.1 adds two more without naming them: "a built-in skill for setting up isolated Python
environments" and "a built-in skill for handing off and verifying browser apps the agent builds"
(https://dev.meta.ai/docs/muse-code/changelog/) — these are `python-env` and `browser-app-delivery`
**[RE]** `re/skills.md` §10.3.

### 2.5 Hooks — the documented lifecycle

Three sources (https://dev.meta.ai/docs/muse-code/extending/):

- **Project**: `<project-root>/.muse/hooks.json`
- **User**: defined in the settings file
- **Managed**: pointed to by the `managed_hooks_path` setting

**13 documented events:**

`SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`,
`PreLLMCall`, `PostLLMCall`, `PreCompact`, `PostCompact`, `SubagentStart`, `SubagentStop`,
`Stop`, `SessionEnd`

Verbatim warning: "A hook's command runs directly through your shell, **outside** the sandbox and
approval."

Changelog 0.2.1: "Hook commands now receive a selected set of environment variables"; "You can see
which of your hooks are running in the live activity area"; "Structured JSON output from file hooks
is preserved instead of flattened"; "Project hooks take effect as soon as you trust a folder,
without a restart" (https://dev.meta.ai/docs/muse-code/changelog/).

### 2.6 Subagents

https://dev.meta.ai/docs/muse-code/extending/:

- `agents.execution_capacity` in `settings.json`: **1–64** agents, default **8**; an `ultra`
  reasoning root uses **64**.
- `--subagent-worktree-isolation` launch flag → one git worktree per child.
- "Children can spawn grandchildren; all share root-tree capacity."

Codersera contradicts the grandchildren claim: subagents are "One level deep (children cannot spawn
children)", with concurrency "core count minus two (clamped 2-16)", and lists TUI controls
`/subagents`, `/agent-note`, `/agent-followup`, `/agent-interrupt`, `/agent-stop`, `/agent-resume`
(https://codersera.com/blog/muse-code-complete-guide-2026/) — none of which appear in Meta's own
slash-command list.

**Background observer agents** (Codersera, not in Meta docs): four optional watchers, each making
separate model calls — **memory recall** (on), **skill recall** (on), **goal tracking** (on),
**verification** (off) — toggled in `runtime_capabilities`.

### 2.7 Workflows

https://dev.meta.ai/docs/muse-code/workflows/:

- **Limits**: max **1,000 child tasks per workflow lifetime**; up to **16 active children**
  (CPU-derived, capped); a 1,001st child call fails the workflow.
- **Modes** (`/settings` → Tools > Workflows): `auto` (Muse proposes), `explicit` (only on
  request), `off` (tool removed).
- **Control room**: `/workflows`; keys — Up/Down or J/K select, Enter/Right open,
  **C** cancel, **R** results, **P** pause/resume, **X** skip child, Space/Esc close.
- **Custom workflows are JavaScript files**:

```bash
muse workflows save review-change --from ./review-change.js --scope project
```

  Saved to `.agents/workflows/review-change.js` (project) or the config dir (user).
  Naming: lowercase letters/digits/`.`/`_`/`-`, must start lowercase alnum, ≤64 chars.
  File: "regular, nonempty UTF-8 file no larger than 512 KiB".
- **Recovery**: same-process, edit `scriptPath` and call the Workflow tool with `resumeFromRunId`;
  after restart, `muse workflows recover <run-id> --session <session-id> --apply`.

> This is the closest thing Meta ships to a *scriptable* extension point, and it is barely
> promoted. `muse workflows` does not appear in any `muse --help` listing published by Meta.

### 2.8 Permissions, approval, sandbox

https://dev.meta.ai/docs/muse-code/permissions/:

- `--approval-mode`:
  - `on-request` (default) — "recognized, non-dangerous parsed shell stages normally pass without a
    prompt", dangerous patterns stop
  - `untrusted` — "a shell stage with no matching allow rule stops for review, not only the
    dangerous ones"
  - `never` — "nothing stops for approval. The sandbox alone contains what runs."
- `--approval-judge off` disables the built-in model-based reviewer.
- **Sandbox**: macOS **Seatbelt**; Linux **"a bundled bubblewrap helper"**. Write access to
  workspace + temp; everything else read-only, **including `.git`, `.muse`, `.agents`** — "to
  prevent agent self-modification."
- **Prefix rules apply by specificity. "A deny rule always overrides an allow rule, whatever the
  specificity."**
- **Workspace trust**: asked on first open; allow once / always allow in this workspace / reject.
- `--sandbox-network`: `proxy-only` (default, per-destination approval), `restricted` (none),
  `enabled` (full).
- Escape hatches: `--yolo` (no approval, no sandbox), `--disable-approval` (sandbox kept),
  `--disable-sandbox` (approval kept).

Changelog 0.2.1 adds "Automatic approval pre-screening: a model-based reviewer clears tool requests
it judges safe… Anything it doesn't clear still comes to you, and it can be disabled"
(https://dev.meta.ai/docs/muse-code/changelog/).

### 2.9 Headless / CI

https://dev.meta.ai/docs/muse-code/extending/:

```bash
muse exec "prompt text"
muse exec --prompt-file ./task.txt
muse exec --json "prompt"           # JSONL events on stdout
muse exec --session-id <uuid> "Continue"
muse export --session <uuid> --out run.json
```

Flags: `--disable-approval`, `--yolo`, `--max-model-steps`, `--allow-workspace-switch`.
Exit codes: `0` completion, `1` failure/cancellation, `2` usage error, `130`/`143` SIGINT/SIGTERM.

### 2.10 Slash commands — the full documented set

https://dev.meta.ai/docs/muse-code/interactive/:

`/new` · `/clear` · `/name` · `/resume` · `/resume --last` · `/fork` · `/side` · `/side <prompt>` ·
`/btw <prompt>` · `/compact` · `/recap` · `/export transcript` · `/export trajectory` · `/copy` ·
`/goal <objective>` · `/goal edit` · `/goal pause` · `/goal resume` · `/goal clear` · `/loop` ·
`/tasks` · `/subagents` · `/stop` · `/workflows` · `/plan` · `/voice status` · `/voice debug` ·
`/help` · `/keymap` · `/settings`

Plus from the changelog: `/status` ("Redesigned `/status` as a cleaner summary card"), `/usage`,
`/models`, `/login`.

Keyboard: `Enter` steer into a running turn · `Alt+Enter` queue for next turn · `Esc` interrupt ·
`Esc` on empty composer retracts a queued instruction · `Ctrl+C` interrupt/clear ·
`Alt+V` / `option-V` voice · `Ctrl+C` returns from a side conversation · **double `Esc` = Rewind**.

### 2.11 CLI flags — documented

https://dev.meta.ai/docs/muse-code/configuration/:

- `--model <id>` (default `muse-spark-1.2`)
- `--reasoning-effort <none|minimal|low|medium|high|xhigh|ultra>` (default `high`)
- `--sandbox-network`, `--disable-sandbox`, `--disable-approval`, `--yolo`, `--trust-workspace`
- `--approval-mode`, `--approval-judge`
- `--workspace <path>`
- `--no-session-log`
- headless-only: `--json`, `--prompt-file <path>`, `--max-model-steps <n>`

Changelog 0.2.1: "`--model` accepts any model id; unknown ids use sensible assumed metadata instead
of being rejected."

### 2.12 Auth

https://dev.meta.ai/docs/muse-code/auth/:

- Browser sign-in, or paste an API key.
- Env var **`META_API_KEY`**. "Muse Code uses `META_API_KEY` if set, then a stored key, and only
  then a stored browser session."
- `muse auth set`, `muse logout`, `/login`.
- "Meta Managed Account (MMA) users must authenticate with an API key: MMA accounts can't use the
  browser sign-in flow."
- **Credential storage path is not documented.**

> **Env var conflict:** the Model API docs and musecodes.io both use **`MODEL_API_KEY`**
> (https://dev.meta.ai/docs/overview/, https://musecodes.io/docs/,
> `export MODEL_API_KEY="LLM|{numeric_id}|{secret}"`), while the Muse Code auth page uses
> **`META_API_KEY`**. Both appear in first-party material.

### 2.13 Session messaging

https://dev.meta.ai/docs/muse-code/session-messaging/:

- `/name`, `/name <name>`; names 3–32 chars, lowercase alnum + hyphens, no leading digit-start
  violation, no trailing/consecutive hyphens.
- Messages are **plain text, ≤8,192 bytes**, sent by natural-language request, not a structured
  command; modes are steer / queue / notify-only.
- Receiver approves senders; **pending approvals expire after 30 minutes**.
- Discovery: same user account, **same Unix machine**, interactive sessions only (excludes headless
  and `--no-session-log`). Self-messaging rejected; duplicate message IDs deduped.
- Messages "cannot approve tool calls, grant consent, or change permissions" and are "treated as
  unverified agent-provided data."
- Toggle in `/settings` > Tools > Session messaging; **applies on next launch**.
- Transport is documented only in the GA blog: **Unix sockets**
  (https://developer.meta.com/ai/resources/blog/muse-code-new-plans-and-features/).
- CLI form (not in Meta docs): `muse session-message <send|serve>`
  (https://qainsights.com/getting-started-with-muse-code-cli-commands-syntax-and-purpose/).

### 2.14 Rewind

https://dev.meta.ai/docs/muse-code/rewind/:

> "Rewind creates a retained conversation branch and restores the selected message as an unsent
> draft."
>
> "Rewind is available only in interactive terminal sessions with session logging enabled. It is
> unavailable in sessions started with `muse --no-session-log`."

**Not documented anywhere:** the event log's on-disk path, format, retention, or the `muse replay` /
`muse trace` commands — even though musecodes.io tells users to run `muse replay`
(https://musecodes.io/docs/) and QAInsights documents `muse trace inspect`
(https://qainsights.com/getting-started-with-muse-code-cli-commands-syntax-and-purpose/).

### 2.15 Interop with other agents (an underrated official surface)

https://dev.meta.ai/docs/coding-agents/ tells you how to point **Claude Code**, **Codex**, and
**OpenCode** at Meta's models. Verbatim Claude Code block:

```shell
export ANTHROPIC_BASE_URL="https://api.meta.ai"
export ANTHROPIC_AUTH_TOKEN="$MODEL_API_KEY"
export ANTHROPIC_MODEL="muse-spark-1.2"
export ANTHROPIC_DEFAULT_OPUS_MODEL="muse-spark-1.2"
export ANTHROPIC_DEFAULT_SONNET_MODEL="muse-spark-1.2"
export ANTHROPIC_DEFAULT_HAIKU_MODEL="muse-spark-1.2"
export CLAUDE_CODE_SUBAGENT_MODEL="muse-spark-1.2"
export ENABLE_TOOL_SEARCH="true"
```

Codex, `~/.codex/config.toml`:

```toml
model = "muse-spark-1.2"
model_provider = "meta"
model_reasoning_effort = "high"
model_reasoning_summary = "auto"
model_context_window = 1048576
model_supports_reasoning_summaries = true
model_auto_compact_token_limit = 900000

[model_providers.meta]
name = "Meta Model API"
base_url = "https://api.meta.ai/v1"
env_key = "MODEL_API_KEY"
wire_api = "responses"
```

OpenCode, `opencode.json`:

```json
{
  "provider": {
    "meta": {
      "name": "Meta Model API",
      "npm": "@ai-sdk/openai",
      "options": { "baseURL": "https://api.meta.ai/v1" },
      "models": {
        "muse-spark-1.2": {
          "name": "muse-spark-1.2",
          "reasoning": true,
          "limit": { "context": 1048576, "output": 131072 },
          "modalities": { "input": ["text","image","pdf","video"], "output": ["text"] },
          "options": {
            "reasoningEffort": "high",
            "reasoningSummary": "auto",
            "include": ["reasoning.encrypted_content"]
          }
        }
      }
    }
  }
}
```

The strategic read (https://musecodes.io/blog/model-api-ecosystem/): Meta is playing an
"any-harness" game on the model, and a first-party game on the agent. **Meta explicitly blesses
running competitors' harnesses on its model.** That posture is why the binary's Claude/Codex
compatibility (`~/.claude/skills`, `.claude-plugin/`, `skills import --from claude`) is not an
accident.

---

## 3. What Meta says about EXTENSIBILITY and a plugin ecosystem

**Short answer: almost nothing, and what it does say is a single throwaway line.**

- The word "plugin" appears **once** in Meta's Muse Code documentation, as a skill *source*:
  "**Plugin**: contributed by enabled plugin bundles"
  (https://dev.meta.ai/docs/muse-code/extending/).
- There is **no `muse plugins` command in any published `muse --help`**, no plugin manifest schema,
  no marketplace docs, no `create-plugin` mention, and no plugin page in the doc nav (the full nav
  is enumerated in §0 — `/docs/muse-code/{auth,changelog,configuration,extending,interactive,permissions,rewind,session-messaging,subscriptions,workflows}` and nothing else).
- The GA blog's extensibility story is the **SDK**, not plugins: "Muse Code is now programmable" —
  "sessions, tools, and permission control — as a TypeScript library," talking MSP,
  "a JSON-over-stdio contract with a generated schema"
  (https://developer.meta.com/ai/resources/blog/muse-code-new-plans-and-features/). **It has no
  docs page.**
- Meta's public framing of customisation is: system messages for standing rules
  (https://developer.meta.com/ai/resources/blog/build-with-muse-code/) and `AGENTS.md`
  (https://dev.meta.ai/docs/muse-code/configuration/).

Third-party verdicts:

- Composio: Muse "arrived unusually complete for a week-one product" — MCP over stdio and
  streamable HTTP, hooks, skills, sandboxing, worktree parallelism at launch — **but** "no Windows
  build. No IDE extension. No subscription tier. **No third-party plugin ecosystem, no
  marketplace, no community catalogue.**" (https://composio.dev/content/muse-code-vs-claude-code)
- Layer3Labs' gap list at launch: "Config system: not documented. Skills/tools: full list
  unpublished. **MCP support: no mention. Plugin ecosystem: none described.** Open source: CLI is
  proprietary. Rate limits: undocumented."
  (https://www.layer3labs.io/guides/muse-code-explained)
- Codersera's "What's missing": "No third-party plugin ecosystem"; "No IDE extensions"; "No Windows
  native build"; "Single model, no fallback"; "No parameter count or architecture disclosure"; "No
  knowledge cutoff published" (https://codersera.com/blog/muse-code-complete-guide-2026/).
- Codersera also corrects the launch coverage: "Contrary to launch coverage, **MCP is
  supported… it is documented and present in the shipped binary**." (same)
- EveryDev.ai: "Lifecycle hooks binding shell commands to session, prompt, tool, model, and
  subagent events" are available "but undocumented"; "MCP servers" appear "as an integration point
  but [with] no implementation details." (https://www.everydev.ai/tools/muse-code)

---

## 4. Community reaction — what people say is missing

- **Muted volume.** "two days after launch, total discussion across Hacker News and Reddit amounted
  to a few hundred comments, with only a handful of people posting first-hand results"; the HN
  thread settled on "a nice release and a solid improvement over Spark 1.1, not SOTA, but solid"
  (https://composio.dev/content/muse-code-vs-claude-code,
  https://codersera.com/blog/muse-code-vs-claude-code-2026/).
- **The ecosystem gap is the headline complaint**: "Muse Code has none of the integrations and a
  day-old ecosystem compared to Claude Code's maturity" (same).
- **No Windows, no IDE extension** — repeated everywhere
  (https://composio.dev/content/muse-code-vs-claude-code,
  https://codersera.com/blog/muse-code-complete-guide-2026/,
  https://www.verdent.ai/guides/agents/what-is-muse-code).
- **Signup friction**: identity verification, Facebook/Instagram account requirements; early bugs
  (Docker sign-in failures, startup crashes, billing display errors)
  (https://codersera.com/blog/muse-code-complete-guide-2026/).
- **Trust concerns** over closed weights plus Meta's advertising business (same).
- **The co-training claim is contested.** A competitor "extracted Muse Code's system prompt into
  their own harness and reported '2.7x fewer tokens, 2x faster and 2.4x cheaper results,'
  suggesting 'part of what Meta calls co-training is transferable prompt engineering.'" (same)
- **Theo's stress test**: reviewed "222 PRs in under five minutes" for "ten cents" on contributor
  pricing, but generated code that "didn't run" and mapped tasks onto "entirely unrelated
  products" — verdict: "a **triage tool, not a merge tool**."
  (https://composio.dev/content/muse-code-vs-claude-code)
- **Meta's own benchmarks show a deficit**: Claude Opus 5 scores 79.4% vs Muse Spark 1.2's 70.6% on
  Meta's internal bench — "an 8.8-point deficit published voluntarily in the launch post"
  (https://trycodus.com/blog/muse-code-vs-claude-code-benchmarks).
- **Claim-vs-evidence skepticism**: "two days in… the claims outnumber the evidence"
  (https://www.verdent.ai/guides/agents/what-is-muse-code).
- **SitePoint's own gap note**: "most coverage is missing a clear setup walkthrough explaining
  `.museignore` and `MUSE_CODE.md` workflows" (search extract from
  https://www.sitepoint.com/meta-muse-code-getting-started/) — note that **neither `.museignore`
  nor `MUSE_CODE.md` exists in the binary**, so this is third-party invention. Flagged as a
  cautionary example of how thin the public documentation is.

---

## 5. Release cadence and the channel model

### 5.1 What Meta publishes

- Install: `curl -fsSL https://dev.meta.ai/install.sh | bash` (or `| sh`)
  (https://dev.meta.ai/docs/muse-code/, https://musecodes.io/docs/).
- Changelog with **three entries only**: `0.2.1`, `0.1.x`, `0.1.0` ("Launch version"), **no dates,
  no channel names, no update mechanism** (https://dev.meta.ai/docs/muse-code/changelog/).
- musecodes.io's changelog is dated but product-level, not build-level
  (https://musecodes.io/changelog/).
- **Nothing about auto-update, channels, pinning, or uninstall on any Meta page.**

### 5.2 What the installer/launcher actually implement

From `install.sh` and `muse-launcher.sh` (local copies; the launcher is fetched from
`https://api.meta.ai/muse-launcher.sh`), corroborated by
https://codersera.com/blog/how-to-install-muse-code-cli-2026/:

```sh
# install.sh
install_dir="${MUSE_INSTALL_DIR:-${HOME:?HOME is not set}/.local/bin}"
launcher_url="${MUSE_LAUNCHER_URL:-https://api.meta.ai/muse-launcher.sh}"
# ... MUSE_NO_MODIFY_PATH, MUSE_LAUNCHER_INSTALL=1, MUSE_UPGRADE_MODE

# muse-launcher.sh
channel="muse-stable"
channel_url="${MUSE_CHANNEL_URL:-https://api.meta.ai/muse-code/channels/muse-stable}"
update_interval="${MUSE_UPDATE_INTERVAL_SECONDS:-3600}"
auth_url="${MUSE_AUTH_URL:-https://auth.meta.com}"
client_id="${MUSE_CLIENT_ID:-1031625952748946}"
download_host="${MUSE_DOWNLOAD_HOST:-lookaside.facebook.com}"
```

- **Auto-updates hourly by default** (`update_interval` 3600s); `MUSE_NO_AUTO_UPDATE=1` disables;
  `MUSE_SYNC_UPDATE=1` makes the update blocking
  (https://codersera.com/blog/how-to-install-muse-code-cli-2026/ + launcher source).
- Launcher exports `MUSE_RELEASE_INFO` into the child binary's environment.
- Layout: `~/.local/bin/muse` (launcher) + `~/.local/bin/muse-bin-<version>` (binary)
  (https://codersera.com/blog/how-to-install-muse-code-cli-2026/).

### 5.3 Channel manifest schema (live, undocumented)

`GET https://api.meta.ai/muse-code/channels/muse-stable` →

```json
{"channel":"muse-stable","version":"1.0.1-R2006.1",
 "manifest_url":"https://lookaside.facebook.com/lookaside/muse/download/?channel=muse&version=1.0.1-R2006.1&file=manifest.json",
 "urgency":"none","notification_text":"","state":"public","min_version":null}
```

`manifest.json` → `{version, checksum_algorithm:"sha256", artifacts:{<platform>:{url,checksum,size}}}`.

### 5.4 **FINDING: there is a second, undocumented channel — `muse-canary`**

Probing `https://api.meta.ai/muse-code/channels/<name>` on 2026-09-01:

| Channel | HTTP | Version | `state` |
|---|---|---|---|
| `muse-stable` | **200** | `1.0.1-R2006.1` | `public` |
| **`muse-canary`** | **200** | **`1.1.0-R2009.1`** | **`canary`** |
| `muse-beta`, `muse-nightly`, `muse-dev`, `muse-latest`, `muse-rc`, `muse-edge`, `muse-alpha`, `muse-experimental`, `muse-next`, `muse-insider`, `muse-enterprise`, `muse-lts`, `muse-test`, `stable`, `beta`, `nightly` | 404 `{"title":"Not Found","detail":"Channel metadata is not available."}` | — | — |

So the channel model is **exactly two channels**, `state ∈ {public, canary}`, and canary is a full
minor version ahead (**1.1.0** vs **1.0.1**). Selecting it requires only
`MUSE_CHANNEL_URL=https://api.meta.ai/muse-code/channels/muse-canary`. Meta documents neither the
existence of channels nor the env var.

Version-string format: `<semver>-R<build>.<n>`; `--version` prints `Muse Code 1.0.1 (1.0.1-R2006.1)`
**[RE]** `re/cli-surface.md`. Compare Codersera's launch-day reading, `Muse Code 0.1.0 (0.1.0-R708.1)`
"published on the muse-stable channel" (https://codersera.com/blog/how-to-install-muse-code-cli-2026/):
R708 → R2006 in under a month is roughly **43 builds/day** on the release train, of which only a
few are promoted.

---

## 6. Where the official docs CONTRADICT or OMIT what the binary contains

This is the opportunity list. Each row is "what Meta says" vs "what 1.0.1-R2006.1 does",
with the RE artefact that proves it.

### 6.1 The plugin subsystem is entirely undocumented — **the single largest gap**

| Docs | Binary **[RE]** `re/plugins.md` |
|---|---|
| One phrase: skills may come from "enabled plugin bundles" (https://dev.meta.ai/docs/muse-code/extending/). No CLI, no schema, no marketplace. | A complete **16-verb `muse plugins` command tree** behind `MUSE_EXPERIMENTAL_PLUGINS=1`; without the gate: `plugins are not available in this build`. |
| — | **Four manifest families**: `.muse-plugin/plugin.json` (native), `.claude-plugin/plugin.json` (**Claude Code plugins are a first-class supported input**), `.codex-plugin/plugin.json`, and root `plugin.json` for "Agent Plugins 1.0.0". |
| — | Native capabilities: `skills`, `commands`, `hooks`, `mcpServers`, `reminders` (+ `developerPrompts`, gated off). Rejected: `tools`, `agents`, `outputStyles`, `settings`, `apps`. |
| — | **Marketplaces**: local dir, local file, and **Git**, including native support for Claude Code's `.claude-plugin/marketplace.json`; `.muse-claude-sources` / `.muse-codex-sources` clone targets. |
| — | Two lockfiles under `$DATA_DIR/muse/plugins/`: `installed.json`, `marketplaces.json`, plus per-marketplace `snapshot.json`. |
| — | **Per-capability trust** keyed by a `definition_hash` bound to the package digest, stored in `settings.json` → `runtime_capabilities`; hooks/MCP/agent-definitions are inert until `muse plugins approve`. |
| — | A bundled `create-plugin` skill (hidden unless the gate is on) ships the authoring contract, including the minimal manifest. |

Minimal native manifest, verbatim from the bundled `create-plugin/references/native-plugin-contract.md`
**[RE]** `re/config-paths.md` §6.6:

```json
{
  "schemaVersion": 1,
  "name": "example-plugin",
  "displayName": "Example Plugin",
  "version": "0.1.0",
  "description": "One plain-language sentence.",
  "compat": { "source": "native", "manifestDir": ".muse-plugin" },
  "capabilities": { "skills": [], "commands": [], "hooks": [], "mcpServers": [], "reminders": [] }
}
```

Plugin ID grammar `^[a-z0-9][a-z0-9._-]{0,79}$`; `loop` and `muse-core` reserved.

**Opportunity:** every article says Muse Code has "no plugin ecosystem, no marketplace, no
community catalogue" (https://composio.dev/content/muse-code-vs-claude-code). It has all three,
one env var away, plus a Claude-Code plugin importer. An `oh-my-musecode` that ships as a *native
Muse plugin package* and *also* registers as a Git marketplace is buildable **today** against an
undocumented but complete surface.

### 6.2 Windows is shipped but denied

| Docs / press | Reality |
|---|---|
| "macOS and Linux" (https://dev.meta.ai/docs/muse-code/, https://musecodes.io/docs/, https://9to5mac.com/2026/08/05/meta-launches-muse-code-ai-coding-agent-for-macos-and-linux/); "Windows requires WSL2", installer "hard-fails with `unsupported platform`" (https://codersera.com/blog/how-to-install-muse-code-cli-2026/) | **Both** release manifests ship `x86_windows` and `aarch64_windows` `.exe` artifacts. |

Stable `1.0.1-R2006.1` artifacts: `x86_macos`, `aarch64_macos`, `x86_linux`, `aarch64_linux`,
`universal_macos_pkg`, **`x86_windows` (310,981,880 B)**, **`aarch64_windows` (282,744,568 B)**.
Canary `1.1.0-R2009.1` ships the same seven. There is also a **`muse.pkg` universal macOS
installer** (187 MB) that no documentation mentions.

Corroborating: the CLI has `muse sandbox windows check|setup`
(https://qainsights.com/getting-started-with-muse-code-cli-commands-syntax-and-purpose/) and the
binary implements a `windows_elevated` sandbox mode "requires a setup step"
**[RE]** `re/security-permissions.md` §4.3.

### 6.3 Release channels and auto-update are undocumented

See §5.4. Meta documents no channels; two exist. Meta documents no auto-update; the launcher polls
hourly. `MUSE_CHANNEL_URL`, `MUSE_UPDATE_INTERVAL_SECONDS`, `MUSE_NO_AUTO_UPDATE`,
`MUSE_SYNC_UPDATE`, `MUSE_DOWNLOAD_HOST`, `MUSE_INSTALL_DIR`, `MUSE_LAUNCHER_URL`,
`MUSE_NO_MODIFY_PATH` all exist; only the last four are described anywhere, and only by Codersera.

### 6.4 The changelog is three versions behind the product

Docs changelog tops out at **0.2.1** (https://dev.meta.ai/docs/muse-code/changelog/). Stable is
**1.0.1-R2006.1**; canary is **1.1.0-R2009.1**. Everything between 0.2.1 and 1.0.1 — including the
GA feature set the marketing blog announces — has **no release notes at all**.

### 6.5 `.muse/hooks.json` is documented but appears not to be discovered

Docs: project hooks live at `<project-root>/.muse/hooks.json`
(https://dev.meta.ai/docs/muse-code/extending/).

**[RE]** `re/config-paths.md` §6.5: planting a poison `.muse/hooks.json`, and separately pointing
both `TBH_MANAGED_HOOKS_PATH` and `settings.managed_hooks_path` at it, produced **no hook error**.
Workspace hook auto-discovery at that path is unconfirmed; hooks reach the runtime via
`settings.hooks`, plugin capability manifests, and `managed_hooks_path`. Meanwhile `.muse/` in a
workspace is used for **worktrees only** (§6.4 of the same report), and Muse appends
`/.muse/worktrees/` to `.git/info/exclude`.

**Consequence for a framework:** do not write `.muse/hooks.json` and expect it to fire. Merge into
`~/.config/muse/settings.json` → `hooks`, or ship hooks as a plugin capability.

### 6.6 The hook event list is incomplete

| Docs (13) | Binary **[RE]** `re/config-paths.md` §5.14 (18) |
|---|---|
| `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `PreLLMCall`, `PostLLMCall`, `PreCompact`, `PostCompact`, `SubagentStart`, `SubagentStop`, `Stop`, `SessionEnd` | the same 13, **plus** `notification`, `post_tool_use_failure`, `stop_failure`, `post_tool_batch` |

Hook entry fields the docs never mention: `commandWindows` / `command_windows` (another Windows
tell), `timeout`, `statusMessage`, `async`, `asyncRewake`, `rewake`, `rewakeMessage`,
`rewakeSummary`, `shell`, `condition`, `if`, `silent`, `outputCapabilities`. Also
`managed_hooks_env_vars` and `max_consecutive_stop_hook_continuations`.

### 6.7 "There is no workspace settings file" is never stated — and it matters enormously

Docs describe `~/.config/muse/settings.json` and project **instruction** files, but never say the
obvious corollary. **[RE]** `re/config-paths.md` §6.1 proves it by planting a poison document at
nine candidate paths (`.muse/settings.json`, `.agents/settings.json`, `settings.json`,
`.muse/config.json`, `muse.json`, `.musecode.json`, `.muse/muse.json`, `.agents/config.json`,
`.muse.json`) — none is read. The bundled `manage-settings` skill states the rule:

> "That config root is the whole answer surface for this skill. A configuration file outside it is
> not Muse Code configuration — do not hunt for substitute configuration files."

**The classic "workspace overrides user" tier does not exist for settings.** Workspace influence is
limited to rules, skills, plugins, and worktrees.

### 6.8 The settings schema is 29 top-level keys; docs describe ~8

**[RE]** `re/config-paths.md` §5.3 (`struct SettingsFileDocument with 29 elements`):

`schema_version` · `agents` · **`agent_definitions`** · **`provider`** (`echo` | `meta`) · `model` ·
`reasoning_effort` · **`first_turn_minimal_effort_regex`** · `context_compaction` · `run` ·
`provider_retry` · `tui` (15 fields) · `context` · **`local_session_messaging`** ·
**`feature_config`** · `tools` · `skills` · **`model_catalog`** · `mcpServers` (alias `mcp_servers`) ·
**`presets`** · `hooks` · `runtime_capabilities` · **`permissions`** · **`plugins`** ·
`managed_hooks_path` · `managed_hooks_env_vars` · `max_consecutive_stop_hook_continuations` ·
`endpoint_transport` · `telemetry` · `notifications`

Bold = never mentioned in any Meta documentation. **`permissions` and `plugins` are the two that
an extension framework most needs.**

### 6.9 Named permission profiles are completely undocumented

**[RE]** `re/security-permissions.md` §2: `--permission-profile <ID>`, defined in user
`settings.json` under `permissions.profiles` (`UserPermissionSettingsV1` /
`PermissionProfileDefinitionInputV1`), ≤64 profiles, ids `[a-z0-9_-]{1,64}`, with an `extends` key
for composition. Profiles are **mutually exclusive** with `--approval-mode`, `--sandbox-network`,
`--disable-approval`, `--disable-sandbox`, `--yolo`. They can express per-path read/write/deny,
per-host allow/deny, and per-unix-socket allow/deny (the last two only in `proxy_only` mode), and
select `reviewer: auto_review | human | none`.

The docs describe only the four launch flags. A profile is the **only real tool-restriction
surface in the product** — and `allowed-tools` in a `SKILL.md` frontmatter is a **no-op**
(**[RE]** `re/security-permissions.md` §7, `re/plugins.md` §4.4).

### 6.10 The MSP / `muse serve` / `muse schema` surface has no docs page

Announced as an SDK in a blog
(https://developer.meta.com/ai/resources/blog/muse-code-new-plans-and-features/); absent from the
doc nav. **[RE]** `re/msp-protocol.md`: JSON-RPC 2.0 over newline-delimited stdio, CQRS /
event-sourced, **31 client→server methods, 23 published + 2 unpublished notifications, a 29-row
error registry**, envelope schema version `1`, content fingerprint
`sha256:03312c213efd14277a0e0a102f70adeae497a469ca4edf7242f479953ed758b7` echoed in `initialize`.
`muse schema generate-ts` emits a typed client. Undocumented constraints that will bite:
**command admission capacity is 4** (excess → error `-32031`), `muse serve` **cannot select a
provider** (no `--provider echo` on `serve`), and `item/readOutput` is unrouted so `outputRef` is a
dangling pointer.

### 6.11 Undocumented CLI surface

Meta's docs never publish a `muse --help`. Third-party and RE surface:

- Commands never documented by Meta: `muse trace inspect`, `muse schema`, `muse serve`,
  `muse workflows` (hidden but always on), `muse plugins` (gated), `muse config` (mentioned once in
  changelog 0.2.1 as "validate enterprise-managed configuration documents", with no page),
  `muse sandbox windows check|setup`, `muse session-message send|serve`, `muse replay` (referenced
  by https://musecodes.io/docs/ but absent from Meta's own pages).
- Flags never documented by Meta: `--provider <meta|echo>`, `--base-url`,
  `--preset <native-basic|miniswe>`, `--parallel-tool-calls`, `--image`, `-w/--worktree`,
  `--worktree-base`, `--worktree-existing`, `--max-tool-output-bytes`, `--session-id`,
  `--disable-write`, `--disable-shell`
  (https://qainsights.com/getting-started-with-muse-code-cli-commands-syntax-and-purpose/,
  **[RE]** `re/cli-surface.md`).
- **41 `MUSE_EXPERIMENTAL_*` gates** and ~110 `TBH_*` env vars **[RE]** `re/cli-surface.md` §4 —
  zero documented. `TBH` is the internal codename; the source tree is
  `fbcode/musecode/build/src/crates/` **[RE]** `re/plugins.md`.

### 6.12 Enterprise configuration: shape without enforcement

Changelog 0.2.1 mentions `muse config`; there is no page. **[RE]**
`re/security-permissions.md` §6.3: **"Every `execution.*` policy field is INERT in
1.0.1-R2006.1"** — `forbid_approval_bypass` / `forbid_sandbox_bypass` validate as
`field_not_activated`. A framework can rely on the *shape* but must not rely on the *enforcement*.

### 6.13 Smaller mismatches

| Topic | Docs | Reality |
|---|---|---|
| Subagent depth | "Children can spawn grandchildren" (https://dev.meta.ai/docs/muse-code/extending/) | "One level deep" (https://codersera.com/blog/muse-code-complete-guide-2026/) — needs runtime confirmation |
| Concurrency | `agents.execution_capacity` 1–64, default 8 (extending) vs "up to 16 active children (CPU-derived, capped)" (workflows) | two different limits for two different systems, never reconciled |
| API key env | `META_API_KEY` (https://dev.meta.ai/docs/muse-code/auth/) | `MODEL_API_KEY` (https://dev.meta.ai/docs/overview/, https://musecodes.io/docs/) |
| Contributor limit | 100 RPM (https://dev.meta.ai/docs/pricing-rate-limits/) | "rate-limited by tokens in a rolling 5-hour window, not by request count" (https://developer.meta.com/ai/resources/blog/build-with-muse-code/) |
| Skill roster | three mutually inconsistent lists (§2.4) | 15 bundled, 14 default-visible **[RE]** |
| Foreign session import | `skills import --from claude\|codex` only | bundled `import` skill handles **"a Claude Code, Codex, or Grok session"** **[RE]** `re/skills.md` §10.3 |
| Event log | "plain JSONL on your disk" | path/format/retention never published; `~/.local/share/muse/sessions/YYYY/MM/DD/` per https://qainsights.com/... |

---

## 7. What transfers to an `oh-my-musecode` framework

### 7.1 The four real installation targets, ranked

1. **Skills** — the only *documented*, *ungated*, *multi-root*, *precedence-ordered* extension
   point. Project root `<repo>/.agents/skills/<id>/SKILL.md`; user root
   `~/.config/muse/skills` (managed, has a `lock.json` + `audit.log`) or `~/.agents/skills`
   (plain drop-in). Muse also reads `~/.claude/skills` and `~/.codex/skills`, so a framework can
   ship **one** skill tree that works in Claude Code, Codex, and Muse.
   (https://dev.meta.ai/docs/muse-code/extending/, **[RE]** `re/skills.md` §3.1)
2. **Plugins** — undocumented but complete: bundles skills + commands + hooks + mcpServers +
   reminders, with marketplaces, lockfiles, and per-capability trust. Gated by
   `MUSE_EXPERIMENTAL_PLUGINS=1`, which a framework's own launcher wrapper can set.
   (**[RE]** `re/plugins.md`)
3. **Workflows** — `.agents/workflows/<name>.js`, ≤512 KiB, project or user scope. **Real
   JavaScript, project-committable, and the only place a framework can express orchestration
   logic as code rather than prose.** (https://dev.meta.ai/docs/muse-code/workflows/)
4. **MSP control plane** — `muse serve` + `muse schema generate-ts` for anything that needs to
   observe or drive sessions from outside. (**[RE]** `re/msp-protocol.md`)

### 7.2 Architecture patterns worth stealing

- **Event-sourced core, everything else is a projection.** "crash recovery, step-through debugging,
  session handoff, and compliance export are all the same log wearing different hats"
  (https://musecodes.io/blog/event-log-replay/). MSP's `view/page`, `session/read`,
  `session/resume` and the snapshot rung are four projections of one `session.jsonl`
  (**[RE]** `re/msp-protocol.md`). Any framework state you add should be *derived from* the log,
  never a parallel store that can desync.
- **Ack ≠ outcome.** MSP answers commands with an admission ack (`{"commandId","status":"accepted"}`)
  and delivers outcomes on a separate cursor-stamped view stream. Client-minted UUIDv7 `commandId`
  makes retries idempotent by construction. Copy this shape for any framework RPC.
- **Isolation via boring git.** Worktrees under `.muse/worktrees/`, detached HEAD, `.git/info/exclude`
  appended, leaf naming `<YYYYMMDD>-<4 hex>`, reservation records with `schema_version`
  (https://musecodes.io/blog/agent-fanout-worktrees/, **[RE]** `re/config-paths.md` §6.4). "no
  custom sync layer to trust, no proprietary state to debug."
- **Persistent observers, not per-task spawns.** Four always-on background agents (memory recall,
  skill recall, goal tracking, verification) each making their own model calls, toggled in
  `runtime_capabilities` (https://codersera.com/blog/muse-code-complete-guide-2026/,
  https://musecodes.io/blog/async-background-agents/). This is the mechanism `oh-my-musecode`
  would use to inject cross-cutting behaviour — a "reminder" capability rather than a wrapper.
- **Skill descriptions are the routing table.** The bundled skills' `description` fields are long,
  negative-constrained trigger specifications ("Do NOT use for…", "Always call read_skill for X
  before Y") **[RE]** `re/skills.md` §10.3. The catalog block sent to the model has a **byte
  budget** with description slimming (§5.2–5.3), so a framework shipping 60 skills must write
  descriptions that survive truncation, or it silently loses routing.
- **Gate the workflow, not the model.** `/plan` → `/grill` → approve → `/goal` is a chain where
  "each stage gates the next, which is exactly how you'd want an autonomous system to earn
  incremental trust" (https://musecodes.io/blog/bundled-skills/). Frameworks should ship *gates*,
  not *personas*.

### 7.3 Hard constraints an installer must respect

1. **There is no project settings file.** Anything settings-shaped must be **merged into
   `~/.config/muse/settings.json`**, preserving `"schema_version": 1`, or it is silently ignored.
   A malformed file **fails startup**, so the merge must be atomic and validated.
   (**[RE]** `re/config-paths.md` §6.1, https://dev.meta.ai/docs/muse-code/configuration/)
2. **Workspace trust gates everything project-scoped.** Skills in `.agents/skills`, rules in
   `AGENTS.md`, hooks and workflows are all skipped in an untrusted workspace. Ship a `trust.json`
   writer or teach `--trust-workspace`. (**[RE]** `re/security-permissions.md` §5,
   https://dev.meta.ai/docs/muse-code/configuration/)
3. **Permission profiles cannot be dropped in as a file** — they live in `settings.json` under
   `permissions.profiles`, are capped at 64, and **refuse to coexist** with `--approval-mode`,
   `--sandbox-network`, `--disable-approval`, `--disable-sandbox`, `--yolo`. Each framework
   "recipe" must therefore be a *complete* four-dimension profile, composed via `extends`.
   (**[RE]** `re/security-permissions.md` §2, §10)
4. **`allowed-tools` in SKILL.md frontmatter enforces nothing.** Porting Claude Code skills must
   not assume tool restriction. The only real restriction surface is a permission profile.
   (**[RE]** `re/security-permissions.md` §7, `re/plugins.md` §4.4)
5. **Hooks run outside the sandbox and outside approval** — Meta says so verbatim
   (https://dev.meta.ai/docs/muse-code/extending/). Same for MCP servers. A framework that ships
   hooks is shipping unsandboxed code execution; treat the hook manifest as a security boundary and
   make it reviewable.
6. **Plugin trust is bound to the package bytes** via `definition_hash`; hooks/MCP/agent-definitions
   stay inert until `muse plugins approve`. Any framework update invalidates approval — design the
   update flow around a re-approval prompt, not around silent upgrade.
   (**[RE]** `re/plugins.md` §9.4)
7. **The binary auto-updates hourly.** A framework pinned to undocumented internals must either set
   `MUSE_NO_AUTO_UPDATE=1` in its own launcher, or version-detect at startup. Canary is already a
   full minor ahead. (§5.2, §5.4)
8. **`muse serve` cannot use `--provider echo`.** CI for anything MSP-based must either
   authenticate (costs money) or confine itself to `session/userShell` plus the lifecycle/approval
   plane. This is the biggest practical blocker to testing an `oh-my-musecode` control plane.
   (**[RE]** `re/msp-protocol.md` §11)
9. **Decode permissively.** The published MSP schema "is neither complete nor exactly true" —
   `session/started` is emitted but unpublished, `turn/start.providerRequestOptions` is accepted
   but unpublished, a `mention` variant exists only in the deserializer. Unknown notification →
   log-and-ignore; unknown `ItemKind` → render `kind` + `status` + `fallbackText`.
   (**[RE]** `re/msp-protocol.md` §11)
10. **Admission capacity is 4.** Semaphore your MSP driver or watch 6-of-10 commands fail with
    `-32031`. (**[RE]** `re/msp-protocol.md` §4.2)

### 7.4 Distribution strategy the docs actually hand you

Meta blesses running **Claude Code and Codex against `api.meta.ai`**
(https://dev.meta.ai/docs/coding-agents/) and the binary reads Claude/Codex skill roots, imports
Claude/Codex/Grok sessions, and installs Claude/Codex **plugin packages**
(**[RE]** `re/plugins.md` §4, §5; `re/skills.md` §8, §13). The correct shape for `oh-my-musecode` is
therefore **not** a Muse-only framework. Ship:

- one `SKILL.md` tree that resolves under `.agents/skills` (native), `.claude/skills`, and
  `.codex/skills`;
- a `.muse-plugin/plugin.json` **and** a `.claude-plugin/plugin.json` in the same package
  directory — note the binary requires **exactly one** manifest per package, so these must be
  sibling packages, not one directory
  (**[RE]** `re/plugins.md` §2.1);
- a Git marketplace repo carrying both `.claude-plugin/marketplace.json` and the native catalog,
  which Muse clones into `.muse-claude-sources` / `.muse-codex-sources`
  (**[RE]** `re/plugins.md` §10);
- a `settings.json` merge tool (profiles + hooks + mcpServers + runtime_capabilities), because
  four of the five capability types have **no file-based project surface**.

### 7.5 The five highest-leverage things missing from the product

Ranked by (community demand × implementability on the existing surface):

1. **A plugin marketplace and installer UX** — the subsystem exists and is fully gated; nobody
   knows. `oh-my-musecode` can *be* the marketplace.
   (https://composio.dev/content/muse-code-vs-claude-code)
2. **A documented, shareable permission-profile library** — the only real sandboxing/tool-gating
   surface, undocumented, composable via `extends`. Ship "recipes": `readonly-review`,
   `ci-headless`, `network-off`, `frontend-only`.
3. **A settings merge/lint tool** — `"schema_version": 1` missing → startup failure; 29 keys, no
   published schema; no project override tier. This is the single most user-hostile part of the
   product.
4. **An observer/control plane over MSP** — `session/read`, `session/list`, `view/page` take **no
   writer lease**, so a cost tracker / policy auditor / Slack approval bridge can watch every
   session on the machine while the human's TUI keeps the lease. "This is genuinely rare in agent
   CLIs." (**[RE]** `re/msp-protocol.md` §11)
5. **Channel/version management** — `muse-canary` exists, auto-update is hourly and silent, and the
   changelog is three versions stale. A `muse-version`-style pin/switch tool is 50 lines of shell
   against `MUSE_CHANNEL_URL` + the lookaside manifest.
