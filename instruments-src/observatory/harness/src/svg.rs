//! The harness's minimal SVG painter — the sandbox stand-in for INC4's `sparq-ui/src/displaylist.rs`.
//!
//! It consumes the SAME core [`Item`]s and resolves the SAME token ids against the SAME checked-in
//! bundle ([`crate::tokens::Tokens`]) as the host painter will, so the harness SVGs are a faithful
//! preview of what the device renders (WO-019's "identical through both painters from the same
//! display list", demonstrated in the sandbox before a runtime exists). It is deliberately minimal:
//! no tessellation, no GPU, no motion — just the eight primitives to SVG elements, colours resolved
//! to hex (this is a REVIEW artefact, not a token-conformant package file; the package's `preview.svg`
//! is validator-generated on the device, never this).
//!
//! # Fidelity notes (recorded, not hidden)
//!
//! * `heat-cells` at full grid resolution (the aurora's 180 × 90 = 16 200 cells) would make a
//!   multi-megabyte SVG of little review value, so the painter downsamples a heat grid to ≤
//!   [`HEAT_SVG_CELL_CAP`] cells for the SVG ONLY. The IR (and the golden) keep full resolution —
//!   the SVG is a preview, the IR is canonical.
//! * `trace-item` maps across "the item's box", which the frozen record does not carry; the painter
//!   maps it across the whole display (the full-display scope case the primitive was shaped for). The
//!   Observatory emits no traces (its per-cell series ride polylines — see `render/timeseries.rs`).

use observatory_core::ir::{Corner, Dash, Item, PathSegment, PointsItem, Surface};
use std::fmt::Write as _;

use crate::tokens::Tokens;

/// The most heat cells the SVG painter draws per grid (downsampling above this; see the module docs).
pub const HEAT_SVG_CELL_CAP: u32 = 8192;

/// The monospace stack (the bundle's `typography.family.mono`; the host type engine owns the real
/// face, the SVG approximates it so review renders read as the tabular scientific software they are).
const MONO: &str =
    "ui-monospace, 'Cascadia Mono', 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace";

