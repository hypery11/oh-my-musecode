# omm hooks

One JSON file per hook, in the NATIVE plugin `capabilities.hooks[]` shape. The family is CLOSED: only
`id event command timeoutMs statusMessage async compatibilityName outputCapabilities` are accepted;
`matcher`, `silent`, `description`, `commandWindows` are rejected (`unsupported-field`). `command` is an
argv ARRAY - no shell, no `${VAR}` expansion - and every entry is exactly `["omm","hook","<name>"]`,
which is why the same bytes run on macOS, Linux and Windows (R16). Bare argv words are not treated as
package paths (validated: `source_path: null`).

Native plugin hooks key on `event` only and have no matcher, so every handler fires for every
occurrence of its event and must decide in-process, fast (target < 5 ms), and fail open: any internal
error prints nothing and exits 0. Only exit code 2 blocks (stderr = reason); 1/127 are non-blocking
failures. stdout ceiling 16,384 B. Payload keys common to every event: `hook_event_name session_id
cwd transcript_path model permission_mode` (+ `turn_id` when turn-scoped). Records land in
`session.jsonl` as `hook_run_terminal` with `origin.plugin_id = "oh-my-musecode"`.

Plugin hooks run only after `muse plugins approve plugin:omm:hook:<id>` and while the capability is
literally `trusted_enabled` (`review_needed`, `modified`, `trusted_disabled` or a missing line are
all silent). Any byte change to the package flips them to `modified`; omm install/update re-approves.

## omm-session-start (`SessionStart`)

stdin: `{"hook_event_name":"SessionStart","source":"startup|resume|clear|fork", ...common}`.

stdout on success:

```json
{"hookSpecificOutput":{"hookEventName":"SessionStart",
  "additionalContext":"omm 0.1.0 · profile default · catalog 18,420/32,000 B (27 entries)"}}
```

Append ` · WARNING: N skill descriptions silently dropped (stage 2) - run omm cost` when the catalog
estimate is over budget. Injected as a developer-role context block `hook:session_start:session:<n>`
at order 900000+. `continue:false` is the only other supported decision on this event; never use it.

## omm-guard (`PreToolUse`)

stdin adds `tool_name`, `tool_input` (for `bash`: `{"command": "..."}`), `tool_use_id`. Return `{}`
immediately unless `tool_name` is `bash` or `bash_input`.

**The guard is a heuristic over the literal command text, not a security boundary.** It catches the
spellings in the tables below and nothing cleverer: a script, an alias, a function, a variable holding
the path (`rm -rf "$DIR"`), a target arriving on stdin (`find / | xargs rm -rf`), a write the jail
cannot resolve (backticks, after an unresolvable `cd`), or any host feature
that runs a command string it never sees all pass. The sandbox and the host's permission profile are
the boundary; the guard only stops an agent from typing the obvious thing by accident.

### Input bound

stdin is read up to **4 MiB** (`STDIN_LIMIT_BYTES`; a real event is a few KB). A payload the guard
cannot evaluate — cut at that bound, not one JSON document, or empty — is **denied**, not passed:

```json
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny",
  "permissionDecisionReason":"omm guard: event too large or malformed to evaluate"}}
```

still with exit 0 (fail-open at the process level, R16: only exit 2 blocks). Every other hook keeps
answering `{}` to such a payload. (Before this rule a command padded past the old 1 MiB cut arrived
truncated, failed to parse and was allowed as `{}`.)

### How a line is read

1. `DROP TABLE` / `DROP DATABASE` / `DROP SCHEMA` anywhere in the line, case-insensitive, whitespace
   collapsed → `drop-table`, before anything else.
2. The line is split into simple commands on `;`, `&&`, `||`, `|`, `&` and newlines **outside quotes**
   (a backslash escapes the next character).
3. Per simple command, whitespace tokens with every `"` and `'` removed (`"$HOME"/` and `'/'` spell the
   same target as the bare word), then, repeatedly from the left:
   - one leading backslash is stripped from the program word (`\rm`, the alias bypass, is `rm`);
   - a `NAME=value` assignment is dropped;
   - a wrapper is dropped with its flags — `sudo env command nohup time exec builtin doas xargs
     timeout nice ionice setsid caffeinate stdbuf chroot`; a flag is any leading `-…` token, `--`
     ends them, and a flag that takes the next token as its value drops that too (`sudo -u root`,
     `xargs -I {}`, `env -u X`, `time -o f`, `timeout -s KILL`, `nice -n 10`; attached forms like
     `-I{}` and `-n1` are single tokens); `timeout` and `chroot` also drop their one positional
     (`timeout 5 rm …`, `chroot / rm …`);
   - a leading shell group opener or keyword is dropped — `{`, `(` (attached or not: `(rm`, `{ rm`),
     `then`, `do`, `else`, `!` — and a trailing `)` / `}` closer at the end of the simple command
     (`{ rm -rf /; }`, `( rm -rf / )`, `(rm -rf /)`, `if …; then rm -rf /; fi`, `! rm -rf /`).
