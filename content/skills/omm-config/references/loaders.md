# Config loaders, parsing and redaction, per language

Use the repo's loader if it has one. Otherwise the first column is the usual
typed choice; the rest are what to check when reviewing any loader.

| Language | Typed loader | Bool parse | Duration parse | Secret wrapper | `.env` for dev |
|---|---|---|---|---|---|
| Python | `pydantic-settings` (`BaseSettings`, `env_prefix`, `env_nested_delimiter="__"`, `SecretStr`) | pydantic: `1 0 true false yes no on off`, case-insensitive | none built in; parse `10s`/`5m` with a validator or use `timedelta` seconds | `SecretStr` (`repr` prints `**********`; `.get_secret_value()`) | `python-dotenv` or `env_file=` on the settings class |
| Node / TypeScript | `zod` schema over `process.env` (`z.coerce.number()`, `z.enum`), or `convict`, `envalid` | `envalid.bool`: `true false 1 0 t f yes no`; raw `process.env` is strings only, `Boolean("false") === true` | `ms` package or a `z.string().transform` | none standard: a `redact` list handed to the logger (`pino` `redact: ['db.password']`) | `dotenv` (`dotenv.config()` only when `NODE_ENV !== 'production'`) |
| Rust | `figment` (defaults -> `Toml::file` -> `Env::prefixed("APP_").split("__")`) or `config` crate; `clap` for flags with `env = "APP_PORT"` | `serde` bool from env is `true`/`false` only; document that | `humantime_serde` (`10s`, `2m`) | `secrecy::Secret<T>` (`Debug` prints `Secret([REDACTED])`), `ExposeSecret` | `dotenvy` behind `cfg(debug_assertions)` or an explicit check |
| Go | `envconfig` (`kelseyhightower`), `viper`, or `caarlos0/env` with struct tags `env:"APP_PORT" envDefault:"8080"` | `strconv.ParseBool`: `1 t T TRUE true True 0 f F FALSE false False` | `time.ParseDuration` (`1h30m`) via the `time.Duration` field type | none standard: a type whose `String()` returns `***` | `godotenv` behind `APP_ENV == "development"` |
| Java / Kotlin | Spring `@ConfigurationProperties` (relaxed binding `APP_DB_URL` -> `app.db.url`), or `config` (Typesafe) | Spring: `true false on off yes no 1 0` | Spring `Duration` (`10s`, `PT10S`) | mask in `toString`; Spring Actuator `management.endpoint.env.show-values=NEVER` | `spring.config.import=optional:file:.env[.properties]` |
| .NET | `IConfiguration` + `IOptions<T>` with `services.AddOptions<T>().Bind().ValidateDataAnnotations().ValidateOnStart()` | `bool.Parse`: `true`/`false` only | `TimeSpan.Parse` (`00:00:10`) | none standard: `[JsonIgnore]` on secrets for dumps; user-secrets in dev | `appsettings.Development.json` + `dotnet user-secrets`, not `.env` |
| Ruby | `dry-configurable` or `anyway_config`; Rails `credentials` for secrets | `ActiveModel::Type::Boolean`: `1 t true on y yes` | none built in | Rails `filter_parameters` | `dotenv-rails` (development and test groups only) |

## `*_FILE` convention

For each secret `APP_X`, also accept `APP_X_FILE=/run/secrets/x`: read the
file, strip one trailing newline, and use its contents. Exactly one of the two
may be set; both set is a startup error. This is how container secret mounts
and `systemd LoadCredential` deliver secrets without exposing them in `ps` or
`docker inspect`.

## Startup dump

Print once, at info level, after validation passes: one line per key with
`key=<value> (source: default|file:<path>|env|flag)`. Secret fields print
`key=*** (source: env)`. Never print before validation, never at debug level
unredacted, never on every request.

## Secret patterns for the history check

`bash`, from the repo root, before declaring a value safe:

```
git log --all -S'<8-plus chars of the value>' --oneline | head
git log --all -p -- .env 2>/dev/null | grep -c '^+' # a tracked .env in history
```

Ever committed: rotate, then remove. Removing without rotating is theatre.
Patterns worth a `search` (mode `regex`, `hidden: true`, `no_ignore: true`):
`AKIA[0-9A-Z]{16}`, `-----BEGIN [A-Z ]*PRIVATE KEY-----`, `ghp_[A-Za-z0-9]{36}`,
`sk-[A-Za-z0-9]{20,}`, `xox[baprs]-`, `postgres(ql)?://[^:]+:[^@]+@`,
`password\s*[:=]\s*["'][^"']{6,}`.

## `.gitignore` block

```
.env
.env.*
!.env.example
*.pem
*.key
```

`git check-ignore -v .env .env.example` shows which rule matched each; the
example must NOT be ignored.
