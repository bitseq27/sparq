//! `observatory-wasm` — the SDK glue for `dat/observatory` (WO-020 plan D13).
//!
//! This is the thin (~300-line) boundary between the frozen contract and the pure core. It does
//! three jobs and no logic of its own:
//!
//! 1. **Implements [`InstrumentModule`]** — the contract's export face — over a [`Wall`] (the core's
//!    display-instance state). `process` is a no-op that returns the exact negotiated (empty — the
//!    instrument is port-less, D5) shapes and mirrors the param snapshot into the wall; `draw`
//!    fetches each cell's bound stream window through the `sources` door and calls the core.
//! 2. **Converts contract types → core types** at the input edge: the WIT `frame-context`, the
//!    `data-record` window from `sources.snapshot`, and the LOD enum.
//! 3. **Converts core IR → contract types** at the output edge: the display-list [`Item`]s the core
//!    emits become the SDK's generated `display::Item`s, field for field.
//!
//! The conversions are total and mechanical; every one is unit-tested natively below (the SDK
//! compiles for the host, and `sparq_instrument!`'s component export is `#[cfg(wasm32)]`-gated, so
//! `cargo test -p observatory-wasm` exercises the glue without a runtime — D13's whole point).
//!
//! # Two instances, one component (instrument-host §4)
//!
//! The host runs an AUDIO instance (`process`, authoritative for params/state — `save_state` is
//! called here) and a DISPLAY instance (`draw`). They share nothing mutable; the host keeps them
//! coherent by applying the same `configure` bytes to both. So `process` mirrors params into the
//! wall (and `save_state` serialises them), while `configure` applies params to whichever instance
//! receives it. The ticker scroll phase is display-local animation and is NOT persisted (see
//! `observatory_core::state`).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use observatory_core::ir as core_ir;
use observatory_core::record as core_rec;
use observatory_core::view::CellStatus;
use observatory_core::wall::{CellInput, FrameContext as CoreFrame, Wall};
use observatory_core::{params::Params, state::State, CELL_COUNT, DISPLAY_ID, INSTRUMENT_ID};

use sparq::bindings::sparq::instrument::display as d;
use sparq::bindings::sparq::instrument::types as t;
use sparq::{
    BlockInput, BlockOutput, BlockStatus, FrameContext, InstrumentModule, Lod, ModuleError,
    Resources, StreamSnapshot, Surface,
};
use sparq_module_guest as sparq;

/// The instrument: one wall of live Earth-and-space data. All the logic is in the core; this struct
/// is the contract face plus the display-instance state.
#[derive(Default)]
pub struct Observatory {
    wall: Wall,
    /// The negotiated output shapes are empty (port-less, D5), but `prepare` records that it ran so
    /// a `draw` before `prepare` can be refused honestly rather than drawing an uninitialised wall.
    prepared: bool,
}

impl InstrumentModule for Observatory {
    fn id(&self) -> String {
        INSTRUMENT_ID.to_string()
    }

    fn prepare(&mut self, resources: Resources) -> Result<(), ModuleError> {
        // All allocation happens here (the wall parses the embedded coastline). Idempotent: a
        // re-prepare (any resources field changed) rebuilds the wall cleanly.
        if resources.block_frames == 0 {
            return Err(ModuleError::Resources("block-frames must be > 0".into()));
        }
        self.wall = Wall::new();
        self.prepared = true;
        Ok(())
    }

