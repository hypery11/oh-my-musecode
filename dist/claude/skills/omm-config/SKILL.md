---
name: "omm-config"
description: "Use for app config, env vars, secrets, or .env files: one precedence (defaults < file < env < flags), typed at startup, no secrets in repo or logs; Do not use for Muse settings (bundled:manage-settings) or omm profiles (omm-self)."
---

# Configuration and secrets

Contract: every value the program reads from outside itself has one name, one
source order, one type, one parse site, and one line in `.env.example`. Done
means startup rejects a missing or malformed value with a message naming the
key and where to set it, no secret sits in the repo or the logs, and the step 5
runs are pasted from `bash`. "It reads the env now" is not done. A setting read
where it is used (`os.environ.get(...)` inside a handler) is the bug this skill
removes; never add another. Over two keys: `write_todos`, one item per key,
sub-steps schema / callers / example / check.

## 0. Map what exists

- Reads: `search` (mode `regex`) for
  `getenv|os\.environ|process\.env|env::var|os\.Getenv|System\.getenv|ENV\[`.
  Record per key: name, `file:line`, default, the type it becomes. Every hit
  outside one config module is a stray read to route in step 2.
- Loader: `search` for `Settings|Config|BaseSettings|figment|envconfig|viper|
  convict|zod` and for `config/`, `settings*.py`, `appsettings*.json`, `.env*`.
  One exists: `read_file` it and extend it, never add a second. None: the
  ecosystem's usual typed one; candidates, bool and duration parsing, `*_FILE`
  and redaction per language in `references/loaders.md` beside this file.
- `read_file` `.gitignore` (`.env`, `.env.*` ignored, `!.env.example` kept?)
  and the README configuration section: the contract to keep true.

## 1. One precedence, written once

`defaults < config file < environment < flags`. Higher wins per key, never per
file. State the order in the loader's doc comment and the README.

- Defaults only for values safe in every environment (port, log level,
  timeouts). No default for anything that differs per deployment or is a
  secret: missing means startup fails. A default pointing at a shared or
  production resource is a bug.
- File: committed, non-secret, structured. Per-environment file chosen by ONE
  closed enum (`APP_ENV=development|test|staging|production`), never by
  hostname, `.exists()` probing, `CI`, or `NODE_ENV`. Code branches on config
  values, not on the environment's name.
- Environment: one uppercase prefix, `__` for nesting (`APP_DB__HOST` ->
  `db.host`). No bare names (`PORT` collides in shared shells). `.env` loads
  only when `APP_ENV` is `development` or unset; never copied into an image.
- Flags: only what an operator varies per invocation; every flag aliases a
  schema key. One name per key: an alias is read only to warn and map.

## 2. Typed and validated at startup

- One schema (struct, dataclass, `BaseSettings`, `zod` object, serde type)
  declares every key: name, type, default or required, one-line doc, secret
  flag. Nothing else declares keys.
- Parse all of it before the first I/O into an immutable value; pass it down.
  Collect every error, then exit non-zero once.
- Parse to real types at the boundary: bool from `1 0 true false yes no` only
  (the string `"false"` is truthy); durations with units (`30s`, `5m`); URLs
  parsed; ports ranged; paths absolutized against a stated root; enums closed.
- Cross-field rules live in the schema: `TLS_CERT` requires `TLS_KEY`;
  `production` forbids `DEBUG=true`.
- Error shape: `config: APP_DB_URL: required. Set env APP_DB_URL or db.url in
  config/<env>.yaml`. Key, problem, where to fix it. Never the value.
- Tests, red first (omm-tdd): one per invalid case, environment injected as a
  map or through a fixture that restores it; never mutate the real process
  environment in a test body.

## 3. Secrets

Secret = anything whose leak forces a rotation: password, token, API key,
private or signing key, connection string with credentials, webhook secret.

