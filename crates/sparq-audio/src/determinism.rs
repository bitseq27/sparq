//! The determinism harness (WO-008 task 7): the same script renders the same samples — bit for
//! bit, hash-equal.
//!
//! ADR-007's promise, made into a reusable instrument instead of a one-off test: a **script** is
//! a seed and a mutation count; the harness plays it against the registry — a seeded
//! `SplitMix64` drives a mutation schedule (add / remove / connect / disconnect / set-param /
//! set-latency) over a kernel [`Graph`], every accepted mutation rebuilds an [`Executor`] and
//! stages it through the [`Engine`] (the task-4 boundary swap, exercised as the mutation path it
//! exists for), every block is rendered and accumulated, and the run's evidence is the FNV-1a
//! hash of every sample plus the counters.
//!
//! Two runs of one script are **required** to produce identical hashes: same seed ⇒ same
//! mutation schedule ⇒ same graphs in the same order ⇒ same swaps at the same boundaries ⇒ same
//! samples. Anything that breaks that chain — a nondeterministic order, a HashMap iteration
//! leaking into audio, an unseeded RNG, a swap landing a block late — fails the harness instead
//! of drifting into a heisenbug on stage.
//!
//! The harness is also the mutation **stress** vehicle (the acceptance's 10 000-iteration run
//! lives in `tests/mutation_stress.rs`, allocation-gated through the kernel's counting
//! allocator): refused mutations are counted, never hidden, and a refusal leaves no trace — the
//! live executor keeps rendering, exactly as a refused canvas gesture does.

use std::collections::HashMap;

use crate::engine::Engine;
use crate::executor::{ExecConfig, Executor, NodeBuild};
use crate::hash::fnv1a64_f32;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_kernel::seed::SplitMix64;
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

/// The three built-in module ids the script mutates over — the registry's own vocabulary (#58:
/// the harness never offers a module that is not installed).
pub const KIND_SINE: &str = "sparq/syn/sine";
/// See [`KIND_SINE`].
pub const KIND_GAIN: &str = "sparq/util/gain";
/// See [`KIND_SINE`].
pub const KIND_RMS: &str = "sparq/ana/rms";

/// A script: a seed and a length. Everything else is derived, deterministically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Script {
    /// The RNG seed — the whole schedule is a pure function of it.
    pub seed: u64,
    /// How many mutations to attempt (one rendered block per attempt, plus the opening block).
    pub mutations: usize,
    /// Arm the kernel's allocation counter around every rendered block and record violations.
    /// Only meaningful in a binary whose global allocator is the counting one; elsewhere the
    /// windows open and close inert.
    pub alloc_gate: bool,
}

impl Script {
    /// A script with the allocation gate off (determinism runs).
    #[must_use]
    pub const fn new(seed: u64, mutations: usize) -> Self {
        Self { seed, mutations, alloc_gate: false }
    }
}

/// What one scripted run measured. Every field is evidence: the hash is the determinism claim,
/// the counters prove the run actually exercised the machinery it claims to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScriptRun {
    /// FNV-1a over every rendered sample, in order (ADR-007's golden hash, same function).
    pub hash: u64,
    /// Blocks rendered (mutations + the opening block).
    pub blocks: usize,
    /// Boundary swaps performed.
    pub swaps: u64,
    /// Mutations that were refused (kernel refusal or executor build failure) — each one left
    /// the live patch untouched and the render going.
    pub refused: u64,
    /// Staged successors superseded before going live (can only happen if the harness ever
    /// stages twice per block — recorded so "never" stays measured, not assumed).
    pub superseded: u64,
    /// Blocks whose render allocated (only nonzero where `alloc_gate` met a counting allocator).
    pub alloc_violations: u64,
    /// Nodes in the final graph.
    pub final_nodes: usize,
    /// Edges in the final graph.
    pub final_edges: usize,
    /// The final graph's slowest path latency, samples (task 5's readout, along for the ride).
    pub max_path_latency: u32,
}

