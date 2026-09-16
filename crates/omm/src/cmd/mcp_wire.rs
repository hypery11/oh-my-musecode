//! The wire layer of `omm mcp` (PLAN.md 2.2): stdio framing and JSON-RPC 2.0
//! envelopes, pure and bounded, so the server loop in [`super::mcp`] never
//! panics on what the pipe delivers.
//!
//! Framing (measured, `docs/experiments/mcp-tools-call.md` §2.1 and
//! `tools/mockprovider/mcp_echo_server.py`): the host's MCP client speaks
//! **line-delimited JSON** — one JSON object per line, `\n`-terminated. The
//! LSP-style `Content-Length: N\r\n\r\n<body>` framing other hosts use is
//! accepted too, and every reply mirrors the framing of the frame it answers
//! ([`Frame::framing`]), so a client that chose one convention never sees the
//! other. A frame is bounded at [`MAX_FRAME_BYTES`]: a longer line or a larger
//! declared body is drained and reported as [`ReadError::TooLarge`] — the
//! caller answers with a JSON-RPC error and keeps serving, never panics and
//! never buffers past the bound.
//!
//! Envelopes: [`parse`] turns the bytes of one frame into an [`Incoming`] —
//! a request (has `id`), a notification (no `id`), or the JSON-RPC error the
//! frame earns (parse error `-32700`, invalid request `-32600`). Batches
//! (arrays) are refused as invalid: the host never sends one, and a
//! single-threaded stdio server has nothing to gain from them.

use std::io::{self, BufRead, Read, Write};

use serde_json::{json, Map, Value};

/// The most one frame may carry, either as a line or as a declared body
/// (1 MiB). The host's largest frame is a `tools/call` with the model's
/// arguments; ours are two small objects.
pub const MAX_FRAME_BYTES: usize = 1 << 20;

/// The most header lines a `Content-Length` block may carry before the
/// blank line; past it the block is malformed.
const MAX_HEADER_LINES: usize = 32;

/// JSON-RPC 2.0 error codes (the spec's reserved range).
pub const PARSE_ERROR: i64 = -32700;
/// See [`PARSE_ERROR`].
pub const INVALID_REQUEST: i64 = -32600;
/// See [`PARSE_ERROR`].
pub const METHOD_NOT_FOUND: i64 = -32601;
/// See [`PARSE_ERROR`].
pub const INVALID_PARAMS: i64 = -32602;
/// See [`PARSE_ERROR`].
pub const INTERNAL_ERROR: i64 = -32603;

/// How a frame was delimited on the pipe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framing {
    /// One JSON document per `\n`-terminated line (the host's convention).
    Line,
    /// `Content-Length: N\r\n\r\n` followed by exactly `N` bytes.
    ContentLength,
}

/// One frame read from the pipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub body: Vec<u8>,
    pub framing: Framing,
}

/// Why [`read_frame`] returned no frame.
#[derive(Debug)]
pub enum ReadError {
    /// The peer closed the pipe (or a body ended early): the server exits 0.
    Eof,
    /// The pipe failed: the server exits 0 (there is nobody left to tell).
    Io(io::Error),
    /// A frame past [`MAX_FRAME_BYTES`]; the excess was drained and the next
    /// frame can be read. `len` is the declared body size, or the bound plus
    /// one for a line that never ended within it.
    TooLarge { framing: Framing, len: usize },
    /// A `Content-Length` block that does not parse; the offending line is
    /// consumed and reading resumes at the next line.
    BadHeader(String),
}

impl ReadError {
    /// The framing a reply to this failure should use, when there is one.
    pub fn framing(&self) -> Framing {
        match self {
            ReadError::TooLarge { framing, .. } => *framing,
            ReadError::BadHeader(_) => Framing::ContentLength,
            ReadError::Eof | ReadError::Io(_) => Framing::Line,
        }
    }
    /// The one-line reason for the JSON-RPC error a recoverable failure earns.
    pub fn message(&self) -> String {
        match self {
            ReadError::TooLarge { len, .. } => format!(
                "frame of {len} bytes exceeds the {MAX_FRAME_BYTES}-byte bound; it was discarded"
            ),
            ReadError::BadHeader(detail) => format!("malformed Content-Length framing: {detail}"),
            ReadError::Eof => "end of input".to_string(),
            ReadError::Io(e) => format!("read failed: {e}"),
        }
    }
}

