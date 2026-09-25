//! `sparq render` — the offline, deterministic path.

use std::process::ExitCode;
use std::time::Instant;

use sparq_audio::graph::{GraphConfig, Wo005Graph};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::wav::{write_wav, SampleFormat};
use sparq_kernel::device::StreamConfig;

use crate::cli::RenderOpts;

pub fn run(o: RenderOpts) -> Result<ExitCode, String> {
    let format = match o.format.as_str() {
        "f32" | "float" | "float32" => SampleFormat::Float32,
        "pcm16" | "16" => SampleFormat::Pcm16,
        "pcm24" | "24" => SampleFormat::Pcm24,
        "pcm32" | "32" => SampleFormat::Pcm32,
        other => return Err(format!("unknown --format `{other}` (f32|pcm16|pcm24|pcm32)")),
    };

    let stream = StreamConfig {
        sample_rate: o.sample_rate,
        block_frames: o.block,
        inputs: 0,
        outputs: o.channels,
        exclusive: false,
    };
    let cfg = GraphConfig { stream, freq: o.freq, amp: o.amp, gain_db: o.gain_db, smooth_ms: 5.0 };

    let total_frames = (o.seconds * f64::from(o.sample_rate)).round() as usize;
    let mut graph = Wo005Graph::new(cfg);

    let started = Instant::now();
    let report = graph.render(total_frames);
    let wall = started.elapsed();

    let peak = report.samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    let rms = (report.samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        / report.samples.len().max(1) as f64)
        .sqrt();

    write_wav(&o.out, o.sample_rate, o.channels as u16, format, &report.samples)
        .map_err(|e| format!("writing {}: {e}", o.out))?;

    // Diagnostics readout. These are the numbers the top bar will show (WO-012) and the numbers
    // the Phase 0 gate table measures (PHASE0-WORKORDERS.md §4).
    println!("sparq render · WO-005");
    println!("  out         {}", o.out);
    println!(
        "  stream      {} Hz · {} ch · block {} sm · {:?}",
        o.sample_rate, o.channels, o.block, format
    );
    println!(
        "  signal      syn/sine {:.3} Hz @ {:.3} → util/gain {:+.1} dB → out/main",
        o.freq, o.amp, o.gain_db
    );
    println!(
        "  length      {:.3} s · {} frames · {} blocks",
        o.seconds, total_frames, report.counters.blocks
    );
    println!("  level       peak {:.1} dB · rms {:.1} dB", db(peak), db(rms as f32));
    println!("  counters    {}", report.counters.summary());
    println!(
        "  timing      wall {:.1} ms ({:.1}× realtime){}",
        wall.as_secs_f64() * 1e3,
        (o.seconds * 1e3) / wall.as_secs_f64().max(1e-9),
        report
            .peak_block
            .map_or_else(String::new, |p| format!(" · peak block {:.1} µs", p.as_secs_f64() * 1e6))
    );
    println!("  hash        {}", hex64(fnv1a64_f32(&report.samples)));

    if report.counters.audio_allocations != 0 {
        return Err(format!(
            "audio path allocated {} time(s) — plan §5.2 rule 1 violated",
            report.counters.audio_allocations
        ));
    }
    if report.counters.underruns != 0 {
        return Err(format!(
            "{} underrun(s) during an offline render — should be impossible",
            report.counters.underruns
        ));
    }
    Ok(ExitCode::SUCCESS)
}

fn db(v: f32) -> f32 {
    if v <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * v.log10()
    }
}

/// Emit the exact `key=value` lines the golden manifest expects, for a given config.
/// Used by `xtask golden` to regenerate `reference/golden/wo005/manifest.txt`.
pub fn golden_values(seconds: f64) -> String {
    use sparq_audio::graph::{GraphConfig, Wo005Graph};
    use sparq_audio::hash::{fnv1a64_f32, hex64};
    let stream = StreamConfig {
        sample_rate: 48_000,
        block_frames: 64,
        inputs: 0,
        outputs: 2,
        exclusive: false,
    };
    let cfg = GraphConfig { stream, freq: 220.0, amp: 0.5, gain_db: -6.0, smooth_ms: 5.0 };
    let frames = (seconds * 48_000.0).round() as usize;
    let mut g = Wo005Graph::new(cfg);
    let r = g.render(frames);
    let peak = r.samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    let rms = (r.samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        / r.samples.len() as f64)
        .sqrt();
    format!("hash={}\nrms={:.12}\npeak={:.9}\n", hex64(fnv1a64_f32(&r.samples)), rms, peak)
}
