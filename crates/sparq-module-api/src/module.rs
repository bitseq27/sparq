//! The module surface: six things a module does, on two threads.
//!
//! | Stage | Thread | May allocate? | Deterministic? |
//! |---|---|---|---|
//! | [`Module::configure`] | control | yes | must be |
//! | [`Module::prepare`] | control | **all of it, here** | must be |
//! | [`Module::activate`] / [`Module::deactivate`] | control | no / no | must be |
//! | [`Module::process`] | **audio** | **no** | **must be** |
//! | [`Module::message`] | control | yes | must be |
//!
//! [`AudioCtx`] is how "no" is enforced: it carries a block context, an immutable parameter
//! snapshot and **borrowed, fixed-capacity port views** — and it exposes no allocator, no clock,
//! no filesystem and no lock. The rule is the absence of an API rather than a comment
//! (module-api §9). Rust cannot forbid `Vec::new()` by types alone, so the claim is also
//! *measured* — `tests/contract.rs` installs the kernel's counting allocator as its global
//! allocator and fails if a reference module allocates once inside `process`.
//!
//! # Contract v1: multi-port (WO-008 increment 4)
//!
//! v0 carried exactly one interleaved audio input and one audio output as public fields. v1
//! carries the module's **declared ports**, in manifest order, per type:
//!
//! * [`AudioCtx::audio_in`] / [`AudioCtx::audio_out`] — interleaved audio buffers, ≤ [`MAX_PORTS_PER_CLASS`]
//!   per direction. An unconnected optional input is an **empty slice** (never absent, never
//!   silence-by-accident); `None` means the port does not exist.
//! * [`AudioCtx::cv_in`] / [`AudioCtx::cv_out`] — [`CvIn`] (a block-rate value, an audio-rate
//!   slice, or explicitly [`CvIn::Unconnected`]) and [`CvOut`]. The rate a port presents is the
//!   rate the module DECLARED: the host performs any rate change (the receiver's `cv_reduce` /
//!   `cv_interp` — compat-matrix G3/G4) before the module sees a sample.
//! * [`AudioCtx::events_in`] / [`AudioCtx::event_out`] — a `&[Event]` **pre-sorted by sample
//!   offset** (module-api §9's host guarantee, ties stable per the matrix's G5 tie-break) and a
//!   bounded [`EventSink`](crate::event::EventSink) for emission.
//!
//! `data`, `gpu` and `atom` payloads do not travel through `AudioCtx`: `gpu` and `atom` are
//! never on the audio thread (ADR-005), and `data` waits for the schema/staleness machinery —
//! the executor refuses those edges **in words** rather than ignoring a drawn wire.
//!
//! Port indices count **only ports of that type**, in manifest order: `audio_in(0)` is the
//! module's first `audio` `in` port even if the manifest lists a `cv` port before it. The
//! v0-shaped conveniences remain as methods over the first audio port: [`AudioCtx::input`],
//! [`AudioCtx::output`], [`AudioCtx::has_input`].
//!
//! **Borrow discipline for module authors:** the input accessors return borrows with the
//! context's own lifetime (`&'a`), so the two-step
//! `let inp = ctx.input(); for (o, i) in ctx.output().iter_mut().zip(inp) { … }`
//! compiles — an input taken first never conflicts with an output taken second. Output views
//! come in two flavours: the reborrowing accessors ([`AudioCtx::audio_out`], [`AudioCtx::cv_out`],
//! [`AudioCtx::event_out`]) for the common one-port-at-a-time case, and the `take_*` variants,
//! which MOVE the view out with the context's lifetime so a multi-output module can hold several
//! at once — take each port once per block.

use std::fmt;

use sparq_kernel::block::BlockContext;

use crate::event::{Event, EventBuf, EventSink};
use crate::params::ParamSet;

/// The most ports of one type in one direction a module may declare (audio inputs, audio
/// outputs, cv inputs, cv outputs, event inputs, event outputs — each capped separately).
///
/// A hard v1 limit in the same family as [`crate::params::MAX_PARAMS`]: the context is a
/// fixed-capacity struct of borrowed slices so that assembling it on the audio thread cannot
/// allocate. A manifest declaring more is rejected at validation (as `E-CROSS-FIELD:ports`)
/// rather than silently truncated — a port that cannot be presented must not pretend to exist.
pub const MAX_PORTS_PER_CLASS: usize = 8;

