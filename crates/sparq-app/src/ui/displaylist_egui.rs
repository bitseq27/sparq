//! The egui back end of the display-list painter (WO-020 INC4 §8.1) — the on-canvas face.
//!
//! The SVG back end lives in `sparq-ui::displaylist` (headless, zero-dep); this is the same
//! resolved [`Painted`] list painted into an egui [`Painter`] at a screen offset and zoom. One
//! resolution, two painters (WO-019's property): the token ids were resolved ONCE by
//! [`sparq_ui::displaylist::Painter`], so both back ends draw the same colours, widths and dashes —
//! a re-theme moves both because both read the bundle, neither copies it.
//!
//! Dashes: egui strokes carry no dash pattern, so a dashed/dotted stroke is painted as its segment
//! decomposition (the same geometry the SVG `stroke-dasharray` approximates) — the non-colour
//! encoding survives the toolkit (token rule 7), which is what matters.

use egui::{Align2, Color32, Painter, Pos2, Stroke};
use sparq_ui::displaylist::{Dash, Painted, Rgba, Seg};

/// The dash pattern as (on, off) px at zoom 1 (scaled with z): the contract's two non-solid
/// patterns, on the look-board's idiom (dashed 6/4, dotted 1/4 — the SVG back end's arrays).
fn dash_pair(d: Dash) -> Option<(f32, f32)> {
    match d {
        Dash::Solid | Dash::Hidden => None,
        Dash::Dashed => Some((6.0, 4.0)),
        Dash::Dotted => Some((1.0, 4.0)),
    }
}

fn c32(c: Rgba) -> Color32 {
    Color32::from_rgba_premultiplied(
        (c.r as f32 * c.a) as u8,
        (c.g as f32 * c.a) as u8,
        (c.b as f32 * c.a) as u8,
        (c.a * 255.0) as u8,
    )
}

/// Flattens one subpath's segments into points (curves → 12 chords each — the egui face has no
/// curve primitive; the SVG face keeps the true curve. Both are the same geometry at review
/// tolerance; the interchange, not either painter, is the canonical form).
fn flatten(sub: &[Seg], closed: bool) -> Vec<Pos2> {
    let mut pts: Vec<Pos2> = Vec::new();
    let mut cur: Option<(f32, f32)> = None;
    let push = |pts: &mut Vec<Pos2>, x: f32, y: f32| pts.push(Pos2::new(x, y));
    for seg in sub {
        match *seg {
            Seg::Line(x, y) => {
                push(&mut pts, x, y);
                cur = Some((x, y));
            },
            Seg::Quad(cx, cy, x, y) => {
                let (x0, y0) = cur.unwrap_or((cx, cy));
                for i in 1..=12 {
                    let t = i as f32 / 12.0;
                    let u = 1.0 - t;
                    let px = u * u * x0 + 2.0 * u * t * cx + t * t * x;
                    let py = u * u * y0 + 2.0 * u * t * cy + t * t * y;
                    push(&mut pts, px, py);
                }
                cur = Some((x, y));
            },
            Seg::Cubic(a, b, c, d, x, y) => {
                let (x0, y0) = cur.unwrap_or((a, b));
                for i in 1..=12 {
                    let t = i as f32 / 12.0;
                    let u = 1.0 - t;
                    let px =
                        u * u * u * x0 + 3.0 * u * u * t * a + 3.0 * u * t * t * c + t * t * t * x;
                    let py =
                        u * u * u * y0 + 3.0 * u * u * t * b + 3.0 * u * t * t * d + t * t * t * y;
                    push(&mut pts, px, py);
                }
                cur = Some((x, y));
            },
        }
    }
    if closed && !pts.is_empty() {
        pts.push(pts[0]);
    }
    pts
}

