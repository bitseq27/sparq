//! WO-008 executor tests, measured. This binary installs the kernel's counting allocator as its
//! **global allocator**, so the real-time claim is proven rather than asserted: if the executor
//! allocates once inside `render_block`, `process_makes_zero_allocations` fails. Same pattern as
//! `rt_discipline.rs` and `contract.rs`.
//!
//! The modules below are deliberately trivial (DC source, gain, passthrough, a failure) — the
//! subject under test is the *executor*: ordering, wiring, channel conversion, feedback, status
//! handling, determinism, and the allocation gate.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, ExecError, Executor, LatencyMode, NodeBuild, Watchdog};
use sparq_audio::hash::fnv1a64_f32;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::manifest::{
    Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;

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

// ------------------------------------------------------------------ test modules

/// A constant DC source: output = param(0) on every sample. Mono out.
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

/// Gain: out = in * param(0). Mono or stereo (channel-agnostic). Silences when unconnected.
struct Gain;
impl Module for Gain {
    fn id(&self) -> &str {
        "sparq/test/gain"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let g = ctx.param(0);
        let input = ctx.input();
        for (o, i) in ctx.output().iter_mut().zip(input.iter()) {
            *o = *i * g;
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("gain takes no messages"))
    }
}

/// Passthrough: out = in. Silences when unconnected.
struct Thru;
impl Module for Thru {
    fn id(&self) -> &str {
        "sparq/test/thru"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let input = ctx.input();
        ctx.output().copy_from_slice(input);
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("thru takes no messages"))
    }
}

/// Always fails — the executor must silence its output and flag it, never unwind.
struct Boom;
impl Module for Boom {
    fn id(&self) -> &str {
        "sparq/test/boom"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        for s in ctx.output().iter_mut() {
            *s = f32::NAN; // a module that misbehaves; the executor must overwrite this
        }
        BlockStatus::Failed
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("boom takes no messages"))
    }
}

// ------------------------------------------------------------------ manifest builders

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

fn base(id: &str, ports: Vec<PortSpec>, params: Vec<ParamSpec>) -> Manifest {
    let mut m = Manifest {
        identity: Identity {
            id: Some(format!("sparq/test/{id}")),
            version: Some("0.1.0".into()),
            host_api_min: Some(1),
            host_api_max: Some(1),
            display_name: Some(id.into()),
            summary: Some("test module".into()),
            authors: vec!["sparq".into()],
            license: Some("MIT".into()),
        },
        classification: Classification {
            category: Some(format!("utility/{id}")),
            top: Some("util".into()),
            kind: Some("processor".into()),
            tier: Some("t1".into()),
            stability: Some("stable".into()),
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

fn fparam(id: &str, def: f64) -> ParamSpec {
    ParamSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        kind: Some("float".into()),
        unit: Some("ratio".into()),
        min: Some(-2.0),
        max: Some(2.0),
        default: Some(def),
        ..ParamSpec::default()
    }
}

fn dc_manifest() -> Manifest {
    base("dc", vec![audio_port("out", "out", "mono")], vec![fparam("level", 0.0)])
}
fn gain_manifest(set: &str) -> Manifest {
    base(
        "gain",
        vec![audio_port("in", "in", set), audio_port("out", "out", set)],
        vec![fparam("gain", 1.0)],
    )
}
fn thru_manifest(set: &str) -> Manifest {
    base("thru", vec![audio_port("in", "in", set), audio_port("out", "out", set)], vec![])
}
fn boom_manifest() -> Manifest {
    base("boom", vec![audio_port("in", "in", "mono"), audio_port("out", "out", "mono")], vec![])
}

fn validated(m: Manifest) -> sparq_module_api::ValidatedManifest {
    m.validate().unwrap_or_else(|r| panic!("manifest should validate: {r}"))
}

fn params(values: &[f32]) -> ParamSet {
    ParamSet::new(0, values).unwrap()
}

// ------------------------------------------------------------------ helpers

fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

/// dc(level) → gain(g), both mono, master = gain. Returns the executor after one rendered block.
fn dc_gain_chain(level: f32, g: f32) -> (Executor, NodeId) {
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let gn = graph.add_node(0);
    // dc.out = port 0; gain.in = port 0, gain.out = port 1
    graph.connect(PortRef::new(dc, 0), PortRef::new(gn, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[level]),
            },
        ),
        (
            gn,
            NodeBuild {
                module: Box::new(Gain),
                manifest: validated(gain_manifest("mono")),
                params: params(&[g]),
            },
        ),
    ];
    (Executor::build(graph, builds, cfg(1)).unwrap(), gn)
}

