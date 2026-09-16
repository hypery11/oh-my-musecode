//! Group B (`doctor`, `cost`, `lint`, `build`) against the real binary: the
//! `omm` executable is run as a subprocess inside a throwaway HOME / XDG
//! sandbox with a temp cwd (never the repository, never the user's roots),
//! `OMM_MUSE_BIN` pointing at the pinned host. Skipped with a message when
//! `OMM_MUSE_BIN` is unset.
//!
//! What is asserted beyond exit codes and JSON shapes: the read commands
//! leave the sandbox's config and data roots untouched (their live
//! measurements run in roots of their own), `build --dry-run` writes
//! nothing, `build` lands the package with the digest the binary reports,
//! `build --check` is clean right after a build and drifts after a content
//! edit, and `omm build` on content with a lint error refuses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

use omm_host::host_reality as hr;

/// The sandbox one test runs in.
struct Sb {
    _tmp: TempDir,
    root: PathBuf,
    config: PathBuf,
    data: PathBuf,
    ws: PathBuf,
    bin: PathBuf,
}

fn sandbox() -> Option<Sb> {
    let bin = std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)?;
    let tmp = tempfile::Builder::new()
        .prefix("omm-cmd-b-")
        .tempdir()
        .expect("temp dir");
    let root = std::fs::canonicalize(tmp.path()).expect("canonical temp dir");
    let sb = Sb {
        root: root.clone(),
        config: root.join("config"),
        data: root.join("data"),
        ws: root.join("ws"),
        bin,
        _tmp: tmp,
    };
    for d in [
        root.join("home"),
        sb.config.join("muse"),
        sb.data.clone(),
        sb.ws.clone(),
    ] {
        std::fs::create_dir_all(d).expect("sandbox dir");
    }
    std::fs::write(
        sb.config.join("muse").join("settings.json"),
        b"{\"schema_version\":1}\n",
    )
    .expect("settings.json");
    Some(sb)
}

macro_rules! sandbox_or_skip {
    () => {
        match sandbox() {
            Some(s) => s,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

impl Sb {
    /// Run `omm <args>` in the sandbox with `cwd`.
    fn omm(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_omm"))
            .args(args)
            .current_dir(cwd)
            .env("HOME", self.root.join("home"))
            .env("XDG_CONFIG_HOME", &self.config)
            .env("XDG_DATA_HOME", &self.data)
            .env("OMM_MUSE_BIN", &self.bin)
            .env("MUSE_NO_AUTO_UPDATE", "1")
            .env_remove("CI")
            .stdin(Stdio::null())
            .output()
            .expect("run omm")
    }

    /// `omm --json <args>` from the workspace; the parsed document and the exit code.
    fn json(&self, args: &[&str]) -> (Value, i32) {
        self.json_in(&self.ws.clone(), args)
    }

    fn json_in(&self, cwd: &Path, args: &[&str]) -> (Value, i32) {
        let mut argv = vec!["--json"];
        argv.extend_from_slice(args);
        let out = self.omm(cwd, &argv);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            stdout.lines().count(),
            1,
            "omm --json {args:?}: stdout must be exactly one line, got {stdout:?} (stderr {stderr:?})"
        );
        let doc: Value = serde_json::from_str(stdout.trim())
            .unwrap_or_else(|e| panic!("omm --json {args:?}: not JSON ({e}): {stdout:?}"));
        (doc, out.status.code().unwrap_or(-1))
    }

    /// Nothing of the read commands lands in the sandbox roots: no `$OMM`,
    /// the seed settings.json byte-identical, no session under the data root.
    fn assert_roots_untouched(&self) {
        assert!(
            !self.config.join("omm").exists(),
            "$OMM was created: {}",
            self.config.join("omm").display()
        );
        assert_eq!(
            std::fs::read(self.config.join("muse").join("settings.json")).unwrap(),
            b"{\"schema_version\":1}\n"
        );
        assert!(
            !self.data.join("muse").join("sessions").exists(),
            "a session was written under the sandbox data root"
        );
    }
}

/// The repository this crate was built in.
fn repo_root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".."))
        .expect("repo root")
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for entry in std::fs::read_dir(src).expect("read_dir") {
        let entry = entry.expect("entry");
        let ty = entry.file_type().expect("file type");
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to);
        } else if ty.is_file() {
            std::fs::copy(entry.path(), &to).expect("copy");
        } else {
            panic!(
                "{} is neither a file nor a directory",
                entry.path().display()
            );
        }
    }
}

