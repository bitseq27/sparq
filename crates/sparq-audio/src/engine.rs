//! The boundary-swap engines (WO-008 task 4 + increment 5): one live executor, one staged
//! replacement, and a single moment where they meet — the block boundary.
//!
//! ADR-009 decision 3, in two shapes:
//!
//! * [`Engine`] — the **single-owner** engine (task 4). The swap is `Option::take` at the top of
//!   [`Engine::render_block`]; the retired executor drops at that same point, and `ExecNode`'s
//!   `Drop` deactivates its modules: retirement is deterministic, not deferred, because in this
//!   shape nothing else can still be inside the old executor. This is the shape the determinism
//!   harness and the 10 000-mutation stress (`tests/mutation_stress.rs`) drive: boundary
//!   semantics, allocation-gated and hash-deterministic, with no second thread.
//! * [`SharedEngine`] + [`AudioEngine`] — the **cross-thread** engine (increment 5), built on the
//!   kernel's hot-swap primitive (`sparq_kernel::sync::hotswap`: decision 3's literal pointer
//!   swap with an epoch-based grace period — allowlisted-`unsafe`, Miri-run). The control thread
//!   builds complete patches and stages them; the audio thread takes them at boundaries and
//!   retires the outgoing patch back to the control thread, which drops it — so module
//!   deactivation never runs inside a device callback. Meters are published per block through a
//!   lock-free ring (decision 8: *published, not polled*), and a command ring carries
//!   between-block edits (params, musical position, bypass clears) to the patch that renders the
//!   next block. The two-thread acceptance is `tests/cross_thread.rs`: 10 000 seeded mutations
//!   staged while the audio thread renders continuously — zero failed blocks, zero audio-thread
//!   allocations, every retirement reclaimed and dropped exactly once. The paced zero-xrun half
//!   of "while playing" stays device-track (the HAL's loaded soak), exactly as the WO schedules
//!   it.
//!
//! What the swap carries, and the difference the operator's round-4 D1 ruling made. The
//! SINGLE-OWNER [`Engine`] still carries only the transport clock and the block count
//! ([`Executor::inherit_runtime`]): a swapped-in module starts from its `prepare`/`activate`
//! state, because that engine is the determinism harness's, and a stress hash that depended on
//! instance adoption would be pinning the wrong thing. The CROSS-THREAD engine — the live
//! session's path — additionally ADOPTS: at the boundary hook, the successor takes the
//! predecessor's live module instances for every node whose canvas identity and version hash
//! match ([`Executor::adopt_runtime`]), so adding a module to a playing patch no longer resets
//! the modules the user was already hearing. Parameters deliberately do NOT ride the adoption
//! (the staged build's canvas snapshot rules), and the timeline still inherits as before,
//! because the clock belongs to the stream, not to the patch. The shapes differ in WHEN this
//! runs: [`Engine::stage`] inherits at staging time (control side, single thread); the
//! cross-thread engine adopts and inherits at the swap point itself (audio side, inside the
//! boundary — the successor adopts from the predecessor as it actually stands).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::executor::{status_to_u8, ExecError, Executor};
use sparq_kernel::graph::NodeId;
use sparq_kernel::sync::{HotSwap, HotSwapAudio, HotSwapControl, SpscRing, SwapStats};
use sparq_module_api::module::BlockStatus;
use sparq_module_api::params::ParamSet;

/// A live executor with a staging slot for its replacement.
#[derive(Debug)]
pub struct Engine {
    live: Executor,
    staged: Option<Executor>,
    swaps: u64,
    superseded: u64,
}

impl Engine {
    /// An engine around a freshly built executor.
    #[must_use]
    pub fn new(live: Executor) -> Self {
        Self { live, staged: None, swaps: 0, superseded: 0 }
    }

    /// Stage a replacement for the next block boundary (control side).
    ///
    /// The successor adopts the live executor's transport clock and block count *here*, at
    /// staging time, so the swap itself is a pure handover. Staging twice before a boundary
    /// retires the first successor unused (counted by [`Engine::superseded_stages`]) — the
    /// timeline never skips: what renders next is always exactly one boundary away.
    pub fn stage(&mut self, mut next: Executor) {
        next.inherit_runtime(&self.live);
        if self.staged.is_some() {
            self.superseded += 1;
        }
        self.staged = Some(next);
    }

    /// Render one block: apply the staged swap first (if any), then render.
    ///
    /// The swap point is the boundary ADR-009 d3 names — no sample of the new block is computed
    /// against the old graph, and no sample of the old block ever met the new one. The old
    /// executor drops here (modules deactivated through `ExecNode::Drop`); on a single-owner
    /// engine that retirement point is exact.
    ///
    /// # Errors
    /// Whatever [`Executor::render_block`] reports — note that a swap which removed the master
    /// node surfaces here as `NoSuchNode`, in words, rather than rendering silence: staging a
    /// patch without the node the listener hears is a control-side bug and is reported as one.
    pub fn render_block(&mut self, master: NodeId, out: &mut [f32]) -> Result<(), ExecError> {
        if let Some(next) = self.staged.take() {
            self.live = next;
            self.swaps += 1;
        }
        self.live.render_block(master, out)
    }

