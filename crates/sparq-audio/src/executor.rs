//! The graph executor (WO-008 tasks 2–3, contract v1 in increment 4): builds a runnable patch
//! from a kernel [`Graph`] plus a set of contract modules, and renders it one block at a time —
//! deterministically, allocation-free on the audio path, in the cached order the kernel graph
//! computed.
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
//! Per node, in the kernel graph's order: **wire** (fill the node's input staging from its
//! incoming edges — audio: direct copy, mono→multi replication, multi→mono summing, fan-in
//! summation and the two delay kinds; `cv`: the receiver's declared rate change, reduced or
//! expanded per its own `cv_reduce`/`cv_interp`; `event`: every source merged into one list
//! pre-sorted by sample offset with the matrix's stable tie-break), **process** (one
//! `Box<dyn Module>` dispatch per node per block — ADR-009 decision 7's hybrid: trait object at
//! block granularity, nothing per sample), **handle the status** (`Failed` → the outputs are
//! silenced and flagged, never unwound; `Overrun` is counted toward the task-6 watchdog),
//! **meter** (peak/RMS over the node's audio outputs into relaxed atomics — decision 8's
//! "published, not polled", same-thread for now; cross-thread rings land with the RCU
//! increment). After every node ran, block-delay histories and unit-delay held samples are
//! updated — feedback reads *previous*-block storage, which is what makes cycles legal and the
//! sort simple (§5.4, refined by increment 2).
//!
//! # Buffers (task 2)
//!
//! Every buffer — node inputs, node outputs, delay histories, `cv` wire buffers, event staging —
//! is allocated and zero-touched in [`Executor::build`]; the audio path only ever writes into
//! existing memory, which the counting allocator proves (`tests/executor.rs`).
//! [`Executor::memory_budget_bytes`] is the number the ADR-009 decision-4 "printed at patch
//! load" line reports.
//!
//! # Contract v1 (increment 4): what travels, and what still refuses in words
//!
//! * **Audio**: any number of ports per module (≤ `MAX_PORTS_PER_CLASS` per direction — the
//!   manifests are validated against the same cap), each with its own negotiated channel count,
//!   fan-in sum and latency compensation.
//! * **`cv`**: block-rate values and audio-rate buffers both travel. The RECEIVING port owns any
//!   rate change (matrix G3/G4): an audio-rate source into a block-rate input is reduced by the
//!   receiver's `cv_reduce`; a block-rate source into an audio-rate input is expanded by the
//!   receiver's `cv_interp` (`hold`/`linear`; a receiver declaring `spline` is refused at build —
//!   the vocabulary promises it, the host has not implemented it, and a silent hold would be the
//!   invisible transformation ADR-005 exists to prevent). Range mismatches are refused through
//!   `port.rs::connect_cv` at `Phase::Zero` — the matrix names `util/range`, which is Phase 1
//!   and not in this build, so the refusal says that instead of offering a tap that does nothing.
//!   `cv` fan-in is refused naming `util/mixer` (same reason); `cv` fan-out is free.
//! * **`event`**: fan-in is free and total-ordered — host-injected events first, then edges by
//!   id, then insertion order within a source (matrix G5), merged by a linear k-way merge of
//!   per-source sorted lists, so the receiving module's §9 guarantee ("pre-sorted by sample
//!   offset") is a property of the wire pass, not of the module's patience. Sinks are bounded
//!   ([`EVENTS_PER_BLOCK`]); overflow is counted ([`Executor::event_drops_total`]), never grown.
//!   [`Executor::push_host_event`] is the control-side door — WO-009's transport will feed the
//!   same door; this increment deliberately contains no clock.
//! * **`data`/`gpu`/`atom` edges are still refused at build**, now each with its own sentence:
//!   a wire the user drew must never do nothing without saying so.
//! * The producer's `event_kinds` must be a subset the consumer accepts, checked at build.
//!
//! # v1 limits, declared not discovered
//!
//! * The musical clock is CARRIED, not computed (WO-009): `set_musical_position` is the
//!   transport's door, and a driver that never calls it gets the static `tick = 0` the goldens
//!   and the determinism harness were built against. The map itself lives in `sparq-music`.
//! * Mutation is boundary-swap through [`crate::engine::Engine`]; the cross-thread hot-swap
//!   primitive is parked kernel work (ADR-009 d3).
//! * Channel-set `variable` resolution is single-pass with a device-channels fallback; every
//!   Phase 0 module declares concrete sets, and the cascade rule (module-api §2) is exercised
//!   properly when a variable-set module exists.
//! * A `required` input that is unconnected is NOT refused at build: the module sees the explicit
//!   unconnected signal and answers with the status it chooses (`Silenced` for the reference
//!   modules). Host-side enforcement is a declared open item — the determinism world's unwired
//!   spare node relies on the v0 behaviour, and changing refusals mid-stress would move the
//!   harness's counters, so the change gets its own increment.
//! * Latency compensation aligns PLAIN audio fan-in arms (task 5's declared scope); `cv` and
//!   `event` edges carry no compensation — control payloads arrive when the block says they do.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};

use sparq_kernel::block::BlockContext;
use sparq_kernel::graph::{EdgeKind, Graph, GraphError, NodeId};
use sparq_module_api::event::{Event, EventBuf, EventKind, EVENTS_PER_BLOCK};
use sparq_module_api::manifest::{Port, ValidatedManifest};
use sparq_module_api::module::{
    AudioCtx, BlockStatus, CvIn, CvOut, Module, ModuleError, Oversampling, Resources,
    MAX_PORTS_PER_CLASS,
};
use sparq_module_api::params::ParamSet;
use sparq_module_api::port::{
    connect_cv, ChannelSet, CvInterp, CvRate, CvReduce, Direction, Phase, PortType, Verdict,
};

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
///   buffers allocated (and pre-touched) at build; the audio path only copies. Compensation
///   applies to PLAIN AUDIO arms (contract v1 keeps task 5's scope; `cv`/`event` wires carry no
///   compensation).
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
    /// The graph asks for something the executor cannot carry; the message says what and why.
    Unsupported {
        /// Which node (when node-scoped).
        node: Option<NodeId>,
        /// The sentence.
        why: String,
    },
    /// An edge whose payload the executor does not carry (`data`/`gpu`/`atom` in contract v1 —
    /// `cv` and `event` payloads travel).
    PayloadNotCarried {
        /// Which edge.
        edge: sparq_kernel::graph::EdgeId,
        /// The port type name that cannot travel yet.
        port_type: String,
    },
    /// An edge the compatibility matrix or the contract's port rules refuse (contract v1: cv
    /// range mismatch, cv fan-in, `spline` interpolation, event-kind mismatch, delay edges on
    /// non-audio wires, cross-type wires). The `why` names the rule and the remedy.
    EdgeRefused {
        /// Which edge.
        edge: sparq_kernel::graph::EdgeId,
        /// The sentence.
        why: String,
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
                Some(id) => write!(f, "node {} is not supported by the executor: {why}", id.0),
                None => write!(f, "not supported by the executor: {why}"),
            },
            Self::PayloadNotCarried { edge, port_type } => {
                let remedy = match port_type.as_str() {
                    "data" => {
                        "data records need the schema registry and the staleness machinery \
                               (module-api §4), which have not shipped — remove the edge or wait \
                               for the data-plane increment"
                    },
                    "gpu" => {
                        "gpu payloads never ride the audio thread (ADR-005); the display \
                              plumbing arrives with the ring publication (ADR-009 decision 8)"
                    },
                    "atom" => {
                        "atom is the control-thread channel — deliver it through \
                               `Module::message`, not through a wire"
                    },
                    _ => "remove the edge",
                };
                write!(
                    f,
                    "edge {} carries `{port_type}` payloads, which this executor does not carry \
                     — {remedy}; the alternative is ignoring a wire the user drew, and sparq \
                     does not do that silently",
                    edge.0
                )
            },
            Self::EdgeRefused { edge, why } => write!(f, "edge {} refused: {why}", edge.0),
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