/// Paints a display list to a standalone SVG string.
///
/// # Errors
/// Propagates only a `fmt::Error` (string writing), which cannot occur for an in-memory `String`;
/// the signature keeps it honest rather than `unwrap`ing.
pub fn render_svg(
    surface: &Surface,
    width: f32,
    height: f32,
    tokens: &Tokens,
) -> Result<String, std::fmt::Error> {
    let mut s = String::new();
    writeln!(s, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>")?;
    writeln!(
        s,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {width:.3} {height:.3}\" font-family=\"{MONO}\">"
    )?;
    // The display ground (the card surface the cells sit on; the gap between cells reads as this).
    let ground =
        tokens.color("color.ground.panel").map_or_else(|| "#151210".to_string(), |c| c.hex());
    writeln!(
        s,
        "<rect x=\"0\" y=\"0\" width=\"{width:.3}\" height=\"{height:.3}\" fill=\"{ground}\"/>"
    )?;

    if let Surface::Items(items) = surface {
        for item in items {
            paint_item(&mut s, item, width, height, tokens)?;
        }
    }
    writeln!(s, "</svg>")?;
    Ok(s)
}

/// Paints one item. An unresolved token skips the primitive with a diagnostic recorded on `tokens`
/// (the fail-soft rule) — the show goes on, and the harness prints the skip list.
fn paint_item(
    s: &mut String,
    item: &Item,
    dw: f32,
    dh: f32,
    tokens: &Tokens,
) -> Result<(), std::fmt::Error> {
    match item {
        Item::Rect(r) => {
            let (fill, fo) = paint_colour(r.style.fill.as_deref(), tokens);
            let (stroke, so) = paint_colour(r.style.stroke.as_deref(), tokens);
            let rx = corner_px(r.style.corner);
            let dash = dash_attr(r.style.dash);
            writeln!(
                s,
                "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\" rx=\"{rx:.1}\"{fill}{stroke}{dash}/>",
                r.x,
                r.y,
                r.w.max(0.0),
                r.h.max(0.0),
                fill = fill_attr(fill.as_deref(), fo),
                stroke = stroke_attr(stroke.as_deref(), so, r.style.stroke_width.px()),
            )?;
        },
        Item::GlyphRun(g) => {
            let size = tokens.size(&g.size).unwrap_or(11.0);
            let col = tokens.color(&g.colour).map_or_else(|| "#B9AC9C".to_string(), |c| c.hex());
            writeln!(
                s,
                "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{size:.2}\" fill=\"{col}\" xml:space=\"preserve\">{}</text>",
                g.x,
                g.y,
                xml_escape(&g.text)
            )?;
        },
        Item::Arc(a) => {
            let (fill, fo) = paint_colour(a.style.fill.as_deref(), tokens);
            let (stroke, so) = paint_colour(a.style.stroke.as_deref(), tokens);
            let sw = a.style.stroke_width.px();
            let sweep = a.a1_deg - a.a0_deg;
            if sweep.abs() >= 359.999 {
                // A full circle (the LED, a marker dot).
                writeln!(
                    s,
                    "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\"{fill}{stroke}/>",
                    a.cx,
                    a.cy,
                    a.r.max(0.0),
                    fill = fill_attr(fill.as_deref(), fo),
                    stroke = stroke_attr(stroke.as_deref(), so, sw),
                )?;
            } else {
                // An arc sweep (the gauge). Angles: 0° at +x, clockwise in y-down space.
                let (x0, y0) = arc_pt(a.cx, a.cy, a.r, a.a0_deg);
                let (x1, y1) = arc_pt(a.cx, a.cy, a.r, a.a1_deg);
                let large = if sweep.abs() > 180.0 { 1 } else { 0 };
                let sweep_flag = if sweep >= 0.0 { 1 } else { 0 };
                writeln!(
                    s,
                    "<path d=\"M {x0:.2} {y0:.2} A {:.2} {:.2} 0 {large} {sweep_flag} {x1:.2} {y1:.2}\" fill=\"none\"{stroke}/>",
                    a.r.max(0.0),
                    a.r.max(0.0),
                    stroke = stroke_attr(stroke.as_deref(), so, sw),
                )?;
            }
        },
        Item::Polyline(p) => {
            if p.points.len() < 2 {
                return Ok(());
            }
            let (stroke, so) = paint_colour(p.style.stroke.as_deref(), tokens);
            let sw = p.style.stroke_width.px();
            let dash = dash_attr(p.style.dash);
            let pts: Vec<String> =
                p.points.iter().map(|pt| format!("{:.2},{:.2}", pt.x, pt.y)).collect();
            writeln!(
                s,
                "<polyline points=\"{}\" fill=\"none\"{stroke}{dash}/>",
                pts.join(" "),
                stroke = stroke_attr(stroke.as_deref(), so, sw),
            )?;
        },
        Item::Path(p) => {
            let d = path_d(&p.segments, p.closed);
            let (fill, fo) = paint_colour(p.style.fill.as_deref(), tokens);
            let (stroke, so) = paint_colour(p.style.stroke.as_deref(), tokens);
            let sw = p.style.stroke_width.px();
            writeln!(
                s,
                "<path d=\"{d}\"{fill}{stroke}/>",
                fill = fill_attr(fill.as_deref(), fo),
                stroke = stroke_attr(stroke.as_deref(), so, sw),
            )?;
        },
        Item::Points(p) => paint_points(s, p, tokens)?,
        Item::HeatCells(h) => paint_heat(s, h, tokens)?,
        Item::Trace(t) => {
            // Map values across the whole display (see the module docs). The Observatory emits none.
            let n = t.values.len();
            if n >= 2 {
                let col =
                    tokens.color(&t.colour).map_or_else(|| "#8BE36A".to_string(), |c| c.hex());
                let pts: Vec<String> = t
                    .values
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let x = i as f32 / (n - 1) as f32 * dw;
                        let y = dh - v.clamp(0.0, 1.0) * dh;
                        format!("{x:.2},{y:.2}")
                    })
                    .collect();
                writeln!(
                    s,
                    "<polyline points=\"{}\" fill=\"none\" stroke=\"{col}\" stroke-width=\"{}\"/>",
                    pts.join(" "),
                    t.width.px()
                )?;
            }
        },
    }
    Ok(())
}

