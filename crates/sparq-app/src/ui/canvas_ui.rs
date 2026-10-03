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
use sparq_ui::canvas::inset::{self, Well};
use sparq_ui::canvas::interact::{CanvasState, Interaction};
use sparq_ui::canvas::layout::{CanvasLayout, NodeLayout, SignalClass, WireEndSide};
use sparq_ui::canvas::levels::{LevelHistories, LiveMeters};
use sparq_ui::canvas::model::{Graph, Node, NodeId};
use sparq_ui::canvas::response::{self, Curves};
use sparq_ui::canvas::scope::{self, ScopeTraces, ScopeView};
use sparq_ui::geom::{Rect, Vec2};
use sparq_ui::tokens::*;

use crate::ui::adapter::{c32, egui_rect, font_s, font_xs, sp_rect, Palette};

/// The unfinished-flag body tint: the error token at a LIGHT alpha — a highlight over the
/// panel fill, readable as "waiting", never as a filled state.
const FLAG_TINT_ALPHA: f32 = 0.12;

/// Paint the whole canvas for one frame. `view` is the canvas rect (screen px); `audit` collects
/// the touch targets the layout audit measures. `main_info` is the live session's driver truth
/// for the permanent Main Out card's info band (operator ruling 2026-10-01) — `None` at rest.
/// `hists` are the display-side rolling level histories (operator round 4, D13 — the shell
/// pushes them per live frame; empty at rest, so the graph wells sit empty, never faked).
/// `pointer` is the shell's last contact position (mouse hover counts — pointer.rs's rule):
/// the cable node's hover ghost reads it, and nothing else may (a touch-first canvas has no
/// hover-DEPENDENT gesture — the ghost only SHOWS the tap-insert door the hit-test already has).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    layout: &CanvasLayout,
    view: Rect,
    ctx: &ConnectContext<'_>,
    traces: &ScopeTraces,
    meters: &LiveMeters,
    curves: &Curves,
    hists: &LevelHistories,
    main_info: Option<&[String; 2]>,
    pointer: Option<Vec2>,
    audit: &mut Vec<InteractiveElement>,
) {
    draw_grid(p, pal, canvas, view);
    draw_wires(p, pal, graph, canvas, layout, pointer);
    draw_pending_wire(p, pal, graph, canvas, layout, ctx);
    draw_repatch(p, pal, graph, canvas, layout, ctx);
    // The resolved master (explicit or the documented default rule) wears a MASTER badge: the
    // user must be able to see which node the render will carry, in words, before asking.
    let master_id = canvas.resolve_master(graph);
    // The light-red "unfinished" flags (operator ruling 2026-09-30): required inputs with no
    // wire. The canvas reads them from the graph per frame — the same truth the executor's
    // build flags on the audio side — so the highlight is there at rest AND while playing,
    // and clears the frame the wire lands.
    let missing = graph.missing_required_inputs();
    draw_nodes(
        p, pal, graph, canvas, layout, master_id, traces, meters, curves, hists, &missing,
        main_info, audit,
    );
    draw_marquee(p, pal, canvas);
    draw_menu(p, pal, canvas, view, audit);
    draw_browser(p, pal, canvas, view, audit);
    draw_rename(p, pal, canvas, view, audit);
}

// --------------------------------------------------------------------- colour helpers

/// The class accent — shared with the shell chrome (the inspector's port dots, the wire
/// legend) so a signal class has ONE colour everywhere.
pub fn class_colour(c: SignalClass, pal: &Palette) -> Color32 {
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

/// A toggle switch for a BINARY setting (operator ruling 2026-10-01: binary settings are
/// toggle buttons, never sliders — a slider that can only rest in two places is a switch
/// wearing a slider's clothes). Pill body on the token space scale, round knob (the knob/port
/// circle family), ON in the control accent, OFF an empty inset well — position AND fill AND
/// the row's ON/OFF word all carry the state, so colour is never alone.
pub fn draw_toggle(p: &Painter, pal: &Palette, right: Pos2, on: bool, col: Color32, z: f32) {
    let h = LAYOUT_SPACE_3 as f32 * z;
    let w = LAYOUT_SPACE_5 as f32 * z;
    let body = egui::Rect::from_min_max(
        egui::pos2(right.x - w, right.y - h / 2.0),
        egui::pos2(right.x, right.y + h / 2.0),
    );
    let r = h / 2.0;
    let kr = (r - LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5 * z);
    if on {
        p.rect_filled(body, r, col);
        p.circle_filled(egui::pos2(body.max.x - r, right.y), kr, pal.ground_panel);
    } else {
        p.rect_filled(body, r, pal.ground_inset);
        p.rect_stroke(
            body,
            r,
            pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32 * z),
            StrokeKind::Middle,
        );
        p.circle_filled(egui::pos2(body.min.x + r, right.y), kr, pal.text_disabled);
    }
}

/// The button row of a MULTI-CHOICE setting (operator ruling 2026-10-01): one segment button
/// per choice — the active one filled in the row's class colour with its number in the ground
/// tint, the rest empty inset wells with their number dim. Multi-choice settings are buttons,
/// never sliders: a slider over a menu is a menu pretending to be a scale. `band` is the rect
/// the buttons occupy (already zoom-scaled by the caller); the digits ride the xs font at the
/// caller's zoom so the widget shrinks with the card like every other content object.
pub fn draw_choices(
    p: &Painter,
    pal: &Palette,
    band: egui::Rect,
    desc: &sparq_ui::canvas::model::ParamDesc,
    value: f32,
    col: Color32,
    z: f32,
) {
    let n = sparq_ui::canvas::inspector::choice_count(desc);
    let active = (value - desc.min as f32).round().clamp(0.0, n as f32 - 1.0) as usize;
    let gap = (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5);
    let w = (band.width() - gap * (n.saturating_sub(1)) as f32) / n.max(1) as f32;
    let font = scaled_font(font_xs(), z);
    for i in 0..n {
        let x0 = band.min.x + i as f32 * (w + gap);
        let r =
            egui::Rect::from_min_max(egui::pos2(x0, band.min.y), egui::pos2(x0 + w, band.max.y));
        let word = format!("{}", desc.min as i64 + i as i64);
        if i == active {
            p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, col);
            p.text(r.center(), Align2::CENTER_CENTER, word, font.clone(), pal.ground_panel);
        } else {
            p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
            p.rect_stroke(
                r,
                LAYOUT_CORNER_MICRO as u8,
                pal.hairline(pal.hairline_regular, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)),
                StrokeKind::Middle,
            );
            p.text(r.center(), Align2::CENTER_CENTER, word, font.clone(), pal.text_disabled);
        }
    }
}

/// A font at the canvas zoom — module CONTENT keeps its relative scale (operator ruling
/// 2026-10-01): text and objects on a card shrink as the camera zooms out, because they are
/// world objects, not chrome.
fn scaled_font(f: egui::FontId, z: f32) -> egui::FontId {
    egui::FontId { size: (f.size * z).max(1.0), family: f.family }
}

