//! The graph executor (WO-008 tasks 2–3): builds a runnable patch from a kernel [`Graph`] plus a
//! set of contract modules, and renders it one block at a time — deterministically, allocation-
//! free on the audio path, in the cached order the kernel graph computed.
//!
//! # Why this crate and not the kernel
//!
//! The executor calls [`Module::process`], and the `Module` trait lives in `sparq-module-api`,
//! which sits *above* `sparq-kernel` in the dependency order. The kernel owns the structure
//! (`sparq_kernel::graph`: nodes, edges, versions, order — increment 1); this crate owns the
//! machinery that needs the contract. ADR-009's ratification notes will record the placement
//! when WO-008 closes.
//!
//! # What a block does
//!
//! Per node, in the kernel graph's order: **wire** (fill the node's input buffer from its
//! incoming audio edges — direct copy, mono→multi replication, multi→mono summing, fan-in
//! summation, and the two delay kinds), **process** (one `Box<dyn Module>` dispatch per node per
//! block — ADR-009 decision 7's hybrid: trait object at block granularity, nothing per sample),
//! **handle the status** (`Failed` → the output is silenced and flagged, never unwound;
//! `Overrun` is counted toward the task-6 watchdog), **meter** (peak/RMS into relaxed atomics —
//! decision 8's "published, not polled", same-thread for now; cross-thread rings land with the
//! RCU increment). After every node ran, block-delay histories and unit-delay held samples are
//! updated — feedback reads *previous*-block storage, which is what makes cycles legal and the
//! sort simple (§5.4, refined by increment 2).
//!
//! # Buffers (task 2)
//!
//! Every buffer — node inputs, node outputs, delay histories — is allocated and zero-touched in
//! [`Executor::build`]; the audio path only ever writes into existing memory, which the counting
//! allocator proves (`tests/executor.rs`). [`Executor::memory_budget_bytes`] is the number the
//! ADR-009 decision-4 "printed at patch load" line reports.
//!
//! # v0 limits, declared not discovered
//!
//! * One interleaved audio input and one audio output per module (the contract's own v0 limit —
//!   `AudioCtx` has exactly two buffer slots). A manifest declaring two audio inputs is refused
//!   at build with words.
//! * **Non-audio edges do not execute yet**: `cv`/`event`/`data` payloads have nowhere to travel
//!   in v0 `AudioCtx`, so a graph containing one is *refused at build* rather than silently
//!   ignored — a wire the user drew must never do nothing without saying so. Carrying them needs
//!   the multi-port `AudioCtx` (contract v1).
//! * The musical clock is static (`tick` stays 0): three clocks are WO-009.
//! * Mutation is rebuild-and-swap: there is no in-place mutation API — that is the RCU increment
//!   (task 4), and building a whole new `Executor` on the control thread is already the shape it
//!   will use.
//! * Channel-set `variable` resolution is single-pass with a device-channels fallback; every
//!   Phase 0 module declares concrete sets, and the cascade rule (module-api §2) is exercised
//!   properly when a variable-set module exists.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use sparq_kernel::block::BlockContext;
use sparq_kernel::graph::{EdgeKind, Graph, GraphError, NodeId};
use sparq_module_api::manifest::{Port, ValidatedManifest};
use sparq_module_api::module::{
    AudioCtx, BlockStatus, Module, ModuleError, Oversampling, Resources,
};
use sparq_module_api::params::ParamSet;
use sparq_module_api::port::{ChannelSet, Direction, PortType};

/// Executor configuration: the device shape the patch renders into, plus the two policy knobs
/// tasks 5 and 6 added — both defaulted so an untouched config renders exactly as before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecConfig {
    /// Device sample rate in Hz.
    pub sample_rate: u32,
    /// Frames per block (the callback contract; 64 for performance work per §5.4).
    pub block_frames: usize,
    /// Device output channels — the master render target and the variable-set fallback.
    pub device_channels: usize,
    /// The latency policy (task 5, ADR-009 decision 5 / ADR-006.6): report only, or align.
    pub latency: LatencyMode,
    /// The per-module watchdog (task 6, ADR-009 decision 6).
    pub watchdog: Watchdog,
}

/// The global raw/compensated switch (task 5; ADR-006.6 makes compensation opt-in).
///
/// * [`LatencyMode::Raw`] renders what the modules render: each path arrives when it arrives,
///   and the per-path numbers are the kernel graph's [`latency_map`] readout.
/// * [`LatencyMode::Compensated`] aligns every FAN-IN: each incoming edge of a summed input is
///   delayed by `slowest_arm − this_arm` (from the kernel's per-path latency map), so parallel
///   paths arrive at their merge together instead of flamming/comb-filtering. Single-input
///   nodes and chains are untouched — end-to-end latency stays exactly what the map reports,
///   and no global pipeline offset is invented. The delay lines are integer-sample circular
///   buffers allocated (and pre-touched) at build; the audio path only copies.
///
/// [`latency_map`]: sparq_kernel::graph::Graph::latency_map
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LatencyMode {
    /// Report latency; do not touch the signal.
    #[default]
    Raw,
    /// Align every path to the slowest one.
    Compensated,
}

/// The watchdog's one knob (task 6): how many CONSECUTIVE `Overrun` blocks a module may return
/// before it is auto-bypassed. `0` disables the watchdog (overruns stay counted, nothing acts).
///
/// Timing-based detection belongs to the HAL pump, which owns the clock (the executor's audio
/// path may not read one — clippy's RT denials, ADR-006); the executor acts on the status the
/// infrastructure reports. The two halves are the same watchdog: the pump measures and flags,
/// the executor isolates so the rest of the graph keeps playing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Watchdog {
    /// Consecutive overruns that trigger auto-bypass; 0 = disabled.
    pub consecutive_overruns: u32,
}

impl Default for Watchdog {
    fn default() -> Self {
        // N = 3: one bad block is the OS, two is a pattern, three is the module.
        Self { consecutive_overruns: 3 }
    }
}

impl Watchdog {
    /// A watchdog that counts but never acts.
    #[must_use]
    pub const fn disabled() -> Self {
        Self { consecutive_overruns: 0 }
    }
}

/// One watchdog action, journaled executor-side (the project journal itself is WO-011).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatchdogEvent {
    /// The node that was auto-bypassed.
    pub node: NodeId,
    /// The block index at which the bypass engaged.
    pub block: u64,
}

