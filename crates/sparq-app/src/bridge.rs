//! The canvas → executor bridge (WO-013 increment 2): turns the patch the user drew into blocks
//! of audio, through the registry and the WO-008 executor, and writes the WAV.
//!
//! This module is deliberately NOT behind the `ui` feature: it touches no egui type — it converts
//! `sparq_ui::canvas::model::Graph` (the canvas's in-memory truth) into a `sparq_kernel::graph`
//! plus contract module instances, hands that to `sparq_audio::executor`, and renders. Keeping it
//! ungated means `cargo test --workspace` (default features, the CI path) exercises the whole
//! chain: manifest → registry → factory → graph → executor → samples → WAV.
//!
//! The honest limits, declared: parameters come from the manifests' DEFAULTS until the inspector
//! (WO-013 increment 3) gives nodes param state; every canvas wire becomes a `Plain` edge because
//! the canvas has no delay-edge UI yet; and a patch containing cv/event/data edges is refused by
//! the executor in words (v0 `AudioCtx` carries audio only) — the refusal surfaces verbatim in
//! the shell log rather than rendering silence.

// Without the `ui` feature nothing calls into the bridge — its only consumer (the shell) is
// ui-gated — but its tests still run in the default build, which is exactly why it is ungated.
#![cfg_attr(not(feature = "ui"), allow(dead_code))]

use std::collections::HashMap;
use std::path::Path;

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::wav::{write_wav, SampleFormat};
use sparq_kernel::graph::{
    EdgeKind, Graph as KernelGraph, NodeId as KernelNodeId, PortRef as KernelPortRef,
};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;
use sparq_ui::canvas::model::{Graph as CanvasGraph, NodeId as CanvasNodeId, NodeSpec};
use sparq_ui::geom::Vec2;
use sparq_ui::tokens::LAYOUT_CANVAS_SNAP;

/// Render length for a canvas render, seconds. Transport-aware lengths are WO-009's; until then
/// a fixed, stated number — the log line always says what was rendered.
pub const RENDER_SECONDS: f64 = 5.0;
/// Render sample rate.
pub const RENDER_RATE: u32 = 48_000;
/// Render block size (the performance default, §5.4).
pub const RENDER_BLOCK: usize = 64;
/// Render device channels.
pub const RENDER_CHANNELS: usize = 2;

/// The demo patch the shell starts with, built from the REGISTRY — the same manifests discovery
/// reads, never a hand-copied port list (the drift class WO-007 exists to prevent).
///
/// sine → gain → rms (analysis tap) plus one unconnected spare Gain, mirroring the reference
/// modules. Only modules that actually exist (#58).
///
/// # Errors
/// A sentence naming the built-in that is missing — which is a build bug, not a runtime state.
pub fn demo_graph(registry: &Registry) -> Result<CanvasGraph, String> {
    let spec = |id: &str| -> Result<NodeSpec, String> {
        let reg = registry.get(id).ok_or_else(|| {
            format!("built-in `{id}` is not in the registry — register_builtins first")
        })?;
        let m = reg.manifest();
        let name = m.manifest().identity.display_name.clone().unwrap_or_else(|| id.to_string());
        Ok(NodeSpec::new(id, name, m.ports().to_vec()))
    };

    let mut g = CanvasGraph::new();
    let grid = LAYOUT_CANVAS_SNAP as f32;
    let sine = g.op_add_node(spec("sparq/syn/sine")?, Vec2::new(0.0, 0.0));
    let gain = g.op_add_node(spec("sparq/util/gain")?, Vec2::new(grid * 40.0, 0.0));
    let rms = g.op_add_node(spec("sparq/ana/rms")?, Vec2::new(grid * 80.0, 0.0));
    let _spare = g.op_add_node(spec("sparq/util/gain")?, Vec2::new(grid * 40.0, grid * 25.0));
    let (sid, gid, rid) = (node_id(&sine), node_id(&gain), node_id(&rms));
    // sine.out (mono) → gain.in (stereo): the documented fan-out.
    let _ = g.op_add_wire(
        sparq_ui::canvas::model::PortRef::new(sid, 0),
        sparq_ui::canvas::model::PortRef::new(gid, 0),
    );
    // gain.out → rms.in: the analysis tap. rms.level (cv out) stays unconnected — undrawn wires
    // are fine; drawn cv wires are the executor's refused-with-words case.
    let _ = g.op_add_wire(
        sparq_ui::canvas::model::PortRef::new(gid, 1),
        sparq_ui::canvas::model::PortRef::new(rid, 0),
    );
    Ok(g)
}

