---
name: omm-shell
description: Write, fix, or harden a shell script (write a script, bash, sh, .sh, cron job, deploy script): strict mode, quoting, arrays, trap cleanup, dry-run on destructive steps, shellcheck run pasted; Do not use for a one-off interactive command.
---

# Shell script

Contract: the script is done only when your final message pastes the
`shellcheck` run (or why it could not run), the dry run, and a real run in a
scratch directory, verbatim from `bash` output with `echo exit=$?` after
each. "Should work" is not done. A destructive step with no dry-run path is
a bug, not a style choice.

## 0. Decide before typing

- Interpreter. bash, unless the target lacks it: Alpine or busybox images,
  Debian maintainer scripts, a repo already on `#!/bin/sh`. `search`
  (`glob: ["**/*.sh", "scripts/**"]`, pattern `#!`) for the shebang the repo
  uses and match it. macOS `/bin/bash` is 3.2: write `#!/usr/bin/env bash`,
  and no `mapfile`, `declare -A`, `${v,,}` unless the script checks
  `(( BASH_VERSINFO[0] >= 4 )) || die "bash 4+ required"`.
- Wrong tool? Nested data, JSON beyond one `jq` call, floats, retries, more
  than ~200 lines: say once that this wants a real language, then write what
  the user chose.
- Conventions. `read_file` the nearest existing script; copy its header,
  helpers and argument style. Nothing there: two-space indent,
  `scripts/<verb>-<noun>.sh`.
- Tooling. `command -v shellcheck shfmt bats dash` with `bash`. No
  shellcheck: say so and give the install line (`brew install shellcheck` /
  `apt-get install shellcheck`). Three or more steps: `write_todos`.

## 1. Header and failure mode

```bash
#!/usr/bin/env bash
set -euo pipefail
```

sh target: `#!/bin/sh` and `set -eu`; `pipefail` is not portable, so check
each pipeline's producer explicitly (`references/portability.md` beside this
file).

`set -e` is a net, not a guarantee. It does not fire inside `if` or `while`
conditions, in `cmd && x` / `cmd || x` chains, in `local v=$(cmd)` (the
`local` wins the exit code: declare, then assign), in a pipeline without
`pipefail`, or in a function called from a condition. Put `|| die "..."` on
every call whose failure changes the outcome. `-u` catches unset, not empty:
before a destructive path, `: "${dir:?}"`.

## 2. Rules that delete whole bug classes

| Rule | Not this | This |
|---|---|---|
| Quote every expansion | `rm $f` | `rm -- "$f"`; unquoted only for a deliberate glob or split, marked `# intentional` |
| Never parse `ls` | `for f in $(ls *.log)` | `shopt -s nullglob; for f in "$d"/*.log`; deep: `find ... -print0` into `while IFS= read -r -d '' f` |
| Lists are arrays | `opts="-a $x"; cmd $opts` | `opts=(-a "$x"); cmd "${opts[@]}"`; sh: `set -- -a "$x"; cmd "$@"` |
| No `eval`, no command strings | `eval "$cmd $args"` | call the array; a `run "$@"` wrapper when it must be indirect |
| `$(...)`; `[[ ]]` in bash, quoted `[ ]` in sh | backticks; `[ $a == $b ]` | `[[ $a == "$b" ]]`; sh: `[ "$a" = "$b" ]` |
| `mktemp`, cleaned by one `trap` | `/tmp/out.$$` | `tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT` (a second `trap ... EXIT` replaces the first) |
| Guard the root of `rm -rf` | `rm -rf "$dir/"`; `cd "$dir"; rm -rf *` | `rm -rf "${dir:?}/"`; a failed `cd` leaves you in the old cwd |
| `printf`, not `echo`, for data | `echo "$s"` | `printf '%s\n' "$s"` (`-n`, `-e`, a leading `-` all vary) |
| `--` before path arguments | `mv "$f" "$d"` | `mv -- "$f" "$d"`; a path may start with `-` |
| Errors to stderr; usage exits 2 | `echo error` | `die() { printf '%s: %s\n' "${0##*/}" "$*" >&2; exit 1; }` |
| `local` in functions; `${1:-}` under `-u` | `$1` | `name=${1:-}; [[ -n $name ]] \|\| usage` |
| Secrets by env or file, never argv | `--password "$p"` | `ps` shows argv; no `set -x` while a secret is in scope |

## 3. Destructive steps get a dry run

Destructive: `rm`, `mv` onto an existing path, `chmod -R`, `git push`,
`kubectl apply`/`delete`, `DROP`, `ssh host cmd`, a package publish,
anything under `sudo`. Route every one through:

```bash
run() { if (( apply )); then "$@"; else printf 'dry-run:'; printf ' %q' "$@"; echo; fi; }
```

- The default direction is a decision. Unattended (CI, cron): real by
  default, `--dry-run` previews. A human against data or production: preview
  by default, `--apply` executes. State which and why.
