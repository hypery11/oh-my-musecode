//! `omm run -- <muse args>` — the launcher shim (ARCHITECTURE.md §7; R20;
//! e2e scenario 8).
//!
//! Host facts: there is NO unknown-command exit — an unrecognised first
//! token is the `[PROMPT]` positional and silently starts a billed session,
//! and a prompt can hide behind root flags (`muse --provider echo hi`,
//! `muse -w hi`; host-reality.md "Exit codes": unknown first token, bare
//! token after root flags). So the argv is checked against the 17 top-level
//! commands and the root-flag walk of `omm_host::allowlist` BEFORE the
//! binary is even located, with root flags allowed (`omm run -- --version`
//! is the user's explicit wish) and a trailing prompt refused. The host is
//! then `exec`ed through the `Invoker` with the user's environment inherited
//! (their own gates survive), `MUSE_NO_AUTO_UPDATE=1` (launcher-only, harmless
//! on the binary — host-reality.md "Paths"), the routing gates when
//! `skill_routing` is on (`hr::ENV_ROUTING_GATE` + `_APPLY`; both needed —
//! gates.json rows 36/37), and the active profile's extra gates from
//! `$OMM/config.json`, each resolved through gates.json (R8).

use std::path::PathBuf;

use serde_json::{json, Value};

use omm_host::allowlist::{validate_argv, ArgvPolicy};
use omm_host::host_reality as hr;

use super::OmmConfig;
use crate::cmd::Ctx;
use crate::error::{OmmError, Result};

/// What `omm run` would exec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunPlan {
    pub bin: PathBuf,
    pub argv: Vec<String>,
    /// Variables set on top of the inherited environment, sorted by name.
    pub env: Vec<(String, String)>,
    pub skill_routing: bool,
}

impl RunPlan {
    pub fn to_json(&self) -> Value {
        json!({
            "bin": self.bin.display().to_string(),
            "argv": self.argv,
            "env": self.env.iter().map(|(k, v)| json!({"name": k, "value": v})).collect::<Vec<_>>(),
            "skill_routing": self.skill_routing,
        })
    }

    pub fn render(&self) -> String {
        let env: Vec<String> = self.env.iter().map(|(k, v)| format!("{k}={v}")).collect();
        format!(
            "would exec: {} {}\nenv: {}{}",
            self.bin.display(),
            self.argv.join(" "),
            if env.is_empty() {
                "(inherited only)".to_string()
            } else {
                env.join(" ")
            },
            if self.skill_routing {
                "\nskill routing: on"
            } else {
                ""
            }
        )
    }
}

/// The gate variables the config turns on: `skill_routing` → the two routing
/// gates; `gates: [...]` entries resolved by gate id or `MUSE_EXPERIMENTAL_*`
/// name against gates.json (an unknown name is refused, never exported).
pub fn gate_env(config: &OmmConfig) -> Result<Vec<(String, String)>> {
    let mut env: Vec<(String, String)> = Vec::new();
    let mut push = |name: &str| {
        if !env.iter().any(|(k, _)| k == name) {
            env.push((name.to_string(), hr::GATE_ON_VALUE.to_string()));
        }
    };
    if config.skill_routing() {
        push(hr::ENV_ROUTING_GATE);
        push(hr::ENV_ROUTING_APPLY_GATE);
    }
    if let Some(raw) = config.gates_raw() {
        if !raw.is_array() {
            return Err(OmmError::Usage(format!(
                "config.json: `{}` must be an array of gate ids, found {}",
                super::CONFIG_KEY_GATES,
                super::json_kind(raw)
            )));
        }
    }
    let gates = hr::gates()?;
    for wanted in config.gates() {
        let gate = gates
            .items
            .iter()
            .find(|g| g.id == wanted || g.env == wanted)
            .ok_or_else(|| {
                OmmError::Usage(format!(
                    "config.json: `{}` names an unknown gate {wanted:?} (gates.json knows {} gates by id or {}* name)",
                    super::CONFIG_KEY_GATES,
                    gates.items.len(),
                    gates.env_prefix
                ))
            })?;
        push(&gate.env);
    }
    env.sort();
    Ok(env)
}

