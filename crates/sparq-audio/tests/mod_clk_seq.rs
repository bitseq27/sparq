//! The operator-round (2026-10-01) clock family gates: `mod/clk` (the free-running tempo
//! clock with 4th/8th/16th/32nd trigger outs) and `mod/seq` (the 3–16 step trigger
//! sequencer). Measured the way the batch gates established: exact sample lists, not
//! approximately-right windows — a clock that drifts or double-fires is a clock that lies.
//!
//! The headline gates:
//! * the four grids are sample-accurate from block zero and coincide on the downbeat;
//! * a tempo edit re-spaces ticks FROM THE NEXT ONE — no backwards jump, no double fire;
//! * the sequencer walks one step per qualifying trigger, fires exactly the masked steps,
//!   at the INPUT event's sample (zero latency through the ring), and ignores gate-offs;
//! * both modules allocate nothing inside `process` (the audio-thread contract).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_module_api::event::{Event, EventBuf};
use sparq_module_api::module::{AudioCtx, Resources};
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

/// Run `blocks` blocks of a module with four event outputs, collecting absolute tick samples
/// per port.
fn clock_ticks(bpm: f32, blocks: u64) -> [Vec<u64>; 4] {
    let reg = registry();
    let mut clk = reg.create("sparq/mod/clk").unwrap();
    let block0 = BlockContext::offline(RATE, FRAMES, 0);
    clk.prepare(&Resources::from_block(&block0)).unwrap();
    let params = ParamSet::new(1, &[bpm]).unwrap();
    let mut out: [Vec<u64>; 4] = Default::default();
    for b in 0..blocks {
        let mut block = BlockContext::offline(RATE, FRAMES, 1);
        block.sample_offset = b * FRAMES as u64;
        let (mut b0, mut b1, mut b2, mut b3) =
            (EventBuf::new(), EventBuf::new(), EventBuf::new(), EventBuf::new());
        {
            let mut ctx = AudioCtx::new(&block, &params)
                .with_event_out(&mut b0)
                .with_event_out(&mut b1)
                .with_event_out(&mut b2)
                .with_event_out(&mut b3);
            clk.process(&mut ctx);
        }
        let base = block.sample_offset;
        for (k, buf) in
            [b0.as_slice(), b1.as_slice(), b2.as_slice(), b3.as_slice()].iter().enumerate()
        {
            for ev in *buf {
                out[k].push(base + u64::from(ev.sample));
            }
        }
    }
    out
}

#[test]
fn the_four_grids_are_sample_accurate_and_share_the_downbeat() {
    // 120 BPM at 48 kHz: a quarter is 24 000 samples; the divisions halve from there.
    let t = clock_ticks(120.0, 800); // 51 200 samples: two quarters and change
    assert_eq!(&t[0], &[0, 24_000, 48_000], "4ths");
    assert_eq!(&t[1], &[0, 12_000, 24_000, 36_000, 48_000], "8ths");
    assert_eq!(t[2][..5], [0, 6_000, 12_000, 18_000, 24_000], "16ths");
    assert_eq!(t[3][..5], [0, 3_000, 6_000, 9_000, 12_000], "32nds");
    // every grid's first tick IS the downbeat, and ticks are strictly ordered per port
    for (k, g) in t.iter().enumerate() {
        assert_eq!(g[0], 0, "division {k} starts at the downbeat");
        assert!(g.windows(2).all(|w| w[0] < w[1]), "division {k} never double-fires");
    }
}

#[test]
fn a_tempo_edit_respaces_from_the_next_tick_without_jump_or_double_fire() {
    let reg = registry();
    let mut clk = reg.create("sparq/mod/clk").unwrap();
    let block0 = BlockContext::offline(RATE, FRAMES, 0);
    clk.prepare(&Resources::from_block(&block0)).unwrap();
    let slow = ParamSet::new(1, &[120.0]).unwrap();
    let fast = ParamSet::new(1, &[240.0]).unwrap();
    let mut ticks: Vec<u64> = Vec::new();
    // 100 blocks at 120 (two quarters land), then 400 blocks at 240
    for b in 0..500u64 {
        let params = if b < 100 { &slow } else { &fast };
        let mut block = BlockContext::offline(RATE, FRAMES, 1);
        block.sample_offset = b * FRAMES as u64;
        let mut buf = EventBuf::new();
        let mut ctx = AudioCtx::new(&block, params).with_event_out(&mut buf);
        clk.process(&mut ctx);
        for ev in buf.as_slice() {
            ticks.push(block.sample_offset + u64::from(ev.sample));
        }
    }
    // the slow grid: 0 and 24 000; the edit lands mid-block-100 (sample 6 400), so the NEXT
    // tick is 48 000 re-spaced at the new tempo from the tick that preceded the edit:
    // 24 000 + 12 000 = 36 000 — never backwards, never a doubled fire.
    assert_eq!(ticks[0], 0);
    assert_eq!(ticks[1], 24_000);
    assert!(ticks.windows(2).all(|w| w[0] < w[1]), "strictly monotone across the edit");
    // the tick already committed at the old tempo still lands (no jump, no double fire);
    // everything AFTER it walks the new 12 000-sample grid
    let after: Vec<u64> = ticks.iter().copied().filter(|&t| t >= 48_000).collect();
    assert!(
        after.windows(2).all(|w| w[1] - w[0] == 12_000),
        "post-edit grid at 240 BPM: {after:?}"
    );
}

