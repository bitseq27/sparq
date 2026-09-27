//! The canvas, drawn (WO-013 increment 1): nodes, wires, ports, the in-flight wire, the marquee
//! and the long-press menu — all from the computed [`CanvasLayout`], in the token language.
//!
//! This is the drawing half of the WO-012 rule *"wrap egui: all widgets go through your own
//! gesture layer so the Phase 6 swap only replaces the drawing half."* Nothing here interprets
//! input or owns state: it reads `sparq_ui::canvas`'s computed geometry and paints it, then
//! registers the touch targets for the layout audit. Replace egui tomorrow and this file changes;
//! the canvas core does not.
//!
//! No colour or size literal may appear here (`token_audit.py` R6): every colour is a `Palette`
//! field or a `COLOR_*` token, every dimension a `LAYOUT_*` token. Signal class is never carried by
//! colour alone — wires take their token stroke *encoding* (solid / thin / dashed / dotted /
//! double) and ports wear their token letter (A/C/E/D/S), per the look-board's redundant-encoding
//! rule.

use egui::{Align2, Color32, Painter, Pos2, Stroke, StrokeKind};
use sparq_ui::audit::{InteractiveElement, TouchClass};
use sparq_ui::canvas::browser;
use sparq_ui::canvas::camera::Lod;
use sparq_ui::canvas::connect::{self, ConnectContext, Preview};
use sparq_ui::canvas::interact::{CanvasState, Interaction};
use sparq_ui::canvas::layout::{CanvasLayout, NodeLayout, SignalClass, WireEndSide};
use sparq_ui::canvas::model::{Graph, Node, NodeId};
use sparq_ui::geom::{Rect, Vec2};
use sparq_ui::tokens::*;

use crate::ui::adapter::{c32, egui_rect, font_s, font_xs, Palette};

/// Paint the whole canvas for one frame. `view` is the canvas rect (screen px); `audit` collects
/// the touch targets the layout audit measures.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    layout: &CanvasLayout,
    view: Rect,
    ctx: &ConnectContext<'_>,
    audit: &mut Vec<InteractiveElement>,
) {
    draw_grid(p, pal, canvas, view);
    draw_wires(p, pal, graph, canvas, layout);
    draw_pending_wire(p, pal, graph, canvas, layout, ctx);
    draw_repatch(p, pal, graph, canvas, layout, ctx);
    // The resolved master (explicit or the documented default rule) wears a MASTER badge: the
    // user must be able to see which node the render will carry, in words, before asking.
    let master_id = canvas.resolve_master(graph);
    draw_nodes(p, pal, graph, canvas, layout, master_id, audit);
    draw_marquee(p, pal, canvas);
    draw_menu(p, pal, canvas, view, audit);
    draw_browser(p, pal, canvas, view, audit);
    draw_rename(p, pal, canvas, view, audit);
}

// --------------------------------------------------------------------- colour helpers

fn class_colour(c: SignalClass, pal: &Palette) -> Color32 {
    match c {
        SignalClass::Audio => pal.audio,
        SignalClass::Cv => pal.cv,
        SignalClass::Event => pal.event,
        SignalClass::Data => pal.data,
        SignalClass::Spatial => pal.spatial,
        SignalClass::Neutral => pal.text_secondary,
    }
}

fn class_glow(c: SignalClass, pal: &Palette) -> Color32 {
    match c {
        SignalClass::Audio => c32(&COLOR_SIGNAL_AUDIO_GLOW),
        SignalClass::Cv => c32(&COLOR_SIGNAL_CV_GLOW),
        SignalClass::Event => c32(&COLOR_SIGNAL_EVENT_GLOW),
        SignalClass::Data => c32(&COLOR_SIGNAL_DATA_GLOW),
        SignalClass::Spatial => c32(&COLOR_SIGNAL_SPATIAL_GLOW),
        SignalClass::Neutral => pal.text_primary,
    }
}

fn class_dim(c: SignalClass, pal: &Palette) -> Color32 {
    match c {
        SignalClass::Audio => c32(&COLOR_SIGNAL_AUDIO_DIM),
        SignalClass::Cv => c32(&COLOR_SIGNAL_CV_DIM),
        SignalClass::Event => c32(&COLOR_SIGNAL_EVENT_DIM),
        SignalClass::Data => c32(&COLOR_SIGNAL_DATA_DIM),
        SignalClass::Spatial => c32(&COLOR_SIGNAL_SPATIAL_DIM),
        SignalClass::Neutral => pal.text_disabled,
    }
}

fn class_label(c: SignalClass) -> &'static str {
    match c {
        SignalClass::Audio => COLOR_SIGNAL_AUDIO_LABEL,
        SignalClass::Cv => COLOR_SIGNAL_CV_LABEL,
        SignalClass::Event => COLOR_SIGNAL_EVENT_LABEL,
        SignalClass::Data => COLOR_SIGNAL_DATA_LABEL,
        SignalClass::Spatial => COLOR_SIGNAL_SPATIAL_LABEL,
        SignalClass::Neutral => "",
    }
}

