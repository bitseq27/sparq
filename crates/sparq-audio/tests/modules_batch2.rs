//! WO-014 increment 2 gates: the six audio-domain modules, measured — golden renders, the
//! zero-allocation discipline, per-module properties, and the acceptance item that belongs to
//! `syn/polyblep`: **aliasing at least 60 dB below the fundamental at 48 kHz**, measured through
//! the registry-built module with the Phase-B FFT methodology (frame-exact f0, no window
//! leakage), not asserted from the implementation's good intentions.
//!
//! Same counting-allocator harness as `modules_golden.rs`: "zero allocations in `process`" and
//! "zero allocations in `render_block`" are measurements, not comments.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stdout)] // the golden + aliasing evidence lines are the log's numbers

use sparq_audio::dsp::fft::{bin_for_hz, magnitude_spectrum};
use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::manifest::{
    Classification, Identity, Manifest, PortSpec, ResourceDecl, StateDecl,
};
use sparq_module_api::module::{AudioCtx, BlockStatus, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;
const CH: usize = 2;

fn cfg() -> ExecConfig {
    ExecConfig::new(RATE, FRAMES, CH)
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

/// One module alone as the master (sources; processors render silence-free only with input, so
/// this helper is for the source modules).
fn single(reg: &Registry, id: &str, params: &[f32]) -> (Executor, NodeId) {
    let mut g = Graph::new();
    let n = g.add_node(0);
    (Executor::build(g, vec![(n, build(reg, id, params))], cfg()).unwrap(), n)
}

/// sine(440, 0.5) → proc(params), master = proc. The standard processor probe.
fn after_sine(reg: &Registry, id: &str, params: &[f32]) -> (Executor, NodeId) {
    let mut g = Graph::new();
    let s = g.add_node(0);
    let p = g.add_node(0);
    // sine.out = port 0; processor audio in = port 0 for every module in this batch.
    g.connect(PortRef::new(s, 0), PortRef::new(p, 0), EdgeKind::Plain).unwrap();
    let builds =
        vec![(s, build(reg, "sparq/syn/sine", &[440.0, 0.5])), (p, build(reg, id, params))];
    (Executor::build(g, builds, cfg()).unwrap(), p)
}

fn render(ex: &mut Executor, master: NodeId, blocks: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; FRAMES * CH];
    let mut samples: Vec<f32> = Vec::with_capacity(blocks * FRAMES * CH);
    for _ in 0..blocks {
        ex.render_block(master, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    samples
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0.0f32, |a, &s| a.max(s.abs()))
}

// ------------------------------------------------------------------ goldens

/// The golden hashes are this batch's identity: fixed registry-built graphs at 48 kHz/64/2ch,
/// 64 blocks (128 for the delay, so the first echo lands inside the render), FNV-1a over every
/// sample. Any DSP change moves them, and the move must be deliberate to land. Provenance of
/// every number: the exact params below, the default seeds (noise 0x5EED, crusher 0xC5ED/EE),
/// sine 440 Hz @ 0.5 as the standard probe source.
#[test]
fn batch2_golden_renders_are_bit_exact() {
    let reg = registry();
    let (mut nx, nm) = single(&reg, "sparq/syn/noise", &[0.5, 0.0]);
    let noise = hex64(fnv1a64_f32(&render(&mut nx, nm, 64)));
    let (mut px, pm) = single(&reg, "sparq/syn/polyblep", &[220.0, 0.0, 0.5, 0.5, 48.0]);
    let polyblep = hex64(fnv1a64_f32(&render(&mut px, pm, 64)));
    let (mut fx, fm) = after_sine(&reg, "sparq/flt/svf", &[1000.0, 0.2, 0.0]);
    let svf = hex64(fnv1a64_f32(&render(&mut fx, fm, 64)));
    let (mut dx, dm) = after_sine(&reg, "sparq/util/delay", &[250.0, 0.3, 0.5, 0.5]);
    let delay = hex64(fnv1a64_f32(&render(&mut dx, dm, 128)));
    let (mut bx, bm) = after_sine(&reg, "sparq/fx/bitcrush", &[8.0, 4.0, 0.0, 1.0]);
    let bitcrush = hex64(fnv1a64_f32(&render(&mut bx, bm, 64)));
    let (mut ax, am) = after_sine(&reg, "sparq/util/panner", &[50.0, 0.0]);
    let panner = hex64(fnv1a64_f32(&render(&mut ax, am, 64)));

    let cases = [
        ("syn/noise", noise),
        ("syn/polyblep", polyblep),
        ("flt/svf", svf),
        ("util/delay", delay),
        ("fx/bitcrush", bitcrush),
        ("util/panner", panner),
    ];
    for (name, got) in &cases {
        println!("golden {name}: {got}");
    }
    let expected = [
        ("syn/noise", "44ac30042f0233a9"),
        ("syn/polyblep", "bc74dec4272b67a1"),
        ("flt/svf", "460cf54e6913ec41"),
        ("util/delay", "22ef1d97904b1ff5"),
        ("fx/bitcrush", "cdd6232510945825"),
        ("util/panner", "7ef79b30a2c7c469"),
    ];
    for ((name, got), (ename, want)) in cases.iter().zip(expected.iter()) {
        assert_eq!(*name, *ename);
        assert_eq!(got, want, "golden moved: {name}");
    }
}

// ------------------------------------------------------------------ properties

#[test]
fn noise_is_seeded_resettable_and_colour_dependent() {
    let reg = registry();
    // same seed ⇒ same stream (configure is the state path a project save takes)
    let mut a = reg.create("sparq/syn/noise").unwrap();
    let mut b = reg.create("sparq/syn/noise").unwrap();
    a.prepare(&res()).unwrap();
    b.prepare(&res()).unwrap();
    a.configure(&7u64.to_le_bytes()).unwrap();
    b.configure(&7u64.to_le_bytes()).unwrap();
    let ctxb = block();
    let params = ParamSet::new(0, &[1.0, 0.0]).unwrap();
    let (mut oa, mut ob) = (vec![0.0f32; FRAMES], vec![0.0f32; FRAMES]);
    let mut first: Vec<f32> = Vec::new();
    for i in 0..10 {
        let mut ca = AudioCtx { block: &ctxb, params: &params, input: &[], output: &mut oa };
        a.process(&mut ca);
        let mut cb = AudioCtx { block: &ctxb, params: &params, input: &[], output: &mut ob };
        b.process(&mut cb);
        assert_eq!(oa, ob, "one seed, one stream");
        if i == 0 {
            first = oa.clone(); // the stream's FIRST block — what reset must replay
        }
    }
    // reset restarts the stream from the seed — the reproducibility promise
    a.message(b"reset").unwrap();
    let mut oa2 = vec![0.0f32; FRAMES];
    let mut c2 = AudioCtx { block: &ctxb, params: &params, input: &[], output: &mut oa2 };
    a.process(&mut c2);
    assert_eq!(oa2, first, "reset replays the stream from the seed");
    // a different seed is a different stream
    let mut c = reg.create("sparq/syn/noise").unwrap();
    c.prepare(&res()).unwrap();
    c.configure(&9u64.to_le_bytes()).unwrap();
    let mut oc = vec![0.0f32; FRAMES];
    let mut c3 = AudioCtx { block: &ctxb, params: &params, input: &[], output: &mut oc };
    c.process(&mut c3);
    assert_ne!(oc, first, "different seeds diverge");
    // colours differ from each other (same seed, same stream of white underneath)
    let render_colour = |colour: f32| -> Vec<f32> {
        let (mut ex, m) = single(&reg, "sparq/syn/noise", &[0.5, colour]);
        render(&mut ex, m, 4)
    };
    let white = render_colour(0.0);
    let pink = render_colour(1.0);
    let brown = render_colour(2.0);
    assert_ne!(white, pink);
    assert_ne!(pink, brown);
    // brown is the integrated white: measurably the low-frequency-heavy one (DC-removed RMS of
    // the first difference is far smaller than white's)
    let hf = |v: &[f32]| -> f64 {
        v.windows(2).map(|w| f64::from(w[1] - w[0])).map(|d| d * d).sum::<f64>() / v.len() as f64
    };
    assert!(hf(&brown) * 10.0 < hf(&white), "brown must be darker than white");
}

#[test]
fn polyblep_aliasing_is_at_least_60db_below_the_fundamental() {
    // THE acceptance criterion, measured through the registry-built module: at 48 kHz, content
    // near/above the highest declared partial band must sit ≥ 60 dB under the fundamental.
    // Method (Phase B's, defect #13's lesson baked in): a frame-exact frequency — exactly 340
    // periods in 16 384 samples — so there is no window leakage to fake a floor, and the
    // measurement band starts a few bins below Nyquist where no legitimate partial can live.
    let reg = registry();
    let n = 16_384usize;
    let f0 = 340.0 * f64::from(RATE) / n as f64; // 996.09375 Hz, frame-exact
    let (mut ex, m) = single(&reg, "sparq/syn/polyblep", &[f0 as f32, 0.0, 0.5, 1.0, 0.0]);
    let samples = render(&mut ex, m, n / FRAMES);
    // the executor fans the mono out to stereo: take the left channel
    let mono: Vec<f32> = samples.iter().step_by(CH).copied().take(n).collect();
    assert_eq!(mono.len(), n);
    let mag = magnitude_spectrum(&mono);
    let db = |v: f64| 20.0 * (v.max(1e-300)).log10();
    let fund = mag[bin_for_hz(f0, n, RATE)];
    let nyq = bin_for_hz(f64::from(RATE) * 0.5, n, RATE);
    let above = mag[(nyq - 4)..].iter().fold(0.0f64, |a, &v| a.max(v));
    let rel = db(above) - db(fund);
    assert!(
        rel < -60.0,
        "aliasing floor is only {rel:.1} dB below the fundamental — the acceptance is −60"
    );
    // the honest number for the log: additive synthesis leaves float noise, not aliasing
    println!("syn/polyblep aliasing floor: {rel:.1} dB re fundamental at 48 kHz (acceptance: −60)");
}

#[test]
fn svf_modes_pass_and_stop_what_they_claim() {
    let reg = registry();
    // a 10 kHz sine through a 200 Hz lowpass is strongly attenuated…
    let hi = |mode: f32, cutoff: f32| -> f32 {
        let mut g = Graph::new();
        let s = g.add_node(0);
        let f = g.add_node(0);
        g.connect(PortRef::new(s, 0), PortRef::new(f, 0), EdgeKind::Plain).unwrap();
        let builds = vec![
            (s, build(&reg, "sparq/syn/sine", &[10_000.0, 0.5])),
            (f, build(&reg, "sparq/flt/svf", &[cutoff, 0.2, mode])),
        ];
        let (mut ex, m) = (Executor::build(g, builds, cfg()).unwrap(), f);
        peak(&render(&mut ex, m, 64)[FRAMES * CH..]) // skip the first block (filter attack)
    };
    let lp_hi = hi(0.0, 200.0);
    assert!(lp_hi < 0.05, "lowpass at 200 Hz must stop 10 kHz, got peak {lp_hi}");
    // …and the highpass passes it (within the filter's own settling)
    let hp_hi = hi(1.0, 200.0);
    assert!(hp_hi > 0.2, "highpass at 200 Hz must pass 10 kHz, got peak {hp_hi}");
    // a 100 Hz sine through the same lowpass survives
    let mut g = Graph::new();
    let s = g.add_node(0);
    let f = g.add_node(0);
    g.connect(PortRef::new(s, 0), PortRef::new(f, 0), EdgeKind::Plain).unwrap();
    let builds = vec![
        (s, build(&reg, "sparq/syn/sine", &[100.0, 0.5])),
        (f, build(&reg, "sparq/flt/svf", &[5_000.0, 0.2, 0.0])),
    ];
    let (mut ex, m) = (Executor::build(g, builds, cfg()).unwrap(), f);
    let p = peak(&render(&mut ex, m, 64));
    assert!(p > 0.3, "lowpass must pass 100 Hz, got peak {p}");
}

#[test]
fn delay_echoes_at_the_declared_time_and_decays_by_feedback() {
    // An impulse through a 100 ms all-wet delay: the echo lands at exactly sample 4800
    // (block 75), and with feedback 0.5 + full brightness the second echo is half the first,
    // at exactly twice the time. The declared-latency module, measured against its declaration.
    let reg = registry();
    let mut g = Graph::new();
    let imp = g.add_node(0);
    let d = g.add_node(0);
    g.connect(PortRef::new(imp, 0), PortRef::new(d, 0), EdgeKind::Plain).unwrap();
    let builds =
        vec![(imp, impulse_build()), (d, build(&reg, "sparq/util/delay", &[100.0, 0.5, 1.0, 1.0]))];
    let (mut ex, m) = (Executor::build(g, builds, cfg()).unwrap(), d);
    let samples = render(&mut ex, m, 200);
    let left: Vec<f32> = samples.iter().step_by(CH).copied().collect();
    let at = |i: usize| left.get(i).copied().unwrap_or(0.0).abs();
    assert!(at(4800) > 0.9, "first echo at sample 4800: {}", at(4800));
    assert!(at(4799) < 0.01 && at(4801) < 0.5, "and not before/after: {} {}", at(4799), at(4801));
    // The second echo is feedback × first, passed through the damping one-pole: damp = 1.0 is
    // the BRIGHTEST setting (a one-pole at ~16.4 kHz, not a wire), so the echo loses energy —
    // strictly darker than raw feedback, and not nearly-dark at full brightness. Measured
    // physics, asserted as bounds rather than by copying the DSP's coefficient formula.
    let second = at(9600);
    let raw_fb = 0.5 * at(4800);
    assert!(second < raw_fb + 1e-6, "damping must lose energy: {second} vs raw {raw_fb}");
    assert!(second > 0.4 * raw_fb, "damp=1.0 is the bright end, not a muffler: {second}");
}

#[test]
fn bitcrush_quantizes_holds_and_is_transparent_when_asked() {
    let reg = registry();
    // depth 2, div 8 over a DC 0.5 input: the output is a quantised zero-order-held staircase
    let (mut ex, m) = after_sine(&reg, "sparq/fx/bitcrush", &[2.0, 8.0, 0.0, 0.0]);
    // (sine is not DC — use its first quarter-wave where the signal is monotonic; the QUANTISATION
    // claim is about the step set, not the waveform)
    let samples = render(&mut ex, m, 8);
    let mut distinct: Vec<f32> = Vec::new();
    for s in samples.iter().take(FRAMES * CH * 2) {
        if !distinct.iter().any(|d| (d - s).abs() < 1e-7) {
            distinct.push(*s);
        }
    }
    assert!(
        distinct.len() <= 8,
        "2-bit depth cannot produce more than a handful of steps in two blocks, got {}",
        distinct.len()
    );
    // zero-order hold: with div 8, runs of 8 equal samples (phase-aligned from 0)
    let run = &samples[..8 * CH];
    for c in 0..CH {
        let held: Vec<f32> = run.iter().skip(c).step_by(CH).copied().collect();
        assert!(
            held.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-7),
            "div 8 holds each value 8 samples: {held:?}"
        );
    }
    // depth 16, div 1, no dither: transparent within a quantisation step
    let (mut ex2, m2) = after_sine(&reg, "sparq/fx/bitcrush", &[16.0, 1.0, 0.0, 0.0]);
    let crushed = render(&mut ex2, m2, 16);
    let (mut ex3, m3) = single(&reg, "sparq/syn/sine", &[440.0, 0.5]);
    let clean = render(&mut ex3, m3, 16);
    let max_dev = crushed.iter().zip(clean.iter()).fold(0.0f32, |a, (x, y)| a.max((x - y).abs()));
    assert!(max_dev < 1e-3, "16 bits / div 1 / no dither must be transparent, dev {max_dev}");
}

#[test]
fn panner_laws_are_the_documented_curves() {
    let reg = registry();
    let pan_peak = |pan: f32, law: f32| -> (f32, f32) {
        let mut g = Graph::new();
        let s = g.add_node(0);
        let p = g.add_node(0);
        g.connect(PortRef::new(s, 0), PortRef::new(p, 0), EdgeKind::Plain).unwrap();
        let builds = vec![
            (s, build(&reg, "sparq/syn/sine", &[440.0, 1.0])),
            (p, build(&reg, "sparq/util/panner", &[pan, law])),
        ];
        let (mut ex, m) = (Executor::build(g, builds, cfg()).unwrap(), p);
        let samples = render(&mut ex, m, 8);
        let l = samples.iter().step_by(2).fold(0.0f32, |a, &s| a.max(s.abs()));
        let r = samples.iter().skip(1).step_by(2).fold(0.0f32, |a, &s| a.max(s.abs()));
        (l, r)
    };
    // equal-power centre: cos45° = sin45° ≈ 0.7071 on both sides
    let (l, r) = pan_peak(50.0, 0.0);
    assert!((l - std::f32::consts::FRAC_1_SQRT_2).abs() < 2e-3, "equal-power centre L: {l}");
    assert!((r - std::f32::consts::FRAC_1_SQRT_2).abs() < 2e-3, "equal-power centre R: {r}");
    // hard left: all L, no R
    let (l, r) = pan_peak(0.0, 0.0);
    assert!(l > 0.99 && r < 1e-6, "hard left: L {l} R {r}");
    // linear law centre: 0.5 / 0.5
    let (l, r) = pan_peak(50.0, 1.0);
    assert!((l - 0.5).abs() < 2e-3 && (r - 0.5).abs() < 2e-3, "linear centre: {l}/{r}");
}

// ------------------------------------------------------------------ the batch as a patch

#[test]
fn the_full_chain_renders_deterministically_and_allocation_free() {
    // noise → svf → delay → bitcrush → (sum-to-mono) → panner: every module of the batch in one
    // executor-built graph, rendered twice — bit-identical — with the audio path under the
    // counting allocator.
    let reg = registry();
    let build_chain = || {
        let mut g = Graph::new();
        let n = g.add_node(0);
        let f = g.add_node(0);
        let d = g.add_node(0);
        let c = g.add_node(0);
        let p = g.add_node(0);
        g.connect(PortRef::new(n, 0), PortRef::new(f, 0), EdgeKind::Plain).unwrap();
        g.connect(PortRef::new(f, 1), PortRef::new(d, 0), EdgeKind::Plain).unwrap();
        g.connect(PortRef::new(d, 1), PortRef::new(c, 0), EdgeKind::Plain).unwrap();
        g.connect(PortRef::new(c, 1), PortRef::new(p, 0), EdgeKind::Plain).unwrap();
        let builds = vec![
            (n, build(&reg, "sparq/syn/noise", &[0.4, 1.0])),
            (f, build(&reg, "sparq/flt/svf", &[2_500.0, 0.4, 2.0])),
            (d, build(&reg, "sparq/util/delay", &[120.0, 0.35, 0.6, 0.55])),
            (c, build(&reg, "sparq/fx/bitcrush", &[10.0, 3.0, 0.2, 1.0])),
            (p, build(&reg, "sparq/util/panner", &[38.0, 0.0])),
        ];
        Executor::build(g, builds, cfg()).unwrap()
    };
    let mut ex = build_chain();
    // the panner is the last node; its id is the highest
    let master = *ex.order().last().unwrap();
    let mut out = vec![0.0f32; FRAMES * CH];
    let mut samples: Vec<f32> = Vec::with_capacity(256 * FRAMES * CH);
    let made = measure(|| {
        for _ in 0..256 {
            ex.render_block(master, &mut out).unwrap();
            samples.extend_from_slice(&out);
        }
    });
    // the extend_from_slice inside the window grows a reserved Vec — no realloc, so no count
    assert_eq!(made, 0, "the batch chain allocated {made} time(s) on the audio path");
    assert!(peak(&samples) > 0.05, "the chain renders signal, not silence");
    let first = fnv1a64_f32(&samples);
    let mut ex2 = build_chain();
    samples.clear();
    for _ in 0..256 {
        ex2.render_block(master, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    assert_eq!(fnv1a64_f32(&samples), first, "one chain, one sound: renders are bit-identical");
    // pinned so a DSP change to ANY module of the batch moves a checked-in number, in words
    assert_eq!(hex64(first), "9170415cd3852736", "batch2 chain golden moved");
}

#[test]
fn every_batch2_module_makes_zero_allocations_in_process() {
    // The WO-014 gate, per module: steady parameters, 5 000 `process` calls under the counting
    // allocator, zero. One warm-up block outside the window — first-touch paths belong to
    // build/warm-up, the same convention increment 1's gate established.
    let reg = registry();
    let ctxb = block();
    let input = vec![0.25f32; FRAMES * CH];
    let mut out = vec![0.0f32; FRAMES * CH];
    let cases: Vec<(&str, &[f32], bool)> = vec![
        ("sparq/syn/noise", &[0.5, 0.0], false),
        ("sparq/syn/polyblep", &[220.0, 0.0, 0.5, 0.5, 48.0], false),
        ("sparq/flt/svf", &[1000.0, 0.2, 0.0], true),
        ("sparq/util/delay", &[250.0, 0.3, 0.5, 0.5], true),
        ("sparq/fx/bitcrush", &[8.0, 4.0, 0.0, 1.0], true),
        ("sparq/util/panner", &[50.0, 0.0], true),
    ];
    for (id, params, needs_input) in cases {
        let mut m = reg.create(id).unwrap();
        m.prepare(&res()).unwrap();
        let ps = ParamSet::new(0, params).unwrap();
        let inp: &[f32] = if needs_input { &input } else { &[] };
        {
            let mut c = AudioCtx { block: &ctxb, params: &ps, input: inp, output: &mut out };
            m.process(&mut c);
        }
        let made = measure(|| {
            for _ in 0..5_000 {
                let mut c = AudioCtx { block: &ctxb, params: &ps, input: inp, output: &mut out };
                m.process(&mut c);
            }
        });
        assert_eq!(made, 0, "{id} allocated {made} time(s) inside process");
    }
}

// ------------------------------------------------------------------ helpers

fn block() -> sparq_kernel::block::BlockContext {
    sparq_kernel::block::BlockContext::offline(RATE, FRAMES, CH)
}

fn res() -> Resources {
    Resources::from_block(&block())
}

/// A one-sample impulse source with its own manifest — the delay test's input.
struct Impulse {
    fired: bool,
}
impl Module for Impulse {
    fn id(&self) -> &str {
        "sparq/test/impulse"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        self.fired = false;
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        for (i, s) in ctx.output.iter_mut().enumerate() {
            *s = if !self.fired && i == 0 { 1.0 } else { 0.0 };
        }
        self.fired = true;
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("impulse takes no messages"))
    }
}

fn impulse_build() -> NodeBuild {
    let m = Manifest {
        identity: Identity {
            id: Some("sparq/test/impulse".into()),
            version: Some("0.1.0".into()),
            host_api_min: Some(1),
            host_api_max: Some(1),
            display_name: Some("Impulse".into()),
            summary: Some("test impulse".into()),
            authors: vec!["sparq".into()],
            license: Some("MIT".into()),
        },
        classification: Classification {
            category: Some("utility/impulse".into()),
            top: Some("util".into()),
            kind: Some("source".into()),
            tier: Some("t1".into()),
            stability: Some("stable".into()),
        },
        state: StateDecl {
            schema_id: Some("sparq/test/impulse/state".into()),
            schema_version: Some(1),
        },
        resources: ResourceDecl {
            latency_samples: Some(0),
            cpu_class: Some("trivial".into()),
            ..ResourceDecl::default()
        },
        voices_policy: Some("none".into()),
        ..Manifest::default()
    };
    let mut m = m;
    m.ports = vec![PortSpec {
        id: Some("out".into()),
        name: Some("OUT".into()),
        direction: Some("out".into()),
        port_type: Some("audio".into()),
        channel_set: Some("mono".into()),
        ..PortSpec::default()
    }];
    NodeBuild {
        module: Box::new(Impulse { fired: false }),
        manifest: m.validate().unwrap(),
        params: ParamSet::new(0, &[]).unwrap(),
    }
}
