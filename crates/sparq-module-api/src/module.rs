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
//! snapshot and two borrowed buffers, and it exposes no allocator, no clock, no filesystem and no
//! lock. The rule is the absence of an API rather than a comment (module-api §9). Rust cannot
//! forbid `Vec::new()` by types alone, so the claim is also *measured* — `tests/contract.rs`
//! installs the kernel's counting allocator as its global allocator and fails if a reference module
//! allocates once inside `process`.

use std::fmt;

use sparq_kernel::block::BlockContext;

use crate::params::ParamSet;

/// What `prepare` receives: everything that can change a module's memory or timing shape.
///
/// Nothing in `process` may assume a value that was not given here. Any change to any of these
/// fields is a **resource** change and runs `deactivate → prepare → activate` — including a
/// channel-set resolution, which cascades to dependants in topological order without touching the
/// graph's version or mutation counter (module-api §2, ADR-009 executor decision 7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Resources {
    /// Device sample rate in Hz.
    pub sample_rate: u32,
    /// Frames per block.
    pub block_frames: usize,
    /// Channels on the audio input, 0 if unconnected.
    pub input_channels: usize,
    /// Channels on the audio output.
    pub output_channels: usize,
    /// Host-provided oversampling (module-api §16 Q4: the host provides, the module declares need).
    pub oversampling: Oversampling,
    /// Voice count, for modules whose `voices.policy` is not `none`.
    pub voices: u32,
    /// The arena budget this module may reserve in `prepare`.
    pub arena_bytes: usize,
}

impl Resources {
    /// Resources for an offline render of `ctx`, with no oversampling and one voice.
    #[must_use]
    pub fn from_block(ctx: &BlockContext) -> Self {
        Self {
            sample_rate: ctx.sample_rate,
            block_frames: ctx.frames,
            input_channels: ctx.channels,
            output_channels: ctx.channels,
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

/// Everything `process` can reach. Deliberately small: no allocator, no clock, no filesystem, no
/// lock, and no way to obtain one.
///
/// v0 carries one interleaved audio input and one interleaved audio output. Multi-port buffers
/// arrive with the WO-008 executor, which owns buffer pooling and per-path latency.
#[derive(Debug)]
pub struct AudioCtx<'a> {
    /// Which block this is, its sample offset, rate, channel count and musical position.
    pub block: &'a BlockContext,
    /// The parameter snapshot, immutable for the whole block.
    pub params: &'a ParamSet,
    /// Interleaved input, empty when the input port is unconnected. An optional input receives an
    /// explicit unconnected signal, never silence-by-accident.
    pub input: &'a [f32],
    /// Interleaved output, exactly `block.frames * output_channels` long.
    pub output: &'a mut [f32],
}

impl AudioCtx<'_> {
    /// Frames in this block.
    #[must_use]
    pub fn frames(&self) -> usize {
        self.block.frames
    }

    /// Whether the input port is connected.
    #[must_use]
    pub fn has_input(&self) -> bool {
        !self.input.is_empty()
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
            for s in ctx.output.iter_mut() {
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
            let mut ctx = AudioCtx { block: &block, params: &params, input: &[], output: &mut out };
            assert_eq!(m.process(&mut ctx), BlockStatus::Ok);
            assert_eq!(ctx.frames(), 64);
            assert!(!ctx.has_input());
            assert_eq!(ctx.param(0), 0.0);
        }
        assert!(out.iter().all(|v| *v == 0.0), "the null module wrote silence");
    }

    #[test]
    fn resources_follow_the_block_context() {
        let block = BlockContext::offline(96_000, 64, 4);
        let r = Resources::from_block(&block);
        assert_eq!(r.sample_rate, 96_000);
        assert_eq!(r.block_frames, 64);
        assert_eq!(r.input_channels, 4);
        assert_eq!(r.output_channels, 4);
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
}
