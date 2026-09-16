//! `omm mcp` — the stdio MCP server inside the omm binary (PLAN.md 2.2;
//! ARCHITECTURE.md §7). It exposes two tools, `omm_doctor` and `omm_cost`,
//! whose results are the `--json` documents of `omm doctor` and `omm cost`,
//! and it dogfoods the tools story: the bundle declares it as
//! `content/mcp/doc.json` → `capabilities.mcpServers[{id:"doc",
//! command:["omm","mcp"]}]`. The host sanitizes `-` to `_` on the wire, so
//! the namespace the model sees is `mcp__plugin_oh_my_musecode_doc`
//! (`len("oh-my-musecode") + len("doc") = 17 ≤ 18`, R19), and the model
//! calls `mcp__plugin_oh_my_musecode_doc.omm_doctor` (or the `__` form — both
//! dispatch while the namespace is unrewritten,
//! `docs/experiments/mcp-tools-call.md` §3).
//!
//! What the host does when it spawns this (measured, `research/experiments/
//! plugin-mcp.md` §4, re-measured 2026-09-02 with a bare `["ommprobe","mcp"]`
//! declaration: `command[1]` that names no package file is handed over
//! verbatim, `source_relative_path: null`):
//!
//! * `command[0]` — `omm` — is resolved through the **`PATH` the host
//!   inherited**, with no other hint. Nothing says so when it does not
//!   resolve: no stderr, no session record, the namespace is simply absent
//!   from `tools[]`. Doctor D15 exists for exactly that.
//! * the child environment is the 16-key scrub: `HOME LANG LOGNAME PATH SHELL
//!   TERM TMPDIR USER __CF_USER_TEXT_ENCODING` plus the seven plugin
//!   variables (`MUSE_PLUGIN_DATA_DIR MUSE_PLUGIN_ID MUSE_PLUGIN_ROOT` and
//!   their `CLAUDE_`/bare twins). **No `XDG_*`, no `OMM_MUSE_BIN`, no
//!   `MUSE_INSTALL_DIR`.** The server therefore resolves its roots from
//!   `HOME` exactly as the CLI does without those variables
//!   (`Roots::from_env` → `~/.config/muse`, `~/.local/share/muse`), and the
//!   Muse binary through the same `omm_host::locate` search the CLI uses —
//!   of which only the last two rungs can fire here: the launcher's install
//!   dir (`~/.local/bin/.muse-version` → `muse-bin-<version>`) and `muse` on
//!   `PATH`. A user whose Muse lives elsewhere and who relies on
//!   `OMM_MUSE_BIN` for the CLI gets a tool result saying so (`isError`).
//! * cwd is the **workspace root**, not the plugin root — which is what
//!   `omm doctor` wants (D12 checks the cwd's trust).
//! * `MUSE_PLUGIN_DATA_DIR` (`$XDG_DATA_HOME/muse/plugins/data/omm`) is
//!   advertised but **not created**; the server `mkdir -p`s it and keeps a
//!   small, bounded trace there ([`TRACE_FILE`]: one line per frame —
//!   method, id, tool name and arguments, reply size — never a result body),
//!   so "the server never started" and "the server answered" are both
//!   visible afterwards. The directory is the host's per-plugin data dir:
//!   `omm uninstall` runs `muse plugins remove omm --delete-data`, which
//!   takes it away, and the R5 snapshot excludes `plugins/` as host-owned.
//!
//! Wire contract (measured; `mcp_wire`): line-delimited JSON-RPC 2.0 —
//! `Content-Length` framing accepted and mirrored — protocol version
//! [`PROTOCOL_VERSION`], client `tbh`; `initialize` → `notifications/
//! initialized` → `tools/list` → `tools/call` → `ping`; a tool result is one
//! `{"type":"text"}` part forwarded to the model byte for byte as the
//! `function_call_output`, so errors are spelled out in the text and flagged
//! `isError` (the flag's own rendering is unmeasured, mcp-tools-call.md §5.3).
//! Frames are bounded (1 MiB), malformed input is answered with a JSON-RPC
//! error and never a panic, EOF exits 0.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Args;
use serde_json::{json, Map, Value};

