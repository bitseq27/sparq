//! Operator round 4 (2026-10-02) NEW-module gates: `fx/fold` (D8), `util/quant` (D11) and
//! `mod/rand` (D12), measured the way the batch files established — hand-computed arithmetic,
//! exact compares, pinned constants, zero allocations counted.
//!
//! The rules this file proves, in words:
//!
//! * **the seed-42 ring is PINNED** — `step_hash` is pure integer arithmetic (the lowbias32
//!   finalizer of the splitmix32 family over `seed ^ (i × 0x9E37_79B9)`, top 24 bits), and the
//!   ring it produces for seed 42 is the checked-in list from the round-4 handoff. The card's
//!   step-bars display mirrors this function under the same pin, so the bars can never drift
//!   from the module.
//! * **rand walks, echoes, gates and restores** — triggers advance the cursor and echo at the
//!   input's sample; value/pos publish the post-advance cursor; the walk is sink-gated (the
//!   seq rule); the 8-byte cursor round-trips and `reset` rewinds it.
//! * **fold at amount 0 is a wire** — the identity branch, bit-exact; `fold_tri` is the
//!   declared triangle (pinned values, continuity, odd symmetry, range ≤ 1); a driven fold
//!   stays in range and grows crossings (the bloom is real).
//! * **quant snaps to the nearest scale semitone, ties down** — chromatic pins, the standard
//!   fifteen are the declared table (names AND masks pinned), the Custom mask is the
//!   keyboard's, an EMPTY mask passes through (never snaps to a note the user turned off),
//!   and the trigger input samples the input AT THE EVENT for that block's word.
//! * **the span is 120 semitones** — the publication clamps into 0..1; the display side's
//!   mirrored constant is pinned to the same number from its own file.
//! * **the registry carries twenty-four** — the round's count, with every new id present and
//!   every factory honest about its id.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::modules::{
    fold_tri, register_builtins, snap_to_scale, step_hash, step_value, BUILTINS,
    QUANT_PITCH_SEMITONES, QUANT_SCALES,
};
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_module_api::event::{Event, EventBuf};
use sparq_module_api::module::{AudioCtx, CvIn, CvOut, Resources};
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

// --------------------------------------------------------------------------- mod/rand (D12)

/// The round-4 handoff's checked-in ring: `step_hash(42, i) >> 8` for i in 0..8, over 2²⁴.
const SEED42_RING: [u32; 8] =
    [1517363, 10542902, 13721998, 8088911, 6531285, 16292829, 12864735, 5502068];

#[test]
fn the_seed_42_ring_is_pinned_to_the_exact_integer_hash() {
    for (i, want) in SEED42_RING.iter().enumerate() {
        let h = step_hash(42, i as u32);
        assert_eq!(h >> 8, *want, "step_hash(42, {i}) top 24 bits");
        assert_eq!(
            step_value(42, i as u32),
            *want as f32 / 16_777_216.0,
            "step_value is the ring over 2^24"
        );
        assert_eq!(step_hash(42, i as u32), h, "the hash is pure: same in, same out");
    }
    // Different seeds walk different rings (a seed that changed nothing would be a lie), and
    // the index mixing is the golden-ratio stride: i = 0 reads the seed alone.
    assert_ne!(step_hash(42, 0), step_hash(43, 0), "the seed voices the ring");
    assert_eq!(step_hash(42, 0), step_hash(42, 0), "…deterministically");
    let mut seen = std::collections::HashSet::new();
    for i in 0..64u32 {
        assert!(seen.insert(step_hash(42, i)), "ring value {i} repeats within 64 steps");
    }
}

