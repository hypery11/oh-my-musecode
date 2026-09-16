//! No new stubs: `OmmError::NotImplemented` is legacy.
//!
//! `tests/hook_fail_open.rs → no_command_is_a_stub_any_more_and_the_hook_never_was`
//! proves no command prints `not implemented` at runtime; this lint proves
//! nobody reintroduces the construction at compile time. The variant itself
//! stays (see `src/error.rs`): removing it would break the total exit-code
//! contract for out-of-tree callers.

use std::path::{Path, PathBuf};

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_new_not_implemented_constructions() {
    let root = src_dir();
    let mut files = Vec::new();
    collect_rs(&root, &mut files);
    assert!(files.len() > 10, "expected the omm src tree");
    let mut hits = Vec::new();
    for file in &files {
        let name = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        // The variant's home: definition, exit-code mapping and its test.
        if name == "error.rs" {
            continue;
        }
        let text = std::fs::read_to_string(file).expect("read source");
        for (n, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            // Doc comments may name the legacy variant freely.
            if trimmed.starts_with("//") || !trimmed.contains("NotImplemented") {
                continue;
            }
            // `main.rs` keeps the display arm so the mapping stays total.
            if name == "main.rs" && trimmed.contains("OmmError::NotImplemented(cmd)") {
                continue;
            }
            hits.push(format!(
                "{}:{}: {}",
                file.strip_prefix(&root).unwrap_or(file).display(),
                n + 1,
                trimmed
            ));
        }
    }
    assert!(
        hits.is_empty(),
        "NotImplemented is legacy — no new constructions:\n{}",
        hits.join("\n")
    );
}
