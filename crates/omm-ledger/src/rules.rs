//! The marked regions of the personal rules file — `$CONFIG_DIR/AGENTS.md`,
//! the first rung of the host's ladder of which exactly ONE file loads
//! (`docs/host-reality.md` "Paths": personal rules).
//!
//! The file model (Gate 1 round 5, decision H2): `[anything]` + the MANAGED
//! block (`managed-start` .. `managed-end`, marker lines included) +
//! `[anything]`. omm owns ONLY the managed block. Install and update replace
//! the managed region in place — or insert the block at the top of a file
//! that has none — and keep every other byte; uninstall takes away the
//! managed block, its marker lines and the template's [`Frame`] recorded in
//! the entry, and keeps the rest byte for byte (a rule the user appended
//! after the user-end marker, a title above the block: round 5 found every
//! rewrite dropping them). The `user-start` / `user-end` pair the template
//! ships stays supported as a legacy region — its marker lines are omm's
//! and go at uninstall, its text is the user's and stays — but it is no
//! longer the only thing preserved. Nothing here spells a marker: the caller
//! passes them as [`Markers`] (R8 spirit — the markers are content,
//! `content/rules/AGENTS.md.tmpl`). Everything here is pure text.

use serde::{Deserialize, Serialize};

use crate::hash;

/// The four marker lines of a rules document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Markers {
    pub managed_start: String,
    pub managed_end: String,
    pub user_start: String,
    pub user_end: String,
}

/// The two regions of a marked document: the text strictly between each
/// start marker's line end and its end marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Regions {
    pub managed: String,
    pub user: String,
}

/// A marked document cut at its markers: what lies before the managed-start
/// marker, the managed region, and what follows the managed-end marker —
/// split further, when the legacy user pair follows the managed pair, into
/// the text between the managed-end marker and the user-start marker, the
/// user region, and what follows the user-end marker. Only the managed
/// region and the marker lines are omm's; everything else is the user's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parts {
    /// Before the managed-start marker.
    pub pre: String,
    /// Strictly between the managed-start marker's line end and the
    /// managed-end marker.
    pub managed: String,
    /// From the managed-end marker (its line end included) to the user-start
    /// marker — or to the end of the document when there is no user pair.
    pub mid: String,
    /// The legacy user region, when the user pair follows the managed pair.
    pub user: Option<String>,
    /// After the user-end marker; empty without a user pair.
    pub post: String,
}

impl Parts {
    /// The text outside the managed region and the user region: what a
    /// template contributes besides the managed block.
    pub fn frame(&self) -> Frame {
        Frame {
            pre: self.pre.clone(),
            mid: self.mid.clone(),
            post: self.post.clone(),
            user: normalise(self.user.as_deref().unwrap_or("")),
        }
    }
}

/// The template's text outside the managed region — what omm's seed
/// contributes besides the managed block (a title line, blank lines, the
/// user marker lines' placeholder text). Recorded in the rules entry's
/// `prior` so uninstall can take exactly that away again and keep whatever
/// else the user put outside the markers; a piece the user changed no
/// longer matches and stays whole.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frame {
    pub pre: String,
    pub mid: String,
    pub post: String,
    /// The template's default user region, normalised (the placeholder
    /// comment): omm's while the user left it exactly so.
    #[serde(default)]
    pub user: String,
}

impl Frame {
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    pub fn from_json(v: &serde_json::Value) -> Option<Frame> {
        serde_json::from_value(v.clone()).ok()
    }
}

/// The index just past the line `at` sits on (`at` itself when no newline
/// follows — the same rule as [`between`]).
fn line_end(text: &str, at: usize) -> usize {
    match text[at..].find('\n') {
        Some(i) => at + i + 1,
        None => at,
    }
}

/// `mid` with the frame's `mid` taken away: equal → nothing; else the frame
/// stripped from either end; else `mid` whole (the user changed it).
fn subtract_mid<'a>(mid: &'a str, frame: &str) -> &'a str {
    if mid == frame {
        ""
    } else {
        mid.strip_prefix(frame)
            .or_else(|| mid.strip_suffix(frame))
            .unwrap_or(mid)
    }
}