    /// The executor that will render the next block (after any staged swap).
    #[must_use]
    pub fn live(&self) -> &Executor {
        &self.live
    }

    /// Mutable access to the live executor — control-side reads and edits (`set_params`,
    /// `clear_auto_bypass`) between blocks. Never called from inside a render.
    pub fn live_mut(&mut self) -> &mut Executor {
        &mut self.live
    }

    /// Whether a replacement is waiting for the next boundary.
    #[must_use]
    pub fn is_staged(&self) -> bool {
        self.staged.is_some()
    }

    /// How many boundary swaps have happened.
    #[must_use]
    pub fn swaps(&self) -> u64 {
        self.swaps
    }

    /// How many staged successors were replaced before ever going live (each one retired
    /// unused, its modules deactivated without a single block — counted, never silent).
    #[must_use]
    pub fn superseded_stages(&self) -> u64 {
        self.superseded
    }

    /// Blocks rendered across all swaps — the clock the swap inherits, made observable.
    #[must_use]
    pub fn blocks_rendered(&self) -> u64 {
        self.live.blocks_rendered()
    }
}

// =========================================================================================
// The cross-thread engine (WO-008 increment 5): ADR-009 d3's swap over the kernel primitive,
// d8's meter publication, and a bounded command ring for between-block edits.
// =========================================================================================

/// The live payload: an executor plus the master node the device listens to.
///
/// A staged patch carries its own master — the listener's node is part of "what renders next",
/// not a render-time argument, so the audio thread never has to be told twice and can never
/// disagree with itself about which node is the output.
#[derive(Debug)]
pub struct LivePatch {
    /// The built, runnable patch.
    pub executor: Executor,
    /// The node whose output the device renders.
    pub master: NodeId,
}

/// One meter reading, published per block per audio OUTPUT port (ADR-009 decision 8: analysis
/// taps and meters are PUBLISHED, not polled — the audio thread writes into a lock-free ring and
/// the UI/visuals consume; visual work never touches the audio thread). A node with no audio
/// outputs publishes ONE [`MeterUpdate::FOLDED`] entry so its status still crosses the ring.
///
/// `Copy` and fixed-size so it rides the kernel's [`SpscRing`] without allocation. This is the
/// reader the live wire levels consume — offline through the executor's folded [`crate::executor::
/// Executor::meter`], live through this ring's per-port entries.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MeterUpdate {
    /// The node this reading belongs to.
    pub node: u32,
    /// The manifest port index of the AUDIO OUTPUT this reading belongs to — the ring's port
    /// id (WO-012 increment 2: per-port meters, so one wire out of a multi-output node can
    /// carry its own level instead of the node's fold). [`MeterUpdate::FOLDED`] for a node
    /// with no audio outputs at all, whose status must not vanish from the ring.
    pub port: u32,
    /// The block count the reading was taken at (the stream's clock, inherited across swaps).
    pub block: u64,
    /// Peak |sample| across the port's channels in that block (the folded reading for a
    /// [`MeterUpdate::FOLDED`] entry: zeros, with the node's status).
    pub peak: f32,
    /// The FIRST channel's peak in that block — the left meter bar (WO-012 increment 4). A
    /// mono port duplicates `peak`; the `FOLDED` entry keeps honest zeros.
    pub peak_l: f32,
    /// The SECOND channel's peak (the right meter bar); a mono port duplicates `peak_l`.
    pub peak_r: f32,
    /// RMS across the port's channels in that block (same f64 accumulator as the executor's
    /// folded meter — for a single-output node the two readings are bit-identical).
    pub rms: f32,
    /// The node's block status, in the executor's wire encoding — repeated on every port entry
    /// of the node, because status is a NODE fact and an entry that carried it conditionally
    /// would make every consumer ask which entry is the special one.
    pub status: u8,
}

impl MeterUpdate {
    /// The port value of a node with NO audio outputs: the entry carries the node's status and
    /// zeroed levels. A named sentinel, not a magic number at the call sites.
    pub const FOLDED: u32 = u32::MAX;

    /// The status as the contract's enum (the wire encoding exists only to keep this struct
    /// `Copy` and small enough for a ring slot).
    #[must_use]
    pub fn block_status(&self) -> BlockStatus {
        crate::executor::status_from_u8(self.status)
    }
}

