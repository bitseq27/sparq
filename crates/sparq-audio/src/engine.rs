//! The boundary-swap engine (WO-008 task 4): one live executor, one staged replacement, and a
//! single moment where they meet — the block boundary.
//!
//! ADR-009 decision 3, in the shape this project can prove today: *the control thread builds a
//! complete new graph, then swaps at a block boundary; the audio thread never observes a
//! half-built graph.* Building is pure control-side work ([`Executor::build`] touches nothing
//! live); the swap is `Option::take` at the top of [`Engine::render_block`] — before a single
//! sample of the new block exists, after every sample of the old one. The retired executor
//! drops at that same point, and `ExecNode`'s `Drop` deactivates its modules: retirement is
//! deterministic, not deferred, because in this shape nothing else can still be inside the old
//! executor.
//!
//! **The honest scope of v0, declared:** `Engine` is single-owner. A live rig hands the staged
//! executor from the control thread to the audio thread through the kernel's hot-swap primitive
//! (decision 3's literal pointer swap, with an epoch-based grace period before retirement) — that
//! primitive is allowlisted-`unsafe` kernel work and gets its own increment with its own proofs;
//! until then the mutation contract is proven where it can be measured: the stress test
//! (`tests/mutation_stress.rs`) interleaves 10 000 control-side mutations with rendered blocks
//! through this engine, allocation-gated and hash-deterministic, which is exactly the boundary
//! semantics the swap guarantees minus the concurrency the primitive will carry. The HAL's
//! loaded soak (200 modules, 30 min) remains the device-side acceptance.
//!
//! What the swap does NOT carry: module state. A swapped-in module starts from its
//! `prepare`/`activate` state — carrying state across a patch change is the state protocol's job
//! (schema'd, journaled — WO-011), not the swap's. What it DOES carry: the transport clock and
//! the block count ([`Executor::inherit_runtime`]), because the timeline belongs to the stream,
//! not to the patch.

use crate::executor::{ExecError, Executor};
use sparq_kernel::graph::NodeId;

/// A live executor with a staging slot for its replacement.
#[derive(Debug)]
pub struct Engine {
    live: Executor,
    staged: Option<Executor>,
    swaps: u64,
    superseded: u64,
}

impl Engine {
    /// An engine around a freshly built executor.
    #[must_use]
    pub fn new(live: Executor) -> Self {
        Self { live, staged: None, swaps: 0, superseded: 0 }
    }

    /// Stage a replacement for the next block boundary (control side).
    ///
    /// The successor adopts the live executor's transport clock and block count *here*, at
    /// staging time, so the swap itself is a pure handover. Staging twice before a boundary
    /// retires the first successor unused (counted by [`Engine::superseded_stages`]) — the
    /// timeline never skips: what renders next is always exactly one boundary away.
    pub fn stage(&mut self, mut next: Executor) {
        next.inherit_runtime(&self.live);
        if self.staged.is_some() {
            self.superseded += 1;
        }
        self.staged = Some(next);
    }

    /// Render one block: apply the staged swap first (if any), then render.
    ///
    /// The swap point is the boundary ADR-009 d3 names — no sample of the new block is computed
    /// against the old graph, and no sample of the old block ever met the new one. The old
    /// executor drops here (modules deactivated through `ExecNode::Drop`); on a single-owner
    /// engine that retirement point is exact.
    ///
    /// # Errors
    /// Whatever [`Executor::render_block`] reports — note that a swap which removed the master
    /// node surfaces here as `NoSuchNode`, in words, rather than rendering silence: staging a
    /// patch without the node the listener hears is a control-side bug and is reported as one.
    pub fn render_block(&mut self, master: NodeId, out: &mut [f32]) -> Result<(), ExecError> {
        if let Some(next) = self.staged.take() {
            self.live = next;
            self.swaps += 1;
        }
        self.live.render_block(master, out)
    }

    /// The executor that will render the next block (after any staged swap).
    #[must_use]
    pub fn live(&self) -> &Executor {
        &self.live
    }

    /// Mutable access to the live executor — control-side reads and edits (`set_params`,
    /// `clear_auto_bypass`) between blocks. Never called from inside a render.
    pub fn live_mut(&mut self) -> &mut Executor {
        &mut self.live
    }

    /// Whether a replacement is waiting for the next boundary.
    #[must_use]
    pub fn is_staged(&self) -> bool {
        self.staged.is_some()
    }

    /// How many boundary swaps have happened.
    #[must_use]
    pub fn swaps(&self) -> u64 {
        self.swaps
    }

    /// How many staged successors were replaced before ever going live (each one retired
    /// unused, its modules deactivated without a single block — counted, never silent).
    #[must_use]
    pub fn superseded_stages(&self) -> u64 {
        self.superseded
    }