/// How many watchdog events the bounded log holds. Pre-reserved at build, so logging on the
/// audio path never allocates; beyond the cap, events are COUNTED as dropped, never grown —
/// a log that reallocates inside render_block would be worse than no log.
pub const WATCHDOG_EVENT_CAP: usize = 64;

impl ExecConfig {
    /// The usual shape: a device form with the default policies — raw latency (report, do not
    /// touch) and the watchdog armed at its default N. Policy knobs are set explicitly by the
    /// callers that mean to (tests, the compensated render path).
    #[must_use]
    pub const fn new(sample_rate: u32, block_frames: usize, device_channels: usize) -> Self {
        Self {
            sample_rate,
            block_frames,
            device_channels,
            latency: LatencyMode::Raw,
            watchdog: Watchdog { consecutive_overruns: 3 },
        }
    }
}

/// Everything needed to instantiate one node.
pub struct NodeBuild {
    /// The module instance. Its `id()` must equal the manifest's.
    pub module: Box<dyn Module>,
    /// The validated manifest — the port vocabulary the wiring is computed from.
    pub manifest: ValidatedManifest,
    /// The initial parameter snapshot.
    pub params: ParamSet,
}

/// Why a build or render failed. Every variant renders as a sentence with a remedy (plan §7.8).
#[derive(Clone, Debug, PartialEq)]
pub enum ExecError {
    /// The kernel graph refused (cycle, missing node, invariant).
    Graph(GraphError),
    /// A node has no module/manifest pair.
    MissingModule(NodeId),
    /// A module was provided for a node that is not in the graph.
    ExtraModule(NodeId),
    /// `module.id()` and the manifest's id disagree — the contract's first invariant.
    IdMismatch {
        /// Which node.
        node: NodeId,
        /// What the module said.
        module: String,
        /// What the manifest said.
        manifest: String,
    },
    /// A module refused its resources or activation.
    Prepare {
        /// Which node.
        node: NodeId,
        /// The module's own error.
        err: ModuleError,
    },
    /// The graph asks for something v0 cannot carry; the message says what and why.
    Unsupported {
        /// Which node (when node-scoped).
        node: Option<NodeId>,
        /// The sentence.
        why: String,
    },
    /// An edge whose payload v0 cannot carry (cv/event/data/gpu/atom).
    PayloadNotCarried {
        /// Which edge.
        edge: sparq_kernel::graph::EdgeId,
        /// The port type name that cannot travel yet.
        port_type: String,
    },
    /// Two concrete channel sets the matrix gives no rule for (the connect layer should have
    /// refused this edge; the executor refuses it again rather than guess).
    ChannelNegotiation {
        /// Which edge.
        edge: sparq_kernel::graph::EdgeId,
        /// Source channels.
        from: usize,
        /// Destination channels.
        to: usize,
    },
    /// The render target buffer is the wrong size.
    DeviceBuffer {
        /// Expected length in samples.
        expected: usize,
        /// Got length in samples.
        got: usize,
    },
}

impl std::fmt::Display for ExecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graph(e) => write!(f, "graph: {e}"),
            Self::MissingModule(id) => write!(
                f,
                "node {} has no module — every graph node needs a module + manifest pair",
                id.0
            ),
            Self::ExtraModule(id) => {
                write!(f, "a module was provided for node {}, which is not in the graph", id.0)
            },
            Self::IdMismatch { node, module, manifest } => write!(
                f,
                "node {}: the module says it is `{module}` but the manifest says `{manifest}` — \
                 a module and its manifest cannot disagree about who it is",
                node.0
            ),
            Self::Prepare { node, err } => write!(f, "node {} refused to prepare: {err}", node.0),
            Self::Unsupported { node, why } => match node {
                Some(id) => write!(f, "node {} is not supported by executor v0: {why}", id.0),
                None => write!(f, "not supported by executor v0: {why}"),
            },
            Self::PayloadNotCarried { edge, port_type } => write!(
                f,
                "edge {} carries `{port_type}`, whose payload executor v0 cannot deliver \
                 (the v0 AudioCtx carries interleaved audio only) — remove the edge, or wait \
                 for the multi-port contract (v1); the alternative is ignoring a wire the user \
                 drew, and sparq does not do that silently",
                edge.0
            ),
            Self::ChannelNegotiation { edge, from, to } => write!(
                f,
                "edge {}: {from} ch → {to} ch has no rule in the compatibility matrix \
                 (mono→multi fans out, multi→mono sums, anything else converts through a \
                 module) — the connect layer should have refused this edge",
                edge.0
            ),
            Self::DeviceBuffer { expected, got } => write!(
                f,
                "render buffer is {got} samples but the device shape needs {expected} \
                 (frames × device channels)"
            ),
        }
    }
}

impl std::error::Error for ExecError {}

impl From<GraphError> for ExecError {
    fn from(e: GraphError) -> Self {
        Self::Graph(e)
    }
}

/// One node's runtime home.
struct ExecNode {
    id: NodeId,
    module: Box<dyn Module>,
    params: ParamSet,
    /// Output buffer: `frames × out_ch`, written by `process`.
    out_buf: Vec<f32>,
    /// Input buffer: `frames × in_ch`, filled by the wiring pass. Empty when unconnected — an
    /// unconnected input is *reported* (`AudioCtx.input` empty), never silence-by-accident.
    in_buf: Vec<f32>,
    in_ch: usize,
    out_ch: usize,
    status: BlockStatus,
    failed_blocks: u64,
    overrun_blocks: u64,
    /// Consecutive `Overrun` blocks — the watchdog's trigger counter (task 6). Any other status
    /// resets it: the criterion is *consecutive*, one good block earns a fresh slate.
    overrun_streak: u32,
    /// Set by the watchdog: the module is no longer called; its output is passthrough (shapes
    /// match) or silence. Cleared only from the control thread ([`Executor::clear_auto_bypass`]).
    auto_bypassed: bool,
    /// Blocks rendered while auto-bypassed (evidence the isolation is holding, not hiding).
    bypassed_blocks: u64,
}

impl Drop for ExecNode {
    fn drop(&mut self) {
        // The contract: deactivate is control-thread, must not allocate, and cannot fail —
        // so a Drop is a legal place to stop a module that was started.
        self.module.deactivate();
    }
}