#[test]
fn rand_walks_the_ring_on_triggers_and_publishes_cursor_and_value() {
    let reg = registry();
    let mut rand = reg.create("sparq/mod/rand").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    rand.prepare(&Resources::from_block(&block)).unwrap();
    // 8 steps, seed 42 — the pinned ring.
    let params = ParamSet::new(1, &[8.0, 42.0]).unwrap();
    let events = [Event::trigger(10, 1.0), Event::trigger(30, 0.0), Event::trigger(40, 1.0)];
    let mut sink = EventBuf::new();
    let (mut value, mut pos) = (f32::NAN, f32::NAN);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&events)
            .with_event_out(&mut sink)
            .with_cv_out(CvOut::Block(&mut value))
            .with_cv_out(CvOut::Block(&mut pos));
        rand.process(&mut ctx);
    }
    // Two qualifying triggers (the gate-off between them is not a pulse): the cursor walks
    // 0→2, the echo carries BOTH pulses at the INPUT's samples, and the publications are the
    // post-advance cursor's ring value and position.
    let echoed: Vec<u32> = sink.as_slice().iter().map(|e| e.sample).collect();
    assert_eq!(echoed, [10, 40], "the trigger out echoes the qualifying pulses");
    assert_eq!(value, step_value(42, 2), "value publishes the walked-to step");
    assert_eq!(pos, 2.0 / 8.0, "pos publishes the cursor");
    // A silent block holds the cursor (the publication is the position, not motion)…
    let mut sink2 = EventBuf::new();
    let (mut v2, mut p2) = (f32::NAN, f32::NAN);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&[])
            .with_event_out(&mut sink2)
            .with_cv_out(CvOut::Block(&mut v2))
            .with_cv_out(CvOut::Block(&mut p2));
        rand.process(&mut ctx);
    }
    assert!(sink2.as_slice().is_empty(), "no pulse, no echo");
    assert_eq!((v2, p2), (value, pos), "the held cursor re-publishes");
    // …and the ring WRAPS at `steps`: seven more pulses land the cursor on 1.
    let seven: Vec<Event> = (0..7).map(|i| Event::trigger(i * 8, 1.0)).collect();
    let mut sink3 = EventBuf::new();
    let (mut v3, mut p3) = (f32::NAN, f32::NAN);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&seven)
            .with_event_out(&mut sink3)
            .with_cv_out(CvOut::Block(&mut v3))
            .with_cv_out(CvOut::Block(&mut p3));
        rand.process(&mut ctx);
    }
    assert_eq!(sink3.as_slice().len(), 7);
    assert_eq!(v3, step_value(42, 1), "(2 + 7) mod 8 = 1");
    assert_eq!(p3, 1.0 / 8.0);
}

#[test]
fn rand_is_sink_gated_and_its_cursor_round_trips() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    let params = ParamSet::new(1, &[8.0, 42.0]).unwrap();
    let events = [Event::trigger(5, 1.0), Event::trigger(20, 1.0), Event::trigger(40, 1.0)];
    // No event-out presented: no advance, no publication (the sentinels stay) — the seq rule.
    let mut rand = reg.create("sparq/mod/rand").unwrap();
    rand.prepare(&Resources::from_block(&block)).unwrap();
    let (mut value, mut pos) = (-1.0f32, -1.0f32);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&events)
            .with_cv_out(CvOut::Block(&mut value))
            .with_cv_out(CvOut::Block(&mut pos));
        rand.process(&mut ctx);
    }
    assert_eq!((value, pos), (-1.0, -1.0), "an unpresented event out publishes nothing");
    // The unheard pulses advanced nothing: with the sink back, the first pulse echoes and the
    // cursor reads 1 — not 4.
    let one = [Event::trigger(9, 1.0)];
    let mut sink = EventBuf::new();
    let (mut v2, mut p2) = (f32::NAN, f32::NAN);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&one)
            .with_event_out(&mut sink)
            .with_cv_out(CvOut::Block(&mut v2))
            .with_cv_out(CvOut::Block(&mut p2));
        rand.process(&mut ctx);
    }
    assert_eq!(sink.as_slice().len(), 1);
    assert_eq!(p2, 1.0 / 8.0, "the gated pulses counted nowhere");
    // The state blob is the 8-byte cursor: a restored ring resumes mid-walk, `reset` rewinds
    // it, and a wrong-sized blob is refused in words.
    let mut restored = reg.create("sparq/mod/rand").unwrap();
    restored.configure(&5u64.to_le_bytes()).unwrap();
    restored.prepare(&Resources::from_block(&block)).unwrap();
    let mut sink2 = EventBuf::new();
    let (mut v3, mut p3) = (f32::NAN, f32::NAN);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&one)
            .with_event_out(&mut sink2)
            .with_cv_out(CvOut::Block(&mut v3))
            .with_cv_out(CvOut::Block(&mut p3));
        restored.process(&mut ctx);
    }
    assert_eq!(p3, 6.0 / 8.0, "restored at 5, one pulse walks to 6");
    restored.message(b"reset").unwrap();
    let mut sink3 = EventBuf::new();
    let (mut v4, mut p4) = (f32::NAN, f32::NAN);
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_events_in(&[])
            .with_event_out(&mut sink3)
            .with_cv_out(CvOut::Block(&mut v4))
            .with_cv_out(CvOut::Block(&mut p4));
        restored.process(&mut ctx);
    }
    assert_eq!(p4, 0.0, "reset rewinds the walk");
    assert_eq!(v4, step_value(42, 0));
    let mut bad = reg.create("sparq/mod/rand").unwrap();
    let err = bad.configure(&[1, 2, 3]).unwrap_err();
    assert!(err.to_string().contains("8 bytes"), "{err}");
}