/// The byte range strictly between `start`'s line end and `end`.
pub fn between(text: &str, start: &str, end: &str) -> Option<(usize, usize)> {
    let s = text.find(start)? + start.len();
    let after_line = match text[s..].find('\n') {
        Some(i) => s + i + 1,
        None => s,
    };
    let e = after_line + text[after_line..].find(end)?;
    Some((after_line, e))
}

impl Markers {
    /// Split a marked document. `None` when either pair of markers is missing.
    pub fn split(&self, text: &str) -> Option<Regions> {
        let (ms, me) = between(text, &self.managed_start, &self.managed_end)?;
        let (us, ue) = between(text, &self.user_start, &self.user_end)?;
        Some(Regions {
            managed: text[ms..me].to_string(),
            user: text[us..ue].to_string(),
        })
    }

    /// The managed region of a document — what omm owns in the file. `None`
    /// when the managed markers are missing.
    pub fn managed_region(&self, text: &str) -> Option<String> {
        let (ms, me) = between(text, &self.managed_start, &self.managed_end)?;
        Some(text[ms..me].to_string())
    }

    /// SHA-256 of the managed region. `None` when the document carries no
    /// managed markers.
    pub fn managed_sha256(&self, text: &str) -> Option<String> {
        self.managed_region(text)
            .map(|m| hash::sha256_bytes(m.as_bytes()))
    }

    /// What an existing file contributes to the user region: its marked
    /// user region when it carries the user markers, else the whole file.
    pub fn user_region_of(&self, existing: &str) -> String {
        match between(existing, &self.user_start, &self.user_end) {
            Some((us, ue)) => existing[us..ue].to_string(),
            None => existing.to_string(),
        }
    }

    /// Cut a document at its markers ([`Parts`]). `None` when the managed
    /// pair is missing or out of order. The user pair is read only when it
    /// follows the managed pair (as the template has them); anything else
    /// is the user's text.
    pub fn parts(&self, text: &str) -> Option<Parts> {
        let ms = text.find(&self.managed_start)?;
        let ms_end = line_end(text, ms + self.managed_start.len());
        let me = ms_end + text[ms_end..].find(&self.managed_end)?;
        let me_end = me + self.managed_end.len();
        let pre = text[..ms].to_string();
        let managed = text[ms_end..me].to_string();
        let user_pair = text[me_end..].find(&self.user_start).and_then(|rel| {
            let us = me_end + rel;
            let us_end = line_end(text, us + self.user_start.len());
            let ue = us_end + text[us_end..].find(&self.user_end)?;
            Some((us, us_end, ue, ue + self.user_end.len()))
        });
        Some(match user_pair {
            Some((us, us_end, ue, ue_end)) => Parts {
                pre,
                managed,
                mid: text[me_end..us].to_string(),
                user: Some(text[us_end..ue].to_string()),
                post: text[ue_end..].to_string(),
            },
            None => Parts {
                pre,
                managed,
                mid: text[me_end..].to_string(),
                user: None,
                post: String::new(),
            },
        })
    }

    /// The managed block as omm writes it: the two marker lines around
    /// `managed`.
    pub fn block(&self, managed: &str) -> String {
        format!("{}\n{managed}{}\n", self.managed_start, self.managed_end)
    }

    /// `text` with its managed region replaced by `managed` and every
    /// other byte kept — how install and update refresh what omm owns
    /// without touching what the user wrote anywhere else. `None` when the
    /// managed markers are missing.
    pub fn replace_managed(&self, text: &str, managed: &str) -> Option<String> {
        let (ms, me) = between(text, &self.managed_start, &self.managed_end)?;
        let mut out = String::with_capacity(text.len() + managed.len());
        out.push_str(&text[..ms]);
        out.push_str(managed);
        out.push_str(&text[me..]);
        Some(out)
    }

    /// The managed block inserted at the top of a file that has none, one
    /// blank line between the block and the file, the file's own bytes
    /// untouched. The frame of such a document is [`Markers::inserted_frame`].
    pub fn insert_managed(&self, text: &str, managed: &str) -> String {
        format!("{}\n{text}", self.block(managed))
    }

