//! The card and cell geometry — §5.1's numbers as functions, and as tests.
//!
//! The Observatory is a wall-class instrument: one giant `display_list` display divided into a
//! 4…16-cell grid. This module turns the declared display rect + the layout/solo params + the
//! ticker flag into concrete cell rectangles, and splits each cell into the §5.3 anatomy bands
//! (header / body / axis / footer). It is pure geometry in logical px — the guest lays out against
//! logical px, never device pixels (frame-context's contract) — and every constant is a token value
//! ([`Metrics`]) so the host's spacing decisions flow in rather than being remembered here.
//!
//! # The §5.1 table, reproduced by formula (not by memory)
//!
//! With the display at its declared `min_size` (2176 × 1120), gap = `layout.space.4` = 16, and
//! `cell = round((span − (n−1)·gap) / n)`:
//!
//! ```text
//!   2×2 → 1080 × 552    4×2 → 532 × 552    3×3 → 715 × 363
//!   4×3 →  532 × 363    4×4 → 532 × 268    SOLO → 2176 × 1120
//! ```
//!
//! Every value matches the plan's §5.1 table except the 4×4 cell HEIGHT: the table prints 270, but
//! the plan's own gap (`space.4` = 16) gives `round((1120 − 3·16)/4) = round(268.0) = 268`. The
//! formula is the source of truth (a hand-computed table cell is exactly the kind of number the
//! house rule says must be measured, not remembered); the 2 px is a §5.1 erratum found by building
//! it, recorded here and in the state card rather than papered over by an inconsistent formula.

use crate::ir::Lod;

/// The layout enum's five grids (§5.2's LAYOUT picker, §5.1's cell counts). The manifest's
/// `layout` param rides its option index; [`Layout::from_option`] decodes it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    /// 2 × 2 — 4 cells (the biggest cells, the tablet/SOLO-adjacent case).
    Grid2x2,
    /// 4 × 2 — 8 cells.
    Grid4x2,
    /// 3 × 3 — 9 cells.
    Grid3x3,
    /// 4 × 3 — 12 cells.
    Grid4x3,
    /// 4 × 4 — 16 cells (the default face, §5.4: the full wall).
    #[default]
    Grid4x4,
}

impl Layout {
    /// The five layouts in manifest option order (index 0…4 — the `layout` param's enum options).
    pub const OPTIONS: [Self; 5] =
        [Self::Grid2x2, Self::Grid4x2, Self::Grid3x3, Self::Grid4x3, Self::Grid4x4];

    /// Decodes the `layout` param's option index. An out-of-range index falls back to the default
    /// (4×4) rather than refusing — a param snapshot past the declared options is a host bug the
    /// display should survive (draw the default wall, never garbage).
    #[must_use]
    pub fn from_option(option: usize) -> Self {
        Self::OPTIONS.get(option).copied().unwrap_or(Self::Grid4x4)
    }

    /// This layout's option index (0…4).
    #[must_use]
    pub fn option(self) -> usize {
        // OPTIONS is small and self is Copy; a linear find is clearer than a match and cannot drift.
        Self::OPTIONS.iter().position(|&l| l == self).unwrap_or(4)
    }

    /// Column count.
    #[must_use]
    pub fn cols(self) -> usize {
        match self {
            Self::Grid2x2 => 2,
            Self::Grid4x2 | Self::Grid4x3 | Self::Grid4x4 => 4,
            Self::Grid3x3 => 3,
        }
    }

    /// Row count.
    #[must_use]
    pub fn rows(self) -> usize {
        match self {
            Self::Grid2x2 => 2,
            Self::Grid4x2 => 2,
            Self::Grid3x3 | Self::Grid4x3 => 3,
            Self::Grid4x4 => 4,
        }
    }

    /// Cell count (`cols × rows`) — 4…16.
    #[must_use]
    pub fn cell_count(self) -> usize {
        self.cols() * self.rows()
    }
}