/// The waveform length one [`AnalysisUpdate`] carries — one block at the standard 64-frame block,
/// with headroom to 128 so a larger host block still publishes its first 128 samples. Bounded and
/// fixed so the payload stays `Copy` and rides the kernel's [`SpscRing`] without allocation; a
/// longer block is truncated (the scope's UI accumulates blocks up to its timebase anyway), and
/// [`AnalysisUpdate::len`] says how many samples are valid.
pub const ANALYSIS_WAVE_LEN: usize = 128;

/// One node's published analysis waveform — ADR-009 decision 8's "published, not polled" extended
/// from [`MeterUpdate`] (peak/rms) to the SAMPLES a display needs. `Copy` and fixed-size so it
/// rides the kernel's [`SpscRing`] without allocation. This is the payload a `dsp/scope`'s UI
/// reads — off the ring, never off the audio thread and never off the executor — and the reason
/// `ana/tap`'s audio-rate `wave` port exists: the tap writes the waveform, the audio engine
/// publishes it here once per block, the visual thread draws it. Zero audio-thread cost on the
/// display side is structural: the display consumer is on the other end of this ring.
///
/// Published for EVERY `cv` output in the patch (see [`AudioEngine::publish_analysis`]) — the
/// generic rule needs no per-node kind storage, it is bounded by the ring depth, and it means any
/// cv signal (a tap's waveform, an envelope, an LFO) is scopable. An audio-rate port travels as
/// its block buffer; a block-rate port travels as a ONE-sample waveform (`len == 1`), so a
/// consumer's peak rule reads both rates the same way and a cv wire's live level needs no second
/// channel. A consumer filters by the `(node, port)` its display is bound to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalysisUpdate {
    /// The node that produced the waveform.
    pub node: u32,
    /// The manifest port index of the audio-rate `cv` output it came from.
    pub port: u32,
    /// The block count the waveform was captured at (the stream's clock, inherited across swaps).
    pub block: u64,
    /// How many samples of [`Self::samples`] are valid (≤ [`ANALYSIS_WAVE_LEN`]).
    pub len: u32,
    /// The waveform samples, mono, in block order.
    pub samples: [f32; ANALYSIS_WAVE_LEN],
}

impl AnalysisUpdate {
    /// An empty update for one `(node, port)` at `block`.
    #[must_use]
    pub fn new(node: u32, port: u32, block: u64) -> Self {
        Self { node, port, block, len: 0, samples: [0.0; ANALYSIS_WAVE_LEN] }
    }

    /// Fill from a waveform slice, truncated to [`ANALYSIS_WAVE_LEN`]; records the copied length.
    pub fn set_samples(&mut self, wave: &[f32]) {
        let n = wave.len().min(ANALYSIS_WAVE_LEN);
        self.samples[..n].copy_from_slice(&wave[..n]);
        self.len = n as u32;
    }

    /// Fill from an INTERLEAVED multi-channel buffer taking channel zero every `stride`
    /// samples (operator ruling 2026-10-01 r3: the scope accepts ANY source, so an audio
    /// output's waveform crosses the ring as its first channel — the scope is a mono
    /// picture of a bus, declared, not a hidden downmix of the whole set).
    pub fn set_samples_strided(&mut self, buf: &[f32], stride: usize) {
        let stride = stride.max(1);
        let mut n = 0usize;
        for (i, &s) in buf.iter().step_by(stride).enumerate() {
            if i >= ANALYSIS_WAVE_LEN {
                break;
            }
            self.samples[i] = s;
            n = i + 1;
        }
        self.len = n as u32;
    }

    /// The valid waveform samples.
    #[must_use]
    pub fn wave(&self) -> &[f32] {
        &self.samples[..(self.len as usize).min(ANALYSIS_WAVE_LEN)]
    }
}

impl Default for AnalysisUpdate {
    fn default() -> Self {
        Self::new(0, 0, 0)
    }
}

/// A control → audio command: the between-block edits the single-owner engine makes through
/// `live_mut`, crossing the thread boundary as `Copy` ring payloads instead (plan §4.3's
/// bounded rings; [`ParamSet`] was made `Copy` for exactly this trip).
///
/// Commands apply, in ring order, to the patch that is live AFTER the block's boundary swap —
/// one interpretation, no races about which patch an edit meant. A command that cannot apply
/// (unknown node, ...) is refused and counted ([`EngineStats::cmd_refused`]), never silent.
#[derive(Clone, Copy, Debug, Default)]
pub enum EngineCmd {
    /// The ring's default fill; carries no meaning and is not counted.
    #[default]
    Noop,
    /// Replace a node's parameter snapshot (control-side semantics, audio-side application).
    SetParams {
        /// The target node.
        node: u32,
        /// The snapshot to apply.
        params: ParamSet,
    },
    /// Set the musical position door (WO-009: the transport computes, the executor carries).
    SetMusical {
        /// Tick at the first frame of the next block.
        tick: u64,
        /// Ticks per quarter note.
        ppqn: u32,
    },
    /// Close the musical door: the context keeps the last position (a frozen clock, not a
    /// rewound one — resetting the timeline is the transport's decision, not a command's).
    ClearMusical,
    /// Clear a node's watchdog auto-bypass (ADR-009 d6's reversible rung).
    ClearAutoBypass {
        /// The target node.
        node: u32,
    },
}

