//! The computed canvas layout: one pure pass from (graph, camera, view rect) to screen-space
//! geometry, plus hit-testing. Nothing here draws; the painter in `sparq-app` renders exactly
//! these rects, circles and polylines and invents nothing — the same contract `shell.rs` holds, so
//! the layout audit and the Phase 6 renderer both read *this* and agree by construction.
//!
//! Two spaces, one meaning: the model lives in **world** px (infinite, y-down, on the 8 px snap
//! grid); the [`Camera`] maps world→**screen** px inside the canvas view rect. Capture radii and
//! hit widths are **screen** px (a finger is finger-sized regardless of zoom), node sizes are
//! **world** px (a node is the same patch object however far you've zoomed).

use crate::canvas::camera::{Camera, Lod};
use crate::canvas::model::{Graph, Node, NodeSpec, PortRef, WireId, WireTrim};
use crate::geom::{Rect, Vec2};
use crate::tokens::{
    LAYOUT_CANVAS_NODE_HEADER_HEIGHT, LAYOUT_CANVAS_NODE_INFO_HEIGHT, LAYOUT_CANVAS_NODE_PARAM_ROW,
    LAYOUT_CANVAS_NODE_PARAM_ROWS_MAX, LAYOUT_CANVAS_NODE_PORT_OFFSET, LAYOUT_CANVAS_NODE_PORT_ROW,
    LAYOUT_CANVAS_NODE_WELL_HEIGHT, LAYOUT_CANVAS_NODE_WELL_HEIGHT_SCOPE,
    LAYOUT_CANVAS_NODE_WIDTH_DEFAULT, LAYOUT_SPACE_1, LAYOUT_SPACE_2,
    LAYOUT_TOUCH_PORT_CAPTURE_RADIUS, LAYOUT_TOUCH_WIRE_HIT_WIDTH,
};
use sparq_module_api::manifest::Port;
use sparq_module_api::port::Direction;

/// The signal class a port belongs to — the colour axis of the whole identity (plan §3.3, one
/// accent per signal class), derived from the port's type and channel set. `Neutral` is for the
/// two non-signal types (`gpu`, `atom`) that carry no accent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalClass {
    /// Audio-rate signal (amber).
    Audio,
    /// Control voltage (cyan).
    Cv,
    /// Events: MIDI/OSC/trigger/gate (magenta).
    Event,
    /// Data streams (green).
    Data,
    /// Multichannel / ambisonic / object audio (violet) — an audio port with a spatial set.
    Spatial,
    /// Non-signal types (`gpu`, `atom`): no accent, drawn in the neutral text colour.
    Neutral,
}

/// The signal class of a port.
#[must_use]
pub fn signal_class(port: &Port) -> SignalClass {
    use sparq_module_api::port::PortType;
    match port.port_type {
        PortType::Audio => {
            if port.channel_set.is_some_and(|c| c.is_spatial()) {
                SignalClass::Spatial
            } else {
                SignalClass::Audio
            }
        },
        PortType::Cv => SignalClass::Cv,
        PortType::Event => SignalClass::Event,
        PortType::Data => SignalClass::Data,
        PortType::Gpu | PortType::Atom => SignalClass::Neutral,
    }
}

/// A port, laid out: where its circle sits (world and screen), which side, and its class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortLayout {
    /// Which port.
    pub pref: PortRef,
    /// Input or output.
    pub dir: Direction,
    /// Signal class (colour + redundant encoding).
    pub class: SignalClass,
    /// Circle centre in world px.
    pub world: Vec2,
    /// Circle centre in screen px.
    pub screen: Vec2,
}

/// A node, laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeLayout {
    /// Which node.
    pub id: crate::canvas::model::NodeId,
    /// Body rect in world px.
    pub world: Rect,
    /// Body rect in screen px.
    pub screen: Rect,
    /// Header band in screen px (the title strip).
    pub header_screen: Rect,
    /// Inline parameter rows (WO-012 increment 6, the PN convergence): manifest order, capped at
    /// [`LAYOUT_CANVAS_NODE_PARAM_ROWS_MAX`]; the row rect is the touch/draw band, the track is
    /// the slider's x-mapping span. Screen px. Empty at any LOD — the rows exist in the geometry
    /// regardless, the painter and the hit-test both gate on Full.
    pub param_rows: Vec<ParamRowLayout>,
    /// Parameters beyond the cap: how many, and where the "+N MORE" word sits (screen px).
    pub param_overflow: Option<(usize, Rect)>,
    /// The inset display band (scope / meters / envelope / sparkline / curve) in screen px —
    /// `None` when the registry gives this node no well.
    pub well_band: Option<Rect>,
    /// The driver-info band (operator ruling 2026-10-01): `Some` ONLY on the permanent
    /// `out/main` node — where the negotiated device truth (backend, device, rate, block,
    /// channels, format) is read. Screen px.
    pub info_band: Option<Rect>,
    /// The 16 step buttons of `mod/seq`'s pattern row (operator ruling 2026-10-01 r3): the
    /// sequencer's program is a row of buttons the clock walks, not a mask slider — the
    /// cells are screen px, in card order left→right = step 1→16, and the param row they
    /// replace keeps its band for label and hit routing. Empty for every other node.
    pub step_cells: Vec<Rect>,
    /// Which param the step cells edit (`mod/seq`'s pattern); `None` elsewhere.
    pub step_param: Option<usize>,
    /// The quantizer keyboard's 12 keys (operator round 4, D11): the WELL band cut into 12
    /// equal cells (screen px, card order left→right = pitch class 0→11), inset 15 %
    /// vertically so the painter's black-key overlay has room to hang into. A tap flips the
    /// class's CUSTOM-scale membership through [`Hit::Key`]. Empty for every other node.
    pub key_cells: Vec<Rect>,
    /// Which param the keyboard edits (`util/quant`'s `custom-mask`); `None` elsewhere.
    pub key_param: Option<usize>,
    /// Ports, inputs then outputs, in spec order within each direction.
    pub ports: Vec<PortLayout>,
}

/// One inline parameter row on a node card: which parameter, its band, and the slider track the
/// gesture path maps x → value through (the inspector's mapping, on the canvas).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamRowLayout {
    /// Index into the node spec's params (manifest order).
    pub index: usize,
    /// The row band, screen px — the touch target and the label/value line.
    pub row: Rect,
    /// The slider track span, screen px (x-mapping; the drawn line is thinner).
    pub track: Rect,
    /// The CONTROL SINK dot (operator ruling 2026-10-01 r3): where a cv wire lands to modulate
    /// this float parameter — a small dot floating beside the row, left of the card, in the
    /// ports' own column but at the row's height. `None` for non-float params (a toggle or a
    /// menu is a decision, not a voltage). Drawn only while a control wire is in flight.
    pub sink: Option<Vec2>,
}

/// A wire, laid out as a sampled bézier polyline in screen px.
#[derive(Clone, Debug, PartialEq)]
pub struct WireLayout {
    /// Which wire.
    pub id: WireId,
    /// Signal class (from the source port).
    pub class: SignalClass,
    /// The node owning the SOURCE end — the occlusion rule needs the owner's draw order.
    pub from_node: crate::canvas::model::NodeId,
    /// The node owning the DESTINATION end.
    pub to_node: crate::canvas::model::NodeId,
    /// The bézier, sampled to a polyline in screen px (index 0 = source, last = destination).
    pub points: Vec<Vec2>,
    /// True when the matrix required a conversion (multi→mono): draw a warning hairline.
    pub conversion: bool,
    /// The re-patch grab point of the SOURCE end: a fixed screen distance along the wire from the
    /// port, clear of the port's own 24 px capture, so "grab the wire end" and "draw a new wire
    /// from the port" are two distinguishable touches (increment 3).
    pub grab_from: Vec2,
    /// The re-patch grab point of the DESTINATION end (same rule, walked from the far end).
    pub grab_to: Vec2,
    /// A CONTROL wire (operator ruling 2026-10-01 r3): cv source → float parameter sink.
    /// Drawn dashed in the control colour with a dot at the sink; not re-patchable.
    pub is_param: bool,
    /// The cable node (operator round 4, D15): the wire's own trim record, `None` when the
    /// wire runs clean. The painter draws the handle (or, on a clean wire under the pointer,
    /// the hover ghost) at [`Self::trim_point`]; the hit-test offers [`Hit::WireTrim`] on the
    /// same point — one geometry, two readers.
    pub trim: Option<WireTrim>,
    /// Where the cable node sits: the ARC MIDPOINT of the sampled polyline, screen px — the
    /// cable's visible centre, for béziers and straight runs alike ([`arc_midpoint`]).
    /// Recomputed with the points in [`CanvasLayout::make_wires_straight`], so the handle can
    /// never sit off the wire the style toggle restyled.
    pub trim_point: Vec2,
}

/// Which end of a wire a hit or a re-patch drag names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireEndSide {
    /// The source (output-port) end.
    From,
    /// The destination (input-port) end.
    To,
}

