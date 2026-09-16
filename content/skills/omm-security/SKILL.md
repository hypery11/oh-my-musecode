---
name: omm-security
description: "Security review of a change or module (is this safe, auth/crypto/upload/shell-exec code): entry-to-sink trace, STRIDE, OWASP, secrets; report only exploitable findings, each with a repro; Do not use when a general code review is wanted."
---

# Security review

Report-only: never edit, never run a repro outside a local sandbox. A finding
is a traced path from attacker-controlled input to a dangerous sink, with a repro
sketch. A category name is not a finding. Anything without the path goes under
Hardening or is dropped.

## 1. Scope and threat model

Target: a diff (`bash`: `git diff HEAD`, `git diff <base>...HEAD`, or `gh pr diff <n>`)
or a module (`read_file` every file in it). For a diff, `read_file` every function
a hunk touches, whole, before judging any of it.

Before hunting, write: entry points (routes, CLI args, env/config, files read,
queue/webhook payloads, headers, cookies, DB rows written by another principal);
who reaches each (anonymous, user, admin, local user, other service); assets (data,
credentials, money, other tenants, the host); out of scope.

Threat model, one line: the user's if given, else network-facing code =
unauthenticated internet client; CLI or daemon = local unprivileged user; dev
tooling, CI, build scripts = a malicious repository or dependency; multi-tenant = an
authenticated user of another tenant. An authenticated user is still untrusted; the
operator's own config is trusted unless the user says otherwise.

More than 3 files: `write_todos`, one item per entry point; tick each when traced.

## 2. Map entries and sinks, then trace

`search` (mode `regex`, `paths` or `glob` scoped to the target) for sinks: shell
(`exec`, `system`, `spawn`, `shell=True`, backticks); SQL/NoSQL (`execute`/`query`/
`raw` built with `+`, f-string, `${`; `$where`); path (`open`, `readFile`, `sendFile`,
`path.join`, `require` with a variable; archive extraction); template/HTML
(`innerHTML`, `|safe`, `mark_safe`, `{{{`, autoescape off); deserializers (`pickle`,
`yaml.load`, `unserialize`, `ObjectInputStream`, `eval`); outbound fetch of a
variable URL; crypto (`==` on secrets, `jwt.decode`, `verify=False`, md5/sha1,
`random.` for tokens, ECB, static IV). Per-language regexes, secrets patterns,
encoding traps: `references/sinks.md` beside this file (directory from the
`locator:` line of the read_skill result).

For each entry, follow the value forward to every sink it reaches; for each sink,
walk back to its source (forward alone misses sinks, backward alone misses
entries). Where they meet ask: validation before the sink? allowlist, not blocklist?
applied to the decoded, normalised form (URL-decoded, unicode-normalised, `..`
resolved, case-folded, null byte rejected)? bounded length? Any "no" is a
candidate: note `path:line - claim`.

STRIDE, once per entry point, one question each:

| threat | ask | look for |
|---|---|---|
| Spoofing | can the caller be someone else? | token `==`, JWT `alg` unchecked, trust in `X-Forwarded-For`/`Host`, session not rotated |
| Tampering | can data change in flight or at rest? | mass assignment, id/role from the body, unsigned cookie, client-side price, TOCTOU |
| Repudiation | can the actor deny it later? | sensitive action unlogged; log line built from raw input |
| Info disclosure | can data reach the wrong reader? | secrets/PII in logs, errors, URLs; 404 vs 403; timing; debug endpoint |
| Denial of service | can one input exhaust a resource? | unbounded body/list/file, nested-quantifier regex, decompression, no timeout |
| Elevation | can low privilege do a high-privilege act? | per-object (IDOR) and per-action authz on every route; admin flag from input |