// --------------------------------------------------------------------------- fx/fold (D8)

#[test]
fn fold_at_zero_amount_is_a_bit_exact_wire() {
    let reg = registry();
    let mut fold = reg.create("sparq/fx/fold").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    fold.prepare(&Resources::from_block(&block)).unwrap();
    let params = ParamSet::new(1, &[0.0]).unwrap();
    // A shaped, in-range input: the identity branch must reproduce it sample for sample.
    let input: Vec<f32> = (0..FRAMES * 2).map(|i| ((i * 37 % 200) as f32 / 100.0) - 1.0).collect();
    let mut out = vec![0.0f32; FRAMES * 2];
    {
        let mut ctx = AudioCtx::single(&block, &params, &input, &mut out);
        fold.process(&mut ctx);
    }
    assert_eq!(out, input, "amount 0 is a wire — every sample bit-exact");
    // No input: silence and the honest status (the gain/vca family's rule).
    let mut silent = vec![9.9f32; FRAMES * 2];
    {
        let mut ctx = AudioCtx::single(&block, &params, &[], &mut silent);
        assert!(matches!(fold.process(&mut ctx), sparq_module_api::module::BlockStatus::Silenced));
    }
    assert!(silent.iter().all(|&s| s == 0.0), "unconnected in, exact zeros out");
}

#[test]
fn fold_tri_is_the_declared_triangle() {
    // The pinned values of the manifest header: |x| ≤ 1 passes, the rails reflect.
    let pinned = [
        (0.0f32, 0.0f32),
        (1.0, 1.0),
        (-1.0, -1.0),
        (0.5, 0.5),
        (1.5, 0.5),
        (2.0, 0.0),
        (3.0, -1.0),
        (4.0, 0.0),
        (5.0, 1.0),
        (-1.5, -0.5),
        (-2.0, 0.0),
        (-3.0, 1.0),
    ];
    for (x, want) in pinned {
        assert!((fold_tri(x) - want).abs() < 1e-6, "fold_tri({x}) = {want}");
    }
    // Period 4, range ≤ 1, odd-symmetric, and CONTINUOUS: a fold reflects, it never jumps.
    let mut prev = fold_tri(-8.0);
    let step = 16.0 / 4000.0;
    for i in 1..=4000 {
        let x = -8.0 + i as f32 * step;
        let y = fold_tri(x);
        assert!(y.abs() <= 1.0 + 1e-6, "range at {x}: {y}");
        assert!((y - prev).abs() <= step + 1e-5, "continuous at {x}: {prev} → {y}");
        assert!((fold_tri(-x) + y).abs() < 1e-5, "odd-symmetric at {x}");
        assert!((fold_tri(x + 4.0) - y).abs() < 1e-5, "period 4 at {x}");
        prev = y;
    }
}

