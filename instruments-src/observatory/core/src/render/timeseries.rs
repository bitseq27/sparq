//! The TIMESERIES renderer (§5.5): a scalar-over-time window as a positioned polyline with unit
//! axes, a hairline grid, and — for signed feeds like Bz — a bipolar zero-crossing.
//!
//! # Why polyline, not `trace-item`
//!
//! The frozen `trace-item` carries `values` but NO box (display.wit): "values map across the item's
//! box" — and for the one full-display trace primitive that box is the whole display (the `dsp/scope`
//! inset case). A per-cell series in a 4×4 wall has no full-display box to map across, so the
//! Observatory rides the vocabulary's other approved option — §5.5 names "trace (phosphor…) OR
//! polyline" — and positions each sample itself. This is a deliberate, recorded reading of the frozen
//! surface (the plan's whole point is to falsify it while amendment is cheap): `trace-item` is the
//! full-display scope's primitive; a positioned multi-cell series is a polyline. The phosphor
//! decay/glow for a live ≤ 60 s feed is the host's motion token on the polyline's signal colour, not
//! a guest simulation (§5.6 rule 4).

use crate::ir::{Dash, Item, Lod, Point, Style};
use crate::layout::{CellRect, Rect};
use crate::render::{
    glyph, line, norm01, polyline, text_width, truncate, utc_hms, RenderCtx, T_HAIRLINE,
    T_SCALE_XS, T_SIGNAL_DATA, T_TEXT_TERTIARY,
};
use crate::view::{format_with_unit, CellView, SeriesView};

/// The most polyline vertices a series emits (≤ 1 per px column, the scope precedent, §12 risk 10):
/// a 2048-record window into a ~500 px body decimates to ~500 points, keeping the vertex budget honest.
fn decimate(points: &[(i64, f64)], max: usize) -> Vec<(i64, f64)> {
    if points.len() <= max || max < 2 {
        return points.to_vec();
    }
    // Ceiling stride, so the result never exceeds `max` (a floor stride overshoots the budget).
    let stride = points.len().div_ceil(max);
    let mut out: Vec<(i64, f64)> = points.iter().step_by(stride.max(1)).copied().collect();
    // Always keep the newest sample (the right edge of the trace).
    if let (Some(last), Some(actual_last)) = (out.last(), points.last()) {
        if last.0 != actual_last.0 {
            out.push(*actual_last);
        }
    }
    out
}

