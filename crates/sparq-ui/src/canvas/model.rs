//! The graph as data: nodes, wires, flags, and every mutation as an invertible [`Op`].
//!
//! Two disciplines shape this file:
//!
//! 1. **Ops are data with inverses.** Nothing mutates the [`Graph`] in place from a gesture; a
//!    gesture produces an [`Op`], the op is applied, and its [`Op::inverse`] is what undo applies.
//!    This is what makes "undo/redo restores the graph correctly for 20 random operations" a
//!    property of the representation rather than of hand-written rollback code — and it is the
//!    same shape the WO-008 journal will want (plan §5.6: graph mutation events are journal
//!    records).
//! 2. **Stable ids, never reused.** Node and wire ids are allocated once and monotonically; an
//!    undo/redo round-trip re-applies the *same* id, so a wire's endpoints stay valid across
//!    history. Gaps in the id space are harmless and preferred to reuse.
//!
//! The graph is the canvas's own truth for increment 1. When the WO-008 executor lands it becomes
//! a *view* kept in step with the engine graph; the op vocabulary is deliberately the subset the
//! executor's atomic mutation (ADR-009 decision 7) will accept.

use crate::geom::{Rect, Vec2};
use sparq_module_api::manifest::Port;

/// A node identity. Stable, never reused (see the module docs).
pub type NodeId = u32;

/// A wire identity. Stable, never reused.
pub type WireId = u32;

/// One end of a wire: a node and the index of one of its ports.
///
/// The index is into [`NodeSpec::ports`], which is fixed for the life of the node in increment 1
/// (hot reload that changes a port list is a migration, plan §6.7 — out of scope here), so an
/// index is as stable as the module id it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortRef {
    /// The node the port belongs to.
    pub node: NodeId,
    /// Index into [`NodeSpec::ports`].
    pub index: usize,
}

impl PortRef {
    /// A port reference.
    #[must_use]
    pub const fn new(node: NodeId, index: usize) -> Self {
        Self { node, index }
    }
}

/// Per-node performance flags, set from the long-press context menu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeFlags {
    /// Signal passes around this node (drawn dimmed, badged BYPASS).
    pub bypassed: bool,
    /// The node's output is silenced (badged MUTE).
    pub muted: bool,
    /// The node resists move and delete — the on-stage "don't nudge my kick chain" lock.
    pub locked: bool,
}

/// What a canvas node *is*: the identity and port vocabulary it renders and wires against.
///
/// Increment 1 carries only what the canvas needs to draw and connect. Parameters, custom panels
/// and the UI descriptor (plan §6.6) arrive with the inspector increment; the port list is the
/// validated [`Port`] vocabulary straight from `sparq-module-api`, never a copy.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    /// The module id, e.g. `sparq/syn/sine`.
    pub module_id: String,
    /// The display name, e.g. `Sine` — the node header text.
    pub display_name: String,
    /// The module's validated ports, in manifest order (inputs and outputs interleaved as
    /// declared; [`NodeSpec::inputs`]/[`NodeSpec::outputs`] filter by direction).
    pub ports: Vec<Port>,
}

impl NodeSpec {
    /// A spec from an id, a name and its ports.
    #[must_use]
    pub fn new(
        module_id: impl Into<String>,
        display_name: impl Into<String>,
        ports: Vec<Port>,
    ) -> Self {
        Self { module_id: module_id.into(), display_name: display_name.into(), ports }
    }

    /// Indices of the input ports, in manifest order.
    pub fn inputs(&self) -> impl Iterator<Item = usize> + '_ {
        self.ports_in_direction(sparq_module_api::port::Direction::In)
    }

    /// Indices of the output ports, in manifest order.
    pub fn outputs(&self) -> impl Iterator<Item = usize> + '_ {
        self.ports_in_direction(sparq_module_api::port::Direction::Out)
    }

    fn ports_in_direction(
        &self,
        dir: sparq_module_api::port::Direction,
    ) -> impl Iterator<Item = usize> + '_ {
        self.ports.iter().enumerate().filter(move |(_, p)| p.direction == dir).map(|(i, _)| i)
    }

    /// The port at `index`, if it exists.
    #[must_use]
    pub fn port(&self, index: usize) -> Option<&Port> {
        self.ports.get(index)
    }
}

/// A node instance on the canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    /// Stable identity.
    pub id: NodeId,
    /// What it is.
    pub spec: NodeSpec,
    /// Top-left corner in **world** px, always on the snap grid after a committed move.
    pub pos: Vec2,
    /// Performance flags.
    pub flags: NodeFlags,
    /// A user rename (increment 2 supplies the text entry; the field exists so [`Op::Rename`] is
    /// representable and undoable from day one).
    pub custom_name: Option<String>,
}