use omm_doctor::cost::{self, CostOptions};

use crate::cmd::mcp_wire::{self as wire, Framing, Incoming, ReadError};
use crate::cmd::{inspect, Ctx, OMM_VERSION};
use crate::error::Result;

/// The MCP protocol version the host speaks (`mcp-tools-call.md` §2.1).
pub const PROTOCOL_VERSION: &str = "2024-11-05";
/// `serverInfo.name`.
pub const SERVER_NAME: &str = "omm";
/// The doctor tool: `{fast?: bool}` → the `omm doctor --json` document.
pub const TOOL_DOCTOR: &str = "omm_doctor";
/// The cost tool: no arguments → the `omm cost --json` document.
pub const TOOL_COST: &str = "omm_cost";
/// The host's per-plugin data directory, advertised to the server and not
/// created (`research/experiments/plugin-mcp.md` §4.1; host-reality "Paths").
pub const ENV_PLUGIN_DATA_DIR: &str = "MUSE_PLUGIN_DATA_DIR";
/// The plugin id the host advertises beside it.
pub const ENV_PLUGIN_ID: &str = "MUSE_PLUGIN_ID";
/// The trace file under the data dir (named for the command, never for the
/// server id — R8).
pub const TRACE_FILE: &str = "omm-mcp.log";
/// The trace is truncated when it grows past this (1 MiB).
pub const TRACE_MAX_BYTES: u64 = 1 << 20;

/// `omm mcp`. No flags of its own: the manifest's command is exactly
/// `["omm","mcp"]` and the host passes nothing else. The global `--verbose`
/// mirrors the trace to stderr.
#[derive(Args, Clone, Debug, Default, PartialEq, Eq)]
pub struct McpArgs {}

/// Serve stdin → stdout until EOF. Always exit 0: a closed pipe means the
/// host went away, and there is nobody left to report to.
pub fn mcp(ctx: &Ctx, _args: &McpArgs) -> Result<ExitCode> {
    let data_dir = std::env::var_os(ENV_PLUGIN_DATA_DIR)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let trace = TraceLog::open(data_dir.as_deref(), ctx.verbose);
    let mut server = Server::new(ctx, trace);
    let stdin = io::stdin();
    let stdout = io::stdout();
    server.serve(&mut stdin.lock(), &mut stdout.lock());
    Ok(ExitCode::SUCCESS)
}

/// The `tools/list` result: the two tools with their input schemas. Kept
/// short — every byte of a tool description is on the wire in every request.
pub fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": TOOL_DOCTOR,
                "description": "Run omm's health checks (omm doctor) and return the JSON report: one row per check with id, severity (info|warn|critical), observed, why_silent and the exact fix command; exit_code 1 means a critical row. fast=true skips the two slow probes (the live echo session and the host self-test).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "fast": {
                            "type": "boolean",
                            "description": "Skip the live echo session (D8) and the host self-test (D11). Default false."
                        }
                    },
                    "additionalProperties": false
                }
            },
            {
                "name": TOOL_COST,
                "description": "Measure the context cost of this installation (omm cost) with one live echo session and return the JSON report: skills catalog bytes per source, the refundable built-in tax, memory, rules, the workflow cookbook, a bytes/4 token estimate and the settings cuts that save the most.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        ]
    })
}

/// `omm_doctor`'s arguments: `{fast?: bool}` and nothing else.
pub fn doctor_args(args: &Map<String, Value>) -> std::result::Result<bool, String> {
    let mut fast = false;
    for (k, v) in args {
        match (k.as_str(), v) {
            ("fast", Value::Bool(b)) => fast = *b,
            ("fast", other) => {
                return Err(format!(
                    "argument `fast` must be a boolean, got {other}; {TOOL_DOCTOR} accepts {{\"fast\"?: boolean}}"
                ))
            }
            (other, _) => {
                return Err(format!(
                    "unknown argument `{other}`; {TOOL_DOCTOR} accepts {{\"fast\"?: boolean}}"
                ))
            }
        }
    }
    Ok(fast)
}