/// The control-side world the script mutates: the kernel graph plus what the graph cannot know
/// (each node's module kind and gain value). Node ids are the kernel's — stable, never reused.
pub struct World {
    /// The structural graph (kernel vocabulary): stable, never-reused node ids.
    pub graph: Graph,
    /// Each node's module kind — what the graph cannot know.
    pub kinds: HashMap<NodeId, &'static str>,
    /// Each gain node's scripted param-0 value.
    pub gains: HashMap<NodeId, f32>,
}

impl World {
    /// The reference chain the scripts mutate over: sine → gain → { rms, spare } — the spare
    /// gain fed from the chain gain's output. The spare USED to sit unwired: legal under v0's
    /// unenforced `required`, illegal under increment 7's host-side enforcement, so the world
    /// wires it. Fed from the chain gain (not the sine) on purpose: the spare is the world's
    /// initial master (highest-id audio-out node), so every retune mutation of the chain gain
    /// reaches the rendered samples and the script's hash stays sensitive to parameter edits.
    /// The spare stays a gain — the retune target, a master candidate and a connect
    /// destination, the reasons it exists. A disconnect mutation can still bare it, and the
    /// candidate's rebuild is then REFUSED and counted: the enforcement exercising exactly the
    /// path this harness exists to stress.
    #[must_use]
    pub fn initial() -> Self {
        let mut graph = Graph::new();
        let sine = graph.add_node(0);
        let gain = graph.add_node(0);
        let rms = graph.add_node(0);
        let spare = graph.add_node(0);
        // sine.out(0) → gain.in(0); gain.out(1) → rms.in(0); gain.out(1) → spare.in(0) — the
        // reference chain plus the fed spare (a plain audio fan-out from the chain gain), so
        // early mutations have somewhere to go and the initial world is a legal patch under
        // the required-input enforcement.
        let _ = graph.connect(PortRef::new(sine, 0), PortRef::new(gain, 0), EdgeKind::Plain);
        let _ = graph.connect(PortRef::new(gain, 1), PortRef::new(rms, 0), EdgeKind::Plain);
        let _ = graph.connect(PortRef::new(gain, 1), PortRef::new(spare, 0), EdgeKind::Plain);
        let mut kinds = HashMap::new();
        kinds.insert(sine, KIND_SINE);
        kinds.insert(gain, KIND_GAIN);
        kinds.insert(rms, KIND_RMS);
        kinds.insert(spare, KIND_GAIN);
        Self { graph, kinds, gains: HashMap::new() }
    }

    /// The master: the highest-id node with an audio output (sine or gain — rms's output is
    /// cv). Node 0 (the sine) is never removed, so this always exists.
    pub fn master(&self) -> NodeId {
        self.graph
            .nodes()
            .iter()
            .filter(|n| matches!(self.kinds.get(&n.id).copied(), Some(KIND_SINE) | Some(KIND_GAIN)))
            .map(|n| n.id)
            .max()
            .unwrap_or(NodeId(0))
    }

    /// Audio-out port index by kind (sine: 0; gain: 1), or `None` for kinds with no audio out.
    fn out_port(kind: &str) -> Option<u32> {
        match kind {
            KIND_SINE => Some(0),
            KIND_GAIN => Some(1),
            _ => None,
        }
    }

    /// Audio-in port index by kind (gain: 0; rms: 0), or `None` for source-only kinds.
    fn in_port(kind: &str) -> Option<u32> {
        match kind {
            KIND_GAIN | KIND_RMS => Some(0),
            _ => None,
        }
    }