impl Node {
    /// The name the header shows: the rename if set, else the module's display name.
    #[must_use]
    pub fn title(&self) -> &str {
        self.custom_name.as_deref().unwrap_or(&self.spec.display_name)
    }
}

/// A directed connection from an output port to an input port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wire {
    /// Stable identity.
    pub id: WireId,
    /// The source (an `Out` port).
    pub from: PortRef,
    /// The destination (an `In` port).
    pub to: PortRef,
}

/// One graph mutation, as data. Every variant has an [`Op::inverse`].
///
/// [`Op::Batch`] is atomic: undo reverses it as a unit, in reverse order. Composite gestures
/// (delete a node and its wires; replace the wire on an occupied single input) are batches, so one
/// three-finger tap undoes the whole thing — the user's mental model is "that action", not "those
/// two edits".
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// Insert a node (id already assigned).
    AddNode(Node),
    /// Remove a node **and** the wires that were attached to it. The wires ride along so the
    /// inverse can restore them; see [`Graph::op_remove_node`], which detaches them first.
    RemoveNode {
        /// The removed node.
        node: Node,
        /// The wires that were connected to it, removed as part of the same batch.
        wires: Vec<Wire>,
    },
    /// Move one node from `from` to `to` (world px).
    MoveNode {
        /// Which node.
        id: NodeId,
        /// Previous top-left.
        from: Vec2,
        /// New top-left.
        to: Vec2,
    },
    /// Insert a wire (id already assigned).
    AddWire(Wire),
    /// Remove a wire.
    RemoveWire(Wire),
    /// Change a node's flags.
    SetFlags {
        /// Which node.
        id: NodeId,
        /// Previous flags.
        from: NodeFlags,
        /// New flags.
        to: NodeFlags,
    },
    /// Rename a node.
    Rename {
        /// Which node.
        id: NodeId,
        /// Previous custom name.
        from: Option<String>,
        /// New custom name.
        to: Option<String>,
    },
    /// A group of ops applied and undone as one unit.
    Batch(Vec<Op>),
}

impl Op {
    /// The op that undoes this one. Purely data-driven: the inverse carries everything it needs
    /// (removed nodes/wires store their own values), so it never has to consult the graph.
    #[must_use]
    pub fn inverse(&self) -> Op {
        match self {
            Self::AddNode(n) => Self::RemoveNode { node: n.clone(), wires: Vec::new() },
            Self::RemoveNode { node, wires } => {
                let mut back = vec![Self::AddNode(node.clone())];
                back.extend(wires.iter().map(|w| Self::AddWire(*w)));
                Self::Batch(back)
            },
            Self::MoveNode { id, from, to } => Self::MoveNode { id: *id, from: *to, to: *from },
            Self::AddWire(w) => Self::RemoveWire(*w),
            Self::RemoveWire(w) => Self::AddWire(*w),
            Self::SetFlags { id, from, to } => Self::SetFlags { id: *id, from: *to, to: *from },
            Self::Rename { id, from, to } => {
                Self::Rename { id: *id, from: to.clone(), to: from.clone() }
            },
            Self::Batch(ops) => Self::Batch(ops.iter().rev().map(Op::inverse).collect()),
        }
    }

    /// A short human label, for the shell's "explain yourself" log.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::AddNode(_) => "add node",
            Self::RemoveNode { .. } => "delete node",
            Self::MoveNode { .. } => "move",
            Self::AddWire(_) => "connect",
            Self::RemoveWire(_) => "disconnect",
            Self::SetFlags { .. } => "flags",
            Self::Rename { .. } => "rename",
            Self::Batch(_) => "batch",
        }
    }
}

/// The patch graph: nodes, wires, and the id counters that keep both stable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Graph {
    nodes: Vec<Node>,
    wires: Vec<Wire>,
    next_node: NodeId,
    next_wire: WireId,
}

impl Graph {
    /// An empty graph.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// All nodes, in insertion order.
    #[must_use]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// All wires, in insertion order.
    #[must_use]
    pub fn wires(&self) -> &[Wire] {
        &self.wires
    }

    /// Node count.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Wire count.
    #[must_use]
    pub fn wire_count(&self) -> usize {
        self.wires.len()
    }

    /// The node with `id`, if present.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The mutable node with `id`, if present.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    /// The wire with `id`, if present.
    #[must_use]
    pub fn wire(&self, id: WireId) -> Option<&Wire> {
        self.wires.iter().find(|w| w.id == id)
    }

