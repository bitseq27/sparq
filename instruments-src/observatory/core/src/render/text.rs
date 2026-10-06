//! The TEXT renderer (§5.5): the RAW feed, verbatim, wrapped and scrolling.
//!
//! The operator asked for "text tickers of the raw data streams" literally (plan §2/D8), so this
//! draws the payload as words — a WWV bulletin, a 3-day forecast, a TLE table — monospace-tabular
//! (the host type engine's job; the guest supplies the string + a size token), wrapped to the cell
//! width and scrolling upward from the pause-aware [`RenderCtx::scroll_phase`]. Overflow scrolls and
//! the oldest lines fall off the top; PAUSE freezes the scroll (the phase stops advancing in the wall).
//!
//! The display list has no clip primitive, so the renderer emits only the lines whose baseline falls
//! inside the body — a line scrolled off the top is simply not in the list (never drawn outside its
//! cell). The text is the record's `text` channel verbatim: no reformatting, no interpretation.

use crate::ir::{Item, Lod};
use crate::layout::CellRect;
use crate::render::{char_width, glyph, RenderCtx, T_SCALE_S, T_SIGNAL_DATA_DIM, T_TEXT_SECONDARY};
use crate::view::{CellView, TextView};

/// Vertical scroll speed, px per accumulated phase-second (the phase already carries `ticker_speed`).
pub const PX_PER_SEC: f64 = 12.0;

/// Wraps `lines` to `max_px` at `font_px`, by character count (the guest's width estimate — see
/// [`super::char_width`]). Verbatim: no trimming, no case change, no reflow beyond wrapping.
#[must_use]
pub fn wrap(lines: &[String], max_px: f32, font_px: f32) -> Vec<String> {
    let cw = char_width(font_px);
    let per_line = ((max_px / cw).floor().max(1.0)) as usize;
    let mut out = Vec::new();
    for l in lines {
        if l.is_empty() {
            out.push(String::new());
            continue;
        }
        let chars: Vec<char> = l.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let end = (i + per_line).min(chars.len());
            out.push(chars[i..end].iter().collect());
            i = end;
        }
    }
    out
}

/// Renders the raw text feed, scrolling.
#[must_use]
pub fn render(
    text: &TextView,
    _view: &CellView,
    cell: &CellRect,
    ctx: &RenderCtx<'_>,
) -> Vec<Item> {
    let body = cell.body;
    let mut out = Vec::new();
    if body.w < 8.0 || body.h < 8.0 || text.lines.is_empty() {
        return out;
    }
    let font = ctx.metrics.small_px;
    let line_h = font * 1.35;
    let wrapped = wrap(&text.lines, body.w - 6.0, font);
    if wrapped.is_empty() {
        return out;
    }
    let content_h = wrapped.len() as f64 * line_h as f64;
    // The scroll offset loops over (content + one body) so the text re-enters from the bottom after
    // the last line clears the top — a continuous marquee, oldest-first.
    let span = content_h + body.h as f64;
    let offset = if span > 0.0 { (ctx.scroll_phase * PX_PER_SEC).rem_euclid(span) } else { 0.0 };

    // A left accent rule (the data class, §5.6 rule 7 — the accent is class identity, not data colour).
    out.push(crate::render::line(
        body.x,
        body.y,
        body.x,
        body.bottom(),
        crate::ir::Style::hairline(T_SIGNAL_DATA_DIM),
    ));

    for (i, l) in wrapped.iter().enumerate() {
        let y = body.y + i as f64 as f32 * line_h - offset as f32 + font;
        // Emit only lines whose baseline is inside the body (no clip primitive; off-cell = not drawn).
        if y < body.y || y > body.bottom() {
            continue;
        }
        // Empty wrapped lines (blank source lines) are skipped — a glyph-run of "" draws nothing.
        if l.is_empty() {
            continue;
        }
        // At Minimal LOD, only the first visible line survives (§5.7: one value/headline).
        if ctx.lod == Lod::Minimal && i > 0 {
            break;
        }
        out.push(glyph(l.clone(), body.x + 4.0, y, T_SCALE_S, T_TEXT_SECONDARY));
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

    fn ctx_at<'a>(p: &'a Params, m: &'a Metrics, phase: f64, lod: Lod) -> RenderCtx<'a> {
        RenderCtx {
            params: p,
            metrics: m,
            lod,
            time_sec: phase,
            scroll_phase: phase,
            coast: &crate::coastline::EMPTY,
        }
    }
    fn view() -> CellView {
        CellView {
            stream_id: Some("swpc.wwv"),
            chrome: CellChrome {
                title: "WWV".into(),
                description: "SW · 600 s".into(),
                status: CellStatus::Live,
                unit: String::new(),
                stamp_utc: None,
                age_s: None,
                headline: None,
            },
            body: crate::view::Body::NoData,
        }
    }

    #[test]
    fn wrapping_respects_the_width_budget() {
        let long = "x".repeat(200);
        let w = wrap(&[long], 66.0, 11.0); // 66 / 6.6 = 10 chars per line
        assert!(w.len() >= 20, "a 200-char line wraps into many");
        assert!(w.iter().all(|l| crate::render::text_width(l, 11.0) <= 66.0 + char_width(11.0)));
    }

    #[test]
    fn text_draws_glyph_runs_inside_the_body() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let text = TextView {
            lines: vec!["Solar-terrestrial indices for 06 Oct".into(), "Ap 12".into()],
            issued_utc: 0,
        };
        let items = render(&text, &view(), &cell, &ctx_at(&p, &m, 0.0, Lod::Full));
        let runs: Vec<_> = items
            .iter()
            .filter_map(|i| match i {
                Item::GlyphRun(g) => Some(g),
                _ => None,
            })
            .collect();
        assert!(!runs.is_empty(), "the raw feed draws as glyph runs");
        for g in &runs {
            assert!(
                g.y >= cell.body.y && g.y <= cell.body.bottom() + m.small_px,
                "a run stays in the body: {g:?}"
            );
        }
    }

    #[test]
    fn scrolling_moves_the_text_and_pause_freezes_it() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let text = TextView {
            lines: (0..40).map(|i| format!("line {i} of the bulletin")).collect(),
            issued_utc: 0,
        };
        let a = render(&text, &view(), &cell, &ctx_at(&p, &m, 0.0, Lod::Full));
        let b = render(&text, &view(), &cell, &ctx_at(&p, &m, 5.0, Lod::Full));
        // Different phase → different visible window (the scroll moved).
        assert_ne!(a, b, "a phase change scrolls the text");
        // Same phase twice → identical (PAUSE holds the phase, so the render is stable).
        let c = render(&text, &view(), &cell, &ctx_at(&p, &m, 5.0, Lod::Full));
        assert_eq!(b, c, "an unchanged phase is a stable render (pause)");
    }

    #[test]
    fn minimal_lod_keeps_one_line() {
        let p = Params::default();
        let m = Metrics::default();
        let cell = CellRect::split(0, Rect::new(0.0, 0.0, 532.0, 268.0), &m, false, false);
        let text =
            TextView { lines: (0..40).map(|i| format!("line {i}")).collect(), issued_utc: 0 };
        let items = render(&text, &view(), &cell, &ctx_at(&p, &m, 0.0, Lod::Minimal));
        let runs = items.iter().filter(|i| matches!(i, Item::GlyphRun(_))).count();
        assert!(runs <= 1, "Minimal keeps at most one line (§5.7), got {runs}");
    }
}
