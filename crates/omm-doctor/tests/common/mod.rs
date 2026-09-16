//! Shared harness of the doctor integration tests: a throwaway HOME/XDG
//! sandbox, the real binary (`OMM_MUSE_BIN`, skipped when unset), fixture
//! plugins, and fake hosts that wrap the real one for planted drift.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::TempDir;

use omm_doctor::{Context, Options};
use omm_host::fixtures::{self, PackageSpec};
use omm_host::probe;
use omm_host::{Invoker, Roots, Sandbox};

pub struct Host {
    pub bin: PathBuf,
    pub inv: Invoker,
    pub sb: Sandbox,
    pub roots: Roots,
    pub tmp: TempDir,
}

pub fn host() -> Option<Host> {
    let bin = std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)?;
    let tmp = tempfile::Builder::new()
        .prefix("omm-doctor-test-")
        .tempdir()
        .expect("temp dir");
    let sb = Sandbox::create(tmp.path()).expect("sandbox");
    let cfg = sb.config_home.join("muse");
    std::fs::create_dir_all(&cfg).expect("config dir");
    std::fs::write(cfg.join("settings.json"), b"{\"schema_version\":1}\n").expect("settings");
    let inv = Invoker::new(&bin).sandboxed(&sb);
    let roots = sb.roots().expect("roots");
    Some(Host {
        bin,
        inv,
        sb,
        roots,
        tmp,
    })
}

#[macro_export]
macro_rules! host_or_skip {
    () => {
        match $crate::common::host() {
            Some(h) => h,
            None => {
                eprintln!("skipped: OMM_MUSE_BIN is unset");
                return;
            }
        }
    };
}

impl Host {
    pub fn settings_path(&self) -> PathBuf {
        self.roots.settings_file()
    }
    /// Overwrite settings.json with a JSON document.
    pub fn write_settings(&self, doc: Value) {
        std::fs::write(self.settings_path(), doc.to_string()).expect("write settings");
    }
    /// Overwrite settings.json with raw bytes (for malformed documents).
    pub fn write_settings_raw(&self, bytes: &[u8]) {
        std::fs::write(self.settings_path(), bytes).expect("write settings");
    }
    /// A workspace directory under the sandbox.
    pub fn workspace(&self, name: &str) -> PathBuf {
        let ws = self.sb.root.join(name);
        std::fs::create_dir_all(&ws).expect("workspace");
        ws
    }
    /// A context over the sandbox with the slow probes off; tests switch them on.
    pub fn ctx(&self) -> Context {
        Context::new(self.inv.clone(), self.roots.clone())
            .expect("context")
            .with_workspace(Some(self.workspace("ws")))
            .with_options(Options {
                live: false,
                host_drift: false,
                ..Options::default()
            })
    }
    /// The same over another invoker (a fake host).
    pub fn ctx_with(&self, inv: Invoker) -> Context {
        Context::new(inv, self.roots.clone())
            .expect("context")
            .with_workspace(Some(self.workspace("ws")))
            .with_options(Options {
                live: false,
                host_drift: false,
                ..Options::default()
            })
    }
    /// Write and install a five-family native fixture plugin, unapproved.
    pub fn install_fixture(&self, name: &str) -> PathBuf {
        let spec = PackageSpec::native(name, fixtures::five_family_capabilities().unwrap());
        let root = fixtures::write_package(&self.sb.root.join("pkgs"), name, &spec).unwrap();
        self.inv
            .run(&[
                "plugins".to_string(),
                "install".to_string(),
                root.to_string_lossy().into_owned(),
                "--json".to_string(),
            ])
            .expect("install")
            .expect_ok()
            .expect("install ok");
        root
    }
    /// Install and approve every capability.
    pub fn install_and_approve(&self, name: &str) -> PathBuf {
        let root = self.install_fixture(name);
        probe::plugins_approve(&self.inv, name).expect("approve");
        root
    }
    /// Install a `.claude-plugin` package (installs as `claude-compatible`).
    pub fn install_claude_fixture(&self, name: &str) -> PathBuf {
        let root = self.sb.root.join("pkgs").join(name);
        std::fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        std::fs::create_dir_all(root.join("skills/omm-fx")).unwrap();
        std::fs::write(
            root.join("skills/omm-fx/SKILL.md"),
            "---\nname: omm-fx\ndescription: omm host fixture skill. Do not use for anything.\n---\n\nFixture body.\n",
        )
        .unwrap();
        std::fs::write(
            root.join(".claude-plugin/plugin.json"),
            format!("{{\"name\":\"{name}\",\"version\":\"0.1.0\",\"description\":\"fixture\",\"skills\":[\"./skills/omm-fx\"]}}"),
        )
        .unwrap();
        self.inv
            .run(&[
                "plugins".to_string(),
                "install".to_string(),
                root.to_string_lossy().into_owned(),
                "--json".to_string(),
            ])
            .expect("install")
            .expect_ok()
            .expect("install ok");
        root
    }
    /// Run a muse argv and require exit 0.
    pub fn muse(&self, argv: &[&str]) {
        self.inv
            .run(argv)
            .expect("run")
            .expect_ok()
            .unwrap_or_else(|e| panic!("{argv:?}: {e}"));
    }
    /// A bash wrapper around the real binary that runs `script_body` first
    /// (with `$REAL` set) and otherwise execs the real binary. For planted
    /// host drift.
    pub fn fake_host(&self, name: &str, script_body: &str) -> Invoker {
        let path = self.sb.root.join(format!("fake-{name}"));
        let script = format!(
            "#!/bin/bash\nREAL='{}'\n{script_body}\nexec \"$REAL\" \"$@\"\n",
            self.bin.display()
        );
        std::fs::write(&path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        Invoker::new(&path).sandboxed(&self.sb)
    }
    /// N user skills with long two-sentence descriptions under the config root.
    pub fn user_skills(&self, n: usize, description_pad: usize) {
        let pad = "x".repeat(description_pad);
        for i in 0..n {
            let dir = self.roots.personal_skills_dir().join(format!("cs-{i:03}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("SKILL.md"),
                format!("---\nname: cs-{i:03}\ndescription: Probe {i} first sentence. Second sentence {pad}.\n---\n# cs-{i:03}\n"),
            )
            .unwrap();
        }
    }
}

/// The ledger document D13 reads, written under `$OMM/`.
pub fn write_ledger(roots: &Roots, doc: &Value) -> PathBuf {
    let dir = roots.omm_root();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("omm.lock.json");
    std::fs::write(&path, doc.to_string()).unwrap();
    path
}

pub fn exists(p: &Path) -> bool {
    p.exists()
}
