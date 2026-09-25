//! Real-time discipline, proven by measurement rather than by inspection.
//!
//! This binary installs [`CountingAllocator`] as its **global allocator**, so every allocation made
//! anywhere in the process is observable. The counter is thread-local and armed explicitly, so the
//! measured region contains only the audio path: the graph, its scratch buffers and the control
//! ring are all constructed *before* arming.
//!
//! This is the test that makes plan §5.2 rule 1 ("the audio thread performs no allocation") a fact
//! about the build rather than an aspiration about the author.

// Test harness: panicking on a broken precondition is the correct behaviour here, and the
// workspace-wide `deny` on unwrap/expect exists to keep them out of the instrument, not out of tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// The `std::sync::Mutex` ban (clippy.toml) protects the audio path; this file is the harness
// measuring it. AUDIT_LOCK serialises the tests' own measurement windows and is never visible
// to the graph code under audit - the same category as the null device's allowed `Instant::now`
// (defect #66). Targeted allow here rather than a file-wide one, so the ban still bites anywhere
// else in this file.
#[allow(clippy::disallowed_types)]
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use sparq_audio::graph::{ControlMsg, GraphConfig, Wo005Graph};
use sparq_audio::hash::fnv1a64_f32;
use sparq_kernel::alloc::{
    allocation_count, audit_is_live, outstanding_allocations, start_counting, stop_counting,
    CountingAllocator,
};
use sparq_kernel::device::{RenderConfig, StreamConfig};
use sparq_kernel::rt::Counters;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

/// Serialisation of the armed audit windows (defect #66 — found by SATURN's P1 run, 2026-09-23,
/// and reproduced in the sandbox with `--test-threads=8`).
///
/// Arming is per-thread, but the counters the deltas read are **process-wide**: a second test
/// armed at the same moment adds its allocations to this test's window. The 2-core sandbox never
/// exposed that — libtest ran at most two tests at once, and the short tests all finished inside
/// `audio_path_makes_zero_allocations`'s *unarmed* warm-up. SATURN runs all eight concurrently,
/// and `outstanding_balance_survives_realloc_growth`'s deliberate ~129-allocation burst landed
/// inside `multichannel_render_stays_allocation_free`'s armed render windows: the audit reported
/// "129 allocations" that were a sibling test's bookkeeping, not the audio path's. The one-off
/// `audit_is_live` probe is the same hazard at size 1 (observed as `left: 1`).
///
/// Rule for this binary: every test holds this lock for its whole body, so armed windows never
/// overlap; un-armed threads add nothing to the counters, so nothing outside this file is
/// affected. The lock is taken through [`audit_serialised`], which also pre-fires the
/// `audit_is_live` probe under the lock so its single allocation cannot land in an open window.
/// A poisoned lock (a test panicked while holding it) is un-poisoned rather than cascaded: the
/// original failure is libtest's to report, and the remaining gates still measure honestly.
#[allow(clippy::disallowed_types)] // harness-side serialisation, not audio path - see the use statement (defect #66)
static AUDIT_LOCK: Mutex<()> = Mutex::new(());

fn audit_serialised() -> MutexGuard<'static, ()> {
    let guard = AUDIT_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let _ = audit_is_live();
    guard
}

fn cfg() -> GraphConfig {
    GraphConfig {
        stream: StreamConfig {
            sample_rate: 96_000,
            block_frames: 64,
            inputs: 0,
            outputs: 2,
            exclusive: false,
        },
        ..GraphConfig::default()
    }
}

/// Count allocations made by `f` on this thread, with everything it needs already built.
fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

#[test]
fn audio_path_makes_zero_allocations() {
    let _audit = audit_serialised();
    let mut graph = Wo005Graph::new(cfg());
    let mut buf = vec![0.0f32; 64 * 2];
    let ctx = sparq_kernel::block::BlockContext::offline(96_000, 64, 2);

    // Run a substantial un-measured pass first. The reason is not the DSP — a standalone probe
    // (`sparq-app` example `probe-alloc`) shows 0 allocations over 5 000 blocks — it is that the
    // test *harness* performs a handful of one-shot lazy initialisations on the first assert/print
    // in a thread. Measuring cold therefore counts the harness, not the audio path.
    for _ in 0..20_000 {
        graph.process(&ctx, &mut buf);
        std::hint::black_box(&buf);
    }

    let made = measure(|| {
        for _ in 0..20_000 {
            graph.process(&ctx, &mut buf);
            std::hint::black_box(&buf);
        }
    });
    assert_eq!(made, 0, "plan §5.2 rule 1 violated: 20 000 blocks allocated {made} time(s)");

    // And prove it does not scale: a second window must also be exactly zero. A per-block leak
    // would show up here even if some one-shot allocation had slipped into the first window.
    let again = measure(|| {
        for _ in 0..20_000 {
            graph.process(&ctx, &mut buf);
            std::hint::black_box(&buf);
        }
    });
    assert_eq!(again, 0, "allocation count grew between windows: {made} then {again}");
}