fn node_id(op: &sparq_ui::canvas::model::Op) -> CanvasNodeId {
    match op {
        sparq_ui::canvas::model::Op::AddNode(n) => n.id,
        // demo_graph only ever adds; a different op here is a bug in this file, and the fallback
        // id fails the build loudly at the first wire instead of silently miswiring.
        _ => CanvasNodeId::MAX,
    }
}

/// Convert a canvas graph into a kernel graph + executor, with `master` as the rendered node.
///
/// # Errors
/// A sentence for the operator: a module the canvas shows but the registry does not have (the
/// #58 invariant, re-checked at the last gate before sound), a wire the kernel refuses, or an
/// executor refusal (payload, channels, shape) passed through verbatim.
pub fn build(
    graph: &CanvasGraph,
    master: CanvasNodeId,
    registry: &Registry,
) -> Result<(Executor, KernelNodeId), String> {
    let mut kg = KernelGraph::new();
    let mut map: HashMap<CanvasNodeId, KernelNodeId> = HashMap::new();
    for n in graph.nodes() {
        let Some(reg) = registry.get(&n.spec.module_id) else {
            return Err(format!(
                "module `{}` is not installed — the canvas must not offer what does not exist \
                 (defect #58); remove the node or install the module",
                n.spec.module_id
            ));
        };
        let latency = reg.manifest().manifest().resources.latency_samples.unwrap_or(0);
        let kid = kg.add_node(latency);
        map.insert(n.id, kid);
    }
    for w in graph.wires() {
        let (Some(from), Some(to)) = (map.get(&w.from.node), map.get(&w.to.node)) else {
            return Err(format!("wire {} references a node outside the graph", w.id));
        };
        kg.connect(
            KernelPortRef::new(*from, w.from.index as u32),
            KernelPortRef::new(*to, w.to.index as u32),
            EdgeKind::Plain,
        )
        .map_err(|e| {
            format!("the kernel graph refused a wire the canvas accepted: {e} — report this")
        })?;
    }
    let Some(&k_master) = map.get(&master) else {
        return Err("the master node is not in the graph".to_string());
    };

    let mut builds = Vec::new();
    for n in graph.nodes() {
        let reg = registry
            .get(&n.spec.module_id)
            .ok_or_else(|| format!("module `{}` vanished mid-build", n.spec.module_id))?;
        // Parameters: the manifest defaults, until the inspector gives nodes param state
        // (WO-013 increment 3). Stated here, and stated in the render log line.
        let defaults: Vec<f32> = reg
            .manifest()
            .manifest()
            .params
            .iter()
            .map(|p| p.default.unwrap_or(0.0) as f32)
            .collect();
        let params = ParamSet::new(1, &defaults).ok_or_else(|| {
            format!("module `{}` declares more than MAX_PARAMS parameters", n.spec.module_id)
        })?;
        builds.push((
            map[&n.id],
            NodeBuild { module: reg.create(), manifest: reg.manifest().clone(), params },
        ));
    }

    let cfg = ExecConfig {
        sample_rate: RENDER_RATE,
        block_frames: RENDER_BLOCK,
        device_channels: RENDER_CHANNELS,
    };
    let ex = Executor::build(kg, builds, cfg).map_err(|e| e.to_string())?;
    Ok((ex, k_master))
}

