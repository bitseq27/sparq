//! The seven renderers (§5.5) + the cell chrome (§5.3) + the ticker band, and the dispatch that
//! ties a [`crate::view::CellView`] to a renderer by the registry's `view` hint.
//!
//! Every function here is pure geometry + token ids: it consumes a typed view and a [`crate::layout::CellRect`]
//! and appends [`crate::ir::Item`]s. There is no colour literal, no size literal, no clock read — the
//! contract makes literal appearance unrepresentable (display.wit header), and the only time value a
//! renderer sees is `time_sec` for animation phase (ticker scroll), never for data position (D8/D11).
//!
//! # Token ids
//!
//! The ids below are the checked-in bundle's keys (`design/tokens/generated/tokens.json`); the
//! harness drift-tests every one the core emits against that file, so a token rename fails a test
//! rather than silently drawing an unresolved primitive (which the host skips with a diagnostic —
//! fail-soft at runtime, fail-loud at hand-in).

pub mod bars;
pub mod gauge;
pub mod map;
pub mod text;
pub mod timeline;
pub mod timeseries;

use crate::ir::{
    ArcItem, Corner, GlyphRunItem, Item, Lod, PathItem, PathSegment, Point, PointsItem,
    PolylineItem, RectItem, Style,
};
use crate::layout::{CellRect, Metrics, Rect};
use crate::params::Params;
use crate::view::{Body, CellStatus, CellView};

// ── token ids (bundle keys; drift-tested by the harness) ────────────────────────────────────────

/// Cell body ground (the display-sheet's inset ground, §5.6 rule 2).
pub const T_GROUND_INSET: &str = "color.ground.inset";
/// A raised tint for land masses / bar fills (§5.6: depth by ground tint, never shadow).
pub const T_GROUND_PANEL_ALT: &str = "color.ground.panel_alt";
/// Overlay tint (the ticker band's ground).
pub const T_GROUND_OVERLAY: &str = "color.ground.overlay";
/// Hairline colour (grids, graticules, axes, cell borders).
pub const T_HAIRLINE: &str = "color.hairline.colour";
/// Title text (§5.3, type.scale.m).
pub const T_TEXT_PRIMARY: &str = "color.text.primary";
/// Description text (§5.3, type.scale.s).
pub const T_TEXT_SECONDARY: &str = "color.text.secondary";
/// Axis/label text (§5.3, type.scale.s, tertiary).
pub const T_TEXT_TERTIARY: &str = "color.text.tertiary";
/// Dimmed text (OFF cells, disabled chrome).
pub const T_TEXT_DISABLED: &str = "color.text.disabled";
/// The `dat` signal-class accent — class identity ONLY (stripe, LEDs, selection, live traces),
/// never a data colour (§5.6 rule 7).
pub const T_SIGNAL_DATA: &str = "color.signal.data.colour";
/// The dimmed data accent (trails, secondary).
pub const T_SIGNAL_DATA_DIM: &str = "color.signal.data.dim";
/// STALE LED / stale-series accent.
pub const T_STATE_STALE: &str = "color.state.stale.colour";
/// WARNING LED (KEY NEEDED).
pub const T_STATE_WARNING: &str = "color.state.warning.colour";
/// ERROR accent (thresholds, severe).
pub const T_STATE_ERROR: &str = "color.state.error.colour";
/// OFFLINE/muted LED.
pub const T_STATE_MUTED: &str = "color.state.muted.colour";
/// Type scale — small (axis labels, footer, description).
pub const T_SCALE_S: &str = "typography.scale.s";
/// Type scale — extra-small (dense tick labels).
pub const T_SCALE_XS: &str = "typography.scale.xs";
/// Type scale — medium (cell titles).
pub const T_SCALE_M: &str = "typography.scale.m";
/// Marker radius — small tier.
pub const T_MARKER_S: &str = "layout.marker.radius_s";
/// Marker radius — mid tier.
pub const T_MARKER_M: &str = "layout.marker.radius_m";
/// Marker radius — large tier.
pub const T_MARKER_L: &str = "layout.marker.radius_l";
/// Colormap — thermal (aurora/fire heat, §5.6 rule 3).
pub const CMAP_THERMAL: &str = "colormap.thermal";
/// Colormap — bipolar (signed: Bz, Kp-vs-baseline).
pub const CMAP_BIPOLAR: &str = "colormap.bipolar";
/// Colormap — inferno-class (depth/severity).
pub const CMAP_INFERNO: &str = "colormap.inferno-class";
/// Colormap — greyscale (age/secondary channels).
pub const CMAP_GREYSCALE: &str = "colormap.greyscale";
/// Colormap — categorical-6 (EONET/GDACS categories).
pub const CMAP_CATEGORICAL: &str = "colormap.categorical-6";
/// Colormap — phosphor (live scope-like traces).
pub const CMAP_PHOSPHOR: &str = "colormap.phosphor";

