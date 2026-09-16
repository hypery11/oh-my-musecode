//! The personal rules file omm seeds — `$CONFIG_DIR/AGENTS.md`, the first
//! rung of the host's four-rung ladder of which exactly ONE file loads
//! (`docs/host-reality.md` "Paths": personal rules) — rendered from
//! `content/rules/AGENTS.md.tmpl`.
//!
//! The file model (`omm_ledger::rules`, Gate 1 round 5 decision H2):
//! `[anything]` + the MANAGED block (`omm_manifest::lint::RULES_MANAGED_START
//! / END`, the lint's `rules-markers` rule, marker lines included) +
//! `[anything]`. omm owns ONLY the managed block: install and update replace
//! its region in place — or insert the block at the top of a file that has
//! none — and keep every other byte; uninstall takes the block, its marker
//! lines and the template's recorded frame away and keeps the rest. The
//! USER region ([`USER_START`] / [`USER_END`]) the template ships is a
//! legacy region: preserved like everything else, its marker lines omm's.
//! Everything here is pure text; the callers decide what to write.

use omm_ledger::rules::{self as ledger_rules, Markers};
use omm_manifest::lint::{RULES_MANAGED_END, RULES_MANAGED_START};

/// Start marker of the user region (`content/rules/AGENTS.md.tmpl`).
pub const USER_START: &str = "<!-- omm:user-start -->";
/// End marker of the user region.
pub const USER_END: &str = "<!-- omm:user-end -->";

/// The regions, the cut and the frame of a marked rules document
/// (`omm_ledger::rules`).
pub use omm_ledger::rules::{Frame, Parts, Regions};

/// The four markers as the ledger's uninstall planner takes them
/// (`omm_ledger::uninstall::Options::rules`): the managed pair from the lint
/// constants, the user pair from this module. The one place the CLI hands
/// the marker text to the ledger crate, which spells none of it.
pub fn markers() -> Markers {
    Markers {
        managed_start: RULES_MANAGED_START.to_string(),
        managed_end: RULES_MANAGED_END.to_string(),
        user_start: USER_START.to_string(),
        user_end: USER_END.to_string(),
    }
}

/// What an existing file contributes to the user region.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserRegion {
    /// The file carries the user markers; this is what lies between them.
    Marked(String),
    /// No markers: the whole file becomes the user region.
    Whole(String),
    /// Nothing usable (empty or whitespace only).
    Empty,
}

impl UserRegion {
    /// The region text to render.
    pub fn text(&self) -> &str {
        match self {
            UserRegion::Marked(s) | UserRegion::Whole(s) => s,
            UserRegion::Empty => "",
        }
    }
}

/// The byte range strictly between `start`'s line end and `end`.
fn between(text: &str, start: &str, end: &str) -> Option<(usize, usize)> {
    ledger_rules::between(text, start, end)
}

/// Split a marked document. `None` when either pair of markers is missing.
pub fn split(text: &str) -> Option<Regions> {
    markers().split(text)
}

/// Cut a document at its markers ([`Parts`]); `None` without the managed
/// pair.
pub fn parts(text: &str) -> Option<Parts> {
    markers().parts(text)
}

/// SHA-256 of the managed region — what omm owns in the file. `None` when
/// the document carries no managed markers.
pub fn managed_sha256(text: &str) -> Option<String> {
    markers().managed_sha256(text)
}

/// `text` with its managed region replaced by `managed`, every other byte
/// kept; `None` without the managed pair.
pub fn replace_managed(text: &str, managed: &str) -> Option<String> {
    markers().replace_managed(text, managed)
}

/// The managed block inserted at the top of an unmarked file, the file's
/// bytes untouched; its frame is [`inserted_frame`].
pub fn insert_managed(text: &str, managed: &str) -> String {
    markers().insert_managed(text, managed)
}

/// What [`insert_managed`] contributes besides the block.
pub fn inserted_frame() -> Frame {
    markers().inserted_frame()
}

/// Normalise a user region so rendering is idempotent: no leading blank
/// lines, exactly one trailing newline when non-empty.
fn normalise(user: &str) -> String {
    ledger_rules::normalise(user)
}

