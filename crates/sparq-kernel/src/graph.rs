//! The patch graph: structure, versioning, and execution order (WO-008 task 1).
//!
//! This is the **structural** half of ADR-009's executor decision, and deliberately only that
//! half. The kernel sits *below* the module contract in the dependency order (`sparq-module-api`
//! depends on `sparq-kernel`, never the reverse), so this module knows nothing about port types,
//! channel sets or the compatibility matrix: an edge here is two opaque port references and an
//! [`EdgeKind`]. The typed verdict — may this `cv` out feed that `audio` in — is computed by the
//! layer above (via `sparq-module-api`'s `connect_*`, the single copy of the matrix) *before* an
//! edge is offered to this graph. What the kernel owns is everything that must be true no matter
//! what the types say:
//!
//! * **Versioning** (ADR-009 decision 1): every committed topology mutation bumps a monotonic
//!   version; the execution order is cached against it and recomputed only when it changes.
//! * **The cycle rule** (decision 2, plan §5.4, *refined by increment 2*): the graph is acyclic
//!   by default; feedback loops are legal but **must contain at least one `block_delay` edge**.
//!   The refinement is computability, and it is what §5.4's own parenthetical demands ("keeps
//!   the executor a simple topological sort"): a `unit_delay` shifts by one sample *within* the
//!   block, so it constrains the order like a plain edge and cannot close a loop — a loop of
//!   in-block dependencies has no first node to compute. Ordering therefore runs over the
//!   plain + unit-delay subgraph; `block_delay` edges cross the block boundary (the executor
//!   services them from previous-block storage) and are invisible to the sort. An edge that
//!   would close an unorderable loop is refused **with the loop's path in the error**, because
//!   "actionable" is the acceptance criterion, not "correct".
//! * **Determinism** (ADR-007): ids are stable and never reused, and the sort tie-breaks by
//!   smallest id — the order is a pure function of the graph state, so a journal replay lands on
//!   the identical execution order.
//!
//! # Real-time discipline
//!
//! [`Graph::order`] may allocate when it recomputes; it is a **control-thread** call. The RCU
//! swap (ADR-009 decision 3, increment 3 of this WO) guarantees the audio thread only ever sees a
//! graph whose order is already computed — the audio path never calls into the cache-miss branch.
//! Nothing else in this module allocates on a committed path except the growth of the node/edge
//! vectors themselves, which is control-thread territory by the same rule.
//!
//! # What is NOT here yet (declared, per the WO)
//!
//! Buffer pooling and channel-set negotiation (task 2), the executor loop (task 3), the atomic
//! swap + grace period (task 4), latency path computation (task 5 — the per-node
//! `latency_samples` field is stored for it), the watchdog (task 6), and the determinism harness
//! (task 7, which wants the executor first).

use std::collections::{BTreeSet, HashMap};

/// A node identity. Stable, monotonic, never reused — undo/redo and journal replay both depend
/// on an id meaning the same node forever.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);

/// An edge identity. Stable, monotonic, never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(pub u32);

/// A port on a node, structurally: which node, and the index of the port in that module's
/// validated manifest. The kernel never interprets the index — the contract layer above resolves
/// it to a typed port before an edge is offered here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PortRef {
    /// The owning node.
    pub node: NodeId,
    /// Index into the module's validated port list.
    pub port: u32,
}

impl PortRef {
    /// A port reference.
    #[must_use]
    pub const fn new(node: NodeId, port: u32) -> Self {
        Self { node, port }
    }
}

/// What kind of edge this is — the *only* vocabulary the cycle rule needs (plan §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// A plain sample-flow edge. Contributes to the topological order; may never close a cycle.
    Plain,
    /// One-sample delay (z⁻¹): the target reads the source's previous sample *within the block*,
    /// so the source must still run first — a unit-delay edge **constrains the order** exactly
    /// like a plain edge. It therefore cannot close a loop: a loop of in-block dependencies has
    /// no first node to compute (WO-008 increment-2 refinement of §5.4 — the parenthetical
    /// "keeps the executor a simple topological sort" is the clause that decides it).
    UnitDelay,
    /// One-block delay (z^-block): the target reads the source's *previous block* from
    /// executor-owned storage, so the edge crosses the block boundary and is invisible to the
    /// ordering. This is the loop-closer: every feedback cycle must contain at least one.
    BlockDelay,
}

impl EdgeKind {
    /// Whether this kind may legally close a feedback loop.
    #[must_use]
    pub const fn is_delay(self) -> bool {
        !matches!(self, Self::Plain)
    }
}

/// An edge: an output port feeding an input port, with its kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    /// Stable identity.
    pub id: EdgeId,
    /// The source port (an output — enforced by the layer above, which owns directions).
    pub from: PortRef,
    /// The destination port (an input).
    pub to: PortRef,
    /// Plain or one of the two delay kinds.
    pub kind: EdgeKind,
}

