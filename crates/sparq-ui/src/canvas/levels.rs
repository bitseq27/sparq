//! Live wire levels (WO-013 increment 4): the per-wire signal level the painter modulates by —
//! the "signature sparq image" the increment-3 park note named.
//!
//! Two rules shape this module, and both are the reason it exists as its own file rather than a
//! few lines in the painter:
//!
//! 1. **Computed, not drawn.** Like the rest of `sparq-ui::canvas`, this owns the pure mapping
//!    (per-node levels → per-wire levels) so the egui painter just reads a number and never
//!    invents one. Swapping the renderer replaces the drawing, not this.
//! 2. **Not faked.** The level SOURCE is the executor's meters (peak/rms across a node's audio
//!    outputs), read by the bridge after a real render — never a value synthesised from canvas
//!    data. The park note was explicit: wires drew in class colour at rest *until the executor
//!    taps existed*, because animating them from anything but the real signal would be a lie in
//!    motion. The taps exist now (WO-008: `Executor::meter`, and the cross-thread
//!    `SharedEngine::read_meters`), so the bridge feeds this from them and this maps it to wires.
//!
//! A wire carries the signal its SOURCE published, so its level is its source node's level. A
//! node with no published level yet (nothing rendered, or a node whose outputs are all silent)
//! reads 0.0 — a wire at rest, which is the honest state before the first block.
//!
//! **Per-port levels (increment 5)** refine that rule where the source publishes per port: a
//! wire whose source port carries its own published level takes THAT level, and only falls back
//! to the node's folded level when the port has none. This is what lights `cv` wires from their
//! own values — increment 4's declared limit was a node meter folding its AUDIO outputs, so a
//! cv-only source (`mod/lfo`, `env/ad`, `ana/rms`) read at rest; the bridge now also publishes
//! each cv output port's real value (magnitude for a bipolar swing) and the cv wire lights from
//! the signal it actually carries. Audio PORT levels (and the ring extension that would carry
//! them live) remain the parked item — an audio wire keeps the increment-4 semantics exactly:
//! its source node's folded meter.

use std::collections::BTreeMap;

use crate::canvas::model::{Graph, NodeId, WireId};

/// One stereo meter reading for the live bars (WO-012 increment 4): the two channel peaks and
/// their peak-hold positions, all in `0.0..=1.0`. The hold decays on AUDIO time (block counts
/// at the negotiated rate — no wall clock on the display path), half-life `motion.toml`'s
/// 300 ms expressed in blocks.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StereoMeter {
    /// Left (or mono) channel peak this block.
    pub l: f32,
    /// Right channel peak (a mono port duplicates `l`).
    pub r: f32,
    /// Peak-hold position, left.
    pub hold_l: f32,
    /// Peak-hold position, right.
    pub hold_r: f32,
    /// The CLIP latch, left (operator round 4, D2): set the block a peak reaches 1.0 and
    /// HELD until the session ends — the peak-hold's own state machinery hosts it, so it
    /// decays never and forgets never; a fresh session starts unlatched by construction.
    pub clip_l: bool,
    /// The clip latch, right (D2).
    pub clip_r: bool,
    /// The block the peaks were taken at — the hold decays on AUDIO time (block distance),
    /// never on a wall clock the display path could lie about.
    pub block: u64,
}

/// The live per-(node, output port) stereo meters the session drains per frame — the single
/// source the painter's meter bars read (the `ScopeTraces` discipline: at rest the shell hands
/// the painter an empty map and the wells stay empty). `BTreeMap` for deterministic iteration.
pub type LiveMeters =
    std::collections::BTreeMap<(crate::canvas::model::NodeId, usize), StereoMeter>;

/// Per-node signal levels for one frame, keyed by canvas node id, each in `0.0..=1.0`. The bridge
/// fills this from the executor's meters (see `sparq-app`'s `bridge::node_levels`); the canvas
/// maps it onto wires and the painter reads it. `BTreeMap` (not `HashMap`) so iteration — and any
/// evidence line printed from it — is deterministic, the discipline the determinism harness holds.
///
/// Alongside the per-node levels (a node's folded audio meter), the set carries optional PER-PORT
/// levels keyed by `(node, output port index)` — the level a specific port published, which a
/// wire out of that port prefers over the node's fold. The bridge publishes cv output ports
/// today; the map is the shape the live-HAL increment fills from the rings (which can carry port
/// ids) without touching this model again.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeLevels {
    map: BTreeMap<NodeId, f32>,
    ports: BTreeMap<(NodeId, usize), f32>,
}