/// A copy of the repository's source of truth — `content/`, the CLI crate's
/// `Cargo.toml` and the workspace `Cargo.toml` (the version lives there) —
/// with no generated output: what a clone looks like before `omm build`.
fn source_copy(sb: &Sb, name: &str) -> PathBuf {
    let repo = repo_root();
    let dst = sb.root.join(name);
    copy_dir(&repo.join("content"), &dst.join("content"));
    std::fs::create_dir_all(dst.join("crates").join("omm")).unwrap();
    std::fs::copy(
        repo.join("crates").join("omm").join("Cargo.toml"),
        dst.join("crates").join("omm").join("Cargo.toml"),
    )
    .unwrap();
    std::fs::copy(repo.join("Cargo.toml"), dst.join("Cargo.toml")).unwrap();
    dst
}

fn check(doc: &Value, id: &str) -> Value {
    doc["checks"]
        .as_array()
        .expect("checks[]")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("no check {id} in {doc}"))
        .clone()
}

#[test]
fn doctor_on_a_fresh_sandbox_is_critical_on_d1_with_its_fix_and_writes_nothing() {
    let sb = sandbox_or_skip!();
    let (doc, rc) = sb.json(&["doctor", "--fast"]);
    assert_eq!(rc, 1, "{doc}");
    assert_eq!(doc["ok"], false);
    assert_eq!(doc["exit_code"], 1);
    // D1–D15 from omm-doctor plus D16 (skill routing), appended by the CLI.
    assert_eq!(doc["checks"].as_array().unwrap().len(), 16);
    assert_eq!(
        check(&doc, "D16")["severity"],
        "info",
        "routing is off: a pass"
    );
    assert_eq!(doc["host"]["plugin_id"], "oh-my-musecode");
    assert_eq!(
        doc["host"]["config_root"],
        Value::String(sb.config.join("muse").display().to_string())
    );
    let d1 = check(&doc, "D1");
    assert_eq!(d1["severity"], "critical");
    assert_eq!(d1["fix"], "omm install");
    assert!(d1["observed"].as_str().unwrap().contains("not installed"));
    // `--fast` names what it skipped instead of pretending.
    assert!(check(&doc, "D8")["observed"]
        .as_str()
        .unwrap()
        .contains("disabled by options"));
    assert!(check(&doc, "D11")["observed"]
        .as_str()
        .unwrap()
        .contains("skipped"));
    assert_eq!(check(&doc, "D13")["severity"], "info");
    assert_eq!(check(&doc, "D12")["fix"], "omm trust .");
    // Human rendering: the header, one line per check, the fix under D1.
    let out = sb.omm(&sb.ws, &["doctor", "--fast"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("omm doctor — "), "{text}");
    assert!(text.contains("CRIT D1 "), "{text}");
    assert!(text.contains("        fix: omm install"), "{text}");
    assert!(text.contains("16 checks: 1 critical"), "{text}");
    sb.assert_roots_untouched();
}

#[test]
fn doctor_full_run_measures_a_live_session_and_the_host_in_roots_of_its_own() {
    let sb = sandbox_or_skip!();
    let (doc, rc) = sb.json(&["doctor"]);
    assert_eq!(rc, 1, "{doc}");
    let d8 = check(&doc, "D8");
    let observed = d8["observed"].as_str().unwrap();
    assert!(observed.contains("entries"), "{observed}");
    let d11 = check(&doc, "D11");
    assert_eq!(d11["severity"], "info", "{d11}");
    assert!(
        d11["observed"].as_str().unwrap().contains("rows match"),
        "{d11}"
    );
    let d14 = check(&doc, "D14");
    assert_eq!(d14["severity"], "info", "{d14}");
    sb.assert_roots_untouched();
}

#[test]
fn report_drift_prints_no_drift_on_the_pinned_binary() {
    let sb = sandbox_or_skip!();
    let (doc, rc) = sb.json(&["doctor", "--report-drift"]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["ok"], true);
    assert_eq!(doc["exit_code"], 0);
    assert_eq!(doc["drifted"], serde_json::json!([]));
    assert!(doc["checks"].as_array().unwrap().len() >= 14);
    let out = sb.omm(&sb.ws, &["doctor", "--report-drift"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("host drift: "), "{text}");
    assert!(text.contains("0 of "), "{text}");
    assert!(
        text.trim_end()
            .ends_with("no drift: every P0/P1 row matches docs/host-reality.md"),
        "{text}"
    );
    sb.assert_roots_untouched();
}

#[test]
fn cost_measures_the_bundled_tax_in_a_throwaway_root() {
    let sb = sandbox_or_skip!();
    let (doc, rc) = sb.json(&["cost", "--no-cuts"]);
    assert_eq!(rc, 0, "{doc}");
    // The pristine catalog is Meta's 14 visible built-ins, gate off
    // (host-reality.md "Budgets", "bundled skills in catalog").
    assert_eq!(
        doc["catalog"]["total_bytes"].as_u64().unwrap(),
        hr::BUILTIN_SKILLS_CATALOG_BLOCK_BYTES_GATE_OFF
    );
    // 19 bundled skills plus the threejs plugin skill, which composes
    // unconditionally since 1.3.0-R3057.1.
    assert_eq!(
        doc["catalog"]["entries"].as_u64().unwrap() as usize,
        hr::BUNDLED_SKILLS_VISIBLE_DEFAULT + 1
    );
    assert!(doc["tokens"]["method"]
        .as_str()
        .unwrap()
        .contains("bytes/4"));
    assert!(doc["cuts"].as_array().unwrap().len() >= hr::BUNDLED_SKILLS_VISIBLE_DEFAULT);
    let out = sb.omm(&sb.ws, &["cost", "--no-cuts"]);
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("omm cost — "), "{text}");
    assert!(text.contains("skills catalog (order 200)"), "{text}");
    assert!(text.contains("what to cut"), "{text}");
    sb.assert_roots_untouched();
}

#[test]
fn lint_the_shipped_content_with_the_host_checkpoints() {
    let sb = sandbox_or_skip!();
    let repo = repo_root();
    let (doc, rc) = sb.json(&["lint", repo.to_str().unwrap()]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["clean"], true);
    assert_eq!(doc["errors"], 0);
    assert_eq!(doc["host_checked"], true);
    assert_eq!(doc["budget"]["within_full"], true);
    assert!(doc["budget"]["total_full"].as_u64().unwrap() <= hr::BUNDLE_BUDGET_BYTES);
    assert!(doc["package_files"].as_u64().unwrap() > 0);
    assert_eq!(doc["repo"], Value::String(repo.display().to_string()));
    sb.assert_roots_untouched();
}

#[test]
fn lint_finds_the_repo_from_the_cwd_and_names_a_planted_symlink() {
    let sb = sandbox_or_skip!();
    let copy = source_copy(&sb, "planted");
    let skills = copy.join("content").join("skills");
    let first = std::fs::read_dir(&skills)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.is_dir())
        .expect("a skill dir");
    #[cfg(unix)]
    std::os::unix::fs::symlink(first.join("SKILL.md"), first.join("LINK.md")).unwrap();
    #[cfg(not(unix))]
    return;
    // Explicit path: the content dir itself resolves to its parent.
    let (doc, rc) = sb.json(&["lint", "--no-host", copy.join("content").to_str().unwrap()]);
    assert_eq!(rc, 1, "{doc}");
    assert_eq!(doc["clean"], false);
    assert_eq!(doc["host_checked"], false);
    assert_eq!(doc["repo"], Value::String(copy.display().to_string()));
    let rules: Vec<&str> = doc["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["rule"].as_str().unwrap())
        .collect();
    assert!(rules.contains(&"fs-symlink"), "{rules:?}");
    let link = doc["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["rule"] == "fs-symlink")
        .unwrap();
    assert!(
        link["path"]
            .as_str()
            .unwrap()
            .starts_with("content/skills/"),
        "{link}"
    );
    assert_eq!(link["severity"], "error");
    // No path: the checkout is found from the cwd.
    let (doc, rc) = sb.json_in(&skills, &["lint", "--no-host"]);
    assert_eq!(rc, 1, "{doc}");
    assert_eq!(doc["repo"], Value::String(copy.display().to_string()));
    // A cwd outside any checkout is a usage error (exit 2, JSON on stderr).
    let out = sb.omm(&sb.ws, &["--json", "lint", "--no-host"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());
    let err: Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(err["exit_code"], 2);
    assert!(err["error"]
        .as_str()
        .unwrap()
        .contains("content/catalog.json"));
    // `omm build` on content with a lint error refuses and writes nothing.
    let (doc, rc) = sb.json(&["build", copy.to_str().unwrap()]);
    assert_eq!(rc, 1, "{doc}");
    assert_eq!(doc["refused"], true);
    assert_eq!(doc["lint"]["clean"], false);
    assert!(!copy.join("plugins").exists());
    assert!(!copy.join("marketplace.json").exists());
}

#[test]
fn build_lands_the_package_check_is_clean_and_drifts_after_an_edit() {
    let sb = sandbox_or_skip!();
    let copy = source_copy(&sb, "clone");
    let root = copy.to_str().unwrap();
    // 1. Dry run: everything would be written, nothing is.
    let (doc, rc) = sb.json(&["--dry-run", "build", root]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["dry_run"], true);
    assert_eq!(doc["converge"]["dry_run"], true);
    assert!(doc["converge"]["total"]["updated"].as_u64().unwrap() > 0);
    // The only thing already current in a fresh copy is content/catalog.json's
    // budget numbers (the committed catalog is up to date).
    assert_eq!(doc["converge"]["total"]["unchanged"], 1, "{doc}");
    assert_eq!(
        doc["converge"]["categories"]["catalog budget"]["unchanged"], 1,
        "{doc}"
    );
    assert!(doc["digest"].as_str().unwrap().starts_with("sha256:"));
    assert!(!copy.join("plugins").exists(), "dry run wrote plugins/");
    assert!(!copy.join("dist").exists(), "dry run wrote dist/");
    assert!(
        !copy.join("marketplace.json").exists(),
        "dry run wrote marketplace.json"
    );
    // 2. The real build lands the package, the projections and the catalogs.
    let (doc, rc) = sb.json(&["build", root]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["dry_run"], false);
    assert_eq!(doc["plugin_id"], "oh-my-musecode");
    let digest = doc["digest"].as_str().unwrap().to_string();
    let updated = doc["converge"]["total"]["updated"].as_u64().unwrap();
    assert!(updated > 0, "{doc}");
    assert_eq!(doc["converge"]["total"]["removed"], 0);
    assert!(copy
        .join("plugins/oh-my-musecode/.muse-plugin/plugin.json")
        .is_file());
    assert!(copy
        .join("dist/claude/.claude-plugin/plugin.json")
        .is_file());
    assert!(copy.join("dist/codex/.codex-plugin/plugin.json").is_file());
    assert!(copy.join(".agents/plugins/marketplace.json").is_file());
    assert!(copy.join(".claude-plugin/marketplace.json").is_file());
    let native: Value =
        serde_json::from_slice(&std::fs::read(copy.join("marketplace.json")).unwrap()).unwrap();
    assert_eq!(
        native["plugins"][0]["integrity"]["digest"],
        Value::String(digest.clone())
    );
    assert_eq!(native["plugins"][0]["name"], "oh-my-musecode");
    // Category names are the repo-relative destinations.
    let cats = doc["converge"]["categories"].as_object().unwrap();
    for name in [
        "plugins/oh-my-musecode",
        "dist/claude",
        "dist/codex",
        "marketplace catalogs",
    ] {
        assert!(cats.contains_key(name), "{cats:?}");
    }
    assert_eq!(cats["marketplace catalogs"]["updated"], 3);
    // 3. The drift gate is clean right after a build, with the digest re-obtained.
    let (doc, rc) = sb.json(&["build", "--check", root]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["clean"], true);
    assert_eq!(doc["digest_checked"], true);
    assert_eq!(doc["version_bump_pending"], false);
    // 4. A second build is a no-op.
    let (doc, rc) = sb.json(&["build", root]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["converge"]["noop"], true);
    assert_eq!(doc["converge"]["total"]["updated"], 0);
    assert_eq!(doc["digest"], Value::String(digest.clone()));
    // 5. The full lint (host checkpoints + the committed catalogs' digest) passes.
    let (doc, rc) = sb.json(&["lint", root]);
    assert_eq!(rc, 0, "{doc}");
    assert_eq!(doc["clean"], true);
    assert_eq!(doc["host_checked"], true);
    // 6. Edit a shipped command: the gate names the drifted generated files.
    let command = std::fs::read_dir(copy.join("content").join("commands"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.extension().is_some_and(|e| e == "md"))
        .expect("a command file");
    let mut text = std::fs::read_to_string(&command).unwrap();
    text.push_str("\nOne more line for the drift gate.\n");
    std::fs::write(&command, text).unwrap();
    let (doc, rc) = sb.json(&["build", "--check", root]);
    assert_eq!(rc, 1, "{doc}");
    assert_eq!(doc["clean"], false);
    assert_eq!(doc["exit_code"], 1);
    let paths: Vec<&str> = doc["drifts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].as_str().unwrap())
        .collect();
    let stem = command.file_name().unwrap().to_str().unwrap();
    assert!(
        paths
            .iter()
            .any(|p| p.starts_with("plugins/oh-my-musecode/commands/") && p.ends_with(stem)),
        "{paths:?}"
    );
    assert!(
        paths.contains(&"marketplace.json"),
        "digest must drift too: {paths:?}"
    );
    let out = sb.omm(&sb.ws, &["build", "--check", root]);
    assert_eq!(out.status.code(), Some(1));
    let human = String::from_utf8_lossy(&out.stdout);
    assert!(human.contains("path(s) drifted"), "{human}");
    assert!(
        human
            .trim_end()
            .ends_with("fix: run `omm build` and commit the result"),
        "{human}"
    );
    // The user's roots stayed untouched throughout.
    sb.assert_roots_untouched();
}