/// Paints a resolved display list into `p`, mapped through `to_screen` (display-logical px →
/// screen px: the caller supplies band origin + zoom). `z` scales stroke widths and font sizes so
/// the wall reads at the canvas zoom like every other card furniture.
pub fn paint(p: &Painter, painted: &[Painted], z: f32, to_screen: impl Fn(f32, f32) -> Pos2) {
    for it in painted {
        match it {
            Painted::Rect { x, y, w, h, rx, fill, stroke, stroke_w, dash } => {
                let r = egui::Rect::from_min_size(
                    to_screen(*x, *y),
                    egui::vec2(w.max(0.0) * z, h.max(0.0) * z),
                );
                let rounding = egui::CornerRadius::same((rx * z).round() as u8);
                if let Some(f) = fill {
                    p.rect_filled(r, rounding, c32(*f));
                }
                if let Some(s) = stroke {
                    if *dash == Dash::Hidden {
                        continue;
                    }
                    if let Some((on, off)) = dash_pair(*dash) {
                        for (a, b) in dash_segments(&rect_loop(r), on * z, off * z) {
                            p.line_segment([a, b], Stroke::new(stroke_w * z, c32(*s)));
                        }
                    } else {
                        p.rect_stroke(
                            r,
                            rounding,
                            Stroke::new(stroke_w * z, c32(*s)),
                            egui::StrokeKind::Outside,
                        );
                    }
                }
            },
            Painted::Polyline { pts, stroke, stroke_w, dash } => {
                let Some(s) = stroke else { continue };
                if *dash == Dash::Hidden {
                    continue;
                }
                let sp: Vec<Pos2> = pts.iter().map(|(x, y)| to_screen(*x, *y)).collect();
                match dash_pair(*dash) {
                    None => {
                        for w in sp.windows(2) {
                            p.line_segment([w[0], w[1]], Stroke::new(stroke_w * z, c32(*s)));
                        }
                    },
                    Some((on, off)) => {
                        for (a, b) in dash_segments(&sp, on * z, off * z) {
                            p.line_segment([a, b], Stroke::new(stroke_w * z, c32(*s)));
                        }
                    },
                }
            },
            Painted::Path { subpaths, closed, fill, stroke, stroke_w, dash } => {
                // A filled multi-subpath (the coastline) paints as one egui convex-ish mesh per
                // subpath via Shape::convex_polyline; strokes paint per subpath.
                for sub in subpaths {
                    let pts = flatten(sub, *closed);
                    if pts.len() < 2 {
                        continue;
                    }
                    let mapped: Vec<Pos2> = pts.iter().map(|q| to_screen(q.x, q.y)).collect();
                    if let Some(f) = fill {
                        p.add(egui::Shape::convex_polygon(mapped.clone(), c32(*f), Stroke::NONE));
                    }
                    if let Some(s) = stroke {
                        if *dash == Dash::Hidden {
                            continue;
                        }
                        let segs = match dash_pair(*dash) {
                            None => mapped.windows(2).map(|w| (w[0], w[1])).collect::<Vec<_>>(),
                            Some((on, off)) => dash_segments(&mapped, on * z, off * z),
                        };
                        for (a, b) in segs {
                            p.line_segment([a, b], Stroke::new(stroke_w * z, c32(*s)));
                        }
                    }
                }
            },
            Painted::Arc { cx, cy, r, a0, a1, fill, stroke, stroke_w } => {
                let sweep = a1 - a0;
                let c = to_screen(*cx, *cy);
                let rr = r.max(0.0) * z;
                if sweep.abs() >= 359.999 {
                    if let Some(f) = fill {
                        p.circle_filled(c, rr, c32(*f));
                    }
                    if let Some(s) = stroke {
                        p.circle_stroke(c, rr, Stroke::new(stroke_w * z, c32(*s)));
                    }
                } else {
                    // Flatten the sweep into chords (egui has no arc primitive).
                    let steps = ((sweep.abs() / 5.0).ceil() as usize).max(2);
                    let pts: Vec<Pos2> = (0..=steps)
                        .map(|i| {
                            let ang = (a0 + sweep * i as f32 / steps as f32).to_radians();
                            Pos2::new(c.x + rr * ang.cos(), c.y + rr * ang.sin())
                        })
                        .collect();
                    if let Some(f) = fill {
                        p.add(egui::Shape::convex_polygon(pts.clone(), c32(*f), Stroke::NONE));
                    }
                    if let Some(s) = stroke {
                        for w in pts.windows(2) {
                            p.line_segment([w[0], w[1]], Stroke::new(stroke_w * z, c32(*s)));
                        }
                    }
                }
            },
            Painted::Text { text, x, y, size_px, colour } => {
                p.text(
                    to_screen(*x, *y),
                    Align2::LEFT_BOTTOM,
                    text,
                    egui::FontId::proportional(size_px * z),
                    c32(*colour),
                );
            },
            Painted::Dots { pts } => {
                for (x, y, r, c) in pts {
                    p.circle_filled(to_screen(*x, *y), r.max(0.5) * z, c32(*c));
                }
            },
            Painted::Heat { cells } => {
                for (x, y, w, h, c) in cells {
                    let r = egui::Rect::from_min_size(
                        to_screen(*x, *y),
                        egui::vec2(w * z + 0.5, h * z + 0.5),
                    );
                    p.rect_filled(r, 0.0, c32(*c));
                }
            },
        }
    }
}