/// One incoming audio edge, pre-resolved into wiring instructions.
struct InputPlan {
    kind: EdgeKind,
    /// Which slot's `out_buf` feeds this edge (the source node).
    src_slot: usize,
    /// Source channel count.
    src_ch: usize,
    /// Index into `Executor::delays` for delay kinds.
    delay: Option<usize>,
    /// First edge into the input writes; later edges add (deterministic fan-in sum, edge-id
    /// order — the matrix's stable tie-break).
    add: bool,
    /// Fan-in compensation (task 5, `LatencyMode::Compensated`): the edge's delay line and the
    /// scratch the source block is copied into before the line transforms it. `None` in Raw
    /// mode and for zero-delay arms — the common case pays one `is_some` branch per edge.
    comp: Option<(CompLine, Vec<f32>)>,
}

/// One EDGE's latency-compensation line (task 5, `LatencyMode::Compensated`): an integer-sample
/// circular delay of `delay` interleaved floats over a ring of `delay + block` floats, so the
/// signal this edge contributes is the source's output `delay` samples earlier — the fan-in
/// alignment that makes parallel arms arrive together. Allocated and pre-touched at build; the
/// audio path reads one slot, writes one slot, advances `pos` — no allocation, no branches per
/// sample beyond the wrap.
struct CompLine {
    ring: Vec<f32>,
    /// Write cursor, in interleaved floats.
    pos: usize,
    /// The delay, in interleaved floats (samples × channels).
    delay: usize,
}

impl CompLine {
    fn new(delay_samples: usize, frames: usize, ch: usize) -> Self {
        let delay = delay_samples * ch;
        let len = frames * ch;
        Self { ring: vec![0.0; delay + len], pos: 0, delay }
    }

    /// Delay `out` in place: every sample is replaced by the one written `delay` floats ago,
    /// and the fresh value takes its slot in the ring.
    fn apply(&mut self, out: &mut [f32]) {
        let r = self.ring.len();
        for (i, s) in out.iter_mut().enumerate() {
            let read = (self.pos + i + r - self.delay) % r;
            let fresh = *s;
            *s = self.ring[read];
            self.ring[(self.pos + i) % r] = fresh;
        }
        self.pos = (self.pos + out.len()) % r;
    }
}

/// Executor-owned storage for one delay edge: the previous-block (or previous-sample) memory
/// that makes feedback legal without cycles in the order.
struct DelayState {
    kind: EdgeKind,
    src_slot: usize,
    /// BlockDelay: `frames × src_ch` (the source's last block). UnitDelay: `src_ch` (the
    /// source's last frame).
    hist: Vec<f32>,
}

/// Per-node meters, written by the audio thread with relaxed atomics and readable from any
/// thread that holds a reference — the hal diagnostics pattern. Cross-thread *publication*
/// (lock-free rings, ADR-009 decision 8) lands with the RCU increment; until then the honest
/// claim is: cheap, race-free, same-process.
struct MeterCell {
    peak: AtomicU32,
    rms: AtomicU32,
    status: AtomicU8,
}

impl MeterCell {
    fn new() -> Self {
        Self { peak: AtomicU32::new(0), rms: AtomicU32::new(0), status: AtomicU8::new(0) }
    }
}

/// A meter snapshot, readable from any thread.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeterReading {
    /// Peak |sample| of the node's output in the most recent block.
    pub peak: f32,
    /// RMS of the node's output in the most recent block.
    pub rms: f32,
    /// The node's most recent block status.
    pub status: BlockStatus,
}

fn status_to_u8(s: BlockStatus) -> u8 {
    match s {
        BlockStatus::Ok => 0,
        BlockStatus::Silenced => 1,
        BlockStatus::Overrun => 2,
        BlockStatus::Failed => 3,
    }
}

fn status_from_u8(v: u8) -> BlockStatus {
    match v {
        1 => BlockStatus::Silenced,
        2 => BlockStatus::Overrun,
        3 => BlockStatus::Failed,
        _ => BlockStatus::Ok,
    }
}

/// The built, runnable patch. Build on the control thread; call [`Executor::render_block`] from
/// the audio thread (or the offline driver); mutate by rebuilding and swapping (RCU, task 4).
pub struct Executor {
    cfg: ExecConfig,
    graph: Graph,
    order: Vec<NodeId>,
    slot_of: HashMap<NodeId, usize>,
    nodes: Vec<ExecNode>,
    wiring: Vec<Vec<InputPlan>>,
    delays: Vec<DelayState>,
    meters: Vec<MeterCell>,
    ctx: BlockContext,
    budget_bytes: usize,
    blocks_rendered: u64,
    /// The bounded watchdog journal (pre-reserved; audio-path pushes never allocate).
    wd_events: Vec<WatchdogEvent>,
    /// Events not logged because the journal was full — counted, never silently dropped.
    wd_dropped: u64,
}

impl std::fmt::Debug for Executor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Modules are trait objects; the debug view reports the shape, not the instances.
        f.debug_struct("Executor")
            .field("cfg", &self.cfg)
            .field("nodes", &self.nodes.len())
            .field("edges", &self.graph.edge_count())
            .field("delay_edges", &self.delays.len())
            .field("budget_bytes", &self.budget_bytes)
            .field("blocks_rendered", &self.blocks_rendered)
            .field("auto_bypassed", &self.nodes.iter().filter(|n| n.auto_bypassed).count())
            .finish_non_exhaustive()
    }
}

