//! First-party T1 modules (WO-014 increment 1): the three reference modules promoted from
//! contract tests to library code, registered from manifests that live **on disk** and are
//! compiled in — one copy, two consumers, no drift.
//!
//! Each module here is the WO-007 reference implementation, character for character where the
//! contract defined one (`syn/sine`'s f64 phase accumulation and 8-byte state, `util/gain`'s
//! 4-byte state and `reset` message, `ana/rms`'s f64 summing and floor), because those tests are
//! the executable spec. What promotion adds: the manifests are real files under `modules/`
//! (discovered by `sparq modules`, `include_str!`'d here), factories are registered in a
//! [`Registry`], and every claim the contract makes is re-measured against *these* builds
//! (`tests/modules_golden.rs`: zero allocations in `process`, golden hashes, properties).
//!
//! Increment 1 is three modules; the WO's seventeen land in batches ( Appendix B's Phase-1 list),
//! each batch behind the same gate: manifest validates → identity matches → golden checked in →
//! zero allocations measured.
//!
//! **Increment 2 (batch 2) shipped the six AUDIO-domain modules**: `syn/noise`, `syn/polyblep`,
//! `flt/svf`, `util/delay`, `fx/bitcrush`, `util/panner` — each a contract-conforming wrapper of
//! a Phase-B DSP primitive that already earned its measurements.
//!
//! **Contract v1 (WO-008 increment 4) migrated the batch and made two of them honest**: `ana/rms`
//! publishes on its declared `cv` port instead of riding in an audio buffer its manifest never
//! declared, and `flt/svf` grew the block-rate cv modulation input that the rms→filter acceptance
//! demo drives. **Increment 3 shipped the three the contract unblocked** (`syn/membrane`,
//! `env/ad`, `util/mixer`), and **increment 4 shipped the two the clocks unblocked** (`mod/lfo`,
//! `mod/clk-div` — the first event-PROCESSING module: events in, events out). **Increment 5 ships
//! the last three, completing the set at SEVENTEEN**: `ana/tap` (the generic signal tap — the
//! waveform a display rides, published onto the analysis ring), `dsp/scope` (the first real visual
//! module — a display whose `process` is a deliberate NO-OP, so its zero-audio-thread-cost
//! acceptance is architecture, not optimisation), and `out/main` (the master output with the
//! metering hook, shipping with the canvas master-handover rule). They waited on the ring
//! publication (ADR-009 d8, WO-008 inc 5) and the canvas handover, not on effort — declared
//! rather than faked: a module whose payload cannot travel is a lie with a manifest, and now they
//! travel.
//!
//! The v0 parameter discipline, stated once for the batch: parameters are read once per block
//! (zipper-free at block granularity, like `syn/sine`), `process` is allocation-free **at steady
//! parameters**, and a parameter CHANGE may recompute tables (`syn/polyblep`'s partial rebuild)
//! — allocation-free parameter *transitions* arrive with the control-rate design of contract v1.
//! The zero-allocation gates measure the steady state, which is the audio-thread claim.

use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::discovery::{Origin, Source};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::registry::{Factory, RegisterError, Registry};

use crate::executor::{ExecConfig, Executor};

/// The `syn/sine` manifest — the on-disk file, compiled in.
pub const SINE_MANIFEST: &str = include_str!("../../../modules/syn/sine/sparqmod.toml");
/// The `util/gain` manifest — the on-disk file, compiled in.
pub const GAIN_MANIFEST: &str = include_str!("../../../modules/util/gain/sparqmod.toml");
/// The `ana/rms` manifest — the on-disk file, compiled in.
pub const RMS_MANIFEST: &str = include_str!("../../../modules/ana/rms/sparqmod.toml");
/// The `syn/noise` manifest — the on-disk file, compiled in.
pub const NOISE_MANIFEST: &str = include_str!("../../../modules/syn/noise/sparqmod.toml");
/// The `syn/polyblep` manifest — the on-disk file, compiled in.
pub const POLYBLEP_MANIFEST: &str = include_str!("../../../modules/syn/polyblep/sparqmod.toml");
/// The `flt/svf` manifest — the on-disk file, compiled in.
pub const SVF_MANIFEST: &str = include_str!("../../../modules/flt/svf/sparqmod.toml");
/// The `util/delay` manifest — the on-disk file, compiled in.
pub const DELAY_MANIFEST: &str = include_str!("../../../modules/util/delay/sparqmod.toml");
/// The `fx/bitcrush` manifest — the on-disk file, compiled in.
pub const BITCRUSH_MANIFEST: &str = include_str!("../../../modules/fx/bitcrush/sparqmod.toml");
/// The `util/panner` manifest — the on-disk file, compiled in.
pub const PANNER_MANIFEST: &str = include_str!("../../../modules/util/panner/sparqmod.toml");
/// The `syn/membrane` manifest — the on-disk file, compiled in.
pub const MEMBRANE_MANIFEST: &str = include_str!("../../../modules/syn/membrane/sparqmod.toml");
/// The `env/ad` manifest — the on-disk file, compiled in.
pub const ENV_AD_MANIFEST: &str = include_str!("../../../modules/env/ad/sparqmod.toml");
/// The `util/mixer` manifest — the on-disk file, compiled in.
pub const MIXER_MANIFEST: &str = include_str!("../../../modules/util/mixer/sparqmod.toml");
/// The `mod/lfo` manifest — the on-disk file, compiled in.
pub const LFO_MANIFEST: &str = include_str!("../../../modules/mod/lfo/sparqmod.toml");
/// The `mod/clk-div` manifest — the on-disk file, compiled in.
pub const CLK_DIV_MANIFEST: &str = include_str!("../../../modules/mod/clk-div/sparqmod.toml");
/// The `ana/tap` manifest — the on-disk file, compiled in.
pub const TAP_MANIFEST: &str = include_str!("../../../modules/ana/tap/sparqmod.toml");
/// The `dsp/scope` manifest — the on-disk file, compiled in.
pub const SCOPE_MANIFEST: &str = include_str!("../../../modules/dsp/scope/sparqmod.toml");
/// The `out/main` manifest — the on-disk file, compiled in.
pub const OUT_MAIN_MANIFEST: &str = include_str!("../../../modules/out/main/sparqmod.toml");

// --------------------------------------------------------------------------- tempo derivation

/// Module-side tempo derivation from the block-start tick the executor already carries — the door
/// the WO-014 increment-5 DECISION chose over a per-frame tick view (which would have been a
/// contract change rippling through every implementor and the `api_snapshot` pins).
///
/// There is no clock in [`AudioCtx`] and this adds none: the module reads the musical position the
/// block context carries (WO-009's `set_musical_position` door, fed on the cross-thread side by the
/// `SetMusical` command) and DIFFS it against the previous block. `bpm = (Δtick / ppqn) / Δseconds
/// × 60`, where `Δseconds = frames / sample_rate`. A tempo EDIT thus takes effect one block late —
/// 1.3 ms at 96 kHz/64, below any musical threshold — which is the declared cost of not changing the
/// contract, recorded rather than hidden.
///
/// Honest edge cases: no previous block yet, a frozen tick (transport stopped, or the musical door
/// never set), or a backwards tick (a transport seek) all leave the estimate HELD rather than
/// inventing a tempo — `bpm()` returns the last good estimate, or `0.0` when there has never been
/// one, and callers fall back to their free-running parameter at `0.0` so a beat-locked module
/// never silently stops. This is performance state, not project state: it is deliberately NOT in any
/// state blob (a save/restore re-derives it from the transport on the next block, exactly like
/// `mod/clk-div`'s count and `syn/membrane`'s hit memory).
#[derive(Clone, Copy, Debug)]
struct TempoFollower {
    prev_tick: u64,
    have_prev: bool,
    bpm: f64,
}

impl TempoFollower {
    /// A follower with no estimate yet (`bpm() == 0.0`).
    const fn new() -> Self {
        Self { prev_tick: 0, have_prev: false, bpm: 0.0 }
    }

    /// Feed this block's musical position; returns the current bpm estimate (`0.0` = never derived).
    fn update(&mut self, tick: u64, ppqn: u32, frames: usize, sample_rate: u32) -> f64 {
        if self.have_prev && tick > self.prev_tick && ppqn > 0 && frames > 0 && sample_rate > 0 {
            let dticks = (tick - self.prev_tick) as f64;
            let dsec = frames as f64 / f64::from(sample_rate);
            let beats = dticks / f64::from(ppqn);
            let derived = beats / dsec * 60.0;
            if derived.is_finite() && derived > 0.0 {
                self.bpm = derived;
            }
        }
        // A frozen or backwards tick holds the last estimate (and re-anchors prev so a seek
        // recovers on the next advancing block rather than deriving a huge spurious tempo).
        self.prev_tick = tick;
        self.have_prev = true;
        self.bpm
    }
}

// --------------------------------------------------------------------------- syn/sine

/// An exact-frequency sine oscillator. Parameter 0 is frequency in Hz, parameter 1 amplitude.
///
/// The phase accumulator is f64 (WO-014's stated stress: "param smoothing, f64 phase
/// accumulation") and the state blob is exactly those 8 bytes, so a project save/restore lands
/// phase-continuous. Per-block parameter *smoothing* is a declared increment-2 item: v0 reads the
/// snapshot once per block, which is zipper-free at block granularity and honest about it.
#[derive(Clone, Debug)]
pub struct Sine {
    phase: f64,
    rate: u32,
}

impl Sine {
    /// A fresh oscillator at phase zero.
    #[must_use]
    pub fn new() -> Self {
        Self { phase: 0.0, rate: 48_000 }
    }
}