    /// What [`Markers::insert_managed`] contributes besides the managed
    /// block: the blank line after it.
    pub fn inserted_frame(&self) -> Frame {
        Frame {
            pre: String::new(),
            mid: "\n\n".to_string(),
            post: String::new(),
            user: String::new(),
        }
    }

    /// What uninstall leaves of a marked document: the managed block, its
    /// marker lines, the legacy user marker lines and the recorded `frame`
    /// gone, everything else kept — the user region normalised as it was
    /// rendered (dropped when it is still exactly the template's default),
    /// the text outside the markers byte for byte. With no frame recorded
    /// nothing outside the managed block is known to be omm's, so all of it
    /// stays.
    pub fn without_managed(&self, parts: &Parts, frame: Option<&Frame>) -> String {
        let (pre, mid, post) = match frame {
            Some(f) => (
                parts.pre.strip_suffix(f.pre.as_str()).unwrap_or(&parts.pre),
                subtract_mid(&parts.mid, &f.mid),
                parts
                    .post
                    .strip_prefix(f.post.as_str())
                    .unwrap_or(&parts.post),
            ),
            None => (parts.pre.as_str(), parts.mid.as_str(), parts.post.as_str()),
        };
        let user = match &parts.user {
            Some(u) => {
                let n = normalise(u);
                let is_default = frame
                    .map(|f| !f.user.is_empty() && normalise(&f.user) == n)
                    .unwrap_or(false);
                if is_default {
                    String::new()
                } else {
                    n
                }
            }
            None => String::new(),
        };
        format!("{pre}{mid}{user}{post}")
    }
}