/// Hatch a rect with 45° hairlines — the BYPASS state pattern (increment 5, the mockup's
/// `hatch8`: 8 px horizontal pitch, hairline weight, low alpha over the body fill). A pattern,
/// not a colour: it survives greyscale and it survives LOD, which words do not (look-board §4:
/// "states use pattern first — colour is redundant").
fn hatch_rect(p: &Painter, r: egui::Rect, col: Color32, z: f32) {
    // 8 px horizontal pitch at 45° at zoom 1 — a world pattern, so it scales with the card
    let step = LAYOUT_SPACE_2 as f32 * std::f32::consts::SQRT_2 * z;
    let stroke = Stroke::new((LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5), col);
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
fn dashed_rect(p: &Painter, r: egui::Rect, stroke: Stroke, z: f32) {
    let pts = vec![r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    draw_dashed(p, &pts, LAYOUT_SPACE_2 as f32 * z, LAYOUT_SPACE_1 as f32 * z, stroke);
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
    pointer: Option<Vec2>,
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

        // Control wires (operator ruling 2026-10-01 r3) are MODULATION, not signal: dashed at
        // hairline weight in the control accent, with a small dot on the parameter's sink —
        // the same vocabulary the wire legend reserves for control, one tier quieter.
        if w.is_param {
            let z = canvas.camera.zoom;
            let cst = Stroke::new(
                (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5),
                class_colour(SignalClass::Cv, pal),
            );
            draw_dashed(p, &pts, LAYOUT_SPACE_2 as f32 * z, LAYOUT_SPACE_1 as f32 * z, cst);
            if let Some(&end) = pts.last() {
                p.circle_filled(
                    end,
                    LAYOUT_TOUCH_PORT_RADIUS as f32 * 0.5 * z,
                    class_colour(SignalClass::Cv, pal),
                );
            }
            continue;
        }

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

            // The cable node (operator round 4, D15): the trim handle at the wire's arc
            // midpoint — a filled dot at rest at capture × 0.25 (the shrink the operator
            // named), grown to × 0.5 under the pointer or mid-drag, its amp read as the inner
            // level disc (half-full at identity, full at the 2.0 rail). A CLEAN wire shows the
            // hover GHOST at the same point and the same capture radius the tap-insert door
            // uses — the drawing and the gesture read one geometry, so what you can touch is
            // what you see, and what you cannot see (no pointer near) draws nothing.
            let cap = LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32;
            let near = pointer.is_some_and(|pp| pp.distance(w.trim_point) <= cap);
            if let Some(t) = w.trim {
                let dragging =
                    matches!(&canvas.interaction, Interaction::Trim { wire, .. } if *wire == w.id);
                let r = cap * if dragging || near { 0.5 } else { 0.25 };
                let c = pos(w.trim_point);
                p.circle_filled(c, r, class_colour(w.class, pal));
                p.circle_stroke(
                    c,
                    r,
                    pal.hairline(pal.hairline_strong, (LAYOUT_STROKE_HAIRLINE as f32).max(0.5)),
                );
                let amp_r = r * (t.amp.clamp(0.0, 2.0) / 2.0);
                if amp_r > 0.5 {
                    p.circle_filled(c, amp_r, class_glow(w.class, pal));
                }
            } else if near {
                p.circle_stroke(
                    pos(w.trim_point),
                    cap * 0.25,
                    pal.hairline(pal.hairline_regular, (LAYOUT_STROKE_HAIRLINE as f32).max(0.5)),
                );
            }
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

    // Control-drag affordance (operator ruling 2026-10-01 r3): while a CV output is in
    // flight, the hovered module wears a small blue dot beside every float setting — the
    // controllable inputs, smaller than the main in/out ports, visible exactly when they
    // can be touched. The dot under the cursor glows; the wire snaps to it.
    let z = canvas.camera.zoom;
    let src_is_cv =
        graph.port(pw.from).is_some_and(|pt| pt.port_type == sparq_module_api::port::PortType::Cv);
    let mut sink_end: Option<Vec2> = None;
    if src_is_cv {
        let hov = layout.nodes.iter().find(|n| n.screen.contains(pw.cursor_screen));
        let hov_id = hov.map(|n| n.id);
        for n in &layout.nodes {
            if Some(n.id) != hov_id {
                continue;
            }
            for pr in &n.param_rows {
                let Some(sink) = pr.sink else { continue };
                let r = LAYOUT_TOUCH_PORT_RADIUS as f32 * 0.5 * z;
                let captured = pw.hovered_param == Some((n.id, pr.index));
                if captured {
                    sink_end = Some(sink);
                    p.circle_filled(
                        pos(sink),
                        r + LAYOUT_SPACE_1 as f32 * z,
                        class_glow(SignalClass::Cv, pal),
                    );
                }
                p.circle_filled(pos(sink), r, class_colour(SignalClass::Cv, pal));
                p.circle_stroke(
                    pos(sink),
                    r,
                    pal.hairline(pal.hairline_strong, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)),
                );
            }
        }
    }

    // The in-flight wire: source port → hovered port (magnet), hovered sink, or the raw cursor.
    if let Some((src, _)) = find_port(pw.from) {
        let end =
            pw.hovered.and_then(find_port).map(|(s, _)| s).or(sink_end).unwrap_or(pw.cursor_screen);
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
    traces: &ScopeTraces,
    meters: &LiveMeters,
    curves: &Curves,
    hists: &LevelHistories,
    missing: &[(NodeId, usize)],
    main_info: Option<&[String; 2]>,
    audit: &mut Vec<InteractiveElement>,
) {
    let z = canvas.camera.zoom;
    for nl in &layout.nodes {
        let Some(node) = graph.node(nl.id) else { continue };
        let selected = canvas.selection.nodes.contains(&nl.id);
        let is_master = master_id == Some(nl.id);
        let flagged = missing.iter().any(|(id, _)| *id == nl.id);
        match layout.lod {
            Lod::Dot => draw_node_dot(p, pal, node, nl, selected, is_master, flagged, z),
            Lod::Simplified => draw_node_box(
                p, pal, graph, canvas, node, nl, selected, is_master, false, traces, meters,
                curves, hists, flagged, main_info, z, audit,
            ),
            Lod::Full => draw_node_box(
                p, pal, graph, canvas, node, nl, selected, is_master, true, traces, meters, curves,
                hists, flagged, main_info, z, audit,
            ),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn dominant_class(node: &Node) -> SignalClass {
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

/// The `dsp/scope` display (WO-013 increment 6; ENLARGED with measurements and a graticule by
/// operator ruling 2026-10-01): a `ground.inset` well on the scope's own taller band token, a
/// 10 × 8 division graticule in hairline-faint (the classic oscilloscope grid, computed by
/// `sparq_ui::canvas::scope::graticule` — the painter invents no geometry), the trace in the
/// WIRES' glow vocabulary (class colour lerped toward the class glow by the window's peak, plus
/// the under-glow pass at the wires' alpha rule), and — at Full LOD — the MEASUREMENT line:
/// ms/div, Vpp, RMS and peak of the SAME trigger-selected window the trace draws, so the
/// numbers and the picture can never disagree. The samples come from the analysis ring through
/// the session's traces — never faked, never stale: an unbound or at-rest scope shows the flat
/// rest line and says NO SIGNAL in words. The display steals no gestures: it is pixels under
/// the node body's existing touch target.
#[allow(clippy::too_many_arguments)] // the scope reads the graph, the traces, the LOD and the zoom
fn draw_scope_display(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    node: &Node,
    nl: &NodeLayout,
    full: bool,
    traces: &ScopeTraces,
    z: f32,
) {
    /// The graticule's division counts (the classic 10 × 8 oscilloscope screen).
    const DIVS: (usize, usize) = (10, 8);
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_2 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return; // a collapsed viewport is not a scope's problem to solve
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    let grid = pal.hairline(pal.hairline_faint, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5));
    // The graticule: minor divisions hairline-faint, the centre cross one tier up — the
    // structure reads without competing with the trace.
    let (vs, hs) = scope::graticule(sp_rect(disp), DIVS.0, DIVS.1);
    let mid_v = DIVS.0 / 2;
    let mid_h = DIVS.1 / 2;
    for (i, (a, b)) in vs.iter().enumerate() {
        let stroke = if i == mid_v {
            pal.hairline(pal.hairline_regular, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5))
        } else {
            grid
        };
        p.line_segment([pos(*a), pos(*b)], stroke);
    }
    for (j, (a, b)) in hs.iter().enumerate() {
        let stroke = if j == mid_h {
            pal.hairline(pal.hairline_regular, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5))
        } else {
            grid
        };
        p.line_segment([pos(*a), pos(*b)], stroke);
    }
    if !full {
        return; // Simplified: the well says "scope"; trace and words are Full-LOD readings (D8)
    }

    // The view params, read from the node per frame and clamped by the model (D10). Index
    // order is the manifest's own — timebase, mode, trigger, gain — pinned by a session test.
    let prm = node.effective_params();
    let view = ScopeView::from_params(
        prm.first().copied().unwrap_or(20.0),
        prm.get(1).copied().unwrap_or(0.0),
        prm.get(2).copied().unwrap_or(0.0),
        prm.get(3).copied().unwrap_or(1.0),
    );
    let r = sp_rect(disp);
    let tr = traces.get(nl.id);
    // The trace wears the colour of the SIGNAL IT DISPLAYS (operator ruling 2026-10-01 r3):
    // the scope accepts any source class, so an LFO on the scope draws in control blue, a
    // kick in audio amber — the wire's own class vocabulary, read off the binding's source
    // port. No binding: the audio convention (the scope's historical voice).
    let src_port = graph
        .wires()
        .iter()
        .find(|w| w.to.node == nl.id && w.to.index == 0)
        .or_else(|| graph.wires().iter().find(|w| w.to.node == nl.id))
        .map(|w| w.from);
    let src_class = src_port
        .and_then(|pref| graph.port(pref))
        .map(sparq_ui::canvas::layout::signal_class)
        .unwrap_or(SignalClass::Audio);
    let no_wave = matches!(src_class, SignalClass::Event | SignalClass::Data);
    let pts: Vec<Pos2> = match (view.mode_xy, tr) {
        (true, Some(t)) => scope::xy_polyline(&t.x, &t.y, &view, r),
        (false, Some(t)) => scope::trace_polyline(&t.x, &view, r),
        (_, None) => Vec::new(),
    }
    .into_iter()
    .map(pos)
    .collect();
    let level = tr
        .map(|t| if view.mode_xy { t.x.peak().max(t.y.peak()) } else { t.x.peak() })
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    if pts.len() >= 2 {
        let base = class_colour(src_class, pal);
        let colour =
            if level > 0.0 { lerp_colour(base, class_glow(src_class, pal), level) } else { base };
        let width = class_width(src_class) * z;
        // The under-glow pass, the wires' own expression — glow is data: it encodes level.
        if level > 0.02 {
            let glow = with_alpha(class_glow(src_class, pal), 0.15 + 0.45 * level);
            p.line(
                pts.clone(),
                Stroke::new(width + LAYOUT_SPACE_1 as f32 * z * (0.5 + level), glow),
            );
        }
        p.line(pts, Stroke::new(width, colour));
    } else {
        // The rest line: unbound, at rest, or a zero signal — the same claim, the same pixels.
        let [a, b] = scope::rest_line(r);
        p.line_segment([pos(a), pos(b)], grid);
    }

    // The measurement line: the numbers OF THE DRAWN WINDOW — ms/div from the timebase and
    // the graticule's own column count, Vpp/RMS/PK from `scope::measure` on the same
    // trigger-selected samples. Every number with its unit or word; NO SIGNAL when the window
    // is empty, because confident zeros about a signal that does not exist would be a lie.
    let words = match tr.and_then(|t| {
        if view.mode_xy {
            scope::measure(&t.y, &view).or_else(|| scope::measure(&t.x, &view))
        } else {
            scope::measure(&t.x, &view)
        }
    }) {
        // An event or data source is a legal binding (r3) but publishes no waveform: the
        // scope says exactly that, in words, rather than drawing a confident flat line about
        // a signal shape it cannot have.
        Some(m) if !no_wave => format!(
            "{mode}{:.1} ms/div  Vpp {:.2}  RMS {:.2}  PK {:.2}",
            scope::ms_per_div(&view, DIVS.0),
            m.vpp,
            m.rms,
            m.peak,
            mode = if view.mode_xy { "X/Y  " } else { "" },
        ),
        None if no_wave => format!(
            "{mode}{:.1} ms/div  NO WAVEFORM - {:?} SOURCE",
            scope::ms_per_div(&view, DIVS.0),
            src_class,
            mode = if view.mode_xy { "X/Y  " } else { "" },
        ),
        _ => format!(
            "{mode}{:.1} ms/div  NO SIGNAL",
            scope::ms_per_div(&view, DIVS.0),
            mode = if view.mode_xy { "X/Y  " } else { "" },
        ),
    };
    p.text(
        disp.left_top() + egui::vec2(LAYOUT_SPACE_1 as f32 * z, LAYOUT_SPACE_1 as f32 * 0.5 * z),
        Align2::LEFT_TOP,
        words,
        scaled_font(font_xs(), z),
        pal.text_tertiary,
    );
}

/// A node at Dot LOD, with its states as SHAPES (increment 5 — the Dot contract is "a
/// colour-coded dot per node", and at 8 px words are not an option): bypassed = a HOLLOW ring
/// (the signal goes around it), muted = a DIMMED fill, locked = a concentric hairline ring,
/// master = a `selected`-colour hairline ring (selection's own ring is emphasis-WEIGHT at the
/// same radius, so the two never rely on colour alone to tell them apart).
#[allow(clippy::too_many_arguments)] // the LOD dot carries its states AND the zoom factor
fn draw_node_dot(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    selected: bool,
    master: bool,
    flagged: bool,
    z: f32,
) {
    // Dot LOD: the dot IS the module, so it scales with the camera like every other content
    // object (operator ruling 2026-10-01); the state rings scale with it.
    let c = pos(nl.screen.center());
    let r = LAYOUT_SPACE_2 as f32 * z;
    let col = class_colour(dominant_class(node), pal);
    let dimmed = node.flags.bypassed || node.flags.muted;
    let fill_col = if dimmed { col.gamma_multiply(0.4) } else { col };
    let hair = (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5);
    if node.flags.bypassed {
        p.circle_stroke(c, r, Stroke::new(LAYOUT_STROKE_SIGNAL as f32 * z, fill_col));
    } else {
        p.circle_filled(c, r, fill_col);
    }
    if node.flags.locked {
        p.circle_stroke(
            c,
            r + LAYOUT_SPACE_1 as f32 * 0.5 * z,
            pal.hairline(pal.hairline_strong, hair),
        );
    }
    if master {
        p.circle_stroke(c, r + LAYOUT_SPACE_1 as f32 * z, Stroke::new(hair, pal.selected));
    }
    if flagged {
        // The Dot-LOD half of the unfinished flag: an error-colour ring (the bypass ring's
        // radius, the flag's own colour — colour plus the box LOD's tint and word, never
        // colour alone).
        p.circle_stroke(c, r + LAYOUT_SPACE_1 as f32 * 0.5 * z, Stroke::new(hair, pal.error));
    }
    if selected {
        p.circle_stroke(
            c,
            r + LAYOUT_SPACE_1 as f32 * z,
            Stroke::new(LAYOUT_STROKE_EMPHASIS as f32 * z, pal.selected),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_node_box(
    p: &Painter,
    pal: &Palette,
    graph: &Graph,
    canvas: &CanvasState,
    node: &Node,
    nl: &NodeLayout,
    selected: bool,
    master: bool,
    full: bool,
    traces: &ScopeTraces,
    meters: &LiveMeters,
    curves: &Curves,
    hists: &LevelHistories,
    flagged: bool,
    main_info: Option<&[String; 2]>,
    z: f32,
    audit: &mut Vec<InteractiveElement>,
) {
    // ONE zoom factor for every content object on the card (operator ruling 2026-10-01):
    // text, knobs, ports, switches, wells and strokes are WORLD objects and shrink with the
    // camera; touch targets and the selection frame stay screen-sized, because a finger and
    // a "which node am I editing" cue do not shrink.
    let hair = |a: f32| pal.hairline(a, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5));
    // The junction bus wears no WORDS (operator round 4, D6): the dots wear their role, and a
    // name/category/label stack on a 60 px strip would be chrome around nothing.
    let is_mult = node.spec.module_id == sparq_ui::canvas::MULT_ID;
    let body = egui_rect(nl.screen);
    let corner = LAYOUT_CANVAS_NODE_RADIUS as u8;
    let dimmed = node.flags.bypassed || node.flags.muted;
    let fill = if dimmed { pal.ground_panel.gamma_multiply(0.6) } else { pal.ground_panel };
    p.rect_filled(body, corner, fill);

    // The light-red "unfinished" highlight (operator ruling 2026-09-30): a required input has
    // no wire. A LIGHT tint of the error token over the body fill — a highlight, not a state
    // pattern: the node is not bypassed or muted, it is WAITING for a wire, and playback still
    // runs with its module silenced. The border and (at Full LOD) the `NO IN` word carry the
    // same fact redundantly, per the look-board's pattern-first rule.
    if flagged {
        p.rect_filled(body, corner, with_alpha(pal.error, FLAG_TINT_ALPHA));
    }

    // The card's left stripe (increment 6, the PN convergence): the node's DOMINANT SIGNAL
    // CLASS — sparq's §4 rule (colour says what the module carries) read in the reference's
    // category-stripe position. TWO px thin at zoom 1 (operator ruling 2026-10-01), and a
    // world object: it scales with the card.
    let stripe = egui::Rect::from_min_size(
        body.left_top(),
        egui::vec2((LAYOUT_STROKE_SIGNAL as f32 * z).max(0.5), body.height()),
    );
    p.rect_filled(stripe, LAYOUT_CORNER_NONE as u8, class_colour(dominant_class(node), pal));

    // State patterns (increment 5, look-board §4 "pattern first — colour is redundant"): the
    // shapes survive the LOD reduction AND the monochrome test, which words and tints do not.
    // BYPASSED hatches the body — the mockup's own encoding (`design-mode.svg`'s `hatch8`).
    // The header band draws over the hatch, so the pattern reads as "the body is bypassed".
    if node.flags.bypassed {
        hatch_rect(p, body, pal.hairline_colour.gamma_multiply(0.35), z);
    }

    // Border: selected wears the selection accent at emphasis width; else the regular hairline —
    // and a MUTED node's border is dashed (the state pattern; the selection accent outranks it,
    // because "which node am I editing" beats "which node is silent" at the border's one job).
    let border = if selected {
        Stroke::new((LAYOUT_STROKE_EMPHASIS as f32 * z).max(0.5), pal.selected)
    } else if flagged {
        Stroke::new((LAYOUT_STROKE_SIGNAL as f32 * z).max(0.5), pal.error)
    } else {
        hair(pal.hairline_regular)
    };
    if node.flags.muted && !selected {
        dashed_rect(p, body, border, z);
    } else {
        p.rect_stroke(body, corner, border, StrokeKind::Middle);
    }
    // LOCKED: a second, inset border — the "double" pattern the stroke language already uses for
    // a doubled signal, here meaning "this node is pinned in place".
    if node.flags.locked {
        p.rect_stroke(
            body.shrink(LAYOUT_SPACE_1 as f32 * z),
            corner,
            hair(pal.hairline_faint),
            StrokeKind::Middle,
        );
    }

    // Header band.
    let header = egui_rect(nl.header_screen);
    p.rect_filled(header, corner, pal.ground_panel_alt);
    p.line_segment([header.left_bottom(), header.right_bottom()], hair(pal.hairline_faint));

    if full && !is_mult {
        p.text(
            header.left_center() + egui::vec2(LAYOUT_SPACE_2 as f32 * z, 0.0),
            Align2::LEFT_CENTER,
            node.title(),
            scaled_font(font_s(), z),
            if dimmed { pal.text_disabled } else { pal.text_primary },
        );
    } else if master {
        // MASTER at Simplified: the word is gone with every other text, but the role must still
        // be readable — a filled chip in the badge's own accent sits where the word would be.
        // The pattern set (hatch/dash/double) stays distinguishable from it: those are outlines
        // on the body, this is a solid on the header.
        let chip = LAYOUT_SPACE_2 as f32 * z;
        let c = header.right_center() - egui::vec2(LAYOUT_SPACE_2 as f32 * z + chip / 2.0, 0.0);
        p.rect_filled(
            egui::Rect::from_center_size(c, egui::vec2(chip, chip)),
            LAYOUT_CORNER_MICRO as u8,
            pal.selected,
        );
    }

    // The header's right side (increment 6, PN's anatomy): the category word and the node id,
    // dim — what the card is and which instance of it, before any state word competes.
    let mut cat_w = 0.0f32;
    if full && !is_mult {
        let top = node.spec.module_id.split('/').nth(1).unwrap_or("");
        let cat = format!("{top} · {}", nl.id);
        cat_w = cat.len() as f32 * LAYOUT_SPACE_2 as f32 * z;
        p.text(
            header.right_center() - egui::vec2(LAYOUT_SPACE_2 as f32 * z, 0.0),
            Align2::RIGHT_CENTER,
            cat,
            scaled_font(font_xs(), z),
            pal.text_disabled,
        );
    }

    // Flag badges, in WORDS — at Full LOD only (increment 5: the Simplified contract is "node
    // box, coloured ports, NO TEXT", and the patterns above are what carries the states down
    // there). At Full the word rides ON TOP of the pattern: redundant encoding, never either
    // alone — the mockup shows exactly this pairing for bypass (hatch + "bypassed"). The badges
    // start LEFT of the category word: a state outranks a label for the right edge.
    if full {
        let mut badge_x =
            header.right_center().x - LAYOUT_SPACE_2 as f32 * z - cat_w - LAYOUT_SPACE_2 as f32 * z;
        for (on, label, col) in [
            (master, "MASTER", pal.selected),
            (node.flags.locked, "LOCK", pal.text_tertiary),
            (node.flags.muted, "MUTE", pal.text_disabled),
            (node.flags.bypassed, "BYPASS", pal.warning),
            (flagged, "NO IN", pal.error),
        ] {
            if !on {
                continue;
            }
            let g = scaled_font(font_xs(), z);
            let w = label.len() as f32 * LAYOUT_SPACE_2 as f32 * z;
            badge_x -= w;
            p.text(Pos2::new(badge_x, header.center().y), Align2::LEFT_CENTER, label, g, col);
            badge_x -= LAYOUT_SPACE_2 as f32 * z;
        }
    }

    // Inline parameter rows (increment 6, the PN card anatomy): label left in xs tertiary
    // uppercase, value right in the node's dominant class colour, and the control itself in
    // the row's lower band — slider, toggle or button row by the param's SHAPE (operator
    // ruling 2026-10-01: binary = toggle, multi-choice = buttons, the rest = slider) — and
    // the SAME op path as the inspector (Hit::Param → Interaction::Param), so one door serves
    // both surfaces and the live ring cannot hear a difference. Full LOD only: the Simplified
    // contract is ports, wells and patterns, no text.
    if full {
        let col = class_colour(dominant_class(node), pal);
        for pr in &nl.param_rows {
            let Some(desc) = node.spec.params.get(pr.index) else { continue };
            let value = node.param_value(pr.index).unwrap_or(desc.default as f32);
            let row = egui_rect(pr.row);
            let label_y = row.min.y + row.height() * 0.30;
            // The value owns the right edge; the label takes what is left and truncates with
            // an ellipsis rather than colliding (a number without its unit is a bug, so the
            // VALUE is never the thing that gets cut — look-board §5).
            let value_s = sparq_ui::canvas::inspector::value_text(desc, value);
            let char_w = LAYOUT_SPACE_2 as f32 * z;
            let avail = row.width() - char_w * 3.0 - value_s.len() as f32 * char_w;
            let max_chars = (avail / char_w).floor().max(4.0) as usize;
            let name = desc.name.to_uppercase();
            let label_s = if name.len() > max_chars {
                format!("{}…", &name[..max_chars.saturating_sub(1)])
            } else {
                name
            };
            p.text(
                egui::pos2(row.min.x + LAYOUT_SPACE_2 as f32 * z, label_y),
                Align2::LEFT_CENTER,
                label_s,
                scaled_font(font_xs(), z),
                pal.text_tertiary,
            );
            p.text(
                egui::pos2(row.max.x - LAYOUT_SPACE_2 as f32 * z, label_y),
                Align2::RIGHT_CENTER,
                value_s,
                scaled_font(font_xs(), z),
                col,
            );
            let tr = egui_rect(pr.track);
            let mid_y = tr.center().y;
            if nl.step_param == Some(pr.index) && !nl.step_cells.is_empty() {
                // The sequencer's program: 16 step buttons the clock walks (operator ruling
                // 2026-10-01 r3) — set steps filled in the class accent, unset steps empty
                // wells; the mask number to the right stays the redundant word.
                let mask = value as i64;
                for (k, cell) in nl.step_cells.iter().enumerate() {
                    let r = egui_rect(*cell);
                    if mask & (1 << k.min(15)) != 0 {
                        p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, col);
                    } else {
                        p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
                        p.rect_stroke(
                            r,
                            LAYOUT_CORNER_MICRO as u8,
                            hair(pal.hairline_regular),
                            StrokeKind::Middle,
                        );
                    }
                }
                // The walking light (operator round 4, D5): the cursor cell — read from the
                // module's OWN `step` publication (`floor(pub × steps)`, never a display-side
                // counter) — wears the EVENT accent ring. The pattern bits stay the
                // authoritative fill; the cursor is a reading over them, and at rest (no
                // session, no publication) no cell is lit: nothing is faked.
                let steps_val = node
                    .spec
                    .params
                    .iter()
                    .position(|d| d.id == "steps")
                    .and_then(|i| node.param_value(i))
                    .unwrap_or(8.0)
                    .round()
                    .clamp(3.0, 16.0) as usize;
                if let Some(ci) = inset::nth_cv_out(&node.spec, 0)
                    .and_then(|pt| canvas.levels.port(nl.id, pt))
                    .map(|pub01| ((pub01 * steps_val as f32).floor() as usize).min(15))
                {
                    if let Some(cell) = nl.step_cells.get(ci) {
                        p.rect_stroke(
                            egui_rect(*cell),
                            LAYOUT_CORNER_MICRO as u8,
                            Stroke::new(
                                (LAYOUT_STROKE_EMPHASIS as f32 * z).max(1.0),
                                class_colour(SignalClass::Event, pal),
                            ),
                            StrokeKind::Middle,
                        );
                    }
                }
                let _ = (tr, mid_y);
            } else if sparq_ui::canvas::inspector::is_choice(desc) {
                draw_choices(p, pal, tr, desc, value, col, z);
            } else if sparq_ui::canvas::inspector::is_binary(desc) {
                // Binary setting → toggle switch, right-aligned on the track; the ON/OFF word
                // above is the redundant encoding.
                draw_toggle(p, pal, egui::pos2(tr.max.x, mid_y), value >= 0.5, col, z);
            } else {
                p.line_segment(
                    [egui::pos2(tr.min.x, mid_y), egui::pos2(tr.max.x, mid_y)],
                    hair(pal.hairline_regular),
                );
                let kx = sparq_ui::canvas::inspector::knob_x(desc, pr.track, value);
                p.line_segment(
                    [egui::pos2(tr.min.x, mid_y), egui::pos2(kx, mid_y)],
                    Stroke::new((LAYOUT_STROKE_SIGNAL as f32 * z).max(0.5), col),
                );
                p.circle_filled(egui::pos2(kx, mid_y), LAYOUT_SPACE_1 as f32 * z, col);
            }
        }
        if let Some((more, r)) = nl.param_overflow {
            p.text(
                egui_rect(r).center(),
                Align2::CENTER_CENTER,
                format!("+{more} MORE - INSPECTOR"),
                scaled_font(font_xs(), z),
                pal.text_disabled,
            );
        }
    }

    // The node inset displays (WO-012 increment 5, D1): the WELL REGISTRY dispatch — one
    // lookup (`sparq_ui::canvas::inset::well_for`) says which well this node wears, and the
    // painter matches on the Well and calls the matching draw fn. No module-id literal reaches
    // this file (the `OUT_MAIN_ID`/`SCOPE_ID` if-chain this replaces was the pattern's first
    // draft). Wells draw at Full AND Simplified (a meter or a shape is a reading, not text —
    // inc 4 D3's rule); at Dot the node is a dot and wears no well. A well steals no gesture:
    // it is pixels under the node body's existing class-L touch target (the scope's rule).
    // LIVE where the rings carry it (meters), the param-derived declared shape otherwise —
    // at rest, never faked.
    match inset::well_for(&node.spec) {
        Some(Well::Scope) => draw_scope_display(p, pal, graph, node, nl, full, traces, z),
        Some(Well::Meters) => draw_meters_well(p, pal, node, nl, meters, z),
        Some(Well::Envelope) => draw_envelope_well(p, pal, node, nl, z),
        Some(Well::Sparkline) => draw_sparkline_well(p, pal, node, nl, z),
        Some(Well::Curve) => draw_curve_well(p, pal, node, nl, curves, z),
        // The round-4 wells (operator rulings 2026-10-02): each draws a PUBLISHED value (the
        // level set the session drains) or a DECLARED param-derived shape — the compute-then-
        // draw rule the whole file holds: nothing here invents motion, and at rest every one
        // of them sits at its declared at-rest shape, never a frozen lie.
        Some(Well::ClockWheel) => draw_clock_wheel(p, pal, node, nl, canvas, full, z),
        Some(Well::Graph) => draw_level_graph(p, pal, node, nl, canvas, hists, z),
        Some(Well::Keyboard) => draw_keyboard_well(p, pal, node, nl, canvas, z),
        Some(Well::Steps) => draw_step_bars(p, pal, node, nl, canvas, z),
        None => {}, // no well: the body is the box, honest
    }

    // The Main Out driver-info band (operator ruling 2026-10-01): the permanent node says
    // WHICH driver is under it and the negotiated truth (rate · channels · block · format) in
    // words with units, while a session runs — and says the at-rest fact when none does. The
    // layout reserves the band on `out/main` alone; the lines come from the session itself,
    // never invented here. The words are CLIPPED to the band (operator report: a long device
    // name broke the frame) — an ellipsis, never an overflow.
    if let Some(band) = nl.info_band {
        let r = egui_rect(band);
        if r.width() > 0.0 && r.height() > 0.0 {
            p.rect_filled(r, LAYOUT_CORNER_NONE as u8, pal.ground_inset);
            p.line_segment([r.left_top(), r.right_top()], hair(pal.hairline_faint));
            // The clip latch's WORD (operator round 4, D2): while any of this node's meters
            // carries the latch, CLIP sits at the band's right edge in the error token — the
            // redundant encoding of the red cap the meter bars wear (a latch a colour-blind
            // operator can still read, and a word the log can quote).
            let clipped = meters.iter().any(|((id, _), m)| *id == nl.id && (m.clip_l || m.clip_r));
            if full {
                let pad = LAYOUT_SPACE_2 as f32 * z;
                let char_w = LAYOUT_SPACE_2 as f32 * z;
                // The CLIP word reserves its own width: the driver lines clip to what is left.
                let clip_w = if clipped { 5.0 * char_w } else { 0.0 };
                let max_chars =
                    ((r.width() - pad * 2.0 - clip_w) / char_w).floor().max(4.0) as usize;
                // char-boundary-safe: the driver lines carry `·` separators (multi-byte), and
                // a byte-slice ellipsis there is a panic, not a truncation
                let clip = |line: &str| -> String {
                    if line.chars().count() <= max_chars {
                        line.to_string()
                    } else {
                        let cut: String = line.chars().take(max_chars.saturating_sub(1)).collect();
                        format!("{cut}…")
                    }
                };
                let line_h = r.height() / 2.0;
                match main_info {
                    Some(lines) => {
                        for (i, line) in lines.iter().enumerate() {
                            p.text(
                                egui::pos2(r.min.x + pad, r.min.y + line_h * (i as f32 + 0.5)),
                                Align2::LEFT_CENTER,
                                clip(line),
                                scaled_font(font_xs(), z),
                                if i == 0 { pal.text_secondary } else { pal.text_tertiary },
                            );
                        }
                    },
                    None => {
                        p.text(
                            egui::pos2(r.min.x + pad, r.center().y),
                            Align2::LEFT_CENTER,
                            clip("NO SESSION - TRANSPORT'S PLAY OPENS THE DEVICE"),
                            scaled_font(font_xs(), z),
                            pal.text_disabled,
                        );
                    },
                }
                if clipped {
                    p.text(
                        r.right_center() - egui::vec2(pad, 0.0),
                        Align2::RIGHT_CENTER,
                        "CLIP",
                        scaled_font(font_xs(), z),
                        pal.error,
                    );
                }
            }
        }
    }

    // Ports: the circles are world objects and scale with the card (operator ruling
    // 2026-10-01); their CAPTURE stays screen-sized (the audit's zoom-invariant 24 px ring),
    // because a finger does not shrink with the zoom.
    // The junction bus (operator ruling 2026-10-01 r3): a mult dot wears its ROLE, not a
    // static direction — uncommitted a hollow neutral ring, carrying an input a ring in the
    // bus's class, carrying an output a filled dot in it. The first connection's class is
    // the bus's colour from then on, so the dot says what it carries without a word.
    for pl in &nl.ports {
        let (col, filled) = if is_mult {
            let role = graph.mult_port_role(nl.id, pl.pref.index);
            let cls = graph
                .mult_defining_port(nl.id, pl.pref.index)
                .and_then(|pref| graph.port(pref))
                .map(sparq_ui::canvas::layout::signal_class)
                .unwrap_or(SignalClass::Neutral);
            (class_colour(cls, pal), role == Some(sparq_module_api::port::Direction::Out))
        } else {
            (class_colour(pl.class, pal), true)
        };
        let pr = LAYOUT_TOUCH_PORT_RADIUS as f32 * z;
        if filled {
            p.circle_filled(pos(pl.screen), pr, col);
            p.circle_stroke(pos(pl.screen), pr, hair(pal.hairline_strong));
        } else {
            p.circle_stroke(
                pos(pl.screen),
                pr,
                Stroke::new((LAYOUT_STROKE_SIGNAL as f32 * z).max(0.5), col),
            );
        }
        if full && !is_mult {
            // Redundant encoding: the class letter rides beside the port name. (A mult dot
            // wears no name and no letter — D6: the dot's ROLE is its whole vocabulary.)
            let name =
                node.spec.port(pl.pref.index).map(|pt| pt.id.to_uppercase()).unwrap_or_default();
            let label = format!("{name} {}", class_label(pl.class));
            let anchor = match pl.dir {
                sparq_module_api::port::Direction::In => {
                    pos(pl.screen) + egui::vec2(pr + LAYOUT_SPACE_2 as f32 * z, 0.0)
                },
                sparq_module_api::port::Direction::Out => {
                    pos(pl.screen) - egui::vec2(pr + LAYOUT_SPACE_2 as f32 * z, 0.0)
                },
            };
            let align = match pl.dir {
                sparq_module_api::port::Direction::In => Align2::LEFT_CENTER,
                sparq_module_api::port::Direction::Out => Align2::RIGHT_CENTER,
            };
            p.text(anchor, align, label, scaled_font(font_xs(), z), pal.text_tertiary);
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

    // Inline sliders are controls: registered at the dense floor (non-destructive edits,
    // badged) while their on-screen row clears it; below that zoom the row is a reading and
    // the inspector is the door — the same honesty as the node-body gating below.
    if full {
        for pr in &nl.param_rows {
            if pr.row.height() >= LAYOUT_TOUCH_MIN_TARGET_DENSE as f32 {
                audit.push(InteractiveElement {
                    id: format!("canvas/param/{}/{}", nl.id, pr.index),
                    class: TouchClass::S,
                    rect: pr.row,
                    dense_allowed: true,
                });
            }
        }
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

/// The wire-encoding legend (WO-012 increment 3, the mockup's floating box): five rows, each
/// the EXACT stroke the class draws on a wire (same `class_encoding` table, same class colour —
/// the legend and the wires cannot drift) beside the class word. Chrome, not a control: nothing
/// to touch, nothing registered. Drawn by the canvas painter because the encoding table lives
/// here.
pub fn draw_wire_legend(p: &Painter, pal: &Palette, r: Rect) {
    let eg = egui_rect(r);
    p.rect_filled(eg, LAYOUT_CORNER_MICRO as u8, pal.ground_panel);
    p.rect_stroke(
        eg,
        LAYOUT_CORNER_MICRO as u8,
        pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32),
        egui::StrokeKind::Middle,
    );
    let row_h = LAYOUT_SPACE_4 as f32;
    let sample_w = LAYOUT_SPACE_7 as f32;
    let x0 = eg.min.x + LAYOUT_SPACE_3 as f32;
    let mut y = eg.min.y + LAYOUT_SPACE_2 as f32 + row_h / 2.0;
    for (class, word) in [
        (SignalClass::Audio, "AUDIO"),
        (SignalClass::Cv, "CONTROL"),
        (SignalClass::Event, "EVENT"),
        (SignalClass::Data, "DATA"),
        (SignalClass::Spatial, "SPATIAL"),
    ] {
        let col = class_colour(class, pal);
        let a = egui::pos2(x0, y);
        let b = egui::pos2(x0 + sample_w, y);
        let enc = class_encoding(class);
        let (pattern, double) = parse_encoding(enc);
        let st = Stroke::new(class_width(class), col);
        if let Some((on, off)) = pattern {
            draw_dashed(p, &[a, b], on, off, st);
        } else if double {
            let off = LAYOUT_STROKE_HAIRLINE as f32 * 2.0;
            p.line_segment([a - egui::vec2(0.0, off), b - egui::vec2(0.0, off)], st);
            p.line_segment([a + egui::vec2(0.0, off), b + egui::vec2(0.0, off)], st);
        } else {
            p.line_segment([a, b], st);
        }
        p.text(
            egui::pos2(x0 + sample_w + LAYOUT_SPACE_3 as f32, y),
            Align2::LEFT_CENTER,
            word,
            font_xs(),
            pal.text_secondary,
        );
        y += row_h;
    }
}

/// The Meters well (WO-012 increment 4 for `out/main`, increment 5 for EVERY audio-output
/// node and the two registered analysers): meter bars in the shared bar vocabulary — a
/// hairline well, a DATA-class fill (a meter is data *about* the signal — §4 under that
/// reading) and an AUDIO-class peak-hold block decaying on AUDIO time in the session's drain.
///
/// Two sources, both READ, never invented:
///
/// * a node with an audio output takes its first audio port's [`LiveMeters`] entry — the ring
///   already publishes per-port stereo peaks for all of them (`publish_meters` walks the whole
///   order); increment 4 painted only `out/main`, the registry now paints every one;
/// * `ana/rms` / `ana/tap` (registry: Meters, no audio out) take their cv port's published
///   value ([`NodeLevels::port`] — the same reading the wire levels light from) as a SINGLE
///   bar. The cv path carries no block stamp, so no hold block is drawn — an honest gap, not
///   a frozen one.
///
/// At rest the shell hands over empty maps and the wells sit empty, because a frozen bar from
/// a dead stream is a lie with a scale on it.
fn draw_meters_well(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    meters: &LiveMeters,
    z: f32,
) {
    // `out/main` alone wears bars (operator ruling 2026-09-30): its first audio output's
    // per-port ring entry. At rest the shell hands over an empty map and the wells sit empty,
    // because a frozen bar from a dead stream is a lie with a scale on it.
    let Some(band) = nl.well_band else { return };
    if let Some(port) = inset::first_audio_out(&node.spec) {
        let m = meters.get(&(nl.id, port)).copied().unwrap_or_default();
        draw_meter_bars(p, pal, band, &[(m.l, m.hold_l, m.clip_l), (m.r, m.hold_r, m.clip_r)], z);
    }
}

/// The bar vocabulary itself (increment 4's, plus the round-4 clip cap): one hairline well per
/// channel, a data-class fill to the level, an audio-class hold block where a hold was
/// published — and, when the session's clip LATCH is set for the channel (operator round 4,
/// D2: peak ≥ 1.0, held until STOP), a red cap segment AT FULL SCALE: the rail itself wears
/// the latch, in the error token, redundant with the CLIP word on the info band.
fn draw_meter_bars(p: &Painter, pal: &Palette, band: Rect, channels: &[(f32, f32, bool)], z: f32) {
    let body = egui_rect(band);
    let inset = LAYOUT_SPACE_2 as f32 * z;
    let bar_h = LAYOUT_SPACE_2 as f32 * z;
    let well_w = body.width() - inset * 4.0;
    let x0 = body.min.x + inset * 2.0;
    let mut y = body.min.y + inset * 1.5;
    for &(level, hold, clip) in channels {
        let well = egui::Rect::from_min_size(egui::pos2(x0, y), egui::vec2(well_w, bar_h));
        p.rect_stroke(
            well,
            LAYOUT_CORNER_NONE as u8,
            pal.hairline(pal.hairline_faint, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)),
            egui::StrokeKind::Middle,
        );
        let lv = level.clamp(0.0, 1.0);
        if lv > 0.0 {
            let fill = egui::Rect::from_min_size(
                well.left_top(),
                egui::vec2((well_w * lv).max(LAYOUT_STROKE_SIGNAL as f32 * z), bar_h),
            );
            p.rect_filled(fill, LAYOUT_CORNER_NONE as u8, pal.data);
        }
        let hd = hold.clamp(0.0, 1.0);
        if hd > 0.0 {
            let hx = x0 + well_w * hd;
            let block = egui::Rect::from_min_size(
                egui::pos2((hx - LAYOUT_SPACE_1 as f32 * z).max(x0), y),
                egui::vec2(LAYOUT_SPACE_1 as f32 * z, bar_h),
            );
            p.rect_filled(block, LAYOUT_CORNER_NONE as u8, pal.audio);
        }
        if clip {
            let cap_w = (LAYOUT_SPACE_2 as f32 * z).max(2.0);
            let cap = egui::Rect::from_min_size(
                egui::pos2(x0 + well_w - cap_w, y),
                egui::vec2(cap_w, bar_h),
            );
            p.rect_filled(cap, LAYOUT_CORNER_NONE as u8, pal.error);
        }
        y += bar_h + inset;
    }
}

/// The Envelope well (`env/ad`): the attack/decay triangle from params — the module's DECLARED
/// shape, honest at rest and before the first block (D1′: the live cv-value overlay is the
/// declared next half, waiting on a ring that carries per-block cv history). Hairline-faint at
/// rest — the scope rest line's own stroke. Param order is the manifest's (0 attack · 1 decay ·
/// 2 loop · 3 curve), pinned by a sparq-app test.
fn draw_envelope_well(p: &Painter, pal: &Palette, node: &Node, nl: &NodeLayout, z: f32) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    let prm = node.effective_params();
    let pts = inset::envelope_polyline(
        prm.first().copied().unwrap_or(inset::ENV_ATTACK_DEFAULT),
        prm.get(1).copied().unwrap_or(inset::ENV_DECAY_DEFAULT),
        prm.get(3).copied().unwrap_or(0.0) >= 0.5, // the manifest's curve param: 0 exp, 1 lin
        sp_rect(disp),
    );
    if pts.len() >= 2 {
        // The control module's wave display is BLUE (operator ruling 2026-10-01): the LFO's
        // period outline rides the CONTROL class accent at signal weight — the wave is the
        // module's promise, not chrome, so it wears its class colour, not a hairline grey.
        let line = pts.into_iter().map(pos).collect::<Vec<Pos2>>();
        p.line(
            line,
            Stroke::new(
                (LAYOUT_STROKE_SIGNAL as f32 * z).max(0.5),
                class_colour(SignalClass::Cv, pal),
            ),
        );
    }
}

/// The Sparkline well (`mod/lfo`): ONE period of the declared wave from params (0 rate ·
/// 1 shape · 2 depth — the rate does not appear because the x-axis IS one period), the honest
/// at-rest outline; the rolling cv-history overlay is D1′'s declared next half. Hairline-faint
/// at rest, the envelope's stroke.
fn draw_sparkline_well(p: &Painter, pal: &Palette, node: &Node, nl: &NodeLayout, z: f32) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    let prm = node.effective_params();
    let pts = inset::lfo_period_polyline(
        prm.get(1).copied().unwrap_or(0.0),
        prm.get(2).copied().unwrap_or(inset::LFO_DEPTH_DEFAULT),
        sp_rect(disp),
    );
    if pts.len() >= 2 {
        let line = pts.into_iter().map(pos).collect::<Vec<Pos2>>();
        p.line(
            line,
            pal.hairline(pal.hairline_faint, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)),
        );
    }
}

/// The Curve well (`flt/svf`): the thumbnail of the D2 response, no marker (the marker lives
/// in the inspector's plot — one control, one home). The magnitudes come from the shell's
/// dispatch (the filter's own `magnitude_at`, the contract's single source); the curve draws
/// in the node's dominant class colour over a 0 dB datum. Param-derived means it is NEVER at
/// rest while params exist — the honest fallback when the shell supplied no frame (a rate
/// never negotiated) is the flat rest centre line, the scope's own vocabulary.
fn draw_curve_well(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    curves: &Curves,
    z: f32,
) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    let grid = pal.hairline(pal.hairline_faint, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5));
    let Some(frame) = curves.get(&nl.id) else {
        let [a, b] = scope::rest_line(sp_rect(disp));
        p.line_segment([pos(a), pos(b)], grid);
        return;
    };
    let r = sp_rect(disp);
    let y0 = frame.axes.y_of_db(r, 0.0);
    p.line_segment([Pos2::new(disp.min.x, y0), Pos2::new(disp.max.x, y0)], grid);
    let pts = response::polyline(&frame.freqs, &frame.mags, &frame.axes, r);
    if pts.len() >= 2 {
        let class = dominant_class(node);
        let line = pts.into_iter().map(pos).collect::<Vec<Pos2>>();
        p.line(line, Stroke::new(class_width(class) * z, class_colour(class, pal)));
    }
}

// ------------------------------------------------------ the round-4 wells (operator rulings)

/// The clock wheel (`mod/clk`, operator round 4 D4): four concentric divisor rings — 4/8/16/32
/// centre-out on [`inset::clock_ring_rects`] — each with cardinal ticks, its numeral on the
/// top-left diagonal (Full LOD only; the Simplified contract is no text), and a phase dot at
/// `frac(phase × div)`, `div = [1, 2, 4, 8]`: the outer rings turn faster, which IS the musical
/// statement (a 32nd grid spins eight times per quarter). The phase is the module's OWN
/// publication (its 0th cv out, read through [`inset::nth_cv_out`] — no port literal here);
/// at rest the level set is empty, the phase reads 0, and the wheel sits static with its dots
/// at the top — the declared at-rest rule, never faked motion.
fn draw_clock_wheel(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    canvas: &CanvasState,
    full: bool,
    z: f32,
) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    let phase = inset::nth_cv_out(&node.spec, 0)
        .and_then(|pt| canvas.levels.port(nl.id, pt))
        .unwrap_or(0.0);
    let rings = inset::clock_ring_rects(sp_rect(disp));
    let divs = [1.0f32, 2.0, 4.0, 8.0];
    let words = ["4", "8", "16", "32"];
    let hair = pal.hairline(pal.hairline_faint, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5));
    let dot_col = class_colour(SignalClass::Event, pal);
    let tick_len = LAYOUT_SPACE_1 as f32 * z;
    for (k, rr) in rings.iter().enumerate() {
        let rad = rr.width() * 0.5;
        let c = rr.center();
        p.circle_stroke(pos(c), rad, hair);
        for t in 0..4u32 {
            let ph = t as f32 / 4.0;
            let a = pos(inset::ring_point(c, rad, ph));
            let b = pos(inset::ring_point(c, (rad - tick_len).max(0.0), ph));
            p.line_segment([a, b], hair);
        }
        // The phase dot: this ring's own division of the quarter grid.
        let d = pos(inset::ring_point(c, rad, (phase * divs[k]).rem_euclid(1.0)));
        p.circle_filled(d, (LAYOUT_SPACE_1 as f32 * z).max(1.0), dot_col);
        if full {
            // The numeral rides the top-left diagonal — the phase-0 dot is at the top, so the
            // word and the dot never collide at rest (or anywhere on the wheel's own geometry).
            let at = pos(inset::ring_point(c, rad, 0.875));
            p.text(
                at,
                Align2::CENTER_CENTER,
                words[k],
                scaled_font(font_xs(), z),
                pal.text_tertiary,
            );
        }
    }
}

