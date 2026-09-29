//! WO-008 increment 5 acceptance: the cross-thread engine — ADR-009 decision 3's hot swap with
//! TWO REAL THREADS, decision 8's meter publication, and the command ring's between-block edits.
//!
//! What is proven here, and what is not, in words:
//! * **Proven (sandbox):** a staged patch crosses to the audio thread and goes live at exactly
//!   one boundary; the transport clock survives the crossing; commands apply to the patch that
//!   renders the block they precede, and a bad command is refused-and-counted; meters arrive on
//!   the control side per block per node, with an absent reader turning into counted refusals
//!   instead of memory growth; the audio thread allocates NOTHING across boundaries, swaps and
//!   publications; and the acceptance — **10 000 seeded random mutations staged from a control
//!   thread while the audio thread renders continuously** — with every rendered block
//!   succeeding, every retirement reclaimed and dropped exactly once, and the ledger balanced
//!   to the payload.
//! * **NOT proven here (device-side, declared):** the PACED zero-xrun half of the allowlist's
//!   "mutation-while-playing" test — real-time against a live HAL is the loaded soak's job
//!   (200 modules, 30 min, SATURN), exactly as the WO schedules it. This binary removes every
//!   excuse that soak could find that is not concurrency-against-a-device-clock itself.
//!
//! Unlike `mutation_stress.rs`, nothing here hashes to a golden: WHICH block a mutation lands
//! on is a scheduling fact, and a golden over scheduling would be a flake generator. The
//! determinism claim stays where it is provable (the single-owner stress, hash
//! `7bb06379bd6845e5` since increment 7's required-input enforcement moved the baseline on
//! purpose — `b42068ec7b206789` before it); the cross-thread claims are counters, ledgers and
//! orderings — every one
//! of them exact, none of them timing-dependent, because staging is paced: a patch is consumed
//! (swapped or superseded) before the next is offered.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// The stress run prints one evidence line for the log — stdout is the test binary's report
// channel, same as the gates' other measured numbers.
#![allow(clippy::print_stdout)]

// Defect #66's discipline: the counting allocator's global counter is fed by whichever thread is
// armed, so every test in this binary that arms a window serialises on one lock — two armed
// audio threads would corrupt each other's deltas.
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
#[allow(clippy::disallowed_types)] // harness-side serialisation, not audio path (defect #66)
use std::sync::{Mutex, MutexGuard};

use sparq_audio::determinism::World;
use sparq_audio::engine::{LivePatch, MeterUpdate, SharedEngine};
use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_kernel::alloc::CountingAllocator;
use sparq_kernel::graph::{Graph, NodeId};
use sparq_kernel::seed::SplitMix64;
use sparq_module_api::manifest::{
    Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

#[allow(clippy::disallowed_types)] // see the use statement (defect #66)
static AUDIT_LOCK: Mutex<()> = Mutex::new(());

fn audit_serialised() -> MutexGuard<'static, ()> {
    AUDIT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn registry() -> Registry {
    let mut r = Registry::new();
    sparq_audio::modules::register_builtins(&mut r).unwrap();
    r
}

/// A generous spin cap so a dead audio thread fails the test in words instead of hanging CI.
const SPIN_CAP: u64 = 500_000_000;

fn spin_until(mut done: impl FnMut() -> bool, what: &str) {
    let mut spins = 0u64;
    while !done() {
        spins += 1;
        if spins > SPIN_CAP {
            panic!("gave up waiting for {what} after {spins} spins — the other side is stuck");
        }
        std::hint::spin_loop();
    }
}

// --------------------------------------------------------------------------- probe modules

/// A DC source: out = param(0). Exact peaks make the boundary assertions equalities.
struct DcProbe;
impl Module for DcProbe {
    fn id(&self) -> &str {
        "sparq/test/dc-probe"
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
        Err(ModuleError::Message("dc-probe takes no messages"))
    }
}

/// Writes the block's musical tick into its output — the command ring's proof that a position
/// set on the control side reaches the module context on the audio side.
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
        let tick = ctx.block.tick as f32;
        let out = ctx.output();
        if let Some(s) = out.first_mut() {
            *s = tick;
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("tick-probe takes no messages"))
    }
}