/// The whole canvas, laid out for one frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanvasLayout {
    /// Level of detail for this zoom.
    pub lod: Lod,
    /// Nodes, in graph order.
    pub nodes: Vec<NodeLayout>,
    /// Wires, in graph order.
    pub wires: Vec<WireLayout>,
}

/// How many segments a wire bézier is sampled into. A geometric fidelity constant (not a visual
/// size): enough that the 20 px hit test never misses a curve, cheap enough for 200 nodes.
const WIRE_SEGMENTS: usize = 24;

/// The control-sink dot's float offset left of the card (operator ruling 2026-10-01 r3): the
/// ports' own column, at the parameter row's height — beside the setting it controls.
fn off_sink() -> f32 {
    LAYOUT_CANVAS_NODE_PORT_OFFSET as f32
}

/// Whether param `i` of `n` is a float — the only kind a control wire can drive (r3).
fn desc_kind_is_float(n: &Node, i: usize) -> bool {
    use sparq_module_api::manifest::ParamKind;
    n.spec.params.get(i).is_some_and(|d| d.kind == ParamKind::Float)
}

/// The capture radius of a control-sink dot, screen px: HALF the port capture — the dot is
/// smaller than the main in/out (operator ruling r3), and so is its grab, so a sink can never
/// steal a press from a port that happens to sit level with a row.
pub const SINK_CAPTURE_RADIUS: f32 = LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32 / 2.0;

/// The control sink under a screen point, if any float-param dot of any node is within
/// [`SINK_CAPTURE_RADIUS`]. Called ONLY while a control wire is in flight — at rest the dots
/// are not drawn, and what you cannot see you cannot touch.
#[must_use]
pub fn param_sink_at(
    layout: &CanvasLayout,
    pos: Vec2,
) -> Option<(crate::canvas::model::NodeId, usize)> {
    for n in layout.nodes.iter().rev() {
        for pr in &n.param_rows {
            if let Some(sink) = pr.sink {
                if sink.distance(pos) <= SINK_CAPTURE_RADIUS {
                    return Some((n.id, pr.index));
                }
            }
        }
    }
    None
}

/// How far along a wire (screen px) its re-patch grab points sit: clear of the port's capture
/// circle (24 px) by one space step, so the two gestures never fight over the same pixel. On a
/// wire shorter than twice this, both grabs converge to the midpoint and the SOURCE end wins the
/// hit — a degenerate case, documented rather than special-cased.
const WIRE_END_GRAB_OFFSET: f32 = (LAYOUT_TOUCH_PORT_CAPTURE_RADIUS + LAYOUT_SPACE_2) as f32;

/// The world size of a node from its spec: fixed token width, height from the busier edge.
///
/// Rows = the larger of the input and output counts (at least one, so an empty module is still a
/// grabbable body). Height = header + rows × port row. With one row this is 32 + 48 = 80 world px,
/// whose shorter side (80) clears the class-L minimum (72), which is what makes a node auditable
/// as a touch target without a special case.
/// How many inline parameter rows a card draws (the cap), and how many parameters overflow it.
#[must_use]
pub fn param_split(spec: &NodeSpec) -> (usize, usize) {
    let n = spec.params.len();
    let shown = n.min(LAYOUT_CANVAS_NODE_PARAM_ROWS_MAX as usize);
    (shown, n - shown)
}

/// The card's vertical anatomy (world px): header, param band, well band, port band — the
/// Persistent-Nodes convergence's card, in sparq's token scale (WO-012 increment 6). The scope
/// well is the BIG one (operator ruling 2026-10-01: a display you can measure needs room).
#[must_use]
pub fn node_bands(spec: &NodeSpec) -> (f32, Option<f32>, f32) {
    // WO-020 INC4 §8.2: an instrument's mid band is its DISPLAY (the declared min_size), and its
    // param band is the manifest's panel-widget rows (the toolbar), not the backbone's inline
    // param rows — the wall's 26 params live in the inspector + the dropdown picker (§8.4).
    if spec.layer == sparq_module_api::manifest::Layer::Instrument {
        if let Some(d) = spec.display {
            let param_h = instrument_panel_band(d.panel_rows);
            return (param_h, Some(d.min_h), param_h + d.min_h);
        }
    }
    let (shown, overflow) = param_split(spec);
    let row = LAYOUT_CANVAS_NODE_PARAM_ROW as f32;
    let mut param_h = shown as f32 * row;
    if overflow > 0 {
        param_h += row / 2.0; // the "+N MORE" word rides a half row — a reading, not a control
    }
    let well = crate::canvas::inset::well_for(spec).map(|w| match w {
        crate::canvas::inset::Well::Scope => LAYOUT_CANVAS_NODE_WELL_HEIGHT_SCOPE as f32,
        _ => LAYOUT_CANVAS_NODE_WELL_HEIGHT as f32,
    });
    (param_h, well, param_h + well.unwrap_or(0.0))
}

/// The height of the `out/main` driver-info band (operator ruling 2026-10-01): Main Out is the
/// permanent node, and the negotiated device truth (backend · device · rate · block · channels ·
/// format) is read ON it — `Some` only for `out/main`, `None` for everything else.
#[must_use]
pub fn info_band_height(spec: &NodeSpec) -> Option<f32> {
    (spec.module_id == crate::canvas::OUT_MAIN_ID).then_some(LAYOUT_CANVAS_NODE_INFO_HEIGHT as f32)
}

/// Whether a node's card is an instrument's (D6 sizing, display band, no inline param rows).
fn spec_is_instrument(spec: &NodeSpec) -> bool {
    spec.layer == sparq_module_api::manifest::Layer::Instrument && spec.display.is_some()
}

/// The instrument panel band's height (D6): `rows × the 44 px touch floor + the 16 px gaps`
/// between them; zero rows cost nothing.
#[must_use]
pub fn instrument_panel_band(rows: u32) -> f32 {
    if rows == 0 {
        return 0.0;
    }
    let row = crate::tokens::LAYOUT_TOUCH_TARGET_S as f32;
    let gap = crate::tokens::LAYOUT_SPACE_4 as f32;
    rows as f32 * row + (rows - 1) as f32 * gap
}

/// The rect an instrument's display band occupies inside its card (D6/§8.2): the card minus the
/// gutters, the header, and the panel band — i.e. exactly the declared `min_size` area the guest
/// lays out against. `None` for non-instrument nodes (their body is the well band, not a display).
#[must_use]
pub fn instrument_display_band(card: Rect, spec: &NodeSpec) -> Option<Rect> {
    let d = spec.display?;
    if spec.layer != sparq_module_api::manifest::Layer::Instrument {
        return None;
    }
    let gutter = crate::tokens::LAYOUT_SPACE_GUTTER as f32;
    let header = crate::tokens::LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32;
    let y = card.min.y + gutter + header + instrument_panel_band(d.panel_rows);
    Some(Rect::from_min_size(
        Vec2::new(card.min.x + gutter, y),
        Vec2::new(d.min_w.min(card.width() - 2.0 * gutter), d.min_h.min((card.max.y - gutter) - y)),
    ))
}

/// The `util/mult` card's width factor (operator round 4, D6): the junction bus is a strip of
/// dots, not a two-sided card — a quarter of the default width, the narrowest thing on the
/// canvas, because it is the least thing: a bus, not a module.
pub const MULT_WIDTH_FACTOR: f32 = 0.25;

/// The node card's world size: header + param band + well band + info band + port band (the
/// increment-6 anatomy). With one port row and no params/well this is the old 80 px box, whose
/// shorter side clears the class-L minimum — the audit property the original size was chosen for.
#[must_use]
pub fn node_size(spec: &NodeSpec) -> Vec2 {
    // WO-020 plan D6: an instrument's card is its DISPLAY plus chrome — the wall-class ceiling
    // (`node_width_instrument_max`) governs, not the backbone's 480. card = min_size + gutter each
    // side + header + panel band (rows × the 44 touch floor + the 16 gaps) + gutter top/bottom; the
    // port band is absent because a port-less instrument has no ports (D5). The formula reproduces
    // the plan's numbers: 2176×1120 + chrome = 2208×1288.
    if spec.layer == sparq_module_api::manifest::Layer::Instrument {
        if let Some(d) = spec.display {
            let gutter = crate::tokens::LAYOUT_SPACE_GUTTER as f32;
            let header = crate::tokens::LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32;
            let w = (d.min_w + 2.0 * gutter)
                .min(crate::tokens::LAYOUT_CANVAS_NODE_WIDTH_INSTRUMENT_MAX as f32);
            let h = d.min_h + header + instrument_panel_band(d.panel_rows) + 2.0 * gutter;
            return Vec2::new(w, h);
        }
    }
    let inputs = spec.inputs().count();
    let outputs = spec.outputs().count();
    let rows = inputs.max(outputs).max(1);
    // The mult strip (D6): quarter width — the dots are the module, and card chrome around
    // them would be chrome around nothing. Every other module wears the token width.
    let w = if spec.module_id == crate::canvas::MULT_ID {
        LAYOUT_CANVAS_NODE_WIDTH_DEFAULT as f32 * MULT_WIDTH_FACTOR
    } else {
        LAYOUT_CANVAS_NODE_WIDTH_DEFAULT as f32
    };
    let (param_h, well_h, _) = node_bands(spec);
    let h = LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32
        + param_h
        + well_h.unwrap_or(0.0)
        + info_band_height(spec).unwrap_or(0.0)
        + rows as f32 * LAYOUT_CANVAS_NODE_PORT_ROW as f32;
    Vec2::new(w, h)
}

