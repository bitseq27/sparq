//! Operator round 4 (2026-10-02) module gates: the four UPGRADED first-party modules, measured
//! the way the batch files established — exact values, not approximately-right windows.
//!
//! The rules this file proves, in words:
//!
//! * **D4, the clock's `phase`** — the quarter grid's fractional position, published per block
//!   from the SAME counter the 4th ticks fire on: wrap-correct at the beat boundary (the block
//!   whose edge is the downbeat reads 0, never 1 — the trap the ruling named), exactly
//!   `(t mod interval) / interval` between beats, and the 0.5 crossing lands in the block the
//!   8th tick fires in. The tick lists stand: with and without the phase sink presented, the
//!   four grids fire the identical samples (the publication changes no timing, and a module
//!   run outside its manifest's shape publishes nowhere — the house rule).
//! * **D5, the sequencer's `step`** — the ring cursor after this block's advances, as
//!   `cursor / steps`: the walking lights' source. Sink-gated like everything the seq
//!   publishes: no event-out presented, no advance AND no publication.
//! * **D13, the rms `slew`** — a one-pole over the follower's memory: default 0 is the RAW
//!   block value BIT-EXACTLY (the identity branch — the goldens unmoved, and a pre-0.2 patch
//!   that never heard of `slew` renders the identical samples via the out-of-range param
//!   read), above 0 the publication lags by the exact recurrence. The module is stateful now:
//!   the 4-byte memory round-trips through `configure`, and the legacy EMPTY blob still
//!   restores (an old save must not fail).
//! * **D14, the delay `sync`** — a rising trigger/clock event dumps both lines' tails AT THE
//!   EVENT'S SAMPLE: outputs before the sample are the no-sync outputs bit-exactly, outputs
//!   from the sample on are exact zeros while the un-synced twin keeps echoing. Gate-offs are
//!   ignored, and an unwired `sync` is byte-for-byte the 0.2.0 module (the append changed no
//!   index and no sample).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_module_api::event::{Event, EventBuf};
use sparq_module_api::module::{AudioCtx, CvOut, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

fn registry() -> Registry {
    let mut r = Registry::new();
    register_builtins(&mut r).unwrap();
    r
}

fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

// --------------------------------------------------------------------------- mod/clk phase (D4)

/// Run the clock for `blocks` blocks at `bpm`, presenting the four event sinks and — when
/// `with_phase` — the block-rate phase cell. Returns (tick samples per division, phase per
/// block). The run is the mod_clk_seq harness, extended by one publication.
fn clock_run(bpm: f32, blocks: u64, with_phase: bool) -> ([Vec<u64>; 4], Vec<f32>) {
    let reg = registry();
    let mut clk = reg.create("sparq/mod/clk").unwrap();
    let block0 = BlockContext::offline(RATE, FRAMES, 0);
    clk.prepare(&Resources::from_block(&block0)).unwrap();
    let params = ParamSet::new(1, &[bpm]).unwrap();
    let mut ticks: [Vec<u64>; 4] = Default::default();
    let mut phases = Vec::new();
    for b in 0..blocks {
        let mut block = BlockContext::offline(RATE, FRAMES, 1);
        block.sample_offset = b * FRAMES as u64;
        let (mut b0, mut b1, mut b2, mut b3) =
            (EventBuf::new(), EventBuf::new(), EventBuf::new(), EventBuf::new());
        let mut cell = f32::NAN;
        {
            let mut ctx = AudioCtx::new(&block, &params)
                .with_event_out(&mut b0)
                .with_event_out(&mut b1)
                .with_event_out(&mut b2)
                .with_event_out(&mut b3);
            if with_phase {
                ctx = ctx.with_cv_out(CvOut::Block(&mut cell));
            }
            clk.process(&mut ctx);
        }
        if with_phase {
            phases.push(cell);
        }
        let base = block.sample_offset;
        for (k, buf) in
            [b0.as_slice(), b1.as_slice(), b2.as_slice(), b3.as_slice()].iter().enumerate()
        {
            for ev in *buf {
                ticks[k].push(base + u64::from(ev.sample));
            }
        }
    }
    (ticks, phases)
}

#[test]
fn the_clock_phase_is_wrap_correct_on_the_quarter_grid() {
    // 120 BPM at 48 kHz: the quarter interval is 24 000 samples. The phase at the END of
    // block b is exactly `(t mod 24 000) / 24 000` with t = 64(b+1) — including the beat
    // boundary, where the naive `(interval − until) / interval` reads 1.0 (trap #9) and the
    // wrap-correct modulo reads 0.
    let (ticks, phases) = clock_run(120.0, 800, true);
    assert_eq!(ticks[0][0], 0, "the downbeat grid is the pinned one");
    assert_eq!(ticks[0][1], 24_000);
    for (b, &p) in phases.iter().enumerate() {
        let t = (b as u64 + 1) * FRAMES as u64;
        let want = ((t % 24_000) as f64 / 24_000.0) as f32;
        assert_eq!(p, want, "block {b} (t = {t}) publishes the exact wrapped phase");
        assert!((0.0..1.0).contains(&p), "the phase never leaves [0, 1)");
    }
    // The boundary block itself: t = 24 000 is block 374's end — phase 0, NOT 1.
    assert_eq!(phases[374], 0.0, "the beat-boundary block reads 0 (the trap the ruling named)");
    assert_eq!(phases[374 * 2 + 1], 0.0, "…and every downbeat edge after it");
}

#[test]
fn the_clock_phase_rides_the_same_counter_as_the_ticks() {
    // The 8th grid fires at 12 000, inside the block that ends at 12 032 — the same block
    // where the quarter phase first crosses 0.5. One counter, two publications: they cannot
    // disagree, and this is the measurement that says so.
    let (ticks, phases) = clock_run(120.0, 400, true);
    // All four grids start together at the downbeat, so the 8th's SECOND tick is the one at
    // 12 000 — the first crossing of the quarter's halfway point.
    let eighth = ticks[1][1];
    assert_eq!(eighth, 12_000, "the pinned 8th tick");
    let tick_block = (eighth / FRAMES as u64) as usize; // the block CONTAINING sample 12 000
    let cross = phases.iter().position(|&p| p >= 0.5).unwrap();
    assert_eq!(
        cross, tick_block,
        "the phase crosses 0.5 in the block the 8th tick fires in (block {tick_block})"
    );
    assert!(phases[cross - 1] < 0.5, "and not a block earlier");
    // A tempo edit re-spaces the grid from the next tick; the phase stays wrapped and honest
    // (until can exceed the NEW interval — the euclidean remainder is what keeps it in range).
    let reg = registry();
    let mut clk = reg.create("sparq/mod/clk").unwrap();
    let block0 = BlockContext::offline(RATE, FRAMES, 0);
    clk.prepare(&Resources::from_block(&block0)).unwrap();
    let mut cell = f32::NAN;
    for b in 0..900u64 {
        let mut block = BlockContext::offline(RATE, FRAMES, 1);
        block.sample_offset = b * FRAMES as u64;
        let params = ParamSet::new(1, &[if b < 400 { 120.0 } else { 240.0 }]).unwrap();
        let mut b0 = EventBuf::new();
        let mut ctx = AudioCtx::new(&block, &params)
            .with_event_out(&mut b0)
            .with_cv_out(CvOut::Block(&mut cell));
        // the other three sinks are simply not presented this run: sink-gated, counted nowhere
        clk.process(&mut ctx);
        assert!((0.0..1.0).contains(&cell), "block {b}: the phase stays in [0, 1) across the edit");
    }
}

#[test]
fn the_clock_tick_lists_stand_with_and_without_the_phase_sink() {
    // The publication is sink-gated and side-effect-free: the four grids fire the IDENTICAL
    // samples whether or not the phase port is presented — "appended, so every event index
    // stands" (the goldens' and mod_clk_seq's pin), measured.
    let (with, phases) = clock_run(120.0, 200, true);
    let (without, none) = clock_run(120.0, 200, false);
    assert_eq!(with, without, "the tick lists stand");
    assert!(none.is_empty(), "an unpresented port publishes nowhere");
    assert_eq!(phases.len(), 200, "the presented one publishes every block");
}

// --------------------------------------------------------------------------- mod/seq step (D5)

#[test]
fn the_seq_publishes_the_cursor_after_the_blocks_advances() {
    let reg = registry();
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    seq.prepare(&Resources::from_block(&block)).unwrap();
    // 8 steps, mask irrelevant to the cursor: three qualifying triggers in block 0…
    let params = ParamSet::new(1, &[8.0, 0.0]).unwrap();
    let events = [
        Event::trigger(5, 1.0),
        Event::trigger(20, 0.0),
        Event::trigger(40, 1.0),
        Event::trigger(50, 1.0),
    ];
    let mut buf = EventBuf::new();
    let mut cell = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&events)
            .with_event_out(&mut buf)
            .with_cv_out(CvOut::Block(&mut cell));
        seq.process(&mut ctx);
    }
    // …one gate-off among them: the cursor counts the three pulses, and the publication is
    // cursor/steps — exactly the value the walking lights floor back into a cell index.
    assert_eq!(cell, 3.0 / 8.0, "the cursor after the block's advances");
    // A block with no triggers holds the cursor (the publication is the position, not motion).
    let mut buf2 = EventBuf::new();
    let mut cell2 = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&[])
            .with_event_out(&mut buf2)
            .with_cv_out(CvOut::Block(&mut cell2));
        seq.process(&mut ctx);
    }
    assert_eq!(cell2, cell, "a silent block re-publishes the held cursor");
    // Two more pulses walk it to 5/8.
    let two = [Event::trigger(10, 1.0), Event::trigger(30, 1.0)];
    let mut buf3 = EventBuf::new();
    let mut cell3 = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&two)
            .with_event_out(&mut buf3)
            .with_cv_out(CvOut::Block(&mut cell3));
        seq.process(&mut ctx);
    }
    assert_eq!(cell3, 5.0 / 8.0, "the cursor walks, wrapping at `steps`");
}