fn class_encoding(c: SignalClass) -> &'static str {
    match c {
        SignalClass::Audio => COLOR_SIGNAL_AUDIO_ENCODING,
        SignalClass::Cv => COLOR_SIGNAL_CV_ENCODING,
        SignalClass::Event => COLOR_SIGNAL_EVENT_ENCODING,
        SignalClass::Data => COLOR_SIGNAL_DATA_ENCODING,
        SignalClass::Spatial => COLOR_SIGNAL_SPATIAL_ENCODING,
        SignalClass::Neutral => COLOR_SIGNAL_CV_ENCODING,
    }
}

/// The stroke width for a class: `cv` is the thin solid ("solid-thin"), everything else the signal
/// width. Both are token stroke widths, never invented.
fn class_width(c: SignalClass) -> f32 {
    match c {
        SignalClass::Cv => LAYOUT_STROKE_HAIRLINE as f32,
        _ => LAYOUT_STROKE_SIGNAL as f32,
    }
}

fn pos(v: Vec2) -> Pos2 {
    Pos2::new(v.x, v.y)
}

/// Blend two token colours by `t` (0..1), per channel. The live-wire-level modulation uses this to
/// fade a wire from its class colour (at rest) toward its class glow (hot) without inventing a
/// third colour — egui's `Color32` has no built-in lerp, so the two-endpoint blend lives here.
fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let mix = |x: u8, y: u8| -> u8 {
        (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// The same token colour at an explicit alpha — the under-glow's transparency, scaled by level.
fn with_alpha(c: Color32, a: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Hatch a rect with 45° hairlines — the BYPASS state pattern (increment 5, the mockup's
/// `hatch8`: 8 px horizontal pitch, hairline weight, low alpha over the body fill). A pattern,
/// not a colour: it survives greyscale and it survives LOD, which words do not (look-board §4:
/// "states use pattern first — colour is redundant").
fn hatch_rect(p: &Painter, r: egui::Rect, col: Color32) {
    let step = LAYOUT_SPACE_2 as f32 * std::f32::consts::SQRT_2; // 8 px horizontal pitch at 45°
    let stroke = Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, col);
    let mut c = r.min.x + r.min.y;
    let end = r.max.x + r.max.y;
    while c <= end {
        // The segment of the diagonal x + y = c that lies inside the rect.
        let lo = (c - r.max.y).max(r.min.x);
        let hi = (c - r.min.y).min(r.max.x);
        if hi >= lo {
            p.line_segment([Pos2::new(lo, c - lo), Pos2::new(hi, c - hi)], stroke);
        }
        c += step;
    }
}

/// Stroke a rect's outline as a dash run — the MUTE state pattern (increment 5). The dash
/// lengths ride the 8 px space scale (on = `space.2`, off = `space.1`), the same budget the wire
/// class encodings use, so no dash number here is invented either.
fn dashed_rect(p: &Painter, r: egui::Rect, stroke: Stroke) {
    let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    draw_dashed(p, &pts, LAYOUT_SPACE_2 as f32, LAYOUT_SPACE_1 as f32, stroke);
}

// --------------------------------------------------------------------- grid

fn draw_grid(p: &Painter, pal: &Palette, canvas: &CanvasState, view: Rect) {
    let cam = &canvas.camera;
    let lod = cam.lod();
    if lod == Lod::Dot {
        return; // data-ink: no grid when the graph is dots
    }
    let minor = LAYOUT_CANVAS_GRID_MINOR as f32;
    let major = LAYOUT_CANVAS_GRID_MAJOR as f32;
    let dot_r = LAYOUT_CANVAS_GRID_DOT_RADIUS as f32;
    let dot_col = pal.hairline_colour.gamma_multiply(LAYOUT_CANVAS_GRID_DOT_OPACITY);
    let major_col = pal.hairline_colour.gamma_multiply(pal.hairline_faint);
    // At Simplified LOD only the major grid survives (keeps the dot count — and the frame — sane).
    let step = if lod == Lod::Full { minor } else { major };

    let w0 = cam.to_world(view.min, view);
    let w1 = cam.to_world(view.max, view);
    let start_x = (w0.x / step).floor() * step;
    let start_y = (w0.y / step).floor() * step;
    let mut wy = start_y;
    while wy <= w1.y {
        let mut wx = start_x;
        while wx <= w1.x {
            let s = cam.to_screen(Vec2::new(wx, wy), view);
            let on_major = (wx % major).abs() < 0.5 && (wy % major).abs() < 0.5;
            let col = if on_major { major_col } else { dot_col };
            let r = if on_major { dot_r * 2.0 } else { dot_r };
            p.circle_filled(pos(s), r, col);
            wx += step;
        }
        wy += step;
    }
}

// --------------------------------------------------------------------- wires

/// Parse a token encoding string ("dashed-6-3", "dotted-2-4", "double-stroke", "solid-*") into a
/// dash pattern and a double-stroke flag. The numbers come from the token, not from here.
fn parse_encoding(enc: &str) -> (Option<(f32, f32)>, bool) {
    let parts: Vec<&str> = enc.split('-').collect();
    let double = parts.first().is_some_and(|h| *h == "double");
    let nums: Vec<f32> = parts.iter().filter_map(|s| s.parse::<f32>().ok()).collect();
    let pattern = if nums.len() >= 2 { Some((nums[0], nums[1])) } else { None };
    (pattern, double)
}

fn draw_wires(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    layout: &CanvasLayout,
) {
    // Live wire levels (WO-013 increment 4): when the bridge has supplied meter-driven levels, a
    // wire carrying signal brightens from its class colour toward its class glow and grows a soft
    // under-glow, in proportion to the level. The levels come from the executor's meters (never
    // faked); an empty level set (nothing rendered yet) leaves every wire exactly at rest, so this
    // is purely additive over the increment-3 rendering. Both colours are tokens — the modulation
    // interpolates between two token colours and never invents a third.
    let live = !canvas.levels.is_empty();
    for w in &layout.wires {
        let selected = canvas.selection.wires.contains(&w.id);
        let enc = class_encoding(w.class);
        let (pattern, double) = parse_encoding(enc);
        let level = if live {
            sparq_ui::canvas::levels::wire_level(graph, &canvas.levels, w.id)
        } else {
            0.0
        };
        // Width: selection emphasises at any LOD; otherwise the Dot contract is literal —
        // "a colour-coded dot per node, HAIRLINE wires" (camera.rs) — the class widths belong to
        // the zoomed-in language, where the dash/double encodings are actually legible.
        let width = if selected {
            LAYOUT_STROKE_EMPHASIS as f32
        } else if layout.lod == Lod::Dot {
            LAYOUT_STROKE_HAIRLINE as f32
        } else {
            class_width(w.class)
        };
        let colour = if selected {
            class_glow(w.class, pal)
        } else if level > 0.0 {
            lerp_colour(class_colour(w.class, pal), class_glow(w.class, pal), level)
        } else {
            class_colour(w.class, pal)
        };
        let stroke = Stroke::new(width, colour);
        let pts: Vec<Pos2> = w.points.iter().copied().map(pos).collect();

        // The under-glow: a wider, low-alpha pass in the class glow, drawn first so the crisp
        // stroke sits on top. Alpha scales with the level, so a hot wire visibly "energises".
        if level > 0.02 && layout.lod != Lod::Dot {
            let glow = with_alpha(class_glow(w.class, pal), 0.15 + 0.45 * level);
            let under = Stroke::new(width + LAYOUT_SPACE_1 as f32 * (0.5 + level), glow);
            p.line(pts.clone(), under);
        }

        if let Some((on, off)) = pattern {
            draw_dashed(p, &pts, on, off, stroke);
        } else if double {
            let off = LAYOUT_STROKE_HAIRLINE as f32 * 2.0;
            let a: Vec<Pos2> = pts.iter().map(|q| Pos2::new(q.x, q.y - off)).collect();
            let b: Vec<Pos2> = pts.iter().map(|q| Pos2::new(q.x, q.y + off)).collect();
            p.line(a, stroke);
            p.line(b, stroke);
        } else {
            p.line(pts.clone(), stroke);
        }

        if w.conversion && layout.lod != Lod::Dot {
            // The one silent conversion (multi → mono) is flagged in WORDS, not colour alone —
            // at every LOD where words are legible. At Dot the word is suppressed (unreadable at
            // that scale by definition); the class encoding survives and the flag returns on
            // zoom-in. Declared, not silently dropped.
            if let Some(mid) = midpoint(&pts) {
                p.text(mid, Align2::CENTER_CENTER, "SUM", font_xs(), pal.warning);
            }
        }

        // Re-patch handles: wherever the layout says a wire end is grabbable
        // (`layout::hit_test`'s WireEnd zones are exactly these points), draw it — what you can
        // touch is what you see, at every LOD where the ends are targetable.
        if layout.lod != Lod::Dot {
            let r = LAYOUT_SPACE_1 as f32;
            let tick = pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32);
            p.circle_stroke(pos(w.grab_from), r, tick);
            p.circle_stroke(pos(w.grab_to), r, tick);
        }
    }
}

fn midpoint(pts: &[Pos2]) -> Option<Pos2> {
    pts.get(pts.len() / 2).copied()
}

/// Walk a polyline at ~1 px and draw the "on" runs of an `on`/`off` dash pattern. Sampling dense
/// keeps the token dash lengths accurate to a pixel; the polylines are short (a wire is a few
/// hundred px), so this stays cheap for the graphs increment 1 targets.
fn draw_dashed(p: &Painter, pts: &[Pos2], on: f32, off: f32, stroke: Stroke) {
    if pts.len() < 2 {
        return;
    }
    let period = (on + off).max(1e-3);
    let mut run: Vec<Pos2> = Vec::new();
    let mut arc = 0.0f32;
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        let n = ((seg / 1.0).ceil() as usize).max(1);
        for i in 0..=n {
            let t = i as f32 / n as f32;
            let pt = Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
            let is_on = ((arc + seg * t) % period) < on;
            if is_on {
                run.push(pt);
            } else if run.len() >= 2 {
                p.line(std::mem::take(&mut run), stroke);
            } else {
                run.clear();
            }
        }
        arc += seg;
    }
    if run.len() >= 2 {
        p.line(run, stroke);
    }
}

// --------------------------------------------------------------------- pending wire

fn draw_pending_wire(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    layout: &CanvasLayout,
    ctx: &ConnectContext<'_>,
) {
    let Some(pw) = canvas.pending_wire() else { return };
    let find_port = |pref: sparq_ui::canvas::model::PortRef| -> Option<(Vec2, SignalClass)> {
        layout
            .nodes
            .iter()
            .find_map(|n| n.ports.iter().find(|pl| pl.pref == pref))
            .map(|pl| (pl.screen, pl.class))
    };

    // Affordances: every other port glows or dims by its verdict against the dragged source.
    let glow_r = LAYOUT_TOUCH_PORT_RADIUS as f32 + LAYOUT_SPACE_1 as f32;
    let dim_r = LAYOUT_TOUCH_PORT_RADIUS as f32;
    for n in &layout.nodes {
        for pl in &n.ports {
            if pl.pref == pw.from {
                continue;
            }
            let pv = connect::preview(graph, pw.from, pl.pref, ctx);
            let (col, r) = match pv {
                Preview::Compatible | Preview::Conversion | Preview::Adapter(true) => {
                    (class_glow(pl.class, pal), glow_r)
                },
                Preview::Adapter(false) | Preview::Dim => (class_dim(pl.class, pal), dim_r),
            };
            p.circle_filled(pos(pl.screen), r, col);
        }
    }

    // The in-flight wire: source port → hovered port (magnet) or the raw cursor.
    if let Some((src, _)) = find_port(pw.from) {
        let end = pw.hovered.and_then(find_port).map(|(s, _)| s).unwrap_or(pw.cursor_screen);
        let stroke = Stroke::new(LAYOUT_STROKE_SIGNAL as f32, pal.text_secondary);
        p.line_segment([pos(src), pos(end)], stroke);
        // The source port itself is armed: draw it filled and ringed.
        p.circle_filled(pos(src), glow_r, pal.text_primary);
    }
}

// --------------------------------------------------------------------- nodes

#[allow(clippy::too_many_arguments)]
fn draw_nodes(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    layout: &CanvasLayout,
    master_id: Option<NodeId>,
    audit: &mut Vec<InteractiveElement>,
) {
    for nl in &layout.nodes {
        let Some(node) = graph.node(nl.id) else { continue };
        let selected = canvas.selection.nodes.contains(&nl.id);
        let is_master = master_id == Some(nl.id);
        match layout.lod {
            Lod::Dot => draw_node_dot(p, pal, node, nl, selected, is_master),
            Lod::Simplified => draw_node_box(p, pal, node, nl, selected, is_master, false, audit),
            Lod::Full => draw_node_box(p, pal, node, nl, selected, is_master, true, audit),
        }
    }
}

fn dominant_class(node: &Node) -> SignalClass {
    use sparq_module_api::port::Direction;
    let pick = |dir: Direction| {
        node.spec
            .ports
            .iter()
            .enumerate()
            .find(|(_, pt)| pt.direction == dir)
            .map(|(_, pt)| sparq_ui::canvas::layout::signal_class(pt))
    };
    pick(Direction::Out).or_else(|| pick(Direction::In)).unwrap_or(SignalClass::Neutral)
}

/// A node at Dot LOD, with its states as SHAPES (increment 5 — the Dot contract is "a
/// colour-coded dot per node", and at 8 px words are not an option): bypassed = a HOLLOW ring
/// (the signal goes around it), muted = a DIMMED fill, locked = a concentric hairline ring,
/// master = a `selected`-colour hairline ring (selection's own ring is emphasis-WEIGHT at the
/// same radius, so the two never rely on colour alone to tell them apart).
fn draw_node_dot(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    selected: bool,
    master: bool,
) {
    let c = pos(nl.screen.center());
    let r = LAYOUT_SPACE_2 as f32;
    let col = class_colour(dominant_class(node), pal);
    let dimmed = node.flags.bypassed || node.flags.muted;
    let fill_col = if dimmed { col.gamma_multiply(0.4) } else { col };
    if node.flags.bypassed {
        p.circle_stroke(c, r, Stroke::new(LAYOUT_STROKE_SIGNAL as f32, fill_col));
    } else {
        p.circle_filled(c, r, fill_col);
    }
    if node.flags.locked {
        p.circle_stroke(
            c,
            r + LAYOUT_SPACE_1 as f32 * 0.5,
            pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32),
        );
    }
    if master {
        p.circle_stroke(
            c,
            r + LAYOUT_SPACE_1 as f32,
            Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, pal.selected),
        );
    }
    if selected {
        p.circle_stroke(
            c,
            r + LAYOUT_SPACE_1 as f32,
            Stroke::new(LAYOUT_STROKE_EMPHASIS as f32, pal.selected),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_node_box(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    selected: bool,
    master: bool,
    full: bool,
    audit: &mut Vec<InteractiveElement>,
) {
    let body = egui_rect(nl.screen);
    let corner = LAYOUT_CANVAS_NODE_RADIUS as u8;
    let dimmed = node.flags.bypassed || node.flags.muted;
    let fill = if dimmed { pal.ground_panel.gamma_multiply(0.6) } else { pal.ground_panel };
    p.rect_filled(body, corner, fill);

    // State patterns (increment 5, look-board §4 "pattern first — colour is redundant"): the
    // shapes survive the LOD reduction AND the monochrome test, which words and tints do not.
    // BYPASSED hatches the body — the mockup's own encoding (`design-mode.svg`'s `hatch8`).
    // The header band draws over the hatch, so the pattern reads as "the body is bypassed".
    if node.flags.bypassed {
        hatch_rect(p, body, pal.hairline_colour.gamma_multiply(0.35));
    }

    // Border: selected wears the selection accent at emphasis width; else the regular hairline —
    // and a MUTED node's border is dashed (the state pattern; the selection accent outranks it,
    // because "which node am I editing" beats "which node is silent" at the border's one job).
    let border = if selected {
        Stroke::new(LAYOUT_STROKE_EMPHASIS as f32, pal.selected)
    } else {
        pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32)
    };
    if node.flags.muted && !selected {
        dashed_rect(p, body, border);
    } else {
        p.rect_stroke(body, corner, border, StrokeKind::Middle);
    }
    // LOCKED: a second, inset border — the "double" pattern the stroke language already uses for
    // a doubled signal, here meaning "this node is pinned in place".
    if node.flags.locked {
        p.rect_stroke(
            body.shrink(LAYOUT_SPACE_1 as f32),
            corner,
            pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
            StrokeKind::Middle,
        );
    }

    // Header band.
    let header = egui_rect(nl.header_screen);
    p.rect_filled(header, corner, pal.ground_panel_alt);
    p.line_segment(
        [header.left_bottom(), header.right_bottom()],
        pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
    );

    if full {
        p.text(
            header.left_center() + egui::vec2(LAYOUT_SPACE_2 as f32, 0.0),
            Align2::LEFT_CENTER,
            node.title(),
            font_s(),
            if dimmed { pal.text_disabled } else { pal.text_primary },
        );
    } else if master {
        // MASTER at Simplified: the word is gone with every other text, but the role must still
        // be readable — a filled chip in the badge's own accent sits where the word would be.
        // The pattern set (hatch/dash/double) stays distinguishable from it: those are outlines
        // on the body, this is a solid on the header.
        let chip = LAYOUT_SPACE_2 as f32;
        let c = header.right_center() - egui::vec2(LAYOUT_SPACE_2 as f32 + chip / 2.0, 0.0);
        p.rect_filled(
            egui::Rect::from_center_size(c, egui::vec2(chip, chip)),
            LAYOUT_CORNER_MICRO as u8,
            pal.selected,
        );
    }

    // Flag badges, in WORDS — at Full LOD only (increment 5: the Simplified contract is "node
    // box, coloured ports, NO TEXT", and the patterns above are what carries the states down
    // there). At Full the word rides ON TOP of the pattern: redundant encoding, never either
    // alone — the mockup shows exactly this pairing for bypass (hatch + "bypassed").
    if full {
        let mut badge_x = header.right_center().x - LAYOUT_SPACE_2 as f32;
        for (on, label, col) in [
            (master, "MASTER", pal.selected),
            (node.flags.locked, "LOCK", pal.text_tertiary),
            (node.flags.muted, "MUTE", pal.text_disabled),
            (node.flags.bypassed, "BYPASS", pal.warning),
        ] {
            if !on {
                continue;
            }
            let g = font_xs();
            let w = label.len() as f32 * LAYOUT_SPACE_2 as f32;
            badge_x -= w;
            p.text(Pos2::new(badge_x, header.center().y), Align2::LEFT_CENTER, label, g, col);
            badge_x -= LAYOUT_SPACE_2 as f32;
        }
    }

    // Ports.
    for pl in &nl.ports {
        let col = class_colour(pl.class, pal);
        p.circle_filled(pos(pl.screen), LAYOUT_TOUCH_PORT_RADIUS as f32, col);
        p.circle_stroke(
            pos(pl.screen),
            LAYOUT_TOUCH_PORT_RADIUS as f32,
            pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32),
        );
        if full {
            // Redundant encoding: the class letter rides beside the port name.
            let name =
                node.spec.port(pl.pref.index).map(|pt| pt.id.to_uppercase()).unwrap_or_default();
            let label = format!("{name} {}", class_label(pl.class));
            let anchor = match pl.dir {
                sparq_module_api::port::Direction::In => {
                    pos(pl.screen)
                        + egui::vec2(LAYOUT_TOUCH_PORT_RADIUS as f32 + LAYOUT_SPACE_2 as f32, 0.0)
                },
                sparq_module_api::port::Direction::Out => {
                    pos(pl.screen)
                        - egui::vec2(LAYOUT_TOUCH_PORT_RADIUS as f32 + LAYOUT_SPACE_2 as f32, 0.0)
                },
            };
            let align = match pl.dir {
                sparq_module_api::port::Direction::In => Align2::LEFT_CENTER,
                sparq_module_api::port::Direction::Out => Align2::RIGHT_CENTER,
            };
            p.text(anchor, align, label, font_xs(), pal.text_tertiary);
        }
        // Register the port's capture circle (zoom-invariant, ≥ 44 px) as a class-S touch target.
        let cap = LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32;
        audit.push(InteractiveElement {
            id: format!("canvas/port/{}/{}", pl.pref.node, pl.pref.index),
            class: TouchClass::S,
            rect: Rect::new(
                Vec2::new(pl.screen.x - cap, pl.screen.y - cap),
                Vec2::new(pl.screen.x + cap, pl.screen.y + cap),
            ),
            dense_allowed: false,
        });
    }

    // Register the node body as a class-L touch target ONLY when it is actually that big on
    // screen. Below class L (zoomed out) a node is navigated, not individually touched — you zoom
    // in or marquee-select. Gating on measured size keeps the audit honest at every zoom.
    if nl.screen.min_side() >= TouchClass::L.min_px() {
        audit.push(InteractiveElement {
            id: format!("canvas/node/{}", nl.id),
            class: TouchClass::L,
            rect: nl.screen,
            dense_allowed: false,
        });
    }
}