/// Why a mutation was refused. Every variant carries the facts needed to *act* — the WO-008
/// acceptance criterion is "rejected at connect time with a clear error", and plan §7.8 says
/// every error is stated in words with a remedy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphError {
    /// The named node does not exist (or was removed).
    NoSuchNode(NodeId),
    /// The named edge does not exist (or was removed).
    NoSuchEdge(EdgeId),
    /// An identical edge (same ports, same kind) already exists.
    DuplicateEdge {
        /// The source.
        from: PortRef,
        /// The destination.
        to: PortRef,
        /// The kind that was already there.
        kind: EdgeKind,
    },
    /// A plain edge from a node to itself. (A *delay* self-edge is legal — it is the classic
    /// feedback idiom.)
    SelfLoop {
        /// The node.
        node: NodeId,
    },
    /// A plain edge would close a cycle. `path` is the existing plain-edge path from the target
    /// back to the source — i.e. the loop the new edge would complete, in order, so the UI can
    /// highlight it and the log can name it.
    Cycle {
        /// The would-be source.
        from: NodeId,
        /// The would-be target.
        to: NodeId,
        /// `to → … → from` over plain edges; adding `from → to` closes the loop.
        path: Vec<NodeId>,
    },
    /// The ordering invariant was found broken (a plain cycle exists despite the connect rule).
    /// Unreachable through the public API; if it ever fires it is a bug, named as one, rather
    /// than a silently truncated order.
    OrderInvariantViolated,
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSuchNode(id) => write!(f, "node {} does not exist", id.0),
            Self::NoSuchEdge(id) => write!(f, "edge {} does not exist", id.0),
            Self::DuplicateEdge { from, to, kind } => write!(
                f,
                "node {} port {} already feeds node {} port {} ({kind:?}) — one wire per port pair",
                from.node.0, from.port, to.node.0, to.port
            ),
            Self::SelfLoop { node } => write!(
                f,
                "node {} cannot feed itself directly — close the loop through a delay edge",
                node.0
            ),
            Self::Cycle { from, to, path } => {
                write!(f, "connecting node {} → node {} would create a cycle (", from.0, to.0)?;
                for (i, n) in path.iter().enumerate() {
                    if i > 0 {
                        write!(f, " → ")?;
                    }
                    write!(f, "{}", n.0)?;
                }
                write!(
                    f,
                    " → {}); a feedback loop must close through a block_delay edge — \
                     unit_delay runs within the block and cannot close a loop (§5.4, \
                     refined by WO-008 increment 2)",
                    to.0
                )
            },
            Self::OrderInvariantViolated => write!(
                f,
                "internal: a plain-edge cycle exists despite the connect rule — this is a bug, \
                 report it with the journal"
            ),
        }
    }
}

impl std::error::Error for GraphError {}

/// One node record. The module instance, buffers and typed ports live in the executor layer
/// above (increment 2+); the kernel stores only what kernel-level computations need — the id and
/// the declared latency, which task 5's per-path accounting sums over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NodeRec {
    /// Stable identity.
    pub id: NodeId,
    /// The module's declared latency in samples (0 when parametric — the upper layer resolves
    /// parametric latency at prepare; this field then holds the resolved value).
    pub latency_samples: u32,
}

/// The patch graph: nodes, edges, a topology version, and the cached execution order.
///
/// Cheap to clone (everything is `Vec`/`u64`), which increment 3 will exploit: the RCU pattern
/// *builds a complete new graph* on the control thread and swaps it in at a block boundary, so
/// "clone, mutate, swap" is the intended mutation path during playback — not in-place edits.
#[derive(Clone, Debug)]
pub struct Graph {
    nodes: Vec<NodeRec>,
    edges: Vec<Edge>,
    version: u64,
    next_node: u32,
    next_edge: u32,
    /// (version the order was computed at, the order itself). `None` until first use.
    order_cache: Option<(u64, Vec<NodeId>)>,
    /// How many times the sort actually ran — the observable proof that the cache works
    /// (ADR-009 decision 1: "recomputed only when the version changes").
    order_computes: u64,
    /// The per-path latency map (task 5), cached on the same discipline as the order — with one
    /// extra key: `set_latency` is a DATA edit that deliberately does not bump the version, so
    /// the cache also keys on a private edit counter, or it would serve numbers it just watched
    /// change. `block_frames` is a key because `BlockDelay` contributions are measured in it.
    latency_cache: Option<LatencyCache>,
    /// Bumped by every successful `set_latency` (the cache key above).
    latency_edits: u64,
    /// How many times the latency map actually recomputed — the order cache's observable proof,
    /// applied to the second cached computation.
    latency_computes: u64,
}