/// `omm_cost`'s arguments: none.
pub fn cost_args(args: &Map<String, Value>) -> std::result::Result<(), String> {
    match args.keys().next() {
        None => Ok(()),
        Some(k) => Err(format!(
            "unknown argument `{k}`; {TOOL_COST} takes no arguments"
        )),
    }
}

/// What a tool call produced: the text the model receives, verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolOutcome {
    pub text: String,
    pub is_error: bool,
}

impl ToolOutcome {
    fn ok(text: String) -> ToolOutcome {
        ToolOutcome {
            text,
            is_error: false,
        }
    }
    fn err(text: impl Into<String>) -> ToolOutcome {
        ToolOutcome {
            text: text.into(),
            is_error: true,
        }
    }
    /// The `tools/call` result: exactly one text part (mcp-tools-call.md §6.5).
    pub fn to_result(&self) -> Value {
        json!({
            "content": [{ "type": "text", "text": self.text }],
            "isError": self.is_error,
        })
    }
}

/// The server: the CLI context (roots from the scrubbed environment, the
/// host located lazily on the first tool call) and the trace.
pub struct Server<'a> {
    ctx: &'a Ctx,
    trace: TraceLog,
}

impl std::fmt::Debug for Server<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("trace", &self.trace)
            .finish_non_exhaustive()
    }
}