impl Default for Sine {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Sine {
    fn id(&self) -> &str {
        "sparq/syn/sine"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let bytes: [u8; 8] = state
            .try_into()
            .map_err(|_| ModuleError::State("sparq/syn/sine state is exactly 8 bytes (phase)"))?;
        self.phase = f64::from_le_bytes(bytes);
        Ok(())
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        self.rate = resources.sample_rate;
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let freq = f64::from(ctx.param(0));
        let amp = ctx.param(1);
        let inc = freq / f64::from(self.rate);
        let block_frames = ctx.frames();
        let out = ctx.output();
        let frames = block_frames.min(out.len());
        for s in out.iter_mut().take(frames) {
            *s = ((self.phase * std::f64::consts::TAU).sin() * f64::from(amp)) as f32;
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"phase-reset" {
            self.phase = 0.0;
            return Ok(());
        }
        Err(ModuleError::Message("sparq/syn/sine understands only `phase-reset`"))
    }
}

/// Factory for [`Sine`].
#[must_use]
pub fn create_sine() -> Box<dyn Module> {
    Box::new(Sine::new())
}

// --------------------------------------------------------------------------- util/gain

/// Gain/trim: `out = in × param(0)`. The trivial baseline (WO-014's words) and the contract's
/// transparency reference: at unity the output is **bit-exact** the input.
#[derive(Clone, Debug)]
pub struct Gain {
    gain: f32,
}

impl Gain {
    /// A fresh gain at unity.
    #[must_use]
    pub fn new() -> Self {
        Self { gain: 1.0 }
    }
}

impl Default for Gain {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Gain {
    fn id(&self) -> &str {
        "sparq/util/gain"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        // State is four bytes: the linear gain as an f32. Anything else is refused rather than
        // guessed at, because a silently misapplied state blob is a wrong-sound bug.
        let bytes: [u8; 4] = state
            .try_into()
            .map_err(|_| ModuleError::State("sparq/util/gain state is exactly 4 bytes"))?;
        self.gain = f32::from_le_bytes(bytes);
        Ok(())
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.block_frames == 0 {
            return Err(ModuleError::Resources("block_frames must be at least 1"));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        self.gain = ctx.param(0);
        if !ctx.has_input() {
            // An unconnected input is an explicit signal, never silence-by-accident: the status
            // says so, and the buffer is still written.
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        // Contract v1 borrow discipline: the input view escapes with the context's lifetime, so
        // taking it before the output borrow is not a workaround — it is the documented two-step.
        let input = ctx.input();
        for (o, i) in ctx.output().iter_mut().zip(input.iter()) {
            *o = *i * self.gain;
        }
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            self.gain = 1.0;
            return Ok(());
        }
        Err(ModuleError::Message("sparq/util/gain understands only `reset`"))
    }
}

/// Factory for [`Gain`].
#[must_use]
pub fn create_gain() -> Box<dyn Module> {
    Box::new(Gain::new())
}

// --------------------------------------------------------------------------- ana/rms

/// RMS follower as a cv source — the analysis-as-control-source principle in its smallest form.
/// Parameter 0 is a floor: below it the output is zero (a gate, not a squeeze).
///
/// Contract v1: the value travels on the module's declared block-rate `cv` output port
/// (`level`) — the v0 convention of riding in `output[0]` of an audio buffer the manifest never
/// declared is gone, because a wire that says `cv` and a payload that travels as `audio` is a
/// lie in two places. Peak following and the `data` port arrive in a later batch.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rms;

impl Rms {
    /// A fresh follower.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Module for Rms {
    fn id(&self) -> &str {
        "sparq/ana/rms"
    }

    fn configure(&mut self, _state: &[u8]) -> Result<(), ModuleError> {
        Ok(()) // stateless: an empty blob is the whole state
    }

    fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let level = if !ctx.has_input() {
            0.0
        } else {
            let mut sum = 0.0f64;
            let mut n = 0usize;
            for (i, v) in ctx.input().iter().enumerate() {
                let x = f64::from(*v);
                sum += x * x;
                n = i + 1;
            }
            if n == 0 {
                0.0
            } else {
                let rms = (sum / n as f64).sqrt();
                let floor = f64::from(ctx.param(0));
                if rms < floor {
                    0.0
                } else {
                    rms as f32
                }
            }
        };
        // The declared port carries the value; a host that did not present it (a module run
        // outside its manifest's shape — tests do this) gets the number nowhere, and the status
        // still tells the truth about the input.
        if let Some(mut cv) = ctx.cv_out(0) {
            cv.set(level);
        }
        if ctx.has_input() {
            BlockStatus::Ok
        } else {
            BlockStatus::Silenced
        }
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/ana/rms takes no messages"))
    }
}

/// Factory for [`Rms`].
#[must_use]
pub fn create_rms() -> Box<dyn Module> {
    Box::new(Rms::new())
}

// --------------------------------------------------------------------------- syn/noise

/// Seeded coloured noise (WO-014's stated stress: *seeded RNG from the seed tree*, §5.6).
/// Parameter 0 is amplitude; parameter 1 selects the colour (0 white · 1 pink · 2 brown ·
/// 3 blue · 4 violet — the manifest's int scale until `enum` params grow `options[]`).
/// The state blob is the 8-byte seed, and the `reset` message restarts the stream from it, so a
/// render is reproducible from any save point — the same promise `syn/sine` makes with phase.
#[derive(Clone, Debug)]
pub struct Noise {
    inner: crate::dsp::noise::Noise,
    seed: u64,
}

impl Noise {
    /// A fresh generator at the conventional default seed.
    #[must_use]
    pub fn new() -> Self {
        Self { inner: crate::dsp::noise::Noise::new(0x5EED), seed: 0x5EED }
    }
}

impl Default for Noise {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Noise {
    fn id(&self) -> &str {
        "sparq/syn/noise"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let bytes: [u8; 8] = state
            .try_into()
            .map_err(|_| ModuleError::State("sparq/syn/noise state is exactly 8 bytes (seed)"))?;
        self.seed = u64::from_le_bytes(bytes);
        self.inner.set_seed(self.seed);
        self.inner.reset();
        Ok(())
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        use crate::dsp::noise::NoiseColour;
        self.inner.amp = ctx.param(0);
        self.inner.colour = match ctx.param(1).round() as i32 {
            1 => NoiseColour::Pink,
            2 => NoiseColour::Brown,
            3 => NoiseColour::Blue,
            4 => NoiseColour::Violet,
            _ => NoiseColour::White,
        };
        let block_frames = ctx.frames();
        let out = ctx.output();
        let frames = block_frames.min(out.len());
        self.inner.process(&mut out[..frames]);
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            self.inner.reset();
            return Ok(());
        }
        Err(ModuleError::Message("sparq/syn/noise understands only `reset`"))
    }
}

/// Factory for [`Noise`].
#[must_use]
pub fn create_noise() -> Box<dyn Module> {
    Box::new(Noise::new())
}

// --------------------------------------------------------------------------- syn/polyblep

/// Bandlimited saw / square / pulse. The id is the stable Appendix-B id; the implementation is
/// the truncated additive synthesis that replaced the failed PolyBLEP residual (defect #12:
/// measured −29 dB aliasing against a claimed −60 dB floor — replaced rather than shipped wrong).
/// Additive has **no harmonic above Nyquist to alias**; the module-level gate re-measures the
/// floor through the registry build, so the acceptance lives at the module, not just the DSP.
///
/// Parameters: 0 freq (Hz) · 1 shape (0 saw, 1 square, 2 pulse) · 2 pulse width · 3 amp ·
/// 4 max partials (0 = the Nyquist cap; the CPU/brightness trade is a visible parameter, never a
/// hidden optimisation). State: the 8-byte phase, so a project restore lands phase-continuous.
#[derive(Clone, Debug)]
pub struct PolyBlep {
    inner: crate::dsp::osc::PolyBlepOsc,
    applied: (f64, i32, f64, usize),
}

impl PolyBlep {
    /// A fresh oscillator at 220 Hz.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: crate::dsp::osc::PolyBlepOsc::new(220.0, 0.5),
            applied: (f64::NAN, i32::MIN, f64::NAN, usize::MAX),
        }
    }
}

impl Default for PolyBlep {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for PolyBlep {
    fn id(&self) -> &str {
        "sparq/syn/polyblep"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let bytes: [u8; 8] = state.try_into().map_err(|_| {
            ModuleError::State("sparq/syn/polyblep state is exactly 8 bytes (phase)")
        })?;
        self.inner.set_phase(f64::from_le_bytes(bytes));
        Ok(())
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        self.inner.prepare(resources.sample_rate);
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        use crate::dsp::osc::WaveShape;
        let freq = f64::from(ctx.param(0));
        let shape = (ctx.param(1).round() as i32).clamp(0, 2);
        let pw = f64::from(ctx.param(2).clamp(0.05, 0.95));
        let amp = ctx.param(3);
        let partials = (ctx.param(4).round() as i32).clamp(0, 128) as usize;
        // Apply on CHANGE only: set_freq has its own rebuild guard, set_max_partials does not,
        // and a per-block re-assert of unchanged parameters must cost nothing (the defect #14
        // lesson, applied at the module layer too).
        if self.applied != (freq, shape, pw, partials) {
            self.inner.set_freq(freq);
            // shape 1 (square) and 2 (pulse) are both the pulse family at different duty cycles;
            // the setters are rebuild-guarded, so re-asserting unchanged values costs nothing.
            self.inner.set_shape(if shape == 0 { WaveShape::Saw } else { WaveShape::Square });
            self.inner.set_pulse_width(if shape == 1 { 0.5 } else { pw });
            self.inner.set_max_partials(partials);
            self.applied = (freq, shape, pw, partials);
        }
        self.inner.amp = amp;
        let block_frames = ctx.frames();
        let out = ctx.output();
        let frames = block_frames.min(out.len());
        self.inner.process(&mut out[..frames]);
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"phase-reset" {
            self.inner.reset_phase();
            return Ok(());
        }
        Err(ModuleError::Message("sparq/syn/polyblep understands only `phase-reset`"))
    }
}

/// Factory for [`PolyBlep`].
#[must_use]
pub fn create_polyblep() -> Box<dyn Module> {
    Box::new(PolyBlep::new())
}

// --------------------------------------------------------------------------- flt/svf

/// The state-variable filter (ZDF trapezoidal, Cytomic derivation — the Phase-B primitive whose
/// defect #11 caught "every filter silently a 1 Hz lowpass" and pinned the fix with a test).
/// Parameters: 0 cutoff (Hz) · 1 resonance (0..1) · 2 mode (0 lp · 1 hp · 2 bp · 3 notch ·
/// 4 peak) · 3 mod (0..1, the depth of the cv modulation input). Stereo: one filter state per
/// channel, ticked per sample — no de-interleave scratch, so `process` stays allocation-free.
///
/// **Contract v1 modulation input** (`cutoff-mod`, block-rate unipolar cv, optional): when
/// connected, the cutoff for the block is
/// `cutoff · 2^(2 · mod · cv)`, clamped to the parameter's range and quantised to f32 — the
/// parameter's own precision, deliberately, so that a hand-driven reference filter set to the
/// computed cutoff renders BIT-IDENTICAL to the modulated one. That equality is the rms→filter
/// acceptance test's arithmetic (`tests/contract_v1.rs`): the cv wire is proven exact, not
/// approximately right. At the default (`mod = 0`) or unconnected, this is byte-for-byte the
/// increment-2 filter, which the module goldens pin.
///
/// v0 state is EMPTY and says so: integrator memory (so a save/restore lands mid-ring instead of
/// restarting it) arrives with state schema v1 — an empty blob that claims to be the whole truth
/// is honest; a partial one would not be.
#[derive(Clone, Debug)]
pub struct Svf {
    ch: [crate::dsp::filter::SvfFilter; 2],
    applied: (f64, f32, i32),
}

impl Svf {
    /// A fresh stereo filter at 1 kHz.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ch: [
                crate::dsp::filter::SvfFilter::new(1000.0, 0.2),
                crate::dsp::filter::SvfFilter::new(1000.0, 0.2),
            ],
            applied: (f64::NAN, f32::NAN, i32::MIN),
        }
    }
}