/// The spacing/band metrics the layout computes against, all token values (plan D6/§5.1). The
/// defaults mirror the checked-in bundle (`layout.space.*`, `typography.scale.*`); the wasm glue
/// overrides them from the runtime `token_bundle()` in `prepare`, and the harness drift-tests the
/// defaults against `design/tokens/generated/tokens.json`. The host owns the values; the core owns
/// only the arithmetic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// Cell-grid gap, logical px (`layout.space.4` = 16).
    pub gap: f32,
    /// Inner cell padding (`layout.space.3` = 12) — between the cell edge and its content.
    pub pad: f32,
    /// Header band height (title + description + LED row).
    pub header_h: f32,
    /// Footer band height (the stamp · age · headline line).
    pub footer_h: f32,
    /// Axis band height (x-axis tick labels; renderers that have no x-axis give it back to the body).
    pub axis_h: f32,
    /// Global ticker band height (§5.5: the bottom 40 px of the display).
    pub ticker_h: f32,
    /// Status-LED radius (an `arc` 0→360; the LED is furniture, drawn small).
    pub led_r: f32,
    /// The title type size, px (`typography.scale.m` = 13) — used to fit the header text.
    pub title_px: f32,
    /// The small type size, px (`typography.scale.s` = 11) — footer/axis/description text.
    pub small_px: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            gap: 16.0,      // layout.space.4
            pad: 12.0,      // layout.space.3
            header_h: 22.0, // fits a 13 px title + a 5 px LED with breathing room
            footer_h: 16.0, // fits an 11 px stamp line
            axis_h: 14.0,   // fits 11 px axis labels
            ticker_h: 40.0, // §5.5
            led_r: 5.0,
            title_px: 13.0, // typography.scale.m
            small_px: 11.0, // typography.scale.s
        }
    }
}

/// A rectangle in logical px (the cell's outer bounds, or an inner band).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
}

impl Rect {
    /// A rect at (`x`, `y`) sized (`w`, `h`).
    #[must_use]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// The right edge.
    #[must_use]
    pub const fn right(self) -> f32 {
        self.x + self.w
    }

    /// The bottom edge.
    #[must_use]
    pub const fn bottom(self) -> f32 {
        self.y + self.h
    }

    /// The centre x.
    #[must_use]
    pub const fn cx(self) -> f32 {
        self.x + self.w / 2.0
    }

    /// The centre y.
    #[must_use]
    pub const fn cy(self) -> f32 {
        self.y + self.h / 2.0
    }

    /// This rect inset by `d` on all four sides (clamped to non-negative size).
    #[must_use]
    pub fn inset(self, d: f32) -> Self {
        let w = (self.w - 2.0 * d).max(0.0);
        let h = (self.h - 2.0 * d).max(0.0);
        Self { x: self.x + d, y: self.y + d, w, h }
    }

    /// The top `h` slice of this rect.
    #[must_use]
    pub fn take_top(self, h: f32) -> Self {
        Self { x: self.x, y: self.y, w: self.w, h: h.min(self.h) }
    }

    /// The bottom `h` slice of this rect.
    #[must_use]
    pub fn take_bottom(self, h: f32) -> Self {
        let h = h.min(self.h);
        Self { x: self.x, y: self.bottom() - h, w: self.w, h }
    }

    /// Whether a point is inside (inclusive).
    #[must_use]
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.right() && y >= self.y && y <= self.bottom()
    }
}

/// One cell's geometry: its outer rect and the §5.3 anatomy bands within it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CellRect {
    /// The cell index (0…15), = `row * cols + col`.
    pub index: usize,
    /// The outer rect (the grid slot, gap-excluded).
    pub outer: Rect,
    /// The content rect (outer inset by [`Metrics::pad`]).
    pub content: Rect,
    /// The header band (title/description/LED) at the top of `content`.
    pub header: Rect,
    /// The footer band (stamp · age · headline) at the bottom of `content`.
    pub footer: Rect,
    /// The axis band (x-axis labels) just above the footer; zero-height when the renderer has none.
    pub axis: Rect,
    /// The body rect the renderer draws into (content minus header, axis, footer).
    pub body: Rect,
}