/// What `prepare` receives: everything that can change a module's memory or timing shape.
///
/// Nothing in `process` may assume a value that was not given here. Any change to any of these
/// fields is a **resource** change and runs `deactivate → prepare → activate` — including a
/// channel-set resolution, which cascades to dependants in topological order without touching the
/// graph's version or mutation counter (module-api §2, ADR-009 executor decision 7).
///
/// Contract v1 adds the per-port channel counts: `input_channels`/`output_channels` remain as the
/// FIRST audio port's view (the v0 shape), and the arrays carry every audio port's resolved
/// channel count in manifest order, 0 for unconnected. cv/event port *shapes* are not repeated
/// here: a module declares them in its own manifest and the host presents exactly what was
/// declared — the only host-side fact a module cannot know is a `variable` channel-set
/// resolution, and only audio ports have channel sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resources {
    /// Device sample rate in Hz.
    pub sample_rate: u32,
    /// Frames per block.
    pub block_frames: usize,
    /// Channels on the FIRST audio input, 0 if unconnected or absent (the v0 view).
    pub input_channels: usize,
    /// Channels on the FIRST audio output (the v0 view).
    pub output_channels: usize,
    /// Resolved channels per audio INPUT port, manifest order; 0 = unconnected. Entries past
    /// `audio_in_count` are meaningless.
    pub audio_in_channels: [usize; MAX_PORTS_PER_CLASS],
    /// How many audio input ports the host presents.
    pub audio_in_count: usize,
    /// Resolved channels per audio OUTPUT port, manifest order. Entries past `audio_out_count`
    /// are meaningless.
    pub audio_out_channels: [usize; MAX_PORTS_PER_CLASS],
    /// How many audio output ports the host presents.
    pub audio_out_count: usize,
    /// Host-provided oversampling (module-api §16 Q4: the host provides, the module declares need).
    pub oversampling: Oversampling,
    /// Voice count, for modules whose `voices.policy` is not `none`.
    pub voices: u32,
    /// The arena budget this module may reserve in `prepare`.
    pub arena_bytes: usize,
}

impl Resources {
    /// Resources for an offline render of `ctx`, with no oversampling and one voice: the v0
    /// single-in/single-out shape (one audio port each way, `ctx.channels` wide). Tests and
    /// simple hosts use this; the executor computes the real per-port arrays from the graph.
    #[must_use]
    pub fn from_block(ctx: &BlockContext) -> Self {
        let mut audio_in_channels = [0usize; MAX_PORTS_PER_CLASS];
        let mut audio_out_channels = [0usize; MAX_PORTS_PER_CLASS];
        audio_in_channels[0] = ctx.channels;
        audio_out_channels[0] = ctx.channels;
        Self {
            sample_rate: ctx.sample_rate,
            block_frames: ctx.frames,
            input_channels: ctx.channels,
            output_channels: ctx.channels,
            audio_in_channels,
            audio_in_count: 1,
            audio_out_channels,
            audio_out_count: 1,
            oversampling: Oversampling::None,
            voices: 1,
            arena_bytes: 0,
        }
    }
}

/// Host-provided oversampling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Oversampling {
    /// No oversampling.
    #[default]
    None,
    /// 2×.
    X2,
    /// 4×.
    X4,
    /// 8×.
    X8,
}

impl Oversampling {
    /// Parses a manifest spelling (`none` | `x2` | `x4` | `x8`).
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "none" => Some(Self::None),
            "x2" => Some(Self::X2),
            "x4" => Some(Self::X4),
            "x8" => Some(Self::X8),
            _ => None,
        }
    }

    /// The factor as an integer.
    #[must_use]
    pub fn factor(self) -> usize {
        match self {
            Self::None => 1,
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
        }
    }
}

/// What `process` returns. Errors are reported through this value and the executor's watchdog,
/// never by unwinding across the boundary (module-api §9 rule 7).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlockStatus {
    /// The block was produced as declared.
    #[default]
    Ok,
    /// The module produced silence deliberately (muted, bypassed internally, or starved of input it
    /// requires). Distinct from `Ok` so a silent output is a statement, not a surprise.
    Silenced,
    /// The module exceeded its declared block-time budget. N consecutive overruns make the executor
    /// auto-bypass it, draw a red hairline and write a journal entry (ADR-009 decision 6).
    Overrun,
    /// The module could not produce this block. The executor substitutes silence and flags it; the
    /// audio thread never unwinds.
    Failed,
}