impl Default for Svf {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Svf {
    fn id(&self) -> &str {
        "sparq/flt/svf"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(());
        }
        Err(ModuleError::State(
            "sparq/flt/svf v0 state is empty; integrator memory arrives with state schema v1",
        ))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        // Defect #11's remedy, honoured: coefficients are computed with the rate KNOWN.
        for f in &mut self.ch {
            f.prepare(resources.sample_rate);
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let mod_depth = ctx.param(3).clamp(0.0, 1.0);
        // Block-rate cv: `at(0)` is the block's value; Unconnected reads as an explicit 0.0, so
        // "nothing plugged in" and "plugged in at zero" both mean no modulation — the same sound
        // from the same declaration, which is what the golden needs and the contract allows.
        let cv = ctx.cv_in(0).map_or(0.0, |v| v.at(0).clamp(0.0, 1.0));
        let base = ctx.param(0).clamp(10.0, 20_000.0);
        let modulated =
            (f64::from(base) * 2.0f64.powf(2.0 * f64::from(mod_depth) * f64::from(cv))) as f32;
        let cutoff = f64::from(modulated.clamp(10.0, 20_000.0));
        let reso = ctx.param(1).clamp(0.0, 1.0);
        let mode = (ctx.param(2).round() as i32).clamp(0, 4);
        if self.applied.0 != cutoff || self.applied.1 != reso || self.applied.2 != mode {
            use crate::dsp::filter::SvfMode;
            for f in &mut self.ch {
                f.set_cutoff(cutoff);
                f.set_resonance(reso);
                f.mode = match mode {
                    1 => SvfMode::HighPass,
                    2 => SvfMode::BandPass,
                    3 => SvfMode::Notch,
                    4 => SvfMode::Peak,
                    _ => SvfMode::LowPass,
                };
            }
            self.applied = (cutoff, reso, mode);
        }
        stereo_tick(ctx, |c, v| self.ch[c].tick(v))
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/flt/svf takes no messages"))
    }
}

/// Factory for [`Svf`].
#[must_use]
pub fn create_svf() -> Box<dyn Module> {
    Box::new(Svf::new())
}

/// The shared per-sample frame walk for the stereo processors of this batch: for every frame,
/// every output channel is fed from the matching input channel (or channel 0 when the input is
/// narrower — a shape the executor's edge rules already forbid, handled rather than trusted) and
/// transformed by `tick(ch, v)` with the channel's own state (`ch` clamped to the module's two
/// states). No scratch, no allocation, no shape assumptions beyond the interleaving.
fn stereo_tick<F: FnMut(usize, f32) -> f32>(ctx: &mut AudioCtx<'_>, mut tick: F) -> BlockStatus {
    let frames = ctx.frames();
    if frames == 0 {
        return BlockStatus::Ok;
    }
    // Contract v1 borrow discipline: both views taken once, before the walk.
    let input = ctx.input();
    let output = ctx.output();
    let in_ch = (input.len() / frames).max(1);
    let out_ch = (output.len() / frames).max(1);
    for f in 0..frames {
        for c in 0..out_ch {
            let src = f * in_ch + c.min(in_ch - 1);
            let v = input.get(src).copied().unwrap_or(0.0);
            let y = tick(c.min(1), v);
            if let Some(slot) = output.get_mut(f * out_ch + c) {
                *slot = y;
            }
        }
    }
    BlockStatus::Ok
}

// --------------------------------------------------------------------------- util/delay

/// The feedback delay: fractional line, damping in the feedback path (a long tail darkens the
/// way an analogue one does), dry/wet mix, and **tempo sync** — the `set_tempo_sync` path the
/// WO-014 increment-5 decision unblocked. When `tempo-sync` is on, the delay time is a beat
/// division derived from a module-side bpm estimate ([`TempoFollower`], the same door the LFO's
/// beat-lock rides): `time = division × 60 / bpm`, no clock in the context and no contract change.
/// With no transport feeding a tick, it falls back to the `time` parameter in ms, so a tempo-synced
/// delay never silently collapses to zero.
///
/// The WO's contract stress for this module is *declared latency + legal cycles*, and both are
/// real: the manifest declares `latency = "param:time"` (the wet path delays by `time`, the dry
/// path by zero — a mixed delay has no single static number, which is exactly what the parametric
/// form exists for), and the internal feedback reads the previous sample, so the module is
/// unit-delay-safe by construction; EXTERNAL loops still need a `block_delay` edge, because
/// §5.4's cycle rule lives in the kernel, not in a module's good manners.
///
/// Parameters: 0 time (ms) · 1 feedback (0..0.95) · 2 damping (0..1) · 3 mix (0..1) ·
/// 4 tempo-sync (0 = ms, 1 = beat-locked) · 5 division (delay time in beats when tempo-sync = 1;
/// e.g. 0.25 = a sixteenth, 0.5 = an eighth, 1 = a quarter). v0 state is empty and says so: two
/// seconds of delay memory is not project state, and the tempo estimate is performance state —
/// a restore starts the tail again and re-derives tempo from the transport, declared not hidden.
#[derive(Clone, Debug)]
pub struct Delay {
    lines: [crate::dsp::delay::DelayLine; 2],
    applied: (f32, f32, f32, f32),
    rate: u32,
    tempo: TempoFollower,
}

impl Delay {
    /// A fresh stereo delay, 2 s of line allocated at construction (control-side, per the
    /// manifest's 2000 ms ceiling).
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: [crate::dsp::delay::DelayLine::new(2.0), crate::dsp::delay::DelayLine::new(2.0)],
            applied: (f32::NAN, f32::NAN, f32::NAN, f32::NAN),
            rate: 48_000,
            tempo: TempoFollower::new(),
        }
    }
}

impl Default for Delay {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Delay {
    fn id(&self) -> &str {
        "sparq/util/delay"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(());
        }
        Err(ModuleError::State(
            "sparq/util/delay v0 state is empty; delay memory is not project state",
        ))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        self.rate = resources.sample_rate;
        self.tempo = TempoFollower::new();
        for l in &mut self.lines {
            l.prepare(resources.sample_rate);
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let time_ms = ctx.param(0).clamp(1.0, 2000.0);
        let fb = ctx.param(1).clamp(0.0, 0.95);
        let damp = ctx.param(2).clamp(0.0, 1.0);
        let mix = ctx.param(3).clamp(0.0, 1.0);
        let tempo_sync = ctx.param(4).round().clamp(0.0, 1.0) as i32; // 0 = ms, 1 = beat-locked
        let division = ctx.param(5).clamp(0.0, 16.0); // delay time in beats, when beat-locked
        let bpm = self.tempo.update(ctx.block.tick, ctx.block.ppqn, ctx.frames(), self.rate);
        let synced = tempo_sync == 1 && bpm > 0.0 && division > 0.0;
        // The effective delay time in seconds, however derived — the guard key, so a steady
        // tempo/param block costs nothing. Beat-lock rides `set_tempo_sync` (the once-unreachable
        // DSP path); no transport falls back to the ms parameter so a synced delay never collapses.
        let secs =
            if synced { f64::from(division) * 60.0 / bpm } else { f64::from(time_ms) / 1000.0 };
        let key = (secs.clamp(0.0, 2.0) as f32, fb, damp, mix);
        if self.applied != key {
            for l in &mut self.lines {
                if synced {
                    l.set_tempo_sync(bpm, f64::from(division), 1.0);
                } else {
                    l.set_time(f64::from(time_ms) / 1000.0);
                }
                l.feedback = fb;
                l.damp = damp;
                l.mix = mix;
            }
            self.applied = key;
        }
        stereo_tick(ctx, |c, v| self.lines[c].tick(v))
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            for l in &mut self.lines {
                l.reset();
            }
            return Ok(());
        }
        Err(ModuleError::Message("sparq/util/delay understands only `reset`"))
    }
}

/// Factory for [`Delay`].
#[must_use]
pub fn create_delay() -> Box<dyn Module> {
    Box::new(Delay::new())
}

// --------------------------------------------------------------------------- fx/bitcrush

/// Depth + rate reduction — *deliberate aliasing as a feature* (the WO's own words): the images
/// the rate divider folds back ARE the sound, and the optional anti-alias one-pole is there for
/// the crush without the images. Parameters: 0 depth (bits, 1..16) · 1 rate divider (1..64) ·
/// 2 anti-alias amount (0..1) · 3 dither (bool). State: the 8-byte dither seed, so a crush is
/// independently reproducible — the dither PRNG is separate from any synth noise by design.
#[derive(Clone, Debug)]
pub struct BitCrusher {
    crush: [crate::dsp::bitcrush::BitCrush; 2],
    seed: u64,
    applied: (i32, i32, f32, bool),
}

impl BitCrusher {
    /// A fresh stereo crusher at the manifest defaults (8 bits, ÷4, no anti-alias, dithered).
    #[must_use]
    pub fn new() -> Self {
        let mut a = crate::dsp::bitcrush::BitCrush::new(8, 4);
        let mut b = crate::dsp::bitcrush::BitCrush::new(8, 4);
        a.set_seed(0xC5ED);
        b.set_seed(0xC5EE); // distinct per channel: no correlated dither between L and R
        Self { crush: [a, b], seed: 0xC5ED, applied: (8, 4, 0.0, true) }
    }
}

impl Default for BitCrusher {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for BitCrusher {
    fn id(&self) -> &str {
        "sparq/fx/bitcrush"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let bytes: [u8; 8] = state.try_into().map_err(|_| {
            ModuleError::State("sparq/fx/bitcrush state is exactly 8 bytes (dither seed)")
        })?;
        self.seed = u64::from_le_bytes(bytes);
        self.crush[0].set_seed(self.seed);
        self.crush[1].set_seed(self.seed.wrapping_add(1));
        Ok(())
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        for c in &mut self.crush {
            c.prepare(resources.sample_rate);
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let bits = (ctx.param(0).round() as i32).clamp(1, 16);
        let div = (ctx.param(1).round() as i32).clamp(1, 64);
        let aa = ctx.param(2).clamp(0.0, 1.0);
        let dither = ctx.param(3) >= 0.5;
        if self.applied != (bits, div, aa, dither) {
            for c in &mut self.crush {
                c.bits = bits as u32;
                c.div = div as u32;
                c.set_anti_alias(aa);
                c.dither = dither;
            }
            self.applied = (bits, div, aa, dither);
        }
        stereo_tick(ctx, |c, v| self.crush[c].tick(v))
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            for (i, c) in self.crush.iter_mut().enumerate() {
                c.set_seed(self.seed.wrapping_add(i as u64));
                c.reset();
            }
            return Ok(());
        }
        Err(ModuleError::Message("sparq/fx/bitcrush understands only `reset`"))
    }
}

/// Factory for [`BitCrusher`].
#[must_use]
pub fn create_bitcrush() -> Box<dyn Module> {
    Box::new(BitCrusher::new())
}

// --------------------------------------------------------------------------- util/panner

/// Mono → stereo panner, the spatial precursor. Parameters: 0 pan (0..100, 50 = centre) ·
/// 1 law (0 = equal-power, the constant-power cosine/sine law; 1 = linear). The gains are
/// computed once per block, not per sample — a pan move is zipper-free at block granularity
/// like every other v0 parameter. A stereo BALANCE mode (stereo input) waits for the multi-port
/// `AudioCtx`; the mono port declaration makes that absence structural, not a silent gap.
#[derive(Clone, Copy, Debug, Default)]
pub struct Panner;

impl Panner {
    /// A fresh centred panner.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Module for Panner {
    fn id(&self) -> &str {
        "sparq/util/panner"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(()); // stateless: an empty blob is the whole state
        }
        Err(ModuleError::State("sparq/util/panner is stateless; its state blob is empty"))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let p = (ctx.param(0).clamp(0.0, 100.0) / 100.0) as f64;
        let (gl, gr) = if (ctx.param(1).round() as i32) == 1 {
            (1.0 - p, p) // linear
        } else {
            let t = p * std::f64::consts::FRAC_PI_2; // equal-power
            (t.cos(), t.sin())
        };
        let (gl, gr) = (gl as f32, gr as f32);
        let frames = ctx.frames();
        let input = ctx.input();
        let output = ctx.output();
        for f in 0..frames {
            let v = input.get(f).copied().unwrap_or(0.0); // declared mono: one sample per frame
            if let Some(l) = output.get_mut(f * 2) {
                *l = v * gl;
            }
            if let Some(r) = output.get_mut(f * 2 + 1) {
                *r = v * gr;
            }
        }
        BlockStatus::Ok
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/util/panner takes no messages"))
    }
}

