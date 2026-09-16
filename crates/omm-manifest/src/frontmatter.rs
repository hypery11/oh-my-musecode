//! The SKILL.md / command front-matter loader, with the host's own rules.
//!
//! `research/musecode/skills.md` §2.2 (proven by error injection): the file
//! must start with `---`, the block must be closed by a `---` line,
//! `description` is the only required key of a skill, a duplicate key is a
//! diagnostic, a BOM is `bom_forbidden`. `docs/host-reality.md` "SKILL.md
//! frontmatter loader": surrounding double quotes are stripped but escapes are
//! NOT processed (`\t`, `\"` stay literal), and whitespace runs collapse to
//! one space — [`collapse_whitespace`] is what the catalog renders, so the
//! budget estimate uses it too.
//!
//! The grammar accepted here is the subset the host's loader and the shipped
//! content use: `key: value` scalars at column 0 and one level of nested
//! `key:` blocks (`metadata:` / `short-description:`) indented by spaces.
//! Anything else is reported rather than guessed.

use std::collections::BTreeMap;

/// A parsed front matter and the body that follows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrontMatter {
    /// Top-level scalar keys, in file order.
    pub fields: Vec<(String, String)>,
    /// Nested one-level maps (`metadata`), in file order.
    pub maps: Vec<(String, Vec<(String, String)>)>,
    /// Bytes after the closing `---` line (the skill body; §5.3 caps it).
    pub body: Vec<u8>,
    /// The file started with a UTF-8 BOM (`bom_forbidden` for the host).
    pub bom: bool,
    /// Keys that appeared twice (`duplicate_key` for the host).
    pub duplicate_keys: Vec<String>,
}

impl FrontMatter {
    /// A top-level scalar by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    /// A nested scalar, e.g. `("metadata", "short-description")`.
    pub fn get_nested(&self, map: &str, key: &str) -> Option<&str> {
        self.maps
            .iter()
            .find(|(m, _)| m == map)
            .and_then(|(_, kv)| kv.iter().find(|(k, _)| k == key))
            .map(|(_, v)| v.as_str())
    }
    /// Every top-level key, scalar or map, in file order.
    pub fn keys(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.fields.iter().map(|(k, _)| k.as_str()).collect();
        out.extend(self.maps.iter().map(|(k, _)| k.as_str()));
        out
    }
    /// The nested map by name.
    pub fn map(&self, name: &str) -> Option<&[(String, String)]> {
        self.maps
            .iter()
            .find(|(m, _)| m == name)
            .map(|(_, kv)| kv.as_slice())
    }
    /// `metadata` as a JSON-ready ordered map (for the projections).
    pub fn metadata(&self) -> BTreeMap<String, String> {
        self.map("metadata")
            .map(|kv| kv.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Re-serialize with every scalar double-quoted and escaped, so a strict
    /// YAML parser reads the same values the host's lenient loader does. The
    /// foreign-schema projections need this: their command front matter is
    /// parsed as real YAML (`Claude command frontmatter is invalid YAML`,
    /// measured on `argument-hint: [--self-test] [--report-drift]`, which a
    /// YAML parser takes for a flow sequence), whereas the native family never
    /// inspects the file. Keys, order and the body are kept verbatim.
    pub fn to_yaml_safe_bytes(&self) -> Vec<u8> {
        let mut out = String::from("---\n");
        for (k, v) in &self.fields {
            out.push_str(&format!("{k}: {}\n", yaml_quote(v)));
        }
        for (name, kv) in &self.maps {
            out.push_str(&format!("{name}:\n"));
            for (k, v) in kv {
                out.push_str(&format!("  {k}: {}\n", yaml_quote(v)));
            }
        }
        out.push_str("---\n");
        let mut bytes = out.into_bytes();
        bytes.extend_from_slice(&self.body);
        bytes
    }
}

/// A YAML double-quoted scalar: `\` and `"` escaped, control characters as
/// `\n` / `\t`.
pub fn yaml_quote(value: &str) -> String {
    let mut s = String::with_capacity(value.len() + 2);
    s.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '\n' => s.push_str("\\n"),
            '\t' => s.push_str("\\t"),
            '\r' => s.push_str("\\r"),
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

/// Why a front matter could not be parsed — the host's own words where it has them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrontMatterError {
    /// `SKILL.md must start with YAML frontmatter`.
    MissingOpen,
    /// `SKILL.md frontmatter must end with ---`.
    Unclosed,
    /// The file is not UTF-8 (`skill file at <p> is not valid UTF-8`).
    NotUtf8,
    /// A line this loader does not understand.
    Syntax { line: usize, text: String },
}

impl std::fmt::Display for FrontMatterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrontMatterError::MissingOpen => write!(f, "must start with YAML frontmatter (`---`)"),
            FrontMatterError::Unclosed => write!(f, "frontmatter must end with ---"),
            FrontMatterError::NotUtf8 => write!(f, "not valid UTF-8"),
            FrontMatterError::Syntax { line, text } => {
                write!(f, "line {line}: cannot parse frontmatter line {text:?}")
            }
        }
    }
}