fn paint_points(s: &mut String, p: &PointsItem, tokens: &Tokens) -> Result<(), std::fmt::Error> {
    let r = tokens.size(&p.size).unwrap_or(4.0) as f32;
    let flat = p.colour.as_ref().and_then(|c| tokens.color(c));
    for (i, pt) in p.positions.iter().enumerate() {
        let fill = match (&p.colormap, &p.values) {
            (Some(cm), Some(vals)) => {
                let t = vals.get(i).copied().unwrap_or(0.0);
                if !t.is_finite() {
                    continue; // a NaN value = no point (mirrors the no-cell sentinel)
                }
                tokens.sample(cm, t).hex()
            },
            _ => flat.map_or_else(|| "#8BE36A".to_string(), |c| c.hex()),
        };
        writeln!(
            s,
            "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{r:.2}\" fill=\"{fill}\"/>",
            pt.x, pt.y
        )?;
    }
    Ok(())
}

fn paint_heat(
    s: &mut String,
    h: &observatory_core::ir::HeatCellsItem,
    tokens: &Tokens,
) -> Result<(), std::fmt::Error> {
    if h.cols == 0 || h.rows == 0 {
        return Ok(());
    }
    // Downsample for the SVG if the grid is huge (the IR keeps full resolution — module docs).
    let total = h.cols.saturating_mul(h.rows);
    let factor = if total > HEAT_SVG_CELL_CAP {
        ((total as f64 / HEAT_SVG_CELL_CAP as f64).sqrt().ceil()) as u32
    } else {
        1
    };
    let cols = (h.cols / factor).max(1);
    let rows = (h.rows / factor).max(1);
    let cw = h.w / cols as f32;
    let ch = h.h / rows as f32;
    for row in 0..rows {
        for col in 0..cols {
            // Sample the source grid at the downsampled cell centre.
            let sc = (col * factor + factor / 2).min(h.cols - 1);
            let sr = (row * factor + factor / 2).min(h.rows - 1);
            let idx = (sr * h.cols + sc) as usize;
            let v = h.values.get(idx).copied().unwrap_or(f32::NAN);
            if !v.is_finite() {
                continue; // NaN = no cell (the contract's sentinel)
            }
            let rgb = tokens.sample(&h.colormap, v);
            let x = h.x + col as f32 * cw;
            let y = h.y + row as f32 * ch;
            writeln!(
                s,
                "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" height=\"{ch:.2}\" fill=\"{}\"/>",
                cw + 0.5,
                rgb.hex()
            )?;
        }
    }
    Ok(())
}

/// A point on an arc, degrees → (x, y) in y-down space (0° at +x, clockwise).
fn arc_pt(cx: f32, cy: f32, r: f32, deg: f32) -> (f32, f32) {
    let a = deg.to_radians();
    (cx + r * a.cos(), cy + r * a.sin())
}

/// Builds an SVG path `d` from path segments, closing each subpath when `closed`.
fn path_d(segments: &[PathSegment], closed: bool) -> String {
    let mut d = String::new();
    for seg in segments {
        match seg {
            PathSegment::MoveTo(p) => {
                let _ = write!(d, "M {:.2} {:.2} ", p.x, p.y);
            },
            PathSegment::LineTo(p) => {
                let _ = write!(d, "L {:.2} {:.2} ", p.x, p.y);
            },
            PathSegment::QuadTo(c, p) => {
                let _ = write!(d, "Q {:.2} {:.2} {:.2} {:.2} ", c.x, c.y, p.x, p.y);
            },
            PathSegment::CubicTo(c1, c2, p) => {
                let _ = write!(
                    d,
                    "C {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} ",
                    c1.x, c1.y, c2.x, c2.y, p.x, p.y
                );
            },
        }
    }
    if closed {
        // Close each subpath: SVG's `Z` closes back to the last `M`, and a multi-subpath path with a
        // single trailing Z would only close the last one — so we emit Z before every M after the
        // first. The core's coastline is one multi-subpath path; this closes every ring.
        d = close_subpaths(&d);
    }
    d.trim().to_string()
}

