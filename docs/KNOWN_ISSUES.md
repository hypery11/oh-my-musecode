# Known issues

Findings of the Gate 1 review that do not block the gate (medium / low): each with its severity, an
exact reproduction against the pinned host, the intended fix and the file that owns it. Every entry
is reproducible with the e2e harness's sandbox (`crates/omm/tests/e2e.rs`: a fresh `HOME`,
`XDG_CONFIG_HOME`, `XDG_DATA_HOME`, a git workspace as cwd, `OMM_MUSE_BIN` set).

## 1. Corrupt-ledger recovery rebuilds a ledger without the priors of seeded settings keys and trust entries

**Severity:** medium. Nothing is deleted or corrupted; a later uninstall leaves the profile's
settings keys and the workspace's trust entry in place and names them.

**Repro.**
```
omm install --no-plugin --source <checkout>
printf '{"schema_version":1, broken' > $XDG_CONFIG_HOME/omm/omm.lock.json
omm doctor --fast                      # D13 critical: omm reconcile / omm install --no-plugin
omm reconcile && omm install --no-plugin --source <checkout>
omm uninstall --dry-run                # ✓ kept (current, but never registered — prior unknown):
                                       #   run.context_slimming.* …, trust entry <workspace>
omm uninstall                          # settings.json keeps the profile's keys; trust.json keeps the entry
```
e2e scenario 20 (`s20_a_corrupt_ledger_in_managed_store_mode_heals_through_the_printed_fixes`)
asserts exactly this: the rebuilt ledger cannot know that omm seeded `settings.json` / `trust.json`
or what the keys held before, so both are named under "kept, prior unknown" and left
(ARCHITECTURE.md §4, "Settings keys and trust entries that are current but never registered").

**Intended fix.** `omm reconcile` mines the quarantined `omm.lock.json.bad-<ts>` for the
`settings-key` / `trust` registrations and the `shared` entries it still carries (a corrupt file is
usually a truncated one; every registration that parses is adopted with its recorded `prior` and
`value`), and adopts a key that holds the profile's value with `prior: unknown` otherwise — the
uninstall then restores what it knows and keeps naming the rest.
**Owning file:** `crates/omm/src/cmd/lifecycle.rs` (`reconcile`), `crates/omm-ledger/src/store.rs`
(the quarantine).

## 2. `OMM_THEMES_DIR` inside the host's managed skill store is accepted; a file path is refused with an "outside the base" wording

**Severity:** low. Both are operator errors on an opt-in escape hatch; neither loses data.