/// Factory for [`Panner`].
#[must_use]
pub fn create_panner() -> Box<dyn Module> {
    Box::new(Panner::new())
}

// --------------------------------------------------------------------------- syn/membrane

/// The fixed seed of the membrane's noise burst. A constant, not a parameter: v0 renders are
/// reproducible from the start (ADR-007), and a per-patch seed arrives with the seed tree —
/// declared here rather than left implicit in a constructor.
const MEMBRANE_SEED: u64 = 0x4D45_4D42_5241_4E45; // "MEMBRANE"

/// The drum-membrane voice (WO-014 increment 3): the Phase-B kick topology — measured, inside
/// the phase-b demo golden — promoted to a trigger-driven module. Per hit: a sine body at
/// `pitch` lifted by up to `punch` Hz through a FIXED fast pitch envelope (0.5/55 ms — the
/// 909 click-then-body character the recipe earned), a gated seeded noise burst, an amplitude
/// AD (`decay`), and a body lowpass (`damp`).
///
/// Contract v1 is the point of the module: the frame loop consumes the block's pre-sorted
/// events BEFORE computing each frame, so a trigger at sample 37 starts the membrane at sample
/// 37 — not at the next block, and not "somewhere in there". Events with value 0 are gate-offs
/// and are ignored (a one-shot voice says so in its manifest).
///
/// Parameters: 0 pitch (Hz) · 1 punch (Hz) · 2 decay (ms) · 3 noise (0..1) · 4 damp (Hz).
/// v0 state is empty and says so (the `flt/svf` declaration): hit memory is performance state,
/// not project state. `reset` returns the voice — envelopes, phase and the noise stream — to
/// its constructed state, so a render replays from any save point.
#[derive(Clone, Copy, Debug)]
pub struct Membrane {
    sine: crate::dsp::core::SineOsc,
    pitch_env: crate::dsp::env::AdEnv,
    noise: crate::dsp::noise::Noise,
    noise_env: crate::dsp::env::AdEnv,
    amp_env: crate::dsp::env::AdEnv,
    lp: crate::dsp::filter::SvfFilter,
    rate: u32,
    applied: (f32, f32, f32, f32, f32),
}

impl Membrane {
    /// A fresh voice with the Phase-B kick defaults.
    #[must_use]
    pub fn new() -> Self {
        use crate::dsp::core::SineOsc;
        use crate::dsp::env::{AdEnv, EnvCurve};
        use crate::dsp::filter::SvfFilter;
        use crate::dsp::noise::{Noise, NoiseColour};
        const R: u32 = 48_000;
        let mut sine = SineOsc::new(50.0, 1.0);
        sine.prepare(R);
        let mut pitch_env = AdEnv::new(0.5, 55.0);
        pitch_env.curve = EnvCurve::Exp;
        pitch_env.prepare(R);
        let mut noise = Noise::new(MEMBRANE_SEED);
        noise.colour = NoiseColour::White;
        noise.amp = 0.6;
        let mut noise_env = AdEnv::new(0.2, 18.0);
        noise_env.prepare(R);
        let mut amp_env = AdEnv::new(0.4, 260.0);
        amp_env.prepare(R);
        let mut lp = SvfFilter::new(320.0, 0.25);
        lp.prepare(R);
        Self {
            sine,
            pitch_env,
            noise,
            noise_env,
            amp_env,
            lp,
            rate: R,
            applied: (f32::NAN, f32::NAN, f32::NAN, f32::NAN, f32::NAN),
        }
    }
}

impl Default for Membrane {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Membrane {
    fn id(&self) -> &str {
        "sparq/syn/membrane"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(());
        }
        Err(ModuleError::State(
            "sparq/syn/membrane v0 state is empty; hit memory arrives with state schema v1",
        ))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        self.rate = resources.sample_rate;
        self.sine.prepare(resources.sample_rate);
        self.pitch_env.prepare(resources.sample_rate);
        self.noise_env.prepare(resources.sample_rate);
        self.amp_env.prepare(resources.sample_rate);
        self.lp.prepare(resources.sample_rate);
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // Inputs first (they escape with the context's lifetime), scalars next, and the OUTPUT
        // view LAST — the v1 borrow discipline: `output()` reborrow's the context, so every
        // `&self` read must already be done when it is taken.
        let events = ctx.events_in(0).unwrap_or(&[]);
        let block_frames = ctx.frames();

        let pitch = ctx.param(0).clamp(20.0, 200.0);
        let punch = ctx.param(1).clamp(0.0, 400.0);
        let decay = ctx.param(2).clamp(20.0, 1000.0);
        let noise_amt = ctx.param(3).clamp(0.0, 1.0);
        let damp = ctx.param(4).clamp(80.0, 2000.0);
        if self.applied != (pitch, punch, decay, noise_amt, damp) {
            self.amp_env.decay_ms = decay;
            self.amp_env.recompute();
            self.lp.set_cutoff(f64::from(damp));
            self.applied = (pitch, punch, decay, noise_amt, damp);
        }

        let rate = self.rate;
        let out = ctx.output();
        let frames = block_frames.min(out.len());
        let mut cursor = 0usize;
        for (f, slot) in out.iter_mut().enumerate().take(frames) {
            // Sample-accurate onsets: every event at or before this frame fires before the
            // frame is computed. The list is pre-sorted (the host guarantee), so one cursor
            // walk is the whole cost.
            while cursor < events.len() && events[cursor].sample as usize <= f {
                if events[cursor].value > 0.0 {
                    self.pitch_env.trigger();
                    self.noise_env.trigger();
                    self.amp_env.trigger();
                }
                cursor += 1;
            }
            // The Phase-B recipe, exactly: envelopes are TICKED (driven), never sampled — the
            // frozen-attack bug the demo's build log records is why this comment exists.
            let penv = self.pitch_env.tick();
            let freq = f64::from(pitch + punch * penv);
            self.sine.set_freq(freq, rate);
            let body = self.sine.process_mono();
            let click = self.noise.tick() * self.noise_env.tick();
            let v = (body * 0.9 + click * noise_amt) * self.amp_env.tick();
            *slot = self.lp.tick(v);
        }
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            // The LIVE rate is captured BEFORE the voice is rebuilt: `Self::new()` constructs at
            // the 48 kHz reference, and a reset between blocks at any other rate must not leave
            // the reference-rate coefficients behind (this ordering was a real bug in the first
            // draft — the kind a 48 kHz-only test suite would never catch).
            let rate = self.rate;
            *self = Self::new();
            self.rate = rate;
            self.sine.prepare(rate);
            self.pitch_env.prepare(rate);
            self.noise_env.prepare(rate);
            self.amp_env.prepare(rate);
            self.lp.prepare(rate);
            return Ok(());
        }
        Err(ModuleError::Message("sparq/syn/membrane understands only `reset`"))
    }
}

/// Factory for [`Membrane`].
#[must_use]
pub fn create_membrane() -> Box<dyn Module> {
    Box::new(Membrane::new())
}

// --------------------------------------------------------------------------- env/ad

/// The attack/decay envelope (WO-014 increment 3) — contract v1 in its purest shape: an `event`
/// input, an AUDIO-rate `cv` output, no audio ports at all. A cycle starts at the exact sample
/// offset of any event with value > 0; `loop` re-triggers at cycle end (free-running from
/// activation, which is the honest substitute for a clock until WO-009 ships); gate-off events
/// are ignored — this is AD, not ADSR, and the manifest says so rather than hiding a hold mode.
///
/// Wraps the Phase-B [`AdEnv`](crate::dsp::env::AdEnv) — the percussion workhorse whose
/// five-time-constants arrival semantics and retrigger-from-current-level smoothness are already
/// measured. Parameters: 0 attack (ms) · 1 decay (ms) · 2 loop (0/1) · 3 curve (0 exp · 1 lin).
/// v0 state is empty and says so.
#[derive(Clone, Copy, Debug)]
pub struct EnvAd {
    env: crate::dsp::env::AdEnv,
    applied: (f32, f32, i32, i32),
}

impl EnvAd {
    /// A fresh envelope at the manifest defaults (2 ms / 200 ms, exp, one-shot).
    #[must_use]
    pub fn new() -> Self {
        let mut env = crate::dsp::env::AdEnv::new(2.0, 200.0);
        env.prepare(48_000);
        Self { env, applied: (f32::NAN, f32::NAN, i32::MIN, i32::MIN) }
    }
}

impl Default for EnvAd {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for EnvAd {
    fn id(&self) -> &str {
        "sparq/env/ad"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(());
        }
        Err(ModuleError::State(
            "sparq/env/ad v0 state is empty; a mid-cycle save/restore arrives with state schema v1",
        ))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        self.env.prepare(resources.sample_rate);
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let events = ctx.events_in(0).unwrap_or(&[]);
        let frames = ctx.frames();

        let attack = ctx.param(0).clamp(0.1, 2000.0);
        let decay = ctx.param(1).clamp(1.0, 4000.0);
        let loop_on = ctx.param(2).round().clamp(0.0, 1.0) as i32;
        let curve = ctx.param(3).round().clamp(0.0, 1.0) as i32;
        if self.applied != (attack, decay, loop_on, curve) {
            self.env.attack_ms = attack;
            self.env.decay_ms = decay;
            self.env.curve = if curve == 1 {
                crate::dsp::env::EnvCurve::Linear
            } else {
                crate::dsp::env::EnvCurve::Exp
            };
            self.env.recompute();
            self.applied = (attack, decay, loop_on, curve);
        }

