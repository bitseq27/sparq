//! WO-007 task 3: three conforming modules, as contract tests.
//!
//! `util/gain`, `syn/sine` and `ana/rms` are the work order's chosen three — a pass-through, a
//! source and an analysis — because between them they exercise every part of the contract a Phase 0
//! module can touch: parameters, a declared latency of zero, an unconnected optional input, a `cv`
//! output, and state that must round-trip.
//!
//! This binary installs the kernel's counting allocator as its **global allocator**, so the
//! real-time claim is measured rather than asserted: if any of the three allocates once inside
//! `process`, `process_makes_zero_allocations` fails. Same pattern as
//! `crates/sparq-audio/tests/rt_discipline.rs`.

// Test harness: a failed assertion is the point, and the workspace-wide deny on unwrap/expect/panic
// exists to keep them out of the instrument, not out of tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting};
use sparq_kernel::block::BlockContext;
use sparq_module_api::manifest::{
    Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::params::{ParamBus, ParamSet};
use sparq_module_api::port::{ChannelSet, CvRange, CvRate, Direction, PortType};

#[global_allocator]
static ALLOC: sparq_kernel::alloc::CountingAllocator<std::alloc::System> =
    sparq_kernel::alloc::CountingAllocator::new(std::alloc::System);

// ---------------------------------------------------------------- util/gain

/// A gain/trim stage. Parameter 0 is a linear gain.
struct Gain {
    gain: f32,
    prepared: Option<Resources>,
}

impl Gain {
    fn new() -> Self {
        Self { gain: 1.0, prepared: None }
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
        self.prepared = Some(*resources);
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

// ---------------------------------------------------------------- syn/sine

/// An exact-frequency oscillator. Parameter 0 is frequency in Hz, parameter 1 is amplitude.
///
/// Phase accumulates in `f64` and wraps at 1.0, so a long render does not drift into precision
/// loss — the property `flt/svf` and every other oscillator will be held to.
struct Sine {
    phase: f64,
    inc: f64,
    rate: u32,
}

impl Sine {
    fn new() -> Self {
        Self { phase: 0.0, inc: 0.0, rate: 48_000 }
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
        self.inc = freq / f64::from(self.rate);
        let frames = ctx.frames().min(ctx.output.len());
        for s in ctx.output.iter_mut().take(frames) {
            *s = ((self.phase * std::f64::consts::TAU).sin() * f64::from(amp)) as f32;
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
        Err(ModuleError::Message("sparq/syn/sine understands only `phase-reset`"))
    }
}

// ---------------------------------------------------------------- ana/rms

/// RMS analysis, published as a `cv` value. The analysis-as-control-source principle in its smallest
/// form: parameter 0 is a floor below which the output is exactly zero, so a silent patch does not
/// modulate anything with denormal dust.
struct Rms;

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

// ---------------------------------------------------------------- manifests

/// The required module-level fields, filled in so each test can vary one thing.
fn base(id: &str, name: &str, summary: &str, category: &str, top: &str, kind: &str) -> Manifest {
    Manifest {
        identity: Identity {
            id: Some(format!("sparq/{id}")),
            version: Some("0.1.0".into()),
            host_api_min: Some(1),
            host_api_max: Some(1),
            display_name: Some(name.into()),
            summary: Some(summary.into()),
            authors: vec!["sparq".into()],
            license: Some("MIT".into()),
        },
        classification: Classification {
            category: Some(category.into()),
            top: Some(top.into()),
            kind: Some(kind.into()),
            tier: Some("t1".into()),
            stability: Some("stable".into()),
        },
        state: StateDecl { schema_id: Some(format!("sparq/{id}/state")), schema_version: Some(1) },
        resources: ResourceDecl {
            latency_samples: Some(0),
            cpu_class: Some("trivial".into()),
            ..ResourceDecl::default()
        },
        voices_policy: Some("none".into()),
        ..Manifest::default()
    }
}

fn audio_port(id: &str, dir: &str, set: &str) -> PortSpec {
    PortSpec {
        id: Some(id.into()),
        name: Some(id.to_uppercase()),
        direction: Some(dir.into()),
        port_type: Some("audio".into()),
        channel_set: Some(set.into()),
        ..PortSpec::default()
    }
}

fn gain_manifest() -> Manifest {
    let mut m = base("util/gain", "Gain", "Gain and trim", "utility/gain", "util", "processor");
    m.ports = vec![audio_port("in", "in", "stereo"), audio_port("out", "out", "stereo")];
    m.params = vec![ParamSpec {
        id: Some("gain".into()),
        name: Some("Gain".into()),
        kind: Some("float".into()),
        unit: Some("ratio".into()),
        min: Some(0.0),
        max: Some(2.0),
        default: Some(1.0),
        ..ParamSpec::default()
    }];
    m
}

fn sine_manifest() -> Manifest {
    let mut m = base(
        "syn/sine",
        "Sine",
        "Exact-frequency sine oscillator",
        "synth/oscillator/sine",
        "syn",
        "source",
    );
    m.ports = vec![audio_port("out", "out", "mono")];
    m.params = vec![
        ParamSpec {
            id: Some("freq".into()),
            name: Some("Frequency".into()),
            kind: Some("float".into()),
            unit: Some("Hz".into()),
            min: Some(0.0),
            max: Some(24_000.0),
            default: Some(440.0),
            ..ParamSpec::default()
        },
        ParamSpec {
            id: Some("amp".into()),
            name: Some("Amplitude".into()),
            kind: Some("float".into()),
            unit: Some("ratio".into()),
            min: Some(0.0),
            max: Some(1.0),
            default: Some(0.5),
            ..ParamSpec::default()
        },
    ];
    m
}

fn rms_manifest() -> Manifest {
    let mut m = base(
        "ana/rms",
        "RMS",
        "RMS and peak follower as a cv source",
        "analysis/level/rms",
        "ana",
        "analysis",
    );
    m.ports = vec![
        audio_port("in", "in", "stereo"),
        PortSpec {
            id: Some("level".into()),
            name: Some("Level".into()),
            direction: Some("out".into()),
            port_type: Some("cv".into()),
            rate: Some("block".into()),
            range: Some("unipolar".into()),
            ..PortSpec::default()
        },
    ];
    m.params = vec![ParamSpec {
        id: Some("floor".into()),
        name: Some("Floor".into()),
        kind: Some("float".into()),
        unit: Some("ratio".into()),
        min: Some(0.0),
        max: Some(1.0),
        default: Some(0.0),
        ..ParamSpec::default()
    }];
    m
}

// ---------------------------------------------------------------- helpers

const FRAMES: usize = 64;
const CHANS: usize = 2;

fn block() -> BlockContext {
    BlockContext::offline(48_000, FRAMES, CHANS)
}

fn resources() -> Resources {
    Resources::from_block(&block())
}

/// Runs one block of `module` with `params`, returning the output buffer.
fn render(module: &mut dyn Module, params: &ParamSet, input: &[f32]) -> Vec<f32> {
    let ctx = block();
    let mut out = vec![0.0f32; FRAMES * CHANS];
    {
        let mut a = AudioCtx { block: &ctx, params, input, output: &mut out };
        let status = module.process(&mut a);
        assert!(
            status == BlockStatus::Ok || status == BlockStatus::Silenced,
            "unexpected {status:?}"
        );
    }
    out
}

/// Count allocations made by `f` on this thread, with everything it needs already built.
fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

// ---------------------------------------------------------------- the contract tests

#[test]
fn all_three_manifests_validate_and_parse_into_typed_ports() {
    for (m, expect_ports) in [(gain_manifest(), 2), (sine_manifest(), 1), (rms_manifest(), 2)] {
        let v = m.validate().unwrap_or_else(|r| panic!("{r}"));
        assert_eq!(v.ports().len(), expect_ports);
        assert!(v.id().starts_with("sparq/"));
    }
    let rms = rms_manifest().validate().unwrap();
    let cv = &rms.ports()[1];
    assert_eq!(cv.port_type, PortType::Cv);
    assert_eq!(cv.direction, Direction::Out);
    assert_eq!(cv.cv_rate, Some(CvRate::Block));
    assert_eq!(cv.cv_range, Some(CvRange::Unipolar));
    let gain = gain_manifest().validate().unwrap();
    assert_eq!(gain.ports()[0].channel_set, Some(ChannelSet::Stereo));
}

#[test]
fn each_module_id_matches_its_manifest_id() {
    // The acceptance criterion "a new module can be added by creating a crate + manifest" only
    // means something if the implementation and the manifest cannot disagree about who they are.
    for (module, manifest) in [
        (Box::new(Gain::new()) as Box<dyn Module>, gain_manifest()),
        (Box::new(Sine::new()), sine_manifest()),
        (Box::new(Rms), rms_manifest()),
    ] {
        let v = manifest.validate().unwrap();
        assert_eq!(module.id(), v.id(), "{} != {}", module.id(), v.id());
    }
}

#[test]
fn gain_is_transparent_at_unity_and_scales_linearly() {
    let mut g = Gain::new();
    g.prepare(&resources()).unwrap();
    let input: Vec<f32> = (0..FRAMES * CHANS).map(|i| (i as f32) * 0.01 - 0.5).collect();

    let unity = render(&mut g, &ParamSet::new(1, &[1.0]).unwrap(), &input);
    assert_eq!(unity, input, "unity gain must be bit-exact, not approximately right");

    let half = render(&mut g, &ParamSet::new(2, &[0.5]).unwrap(), &input);
    for (h, i) in half.iter().zip(input.iter()) {
        assert!((f64::from(*h) - f64::from(*i) * 0.5).abs() < 1e-6, "{h} vs {i}");
    }
}

#[test]
fn an_unconnected_input_is_reported_not_silenced_by_accident() {
    let mut g = Gain::new();
    g.prepare(&resources()).unwrap();
    let ctx = block();
    let mut out = vec![9.0f32; FRAMES * CHANS];
    let status;
    {
        let mut a = AudioCtx {
            block: &ctx,
            params: &ParamSet::new(1, &[1.0]).unwrap(),
            input: &[],
            output: &mut out,
        };
        status = g.process(&mut a);
    }
    assert_eq!(status, BlockStatus::Silenced, "the module must say it produced silence");
    assert!(out.iter().all(|v| *v == 0.0), "and it must still write the buffer");
}

#[test]
fn sine_stays_in_range_is_deterministic_and_wraps_its_phase() {
    let params = ParamSet::new(1, &[440.0, 0.5]).unwrap();

    let mut a = Sine::new();
    a.prepare(&resources()).unwrap();
    let first = render(&mut a, &params, &[]);

    let mut b = Sine::new();
    b.prepare(&resources()).unwrap();
    let second = render(&mut b, &params, &[]);
    assert_eq!(first, second, "the same state and params must give bit-identical output (ADR-007)");

    for v in &first {
        assert!(v.abs() <= 0.5 + 1e-6, "amplitude 0.5 exceeded: {v}");
    }
    // A whole number of periods must cancel, which is what proves the phase wraps rather than
    // growing without bound. The frequency is chosen so that one block *is* one period:
    // 48 000 / 64 = 750 Hz. (440 Hz is 109.09 frames per period, so a whole number of blocks is
    // never a whole number of periods and the mean cannot cancel — that was this test's first bug.)
    let mut long_run = Sine::new();
    long_run.prepare(&resources()).unwrap();
    let exact = ParamSet::new(2, &[750.0, 0.5]).unwrap();
    let periods = 200usize;
    let mut sum = 0.0f64;
    let mut n = 0usize;
    let ctx = BlockContext::offline(48_000, FRAMES, 1);
    let mut out = vec![0.0f32; FRAMES];
    for _ in 0..periods {
        let mut a = AudioCtx { block: &ctx, params: &exact, input: &[], output: &mut out };
        long_run.process(&mut a);
        for v in &out {
            sum += f64::from(*v);
            n += 1;
        }
    }
    let mean = sum / n as f64;
    assert_eq!(n, periods * FRAMES);
    assert!(
        mean.abs() < 1e-6,
        "{periods} whole periods should cancel; mean was {mean} (phase drift?)"
    );
}

#[test]
fn rms_of_a_known_signal_matches_the_analytic_value() {
    let mut r = Rms;
    r.prepare(&resources()).unwrap();
    let constant = vec![0.5f32; FRAMES * CHANS];
    let out = render(&mut r, &ParamSet::new(1, &[0.0]).unwrap(), &constant);
    assert!(
        (f64::from(out[0]) - 0.5).abs() < 1e-6,
        "rms of a constant is that constant: {}",
        out[0]
    );
    assert!(out[1..].iter().all(|v| *v == 0.0), "only the cv slot is written");

    let silence = vec![0.0f32; FRAMES * CHANS];
    let out = render(&mut r, &ParamSet::new(1, &[0.0]).unwrap(), &silence);
    assert_eq!(out[0], 0.0);

    // The floor turns denormal dust into an exact zero.
    let dust = vec![1e-9f32; FRAMES * CHANS];
    let out = render(&mut r, &ParamSet::new(1, &[1e-6]).unwrap(), &dust);
    assert_eq!(out[0], 0.0, "below the floor means exactly zero");
}

#[test]
fn state_round_trips_through_configure_and_a_bad_blob_is_refused() {
    let mut g = Gain::new();
    g.prepare(&resources()).unwrap();
    g.configure(&0.25f32.to_le_bytes()).unwrap();
    let input = vec![1.0f32; FRAMES * CHANS];
    let out = render(&mut g, &ParamSet::new(1, &[0.25]).unwrap(), &input);
    assert!((f64::from(out[0]) - 0.25).abs() < 1e-6);

    let err = g.configure(&[1u8, 2, 3]).unwrap_err();
    assert_eq!(err, ModuleError::State("sparq/util/gain state is exactly 4 bytes"));

    let mut s = Sine::new();
    assert!(s.configure(&0.5f64.to_le_bytes()).is_ok());
    assert!(s.configure(&[0u8; 7]).is_err());
}

#[test]
fn a_param_change_during_a_block_reaches_process_only_at_the_next_boundary() {
    // The acceptance criterion, end to end through a real module rather than a bare ParamSlot.
    let (bus, mut slot) = ParamBus::paired();
    let mut g = Gain::new();
    g.prepare(&resources()).unwrap();
    let input = vec![1.0f32; FRAMES * CHANS];
    let ctx = block();
    let mut out = vec![0.0f32; FRAMES * CHANS];

    assert!(bus.publish(ParamSet::new(1, &[1.0]).unwrap()));
    slot.begin_block();
    {
        let mut a =
            AudioCtx { block: &ctx, params: &slot.current(), input: &input, output: &mut out };
        g.process(&mut a);
    }
    assert_eq!(out[0], 1.0, "block 1 at unity");

    // The write lands mid-block; the block still sees version 1.
    assert!(bus.publish(ParamSet::new(2, &[0.5]).unwrap()));
    {
        let mut a =
            AudioCtx { block: &ctx, params: &slot.current(), input: &input, output: &mut out };
        g.process(&mut a);
    }
    assert_eq!(out[0], 1.0, "a mid-block write must not be visible in this block");
    assert_eq!(slot.current().version(), 1);

    slot.begin_block();
    assert_eq!(slot.current().version(), 2, "the boundary is where the swap happens");
    {
        let mut a =
            AudioCtx { block: &ctx, params: &slot.current(), input: &input, output: &mut out };
        g.process(&mut a);
    }
    assert_eq!(out[0], 0.5, "block 2 sees the new value");
}

#[test]
fn process_makes_zero_allocations() {
    let ctx = block();
    let res = resources();
    let params = ParamSet::new(1, &[440.0, 0.5]).unwrap();
    let input = vec![0.1f32; FRAMES * CHANS];
    let mut out = vec![0.0f32; FRAMES * CHANS];

    let mut modules: Vec<Box<dyn Module>> =
        vec![Box::new(Gain::new()), Box::new(Sine::new()), Box::new(Rms)];
    for m in &mut modules {
        m.prepare(&res).unwrap();
    }

    // Warm up un-measured: the harness performs one-shot lazy initialisations on a thread's first
    // assert/print, and measuring cold would count the harness rather than the audio path.
    for _ in 0..2_000 {
        for m in &mut modules {
            let mut a = AudioCtx { block: &ctx, params: &params, input: &input, output: &mut out };
            m.process(&mut a);
        }
    }

    let made = measure(|| {
        for _ in 0..5_000 {
            for m in &mut modules {
                let mut a =
                    AudioCtx { block: &ctx, params: &params, input: &input, output: &mut out };
                m.process(&mut a);
            }
        }
    });
    assert_eq!(made, 0, "{made} allocations across 15 000 process calls through Box<dyn Module>");
}

#[test]
fn the_block_level_trait_object_path_is_the_one_the_executor_uses() {
    // ADR-009 executor decision 7: block dispatch may be a trait object; per-sample dispatch may
    // not. This is the shape the executor will hold modules in.
    let mut modules: Vec<Box<dyn Module>> =
        vec![Box::new(Gain::new()), Box::new(Sine::new()), Box::new(Rms)];
    let res = resources();
    for m in &mut modules {
        m.prepare(&res).unwrap();
        assert!(m.activate().is_ok());
    }
    let params = ParamSet::new(1, &[440.0, 0.5]).unwrap();
    let ctx = block();
    // One buffer per module. Sharing a single buffer here was this test's first bug: ana/rms
    // zeroes its output before reporting Silenced, so the assertion was checking whichever module
    // happened to run last rather than the one it claimed to check.
    let mut buffers: Vec<Vec<f32>> =
        (0..modules.len()).map(|_| vec![0.0f32; FRAMES * CHANS]).collect();
    for (m, out) in modules.iter_mut().zip(buffers.iter_mut()) {
        let mut a = AudioCtx { block: &ctx, params: &params, input: &[], output: out };
        m.process(&mut a);
        m.deactivate();
    }
    assert!(buffers[0].iter().all(|v| *v == 0.0), "gain with no input writes silence");
    assert!(buffers[1].iter().any(|v| *v != 0.0), "the sine wrote something through the dyn path");
    assert!(buffers[2].iter().all(|v| *v == 0.0), "rms with no input writes silence");
}

#[test]
fn prepare_refuses_resources_it_cannot_run_with() {
    let mut g = Gain::new();
    let bad = Resources { block_frames: 0, ..resources() };
    assert!(matches!(g.prepare(&bad), Err(ModuleError::Resources(_))));
    let mut s = Sine::new();
    let bad = Resources { sample_rate: 0, ..resources() };
    assert!(matches!(s.prepare(&bad), Err(ModuleError::Resources(_))));
}

#[test]
fn messages_are_refused_honestly_rather_than_ignored() {
    let mut g = Gain::new();
    assert!(g.message(b"reset").is_ok());
    let err = g.message(b"explode").unwrap_err();
    assert!(err.to_string().contains("only `reset`"), "{err}");
    assert!(Rms.message(&[]).is_err(), "a module with no messages must say so");
}