#[test]
fn a_driven_fold_stays_in_range_and_grows_the_harmonics() {
    let reg = registry();
    let mut fold = reg.create("sparq/fx/fold").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    fold.prepare(&Resources::from_block(&block)).unwrap();
    let params = ParamSet::new(1, &[1.0]).unwrap(); // drive 8: three and a half full folds
                                                    // One slow cycle of a 0.9-amplitude sine, stereo.
    let input: Vec<f32> = (0..FRAMES * 2)
        .map(|i| {
            let t = (i % FRAMES) as f32 / FRAMES as f32;
            0.9 * (t * std::f32::consts::TAU).sin()
        })
        .collect();
    let mut out = vec![0.0f32; FRAMES * 2];
    {
        let mut ctx = AudioCtx::single(&block, &params, &input, &mut out);
        fold.process(&mut ctx);
    }
    assert!(out.iter().all(|&s| s.abs() <= 1.0 + 1e-6), "the fold never leaves the rails");
    assert_ne!(out, input, "drive 8 is not a wire");
    let crossings = |v: &[f32]| {
        v.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0) && w[0] != 0.0 && w[1] != 0.0).count()
    };
    assert!(
        crossings(&out) > crossings(&input) * 3,
        "the folded wave crosses far more often — the bloom is real ({} vs {})",
        crossings(&out),
        crossings(&input)
    );
}

// --------------------------------------------------------------------------- util/quant (D11)

/// Run one quant block over a block-rate pitch word (no triggers) and return the publication.
fn quant_block(scale: f32, mask: f32, pitch: f32) -> f32 {
    let reg = registry();
    let mut q = reg.create("sparq/util/quant").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    q.prepare(&Resources::from_block(&block)).unwrap();
    let params = ParamSet::new(1, &[scale, mask]).unwrap();
    let mut cell = f32::NAN;
    let mut ctx = AudioCtx::new(&block, &params)
        .with_cv_in(CvIn::Block(pitch))
        .with_cv_out(CvOut::Block(&mut cell));
    q.process(&mut ctx);
    cell
}

#[test]
fn quant_snaps_to_the_nearest_semitone_with_ties_down() {
    // Chromatic (Custom with all twelve bits): the semitone grid, exact.
    assert_eq!(quant_block(0.0, 4095.0, 0.5), 0.5, "semitone 60 is on the grid");
    let want61 = (61.0f64 / 120.0) as f32;
    assert_eq!(quant_block(0.0, 4095.0, 60.6 / 120.0), want61, "60.6 rounds up to 61");
    assert_eq!(quant_block(0.0, 4095.0, 60.4 / 120.0), 0.5, "60.4 rounds down to 60");
    // An exact tie resolves DOWN — the deterministic declared rule, pinned at the helper's
    // own f64 precision (an exact .5 semitone is unreachable through the f32 pitch word —
    // 120 is not a power of two — so the rule lives where the arithmetic is exact).
    for s in 0..12 {
        assert_eq!(snap_to_scale(s as f64 + 0.5, 0xFFF), s as f64, "the tie above {s} goes down");
    }
    // The same arithmetic through the public helper, hand-checked across an octave.
    for s in 0..12 {
        assert_eq!(snap_to_scale(s as f64, 0xFFF), s as f64, "grid points are fixed");
    }
}

#[test]
fn quant_scales_are_the_standard_fifteen() {
    // The ruling's table: names in the declared order, masks hand-computed from the semitone
    // sets the manifest header lists. One copy of the vocabulary, pinned here so a reorder
    // (which would silently revoice every patch that picked a scale by number) fails loudly.
    assert_eq!(QUANT_SCALES.len(), 15, "the standard fifteen");
    let want: [(&str, u32); 15] = [
        ("Chromatic", 4095),
        ("Major", 2741),
        ("Minor", 1453),
        ("Dorian", 1709),
        ("Phrygian", 1451),
        ("Lydian", 2773),
        ("Mixolydian", 1717),
        ("Locrian", 1387),
        ("Harmonic Minor", 2477),
        ("Melodic Minor", 2733),
        ("Major Pentatonic", 661),
        ("Minor Pentatonic", 1193),
        ("Blues", 1257),
        ("Whole Tone", 1365),
        ("Diminished", 2925),
    ];
    assert_eq!(QUANT_SCALES, want, "names AND masks, in order");
    // Behaviour through the param door: scale 2 (Major) snaps C# (semitone 61) down to C —
    // the Major mask has no class 1, and 61 is 1 away from 60 but 1 away from 62 too: tie, down.
    assert_eq!(quant_block(2.0, 0.0, 61.0 / 120.0), 0.5, "Major: 61 → 60 (tie down)");
    // D (62) is in Major and stays.
    assert_eq!(
        quant_block(2.0, 0.0, 62.0 / 120.0),
        (62.0f64 / 120.0) as f32,
        "Major: 62 is a scale tone"
    );
    // Custom with only C set: everything snaps to the nearest C — 65 semitones → 60.
    assert_eq!(quant_block(0.0, 1.0, 65.0 / 120.0), 0.5, "Custom mask holding only C: 65 → 60");
    // An EMPTY Custom mask passes the pitch through — never snaps to a note the user turned off.
    let through = 65.3f32 / 120.0;
    assert!(
        (quant_block(0.0, 0.0, through) - through).abs() < 1e-6,
        "the empty mask is a passthrough"
    );
}

