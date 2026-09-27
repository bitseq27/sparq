//! WO-014 increment 1 golden + discipline tests: the three first-party modules, measured.
//!
//! This binary installs the kernel's counting allocator as its **global allocator** (the
//! `contract.rs` / `rt_discipline.rs` pattern), so "zero allocations in `process`" and "zero
//! allocations in `render_block`" are measurements, not comments. The golden hashes are the
//! module set's identity: any change to a module's DSP changes them, and the change must be
//! deliberate to land.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::ExecConfig;
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::{
    builtin_sources, create_gain, create_rms, create_sine, demo_patch, register_builtins,
};
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::block::BlockContext;
use sparq_module_api::module::{AudioCtx, BlockStatus, CvOut, Resources};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;
const CH: usize = 2;

fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

fn block() -> BlockContext {
    BlockContext::offline(RATE, FRAMES, CH)
}

fn res() -> Resources {
    Resources::from_block(&block())
}

/// Render `blocks` blocks of the demo patch at 48 kHz/64/stereo. Returns the master samples, the
/// rms tap VALUE (contract v1: read from the node's declared block-rate `cv` port — the v0
/// convention of riding in an audio `output[0]` the manifest never declared is gone), and the
/// master's metered rms (the audio-node meter, which must agree with the tap).
fn render_demo(blocks: usize) -> (Vec<f32>, f32, f32) {
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let cfg = ExecConfig::new(RATE, FRAMES, CH);
    let mut demo = demo_patch(&reg, cfg).unwrap();
    let mut out = vec![0.0f32; FRAMES * CH];
    let mut samples: Vec<f32> = Vec::with_capacity(blocks * FRAMES * CH);
    for _ in 0..blocks {
        demo.executor.render_block(demo.gain, &mut out).unwrap();
        samples.extend_from_slice(&out);
    }
    // rms.level is manifest port 1 (the module's block-rate cv output).
    let tap = demo.executor.node_cv_block(demo.rms, 1).unwrap_or(f32::NAN);
    let master_rms = demo.executor.meter(demo.gain).map(|m| m.rms).unwrap_or(f32::NAN);
    (samples, tap, master_rms)
}

// ------------------------------------------------------------------ registry + discovery

