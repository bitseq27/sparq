//! The WO-008 acceptance stress: **10 000 random mutations while the graph is rendering**, with
//! the audio path under the counting allocator. This is the risk column's mitigation shipped as
//! a first-class deliverable, not an afterthought: "mutation-during-playback races are the
//! classic modular-system bug" — so the mutation path gets its own binary, its own gate, and a
//! number with a comma in it.
//!
//! What is proven here, and what is not, in words:
//! * **Proven (sandbox, deterministic):** 10 000 seeded mutations — add / remove / connect /
//!   disconnect / retune / re-declare latency — each one rebuilt on the control side and swapped
//!   at exactly one block boundary through the task-4 engine; every one of the 10 001 rendered
//!   blocks succeeds; **zero allocations on the audio path** across all of them; refusals
//!   (cycles, duplicates, self-loops, the node cap) leave no trace and the render never stops;
//!   the block count continues monotonically across every swap; and the whole script replays
//!   **bit-identical** (task 7's harness over the same machinery).
//! * **NOT proven here (device-side, declared):** true two-thread concurrency against a live
//!   HAL — the cross-thread hot-swap primitive (ADR-009 d3's literal pointer swap with an
//!   epoch-based grace period) is allowlisted-`unsafe` kernel work with its own increment and
//!   its own proofs; until then the engine is single-owner and the loaded soak (200 modules,
//!   30 min, zero xruns) on SATURN remains the hardware acceptance, exactly as the WO schedules
//!   it. This binary removes every excuse that soak could find that is not concurrency itself.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// The stress run prints one evidence line for the log (blocks · swaps · refusals · hash) —
// stdout is the test binary's report channel, same as the gates' other measured numbers.
#![allow(clippy::print_stdout)]

// Defect #66's discipline: the counting allocator's windows are process-wide, so every test in
// this binary serialises on one lock — armed windows never overlap a sibling's bookkeeping.
#[allow(clippy::disallowed_types)] // harness-side serialisation, not audio path (defect #66)
use std::sync::{Mutex, MutexGuard};

use sparq_audio::determinism::{run_script, Script, ScriptRun};
use sparq_audio::executor::ExecConfig;
use sparq_kernel::alloc::CountingAllocator;
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

/// The acceptance number: 10 000 iterations, at the acceptance rate (96 kHz / 64), with the
/// allocation gate armed around every single rendered block.
#[test]
fn ten_thousand_mutations_while_playing_never_xrun_never_allocate() {
    let _guard = audit_serialised();
    let reg = registry();
    let cfg = ExecConfig::new(96_000, 64, 2);
    let script = Script { seed: 20_260_925, mutations: 10_000, alloc_gate: true };
    let mut out = vec![0.0f32; 64 * 2];
    let r: ScriptRun = run_script(&reg, cfg, &script, &mut out).expect("the script must run");

    assert_eq!(r.blocks, 10_001, "one block per mutation attempt plus the opening block");
    assert_eq!(r.alloc_violations, 0, "the audio path allocated during rendered blocks");
    assert!(r.swaps > 1_000, "mutations actually staged and swapped: {}", r.swaps);
    assert!(
        r.refused > 0,
        "10 000 random wiring attempts MUST hit the kernel's refusals — a stress test that is \
         never refused is not stressing the gate (refused: {})",
        r.refused
    );
    assert_eq!(r.superseded, 0, "one stage per boundary: nothing is superseded");
    assert!(r.final_nodes >= 1 && r.final_nodes <= 8, "final graph: {}", r.final_nodes);
    println!(
        "mutation stress: {} blocks · {} swaps · {} refused · {} nodes / {} edges final · \
         max path latency {} · hash {:016x}",
        r.blocks, r.swaps, r.refused, r.final_nodes, r.final_edges, r.max_path_latency, r.hash
    );
}

/// Task 7 over the same machinery at stress scale: one script, two runs, bit-identical samples.
/// (The 10 000-run above is the endurance number; the replay claim is checked at 1 000 so the
/// pair stays quick enough to gate every commit.)
#[test]
fn the_mutation_script_replays_bit_identical() {
    let _guard = audit_serialised();
    let reg = registry();
    let cfg = ExecConfig::new(96_000, 64, 2);
    let script = Script { seed: 0xC0FFEE, mutations: 1_000, alloc_gate: true };
    let mut out = vec![0.0f32; 64 * 2];
    let a = run_script(&reg, cfg, &script, &mut out).unwrap();
    let b = run_script(&reg, cfg, &script, &mut out).unwrap();
    assert_eq!(a.hash, b.hash, "same seed ⇒ same schedule ⇒ same samples");
    assert_eq!(a.swaps, b.swaps);
    assert_eq!(a.refused, b.refused);
    assert_eq!(a.alloc_violations, 0);
    assert_eq!(b.alloc_violations, 0);
}