// ------------------------------------------------------------------ tests

#[test]
fn a_dc_to_gain_chain_renders_the_product() {
    let (mut ex, master) = dc_gain_chain(0.5, 0.25);
    let mut out = vec![0.0f32; FRAMES];
    ex.render_block(master, &mut out).unwrap();
    assert!(
        out.iter().all(|&s| (s - 0.125).abs() < 1e-6),
        "0.5 × 0.25 = 0.125, got {:?}",
        &out[..4]
    );
    assert_eq!(ex.blocks_rendered(), 1);
}

#[test]
fn process_makes_zero_allocations() {
    // The WO-008 headline, measured: build (allocates freely), then render under the counter.
    let (mut ex, master) = dc_gain_chain(0.5, 0.5);
    let mut out = vec![0.0f32; FRAMES];
    let made = measure(|| {
        for _ in 0..1000 {
            ex.render_block(master, &mut out).unwrap();
        }
    });
    assert_eq!(made, 0, "the audio path allocated {made} time(s) across 1000 blocks");
}

#[test]
fn a_failed_module_is_silenced_and_flagged_not_unwound() {
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let boom = graph.add_node(0);
    graph.connect(PortRef::new(dc, 0), PortRef::new(boom, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[1.0]),
            },
        ),
        (
            boom,
            NodeBuild {
                module: Box::new(Boom),
                manifest: validated(boom_manifest()),
                params: params(&[]),
            },
        ),
    ];
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    let mut out = vec![9.0f32; FRAMES];
    ex.render_block(boom, &mut out).unwrap();
    assert!(out.iter().all(|&s| s == 0.0), "a Failed module's output is silence, never NaN");
    assert_eq!(ex.failed_blocks(boom), Some(1));
    assert_eq!(ex.meter(boom).map(|m| m.status), Some(BlockStatus::Failed));
}

#[test]
fn an_unconnected_input_is_silenced_and_reported() {
    // A gain with no input edge: the module sees has_input()==false and declares Silenced.
    let mut graph = Graph::new();
    let gn = graph.add_node(0);
    let builds = vec![(
        gn,
        NodeBuild {
            module: Box::new(Gain),
            manifest: validated(gain_manifest("mono")),
            params: params(&[1.0]),
        },
    )];
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    let mut out = vec![9.0f32; FRAMES];
    ex.render_block(gn, &mut out).unwrap();
    assert!(out.iter().all(|&s| s == 0.0));
    assert_eq!(ex.meter(gn).map(|m| m.status), Some(BlockStatus::Silenced));
}

#[test]
fn mono_to_stereo_fans_out_and_the_master_converts_to_the_device() {
    // dc(mono) → gain(stereo): the mono source replicates across the stereo input; the master
    // renders stereo into a 2-channel device buffer.
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let gn = graph.add_node(0);
    graph.connect(PortRef::new(dc, 0), PortRef::new(gn, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[0.5]),
            },
        ),
        (
            gn,
            NodeBuild {
                module: Box::new(Gain),
                manifest: validated(gain_manifest("stereo")),
                params: params(&[1.0]),
            },
        ),
    ];
    let mut ex = Executor::build(graph, builds, cfg(2)).unwrap();
    let mut out = vec![0.0f32; FRAMES * 2];
    ex.render_block(gn, &mut out).unwrap();
    assert!(out.iter().all(|&s| (s - 0.5).abs() < 1e-6), "both channels carry the mono source");
}

