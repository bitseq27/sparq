//! The top-level `draw`: params + per-cell windows + frame-context → one display list.
//!
//! This is the guest's frame entry point (the wasm glue calls it from `draw`; the harness calls it
//! directly). It owns the instrument's mutable display-instance state — the params mirror and the
//! pause-aware ticker/scroll phase — and the embedded coastline, and it assembles the wall: the cell
//! grid (§5.1), each cell's body (dispatched to a renderer) + chrome (§5.3), the selection halo, and
//! the global ticker band (§5.5). It applies the §5.7 LOD ladder and counts the frame against the
//! `gpu_class` ceilings (§5.8) so the harness can assert the budget without a runtime.
//!
//! # Clocks (D8/D11)
//!
//! The only clock read here is `frame.time_sec` (the UI animation clock, legal because a display is
//! never replayed). It advances the scroll phase — and ONLY while unpaused, so PAUSE freezes the
//! ticker and the text cells. Data is positioned from the records' own stamps, never from this clock.

use crate::coastline::Coastline;
use crate::ir::{Item, Lod, Style, Surface};
use crate::layout::{Metrics, Rect, WallLayout};
use crate::params::Params;
use crate::record::DataRecord;
use crate::render::{self, glyph, text_width, RenderCtx};
use crate::streams_table;
use crate::view::{build_view, CellStatus, CellView};

/// What one `draw` call receives (the pure mirror of display.wit's `frame-context`; the wasm glue
/// builds it from the contract record, the harness from its fixtures).
#[derive(Clone, Debug, PartialEq)]
pub struct FrameContext {
    /// Which declared display this draws (the Observatory has one: `wall`).
    pub display_id: String,
    /// Logical px width of the display rect.
    pub width_px: f32,
    /// Logical px height.
    pub height_px: f32,
    /// The host's LOD level (§5.7).
    pub lod: Lod,
    /// The UI animation clock, seconds (animation phase only — D8).
    pub time_sec: f64,
}

/// One cell's served window, passed in by the host side (glue/harness). The stream a cell shows comes
/// from the params (the guest resolves its own `cell_NN` enum), so this carries only the freshness
/// and the records.
#[derive(Clone, Copy, Debug)]
pub struct CellInput<'a> {
    /// The host-computed freshness/key state (the core has no clock and no env, D8/D14).
    pub status: CellStatus,
    /// The records the host served for this cell's `cell-NN` source binding (a window, oldest→newest).
    pub records: &'a [DataRecord],
}

/// The instrument's display-instance state.
#[derive(Clone, Debug)]
pub struct Wall {
    /// The current params (mirrored from the audio instance's state stream — see the wasm glue).
    pub params: Params,
    /// The layout metrics (token-derived; the glue may refresh from the runtime bundle).
    pub metrics: Metrics,
    /// The embedded coastline (D12).
    coast: Coastline,
    /// The pause-aware accumulated scroll phase, seconds.
    ticker_phase: f64,
    /// The last frame's `time_sec`, to compute the phase delta (None until the first frame).
    last_time: Option<f64>,
}

impl Default for Wall {
    fn default() -> Self {
        Self::new()
    }
}

impl Wall {
    /// Builds the wall with default params, the token-default metrics, and the embedded coastline.
    /// A coastline parse failure is a build defect (the asset is compiled in); it degrades to an
    /// empty coastline so the instrument still draws (maps without land) rather than refusing to
    /// exist — the harness surfaces the parse error loudly in its own tests.
    #[must_use]
    pub fn new() -> Self {
        let coast = Coastline::from_asset().unwrap_or_else(|_| Coastline::empty());
        Self {
            params: Params::default(),
            metrics: Metrics::default(),
            coast,
            ticker_phase: 0.0,
            last_time: None,
        }
    }

    /// Builds the wall with explicit params + metrics (the harness/glue path).
    #[must_use]
    pub fn with_params(params: Params, metrics: Metrics) -> Self {
        let coast = Coastline::from_asset().unwrap_or_else(|_| Coastline::empty());
        Self { params, metrics, coast, ticker_phase: 0.0, last_time: None }
    }

    /// The accumulated scroll phase (seconds) — exposed so the state blob can persist it.
    #[must_use]
    pub fn ticker_phase(&self) -> f64 {
        self.ticker_phase
    }

    /// Sets the scroll phase (state restore).
    pub fn set_ticker_phase(&mut self, phase: f64) {
        self.ticker_phase = if phase.is_finite() { phase } else { 0.0 };
    }