/// Command-application counters, shared by the halves so the control side can see what the
/// audio side did. Counted, never silent (plan §4.3).
struct CmdCounters {
    applied: AtomicU64,
    refused: AtomicU64,
}

/// Everything the control side can observe about the cross-thread engine in one snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineStats {
    /// The kernel hot-swap slot's counters (epoch, swaps, superseded, deferred, retirements).
    pub swap: SwapStats,
    /// Commands the audio side applied.
    pub cmd_applied: u64,
    /// Commands the audio side refused (unknown node and friends) — counted, never silent.
    pub cmd_refused: u64,
    /// `send` calls refused because the command ring was full (the caller still holds the
    /// command; a retry is backpressure, not loss — see `SpscRing::refusals`).
    pub cmd_queue_refusals: u64,
    /// Meter publications refused because the meter ring was full (the UI fell behind; the
    /// audio thread never waits for a reader).
    pub meter_refusals: u64,
    /// Analysis publications refused because the analysis ring was full (the visual consumer fell
    /// behind; like meters, the audio thread never waits — a dropped waveform is a dropped frame
    /// of a scope, which is exactly what a real-time display is allowed to do).
    pub analysis_refusals: u64,
}

/// Default command-ring depth: a UI editing several params per frame stays far inside it, and a
/// burst beyond it is refused-and-counted rather than queued without bound.
pub const CMD_RING_SLOTS: usize = 256;
/// Default meter-ring depth: 4 096 updates ≈ 512 blocks × 8 nodes ≈ 0.7 s of unread meters at
/// 48 kHz/64 — a frame-rate consumer never comes close; an absent one wraps into counted
/// refusals instead of memory growth.
pub const METER_RING_SLOTS: usize = 4096;
/// Default analysis-ring depth. Analysis payloads are bigger than meters (a waveform each), so the
/// ring is shallower: 512 slots hold a few blocks' worth of every audio-rate cv output in a small
/// patch. A frame-rate visual consumer drains it every frame; an absent one wraps into counted
/// refusals (the audio thread never waits for a reader, exactly like the meter ring).
pub const ANALYSIS_RING_SLOTS: usize = 512;
/// Commands drained per block. The rest wait for the next boundary — bounded latency, bounded
/// stack, no allocation.
const CMD_BATCH: usize = 64;

/// The control-thread half of the cross-thread engine: stage complete patches, send
/// between-block commands, reclaim retired patches (and drop them HERE — off the audio
/// thread), consume published meters.
///
/// Every method is lock-free and allocation-free after construction. The discipline is one
/// control thread and one audio thread (the kernel primitive's contract); `&self` everywhere,
/// so a UI may hold references from several threads as long as ONE of them stages.
pub struct SharedEngine {
    control: HotSwapControl<LivePatch>,
    cmds: Arc<SpscRing<EngineCmd>>,
    meters: Arc<SpscRing<MeterUpdate>>,
    analysis: Arc<SpscRing<AnalysisUpdate>>,
    counters: Arc<CmdCounters>,
}

impl SharedEngine {
    /// Split a live patch into its two halves: the control side keeps the staging slot, the
    /// audio side takes the patch and the rings' other ends. Default ring depths
    /// ([`CMD_RING_SLOTS`], [`METER_RING_SLOTS`]).
    #[must_use]
    pub fn new(executor: Executor, master: NodeId) -> (Self, AudioEngine) {
        Self::with_capacities(executor, master, CMD_RING_SLOTS, METER_RING_SLOTS)
    }

    /// [`SharedEngine::new`] with explicit ring depths (a rig with 200 nodes wants a deeper
    /// meter ring than a two-node sketch; both depths are bounded either way).
    #[must_use]
    pub fn with_capacities(
        executor: Executor,
        master: NodeId,
        cmd_slots: usize,
        meter_slots: usize,
    ) -> (Self, AudioEngine) {
        Self::with_all_capacities(executor, master, cmd_slots, meter_slots, ANALYSIS_RING_SLOTS)
    }