        let Some(mut cv) = ctx.take_cv_out(0) else {
            // A host that presents no cv port for a module whose ONLY output is cv has bigger
            // problems than this block; say so rather than tick into nowhere and look busy.
            return BlockStatus::Failed;
        };
        let Some(buf) = cv.as_audio() else {
            return BlockStatus::Failed; // declared audio-rate; a block-rate slot is a host bug
        };
        let mut cursor = 0usize;
        for f in 0..frames.min(buf.len()) {
            while cursor < events.len() && events[cursor].sample as usize <= f {
                if events[cursor].value > 0.0 {
                    self.env.trigger();
                }
                cursor += 1;
            }
            let v = self.env.tick();
            if loop_on == 1 && self.env.is_idle() {
                // Free-run: the cycle re-starts from its own end. Placed AFTER the tick so the
                // idle frame still publishes its (zero) level — the seam is one sample, and it
                // is the declared behaviour, not a glitch to hide.
                self.env.trigger();
            }
            buf[f] = v;
        }
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            self.env.reset();
            return Ok(());
        }
        Err(ModuleError::Message("sparq/env/ad understands only `reset`"))
    }
}

/// Factory for [`EnvAd`].
#[must_use]
pub fn create_env_ad() -> Box<dyn Module> {
    Box::new(EnvAd::new())
}

// --------------------------------------------------------------------------- util/mixer

/// The 4×4 stereo matrix (WO-014 increment 3): the explicit merge the connection rules require —
/// audio fan-in sums HERE, in a module the patch can see, never silently in the host. Eight
/// audio ports make it the first multi-port first-party module, riding contract v1's `take_*`
/// output pattern; per-cell gains plus per-output trims are 20 of the 32 snapshot parameters,
/// which is why the matrix is 4×4 and not 8×8 (declared in the manifest header, not discovered
/// by an author at 3 a.m.).
///
/// Defaults are the IDENTITY wiring — an untouched mixer is four parallel bit-exact
/// pass-throughs, and the golden pins exactly that. Cell order in the snapshot is row-major:
/// `param(n*4 + m)` is In n → Out m; `param(16 + m)` is Out m's trim. Sums accumulate in f64
/// (plan §5.4's rule for every summing path) and are written once, as f32.
///
/// v0 state is empty and says so: the matrix IS the parameter snapshot, which the project
/// already saves.
#[derive(Clone, Copy, Debug)]
pub struct Mixer;

impl Mixer {
    /// The matrix is stateless; the constructor exists for the factory shape.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Mixer {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Mixer {
    fn id(&self) -> &str {
        "sparq/util/mixer"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(());
        }
        Err(ModuleError::State(
            "sparq/util/mixer state is the parameter snapshot the project already saves; its own blob is empty",
        ))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.block_frames == 0 {
            return Err(ModuleError::Resources("block_frames must be at least 1"));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // Inputs first (escaped copies), then the four TAKEN output views — the multi-output
        // pattern the author guide documents: two reborrow accessors cannot coexist, two taken
        // views can.
        let ins = [ctx.audio_in(0), ctx.audio_in(1), ctx.audio_in(2), ctx.audio_in(3)];
        let outs = [
            ctx.take_audio_out(0),
            ctx.take_audio_out(1),
            ctx.take_audio_out(2),
            ctx.take_audio_out(3),
        ];
        let frames = ctx.frames();

        let mut cells = [[0.0f32; 4]; 4];
        let mut trims = [0.0f32; 4];
        for (n, row) in cells.iter_mut().enumerate() {
            for (m, cell) in row.iter_mut().enumerate() {
                *cell = ctx.param(n * 4 + m).clamp(0.0, 2.0);
            }
        }
        for (t, slot) in trims.iter_mut().enumerate() {
            *slot = ctx.param(16 + t).clamp(0.0, 2.0);
        }

        let connected = ins.iter().flatten().any(|i| !i.is_empty());
        if !connected {
            // Every input unconnected: an explicit statement, never silence-by-accident.
            for out in outs.into_iter().flatten() {
                for s in out.iter_mut() {
                    *s = 0.0;
                }
            }
            return BlockStatus::Silenced;
        }
        for (m, out) in outs.into_iter().enumerate() {
            let Some(out) = out else { continue };
            for f in 0..frames {
                for ch in 0..2 {
                    let idx = f * 2 + ch;
                    let mut acc = 0.0f64;
                    for n in 0..4 {
                        if let Some(inp) = ins[n] {
                            if let Some(&v) = inp.get(idx) {
                                acc += f64::from(cells[n][m]) * f64::from(v);
                            }
                        }
                    }
                    if let Some(slot) = out.get_mut(idx) {
                        *slot = (acc * f64::from(trims[m])) as f32;
                    }
                }
            }
        }
        BlockStatus::Ok
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/util/mixer takes no messages"))
    }
}

/// Factory for [`Mixer`].
#[must_use]
pub fn create_mixer() -> Box<dyn Module> {
    Box::new(Mixer::new())
}

// --------------------------------------------------------------------------- mod/lfo

/// The low-frequency oscillator (WO-014 increment 4): four shapes mapped into 0..1, an
/// audio-rate unipolar `cv` output, and phase reset from either the `phase-reset` message or —
/// the contract-v1 half — a `sync` event at its EXACT sample. The transport's beat/bar
/// triggers are the intended sync source (WO-009 emits them); the range and sync decisions are
/// recorded in the manifest header, not buried here.
///
/// Parameters: 0 rate (Hz) · 1 shape (0 sine · 1 tri · 2 saw · 3 square) · 2 depth (0..1) ·
/// 3 sync (0 = free-running Hz, 1 = beat-locked to the transport) · 4 division (cycles per beat
/// when sync = 1; e.g. 1 = one cycle per beat, 0.5 = one per two beats, 2 = two per beat). The
/// beat-lock is the WO-014 increment-5 DECISION realised: the rate comes from a module-side bpm
/// derivation ([`TempoFollower`]) over the block-start tick the executor already carries — no
/// contract change, and a tempo edit lands one block late (declared). With no transport feeding a
/// tick, sync = 1 falls back to the free-running `rate` so the LFO never silently stops.
/// The state blob is exactly 8 bytes — the f64 phase, the `syn/sine` promise: a save/restore
/// lands phase-continuous. The tempo estimate is performance state, deliberately NOT in the blob.
#[derive(Clone, Copy, Debug)]
pub struct Lfo {
    phase: f64,
    inc: f64,
    rate: u32,
    applied: (f32, f32),
    tempo: TempoFollower,
}

