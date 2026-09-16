# Portability: bash vs POSIX sh, and the idioms that replace what sh lacks

Use when the script must carry `#!/bin/sh` (Alpine/busybox, Debian
maintainer scripts, embedded, a repo convention). Run it under `dash` or
`busybox sh` at least once; `bash` in POSIX mode still accepts bashisms.
`shellcheck -s sh` flags most of them; `checkbashisms` (devscripts) catches
the rest.

## What is bash-only

| bash | sh replacement |
|---|---|
| `set -o pipefail` | see "Pipelines without pipefail" below |
| arrays `a=(x y)`, `"${a[@]}"` | the positional parameters: `set -- x y; cmd "$@"`; a second list goes through a NUL-separated file (`find -print0`, `xargs -0`) |
| `[[ ... ]]`, `=~`, `==` | `[ ... ]` with every variable quoted, `=` not `==`; regex via `case` globs or `expr`/`grep -E` |
| `(( n++ ))`, `$(( ))` with `**` | `n=$((n + 1))`; `$(( ))` is POSIX, `**` is not |
| `local` | widely supported (dash, busybox) but not POSIX; acceptable when the target shells are known |
| `${v,,}` `${v^^}` `${v//a/b}` `${v:0:3}` | `tr`, `sed`, `${v#prefix}` / `${v%suffix}` (these two are POSIX) |
| `$'\n'`, `echo -e`, `echo -n` | `printf '%s\n'`, `printf '%b'`; `nl=$(printf '\n.')`; `nl=${nl%.}` |
| `read -d ''`, `read -a`, `mapfile` | `while IFS= read -r line` over a file or `set -- $(...)` only for whitespace-safe tokens |
| `<(cmd)`, `>(cmd)` | a `mktemp` FIFO or a temp file |
| `${BASH_SOURCE[0]}` | `$0` (unreliable when sourced; pass the directory in explicitly) |
| `shopt -s nullglob` | `for f in "$d"/*.log; do [ -e "$f" ] \|\| continue; ...` |
| `trap ... ERR`, `trap ... RETURN` | none; check exits explicitly |
| `function f { }`, `&>` | `f() { }`, `>file 2>&1` |
| `printf %q` | no equivalent; print the arguments one per line, or single-quote them by hand |
| `$RANDOM`, `$SECONDS`, `$EPOCHSECONDS` | `date +%s`; `od -An -N2 -tu2 /dev/urandom` |
| `declare -A` | none: two parallel lists, or a temp directory as a map |

## Pipelines without pipefail

`set -e` sees only the last command of a pipeline. Options, in order of
preference:

1. Avoid the pipe: `out=$(producer) || die "producer failed"; printf '%s\n' "$out" | consumer`.
2. Temp file: `producer >"$tmp" || die ...; consumer <"$tmp"`.
3. Status through a FIFO or a marker line:
   `{ producer || echo '__FAIL__'; } | consumer` and have the consumer stop on the marker.
4. Newer dash and busybox accept `set -o pipefail` (POSIX 2024); probe with
   `(set -o pipefail) 2>/dev/null && set -o pipefail`, and still write the
   script so a shell without it fails loudly rather than silently.

## Iterating files safely on sh

```sh
find "$dir" -maxdepth 1 -type f -name '*.log' -print0 |
  while IFS= read -r f; do ...; done   # WRONG: sh read has no -d ''
```

sh `read` cannot split on NUL. Choose one:

- `find ... -exec sh -c 'for f; do ...; done' sh {} +` (arguments arrive
  intact; the inner `sh -c` gets its own quoting).
- `find ... -print0 | xargs -0 cmd` when the loop body is one command.
- Wildcards in the shell: `for f in "$dir"/*.log; do [ -e "$f" ] || continue; ...; done`
  (no recursion, but no delimiter problem either).
- Only when file names are known to be newline-free: `find ... | while IFS= read -r f`.

Never `for f in $(find ...)`: it splits on spaces and expands globs.

## Cron and unattended scripts

- `PATH` is minimal: set it at the top or use absolute paths for anything
  outside `/usr/bin:/bin`.
- No tty: no prompts, no colour codes, no `sudo` password. Guard with
  `[ -t 0 ]` if the script is also run by hand.
- Home and cwd differ from the shell you tested in: derive paths from the
  script's own location, never from `.`.
- Two copies must not overlap: `flock -n "$lockfile" "$0" "$@"` at the top
  (Linux), or `mkdir "$lockdir" 2>/dev/null || exit 0` with the lock removed
  in the EXIT trap (portable).
- Log with a timestamp and the exit code on the last line; stderr goes to
  mail by default, so silence success and be loud on failure.

## Bash 3.2 (macOS /bin/bash) vs 4+

Missing in 3.2: `mapfile`/`readarray`, `declare -A`, `${v,,}`/`${v^^}`,
`${v@Q}`, `|&`, `;;&` in `case`, `read -N`, `printf -v` (present, 3.1+),
negative array indices. `[[ $s =~ $re ]]` works but the pattern must be
unquoted or in a variable. Test on macOS with `/bin/bash script.sh`, or
guard: `(( BASH_VERSINFO[0] >= 4 )) || die "bash 4+ required (brew install bash)"`.

## shellcheck codes that are almost always real

SC2086 (unquoted expansion), SC2046 (unquoted `$(...)`), SC2006 (backticks),
SC2012 (`ls` parsing), SC2164 (`cd` without `||`), SC2115 (`rm -rf "$var/"`
without `:?`), SC2155 (`local v=$(cmd)` masks the exit), SC2162 (`read`
without `-r`), SC2181 (`$?` checked after the fact), SC2029 (`ssh` argument
expands locally), SC2317 (unreachable: usually a wrong `exit` or `return`).
Disable one only with the reason on the same line.

## Script-relative paths

Never `. ./lib.sh` or `./helper`: the cwd belongs to the caller. bash:
`here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd); . "$here/lib.sh"`.
sh: `here=$(cd "$(dirname "$0")" && pwd)` works when the script is executed,
not when it is sourced; pass the directory in explicitly for a sourced
library.