impl<'a> Server<'a> {
    pub fn new(ctx: &'a Ctx, trace: TraceLog) -> Server<'a> {
        Server { ctx, trace }
    }

    /// The read → dispatch → write loop. Returns at EOF or when the pipe
    /// fails in either direction.
    pub fn serve<R: BufRead, W: Write>(&mut self, r: &mut R, w: &mut W) {
        self.trace.event(
            "spawn",
            json!({
                "omm_version": OMM_VERSION,
                "cwd": std::env::current_dir().ok(),
                "plugin_id": std::env::var(ENV_PLUGIN_ID).ok(),
                "argv": std::env::args().collect::<Vec<_>>(),
            }),
        );
        loop {
            let frame = match wire::read_frame(r) {
                Ok(f) => f,
                Err(ReadError::Eof) => {
                    self.trace.event("eof", json!({}));
                    return;
                }
                Err(ReadError::Io(e)) => {
                    self.trace
                        .event("read-error", json!({ "error": e.to_string() }));
                    return;
                }
                Err(e) => {
                    self.trace
                        .event("dropped", json!({ "reason": e.message() }));
                    let reply = wire::error(&Value::Null, wire::INVALID_REQUEST, e.message(), None);
                    if self.send(w, &reply, e.framing()).is_err() {
                        return;
                    }
                    continue;
                }
            };
            let reply = match wire::parse(&frame.body) {
                Incoming::Invalid { id, code, message } => {
                    self.trace
                        .event("invalid", json!({ "code": code, "message": message }));
                    Some(wire::error(&id, code, message, None))
                }
                Incoming::Notification { method, .. } => {
                    self.trace.event("notify", json!({ "method": method }));
                    None
                }
                Incoming::Request { id, method, params } => {
                    Some(self.handle(&id, &method, params.as_ref()))
                }
            };
            if let Some(reply) = reply {
                if self.send(w, &reply, frame.framing).is_err() {
                    return;
                }
            }
        }
    }

    /// Answer one request with a JSON-RPC envelope.
    pub fn handle(&mut self, id: &Value, method: &str, params: Option<&Value>) -> Value {
        match method {
            "initialize" => {
                let client_version = params
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(Value::as_str);
                let client = params
                    .and_then(|p| p.get("clientInfo"))
                    .cloned()
                    .unwrap_or(Value::Null);
                self.trace.event(
                    "recv",
                    json!({ "method": method, "id": id, "protocolVersion": client_version, "clientInfo": client }),
                );
                // The version we implement; a client asking for the same gets
                // it echoed, any other gets ours and decides (MCP lifecycle).
                wire::result(
                    id,
                    json!({
                        "protocolVersion": PROTOCOL_VERSION,
                        "capabilities": { "tools": {} },
                        "serverInfo": { "name": SERVER_NAME, "version": OMM_VERSION },
                    }),
                )
            }
            "ping" => {
                self.trace
                    .event("recv", json!({ "method": method, "id": id }));
                wire::result(id, json!({}))
            }
            "tools/list" => {
                self.trace
                    .event("recv", json!({ "method": method, "id": id }));
                wire::result(id, tools_list())
            }
            "tools/call" => self.tools_call(id, params),
            // Empty families the host might enumerate although only `tools`
            // is advertised; an empty list is safer than "method not found".
            "resources/list" => {
                self.trace
                    .event("recv", json!({ "method": method, "id": id }));
                wire::result(id, json!({ "resources": [] }))
            }
            "resources/templates/list" => {
                self.trace
                    .event("recv", json!({ "method": method, "id": id }));
                wire::result(id, json!({ "resourceTemplates": [] }))
            }
            "prompts/list" => {
                self.trace
                    .event("recv", json!({ "method": method, "id": id }));
                wire::result(id, json!({ "prompts": [] }))
            }
            other => {
                self.trace.event(
                    "recv",
                    json!({ "method": other, "id": id, "unknown": true }),
                );
                wire::error(
                    id,
                    wire::METHOD_NOT_FOUND,
                    format!("method not found: {other}"),
                    None,
                )
            }
        }
    }

    fn tools_call(&mut self, id: &Value, params: Option<&Value>) -> Value {
        let params = match wire::params_object(params) {
            Ok(p) => p,
            Err(e) => {
                self.trace.event(
                    "recv",
                    json!({ "method": "tools/call", "id": id, "bad_params": e }),
                );
                return wire::error(id, wire::INVALID_PARAMS, format!("tools/call: {e}"), None);
            }
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            self.trace.event(
                "recv",
                json!({ "method": "tools/call", "id": id, "bad_params": "name" }),
            );
            return wire::error(
                id,
                wire::INVALID_PARAMS,
                "tools/call: params.name must be a string",
                None,
            );
        };
        let args = match params.get("arguments") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(m)) => m.clone(),
            Some(_) => {
                self.trace.event("recv", json!({ "method": "tools/call", "id": id, "name": name, "bad_params": "arguments" }));
                return wire::error(
                    id,
                    wire::INVALID_PARAMS,
                    "tools/call: params.arguments must be an object",
                    None,
                );
            }
        };
        self.trace.event(
            "recv",
            json!({ "method": "tools/call", "id": id, "name": name, "arguments": args }),
        );
        let outcome = self.call(name, &args);
        self.trace.event(
            "tool",
            json!({ "id": id, "name": name, "isError": outcome.is_error, "bytes": outcome.text.len() }),
        );
        wire::result(id, outcome.to_result())
    }

    /// Run one tool. Everything that can go wrong is a text the model can
    /// read, flagged `isError`; nothing here returns a JSON-RPC error.
    pub fn call(&self, name: &str, args: &Map<String, Value>) -> ToolOutcome {
        match name {
            TOOL_DOCTOR => match doctor_args(args) {
                Ok(fast) => self.run_doctor(fast),
                Err(e) => ToolOutcome::err(e),
            },
            TOOL_COST => match cost_args(args) {
                Ok(()) => self.run_cost(),
                Err(e) => ToolOutcome::err(e),
            },
            other => ToolOutcome::err(format!(
                "unknown tool `{other}`; this server has {TOOL_DOCTOR} and {TOOL_COST}"
            )),
        }
    }

    fn run_doctor(&self, fast: bool) -> ToolOutcome {
        match inspect::doctor_context(self.ctx, fast) {
            Ok(dctx) => ToolOutcome::ok(omm_doctor::run(&dctx).to_json().to_string()),
            Err(e) => ToolOutcome::err(host_error_text(&e.to_string())),
        }
    }

    fn run_cost(&self) -> ToolOutcome {
        let dctx = match inspect::doctor_context(self.ctx, false) {
            Ok(d) => d,
            Err(e) => return ToolOutcome::err(host_error_text(&e.to_string())),
        };
        match cost::measure(&dctx, &CostOptions { cuts: true }) {
            Ok(report) => ToolOutcome::ok(report.to_json().to_string()),
            Err(e) => ToolOutcome::err(format!("error: omm cost failed: {e}")),
        }
    }

    fn send<W: Write>(&mut self, w: &mut W, reply: &Value, framing: Framing) -> io::Result<()> {
        let body = reply.to_string();
        let sent = wire::write_frame(w, body.as_bytes(), framing);
        self.trace.event(
            "sent",
            json!({
                "id": reply.get("id"),
                "bytes": body.len(),
                "error": reply.get("error").and_then(|e| e.get("code")),
                "write_failed": sent.is_err(),
            }),
        );
        sent
    }
}

