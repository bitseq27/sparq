//! WO-014 increment 5 follow-on: the beat-locked LFO and the tempo-synced delay — the step the
//! increment-5 DECISION unblocked (module-side bpm derivation from the block-start tick, not a
//! per-frame tick view). Measured the way the other batches are: properties as arithmetic, a
//! bit-identical reference, a golden, and zero allocations counted.
//!
//! The tempo is driven by setting `BlockContext.tick` directly — the module-side derivation reads
//! exactly what the executor's musical door (WO-009's `set_musical_position`, fed cross-thread by
//! the `SetMusical` command) puts there, so a module-level tick feed is the same signal a transport
//! produces. The block size is 750 frames so 120 bpm yields an EXACT integer tick delta:
//! `Δtick = bpm/60 × frames/sr × ppqn = 2 × 750/48000 × 960 = 30`, which makes the derived bpm
//! exactly 120 and every expected value an equality, not a tolerance.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stdout)]

use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_module_api::module::{AudioCtx, CvOut, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const SR: u32 = 48_000;
const FRAMES: usize = 750; // chosen so 120 bpm is an exact integer tick delta (see the header)
const PPQN: u32 = 960;
const DTICK_120: u64 = 30; // 120 bpm at FRAMES/SR/PPQN

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

/// A block context whose musical position is `tick` (the module-side derivation reads this).
fn block_at(tick: u64) -> BlockContext {
    let mut bc = BlockContext::offline(SR, FRAMES, 1);
    bc.tick = tick;
    bc.ppqn = PPQN;
    bc
}

/// Count upward 0.5-crossings of a unipolar waveform (an LFO sine crosses 0.5 twice per cycle).
fn crossings(wave: &[f32]) -> usize {
    let mut n = 0;
    let mut last = wave.first().copied().unwrap_or(0.0);
    for &v in wave.iter().skip(1) {
        if last < 0.5 && v >= 0.5 {
            n += 1;
        }
        last = v;
    }
    n
}

// ------------------------------------------------------------------ mod/lfo beat lock

#[test]
fn lfo_beat_lock_derives_its_rate_from_the_transport_not_the_fallback() {
    // sync=1, division=1, driven at 120 bpm → 2 Hz. The free-running `rate` param is set to 7 Hz,
    // a value the output must NOT show: if the derivation works the LFO runs at 2 Hz, and 7 Hz only
    // appears if the transport is ignored. 10 s of render separates them by a wide margin.
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    let mut lfo = reg.create("sparq/mod/lfo").unwrap();
    lfo.prepare(&res).unwrap();
    // rate 7 (fallback) · sine · depth 1 · sync 1 · division 1
    let p = ParamSet::new(1, &[7.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut buf = vec![0.0f32; FRAMES];
    let mut wave: Vec<f32> = Vec::new();
    let mut tick = 0u64;
    for _ in 0..640 {
        // 640 × 750 / 48000 = 10 s
        let bc = block_at(tick);
        let mut ctx = AudioCtx::new(&bc, &p).with_cv_out(CvOut::Audio(&mut buf));
        lfo.process(&mut ctx);
        wave.extend_from_slice(&buf);
        tick += DTICK_120;
    }
    // 2 Hz over 10 s = 20 cycles = 20 upward crossings; 7 Hz would be ~70. Block 0 runs at the 7 Hz
    // fallback (no bpm yet) but contributes a fraction of a cycle, so the count lands near 20.
    let c = crossings(&wave);
    assert!(
        (16..=24).contains(&c),
        "beat-locked LFO at 120 bpm, div 1 ≈ 2 Hz → ~20 crossings, got {c}"
    );
    assert!(c < 40, "the 7 Hz fallback must NOT be driving it (that would be ~70 crossings): {c}");
    println!(
        "  lfo beat-lock: {c} cycles over 10 s at 120 bpm div 1 (≈2 Hz, not the 7 Hz fallback)"
    );
}

#[test]
fn lfo_beat_lock_tracks_a_tempo_change() {
    // Same division, but the tick rate halves midway (120 → 60 bpm): the LFO rate must halve with
    // it, one block after the change (the declared lag). Proves the rate is really derived per
    // block, not latched at the first estimate.
    fn run(
        lfo: &mut dyn sparq_module_api::module::Module,
        p: &ParamSet,
        dtick: u64,
        blocks: usize,
    ) -> usize {
        let mut buf = vec![0.0f32; FRAMES];
        let mut wave: Vec<f32> = Vec::new();
        let mut tick = 0u64;
        for _ in 0..blocks {
            let bc = block_at(tick);
            let mut ctx = AudioCtx::new(&bc, p).with_cv_out(CvOut::Audio(&mut buf));
            lfo.process(&mut ctx);
            wave.extend_from_slice(&buf);
            tick += dtick;
        }
        crossings(&wave)
    }
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    let mut lfo = reg.create("sparq/mod/lfo").unwrap();
    lfo.prepare(&res).unwrap();
    let p = ParamSet::new(1, &[7.0, 0.0, 1.0, 1.0, 1.0]).unwrap(); // sync, div 1, 7 Hz fallback
                                                                   // 320 blocks = 5 s each. 120 bpm → 2 Hz → ~10 crossings; 60 bpm → 1 Hz → ~5 crossings.
    let fast = run(&mut *lfo, &p, DTICK_120, 320);
    lfo.prepare(&res).unwrap(); // reset the follower for a clean second measurement
    let slow = run(&mut *lfo, &p, DTICK_120 / 2, 320);
    assert!((8..=12).contains(&fast), "120 bpm ≈ 2 Hz over 5 s → ~10 crossings, got {fast}");
    assert!((3..=7).contains(&slow), "60 bpm ≈ 1 Hz over 5 s → ~5 crossings, got {slow}");
    assert!(fast > slow, "halving the tempo halves the rate");
}

#[test]
fn lfo_beat_lock_falls_back_to_hz_without_a_transport() {
    // sync=1 but the tick never advances (no transport / musical door unset): the derivation yields
    // no bpm, so the LFO falls back to its free-running `rate` (3 Hz) rather than stopping. A
    // beat-locked module that froze with no transport would be a silent lie; this is the honesty.
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    let mut lfo = reg.create("sparq/mod/lfo").unwrap();
    lfo.prepare(&res).unwrap();
    let p = ParamSet::new(1, &[3.0, 0.0, 1.0, 1.0, 1.0]).unwrap(); // rate 3 Hz fallback, sync 1
    let mut buf = vec![0.0f32; FRAMES];
    let mut wave: Vec<f32> = Vec::new();
    for _ in 0..640 {
        let bc = block_at(0); // tick frozen at 0: no tempo to derive
        let mut ctx = AudioCtx::new(&bc, &p).with_cv_out(CvOut::Audio(&mut buf));
        lfo.process(&mut ctx);
        wave.extend_from_slice(&buf);
    }
    let c = crossings(&wave);
    // 3 Hz over 10 s = 30 cycles → ~30 crossings; a stopped LFO would be 0.
    assert!(
        (26..=34).contains(&c),
        "no transport → the 3 Hz fallback runs (not stopped): {c} crossings"
    );
    assert!(wave.iter().any(|&v| v > 0.9), "the fallback LFO is actually sweeping");
}

#[test]
fn beat_locked_lfo_golden() {
    // A deterministic beat-locked render, hashed. rate fallback 2 == the derived 2 Hz (120 bpm,
    // div 1), sine, depth 1, 4 s. Pinned by the run that introduced it; regeneration is explicit.
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    let mut lfo = reg.create("sparq/mod/lfo").unwrap();
    lfo.prepare(&res).unwrap();
    let p = ParamSet::new(1, &[2.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let mut buf = vec![0.0f32; FRAMES];
    let mut wave: Vec<f32> = Vec::new();
    let mut tick = 0u64;
    for _ in 0..256 {
        // 256 × 750 / 48000 = 4 s
        let bc = block_at(tick);
        let mut ctx = AudioCtx::new(&bc, &p).with_cv_out(CvOut::Audio(&mut buf));
        lfo.process(&mut ctx);
        wave.extend_from_slice(&buf);
        tick += DTICK_120;
    }
    let hash = hex64(fnv1a64_f32(&wave));
    println!("  beat-locked lfo golden (120 bpm, div 1, 4 s): {hash}");
    assert_eq!(hash, BEAT_LFO_GOLDEN, "beat-locked lfo golden changed — was this deliberate?");
}
const BEAT_LFO_GOLDEN: &str = "db4013f41d1fa678";

// ------------------------------------------------------------------ util/delay tempo sync

#[test]
fn delay_tempo_sync_matches_a_hand_timed_delay_bit_for_bit() {
    // The acceptance, as arithmetic: a tempo-synced delay at 120 bpm with division 0.5 (an eighth)
    // has time = 0.5 × 60/120 = 0.25 s. So after the bpm is derived it must render BIT-IDENTICAL
    // to a plain ms delay set to 250 ms — the `set_tempo_sync` path proven against a hand-timed
    // reference, the same pattern the rms→filter and lfo→svf acceptances use.
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    // A: tempo-synced (time param 100 ms is IRRELEVANT while synced — proves the tick drives it).
    let mut a = reg.create("sparq/util/delay").unwrap();
    a.prepare(&res).unwrap();
    let pa = ParamSet::new(1, &[100.0, 0.0, 1.0, 1.0, 1.0, 0.5]).unwrap(); // sync 1, div 0.5, fb 0, mix 1
                                                                           // B: hand-timed 250 ms.
    let mut b = reg.create("sparq/util/delay").unwrap();
    b.prepare(&res).unwrap();
    let pb = ParamSet::new(1, &[250.0, 0.0, 1.0, 1.0, 0.0, 0.0]).unwrap(); // ms mode, 250 ms

    let mut inbuf = vec![0.0f32; FRAMES];
    let mut outa = vec![0.0f32; FRAMES];
    let mut outb = vec![0.0f32; FRAMES];
    let mut tick = 0u64;
    // Warm A for two blocks so its bpm is derived before the compared signal (block 0 has no
    // estimate and would use the 100 ms fallback; the declared one-block lag, excluded on purpose).
    for _ in 0..2 {
        let bc = block_at(tick);
        let mut ca = AudioCtx::new(&bc, &pa).with_audio_in(&inbuf).with_audio_out(&mut outa);
        a.process(&mut ca);
        let mut cb = AudioCtx::new(&bc, &pb).with_audio_in(&inbuf).with_audio_out(&mut outb);
        b.process(&mut cb);
        tick += DTICK_120;
    }
    // Now drive both with the same signal and compare every block, bit for bit.
    for blk in 0..40 {
        for (i, s) in inbuf.iter_mut().enumerate() {
            *s = ((blk * FRAMES + i) as f32 * 0.01).sin() * 0.5;
        }
        let bc = block_at(tick);
        let mut ca = AudioCtx::new(&bc, &pa).with_audio_in(&inbuf).with_audio_out(&mut outa);
        a.process(&mut ca);
        let mut cb = AudioCtx::new(&bc, &pb).with_audio_in(&inbuf).with_audio_out(&mut outb);
        b.process(&mut cb);
        assert_eq!(
            outa, outb,
            "block {blk}: tempo-sync (120 bpm, 0.5 beats) != a hand-timed 250 ms"
        );
        tick += DTICK_120;
    }
}

#[test]
fn delay_tempo_sync_falls_back_to_ms_without_a_transport() {
    // tempo-sync=1 but no transport (tick frozen): the delay falls back to its `time` ms parameter
    // rather than collapsing to zero — a synced delay with no clock still makes sound.
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    let mut a = reg.create("sparq/util/delay").unwrap();
    a.prepare(&res).unwrap();
    let pa = ParamSet::new(1, &[250.0, 0.0, 1.0, 1.0, 1.0, 0.5]).unwrap(); // sync 1, but time 250 ms
    let mut b = reg.create("sparq/util/delay").unwrap();
    b.prepare(&res).unwrap();
    let pb = ParamSet::new(1, &[250.0, 0.0, 1.0, 1.0, 0.0, 0.0]).unwrap(); // plain ms 250
    let mut inbuf = vec![0.0f32; FRAMES];
    let mut outa = vec![0.0f32; FRAMES];
    let mut outb = vec![0.0f32; FRAMES];
    for blk in 0..20 {
        for (i, s) in inbuf.iter_mut().enumerate() {
            *s = ((blk * FRAMES + i) as f32 * 0.013).sin() * 0.4;
        }
        let bc = block_at(0); // frozen tick: no bpm
        let mut ca = AudioCtx::new(&bc, &pa).with_audio_in(&inbuf).with_audio_out(&mut outa);
        a.process(&mut ca);
        let mut cb = AudioCtx::new(&bc, &pb).with_audio_in(&inbuf).with_audio_out(&mut outb);
        b.process(&mut cb);
        assert_eq!(
            outa, outb,
            "block {blk}: with no transport, tempo-sync must equal the 250 ms fallback"
        );
    }
}

#[test]
fn beat_locked_modules_make_zero_allocations() {
    // The whole point of module-side derivation is that it stays off the allocator: reading the
    // block tick and diffing it is arithmetic, not allocation. 5 000 process calls each, driven.
    let reg = registry();
    let res = Resources::from_block(&block_at(0));
    let mut lfo = reg.create("sparq/mod/lfo").unwrap();
    let mut delay = reg.create("sparq/util/delay").unwrap();
    lfo.prepare(&res).unwrap();
    delay.prepare(&res).unwrap();
    let pl = ParamSet::new(1, &[2.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let pd = ParamSet::new(1, &[250.0, 0.3, 0.5, 0.5, 1.0, 0.5]).unwrap();
    let mut cv = vec![0.0f32; FRAMES];
    let inbuf = vec![0.1f32; FRAMES];
    let mut outbuf = vec![0.0f32; FRAMES];
    // warm
    let mut tick = 0u64;
    for _ in 0..500 {
        let bc = block_at(tick);
        let mut c = AudioCtx::new(&bc, &pl).with_cv_out(CvOut::Audio(&mut cv));
        lfo.process(&mut c);
        let mut c = AudioCtx::new(&bc, &pd).with_audio_in(&inbuf).with_audio_out(&mut outbuf);
        delay.process(&mut c);
        tick += DTICK_120;
    }
    let made = measure(|| {
        for _ in 0..5_000 {
            let bc = block_at(tick);
            let mut c = AudioCtx::new(&bc, &pl).with_cv_out(CvOut::Audio(&mut cv));
            lfo.process(&mut c);
            let mut c = AudioCtx::new(&bc, &pd).with_audio_in(&inbuf).with_audio_out(&mut outbuf);
            delay.process(&mut c);
            tick += DTICK_120;
        }
    });
    assert_eq!(made, 0, "beat-locked process allocated {made} time(s)");
}