#[test]
fn a_block_delay_self_loop_accumulates_across_blocks() {
    // gain(0.5) with dc(1.0) in AND its own block-delayed output summed back in:
    //   block 0: in = 1.0 + 0        → out 0.5
    //   block 1: in = 1.0 + 0.5      → out 0.75
    //   block 2: in = 1.0 + 0.75     → out 0.875   (converging to 1.0)
    // This is the legal feedback §5.4 allows, and it only works because the delay reads the
    // PREVIOUS block.
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let gn = graph.add_node(0);
    graph.connect(PortRef::new(dc, 0), PortRef::new(gn, 0), EdgeKind::Plain).unwrap();
    graph.connect(PortRef::new(gn, 1), PortRef::new(gn, 0), EdgeKind::BlockDelay).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[1.0]),
            },
        ),
        (
            gn,
            NodeBuild {
                module: Box::new(Gain),
                manifest: validated(gain_manifest("mono")),
                params: params(&[0.5]),
            },
        ),
    ];
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    let first = |ex: &mut Executor, out: &mut [f32]| {
        ex.render_block(gn, out).unwrap();
        out[0]
    };
    let b0 = first(&mut ex, &mut out);
    let b1 = first(&mut ex, &mut out);
    let b2 = first(&mut ex, &mut out);
    assert!((b0 - 0.5).abs() < 1e-6, "block 0: {b0}");
    assert!((b1 - 0.75).abs() < 1e-6, "block 1: {b1}");
    assert!((b2 - 0.875).abs() < 1e-6, "block 2: {b2}");
}

#[test]
fn a_unit_delay_edge_shifts_by_one_sample() {
    // dc(1.0) → unit-delay → thru. Block 0: frame 0 sees the held sample (0), frames 1.. see the
    // source shifted by one; every later block is all ones once the held sample is 1.0.
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let th = graph.add_node(0);
    graph.connect(PortRef::new(dc, 0), PortRef::new(th, 0), EdgeKind::UnitDelay).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[1.0]),
            },
        ),
        (
            th,
            NodeBuild {
                module: Box::new(Thru),
                manifest: validated(thru_manifest("mono")),
                params: params(&[]),
            },
        ),
    ];
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    let mut out = vec![9.0f32; FRAMES];
    ex.render_block(th, &mut out).unwrap();
    assert_eq!(out[0], 0.0, "frame 0 is the pre-roll held sample");
    assert!(out[1..].iter().all(|&s| s == 1.0), "frames 1.. see the source, shifted by one");
    ex.render_block(th, &mut out).unwrap();
    assert!(
        out.iter().all(|&s| s == 1.0),
        "block 1: the held sample is now the source's last frame"
    );
}

#[test]
fn a_cross_type_edge_is_refused_in_words_not_ignored() {
    // The v0 shape of this test was "a cv edge is refused because the payload cannot travel".
    // Contract v1 CARRIES cv→cv, so the honest refusal left in this graph is the cross-type one:
    // a.port1 is a cv OUTPUT wired into b.port0, an AUDIO input — same-type-required (ADR-005),
    // which the connect layer refuses upstream and the executor refuses again rather than guess.
    let mut cv_manifest =
        base("dc", vec![audio_port("out", "out", "mono")], vec![fparam("level", 0.0)]);
    cv_manifest.ports.push(PortSpec {
        id: Some("level".into()),
        name: Some("Level".into()),
        direction: Some("out".into()),
        port_type: Some("cv".into()),
        rate: Some("block".into()),
        range: Some("unipolar".into()),
        ..PortSpec::default()
    });
    let mut graph = Graph::new();
    let a = graph.add_node(0);
    let b = graph.add_node(0);
    graph.connect(PortRef::new(a, 1), PortRef::new(b, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            a,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(cv_manifest),
                params: params(&[]),
            },
        ),
        (
            b,
            NodeBuild {
                module: Box::new(Gain),
                manifest: validated(gain_manifest("stereo")),
                params: params(&[1.0]),
            },
        ),
    ];
    let err = Executor::build(graph, builds, cfg(2)).unwrap_err();
    assert!(matches!(err, ExecError::EdgeRefused { .. }), "got {err:?}");
    assert!(err.to_string().contains("crosses port types"), "{err}");
}