- `run` cannot wrap a pipe or redirection: restructure so the printed plan
  is the executed command.
- Print the plan before the first destructive call; prompt only when
  `[[ -t 0 ]]`, never in a cron script.
- Idempotent: a second run reaches the same state with nothing to destroy
  (`mkdir -p`, `ln -sfn`, check before create).
- `sudo` stays outside: `(( EUID == 0 )) || die "run as root"`.

## 4. Lint, then run

- `bash -n <script>` (`sh -n` for portable), then `shellcheck -x <script>`
  (`-s sh` for a POSIX target), `shfmt -d -i 2 -ci` when present; paste each.
  Zero findings, or each `# shellcheck disable=SCnnnn` sits on the line it
  covers with the reason after it; never file-wide.
- A sh target also runs once under `dash` or `busybox sh`.
- Three inputs find most bugs: a path with a space, a name starting with
  `-`, an empty list. Then the error paths: missing directory, bad flag.
- Runs to paste, in a `mktemp -d` tree, never the user's: `--help` (exit 0),
  the dry run, the real run, a second real run (idempotence), one failure
  path (usage on stderr, exit 2; the temp dir gone). A `bats` suite in the
  repo: add a test there.
- Final message: interpreter and why, flag default, shellcheck output, the
  runs, anything not run.

## Judgment calls

- `grep` finds nothing and `set -e` kills the script: decide whether no
  match is an error. Normal: `if grep -q ...; then`. Error: keep the exit,
  add a message. `|| true` only with a comment naming the exit it swallows.
- `cmd | head -1` under `pipefail` exits 141 when `cmd` keeps writing:
  capture the whole output in a variable first.
- Portable sh with a real list: the positional parameters (`set --`) hold
  one list; two lists means bash, or NUL-separated temp files.
- Long-running: `trap 'cleanup; exit 130' INT TERM` beside the EXIT trap;
  `flock -n` (or `mkdir "$lock"` on sh) when two copies must not overlap.
- Existing script: fix what was asked plus what shellcheck rates `error`;
  list the rest, do not rewrite unasked.
- A command the user wants run once: run it with `bash`. No file.

## Refuse

`curl <url> | sh` inside a script. `eval` on anything a user or a file
supplied. `rm -rf` on an unguarded variable. `#!/bin/sh` above bash syntax.
`|| true` or `2>/dev/null` on a failure you have not explained.
`# shellcheck disable` at file top. "shellcheck clean" without the pasted
run. The destructive path executed in the user's tree as the "test". A prompt
in a cron script.

## Micro-example

"Gzip logs older than a week in a directory." Decisions: bash (repo scripts
are bash; macOS laptops, so 3.2 syntax); it rewrites files, so preview by
default and `--apply` to execute; no temp files, so no trap.

```bash
#!/usr/bin/env bash
# rotate-logs: gzip *.log older than DAYS in DIR; plan only unless --apply.
set -euo pipefail

usage() { printf 'usage: %s [--apply] [--days N] DIR\n' "${0##*/}" >&2; exit "${1:-2}"; }
die()   { printf '%s: %s\n' "${0##*/}" "$*" >&2; exit 1; }
run()   { if (( apply )); then "$@"; else printf 'dry-run:'; printf ' %q' "$@"; echo; fi; }

apply=0 days=7
while (( $# )); do
  case $1 in
    --apply)   apply=1 ;;
    --days)    [[ ${2:-} =~ ^[0-9]+$ ]] || usage; days=$2; shift ;;
    -h|--help) usage 0 ;;
    --)        shift; break ;;
    -*)        usage ;;
    *)         break ;;
  esac
  shift
done
(( $# == 1 )) || usage
dir=$1
[[ -d $dir ]] || die "not a directory: $dir"

while IFS= read -r -d '' f; do
  run gzip -- "$f"
done < <(find "$dir" -maxdepth 1 -type f -name '*.log' -mtime +"$days" -print0)
```

`shellcheck -x rotate-logs.sh; echo exit=$?` -> `exit=0`. Scratch runs:

```
$ T=$(mktemp -d); touch -t 202401010000 "$T/a b.log" "$T/-c.log"; touch "$T/new.log"
$ ./rotate-logs.sh "$T"; echo exit=$?
dry-run: gzip -- /tmp/tmp.oErM4/-c.log
dry-run: gzip -- /tmp/tmp.oErM4/a\ b.log
exit=0
$ ./rotate-logs.sh --apply "$T"; echo exit=$?; ls "$T"
exit=0
-c.log.gz  a b.log.gz  new.log
```

Space and leading dash survived (quoting, `--`). Also pasted: a second
`--apply` run (idempotence, `exit=0`); `--days abc` -> usage, `exit=2`;
`/nonexistent` -> `not a directory`, `exit=1`.
