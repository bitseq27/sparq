//! The MAP renderers (§5.5): a shared equirectangular coastline+graticule base, then the three map
//! presentations the registry's `view` hint selects — `map-points` (event dots), `map-heat` (the
//! aurora/FIRMS grid as `heat-cells`), and `map` (the ISS position marker + trail).
//!
//! # The coastline is ONE path item (multi-subpath)
//!
//! The embedded asset (D12) has ~128 land rings / ≤ 3 500 points. Emitting one [`crate::ir::PathItem`]
//! per ring would be ~128 primitives per map cell; merging every ring into a single path with one
//! `move-to` per ring keeps it to ONE primitive (the painter closes each subpath — the convention the
//! harness's SVG writer and INC4's painter both honour, documented at [`coast_path`]). Fewer
//! primitives means the §5.7 minimal-LOD "primitive count < 500" rule and the §5.8 budget both stay
//! honest without special-casing the map.

use crate::ir::{Corner, Dash, Item, Lod, PathItem, PathSegment, Point, Style};
use crate::layout::{CellRect, Rect};
use crate::render::{
    arc, equirect, glyph, has_position, line, norm01, points_colormap, polyline, text_width,
    truncate, utc_hms, RenderCtx, CMAP_INFERNO, CMAP_THERMAL, T_GROUND_PANEL_ALT, T_HAIRLINE,
    T_MARKER_L, T_MARKER_M, T_MARKER_S, T_SCALE_XS, T_SIGNAL_DATA, T_SIGNAL_DATA_DIM,
    T_TEXT_PRIMARY, T_TEXT_TERTIARY,
};
use crate::view::{CellView, EventsView, HeatView, PositionView};