#[test]
fn data_gpu_and_atom_edges_are_still_refused_each_with_its_own_sentence() {
    // Contract v1 carries audio, cv and event. The other three types keep the v0 promise — a
    // drawn wire is never a silent no-op — but each refusal now says what WOULD carry it, so the
    // operator gets a remedy instead of a dead end.
    fn typed_port(id: &str, dir: &str, ty: &str) -> PortSpec {
        PortSpec {
            id: Some(id.into()),
            name: Some(id.to_uppercase()),
            direction: Some(dir.into()),
            port_type: Some(ty.into()),
            required: Some(false),
            ..PortSpec::default()
        }
    }
    for (ty, expect) in [("data", "schema"), ("gpu", "audio thread"), ("atom", "control-thread")] {
        let mut src_manifest = base("dc", vec![audio_port("out", "out", "mono")], vec![]);
        src_manifest.ports.push(typed_port("side-out", "out", ty));
        let mut dst_manifest = base(
            "thru",
            vec![audio_port("in", "in", "mono"), audio_port("out", "out", "mono")],
            vec![],
        );
        dst_manifest.ports.push(typed_port("side-in", "in", ty));
        let mut graph = Graph::new();
        let a = graph.add_node(0);
        let b = graph.add_node(0);
        graph.connect(PortRef::new(a, 1), PortRef::new(b, 2), EdgeKind::Plain).unwrap();
        let builds = vec![
            (
                a,
                NodeBuild {
                    module: Box::new(Dc),
                    manifest: validated(src_manifest),
                    params: params(&[1.0]),
                },
            ),
            (
                b,
                NodeBuild {
                    module: Box::new(Thru),
                    manifest: validated(dst_manifest),
                    params: params(&[]),
                },
            ),
        ];
        let err = Executor::build(graph, builds, cfg(1)).unwrap_err();
        assert!(matches!(err, ExecError::PayloadNotCarried { .. }), "{ty}: got {err:?}");
        assert!(err.to_string().contains(ty), "the message names the payload: {err}");
        assert!(err.to_string().contains(expect), "and its remedy: {err}");
    }
}

#[test]
fn an_id_mismatch_is_refused() {
    let mut graph = Graph::new();
    let a = graph.add_node(0);
    // A Dc module with a Gain manifest: the identities disagree.
    let builds = vec![(
        a,
        NodeBuild {
            module: Box::new(Dc),
            manifest: validated(gain_manifest("mono")),
            params: params(&[]),
        },
    )];
    let err = Executor::build(graph, builds, cfg(1)).unwrap_err();
    assert!(matches!(err, ExecError::IdMismatch { .. }), "got {err:?}");
}

#[test]
fn a_cycle_without_a_delay_is_refused_by_the_graph() {
    let mut graph = Graph::new();
    let a = graph.add_node(0);
    let b = graph.add_node(0);
    graph.connect(PortRef::new(a, 1), PortRef::new(b, 0), EdgeKind::Plain).unwrap();
    let err = graph.connect(PortRef::new(b, 1), PortRef::new(a, 0), EdgeKind::Plain).unwrap_err();
    assert!(matches!(err, sparq_kernel::graph::GraphError::Cycle { .. }), "got {err:?}");
}

#[test]
fn two_identical_builds_render_bit_identical_output() {
    // Determinism (ADR-007): same graph, same params, same input → same bits. Build twice, render
    // 100 blocks each through a chain with feedback, hash the master outputs.
    fn run() -> u64 {
        let (mut ex, master) = {
            let mut graph = Graph::new();
            let dc = graph.add_node(0);
            let gn = graph.add_node(0);
            graph.connect(PortRef::new(dc, 0), PortRef::new(gn, 0), EdgeKind::Plain).unwrap();
            graph.connect(PortRef::new(gn, 1), PortRef::new(gn, 0), EdgeKind::BlockDelay).unwrap();
            let builds = vec![
                (
                    dc,
                    NodeBuild {
                        module: Box::new(Dc),
                        manifest: validated(dc_manifest()),
                        params: params(&[1.0]),
                    },
                ),
                (
                    gn,
                    NodeBuild {
                        module: Box::new(Gain),
                        manifest: validated(gain_manifest("mono")),
                        params: params(&[0.5]),
                    },
                ),
            ];
            (Executor::build(graph, builds, cfg(1)).unwrap(), gn)
        };
        let mut out = vec![0.0f32; FRAMES];
        let mut acc: Vec<f32> = Vec::new();
        for _ in 0..100 {
            ex.render_block(master, &mut out).unwrap();
            acc.extend_from_slice(&out);
        }
        fnv1a64_f32(&acc)
    }
    assert_eq!(run(), run(), "two identical builds diverged — determinism is broken");
}

