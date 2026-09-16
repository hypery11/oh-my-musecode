# Experiment — marketplace file precedence and cross-family install

Binary: `.host/bin/muse-bin-1.0.1-R2006.1` (`Muse Code 1.0.1 (1.0.1-R2006.1)`), 2026-09-02.
Every run used a throw-away `HOME`/`XDG_CONFIG_HOME`/`XDG_DATA_HOME`, `MUSE_NO_AUTO_UPDATE=1`,
`MUSE_EXPERIMENTAL_PLUGINS=1`, `NO_COLOR=1`, no provider, no network (git over `file://` only).
Experiment ids (A1, B5, E3.2 …) below are the labels in the repro scripts (§8).

Questions from `docs/PLAN.md` 0.5(b):
(a) does Muse read both catalog files or first-found, in which order;
(b) does a Claude-schema entry pointing at a `.muse-plugin` package install as `native`;
(c) what happens when one catalog lists the same id twice;
(d) what a git marketplace writes vs a local one, and whether `marketplace update` re-clones.

## 0. Headline answers

| Q | Answer | Evidence |
|---|---|---|
| (a) | **First-found-wins, no merge, no fallback.** A directory source is probed in the order **`marketplace.json` → `.agents/plugins/marketplace.json` → `.claude-plugin/marketplace.json`**. The root `marketplace.json` is a *third*, undocumented **native** Muse catalog (§2). The first file that *exists* is the only one parsed; if it is unparseable the add fails and the next file is never tried. | A1, A3, A3b, A4c, R4, R5b, F1–F3 |
| (b) | **Yes.** The package's manifest directory decides the family, never the catalog's schema: Claude catalog → `.muse-plugin` package → `manifest_family: "native"`, provenance `marketplace-user-added`, skill visible as `plugin:omm:omm-plan`. Same from a Codex catalog and from the native root catalog. | B1, B3, R5, F1 |
| (c) | Both entries are kept (`plugin_count: 2`, `skipped: []`, no diagnostic); `plugins list --available` lists both; **`install <id>@<mkt>` takes the first entry in file order.** Same for all three parsers. | C1–C4, R8f |
| (d) | Git source = shallow single-branch clone into `$DATA/plugins/marketplaces/<n>/source`. **Every `marketplace update` (git *and* local-dir) creates a fresh generation `…/<n>/generations/<epoch-ns>/{source,snapshot.json}` and repoints `marketplaces.json`; retention is current + previous.** The installed plugin stays pinned to the path it was installed from; `plugins update <id>` re-reads *that* path (so it never picks up a new generation) and fails with `plugin-source-unavailable` once the generation is rotated out. The only update path is `remove` → `install <id>@<mkt>` → re-approve. `.muse-claude-sources/<ordinal>/<name>` is where remote (`url`) per-plugin entries are exported (no `.git`), only under a git marketplace. | D1, E1.1–E1.8, E2, E3, D4, D6 |

Side finding that blocks R16 as written: a **native plugin hook rejects `commandWindows`** (§7).

## 1. (a) Precedence — what `marketplace add <dir>` reads

Fixture: one marketplace root with packages under `pkgs/`, catalogs written per variant, then
`muse plugins marketplace add mkt <dir> --json` and `muse plugins list --available --json`.

| id | root `marketplace.json` | `.agents/plugins/…` (Codex) | `.claude-plugin/…` (Claude) | result |
|---|---|---|---|---|
| A1 | – | `only-codex` | `only-claude` | rc 0, `plugin_count: 1`, available = **`only-codex` only** |
| A2 | – | `omm` → `.claude-plugin` pkg | `omm` → `.muse-plugin` pkg | install → `claude-compatible` (Codex file won) |
| A2r | – | `omm` → `.muse-plugin` pkg | `omm` → `.claude-plugin` pkg | install → `native` (Codex file won) |
| A3 | – | valid | `{not json` | rc 0, Codex entries; Claude file never parsed |
| A3c | – | valid | valid JSON, `plugins: {}` | rc 0, Codex entries; Claude file never parsed |
| A3b | – | `{not json` | valid | **rc 1** `Codex marketplace JSON is invalid: key must be a string at line 1 column 2`; Claude file NOT used as fallback |
| A4a/A4b | – | one only | one only | rc 0 (baselines) |
| A4c | – | – | – | rc 1 `plugin-store-read-failed`: ``no marketplace catalog found; probed `marketplace.json`, `.agents/plugins/marketplace.json`, `.claude-plugin/marketplace.json` `` |
| R4/R5b | native, `omm` → `plugins/omm` | `omm` → `dist/codex` | `omm` → `dist/claude` | available = root entry only; install → `native` |
| F2 | native, valid | `{not json` | `{not json` | rc 0, `plugin_count: 1` — foreign files never opened |
| F3 | (deleted) | `omm` → `dist/codex` | `omm` → `dist/claude` | install → **`codex-compatible`** |

