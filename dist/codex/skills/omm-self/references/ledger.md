# The ledger, reconcile, uninstall and overlay

## `$XDG_CONFIG_HOME/omm/omm.lock.json`

```jsonc
{
  "schema_version": 1,
  "omm_version": "0.1.0",
  "host": { "version": "1.0.1-R2006.1", "sha256": "..." },   // observed at install
  "scope": "user",
  "entries": [                       // sorted by (base, path) - byte-stable reruns
    { "base": "muse-config",         // muse-config | muse-data | omm | workspace
      "path": "skills/omm-self/SKILL.md",   // ALWAYS base-relative, never absolute
      "kind": "skill",               // skill|command|hook|agent|theme|rules|settings-key|trust|plugin
      "sha256": "...",               // what omm WROTE - the ancestor for the next reconcile
      "source_version": "0.1.0",
      "writer": "omm install",
      "mechanism": "copy",           // copy | muse-skills-install | muse-plugins-install | settings-patch | trust-merge
      "class": "exclusive",          // exclusive | shared-key | seeded
      "prior": null }                // settings-key / trust: the value before omm touched it (exact undo)
  ],
  "registrations": [                 // not files, still undone by uninstall
    { "kind": "muse-plugin", "id": "oh-my-musecode", "approved": ["plugin:oh-my-musecode:hook:omm-verify", "..."] },
    { "kind": "muse-marketplace", "name": "omm", "source": "..." }
  ]
}
```

Rules: the ledger records what omm wrote, never a scan of the destination. A corrupt ledger is
renamed `omm.lock.json.bad-<ts>`, treated as absent, warned loudly (doctor D13). A directory containing a
symlink or socket can never hash equal and is therefore always retained. Framework state never goes
into Muse's `settings.json`; `config.json` holds profile, disabled ids and overlay options only.
`install-provenance.json` next to the ledger records how omm itself was installed (brew/curl/cargo).

## Reconcile (`omm update`), per entry

ancestor = `entry.sha256`, theirs = new source sha, mine = on-disk sha.

| ancestor vs theirs | mine vs ancestor | outcome |
|---|---|---|
| equal | any | no-op |
| differ | equal | overwrite (user never touched it) |
| differ | equal to theirs | adopt (user already has the new content) |
| differ | differ | STAGE to `updates/<version>/<path>`, report; on-disk untouched |

Before any update every ledgered file is copied to `snapshots/<ts>/` (rolling, keep 5). Every
install / update / uninstall / approve appends one JSONL line to `audit.log`. A staged conflict is
resolved by hand; a permanent edit belongs in `custom/<kind>/<id>` (never ledgered, never staged).

## Uninstall

Reverse traversal, deepest first. Two-section preview: remove (on-disk sha == ledger sha) and
preserve (user edited it; `--force` removes anyway). Allowed roots are exactly the four bases;
every path is `canonicalize()` + `strip_prefix()` checked; `/`, `$HOME` and any base root itself
are refused. Registrations undone: `muse plugins remove omm --delete-data`,
`muse plugins marketplace remove ohmy`, each settings key restored to its `prior`. The ledger is
removed last. Kept residue is named in the preview. CI asserts install -> uninstall on a clean HOME
is byte-identical.

## Overlay resolution (first hit wins per id)

```
1  $XDG_CONFIG_HOME/omm/custom/<kind>/<id>            user overlay - REPLACE
2  bundled content
+  $XDG_CONFIG_HOME/omm/custom/<kind>/<id>_append.md  APPEND, resolved independently
-  config.json -> "disabled": ["skill:omm-pdf", "hook:omm-verify"]   kind-qualified ids
```

kinds: `skills commands hooks agents themes`. Shadowed items stay listed with `_shadowed: true` in
`omm list --json`. Overlay files are yours, not ledger entries: `omm update` never touches them and `omm uninstall`
leaves them in place (it removes only ledgered paths and refuses every base root itself).
