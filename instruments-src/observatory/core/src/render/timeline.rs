//! The TIMELINE / TIMELINE-SCATTER renderer (§5.5): events against time.
//!
//! Two presentations of the events schema, chosen by the registry's `view` hint:
//! * `timeline-scatter` (GOES X-ray flares) — y is the CLASS row (B/C/M/X, a categorical axis), the
//!   class letters drawn as `glyph-run` annotations (a non-colour encoding, token rule 7);
//! * `timeline` (NASA NEO close approaches) — y is the magnitude/miss-distance, linear, auto-ranged.
//!
//! Both scatter points coloured by severity through `colormap.inferno-class` (§5.6 rule 3) and share
//! the time x-axis + axis band with the timeseries renderer.

use crate::ir::{Dash, Item, Lod, Point, Style};
use crate::layout::CellRect;
use crate::render::{
    glyph, line, norm01, points_colormap, text_width, truncate, utc_hms, RenderCtx, CMAP_INFERNO,
    T_HAIRLINE, T_SCALE_XS, T_TEXT_TERTIARY,
};
use crate::view::{CellView, EventsView};

/// The flare class rows (categorical y for `timeline-scatter`), lowest → highest. The GOES class a
/// severity tier maps to; the letters are the non-colour encoding (rule 7).
const FLARE_ROWS: [&str; 5] = ["—", "B", "C", "M", "X"];