**Repro.**
```
OMM_THEMES_DIR=$XDG_CONFIG_HOME/muse/skills/omm-themes omm install --source <checkout>
# accepted: three .tmTheme files land inside the managed store, which `muse skills list` then
# walks and reports as a malformed skill directory
touch $XDG_CONFIG_HOME/muse/notadir
OMM_THEMES_DIR=$XDG_CONFIG_HOME/muse/notadir omm install --source <checkout>
# refused (nothing written), but as a containment failure of each theme path — `path
# "notadir/omm-paper.tmTheme" refused under base muse-config: stat failed: Not a directory` —
# rather than as `OMM_THEMES_DIR … is not a directory` (measured 2026-09-02, dry run)
```

**Intended fix.** `files::themes_dir` refuses a value under `Roots::personal_skills_dir()` (the
host's store is the host's), and words an existing non-directory as "is not a directory" (ENOTDIR)
instead of the containment refusal.
**Owning file:** `crates/omm/src/cmd/a_lifecycle/files.rs` (`themes_dir`).

## 3. After an uninstall that preserved an edited or linked managed skill, doctor D13 keeps warning and a later `omm uninstall` refuses until `--reconcile-host`

**Severity:** low. The preserved skill is the user's by decision (an edit, or a symlinked store
directory — the R2 sentinel); what is wrong is the reporting afterwards.

**Repro.**
```
omm install --no-plugin --source <checkout>
echo "mine" >> $XDG_CONFIG_HOME/muse/skills/omm-commit/SKILL.md
omm uninstall                          # ✓ preserved: skills/omm-commit/SKILL.md (edited); rc 0
omm doctor --fast                      # D13 warn: no ledger but the managed store holds 1 `omm-*` skill
omm uninstall                          # refuses: the host holds 1 thing of omm's … --reconcile-host
```
e2e scenarios 22 and 25 leave the store in exactly this state (a symlinked `omm-commit`).

**Intended fix.** The uninstall writes `$OMM/preserved.json` (`{id, path, reason, ts}` per skill it
kept by decision) and keeps `$OMM/` while it is non-empty; D13 and the ledger-less uninstall read
it and report those skills as "kept by you" (Info), refusing nothing; `--reconcile-host` or a
hand removal clears the record.
**Owning file:** `crates/omm/src/cmd/lifecycle.rs` (`uninstall`, `uninstall_without_ledger`),
`crates/omm-doctor/src/checks/d13_ledger_integrity.rs`.

## 4. The destructive-command guard is a heuristic over the literal command text, not a security boundary

**Severity:** low (documented limit). `content/hooks/README.md` → omm-guard states it: a script, an
alias, a function, a variable holding the path (`rm -rf "$DIR"`), a target arriving on stdin
(`find / | xargs rm -rf`), a command the host runs without showing the text — all pass. Round 5
added the `timeout nice ionice setsid caffeinate stdbuf chroot` wrappers and the shell group
tokens (`{ ( then do else !` and a trailing `) }`); further prefix forms — `command -p`, `exec -a`,
`eval "…"`, `ssh host 'rm -rf /'`, `docker run … rm -rf /`, `python -c 'os.system(…)'`, a `case`
arm (`case x in x) rm -rf /;; esac`: the arm's `x)` is the program word), a command substitution
(`echo $(rm -rf /)`) — remain out of scope by design (measured 2026-09-03, release binary).

**Repro.**
```
printf '{"hook_event_name":"PreToolUse","tool_name":"bash","tool_input":{"command":"D=/; rm -rf \\"$D\\""}}' | omm hook guard
# {} — allowed: the target is a variable
```

**Intended fix.** None planned for the heuristic itself; the sandbox and the host's permission
profile are the boundary. New spellings that show up in real transcripts are added to the table
and to `guard_denies_the_readme_rules_and_nothing_else`, never the other way round.
**Owning file:** `crates/omm/src/cmd/c_tune/hook.rs` (`tokens`, `guard_verdict`),
`content/hooks/README.md`.

## 5. A pre-existing `AGENTS.md` that carries only the legacy `omm:user-start` / `omm:user-end` pair does not come back byte for byte

**Severity:** low. The user's text survives; the file's own two marker lines are dropped and a blank
line is added. Reachable only by hand (a file that kept the legacy user pair without the managed pair).

**Repro.**
```
printf '# T\n<!-- omm:user-start -->\nmine\n<!-- omm:user-end -->\n' > $XDG_CONFIG_HOME/muse/AGENTS.md
omm install --no-plugin --source <checkout>   # "the managed block was inserted at the top; your file is kept below"
omm uninstall                                  # AGENTS.md is now "# T\nmine\n\n" — the recorded prior.replaced bytes
                                               # are not used because stripping the block leaves a different text
