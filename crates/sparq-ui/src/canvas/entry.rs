//! Text entry (WO-013 increment 5): the toolkit-independent single-line buffer the rename sheet
//! runs on — the surface the browser's provisional keyboard feed was declared to be waiting for.
//!
//! Like everything in `sparq-ui::canvas`, this is **computed, not drawn**: the buffer, its cap
//! and the sheet geometry live here; the painter in `sparq-app` renders exactly these rects and
//! the shell pipes exactly these keys. The toolkit side (egui raw events → characters) stays in
//! the shell, where the WO-012 wrap-egui rule puts it — no widget owns input behind the
//! recogniser's back, and swapping the renderer swaps nothing here.
//!
//! v0 limits, stated rather than hidden:
//! * the caret is the END of the buffer — insert appends, backspace pops, no caret movement
//!   (the browser query has always worked this way; a shared surface keeps one honest rule);
//! * one line, capped at [`ENTRY_MAX_CHARS`] — the cap is the node header's budget: a rename
//!   longer than the header can show would be a name the canvas cannot display;
//! * Enter commits and Escape cancels are SHELL decisions (the core has no keyboard policy) —
//!   the core exposes `commit`/`cancel` and the shell decides which key calls them.

use crate::canvas::model::NodeId;
use crate::geom::{Rect, Vec2};
use crate::tokens::{LAYOUT_SPACE_2, LAYOUT_SPACE_3, LAYOUT_TOUCH_ROW_HEIGHT_LIST};

/// The longest name the entry accepts, in characters. A node header draws at the canvas's Full
/// LOD inside a default node width; 32 monospace characters is the budget that fits it, so the
/// name you can type is a name the canvas can show. The refusal is a cap, not an error: extra
/// characters are declined silently because the sheet shows the buffer with the caret at the
/// end — you can SEE that typing stopped advancing.
pub const ENTRY_MAX_CHARS: usize = 32;

/// A single-line text buffer with an end caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEntry {
    buf: String,
    initial: String,
}

impl TextEntry {
    /// An entry pre-filled with `initial` (the rename sheet opens showing the current name).
    #[must_use]
    pub fn new(initial: &str) -> Self {
        let clean = Self::sanitize(initial);
        Self { buf: clean.clone(), initial: clean }
    }

    /// The buffer as it stands.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.buf
    }

    /// What the entry opened with (the painter's header shows the module's own name next to it).
    #[must_use]
    pub fn initial(&self) -> &str {
        &self.initial
    }

    /// The buffer with the painter's caret appended — the sheet's entry line, one string. An
    /// empty buffer shows a bare caret: the hint row already says what committing it means.
    #[must_use]
    pub fn caret_text(&self) -> String {
        if self.buf.is_empty() {
            "\u{2026}_".to_string()
        } else {
            format!("{}_", self.buf)
        }
    }

    /// Whether the buffer still equals what it opened with (a commit of an unchanged name is a
    /// no-op the core reports in words rather than pushing an empty history entry).
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.buf == self.initial
    }

    /// Append `text`, dropping control characters and stopping at [`ENTRY_MAX_CHARS`].
    pub fn insert(&mut self, text: &str) {
        let room = ENTRY_MAX_CHARS.saturating_sub(self.buf.chars().count());
        let added: String = Self::sanitize(text).chars().take(room).collect();
        self.buf.push_str(&added);
    }

    /// Delete the character before the caret (the end). A no-op on an empty buffer.
    pub fn backspace(&mut self) {
        self.buf.pop();
    }

    /// Replace the whole buffer (the documented headless path, mirroring `browser_set_query`).
    pub fn set_text(&mut self, text: &str) {
        self.buf.clear();
        self.insert(text);
    }

    /// The committed value: the buffer, trimmed. An empty result means "clear the rename" — the
    /// node falls back to its module's display name (the sheet's hint row says exactly that).
    #[must_use]
    pub fn committed(&self) -> String {
        self.buf.trim().chars().take(ENTRY_MAX_CHARS).collect()
    }

    /// Printables only, capped — the one cleaning rule every path through the buffer obeys.
    fn sanitize(text: &str) -> String {
        text.chars().filter(|c| !c.is_control()).take(ENTRY_MAX_CHARS).collect()
    }
}