/// Renders an events window as a timeline. `hint` is `timeline-scatter` (categorical class rows) or
/// `timeline` (linear magnitude).
#[must_use]
pub fn render(
    events: &EventsView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
    hint: &str,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = Vec::new();
    if body.w < 8.0 || body.h < 8.0 || events.events.is_empty() {
        return out;
    }
    let scatter = hint == "timeline-scatter";

    // x-window: [latest_t − history_s, latest_t] (clock-free, D8).
    let latest_t = events.events.iter().map(|e| e.t_utc).max().unwrap_or(0);
    let earliest = events.events.iter().map(|e| e.t_utc).min().unwrap_or(0);
    let hist = f64::from(ctx.params.history_s) as i64;
    let t1 = latest_t.max(earliest);
    let t0 = (t1 - hist).min(earliest); // never clip the oldest event off the left
    let tspan = (t1 - t0).max(1) as f32;
    let px = |t: i64| body.x + ((t - t0) as f32 / tspan) * body.w;

    // y-mapping.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    if !scatter {
        for e in &events.events {
            if e.magnitude.is_finite() {
                lo = lo.min(e.magnitude);
                hi = hi.max(e.magnitude);
            }
        }
        if !lo.is_finite() {
            lo = 0.0;
            hi = 1.0;
        }
        if (hi - lo).abs() < f64::EPSILON {
            hi += 1.0;
        }
    }
    let py = |e: &crate::view::EventPt| -> f32 {
        if scatter {
            // Categorical rows: severity 0…4 → row centres, top row = highest class.
            let row = e.severity.min(4) as f32;
            body.y + (1.0 - row / 4.0) * body.h
        } else {
            body.y + (1.0 - norm01(e.magnitude, lo, hi)) * body.h
        }
    };

    // Grid + axis labels at Full LOD.
    if ctx.lod == Lod::Full {
        if scatter {
            // Class-row guides + letters (the non-colour encoding).
            for (i, label) in FLARE_ROWS.iter().enumerate() {
                let y = body.y + (1.0 - i as f32 / 4.0) * body.h;
                out.push(line(
                    body.x,
                    y,
                    body.right(),
                    y,
                    Style::hairline_dashed(T_HAIRLINE, Dash::Dotted),
                ));
                out.push(glyph(*label, body.x + 2.0, y - 2.0, T_SCALE_XS, T_TEXT_TERTIARY));
            }
        } else {
            for i in 0..=2 {
                let frac = i as f32 / 2.0;
                let y = body.y + frac * body.h;
                out.push(line(
                    body.x,
                    y,
                    body.right(),
                    y,
                    Style::hairline_dashed(T_HAIRLINE, Dash::Dotted),
                ));
                let v = hi - f64::from(frac) * (hi - lo);
                let label = crate::view::format_with_unit(v, view.chrome.unit.as_str());
                out.push(glyph(
                    truncate(&label, body.w * 0.4, ctx.metrics.small_px),
                    body.x + 2.0,
                    y - 2.0,
                    T_SCALE_XS,
                    T_TEXT_TERTIARY,
                ));
            }
        }
        // x-axis time labels.
        if cell.axis.h > 0.0 {
            let ay = cell.axis.y + ctx.metrics.small_px;
            for (frac, t) in [(0.0f32, t0), (1.0, t1)] {
                let label = format!("{} UTC", utc_hms(t));
                let x = if frac < 0.5 {
                    body.x
                } else {
                    body.right() - text_width(&label, ctx.metrics.small_px)
                };
                out.push(glyph(label, x, ay, T_SCALE_XS, T_TEXT_TERTIARY));
            }
        }
    }

    // The scatter points, coloured by severity (inferno-class), sized mid-tier (events are discrete;
    // magnitude already drives the y-position for the linear case, so a single marker size reads
    // cleanly and keeps the instance count to one points-item).
    let positions: Vec<Point> =
        events.events.iter().map(|e| Point::new(px(e.t_utc), py(e))).collect();
    let values: Vec<f32> =
        events.events.iter().map(|e| norm01(f64::from(e.severity), 0.0, 4.0)).collect();
    if !positions.is_empty() {
        out.push(points_colormap(positions, super::T_MARKER_M, CMAP_INFERNO, values));
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::layout::{CellRect, Metrics, Rect};
    use crate::params::Params;
    use crate::view::{CellChrome, CellStatus, EventPt};

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
    fn view(unit: &str) -> CellView {
        CellView {
            stream_id: Some("swpc.xray-flares"),
            chrome: CellChrome {
                title: "Flares".into(),
                description: "SW · 300 s".into(),
                status: CellStatus::Live,
                unit: unit.into(),
                stamp_utc: None,
                age_s: None,
                headline: None,
            },
            body: crate::view::Body::NoData,
        }
    }
    fn ev(t: i64, mag: f64, sev: u32) -> EventPt {
        EventPt {
            t_utc: t,
            lat: f64::NAN,
            lon: f64::NAN,
            magnitude: mag,
            depth_km: 0.0,
            severity: sev,
            age_s: 1.0,
            headline: "x".into(),
        }
    }

    #[test]
    fn scatter_draws_class_rows_and_points() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, true, false);
        let events = EventsView { events: vec![ev(100, 4.4, 1), ev(200, 1.2, 3), ev(300, 0.0, 4)] };
        let items = render(&events, &view("GOES class"), &cell, &ctx(&p, &m), "timeline-scatter");
        assert!(items.iter().any(|i| matches!(i, Item::Points(_))), "scatter draws a points-item");
        // The class letters are glyph annotations (non-colour encoding, rule 7).
        let letters = items
            .iter()
            .filter(|i| matches!(i, Item::GlyphRun(g) if FLARE_ROWS.contains(&g.text.as_str())))
            .count();
        assert!(letters >= 4, "the B/C/M/X row letters are drawn, got {letters}");
    }

    #[test]
    fn a_linear_timeline_ranges_on_magnitude() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, true, false);
        let events = EventsView { events: vec![ev(100, 1e6, 1), ev(200, 5e6, 2)] };
        let items = render(&events, &view("miss distance km"), &cell, &ctx(&p, &m), "timeline");
        assert!(items.iter().any(|i| matches!(i, Item::Points(_))));
        // Points stay inside the body.
        for it in &items {
            if let Item::Points(pt) = it {
                for p in &pt.positions {
                    assert!(cell.body.inset(-2.0).contains(p.x, p.y), "point escapes body: {p:?}");
                }
            }
        }
    }

    #[test]
    fn empty_events_draw_nothing() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, true, false);
        let events = EventsView { events: vec![] };
        assert!(render(&events, &view(""), &cell, &ctx(&p, &m), "timeline").is_empty());
    }
}
