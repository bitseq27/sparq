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
//! **Increment 2 (batch 2) ships the six AUDIO-domain modules**: `syn/noise`, `syn/polyblep`,
//! `flt/svf`, `util/delay`, `fx/bitcrush`, `util/panner` — each a contract-conforming wrapper of
//! a Phase-B DSP primitive that already earned its measurements. The remaining eight
//! (`syn/membrane`, `env/ad`, `mod/lfo`, `mod/clk-div`, `util/mixer`, `ana/tap`, `dsp/scope`,
//! `out/main`) are event/cv/multi-port/display domain: they wait for the multi-port `AudioCtx`
//! (contract v1) and WO-009's clocks, declared rather than faked — a module whose trigger port
//! cannot receive a trigger is a lie with a manifest.
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
        let frames = ctx.frames().min(ctx.output.len());
        for s in ctx.output.iter_mut().take(frames) {
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
            for s in ctx.output.iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        for (o, i) in ctx.output.iter_mut().zip(ctx.input.iter()) {
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
/// Parameter 0 is a floor: below it the output is zero (a gate, not a squeeze). The value rides
/// in `output[0]` per block (the v0 convention the contract reference established); peak
/// following and the `data` port arrive in a later batch.
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
        for s in ctx.output.iter_mut() {
            *s = 0.0;
        }
        if !ctx.has_input() {
            return BlockStatus::Silenced;
        }
        let mut sum = 0.0f64;
        let mut n = 0usize;
        for (i, v) in ctx.input.iter().enumerate() {
            let x = f64::from(*v);
            sum += x * x;
            n = i + 1;
        }
        if n == 0 {
            return BlockStatus::Ok;
        }
        let rms = (sum / n as f64).sqrt();
        let floor = f64::from(ctx.param(0));
        ctx.output[0] = if rms < floor { 0.0 } else { rms as f32 };
        BlockStatus::Ok
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
        let frames = ctx.frames().min(ctx.output.len());
        self.inner.process(&mut ctx.output[..frames]);
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
        let frames = ctx.frames().min(ctx.output.len());
        self.inner.process(&mut ctx.output[..frames]);
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
/// 4 peak). Stereo: one filter state per channel, ticked per sample — no de-interleave scratch,
/// so `process` stays allocation-free.
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
            for s in ctx.output.iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let cutoff = f64::from(ctx.param(0).clamp(10.0, 20_000.0));
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
    let in_ch = (ctx.input.len() / frames).max(1);
    let out_ch = (ctx.output.len() / frames).max(1);
    for f in 0..frames {
        for c in 0..out_ch {
            let src = f * in_ch + c.min(in_ch - 1);
            let v = ctx.input.get(src).copied().unwrap_or(0.0);
            let y = tick(c.min(1), v);
            if let Some(slot) = ctx.output.get_mut(f * out_ch + c) {
                *slot = y;
            }
        }
    }
    BlockStatus::Ok
}

// --------------------------------------------------------------------------- util/delay

/// The feedback delay: fractional line, damping in the feedback path (a long tail darkens the
/// way an analogue one does), dry/wet mix, and tempo sync through `set_tempo_sync` when a
/// transport exists (WO-009 — until then `time` in ms is the source of truth).
///
/// The WO's contract stress for this module is *declared latency + legal cycles*, and both are
/// real: the manifest declares `latency = "param:time"` (the wet path delays by `time`, the dry
/// path by zero — a mixed delay has no single static number, which is exactly what the parametric
/// form exists for), and the internal feedback reads the previous sample, so the module is
/// unit-delay-safe by construction; EXTERNAL loops still need a `block_delay` edge, because
/// §5.4's cycle rule lives in the kernel, not in a module's good manners.
///
/// Parameters: 0 time (ms) · 1 feedback (0..0.95) · 2 damping (0..1) · 3 mix (0..1). v0 state is
/// empty and says so: two seconds of delay memory is not project state — a restore starts the
/// tail again, declared rather than hidden.
#[derive(Clone, Debug)]
pub struct Delay {
    lines: [crate::dsp::delay::DelayLine; 2],
    applied: (f32, f32, f32, f32),
}

impl Delay {
    /// A fresh stereo delay, 2 s of line allocated at construction (control-side, per the
    /// manifest's 2000 ms ceiling).
    #[must_use]
    pub fn new() -> Self {
        Self {
            lines: [crate::dsp::delay::DelayLine::new(2.0), crate::dsp::delay::DelayLine::new(2.0)],
            applied: (f32::NAN, f32::NAN, f32::NAN, f32::NAN),
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
        for l in &mut self.lines {
            l.prepare(resources.sample_rate);
        }
        Ok(())
    }

    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output.iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let time = ctx.param(0).clamp(1.0, 2000.0);
        let fb = ctx.param(1).clamp(0.0, 0.95);
        let damp = ctx.param(2).clamp(0.0, 1.0);
        let mix = ctx.param(3).clamp(0.0, 1.0);
        if self.applied != (time, fb, damp, mix) {
            for l in &mut self.lines {
                l.set_time(f64::from(time) / 1000.0);
                l.feedback = fb;
                l.damp = damp;
                l.mix = mix;
            }
            self.applied = (time, fb, damp, mix);
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
            for s in ctx.output.iter_mut() {
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
            for s in ctx.output.iter_mut() {
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
        for f in 0..frames {
            let v = ctx.input.get(f).copied().unwrap_or(0.0); // declared mono: one sample per frame
            if let Some(l) = ctx.output.get_mut(f * 2) {
                *l = v * gl;
            }
            if let Some(r) = ctx.output.get_mut(f * 2 + 1) {
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

// --------------------------------------------------------------------------- the table

/// Every built-in: id → (manifest text, factory). The single place that knows the first-party
/// set; discovery pairs these with on-disk manifests by id (§11 precedence: built-in wins).
pub const BUILTINS: [(&str, &str, Factory); 9] = [
    ("sparq/syn/sine", SINE_MANIFEST, create_sine),
    ("sparq/syn/noise", NOISE_MANIFEST, create_noise),
    ("sparq/syn/polyblep", POLYBLEP_MANIFEST, create_polyblep),
    ("sparq/flt/svf", SVF_MANIFEST, create_svf),
    ("sparq/util/gain", GAIN_MANIFEST, create_gain),
    ("sparq/util/delay", DELAY_MANIFEST, create_delay),
    ("sparq/util/panner", PANNER_MANIFEST, create_panner),
    ("sparq/fx/bitcrush", BITCRUSH_MANIFEST, create_bitcrush),
    ("sparq/ana/rms", RMS_MANIFEST, create_rms),
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use sparq_kernel::block::BlockContext;
    use sparq_module_api::decode;
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
            let mut a = AudioCtx { block: &ctx, params: &params, input: &[], output: &mut out };
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
            let mut a = AudioCtx { block: &ctx, params: &unity, input: &input, output: &mut out };
            g.process(&mut a);
        }
        assert_eq!(out, input, "unity gain must be bit-exact");
        let half = ParamSet::new(2, &[0.5]).unwrap();
        {
            let mut a = AudioCtx { block: &ctx, params: &half, input: &input, output: &mut out };
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
        let mut a = AudioCtx { block: &ctx, params: &p, input: &[], output: &mut out };
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
        let mut out = vec![9.0f32; FRAMES]; // rms out_ch = 1 by the executor's cv convention
        let p = ParamSet::new(1, &[0.0]).unwrap();
        {
            let mut a = AudioCtx { block: &ctx, params: &p, input: &input, output: &mut out };
            r.process(&mut a);
        }
        assert!((out[0] - 0.5).abs() < 1e-6, "rms of DC 0.5 is 0.5, got {}", out[0]);
        let gated = ParamSet::new(2, &[0.75]).unwrap();
        {
            let mut a = AudioCtx { block: &ctx, params: &gated, input: &input, output: &mut out };
            r.process(&mut a);
        }
        assert_eq!(out[0], 0.0, "below the floor the output is zero");
    }
}