The probe order in A4c's error text is literal and matches every observation. Git sources
(D1, E1, F1) behave identically — the probe runs inside the clone.

**`local-file` sources bypass the probe.** Passing a single file (`marketplace add m <repo>/.claude-plugin/marketplace.json`)
records `kind: "local-file"` and parses it with the **native** parser regardless of file name or
location (L): the Claude and Codex files fail with ``must declare `/install/transport` ``, the root
native file works. Only directory (`local-dir`) and git sources run the three-file probe.

**Skip vs fail semantics.** The foreign parsers (Codex/Claude) never fail the add for a bad *entry*:
the entry lands in `skipped[]` on the add result and as a `severity: "warning"` in
`snapshot.json → diagnostics`, and rc stays 0 even when `plugin_count` is 0 (P-*, Q-* in §3.3).
Malformed JSON, or an unusable native-catalog entry, fails the add (rc 1,
``marketplace has no usable plugins (<name>: …)`` for the native parser — A4d, R1–R3).

## 2. The native root catalog (`<root>/marketplace.json`) — recovered by probing

Not in `research/musecode/plugins.md`; it is the parser behind the snapshot format (§8.3 there)
and is probed *first*. Errors use JSON pointers. Everything below was measured (R1–R10, R8*, L).

```json
{
  "schemaVersion": 1,
  "source": "local",
  "plugins": [
    {
      "name": "omm",
      "version": "0.1.0",
      "install":      { "transport": "local-path", "source": "plugins/omm" },
      "integrity":    { "digest": "sha256:58c933df32ca7ccdb1b6c0b50d138a80679bb4bb9af996f700c9e36f225cee6a" },
      "availability": { "status": "available" }
    }
  ]
}
```