    /// One seeded mutation attempt, returning the CANDIDATE world without committing it.
    ///
    /// Split out of [`run_script`] for WO-008 increment 5 so the cross-thread stress can drive
    /// the same schedule from a control thread. The split is behaviour-preserving on purpose:
    /// the caller commits the candidate only after the executor build succeeds, exactly as the
    /// inline code did, so a refusal (mutation-level OR build-level) still leaves the live
    /// world untouched and the RNG draw order identical — the stress hash does not move.
    ///
    /// # Errors
    /// A sentence naming the mutation-level refusal (node cap, nothing removable, no
    /// connectable pair, a kernel cycle/duplicate refusal, ...). Build-level refusals are the
    /// caller's to count, as before.
    pub fn candidate_mutation(&self, rng: &mut SplitMix64) -> Result<World, String> {
        let roll = rng.next_below(100);
        let mut candidate = self.graph.clone();
        let mut kinds = self.kinds.clone();
        let mut gains = self.gains.clone();
        let accepted: Result<(), String> = match roll {
            // add a module (capped at 8 nodes — the stress graph stays small so 10 000
            // iterations cost seconds; scale is the HAL soak's job)
            r if r < 20 => {
                if candidate.node_count() >= 8 {
                    Err("node cap".into())
                } else {
                    let kind = [KIND_SINE, KIND_GAIN, KIND_RMS][rng.next_index(3)];
                    let id = candidate.add_node(0);
                    kinds.insert(id, kind);
                    Ok(())
                }
            },
            // remove a module (never node 0 — the sine is the floor the master rule stands on)
            r if r < 40 => {
                let victims: Vec<NodeId> =
                    candidate.nodes().iter().map(|n| n.id).filter(|id| id.0 != 0).collect();
                if victims.is_empty() {
                    Err("nothing removable".into())
                } else {
                    let id = victims[rng.next_index(victims.len())];
                    candidate.remove_node(id).map(|_| ()).map_err(|e| format!("{e}"))?;
                    kinds.remove(&id);
                    gains.remove(&id);
                    Ok(())
                }
            },
            // connect two random compatible ports (the kernel refuses duplicates, self-loops
            // and cycles — the refusals are the point, they prove the gate under churn)
            r if r < 70 => {
                let srcs: Vec<NodeId> = candidate
                    .nodes()
                    .iter()
                    .filter(|n| kinds.get(&n.id).and_then(|k| World::out_port(k)).is_some())
                    .map(|n| n.id)
                    .collect();
                let dsts: Vec<NodeId> = candidate
                    .nodes()
                    .iter()
                    .filter(|n| kinds.get(&n.id).and_then(|k| World::in_port(k)).is_some())
                    .map(|n| n.id)
                    .collect();
                if srcs.is_empty() || dsts.is_empty() {
                    Err("no connectable pair".into())
                } else {
                    let src = srcs[rng.next_index(srcs.len())];
                    let dst = dsts[rng.next_index(dsts.len())];
                    let sp = World::out_port(kinds[&src]).unwrap_or(0);
                    let dp = World::in_port(kinds[&dst]).unwrap_or(0);
                    candidate
                        .connect(PortRef::new(src, sp), PortRef::new(dst, dp), EdgeKind::Plain)
                        .map(|_| ())
                        .map_err(|e| format!("{e}"))
                }
            },
            // disconnect a random edge
            r if r < 80 => {
                let ids: Vec<_> = candidate.edges().iter().map(|e| e.id).collect();
                if ids.is_empty() {
                    Err("no edges".into())
                } else {
                    let id = ids[rng.next_index(ids.len())];
                    candidate.disconnect(id).map(|_| ()).map_err(|e| format!("{e}"))
                }
            },
            // retune a random gain (param 0 of every gain node). The candidate list walks the
            // GRAPH's node Vec, never the kinds HashMap — HashMap iteration order is per-map
            // random, and the stress replay caught exactly that: same seed, different choice,
            // different samples. Every collection the RNG indexes into must be deterministically
            // ordered; this is the harness earning its keep on day one.
            r if r < 90 => {
                let gs: Vec<NodeId> = candidate
                    .nodes()
                    .iter()
                    .map(|n| n.id)
                    .filter(|id| kinds.get(id).copied() == Some(KIND_GAIN))
                    .collect();
                if gs.is_empty() {
                    Err("no gain node".into())
                } else {
                    let id = gs[rng.next_index(gs.len())];
                    gains.insert(id, rng.range_f64(0.0, 1.0) as f32);
                    Ok(())
                }
            },
            // re-declare a random node's latency (exercises the task-5 cache invalidation
            // under churn: a data edit that must never serve a stale map)
            _ => {
                let ids: Vec<NodeId> = candidate.nodes().iter().map(|n| n.id).collect();
                if ids.is_empty() {
                    Err("no nodes".into())
                } else {
                    let id = ids[rng.next_index(ids.len())];
                    candidate.set_latency(id, rng.next_below(257)).map_err(|e| format!("{e}"))
                }
            },
        };
        accepted.map(|()| World { graph: candidate, kinds, gains })
    }

