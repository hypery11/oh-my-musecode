# Worked micro-example for omm-security

Diff adds a download route:

```js
app.get('/files/:name', (req, res) => {
  res.sendFile(path.join(UPLOAD_DIR, req.params.name));
});
```

Threat model: unauthenticated internet client. Entry: `req.params.name`. Sink:
filesystem read via `res.sendFile`.

Trace: `search` literal `basename` and `resolve` with `paths` scoped to the module:
absent. `search` literal `/files` across the app: no middleware on the route.
Naive `../../etc/passwd` fails (the router matches `:name` to one segment and the
client collapses `..` before sending), so encode it: `..%2f..%2fetc%2fpasswd` is one
segment, is URL-decoded into `req.params.name`, and `path.join` collapses the `..`
before `sendFile` runs its own `..` check on the joined path. Precondition: none.

Report:

```
Threat model: unauthenticated internet client.
## Findings
1. [Critical] src/files.js:12 - arbitrary file read as the service user (CWE-22)
   Path: URL segment :name -> path.join -> res.sendFile. Precondition: none.
   Repro: curl 'http://host/files/..%2f..%2fetc%2fpasswd'
   Fix: p = path.resolve(UPLOAD_DIR, name); require p.startsWith(UPLOAD_DIR + sep), else 404.
## Verdict
Do not ship - unauthenticated file read. Traced: 1/1.
```

Deliberately not reported:

- The handler also logs `name` raw. Nothing parses that log, so there is no sink:
  Hardening at most, and only if the user wants a Hardening section.
- `UPLOAD_DIR` comes from the operator's config file. Trusted by default; not an entry.