#[test]
fn the_seq_cursor_stays_sink_gated_with_the_publication_behind_it() {
    // No event-out presented: the cursor does not advance (the house rule) AND the step cell
    // is not written — the publication sits behind the same gate as the counting.
    let reg = registry();
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    seq.prepare(&Resources::from_block(&block)).unwrap();
    let params = ParamSet::new(1, &[8.0, 1.0]).unwrap(); // mask 0b1: step 0 fires
    let events = [Event::trigger(5, 1.0), Event::trigger(20, 1.0), Event::trigger(40, 1.0)];
    let mut cell = -1.0f32; // a sentinel the module must not touch
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&events)
            .with_cv_out(CvOut::Block(&mut cell));
        seq.process(&mut ctx);
    }
    assert_eq!(cell, -1.0, "an unpresented event out publishes the step nowhere");
    // …and the three unheard pulses advanced nothing: with the sink presented again and mask
    // 0b1, the NEXT pulse fires step 0. Had the gated pulses counted, the walk would stand at
    // step 3 and this pulse would fire nothing — the silence would be the lie.
    let one = [Event::trigger(9, 1.0)];
    let mut buf = EventBuf::new();
    let mut cell2 = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&one)
            .with_event_out(&mut buf)
            .with_cv_out(CvOut::Block(&mut cell2));
        seq.process(&mut ctx);
    }
    assert_eq!(buf.as_slice().len(), 1, "the walk resumed at step 0 and fired its bit");
    assert_eq!(cell2, 1.0 / 8.0, "and the cursor says so");
}

