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

// --------------------------------------------------------------------------- the table

/// Every built-in: id → (manifest text, factory). The single place that knows the first-party
/// set; discovery pairs these with on-disk manifests by id (§11 precedence: built-in wins).
pub const BUILTINS: [(&str, &str, Factory); 3] = [
    ("sparq/syn/sine", SINE_MANIFEST, create_sine),
    ("sparq/util/gain", GAIN_MANIFEST, create_gain),
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
        assert_eq!(register_builtins(&mut reg).unwrap(), 3);
        assert!(reg.get("sparq/syn/sine").is_some());
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