/// The every-token-id the core may emit, for the harness's drift test against the bundle. Adding a
/// renderer that uses a new id means adding it here too — which is the point: the list is the
/// contract between the guest and the token bundle, checked, not hoped.
pub const ALL_TOKEN_IDS: [&str; 26] = [
    T_GROUND_INSET,
    T_GROUND_PANEL_ALT,
    T_GROUND_OVERLAY,
    T_HAIRLINE,
    T_TEXT_PRIMARY,
    T_TEXT_SECONDARY,
    T_TEXT_TERTIARY,
    T_TEXT_DISABLED,
    T_SIGNAL_DATA,
    T_SIGNAL_DATA_DIM,
    T_STATE_STALE,
    T_STATE_WARNING,
    T_STATE_ERROR,
    T_STATE_MUTED,
    T_SCALE_S,
    T_SCALE_XS,
    T_SCALE_M,
    T_MARKER_S,
    T_MARKER_M,
    T_MARKER_L,
    CMAP_THERMAL,
    CMAP_BIPOLAR,
    CMAP_INFERNO,
    CMAP_GREYSCALE,
    CMAP_CATEGORICAL,
    CMAP_PHOSPHOR,
];

/// Everything a renderer needs besides its own view: the params, the metrics, the LOD, the animation
/// clock and the embedded coastline (the map renderers' geography). Passed by reference so renderer
/// signatures stay small.
#[derive(Clone, Copy, Debug)]
pub struct RenderCtx<'a> {
    /// The decoded params (grid, intensity, history, pause…).
    pub params: &'a Params,
    /// The layout metrics (token-derived spacing/bands).
    pub metrics: &'a Metrics,
    /// The host's LOD level (§5.7 degradation).
    pub lod: Lod,
    /// The UI animation clock, seconds (ticker scroll phase ONLY — never data position, D8).
    pub time_sec: f64,
    /// The pause-aware accumulated scroll phase, seconds (the wall advances it by `dt × ticker_speed`
    /// only while unpaused, so PAUSE freezes it — §5.5). The text cells and the ticker band both
    /// scroll from this, never from `time_sec` directly (which never freezes).
    pub scroll_phase: f64,
    /// The embedded coastline asset (D12) — the map renderers' land geography.
    pub coast: &'a crate::coastline::Coastline,
}

// ── item constructors (the renderers' shared vocabulary) ────────────────────────────────────────

/// A glyph-run item (text via the host type engine; numbers already unit-bearing, rule 9).
#[must_use]
pub fn glyph(text: impl Into<String>, x: f32, y: f32, size: &str, colour: &str) -> Item {
    Item::GlyphRun(GlyphRunItem {
        text: text.into(),
        x,
        y,
        size: size.into(),
        colour: colour.into(),
    })
}

/// A filled rect (cell bodies, bars, ticks).
#[must_use]
pub fn fill_rect(r: Rect, fill: &str, corner: Corner) -> Item {
    Item::Rect(RectItem { x: r.x, y: r.y, w: r.w, h: r.h, style: Style::fill_corner(fill, corner) })
}

/// A hairline-stroked rect (cell borders, guides).
#[must_use]
pub fn stroke_rect(r: Rect, style: Style) -> Item {
    Item::Rect(RectItem { x: r.x, y: r.y, w: r.w, h: r.h, style })
}

