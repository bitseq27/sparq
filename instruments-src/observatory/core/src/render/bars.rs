//! The BARS renderer (§5.5): a scalar window as vertical bars.
//!
//! The one v1 stream that uses it is `swpc.kp` (view `bars+gauge`): the 3-day planetary K-index as a
//! bar per 3-hour sample, with the Kp ≥ 5 storm threshold drawn as a labelled line, and the current
//! Kp as a corner readout (the "gauge" half, a value-with-unit rather than a redundant second arc —
//! the gauge renderer owns sweeps).
//!
//! # A recorded friction with the frozen vocabulary
//!
//! §5.5 sketches "Kp bars coloured by `colormap.bipolar` per level". The frozen `rect-item` carries a
//! single `fill` token and NO colormap (colormaps ride only points/heat-cells/mesh/heightfield, where
//! a `values` channel feeds them — display.wit). So a per-bar colormap is not representable on a rect
//! without inventing a primitive. v1 draws the bars in the `dat` accent and encodes the storm
//! threshold as a line + the level as the bar height (the height IS the data); the bipolar-colour idea
//! is recorded here and in the state card as ADR-010 review-trigger material (an additive `rect`
//! colormap channel, or bars-as-heat-cells, is a later minor), not papered over with a fake primitive.

use crate::ir::{Corner, Dash, Item, Lod, Style};
use crate::layout::CellRect;
use crate::render::{
    fill_rect, glyph, line, text_width, truncate, RenderCtx, T_HAIRLINE, T_SCALE_XS, T_SIGNAL_DATA,
    T_STATE_ERROR, T_TEXT_TERTIARY,
};
use crate::view::{format_with_unit, CellView, SeriesView};

/// The Kp storm threshold (a geomagnetic storm is Kp ≥ 5 — registry metadata, a data-driven line, not
/// taste, §5.5/§5.6 rule 6).
pub const KP_STORM: f64 = 5.0;
/// The Kp scale maximum (the index runs 0…9).
pub const KP_MAX: f64 = 9.0;

/// Renders the Kp window as bars + the storm threshold + a current-value readout (`bars+gauge`).
#[must_use]
pub fn render_kp(
    series: &SeriesView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = Vec::new();
    if body.w < 8.0 || body.h < 8.0 || series.points.is_empty() {
        return out;
    }
    // Window to the history span (clock-free).
    let latest_t = series.points.last().map_or(0, |(t, _)| *t);
    let t0 = latest_t - f64::from(ctx.params.history_s) as i64;
    let pts: Vec<&(i64, f64)> = series.points.iter().filter(|(t, _)| *t >= t0).collect();
    let pts = if pts.is_empty() { series.points.iter().collect() } else { pts };
    let n = pts.len();
    let slot = body.w / n as f32;
    let bar_w = (slot * 0.7).max(1.0);

    let y_of = |v: f64| body.bottom() - (v.clamp(0.0, KP_MAX) / KP_MAX) as f32 * body.h;

    // The storm threshold line (Kp ≥ 5), labelled.
    let ys = y_of(KP_STORM);
    out.push(line(
        body.x,
        ys,
        body.right(),
        ys,
        Style::hairline_dashed(T_STATE_ERROR, Dash::Dashed),
    ));
    if ctx.lod == Lod::Full {
        let lbl = "Kp 5 storm";
        out.push(glyph(
            lbl,
            body.right() - text_width(lbl, ctx.metrics.small_px) - 2.0,
            ys - 2.0,
            T_SCALE_XS,
            T_TEXT_TERTIARY,
        ));
    }

    // The bars (data accent; height IS the Kp value).
    for (i, &&(_, v)) in pts.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        let x = body.x + i as f32 * slot + (slot - bar_w) / 2.0;
        let y = y_of(v);
        let h = (body.bottom() - y).max(0.5);
        // A bar at/above the storm line reads in the error accent (the non-colour encoding is the
        // height + the threshold line; the accent reinforces it, rule 5/7).
        let colour = if v >= KP_STORM { T_STATE_ERROR } else { T_SIGNAL_DATA };
        out.push(fill_rect(crate::layout::Rect::new(x, y, bar_w, h), colour, Corner::C0));
    }

    // The current Kp readout (the "+gauge" half), top-left, with units.
    if ctx.lod >= Lod::Reduced {
        if let Some(v) = series.latest {
            let label = format_with_unit(v, view.chrome.unit.as_str());
            out.push(glyph(
                truncate(&label, body.w * 0.5, ctx.metrics.small_px),
                body.x + 3.0,
                body.y + ctx.metrics.small_px + 1.0,
                T_SCALE_XS,
                T_TEXT_TERTIARY,
            ));
        }
    }
    // A baseline hairline.
    out.push(line(body.x, body.bottom(), body.right(), body.bottom(), Style::hairline(T_HAIRLINE)));
    out
}

