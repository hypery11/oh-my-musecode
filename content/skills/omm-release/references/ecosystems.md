# omm-release: where the version lives, per ecosystem

Step 3 searches for the previous version string and decides every hit; this table says where
to expect hits, which tool bumps them, how the lockfile follows, and which cache directory the
clean install (step 8) must point inside the throwaway `HOME`.

| Ecosystem | Version homes | Bump tool | Lockfile follows via | Cache dir to redirect in step 8 |
|---|---|---|---|---|
| Rust (cargo) | `Cargo.toml` `[package].version` (one per workspace member, or `workspace.package.version`), `Cargo.lock` | `cargo set-version <x.y.z>` (cargo-edit); else `edit_file` | `cargo check` | `CARGO_HOME=$D/.cargo` |
| Node (npm/pnpm/yarn) | `package.json` `version`, `package-lock.json` / `pnpm-lock.yaml` / `yarn.lock`, `npm-shrinkwrap.json` | `npm version <x.y.z> --no-git-tag-version` | `npm install --package-lock-only`, `pnpm install --lockfile-only` | `npm_config_cache=$D/.npm`, `PNPM_HOME=$D/.pnpm` |
| Python (uv/poetry/setuptools) | `pyproject.toml` `[project].version`, `<pkg>/__init__.py` `__version__`, `setup.cfg`, `uv.lock` / `poetry.lock` | `poetry version <x.y.z>`, `uv version <x.y.z>`, `bump-my-version` | `uv lock`, `poetry lock --no-update` | `UV_CACHE_DIR=$D/.uv`, `PIP_CACHE_DIR=$D/.pip` |
| Go | `version.go` / `internal/version` constant, `-ldflags "-X main.version=<x.y.z>"` in the build recipe; the module path carries `/v2` only on a major | `edit_file`; a tag alone is enough for library modules | none (`go.sum` is content, not version) | `GOMODCACHE=$D/go/pkg/mod`, `GOCACHE=$D/.gocache`, `GOPATH=$D/go` |
| Ruby | `lib/<gem>/version.rb`, `*.gemspec`, `Gemfile.lock` | `edit_file` | `bundle install` | `GEM_HOME=$D/.gem`, `BUNDLE_PATH=$D/.bundle` |
| Java/Kotlin | `pom.xml` `<version>`, `build.gradle(.kts)` `version =`, `gradle.properties` | `mvn versions:set -DnewVersion=<x.y.z>` | none | `MAVEN_OPTS=-Dmaven.repo.local=$D/.m2`, `GRADLE_USER_HOME=$D/.gradle` |
| .NET | `Directory.Build.props` / `*.csproj` `<Version>`, `packages.lock.json` | `edit_file` | `dotnet restore --force-evaluate` | `NUGET_PACKAGES=$D/.nuget` |
| Shell / single binary | `VERSION` file, `version=` line in the script, `--version` output | `edit_file` | none | none (copy the artifact into `$D` and run it) |
| Every ecosystem | `CHANGELOG.md` (step 4, excluded from the search), `plugin.json` / `manifest.json`, `action.yml`, `CITATION.cff` `version:` and `date-released:`, `Dockerfile` `LABEL org.opencontainers.image.version`, `openapi.*` `info.version`, `README` install lines (`@1.1.0`, `/v1.1.0/` inside a curl URL, `pip install <pkg>==1.1.0`) | `edit_file` per hit | - | - |

Rules the table does not repeat: a lockfile is never edited by hand; after the bump the only
lockfile change is the package's own version line; a hit left at the old value is named in the
report with its reason (a fixture, a "removed in 1.1.0" note, a compatibility table).