/// Render the canvas patch to a WAV file. Returns the evidence line for the shell log.
///
/// # Errors
/// Anything [`build`] reports, plus render or file-write failures — always a sentence.
pub fn render_wav(
    graph: &CanvasGraph,
    master: CanvasNodeId,
    registry: &Registry,
    path: &Path,
) -> Result<String, String> {
    let master_name = graph
        .node(master)
        .map(|n| n.title().to_string())
        .unwrap_or_else(|| format!("node {master}"));
    let (mut ex, k_master) = build(graph, master, registry)?;
    let blocks = ((RENDER_SECONDS * f64::from(RENDER_RATE)).ceil() as usize / RENDER_BLOCK).max(1);
    let mut out = vec![0.0f32; RENDER_BLOCK * RENDER_CHANNELS];
    let mut samples: Vec<f32> = Vec::with_capacity(blocks * RENDER_BLOCK * RENDER_CHANNELS);
    for _ in 0..blocks {
        ex.render_block(k_master, &mut out).map_err(|e| format!("render failed: {e}"))?;
        samples.extend_from_slice(&out);
    }
    let hash = hex64(fnv1a64_f32(&samples));
    let bytes =
        write_wav(path, RENDER_RATE, RENDER_CHANNELS as u16, SampleFormat::Float32, &samples)
            .map_err(|e| format!("cannot write `{}`: {e}", path.display()))?;
    Ok(format!(
        "master `{master_name}` · {} blocks · {:.1} s · {} Hz/{} ch f32 · {} bytes · budget {} B · hash {hash}",
        blocks,
        RENDER_SECONDS,
        RENDER_RATE,
        RENDER_CHANNELS,
        bytes,
        ex.memory_budget_bytes()
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use sparq_audio::modules::register_builtins;
    use sparq_ui::canvas::interact::CanvasState;
    use sparq_ui::canvas::model::Op;

    fn registry() -> Registry {
        let mut r = Registry::new();
        register_builtins(&mut r).unwrap();
        r
    }

    #[test]
    fn the_demo_patch_builds_and_renders_the_expected_signal() {
        let reg = registry();
        let g = demo_graph(&reg).unwrap();
        let master = CanvasState::new().resolve_master(&g).expect("the demo has a master");
        let (mut ex, km) = build(&g, master, &reg).unwrap();
        let mut out = vec![0.0f32; RENDER_BLOCK * RENDER_CHANNELS];
        for _ in 0..100 {
            ex.render_block(km, &mut out).unwrap();
        }
        // sine 440 @0.5 → gain (manifest default 1.0): the master carries a 0.5-amplitude sine.
        let peak = out.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!((peak - 0.5).abs() < 1e-3, "peak {peak}");
        assert!(ex.memory_budget_bytes() > 0);
    }

    #[test]
    fn a_module_the_registry_does_not_have_is_refused_in_words() {
        let reg = registry();
        let mut g = CanvasGraph::new();
        let op = g.op_add_node(NodeSpec::new("sparq/nope", "Nope", vec![]), Vec2::ZERO);
        let id = match op {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        };
        let err = build(&g, id, &reg).unwrap_err();
        assert!(err.contains("not installed"), "{err}");
        assert!(err.contains("sparq/nope"), "{err}");
    }

    #[test]
    fn render_wav_writes_a_real_file_with_evidence() {
        let reg = registry();
        let g = demo_graph(&reg).unwrap();
        let master = CanvasState::new().resolve_master(&g).unwrap();
        let path = std::env::temp_dir().join(format!("sparq-bridge-{}.wav", std::process::id()));
        let ev = render_wav(&g, master, &reg, &path).unwrap();
        assert!(path.exists(), "the wav was not written");
        let len = std::fs::metadata(&path).unwrap().len();
        // 5 s × 48 kHz × 2 ch × 4 B ≈ 1.92 MB + header.
        assert!(len > 1_900_000, "suspiciously small wav: {len} bytes");
        assert!(ev.contains("hash") && ev.contains("master"), "evidence line: {ev}");
        std::fs::remove_file(&path).ok();
    }
}