#[test]
fn a_wide_graph_orders_and_renders() {
    // 200 passthrough nodes in a chain, dc at the head, master at the tail. Proves the order is
    // honoured at scale (the tail cannot render before the head fills it) and stays allocation-free.
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let mut chain = Vec::new();
    for _ in 0..200 {
        chain.push(graph.add_node(0));
    }
    let mut builds = vec![(
        dc,
        NodeBuild {
            module: Box::new(Dc),
            manifest: validated(dc_manifest()),
            params: params(&[0.75]),
        },
    )];
    graph.connect(PortRef::new(dc, 0), PortRef::new(chain[0], 0), EdgeKind::Plain).unwrap();
    builds.push((
        chain[0],
        NodeBuild {
            module: Box::new(Thru),
            manifest: validated(thru_manifest("mono")),
            params: params(&[]),
        },
    ));
    for w in chain.windows(2) {
        graph.connect(PortRef::new(w[0], 1), PortRef::new(w[1], 0), EdgeKind::Plain).unwrap();
        builds.push((
            w[1],
            NodeBuild {
                module: Box::new(Thru),
                manifest: validated(thru_manifest("mono")),
                params: params(&[]),
            },
        ));
    }
    let master = *chain.last().unwrap();
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    assert_eq!(ex.order().len(), 201);
    let mut out = vec![0.0f32; FRAMES];
    let made = measure(|| ex.render_block(master, &mut out).unwrap());
    assert_eq!(made, 0, "a 200-node block allocated {made} time(s)");
    assert!(out.iter().all(|&s| (s - 0.75).abs() < 1e-6), "the DC value survived 200 passthroughs");
}

#[test]
fn the_buffer_budget_is_reported_and_matches_the_buffers() {
    // dc(mono) → gain(stereo): out buffers dc=FRAMES*1, gain=FRAMES*2; in buffer gain=FRAMES*2.
    // Budget = (1 + 2 + 2) * FRAMES * 4 bytes.
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let gn = graph.add_node(0);
    graph.connect(PortRef::new(dc, 0), PortRef::new(gn, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[0.0]),
            },
        ),
        (
            gn,
            NodeBuild {
                module: Box::new(Gain),
                manifest: validated(gain_manifest("stereo")),
                params: params(&[1.0]),
            },
        ),
    ];
    let ex = Executor::build(graph, builds, cfg(2)).unwrap();
    let expected = (1 + 2 + 2) * FRAMES * std::mem::size_of::<f32>();
    assert_eq!(ex.memory_budget_bytes(), expected, "budget must equal the allocated buffers");
}

// ------------------------------------------------------------------ task 5+6 modules

/// Returns `Overrun` on every block and writes zeros — the deterministic stand-in for a module
/// that blew its budget. (Real timing detection belongs to the HAL pump, which owns the clock;
/// the executor watchdog acts on the status the infrastructure reports — the two halves are the
/// same watchdog, and the selftest's HAL gate proves the measuring half.)
struct Stall;
impl Module for Stall {
    fn id(&self) -> &str {
        "sparq/test/stall"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        for s in ctx.output().iter_mut() {
            *s = 0.0;
        }
        BlockStatus::Overrun
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("stall takes no messages"))
    }
}

/// One impulse: 1.0 at sample 0 of the first block, silence forever after — the reference
/// signal for the latency-alignment arithmetic.
struct Impulse {
    fired: bool,
}
impl Module for Impulse {
    fn id(&self) -> &str {
        "sparq/test/impulse"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        self.fired = false;
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        for (i, s) in ctx.output().iter_mut().enumerate() {
            *s = if !self.fired && i == 0 { 1.0 } else { 0.0 };
        }
        self.fired = true;
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("impulse takes no messages"))
    }
}

