//! The canvas → executor bridge (WO-013 increment 2): turns the patch the user drew into blocks
//! of audio, through the registry and the WO-008 executor, and writes the WAV.
//!
//! This module is deliberately NOT behind the `ui` feature: it touches no egui type — it converts
//! `sparq_ui::canvas::model::Graph` (the canvas's in-memory truth) into a `sparq_kernel::graph`
//! plus contract module instances, hands that to `sparq_audio::executor`, and renders. Keeping it
//! ungated means `cargo test --workspace` (default features, the CI path) exercises the whole
//! chain: manifest → registry → factory → graph → executor → samples → WAV.
//!
//! The honest limits, declared: parameters come from the NODE'S param state (the inspector,
//! WO-013 increment 3) — which starts at the manifests' defaults, so an untouched patch renders
//! exactly what an increment-2 patch did; every canvas wire becomes a `Plain` edge because the
//! canvas has no delay-edge UI yet. Since contract v1 (WO-008 increment 4) the executor CARRIES
//! `cv` and `event` edges, so a canvas patch that wires them builds and renders for real; a
//! `data`/`gpu`/`atom` edge is still refused by the executor in words (those payloads do not
//! travel), and that refusal surfaces verbatim in the shell log rather than rendering silence.

// Without the `ui` feature nothing calls into the bridge — its only consumer (the shell) is
// ui-gated — but its tests still run in the default build, which is exactly why it is ungated.
#![cfg_attr(not(feature = "ui"), allow(dead_code))]

use std::collections::HashMap;
use std::path::Path;

