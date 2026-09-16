//! `omm mcp` end to end (PLAN.md 2.2): the binary on its own pipe, and the
//! host driving it through the mock provider harness
//! (`tools/mockprovider/run-omm-mcp.sh`).
//!
//! Every run is sandboxed: a throwaway `HOME` / `XDG_CONFIG_HOME` /
//! `XDG_DATA_HOME`, a workspace as cwd, the server's `MUSE_PLUGIN_DATA_DIR`
//! under the sandbox's data root (the host advertises but never creates it —
//! the server must). The tests that need the host skip with a message when
//! `OMM_MUSE_BIN` is unset; the harness test also needs `python3`.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use serde_json::{json, Value};
use tempfile::TempDir;

use omm_host::Sandbox;

fn repo_root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(".."))
        .expect("repo root")
}

fn host_bin() -> Option<PathBuf> {
    std::env::var_os("OMM_MUSE_BIN")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

struct Server {
    _tmp: TempDir,
    sb: Sandbox,
    ws: PathBuf,
    child: Child,
    reader: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl Server {
    /// Spawn `omm mcp` the way the host does: a scrubbed environment (`HOME`
    /// and `PATH`; `XDG_*` only because the sandbox redirects them there),
    /// the workspace as cwd, `MUSE_PLUGIN_DATA_DIR` advertised. `host` is
    /// `OMM_MUSE_BIN` for the tool calls that need it — the CLI's own
    /// locate honours it when present; the host never sets it.
    fn spawn(host: Option<&Path>, path: Option<&Path>) -> Server {
        let tmp = tempfile::Builder::new()
            .prefix("omm-mcp-test-")
            .tempdir()
            .expect("temp dir");
        let sb = Sandbox::create(tmp.path()).expect("sandbox");
        let ws = tmp.path().join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        let data_dir = sb
            .data_home
            .join("muse")
            .join("plugins")
            .join("data")
            .join("oh-my-musecode");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_omm"));
        cmd.arg("mcp")
            .env_clear()
            .env(
                "PATH",
                path.map(|p| p.as_os_str().to_owned())
                    .unwrap_or_else(|| std::env::var_os("PATH").unwrap_or_default()),
            )
            .env("HOME", &sb.home)
            .env("XDG_CONFIG_HOME", &sb.config_home)
            .env("XDG_DATA_HOME", &sb.data_home)
            .env("TMPDIR", std::env::temp_dir())
            .env("MUSE_PLUGIN_DATA_DIR", &data_dir)
            .env("MUSE_PLUGIN_ID", "oh-my-musecode")
            .current_dir(&ws)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(h) = host {
            cmd.env("OMM_MUSE_BIN", h);
        }
        let mut child = cmd.spawn().expect("spawn omm mcp");
        let reader = BufReader::new(child.stdout.take().expect("stdout"));
        Server {
            _tmp: tmp,
            sb,
            ws,
            child,
            reader,
            next_id: 1,
        }
    }

    fn send_raw(&mut self, line: &str) {
        let stdin = self.child.stdin.as_mut().expect("stdin");
        stdin.write_all(line.as_bytes()).expect("write");
        stdin.write_all(b"\n").expect("write");
        stdin.flush().expect("flush");
    }

    fn read_line(&mut self) -> Value {
        let mut line = String::new();
        let n = self.reader.read_line(&mut line).expect("read");
        assert!(n > 0, "the server closed stdout");
        serde_json::from_str(line.trim()).unwrap_or_else(|e| panic!("{line:?}: {e}"))
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send_raw(
            &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string(),
        );
        let reply = self.read_line();
        assert_eq!(reply["id"], id, "{reply}");
        reply
    }

    fn notify(&mut self, method: &str) {
        self.send_raw(&json!({"jsonrpc": "2.0", "method": method}).to_string());
    }

    fn handshake(&mut self) -> Value {
        let init = self.request(
            "initialize",
            json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "tbh", "version": "1.0.1"}}),
        );
        self.notify("notifications/initialized");
        init
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let reply = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        assert!(reply.get("error").is_none(), "{reply}");
        reply["result"].clone()
    }

    fn text_of(result: &Value) -> &str {
        let content = result["content"].as_array().expect("content[]");
        assert_eq!(content.len(), 1, "exactly one part: {result}");
        assert_eq!(content[0]["type"], "text");
        content[0]["text"].as_str().expect("text")
    }

    /// Close stdin and collect the exit status, stderr and the text of the
    /// server's trace (`$MUSE_PLUGIN_DATA_DIR/omm-mcp.log`). The trace is
    /// read here, while `_tmp` is still alive: `finish` consumes the
    /// sandbox, so a returned path would dangle.
    fn finish(mut self) -> (i32, String, String) {
        drop(self.child.stdin.take());
        let out = self.child.wait_with_output().expect("wait");
        let trace = self
            .sb
            .data_home
            .join("muse/plugins/data/oh-my-musecode/omm-mcp.log");
        // The dir under MUSE_PLUGIN_DATA_DIR was created by the server.
        let text =
            std::fs::read_to_string(&trace).unwrap_or_else(|e| panic!("{}: {e}", trace.display()));
        (
            out.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&out.stderr).into_owned(),
            text,
        )
    }
}