// --------------------------------------------------------------------- marquee + menu

fn draw_marquee(p: &Painter, pal: &Palette, canvas: &CanvasState) {
    let Some(r) = canvas.marquee_screen() else { return };
    let eg = egui_rect(r);
    p.rect_filled(eg, LAYOUT_CORNER_NONE as u8, pal.selected.gamma_multiply(0.12));
    p.rect_stroke(
        eg,
        LAYOUT_CORNER_NONE as u8,
        Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, pal.selected),
        StrokeKind::Middle,
    );
}

fn draw_menu(
    p: &Painter,
    pal: &Palette,
    canvas: &CanvasState,
    view: Rect,
    audit: &mut Vec<InteractiveElement>,
) {
    let (Some(menu), Some(origin)) = (canvas.menu.as_ref(), canvas.menu_origin(view)) else {
        return;
    };
    let w = menu.width();
    let rows = menu.rows.len();
    let h = rows as f32 * LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32;
    let rect = Rect::from_min_size(origin, Vec2::new(w, h));
    let eg = egui_rect(rect);
    p.rect_filled(eg, LAYOUT_CORNER_PANEL as u8, pal.ground_overlay);
    p.rect_stroke(
        eg,
        LAYOUT_CORNER_PANEL as u8,
        pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32),
        StrokeKind::Middle,
    );
    for (i, row) in menu.rows.iter().enumerate() {
        let rr = menu.row_rect(origin, i);
        // Row separators, faint.
        if i > 0 {
            p.line_segment(
                [pos(rr.min), pos(Vec2::new(rr.max.x, rr.min.y))],
                pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
            );
        }
        let col = if row.enabled { pal.text_secondary } else { pal.text_disabled };
        p.text(
            pos(rr.min) + egui::vec2(LAYOUT_SPACE_3 as f32, rr.height() / 2.0),
            Align2::LEFT_CENTER,
            row.label,
            font_s(),
            col,
        );
        // Only enabled rows are touch targets, so only they are audited (the shell's convention:
        // a disabled control is drawn honestly and kept out of the audit).
        if row.enabled {
            audit.push(InteractiveElement {
                id: format!("canvas/menu/{i}"),
                class: TouchClass::S,
                rect: rr,
                dense_allowed: false,
            });
        }
    }
}

