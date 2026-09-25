//! Golden reference for the Phase B demo patch.
//!
//! This is a much stronger reference than the WO-005 one: a single bit-exact hash covers the SVF
//! filter, both envelope generators, the additive oscillator, three noise sources, the bit crusher,
//! the feedback delay and the sample-accurate clock. Any regression in any of them changes the hash.
//!
//! A failure prints the useful numbers, not just "hash differs".

// Test harness: panicking on a broken precondition is correct here. The workspace-wide deny on
// unwrap/expect/panic exists to keep them out of the instrument, not out of tests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::print_stdout)]

use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::patch::{render_demo, DemoConfig};

struct Manifest {
    hash: String,
    rms: f64,
    peak: f32,
    tolerance: f64,
    bpm: f64,
    bars: usize,
    sample_rate: u32,
    channels: usize,
    seed: u64,
    gain_db: f32,
    delay_send: f32,
    crush_bits: u32,
    frames: usize,
}

fn load() -> Manifest {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/golden/phaseb-demo/manifest.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
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
        bpm: get("bpm").parse().expect("bpm"),
        bars: get("bars").parse().expect("bars"),
        sample_rate: get("sample_rate").parse().expect("sample_rate"),
        channels: get("channels").parse().expect("channels"),
        seed: get("seed").parse().expect("seed"),
        gain_db: get("gain_db").parse().expect("gain_db"),
        delay_send: get("delay_send").parse().expect("delay_send"),
        crush_bits: get("crush_bits").parse().expect("crush_bits"),
        frames: get("frames").parse().expect("frames"),
    }
}

fn config(m: &Manifest) -> DemoConfig {
    DemoConfig {
        bpm: m.bpm,
        bars: m.bars,
        sample_rate: m.sample_rate,
        channels: m.channels,
        seed: m.seed,
        gain_db: m.gain_db,
        delay_send: m.delay_send,
        hat_crush_bits: m.crush_bits,
        ..DemoConfig::default()
    }
}

#[test]
fn phaseb_demo_matches_golden_reference() {
    let m = load();
    let samples = render_demo(&config(&m));

    assert_eq!(samples.len(), m.frames * m.channels, "frame count changed");
    let peak = samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    let rms = (samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        / samples.len() as f64)
        .sqrt();
    let hash = hex64(fnv1a64_f32(&samples));
    println!(
        "golden phaseb-demo: hash={hash} rms={rms:.12} peak={peak:.9} arch={}",
        std::env::consts::ARCH
    );

    assert!(
        (rms - m.rms).abs() <= m.tolerance.max(1e-6),
        "rms drift: got {rms:.12}, golden {:.12}",
        m.rms
    );
    assert!((peak - m.peak).abs() <= 1e-5, "peak drift: got {peak:.9}, golden {:.9}", m.peak);
    if std::env::consts::ARCH == "x86_64" {
        assert_eq!(
            hash, m.hash,
            "the Phase B patch render changed bit-exactly. If intentional, regenerate with \
             `sparq demo --bpm 138 --bars 4 --rate 48000 --ch 2 --seed 0xA17E --golden-values` \
             and update reference/golden/phaseb-demo/manifest.txt with a reason. If not, one of: \
             svf filter, envelopes, additive oscillator, noise, bitcrush, delay, or the clock \
             has regressed."
        );
    } else {
        println!(
            "golden phaseb-demo: hash assertion skipped on {} (recorded on x86_64)",
            std::env::consts::ARCH
        );
    }
}

#[test]
fn phaseb_demo_is_reproducible_within_a_run() {
    let m = load();
    let a = render_demo(&config(&m));
    let b = render_demo(&config(&m));
    assert_eq!(fnv1a64_f32(&a), fnv1a64_f32(&b), "ADR-007: two renders must be bit-identical");
}