/// Validate the argv (R20) and build the plan.
pub fn plan(ctx: &Ctx, argv: &[String]) -> Result<RunPlan> {
    validate_argv(argv, ArgvPolicy::ROOT_FLAGS_OK)?;
    let config = OmmConfig::load(&ctx.omm_root())?;
    let mut env = gate_env(&config)?;
    env.push((
        hr::ENV_NO_AUTO_UPDATE.to_string(),
        hr::GATE_ON_VALUE.to_string(),
    ));
    env.sort();
    let bin = ctx.invoker()?.bin().to_path_buf();
    Ok(RunPlan {
        bin,
        argv: argv.to_vec(),
        env,
        skill_routing: config.skill_routing(),
    })
}

/// Replace this process with the host. Only returns on a failure to exec
/// (or, on non-unix targets, with the child's exit code).
pub fn exec(ctx: &Ctx, plan: &RunPlan) -> Result<std::process::ExitCode> {
    let mut inv = ctx.invoker()?.clone().inherit_env().allow_root_flags();
    for (k, v) in &plan.env {
        inv = inv.env(k, v);
    }
    #[cfg(unix)]
    {
        match inv.exec(&plan.argv) {
            Ok(never) => match never {},
            Err(e) => Err(e.into()),
        }
    }
    #[cfg(not(unix))]
    {
        let code = inv.spawn_inherited(&plan.argv)?;
        Ok(std::process::ExitCode::from(
            u8::try_from(code.unwrap_or(1)).unwrap_or(1),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(json: &str) -> OmmConfig {
        OmmConfig::from_bytes_lenient(json.as_bytes())
    }

    #[test]
    fn gate_env_comes_from_the_data_file() {
        assert!(gate_env(&config("{}")).unwrap().is_empty());
        let routing = gate_env(&config(r#"{"skill_routing": true}"#)).unwrap();
        assert_eq!(
            routing,
            vec![
                (hr::ENV_ROUTING_GATE.to_string(), "1".to_string()),
                (hr::ENV_ROUTING_APPLY_GATE.to_string(), "1".to_string()),
            ]
        );
        // By id and by env name, de-duplicated, sorted.
        let by_id = gate_env(&config(
            r#"{"gates": ["hook_selected_skills", "MUSE_EXPERIMENTAL_HOOK_SELECTED_SKILLS", "plugins"]}"#,
        ))
        .unwrap();
        assert_eq!(
            by_id,
            vec![
                (hr::ENV_ROUTING_GATE.to_string(), "1".to_string()),
                (hr::ENV_PLUGINS_GATE.to_string(), "1".to_string()),
            ]
        );
        let err = gate_env(&config(r#"{"gates": ["no_such_gate"]}"#)).unwrap_err();
        assert!(err.to_string().contains("no_such_gate"), "{err}");
        assert_eq!(err.exit_code(), 2);
        assert!(gate_env(&config(r#"{"gates": "x"}"#)).is_err());
    }

    #[test]
    fn plan_renders_and_serialises() {
        let p = RunPlan {
            bin: PathBuf::from("/x/muse-bin"),
            argv: vec!["--version".into()],
            env: vec![("MUSE_NO_AUTO_UPDATE".into(), "1".into())],
            skill_routing: false,
        };
        let text = p.render();
        assert!(text.contains("would exec: /x/muse-bin --version"));
        assert!(text.contains("MUSE_NO_AUTO_UPDATE=1"));
        assert!(!text.contains("skill routing"));
        let j = p.to_json();
        assert_eq!(j["argv"][0], "--version");
        assert_eq!(j["env"][0]["name"], "MUSE_NO_AUTO_UPDATE");
    }
}