impl Executor {
    /// Build a runnable patch.
    ///
    /// The gauntlet, in the order that produces the most useful refusal: graph order → the node
    /// sets must match exactly → per node, the module and manifest must agree on identity and
    /// fit the v0 shape (≤1 audio in, ≤1 audio out) → every edge must carry audio (v0) and its
    /// channel pair must have a matrix rule → buffers allocated and touched → `prepare` and
    /// `activate` in execution order (the cascade rule of module-api §2).
    ///
    /// # Errors
    /// Any [`ExecError`]; nothing is half-built — on error, no `Executor` exists.
    pub fn build(
        mut graph: Graph,
        builds: Vec<(NodeId, NodeBuild)>,
        cfg: ExecConfig,
    ) -> Result<Self, ExecError> {
        if cfg.block_frames < 1 || cfg.device_channels < 1 || cfg.sample_rate < 1 {
            return Err(ExecError::Unsupported {
                node: None,
                why: format!(
                    "config {} Hz / {} fr / {} ch is outside the envelope",
                    cfg.sample_rate, cfg.block_frames, cfg.device_channels
                ),
            });
        }
        let order = graph.order()?.to_vec();

        // ---- the node sets must match exactly
        let mut provided: HashMap<NodeId, NodeBuild> = HashMap::new();
        for (id, b) in builds {
            provided.insert(id, b);
        }
        for n in graph.nodes() {
            if !provided.contains_key(&n.id) {
                return Err(ExecError::MissingModule(n.id));
            }
        }
        for id in provided.keys() {
            if graph.node(*id).is_none() {
                return Err(ExecError::ExtraModule(*id));
            }
        }

        let frames = cfg.block_frames;

        // ---- per-node port analysis + identity, in execution order
        struct Ports {
            audio_out: Option<u32>,
            cv_out: bool,
            in_set: Option<ChannelSet>,
            out_set: Option<ChannelSet>,
        }
        let mut slots: Vec<(NodeId, NodeBuild, Ports)> = Vec::with_capacity(order.len());
        for id in &order {
            let Some(build) = provided.remove(id) else {
                return Err(ExecError::MissingModule(*id));
            };
            let mid = build.module.id().to_string();
            let fid = build.manifest.id().to_string();
            if mid != fid {
                return Err(ExecError::IdMismatch { node: *id, module: mid, manifest: fid });
            }
            let ports = build.manifest.ports();
            let audio_ins: Vec<u32> = port_indices(ports, Direction::In, PortType::Audio);
            let audio_outs: Vec<u32> = port_indices(ports, Direction::Out, PortType::Audio);
            if audio_ins.len() > 1 || audio_outs.len() > 1 {
                return Err(ExecError::Unsupported {
                    node: Some(*id),
                    why: format!(
                        "manifest declares {} audio inputs and {} audio outputs; the v0 \
                         AudioCtx carries one interleaved input and one output — multi-port \
                         buffers arrive with contract v1",
                        audio_ins.len(),
                        audio_outs.len()
                    ),
                });
            }
            let cv_out = ports
                .iter()
                .any(|pt| pt.direction == Direction::Out && pt.port_type == PortType::Cv);
            let in_set =
                audio_ins.first().and_then(|i| ports.get(*i as usize)).and_then(|p| p.channel_set);
            let out_set =
                audio_outs.first().and_then(|i| ports.get(*i as usize)).and_then(|p| p.channel_set);
            let pr = Ports { audio_out: audio_outs.first().copied(), cv_out, in_set, out_set };
            slots.push((*id, build, pr));
        }

        let slot_of: HashMap<NodeId, usize> =
            slots.iter().enumerate().map(|(i, (id, _, _))| (*id, i)).collect();

        // ---- every edge must carry audio in v0, both ends
        for e in graph.edges() {
            for (pref, want_dir) in [(e.from, Direction::Out), (e.to, Direction::In)] {
                let slot = slot_of[&pref.node];
                let ports = slots[slot].1.manifest.ports();
                let Some(p) = ports.get(pref.port as usize) else {
                    return Err(ExecError::Unsupported {
                        node: Some(pref.node),
                        why: format!(
                            "edge {} references port index {}, which the manifest does not have",
                            e.id.0, pref.port
                        ),
                    });
                };
                if p.direction != want_dir {
                    return Err(ExecError::Unsupported {
                        node: Some(pref.node),
                        why: format!(
                            "edge {} attaches to port '{}' from the wrong side — a wire runs                              out → in (this end is a {:?} port)",
                            e.id.0, p.id, p.direction
                        ),
                    });
                }
                if p.port_type != PortType::Audio {
                    return Err(ExecError::PayloadNotCarried {
                        edge: e.id,
                        port_type: p.port_type.as_str().to_string(),
                    });
                }
            }
        }

        // ---- channel negotiation (task 2)
        // out_ch: the concrete declared set, else (variable) the first consumer's concrete set,
        // else the device channels. A node with no audio output but a cv output gets one channel
        // (the reference Rms convention: the value rides in output[0]); no outputs at all → 0.
        let mut out_ch: Vec<usize> = vec![0; slots.len()];
        for (slot, (_, _, ports)) in slots.iter().enumerate() {
            out_ch[slot] = if let Some(_ai) = ports.audio_out {
                concrete(ports.out_set).unwrap_or(0) // 0 = pending variable resolution below
            } else if ports.cv_out {
                1
            } else {
                0
            };
        }
        // Resolve pending (variable) outputs from consumers, then fall back to the device shape.
        for e in graph.edges() {
            let src_slot = slot_of[&e.from.node];
            if out_ch[src_slot] == 0 && slots[src_slot].2.audio_out.is_some() {
                let dst_slot = slot_of[&e.to.node];
                if let Some(c) = concrete(slots[dst_slot].2.in_set) {
                    out_ch[src_slot] = c;
                }
            }
        }
        for (slot, (_, _, ports)) in slots.iter().enumerate() {
            if out_ch[slot] == 0 && ports.audio_out.is_some() {
                out_ch[slot] = cfg.device_channels;
            }
        }
        // in_ch: 0 when unconnected (the module SEES unconnected, per the contract); else the
        // concrete declared set, else the first source's resolved out_ch, else the device shape.
        let mut in_ch: Vec<usize> = vec![0; slots.len()];
        let mut connected: Vec<bool> = vec![false; slots.len()];
        for e in graph.edges() {
            let dst_slot = slot_of[&e.to.node];
            connected[dst_slot] = true;
            if in_ch[dst_slot] == 0 {
                if let Some(c) = concrete(slots[dst_slot].2.in_set) {
                    in_ch[dst_slot] = c;
                } else {
                    let src_slot = slot_of[&e.from.node];
                    in_ch[dst_slot] =
                        if out_ch[src_slot] > 0 { out_ch[src_slot] } else { cfg.device_channels };
                }
            }
        }
        for slot in 0..slots.len() {
            if !connected[slot] {
                in_ch[slot] = 0;
            }
        }

        // ---- per-edge channel rules: equal, mono→multi (replicate), multi→mono (sum)
        for e in graph.edges() {
            let s = out_ch[slot_of[&e.from.node]];
            let d = in_ch[slot_of[&e.to.node]];
            if s != d && s != 1 && d != 1 {
                return Err(ExecError::ChannelNegotiation { edge: e.id, from: s, to: d });
            }
        }

        // ---- fan-in compensation plan (task 5, `LatencyMode::Compensated`): per EDGE, from
        // the kernel's own latency map — delay_e = slowest arm into e's destination − e's own
        // arm latency (source path latency + the edge's own delay contribution). Single-input
        // destinations get 0 on every arm and cost nothing. Raw — the default — plans no lines
        // at all and renders byte-identical to increment 2, which the goldens pin.
        let mut edge_comp: Vec<usize> = vec![0; graph.edges().len()];
        if cfg.latency == LatencyMode::Compensated {
            let bf = u32::try_from(frames).unwrap_or(u32::MAX);
            // Copied out: `latency_map` borrows the graph mutably (it may recompute), and the
            // plan below needs immutable edge access while consulting the numbers.
            let map: Vec<(NodeId, u32)> = graph.latency_map(bf)?.to_vec();
            let lat_of =
                |id: NodeId| map.iter().find(|(n, _)| *n == id).map(|(_, l)| *l).unwrap_or(0);
            // v0 aligns the PLAIN arms of a fan-in. Delay-edge arms are excluded on both sides
            // of the computation (they neither set nor receive compensation): a feedback arm is
            // deliberately offset — that offset IS the sound — and block-granular processing
            // cannot place a 1-sample unit arm inside a summed block anyway (sub-block
            // processing is out of the WO's scope). Declared, not hidden.
            let arms: Vec<(NodeId, NodeId)> = graph
                .edges()
                .iter()
                .filter(|e| e.kind == EdgeKind::Plain)
                .map(|e| (e.from.node, e.to.node))
                .collect();
            // per destination: the slowest plain arm
            let mut max_in: HashMap<NodeId, u32> = HashMap::new();
            for (src, dst) in &arms {
                let slot = max_in.entry(*dst).or_insert(0);
                *slot = (*slot).max(lat_of(*src));
            }
            for (ei, e) in graph.edges().iter().enumerate() {
                if e.kind != EdgeKind::Plain {
                    continue;
                }
                edge_comp[ei] = max_in
                    .get(&e.to.node)
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(lat_of(e.from.node)) as usize;
            }
        }

        // ---- delay storage + wiring plans
        let mut delays: Vec<DelayState> = Vec::new();
        let mut wiring: Vec<Vec<InputPlan>> = (0..slots.len()).map(|_| Vec::new()).collect();
        let mut budget_comp: usize = 0;
        for (ei, e) in graph.edges().iter().enumerate() {
            let src_slot = slot_of[&e.from.node];
            let dst_slot = slot_of[&e.to.node];
            let src_ch = out_ch[src_slot];
            let delay = if e.kind == EdgeKind::BlockDelay {
                let d = delays.len();
                delays.push(DelayState {
                    kind: e.kind,
                    src_slot,
                    hist: vec![0.0; frames * src_ch],
                });
                Some(d)
            } else if e.kind == EdgeKind::UnitDelay {
                let d = delays.len();
                delays.push(DelayState { kind: e.kind, src_slot, hist: vec![0.0; src_ch.max(1)] });
                Some(d)
            } else {
                None
            };
            let add = !wiring[dst_slot].is_empty();
            let comp = if edge_comp[ei] > 0 && src_ch > 0 {
                let mut cl = CompLine::new(edge_comp[ei], frames, src_ch);
                let mut scratch = vec![0.0f32; frames * src_ch];
                for b in cl.ring.iter_mut().chain(scratch.iter_mut()) {
                    *b = std::hint::black_box(0.0); // pre-touch: page faults belong to build
                }
                budget_comp += (cl.ring.len() + scratch.len()) * std::mem::size_of::<f32>();
                Some((cl, scratch))
            } else {
                None
            };
            wiring[dst_slot].push(InputPlan { kind: e.kind, src_slot, src_ch, delay, add, comp });
        }

        // ---- buffers, allocated and touched now, never on the audio path (decision 4)
        let mut nodes: Vec<ExecNode> = Vec::with_capacity(slots.len());
        let mut budget: usize = 0;
        for (slot, (id, build, _)) in slots.into_iter().enumerate() {
            let mut out_buf = vec![0.0f32; frames * out_ch[slot]];
            let mut in_buf = vec![0.0f32; frames * in_ch[slot]];
            for b in out_buf.iter_mut().chain(in_buf.iter_mut()) {
                *b = std::hint::black_box(0.0); // pre-touch: page faults belong to build
            }
            budget += (out_buf.len() + in_buf.len()) * std::mem::size_of::<f32>();
            nodes.push(ExecNode {
                id,
                module: build.module,
                params: build.params,
                out_buf,
                in_buf,
                in_ch: in_ch[slot],
                out_ch: out_ch[slot],
                status: BlockStatus::Ok,
                failed_blocks: 0,
                overrun_blocks: 0,
                overrun_streak: 0,
                auto_bypassed: false,
                bypassed_blocks: 0,
            });
        }
        for d in &delays {
            budget += d.hist.len() * std::mem::size_of::<f32>();
        }
        budget += budget_comp;

        // ---- prepare + activate in execution order (module-api §2 cascade rule)
        for n in nodes.iter_mut() {
            let res = Resources {
                sample_rate: cfg.sample_rate,
                block_frames: frames,
                input_channels: n.in_ch,
                output_channels: n.out_ch,
                oversampling: Oversampling::None,
                voices: 1,
                arena_bytes: 0,
            };
            if let Err(err) = n.module.prepare(&res) {
                return Err(ExecError::Prepare { node: n.id, err });
            }
        }
        for n in nodes.iter_mut() {
            if let Err(err) = n.module.activate() {
                return Err(ExecError::Prepare { node: n.id, err });
            }
        }

        let meters: Vec<MeterCell> = (0..nodes.len()).map(|_| MeterCell::new()).collect();
        Ok(Self {
            cfg,
            graph,
            order,
            slot_of,
            nodes,
            wiring,
            delays,
            meters,
            ctx: BlockContext::offline(cfg.sample_rate, frames, cfg.device_channels),
            budget_bytes: budget,
            blocks_rendered: 0,
            // Reserved, not filled: the audio path may push into this, and a push inside
            // capacity never allocates. Beyond the cap events are counted as dropped.
            wd_events: Vec::with_capacity(WATCHDOG_EVENT_CAP),
            wd_dropped: 0,
        })
    }