/// Compute the layout for `graph` under `camera` inside `view` (the canvas rect, screen px).
#[must_use]
pub fn compute(graph: &Graph, camera: &Camera, view: Rect) -> CanvasLayout {
    let lod = camera.lod();
    let mut nodes = Vec::with_capacity(graph.node_count());
    // Port world positions, keyed by (node, index), so wires can find their endpoints after the
    // node pass without recomputing geometry.
    let mut port_world: Vec<(PortRef, Vec2, Direction, SignalClass)> = Vec::new();

    for n in graph.nodes() {
        let nl = layout_node(n, camera, view, &mut port_world);
        nodes.push(nl);
    }

    let wires = graph
        .wires()
        .iter()
        .filter_map(|w| {
            let from = port_world.iter().find(|(p, _, _, _)| *p == w.from)?;
            let (_, f_world, _, f_class) = *from;
            let f_screen = camera.to_screen(f_world, view);
            // A control wire (r3) ends at the parameter's sink dot, not at a port.
            let t_screen = match w.param {
                Some(pi) => nodes
                    .iter()
                    .find(|n| n.id == w.to.node)
                    .and_then(|n| n.param_rows.iter().find(|pr| pr.index == pi))
                    .and_then(|pr| pr.sink)?,
                None => {
                    let to = port_world.iter().find(|(p, _, _, _)| *p == w.to)?;
                    camera.to_screen(to.1, view)
                },
            };
            let points = sample_bezier(f_screen, t_screen);
            let (grab_from, grab_to) = grab_points(&points);
            // The cable node (D15): the record rides the wire, the handle sits at the arc
            // midpoint — the layout reads the model, it never re-derives either.
            let trim_point = arc_midpoint(&points);
            // A conversion (multi→mono) is a property of the two channel sets; recompute it from
            // the source/dest ports so the painter can draw the warning hairline.
            let conversion = is_conversion(graph, w.from, w.to);
            Some(WireLayout {
                id: w.id,
                class: f_class,
                from_node: w.from.node,
                to_node: w.to.node,
                points,
                conversion,
                grab_from,
                grab_to,
                is_param: w.param.is_some(),
                trim: w.trim,
                trim_point,
            })
        })
        .collect();

    CanvasLayout { lod, nodes, wires }
}

fn layout_node(
    n: &Node,
    camera: &Camera,
    view: Rect,
    port_world: &mut Vec<(PortRef, Vec2, Direction, SignalClass)>,
) -> NodeLayout {
    let size = node_size(&n.spec);
    let world = Rect::from_min_size(n.pos, size);
    let screen = Rect::new(camera.to_screen(world.min, view), camera.to_screen(world.max, view));
    let header_h = camera.screen_len(LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32);
    let header_screen = Rect::new(screen.min, Vec2::new(screen.max.x, screen.min.y + header_h));

    let row = LAYOUT_CANVAS_NODE_PORT_ROW as f32;
    let header = LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32;
    let (param_h, well_h, mid_h) = node_bands(&n.spec);
    let mut ports = Vec::new();

    // Inline parameter rows (increment 6): manifest order, capped; the track spans the card
    // minus token padding, sitting in the row's lower half (label/value line above it).
    // Instrument cards show NONE: their panel band is the manifest toolbar's reservation and
    // their params live in the inspector + picker (§8.4) until WO-014 types the panel.
    let is_instrument = spec_is_instrument(&n.spec);
    let (shown, overflow) = if is_instrument { (0, 0) } else { param_split(&n.spec) };
    let prow = LAYOUT_CANVAS_NODE_PARAM_ROW as f32;
    let pad = LAYOUT_SPACE_2 as f32;
    let mut param_rows = Vec::with_capacity(shown);
    for i in 0..shown {
        let y0 = n.pos.y + header + i as f32 * prow;
        let row_rect = Rect::new(Vec2::new(n.pos.x, y0), Vec2::new(n.pos.x + size.x, y0 + prow));
        let track = Rect::new(
            Vec2::new(n.pos.x + pad, y0 + prow * 0.55),
            Vec2::new(n.pos.x + size.x - pad, y0 + prow - pad * 0.5),
        );
        let sink = (desc_kind_is_float(n, i)).then(|| {
            let wpos = Vec2::new(n.pos.x - off_sink(), row_rect.center().y);
            camera.to_screen(wpos, view)
        });
        param_rows.push(ParamRowLayout {
            index: i,
            row: Rect::new(
                camera.to_screen(row_rect.min, view),
                camera.to_screen(row_rect.max, view),
            ),
            track: Rect::new(camera.to_screen(track.min, view), camera.to_screen(track.max, view)),
            sink,
        });
    }
    let param_overflow = (overflow > 0).then(|| {
        let y0 = n.pos.y + header + shown as f32 * prow;
        let r = Rect::new(Vec2::new(n.pos.x, y0), Vec2::new(n.pos.x + size.x, y0 + prow / 2.0));
        (overflow, Rect::new(camera.to_screen(r.min, view), camera.to_screen(r.max, view)))
    });
    let well_band = if is_instrument {
        // The display band: exactly the declared min_size area inside the gutters (D6).
        instrument_display_band(Rect::from_min_size(n.pos, size), &n.spec)
            .map(|r| Rect::new(camera.to_screen(r.min, view), camera.to_screen(r.max, view)))
    } else {
        well_h.map(|wh| {
            let y0 = n.pos.y + header + param_h;
            let r = Rect::new(Vec2::new(n.pos.x, y0), Vec2::new(n.pos.x + size.x, y0 + wh));
            Rect::new(camera.to_screen(r.min, view), camera.to_screen(r.max, view))
        })
    };
    // The out/main driver-info band (operator ruling 2026-10-01): under the well, above the
    // port band — the permanent node's window onto the negotiated device truth.
    let info_band = info_band_height(&n.spec).map(|ih| {
        let y0 = n.pos.y + header + param_h + well_h.unwrap_or(0.0);
        let r = Rect::new(Vec2::new(n.pos.x, y0), Vec2::new(n.pos.x + size.x, y0 + ih));
        Rect::new(camera.to_screen(r.min, view), camera.to_screen(r.max, view))
    });
    let _ = mid_h;

    // The sequencer's 16 step buttons (operator ruling 2026-10-01 r3): the pattern param's
    // row band carries a 16-cell strip instead of a slider track — the program is buttons
    // the clock walks, not a number.
    let (step_cells, step_param) = if n.spec.module_id == crate::canvas::SEQ_ID {
        let idx = n.spec.params.iter().position(|d| d.id == "pattern");
        let cells = idx.map(|pi| {
            let y0 = n.pos.y + header + pi as f32 * prow;
            let band = Rect::new(
                Vec2::new(n.pos.x + pad, y0 + prow * 0.5),
                Vec2::new(n.pos.x + size.x - pad, y0 + prow - pad * 0.5),
            );
            let gap = LAYOUT_SPACE_1 as f32 * 0.5;
            let w = (band.width() - gap * 15.0) / 16.0;
            (0..16)
                .map(|k| {
                    let x0 = band.min.x + k as f32 * (w + gap);
                    Rect::new(
                        camera.to_screen(Vec2::new(x0, band.min.y), view),
                        camera.to_screen(Vec2::new(x0 + w, band.max.y), view),
                    )
                })
                .collect::<Vec<Rect>>()
        });
        (cells.unwrap_or_default(), idx)
    } else {
        (Vec::new(), None)
    };

    // The quantizer's keyboard (operator round 4, D11): the WELL band — not a param row, the
    // keys are not sliders — cut into 12 equal cells with a 15 % vertical inset (the painter's
    // black-key overlay hangs into it). The cells exist only when the card has BOTH the mask
    // param to edit and the well band to put them in (the step_cells block's discipline: never
    // route a tap to a param that does not exist).
    let (key_cells, key_param) = if n.spec.module_id == crate::canvas::QUANT_ID {
        let idx = n.spec.params.iter().position(|d| d.id == "custom-mask");
        let cells = idx.zip(well_band).map(|(_, band)| {
            let keys = crate::canvas::inset::KEYBOARD_KEYS;
            let w = band.width() / keys as f32;
            let inset_y = band.height() * 0.15;
            (0..keys)
                .map(|k| {
                    Rect::new(
                        Vec2::new(band.min.x + k as f32 * w, band.min.y + inset_y),
                        Vec2::new(band.min.x + (k + 1) as f32 * w, band.max.y - inset_y),
                    )
                })
                .collect::<Vec<Rect>>()
        });
        (cells.unwrap_or_default(), idx)
    } else {
        (Vec::new(), None)
    };

    // Inputs down the left edge, outputs down the right, each in its own row stack — the stack
    // starts BELOW the param and well bands (the card's anatomy, increment 6). The circles
    // FLOAT beside the window (operator ruling 2026-10-01): the port offset token outside the
    // card edge, so a port is never half-buried in the body it belongs to.
    //
    // The junction bus is the one exception (operator round 4, D6): mult's six dots wear ONE
    // CENTRED COLUMN — the bus is not a two-sided card (every dot is in and out at once, its
    // role the first connection's), so the left/right split would be a lie the geometry tells.
    // The rows ride the same stack formula (mult has no params, well or info band, so those
    // terms are zero — the formula, not a special case), and `dir` stays what the manifest
    // declares: the connect rules already make the dots bidirectional; positions move,
    // semantics don't.
    let off = LAYOUT_CANVAS_NODE_PORT_OFFSET as f32;
    let info_h = info_band_height(&n.spec).unwrap_or(0.0);
    if n.spec.module_id == crate::canvas::MULT_ID {
        for (idx, port) in n.spec.ports.iter().enumerate() {
            let wpos = Vec2::new(
                n.pos.x + size.x * 0.5,
                n.pos.y
                    + header
                    + param_h
                    + well_h.unwrap_or(0.0)
                    + info_h
                    + idx as f32 * row
                    + row / 2.0,
            );
            let class = signal_class(port);
            let pref = PortRef::new(n.id, idx);
            ports.push(PortLayout {
                pref,
                dir: port.direction,
                class,
                world: wpos,
                screen: camera.to_screen(wpos, view),
            });
            port_world.push((pref, wpos, port.direction, class));
        }
    } else {
        for (side, dir) in [(0.0, Direction::In), (size.x, Direction::Out)] {
            let mut r = 0usize;
            for idx in
                n.spec.ports.iter().enumerate().filter(|(_, p)| p.direction == dir).map(|(i, _)| i)
            {
                let port = match n.spec.port(idx) {
                    Some(p) => p,
                    None => continue,
                };
                let wx = n.pos.x + side + if dir == Direction::In { -off } else { off };
                let wy = n.pos.y
                    + header
                    + param_h
                    + well_h.unwrap_or(0.0)
                    + info_h
                    + r as f32 * row
                    + row / 2.0;
                let wpos = Vec2::new(wx, wy);
                let class = signal_class(port);
                let pref = PortRef::new(n.id, idx);
                ports.push(PortLayout {
                    pref,
                    dir,
                    class,
                    world: wpos,
                    screen: camera.to_screen(wpos, view),
                });
                port_world.push((pref, wpos, dir, class));
                r += 1;
            }
        }
    }

    NodeLayout {
        id: n.id,
        world,
        screen,
        header_screen,
        param_rows,
        param_overflow,
        well_band,
        info_band,
        step_cells,
        step_param,
        key_cells,
        key_param,
        ports,
    }
}

