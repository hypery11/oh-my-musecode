# Sink patterns and secret patterns for omm-security

Use with `search` (mode `regex`, `paths` or `glob` scoped to the target). A hit is a
sink to trace back from, not a finding. Every pattern is a starting point; read the
call.

## Shell execution
- Python: `subprocess\.(run|call|Popen|check_output)\(.*shell\s*=\s*True`, `os\.(system|popen)\(`
- Node: `child_process|\b(exec|execSync)\(`; `spawn\(` with `shell:\s*true`
- Go: `exec\.Command\(\s*"(sh|bash|cmd)"`
- Ruby: `` `.*#\{ ``, `system\(.*#\{`, `Open3`
- Java/Kotlin: `Runtime\.getRuntime\(\)\.exec\(`, `ProcessBuilder\(`
- Rust: `Command::new\(\s*"(sh|bash)"`
- Any: `-c` followed by a concatenated or interpolated string
Fix shape: argv array, no shell; allowlist the command; never interpolate.

## SQL / query languages
- String-built queries: `(execute|query|raw|exec)\(\s*(f"|"[^"]*"\s*\+|'[^']*'\s*\+|`[^`]*\$\{)`
- `\.format\(` or `%\s*\(` on a line containing `SELECT|INSERT|UPDATE|DELETE|WHERE`
- ORM escape hatches: `\.raw\(`, `\.extra\(`, `RawSQL`, `\$queryRawUnsafe`, `sequelize\.query\(`, `knex\.raw\(`
- NoSQL: `\$where`, object built from the request body passed straight to `find\(`
Fix shape: parameters/placeholders; identifiers (table, column, ORDER BY) via allowlist.

## Filesystem paths
- `(open|readFile|readFileSync|createReadStream|sendFile|send_file|File\.open|os\.Open|fs::read)\(` with a non-literal argument
- `path\.join\(|os\.path\.join\(|filepath\.Join\(|Path::new\(` where one segment is external
- `include|require\(` with a variable (local file inclusion)
- Archive extraction (`zipfile|tarfile|unzip|extractall|tar\.Extract`) without per-entry containment (zip-slip)
Fix shape: resolve/canonicalize, then require `startsWith(base + sep)`; or map ids to
filenames and never accept a name. Reject symlinks inside upload dirs.

## Template / HTML / JS
- `\|\s*safe\b`, `mark_safe\(`, `Markup\(`, `raw\(`, `html_safe`, `{{{`, `<%-`
- `dangerouslySetInnerHTML`, `\.innerHTML\s*=`, `insertAdjacentHTML\(`, `document\.write\(`
- `autoescape\s*(=\s*False|off)`, `render_template_string\(`, `Template\(` with a request value (SSTI)
- URL sinks: `href=`, `location\s*=`, `window\.open\(` with a value that can start `javascript:`
Fix shape: escape at output for the context (HTML, attribute, JS, URL); never build a
template from input.

## Deserialization and code loading
- `pickle\.loads?\(`, `yaml\.load\((?!.*SafeLoader)`, `marshal\.loads`, `shelve`
- `unserialize\(` (PHP), `Marshal\.load` (Ruby), `ObjectInputStream`, `XMLDecoder`, `readObject\(`
- `eval\(`, `exec\(`, `new Function\(`, `vm\.runIn`, `importlib\.import_module\(` with a variable
- JSON with type discriminators fed to a polymorphic deserializer (`enableDefaultTyping`, `TypeNameHandling`)
Fix shape: a data-only format with a schema; safe loaders; allowlist the types.

## Outbound requests (SSRF)
- `(fetch|axios|requests\.(get|post)|http\.Get|urllib|HttpClient)\(` with a URL derived from input
- Webhooks, image fetchers, PDF renderers, URL preview, "import from URL"
Fix shape: allowlist hosts, resolve DNS then check the IP (block loopback, link-local
169.254/16, RFC1918, metadata endpoints), disable redirects or re-check after each.

## Crypto and auth
- `MD5|SHA1|sha1\(|md5\(` near `password|token`; `ECB`; a literal IV or key; `Random\(\)|Math\.random|rand\(` for tokens
- `==` or `===` comparing a secret, HMAC, or token (timing); use constant-time compare
- JWT: `verify\(.*algorithms` missing, `alg.*none`, `decode\(` used where `verify\(` is meant
- Password hashing anything other than argon2, bcrypt, scrypt, PBKDF2 with a work factor
- Session not rotated on login, not destroyed on logout/password change; reset token without expiry
- `verify\s*=\s*False`, `rejectUnauthorized:\s*false`, `InsecureSkipVerify:\s*true`
- Trust in `X-Forwarded-For`, `Host`, `Referer` for an authz or link-building decision

## File upload
- Type decided by extension or client `Content-Type` only; no size cap; stored under a
  web-served or executable path; original filename used on disk; image parsed by a
  library without a pixel limit (decompression bomb); SVG served inline (script).

## Dependencies
- Lockfile in scope: `bash` -> `npm audit` / `pip-audit` / `cargo audit` / `govulncheck`
  / `bundle audit`. No tool: `web_search` "<package> <version> CVE".
- Report only advisories whose vulnerable function the target calls with attacker input.

## Secrets
Regex over the diff and over newly added config:
```
(?i)(api[_-]?key|secret|passw(or)?d|token|auth|private_key)\s*[:=]\s*['"][^'"]{8,}
AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{36}|xox[baprs]-[A-Za-z0-9-]{10,}|sk_live_[A-Za-z0-9]{10,}|sk-[A-Za-z0-9]{20,}
-----BEGIN [A-Z ]*PRIVATE KEY-----
```
Also check: `.env`, `*.pem`, `*.p12`, `credentials*`, `id_rsa` added to the tree; secrets
in URLs (`https://user:pass@`), in log lines, in error responses, in `Dockerfile ENV`,
passed as argv (visible in `ps`). A removed secret is still in history:
`git log -p -S '<value>' --all`; the fix is rotation, and it is a finding.

## Encoding traps when writing the repro
- Path segments: `..%2f`, `..%5c`, `%2e%2e/`, double encoding `%252e` / `..%252f`,
  `....//` (survives a single `..` strip), overlong UTF-8, null byte `%00` before the
  extension check, Windows `..\` and `C:\`, trailing `.` or space on Windows,
  case-insensitive filesystems.
- Shell: `$(...)`, backticks, `;`, `|`, `\n`, argument injection via a leading `-`
  (`--output=`, `-oProxyCommand=`) even with an argv array.
- SQL: comment terminators `--`, `/*`, quote in the middle of an identifier.
- Headers: CRLF `%0d%0a` into a header value; Host header used to build links.
- Regex DoS: nested quantifiers `(a+)+`, `(\w+\s?)*` on unbounded input.