impl Lfo {
    /// A fresh oscillator at phase zero, 1 Hz.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phase: 0.0,
            inc: 1.0 / 48_000.0,
            rate: 48_000,
            applied: (f32::NAN, f32::NAN),
            tempo: TempoFollower::new(),
        }
    }

    /// The shape at phase `p` (0..1), mapped into 0..1 — the manifest header's range decision.
    #[must_use]
    fn shape(shape: i32, p: f64) -> f64 {
        match shape {
            0 => ((p * std::f64::consts::TAU).sin() + 1.0) / 2.0,
            1 => {
                if p < 0.5 {
                    2.0 * p
                } else {
                    2.0 - 2.0 * p
                }
            },
            2 => p,
            3 => {
                if p < 0.5 {
                    1.0
                } else {
                    0.0
                }
            },
            _ => 0.5, // an out-of-domain shape is a held middle, not a panic (contract §9.7)
        }
    }
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for Lfo {
    fn id(&self) -> &str {
        "sparq/mod/lfo"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let bytes: [u8; 8] = state
            .try_into()
            .map_err(|_| ModuleError::State("sparq/mod/lfo state is exactly 8 bytes (phase)"))?;
        self.phase = f64::from_le_bytes(bytes);
        Ok(())
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.sample_rate == 0 {
            return Err(ModuleError::Resources("sample_rate must be non-zero"));
        }
        self.rate = resources.sample_rate;
        self.applied = (f32::NAN, f32::NAN); // force the increment recompute on the first block
        self.tempo = TempoFollower::new(); // a resource change re-derives tempo from the transport
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // Inputs and scalars first, the taken output view last — the v1 borrow discipline.
        let events = ctx.events_in(0).unwrap_or(&[]);
        let frames = ctx.frames();
        let rate_hz = ctx.param(0).clamp(0.05, 50.0);
        let depth = ctx.param(2).clamp(0.0, 1.0);
        let shape = ctx.param(1).round().clamp(0.0, 3.0) as i32;
        let sync = ctx.param(3).round().clamp(0.0, 1.0) as i32; // 0 = free Hz, 1 = beat-locked
        let division = ctx.param(4).clamp(0.0, 64.0); // cycles per beat, when beat-locked
                                                      // BEAT LOCK (the WO-014 inc-5 decision, module-side): derive bpm from the block-start tick
                                                      // delta and run at `division` cycles per beat. A transport that has not yielded a bpm yet
                                                      // (bpm 0.0 — the musical door never set, or stopped) falls back to the free-running `rate`
                                                      // parameter, so a beat-locked LFO never silently stops; the one-block lag on a tempo EDIT is
                                                      // the declared, measured cost of not changing the contract.
        let bpm = self.tempo.update(ctx.block.tick, ctx.block.ppqn, frames, self.rate);
        let rate = if sync == 1 && bpm > 0.0 && division > 0.0 {
            ((f64::from(division) * bpm / 60.0) as f32).clamp(0.0, 1000.0)
        } else {
            rate_hz
        };
        if self.applied != (rate, depth) {
            self.inc = f64::from(rate) / f64::from(self.rate);
            self.applied = (rate, depth);
        }
        let Some(mut cv) = ctx.take_cv_out(0) else {
            // No cv port presented: run the phase so the state stays honest, publish nowhere.
            for _ in 0..frames {
                self.phase += self.inc;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
            }
            return BlockStatus::Ok;
        };
        let Some(buf) = cv.as_audio() else {
            return BlockStatus::Failed; // declared audio-rate; a block slot is a host bug
        };
        let mut cursor = 0usize;
        for (f, slot) in buf.iter_mut().enumerate().take(frames) {
            // Sample-accurate sync: an event at this frame resets the phase BEFORE the frame is
            // computed — the membrane's onset discipline, on the modulation side.
            while cursor < events.len() && events[cursor].sample as usize <= f {
                if events[cursor].value > 0.0 {
                    self.phase = 0.0;
                }
                cursor += 1;
            }
            *slot = (Self::shape(shape, self.phase) * f64::from(depth)) as f32;
            self.phase += self.inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"phase-reset" {
            self.phase = 0.0;
            return Ok(());
        }
        Err(ModuleError::Message("sparq/mod/lfo understands only `phase-reset`"))
    }
}

/// Factory for [`Lfo`].
#[must_use]
pub fn create_lfo() -> Box<dyn Module> {
    Box::new(Lfo::new())
}

// --------------------------------------------------------------------------- mod/clk-div

/// The pending-schedule capacity: multiply ≤ 8 sub-triggers per passing input, so 32 slots hold
/// four inputs' worth — a divider fed faster than its own schedule can carry is a patching
/// error, and the module degrades by dropping (counted internally), never by growing on the
/// audio thread.
const CLKDIV_PENDING_CAP: usize = 32;

/// The clock divider/multiplier with a seeded probability gate (WO-014 increment 4) — the first
/// event-PROCESSING module: events in, events out, the contract-v1 wire proven from the
/// consuming end. The exact semantics (first input passes, sub-triggers measured from the
/// previous arrival, seeded independent draws) are the manifest header's, and the goldens pin
/// them.
///
/// Parameters: 0 divide (1..16) · 1 multiply (1..8) · 2 probability (0..1). The state blob is
/// the 8-byte seed; `reset` replays the stream from it — the `syn/noise` promise.
#[derive(Clone, Copy, Debug)]
pub struct ClkDiv {
    base_seed: u64,
    rng: u64,
    count: u32,
    last_in: u64,
    has_last: bool,
    pending: [u64; CLKDIV_PENDING_CAP],
    pending_len: usize,
    dropped_pending: u64,
}

impl ClkDiv {
    /// The conventional default seed (`syn/noise`'s), so an unconfigured divider is reproducible.
    pub const DEFAULT_SEED: u64 = 0x5EED;

    /// A fresh divider at the default seed.
    #[must_use]
    pub fn new() -> Self {
        Self {
            base_seed: Self::DEFAULT_SEED,
            rng: Self::DEFAULT_SEED,
            count: 0,
            last_in: 0,
            has_last: false,
            pending: [0; CLKDIV_PENDING_CAP],
            pending_len: 0,
            dropped_pending: 0,
        }
    }

    /// xorshift64 — the seeded stream ADR-007 requires: the same seed, the same decisions.
    fn next_rand(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// The probability gate: `prob >= 1` and `prob <= 0` are exact, the middle is a draw in
    /// [0,1) from 53 bits of the stream.
    fn gate_passes(&mut self, prob: f32) -> bool {
        if prob >= 1.0 {
            return true;
        }
        if prob <= 0.0 {
            return false;
        }
        let x = (self.next_rand() >> 11) as f64 / (1u64 << 53) as f64;
        x < f64::from(prob)
    }

    /// Sorted insert into the bounded pending schedule; overflow drops the newcomer and counts.
    fn push_pending(&mut self, abs_sample: u64) {
        if self.pending_len >= CLKDIV_PENDING_CAP {
            self.dropped_pending += 1;
            return;
        }
        let mut i = self.pending_len;
        while i > 0 && self.pending[i - 1] > abs_sample {
            self.pending[i] = self.pending[i - 1];
            i -= 1;
        }
        self.pending[i] = abs_sample;
        self.pending_len += 1;
    }
}

impl Default for ClkDiv {
    fn default() -> Self {
        Self::new()
    }
}

impl Module for ClkDiv {
    fn id(&self) -> &str {
        "sparq/mod/clk-div"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        let bytes: [u8; 8] = state
            .try_into()
            .map_err(|_| ModuleError::State("sparq/mod/clk-div state is exactly 8 bytes (seed)"))?;
        self.base_seed = u64::from_le_bytes(bytes);
        self.rng = self.base_seed;
        Ok(())
    }

    fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let events = ctx.events_in(0).unwrap_or(&[]);
        let frames = ctx.frames();
        let base = ctx.block.sample_offset;
        let divide = (ctx.param(0).round() as i32).clamp(1, 16) as u32;
        let multiply = (ctx.param(1).round() as i32).clamp(1, 8) as u64;
        let prob = ctx.param(2).clamp(0.0, 1.0);

        let Some(mut sink) = ctx.take_event_out(0) else {
            // No event-out port presented — impossible through a validated manifest (the
            // executor presents every declared port); a module run outside its shape publishes
            // nowhere and counts nothing, rather than counting into the void.
            return BlockStatus::Ok;
        };
        let mut cursor = 0usize;
        for f in 0..frames {
            let abs = base + f as u64;
            // 1. sub-triggers that came due (bounded schedule, sorted — one cursor at [0]).
            while self.pending_len > 0 && self.pending[0] <= abs {
                // shift out
                for i in 0..self.pending_len - 1 {
                    self.pending[i] = self.pending[i + 1];
                }
                self.pending_len -= 1;
                if self.gate_passes(prob) {
                    sink.push(sparq_module_api::event::Event::trigger(f as u32, 1.0));
                }
            }
            // 2. inputs at this frame.
            while cursor < events.len() && events[cursor].sample as usize <= f {
                let ev = events[cursor];
                cursor += 1;
                if ev.value <= 0.0 {
                    continue; // gate-offs are not clock pulses
                }
                self.count += 1;
                if (self.count - 1) % divide != 0 {
                    continue;
                }
                // A passing input: the interval to the previous PASSING input paces the
                // multiplier's sub-triggers.
                let interval = if self.has_last && abs > self.last_in {
                    Some(abs - self.last_in)
                } else {
                    None
                };
                self.last_in = abs;
                self.has_last = true;
                if self.gate_passes(prob) {
                    sink.push(sparq_module_api::event::Event::trigger(f as u32, ev.value));
                }
                if multiply > 1 {
                    if let Some(span) = interval {
                        for k in 1..multiply {
                            self.push_pending(abs + span * k / multiply);
                        }
                    }
                }
            }
        }
        BlockStatus::Ok
    }

    fn message(&mut self, payload: &[u8]) -> Result<(), ModuleError> {
        if payload == b"reset" {
            let seed = self.base_seed;
            *self = Self::new();
            self.base_seed = seed;
            self.rng = seed;
            return Ok(());
        }
        Err(ModuleError::Message("sparq/mod/clk-div understands only `reset`"))
    }
}

/// Factory for [`ClkDiv`].
#[must_use]
pub fn create_clk_div() -> Box<dyn Module> {
    Box::new(ClkDiv::new())
}

// --------------------------------------------------------------------------- ana/tap

/// The generic signal tap for displays (WO-014 increment 5) — the analysis source the display
/// side rides. Where [`Rms`] publishes ONE block-rate level, `Tap` publishes the WAVEFORM at
/// audio rate plus the block's peak and rms, so a [`Scope`] can render a signal's shape and not
/// just its loudness.
///
/// Three declared `cv` outputs, one payload each: `wave` (audio-rate, bipolar — the signed mono
/// monitor mix, the display payload the cross-thread engine publishes onto the analysis ring),
/// `peak` and `rms` (block-rate, unipolar — magnitudes in 0..1, the `ana/rms` convention, so a
/// tap drives modulation exactly like an rms follower). `gain` scales the WAVEFORM only (a
/// display zoom); peak/rms report the TRUE signal, never the trimmed one, so a tap used for
/// modulation stays honest about level while a tap used for a scope can be zoomed.
///
/// Memoryless: v0 state is empty and says so — the waveform it publishes is the block it just
/// processed, with no follower or history behind it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Tap;

impl Tap {
    /// A fresh tap.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Module for Tap {
    fn id(&self) -> &str {
        "sparq/ana/tap"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(()); // memoryless: an empty blob is the whole state
        }
        Err(ModuleError::State("sparq/ana/tap is memoryless; its state blob is empty"))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.block_frames == 0 {
            return Err(ModuleError::Resources("block_frames must be at least 1"));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let frames = ctx.frames();
        let gain = ctx.param(0).clamp(0.0, 4.0);
        // Inputs first (they escape with the context's lifetime), so the cv outputs can be taken
        // mutably below without a borrow conflict — the v1 discipline the contract header names.
        let input = ctx.input();
        let connected = !input.is_empty();
        let in_ch = input.len().checked_div(frames).map_or(1, |q| q.max(1));

        // Peak and RMS over the TRUE signal (no display gain), summed in f64 like ana/rms.
        let mut peak = 0.0f32;
        let mut sum = 0.0f64;
        let mut n = 0usize;
        for &v in input.iter() {
            let a = v.abs();
            if a > peak {
                peak = a;
            }
            let x = f64::from(v);
            sum += x * x;
            n += 1;
        }
        let rms = if n > 0 { (sum / n as f64).sqrt() as f32 } else { 0.0 };

        // Waveform: the per-frame mono mix, scaled by the display gain, into the audio-rate port.
        if let Some(mut cv) = ctx.take_cv_out(0) {
            if let Some(buf) = cv.as_audio() {
                for f in 0..frames.min(buf.len()) {
                    let mut acc = 0.0f32;
                    for c in 0..in_ch {
                        acc += input.get(f * in_ch + c).copied().unwrap_or(0.0);
                    }
                    buf[f] = (acc / in_ch as f32) * gain;
                }
            }
        }
        // Peak and RMS: block-rate cells (manifest ports 1 and 2). A host that did not present
        // them (a module run outside its manifest's shape — tests do this) gets the number
        // nowhere, and the status still tells the truth about the input.
        if let Some(mut cv) = ctx.cv_out(1) {
            cv.set(if connected { peak } else { 0.0 });
        }
        if let Some(mut cv) = ctx.cv_out(2) {
            cv.set(if connected { rms } else { 0.0 });
        }

        if connected {
            BlockStatus::Ok
        } else {
            BlockStatus::Silenced
        }
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/ana/tap takes no messages"))
    }
}

/// Factory for [`Tap`].
#[must_use]
pub fn create_tap() -> Box<dyn Module> {
    Box::new(Tap::new())
}

// --------------------------------------------------------------------------- dsp/scope

/// The first real visual module (WO-014 increment 5): a DISPLAY, and the set's only module whose
/// `process` is a deliberate NO-OP. Zero audio-thread cost is the WO's acceptance box
/// (*"`dsp/scope` renders at ≥ 60 fps alongside audio with zero audio-thread cost"*), and it is
/// met by architecture, not optimisation: the waveform a scope shows is published by its source
/// [`Tap`] onto the analysis ring (ADR-009 d8's "published, not polled") and drawn by the UI /
/// visual thread FROM THE RING — the audio thread computes no pixel and the scope module touches
/// no sample. Rendering inside `process` would be a lie against the module's own acceptance box,
/// so it does not happen.
///
/// What the module DOES carry is the declarative half: the two audio-rate bipolar `cv` inputs
/// (`x` the horizontal deflection, `y` the vertical for X/Y mode) record in the patch GRAPH which
/// taps the scope displays, and the parameters (timebase, mode, trigger, gain, colour map)
/// configure how the UI draws them. The UI resolves each input wire to its source tap and reads
/// that tap's published waveform — the wire is the binding, the ring is the payload.
#[derive(Clone, Copy, Debug, Default)]
pub struct Scope;

impl Scope {
    /// A fresh scope.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Module for Scope {
    fn id(&self) -> &str {
        "sparq/dsp/scope"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(()); // memoryless on the audio side: the UI owns any persistence buffer
        }
        Err(ModuleError::State(
            "sparq/dsp/scope is memoryless on the audio side; its state blob is empty",
        ))
    }

    fn prepare(&mut self, _resources: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }

    fn process(&mut self, _ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // Zero audio-thread cost, deliberately. The display renders on the UI thread from the
        // analysis ring; a no-op here is the acceptance criterion, not an unfinished stub.
        BlockStatus::Ok
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/dsp/scope takes no messages"))
    }
}