    fn process(&mut self, input: BlockInput) -> BlockOutput {
        // Mirror the param snapshot into the wall (the audio instance is authoritative for params;
        // save_state serialises these). No audio path (D5): the output is the exact negotiated shape
        // for zero ports — empty lists — with status Ok (the block was produced as declared).
        self.wall.params = Params::from_values(&input.params.values);
        BlockOutput {
            status: BlockStatus::Ok,
            audio_out: Vec::new(),
            cv_out: Vec::new(),
            events_out: Vec::new(),
            data_out: Vec::new(),
        }
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(()); // no state to apply — the defaults stand
        }
        match State::restore(state) {
            Ok(s) => {
                self.wall.params = s.params;
                Ok(())
            },
            // Refuse in words (the contract's rule): a blob we cannot honour is not silently ignored.
            Err(e) => Err(ModuleError::State(e.to_string())),
        }
    }

    fn save_state(&mut self) -> Vec<u8> {
        State { params: self.wall.params }.save()
    }

    fn draw(&mut self, frame: FrameContext) -> Surface {
        if !self.prepared {
            // A draw before prepare is a host-ordering bug; the honest answer is an empty (at-rest)
            // surface, never a half-built wall. (The host calls prepare first; this is belt-and-braces.)
            return Surface::Items(Vec::new());
        }
        let core_frame = to_core_frame(&frame);
        // Fetch each cell's bound stream window through the sources door (D4: the host resolved the
        // `param:cell_NN` binding to a registry id and served that stream's window).
        let windows: Vec<Option<Vec<core_rec::DataRecord>>> =
            (0..CELL_COUNT).map(|i| fetch_cell(&format!("cell-{:02}", i + 1))).collect();
        let inputs: Vec<CellInput<'_>> = (0..CELL_COUNT)
            .map(|i| {
                let stream_id = self.wall.params.cell_stream(i);
                let needs_key = stream_id
                    .and_then(observatory_core::streams_table::by_id)
                    .is_some_and(|m| m.requires_key());
                let status = cell_status(&windows[i], needs_key);
                CellInput { status, records: windows[i].as_deref().unwrap_or(&[]) }
            })
            .collect();
        let surface = self.wall.draw(&core_frame, &inputs);
        to_sdk_surface(&surface)
    }
}

/// Fetches one cell's stream window through the `sources` door, converting the contract's
/// `data-record`s into core records. `None` = the binding is unconnected/stale-expired (the host's
/// at-rest signal) — the cell draws WORDS, never garbage.
fn fetch_cell(source_id: &str) -> Option<Vec<core_rec::DataRecord>> {
    match sparq::source(DISPLAY_ID, source_id) {
        Some(StreamSnapshot::Window(records)) => Some(records.iter().map(to_core_record).collect()),
        // A stream binding never serves a ring or a scalar (those are port/analysis bindings); if
        // the host handed one, it is a mis-binding — treat it as no data (draw at rest), not a guess.
        Some(_) => None,
        None => None,
    }
}

/// Derives the cell's freshness/key status from its served window. The core has no clock and no env
/// (D8/D14), so the glue reads the host's signals: no window (or an empty one) + a key-requiring
/// stream = KEY NEEDED; no window otherwise = OFFLINE; a window with a Stale flag = STALE; else LIVE.
fn cell_status(window: &Option<Vec<core_rec::DataRecord>>, needs_key: bool) -> CellStatus {
    match window {
        Some(records) if !records.is_empty() => {
            let stale = records.iter().any(|r| r.record_flags == core_rec::DataFlags::Stale);
            if stale {
                CellStatus::Stale
            } else {
                CellStatus::Live
            }
        },
        // No window, or an empty one: the stream is not delivering. Name the fix if it needs a key.
        _ => {
            if needs_key {
                CellStatus::KeyNeeded(key_env_or_default())
            } else {
                CellStatus::Offline
            }
        },
    }
}

/// The KEY NEEDED words name the env var to set. v1's only key-requiring stream in the default set
/// is FIRMS; the glue reports the stream's own `key_env` where the core table carries it, else the
/// generic name. (The precise per-cell env is a harness/host concern; the guest shows the fix.)
fn key_env_or_default() -> &'static str {
    "SPARQ_FIRMS_KEY"
}

// ── contract → core conversions (the input edge) ────────────────────────────────────────────────

/// Converts the contract's `frame-context` into the core's.
fn to_core_frame(f: &FrameContext) -> CoreFrame {
    CoreFrame {
        display_id: f.display_id.clone(),
        width_px: f.width_px,
        height_px: f.height_px,
        lod: to_core_lod(f.lod),
        time_sec: f.time_sec,
    }
}