```
(measured 2026-09-03, release binary, sandbox.) `Markers::parts` reads the user pair as a legacy
region — its marker lines omm's, its text normalised — although the entry's frame is
`inserted_frame()` (`user: ""`), i.e. omm never wrote those lines.

**Intended fix.** When the entry's frame records no `user` text (the block was inserted above a
file omm did not seed), `without_managed` treats the whole text after the inserted block as the
user's — no user-pair parsing — so `rest == prior.replaced` and the file is restored byte for byte.
**Owning file:** `crates/omm-ledger/src/rules.rs` (`without_managed`, `Parts`),
`crates/omm-ledger/src/uninstall.rs` (`decide_rules`).

## 6. A rule written under the template's placeholder comment keeps the placeholder after uninstall

**Severity:** low (cosmetic: one HTML comment line that names omm stays in the user's file).

**Repro.**
```
omm install --no-plugin --source <checkout>
# add "- mine" on the line after the placeholder comment, inside the user markers
omm uninstall            # AGENTS.md = "<!-- Your rules. omm owns only the block … -->\n- mine\n"
```
By design today: the user region is dropped only while it is exactly the template's default
(`rules::tests::without_managed_takes_away_exactly_the_frame_and_keeps_the_rest`).

**Intended fix.** `without_managed` also strips the placeholder when it is the first line of the
normalised user region and equals the frame's `user` text.
**Owning file:** `crates/omm-ledger/src/rules.rs` (`without_managed`).

## 7. With no ledger at all, an `AGENTS.md` carrying omm's managed block is invisible to doctor D13 and to the ledger-less uninstall

**Severity:** low. Nothing is lost; the host keeps loading a block that names skills that are gone,
and doctor says "omm never installed here". The way out exists (`omm install [--no-plugin]` adopts
the file; the next `omm uninstall` removes the untouched seed).

**Repro.**
```
omm install --no-plugin --source <checkout>
# drop the AGENTS.md entry from omm.lock.json (a legacy install's state)
omm uninstall            # "! present but unlisted: … AGENTS.md" — left in place, ledger removed (rc 0)
omm doctor --fast        # D13 info: "no ledger … omm never installed here", fix none
omm uninstall --reconcile-host   # says nothing about the file
omm install --no-plugin --source <checkout>   # "an identical managed block was already there; adopted"
omm uninstall            # removed
```
(measured 2026-09-03.) D13's `LedgerState::Absent` branch returns before the marked-file check;
`uninstall_without_ledger` has no `unlisted_managed_rules` line.

**Intended fix.** Run the `RULES_MANAGED_START_MARKER` check in D13's absent-ledger branch (Warn,
fix `omm install [--no-plugin]`) and print the same `! present but unlisted` line from
`uninstall_without_ledger`; with issue 3's `$OMM/preserved.json` the ledgered uninstall would also
keep `$OMM/` while such a file remains.
**Owning file:** `crates/omm-doctor/src/checks/d13_ledger_integrity.rs`,
`crates/omm/src/cmd/lifecycle.rs` (`uninstall_without_ledger`).

## 8. The ledger-less uninstall stops on a malformed `settings.json` / `trust.json` instead of degrading to a blind undo

**Severity:** low. Only the combination "no ledger" + "malformed host config" is affected; nothing
is written, the host's reason is printed, and a repaired rerun completes.

**Repro.**
```
omm install --source <checkout>
rm -rf $XDG_CONFIG_HOME/omm
sed -i '' 's/"trusted"/"typo"/' $XDG_CONFIG_HOME/muse/trust.json
omm uninstall --reconcile-host   # rc 1: `muse skills list --source user --json` exited 1: malformed trust store …
                                 # plugin `oh-my-musecode` and marketplace `omm` still registered
```
(measured 2026-09-03.) The ledgered `uninstall()` degrades (`host_view` failure → "undone blind");
`uninstall_without_ledger` propagates `managed_omm_skills`'s error.

**Intended fix.** Catch the `skills list` failure in `uninstall_without_ledger` like `uninstall()`
does: warn, skip the managed-store reconciliation, still undo the plugin and marketplace
(`plugins list/remove`, `marketplace list/remove` answer over a malformed `trust.json`).
**Owning file:** `crates/omm/src/cmd/lifecycle.rs` (`uninstall_without_ledger`).

## 9. A rules entry whose write never landed makes the rerun stage the bare template and report "the markers were removed"

**Severity:** medium. Nothing on disk is touched (the user's file is preserved by install, update
and uninstall, and `--force` restores the recorded pre-existing bytes), but `omm install` — the
documented recovery command — does not converge on the rules file, doctor D13 reports nothing, and
the staged `updates/<version>/muse-config/AGENTS.md` is the template alone (the user's text is not
in it), so applying the staged file by hand would drop the user's rules. The same state arises when
the user deletes the markers from a file omm inserted its block into.

**Repro.**
```
printf 'my rules\n' > $XDG_CONFIG_HOME/muse/AGENTS.md
omm install --no-plugin --source <checkout>
printf 'my rules\n' > $XDG_CONFIG_HOME/muse/AGENTS.md   # the state a kill between the ledger save and the write leaves
omm install --no-plugin --source <checkout>   # rules: skipped — "the markers were removed; the file is no longer the
                                              # one omm wrote; the refreshed file is staged" (staged = the template)