/// One latency-cache entry: the keys it was computed under and the map itself.
#[derive(Clone, Debug)]
struct LatencyCache {
    version: u64,
    edits: u64,
    block_frames: u32,
    map: Vec<(NodeId, u32)>,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

impl Graph {
    /// An empty graph at version 0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            version: 0,
            next_node: 0,
            next_edge: 0,
            order_cache: None,
            order_computes: 0,
            latency_cache: None,
            latency_edits: 0,
            latency_computes: 0,
        }
    }

    /// The topology version. Bumps on every *committed* structural mutation (add/remove node,
    /// connect/disconnect); refused mutations never bump it. Latency edits do not bump it either
    /// — they cannot change the order, and recomputing a valid sort for a data-only change would
    /// be the cache's whole point lost.
    #[must_use]
    pub fn version(&self) -> u64 {
        self.version
    }

    /// How many times the topological sort has actually run (cache diagnostics).
    #[must_use]
    pub fn order_computes(&self) -> u64 {
        self.order_computes
    }

    /// The node record, if present.
    #[must_use]
    pub fn node(&self, id: NodeId) -> Option<&NodeRec> {
        self.nodes.iter().find(|n| n.id == id)
    }

    /// The edge, if present.
    #[must_use]
    pub fn edge(&self, id: EdgeId) -> Option<&Edge> {
        self.edges.iter().find(|e| e.id == id)
    }

    /// All nodes, in insertion order (NOT execution order — that is [`Self::order`]).
    #[must_use]
    pub fn nodes(&self) -> &[NodeRec] {
        &self.nodes
    }

    /// All edges, in insertion order.
    #[must_use]
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Every edge touching a node, in either direction.
    pub fn edges_of(&self, node: NodeId) -> impl Iterator<Item = &Edge> + '_ {
        self.edges.iter().filter(move |e| e.from.node == node || e.to.node == node)
    }

    /// The delay edges — the legal feedback paths. The executor reads these to know which
    /// targets consume *previous*-block (or previous-sample) data.
    pub fn feedback_edges(&self) -> impl Iterator<Item = &Edge> + '_ {
        self.edges.iter().filter(|e| e.kind.is_delay())
    }

    /// Node count.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Edge count.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    // ------------------------------------------------------------------ mutations

    /// Add a node with its declared latency. Returns the fresh id. Bumps the version.
    pub fn add_node(&mut self, latency_samples: u32) -> NodeId {
        let id = NodeId(self.next_node);
        self.next_node = self.next_node.wrapping_add(1);
        self.nodes.push(NodeRec { id, latency_samples });
        self.version = self.version.wrapping_add(1);
        id
    }

    /// Set a node's resolved latency. Data-only: does NOT bump the version (see
    /// [`Self::version`]).
    ///
    /// # Errors
    /// [`GraphError::NoSuchNode`] if the node is gone.
    pub fn set_latency(&mut self, id: NodeId, latency_samples: u32) -> Result<(), GraphError> {
        match self.nodes.iter_mut().find(|n| n.id == id) {
            Some(n) => {
                n.latency_samples = latency_samples;
                // Data edit: the version stays (the order cannot change), but the LATENCY cache
                // keys on this counter, so the next read recomputes rather than serving the
                // number it just watched change.
                self.latency_edits = self.latency_edits.wrapping_add(1);
                Ok(())
            },
            None => Err(GraphError::NoSuchNode(id)),
        }
    }

    /// Remove a node and every edge touching it. Returns the removed edges (the journal and any
    /// undo above this layer need them; ids are never reused, so restoring is a re-add, not an
    /// un-remove). Bumps the version once, whatever the edge count.
    ///
    /// # Errors
    /// [`GraphError::NoSuchNode`] if the node is gone.
    pub fn remove_node(&mut self, id: NodeId) -> Result<Vec<Edge>, GraphError> {
        if !self.nodes.iter().any(|n| n.id == id) {
            return Err(GraphError::NoSuchNode(id));
        }
        self.nodes.retain(|n| n.id != id);
        let mut removed = Vec::new();
        self.edges.retain(|e| {
            if e.from.node == id || e.to.node == id {
                removed.push(*e);
                false
            } else {
                true
            }
        });
        self.version = self.version.wrapping_add(1);
        Ok(removed)
    }

    /// Connect an output port to an input port.
    ///
    /// The structural gauntlet, in the order that produces the most useful refusal: the nodes
    /// must exist → the exact edge must not already exist → a plain edge must not close a cycle
    /// (the refusal carries the loop's path). Delay edges skip the cycle check *by definition* —
    /// they are the legal cycle-closers (§5.4).
    ///
    /// Type compatibility is NOT checked here (the kernel does not know types); the layer above
    /// must hold the contract verdict before calling. On success the edge exists, the version
    /// bumped, and the order cache is stale-by-version (it recomputes lazily on next use).
    ///
    /// # Errors
    /// [`GraphError::NoSuchNode`], [`GraphError::DuplicateEdge`], [`GraphError::SelfLoop`], or
    /// [`GraphError::Cycle`] — each with the facts needed to act on it.
    pub fn connect(
        &mut self,
        from: PortRef,
        to: PortRef,
        kind: EdgeKind,
    ) -> Result<EdgeId, GraphError> {
        if !self.nodes.iter().any(|n| n.id == from.node) {
            return Err(GraphError::NoSuchNode(from.node));
        }
        if !self.nodes.iter().any(|n| n.id == to.node) {
            return Err(GraphError::NoSuchNode(to.node));
        }
        if self.edges.iter().any(|e| e.from == from && e.to == to && e.kind == kind) {
            return Err(GraphError::DuplicateEdge { from, to, kind });
        }
        if kind == EdgeKind::Plain && from.node == to.node {
            return Err(GraphError::SelfLoop { node: from.node });
        }
        // Plain AND unit-delay edges both live inside the block, so both constrain the order:
        // the ordered subgraph (everything except block-delay edges) must stay acyclic. A path
        // to→from already existing means from→to closes an unorderable loop; the path rides
        // along in the error — actionable, not just correct. Block-delay edges skip this check
        // by definition: they ARE the legal closers.
        if kind != EdgeKind::BlockDelay {
            if let Some(path) = self.ordered_path(to.node, from.node) {
                return Err(GraphError::Cycle { from: from.node, to: to.node, path });
            }
        }
        let id = EdgeId(self.next_edge);
        self.next_edge = self.next_edge.wrapping_add(1);
        self.edges.push(Edge { id, from, to, kind });
        self.version = self.version.wrapping_add(1);
        Ok(id)
    }

    /// Remove an edge. Returns it (journal/undo material). Bumps the version.
    ///
    /// # Errors
    /// [`GraphError::NoSuchEdge`] if the edge is gone.
    pub fn disconnect(&mut self, id: EdgeId) -> Result<Edge, GraphError> {
        let Some(pos) = self.edges.iter().position(|e| e.id == id) else {
            return Err(GraphError::NoSuchEdge(id));
        };
        let edge = self.edges.remove(pos);
        self.version = self.version.wrapping_add(1);
        Ok(edge)
    }

    // ------------------------------------------------------------------ ordering

    /// The execution order: every node exactly once; every **plain and unit-delay** edge's
    /// source before its destination (both are in-block dependencies — the unit delay shifts
    /// within the block, it does not cross it); tie-broken by smallest node id so the order is a
    /// pure function of the graph state (ADR-007: a journal replay must land on the identical
    /// order). **Block-delay edges are absent from the ordering by design** — they are the
    /// feedback taps the executor services from previous-block storage.
    ///
    /// Cached against the version (ADR-009 decision 1): the sort runs only when the topology
    /// actually changed; [`Self::order_computes`] counts the real runs.
    ///
    /// Control-thread only (may allocate on a cache miss).
    ///
    /// # Errors
    /// [`GraphError::OrderInvariantViolated`] if the plain subgraph somehow contains a cycle —
    /// unreachable through the public API; the error exists so an impossible state is *named*
    /// rather than silently served as a truncated order.
    pub fn order(&mut self) -> Result<&[NodeId], GraphError> {
        // Phase 1: a hit check that derives only a bool, so the borrow dies with the statement
        // (returning a reference straight out of the cache borrow would taint the whole body
        // under NLL and forbid the re-store below).
        let stale = match &self.order_cache {
            Some((v, _)) => *v != self.version,
            None => true,
        };
        // Phase 2: recompute and re-store only on a version change (ADR-009 decision 1).
        if stale {
            let order = self.compute_order()?;
            self.order_computes += 1;
            self.order_cache = Some((self.version, order));
        }
        // Phase 3: the escaping borrow, with no mutation after it. The None arm is unreachable
        // (phase 2 just stored) and exists so an impossible state is named, not silently served.
        match self.order_cache.as_ref() {
            Some((_, o)) => Ok(o.as_slice()),
            None => Err(GraphError::OrderInvariantViolated),
        }
    }

    // ------------------------------------------------------------ per-path latency (task 5)

    /// The per-path latency map, cached: for every node, the samples a signal arriving at its
    /// OUTPUT has travelled — the node's own declared latency plus the maximum over its incoming
    /// edges of (the source's path latency + the edge's delay). Computed in topological order, so
    /// a node's number exists only once its sources' do; returned in execution order, matching
    /// [`Self::order`].
    ///
    /// Two declared rules (ADR-006.6, plan §5.4):
    /// * a `UnitDelay` edge contributes **1 sample**; a `BlockDelay` edge contributes
    ///   `block_frames` — but `BlockDelay` edges are FEEDBACK edges, outside the topological
    ///   order, so they do not contribute to the static path latency. A signal circling a
    ///   feedback loop travels the delay once per circulation; that is the loop's sound, not a
    ///   static readout, and pretending otherwise would be a number that cannot be true.
    /// * sums saturate at `u32::MAX` — an honest ceiling beats a wrapped-around lie.
    ///
    /// The cache keys on (version, latency edits, block_frames): unlike the order, the latency
    /// map watches a DATA field (`set_latency` deliberately does not bump the version), so it
    /// needs its own edit counter or it would serve the number it just watched change.
    ///
    /// # Errors
    /// Whatever [`Self::order`] reports (a cycle that slipped past `connect` — unreachable
    /// through the checked API).
    pub fn latency_map(&mut self, block_frames: u32) -> Result<&[(NodeId, u32)], GraphError> {
        let stale = match &self.latency_cache {
            Some(c) => {
                c.version != self.version
                    || c.edits != self.latency_edits
                    || c.block_frames != block_frames
            },
            None => true,
        };
        if stale {
            let order = self.order()?.to_vec();
            let mut map: Vec<(NodeId, u32)> = Vec::with_capacity(order.len());
            for id in &order {
                let own =
                    self.nodes.iter().find(|n| n.id == *id).map(|n| n.latency_samples).unwrap_or(0);
                let mut incoming = 0u32;
                for e in
                    self.edges.iter().filter(|e| e.to.node == *id && e.kind != EdgeKind::BlockDelay)
                {
                    let edge_delay = if e.kind == EdgeKind::UnitDelay { 1 } else { 0 };
                    // Sources precede this node in the order, so their numbers already exist.
                    let src_lat =
                        map.iter().find(|(n, _)| *n == e.from.node).map(|(_, l)| *l).unwrap_or(0);
                    incoming = incoming.max(src_lat.saturating_add(edge_delay));
                }
                map.push((*id, own.saturating_add(incoming)));
            }
            self.latency_computes += 1;
            self.latency_cache = Some(LatencyCache {
                version: self.version,
                edits: self.latency_edits,
                block_frames,
                map,
            });
        }
        match self.latency_cache.as_ref() {
            Some(c) => Ok(c.map.as_slice()),
            None => Err(GraphError::OrderInvariantViolated),
        }
    }

    /// One node's path latency in samples (see [`Self::latency_map`]).
    ///
    /// # Errors
    /// [`GraphError::NoSuchNode`] when the node is gone; whatever `order` reports otherwise.
    pub fn path_latency(&mut self, node: NodeId, block_frames: u32) -> Result<u32, GraphError> {
        let found =
            self.latency_map(block_frames)?.iter().find(|(id, _)| *id == node).map(|(_, l)| *l);
        found.ok_or(GraphError::NoSuchNode(node))
    }

    /// The maximum path latency over all nodes, in samples — the alignment target the executor's
    /// compensated mode delays every node up to, and the shell's "total latency" readout. Zero
    /// for an empty graph.
    ///
    /// # Errors
    /// Whatever [`Self::latency_map`] reports.
    pub fn max_path_latency(&mut self, block_frames: u32) -> Result<u32, GraphError> {
        Ok(self.latency_map(block_frames)?.iter().map(|(_, l)| *l).max().unwrap_or(0))
    }

    /// How many times the latency map actually recomputed — `order_computes`' twin for the
    /// second cached computation.
    #[must_use]
    pub fn latency_computes(&self) -> u64 {
        self.latency_computes
    }

    /// Kahn's algorithm over the plain-edge subgraph with a min-id frontier (`BTreeSet`), which
    /// is what makes the order deterministic rather than merely valid.
    fn compute_order(&self) -> Result<Vec<NodeId>, GraphError> {
        let mut indeg: HashMap<NodeId, u32> = HashMap::new();
        for n in &self.nodes {
            indeg.insert(n.id, 0);
        }
        for e in &self.edges {
            if e.kind != EdgeKind::BlockDelay {
                if let Some(d) = indeg.get_mut(&e.to.node) {
                    *d = d.saturating_add(1);
                }
            }
        }
        let mut frontier: BTreeSet<NodeId> = BTreeSet::new();
        for (id, d) in &indeg {
            if *d == 0 {
                frontier.insert(*id);
            }
        }
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(id) = frontier.pop_first() {
            order.push(id);
            for e in &self.edges {
                if e.kind != EdgeKind::BlockDelay && e.from.node == id {
                    if let Some(d) = indeg.get_mut(&e.to.node) {
                        *d = d.saturating_sub(1);
                        if *d == 0 {
                            frontier.insert(e.to.node);
                        }
                    }
                }
            }
        }
        if order.len() == self.nodes.len() {
            Ok(order)
        } else {
            Err(GraphError::OrderInvariantViolated)
        }
    }

    /// The ordered-subgraph path `from → … → to` (plain + unit-delay edges), inclusive, if one
    /// exists. BFS with a parent map so the refusal can carry the shortest loop it would create
    /// — the path a UI would highlight.
    fn ordered_path(&self, from: NodeId, to: NodeId) -> Option<Vec<NodeId>> {
        if from == to {
            return Some(vec![from]);
        }
        let mut parents: HashMap<NodeId, NodeId> = HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(from);
        while let Some(n) = queue.pop_front() {
            for e in &self.edges {
                if e.kind != EdgeKind::BlockDelay
                    && e.from.node == n
                    && !parents.contains_key(&e.to.node)
                {
                    parents.insert(e.to.node, n);
                    if e.to.node == to {
                        // Walk the parent chain back and reverse: to → … → from becomes
                        // from-position-first, which reads as "the loop the edge would close".
                        let mut path = vec![to];
                        let mut cur = to;
                        while let Some(&p) = parents.get(&cur) {
                            path.push(p);
                            cur = p;
                        }
                        path.reverse();
                        return Some(path);
                    }
                    queue.push_back(e.to.node);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    /// out port 0 of node a → in port 0 of node b, plain.
    fn ab(g: &mut Graph, a: NodeId, b: NodeId) -> Result<EdgeId, GraphError> {
        g.connect(PortRef::new(a, 1), PortRef::new(b, 0), EdgeKind::Plain)
    }

    #[test]
    fn order_respects_plain_edges_and_includes_isolated_nodes() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        let c = g.add_node(0); // isolated
        ab(&mut g, a, b).unwrap();
        let order = g.order().unwrap().to_vec();
        assert_eq!(order.len(), 3);
        assert!(order.iter().position(|x| *x == a) < order.iter().position(|x| *x == b));
        assert!(order.contains(&c), "isolated nodes execute too");
    }

    #[test]
    fn order_is_deterministic_smallest_id_first_among_peers() {
        let mut g = Graph::new();
        // Insert in a scrambled order; no edges — the tie-break must sort by id.
        let x = g.add_node(0);
        let y = g.add_node(0);
        let z = g.add_node(0);
        assert_eq!(g.order().unwrap(), &[x, y, z][..]);
        // A second graph built with the same mutations produces the same ids and order.
        let mut g2 = Graph::new();
        let x2 = g2.add_node(0);
        let y2 = g2.add_node(0);
        let z2 = g2.add_node(0);
        assert_eq!((x, y, z), (x2, y2, z2));
        assert_eq!(g.order().unwrap(), g2.order().unwrap());
    }

    #[test]
    fn a_plain_cycle_is_refused_with_the_path() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        let c = g.add_node(0);
        ab(&mut g, a, b).unwrap();
        ab(&mut g, b, c).unwrap();
        let v_before = g.version();
        let err = ab(&mut g, c, a).unwrap_err();
        match err {
            GraphError::Cycle { from, to, path } => {
                assert_eq!((from, to), (c, a));
                assert_eq!(path, vec![a, b, c], "the loop the new edge would close, in order");
                let msg = GraphError::Cycle { from, to, path }.to_string();
                assert!(msg.contains("block_delay"), "the remedy is in the message: {msg}");
            },
            other => panic!("expected Cycle, got {other:?}"),
        }
        assert_eq!(g.version(), v_before, "a refused mutation never bumps the version");
        assert_eq!(g.edge_count(), 2, "and never adds the edge");
    }

    #[test]
    fn delay_edges_close_loops_legally_and_stay_out_of_the_order() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        ab(&mut g, a, b).unwrap();
        // b → a as a block delay: the classic feedback idiom, legal per §5.4.
        g.connect(PortRef::new(b, 1), PortRef::new(a, 0), EdgeKind::BlockDelay).unwrap();
        let order = g.order().unwrap().to_vec();
        assert_eq!(order, vec![a, b], "the delay edge does not constrain the order");
        let fb: Vec<EdgeId> = g.feedback_edges().map(|e| e.id).collect();
        assert_eq!(fb.len(), 1);
    }

    #[test]
    fn a_block_delay_self_loop_is_legal_but_plain_and_unit_are_not() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let err = g.connect(PortRef::new(a, 1), PortRef::new(a, 0), EdgeKind::Plain).unwrap_err();
        assert!(matches!(err, GraphError::SelfLoop { .. }));
        // A unit-delay self-loop is a one-node in-block cycle: uncomputable, refused with the
        // block_delay remedy in the message.
        let err =
            g.connect(PortRef::new(a, 1), PortRef::new(a, 0), EdgeKind::UnitDelay).unwrap_err();
        assert!(matches!(err, GraphError::Cycle { .. }), "got {err:?}");
        assert!(err.to_string().contains("block_delay"));
        g.connect(PortRef::new(a, 1), PortRef::new(a, 0), EdgeKind::BlockDelay).unwrap();
        assert_eq!(g.order().unwrap(), &[a][..]);
    }

    #[test]
    fn a_unit_delay_edge_constrains_the_order() {
        // x gets id 0, y gets id 1; the unit-delay edge runs y → x, so y must execute FIRST
        // despite the id tie-break preferring x. Proves unit-delay edges are ordering edges.
        let mut g = Graph::new();
        let x = g.add_node(0);
        let y = g.add_node(0);
        g.connect(PortRef::new(y, 1), PortRef::new(x, 0), EdgeKind::UnitDelay).unwrap();
        assert_eq!(g.order().unwrap(), &[y, x][..]);
    }

    #[test]
    fn a_unit_delay_loop_is_refused_with_the_block_delay_remedy() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        ab(&mut g, a, b).unwrap();
        // b → a as a UNIT delay would close an entirely in-block loop.
        let err =
            g.connect(PortRef::new(b, 1), PortRef::new(a, 0), EdgeKind::UnitDelay).unwrap_err();
        assert!(matches!(err, GraphError::Cycle { .. }));
        assert!(err.to_string().contains("block_delay"));
        // The same loop closed with a BLOCK delay is legal, and the order stays a → b.
        g.connect(PortRef::new(b, 1), PortRef::new(a, 0), EdgeKind::BlockDelay).unwrap();
        assert_eq!(g.order().unwrap(), &[a, b][..]);
    }

    #[test]
    fn duplicates_are_refused_and_kinds_are_distinct() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        ab(&mut g, a, b).unwrap();
        let err = ab(&mut g, a, b).unwrap_err();
        assert!(matches!(err, GraphError::DuplicateEdge { .. }));
        // The same port pair with a DIFFERENT kind is a different edge (degenerate but structural;
        // the contract layer polices sense, the kernel polices structure).
        g.connect(PortRef::new(a, 1), PortRef::new(b, 0), EdgeKind::UnitDelay).unwrap();
        assert_eq!(g.edge_count(), 2);
    }

    #[test]
    fn version_tracks_committed_mutations_only() {
        let mut g = Graph::new();
        assert_eq!(g.version(), 0);
        let a = g.add_node(3);
        assert_eq!(g.version(), 1);
        g.set_latency(a, 7).unwrap();
        assert_eq!(g.version(), 1, "latency is data, not topology");
        assert_eq!(g.node(a).unwrap().latency_samples, 7);
        let b = g.add_node(0);
        let e = ab(&mut g, a, b).unwrap();
        assert_eq!(g.version(), 3);
        let _ = ab(&mut g, b, a); // refused (cycle) — no bump
        assert_eq!(g.version(), 3);
        g.disconnect(e).unwrap();
        assert_eq!(g.version(), 4);
        g.remove_node(b).unwrap();
        assert_eq!(g.version(), 5);
    }

    #[test]
    fn the_order_cache_recomputes_only_on_version_change() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        ab(&mut g, a, b).unwrap();
        g.order().unwrap();
        g.order().unwrap();
        g.order().unwrap();
        assert_eq!(g.order_computes(), 1, "three reads, one computation");
        let c = g.add_node(0);
        g.order().unwrap();
        assert_eq!(g.order_computes(), 2, "a topology change forces exactly one recompute");
        g.set_latency(c, 11).unwrap();
        g.order().unwrap();
        assert_eq!(g.order_computes(), 2, "a latency edit does not");
    }

    #[test]
    fn removing_a_node_takes_its_edges_and_reports_them() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        let c = g.add_node(0);
        let e1 = ab(&mut g, a, b).unwrap();
        let e2 = ab(&mut g, b, c).unwrap();
        let removed = g.remove_node(b).unwrap();
        assert_eq!(removed.len(), 2, "both touching edges are returned for the journal");
        assert!(removed.iter().any(|e| e.id == e1) && removed.iter().any(|e| e.id == e2));
        assert_eq!(g.node_count(), 2);
        assert_eq!(g.edge_count(), 0);
        assert_eq!(g.order().unwrap(), &[a, c], "a and c are now peers, id-ordered");
    }

    #[test]
    fn a_two_hundred_node_graph_orders_correctly() {
        // The WO-008 acceptance scale. A chain plus fan-out: 200 nodes, 399 plain edges.
        let mut g = Graph::new();
        let mut ids = Vec::new();
        for _ in 0..200 {
            ids.push(g.add_node(0));
        }
        for w in ids.windows(2) {
            ab(&mut g, w[0], w[1]).unwrap();
        }
        for i in 2..200 {
            g.connect(PortRef::new(ids[0], 1), PortRef::new(ids[i], 0), EdgeKind::Plain).unwrap();
        }
        let order = g.order().unwrap().to_vec();
        assert_eq!(order.len(), 200);
        let pos: HashMap<NodeId, usize> =
            order.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        for e in g.edges() {
            assert!(pos[&e.from.node] < pos[&e.to.node], "edge {} violates the order", e.id.0);
        }
    }

    #[test]
    fn ids_are_stable_across_removal() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(0);
        g.remove_node(a).unwrap();
        let c = g.add_node(0);
        assert_ne!(c, a, "a removed id is never reused");
        assert_ne!(c, b);
    }

    #[test]
    fn errors_render_as_sentences_with_remedies() {
        let cycle = GraphError::Cycle {
            from: NodeId(2),
            to: NodeId(0),
            path: vec![NodeId(0), NodeId(1), NodeId(2)],
        };
        let msg = cycle.to_string();
        assert!(msg.contains("cycle") && msg.contains("0 → 1 → 2"), "{msg}");
        let sl = GraphError::SelfLoop { node: NodeId(7) }.to_string();
        assert!(sl.contains("delay"), "the self-loop message names the remedy: {sl}");
    }

    // ------------------------------------------------------- task 5: per-path latency

    #[test]
    fn latency_reference_1_a_linear_chain_sums_the_declarations() {
        // a(0) → b(128 declared) → c(7) — hand-computed: lat(a)=0, lat(b)=128, lat(c)=135.
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(128);
        let c = g.add_node(7);
        ab(&mut g, a, b).unwrap();
        ab(&mut g, b, c).unwrap();
        assert_eq!(g.path_latency(a, 64).unwrap(), 0);
        assert_eq!(g.path_latency(b, 64).unwrap(), 128);
        assert_eq!(g.path_latency(c, 64).unwrap(), 135);
        assert_eq!(g.max_path_latency(64).unwrap(), 135);
    }

    #[test]
    fn latency_reference_2_a_diamond_takes_the_slow_arm() {
        // s(0) → x(64) → m(10) and s → y(0) → m — hand-computed: lat(m) = 10 + max(64, 0) = 74.
        let mut g = Graph::new();
        let s = g.add_node(0);
        let x = g.add_node(64);
        let y = g.add_node(0);
        let m = g.add_node(10);
        ab(&mut g, s, x).unwrap();
        ab(&mut g, s, y).unwrap();
        ab(&mut g, x, m).unwrap();
        ab(&mut g, y, m).unwrap();
        assert_eq!(g.path_latency(m, 64).unwrap(), 74);
        assert_eq!(g.max_path_latency(64).unwrap(), 74);
    }

    #[test]
    fn latency_reference_3_a_unit_delay_edge_contributes_exactly_one_sample() {
        // s(0) -unit_delay-> u(5) — hand-computed: lat(u) = 5 + (0 + 1) = 6, at ANY block size.
        let mut g = Graph::new();
        let s = g.add_node(0);
        let u = g.add_node(5);
        g.connect(PortRef::new(s, 1), PortRef::new(u, 0), EdgeKind::UnitDelay).unwrap();
        assert_eq!(g.path_latency(u, 64).unwrap(), 6);
        assert_eq!(g.path_latency(u, 512).unwrap(), 6, "a unit delay is a sample, not a block");
    }

    #[test]
    fn a_feedback_edge_does_not_contribute_to_static_latency() {
        // s → n plain, n → s block_delay (the legal feedback idiom): s's incoming edge is the
        // feedback one, and a loop has no static path latency — s stays at its own declaration.
        let mut g = Graph::new();
        let s = g.add_node(3);
        let n = g.add_node(11);
        ab(&mut g, s, n).unwrap();
        g.connect(PortRef::new(n, 1), PortRef::new(s, 0), EdgeKind::BlockDelay).unwrap();
        assert_eq!(g.path_latency(s, 64).unwrap(), 3);
        assert_eq!(g.path_latency(n, 64).unwrap(), 14);
    }

    #[test]
    fn the_latency_cache_recomputes_only_on_a_real_change() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        let b = g.add_node(10);
        ab(&mut g, a, b).unwrap();
        assert_eq!(g.latency_computes(), 0);
        assert_eq!(g.path_latency(b, 64).unwrap(), 10);
        assert_eq!(g.latency_computes(), 1);
        // reads at the same state are cache hits
        for _ in 0..5 {
            assert_eq!(g.path_latency(b, 64).unwrap(), 10);
        }
        assert_eq!(g.latency_computes(), 1, "no recompute without a change");
        // a DATA edit (no version bump) must still invalidate — the map cannot serve the number
        // it just watched change
        let v_before = g.version();
        g.set_latency(b, 200).unwrap();
        assert_eq!(g.version(), v_before, "latency edits stay data, not topology");
        assert_eq!(g.path_latency(b, 64).unwrap(), 200);
        assert_eq!(g.latency_computes(), 2);
        // a different block size is a different question
        let _ = g.max_path_latency(128).unwrap();
        assert_eq!(g.latency_computes(), 3);
        // a topology edit invalidates too
        let c = g.add_node(1);
        ab(&mut g, b, c).unwrap();
        assert_eq!(g.path_latency(c, 128).unwrap(), 201);
        assert_eq!(g.latency_computes(), 4);
    }

    #[test]
    fn latency_of_a_missing_node_is_an_error_not_a_zero() {
        let mut g = Graph::new();
        let a = g.add_node(0);
        g.remove_node(a).unwrap();
        assert!(matches!(g.path_latency(a, 64), Err(GraphError::NoSuchNode(_))));
    }
}