use sparq_audio::executor::{ExecConfig, Executor, LatencyMode, NodeBuild, Watchdog};
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
/// sine → gain → rms (analysis tap) plus a bare `out/main` waiting to be wired. Only modules
/// that actually exist (#58).
///
/// The fourth node USED to be an unwired spare Gain. WO-008 increment 7's host-side `required`
/// enforcement made that an illegal patch (a bare processor is exactly what the manifest
/// vocabulary forbids), so the demo's spare became the one module DESIGNED to sit bare:
/// `out/main`'s input is `required = false` (an unwired master out is a legal patch shape),
/// and `resolve_master`'s documented handover rule ignores an unwired out/main — so the master
/// stays the chain gain and the rendered samples are bit-identical to the old demo's (the
/// device's canvas-render baseline does not move). The smokes keep their free stereo input to
/// drag onto, and the wire-level gates keep a node that honestly reads at rest.
///
/// # Errors
/// A sentence naming the built-in that is missing — which is a build bug, not a runtime state.
pub fn demo_graph(registry: &Registry) -> Result<CanvasGraph, String> {
    let spec = |id: &str| -> Result<NodeSpec, String> {
        let reg = registry.get(id).ok_or_else(|| {
            format!("built-in `{id}` is not in the registry — register_builtins first")
        })?;
        // The validated manifest IS the spec source — ports and params, never a hand-copy.
        Ok(NodeSpec::from_manifest(reg.manifest()))
    };

    let mut g = CanvasGraph::new();
    let grid = LAYOUT_CANVAS_SNAP as f32;
    // Positions tell the patch's story the way the mockup's does (design-mode.svg): source
    // left, processor centre, analysis below the chain, output right — a patch that reads
    // left-to-right at first sight instead of huddling in the corner.
    let sine = g.op_add_node(spec("sparq/syn/sine")?, Vec2::new(0.0, grid * 20.0));
    let gain = g.op_add_node(spec("sparq/util/gain")?, Vec2::new(grid * 50.0, grid * 20.0));
    let rms = g.op_add_node(spec("sparq/ana/rms")?, Vec2::new(grid * 50.0, grid * 50.0));
    let _main = g.op_add_node(spec("sparq/out/main")?, Vec2::new(grid * 100.0, grid * 20.0));
    let (sid, gid, rid) = (node_id(&sine), node_id(&gain), node_id(&rms));
    // sine.out (mono) → gain.in (stereo): the documented fan-out.
    let _ = g.op_add_wire(
        sparq_ui::canvas::model::PortRef::new(sid, 0),
        sparq_ui::canvas::model::PortRef::new(gid, 0),
    );
    // gain.out → rms.in: the analysis tap. rms.level (cv out) stays unconnected — undrawn
    // OUTPUT wires are fine (the enforcement is about required INPUTS); drawn cv wires are the
    // executor's refused-with-words case. The out/main stays unwired on purpose: it is the
    // master-handover module waiting for the operator (wire it and the MASTER badge moves to
    // it, per resolve_master's documented rule).
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

/// The module browser's catalogue, built from the registry (WO-013 increment 3). This is the
/// ONLY place the registry becomes UI catalogue data, and it is built from the same validated
/// manifests the executor instantiates from — so the browser cannot offer a module that is not
/// installed (defect #58's rule, structurally, at the third consumer). Sorted by module id:
/// deterministic rows for the audit and the goldens.
#[must_use]
pub fn browser_catalog(registry: &Registry) -> Vec<sparq_ui::canvas::browser::BrowserItem> {
    let mut ids = registry.ids();
    ids.sort_unstable();
    ids.iter()
        .filter_map(|id| {
            registry.get(id).map(|reg| {
                let m = reg.manifest().manifest();
                sparq_ui::canvas::browser::BrowserItem {
                    spec: NodeSpec::from_manifest(reg.manifest()),
                    summary: m.identity.summary.clone().unwrap_or_default(),
                    category: m.classification.category.clone().unwrap_or_default(),
                }
            })
        })
        .collect()
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
    let (ex, k_master, _map) = build_with_map(graph, master, registry)?;
    Ok((ex, k_master))
}

/// [`build`], also returning the canvas-node → kernel-node map. The map is what lets a caller read
/// per-node executor state (meters, taps) back into canvas terms — which is exactly what the live
/// wire levels need ([`node_levels`]). Kept separate so [`build`]'s signature stays the two things
/// a renderer wants.
///
/// # Errors
/// Exactly what [`build`] reports.
pub fn build_with_map(
    graph: &CanvasGraph,
    master: CanvasNodeId,
    registry: &Registry,
) -> Result<(Executor, KernelNodeId, HashMap<CanvasNodeId, KernelNodeId>), String> {
    build_with_map_at(graph, master, registry, render_config())
}

/// The [`ExecConfig`] every OFFLINE canvas path builds at: the render constants, raw latency
/// (the canvas render reports what the patch declares, it does not silently re-time it —
/// ADR-006.6's switch is opt-in; the shell will expose it when the canvas does), the default
/// watchdog. One named door so the live path can differ in exactly one dimension: the numbers.
#[must_use]
pub fn render_config() -> ExecConfig {
    ExecConfig {
        sample_rate: RENDER_RATE,
        block_frames: RENDER_BLOCK,
        device_channels: RENDER_CHANNELS,
        latency: LatencyMode::Raw,
        watchdog: Watchdog::default(),
    }
}

/// [`build_with_map`] at an explicit config — the LIVE session's door (WO-012 increment 2). The
/// executor a device stream renders through must be built for the NEGOTIATED truth (the probe
/// open's rate · block · channels), never for the offline render constants: a shared-mode ladder
/// can land on the engine's mix format, and a patch built at the wrong rate is a latency lie the
/// `play` path learned to refuse. Everything else — registry instantiation, node params, the
/// map — is the identical code path, so an offline and a live build of the same patch at the
/// same numbers are the same executor.
///
/// # Errors
/// Exactly what [`build_with_map`] reports.
pub fn build_with_map_at(
    graph: &CanvasGraph,
    master: CanvasNodeId,
    registry: &Registry,
    cfg: ExecConfig,
) -> Result<(Executor, KernelNodeId, HashMap<CanvasNodeId, KernelNodeId>), String> {
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
        // Parameters: the NODE's state (inspector edits, undoable), which starts at the
        // manifest defaults — so a never-touched patch renders exactly as before increment 3.
        // The count is the manifest's own: `effective_params` is one value per declared param.
        let values = n.effective_params();
        let params = ParamSet::new(1, &values).ok_or_else(|| {
            format!("module `{}` declares more than MAX_PARAMS parameters", n.spec.module_id)
        })?;
        builds.push((
            map[&n.id],
            NodeBuild { module: reg.create(), manifest: reg.manifest().clone(), params },
        ));
    }

    let ex = Executor::build(kg, builds, cfg).map_err(|e| e.to_string())?;
    Ok((ex, k_master, map))
}

