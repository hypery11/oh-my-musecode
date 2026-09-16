# oh-my-zsh: An Architectural Autopsy

**Scope.** This reads oh-my-zsh (OMZ) as a *system design*, not as a user manual. All file paths
are repo-relative to `github.com/ohmyzsh/ohmyzsh` at `master`. Source verified by direct fetch on
2026-09-01.

**Scale at time of writing** (GitHub API, `/repos/ohmyzsh/ohmyzsh`):

| Metric | Value |
|---|---|
| Stars | 189,488 |
| Forks | 26,593 |
| Created | 2009-08-28 |
| Bundled plugins (`plugins/*`) | **359** |
| Bundled themes (`themes/*.zsh-theme`) | **143** |
| Bundled lib files (`lib/*.zsh`) | 21 + `lib/tests/` |
| Open PRs | 420 |
| Open issues | 154 |
| Core loader size | **236 lines** (`oh-my-zsh.sh`) |

The headline number that explains everything else: **the entire framework kernel is 236 lines of
zsh.** Everything else is content. That ratio — a trivially auditable core plus a giant vendored
content library — *is* the architecture.

---

## 1. What OMZ actually is

Zsh already had the extension primitives before OMZ existed:

- `$fpath` — an array of directories searched for autoloadable functions and `_`-prefixed
  completion definitions.
- `autoload -Uz` — lazy function definition.
- `compinit` — builds the completion system from `$fpath`, caching to `.zcompdump`.
- `source` — the entire module system.

OMZ adds **zero new primitives**. It contributes four things:

1. A **naming convention** (`<name>/<name>.plugin.zsh`, `<name>.zsh-theme`) that turns a directory
   into a discoverable unit.
2. A **declaration surface** (`plugins=(...)`, `ZSH_THEME="..."`) placed in the user's `~/.zshrc`.
3. A **deterministic load order** with a documented override seam (`$ZSH_CUSTOM`).
4. A **distribution mechanism** — one git repo, one curl installer, one self-updater.

It is therefore *not* a plugin manager in the antigen/zplug/zinit sense: it does not fetch,
resolve, version, or lock anything third-party. It is a **vendored monorepo + a loader + an
updater**. That distinction is the source of both its wins and its failures.

---

## 2. The directory contract

```
$ZSH/                          # default ~/.oh-my-zsh — a git working tree, not a package dir
├── oh-my-zsh.sh               # the kernel; the only file .zshrc sources
├── lib/                       # 21 always-loaded .zsh files — the "stdlib"
│   ├── cli.zsh                # the `omz` command (944 lines — 4x the kernel)
│   ├── completion.zsh         # zstyle setup for the completion system
│   ├── compfix.zsh            # insecure-fpath detection
│   ├── async_prompt.zsh       # fd-based async prompt segment framework
│   ├── git.zsh, directories.zsh, history.zsh, key-bindings.zsh,
│   ├── theme-and-appearance.zsh, termsupport.zsh, spectrum.zsh,
│   ├── prompt_info_functions.zsh, vcs_info.zsh, functions.zsh,
│   ├── clipboard.zsh, correction.zsh, diagnostics.zsh, grep.zsh,
│   ├── misc.zsh, nvm.zsh, bzr.zsh
│   └── tests/
├── plugins/<name>/            # 359 of these
│   ├── <name>.plugin.zsh      # sourced when enabled
│   ├── _<name>                # optional: zsh completion function (loaded via fpath)
│   └── README.md              # consumed by `omz plugin info`
├── themes/<name>.zsh-theme    # 143 of these
├── templates/
│   ├── zshrc.zsh-template     # the .zshrc the installer writes
│   └── minimal.zshrc          # shown when the user declines overwrite
├── tools/
│   ├── install.sh             # the curl|sh installer (603 lines)
│   ├── upgrade.sh             # the actual updater (295 lines)
│   ├── check_for_upgrade.sh   # the cadence/prompt policy (302 lines)
│   ├── uninstall.sh           # 41 lines
│   ├── changelog.sh, theme_chooser.sh, require_tool.sh
├── custom/                    # == $ZSH_CUSTOM by default; git-ignored
│   ├── example.zsh
│   ├── plugins/               # third-party plugins land here
│   └── themes/                # third-party themes land here
├── cache/                     # == $ZSH_CACHE_DIR by default
│   └── completions/           # generated _<cmd> files, prepended to $fpath
└── log/
    └── update.lock            # mkdir-based mutex for the self-updater
```

### The four variables that *are* the contract

From `oh-my-zsh.sh:52-68`:

```zsh
# If ZSH is not defined, use the current script's directory.
[[ -n "$ZSH" ]] || export ZSH="${${(%):-%x}:a:h}"

# Set ZSH_CUSTOM to the path where your custom config files
# and plugins exists, or else we will use the default custom/
[[ -n "$ZSH_CUSTOM" ]] || ZSH_CUSTOM="$ZSH/custom"

# Set ZSH_CACHE_DIR to the path where cache files should be created
# or else we will use the default cache/
[[ -n "$ZSH_CACHE_DIR" ]] || ZSH_CACHE_DIR="$ZSH/cache"

# Make sure $ZSH_CACHE_DIR is writable, otherwise use a directory in $HOME
if [[ ! -w "$ZSH_CACHE_DIR" ]]; then
  ZSH_CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/oh-my-zsh"
fi
```

Four design notes worth stealing:

- **`$ZSH` self-locates.** `${${(%):-%x}:a:h}` = "absolute directory of the currently sourced
  file." The framework works even if the user never exports `$ZSH`. Relocatable by construction.
- **`$ZSH_CUSTOM` defaults *inside* `$ZSH`** but is overridable to anywhere. This is what lets
  people keep customizations in a separate dotfiles repo while `$ZSH` stays a pristine git clone
  the updater can `git pull --rebase` without conflicts. `custom/` is in the repo's `.gitignore`.
- **`$ZSH_CACHE_DIR` degrades.** If `$ZSH` is read-only (system-wide install, Nix store, container
  image), cache falls back to XDG. Immutable-install support was retrofitted *here*, in three
  lines, rather than by redesigning the layout.
- **Cache dir is prepended to `$fpath` before anything else** (`oh-my-zsh.sh:70-72`), so generated
  completions win over bundled ones:

```zsh
mkdir -p "$ZSH_CACHE_DIR/completions"
(( ${fpath[(Ie)$ZSH_CACHE_DIR/completions]} )) || fpath=("$ZSH_CACHE_DIR/completions" $fpath)
```

Source: <https://github.com/ohmyzsh/ohmyzsh/blob/master/oh-my-zsh.sh>

---

## 3. The boot sequence, exactly

`~/.zshrc` (written by the installer from `templates/zshrc.zsh-template`) does only this:

```zsh
export ZSH="$HOME/.oh-my-zsh"
ZSH_THEME="robbyrussell"
plugins=(git)
source $ZSH/oh-my-zsh.sh
```

**Everything above the `source` line is declaration; everything below is user config.** That single
ordering rule is the whole user-facing mental model, and it is enforced by nothing but the
template's comment placement. (This is also OMZ's most common support burden — see §12.)

`oh-my-zsh.sh` then runs, in order:

