//! Contract v1 acceptance (WO-008 increment 4): cv and event payloads travel, multi-port audio
//! buffers are independent, and the WO-014 acceptance item — *"`ana/rms` output demonstrably
//! modulates a filter cutoff"* — is proven with arithmetic, not adjectives: a modulated filter
//! renders **bit-identical** to a hand-driven reference whose cutoff parameter is set, per block,
//! to exactly the value the contract says the wire must carry.
//!
//! This binary installs the kernel's counting allocator as its **global allocator** (the
//! `executor.rs`/`rt_discipline.rs` pattern): the zero-allocation claim covers the cv and event
//! wire paths too, measured rather than asserted.
//!
//! The modules below are test-local probes and emitters — the throwaway-module discipline: the
//! engine sources never learn their names.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, ExecError, Executor, LatencyMode, NodeBuild, Watchdog};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::{mod_demo_patch, register_builtins};
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::event::{Event, EVENTS_PER_BLOCK};
use sparq_module_api::manifest::{
    Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
};
use sparq_module_api::module::{AudioCtx, BlockStatus, CvIn, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

fn cfg(ch: usize) -> ExecConfig {
    ExecConfig {
        sample_rate: RATE,
        block_frames: FRAMES,
        device_channels: ch,
        latency: LatencyMode::default(),
        watchdog: Watchdog::default(),
    }
}

// ------------------------------------------------------------------ manifest helpers

fn base(id: &str, ports: Vec<PortSpec>, params: Vec<ParamSpec>) -> Manifest {
    let mut m = Manifest {
        identity: Identity {
            id: Some(format!("sparq/test/{id}")),
            version: Some("0.1.0".into()),
            host_api_min: Some(1),
            host_api_max: Some(1),
            display_name: Some(id.into()),
            summary: Some("contract-v1 test module".into()),
            authors: vec!["sparq".into()],
            license: Some("MIT".into()),
        },
        classification: Classification {
            category: Some(format!("utility/{id}")),
            top: Some("util".into()),
            kind: Some("processor".into()),
            tier: Some("t1".into()),
            layer: None,
            stability: Some("experimental".into()),
        },
        state: StateDecl {
            schema_id: Some(format!("sparq/test/{id}/state")),
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
    m.ports = ports;
    m.params = params;
    m
}

fn audio_port(id: &str, dir: &str, set: &str) -> PortSpec {
    PortSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        direction: Some(dir.into()),
        port_type: Some("audio".into()),
        channel_set: Some(set.into()),
        required: Some(false),
        ..PortSpec::default()
    }
}

fn cv_port(id: &str, dir: &str, rate: &str, range: &str) -> PortSpec {
    PortSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        direction: Some(dir.into()),
        port_type: Some("cv".into()),
        rate: Some(rate.into()),
        range: Some(range.into()),
        required: Some(false),
        ..PortSpec::default()
    }
}

fn event_port(id: &str, dir: &str, kinds: &[&str]) -> PortSpec {
    PortSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        direction: Some(dir.into()),
        port_type: Some("event".into()),
        event_kinds: kinds.iter().map(|k| (*k).to_string()).collect(),
        required: Some(false),
        ..PortSpec::default()
    }
}

fn nparam(id: &str, def: f64, min: f64, max: f64) -> ParamSpec {
    ParamSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        kind: Some("float".into()),
        unit: Some("x".into()),
        min: Some(min),
        max: Some(max),
        default: Some(def),
        ..ParamSpec::default()
    }
}

fn validated(m: Manifest) -> sparq_module_api::ValidatedManifest {
    m.validate().unwrap_or_else(|r| panic!("{r}"))
}

fn params(values: &[f32]) -> ParamSet {
    ParamSet::new(1, values).unwrap()
}

// ------------------------------------------------------------------ test modules

/// Publishes `param(0)` on its block-rate cv output. No audio ports at all — the smallest cv
/// source, and proof that a module without audio buffers is a first-class citizen in v1.
struct CvConst;
impl Module for CvConst {
    fn id(&self) -> &str {
        "sparq/test/cv-const"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let v = ctx.param(0);
        if let Some(mut cv) = ctx.cv_out(0) {
            cv.set(v);
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("cv-const takes no messages"))
    }
}
fn cv_const_manifest(range: &str) -> Manifest {
    base(
        "cv-const",
        vec![cv_port("level", "out", "block", range)],
        vec![nparam("level", 0.0, -1.0, 1.0)],
    )
}