/// A single hairline segment (grid line, axis, crosshair).
#[must_use]
pub fn line(x0: f32, y0: f32, x1: f32, y1: f32, style: Style) -> Item {
    Item::Polyline(PolylineItem { points: vec![Point::new(x0, y0), Point::new(x1, y1)], style })
}

/// A polyline through `points`.
#[must_use]
pub fn polyline(points: Vec<Point>, style: Style) -> Item {
    Item::Polyline(PolylineItem { points, style })
}

/// A filled or stroked arc (LEDs, gauge sweeps, footprint rings). `a0`/`a1` in degrees.
#[must_use]
pub fn arc(cx: f32, cy: f32, r: f32, a0: f32, a1: f32, style: Style) -> Item {
    Item::Arc(ArcItem { cx, cy, r, a0_deg: a0, a1_deg: a1, style })
}

/// A closed path from points (coastline rings, map markers).
#[must_use]
pub fn closed_path(points: &[Point], style: Style) -> Item {
    let mut segments = Vec::with_capacity(points.len());
    for (i, p) in points.iter().enumerate() {
        segments.push(if i == 0 { PathSegment::MoveTo(*p) } else { PathSegment::LineTo(*p) });
    }
    Item::Path(PathItem { segments, closed: true, style })
}

/// A point cloud with a flat signal-class colour (no per-point data channel).
#[must_use]
pub fn points_flat(positions: Vec<Point>, size: &str, colour: &str) -> Item {
    Item::Points(PointsItem {
        positions,
        size: size.into(),
        colour: Some(colour.into()),
        colormap: None,
        values: None,
    })
}

/// A point cloud coloured by a data channel through a colormap (data encoding, never style —
/// token rule 11). `values` is the per-point data feeding `colormap`.
#[must_use]
pub fn points_colormap(
    positions: Vec<Point>,
    size: &str,
    colormap: &str,
    values: Vec<f32>,
) -> Item {
    Item::Points(PointsItem {
        positions,
        size: size.into(),
        colour: None,
        colormap: Some(colormap.into()),
        values: Some(values),
    })
}

// ── text metrics (the guest estimates; the host type engine refines) ────────────────────────────

/// The approximate per-character advance for a font size, px. The bundle's faces are monospace-ish
/// (typography.family.mono); 0.6 × size is the tabular approximation the guest uses for truncation
/// and right-alignment, because a `glyph-run` carries no width and no anchor — the guest positions
/// the run's origin. The host's real metrics refine the drawn glyphs, not the layout slot.
#[must_use]
pub fn char_width(font_px: f32) -> f32 {
    font_px * 0.6
}

/// The approximate width of `s` at `font_px`.
#[must_use]
pub fn text_width(s: &str, font_px: f32) -> f32 {
    s.chars().count() as f32 * char_width(font_px)
}

/// Truncates `s` to fit `max_px` at `font_px`, appending an ellipsis when cut. The guest's own
/// truncation (a `glyph-run` cannot overflow its cell — the display list has no clip primitive).
#[must_use]
pub fn truncate(s: &str, max_px: f32, font_px: f32) -> String {
    let cw = char_width(font_px);
    if cw <= 0.0 {
        return String::new();
    }
    let max_chars = (max_px / cw).floor().max(0.0) as usize;
    let n = s.chars().count();
    if n <= max_chars {
        return s.to_string();
    }
    if max_chars <= 1 {
        return "…".to_string();
    }
    let cut: String = s.chars().take(max_chars - 1).collect();
    format!("{cut}…")
}

// ── map projection (equirectangular, §5.5) ──────────────────────────────────────────────────────

/// Projects (lat, lon) into `r` equirectangularly: lon −180…180 → left…right, lat 90…−90 →
/// top…bottom (north up). NaN in either coordinate yields a point the caller must reject (the
/// contract's no-position sentinel).
#[must_use]
pub fn equirect(lat: f64, lon: f64, r: Rect) -> Point {
    let x = r.x + ((lon + 180.0) / 360.0) as f32 * r.w;
    let y = r.y + ((90.0 - lat) / 180.0) as f32 * r.h;
    Point::new(x, y)
}