#[test]
fn quant_triggers_sample_the_input_at_the_event() {
    // An audio-rate sweep in, block-rate word out. WITHOUT a trigger the publication is the
    // settled word (the last sample, the vca discipline); WITH triggers it is the input AT THE
    // LAST QUALIFYING TRIGGER'S SAMPLE — the block-scoped S&H the manifest declares.
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    let params = ParamSet::new(1, &[0.0, 4095.0]).unwrap(); // Custom chromatic
                                                            // A rising sweep: sample i reads i/FRAMES across the span (semitone i·120/64 = 1.875·i).
    let sweep: Vec<f32> = (0..FRAMES).map(|i| i as f32 / FRAMES as f32).collect();
    let mut q = reg.create("sparq/util/quant").unwrap();
    q.prepare(&Resources::from_block(&block)).unwrap();
    // Continuous: the settled word is the LAST sample (63/64 → semitone 118.125 → 118).
    let mut cell = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_cv_in(CvIn::Audio(&sweep))
            .with_cv_out(CvOut::Block(&mut cell));
        q.process(&mut ctx);
    }
    assert_eq!(cell, (118.0f64 / 120.0) as f32, "continuous mode publishes the settled word");
    // Triggered at sample 10 (semitone 18.75 → 19): the publication is the TRIGGER's sample.
    let trigs = [Event::trigger(10, 1.0), Event::trigger(20, 0.0)]; // the gate-off does not sample
    let mut q2 = reg.create("sparq/util/quant").unwrap();
    q2.prepare(&Resources::from_block(&block)).unwrap();
    let mut cell2 = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_cv_in(CvIn::Audio(&sweep))
            .with_events_in(&trigs)
            .with_cv_out(CvOut::Block(&mut cell2));
        q2.process(&mut ctx);
    }
    assert_eq!(
        cell2,
        (19.0f64 / 120.0) as f32,
        "the qualifying trigger's sample wins the block (10 × 1.875 = 18.75 → 19)"
    );
    // The LAST qualifying trigger of the block is the latch (two triggers: sample 40 wins).
    let two = [Event::trigger(10, 1.0), Event::trigger(40, 1.0)];
    let mut q3 = reg.create("sparq/util/quant").unwrap();
    q3.prepare(&Resources::from_block(&block)).unwrap();
    let mut cell3 = f32::NAN;
    {
        let mut ctx = AudioCtx::new(&block, &params)
            .with_cv_in(CvIn::Audio(&sweep))
            .with_events_in(&two)
            .with_cv_out(CvOut::Block(&mut cell3));
        q3.process(&mut ctx);
    }
    assert_eq!(cell3, (75.0f64 / 120.0) as f32, "40 × 1.875 = 75, on the grid");
}

#[test]
fn the_quant_span_is_120_semitones_and_the_word_clamps_into_range() {
    assert_eq!(QUANT_PITCH_SEMITONES, 120.0, "ten octaves — the display side pins the same");
    // The top of the span: semitone 120 is class 0 of the tenth octave — 1.0 in, 1.0 out.
    assert_eq!(quant_block(0.0, 4095.0, 1.0), 1.0);
    // Out-of-domain inputs clamp (a wild cv never publishes outside the declared unipolar word).
    assert_eq!(quant_block(0.0, 4095.0, 1.7), 1.0, "above the span clamps to the top");
    assert_eq!(quant_block(0.0, 4095.0, -0.4), 0.0, "below the span clamps to the bottom");
    // Every scale's publication stays in 0..1 across a sweep of inputs.
    for (si, _) in QUANT_SCALES.iter().enumerate() {
        for k in 0..=12 {
            let p = k as f32 / 12.0;
            let out = quant_block((si + 1) as f32, 0.0, p);
            assert!((0.0..=1.0).contains(&out), "scale {} at {p} → {out}", si + 1);
        }
    }
}