// --------------------------------------------------------------------------- ana/rms slew (D13)

/// Run an rms over `blocks` blocks of a constant-amplitude input, returning the published
/// level per block. `params` is the whole snapshot (a 1-value snapshot exercises the
/// out-of-range `slew` read; a 2-value one the declared param).
fn rms_run(params: &[f32], amp: f32, blocks: usize) -> Vec<f32> {
    let reg = registry();
    let mut rms = reg.create("sparq/ana/rms").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    rms.prepare(&Resources::from_block(&block)).unwrap();
    let p = ParamSet::new(1, params).unwrap();
    let input = vec![amp; FRAMES * 2];
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut levels = Vec::new();
    for _ in 0..blocks {
        let mut cell = f32::NAN;
        {
            let mut ctx =
                AudioCtx::single(&block, &p, &input, &mut out).with_cv_out(CvOut::Block(&mut cell));
            rms.process(&mut ctx);
        }
        levels.push(cell);
    }
    levels
}

#[test]
fn rms_slew_zero_is_the_raw_block_bit_exactly() {
    // The constant input's rms is exactly 0.5 (0.25 sums, /n, sqrt — f64 all the way), so the
    // raw publication is the pinned number, and BOTH shapes of "no slew" agree with it:
    // the explicit 0 and the pre-0.2 one-param snapshot (whose out-of-range read is 0.0 —
    // appended params are safe for old shapes, the trap the handoff named).
    let explicit = rms_run(&[0.0, 0.0], 0.5, 8);
    let legacy = rms_run(&[0.0], 0.5, 8);
    assert!(explicit.iter().all(|&l| l == 0.5), "raw is exact: {explicit:?}");
    assert_eq!(explicit, legacy, "the legacy snapshot renders the identical samples");
}