/// A true 128-sample delay (mono) — the module whose DECLARED latency matches what it does, so
/// the compensation arithmetic can be checked against physical reality, not just the claim.
struct Delay128 {
    hist: Vec<f32>,
    pos: usize,
}
impl Module for Delay128 {
    fn id(&self) -> &str {
        "sparq/test/delay128"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        self.hist = vec![0.0; 128]; // build-time allocation, per decision 4
        self.pos = 0;
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        if !ctx.has_input() {
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let input = ctx.input();
        for (o, &i) in ctx.output().iter_mut().zip(input.iter()) {
            *o = self.hist[self.pos];
            self.hist[self.pos] = i;
            self.pos = (self.pos + 1) % 128;
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("delay128 takes no messages"))
    }
}

fn stall_manifest() -> Manifest {
    base("stall", vec![audio_port("in", "in", "mono"), audio_port("out", "out", "mono")], vec![])
}
fn impulse_manifest() -> Manifest {
    base("impulse", vec![audio_port("out", "out", "mono")], vec![])
}
fn delay128_manifest() -> Manifest {
    base("delay128", vec![audio_port("in", "in", "mono"), audio_port("out", "out", "mono")], vec![])
}

// ------------------------------------------------------------------ task 6: the watchdog

#[test]
fn the_watchdog_bypasses_a_stalling_module_and_the_graph_keeps_playing() {
    // dc(1.0) → stall → thru(master): while the stall stalls, the master hears silence; after
    // N=3 consecutive overruns the watchdog isolates it — the module is never called again, its
    // output becomes passthrough, and the master hears the dc again. The rest plays on.
    let mut graph = Graph::new();
    let dc = graph.add_node(0);
    let st = graph.add_node(0);
    let th = graph.add_node(0);
    graph.connect(PortRef::new(dc, 0), PortRef::new(st, 0), EdgeKind::Plain).unwrap();
    graph.connect(PortRef::new(st, 1), PortRef::new(th, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            dc,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[1.0]),
            },
        ),
        (
            st,
            NodeBuild {
                module: Box::new(Stall),
                manifest: validated(stall_manifest()),
                params: params(&[]),
            },
        ),
        (
            th,
            NodeBuild {
                module: Box::new(Thru),
                manifest: validated(thru_manifest("mono")),
                params: params(&[]),
            },
        ),
    ];
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    let peak = |o: &[f32]| o.iter().fold(0.0f32, |a, &s| a.max(s.abs()));

    for b in 0..2 {
        ex.render_block(th, &mut out).unwrap();
        assert_eq!(peak(&out), 0.0, "block {b}: the stall silences the chain");
        assert_eq!(ex.auto_bypassed(st), Some(false));
    }
    ex.render_block(th, &mut out).unwrap(); // block 2: the third consecutive overrun
    assert_eq!(ex.auto_bypassed(st), Some(true), "bypassed within N blocks (N=3)");
    assert_eq!(ex.watchdog_events().len(), 1);
    assert_eq!(ex.watchdog_events()[0].node, st);
    assert_eq!(ex.watchdog_events()[0].block, 2, "journaled at the block it engaged");

    ex.render_block(th, &mut out).unwrap();
    assert_eq!(peak(&out), 1.0, "bypass is passthrough: the rest of the graph keeps playing");
    // The engaging block is still processed by the module (the verdict comes from ITS status);
    // the bypass takes effect from the NEXT block — so one rendered block has been bypassed.
    assert_eq!(ex.bypassed_blocks(st), Some(1));

    // clearing the bypass hands the module its stall back — and the watchdog re-arms
    assert!(ex.clear_auto_bypass(st));
    for _ in 0..3 {
        ex.render_block(th, &mut out).unwrap();
    }
    assert_eq!(ex.auto_bypassed(st), Some(true), "re-bypassed after N fresh consecutive overruns");
    assert_eq!(ex.watchdog_events().len(), 2, "both actions journaled");
    assert_eq!(ex.overrun_blocks(st), Some(6));
}

#[test]
fn a_disabled_watchdog_counts_but_never_acts() {
    let mut graph = Graph::new();
    let st = graph.add_node(0);
    let builds = vec![(
        st,
        NodeBuild {
            module: Box::new(Stall),
            manifest: validated(stall_manifest()),
            params: params(&[]),
        },
    )];
    let mut cfg_off = cfg(1);
    cfg_off.watchdog = Watchdog::disabled();
    let mut ex = Executor::build(graph, builds, cfg_off).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    for _ in 0..10 {
        ex.render_block(st, &mut out).unwrap();
    }
    assert_eq!(ex.overrun_blocks(st), Some(10), "counted…");
    assert_eq!(ex.auto_bypassed(st), Some(false), "…but never acted");
    assert!(ex.watchdog_events().is_empty());
}