/// The text of a tool result when the CLI context could not be built —
/// almost always the host binary not found from the scrubbed environment.
fn host_error_text(e: &str) -> String {
    format!(
        "error: {e}. The MCP server runs with the host's scrubbed environment (HOME and PATH only, no OMM_MUSE_BIN): the Muse binary is found through ~/.local/bin/.muse-version (the launcher's install dir) or as `muse` on PATH, and the roots through HOME."
    )
}

/// The bounded trace under `MUSE_PLUGIN_DATA_DIR` (and stderr under
/// `--verbose`). Every failure here is swallowed: the trace exists for the
/// operator's benefit and must never cost a frame.
pub struct TraceLog {
    file: Option<File>,
    path: Option<PathBuf>,
    stderr: bool,
}

impl std::fmt::Debug for TraceLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TraceLog")
            .field("path", &self.path)
            .field("stderr", &self.stderr)
            .finish()
    }
}

impl TraceLog {
    /// Open (creating the directory, truncating a file past
    /// [`TRACE_MAX_BYTES`]); `None` dir → no file.
    pub fn open(dir: Option<&Path>, stderr: bool) -> TraceLog {
        let path = dir.map(|d| d.join(TRACE_FILE));
        let file = path.as_ref().and_then(|p| {
            let dir = p.parent()?;
            std::fs::create_dir_all(dir).ok()?;
            let f = OpenOptions::new().create(true).append(true).open(p).ok()?;
            if f.metadata()
                .map(|m| m.len() > TRACE_MAX_BYTES)
                .unwrap_or(false)
            {
                // A fresh start rather than an unbounded file.
                let _ = f.set_len(0);
            }
            Some(f)
        });
        TraceLog { file, path, stderr }
    }

    /// Nothing on disk, nothing on stderr.
    pub fn silent() -> TraceLog {
        TraceLog {
            file: None,
            path: None,
            stderr: false,
        }
    }