/// Publishes a fixed permutation ramp on an AUDIO-rate cv output: `v[f] = ((f·7) mod 64)/64` —
/// a 0..63 permutation scaled into unipolar, so every `cv_reduce` policy computes a DIFFERENT
/// hand-checkable number (first 0, last 57/64, mean 31.5/64, min 0, max = peak = 63/64).
struct CvRamp;
impl Module for CvRamp {
    fn id(&self) -> &str {
        "sparq/test/cv-ramp"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if let Some(mut cv) = ctx.cv_out(0) {
            if let Some(buf) = cv.as_audio() {
                for (f, s) in buf.iter_mut().enumerate() {
                    *s = ((f * 7) % FRAMES) as f32 / FRAMES as f32;
                }
            }
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("cv-ramp takes no messages"))
    }
}
fn cv_ramp_manifest() -> Manifest {
    base("cv-ramp", vec![cv_port("level", "out", "audio", "unipolar")], vec![])
}

/// Renders whatever its cv input carries into a mono audio output, so a test can READ the wire:
/// `Block(v)` → constant v · `Audio(s)` → copy of s · `Unconnected` → the −2.0 marker · no port
/// at all → the −3.0 marker. Markers, not silence: an unwritten buffer must not read as a value.
struct CvProbe;
impl Module for CvProbe {
    fn id(&self) -> &str {
        "sparq/test/cv-probe"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let cv = ctx.cv_in(0); // inputs first: they escape with the context's lifetime
        let out = ctx.output();
        match cv {
            None => {
                for s in out.iter_mut() {
                    *s = -3.0;
                }
            },
            Some(CvIn::Unconnected) => {
                for s in out.iter_mut() {
                    *s = -2.0;
                }
            },
            Some(cv) => {
                if let Some(src) = cv.audio() {
                    let n = out.len().min(src.len());
                    out[..n].copy_from_slice(&src[..n]);
                } else {
                    let v = cv.at(0);
                    for s in out.iter_mut() {
                        *s = v;
                    }
                }
            },
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("cv-probe takes no messages"))
    }
}
fn cv_probe_manifest(rate: &str, range: &str) -> Manifest {
    let mut p = cv_port("mod", "in", rate, range);
    p.cv_reduce = None;
    p.cv_interp = None;
    base("cv-probe", vec![p, audio_port("out", "out", "mono")], vec![])
}
fn cv_probe_manifest_with(rate: &str, reduce: Option<&str>, interp: Option<&str>) -> Manifest {
    let mut p = cv_port("mod", "in", rate, "unipolar");
    p.cv_reduce = reduce.map(str::to_string);
    p.cv_interp = interp.map(str::to_string);
    base("cv-probe", vec![p, audio_port("out", "out", "mono")], vec![])
}

/// Emits triggers on its event output: `param(0)` events at `param(1) + i·param(3)` samples,
/// channel `param(2)`. Deliberately capable of emitting out of order (a negative step) and over
/// capacity (a count past [`EVENTS_PER_BLOCK`]) — the host guarantees sorting, and counts drops.
struct EvEmitter;
impl Module for EvEmitter {
    fn id(&self) -> &str {
        "sparq/test/ev-emitter"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let count = ctx.param(0) as i32;
        let first = ctx.param(1);
        let channel = ctx.param(2) as u16;
        let step = ctx.param(3);
        if let Some(mut sink) = ctx.event_out(0) {
            for i in 0..count.max(0) {
                let sample = first + i as f32 * step;
                if sample < 0.0 {
                    continue;
                }
                sink.push(Event { channel, ..Event::trigger(sample as u32, 0.5) });
            }
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("ev-emitter takes no messages"))
    }
}
fn ev_emitter_manifest() -> Manifest {
    base(
        "ev-emitter",
        vec![event_port("trig", "out", &["trigger"])],
        vec![
            nparam("count", 0.0, 0.0, 128.0),
            nparam("first", 0.0, 0.0, 63.0),
            nparam("channel", 0.0, 0.0, 15.0),
            nparam("step", 1.0, -64.0, 64.0),
        ],
    )
}

/// Transcribes its event input into a mono audio output: slot `i` holds
/// `channel + sample/1000` for the `i`-th event of the block, so a test reads the delivered
/// ORDER and IDENTITY off the render. An empty list writes the −1.0 marker in slot 0; a missing
/// port writes −3.0. (Slots past the event count are left at the buffer's zero.)
struct EvProbe;
impl Module for EvProbe {
    fn id(&self) -> &str {
        "sparq/test/ev-probe"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let events = ctx.events_in(0); // inputs first: they escape with the context's lifetime
        let out = ctx.output();
        for s in out.iter_mut() {
            *s = 0.0;
        }
        match events {
            None => {
                if let Some(s) = out.first_mut() {
                    *s = -3.0;
                }
            },
            Some(events) => {
                if events.is_empty() {
                    if let Some(s) = out.first_mut() {
                        *s = -1.0;
                    }
                }
                for (i, e) in events.iter().enumerate().take(out.len()) {
                    out[i] = f32::from(e.channel) + e.sample as f32 / 1000.0;
                }
            },
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("ev-probe takes no messages"))
    }
}
fn ev_probe_manifest(kinds: &[&str]) -> Manifest {
    base(
        "ev-probe",
        vec![event_port("trig", "in", kinds), audio_port("out", "out", "mono")],
        vec![],
    )
}