/// A control-thread failure. Carries a `&'static str` rather than a `String` so that constructing
/// one cannot allocate — a failure path that allocates is a failure path that can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleError {
    /// The module does not support what was asked of it.
    Unsupported(&'static str),
    /// The state blob could not be applied.
    State(&'static str),
    /// [`Resources`] are outside what the module can run with.
    Resources(&'static str),
    /// An `atom` message was not understood.
    Message(&'static str),
}

impl fmt::Display for ModuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, why) = match self {
            Self::Unsupported(w) => ("unsupported", w),
            Self::State(w) => ("state", w),
            Self::Resources(w) => ("resources", w),
            Self::Message(w) => ("message", w),
        };
        write!(f, "module {kind} error: {why}")
    }
}

impl std::error::Error for ModuleError {}

/// A `cv` INPUT as the module sees it — always at the rate the module declared, because the host
/// performs any rate change (the receiver's own `cv_reduce`/`cv_interp`, compat-matrix G3/G4:
/// the receiving module owns any rate change on its own input, and the host does the work).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CvIn<'a> {
    /// The port is optional and nothing is connected — an explicit signal, never 0.0-by-accident.
    Unconnected,
    /// A block-rate port: one value for this block.
    Block(f32),
    /// An audio-rate port: one value per frame, exactly `block.frames` long. A block-rate source
    /// feeding this port arrives pre-expanded by the receiver's declared `cv_interp`.
    Audio(&'a [f32]),
}

impl<'a> CvIn<'a> {
    /// Whether something is connected.
    #[must_use]
    pub fn is_connected(self) -> bool {
        !matches!(self, Self::Unconnected)
    }

    /// The block-rate value; `None` for an audio-rate or unconnected port.
    #[must_use]
    pub fn block(self) -> Option<f32> {
        match self {
            Self::Block(v) => Some(v),
            Self::Unconnected | Self::Audio(_) => None,
        }
    }

    /// The audio-rate samples; `None` for a block-rate or unconnected port.
    #[must_use]
    pub fn audio(self) -> Option<&'a [f32]> {
        match self {
            Self::Audio(s) => Some(s),
            Self::Unconnected | Self::Block(_) => None,
        }
    }

    /// The value at one frame: the held block value for block-rate ports, `slice[frame]` for
    /// audio-rate ones (0.0 past the end — a short buffer is the host's bug, and reading it must
    /// still not be a module's crash), 0.0 when unconnected.
    #[must_use]
    pub fn at(self, frame: usize) -> f32 {
        match self {
            Self::Block(v) => v,
            Self::Audio(s) => s.get(frame).copied().unwrap_or(0.0),
            Self::Unconnected => 0.0,
        }
    }
}

/// A `cv` OUTPUT as the module drives it, at the rate the module declared.
#[derive(Debug)]
pub enum CvOut<'a> {
    /// A block-rate port: write the one value for this block.
    Block(&'a mut f32),
    /// An audio-rate port: a buffer of exactly `block.frames` samples to fill.
    Audio(&'a mut [f32]),
}

impl CvOut<'_> {
    /// Writes one value: the cell for a block-rate port, a constant fill for an audio-rate one
    /// (a module producing a steady cv at audio rate should not hand-roll the loop).
    pub fn set(&mut self, v: f32) {
        match self {
            Self::Block(cell) => **cell = v,
            Self::Audio(buf) => {
                for s in buf.iter_mut() {
                    *s = v;
                }
            },
        }
    }

    /// The block-rate cell, or `None` for an audio-rate port.
    #[must_use]
    pub fn as_block(&self) -> Option<&f32> {
        match self {
            Self::Block(cell) => Some(*cell),
            Self::Audio(_) => None,
        }
    }

    /// The audio-rate buffer, or `None` for a block-rate port.
    #[must_use]
    pub fn as_audio(&mut self) -> Option<&mut [f32]> {
        match self {
            Self::Audio(buf) => Some(buf),
            Self::Block(_) => None,
        }
    }
}

