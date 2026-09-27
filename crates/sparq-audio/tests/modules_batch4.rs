//! WO-014 increment 4 gates: `mod/lfo` and `mod/clk-div` — the two modules WO-009's clocks
//! unblocked, and the first event-PROCESSING module (events in, events out) in the set.
//! Measured the way batches 2 and 3 established: properties as arithmetic, golden renders
//! checked in, zero allocations counted.
//!
//! The headline gates:
//! * the LFO's saw lands on quarter-cycle values at the exact frames its rate implies, and a
//!   sync event resets the phase AT its sample (the membrane's onset discipline, modulation side);
//! * **lfo → svf cutoff renders BIT-IDENTICAL to a hand-driven reference** — the contract-v1
//!   acceptance pattern reused: an audio-rate cv source reduced (`last`) into the filter's
//!   block-rate input, proven exact rather than approximately right;
//! * the divider's count, the multiplier's measured-interval sub-triggers and the probability
//!   gate's seeded reproducibility all assert on exact sample lists;
//! * **host 16ths → clk-div(÷4) → membrane** — the event→event→audio chain, golden-pinned, with
//!   the kicks on exactly the frames the arithmetic says.
//!
//! Same counting-allocator harness as the other gate binaries.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stdout)] // the golden evidence lines are the log's numbers

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::event::{Event, EventBuf};
use sparq_module_api::module::{AudioCtx, CvOut, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

fn cfg(ch: usize) -> ExecConfig {
    ExecConfig::new(RATE, FRAMES, ch)
}

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

fn build(reg: &Registry, id: &str, params: &[f32]) -> NodeBuild {
    let r = reg.get(id).unwrap_or_else(|| panic!("{id} missing from the registry"));
    NodeBuild {
        module: r.create(),
        manifest: r.manifest().clone(),
        params: ParamSet::new(0, params).unwrap(),
    }
}

fn single(reg: &Registry, id: &str, params: &[f32], ch: usize) -> (Executor, NodeId) {
    let mut g = Graph::new();
    let n = g.add_node(0);
    let ex = Executor::build(g, vec![(n, build(reg, id, params))], cfg(ch)).unwrap();
    (ex, n)
}

/// Render `blocks` blocks of a lone cv source, collecting its audio-rate cv buffer per block.
fn render_cv(reg: &Registry, id: &str, params: &[f32], port: u32, blocks: usize) -> Vec<f32> {
    let (mut ex, n) = single(reg, id, params, 1);
    let mut out = vec![0.0f32; FRAMES];
    let mut stream = Vec::with_capacity(blocks * FRAMES);
    for _ in 0..blocks {
        ex.render_block(n, &mut out).unwrap();
        stream.extend_from_slice(ex.node_cv_audio(n, port).unwrap());
    }
    stream
}

// ------------------------------------------------------------------ mod/lfo

#[test]
fn lfo_saw_lands_on_the_exact_frames_its_rate_implies() {
    // Rate = 48000/2^15 Hz, so the increment is 2^-15 — EXACT in binary. Every phase is an
    // exact dyadic rational, the wrap lands on frame 32768 to the bit, and the assertions below
    // are equalities rather than tolerances. (This test's first draft used 2 Hz and its wrap
    // assert was one accumulated-ulp coin-flip from failing — exactness is the point of an
    // arithmetic gate, so the rate is chosen to HAVE it.)
    let reg = registry();
    let rate = 48_000.0 / 32_768.0; // 1.46484375 Hz
    let stream = render_cv(&reg, "sparq/mod/lfo", &[rate as f32, 2.0, 1.0], 1, 600);
    assert!(stream.len() >= 32_769);
    assert_eq!(stream[0], 0.0, "saw starts at zero");
    assert_eq!(stream[8_192], 0.25, "quarter cycle, exactly — dyadic phase, dyadic value");
    assert_eq!(stream[16_384], 0.5, "half cycle, exactly");
    assert!(stream[32_767] > 0.999, "the cycle tops out just before the wrap: {}", stream[32_767]);
    assert_eq!(stream[32_768], 0.0, "and wraps at exactly its period");
    assert!(stream.iter().all(|&v| (0.0..=1.0).contains(&v)), "unipolar, as declared");
    let _ = rate;
}

#[test]
fn lfo_sync_events_reset_the_phase_at_the_exact_sample() {
    let reg = registry();
    let (mut ex, n) = single(&reg, "sparq/mod/lfo", &[2.0, 2.0, 1.0], 1);
    let mut out = vec![0.0f32; FRAMES];
    let mut stream = Vec::new();
    for b in 0..3 {
        if b == 1 {
            // The transport stand-in: a sync trigger at sample 37 of block 1 (absolute 101).
            ex.push_host_event(n, 0, Event::trigger(37, 1.0)).unwrap();
        }
        ex.render_block(n, &mut out).unwrap();
        stream.extend_from_slice(ex.node_cv_audio(n, 1).unwrap());
    }
    // 2 Hz saw: one increment per sample is 2/48000. Frame 100 is mid-ramp (100 increments in),
    // frame 101 is the reset — exact zero — and frame 102 ramps from there.
    let inc = (2.0 / 48_000.0) as f32;
    assert!((stream[100] - 100.0 * inc).abs() < 1e-6, "mid-ramp before the reset: {}", stream[100]);
    assert_eq!(stream[101], 0.0, "the saw restarts ON the event's sample — exact zero");
    assert!((stream[102] - inc).abs() < 1e-9, "and ramps from there");
}

#[test]
fn lfo_square_is_exactly_two_levels_and_state_roundtrips() {
    let reg = registry();
    let stream = render_cv(&reg, "sparq/mod/lfo", &[4.0, 3.0, 0.5], 1, 100); // square, depth .5
    assert!(
        stream.iter().all(|&v| v == 0.0 || v == 0.5),
        "a square is its two levels, exactly — no interpolation dust"
    );
    assert!(stream.contains(&0.5) && stream.contains(&0.0), "both levels occur");

    // The 8-byte phase blob, the syn/sine promise: restore lands phase-continuous.
    let mut m = reg.create("sparq/mod/lfo").unwrap();
    m.prepare(&Resources::from_block(&BlockContext::offline(RATE, FRAMES, 1))).unwrap();
    m.configure(&0.25f64.to_le_bytes()).unwrap();
    let block = BlockContext::offline(RATE, FRAMES, 1);
    let ps = ParamSet::new(1, &[1.0, 2.0, 1.0]).unwrap(); // saw
    let mut buf = vec![9.0f32; FRAMES];
    {
        let mut ctx = AudioCtx::new(&block, &ps).with_cv_out(CvOut::Audio(&mut buf));
        m.process(&mut ctx);
    }
    assert!((buf[0] - 0.25).abs() < 1e-6, "the restored phase is where the blob said: {}", buf[0]);
    assert!(m.configure(&[1, 2, 3]).is_err(), "a wrong-size blob is refused, not guessed at");
    m.message(b"phase-reset").unwrap();
    {
        let mut ctx = AudioCtx::new(&block, &ps).with_cv_out(CvOut::Audio(&mut buf));
        m.process(&mut ctx);
    }
    assert_eq!(buf[0], 0.0, "phase-reset means phase zero");
}

#[test]
fn lfo_modulates_the_svf_cutoff_bit_identically_to_a_hand_driven_reference() {
    // The contract-v1 acceptance pattern, on the LFO: graph A wires lfo.out (audio-rate
    // unipolar) → svf.cutoff-mod (block-rate unipolar — the receiver's default `last` reduction
    // collapses the block). Graph B is the same filter with NO wire, its cutoff parameter set
    // by hand per block to (200 · 2^(2·mod·cv)) as f32 — the module's own formula. Bit-identical
    // blocks mean the rate change, the reduction and the formula all did exactly what the
    // manifests declare.
    let reg = registry();
    let svf_params: &[f32] = &[200.0, 0.2, 2.0, 1.0]; // bp, mod depth 1.0
    let lfo_params: &[f32] = &[1.0, 2.0, 1.0]; // 1 Hz saw, depth 1 — a rising cutoff sweep

    // A: sine → svf ← lfo
    let mut ga = Graph::new();
    let sine_a = ga.add_node(0);
    let svf_a = ga.add_node(0);
    let lfo_a = ga.add_node(0);
    ga.connect(PortRef::new(sine_a, 0), PortRef::new(svf_a, 0), EdgeKind::Plain).unwrap();
    ga.connect(PortRef::new(lfo_a, 1), PortRef::new(svf_a, 2), EdgeKind::Plain).unwrap();
    let mut ex_a = Executor::build(
        ga,
        vec![
            (sine_a, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (svf_a, build(&reg, "sparq/flt/svf", svf_params)),
            (lfo_a, build(&reg, "sparq/mod/lfo", lfo_params)),
        ],
        cfg(2),
    )
    .unwrap();

    // B: sine → svf, cutoff by hand. (This test's first draft ran a shadow lfo in graph B and
    // rendered it per block — which rendered B's whole graph twice per iteration and advanced
    // the reference FILTER two blocks to A's one. The cv the wired filter consumed is readable
    // from graph A's own lfo node; no shadow, no double-advance.)
    let mut gb = Graph::new();
    let sine_b = gb.add_node(0);
    let svf_b = gb.add_node(0);
    gb.connect(PortRef::new(sine_b, 0), PortRef::new(svf_b, 0), EdgeKind::Plain).unwrap(); // the reference filter needs the same source the wired one hears
    let mut ex_b = Executor::build(
        gb,
        vec![
            (sine_b, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (svf_b, build(&reg, "sparq/flt/svf", &[200.0, 0.2, 2.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();

    let mut out_a = vec![0.0f32; FRAMES * 2];
    let mut out_b = vec![0.0f32; FRAMES * 2];
    let mut version = 2u64;
    let mut last_cutoff = 0.0f32;
    for block in 0..200 {
        ex_a.render_block(svf_a, &mut out_a).unwrap();
        // The cv the wired filter just consumed: graph A's own lfo, last sample of the block —
        // exactly what the receiver's declared `last` reduction collapsed.
        let cv = ex_a.node_cv_audio(lfo_a, 1).unwrap()[FRAMES - 1];
        let cutoff =
            ((200.0f64 * 2.0f64.powf(2.0 * 1.0 * f64::from(cv))) as f32).clamp(10.0, 20_000.0);
        last_cutoff = cutoff;
        version += 1;
        ex_b.set_params(svf_b, ParamSet::new(version, &[cutoff, 0.2, 2.0, 0.0]).unwrap()).unwrap();
        ex_b.render_block(svf_b, &mut out_b).unwrap();
        assert_eq!(
            out_a, out_b,
            "block {block}: the wired LFO and the hand-driven reference diverged"
        );
    }
    // 1 Hz saw over 200 blocks ≈ 0.27 cycles: the cutoff rose from 200 and is still rising.
    assert!(last_cutoff > 230.0, "the sweep is real: cutoff ended at {last_cutoff}");
    println!("  lfo→svf sweep: final effective cutoff {last_cutoff:.3} Hz (from 200, mod 1.0)");
}

// ------------------------------------------------------------------ mod/clk-div

/// Feed 0-value-free triggers into the divider's event-in (manifest port 0) every `every`
/// blocks; collect its event-out (manifest port 1) as absolute samples.
fn run_divider(
    reg: &Registry,
    params: &[f32],
    seed: Option<u64>,
    input_every_blocks: u64,
    blocks: u64,
) -> Vec<u64> {
    let mut g = Graph::new();
    let n = g.add_node(0);
    let mut nb = build(reg, "sparq/mod/clk-div", params);
    if let Some(seed) = seed {
        nb.module.configure(&seed.to_le_bytes()).unwrap();
    }
    let mut ex = Executor::build(g, vec![(n, nb)], cfg(1)).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    let mut fired = Vec::new();
    for b in 0..blocks {
        if b % input_every_blocks == 0 {
            ex.push_host_event(n, 0, Event::trigger(0, 1.0)).unwrap();
        }
        ex.render_block(n, &mut out).unwrap();
        for e in ex.node_events(n, 1).unwrap() {
            fired.push(b * FRAMES as u64 + u64::from(e.sample));
        }
    }
    fired
}

#[test]
fn clk_div_divides_from_the_first_input() {
    let reg = registry();
    // Inputs every 4800 samples (75 blocks); divide 4 → outputs on inputs #1, #5, #9, #13:
    // absolute samples 0, 19200, 38400, 57600. The downbeat of the count always passes.
    let fired = run_divider(&reg, &[4.0, 1.0, 1.0], None, 75, 901);
    assert_eq!(fired, vec![0, 19_200, 38_400, 57_600], "÷4 of a 10 Hz stream");
}

#[test]
fn clk_div_multiplies_across_the_measured_interval() {
    let reg = registry();
    // Multiply 2: every passing input emits at its own sample AND halfway to the next one —
    // the interval MEASURED from the previous arrival (the first input has none and passes
    // alone). Inputs every 4800 ⇒ outputs every 2400 from the second input on.
    let fired = run_divider(&reg, &[1.0, 2.0, 1.0], None, 75, 450);
    assert_eq!(
        &fired[..8],
        &[0, 4_800, 7_200, 9_600, 12_000, 14_400, 16_800, 19_200],
        "×2 turns a quarter-note stream into eighths, on exact samples"
    );
}

#[test]
fn the_probability_gate_is_seeded_reproducible_and_honest_at_its_ends() {
    let reg = registry();
    // prob 0 and prob 1 are exact, not statistical.
    assert!(run_divider(&reg, &[1.0, 1.0, 0.0], None, 75, 300).is_empty(), "p=0 passes nothing");
    let all = run_divider(&reg, &[1.0, 1.0, 1.0], None, 75, 300);
    assert_eq!(all.len(), 4, "p=1 passes everything (inputs at blocks 0/75/150/225 — four)");

    // The seeded middle: two dividers with the SAME seed, the same input stream, fire on
    // exactly the same inputs — ADR-007's promise at the event level; a different seed differs.
    let a = run_divider(&reg, &[1.0, 1.0, 0.5], Some(42), 8, 4_000);
    let b = run_divider(&reg, &[1.0, 1.0, 0.5], Some(42), 8, 4_000);
    let c = run_divider(&reg, &[1.0, 1.0, 0.5], Some(43), 8, 4_000);
    assert_eq!(a, b, "one seed, one decision stream");
    assert_ne!(a, c, "a different seed is a different stream");
    // And the rate is in the neighbourhood of the declaration (a gate that fires 3 % of the
    // time at p=0.5 is a broken rng, not a rare streak): 500 inputs in 4000 blocks.
    let ratio = a.len() as f64 / 500.0;
    assert!((0.35..0.65).contains(&ratio), "p=0.5 passed {ratio} of inputs");
}

#[test]
fn sixteenths_through_divide_four_land_kicks_on_exact_frames() {
    // The chain golden: host 16ths (every 6144 samples — a block-aligned grid, chosen for exact
    // arithmetic, not for its tempo) → clk-div ÷4 → membrane → master. Kicks must land on
    // inputs #1, #5, #9, #13 = frames 0, 24576, 49152, 73728 — event wire to event wire to
    // audio, sample-exact end to end.
    let reg = registry();
    let mut g = Graph::new();
    let div = g.add_node(0);
    let mem = g.add_node(0);
    // clk-div.out (manifest port 1, event) → membrane.trig (manifest port 0, event):
    // producer [trigger] ⊆ consumer [trigger, gate] — the matrix's subset cell, for real.
    g.connect(PortRef::new(div, 1), PortRef::new(mem, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (div, build(&reg, "sparq/mod/clk-div", &[4.0, 1.0, 1.0])),
            (mem, build(&reg, "sparq/syn/membrane", &[50.0, 130.0, 260.0, 0.5, 320.0])),
        ],
        cfg(1),
    )
    .unwrap();
    let mut out = vec![0.0f32; FRAMES];
    let mut samples: Vec<f32> = Vec::new();
    let blocks = 1500; // 2 s
    for b in 0..blocks {
        if b % 96 == 0 {
            // 16th-note grid: every 6144 samples.
            ex.push_host_event(div, 0, Event::trigger(0, 1.0)).unwrap();
        }
        ex.render_block(mem, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    let window_peak =
        |from: usize, to: usize| (from..to).map(|f| samples[f].abs()).fold(0.0f32, f32::max);
    for hit in [0usize, 24_576, 49_152, 73_728] {
        assert!(
            window_peak(hit, hit + FRAMES) > 0.3,
            "a kick must land at frame {hit}: block peak {}",
            window_peak(hit, hit + FRAMES)
        );
    }
    assert!(window_peak(42_000, 42_064) < 0.05, "between divided hits: rest, not phantom kicks");
    let hash = hex64(fnv1a64_f32(&samples));
    println!("  16ths→÷4→membrane chain golden (2 s, mono): {hash}");
    assert_eq!(hash, DIV_CHAIN_GOLDEN, "chain golden changed — was this deliberate?");
}
/// Pinned by the run that introduced it; regeneration is explicit + reviewed (ADR-007).
const DIV_CHAIN_GOLDEN: &str = "914d9063ce9d8a0f";

// ------------------------------------------------------------------ allocation discipline

#[test]
fn both_new_modules_make_zero_allocations_in_process() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 1);
    let res = Resources::from_block(&block);
    let mut lfo = reg.create("sparq/mod/lfo").unwrap();
    let mut div = reg.create("sparq/mod/clk-div").unwrap();
    lfo.prepare(&res).unwrap();
    div.prepare(&res).unwrap();

    let p_lfo = ParamSet::new(1, &[2.0, 0.0, 1.0]).unwrap();
    let p_div = ParamSet::new(1, &[4.0, 2.0, 0.5]).unwrap();
    let events = [Event::trigger(0, 1.0), Event::trigger(37, 0.8)];
    let mut cv_buf = vec![0.0f32; FRAMES];
    let mut ev_buf = EventBuf::new();

    for _ in 0..2_000 {
        let mut ctx = AudioCtx::new(&block, &p_lfo)
            .with_events_in(&events)
            .with_cv_out(CvOut::Audio(&mut cv_buf));
        lfo.process(&mut ctx);
        let mut ctx =
            AudioCtx::new(&block, &p_div).with_events_in(&events).with_event_out(&mut ev_buf);
        div.process(&mut ctx);
        ev_buf.clear();
    }
    let made = measure(|| {
        for _ in 0..5_000 {
            let mut ctx = AudioCtx::new(&block, &p_lfo)
                .with_events_in(&events)
                .with_cv_out(CvOut::Audio(&mut cv_buf));
            lfo.process(&mut ctx);
            let mut ctx =
                AudioCtx::new(&block, &p_div).with_events_in(&events).with_event_out(&mut ev_buf);
            div.process(&mut ctx);
            ev_buf.clear();
        }
    });
    assert_eq!(made, 0, "a mod-family module allocated {made} time(s) inside process");
}

#[test]
fn the_divided_drum_chain_renders_allocation_free() {
    // Host pushes + event→event wire + membrane + 1000 blocks: the whole increment-4 path
    // under the counting allocator.
    let reg = registry();
    let mut g = Graph::new();
    let div = g.add_node(0);
    let mem = g.add_node(0);
    g.connect(PortRef::new(div, 1), PortRef::new(mem, 0), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (div, build(&reg, "sparq/mod/clk-div", &[4.0, 1.0, 0.9])),
            (mem, build(&reg, "sparq/syn/membrane", &[50.0, 130.0, 260.0, 0.5, 320.0])),
        ],
        cfg(1),
    )
    .unwrap();
    let mut out = vec![0.0f32; FRAMES];
    for b in 0..100 {
        if b % 96 == 0 {
            ex.push_host_event(div, 0, Event::trigger(0, 1.0)).unwrap();
        }
        ex.render_block(mem, &mut out).unwrap();
    }
    let made = measure(|| {
        for b in 100..1100u64 {
            if b % 96 == 0 {
                ex.push_host_event(div, 0, Event::trigger(0, 1.0)).unwrap();
            }
            ex.render_block(mem, &mut out).unwrap();
        }
    });
    assert_eq!(made, 0, "the divided-drum path allocated {made} time(s) across 1000 blocks");
}