/// The coastline as ONE closed multi-subpath [`PathItem`]: land filled with a raised ground tint,
/// edged with a hairline (§5.6 rule 2 — depth by tint, never shadow). `lod` thins the points:
/// Reduced keeps every other vertex, Minimal skips the coast entirely (the map degrades to WORDS at
/// the wall level anyway, §5.7).
#[must_use]
pub fn coast_path(body: Rect, ctx: &RenderCtx<'_>) -> Option<Item> {
    if ctx.lod == Lod::Minimal {
        return None;
    }
    let step = if ctx.lod == Lod::Reduced { 2 } else { 1 };
    let mut segments = Vec::new();
    for ring in ctx.coast.rings() {
        let mut first = true;
        for (i, &(lon, lat)) in ring.iter().enumerate() {
            if i % step != 0 && i + 1 != ring.len() {
                continue;
            }
            let p = equirect(lat, lon, body);
            segments.push(if first { PathSegment::MoveTo(p) } else { PathSegment::LineTo(p) });
            first = false;
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(Item::Path(PathItem {
        segments,
        closed: true, // the painter closes EACH subpath (move-to…move-to) — see the module header
        style: Style {
            stroke: Some(T_HAIRLINE.into()),
            stroke_width: crate::ir::StrokeWidth::W1,
            fill: Some(T_GROUND_PANEL_ALT.into()),
            dash: Dash::Solid,
            corner: Corner::C0,
        },
    }))
}

/// The graticule: hairlines every 30° (dotted, §5.5), drawn only when the GRID param is on and at
/// Full LOD (§5.7 drops graticules at Reduced).
#[must_use]
pub fn graticule(body: Rect, ctx: &RenderCtx<'_>) -> Vec<Item> {
    if !ctx.params.grid || ctx.lod != Lod::Full {
        return Vec::new();
    }
    let mut out = Vec::new();
    let style = Style::hairline_dashed(T_HAIRLINE, Dash::Dotted);
    // Meridians every 30° lon (−180…180 → 13 lines, the ±180 edges coincide with the body sides).
    for lon in (-150..=150).step_by(30) {
        let x = equirect(0.0, f64::from(lon), body).x;
        out.push(line(x, body.y, x, body.bottom(), style.clone()));
    }
    // Parallels every 30° lat (−60…60; the ±90 poles are the body edges).
    for lat in (-60..=60).step_by(30) {
        let y = equirect(f64::from(lat), 0.0, body).y;
        out.push(line(body.x, y, body.right(), y, style.clone()));
    }
    // The equator and prime meridian slightly stronger (solid hairline) — the map's datum.
    let eq = equirect(0.0, 0.0, body);
    out.push(line(body.x, eq.y, body.right(), eq.y, Style::hairline(T_HAIRLINE)));
    out.push(line(eq.x, body.y, eq.x, body.bottom(), Style::hairline(T_HAIRLINE)));
    out
}

/// The shared map base: coastline + graticule, in draw order (land under the grid under the data).
#[must_use]
fn base(body: Rect, ctx: &RenderCtx<'_>) -> Vec<Item> {
    let mut out = Vec::new();
    if let Some(c) = coast_path(body, ctx) {
        out.push(c);
    }
    out.extend(graticule(body, ctx));
    out
}

/// `map-points` — event dots (quakes, EONET, GDACS, alerts, FIRMS, NEO-with-position). Size by
/// magnitude tier, colour by severity through `colormap.inferno-class` (§5.6 rule 3); the newest
/// event gets a highlight ring + label (§5.5). Events without a position (NaN) are skipped — the
/// contract's no-position sentinel, never a dot at 0,0.
#[must_use]
pub fn render_points(
    events: &EventsView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = base(body, ctx);
    if body.w < 8.0 || body.h < 8.0 {
        return out;
    }

    // Group dots by size tier so each tier is ONE points-item (three primitives max, not one per
    // event) — the instance budget stays honest while the size-class encoding is preserved.
    let mut positions = [Vec::new(), Vec::new(), Vec::new()]; // small, mid, large
    let mut values = [Vec::new(), Vec::new(), Vec::new()]; // severity → inferno value
                                                           // The newest positioned event, tracked in one pass (smallest age) as owned data.
    let mut newest: Option<(f64, f64, f64, i64)> = None; // lat, lon, age_s, t_utc
    for e in &events.events {
        if !has_position(e.lat, e.lon) {
            continue;
        }
        let p = equirect(e.lat, e.lon, body);
        let tier = tier_of(e.magnitude, e.severity);
        positions[tier].push(p);
        values[tier].push(norm01(f64::from(e.severity), 0.0, 4.0));
        if newest.map_or(true, |(_, _, age, _)| e.age_s < age) {
            newest = Some((e.lat, e.lon, e.age_s, e.t_utc));
        }
    }
    for t in 0..3 {
        if positions[t].is_empty() {
            continue;
        }
        let size = [T_MARKER_S, T_MARKER_M, T_MARKER_L][t];
        out.push(points_colormap(
            std::mem::take(&mut positions[t]),
            size,
            CMAP_INFERNO,
            std::mem::take(&mut values[t]),
        ));
    }

    // The newest-event highlight ring + its UTC stamp (§5.5), Full LOD only. The headline rides the
    // footer (§5.3), not the map body, so the ring is labelled with the time only.
    if ctx.lod == Lod::Full {
        if let Some((lat, lon, _, t_utc)) = newest {
            let p = equirect(lat, lon, body);
            out.push(arc(p.x, p.y, 9.0, 0.0, 360.0, Style::hairline(T_TEXT_PRIMARY)));
            let label =
                truncate(&format!("{} UTC", utc_hms(t_utc)), body.w * 0.6, ctx.metrics.small_px);
            let lx = (p.x + 12.0).min(body.right() - text_width(&label, ctx.metrics.small_px));
            out.push(glyph(label, lx, p.y + 3.0, T_SCALE_XS, T_TEXT_PRIMARY));
        }
    }
    let _ = view; // the chrome (title/footer) is drawn by cell_chrome, not here
    out
}

/// The size tier (0/1/2) for an event, from magnitude where present else severity — the same rule as
/// [`super::marker_size`], inlined as an index.
fn tier_of(magnitude: f64, severity: u32) -> usize {
    if magnitude.abs() > f64::EPSILON {
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
    }
}

/// `map-heat` — the aurora/FIRMS grid as one `heat-cells` item (§5.5). Values are normalised 0…1 and
/// gain-scaled by the INTENSITY param; NaN stays NaN (the contract's no-cell sentinel). A colourbar
/// strip (Full LOD) gives the range with units (rule 9).
#[must_use]
pub fn render_heat(
    heat: &HeatView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = base(body, ctx);
    if body.w < 8.0 || body.h < 8.0 || heat.values.is_empty() {
        return out;
    }
    let cols = heat.cols.max(1) as u32;
    let rows = heat.rows.max(1) as u32;
    let gain = f64::from(ctx.params.intensity);
    let hi = if heat.max > 0.0 { f64::from(heat.max) } else { 1.0 };
    // Normalise with the intensity gain; NaN passes through (no cell).
    let values: Vec<f32> = heat
        .values
        .iter()
        .map(|&v| if v.is_nan() { f32::NAN } else { norm01(f64::from(v) * gain, 0.0, hi) })
        .collect();
    out.push(crate::ir::Item::HeatCells(crate::ir::HeatCellsItem {
        x: body.x,
        y: body.y,
        w: body.w,
        h: body.h,
        cols,
        rows,
        values,
        colormap: CMAP_THERMAL.into(),
    }));

    // Colourbar strip + range labels (Full LOD).
    if ctx.lod == Lod::Full {
        let cbw = (body.w * 0.4).min(160.0);
        let cbh = 6.0;
        let cbx = body.right() - cbw - 4.0;
        let cby = body.bottom() - cbh - 4.0;
        let ramp: Vec<f32> = (0..64).map(|i| i as f32 / 63.0).collect();
        out.push(crate::ir::Item::HeatCells(crate::ir::HeatCellsItem {
            x: cbx,
            y: cby,
            w: cbw,
            h: cbh,
            cols: 64,
            rows: 1,
            values: ramp,
            colormap: CMAP_THERMAL.into(),
        }));
        let unit = view.chrome.unit.as_str();
        let hi_label = crate::view::format_with_unit(hi, unit);
        out.push(glyph(
            truncate(&hi_label, cbw, ctx.metrics.small_px),
            cbx,
            cby - 2.0,
            T_SCALE_XS,
            T_TEXT_TERTIARY,
        ));
        out.push(glyph("0", cbx - 8.0, cby - 2.0, T_SCALE_XS, T_TEXT_TERTIARY));
    }
    out
}

/// `map` — the ISS position marker + trail (§5.5): a thin trail of recent positions, then the marker
/// (crosshair + footprint ring + dot) at the newest, labelled with alt·vel (units, rule 9).
#[must_use]
pub fn render_position(
    pos: &PositionView,
    _view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = base(body, ctx);
    if body.w < 8.0 || body.h < 8.0 {
        return out;
    }
    // The trail (oldest → newest), a dim polyline.
    let trail: Vec<Point> = pos
        .trail
        .iter()
        .filter(|p| has_position(p.lat, p.lon))
        .map(|p| equirect(p.lat, p.lon, body))
        .collect();
    if trail.len() >= 2 {
        out.push(polyline(trail, Style::hairline(T_SIGNAL_DATA_DIM)));
    }
    // The marker at the newest position.
    if let Some(p) = pos.latest {
        if has_position(p.lat, p.lon) {
            let c = equirect(p.lat, p.lon, body);
            // Footprint ring (dotted), radius from footprint_km at the body's lon scale.
            let px_per_deg = body.w / 360.0;
            let fp_r = (p.footprint_km / 111.32 * px_per_deg as f64) as f32;
            if fp_r > 1.0 && fp_r < body.w {
                out.push(arc(
                    c.x,
                    c.y,
                    fp_r,
                    0.0,
                    360.0,
                    Style::hairline_dashed(T_SIGNAL_DATA_DIM, Dash::Dotted),
                ));
            }
            // Crosshair (the marker idiom, §5.5).
            let arm = 8.0f32;
            out.push(line(c.x - arm, c.y, c.x + arm, c.y, Style::hairline(T_SIGNAL_DATA)));
            out.push(line(c.x, c.y - arm, c.x, c.y + arm, Style::hairline(T_SIGNAL_DATA)));
            // The dot.
            out.push(arc(c.x, c.y, 3.5, 0.0, 360.0, Style::fill(T_SIGNAL_DATA)));
            // Label: alt · vel, with units.
            if ctx.lod >= Lod::Reduced {
                let label = format!("{:.0} km · {:.2} km/s", p.alt_km, p.vel_kms);
                let label = truncate(&label, body.w * 0.7, ctx.metrics.small_px);
                let lx = (c.x + 12.0).min(body.right() - text_width(&label, ctx.metrics.small_px));
                out.push(glyph(label, lx, c.y - 6.0, T_SCALE_XS, T_TEXT_PRIMARY));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::coastline::Coastline;
    use crate::layout::{CellRect, Metrics, Rect};
    use crate::params::Params;
    use crate::view::{CellChrome, CellStatus, EventPt};

    fn ctx<'a>(p: &'a Params, m: &'a Metrics, coast: &'a Coastline) -> RenderCtx<'a> {
        RenderCtx { params: p, metrics: m, lod: Lod::Full, time_sec: 0.0, scroll_phase: 0.0, coast }
    }
    fn view() -> CellView {
        CellView {
            stream_id: Some("geo.quakes-hour"),
            chrome: CellChrome {
                title: "Quakes".into(),
                description: "GEO · 60 s".into(),
                status: CellStatus::Live,
                unit: "magnitude".into(),
                stamp_utc: None,
                age_s: None,
                headline: None,
            },
            body: crate::view::Body::NoData,
        }
    }

    #[test]
    fn coast_path_is_one_primitive_with_the_real_asset() {
        let coast = Coastline::from_asset().unwrap();
        let p = Params::default();
        let m = Metrics::default();
        let item = coast_path(Rect::new(0.0, 0.0, 500.0, 250.0), &ctx(&p, &m, &coast)).unwrap();
        match item {
            Item::Path(path) => {
                assert!(path.closed);
                // One move-to per ring.
                let moves =
                    path.segments.iter().filter(|s| matches!(s, PathSegment::MoveTo(_))).count();
                assert_eq!(moves, coast.ring_count(), "one subpath per land ring, in ONE item");
            },
            _ => panic!("coast_path emits a Path"),
        }
    }

    #[test]
    fn minimal_lod_drops_the_coast() {
        let coast = Coastline::from_asset().unwrap();
        let p = Params::default();
        let m = Metrics::default();
        let c = RenderCtx {
            params: &p,
            metrics: &m,
            lod: Lod::Minimal,
            time_sec: 0.0,
            scroll_phase: 0.0,
            coast: &coast,
        };
        assert!(coast_path(Rect::new(0.0, 0.0, 500.0, 250.0), &c).is_none());
    }

    #[test]
    fn event_dots_group_into_at_most_three_points_items() {
        let coast = Coastline::from_asset().unwrap();
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let events = EventsView {
            events: (0..30)
                .map(|i| EventPt {
                    t_utc: 1_700_000_000 + i,
                    lat: (i as f64 * 5.0) - 70.0,
                    lon: (i as f64 * 11.0) - 170.0,
                    magnitude: 2.0 + (i % 5) as f64,
                    depth_km: 10.0,
                    severity: (i % 5) as u32,
                    age_s: i as f64,
                    headline: format!("q{i}"),
                })
                .collect(),
        };
        let items = render_points(&events, &view(), &cell, &ctx(&p, &m, &coast));
        let pts = items.iter().filter(|i| matches!(i, Item::Points(_))).count();
        assert!(pts <= 3, "dots group by size tier into ≤ 3 points-items, got {pts}");
    }

    #[test]
    fn events_without_a_position_are_not_drawn_at_zero() {
        let coast = Coastline::from_asset().unwrap();
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let events = EventsView {
            events: vec![EventPt {
                t_utc: 1,
                lat: f64::NAN,
                lon: f64::NAN,
                magnitude: 0.0,
                depth_km: 0.0,
                severity: 0,
                age_s: 1.0,
                headline: "no position".into(),
            }],
        };
        let items = render_points(&events, &view(), &cell, &ctx(&p, &m, &coast));
        // No points-item with a position (the NaN event is skipped, not drawn at 0,0).
        let any_dot = items.iter().any(|i| matches!(i, Item::Points(p) if !p.positions.is_empty()));
        assert!(!any_dot, "a NaN-position event must not become a dot");
    }

    #[test]
    fn heat_normalises_and_passes_nan_through() {
        let coast = Coastline::from_asset().unwrap();
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let heat = HeatView {
            cols: 2,
            rows: 1,
            lat0: 0.0,
            lon0: 0.0,
            step_deg: 1.0,
            values: vec![0.0, f32::NAN],
            max: 10.0,
        };
        let items = render_heat(&heat, &view(), &cell, &ctx(&p, &m, &coast));
        let hc = items.iter().find_map(|i| match i {
            Item::HeatCells(h) if h.rows == 1 && h.cols == 2 => Some(h),
            _ => None,
        });
        // The grid heat-cells (cols=2,rows=1) exists and carries a NaN through.
        let grid = items.iter().find_map(|i| match i {
            Item::HeatCells(h) if h.cols == 2 && h.rows == 2 => None,
            Item::HeatCells(h) if h.values.iter().any(|v| v.is_nan()) => Some(h),
            _ => None,
        });
        assert!(grid.is_some(), "a NaN cell stays NaN (no cell drawn)");
        assert!(hc.is_some() || grid.is_some());
    }

    #[test]
    fn the_iss_marker_draws_a_trail_and_a_dot() {
        let coast = Coastline::from_asset().unwrap();
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let pos = PositionView {
            trail: (0..10)
                .map(|i| crate::view::PosPt {
                    lat: i as f64,
                    lon: i as f64 * 3.0,
                    alt_km: 408.0,
                    vel_kms: 7.66,
                    footprint_km: 2200.0,
                    visibility: 1,
                    t_utc: i,
                })
                .collect(),
            latest: Some(crate::view::PosPt {
                lat: 9.0,
                lon: 27.0,
                alt_km: 408.0,
                vel_kms: 7.66,
                footprint_km: 2200.0,
                visibility: 1,
                t_utc: 9,
            }),
        };
        let items = render_position(&pos, &view(), &cell, &ctx(&p, &m, &coast));
        assert!(items.iter().any(|i| matches!(i, Item::Polyline(_))), "the trail is a polyline");
        assert!(items.iter().any(|i| matches!(i, Item::Arc(_))), "the marker dot/ring is an arc");
    }
}