| # | Lines | Action |
|---|---|---|
| 1 | 1–47 | **Guard rails.** Refuse to run under bash/sh (prints a `ps` process tree to explain), refuse to run under `emulate sh/ksh`. Written in POSIX sh so the error message survives the wrong interpreter. |
| 2 | 51–68 | Resolve `$ZSH`, `$ZSH_CUSTOM`, `$ZSH_CACHE_DIR` (with writability fallback). |
| 3 | 70–72 | `mkdir -p $ZSH_CACHE_DIR/completions`; prepend to `$fpath`. |
| 4 | 75 | `source "$ZSH/tools/check_for_upgrade.sh"` — **the updater runs before any user code.** |
| 5 | 80 | `fpath=($ZSH/{functions,completions} $ZSH_CUSTOM/{functions,completions} $fpath)` |
| 6 | 83 | `autoload -U compaudit compinit zrecompile` |
| 7 | 85–104 | For each `$plugins` entry: resolve custom-first, then bundled, prepend its dir to `$fpath`. **Print `[oh-my-zsh] plugin 'X' not found` and continue** — never fatal. |
| 8 | 107–137 | Compute `$ZSH_COMPDUMP`, stamp it with OMZ metadata, invalidate if stale (see §5). |
| 9 | 139–150 | `compinit` (with or without compfix). |
| 10 | 152–172 | Append metadata, `zrecompile` the dump under a `mkdir` lock. |
| 11 | 174–213 | Define `_omz_source` — the override + alias-suppression helper. |
| 12 | 216–219 | `for lib_file in $ZSH/lib/*.zsh; _omz_source "lib/${lib_file:t}"` |
| 13 | 222–225 | `for plugin in $plugins; _omz_source "plugins/$plugin/$plugin.plugin.zsh"` |
| 14 | 228–231 | `for config_file in $ZSH_CUSTOM/*.zsh(N); source $config_file` |
| 15 | 234–250 | Resolve and source the theme (three candidate paths). |
| 16 | 253 | `zstyle ':completion:*' list-colors` from `$LS_COLORS`. |

The critical ordering invariant: **`compinit` runs at step 9, before any plugin body is sourced at
step 13, but after every plugin directory is on `$fpath` at step 7.** Two-phase loading —
*register, then initialize, then execute* — is the single most important structural decision in the
file, and it is why the `$fpath` loop is separate from the source loop.

---

## 4. How a plugin is declared and loaded

### Declaration

A plugin is a directory. It is valid if **either** of two files exists —
`oh-my-zsh.sh:85-90`:

```zsh
is_plugin() {
  local base_dir=$1
  local name=$2
  builtin test -f $base_dir/plugins/$name/$name.plugin.zsh \
    || builtin test -f $base_dir/plugins/$name/_$name
}
```

So there are two plugin shapes:

- **Behavior plugin** — `plugins/foo/foo.plugin.zsh`. Sourced into the interactive shell.
- **Completion-only plugin** — `plugins/foo/_foo`. Never sourced; picked up by `compinit` off
  `$fpath`. Costs *nothing* at startup beyond a `$fpath` entry.

There is no manifest. No version. No declared dependencies. No declared capabilities. **The
filename is the entire schema.** Metadata that exists lives in `README.md` and is only surfaced by
`omz plugin info`, which literally pages the README (`lib/cli.zsh:418-453`).

### Enabling

```zsh
plugins=(git docker kubectl zsh-autosuggestions)
```

A plain zsh array, whitespace-separated. The wiki's only formatting warning is emphatic because it
is the #1 syntax error:

> **_NOTE: elements in zsh arrays are separated by whitespace (spaces, tabs, newlines...). DO NOT
> use commas._**
> — <https://github.com/ohmyzsh/ohmyzsh/wiki/Plugins>

`omz plugin enable`/`disable` (`lib/cli.zsh:228-416`) mutate this array **by awk-rewriting the
user's `~/.zshrc` in place**, then validating with `zsh -n` and rolling back on syntax error. It is
a text transform on a config file, not a database.

### Loading

Two loops, deliberately split (`oh-my-zsh.sh:92-104` and `221-225`):

```zsh
# Add all defined plugins to fpath. This must be done
# before running compinit.
for plugin ($plugins); do
  if is_plugin "$ZSH_CUSTOM" "$plugin"; then
    fpath=("$ZSH_CUSTOM/plugins/$plugin" $fpath)
  elif is_plugin "$ZSH" "$plugin"; then
    fpath=("$ZSH/plugins/$plugin" $fpath)
  else
    echo "[oh-my-zsh] plugin '$plugin' not found"
  fi
done
...
# Load all of the plugins that were defined in ~/.zshrc
for plugin ($plugins); do
  _omz_source "plugins/$plugin/$plugin.plugin.zsh"
done
```

Order of the array is order of execution. Last plugin wins on alias/function collisions. There is
no dependency graph and no topological sort — **the user is the dependency resolver.**

### `_omz_source`: override + alias suppression

`oh-my-zsh.sh:174-213`. Two jobs in one function:

```zsh
_omz_source() {
  local context filepath="$1"

  # Construct zstyle context based on path
  case "$filepath" in
  lib/*) context="lib:${filepath:t:r}" ;;         # :t = lib_name.zsh, :r = lib_name
  plugins/*) context="plugins:${filepath:h:t}" ;; # :h = plugins/plugin_name, :t = plugin_name
  esac

  local disable_aliases=0
  zstyle -T ":omz:${context}" aliases || disable_aliases=1

  # Back up alias names prior to sourcing
  local -A aliases_pre galiases_pre
  if (( disable_aliases )); then
    aliases_pre=("${(@kv)aliases}")
    galiases_pre=("${(@kv)galiases}")
  fi

  # Source file from $ZSH_CUSTOM if it exists, otherwise from $ZSH
  if [[ -f "$ZSH_CUSTOM/$filepath" ]]; then
    source "$ZSH_CUSTOM/$filepath"
  elif [[ -f "$ZSH/$filepath" ]]; then
    source "$ZSH/$filepath"
  fi

  # Unset all aliases that don't appear in the backed up list of aliases
  if (( disable_aliases )); then
    ...  # diff and unalias
  fi
}
```

Two things to note:

1. **Custom-first resolution is centralized in one function** (plus the parallel `$fpath` loop and
   the theme block). Three call sites total. The override rule is not scattered.
2. **Alias suppression is a retrofit.** `zstyle ':omz:plugins:git' aliases no` snapshots the
   `aliases`/`galiases` associative arrays, sources the plugin, then diffs and `unalias`es the
   delta. This is a *post-hoc* sandbox bolted onto a system that never had capability declarations.
   It works, but it is the architectural tell: OMZ had to invent a diff-based capability firewall
   because plugins declare nothing about what they do.

### Lazy loading

There is **no framework-level lazy loading.** Every enabled plugin's body is sourced
unconditionally at startup. What exists instead is a **per-plugin convention** — the first lines of
almost every modern OMZ plugin are a cheap bail-out:

```zsh
# plugins/kubectl/kubectl.plugin.zsh:1-3
if (( ! $+commands[kubectl] )); then
  return
fi
```

`$+commands[x]` is a hash lookup against zsh's command table — near-free. So the cost of an
irrelevant plugin is ~1 `source` + 1 hash lookup, not a `which` fork. This convention is *not
enforced*; it spread by code review and copy-paste. Newer plugins also use `&|` (async disown) to
push real work off the startup path — see §5.

The one true lazy mechanism in the tree is `lib/async_prompt.zsh`, an fd-based async framework for
*prompt segments* only, borrowed from `zsh-autosuggestions` and `git-prompt.zsh`:

```zsh
# For now, async prompt function handlers are set up like so:
#  function _git_prompt_status_async { ... }
#  _omz_register_handler _git_prompt_status_async
# Then add a stub prompt function in $PROMPT ... which shows
#  "$_OMZ_ASYNC_OUTPUT[handler_name]"
```

Source: <https://github.com/ohmyzsh/ohmyzsh/blob/master/lib/async_prompt.zsh>

---

## 5. The completion-cache problem

This is the hardest engineering in the repo and the part most worth studying.

### The problem, in three parts

