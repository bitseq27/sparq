//! WO-014 increment 3 gates: the three modules contract v1 unblocked — `syn/membrane`,
//! `env/ad`, `util/mixer` — measured the way batch 2 established: golden renders checked in,
//! properties as arithmetic, zero allocations counted, and the acceptance claims that belong to
//! THIS batch proven where they live:
//!
//! * **membrane**: a trigger at sample 37 starts the hit at sample 37 — the WO's "event-driven,
//!   sample-accurate start" stress, asserted on the exact frames, not on a vibe;
//! * **env/ad**: the envelope publishes on its declared audio-rate cv port and free-runs its
//!   declared loop — a module with no audio ports at all, rendering as a first-class citizen;
//! * **mixer**: the identity default is a bit-exact pass-through (the transparency reference
//!   `util/gain` set), and the matrix routes and sums by the hand-computed cell arithmetic —
//!   the explicit merge the connection rules require, in a module the patch can see;
//! * **drum-demo**: host triggers at 120 BPM land on EXACTLY frames 0/24000/48000/72000 through
//!   membrane → mixer — the whole contract-v1 stack (event wire, host door, multi-port audio,
//!   mono→stereo fan-out) under one golden.
//!
//! Same counting-allocator harness as the other gate binaries: "zero allocations" is a
//! measurement, not a comment.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stdout)] // the golden evidence lines are the log's numbers

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::{drum_demo_patch, register_builtins, DrumDemoPatch};
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::event::Event;
use sparq_module_api::module::{AudioCtx, BlockStatus, CvOut, Resources};
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

/// One module alone as the master.
fn single(reg: &Registry, id: &str, params: &[f32], ch: usize) -> (Executor, NodeId) {
    let mut g = Graph::new();
    let n = g.add_node(0);
    let ex = Executor::build(g, vec![(n, build(reg, id, params))], cfg(ch)).unwrap();
    (ex, n)
}

fn render(ex: &mut Executor, master: NodeId, blocks: usize, ch: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; FRAMES * ch];
    let mut all = Vec::with_capacity(blocks * FRAMES * ch);
    for _ in 0..blocks {
        ex.render_block(master, &mut out).unwrap();
        all.extend_from_slice(&out);
    }
    all
}

// ------------------------------------------------------------------ syn/membrane

#[test]
fn membrane_starts_at_the_exact_sample_of_its_trigger() {
    // The WO's stress for this module, as arithmetic: silence before the event's offset — exact
    // zeros, because nothing ran — and a hit inside the same block, because the frame loop
    // consumes the pre-sorted events before it computes each frame.
    let reg = registry();
    let (mut ex, n) = single(&reg, "sparq/syn/membrane", &[50.0, 130.0, 260.0, 0.5, 320.0], 1);
    ex.push_host_event(n, 0, Event::trigger(37, 1.0)).unwrap();
    let out = render(&mut ex, n, 1, 1);
    assert!(
        out[..37].iter().all(|&v| v == 0.0),
        "frames before the trigger must be EXACT silence — a hit that starts early is a lie \
         about the event's timestamp (first non-zero at {:?})",
        out[..37].iter().position(|&v| v != 0.0)
    );
    let tail_peak = out[37..].iter().map(|v| v.abs()).fold(0.0f32, f32::max);
    assert!(tail_peak > 0.1, "the hit lands in the trigger's own block: peak {tail_peak}");
}

