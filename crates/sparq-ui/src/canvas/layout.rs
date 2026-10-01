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
use crate::canvas::model::{Graph, Node, NodeSpec, PortRef, WireId};
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

/// The node card's world size: header + param band + well band + info band + port band (the
/// increment-6 anatomy). With one port row and no params/well this is the old 80 px box, whose
/// shorter side clears the class-L minimum — the audit property the original size was chosen for.
#[must_use]
pub fn node_size(spec: &NodeSpec) -> Vec2 {
    let inputs = spec.inputs().count();
    let outputs = spec.outputs().count();
    let rows = inputs.max(outputs).max(1);
    let w = LAYOUT_CANVAS_NODE_WIDTH_DEFAULT as f32;
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
    let (shown, overflow) = param_split(&n.spec);
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
    let well_band = well_h.map(|wh| {
        let y0 = n.pos.y + header + param_h;
        let r = Rect::new(Vec2::new(n.pos.x, y0), Vec2::new(n.pos.x + size.x, y0 + wh));
        Rect::new(camera.to_screen(r.min, view), camera.to_screen(r.max, view))
    });
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

    // Inputs down the left edge, outputs down the right, each in its own row stack — the stack
    // starts BELOW the param and well bands (the card's anatomy, increment 6). The circles
    // FLOAT beside the window (operator ruling 2026-10-01): the port offset token outside the
    // card edge, so a port is never half-buried in the body it belongs to.
    let off = LAYOUT_CANVAS_NODE_PORT_OFFSET as f32;
    let info_h = info_band_height(&n.spec).unwrap_or(0.0);
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

/// What a screen point lands on. Ports beat wire ends beat node bodies beat wires, so a finger
/// near an edge port wires rather than selects the node — and a finger on a wire's grab point
/// re-patches rather than moves whatever is behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    /// A port (with its direction), within the capture radius.
    Port(PortRef, Direction),
    /// A wire's re-patch grab point (which wire, which end), within the capture radius.
    WireEnd(WireId, WireEndSide),
    /// An inline parameter row on a node card (which node, which param, the slider track) —
    /// the row under the finger is the row you edit (increment 6).
    Param(crate::canvas::model::NodeId, usize, Rect),
    /// A step button of the sequencer's 16-button row (which node, which step) — the button
    /// under the finger toggles that step's bit (operator ruling 2026-10-01 r3).
    Step(crate::canvas::model::NodeId, usize),
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
    /// the painter keep agreeing — what you see is what you can grab.
    pub fn make_wires_straight(&mut self) {
        for w in &mut self.wires {
            let (Some(&first), Some(&last)) = (w.points.first(), w.points.last()) else {
                continue;
            };
            w.points = vec![first, last];
            let (gf, gt) = grab_points(&w.points);
            w.grab_from = gf;
            w.grab_to = gt;
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
    // step, not the mask's x-mapping (operator ruling 2026-10-01 r3).
    if lod == Lod::Full {
        for n in layout.nodes.iter().rev() {
            for (k, cell) in n.step_cells.iter().enumerate() {
                if cell.contains(pos_screen) {
                    return Hit::Step(n.id, k);
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
    use sparq_module_api::port::{ChannelSet, Direction, Multiplicity, PortType};

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
}