impl CellRect {
    /// Splits `outer` (cell `index`) into the §5.3 bands. `has_axis` is false for renderers with no
    /// x-axis (text, gauge), which gives the axis band's height back to the body. `full` (the FULL
    /// param, §5.7) drops the header/footer to hairline minimal so the data takes the surface.
    #[must_use]
    pub fn split(index: usize, outer: Rect, m: &Metrics, has_axis: bool, full: bool) -> Self {
        let content = outer.inset(m.pad);
        if full {
            // FULL: minimal chrome — no header/footer/axis bands, the body is the whole content
            // rect (the data takes everything, §5.7). The LED still draws (a hairline dot), but the
            // bands collapse to zero so the renderer gets the surface.
            return Self {
                index,
                outer,
                content,
                header: Rect::new(content.x, content.y, content.w, 0.0),
                footer: Rect::new(content.x, content.bottom(), content.w, 0.0),
                axis: Rect::new(content.x, content.bottom(), content.w, 0.0),
                body: content,
            };
        }
        let header = content.take_top(m.header_h);
        let footer = content.take_bottom(m.footer_h);
        let axis_h = if has_axis { m.axis_h } else { 0.0 };
        let axis = Rect::new(content.x, footer.y - axis_h, content.w, axis_h);
        let body =
            Rect::new(content.x, header.bottom(), content.w, (axis.y - header.bottom()).max(0.0));
        Self { index, outer, content, header, footer, axis, body }
    }
}

/// The whole wall's layout: the display rect, the ticker band, and every cell's [`CellRect`].
#[derive(Clone, Debug, PartialEq)]
pub struct WallLayout {
    /// The full display rect (frame-context's width × height).
    pub display: Rect,
    /// The ticker band (bottom [`Metrics::ticker_h`]), or `None` when the ticker is off.
    pub ticker: Option<Rect>,
    /// The wall area the cell grid fills (display minus the ticker band).
    pub wall: Rect,
    /// The layout in force.
    pub layout: Layout,
    /// Whether SOLO is on (one cell fills the wall).
    pub solo: bool,
    /// The selected cell index (0…15) — the SOLO cell, and the cell the STREAM picker edits.
    pub selected: usize,
    /// The cell rects, in index order. In SOLO this is one cell (the selected one); otherwise it is
    /// `layout.cell_count()` cells (indices 0…count−1). Cells beyond the layout's count are not
    /// laid out (their `cell_NN` params are honestly ignored, §D7).
    pub cells: Vec<CellRect>,
}

/// One axis of a grid: `round((span − (n−1)·gap) / n)`, the §5.1 cell-size formula.
#[must_use]
pub fn cell_span(span: f32, n: usize, gap: f32) -> f32 {
    if n == 0 {
        return 0.0;
    }
    let n_f = n as f32;
    ((span - (n_f - 1.0) * gap) / n_f).round()
}

