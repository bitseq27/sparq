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
use sparq_module_api::manifest::{Port, ValidatedManifest};

// The param value-type is the CONTRACT's enum, re-exported so consumers of `ParamDesc` can match
// on `kind` without a second import path (one vocabulary, one name in the canvas namespace).
pub use sparq_module_api::manifest::ParamKind;

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

/// One parameter the inspector can show, as a **validated view** of the manifest's `params[]`
/// entry — the same discipline [`Port`] holds for `[[ports]]`: the canvas never re-reads raw
/// `Option` fields, and never copies the vocabulary (the `kind` is the contract's own
/// [`ParamKind`]).
///
/// Numeric kinds (`Float`, `Int`) are guaranteed by manifest validation to carry `unit`, `min`,
/// `max` and an in-range `default`; the defensive `unwrap_or`s in [`param_descs`] only cover the
/// impossible-so-it-is-documented case. `Bool` maps onto `[0, 1]`. The non-numeric kinds
/// (`Enum`, `Text`, `Blob`) keep zeroed ranges and are **not editable in v0** — the inspector
/// shows them greyed with the reason, because inventing an options editor before the manifest
/// schema grows `options[]` would be a lie the user could act on.
#[derive(Clone, Debug, PartialEq)]
pub struct ParamDesc {
    /// Stable id, unique within the module.
    pub id: String,
    /// Display label.
    pub name: String,
    /// The contract's value type.
    pub kind: ParamKind,
    /// Unit for numeric kinds (the closed vocabulary's spelling, e.g. `Hz`).
    pub unit: Option<String>,
    /// Inclusive minimum (numeric kinds); 0 for `Bool`/non-numeric.
    pub min: f64,
    /// Inclusive maximum (numeric kinds); 1 for `Bool`, 0 for non-numeric.
    pub max: f64,
    /// The value a fresh node starts at, inside `[min, max]` for editable kinds.
    pub default: f64,
}

impl ParamDesc {
    /// Whether the v0 inspector can edit this parameter with a slider.
    #[must_use]
    pub fn editable(&self) -> bool {
        matches!(self.kind, ParamKind::Float | ParamKind::Int | ParamKind::Bool)
    }
}

/// The validated parameter view of a manifest's `params[]`, in declared order. Non-numeric kinds
/// are carried (so the inspector can *show* them) but flagged non-editable by
/// [`ParamDesc::editable`].
#[must_use]
pub fn param_descs(m: &ValidatedManifest) -> Vec<ParamDesc> {
    m.manifest()
        .params
        .iter()
        .map(|p| {
            let kind = p.kind.as_deref().and_then(ParamKind::parse).unwrap_or(ParamKind::Blob);
            let id = p.id.clone().unwrap_or_default();
            let name = p.name.clone().unwrap_or_else(|| id.clone());
            match kind {
                ParamKind::Float | ParamKind::Int => ParamDesc {
                    id,
                    name,
                    kind,
                    unit: p.unit.clone(),
                    // Validation required all three for numeric kinds; the fallbacks keep this
                    // function total without pretending a missing field is meaningful.
                    min: p.min.unwrap_or(0.0),
                    max: p.max.unwrap_or(1.0),
                    default: p.default.unwrap_or(0.0),
                },
                ParamKind::Bool => ParamDesc {
                    id,
                    name,
                    kind,
                    unit: None,
                    min: 0.0,
                    max: 1.0,
                    default: if p.default.unwrap_or(0.0) != 0.0 { 1.0 } else { 0.0 },
                },
                _ => ParamDesc { id, name, kind, unit: None, min: 0.0, max: 0.0, default: 0.0 },
            }
        })
        .collect()
}

/// What a canvas node *is*: the identity and port vocabulary it renders and wires against.
///
/// Increment 1 carried only what the canvas needs to draw and connect. Increment 3 adds the
/// parameter descriptors the inspector shows ([`NodeSpec::params`]) — still a validated *view* of
/// the manifest, never a copy of raw schema; the port list remains the validated [`Port`]
/// vocabulary straight from `sparq-module-api`. Custom panels and the UI descriptor (plan §6.6)
/// stay future work.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    /// The module id, e.g. `sparq/syn/sine`.
    pub module_id: String,
    /// The display name, e.g. `Sine` — the node header text.
    pub display_name: String,
    /// The module's validated ports, in manifest order (inputs and outputs interleaved as
    /// declared; [`NodeSpec::inputs`]/[`NodeSpec::outputs`] filter by direction).
    pub ports: Vec<Port>,
    /// The module's validated parameters, in manifest order — what the inspector renders and
    /// what [`Node::param_values`] indexes into.
    pub params: Vec<ParamDesc>,
}