impl NodeLevels {
    /// An empty set: every wire reads at rest (0.0) until levels are supplied.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a node's level for this frame, clamped into `0.0..=1.0` (a meter can momentarily
    /// exceed unity on a transient; the visual range is bounded even when the signal is not).
    pub fn set(&mut self, node: NodeId, level: f32) {
        self.map.insert(node, sanitised(level));
    }

    /// Record the level ONE OUTPUT PORT published (increment 5), with the same clamp and
    /// NaN-to-rest rule as [`Self::set`] — one sanitising rule for both maps.
    pub fn set_port(&mut self, node: NodeId, port: usize, level: f32) {
        self.ports.insert((node, port), sanitised(level));
    }

    /// A node's level this frame; `0.0` when it has none (at rest).
    #[must_use]
    pub fn get(&self, node: NodeId) -> f32 {
        self.map.get(&node).copied().unwrap_or(0.0)
    }

    /// The level one output port published, or `None` when this set carries no per-port level
    /// for it — [`wire_level`]'s fallback switch: `None` means "read the node's fold", NOT
    /// "at rest", so an unpublished audio port keeps the increment-4 semantics exactly.
    #[must_use]
    pub fn port(&self, node: NodeId, port: usize) -> Option<f32> {
        self.ports.get(&(node, port)).copied()
    }

    /// Whether any levels were supplied (the painter skips the modulation entirely when the patch
    /// has never rendered, so wires draw exactly as they did before this increment).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty() && self.ports.is_empty()
    }

    /// How many nodes carry a level (evidence for tests and the shell log).
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// How many output ports carry their own level (evidence for tests and the shell log).
    #[must_use]
    pub fn port_len(&self) -> usize {
        self.ports.len()
    }

    /// The hottest level in the set (0.0 when empty) — the one-number summary a smoke or a log line
    /// wants: "is anything carrying signal right now?". Spans both maps: a hot cv port on an
    /// otherwise audio-silent node IS signal flowing.
    #[must_use]
    pub fn max_level(&self) -> f32 {
        self.map.values().chain(self.ports.values()).copied().fold(0.0f32, f32::max)
    }
}

/// A NaN level would poison the painter's colour math; treat it as at-rest rather than
/// propagating it (a meter that reads NaN is a separate defect, not this module's to hide).
fn sanitised(level: f32) -> f32 {
    if level.is_finite() {
        level.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// How many frames of level history the rolling graph carries (operator round 4, D13):
/// ≈4 s at the 60 fps nominal cadence. A FRAME count, not a time constant — the display
/// samples once per frame, and the window it shows is declared in the same unit.
pub const LEVEL_HISTORY_FRAMES: usize = 240;

/// One node's rolling level history (operator round 4, D13): the `Well::Graph` trace's model,
/// pushed once per live frame by the shell from the drained level publication. DISPLAY-SIDE
/// ONLY — it never enters the engine, the journal or a save (a restart starts empty, which is
/// the declared rule, not a gap); bounded, so a long session cannot grow it; sanitised at the
/// door like every level (`sanitised`), so a NaN frame cannot poison the polyline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LevelHistory {
    samples: std::collections::VecDeque<f32>,
}

impl LevelHistory {
    /// An empty history — the at-rest and the restart shape.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Push one frame's level word. Bounded at [`LEVEL_HISTORY_FRAMES`]: the oldest sample
    /// leaves as the new one enters (the window ROLLS, it does not grow).
    pub fn push(&mut self, level: f32) {
        if self.samples.len() >= LEVEL_HISTORY_FRAMES {
            self.samples.pop_front();
        }
        self.samples.push_back(sanitised(level));
    }

    /// The samples, oldest first.
    pub fn samples(&self) -> impl Iterator<Item = f32> + '_ {
        self.samples.iter().copied()
    }

    /// How many frames the window currently holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the window is empty (the painter draws the rest well, never a faked trace).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Drop the window (a restart, a session start — "restart starts empty", declared).
    pub fn clear(&mut self) {
        self.samples.clear();
    }

    /// The trace polyline inside `r` (y-up: level 1 at the top). The x-mapping is the rolling
    /// window's own: one slot per frame of the FULL window — a young history GROWS from the
    /// left edge toward the right, and a full one SCROLLS (the oldest sample leaves the left
    /// edge as the newest enters at the right). Fewer than two samples: no line (a single
    /// reading is a bar, not a trace).
    #[must_use]
    pub fn polyline(&self, r: crate::geom::Rect) -> Vec<crate::geom::Vec2> {
        use crate::geom::Vec2;
        let n = self.samples.len();
        if n < 2 {
            return Vec::new();
        }
        let slots = (LEVEL_HISTORY_FRAMES - 1).max(1) as f32;
        self.samples
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                Vec2::new(
                    r.min.x + r.width() * i as f32 / slots,
                    r.max.y - r.height() * v.clamp(0.0, 1.0),
                )
            })
            .collect()
    }
}