/// Whether a wire is a documented conversion (the only silent one: multi→mono summing).
fn is_conversion(graph: &Graph, from: PortRef, to: PortRef) -> bool {
    use sparq_module_api::port::ChannelSet;
    let (Some(s), Some(d)) = (graph.port(from), graph.port(to)) else {
        return false;
    };
    matches!(
        (s.channel_set, d.channel_set),
        (Some(sc), Some(ChannelSet::Mono)) if sc != ChannelSet::Mono && sc != ChannelSet::Variable
    )
}

/// Sample the horizontal bézier from `a` (an output) to `b` (an input). Control-point offset is
/// 0.5 × the horizontal distance (the token's `wire_style = "bezier-horizontal"`), so a wire
/// leaves its port heading right and enters its target heading right — the readable modular look.
fn sample_bezier(a: Vec2, b: Vec2) -> Vec<Vec2> {
    let dx = b.x - a.x;
    let off = 0.5 * dx;
    let c1 = Vec2::new(a.x + off, a.y);
    let c2 = Vec2::new(b.x - off, b.y);
    let mut pts = Vec::with_capacity(WIRE_SEGMENTS + 1);
    for i in 0..=WIRE_SEGMENTS {
        let t = i as f32 / WIRE_SEGMENTS as f32;
        pts.push(cubic(a, c1, c2, b, t));
    }
    pts
}

fn cubic(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2, t: f32) -> Vec2 {
    let u = 1.0 - t;
    let a = u * u * u;
    let b = 3.0 * u * u * t;
    let c = 3.0 * u * t * t;
    let d = t * t * t;
    Vec2::new(a * p0.x + b * p1.x + c * p2.x + d * p3.x, a * p0.y + b * p1.y + c * p2.y + d * p3.y)
}

/// The two re-patch grab points of a sampled wire: [`WIRE_END_GRAB_OFFSET`] screen px along the
/// polyline from each end (clamped to the midpoint on short wires, so the grabs never cross).
fn grab_points(points: &[Vec2]) -> (Vec2, Vec2) {
    let total = polyline_len(points);
    let off = WIRE_END_GRAB_OFFSET.min(total * 0.5);
    (walk_along(points, off, true), walk_along(points, off, false))
}

/// Total polyline length, screen px.
fn polyline_len(points: &[Vec2]) -> f32 {
    let mut acc = 0.0;
    for w in points.windows(2) {
        acc += w[0].distance(w[1]);
    }
    acc
}

/// The point `offset` px along the polyline, walked from the start (`from_start`) or the end.
/// Degenerate inputs (empty / single point / offset past the end) return the nearest real point
/// rather than a fabricated one.
fn walk_along(points: &[Vec2], offset: f32, from_start: bool) -> Vec2 {
    let n = points.len();
    if n == 0 {
        return Vec2::ZERO;
    }
    if n == 1 {
        return points[0];
    }
    let mut acc = 0.0;
    for i in 0..n - 1 {
        let (a, b) = if from_start {
            (points[i], points[i + 1])
        } else {
            (points[n - 1 - i], points[n - 2 - i])
        };
        let seg = a.distance(b);
        let last = i == n - 2;
        if acc + seg >= offset || last {
            let t = if last && acc + seg < offset {
                1.0
            } else {
                ((offset - acc) / seg.max(f32::EPSILON)).clamp(0.0, 1.0)
            };
            return Vec2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
        }
        acc += seg;
    }
    points[0]
}

/// The ARC MIDPOINT of a sampled wire: the point at half the cumulative distance along the
/// polyline — where the cable node sits (operator round 4, D15), so the handle is at the
/// cable's visible centre on a sagging bézier too, not at the middle sample index. Degenerate
/// inputs read the nearest real thing, never a fabrication: empty is the origin, a single
/// point is that point (the `walk_along` discipline — this IS that walk, at half length).
fn arc_midpoint(points: &[Vec2]) -> Vec2 {
    walk_along(points, polyline_len(points) * 0.5, true)
}

/// What a screen point lands on. Ports beat wire ends beat node bodies beat wires, so a finger
/// near an edge port wires rather than selects the node — and a finger on a wire's grab point
/// re-patches rather than moves whatever is behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// A port (with its direction), within the capture radius.
    Port(PortRef, Direction),
    /// A wire's re-patch grab point (which wire, which end), within the capture radius.
    WireEnd(WireId, WireEndSide),
    /// A wire's CABLE NODE (operator round 4, D15): the trim handle at the wire's arc
    /// midpoint, within the capture radius. Offered only while the wire CARRIES a trim —
    /// what is not drawn is not touchable; the hover-ghost tap-to-insert rides the
    /// [`Hit::Wire`] arm instead. Ranked above the re-patch grabs (a visible handle beats
    /// the wire's ends), and dead under a covering card like every wire furniture.
    WireTrim(WireId),
    /// An inline parameter row on a node card (which node, which param, the slider track) —
    /// the row under the finger is the row you edit (increment 6).
    Param(crate::canvas::model::NodeId, usize, Rect),
    /// A step button of the sequencer's 16-button row (which node, which step) — the button
    /// under the finger toggles that step's bit (operator ruling 2026-10-01 r3).
    Step(crate::canvas::model::NodeId, usize),
    /// A key of the quantizer's keyboard well (which node, which pitch class 0..11, operator
    /// round 4 D11) — the key under the finger flips that class's membership in the CUSTOM
    /// scale through the param door; a preset scale refuses in words.
    Key(crate::canvas::model::NodeId, usize),
    /// A node body.
    Node(crate::canvas::model::NodeId),
    /// A wire.
    Wire(WireId),
    /// Empty canvas.
    Empty,
}