fn probe_manifest(id: &str, with_param: bool) -> sparq_module_api::manifest::ValidatedManifest {
    let m = Manifest {
        identity: Identity {
            id: Some(id.into()),
            version: Some("0.1.0".into()),
            host_api_min: Some(1),
            host_api_max: Some(1),
            display_name: Some("Probe".into()),
            summary: Some("test probe".into()),
            authors: vec!["sparq".into()],
            license: Some("MIT".into()),
        },
        classification: Classification {
            category: Some("utility/probe".into()),
            top: Some("util".into()),
            kind: Some("source".into()),
            tier: Some("t1".into()),
            stability: Some("stable".into()),
        },
        state: StateDecl { schema_id: Some(format!("{id}/state")), schema_version: Some(1) },
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
    if with_param {
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
    }
    m.validate().expect("the probe manifest is valid")
}

/// A one-node executor: the probe at `level`, mono, 48 kHz/64.
fn probe_executor<M: Module + 'static>(
    module: M,
    id: &str,
    with_param: bool,
    level: f32,
) -> (Executor, NodeId) {
    let mut g = Graph::new();
    let n = g.add_node(0);
    let params = if with_param {
        ParamSet::new(0, &[level]).unwrap()
    } else {
        ParamSet::new(0, &[]).unwrap()
    };
    let ex = Executor::build(
        g,
        vec![(
            n,
            NodeBuild {
                module: Box::new(module),
                manifest: probe_manifest(id, with_param),
                params,
            },
        )],
        ExecConfig::new(48_000, 64, 1),
    )
    .unwrap();
    (ex, n)
}

fn dc_executor(level: f32) -> (Executor, NodeId) {
    probe_executor(DcProbe, "sparq/test/dc-probe", true, level)
}

fn peak(out: &[f32]) -> f32 {
    out.iter().fold(0.0f32, |a, &s| a.max(s.abs()))
}

// --------------------------------------------------------------------------- the handover

#[test]
fn a_staged_patch_crosses_to_the_audio_thread_at_exactly_one_boundary() {
    let (live, master) = dc_executor(0.25);
    let (control, audio_engine) = SharedEngine::new(live, master);
    let staged_once = Arc::new(AtomicBool::new(false));
    let block_one_done = Arc::new(AtomicBool::new(false));

    let audio_thread = {
        let staged_once = Arc::clone(&staged_once);
        let block_one_done = Arc::clone(&block_one_done);
        std::thread::spawn(move || {
            let mut ae = audio_engine;
            let mut out = vec![0.0f32; 64];
            let mut peaks = Vec::new();
            ae.render_block(&mut out).unwrap();
            peaks.push(peak(&out));
            block_one_done.store(true, Ordering::Release);
            // The next render must cross the boundary: spin until the control side has staged.
            while !staged_once.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            ae.render_block(&mut out).unwrap();
            peaks.push(peak(&out));
            let blocks = ae.blocks_rendered();
            let nodes = ae.live().graph().node_count();
            (peaks, blocks, nodes, ae.shutdown())
        })
    };

    spin_until(|| block_one_done.load(Ordering::Acquire), "the audio thread's first block");
    let (next, next_master) = dc_executor(0.75);
    assert!(!control.stage(next, next_master), "an empty staging slot supersedes nothing");
    staged_once.store(true, Ordering::Release);

    let (peaks, blocks, nodes, final_patch) = audio_thread.join().unwrap();
    assert_eq!(
        peaks,
        vec![0.25, 0.75],
        "the old patch rendered until the boundary, the new one from it"
    );
    assert_eq!(blocks, 2, "the transport clock counts across the crossing");
    assert_eq!(nodes, 1, "the final live patch is the staged one");
    assert_eq!(final_patch.master, next_master);

    // The retired patch comes back to the control side — module deactivation runs HERE.
    let retired = control.reclaim().expect("the boundary passed; the retirement is reclaimable");
    assert_eq!(retired.master, master);
    drop(retired);
    let stats = control.stats();
    assert_eq!(stats.swap.swaps, 1);
    assert_eq!(stats.swap.superseded, 0);
    assert_eq!(stats.swap.deferred, 0);
    assert_eq!(stats.swap.retirements, 1);
    assert!(stats.swap.epoch >= 2);
}

