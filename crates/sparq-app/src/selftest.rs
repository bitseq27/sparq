//! `sparq selftest` — runs the Phase 0 gates in-process and prints a report.
//!
//! This exists so the gate table in `PHASE0-WORKORDERS.md` §4 can be produced on any machine,
//! including a venue laptop with no toolchain, with one command. Every line is measured, not
//! asserted (plan §17 discipline).

use std::process::ExitCode;
use std::time::{Duration, Instant};

use sparq_audio::graph::{ControlMsg, GraphConfig, Wo005Graph};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_kernel::device::{RenderConfig, StreamConfig};

/// The canonical selftest configuration: 48 kHz, stereo, 64-frame blocks.
pub const fn selftest_stream() -> StreamConfig {
    StreamConfig { sample_rate: 48_000, block_frames: 64, inputs: 0, outputs: 2, exclusive: false }
}

pub fn run(with_golden: bool) -> Result<ExitCode, String> {
    let mut failures: Vec<String> = Vec::new();
    println!("sparq selftest · WO-005/WO-006 gates");
    println!(
        "  build     {} · {} · {:?}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::ARCH,
        cfg_profile()
    );

    // ---- gate 1: determinism (ADR-007) ---------------------------------------------
    let cfg = GraphConfig { stream: selftest_stream(), ..GraphConfig::default() };
    let frames = 48_000 * 5;
    let mut a = Wo005Graph::new(cfg);
    let mut b = Wo005Graph::new(cfg);
    let ra = a.render(frames);
    let rb = b.render(frames);
    let ha = hex64(fnv1a64_f32(&ra.samples));
    let hb = hex64(fnv1a64_f32(&rb.samples));
    check(
        &mut failures,
        "determinism: two renders bit-identical",
        ha == hb,
        format!("{ha} vs {hb}"),
    );

    // ---- gate 2: zero allocations on the audio path (plan §5.2 rule 1) --------------
    check(
        &mut failures,
        "rt discipline: 0 allocations on the audio path",
        ra.counters.audio_allocations == 0,
        format!("{} allocation(s)", ra.counters.audio_allocations),
    );

    // ---- gate 3: zero underruns offline --------------------------------------------
    check(
        &mut failures,
        "offline render: 0 underruns",
        ra.counters.underruns == 0,
        format!("{} underrun(s)", ra.counters.underruns),
    );

    // ---- gate 4: the underrun counter is not decorative ----------------------------
    // An impossible budget must be detected. This is the WO-005 acceptance criterion
    // "you can make it underrun deliberately and see the count rise".
    let mut c = Wo005Graph::new(cfg);
    let mut stall_cfg = RenderConfig::realtime(
        selftest_stream(),
        64 * 8,
        RenderConfig::natural_budget(&selftest_stream()),
    );
    // 64 frames at 48 kHz is a 1.333 ms budget; stall block 3 for 4 ms.
    stall_cfg.stall = Some((3, Duration::from_millis(4)));
    let rc = c.render_with(stall_cfg);
    check(
        &mut failures,
        "watchdog: deliberate 4 ms overload inside a 1.33 ms budget is counted",
        rc.counters.underruns >= 1,
        format!("{} underrun(s) of {} blocks", rc.counters.underruns, rc.counters.blocks),
    );

    // ---- gate 5: control ring delivers and drops loudly ----------------------------
    let d = Wo005Graph::new(cfg);
    let mut sent = 0usize;
    for i in 0..1000i16 {
        if d.control().push(ControlMsg::GainDb(f32::from(i % 40) - 40.0)) {
            sent += 1;
        }
    }
    let refusals = d.control().refusals();
    check(
        &mut failures,
        "control ring: refusals are counted, never silent (plan §4.3)",
        sent + refusals as usize == 1000,
        format!("{sent} accepted, {refusals} refused (ring capacity 64, nothing drained)"),
    );

    // ---- gate 6: throughput ---------------------------------------------------------
    let stream96 =
        StreamConfig { sample_rate: 96_000, block_frames: 64, outputs: 2, ..selftest_stream() };
    let cfg96 = GraphConfig { stream: stream96, ..GraphConfig::default() };
    let mut e = Wo005Graph::new(cfg96);
    let started = Instant::now();
    let re = e.render(96_000 * 60); // one minute of audio at 96 kHz
    let wall = started.elapsed();
    check(
        &mut failures,
        "throughput render stayed clean (0 alloc, 0 underrun)",
        re.counters.audio_allocations == 0 && re.counters.underruns == 0,
        re.counters.summary(),
    );
    let realtime_factor = 60.0 / wall.as_secs_f64();
    check(
        &mut failures,
        "throughput: 1 min at 96 kHz/64 renders >20× realtime",
        realtime_factor > 20.0,
        format!("{realtime_factor:.1}× realtime in {:.0} ms", wall.as_secs_f64() * 1e3),
    );

    // ---- gate 6b: the allocation gate is not decorative -----------------------------
    // Same discipline as the token and unsafe auditors: a gate that cannot fail is not a gate.
    // Measure a deliberately allocating loop through the same counter the audio path is judged by.
    let viol = {
        let base = sparq_kernel::alloc::start_counting();
        for i in 0..64u32 {
            let leaked: Vec<u8> = vec![0; i as usize + 1];
            std::hint::black_box(&leaked);
        }
        let n = sparq_kernel::alloc::allocation_count().saturating_sub(base);
        sparq_kernel::alloc::stop_counting();
        n
    };
    check(
        &mut failures,
        "allocation gate is live (a deliberate violation is caught)",
        sparq_kernel::alloc::audit_is_live() && viol >= 64,
        format!(
            "audit live: {} · violation observed {viol} allocations",
            sparq_kernel::alloc::audit_is_live()
        ),
    );

    // ---- gate 8: the HAL null backend keeps its promises (WO-006) -------------------
    // Conformance in-process: enumeration, lifecycle, latency math, fault counting, and the
    // reopen-without-leak cycle. The same suite runs against the WASAPI backends on Windows
    // (`sparq devices --conformance`) — this gate is its device-free rehearsal.
    {
        use sparq_kernel::hal::conformance::{conformance_offline, reopen_cycles_do_not_leak};
        use sparq_kernel::hal::null::NullBackend;
        let backend = NullBackend::new();
        let report = conformance_offline(&backend, false);
        let failed: Vec<&str> = report.iter().filter(|c| !c.passed).map(|c| c.name).collect();
        let leak = reopen_cycles_do_not_leak(&backend, 8);
        check(
            &mut failures,
            "HAL null conformance (WO-006)",
            failed.is_empty() && leak.passed,
            format!(
                "{} check(s){}, reopen-leak: {}",
                report.len(),
                if failed.is_empty() {
                    String::new()
                } else {
                    format!(", FAILED: {}", failed.join(", "))
                },
                leak.detail
            ),
        );
    }

    // ---- gate 7: golden reference ---------------------------------------------------
    if with_golden {
        match golden_check() {
            Ok(detail) => check(&mut failures, "golden reference matches", true, detail),
            Err(e) => check(&mut failures, "golden reference matches", false, e),
        }
    } else {
        println!("  [SKIP] golden reference (pass --golden)");
    }

    // ---- report --------------------------------------------------------------------
    println!("\n  render  {}", ra.summary());
    println!("  hash    {ha}");
    println!("  peak    {:.1} dB", db(ra.samples.iter().fold(0.0f32, |m, &s| m.max(s.abs()))));
    if failures.is_empty() {
        println!("\nselftest: PASS ({} gates)", 7 + usize::from(with_golden));
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\nselftest: FAIL");
        for f in &failures {
            println!("  - {f}");
        }
        Ok(ExitCode::FAILURE)
    }
}