#[test]
fn membrane_is_silent_without_events_and_reproducible_after_reset() {
    let reg = registry();
    // No triggers: a one-shot voice at rest renders exact zeros and says Ok — the silence IS the
    // instrument's output between hits, not a starvation status.
    let (mut ex, n) = single(&reg, "sparq/syn/membrane", &[50.0, 130.0, 260.0, 0.5, 320.0], 1);
    let idle = render(&mut ex, n, 4, 1);
    assert!(idle.iter().all(|&v| v == 0.0), "no events, no sound, no dust");

    // reset replays the hit from the constructed state — the reproducibility promise `syn/noise`
    // set, kept at 96 kHz too (the reset ordering bug the module's comment names would show up
    // here as different coefficients, hence the non-48k rate on purpose).
    let params = &[50.0f32, 130.0, 260.0, 0.5, 320.0][..];
    let mut g = Graph::new();
    let node = g.add_node(0);
    let mut ex96 = Executor::build(
        g,
        vec![(node, build(&reg, "sparq/syn/membrane", params))],
        ExecConfig::new(96_000, FRAMES, 1),
    )
    .unwrap();
    ex96.push_host_event(node, 0, Event::trigger(0, 1.0)).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    ex96.render_block(node, &mut out).unwrap();
    let first = out.clone();
    // reset through the module: rebuild the executor (the module lives inside it), so use the
    // message path on a fresh instance instead — the promise is the module's, and the module is
    // what the message reaches.
    let mut m = reg.create("sparq/syn/membrane").unwrap();
    m.prepare(&Resources::from_block(&sparq_kernel::block::BlockContext::offline(
        96_000, FRAMES, 1,
    )))
    .unwrap();
    let ps = ParamSet::new(1, params).unwrap();
    let block = sparq_kernel::block::BlockContext::offline(96_000, FRAMES, 1);
    let events = [Event::trigger(0, 1.0)];
    let mut a = vec![9.0f32; FRAMES];
    {
        let mut ctx = AudioCtx::new(&block, &ps).with_audio_out(&mut a).with_events_in(&events);
        assert_eq!(m.process(&mut ctx), BlockStatus::Ok);
    }
    m.message(b"reset").unwrap();
    let mut b = vec![9.0f32; FRAMES];
    {
        let mut ctx = AudioCtx::new(&block, &ps).with_audio_out(&mut b).with_events_in(&events);
        m.process(&mut ctx);
    }
    assert_eq!(a, b, "reset replays the hit bit-exactly at 96 kHz");
    assert_eq!(a, first, "and the executor path rendered the same block");
}