    /// Render one block and copy the master node's output into `out`
    /// (`block_frames × device_channels` samples, interleaved).
    ///
    /// Audio-thread safe: no allocation, no locks, no syscalls — the counting-allocator test
    /// measures the claim. Errors here are configuration errors (wrong master, wrong buffer
    /// size) and are decided before any module runs.
    ///
    /// # Errors
    /// [`ExecError::Graph`]-wrapped `NoSuchNode` semantics via the master lookup, or
    /// [`ExecError::DeviceBuffer`] for a mis-sized target.
    pub fn render_block(&mut self, master: NodeId, out: &mut [f32]) -> Result<(), ExecError> {
        let Some(&master_slot) = self.slot_of.get(&master) else {
            return Err(ExecError::Graph(GraphError::NoSuchNode(master)));
        };
        let frames = self.cfg.block_frames;
        let dev = self.cfg.device_channels;
        if out.len() != frames * dev {
            return Err(ExecError::DeviceBuffer { expected: frames * dev, got: out.len() });
        }
        let master_ch = self.nodes[master_slot].out_ch;
        if master_ch != dev && master_ch != 1 && dev != 1 && master_ch != 0 {
            return Err(ExecError::ChannelNegotiation {
                edge: sparq_kernel::graph::EdgeId(u32::MAX),
                from: master_ch,
                to: dev,
            });
        }

        let wd_limit = self.cfg.watchdog.consecutive_overruns;
        for slot in 0..self.order.len() {
            // -- wire: fill this node's input buffer from its incoming edges.
            let in_ch = self.nodes[slot].in_ch;
            let mut in_local = std::mem::take(&mut self.nodes[slot].in_buf);
            if in_ch > 0 {
                for s in in_local.iter_mut() {
                    *s = 0.0;
                }
                for plan in &mut self.wiring[slot] {
                    match (plan.kind, plan.delay) {
                        (EdgeKind::BlockDelay, Some(d)) => {
                            let (hist, add) = (&self.delays[d].hist, plan.add);
                            write_edge(&mut in_local, in_ch, hist, plan.src_ch, add, None);
                        },
                        (EdgeKind::UnitDelay, Some(d)) => {
                            // Both borrows are immutable (the held memory refreshes at block
                            // end), so no copy is needed — and a clone here would allocate on
                            // the audio path, which the counting test would rightly fail.
                            let held: &[f32] = &self.delays[d].hist;
                            let src = &self.nodes[plan.src_slot].out_buf;
                            write_edge(
                                &mut in_local,
                                in_ch,
                                src,
                                plan.src_ch,
                                plan.add,
                                Some(held),
                            );
                        },
                        _ => {
                            if let Some((line, scratch)) = plan.comp.as_mut() {
                                // Fan-in compensation (task 5): copy the source block into the
                                // pre-allocated scratch, transform it in place through the edge's
                                // delay line, and wire FROM the delayed signal. Scratch and ring
                                // were sized at build; the audio path allocates nothing.
                                let src = &self.nodes[plan.src_slot].out_buf;
                                let n = scratch.len().min(src.len());
                                scratch[..n].copy_from_slice(&src[..n]);
                                line.apply(&mut scratch[..n]);
                                write_edge(
                                    &mut in_local,
                                    in_ch,
                                    &scratch[..n],
                                    plan.src_ch,
                                    plan.add,
                                    None,
                                );
                            } else {
                                let src = &self.nodes[plan.src_slot].out_buf;
                                write_edge(&mut in_local, in_ch, src, plan.src_ch, plan.add, None);
                            }
                        },
                    }
                }
            }

            // -- process: one trait-object dispatch per node per block (ADR-009 decision 7) —
            //    unless the watchdog isolated this node (task 6). A bypassed module is NEVER
            //    CALLED — the staller cannot stall the block — and its output becomes
            //    passthrough when the shapes match, silence otherwise; the flag, the meter and
            //    the journal all say so, so the isolation is visible, never mysterious.
            let status = if self.nodes[slot].auto_bypassed {
                let n = &mut self.nodes[slot];
                if n.in_ch > 0 && n.in_ch == n.out_ch && n.out_buf.len() == in_local.len() {
                    n.out_buf.copy_from_slice(&in_local);
                } else {
                    for s in n.out_buf.iter_mut() {
                        *s = 0.0;
                    }
                }
                n.bypassed_blocks += 1;
                BlockStatus::Ok
            } else {
                let n = &mut self.nodes[slot];
                let input: &[f32] = if n.in_ch > 0 { &in_local } else { &[] };
                let mut ctx =
                    AudioCtx { block: &self.ctx, params: &n.params, input, output: &mut n.out_buf };
                n.module.process(&mut ctx)
            };

            // -- status handling: Failed is silenced and flagged, never unwound (contract §9).
            //    Overruns feed the watchdog: N consecutive (task 6, ADR-009 d6) auto-bypass the
            //    module so the rest of the graph keeps playing; any other status resets the
            //    streak, because the criterion is *consecutive*.
            let mut just_bypassed = false;
            {
                let n = &mut self.nodes[slot];
                match status {
                    BlockStatus::Failed => {
                        for s in n.out_buf.iter_mut() {
                            *s = 0.0;
                        }
                        n.failed_blocks += 1;
                        n.overrun_streak = 0;
                    },
                    BlockStatus::Overrun => {
                        n.overrun_blocks += 1;
                        n.overrun_streak += 1;
                        if wd_limit > 0 && !n.auto_bypassed && n.overrun_streak >= wd_limit {
                            n.auto_bypassed = true;
                            just_bypassed = true;
                        }
                    },
                    BlockStatus::Ok | BlockStatus::Silenced => n.overrun_streak = 0,
                }
                n.status = status;
                // -- meters (decision 8, same-thread for now). The meters read the node's own
                //    output; edge compensation lives in the WIRING pass (task 5), so a meter
                //    shows what the module made, and the aligned sum is what destinations get.
                let (peak, rms) = peak_rms(&n.out_buf);
                self.meters[slot].peak.store(peak.to_bits(), Ordering::Relaxed);
                self.meters[slot].rms.store(rms.to_bits(), Ordering::Relaxed);
                self.meters[slot].status.store(status_to_u8(status), Ordering::Relaxed);
            }
            if just_bypassed {
                // The journal push is outside the node borrow and inside capacity — bounded,
                // pre-reserved, never an audio-path allocation.
                let (node, block) = (self.nodes[slot].id, self.ctx.block.0);
                if self.wd_events.len() < WATCHDOG_EVENT_CAP {
                    self.wd_events.push(WatchdogEvent { node, block });
                } else {
                    self.wd_dropped += 1;
                }
            }

            self.nodes[slot].in_buf = in_local;
        }

        // -- block end: refresh the delay memories from the sources' finished outputs.
        for d in 0..self.delays.len() {
            let (src_slot, kind) = (self.delays[d].src_slot, self.delays[d].kind);
            let src = &self.nodes[src_slot].out_buf;
            let hist = &mut self.delays[d].hist;
            if kind == EdgeKind::BlockDelay {
                if hist.len() == src.len() {
                    hist.copy_from_slice(src);
                }
            } else if let Some(last) = src.len().checked_sub(hist.len()) {
                hist.copy_from_slice(&src[last..]);
            }
        }

        // -- master render: the node's output, converted to the device shape. A master with no
        // output at all renders silence — an empty device buffer is never left un-written.
        let (m_out, m_ch) = {
            let n = &self.nodes[master_slot];
            (&n.out_buf[..], n.out_ch)
        };
        if m_ch == 0 {
            for s in out.iter_mut() {
                *s = 0.0;
            }
        } else {
            write_edge(out, dev, m_out, m_ch, false, None);
        }

        // -- advance the sample clock (the musical clock is WO-009).
        self.ctx.block = sparq_kernel::block::BlockId(self.ctx.block.0 + 1);
        self.ctx.sample_offset += frames as u64;
        self.blocks_rendered += 1;
        Ok(())
    }