#[test]
fn the_handshake_tools_list_and_junk_over_the_real_pipe_then_exit_0_at_eof() {
    let mut s = Server::spawn(None, None);
    let init = s.handshake();
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(init["result"]["capabilities"], json!({"tools": {}}));
    assert_eq!(init["result"]["serverInfo"]["name"], "omm");
    let list = s.request("tools/list", json!({}));
    let names: Vec<&str> = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["omm_doctor", "omm_cost"]);
    assert_eq!(s.request("ping", json!({}))["result"], json!({}));
    // Junk on the pipe is answered, never fatal.
    s.send_raw("this is not json");
    let err = s.read_line();
    assert_eq!(err["error"]["code"], -32700);
    assert_eq!(err["id"], Value::Null);
    s.send_raw(r#"{"jsonrpc":"2.0","id":77,"method":"tools/call","params":{"name":"nope"}}"#);
    let r = s.read_line();
    assert_eq!(r["id"], 77);
    assert_eq!(r["result"]["isError"], true);
    assert!(Server::text_of(&r["result"]).contains("unknown tool `nope`"));
    assert_eq!(s.request("ping", json!({}))["result"], json!({}));
    let (code, stderr, text) = s.finish();
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stderr.is_empty(), "{stderr}");
    // The trace under MUSE_PLUGIN_DATA_DIR (read by `finish` while the
    // sandbox was alive): one line per frame.
    assert!(text.contains("\tEV=spawn\t"), "{text}");
    assert!(text.contains("\"method\":\"initialize\""), "{text}");
    assert!(text.contains("\tEV=eof\t"), "{text}");
    assert!(
        !text.contains("omm_doctor\",\"description"),
        "no result bodies in the trace: {text}"
    );
}

#[test]
fn a_tool_call_without_a_reachable_host_is_a_readable_error_result() {
    let empty = tempfile::tempdir().unwrap();
    // PATH holds nothing, HOME has no ~/.local/bin/.muse-version, no OMM_MUSE_BIN.
    let mut s = Server::spawn(None, Some(empty.path()));
    s.handshake();
    let r = s.call("omm_doctor", json!({"fast": true}));
    assert_eq!(r["isError"], true, "{r}");
    let text = Server::text_of(&r);
    assert!(text.starts_with("error: "), "{text}");
    assert!(text.contains("scrubbed environment"), "{text}");
    assert!(text.contains(".muse-version"), "{text}");
    let (code, _, _) = s.finish();
    assert_eq!(code, 0);
}