// --------------------------------------------------------------------- re-patch drag

/// An in-flight wire-end re-patch (increment 3): the detached end follows the finger, the fixed
/// end stays home, and every other port glows or dims by its verdict against the MOVING end —
/// the same affordance language as a fresh wire, so the two drags feel like one gesture family.
fn draw_repatch(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    layout: &CanvasLayout,
    ctx: &ConnectContext<'_>,
) {
    let Interaction::Repatch { side, orig, cursor_screen, hovered, .. } = &canvas.interaction
    else {
        return;
    };
    let find_port = |pref: sparq_ui::canvas::model::PortRef| -> Option<(Vec2, SignalClass)> {
        layout
            .nodes
            .iter()
            .find_map(|n| n.ports.iter().find(|pl| pl.pref == pref))
            .map(|pl| (pl.screen, pl.class))
    };
    // The detached end is the moving one: re-patching the FROM end means a new SOURCE is being
    // chosen (the destination stays), and vice versa.
    let (fixed, moving_is_source) = match side {
        WireEndSide::From => (orig.to, true),
        WireEndSide::To => (orig.from, false),
    };

    let glow_r = LAYOUT_TOUCH_PORT_RADIUS as f32 + LAYOUT_SPACE_1 as f32;
    let dim_r = LAYOUT_TOUCH_PORT_RADIUS as f32;
    for n in &layout.nodes {
        for pl in &n.ports {
            if pl.pref == orig.from || pl.pref == orig.to {
                continue; // the re-patched wire's own ends take no part in the glow/dim
            }
            let pv = if moving_is_source {
                connect::preview(graph, pl.pref, fixed, ctx)
            } else {
                connect::preview(graph, fixed, pl.pref, ctx)
            };
            let (col, r) = match pv {
                Preview::Compatible | Preview::Conversion | Preview::Adapter(true) => {
                    (class_glow(pl.class, pal), glow_r)
                },
                Preview::Adapter(false) | Preview::Dim => (class_dim(pl.class, pal), dim_r),
            };
            p.circle_filled(pos(pl.screen), r, col);
        }
    }

    if let Some((fixed_screen, _)) = find_port(fixed) {
        let end = hovered.and_then(find_port).map(|(s, _)| s).unwrap_or(*cursor_screen);
        let stroke = Stroke::new(LAYOUT_STROKE_SIGNAL as f32, pal.text_secondary);
        p.line_segment([pos(fixed_screen), pos(end)], stroke);
        // The detached end wears an open warning ring at the cursor: "unconnected right now".
        p.circle_stroke(
            pos(*cursor_screen),
            glow_r,
            Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, pal.warning),
        );
    }
}