4. `<shell> -c <body>` (`sh bash zsh dash`, by name or path): the text after the first `-c` token, one
   pair of surrounding quotes removed, is read as a line of its own (steps 1–5), four levels deep at
   most.
5. `cd <dir>` / `pushd <dir>` earlier on the same line moves the tracked working directory for the
   commands after it (resolved lexically against the event `cwd`; an unresolvable `cd` — bare,
   `-`, `~` — leaves later relative targets unevaluated).

Targets are normalised before matching: runs of `/` collapse to one (`//`), a trailing `/`, `/*` or
`/**` is stripped repeatedly (`/*/`, `~//`), `.` segments are dropped and `..` is resolved lexically
(`/./`, `/home/../`, `$HOME/x/..` all name the root; `~/..` does not).

### Rules

| rule | what is denied |
|---|---|
| rm-root | `rm` with `-r`, `-R`, `--recursive` (in any flag cluster, e.g. `-rf`, `-fr`, `-Rf`; targets after `--` count) whose target normalises to `/`, `~`, `$HOME` or `${HOME}` — or, after a `cd` into one of those, to `.`, `./`, `*` or `./*` |
| find-root-delete | `find` whose starting points (the leading arguments before the first `-…`, `(` or `!`) include one of those roots — or, after a `cd` into one, `.`, `./`, `*`, `./*`, or no starting point at all — with `-delete`, or `-exec` / `-execdir` / `-ok` / `-okdir` followed by `rm`, `rmdir` or `unlink`, anywhere after them |
| force-push-main | `git push` with `--force`, any `-f…` cluster, `--force-with-lease[=…]`, `--force-if-includes` or a `+refspec`, to a destination `main` or `master` (bare, `src:main`, or `refs/heads/main`) |
| reset-hard | `git reset --hard` (any target) |
| drop-table | `DROP TABLE` / `DROP DATABASE` / `DROP SCHEMA` (case-insensitive) |
| write-outside-jail | a redirect (`>`, `>>`, `>\|`, `<>`, `2>`, `&>` and attached/fd spellings), a `tee` operand, or a `cp` / `mv` / `install` destination that resolves outside the event `cwd` (relative targets against the tracked directory, `cd`-aware). Inside the workspace, `/dev/null` and friends, and the OS temp roots (`/tmp`, `/private/tmp`, `/var/folders`) pass; reads (`<`, heredocs), descriptor dups (`2>&1`) and unresolvable targets (`$OUT`, backticks) pass through unevaluated. Needs the event `cwd` — without it only the rootless rules apply |

Examples that are denied: `rm -rf /`, `rm -rf //`, `rm -rf "$HOME"/`, `rm -rf /*/`, `rm -rf /./`,
`rm -rf /home/../`, `\rm -rf /`, `sudo -u root rm -rf /`, `env -i rm -rf /`, `FOO=1 rm -rf -- /`,
`cd / && rm -rf .`, `cd ~ ; rm -rf *`, `bash -c "rm -rf /"`, `sh -c 'cd / && rm -rf .'`,
`echo hi | xargs -I{} rm -rf /`, `timeout 5 rm -rf /`, `nice -n 10 rm -rf /`, `setsid rm -rf /`,
`stdbuf -o0 rm -rf /`, `chroot / rm -rf /`, `{ rm -rf /; }`, `(rm -rf /)`, `! rm -rf /`,
`for x in 1; do rm -rf /; done`, `find / -delete`, `find / -name '*.log' -delete`,
`find "$HOME"/ -mtime +30 -exec rm -f {} \;`, `cd / && find . -delete`, `git push --force origin main`,
`git push origin +main`, `git push -fu origin refs/heads/main`, `git reset --hard HEAD~3`,
`psql -c 'DROP TABLE users'`, `echo hi > /etc/motd`, `echo hi | sudo tee /etc/motd`, `tee -a /etc/motd`,
`cp new /etc/cron.d/x`, `install -m 644 f /usr/local/bin/f`, `echo hi > ~/notes.txt`,
`cd /etc && echo hi > motd`, `sh -c 'echo hi > /etc/x'`.