/// Factory for [`Scope`].
#[must_use]
pub fn create_scope() -> Box<dyn Module> {
    Box::new(Scope::new())
}

// --------------------------------------------------------------------------- out/main

/// The master output (WO-014 increment 5) — the canonical terminus, the node the listener hears.
/// A unity-by-default pass-through with a trim and a hard mute. Because the executor meters every
/// node, `out/main`'s peak/rms ARE the master meters: that is the "metering hook" the WO names.
///
/// It has an audio OUTPUT because the executor renders the MASTER node's FIRST audio output to
/// the device buffer — a pure sink with no output would render silence as master. So the input is
/// the mix, the output is what the device plays. The input is `variable` (it accepts whatever
/// width the mix bus arrives at, resolved at prepare) and the output stereo (the v0 device
/// target). At the default trim 1.0 and mute off, an untouched `out/main` is a **bit-exact wire**
/// (`v × 1.0 == v` in IEEE-754), which its golden pins.
///
/// This module ships together with the WO-013-side master-handover rule (the canvas
/// `resolve_master` prefers an `out/main` node over the default highest-id-terminus guess), so
/// the MASTER badge never lies: when an `out/main` is in the patch, IT is the master, explicitly.
/// v0 state is empty and says so — the trim is the parameter snapshot the project already saves.
#[derive(Clone, Copy, Debug, Default)]
pub struct OutMain;

impl OutMain {
    /// A fresh master output at unity.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Module for OutMain {
    fn id(&self) -> &str {
        "sparq/out/main"
    }

    fn configure(&mut self, state: &[u8]) -> Result<(), ModuleError> {
        if state.is_empty() {
            return Ok(()); // stateless: the trim is the snapshot the project saves
        }
        Err(ModuleError::State(
            "sparq/out/main state is the parameter snapshot the project already saves; its own blob is empty",
        ))
    }