#[test]
fn an_overrun_streak_resets_on_a_good_block() {
    // The criterion is CONSECUTIVE: Overrun, Overrun, Ok, Overrun, Overrun must not bypass at
    // N=3. Flaky is not stalled.
    struct Flaky(u32);
    impl Module for Flaky {
        fn id(&self) -> &str {
            // rides the stall manifest: the contract's id invariant means module and manifest
            // must agree, and this module IS the staller with one good block in it
            "sparq/test/stall"
        }
        fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
            Ok(())
        }
        fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
            Ok(())
        }
        fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
            self.0 += 1;
            for s in ctx.output().iter_mut() {
                *s = 0.0;
            }
            // blocks 1,2 over; 3 ok (streak resets); 4..8 over → bypass engages at 8
            if self.0 == 3 {
                BlockStatus::Ok
            } else {
                BlockStatus::Overrun
            }
        }
        fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
            Err(ModuleError::Message("flaky takes no messages"))
        }
    }
    let mut graph = Graph::new();
    let f = graph.add_node(0);
    let builds = vec![(
        f,
        NodeBuild {
            module: Box::new(Flaky(0)),
            manifest: validated(stall_manifest()),
            params: params(&[]),
        },
    )];
    let mut ex = Executor::build(graph, builds, cfg(1)).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    for b in 1..=5 {
        ex.render_block(f, &mut out).unwrap();
        assert_eq!(ex.auto_bypassed(f), Some(false), "block {b}: streak never reached 3");
    }
    for _ in 6..=8 {
        ex.render_block(f, &mut out).unwrap();
    }
    assert_eq!(ex.auto_bypassed(f), Some(true), "three CONSECUTIVE overruns engaged at block 8");
}

// ------------------------------------------------------------------ task 5: compensation

/// impulse → {Delay128 (declared 128) | Thru (declared 0)} → Thru(master), all mono.
/// Hand-computed: raw renders the impulse at the master in block 0 (fast arm) and block 2
/// (slow arm, 128 samples = exactly 2 blocks); compensated delays the FAST ARM's edge by 128,
/// so both contributions arrive in block 2 and SUM to 2.0 — the fan-in alignment, arithmetic
/// checked against a module that physically does what it declares.
fn compensation_graph(mode: LatencyMode) -> (Executor, NodeId) {
    let mut graph = Graph::new();
    let imp = graph.add_node(0);
    let slow = graph.add_node(128); // declares what Delay128 physically does
    let fast = graph.add_node(0);
    let m = graph.add_node(0);
    graph.connect(PortRef::new(imp, 0), PortRef::new(slow, 0), EdgeKind::Plain).unwrap();
    graph.connect(PortRef::new(imp, 0), PortRef::new(fast, 0), EdgeKind::Plain).unwrap();
    graph.connect(PortRef::new(slow, 1), PortRef::new(m, 0), EdgeKind::Plain).unwrap();
    graph.connect(PortRef::new(fast, 1), PortRef::new(m, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (
            imp,
            NodeBuild {
                module: Box::new(Impulse { fired: false }),
                manifest: validated(impulse_manifest()),
                params: params(&[]),
            },
        ),
        (
            slow,
            NodeBuild {
                module: Box::new(Delay128 { hist: Vec::new(), pos: 0 }),
                manifest: validated(delay128_manifest()),
                params: params(&[]),
            },
        ),
        (
            fast,
            NodeBuild {
                module: Box::new(Thru),
                manifest: validated(thru_manifest("mono")),
                params: params(&[]),
            },
        ),
        (
            m,
            NodeBuild {
                module: Box::new(Thru),
                manifest: validated(thru_manifest("mono")),
                params: params(&[]),
            },
        ),
    ];
    let mut c = cfg(1);
    c.latency = mode;
    (Executor::build(graph, builds, c).unwrap(), m)
}

fn block_peaks(ex: &mut Executor, m: NodeId, n: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; FRAMES];
    let mut peaks = Vec::with_capacity(n);
    for _ in 0..n {
        ex.render_block(m, &mut out).unwrap();
        peaks.push(out.iter().fold(0.0f32, |a, &s| a.max(s.abs())));
    }
    peaks
}

#[test]
fn raw_mode_renders_the_arms_when_they_arrive() {
    let (mut ex, m) = compensation_graph(LatencyMode::Raw);
    let peaks = block_peaks(&mut ex, m, 5);
    assert!((peaks[0] - 1.0).abs() < 1e-6, "fast arm, block 0: {peaks:?}");
    assert_eq!(peaks[1], 0.0);
    assert!((peaks[2] - 1.0).abs() < 1e-6, "slow arm, block 2 (128 = 2 blocks): {peaks:?}");
    assert_eq!(peaks[3], 0.0);
    // the readout is honest in raw mode too: the switch moves the signal, never the numbers
    assert_eq!(ex.max_path_latency().unwrap(), 128);
}