// --------------------------------------------------------------------- rename sheet

/// The rename text-entry sheet (increment 5): header names the module, the inset well carries
/// the buffer with its caret, the hint row states the keys and the empty-commits-default rule.
/// Geometry is `RenameState`'s own — the same rects `CanvasState::rename_contains` hit-tests, so
/// the sheet you see is the sheet that captures your taps.
fn draw_rename(
    p: &Painter,
    pal: &Palette,
    canvas: &CanvasState,
    view: Rect,
    audit: &mut Vec<InteractiveElement>,
) {
    let Some(r) = canvas.rename() else { return };
    let (max_w, _) = browser::caps(view);
    let origin = r.sheet_origin(view, max_w);
    let sheet = Rect::from_min_size(origin, Vec2::new(r.width(max_w), r.height()));
    let eg = egui_rect(sheet);
    p.rect_filled(eg, LAYOUT_CORNER_PANEL as u8, pal.ground_overlay);
    p.rect_stroke(
        eg,
        LAYOUT_CORNER_PANEL as u8,
        pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32),
        StrokeKind::Middle,
    );

    // Header: what is being renamed, in words.
    let hr = r.row_rect(origin, 0, max_w);
    p.text(
        pos(hr.min) + egui::vec2(LAYOUT_SPACE_3 as f32, hr.height() / 2.0),
        Align2::LEFT_CENTER,
        r.header_text(),
        font_xs(),
        pal.text_tertiary,
    );

    // The entry well: inset ground (the token language for "a field"), the buffer with its caret.
    let er = r.row_rect(origin, 1, max_w);
    let well = Rect::new(
        Vec2::new(er.min.x + LAYOUT_SPACE_3 as f32, er.min.y + LAYOUT_SPACE_1 as f32),
        Vec2::new(er.max.x - LAYOUT_SPACE_3 as f32, er.max.y - LAYOUT_SPACE_1 as f32),
    );
    p.rect_filled(egui_rect(well), LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    p.text(
        pos(well.min) + egui::vec2(LAYOUT_SPACE_2 as f32, well.height() / 2.0),
        Align2::LEFT_CENTER,
        r.entry.caret_text(),
        font_s(),
        pal.text_primary,
    );
    audit.push(InteractiveElement {
        id: "canvas/rename/entry".to_string(),
        class: TouchClass::M,
        rect: er,
        dense_allowed: false,
    });

    // The hint row: the keys and the empty rule, stated — the sheet explains itself.
    let kr = r.row_rect(origin, 2, max_w);
    p.text(
        pos(kr.min) + egui::vec2(LAYOUT_SPACE_3 as f32, kr.height() / 2.0),
        Align2::LEFT_CENTER,
        sparq_ui::canvas::entry::RENAME_HINT,
        font_xs(),
        pal.text_disabled,
    );
}