omm doctor --fast                             # D13 info: every file present and contained
omm uninstall --dry-run                       # ✓ preserve … "--force removes it" (it restores prior.replaced)
```
(measured 2026-09-03, release binary.)

**Intended fix.** In `converge_rules` `(Some(entry), Some(text))` with `rules::parts(text)` absent
and `prior.replaced` recorded: when `text` equals the replaced bytes, treat the run as a fresh
insert (`insert_managed`, `inserted_frame`, the same `replaced`); otherwise stage
`insert_managed(text)` rather than the template. Word the preserve reason "--force restores the
file that was there before omm" when `replaced` is recorded.
**Owning file:** `crates/omm/src/cmd/a_lifecycle/files.rs` (`converge_rules`),
`crates/omm-ledger/src/uninstall.rs` (the preserve reason of `plan`).

## 10. The orphan store directory is finished with `remove_dir_all`

**Severity:** low. `classify_orphan_store_dir` verifies that nothing beyond the source's files is in
the directory and `remove_orphan_store_dir` removes those files deepest-first, but the directory
itself then goes with `std::fs::remove_dir_all`; a file created inside it between the
classification and the removal (the length of one `omm uninstall --reconcile-host` apply) goes with
it. Not reproducible without a race.

**Intended fix.** Remove the emptied directories bottom-up with `std::fs::remove_dir` (refuses a
non-empty directory) and name what was left.
**Owning file:** `crates/omm/src/cmd/a_lifecycle/skills.rs` (`remove_orphan_store_dir`).

## 11. `omm mcp` writes its trace wherever `MUSE_PLUGIN_DATA_DIR` points, uncontained

**Severity:** low. The variable is the host's per-plugin data dir (host-reality "Paths": advertised,
never created) and the host always hands an absolute path under `$XDG_DATA_HOME/muse/plugins/data/`;
only a hand-spawned server sees anything else. Nothing is overwritten (the file is append-only and
truncated past 1 MiB), but the path is neither canonicalised nor contained (R4's rule for every write).

**Repro.**
```
cd <workspace>
printf '{"jsonrpc":"2.0","id":1,"method":"ping"}\n' | \
  env -i PATH=/usr/bin:/bin HOME=$HOME MUSE_PLUGIN_DATA_DIR=rel/dir MUSE_PLUGIN_ID=omm omm mcp