/// The template with `user` as its user region. `None` when the template
/// lacks the markers (the lint refuses such a template before it ships).
pub fn render(template: &str, user: &str) -> Option<String> {
    let (us, ue) = between(template, USER_START, USER_END)?;
    let mut out = String::with_capacity(template.len() + user.len());
    out.push_str(&template[..us]);
    out.push_str(&normalise(user));
    out.push_str(&template[ue..]);
    Some(out)
}

/// What an existing file contributes to the user region: its marked user
/// region when it carries the markers, else the whole file.
pub fn user_region_of(existing: &str) -> UserRegion {
    if let Some((us, ue)) = between(existing, USER_START, USER_END) {
        let region = &existing[us..ue];
        if region.trim().is_empty() {
            UserRegion::Empty
        } else {
            UserRegion::Marked(region.to_string())
        }
    } else if existing.trim().is_empty() {
        UserRegion::Empty
    } else {
        UserRegion::Whole(existing.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEMPLATE: &str = "# Rules\n\n<!-- omm:managed-start -->\n- managed one\n- managed two\n<!-- omm:managed-end -->\n\n<!-- omm:user-start -->\n<!-- Your rules. -->\n<!-- omm:user-end -->\n";

    #[test]
    fn split_render_and_round_trip() {
        let r = split(TEMPLATE).unwrap();
        assert_eq!(r.managed, "- managed one\n- managed two\n");
        assert_eq!(r.user, "<!-- Your rules. -->\n");
        let rendered = render(TEMPLATE, "- mine\n\n- also mine\n").unwrap();
        assert!(rendered
            .contains("<!-- omm:user-start -->\n- mine\n\n- also mine\n<!-- omm:user-end -->"));
        assert!(!rendered.contains("Your rules"));
        // Idempotent: rendering with the user region of the rendering changes nothing.
        let again = render(TEMPLATE, user_region_of(&rendered).text()).unwrap();
        assert_eq!(again, rendered);
        // The managed region is untouched by the user region.
        assert_eq!(managed_sha256(&rendered), managed_sha256(TEMPLATE));
        // An empty user region renders to adjacent markers.
        let empty = render(TEMPLATE, "\n\n  \n").unwrap();
        assert!(empty.contains("<!-- omm:user-start -->\n<!-- omm:user-end -->"));
        assert_eq!(user_region_of(&empty), UserRegion::Empty);
    }

    #[test]
    fn unmarked_files_become_the_user_region_whole() {
        assert_eq!(
            user_region_of("my own rules\n"),
            UserRegion::Whole("my own rules\n".into())
        );
        assert_eq!(user_region_of("  \n"), UserRegion::Empty);
        let rendered = render(TEMPLATE, "my own rules\n").unwrap();
        assert!(rendered.contains("<!-- omm:user-start -->\nmy own rules\n<!-- omm:user-end -->"));
        assert_eq!(split("no markers at all"), None);
        assert_eq!(managed_sha256("no markers"), None);
        assert_eq!(render("no markers", "x"), None);
        // Markers out of order (end before start) are not a region.
        assert_eq!(
            split("<!-- omm:managed-end -->\n<!-- omm:managed-start -->\n<!-- omm:user-start -->\n<!-- omm:user-end -->"),
            None
        );
    }

    #[test]
    fn the_managed_block_is_replaced_in_place_or_inserted_at_the_top() {
        // Round 5 (H2): text outside the markers is never rewritten.
        let p = parts(TEMPLATE).unwrap();
        assert_eq!(p.managed, "- managed one\n- managed two\n");
        assert_eq!(p.frame().pre, "# Rules\n\n");
        assert_eq!(p.frame().user, "<!-- Your rules. -->\n");
        let edited = format!("TITLE\n{TEMPLATE}TRAILING\n");
        let next = replace_managed(&edited, "- managed three\n").unwrap();
        assert!(next.starts_with("TITLE\n# Rules\n\n<!-- omm:managed-start -->\n- managed three\n<!-- omm:managed-end -->\n"));
        assert!(next.ends_with("<!-- omm:user-end -->\nTRAILING\n"));
        assert_eq!(replace_managed("plain", "x"), None);
        let seeded = insert_managed("plain\n- mine", "- managed one\n");
        assert_eq!(
            seeded,
            "<!-- omm:managed-start -->\n- managed one\n<!-- omm:managed-end -->\n\nplain\n- mine"
        );
        assert_eq!(
            markers().without_managed(&parts(&seeded).unwrap(), Some(&inserted_frame())),
            "plain\n- mine"
        );
    }
}