/// Renders a timeseries into `cell.body` (+ axis labels in `cell.axis`).
#[must_use]
pub fn render(
    series: &SeriesView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    if body.w < 4.0 || body.h < 4.0 || series.points.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let unit = view.chrome.unit.as_str();

    // The x-window: [latest_t − history_s, latest_t], clock-free (D8) — the newest sample is "now".
    let latest_t = series.points.last().map_or(0, |(t, _)| *t);
    let hist = f64::from(ctx.params.history_s);
    let t0 = latest_t - hist as i64;
    let t1 = latest_t.max(t0 + 1);

    // The y-range over the windowed samples, padded; bipolar series are symmetric about zero.
    let in_window: Vec<(i64, f64)> =
        series.points.iter().copied().filter(|(t, _)| *t >= t0 && *t <= t1).collect();
    let pts = if in_window.is_empty() { series.points.clone() } else { in_window };
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(_, v) in &pts {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    if !lo.is_finite() || !hi.is_finite() {
        return out;
    }
    if series.bipolar {
        let m = lo.abs().max(hi.abs());
        lo = -m;
        hi = m;
    } else {
        lo = lo.min(0.0); // a magnitude series sits on a zero baseline
    }
    if (hi - lo).abs() < f64::EPSILON {
        hi += 1.0;
        lo -= if series.bipolar { 1.0 } else { 0.0 };
    }
    let pad = (hi - lo) * 0.08;
    hi += pad;
    lo -= pad;

    let px = |t: i64| body.x + ((t - t0) as f32 / (t1 - t0) as f32) * body.w;
    let py = |v: f64| body.y + (1.0 - norm01(v, lo, hi)) * body.h;

    // Grid + axis labels at Full LOD (§5.7: Reduced drops them, Minimal keeps only the trace).
    if ctx.lod == Lod::Full {
        // Horizontal gridlines at quarters, with y-value labels (units, rule 9).
        for i in 0..=4 {
            let frac = i as f32 / 4.0;
            let y = body.y + frac * body.h;
            out.push(line(
                body.x,
                y,
                body.right(),
                y,
                Style::hairline_dashed(T_HAIRLINE, Dash::Dotted),
            ));
            let v = hi - f64::from(frac) * (hi - lo);
            let label = format_with_unit(v, unit);
            out.push(glyph(
                truncate(&label, body.w * 0.5, ctx.metrics.small_px),
                body.x + 3.0,
                y - 2.0,
                T_SCALE_XS,
                T_TEXT_TERTIARY,
            ));
        }
        // The bipolar zero line, emphasised (a signed series crosses it at cell mid, §5.5).
        if series.bipolar {
            let y0 = py(0.0);
            out.push(line(body.x, y0, body.right(), y0, Style::hairline(T_HAIRLINE)));
        }
        // x-axis time labels in the axis band (left/centre/right).
        if cell.axis.h > 0.0 {
            let ay = cell.axis.y + ctx.metrics.small_px;
            for (frac, t) in [(0.0f32, t0), (0.5, (t0 + t1) / 2), (1.0, t1)] {
                let label = utc_hms(t);
                let x = body.x + frac * body.w;
                let x = if frac < 0.5 {
                    x
                } else if frac > 0.5 {
                    x - text_width(&label, ctx.metrics.small_px)
                } else {
                    x - text_width(&label, ctx.metrics.small_px) / 2.0
                };
                out.push(glyph(format!("{label} UTC"), x, ay, T_SCALE_XS, T_TEXT_TERTIARY));
            }
        }
    }

    // The trace itself: a positioned polyline in the dat accent (class identity, §5.6 rule 7).
    let drawn = decimate(&pts, body.w.max(2.0) as usize);
    let verts: Vec<Point> = drawn.iter().map(|(t, v)| Point::new(px(*t), py(*v))).collect();
    if verts.len() >= 2 {
        out.push(polyline(verts, Style::trace(T_SIGNAL_DATA)));
    } else if let Some(&(t, v)) = drawn.first() {
        // A single sample: a small dot so the cell is not blank.
        out.push(crate::render::arc(px(t), py(v), 2.0, 0.0, 360.0, Style::fill(T_SIGNAL_DATA)));
    }

    // The latest value, top-right (Full/Reduced; Minimal keeps only the trace).
    if ctx.lod >= Lod::Reduced {
        if let Some(v) = series.latest {
            let label = format_with_unit(v, unit);
            let x = body.right() - text_width(&label, ctx.metrics.small_px) - 3.0;
            out.push(glyph(
                label,
                x,
                body.y + ctx.metrics.small_px + 1.0,
                T_SCALE_XS,
                T_TEXT_TERTIARY,
            ));
        }
    }

    // A stale series is dashed (the flag and the word are the same statement, §6.1) — but only the
    // trace style changes; the geometry is identical, so a golden with the same data is stable.
    if ctx.lod == Lod::Full && view.chrome.status == crate::view::CellStatus::Stale {
        if let Some(Item::Polyline(pl)) = out.last_mut() {
            pl.style.dash = Dash::Dashed;
        }
    }
    out
}

/// Renders a series as a filled area under the trace (used by the map-heat legend strip's ramp and
/// any future filled-series need; kept separate so the plain trace stays a plain trace).
#[must_use]
pub fn render_filled(
    series: &SeriesView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
    r: Rect,
) -> Vec<Item> {
    let _ = (series, view, cell, ctx, r);
    // v1 does not fill under the curve (the display-sheet idiom is an open phosphor trace); this
    // stub keeps the seam for a later increment without pretending to draw something it does not.
    Vec::new()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::layout::{CellRect, Metrics, Rect};
    use crate::params::Params;
    use crate::view::{CellChrome, CellStatus};

    fn ctx<'a>(p: &'a Params, m: &'a Metrics) -> RenderCtx<'a> {
        RenderCtx {
            params: p,
            metrics: m,
            lod: Lod::Full,
            time_sec: 0.0,
            scroll_phase: 0.0,
            coast: &crate::coastline::EMPTY,
        }
    }

    fn view(unit: &str, status: CellStatus) -> CellView {
        CellView {
            stream_id: Some("swpc.solar-wind"),
            chrome: CellChrome {
                title: "Solar wind".into(),
                description: "SW · 60 s".into(),
                status,
                unit: unit.into(),
                stamp_utc: Some(1_700_000_000),
                age_s: Some(10.0),
                headline: None,
            },
            body: crate::view::Body::NoData,
        }
    }

    #[test]
    fn decimation_caps_vertices_and_keeps_the_newest() {
        let pts: Vec<(i64, f64)> = (0..2048).map(|i| (i as i64, i as f64)).collect();
        let d = decimate(&pts, 500);
        assert!(d.len() <= 501, "decimated to the column budget (+1 for the forced newest)");
        assert_eq!(d.last().unwrap().0, 2047, "the newest sample is always kept");
    }

    #[test]
    fn a_series_renders_a_polyline_in_the_body() {
        let p = Params::default();
        let m = Metrics::default();
        let outer = Rect::new(0.0, 0.0, 532.0, 268.0);
        let cell = CellRect::split(0, outer, &m, true, false);
        let series = SeriesView {
            points: (0..50).map(|i| (1_700_000_000 + i * 60, 300.0 + (i as f64) * 2.0)).collect(),
            latest: Some(398.0),
            bipolar: false,
        };
        let items = render(&series, &view("km/s", CellStatus::Live), &cell, &ctx(&p, &m));
        assert!(items.iter().any(|i| matches!(i, Item::Polyline(_))), "a series draws a polyline");
        // Every polyline vertex is inside the body rect (no primitive may escape its cell).
        for it in &items {
            if let Item::Polyline(pl) = it {
                for pt in &pl.points {
                    assert!(
                        cell.body.inset(-2.0).contains(pt.x, pt.y),
                        "vertex escapes the body: {pt:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_bipolar_series_is_symmetric_about_zero() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, true, false);
        let series = SeriesView {
            points: vec![(100, -4.0), (160, 2.0), (220, -1.0)],
            latest: Some(-1.0),
            bipolar: true,
        };
        let items = render(&series, &view("nT", CellStatus::Live), &cell, &ctx(&p, &m));
        // The zero line is drawn (a solid hairline at the vertical centre).
        let zero_y = cell.body.cy();
        let has_zero = items.iter().any(|i| match i {
            Item::Polyline(pl) if pl.points.len() == 2 && pl.style.dash == Dash::Solid => {
                (pl.points[0].y - zero_y).abs() < 1.0 && (pl.points[1].y - zero_y).abs() < 1.0
            },
            _ => false,
        });
        assert!(has_zero, "a bipolar series draws its zero line at cell mid");
    }

    #[test]
    fn reduced_lod_drops_the_grid() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, true, false);
        let series = SeriesView {
            points: (0..10).map(|i| (i as i64 * 60, i as f64)).collect(),
            latest: Some(9.0),
            bipolar: false,
        };
        let full = render(
            &series,
            &view("", CellStatus::Live),
            &cell,
            &RenderCtx {
                params: &p,
                metrics: &m,
                lod: Lod::Full,
                time_sec: 0.0,
                scroll_phase: 0.0,
                coast: &crate::coastline::EMPTY,
            },
        );
        let reduced = render(
            &series,
            &view("", CellStatus::Live),
            &cell,
            &RenderCtx {
                params: &p,
                metrics: &m,
                lod: Lod::Reduced,
                time_sec: 0.0,
                scroll_phase: 0.0,
                coast: &crate::coastline::EMPTY,
            },
        );
        assert!(reduced.len() < full.len(), "Reduced drops gridlines + tick labels (§5.7)");
        assert!(reduced.iter().any(|i| matches!(i, Item::Polyline(_))), "…but keeps the trace");
    }

    #[test]
    fn an_empty_series_draws_nothing() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, true, false);
        let series = SeriesView { points: vec![], latest: None, bipolar: false };
        assert!(render(&series, &view("", CellStatus::Live), &cell, &ctx(&p, &m)).is_empty());
    }
}