**(a) `compinit` is expensive and must run exactly once, at the right time.** It walks every
directory in `$fpath`, parses every `_`-prefixed file, and writes a compiled dump. Profiling of
real OMZ configs attributes **~42% of total startup** to `compinit` alone
(<https://unixy.io/blog/case-against-oh-my-zsh/>). It must run *after* all plugin dirs are on
`$fpath` (or their completions are invisible) and *before* plugin bodies run (or plugins can't call
`compdef`). Hence the two-loop split in §4.

**(b) The dump cache goes stale invisibly.** Zsh's own staleness check is weak. OMZ therefore
stamps its own metadata into the dump and invalidates on *its own* notion of change —
`oh-my-zsh.sh:106-125`:

```zsh
# Save the location of the current completion dump file.
if [[ -z "$ZSH_COMPDUMP" ]]; then
  ZSH_COMPDUMP="${ZDOTDIR:-$HOME}/.zcompdump-${SHORT_HOST}-${ZSH_VERSION}"
fi

# Construct zcompdump OMZ metadata
zcompdump_revision="#omz revision: $(builtin cd -q "$ZSH"; git rev-parse HEAD 2>/dev/null)"
zcompdump_fpath="#omz fpath: $fpath"

# Delete the zcompdump file if OMZ zcompdump metadata changed
if ! command grep -q -Fx "$zcompdump_revision" "$ZSH_COMPDUMP" 2>/dev/null \
   || ! command grep -q -Fx "$zcompdump_fpath" "$ZSH_COMPDUMP" 2>/dev/null; then
  command rm -f "$ZSH_COMPDUMP"
  zcompdump_refresh=1
fi
```

The cache key is **(git HEAD of $ZSH) × (the full `$fpath` string) × (hostname) × (zsh version)**.
Changing your plugins array changes `$fpath`, which busts the cache exactly once, automatically.
Updating OMZ changes HEAD, which busts it. Sharing `$HOME` over NFS between hosts or zsh versions
gets separate dumps. This is a **content-addressed cache with a hand-rolled composite key**, and it
is the right answer: it fixed a years-long class of "my completions are wrong after updating" bugs.

Note the cost, though: `git rev-parse HEAD` **forks git on every single shell startup**. That is a
real, unconditional tax paid to keep the cache honest.

**(c) Tool-generated completions are slow to produce.** `kubectl completion zsh` takes 100–400ms.
You cannot run that at startup. OMZ's answer is a **write-behind cache into
`$ZSH_CACHE_DIR/completions/`** — `plugins/kubectl/kubectl.plugin.zsh:5-17`, verbatim:

```zsh
# If the completion file doesn't exist yet, we need to autoload it and
# bind it to `kubectl`. Otherwise, compinit will have already done that.
if [[ ! -f "$ZSH_CACHE_DIR/completions/_kubectl" ]]; then
  typeset -g -A _comps
  autoload -Uz _kubectl
  _comps[kubectl]=_kubectl
fi

zmodload -F zsh/files b:zf_mv
() {
  local TMPPREFIX="$ZSH_CACHE_DIR/completions/_kubectl"
  zf_mv -f -- =( kubectl completion zsh 2> /dev/null ) "$TMPPREFIX"
} &|
```

Read that carefully — it is a five-part trick:

1. **Cold path**: file missing → manually `autoload` the function name and hand-register it in
   `_comps` so *this* session still completes, without a second `compinit`.
2. **Regeneration is unconditional but backgrounded**: `&|` = run in background *and* disown, so
   the shell never waits and never reports the job.
3. `=( ... )` is zsh's **process substitution to a temp file** — the subprocess output lands on
   disk, not in a pipe.
4. `zf_mv` is the **builtin** move from `zsh/files`; no `fork`/`exec` of `/bin/mv`. Atomic rename.
5. **Warm path**: on the next shell, the file exists and is already on `$fpath` (which was
   prepended at `oh-my-zsh.sh:71`), so `compinit` picks it up with zero extra work — and the
   `$fpath` string is unchanged, so the dump is *not* busted.

The same pattern is copy-pasted verbatim into `plugins/gh/gh.plugin.zsh`, and dozens of others.
Which is the tell again: **this is a framework-level concern implemented as a per-plugin idiom.**
There is no `omz_cache_completion <cmd> <generator>` helper. 30+ plugins each carry their own copy
of that 12-line block, so a bug fix in the pattern requires 30+ PRs.

### The rest of the completion machinery

- `compinit -i -d "$ZSH_COMPDUMP"` by default; `-u` if `ZSH_DISABLE_COMPFIX=true`
  (`oh-my-zsh.sh:139-150`). `lib/compfix.zsh` handles the "insecure directories" warning, deferred
  to a background job (`&|`) so it doesn't block the prompt.
- `zrecompile -q -p "$ZSH_COMPDUMP"` under a `mkdir`-based lock (`oh-my-zsh.sh:155-158`) compiles
  the dump to `.zwc` bytecode. `mkdir` is used as the mutex primitive because it is atomic on every
  filesystem and needs no flock.
- `omz plugin load` (`lib/cli.zsh:493-539`) exists for loading a plugin into the *current* session
  without editing `.zshrc`, and correctly calls `compinit -D -d "$_comp_dumpfile"` — `-D` meaning
  "don't write a dump" — so a temporary load cannot corrupt the persistent cache. That comment in
  the source is a masterclass in cache discipline.

---

## 6. How a theme is declared and loaded

Simpler, and deliberately so. A theme is **one file**: `themes/<name>.zsh-theme`. Declared as a
scalar:

```zsh
ZSH_THEME="robbyrussell"
ZSH_THEME="random"                                  # picks one at random
ZSH_THEME_RANDOM_CANDIDATES=( "robbyrussell" "agnoster" )   # constrain the random pool
```

Loaded **last**, after libs, plugins, and `custom/*.zsh` (`oh-my-zsh.sh:233-250`):

```zsh
is_theme() {
  local base_dir=$1
  local name=$2
  builtin test -f $base_dir/$name.zsh-theme
}

if [[ -n "$ZSH_THEME" ]]; then
  if is_theme "$ZSH_CUSTOM" "$ZSH_THEME"; then
    source "$ZSH_CUSTOM/$ZSH_THEME.zsh-theme"
  elif is_theme "$ZSH_CUSTOM/themes" "$ZSH_THEME"; then
    source "$ZSH_CUSTOM/themes/$ZSH_THEME.zsh-theme"
  elif is_theme "$ZSH/themes" "$ZSH_THEME"; then
    source "$ZSH/themes/$ZSH_THEME.zsh-theme"
  else
    echo "[oh-my-zsh] theme '$ZSH_THEME' not found"
  fi
fi
```

**Three** search paths, not two — `$ZSH_CUSTOM/` root is checked before `$ZSH_CUSTOM/themes/`, a
backwards-compat wart. Theme-last is the key ordering choice: it means a theme can freely override
prompt variables that any lib or plugin set, and it is why powerlevel10k et al. can be installed as
"themes" despite being far larger than any plugin.

The theme/plugin split is a **capability split, not a technical one** — both are just sourced zsh.
Themes are conventionally allowed to set only `PROMPT`/`RPROMPT`/`ZSH_THEME_*` variables. Nothing
enforces this.

---

## 7. The `$ZSH_CUSTOM` override mechanism

The mechanism that let OMZ survive being a monorepo. Three override surfaces, all resolved
custom-first, none requiring a fork:

| To override | Put a file at | Resolved by |
|---|---|---|
| A bundled **lib** file | `$ZSH_CUSTOM/lib/<name>.zsh` | `_omz_source` (`oh-my-zsh.sh:186-191`) |
| A bundled **plugin** | `$ZSH_CUSTOM/plugins/<name>/<name>.plugin.zsh` | `is_plugin` loop + `_omz_source` |
| A bundled **theme** | `$ZSH_CUSTOM/themes/<name>.zsh-theme` | `is_theme` chain |
| Anything at all, last | `$ZSH_CUSTOM/*.zsh` | glob loop (`oh-my-zsh.sh:228-231`) |

From the wiki (<https://github.com/ohmyzsh/ohmyzsh/wiki/Customization>):

> To replace an existing plugin, place your custom version at `$ZSH_CUSTOM/plugins/<plugin_name>/`
> — this "will override the entire plugin: your custom plugin files will be loaded _instead_ of the
> files from the original plugin."

> You can "fully override an existing `lib/*.zsh` file by providing a `$ZSH_CUSTOM/lib/<name>.zsh`
> file of the same name. It will be loaded instead of the corresponding base lib file."

And from `custom/example.zsh` in the repo itself:

```
# Files in the custom/ directory will be:
# - loaded automatically by the init script, in alphabetical order
# - loaded last, after all built-ins in the lib/ directory, to override them
# - ignored by git by default
```

**Critical properties:**

- **Whole-file replacement, not merge or patch.** There is no partial override, no
  `super`/`next()`, no hook point. You take the whole file or none of it. Simple to reason about;
  brutal on maintenance — your copied `lib/git.zsh` silently stops receiving upstream fixes
  forever, with no drift warning.
- **`custom/` is git-ignored inside `$ZSH`.** This is the load-bearing detail. The updater does
  `git pull --rebase` on `$ZSH`; because customizations live in an ignored subtree, **the update
  path can never conflict with user data.** Separating mutable user state from immutable framework
  content *inside the same directory* — and enforcing it with `.gitignore` — is the trick that made
  a git-clone-as-package-manager viable for 15 years.
- **`ZSH_CUSTOM` can point anywhere**, e.g. `~/dotfiles/omz-custom`, so customizations can live in
  the user's own versioned repo while `$ZSH` stays disposable.

---

## 8. The `omz` CLI

`lib/cli.zsh` is 944 lines — **four times the kernel.** It is a shell function dispatcher:

```zsh
function omz {
  ...
  (( ${+functions[_omz::$command]} )) || { _omz::help; return 1; }
  _omz::$command "$@"
}
```

Commands (`_omz::help`):

```
  help                Print this help message
  changelog           Print the changelog
  plugin <command>    Manage plugins       (disable|enable|info|list|load)
  pr     <command>    Manage Oh My Zsh Pull Requests   (clean|test)
  reload              Reload the current zsh session
  shop                Open the Oh My Zsh shop
  theme  <command>    Manage themes        (list|set|use)
  update              Update Oh My Zsh
  version             Show the version
```

Design notes:

- **`omz plugin enable/disable` and `omz theme set` edit `~/.zshrc` with awk**, `zsh -n`-validate,
  and roll back on parse failure (`lib/cli.zsh:228-416`, `796-861`). Config is a *file the user
  owns*, and the CLI is a careful text editor of it, not a competing source of truth.
- **`omz theme use` vs `omz theme set`** — `use` applies for the session only, `set` persists.
  Try-then-commit as two distinct verbs.
- **`omz reload` is `exec zsh`** with the compdump deleted first:

```zsh
function _omz::reload {
  command rm -f $_comp_dumpfile $ZSH_COMPDUMP
  local zsh="${ZSH_ARGZERO:-${functrace[-1]%:*}}"
  [[ "$zsh" = -* || -o login ]] && exec -l "${zsh#-}" || exec "$zsh"
}
```

  No incremental reload machinery. Process replacement, correctly preserving login-shell status.
  Cheap and always correct.
- **`omz pr test <N>`** fetches a PR branch, rebases it on master, and `exec`s a subshell to test
  it — with a **security gate**: it checks the PR for a `testers needed` label and warns

  > "PR #$1 does not have the 'testers needed' label. This means that the PR has not been reviewed
  > by a maintainer and may contain malicious code."

  A code-review-gated preview channel built out of `git fetch` and a GitHub label. There is no
  staging registry; the label *is* the channel.
- **`omz version`** derives from git: `git describe --tags` → `symbolic-ref` → `name-rev` →
  `<detached>`, plus short hash. **The framework has no version number of its own.** Its version is
  a git commit.

---

## 9. The installer

`tools/install.sh`, 603 lines, POSIX sh (not zsh — it must run before zsh is confirmed present),
`set -e`.

Canonical invocation:

```sh
sh -c "$(curl -fsSL https://raw.githubusercontent.com/ohmyzsh/ohmyzsh/master/tools/install.sh)"
```

### Its parameterization surface (`install.sh:16-36`)

```
#   ZSH     - path to the Oh My Zsh repository folder (default: $HOME/.oh-my-zsh)
#   REPO    - name of the GitHub repo to install from (default: ohmyzsh/ohmyzsh)
#   REMOTE  - full remote URL of the git repo to install (default: GitHub via HTTPS)
#   BRANCH  - branch to check out immediately after install (default: master)
#   CHSH                   - 'no' means the installer will not change the default shell (default: yes)
#   RUNZSH                 - 'no' means the installer will not run zsh after the install (default: yes)
#   KEEP_ZSHRC             - 'yes' means the installer will not replace an existing .zshrc (default: no)
#   OVERWRITE_CONFIRMATION - 'no' means the installer will not ask for confirmation (default: yes)
#
#   --skip-chsh, --unattended, --keep-zshrc
```

`REPO`/`REMOTE`/`BRANCH` mean **the installer can install a fork** — the same one-liner works for
enterprise forks and for testing branches. That is a deliberate escape hatch in a `curl|sh`.

### What it mutates

1. **Clones `$ZSH`** — but note, *not* with `git clone` (`install.sh:307-350`):

```sh
  umask g-w,o-w   # prevent insecure perms -> compinit "command not found: compdef" errors
  ...
  git init --quiet "$ZSH" && cd "$ZSH" \
  && git config core.eol lf \
  && git config core.autocrlf false \
  && git config fsck.zeroPaddedFilemode ignore \
  && git config fetch.fsck.zeroPaddedFilemode ignore \
  && git config receive.fsck.zeroPaddedFilemode ignore \
  && git config oh-my-zsh.remote origin \
  && git config oh-my-zsh.branch "$BRANCH" \
  && git remote add origin "$REMOTE" \
  && git fetch --depth=1 origin \
  && git checkout -b "$BRANCH" "origin/$BRANCH"
```

   `init`+`fetch --depth=1` instead of `clone` for git <1.7.2 compat and a shallow first fetch.
   `oh-my-zsh.remote`/`oh-my-zsh.branch` are **custom git-config keys used as the updater's
   channel configuration** — a package manager's channel table stored in `.git/config`.
   The `umask g-w,o-w` at the top exists solely because zsh's `compaudit` refuses group-writable
   `$fpath` dirs.

2. **Replaces `~/.zshrc`**, backing up to `~/.zshrc.pre-oh-my-zsh` (`install.sh:354-414`). If a
   backup already exists it refuses to clobber it. If `KEEP_ZSHRC=yes`, it prints
   `templates/minimal.zshrc` and tells you what to add yourself. It respects `$ZDOTDIR`, and
   rewrites the `export ZSH=` line to the literal `$ZDOTDIR`/`$HOME` form:

```sh
  sed "s|^export ZSH=.*$|export ZSH=\"${omz}\"|" "$ZSH/templates/zshrc.zsh-template" > "$zdot/.zshrc-omztemp"
  mv -f "$zdot/.zshrc-omztemp" "$zdot/.zshrc"
```

3. **Runs `chsh -s $(which zsh)`** (`install.sh:418-510`) — the most invasive act. It records the
   old shell to `~/.shell.pre-oh-my-zsh`, validates the target against `/etc/shells`, and tries
   with and without `sudo`. On failure it degrades to a printed instruction rather than aborting.

4. **`exec zsh -l`** at the end unless `RUNZSH=no`.

`tools/uninstall.sh` (41 lines) reverses all three: `chsh` back from `~/.shell.pre-oh-my-zsh`,
`rm -rf ~/.oh-my-zsh`, move the current `.zshrc` aside with a timestamp and restore
`.zshrc.pre-oh-my-zsh`. **Every mutation has a named restore file.** That symmetry is why people
trusted a `curl|sh` that changes their login shell.

---

## 10. The self-updater

Three files, clean policy/mechanism split:

- `tools/check_for_upgrade.sh` (302 lines) — **policy**: when to check, when to ask, when to skip.
  Sourced at `oh-my-zsh.sh:75`, i.e. before any user code.
- `tools/upgrade.sh` (295 lines) — **mechanism**: the actual `git pull --rebase`, run as
  `zsh -f` (no rcs) in a clean environment: `LANG= ZSH="$ZSH" zsh -f "$ZSH/tools/upgrade.sh" -i -v $verbose_mode`.
- `lib/cli.zsh :: _omz::update` — **manual trigger**, same mechanism.

### Cadence and modes

```zsh
# - prompt (default): the user is asked before updating when it's time to update
# - auto: the update is performed automatically when it's time
# - reminder: a reminder is shown to the user when it's time to update
# - background-alpha: an experimental update-on-the-background option
# - disabled: automatic update is turned off
zstyle -s ':omz:update' mode update_mode || { ... }
```

Configured in `.zshrc` via `zstyle`, with legacy `DISABLE_UPDATE_PROMPT` / `DISABLE_AUTO_UPDATE`
booleans still honored as a fallback branch. **Default frequency is 13 days** (a prime number, so
update prompts don't synchronize with weekly rhythms):

```zsh
zstyle -s ':omz:update' frequency epoch_target || epoch_target=${UPDATE_ZSH_DAYS:-13}
```

State lives in `$ZSH_CACHE_DIR/.zsh-update` as sourceable shell:

```
LAST_EPOCH=20334
EXIT_STATUS=0
ERROR='Update successful'
```

with a migration shim at the top of the file for the old `~/.zsh-update` location.

### The bail-out conditions — the interesting part

```zsh
if [[ "$update_mode" = disabled ]] \
   || [[ ! -w "$ZSH" || ! -O "$ZSH" ]] \
   || [[ ! -t 1 && ${POWERLEVEL9K_INSTANT_PROMPT:-off} == off ]] \
   || ! command git --version >/dev/null 2>&1 \
   || (builtin cd -q "$ZSH"; ! command git rev-parse --is-inside-work-tree &>/dev/null); then
  unset update_mode
  return
fi
```

Refuses to update when: disabled, `$ZSH` isn't writable *or isn't owned by you* (system-wide
installs, root-owned clones), stdout isn't a tty (scripts, CI, `ssh host cmd`), git is missing, or
`$ZSH` isn't a git repo (tarball/distro-package installs). **The self-updater knows it might not be
the thing that owns the install.** That check is why OMZ can be packaged by Homebrew/Nix/apt
without fighting them.

### Concurrency and interruption

```zsh
# Remove lock directory if older than a day
if mtime=$(zstat +mtime "$ZSH/log/update.lock" 2>/dev/null); then
  if (( (mtime + 3600 * 24) < EPOCHSECONDS )); then
    command rm -rf "$ZSH/log/update.lock"
  fi
fi

# Check for lock directory
if ! command mkdir "$ZSH/log/update.lock" 2>/dev/null; then
  return
fi

trap "ret=\$?; ...; command rm -rf '$ZSH/log/update.lock'; return \$ret" EXIT INT QUIT
```

`mkdir` as an atomic mutex, with a 24-hour stale-lock reaper, and a trap that **preserves the exit
status through SIGINT** so Ctrl-C during an update still exits the shell correctly. Opening ten
terminals at once produces one update attempt, not ten.

### The remote check

`is_update_available()` does *not* fetch. It resolves the remote to `owner/repo`, and only if it is
literally `ohmyzsh/ohmyzsh` does it hit
`https://api.github.com/repos/ohmyzsh/ohmyzsh/commits/master` with
`Accept: application/vnd.github.v3.sha`, `--connect-timeout 2`, falling back curl → wget → fetch.
It then `git merge-base`s local vs remote HEAD to distinguish "behind" from "diverged". **A 2-second
HEAD-SHA check instead of a full `git fetch`** — the cheapest possible "is there news" probe. For
forks, it assumes updates are available and lets `git pull --rebase` decide.

### The prompt

```zsh
# If in reminder mode or user has typed input, show reminder and exit
if [[ "$update_mode" = reminder ]] || { ... has_typed_input }; then
  printf '\r\e[0K'
  echo "[oh-my-zsh] It's time to update! You can do that by running \`omz update\`"
  return 0
fi
...
printf "[oh-my-zsh] Would you like to update? [Y/n] "
read -r -k 1 option
```

`has_typed_input()` (credited in-source to Philippe Troin, from a zsh-users mailing list post) uses
`stty -icanon` + `zselect -t 0 -r 0` to detect whether the user has *already started typing* into
the new shell — and if so, degrades from a blocking prompt to a one-line reminder, then restores
stty in an `always` block. **They will not steal your keystrokes.** This is the most
user-respectful piece of code in the repo, and it is 20 lines to avoid one class of annoyance.

There is also `POWERLEVEL9K_INSTANT_PROMPT` special-casing in three places — the framework
explicitly defers to a *third-party theme's* startup contract. Real-world coupling, honestly
handled.

---

## 11. Third-party distribution: there is no registry

**How it works today.** For anything not in the monorepo, the entire published procedure is:

```sh
git clone https://github.com/zsh-users/zsh-autosuggestions \
  ${ZSH_CUSTOM:-~/.oh-my-zsh/custom}/plugins/zsh-autosuggestions
# then add `zsh-autosuggestions` to plugins=() in ~/.zshrc
```

The discovery layer is a **hand-edited wiki page**:
<https://github.com/ohmyzsh/ohmyzsh/wiki/External-plugins> — a categorized list (CLI, FUN, GIT,
GPG, NOTES, NODE, PYTHON, SSH, UNORGANISED) of names, GitHub links, and descriptions. That page
frames itself as addressing

> "a common concern: uncertainty about whether your plugin might inadvertently cause harm,
> potentially disrupting the system or its functionalities."

…but establishes no vetting, no signing, no scanning, no ownership verification. It is a
bookmark list.

### Everything a registry would have given you, and doesn't exist

| Registry function | OMZ status |
|---|---|
| Discovery / search | Wiki page, manually curated, no search, no ranking |
| Install command | None. `git clone` into a path you must type correctly |
| Uninstall | `rm -rf` |
| Versioning | None. You get whatever `master` is at clone time |
| Update | None. **Third-party plugins never update.** `omz update` pulls `$ZSH` only; `custom/` is git-ignored |
| Lockfile / reproducibility | None |
| Dependency declaration | None |
| Namespace / name collision | First match wins by directory name; two authors can both ship `docker` |
| Integrity / signing | None |
| Deprecation / yank | None |
| Telemetry / popularity signal | GitHub stars, by hand |

### The pain this causes, concretely

1. **Silent staleness.** The two most-installed OMZ plugins in the world
   (`zsh-autosuggestions`, `zsh-syntax-highlighting`) are third-party and therefore *frozen at
   clone time* for most users. Millions of installs sit years behind upstream, and OMZ has no way
   to know or tell them.
2. **The most common install error is a path typo.** `${ZSH_CUSTOM:-~/.oh-my-zsh/custom}` inside a
   copy-pasted command is a footgun with three failure modes (unset var, wrong quoting, wrong
   nesting depth). The resulting symptom — `[oh-my-zsh] plugin 'X' not found` — points at the
   plugins array, not at the clone.
3. **The nesting mistake.** Cloning to `custom/plugins/` instead of
   `custom/plugins/<name>/` puts `<name>.plugin.zsh` at the wrong depth and `is_plugin` fails. The
   convention is entirely positional and unverified.
4. **`git clone` is `curl | sh` with extra steps.** Every third-party plugin is arbitrary code
   sourced into your interactive login shell, from an unaudited URL, with no signature, forever.
   The framework's only defense is the `omz pr test` "testers needed" label — which applies only to
   PRs against the monorepo, i.e. exactly the code that *isn't* the risk.
5. **All curation pressure lands on the monorepo.** Because "in the repo" is the only quality
   signal, every plugin author wants in. Result: 420 open PRs, and formal contribution freezes.
   Themes are closed outright:

   > "We have enough themes for the time being. Please fork the project and add on in there, you
   > can let people know how to grab it from there."
   > — <https://github.com/ohmyzsh/ohmyzsh/wiki/Themes#dont-send-us-your-theme-for-now>

   And `CONTRIBUTING.md` now gates *aliases* with a five-point justification test:

   > "Because of this, from now on, we require that new aliases follow these conditions:
   > 1. They will be used by many people, not just a few. 2. The aliases will be used many times
   > and for common tasks. 3. Prefer one generic alias over many specific ones. …
   > Please remember that your alias will be in the machines of many people."

   That is a maintainer team **rate-limiting the ecosystem through review capacity** because there
   is no other place for contributions to go.
6. **The vacuum got filled by competitors.** antigen, zplug, zgen, zinit, antibody, znap, and
   sheldon all exist primarily to be *the registry/installer OMZ never shipped*. Several of them
   advertise "loads oh-my-zsh plugins" — i.e. they adopted OMZ's *convention* and replaced its
   *distribution*. The convention won; the distribution lost.

---

## 12. Startup cost and the criticism

### Numbers

| Configuration | Startup |
|---|---|
| Bare zsh, no rc | ~10–25 ms |
| Vanilla zsh, small `.zshrc` | ~50–100 ms |
| Well-tuned OMZ | ~150–300 ms |
| Typical OMZ (10–15 plugins) | ~400 ms |
| Heavy OMZ + version managers | 1–5 s |

Sources: <https://unixy.io/blog/case-against-oh-my-zsh/> (400 ms vs 25 ms minimal, **16×**),
<https://jonlu.ca/posts/speeding-up-zsh>, <https://blog.mattclemente.com/2020/06/26/oh-my-zsh-slow-to-load/>,
<https://github.com/ohmyzsh/ohmyzsh/issues/8536> ("4-5s"),
<https://github.com/ohmyzsh/ohmyzsh/discussions/12642>.

### Where it goes

Profiling (`zmodload zsh/zprof`) of a representative config attributes:

- **~42.5% — `compinit`.** Structural, not incidental: it scales with `$fpath` size, and `$fpath`
  grows with every enabled plugin.
- **~21% — OMZ's own sourcing.** 21 unconditional `lib/*.zsh` files plus N plugins; "23 calls to
  `source` before the prompt appears" in the cited profile.
- The remainder — per-plugin `$+commands` probes, `git rev-parse HEAD` for the compdump stamp,
  `scutil --get LocalHostName` on macOS (`oh-my-zsh.sh:98`), the `check_for_upgrade` state read,
  and whatever version managers the user added.

### The architectural criticisms, stated fairly

1. **`lib/` is 21 files with no opt-out.** Plugins are opt-in; the "stdlib" is not. If you want
   OMZ's plugin loader you also get its history options, key bindings, `ls` colors, correction
   setup, terminal-title hooks, and clipboard shims. The `$ZSH_CUSTOM/lib/<name>.zsh` override lets
   you *replace* a lib file with an empty one, but there is no `omz_libs=(...)` array. This is the
   clearest place where the framework is not composable.
2. **Everything is eager.** No manifest means the framework cannot know that `plugins/kubectl`
   contributes only aliases + a completion, so it cannot defer it. The `(( ! $+commands[x] ))`
   guard is a *convention* mitigating a *structural* absence.
3. **Namespace pollution with silent collisions.** The `git` plugin defines 150+ aliases; a typical
   config ends up with 200+. `gc`, `gs`, `gp` shadow real binaries (`gc` is a GHC/Go tool on some
   systems; `gs` is Ghostscript on many). OMZ warns about none of it. The critique lands hard:

   > "Your shell is no longer yours; it belongs to Oh-My-Zsh, with your customizations layered on
   > top."

   The `zstyle ':omz:plugins:git' aliases no` escape hatch exists, is per-plugin, and is
   undiscoverable — it's documented in the wiki, not in the generated `.zshrc`.
4. **Cost is invisible and unattributed.** There is no `omz doctor`, no `omz profile`, no per-plugin
   timing. Users experience "my shell is slow," and the debugging path is to learn `zprof` and read
   235 lines of framework. Contrast: the framework *does* ship `lib/diagnostics.zsh`, but it dumps
   config for bug reports — it does not measure.
5. **The fair rebuttal.** Most of the 1–5s reports are `nvm`/`rbenv`/`pyenv`/`conda` init blocks the
   user pasted below `source $ZSH/oh-my-zsh.sh`, which OMZ neither installed nor controls. And 400ms
   was genuinely acceptable in 2012 on hardware where nothing was fast. The criticism is really
   "OMZ optimized for capability-per-line-of-config in an era when that was the binding constraint,
   and never re-optimized when startup latency became the binding constraint."

---

## 13. The 8 design decisions that made oh-my-zsh win

1. **Convention over configuration, taken to the extreme: the filename is the schema.**
   `plugins/foo/foo.plugin.zsh` and `themes/bar.zsh-theme`. No manifest, no registration, no
   metadata file, no build step. `is_plugin()` is 5 lines. The cost of *authoring* a plugin dropped
   to "make a directory and a file," which is why 359 shipped and thousands more exist. Every
   competitor that required a manifest lost the authoring-volume race.

2. **A one-line install that leaves you in a working, better shell.**
   `sh -c "$(curl -fsSL .../install.sh)"` clones, writes a `.zshrc` with a good default theme and
   `plugins=(git)` already on, `chsh`es you, and `exec`s zsh. Time-to-visible-value is ~15 seconds,
   and the visible value is a **prompt that looks different** — the change is instantly legible to
   the user and to anyone watching over their shoulder. Word-of-mouth was a *screenshot*.

3. **Ship a huge curated default library, in-repo, vendored.**
   359 plugins and 143 themes arrive with the clone. No install step, no network, no resolution, no
   version conflicts, no supply chain. Enabling a plugin is *editing an array* — a sub-second,
   zero-risk, reversible act. This is the decision that made "batteries included" a distribution
   strategy rather than a slogan.

4. **A three-line declaration surface in a file the user already owns.**
   `ZSH_THEME=`, `plugins=()`, `source $ZSH/oh-my-zsh.sh`. The config format is *the language the
   config is written in*, so users can compute it, conditionalize it, and share it. Dotfiles repos
   became the distribution channel for OMZ configurations, and `.zshrc` diffs became a social
   object.

5. **`$ZSH_CUSTOM` as a git-ignored escape hatch inside the install.**
   Whole-file override for libs, plugins, and themes; plus `custom/*.zsh` loaded last for
   everything else; plus the ability to relocate `$ZSH_CUSTOM` into your own repo. Because it is
   `.gitignore`d, **`git pull --rebase` on `$ZSH` can never conflict with user data.** This is the
   single decision that made "the package manager is git" survivable for 15 years.

6. **Deterministic, documented, boring load order — and it never changed.**
   libs → plugins (array order) → `custom/*.zsh` → theme. Last writer wins. No dependency graph, no
   priority numbers, no lifecycle phases. Users can predict and debug the result by reading a
   linear file. The ordering has been stable long enough that the entire ecosystem's override
   idioms depend on it, and it has never been broken.

7. **A self-updater that respects ownership, concurrency, and attention.**
   13-day cadence, `zstyle`-configurable modes (prompt/auto/reminder/disabled), a 2-second GitHub
   SHA probe instead of a fetch, `mkdir` lock with a stale reaper, refusal to run when `$ZSH` isn't
   yours or stdout isn't a tty, and `has_typed_input()` so it never eats your keystrokes. The
   *social* effect: an install from 2014 that a user never touched is running current code today.
   OMZ ships to its installed base, which no dotfiles repo can do.

8. **Two-phase loading — register `$fpath`, then `compinit`, then execute — with a
   content-addressed dump cache.**
   The `$fpath` loop and the source loop are separate, with `compinit` between them, so
   completion-only plugins cost nothing and behavior plugins can call `compdef`. The dump is keyed
   on `git HEAD × $fpath × hostname × zsh version`, so editing `plugins=()` invalidates it exactly
   once, automatically. This is genuinely good cache engineering and it eliminated a whole bug
   class.

**Bonus (9): every mutation has a named restore file.** `.zshrc.pre-oh-my-zsh`,
`.shell.pre-oh-my-zsh`, `.zshrc.omz-uninstalled-<timestamp>`. A 41-line uninstaller that actually
works is what earns permission for a 603-line installer that runs `chsh`.

---

## 14. The 5 decisions that aged badly

1. **No plugin manifest — so no capabilities, no versions, no dependencies, no lazy loading, ever.**
   The filename-as-schema decision that won §13.1 is the same one that made every later problem
   unsolvable. Because a plugin declares nothing, the framework cannot: defer it, sandbox it,
   diff-check it for conflicts, version it, or tell the user what it will do before enabling it.
   Every mitigation is therefore a retrofit implemented *inside* the loader by *observing side
   effects* — most visibly the alias diff-and-unalias in `_omz_source`. **The lesson is not "always
   ship a manifest"; it is that the cheapest possible unit boundary buys adoption and then charges
   compound interest forever.**

2. **No registry, and no plan to become one.**
   Third-party plugins are `git clone` into a git-ignored directory. They therefore have no
   install, no update, no uninstall, no version, no lock, no namespace, no integrity check, and no
   deprecation path. Two consequences compounded: (a) the most-used plugins in the ecosystem are
   permanently stale on most machines; (b) all curation pressure was routed into monorepo PRs,
   producing a 420-PR backlog, a formal theme freeze, and an alias-justification checklist. The
   ecosystem's answer was to route around OMZ entirely — antigen/zinit/zplug/znap/sheldon exist
   mainly to be the missing registry, and they *kept OMZ's file convention while replacing its
   distribution.*

3. **Everything is eager, and `lib/` is not even opt-in.**
   21 lib files load unconditionally on every shell. Every enabled plugin's body is sourced. There
   is no `omz_libs=()`, no plugin-level `defer`, no capability-based skip. The result is a
   fixed ~150–300 ms floor before user code, ~42% of it `compinit` that scales with plugin count.
   In 2012 the constraint was "how much can I get for one line of config." Today it is "how fast is
   my prompt," and the architecture cannot be re-tuned for the new constraint without a manifest
   (see #1).

4. **Aliases as the primary payload, with unmanaged global namespace.**
   The `git` plugin alone ships 150+ two-and-three-letter aliases; a normal setup carries 200+.
   These shadow real binaries silently, are not attributed to a source at collision time, and are
   the single largest cause of "OMZ broke my system" reports. The framework's own `CONTRIBUTING.md`
   now concedes the design has "become an issue for two opposing reasons" and gates new aliases
   behind a five-point test — an admission that the payload format was wrong, made too late to
   change. The `zstyle ... aliases no` opt-out is per-plugin, undocumented in the generated
   `.zshrc`, and all-or-nothing.

5. **`$ZSH` is a git working tree, and `git` is the package manager.**
   Elegant in 2009 and responsible for the updater's existence. But it means: version = a commit
   SHA (`omz version` literally shells out to `git describe`); no atomic upgrade or rollback; no
   integrity verification beyond TLS-to-GitHub; a `git rev-parse HEAD` **fork on every shell
   startup** just to key a cache; broken installs when `$ZSH` is packaged by Homebrew/Nix/apt
   (handled by *disabling* the updater, not by supporting them); and 15 years of accumulated
   compat config (`fsck.zeroPaddedFilemode ignore`, `core.autocrlf false`, `init`+shallow-`fetch`
   instead of `clone`) carried in the installer.

**Bonus (6): whole-file override with no drift detection.**
`$ZSH_CUSTOM` overrides replace a file entirely. A user who copied `lib/git.zsh` in 2019 to change
one function has been running 2019's code ever since, silently, receiving no upstream fixes and no
warning. There is no hook point, no partial override, no `omz diff-custom`, no "your override of X
is 340 commits behind" notice. Whole-file replacement is easy to *implement* and expensive to
*live with*.

---

## 15. What transfers to an "oh-my-musecode" on a compiled Rust CLI agent

The starting position is materially different from zsh's in 2009, and the differences should drive
the design.

**Zsh gave OMZ**: `$fpath`, `autoload`, `compinit`, `source`. Primitives only, no policy. OMZ's
value-add was convention + bundle + updater.

**Muse already gives you** (observed in the shipped binary's embedded strings — `plugins install
<path> [--scope user|project]`, `plugins install <plugin>@<marketplace>`, `marketplace_add /
install / enable / update / disable / uninstall / marketplace_remove`,
`.muse-plugin/plugin.json` with `schemaVersion`/`compat`/`capabilities:{skills, commands, hooks,
mcpServers, reminders}`, scope precedence `user|project|bundled|plugin`, trust classifications
`user-local|project-trusted|curated|marketplace-user-added|foreign-import|native-local`,
`~/.muse/lock.json` + `quarantine/` + `audit.log` + `.skills.lock`, `MUSE_PLUGIN_ID` /
`MUSE_PLUGIN_DATA_DIR`, foreign-manifest import from `.claude-plugin` / `.codex-plugin`):

**a manifest, a capability model, scopes, a lockfile, an audit log, trust tiers, and a marketplace
verb set.** That is precisely the list of things OMZ never had, in the order it needed them.

So the honest framing: **oh-my-musecode is not the loader. The loader already exists and is better
than OMZ's. oh-my-musecode is the *content bundle*, the *convention layer above the manifest*, and
the *curation/updater*.** It occupies OMZ's §13.3/§13.7 position (curated batteries + shipping to
the installed base), not its §3/§4 position (boot sequence + resolution).

### Nine transferable lessons, specific

1. **Your differentiator is the curated bundle, not the runtime.** OMZ won because 359 plugins
   arrived with the clone and enabling one was editing an array. The equivalent is: one install
   that lands N vetted skills/commands/agents already present on disk, with the good ones enabled
   by default, and enabling more is a one-token edit — not a network install. Vendor the bundle;
   make the *default* config already good. Muse's `enabledDefault?` field on skills and commands
   is the hook for exactly this.

2. **Keep a three-line declaration surface even though the manifest is rich.** OMZ's user-facing
   API was `ZSH_THEME=`, `plugins=()`, `source`. Muse's `~/.config/muse/settings.json` is JSON, so
   the equivalent is a single flat array of enabled ids plus one persona/theme scalar — with the
   manifest's richness living in the *package*, never in the user's config. **Rich package
   metadata, poor user config.** The moment users must write capability blocks in `settings.json`,
   you have lost the copy-paste-a-dotfile distribution channel that made OMZ spread.

3. **Ship the `$ZSH_CUSTOM` equivalent and git-ignore it — but do partial override, not whole-file.**
   OMZ's biggest maintenance sin (§14 bonus) is whole-file replacement with no drift detection.
   With a manifest you can do better cheaply: allow a user override to declare
   `overrides: {skill: "<id>", base: "<version-or-hash>"}` and emit a warning when the base has
   moved. That single field turns a silent 5-year fork into a visible upgrade prompt. Keep OMZ's
   actual win — mutable user state lives in a directory the updater can never conflict with.

4. **Copy the two-phase load, applied to context rather than `$fpath`.**
   OMZ's structural insight is *register cheaply, initialize once, execute lazily*. The agent
   analogue: a skill/command must be **discoverable** (id + one-line description in the catalog)
   without being **loaded** (SKILL.md body in the context window). Muse already models this — the
   strings show `skill_catalog_descriptions` and `full_skill_description_ids` as separate settings,
   and a `<skill-catalog>` system-reminder distinct from `<skill id=... source="skill-body">`.
   **Your `$fpath` is the catalog; your `compinit` is catalog assembly; your `source` is body
   injection.** Design every bundled item so its catalog line is cheap and its body is only paid
   for on invocation.

5. **Token budget is your startup time — and OMZ's failure to measure it is the mistake to avoid.**
   OMZ's fatal ergonomic gap is that a 400 ms shell gives the user no attribution: no per-plugin
   timing, no `omz profile`. The equivalent failure mode for a skill framework is a bloated system
   prompt with no per-skill token attribution. **Ship `omm doctor` / `omm profile` on day one**:
   tokens contributed per enabled skill's catalog line, per always-on reminder, per hook, per MCP
   server's tool schemas. Muse's `reminders` capability and `mcpServers` are the two that will
   silently dominate — MCP tool schemas are the `compinit` of agent startup: cost scales with
   count, is invisible, and nobody attributes it correctly.

6. **A default-on payload with unmanaged namespace will become your alias problem.** OMZ's 200+
   aliases are its most-regretted design (§14.4). The agent equivalents are (a) always-on reminders
   / appended system prompts, and (b) slash-command id collisions across plugins. Decide the policy
   *before* the bundle grows: reminders should be opt-in with a hard budget and an attributed
   source line; command ids should be namespaced (`plugin:command`) with the bare name resolving
   only when unambiguous. OMZ's `zstyle ':omz:plugins:git' aliases no` is the right *idea*
   (per-source capability suppression) implemented too late and undiscoverable — make it a
   first-class, listed setting from the start.

7. **Curate with a preview channel, not a review queue.** OMZ's 420-PR backlog and theme freeze are
   what happens when "in the repo" is the only quality signal. But note what OMZ got *right*:
   `omz pr test <N>` plus a `testers needed` label is a distributed preview channel built from a
   git fetch and a GitHub label, with an explicit unreviewed-code warning. Given Muse's existing
   trust tiers (`curated` vs `marketplace-user-added` vs `foreign-import`), the move is to make
   **`curated` a badge applied to externally-hosted packages**, not a merge into your monorepo.
   Curation as a *signal on a pointer*, not custody of the code. That is the single biggest
   structural improvement available over OMZ, and it is the thing that kept OMZ's maintainers
   underwater for a decade.

8. **The updater's bail-out list is the part to copy verbatim.** OMZ's `check_for_upgrade.sh`
   refuses to act when: `$ZSH` isn't writable *or isn't owned by you*, stdout isn't a tty, git is
   absent, or the install isn't a git repo. Plus `mkdir` lock, stale-lock reaper, exit-status-
   preserving trap, prime-number cadence, and `has_typed_input()` to avoid stealing keystrokes.
   Translate each: never mutate a package store you don't own (Homebrew/Nix/enterprise-managed
   installs), never prompt in a non-interactive or piped session, never prompt mid-turn while the
   agent is working, hold a lock so ten concurrent sessions produce one update, and put the
   staleness state in a cheap sourceable/parseable file under `~/.muse/`. **Because a compiled Rust
   binary already has its own release channel** (`manifest.json` with per-arch sha256, versioned
   `1.0.1-R2006.1`), oh-my-musecode's updater must be *content-only* and must explicitly refuse to
   touch the binary — and must record the binary version it was tested against, since the plugin
   manifest already carries `compat` and the validator already emits
   `incompatible-plugin-package`.

9. **Two things OMZ never had that you get for free — use them.**
   (a) **A lockfile.** `~/.muse/lock.json` and `.skills.lock` already exist. Ship
   `omm.lock` semantics from v1: pinned versions + hashes for every bundled and third-party item,
   so a dotfiles repo reproduces an *exact* agent, not "whatever master was." This is the direct
   answer to OMZ's worst practical failure (§14.2a: the world's most-installed plugins are
   permanently stale).
   (b) **A validated schema with a compiled validator.** The binary already reports
   `known_fields` / `unknown_fields` / `unsupported_fields` and `invalid-plugin-package`. Make
   `omm lint` the authoring contract — OMZ's authoring bar was "make a file" and its review bar was
   "a maintainer reads it"; yours can be "the compiled validator passes with zero diagnostics,"
   which scales without maintainer attention. That is how you get OMZ's §13.1 authoring volume
   *without* OMZ's §14.1 consequences.

### The one-sentence version

OMZ won on **zero-friction authoring, a vendored curated bundle, a three-line config, a git-ignored
override directory, and an updater that respected the user** — and lost on **no manifest, no
registry, no laziness, and an unmanaged global namespace.** Muse's runtime already fixes the four
losses; oh-my-musecode's job is to reproduce the five wins on top of it, and to add the two things
OMZ never had time to build: **per-item cost attribution** and **curation as a badge on a pointer
rather than custody of the code.**

---

## Sources

**Primary (repo source, fetched 2026-09-01):**
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/oh-my-zsh.sh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/lib/cli.zsh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/lib/compfix.zsh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/lib/async_prompt.zsh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/tools/install.sh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/tools/check_for_upgrade.sh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/tools/upgrade.sh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/tools/uninstall.sh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/templates/zshrc.zsh-template>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/custom/example.zsh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/CONTRIBUTING.md>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/plugins/kubectl/kubectl.plugin.zsh>
- <https://github.com/ohmyzsh/ohmyzsh/blob/master/plugins/gh/gh.plugin.zsh>

**Wiki:**
- <https://github.com/ohmyzsh/ohmyzsh/wiki/Customization>
- <https://github.com/ohmyzsh/ohmyzsh/wiki/Plugins>
- <https://github.com/ohmyzsh/ohmyzsh/wiki/External-plugins>
- <https://github.com/ohmyzsh/ohmyzsh/wiki/Themes#dont-send-us-your-theme-for-now>

**Criticism / benchmarks:**
- <https://unixy.io/blog/case-against-oh-my-zsh/>
- <https://jonlu.ca/posts/speeding-up-zsh>
- <https://blog.mattclemente.com/2020/06/26/oh-my-zsh-slow-to-load/>
- <https://blog.openreplay.com/zsh-slow-startup-fix/>
- <https://github.com/ohmyzsh/ohmyzsh/issues/8536>
- <https://github.com/ohmyzsh/ohmyzsh/discussions/12642>
- <https://dev.to/tmlr/achieving-30ms-zsh-startup-40n1>

**Muse runtime surface** — extracted from the shipped binary at
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/muse-aarch64-macos`
(strings dump at `.../scratchpad/strings4.txt`), version `1.0.1-R2006.1` per `.../scratchpad/manifest.json`.