    /// [`SharedEngine::new`] with every ring depth explicit, including the analysis ring. The
    /// analysis ring carries waveforms (bigger than meters), so a rig that scopes many taps at
    /// once may want it deeper; like the others it is bounded and refuses-and-counts when full.
    #[must_use]
    pub fn with_all_capacities(
        executor: Executor,
        master: NodeId,
        cmd_slots: usize,
        meter_slots: usize,
        analysis_slots: usize,
    ) -> (Self, AudioEngine) {
        let (control, audio) = HotSwap::split(LivePatch { executor, master });
        let cmds = Arc::new(SpscRing::new(cmd_slots));
        let meters = Arc::new(SpscRing::new(meter_slots));
        let analysis = Arc::new(SpscRing::new(analysis_slots));
        let counters =
            Arc::new(CmdCounters { applied: AtomicU64::new(0), refused: AtomicU64::new(0) });
        (
            Self {
                control,
                cmds: Arc::clone(&cmds),
                meters: Arc::clone(&meters),
                analysis: Arc::clone(&analysis),
                counters: Arc::clone(&counters),
            },
            AudioEngine {
                audio,
                cmds,
                meters,
                analysis,
                counters,
                cmd_batch: [EngineCmd::Noop; CMD_BATCH],
            },
        )
    }

    /// Stage a complete patch for the next block boundary (control side; building it is pure
    /// control-side work — [`Executor::build`] touches nothing live). Returns `true` when this
    /// stage SUPERSEDED a patch that was still waiting: the superseded patch is dropped here,
    /// immediately, and counted. The transport clock is NOT adopted here (the control thread
    /// may not touch the live patch) — the successor inherits it at the swap point, inside the
    /// boundary, from the predecessor as it actually stands.
    pub fn stage(&self, executor: Executor, master: NodeId) -> bool {
        self.control.stage(Box::new(LivePatch { executor, master }))
    }

    /// Queue a command for the audio side. `false` means the ring was full: the command is
    /// still the caller's, and the refusal is counted (plan §4.3 — backpressure is not loss,
    /// but it is never silent either).
    pub fn send(&self, cmd: EngineCmd) -> bool {
        self.cmds.push(cmd)
    }

    /// Convenience: queue a parameter snapshot for one node.
    pub fn set_params(&self, node: NodeId, params: ParamSet) -> bool {
        self.send(EngineCmd::SetParams { node: node.0, params })
    }

    /// Convenience: move (or close, with `None`) the musical position door — WO-009's
    /// transport publishing through the same ring everything else crosses.
    pub fn set_musical_position(&self, tick_ppqn: Option<(u64, u32)>) -> bool {
        match tick_ppqn {
            Some((tick, ppqn)) => self.send(EngineCmd::SetMusical { tick, ppqn }),
            None => self.send(EngineCmd::ClearMusical),
        }
    }

    /// Take the next retired patch whose grace period has passed, to drop HERE (module
    /// deactivation runs on this thread, never inside a device callback). Call in a loop to
    /// drain; `None` means nothing is reclaimable right now.
    #[must_use]
    pub fn reclaim(&self) -> Option<Box<LivePatch>> {
        self.control.reclaim()
    }

    /// Drain up to `out.len()` published meter updates (UI/visual side). Returns how many were
    /// read; allocation-free — the caller owns the destination.
    pub fn read_meters(&self, out: &mut [MeterUpdate]) -> usize {
        self.meters.drain(out)
    }

    /// Drain up to `out.len()` published analysis waveforms (the `dsp/scope` / visual side).
    /// Returns how many were read; allocation-free — the caller owns the destination. A scope
    /// keeps the newest waveform per `(node, port)` it is bound to and ignores the rest; the ring
    /// is the only place a display reads signal, so the audio thread is never touched.
    pub fn read_analysis(&self, out: &mut [AnalysisUpdate]) -> usize {
        self.analysis.drain(out)
    }

    /// A snapshot of every counter. Cheap enough to poll per UI frame.
    #[must_use]
    pub fn stats(&self) -> EngineStats {
        EngineStats {
            swap: self.control.stats(),
            cmd_applied: self.counters.applied.load(Ordering::Relaxed),
            cmd_refused: self.counters.refused.load(Ordering::Relaxed),
            cmd_queue_refusals: self.cmds.refusals(),
            meter_refusals: self.meters.refusals(),
            analysis_refusals: self.analysis.refusals(),
        }
    }
}

/// The audio-thread half of the cross-thread engine. Move it to the audio thread (it is
/// `Send`); call [`AudioEngine::render_block`] once per device block; hand it back with
/// [`AudioEngine::shutdown`] when the stream stops so the final patch drops where teardown
/// belongs.
///
/// The per-block protocol, in order: **boundary swap** (the staged patch goes live, inheriting
/// the transport clock) → **commands** (applied to the patch that will render THIS block) →
/// **render** → **meter publication**. No locks, no allocation, no blocking, no wall-clock
/// reads, and no payload ever drops on this thread.
pub struct AudioEngine {
    audio: HotSwapAudio<LivePatch>,
    cmds: Arc<SpscRing<EngineCmd>>,
    meters: Arc<SpscRing<MeterUpdate>>,
    analysis: Arc<SpscRing<AnalysisUpdate>>,
    counters: Arc<CmdCounters>,
    /// Pre-allocated drain target: bounded stack, never a fresh Vec per block.
    cmd_batch: [EngineCmd; CMD_BATCH],
}