/// A generic bars renderer (auto-ranged) for any future `view = "bars"` stream. No stream uses it in
/// v1 (Kp is `bars+gauge`); it exists so the dispatch table's `bars` arm is honest rather than a
/// fallthrough to timeseries.
#[must_use]
pub fn render_bars(
    series: &SeriesView,
    view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = Vec::new();
    if body.w < 8.0 || body.h < 8.0 || series.points.is_empty() {
        return out;
    }
    let hi =
        series.points.iter().map(|(_, v)| *v).fold(f64::NEG_INFINITY, f64::max).max(f64::EPSILON);
    let n = series.points.len();
    let slot = body.w / n as f32;
    let bar_w = (slot * 0.7).max(1.0);
    for (i, &(_, v)) in series.points.iter().enumerate() {
        if !v.is_finite() {
            continue;
        }
        let h = ((v / hi).clamp(0.0, 1.0)) as f32 * body.h;
        let x = body.x + i as f32 * slot + (slot - bar_w) / 2.0;
        out.push(fill_rect(
            crate::layout::Rect::new(x, body.bottom() - h, bar_w, h),
            T_SIGNAL_DATA,
            Corner::C0,
        ));
    }
    out.push(line(body.x, body.bottom(), body.right(), body.bottom(), Style::hairline(T_HAIRLINE)));
    let _ = (view, ctx);
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
            stream_id: Some("swpc.kp"),
            chrome: CellChrome {
                title: "Kp".into(),
                description: "SW · 300 s".into(),
                status: CellStatus::Live,
                unit: "Kp".into(),
                stamp_utc: None,
                age_s: None,
                headline: None,
            },
            body: crate::view::Body::NoData,
        }
    }

    #[test]
    fn kp_bars_storm_threshold_and_readout() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        // A window with one storm-level bar (Kp 6) and quiet ones.
        let series = SeriesView {
            points: vec![(0, 2.0), (10_800, 6.0), (21_600, 3.0)],
            latest: Some(3.0),
            bipolar: false,
        };
        let items = render_kp(&series, &view(), &cell, &ctx(&p, &m));
        // Bars are rects; at least one uses the error accent (the Kp 6 storm bar).
        let storm_bar = items
            .iter()
            .any(|i| matches!(i, Item::Rect(r) if r.style.fill.as_deref() == Some(T_STATE_ERROR)));
        assert!(storm_bar, "a Kp ≥ 5 bar reads in the storm accent");
        // The threshold line is present.
        assert!(items
            .iter()
            .any(|i| matches!(i, Item::Polyline(pl) if pl.style.dash == Dash::Dashed)));
    }

    #[test]
    fn bars_stay_within_the_body() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let series = SeriesView {
            points: (0..24).map(|i| (i as i64 * 10_800, (i % 9) as f64)).collect(),
            latest: Some(1.0),
            bipolar: false,
        };
        let items = render_kp(&series, &view(), &cell, &ctx(&p, &m));
        for it in &items {
            if let Item::Rect(r) = it {
                assert!(
                    r.x >= cell.body.x - 0.5 && r.x + r.w <= cell.body.right() + 0.5,
                    "bar escapes body width"
                );
                assert!(r.y >= cell.body.y - 0.5, "bar above body top");
            }
        }
    }

    #[test]
    fn empty_series_draws_nothing() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let series = SeriesView { points: vec![], latest: None, bipolar: false };
        assert!(render_kp(&series, &view(), &cell, &ctx(&p, &m)).is_empty());
    }
}