/// Two audio in, two audio out: `out0 = in0 + in1`, `out1 = in0 − in1`. The multi-port proof:
/// per-port buffers stay independent and `audio_in(i)`/`audio_out(i)` follow manifest order.
struct MultiProc;
impl Module for MultiProc {
    fn id(&self) -> &str {
        "sparq/test/multi-proc"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // The multi-port pattern contract v1 documents: inputs are copies and coexist freely;
        // the two OUTPUT views are TAKEN, because two `&mut self` reborrow accessors cannot.
        let (a, b) = (ctx.audio_in(0), ctx.audio_in(1));
        let (sum, mut diff) = (ctx.take_audio_out(0), ctx.take_audio_out(1));
        let (Some(sum), Some(a), Some(b)) = (sum, a, b) else {
            return BlockStatus::Silenced;
        };
        for i in 0..sum.len() {
            let (x, y) = (a.get(i).copied().unwrap_or(0.0), b.get(i).copied().unwrap_or(0.0));
            sum[i] = x + y;
            if let Some(d) = diff.as_deref_mut() {
                if i < d.len() {
                    d[i] = x - y;
                }
            }
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("multi-proc takes no messages"))
    }
}
fn multi_proc_manifest() -> Manifest {
    base(
        "multi-proc",
        vec![
            audio_port("in-a", "in", "mono"),
            audio_port("in-b", "in", "mono"),
            audio_port("sum", "out", "mono"),
            audio_port("diff", "out", "mono"),
        ],
        vec![],
    )
}

/// A constant DC source (mono audio out) — the multi-port test's inputs.
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
fn dc_manifest() -> Manifest {
    base("dc", vec![audio_port("out", "out", "mono")], vec![nparam("level", 0.0, -1.0, 1.0)])
}

// ------------------------------------------------------------------ build helpers

fn build(node: NodeId, m: Box<dyn Module>, manifest: Manifest, p: &[f32]) -> (NodeId, NodeBuild) {
    (node, NodeBuild { module: m, manifest: validated(manifest), params: params(p) })
}

fn render(ex: &mut Executor, master: NodeId, blocks: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; FRAMES];
    let mut all = Vec::with_capacity(blocks * FRAMES);
    for _ in 0..blocks {
        ex.render_block(master, &mut out).unwrap();
        all.extend_from_slice(&out);
    }
    all
}

// ------------------------------------------------------------------ cv carriage

#[test]
fn a_block_rate_cv_wire_carries_the_published_value() {
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    // src manifest: [level=cv0]; probe manifest: [mod=cv0, out=audio1] — the cv edge runs
    // manifest-port 0 → manifest-port 0, and the per-type index spaces meet in the executor.
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.6]),
            build(probe, Box::new(CvProbe), cv_probe_manifest("block", "unipolar"), &[]),
        ],
        cfg(1),
    )
    .unwrap();
    let out = render(&mut ex, probe, 1);
    assert!(out.iter().all(|&v| v == 0.6), "the block value rides the wire: {out:?}");
    assert_eq!(ex.node_cv_block(src, 0), Some(0.6), "and the control side can read the same cell");
}

#[test]
fn an_unconnected_optional_cv_input_is_an_explicit_signal_not_a_silent_zero() {
    let mut g = Graph::new();
    let probe = g.add_node(0);
    let mut ex = Executor::build(
        g,
        vec![build(probe, Box::new(CvProbe), cv_probe_manifest("block", "unipolar"), &[])],
        cfg(1),
    )
    .unwrap();
    let out = render(&mut ex, probe, 1);
    assert!(
        out.iter().all(|&v| v == -2.0),
        "the module saw CvIn::Unconnected — never 0.0-by-accident: {out:?}"
    );
}