impl WallLayout {
    /// Computes the wall layout for a display rect, the layout/solo/selected/ticker params, and the
    /// per-renderer `has_axis` predicate (a cell's renderer decides whether it reserves an axis
    /// band). `full` collapses chrome (§5.7).
    //
    // The eight arguments ARE the layout inputs (display rect, the five layout-affecting params,
    // the metrics, and the per-cell axis predicate); bundling them into a struct would just move
    // the same eight fields behind a name, so they stay explicit (the schema.rs precedent).
    ///
    /// The cell grid fills the wall area (display minus the ticker band). Cell *i* maps to grid
    /// position `(col = i % cols, row = i / cols)`. In SOLO, only the selected cell is laid out,
    /// filling the whole wall area.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn compute(
        display: Rect,
        layout: Layout,
        solo: bool,
        selected: usize,
        ticker_on: bool,
        full: bool,
        m: &Metrics,
        has_axis: impl Fn(usize) -> bool,
    ) -> Self {
        let ticker = if ticker_on { Some(display.take_bottom(m.ticker_h)) } else { None };
        let wall_h = display.h - ticker.map_or(0.0, |t| t.h);
        let wall = Rect::new(display.x, display.y, display.w, wall_h.max(0.0));

        if solo {
            let sel = selected.min(crate::CELL_COUNT - 1);
            let cell = CellRect::split(sel, wall, m, has_axis(sel), full);
            return Self {
                display,
                ticker,
                wall,
                layout,
                solo: true,
                selected: sel,
                cells: vec![cell],
            };
        }

        let (cols, rows) = (layout.cols(), layout.rows());
        let cw = cell_span(wall.w, cols, m.gap);
        let ch = cell_span(wall.h, rows, m.gap);
        let count = layout.cell_count();
        let mut cells = Vec::with_capacity(count);
        for i in 0..count {
            let col = i % cols;
            let row = i / cols;
            // Centre the grid in the wall area when rounding leaves a remainder (the plan's table
            // rounds each cell to an integer px, so a few px may be left over; centring keeps the
            // wall symmetric rather than pinned to the top-left).
            let x = wall.x + (col as f32) * (cw + m.gap);
            let y = wall.y + (row as f32) * (ch + m.gap);
            let outer = Rect::new(x, y, cw, ch);
            cells.push(CellRect::split(i, outer, m, has_axis(i), full));
        }
        Self { display, ticker, wall, layout, solo: false, selected, cells }
    }

    /// The LOD the host passed, degraded one further notch in SOLO/FULL is NOT applied — the guest
    /// draws at the host's LOD (§5.7). This helper exists so the renderers ask one place "am I at
    /// least Reduced?".
    #[must_use]
    pub fn at_least(lod: Lod, level: Lod) -> bool {
        lod >= level
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    const DISPLAY: Rect = Rect::new(0.0, 0.0, 2176.0, 1120.0);

    fn no_axis(_: usize) -> bool {
        false
    }

    fn layout_cells(l: Layout) -> (f32, f32) {
        let m = Metrics::default();
        // Ticker off, solo off, so the wall == the display and the cell size is the raw formula.
        let wl = WallLayout::compute(DISPLAY, l, false, 0, false, false, &m, no_axis);
        let c = wl.cells[0].outer;
        (c.w, c.h)
    }

    #[test]
    fn the_5_1_table_is_reproduced_by_the_formula() {
        // The §5.1 cell sizes, computed not remembered. See the module header on the 4×4 height
        // erratum (the table prints 270; the plan's own gap=16 gives 268).
        assert_eq!(layout_cells(Layout::Grid2x2), (1080.0, 552.0), "2×2");
        assert_eq!(layout_cells(Layout::Grid4x2), (532.0, 552.0), "4×2");
        assert_eq!(layout_cells(Layout::Grid3x3), (715.0, 363.0), "3×3");
        assert_eq!(layout_cells(Layout::Grid4x3), (532.0, 363.0), "4×3");
        assert_eq!(
            layout_cells(Layout::Grid4x4),
            (532.0, 268.0),
            "4×4 (§5.1 prints 270; gap=16 gives 268)"
        );
    }

    #[test]
    fn cell_counts_match_the_layout_names() {
        assert_eq!(Layout::Grid2x2.cell_count(), 4);
        assert_eq!(Layout::Grid4x2.cell_count(), 8);
        assert_eq!(Layout::Grid3x3.cell_count(), 9);
        assert_eq!(Layout::Grid4x3.cell_count(), 12);
        assert_eq!(Layout::Grid4x4.cell_count(), 16);
    }

    #[test]
    fn layout_option_round_trips() {
        for (i, l) in Layout::OPTIONS.iter().enumerate() {
            assert_eq!(Layout::from_option(i), *l);
            assert_eq!(l.option(), i);
        }
        // Past the end falls back to the default (4×4), never refuses.
        assert_eq!(Layout::from_option(99), Layout::Grid4x4);
    }

    #[test]
    fn solo_fills_the_wall_with_the_selected_cell() {
        let m = Metrics::default();
        let wl = WallLayout::compute(DISPLAY, Layout::Grid4x4, true, 5, false, false, &m, no_axis);
        assert_eq!(wl.cells.len(), 1, "SOLO lays out exactly one cell");
        assert_eq!(wl.cells[0].index, 5, "…the selected one");
        assert_eq!(wl.cells[0].outer, DISPLAY, "SOLO cell fills the wall (== display, ticker off)");
    }

    #[test]
    fn the_ticker_band_comes_off_the_bottom_and_shrinks_the_wall() {
        let m = Metrics::default();
        let wl = WallLayout::compute(DISPLAY, Layout::Grid4x4, false, 0, true, false, &m, no_axis);
        let t = wl.ticker.unwrap();
        assert_eq!(t.h, 40.0, "§5.5: the ticker band is the bottom 40 px");
        assert_eq!(t.y, 1080.0);
        assert_eq!(wl.wall.h, 1080.0, "the wall area is the display minus the ticker");
    }

    #[test]
    fn cells_tile_the_wall_without_overlap() {
        let m = Metrics::default();
        let wl = WallLayout::compute(DISPLAY, Layout::Grid4x4, false, 0, false, false, &m, no_axis);
        assert_eq!(wl.cells.len(), 16);
        // Adjacent cells in a row are gap-separated, not overlapping.
        let c0 = wl.cells[0].outer;
        let c1 = wl.cells[1].outer;
        assert!(c1.x >= c0.right(), "col 1 starts at/after col 0's right edge");
        assert!((c1.x - c0.right() - m.gap).abs() < 0.5, "the gap between columns is ~16");
    }

    #[test]
    fn the_cell_anatomy_bands_stack_inside_the_content_rect() {
        let m = Metrics::default();
        let outer = Rect::new(0.0, 0.0, 532.0, 268.0);
        let c = CellRect::split(0, outer, &m, true, false);
        // content is outer inset by pad
        assert_eq!(c.content, outer.inset(m.pad));
        // header at top, footer at bottom, axis above footer, body in the middle
        assert_eq!(c.header.y, c.content.y);
        assert!((c.footer.bottom() - c.content.bottom()).abs() < 1e-3);
        assert!((c.axis.bottom() - c.footer.y).abs() < 1e-3);
        assert_eq!(c.body.y, c.header.bottom());
        assert!((c.body.bottom() - c.axis.y).abs() < 1e-3);
        assert!(c.body.h > 0.0, "the body has room to draw");
    }

    #[test]
    fn full_collapses_the_chrome_so_the_body_is_the_content() {
        let m = Metrics::default();
        let outer = Rect::new(0.0, 0.0, 2176.0, 1120.0);
        let c = CellRect::split(0, outer, &m, true, true);
        assert_eq!(c.body, c.content, "FULL: the data takes the whole surface (§5.7)");
        assert_eq!(c.header.h, 0.0);
        assert_eq!(c.footer.h, 0.0);
    }

    #[test]
    fn a_renderer_without_an_axis_gives_the_band_back_to_the_body() {
        let m = Metrics::default();
        let outer = Rect::new(0.0, 0.0, 532.0, 268.0);
        let with = CellRect::split(0, outer, &m, true, false);
        let without = CellRect::split(0, outer, &m, false, false);
        assert!(without.body.h > with.body.h, "no axis band → a taller body");
        assert_eq!(without.axis.h, 0.0);
    }

    #[test]
    fn metrics_defaults_mirror_the_checked_in_bundle() {
        // The drift gate's core-side half: these defaults ARE the bundle values (the harness test
        // reads tokens.json and asserts equality, so a token change without a Metrics change fails
        // there). Asserted here too so the core's own geometry is self-consistent.
        let m = Metrics::default();
        assert_eq!(m.gap, 16.0, "layout.space.4");
        assert_eq!(m.pad, 12.0, "layout.space.3");
        assert_eq!(m.ticker_h, 40.0, "§5.5");
        assert_eq!(m.title_px, 13.0, "typography.scale.m");
        assert_eq!(m.small_px, 11.0, "typography.scale.s");
    }
}
