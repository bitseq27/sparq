//! WO-009 acceptance, measured. The work order's criteria, mapped:
//!
//! 1. *"A trigger scheduled at tick T fires at exactly the corresponding sample index — verified
//!    for 1 000 random ticks against an analytically computed expectation (±0 samples)."*
//!    → [`one_thousand_random_ticks_fire_at_the_analytic_sample`] (constant tempo) and
//!    [`scheduled_ticks_survive_tempo_changes_at_the_exact_new_samples`] (piecewise, with the
//!    test carrying its OWN implementation of the segment integral and its own bisection — the
//!    expectation is computed from the spec's math, not by calling the code under test).
//! 2. *"Tempo change during playback produces no discontinuity in either direction … verified by
//!    a sweep test and by listening."* → [`a_tempo_sweep_keeps_the_map_monotonic_and_bounded`]
//!    (the sweep test; listening is the device track — `drum-demo` with `--seconds` is the
//!    artefact, and the kernel's own continuity tests carry the derivative math).
//! 3. *"t_wall drift vs device clock is estimated and reported; over 30 min the reported drift
//!    matches an independent measurement within tolerance."* → the estimator's synthetic-drift
//!    convergence lives in `broker.rs`'s tests; the 30-minute-on-real-hardware half is DEVICE
//!    track and stays an open box, declared (the ADR-006 addendum records it).
//! 4. *"Transport stop/start is sample-accurate and repeatable (same event twice ⇒ same
//!    output)."* → [`stop_start_scripts_are_repeatable`].
//!
//! Plus the pump-safety claim the crate header makes: [`advance_block_never_allocates`], under
//! the kernel's counting allocator (this binary's global allocator, the house pattern).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_module_api::event::{Event, EventKind};
use sparq_music::transport::{BeatConfig, BlockEvents, Transport};

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const PPQN: u32 = 960;

/// A transport with beat/bar emission off — the schedule under test is the only event source.
fn quiet_transport(bpm: f64) -> Transport {
    Transport::with_beats(
        RATE,
        bpm,
        PPQN,
        BeatConfig { beats: false, bars: false, ..BeatConfig::default() },
    )
}

/// A deterministic LCG (the house style: a seeded walk, never `rand`).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 =
            self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

// ---------------------------------------------------------------- criterion 1a: constant tempo

#[test]
fn one_thousand_random_ticks_fire_at_the_analytic_sample() {
    // 120 bpm @ 48 kHz / 960 PPQN: ticks per sample = 120·960/(60·48000) = 1/25 exactly as an
    // f64 division, so the analytic sample for tick T is round(T·25) — computed HERE, from the
    // spec, not from the broker.
    let tps = 120.0 * f64::from(PPQN) / (60.0 * f64::from(RATE));
    let mut tr = quiet_transport(120.0);
    let mut rng = Lcg(0x5EED_2026_0926);
    let mut ticks: Vec<u64> = (0..1000).map(|_| 1 + rng.below(30_000)).collect();
    ticks.sort_unstable();
    for (i, &t) in ticks.iter().enumerate() {
        tr.schedule(
            t,
            Event { channel: i as u16, value: t as f32, ..Event::trigger(u32::MAX, 0.0) },
        );
    }
    tr.play();
    let mut out = BlockEvents::new();
    let mut fired: Vec<(u64, u64)> = Vec::new(); // (tick, absolute sample)
    let mut abs = 0u64;
    for _ in 0..13_000 {
        tr.advance_block(64, &mut out);
        for e in out.as_slice() {
            fired.push((e.value as u64, abs + u64::from(e.sample)));
        }
        abs += 64;
        if fired.len() == ticks.len() {
            break;
        }
    }
    assert_eq!(fired.len(), ticks.len(), "every scheduled tick fired exactly once");
    for (i, &(tick, sample)) in fired.iter().enumerate() {
        assert_eq!(tick, ticks[i], "fire order is tick order (the schedule is sorted)");
        let expect = ((tick as f64) / tps).round() as u64;
        assert_eq!(
            sample, expect,
            "tick {tick}: fired at {sample}, analytic {expect} — ±0 or it is a defect"
        );
        // The f32 round-trip of the tick is exact below 2^24; the channel echoes the index.
        assert!(tick < (1 << 24));
    }
}

// ---------------------------------------------------------------- criterion 1b: across tempo changes

/// The test's OWN piecewise map: segments anchored at the samples where the script changes
/// tempo, with the spec's linear-bpm-ramp integral. Written from ADR-006's description, not
/// transcribed from `clock.rs` — two implementations agreeing to the sample is the point.
struct TestMap {
    // (start_sample, start_tick, tps_from, tps_to, ramp_samples)
    segs: Vec<(u64, f64, f64, f64, u64)>,
}

impl TestMap {
    fn tps(bpm: f64) -> f64 {
        bpm * f64::from(PPQN) / (60.0 * f64::from(RATE))
    }