/// Inserts a `Z` before each interior `M` and at the end, so every subpath of a multi-subpath path
/// closes (the coastline's per-ring closing convention).
fn close_subpaths(d: &str) -> String {
    let mut out = String::new();
    let mut first = true;
    for tok in d.split(' ') {
        if tok.is_empty() {
            continue;
        }
        if tok == "M" {
            if !first {
                out.push_str("Z ");
            }
            first = false;
        }
        out.push_str(tok);
        out.push(' ');
    }
    out.push_str("Z ");
    out
}

fn corner_px(c: Corner) -> f32 {
    match c {
        Corner::C0 => 0.0,
        Corner::C2 => 2.0,
        Corner::C4 => 4.0,
    }
}

/// Resolves a paint token id to (hex, opacity). A missing/unresolved id yields (None, 1.0) so the
/// attribute is omitted (the element draws with no fill/stroke rather than a guessed colour).
fn paint_colour(id: Option<&str>, tokens: &Tokens) -> (Option<String>, f32) {
    match id {
        Some(id) => match tokens.color(id) {
            Some(c) => (Some(c.hex()), tokens.opacity_for(id)),
            None => (None, 1.0),
        },
        None => (None, 1.0),
    }
}

fn fill_attr(hex: Option<&str>, opacity: f32) -> String {
    match hex {
        Some(h) if opacity >= 0.999 => format!(" fill=\"{h}\""),
        Some(h) => format!(" fill=\"{h}\" fill-opacity=\"{opacity:.3}\""),
        None => " fill=\"none\"".to_string(),
    }
}

fn stroke_attr(hex: Option<&str>, opacity: f32, sw: f32) -> String {
    match hex {
        Some(h) if opacity >= 0.999 => format!(" stroke=\"{h}\" stroke-width=\"{sw:.1}\""),
        Some(h) => {
            format!(" stroke=\"{h}\" stroke-opacity=\"{opacity:.3}\" stroke-width=\"{sw:.1}\"")
        },
        None => String::new(),
    }
}

fn dash_attr(d: Dash) -> String {
    match d {
        Dash::Solid => String::new(),
        Dash::Dashed => " stroke-dasharray=\"6 4\"".to_string(),
        Dash::Dotted => " stroke-dasharray=\"1 4\"".to_string(),
        Dash::Hidden => " stroke-opacity=\"0\"".to_string(),
    }
}

/// XML-escapes text content (the raw feeds carry `<`, `&`, quotes — they must not break the SVG).
#[must_use]
pub fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            // Strip control chars (raw feeds may carry them); keep the printable + tab.
            c if (c as u32) < 0x20 && c != '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A convenience: paint a surface, returning the SVG and the unresolved-token diagnostics.