/// Blocks rendered to freshen the meters before a level read. A wire level is the peak of the most
/// recent block, so the preview must be long enough for a real signal to have sounded: ~85 ms at
/// 48 kHz/64 clears a low note's attack and is short enough to feel instant on a UI refresh.
pub const LEVEL_PREVIEW_BLOCKS: usize = 64;

/// The live wire levels for a patch: render a short preview and read each node's meter, mapping the
/// executor's kernel nodes back to canvas nodes. This is the ONLY place wire levels come from —
/// the executor's own peak meter, never a value invented from canvas data (the park note's rule:
/// animating wires from anything but the real signal would be a lie in motion). The painter maps
/// the result onto wires with [`sparq_ui::canvas::levels::wire_level`].
///
/// The level is the node's PEAK across its audio outputs in the last previewed block: peak is the
/// honest "is signal flowing right now" reading for a wire glow (rms would under-read transients).
/// A node's folded meter covers its AUDIO outputs, so a cv-only source (`ana/rms`, `mod/lfo`,
/// `env/ad`) reads 0.0 there — which is true of its audio, and useless as a wire level. So the
/// set carries a second kind of entry (WO-013 increment 5, retiring increment 4's declared limit):
/// each cv OUTPUT port's own level, read from the value the executor already holds for that port
/// (`node_cv_block` / `node_cv_audio`) as a magnitude, and `wire_level` prefers it for a wire out
/// of that port. Still read, never invented. Parked and still not faked: per-port AUDIO levels
/// (the rings can carry port ids) and the continuous play-time fill, both of which ride the
/// live-HAL-through-`SharedEngine` increment; when that lands the same canvas field is filled
/// from `read_meters` instead of a preview render, and the mapping is identical.
///
/// # Errors
/// Exactly what [`build_with_map`] or the render reports.
pub fn node_levels(
    graph: &CanvasGraph,
    master: CanvasNodeId,
    registry: &Registry,
) -> Result<sparq_ui::canvas::levels::NodeLevels, String> {
    let (mut ex, k_master, map) = build_with_map(graph, master, registry)?;
    let mut out = vec![0.0f32; RENDER_BLOCK * RENDER_CHANNELS];
    for _ in 0..LEVEL_PREVIEW_BLOCKS {
        ex.render_block(k_master, &mut out).map_err(|e| format!("level preview failed: {e}"))?;
    }
    let mut levels = sparq_ui::canvas::levels::NodeLevels::new();
    for (canvas_id, kernel_id) in &map {
        if let Some(m) = ex.meter(*kernel_id) {
            levels.set(*canvas_id, m.peak);
        }
    }
    // Per-port cv levels (WO-013 increment 5). A node's folded meter covers its AUDIO outputs, so
    // a cv-only source (`mod/lfo`, `env/ad`, `ana/rms`) read at rest and its cv wire never lit —
    // increment 4's declared limit. The executor already holds what each cv output published this
    // block (`node_cv_block` for a block-rate value, `node_cv_audio` for an audio-rate buffer), so
    // the level is READ, not invented: the magnitude of the real signal on that port. Audio wires
    // keep the folded node meter — per-port AUDIO levels ride the live-HAL increment.
    for n in graph.nodes() {
        let Some(&kernel_id) = map.get(&n.id) else { continue };
        for (port, p) in n.spec.ports.iter().enumerate() {
            if p.direction != sparq_module_api::port::Direction::Out
                || p.port_type != sparq_module_api::port::PortType::Cv
            {
                continue;
            }
            let idx = u32::try_from(port).unwrap_or(u32::MAX);
            let level = ex.node_cv_audio(kernel_id, idx).map_or_else(
                || ex.node_cv_block(kernel_id, idx).map(f32::abs).unwrap_or(0.0),
                |buf| buf.iter().fold(0.0f32, |a, &s| a.max(s.abs())),
            );
            levels.set_port(n.id, port, level);
        }
    }
    Ok(levels)
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
        // sine 440 @0.5 → gain (manifest default 1.0): the master carries a 0.5-amplitude
        // sine. (The demo's bare out/main does not count for master resolution — the
        // documented handover rule ignores an unwired one — and renders Silenced at rest.)
        let peak = out.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!((peak - 0.5).abs() < 1e-3, "peak {peak}");
        assert!(ex.memory_budget_bytes() > 0);
    }

    #[test]
    fn the_config_door_delegates_bit_identically_and_the_live_door_takes_the_negotiated_truth() {
        // WO-012 increment 2, D5: `build_with_map` now delegates to `build_with_map_at` at the
        // render constants — the delegation must be bit-identical (pinned, not hoped), and the
        // new door must genuinely build at DIFFERENT numbers, because a live session builds for
        // the negotiated device truth, whatever it turns out to be.
        let reg = registry();
        let g = demo_graph(&reg).unwrap();
        let master = CanvasState::new().resolve_master(&g).unwrap();

        let (mut a, am, map_a) = build_with_map(&g, master, &reg).unwrap();
        let (mut b, bm, map_b) = build_with_map_at(&g, master, &reg, render_config()).unwrap();
        assert_eq!(am, bm, "the master designation is the same node");
        assert_eq!(map_a, map_b, "the canvas→kernel map is identical");
        let n = RENDER_BLOCK * RENDER_CHANNELS;
        let (mut out_a, mut out_b) = (vec![0.0f32; n], vec![0.0f32; n]);
        for _ in 0..100 {
            a.render_block(am, &mut out_a).unwrap();
            b.render_block(bm, &mut out_b).unwrap();
            assert!(
                out_a.iter().zip(&out_b).all(|(x, y)| x.to_bits() == y.to_bits()),
                "the delegation renders bit-identically"
            );
        }

        // The live door at numbers no offline path uses: it builds, it renders at ITS block
        // size, and the map still covers every node — the negotiated truth is honoured.
        let cfg = ExecConfig::new(44_100, 96, 2);
        let (mut c, cm, map_c) = build_with_map_at(&g, master, &reg, cfg).unwrap();
        assert_eq!(map_c.len(), map_a.len(), "every node is mapped at any config");
        let mut out_c = vec![0.0f32; 96 * 2];
        c.render_block(cm, &mut out_c).unwrap();
        let peak = out_c.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
        assert!(peak > 0.0, "the 44.1 kHz/96-frame executor really renders the patch");
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

    #[test]
    fn the_catalogue_is_the_registry_sorted_with_full_specs() {
        let reg = registry();
        let cat = browser_catalog(&reg);
        assert_eq!(cat.len(), reg.len());
        let ids: Vec<&str> = cat.iter().map(|i| i.spec.module_id.as_str()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "catalogue order is deterministic");
        let sine = cat.iter().find(|i| i.spec.module_id == "sparq/syn/sine").unwrap();
        assert_eq!(sine.spec.params.len(), 2, "freq + amp travel with the spec");
        assert_eq!(sine.spec.ports.len(), 1);
        assert!(!sine.summary.is_empty() && !sine.category.is_empty(), "row prose is populated");
    }

    #[test]
    fn wire_levels_come_from_real_meters_not_from_canvas_data() {
        let reg = registry();
        let g = demo_graph(&reg).unwrap();
        let master = CanvasState::new().resolve_master(&g).unwrap();
        let levels = super::node_levels(&g, master, &reg).unwrap();

        // The demo is sine(0.5) → gain(1.0) → rms, plus a bare out/main. The levels must be
        // the metered signal: the sine and the in-chain gain carry the 0.5 peak; the rms node
        // reads at rest (it has no audio output to meter); and the bare out/main reads at rest
        // too — it renders Silenced (optional input, nothing wired), and a level invented from
        // canvas data would not know the difference between "unwired" and "silent signal".
        let by_module = |id: &str| -> Vec<CanvasNodeId> {
            g.nodes().iter().filter(|n| n.spec.module_id == id).map(|n| n.id).collect()
        };
        let sine = by_module("sparq/syn/sine")[0];
        let rms = by_module("sparq/ana/rms")[0];
        let gains = by_module("sparq/util/gain");
        assert!(
            (levels.get(sine) - 0.5).abs() < 1e-2,
            "the sine wire is hot at its 0.5 peak: {}",
            levels.get(sine)
        );
        assert_eq!(
            levels.get(rms),
            0.0,
            "an analysis node carries no audio, so its wire is at rest"
        );
        let hot = gains.iter().any(|&gid| levels.get(gid) > 0.4);
        assert!(hot, "the in-chain gain is hot at its metered 0.5 peak: {gains:?}");
        let main = by_module("sparq/out/main")[0];
        assert_eq!(
            levels.get(main),
            0.0,
            "the bare out/main rendered Silenced, and its level says so — cold, not faked"
        );
    }

    #[test]
    fn wire_levels_follow_the_signal_when_a_param_changes() {
        // The "not faked" proof from the other side: turn the gain down and the level it publishes
        // falls with it. A level synthesised from canvas data could not track the rendered signal.
        let reg = registry();
        let mut g = demo_graph(&reg).unwrap();
        let master = CanvasState::new().resolve_master(&g).unwrap();
        let gid = g
            .nodes()
            .iter()
            .find(|n| {
                n.spec.module_id == "sparq/util/gain" && g.wires().iter().any(|w| w.to.node == n.id)
            })
            .map(|n| n.id)
            .unwrap();
        let before = super::node_levels(&g, master, &reg).unwrap().get(gid);
        let pidx = g.node(gid).unwrap().spec.params.iter().position(|p| p.id == "gain").unwrap();
        g.op_set_param(gid, pidx, 0.1).unwrap();
        let after = super::node_levels(&g, master, &reg).unwrap().get(gid);
        assert!(before > 0.4, "unity gain is hot: {before}");
        assert!(after < 0.06, "gain 0.1 over a 0.5 sine peaks near 0.05: {after}");
        assert!(after < before, "the level FOLLOWS the signal down");
    }

    /// The increment-5 cv rig: sine → { svf.in, rms.in }, rms.level → svf.cutoff-mod — the
    /// analysis-as-control-source patch the mockup calls the thesis wire, as a canvas graph.
    /// Returns (graph, sine id, rms id, the cv wire's id, the audio wire's id).
    fn cv_rig(reg: &Registry) -> (CanvasGraph, CanvasNodeId, CanvasNodeId, u32, u32) {
        let spec =
            |id: &str| -> NodeSpec { NodeSpec::from_manifest(reg.get(id).unwrap().manifest()) };
        let mut g = CanvasGraph::new();
        let sine = g.op_add_node(spec("sparq/syn/sine"), Vec2::ZERO);
        let svf = g.op_add_node(spec("sparq/flt/svf"), Vec2::new(320.0, 0.0));
        let rms = g.op_add_node(spec("sparq/ana/rms"), Vec2::new(320.0, 240.0));
        let (sid, fid, rid) = (node_id(&sine), node_id(&svf), node_id(&rms));
        let w_audio = g.op_add_wire(
            sparq_ui::canvas::model::PortRef::new(sid, 0),
            sparq_ui::canvas::model::PortRef::new(fid, 0),
        );
        let _w_tap = g.op_add_wire(
            sparq_ui::canvas::model::PortRef::new(sid, 0),
            sparq_ui::canvas::model::PortRef::new(rid, 0),
        );
        let w_cv = g.op_add_wire(
            sparq_ui::canvas::model::PortRef::new(rid, 1),
            sparq_ui::canvas::model::PortRef::new(fid, 2),
        );
        let wid = |op: &sparq_ui::canvas::model::Op| match op {
            sparq_ui::canvas::model::Op::AddWire(w) => w.id,
            _ => unreachable!(),
        };
        (g, sid, rid, wid(&w_cv), wid(&w_audio))
    }

    #[test]
    fn a_cv_wire_lights_from_its_own_published_value() {
        // Increment 4's declared limit, retired: the rms node carries NO audio, so its folded
        // meter reads at rest — but its `level` cv port publishes a real value every block, and
        // the wire out of that port must light from THAT, read from the executor, never faked.
        let reg = registry();
        let (g, sid, rid, w_cv, w_audio) = cv_rig(&reg);
        let master = CanvasState::new().resolve_master(&g).expect("the svf is the master");
        let levels = super::node_levels(&g, master, &reg).unwrap();

        assert_eq!(levels.get(rid), 0.0, "the rms node folds no audio: its NODE level is at rest");
        let cv = levels.port(rid, 1).expect("the cv port publishes its own level");
        assert!(
            (0.1..0.5).contains(&cv),
            "the cv level is the metered magnitude of the 0.5 sine's rms: {cv}"
        );
        assert_eq!(
            sparq_ui::canvas::levels::wire_level(&g, &levels, w_cv),
            cv,
            "the cv wire carries its SOURCE PORT's own level"
        );
        // The audio side keeps the increment-4 semantics exactly: the node's folded peak meter.
        let sine_peak = levels.get(sid);
        assert!((sine_peak - 0.5).abs() < 1e-2, "the sine's folded meter: {sine_peak}");
        assert_eq!(sparq_ui::canvas::levels::wire_level(&g, &levels, w_audio), sine_peak);
        assert_eq!(
            levels.port(sid, 0),
            None,
            "AUDIO ports publish no per-port level yet — the parked half stays parked, pinned"
        );
    }

    #[test]
    fn the_cv_level_follows_the_signal_not_the_canvas() {
        // The "not faked" proof for the cv half: turn the sine down and the cv wire's level falls
        // with it. A value synthesised from canvas data could not track the rendered signal.
        let reg = registry();
        let (mut g, sid, rid, _, _) = cv_rig(&reg);
        let master = CanvasState::new().resolve_master(&g).unwrap();
        let before = super::node_levels(&g, master, &reg).unwrap().port(rid, 1).unwrap();
        let amp = g.node(sid).unwrap().spec.params.iter().position(|p| p.id == "amp").unwrap();
        g.op_set_param(sid, amp, 0.1).unwrap();
        let after = super::node_levels(&g, master, &reg).unwrap().port(rid, 1).unwrap();
        assert!(before > 0.1, "the full-amplitude rig is hot: {before}");
        assert!(after < before * 0.5, "amp 0.5 → 0.1 takes the rms level down with it: {after}");
    }

    #[test]
    fn a_node_param_edit_reaches_the_samples_and_back() {
        let reg = registry();
        let mut g = demo_graph(&reg).unwrap();
        let master = CanvasState::new().resolve_master(&g).unwrap();
        let peak_of = |g: &CanvasGraph| -> f32 {
            let (mut ex, km) = build(g, master, &reg).unwrap();
            let mut out = vec![0.0f32; RENDER_BLOCK * RENDER_CHANNELS];
            let mut peak = 0.0f32;
            for _ in 0..100 {
                ex.render_block(km, &mut out).unwrap();
                for s in &out {
                    peak = peak.max(s.abs());
                }
            }
            peak
        };
        let before = peak_of(&g);
        assert!((before - 0.5).abs() < 1e-3, "defaults: sine 0.5 × gain 1.0");

        // The gain node IN THE CHAIN (the demo's only gain), edited through the same op the
        // inspector uses — the bridge must render the node's state, not the manifest default.
        let gid = g
            .nodes()
            .iter()
            .find(|n| {
                n.spec.module_id == "sparq/util/gain" && g.wires().iter().any(|w| w.to.node == n.id)
            })
            .map(|n| n.id)
            .unwrap();
        let pidx = g.node(gid).unwrap().spec.params.iter().position(|p| p.id == "gain").unwrap();
        g.op_set_param(gid, pidx, 0.25).unwrap();
        let after = peak_of(&g);
        assert!((after - 0.125).abs() < 1e-3, "sine 0.5 × gain 0.25 → peak {after}");

        // Undo-equivalent (the inverse op's value): back to the default render, bit-exact.
        g.op_set_param(gid, pidx, 1.0).unwrap();
        assert_eq!(peak_of(&g), before, "the default render is reproducible");
    }
}