/// The rolling level graph (`ana/rms`, operator round 4 D13): the display-side history the
/// shell pushes per live frame ([`LevelHistories`] — never the engine's, never the journal's)
/// as a hairline trace in the cv accent, with the LIVE word's bar along the bottom (the meter
/// bars' data fill — the reading you can trust this frame). At rest the history is empty and
/// the well shows only its ground: a restart starts empty, declared, never a frozen trace.
fn draw_level_graph(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    canvas: &CanvasState,
    hists: &LevelHistories,
    z: f32,
) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    if let Some(h) = hists.get(&nl.id) {
        let pts = h.polyline(sp_rect(disp));
        if pts.len() >= 2 {
            let stroke = Stroke::new(
                (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5),
                with_alpha(class_colour(SignalClass::Cv, pal), 0.8),
            );
            p.line(pts.into_iter().map(pos).collect::<Vec<Pos2>>(), stroke);
        }
    }
    // The live word: the current publication as a bar along the well's floor — in the CV
    // class colour, NOT the meters' data fill: a meter bar is out/main's alone (the
    // 2026-09-30 ruling the audit pins), and this bar is a cv word about a cv word.
    let lv = inset::nth_cv_out(&node.spec, 0)
        .and_then(|pt| canvas.levels.port(nl.id, pt))
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    if lv > 0.0 {
        let bar_h = (LAYOUT_SPACE_1 as f32 * z).max(1.0);
        let r = egui::Rect::from_min_size(
            disp.left_bottom() - egui::vec2(0.0, bar_h),
            egui::vec2((disp.width() * lv).max(LAYOUT_STROKE_SIGNAL as f32 * z), bar_h),
        );
        p.rect_filled(r, LAYOUT_CORNER_NONE as u8, class_colour(SignalClass::Cv, pal));
    }
}

