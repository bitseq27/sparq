//! The dropdown picker (round-4 OWED item 11, discharged in WO-020 INC4, plan §8.4).
//!
//! `enum_select` for the inspector AND the panel widgets: an option list from the manifest, 44 px
//! rows (the touch floor — a picker whose rows shrink on a small screen is a picker that mis-taps),
//! the selected row in the class accent, scroll for long lists (the Observatory's cell enums carry
//! 24 options), keyboard-free (touch first). Toolkit-independent like the rest of the canvas core:
//! state + geometry + hit-testing here, the egui face in `sparq-app`.
//!
//! The picker is MODAL over the canvas like the menu and the browser: while open it captures taps
//! (a row selects, outside closes) and the wheel (scrolls). It edits exactly one param of one node;
//! the edit rides the ordinary [`crate::canvas::model::Op::SetParam`] (undoable, journal-safe).
//!
//! Refuse-in-words: a viewport too small to show even one touch-floor row does not get a
//! mis-tappable sheet — [`PickerState::sheet_rect`] answers `None` and the shell draws
//! [`TOO_SMALL_WORDS`] instead (the shell's existing refuse-in-words behaviour).

use crate::canvas::model::NodeId;
use crate::geom::{Rect, Vec2};
use crate::tokens::LAYOUT_TOUCH_TARGET_S;

/// The row height: the touch floor, exactly (`layout.touch.target_s`). §8.4: 44 px rows.
pub const ROW_H: f32 = LAYOUT_TOUCH_TARGET_S as f32;
/// The most rows shown before the sheet scrolls (24-option lists scroll; the sheet never grows
/// past the view anyway — this is the "one screenful" cap).
pub const MAX_VISIBLE_ROWS: usize = 8;
/// The sheet's width, logical px (clamped into the view).
pub const SHEET_W: f32 = 320.0;
/// The words a too-small viewport gets instead of a mis-tappable sheet.
pub const TOO_SMALL_WORDS: &str = "screen too small for the picker — enlarge the window";

/// Which param of which node the picker edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickerTarget {
    /// The node.
    pub node: NodeId,
    /// The param index (manifest order — the ParamSet position).
    pub param: usize,
}

/// The open picker's state. Closed = `target: None`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PickerState {
    /// The param being edited (`None` = closed).
    pub target: Option<PickerTarget>,
    /// The option rows `(value, label)` in manifest order — the index IS the enum's value.
    pub rows: Vec<(String, String)>,
    /// The scroll offset, px (clamped to [`PickerState::max_scroll`]).
    pub scroll: f32,
    /// Where the picker was opened (screen px) — the sheet anchors below it, clamped into view.
    pub anchor: Vec2,
    /// The param's value when the sheet opened (the enum's option index) — the row that wears the
    /// class accent. Read once at open: the sheet is a snapshot of the choice, and the select op
    /// carries the new value through the undoable door.
    pub current: f32,
}

impl PickerState {
    /// Opens (or re-opens) the picker for one param. An enum with no options refuses in words
    /// (there is nothing to choose — the shell shows the reason rather than an empty sheet).
    ///
    /// # Errors
    /// The words, when the param has no selectable options.
    pub fn open(
        &mut self,
        target: PickerTarget,
        rows: Vec<(String, String)>,
        anchor: Vec2,
        current: f32,
    ) -> Result<(), String> {
        if rows.is_empty() {
            return Err("this param has no selectable options — nothing to pick".to_string());
        }
        self.target = Some(target);
        self.rows = rows;
        self.scroll = 0.0;
        self.anchor = anchor;
        self.current = current;
        Ok(())
    }

    /// Closes the picker.
    pub fn close(&mut self) {
        self.target = None;
        self.rows.clear();
        self.scroll = 0.0;
        self.current = 0.0;
    }

    /// Whether the sheet is open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    /// The sheet's rect in screen px: anchored below the opening tap, clamped into the view,
    /// height = visible rows × [`ROW_H`]. `None` when the view cannot show one touch-floor row
    /// (the refuse-in-words case).
    #[must_use]
    pub fn sheet_rect(&self, view: Rect) -> Option<Rect> {
        if view.height() < ROW_H {
            return None;
        }
        let visible = self.rows.len().min(MAX_VISIBLE_ROWS);
        let h = visible as f32 * ROW_H;
        let h = h.min(view.height());
        let w = SHEET_W.min(view.width());
        // Anchor below the tap, clamped so the sheet is fully inside the view.
        let x = self.anchor.x.clamp(view.min.x, view.max.x - w);
        let y = (self.anchor.y + ROW_H * 0.5).clamp(view.min.y, view.max.y - h);
        Some(Rect::from_min_size(Vec2::new(x, y), Vec2::new(w, h)))
    }

    /// The scroll ceiling: the hidden rows below the sheet.
    #[must_use]
    pub fn max_scroll(&self, sheet: Rect) -> f32 {
        let total = self.rows.len() as f32 * ROW_H;
        (total - sheet.height()).max(0.0)
    }

    /// Scrolls by `delta` px (positive = towards later options), clamped to the ceiling.
    pub fn scroll_by(&mut self, delta: f32, sheet: Rect) {
        self.scroll = (self.scroll + delta).clamp(0.0, self.max_scroll(sheet));
    }

    /// The rect of option `index` (absolute, scroll applied), or `None` when it is scrolled out of
    /// the sheet (the drawer skips it — off-sheet rows are not drawn and not tappable).
    #[must_use]
    pub fn row_rect(&self, sheet: Rect, index: usize) -> Option<Rect> {
        let y = sheet.min.y + index as f32 * ROW_H - self.scroll;
        let r = Rect::from_min_size(Vec2::new(sheet.min.x, y), Vec2::new(sheet.width(), ROW_H));
        // Visible iff it overlaps the sheet vertically (a partial row at the edge is clipped by
        // the drawer but still tappable on its visible part — the touch floor is the row's own).
        (r.max.y > sheet.min.y && r.min.y < sheet.max.y).then_some(r)
    }

