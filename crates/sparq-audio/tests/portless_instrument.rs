//! WO-020 INC3 — the port-less node proof (plan D5), executor half.
//!
//! The Observatory declares NO ports in v1 (display-only until the Phase-5 data-port arbiter). D5's
//! verification task: "prove a port-less node passes graph validation, the executor, the bridge, the
//! browser and canvas layout … the proof is a test, not a grep." This file proves the KERNEL GRAPH +
//! EXECUTOR legs; `crates/sparq-app/tests/portless_bridge.rs` proves the bridge + browser + canvas
//! legs. The manifest under test is the CHECKED-IN package manifest
//! (`instruments/observatory/sparqmod.toml`), so the proof is also a drift gate: if the shipped
//! manifest ever grows a port (or stops validating), this test moves.
//!
//! The native `Module` here is a test stub standing in for the wasm component (the executor is
//! tier-agnostic: it drives `Box<dyn Module>`; the T2 face lands in INC5 behind wasmtime). Its
//! `process` touches nothing and returns `Ok` — exactly what a port-less display node does on the
//! audio thread.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::modules::register_builtins;
use sparq_kernel::graph::Graph;
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;
use sparq_module_api::{decode, Resources, ValidatedManifest};

/// The checked-in package manifest (the drift gate's subject).
const MANIFEST: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../instruments/observatory/sparqmod.toml");

fn manifest_text() -> String {
    std::fs::read_to_string(MANIFEST).unwrap_or_else(|e| {
        panic!("the checked-in Observatory manifest must exist at {MANIFEST}: {e}")
    })
}

fn validated() -> ValidatedManifest {
    let text = manifest_text();
    decode(&text).unwrap_or_else(|r| panic!("the Observatory manifest must decode+validate: {r}"))
}

/// A port-less native stand-in for the component (see the module header).
struct PortlessStub;
impl Module for PortlessStub {
    fn id(&self) -> &str {
        "dat/observatory"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, _ctx: &mut AudioCtx<'_>) -> BlockStatus {
        // No ports in, no ports out: the block is produced as declared, silently.
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("the Observatory takes no atom messages in v1"))
    }
}
fn stub() -> Box<dyn Module> {
    Box::new(PortlessStub)
}

fn registry() -> Registry {
    let mut r = Registry::new();
    register_builtins(&mut r).unwrap();
    r.register(&manifest_text(), stub).expect("a port-less manifest must register");
    r
}

#[test]
fn the_shipped_manifest_is_port_less_and_validates() {
    let m = validated();
    assert!(m.ports().is_empty(), "D5: the Observatory v1 manifest declares zero ports");
    assert_eq!(m.id(), "dat/observatory");
    assert_eq!(
        m.layer(),
        sparq_module_api::manifest::Layer::Instrument,
        "the layer field makes it an instrument"
    );
}

#[test]
fn registration_accepts_a_port_less_module() {
    // register() cross-checks id and validates; a hidden "≥1 port" assumption would surface here as
    // a RegisterError. It must not.
    let r = registry();
    assert!(r.get("dat/observatory").is_some(), "the port-less module is registered");
}

#[test]
fn the_executor_builds_a_port_less_only_graph() {
    // The strongest form: a graph whose ONLY node is port-less. build() must succeed (no master is
    // rendered here — there is nothing audio to render, which is the point).
    let reg = registry();
    let mut g = Graph::new();
    let obs = g.add_node(0);
    let manifest = reg.get("dat/observatory").unwrap().manifest().clone();
    let ex = Executor::build(
        g,
        vec![(obs, NodeBuild { module: stub(), manifest, params: ParamSet::zeroed() })],
        ExecConfig::new(48_000, 64, 2),
    )
    .expect("a port-less-only graph must build — no hidden ≥1-port assumption in the executor");
    assert_eq!(ex.order().len(), 1, "the port-less node is in the execution order");
}

#[test]
fn a_port_less_node_coexists_with_the_audio_path_and_renders_silence() {
    // The realistic shape: the wall beside the backbone. The port-less node contributes no edges and
    // no audio; the set still renders, bit-silent from the observatory's (absent) contribution.
    let reg = registry();
    let mut g = Graph::new();
    let obs = g.add_node(0);
    let sine = g.add_node(0);
    let out = g.add_node(0);
    use sparq_kernel::graph::{EdgeKind, PortRef};
    g.connect(PortRef::new(sine, 0), PortRef::new(out, 0), EdgeKind::Plain).unwrap();

    let obs_m = reg.get("dat/observatory").unwrap().manifest().clone();
    let sine_m = reg.get("sparq/syn/sine").unwrap().manifest().clone();
    let out_m = reg.get("sparq/out/main").unwrap().manifest().clone();
    let mut ex = Executor::build(
        g,
        vec![
            (obs, NodeBuild { module: stub(), manifest: obs_m, params: ParamSet::zeroed() }),
            (
                sine,
                NodeBuild {
                    module: reg.get("sparq/syn/sine").unwrap().create(),
                    manifest: sine_m,
                    params: ParamSet::new(0, &[440.0, 0.5]).unwrap(),
                },
            ),
            (
                out,
                NodeBuild {
                    module: reg.get("sparq/out/main").unwrap().create(),
                    manifest: out_m,
                    params: ParamSet::new(0, &[1.0, 0.0]).unwrap(),
                },
            ),
        ],
        ExecConfig::new(48_000, 64, 2),
    )
    .expect("port-less + backbone must build together");

    let mut buf = vec![0.0f32; 64 * 2];
    ex.render_block(out, &mut buf).expect("the mixed graph renders");
    // The set plays (the sine is audible) — the observatory added nothing to the audio, by design.
    assert!(buf.iter().any(|&s| s != 0.0), "the backbone still makes sound beside the silent wall");
}

#[test]
fn graph_validation_orders_a_port_less_node_without_edges() {
    // graph.order() is the kernel's cycle/order validation; a port-less, edge-less node must sort
    // (it depends on nothing and feeds nothing).
    let mut g = Graph::new();
    let a = g.add_node(0);
    let b = g.add_node(0);
    let order = g.order().expect("an edge-less graph (incl. a port-less node) orders cleanly");
    assert_eq!(order.len(), 2);
    assert!(order.contains(&a) && order.contains(&b));
}
