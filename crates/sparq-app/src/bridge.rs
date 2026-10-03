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
use sparq_ui::canvas::layout::{signal_class, SignalClass};
use sparq_ui::canvas::model::{
    Graph as CanvasGraph, NodeId as CanvasNodeId, NodeSpec, Wire as CanvasWire,
    WireId as CanvasWireId, WireTrim,
};
use sparq_ui::geom::Vec2;
use sparq_ui::tokens::LAYOUT_CANVAS_SNAP;

/// The kernel gain nodes synthesised for AUDIO cable-node trims, keyed by the CANVAS wire id
/// (operator round 4, D15/S8). Returned from the build doors so a live session COULD cross amp
/// drags straight to the gain's param — the shipped behaviour is the structural re-stage,
/// which adoption makes silent (the gain instance survives the re-stage on its synthetic
/// wire-derived canvas key, and defect #85's one-pole glide rides the drag zipper-free by
/// construction). The command-ring fast path is a declared LATER optimisation; this map is
/// the door it will walk through.
pub type TrimGainMap = HashMap<CanvasWireId, KernelNodeId>;

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
///
/// The permanent `out/main` is EXCLUDED (operator ruling 2026-10-01): Main Out is not a module
/// the user creates — it is always already on the canvas — so no palette, library or browser
/// offers it, and the spawn door in `sparq-ui::canvas::interact` refuses it structurally too.
#[must_use]
pub fn browser_catalog(registry: &Registry) -> Vec<sparq_ui::canvas::browser::BrowserItem> {
    let mut ids = registry.ids();
    ids.sort_unstable();
    ids.iter()
        .filter(|id| &id[..] != sparq_ui::canvas::OUT_MAIN_ID)
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
    let (ex, k_master, _map, _gains) = build_with_map(graph, master, registry)?;
    Ok((ex, k_master))
}