const BOM: &[u8] = b"\xef\xbb\xbf";

/// Parse a Markdown file with YAML front matter.
pub fn parse(bytes: &[u8]) -> Result<FrontMatter, FrontMatterError> {
    let (bom, rest) = match bytes.strip_prefix(BOM) {
        Some(r) => (true, r),
        None => (false, bytes),
    };
    let text = std::str::from_utf8(rest).map_err(|_| FrontMatterError::NotUtf8)?;
    let mut lines = text.split_inclusive('\n');
    let first = lines.next().unwrap_or("");
    if first.trim_end_matches(['\r', '\n']) != "---" {
        return Err(FrontMatterError::MissingOpen);
    }
    let mut fm = FrontMatter {
        bom,
        ..FrontMatter::default()
    };
    let mut offset = first.len();
    let mut closed = false;
    let mut current_map: Option<String> = None;
    // Body lines are numbered from 2 (line 1 is the opening `---`).
    for (line_no, line) in (2usize..).zip(lines) {
        offset += line.len();
        let raw = line.trim_end_matches(['\r', '\n']);
        if raw == "---" {
            closed = true;
            break;
        }
        if raw.trim().is_empty() || raw.trim_start().starts_with('#') {
            continue;
        }
        let indented = raw.starts_with(' ') || raw.starts_with('\t');
        let (key, value) = split_kv(raw.trim()).ok_or_else(|| FrontMatterError::Syntax {
            line: line_no,
            text: raw.to_string(),
        })?;
        if indented {
            let Some(map) = current_map.as_ref() else {
                return Err(FrontMatterError::Syntax {
                    line: line_no,
                    text: raw.to_string(),
                });
            };
            let entry = fm.maps.iter_mut().find(|(m, _)| m == map).map(|(_, kv)| kv);
            if let Some(kv) = entry {
                if kv.iter().any(|(k, _)| k == key) {
                    fm.duplicate_keys.push(format!("{map}.{key}"));
                }
                kv.push((key.to_string(), unquote(value)));
            }
            continue;
        }
        if value.is_empty() {
            // `metadata:` opens a nested block.
            if fm.maps.iter().any(|(m, _)| m == key) || fm.get(key).is_some() {
                fm.duplicate_keys.push(key.to_string());
            }
            fm.maps.push((key.to_string(), Vec::new()));
            current_map = Some(key.to_string());
            continue;
        }
        current_map = None;
        if fm.get(key).is_some() || fm.maps.iter().any(|(m, _)| m == key) {
            fm.duplicate_keys.push(key.to_string());
        }
        fm.fields.push((key.to_string(), unquote(value)));
    }
    if !closed {
        return Err(FrontMatterError::Unclosed);
    }
    fm.body = rest[offset.min(rest.len())..].to_vec();
    Ok(fm)
}

/// `key: value` → `(key, value)`; `key:` → `(key, "")`.
fn split_kv(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once(':')?;
    let key = k.trim();
    if key.is_empty() || key.contains(' ') {
        return None;
    }
    Some((key, v.trim()))
}