    /// Advances the scroll phase by the frame delta × ticker_speed, only while unpaused (§5.5). The
    /// first frame establishes the baseline without advancing (no jump from a zero phase).
    fn advance_phase(&mut self, time_sec: f64) {
        if let Some(last) = self.last_time {
            if !self.params.pause {
                let dt = (time_sec - last).max(0.0);
                if dt.is_finite() {
                    self.ticker_phase += dt * f64::from(self.params.ticker_speed);
                }
            }
        }
        self.last_time = Some(time_sec);
        // Keep the phase bounded (it is a scroll offset; wrapping is the renderer's job). A runaway
        // f64 would eventually lose precision, so fold it at a large-but-finite horizon.
        if self.ticker_phase > 1e9 {
            self.ticker_phase = 0.0;
        }
    }

    /// Draws one frame. `cells` is indexed by cell number (0…15); a cell beyond the array (or an
    /// absent input) reads as OFFLINE with no records. Returns the display list as a [`Surface`].
    #[must_use]
    pub fn draw(&mut self, frame: &FrameContext, cells: &[CellInput<'_>]) -> Surface {
        self.advance_phase(frame.time_sec);
        let ctx = RenderCtx {
            params: &self.params,
            metrics: &self.metrics,
            lod: frame.lod,
            time_sec: frame.time_sec,
            scroll_phase: self.ticker_phase,
            coast: &self.coast,
        };
        let display = Rect::new(0.0, 0.0, frame.width_px.max(1.0), frame.height_px.max(1.0));

        // `has_axis` per cell comes from its stream's view hint (the layout reserves the axis band
        // only for renderers that draw one).
        let params = self.params;
        let layout = WallLayout::compute(
            display,
            params.layout,
            params.solo,
            params.selected_cell,
            params.ticker,
            params.full,
            &self.metrics,
            |i| cell_has_axis(&params, i),
        );

        let mut items: Vec<Item> = Vec::new();
        // Collect the LIVE cells' headlines for the ticker band (newest first).
        let mut ticker_parts: Vec<(String, String)> = Vec::new();

        for cell in &layout.cells {
            let idx = cell.index;
            let stream_id = params.cell_stream(idx);
            let meta = stream_id.and_then(streams_table::by_id);
            let input = cells.get(idx);
            let status = input.map_or(CellStatus::Offline, |c| c.status);
            let records: &[DataRecord] = input.map_or(&[], |c| c.records);
            let view: CellView = build_view(meta, status, records, stream_id);

            // Draw order: ground (under) → body (data) → chrome (over). The ground is a background;
            // painting it after the body would bury the render (the bug this order fixes).
            items.extend(render::cell_ground(cell, &ctx));
            items.extend(render::render_body(&view, cell, &ctx));
            items.extend(render::cell_chrome(&view, cell, &ctx, idx == params.selected_cell));

            // The ticker merges every LIVE cell's newest headline, prefixed by its stream label.
            if status == CellStatus::Live {
                if let Some(h) = &view.chrome.headline {
                    if !h.is_empty() {
                        let label = meta.map_or(stream_id.unwrap_or("?"), |m| m.label);
                        ticker_parts.push((short_label(label), h.clone()));
                    }
                }
            }
        }

        // The global ticker band (§5.5) — survives all LOD levels (the cheapest, most legible element).
        if params.ticker {
            if let Some(band) = layout.ticker {
                items.extend(render_ticker(&ticker_parts, band, &ctx));
            }
        }

        // §5.7 minimal-LOD budget safety: if the wall is still heavy at Minimal, fall back to WORDS
        // per cell rather than draw a dense wall the host would degrade anyway.
        if frame.lod == Lod::Minimal && items.len() >= 500 {
            items = minimal_words(&layout, &ctx);
        }

        Surface::Items(items)
    }
}

/// Whether cell `i`'s renderer reserves an x-axis band (the layout's `has_axis` predicate).
fn cell_has_axis(params: &Params, i: usize) -> bool {
    params
        .cell_stream(i)
        .and_then(streams_table::by_id)
        .is_some_and(|m| render::view_has_axis(m.view))
}

/// A stream label shortened for the ticker prefix (the full labels are long; the ticker wants a
/// compact source tag). Takes the part before an em-dash/parenthesis, uppercased.
fn short_label(label: &str) -> String {
    let cut = label.split(['—', '(']).next().unwrap_or(label).trim();
    cut.to_uppercase()
}

/// The §5.7 minimal-LOD WORDS fallback: one glyph per cell ("N cells — zoom in for data"), the whole
/// wall reduced to words when it cannot degrade cheaply.
fn minimal_words(layout: &WallLayout, ctx: &RenderCtx<'_>) -> Vec<Item> {
    let mut out = Vec::new();
    for cell in &layout.cells {
        let body = cell.body;
        let words = "zoom in for data";
        let x = body.cx() - text_width(words, ctx.metrics.small_px) / 2.0;
        out.push(glyph(words, x, body.cy(), render::T_SCALE_S, render::T_TEXT_DISABLED));
    }
    out
}

/// The global ticker band (§5.5): one scrolling marquee merging the LIVE cells' headlines, each
/// prefixed by its stream label, separated by the class dash. Scrolling rides the pause-aware phase;
/// PAUSE shows the ❚❚ glyph at the band's left (§5.7).
#[must_use]
pub fn render_ticker(parts: &[(String, String)], band: Rect, ctx: &RenderCtx<'_>) -> Vec<Item> {
    let mut out = Vec::new();
    // The band ground (an overlay tint, §5.6 rule 2) + a hairline top edge.
    out.push(crate::render::fill_rect(band, render::T_GROUND_OVERLAY, crate::ir::Corner::C0));
    out.push(render::line(
        band.x,
        band.y,
        band.right(),
        band.y,
        Style::hairline(render::T_HAIRLINE),
    ));

    let font = ctx.metrics.small_px;
    let baseline = band.y + band.h / 2.0 + font / 3.0;
    let mut left = band.x + 6.0;

    // PAUSE indicator (host glyph vocabulary, §5.7).
    if ctx.params.pause {
        out.push(glyph("❚❚", left, baseline, render::T_SCALE_S, render::T_STATE_STALE));
        left += text_width("❚❚ ", font);
    }

    if parts.is_empty() {
        let words = "no live cells — the ticker merges LIVE headlines";
        out.push(glyph(words, left, baseline, render::T_SCALE_S, render::T_TEXT_TERTIARY));
        return out;
    }

    // The merged marquee text: "◂ LABEL  headline · LABEL  headline · …".
    let mut merged = String::from("◂ ");
    for (i, (label, text)) in parts.iter().enumerate() {
        if i > 0 {
            merged.push_str(" · ");
        }
        merged.push_str(label);
        merged.push_str("  ");
        merged.push_str(text);
    }
    merged.push_str(" ◂");

    let w = text_width(&merged, font);
    let span = band.w - (left - band.x);
    // Scroll left, looping over (text + span) so it re-enters continuously.
    let loop_w = (w + span).max(1.0);
    let offset = (ctx.scroll_phase * render::text::PX_PER_SEC).rem_euclid(f64::from(loop_w));
    let x = left - offset as f32;
    out.push(glyph(merged.clone(), x, baseline, render::T_SCALE_S, render::T_TEXT_SECONDARY));
    // A second copy one loop-width right, so the band never shows a gap while the first scrolls out.
    if w < span * 2.0 {
        out.push(glyph(merged, x + loop_w, baseline, render::T_SCALE_S, render::T_TEXT_SECONDARY));
    }
    out
}

/// The per-frame budget against the `gpu_class` ceilings (§5.8): counts vertices, instances and heat
/// cells the way `sparq-host-wasm/ceilings.rs` budgets them, so the harness can assert the frame fits
/// the declared class WITHOUT a runtime. The counting convention (documented for INC4's painter to
/// match):
/// * polyline/path/trace → their point/segment/value count as VERTICES;
/// * points → position count as INSTANCES (point sprites);
/// * glyph-run → 1 INSTANCE;
/// * rect → 4 vertices; arc → 32 vertices (a tessellated sweep);
/// * heat-cells → cols × rows as HEAT CELLS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Budget {
    /// Total vertices (points/lines/paths/traces/rects/arcs).
    pub vertices: u32,
    /// Total instances (glyph runs + point sprites).
    pub instances: u32,
    /// Total heat cells.
    pub heat_cells: u32,
}