impl AudioEngine {
    /// Render one block: boundary swap, command application, render, meter publication.
    ///
    /// # Errors
    /// Whatever the live executor's [`Executor::render_block`] reports — a swap that removed
    /// the master node surfaces here as `NoSuchNode`, in words, rather than rendering silence
    /// (staging a patch without the node the listener hears is a control-side bug and is
    /// reported as one).
    pub fn render_block(&mut self, out: &mut [f32]) -> Result<(), ExecError> {
        // 1. The boundary: take the staged patch if one is waiting. The hook is the
        //    inheritance point — RT-safe by construction: the adoption swaps instances and
        //    copies into build-allocated storage (nothing allocates), the inheritance is two
        //    field copies. ADOPT FIRST, THEN INHERIT (operator round 4, D1): the successor
        //    takes over the predecessor's live modules — the patch change stops resetting the
        //    performance — and then the stream's clock, which belongs to the swap either way.
        self.audio.boundary(|incoming, outgoing| {
            incoming.executor.adopt_runtime(&mut outgoing.executor);
            incoming.executor.inherit_runtime(&outgoing.executor);
        });
        // 2. Commands: bounded batch, applied to the patch that renders this block.
        let n = self.cmds.drain(&mut self.cmd_batch);
        for i in 0..n {
            self.apply(self.cmd_batch[i]);
        }
        // 3. Render.
        let patch = self.audio.live_mut();
        let result = patch.executor.render_block(patch.master, out);
        // 4. Publish the meters and the analysis waveforms (decision 8). A full ring
        //    refuses-and-counts inside the ring; the audio thread never waits for a reader.
        self.publish_meters();
        self.publish_analysis();
        result
    }

    /// The live executor (audio thread only — this reference must not escape the thread).
    #[must_use]
    pub fn live(&self) -> &Executor {
        &self.audio.live().executor
    }

    /// Blocks rendered across all swaps — the inherited stream clock, observable.
    #[must_use]
    pub fn blocks_rendered(&self) -> u64 {
        self.audio.live().executor.blocks_rendered()
    }

    /// Stop: hand the final patch back to the caller so it drops where teardown belongs (NOT
    /// inside a device callback). Drain [`SharedEngine::reclaim`] afterwards; whatever is
    /// still staged or retired is freed exactly once when the last handle drops.
    pub fn shutdown(self) -> LivePatch {
        self.audio.shutdown()
    }