/// The rename sheet: which node is being renamed, the entry buffer, and where the long-press
/// anchored it. Modal over the canvas like the menu and the browser; the sheet geometry mirrors
/// the browser's (row-height tokens, the same monospace advance budget, `clamp_origin`), so both
/// overlays sit on the screen by one rule.
#[derive(Clone, Debug, PartialEq)]
pub struct RenameState {
    /// The node being renamed.
    pub node: NodeId,
    /// The module id, for the header line ("RENAME sparq/syn/sine").
    pub module_id: String,
    /// The buffer.
    pub entry: TextEntry,
    /// Where the long-press landed, screen px (the sheet anchors here, clamped into view).
    pub anchor: Vec2,
}

/// The sheet's rows: header (what is being renamed), entry (the buffer + caret), hint (the keys
/// and the empty-commits-default rule). A count, not a visual size — the row height is the token.
const SHEET_ROWS: f32 = 3.0;
/// The hint line, drawn dim under the buffer. Its length also floors the sheet width, so the
/// three rows never disagree about how wide the sheet is.
pub const RENAME_HINT: &str = "ENTER COMMIT \u{b7} ESC CANCEL \u{b7} EMPTY = MODULE DEFAULT";

impl RenameState {
    /// Open the sheet for `node`, pre-filled with its current title.
    #[must_use]
    pub fn open(node: NodeId, module_id: &str, title: &str, anchor: Vec2) -> Self {
        Self { node, module_id: module_id.to_string(), entry: TextEntry::new(title), anchor }
    }

    /// The header line: what this sheet renames.
    #[must_use]
    pub fn header_text(&self) -> String {
        format!("RENAME {}", self.module_id)
    }

    /// The sheet width: the widest of its three lines on the monospace advance budget the menu
    /// and the browser use, clamped to `max_w`.
    #[must_use]
    pub fn width(&self, max_w: f32) -> f32 {
        let char_w = LAYOUT_SPACE_2 as f32;
        let pad = LAYOUT_SPACE_3 as f32 * 2.0;
        let longest = self
            .header_text()
            .chars()
            .count()
            .max(RENAME_HINT.chars().count())
            .max(self.entry.text().chars().count() + 1); // + the caret
        (pad + longest as f32 * char_w).min(max_w)
    }

    /// The sheet height: three token rows.
    #[must_use]
    pub fn height(&self) -> f32 {
        SHEET_ROWS * LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32
    }

    /// The clamped top-left of the sheet inside `view`.
    #[must_use]
    pub fn sheet_origin(&self, view: Rect, max_w: f32) -> Vec2 {
        let size = Vec2::new(self.width(max_w), self.height());
        crate::canvas::interact::clamp_origin(self.anchor, size, view)
    }

    /// Row `i` of the sheet (0 header, 1 entry, 2 hint), given the clamped `origin`.
    #[must_use]
    pub fn row_rect(&self, origin: Vec2, i: usize, max_w: f32) -> Rect {
        let row = LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32;
        Rect::from_min_size(
            Vec2::new(origin.x, origin.y + i as f32 * row),
            Vec2::new(self.width(max_w), row),
        )
    }