/// The quantizer keyboard (`util/quant`, operator round 4 D11): the layout's 12 equal key
/// cells — the hit-test's own geometry, so every drawn key lives inside the cell that routes
/// its tap — with the seven naturals full-height in the panel ground and the five accidentals
/// as the classic shorter, narrower, darker overlay. FILLED = in the active scale: Custom
/// reads the node's own `custom-mask` (the parameter the keys themselves edit), a preset reads
/// the module's own [`sparq_audio::modules::QUANT_SCALES`] table — one source, so the display
/// can never revoice a scale. Out-of-scale keys draw as empty wells (the membership IS the
/// display). The passing pitch lights its key in the cv accent, read from the module's
/// quantized-pitch publication through [`inset::keyboard_key`] — the lit key and the snapped
/// note are the same arithmetic.
fn draw_keyboard_well(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    canvas: &CanvasState,
    z: f32,
) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    if nl.key_cells.is_empty() {
        return; // no mask param on this card: the well is ground, honestly
    }
    let prm = node.effective_params();
    let pidx = |id: &str| node.spec.params.iter().position(|d| d.id == id);
    let scale =
        pidx("scale").and_then(|i| prm.get(i).copied()).unwrap_or(0.0).round().clamp(0.0, 15.0)
            as usize;
    let mask: u32 = if scale == 0 {
        pidx("custom-mask")
            .and_then(|i| prm.get(i).copied())
            .unwrap_or(0.0)
            .round()
            .clamp(0.0, 4095.0) as u32
    } else {
        sparq_audio::modules::QUANT_SCALES.get(scale - 1).map(|&(_, m)| m).unwrap_or(0xFFF)
    };
    let lit = inset::nth_cv_out(&node.spec, 0)
        .and_then(|pt| canvas.levels.port(nl.id, pt))
        .map(inset::keyboard_key);
    const BLACKS: [usize; 5] = [1, 3, 6, 8, 10];
    let gap = (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5);
    for (k, cell) in nl.key_cells.iter().enumerate() {
        let mut r = egui_rect(*cell);
        r = egui::Rect::from_min_max(
            r.min + egui::vec2(gap * 0.5, 0.0),
            r.max - egui::vec2(gap * 0.5, 0.0),
        );
        let black = BLACKS.contains(&k);
        if black {
            let dx = r.width() * 0.15;
            r = egui::Rect::from_min_max(
                r.min + egui::vec2(dx, 0.0),
                egui::pos2(r.max.x - dx, r.min.y + r.height() * 0.62),
            );
        }
        if lit == Some(k) {
            // The passing pitch: the key wears the cv accent — the same word the wire lights.
            p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, class_colour(SignalClass::Cv, pal));
            p.rect_stroke(
                r,
                LAYOUT_CORNER_MICRO as u8,
                pal.hairline(pal.hairline_strong, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)),
                StrokeKind::Middle,
            );
        } else if mask & (1 << k) != 0 {
            let fill =
                if black { pal.ground_panel_alt.gamma_multiply(0.35) } else { pal.ground_panel };
            p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, fill);
        } else {
            p.rect_stroke(
                r,
                LAYOUT_CORNER_MICRO as u8,
                pal.hairline(pal.hairline_regular, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)),
                StrokeKind::Middle,
            );
        }
    }
}