#[test]
fn rms_slew_lags_by_the_exact_one_pole_recurrence() {
    // out = held + (raw − held) × (1 − slew), held following the publication: at slew 0.5 on a
    // constant raw 0.5 the sequence is 0.25, 0.375, 0.4375, … — hand-computed in the SAME f32
    // arithmetic, compared exactly, monotone and bounded by the raw level.
    let slew = 0.5f32;
    let raw = 0.5f32;
    let levels = rms_run(&[0.0, slew], raw, 10);
    let mut held = 0.0f32;
    for (b, &l) in levels.iter().enumerate() {
        held += (raw - held) * (1.0 - slew);
        assert_eq!(l, held, "block {b} follows the declared recurrence exactly");
    }
    assert!(levels.windows(2).all(|w| w[1] > w[0]), "the lag approaches from below");
    assert!(levels.iter().all(|&l| l < raw), "…and never overshoots the raw level");
    assert!(levels[9] > 0.49, "ten blocks at 0.5 slew are most of the way there: {}", levels[9]);
}

#[test]
fn the_rms_memory_round_trips_and_the_legacy_empty_blob_still_restores() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let p = ParamSet::new(1, &[0.0, 0.5]).unwrap();
    let input = vec![0.5f32; FRAMES * 2];
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut publish = |m: &mut Box<dyn sparq_module_api::module::Module>| {
        let mut cell = f32::NAN;
        let mut ctx =
            AudioCtx::single(&block, &p, &input, &mut out).with_cv_out(CvOut::Block(&mut cell));
        m.process(&mut ctx);
        cell
    };
    // A restored memory of 0.9: the first publication is the recurrence FROM 0.9, not from 0.
    let mut restored = reg.create("sparq/ana/rms").unwrap();
    restored.configure(&0.9f32.to_le_bytes()).unwrap();
    restored.prepare(&Resources::from_block(&block)).unwrap();
    assert_eq!(publish(&mut restored), 0.9 + (0.5 - 0.9) * 0.5, "the memory is the state");
    // The legacy EMPTY blob (a pre-0.2 save — the follower was stateless): accepted, memory
    // starts empty. An old project restores instead of failing at the door.
    let mut legacy = reg.create("sparq/ana/rms").unwrap();
    legacy.configure(&[]).unwrap();
    legacy.prepare(&Resources::from_block(&block)).unwrap();
    assert_eq!(publish(&mut legacy), 0.25, "empty blob = fresh memory");
    // A wrong-sized blob is refused in words (the state protocol's honesty).
    let mut bad = reg.create("sparq/ana/rms").unwrap();
    let err = bad.configure(&[1, 2, 3]).unwrap_err();
    assert!(err.to_string().contains("4 bytes"), "{err}");
}

// --------------------------------------------------------------------------- util/delay sync (D14)

/// Render `blocks` blocks of the delay with a hot first block and silent input after; when
/// `sync_at` is Some((block, sample)), a rising trigger lands there. Returns every output
/// block, concatenated. Stereo throughout, wet-only (mix 1) so the tail is the whole signal.
fn delay_run(blocks: usize, sync_at: Option<(usize, u32, f32)>) -> Vec<f32> {
    let reg = registry();
    let mut d = reg.create("sparq/util/delay").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    d.prepare(&Resources::from_block(&block)).unwrap();
    // time 5 ms (240 samples), feedback 0.8, no damping, WET ONLY, ms mode.
    let p = ParamSet::new(1, &[5.0, 0.8, 0.0, 1.0, 0.0, 0.5]).unwrap();
    let hot = vec![0.5f32; FRAMES * 2];
    let cold = vec![0.0f32; FRAMES * 2];
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut all = Vec::new();
    for b in 0..blocks {
        let input: &[f32] = if b == 0 { &hot } else { &cold };
        let evs;
        let mut ctx = AudioCtx::single(&block, &p, input, &mut out);
        if let Some((sb, sample, value)) = sync_at {
            if sb == b {
                evs = [Event::trigger(sample, value)];
                ctx = ctx.with_events_in(&evs);
            }
        }
        d.process(&mut ctx);
        all.extend_from_slice(&out);
    }
    all
}