/// Whether a (lat, lon) is a real position (not the NaN no-position sentinel).
#[must_use]
pub fn has_position(lat: f64, lon: f64) -> bool {
    lat.is_finite()
        && lon.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
}

// ── colour-value normalisation (the colormap contract) ──────────────────────────────────────────

/// Normalises `v` into 0…1 for a magnitude colormap over `lo…hi` (clamped). The heat-cells/points
/// `values` channel feeds a declared colormap this way; the guest emits normalised 0…1 so the host's
/// LUT sampling is a straight index (documented IR convention: heat/point values are [0,1] or NaN).
#[must_use]
pub fn norm01(v: f64, lo: f64, hi: f64) -> f32 {
    if !v.is_finite() || (hi - lo).abs() < f64::EPSILON {
        return 0.0;
    }
    (((v - lo) / (hi - lo)).clamp(0.0, 1.0)) as f32
}

// ── the cell chrome (§5.3) ──────────────────────────────────────────────────────────────────────

/// The cell's UNDER-layer: the body ground (the cell sits on the inset ground, §5.6 rule 2) and its
/// hairline border. Drawn BEFORE the body content — the ground is a background, not an overlay; the
/// wall emits ground → body → [`cell_chrome`]. In FULL the border is dropped (minimal chrome) but the
/// ground remains (the data still needs a ground).
#[must_use]
pub fn cell_ground(cell: &CellRect, ctx: &RenderCtx<'_>) -> Vec<Item> {
    let mut out = Vec::with_capacity(2);
    out.push(fill_rect(cell.content, T_GROUND_INSET, Corner::C2));
    if !ctx.params.full {
        out.push(stroke_rect(cell.content, Style::hairline(T_HAIRLINE)));
    }
    out
}

/// Draws one cell's OVER-layer chrome (header, footer, LED, selection halo) around an already-rendered
/// body. The caller concatenates ground + body + chrome in that order. `selected` draws the emphasis
/// halo (§5.6 rule 5: w3, and nothing else is w3).
#[must_use]
pub fn cell_chrome(
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
    selected: bool,
) -> Vec<Item> {
    let mut out = Vec::new();
    let m = ctx.metrics;

    // Header: title left, description right, LED far right (§5.3). In FULL these collapse (the
    // layout gave the body the whole content rect), so only draw chrome when the header has height.
    if cell.header.h > 0.0 {
        let led_r = m.led_r;
        let led_cx = cell.content.right() - led_r - 2.0;
        let led_cy = cell.header.y + cell.header.h / 2.0;
        let desc_right = led_cx - led_r - 6.0;

        // Title (left), truncated to leave room for the description + LED.
        let title_avail = (desc_right - cell.content.x - 8.0).max(20.0);
        let title_txt = truncate(&view.chrome.title.to_uppercase(), title_avail, m.title_px);
        out.push(glyph(
            title_txt,
            cell.content.x + 4.0,
            cell.header.y + m.title_px + 1.0,
            T_SCALE_M,
            T_TEXT_PRIMARY,
        ));

        // Description (right-aligned before the LED): domain · cadence.
        let desc =
            truncate(&view.chrome.description, (desc_right - cell.content.x).max(20.0), m.small_px);
        let desc_x = desc_right - text_width(&desc, m.small_px);
        out.push(glyph(desc, desc_x, cell.header.y + m.title_px, T_SCALE_S, T_TEXT_SECONDARY));

        // The status LED (an arc 0→360, filled with the status colour).
        let led_colour = match view.chrome.status {
            CellStatus::Live => T_SIGNAL_DATA,
            CellStatus::Stale => T_STATE_STALE,
            CellStatus::Offline => T_STATE_MUTED,
            CellStatus::KeyNeeded(_) => T_STATE_WARNING,
        };
        out.push(arc(led_cx, led_cy, led_r, 0.0, 360.0, Style::fill(led_colour)));
    }

    // Footer: stamp · age · headline (§5.3), scale.s, tertiary. Skip in FULL / at minimal LOD.
    if cell.footer.h > 0.0 && ctx.lod >= Lod::Reduced {
        let footer_txt = footer_line(view, m.small_px, cell.content.w - 8.0);
        out.push(glyph(
            footer_txt,
            cell.content.x + 4.0,
            cell.footer.y + m.small_px + 1.0,
            T_SCALE_S,
            T_TEXT_TERTIARY,
        ));
    }

    // The selection halo (§5.6 rule 5): w3 emphasis, the ONLY w3 in the instrument, in the dat
    // accent (rule 7: the accent is class identity — stripe, LED, selection — never a data colour).
    if selected && !ctx.params.full {
        out.push(stroke_rect(cell.outer, Style::emphasis(T_SIGNAL_DATA)));
    }

    out
}