impl NodeSpec {
    /// A spec from an id, a name and its ports (no parameters — tests and hand-built specs).
    #[must_use]
    pub fn new(
        module_id: impl Into<String>,
        display_name: impl Into<String>,
        ports: Vec<Port>,
    ) -> Self {
        Self {
            module_id: module_id.into(),
            display_name: display_name.into(),
            ports,
            params: Vec::new(),
        }
    }

    /// Builder: attach the parameter descriptors.
    #[must_use]
    pub fn with_params(mut self, params: Vec<ParamDesc>) -> Self {
        self.params = params;
        self
    }

    /// The spec of a validated manifest — THE construction path for anything that came through
    /// the registry, so ports and params both stay the contract's validated views (one copy).
    #[must_use]
    pub fn from_manifest(m: &ValidatedManifest) -> Self {
        let id = m.id().to_string();
        let display_name = m.manifest().identity.display_name.clone().unwrap_or_else(|| id.clone());
        Self { module_id: id, display_name, ports: m.ports().to_vec(), params: param_descs(m) }
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
    /// Per-node parameter values, indexed into [`NodeSpec::params`]. Empty means "all defaults"
    /// (nodes restored from pre-increment-3 history); [`Op::SetParam`] materialises the vector
    /// from the spec's defaults before writing, so a partially-filled vector is never observable.
    pub param_values: Vec<f32>,
}

impl Node {
    /// The name the header shows: the rename if set, else the module's display name.
    #[must_use]
    pub fn title(&self) -> &str {
        self.custom_name.as_deref().unwrap_or(&self.spec.display_name)
    }

    /// The current value of parameter `index`: the override if set, else the spec default.
    /// `None` only when the index is out of range.
    #[must_use]
    pub fn param_value(&self, index: usize) -> Option<f32> {
        if index >= self.spec.params.len() {
            return None;
        }
        Some(
            self.param_values.get(index).copied().unwrap_or(self.spec.params[index].default as f32),
        )
    }

    /// Every parameter value in manifest order — what the bridge hands the executor. Always the
    /// full length of [`NodeSpec::params`], defaults filled in.
    #[must_use]
    pub fn effective_params(&self) -> Vec<f32> {
        self.spec
            .params
            .iter()
            .enumerate()
            .map(|(i, d)| self.param_values.get(i).copied().unwrap_or(d.default as f32))
            .collect()
    }