    /// Whether a screen point lands inside the sheet (the modal's own capture test).
    #[must_use]
    pub fn contains(&self, pos: Vec2, view: Rect, max_w: f32) -> bool {
        let origin = self.sheet_origin(view, max_w);
        Rect::from_min_size(origin, Vec2::new(self.width(max_w), self.height())).contains(pos)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    fn view() -> Rect {
        Rect::from_min_size(Vec2::new(100.0, 50.0), Vec2::new(1000.0, 700.0))
    }

    #[test]
    fn the_buffer_inserts_pops_and_caps() {
        let mut e = TextEntry::new("Sine");
        assert_eq!(e.text(), "Sine");
        assert!(e.is_unchanged());
        e.insert(" Bass");
        assert_eq!(e.text(), "Sine Bass");
        assert!(!e.is_unchanged());
        e.backspace();
        assert_eq!(e.text(), "Sine Bas");
        // Control characters never enter the buffer (some backends deliver them as Text).
        e.insert("\n\t\u{7f}X");
        assert_eq!(e.text(), "Sine BasX");
        // The cap is a wall, not an error: typing past it declines silently.
        let mut full = TextEntry::new("");
        full.insert(&"A".repeat(ENTRY_MAX_CHARS + 40));
        assert_eq!(full.text().chars().count(), ENTRY_MAX_CHARS);
        full.insert("B");
        assert_eq!(full.text().chars().count(), ENTRY_MAX_CHARS, "the cap holds");
        full.set_text("short");
        assert_eq!(full.text(), "short", "set_text replaces (and sanitises)");
        let mut empty = TextEntry::new("");
        empty.backspace();
        assert_eq!(empty.text(), "", "backspace on empty is a no-op, never a panic");
    }

    #[test]
    fn an_over_long_initial_is_sanitised_once_at_the_door() {
        let e = TextEntry::new(&"N".repeat(ENTRY_MAX_CHARS + 10));
        assert_eq!(e.text().chars().count(), ENTRY_MAX_CHARS);
        assert_eq!(e.initial(), e.text(), "unchanged means unchanged, even after sanitising");
        assert!(e.is_unchanged());
    }

    #[test]
    fn committed_trims_and_empty_means_clear() {
        let mut e = TextEntry::new("Sine");
        e.set_text("  Kick Drum  ");
        assert_eq!(e.committed(), "Kick Drum", "the committed value is trimmed");
        e.set_text("   ");
        assert_eq!(e.committed(), "", "whitespace commits as empty = clear the rename");
    }

    #[test]
    fn the_sheet_geometry_clamps_into_view_and_rows_partition_it() {
        let s = RenameState::open(3, "sparq/syn/sine", "Sine", Vec2::new(400.0, 300.0));
        let (max_w, _) = crate::canvas::browser::caps(view());
        let origin = s.sheet_origin(view(), max_w);
        let h = s.height();
        assert_eq!(h, 3.0 * LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32);
        for i in 0..3 {
            let r = s.row_rect(origin, i, max_w);
            assert!(r.min.y >= origin.y - 1e-3 && r.max.y <= origin.y + h + 1e-3);
            assert!(r.width() <= max_w + 1e-3, "the sheet respects the browser's width cap");
        }
        // An anchor off the edge clamps the whole sheet inside the view (the shared rule).
        let edge = RenameState::open(3, "sparq/syn/sine", "Sine", Vec2::new(5000.0, 5000.0));
        let o = edge.sheet_origin(view(), max_w);
        let rect = Rect::from_min_size(o, Vec2::new(edge.width(max_w), edge.height()));
        assert!(rect.max.x <= view().max.x + 1e-3 && rect.max.y <= view().max.y + 1e-3);
        assert!(edge.contains(view().center(), view(), max_w) || !rect.contains(view().center()));
        assert!(edge.contains(Vec2::new(o.x + 5.0, o.y + 5.0), view(), max_w));
    }

    #[test]
    fn the_header_names_the_module_being_renamed() {
        let s = RenameState::open(7, "sparq/util/gain", "Gain", Vec2::ZERO);
        assert_eq!(s.header_text(), "RENAME sparq/util/gain");
        assert_eq!(s.entry.text(), "Gain", "the buffer opens pre-filled with the current title");
        assert!(s.entry.is_unchanged());
    }
}