/// The per-node rolling histories the shell owns and the painter reads (the `ScopeTraces`
/// discipline: at rest the shell hands over an empty map and the wells stay empty). `BTreeMap`
/// for deterministic iteration.
pub type LevelHistories = std::collections::BTreeMap<crate::canvas::model::NodeId, LevelHistory>;

/// The level a wire carries: its SOURCE PORT's own level when one is published (a cv wire lights
/// from the value its source publishes on that port), else its SOURCE NODE's folded level (the
/// increment-4 rule: a wire carries the signal its source published). `0.0` for an unknown wire
/// or a source with no published level — a wire at rest.
#[must_use]
pub fn wire_level(graph: &Graph, levels: &NodeLevels, wire: WireId) -> f32 {
    let Some(w) = graph.wire(wire) else { return 0.0 };
    // The junction bus (operator ruling 2026-10-01 r3): a wire leaving a mult dot carries
    // the SOURCE's level — the dot copies, so the copy lights exactly like the original.
    let src = if graph.node(w.from.node).is_some_and(|n| n.spec.module_id == crate::canvas::MULT_ID)
    {
        graph
            .wires()
            .iter()
            .find(|f| f.to.node == w.from.node && f.param.is_none())
            .map(|f| f.from)
            .unwrap_or(w.from)
    } else {
        w.from
    };
    levels.port(src.node, src.index).unwrap_or_else(|| levels.get(src.node))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::canvas::model::{Graph, NodeSpec, PortRef};
    use crate::geom::Vec2;
    use sparq_module_api::manifest::Port;
    use sparq_module_api::port::{ChannelSet, Direction, Multiplicity, PortType};

    fn audio(id: &str, dir: Direction) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: PortType::Audio,
            required: false,
            channel_set: Some(ChannelSet::Mono),
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
    fn src() -> NodeSpec {
        NodeSpec::new("sparq/syn/sine", "Sine", vec![audio("out", Direction::Out)])
    }
    fn dst() -> NodeSpec {
        NodeSpec::new("sparq/util/gain", "Gain", vec![audio("in", Direction::In)])
    }
    fn nid(op: &crate::canvas::model::Op) -> NodeId {
        match op {
            crate::canvas::model::Op::AddNode(n) => n.id,
            _ => panic!("expected an add-node op"),
        }
    }
    fn wid(op: &crate::canvas::model::Op) -> WireId {
        match op {
            crate::canvas::model::Op::AddWire(w) => w.id,
            _ => panic!("expected an add-wire op"),
        }
    }

    #[test]
    fn a_wire_takes_its_source_node_level() {
        let mut g = Graph::new();
        let a = g.op_add_node(src(), Vec2::ZERO);
        let b = g.op_add_node(dst(), Vec2::new(400.0, 0.0));
        let (aid, bid) = (nid(&a), nid(&b));
        let w = g.op_add_wire(PortRef::new(aid, 0), PortRef::new(bid, 0));
        let wid = wid(&w);
        let mut levels = NodeLevels::new();
        assert_eq!(wire_level(&g, &levels, wid), 0.0, "no levels yet: at rest");
        levels.set(aid, 0.75);
        levels.set(bid, 0.10);
        assert_eq!(
            wire_level(&g, &levels, wid),
            0.75,
            "the wire carries its SOURCE's level, not the destination's"
        );
    }

    #[test]
    fn levels_clamp_and_reject_nan_to_rest() {
        let mut l = NodeLevels::new();
        l.set(1, 1.7);
        l.set(2, -0.3);
        l.set(3, f32::NAN);
        assert_eq!(l.get(1), 1.0, "a transient over unity clamps to the visual ceiling");
        assert_eq!(l.get(2), 0.0, "a negative level clamps to rest");
        assert_eq!(l.get(3), 0.0, "NaN reads as rest, never propagates to the colour math");
        assert_eq!(l.get(99), 0.0, "an unknown node is at rest");
        assert_eq!(l.len(), 3);
        assert_eq!(l.max_level(), 1.0, "the hottest level is the clamped-to-unity one");
        assert!(!l.is_empty());
        assert!(NodeLevels::new().is_empty());
        assert_eq!(NodeLevels::new().max_level(), 0.0, "an empty set is at rest");
    }

    #[test]
    fn an_unknown_wire_reads_at_rest() {
        let g = Graph::new();
        let levels = {
            let mut l = NodeLevels::new();
            l.set(0, 1.0);
            l
        };
        assert_eq!(wire_level(&g, &levels, 12345), 0.0, "no such wire: at rest, never a panic");
    }

    // ------------------------------------------------------------ per-port levels (increment 5)

    fn cv(id: &str, dir: Direction) -> Port {
        Port {
            port_type: PortType::Cv,
            cv_rate: Some(sparq_module_api::port::CvRate::Block),
            ..audio(id, dir)
        }
    }

    /// A source with an audio out (port 0) AND a cv out (port 1), wired to two destinations.
    fn two_port_graph() -> (Graph, WireId, WireId, NodeId) {
        let spec = NodeSpec::new(
            "sparq/ana/rms",
            "RMS",
            vec![
                audio("in", Direction::In),
                cv("level", Direction::Out),
                audio("out", Direction::Out),
            ],
        );
        let mut g = Graph::new();
        let s = g.op_add_node(spec, Vec2::ZERO);
        let d1 = g.op_add_node(dst(), Vec2::new(400.0, -100.0));
        let d2 = g.op_add_node(dst(), Vec2::new(400.0, 100.0));
        let (sid, d1id, d2id) = (nid(&s), nid(&d1), nid(&d2));
        let w_audio = wid(&g.op_add_wire(PortRef::new(sid, 2), PortRef::new(d1id, 0)));
        let w_cv = wid(&g.op_add_wire(PortRef::new(sid, 1), PortRef::new(d2id, 0)));
        (g, w_audio, w_cv, sid)
    }

    #[test]
    fn a_wire_prefers_its_source_ports_own_level_and_falls_back_to_the_node_fold() {
        let (g, w_audio, w_cv, sid) = two_port_graph();
        let mut l = NodeLevels::new();
        l.set(sid, 0.5); // the node's folded audio meter
        assert_eq!(wire_level(&g, &l, w_audio), 0.5, "no port level: the inc-4 node rule, exactly");
        assert_eq!(wire_level(&g, &l, w_cv), 0.5, "…and a cv wire falls back the same way");
        l.set_port(sid, 1, 0.35); // the cv port's own published value
        assert_eq!(wire_level(&g, &l, w_cv), 0.35, "the cv wire now lights from its OWN port");
        assert_eq!(
            wire_level(&g, &l, w_audio),
            0.5,
            "the audio wire keeps the node fold — untouched"
        );
        assert_eq!(l.port(sid, 1), Some(0.35));
        assert_eq!(l.port(sid, 2), None, "an unpublished port reports None, not 0.0");
        assert_eq!(l.port_len(), 1);
    }

    #[test]
    fn port_levels_obey_the_same_sanitising_rule_and_span_the_summary() {
        let mut l = NodeLevels::new();
        l.set_port(1, 0, 1.7);
        l.set_port(1, 1, f32::NAN);
        l.set_port(2, 0, -0.4);
        assert_eq!(l.port(1, 0), Some(1.0), "a transient over unity clamps");
        assert_eq!(l.port(1, 1), Some(0.0), "NaN reads as rest");
        assert_eq!(l.port(2, 0), Some(0.0), "negative reads as rest");
        assert_eq!(l.len(), 0, "no NODE levels were set…");
        assert!(!l.is_empty(), "…yet the port entries alone make the set non-empty");
        assert_eq!(l.max_level(), 1.0, "…and the hottest level spans BOTH maps");
        let mut nodes_only = NodeLevels::new();
        nodes_only.set(9, 0.2);
        assert_eq!(nodes_only.port_len(), 0);
        assert!(!nodes_only.is_empty());
    }
}

