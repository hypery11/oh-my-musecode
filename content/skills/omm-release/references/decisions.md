# omm-release: decisions and refusals

## Decisions

| Situation | Decision |
|---|---|
| Gate red on a test the release did not touch | Still red; fix or revert first, then step 0 |
| Monorepo | Only packages changed since their own last tag; per-package prefix (`pkg-v1.2.0`), or one tag when versions move in lockstep |
| Hotfix for an old line | Branch from the old tag (`release/1.1`), cherry-pick, patch bump there; never tag 1.1.x on main |
| Pre-release | `1.2.0-rc.1`, `--prerelease`; CHANGELOG heading as previous pre-releases did |
| Defect found after the tag is pushed | Never move the tag; fix, release X.Y.Z+1 with a Fixed line |
| Version already bumped by a merged PR | Skip the edits in step 3; still run its search to prove every place agrees |
| No CI, no tests | Gate is build + `--version` + one smoke run; report says "no test suite" |
| Registry publish | Only when the request or the repo's release script names it, and only if `<prev>` is already there (`npm view <pkg> versions`, `cargo info <crate>`, `pip index versions <pkg>`); `--dry-run` first with its output pasted. A bad version is yanked (`cargo yank`, `npm deprecate`, `gem yank`), never replaced in place |
| Upload interrupted | `gh release view` for what landed; upload the missing files; never recompute checksums |
| Release script asks on a tty | `tty: true`, answer through `bash_input`; never pipe `yes` |

## Refuse

- A dirty tree, a branch behind or off its upstream, a red or skipped gate;
  `--no-verify` or a skipped test to get green.
- `git tag -f`, `git push --force`, deleting a pushed tag, replacing a
  published asset in place.
- Guessing the date, the version, or artifact names.
- Hand-edited lockfiles; checksums computed before the final build or
  copied from the last release; artifacts built from an uncommitted tree.
- Skipping step 8 because CI is green; CI does not read the README.
- A registry publish the request did not name.