    /// The option index a screen point hits inside the sheet (`None` outside / in a gap).
    #[must_use]
    pub fn row_at(&self, sheet: Rect, pos: Vec2) -> Option<usize> {
        if !sheet.contains(pos) {
            return None;
        }
        let idx = ((pos.y - sheet.min.y + self.scroll) / ROW_H).floor();
        let idx = idx as usize;
        (idx < self.rows.len()).then_some(idx)
    }

    /// The index of the currently-selected option (the param's current value rides its option
    /// index), clamped — a value past the options selects nothing (`None`).
    #[must_use]
    pub fn selected(&self, current_value: f32) -> Option<usize> {
        if !current_value.is_finite() {
            return None;
        }
        let i = current_value.round() as usize;
        (i < self.rows.len()).then_some(i)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn rows(n: usize) -> Vec<(String, String)> {
        (0..n).map(|i| (format!("v{i}"), format!("Option {i}"))).collect()
    }

    fn view() -> Rect {
        Rect::from_min_size(Vec2::new(0.0, 0.0), Vec2::new(1200.0, 800.0))
    }

    #[test]
    fn open_close_round_trip() {
        let mut p = PickerState::default();
        assert!(!p.is_open());
        p.open(PickerTarget { node: 1, param: 2 }, rows(24), Vec2::new(100.0, 100.0), 0.0).unwrap();
        assert!(p.is_open());
        assert_eq!(p.rows.len(), 24);
        p.close();
        assert!(!p.is_open() && p.rows.is_empty());
    }

    #[test]
    fn an_enum_with_no_options_refuses_in_words() {
        let mut p = PickerState::default();
        let e = p.open(PickerTarget { node: 1, param: 0 }, vec![], Vec2::ZERO, 0.0).unwrap_err();
        assert!(e.contains("no selectable options"), "{e}");
        assert!(!p.is_open(), "a refused open leaves the picker closed");
    }

    #[test]
    fn the_sheet_shows_at_most_eight_touch_floor_rows_and_stays_in_view() {
        let mut p = PickerState::default();
        p.open(PickerTarget { node: 0, param: 0 }, rows(24), Vec2::new(1100.0, 780.0), 0.0)
            .unwrap();
        let sheet = p.sheet_rect(view()).unwrap();
        assert_eq!(sheet.height(), MAX_VISIBLE_ROWS as f32 * ROW_H, "8 rows of 44 px");
        assert!(
            sheet.max.x <= view().max.x && sheet.max.y <= view().max.y,
            "clamped into the view"
        );
        assert!(sheet.width() <= SHEET_W);
    }

    #[test]
    fn a_tiny_viewport_refuses_with_words_not_a_mis_tappable_sheet() {
        let p = PickerState {
            target: Some(PickerTarget { node: 0, param: 0 }),
            rows: rows(3),
            scroll: 0.0,
            anchor: Vec2::ZERO,
            current: 0.0,
        };
        let tiny = Rect::from_min_size(Vec2::ZERO, Vec2::new(200.0, 30.0));
        assert!(p.sheet_rect(tiny).is_none(), "a 30 px viewport cannot show a 44 px row");
        assert!(!TOO_SMALL_WORDS.is_empty());
    }

    #[test]
    fn hits_map_to_option_indices_and_scroll_moves_them() {
        let mut p = PickerState::default();
        p.open(PickerTarget { node: 0, param: 0 }, rows(24), Vec2::new(10.0, 10.0), 0.0).unwrap();
        let sheet = p.sheet_rect(view()).unwrap();
        // The first row's centre hits option 0.
        let r0 = p.row_rect(sheet, 0).unwrap();
        assert_eq!(p.row_at(sheet, Vec2::new(sheet.min.x + 5.0, r0.center().y)), Some(0));
        // Scroll one row: the same screen point now hits option 1.
        p.scroll_by(ROW_H, sheet);
        assert_eq!(p.scroll, ROW_H);
        assert_eq!(p.row_at(sheet, Vec2::new(sheet.min.x + 5.0, r0.center().y)), Some(1));
        // The scroll clamps at the ceiling.
        p.scroll_by(10_000.0, sheet);
        assert_eq!(p.scroll, p.max_scroll(sheet));
        assert!((p.max_scroll(sheet) - (24.0 - MAX_VISIBLE_ROWS as f32) * ROW_H).abs() < 1e-3);
        // Outside the sheet is not a hit.
        assert_eq!(p.row_at(sheet, Vec2::new(sheet.max.x + 50.0, sheet.min.y + 5.0)), None);
    }

    #[test]
    fn the_selected_row_is_the_param_value_s_option_index() {
        let p = PickerState {
            target: None,
            rows: rows(24),
            scroll: 0.0,
            anchor: Vec2::ZERO,
            current: 0.0,
        };
        assert_eq!(p.selected(5.0), Some(5));
        assert_eq!(p.selected(5.4), Some(5), "the value rounds to an index");
        assert_eq!(p.selected(99.0), None, "past the options selects nothing");
        assert_eq!(p.selected(f32::NAN), None);
    }

    #[test]
    fn rows_are_the_touch_floor() {
        assert_eq!(ROW_H, 44.0, "§8.4: 44 px rows, the contract's touch minimum");
        assert_eq!(crate::tokens::LAYOUT_SPACE_4, 16, "the sheet's padding rides the 8 px grid");
    }
}