#[test]
fn commands_apply_between_blocks_and_a_bad_one_is_refused_and_counted() {
    let (live, master) = dc_executor(0.5);
    let (control, audio_engine) = SharedEngine::new(live, master);
    let phase = Arc::new(AtomicU64::new(0)); // 0: render b1 · 1: cmd sent · 2: done

    let audio_thread = {
        let phase = Arc::clone(&phase);
        std::thread::spawn(move || {
            let mut ae = audio_engine;
            let mut out = vec![0.0f32; 64];
            ae.render_block(&mut out).unwrap();
            let p1 = peak(&out);
            phase.store(1, Ordering::Release);
            while phase.load(Ordering::Acquire) < 2 {
                std::hint::spin_loop();
            }
            ae.render_block(&mut out).unwrap();
            let p2 = peak(&out);
            (p1, p2, ae.shutdown())
        })
    };

    spin_until(|| phase.load(Ordering::Acquire) >= 1, "the audio thread's first block");
    let params = ParamSet::new(7, &[0.9]).unwrap();
    assert!(control.set_params(master, params), "the command ring has room");
    // The bad command queues fine — the REFUSAL is audio-side (unknown node), and counted there.
    assert!(control.set_params(NodeId(999), params), "the ring does not judge; the executor does");
    phase.store(2, Ordering::Release);

    let (p1, p2, _final_patch) = audio_thread.join().unwrap();
    assert_eq!(p1, 0.5);
    assert_eq!(p2, 0.9, "the parameter crossed the thread boundary without a swap");
    let stats = control.stats();
    assert_eq!(stats.swap.swaps, 0, "no patch was staged — this is the command path alone");
    assert_eq!(stats.cmd_applied, 1, "the good command applied");
    assert_eq!(stats.cmd_refused, 1, "the unknown node was refused and counted, never silent");
}

#[test]
fn the_musical_position_command_reaches_the_module_context() {
    let (live, master) = probe_executor(TickProbe, "sparq/test/tick-probe", false, 0.0);
    let (control, audio_engine) = SharedEngine::new(live, master);
    let phase = Arc::new(AtomicU64::new(0));

    let audio_thread = {
        let phase = Arc::clone(&phase);
        std::thread::spawn(move || {
            let mut ae = audio_engine;
            let mut out = vec![0.0f32; 64];
            ae.render_block(&mut out).unwrap();
            let before = out[0];
            phase.store(1, Ordering::Release);
            while phase.load(Ordering::Acquire) < 2 {
                std::hint::spin_loop();
            }
            ae.render_block(&mut out).unwrap();
            let during = out[0];
            phase.store(3, Ordering::Release);
            while phase.load(Ordering::Acquire) < 4 {
                std::hint::spin_loop();
            }
            ae.render_block(&mut out).unwrap();
            let after = out[0];
            (before, during, after, ae.shutdown())
        })
    };

    spin_until(|| phase.load(Ordering::Acquire) >= 1, "the first block");
    assert!(control.set_musical_position(Some((12_345, 960))));
    phase.store(2, Ordering::Release);
    spin_until(|| phase.load(Ordering::Acquire) >= 3, "the positioned block");
    assert!(control.set_musical_position(None));
    phase.store(4, Ordering::Release);

    let (before, during, after, _patch) = audio_thread.join().unwrap();
    assert_eq!(before, 0.0, "the never-set door renders the static tick every golden knows");
    assert_eq!(during, 12_345.0, "the transport computes, the executor carries — across threads");
    assert_eq!(after, 12_345.0, "ClearMusical freezes the clock; rewinding is the transport's job");
    assert_eq!(control.stats().cmd_applied, 2);
}