/// Normalise a user region so rendering is idempotent: no leading or
/// trailing blank lines, exactly one trailing newline when non-empty, empty
/// when it holds only whitespace.
pub fn normalise(user: &str) -> String {
    let trimmed = user.trim_matches('\n');
    if trimmed.trim().is_empty() {
        String::new()
    } else {
        format!("{trimmed}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn markers() -> Markers {
        Markers {
            managed_start: "<!-- m-start -->".into(),
            managed_end: "<!-- m-end -->".into(),
            user_start: "<!-- u-start -->".into(),
            user_end: "<!-- u-end -->".into(),
        }
    }

    #[test]
    fn split_regions_and_user_region_of() {
        let m = markers();
        let doc = "# T\n<!-- m-start -->\n- one\n<!-- m-end -->\n\n<!-- u-start -->\nmine\n<!-- u-end -->\n";
        let r = m.split(doc).unwrap();
        assert_eq!(r.managed, "- one\n");
        assert_eq!(r.user, "mine\n");
        assert_eq!(m.user_region_of(doc), "mine\n");
        assert_eq!(m.user_region_of("plain file"), "plain file");
        assert_eq!(m.split("no markers"), None);
        assert_eq!(m.managed_sha256(doc), Some(hash::sha256_bytes(b"- one\n")));
        assert_eq!(m.managed_sha256("nope"), None);
        assert_eq!(m.managed_region(doc).as_deref(), Some("- one\n"));
        // The managed sha needs the managed pair only (no user pair).
        assert_eq!(
            m.managed_sha256("<!-- m-start -->\n- one\n<!-- m-end -->\nrest"),
            Some(hash::sha256_bytes(b"- one\n"))
        );
        // Markers out of order are not a region.
        assert_eq!(
            m.split("<!-- m-end -->\n<!-- m-start -->\n<!-- u-start -->\n<!-- u-end -->"),
            None
        );
    }

    #[test]
    fn parts_cut_at_the_markers_and_replace_managed_keeps_every_other_byte() {
        let m = markers();
        let doc = "TITLE\n\n<!-- m-start -->\n- one\n<!-- m-end -->\n\n<!-- u-start -->\nmine\n<!-- u-end -->\nTRAILING\n";
        let p = m.parts(doc).unwrap();
        assert_eq!(p.pre, "TITLE\n\n");
        assert_eq!(p.managed, "- one\n");
        assert_eq!(p.mid, "\n\n");
        assert_eq!(p.user.as_deref(), Some("mine\n"));
        assert_eq!(p.post, "\nTRAILING\n");
        assert_eq!(p.managed, m.split(doc).unwrap().managed);
        assert_eq!(
            p.frame(),
            Frame {
                pre: "TITLE\n\n".into(),
                mid: "\n\n".into(),
                post: "\nTRAILING\n".into(),
                user: "mine\n".into(),
            }
        );
        // The managed pair alone: everything after the block is `mid`.
        let solo = "<!-- m-start -->\n- one\n<!-- m-end -->\n\nrest\n";
        let p = m.parts(solo).unwrap();
        assert_eq!(p.pre, "");
        assert_eq!(p.managed, "- one\n");
        assert_eq!(p.mid, "\n\nrest\n");
        assert_eq!(p.user, None);
        assert_eq!(p.post, "");
        // A user pair BEFORE the managed pair is the user's text, not a region.
        let before =
            "<!-- u-start -->\nx\n<!-- u-end -->\n<!-- m-start -->\n- one\n<!-- m-end -->\n";
        let p = m.parts(before).unwrap();
        assert_eq!(p.pre, "<!-- u-start -->\nx\n<!-- u-end -->\n");
        assert_eq!(p.user, None);
        // No managed pair, or out of order: no parts.
        assert_eq!(m.parts("<!-- u-start -->\nx\n<!-- u-end -->\n"), None);
        assert_eq!(m.parts("<!-- m-end -->\n<!-- m-start -->\n"), None);
        // replace_managed touches the managed region alone.
        let next = m.replace_managed(doc, "- two\n- three\n").unwrap();
        assert_eq!(
            next,
            "TITLE\n\n<!-- m-start -->\n- two\n- three\n<!-- m-end -->\n\n<!-- u-start -->\nmine\n<!-- u-end -->\nTRAILING\n"
        );
        assert_eq!(m.replace_managed(doc, "- one\n").unwrap(), doc);
        assert_eq!(m.replace_managed("no markers", "x"), None);
        // A frame survives the JSON round trip; an older record without
        // `user` still loads.
        let f = m.parts(doc).unwrap().frame();
        assert_eq!(Frame::from_json(&f.to_json()), Some(f));
        assert_eq!(Frame::from_json(&serde_json::Value::Null), None);
        assert_eq!(
            Frame::from_json(&serde_json::json!({"pre": "a", "mid": "b", "post": "c"})),
            Some(Frame {
                pre: "a".into(),
                mid: "b".into(),
                post: "c".into(),
                user: String::new()
            })
        );
    }

    #[test]
    fn insert_managed_prepends_the_block_and_keeps_the_file_byte_for_byte() {
        let m = markers();
        let original = "my own rules\n- never delete me";
        let seeded = m.insert_managed(original, "- one\n");
        assert_eq!(
            seeded,
            "<!-- m-start -->\n- one\n<!-- m-end -->\n\nmy own rules\n- never delete me"
        );
        assert!(seeded.ends_with(original));
        assert_eq!(
            m.block("- one\n"),
            "<!-- m-start -->\n- one\n<!-- m-end -->\n"
        );
        // Its frame takes exactly the block and the blank line away again.
        let p = m.parts(&seeded).unwrap();
        assert_eq!(m.without_managed(&p, Some(&m.inserted_frame())), original);
        // A rule appended below survives byte for byte.
        let appended = format!("{seeded}\n- appended\n");
        let p = m.parts(&appended).unwrap();
        assert_eq!(
            m.without_managed(&p, Some(&m.inserted_frame())),
            format!("{original}\n- appended\n")
        );
        // Refreshing the block in place keeps the rest.
        let next = m.replace_managed(&appended, "- two\n").unwrap();
        assert!(next.starts_with("<!-- m-start -->\n- two\n<!-- m-end -->\n\nmy own rules"));
        assert!(next.ends_with("- appended\n"));
    }

    #[test]
    fn without_managed_takes_away_exactly_the_frame_and_keeps_the_rest() {
        let m = markers();
        let placeholder = "<!-- Your rules. -->\n";
        let frame = Frame {
            pre: "# T\n\n".into(),
            mid: "\n\n".into(),
            post: "\n".into(),
            user: placeholder.into(),
        };
        // Exactly what omm rendered plus a user region: the region alone.
        let seeded = "# T\n\n<!-- m-start -->\n- one\n<!-- m-end -->\n\n<!-- u-start -->\nmine\n<!-- u-end -->\n";
        let p = m.parts(seeded).unwrap();
        assert_eq!(m.without_managed(&p, Some(&frame)), "mine\n");
        // Text before the frame and after it: kept byte for byte (round 5:
        // a title prepended and a rule appended after the user-end marker).
        let extra = format!("LEAD\n{seeded}TRAIL\n");
        let p = m.parts(&extra).unwrap();
        assert_eq!(m.without_managed(&p, Some(&frame)), "LEAD\nmine\nTRAIL\n");
        // The template's own placeholder in the user region is omm's: gone
        // with the rest, so a seed with only outside edits leaves those alone.
        let untouched_region = format!("LEAD\n{}TRAIL\n", seeded.replace("mine\n", placeholder));
        let p = m.parts(&untouched_region).unwrap();
        assert_eq!(m.without_managed(&p, Some(&frame)), "LEAD\nTRAIL\n");
        // …but the placeholder plus a rule below it is the user's region whole.
        let kept = seeded.replace("mine\n", &format!("{placeholder}- mine\n"));
        let p = m.parts(&kept).unwrap();
        assert_eq!(
            m.without_managed(&p, Some(&frame)),
            format!("{placeholder}- mine\n")
        );
        // A title the user rewrote no longer matches the frame: kept whole.
        let retitled = seeded.replace("# T\n", "# Mine\n");
        let p = m.parts(&retitled).unwrap();
        assert_eq!(m.without_managed(&p, Some(&frame)), "# Mine\n\nmine\n");
        // Something between the two blocks stays too.
        let mid = seeded.replace("<!-- m-end -->\n\n", "<!-- m-end -->\n\nBETWEEN\n");
        let p = m.parts(&mid).unwrap();
        assert_eq!(m.without_managed(&p, Some(&frame)), "BETWEEN\nmine\n");
        // An empty user region and nothing outside: nothing left.
        let empty = seeded.replace("mine\n", "");
        let p = m.parts(&empty).unwrap();
        assert_eq!(m.without_managed(&p, Some(&frame)), "");
        // No frame recorded: nothing outside the markers is known to be
        // omm's, so it all stays (the marker lines still go).
        let p = m.parts(seeded).unwrap();
        assert_eq!(m.without_managed(&p, None), "# T\n\n\n\nmine\n\n");
        // The managed pair alone, no frame: the block and its markers go.
        let p = m
            .parts("A\n<!-- m-start -->\n- one\n<!-- m-end -->\nB\n")
            .unwrap();
        assert_eq!(m.without_managed(&p, None), "A\n\nB\n");
        // The template's frame on a document omm did not seed from the
        // template (round 5: a marked file adopted after its ledger was
        // lost records the template's frame): only the pieces that still
        // equal the template's go — the block inserted above a user's file
        // shares the template's `mid`, so the user's file comes out whole.
        let inserted = m.insert_managed("my own rules\n- never delete me", "- one\n");
        let p = m.parts(&inserted).unwrap();
        assert_eq!(
            m.without_managed(&p, Some(&frame)),
            "my own rules\n- never delete me"
        );
        // …and a title the user put above the block stays with it.
        let p = m.parts(&format!("MINE\n{inserted}")).unwrap();
        assert_eq!(
            m.without_managed(&p, Some(&frame)),
            "MINE\nmy own rules\n- never delete me"
        );
    }

    #[test]
    fn normalise_is_idempotent() {
        assert_eq!(normalise("\n\nmine\n\n"), "mine\n");
        assert_eq!(normalise("mine"), "mine\n");
        assert_eq!(normalise("  \n \n"), "");
        assert_eq!(normalise(&normalise("a\n\nb")), "a\n\nb\n");
    }
}