    /// The port a [`PortRef`] names, if the node and index exist.
    #[must_use]
    pub fn port(&self, pref: PortRef) -> Option<&Port> {
        self.node(pref.node)?.spec.port(pref.index)
    }

    /// The wire already feeding `input` (an `In` port), if any. Used for the single-input
    /// replacement rule in [`super::connect`].
    #[must_use]
    pub fn incoming(&self, input: PortRef) -> Option<&Wire> {
        self.wires.iter().find(|w| w.to == input)
    }

    /// Every wire touching `node` (either end).
    #[must_use]
    pub fn wires_touching(&self, node: NodeId) -> Vec<Wire> {
        self.wires.iter().filter(|w| w.from.node == node || w.to.node == node).copied().collect()
    }

    /// Whether a directed path already leads from `from` to `to` through the wires. Adding an edge
    /// `to → from` would then close a cycle — the check [`super::connect`] uses to honour §5.4
    /// ("acyclic by default"; no delay module exists yet to legalise a loop).
    #[must_use]
    pub fn reaches(&self, from: NodeId, to: NodeId) -> bool {
        // Iterative DFS over wire direction (from.output → to.input). Small graphs; no allocation
        // beyond the visited set and the work stack, both bounded by node count.
        let mut stack = vec![from];
        let mut seen = std::collections::HashSet::new();
        while let Some(n) = stack.pop() {
            if n == to {
                return true;
            }
            if !seen.insert(n) {
                continue;
            }
            for w in &self.wires {
                if w.from.node == n {
                    stack.push(w.to.node);
                }
            }
        }
        false
    }

    /// The union of every node's rect in **world** px, or `None` for an empty graph. Zoom-to-fit
    /// reads this. Node sizes need the layout metrics, so the caller passes the per-node size
    /// function; keeping it here (rather than in `layout`) avoids a dependency cycle.
    #[must_use]
    pub fn world_bounds(&self, size_of: impl Fn(&Node) -> Vec2) -> Option<Rect> {
        let mut acc: Option<Rect> = None;
        for n in &self.nodes {
            let size = size_of(n);
            let r = Rect::from_min_size(n.pos, size);
            acc = Some(match acc {
                None => r,
                Some(a) => union(a, r),
            });
        }
        acc
    }

    /// Allocate the next node id (never reused).
    fn alloc_node(&mut self) -> NodeId {
        let id = self.next_node;
        self.next_node = self.next_node.wrapping_add(1);
        id
    }

    /// Allocate the next wire id (never reused).
    fn alloc_wire(&mut self) -> WireId {
        let id = self.next_wire;
        self.next_wire = self.next_wire.wrapping_add(1);
        id
    }

    // ------------------------------------------------------------ high-level op constructors
    // Each assigns stable ids, applies the mutation, and returns the Op so the caller can push it
    // on the undo stack. Validation that needs the compatibility matrix lives in `connect`, not
    // here: these are the *structural* primitives.

    /// Build and apply an [`Op::AddNode`] for `spec` at `pos` (snapped to the grid by the caller).
    pub fn op_add_node(&mut self, spec: NodeSpec, pos: Vec2) -> Op {
        let id = self.alloc_node();
        let node = Node { id, spec, pos, flags: NodeFlags::default(), custom_name: None };
        self.apply(&Op::AddNode(node.clone()));
        Op::AddNode(node)
    }

    /// Build and apply an [`Op::AddWire`]. The caller (via `connect`) has already validated the
    /// verdict; this only assigns the id and inserts.
    pub fn op_add_wire(&mut self, from: PortRef, to: PortRef) -> Op {
        let id = self.alloc_wire();
        let wire = Wire { id, from, to };
        self.apply(&Op::AddWire(wire));
        Op::AddWire(wire)
    }

    /// Build and apply a move for one node, or `None` if it is already exactly there.
    pub fn op_move_node(&mut self, id: NodeId, to: Vec2) -> Option<Op> {
        let from = self.node(id)?.pos;
        if (from.x - to.x).abs() < f32::EPSILON && (from.y - to.y).abs() < f32::EPSILON {
            return None;
        }
        let op = Op::MoveNode { id, from, to };
        self.apply(&op);
        Some(op)
    }

    /// Build and apply a node deletion as an atomic [`Op::Batch`]: the attached wires go first,
    /// then the node. Returns `None` if the node does not exist.
    pub fn op_remove_node(&mut self, id: NodeId) -> Option<Op> {
        let node = self.node(id)?.clone();
        let wires = self.wires_touching(id);
        let mut batch = Vec::with_capacity(wires.len() + 1);
        for w in &wires {
            batch.push(Op::RemoveWire(*w));
        }
        batch.push(Op::RemoveNode { node: node.clone(), wires: wires.clone() });
        let op = Op::Batch(batch);
        self.apply(&op);
        Some(op)
    }

