//! WO-008 executor tests, measured. This binary installs the kernel's counting allocator as its
//! **global allocator**, so the real-time claim is proven rather than asserted: if the executor
//! allocates once inside `render_block`, `process_makes_zero_allocations` fails. Same pattern as
//! `rt_discipline.rs` and `contract.rs`.
//!
//! The modules below are deliberately trivial (DC source, gain, passthrough, a failure) — the
//! subject under test is the *executor*: ordering, wiring, channel conversion, feedback, status
//! handling, determinism, and the allocation gate.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, ExecError, Executor, NodeBuild};
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
    ExecConfig { sample_rate: RATE, block_frames: FRAMES, device_channels: ch }
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
        for s in ctx.output.iter_mut() {
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
            for s in ctx.output.iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        let g = ctx.param(0);
        for (o, i) in ctx.output.iter_mut().zip(ctx.input.iter()) {
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
            for s in ctx.output.iter_mut() {
                *s = 0.0;
            }
            return BlockStatus::Silenced;
        }
        ctx.output.copy_from_slice(ctx.input);
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
        for s in ctx.output.iter_mut() {
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
fn a_cv_edge_is_refused_at_build_in_words() {
    // A cv→cv edge has no payload path in v0; the executor must refuse the whole build rather
    // than silently ignore a wire. Reuse the real reference contract shape: an rms-style cv out.
    // The Dc module with an extra cv OUTPUT, so the identity check passes and the refusal we
    // provoke is the payload one (order of checks: identity first, edges second).
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
    // a.port1 is the cv out; b is a gain whose input is audio — a cv→audio edge, which the
    // matrix would refuse upstream; the executor refuses it because the payload cannot travel.
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
    assert!(matches!(err, ExecError::PayloadNotCarried { .. }), "got {err:?}");
    assert!(err.to_string().contains("cv"), "the message names the payload: {err}");
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
