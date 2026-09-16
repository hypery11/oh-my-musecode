# Partial staging and index screening

Companion to omm-commit. Load with `read_file` when `git add -p` is not enough
or the reject list in SKILL.md needs its full pattern set.

## Interleaved hunks `git add -p` cannot separate

When two logical changes sit inside one hunk and `s` (split) no longer
divides it, `e` (edit) opens an editor, which is awkward from a tool call.
Use a patch instead:

1. `git diff <path> > /tmp/omm-commit-full.patch` with `bash`, then
   `read_file` it.
2. `write_file` a copy (`/tmp/omm-commit-part.patch`) containing only the
   lines for this partition. Keep every hunk header (`@@ -a,b +c,d @@`); `b`
   is the number of context plus `-` lines, `d` is context plus `+` lines.
   Drop a `+` line by deleting it; drop a `-` line by turning it into a
   context line (leading space). Never drop context lines.
3. `git apply --cached --recount /tmp/omm-commit-part.patch`. `--recount`
   tolerates wrong counts in the hunk header; a hunk that no longer applies
   means a context line was removed.
4. `git diff --cached <path>` must show exactly the partition. `git diff
   <path>` must show the remainder. Both non-empty means the split worked.

If the two changes are truly one edit (one line serves both), stop splitting:
commit both together and say so in the body.

## `git add -p` prompt answers

| Key | Effect |
|---|---|
| `y` / `n` | stage / skip this hunk |
| `s` | split into smaller hunks when a context line separates them |
| `a` / `d` | stage / skip all remaining hunks in this file |
| `q` | quit, keep what is staged so far |
| `?` | help |

Run `git add -p <path>` with `bash` and `tty: true`; it yields a session id.
Send each key plus newline via `bash_input` `chars` (for example `"y\n"`)
with that `session_id`. The prompt text is
`Stage this hunk [y,n,q,a,d,s,e,?]?`. The session ends on `q` or after the
last hunk; `git diff --cached` confirms the result.

## Extended reject patterns

Names (check `git diff --cached --name-only`):

```
.env .env.* *.pem *.key *.p12 *.pfx id_rsa* id_ed25519* *.keystore
credentials* secrets* *.tfvars *.tfstate .npmrc .pypirc .netrc kubeconfig
dist/ build/ out/ target/ node_modules/ vendor/ (unless tracked by policy)
coverage/ .nyc_output/ *.min.js *.min.css *.map __pycache__/ *.pyc
.DS_Store Thumbs.db *.swp *.swo *~ .idea/ .vscode/ (unless tracked)
*.log *.tmp core core.* *.orig *.rej
```

Text (check `git diff --cached | grep -nE ...`):

```
-----BEGIN (RSA|EC|OPENSSH|PGP|DSA) PRIVATE KEY
AKIA[0-9A-Z]{16}            AWS access key id
ghp_[A-Za-z0-9]{36}         GitHub token; also gho_ ghu_ ghs_ ghr_ github_pat_
sk-[A-Za-z0-9_-]{20,}       API secret keys of several vendors
xox[abp]-[0-9A-Za-z-]+      Slack tokens
(password|passwd|secret|token|api[_-]?key)\s*[:=]\s*["'][^"']{6,}
^<{7} |^={7}$|^>{7}         conflict markers
console\.log\(|debugger;|print\(.*DEBUG|dbg!\(|binding\.pry|pdb\.set_trace
```

A hit is a reason to read the line, not an automatic reject: a test fixture
with a fake key is fine when it is obviously fake and the file says so.

## Lockfiles

- Source manifest changed by you (`package.json`, `pyproject.toml`,
  `Cargo.toml`, `go.mod`): the lockfile diff belongs in the same commit.
- Lockfile changed but no manifest changed: an install or tool touched it.
  Unstage it and say so; commit it only if the user wants the refresh.
- Lockfile partially staged: never. Whole file or nothing.