/// Everything `process` can reach. Deliberately small: no allocator, no clock, no filesystem, no
/// lock, and no way to obtain one.
///
/// Contract v1 (WO-008 increment 4) presents the module's declared ports — audio, `cv` and
/// `event` — as fixed-capacity borrowed views, ≤ [`MAX_PORTS_PER_CLASS`] per type per direction. The
/// accessors return `None` past the ports the host presented; the inputs distinguish *absent*
/// (`None`) from *unconnected* (an empty slice / [`CvIn::Unconnected`]) because an optional input
/// receives an explicit unconnected signal, never silence-by-accident.
///
/// Hosts (the executor, tests) assemble one with [`AudioCtx::new`] / [`AudioCtx::single`] and the
/// `with_*` builders; modules only read and write through the accessors.
#[derive(Debug)]
pub struct AudioCtx<'a> {
    /// Which block this is, its sample offset, rate, channel count and musical position.
    pub block: &'a BlockContext,
    /// The parameter snapshot, immutable for the whole block.
    pub params: &'a ParamSet,
    ain: [Option<&'a [f32]>; MAX_PORTS_PER_CLASS],
    n_ain: usize,
    aout: [Option<&'a mut [f32]>; MAX_PORTS_PER_CLASS],
    n_aout: usize,
    cvin: [Option<CvIn<'a>>; MAX_PORTS_PER_CLASS],
    n_cvin: usize,
    cvout: [Option<CvOut<'a>>; MAX_PORTS_PER_CLASS],
    n_cvout: usize,
    evin: [Option<&'a [Event]>; MAX_PORTS_PER_CLASS],
    n_evin: usize,
    evout: [Option<&'a mut EventBuf>; MAX_PORTS_PER_CLASS],
    n_evout: usize,
}

impl<'a> AudioCtx<'a> {
    /// A context with no ports; add them with the `with_*` builders.
    #[must_use]
    pub fn new(block: &'a BlockContext, params: &'a ParamSet) -> Self {
        Self {
            block,
            params,
            ain: [None; MAX_PORTS_PER_CLASS],
            n_ain: 0,
            aout: std::array::from_fn(|_| None),
            n_aout: 0,
            cvin: [None; MAX_PORTS_PER_CLASS],
            n_cvin: 0,
            cvout: std::array::from_fn(|_| None),
            n_cvout: 0,
            evin: [None; MAX_PORTS_PER_CLASS],
            n_evin: 0,
            evout: std::array::from_fn(|_| None),
            n_evout: 0,
        }
    }

    /// The v0 shape, still the common case: one interleaved audio input (empty = unconnected)
    /// and one interleaved audio output.
    #[must_use]
    pub fn single(
        block: &'a BlockContext,
        params: &'a ParamSet,
        input: &'a [f32],
        output: &'a mut [f32],
    ) -> Self {
        Self::new(block, params).with_audio_in(input).with_audio_out(output)
    }

    /// Appends an audio INPUT port (manifest order). Past [`MAX_PORTS_PER_CLASS`] the port is dropped and
    /// a debug assertion names the host bug — validation caps manifests at the same constant, so
    /// the validated path cannot get here.
    #[must_use]
    pub fn with_audio_in(mut self, buf: &'a [f32]) -> Self {
        debug_assert!(self.n_ain < MAX_PORTS_PER_CLASS, "audio inputs exceed MAX_PORTS_PER_CLASS");
        if self.n_ain < MAX_PORTS_PER_CLASS {
            self.ain[self.n_ain] = Some(buf);
            self.n_ain += 1;
        }
        self
    }

    /// Appends an audio OUTPUT port (manifest order). The executor's contract to the module: the
    /// buffer is exactly `block.frames × channels` for this port.
    #[must_use]
    pub fn with_audio_out(mut self, buf: &'a mut [f32]) -> Self {
        debug_assert!(
            self.n_aout < MAX_PORTS_PER_CLASS,
            "audio outputs exceed MAX_PORTS_PER_CLASS"
        );
        if self.n_aout < MAX_PORTS_PER_CLASS {
            self.aout[self.n_aout] = Some(buf);
            self.n_aout += 1;
        }
        self
    }

    /// Appends a `cv` INPUT port (manifest order).
    #[must_use]
    pub fn with_cv_in(mut self, cv: CvIn<'a>) -> Self {
        debug_assert!(self.n_cvin < MAX_PORTS_PER_CLASS, "cv inputs exceed MAX_PORTS_PER_CLASS");
        if self.n_cvin < MAX_PORTS_PER_CLASS {
            self.cvin[self.n_cvin] = Some(cv);
            self.n_cvin += 1;
        }
        self
    }

    /// Appends a `cv` OUTPUT port (manifest order).
    #[must_use]
    pub fn with_cv_out(mut self, cv: CvOut<'a>) -> Self {
        debug_assert!(self.n_cvout < MAX_PORTS_PER_CLASS, "cv outputs exceed MAX_PORTS_PER_CLASS");
        if self.n_cvout < MAX_PORTS_PER_CLASS {
            self.cvout[self.n_cvout] = Some(cv);
            self.n_cvout += 1;
        }
        self
    }

    /// Appends an `event` INPUT port (manifest order). The host guarantees the slice is sorted by
    /// [`Event::sample`] with the matrix's stable tie-break.
    #[must_use]
    pub fn with_events_in(mut self, events: &'a [Event]) -> Self {
        debug_assert!(self.n_evin < MAX_PORTS_PER_CLASS, "event inputs exceed MAX_PORTS_PER_CLASS");
        if self.n_evin < MAX_PORTS_PER_CLASS {
            self.evin[self.n_evin] = Some(events);
            self.n_evin += 1;
        }
        self
    }

    /// Appends an `event` OUTPUT port (manifest order), backed by executor-owned bounded storage.
    #[must_use]
    pub fn with_event_out(mut self, buf: &'a mut EventBuf) -> Self {
        debug_assert!(
            self.n_evout < MAX_PORTS_PER_CLASS,
            "event outputs exceed MAX_PORTS_PER_CLASS"
        );
        if self.n_evout < MAX_PORTS_PER_CLASS {
            self.evout[self.n_evout] = Some(buf);
            self.n_evout += 1;
        }
        self
    }

    /// Frames in this block.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.block.frames
    }

    /// Audio input `i` (per-type manifest order): `None` past the presented ports, `Some(&[])`
    /// when the port exists but is unconnected.
    #[must_use]
    pub fn audio_in(&self, i: usize) -> Option<&'a [f32]> {
        if i < self.n_ain {
            self.ain[i]
        } else {
            None
        }
    }

    /// Audio output `i` (per-type manifest order): `None` past the presented ports; otherwise a
    /// buffer of exactly `frames × channels` for that port. The view is REBORROWED from the
    /// context, so only one such view can be held at a time — a module driving several outputs
    /// at once uses [`Self::take_audio_out`] instead.
    #[must_use]
    pub fn audio_out(&mut self, i: usize) -> Option<&mut [f32]> {
        if i < self.n_aout {
            self.aout[i].as_deref_mut()
        } else {
            None
        }
    }

    /// Moves audio output `i` OUT of the context, with the context's own lifetime: several
    /// taken views coexist (they point at disjoint executor buffers), which is how a multi-output
    /// module (`util/mixer`'s matrix, a scope's split paths) drives more than one port at a time.
    /// Taken means taken: the slot reads `None` afterwards, so take each port once per block —
    /// the pattern is `let (a, b) = (ctx.take_audio_out(0), ctx.take_audio_out(1));`.
    #[must_use]
    pub fn take_audio_out(&mut self, i: usize) -> Option<&'a mut [f32]> {
        if i < self.n_aout {
            self.aout[i].take()
        } else {
            None
        }
    }

    /// `cv` input `i` (per-type manifest order), at the declared rate.
    #[must_use]
    pub fn cv_in(&self, i: usize) -> Option<CvIn<'a>> {
        if i < self.n_cvin {
            self.cvin[i]
        } else {
            None
        }
    }

    /// `cv` output `i` (per-type manifest order), at the declared rate. Reborrowed per call, so a
    /// module may take it, write, drop it and take it again.
    #[must_use]
    pub fn cv_out(&mut self, i: usize) -> Option<CvOut<'_>> {
        let slot = if i < self.n_cvout { self.cvout.get_mut(i) } else { None };
        match slot.and_then(|o| o.as_mut()) {
            Some(CvOut::Block(cell)) => Some(CvOut::Block(cell)),
            Some(CvOut::Audio(buf)) => Some(CvOut::Audio(buf)),
            None => None,
        }
    }

    /// Moves `cv` output `i` out of the context (the [`Self::take_audio_out`] pattern for cv):
    /// several taken views coexist; the slot reads `None` afterwards.
    #[must_use]
    pub fn take_cv_out(&mut self, i: usize) -> Option<CvOut<'a>> {
        if i < self.n_cvout {
            self.cvout[i].take()
        } else {
            None
        }
    }

    /// `event` input `i` (per-type manifest order): pre-sorted by sample offset, empty when
    /// nothing arrived this block.
    #[must_use]
    pub fn events_in(&self, i: usize) -> Option<&'a [Event]> {
        if i < self.n_evin {
            self.evin[i]
        } else {
            None
        }
    }

    /// `event` output `i` (per-type manifest order): a bounded sink; `push` past capacity returns
    /// `false` and is counted host-side, never grown.
    #[must_use]
    pub fn event_out(&mut self, i: usize) -> Option<EventSink<'_>> {
        let slot = if i < self.n_evout { self.evout.get_mut(i) } else { None };
        slot.and_then(|o| o.as_mut()).map(|b| EventSink::new(b))
    }

    /// Moves the sink for `event` output `i` out of the context (the [`Self::take_audio_out`]
    /// pattern for events): several taken sinks coexist; the slot reads `None` afterwards.
    #[must_use]
    pub fn take_event_out(&mut self, i: usize) -> Option<EventSink<'a>> {
        if i < self.n_evout {
            self.evout[i].take().map(EventSink::new)
        } else {
            None
        }
    }

    /// The FIRST audio input (the v0 view): empty when absent or unconnected.
    #[must_use]
    pub fn input(&self) -> &'a [f32] {
        self.audio_in(0).unwrap_or(&[])
    }

    /// The FIRST audio output (the v0 view): empty when the module declares no audio output.
    #[must_use]
    pub fn output(&mut self) -> &mut [f32] {
        match self.audio_out(0) {
            Some(buf) => buf,
            None => &mut [],
        }
    }

    /// Whether the first audio input is connected (non-empty).
    #[must_use]
    pub fn has_input(&self) -> bool {
        !self.input().is_empty()
    }

    /// Parameter `index` for this block, or 0.0 past the declared count.
    #[must_use]
    pub fn param(&self, index: usize) -> f32 {
        self.params.at(index)
    }
}