/// Read one line of at most `MAX_FRAME_BYTES` bytes (the newline included
/// when present). `Ok(None)` at a clean EOF; `Err(TooLarge)` after draining
/// the rest of an oversized line.
fn read_bounded_line<R: BufRead>(
    r: &mut R,
    framing: Framing,
) -> Result<Option<Vec<u8>>, ReadError> {
    let mut buf = Vec::new();
    let bound = MAX_FRAME_BYTES as u64 + 1;
    let n = r
        .by_ref()
        .take(bound)
        .read_until(b'\n', &mut buf)
        .map_err(ReadError::Io)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.last() != Some(&b'\n') && buf.len() as u64 >= bound {
        // Past the bound without a newline: drain to the end of the line.
        let mut sink = Vec::new();
        loop {
            sink.clear();
            let m = r
                .by_ref()
                .take(bound)
                .read_until(b'\n', &mut sink)
                .map_err(ReadError::Io)?;
            if m == 0 || sink.last() == Some(&b'\n') {
                break;
            }
        }
        return Err(ReadError::TooLarge {
            framing,
            len: MAX_FRAME_BYTES + 1,
        });
    }
    Ok(Some(buf))
}

/// Trim the line terminator and surrounding ASCII whitespace.
fn trimmed(line: &[u8]) -> &[u8] {
    let mut s = line;
    while let Some((&last, rest)) = s.split_last() {
        if last.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    while let Some((&first, rest)) = s.split_first() {
        if first.is_ascii_whitespace() {
            s = rest;
        } else {
            break;
        }
    }
    s
}

/// The declared length when `line` is a `Content-Length:` header
/// (case-insensitive), else `None`; a header with a bad number is an error.
fn content_length(line: &[u8]) -> Option<Result<usize, String>> {
    const NAME: &[u8] = b"content-length:";
    if line.len() < NAME.len() || !line[..NAME.len()].eq_ignore_ascii_case(NAME) {
        return None;
    }
    let value = String::from_utf8_lossy(trimmed(&line[NAME.len()..])).into_owned();
    Some(
        value
            .parse::<usize>()
            .map_err(|_| format!("Content-Length value `{value}` is not a byte count")),
    )
}

/// Discard exactly `n` bytes (or until EOF).
fn drain<R: Read>(r: &mut R, n: usize) -> Result<(), ReadError> {
    io::copy(&mut r.by_ref().take(n as u64), &mut io::sink())
        .map(|_| ())
        .map_err(ReadError::Io)
}

/// Read the next frame: blank lines are skipped; a `Content-Length` header
/// starts a header block (further headers ignored, up to
/// [`MAX_HEADER_LINES`]) that ends at the first blank line and is followed
/// by the declared body; anything else is a line frame.
pub fn read_frame<R: BufRead>(r: &mut R) -> Result<Frame, ReadError> {
    loop {
        let Some(line) = read_bounded_line(r, Framing::Line)? else {
            return Err(ReadError::Eof);
        };
        let body = trimmed(&line);
        if body.is_empty() {
            continue;
        }
        let Some(declared) = content_length(body) else {
            return Ok(Frame {
                body: body.to_vec(),
                framing: Framing::Line,
            });
        };
        let len = declared.map_err(ReadError::BadHeader)?;
        // The rest of the header block, up to the blank line.
        let mut headers = 0usize;
        loop {
            let Some(h) = read_bounded_line(r, Framing::ContentLength)? else {
                return Err(ReadError::Eof);
            };
            if trimmed(&h).is_empty() {
                break;
            }
            headers += 1;
            if headers > MAX_HEADER_LINES {
                return Err(ReadError::BadHeader(format!(
                    "more than {MAX_HEADER_LINES} header lines before the blank line"
                )));
            }
        }
        if len > MAX_FRAME_BYTES {
            drain(r, len)?;
            return Err(ReadError::TooLarge {
                framing: Framing::ContentLength,
                len,
            });
        }
        let mut body = vec![0u8; len];
        match r.read_exact(&mut body) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(ReadError::Eof),
            Err(e) => return Err(ReadError::Io(e)),
        }
        return Ok(Frame {
            body,
            framing: Framing::ContentLength,
        });
    }
}