    /// Build and apply a wire removal. Returns `None` if the wire does not exist.
    pub fn op_remove_wire(&mut self, id: WireId) -> Option<Op> {
        let wire = self.wire(id)?;
        let op = Op::RemoveWire(*wire);
        self.apply(&op);
        Some(op)
    }

    /// Build and apply a flag change, or `None` if unchanged / missing.
    pub fn op_set_flags(&mut self, id: NodeId, to: NodeFlags) -> Option<Op> {
        let from = self.node(id)?.flags;
        if from == to {
            return None;
        }
        let op = Op::SetFlags { id, from, to };
        self.apply(&op);
        Some(op)
    }

    /// Build and apply a rename, or `None` if unchanged / missing.
    pub fn op_rename(&mut self, id: NodeId, to: Option<String>) -> Option<Op> {
        let from = self.node(id)?.custom_name.clone();
        if from == to {
            return None;
        }
        let op = Op::Rename { id, from, to };
        self.apply(&op);
        Some(op)
    }

    // ------------------------------------------------------------ the structural mutator

    /// Apply an op to the graph **without validation** — the low-level mutator that both the
    /// high-level constructors and undo/redo drive. Ids are already assigned; applying an
    /// [`Op::AddNode`]/[`Op::AddWire`] keeps the counters ahead of any restored id so a later
    /// allocation never collides with a node that history can still re-add.
    pub fn apply(&mut self, op: &Op) {
        match op {
            Op::AddNode(n) => {
                if !self.nodes.iter().any(|x| x.id == n.id) {
                    if n.id >= self.next_node {
                        self.next_node = n.id.wrapping_add(1);
                    }
                    self.nodes.push(n.clone());
                }
            },
            Op::RemoveNode { node, .. } => {
                self.nodes.retain(|n| n.id != node.id);
            },
            Op::MoveNode { id, to, .. } => {
                if let Some(n) = self.node_mut(*id) {
                    n.pos = *to;
                }
            },
            Op::AddWire(w) => {
                if !self.wires.iter().any(|x| x.id == w.id) {
                    if w.id >= self.next_wire {
                        self.next_wire = w.id.wrapping_add(1);
                    }
                    self.wires.push(*w);
                }
            },
            Op::RemoveWire(w) => {
                self.wires.retain(|x| x.id != w.id);
            },
            Op::SetFlags { id, to, .. } => {
                if let Some(n) = self.node_mut(*id) {
                    n.flags = *to;
                }
            },
            Op::Rename { id, to, .. } => {
                if let Some(n) = self.node_mut(*id) {
                    n.custom_name = to.clone();
                }
            },
            Op::Batch(ops) => {
                for o in ops {
                    self.apply(o);
                }
            },
        }
    }
}