impl CanvasLayout {
    /// Restyle every wire to a straight run between its endpoints (the toolbar's wire select,
    /// increment 6): the points AND the re-patch grab points move together, so the hit-test and
    /// the painter keep agreeing — what you see is what you can grab. The cable node rides the
    /// restyled run too (its arc midpoint is recomputed): a handle left on the old bézier would
    /// sit off the wire you see, and what you cannot see on the cable you cannot touch.
    pub fn make_wires_straight(&mut self) {
        for w in &mut self.wires {
            let (Some(&first), Some(&last)) = (w.points.first(), w.points.last()) else {
                continue;
            };
            w.points = vec![first, last];
            let (gf, gt) = grab_points(&w.points);
            w.grab_from = gf;
            w.grab_to = gt;
            w.trim_point = arc_midpoint(&w.points);
        }
    }
}

/// Hit-test a screen point against a computed layout. `lod` gates port and wire-end hits: at
/// [`Lod::Dot`] neither is drawn at scale, so neither is targetable (a 24 px capture around an
/// invisible handle would only cause mis-wires).
///
/// COVERED IS UNTOUCHABLE (operator ruling 2026-10-01): nodes draw in list order — later nodes
/// sit ON TOP — so a port (or a wire's re-patch grab) belonging to an EARLIER node that lies
/// under a LATER node's body is invisible, and what you cannot see you cannot touch: the click
/// lands on the covering card, never on the buried port.
#[must_use]
pub fn hit_test(layout: &CanvasLayout, pos_screen: Vec2, lod: Lod) -> Hit {
    let capture = LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32;
    // Is the point `at` under a node card drawn AFTER the owner at `owner_idx`?
    let covered = |owner_idx: usize, at: Vec2| -> bool {
        layout.nodes.iter().enumerate().skip(owner_idx + 1).any(|(_, n)| n.screen.contains(at))
    };
    let node_index =
        |id: crate::canvas::model::NodeId| layout.nodes.iter().position(|n| n.id == id);
    if lod != Lod::Dot {
        // A press INSIDE a card body never belongs to a port or a wire grab: the card's own
        // surface (its slider rows, its body) is what you see there. Ports float OUTSIDE the
        // window (operator ruling 2026-10-01), and their 24 px capture ring reaches back over
        // the card edge — without this rule the ring silently ate clicks on the last param
        // row's ends ("the card slider sometimes does not respond"), while the inspector's
        // row, which no port ring touches, always did. A port stays hittable where it is
        // drawn: outside every body that draws over it (its owner's own body included — the
        // capture ring over the owner's edge is invisible ink).
        let body_at = |at: Vec2| layout.nodes.iter().position(|n| n.screen.contains(at));
        // A port is blocked exactly when the point lands on its owner's own card or on any
        // card drawn later (the owner's body included: the capture ring over the owner's edge
        // is invisible ink). A point on an EARLIER card is fine — this port draws over it.
        let port_blocked =
            |owner_idx: usize, at: Vec2| -> bool { body_at(at).is_some_and(|bi| bi >= owner_idx) };
        for (ni, n) in layout.nodes.iter().enumerate() {
            for p in &n.ports {
                if p.screen.distance(pos_screen) <= capture
                    && !port_blocked(ni, p.screen)
                    && !port_blocked(ni, pos_screen)
                {
                    return Hit::Port(p.pref, p.dir);
                }
            }
        }
        // Cable nodes (operator round 4, D15): a wire's trim handle outranks its re-patch
        // grabs — the handle is DRAWN furniture at the arc midpoint, while the grabs are
        // invisible until you know they are there — and it obeys the same visibility
        // discipline: never under a card body (wires draw under every card), never at Dot
        // LOD (nothing of the wire is drawn to scale there). Only a wire that CARRIES a trim
        // offers the handle; the clean wire's hover ghost is the painter's, and its tap
        // rides the Wire arm in `interact`.
        for w in &layout.wires {
            if w.trim.is_some()
                && w.trim_point.distance(pos_screen) <= capture
                && body_at(pos_screen).is_none()
                && body_at(w.trim_point).is_none()
            {
                return Hit::WireTrim(w.id);
            }
        }
        // Wire ends: the grab points sit clear of the ports (see WIRE_END_GRAB_OFFSET), so this
        // ring never overlaps a port capture — but it is checked first among the wire hits, and
        // before node bodies, so a dense patch keeps its ends grabbable. From wins To on the
        // degenerate short-wire midpoint (documented at the constant). Wires (and their grabs)
        // draw UNDER every card, so a press inside ANY body belongs to that card, and an end
        // whose owner is buried under a later card is skipped exactly like a covered port.
        for w in &layout.wires {
            let grabbable = |grab: Vec2, owner: crate::canvas::model::NodeId| -> bool {
                !w.is_param // control wires are not re-patched: delete and redraw
                    && grab.distance(pos_screen) <= capture
                    && body_at(pos_screen).is_none()
                    && body_at(grab).is_none()
                    && node_index(owner)
                        .map_or(true, |oi| !covered(oi, grab) && !covered(oi, pos_screen))
            };
            if grabbable(w.grab_from, w.from_node) {
                return Hit::WireEnd(w.id, WireEndSide::From);
            }
            if grabbable(w.grab_to, w.to_node) {
                return Hit::WireEnd(w.id, WireEndSide::To);
            }
        }
    }

    // Inline parameter rows (increment 6): a row under the finger is the row you edit. Full LOD
    // only — at Simplified the rows are not drawn, and what you cannot see you cannot touch.
    // The sequencer's step buttons win inside their own row: a press on a button toggles THAT
    // step, not the mask's x-mapping (operator ruling 2026-10-01 r3). The quantizer's keys win
    // inside their own well band the same way: a press on a key flips THAT pitch class, not
    // whatever row geometry the band overlaps (operator round 4, D11).
    if lod == Lod::Full {
        for n in layout.nodes.iter().rev() {
            for (k, cell) in n.step_cells.iter().enumerate() {
                if cell.contains(pos_screen) {
                    return Hit::Step(n.id, k);
                }
            }
            for (k, cell) in n.key_cells.iter().enumerate() {
                if cell.contains(pos_screen) {
                    return Hit::Key(n.id, k);
                }
            }
            for pr in &n.param_rows {
                if pr.row.contains(pos_screen) {
                    return Hit::Param(n.id, pr.index, pr.track);
                }
            }
        }
    }
    // Node bodies: iterate in reverse so a node drawn later (on top) wins an overlap.
    for n in layout.nodes.iter().rev() {
        if n.screen.contains(pos_screen) {
            return Hit::Node(n.id);
        }
    }
    let half = LAYOUT_TOUCH_WIRE_HIT_WIDTH as f32 / 2.0;
    for w in &layout.wires {
        if polyline_distance(&w.points, pos_screen) <= half {
            return Hit::Wire(w.id);
        }
    }
    Hit::Empty
}

/// The minimum distance from `p` to a polyline.
fn polyline_distance(points: &[Vec2], p: Vec2) -> f32 {
    let mut best = f32::MAX;
    for win in points.windows(2) {
        // windows(2) guarantees two elements; the direct index is not a panic risk.
        let (a, b) = (win[0], win[1]);
        best = best.min(segment_distance(a, b, p));
    }
    if points.len() == 1 {
        best = best.min(points[0].distance(p));
    }
    best
}