    /// Build the executor for this world from the registry: manifest defaults for every param,
    /// with the scripted gain value overriding param 0 of gain nodes.
    pub fn build(&self, registry: &Registry, cfg: ExecConfig) -> Result<Executor, String> {
        let mut builds = Vec::with_capacity(self.graph.node_count());
        for n in self.graph.nodes() {
            let kind = self.kinds.get(&n.id).copied().ok_or("a node without a kind")?;
            let reg = registry
                .get(kind)
                .ok_or_else(|| format!("module `{kind}` is not in the registry (#58)"))?;
            let mut values: Vec<f32> = reg
                .manifest()
                .manifest()
                .params
                .iter()
                .map(|p| p.default.unwrap_or(0.0) as f32)
                .collect();
            if kind == KIND_GAIN {
                if let Some(g) = self.gains.get(&n.id) {
                    if let Some(v) = values.first_mut() {
                        *v = *g;
                    }
                }
            }
            let params = ParamSet::new(0, &values)
                .ok_or_else(|| format!("module `{kind}` exceeds MAX_PARAMS"))?;
            builds.push((
                n.id,
                NodeBuild { module: reg.create(), manifest: reg.manifest().clone(), params },
            ));
        }
        Executor::build(self.graph.clone(), builds, cfg).map_err(|e| e.to_string())
    }
}