// --------------------------------------------------------------------- module browser

/// The module browser sheet: query header, ranked rows (name + id, summary · category dim), the
/// selection worn as a filled band + accent bar. Geometry comes from `browser::caps` and the
/// `BrowserState` rects — the same numbers `BrowserState::hit` uses, so the row you see is the
/// row you touch.
fn draw_browser(
    p: &Painter,
    pal: &Palette,
    canvas: &CanvasState,
    view: Rect,
    audit: &mut Vec<InteractiveElement>,
) {
    let Some(b) = canvas.browser() else { return };
    let (max_w, max_h) = browser::caps(view);
    let origin = b.sheet_origin(view, max_w, max_h);
    let page = browser::BrowserState::page_rows(max_h);
    let sheet = Rect::from_min_size(origin, Vec2::new(b.width(max_w), b.height(page).min(max_h)));
    let eg = egui_rect(sheet);
    p.rect_filled(eg, LAYOUT_CORNER_PANEL as u8, pal.ground_overlay);
    p.rect_stroke(
        eg,
        LAYOUT_CORNER_PANEL as u8,
        pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32),
        StrokeKind::Middle,
    );

    // Header: the query (with a caret — this is the sheet's text-entry target) and the match
    // count, in words.
    let hr = b.header_rect(origin, view, max_w);
    p.rect_filled(egui_rect(hr), LAYOUT_CORNER_NONE as u8, pal.ground_inset);
    let q = b.query();
    p.text(
        pos(hr.min) + egui::vec2(LAYOUT_SPACE_3 as f32, hr.height() / 2.0),
        Align2::LEFT_CENTER,
        if q.is_empty() { "SEARCH\u{2026}_".to_string() } else { format!("SEARCH {q}_") },
        font_s(),
        if q.is_empty() { pal.text_disabled } else { pal.text_primary },
    );
    p.text(
        pos(hr.max) - egui::vec2(LAYOUT_SPACE_3 as f32, hr.height() / 2.0),
        Align2::RIGHT_CENTER,
        format!("{} MATCH", b.visible_len()),
        font_xs(),
        pal.text_tertiary,
    );
    audit.push(InteractiveElement {
        id: "canvas/browser/query".to_string(),
        class: TouchClass::M,
        rect: hr,
        dense_allowed: false,
    });

    if b.visible_len() == 0 {
        // The empty ranking is an ANSWER, said in words — not a blank sheet.
        let r0 = b.row_rect(origin, 0, view, max_w);
        p.text(
            pos(r0.center()),
            Align2::CENTER_CENTER,
            "NO MATCH — CLEAR THE SEARCH",
            font_s(),
            pal.text_disabled,
        );
    }
    let rows = page.min(b.visible_len());
    for (page_i, item) in b.visible().skip(b.scroll()).take(rows).enumerate() {
        let rr = b.row_rect(origin, page_i, view, max_w);
        let rank = b.scroll() + page_i;
        let selected = rank == b.selected();
        if selected {
            p.rect_filled(
                egui_rect(rr),
                LAYOUT_CORNER_NONE as u8,
                pal.selected.gamma_multiply(0.18),
            );
            let bar = Rect::new(rr.min, Vec2::new(rr.min.x + LAYOUT_SPACE_1 as f32, rr.max.y));
            p.rect_filled(egui_rect(bar), LAYOUT_CORNER_NONE as u8, pal.selected);
        }
        if page_i > 0 {
            p.line_segment(
                [pos(rr.min), pos(Vec2::new(rr.max.x, rr.min.y))],
                pal.hairline(pal.hairline_faint, LAYOUT_STROKE_HAIRLINE as f32),
            );
        }
        let pad = LAYOUT_SPACE_3 as f32 + LAYOUT_SPACE_1 as f32;
        p.text(
            pos(rr.min) + egui::vec2(pad, rr.height() * 0.32),
            Align2::LEFT_CENTER,
            item.row_text(),
            font_s(),
            if selected { pal.text_primary } else { pal.text_secondary },
        );
        let sub = if item.category.is_empty() {
            item.summary.clone()
        } else {
            format!("{} · {}", item.summary, item.category)
        };
        p.text(
            pos(rr.min) + egui::vec2(pad, rr.height() * 0.72),
            Align2::LEFT_CENTER,
            sub,
            font_xs(),
            pal.text_tertiary,
        );
        audit.push(InteractiveElement {
            id: format!("canvas/browser/{rank}"),
            class: TouchClass::M,
            rect: rr,
            dense_allowed: false,
        });
    }
}
