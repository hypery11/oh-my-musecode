# omm-release: worked example

Request: "cut v1.4.0". Repo: a Rust CLI, `cargo` workspace, tag-triggered GitHub workflow that
builds the release matrix and uploads to the GitHub release, no registry publish.

Writes announced first: version commit on `main`, annotated tag `v1.4.0`, push of both, GitHub
release with the workflow's artifacts and `SHA256SUMS`. No registry.

```
0  git status --porcelain --untracked-files=all; echo exit=$?      -> (nothing) exit=0
   git fetch origin && git status -sb                              -> ## main...origin/main
   git describe --tags --abbrev=0                                  -> v1.3.2
   git log --oneline v1.3.2..HEAD | wc -l                          -> 17
   search RELEASING* / release in Makefile, justfile               -> none; .github/workflows/release.yml (on: push tags v*)
1  cargo clean && cargo build --release && cargo test --workspace  -> test result: ok. 412 passed; 0 failed   exit=0
   cargo clippy --workspace -- -D warnings                         -> exit=0
2  git tag --list --sort=-v:refname | head -3                      -> v1.3.2 v1.3.1 v1.3.0   (prefix v, semver)
   commits: 3 feat, 11 fix, 3 chore; no BREAKING                   -> minor: v1.4.0
   git tag -l v1.4.0                                               -> (nothing)
3  search "1.3.2" hidden:true glob:["!CHANGELOG*"]                 -> Cargo.toml:3, Cargo.lock:214, README.md:41 (curl .../v1.3.2/...), CITATION.cff:12
   cargo set-version 1.4.0; cargo check                            -> Cargo.lock: only the crate's own version line
   edit_file README.md, CITATION.cff (version + date-released)
   re-search "1.3.2"                                               -> CHANGELOG.md only (excluded)
   ./target/release/mytool --version                               -> mytool 1.4.0
4  read_file CHANGELOG.md head; ## [Unreleased] -> ## [1.4.0] - 2026-09-05; 17 commits -> 14 lines, 3 skipped (chore: CI)
5  git add Cargo.toml Cargo.lock README.md CITATION.cff CHANGELOG.md; git diff --cached --stat -> 5 files
   git commit -F - <<'EOF' ... "release: v1.4.0" EOF
   cargo test -p mytool version_string                             -> ok (asserts 1.4.0)
   git tag -a v1.4.0 -m v1.4.0; git describe --exact-match HEAD    -> v1.4.0
6  recipe lives in the tag workflow                                -> nothing built locally
7  git push origin main; git push origin v1.4.0
   gh run watch <id>                                               -> completed success
   gh release download v1.4.0 -D /tmp/rel && (cd /tmp/rel && shasum -a 256 -c SHA256SUMS) -> 4 files OK
   gh release view v1.4.0 --json assets -q '.assets|length'        -> 5   (4 archives + SHA256SUMS)
8  D=$(mktemp -d); HOME=$D CARGO_HOME=$D/.cargo sh -c "$(curl -fsSL https://.../v1.4.0/install.sh)"
   $D/.local/bin/mytool --version                                  -> mytool 1.4.0
   $D/.local/bin/mytool check .                                    -> ok
```

Report:

```
Released v1.4.0 -> 9f3c2ab   (range v1.3.2..v1.4.0, 17 commits)
Gate: cargo test --workspace -> 412 passed; 0 failed, exit 0 (re-run at 9f3c2ab)
Version: 4 files bumped; old string 0 hits outside CHANGELOG
Changelog: 17 commits -> 14 lines, 3 skipped (chore: CI only)
Artifacts: mytool-1.4.0-{linux-x86_64,linux-aarch64,darwin-x86_64,darwin-aarch64}.tar.gz; SHA256SUMS verified after download
Published: https://github.com/org/mytool/releases/tag/v1.4.0; registry: none
Clean install: curl ... install.sh -> mytool 1.4.0
Not done: none
```