#[test]
fn compensated_mode_aligns_the_fan_in_exactly_as_hand_computed() {
    let (mut ex, m) = compensation_graph(LatencyMode::Compensated);
    let peaks = block_peaks(&mut ex, m, 5);
    assert_eq!(peaks[0], 0.0, "the fast arm is held back: {peaks:?}");
    assert_eq!(peaks[1], 0.0);
    assert!((peaks[2] - 2.0).abs() < 1e-6, "both arms land in block 2 and SUM: {peaks:?}");
    assert_eq!(peaks[3], 0.0);
    assert_eq!(ex.max_path_latency().unwrap(), 128);
    // the compensation budget is in the printed memory budget (decision 4)
    let (raw, _) = compensation_graph(LatencyMode::Raw);
    assert!(
        ex.memory_budget_bytes() > raw.memory_budget_bytes(),
        "the delay lines cost memory, and the budget says so"
    );
}

#[test]
fn compensation_of_an_untouched_chain_changes_nothing() {
    // Single-input nodes and chains get zero on every arm: compensated mode on a chain renders
    // byte-identical to raw — the switch aligns fan-ins, it does not re-time the world.
    let (mut raw_ex, raw_m) = dc_gain_chain(0.5, 0.5);
    let (mut graph2, builds2) = {
        let mut graph = Graph::new();
        let dc = graph.add_node(0);
        let gn = graph.add_node(0);
        graph.connect(PortRef::new(dc, 0), PortRef::new(gn, 0), EdgeKind::Plain).unwrap();
        (graph, (dc, gn))
    };
    let _ = &mut graph2;
    let builds = vec![
        (
            builds2.0,
            NodeBuild {
                module: Box::new(Dc),
                manifest: validated(dc_manifest()),
                params: params(&[0.5]),
            },
        ),
        (
            builds2.1,
            NodeBuild {
                module: Box::new(Gain),
                manifest: validated(gain_manifest("mono")),
                params: params(&[0.5]),
            },
        ),
    ];
    let mut c = cfg(1);
    c.latency = LatencyMode::Compensated;
    let mut comp_ex = Executor::build(graph2, builds, c).unwrap();
    let (mut a, mut b) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
    for _ in 0..10 {
        raw_ex.render_block(raw_m, &mut a).unwrap();
        comp_ex.render_block(builds2.1, &mut b).unwrap();
        assert_eq!(a, b, "a chain is byte-identical under the switch");
    }
}

// ------------------------------------------------------------------ WO-009: the musical-position door

/// Writes the block's musical position into its output — frame 0 carries `ctx.block.tick`,
/// frame 1 carries `ctx.block.ppqn` — so a test can read back exactly what the executor handed
/// every module for that block.
struct TickProbe;
impl Module for TickProbe {
    fn id(&self) -> &str {
        "sparq/test/tick-probe"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // Scalars first, output view last — the v1 borrow discipline the author guide teaches.
        let (tick, ppqn) = (ctx.block.tick as f32, ctx.block.ppqn as f32);
        let out = ctx.output();
        if let Some(s) = out.first_mut() {
            *s = tick;
        }
        if let Some(s) = out.get_mut(1) {
            *s = ppqn;
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("tick-probe takes no messages"))
    }
}

#[test]
fn the_musical_position_door_carries_what_the_transport_says_and_nothing_more() {
    // The executor performs no musical math (WO-009's placement rule): with no position set,
    // modules see the declared static clock (tick 0 — the shape every golden was built against);
    // with one set, every module sees exactly that value for the whole block; with None, the
    // last value freezes rather than snapping back to zero.
    let mut g = Graph::new();
    let probe = g.add_node(0);
    let mut ex = Executor::build(
        g,
        vec![(
            probe,
            NodeBuild {
                module: Box::new(TickProbe),
                manifest: validated(base(
                    "tick-probe",
                    vec![audio_port("out", "out", "mono")],
                    vec![],
                )),
                params: params(&[]),
            },
        )],
        cfg(1),
    )
    .unwrap();
    let mut out = vec![9.0f32; FRAMES];

    ex.render_block(probe, &mut out).unwrap();
    assert_eq!(out[0], 0.0, "no transport, no tick — the v1 default, pinned");

    ex.set_musical_position(Some((960, 480)));
    ex.render_block(probe, &mut out).unwrap();
    assert_eq!(out[0], 960.0, "the door carries the tick");
    assert_eq!(out[1], 480.0, "and the ppqn it belongs to");

    ex.set_musical_position(None);
    ex.render_block(probe, &mut out).unwrap();
    assert_eq!(out[0], 960.0, "None freezes the last position; it does not rewind to zero");
}