/// Converts the contract's LOD enum into the core's (by name — the core's variant ORDER is
/// detail-ascending for its `>=` comparisons, which the WIT order is not; the mapping is by name).
fn to_core_lod(lod: Lod) -> core_ir::Lod {
    match lod {
        Lod::Full => core_ir::Lod::Full,
        Lod::Reduced => core_ir::Lod::Reduced,
        Lod::Minimal => core_ir::Lod::Minimal,
    }
}

/// Converts one contract `data-record` into a core record (field-for-field; the carriers are closed
/// and identical, so this is total).
fn to_core_record(r: &t::DataRecord) -> core_rec::DataRecord {
    core_rec::DataRecord {
        schema_id: r.schema_id.clone(),
        schema_version: r.schema_version,
        channels: r.channels.iter().map(to_core_value).collect(),
        t_wall_ns: r.t_wall_ns,
        t_sample: r.t_sample,
        seq: r.seq,
        record_flags: to_core_flags(r.record_flags),
    }
}

/// Converts one contract `data-value` into a core value.
fn to_core_value(v: &t::DataValue) -> core_rec::DataValue {
    match v {
        t::DataValue::Single(x) => core_rec::DataValue::Single(*x),
        t::DataValue::Double(x) => core_rec::DataValue::Double(*x),
        t::DataValue::Int32(n) => core_rec::DataValue::Int32(*n),
        t::DataValue::Int64(n) => core_rec::DataValue::Int64(*n),
        t::DataValue::Boolean(b) => core_rec::DataValue::Boolean(*b),
        t::DataValue::Vec(xs) => core_rec::DataValue::Vec(xs.clone()),
        t::DataValue::Enumerated(n) => core_rec::DataValue::Enumerated(*n),
        t::DataValue::Text(s) => core_rec::DataValue::Text(s.clone()),
        t::DataValue::Blob(b) => core_rec::DataValue::Blob(b.clone()),
    }
}

/// Converts the contract's record flags into the core's.
fn to_core_flags(f: t::DataFlags) -> core_rec::DataFlags {
    match f {
        t::DataFlags::Ok => core_rec::DataFlags::Ok,
        t::DataFlags::Stale => core_rec::DataFlags::Stale,
        t::DataFlags::Discontinuity => core_rec::DataFlags::Discontinuity,
        t::DataFlags::Estimated => core_rec::DataFlags::Estimated,
    }
}

// ── core → contract conversions (the output edge) ───────────────────────────────────────────────

/// Converts a core [`Surface`] into the contract's. The Observatory only emits display lists; a
/// scene (never built by the core in v1) maps to an empty list rather than a half-scene.
fn to_sdk_surface(s: &observatory_core::ir::Surface) -> Surface {
    match s {
        observatory_core::ir::Surface::Items(items) => {
            Surface::Items(items.iter().map(to_sdk_item).collect())
        },
        observatory_core::ir::Surface::Scene(_) => Surface::Items(Vec::new()),
    }
}

/// Converts one core IR item into the contract's `display::Item` (field-for-field; both vocabularies
/// are the frozen display.wit v1, so this is total).
fn to_sdk_item(item: &core_ir::Item) -> d::Item {
    match item {
        core_ir::Item::Polyline(p) => d::Item::Polyline(d::PolylineItem {
            points: p.points.iter().map(to_sdk_point).collect(),
            style: to_sdk_style(&p.style),
        }),
        core_ir::Item::Path(p) => d::Item::Path(d::PathItem {
            segments: p.segments.iter().map(to_sdk_segment).collect(),
            closed: p.closed,
            style: to_sdk_style(&p.style),
        }),
        core_ir::Item::Rect(r) => d::Item::Rect(d::RectItem {
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            style: to_sdk_style(&r.style),
        }),
        core_ir::Item::Arc(a) => d::Item::Arc(d::ArcItem {
            cx: a.cx,
            cy: a.cy,
            r: a.r,
            a0_deg: a.a0_deg,
            a1_deg: a.a1_deg,
            style: to_sdk_style(&a.style),
        }),
        core_ir::Item::GlyphRun(g) => d::Item::GlyphRun(d::GlyphRunItem {
            text: g.text.clone(),
            x: g.x,
            y: g.y,
            size: g.size.clone(),
            colour: g.colour.clone(),
        }),
        core_ir::Item::Points(p) => d::Item::Points(d::PointsItem {
            positions: p.positions.iter().map(to_sdk_point).collect(),
            size: p.size.clone(),
            colour: p.colour.clone(),
            colormap: p.colormap.clone(),
            values: p.values.clone(),
        }),
        core_ir::Item::HeatCells(h) => d::Item::HeatCells(d::HeatCellsItem {
            x: h.x,
            y: h.y,
            w: h.w,
            h: h.h,
            cols: h.cols,
            rows: h.rows,
            values: h.values.clone(),
            colormap: h.colormap.clone(),
        }),
        core_ir::Item::Trace(t) => d::Item::Trace(d::TraceItem {
            values: t.values.clone(),
            colour: t.colour.clone(),
            width: to_sdk_width(t.width),
            phosphor: t.phosphor,
        }),
    }
}