/// The random-step bars (`mod/rand`, operator round 4 D12): one bar per step, its height the
/// ring value MIRRORED from the module's own pinned hash ([`inset::rand_step_value`] — the
/// display shows the ring the patch will actually walk, and the cross-crate pin in
/// `bridge.rs`'s tests proves the mirror never drifts). The cursor bar — read from the
/// module's `pos` publication, `floor(pos × steps)` — wears the class glow; the others the
/// class colour. At rest (no session) no bar is lit as cursor: the ring is param-derived and
/// honest before the first block, the walk is a reading, never an invention.
fn draw_step_bars(
    p: &Painter,
    pal: &Palette,
    node: &Node,
    nl: &NodeLayout,
    canvas: &CanvasState,
    z: f32,
) {
    let Some(band) = nl.well_band else { return };
    let disp = egui_rect(band).shrink(LAYOUT_SPACE_1 as f32 * z);
    if disp.width() <= 0.0 || disp.height() <= 0.0 {
        return;
    }
    p.rect_filled(disp, LAYOUT_CORNER_MICRO as u8, pal.ground_inset);
    let prm = node.effective_params();
    let steps = prm.first().copied().unwrap_or(8.0).round().clamp(3.0, 16.0) as usize;
    let seed = prm.get(1).copied().unwrap_or(0.0).round().clamp(0.0, 65_535.0) as u32;
    let cursor = inset::nth_cv_out(&node.spec, 1)
        .and_then(|pt| canvas.levels.port(nl.id, pt))
        .map(|pos01| ((pos01 * steps as f32).floor() as usize).min(steps - 1));
    let gap = LAYOUT_SPACE_1 as f32 * 0.5 * z;
    let w = ((disp.width() - gap * (steps - 1) as f32) / steps as f32).max(1.0);
    let col = class_colour(SignalClass::Cv, pal);
    let glow = class_glow(SignalClass::Cv, pal);
    let floor_h = (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5);
    for i in 0..steps {
        let x = disp.min.x + i as f32 * (w + gap);
        // The slot's floor tick: a zero ring value is still a visible slot (the ring's shape,
        // honestly — an invisible bar would be a value the display swallowed).
        p.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, disp.max.y - floor_h), egui::vec2(w, floor_h)),
            LAYOUT_CORNER_NONE as u8,
            pal.hairline(pal.hairline_regular, (LAYOUT_STROKE_HAIRLINE as f32 * z).max(0.5)).color,
        );
        let v = inset::rand_step_value(seed, i as u32).clamp(0.0, 1.0);
        let h = disp.height() * v;
        if h > floor_h {
            let r = egui::Rect::from_min_size(egui::pos2(x, disp.max.y - h), egui::vec2(w, h));
            p.rect_filled(r, LAYOUT_CORNER_MICRO as u8, if cursor == Some(i) { glow } else { col });
        } else if cursor == Some(i) {
            // The cursor on a zero-height bar: the floor tick itself wears the glow.
            p.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(x, disp.max.y - floor_h),
                    egui::vec2(w, floor_h),
                ),
                LAYOUT_CORNER_NONE as u8,
                glow,
            );
        }
    }
}