#[test]
fn the_transport_clock_survives_the_cross_thread_swap() {
    let (live, master) = dc_executor(0.5);
    let (control, audio_engine) = SharedEngine::new(live, master);
    let phase = Arc::new(AtomicU64::new(0));

    let audio_thread = {
        let phase = Arc::clone(&phase);
        std::thread::spawn(move || {
            let mut ae = audio_engine;
            let mut out = vec![0.0f32; 64];
            let mut last = 0u64;
            let mut regressions = 0u64;
            for _ in 0..5 {
                ae.render_block(&mut out).unwrap();
                let b = ae.blocks_rendered();
                if b <= last {
                    regressions += 1;
                }
                last = b;
            }
            phase.store(1, Ordering::Release);
            while phase.load(Ordering::Acquire) < 2 {
                std::hint::spin_loop();
            }
            for _ in 0..3 {
                ae.render_block(&mut out).unwrap();
                let b = ae.blocks_rendered();
                if b <= last {
                    regressions += 1;
                }
                last = b;
            }
            (last, regressions, ae.shutdown())
        })
    };

    spin_until(|| phase.load(Ordering::Acquire) >= 1, "the first five blocks");
    let (next, next_master) = dc_executor(0.5);
    control.stage(next, next_master);
    phase.store(2, Ordering::Release);

    let (last, regressions, _patch) = audio_thread.join().unwrap();
    assert_eq!(
        last, 8,
        "five blocks + the swap boundary + three blocks: the count never restarted"
    );
    assert_eq!(regressions, 0, "the clock never moved backwards across the swap");
    while control.reclaim().is_some() {}
    assert_eq!(control.stats().swap.swaps, 1);
}

#[test]
fn meters_are_published_per_block_and_an_absent_reader_is_counted_not_queued() {
    // The reference world (sine → gain → rms + spare, 4 nodes), a meter ring of 8 slots, and
    // ten rendered blocks: 40 publications into 8 slots with nobody reading — decision 8's
    // promise is that the audio thread NEVER waits for a reader, and plan §4.3's promise is
    // that the overflow is counted, not silent.
    let reg = registry();
    let cfg = ExecConfig::new(48_000, 64, 2);
    let world = World::initial();
    let first = world.build(&reg, cfg).unwrap();
    let (control, audio_engine) = SharedEngine::with_capacities(first, world.master(), 16, 8);

    let audio_thread = std::thread::spawn(move || {
        let mut ae = audio_engine;
        let mut out = vec![0.0f32; 64 * 2];
        for _ in 0..10 {
            ae.render_block(&mut out).unwrap();
        }
        (ae.blocks_rendered(), ae.shutdown())
    });

    let (blocks, _patch) = audio_thread.join().unwrap();
    assert_eq!(blocks, 10);
    let mut buf = [MeterUpdate::default(); 64];
    let read = control.read_meters(&mut buf);
    assert_eq!(
        read, 8,
        "the ring kept exactly its capacity: the OLDEST updates, every newer one refused-and-counted"
    );
    let node_ids: Vec<u32> = world.graph.nodes().iter().map(|n| n.id.0).collect();
    for u in &buf[..read] {
        assert!(node_ids.contains(&u.node), "meter for a node that is not in the graph");
        assert!(u.block >= 1 && u.block <= 2, "with nobody reading, only the first blocks fit");
        assert!(u.peak >= 0.0 && u.peak.is_finite());
        // Every rendered node runs Ok — and since increment 7 that is the ONLY legal answer:
        // the world's spare gain used to sit unwired and honestly report Silenced, but a bare
        // required input is now refused at build, so no world node can render without its
        // signal. The meter publication tells the truth about every node; a Silenced here
        // would mean the enforcement leaked.
        assert!(
            matches!(u.block_status(), BlockStatus::Ok),
            "unexpected status {:?}",
            u.block_status()
        );
    }
    assert!(buf[..read].iter().any(|u| u.peak > 0.0), "the sine node meters signal, not silence");
    let stats = control.stats();
    assert_eq!(
        stats.meter_refusals,
        40 - 8,
        "every publication the ring could not hold is counted"
    );
}