fn to_sdk_point(p: &core_ir::Point) -> d::Point {
    d::Point { x: p.x, y: p.y }
}

fn to_sdk_segment(s: &core_ir::PathSegment) -> d::PathSegment {
    match s {
        core_ir::PathSegment::MoveTo(p) => d::PathSegment::MoveTo(to_sdk_point(p)),
        core_ir::PathSegment::LineTo(p) => d::PathSegment::LineTo(to_sdk_point(p)),
        core_ir::PathSegment::QuadTo(a, b) => {
            d::PathSegment::QuadTo((to_sdk_point(a), to_sdk_point(b)))
        },
        core_ir::PathSegment::CubicTo(a, b, c) => {
            d::PathSegment::CubicTo((to_sdk_point(a), to_sdk_point(b), to_sdk_point(c)))
        },
    }
}

fn to_sdk_style(s: &core_ir::Style) -> d::Style {
    d::Style {
        stroke: s.stroke.clone(),
        stroke_width: to_sdk_width(s.stroke_width),
        fill: s.fill.clone(),
        dash: to_sdk_dash(s.dash),
        corner: to_sdk_corner(s.corner),
    }
}

fn to_sdk_width(w: core_ir::StrokeWidth) -> d::StrokeWidth {
    match w {
        core_ir::StrokeWidth::W1 => d::StrokeWidth::W1,
        core_ir::StrokeWidth::W2 => d::StrokeWidth::W2,
        core_ir::StrokeWidth::W3 => d::StrokeWidth::W3,
    }
}

fn to_sdk_dash(x: core_ir::Dash) -> d::Dash {
    match x {
        core_ir::Dash::Solid => d::Dash::Solid,
        core_ir::Dash::Dashed => d::Dash::Dashed,
        core_ir::Dash::Dotted => d::Dash::Dotted,
        core_ir::Dash::Hidden => d::Dash::Hidden,
    }
}

fn to_sdk_corner(c: core_ir::Corner) -> d::Corner {
    match c {
        core_ir::Corner::C0 => d::Corner::C0,
        core_ir::Corner::C2 => d::Corner::C2,
        core_ir::Corner::C4 => d::Corner::C4,
    }
}

