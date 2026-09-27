//! WO-014 increment 5 gates: `ana/tap`, `dsp/scope`, `out/main` — the last three modules, the
//! ones that waited on the ring publication (ADR-009 d8, shipped WO-008 inc 5) and the canvas
//! master handover. Measured the way batches 2–4 established: properties as arithmetic, golden
//! renders checked in, zero allocations counted.
//!
//! The headline gates:
//! * `out/main` is a **bit-exact wire at unity** (`v × 1.0 == v`), its trim scales, its mute writes
//!   EXACT zeros, and — because the executor meters every node — its meter IS the master meter
//!   (the "metering hook");
//! * `ana/tap` publishes the **mono mix waveform** at audio rate plus the block's **true** peak and
//!   rms (the display `gain` scales the waveform ONLY, never the reported level), and an
//!   unconnected tap is `Silenced` with a flat wave, never silence-by-accident elsewhere;
//! * `dsp/scope`'s `process` is a **deliberate no-op** — its zero-audio-thread-cost acceptance is
//!   architecture, and the proof is that adding a tap+scope to a patch leaves the master render
//!   **bit-identical** (the scope-rig golden EQUALS the out/main golden);
//! * the tap → scope binding travels as a real audio-rate bipolar cv wire (connect_cv's compatible
//!   cell), and the waveform the scope is bound to is exactly the tap's published wave.
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
use sparq_module_api::module::{AudioCtx, BlockStatus, CvOut, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

// ana/tap manifest port indices (in, wave, peak, rms).
const TAP_IN: u32 = 0;
const TAP_WAVE: u32 = 1;
const TAP_PEAK: u32 = 2;
const TAP_RMS: u32 = 3;
// dsp/scope manifest port indices (x, y).
const SCOPE_X: u32 = 0;
// out/main manifest port indices (in, out).
const OUT_IN: u32 = 0;

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

// ------------------------------------------------------------------ out/main

#[test]
fn out_main_is_a_bit_exact_wire_at_unity() {
    // sine → out/main(trim 1.0). The master render must equal the sine rendered alone, to the bit:
    // `v × 1.0 == v` in IEEE-754, and the mono→stereo fan-out is the same conversion the master
    // render performs anyway. This is the transparency reference the mixer's identity default and
    // util/gain's unity both pin, now on the master bus.
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let out = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(out, OUT_IN), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (out, build(&reg, "sparq/out/main", &[1.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();

    // Reference: the same sine rendered straight to a stereo device (no out/main).
    let mut gref = Graph::new();
    let sref = gref.add_node(0);
    let mut exref =
        Executor::build(gref, vec![(sref, build(&reg, "sparq/syn/sine", &[440.0, 0.5]))], cfg(2))
            .unwrap();

    let mut a = vec![0.0f32; FRAMES * 2];
    let mut b = vec![0.0f32; FRAMES * 2];
    for _ in 0..200 {
        ex.render_block(out, &mut a).unwrap();
        exref.render_block(sref, &mut b).unwrap();
        assert_eq!(a, b, "out/main at unity must be a bit-exact wire");
    }
}

#[test]
fn out_main_trim_scales_and_mute_writes_exact_zeros() {
    let reg = registry();
    // Trim 0.5 halves; mute writes EXACT zeros and reports Silenced (a near-zero would be a lie).
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let out = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(out, OUT_IN), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (out, build(&reg, "sparq/out/main", &[0.5, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();
    // Render enough blocks to span a full 440 Hz period so the discrete peak converges on the
    // true 0.5 × 0.5 = 0.25 (one block alone undershoots — no sample lands exactly on the crest).
    let mut half = vec![0.0f32; FRAMES * 2];
    let mut peak_half = 0.0f32;
    for _ in 0..200 {
        ex.render_block(out, &mut half).unwrap();
        peak_half = half.iter().fold(peak_half, |m, &s| m.max(s.abs()));
    }
    assert!(
        (peak_half - 0.25).abs() < 1e-3,
        "trim 0.5 over a 0.5-amp sine peaks at 0.25: {peak_half}"
    );

    // Mute: param 1 = 1.
    ex.set_params(out, ParamSet::new(1, &[0.5, 1.0]).unwrap()).unwrap();
    let mut muted = vec![9.0f32; FRAMES * 2];
    ex.render_block(out, &mut muted).unwrap();
    assert!(muted.iter().all(|&s| s == 0.0), "mute writes EXACT zeros, never a near-zero");
    assert_eq!(ex.meter(out).unwrap().status, BlockStatus::Silenced, "a muted master says so");
    assert_eq!(ex.meter(out).unwrap().peak, 0.0, "and its meter reads zero");
}

#[test]
fn out_main_meter_is_the_master_metering_hook() {
    // The "metering hook": the executor meters every node, so out/main's peak/rms ARE the master
    // meters. A 0.5-amp sine at unity trim peaks at 0.5 and its meter says so.
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let out = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(out, OUT_IN), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (out, build(&reg, "sparq/out/main", &[1.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();
    let mut o = vec![0.0f32; FRAMES * 2];
    let mut peak = 0.0f32;
    for _ in 0..200 {
        ex.render_block(out, &mut o).unwrap();
        peak = peak.max(ex.meter(out).unwrap().peak);
    }
    assert!((peak - 0.5).abs() < 1e-3, "the master meter reads the sine's 0.5 peak: {peak}");
}

#[test]
fn out_main_master_render_golden() {
    // The master-output golden: sine(440, 0.5) → out/main(unity), 2 s stereo. Pinned by the run
    // that introduced it; regeneration is explicit + reviewed (ADR-007).
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let out = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(out, OUT_IN), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (out, build(&reg, "sparq/out/main", &[1.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();
    let mut o = vec![0.0f32; FRAMES * 2];
    let mut samples: Vec<f32> = Vec::new();
    for _ in 0..1500 {
        ex.render_block(out, &mut o).unwrap();
        samples.extend_from_slice(&o);
    }
    let hash = hex64(fnv1a64_f32(&samples));
    println!("  out/main master golden (2 s, stereo): {hash}");
    assert_eq!(hash, OUT_MAIN_GOLDEN, "out/main golden changed — was this deliberate?");
}
const OUT_MAIN_GOLDEN: &str = "75bc7f2f18cac9d5";

// ------------------------------------------------------------------ ana/tap

#[test]
fn tap_wave_is_the_mono_mix_and_peak_rms_report_the_true_signal() {
    // A DC 0.5 stereo input: the mono mix is 0.5 per frame, peak 0.5, rms 0.5. The waveform rides
    // the audio-rate cv port; peak/rms ride block-rate cells (the ana/rms convention).
    let reg = registry();
    let (mut ex, n) = single(&reg, "sparq/ana/tap", &[1.0], 2);
    // Drive a DC input by hand: build a tiny graph with a DC source is overkill — instead feed the
    // tap through a gain whose input we set. Simpler: use a sine at DC is not possible, so assert
    // on a sine's peak/rms relationship instead, and the mono-mix property on a known waveform.
    let _ = (&mut ex, n);

    // Direct module-level assertion on a DC block (no graph needed): the cleanest arithmetic.
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut tap = reg.create("sparq/ana/tap").unwrap();
    tap.prepare(&res).unwrap();
    let input = vec![0.5f32; FRAMES * 2]; // stereo DC 0.5
    let mut wave = vec![f32::NAN; FRAMES];
    let mut peak_cell = f32::NAN;
    let mut rms_cell = f32::NAN;
    let p = ParamSet::new(1, &[1.0]).unwrap();
    {
        let mut ctx = AudioCtx::new(&block, &p)
            .with_audio_in(&input)
            .with_cv_out(CvOut::Audio(&mut wave))
            .with_cv_out(CvOut::Block(&mut peak_cell))
            .with_cv_out(CvOut::Block(&mut rms_cell));
        assert_eq!(tap.process(&mut ctx), BlockStatus::Ok);
    }
    assert!(wave.iter().all(|&w| (w - 0.5).abs() < 1e-6), "mono mix of DC 0.5 is 0.5: {wave:?}");
    assert!((peak_cell - 0.5).abs() < 1e-6, "peak of DC 0.5 is 0.5: {peak_cell}");
    assert!((rms_cell - 0.5).abs() < 1e-6, "rms of DC 0.5 is 0.5: {rms_cell}");
}

#[test]
fn tap_gain_scales_the_wave_only_never_the_reported_level() {
    // gain 2 doubles the waveform (a display zoom) but peak/rms report the TRUE signal — a tap
    // used for modulation must stay honest about level while a tap used for a scope can be zoomed.
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut tap = reg.create("sparq/ana/tap").unwrap();
    tap.prepare(&res).unwrap();
    let input = vec![0.25f32; FRAMES * 2];
    let mut wave = vec![0.0f32; FRAMES];
    let mut peak_cell = 0.0f32;
    let mut rms_cell = 0.0f32;
    let p = ParamSet::new(1, &[2.0]).unwrap(); // gain 2
    {
        let mut ctx = AudioCtx::new(&block, &p)
            .with_audio_in(&input)
            .with_cv_out(CvOut::Audio(&mut wave))
            .with_cv_out(CvOut::Block(&mut peak_cell))
            .with_cv_out(CvOut::Block(&mut rms_cell));
        tap.process(&mut ctx);
    }
    assert!((wave[0] - 0.5).abs() < 1e-6, "wave = mix × gain = 0.25 × 2: {}", wave[0]);
    assert!((peak_cell - 0.25).abs() < 1e-6, "peak reports the TRUE 0.25, not the zoomed wave");
    assert!((rms_cell - 0.25).abs() < 1e-6, "rms reports the TRUE 0.25");
}

#[test]
fn tap_unconnected_is_silenced_with_a_flat_wave() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut tap = reg.create("sparq/ana/tap").unwrap();
    tap.prepare(&res).unwrap();
    let mut wave = vec![9.0f32; FRAMES];
    let mut peak_cell = 9.0f32;
    let mut rms_cell = 9.0f32;
    let p = ParamSet::new(1, &[1.0]).unwrap();
    {
        let mut ctx = AudioCtx::new(&block, &p)
            .with_audio_in(&[]) // unconnected
            .with_cv_out(CvOut::Audio(&mut wave))
            .with_cv_out(CvOut::Block(&mut peak_cell))
            .with_cv_out(CvOut::Block(&mut rms_cell));
        assert_eq!(tap.process(&mut ctx), BlockStatus::Silenced);
    }
    assert!(wave.iter().all(|&w| w == 0.0), "an unconnected tap shows a flat line: {wave:?}");
    assert_eq!(peak_cell, 0.0);
    assert_eq!(rms_cell, 0.0);
}

#[test]
fn tap_wave_golden_through_the_executor() {
    // The tap's waveform golden: sine(440, 0.5, mono) fanned to the tap's stereo input, wave
    // collected over 1 s and hashed. Proves the audio-rate cv path publishes the signal shape.
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let tap = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(tap, TAP_IN), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (tap, build(&reg, "sparq/ana/tap", &[1.0])),
        ],
        cfg(2),
    )
    .unwrap();
    let mut o = vec![0.0f32; FRAMES * 2];
    let mut wave: Vec<f32> = Vec::new();
    for _ in 0..750 {
        ex.render_block(tap, &mut o).unwrap();
        wave.extend_from_slice(ex.node_cv_audio(tap, TAP_WAVE).unwrap());
    }
    // The wave peaks near the sine's 0.5 amplitude (mono mix of a fanned mono signal).
    let peak = wave.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!((peak - 0.5).abs() < 1e-3, "the tapped wave peaks at the sine's 0.5: {peak}");
    let hash = hex64(fnv1a64_f32(&wave));
    println!("  ana/tap wave golden (1 s): {hash}");
    assert_eq!(hash, TAP_WAVE_GOLDEN, "tap wave golden changed — was this deliberate?");
}
/// CROSS-VALIDATION, not coincidence: this EQUALS the `syn/sine` 1 s golden (`3f325d4f99ca2a01`
/// in modules_golden.rs). The tap's mono mix of a mono signal fanned to its stereo input is
/// `(s + s) / 2 × 1.0 == s`, so the waveform it publishes is the sine itself, bit for bit — two
/// mechanisms (a direct render, an analysis tap) producing one bit pattern. The tap colours
/// nothing; it reports exactly what passed through.
const TAP_WAVE_GOLDEN: &str = "3f325d4f99ca2a01";

#[test]
fn tap_peak_and_rms_publish_on_their_declared_block_rate_ports() {
    // The block-rate cv ports travel through the executor: after a block, `node_cv_block` reads
    // the peak and rms the tap published, and they agree with the signal (a 0.5-amp sine peaks at
    // 0.5; its rms over a whole number of cycles is 0.5/√2 ≈ 0.354). This is the analysis-as-
    // control-source path ana/rms proved, now for the tap's two magnitude ports.
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let tap = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(tap, TAP_IN), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (tap, build(&reg, "sparq/ana/tap", &[1.0])),
        ],
        cfg(2),
    )
    .unwrap();
    let mut o = vec![0.0f32; FRAMES * 2];
    let mut peak = 0.0f32;
    let mut rms = 0.0f32;
    for _ in 0..200 {
        ex.render_block(tap, &mut o).unwrap();
        peak = peak.max(ex.node_cv_block(tap, TAP_PEAK).unwrap_or(0.0));
        rms = ex.node_cv_block(tap, TAP_RMS).unwrap_or(0.0);
    }
    assert!((peak - 0.5).abs() < 1e-3, "the tap's peak port reads the sine's 0.5: {peak}");
    // Per-block rms of a 440 Hz sine over 64 frames (not a whole number of cycles) fluctuates
    // around 0.5/√2 ≈ 0.354 with the block's start phase; assert the physical band, not a coin-flip
    // equality. The peak (a max over 200 blocks) converges tightly; the rms is a per-block read.
    assert!((0.30..0.42).contains(&rms), "the tap's rms port reads ~0.5/√2 for the sine: {rms}");
}

// ------------------------------------------------------------------ dsp/scope

#[test]
fn scope_process_is_a_zero_cost_no_op() {
    // The display's audio-thread cost is ZERO by architecture: process is a no-op. It has no
    // outputs, touches no samples, and returns Ok. This is the acceptance box, asserted.
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut scope = reg.create("sparq/dsp/scope").unwrap();
    scope.prepare(&res).unwrap();
    let x = vec![0.3f32; FRAMES];
    let p = ParamSet::new(1, &[20.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
    let mut ctx = AudioCtx::new(&block, &p).with_cv_in(sparq_module_api::module::CvIn::Audio(&x));
    assert_eq!(scope.process(&mut ctx), BlockStatus::Ok);
    assert_eq!(
        scope.message(b"anything"),
        Err(sparq_module_api::module::ModuleError::Message("sparq/dsp/scope takes no messages"))
    );
}

#[test]
fn scope_rig_leaves_the_master_render_bit_identical() {
    // The zero-cost proof at the patch level: sine → out/main (master) renders BIT-IDENTICALLY
    // whether or not a tap+scope hang off the same sine. The display path takes nothing from the
    // signal and adds nothing to the render — so the scope-rig golden EQUALS the out/main golden.
    let reg = registry();

    // A: sine → out/main only.
    let mut ga = Graph::new();
    let sa = ga.add_node(0);
    let oa = ga.add_node(0);
    ga.connect(PortRef::new(sa, 0), PortRef::new(oa, OUT_IN), EdgeKind::Plain).unwrap();
    let mut exa = Executor::build(
        ga,
        vec![
            (sa, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (oa, build(&reg, "sparq/out/main", &[1.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();

    // B: sine → out/main (master) AND sine → tap → scope (the display rig).
    let mut gb = Graph::new();
    let sb = gb.add_node(0);
    let ob = gb.add_node(0);
    let tb = gb.add_node(0);
    let sc = gb.add_node(0);
    gb.connect(PortRef::new(sb, 0), PortRef::new(ob, OUT_IN), EdgeKind::Plain).unwrap();
    gb.connect(PortRef::new(sb, 0), PortRef::new(tb, TAP_IN), EdgeKind::Plain).unwrap();
    // tap.wave (audio-rate bipolar cv) → scope.x (audio-rate bipolar cv): connect_cv's compatible
    // cell, carried for real — the display binding is a wire in the graph, not a UI-side guess.
    gb.connect(PortRef::new(tb, TAP_WAVE), PortRef::new(sc, SCOPE_X), EdgeKind::Plain).unwrap();
    let mut exb = Executor::build(
        gb,
        vec![
            (sb, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (ob, build(&reg, "sparq/out/main", &[1.0, 0.0])),
            (tb, build(&reg, "sparq/ana/tap", &[1.0])),
            (sc, build(&reg, "sparq/dsp/scope", &[20.0, 0.0, 0.0, 1.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();

    let mut a = vec![0.0f32; FRAMES * 2];
    let mut b = vec![0.0f32; FRAMES * 2];
    let mut acc: Vec<f32> = Vec::new();
    for _ in 0..1500 {
        exa.render_block(oa, &mut a).unwrap();
        exb.render_block(ob, &mut b).unwrap();
        assert_eq!(a, b, "a tap+scope on the patch changed the master render — it must not");
        acc.extend_from_slice(&b);
    }
    // And the scope IS bound to a live waveform: the tap it reads publishes the sine's shape.
    let wave = exb.node_cv_audio(tb, TAP_WAVE).unwrap();
    let peak = wave.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
    assert!(peak > 0.4, "the scope's source tap publishes a real waveform (peak {peak})");
    // The accumulated master render hashes to EXACTLY the out/main golden: the display path is
    // bit-transparent end to end, over the whole 2 s, not just per block.
    let hash = hex64(fnv1a64_f32(&acc));
    println!("  scope-rig master golden == out/main golden: {hash}");
    assert_eq!(hash, OUT_MAIN_GOLDEN, "the scope rig must render exactly the out/main golden");
}

// ------------------------------------------------------------------ allocation discipline

#[test]
fn all_three_new_modules_make_zero_allocations_in_process() {
    let reg = registry();
    let block = BlockContext::offline(RATE, FRAMES, 2);
    let res = Resources::from_block(&block);
    let mut tap = reg.create("sparq/ana/tap").unwrap();
    let mut scope = reg.create("sparq/dsp/scope").unwrap();
    let mut out = reg.create("sparq/out/main").unwrap();
    tap.prepare(&res).unwrap();
    scope.prepare(&res).unwrap();
    out.prepare(&res).unwrap();

    let input = vec![0.3f32; FRAMES * 2];
    let mut audio_out = vec![0.0f32; FRAMES * 2];
    let mut wave = vec![0.0f32; FRAMES];
    let mut peak_cell = 0.0f32;
    let mut rms_cell = 0.0f32;
    let p_tap = ParamSet::new(1, &[1.0]).unwrap();
    let p_scope = ParamSet::new(1, &[20.0, 0.0, 0.0, 1.0, 0.0]).unwrap();
    let p_out = ParamSet::new(1, &[1.0, 0.0]).unwrap();

    // Warm up (any first-call growth happens here, outside the measurement).
    for _ in 0..2_000 {
        let mut c = AudioCtx::new(&block, &p_tap)
            .with_audio_in(&input)
            .with_cv_out(CvOut::Audio(&mut wave))
            .with_cv_out(CvOut::Block(&mut peak_cell))
            .with_cv_out(CvOut::Block(&mut rms_cell));
        tap.process(&mut c);
        let mut c =
            AudioCtx::new(&block, &p_out).with_audio_in(&input).with_audio_out(&mut audio_out);
        out.process(&mut c);
        let mut c = AudioCtx::new(&block, &p_scope)
            .with_cv_in(sparq_module_api::module::CvIn::Audio(&wave));
        scope.process(&mut c);
    }
    let made = measure(|| {
        for _ in 0..5_000 {
            let mut c = AudioCtx::new(&block, &p_tap)
                .with_audio_in(&input)
                .with_cv_out(CvOut::Audio(&mut wave))
                .with_cv_out(CvOut::Block(&mut peak_cell))
                .with_cv_out(CvOut::Block(&mut rms_cell));
            tap.process(&mut c);
            let mut c =
                AudioCtx::new(&block, &p_out).with_audio_in(&input).with_audio_out(&mut audio_out);
            out.process(&mut c);
            let mut c = AudioCtx::new(&block, &p_scope)
                .with_cv_in(sparq_module_api::module::CvIn::Audio(&wave));
            scope.process(&mut c);
        }
    });
    assert_eq!(made, 0, "a batch-5 module allocated {made} time(s) inside process");
}

#[test]
fn the_scope_rig_renders_allocation_free() {
    // The whole display path — sine → out/main + sine → tap → scope — under the counting
    // allocator over 1000 blocks: zero allocations on the audio path.
    let reg = registry();
    let mut g = Graph::new();
    let s = g.add_node(0);
    let o = g.add_node(0);
    let t = g.add_node(0);
    let sc = g.add_node(0);
    g.connect(PortRef::new(s, 0), PortRef::new(o, OUT_IN), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(s, 0), PortRef::new(t, TAP_IN), EdgeKind::Plain).unwrap();
    g.connect(PortRef::new(t, TAP_WAVE), PortRef::new(sc, SCOPE_X), EdgeKind::Plain).unwrap();
    let mut ex = Executor::build(
        g,
        vec![
            (s, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (o, build(&reg, "sparq/out/main", &[1.0, 0.0])),
            (t, build(&reg, "sparq/ana/tap", &[1.0])),
            (sc, build(&reg, "sparq/dsp/scope", &[20.0, 0.0, 0.0, 1.0, 0.0])),
        ],
        cfg(2),
    )
    .unwrap();
    let mut out = vec![0.0f32; FRAMES * 2];
    for _ in 0..100 {
        ex.render_block(o, &mut out).unwrap();
    }
    let made = measure(|| {
        for _ in 100..1100 {
            ex.render_block(o, &mut out).unwrap();
        }
    });
    assert_eq!(made, 0, "the scope rig allocated {made} time(s) on the audio path");
}