#[test]
fn omm_doctor_returns_the_doctor_json_and_writes_nothing_under_the_roots() {
    let Some(host) = host_bin() else {
        eprintln!("skipped: OMM_MUSE_BIN is unset");
        return;
    };
    let mut s = Server::spawn(Some(&host), None);
    s.handshake();
    let before = snapshot(&s.sb.config_home);
    let r = s.call("omm_doctor", json!({"fast": true}));
    assert_eq!(r["isError"], false, "{r}");
    let doc: Value = serde_json::from_str(Server::text_of(&r)).expect("doctor JSON");
    let ids: Vec<&str> = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids.len(), 15, "{ids:?}");
    assert_eq!(ids[14], "D15");
    assert_eq!(doc["host"]["plugin_id"], "oh-my-musecode");
    assert_eq!(
        doc["host"]["config_root"],
        json!(s.sb.config_home.join("muse"))
    );
    assert_eq!(
        doc["host"]["workspace"],
        json!(std::fs::canonicalize(&s.ws).unwrap())
    );
    // Nothing installed in the sandbox: D1 is the critical row, D15 has nothing to spawn.
    let d1 = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "D1")
        .unwrap();
    assert_eq!(d1["severity"], "critical");
    let d15 = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "D15")
        .unwrap();
    assert_eq!(d15["severity"], "info", "{d15}");
    assert_eq!(doc["exit_code"], 1);
    // `--fast` semantics: D8 and D11 say they were skipped.
    let d8 = doc["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "D8")
        .unwrap();
    assert!(
        d8["observed"]
            .as_str()
            .unwrap()
            .contains("disabled by options"),
        "{d8}"
    );
    // The argument shape is enforced.
    let bad = s.call("omm_doctor", json!({"fast": "yes"}));
    assert_eq!(bad["isError"], true);
    assert!(Server::text_of(&bad).contains("must be a boolean"));
    let bad = s.call("omm_cost", json!({"cuts": false}));
    assert_eq!(bad["isError"], true);
    assert!(Server::text_of(&bad).contains("takes no arguments"));
    // The doctor wrote nothing under the config root (the host's own startup
    // locks excepted — `snapshot` leaves them out); the server's own trace
    // is the only thing under the data root's plugin data dir.
    assert_eq!(snapshot(&s.sb.config_home), before);
    assert!(!s.sb.config_home.join("omm").exists());
    let (code, _, text) = s.finish();
    assert_eq!(code, 0);
    assert!(
        text.contains("\"name\":\"omm_doctor\",\"arguments\":{\"fast\":true}"),
        "{text}"
    );
    assert!(text.contains("\tEV=tool\t"), "{text}");
}

/// The host's two startup locks under the config root, written by the host
/// itself on every verb (read-only ones included) — host-owned residue, not
/// an omm write; the same names e2e's `host_owned_residue` and
/// INSTALL_FOR_AGENTS §5 exclude.
const HOST_LOCKS: [&str; 2] = ["muse/.auth.json.lock", "muse/.settings.json.lock"];

/// `{relative path → size}` of every file under `root`, [`HOST_LOCKS`]
/// left out.
fn snapshot(root: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for entry in walkdir::WalkDir::new(root).into_iter().flatten() {
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .unwrap()
                .display()
                .to_string();
            if HOST_LOCKS.contains(&rel.as_str()) {
                continue;
            }
            out.push((rel, entry.metadata().map(|m| m.len()).unwrap_or(0)));
        }
    }
    out.sort();
    out
}

#[test]
fn the_host_drives_omm_mcp_through_the_mock_provider_harness() {
    let Some(host) = host_bin() else {
        eprintln!("skipped: OMM_MUSE_BIN is unset");
        return;
    };
    let python = ["python3", "python"].iter().find(|p| {
        Command::new(p)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });
    let Some(_) = python else {
        eprintln!("skipped: no python3 on PATH");
        return;
    };
    let work = tempfile::Builder::new()
        .prefix("omm-mcp-harness-")
        .tempdir()
        .expect("temp dir");
    let script = repo_root().join("tools/mockprovider/run-omm-mcp.sh");
    let out = Command::new("bash")
        .arg(&script)
        .arg(work.path().join("w"))
        .env("OMM_MUSE_BIN", &host)
        .env("OMM_BIN", env!("CARGO_BIN_EXE_omm"))
        .env("MOCK_PORT", "8746")
        .env("TIMEOUT_S", "180")
        .stdin(Stdio::null())
        .output()
        .expect("run the harness");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "run-omm-mcp.sh failed\n--- stdout\n{stdout}\n--- stderr\n{stderr}"
    );
    for proof in ["[PASS] P1", "[PASS] P2", "[PASS] P3"] {
        assert!(stdout.contains(proof), "{proof} missing:\n{stdout}");
    }
    assert!(
        stdout.contains("doc server trusted_enabled and command [omm, mcp]: yes"),
        "{stdout}"
    );
    assert!(stdout.contains("MOCK-FINAL-ANSWER"), "{stdout}");
}