#[cfg(test)]
mod r4_tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;
    use crate::geom::{Rect, Vec2};

    #[test]
    fn the_clip_latch_defaults_false_and_the_hold_fields_stay_their_own_words() {
        let m = StereoMeter::default();
        assert!(!m.clip_l && !m.clip_r, "a fresh meter is unlatched");
        // The latch is data the SESSION owns (live.rs sets it, the painter reads it); the
        // model's job is the honest default and the field's place in the vocabulary.
        let hot = StereoMeter { l: 1.0, r: 0.5, clip_l: true, ..StereoMeter::default() };
        assert!(hot.clip_l && !hot.clip_r, "per-channel, never folded");
    }

    #[test]
    fn the_history_rolls_at_its_declared_window() {
        let mut h = LevelHistory::new();
        assert!(h.is_empty());
        for i in 0..(LEVEL_HISTORY_FRAMES + 50) {
            h.push((i % 100) as f32 / 100.0);
        }
        assert_eq!(h.len(), LEVEL_HISTORY_FRAMES, "bounded: the window rolls, it does not grow");
        // The oldest samples left first: the window holds the LAST cap pushes.
        let first = h.samples().next().unwrap();
        let want_first = ((LEVEL_HISTORY_FRAMES + 50 - LEVEL_HISTORY_FRAMES) % 100) as f32 / 100.0;
        assert!((first - want_first).abs() < 1e-6, "{first} vs {want_first}");
        h.clear();
        assert!(h.is_empty(), "a restart starts empty");
        assert_eq!(h.samples().count(), 0, "…and the window hands back nothing");
    }

    #[test]
    fn the_history_sanitises_at_the_door() {
        let mut h = LevelHistory::new();
        h.push(f32::NAN);
        h.push(3.5);
        h.push(-2.0);
        let v: Vec<f32> = h.samples().collect();
        assert_eq!(v, vec![0.0, 1.0, 0.0], "NaN reads rest; the window clamps to 0..1");
    }

    #[test]
    fn the_polyline_grows_from_the_left_then_scrolls_and_stays_in_the_rect() {
        let r = Rect::new(Vec2::new(10.0, 20.0), Vec2::new(110.0, 70.0)); // 100 × 50
        let mut h = LevelHistory::new();
        assert!(h.polyline(r).is_empty(), "no samples, no line");
        h.push(0.5);
        assert!(h.polyline(r).is_empty(), "one reading is a bar, not a trace");
        // A young window: the newest sample sits at its SLOT (len-1 of the full window), so
        // the trace grows from the left instead of stretching across the band.
        for i in 0..10 {
            h.push(i as f32 / 10.0);
        }
        let pts = h.polyline(r);
        assert_eq!(pts.len(), 11);
        let want_x_last = r.min.x + r.width() * 10.0 / (LEVEL_HISTORY_FRAMES - 1) as f32;
        assert!((pts.last().unwrap().x - want_x_last).abs() < 1e-2, "slot-mapped, not stretched");
        assert!(
            (pts.first().unwrap().x - r.min.x).abs() < 1e-3,
            "the oldest sample is at the left"
        );
        // A FULL window: the newest sits at the right edge, the oldest at the left — scrolling.
        let mut full = LevelHistory::new();
        for i in 0..LEVEL_HISTORY_FRAMES {
            full.push(if i % 2 == 0 { 0.25 } else { 0.75 });
        }
        let fpts = full.polyline(r);
        assert!((fpts.first().unwrap().x - r.min.x).abs() < 1e-3);
        assert!((fpts.last().unwrap().x - r.max.x).abs() < 1e-2, "the newest rides the right edge");
        // y is the level, y-up: 0.25 → three quarters down, 0.75 → a quarter down.
        let y_of = |v: f32| r.max.y - r.height() * v;
        assert!((fpts[0].y - y_of(0.25)).abs() < 1e-3);
        assert!((fpts[1].y - y_of(0.75)).abs() < 1e-3);
        assert!(fpts.iter().all(|p| p.x >= r.min.x - 1e-3 && p.x <= r.max.x + 1e-3));
    }
}