    /// Apply one command against the live patch; count the outcome.
    fn apply(&mut self, cmd: EngineCmd) {
        let patch = self.audio.live_mut();
        let ok = match cmd {
            EngineCmd::Noop => return, // the ring's default fill: no meaning, no count
            EngineCmd::SetParams { node, params } => {
                patch.executor.set_params(NodeId(node), params).is_ok()
            },
            EngineCmd::SetMusical { tick, ppqn } => {
                patch.executor.set_musical_position(Some((tick, ppqn)));
                true
            },
            EngineCmd::ClearMusical => {
                patch.executor.set_musical_position(None);
                true
            },
            EngineCmd::ClearAutoBypass { node } => {
                // `clear_auto_bypass` answers "was there a bypass to clear"; the command's
                // success question is "does the node exist" — a clear on a running node is a
                // no-op, not a refusal, so existence is checked first and honestly.
                if patch.executor.meter(NodeId(node)).is_some() {
                    patch.executor.clear_auto_bypass(NodeId(node));
                    true
                } else {
                    false
                }
            },
        };
        if ok {
            self.counters.applied.fetch_add(1, Ordering::Relaxed);
        } else {
            self.counters.refused.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Publish the block's meters, one entry per AUDIO OUTPUT port. Allocation-free: the ring
    /// storage exists, `order()` is the cached sort, [`Executor::with_audio_out`] walks the
    /// build-time port map, and `MeterUpdate` is `Copy`. A node with no audio outputs publishes
    /// one [`MeterUpdate::FOLDED`] entry instead, so a cv-only node's status still crosses.
    fn publish_meters(&self) {
        let patch = self.audio.live();
        let block = patch.executor.blocks_rendered();
        for &node in patch.executor.order() {
            let Some(m) = patch.executor.meter(node) else { continue };
            let status = status_to_u8(m.status);
            let mut ports = 0u32;
            let block_frames = patch.executor.config().block_frames.max(1);
            patch.executor.with_audio_out(node, |port, buf| {
                let (peak, rms, peak_l, peak_r) = peak_rms_channels(buf, block_frames);
                // A full ring refuses and counts inside itself — the publication never waits.
                self.meters.push(MeterUpdate {
                    node: node.0,
                    port,
                    block,
                    peak,
                    rms,
                    peak_l,
                    peak_r,
                    status,
                });
                ports += 1;
            });
            if ports == 0 {
                self.meters.push(MeterUpdate {
                    node: node.0,
                    port: MeterUpdate::FOLDED,
                    block,
                    peak: m.peak,
                    peak_l: 0.0,
                    peak_r: 0.0,
                    rms: m.rms,
                    status,
                });
            }
        }
    }

    /// Publish every cv OUTPUT's waveform for the block just rendered — the payload a
    /// `dsp/scope` draws, and (since WO-012 increment 2) the live source of a cv WIRE's level:
    /// audio-rate ports travel as their block buffer, block-rate ports as a one-sample waveform,
    /// so a consumer's `peak |wave|` rule reads the same language for both rates.
    /// Allocation-free and bounded: the executor hands over the buffers/cells it already holds
    /// ([`Executor::with_audio_rate_cv_out`] / [`Executor::with_block_rate_cv_out`]), the update
    /// is a `Copy` struct built on the stack, and a full ring refuses-and-counts inside itself
    /// (a dropped waveform is a dropped frame of a display, which is what a real-time display is
    /// allowed to do — the audio thread never waits for the visual consumer).
    fn publish_analysis(&self) {
        let patch = self.audio.live();
        let block = patch.executor.blocks_rendered();
        // One reused stack buffer: the samples beyond `len` are never read (`wave()` bounds by
        // `len`), so re-filling only the valid prefix keeps the audio-thread work minimal.
        let mut update = AnalysisUpdate::default();
        for &node in patch.executor.order() {
            patch.executor.with_audio_rate_cv_out(node, |port, wave| {
                update.node = node.0;
                update.port = port;
                update.block = block;
                update.set_samples(wave);
                self.analysis.push(update);
            });
            patch.executor.with_block_rate_cv_out(node, |port, v| {
                update.node = node.0;
                update.port = port;
                update.block = block;
                update.set_samples(&[v]);
                self.analysis.push(update);
            });
            // Operator ruling 2026-10-01 r3: the scope accepts ANY source, so AUDIO outputs
            // publish their first channel's waveform too (strided out of the interleaved
            // bus). The ring stays bounded; a consumer that falls behind is refused and
            // counted, never waited for.
            let frames = patch.executor.config().block_frames.max(1);
            patch.executor.with_audio_out(node, |port, buf| {
                let stride = (buf.len() / frames).max(1);
                update.node = node.0;
                update.port = port;
                update.block = block;
                update.set_samples_strided(buf, stride);
                self.analysis.push(update);
            });
        }
    }
}

/// Peak and RMS over ONE port buffer — the same arithmetic as the executor's folded
/// `peak_rms_ports` (f64 accumulator, same order), so for a single-output node the per-port
/// reading is bit-identical to the folded reading it sits beside. Empty → zeros; no allocation.
/// [`peak_rms`] plus the per-CHANNEL peaks the meter bars read (WO-012 increment 4): the
/// buffer is interleaved frames × channels, and the channel count is the buffer length over
/// the block size — a mono port therefore duplicates its single channel into `peak_l` and
/// `peak_r`, which is exactly the additive pin the cross-thread test holds. One cache-warm
/// pass, same f64 accumulator, no allocation.
fn peak_rms_channels(buf: &[f32], block_frames: usize) -> (f32, f32, f32, f32) {
    let channels = buf.len().checked_div(block_frames).unwrap_or(1).max(1);
    let mut peak = 0.0f32;
    let mut sum = 0.0f64;
    let mut ch = [0.0f32; 2];
    for (i, &s) in buf.iter().enumerate() {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        sum += f64::from(s) * f64::from(s);
        let c = i % channels;
        if c < 2 && a > ch[c] {
            ch[c] = a;
        }
    }
    let rms = if buf.is_empty() { 0.0 } else { (sum / buf.len() as f64).sqrt() as f32 };
    let (l, r) = if channels >= 2 { (ch[0], ch[1]) } else { (ch[0], ch[0]) };
    (peak, rms, l, r)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::executor::{ExecConfig, Executor, NodeBuild};
    use sparq_kernel::graph::Graph;
    use sparq_module_api::manifest::{
        Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
    };
    use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
    use sparq_module_api::params::ParamSet;

    /// A DC source: out = param(0), mono.
    struct Dc;
    impl Module for Dc {
        fn id(&self) -> &str {
            "sparq/test/dc"
        }
        fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
        fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
            Ok(())
        }
        fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
            let v = ctx.param(0);
            for s in ctx.output().iter_mut() {
                *s = v;
            }
            BlockStatus::Ok
        }
        fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
            Err(ModuleError::Message("dc takes no messages"))
        }
    }

    fn dc_manifest() -> sparq_module_api::manifest::ValidatedManifest {
        let m = Manifest {
            identity: Identity {
                id: Some("sparq/test/dc".into()),
                version: Some("0.1.0".into()),
                host_api_min: Some(1),
                host_api_max: Some(1),
                display_name: Some("DC".into()),
                summary: Some("test source".into()),
                authors: vec!["sparq".into()],
                license: Some("MIT".into()),
            },
            classification: Classification {
                category: Some("utility/dc".into()),
                top: Some("util".into()),
                kind: Some("source".into()),
                tier: Some("t1".into()),
                layer: None,
                stability: Some("stable".into()),
            },
            state: StateDecl {
                schema_id: Some("sparq/test/dc/state".into()),
                schema_version: Some(1),
            },
            resources: ResourceDecl {
                latency_samples: Some(0),
                cpu_class: Some("trivial".into()),
                ..ResourceDecl::default()
            },
            voices_policy: Some("none".into()),
            ..Manifest::default()
        };
        let mut m = m;
        m.ports = vec![PortSpec {
            id: Some("out".into()),
            name: Some("OUT".into()),
            direction: Some("out".into()),
            port_type: Some("audio".into()),
            channel_set: Some("mono".into()),
            ..PortSpec::default()
        }];
        m.params = vec![ParamSpec {
            id: Some("level".into()),
            name: Some("level".into()),
            kind: Some("float".into()),
            unit: Some("ratio".into()),
            min: Some(-2.0),
            max: Some(2.0),
            default: Some(1.0),
            ..ParamSpec::default()
        }];
        m.validate().expect("the test manifest is valid")
    }

    fn dc_executor(level: f32) -> Executor {
        let mut g = Graph::new();
        let n = g.add_node(0);
        Executor::build(
            g,
            vec![(
                n,
                NodeBuild {
                    module: Box::new(Dc),
                    manifest: dc_manifest(),
                    params: ParamSet::new(0, &[level]).unwrap(),
                },
            )],
            ExecConfig::new(48_000, 64, 1),
        )
        .unwrap()
    }

    fn peak(out: &[f32]) -> f32 {
        out.iter().fold(0.0f32, |a, &s| a.max(s.abs()))
    }

    #[test]
    fn a_staged_swap_takes_effect_at_exactly_the_next_boundary() {
        let mut eng = Engine::new(dc_executor(0.25));
        let mut out = vec![0.0f32; 64];
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(peak(&out), 0.25);

        eng.stage(dc_executor(0.75));
        assert!(eng.is_staged());
        // the block AFTER staging still renders the OLD patch until the boundary is crossed…
        // (staging happened between blocks, so the very next render IS the boundary)
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(peak(&out), 0.75, "the swap applied at the boundary");
        assert_eq!(eng.swaps(), 1);
        assert!(!eng.is_staged());
    }

    #[test]
    fn the_transport_clock_survives_the_swap() {
        let mut eng = Engine::new(dc_executor(0.5));
        let mut out = vec![0.0f32; 64];
        for _ in 0..10 {
            eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        }
        eng.stage(dc_executor(0.5));
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(eng.blocks_rendered(), 11, "the count continues across the swap");
        for _ in 0..5 {
            eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        }
        assert_eq!(eng.blocks_rendered(), 16);
    }

    #[test]
    fn a_double_stage_supersedes_the_first_successor_and_counts_it() {
        let mut eng = Engine::new(dc_executor(0.25));
        eng.stage(dc_executor(0.5));
        eng.stage(dc_executor(0.75));
        assert_eq!(eng.superseded_stages(), 1);
        let mut out = vec![0.0f32; 64];
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(peak(&out), 0.75, "the LAST staged successor is the one that goes live");
        assert_eq!(eng.swaps(), 1);
    }

    #[test]
    fn a_swap_that_drops_the_master_is_reported_not_silenced() {
        // Live: two independent DCs, master = node 1. Successor: only node 0. After the swap,
        // asking for node 1 must refuse in words (NoSuchNode), never render silence.
        let mut g_live = Graph::new();
        let _a = g_live.add_node(0);
        let b = g_live.add_node(0);
        let builds = |g: &Graph| {
            g.nodes()
                .iter()
                .map(|n| {
                    (
                        n.id,
                        NodeBuild {
                            module: Box::new(Dc),
                            manifest: dc_manifest(),
                            params: ParamSet::new(0, &[0.5]).unwrap(),
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let live = Executor::build(g_live.clone(), builds(&g_live), ExecConfig::new(48_000, 64, 1))
            .unwrap();
        let mut g_next = Graph::new();
        let _n = g_next.add_node(0);
        let next = Executor::build(g_next.clone(), builds(&g_next), ExecConfig::new(48_000, 64, 1))
            .unwrap();
        let mut eng = Engine::new(live);
        let mut out = vec![0.0f32; 64];
        eng.render_block(b, &mut out).unwrap(); // master node 1 renders fine pre-swap
        eng.stage(next);
        let err = eng.render_block(b, &mut out).unwrap_err();
        assert!(matches!(err, ExecError::Graph(_)), "{err}");
        assert_eq!(eng.swaps(), 1, "the swap happened; the refusal is the new graph's truth");
    }
}