| field | rule | verbatim error when violated |
|---|---|---|
| `schemaVersion` | must be `1` | `marketplace must declare schemaVersion 1` (R8, value 2) |
| `source` | must be the string `"local"` — `"git"`, an object, `"local-dir"` or absent all fail; **works from a git marketplace too** (R10, F1) | `only local marketplace sources are supported` (R9) |
| `plugins[].name` | required; must equal the package manifest `name` (§3.2) | (`version` is reported missing first when the entry is Claude-shaped — A4d) |
| `plugins[].version` | required; informational (shown by `list --available`; install records the manifest's) | ``marketplace `<file>` must declare `version` `` |
| `install.transport` | only `local-path` | ``marketplace transport `git` is not supported`` (R3d) / ``must declare `/install/transport` `` |
| `install.source` | relative to the catalog dir; `./` ok; **`..` and absolute paths are accepted and install** (R8h, R8x, R8i — no containment check, unlike both foreign parsers) | ``must declare `/install/source` `` |
| `integrity.digest` | required; `sha256:` + 64 hex; **checked at install, not at add**: wrong digest → add rc 0, `install` rc 1 ``plugin `omm` in marketplace `ohmy` failed integrity check`` (R6, R6b, F4) | ``must declare `/integrity/digest` ``; `marketplace digest must start with sha256:` (R6c) |
| `availability.status` | required; `available` \| `blocked` \| `deprecated`; the last two are still listed but install fails ``plugin `omm` in marketplace `ohmy` is blocked`` / `… is deprecated` (R7) | ``must declare `/availability/status` `` |
| unknown keys, `policy.installation: manual|automatic` | ignored / accepted (R8 b, c, c2) | – |
| duplicate `name` | accepted; install takes the first (R8f) | – |

**The digest is Muse's `package_sha256`.** It is content-addressed — identical for the same bytes
under different roots (B1 = B3 = B4 = D1 = `58c933df…`) and after a git round-trip (R10b).
`manifest_sha256` is plain `sha256(plugin.json bytes)` (verified), but the package scheme did not
match 16 candidate constructions (sorted concat, path+content, path+sha, len-prefixed, JSON lists,
git-blob-like), so **do not reimplement it** — obtain it from the binary:
`plugins validate --json` does *not* emit it; a temp local-dir marketplace does
(`muse_digest()` in `expFinal.py`: copy the package next to a one-line Codex catalog in a temp dir,
`marketplace add d <tmp> --json` under a temp `XDG_DATA_HOME`, read
`plugins list --available --json → available[0].digest`). ~60 ms, offline.

## 3. (b) Cross-family install and catalog/manifest consistency

### 3.1 Family is decided by the package, not the catalog

| id | catalog schema | package manifest dir | `installed.manifest_family` (wire `installed.json`) | provenance |
|---|---|---|---|---|
| B1 | Claude | `.muse-plugin` | **`native`** (`"muse"`) | `marketplace-user-added` |
| B3 / A2r | Codex | `.muse-plugin` | **`native`** | `marketplace-user-added` |
| R5 / F1 / R10 | native root (local-dir / git) | `.muse-plugin` | **`native`** | `marketplace-user-added` |
| B2 / A2 / C2 | Claude / Codex | `.claude-plugin` | `claude-compatible` | `marketplace-user-added` |
| F3 | Codex | `.codex-plugin` | `codex-compatible` | `marketplace-user-added` |

B1 detail: `plugins inspect omm --json → plugin.manifest_family == "native"`, `diagnostics: []`,
`skills list --json` contains `plugin:omm:omm-plan`. Note `inspect` has no top-level
`compatibility.summary`/`enabled` (they live in `record`/`plugin`); R11's four predicates read
`plugin.manifest_family` here and `installed.manifest_family` on the install result.

### 3.2 Catalog ↔ manifest consistency

| rule | observed | id |
|---|---|---|
| catalog `name` ≠ manifest `name` | add rc 0; **install rc 1** ``marketplace plugin `ohmy-omm` resolves to a bundle whose manifest id `omm` does not match``; nothing installed under either id | B5, R8e |
| catalog `version` ≠ manifest `version` | no diagnostic anywhere (`snapshot.diagnostics: []`, install `warning: null`); Codex and Claude parsers report the **manifest** version in `list --available`; the native parser reports the **catalog** version; install always records the manifest version. The string ``catalog version `V1` differs from manifest version `V2` `` exists in the binary but did not fire | B4, B6, R8dd |
| catalog entry points at an invalid package | entry skipped with the package's validation diagnostics joined by `; ` (e.g. ``invalid-manifest-schema: plugin manifest must declare string field `version`; …; manifest-family-mismatch: plugin manifest must declare compat.manifestDir; …``) | P-invalid-pkg, Q-invalid-pkg, Q-two-valid-one-broken |
| already installed from another marketplace/path | ``plugin `omm` is already installed from a different source`` (rc 1); reinstall from the *same* source is idempotent (rc 0) | C5, E1.3 |
| `install omm` without `@mkt` | treated as a local path: ``failed to read plugin store `omm`: No such file or directory`` | C5 |

### 3.3 Minimal entry shapes (what a generator must emit)

Codex `.agents/plugins/marketplace.json` (P-*): required per entry = `name` + `source` **object**
`{"source":"local","path":"<rel>"}`; `version`, `description`, top-level `schemaVersion` (even `2`
or absent), unknown keys — all accepted. A string `source` is skipped
(``Codex marketplace `<file>` must declare `source` ``). `path` may start with `./`; `..` →
`plugin capability path must stay inside plugin root`; absolute → `plugin capability path must be
relative`; `{"source":"url",…}` under local-dir → `Codex marketplace url sources require a Git
marketplace source`; `{"source":"github"}` → ``Codex marketplace source `github` is not supported``;
name `Omm Bad` → ``Codex marketplace plugin name `Omm Bad` is invalid``.

Claude `.claude-plugin/marketplace.json` (Q-*): required per entry = `name` + `source` **string**
(relative, with or without `./`); marketplace `name`, `owner`, `metadata`, `$schema`, entry
`version`/`description`/`author`/`category`/`keywords`/`strict` — all accepted and ignored. An
object `source` — **even `{"source":"local","path":…}`** — is treated as remote and skipped under
local-dir (`Claude marketplace remote sources require a Git marketplace source`); `..`/absolute →
same containment messages as Codex.

## 4. (c) Duplicate ids

| id | catalog | order | `plugin_count` / `skipped` / diagnostics | `install omm@mkt` |
|---|---|---|---|---|
| C1 | Claude | native 1.0.0, then claude 2.0.0 | 2 / [] / [] | `native` 1.0.0 |
| C2 | Claude | claude 2.0.0, then native 1.0.0 | 2 / [] / [] | `claude-compatible` 2.0.0 |
| C3 | Codex | native, claude | 2 / [] / [] | `native` 1.0.0 |
| C4 | Codex | claude, native | 2 / [] / [] | `claude-compatible` 2.0.0 |
| R8f | native root | 0.2.0 (`plugins/omm2`), 0.1.0 (`plugins/omm`) | 2 / [] / [] | 0.2.0 from `omm2` |

So a single catalog cannot offer two families under one id in a way another consumer could pick
from — Muse silently takes the first. Two marketplaces offering the same id: both listed
(`[('m1','omm','1.0.0'), ('m2','omm','2.0.0')]`), the second install is refused (C5, §3.2).

## 5. (d) Git marketplaces, generations, and what `update` really does

### 5.1 On disk after `marketplace add g1 file:///…/ohmy.git` (D1, E1.1, G)

```
$DATA/plugins/marketplaces.json
  "g1": { "name":"g1",
          "source": { "kind":"git", "path":"file:///…/ohmy.git",
                      "worktree_path":"plugins/marketplaces/g1/source" },
          "snapshot_path":"plugins/marketplaces/g1/snapshot.json",
          "last_updated_at":"2026-09-01T20:13:32.584938Z" }          # UTC
$DATA/plugins/marketplaces/g1/snapshot.json                            # §2 shape, source:"local"
$DATA/plugins/marketplaces/g1/source/                                  # the clone
   .git/shallow present; rev-list --count HEAD == 1
   .git/config: fetch = +refs/heads/main:refs/remotes/origin/main      # single-branch = remote HEAD
   <every repo file>                                                   # incl. both catalog files
```
`snapshot.json → plugins[].install = {"transport":"local-path","source":"<abs path into the worktree>/plugins/omm"}`.
`install omm@g1` → `source: {"provenance":"marketplace-user-added","path":"$DATA/plugins/marketplaces/g1/source/plugins/omm"}`,
`cache_path: plugins/cache/local/omm/<package_sha256>/package` (runtime reads the cache, never the worktree).
A local-dir marketplace differs only in `source.kind: "local-dir"`, no `worktree_path`, and
`install.source` pointing into the user's directory.

### 5.2 `marketplace update` = new generation, rolling keep-2 (E1.2–E1.7, D4)

Every update — git **or local-dir**, with or without an upstream change — does:
1. fresh shallow clone (git) / fresh scan (local-dir) into
   `plugins/marketplaces/<n>/generations/<epoch-ns>/source` and `…/<epoch-ns>/snapshot.json`
   (local-dir writes only the snapshot);
2. repoints `marketplaces.json → worktree_path`, `snapshot_path`, `last_updated_at`;
3. deletes everything older than the previous generation.

Observed trees (E1): after add: `snapshot.json, source/`; after update #1: `snapshot.json, source/,
generations/G1`; after #2: `generations/G1, generations/G2` (root `source/` and `snapshot.json`
gone); after #3: `generations/G2, generations/G3`. Untracked/edited files in a worktree are not
carried over (E1.7). A failed clone (upstream unreachable, D6) is **atomic**: rc 1
``failed to clone marketplace source `file://…/gone.git`: fatal: … does not appear to be a git repository``,
lockfile byte-identical, previous worktree and snapshot still served.

### 5.3 The installed plugin is pinned to a generation (E1.3–E1.6, E3.2–E3.4, E2)

* `installed.json → source.path` is the absolute path *inside the generation it was installed from*.
* `plugins update omm` re-reads **that** path. Upstream 0.2.0 + one `marketplace update` +
  `plugins update omm` → rc 0, `"updated"`, still **0.1.0** (E3.2). It is a refresh of the old
  source, never a move to the new generation (also E2: `plugins update remote-pkg` stays 0.2.0 while
  the new snapshot says 0.3.0).
* After the generation is rotated out (two updates, or `marketplace remove`):
  `plugins update omm` → rc 1 `plugin-source-unavailable`:
  ``plugin `omm` was installed from marketplace `g1` and its cached source is no longer available; run `muse plugins remove omm`, then `muse plugins install omm@g1` ``
  (after `marketplace remove`: ``plugin `omm` source `<path>` is no longer available; run `muse plugins remove omm`, then reinstall the plugin from an available source``).
  The plugin keeps running from cache meanwhile (`skills list` still shows `plugin:omm:omm-plan`).
* `plugins install omm@g1` while installed → ``plugin `omm` is already installed from a different source`` (E1.3, E3.2).
* No staleness signal: `plugins list --json → warning: null` throughout; the only "update
  available" signal is `plugins list --available --json → digest` ≠ installed `package_sha256`.
* Local-dir marketplaces (D4): `source.path` is the live user directory, so `plugins update omm`
  *does* pick up edits there (0.1.0 → 0.2.0) even without `marketplace update`; the snapshot/digest
  in `list --available` stays stale until `marketplace update`; `marketplace remove` leaves the path
  usable.

**The only working update path** (E3.3): `plugins remove oh-my-musecode` (no `--delete-data`; strips
`settings.json → runtime_capabilities`, deletes the cache dir, skills disappear) → `plugins install
oh-my-musecode@omm` (new generation, new `package_sha256`, every runtime capability back to `review_needed`)
→ `plugins approve plugin:oh-my-musecode:<kind>:<id>` each → `trusted_enabled`. Between remove and install
the plugin is absent, and a failed install (F4: stale digest) **leaves nothing installed**
(`skills list` empty) — hence the pre-flight in §6.4. `$DATA/plugins/data/omm` was never created
(consistent with host-reality "plugin data dir NOT created").

### 5.4 `marketplace remove` (E1.8, D4)

Deletes `plugins/marketplaces/<n>/` entirely (all generations) and the lockfile record; the
installed plugin keeps working from cache; `plugins update` then fails as above. After
`plugins remove omm --delete-data` the leftovers under `$DATA/plugins/` are `cache/local/` (empty),
`installed.json`, `.tmp/`, `.installed.lock`.

### 5.5 `.muse-claude-sources/` and `.muse-codex-sources/` (E2, E2b, E2c)

Only under a **git** marketplace, and only for remote per-plugin entries
(Claude `{"source":"url","url":"<git url>","ref":"<branch|tag|sha>"}`, Codex `{"source":"url",…}`):
* exported (no `.git` inside) to `<worktree>/.muse-claude-sources/<ordinal>/<name>/` resp.
  `.muse-codex-sources/<ordinal>/<name>/`, **ordinal = index of the entry in `plugins[]`** (E2:
  entry 1 → `/1/remote-pkg`); shows as `?? .muse-claude-sources/` in the worktree's `git status`;
* normalised in the snapshot to `transport: local-path` pointing at that directory;
* `ref` may be a branch, a tag or a full commit sha; absent → remote HEAD; unknown →
  entry skipped, `plugin_count` decremented, diagnostic ``failed to clone marketplace source `<url>#nope`: fatal: Remote branch nope not found in upstream origin``;
* **re-cloned on every `marketplace update`** even when the marketplace repo itself is unchanged
  (E2: snapshot moved 0.2.0 → 0.3.0), so an unpinned `ref` floats;
* under a local-dir marketplace such entries are skipped (§3.3); Codex `github` is unsupported;
  Claude `github` has an accepted shape (research §10.4) but is remote, so same local-dir skip.

Not needed for oh-my-musecode (all packages live in the marketplace repo), documented because a
`.muse-*-sources` directory appearing in a worktree is otherwise unexplained.

## 6. Recommendation — repo-root marketplace files for oh-my-musecode

### 6.1 Ship three catalogs; the native one is Muse's

Because Muse reads exactly one file and probes the native root file first, the clean design is
**one file per consumer, each pointing at that consumer's package**, all three at the repo root:

| file | parser that reads it | points at | consumer |
|---|---|---|---|
| `marketplace.json` | Muse (native, probed first) | `plugins/omm` (`.muse-plugin`) | Muse — installs `native` (F1) |
| `.agents/plugins/marketplace.json` | hosts of the Codex schema (Muse never opens it while `marketplace.json` exists — F2) | `dist/codex` (`.codex-plugin`) | users of those hosts |
| `.claude-plugin/marketplace.json` | hosts of the Claude schema (Muse never opens it — F2) | `dist/claude` (`.claude-plugin`) | users of those hosts |

Exact contents (versions and the digest are generator outputs):

`marketplace.json`
```json
{
  "schemaVersion": 1,
  "source": "local",
  "plugins": [
    {
      "name": "omm",
      "version": "<omm version>",
      "install":      { "transport": "local-path", "source": "plugins/omm" },
      "integrity":    { "digest": "<package_sha256 of plugins/omm, obtained from the binary — §2>" },
      "availability": { "status": "available" }
    }
  ]
}
```

`.agents/plugins/marketplace.json`
```json
{
  "schemaVersion": 1,
  "plugins": [
    { "name": "omm", "version": "<omm version>", "description": "<one line>",
      "source": { "source": "local", "path": "dist/codex" } }
  ]
}
```

`.claude-plugin/marketplace.json`
```json
{
  "$schema": "https://json.schemastore.org/claude-code-marketplace.json",
  "name": "omm",
  "owner": { "name": "hypery11" },
  "plugins": [
    { "name": "oh-my-musecode", "version": "<omm version>", "description": "<one line>",
      "source": "./dist/claude" }
  ]
}
```

Why not the two-file variant (Codex file → `plugins/omm`, Claude file → `dist/claude`)? It works for
Muse (B3) but hijacks the Codex-schema catalog to serve a `.muse-plugin` package its own host cannot load, and
the moment Meta ships a root `marketplace.json` convention of its own the Codex file would be shadowed
anyway. The native file costs one digest per release, which the build already has the binary to compute.

Consequences the orchestrator must fold into ARCHITECTURE/PLAN:
1. **`dist/claude` and `dist/codex` must be committed** (ARCHITECTURE §1 marks `dist/` gitignored):
   a marketplace entry's `source` must resolve inside the repo (`..`/absolute are rejected by both
   foreign parsers — §3.3), so the projections have to be in the git tree, drift-gated exactly like
   `plugins/omm`. Alternatively omit the two foreign catalogs until the projections are committed.