/// The rectangle loop as a point ring (for dashed rect strokes).
fn rect_loop(r: egui::Rect) -> Vec<Pos2> {
    vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()]
}

/// Decomposes a polyline into dash segments of `on` px separated by `off` px gaps.
fn dash_segments(pts: &[Pos2], on: f32, off: f32) -> Vec<(Pos2, Pos2)> {
    let mut out = Vec::new();
    let cycle = on + off;
    if cycle <= 0.0 {
        return out;
    }
    let mut carried = 0.0f32; // distance into the current cycle at the start of this segment
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.distance(b);
        if len <= f32::EPSILON {
            continue;
        }
        let dir = (b - a) / len;
        let mut at = 0.0f32;
        while at < len {
            let phase = (carried + at) % cycle;
            if phase < on {
                let stop = (at + (on - phase)).min(len);
                out.push((a + dir * at, a + dir * stop));
                at = stop;
            } else {
                at += (cycle - phase).min(len - at);
            }
        }
        carried = (carried + len) % cycle;
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn dash_segments_cover_the_on_runs_of_a_straight_line() {
        let pts = vec![Pos2::new(0.0, 0.0), Pos2::new(30.0, 0.0)];
        let segs = dash_segments(&pts, 6.0, 4.0);
        let total: f32 = segs.iter().map(|(a, b)| a.distance(*b)).sum();
        // 30 px at 6/4: on-runs at 0-6, 10-16, 20-26, (30 cuts the last cycle) = 18 px of ink.
        assert!((total - 18.0).abs() < 1e-3, "ink {total}");
        for (a, b) in &segs {
            assert!(a.y == 0.0 && b.y == 0.0);
            assert!(b.x > a.x);
        }
    }

    #[test]
    fn flatten_keeps_endpoints_of_a_quad() {
        let sub = vec![Seg::Line(0.0, 0.0), Seg::Quad(5.0, 10.0, 10.0, 0.0)];
        let pts = flatten(&sub, false);
        assert_eq!(pts.first().copied(), Some(Pos2::new(0.0, 0.0)));
        assert_eq!(pts.last().copied(), Some(Pos2::new(10.0, 0.0)));
        assert!(pts.len() > 10, "the curve is chorded");
    }

    #[test]
    fn a_closed_subpath_returns_to_its_start() {
        let sub = vec![Seg::Line(0.0, 0.0), Seg::Line(4.0, 0.0), Seg::Line(0.0, 4.0)];
        let pts = flatten(&sub, true);
        assert_eq!(pts.first(), pts.last());
    }
}
