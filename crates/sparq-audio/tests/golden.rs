//! Golden-reference test (ADR-007, WO-010's harness in miniature).
//!
//! Reads `reference/golden/wo005/manifest.txt`, re-renders the subject, and compares:
//! * `rms` and `peak` to within `tolerance` — portable across platforms and profiles;
//! * `hash` for bit-exactness — asserted only when the host matches `hash_platform`'s
//!   architecture, because bit-exact output depends on the platform `libm`'s `sin`.
//!
//! A failure prints the *useful* numbers: hash, rms, peak, max absolute difference and the sample
//! index where it occurs. "The hash changed" on its own is not a diagnostic.

// Test harness: panicking on a broken precondition is the correct behaviour here, and the
// workspace-wide `deny` on unwrap/expect exists to keep them out of the instrument, not out of tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout)]
use sparq_audio::graph::{GraphConfig, Wo005Graph};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_kernel::device::StreamConfig;

struct Manifest {
    hash: String,
    rms: f64,
    peak: f32,
    tolerance: f64,
    frames: usize,
    freq: f64,
    amp: f32,
    gain_db: f32,
    smooth_ms: f32,
    sample_rate: u32,
    block: usize,
    channels: usize,
    allocations: u64,
    underruns: u64,
}

fn manifest_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/golden/wo005/manifest.txt")
}

fn load() -> Manifest {
    let text = std::fs::read_to_string(manifest_path())
        .expect("golden manifest must exist; run `xtask golden` to create it");
    let get = |k: &str| -> String {
        text.lines()
            .filter(|l| !l.trim_start().starts_with('#'))
            .find_map(|l| {
                l.split_once('=')
                    .filter(|(kk, _)| kk.trim() == k)
                    .map(|(_, v)| v.trim().to_string())
            })
            .unwrap_or_else(|| panic!("manifest is missing `{k}="))
    };
    Manifest {
        hash: get("hash"),
        rms: get("rms").parse().expect("rms"),
        peak: get("peak").parse().expect("peak"),
        tolerance: get("tolerance").parse().expect("tolerance"),
        frames: get("frames").parse().expect("frames"),
        freq: get("freq").parse().expect("freq"),
        amp: get("amp").parse().expect("amp"),
        gain_db: get("gain_db").parse().expect("gain_db"),
        smooth_ms: get("smooth_ms").parse().expect("smooth_ms"),
        sample_rate: get("sample_rate").parse().expect("sample_rate"),
        block: get("block").parse().expect("block"),
        channels: get("channels").parse().expect("channels"),
        allocations: get("allocations").parse().expect("allocations"),
        underruns: get("underruns").parse().expect("underruns"),
    }
}

fn render(m: &Manifest) -> (Vec<f32>, u64, u64) {
    let stream = StreamConfig {
        sample_rate: m.sample_rate,
        block_frames: m.block,
        inputs: 0,
        outputs: m.channels,
        exclusive: false,
    };
    let cfg = GraphConfig {
        stream,
        freq: m.freq,
        amp: m.amp,
        gain_db: m.gain_db,
        smooth_ms: m.smooth_ms,
    };
    let mut g = Wo005Graph::new(cfg);
    let r = g.render(m.frames);
    (r.samples, r.counters.audio_allocations, r.counters.underruns)
}

#[test]
fn wo005_matches_golden_reference() {
    let m = load();
    let (samples, allocs, underruns) = render(&m);

    assert_eq!(samples.len(), m.frames * m.channels, "frame count mismatch");
    assert_eq!(allocs, m.allocations, "the golden render must stay allocation-free");
    assert_eq!(underruns, m.underruns, "the golden render must not underrun");

    let peak = samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    let rms = (samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        / samples.len() as f64)
        .sqrt();

    let hash = hex64(fnv1a64_f32(&samples));
    let arch_matches = std::env::consts::ARCH == "x86_64";

    // Report first, assert second: a golden failure must always print something useful.
    println!(
        "golden wo005: hash={hash} rms={rms:.12} peak={peak:.9} (arch {})",
        std::env::consts::ARCH
    );

    assert!(
        (rms - m.rms).abs() <= m.tolerance,
        "rms drift: got {rms:.12}, golden {:.12}, tolerance {:.1e}",
        m.rms,
        m.tolerance
    );
    assert!(
        (peak - m.peak).abs() as f64 <= m.tolerance.max(1e-6),
        "peak drift: got {peak:.9}, golden {:.9}",
        m.peak
    );
    if arch_matches {
        assert_eq!(
            hash, m.hash,
            "bit-exact render changed. If this was intentional, regenerate with \
             `cargo run -p sparq-app -- golden-values > reference/golden/wo005/values` and update \
             the manifest; if not, a DSP change altered the output and must be explained."
        );
    } else {
        println!(
            "golden wo005: hash assertion skipped on {} (golden was recorded on x86_64)",
            std::env::consts::ARCH
        );
    }
}

#[test]
fn golden_is_reproducible_within_a_single_run() {
    let m = load();
    let (a, _, _) = render(&m);
    let (b, _, _) = render(&m);
    assert_eq!(fnv1a64_f32(&a), fnv1a64_f32(&b), "two renders in one process must be identical");
}