pub fn render_svg_report(
    surface: &Surface,
    w: f32,
    h: f32,
    tokens: &Tokens,
) -> (String, Vec<String>) {
    let svg = render_svg(surface, w, h, tokens)
        .unwrap_or_else(|e| format!("<svg><!-- render error: {e} --></svg>"));
    (svg, tokens.unresolved())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use observatory_core::ir::{GlyphRunItem, HeatCellsItem, Point, PolylineItem, RectItem, Style};

    #[test]
    fn xml_escaping_neutralises_raw_feed_characters() {
        assert_eq!(xml_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(xml_escape("\"q\""), "&quot;q&quot;");
    }

    #[test]
    fn a_rect_paints_with_its_resolved_fill() {
        let tokens = Tokens::load().unwrap();
        let items = Surface::Items(vec![Item::Rect(RectItem {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            style: Style::fill("color.ground.inset"),
        })]);
        let svg = render_svg(&items, 100.0, 100.0, &tokens).unwrap();
        assert!(svg.contains("<rect"), "a rect element is emitted");
        assert!(svg.to_lowercase().contains("#050403"), "the inset ground resolves to its hex");
    }

    #[test]
    fn an_unresolved_token_is_skipped_not_guessed() {
        let tokens = Tokens::load().unwrap();
        let items = Surface::Items(vec![Item::GlyphRun(GlyphRunItem {
            text: "x".into(),
            x: 0.0,
            y: 0.0,
            size: "typography.scale.s".into(),
            colour: "color.does.not.exist".into(),
        })]);
        let svg = render_svg(&items, 100.0, 100.0, &tokens).unwrap();
        // The glyph still draws (text is content), but the unresolved colour fell back and is listed.
        assert!(svg.contains("<text"), "the text still renders (fail-soft)");
        assert!(tokens.unresolved().contains(&"color.does.not.exist".to_string()));
    }

    #[test]
    fn heat_downsamples_a_huge_grid_for_the_svg() {
        let tokens = Tokens::load().unwrap();
        // A 180×90 aurora-sized grid (16 200 cells) must not emit 16 200 rects.
        let values = vec![0.5f32; 180 * 90];
        let items = Surface::Items(vec![Item::HeatCells(HeatCellsItem {
            x: 0.0,
            y: 0.0,
            w: 360.0,
            h: 180.0,
            cols: 180,
            rows: 90,
            values,
            colormap: "colormap.thermal".into(),
        })]);
        let svg = render_svg(&items, 400.0, 200.0, &tokens).unwrap();
        let rects = svg.matches("<rect").count() - 1; // minus the background rect
        assert!(
            rects <= HEAT_SVG_CELL_CAP as usize,
            "downsampled to <= {HEAT_SVG_CELL_CAP}, got {rects}"
        );
        assert!(rects > 0, "some cells still draw");
    }

    #[test]
    fn a_nan_heat_cell_is_not_drawn() {
        let tokens = Tokens::load().unwrap();
        let items = Surface::Items(vec![Item::HeatCells(HeatCellsItem {
            x: 0.0,
            y: 0.0,
            w: 20.0,
            h: 10.0,
            cols: 2,
            rows: 1,
            values: vec![0.5, f32::NAN],
            colormap: "colormap.thermal".into(),
        })]);
        let svg = render_svg(&items, 20.0, 10.0, &tokens).unwrap();
        let rects = svg.matches("<rect").count() - 1;
        assert_eq!(rects, 1, "the NaN cell is skipped, the finite one draws");
    }

    #[test]
    fn a_closed_multisubpath_closes_every_ring() {
        // Two triangles in one path, closed: each subpath must get its own Z.
        let segs = vec![
            PathSegment::MoveTo(Point::new(0.0, 0.0)),
            PathSegment::LineTo(Point::new(1.0, 0.0)),
            PathSegment::LineTo(Point::new(0.0, 1.0)),
            PathSegment::MoveTo(Point::new(5.0, 5.0)),
            PathSegment::LineTo(Point::new(6.0, 5.0)),
            PathSegment::LineTo(Point::new(5.0, 6.0)),
        ];
        let d = path_d(&segs, true);
        assert_eq!(d.matches('Z').count(), 2, "both subpaths close, got: {d}");
    }

    #[test]
    fn a_polyline_paints_its_points() {
        let tokens = Tokens::load().unwrap();
        let items = Surface::Items(vec![Item::Polyline(PolylineItem {
            points: vec![Point::new(0.0, 0.0), Point::new(10.0, 10.0)],
            style: Style::trace("color.signal.data.colour"),
        })]);
        let svg = render_svg(&items, 20.0, 20.0, &tokens).unwrap();
        assert!(svg.contains("<polyline"), "a polyline element is emitted");
        assert!(svg.contains("0.00,0.00 10.00,10.00"), "both points are in the element");
    }

    #[test]
    fn a_full_circle_arc_becomes_a_circle_element() {
        let tokens = Tokens::load().unwrap();
        let items = Surface::Items(vec![Item::Arc(observatory_core::ir::ArcItem {
            cx: 5.0,
            cy: 5.0,
            r: 3.0,
            a0_deg: 0.0,
            a1_deg: 360.0,
            style: Style::fill("color.signal.data.colour"),
        })]);
        let svg = render_svg(&items, 10.0, 10.0, &tokens).unwrap();
        assert!(svg.contains("<circle"), "a 0→360 arc is a circle element");
    }
}