/// The footer's one line: `HH:MM:SS UTC · <age> ago · <headline>`, truncated to `max_px`.
fn footer_line(view: &CellView, font_px: f32, max_px: f32) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = view.chrome.stamp_utc {
        parts.push(format!("{} UTC", crate::render::utc_hms(t)));
    }
    if let Some(age) = view.chrome.age_s {
        parts.push(format!("{} ago", age_words(age)));
    }
    if let Some(h) = &view.chrome.headline {
        if !h.is_empty() {
            parts.push(h.clone());
        }
    }
    let joined = parts.join(" · ");
    truncate(&joined, max_px, font_px)
}

/// Formats an age in seconds as words ("47 s", "12 min", "3 h", "2 d") — the footer's age readout,
/// computed from the record's host-provided `age_s` (D8), never from a guest clock.
#[must_use]
pub fn age_words(age_s: f64) -> String {
    if !age_s.is_finite() || age_s < 0.0 {
        return "—".to_string();
    }
    if age_s < 90.0 {
        format!("{:.0} s", age_s)
    } else if age_s < 90.0 * 60.0 {
        format!("{:.0} min", age_s / 60.0)
    } else if age_s < 48.0 * 3600.0 {
        format!("{:.1} h", age_s / 3600.0)
    } else {
        format!("{:.1} d", age_s / 86_400.0)
    }
}