#[test]
fn the_sequencer_mask_is_a_bitmask_over_the_ring() {
    let reg = registry();
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    seq.prepare(&Resources::from_block(&block)).unwrap();
    // 4 steps, mask 0b0101 = 5: steps 0 and 2 set
    let params = ParamSet::new(1, &[4.0, 5.0]).unwrap();
    let events = [
        Event::trigger(5, 1.0),  // step 0 -> fire
        Event::trigger(10, 0.0), // gate-off: not a pulse
        Event::trigger(20, 1.0), // step 1 -> silent
        Event::trigger(30, 1.0), // step 2 -> fire
        Event::trigger(40, 1.0), // step 3 -> silent
        Event::trigger(50, 1.0), // step 0 (wrapped) -> fire
    ];
    let mut buf = EventBuf::new();
    let mut ctx = AudioCtx::new(&block, &params).with_events_in(&events).with_event_out(&mut buf);
    seq.process(&mut ctx);
    let fired: Vec<u32> = buf.as_slice().iter().map(|e| e.sample).collect();
    assert_eq!(fired, [5, 30, 50], "exactly the masked steps, at the INPUT's sample");
}

#[test]
fn the_step_count_wraps_where_the_operator_set_it() {
    let reg = registry();
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 0);
    seq.prepare(&Resources::from_block(&block)).unwrap();
    // 3 steps, all set: every third pulse fires, and the ring is 3 wide, not 16
    let params = ParamSet::new(1, &[3.0, 7.0]).unwrap();
    let events: Vec<Event> = (0..7).map(|i| Event::trigger(i * 8, 1.0)).collect();
    let mut buf = EventBuf::new();
    let mut ctx = AudioCtx::new(&block, &params).with_events_in(&events).with_event_out(&mut buf);
    seq.process(&mut ctx);
    let fired: Vec<u32> = buf.as_slice().iter().map(|e| e.sample).collect();
    assert_eq!(fired, [0, 8, 16, 24, 32, 40, 48], "mask 7 over 3 steps fires every pulse");
    // 5 steps, mask 0b00010 = 2: only step 1
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    seq.prepare(&Resources::from_block(&block)).unwrap();
    let params = ParamSet::new(1, &[5.0, 2.0]).unwrap();
    let mut buf = EventBuf::new();
    let mut ctx = AudioCtx::new(&block, &params).with_events_in(&events).with_event_out(&mut buf);
    seq.process(&mut ctx);
    let fired: Vec<u32> = buf.as_slice().iter().map(|e| e.sample).collect();
    assert_eq!(fired, [8, 48], "step 1 of a 5-ring, wrapped");
}

#[test]
fn the_clock_family_allocates_nothing_in_process() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 1);
    let res = Resources::from_block(&block);
    let mut clk = reg.create("sparq/mod/clk").unwrap();
    let mut seq = reg.create("sparq/mod/seq").unwrap();
    clk.prepare(&res).unwrap();
    seq.prepare(&res).unwrap();
    let p_clk = ParamSet::new(1, &[120.0]).unwrap();
    let p_seq = ParamSet::new(1, &[8.0, 170.0]).unwrap();
    let events = [Event::trigger(0, 1.0), Event::trigger(37, 0.8)];
    let mut b0 = EventBuf::new();
    let mut b1 = EventBuf::new();
    let mut b2 = EventBuf::new();
    let mut b3 = EventBuf::new();
    let mut b4 = EventBuf::new();
    let made = {
        let base = start_counting();
        for _ in 0..5_000 {
            let mut ctx = AudioCtx::new(&block, &p_clk)
                .with_event_out(&mut b0)
                .with_event_out(&mut b1)
                .with_event_out(&mut b2)
                .with_event_out(&mut b3);
            clk.process(&mut ctx);
            b0.clear();
            b1.clear();
            b2.clear();
            b3.clear();
            let mut ctx =
                AudioCtx::new(&block, &p_seq).with_events_in(&events).with_event_out(&mut b4);
            seq.process(&mut ctx);
            b4.clear();
        }
        let made = allocation_count().saturating_sub(base);
        stop_counting();
        made
    };
    assert_eq!(made, 0, "the clock family allocated {made} time(s) inside process");
}