    /// Update a node's parameter snapshot. Control thread, between blocks; the audio path only
    /// ever reads the snapshot, so this never races a `process` that is not running.
    ///
    /// # Errors
    /// [`ExecError::Graph`] wrapping `NoSuchNode` when the node is unknown.
    pub fn set_params(&mut self, node: NodeId, params: ParamSet) -> Result<(), ExecError> {
        let Some(&slot) = self.slot_of.get(&node) else {
            return Err(ExecError::Graph(GraphError::NoSuchNode(node)));
        };
        self.nodes[slot].params = params;
        Ok(())
    }

    /// A node's most recent output buffer (control thread, after a block) — the analysis-tap
    /// read until the lock-free rings land.
    #[must_use]
    pub fn node_output(&self, node: NodeId) -> Option<&[f32]> {
        self.slot_of.get(&node).map(|&s| &self.nodes[s].out_buf[..])
    }

    /// A node's meters. Any thread holding a reference; relaxed loads, no locks.
    #[must_use]
    pub fn meter(&self, node: NodeId) -> Option<MeterReading> {
        let &slot = self.slot_of.get(&node)?;
        Some(MeterReading {
            peak: f32::from_bits(self.meters[slot].peak.load(Ordering::Relaxed)),
            rms: f32::from_bits(self.meters[slot].rms.load(Ordering::Relaxed)),
            status: status_from_u8(self.meters[slot].status.load(Ordering::Relaxed)),
        })
    }