#[test]
fn the_builtin_registry_is_complete_and_consistent() {
    let mut reg = Registry::new();
    assert_eq!(register_builtins(&mut reg).unwrap(), sparq_audio::modules::BUILTINS.len());
    for (id, _, _) in sparq_audio::modules::BUILTINS {
        let r = reg.get(id).unwrap_or_else(|| panic!("{id} missing from the registry"));
        let m = r.create();
        assert_eq!(m.id(), id);
        assert_eq!(r.manifest().id(), id);
    }
    // Discovery over the built-in sources: every built-in loads, zero fail — the compiled-in
    // manifests are the same bytes `sparq modules` reads from disk.
    let report = sparq_module_api::discover(builtin_sources());
    assert_eq!(report.loaded.len(), sparq_audio::modules::BUILTINS.len());
    assert!(
        report.failed.is_empty(),
        "a built-in manifest failed its own discovery: {:?}",
        report.failed.iter().map(|f| f.label.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn manifests_on_disk_equal_the_compiled_in_bytes() {
    // The single-source claim, tested: include_str! and the file must be the same file. (If this
    // ever fails, someone edited modules/**/sparqmod.toml without rebuilding — cargo would do
    // it, but the message here names the actual drift.)
    // Generic over BUILTINS on purpose: a hardcoded list is the drift this test exists to catch,
    // one meta-level up. The path convention IS the contract: `sparq/<top>/<name>` lives at
    // `modules/<top>/<name>/sparqmod.toml`.
    for (id, text, _) in sparq_audio::modules::BUILTINS {
        let path = format!("modules/{}/sparqmod.toml", id.trim_start_matches("sparq/"));
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let on_disk =
            std::fs::read_to_string(root.join(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
        assert_eq!(on_disk, text, "{path} drifted from the compiled-in manifest");
    }
}

// ------------------------------------------------------------------ allocation discipline

#[test]
fn all_three_modules_make_zero_allocations_in_process() {
    let ctx = block();
    let input = vec![0.25f32; FRAMES * CH];
    let params = ParamSet::new(1, &[440.0, 0.5]).unwrap();
    let mut out = vec![0.0f32; FRAMES * CH];

    let mut sine = create_sine();
    sine.prepare(&res()).unwrap();
    let mut gain = create_gain();
    gain.prepare(&res()).unwrap();
    let mut rms = create_rms();
    rms.prepare(&res()).unwrap();
    // Contract v1: rms publishes on its cv port — the measured shape includes presenting it.
    let mut cell = 0.0f32;

    let made = measure(|| {
        for _ in 0..5_000 {
            let mut a = AudioCtx::single(&ctx, &params, &[], &mut out);
            assert_eq!(sine.process(&mut a), BlockStatus::Ok);
            let mut b = AudioCtx::single(&ctx, &params, &input, &mut out);
            assert_eq!(gain.process(&mut b), BlockStatus::Ok);
            let mut c = AudioCtx::single(&ctx, &params, &input, &mut out)
                .with_cv_out(CvOut::Block(&mut cell));
            assert_eq!(rms.process(&mut c), BlockStatus::Ok);
        }
    });
    assert_eq!(made, 0, "a first-party module allocated {made} time(s) inside process");
}

#[test]
fn the_demo_patch_renders_allocation_free() {
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let cfg = ExecConfig::new(RATE, FRAMES, CH);
    let mut demo = demo_patch(&reg, cfg).unwrap();
    let mut out = vec![0.0f32; FRAMES * CH];
    let made = measure(|| {
        for _ in 0..1_000 {
            demo.executor.render_block(demo.gain, &mut out).unwrap();
        }
    });
    assert_eq!(made, 0, "the executor+modules path allocated {made} time(s) across 1000 blocks");
}

// ------------------------------------------------------------------ goldens

/// The demo patch (sine 440 @0.5 → gain 0.5, rms tap), 2100 blocks = 2.8 s at 48 kHz/64/stereo.
/// These constants ARE the checked-in golden render (WO-014 acceptance); regenerate deliberately.
const DEMO_BLOCKS: usize = 2100;
const DEMO_GOLDEN_HASH: &str = "53de3b1f3f40e3c9";
/// 0.25 / √2 — the RMS of a 0.25-amplitude sine over a FULL cycle. A 64-frame block at 48 kHz is
/// 0.59 cycles of 440 Hz, so the per-block value legitimately ripples a few percent around this;
/// the tolerance is the window ripple, not sloppiness.
const DEMO_GOLDEN_RMS: f32 = 0.176_78;
const DEMO_RMS_TOLERANCE: f32 = 0.02;

#[test]
fn demo_patch_golden_render() {
    let (samples, tap, master_rms) = render_demo(DEMO_BLOCKS);
    assert_eq!(samples.len(), DEMO_BLOCKS * FRAMES * CH);
    let hash = hex64(fnv1a64_f32(&samples));
    assert_eq!(hash, DEMO_GOLDEN_HASH, "demo golden hash changed — was this deliberate?");
    // The analysis-as-control-source principle, measured two ways: the tap VALUE the module
    // computed, and the master's metered rms — both are the level of a 0.25-amplitude sine.
    assert!(
        (tap - DEMO_GOLDEN_RMS).abs() < DEMO_RMS_TOLERANCE,
        "rms tap value {tap}, expected ≈ {DEMO_GOLDEN_RMS} ± window ripple"
    );
    assert!(
        (master_rms - DEMO_GOLDEN_RMS).abs() < DEMO_RMS_TOLERANCE,
        "master meter rms {master_rms}, expected ≈ {DEMO_GOLDEN_RMS} ± window ripple"
    );
    // Sanity beyond the hash: the master is a sine at half amplitude, both channels identical.
    let peak = samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    assert!((peak - 0.25).abs() < 1e-3, "peak {peak}");
    for f in (0..samples.len()).step_by(CH) {
        assert_eq!(samples[f], samples[f + 1], "stereo frames must be identical (mono fan-out)");
    }
}

#[test]
fn demo_patch_is_deterministic_across_builds() {
    let a = render_demo(500);
    let b = render_demo(500);
    assert_eq!(a.0, b.0, "two identical renders diverged — determinism (ADR-007) is broken");
    assert_eq!(a.1, b.1, "the analysis tap diverged too");
}

// ------------------------------------------------------------------ per-module goldens

#[test]
fn sine_golden_one_second() {
    let mut sine = create_sine();
    sine.prepare(&res()).unwrap();
    let ctx = block();
    let params = ParamSet::new(1, &[440.0, 0.5]).unwrap();
    let mut out = vec![0.0f32; FRAMES]; // mono use
    let mut samples: Vec<f32> = Vec::new();
    for _ in 0..750 {
        let mut a = AudioCtx::single(&ctx, &params, &[], &mut out);
        sine.process(&mut a);
        samples.extend_from_slice(&out);
    }
    let hash = hex64(fnv1a64_f32(&samples));
    assert_eq!(hash, "3f325d4f99ca2a01", "sine golden changed");
}

#[test]
fn gain_golden_is_bit_exact_at_unity() {
    let mut gain = create_gain();
    gain.prepare(&res()).unwrap();
    let ctx = block();
    let params = ParamSet::new(1, &[1.0]).unwrap();
    let input: Vec<f32> = (0..FRAMES * CH).map(|i| (i as f32) * 0.001 - 0.064).collect();
    let mut out = vec![0.0f32; FRAMES * CH];
    let mut a = AudioCtx::single(&ctx, &params, &input, &mut out);
    gain.process(&mut a);
    assert_eq!(out, input, "unity gain must be bit-exact — this IS the golden");
}

#[test]
fn rms_golden_of_a_known_signal() {
    let mut rms = create_rms();
    rms.prepare(&res()).unwrap();
    let ctx = block();
    let params = ParamSet::new(1, &[0.0]).unwrap();
    let input: Vec<f32> = (0..FRAMES * CH).map(|i| if i % 2 == 0 { 0.5 } else { -0.5 }).collect();
    // The declared cv port carries the value (contract v1); NAN so an unwritten cell fails.
    let mut cell = f32::NAN;
    let mut a =
        AudioCtx::single(&ctx, &params, &input, &mut []).with_cv_out(CvOut::Block(&mut cell));
    rms.process(&mut a);
    assert!((cell - 0.5).abs() < 1e-6, "rms of ±0.5 square is 0.5, got {cell}");
}
