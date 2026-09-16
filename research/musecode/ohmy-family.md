# The oh-my-X family and its rivals, read as architectures

**Purpose.** Reconnaissance for designing an "oh-my-musecode" framework on top of Meta Muse Code
(`muse`), a single self-updating compiled Rust CLI agent whose extension surface is
`.muse-plugin/plugin.json` + Markdown skills, not a shell.

**Method.** Every claim below is taken from primary source — the actual `.sh` / `.fish` / `.zsh` /
`.json` / `.adoc` files, not from blog posts. All fetched 2026-09-01 and cached under
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/eco/src/`
(filenames given per section). Counts were computed from the GitHub trees API, not from READMEs.

---

## 0. The two axes that actually separate these systems

Everything in this family sits somewhere on two independent axes. Almost every architectural
argument in the ecosystem is really an argument about one of them.

**Axis 1 — Where does the code live?**

| Position | Meaning | Examples |
|---|---|---|
| Monorepo | Framework ships plugins/themes in its own git repo; you get them by cloning it | oh-my-zsh, oh-my-bash, prezto |
| Package manager | Framework is a *fetcher*; content lives in third-party repos | oh-my-fish, fisher, zinit, antidote, sheldon |
| Compiled-in | Framework is a binary; "plugins" are a closed enum of compiled features | oh-my-posh, starship |

**Axis 2 — What is the unit of extension?**

| Position | Meaning | Examples |
|---|---|---|
| Executable script | A plugin is code that gets sourced into your process | every shell framework here |
| Declarative data | A plugin/theme is a document validated against a schema | oh-my-posh (themes), sheldon (`plugins.toml`) |

Muse sits at **(compiled-in binary, declarative data)** — the same quadrant as oh-my-posh — but
wants a **package-manager** distribution model like fisher's. Nothing in the family occupies that
exact cell. The design has to be assembled from parts, and the rest of this report is about which
parts.

---

## 1. oh-my-bash — the port that proves the substrate matters

Sources: `oh-my-bash.sh`, `lib/utils.sh`, `lib/omb-util.sh`, `tools/upgrade.sh`,
`tools/check_for_upgrade.sh`, `themes/agnoster/agnoster.theme.sh`.
Repo: <https://github.com/ohmybash/oh-my-bash>

**Inventory (computed from the tree):** 36 plugins, 83 themes, 9 alias sets, 59 completion files.
Compare oh-my-zsh: 359 plugins, 143 theme files. OMB is ~10% of OMZ's plugin surface after a decade.

### Distribution model

Identical to OMZ. A curl-to-bash installer clones the monorepo:

```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/tools/install.sh)"
```

`$OSH` (default `~/.oh-my-bash`) is the clone. `$OSH_CUSTOM` is the user overlay, with an XDG
fallback that is conditioned on *ownership*, not just existence:

```bash
# oh-my-bash.sh
if [[ ! ${OSH_CUSTOM-} ]]; then
  OSH_CUSTOM=$OSH/custom
  [[ -d $OSH_CUSTOM && -O $OSH_CUSTOM ]] ||
    OSH_CUSTOM=${XDG_DATA_HOME:-$HOME/.local/share}/oh-my-bash/custom