/// The contract. A module is this trait plus a manifest that validates.
///
/// The trait is dyn-compatible on purpose: ADR-009 executor decision 7 measured that a trait object
/// is affordable at block granularity (1.65–1.95 µs for 100 modules × 64 samples, 10× under budget)
/// and unaffordable per sample (27–36 µs against the same 20 µs budget). So `process` may be called
/// through `Box<dyn Module>`, and nothing inside it may be.
pub trait Module {
    /// The module's stable id, e.g. `sparq/util/gain`. Must match the manifest's `identity.id`.
    fn id(&self) -> &str;

    /// Applies a state blob. Control thread; may allocate; must be deterministic.
    ///
    /// # Errors
    /// [`ModuleError::State`] if the blob does not fit this module's declared schema version.
    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError>;

    /// Reserves everything the module needs for the given [`Resources`]. **All** allocation happens
    /// here; any change to these values means `deactivate → prepare → activate`.
    ///
    /// # Errors
    /// [`ModuleError::Resources`] if the module cannot run with these values.
    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError>;

    /// Starts processing. Called after `prepare`, before the first `process`.
    ///
    /// # Errors
    /// [`ModuleError::Resources`] if the module is not in a state to start.
    fn activate(&mut self) -> Result<(), ModuleError> {
        Ok(())
    }