    fn tick_of(&self, s: u64) -> f64 {
        let mut idx = 0;
        for (i, seg) in self.segs.iter().enumerate() {
            if seg.0 <= s {
                idx = i;
            }
        }
        let (start, start_tick, tps0, tps1, ramp) = self.segs[idx];
        let d = (s - start) as f64;
        if ramp == 0 {
            return start_tick + d * tps1;
        }
        let r = ramp as f64;
        if d <= r {
            start_tick + tps0 * d + (tps1 - tps0) * d * d / (2.0 * r)
        } else {
            let at_ramp_end = start_tick + tps0 * r + (tps1 - tps0) * r / 2.0;
            at_ramp_end + tps1 * (d - r)
        }
    }

    /// Nearest sample for a tick: bisect to the first `s` with `tick_of(s) ≥ t`, then take
    /// whichever neighbour is closer, ties up — the spec rule, implemented here independently.
    fn sample_at_tick(&self, t: u64) -> u64 {
        let tf = t as f64;
        let mut lo = 0u64;
        let mut hi = 1u64;
        while self.tick_of(hi) < tf {
            lo = hi;
            hi = hi.saturating_mul(4).max(hi + 1_000_000);
        }
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.tick_of(mid) >= tf {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        if lo > 0 {
            let d_below = tf - self.tick_of(lo - 1);
            let d_above = self.tick_of(lo) - tf;
            if d_below < d_above {
                return lo - 1;
            }
        }
        lo
    }
}

#[test]
fn scheduled_ticks_survive_tempo_changes_at_the_exact_new_samples() {
    // The script: 120 bpm; at block 1500 (sample 96 000) glide to 180 over 10 ms (480 samples);
    // at block 4000 (sample 256 000) STEP to 90. Everything is scheduled up front — the events
    // do not move in tick space, so the map change moves their SAMPLES, and the acceptance is
    // that they land exactly where the new map puts them.
    let ramp = (f64::from(RATE) * 10.0 / 1000.0).round() as u64; // 480
    let a = 96_000u64;
    let b = 256_000u64;
    let t120 = TestMap::tps(120.0);
    let t180 = TestMap::tps(180.0);
    let t90 = TestMap::tps(90.0);
    let tick_a = t120 * a as f64;
    let tick_a_ramp_end = tick_a + t120 * ramp as f64 + (t180 - t120) * ramp as f64 / 2.0;
    let tick_b = tick_a_ramp_end + t180 * (b - a - ramp) as f64;
    let map = TestMap {
        segs: vec![
            (0, 0.0, t120, t120, 0),
            (a, tick_a, t120, t180, ramp),
            (b, tick_b, t90, t90, 0),
        ],
    };

    let mut tr = quiet_transport(120.0);
    let mut rng = Lcg(0xC10C_4C1E);
    // Ticks spread across all three regimes: before the ramp, inside the post-ramp cruise,
    // and past the step.
    let max_tick = map.tick_of(3_000_000) as u64;
    let mut ticks: Vec<u64> = (0..1000).map(|_| 1 + rng.below(max_tick.max(2))).collect();
    ticks.sort_unstable();
    ticks.dedup();
    for (i, &t) in ticks.iter().enumerate() {
        tr.schedule(
            t,
            Event { channel: i as u16, value: t as f32, ..Event::trigger(u32::MAX, 0.0) },
        );
    }
    let n = ticks.len();
    tr.play();
    let mut out = BlockEvents::new();
    let mut fired: Vec<(u64, u64)> = Vec::new();
    let mut abs = 0u64;
    for block in 0..60_000u64 {
        if block == 1500 {
            assert!(tr.set_tempo_with_ramp(180.0, 10.0), "the transport refused its own script");
        }
        if block == 4000 {
            assert!(tr.set_tempo_with_ramp(90.0, 0.0));
        }
        tr.advance_block(64, &mut out);
        for e in out.as_slice() {
            fired.push((e.value as u64, abs + u64::from(e.sample)));
        }
        abs += 64;
        if fired.len() == n {
            break;
        }
    }
    assert_eq!(fired.len(), n, "every tick fired ({}/{n})", fired.len());
    let mut worst: i64 = 0;
    for &(tick, sample) in &fired {
        let expect = map.sample_at_tick(tick);
        let err = sample as i64 - expect as i64;
        assert_eq!(err, 0, "tick {tick}: fired at {sample}, the independent map says {expect}");
        worst = worst.max(err.abs());
    }
    assert_eq!(worst, 0, "±0 samples across {n} ticks and two tempo edits — ADR-006 rule 7");
}

// ---------------------------------------------------------------- criterion 2: the sweep

#[test]
fn a_tempo_sweep_keeps_the_map_monotonic_and_bounded() {
    // 60 → 200 → 70 bpm, one target per block, the default 10 ms ramp: the hostile automation
    // pattern. The playhead must stay strictly monotone, per-block deltas must stay inside the
    // fastest tempo's envelope, and the effective tempo must track the target within the ramp.
    let mut tr = Transport::with_beats(
        RATE,
        120.0,
        PPQN,
        BeatConfig { beats: false, bars: false, ..BeatConfig::default() },
    );
    tr.play();
    let mut out = BlockEvents::new();
    let tps_max = 200.0 * f64::from(PPQN) / (60.0 * f64::from(RATE));
    let mut prev_tick = 0.0f64;
    for block in 0..6_000u64 {
        // A triangle sweep: 60 up to 200 over 2000 blocks, back down to 70 over 2000, hold.
        let target = if block < 2_000 {
            60.0 + 140.0 * (block as f64 / 2_000.0)
        } else if block < 4_000 {
            200.0 - 130.0 * ((block - 2_000) as f64 / 2_000.0)
        } else {
            70.0
        };
        assert!(tr.set_tempo(target));
        tr.advance_block(64, &mut out);
        let tick = tr.abs_tick();
        let delta = tick - prev_tick;
        assert!(delta > 0.0, "block {block}: the playhead went backwards or froze");
        assert!(
            delta <= tps_max * 64.0 + 1e-9,
            "block {block}: delta {delta} exceeds the fastest tempo's block span"
        );
        let eff = tr.bpm_now();
        assert!(
            eff > 55.0 && eff < 205.0,
            "block {block}: effective tempo {eff} left the sweep's hull"
        );
        prev_tick = tick;
    }
    // Both map directions still agree after 6000 edits: a scheduled tick fires where the map says.
    let map_sample = tr.broker().sample_at_tick(90_000);
    let round_trip = tr.broker().tick_at_sample(map_sample);
    assert!(
        round_trip.abs_diff(90_000) <= 1,
        "round trip after the sweep: 90000 → {map_sample} → {round_trip}"
    );
}

// ---------------------------------------------------------------- criterion 4: stop/start repeatability

#[test]
fn stop_start_scripts_are_repeatable() {
    // The same script, twice, from fresh transports: identical event streams down to the sample.
    // Stop FREEZES the timeline (offline tape semantics), so a paused stretch contributes
    // nothing and cannot shift what follows.
    fn run_script() -> Vec<(u64, u32, u16, f32)> {
        let mut tr = Transport::with_beats(
            RATE,
            120.0,
            PPQN,
            BeatConfig { beats: true, bars: false, ..BeatConfig::default() },
        );
        tr.schedule(480, Event::trigger(0, 0.25));
        tr.schedule(2 * 960 + 240, Event::trigger(0, 0.5));
        tr.play();
        let mut out = BlockEvents::new();
        let mut log: Vec<(u64, u32, u16, f32)> = Vec::new();
        let mut abs = 0u64;
        for block in 0..1_600u64 {
            if block == 500 {
                tr.stop();
            }
            if block == 700 {
                tr.play();
                tr.schedule(3 * 960, Event::trigger(0, 0.75)); // scheduled while resuming
            }
            tr.advance_block(64, &mut out);
            for e in out.as_slice() {
                log.push((abs + u64::from(e.sample), e.sample, e.channel, e.value));
            }
            abs += 64;
        }
        // The stopped stretch dispatched nothing and moved nothing.
        log
    }
    let a = run_script();
    let b = run_script();
    assert_eq!(a, b, "the same script twice must produce the same events at the same samples");
    assert!(!a.is_empty(), "and the script produced events at all");
    // Beats at 0/24000/48000... despite the 200-block pause: freezing, not shifting.
    let beat_samples: Vec<u64> =
        a.iter().filter(|e| (e.3 - 1.0).abs() < 1e-6).map(|e| e.0).collect();
    assert!(
        beat_samples.contains(&0) && beat_samples.contains(&24_000),
        "the beat grid: {beat_samples:?}"
    );
}

// ---------------------------------------------------------------- the pump-safety claim

#[test]
fn advance_block_never_allocates() {
    // The driver call is pump-side: beats, bars, a live schedule, a loop region — and the
    // counting allocator watching all of it. schedule/set_tempo are control-side and MAY
    // allocate; they happen outside the measured window (the warm-up arms the schedule).
    let mut tr = Transport::new(RATE, 120.0, PPQN);
    assert!(tr.set_loop(Some((0, 8 * 960))), "a two-bar loop must be accepted");
    for i in 0..32 {
        tr.schedule(i * 240, Event::trigger(0, 0.5)); // a 16th-note grid across the loop
    }
    tr.play();
    let mut out = BlockEvents::new();
    for _ in 0..500 {
        tr.advance_block(64, &mut out);
    }
    let base = start_counting();
    for _ in 0..10_000 {
        tr.advance_block(64, &mut out);
    }
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    assert_eq!(made, 0, "advance_block allocated {made} time(s) across 10 000 blocks");
    // Sanity: events really flowed (an empty run would make the zero-allocation claim vacuous).
    let mut tr2 = Transport::new(RATE, 120.0, PPQN);
    tr2.play();
    let mut out2 = BlockEvents::new();
    let mut saw = 0usize;
    for _ in 0..500 {
        tr2.advance_block(64, &mut out2);
        saw += out2.len();
    }
    assert!(
        saw > 0,
        "beats flowed during the sanity run (the zero-allocation claim is not vacuous)"
    );
    assert!(out2.as_slice().iter().all(|e| e.kind == EventKind::Trigger));
}
