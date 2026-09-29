//! WO-014 increment 5: the analysis-payload publication — ADR-009 decision 8's ring extended from
//! METERS (peak/rms) to the WAVEFORMS a `dsp/scope` displays. This is the mechanism `ana/tap` and
//! `dsp/scope` waited on: the tap writes its waveform into an audio-rate `cv` output on the audio
//! thread, the cross-thread engine publishes it onto a bounded lock-free ring once per block, and
//! the visual consumer reads it off the ring on the OTHER side — never touching the audio thread
//! and never touching the executor. That separation is the whole reason a scope can claim "≥ 60 fps
//! alongside audio with zero audio-thread cost".
//!
//! Proven here: the tap's waveform crosses to the control side and is the signal itself; an absent
//! reader turns into counted refusals, not memory growth (plan §4.3); and the audio thread
//! allocates NOTHING while publishing analysis, exactly as it does for meters.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![allow(clippy::print_stdout)]

// Defect #66's discipline: the counting allocator's global counter is fed by whichever thread is
// armed, so every test that arms a window serialises on one lock.
#[allow(clippy::disallowed_types)]
// harness-side serialisation, not the audio path (defect #66)
use std::sync::{Mutex, MutexGuard};

use sparq_audio::engine::{AnalysisUpdate, SharedEngine, ANALYSIS_WAVE_LEN};
use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::modules::register_builtins;
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

#[allow(clippy::disallowed_types)] // see the use statement (defect #66)
static AUDIT_LOCK: Mutex<()> = Mutex::new(());
fn audit() -> MutexGuard<'static, ()> {
    AUDIT_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

const RATE: u32 = 48_000;
const FRAMES: usize = 64;
const TAP_IN: u32 = 0;
const TAP_WAVE: u32 = 1;

fn registry() -> Registry {
    let mut r = Registry::new();
    register_builtins(&mut r).unwrap();
    r
}

fn build(reg: &Registry, id: &str, params: &[f32]) -> NodeBuild {
    let r = reg.get(id).unwrap_or_else(|| panic!("{id} missing from the registry"));
    NodeBuild {
        module: r.create(),
        manifest: r.manifest().clone(),
        params: ParamSet::new(0, params).unwrap(),
    }
}