2. ARCHITECTURE §1/§5.2 "(Claude schema; Muse reads it)" / "(Codex schema; Muse reads it)" is
   wrong in the presence of the root file and only half-right without it (first-found): reword to
   "Muse reads `marketplace.json`; the other two are for the Claude-schema / Codex-schema hosts".
3. The plugin id `oh-my-musecode` and the marketplace name `omm` give `oh-my-musecode@omm`
   for all three tools (Muse: `plugins install oh-my-musecode@omm`; the Claude-schema host:
   `/plugin install oh-my-musecode@omm` — the Claude file's `name` must therefore be `omm`).
   The foreign hosts' behaviour is *not* verified here.
4. A package holds exactly one manifest dir (research §2.1), so the three entries must point at three
   package trees — they cannot share `plugins/oh-my-musecode`.

### 6.2 Generator (`omm build`) rules

* Emit all three files; `name` in every entry == manifest `name` (`omm`) — a mismatch only fails at
  install time (B5), which is too late.
* Compute the native `integrity.digest` by asking the binary (§2), after the package is final.
  Never hand-compute it; never copy it from a previous release.
* Keep `version` equal to the manifest version in all three (informational, but `list --available`
  shows the catalog value for the native file — R8dd).
* Never emit an object `source` in the Claude file for a local package (skipped as remote — Q-source-obj-local).

### 6.3 Lint / CI gate (extends R11/R13)

In a temp `XDG_DATA_HOME`, against the repo root (a working copy is fine — the digest ignores `.git`, R10b):
```
muse plugins marketplace add ci <repo-root> --json      # expect plugin_count 1, skipped []
muse plugins list --available --json                    # expect one entry `omm`, status available, digest D
muse plugins install omm@ci --json                      # expect installed.manifest_family == "native"
                                                        #        installed.package_sha256 == D, diagnostics []
```
A stale digest fails only the third step (F4) — this gate is the only thing standing between a
content change and a broken `omm install` for every user.
Also lint: `marketplace.json` present at the root (its absence silently degrades installs to
`codex-compatible` — F3; D10 catches it after the fact), no `..`/absolute in any `source`.

### 6.4 `omm install` / `omm update` sequences (Muse side)

Install:
1. `plugins marketplace add omm <git-url|path> --json`; on ``marketplace `omm` is already configured`` run `marketplace update omm` instead.
2. `plugins list --available --json` → assert an `oh-my-musecode` entry with `status: "available"`; keep `digest`.
3. `plugins install oh-my-musecode@omm --json` → assert `installed.manifest_family == "native"` and `installed.package_sha256 == digest`.
4. approve every capability, verify `trusted_enabled` via `inspect` (R14).
5. Ledger registrations: `{"kind":"muse-marketplace","name":"omm","source":…}` and
   `{"kind":"muse-plugin","id":"oh-my-musecode","package_sha256":…,"source_path":…}` (the generation path, for doctor).

Update (R3/R14 must wrap this atomically):
1. `plugins marketplace update omm --json`; rc ≠ 0 → stop, nothing changed (D6).
2. `plugins list --available --json → digest_new`; equal to the installed `package_sha256` → **no-op, do not remove** (E3.4).
3. Pre-flight in a scratch `XDG_DATA_HOME`: `marketplace add probe <worktree_path from marketplaces.json>`
   (a local-dir marketplace over the current generation) → `install omm@probe` → require `native` and
   `package_sha256 == digest_new`. This catches a stale digest (F4) and a family regression (F3) *before* anything is removed.
4. `plugins remove oh-my-musecode` → `plugins install oh-my-musecode@omm` → approve each → verify. **Never `plugins update oh-my-musecode`**:
   it refreshes the pinned generation (E3.2) and fails with `plugin-source-unavailable` after two rotations (E1.3).
5. Record the new `package_sha256` and generation path.

Uninstall: `plugins remove omm --delete-data` then `plugins marketplace remove ohmy` (order does not
matter for Muse; removing the marketplace first only changes the error text of a later `plugins update`).

### 6.5 Doctor additions

* D10 stays (`manifest_family == "native"`), now with a known cause: root `marketplace.json` missing → Codex file served.
* New: installed `source.path` missing on disk (generation rotated out) → info "next `omm update` will reinstall".
* New: `list --available` digest ≠ installed `package_sha256` → "update available" (Muse itself never says so).
* New: `marketplace list --json` lacks `ohmy` → registration drift (plugin still runs from cache).

## 7. Side finding — native plugin hooks reject `commandWindows`

```
$ muse plugins validate <pkg with capabilities.hooks[0].commandWindows> --json
rc=1 {"error":{"code":"unsupported-field","message":"hook capability field `commandWindows` is not supported", … "valid": false …}}
```
Same for `command_windows`; without either the package validates (`valid: true, diagnostics: []`).
The `hooks` family is CLOSED (host-reality "per-family strictness"), and its field list
(`research/musecode/plugins.md` §3.5: `id event command timeoutMs statusMessage async
compatibilityName outputCapabilities`) has no Windows form. `commandWindows` exists only in the
`hooks.json` document tiers (`research/musecode/hooks.md` line 196, user/project hooks). **R16 /
ARCHITECTURE §5.2 "every hook carries `command` AND `commandWindows`" cannot be satisfied inside
`plugins/omm`** — either ship the Windows twin through `settings.json → hooks` / `.muse/hooks.json`
(user/project tiers, ledgered), or accept POSIX-only plugin hooks and document that they are dispatched
into PowerShell on Windows. Not in this experiment's scope to decide; flagged because E3's first
attempt at a hook package failed on it.

## 8. Reproduction

Scripts (scratchpad during the run; every fixture is rebuilt by them, nothing is hand-edited):
`m.sh` (env wrapper), `mk.py` (fixture builder), `expA.py` (A*), `expRBC.py` (R1–R4, B*, C*),
`expR5.py`/`expR11.py`/`expR12.py` (R5–R10, R8*, P*, Q*), `expD2.py` (E1*, E2*), `expE2.py`
(E2*, E2b, E2c), `expE3.py` (E3*), `expFinal.py` (F1–F5), plus the D4/D6/R8f/L/G one-offs.
The two files everything depends on:

```sh
# m.sh — run Muse against a sandbox: m.sh <sandbox-name> <muse argv...>
SB=<scratch dir>; M="$OMM_MUSE_BIN"; name="$1"; shift; root="$SB/sb/$name"
mkdir -p "$root/home" "$root/xdg-config" "$root/xdg-data"
exec env -i PATH=/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin \
  HOME="$root/home" XDG_CONFIG_HOME="$root/xdg-config" XDG_DATA_HOME="$root/xdg-data" \
  MUSE_NO_AUTO_UPDATE=1 MUSE_EXPERIMENTAL_PLUGINS=1 NO_COLOR=1 "$M" "$@"
```

```python
# mk.py — fixtures
def skill(root, sid, desc):                       # skills/<sid>/SKILL.md with name/description frontmatter
def pkg_native(root, name="omm", version="0.1.0") # .muse-plugin/plugin.json: schemaVersion 1, name, version,
                                                  #   description, compat{source:"native",manifestDir:".muse-plugin"},
                                                  #   capabilities.skills[{id:"omm-plan",path,enabledDefault:true}]
def pkg_claude(root, name="omm", version="0.1.0") # .claude-plugin/plugin.json: name, version, description, skills:["./skills/omm-plan"]
def claude_mkt(root, entries, name="ohmy")        # .claude-plugin/marketplace.json: {name, owner:{name}, plugins:entries}
def codex_mkt(root, entries)                      # .agents/plugins/marketplace.json: {schemaVersion:1, plugins:entries}
```

Minimal manual repro of each headline (`$M` = `m.sh <sandbox>`):

```sh
# (a) probe order — A1: both foreign files, disjoint ids → only the Codex id is listed
$M plugins marketplace add mkt <dir with .agents/plugins + .claude-plugin catalogs> --json   # plugin_count 1
$M plugins list --available --json                                                           # only-codex
$M plugins marketplace add mkt <empty dir> --json     # error text lists the probe order

# (b) cross-family — B1
$M plugins marketplace add ohmy <dir: .claude-plugin/marketplace.json → ./plugins/omm (.muse-plugin pkg)> --json
$M plugins install omm@ohmy --json | jq .installed.manifest_family      # "native"
$M plugins inspect omm --json      | jq .plugin.manifest_family         # "native"

# (c) duplicates — C1/C2: list `omm` twice, swap the order, install → first entry wins

# (d) git — E1: bare repo of the marketplace, then
$M plugins marketplace add g1 file:///…/ohmy.git --json; $M plugins install omm@g1 --json
$M plugins marketplace update g1 --json      # generations/<ns> appears, lockfile repointed
$M plugins marketplace update g1 --json      # root source/ deleted; keep-2
$M plugins update omm --json                 # plugin-source-unavailable
$M plugins remove omm --json; $M plugins install omm@g1 --json     # follows the current generation

# native root catalog — R5/F1: write marketplace.json per §2 with the digest from
#   plugins list --available after a temp Codex-catalog add; F4 = change the package, keep the digest → install fails integrity
```