// --------------------------------------------------------------------------- the round's shape

#[test]
fn the_round_4_modules_allocate_nothing_in_process() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut fold = reg.create("sparq/fx/fold").unwrap();
    let mut quant = reg.create("sparq/util/quant").unwrap();
    let mut rand = reg.create("sparq/mod/rand").unwrap();
    for m in [&mut fold, &mut quant, &mut rand] {
        m.prepare(&res).unwrap();
    }
    let p_fold = ParamSet::new(1, &[0.7]).unwrap();
    let p_quant = ParamSet::new(1, &[0.0, 2741.0]).unwrap();
    let p_rand = ParamSet::new(1, &[8.0, 42.0]).unwrap();
    let input = vec![0.25f32; FRAMES * 2];
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut cv_cell_in = 0.4f32;
    let events = [Event::trigger(0, 1.0), Event::trigger(37, 0.8)];
    let mut sink = EventBuf::new();
    let (mut c1, mut c2) = (0.0f32, 0.0f32);
    let made = measure(|| {
        for _ in 0..5_000 {
            {
                let mut ctx = AudioCtx::single(&block, &p_fold, &input, &mut out);
                fold.process(&mut ctx);
            }
            {
                let mut ctx = AudioCtx::new(&block, &p_quant)
                    .with_cv_in(CvIn::Block(cv_cell_in))
                    .with_events_in(&events)
                    .with_cv_out(CvOut::Block(&mut c1));
                quant.process(&mut ctx);
            }
            {
                let mut ctx = AudioCtx::new(&block, &p_rand)
                    .with_events_in(&events)
                    .with_event_out(&mut sink)
                    .with_cv_out(CvOut::Block(&mut c1))
                    .with_cv_out(CvOut::Block(&mut c2));
                rand.process(&mut ctx);
                sink.clear();
            }
            cv_cell_in = (cv_cell_in + 0.001) % 1.0;
        }
    });
    assert_eq!(made, 0, "a round-4 module allocated {made} time(s) inside process");
}

#[test]
fn the_registry_carries_twenty_four_and_the_new_ids_answer_to_it() {
    assert_eq!(BUILTINS.len(), 24, "the round completes the set at twenty-four");
    let reg = registry(); // register_builtins asserts its own count against BUILTINS
    for id in ["sparq/fx/fold", "sparq/util/quant", "sparq/mod/rand"] {
        let entry = reg.get(id).unwrap_or_else(|| panic!("{id} is installed"));
        assert_eq!(entry.create().id(), id, "the factory is honest about its id");
        assert_eq!(entry.manifest().id(), id, "…and so is the manifest");
    }
    // The upgraded four kept their ids and wore their new versions (the append discipline's
    // identity side: a version bump that renamed anything would orphan every patch).
    for (id, version) in [
        ("sparq/mod/clk", "0.2.0"),
        ("sparq/mod/seq", "0.2.0"),
        ("sparq/ana/rms", "0.2.0"),
        ("sparq/util/delay", "0.3.0"),
        ("sparq/util/vca", "0.2.0"),
    ] {
        let m = reg.get(id).unwrap().manifest();
        assert_eq!(
            m.manifest().identity.version.as_deref(),
            Some(version),
            "{id} wears its round-4 version"
        );
    }
    // D7's manifest move, from the registry's own mouth: the vca's cv input is UNIPOLAR, so
    // the lfo's unipolar word is compatible at the matrix (G2: equal ranges connect) — the
    // canvas verdict's ground truth. (The canvas-side gate rides S8's bridge batch.)
    let vca = reg.get("sparq/util/vca").unwrap().manifest();
    let cv_in = &vca.ports()[1];
    use sparq_module_api::port::{connect_cv, CvRange, Phase};
    assert_eq!(cv_in.cv_range, Some(CvRange::Unipolar), "the vca declares unipolar");
    assert!(
        matches!(
            connect_cv(CvRange::Unipolar, cv_in.cv_range.unwrap(), Phase::Zero),
            sparq_module_api::port::Verdict::Compatible
        ),
        "unipolar into unipolar connects — the matrix itself did not move"
    );
}