#[test]
fn an_audio_rate_source_into_a_block_rate_input_reduces_by_the_receivers_declaration() {
    // G3, made arithmetic: the SAME source renders six different numbers through six declared
    // policies. The ramp is a 0..63 permutation scaled to unipolar, so the hand-computed values
    // below are exact f32s, not tolerances. This is why the policy is manifest data and never
    // code: first and last render differently, and a journal replay must reproduce the original.
    let cases: [(&str, f32); 6] = [
        ("last", 57.0 / 64.0), // (63·7) mod 64 = 57
        ("first", 0.0),        // (0·7) mod 64 = 0
        ("mean", 31.5 / 64.0), // a permutation of 0..63 averages 31.5
        ("min", 0.0),
        ("max", 63.0 / 64.0),  // (9·7) mod 64 = 63
        ("peak", 63.0 / 64.0), // all samples are non-negative here, so peak = max
    ];
    for (policy, expected) in cases {
        let mut g = Graph::new();
        let src = g.add_node(0);
        let probe = g.add_node(0);
        g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
        let mut ex = Executor::build(
            g,
            vec![
                build(src, Box::new(CvRamp), cv_ramp_manifest(), &[]),
                build(
                    probe,
                    Box::new(CvProbe),
                    cv_probe_manifest_with("block", Some(policy), None),
                    &[],
                ),
            ],
            cfg(1),
        )
        .unwrap();
        let out = render(&mut ex, probe, 1);
        assert!(
            out.iter().all(|&v| (v - expected).abs() < 1e-6),
            "cv_reduce = {policy}: expected {expected}, got {:?}",
            out.first()
        );
    }
    // The determinism argument in one line: two declared policies, two different renders.
    assert_ne!((57.0f32 / 64.0), 0.0, "first and last MUST render differently (ADR-007)");
}

#[test]
fn a_block_rate_source_into_an_audio_rate_input_expands_by_the_receivers_declaration() {
    // hold: every frame reads the block's value.
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.25]),
            build(
                probe,
                Box::new(CvProbe),
                cv_probe_manifest_with("audio", None, Some("hold")),
                &[],
            ),
        ],
        cfg(1),
    )
    .unwrap();
    let out = render(&mut ex, probe, 1);
    assert!(out.iter().all(|&v| v == 0.25), "hold repeats the value: {out:?}");

    // linear: ramp prev → cur, reaching cur at the NEXT block boundary: v[i] = prev + (cur−prev)·i/64.
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.25]),
            build(
                probe,
                Box::new(CvProbe),
                cv_probe_manifest_with("audio", None, Some("linear")),
                &[],
            ),
        ],
        cfg(1),
    )
    .unwrap();
    let b0 = render(&mut ex, probe, 1);
    assert_eq!(b0[0], 0.0, "block 0 ramps from the initial 0.0");
    assert!((b0[32] - 0.125).abs() < 1e-7, "halfway: {}", b0[32]);
    let b1 = render(&mut ex, probe, 1);
    assert!(
        b1.iter().all(|&v| v == 0.25),
        "block 1: prev == cur, the ramp is flat at the value — continuous across the boundary"
    );
}

#[test]
fn an_audio_rate_cv_wire_copies_sample_for_sample() {
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(src, Box::new(CvRamp), cv_ramp_manifest(), &[]),
            build(
                probe,
                Box::new(CvProbe),
                cv_probe_manifest_with("audio", None, Some("hold")),
                &[],
            ),
        ],
        cfg(1),
    )
    .unwrap();
    let out = render(&mut ex, probe, 1);
    for (f, &v) in out.iter().enumerate() {
        let expected = ((f * 7) % FRAMES) as f32 / FRAMES as f32;
        assert_eq!(v, expected, "frame {f} of the audio-rate wire");
    }
    let tap = ex.node_cv_audio(src, 0).unwrap();
    assert_eq!(tap.len(), FRAMES, "the control-side reader sees the same buffer");
}

#[test]
fn a_receiver_declaring_spline_gets_the_declared_curve() {
    // WO-008 increment 6: the host PERFORMS `spline` — the build refusal this spot pinned
    // retired with the promise kept (faking it with a hold was never an option: that is the
    // invisible transformation ADR-005 exists to prevent). The curve is the parabola through
    // the last three block values, clamped to the wire's declared range; the full proof burden
    // (hand-computed frames, the linear degeneracy, both clamps, startup, determinism, zero
    // allocations) lives in tests/cv_spline.rs. What is pinned HERE is the contract-level fact:
    // the declared spelling builds, and the frames are the declared curve, not a silent hold.
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.25]),
            build(
                probe,
                Box::new(CvProbe),
                cv_probe_manifest_with("audio", None, Some("spline")),
                &[],
            ),
        ],
        cfg(1),
    )
    .unwrap();
    // Block 0 expands the declared zero history: the parabola through (0, 0, 0.25) is
    // v(t) = 0.25·t(t+1)/2 — a ride from 0, where a hold would sit flat at 0.25 from frame 0.
    // t = 32/64: v = 0.25·(0.5·1.5)/2 = 0.09375, exact dyadic, so `==` is the honest compare.
    let b0 = render(&mut ex, probe, 1);
    assert_eq!(b0[0], 0.0, "starts at the zero history, not at the knot");
    assert_eq!(b0[32], 0.09375, "the curve, hand-computed — a hold would read 0.25 here");
    // Block 1's knots are (0, 0.25, 0.25): v(0) = prev exactly — the arrival contract.
    let b1 = render(&mut ex, probe, 1);
    assert_eq!(b1[0], 0.25, "block 1 starts at block 0's knot, exactly");
}