#[test]
fn the_cross_thread_audio_path_allocates_nothing() {
    let _guard = audit_serialised();
    let (live, master) = dc_executor(0.25);
    let (control, audio_engine) = SharedEngine::new(live, master);
    let stop = Arc::new(AtomicBool::new(false));

    let audio_thread = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut ae = audio_engine;
            let mut out = vec![0.0f32; 64]; // allocated BEFORE the window opens
            let base = sparq_kernel::alloc::start_counting();
            let mut blocks = 0u64;
            while !stop.load(Ordering::Acquire) {
                ae.render_block(&mut out).unwrap();
                blocks += 1;
            }
            let made = sparq_kernel::alloc::allocation_count().saturating_sub(base);
            sparq_kernel::alloc::stop_counting();
            drop(ae.shutdown());
            (blocks, made)
        })
    };

    // 100 paced swaps while the audio thread free-runs inside the armed window.
    for i in 0..100u64 {
        let (next, next_master) = dc_executor(0.25 + i as f32 * 0.001);
        control.stage(next, next_master);
        spin_until(
            || control.stats().swap.swaps + control.stats().swap.superseded > i,
            "the staged patch to be consumed",
        );
        while control.reclaim().is_some() {}
    }
    stop.store(true, Ordering::Release);
    let (blocks, made) = audio_thread.join().unwrap();

    assert_eq!(made, 0, "the audio thread allocated across boundaries, swaps, commands and meters");
    assert!(blocks >= 100, "the audio thread free-ran while the control side staged: {blocks}");
    let stats = control.stats();
    assert_eq!(stats.swap.swaps + stats.swap.superseded, 100, "every staged patch was consumed");
    assert_eq!(stats.swap.deferred, 0, "paced reclaim never starved the retirement slots");
    assert!(
        sparq_kernel::alloc::audit_is_live(),
        "the gate is live, not vacuous (alloc.rs's own probe)"
    );
}

// --------------------------------------------------------------------------- the acceptance