sparq::sparq_instrument!(Observatory);

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use observatory_core::ir::{Corner, Dash, StrokeWidth, Style};

    #[test]
    fn the_id_matches_the_manifest() {
        // The host cross-checks id() against identity.id; a lie breaks patch provenance silently.
        assert_eq!(Observatory::default().id(), "dat/observatory");
        assert_eq!(INSTRUMENT_ID, "dat/observatory");
    }

    #[test]
    fn prepare_refuses_a_zero_block_and_initialises() {
        let mut o = Observatory::default();
        let bad = Resources { block_frames: 0, ..test_resources() };
        assert!(matches!(o.prepare(bad), Err(ModuleError::Resources(_))));
        let ok = Resources { block_frames: 64, ..test_resources() };
        assert!(o.prepare(ok).is_ok());
        assert!(o.prepared);
    }

    #[test]
    fn process_is_a_portless_noop_that_mirrors_params() {
        let mut o = Observatory::default();
        o.prepare(test_resources()).unwrap();
        let mut input = empty_block_input();
        // Set cell 0's param to option 2 (a stream) and check it lands in the wall.
        input.params.values[0] = 2.0;
        let out = o.process(input);
        assert_eq!(out.status, BlockStatus::Ok);
        assert!(
            out.audio_out.is_empty() && out.cv_out.is_empty(),
            "port-less: empty negotiated shapes"
        );
        assert_eq!(o.wall.params.cells[0], 2);
    }

    #[test]
    fn state_round_trips_through_configure_and_save() {
        let mut o = Observatory::default();
        o.prepare(test_resources()).unwrap();
        let mut s = State::default();
        s.params.cells[3] = 5;
        s.params.layout = observatory_core::layout::Layout::Grid3x3;
        o.configure(&s.save()).unwrap();
        assert_eq!(o.wall.params.cells[3], 5);
        assert_eq!(o.wall.params.layout, observatory_core::layout::Layout::Grid3x3);
        // save_state re-emits the same params.
        let again = State::restore(&o.save_state()).unwrap();
        assert_eq!(again.params.cells[3], 5);
    }

    #[test]
    fn configure_refuses_a_bad_blob_in_words() {
        let mut o = Observatory::default();
        o.prepare(test_resources()).unwrap();
        let e = o.configure(&[1, 2, 3]).unwrap_err();
        assert!(matches!(e, ModuleError::State(_)), "a bad blob is refused, not ignored");
    }

    #[test]
    fn configure_accepts_an_empty_blob_as_no_state() {
        let mut o = Observatory::default();
        o.prepare(test_resources()).unwrap();
        assert!(o.configure(&[]).is_ok());
    }

    #[test]
    fn ir_to_sdk_item_conversion_is_total_and_faithful() {
        // One of each primitive the core can emit; assert the SDK mirror carries every field.
        let items = [
            core_ir::Item::Rect(core_ir::RectItem {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                style: Style::fill("color.ground.inset"),
            }),
            core_ir::Item::GlyphRun(core_ir::GlyphRunItem {
                text: "hi".into(),
                x: 5.0,
                y: 6.0,
                size: "typography.scale.s".into(),
                colour: "color.text.primary".into(),
            }),
            core_ir::Item::Arc(core_ir::ArcItem {
                cx: 0.0,
                cy: 0.0,
                r: 5.0,
                a0_deg: 0.0,
                a1_deg: 360.0,
                style: Style::fill("color.signal.data.colour"),
            }),
            core_ir::Item::Points(core_ir::PointsItem {
                positions: vec![core_ir::Point::new(1.0, 1.0)],
                size: "layout.marker.radius_m".into(),
                colour: None,
                colormap: Some("colormap.inferno-class".into()),
                values: Some(vec![0.5]),
            }),
            core_ir::Item::HeatCells(core_ir::HeatCellsItem {
                x: 0.0,
                y: 0.0,
                w: 10.0,
                h: 10.0,
                cols: 2,
                rows: 2,
                values: vec![0.0, 1.0, f32::NAN, 0.5],
                colormap: "colormap.thermal".into(),
            }),
            core_ir::Item::Polyline(core_ir::PolylineItem {
                points: vec![core_ir::Point::new(0.0, 0.0), core_ir::Point::new(1.0, 1.0)],
                style: Style::trace("color.signal.data.colour"),
            }),
            core_ir::Item::Trace(core_ir::TraceItem {
                values: vec![0.0, 1.0],
                colour: "color.signal.data.colour".into(),
                width: StrokeWidth::W2,
                phosphor: true,
            }),
            core_ir::Item::Path(core_ir::PathItem {
                segments: vec![
                    core_ir::PathSegment::MoveTo(core_ir::Point::new(0.0, 0.0)),
                    core_ir::PathSegment::LineTo(core_ir::Point::new(1.0, 1.0)),
                ],
                closed: true,
                style: Style {
                    stroke: Some("color.hairline.colour".into()),
                    stroke_width: StrokeWidth::W1,
                    fill: Some("color.ground.panel_alt".into()),
                    dash: Dash::Solid,
                    corner: Corner::C0,
                },
            }),
        ];
        let converted: Vec<d::Item> = items.iter().map(to_sdk_item).collect();
        assert_eq!(converted.len(), items.len(), "every core item maps to one SDK item");
        // Spot-check a field survives (the rect geometry and the glyph text).
        match &converted[0] {
            d::Item::Rect(r) => assert_eq!((r.x, r.y, r.w, r.h), (1.0, 2.0, 3.0, 4.0)),
            _ => panic!("rect mapped wrong"),
        }
        match &converted[1] {
            d::Item::GlyphRun(g) => assert_eq!(g.text, "hi"),
            _ => panic!("glyph mapped wrong"),
        }
        // The heat-cell NaN survives (the no-cell sentinel must not be lost in conversion).
        match &converted[4] {
            d::Item::HeatCells(h) => assert!(h.values[2].is_nan()),
            _ => panic!("heat mapped wrong"),
        }
    }

    #[test]
    fn data_record_conversion_is_field_for_field() {
        let sdk = t::DataRecord {
            schema_id: "observatory/timeseries".into(),
            schema_version: 1,
            channels: vec![
                t::DataValue::Int64(42),
                t::DataValue::Double(3.5),
                t::DataValue::Double(1.0),
            ],
            t_wall_ns: 999,
            t_sample: None,
            seq: 7,
            record_flags: t::DataFlags::Stale,
        };
        let core = to_core_record(&sdk);
        assert_eq!(core.schema_id, "observatory/timeseries");
        assert_eq!(core.t_wall_ns, 999);
        assert_eq!(core.seq, 7);
        assert_eq!(core.record_flags, core_rec::DataFlags::Stale);
        assert_eq!(core.channels.len(), 3);
        assert_eq!(core.num(1), Some(3.5));
    }

    #[test]
    fn cell_status_reads_the_host_signals() {
        // No window + key-requiring stream → KEY NEEDED (D14, never a silent hole).
        let s = cell_status(&None, true);
        assert!(matches!(s, CellStatus::KeyNeeded(_)));
        // No window, no key → OFFLINE.
        let s = cell_status(&None, false);
        assert_eq!(s, CellStatus::Offline);
        // A window with a stale flag → STALE.
        let stale = Some(vec![core_rec::DataRecord {
            schema_id: "observatory/timeseries".into(),
            schema_version: 1,
            channels: vec![],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: core_rec::DataFlags::Stale,
        }]);
        let s = cell_status(&stale, false);
        assert_eq!(s, CellStatus::Stale);
        // A fresh window → LIVE.
        let live = Some(vec![core_rec::DataRecord {
            schema_id: "observatory/timeseries".into(),
            schema_version: 1,
            channels: vec![],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: core_rec::DataFlags::Ok,
        }]);
        let s = cell_status(&live, false);
        assert_eq!(s, CellStatus::Live);
    }

    #[test]
    fn lod_maps_by_name_not_by_order() {
        assert_eq!(to_core_lod(Lod::Full), core_ir::Lod::Full);
        assert_eq!(to_core_lod(Lod::Reduced), core_ir::Lod::Reduced);
        assert_eq!(to_core_lod(Lod::Minimal), core_ir::Lod::Minimal);
    }

    fn test_resources() -> Resources {
        Resources {
            sample_rate: 48_000,
            block_frames: 64,
            audio_in_channels: Vec::new(),
            audio_out_channels: Vec::new(),
            oversampling: sparq::Oversampling::None,
            voices: 0,
            arena_bytes: 0,
            fuel_per_block: 2_000_000,
            token_bundle: sparq::Semver { major: 0, minor: 1, patch: 0 },
        }
    }

    fn empty_block_input() -> BlockInput {
        BlockInput {
            block_id: 0,
            frames: 64,
            t_sample: 0,
            tick: 0,
            ppqn: 960,
            params: sparq::ParamSet { version: 0, values: vec![0.0; 26] },
            audio_in: Vec::new(),
            cv_in: Vec::new(),
            events_in: Vec::new(),
            data_in: Vec::new(),
        }
    }
}