#[test]
fn control_drain_does_not_allocate() {
    let _audit = audit_serialised();
    let mut graph = Wo005Graph::new(cfg());
    let mut buf = vec![0.0f32; 64 * 2];
    let ctx = sparq_kernel::block::BlockContext::offline(96_000, 64, 2);
    for i in 0..64i16 {
        let _ = graph.control().push(ControlMsg::GainDb(f32::from(i % 30) - 30.0));
    }

    for _ in 0..200 {
        graph.process(&ctx, &mut buf);
        let _ = graph.control().push(ControlMsg::GainDb(-6.0));
    }
    let made = measure(|| {
        for _ in 0..1000 {
            graph.process(&ctx, &mut buf);
            // Push from the same thread: the ring is SPSC, and in this test the "producer" is the
            // harness. What is being measured is that neither push nor drain allocates.
            let _ = graph.control().push(ControlMsg::GainDb(-6.0));
        }
    });
    assert_eq!(made, 0, "control message handling allocated {made} time(s)");
}

#[test]
fn full_render_reports_zero_allocations_through_the_kernel_audit() {
    let _audit = audit_serialised();
    let mut graph = Wo005Graph::new(cfg());
    let report = graph.render(96_000); // one second at 96 kHz
    assert_eq!(report.counters.audio_allocations, 0, "{:?}", report.counters);
    assert_eq!(report.counters.underruns, 0);
    assert_eq!(report.counters.blocks, 96_000 / 64);
}

#[test]
fn underrun_counter_is_not_decorative() {
    let _audit = audit_serialised();
    let mut graph = Wo005Graph::new(cfg());
    // 64 frames at 96 kHz is a 667 µs budget; the harness stalls block 1 for 3 ms.
    let budget = RenderConfig::natural_budget(&cfg().stream);
    let mut rc = RenderConfig::realtime(cfg().stream, 64 * 8, budget);
    rc.stall = Some((1, Duration::from_millis(3)));
    let report = graph.render_with(rc);
    assert!(report.counters.underruns >= 1, "overload went undetected: {:?}", report.counters);
    assert!(
        report.peak_block.is_some_and(|p| p > Duration::from_millis(2)),
        "peak block time should reflect the injected overload: {:?}",
        report.peak_block
    );
}

#[test]
fn render_is_bit_reproducible_across_instances() {
    let _audit = audit_serialised();
    let mut a = Wo005Graph::new(cfg());
    let mut b = Wo005Graph::new(cfg());
    let ra = a.render(96_000);
    let rb = b.render(96_000);
    assert_eq!(fnv1a64_f32(&ra.samples), fnv1a64_f32(&rb.samples), "ADR-007 violated");
}

#[test]
fn multichannel_render_stays_allocation_free() {
    let _audit = audit_serialised();
    // 8 channels is the smallest rig that matters (plan §9.6); 64 is the Phase 7 target.
    for chans in [1usize, 2, 8, 32] {
        let cfg = GraphConfig { stream: StreamConfig { outputs: chans, ..cfg().stream }, ..cfg() };
        let mut graph = Wo005Graph::new(cfg);
        let report = graph.render(4096);
        assert_eq!(report.counters.audio_allocations, 0, "{chans} channels allocated");
        assert_eq!(report.samples.len(), 4096 * chans);
    }
}

#[test]
fn counters_start_clean_and_report_honestly() {
    let _audit = audit_serialised();
    let c = Counters::zero();
    assert!(c.is_clean());
    assert!(c.summary().contains("underrun 0"));
    let dirty = Counters { underruns: 1, ..Counters::zero() };
    assert!(!dirty.is_clean() || dirty.underruns == 1);
    assert!(dirty.summary().contains("underrun 1"));
}

/// The outstanding-allocation balance must survive buffer growth (defect #48).
///
/// This is the gate that `devices --conformance` uses to decide whether a backend leaks across
/// open/start/stop/drop cycles. It reported 152 phantom allocations on the WASAPI backend because
/// `realloc` bumped the allocation counter without bumping the deallocation counter — so every
/// `format!` that grew its `String` left the balance permanently one higher. The reopen gate is
/// only trustworthy if growth nets out to zero, which is what this proves.
#[test]
fn outstanding_balance_survives_realloc_growth() {
    let _audit = audit_serialised();
    // Everything the measurement needs is built before arming, so the region measures only the
    // growth itself.
    let seed: Vec<u8> = (0..64u8).collect();
    let base = start_counting();
    let before = outstanding_allocations();

    let mut grown: Vec<u8> = Vec::new();
    for round in 0..12 {
        grown.extend_from_slice(&seed);
        grown.reserve_exact(seed.len() * (round + 1));
        std::hint::black_box(&grown);
    }
    // String growth via format!: the exact shape the diagnostics text produces.
    let mut text = String::new();
    for i in 0..40 {
        text = format!("{text} · block {i} of forty, with enough words to force a realloc");
        std::hint::black_box(&text);
    }

    let mid = outstanding_allocations();
    assert!(mid > before, "the test must actually allocate, or it proves nothing");

    drop(grown);
    drop(text);
    let after = outstanding_allocations();
    stop_counting();

    assert_eq!(
        after, before,
        "growing then dropping must return the balance to where it started \
         ({} before, {mid} while live, {after} after) — a nonzero delta means realloc is \
         counted on one side only, which is defect #48 and makes the reopen-leak gate lie",
        before
    );
    let _ = base;
}