ls rel/dir/omm-mcp.log          # created under the cwd — the workspace — by create_dir_all
# any absolute path is created the same way; a path that is a file or unwritable is skipped silently (rc 0)
```
(measured 2026-09-03, release binary, Gate 2 review round 1.)

**Intended fix.** `TraceLog::open` accepts only an absolute value whose canonical parent lies under
`Roots::data_root().join("plugins/data")` (the host's layout) and keeps no trace otherwise — the
server still serves; the `spawn` event records why the trace is off.
**Owning file:** `crates/omm/src/cmd/mcp.rs` (`mcp`, `TraceLog::open`).

## 12. `omm mcp` exits 1 with a stderr line when neither `HOME` nor the `XDG_*` roots are set

**Severity:** low. Unreachable from the host: its 16-key scrubbed child environment always carries
`HOME` (research/experiments/plugin-mcp.md §4). A hand-spawned server under `env -i` answers nothing
and exits 1 instead of serving `initialize` / `tools/list` and reporting the missing root as an
`isError` tool result.

**Repro.**
```
printf '{"jsonrpc":"2.0","id":1,"method":"ping"}\n' | env -i PATH=/usr/bin:/bin omm mcp; echo rc=$?
# stderr: omm: no home directory: neither HOME nor the XDG_* override is set (the host resolves exactly two candidates per root)
# rc=1, nothing on stdout
```
(measured 2026-09-03, release binary.)

**Intended fix.** Build the `Ctx` lazily on the first `tools/call` (the static methods need no
roots), and turn a `Roots` failure into the same `isError` text `host_error_text` already produces
for a missing binary; `dispatch` routes `Command::Mcp` past `Ctx::from_env` the way it routes `Hook`.
**Owning file:** `crates/omm/src/cmd/mod.rs` (`dispatch`), `crates/omm/src/cmd/mcp.rs` (`mcp`, `Server::call`).

## 13. INSTALL_FOR_AGENTS.md's D1 row does not list the fix doctor prints after `muse plugins disable omm`

**Severity:** low (docs). The row names `muse plugins approve …` · `omm install` · `omm install
--reinstall`; after `muse plugins disable omm` doctor prints `muse plugins enable omm` (e2e s05
asserts exactly that string, and running it heals to a green doctor — measured 2026-09-03).

**Repro.**
```
omm install --source <checkout> --yes
muse plugins disable omm
omm doctor --fast --json | jq -r '.checks[] | select(.id=="D1") | .fix'   # muse plugins enable omm
```

**Intended fix.** Add `muse plugins enable omm` to the D1 row (and the same `muse plugins …` prefix
note the s05 loop relies on: a fix line starting with `muse` runs through the host, not omm).
**Owning file:** `docs/INSTALL_FOR_AGENTS.md` (§4 table).

## 14. The documented clean-environment pass-through names only `XDG_*`/`HOME`; the measured set is wider

**Severity:** low (docs). Every host spawn from the CLI and from the MCP server was measured with a
fake Mach-O host under a hostile environment (Gate 2 review round 1): the keys that reach the host
are exactly `PATH HOME XDG_CONFIG_HOME XDG_DATA_HOME TMPDIR TEMP TMP USER LOGNAME SHELL LANG LC_ALL
LC_CTYPE TERM TZ CODEX_HOME` plus the forced `MUSE_NO_AUTO_UPDATE=1 NO_COLOR=1
TBH_DISABLE_FEATURE_CONFIG=1` (`MUSE_EXPERIMENTAL_PLUGINS=1` on `plugins` verbs only); nothing else
(`MUSE_ENABLE_WEB_TOOLS`, `LD_PRELOAD`, `OMM_SOURCE`, `META_API_KEY`, `XDG_CACHE_HOME`,
`TBH_MANAGED_HOOKS_PATH`, `MUSE_HOME`, the plugin variables) leaked, and no hook spawned anything.
ARCHITECTURE §3 says "pass-through `XDG_*`/`HOME`"; README's table calls `omm run`'s environment
"controlled" although `omm run` inherits the caller's environment whole by design and adds only
`MUSE_NO_AUTO_UPDATE=1` and the enabled gates (`invoke.rs` `EnvMode::Inherit`; INSTALL_FOR_AGENTS §6
says so).

**Intended fix.** Name the full `PASSTHROUGH` list and the forced keys in ARCHITECTURE §3, and word
`omm run` as "the caller's environment plus `MUSE_NO_AUTO_UPDATE=1` and the gates" in README's table
and the clap `about`.
**Owning file:** `docs/ARCHITECTURE.md` (§3), `README.md` (Commands table), `crates/omm/src/cli.rs`.

## 15. `repo_lint` does not scan the repository root, `content/` or `plugins/`

**Severity:** low (lint scope). `public_files` walks `crates docs tools scripts`; `README.md`,
`CHANGELOG.md`, `install.sh`, `marketplace.json`, `content/**` and the generated `plugins/omm/**`
are outside it. Grepped by hand on 2026-09-03: clean (the only hits are the `.cursor` *format* name
in `content/skills/omm-skill-port/` and a TUI "cursor" in `content/VALIDATION.md`).

**Intended fix.** Add the root files, `content/` and `plugins/` (`.md .sh .json .toml`) to
`public_files`, with `.cursor`/`.claude`/`.codex` directory names on the accepted-spellings list.
**Owning file:** `crates/omm-host/tests/repo_lint.rs` (`public_files`, `accepted_format_spellings`).

## 16. `omm hook` dies by SIGABRT with a panic on stderr when the host's read end is gone before the decision is written

**Severity:** medium (contract). The hook never blocks by this — a signal death is not exit 2, so
the host fails open (R16) — and the read end is gone only once the host has stopped listening (it
abandoned the hook, or its own pipe closed), so nothing is user-visible. But `cmd/mod.rs` promises
"this process always exits 0" with an empty stderr, and a Rust panic message is neither. `omm mcp`
handles the same situation correctly (`write_frame` returns `Err`, the loop ends, exit 0).

**Repro.**
```
( sleep 0.3; printf '{"hook_event_name":"PreToolUse","tool_name":"bash","tool_input":{"command":"ls"}}' ) \
  | env -i PATH=/usr/bin:/bin HOME=/tmp/h omm hook guard | true; echo "rc=${PIPESTATUS[1]}"
# stderr: thread 'main' panicked at library/std/src/io/stdio.rs:…: failed printing to stdout: Broken pipe (os error 32)
# rc=134 (SIGABRT); the same for every handler and for the `{}` of hook_noop
( sleep 0.3; printf '{"jsonrpc":"2.0","id":1,"method":"ping"}\n' ) | env -i PATH=/usr/bin:/bin HOME=/tmp/h omm mcp | true
# rc=0, stderr empty — the server's write path already tolerates EPIPE
```
(measured 2026-09-03, release binary, Gate 2/3 review round 2.) A closed stdout (`>&-`) is fine:
Rust's stdout swallows EBADF, the hook exits 0 with nothing printed.

**Intended fix.** Print the decision with a fallible write and drop the error —
`let _ = std::io::stdout().lock().write_all(…)` (or `writeln!` with `let _ =`) — in `tune::hook`
and in `hook_noop`, so the only exit path of `omm hook` is 0 whatever the pipe does.
**Owning file:** `crates/omm/src/cmd/tune.rs` (`hook`), `crates/omm/src/cmd/mod.rs` (`hook_noop`).

**Status:** fixed 2026-09-03 — both writers use a fallible `write_all`/`writeln!` and drop the error; the repro above now exits 0 with an empty stderr.

## 17. `tools/pty/drive.py` is committed with absolute paths under a session-scoped scratchpad directory

**Severity:** low (dev tooling, lint scope). The TUI harness of Phase 0 hardcodes `BIN`, `HOME` and
`WS` under `/private/tmp/<agent-session>/…/scratchpad/` (lines 2–4), so it cannot run from a clean
clone, and the directory name carries a vendor-AI product name that `repo_lint` does not catch:
`product_names()` matches the product spelled as a name, not the lowercase token inside a path.
Grepped 2026-09-03: the only such hits in the repository are those three lines.

**Repro.**
```
sed -n 2,4p tools/pty/drive.py      # BIN="/private/tmp/<session>/…/scratchpad/muse-aarch64-macos" …
cargo test -p omm-host --test repo_lint   # green
```

**Intended fix.** Take the three values from the environment (`OMM_MUSE_BIN`, a `--home`/`--ws`
argument or `$OMM_PTY_SANDBOX`) with no defaults, and add a `repo_lint` rule that refuses an
absolute path under `/private/tmp/` or `/tmp/` in any public file (the vendor token then falls out
with it).
**Owning file:** `tools/pty/drive.py`, `crates/omm-host/tests/repo_lint.rs` (`product_names`,
a new path rule).

**Status:** paths fixed 2026-09-03 — `drive.py` reads `OMM_MUSE_BIN` (required), `OMM_PTY_HOME` and `OMM_PTY_WS` (fresh temp dirs by default); the `repo_lint` path rule is still open.

## 18. The passwd lookup (`id -un`, `dscl`) runs with the caller's environment untouched, and `omm mcp` runs it before answering `initialize`

**Severity:** low. `Roots::from_env` → `paths::account_home` spawns `id -un` and `dscl` once per
process (host-reality "Paths": ~20 ms) through `paths::capture`, which neither clears nor
allow-lists the environment — the only children of omm that see the caller's variables whole
(`LD_PRELOAD`, `META_API_KEY`, … measured with a spy `id` on `PATH`, Gate 2/3 review round 2). They
are system tools that read none of it, and for a host-spawned `omm mcp` the environment is already
the 16-key scrub; the hooks use `Roots::from_env_fast` and spawn nothing (0 spawns measured under a
hostile environment for all four handlers). The server side effect is that `omm mcp` runs the
lookup in `Ctx::from_env` before serving, so a `PATH` whose `id` blocks holds the handshake (the
spawn has no timeout).

**Repro.**
```
mkdir -p /tmp/spy && printf '#!/bin/sh\necho "$0 $*" >> /tmp/spy/log\n' > /tmp/spy/id && chmod +x /tmp/spy/id
printf '{"jsonrpc":"2.0","id":1,"method":"ping"}\n' | env -i PATH=/tmp/spy:/usr/bin:/bin HOME=$HOME LD_PRELOAD=/x omm mcp
cat /tmp/spy/log      # /tmp/spy/id -un — spawned before the first frame was read, with LD_PRELOAD in its environment
```

**Intended fix.** `paths::capture` spawns with `env_clear()` and a `PATH` only; issue 12's lazy
`Ctx` (built on the first `tools/call`) takes the lookup out of the server's startup altogether.
**Owning file:** `crates/omm-host/src/paths.rs` (`capture`), `crates/omm/src/cmd/mod.rs`
(`dispatch`, with issue 12).

## 19. `install-provenance.json` is named by the uninstall planner but never written

**Severity:** low (dead reference). `crates/omm-ledger/src/uninstall.rs` lists
`install-provenance.json` among omm's own state files (so it is removed at uninstall) and
ARCHITECTURE §2 shows it in the footprint tree, but nothing writes it: how omm itself was installed
(brew / curl / cargo) is not recorded, so `omm update` cannot yet refuse to self-mutate a
package-manager-owned install (MATRIX S6). Found by the parity audit (`research/ohmy/01-PARITY.md`).

**Repro.** `grep -rn install-provenance crates/ docs/` → one reader, zero writers.

**Intended fix.** `install.sh` and `scripts/release.sh` write it (`{channel, version, installed_by,
bin_dir}`); `omm doctor` reads it (an `omm` binary whose path is not the recorded `bin_dir` is
Info); `omm update` refuses to replace a binary it did not place. **Owning file:** `install.sh`,
`crates/omm/src/cmd/a_lifecycle/session.rs`, `crates/omm-doctor/src/checks/`.

## 20. On non-unix targets the advisory lock is a no-op: concurrent writers are not serialized

**Severity:** low (documented gap; every supported target is unix).
`fsx::try_flock` on non-unix returns `Ok(true)` without calling the OS, so
`lock_exclusive` hands the lock to every waiter at once: two `omm` processes
could interleave a load–modify–save of `omm.lock.json` or a host-config
commit and lose one writer's entries.

**Repro.** Code inspection only (`crates/omm-host/src/fsx.rs`,
`#[cfg(not(unix))] fn try_flock`): the CI matrix is macOS + Linux and no
Windows host build is pinned, so the path is untested by construction.

**Intended fix.** Serialize the way unix does (a named mutex / `LockFileEx`)
once a Windows leg exists to prove it; until then the `lock_exclusive` doc
comment names the gap and points here.
**Owning file:** `crates/omm-host/src/fsx.rs` (`lock_exclusive`, `try_flock`).
