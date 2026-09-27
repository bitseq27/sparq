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

use std::collections::BTreeMap;

use crate::canvas::model::{Graph, NodeId, WireId};

/// Per-node signal levels for one frame, keyed by canvas node id, each in `0.0..=1.0`. The bridge
/// fills this from the executor's meters (see `sparq-app`'s `bridge::node_levels`); the canvas
/// maps it onto wires and the painter reads it. `BTreeMap` (not `HashMap`) so iteration — and any
/// evidence line printed from it — is deterministic, the discipline the determinism harness holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeLevels {
    map: BTreeMap<NodeId, f32>,
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
        // A NaN level would poison the painter's colour math; treat it as at-rest rather than
        // propagating it (a meter that reads NaN is a separate defect, not this module's to hide).
        let v = if level.is_finite() { level.clamp(0.0, 1.0) } else { 0.0 };
        self.map.insert(node, v);
    }

    /// A node's level this frame; `0.0` when it has none (at rest).
    #[must_use]
    pub fn get(&self, node: NodeId) -> f32 {
        self.map.get(&node).copied().unwrap_or(0.0)
    }

    /// Whether any levels were supplied (the painter skips the modulation entirely when the patch
    /// has never rendered, so wires draw exactly as they did before this increment).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// How many nodes carry a level (evidence for tests and the shell log).
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// The hottest level in the set (0.0 when empty) — the one-number summary a smoke or a log line
    /// wants: "is anything carrying signal right now?".
    #[must_use]
    pub fn max_level(&self) -> f32 {
        self.map.values().copied().fold(0.0f32, f32::max)
    }
}

/// The level a wire carries: its SOURCE node's level (a wire carries the signal its source
/// published). `0.0` for an unknown wire or a source with no published level — a wire at rest.
#[must_use]
pub fn wire_level(graph: &Graph, levels: &NodeLevels, wire: WireId) -> f32 {
    graph.wire(wire).map(|w| levels.get(w.from.node)).unwrap_or(0.0)
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
}