#[test]
fn a_cv_range_mismatch_is_refused_naming_the_converter_and_its_phase() {
    // G2: unipolar → bipolar is REFUSED, never silently rescaled. The matrix names util/range;
    // at Phase 0 it does not exist in this build, and the refusal says so rather than offering
    // a one-tap fix that does nothing.
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let err = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.6]),
            build(probe, Box::new(CvProbe), cv_probe_manifest("block", "bipolar"), &[]),
        ],
        cfg(1),
    )
    .unwrap_err();
    assert!(matches!(err, ExecError::EdgeRefused { .. }), "got {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("util/range"), "names the converter: {msg}");
    assert!(msg.contains("unipolar cv → bipolar cv"), "names the mismatch: {msg}");
}

#[test]
fn cv_fan_in_is_refused_naming_the_merger_and_fan_out_is_free() {
    // Fan-in: no implicit summing — the merge is a module (util/mixer), not a host behaviour.
    let mut g = Graph::new();
    let a = g.add_node(0);
    let b = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(a, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(b, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let err = Executor::build(
        g,
        vec![
            build(a, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.6]),
            build(b, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.3]),
            build(probe, Box::new(CvProbe), cv_probe_manifest("block", "unipolar"), &[]),
        ],
        cfg(1),
    )
    .unwrap_err();
    assert!(matches!(err, ExecError::EdgeRefused { .. }), "got {err:?}");
    assert!(err.to_string().contains("util/mixer"), "names the merge module: {err}");

    // Fan-out: one output → many inputs is free (the universal modulator broadcasts).
    let mut g = Graph::new();
    let src = g.add_node(0);
    let p1 = g.add_node(0);
    let p2 = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(p1, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(src, 0), PortRef::new(p2, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.6]),
            build(p1, Box::new(CvProbe), cv_probe_manifest("block", "unipolar"), &[]),
            build(p2, Box::new(CvProbe), cv_probe_manifest("block", "unipolar"), &[]),
        ],
        cfg(1),
    )
    .unwrap();
    let mut out = vec![0.0f32; FRAMES];
    ex.render_block(p1, &mut out).unwrap();
    assert!(out.iter().all(|&v| v == 0.6), "consumer 1 hears it");
    ex.render_block(p2, &mut out).unwrap();
    assert!(out.iter().all(|&v| v == 0.6), "consumer 2 hears the same broadcast");
}

// ------------------------------------------------------------------ event carriage

#[test]
fn events_from_two_sources_merge_presorted_with_the_matrix_tie_break() {
    // Emitter A: channel 1, samples [4, 8] in order. Emitter B: channel 2, samples [8, 2] —
    // pushed OUT of order, so B's sink exercises the stable per-sink sort. The merged delivery
    // must read: (2, ch2), (4, ch1), (8, ch1), (8, ch2) — ties by connection rank (A's edge was
    // connected first), insertion order within a source. G5, mechanically.
    let mut g = Graph::new();
    let a = g.add_node(0);
    let b = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(a, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(b, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            // count 2, first 4, channel 1, step 4 → samples [4, 8]
            build(a, Box::new(EvEmitter), ev_emitter_manifest(), &[2.0, 4.0, 1.0, 4.0]),
            // count 2, first 8, channel 2, step −6 → pushes [8, 2] out of order
            build(b, Box::new(EvEmitter), ev_emitter_manifest(), &[2.0, 8.0, 2.0, -6.0]),
            build(probe, Box::new(EvProbe), ev_probe_manifest(&["trigger"]), &[]),
        ],
        cfg(1),
    )
    .unwrap();
    let out = render(&mut ex, probe, 1);
    let got = &out[..4];
    let want = [
        2.0 + 2.0 / 1000.0, // ch2 @ sample 2
        1.0 + 4.0 / 1000.0, // ch1 @ sample 4
        1.0 + 8.0 / 1000.0, // ch1 @ sample 8 — the tie, won by connection rank
        2.0 + 8.0 / 1000.0, // ch2 @ sample 8
    ];
    for (i, (g_, w)) in got.iter().zip(want).enumerate() {
        assert!(
            (g_ - w).abs() < 1e-6,
            "slot {i}: got {g_}, want {w} — merge order is the contract"
        );
    }
    // The producer side is readable too (sorted, as published).
    let emitted = ex.node_events(a, 0).unwrap();
    assert_eq!(emitted.len(), 2);
    assert_eq!(emitted[0].sample, 4);
    let emitted_b = ex.node_events(b, 0).unwrap();
    assert_eq!(
        (emitted_b[0].sample, emitted_b[1].sample),
        (2, 8),
        "B's sink was sorted by the host after the module pushed [8, 2]"
    );
}