#[test]
fn a_sync_trigger_dumps_the_tail_at_its_exact_sample() {
    let plain = delay_run(8, None);
    let synced = delay_run(8, Some((3, 20, 1.0)));
    let at = 3 * FRAMES * 2 + 20 * 2; // block 3, frame 20, interleaved stereo: both channels
                                      // BEFORE the event's sample: bit-exactly the un-synced render — the dump is sample-accurate,
                                      // not block-accurate; the echo that was already in the air stays in the air.
    assert_eq!(&plain[..at], &synced[..at], "the samples before the event are untouched");
    // FROM the event's sample on: exact zeros while the input stays silent — the tail is gone.
    assert!(
        synced[at..].iter().all(|&s| s == 0.0),
        "the dumped line echoes nothing: {:?}",
        &synced[at..at + 8]
    );
    // …and the twin really was still echoing (the gate is not vacuous).
    assert!(
        plain[at..].iter().any(|&s| s != 0.0),
        "without the sync the 0.8-feedback tail rings on"
    );
}

#[test]
fn gate_offs_are_ignored_and_an_unwired_sync_changes_no_sample() {
    let plain = delay_run(6, None);
    // A gate-off (value 0) on the sync port: the counting rule — not a pulse, no dump.
    let gated = delay_run(6, Some((3, 20, 0.0)));
    assert_eq!(plain, gated, "a gate-off is not a sync");
    // The unwired port is the identity branch: `delay_run(6, None)` presents no events at all,
    // and its output is bit-exactly the gated run's — the 0.3.0 append moved no sample of the
    // 0.2.0 module (which the module's own golden keeps pinning from the other side).
    let negative = delay_run(6, Some((3, 20, -1.0)));
    assert_eq!(plain, negative, "a negative event is not a sync either");
}

#[test]
fn the_round_4_upgrades_allocate_nothing_in_process() {
    // The batch discipline, on the four upgraded modules with their new publications: the
    // phase cell, the step cell, the slew memory and the sync dump all run inside the
    // audio-thread contract — measured, not commented.
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut clk = reg.create("sparq/mod/clk").unwrap();
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    let mut rms = reg.create("sparq/ana/rms").unwrap();
    let mut delay = reg.create("sparq/util/delay").unwrap();
    for m in [&mut clk, &mut seq, &mut rms, &mut delay] {
        m.prepare(&res).unwrap();
    }
    let p_clk = ParamSet::new(1, &[120.0]).unwrap();
    let p_seq = ParamSet::new(1, &[8.0, 170.0]).unwrap();
    let p_rms = ParamSet::new(1, &[0.0, 0.3]).unwrap();
    let p_del = ParamSet::new(1, &[5.0, 0.8, 0.0, 1.0, 0.0, 0.5]).unwrap();
    let events = [Event::trigger(0, 1.0), Event::trigger(37, 0.8)];
    let audio_in = vec![0.25f32; FRAMES * 2];
    let mut audio_out = vec![0.0f32; FRAMES * 2];
    let mut e0 = EventBuf::new();
    let mut e1 = EventBuf::new();
    let mut cell_a = 0.0f32;
    let mut cell_b = 0.0f32;
    let made = measure(|| {
        for _ in 0..2_000 {
            {
                let mut ctx = AudioCtx::new(&block, &p_clk)
                    .with_event_out(&mut e0)
                    .with_cv_out(CvOut::Block(&mut cell_a));
                clk.process(&mut ctx);
                e0.clear();
            }
            {
                let mut ctx = AudioCtx::new(&block, &p_seq)
                    .with_events_in(&events)
                    .with_event_out(&mut e1)
                    .with_cv_out(CvOut::Block(&mut cell_b));
                seq.process(&mut ctx);
                e1.clear();
            }
            {
                let mut ctx = AudioCtx::single(&block, &p_rms, &audio_in, &mut audio_out)
                    .with_cv_out(CvOut::Block(&mut cell_a));
                rms.process(&mut ctx);
            }
            {
                let mut ctx = AudioCtx::single(&block, &p_del, &audio_in, &mut audio_out)
                    .with_events_in(&events);
                delay.process(&mut ctx);
            }
        }
    });
    assert_eq!(made, 0, "a round-4 upgrade allocated {made} time(s) inside process");
}