/// Counts a display list's budget.
#[must_use]
pub fn count_budget(items: &[Item]) -> Budget {
    let mut b = Budget::default();
    for it in items {
        match it {
            Item::Polyline(p) => b.vertices += p.points.len() as u32,
            Item::Path(p) => b.vertices += p.segments.len() as u32,
            Item::Trace(t) => b.vertices += t.values.len() as u32,
            Item::Rect(_) => b.vertices += 4,
            Item::Arc(_) => b.vertices += 32,
            Item::GlyphRun(_) => b.instances += 1,
            Item::Points(p) => b.instances += p.positions.len() as u32,
            Item::HeatCells(h) => b.heat_cells += h.cols.saturating_mul(h.rows),
        }
    }
    b
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::record::{DataValue, TIMESERIES_ID};

    fn ts_window(n: usize) -> Vec<DataRecord> {
        (0..n)
            .map(|i| DataRecord {
                schema_id: TIMESERIES_ID.into(),
                schema_version: 1,
                channels: vec![
                    DataValue::Int64(1_700_000_000 + i as i64 * 60),
                    DataValue::Double(300.0 + i as f64),
                    DataValue::Double(1.0),
                ],
                t_wall_ns: 0,
                t_sample: None,
                seq: 0,
                record_flags: crate::record::DataFlags::Ok,
            })
            .collect()
    }

    fn frame(w: f32, h: f32, lod: Lod, t: f64) -> FrameContext {
        FrameContext { display_id: "wall".into(), width_px: w, height_px: h, lod, time_sec: t }
    }

    #[test]
    fn a_default_wall_draws_items_at_its_min_size() {
        let mut wall = Wall::new();
        // 16 cells, all fed a timeseries window (a stand-in; the defaults pick real streams).
        let win = ts_window(40);
        let inputs: Vec<CellInput<'_>> =
            (0..16).map(|_| CellInput { status: CellStatus::Live, records: &win }).collect();
        let surface = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 0.0), &inputs);
        match surface {
            Surface::Items(items) => {
                assert!(items.len() > 16, "a 16-cell wall draws many primitives")
            },
            Surface::Scene(_) => panic!("the Observatory is display-list only"),
        }
    }

    #[test]
    fn the_ticker_phase_advances_only_when_unpaused() {
        let mut wall = Wall::new();
        let inputs: Vec<CellInput<'_>> =
            (0..16).map(|_| CellInput { status: CellStatus::Offline, records: &[] }).collect();
        let _ = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 0.0), &inputs);
        let _ = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 2.0), &inputs);
        let moving = wall.ticker_phase();
        assert!(moving > 0.0, "the phase advances while unpaused");
        // Pause and advance the clock: the phase must not move.
        wall.params.pause = true;
        let before = wall.ticker_phase();
        let _ = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 10.0), &inputs);
        assert_eq!(wall.ticker_phase(), before, "PAUSE freezes the scroll phase (§5.5)");
    }

    #[test]
    fn the_first_frame_does_not_jump_the_phase() {
        let mut wall = Wall::new();
        let inputs: Vec<CellInput<'_>> =
            (0..16).map(|_| CellInput { status: CellStatus::Offline, records: &[] }).collect();
        let _ = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 1000.0), &inputs);
        assert_eq!(wall.ticker_phase(), 0.0, "the first frame establishes the baseline, no jump");
    }

    #[test]
    fn a_solo_wall_draws_one_cell() {
        let mut wall = Wall::new();
        wall.params.solo = true;
        wall.params.selected_cell = 3;
        let win = ts_window(20);
        let inputs: Vec<CellInput<'_>> =
            (0..16).map(|_| CellInput { status: CellStatus::Live, records: &win }).collect();
        let surface = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 0.0), &inputs);
        // A solo wall has one cell's worth of chrome (one title, one LED) — count the LED arcs.
        if let Surface::Items(items) = surface {
            let leds = items
                .iter()
                .filter(|i| matches!(i, Item::Arc(a) if (a.a1_deg - a.a0_deg - 360.0).abs() < 0.5))
                .count();
            assert_eq!(leds, 1, "SOLO draws exactly one cell's LED, got {leds}");
        }
    }

    #[test]
    fn the_budget_counts_heat_cells_and_instances() {
        let mut wall = Wall::new();
        // Feed the aurora (a grid) into several cells to exercise the heat-cell budget.
        let grid = vec![DataRecord {
            schema_id: crate::record::GRID_ID.into(),
            schema_version: 1,
            channels: vec![
                DataValue::Int32(180),
                DataValue::Int32(90),
                DataValue::Double(89.0),
                DataValue::Double(-180.0),
                DataValue::Double(2.0),
                DataValue::Vec(vec![1.0; 180 * 90]),
                DataValue::Int64(1_700_000_000),
                DataValue::Double(1.0),
            ],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: crate::record::DataFlags::Ok,
        }];
        // Put the aurora in cell 0 (its default), leave the rest live with a series.
        let win = ts_window(20);
        let mut inputs: Vec<CellInput<'_>> =
            (0..16).map(|_| CellInput { status: CellStatus::Live, records: &win }).collect();
        inputs[0] = CellInput { status: CellStatus::Live, records: &grid };
        let surface = wall.draw(&frame(2176.0, 1120.0, Lod::Full, 0.0), &inputs);
        if let Surface::Items(items) = surface {
            let b = count_budget(&items);
            assert!(b.heat_cells >= 180 * 90, "the aurora grid is counted, got {}", b.heat_cells);
            assert!(b.instances > 0, "glyph runs / points are counted");
        }
    }

    #[test]
    fn short_label_takes_the_part_before_the_dash() {
        assert_eq!(short_label("Aurora forecast — OVATION grid"), "AURORA FORECAST");
        assert_eq!(short_label("Earthquakes — past hour (USGS)"), "EARTHQUAKES");
    }
}