    /// Blocks rendered across all swaps — the clock the swap inherits, made observable.
    #[must_use]
    pub fn blocks_rendered(&self) -> u64 {
        self.live.blocks_rendered()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::executor::{ExecConfig, Executor, NodeBuild};
    use sparq_kernel::graph::Graph;
    use sparq_module_api::manifest::{
        Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
    };
    use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
    use sparq_module_api::params::ParamSet;

    /// A DC source: out = param(0), mono.
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

    fn dc_manifest() -> sparq_module_api::manifest::ValidatedManifest {
        let m = Manifest {
            identity: Identity {
                id: Some("sparq/test/dc".into()),
                version: Some("0.1.0".into()),
                host_api_min: Some(1),
                host_api_max: Some(1),
                display_name: Some("DC".into()),
                summary: Some("test source".into()),
                authors: vec!["sparq".into()],
                license: Some("MIT".into()),
            },
            classification: Classification {
                category: Some("utility/dc".into()),
                top: Some("util".into()),
                kind: Some("source".into()),
                tier: Some("t1".into()),
                stability: Some("stable".into()),
            },
            state: StateDecl {
                schema_id: Some("sparq/test/dc/state".into()),
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
        let mut m = m;
        m.ports = vec![PortSpec {
            id: Some("out".into()),
            name: Some("OUT".into()),
            direction: Some("out".into()),
            port_type: Some("audio".into()),
            channel_set: Some("mono".into()),
            ..PortSpec::default()
        }];
        m.params = vec![ParamSpec {
            id: Some("level".into()),
            name: Some("level".into()),
            kind: Some("float".into()),
            unit: Some("ratio".into()),
            min: Some(-2.0),
            max: Some(2.0),
            default: Some(1.0),
            ..ParamSpec::default()
        }];
        m.validate().expect("the test manifest is valid")
    }

    fn dc_executor(level: f32) -> Executor {
        let mut g = Graph::new();
        let n = g.add_node(0);
        Executor::build(
            g,
            vec![(
                n,
                NodeBuild {
                    module: Box::new(Dc),
                    manifest: dc_manifest(),
                    params: ParamSet::new(0, &[level]).unwrap(),
                },
            )],
            ExecConfig::new(48_000, 64, 1),
        )
        .unwrap()
    }

    fn peak(out: &[f32]) -> f32 {
        out.iter().fold(0.0f32, |a, &s| a.max(s.abs()))
    }

    #[test]
    fn a_staged_swap_takes_effect_at_exactly_the_next_boundary() {
        let mut eng = Engine::new(dc_executor(0.25));
        let mut out = vec![0.0f32; 64];
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(peak(&out), 0.25);

        eng.stage(dc_executor(0.75));
        assert!(eng.is_staged());
        // the block AFTER staging still renders the OLD patch until the boundary is crossed…
        // (staging happened between blocks, so the very next render IS the boundary)
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(peak(&out), 0.75, "the swap applied at the boundary");
        assert_eq!(eng.swaps(), 1);
        assert!(!eng.is_staged());
    }

    #[test]
    fn the_transport_clock_survives_the_swap() {
        let mut eng = Engine::new(dc_executor(0.5));
        let mut out = vec![0.0f32; 64];
        for _ in 0..10 {
            eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        }
        eng.stage(dc_executor(0.5));
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(eng.blocks_rendered(), 11, "the count continues across the swap");
        for _ in 0..5 {
            eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        }
        assert_eq!(eng.blocks_rendered(), 16);
    }

    #[test]
    fn a_double_stage_supersedes_the_first_successor_and_counts_it() {
        let mut eng = Engine::new(dc_executor(0.25));
        eng.stage(dc_executor(0.5));
        eng.stage(dc_executor(0.75));
        assert_eq!(eng.superseded_stages(), 1);
        let mut out = vec![0.0f32; 64];
        eng.render_block(sparq_kernel::graph::NodeId(0), &mut out).unwrap();
        assert_eq!(peak(&out), 0.75, "the LAST staged successor is the one that goes live");
        assert_eq!(eng.swaps(), 1);
    }

    #[test]
    fn a_swap_that_drops_the_master_is_reported_not_silenced() {
        // Live: two independent DCs, master = node 1. Successor: only node 0. After the swap,
        // asking for node 1 must refuse in words (NoSuchNode), never render silence.
        let mut g_live = Graph::new();
        let _a = g_live.add_node(0);
        let b = g_live.add_node(0);
        let builds = |g: &Graph| {
            g.nodes()
                .iter()
                .map(|n| {
                    (
                        n.id,
                        NodeBuild {
                            module: Box::new(Dc),
                            manifest: dc_manifest(),
                            params: ParamSet::new(0, &[0.5]).unwrap(),
                        },
                    )
                })
                .collect::<Vec<_>>()
        };
        let live = Executor::build(g_live.clone(), builds(&g_live), ExecConfig::new(48_000, 64, 1))
            .unwrap();
        let mut g_next = Graph::new();
        let _n = g_next.add_node(0);
        let next = Executor::build(g_next.clone(), builds(&g_next), ExecConfig::new(48_000, 64, 1))
            .unwrap();
        let mut eng = Engine::new(live);
        let mut out = vec![0.0f32; 64];
        eng.render_block(b, &mut out).unwrap(); // master node 1 renders fine pre-swap
        eng.stage(next);
        let err = eng.render_block(b, &mut out).unwrap_err();
        assert!(matches!(err, ExecError::Graph(_)), "{err}");
        assert_eq!(eng.swaps(), 1, "the swap happened; the refusal is the new graph's truth");
    }
}