#[test]
fn host_events_land_in_exactly_one_block_at_rank_zero() {
    let mut g = Graph::new();
    let probe = g.add_node(0);
    let mut ex = Executor::build(
        g,
        vec![build(probe, Box::new(EvProbe), ev_probe_manifest(&["trigger", "gate"]), &[])],
        cfg(1),
    )
    .unwrap();
    // Two events at the SAME sample: the queue keeps push order (stable), so ch9 precedes ch8.
    // One event past the frame count: delivered unchanged — the host guarantees order, not the
    // honesty of a producer's offsets, and a silent clamp would be an invisible transformation.
    ex.push_host_event(probe, 0, Event { channel: 9, ..Event::trigger(5, 0.5) }).unwrap();
    ex.push_host_event(probe, 0, Event { channel: 8, ..Event::gate(5, 1.0) }).unwrap();
    ex.push_host_event(probe, 0, Event::trigger(70, 0.25)).unwrap();
    let out = render(&mut ex, probe, 1);
    let want = [9.0 + 5.0 / 1000.0, 8.0 + 5.0 / 1000.0, 0.0 + 70.0 / 1000.0];
    for (i, w) in want.iter().enumerate() {
        assert!((out[i] - w).abs() < 1e-6, "slot {i}: got {}, want {w}", out[i]);
    }
    // The next block: the queue was consumed, not latched — sample-accurate means exactly once.
    let out2 = render(&mut ex, probe, 1);
    assert_eq!(out2[0], -1.0, "an empty block is an explicit empty list");
    // A non-event port and an unknown node are refused in words, control-side.
    assert!(ex.push_host_event(NodeId(999), 0, Event::clock(0)).is_err());
    let err = ex.push_host_event(probe, 1, Event::clock(0)).unwrap_err();
    assert!(err.to_string().contains("event input"), "{err}");
}

#[test]
fn event_overflow_is_counted_never_grown_never_silent() {
    // A lone emitter is enough: the sink's wall is per block, not per edge.
    let mut g = Graph::new();
    let src = g.add_node(0);
    let mut ex = Executor::build(
        g,
        vec![build(
            src,
            Box::new(EvEmitter),
            ev_emitter_manifest(),
            &[(EVENTS_PER_BLOCK + 6) as f32, 0.0, 3.0, 1.0],
        )],
        cfg(1),
    )
    .unwrap();
    let mut out = vec![0.0f32; FRAMES];
    ex.render_block(src, &mut out).unwrap(); // no audio out: the master render is silence, Ok
    let events = ex.node_events(src, 0).unwrap();
    assert_eq!(events.len(), EVENTS_PER_BLOCK, "the sink is a wall at capacity");
    assert_eq!(ex.event_drops_total(src, 0), Some(6), "and every refusal is counted");
    ex.render_block(src, &mut out).unwrap();
    assert_eq!(
        ex.event_drops_total(src, 0),
        Some(12),
        "the lifetime total accumulates; the per-block count reset with the block"
    );
}

#[test]
fn an_event_dialect_the_consumer_does_not_accept_is_refused() {
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let err = Executor::build(
        g,
        vec![
            build(src, Box::new(EvEmitter), ev_emitter_manifest(), &[0.0, 0.0, 0.0, 1.0]),
            // The probe accepts only `note` — the emitter speaks `trigger`.
            build(probe, Box::new(EvProbe), ev_probe_manifest(&["note"]), &[]),
        ],
        cfg(1),
    )
    .unwrap_err();
    assert!(matches!(err, ExecError::EdgeRefused { .. }), "got {err:?}");
    let msg = err.to_string();
    assert!(msg.contains("trigger") && msg.contains("note"), "names both dialect sets: {msg}");
    assert!(msg.contains("E-EVENTKIND-UNACCEPTED"), "cites the matrix rule: {msg}");
}

#[test]
fn a_delay_edge_on_a_cv_wire_is_refused_in_words() {
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::BlockDelay).unwrap();
    let err = Executor::build(
        g,
        vec![
            build(src, Box::new(CvConst), cv_const_manifest("unipolar"), &[0.6]),
            build(probe, Box::new(CvProbe), cv_probe_manifest("block", "unipolar"), &[]),
        ],
        cfg(1),
    )
    .unwrap_err();
    assert!(matches!(err, ExecError::EdgeRefused { .. }), "got {err:?}");
    assert!(err.to_string().contains("plain"), "{err}");
}

// ------------------------------------------------------------------ multi-port audio