/// Distance from `p` to the segment `a`–`b`.
fn segment_distance(a: Vec2, b: Vec2, p: Vec2) -> f32 {
    let ab = b.sub(a);
    let len2 = ab.dot(ab);
    if len2 <= f32::EPSILON {
        return a.distance(p);
    }
    let t = ((p.sub(a)).dot(ab) / len2).clamp(0.0, 1.0);
    let proj = Vec2::new(a.x + ab.x * t, a.y + ab.y * t);
    proj.distance(p)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::canvas::model::{Graph, NodeSpec};
    use sparq_module_api::manifest::Port;
    use sparq_module_api::port::{ChannelSet, CvRange, CvRate, Direction, Multiplicity, PortType};

    fn audio(id: &str, dir: Direction, set: ChannelSet) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: PortType::Audio,
            required: false,
            channel_set: Some(set),
            channel_set_variable: false,
            cv_rate: None,
            cv_range: None,
            cv_reduce: Default::default(),
            cv_interp: Default::default(),
            event_kinds: Vec::new(),
            multiplicity: Multiplicity::Single,
            latency_contribution: 0,
        }
    }
    fn gain_spec() -> NodeSpec {
        NodeSpec::new(
            "sparq/util/gain",
            "Gain",
            vec![
                audio("in", Direction::In, ChannelSet::Stereo),
                audio("out", Direction::Out, ChannelSet::Stereo),
            ],
        )
    }
    fn gain_spec_with_param() -> NodeSpec {
        use crate::canvas::model::{ParamDesc, ParamKind};
        gain_spec().with_params(vec![ParamDesc {
            id: "gain".into(),
            name: "Gain".into(),
            kind: ParamKind::Float,
            unit: Some("ratio".into()),
            min: 0.0,
            max: 2.0,
            default: 1.0,
            options: Vec::new(),
        }])
    }
    fn view() -> Rect {
        Rect::from_min_size(Vec2::ZERO, Vec2::new(1200.0, 800.0))
    }

    #[test]
    fn a_one_row_node_clears_the_class_l_minimum() {
        let s = node_size(&gain_spec());
        let short = s.x.min(s.y);
        assert!(short >= 72.0, "node shorter side {short} must clear class-L 72 px");
    }

    #[test]
    fn inputs_left_outputs_right_and_wire_runs_between_them() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(400.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        let _ = g.op_add_wire(PortRef::new(aid, 1), PortRef::new(bid, 0));
        let cam = Camera::new();
        let layout = compute(&g, &cam, view());
        let na = layout.nodes.iter().find(|n| n.id == aid).unwrap();
        // a's output (index 1) FLOATS beside the right edge (operator ruling 2026-10-01: the
        // port offset token outside the card, never half-buried in it).
        let outp = na.ports.iter().find(|p| p.pref.index == 1).unwrap();
        assert_eq!(outp.dir, Direction::Out);
        let off = LAYOUT_CANVAS_NODE_PORT_OFFSET as f32;
        assert!(
            (outp.world.x - (0.0 + node_size(&gain_spec()).x + off)).abs() < 1e-3,
            "output floats {} px right of the card edge",
            off
        );
        let inp = na.ports.iter().find(|p| p.pref.index == 0).unwrap();
        assert!(
            (inp.world.x - (0.0 - off)).abs() < 1e-3,
            "input floats {} px left of the card edge",
            off
        );
        // the wire's first point is at a's output, last at b's input
        let w = &layout.wires[0];
        assert!((w.points[0].distance(cam.to_screen(outp.world, view()))) < 1e-2);
    }

    #[test]
    fn hit_test_prefers_a_port_over_the_node_body() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let cam = Camera::new();
        let layout = compute(&g, &cam, view());
        let na = layout.nodes.iter().find(|n| n.id == nid(&a)).unwrap();
        let outp = na.ports.iter().find(|p| p.dir == Direction::Out).unwrap();
        // ON the floating port circle: Port wins (the capture ring lives outside the card)
        assert_eq!(hit_test(&layout, outp.screen, Lod::Full), Hit::Port(outp.pref, Direction::Out));
        // just OUTSIDE the card edge, inside the capture ring: still the port
        let beside = Vec2::new(na.screen.max.x + 10.0, outp.screen.y);
        assert_eq!(hit_test(&layout, beside, Lod::Full), Hit::Port(outp.pref, Direction::Out));
        // the body centre → Node
        assert_eq!(hit_test(&layout, na.screen.center(), Lod::Full), Hit::Node(na.id));
    }

    #[test]
    fn a_port_capture_ring_never_eats_clicks_inside_its_own_card() {
        // The regression (operator report 2026-10-01): the 24 px capture of a floating port
        // reached back over the card edge and stole presses from the param row's ends — "the
        // card slider sometimes does not respond". Inside the owner's own body (or any later
        // card's) the press belongs to the card's surface, never to the port.
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec_with_param(), Vec2::ZERO);
        let cam = Camera::new();
        let layout = compute(&g, &cam, view());
        let na = layout.nodes.iter().find(|n| n.id == nid(&a)).unwrap();
        let row = na.param_rows.first().unwrap();
        // the leftmost pixel of the row, level with the input port's capture ring
        let left_end = Vec2::new(row.row.min.x + 2.0, row.row.center().y);
        assert!(
            matches!(hit_test(&layout, left_end, Lod::Full), Hit::Param(..)),
            "the row's left end is the row's, not the input port's: {:?}",
            hit_test(&layout, left_end, Lod::Full)
        );
        let right_end = Vec2::new(row.row.max.x - 2.0, row.row.center().y);
        assert!(
            matches!(hit_test(&layout, right_end, Lod::Full), Hit::Param(..)),
            "and so is its right end, over the output port's ring"
        );
    }

    #[test]
    fn ports_are_not_targetable_at_dot_lod() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let cam = Camera::new();
        let layout = compute(&g, &cam, view());
        let na = layout.nodes.iter().find(|n| n.id == nid(&a)).unwrap();
        let outp = na.ports.iter().find(|p| p.dir == Direction::Out).unwrap();
        let at = outp.screen;
        assert!(matches!(hit_test(&layout, at, Lod::Full), Hit::Port(..)));
        assert!(!matches!(hit_test(&layout, at, Lod::Dot), Hit::Port(..)));
    }

    #[test]
    fn a_port_covered_by_a_later_card_is_not_interactable() {
        // Operator ruling 2026-10-01: covered is untouchable. Node b is drawn AFTER node a and
        // sits on top of a's right edge — a's output port (floating 4 px right of a) is buried
        // under b's body, so the click lands on b, never on the invisible port.
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        // b covers a's output port: a is 240 wide, its port floats at x=244; b at x=200 covers
        // 200..440 — the port at 244 is under b.
        let b = g.op_add_node(gain_spec(), Vec2::new(200.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        let cam = Camera::new();
        let layout = compute(&g, &cam, view());
        let na = layout.nodes.iter().find(|n| n.id == aid).unwrap();
        let outp = na.ports.iter().find(|p| p.dir == Direction::Out).unwrap();
        // the buried port is refused — the covering card wins the hit
        assert_eq!(hit_test(&layout, outp.screen, Lod::Full), Hit::Node(bid));
        // b's OWN ports stay live (nothing covers them)
        let nb = layout.nodes.iter().find(|n| n.id == bid).unwrap();
        let bout = nb.ports.iter().find(|p| p.dir == Direction::Out).unwrap();
        assert_eq!(hit_test(&layout, bout.screen, Lod::Full), Hit::Port(bout.pref, Direction::Out));
    }

    #[test]
    fn a_wire_end_under_a_later_card_is_not_grabbable() {
        // The re-patch grab of a wire whose SOURCE port is buried under a later card follows the
        // port's rule: invisible, untouchable.
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(600.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        let _ = g.op_add_wire(PortRef::new(aid, 1), PortRef::new(bid, 0));
        // a cover card drawn last, over a's output port AND its grab point
        let c = g.op_add_node(gain_spec(), Vec2::new(160.0, -8.0));
        let cid = nid(&c);
        let cam = Camera::new();
        let layout = compute(&g, &cam, view());
        let w = &layout.wires[0];
        // the grab sits 32 px along the wire from a's output (x=244) → x=276, under c (160..400)
        assert!(!matches!(hit_test(&layout, w.grab_from, Lod::Full), Hit::WireEnd(..)));
        assert_eq!(hit_test(&layout, w.grab_from, Lod::Full), Hit::Node(cid));
    }

    #[test]
    fn spatial_audio_maps_to_the_spatial_class() {
        let p = audio("ambi", Direction::Out, ChannelSet::Ambisonics(3));
        assert_eq!(signal_class(&p), SignalClass::Spatial);
        let mono = audio("m", Direction::Out, ChannelSet::Mono);
        assert_eq!(signal_class(&mono), SignalClass::Audio);
    }

    // ------------------------------------------------- increment 3: wire-end re-patch geometry

    fn wired_layout(g: &Graph) -> CanvasLayout {
        compute(g, &Camera::new(), view())
    }

    #[test]
    fn grab_points_sit_clear_of_the_ports_and_on_the_wire() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(400.0, 0.0));
        let _ = g.op_add_wire(PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 0));
        let l = wired_layout(&g);
        let w = &l.wires[0];
        let src = w.points[0];
        let dst = w.points[w.points.len() - 1];
        let d_from = w.grab_from.distance(src);
        let d_to = w.grab_to.distance(dst);
        assert!(
            (d_from - super::WIRE_END_GRAB_OFFSET).abs() < 2.0,
            "grab_from {d_from} px from the source port"
        );
        assert!(
            (d_to - super::WIRE_END_GRAB_OFFSET).abs() < 2.0,
            "grab_to {d_to} px from the destination port"
        );
        // both grabs are ON the wire (within the polyline hit width)
        assert!(polyline_distance(&w.points, w.grab_from) < 1.0);
        assert!(polyline_distance(&w.points, w.grab_to) < 1.0);
        // and clear of the port capture circles
        assert!(d_from > LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32);
    }

    #[test]
    fn a_short_wires_grabs_converge_to_the_midpoint_without_crossing() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(240.0, 0.0)); // nodes touch: a very short wire
        let _ = g.op_add_wire(PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 0));
        let l = wired_layout(&g);
        let w = &l.wires[0];
        let total = polyline_len(&w.points);
        if total < 2.0 * super::WIRE_END_GRAB_OFFSET {
            let mid = walk_along(&w.points, total * 0.5, true);
            assert!(w.grab_from.distance(mid) < 2.0, "from-grab clamped to the midpoint");
            assert!(w.grab_to.distance(mid) < 2.0, "to-grab clamped to the midpoint");
        }
    }

    #[test]
    fn hit_test_ranks_port_over_wire_end_over_body_over_wire() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(400.0, 0.0));
        let _ = g.op_add_wire(PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 0));
        let l = wired_layout(&g);
        let w = &l.wires[0];
        // exactly on the port → Port wins (a new wire is drawn, not a re-patch)
        assert_eq!(
            hit_test(&l, w.points[0], Lod::Full),
            Hit::Port(PortRef::new(nid(&a), 1), Direction::Out)
        );
        // on the grab point → WireEnd
        assert_eq!(hit_test(&l, w.grab_from, Lod::Full), Hit::WireEnd(w.id, WireEndSide::From));
        assert_eq!(hit_test(&l, w.grab_to, Lod::Full), Hit::WireEnd(w.id, WireEndSide::To));
        // wire midpoint → the body of the wire
        let mid = w.points[w.points.len() / 2];
        assert_eq!(hit_test(&l, mid, Lod::Full), Hit::Wire(w.id));
        // …and at Dot LOD the ends are not targetable (the handles are not drawn)
        assert!(
            !matches!(hit_test(&l, w.grab_from, Lod::Dot), Hit::WireEnd(..)),
            "wire ends are gated by LOD like ports"
        );
    }

    fn nid(op: &crate::canvas::model::Op) -> crate::canvas::model::NodeId {
        match op {
            crate::canvas::model::Op::AddNode(n) => n.id,
            _ => unreachable!(),
        }
    }

    // ------------------------------------------- round 4: mult strip, cable nodes, keyboard

    fn wid_of(op: &crate::canvas::model::Op) -> WireId {
        match op {
            crate::canvas::model::Op::AddWire(w) => w.id,
            _ => unreachable!(),
        }
    }

    /// The junction bus as its manifest declares it: six dots, all `in` (the connect rules
    /// make them bidirectional; the manifest's ports are statically directed).
    fn mult_spec() -> NodeSpec {
        NodeSpec::new(
            crate::canvas::MULT_ID,
            "Mult",
            (1..=6).map(|i| audio(&format!("d{i}"), Direction::In, ChannelSet::Stereo)).collect(),
        )
    }

    fn cvp(id: &str, dir: Direction) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: PortType::Cv,
            required: false,
            channel_set: None,
            channel_set_variable: false,
            cv_rate: Some(CvRate::Block),
            cv_range: Some(CvRange::Bipolar),
            cv_reduce: Default::default(),
            cv_interp: Default::default(),
            event_kinds: Vec::new(),
            multiplicity: Multiplicity::Single,
            latency_contribution: 0,
        }
    }

    /// A quantizer card: `scale` (0 = Custom) then `custom-mask` — the param order the D11
    /// manifest declares; `with_mask: false` builds the crippled card the no-route rule needs.
    fn quant_spec(with_mask: bool) -> NodeSpec {
        use crate::canvas::model::{ParamDesc, ParamKind};
        let mut params = vec![ParamDesc {
            id: "scale".into(),
            name: "Scale".into(),
            kind: ParamKind::Int,
            unit: None,
            min: 0.0,
            max: 14.0,
            default: 0.0,
            options: Vec::new(),
        }];
        if with_mask {
            params.push(ParamDesc {
                id: "custom-mask".into(),
                name: "Custom Scale".into(),
                kind: ParamKind::Int,
                unit: None,
                min: 0.0,
                max: 4095.0,
                default: 0.0,
                options: Vec::new(),
            });
        }
        NodeSpec::new(
            crate::canvas::QUANT_ID,
            "Quant",
            vec![cvp("pitch", Direction::In), cvp("pitch", Direction::Out)],
        )
        .with_params(params)
    }

    #[test]
    fn the_mult_strip_is_quarter_width_with_one_centred_dot_column() {
        let mut g = Graph::new();
        let m = g.op_add_node(mult_spec(), Vec2::ZERO);
        let mid = nid(&m);
        let layout = compute(&g, &Camera::new(), view());
        let nm = layout.nodes.iter().find(|n| n.id == mid).unwrap();
        // D6: a quarter of the default width — a strip of dots, not a card.
        let want_w = LAYOUT_CANVAS_NODE_WIDTH_DEFAULT as f32 * MULT_WIDTH_FACTOR;
        assert!((nm.world.width() - want_w).abs() < 1e-3, "strip width: {:?}", nm.world);
        assert!(
            (node_size(&mult_spec()).x - want_w).abs() < 1e-3,
            "node_size agrees — zoom-to-fit and the marquee read the same strip"
        );
        // ALL SIX dots in ONE column on the strip's centre — no left/right split (every dot
        // is in and out at once, so the split would be a lie the geometry tells).
        assert_eq!(nm.ports.len(), 6);
        let centre_x = nm.world.center().x;
        for (k, p) in nm.ports.iter().enumerate() {
            assert!((p.world.x - centre_x).abs() < 1e-3, "dot {k} sits on the centre line");
            let want_y = nm.world.min.y
                + LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32
                + k as f32 * LAYOUT_CANVAS_NODE_PORT_ROW as f32
                + LAYOUT_CANVAS_NODE_PORT_ROW as f32 / 2.0;
            assert!((p.world.y - want_y).abs() < 1e-3, "dot {k} rides the row stack");
            assert_eq!(p.dir, Direction::In, "the manifest's word stays the manifest's word");
            assert_eq!(p.pref.index, k, "spec order down the column");
        }
    }

    #[test]
    fn trim_point_is_the_arc_midpoint_and_rides_the_restyle() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(400.0, 0.0));
        let wid = wid_of(&g.op_add_wire(PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 0)));
        let l = compute(&g, &Camera::new(), view());
        let w = l.wires.iter().find(|w| w.id == wid).unwrap();
        assert_eq!(w.trim, None, "a fresh wire runs clean");
        // The half-length walk — not the middle sample index — is the definition.
        let half = walk_along(&w.points, polyline_len(&w.points) * 0.5, true);
        assert!(w.trim_point.distance(half) < 1e-3, "trim_point IS the arc midpoint");
        // On this straight run the middle sample agrees, and the point is ON the cable.
        let mid = w.points[w.points.len() / 2];
        assert!(w.trim_point.distance(mid) < 1.0);
        assert!(polyline_distance(&w.points, w.trim_point) < 1.0);
        // The restyle recomputes it: the handle never sits off the wire you see.
        let mut straight = l.clone();
        straight.make_wires_straight();
        let ws = straight.wires.iter().find(|w| w.id == wid).unwrap();
        assert!(
            ws.trim_point.distance(w.trim_point) < 1.0,
            "a straight run between the same ends keeps the midpoint"
        );
        assert!(polyline_distance(&ws.points, ws.trim_point) < 1e-3, "on the restyled cable");
        // The trim record rides the wire into the layout (the painter and the hit-test read
        // the model, never re-derive it).
        g.op_set_trim(wid, Some(WireTrim::identity())).unwrap();
        let l2 = compute(&g, &Camera::new(), view());
        assert_eq!(
            l2.wires.iter().find(|w| w.id == wid).unwrap().trim,
            Some(WireTrim::identity()),
            "the inserted node is published"
        );
    }

    #[test]
    fn a_trim_handle_wins_its_capture_dies_under_a_card_and_needs_a_trim() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(400.0, 0.0));
        let wid = wid_of(&g.op_add_wire(PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 0)));
        let cam = Camera::new();
        // Clean wire: the midpoint is plain wire — the handle is offered only while a trim
        // exists (what is not drawn is not touchable).
        let l = compute(&g, &cam, view());
        let tp = l.wires[0].trim_point;
        assert_eq!(hit_test(&l, tp, Lod::Full), Hit::Wire(wid));
        // Trimmed: the same pixel is the cable node…
        g.op_set_trim(wid, Some(WireTrim::identity())).unwrap();
        let l = compute(&g, &cam, view());
        assert_eq!(hit_test(&l, tp, Lod::Full), Hit::WireTrim(wid));
        // …out to the capture radius, and no further.
        let inside = Vec2::new(tp.x, tp.y + LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32 - 2.0);
        assert_eq!(hit_test(&l, inside, Lod::Full), Hit::WireTrim(wid));
        let outside = Vec2::new(tp.x, tp.y + LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32 + 6.0);
        assert_ne!(hit_test(&l, outside, Lod::Full), Hit::WireTrim(wid));
        // At Dot LOD nothing of the wire is drawn to scale, so the handle is not targetable.
        assert_ne!(hit_test(&l, tp, Lod::Dot), Hit::WireTrim(wid));
        // A card drawn over it kills it: wires and their furniture draw under every card, and
        // what you cannot see you cannot touch.
        let cid = nid(&g.op_add_node(gain_spec(), Vec2::new(tp.x - 60.0, 0.0)));
        let l = compute(&g, &cam, view());
        assert_eq!(hit_test(&l, tp, Lod::Full), Hit::Node(cid), "the covering card wins");
    }

    #[test]
    fn a_trim_handle_outranks_the_repatch_grab_inside_its_capture() {
        // A ~100 px wire: the grabs sit 32 px in from the ports, the handle at 50 — inside
        // each other's 24 px capture. The rank is the ruling: the DRAWN handle wins the
        // invisible grab.
        let mut g = Graph::new();
        let a = g.op_add_node(gain_spec(), Vec2::ZERO);
        let b = g.op_add_node(gain_spec(), Vec2::new(348.0, 0.0));
        let wid = wid_of(&g.op_add_wire(PortRef::new(nid(&a), 1), PortRef::new(nid(&b), 0)));
        let cam = Camera::new();
        let l = compute(&g, &cam, view());
        let w = l.wires.iter().find(|w| w.id == wid).unwrap();
        let (tp, grab) = (w.trim_point, w.grab_from);
        assert!(
            tp.distance(grab) <= LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32,
            "the test's premise: overlapping captures ({tp:?} vs {grab:?})"
        );
        // Clean wire: the grab rules its ring, midpoint included.
        assert_eq!(hit_test(&l, grab, Lod::Full), Hit::WireEnd(wid, WireEndSide::From));
        assert_eq!(hit_test(&l, tp, Lod::Full), Hit::WireEnd(wid, WireEndSide::From));
        // Trimmed: the handle outranks the grab everywhere their captures overlap.
        g.op_set_trim(wid, Some(WireTrim::identity())).unwrap();
        let l = compute(&g, &cam, view());
        assert_eq!(hit_test(&l, tp, Lod::Full), Hit::WireTrim(wid));
        assert_eq!(
            hit_test(&l, grab, Lod::Full),
            Hit::WireTrim(wid),
            "even on the grab's own pixel"
        );
    }

    #[test]
    fn quant_key_cells_tile_the_well_band_and_route_to_the_mask_param() {
        let mut g = Graph::new();
        let q = g.op_add_node(quant_spec(true), Vec2::ZERO);
        let qid = nid(&q);
        let cam = Camera::new();
        let l = compute(&g, &cam, view());
        let nq = l.nodes.iter().find(|n| n.id == qid).unwrap();
        let band = nq.well_band.expect("the keyboard well band is reserved");
        assert_eq!(nq.key_cells.len(), 12, "twelve pitch classes");
        assert_eq!(nq.key_param, Some(1), "the keys edit `custom-mask` (param 1)");
        // Equal cells tiling the band left→right = class 0→11, with the 15 % vertical inset.
        let w = band.width() / 12.0;
        for (k, cell) in nq.key_cells.iter().enumerate() {
            assert!((cell.min.x - (band.min.x + k as f32 * w)).abs() < 1e-3, "cell {k} x");
            assert!((cell.width() - w).abs() < 1e-3, "cell {k} is an equal slice");
            assert!(
                (cell.min.y - (band.min.y + band.height() * 0.15)).abs() < 1e-3,
                "cell {k} inset top"
            );
            assert!(
                (cell.max.y - (band.max.y - band.height() * 0.15)).abs() < 1e-3,
                "cell {k} inset bottom"
            );
        }
        assert!((nq.key_cells[11].max.x - band.max.x).abs() < 1e-3, "the tiling ends at the band");
        // The hit routes a key centre to its class — at Full LOD only (Simplified draws no
        // keys, and what you cannot see you cannot touch).
        let c5 = nq.key_cells[5].center();
        assert_eq!(hit_test(&l, c5, Lod::Full), Hit::Key(qid, 5));
        assert_eq!(
            hit_test(&l, c5, Lod::Simplified),
            Hit::Node(qid),
            "no keys without the drawing"
        );
        // Every other node: no cells, no param — the keyboard is the quantizer's alone.
        let mut g2 = Graph::new();
        let gg = g2.op_add_node(gain_spec(), Vec2::ZERO);
        let l2 = compute(&g2, &cam, view());
        let ng = l2.nodes.iter().find(|n| n.id == nid(&gg)).unwrap();
        assert!(ng.key_cells.is_empty() && ng.key_param.is_none());
        // A quant card WITHOUT the mask param routes no taps (never to a param that is not
        // there — the step_cells discipline).
        let mut g3 = Graph::new();
        let q3 = g3.op_add_node(quant_spec(false), Vec2::ZERO);
        let l3 = compute(&g3, &cam, view());
        let nq3 = l3.nodes.iter().find(|n| n.id == nid(&q3)).unwrap();
        assert!(nq3.key_cells.is_empty(), "no mask param, no cells");
        assert_eq!(nq3.key_param, None);
    }
}