    /// Produces one block. **Audio thread: no allocation, no locking, no blocking, no syscalls, no
    /// wall clock, no unseeded randomness, no panics.** The parameter snapshot is immutable for the
    /// whole block, and any graph mutation takes effect only at a boundary.
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus;

    /// Stops processing. Control thread; must not allocate.
    fn deactivate(&mut self) {}

    /// Handles an `atom` message from the host. Control thread; may allocate.
    ///
    /// # Errors
    /// [`ModuleError::Message`] if the payload is not understood.
    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError>;
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use crate::event::{Event, EventKind};

    /// The smallest module that satisfies the contract, used to test the contract itself.
    struct Null;

    impl Module for Null {
        fn id(&self) -> &str {
            "sparq/test/null"
        }
        fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
        fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
            Ok(())
        }
        fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            BlockStatus::Ok
        }
        fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
            Err(ModuleError::Message("the null module takes no messages"))
        }
    }

    #[test]
    fn the_trait_is_dyn_compatible_so_process_can_be_a_trait_object() {
        // ADR-009 decision 7: block-level dispatch may be a trait object. If this stops compiling,
        // the executor's dispatch strategy has to be revisited, not patched around.
        let mut m: Box<dyn Module> = Box::new(Null);
        assert_eq!(m.id(), "sparq/test/null");
        let block = BlockContext::offline(48_000, 64, 2);
        let params = ParamSet::zeroed();
        let mut out = vec![1.0f32; 128];
        {
            // Scoped so the mutable borrow of `out` ends before the buffer is inspected.
            let mut ctx = AudioCtx::single(&block, &params, &[], &mut out);
            assert_eq!(m.process(&mut ctx), BlockStatus::Ok);
            assert_eq!(ctx.frames(), 64);
            assert!(!ctx.has_input());
            assert_eq!(ctx.param(0), 0.0);
        }
        assert!(out.iter().all(|v| *v == 0.0), "the null module wrote silence");
    }

    #[test]
    fn v1_ports_are_presented_in_push_order_and_absent_past_their_count() {
        let block = BlockContext::offline(48_000, 4, 1);
        let params = ParamSet::zeroed();
        let (in0, in1) = ([1.0f32; 4], [2.0f32; 4]);
        let (mut out0, mut out1) = ([0.0f32; 4], [0.0f32; 4]);
        let mut ctx = AudioCtx::new(&block, &params)
            .with_audio_in(&in0)
            .with_audio_in(&in1)
            .with_audio_out(&mut out0)
            .with_audio_out(&mut out1);
        assert_eq!(ctx.audio_in(0), Some(&[1.0; 4][..]));
        assert_eq!(ctx.audio_in(1), Some(&[2.0; 4][..]));
        assert_eq!(ctx.audio_in(2), None, "absent, not empty — the two are different statements");
        assert_eq!(ctx.input(), &[1.0; 4][..], "the v0 view is the first port");
        ctx.output()[0] = 9.0;
        assert_eq!(ctx.audio_out(1).unwrap()[0], 0.0);
        assert_eq!(ctx.output().len(), 4);
    }

    #[test]
    fn an_unconnected_optional_input_is_empty_and_a_module_without_outputs_gets_an_empty_output() {
        let block = BlockContext::offline(48_000, 4, 1);
        let params = ParamSet::zeroed();
        let mut ctx = AudioCtx::new(&block, &params).with_audio_in(&[]);
        assert_eq!(ctx.audio_in(0), Some(&[][..]), "present but unconnected");
        assert!(!ctx.has_input());
        assert!(ctx.output().is_empty(), "no audio output port: an empty view, never a panic");
        assert!(ctx.audio_out(0).is_none());
    }

    #[test]
    fn cv_views_present_the_declared_rate_and_distinguish_unconnected() {
        let block = BlockContext::offline(48_000, 4, 1);
        let params = ParamSet::zeroed();
        let samples = [0.1f32, 0.2, 0.3, 0.4];
        let mut cell = 0.0f32;
        {
            let mut ctx = AudioCtx::new(&block, &params)
                .with_cv_in(CvIn::Block(0.75))
                .with_cv_in(CvIn::Unconnected)
                .with_cv_in(CvIn::Audio(&samples))
                .with_cv_out(CvOut::Block(&mut cell));
            assert_eq!(ctx.cv_in(0).unwrap().block(), Some(0.75));
            assert_eq!(ctx.cv_in(1), Some(CvIn::Unconnected));
            assert!(!ctx.cv_in(1).unwrap().is_connected());
            assert_eq!(ctx.cv_in(2).unwrap().at(2), 0.3);
            assert_eq!(ctx.cv_in(3), None);
            assert_eq!(CvIn::Unconnected.at(0), 0.0, "unconnected reads as an explicit zero");
            ctx.cv_out(0).unwrap().set(0.5);
            assert!(ctx.cv_out(1).is_none());
        }
        assert_eq!(cell, 0.5, "the block-rate cell is executor-owned storage");
    }

    #[test]
    fn a_block_rate_cv_output_drives_a_real_cell_and_an_audio_rate_one_fills_a_buffer() {
        let block = BlockContext::offline(48_000, 4, 1);
        let params = ParamSet::zeroed();
        let mut buf = [0.0f32; 4];
        let mut cell = 0.0f32;
        {
            let mut ctx = AudioCtx::new(&block, &params)
                .with_cv_out(CvOut::Audio(&mut buf))
                .with_cv_out(CvOut::Block(&mut cell));
            ctx.cv_out(0).unwrap().set(0.25);
            assert!(ctx.cv_out(0).is_some(), "a sink may be taken repeatedly within a block");
        }
        assert_eq!(buf, [0.25; 4], "set() on an audio-rate port is a constant fill");
        assert!(cell == 0.0);
    }

    #[test]
    fn event_inputs_arrive_presorted_and_sinks_emit_into_executor_storage() {
        let block = BlockContext::offline(48_000, 64, 1);
        let params = ParamSet::zeroed();
        let events = [Event::trigger(3, 0.5), Event::trigger(9, 1.0)];
        let mut out_buf = EventBuf::new();
        {
            let mut ctx =
                AudioCtx::new(&block, &params).with_events_in(&events).with_event_out(&mut out_buf);
            let seen = ctx.events_in(0).unwrap();
            assert_eq!(seen.len(), 2);
            assert!(seen[0].sample <= seen[1].sample, "the host guarantee, restated");
            assert!(ctx.events_in(1).is_none());
            let mut sink = ctx.event_out(0).unwrap();
            assert!(sink.is_empty());
            assert!(sink.push(Event::gate(12, 1.0)));
            assert_eq!(sink.len(), 1);
            assert_eq!(sink.capacity(), crate::event::EVENTS_PER_BLOCK);
        }
        assert_eq!(out_buf.len(), 1);
        assert_eq!(out_buf.as_slice()[0].kind, EventKind::Gate);
        assert_eq!(out_buf.dropped(), 0);
    }

    #[test]
    fn inputs_taken_first_never_conflict_with_outputs_taken_second() {
        // The borrow discipline the v1 header promises module authors, compiled as a test.
        let block = BlockContext::offline(48_000, 4, 1);
        let params = ParamSet::zeroed();
        let input = [0.5f32; 4];
        let mut out = [0.0f32; 4];
        let mut ctx = AudioCtx::single(&block, &params, &input, &mut out);
        let inp = ctx.input();
        for (o, i) in ctx.output().iter_mut().zip(inp.iter()) {
            *o = *i * 2.0;
        }
        assert_eq!(ctx.input(), &[0.5; 4][..]);
    }

    #[test]
    fn resources_follow_the_block_context() {
        let block = BlockContext::offline(96_000, 64, 4);
        let r = Resources::from_block(&block);
        assert_eq!(r.sample_rate, 96_000);
        assert_eq!(r.block_frames, 64);
        assert_eq!(r.input_channels, 4);
        assert_eq!(r.output_channels, 4);
        assert_eq!(r.audio_in_count, 1);
        assert_eq!(r.audio_out_count, 1);
        assert_eq!(r.audio_in_channels[0], 4);
        assert_eq!(r.audio_out_channels[0], 4);
        assert_eq!(r.oversampling, Oversampling::None);
    }

    #[test]
    fn oversampling_parses_the_closed_set() {
        for (s, f) in [("none", 1), ("x2", 2), ("x4", 4), ("x8", 8)] {
            assert_eq!(Oversampling::parse(s).map(Oversampling::factor), Some(f));
        }
        assert_eq!(Oversampling::parse("x16"), None);
    }

    #[test]
    fn a_module_error_never_allocates_to_be_created() {
        // &'static str payloads, so the failure path itself cannot fail.
        let e = ModuleError::Resources("block_frames must be a power of two");
        assert_eq!(e.to_string(), "module resources error: block_frames must be a power of two");
        // A `&'static str` is the whole payload — no `String`, no `Vec`, no `Box` — so the error
        // stays `Copy` and cannot outgrow a string slice plus a discriminant.
        assert!(
            std::mem::size_of::<ModuleError>()
                <= std::mem::size_of::<&str>() + std::mem::size_of::<usize>(),
            "ModuleError grew a heap payload: {}",
            std::mem::size_of::<ModuleError>()
        );
    }

    #[test]
    fn lifecycle_defaults_are_no_ops_that_succeed() {
        let mut m = Null;
        assert!(m.activate().is_ok(), "activate has a default that succeeds");
        m.deactivate();
        assert!(m.message(&[]).is_err(), "and this one refuses honestly");
    }

    #[test]
    fn taken_output_views_coexist_and_the_slot_reads_none_afterwards() {
        // The multi-output pattern (util/mixer's shape): two mutable views, held at once,
        // pointing at disjoint executor buffers.
        let block = BlockContext::offline(48_000, 4, 1);
        let params = ParamSet::zeroed();
        let (mut o0, mut o1) = ([0.0f32; 4], [0.0f32; 4]);
        let mut ctx =
            AudioCtx::new(&block, &params).with_audio_out(&mut o0).with_audio_out(&mut o1);
        {
            let (a, b) = (ctx.take_audio_out(0), ctx.take_audio_out(1));
            let (a, b) = (a.unwrap(), b.unwrap());
            for (x, y) in a.iter_mut().zip(b.iter_mut()) {
                *x = 1.0;
                *y = 2.0;
            }
        }
        assert_eq!(ctx.audio_out(0), None, "taken means taken — the slot is empty for this block");
        assert_eq!(o0, [1.0; 4]);
        assert_eq!(o1, [2.0; 4], "the views wrote through to the executor's buffers");
    }

    #[test]
    fn the_context_is_a_fixed_capacity_struct_not_a_growing_one() {
        // The audio thread assembles one of these per node per block. If it ever grows a Vec the
        // zero-allocation claim grows a asterisk, so the shape is pinned: six fixed arrays of
        // borrowed views plus counts. Generous upper bound — two fat pointers and an enum per
        // slot, MAX_PORTS_PER_CLASS slots, six port classes, plus the two shared references.
        let size = std::mem::size_of::<AudioCtx<'_>>();
        assert!(
            size <= 24 * std::mem::size_of::<usize>() * MAX_PORTS_PER_CLASS,
            "AudioCtx grew beyond its fixed-capacity budget: {size} bytes"
        );
    }
}