    fn prepare(&mut self, resources: &Resources) -> Result<(), ModuleError> {
        if resources.block_frames == 0 {
            return Err(ModuleError::Resources("block_frames must be at least 1"));
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let trim = ctx.param(0).clamp(0.0, 2.0);
        let muted = ctx.param(1).round() >= 1.0;
        if muted || !ctx.has_input() {
            // Mute writes EXACT zeros (never a near-zero), so a muted master is bit-exactly
            // silent and its meter reads 0 — an on-stage "cut it now" that means it.
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        // Per-channel pass with the trim; `stereo_tick` maps a narrower input by holding its
        // last channel (mono → both outs) and a wider one by taking the first `out_ch`, which is
        // the device-shape conversion the master render performs anyway.
        stereo_tick(ctx, |_c, v| v * trim)
    }

    fn message(&mut self, _payload: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("sparq/out/main takes no messages"))
    }
}

/// Factory for [`OutMain`].
#[must_use]
pub fn create_out_main() -> Box<dyn Module> {
    Box::new(OutMain::new())
}

// --------------------------------------------------------------------------- the table

/// Every built-in: id → (manifest text, factory). The single place that knows the first-party
/// set; discovery pairs these with on-disk manifests by id (§11 precedence: built-in wins).
pub const BUILTINS: [(&str, &str, Factory); 17] = [
    ("sparq/syn/sine", SINE_MANIFEST, create_sine),
    ("sparq/syn/noise", NOISE_MANIFEST, create_noise),
    ("sparq/syn/polyblep", POLYBLEP_MANIFEST, create_polyblep),
    ("sparq/syn/membrane", MEMBRANE_MANIFEST, create_membrane),
    ("sparq/flt/svf", SVF_MANIFEST, create_svf),
    ("sparq/env/ad", ENV_AD_MANIFEST, create_env_ad),
    ("sparq/util/gain", GAIN_MANIFEST, create_gain),
    ("sparq/util/delay", DELAY_MANIFEST, create_delay),
    ("sparq/util/panner", PANNER_MANIFEST, create_panner),
    ("sparq/util/mixer", MIXER_MANIFEST, create_mixer),
    ("sparq/mod/lfo", LFO_MANIFEST, create_lfo),
    ("sparq/mod/clk-div", CLK_DIV_MANIFEST, create_clk_div),
    ("sparq/fx/bitcrush", BITCRUSH_MANIFEST, create_bitcrush),
    ("sparq/ana/rms", RMS_MANIFEST, create_rms),
    ("sparq/ana/tap", TAP_MANIFEST, create_tap),
    ("sparq/dsp/scope", SCOPE_MANIFEST, create_scope),
    ("sparq/out/main", OUT_MAIN_MANIFEST, create_out_main),
];

/// The factory for a first-party id, if this build has one.
#[must_use]
pub fn first_party(id: &str) -> Option<Factory> {
    BUILTINS.iter().find(|(i, _, _)| *i == id).map(|&(_, _, f)| f)
}

/// The built-in manifests as discovery [`Source`]s (`Origin::BuiltIn`, the highest precedence).
#[must_use]
pub fn builtin_sources() -> Vec<Source> {
    BUILTINS
        .iter()
        .map(|(id, text, _)| Source::new(Origin::BuiltIn, format!("built-in:{id}"), *text))
        .collect()
}

/// Register every built-in into `registry`. Returns the count registered.
///
/// # Errors
/// The first [`RegisterError`] — a built-in that fails its own manifest validation is a build
/// bug, not a runtime condition, and must never be swallowed.
pub fn register_builtins(registry: &mut Registry) -> Result<usize, RegisterError> {
    let mut n = 0;
    for (_, text, factory) in BUILTINS {
        registry.register(text, factory)?;
        n += 1;
    }
    Ok(n)
}

// --------------------------------------------------------------------------- the demo patch

/// The increment-1 demo patch, built and ready to render:
///
/// ```text
/// syn/sine (440 Hz, amp 0.5, mono) ──► util/gain (0.5, stereo) ──► master out
///                                            └──────────────────► ana/rms (analysis tap)
/// ```
///
/// One source, one processor, one analysis tap — the smallest patch that exercises fan-out
/// (mono→stereo), the master render, and analysis-as-control-source. It is a LIBRARY function
/// so the CLI (`sparq exec`) and the golden tests render the identical graph from one copy.
pub struct DemoPatch {
    /// The built executor.
    pub executor: Executor,
    /// The sine node.
    pub sine: NodeId,
    /// The gain node (the master).
    pub gain: NodeId,
    /// The rms node (analysis tap).
    pub rms: NodeId,
}

/// Build the demo patch from a registry that has the three built-ins.
///
/// # Errors
/// A sentence naming what is missing — a registry without the built-ins is a build bug, and the
/// message says so rather than panicking.
pub fn demo_patch(registry: &Registry, cfg: ExecConfig) -> Result<DemoPatch, String> {
    let mut graph = Graph::new();
    let sine = graph.add_node(0);
    let gain = graph.add_node(0);
    let rms = graph.add_node(0);
    // sine.out (idx 0, mono) → gain.in (idx 0, stereo): the documented fan-out.
    graph
        .connect(PortRef::new(sine, 0), PortRef::new(gain, 0), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;
    // gain.out (idx 1, stereo) → rms.in (idx 0, stereo): the analysis tap. rms.level (cv) stays
    // unconnected — cv payloads wait for contract v1, and an undrawn wire is not a refusal.
    graph
        .connect(PortRef::new(gain, 1), PortRef::new(rms, 0), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;

    let mut builds = Vec::new();
    for (id, node, params) in [
        ("sparq/syn/sine", sine, &[440.0f32, 0.5][..]),
        ("sparq/util/gain", gain, &[0.5][..]),
        ("sparq/ana/rms", rms, &[0.0][..]),
    ] {
        let Some(reg) = registry.get(id) else {
            return Err(format!("`{id}` is not registered — register_builtins first"));
        };
        builds.push((
            node,
            crate::executor::NodeBuild {
                module: reg.create(),
                manifest: reg.manifest().clone(),
                params: sparq_module_api::params::ParamSet::new(1, params)
                    .ok_or_else(|| format!("`{id}` params exceed MAX_PARAMS"))?,
            },
        ));
    }
    let executor = Executor::build(graph, builds, cfg).map_err(|e| e.to_string())?;
    Ok(DemoPatch { executor, sine, gain, rms })
}

/// The rms→filter modulation demo patch (WO-014's acceptance: *"`ana/rms` output demonstrably
/// modulates a filter cutoff — the analysis-as-control-source principle proven end to end"*).
///
/// ```text
/// sine(440 Hz, 0.5) ──→ gain(0.5) ──→ svf(bp 200 Hz, mod 1.0) ──→ MASTER
///                          │
///                          └──→ rms ──(cv: level → cutoff-mod)──┘
/// ```
///
/// The cv wire is the point: the filter's cutoff for every block is
/// `200 · 2^(2 · rms)` — the louder the signal, the brighter the filter. The arithmetic proof
/// that the wire carries EXACTLY that value lives in `tests/contract_v1.rs` (a hand-driven
/// reference filter renders bit-identical); this patch is the audible artefact `sparq exec
/// --patch mod-demo` renders.
pub struct ModDemoPatch {
    /// The built executor.
    pub executor: Executor,
    /// The sine node.
    pub sine: NodeId,
    /// The gain node.
    pub gain: NodeId,
    /// The rms node (the cv source).
    pub rms: NodeId,
    /// The filter node (the master, and the cv consumer).
    pub svf: NodeId,
}

/// Build the modulation-demo patch from a registry that has the built-ins.
///
/// # Errors
/// A sentence naming what is missing or what the executor refused.
pub fn mod_demo_patch(registry: &Registry, cfg: ExecConfig) -> Result<ModDemoPatch, String> {
    let mut graph = Graph::new();
    let sine = graph.add_node(0);
    let gain = graph.add_node(0);
    let rms = graph.add_node(0);
    let svf = graph.add_node(0);
    // sine.out (mono) → gain.in (stereo): the documented fan-out.
    graph
        .connect(PortRef::new(sine, 0), PortRef::new(gain, 0), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;
    // gain.out → svf.in: the audio path (master chain).
    graph
        .connect(PortRef::new(gain, 1), PortRef::new(svf, 0), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;
    // gain.out → rms.in: the analysis tap (fan-out is free).
    graph
        .connect(PortRef::new(gain, 1), PortRef::new(rms, 0), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;
    // rms.level → svf.cutoff-mod: THE contract-v1 cv wire — block-rate unipolar into block-rate
    // unipolar, the matrix's plain `compatible` cell, carried for real.
    graph
        .connect(PortRef::new(rms, 1), PortRef::new(svf, 2), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;

    let mut builds = Vec::new();
    for (id, node, params) in [
        ("sparq/syn/sine", sine, &[440.0f32, 0.5][..]),
        ("sparq/util/gain", gain, &[0.5][..]),
        ("sparq/ana/rms", rms, &[0.0][..]),
        // cutoff 200 Hz · resonance 0.2 · mode 2 (bandpass) · mod depth 1.0
        ("sparq/flt/svf", svf, &[200.0f32, 0.2, 2.0, 1.0][..]),
    ] {
        let Some(reg) = registry.get(id) else {
            return Err(format!("`{id}` is not registered — register_builtins first"));
        };
        builds.push((
            node,
            crate::executor::NodeBuild {
                module: reg.create(),
                manifest: reg.manifest().clone(),
                params: sparq_module_api::params::ParamSet::new(1, params)
                    .ok_or_else(|| format!("`{id}` params exceed MAX_PARAMS"))?,
            },
        ));
    }
    let executor = Executor::build(graph, builds, cfg).map_err(|e| e.to_string())?;
    Ok(ModDemoPatch { executor, sine, gain, rms, svf })
}

/// The drum demo patch (WO-014 increment 3): contract v1's event wire driving real modules —
/// `syn/membrane` into `util/mixer`'s first cell, mixer out-0 as master.
///
/// ```text
/// [host triggers: four-on-the-floor] ──(event)──→ membrane ──(audio)──→ mixer(in-0 → out-0) ──→ MASTER
/// ```
///
/// The triggers are NOT in the graph: the render loop injects them through
/// [`Executor::push_host_event`] — the control-side door WO-009's transport will publish through.
/// The schedule lives in [`DrumDemoPatch::is_kick_block`] so the exec command and the golden test
/// cannot drift apart on what "four-on-the-floor" means.
pub struct DrumDemoPatch {
    /// The built executor.
    pub executor: Executor,
    /// The membrane node (its `trig` port is manifest port 0).
    pub membrane: NodeId,
    /// The mixer node (the master).
    pub mixer: NodeId,
}

impl DrumDemoPatch {
    /// Four-on-the-floor at 120 BPM: one kick every half second. Block-granular arithmetic —
    /// `blocks_per_half_second = rate / (2 · block_frames)`; at 48 kHz/64 that is every 375
    /// blocks. The kick lands at sample 0 of those blocks (the trigger's offset), which at 120
    /// BPM is exactly on the beat because 0.5 s is a whole number of blocks at every power-of-two
    /// block size this project uses.
    #[must_use]
    pub fn is_kick_block(block_index: u64, sample_rate: u32, block_frames: usize) -> bool {
        let per_hit = (u64::from(sample_rate) / (2 * block_frames as u64)).max(1);
        block_index % per_hit == 0
    }
}

/// Build the drum-demo patch from a registry that has the built-ins.
///
/// # Errors
/// A sentence naming what is missing or what the executor refused.
pub fn drum_demo_patch(registry: &Registry, cfg: ExecConfig) -> Result<DrumDemoPatch, String> {
    let mut graph = Graph::new();
    let membrane = graph.add_node(0);
    let mixer = graph.add_node(0);
    // membrane.out (manifest port 1, mono) → mixer.in-0 (manifest port 0, stereo): the
    // documented mono→multi fan-out, into the identity cell c00.
    graph
        .connect(PortRef::new(membrane, 1), PortRef::new(mixer, 0), EdgeKind::Plain)
        .map_err(|e| e.to_string())?;
    let mut builds = Vec::new();
    for (id, node, params) in [
        // pitch 50 · punch 130 · decay 260 · noise 0.5 · damp 320 — the Phase-B kick, verbatim.
        ("sparq/syn/membrane", membrane, &[50.0f32, 130.0, 260.0, 0.5, 320.0][..]),
        // The identity matrix, with the out-0 trim at 0.6 as the demo's bus headroom: the
        // membrane's transient peaks near 1.5 (body 0.9 + click 0.5 at full envelope — the
        // Phase-B recipe's own arithmetic), and a demo that clips teaches the wrong lesson.
        // Explicit values, not manifest defaults, so the render cannot move when a default does.
        (
            "sparq/util/mixer",
            mixer,
            &[
                1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
                0.6, 1.0, 1.0, 1.0,
            ][..],
        ),
    ] {
        let Some(reg) = registry.get(id) else {
            return Err(format!("`{id}` is not registered — register_builtins first"));
        };
        builds.push((
            node,
            crate::executor::NodeBuild {
                module: reg.create(),
                manifest: reg.manifest().clone(),
                params: sparq_module_api::params::ParamSet::new(1, params)
                    .ok_or_else(|| format!("`{id}` params exceed MAX_PARAMS"))?,
            },
        ));
    }
    let executor = Executor::build(graph, builds, cfg).map_err(|e| e.to_string())?;
    Ok(DrumDemoPatch { executor, membrane, mixer })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use sparq_kernel::block::BlockContext;
    use sparq_module_api::decode;
    use sparq_module_api::module::CvOut;
    use sparq_module_api::params::ParamSet;

    const FRAMES: usize = 64;

    fn ctx2() -> BlockContext {
        BlockContext::offline(48_000, FRAMES, 2)
    }
    fn res() -> Resources {
        Resources::from_block(&ctx2())
    }

    #[test]
    fn every_builtin_manifest_validates_and_matches_its_implementation() {
        // The contract's first invariant, enforced where the modules live: the compiled-in
        // manifest text decodes, validates, and names the same module the factory builds.
        for (id, text, factory) in BUILTINS {
            let v = decode(text).unwrap_or_else(|r| panic!("{id}: {r}"));
            assert_eq!(v.id(), id);
            let m = factory();
            assert_eq!(m.id(), id, "{id}: the factory and the manifest disagree");
        }
    }

    #[test]
    fn register_builtins_fills_a_registry_and_refuses_duplicates() {
        let mut reg = Registry::new();
        assert_eq!(register_builtins(&mut reg).unwrap(), BUILTINS.len());
        for (id, _, _) in BUILTINS {
            assert!(reg.get(id).is_some(), "{id} did not register");
        }
        assert!(
            matches!(
                reg.register(SINE_MANIFEST, create_sine),
                Err(RegisterError::Duplicate { .. })
            ),
            "a second sine must be refused, not replace the first (§11: pins, never latest-wins)"
        );
        assert!(first_party("sparq/nope").is_none());
    }

    #[test]
    fn sine_holds_its_frequency_and_phase_continues_across_blocks() {
        let mut s = Sine::new();
        s.prepare(&res()).unwrap();
        let params = ParamSet::new(1, &[440.0, 1.0]).unwrap();
        let ctx = ctx2();
        // Mono output: `frames = ctx.frames().min(output.len())` writes 64 sequential samples.
        let mut out = vec![0.0f32; FRAMES];
        let mut crossings = 0usize;
        let mut last = 0.0f32;
        // 750 blocks × 64 frames = 1 s at 48 kHz; 440 Hz must cross zero ≈ 880 times.
        for _ in 0..750 {
            let mut a = AudioCtx::single(&ctx, &params, &[], &mut out);
            s.process(&mut a);
            for v in out.iter().take(FRAMES) {
                if (last < 0.0 && *v >= 0.0) || (last > 0.0 && *v <= 0.0) {
                    crossings += 1;
                }
                last = *v;
            }
        }
        assert!(
            (crossings as i32 - 880).abs() <= 2,
            "440 Hz for 1 s ≈ 880 crossings, got {crossings}"
        );
        assert!(s.phase >= 0.0 && s.phase < 1.0, "phase stays wrapped: {}", s.phase);
    }

    #[test]
    fn sine_state_roundtrips_and_bad_state_is_refused() {
        let mut s = Sine::new();
        s.configure(&0.25f64.to_le_bytes()).unwrap();
        assert!((s.phase - 0.25).abs() < 1e-12);
        assert!(matches!(s.configure(&[1, 2, 3]), Err(ModuleError::State(_))));
        s.message(b"phase-reset").unwrap();
        assert_eq!(s.phase, 0.0);
        assert!(matches!(s.message(b"nope"), Err(ModuleError::Message(_))));
    }

    #[test]
    fn gain_is_transparent_at_unity_and_scales_linearly() {
        let mut g = Gain::new();
        g.prepare(&res()).unwrap();
        let ctx = ctx2();
        let input: Vec<f32> = (0..FRAMES * 2).map(|i| i as f32 * 0.001 - 0.064).collect();
        let mut out = vec![0.0f32; FRAMES * 2];
        let unity = ParamSet::new(1, &[1.0]).unwrap();
        {
            let mut a = AudioCtx::single(&ctx, &unity, &input, &mut out);
            g.process(&mut a);
        }
        assert_eq!(out, input, "unity gain must be bit-exact");
        let half = ParamSet::new(2, &[0.5]).unwrap();
        {
            let mut a = AudioCtx::single(&ctx, &half, &input, &mut out);
            g.process(&mut a);
        }
        for (o, i) in out.iter().zip(input.iter()) {
            assert!((f64::from(*o) - f64::from(*i) * 0.5).abs() < 1e-6);
        }
        // State + message paths.
        g.configure(&0.25f32.to_le_bytes()).unwrap();
        assert!(matches!(g.configure(&[1, 2]), Err(ModuleError::State(_))));
        g.message(b"reset").unwrap();
        assert!(matches!(g.message(b"nope"), Err(ModuleError::Message(_))));
    }

    #[test]
    fn gain_reports_an_unconnected_input_rather_than_silencing_by_accident() {
        let mut g = Gain::new();
        g.prepare(&res()).unwrap();
        let ctx = ctx2();
        let mut out = vec![9.0f32; FRAMES * 2];
        let p = ParamSet::new(1, &[1.0]).unwrap();
        let mut a = AudioCtx::single(&ctx, &p, &[], &mut out);
        let st = g.process(&mut a);
        assert_eq!(st, BlockStatus::Silenced);
        assert!(out.iter().all(|&s| s == 0.0), "the buffer is still written");
    }

    #[test]
    fn rms_of_a_dc_input_is_the_value_and_the_floor_gates() {
        let mut r = Rms::new();
        r.prepare(&res()).unwrap();
        let ctx = ctx2();
        let input = vec![0.5f32; FRAMES * 2];
        // Contract v1: the value travels on the declared cv port, not in an audio buffer the
        // manifest never declared. NAN seed: an unwritten cell must fail loudly, not read as 0.
        let mut cell = f32::NAN;
        let p = ParamSet::new(1, &[0.0]).unwrap();
        {
            let mut a =
                AudioCtx::single(&ctx, &p, &input, &mut []).with_cv_out(CvOut::Block(&mut cell));
            r.process(&mut a);
        }
        assert!((cell - 0.5).abs() < 1e-6, "rms of DC 0.5 is 0.5, got {cell}");
        let gated = ParamSet::new(2, &[0.75]).unwrap();
        {
            let mut a = AudioCtx::single(&ctx, &gated, &input, &mut [])
                .with_cv_out(CvOut::Block(&mut cell));
            r.process(&mut a);
        }
        assert_eq!(cell, 0.0, "below the floor the output is zero");
    }
}