#[cfg(test)]
mod instrument_sizing_tests {
    //! WO-020 INC4 §8.2/D6: the instrument card is its display plus chrome — the numbers the plan
    //! declares, reproduced by formula (2208 × 1288 at the Observatory's min_size).
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::canvas::model::InstrumentDisplay;

    fn obs_spec() -> NodeSpec {
        NodeSpec::new("dat/observatory", "The Observatory", vec![])
            .with_display(InstrumentDisplay { min_w: 2176.0, min_h: 1120.0, panel_rows: 2 })
    }

    #[test]
    fn the_d6_card_numbers_are_reproduced() {
        let size = node_size(&obs_spec());
        assert_eq!(size.x, 2208.0, "2176 + 2×16 gutter (D6)");
        assert_eq!(size.y, 1288.0, "1120 + 32 header + 104 panel band + 32 gutter (D6)");
    }

    #[test]
    fn the_card_ceiling_governs_not_the_backbone_grid() {
        // A hypothetical wider display clamps at node_width_instrument_max (2400), never at the
        // backbone's 480 — the wall class is a different animal (layout.toml's own words).
        let wide = NodeSpec::new("dat/x", "X", vec![]).with_display(InstrumentDisplay {
            min_w: 4000.0,
            min_h: 100.0,
            panel_rows: 0,
        });
        assert_eq!(node_size(&wide).x, 2400.0);
        // And a backbone spec is untouched by the instrument path.
        let back = NodeSpec::new("sparq/util/gain", "Gain", vec![]);
        assert!(node_size(&back).x < 500.0, "backbone cards keep the token width");
    }