/// `sine → tap`, with the sine as the rendered master (the tap is an analysis branch — it has no
/// audio output, so it never feeds the listener, it only publishes). Returns the executor and the
/// two node ids.
fn tap_world() -> (Executor, NodeId, NodeId) {
    let reg = registry();
    let mut g = Graph::new();
    let sine = g.add_node(0);
    let tap = g.add_node(0);
    g.connect(PortRef::new(sine, 0), PortRef::new(tap, TAP_IN), EdgeKind::Plain).unwrap();
    let ex = Executor::build(
        g,
        vec![
            (sine, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (tap, build(&reg, "sparq/ana/tap", &[1.0])),
        ],
        ExecConfig::new(RATE, FRAMES, 2),
    )
    .unwrap();
    (ex, sine, tap)
}

#[test]
fn the_analysis_ring_carries_the_tap_waveform_to_the_control_side() {
    let (ex, sine, tap) = tap_world();
    let master = sine;
    let (control, audio_engine) = SharedEngine::new(ex, master);

    // The audio side renders five blocks on its own thread (the real topology); the control side
    // then drains what was published.
    let audio_thread = std::thread::spawn(move || {
        let mut ae = audio_engine;
        let mut out = vec![0.0f32; FRAMES * 2];
        for _ in 0..5 {
            ae.render_block(&mut out).unwrap();
        }
        ae.shutdown();
    });
    audio_thread.join().unwrap();

    let mut buf: Vec<AnalysisUpdate> = vec![AnalysisUpdate::default(); 64];
    let read = control.read_analysis(&mut buf);
    assert!(read > 0, "the audio thread published analysis; the control side must see it");
    let tap_waves: Vec<&AnalysisUpdate> =
        buf[..read].iter().filter(|u| u.node == tap.0 && u.port == TAP_WAVE).collect();
    assert_eq!(tap_waves.len(), 5, "one waveform per rendered block for the tap's wave port");
    for u in &tap_waves {
        assert_eq!(u.len as usize, FRAMES, "a full block of waveform is published");
        assert!(u.len as usize <= ANALYSIS_WAVE_LEN);
        let wave = u.wave();
        let peak = wave.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(peak > 0.3 && peak <= 0.51, "the published wave is the 0.5-amp sine: peak {peak}");
        assert!(
            wave.iter().any(|&s| s > 0.0) && wave.iter().any(|&s| s < 0.0),
            "it is signed — a real waveform, not an envelope"
        );
    }
    println!(
        "  analysis ring: {} updates drained, {} from the tap's wave port",
        read,
        tap_waves.len()
    );
}

#[test]
fn an_absent_analysis_reader_is_counted_not_queued() {
    // A shallow analysis ring (8 slots) and twenty rendered blocks with nobody reading until the
    // end: the audio thread never waits, the ring keeps its capacity, and every publication it
    // could not hold is counted (plan §4.3 — backpressure is not loss, but it is never silent).
    let (ex, sine, tap) = tap_world();
    let master = sine;
    // cmd 16, meter 64, analysis 8 — only the analysis depth matters here.
    let (control, audio_engine) = SharedEngine::with_all_capacities(ex, master, 16, 64, 8);

    let audio_thread = std::thread::spawn(move || {
        let mut ae = audio_engine;
        let mut out = vec![0.0f32; FRAMES * 2];
        for _ in 0..20 {
            ae.render_block(&mut out).unwrap();
        }
        ae.shutdown();
    });
    audio_thread.join().unwrap();

    let mut buf: Vec<AnalysisUpdate> = vec![AnalysisUpdate::default(); 64];
    let read = control.read_analysis(&mut buf);
    // The tap publishes THREE waveforms per block — its audio-rate `wave` plus its block-rate
    // `peak` and `rms` (a block-rate cv travels as a one-sample waveform, WO-012 increment 2):
    // 60 offered, 8 kept (the OLDEST — a ring drops the newest when full), 52 refused-and-counted.
    assert_eq!(read, 8, "the ring kept exactly its capacity");
    assert!(buf[..read].iter().all(|u| u.node == tap.0), "every kept update is the tap's");
    let stats = control.stats();
    assert_eq!(
        stats.analysis_refusals,
        3 * 20 - 8,
        "every publication the ring could not hold is counted"
    );
    println!(
        "  analysis ring overflow: 60 published, 8 kept, {} refusals counted",
        stats.analysis_refusals
    );
}

#[test]
fn block_rate_cv_outputs_travel_as_one_sample_waveforms() {
    // WO-012 increment 2: the analysis ring carries EVERY cv output — an audio-rate port as its
    // block buffer (the scope payload, unchanged), a BLOCK-rate port as a one-sample waveform,
    // so a live cv WIRE's level reads `peak |wave|` off the same ring in the same language for
    // both rates. The tap publishes `wave` (audio), `peak` and `rms` (block): the two block
    // ports must arrive as len-1 updates carrying the tap's own computed values of the 0.5-amp
    // sine — peak at the crest bound, rms inside its window bound (both hand-computed below).
    const TAP_PEAK: u32 = 2;
    const TAP_RMS: u32 = 3;
    let (ex, sine, tap) = tap_world();
    let master = sine;
    let (control, audio_engine) = SharedEngine::new(ex, master);

    let audio_thread = std::thread::spawn(move || {
        let mut ae = audio_engine;
        let mut out = vec![0.0f32; FRAMES * 2];
        for _ in 0..5 {
            ae.render_block(&mut out).unwrap();
        }
        ae.shutdown();
    });
    audio_thread.join().unwrap();

    let mut buf: Vec<AnalysisUpdate> = vec![AnalysisUpdate::default(); 64];
    let read = control.read_analysis(&mut buf);
    let of_port = |port: u32| {
        buf[..read].iter().filter(|u| u.node == tap.0 && u.port == port).collect::<Vec<_>>()
    };
    let peaks = of_port(TAP_PEAK);
    let rmss = of_port(TAP_RMS);
    assert_eq!(peaks.len(), 5, "one block-rate publication per rendered block (peak)");
    assert_eq!(rmss.len(), 5, "one block-rate publication per rendered block (rms)");
    for u in peaks.iter().chain(rmss.iter()) {
        assert_eq!(u.len, 1, "a block-rate cv travels as a ONE-sample waveform");
        assert_eq!(u.wave().len(), 1);
    }
    for u in &peaks {
        let v = u.wave()[0];
        assert!(
            (0.45..=0.51).contains(&v),
            "the tap's published peak is the sine's own: {v} (expected ≈ 0.5)"
        );
    }
    for u in &rmss {
        let v = u.wave()[0];
        // Hand-computed bound: rms of a 0.5-amp 440 Hz sine over a 64-frame window at 48 kHz.
        // The period (~109.1 frames) never divides the window, so each block's mean(sin²) rides
        // 1/2 − sin(2ωL)/(4ωL) with its start phase: rms ∈ [√(0.25·0.364), √(0.25·0.636)] ≈
        // [0.302, 0.399], around the integer-period limit 0.5/√2 ≈ 0.3536.
        assert!(
            (0.29..=0.41).contains(&v),
            "the tap's published rms is the sine's own: {v} (window bound around 0.3536)"
        );
    }
    // The audio-rate wave port is untouched by the extension: still full-block waveforms.
    assert!(of_port(TAP_WAVE).iter().all(|u| u.len as usize == FRAMES));
    println!("  block-rate cv on the analysis ring: 5 × peak + 5 × rms, len 1, values checked");
}

#[test]
fn the_audio_thread_allocates_nothing_while_publishing_analysis() {
    let _guard = audit();
    let (ex, sine, _tap) = tap_world();
    let master = sine;
    // Construction (rings, Arcs, the executor) is control-side and allocates; the measurement
    // window covers ONLY the render loop, which is the audio-thread claim.
    let (control, mut audio_engine) = SharedEngine::new(ex, master);
    let mut out = vec![0.0f32; FRAMES * 2];
    for _ in 0..50 {
        audio_engine.render_block(&mut out).unwrap(); // warm: any first-call growth happens here
    }
    let base = start_counting();
    for _ in 0..500 {
        audio_engine.render_block(&mut out).unwrap();
    }
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    // Drain so the ring does not stay full across tests sharing the process (not required, tidy).
    let mut buf: Vec<AnalysisUpdate> = vec![AnalysisUpdate::default(); 4096];
    let _ = control.read_analysis(&mut buf);
    assert_eq!(made, 0, "publishing analysis allocated {made} time(s) on the audio thread");
}