OWASP hunt, compressed: authz after authn, per object, per method, CORS origin;
crypto (plaintext at rest or in transit, homemade, non-CSPRNG tokens); injection
into any sink above plus LDAP, XPath, header, log; permissive defaults (debug on,
default creds, CORS `*`, 0777, bind 0.0.0.0); dependencies: lockfile in scope ->
`bash` -> `npm audit` / `pip-audit` / `cargo audit` / `govulncheck`, only advisories
in code the target calls; authn (no lockout on login/reset, token in URL, weak
reset); unpinned CI scripts; SSRF (server fetches a user URL, metadata IP).

Secrets: `search` (mode `regex`) over code, config, CI, fixtures with the sinks.md
patterns. A real-looking hit: `bash` -> `git log -S'<value>' --oneline --all`; ever
committed is a finding even if removed (fix = rotate). Also: secrets logged, in
errors or URLs, passed as argv (`ps`); `.env` and keys not git-ignored.

Large surface: `subagent_spawn` one read-only child per module in the shared
checkout (no `worktree_isolation`), each returning section 5's format; merge and
re-rank yourself. Reject a child's finding that lacks its traced path.

## 3. Judgment calls

- **Exploitable or hardening?** Can an attacker inside the stated threat model put
  chosen bytes on that path to that sink, and what do they get? Yes and something:
  Finding. No, or only with a second bug: Hardening. Always say which.
- **Severity** = what the attacker gets, discounted by what they must already have.
  Critical: unauthenticated RCE, auth bypass, full data read, live secret. High:
  authenticated RCE or injection, cross-tenant read/write, privilege escalation,
  stored XSS, SSRF into the internal network. Medium: needs a precondition (config,
  local access, a victim's click); DoS; reflected XSS. Low: weak config, low-value
  leak, missing defence in depth.
- **Guards elsewhere count only if every path passes them.** `search` for the
  middleware or validator, then check the paths that skip it (internal callers,
  admin routes, background jobs). A client-side check, a comment
  (`// validated upstream`), and a WAF are not guards.
- **Framework defaults are not findings.** Parameterized ORM calls, template
  autoescape, CSRF middleware: flag only where the code opts out.
- **Encoding is part of the trace.** Routers split on `/`, decode `%2e%2e`, strip
  or keep trailing dots; a naive payload failing does not clear the sink. Record the
  repro with the encoding the parser actually sees.
- **Pre-existing bugs are reported**, tagged `pre-existing`, and count in the verdict.
- **Repro: sketch, do not run**, unless a local test server or a unit test with your
  own inputs exists. Never against shared or production systems, never with real
  user data, never leave payloads in tracked files.
- **Reachable, not present.** A vulnerable dependency, a dangerous function, a weak
  cipher: a finding only if attacker input reaches it.
- **Stop and ask** when the threat model flips the verdict (internal-only or
  internet-facing?) or the review needs credentials you lack.

## 4. Not findings

- A sink without its entry; "possible injection" without the trace; input that
  only ever comes from trusted config or the repo itself.
- A category: "no rate limiting" without the endpoint and what brute-forcing yields.
- "Sanitize input" as a fix. Name the sink-specific one: parameterize, argv array,
  escape at output, canonicalize then prefix-check, allowlist.
- Obvious test fixtures as secrets; anything that could be real gets "verify, rotate".
- Style, naming, ordinary bugs (a general review's job); generic advice padding a
  clean review (add CSP, add a WAF). Zero findings is legitimate; say what you traced.

## 5. Report

```
Threat model: <attacker assumed, and the access they start with>
## Findings
1. [Critical] path/file.ext:LINE - <what the attacker gets> (CWE-nnn)
   Path: <entry> -> <sink>. Precondition: <what they must already have, or none>.
   Repro: <request, command, or input sketch, encoded as the parser sees it>.
   Fix: <one line, sink-specific>.
## Hardening                 (not exploitable now; optional)
## Verdict
Ship | Ship after fixing #N | Do not ship - one sentence. Traced: <entries>/<total>.
```

Sort by severity, then path. `LINE` is in the new file. Omit empty sections; never
omit Verdict. No preamble, no praise, no summary of the diff. Do not ship while any
Critical or High stands. Worked example (Express path traversal, encoded repro,
unreported items): `references/example.md`.