    #[test]
    fn the_panel_band_is_rows_times_the_touch_floor_plus_gaps() {
        assert_eq!(instrument_panel_band(0), 0.0);
        assert_eq!(instrument_panel_band(1), 44.0);
        assert_eq!(instrument_panel_band(2), 104.0, "2×44 + 16 (the §5.2 two-row toolbar)");
    }

    #[test]
    fn the_display_band_is_the_declared_min_size_inside_the_card() {
        let card = Rect::from_min_size(Vec2::new(100.0, 50.0), node_size(&obs_spec()));
        let band =
            instrument_display_band(card, &obs_spec()).expect("an instrument card has a band");
        assert_eq!(band.width(), 2176.0, "the band is exactly the display width");
        assert_eq!(band.height(), 1120.0, "…and height");
        assert_eq!(band.min.x, 116.0, "one gutter in from the card edge");
        // A backbone node has no band.
        assert!(
            instrument_display_band(card, &NodeSpec::new("sparq/util/gain", "G", vec![])).is_none()
        );
    }

    #[test]
    fn the_browser_groups_instruments_first_under_their_own_header() {
        use crate::canvas::browser::{sections, BrowserItem};
        let inst = BrowserItem::new(obs_spec());
        let back = BrowserItem::new(NodeSpec::new("sparq/util/gain", "Gain", vec![]));
        let [a, b] = sections(vec![back.clone(), inst.clone()]);
        assert_eq!(a.0, "INSTRUMENTS");
        assert_eq!(a.1, vec![inst], "the instrument section carries the instrument");
        assert_eq!(b.0, "MODULES");
        assert_eq!(b.1, vec![back]);
    }

    #[test]
    fn a_spec_parses_from_raw_manifest_text_with_its_display_face() {
        // The discovery door: raw bytes in, spec with the D6 face out. The subject is the
        // CHECKED-IN package manifest — the same file the validator and the D5 proofs read, so
        // the sizing test cannot drift from what ships.
        let toml = include_str!("../../../../instruments/observatory/sparqmod.toml");
        let spec = NodeSpec::from_manifest_text(toml).expect("the shipped manifest parses");
        assert_eq!(spec.layer, sparq_module_api::manifest::Layer::Instrument);
        let d = spec.display.expect("the display face is parsed");
        assert_eq!((d.min_w, d.min_h, d.panel_rows), (2176.0, 1120.0, 2), "§5.1/§5.2 numbers");
        assert_eq!(node_size(&spec), Vec2::new(2208.0, 1288.0), "the D6 card");
        // A bad manifest never spawns.
        assert!(NodeSpec::from_manifest_text("[identity]\n").is_none());
    }
}