#[test]
fn membrane_golden_two_hits_one_second() {
    // Triggers at sample 0 and sample 24000 (0.5 s) — the second hit lands mid-block 375 at
    // offset 0; pinned hash, 1 s at 48 kHz/64/mono.
    let reg = registry();
    let (mut ex, n) = single(&reg, "sparq/syn/membrane", &[50.0, 130.0, 260.0, 0.5, 320.0], 1);
    let mut out = vec![0.0f32; FRAMES];
    let mut samples: Vec<f32> = Vec::new();
    for b in 0..750 {
        if b == 0 || b == 375 {
            ex.push_host_event(n, 0, Event::trigger(0, 1.0)).unwrap();
        }
        ex.render_block(n, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    let hash = hex64(fnv1a64_f32(&samples));
    println!("  membrane golden (2 hits, 1 s, mono): {hash}");
    assert_eq!(hash, MEMBRANE_GOLDEN, "membrane golden changed — was this deliberate?");
    // Two identical runs agree (ADR-007, the cheap half).
    let (mut ex2, n2) = single(&reg, "sparq/syn/membrane", &[50.0, 130.0, 260.0, 0.5, 320.0], 1);
    let mut samples2: Vec<f32> = Vec::new();
    for b in 0..750 {
        if b == 0 || b == 375 {
            ex2.push_host_event(n2, 0, Event::trigger(0, 1.0)).unwrap();
        }
        ex2.render_block(n2, &mut out).unwrap();
        samples2.extend_from_slice(&out);
    }
    assert_eq!(hex64(fnv1a64_f32(&samples2)), hash, "the membrane is not deterministic");
}
/// Pinned by the run that introduced it (see the build log); regeneration is explicit + reviewed.
const MEMBRANE_GOLDEN: &str = "1d1c84bd37c99b91";

// ------------------------------------------------------------------ env/ad

#[test]
fn env_ad_starts_at_the_exact_sample_and_publishes_on_its_cv_port() {
    // The purest contract-v1 shape: an event in, an audio-rate cv out, NO audio ports — and the
    // master render still works (it renders silence; the payload lives on the cv port, read
    // through the executor's own reader).
    let reg = registry();
    let (mut ex, n) = single(&reg, "sparq/env/ad", &[1.0, 100.0, 0.0, 0.0], 1); // 1 ms attack
    ex.push_host_event(n, 0, Event::trigger(10, 1.0)).unwrap();
    let out = render(&mut ex, n, 1, 1);
    assert!(out.iter().all(|&v| v == 0.0), "no audio ports: the master render is silence");
    let cv = ex.node_cv_audio(n, 1).expect("the declared audio-rate cv port is readable");
    assert_eq!(cv.len(), FRAMES);
    assert!(cv[..10].iter().all(|&v| v == 0.0), "exact zeros before the trigger's sample");
    assert!(cv[10] > 0.0, "the envelope moves ON the trigger's sample");
    // 1 ms attack at 48 kHz = 48 samples, five-time-constant arrival: by frame 10+48 the exp
    // segment is at its peak. Hand-checkable, not eyeballed.
    assert!(cv[58] > 0.9, "attack reaches ≈peak by its declared time: {}", cv[58]);
    assert!(cv[10..58].windows(2).all(|w| w[1] >= w[0]), "the attack is monotonic");
}

#[test]
fn env_ad_loops_free_running_without_a_single_event() {
    let reg = registry();
    // Cycle length, MEASURED not assumed: the wrapped AdEnv's decay segment runs to epsilon
    // arrival, not to a timer — the declared 50 ms is the five-time-constant arrival point
    // (≈0.7 % level), and the segment ends ≈2.3× later. The observed cycle for 1+50 ms is
    // ≈88 blocks; 320 blocks must therefore carry ≥3 peaks. (This test's first draft computed
    // 51 ms cycles and undercounted — the primitive's semantics won, as they should.)
    let (mut ex, n) = single(&reg, "sparq/env/ad", &[1.0, 50.0, 1.0, 0.0], 1);
    let mut out = vec![0.0f32; FRAMES];
    let mut peaks = 0usize;
    let mut prev_hot = false;
    for _ in 0..320 {
        ex.render_block(n, &mut out).unwrap();
        let cv = ex.node_cv_audio(n, 1).unwrap();
        let p = cv.iter().cloned().fold(0.0f32, f32::max);
        let hot = p > 0.9;
        if hot && !prev_hot {
            peaks += 1;
        }
        prev_hot = hot;
    }
    assert!(peaks >= 3, "loop retriggers itself: {peaks} peaks in 320 blocks");
    assert!(peaks <= 5, "and it is one envelope, not a machine gun: {peaks}");
}

#[test]
fn env_ad_ignores_gate_off_per_its_manifest() {
    let reg = registry();
    // Default decay is 200 ms; a gate-off (value 0) at sample 20 must NOT cut the cycle — the
    // manifest says gate-offs are ignored, and the envelope is still near its peak at frame 30
    // (attack 2 ms ≈ done by frame 96+... at default attack 2 ms = 96 samples the peak is at
    // frame ~106; by frame 30 the exp attack is already past 20%).
    let (mut ex, n) = single(&reg, "sparq/env/ad", &[2.0, 200.0, 0.0, 0.0], 1);
    ex.push_host_event(n, 0, Event::trigger(0, 1.0)).unwrap();
    ex.push_host_event(n, 0, Event::gate(20, 0.0)).unwrap();
    let mut out = vec![0.0f32; FRAMES];
    ex.render_block(n, &mut out).unwrap();
    let cv = ex.node_cv_audio(n, 1).unwrap();
    assert!(cv[30] > 0.2, "the cycle continued through the ignored gate-off: {}", cv[30]);
}

// ------------------------------------------------------------------ util/mixer

/// sine alone, rendered — the reference stream the mixer must pass through bit-exactly.
fn sine_reference(reg: &Registry, blocks: usize) -> Vec<f32> {
    let (mut ex, n) = single(reg, "sparq/syn/sine", &[440.0, 0.5], 2);
    render(&mut ex, n, blocks, 2)
}

#[test]
fn mixer_identity_is_a_bit_exact_passthrough() {
    // The transparency reference `util/gain` set, at matrix scale: untouched defaults = four
    // parallel wires. Bit-exact, because "approximately transparent" is a mixer that colors
    // sound it was never asked to touch.
    let reg = registry();
    let reference = sine_reference(&reg, 50);

    let mut g = Graph::new();
    let sine = g.add_node(0);
    let mixer = g.add_node(0);
    // sine.out (port 0, mono) → mixer.in-0 (port 0, stereo): the documented fan-out.
    g.connect(PortRef::new(sine, 0), PortRef::new(mixer, 0), EdgeKind::Plain).unwrap();
    let identity: Vec<f32> = vec![
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0,
        1.0, 1.0,
    ];
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (mixer, build(&reg, "sparq/util/mixer", &identity)),
        ],
        cfg(2),
    )
    .unwrap();
    let through = render(&mut ex, mixer, 50, 2);
    assert_eq!(through, reference, "the identity matrix must be a wire, not an effect");
}