    /// How many blocks a node returned `Failed` (each one silenced and counted).
    #[must_use]
    pub fn failed_blocks(&self, node: NodeId) -> Option<u64> {
        self.slot_of.get(&node).map(|&s| self.nodes[s].failed_blocks)
    }

    /// How many blocks a node returned `Overrun` — the task-6 watchdog's input; see
    /// [`Executor::auto_bypassed`] for its verdict.
    #[must_use]
    pub fn overrun_blocks(&self, node: NodeId) -> Option<u64> {
        self.slot_of.get(&node).map(|&s| self.nodes[s].overrun_blocks)
    }

    /// Whether the watchdog has auto-bypassed this node (task 6). A bypassed module is not
    /// called; its output is passthrough (matching shapes) or silence, and the rest of the
    /// graph keeps playing — the ADR-009 decision-6 isolation, executor-side.
    #[must_use]
    pub fn auto_bypassed(&self, node: NodeId) -> Option<bool> {
        self.slot_of.get(&node).map(|&s| self.nodes[s].auto_bypassed)
    }

    /// Blocks rendered while this node was auto-bypassed.
    #[must_use]
    pub fn bypassed_blocks(&self, node: NodeId) -> Option<u64> {
        self.slot_of.get(&node).map(|&s| self.nodes[s].bypassed_blocks)
    }

    /// Clear a node's auto-bypass (control thread, between blocks): the module is called again
    /// from the next block, with a fresh overrun streak. Returns `false` when the node is
    /// unknown or was not bypassed — the caller can tell "restored" from "nothing to do".
    pub fn clear_auto_bypass(&mut self, node: NodeId) -> bool {
        match self.slot_of.get(&node).map(|&s| &mut self.nodes[s]) {
            Some(n) if n.auto_bypassed => {
                n.auto_bypassed = false;
                n.overrun_streak = 0;
                true
            },
            _ => false,
        }
    }

    /// The bounded watchdog journal: one entry per auto-bypass, in the order they happened
    /// (the executor-side half of ADR-009 d6's "journal entry"; the project journal is WO-011).
    #[must_use]
    pub fn watchdog_events(&self) -> &[WatchdogEvent] {
        &self.wd_events
    }

    /// Watchdog actions that happened but were not journaled because the bounded log was full —
    /// counted, never silently dropped.
    #[must_use]
    pub fn watchdog_events_dropped(&self) -> u64 {
        self.wd_dropped
    }

    /// The per-path latency map (task 5), from the kernel graph's own cached computation —
    /// `(node, samples a signal reaching its output has travelled)`, in execution order.
    ///
    /// # Errors
    /// Whatever the kernel's order reports (unreachable through the checked API).
    pub fn latency_map(&mut self) -> Result<&[(NodeId, u32)], ExecError> {
        let bf = u32::try_from(self.cfg.block_frames).unwrap_or(u32::MAX);
        Ok(self.graph.latency_map(bf)?)
    }

    /// The slowest path's latency in samples — the alignment target [`LatencyMode::Compensated`]
    /// delays every node up to, and the shell's "total latency" readout (raw mode reports it
    /// too: the switch changes the SIGNAL, never the honesty of the number).
    ///
    /// # Errors
    /// Whatever [`Executor::latency_map`] reports.
    pub fn max_path_latency(&mut self) -> Result<u32, ExecError> {
        Ok(self.latency_map()?.iter().map(|(_, l)| *l).max().unwrap_or(0))
    }

    /// Adopt the runtime clock and block count of the executor this one replaces (task 4's
    /// boundary swap): the sample clock is the TRANSPORT's, not the patch's — a graph change
    /// must not reset it, or every mutation would stutter the timeline.
    ///
    /// Module STATE is deliberately not adopted: a swapped-in module starts from its `prepare`
    /// /`activate` state, because carrying state across a patch change is the state protocol's
    /// job (schema'd, journaled — WO-011), not the swap's. Declared, not hidden: a mutation is
    /// an audible event at exactly one block boundary, and nothing else about the timeline moves.
    pub fn inherit_runtime(&mut self, from: &Executor) {
        self.ctx = from.ctx;
        self.blocks_rendered = from.blocks_rendered;
    }

