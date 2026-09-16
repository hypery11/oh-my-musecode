//! Rendering: the human table (one line per check, the fix command and the
//! why-silent text indented under a failed row) and the `--json` document.

use serde_json::{json, Value};

use crate::check::{Report, Severity};

/// Indentation of the lines under a check row.
const INDENT: &str = "        ";

impl Report {
    /// The human form.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "omm doctor — {} ({}) — config {} — plugin `{}`\n",
            self.host.binary.display(),
            self.host.version.as_deref().unwrap_or("version unknown"),
            self.host.config_root.display(),
            self.host.plugin_id
        ));
        let w_title = self
            .checks
            .iter()
            .map(|c| c.title.chars().count())
            .max()
            .unwrap_or(10);
        for c in &self.checks {
            out.push_str(&format!(
                "{} {:<3} {:<w_title$}  {}\n",
                c.severity.tag(),
                c.id,
                c.title,
                c.observed
            ));
            if c.failed() {
                if let Some(fix) = &c.fix {
                    for (i, line) in fix.lines().enumerate() {
                        let label = if i == 0 { "fix: " } else { "     " };
                        out.push_str(&format!("{INDENT}{label}{line}\n"));
                    }
                }
                out.push_str(&format!("{INDENT}why silent: {}\n", c.why_silent));
            }
        }
        let critical = self
            .checks
            .iter()
            .filter(|c| c.severity == Severity::Critical)
            .count();
        let warn = self
            .checks
            .iter()
            .filter(|c| c.severity == Severity::Warn)
            .count();
        out.push_str(&format!(
            "{} checks: {critical} critical, {warn} warn — {} ({} ms, exit {})\n",
            self.checks.len(),
            if self.ok { "ok" } else { "NOT OK" },
            self.elapsed_ms,
            self.exit_code()
        ));
        out
    }

    /// The `--json` document.
    pub fn to_json(&self) -> Value {
        json!({
            "host": self.host,
            "ok": self.ok,
            "exit_code": self.exit_code(),
            "elapsed_ms": self.elapsed_ms,
            "checks": self.checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::{Check, HostInfo};
    use std::path::PathBuf;

    #[test]
    fn renders_one_line_per_check_with_indented_fix() {
        let host = HostInfo {
            binary: PathBuf::from("/bin/muse-bin-1"),
            version: Some("Muse Code 1.0.1 (1.0.1-R2006.1)".into()),
            config_root: PathBuf::from("/c/muse"),
            data_root: PathBuf::from("/d/muse"),
            omm_root: PathBuf::from("/c/omm"),
            plugin_id: "omm".into(),
            workspace: None,
        };
        let r = Report::new(
            host,
            vec![
                Check::info("D2", "provider", "provider = meta", "why2"),
                Check::critical(
                    "D3",
                    "mcp collision",
                    "both keys",
                    "why3",
                    "omm settings fix-mcp-collision\nsecond line",
                ),
            ],
            7,
        );
        let text = r.render();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("omm doctor — /bin/muse-bin-1"));
        assert!(lines[1].starts_with("ok   D2  provider"), "{}", lines[1]);
        assert!(
            lines[2].starts_with("CRIT D3  mcp collision"),
            "{}",
            lines[2]
        );
        assert_eq!(lines[3], "        fix: omm settings fix-mcp-collision");
        assert_eq!(lines[4], "             second line");
        assert!(lines[5].starts_with("        why silent: why3"));
        assert!(
            lines[6].contains("2 checks: 1 critical, 0 warn — NOT OK"),
            "{}",
            lines[6]
        );
        assert!(lines[6].ends_with("exit 1)"));
        let j = r.to_json();
        assert_eq!(j["ok"], false);
        assert_eq!(j["exit_code"], 1);
        assert_eq!(j["checks"][1]["severity"], "critical");
        assert_eq!(j["checks"][0]["fix"], Value::Null);
        assert_eq!(j["host"]["plugin_id"], "omm");
    }
}