Examples that pass (by design or as the heuristic's limit): `rm -rf ./build`, `rm -rf /tmp/x`,
`rm -rf ~/..`, `rm /`, `find . -name '*.o' -delete`, `find / -type f -exec grep -l foo {} +`,
`find / | xargs rm -rf` (the target is on stdin), `xargs rm -rf`, `git push --force origin feature`,
`git reset --soft HEAD~1`, `bash -c "ls /"`, `ls -la /`, `echo drop the table`, `echo hi > out.txt`,
`cmd > /dev/null`, `cmd 2>&1`, `cat < /etc/passwd` (a read), `cp /etc/a ./b`, `cd - && echo hi > rel.txt`
(the `cd` is unresolvable, so the relative target passes unevaluated).

Deny shape (the reason is required and non-empty):

```json
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny",
  "permissionDecisionReason":"omm guard: force-push-main - `git push --force origin main`"}}
```

The reason echoes the simple command that matched (whitespace collapsed, cut at 160 characters).

Never emit `permissionDecision: "allow"`: a bare allow is rejected (it requires `updatedInput`), and
omm never rewrites a command. Never emit `continue` or top-level `additionalContext` here - both fail
the hook. The user can switch the guard off via `$XDG_CONFIG_HOME/omm/config.json → "disabled": ["hook:omm-guard"]`
(the bare handler name `hook:guard` is accepted too); the switch is read on every dispatch, so it takes
effect on the next tool call without a restart. It is user-level, not per project.

## omm-stop (`Stop`)

stdin adds `stop_hook_active`, `last_assistant_message`, `turn_id`. Output `{}` when
`stop_hook_active` is true (never nudge twice in one turn), when the message makes no completion
claim (done / fixed / complete / passing / implemented / ready), or when it already cites evidence
(an exit status, a pass/fail count, a diff/cmp result). Otherwise, one line:

```json
{"decision":"block","reason":"omm: before calling this done, run the check that proves it (the tests, build or lint you touched) and report its exit status."}
```

`block` on `Stop` requests one continuation; `settings.max_consecutive_stop_hook_continuations`
(default 8, `1` in the ci profile) caps it. `additionalContext` is unsupported on `Stop`.

Before the claim/evidence check, the workspace state is consulted: when `<cwd>/.omm/verify.json`
holds `{"status":"pending","claim":"…"}`, the stop blocks with
`omm: verify is still pending (<claim>) — record the result before stopping.`
A recorded `pass`/`fail`, or no file, leaves the decision to the claim check. (`cwd` here is
the event's `cwd`, else `workspace_root`, else `working_directory`, absolute only; without one
the gate answers `{}` rather than guess the workspace.)

## omm-skill-gate / omm-intent-gate (`PreToolUse`)

Opt-in per-workspace gates over mutating tools, read from `<cwd>/.omm/` (same directory and
file shapes as the Python hooks they replace, so an existing `.omm/` keeps working). Both
answer `{}` on the wrong event, on a non-mutating tool, and whenever their state is missing,
oversize (> 64 KiB), or malformed — a corrupt gate file disables the gate, it never denies.
A mutating tool is a write/edit tool by name, or a `bash`/`shell`/`run` command that spells a
write (a writer program word, `sed -i`/`awk -i`, or a `>` redirect outside quotes): the same
literal-text heuristic as the guard, with the same blind spots.

`skill-gate.json`: `{"enabled": true, "required": ["planner", "executor"]}` (`requiredSkills`
and `skills` are accepted too). Until every required skill appears in `read-skills.json`
(a list, or `{"skills"|"read"|"ids": [...]}`), mutating tools are denied:

```json
{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny",
  "permissionDecisionReason":"Oh My Muse Code skill gate: mutating tool Write blocked until these skills are marked read in .omm/read-skills.json: planner, executor"}}
```

`intent-gate.json`: `{"required": ["plan"]}` (`plan.json` spells the same). Until
`.omm/plan.json` exists, mutating tools are denied with an `intent-gate: … blocked until
.omm/plan.json exists` reason. The kill switch covers both (`"disabled": ["hook:omm-skill-gate"]`).

## omm-subagent-start / omm-subagent-stop (`SubagentStart` / `SubagentStop`)

One JSONL line (`ts`, `kind`, `hook`, `subagent_id`, `child_session_id`, `session_id`;
a stop adds `outcome`) appended to `<cwd>/.omm/team/log.jsonl`, camelCase event aliases
accepted. `outcome` is the first non-empty terminal word the stop payload carries
(`status`, `outcome`, `terminal`, …) — cancellations land here when the host reports them.
Appends only under an `.omm/` that already exists — the hooks never create it — and always
answers `{}`: logging never blocks, and a hook that cannot log still allows.

## omm-pre-compact (`PreCompact`)

stdin adds `trigger` (`soft`, measured live on 1.3.0; `hard`/`manual` unobserved),
`turn_id` and `model_provider` to the common keys. The event supports no context
injection, and vetoing (`continue:false`) only skips that compaction while pressure
keeps building, so the handler never vetoes and only observes: one JSONL line
(`ts`, `kind`, `hook`, `trigger`, `session_id`, `turn_id`) appended to
`<cwd>/.omm/compact.log.jsonl`, only under an `.omm/` that already exists, always
answering `{}`.