/// Write one frame in `framing` and flush. A compact `serde_json` document
/// never carries a raw newline, so the line form is always one line.
pub fn write_frame<W: Write>(w: &mut W, body: &[u8], framing: Framing) -> io::Result<()> {
    match framing {
        Framing::Line => {
            w.write_all(body)?;
            w.write_all(b"\n")?;
        }
        Framing::ContentLength => {
            write!(w, "Content-Length: {}\r\n\r\n", body.len())?;
            w.write_all(body)?;
        }
    }
    w.flush()
}

/// One decoded frame.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// A request: the peer expects exactly one reply carrying `id`.
    Request {
        id: Value,
        method: String,
        params: Option<Value>,
    },
    /// A notification: no reply, ever.
    Notification {
        method: String,
        params: Option<Value>,
    },
    /// The frame is not a JSON-RPC 2.0 request; the reply carries `id`
    /// (the frame's own when it had a usable one, else `null`).
    Invalid {
        id: Value,
        code: i64,
        message: String,
    },
}

/// Decode the bytes of one frame.
pub fn parse(body: &[u8]) -> Incoming {
    let value: Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => {
            return Incoming::Invalid {
                id: Value::Null,
                code: PARSE_ERROR,
                message: format!("parse error: {e}"),
            }
        }
    };
    let Value::Object(obj) = value else {
        let what = match value {
            Value::Array(_) => "a batch (array) — batches are not supported",
            _ => "not a JSON object",
        };
        return Incoming::Invalid {
            id: Value::Null,
            code: INVALID_REQUEST,
            message: format!("invalid request: {what}"),
        };
    };
    let id = match obj.get("id") {
        None => None,
        Some(v @ (Value::String(_) | Value::Number(_) | Value::Null)) => Some(v.clone()),
        Some(_) => {
            return Incoming::Invalid {
                id: Value::Null,
                code: INVALID_REQUEST,
                message: "invalid request: `id` must be a string, a number or null".to_string(),
            }
        }
    };
    let reply_id = id.clone().unwrap_or(Value::Null);
    if obj.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Incoming::Invalid {
            id: reply_id,
            code: INVALID_REQUEST,
            message: "invalid request: `jsonrpc` must be \"2.0\"".to_string(),
        };
    }
    let method = match obj.get("method").and_then(Value::as_str) {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => {
            return Incoming::Invalid {
                id: reply_id,
                code: INVALID_REQUEST,
                message: "invalid request: `method` must be a non-empty string".to_string(),
            }
        }
    };
    let params = match obj.get("params") {
        None | Some(Value::Null) => None,
        Some(p @ (Value::Object(_) | Value::Array(_))) => Some(p.clone()),
        Some(_) => {
            return Incoming::Invalid {
                id: reply_id,
                code: INVALID_REQUEST,
                message: "invalid request: `params` must be an object or an array".to_string(),
            }
        }
    };
    match id {
        Some(id) => Incoming::Request { id, method, params },
        None => Incoming::Notification { method, params },
    }
}

/// A success reply.
pub fn result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// An error reply (`data` only when given).
pub fn error(id: &Value, code: i64, message: impl Into<String>, data: Option<Value>) -> Value {
    let mut err = Map::new();
    err.insert("code".into(), json!(code));
    err.insert("message".into(), Value::String(message.into()));
    if let Some(d) = data {
        err.insert("data".into(), d);
    }
    json!({ "jsonrpc": "2.0", "id": id, "error": Value::Object(err) })
}