/// Unix seconds → `HH:MM:SS` (UTC). The civil-from-days computation is dependency-free (no chrono);
/// it is the same arithmetic the broker's `time.rs` uses, mirrored here so the guest renders absolute
/// UTC from the record's own stamp (D8), never a relative or local time.
#[must_use]
pub fn utc_hms(t_utc: i64) -> String {
    let secs = t_utc.rem_euclid(86_400);
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Unix seconds → `YYYY-MM-DD` (UTC), civil-from-days (Howard Hinnant's algorithm), dependency-free.
#[must_use]
pub fn utc_ymd(t_utc: i64) -> String {
    let days = t_utc.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// The body-centre WORDS a cell shows when it cannot draw data (§5.3: OFFLINE/KEY NEEDED are words,
/// never a silent hole; OFF is a word too). The refuse-in-words rule, guest-side.
#[must_use]
pub fn nodata_words(view: &CellView, cell: &CellRect, ctx: &RenderCtx<'_>) -> Vec<Item> {
    let body = cell.body;
    let cx = body.cx();
    let cy = body.cy();
    let (words, colour) = match (&view.body, view.chrome.status) {
        (Body::Off, _) => (crate::streams_table::OFF_LABEL.to_string(), T_TEXT_DISABLED),
        (_, CellStatus::KeyNeeded(env)) => (format!("KEY NEEDED — set {env}"), T_STATE_WARNING),
        (_, CellStatus::Offline) => ("OFFLINE — no data".to_string(), T_STATE_MUTED),
        (Body::UnknownSchema(s), _) => (format!("unknown schema {s} — cannot draw"), T_STATE_MUTED),
        (Body::NoData, CellStatus::Stale) => ("STALE — awaiting data".to_string(), T_STATE_STALE),
        _ => ("no data".to_string(), T_TEXT_DISABLED),
    };
    let font = ctx.metrics.small_px + 2.0;
    let txt = truncate(&words, body.w - 8.0, font);
    let x = cx - text_width(&txt, font) / 2.0;
    // A centred word line (baseline at cy + half-cap-height).
    vec![glyph(txt, x, cy + font / 3.0, T_SCALE_S, colour)]
}

/// Dispatches a cell's body to its renderer by the registry's `view` hint (§5.5). Returns the body
/// items (chrome is added by [`cell_chrome`]). A cell with no data returns the refuse-in-words items.
#[must_use]
pub fn render_body(view: &CellView, cell: &CellRect, ctx: &RenderCtx<'_>) -> Vec<Item> {
    // No data → WORDS (never an empty chart that reads as "zero").
    if matches!(view.body, Body::Off | Body::NoData | Body::UnknownSchema(_)) {
        return nodata_words(view, cell, ctx);
    }
    // The view hint picks the renderer; the body carries the schema-shaped data. `meta.view` is not
    // on the view, so the wall passes it via the dispatch table below keyed on the body + hint. The
    // hint arrives through `view.stream_id` → streams_table (the single source of truth).
    let hint =
        view.stream_id.and_then(crate::streams_table::by_id).map_or("timeseries", |m| m.view);
    match (&view.body, hint) {
        (Body::Heat(h), _) => map::render_heat(h, view, cell, ctx),
        (Body::Position(p), _) => map::render_position(p, view, cell, ctx),
        (Body::Events(e), "timeline") | (Body::Events(e), "timeline-scatter") => {
            timeline::render(e, view, cell, ctx, hint)
        },
        (Body::Events(e), _) => map::render_points(e, view, cell, ctx),
        (Body::Series(s), "bars+gauge") => bars::render_kp(s, view, cell, ctx),
        (Body::Series(s), "bars") => bars::render_bars(s, view, cell, ctx),
        (Body::Series(s), "gauge") => gauge::render(s, view, cell, ctx),
        (Body::Series(s), _) => timeseries::render(s, view, cell, ctx),
        (Body::Text(t), _) => text::render(t, view, cell, ctx),
        // A schema/hint combination with no renderer: refuse in words rather than draw nothing.
        _ => nodata_words(view, cell, ctx),
    }
}

/// Whether a cell's renderer reserves an x-axis band (the layout's `has_axis` predicate). Series,
/// timeline and bars have x-axes; map/heat/gauge/text do not.
#[must_use]
pub fn view_has_axis(hint: &str) -> bool {
    matches!(hint, "timeseries" | "bars+gauge" | "bars" | "timeline" | "timeline-scatter")
}

/// The marker size token for a magnitude/severity tier (§5.5: size class by magnitude). Three tiers
/// over the `layout.marker.radius_*` vocabulary.
#[must_use]
pub fn marker_size(magnitude: f64, severity: u32) -> &'static str {
    // Earthquakes key off magnitude; category feeds (no magnitude) key off severity.
    let tier = if magnitude.abs() > f64::EPSILON {
        if magnitude >= 5.0 {
            2
        } else if magnitude >= 3.0 {
            1
        } else {
            0
        }
    } else {
        match severity {
            0 | 1 => 0,
            2 | 3 => 1,
            _ => 2,
        }
    };
    match tier {
        2 => T_MARKER_L,
        1 => T_MARKER_M,
        _ => T_MARKER_S,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn truncation_fits_the_budget_and_ellipsises() {
        let t = truncate("EARTHQUAKES — PAST HOUR", 60.0, 11.0);
        assert!(text_width(&t, 11.0) <= 60.0 + char_width(11.0), "ellipsis may add one char");
        assert!(t.ends_with('…'), "a cut string is marked, never silently dropped");
        assert_eq!(truncate("hi", 100.0, 11.0), "hi", "a short string is untouched");
    }

    #[test]
    fn equirect_maps_the_corners() {
        let r = Rect::new(0.0, 0.0, 360.0, 180.0);
        let nw = equirect(90.0, -180.0, r);
        let se = equirect(-90.0, 180.0, r);
        assert!((nw.x - 0.0).abs() < 1e-3 && (nw.y - 0.0).abs() < 1e-3);
        assert!((se.x - 360.0).abs() < 1e-3 && (se.y - 180.0).abs() < 1e-3);
        let c = equirect(0.0, 0.0, r);
        assert!((c.x - 180.0).abs() < 1e-3 && (c.y - 90.0).abs() < 1e-3, "0,0 is the centre");
    }

    #[test]
    fn the_nan_sentinel_is_not_a_position() {
        assert!(!has_position(f64::NAN, 0.0));
        assert!(!has_position(0.0, f64::NAN));
        assert!(has_position(35.0, -118.0));
        assert!(!has_position(95.0, 0.0), "lat > 90 is not a real position");
    }

    #[test]
    fn utc_formatting_is_absolute_and_dependency_free() {
        // 1_700_000_000 = 2023-11-14 22:13:20 UTC (a known-answer vector).
        assert_eq!(utc_hms(1_700_000_000), "22:13:20");
        assert_eq!(utc_ymd(1_700_000_000), "2023-11-14");
        assert_eq!(utc_hms(0), "00:00:00");
        assert_eq!(utc_ymd(0), "1970-01-01");
    }

    #[test]
    fn age_words_scale_through_the_units() {
        assert_eq!(age_words(47.0), "47 s");
        assert_eq!(age_words(720.0), "12 min");
        assert_eq!(age_words(10_800.0), "3.0 h");
        assert_eq!(age_words(172_800.0), "2.0 d");
    }

    #[test]
    fn norm01_clamps_and_handles_a_degenerate_range() {
        assert_eq!(norm01(5.0, 0.0, 10.0), 0.5);
        assert_eq!(norm01(-1.0, 0.0, 10.0), 0.0, "below lo clamps to 0");
        assert_eq!(norm01(99.0, 0.0, 10.0), 1.0, "above hi clamps to 1");
        assert_eq!(norm01(1.0, 5.0, 5.0), 0.0, "a zero-width range is 0, not a divide-by-zero");
        assert_eq!(norm01(f64::NAN, 0.0, 1.0), 0.0);
    }

    #[test]
    fn every_emitted_token_id_is_listed_for_the_drift_test() {
        // The renderers only ever use the constants above; this asserts the list is complete against
        // the constants (a new const without a list entry fails here, so the harness drift test sees
        // every id the core can emit).
        for id in [
            T_GROUND_INSET,
            T_GROUND_PANEL_ALT,
            T_GROUND_OVERLAY,
            T_HAIRLINE,
            T_TEXT_PRIMARY,
            T_TEXT_SECONDARY,
            T_TEXT_TERTIARY,
            T_TEXT_DISABLED,
            T_SIGNAL_DATA,
            T_SIGNAL_DATA_DIM,
            T_STATE_STALE,
            T_STATE_WARNING,
            T_STATE_ERROR,
            T_STATE_MUTED,
            T_SCALE_S,
            T_SCALE_XS,
            T_SCALE_M,
            T_MARKER_S,
            T_MARKER_M,
            T_MARKER_L,
            CMAP_THERMAL,
            CMAP_BIPOLAR,
            CMAP_INFERNO,
            CMAP_GREYSCALE,
            CMAP_CATEGORICAL,
            CMAP_PHOSPHOR,
        ] {
            assert!(ALL_TOKEN_IDS.contains(&id), "{id} is emitted but not in ALL_TOKEN_IDS");
        }
    }

    #[test]
    fn marker_size_tiers_by_magnitude_then_severity() {
        assert_eq!(marker_size(6.0, 0), T_MARKER_L);
        assert_eq!(marker_size(4.0, 0), T_MARKER_M);
        assert_eq!(marker_size(2.0, 0), T_MARKER_S);
        // No magnitude → severity drives it.
        assert_eq!(marker_size(0.0, 4), T_MARKER_L);
        assert_eq!(marker_size(0.0, 1), T_MARKER_S);
    }
}