#[test]
fn multiport_audio_buffers_stay_independent_and_in_manifest_order() {
    let mut g = Graph::new();
    let dc_a = g.add_node(0);
    let dc_b = g.add_node(0);
    let m = g.add_node(0);
    // dc_a → in-a (manifest port 0), dc_b → in-b (manifest port 1)
    g.connect(PortRef::new(dc_a, 0), PortRef::new(m, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(dc_b, 0), PortRef::new(m, 1), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(dc_a, Box::new(Dc), dc_manifest(), &[0.25]),
            build(dc_b, Box::new(Dc), dc_manifest(), &[0.1]),
            build(m, Box::new(MultiProc), multi_proc_manifest(), &[]),
        ],
        cfg(1),
    )
    .unwrap();
    // The master renders the FIRST audio out (sum = 0.35); the second (diff = 0.15) is read
    // through the per-port accessor — manifest port 3 is audio-out index 1.
    let out = render(&mut ex, m, 1);
    assert!(out.iter().all(|&v| (v - 0.35).abs() < 1e-6), "sum port: {out:?}");
    let diff = ex.node_audio_out(m, 3).unwrap();
    assert!(diff.iter().all(|&v| (v - 0.15).abs() < 1e-6), "diff port, independently buffered");
}

// ------------------------------------------------------------------ the acceptance item