/// One node's per-class port map: for each carried class, the manifest port indices, in manifest
/// order. `AudioCtx` presents ports per class in this order; edges and host accessors address
/// ports by manifest index (the kernel's `PortRef` convention), and this map is the single place
/// the two index spaces meet.
#[derive(Clone, Debug, Default)]
struct NodePlan {
    audio_in: Vec<u32>,
    audio_out: Vec<u32>,
    cv_in: Vec<u32>,
    cv_out: Vec<u32>,
    ev_in: Vec<u32>,
    ev_out: Vec<u32>,
}

impl NodePlan {
    fn of(ports: &[Port]) -> Self {
        let mut p = Self::default();
        for (i, pt) in ports.iter().enumerate() {
            let i = i as u32;
            match (pt.port_type, pt.direction) {
                (PortType::Audio, Direction::In) => p.audio_in.push(i),
                (PortType::Audio, Direction::Out) => p.audio_out.push(i),
                (PortType::Cv, Direction::In) => p.cv_in.push(i),
                (PortType::Cv, Direction::Out) => p.cv_out.push(i),
                (PortType::Event, Direction::In) => p.ev_in.push(i),
                (PortType::Event, Direction::Out) => p.ev_out.push(i),
                // data/gpu/atom: declared and validated, never carried through AudioCtx — the
                // edge pass refuses wires to them in words (PayloadNotCarried).
                _ => {},
            }
        }
        p
    }

    /// The per-type index of a manifest port index within one class list.
    fn index_in(list: &[u32], manifest_idx: u32) -> Option<usize> {
        list.iter().position(|&m| m == manifest_idx)
    }
}

/// A node's input staging: taken out at the top of its slot in the wire pass (so sources can be
/// read from `nodes` without borrow conflicts), filled, presented to the module through
/// `AudioCtx`, and put back. Every buffer is allocated and pre-touched at build.
#[derive(Clone, Debug, Default)]
struct InStaging {
    /// Per audio-in port: `frames × channels` interleaved, empty when unconnected.
    audio: Vec<Vec<f32>>,
    /// Per cv-in port: the block's value for block-rate ports (dead for audio-rate ones).
    cv_cells: Vec<f32>,
    /// Per cv-in port: `frames` samples for audio-rate ports (empty for block-rate ones).
    cv_bufs: Vec<Vec<f32>>,
    /// Per event-in port: the merged, pre-sorted list (reserved at build; cleared per block).
    events: Vec<Vec<Event>>,
}

/// One node's runtime home.
struct ExecNode {
    id: NodeId,
    module: Box<dyn Module>,
    params: ParamSet,
    /// Audio output buffers, per audio-out port in manifest order: `frames × out_chs[p]`.
    out_bufs: Vec<Vec<f32>>,
    /// Resolved channels per audio-in port; 0 = unconnected (the module SEES unconnected, per
    /// the contract — an empty staging buffer, never silence-by-accident).
    in_chs: Vec<usize>,
    /// Resolved channels per audio-out port.
    out_chs: Vec<usize>,
    /// Per cv-in port: the declared rate (the host performs any rate change before the module
    /// sees a value — G3/G4).
    cv_in_rates: Vec<CvRate>,
    /// Per cv-in port: whether at least one edge feeds it (`CvIn::Unconnected` otherwise).
    cv_in_connected: Vec<bool>,
    /// Per cv-out port: the declared rate, choosing the storage the module writes.
    cv_out_rates: Vec<CvRate>,
    /// Per cv-out port: the block-rate cell (dead for audio-rate ports).
    cv_out_cells: Vec<f32>,
    /// Per cv-out port: `frames` samples (empty for block-rate ports).
    cv_out_bufs: Vec<Vec<f32>>,
    /// Per event-out port: the bounded sink storage, cleared before each dispatch and stable-
    /// sorted after it, so downstream merges read sorted lists.
    ev_out: Vec<EventBuf>,
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
    /// The input staging (taken and put back each block; see [`InStaging`]).
    inputs: InStaging,
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
    /// Which slot's output feeds this edge (the source node).
    src_slot: usize,
    /// The source's audio-out port (per-type index).
    src_port: usize,
    /// The destination's audio-in port (per-type index).
    dst_port: usize,
    /// Source channel count.
    src_ch: usize,
    /// Index into `Executor::delays` for delay kinds.
    delay: Option<usize>,
    /// First edge into this (node, port) writes; later edges add (deterministic fan-in sum,
    /// edge-id order — the matrix's stable tie-break).
    add: bool,
    /// Fan-in compensation (task 5, `LatencyMode::Compensated`): the edge's delay line and the
    /// scratch the source block is copied into before the line transforms it. `None` in Raw
    /// mode and for zero-delay arms — the common case pays one `is_some` branch per edge.
    comp: Option<(CompLine, Vec<f32>)>,
}

/// One incoming `cv` edge, pre-resolved. Fan-in is refused at build (the matrix requires an
/// explicit merge), so a cv-in port has at most one plan.
struct CvPlan {
    src_slot: usize,
    /// The source's cv-out port (per-type index).
    src_port: usize,
    /// The destination's cv-in port (per-type index).
    dst_port: usize,
    src_rate: CvRate,
    dst_rate: CvRate,
    /// The RECEIVER's declared reduction (G3), used when an audio-rate source feeds a block-rate
    /// input.
    reduce: CvReduce,
    /// The RECEIVER's declared expansion (G4), used when a block-rate source feeds an audio-rate
    /// input.
    interp: CvInterp,
    /// The previous block's source value — `linear` ramps from here (executor-owned state,
    /// refreshed in the wire pass; the audio path allocates nothing).
    prev: f32,
}

/// One event-in port's merge plan: its sources in EdgeId order (the matrix's connection-id
/// tie-break) plus the cursors the k-way merge runs on. The host-injection queue is rank 0 and
/// is not listed here — it is read from `Executor::host_events` directly.
struct EvPlan {
    srcs: Vec<(usize, usize)>,
    cursors: Vec<usize>,
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
    /// The source's audio-out port (per-type index).
    src_port: usize,
    /// BlockDelay: `frames × src_ch` (the source's last block). UnitDelay: `src_ch` (the
    /// source's last frame).
    hist: Vec<f32>,
}

/// Per-node meters, written by the audio thread with relaxed atomics and readable from any
/// thread that holds a reference — the hal diagnostics pattern. Cross-thread *publication*
/// (lock-free rings, ADR-009 decision 8) lands with the RCU increment; until then the honest
/// claim is: cheap, race-free, same-process. Contract v1: the reading covers ALL of the node's
/// audio outputs (a multi-output node meters as one — per-port meters wait for the rings).
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
    /// Peak |sample| across the node's audio outputs in the most recent block.
    pub peak: f32,
    /// RMS across the node's audio outputs in the most recent block.
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
    plans: Vec<NodePlan>,
    nodes: Vec<ExecNode>,
    wiring: Vec<Vec<InputPlan>>,
    cv_wiring: Vec<Vec<CvPlan>>,
    ev_wiring: Vec<Vec<EvPlan>>,
    /// Control-side event injection: slot → event-in port → bounded queue, kept sorted by sample
    /// at push time. Emptied into the merge at the destination's slot in the block after the
    /// push. WO-009's transport will publish through this same door.
    host_events: Vec<Vec<Vec<Event>>>,
    delays: Vec<DelayState>,
    meters: Vec<MeterCell>,
    ctx: BlockContext,
    /// The musical position door (WO-009): when set, every rendered block's `BlockContext`
    /// carries this `(tick-at-first-frame, ppqn)` — the transport computes, the executor carries.
    /// `None` leaves `tick` where it was (0 by default — the contract-v1 static-clock shape the
    /// determinism harness pins).
    musical: Option<(u64, u32)>,
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