#[test]
fn mixer_routes_and_sums_by_the_cell_arithmetic() {
    // One source fanned into in-0 AND in-1; cells c00 = 0.5 and c10 = 0.25 route both into
    // out-0, everything else silent. Expected: out0 = (0.5 + 0.25) × s per sample — computed in
    // f64 and written once, so the comparison is against the exact f32 of 0.75 × s (0.75 is a
    // dyadic rational: the f64 round-trip of an f32 product is lossless here).
    let reg = registry();
    let reference = sine_reference(&reg, 20);

    let mut g = Graph::new();
    let sine = g.add_node(0);
    let mixer = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(mixer, 0), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(sine, 0), PortRef::new(mixer, 1), EdgeKind::Plain).unwrap();
    let mut cells = [0.0f32; 20];
    cells[0] = 0.5; // c00: in-0 → out-0
    cells[4] = 0.25; // c10: in-1 → out-0
    cells[16..20].copy_from_slice(&[0.5, 1.0, 1.0, 1.0]); // out-0 trim halves the sum
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (mixer, build(&reg, "sparq/util/mixer", &cells)),
        ],
        cfg(2),
    )
    .unwrap();
    let _ = render(&mut ex, mixer, 20, 2);
    let out0 = ex.node_audio_out(mixer, 4).expect("out-0 is manifest port 4");
    // node_audio_out reads the LAST rendered block; the reference stream's tail is the same sine
    // state (both rendered 20 blocks), so the slices align.
    let tail = &reference[reference.len() - out0.len()..];
    for (i, &v) in out0.iter().enumerate() {
        let s = tail[i];
        let want = ((f64::from(s) * 0.5 + f64::from(s) * 0.25) * 0.5) as f32;
        assert_eq!(v, want, "sample {i}: the matrix arithmetic is the contract");
    }
    // out-1 (manifest port 5) receives nothing: every cell into it is zero.
    let out1 = ex.node_audio_out(mixer, 5).unwrap();
    assert!(out1.iter().all(|&v| v == 0.0), "silent rows stay exactly silent");
}

#[test]
fn mixer_reports_silence_explicitly_when_nothing_is_connected() {
    let reg = registry();
    let identity = [0.0f32; 20]; // values irrelevant; nothing is connected
    let (mut ex, n) = single(&reg, "sparq/util/mixer", &identity, 2);
    let out = render(&mut ex, n, 2, 2);
    assert!(out.iter().all(|&v| v == 0.0));
    let m = ex.meter(n).unwrap();
    assert_eq!(
        m.status,
        BlockStatus::Silenced,
        "an all-unconnected mixer SAYS it silenced — never silence-by-accident"
    );
}