    /// Blocks rendered since build.
    #[must_use]
    pub fn blocks_rendered(&self) -> u64 {
        self.blocks_rendered
    }

    /// The audio buffer budget in bytes (ADR-009 decision 4: printed at patch load).
    #[must_use]
    pub fn memory_budget_bytes(&self) -> usize {
        self.budget_bytes
    }

    /// The execution order (the kernel graph's cached order, captured at build).
    #[must_use]
    pub fn order(&self) -> &[NodeId] {
        &self.order
    }

    /// The graph this executor was built from.
    #[must_use]
    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> ExecConfig {
        self.cfg
    }
}

// --------------------------------------------------------------------------- pure helpers

/// Port indices of a given direction and type, in manifest order.
fn port_indices(ports: &[Port], dir: Direction, ty: PortType) -> Vec<u32> {
    ports
        .iter()
        .enumerate()
        .filter(|(_, p)| p.direction == dir && p.port_type == ty)
        .map(|(i, _)| i as u32)
        .collect()
}

/// A channel set's concrete count; `None` for variable/unset (resolved by the build).
fn concrete(set: Option<ChannelSet>) -> Option<usize> {
    match set {
        Some(ChannelSet::Variable) | None => None,
        Some(s) => s.channels(),
    }
}

/// Write one edge's contribution into a destination interleaved buffer.
///
/// `held` is the unit-delay memory (the source's previous final frame): when present, frame 0
/// reads it and every later frame reads the source shifted by one — z⁻¹, within the block.
/// Conversions: equal channel counts copy sample-wise; mono→multi replicates; multi→mono sums
/// (the two documented silent conversions, §5.4 — the sum is raw, the UI draws the warning
/// hairline). `add` accumulates instead of overwriting (deterministic fan-in).
fn write_edge(
    dst: &mut [f32],
    dst_ch: usize,
    src: &[f32],
    src_ch: usize,
    add: bool,
    held: Option<&[f32]>,
) {
    if dst_ch == 0 || src_ch == 0 {
        return;
    }
    let frames = dst.len() / dst_ch;
    // Fast path: a plain full-frame copy (no conversion, no accumulation, no shift).
    if held.is_none() && !add && dst_ch == src_ch && src.len() == dst.len() {
        dst.copy_from_slice(src);
        return;
    }
    for f in 0..frames {
        let src_frame: &[f32] = match held {
            Some(h) if f == 0 => h,
            Some(_) => {
                let start = (f - 1) * src_ch;
                let end = start + src_ch;
                if end > src.len() {
                    continue;
                }
                &src[start..end]
            },
            None => {
                let start = f * src_ch;
                let end = start + src_ch;
                if end > src.len() {
                    continue;
                }
                &src[start..end]
            },
        };
        let d_start = f * dst_ch;
        if dst_ch == src_ch {
            for c in 0..dst_ch {
                if add {
                    dst[d_start + c] += src_frame[c];
                } else {
                    dst[d_start + c] = src_frame[c];
                }
            }
        } else if src_ch == 1 {
            // mono → multi: fan out (the first documented conversion).
            for c in 0..dst_ch {
                if add {
                    dst[d_start + c] += src_frame[0];
                } else {
                    dst[d_start + c] = src_frame[0];
                }
            }
        } else if dst_ch == 1 {
            // multi → mono: raw sum (the second documented conversion; the UI draws the warning).
            let mut s = 0.0f32;
            for v in src_frame {
                s += *v;
            }
            if add {
                dst[d_start] += s;
            } else {
                dst[d_start] = s;
            }
        }
        // Any other pair is refused at build (ChannelNegotiation): never guess a remix.
    }
}

/// Peak |sample| and RMS over an interleaved buffer. f64 accumulator (plan §5.4's rule for
/// summing paths), empty → zeros. No allocation.
fn peak_rms(buf: &[f32]) -> (f32, f32) {
    let mut peak = 0.0f32;
    let mut sum = 0.0f64;
    for &s in buf {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sum += f64::from(s) * f64::from(s);
    }
    let rms = if buf.is_empty() { 0.0 } else { (sum / buf.len() as f64).sqrt() as f32 };
    (peak, rms)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    #[test]
    fn write_edge_copies_direct() {
        let src = [1.0f32, 2.0, 3.0, 4.0];
        let mut dst = [0.0f32; 4];
        write_edge(&mut dst, 2, &src, 2, false, None);
        assert_eq!(dst, src);
    }

    #[test]
    fn write_edge_replicates_mono_to_stereo() {
        let src = [0.5f32, 0.25];
        let mut dst = [0.0f32; 4];
        write_edge(&mut dst, 2, &src, 1, false, None);
        assert_eq!(dst, [0.5, 0.5, 0.25, 0.25]);
    }

    #[test]
    fn write_edge_sums_stereo_to_mono() {
        let src = [0.5f32, 0.25, -0.5, 0.25];
        let mut dst = [9.0f32; 2];
        write_edge(&mut dst, 1, &src, 2, false, None);
        assert_eq!(dst, [0.75, -0.25], "raw sum, per §5.4");
    }

    #[test]
    fn write_edge_adds_for_fan_in() {
        let a = [0.25f32, 0.25];
        let b = [0.75, 0.5];
        let mut dst = [0.0f32; 2];
        write_edge(&mut dst, 2, &a, 2, false, None);
        write_edge(&mut dst, 2, &b, 2, true, None);
        assert_eq!(dst, [1.0, 0.75]);
    }

    #[test]
    fn write_edge_unit_shift_uses_held_for_frame_zero() {
        // Source this block: [1, 2, 3] (mono, 3 frames); held from last block: [9].
        let src = [1.0f32, 2.0, 3.0];
        let held = [9.0f32];
        let mut dst = [0.0f32; 3];
        write_edge(&mut dst, 1, &src, 1, false, Some(&held));
        assert_eq!(dst, [9.0, 1.0, 2.0], "z^-1: frame 0 is the held sample");
    }

    #[test]
    fn peak_rms_is_exact_for_dc_and_empty() {
        assert_eq!(peak_rms(&[]), (0.0, 0.0));
        let (p, r) = peak_rms(&[0.5, -0.5, 0.5, -0.5]);
        assert_eq!(p, 0.5);
        assert!((r - 0.5).abs() < 1e-7);
    }

    #[test]
    fn concrete_rejects_variable() {
        assert_eq!(concrete(Some(ChannelSet::Stereo)), Some(2));
        assert_eq!(concrete(Some(ChannelSet::Variable)), None);
        assert_eq!(concrete(None), None);
    }
}