    /// Where the trace goes, when it goes anywhere.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// One line: `<epoch-ms>\tEV=<event>\tPID=<pid>\t<json>`.
    pub fn event(&mut self, ev: &str, detail: Value) {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let line = format!("{ms}\tEV={ev}\tPID={}\t{detail}\n", std::process::id());
        if self.stderr {
            eprint!("omm mcp: {line}");
        }
        if let Some(f) = self.file.as_mut() {
            if f.metadata()
                .map(|m| m.len() > TRACE_MAX_BYTES)
                .unwrap_or(false)
            {
                let _ = f.set_len(0);
            }
            let _ = f.write_all(line.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::Flags;
    use std::io::Cursor;

    fn ctx() -> (tempfile::TempDir, Ctx) {
        let tmp = tempfile::tempdir().unwrap();
        let sb = omm_host::Sandbox::create(tmp.path()).unwrap();
        let ctx = Ctx::new(sb.roots().unwrap(), Flags::default());
        (tmp, ctx)
    }

    fn replies(ctx: &Ctx, input: &[u8]) -> Vec<Value> {
        let mut server = Server::new(ctx, TraceLog::silent());
        let mut out = Vec::new();
        server.serve(&mut Cursor::new(input.to_vec()), &mut out);
        let text = String::from_utf8(out).unwrap();
        text.lines()
            .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{l}: {e}")))
            .collect()
    }

    #[test]
    fn tools_list_names_the_two_tools_with_closed_schemas() {
        let t = tools_list();
        let tools = t["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], TOOL_DOCTOR);
        assert_eq!(tools[1]["name"], TOOL_COST);
        for tool in tools {
            let schema = &tool["inputSchema"];
            assert_eq!(schema["type"], "object");
            assert_eq!(schema["additionalProperties"], false);
            assert!(
                tool["description"].as_str().unwrap().len() < 400,
                "descriptions are wire bytes every turn"
            );
        }
        assert_eq!(
            tools[0]["inputSchema"]["properties"]["fast"]["type"],
            "boolean"
        );
        assert!(tools[1]["inputSchema"]["properties"]
            .as_object()
            .unwrap()
            .is_empty());
    }

    #[test]
    fn argument_validation_is_strict_and_readable() {
        assert_eq!(doctor_args(&Map::new()), Ok(false));
        let mut m = Map::new();
        m.insert("fast".into(), json!(true));
        assert_eq!(doctor_args(&m), Ok(true));
        m.insert("fast".into(), json!("yes"));
        assert!(doctor_args(&m).unwrap_err().contains("must be a boolean"));
        let mut m = Map::new();
        m.insert("verbose".into(), json!(true));
        assert!(doctor_args(&m)
            .unwrap_err()
            .contains("unknown argument `verbose`"));
        assert_eq!(cost_args(&Map::new()), Ok(()));
        let mut m = Map::new();
        m.insert("cuts".into(), json!(false));
        assert!(cost_args(&m).unwrap_err().contains("takes no arguments"));
    }

    #[test]
    fn the_handshake_and_the_static_methods_answer_without_the_host() {
        let (_tmp, ctx) = ctx();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"tbh","version":"1.0.1"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"resources/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"prompts/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":5,"method":"resources/templates/list"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":6,"method":"nope/what"}"#,
            "\n",
        );
        let got = replies(&ctx, input.as_bytes());
        assert_eq!(
            got.len(),
            7,
            "one reply per request, none for the notification: {got:?}"
        );
        assert_eq!(got[0]["id"], 0);
        assert_eq!(got[0]["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(got[0]["result"]["capabilities"], json!({"tools": {}}));
        assert_eq!(got[0]["result"]["serverInfo"]["name"], SERVER_NAME);
        assert_eq!(got[0]["result"]["serverInfo"]["version"], OMM_VERSION);
        assert_eq!(got[1]["result"], tools_list());
        assert_eq!(got[2]["result"], json!({}));
        assert_eq!(got[3]["result"], json!({"resources": []}));
        assert_eq!(got[4]["result"], json!({"prompts": []}));
        assert_eq!(got[5]["result"], json!({"resourceTemplates": []}));
        assert_eq!(got[6]["error"]["code"], wire::METHOD_NOT_FOUND);
        assert_eq!(got[6]["id"], 6);
    }

    #[test]
    fn a_client_asking_for_another_protocol_version_gets_ours() {
        let (_tmp, ctx) = ctx();
        let got = replies(
            &ctx,
            br#"{"jsonrpc":"2.0","id":9,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}"#,
        );
        assert_eq!(got[0]["result"]["protocolVersion"], PROTOCOL_VERSION);
    }

    #[test]
    fn malformed_and_oversized_input_earns_an_error_reply_and_the_loop_goes_on() {
        let (_tmp, ctx) = ctx();
        let mut input = b"{not json\n[]\n{\"jsonrpc\":\"2.0\",\"id\":1}\n".to_vec();
        input.extend(std::iter::repeat_n(b'x', wire::MAX_FRAME_BYTES + 1));
        input.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"ping\"}\n");
        let got = replies(&ctx, &input);
        assert_eq!(got.len(), 5, "{got:?}");
        assert_eq!(got[0]["error"]["code"], wire::PARSE_ERROR);
        assert_eq!(got[0]["id"], Value::Null);
        assert_eq!(got[1]["error"]["code"], wire::INVALID_REQUEST);
        assert_eq!(got[2]["error"]["code"], wire::INVALID_REQUEST);
        assert_eq!(got[2]["id"], 1);
        assert_eq!(got[3]["error"]["code"], wire::INVALID_REQUEST);
        assert!(got[3]["error"]["message"]
            .as_str()
            .unwrap()
            .contains("exceeds"));
        assert_eq!(got[4]["result"], json!({}));
        assert_eq!(got[4]["id"], 2);
    }

    #[test]
    fn content_length_framing_is_mirrored_on_the_reply() {
        let (_tmp, ctx) = ctx();
        let body = br#"{"jsonrpc":"2.0","id":"a","method":"ping"}"#;
        let mut input = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
        input.extend_from_slice(body);
        let mut server = Server::new(&ctx, TraceLog::silent());
        let mut out = Vec::new();
        server.serve(&mut Cursor::new(input), &mut out);
        let text = String::from_utf8(out).unwrap();
        let expected = json!({"jsonrpc": "2.0", "id": "a", "result": {}}).to_string();
        assert_eq!(
            text,
            format!("Content-Length: {}\r\n\r\n{expected}", expected.len())
        );
    }

    #[test]
    fn tools_call_validates_params_then_arguments() {
        let (_tmp, ctx) = ctx();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":[1]}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"arguments":{}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"omm_doctor","arguments":"x"}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"zzz","arguments":{}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"omm_doctor","arguments":{"fast":"no"}}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"omm_cost","arguments":{"x":1}}}"#,
            "\n",
        );
        let got = replies(&ctx, input.as_bytes());
        assert_eq!(got.len(), 6, "{got:?}");
        for r in &got[..3] {
            assert_eq!(r["error"]["code"], wire::INVALID_PARAMS, "{r}");
        }
        for r in &got[3..] {
            assert_eq!(r["result"]["isError"], true, "{r}");
            assert_eq!(r["result"]["content"].as_array().unwrap().len(), 1);
            assert_eq!(r["result"]["content"][0]["type"], "text");
        }
        assert!(got[3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("unknown tool `zzz`"));
        assert!(got[4]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("must be a boolean"));
        assert!(got[5]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("takes no arguments"));
    }

    #[test]
    fn the_trace_is_created_under_the_data_dir_and_bounded() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("plugins").join("data").join("omm");
        assert!(!dir.exists(), "the host does not create it");
        let mut t = TraceLog::open(Some(&dir), false);
        t.event("spawn", json!({"k": 1}));
        let path = t.path().unwrap().to_path_buf();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\tEV=spawn\tPID="), "{text}");
        assert!(text.trim_end().ends_with("{\"k\":1}"), "{text}");
        // Past the bound the file starts over.
        drop(t);
        let f = OpenOptions::new().append(true).open(&path).unwrap();
        f.set_len(TRACE_MAX_BYTES + 10).unwrap();
        let mut t = TraceLog::open(Some(&dir), false);
        t.event("eof", json!({}));
        let len = std::fs::metadata(&path).unwrap().len();
        assert!(len < 200, "{len}");
        // No dir: no file, no failure.
        let mut none = TraceLog::open(None, false);
        none.event("x", json!({}));
        assert!(none.path().is_none());
    }
}
