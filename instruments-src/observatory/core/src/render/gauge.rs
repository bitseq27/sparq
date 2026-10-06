//! The GAUGE renderer (§5.5): the current value as an arc sweep with a unit readout.
//!
//! The v1 gauge stream is `wx.air` (US AQI now). The gauge draws a hairline track arc, the value as a
//! `dat`-accent sweep (w2 — w3 is reserved for the selection halo, §5.6 rule 5), the value with its
//! unit at the centre (rule 9), and min/max scale labels. The sweep spans the top semicircle (180°→360°
//! in the display's y-down space, 0° at +x).
//!
//! # Thresholds — recorded, not faked
//!
//! §5.5 wants thresholds from registry metadata ("AQI bands", "Kp ≥ 5 storm line"). The Kp storm line
//! lives in [`super::bars`] (which draws Kp). The AQI *bands* are not yet registry rows, so v1 draws
//! fractional scale ticks (0/50/100 %) instead of inventing band edges; wiring real AQI bands is a
//! registry-row addition recorded in LATER.md, not a guess baked into the renderer.

use crate::ir::{Item, Lod, Style};
use crate::layout::CellRect;
use crate::render::{
    arc, glyph, line, text_width, truncate, RenderCtx, T_HAIRLINE, T_SCALE_S, T_SCALE_XS,
    T_SIGNAL_DATA, T_TEXT_PRIMARY, T_TEXT_TERTIARY,
};
use crate::view::{format_with_unit, CellView, SeriesView};

/// Renders the newest value of a series as a gauge sweep.
#[must_use]
pub fn render(
    series: &SeriesView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = Vec::new();
    if body.w < 16.0 || body.h < 16.0 {
        return out;
    }
    let value = series.latest.unwrap_or(0.0);
    // Scale max: the window's own max, floored so a tiny reading still sweeps visibly. AQI is 0…500
    // but the gauge auto-ranges to the window rather than hard-coding a domain it was not given.
    let hi = series
        .points
        .iter()
        .map(|(_, v)| *v)
        .filter(|v| v.is_finite())
        .fold(f64::NEG_INFINITY, f64::max)
        .max(value)
        .max(1.0);
    let frac = (value / hi).clamp(0.0, 1.0) as f32;

    // The gauge geometry: a semicircle centred low in the body, radius = min(w/2, h) with margin.
    let r = (body.w * 0.42).min(body.h * 0.8).max(8.0);
    let cx = body.cx();
    let cy = body.y + r + 4.0;

    // Track (hairline) + value sweep (dat accent, w2). Angles: 180° (left) → 360° (right) over the top.
    out.push(arc(cx, cy, r, 180.0, 360.0, Style::hairline(T_HAIRLINE)));
    if frac > 0.0 {
        out.push(arc(cx, cy, r, 180.0, 180.0 + frac * 180.0, Style::trace(T_SIGNAL_DATA)));
    }

    // Fractional scale ticks (0 / 50 / 100 %) — small radial hairlines. Real domain bands are a
    // registry-row addition (see the module header), not invented here.
    if ctx.lod == Lod::Full {
        for f in [0.0f32, 0.5, 1.0] {
            let ang = std::f32::consts::PI * (1.0 + f); // 180° → 360° in radians
            let (sx, sy) = (cx + r * ang.cos(), cy + r * ang.sin());
            let (ex, ey) = (cx + (r - 5.0) * ang.cos(), cy + (r - 5.0) * ang.sin());
            out.push(line(sx, sy, ex, ey, Style::hairline(T_HAIRLINE)));
        }
    }

    // The value readout (with units, rule 9) at the centre, and the scale max beneath it.
    let label = format_with_unit(value, view.chrome.unit.as_str());
    let label = truncate(&label, body.w * 0.8, ctx.metrics.title_px);
    let lx = cx - text_width(&label, ctx.metrics.title_px) / 2.0;
    out.push(glyph(label, lx, cy - 2.0, T_SCALE_S, T_TEXT_PRIMARY));
    if ctx.lod == Lod::Full {
        let hi_label = format_with_unit(hi, view.chrome.unit.as_str());
        out.push(glyph(
            truncate(&hi_label, body.w * 0.6, ctx.metrics.small_px),
            cx - text_width(&hi_label, ctx.metrics.small_px) / 2.0,
            cy + ctx.metrics.small_px + 2.0,
            T_SCALE_XS,
            T_TEXT_TERTIARY,
        ));
    }
    out
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
    fn view() -> CellView {
        CellView {
            stream_id: Some("wx.air"),
            chrome: CellChrome {
                title: "Air".into(),
                description: "WX · 900 s".into(),
                status: CellStatus::Live,
                unit: "US AQI".into(),
                stamp_utc: None,
                age_s: None,
                headline: None,
            },
            body: crate::view::Body::NoData,
        }
    }

    #[test]
    fn a_gauge_draws_a_track_and_a_value_sweep() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let series =
            SeriesView { points: vec![(0, 50.0), (1, 38.0)], latest: Some(38.0), bipolar: false };
        let items = render(&series, &view(), &cell, &ctx(&p, &m));
        // Two arcs at minimum: the track (full 180°) and the value sweep (< 180°).
        let arcs: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                Item::Arc(a) => Some(a),
                _ => None,
            })
            .collect();
        assert!(arcs.len() >= 2, "track + sweep, got {}", arcs.len());
        assert!(
            arcs.iter().any(|a| (a.a1_deg - a.a0_deg - 180.0).abs() < 0.5),
            "the track spans 180°"
        );
        assert!(
            arcs.iter().any(|a| a.a1_deg - a.a0_deg < 180.0 && a.a1_deg > a.a0_deg),
            "the sweep is a partial arc"
        );
    }

    #[test]
    fn the_readout_carries_the_unit() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let series = SeriesView { points: vec![(0, 38.0)], latest: Some(38.0), bipolar: false };
        let items = render(&series, &view(), &cell, &ctx(&p, &m));
        assert!(
            items.iter().any(|i| matches!(i, Item::GlyphRun(g) if g.text.contains("US AQI"))),
            "the value readout carries its unit (rule 9)"
        );
    }

    #[test]
    fn a_zero_value_draws_no_sweep_but_still_a_track() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let series = SeriesView { points: vec![(0, 0.0)], latest: Some(0.0), bipolar: false };
        let items = render(&series, &view(), &cell, &ctx(&p, &m));
        assert!(
            items
                .iter()
                .any(|i| matches!(i, Item::Arc(a) if (a.a1_deg - a.a0_deg - 180.0).abs() < 0.5)),
            "the track always draws"
        );
        assert!(
            !items.iter().any(
                |i| matches!(i, Item::Arc(a) if a.a1_deg - a.a0_deg < 179.0 && a.a1_deg > a.a0_deg)
            ),
            "a zero value draws no partial sweep"
        );
    }
}