/// The allowlist entry 6 test, at the executor level: **10 000 random mutations staged from a
/// control thread while the audio thread plays**, with the allocation gate armed on the audio
/// thread for the whole run. Paced staging (each patch is consumed before the next is offered)
/// makes every counter an equality instead of a range; the paced-real-time half stays
/// device-track with the loaded soak, declared in the file header.
#[test]
fn ten_thousand_mutations_while_playing_across_two_threads() {
    let _guard = audit_serialised();
    let reg = registry();
    let cfg = ExecConfig::new(96_000, 64, 2); // the acceptance rate
    let world = World::initial();
    let first = world.build(&reg, cfg).unwrap();
    let (control, audio_engine) = SharedEngine::new(first, world.master());
    let stop = Arc::new(AtomicBool::new(false));

    let audio_thread = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let mut ae = audio_engine;
            let mut out = vec![0.0f32; 64 * 2]; // allocated BEFORE the window opens
            let base = sparq_kernel::alloc::start_counting();
            let mut blocks = 0u64;
            let mut errors = 0u64;
            let mut clock_regressions = 0u64;
            let mut last_count = 0u64;
            while !stop.load(Ordering::Acquire) {
                if ae.render_block(&mut out).is_err() {
                    errors += 1; // counted, not asserted in-thread: a panic here would hang control
                }
                blocks += 1;
                let bc = ae.blocks_rendered();
                if bc < last_count {
                    clock_regressions += 1;
                }
                last_count = bc;
                if blocks % 256 == 0 {
                    std::thread::yield_now();
                }
            }
            let made = sparq_kernel::alloc::allocation_count().saturating_sub(base);
            sparq_kernel::alloc::stop_counting();
            let final_patch: LivePatch = ae.shutdown();
            (blocks, errors, clock_regressions, made, last_count, final_patch)
        })
    };

    // The control side: the SAME seeded mutation schedule the single-owner stress runs —
    // add / remove / connect / disconnect / retune / re-declare latency — but staged across the
    // thread boundary and paced so the ledger is exact.
    let mut world = world;
    let mut rng = SplitMix64::new(20_260_927);
    let mut staged = 0u64;
    let mut refused = 0u64;
    let mut reclaimed = 0u64;
    for _ in 0..10_000 {
        let next_world = match world.candidate_mutation(&mut rng) {
            Ok(w) => w,
            Err(_) => {
                refused += 1;
                continue; // a refusal leaves no trace — the audio thread never notices
            },
        };
        match next_world.build(&reg, cfg) {
            Ok(next) => {
                control.stage(next, next_world.master());
                staged += 1;
                world = next_world;
                spin_until(
                    || control.stats().swap.swaps + control.stats().swap.superseded >= staged,
                    "the staged patch to be consumed by the audio thread",
                );
                while control.reclaim().is_some() {
                    reclaimed += 1;
                }
            },
            Err(_) => refused += 1, // executor-level refusal: same contract, audio renders on
        }
    }
    stop.store(true, Ordering::Release);
    let (blocks, errors, clock_regressions, made, last_count, final_patch) =
        audio_thread.join().unwrap();
    while control.reclaim().is_some() {
        reclaimed += 1;
    }
    drop(final_patch); // the last live patch drops here, control-side, after the stream stopped

    let stats = control.stats();
    assert_eq!(errors, 0, "every rendered block succeeded while the graph was being rebuilt");
    assert_eq!(made, 0, "the audio thread allocated during {blocks} rendered blocks");
    assert_eq!(clock_regressions, 0, "the stream clock never moved backwards across a swap");
    assert!(staged > 1_000, "mutations actually staged across the boundary: {staged}");
    assert!(refused > 0, "10 000 random wiring attempts MUST hit refusals (refused: {refused})");
    assert_eq!(
        stats.swap.swaps + stats.swap.superseded,
        staged,
        "every staged patch was consumed — nothing vanished in the slot"
    );
    assert_eq!(stats.swap.superseded, 0, "paced staging: nothing was superseded");
    assert_eq!(stats.swap.deferred, 0, "paced reclaim: the retirement slots never ran dry");
    assert_eq!(stats.swap.retirements, stats.swap.swaps, "every swap retired exactly one patch");
    assert_eq!(reclaimed, stats.swap.swaps, "every retirement came back to the control thread");
    assert!(
        blocks >= staged,
        "every consumed patch rode a boundary, and the audio thread kept playing between them: \
         {blocks} blocks for {staged} mutations"
    );
    assert_eq!(last_count, blocks, "the inherited clock counts every block exactly once");
    assert!(
        world.graph.node_count() >= 1 && world.graph.node_count() <= 8,
        "the final world stayed inside the stress graph's cap: {} nodes",
        world.graph.node_count()
    );
    println!(
        "cross-thread stress: {} blocks · {} staged · {} swaps · {} refused · {} reclaimed · \
         {} nodes / {} edges final · alloc {} · errors {}",
        blocks,
        staged,
        stats.swap.swaps,
        refused,
        reclaimed,
        world.graph.node_count(),
        world.graph.edge_count(),
        made,
        errors
    );
}
