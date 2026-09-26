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
    LAYOUT_CANVAS_NODE_HEADER_HEIGHT, LAYOUT_CANVAS_NODE_PORT_ROW,
    LAYOUT_CANVAS_NODE_WIDTH_DEFAULT, LAYOUT_SPACE_2, LAYOUT_TOUCH_PORT_CAPTURE_RADIUS,
    LAYOUT_TOUCH_WIRE_HIT_WIDTH,
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
    /// Ports, inputs then outputs, in spec order within each direction.
    pub ports: Vec<PortLayout>,
}

/// A wire, laid out as a sampled bézier polyline in screen px.
#[derive(Clone, Debug, PartialEq)]
pub struct WireLayout {
    /// Which wire.
    pub id: WireId,
    /// Signal class (from the source port).
    pub class: SignalClass,
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
#[must_use]
pub fn node_size(spec: &NodeSpec) -> Vec2 {
    let inputs = spec.inputs().count();
    let outputs = spec.outputs().count();
    let rows = inputs.max(outputs).max(1);
    let w = LAYOUT_CANVAS_NODE_WIDTH_DEFAULT as f32;
    let h =
        LAYOUT_CANVAS_NODE_HEADER_HEIGHT as f32 + rows as f32 * LAYOUT_CANVAS_NODE_PORT_ROW as f32;
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
            let to = port_world.iter().find(|(p, _, _, _)| *p == w.to)?;
            let (_, f_world, _, f_class) = *from;
            let (_, t_world, _, _) = *to;
            let f_screen = camera.to_screen(f_world, view);
            let t_screen = camera.to_screen(t_world, view);
            let points = sample_bezier(f_screen, t_screen);
            let (grab_from, grab_to) = grab_points(&points);
            // A conversion (multi→mono) is a property of the two channel sets; recompute it from
            // the source/dest ports so the painter can draw the warning hairline.
            let conversion = is_conversion(graph, w.from, w.to);
            Some(WireLayout { id: w.id, class: f_class, points, conversion, grab_from, grab_to })
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
    let mut ports = Vec::new();

    // Inputs down the left edge, outputs down the right, each in its own row stack.
    for (side, dir) in [(0.0, Direction::In), (size.x, Direction::Out)] {
        let mut r = 0usize;
        for idx in
            n.spec.ports.iter().enumerate().filter(|(_, p)| p.direction == dir).map(|(i, _)| i)
        {
            let port = match n.spec.port(idx) {
                Some(p) => p,
                None => continue,
            };
            let wx = n.pos.x + side;
            let wy = n.pos.y + header + r as f32 * row + row / 2.0;
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

    NodeLayout { id: n.id, world, screen, header_screen, ports }
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
    /// A node body.
    Node(crate::canvas::model::NodeId),
    /// A wire.
    Wire(WireId),
    /// Empty canvas.
    Empty,
}

/// Hit-test a screen point against a computed layout. `lod` gates port and wire-end hits: at
/// [`Lod::Dot`] neither is drawn at scale, so neither is targetable (a 24 px capture around an
/// invisible handle would only cause mis-wires).
#[must_use]
pub fn hit_test(layout: &CanvasLayout, pos_screen: Vec2, lod: Lod) -> Hit {
    let capture = LAYOUT_TOUCH_PORT_CAPTURE_RADIUS as f32;
    if lod != Lod::Dot {
        for n in &layout.nodes {
            for p in &n.ports {
                if p.screen.distance(pos_screen) <= capture {
                    return Hit::Port(p.pref, p.dir);
                }
            }
        }
        // Wire ends: the grab points sit clear of the ports (see WIRE_END_GRAB_OFFSET), so this
        // ring never overlaps a port capture — but it is checked first among the wire hits, and
        // before node bodies, so a dense patch keeps its ends grabbable. From wins To on the
        // degenerate short-wire midpoint (documented at the constant).
        for w in &layout.wires {
            if w.grab_from.distance(pos_screen) <= capture {
                return Hit::WireEnd(w.id, WireEndSide::From);
            }
            if w.grab_to.distance(pos_screen) <= capture {
                return Hit::WireEnd(w.id, WireEndSide::To);
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
        // a's output (index 1) is on the right edge.
        let outp = na.ports.iter().find(|p| p.pref.index == 1).unwrap();
        assert_eq!(outp.dir, Direction::Out);
        assert!(
            (outp.world.x - (0.0 + node_size(&gain_spec()).x)).abs() < 1e-3,
            "output on right edge"
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
        // a point just inside the body but within the port capture radius → Port, not Node
        let near = Vec2::new(outp.screen.x - 10.0, outp.screen.y);
        assert_eq!(hit_test(&layout, near, Lod::Full), Hit::Port(outp.pref, Direction::Out));
        // the body centre → Node
        assert_eq!(hit_test(&layout, na.screen.center(), Lod::Full), Hit::Node(na.id));
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