/// The `params` of a request as an object (an absent `params` is empty).
pub fn params_object(params: Option<&Value>) -> Result<Map<String, Value>, String> {
    match params {
        None => Ok(Map::new()),
        Some(Value::Object(m)) => Ok(m.clone()),
        Some(_) => Err("params must be an object".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn frames(input: &[u8]) -> Vec<Result<Frame, String>> {
        let mut r = Cursor::new(input.to_vec());
        let mut out = Vec::new();
        loop {
            match read_frame(&mut r) {
                Ok(f) => out.push(Ok(f)),
                Err(ReadError::Eof) => break,
                Err(e) => out.push(Err(e.message())),
            }
        }
        out
    }

    #[test]
    fn line_frames_skip_blank_lines_and_take_the_last_unterminated_line() {
        let got = frames(b"\n{\"a\":1}\r\n\n  {\"b\":2}  \n{\"c\":3}");
        let bodies: Vec<Vec<u8>> = got.into_iter().map(|f| f.unwrap().body).collect();
        assert_eq!(
            bodies,
            vec![
                b"{\"a\":1}".to_vec(),
                b"{\"b\":2}".to_vec(),
                b"{\"c\":3}".to_vec()
            ]
        );
    }

    #[test]
    fn content_length_frames_are_read_exactly_and_mirrored() {
        let body = b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}";
        let mut input = Vec::new();
        input.extend_from_slice(
            format!(
                "content-length: {}\r\nContent-Type: application/json\r\n\r\n",
                body.len()
            )
            .as_bytes(),
        );
        input.extend_from_slice(body);
        // A line frame right after, with no separator.
        input.extend_from_slice(b"{\"x\":1}\n");
        let got = frames(&input);
        assert_eq!(got.len(), 2, "{got:?}");
        let first = got[0].as_ref().unwrap();
        assert_eq!(first.framing, Framing::ContentLength);
        assert_eq!(first.body, body.to_vec());
        let second = got[1].as_ref().unwrap();
        assert_eq!(second.framing, Framing::Line);
        assert_eq!(second.body, b"{\"x\":1}".to_vec());
        // Mirroring.
        let mut w = Vec::new();
        write_frame(&mut w, b"{}", Framing::ContentLength).unwrap();
        assert_eq!(w, b"Content-Length: 2\r\n\r\n{}".to_vec());
        let mut w = Vec::new();
        write_frame(&mut w, b"{}", Framing::Line).unwrap();
        assert_eq!(w, b"{}\n".to_vec());
    }

    #[test]
    fn a_truncated_content_length_body_is_eof() {
        let mut r = Cursor::new(b"Content-Length: 10\r\n\r\n{}".to_vec());
        assert!(matches!(read_frame(&mut r), Err(ReadError::Eof)));
    }

    #[test]
    fn bad_headers_are_reported_and_reading_resumes() {
        let got = frames(b"Content-Length: abc\r\n{\"ok\":1}\n");
        assert_eq!(got.len(), 2, "{got:?}");
        assert!(got[0].as_ref().unwrap_err().contains("not a byte count"));
        assert_eq!(got[1].as_ref().unwrap().body, b"{\"ok\":1}".to_vec());
        // Too many header lines.
        let mut input = b"Content-Length: 2\r\n".to_vec();
        for i in 0..40 {
            input.extend_from_slice(format!("X-{i}: y\r\n").as_bytes());
        }
        input.extend_from_slice(b"\r\n{}");
        let got = frames(&input);
        assert!(
            got[0].as_ref().unwrap_err().contains("header lines"),
            "{got:?}"
        );
    }

    #[test]
    fn oversized_frames_are_drained_and_the_next_frame_is_read() {
        // A line past the bound.
        let mut input = vec![b'x'; MAX_FRAME_BYTES + 5];
        input.push(b'\n');
        input.extend_from_slice(b"{\"after\":1}\n");
        let got = frames(&input);
        assert_eq!(got.len(), 2, "{}", got.len());
        assert!(got[0].as_ref().unwrap_err().contains("exceeds"), "{got:?}");
        assert_eq!(got[1].as_ref().unwrap().body, b"{\"after\":1}".to_vec());
        // A declared body past the bound.
        let big = MAX_FRAME_BYTES + 1;
        let mut input = format!("Content-Length: {big}\r\n\r\n").into_bytes();
        input.extend(std::iter::repeat_n(b'y', big));
        input.extend_from_slice(b"{\"after\":2}\n");
        let got = frames(&input);
        assert_eq!(got.len(), 2, "{}", got.len());
        match &got[0] {
            Err(m) => assert!(m.contains(&big.to_string()), "{m}"),
            Ok(f) => panic!("expected TooLarge, got {f:?}"),
        }
        assert_eq!(got[1].as_ref().unwrap().body, b"{\"after\":2}".to_vec());
        // Exactly at the bound is fine.
        let mut input = format!("Content-Length: {MAX_FRAME_BYTES}\r\n\r\n").into_bytes();
        input.extend(std::iter::repeat_n(b'z', MAX_FRAME_BYTES));
        let got = frames(&input);
        assert_eq!(got[0].as_ref().unwrap().body.len(), MAX_FRAME_BYTES);
    }

    #[test]
    fn parse_classifies_requests_notifications_and_junk() {
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"x"}}"#),
            Incoming::Request {
                id: json!(3),
                method: "tools/call".into(),
                params: Some(json!({"name": "x"})),
            }
        );
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#),
            Incoming::Notification {
                method: "notifications/initialized".into(),
                params: None,
            }
        );
        assert_eq!(
            parse(br#"{"jsonrpc":"2.0","id":"s","method":"ping","params":null}"#),
            Incoming::Request {
                id: json!("s"),
                method: "ping".into(),
                params: None,
            }
        );
        match parse(b"{not json") {
            Incoming::Invalid { id, code, message } => {
                assert_eq!(id, Value::Null);
                assert_eq!(code, PARSE_ERROR);
                assert!(message.starts_with("parse error"));
            }
            other => panic!("{other:?}"),
        }
        match parse(b"[]") {
            Incoming::Invalid { code, message, .. } => {
                assert_eq!(code, INVALID_REQUEST);
                assert!(message.contains("batch"));
            }
            other => panic!("{other:?}"),
        }
        match parse(br#"{"id":7,"method":"ping"}"#) {
            Incoming::Invalid { id, code, message } => {
                assert_eq!(id, json!(7), "the frame's id is echoed");
                assert_eq!(code, INVALID_REQUEST);
                assert!(message.contains("jsonrpc"));
            }
            other => panic!("{other:?}"),
        }
        for junk in [
            br#"{"jsonrpc":"2.0","id":1}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":1,"method":""}"#,
            br#"{"jsonrpc":"2.0","id":{"a":1},"method":"ping"}"#,
            br#"{"jsonrpc":"2.0","id":1,"method":"ping","params":5}"#,
            b"42",
        ] {
            assert!(
                matches!(
                    parse(junk),
                    Incoming::Invalid {
                        code: INVALID_REQUEST,
                        ..
                    }
                ),
                "{}",
                String::from_utf8_lossy(junk)
            );
        }
    }

    #[test]
    fn envelopes_and_params_helpers() {
        let r = result(&json!(1), json!({"ok": true}));
        assert_eq!(
            r,
            json!({"jsonrpc": "2.0", "id": 1, "result": {"ok": true}})
        );
        let e = error(&Value::Null, METHOD_NOT_FOUND, "nope", None);
        assert_eq!(
            e,
            json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32601, "message": "nope"}})
        );
        let e = error(
            &json!("x"),
            INVALID_PARAMS,
            "bad",
            Some(json!({"field": "name"})),
        );
        assert_eq!(e["error"]["data"], json!({"field": "name"}));
        assert!(params_object(None).unwrap().is_empty());
        assert_eq!(params_object(Some(&json!({"a": 1}))).unwrap().len(), 1);
        assert!(params_object(Some(&json!([1]))).is_err());
        // Deeply nested input is refused by the parser's recursion limit, not a stack overflow.
        let deep = format!("{}{}", "[".repeat(100_000), "]".repeat(100_000));
        assert!(matches!(parse(deep.as_bytes()), Incoming::Invalid { .. }));
    }
}
