//! Native plugin packages built on the fly for hostcheck and tests.
//!
//! Layout and required fields per `research/experiments/plugin-contract.md`
//! §1.1–§1.2: the manifest is `<root>/.muse-plugin/plugin.json` with exactly
//! `schemaVersion, name, displayName, version, description, compat,
//! capabilities`; unknown top-level keys are warnings and warnings are
//! failures for creation. Capability entry shapes are the bundled
//! `capability-examples.json` ones (`research/musecode/binary-artifacts/create-plugin/`).

use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::error::{HostError, Result};
use crate::fsx;
use crate::host_reality as hr;

/// A package to materialise.
#[derive(Clone, Debug)]
pub struct PackageSpec {
    /// Plugin id (`name`).
    pub name: String,
    /// The `schemaVersion` value verbatim (so fixtures can send 0, 2, 99, "1").
    pub schema_version: Value,
    /// The `capabilities` object.
    pub capabilities: Value,
    /// `(relative path, content)` files to write beside the manifest.
    pub files: Vec<(String, String)>,
    /// Manifest dir; `.muse-plugin` for the native family.
    pub manifest_dir: String,
}

impl PackageSpec {
    /// A native package with the given capabilities and the standard files.
    pub fn native(name: &str, capabilities: Value) -> PackageSpec {
        PackageSpec {
            name: name.to_string(),
            schema_version: Value::from(hr::PLUGIN_MANIFEST_SCHEMA_VERSION),
            capabilities,
            files: standard_files(),
            manifest_dir: hr::MANIFEST_DIR_NATIVE.to_string(),
        }
    }

    /// The manifest document.
    pub fn manifest(&self) -> Value {
        json!({
            "schemaVersion": self.schema_version,
            "name": self.name,
            "displayName": self.name,
            "version": "0.1.0",
            "description": "omm host fixture package.",
            "compat": {"source": "native", "manifestDir": self.manifest_dir},
            "capabilities": self.capabilities,
        })
    }
}

/// Write the package under `parent/<dir_name>` and return its root.
pub fn write_package(parent: &Path, dir_name: &str, spec: &PackageSpec) -> Result<PathBuf> {
    let root = parent.join(dir_name);
    let manifest_dir = root.join(&spec.manifest_dir);
    fsx::create_dir_all(&manifest_dir)?;
    let manifest_path = manifest_dir.join("plugin.json");
    let bytes = serde_json::to_vec_pretty(&spec.manifest()).map_err(|e| HostError::Parse {
        what: "fixture manifest",
        detail: e.to_string(),
    })?;
    std::fs::write(&manifest_path, bytes).map_err(|e| HostError::io("write", &manifest_path, e))?;
    for (rel, content) in &spec.files {
        let path = root.join(rel);
        if let Some(p) = path.parent() {
            fsx::create_dir_all(p)?;
        }
        std::fs::write(&path, content).map_err(|e| HostError::io("write", &path, e))?;
        #[cfg(unix)]
        if rel.ends_with(".sh") {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .map_err(|e| HostError::io("chmod", &path, e))?;
        }
    }
    Ok(root)
}

/// The files every fixture family may reference.
pub fn standard_files() -> Vec<(String, String)> {
    vec![
        (
            "skills/omm-fx/SKILL.md".to_string(),
            "---\nname: omm-fx\ndescription: omm host fixture skill. Do not use for anything.\n---\n\nFixture body.\n".to_string(),
        ),
        (
            "commands/omm-cmd.md".to_string(),
            "---\ndescription: omm host fixture command\n---\n\nSay hello.\n".to_string(),
        ),
        (
            "hooks/omm-hook.sh".to_string(),
            "#!/bin/sh\necho '{}'\n".to_string(),
        ),
        ("mcp/server.py".to_string(), "print('noop')\n".to_string()),
        (
            "reminders/omm-rem.md".to_string(),
            "omm host fixture reminder.\n".to_string(),
        ),
    ]
}

/// A skill capability entry.
pub fn skill_entry() -> Value {
    json!({"id": "omm-fx", "path": "skills/omm-fx/SKILL.md", "enabledDefault": true})
}
/// A command capability entry.
pub fn command_entry() -> Value {
    json!({"id": "omm-cmd", "path": "commands/omm-cmd.md", "enabledDefault": true})
}
/// A hook capability entry for `event`.
pub fn hook_entry(event: &str) -> Value {
    json!({"id": "omm-hook", "event": event, "command": ["sh", "hooks/omm-hook.sh"], "timeoutMs": 1000})
}
/// An MCP server capability entry.
pub fn mcp_entry() -> Value {
    json!({"id": "omm-mcp", "transport": "stdio", "command": ["python3", "mcp/server.py"]})
}
/// A reminder capability entry (decision block from the bundled example).
pub fn reminder_entry() -> Result<Value> {
    let decision = hr::reminder_decision_fixture()?.decision.clone();
    Ok(json!({
        "id": "omm-rem",
        "path": "reminders/omm-rem.md",
        "tools": ["read_file"],
        "blocking": false,
        "decision": decision,
    }))
}

/// All five supported families, one entry each.
pub fn five_family_capabilities() -> Result<Value> {
    Ok(json!({
        "skills": [skill_entry()],
        "commands": [command_entry()],
        "hooks": [hook_entry("SessionStart")],
        "mcpServers": [mcp_entry()],
        "reminders": [reminder_entry()?],
    }))
}

/// One family only, by its manifest spelling.
pub fn single_family_capabilities(family: &str) -> Result<Value> {
    let entry = match family {
        "skills" => skill_entry(),
        "commands" => command_entry(),
        "hooks" => hook_entry("SessionStart"),
        "mcpServers" => mcp_entry(),
        "reminders" => reminder_entry()?,
        other => {
            return Err(HostError::Probe(format!(
                "no fixture for capability family `{other}`"
            )))
        }
    };
    let mut m = Map::new();
    m.insert(family.to_string(), Value::Array(vec![entry]));
    Ok(Value::Object(m))
}

/// A rejected family (`tools`) with a shape that would otherwise be fine.
pub fn tools_capability() -> Value {
    json!({"tools": [{"id": "omm-tool", "path": "commands/omm-cmd.md"}]})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_a_native_layout() {
        let dir = tempfile::tempdir().unwrap();
        let spec = PackageSpec::native("omm-five", five_family_capabilities().unwrap());
        let root = write_package(dir.path(), "five", &spec).unwrap();
        assert!(root.join(".muse-plugin/plugin.json").exists());
        assert!(root.join("skills/omm-fx/SKILL.md").exists());
        assert!(root.join("hooks/omm-hook.sh").exists());
        let m: Value =
            serde_json::from_slice(&std::fs::read(root.join(".muse-plugin/plugin.json")).unwrap())
                .unwrap();
        assert_eq!(m["schemaVersion"], 1);
        assert_eq!(m["capabilities"].as_object().unwrap().len(), 5);
        assert!(
            m["capabilities"]["reminders"][0]["decision"]["envelope"]["template"]
                .as_str()
                .unwrap()
                .contains("<reminder>")
        );
    }
}