    /// The defaults of the spec's params — the initial `param_values` of a fresh node.
    fn default_param_values(spec: &NodeSpec) -> Vec<f32> {
        spec.params.iter().map(|d| d.default as f32).collect()
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
    /// Set one parameter of one node (already clamped/snapped by [`Graph::op_set_param`], so the
    /// values are exactly what the audio thread will see — the op never carries an out-of-range
    /// value that undo would have to re-validate).
    SetParam {
        /// Which node.
        node: NodeId,
        /// Index into [`NodeSpec::params`].
        index: usize,
        /// Previous value.
        from: f32,
        /// New value.
        to: f32,
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
            Self::SetParam { node, index, from, to } => {
                Self::SetParam { node: *node, index: *index, from: *to, to: *from }
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
            Self::SetParam { .. } => "param",
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
    /// The node's parameter values start at the spec's defaults, materialised — so a fresh node
    /// and an edited-then-undone node carry the same shape of data.
    pub fn op_add_node(&mut self, spec: NodeSpec, pos: Vec2) -> Op {
        let id = self.alloc_node();
        let param_values = Node::default_param_values(&spec);
        let node =
            Node { id, spec, pos, flags: NodeFlags::default(), custom_name: None, param_values };
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

    /// Build and apply a parameter edit, or `None` when there is nothing to record: no such node,
    /// no such parameter, the parameter is not editable in v0 (see [`ParamDesc::editable`]), or
    /// the clamped/snapped value equals the current one.
    ///
    /// The clamp lives HERE, not in the gesture layer, so every path that can change a parameter
    /// (slider drag, numeric entry, a future automation lane) obeys the same range and stepping
    /// rules — and so the [`Op::SetParam`] in history never carries an out-of-range value.
    pub fn op_set_param(&mut self, id: NodeId, index: usize, to: f32) -> Option<Op> {
        let node = self.node(id)?;
        let desc = node.spec.params.get(index)?;
        if !desc.editable() {
            return None;
        }
        let from = node.param_value(index)?;
        let to = clamp_param(desc, to);
        if (from - to).abs() <= f32::EPSILON * from.abs().max(to.abs()).max(1.0) {
            return None;
        }
        let op = Op::SetParam { node: id, index, from, to };
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
            Op::SetParam { node, index, to, .. } => {
                if let Some(n) = self.node_mut(*node) {
                    // Materialise the full vector from defaults before writing, so a node from
                    // older history (empty param_values) upgrades on first edit, and the vector's
                    // length always equals the spec's param count after any edit.
                    if n.param_values.len() < n.spec.params.len() {
                        n.param_values = Node::default_param_values(&n.spec);
                    }
                    if *index < n.param_values.len() {
                        n.param_values[*index] = *to;
                    }
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

/// The parameter range + stepping rule, in one place: clamp to `[min, max]`, snap `Int` to whole
/// steps, snap `Bool` to the nearer pole. `Float` keeps continuous values (zipper-noise concerns
/// belong to the module's smoothing, plan §6 — the canvas must not pre-quantise what the DSP
/// smooths).
fn clamp_param(desc: &ParamDesc, v: f32) -> f32 {
    let lo = desc.min as f32;
    let hi = desc.max as f32;
    let v = v.clamp(lo, hi);
    match desc.kind {
        ParamKind::Int => v.round(),
        ParamKind::Bool => {
            if v >= (lo + hi) / 2.0 {
                hi
            } else {
                lo
            }
        },
        _ => v,
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

    /// The most recent applied op, without removing it. `None` when history is empty.
    #[must_use]
    pub fn top(&self) -> Option<&Op> {
        self.undo.last()
    }

    /// Overwrite the most recent applied op in place (does NOT clear redo — the caller is
    /// *extending* the top action, e.g. one slider drag coalesced into one history entry, not
    /// starting a new one). Returns `false` when history is empty and nothing was replaced.
    pub fn replace_top(&mut self, op: Op) -> bool {
        match self.undo.last_mut() {
            Some(slot) => {
                *slot = op;
                true
            },
            None => false,
        }
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
            cv_reduce: Default::default(),
            cv_interp: Default::default(),
            event_kinds: Vec::new(),
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

    // ------------------------------------------------------- increment 3: per-node param state

    fn pd(id: &str, kind: ParamKind, min: f64, max: f64, default: f64) -> ParamDesc {
        ParamDesc {
            id: id.into(),
            name: id.into(),
            kind,
            unit: Some("Hz".into()),
            min,
            max,
            default,
        }
    }

    fn sine_with_params() -> NodeSpec {
        spec("sine", vec![port("out", Direction::Out, PortType::Audio)]).with_params(vec![
            pd("freq", ParamKind::Float, 0.0, 24000.0, 440.0),
            pd("amp", ParamKind::Float, 0.0, 1.0, 0.5),
        ])
    }

    #[test]
    fn a_fresh_node_carries_the_spec_defaults() {
        let mut g = Graph::new();
        let op = g.op_add_node(sine_with_params(), Vec2::ZERO);
        let id = node_id(&op);
        let n = g.node(id).unwrap();
        assert_eq!(n.effective_params(), vec![440.0, 0.5]);
        assert_eq!(n.param_value(0), Some(440.0));
        assert_eq!(n.param_value(2), None, "out of range is None, not a silent 0");
    }

    #[test]
    fn set_param_clamps_and_undoes() {
        let mut g = Graph::new();
        let op = g.op_add_node(sine_with_params(), Vec2::ZERO);
        let id = node_id(&op);
        let mut h = UndoStack::new();
        h.push(op);

        let set = g.op_set_param(id, 0, 30_000.0).expect("editable");
        assert_eq!(g.node(id).unwrap().param_value(0), Some(24_000.0), "clamped to max");
        h.push(set);

        let set = g.op_set_param(id, 1, 0.25).expect("editable");
        h.push(set);
        assert_eq!(g.node(id).unwrap().effective_params(), vec![24_000.0, 0.25]);

        h.undo(&mut g);
        assert_eq!(g.node(id).unwrap().param_value(1), Some(0.5), "undo restores the value");
        h.undo(&mut g);
        assert_eq!(g.node(id).unwrap().param_value(0), Some(440.0));
        h.redo(&mut g);
        assert_eq!(g.node(id).unwrap().param_value(0), Some(24_000.0), "redo re-applies");
    }

    #[test]
    fn int_params_snap_to_whole_steps_and_bools_to_poles() {
        let mut g = Graph::new();
        let s = spec("x", vec![]).with_params(vec![
            pd("steps", ParamKind::Int, 1.0, 16.0, 4.0),
            pd("on", ParamKind::Bool, 0.0, 1.0, 0.0),
        ]);
        let id = node_id(&g.op_add_node(s, Vec2::ZERO));
        g.op_set_param(id, 0, 7.4).unwrap();
        assert_eq!(g.node(id).unwrap().param_value(0), Some(7.0), "int snaps");
        g.op_set_param(id, 1, 0.7).unwrap();
        assert_eq!(g.node(id).unwrap().param_value(1), Some(1.0), "bool above mid → on");
        g.op_set_param(id, 1, 0.3).unwrap();
        assert_eq!(g.node(id).unwrap().param_value(1), Some(0.0), "bool below mid → off");
        assert!(g.op_set_param(id, 1, 0.2).is_none(), "already off — no-op records nothing");
    }

    #[test]
    fn non_editable_kinds_are_refused_not_faked() {
        let mut g = Graph::new();
        let s = spec("x", vec![]).with_params(vec![pd("mode", ParamKind::Enum, 0.0, 0.0, 0.0)]);
        let id = node_id(&g.op_add_node(s, Vec2::ZERO));
        assert!(g.op_set_param(id, 0, 1.0).is_none(), "enum is not slider-editable in v0");
        assert!(g.op_set_param(id, 9, 1.0).is_none(), "no such parameter");
    }

    #[test]
    fn an_unchanged_value_records_no_op() {
        let mut g = Graph::new();
        let id = node_id(&g.op_add_node(sine_with_params(), Vec2::ZERO));
        assert!(g.op_set_param(id, 0, 440.0).is_none(), "same value is not an edit");
    }

    #[test]
    fn deleting_a_node_takes_its_param_edits_and_undo_restores_them() {
        let mut g = Graph::new();
        let id = node_id(&g.op_add_node(sine_with_params(), Vec2::ZERO));
        let mut h = UndoStack::new();
        h.push(g.op_set_param(id, 0, 880.0).unwrap());
        h.push(g.op_remove_node(id).unwrap());
        assert!(g.node(id).is_none());
        h.undo(&mut g); // restores the delete
        let n = g.node(id).expect("node is back");
        assert_eq!(n.param_value(0), Some(880.0), "the AddNode op carried the edited values");
    }

    #[test]
    fn a_node_from_older_history_materialises_defaults_on_first_edit() {
        let mut g = Graph::new();
        // A node shaped like pre-increment-3 history: spec has params, values are empty.
        let legacy = Node {
            id: 0,
            spec: sine_with_params(),
            pos: Vec2::ZERO,
            flags: NodeFlags::default(),
            custom_name: None,
            param_values: Vec::new(),
        };
        g.apply(&Op::AddNode(legacy));
        assert_eq!(g.node(0).unwrap().param_value(1), Some(0.5), "reads fall back to defaults");
        g.op_set_param(0, 1, 0.9).unwrap();
        assert_eq!(
            g.node(0).unwrap().param_values,
            vec![440.0, 0.9],
            "first edit materialises the whole vector"
        );
    }

    fn node_id(op: &Op) -> NodeId {
        match op {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        }
    }
}