/// Play `script` against `registry`, rendering every block into `out` (one block's worth:
/// `cfg.block_frames × cfg.device_channels` floats). Returns the run's evidence.
///
/// # Errors
/// A sentence when the registry lacks a built-in module or the INITIAL build fails — both are
/// build bugs, not script states. Per-mutation refusals are counted in [`ScriptRun::refused`],
/// never errors.
pub fn run_script(
    registry: &Registry,
    cfg: ExecConfig,
    script: &Script,
    out: &mut [f32],
) -> Result<ScriptRun, String> {
    let frames = cfg.block_frames;
    if out.len() != frames * cfg.device_channels {
        return Err(format!(
            "out holds {} floats, need {} (block_frames × device_channels)",
            out.len(),
            frames * cfg.device_channels
        ));
    }
    let mut world = World::initial();
    let first = world.build(registry, cfg)?;
    let mut engine = Engine::new(first);

    let mut samples: Vec<f32> =
        Vec::with_capacity((script.mutations + 1) * frames * cfg.device_channels);
    let mut rng = SplitMix64::new(script.seed);
    let mut refused = 0u64;
    let mut alloc_violations = 0u64;

    for _ in 0..=script.mutations {
        // ---- render one block (the opening pass has no mutation before it)
        let master = world.master();
        if script.alloc_gate {
            let base = start_counting();
            engine.render_block(master, out).map_err(|e| format!("render failed: {e}"))?;
            let made = allocation_count().saturating_sub(base);
            stop_counting();
            alloc_violations += made;
        } else {
            engine.render_block(master, out).map_err(|e| format!("render failed: {e}"))?;
        }
        samples.extend_from_slice(out);

        // ---- attempt one mutation (skipped on the final block, which only renders)
        if samples.len() >= (script.mutations + 1) * frames * cfg.device_channels {
            break;
        }
        // One mutation attempt, then (if it was accepted) a build and a stage. The candidate
        // world is committed only when the build succeeds, so a refusal leaves no trace.
        let next_world = match world.candidate_mutation(&mut rng) {
            Ok(w) => w,
            Err(_) => {
                refused += 1;
                continue; // a refusal leaves no trace — the live patch renders on
            },
        };
        match next_world.build(registry, cfg) {
            Ok(next) => {
                engine.stage(next);
                world = next_world;
            },
            Err(_e) => refused += 1, // executor-level refusal: same contract, live renders on
        }
    }

    let bf = u32::try_from(frames).unwrap_or(u32::MAX);
    let max_path_latency = world.graph.max_path_latency(bf).unwrap_or(0);
    Ok(ScriptRun {
        hash: fnv1a64_f32(&samples),
        blocks: samples.len() / (frames * cfg.device_channels),
        swaps: engine.swaps(),
        refused,
        superseded: engine.superseded_stages(),
        alloc_violations,
        final_nodes: world.graph.node_count(),
        final_edges: world.graph.edge_count(),
        max_path_latency,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::executor::ExecConfig;
    use crate::modules::register_builtins;

    fn registry() -> Registry {
        let mut r = Registry::new();
        register_builtins(&mut r).unwrap();
        r
    }

    fn run(seed: u64, mutations: usize) -> ScriptRun {
        let reg = registry();
        let cfg = ExecConfig::new(48_000, 64, 2);
        let mut out = vec![0.0f32; 64 * 2];
        run_script(&reg, cfg, &Script::new(seed, mutations), &mut out).unwrap()
    }

    #[test]
    fn the_same_script_renders_bit_identical_samples() {
        // THE task-7 claim, through the full mutation machinery: same seed ⇒ same schedule ⇒
        // same graphs, swaps and boundaries ⇒ same hash. Twice, because once is an accident.
        let a = run(7, 200);
        let b = run(7, 200);
        assert_eq!(a.hash, b.hash, "one script, one sound");
        assert_eq!(a.blocks, b.blocks);
        assert_eq!(a.swaps, b.swaps);
        assert_eq!(a.refused, b.refused);
        assert_eq!(a.final_nodes, b.final_nodes);
    }

    #[test]
    fn different_scripts_diverge() {
        let a = run(7, 200);
        let b = run(99, 200);
        assert_ne!(a.hash, b.hash, "different seeds must produce different music");
    }

    #[test]
    fn a_script_actually_exercises_the_machinery_it_claims() {
        let r = run(7, 400);
        assert_eq!(r.blocks, 401, "one block per mutation plus the opening block");
        assert!(r.swaps > 50, "mutations were staged and swapped: {}", r.swaps);
        assert!(r.refused > 0, "random wiring MUST hit the kernel's refusals — a stress test that never gets refused is not stressing the gate");
        assert_eq!(r.superseded, 0, "one stage per block: nothing is superseded");
        assert!(r.final_nodes >= 1 && r.final_nodes <= 8);
    }

    #[test]
    fn the_no_mutation_script_is_the_reference_chain() {
        let r = run(1, 0);
        assert_eq!(r.blocks, 1);
        assert_eq!(r.swaps, 0);
        assert_eq!(r.refused, 0);
        assert_eq!(r.final_nodes, 4, "sine → gain → rms + the spare fed from the chain gain");
        assert_eq!(r.final_edges, 3, "the world is a legal patch under the required enforcement");
    }

    #[test]
    fn a_mis_sized_output_buffer_is_refused_in_words() {
        let reg = registry();
        let cfg = ExecConfig::new(48_000, 64, 2);
        let mut out = vec![0.0f32; 10];
        let err = run_script(&reg, cfg, &Script::new(1, 4), &mut out).unwrap_err();
        assert!(err.contains("need 128"), "{err}");
    }
}