fi
```

### Plugin contract

There is no manifest and no metadata. A plugin is *a file at a path with a name*:

```bash
# oh-my-bash.sh — _omb_module_require()
case $type in
lib)        locations=({"$OSH_CUSTOM","$OSH"}/lib/"$name".{bash,sh}) ;;
plugin)     locations=({"$OSH_CUSTOM","$OSH"}/plugins/"$name"/"$name".plugin.{bash,sh}) ;;
alias)      locations=({"$OSH_CUSTOM","$OSH"}/aliases/"$name".aliases.{bash,sh}) ;;
completion) locations=({"$OSH_CUSTOM","$OSH"}/completions/"$name".completion.{bash,sh}) ;;
theme)      locations=({"$OSH_CUSTOM"{,/themes},"$OSH"/themes}/"$name"/"$name".theme.{bash,sh}) ;;
esac
```

Selection is a bash array in `~/.bashrc`, exactly OMZ's `plugins=(...)`:

```bash
plugins=(git bundler osx rake ruby)
aliases=(general)
completions=(git composer ssh)
```

Custom-before-default is the whole override story: `{"$OSH_CUSTOM","$OSH"}` in brace-expansion
order, first hit wins, silently. There is no way to say "extend" rather than "replace", no
dependency declaration, no version, no ordering control beyond array order.

### Theme contract

`OSH_THEME="agnoster"` → source `themes/agnoster/agnoster.theme.sh`. The contract is: the file
must end by registering a function that assigns `PS1`.

```bash
# themes/agnoster/agnoster.theme.sh (tail)
function _omb_theme_PROMPT_COMMAND {
  local RETVAL=$?
  ...
  build_prompt
  PS1=$PR
}
_omb_util_add_prompt_command _omb_theme_PROMPT_COMMAND
```

Random themes via `OMB_THEME_RANDOM_CANDIDATES=(...)` / `OMB_THEME_RANDOM_IGNORED=(...)`.

### Update mechanism

Git pull on the framework clone, with a stamp file:

```bash
# tools/check_for_upgrade.sh
echo "LAST_EPOCH=$(_omb_upgrade_current_epoch)" >| ~/.osh-update
local epoch_expires=${UPDATE_OSH_DAYS:-13}
```

```bash
# tools/upgrade.sh
command git --git-dir="$OSH/.git" --work-tree="$OSH" pull --rebase --stat origin master
```

Failure mode is baked in: `pull --rebase` on a user-modified clone conflicts, so the script has an
explicit `git rebase --abort` recovery path and a "please commit, stash or discard them" message.

### Lockfile / registry

**Neither exists.** Reproducibility is "clone at whatever HEAD is today". Third-party plugins are
installed by the user copying files into `$OSH_CUSTOM/plugins/XYZ/XYZ.plugin.sh` by hand.

### Where cloning OMZ into bash breaks — the important part

OMZ's design leans on four zsh features bash does not have. OMB reimplements each, and each
reimplementation is visibly worse. This is the single most useful case study in the family for
anyone porting a framework shape onto a different substrate.

1. **No `fpath` / `autoload`.** zsh lazily compiles a function the first time it is called; prezto
   does `autoload -Uz "$pfunction"` per file. Bash has no such thing, so OMB must `source` every
   selected plugin eagerly at every shell start. Startup cost is linear in plugins *used*, not
   plugins *called*. There is no turbo mode, no deferral, and no equivalent is possible.
2. **No `precmd`/`preexec` hooks.** zsh has `add-zsh-hook precmd f`. Bash has one string variable,
   `PROMPT_COMMAND` (only an array from bash 5.1). OMB's shim has to string-match itself into the
   variable, with a platform-conditional regex dialect:

   ```bash
   # lib/utils.sh:361
   function _omb_util_add_prompt_command {
     ...
     if [[ $OSTYPE == darwin* ]]; then
       prompt_re='[[:<:]]'$hook'[[:>:]]'      # BSD word boundaries
     else
       prompt_re='\<'$hook'\>'                # GNU word boundaries
     fi
     [[ $PROMPT_COMMAND =~ $prompt_re ]] && return 0
     if ((_omb_bash_version >= 50100)); then ... # array form
   ```
   That is three code paths for one hook registration, and it still cannot express ordering.
3. **No `zstyle`.** prezto has a real hierarchical, pattern-matched config namespace
   (`:prezto:module:git:status:ignore`). Bash has global variables, so OMB's plugin/theme options
   are ad-hoc globals in the flat shell namespace: `OMB_PROMPT_SHOW_PYTHON_VENV`, `AG_NO_CONTEXT`,
   `AG_EMACS_DIR`. Every theme invents its own prefix; nothing is discoverable or validated.
4. **No compsys.** zsh's completion system autoloads `_command` files from `fpath`. Bash's
   `complete -F` requires eager registration, which is why OMB ships **59 hand-written
   `.completion.sh` files** that are mostly thin wrappers around distro `bash-completion` scripts —
   duplicated work with no lazy path.

Plus a floor constraint: OMB supports **Bash 3.2** (macOS's system bash), so it cannot use
associative arrays in its core. The load-dedup set is a space-delimited string with a substring test:

```bash
[[ ' '$_omb_module_loaded' ' == *" $module "* ]] && continue
```

And the prompt hand-off is blunt enough to cause width bugs:

```bash
if [[ $PROMPT ]]; then
  export PS1='\['$PROMPT'\]'
fi
```

**Transfer lesson:** the OMZ *shape* (a monorepo of `plugins/`, `themes/`, `lib/`, `custom/`,
selected by a global array) is not portable. It is a shape carved to fit zsh's primitives. Ported to
a substrate with different primitives, every joint has to be re-cut, and the result is a framework
with a tenth of the content and three times the platform-conditional code.

---

## 2. oh-my-fish — the most sophisticated packaging in the family

Sources: `init.fish`, `bin/install`, `pkg/omf/functions/**`, `docs/en-US/Packages.md`,
`repositories`, and the registry repo `oh-my-fish/packages-main`.
Repos: <https://github.com/oh-my-fish/oh-my-fish>, <https://github.com/oh-my-fish/packages-main>

**Inventory:** 254 packages in the official registry.

### Distribution model

Two roots, cleanly split — the only framework in the family that got this right from the start:

```fish
# bin/install
set -g OMF_PATH_DEFAULT   "$XDG_DATA_HOME/omf"    # or $HOME/.local/share/omf   — machine state
set -g OMF_CONFIG_DEFAULT "$CONFIG_PATH/omf"      # ~/.config/omf               — user state
```

`$OMF_CONFIG` is explicitly designed to be committed to dotfiles. It contains exactly three state
files plus three optional user scripts:

```
~/.config/omf/
├── theme               # one line: the active theme name
├── bundle              # the declared package set  ("package <name>" / "theme <name>" lines)
├── channel             # stable | dev
├── init.fish           # user code, sourced after startup
├── before.init.fish    # user code, sourced before startup
└── key_bindings.fish
```

Install has a **release channel** concept (`--channel=stable|dev`); `stable` checks out the newest
`v*` tag after cloning:

```fish
# bin/install
set -l hash (git rev-list --tags='v*' --max-count=1)
  and set -l tag (git describe --tags $hash)
  and git checkout --quiet tags/$tag
```

### Plugin contract

A package is a directory with fish's conventional autoload layout plus OMF-specific hook files:

```
hello_world/
├── README.md
├── init.fish                  # `init` hook: run once at shell start
├── key_bindings.fish          # `key_bindings` hook
├── functions/hello_world.fish # one function per file (fish autoload requirement)
├── completions/hello_world.fish
├── conf.d/*.fish
├── bundle                     # this package's own dependencies
└── hooks/
    ├── install.fish
    ├── update.fish
    └── uninstall.fish
```

**This is the family's only real per-package install/uninstall lifecycle.** The dispatcher is 13
lines and pins the working directory to the package root:

```fish
# pkg/omf/functions/packages/omf.packages.run_hook.fish
function omf.packages.run_hook -a path hook
  set -l hook_script "$path/hooks/$hook.fish"
  set package (basename $path)
  if test -e "$hook_script"
    pushd $path
    source "$hook_script"
    set -l code $status
    popd
    return $code
  end
end
```

Hooks receive `$package`, `$path`, and (for `init`) `$dependencies`. Uninstall additionally fires
two fish events for backwards compatibility, and honours a legacy root-level `uninstall.fish`:

```fish
# pkg/omf/functions/packages/omf.packages.remove.fish
omf.packages.run_hook $path uninstall
if test -f $path/uninstall.fish
  source $path/uninstall.fish 2> /dev/null
end
emit uninstall_$pkg
emit {$pkg}_uninstall
```

Note also **transitive dependencies**: install runs `omf.bundle.install $install_dir/bundle`, so a
package ships its own bundle file and OMF recursively installs it.

### Theme contract

Themes are packages that are *detected* rather than declared — a beautifully pragmatic hack and
also a latent bug source:

```fish
# pkg/omf/functions/packages/omf.packages.install.fish
if not set -q package_type
  test -f $install_dir/fish_prompt.fish -o -f $install_dir/functions/fish_prompt.fish
    and set package_type theme
    or set package_type plugin
end
if test $package_type = theme
  command mv $install_dir $OMF_PATH/themes/$name
end
```

Activation rewrites fish's autoload path and symlinks the prompt into the user's function dir:

```fish
# pkg/omf/functions/themes/omf.theme.set.fish
autoload -e {$OMF_CONFIG,$OMF_PATH}/themes/$current_theme{,/functions}   # unload old
autoload {$OMF_CONFIG,$OMF_PATH}/themes*/$target_theme{,/functions}      # load new
ln -sf $path $user_functions_path/fish_prompt.fish
echo "$target_theme" > "$OMF_CONFIG/theme"
```

A theme may also ship `conf.d/*.fish` and `key_bindings.fish`, both re-sourced on theme switch —
i.e. themes are full plugins with a distinguished entry point. That is a richer theme contract than
anything else in the family except oh-my-posh.

### Registry / DB repo — the interesting bit

`$OMF_PATH/repositories` (and `$OMF_CONFIG/repositories`) is a list of index repos, one per line,
`<url> <branch>`:

```
https://github.com/oh-my-fish/packages-main master
```

`omf.index.update` clones each shallow into a directory whose name is derived by mangling the URL,
then prunes anything no longer listed:

```fish
# pkg/omf/functions/index/omf.index.update.fish
set repositories (awk '{ dir = $1 "_" $2; gsub(/[ \/:@]/, "_", dir); ... }' $lists)
git clone --quiet --depth 1 --branch $branch $url $path
...
for path in (omf.index.path)/*
  if not contains -- $path $valid_paths
    command rm -rf $path      # prune de-registered index repos
  end
end
```

A registry entry is a flat `key = value` property file named after the package, at
`packages/<name>` in the index repo:

```ini
# packages-main/packages/fzf
type = plugin
repository = https://github.com/jethrokuan/fzf
maintainer = Jethro Kuan <hi@jethrokuan.com>
description = Ef-fish-ient fish keybindings for fzf
```

```ini
# packages-main/packages/bobthefish
type = theme
repository = https://github.com/oh-my-fish/theme-bobthefish
description = A Powerline-style, Git-aware fish theme optimized for awesome.
```

Search is a single embedded `awk` program over `(omf.index.path)/*/packages/*` matching `--name=`,
`--text=`, `--type=` (`omf.index.query.fish`). **There is no service, no API, no database — the
registry is a git repo of 254 four-line text files, cloned shallow and grepped.** Submission is a
pull request. Code stays in the author's own repo; the index holds only a pointer.

Resolution falls through three tiers (`omf.packages.install.fish`): index lookup → bare
`owner/repo` shorthand expanded to `https://github.com/$name_or_url` → treat the argument as a URL.
So the registry is an *optional naming layer*, not a gate.

### Update mechanism and lockfile

`omf update` pulls the framework (channel-aware) and `git pull`s each package; `omf.packages.update`
runs the package's `update` hook afterwards. `omf channel` switches stable/dev.

The `bundle` file is *nearly* a lockfile and falls short in one specific, instructive way. From the
README:

> Every time a package/theme is installed or removed, the `bundle` file is updated. You can also
> edit it manually and run `omf install` afterwards to satisfy the changes. Please note that while
> packages/themes added to the bundle get automatically installed, **a package/theme removed from
> bundle isn't removed from user installation.**

`omf.bundle.install` only ever adds:

```fish
for record in $bundle_contents
  set type (echo $record | cut -s -d' ' -f1 | sed 's/ //g')
  contains $type theme package; or continue
  set name_or_url (echo $record | cut -s -d' ' -f2- | sed 's/ //g')
  if not contains $name $packages
    omf.packages.install $name_or_url; ...
  end
end
```

There is **no version, ref, tag or commit anywhere in the bundle format** — it is `theme foo` /
`package bar`, nothing more. So `bundle` is a *wish list*, not a lock: it is append-only in effect,
carries no pinning, and cannot reproduce a machine. Contrast fisher, below, which fixed exactly this.

---

## 3. fisher — the anti-OMF

Sources: `functions/fisher.fish` (v4.4.8, **251 lines, the entire program**), `README.md`.
Repo: <https://github.com/jorgebucaran/fisher>

### Distribution model

There is no framework to install. You install *one function*:

```console
curl -sL https://raw.githubusercontent.com/jorgebucaran/fisher/main/functions/fisher.fish | source && fisher install jorgebucaran/fisher
```

Fisher then manages itself as one of its own plugins. Plugin files are installed *directly into the
user's fish config dir* — there is no framework namespace at all:

```fish
set --query fisher_path || set --local fisher_path $__fish_config_dir
set --local fish_plugins $__fish_config_dir/fish_plugins
```

### Plugin contract

A plugin is **any git host path or local dir containing `functions/`, `conf.d/`, `completions/`,
`themes/`.** No manifest, no metadata, no registration. Install is: fetch tarball, copy those four
directories, `source` the `.fish` files.

```fish
command mkdir -p $source/{completions,conf.d,themes,functions}
set repo (string split -- \@ $plugin) || set repo[2] HEAD
if set path (string replace --regex -- '^(https://)?gitlab.com/' '' $repo[1])
    set url https://gitlab.com/$path/-/archive/$repo[2]/$name-$repo[2].tar.gz
else
    set url https://api.github.com/repos/$repo[1]/tarball/$repo[2]
end
set http (command curl -q --silent -L -o $resp -w %{http_code} $url)
```

Tarballs, not clones — no `.git` directories on disk, and fetches run **in parallel** (each in a
backgrounded `fish --command`, joined with `wait $pid_list`).

Pinning is a suffix on the identifier, `owner/repo@ref`, where ref is a tag, branch or commit:
`fisher install IlanCosman/tide@v6`. Default is `HEAD`.

**Events replace lifecycle scripts.** Fish's native event system does the work OMF's `hooks/`
directory does, without executing an arbitrary install script:

```fish
# emitted per conf.d file, after copying:
contains -- $plugin $install_plugins && set --local event install || set --local event update
for file in (... $$plugin_files_var ...)
    source $file
    if set --local name (string replace --regex -- '.+conf\.d/([^/]+)\.fish$' '$1' $file)
        emit {$name}_$event
    end
end
```

```fish
# plugin side, in flipper/conf.d/flipper.fish
function _flipper_install   --on-event flipper_install   ... end
function _flipper_update    --on-event flipper_update    ... end
function _flipper_uninstall --on-event flipper_uninstall ... end
```

Note the event name is derived from **the conf.d filename**, not the repo name — so the plugin
declares its own event namespace by naming a file.

### Theme contract

Trivially thin: a theme is a plugin that ships `themes/<name>.theme`, consumed by fish's own
built-in `fish_config` (fish ≥ 3.4). Fisher does not own theming at all — it delegates to the shell.
The one wart is documented honestly: if you move `$fisher_path`, fish still looks for themes in
`$__fish_config_dir/themes`, so you symlink.

### Install receipt — the mechanism OMF lacks

For each plugin, fisher records **the exact list of installed files** in a universal variable:

```fish
set --local plugin_files_var _fisher_(string escape --style=var -- $plugin)_files
set --universal $plugin_files_var (string replace -- $source $fisher_path $files | string replace -- ~ \~)
contains -- $plugin $_fisher_plugins || set --universal --append _fisher_plugins $plugin
```

Uninstall is therefore *exact*, and also erases the runtime symbols the files defined:

```fish
command rm -rf (string replace -- \~ ~ $$plugin_files_var)
functions --erase (string replace --filter --regex -- '.+/functions/([^/]+)\.fish$' '$1' $$plugin_files_var)
for name in (string replace --filter --regex -- '.+/completions/([^/]+)\.fish$' '$1' $$plugin_files_var)
    complete --erase --command $name
end
```

Install refuses to clobber pre-existing files — a hard error, not a silent overwrite:

```
fisher: Cannot install "$plugin": please remove or move conflicting files first:
```

### The lockfile and the reconciliation algorithm

`$__fish_config_dir/fish_plugins` is a newline-separated list of plugin identifiers with optional
`@ref`, written after every mutation with `$HOME` re-abbreviated to `~` for portability:

```
jorgebucaran/fisher
ilancosman/tide@v5
jorgebucaran/nvm.fish
PatrickF1/fzf.fish
```

The crucial design decision: **`fisher update` with no arguments is a three-way reconcile between
the file (desired) and the universal variable (installed)**, so editing the file is a first-class
workflow — add a line to install, delete a line to *uninstall*:

```fish
for plugin in $new_plugins            # $new_plugins came from the file
    contains -- "$plugin" $old_plugins &&
        set --append update_plugins $plugin ||
        set --append install_plugins $plugin
end
for plugin in $old_plugins            # $old_plugins came from $_fisher_plugins
    contains -- "$plugin" $new_plugins || set --append remove_plugins $plugin
end
```

That is precisely what OMF's `bundle` cannot do. Ordering is preserved deliberately: file order
first, then anything installed but unlisted.

### Registry

**None, by design.** The plugin identifier *is* the URL. Discovery is `awesome-fish` lists and
GitHub search. Fisher's ecosystem is larger and healthier than OMF's 254-package registry — evidence
that a curated registry is not the thing that makes an ecosystem.

**Trade-off, stated fairly:** `fish_plugins` pins by *ref*, not by *content hash*. `@v6` is mutable
if the tag moves; `HEAD` is not reproducible at all. Fisher is simpler than OMF but weaker than
antidote on supply chain. It also depends on the GitHub tarball API and surfaces rate limiting
directly: `fisher: GitHub API rate limit exceeded (HTTP 403)`.

---

## 4. oh-my-posh — a compiled binary themed by a JSON schema

Sources: `themes/schema.json` (190 KB), `themes/jandedobbeleer.omp.json`,
`website/docs/configuration/general.mdx`, `website/docs/installation/{customize,upgrade}.mdx`,
`website/docs/contributing/segment.mdx`.
Repo: <https://github.com/JanDeDobbeleer/oh-my-posh> · Docs: <https://ohmyposh.dev>

**Inventory:** 123 bundled `themes/*.omp.json`, 118 segment `type` values in the schema enum,
115 `src/segments/*.go` implementations.

This is the closest existing analogue to "theme a compiled binary with a document", and the most
directly transferable design in the whole report.

### Distribution model

A single Go binary from a package manager (`brew`, `winget`, `scoop`) or a release archive. The
shell integration is **generated by the binary itself**, one line per shell:

```bash
eval "$(oh-my-posh init bash --config ~/jandedobbeleer.omp.json)"
eval "$(oh-my-posh init zsh  --config ~/jandedobbeleer.omp.json)"
oh-my-posh init fish --config ~/jandedobbeleer.omp.json | source
oh-my-posh init pwsh --config ~/jandedobbeleer.omp.json | Invoke-Expression
oh-my-posh init nu   --config ~/jandedobbeleer.omp.json
eval (oh-my-posh init elvish --config ~/jandedobbeleer.omp.json)
execx($(oh-my-posh init xonsh --config ~/jandedobbeleer.omp.json))
load(io.popen('oh-my-posh init cmd --config C:/Users/Posh/jandedobbeleer.omp.json'):read("*a"))()
```

Ten shells, one binary, one config. The per-shell adapter is ~20 lines of generated glue; all
rendering logic is in the binary. **The framework does not live in the user's dotfiles at all.**

`--config` accepts three shapes — local path, **bare bundled-theme name**, or **remote URL**:

```
--config 'C:/Users/Posh/myconfig.omp.json'
--config 'jandedobbeleer'
--config 'https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/jandedobbeleer.omp.json'
```

### Theme contract — the schema

JSON, YAML or TOML, all validated against one published schema, referenced from the document itself:

```json
{
  "$schema": "https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/schema.json",
  "version": 4,
  "final_space": true,
  "blocks": [
    {
      "type": "prompt",
      "alignment": "left",
      "segments": [
        {
          "type": "path",
          "style": "powerline",
          "powerline_symbol": "\uE0B0",
          "foreground": "#ffffff",
          "background": "#61AFEF",
          "options": { "style": "folder" }
        }
      ]
    }
  ]
}
```

Top-level properties (from `schema.json`):

```
final_space  cursor_style  enable_cursor_positioning  shell_integration  pwd  upgrade
patch_pwsh_bleed  console_title_template  terminal_background  streaming  blocks
tooltips  transient_prompt  valid_line  error_line  secondary_prompt  debug_prompt
palette  palettes  cycle  accent_color  iterm_features  terminal_features  var
maps  async  tooltips_action  version  extends
```

Five of these are load-bearing patterns worth stealing outright:

- **`version: 4`** — an integer schema version *inside the document*, so the binary can migrate or
  reject old documents deterministically. Currently 4; the project has shipped three prior formats.
- **`extends: "<path>"`** — theme inheritance. One field, and users stop copy-pasting 200-line themes.
- **`palette` / `palettes`** — named colours, indirected. `palettes.template` resolves a template to
  a palette key at runtime, so light/dark switching is data, not a second theme file.
- **`var`** — arbitrary user variables addressable from any template. The user-extensibility escape
  valve that costs nothing.
- **`upgrade`** — the binary's own update policy lives *in the theme document*:

  ```json
  { "upgrade": { "notice": true, "interval": "168h", "auto": false, "source": "cdn" } }
  ```
  `source` is `cdn` (`https://cdn.ohmyposh.dev/releases/latest/version.txt`) or `github`. Two
  documented guard-rails: *"Auto upgrade will never upgrade major versions"*, and upgrade checks are
  disabled entirely when `async` rendering is on.

### Plugin contract — there isn't one, and that is the point

`segment.type` is a **closed enum of 118 strings**. The extensibility surface is the *config*, not
code. Adding a segment means shipping a new binary; the contributing guide (`contributing/segment.mdx`)
spells out the five-step cost:

1. `src/segments/new_feature.go` implementing `Enabled() bool` and `Template() string`.
2. `src/config/segment_types.go`: `gob.Register(&segments.NewFeature{})`, a
   `NEWFEATURE SegmentType = "new-feature"` constant, and an entry in the `Segments` map:
   `NEWFEATURE: func() SegmentWriter { return &segments.NewFeature{} }`.
3. `website/docs/segments/<category>/<id>.mdx`.
4. `website/sidebars.js`.
5. **`themes/schema.json` in two places** — the `type` enum, and an `allOf` branch.

That last step is the schema pattern to copy. The schema is a **discriminated union**: one shared
segment shape plus 120 conditional branches keyed on `type`, each defining that type's `options`
with `unevaluatedProperties: false`:

```json
{
  "if":   { "properties": { "type": { "const": "http" } } },
  "then": {
    "title": "HTTP segment",
    "description": "HTTP Request is a simple segment to return any json data from any HTTP call.",
    "properties": {
      "options": {
        "properties": {
          "url":    { "type": "string", "title": "URL", "default": "" },
          "method": { "type": "string", "enum": ["GET", "POST"], "default": "GET" }
        },
        "unevaluatedProperties": false
      }
    }
  }
}
```

Common segment properties, all types: `type style foreground foreground_templates background
background_templates template templates_logic max_width min_width options properties interactive
alias include_folders exclude_folders cache placeholder fallback_template`.
`style` ∈ `plain | powerline | diamond | accordion`. Blocks: `type` ∈ `prompt | rprompt`,
`alignment` ∈ `left | right`, plus `newline filler overflow leading_diamond trailing_diamond
segments force restart_cycle index`.

Two more mechanisms worth noting:

- **Templating is the computed layer.** Go `text/template` with sprig, over per-segment data:
  ```json
  "background_templates": [
    "{{ if or (.Working.Changed) (.Staging.Changed) }}#FF9248{{ end }}",
    "{{ if and (gt .Ahead 0) (gt .Behind 0) }}#ff4500{{ end }}"
  ],
  "template": " {{ .UpstreamIcon }}{{ .HEAD }}{{ if .Working.Changed }} \uf044 {{ .Working.String }}{{ end }} "
  ```
  This is how a *declarative* config expresses conditional behaviour without embedding a language.
- **Per-segment caching is declared, not inferred:**
  ```json
  "cache": { "duration": "24h", "strategy": "folder" }   // strategy ∈ folder | session | device
  ```
  Slow segments become cheap by configuration. There is no plugin-author code path to get this wrong.

### Update mechanism, lockfile, registry

Update: `oh-my-posh upgrade`, or `oh-my-posh enable upgrade` for background auto-update, driven by
the `upgrade` block above. **One updater for the whole system**, because the binary *is* the whole
system.

Lockfile: **none, and none needed** — the binary version pins all behaviour, and a theme is a static
document with no transitive dependencies.

Registry: **none**. 123 themes ship inside the release; the "registry" is a directory in the repo
plus a gallery page generated from it. `oh-my-posh config export --config jandedobbeleer --output
~/.mytheme.omp.json` forks a bundled theme into a user file, in any of the three formats.

**Honest assessment of the closed-enum trade-off.** The generic `command` segment that older
versions had is gone from the schema (`"const": "command"` no longer appears); the nearest escape
hatches are `http` (fetch JSON from a URL) and `text` (static). So oh-my-posh has traded away
arbitrary user extension for a guarantee that every prompt is fast, safe and validated. That is the
right call for a prompt. **It is the wrong call for an agent framework**, and the rival that got
this balance right is starship, whose Rust binary keeps a `[custom.*]` TOML section that shells out
to a user command. Any binary-centric design should ship the closed enum *and* one declared,
sandboxed, cached escape hatch.

---

## 5. prezto — modules, `zstyle`, and real load-time laziness

Sources: `init.zsh` (197 lines), `runcoms/zpreztorc`, `modules/prompt/init.zsh`, `README.md`.
Repo: <https://github.com/sorin-ionescu/prezto>

**Inventory:** 40 modules, 16 bundled prompt themes.

### Distribution model

Monorepo clone with **submodules** — external content is vendored by reference, which is a real
(if coarse) pinning mechanism, unlike OMZ/OMB:

```console
git clone --recursive https://github.com/sorin-ionescu/prezto.git "${ZDOTDIR:-$HOME}/.zprezto"
```

`ZPREZTODIR=${0:h}` — resolved from the sourced file's own path, explicitly so that third-party
plugin managers can place prezto anywhere (`zinit`/`antidote` both load prezto modules this way).

### Plugin contract — modules

A module is a directory with a conventional shape, loaded by `pmodload`:

```
modules/<name>/
├── init.zsh          # or <name>.plugin.zsh
├── functions/        # autoloaded, added to $fpath
├── alias.zsh
└── README.md
```

`pmodload` is the best module loader in the family. It does four things nothing else does:

```zsh
# init.zsh
pmodule_dirs=("$ZPREZTODIR/modules" "$ZPREZTODIR/contrib" "$user_pmodule_dirs[@]")

locations=(${pmodule_dirs:+${^pmodule_dirs}/$pmodule(-/FN)})
if (( ${#locations} > 1 )); then
  if ! zstyle -t ':prezto:load' pmodule-allow-overrides 'yes'; then
    print "$0: conflicting module locations: $locations"; continue      # 1. conflict is an ERROR
  fi
elif (( ${#locations} < 1 )); then
  print "$0: no such module: $pmodule"; continue
fi

fpath=(${pmodule_location}/functions(-/FN) $fpath)                       # 2. lazy autoload
for pfunction in ${pmodule_location}/functions/$~pfunction_glob; do
  autoload -Uz "$pfunction"
done

if [[ -s "${pmodule_location}/init.zsh" ]]; then
  source "${pmodule_location}/init.zsh"
elif [[ -s "${pmodule_location}/${pmodule}.plugin.zsh" ]]; then         # 3. OMZ compat
  source "${pmodule_location}/${pmodule}.plugin.zsh"
fi

if (( $? == 0 )); then
  zstyle ":prezto:module:$pmodule" loaded 'yes'                          # 4. TRANSACTIONAL:
else
  fpath[(r)${pmodule_location}/functions]=()                             #    on failure, unwind
  for pfunction in ...; do unfunction "$pfunction"; done                 #    fpath and functions
  zstyle ":prezto:module:$pmodule" loaded 'no'
fi
```

1. **Conflicting module locations are an error by default** (opt-in override via
   `pmodule-allow-overrides`) — the opposite of OMZ/OMB's silent custom-dir shadowing.
2. **Only `init.zsh` is sourced eagerly**; everything in `functions/` is `autoload -Uz`, i.e.
   compiled on first call. This is genuine lazy loading with no deferral machinery.
3. Falls back to OMZ's `<name>.plugin.zsh` filename — deliberate cross-framework compatibility.
4. **Load failure is unwound**: the `fpath` entry is removed and the autoloaded functions are
   `unfunction`ed. Nothing else in the family cleans up after a failed plugin load.

Ordering is explicit and documented as significant:

```zsh
# runcoms/zpreztorc
zstyle ':prezto:load' pmodule \
  'environment' 'terminal' 'editor' 'history' 'directory' 'spectrum' \
  'utility' 'completion' 'history-substring-search' 'prompt'
```

Third-party modules are supported without a package manager, by pointing at another directory:

```zsh
zstyle ':prezto:load' pmodule-dirs $HOME/.zprezto-contrib
```

### Configuration contract — the family's only real namespace

Everything is `zstyle ':prezto:module:<module>[:<sub>]' <key> <value>` — hierarchical,
pattern-matched, queryable, and cleanly separated from the shell's global variable namespace:

```zsh
zstyle ':prezto:*:*' color 'yes'
zstyle ':prezto:module:editor' key-bindings 'emacs'
zstyle ':prezto:module:git:status:ignore' submodules 'all'
zstyle ':prezto:module:prompt' theme 'sorin'
zstyle ':prezto:module:syntax-highlighting' highlighters 'main' 'brackets' 'pattern'
```

Dumb-terminal handling is a config override, not a code branch — `zstyle ':prezto:module:prompt'
theme 'off'` when `$TERM == dumb`. That is what a namespaced config buys you.

### Theme contract

Delegated entirely to zsh's built-in `promptinit`:

```zsh
# modules/prompt/init.zsh
autoload -Uz promptinit && promptinit
zstyle -a ':prezto:module:prompt' theme 'prompt_argv'
if [[ $TERM == (dumb|linux|*bsd*) ]] || (( $#prompt_argv < 1 )); then
  prompt 'off'
else
  prompt "$prompt_argv[@]"
fi
```

A theme is a file `modules/prompt/functions/prompt_<name>_setup` defining `prompt_<name>_setup`
(and conventionally `prompt_<name>_preview`, `prompt_<name>_help`). It uses the *shell's* theme
protocol rather than inventing one — the same choice fisher made with fish themes.

### Update / lockfile / registry

`zprezto-update` is a careful fast-forward-only updater that refuses to guess:

```zsh
git fetch
UPSTREAM=$(git rev-parse '@{u}'); LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse "$UPSTREAM"); BASE=$(git merge-base HEAD "$UPSTREAM")
if   [[ $LOCAL == $REMOTE ]]; then "There are no updates."
elif [[ $LOCAL == $BASE   ]]; then git pull --ff-only && git submodule update --init --recursive
elif [[ $REMOTE == $BASE  ]]; then cannot-fast-forward "Commits in master that aren't in upstream."
else                               cannot-fast-forward "Upstream and local have diverged."
fi
```

It also refuses to update if you are not on `master`. Compare OMB's `pull --rebase` + `rebase --abort`
recovery: prezto's version never enters a state it has to back out of.

Lockfile: git submodule SHAs, implicitly. Registry: none; `belak/prezto-contrib` is the community
overflow, consumed via `pmodule-dirs`.

---

## 6. zinit — maximal load-time optimisation, maximal complexity

Source: `README.md` (1245 lines of it). Repo: <https://github.com/zdharma-continuum/zinit>

### Distribution model

Single `zinit.zsh` in `ZINIT[BIN_DIR]` (default `~/.local/share/zinit/zinit.git`); everything else
is fetched on demand into a hash-configured directory tree:

| Hash field | Purpose |
|---|---|
| `ZINIT[BIN_DIR]` | zinit's own code |
| `ZINIT[HOME_DIR]` | root of all working dirs |
| `ZINIT[PLUGINS_DIR]` | cloned plugins |
| `ZINIT[SNIPPETS_DIR]` | single-file snippets |
| `ZINIT[COMPLETIONS_DIR]` | harvested completions |
| `ZINIT[MAN_DIR]` | plugin-installed manpages (default `$ZPFX/man`) |
| `ZINIT[ZCOMPDUMP_PATH]`, `ZINIT[COMPINIT_OPTS]` | completion-cache control |
| `$ZPFX` | `~/.local/share/zinit/polaris` — `--prefix` target for `make`/`configure` |

Every field is also settable via `zstyle ':zinit:config' home-dir` etc.; documented precedence is
explicit `ZINIT[KEY]` > `zstyle` > built-in default.

### Plugin contract — "ice modifiers"

The contract is not on the plugin; it is on the *call site*. A plugin is anything, and the user
declares how to treat it with single-use "ice" modifiers (ice = melts after one command):

```zsh
zinit ice from"gh-r" as"program" \
      atclone"./starship init zsh > init.zsh; ./starship completions zsh > _starship" \
      atpull"%atclone" src"init.zsh"
zinit light starship/starship
```

Categories, from the README's own tables:

- **Cloning:** `from` (`gh`/`gh-r`/`gl`/`bb`/`nb`/full domain), `ver`, `bpick`, `depth`, `proto`,
  `cloneopts`, `pullopts`, `svn`.
- **File selection:** `pick`, `src`, `multisrc`.
- **Lifecycle commands:** `atinit`, `atclone`, `atpull`, `atload`, `make`, `configure`, `mv`, `cp`,
  `reset`, `nocd`, `run-atpull`, `countdown`.
- **Conditional loading, plugin output, completions, sticky shell emulation**, and a documented
  **order of execution**.

Two things here are architecturally significant and unique in the family:

- **`from"gh-r" as"program"` installs a compiled binary from GitHub Releases** and puts it on
  `$PATH`. zinit is the only shell plugin manager that treats "a release artifact for my platform"
  as a first-class plugin kind — directly relevant to anything distributing a Rust binary.
- **Turbo mode** defers loading off the critical path:
  ```zsh
  zinit ice wait lucid          # wait == wait"0": load right after the first prompt
  zinit ice wait"2"             # ...or 2 seconds later
  ```
  This is the load-time-optimisation answer to prezto's laziness: prezto avoids work with
  `autoload`; zinit *moves* work after first paint.

**Snippets** are a second unit: source a single remote file with no repo at all, with shorthands
that explicitly cannibalise OMZ:

```zsh
zinit snippet OMZL::clipboard.zsh     # ohmyzsh/lib/
zinit snippet OMZP::git               # ohmyzsh/plugins/
zinit snippet OMZT::robbyrussell      # ohmyzsh/themes/
```

That is the family's clearest statement that a monorepo framework can be reduced to an à-la-carte
CDN by a sufficiently determined package manager.

**Annexes** extend zinit itself: `zinit-annex-readurl`, `zinit-annex-bin-gem-node`, etc. — a plugin
manager with a plugin system for its own installer logic. Powerful; also the reason zinit has a
reputation for being unmaintainable configuration.

### Theme / update / lockfile / registry

Themes are just plugins (`*.zsh-theme` is one of `pick`'s defaults). Update: `zinit self-update`,
`zinit update [--parallel [N]]`, `zinit update --all`. Compilation: `zinit compile` (zsh `zcompile`).
**No lockfile** — `ver"..."` per ice is the only pinning, and it is per-call-site, not per-machine.
**No registry.**

---

## 7. antidote — the mature supply-chain answer

Sources: `README.md`, `ARCHITECTURE.md`, `man/antidote-{bundle,snapshot,home}.adoc`, `functions/*`.
Repo: <https://github.com/mattmc3/antidote> (v2.3.0) · Docs: <https://antidote.sh>

Lineage: antigen → antibody (Go) → antidote (pure zsh). Currently the most carefully engineered
plugin manager in the family.

### Distribution model and the static-file trick

```zsh
git clone --depth=1 https://github.com/mattmc3/antidote.git ${ZDOTDIR:-$HOME}/.antidote
```

The performance architecture is worth copying verbatim. **antidote is a compiler**: the plugin file
is the source, and a generated zsh script is the artifact, regenerated only when stale:

```zsh
# .zshrc
zsh_plugins=${ZDOTDIR:-$HOME}/.zsh_plugins
if [[ ! ${zsh_plugins}.zsh -nt ${zsh_plugins}.txt ]]; then
  (
    source /path-to-antidote/antidote.zsh
    antidote bundle <${zsh_plugins}.txt >${zsh_plugins}.zsh
  )
fi
source ${zsh_plugins}.zsh
```

At steady state the interactive shell runs *one* `source` of *one* generated file, and antidote
itself is never loaded. `ARCHITECTURE.md` states the discipline that makes this work:

> `antidote.zsh` — The whole engine. Runs as a **subprocess**, not in the user's shell (<2000 sloc)
> `functions/` — Autoloaded functions that must run **in the parent shell**
> The dividing line: if the code must mutate the user's shell (`source`, `fpath`, `PATH`,
> `autoload`), it belongs in `functions/`. Everything else belongs in `antidote.zsh`.

State crosses that process boundary through env vars only (`ANTIDOTE_ZSTYLES`, `ANTIDOTE_HOME`,
`ANTIDOTE_TMPDIR`, `ANTIDOTE_DYNAMIC`, `ANTIDOTE_USING_CTX`).

### Plugin contract — a plaintext DSL with annotations

`.zsh_plugins.txt`, one bundle per line, `keyword:value` annotations:

```
rupa/z
sindresorhus/pure                                       kind:fpath
romkatv/zsh-bench                                       kind:path
zdharma-continuum/fast-syntax-highlighting              kind:defer
sorin-ionescu/prezto path:modules/utility/functions     kind:autoload
ohmyzsh/ohmyzsh path:plugins/macos                      conditional:is_macos
zsh-users/zsh-autosuggestions                           branch:develop
zsh-users/zsh-syntax-highlighting  pin:4f8a1c2b9e7d3a6f5c0b8e2d7a9f4c1b6e3d8a52
```

Annotations: `kind` (`zsh|fpath|path|clone|defer|autoload`), `branch`, `path`, `conditional`,
`pre`/`post`, `autoload`, `pin`. And two *directives*, which are lines that configure later lines:

```
using:ohmyzsh/ohmyzsh path:plugins        # bare names below resolve under this repo+prefix
git
magic-enter

preset:ohmyzsh/ohmyzsh pin:4f8a1c2b...    # default annotations for every later use of this bundle
ohmyzsh/ohmyzsh path:lib
ohmyzsh/ohmyzsh path:plugins/git
```

Precedence is specified: line annotations > `using:` > `preset:`. A repo is cloned once, so
inconsistent `pin:`/`branch:` across lines is a diagnosed error, not a race.

### Pinning, snapshots, and supply-chain policy — the state of the art

- **`pin:<sha>`** requires a full 40-character SHA (*"a short SHA is an error"*). Pinned bundles are
  skipped by `antidote update` and left shallow.
- **`antidote snapshot`** is a real lockfile: *"A snapshot is a bundle file where every repository
  is annotated with `kind:clone pin:<SHA>`, capturing the exact commit of each cloned bundle."*
  Snapshots are **auto-saved after a successful `antidote update` in static mode**, stored in
  `$XDG_DATA_HOME/antidote/snapshots` with a rolling cap (`zstyle ':antidote:snapshot' max 25`),
  and restorable (`antidote snapshot restore`, with an `fzf` picker). The lockfile format is the
  same format as the source file — one grammar, two roles.
- **`min-age`** — a cool-off window, expressed as a zstyle, so a bad upstream push has time to be
  noticed before it reaches your shell:
  ```zsh
  zstyle ':antidote:bundle:*'       min-age 7
  zstyle ':antidote:bundle:foo/bar' min-age 30
  zstyle ':antidote:bundle:foo/baz' min-age 0
  ```
  And the docs say plainly what it is *not*: *"Age is measured by commit date, and commit dates are
  set by whoever makes the commit... Treat `min-age` as a cushion against a mistake or an unnoticed
  compromise, not as a supply chain guarantee. When you need a guarantee, use `pin:`."* That is the
  right way to document a security control.
- **Shallow by default, deepened in the background**, tunable with
  `zstyle ':antidote:bundle:*' shallow yes`; `path-style full|short|escaped` controls clone dir naming.

Home dir: `$ANTIDOTE_HOME` (`antidote home`). Update: `antidote update` (bundles via subprocess,
plus self-update via `git pull`). Registry: **none** — `owner/repo` or a full URL.

---

## 8. Two rivals outside the "oh-my" naming, both directly relevant

### sheldon — a Rust binary, a TOML config, and a real lockfile

Repo: <https://github.com/rossmacarthur/sheldon> · Docs: <https://sheldon.cli.rs>

The closest structural match to the target that exists today: **a compiled Rust CLI that manages
declaratively-configured plugins and emits shell code.**

- **Config:** `$XDG_CONFIG_HOME/sheldon/plugins.toml`, a table of named plugins. Sources are
  `github`/`gitlab`/`git`/`remote`/`local`/`inline`; refs via `branch`/`tag`/`rev`:
  ```toml
  [plugins.base16]
  github = "chriskempson/base16-shell"

  [plugins.example]
  github = "owner/repo"
  tag = "v0.1.0"
  use = ["*.zsh"]
  apply = ["source", "PATH"]
  profiles = ["work"]

  [plugins.example.hooks]
  pre  = "export TEST=test"
  post = "unset TEST"

  [plugins.oneliner]
  inline = 'example() { echo "Just an example of inline shell code" }'
  ```
- **Lockfile:** `sheldon lock` installs sources and **generates `plugins.lock`** under
  `$XDG_DATA_HOME/sheldon`. `sheldon source` *"will first check if there is an up to date lock file,
  if not, then it will first do the equivalent of the lock command"* — the antidote staleness check,
  generalised. `--update` and `--reinstall` are explicit, separate verbs.
- **Templates — the killer idea.** How a plugin is *rendered into shell code* is itself user-editable
  config, not compiled behaviour:
  ```toml
  [templates]
  source = """
  {{ hooks?.pre | nl }}{% for file in files %}source "{{ file }}"
  {% endfor %}{{ hooks?.post | nl }}"""
  PATH  = 'export PATH="{{ dir }}:$PATH"'
  path  = 'path=( "{{ dir }}" $path )'
  fpath = 'fpath=( "{{ dir }}" $fpath )'
  ```
  Built-ins are `source` and `PATH` (both shells) plus `path`/`fpath` (zsh). A plugin's `apply` list
  chooses which templates run. Template variables: `{{ name }}`, `{{ dir }}`, `{{ files }}`,
  `{{ hooks.[KEY] }}`. So sheldon has a closed enum of *source kinds* and an open, data-defined set
  of *behaviours* — exactly the split oh-my-posh refuses to make.
- **`profiles`** — a plugin can be scoped to named profiles selected with `--profile`; conditional
  configuration without a scripting language.
- CLI: `init`, `lock`, `source`, `add`, `edit`, `remove`. Config-editing commands *only* edit config.

### starship — the escape hatch oh-my-posh removed

A Rust prompt binary configured by `~/.config/starship.toml`, initialised per shell the same way
(`eval "$(starship init zsh)"`). Same closed set of built-in modules as oh-my-posh's segments —
**but it keeps `[custom.<name>]` blocks that shell out to a user command**, with `when`, `shell`,
`format` and caching controls. That single feature is the difference between "extensible by config"
and "extensible only by pull request", and it is the right lesson for an agent framework where the
long tail of integrations is the whole point.

---

## 9. Comparison matrix

### 9.1 Distribution & update

| | Distribution | Framework update | Plugin update | Self-update guard |
|---|---|---|---|---|
| **oh-my-zsh** | clone monorepo → `~/.oh-my-zsh` | `omz update` (git pull) | n/a (in monorepo) | update stamp file, N-day interval |
| **oh-my-bash** | clone monorepo → `$OSH` | `upgrade_oh_my_bash` → `git pull --rebase` | n/a | `~/.osh-update` + `UPDATE_OSH_DAYS` (13) |
| **oh-my-fish** | clone → `$OMF_PATH`, config → `$OMF_CONFIG` | `omf update` (channel-aware: stable=tag, dev=HEAD) | git pull per pkg + `update` hook | channel pins stable to newest `v*` tag |
| **fisher** | one function file, self-managing | `fisher update jorgebucaran/fisher` | `fisher update` (reconciles whole file) | — |
| **oh-my-posh** | binary via pkg manager / release | `oh-my-posh upgrade`, or `upgrade.auto` in config | n/a (compiled in) | never auto-crosses a major version |
| **prezto** | clone `--recursive` → `.zprezto` | `zprezto-update` (ff-only + submodules) | submodule update | refuses non-`master`, refuses non-ff |
| **zinit** | one file + on-demand dirs | `zinit self-update` | `zinit update [--parallel N]` | — |
| **antidote** | clone / brew / AUR / nix | `antidote update` (self-update via git pull) | `antidote update` + auto-snapshot | `min-age`, shallow-then-deepen |
| **sheldon** | Rust binary (`cargo install --locked`) | package manager | `sheldon lock --update` | lockfile staleness check |

### 9.2 Plugin contract

| | Unit | Manifest | Discovery | Lifecycle hooks | Conflict handling |
|---|---|---|---|---|---|
| **oh-my-bash** | `plugins/<n>/<n>.plugin.sh` | none | `plugins=(...)` array | none | custom silently shadows core |
| **oh-my-fish** | dir with `init.fish` + `functions/` + `hooks/` | none (registry file is external) | `bundle` file + registry name | **`install` `update` `uninstall` `init` `key_bindings`**, cwd pinned to pkg root, `$package`/`$path`/`$dependencies` | first match in `{$OMF_CONFIG,$OMF_PATH}` |
| **fisher** | git path/URL with 4 known dirs | none | `fish_plugins` file | fish events `<name>_install/_update/_uninstall`, namespaced by conf.d filename | **hard error** on existing file |
| **oh-my-posh** | n/a — 118 compiled segment types | JSON Schema `if/then` per `type` | `segment.type` enum | none (per-segment `cache` instead) | schema validation |
| **prezto** | `modules/<n>/init.zsh` + `functions/` | none | `zstyle ':prezto:load' pmodule ...` (ordered) | none, but **transactional unwind on failure** | **error unless `pmodule-allow-overrides`** |
| **zinit** | anything + ice modifiers | none (ices at call site) | imperative `zinit load/light/snippet` | `atinit` `atclone` `atpull` `atload` `make` `configure` `mv` `cp` | `plugin already registered` warning |
| **antidote** | git repo + annotations | none | `.zsh_plugins.txt` | `pre:` / `post:` function annotations | inconsistent `pin:`/`branch:` diagnosed |
| **sheldon** | TOML table entry | the TOML entry itself | `[plugins.<name>]` | `hooks.pre` / `hooks.post` | named keys cannot collide |

### 9.3 Theme contract

| | Theme is | Selected by | Inheritance | Validated |
|---|---|---|---|---|
| **oh-my-bash** | `.theme.sh` assigning `PS1` via `PROMPT_COMMAND` | `OSH_THEME` string | no | no |
| **oh-my-fish** | package auto-detected by presence of `fish_prompt.fish`; may add `conf.d/`, `key_bindings.fish`, `functions/` | `$OMF_CONFIG/theme` (one line) | no | no |
| **fisher** | plugin shipping `themes/<n>.theme` for fish's `fish_config` | fish's own builtin | no | by fish |
| **oh-my-posh** | **JSON/YAML/TOML document**: blocks → segments, Go templates, palettes | `--config <path\|name\|URL>` | **`extends`** | **published `$schema`, `version: 4`** |
| **prezto** | `prompt_<n>_setup` function in `modules/prompt/functions/` | `zstyle ':prezto:module:prompt' theme` | no | no |
| **zinit** | just a plugin (`*.zsh-theme` is a default `pick`) | call site | no | no |
| **antidote** | just a bundle, usually `kind:fpath` | line in the txt file | no | no |
| **sheldon** | n/a (not a prompt tool) | — | — | TOML schema |

### 9.4 Pinning, lockfile, registry

| | Manifest file (committed) | Pinning granularity | True lockfile | Uninstall receipt | Registry |
|---|---|---|---|---|---|
| **oh-my-bash** | `~/.bashrc` arrays | none | ✗ | ✗ | ✗ |
| **oh-my-fish** | `$OMF_CONFIG/bundle` | **none** (`package <name>` only) | ✗ (additive only; removals don't uninstall) | hooks only | **✓ `packages-main`, 254 `key = value` files, git-cloned + awk-searched** |
| **fisher** | `$__fish_config_dir/fish_plugins` | `@tag` / `@branch` / `@commit` | partial — full reconcile, but ref not hash | **✓ `_fisher_<plugin>_files` universal var** | ✗ (URLs) |
| **oh-my-posh** | the theme document | binary version | n/a | n/a | ✗ (123 bundled themes) |
| **prezto** | `.zpreztorc` | submodule SHAs | implicit (submodules) | ✗ | ✗ (`prezto-contrib`) |
| **zinit** | `.zshrc` (imperative) | `ver"..."` per ice | ✗ | ✗ | ✗ |
| **antidote** | `.zsh_plugins.txt` | **`pin:<40-char sha>`, `branch:`, `min-age`** | **✓ auto-saved snapshots, rolling history, restorable** | ✗ | ✗ |
| **sheldon** | `plugins.toml` | `branch`/`tag`/`rev` | **✓ `plugins.lock`, staleness-checked** | ✗ | ✗ |

### 9.5 Load-time strategy

| | Strategy | Mechanism |
|---|---|---|
| **oh-my-bash** | eager, everything | `source` per selected module; no lazy path exists in bash |
| **oh-my-zsh** | eager + `fpath` completions | `compinit` |
| **oh-my-fish** | eager `init.fish`, lazy functions | fish autoload + `require --path` |
| **fisher** | fish-native | `conf.d/` at startup, `functions/` autoloaded by fish |
| **prezto** | **lazy** | `autoload -Uz` every `functions/` file; only `init.zsh` sourced |
| **zinit** | **deferred** | turbo `wait"N"` + `lucid`; `zcompile` |
| **antidote** | **precompiled** | `.zsh_plugins.txt` → generated `.zsh_plugins.zsh`, regenerated only when stale; engine runs in a subprocess |
| **sheldon** | **precompiled** | `sheldon source` emits shell code from templates, gated on lockfile freshness |
| **oh-my-posh** | **per-segment cache** | `cache: {duration, strategy: folder\|session\|device}`, plus `async` |

---

## 10. Which model fits a single compiled Rust binary with JSON-configured plugins

**Verdict: a three-way hybrid — oh-my-posh's *theme/config plane*, fisher's *reconciliation model*,
antidote/sheldon's *lockfile plane*. Explicitly reject oh-my-fish's registry-first design and
oh-my-bash/OMZ's monorepo-overlay design.**

### Take from oh-my-posh (the theming/config plane)

The target is a binary, so the config document must carry the full weight that a `.zsh` file carries
elsewhere. Copy the whole apparatus, not just the idea:

1. Published `$schema` URL, referenced from every document, versioned alongside the binary.
2. Integer `version` field **inside** the document for deterministic migration and rejection.
3. `extends` for inheritance so users fork by delta, not by copy.
4. Named `palette` + switchable `palettes` so appearance variants are data.
5. `var` for user-defined values addressable from templates.
6. A **discriminated union**: a shared base shape + `if/then` on a `type` const with
   `unevaluatedProperties: false` per variant. In Rust this is free —
   `#[serde(tag = "type")] enum Segment { … }` plus `schemars` to emit the schema, and the
   `unevaluatedProperties: false` behaviour is `#[serde(deny_unknown_fields)]`. One source of truth
   for the parser, the validator, the editor autocomplete and the docs.
7. Templating (`{{ }}`) as the escape valve that keeps a static document expressive.
8. Declared per-unit caching (`duration` + `strategy`) so slowness is a config problem, not a
   plugin-author problem.

**But do not copy the closed enum.** oh-my-posh can afford "extension = a pull request" because a
prompt has a bounded universe. Ship the closed enum *plus* one declarative, sandboxed, cached
escape hatch — starship's `[custom.*]`, or sheldon's user-defined `[templates]`.

### Take from fisher (the plugin-set plane)

1. **The identifier is the URL.** No registry as a precondition to installing anything.
2. **One committed manifest file** listing desired plugins with optional `@ref`, designed to live in
   dotfiles/version control.
3. **`update` is a three-way reconcile**, not a fetch loop: diff *declared* against *installed* and
   compute install/update/remove. Deleting a line must uninstall. This is the single behaviour that
   separates fisher from OMF and it is the one users actually feel.
4. **Keep a per-plugin file receipt** so uninstall is exact and complete, including runtime
   de-registration of what the files defined.
5. **Refuse to clobber.** A collision is an error naming the conflicting paths, never a silent
   overwrite.

### Take from antidote + sheldon (the reproducibility plane)

1. **Two files, one grammar**: a hand-edited manifest and a generated lock. antidote's snapshots are
   literally bundle files with `pin:<sha>` added — no second format to learn or maintain.
2. **Pin by content hash, not by ref.** Require the full digest; reject abbreviations.
3. **Auto-snapshot after every successful update**, keep a rolling history, make restore a command.
4. **Staleness-gated regeneration**: the resolved artifact is rebuilt only when the manifest is
   newer, and every consumer path checks. This is the antidote `-nt` test and sheldon's `source`→`lock`
   fallthrough. It is what makes a "framework" free at steady state.
5. **`profiles` / `conditional`** for machine- and context-scoped plugin sets, expressed as data.

### Take from prezto (the loader plane)

1. **Namespaced, hierarchical configuration** (`:prezto:module:git:status:ignore`) rather than a flat
   bag of globals — the single biggest quality gap between prezto and OMZ/OMB.
2. **Duplicate provider = error by default**, opt-in override. Not last-writer-wins.
3. **Transactional load**: on a plugin's failure, unwind everything it registered.
4. **Explicit, documented load order** as a list, since order is semantically significant.

### Take from zinit (one idea only)

`from"gh-r" as"program"` — treat *"a signed release artifact for this platform"* as a first-class
plugin kind with checksum verification. A binary-centric framework will need this for plugins that
ship their own executables or MCP servers. Take nothing else; zinit's ice-modifier surface is a
warning about what happens when the call site becomes the contract.

### Explicitly reject

- **oh-my-fish's registry-as-precondition.** 254 packages after a decade, against fisher's
  registry-free ecosystem. A registry is a *later* index layer over identifiers that already work
  without it — exactly how OMF's own resolver treats it (index → `owner/repo` → URL).
- **OMF's arbitrary install-time shell hooks.** `hooks/install.fish` is `source`d with the package's
  cwd. For an agent binary that already has a trust model, running vendor code at install time to
  set up a plugin is the wrong shape. Declare capabilities; don't execute scripts.
- **The monorepo + `custom/` overlay.** It produces silent shadowing, an unpinnable "version",
  `git pull --rebase` conflicts on user-modified clones, and a maintenance burden proportional to
  the number of integrations.
- **Cloning a structure across substrates.** The oh-my-bash lesson: OMZ's layout encodes zsh's
  `fpath`, `precmd`, `zstyle` and compsys. Ported without those, every joint needs a shim, and you
  end up with three code paths to register one hook.

---

## 11. Lessons that transfer to "oh-my-musecode"

Muse's existing surfaces (from the reverse-engineering reports at
`.../scratchpad/re/config-paths.md`, `.../re/artifacts/create-plugin/references/native-plugin-contract.md`):

- Plugin manifest `.muse-plugin/plugin.json`, `schemaVersion: 1`, `compat: {source, manifestDir}`,
  `capabilities: { skills, commands, hooks, mcpServers, reminders }`.
- Plugin/capability ID grammar `^[a-z0-9][a-z0-9._-]{0,79}$`; `loop` and `muse-core` reserved;
  Windows device names rejected; relative UTF-8 `/`-separated paths only; symlinks may not escape root.
- Config root `~/.config/muse/{settings.json,auth.json,trust.json,skills/}`; data root
  `~/.local/share/muse/{sessions,plugins/cache,skills/bundled,memory,model-catalog,crashes}`.
- Existing skills lockfile `~/.config/muse/skills/.muse/lock.json` with `content_sha256`,
  per-file `{relative_path, sha256, bytes}`, `trust`, `scan`, plus an `audit.log` (JSONL) and a
  `quarantine/` directory.
- Unused-but-present lock fields in the binary: `provenance uninstalled removed_files kept_files
  compatibility known_fields unknown_fields unsupported_fields allowed_tools`, and source kinds
  `marketplace-static marketplace-git repository requested_ref resolved_revision manifest_path
  manifest_hash previous_content_sha256`.
- Install scopes `--scope user|project`; source classes `user-local project-trusted curated
  marketplace-user-added foreign-import native-local`; `muse plugins marketplace add|list|update`
  writing `marketplace.json` snapshots; `.muse-claude-sources` / `.muse-codex-sources`.
- Theming today is one string: `tui.theme` in `settings.json` (§5.4, `struct TuiSettings`).
- The binary self-updates from a signed `manifest.json` (per-arch URL + `sha256` + `size`, channel
  `muse`, version `1.0.1-R2006.1`).

### The eleven concrete transfers

**1. Ship oh-my-musecode as a muse *plugin bundle*, not as a dotfile repo you clone.**
Every monorepo framework here (OMZ, OMB, prezto) has two update channels and they skew. Muse already
auto-updates itself from `manifest.json`; adding a `git pull ~/.oh-my-musecode` alongside it
reproduces oh-my-bash's `pull --rebase` / `rebase --abort` failure mode. oh-my-posh's answer — the
framework *is* the binary, content is data — is the correct one here: the framework is a
`.muse-plugin/plugin.json` bundle plus a manifest of other bundles, and the binary's own updater
handles the binary.

**2. Add `compat.minMuseVersion` / `maxMuseVersion` to `plugin.json` and enforce it.**
The manifest already carries `schemaVersion` and `compat`; a self-updating binary makes plugin/host
version skew inevitable. oh-my-posh's rule is the model: *auto-upgrade never crosses a major
version*. A plugin that declares nothing should be treated as "current major only".

**3. `muse.plugins.json` (declared) + `muse.plugins.lock.json` (resolved), one grammar, two roles.**
Follow antidote exactly. The lock is the declared file with resolution filled in — the binary's
unused lock fields (`requested_ref`, `resolved_revision`, `manifest_hash`, `previous_content_sha256`)
are already the right columns.

```jsonc
// ~/.config/muse/muse.plugins.json  — hand-edited, committed to dotfiles
{
  "$schema": "https://.../muse-plugins.schema.json",
  "version": 1,
  "plugins": [
    { "id": "ohmy-core",  "source": { "type": "git", "url": "https://github.com/…/ohmy-musecode" },
      "ref": "v1.4.0" },
    { "id": "team-rules", "source": { "type": "git", "url": "…" },
      "pin": "sha256:…", "profiles": ["work"] },
    { "id": "scratch",    "source": { "type": "local", "path": "~/dev/scratch-plugin" } }
  ]
}
```

```jsonc
// ~/.config/muse/muse.plugins.lock.json — generated; never hand-edited
{
  "version": 1,
  "generatedAt": "2026-09-01T14:31:33Z",
  "museVersion": "1.0.1-R2006.1",
  "plugins": {
    "ohmy-core": {
      "source": { "type": "git", "url": "…", "requested_ref": "v1.4.0",
                  "resolved_revision": "e3d8a52…40hex" },
      "manifest_path": ".muse-plugin/plugin.json",
      "manifest_hash": "sha256:…",
      "content_sha256": "sha256:…",
      "previous_content_sha256": "sha256:…",
      "capabilities": { "skills": ["plan","grill"], "commands": ["/ohmy"], "hooks": [], "mcpServers": [] },
      "files": [ { "relative_path": "skills/plan/SKILL.md", "sha256": "sha256:…", "bytes": 4211 } ],
      "trust": "marketplace-user-added",
      "scan": { "status": "passed", "warnings": [] }
    }
  }
}
```

**4. Make `muse plugins update` a three-way reconcile — this is the highest-value single behaviour.**
Diff declared (`muse.plugins.json`) against installed (lock) and compute install / update / remove,
fisher-style. Deleting a line must uninstall, using the `files[]` receipt already in the lock format
and the `removed_files` / `kept_files` fields already present in the binary. OMF's `bundle` is the
counter-example: additive-only, and the README has to warn users that removals don't take effect.
Ship `--dry-run` printing the three sets; that is what makes an agent framework auditable.

**5. Pin by digest, not by ref; require the full digest.**
`ref: "v1.4.0"` is a request; `resolved_revision` + `content_sha256` is the lock. Follow antidote's
rule that a short SHA is an error. Muse's skills lock already computes `content_sha256` plus per-file
hashes — extend the same machinery to plugin bundles rather than inventing a second scheme.

**6. Auto-snapshot after every successful update, with rolling history and restore.**
`~/.local/share/muse/plugins/snapshots/snapshot-<ts>.json`, capped, restorable. An agent framework
mutates skills, hooks and MCP servers — i.e. the model's own behaviour. "Put it back exactly how it
was on Tuesday" needs to be one command. Pair it with the existing `skills/.muse/audit.log` JSONL,
which is already the right shape (`{"time","action","skill","source","result"}`), extended with
`plugin`, `from_sha`, `to_sha`.

**7. Give theming a real schema — this is the biggest current gap.**
`tui.theme` is one string. Do to it what oh-my-posh did to prompts: a document with
`$schema`, integer `version`, `extends`, `palette`/`palettes`, and a discriminated union of themed
surfaces. In Rust, `serde` + `schemars` emits the schema from the same types the renderer uses, so
the editor autocomplete, the validator and the docs cannot drift. Also copy `terminal_background:
auto|light|dark` and the `color_depth` degradation ladder — muse's `TuiSettings` already has
`color_depth: auto|truecolor|256|16|none` and `terminal_background: auto|light|dark`, so the theme
document should key palettes off those, not duplicate them.

```jsonc
{
  "$schema": "https://…/muse-theme.schema.json",
  "version": 1,
  "extends": "muse-default",
  "palette": { "accent": "#c386f1", "danger": "#ff479c", "muted": "#6b7280" },
  "palettes": {
    "template": "{{ .TerminalBackground }}",
    "list": { "light": { "accent": "#7c3aed" }, "dark": { "accent": "#c386f1" } }
  },
  "surfaces": [
    { "type": "status_line", "segments": [
        { "type": "model",   "template": " {{ .Model }} ", "foreground": "p:accent" },
        { "type": "context", "template": " {{ .TokensUsed }}/{{ .ContextWindow }} ",
          "cache": { "duration": "5s", "strategy": "session" } },
        { "type": "git",     "template": " {{ .Branch }}{{ if .Dirty }}*{{ end }} " }
    ]},
    { "type": "tool_call", "collapsed_template": "…", "expanded_template": "…" }
  ]
}
```

**8. Namespace plugin settings; validate them per-plugin.**
prezto's `zstyle ':prezto:module:git:status:ignore'` is the only clean config namespace in the
family, and OMB's flat `AG_NO_CONTEXT` globals are the counter-example. Put plugin config under
`settings.json → plugins.<plugin-id>.<key>`, and let `plugin.json` declare a JSON Schema for its own
block — oh-my-posh's per-segment `options` with `unevaluatedProperties: false`, applied to plugins.
Unknown keys become a diagnostic naming the plugin, not a silent no-op.

**9. Capability collisions are errors, with a named override.**
Muse already reserves `loop` and `muse-core` at the plugin-ID level. Extend it to capability IDs:
two plugins declaring skill `plan` or command `/ship` must fail loudly, prezto-style
(`conflicting module locations`) with an explicit opt-in override, not resolve by scope-precedence
silently the way `{$OSH_CUSTOM,$OSH}` does. Publish the precedence table (`native-local` >
`project-trusted` > `user-local` > `curated` > `marketplace-user-added` > `foreign-import`) as
documented policy rather than glob order.

**10. Transactional install with rollback, and no vendor code at install time.**
prezto unwinds `fpath` and `unfunction`s on module load failure — the only framework that does.
Muse should do the same for capability registration, and it already has the pieces
(`quarantine/`, `scan.status`, `uninstalled`/`removed_files`/`kept_files`). Crucially: **do not
adopt OMF's `hooks/install.fish` model.** Arbitrary vendor code executed at install time in a tool
that holds credentials (`auth.json`) and workspace trust (`trust.json`) is the wrong trade. Anything
a plugin needs at install time should be *declared* — files to place, settings to patch, MCP servers
to register — and shown as a diff before it is applied.

**11. Precompile the resolved plugin set; gate on staleness.**
antidote's `[[ ! ${zsh_plugins}.zsh -nt ${zsh_plugins}.txt ]]` and sheldon's lockfile freshness check
are the same idea: pay resolution cost once, not per start. Muse already has a content-addressed
cache at `~/.local/share/muse/plugins/cache/<class>/<plugin>/<skill>/<sha256>/`; the missing piece
is a single merged `plugins.resolved.json` (all skills, commands, hooks, MCP servers, with their
paths and hashes) regenerated only when the lock hash changes. For an agent the payoff is bigger
than shell startup: skill *frontmatter* is injected into every prompt, so the resolved artifact
should carry the minimal per-skill trigger metadata and defer full `SKILL.md` bodies until
activation — prezto's `autoload` discipline, applied to context budget instead of function
definitions.

### One-line summary

Build the **config plane like oh-my-posh** (schema-versioned document, `extends`, palettes,
discriminated union, per-unit caching, one escape hatch that oh-my-posh itself lacks), the
**plugin-set plane like fisher** (URLs not a registry, one committed manifest, three-way reconcile,
exact file receipts, hard errors on collision), and the **reproducibility plane like
antidote + sheldon** (digest pins, auto-snapshots, staleness-gated precompiled artifact). Borrow
prezto's namespaced config and transactional loader. Ignore the registry — it is the part of
oh-my-fish that took the most engineering and produced the least ecosystem.

---

## Sources

**Primary source files** (cached locally under
`/private/tmp/claude-501/-Volumes-OWC-Envoy-Ultra-omm/121b85f3-1a9e-4c0e-af22-0c459424aa0f/scratchpad/eco/src/`):

- oh-my-bash — <https://github.com/ohmybash/oh-my-bash>
  · [`oh-my-bash.sh`](https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/oh-my-bash.sh)
  · [`lib/utils.sh`](https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/lib/utils.sh)
  · [`lib/omb-util.sh`](https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/lib/omb-util.sh)
  · [`tools/upgrade.sh`](https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/tools/upgrade.sh)
  · [`tools/check_for_upgrade.sh`](https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/tools/check_for_upgrade.sh)
  · [`themes/agnoster/agnoster.theme.sh`](https://raw.githubusercontent.com/ohmybash/oh-my-bash/master/themes/agnoster/agnoster.theme.sh)
- oh-my-fish — <https://github.com/oh-my-fish/oh-my-fish>
  · [`init.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/init.fish)
  · [`bin/install`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/bin/install)
  · [`pkg/omf/functions/packages/omf.packages.install.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/packages/omf.packages.install.fish)
  · [`…/omf.packages.run_hook.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/packages/omf.packages.run_hook.fish)
  · [`…/omf.packages.remove.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/packages/omf.packages.remove.fish)
  · [`…/index/omf.index.update.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/index/omf.index.update.fish)
  · [`…/index/omf.index.query.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/index/omf.index.query.fish)
  · [`…/bundle/omf.bundle.install.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/bundle/omf.bundle.install.fish)
  · [`…/themes/omf.theme.set.fish`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/pkg/omf/functions/themes/omf.theme.set.fish)
  · [`docs/en-US/Packages.md`](https://github.com/oh-my-fish/oh-my-fish/blob/master/docs/en-US/Packages.md)
  · [`repositories`](https://raw.githubusercontent.com/oh-my-fish/oh-my-fish/master/repositories)
- oh-my-fish registry — <https://github.com/oh-my-fish/packages-main>
  · [`README.md`](https://github.com/oh-my-fish/packages-main/blob/master/README.md)
  · [`packages/fzf`](https://raw.githubusercontent.com/oh-my-fish/packages-main/master/packages/fzf)
  · [`packages/bobthefish`](https://raw.githubusercontent.com/oh-my-fish/packages-main/master/packages/bobthefish)
- fisher — <https://github.com/jorgebucaran/fisher>
  · [`functions/fisher.fish` (v4.4.8, 251 lines)](https://raw.githubusercontent.com/jorgebucaran/fisher/main/functions/fisher.fish)
  · [`README.md`](https://github.com/jorgebucaran/fisher/blob/main/README.md)
- oh-my-posh — <https://github.com/JanDeDobbeleer/oh-my-posh> · <https://ohmyposh.dev>
  · [`themes/schema.json`](https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/schema.json)
  · [`themes/jandedobbeleer.omp.json`](https://raw.githubusercontent.com/JanDeDobbeleer/oh-my-posh/main/themes/jandedobbeleer.omp.json)
  · [configuration/general](https://ohmyposh.dev/docs/configuration/general)
  · [configuration/segment](https://ohmyposh.dev/docs/configuration/segment)
  · [installation/customize](https://ohmyposh.dev/docs/installation/customize)
  · [installation/upgrade](https://ohmyposh.dev/docs/installation/upgrade)
  · [contributing/segment](https://ohmyposh.dev/docs/contributing/segment)
- prezto — <https://github.com/sorin-ionescu/prezto>
  · [`init.zsh`](https://raw.githubusercontent.com/sorin-ionescu/prezto/master/init.zsh)
  · [`runcoms/zpreztorc`](https://raw.githubusercontent.com/sorin-ionescu/prezto/master/runcoms/zpreztorc)
  · [`modules/prompt/init.zsh`](https://raw.githubusercontent.com/sorin-ionescu/prezto/master/modules/prompt/init.zsh)
  · [`belak/prezto-contrib`](https://github.com/belak/prezto-contrib)
- zinit — <https://github.com/zdharma-continuum/zinit>
  · [`README.md`](https://github.com/zdharma-continuum/zinit/blob/main/README.md)
  · [Annexes wiki](https://zdharma-continuum.github.io/zinit/wiki/Annexes/)
- antidote — <https://github.com/mattmc3/antidote> · <https://antidote.sh>
  · [`ARCHITECTURE.md`](https://github.com/mattmc3/antidote/blob/main/ARCHITECTURE.md)
  · [`man/antidote-bundle.adoc`](https://raw.githubusercontent.com/mattmc3/antidote/main/man/antidote-bundle.adoc)
  · [`man/antidote-snapshot.adoc`](https://raw.githubusercontent.com/mattmc3/antidote/main/man/antidote-snapshot.adoc)
  · [`man/antidote-home.adoc`](https://raw.githubusercontent.com/mattmc3/antidote/main/man/antidote-home.adoc)
- sheldon — <https://github.com/rossmacarthur/sheldon> · <https://sheldon.cli.rs>
- starship — <https://github.com/starship/starship> · <https://starship.rs/config/#custom-commands>
- oh-my-zsh (baseline counts) — <https://github.com/ohmyzsh/ohmyzsh>

**Muse-side references** (local reverse-engineering reports):
`…/scratchpad/re/config-paths.md`, `…/scratchpad/re/cli-surface.md`, `…/scratchpad/re/msp-protocol.md`,
`…/scratchpad/re/artifacts/create-plugin/references/native-plugin-contract.md`,
`…/scratchpad/re/skills-assets/muse-core-plugin.json`.