// ------------------------------------------------------------------ the drum demo (stack golden)

#[test]
fn drum_demo_lands_four_on_the_floor_on_exact_frames() {
    let reg = registry();
    let mut demo = drum_demo_patch(&reg, cfg(2)).unwrap();
    let mut out = vec![0.0f32; FRAMES * 2];
    let mut samples: Vec<f32> = Vec::new();
    let blocks = 1500; // 2 s at 48 kHz/64
    for b in 0..blocks {
        if DrumDemoPatch::is_kick_block(b as u64, RATE, FRAMES) {
            demo.executor.push_host_event(demo.membrane, 0, Event::trigger(0, 1.0)).unwrap();
        }
        demo.executor.render_block(demo.mixer, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    let peak_at = |frame: usize| samples[frame * 2].abs();
    let window_peak =
        |from: usize, to: usize| (from..to).map(|f| samples[f * 2].abs()).fold(0.0f32, f32::max);
    // 120 BPM = a hit every 24000 frames, on the block boundary, at sample 0 of that block.
    // The window is the hit's whole first block: the transient builds over its first ~20 frames
    // (the 0.4 ms amplitude attack), so a 10-frame window would measure the attack, not the hit.
    for hit in [0usize, 24000, 48000, 72000] {
        assert!(
            window_peak(hit, hit + FRAMES) > 0.3,
            "a kick must land at frame {hit}: block peak {}",
            window_peak(hit, hit + FRAMES)
        );
        if hit > 0 {
            assert!(
                peak_at(hit - 1) < 0.05,
                "and the frame BEFORE frame {hit} is still the previous hit's tail: {}",
                peak_at(hit - 1)
            );
        }
    }
    // Long after a hit and long before the next: the 260 ms decay is done, the output is exactly
    // at rest — no phantom transients between beats.
    assert!(window_peak(40000, 40000 + FRAMES) < 0.05, "between kicks is rest, not hit");

    let hash = hex64(fnv1a64_f32(&samples));
    println!("  drum-demo golden (2 s, 4 kicks, stereo): {hash}");
    assert_eq!(hash, DRUM_DEMO_GOLDEN, "drum-demo golden changed — was this deliberate?");

    // The whole stack twice: rebuild, re-inject, bit-identical (ADR-007).
    let mut demo2 = drum_demo_patch(&reg, cfg(2)).unwrap();
    let mut samples2: Vec<f32> = Vec::new();
    for b in 0..blocks {
        if DrumDemoPatch::is_kick_block(b as u64, RATE, FRAMES) {
            demo2.executor.push_host_event(demo2.membrane, 0, Event::trigger(0, 1.0)).unwrap();
        }
        demo2.executor.render_block(demo2.mixer, &mut out).unwrap();
        samples2.extend_from_slice(&out);
    }
    assert_eq!(hex64(fnv1a64_f32(&samples2)), hash, "the drum demo is not deterministic");
}
/// Pinned by the run that introduced it: 2 s at 48 kHz/64/stereo, kicks at 0/24000/48000/72000.
const DRUM_DEMO_GOLDEN: &str = "f2303f13aa0cf299";

// ------------------------------------------------------------------ allocation discipline

#[test]
fn all_three_new_modules_make_zero_allocations_in_process() {
    let reg = registry();
    let block = sparq_kernel::block::BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let params_m = ParamSet::new(1, &[50.0, 130.0, 260.0, 0.5, 320.0]).unwrap();
    let params_e = ParamSet::new(1, &[2.0, 200.0, 1.0, 0.0]).unwrap();
    let identity: [f32; 20] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0,
        1.0, 1.0,
    ];
    let params_x = ParamSet::new(1, &identity).unwrap();

    let mut membrane = reg.create("sparq/syn/membrane").unwrap();
    let mut env = reg.create("sparq/env/ad").unwrap();
    let mut mixer = reg.create("sparq/util/mixer").unwrap();
    for m in [&mut membrane, &mut env, &mut mixer] {
        m.prepare(&res).unwrap();
    }

    let events = [Event::trigger(0, 1.0), Event::trigger(37, 0.8)];
    let mut m_out = vec![0.0f32; FRAMES];
    let mut e_cell = vec![0.0f32; FRAMES];
    let (x_in0, x_in1) = (vec![0.1f32; FRAMES * 2], vec![0.2f32; FRAMES * 2]);
    let (mut x_o0, mut x_o1) = (vec![0.0f32; FRAMES * 2], vec![0.0f32; FRAMES * 2]);
    let (mut x_o2, mut x_o3) = (vec![0.0f32; FRAMES * 2], vec![0.0f32; FRAMES * 2]);

    // Warm up un-measured (the harness's one-shot lazy inits must not be counted).
    for _ in 0..2_000 {
        let mut ctx =
            AudioCtx::new(&block, &params_m).with_events_in(&events).with_audio_out(&mut m_out);
        membrane.process(&mut ctx);
        let mut ctx = AudioCtx::new(&block, &params_e)
            .with_events_in(&events)
            .with_cv_out(CvOut::Audio(&mut e_cell));
        env.process(&mut ctx);
        let mut ctx = AudioCtx::new(&block, &params_x)
            .with_audio_in(&x_in0)
            .with_audio_in(&x_in1)
            .with_audio_out(&mut x_o0)
            .with_audio_out(&mut x_o1)
            .with_audio_out(&mut x_o2)
            .with_audio_out(&mut x_o3);
        mixer.process(&mut ctx);
    }

    let made = measure(|| {
        for _ in 0..5_000 {
            let mut ctx =
                AudioCtx::new(&block, &params_m).with_events_in(&events).with_audio_out(&mut m_out);
            assert_eq!(membrane.process(&mut ctx), BlockStatus::Ok);
            let mut ctx = AudioCtx::new(&block, &params_e)
                .with_events_in(&events)
                .with_cv_out(CvOut::Audio(&mut e_cell));
            assert_eq!(env.process(&mut ctx), BlockStatus::Ok);
            let mut ctx = AudioCtx::new(&block, &params_x)
                .with_audio_in(&x_in0)
                .with_audio_in(&x_in1)
                .with_audio_out(&mut x_o0)
                .with_audio_out(&mut x_o1)
                .with_audio_out(&mut x_o2)
                .with_audio_out(&mut x_o3);
            assert_eq!(mixer.process(&mut ctx), BlockStatus::Ok);
        }
    });
    assert_eq!(made, 0, "a batch-3 module allocated {made} time(s) inside process");
}

#[test]
fn the_drum_demo_path_renders_allocation_free() {
    // The event wire under a real patch: host-door pushes (inside the reserved queue), the merge,
    // the membrane's frame loop, the mixer's matrix — 1000 blocks, zero allocations, measured.
    let reg = registry();
    let mut demo = drum_demo_patch(&reg, cfg(2)).unwrap();
    let mut out = vec![0.0f32; FRAMES * 2];
    for b in 0..50 {
        if DrumDemoPatch::is_kick_block(b, RATE, FRAMES) {
            demo.executor.push_host_event(demo.membrane, 0, Event::trigger(0, 1.0)).unwrap();
        }
        demo.executor.render_block(demo.mixer, &mut out).unwrap();
    }
    let made = measure(|| {
        for b in 50..1050u64 {
            if DrumDemoPatch::is_kick_block(b, RATE, FRAMES) {
                demo.executor.push_host_event(demo.membrane, 0, Event::trigger(0, 1.0)).unwrap();
            }
            demo.executor.render_block(demo.mixer, &mut out).unwrap();
        }
    });
    assert_eq!(made, 0, "the drum-demo path allocated {made} time(s) across 1000 blocks");
}