/// One end of an edge, resolved against its node's manifest: everything the wire pass needs
/// without re-walking the port list.
#[derive(Clone, Copy)]
struct EdgeEnd {
    slot: usize,
    /// Manifest port index (the kernel's `PortRef` space).
    manifest_port: u32,
    port_type: PortType,
    /// Per-type index within its class (the `AudioCtx` space).
    type_idx: usize,
}

impl Executor {
    /// Build a runnable patch.
    ///
    /// The gauntlet, in the order that produces the most useful refusal: graph order → the node
    /// sets must match exactly → per node, the module and manifest must agree on identity and
    /// fit the contract's port caps → every edge must carry a payload this executor carries
    /// (`audio`/`cv`/`event`), run out→in between ports of the SAME type, and satisfy that
    /// type's matrix rules (channel negotiation, cv range/fan-in/interp, event-kind subsets) →
    /// buffers allocated and touched → `prepare` and `activate` in execution order (the cascade
    /// rule of module-api §2).
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

        // ---- per-node port plans + identity, in execution order
        let mut slots: Vec<(NodeId, NodeBuild, NodePlan)> = Vec::with_capacity(order.len());
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
            let plan = NodePlan::of(ports);
            // Validation caps every class at MAX_PORTS_PER_CLASS; the executor re-checks rather
            // than trusts, in the same spirit as the channel rules below — a context that cannot
            // present a port must refuse the module, not truncate it.
            for (name, n) in [
                ("audio inputs", plan.audio_in.len()),
                ("audio outputs", plan.audio_out.len()),
                ("cv inputs", plan.cv_in.len()),
                ("cv outputs", plan.cv_out.len()),
                ("event inputs", plan.ev_in.len()),
                ("event outputs", plan.ev_out.len()),
            ] {
                if n > MAX_PORTS_PER_CLASS {
                    return Err(ExecError::Unsupported {
                        node: Some(*id),
                        why: format!(
                            "manifest declares {n} {name}; contract v1 carries at most \
                             {MAX_PORTS_PER_CLASS} ports of one type in one direction — split \
                             the module or use wider ports"
                        ),
                    });
                }
            }
            slots.push((*id, build, plan));
        }

        let slot_of: HashMap<NodeId, usize> =
            slots.iter().enumerate().map(|(i, (id, _, _))| (*id, i)).collect();

        // ---- resolve both ends of every edge against the manifests
        let mut ends: Vec<(EdgeEnd, EdgeEnd)> = Vec::with_capacity(graph.edges().len());
        for e in graph.edges() {
            let resolve = |pref: sparq_kernel::graph::PortRef,
                           want_dir: Direction|
             -> Result<EdgeEnd, ExecError> {
                let slot = slot_of[&pref.node];
                let ports = slots[slot].1.manifest.ports();
                let plan = &slots[slot].2;
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
                            "edge {} attaches to port '{}' from the wrong side — a wire runs \
                             out → in (this end is a {:?} port)",
                            e.id.0, p.id, p.direction
                        ),
                    });
                }
                let list: &[u32] = match (p.port_type, p.direction) {
                    (PortType::Audio, Direction::In) => &plan.audio_in,
                    (PortType::Audio, Direction::Out) => &plan.audio_out,
                    (PortType::Cv, Direction::In) => &plan.cv_in,
                    (PortType::Cv, Direction::Out) => &plan.cv_out,
                    (PortType::Event, Direction::In) => &plan.ev_in,
                    (PortType::Event, Direction::Out) => &plan.ev_out,
                    // data/gpu/atom have no carried class; the payload pass refuses their edges.
                    _ => &[],
                };
                let type_idx = NodePlan::index_in(list, pref.port).unwrap_or(0);
                Ok(EdgeEnd { slot, manifest_port: pref.port, port_type: p.port_type, type_idx })
            };
            let src = resolve(e.from, Direction::Out)?;
            let dst = resolve(e.to, Direction::In)?;
            ends.push((src, dst));
        }

        // ---- payload rules per edge: what travels, what refuses, and how
        let mut cv_edge_count: HashMap<(usize, usize), usize> = HashMap::new();
        for (ei, e) in graph.edges().iter().enumerate() {
            let (src, dst) = ends[ei];
            let src_ports = slots[src.slot].1.manifest.ports();
            let dst_ports = slots[dst.slot].1.manifest.ports();
            let sp = &src_ports[src.manifest_port as usize];
            let dp = &dst_ports[dst.manifest_port as usize];

            if sp.port_type != dp.port_type {
                return Err(ExecError::EdgeRefused {
                    edge: e.id,
                    why: format!(
                        "`{}` → `{}` crosses port types — the same type is required (ADR-005); \
                         the canvas offers one-tap adapter insertion where an adapter exists, \
                         and the connect layer should have refused this edge",
                        sp.port_type, dp.port_type
                    ),
                });
            }
            match sp.port_type {
                PortType::Audio => {}, // channel rules follow, below
                PortType::Cv => {
                    if e.kind != EdgeKind::Plain {
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: "delay edges carry AUDIO memories (block/unit delay of a sample \
                                  stream); a cv wire must be plain — a delayed control path is \
                                  WO-009's timing vocabulary, not an edge kind"
                                .to_string(),
                        });
                    }
                    let (Some(sr), Some(dr)) = (sp.cv_range, dp.cv_range) else {
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: "a cv port without a declared range cannot be checked against \
                                  the matrix — validation should have refused this manifest"
                                .to_string(),
                        });
                    };
                    // The matrix's G2 cell, through the one compiled copy of the matrix:
                    // a range mismatch is REFUSED, never silently rescaled. At Phase::Zero the
                    // named adapter (util/range, Phase 1) does not exist yet, and an offer that
                    // does nothing is the failure the canvas suppression log exists to prevent —
                    // so both verdicts render as refusals that name the remedy and its phase.
                    match connect_cv(sr, dr, Phase::Zero) {
                        Verdict::Compatible | Verdict::Conversion => {},
                        Verdict::Adapter(_) | Verdict::Refused => {
                            // At Phase::Zero the verdict is already Refused (the named adapter
                            // is Phase 1 and `exists_at` degraded it) — the sentence still names
                            // the remedy, because a refusal without its fix is a dead end.
                            let remedy = match connect_cv(sr, dr, Phase::Five) {
                                Verdict::Adapter(a) => format!(
                                    "the matrix names `{}` as the converter (Phase 1 — not in \
                                     this build); until it ships, match the ranges",
                                    a.module_id()
                                ),
                                _ => "match the declared ranges".to_string(),
                            };
                            return Err(ExecError::EdgeRefused {
                                edge: e.id,
                                why: format!(
                                    "{} cv → {} cv is refused — nothing invisible happens to \
                                     the sound (compat-matrix G2); {remedy}",
                                    sr.as_str(),
                                    dr.as_str()
                                ),
                            });
                        },
                        Verdict::Undecided => {
                            return Err(ExecError::EdgeRefused {
                                edge: e.id,
                                why: "the compatibility matrix has no rule for this cv cell"
                                    .to_string(),
                            });
                        },
                    }
                    let count = cv_edge_count.entry((dst.slot, dst.type_idx)).or_insert(0);
                    *count += 1;
                    if *count > 1 {
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: "cv fan-in requires an explicit merge — no implicit summing \
                                  (compat-matrix G-cell `fan-in`). The named module `util/mixer` \
                                  ships as the AUDIO 4×4 matrix; its cv merge side is not built \
                                  yet (declared in LATER.md), so until it is: remove one of the \
                                  edges into this input"
                                .to_string(),
                        });
                    }
                    // The receiver's declared interpolation must be one the host performs.
                    if dp.cv_interp == CvInterp::Spline {
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: "the receiving port declares `cv_interp = \"spline\"`, which is \
                                  in the contract's vocabulary but not implemented by this host \
                                  — holding instead would render differently than declared, so \
                                  the edge is refused; declare `hold` or `linear`"
                                .to_string(),
                        });
                    }
                },
                PortType::Event => {
                    if e.kind != EdgeKind::Plain {
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: "delay edges carry AUDIO memories; an event wire must be plain — \
                                  event timing is the sample offset in the payload (and WO-009's \
                                  clock domain above it)"
                                .to_string(),
                        });
                    }
                    // The matrix's event default cell: the producer's dialects must be a subset
                    // the consumer accepts (E-EVENTKIND-UNACCEPTED at the connect layer; the
                    // executor refuses again rather than deliver events nobody declared).
                    if !sp.event_kinds.iter().all(|k| dp.event_kinds.contains(k)) {
                        let speak: Vec<String> =
                            sp.event_kinds.iter().map(EventKind::to_string).collect();
                        let accepts: Vec<String> =
                            dp.event_kinds.iter().map(EventKind::to_string).collect();
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: format!(
                                "the source speaks [{}] but the destination accepts only [{}] — \
                                 a producer's event_kinds must be a subset the consumer accepts \
                                 (compat-matrix, E-EVENTKIND-UNACCEPTED)",
                                speak.join(", "),
                                accepts.join(", ")
                            ),
                        });
                    }
                },
                PortType::Data | PortType::Gpu | PortType::Atom => {
                    return Err(ExecError::PayloadNotCarried {
                        edge: e.id,
                        port_type: sp.port_type.as_str().to_string(),
                    });
                },
            }
        }

        // ---- channel negotiation (task 2, contract v1: per audio PORT)
        // out_chs: the concrete declared set per audio-out port, else (variable) the first
        // consumer's concrete set, else the device channels. Nodes with no audio output have no
        // audio buffers at all — the v0 convention of giving cv sources a 1-channel audio buffer
        // is gone, because a cv payload that rides in an audio buffer is a wire that lies about
        // its type.
        let mut out_chs: Vec<Vec<usize>> = Vec::with_capacity(slots.len());
        let mut in_sets: Vec<Vec<Option<ChannelSet>>> = Vec::with_capacity(slots.len());
        let mut out_sets: Vec<Vec<Option<ChannelSet>>> = Vec::with_capacity(slots.len());
        for (_, build, plan) in &slots {
            let ports = build.manifest.ports();
            out_chs.push(
                plan.audio_out
                    .iter()
                    .map(|&i| concrete(ports[i as usize].channel_set).unwrap_or(0))
                    .collect(),
            );
            in_sets.push(plan.audio_in.iter().map(|&i| ports[i as usize].channel_set).collect());
            out_sets.push(plan.audio_out.iter().map(|&i| ports[i as usize].channel_set).collect());
        }
        // Resolve pending (variable) outputs from consumers, then fall back to the device shape.
        for &(src, dst) in &ends {
            if src.port_type != PortType::Audio {
                continue;
            }
            if out_chs[src.slot][src.type_idx] == 0 {
                if let Some(c) = concrete(in_sets[dst.slot][dst.type_idx]) {
                    out_chs[src.slot][src.type_idx] = c;
                }
            }
        }
        for slot in 0..slots.len() {
            for (p, ch) in out_chs[slot].iter_mut().enumerate() {
                if *ch == 0 && out_sets[slot][p].is_some() {
                    *ch = cfg.device_channels;
                }
            }
        }
        // in_chs: 0 when unconnected (the module SEES unconnected, per the contract); else the
        // concrete declared set, else the first source's resolved out_chs, else the device shape.
        let mut in_chs: Vec<Vec<usize>> =
            (0..slots.len()).map(|s| vec![0; slots[s].2.audio_in.len()]).collect();
        for &(src, dst) in &ends {
            if src.port_type != PortType::Audio {
                continue;
            }
            if in_chs[dst.slot][dst.type_idx] == 0 {
                if let Some(c) = concrete(in_sets[dst.slot][dst.type_idx]) {
                    in_chs[dst.slot][dst.type_idx] = c;
                } else {
                    let sc = out_chs[src.slot][src.type_idx];
                    in_chs[dst.slot][dst.type_idx] = if sc > 0 { sc } else { cfg.device_channels };
                }
            }
        }

        // ---- per-edge channel rules: equal, mono→multi (replicate), multi→mono (sum)
        for (ei, e) in graph.edges().iter().enumerate() {
            let (src, dst) = ends[ei];
            if src.port_type != PortType::Audio {
                continue;
            }
            let s = out_chs[src.slot][src.type_idx];
            let d = in_chs[dst.slot][dst.type_idx];
            if s != d && s != 1 && d != 1 {
                return Err(ExecError::ChannelNegotiation { edge: e.id, from: s, to: d });
            }
        }

        // ---- fan-in compensation plan (task 5, `LatencyMode::Compensated`): per AUDIO edge,
        // from the kernel's own latency map — delay_e = slowest arm into e's destination − e's
        // own arm latency (source path latency + the edge's own delay contribution). Single-input
        // destinations get 0 on every arm and cost nothing. Raw — the default — plans no lines
        // at all and renders byte-identical to increment 2, which the goldens pin. Contract v1
        // keeps the scope declared in increment 3: PLAIN audio arms only, and per PORT (the
        // slowest-arm grouping is per destination node, as the kernel's map is per node).
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
                .zip(ends.iter())
                .filter(|(e, (src, _))| {
                    e.kind == EdgeKind::Plain && src.port_type == PortType::Audio
                })
                .map(|(e, _)| (e.from.node, e.to.node))
                .collect();
            // per destination: the slowest plain arm
            let mut max_in: HashMap<NodeId, u32> = HashMap::new();
            for (src, dst) in &arms {
                let slot = max_in.entry(*dst).or_insert(0);
                *slot = (*slot).max(lat_of(*src));
            }
            for (ei, e) in graph.edges().iter().enumerate() {
                if e.kind != EdgeKind::Plain || ends[ei].0.port_type != PortType::Audio {
                    continue;
                }
                edge_comp[ei] = max_in
                    .get(&e.to.node)
                    .copied()
                    .unwrap_or(0)
                    .saturating_sub(lat_of(e.from.node)) as usize;
            }
        }

        // ---- delay storage + wiring plans (audio), cv plans, event plans
        let mut delays: Vec<DelayState> = Vec::new();
        let mut wiring: Vec<Vec<InputPlan>> = (0..slots.len()).map(|_| Vec::new()).collect();
        let mut cv_wiring: Vec<Vec<CvPlan>> = (0..slots.len()).map(|_| Vec::new()).collect();
        let mut ev_wiring: Vec<Vec<EvPlan>> = (0..slots.len())
            .map(|s| {
                (0..slots[s].2.ev_in.len())
                    .map(|_| EvPlan { srcs: Vec::new(), cursors: Vec::new() })
                    .collect()
            })
            .collect();
        let mut cv_in_connected: Vec<Vec<bool>> =
            (0..slots.len()).map(|s| vec![false; slots[s].2.cv_in.len()]).collect();
        let mut budget_comp: usize = 0;
        let mut audio_writes: Vec<HashMap<(usize, usize), usize>> =
            (0..slots.len()).map(|_| HashMap::new()).collect();
        for (ei, e) in graph.edges().iter().enumerate() {
            let (src, dst) = ends[ei];
            match src.port_type {
                PortType::Audio => {
                    let src_ch = out_chs[src.slot][src.type_idx];
                    let delay = if e.kind == EdgeKind::BlockDelay {
                        let d = delays.len();
                        delays.push(DelayState {
                            kind: e.kind,
                            src_slot: src.slot,
                            src_port: src.type_idx,
                            hist: vec![0.0; frames * src_ch],
                        });
                        Some(d)
                    } else if e.kind == EdgeKind::UnitDelay {
                        let d = delays.len();
                        delays.push(DelayState {
                            kind: e.kind,
                            src_slot: src.slot,
                            src_port: src.type_idx,
                            hist: vec![0.0; src_ch.max(1)],
                        });
                        Some(d)
                    } else {
                        None
                    };
                    let seen = audio_writes[dst.slot].entry((dst.slot, dst.type_idx)).or_insert(0);
                    let add = *seen > 0;
                    *seen += 1;
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
                    wiring[dst.slot].push(InputPlan {
                        kind: e.kind,
                        src_slot: src.slot,
                        src_port: src.type_idx,
                        dst_port: dst.type_idx,
                        src_ch,
                        delay,
                        add,
                        comp,
                    });
                },
                PortType::Cv => {
                    let src_ports = slots[src.slot].1.manifest.ports();
                    let dst_ports = slots[dst.slot].1.manifest.ports();
                    let (Some(src_rate), Some(dst_rate)) = (
                        src_ports[src.manifest_port as usize].cv_rate,
                        dst_ports[dst.manifest_port as usize].cv_rate,
                    ) else {
                        return Err(ExecError::EdgeRefused {
                            edge: e.id,
                            why: "a cv port without a declared rate cannot be wired — validation \
                                  should have refused this manifest"
                                .to_string(),
                        });
                    };
                    cv_in_connected[dst.slot][dst.type_idx] = true;
                    cv_wiring[dst.slot].push(CvPlan {
                        src_slot: src.slot,
                        src_port: src.type_idx,
                        dst_port: dst.type_idx,
                        src_rate,
                        dst_rate,
                        reduce: dst_ports[dst.manifest_port as usize].cv_reduce,
                        interp: dst_ports[dst.manifest_port as usize].cv_interp,
                        prev: 0.0,
                    });
                },
                PortType::Event => {
                    ev_wiring[dst.slot][dst.type_idx].srcs.push((src.slot, src.type_idx));
                },
                PortType::Data | PortType::Gpu | PortType::Atom => {
                    // Unreachable: refused in the payload pass above. Refuse again rather than
                    // silently skipping — a wire is never a no-op.
                    return Err(ExecError::PayloadNotCarried {
                        edge: e.id,
                        port_type: src.port_type.as_str().to_string(),
                    });
                },
            }
        }
        for plan in ev_wiring.iter_mut().flatten() {
            plan.cursors = vec![0; plan.srcs.len() + 1]; // +1: the host-injection rank
        }
        // Event fan-in is free but its capacity is not: reserve the honest worst case — every
        // source full, plus a full host queue — so the merge on the audio path never allocates.
        let host_events: Vec<Vec<Vec<Event>>> = (0..slots.len())
            .map(|s| {
                (0..slots[s].2.ev_in.len())
                    .map(|p| {
                        Vec::with_capacity((ev_wiring[s][p].srcs.len() + 1) * EVENTS_PER_BLOCK)
                    })
                    .collect()
            })
            .collect();

        // ---- buffers, allocated and touched now, never on the audio path (decision 4)
        // The port maps survive the slot consumption: runtime accessors (host events, cv taps,
        // event reads) address ports by manifest index, and this is the one place the two index
        // spaces meet.
        let plans: Vec<NodePlan> = slots.iter().map(|(_, _, p)| p.clone()).collect();
        let mut nodes: Vec<ExecNode> = Vec::with_capacity(slots.len());
        let mut budget: usize = 0;
        for (slot, (id, build, plan)) in slots.into_iter().enumerate() {
            let ports = build.manifest.ports();
            let mut out_bufs: Vec<Vec<f32>> = Vec::with_capacity(plan.audio_out.len());
            for (p, _) in plan.audio_out.iter().enumerate() {
                let mut b = vec![0.0f32; frames * out_chs[slot][p]];
                for x in b.iter_mut() {
                    *x = std::hint::black_box(0.0); // pre-touch: page faults belong to build
                }
                budget += b.len() * std::mem::size_of::<f32>();
                out_bufs.push(b);
            }
            let mut audio_in: Vec<Vec<f32>> = Vec::with_capacity(plan.audio_in.len());
            for (p, _) in plan.audio_in.iter().enumerate() {
                let mut b = vec![0.0f32; frames * in_chs[slot][p]];
                for x in b.iter_mut() {
                    *x = std::hint::black_box(0.0);
                }
                budget += b.len() * std::mem::size_of::<f32>();
                audio_in.push(b);
            }
            // cv storage, aligned per port (a dead cell/empty vec for the other rate keeps the
            // assembly loop a straight zip — the waste is 4 bytes per port, the clarity is total).
            let cv_out_rates: Vec<CvRate> = plan
                .cv_out
                .iter()
                .map(|&i| ports[i as usize].cv_rate.unwrap_or(CvRate::Block))
                .collect();
            let cv_in_rates: Vec<CvRate> = plan
                .cv_in
                .iter()
                .map(|&i| ports[i as usize].cv_rate.unwrap_or(CvRate::Block))
                .collect();
            let mut cv_out_cells = vec![0.0f32; cv_out_rates.len()];
            let mut cv_out_bufs: Vec<Vec<f32>> = Vec::with_capacity(cv_out_rates.len());
            for r in &cv_out_rates {
                let mut b = if *r == CvRate::Audio { vec![0.0f32; frames] } else { Vec::new() };
                for x in b.iter_mut() {
                    *x = std::hint::black_box(0.0);
                }
                budget += b.len() * std::mem::size_of::<f32>();
                cv_out_bufs.push(b);
            }
            let mut cv_cells = vec![0.0f32; cv_in_rates.len()];
            let mut cv_bufs: Vec<Vec<f32>> = Vec::with_capacity(cv_in_rates.len());
            for r in &cv_in_rates {
                let mut b = if *r == CvRate::Audio { vec![0.0f32; frames] } else { Vec::new() };
                for x in b.iter_mut() {
                    *x = std::hint::black_box(0.0);
                }
                budget += b.len() * std::mem::size_of::<f32>();
                cv_bufs.push(b);
            }
            let ev_out: Vec<EventBuf> = (0..plan.ev_out.len()).map(|_| EventBuf::new()).collect();
            budget += ev_out.len() * std::mem::size_of::<EventBuf>();
            let mut events: Vec<Vec<Event>> = Vec::with_capacity(plan.ev_in.len());
            for w in &ev_wiring[slot] {
                let cap = (w.srcs.len() + 1) * EVENTS_PER_BLOCK;
                let mut v: Vec<Event> = Vec::with_capacity(cap);
                // Pre-touch the reservation: fill to capacity, then clear (capacity survives).
                v.resize(cap, Event::clock(0));
                for x in v.iter_mut() {
                    *x = std::hint::black_box(*x);
                }
                v.clear();
                budget += cap * std::mem::size_of::<Event>();
                events.push(v);
            }
            for x in cv_out_cells.iter_mut().chain(cv_cells.iter_mut()) {
                *x = std::hint::black_box(0.0);
            }
            budget += (cv_out_cells.len() + cv_cells.len()) * std::mem::size_of::<f32>();

            nodes.push(ExecNode {
                id,
                module: build.module,
                params: build.params,
                out_bufs,
                in_chs: in_chs[slot].clone(),
                out_chs: out_chs[slot].clone(),
                cv_in_rates,
                cv_in_connected: cv_in_connected[slot].clone(),
                cv_out_rates,
                cv_out_cells,
                cv_out_bufs,
                ev_out,
                status: BlockStatus::Ok,
                failed_blocks: 0,
                overrun_blocks: 0,
                overrun_streak: 0,
                auto_bypassed: false,
                bypassed_blocks: 0,
                inputs: InStaging { audio: audio_in, cv_cells, cv_bufs, events },
            });
        }
        for d in &delays {
            budget += d.hist.len() * std::mem::size_of::<f32>();
        }
        budget += budget_comp;

        // ---- prepare + activate in execution order (module-api §2 cascade rule)
        for n in nodes.iter_mut() {
            let mut ain = [0usize; MAX_PORTS_PER_CLASS];
            let mut aout = [0usize; MAX_PORTS_PER_CLASS];
            for (i, c) in n.in_chs.iter().take(MAX_PORTS_PER_CLASS).enumerate() {
                ain[i] = *c;
            }
            for (i, c) in n.out_chs.iter().take(MAX_PORTS_PER_CLASS).enumerate() {
                aout[i] = *c;
            }
            let res = Resources {
                sample_rate: cfg.sample_rate,
                block_frames: frames,
                input_channels: n.in_chs.first().copied().unwrap_or(0),
                output_channels: n.out_chs.first().copied().unwrap_or(0),
                audio_in_channels: ain,
                audio_in_count: n.in_chs.len(),
                audio_out_channels: aout,
                audio_out_count: n.out_chs.len(),
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
            plans,
            nodes,
            wiring,
            cv_wiring,
            ev_wiring,
            host_events,
            delays,
            meters,
            ctx: BlockContext::offline(cfg.sample_rate, frames, cfg.device_channels),
            musical: None,
            budget_bytes: budget,
            blocks_rendered: 0,
            // Reserved, not filled: the audio path may push into this, and a push inside
            // capacity never allocates. Beyond the cap events are counted as dropped.
            wd_events: Vec::with_capacity(WATCHDOG_EVENT_CAP),
            wd_dropped: 0,
        })
    }

    /// Render one block and copy the master node's FIRST audio output into `out`
    /// (`block_frames × device_channels` samples, interleaved).
    ///
    /// Audio-thread safe: no allocation, no locks, no syscalls — the counting-allocator test
    /// measures the claim, over cv and event wires as well as audio ones. Errors here are
    /// configuration errors (wrong master, wrong buffer size) and are decided before any module
    /// runs.
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
        let master_ch = self.nodes[master_slot].out_chs.first().copied().unwrap_or(0);
        if master_ch != dev && master_ch != 1 && dev != 1 && master_ch != 0 {
            return Err(ExecError::ChannelNegotiation {
                edge: sparq_kernel::graph::EdgeId(u32::MAX),
                from: master_ch,
                to: dev,
            });
        }
        // The musical clock, if the driver feeds one (WO-009): the transport owns the map; the
        // executor's job is to hand every module the SAME position for the whole block (§9's
        // immutability), which is exactly one field copy.
        if let Some((tick, ppqn)) = self.musical {
            self.ctx.tick = tick;
            self.ctx.ppqn = ppqn;
        }

        let wd_limit = self.cfg.watchdog.consecutive_overruns;
        for slot in 0..self.order.len() {
            // -- take this node's input staging out: sources are read from `nodes` below, and a
            //    local destination cannot conflict with a borrowed field.
            let mut bundle = std::mem::take(&mut self.nodes[slot].inputs);

            // -- wire (audio): zero the connected inputs, then apply every plan in edge order
            //    (the fan-in sum is deterministic: first edge writes, later edges add).
            for b in bundle.audio.iter_mut() {
                for s in b.iter_mut() {
                    *s = 0.0;
                }
            }
            for plan in &mut self.wiring[slot] {
                let in_ch = self.nodes[slot].in_chs[plan.dst_port];
                if in_ch == 0 {
                    continue;
                }
                let (add, src_ch, dst_port) = (plan.add, plan.src_ch, plan.dst_port);
                match (plan.kind, plan.delay) {
                    (EdgeKind::BlockDelay, Some(d)) => {
                        let hist = &self.delays[d].hist;
                        let dst = &mut bundle.audio[dst_port];
                        write_edge(dst, in_ch, hist, src_ch, add, None);
                    },
                    (EdgeKind::UnitDelay, Some(d)) => {
                        // Both borrows are immutable (the held memory refreshes at block
                        // end), so no copy is needed — and a clone here would allocate on the
                        // audio path, which the counting test would rightly fail.
                        let held: &[f32] = &self.delays[d].hist;
                        let src = &self.nodes[plan.src_slot].out_bufs[plan.src_port];
                        let dst = &mut bundle.audio[dst_port];
                        write_edge(dst, in_ch, src, src_ch, add, Some(held));
                    },
                    _ => {
                        if let Some((line, scratch)) = plan.comp.as_mut() {
                            // Fan-in compensation (task 5): copy the source block into the
                            // pre-allocated scratch, transform it in place through the edge's
                            // delay line, and wire FROM the delayed signal. Scratch and ring
                            // were sized at build; the audio path allocates nothing.
                            let src = &self.nodes[plan.src_slot].out_bufs[plan.src_port];
                            let n = scratch.len().min(src.len());
                            scratch[..n].copy_from_slice(&src[..n]);
                            line.apply(&mut scratch[..n]);
                            let dst = &mut bundle.audio[dst_port];
                            write_edge(dst, in_ch, &scratch[..n], src_ch, add, None);
                        } else {
                            let src = &self.nodes[plan.src_slot].out_bufs[plan.src_port];
                            let dst = &mut bundle.audio[dst_port];
                            write_edge(dst, in_ch, src, src_ch, add, None);
                        }
                    },
                }
            }

            // -- wire (cv): the receiving port's declared rate is what the module sees; the host
            //    performs any rate change here (G3's reduce, G4's expand), in build-planned
            //    storage. The source ran earlier in the order, so its outputs are this block's.
            for plan in &mut self.cv_wiring[slot] {
                let (dp, sp, ss) = (plan.dst_port, plan.src_port, plan.src_slot);
                match (plan.src_rate, plan.dst_rate) {
                    (CvRate::Block, CvRate::Block) => {
                        bundle.cv_cells[dp] = self.nodes[ss].cv_out_cells[sp];
                    },
                    (CvRate::Audio, CvRate::Block) => {
                        let src = &self.nodes[ss].cv_out_bufs[sp];
                        bundle.cv_cells[dp] = plan.reduce.apply(src);
                    },
                    (CvRate::Block, CvRate::Audio) => {
                        let cur = self.nodes[ss].cv_out_cells[sp];
                        let (prev, interp) = (plan.prev, plan.interp);
                        interp.expand(prev, cur, &mut bundle.cv_bufs[dp]);
                        // The ramp's start point for the next block is this block's value —
                        // executor state, refreshed on the audio path without allocating.
                        plan.prev = cur;
                    },
                    (CvRate::Audio, CvRate::Audio) => {
                        let src = &self.nodes[ss].cv_out_bufs[sp];
                        let dst = &mut bundle.cv_bufs[dp];
                        let n = dst.len().min(src.len());
                        dst[..n].copy_from_slice(&src[..n]);
                    },
                }
            }

            // -- wire (event): merge every source of each event-in port into one list pre-sorted
            //    by sample offset (module-api §9's guarantee). Ranks: the host queue first, then
            //    the edges in EdgeId order, then insertion order within a source — the matrix's
            //    G5 tie-break, made mechanical. Each source list is already sorted (sinks sort
            //    at their producer's dispatch; the host queue sorts at push), so a linear k-way
            //    merge is stable, allocation-free (the destination is reserved at build) and
            //    bounded by (sources + 1) × EVENTS_PER_BLOCK.
            for dp in 0..bundle.events.len() {
                let plan = &mut self.ev_wiring[slot][dp];
                let dst = &mut bundle.events[dp];
                dst.clear();
                for c in plan.cursors.iter_mut() {
                    *c = 0;
                }
                let host: &[Event] = &self.host_events[slot][dp];
                loop {
                    // Pick the smallest (sample, rank) head; strictly-smaller keeps equal keys
                    // in cursor order, which IS insertion order within a list.
                    let mut best: Option<(u32, usize, Event)> = None;
                    if plan.cursors[0] < host.len() {
                        let e = host[plan.cursors[0]];
                        best = Some((e.sample, 0, e));
                    }
                    for (rank, &(ss, sp)) in plan.srcs.iter().enumerate() {
                        let cur = plan.cursors[rank + 1];
                        let sink = self.nodes[ss].ev_out[sp].as_slice();
                        if cur < sink.len() {
                            let e = sink[cur];
                            let key = (e.sample, rank + 1);
                            // `map_or`, not `is_none_or`: the workspace MSRV is 1.80 (clippy's
                            // incompatible_msrv lint is the gate that says so).
                            if best.map_or(true, |(bs, br, _)| key < (bs, br)) {
                                best = Some((e.sample, rank + 1, e));
                            }
                        }
                    }
                    let Some((_, rank, ev)) = best else { break };
                    dst.push(ev);
                    plan.cursors[rank] += 1;
                }
            }
            // The queue is consumed exactly once: cleared after the merge, so a host event lands
            // in exactly one block (sample-accurate dispatch by TICK is WO-009's clock work).
            for q in self.host_events[slot].iter_mut() {
                q.clear();
            }

            // -- sinks are cleared before the dispatch they emit into (per-block drops reset;
            //    lifetime totals survive — the count is the evidence, not the block).
            for s in self.nodes[slot].ev_out.iter_mut() {
                s.clear();
            }

            // -- process: one trait-object dispatch per node per block (ADR-009 decision 7) —
            //    unless the watchdog isolated this node (task 6). A bypassed module is NEVER
            //    CALLED — the staller cannot stall the block — and its outputs become
            //    passthrough when the shapes match (per audio port), silence otherwise; cv
            //    outputs publish 0.0 and event sinks stay empty, so downstream modules see an
            //    explicit "nothing", never a stale value. The flag, the meter and the journal
            //    all say so: the isolation is visible, never mysterious.
            let status = if self.nodes[slot].auto_bypassed {
                let n = &mut self.nodes[slot];
                let paired = n.in_chs.len().min(n.out_chs.len());
                for p in 0..n.out_bufs.len() {
                    if p < paired
                        && n.in_chs[p] == n.out_chs[p]
                        && n.in_chs[p] > 0
                        && bundle.audio[p].len() == n.out_bufs[p].len()
                    {
                        n.out_bufs[p].copy_from_slice(&bundle.audio[p]);
                    } else {
                        for s in n.out_bufs[p].iter_mut() {
                            *s = 0.0;
                        }
                    }
                }
                for c in n.cv_out_cells.iter_mut() {
                    *c = 0.0;
                }
                for b in n.cv_out_bufs.iter_mut() {
                    for s in b.iter_mut() {
                        *s = 0.0;
                    }
                }
                n.bypassed_blocks += 1;
                BlockStatus::Ok
            } else {
                // Assemble the contract-v1 context from disjoint borrows: the staging (local),
                // the parameter snapshot and the output-side storage (fields of `n`). The
                // per-type port order is the manifest order the module was written against.
                let n = &mut self.nodes[slot];
                let mut ctx = AudioCtx::new(&self.ctx, &n.params);
                for b in bundle.audio.iter() {
                    ctx = ctx.with_audio_in(b);
                }
                for b in n.out_bufs.iter_mut() {
                    ctx = ctx.with_audio_out(b);
                }
                for (i, r) in n.cv_in_rates.iter().enumerate() {
                    let view = if !n.cv_in_connected[i] {
                        CvIn::Unconnected
                    } else {
                        match r {
                            CvRate::Block => CvIn::Block(bundle.cv_cells[i]),
                            CvRate::Audio => CvIn::Audio(&bundle.cv_bufs[i]),
                        }
                    };
                    ctx = ctx.with_cv_in(view);
                }
                for (r, (cell, buf)) in n
                    .cv_out_rates
                    .iter()
                    .zip(n.cv_out_cells.iter_mut().zip(n.cv_out_bufs.iter_mut()))
                {
                    ctx = match r {
                        CvRate::Block => ctx.with_cv_out(CvOut::Block(cell)),
                        CvRate::Audio => ctx.with_cv_out(CvOut::Audio(&mut buf[..])),
                    };
                }
                for b in bundle.events.iter() {
                    ctx = ctx.with_events_in(b);
                }
                for b in n.ev_out.iter_mut() {
                    ctx = ctx.with_event_out(b);
                }
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
                        for b in n.out_bufs.iter_mut() {
                            for s in b.iter_mut() {
                                *s = 0.0;
                            }
                        }
                        for c in n.cv_out_cells.iter_mut() {
                            *c = 0.0;
                        }
                        for b in n.cv_out_bufs.iter_mut() {
                            for s in b.iter_mut() {
                                *s = 0.0;
                            }
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
                //    audio outputs; edge compensation lives in the WIRING pass (task 5), so a
                //    meter shows what the module made, and the aligned sum is what destinations
                //    get. Contract v1: ALL audio outputs of the node fold into one reading.
                let (peak, rms) = peak_rms_ports(&n.out_bufs);
                self.meters[slot].peak.store(peak.to_bits(), Ordering::Relaxed);
                self.meters[slot].rms.store(rms.to_bits(), Ordering::Relaxed);
                self.meters[slot].status.store(status_to_u8(status), Ordering::Relaxed);
                // Publish sorted: the downstream merge assumes every sink is ordered by sample,
                // stable within equal samples (the insertion-order half of G5's tie-break).
                for s in n.ev_out.iter_mut() {
                    s.sort_by_sample();
                }
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

            self.nodes[slot].inputs = bundle;
        }

        // -- block end: refresh the delay memories from the sources' finished outputs.
        for d in 0..self.delays.len() {
            let (src_slot, src_port, kind) =
                (self.delays[d].src_slot, self.delays[d].src_port, self.delays[d].kind);
            let Some(src) = self.nodes[src_slot].out_bufs.get(src_port) else { continue };
            let hist = &mut self.delays[d].hist;
            if kind == EdgeKind::BlockDelay {
                if hist.len() == src.len() {
                    hist.copy_from_slice(src);
                }
            } else if let Some(last) = src.len().checked_sub(hist.len()) {
                hist.copy_from_slice(&src[last..]);
            }
        }

        // -- master render: the node's FIRST audio output, converted to the device shape. A
        // master with no audio output at all renders silence — an empty device buffer is never
        // left un-written.
        let (m_out, m_ch) = {
            let n = &self.nodes[master_slot];
            (
                n.out_bufs.first().map(|b| &b[..]).unwrap_or(&[]),
                n.out_chs.first().copied().unwrap_or(0),
            )
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

    /// Set the musical position the next rendered blocks carry: `(tick at the block's first
    /// frame, ppqn)`. This is the transport's door (WO-009): the driver calls it per block with
    /// what `sparq-music` computed — the executor performs no musical math of its own, and a
    /// driver that never calls it renders the declared static-clock behaviour (`tick` stays 0,
    /// which is what the determinism harness and every golden were built against).
    /// `None` freezes the tick at its last value.
    pub fn set_musical_position(&mut self, tick_ppqn: Option<(u64, u32)>) {
        self.musical = tick_ppqn;
    }

    /// Queue one host-side event for a node's `event` INPUT port (manifest port index — the
    /// kernel `PortRef` convention). Control thread, between blocks; the event is delivered in
    /// the NEXT rendered block, merged at rank 0 (before every module source, per G5's
    /// connection-order tie-break with the host first — declared in the executor's header).
    /// WO-009's transport publishes through this same door; nothing here reads a clock.
    ///
    /// # Errors
    /// [`ExecError::Graph`] for an unknown node, [`ExecError::Unsupported`] when the port is not
    /// an event input or when this block's queue is full ([`EVENTS_PER_BLOCK`] — counted in
    /// words, never silently dropped).
    pub fn push_host_event(&mut self, node: NodeId, port: u32, ev: Event) -> Result<(), ExecError> {
        let Some(&slot) = self.slot_of.get(&node) else {
            return Err(ExecError::Graph(GraphError::NoSuchNode(node)));
        };
        let Some(idx) = NodePlan::index_in(&self.plans[slot].ev_in, port) else {
            return Err(ExecError::Unsupported {
                node: Some(node),
                why: format!(
                    "port index {port} is not an event input of this module — host events enter \
                     through declared event-in ports only"
                ),
            });
        };
        let q = &mut self.host_events[slot][idx];
        if q.len() >= EVENTS_PER_BLOCK {
            return Err(ExecError::Unsupported {
                node: Some(node),
                why: format!(
                    "the host event queue for the next block is full ({EVENTS_PER_BLOCK} \
                     events) — schedule fewer events per block or spread them across blocks"
                ),
            });
        }
        // Sorted insert keeps the queue's own order invariant (strictly-after equals ⇒ stable).
        // Inside the reserved capacity, so the control-side insert cannot reallocate either.
        let pos = q.partition_point(|e| e.sample <= ev.sample);
        q.insert(pos, ev);
        Ok(())
    }

    /// A node's most recent FIRST audio output buffer (control thread, after a block) — the
    /// analysis-tap read until the lock-free rings land. `None` for an unknown node or a node
    /// with no audio output port (contract v1: a cv source publishes on its cv port, read it
    /// with [`Executor::node_cv_block`] / [`Executor::node_cv_audio`]).
    #[must_use]
    pub fn node_output(&self, node: NodeId) -> Option<&[f32]> {
        self.slot_of.get(&node).and_then(|&s| self.nodes[s].out_bufs.first().map(|b| &b[..]))
    }

    /// The audio output buffer of one port (manifest port index), after a block.
    #[must_use]
    pub fn node_audio_out(&self, node: NodeId, port: u32) -> Option<&[f32]> {
        let &slot = self.slot_of.get(&node)?;
        let idx = NodePlan::index_in(&self.plans[slot].audio_out, port)?;
        self.nodes[slot].out_bufs.get(idx).map(|b| &b[..])
    }

    /// The most recent value a node published on a BLOCK-rate `cv` output (manifest port
    /// index). `None` for an unknown node/port, or a port of another rate — the two cv shapes
    /// have two readers, because a value and a buffer are different claims.
    #[must_use]
    pub fn node_cv_block(&self, node: NodeId, port: u32) -> Option<f32> {
        let &slot = self.slot_of.get(&node)?;
        let idx = NodePlan::index_in(&self.plans[slot].cv_out, port)?;
        if self.nodes[slot].cv_out_rates.get(idx) == Some(&CvRate::Block) {
            self.nodes[slot].cv_out_cells.get(idx).copied()
        } else {
            None
        }
    }

    /// The most recent buffer a node published on an AUDIO-rate `cv` output (manifest port
    /// index): one value per frame of the last block.
    #[must_use]
    pub fn node_cv_audio(&self, node: NodeId, port: u32) -> Option<&[f32]> {
        let &slot = self.slot_of.get(&node)?;
        let idx = NodePlan::index_in(&self.plans[slot].cv_out, port)?;
        if self.nodes[slot].cv_out_rates.get(idx) == Some(&CvRate::Audio) {
            self.nodes[slot].cv_out_bufs.get(idx).map(|b| &b[..])
        } else {
            None
        }
    }

    /// The events a node emitted on one `event` OUTPUT port (manifest port index) during the
    /// most recent block, sorted by sample offset. Empty when the module emitted nothing.
    #[must_use]
    pub fn node_events(&self, node: NodeId, port: u32) -> Option<&[Event]> {
        let &slot = self.slot_of.get(&node)?;
        let idx = NodePlan::index_in(&self.plans[slot].ev_out, port)?;
        self.nodes[slot].ev_out.get(idx).map(EventBuf::as_slice)
    }

    /// Lifetime total of event pushes refused on one `event` OUTPUT port because the block's
    /// [`EVENTS_PER_BLOCK`] capacity was spent — counted, never silently dropped. A port that
    /// drops every block is lying about its event rate, and this is where the host can see it.
    #[must_use]
    pub fn event_drops_total(&self, node: NodeId, port: u32) -> Option<u64> {
        let &slot = self.slot_of.get(&node)?;
        let idx = NodePlan::index_in(&self.plans[slot].ev_out, port)?;
        self.nodes[slot].ev_out.get(idx).map(EventBuf::dropped_total)
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
        // The musical position is the stream's, like the sample clock (task 4's own sentence):
        // a patch swap mid-playback must not reset the transport's idea of where the music is.
        self.musical = from.musical;
    }

    /// Blocks rendered since build.
    #[must_use]
    pub fn blocks_rendered(&self) -> u64 {
        self.blocks_rendered
    }

    /// The audio buffer budget in bytes (ADR-009 decision 4: printed at patch load). Contract
    /// v1: every carried payload's storage counts — audio buffers, cv cells and wire buffers,
    /// event sinks and merge reservations, delay histories and compensation lines.
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

/// Peak and RMS over ALL of a node's audio output buffers (contract v1's meter reading): one
/// f64 accumulator across every port (plan §5.4's rule for summing paths), so a multi-output
/// node meters as the sum of what it made. Empty → zeros; no allocation. For the single-output
/// node — every node Phase 0 ships — this is bit-identical to increment 3's single-buffer
/// reading, which is what keeps the golden meters where they were.
fn peak_rms_ports(bufs: &[Vec<f32>]) -> (f32, f32) {
    let mut peak = 0.0f32;
    let mut sum = 0.0f64;
    let mut n = 0usize;
    for b in bufs {
        for &s in b.iter() {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            sum += f64::from(s) * f64::from(s);
            n += 1;
        }
    }
    let rms = if n == 0 { 0.0 } else { (sum / n as f64).sqrt() as f32 };
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
        assert_eq!(peak_rms_ports(&[]), (0.0, 0.0), "no outputs: an explicit zero, not a panic");
        let one = vec![0.5f32, -0.5, 0.5, -0.5];
        let (p, r) = peak_rms_ports(&[one]);
        assert_eq!(p, 0.5);
        assert!((r - 0.5).abs() < 1e-7);
    }

    #[test]
    fn peak_rms_ports_folds_every_output() {
        let one = vec![0.5f32, -0.5, 0.5, -0.5];
        let two = vec![1.0f32, 0.0];
        let (p, r) = peak_rms_ports(&[one, two]);
        assert_eq!(p, 1.0);
        // sqrt((4×0.25 + 1.0 + 0.0)/6) = sqrt(2/6)
        assert!((r - (2.0f64 / 6.0).sqrt() as f32).abs() < 1e-7);
    }

    #[test]
    fn concrete_rejects_variable() {
        assert_eq!(concrete(Some(ChannelSet::Stereo)), Some(2));
        assert_eq!(concrete(Some(ChannelSet::Variable)), None);
        assert_eq!(concrete(None), None);
    }
}