/// The union of two rects.
fn union(a: Rect, b: Rect) -> Rect {
    Rect::new(
        Vec2::new(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
        Vec2::new(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
    )
}

/// Graph history: an undo stack and a redo stack of applied [`Op`]s.
///
/// Bounded so a long session cannot grow history without limit; the depth is a count, not a visual
/// size, so it is a plain constant rather than a token. When it overflows, the *oldest* undo entry
/// is dropped (you lose the ability to go back further, never the ability to come forward).
#[derive(Clone, Debug, Default)]
pub struct UndoStack {
    undo: Vec<Op>,
    redo: Vec<Op>,
}

/// How many operations of history to keep.
pub const UNDO_DEPTH: usize = 128;

impl UndoStack {
    /// An empty history.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record an op that was just applied. Clears the redo stack (a new action after an undo
    /// forks history — the abandoned branch is dropped, the universal expectation).
    pub fn push(&mut self, op: Op) {
        self.undo.push(op);
        if self.undo.len() > UNDO_DEPTH {
            let excess = self.undo.len() - UNDO_DEPTH;
            self.undo.drain(0..excess);
        }
        self.redo.clear();
    }

    /// Undo the most recent op: apply its inverse, move it to the redo stack. Returns the op that
    /// was undone (for logging), or `None` if history is empty.
    pub fn undo(&mut self, graph: &mut Graph) -> Option<Op> {
        let op = self.undo.pop()?;
        graph.apply(&op.inverse());
        self.redo.push(op.clone());
        Some(op)
    }

    /// Redo the most recently undone op: re-apply it, move it back to the undo stack.
    pub fn redo(&mut self, graph: &mut Graph) -> Option<Op> {
        let op = self.redo.pop()?;
        graph.apply(&op);
        self.undo.push(op.clone());
        Some(op)
    }

    /// Whether there is anything to undo.
    #[must_use]
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there is anything to redo.
    #[must_use]
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Number of recorded undo steps (diagnostics, tests).
    #[must_use]
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use sparq_module_api::manifest::Port;
    use sparq_module_api::port::{Direction, Multiplicity, PortType};

    fn port(id: &str, dir: Direction, ty: PortType) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: ty,
            required: false,
            channel_set: None,
            channel_set_variable: false,
            cv_rate: None,
            cv_range: None,
            multiplicity: Multiplicity::Single,
            latency_contribution: 0,
        }
    }

    fn spec(name: &str, ports: Vec<Port>) -> NodeSpec {
        NodeSpec::new(format!("sparq/{name}"), name, ports)
    }

    fn gain() -> NodeSpec {
        spec(
            "gain",
            vec![
                port("in", Direction::In, PortType::Audio),
                port("out", Direction::Out, PortType::Audio),
            ],
        )
    }

    #[test]
    fn ids_are_stable_and_never_reused() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain(), Vec2::ZERO);
        let b = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        let aid = match a {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        };
        let bid = match b {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        };
        assert_ne!(aid, bid);
        // delete a, add c: c must not reuse a's id
        let _ = g.op_remove_node(aid);
        let c = g.op_add_node(gain(), Vec2::ZERO);
        let cid = match c {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        };
        assert_ne!(cid, aid, "a deleted id must never be reused");
    }

    #[test]
    fn undo_redo_round_trips_a_move() {
        let mut g = Graph::new();
        let op = g.op_add_node(gain(), Vec2::new(0.0, 0.0));
        let id = match &op {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        };
        let mut h = UndoStack::new();
        h.push(op);
        let mv = g.op_move_node(id, Vec2::new(64.0, 32.0)).expect("moved");
        h.push(mv);
        assert_eq!(g.node(id).unwrap().pos, Vec2::new(64.0, 32.0));
        h.undo(&mut g);
        assert_eq!(g.node(id).unwrap().pos, Vec2::ZERO, "undo restores the old position");
        h.redo(&mut g);
        assert_eq!(g.node(id).unwrap().pos, Vec2::new(64.0, 32.0), "redo re-applies");
    }

    #[test]
    fn deleting_a_node_takes_its_wires_and_undo_restores_both() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain(), Vec2::ZERO);
        let b = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        let (aid, bid) = (node_id(&a), node_id(&b));
        let mut h = UndoStack::new();
        h.push(a);
        h.push(b);
        let w = g.op_add_wire(PortRef::new(aid, 1), PortRef::new(bid, 0));
        h.push(w);
        assert_eq!(g.wire_count(), 1);
        let del = g.op_remove_node(aid).expect("node exists");
        h.push(del);
        assert_eq!(g.node_count(), 1, "a is gone");
        assert_eq!(g.wire_count(), 0, "its wire went with it");
        h.undo(&mut g);
        assert_eq!(g.node_count(), 2, "undo restores the node");
        assert_eq!(g.wire_count(), 1, "and the wire, with the same id");
    }

    #[test]
    fn reaches_detects_a_path_and_ignores_a_non_path() {
        let mut g = Graph::new();
        let a = g.op_add_node(gain(), Vec2::ZERO);
        let b = g.op_add_node(gain(), Vec2::new(300.0, 0.0));
        let c = g.op_add_node(gain(), Vec2::new(600.0, 0.0));
        let (aid, bid, cid) = (node_id(&a), node_id(&b), node_id(&c));
        let _ = g.op_add_wire(PortRef::new(aid, 1), PortRef::new(bid, 0));
        let _ = g.op_add_wire(PortRef::new(bid, 1), PortRef::new(cid, 0));
        assert!(g.reaches(aid, cid), "a → b → c");
        assert!(!g.reaches(cid, aid), "not backwards");
        // closing c → a would be a cycle: connect() checks reaches(to, from) = reaches(a, c).
        assert!(g.reaches(aid, cid), "so connect(c→a) is refused");
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut g = Graph::new();
        let mut h = UndoStack::new();
        for i in 0..(UNDO_DEPTH + 10) {
            let op = g.op_add_node(gain(), Vec2::new(i as f32 * 8.0, 0.0));
            h.push(op);
        }
        assert_eq!(h.undo_len(), UNDO_DEPTH, "history stops growing at the cap");
    }

    fn node_id(op: &Op) -> NodeId {
        match op {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        }
    }
}