/// [`build`], also returning the canvas-node → kernel-node map and the cable nodes'
/// synthesised-gain map ([`TrimGainMap`]). The node map is what lets a caller read per-node
/// executor state (meters, taps) back into canvas terms — which is exactly what the live
/// wire levels need ([`node_levels`]); the gain map is the live session's door to the
/// declared LATER set_params fast path (operator round 4, D15/S8). Kept separate so
/// [`build`]'s signature stays the two things a renderer wants.
///
/// # Errors
/// Exactly what [`build`] reports.
pub fn build_with_map(
    graph: &CanvasGraph,
    master: CanvasNodeId,
    registry: &Registry,
) -> Result<(Executor, KernelNodeId, HashMap<CanvasNodeId, KernelNodeId>, TrimGainMap), String> {
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
) -> Result<(Executor, KernelNodeId, HashMap<CanvasNodeId, KernelNodeId>, TrimGainMap), String> {
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
    // The cable node's VOICE (operator round 4, D15): a trimmed AUDIO wire gets an invisible
    // `util/gain` synthesised into its path (the hand-patch equivalent — the r4_bridge gate
    // pins the sentence), a trimmed CV wire records the executor's own `set_cv_trim` door for
    // after the build, and a trimmed CONTROL wire rides the param-mod formula below. An
    // IDENTITY trim synthesises nothing and calls nothing: the wire renders bit-identical to
    // no trim at all (gated, not commented). Event/data/spatial wires REFUSE the cable node
    // in words — those streams carry no amplitude the executor can trim (event/data), or no
    // per-set gain module exists to trim them with yet (spatial).
    let mut trim_gains: TrimGainMap = HashMap::new();
    let mut gain_builds: Vec<(KernelNodeId, NodeBuild)> = Vec::new();
    let mut cv_trims: Vec<(KernelNodeId, u32, f32, f32)> = Vec::new();
    for w in graph.wires() {
        // A wire INTO a scope is a display binding, not a signal path (operator ruling
        // 2026-10-01 r3: the scope accepts any source class): the UI reads the source's own
        // publication, the module's process is a no-op, and the kernel's channel planning has
        // no business judging an audio-or-event wire that nothing consumes. Skipped, declared.
        if graph.node(w.to.node).is_some_and(|n| n.spec.module_id == sparq_ui::canvas::SCOPE_ID) {
            continue;
        }
        // Control wires (r3) are modulation plans, not kernel edges: they ride the executor's
        // param-mod plan below, and the kernel's channel planning never sees them.
        if w.param.is_some() {
            continue;
        }
        // The junction bus collapses (r3): wires touching a mult dot are patching, not signal
        // paths — the copy-edges are emitted just below, source to destination directly.
        if graph.node(w.from.node).is_some_and(|n| n.spec.module_id == sparq_ui::canvas::MULT_ID)
            || graph.node(w.to.node).is_some_and(|n| n.spec.module_id == sparq_ui::canvas::MULT_ID)
        {
            continue;
        }
        let (Some(from), Some(to)) = (map.get(&w.from.node), map.get(&w.to.node)) else {
            return Err(format!("wire {} references a node outside the graph", w.id));
        };
        let src_pref = KernelPortRef::new(*from, w.from.index as u32);
        let dst_pref = KernelPortRef::new(*to, w.to.index as u32);
        if let Some(t) = effective_trim(w) {
            match wire_class(graph, w) {
                SignalClass::Audio => {
                    // The invisible gain (D15): `t.amp` is the gain param — the module's
                    // one-pole glide (defect #85) makes live amp drags zipper-free across
                    // re-stages, and the offset NEVER enters the audio path (the DC rule),
                    // so the bridge does not even read it. The insertion is the hand-patch
                    // equivalent down to the channel conversions: a trim on a wire into a
                    // mono-declared sink rides the stereo gain's documented summing exactly
                    // as the user's own patched gain would (every first-party audio sink is
                    // stereo-or-variable, so today the leg is transparent; per-set gains
                    // arrive with the spatial phase).
                    let (gk, build) = synth_gain(&mut kg, registry, src_pref, dst_pref, t.amp)?;
                    trim_gains.insert(w.id, gk);
                    gain_builds.push((gk, build));
                    continue; // the direct edge is replaced by the gain's two legs
                },
                SignalClass::Cv => {
                    // The executor's own trim door, recorded for after the build — the wire
                    // must exist before a trim can ride it.
                    cv_trims.push((*to, w.to.index as u32, t.amp, t.offset));
                },
                SignalClass::Event | SignalClass::Data => {
                    return Err(format!(
                        "wire {} carries a cable node on an event/data cable — those streams \
                         have no amplitude to trim (a trigger's word is its sample, a data \
                         stream's its payload); tap the node to remove it, or move the trim \
                         onto an audio or cv cable",
                        w.id
                    ));
                },
                SignalClass::Spatial => {
                    return Err(format!(
                        "wire {} carries a cable node on a spatial cable — util/gain is stereo \
                         and no per-set gain module exists yet (the spatial phase ships one); \
                         tap the node to remove it, or trim a mono/stereo leg of the path",
                        w.id
                    ));
                },
                SignalClass::Neutral => {
                    return Err(format!(
                        "wire {} carries a cable node on a payload the executor does not \
                         carry (gpu/atom) — report this",
                        w.id
                    ));
                },
            }
        }
        kg.connect(src_pref, dst_pref, EdgeKind::Plain).map_err(|e| {
            format!("the kernel graph refused a wire the canvas accepted: {e} — report this")
        })?;
    }
    // The junction bus collapses here (operator ruling 2026-10-01 r3): every wire leaving a
    // mult dot becomes a DIRECT kernel edge from the one source feeding that mult — what the
    // canvas draws as dots, the executor runs as the fan-out the kernel already speaks. An
    // unpowered bus (no input dot) contributes nothing, exactly like an unwired module.
    for n in graph.nodes().iter().filter(|n| n.spec.module_id == sparq_ui::canvas::MULT_ID) {
        let Some(feed) = graph.wires().iter().find(|w| w.to.node == n.id && w.param.is_none())
        else {
            continue;
        };
        let Some(&src_k) = map.get(&feed.from.node) else {
            continue;
        };
        // The cable node rides the COLLAPSED direct edge (round 4's declared interaction):
        // the feed's trim and the copy's own trim compose into ONE affine trim per copy —
        // upstream first, then downstream — applied per copy-edge exactly as the direct-wire
        // path applies it (gain synthesis / set_cv_trim / refusals, same voice, same words).
        let feed_trim = effective_trim(feed);
        let feed_class = graph.port(feed.from).map(signal_class).unwrap_or(SignalClass::Neutral);
        for w in graph.wires().iter().filter(|w| w.from.node == n.id) {
            let Some(&dst_k) = map.get(&w.to.node) else { continue };
            let src_pref = KernelPortRef::new(src_k, feed.from.index as u32);
            let dst_pref = KernelPortRef::new(dst_k, w.to.index as u32);
            // Control copies ride their own wire's param-mod plan below (the r3 src_ref
            // resolution), never a kernel edge trim.
            let composed =
                if w.param.is_none() { compose_trims(feed_trim, effective_trim(w)) } else { None };
            if let Some(t) = composed {
                match feed_class {
                    SignalClass::Audio => {
                        let (gk, build) = synth_gain(&mut kg, registry, src_pref, dst_pref, t.amp)?;
                        trim_gains.insert(w.id, gk);
                        gain_builds.push((gk, build));
                        continue;
                    },
                    SignalClass::Cv => {
                        cv_trims.push((dst_k, w.to.index as u32, t.amp, t.offset));
                    },
                    SignalClass::Event | SignalClass::Data => {
                        return Err(format!(
                            "wire {} copies a cable node onto an event/data path — those \
                             streams have no amplitude to trim; tap the node to remove it",
                            w.id
                        ));
                    },
                    SignalClass::Spatial => {
                        return Err(format!(
                            "wire {} copies a cable node onto a spatial path — no per-set \
                             gain module exists yet; tap the node to remove it",
                            w.id
                        ));
                    },
                    SignalClass::Neutral => {
                        return Err(format!(
                            "wire {} copies a cable node onto a payload the executor does \
                             not carry (gpu/atom) — report this",
                            w.id
                        ));
                    },
                }
            }
            kg.connect(src_pref, dst_pref, EdgeKind::Plain).map_err(|e| {
                format!("the kernel graph refused a collapsed mult wire: {e} — report this")
            })?;
        }
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
    // The synthesised trim gains join the build (their kernel ids were assigned in the wire
    // passes above; the executor's node set must match the graph exactly).
    builds.extend(gain_builds);

    let mut ex = Executor::build(kg, builds, cfg).map_err(|e| e.to_string())?;
    // The canvas identities on every canvas build (operator round 4, D1): one key per kernel
    // id, in kernel-id order — the canvas node's own id IS the key — so a live re-stage can
    // ADOPT the running modules (`Executor::adopt_runtime`) instead of resetting everything
    // the user is hearing (the "add a module" transparency). An unstamped node never adopts;
    // this stamp covers every canvas node AND every synthesised trim gain.
    let mut keys = vec![0u64; map.len() + trim_gains.len()];
    for (canvas_id, kid) in &map {
        keys[kid.0 as usize] = u64::from(*canvas_id);
    }
    // The synthesised gains wear synthetic keys: `0x8000_0000_0000_0000 | wire id` — stable
    // across rebuilds, so a re-staged amp drag ADOPTS the running gain (its glide cell is
    // mid-chase) instead of priming a fresh one, and removing the trim retires it like any
    // deleted node. Canvas node ids are u32, so the high bit is a namespace no canvas id can
    // collide with.
    for (wid, gk) in &trim_gains {
        keys[gk.0 as usize] = 0x8000_0000_0000_0000 | u64::from(*wid);
    }
    ex.set_canvas_keys(keys);
    // The cable node's CV side (D15): the wires exist in the build now, so the executor's
    // trim door can ride them — sanitised and clamped at that door, the model's discipline
    // enforced twice so neither door can be walked around.
    for (kid, port, amp, offset) in cv_trims {
        ex.set_cv_trim(kid, port, amp, offset)
            .map_err(|e| format!("the executor refused a cable node's cv trim: {e}"))?;
    }
    // Control connections (operator ruling 2026-10-01 r3): every canvas wire onto a float
    // parameter becomes an executor param-mod plan — the knob's base plus the cv's deviation
    // per block, clamped into the manifest range the bridge reads straight from the spec.
    for w in graph.wires().iter().filter(|w| w.param.is_some()) {
        // A control wire fed through a junction bus modulates from the bus's SOURCE (r3):
        // the dot copies, so the modulation is the source's signal.
        let mut src_ref = w.from;
        for _ in 0..8 {
            if !graph
                .node(src_ref.node)
                .is_some_and(|n| n.spec.module_id == sparq_ui::canvas::MULT_ID)
            {
                break;
            }
            match graph.wires().iter().find(|x| x.to.node == src_ref.node && x.param.is_none()) {
                Some(feed) => src_ref = feed.from,
                None => break,
            }
        }
        if graph.node(src_ref.node).is_some_and(|n| n.spec.module_id == sparq_ui::canvas::MULT_ID) {
            continue; // an unpowered bus modulates nothing, silently-by-rule, loudly-in-words elsewhere
        }
        let (Some(&dst), Some(&src)) = (map.get(&w.to.node), map.get(&src_ref.node)) else {
            return Err(format!("control wire {} references a node outside the graph", w.id));
        };
        let idx = w.param.unwrap_or(0);
        let (lo, hi) = graph
            .node(w.to.node)
            .and_then(|n| n.spec.params.get(idx))
            .map(|d| (d.min as f32, d.max as f32))
            .unwrap_or((0.0, 1.0));
        // The control wire's cable node (D15): its trim rides the param-mod formula — the
        // executor trims the latched cv word BEFORE the additive modulation. An identity
        // trim hands over (1, 0) exactly like no trim at all, and the executor's identity
        // branch makes that bit-exact.
        let (tscale, toffset) = w.trim.map(|t| (t.amp, t.offset)).unwrap_or((1.0, 0.0));
        ex.add_param_mod(dst, idx, src, src_ref.index as u32, lo, hi, tscale, toffset)
            .map_err(|e| format!("the executor refused a control wire: {e}"))?;
    }
    Ok((ex, k_master, map, trim_gains))
}

/// A wire's trim with the identity folded into `None` — identity IS no cable node as far as
/// the kernel goes: nothing synthesised, nothing called, bit-identical samples (the gate
/// measures the sentence).
fn effective_trim(w: &CanvasWire) -> Option<WireTrim> {
    w.trim.filter(|t| *t != WireTrim::identity())
}

/// A wire's signal class — the SOURCE port's own (the palette's rule: a wire carries what
/// its source publishes). An unresolvable port reads Neutral and the trim refusals answer.
fn wire_class(graph: &CanvasGraph, w: &CanvasWire) -> SignalClass {
    graph.port(w.from).map(signal_class).unwrap_or(SignalClass::Neutral)
}

/// Two cable nodes on one signal path, composed (`up` first, then `down`): the affine
/// composition `(v·a₁+o₁)·a₂+o₂ = v·(a₁a₂) + (o₁a₂+o₂)`. The mult collapse's feed/copy pair
/// is the only place two trims can ride one kernel edge, and this is the declared
/// interaction's arithmetic — one trim per copy-edge, whatever the canvas drew.
fn compose_trims(up: Option<WireTrim>, down: Option<WireTrim>) -> Option<WireTrim> {
    match (up, down) {
        (Some(a), Some(b)) => {
            Some(WireTrim { amp: a.amp * b.amp, offset: a.offset * b.amp + b.offset })
        },
        (a, b) => a.or(b),
    }
}

/// Synthesise the cable node's invisible `util/gain` (operator round 4, D15/S8): a kernel
/// node between `src` and `dst` — `src → gain.in`, `gain.out → dst` replacing the direct
/// edge — with `amp` as its gain param. The registry module and manifest, unmodified: the
/// synthesised gain is the HAND-PATCHED gain, byte for byte, which is exactly what the
/// r4_bridge ≡ gate measures. Returns the gain's kernel id and its build.
///
/// # Errors
/// A sentence when `util/gain` is not installed (a build bug — it is a built-in) or when the
/// kernel refuses either leg (refused in words, like every wire).
fn synth_gain(
    kg: &mut KernelGraph,
    registry: &Registry,
    src: KernelPortRef,
    dst: KernelPortRef,
    amp: f32,
) -> Result<(KernelNodeId, NodeBuild), String> {
    let r = registry.get("sparq/util/gain").ok_or_else(|| {
        "the cable node's invisible gain needs `util/gain` — it is a built-in; report this"
            .to_string()
    })?;
    let latency = r.manifest().manifest().resources.latency_samples.unwrap_or(0);
    let gk = kg.add_node(latency);
    kg.connect(src, KernelPortRef::new(gk, 0), EdgeKind::Plain)
        .map_err(|e| format!("the kernel graph refused a cable node's gain leg: {e}"))?;
    kg.connect(KernelPortRef::new(gk, 1), dst, EdgeKind::Plain)
        .map_err(|e| format!("the kernel graph refused a cable node's gain leg: {e}"))?;
    let params = ParamSet::new(1, &[amp])
        .ok_or_else(|| "util/gain declares more than MAX_PARAMS parameters".to_string())?;
    Ok((gk, NodeBuild { module: r.create(), manifest: r.manifest().clone(), params }))
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
    let (mut ex, k_master, map, _gains) = build_with_map(graph, master, registry)?;
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
    // The junction bus copies (r3): a mult dot reads its source's level, so the wires that
    // leave it light like the wires that enter it.
    for n in graph.nodes() {
        if n.spec.module_id != sparq_ui::canvas::MULT_ID {
            continue;
        }
        if let Some(feed) = graph.wires().iter().find(|w| w.to.node == n.id && w.param.is_none()) {
            levels.set(n.id, levels.get(feed.from.node));
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

/// The per-module response-curve contract dispatch (WO-012 increment 5, D2): module id + param
/// snapshot + negotiated sample rate + a frequency grid → linear magnitudes over that grid.
/// This is the ONLY layer that sees both the display model (`sparq_ui::canvas::response`, which
/// CONSUMES magnitudes) and the DSP that owns the math — `SvfFilter::magnitude_at` computes
/// them from the filter's own cached coefficients, pinned to the sine sweep by a `sparq-audio`
/// test, so the display cannot drift from the DSP (the single-source guarantee, tested).
///
/// `svf` is first and is the reference implementation. The curve is the BASE-param response at
/// the declared cutoff: the `cutoff-mod` input moves the audible cutoff per block when it is
/// connected, and the plot shows the param snapshot, not the modulated one — declared, the same
/// way the scope shows its binding rather than guessing one. A module that declares no curve
/// returns `None` → the inspector's honest no-curve state, never a faked flat line. Adding a
/// second curve module later is one arm here plus its own DSP method; the display model and the
/// contract shape do not change.
#[must_use]
pub fn response_curve(
    module_id: &str,
    params: &[f32],
    sample_rate: u32,
    freqs: &[f64],
) -> Option<Vec<f64>> {
    use sparq_audio::dsp::filter::{SvfFilter, SvfMode};
    if module_id != sparq_ui::canvas::inset::SVF_ID || sample_rate == 0 {
        return None;
    }
    // The svf MODULE's own param mapping, mirrored (manifest order, pinned by the test below):
    // clamped into the declared ranges, a non-finite param sanitised to the manifest default.
    let at = |i: usize, default: f32, lo: f32, hi: f32| -> f32 {
        let v = params.get(i).copied().unwrap_or(default);
        if v.is_finite() {
            v.clamp(lo, hi)
        } else {
            default
        }
    };
    let cutoff = f64::from(at(0, 1_000.0, 10.0, 20_000.0));
    let reso = at(1, 0.2, 0.0, 1.0);
    let mode = at(2, 0.0, 0.0, 4.0).round() as i32;
    let mut f = SvfFilter::new(cutoff, reso);
    f.mode = match mode {
        1 => SvfMode::HighPass,
        2 => SvfMode::BandPass,
        3 => SvfMode::Notch,
        4 => SvfMode::Peak,
        _ => SvfMode::LowPass,
    };
    f.prepare(sample_rate);
    Some(freqs.iter().map(|&hz| f.magnitude_at(hz)).collect())
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
    use sparq_module_api::port::Phase;
    use sparq_ui::canvas::connect::{self, ConnectContext, ConnectOutcome};
    use sparq_ui::canvas::interact::CanvasState;
    use sparq_ui::canvas::model::{Op, PortRef as CPortRef};

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
    fn the_svf_param_order_the_display_reads_is_pinned_against_the_registry_manifest() {
        // The scope.rs discipline, on the curve side: the display's param indices are the
        // manifest's own order. If a param is ever inserted before `mode`, THIS fails — loudly,
        // here, instead of silently plotting resonance where cutoff belongs.
        let reg = registry();
        let reg_svf = reg.get(sparq_ui::canvas::inset::SVF_ID).expect("svf is a built-in");
        let spec = NodeSpec::from_manifest(reg_svf.manifest());
        let ids: Vec<&str> = spec.params.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["cutoff", "resonance", "mode", "mod"], "manifest order moved");
    }

    #[test]
    fn the_curve_dispatch_is_the_filters_own_math_and_nothing_else() {
        use sparq_ui::canvas::inset::SVF_ID;
        use sparq_ui::canvas::response::{grid, Axes};
        let axes = Axes::at(48_000);
        let freqs = grid(&axes);

        // The default lowpass: unity-ish below the cutoff, down above it, all finite.
        let mags = response_curve(SVF_ID, &[1_000.0, 0.2, 0.0, 0.0], 48_000, &freqs)
            .expect("svf declares a curve");
        assert_eq!(mags.len(), freqs.len());
        assert!(mags.iter().all(|m| m.is_finite() && *m >= 0.0));
        // Nearest grid index (the grid is log-spaced; a probe frequency is rarely ON a point).
        let db = |f: f64| {
            let i = freqs
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| (*a - f).abs().total_cmp(&(*b - f).abs()))
                .map(|(i, _)| i)
                .unwrap();
            20.0 * mags[i].log10()
        };
        let (lo, hi) = (db(100.0), db(10_000.0));
        assert!(lo.abs() < 1.0, "100 Hz passes a 1 kHz lowpass: {lo} dB");
        assert!(hi < -20.0, "10 kHz is well down: {hi} dB");

        // Mode 1 flips it (the dispatch mirrors the MODULE's mode mapping, not a guess).
        let hp = response_curve(SVF_ID, &[1_000.0, 0.2, 1.0, 0.0], 48_000, &freqs).unwrap();
        assert!(20.0 * hp[0].log10() < -20.0, "a highpass rejects the floor");
        assert!(
            20.0 * hp[hp.len() / 2..].iter().fold(0.0f64, |a, &m| a.max(m)).log10() > -3.0,
            "…and passes the top"
        );

        // The dispatch is EXACTLY the filter's own method — one source, no second copy: the
        // same params through `SvfFilter::magnitude_at` give the same numbers, bit for bit.
        use sparq_audio::dsp::filter::SvfFilter;
        let mut f = SvfFilter::new(1_000.0, 0.2);
        f.prepare(48_000);
        let direct: Vec<f64> = freqs.iter().map(|&hz| f.magnitude_at(hz)).collect();
        assert_eq!(direct, mags, "the dispatch IS magnitude_at — bit-identical");

        // A non-finite param sanitises to the manifest default; a truncated snapshot too.
        let nan = response_curve(SVF_ID, &[f32::NAN, 0.2, 0.0, 0.0], 48_000, &freqs).unwrap();
        let dflt = response_curve(SVF_ID, &[1_000.0, 0.2, 0.0, 0.0], 48_000, &freqs).unwrap();
        assert_eq!(nan, dflt, "NaN cutoff reads the default, never a NaN curve");
        let short = response_curve(SVF_ID, &[], 48_000, &freqs).unwrap();
        assert_eq!(short, response_curve(SVF_ID, &[1_000.0, 0.2, 0.0], 48_000, &freqs).unwrap());

        // The honest refusals: a module with no curve, and a rate that was never negotiated.
        assert!(response_curve("sparq/syn/sine", &[], 48_000, &freqs).is_none());
        assert!(
            response_curve("sparq/util/delay", &[], 48_000, &freqs).is_none(),
            "a future comb response must register its own arm first"
        );
        assert!(response_curve(SVF_ID, &[1_000.0, 0.2, 0.0, 0.0], 0, &freqs).is_none());
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

        let (mut a, am, map_a, _ga) = build_with_map(&g, master, &reg).unwrap();
        let (mut b, bm, map_b, _gb) = build_with_map_at(&g, master, &reg, render_config()).unwrap();
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
        let (mut c, cm, map_c, _gc) = build_with_map_at(&g, master, &reg, cfg).unwrap();
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
        // Operator ruling 2026-10-01: the catalogue is the registry MINUS the permanent
        // `out/main` — Main Out is never user-created, so no palette offers it.
        assert_eq!(cat.len(), reg.len() - 1);
        assert!(
            !cat.iter().any(|i| i.spec.module_id == sparq_ui::canvas::OUT_MAIN_ID),
            "the permanent Main Out is not offered"
        );
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

    #[test]
    fn the_step_bar_display_mirrors_the_module_hash_and_the_span_constant() {
        // Operator round 4, D12/D11 — the cross-crate pin the mirrors hang on: `sparq-ui` is
        // zero-dependency, so the display's copy of the ring hash and the pitch span live in
        // `inset.rs`; THIS test is the single place that sees both crates and proves they
        // agree — the pinned seed-42 ring first (the checked-in evidence), then a wide sweep,
        // so a drift on either side fails a gate instead of silently revoicing a performance.
        const SEED42_RING: [u32; 8] =
            [1517363, 10542902, 13721998, 8088911, 6531285, 16292829, 12864735, 5502068];
        for (i, want) in SEED42_RING.iter().enumerate() {
            let module = sparq_audio::modules::step_value(42, i as u32);
            let display = sparq_ui::canvas::inset::rand_step_value(42, i as u32);
            assert_eq!(module, *want as f32 / 16_777_216.0, "the module's ring is the pinned one");
            assert_eq!(display, module, "the display mirror drifts at step {i}");
        }
        for seed in [0u32, 1, 7, 42, 4096, 65_535] {
            for i in 0..64u32 {
                assert_eq!(
                    sparq_audio::modules::step_value(seed, i),
                    sparq_ui::canvas::inset::rand_step_value(seed, i),
                    "mirror disagreement at seed {seed}, step {i}"
                );
            }
        }
        // The pitch span: the keyboard's class mapping and the quantizer's snap read the same
        // 120 semitones from their own files — pinned equal here, where both are visible.
        assert_eq!(
            sparq_audio::modules::QUANT_PITCH_SEMITONES,
            sparq_ui::canvas::inset::QUANT_PITCH_SEMITONES,
            "the span constants drifted apart"
        );
        assert_eq!(sparq_audio::modules::QUANT_PITCH_SEMITONES, 120.0);
    }

    // ============================================== operator round 4, S8: the cable node's voice
    // The bridge batch's gates (D15/D7), registry+bridge harness: an audio trim IS the
    // hand-patched gain; identity synthesises nothing; a cv trim IS `set_cv_trim`; a control
    // trim rides the D15 formula; the vca takes the lfo wire; a trimmed mult copy rides the
    // collapsed edge, composed.

    const TRIM_BLOCKS: usize = 200;

    /// A canvas node from the registry's own manifest (the browser's construction path).
    fn t_add(reg: &Registry, g: &mut CanvasGraph, id: &str, x: f32) -> CanvasNodeId {
        let spec = NodeSpec::from_manifest(reg.get(id).unwrap().manifest());
        match g.op_add_node(spec, Vec2::new(x, 0.0)) {
            Op::AddNode(n) => n.id,
            _ => unreachable!(),
        }
    }

    fn t_wire(
        g: &mut CanvasGraph,
        from: (CanvasNodeId, usize),
        to: (CanvasNodeId, usize),
    ) -> CanvasWireId {
        match g.op_add_wire(CPortRef::new(from.0, from.1), CPortRef::new(to.0, to.1)) {
            Op::AddWire(w) => w.id,
            _ => unreachable!(),
        }
    }

    fn t_render_hash(ex: &mut Executor, master: KernelNodeId, cfg: &ExecConfig) -> String {
        let mut out = vec![0.0f32; cfg.block_frames * cfg.device_channels];
        let mut samples: Vec<f32> = Vec::with_capacity(TRIM_BLOCKS * out.len());
        for _ in 0..TRIM_BLOCKS {
            ex.render_block(master, &mut out).unwrap();
            samples.extend_from_slice(&out);
        }
        hex64(fnv1a64_f32(&samples))
    }

    /// The manifest defaults of a module — what an untouched canvas node carries.
    fn t_defaults(reg: &Registry, id: &str) -> Vec<f32> {
        reg.get(id)
            .unwrap()
            .manifest()
            .manifest()
            .params
            .iter()
            .map(|p| p.default.unwrap_or(0.0) as f32)
            .collect()
    }

    #[test]
    fn an_audio_cable_trim_is_the_hand_patched_gain_hash_for_hash() {
        let reg = registry();
        let cfg = render_config();
        // World A: sine → master gain, with a cable node on the wire (amp 0.42 — an odd
        // number, so a wrong formula cannot land on it by accident).
        let mut ga = CanvasGraph::new();
        let sine = t_add(&reg, &mut ga, "sparq/syn/sine", 0.0);
        let mst = t_add(&reg, &mut ga, "sparq/util/gain", 400.0);
        let wid = t_wire(&mut ga, (sine, 0), (mst, 0));
        ga.op_set_trim(wid, Some(WireTrim { amp: 0.42, offset: 0.0 })).unwrap();
        let (mut ex_a, m_a, _map_a, gains_a) = build_with_map_at(&ga, mst, &reg, cfg).unwrap();
        assert_eq!(gains_a.len(), 1, "one trimmed audio wire synthesises exactly one gain");
        let hash_a = t_render_hash(&mut ex_a, m_a, &cfg);
        // World B: the hand patch — an explicit util/gain at 0.42 in the same place on the path.
        let mut gb = CanvasGraph::new();
        let sine = t_add(&reg, &mut gb, "sparq/syn/sine", 0.0);
        let mid = t_add(&reg, &mut gb, "sparq/util/gain", 200.0);
        let mst = t_add(&reg, &mut gb, "sparq/util/gain", 400.0);
        gb.op_set_param(mid, 0, 0.42).unwrap();
        t_wire(&mut gb, (sine, 0), (mid, 0));
        t_wire(&mut gb, (mid, 1), (mst, 0));
        let (mut ex_b, m_b, _map_b, gains_b) = build_with_map_at(&gb, mst, &reg, cfg).unwrap();
        assert!(gains_b.is_empty(), "an untrimmed patch synthesises nothing");
        let hash_b = t_render_hash(&mut ex_b, m_b, &cfg);
        assert_eq!(hash_a, hash_b, "the cable node IS the hand-patched gain, sample for sample");
        // …and it is audible against the untrimmed wire (the gate is not vacuous).
        let mut gc = CanvasGraph::new();
        let sine = t_add(&reg, &mut gc, "sparq/syn/sine", 0.0);
        let mst = t_add(&reg, &mut gc, "sparq/util/gain", 400.0);
        t_wire(&mut gc, (sine, 0), (mst, 0));
        let (mut ex_c, m_c, _, _) = build_with_map_at(&gc, mst, &reg, cfg).unwrap();
        assert_ne!(hash_a, t_render_hash(&mut ex_c, m_c, &cfg), "the trim is audible");
    }

    #[test]
    fn an_identity_trim_synthesises_nothing_and_renders_bit_exactly() {
        let reg = registry();
        let cfg = render_config();
        let world = |trim: Option<WireTrim>| -> (String, usize) {
            let mut g = CanvasGraph::new();
            let sine = t_add(&reg, &mut g, "sparq/syn/sine", 0.0);
            let mst = t_add(&reg, &mut g, "sparq/util/gain", 400.0);
            let wid = t_wire(&mut g, (sine, 0), (mst, 0));
            if trim.is_some() {
                g.op_set_trim(wid, trim).unwrap();
            }
            let (mut ex, m, _, gains) = build_with_map_at(&g, mst, &reg, cfg).unwrap();
            (t_render_hash(&mut ex, m, &cfg), gains.len())
        };
        let (clean, gains_clean) = world(None);
        let (identity, gains_id) = world(Some(WireTrim::identity()));
        assert_eq!(gains_clean, 0);
        assert_eq!(gains_id, 0, "identity synthesises NO node — the map is the mechanical claim");
        assert_eq!(clean, identity, "…and renders bit-identical to no trim at all");
    }

    #[test]
    fn a_cv_cable_trim_is_the_set_cv_trim_door() {
        let reg = registry();
        let cfg = render_config();
        // sine → svf (master); lfo → svf's cutoff-mod with a cable node (0.5, 0.25). The
        // svf's `mod` depth rides at 1.0 — its default 0 would silence the cv input and the
        // gate would measure nothing (a quiet lie is still a lie).
        let canvas = |trim: Option<WireTrim>| -> CanvasGraph {
            let mut g = CanvasGraph::new();
            let sine = t_add(&reg, &mut g, "sparq/syn/sine", 0.0);
            let svf = t_add(&reg, &mut g, "sparq/flt/svf", 200.0);
            let lfo = t_add(&reg, &mut g, "sparq/mod/lfo", 400.0);
            g.op_set_param(svf, 3, 1.0).unwrap();
            t_wire(&mut g, (sine, 0), (svf, 0));
            let w = t_wire(&mut g, (lfo, 1), (svf, 2)); // lfo cv-out (1) → cutoff-mod (2)
            if let Some(t) = trim {
                g.op_set_trim(w, Some(t)).unwrap();
            }
            g
        };
        let svf_id = |g: &CanvasGraph| {
            g.nodes().iter().find(|n| n.spec.module_id == "sparq/flt/svf").unwrap().id
        };
        // World A: the bridge voices the trim.
        let ga = canvas(Some(WireTrim { amp: 0.5, offset: 0.25 }));
        let (mut ex_a, m_a, _, gains_a) = build_with_map_at(&ga, svf_id(&ga), &reg, cfg).unwrap();
        assert!(gains_a.is_empty(), "a cv trim synthesises no gain — it rides the executor's door");
        let hash_a = t_render_hash(&mut ex_a, m_a, &cfg);
        // World B: the same canvas untrimmed, with the manual `set_cv_trim` call after build —
        // the bridge's record and the hand-turned door must be the same samples (right node,
        // right port, right values).
        let gb = canvas(None);
        let (mut ex_b, m_b, map_b, _) = build_with_map_at(&gb, svf_id(&gb), &reg, cfg).unwrap();
        let svf_k = *map_b.get(&svf_id(&gb)).unwrap();
        ex_b.set_cv_trim(svf_k, 2, 0.5, 0.25).unwrap();
        let hash_b = t_render_hash(&mut ex_b, m_b, &cfg);
        assert_eq!(hash_a, hash_b, "the bridge's cv trim IS the set_cv_trim arithmetic");
        // …and the trim is audible against the clean modulation (not vacuous).
        let (mut ex_c, m_c, _, _) = build_with_map_at(&gb, svf_id(&gb), &reg, cfg).unwrap();
        assert_ne!(hash_a, t_render_hash(&mut ex_c, m_c, &cfg), "the cv trim moves the sound");
    }

    #[test]
    fn a_control_cable_trim_rides_the_param_mod_formula() {
        let reg = registry();
        let cfg = render_config();
        // World A (bridge): sine → gain (master); lfo → gain's `gain` param as a CONTROL
        // wire with a cable node (0.5, 0.25).
        let mut ga = CanvasGraph::new();
        let sine = t_add(&reg, &mut ga, "sparq/syn/sine", 0.0);
        let gain = t_add(&reg, &mut ga, "sparq/util/gain", 200.0);
        let lfo = t_add(&reg, &mut ga, "sparq/mod/lfo", 400.0);
        t_wire(&mut ga, (sine, 0), (gain, 0));
        let cw = match ga.op_add_param_wire(CPortRef::new(lfo, 1), gain, 0) {
            Op::AddWire(w) => w.id,
            _ => unreachable!(),
        };
        ga.op_set_trim(cw, Some(WireTrim { amp: 0.5, offset: 0.25 })).unwrap();
        let (mut ex_a, m_a, _, gains_a) = build_with_map_at(&ga, gain, &reg, cfg).unwrap();
        assert!(gains_a.is_empty(), "a control trim rides the formula, it synthesises nothing");
        let hash_a = t_render_hash(&mut ex_a, m_a, &cfg);
        // World B (executor-level): the same three nodes in the same ids, the same edge, and
        // the r3-style `add_param_mod` with the D15 trim args by hand — the bridge's call,
        // replicated. Hash-identical is the claim: the formula, the args and the order agree.
        let mut kg = KernelGraph::new();
        let ks = kg.add_node(0);
        let kn = kg.add_node(0);
        let kl = kg.add_node(0);
        kg.connect(KernelPortRef::new(ks, 0), KernelPortRef::new(kn, 0), EdgeKind::Plain).unwrap();
        let nb = |id: &str| {
            let r = reg.get(id).unwrap();
            NodeBuild {
                module: r.create(),
                manifest: r.manifest().clone(),
                params: ParamSet::new(1, &t_defaults(&reg, id)).unwrap(),
            }
        };
        let mut ex_b = Executor::build(
            kg,
            vec![
                (ks, nb("sparq/syn/sine")),
                (kn, nb("sparq/util/gain")),
                (kl, nb("sparq/mod/lfo")),
            ],
            cfg,
        )
        .unwrap();
        ex_b.add_param_mod(kn, 0, kl, 1, 0.0, 2.0, 0.5, 0.25).unwrap();
        let hash_b = t_render_hash(&mut ex_b, kn, &cfg);
        assert_eq!(hash_a, hash_b, "the control cable node rides the D15 formula exactly");
    }

    #[test]
    fn the_vca_takes_the_lfo_wire_and_the_patch_modulates() {
        // D7's canvas verdict gate: the manifest moved to unipolar, the matrix did NOT — so
        // the lfo's unipolar word is Compatible at the vca's cv input, and the rendered patch
        // proves the connection carries: the lfo sweeps the vca's gain between 1 and 2
        // around the sine (level default 1, depth default 1 — the manifest's own words).
        let reg = registry();
        let cfg = render_config();
        let mut g = CanvasGraph::new();
        let sine = t_add(&reg, &mut g, "sparq/syn/sine", 0.0);
        let vca = t_add(&reg, &mut g, "sparq/util/vca", 200.0);
        let lfo = t_add(&reg, &mut g, "sparq/mod/lfo", 400.0);
        t_wire(&mut g, (sine, 0), (vca, 0));
        let ctx = ConnectContext::no_adapters(Phase::Zero);
        let verdict = connect::resolve(&mut g, CPortRef::new(lfo, 1), CPortRef::new(vca, 1), &ctx);
        assert!(
            matches!(verdict, ConnectOutcome::Connected { .. }),
            "the lfo→vca wire is compatible now: {verdict:?}"
        );
        if let ConnectOutcome::Connected { op, .. } = verdict {
            g.apply(&op);
        }
        let (mut ex, m, _, _) = build_with_map_at(&g, vca, &reg, cfg).unwrap();
        let mut out = vec![0.0f32; cfg.block_frames * cfg.device_channels];
        let mut peak = 0.0f32;
        for _ in 0..400 {
            ex.render_block(m, &mut out).unwrap();
            peak = peak.max(out.iter().fold(0.0f32, |a, &s| a.max(s.abs())));
        }
        // 0.5-amp sine × gain up to 2 → peaks near 1.0; the glide lags the 1 Hz lfo, so the
        // claim is the window, not the instant (the round-4 handoff's trap #6, honoured).
        assert!(peak > 0.85 && peak <= 1.0 + 1e-3, "the modulation is audible and bounded: {peak}");
    }

    #[test]
    fn a_trimmed_mult_copy_rides_the_collapsed_edge_composed() {
        let reg = registry();
        let cfg = render_config();
        // The declared interaction: lfo → mult dot 0 (feed, trim ×2.0), dot 1 → svf
        // cutoff-mod (copy, trim ×0.5 +0.25). The bus collapses, so BOTH trims ride the one
        // direct edge, composed affinely: v·(2.0×0.5) + (0×0.5 + 0.25) = v·1.0 + 0.25 —
        // hash-identical to the direct wire carrying the composed trim.
        let canvas = |via_bus: bool| -> CanvasGraph {
            let mut g = CanvasGraph::new();
            let sine = t_add(&reg, &mut g, "sparq/syn/sine", 0.0);
            let svf = t_add(&reg, &mut g, "sparq/flt/svf", 200.0);
            let lfo = t_add(&reg, &mut g, "sparq/mod/lfo", 400.0);
            g.op_set_param(svf, 3, 1.0).unwrap(); // mod depth 1: the cv word is audible
            t_wire(&mut g, (sine, 0), (svf, 0));
            if via_bus {
                let mult = t_add(&reg, &mut g, "sparq/util/mult", 600.0);
                let feed = t_wire(&mut g, (lfo, 1), (mult, 0));
                g.op_set_trim(feed, Some(WireTrim { amp: 2.0, offset: 0.0 })).unwrap();
                let copy = t_wire(&mut g, (mult, 1), (svf, 2));
                g.op_set_trim(copy, Some(WireTrim { amp: 0.5, offset: 0.25 })).unwrap();
            } else {
                let direct = t_wire(&mut g, (lfo, 1), (svf, 2));
                g.op_set_trim(direct, Some(WireTrim { amp: 1.0, offset: 0.25 })).unwrap();
            }
            g
        };
        let svf_id = |g: &CanvasGraph| {
            g.nodes().iter().find(|n| n.spec.module_id == "sparq/flt/svf").unwrap().id
        };
        let ga = canvas(true);
        let (mut ex_a, m_a, _, gains_a) = build_with_map_at(&ga, svf_id(&ga), &reg, cfg).unwrap();
        assert!(gains_a.is_empty(), "cv copies ride set_cv_trim, never a gain");
        let hash_a = t_render_hash(&mut ex_a, m_a, &cfg);
        let gb = canvas(false);
        let (mut ex_b, m_b, _, _) = build_with_map_at(&gb, svf_id(&gb), &reg, cfg).unwrap();
        let hash_b = t_render_hash(&mut ex_b, m_b, &cfg);
        assert_eq!(hash_a, hash_b, "the composed trims ride the collapsed edge exactly");
        // Not vacuous: the composition is audible against the same bus with no cable nodes.
        let mut gc = CanvasGraph::new();
        let sine = t_add(&reg, &mut gc, "sparq/syn/sine", 0.0);
        let svf = t_add(&reg, &mut gc, "sparq/flt/svf", 200.0);
        let lfo = t_add(&reg, &mut gc, "sparq/mod/lfo", 400.0);
        let mult = t_add(&reg, &mut gc, "sparq/util/mult", 600.0);
        gc.op_set_param(svf, 3, 1.0).unwrap();
        t_wire(&mut gc, (sine, 0), (svf, 0));
        t_wire(&mut gc, (lfo, 1), (mult, 0));
        t_wire(&mut gc, (mult, 1), (svf, 2));
        let (mut ex_c, m_c, _, _) = build_with_map_at(&gc, svf_id(&gc), &reg, cfg).unwrap();
        assert_ne!(hash_a, t_render_hash(&mut ex_c, m_c, &cfg), "the composition moves the sound");
    }
}