#[test]
fn rms_modulates_the_filter_cutoff_bit_identically_to_a_hand_driven_reference() {
    // WO-014 acceptance: "ana/rms output demonstrably modulates a filter cutoff (the
    // analysis-as-control-source principle proven end to end)."
    //
    // The proof is arithmetic, not adjectival. Two executors run the same patch; in A the cv
    // wire is live, in B it is absent and the filter's cutoff PARAMETER is set by hand, per
    // block, to exactly the value the contract says the wire must carry:
    //     cutoff_eff = (200 · 2^(2 · mod · cv)) as f32,  mod = 1, cv = the rms cell.
    // If every block of A is BIT-IDENTICAL to B, then the wire delivered exactly the declared
    // value into exactly the declared formula — end to end, through the registry-built modules.
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();

    let mut a = mod_demo_patch(&reg, cfg(2)).unwrap();

    // B: the same graph minus the cv edge, svf driven by hand (mod = 0).
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let gain = g.add_node(0);
    let rms = g.add_node(0);
    let svf = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(gain, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(gain, 1), PortRef::new(svf, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(gain, 1), PortRef::new(rms, 0), EdgeKind::Plain).unwrap();
    // NOTE: no rms.level → svf edge. The modulation arrives through set_params instead.
    let mut builds = Vec::new();
    for (id, node, p) in [
        ("sparq/syn/sine", sine, &[440.0f32, 0.5][..]),
        ("sparq/util/gain", gain, &[0.5][..]),
        ("sparq/ana/rms", rms, &[0.0][..]),
        ("sparq/flt/svf", svf, &[200.0f32, 0.2, 2.0, 0.0][..]),
    ] {
        let r = reg.get(id).unwrap();
        builds.push((
            node,
            NodeBuild {
                module: r.create(),
                manifest: r.manifest().clone(),
                params: ParamSet::new(1, p).unwrap(),
            },
        ));
    }
    let mut b = Executor::build(g, builds, cfg(2)).unwrap();

    let mut out_a = vec![0.0f32; FRAMES * 2];
    let mut out_b = vec![0.0f32; FRAMES * 2];
    let mut version = 2u64;
    let mut last_cell = f32::NAN;
    for block in 0..200 {
        a.executor.render_block(a.svf, &mut out_a).unwrap();
        // The cell the modulated filter JUST consumed (rms runs before svf in the order).
        let cell = a.executor.node_cv_block(a.rms, 1).unwrap();
        last_cell = cell;
        assert!(cell > 0.1 && cell < 0.25, "block {block}: the rms of a 0.25-amp sine, got {cell}");
        // The module's exact arithmetic, reproduced: f64 math, quantised to the parameter's f32.
        let cutoff = (200.0f64 * 2.0f64.powf(2.0 * 1.0 * f64::from(cell))) as f32;
        let cutoff = cutoff.clamp(10.0, 20_000.0);
        assert!(cutoff > 200.0, "the modulation opens the cutoff: {cutoff}");
        version += 1;
        b.set_params(svf, ParamSet::new(version, &[cutoff, 0.2, 2.0, 0.0]).unwrap()).unwrap();
        b.render_block(svf, &mut out_b).unwrap();
        assert_eq!(
            out_a, out_b,
            "block {block}: the cv-wired filter and the hand-driven reference diverged — the \
             wire is not carrying exactly what the contract declares"
        );
    }
    // And the wire is not a no-op: the modulated render differs from the UNMODULATED one.
    // Both sides start FRESH — comparing against the already-200-blocks-old `a` would prove
    // nothing, because its sine phase and filter state have moved on.
    let mut modded = mod_demo_patch(&reg, cfg(2)).unwrap();
    let mut unmod = mod_demo_patch(&reg, cfg(2)).unwrap();
    unmod
        .executor
        .set_params(unmod.svf, ParamSet::new(9, &[200.0, 0.2, 2.0, 0.0]).unwrap())
        .unwrap();
    let mut out_u = vec![0.0f32; FRAMES * 2];
    let mut out_m = vec![0.0f32; FRAMES * 2];
    let mut differ = false;
    for _ in 0..200 {
        unmod.executor.render_block(unmod.svf, &mut out_u).unwrap();
        modded.executor.render_block(modded.svf, &mut out_m).unwrap();
        if out_u != out_m {
            differ = true;
        }
    }
    assert!(differ, "a modulation wire that changes nothing is a lie; this one changes the sound");
    assert!(!last_cell.is_nan());
}

#[test]
fn the_mod_demo_render_is_golden() {
    // The audible artefact `sparq exec --patch mod-demo` writes, pinned: 1 s at 48 kHz/64/stereo.
    // Regeneration is explicit and reviewed (ADR-007) — never to make a test pass.
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let mut demo = mod_demo_patch(&reg, cfg(2)).unwrap();
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut samples: Vec<f32> = Vec::new();
    for _ in 0..750 {
        demo.executor.render_block(demo.svf, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    let hash = hex64(fnv1a64_f32(&samples));
    assert_eq!(hash, MOD_DEMO_GOLDEN_HASH, "mod-demo golden changed — was this deliberate?");
    // Two identical builds render identically (ADR-007, the cheap half).
    let mut demo2 = mod_demo_patch(&reg, cfg(2)).unwrap();
    let mut samples2: Vec<f32> = Vec::new();
    for _ in 0..750 {
        demo2.executor.render_block(demo2.svf, &mut out).unwrap();
        samples2.extend_from_slice(&out);
    }
    assert_eq!(hex64(fnv1a64_f32(&samples2)), hash, "the mod-demo is not deterministic");
}
/// The mod-demo golden: 1 s (750 blocks) at 48 kHz/64/stereo, first measured 2026-09-26 in the
/// contract-v1 increment (debug and release agree — the determinism half of this test proves the
/// rebuild identity; `just gates`' release golden pass proves the profile identity).
const MOD_DEMO_GOLDEN_HASH: &str = "1621e1f65b1b64e1";

// ------------------------------------------------------------------ allocation discipline

#[test]
fn cv_and_event_wires_render_allocation_free() {
    // The v1 wire pass — cv reduce/interp staging, event merge, sink sorting, host-queue
    // consumption — runs entirely in build-time storage. Measured, like every RT claim here.
    let mut g = Graph::new();
    let cv_src = g.add_node(0);
    let cv_probe = g.add_node(0);
    let ev_src = g.add_node(0);
    let ev_probe = g.add_node(0);
    g.connect(PortRef::new(cv_src, 0), PortRef::new(cv_probe, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(ev_src, 0), PortRef::new(ev_probe, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            build(cv_src, Box::new(CvRamp), cv_ramp_manifest(), &[]),
            build(
                cv_probe,
                Box::new(CvProbe),
                cv_probe_manifest_with("block", Some("mean"), None),
                &[],
            ),
            build(ev_src, Box::new(EvEmitter), ev_emitter_manifest(), &[8.0, 0.0, 1.0, 7.0]),
            build(ev_probe, Box::new(EvProbe), ev_probe_manifest(&["trigger"]), &[]),
        ],
        cfg(1),
    )
    .unwrap();
    // The control-side door, exercised inside the measured loop's warm-up: pushes are inside
    // the reserved capacity, so even the queue path does not allocate.
    for i in 0..8 {
        ex.push_host_event(ev_probe, 0, Event::trigger(i * 8, 0.5)).unwrap();
    }
    let mut out = vec![0.0f32; FRAMES];
    for _ in 0..50 {
        ex.render_block(ev_probe, &mut out).unwrap();
        ex.render_block(cv_probe, &mut out).unwrap();
    }
    let base = start_counting();
    for b in 0..1_000 {
        ex.render_block(ev_probe, &mut out).unwrap();
        ex.render_block(cv_probe, &mut out).unwrap();
        if b % 100 == 0 {
            // A control-side push between blocks — the queue never exceeds its reservation.
            let _ = ex.push_host_event(ev_probe, 0, Event::trigger(3, 0.5));
        }
    }
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    assert_eq!(made, 0, "the contract-v1 wire path allocated {made} time(s) across 2000 blocks");
}