/// Compare a fresh render against `reference/golden/wo005/manifest.txt`.
fn golden_check() -> Result<String, String> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference/golden/wo005/manifest.txt");
    let text = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("cannot read {}: {e}", manifest.display()))?;
    let mut expected_hash = None;
    let mut expected_rms = None;
    let mut tolerance: f64 = 0.0;
    for line in text.lines() {
        let mut it = line.split('=');
        let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
        match k.trim() {
            "hash" => expected_hash = Some(v.trim().to_string()),
            "rms" => expected_rms = v.trim().parse::<f64>().ok(),
            "tolerance" => tolerance = v.trim().parse::<f64>().unwrap_or(0.0),
            _ => {},
        }
    }
    let cfg = GraphConfig { stream: selftest_stream(), ..GraphConfig::default() };
    let mut g = Wo005Graph::new(cfg);
    let rep = g.render(48_000);
    let got = hex64(fnv1a64_f32(&rep.samples));
    let rms = (rep.samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        / rep.samples.len() as f64)
        .sqrt();
    let want_hash = expected_hash.ok_or("manifest has no hash= line")?;
    if got != want_hash {
        let want_rms = expected_rms.unwrap_or(rms);
        return Err(format!(
            "hash {got} != {want_hash} (rms {:.6} vs {:.6}, tolerance {:.1e}); a DSP change altered the output — if it was intentional, regenerate with `xtask golden`",
            rms, want_rms, tolerance
        ));
    }
    if let Some(want) = expected_rms {
        if (rms - want).abs() > tolerance.max(1e-9) {
            return Err(format!("rms {rms:.6} outside tolerance {tolerance:.1e} of {want:.6}"));
        }
    }
    Ok(format!("hash {got} · rms {rms:.6}"))
}

fn check(failures: &mut Vec<String>, name: &str, ok: bool, detail: String) {
    println!("  [{}] {name} — {detail}", if ok { "PASS" } else { "FAIL" });
    if !ok {
        failures.push(format!("{name}: {detail}"));
    }
}

fn db(v: f32) -> f32 {
    if v <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * v.log10()
    }
}

fn cfg_profile() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}