/// Strip ONE pair of surrounding double quotes, never process escapes
/// (host-reality.md "SKILL.md frontmatter loader").
pub fn unquote(value: &str) -> String {
    let v = value.trim();
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

/// Collapse every whitespace run (space, tab, CR, LF, NBSP …) to one space and
/// trim, as the catalog renderer does (`docs/experiments/context-slimming.md`
/// §3; host-reality.md "frontmatter loader").
pub fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            pending = true;
        } else {
            if pending && !out.is_empty() {
                out.push(' ');
            }
            pending = false;
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalars_nested_maps_and_body() {
        let text = "---\nname: omm-x\ndescription: \"Use for x; Do not use for y.\"\nmetadata:\n  short-description: Short x\nargument-hint: \"[a]\"\n---\n\n# Body\n";
        let fm = parse(text.as_bytes()).unwrap();
        assert_eq!(fm.get("name"), Some("omm-x"));
        assert_eq!(fm.get("description"), Some("Use for x; Do not use for y."));
        assert_eq!(
            fm.get_nested("metadata", "short-description"),
            Some("Short x")
        );
        assert_eq!(fm.get("argument-hint"), Some("[a]"));
        assert_eq!(fm.body, b"\n# Body\n");
        assert!(!fm.bom);
        assert!(fm.duplicate_keys.is_empty());
        assert_eq!(
            fm.keys(),
            vec!["name", "description", "argument-hint", "metadata"]
        );
        assert_eq!(
            fm.metadata().get("short-description").map(String::as_str),
            Some("Short x")
        );
    }

    #[test]
    fn bom_duplicates_and_errors() {
        let bom = b"\xef\xbb\xbf---\ndescription: d\n---\nbody";
        let fm = parse(bom).unwrap();
        assert!(fm.bom);
        assert_eq!(fm.body, b"body");
        let dup = "---\nname: a\nname: b\n---\n";
        assert_eq!(parse(dup.as_bytes()).unwrap().duplicate_keys, vec!["name"]);
        assert_eq!(parse(b"no frontmatter"), Err(FrontMatterError::MissingOpen));
        assert_eq!(parse(b"---\nname: a\n"), Err(FrontMatterError::Unclosed));
        assert_eq!(
            parse(b"---\n\xff\xfe\n---\n"),
            Err(FrontMatterError::NotUtf8)
        );
        assert!(matches!(
            parse(b"---\njust words\n---\n"),
            Err(FrontMatterError::Syntax { line: 2, .. })
        ));
        assert!(matches!(
            parse(b"---\n  indented: without a map\n---\n"),
            Err(FrontMatterError::Syntax { .. })
        ));
        // CRLF line endings and comments are fine.
        let crlf = "---\r\nname: x\r\n# c\r\n---\r\nb";
        let fm = parse(crlf.as_bytes()).unwrap();
        assert_eq!(fm.get("name"), Some("x"));
        assert_eq!(fm.body, b"b");
    }

    #[test]
    fn yaml_safe_reserialization_quotes_every_scalar_and_keeps_the_body() {
        let text = "---\ndescription: Run it\nargument-hint: [--self-test] [--report-drift]\nmetadata:\n  short-description: a \"quoted\" \\ thing\n---\nbody\n";
        let fm = parse(text.as_bytes()).unwrap();
        let out = String::from_utf8(fm.to_yaml_safe_bytes()).unwrap();
        assert_eq!(
            out,
            "---\ndescription: \"Run it\"\nargument-hint: \"[--self-test] [--report-drift]\"\nmetadata:\n  short-description: \"a \\\"quoted\\\" \\\\ thing\"\n---\nbody\n"
        );
        // Round-trips through the lenient loader with the same values.
        let again = parse(out.as_bytes()).unwrap();
        assert_eq!(
            again.get("argument-hint"),
            Some("[--self-test] [--report-drift]")
        );
        assert_eq!(again.body, fm.body);
        assert_eq!(yaml_quote("a\tb\n"), "\"a\\tb\\n\"");
    }

    #[test]
    fn unquote_keeps_escapes_and_whitespace_collapses() {
        assert_eq!(unquote("\"a \\t b\""), "a \\t b");
        assert_eq!(unquote("plain"), "plain");
        assert_eq!(unquote("\"unterminated"), "\"unterminated");
        assert_eq!(collapse_whitespace("  a \t\n b\u{a0}c  "), "a b c");
    }
}