- Repo: never. Not in code, fixtures, CI yaml, `Dockerfile` `ENV`/`ARG`, or
  docs. `search` (mode `regex`) for
  `(?i)(api[_-]?key|secret|token|passw(or)?d)\s*[:=]\s*['"][A-Za-z0-9/+_-]{12,}`,
  `-----BEGIN`, `AKIA[0-9A-Z]{16}`, `ghp_[A-Za-z0-9]{20,}`. A real-looking hit:
  `bash` `git log -S'<fragment>' --oneline --all`; ever committed means rotate,
  deleting is not enough, and the report says so. Full audit: omm-security.
- Source, in order of preference: platform secret store injected at runtime;
  `<KEY>_FILE` naming a mounted file; plain env. Dev = untracked `.env`; CI =
  the CI secret store. A tracked plaintext file is never a secret source.
- Never in logs, error messages, `--help` defaults, URLs (access logs), argv
  (`ps`), or a README `export KEY=...`. The schema wraps secret fields
  (`SecretStr`, `Secret<T>`, a redact tag) so `repr`, `Debug`, `toString` and
  structured logs print `***`. Log the effective config once at startup: key,
  source layer, redacted value.
- A secret pasted to you: tell the user to rotate it; do not echo it, file it,
  or `add_memory` it.

## 4. Keep `.env.example` and the doc in sync

- `.env.example`: every schema key, schema order, a comment with type, allowed
  values, default or `required`, and an obviously fake placeholder
  (`APP_API_KEY=<set-me>`). Secrets empty, never a real dev value that works.
- Drift check both directions, in a test over the schema, or at least
  `diff <(grep -oE '^[A-Z_]+' .env.example | sort) <(<schema key dump> | sort)`.
- README config section: the precedence line, the override mechanism, the
  secret sources, and a table `key | type | default | required | secret |
  purpose`, generated or checked from the schema, never typed from memory.
- Add, rename or remove a key: schema, example, README, deploy manifests and
  callers in the same change. After a rename, `search` the old name: zero hits
  outside the changelog.

## 5. Verify, paste

`bash`, each command with `; echo exit=$?`, output verbatim:

1. Required key unset, start the app: error names the key, exit != 0.
2. `APP_PORT=eighty`: error names key, type and source.
3. Same key in file and env with different values, print the effective value:
   env wins. Add the flag: flag wins.
4. Drift check empty; `git check-ignore -v .env` prints a rule;
   `git ls-files '.env*'` prints only the example.
5. `search` for the step 0 regex: hits only in the config module and tests.
6. Startup log shows every secret as `***`.
7. The test suite; paste the summary line.

Report: keys added or changed (name, type, default, required, secret), the
precedence line as written, the seven results, anything not verified and why.

## Judgment calls

- Ad hoc reads everywhere: do not rewrite the loader. Route one read through
  the schema, test, next (omm-refactor). Behaviour identical until the last.
- Dev default for a secret (`SECRET_KEY=dev`): only with the guard "`APP_ENV`
  is `production` and value equals the default: refuse to start".
- Toggle that must change without a restart: a feature flag, not config.
  Config is read once; do not add a reload loop.
- File format: the repo's; none: what the stdlib parses (TOML, JSON). No
  anchors, includes or templating.
- `NODE_ENV`, `RAILS_ENV`, `RUST_LOG` are read by libraries: set them from
  `APP_ENV` in the loader; do not overload them as your selector.
- Changing precedence in a deployed repo: stop and ask which environments
  exist, where production secrets live, what else reads the config file.

## Refuse

- `getenv` at the call site, "just this once"; two loaders or two precedences.
- Committing `.env` "for dev", `git add -f` on an ignored file, or a fixture
  secret that also works against a real service.
- `if env == "production":` in application code; environment by hostname.
- Empty string, null or `changeme` accepted for a required secret.
- Deleting a committed secret without rotating it.
- An unredacted config dump at any log level; removing a validation rule so a
  deploy starts; a `.env.example` edited from memory instead of the schema.
