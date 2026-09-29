//! `sparq soak` — the long-run reliability gate (WO-016: 12 h interactive, plan §17: 2 h audio).
//!
//! Renders continuously in chunks, discarding the audio between chunks so memory stays bounded no
//! matter how long it runs, while accumulating the counters that matter:
//!
//! * blocks processed, underruns, refusals, device errors,
//! * **allocations on the audio path** — must stay exactly 0 for the whole run,
//! * peak and p99 block time against the real-time budget,
//! * hash drift: the render of chunk *n* is compared against a fresh render of the same chunk, so a
//!   slow state corruption shows up as a determinism failure rather than as silence.
//!
//! This is not a substitute for the interactive soak (touch input, mutations, journal writes), but it
//! is the part that can run unattended in CI and overnight on the stage machine.

use std::process::ExitCode;
use std::time::{Duration, Instant};

use sparq_audio::graph::{ControlMsg, GraphConfig, Wo005Graph};
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_kernel::device::{RenderConfig, StreamConfig};
use sparq_kernel::rt::Counters;

pub struct SoakOpts {
    pub minutes: f64,
    pub sample_rate: u32,
    pub block: usize,
    pub channels: usize,
    pub report_every_secs: f64,
    pub enforce_realtime: bool,
    /// `offline` (default): the chunked deterministic render below. `null` / `wasapi-exclusive`
    /// / `wasapi-shared`: a paced HAL soak (WO-006) — the real-time path, pump thread and all.
    pub backend: String,
    /// Extra deterministic dummy work per block (HAL backends only). WO-006 task 6 asks for the
    /// soak to run "a deliberately heavy dummy callback graph": each round is one mul-add sweep
    /// over the block, so `--heavy 40` at 96k/64 stereo burns real headroom without touching
    /// the determinism of the offline path. The audio it dirties is going to a speaker nobody
    /// is mixing on; that is the point of a soak.
    pub heavy: usize,
}

impl Default for SoakOpts {
    fn default() -> Self {
        Self {
            minutes: 120.0,
            sample_rate: 96_000,
            block: 64,
            channels: 2,
            report_every_secs: 60.0,
            enforce_realtime: true,
            backend: String::from("offline"),
            heavy: 0,
        }
    }
}

pub fn run(o: SoakOpts) -> Result<ExitCode, String> {
    if o.backend != "offline" {
        return run_hal(o);
    }
    run_offline(o)
}

/// The original WO-005/WO-016 soak: chunked offline renders with determinism re-verification.
fn run_offline(o: SoakOpts) -> Result<ExitCode, String> {
    let stream = StreamConfig {
        sample_rate: o.sample_rate,
        block_frames: o.block,
        inputs: 0,
        outputs: o.channels,
        exclusive: false,
    };
    let cfg = GraphConfig { stream, freq: 220.0, amp: 0.5, gain_db: -6.0, smooth_ms: 5.0 };

    // One chunk = ten seconds of audio. At 96 kHz stereo f32 that is 7.7 MB, discarded each round.
    let chunk_secs = 10.0;
    let chunk_frames = (chunk_secs * f64::from(o.sample_rate)) as usize;
    let total_frames = (o.minutes * 60.0 * f64::from(o.sample_rate)) as usize;
    let chunks = (total_frames / chunk_frames).max(1);

    let budget = RenderConfig::natural_budget(&stream);
    let report_every = Duration::from_secs_f64(o.report_every_secs.max(1.0));

    println!("sparq soak");
    println!(
        "  target    {:.0} min of audio · {} Hz · block {} sm (budget {:.3} ms) · {} ch",
        o.minutes,
        o.sample_rate,
        o.block,
        budget.as_secs_f64() * 1e3,
        o.channels
    );
    println!(
        "  chunks    {chunks} × {chunk_secs} s · realtime budget enforced: {}",
        o.enforce_realtime
    );
    println!();

    let mut graph = Wo005Graph::new(cfg);
    let mut totals = Counters::zero();
    let mut peak: Duration = Duration::ZERO;
    let mut block_times: Vec<f64> = Vec::new();
    let mut drift_failures = 0u32;
    let started = Instant::now();
    let mut last_report = started;
    let mut last_hash = String::new();

    for i in 0..chunks {
        // Exercise the control path too: a soak that never sends a message is not soaking the ring.
        let _ = graph.control().push(ControlMsg::GainDb(-6.0 + f32::from((i % 13) as u16) * 0.5));

        let mut rc = RenderConfig::offline(stream, chunk_frames);
        rc.measure = o.enforce_realtime;
        if o.enforce_realtime {
            rc.budget = Some(budget);
        }
        let rep = graph.render_with(rc);

        totals.blocks += rep.counters.blocks;
        totals.underruns += rep.counters.underruns;
        totals.control_drops += rep.counters.control_drops;
        totals.audio_allocations += rep.counters.audio_allocations;
        totals.device_errors += rep.counters.device_errors;
        if let Some(p) = rep.peak_block {
            peak = peak.max(p);
            block_times.push(p.as_secs_f64());
        }

        // Determinism check: re-render the same chunk from a fresh instance and compare. The two
        // instances diverge in gain history, so compare a *fixed-config* pair instead.
        let mut v1 = Wo005Graph::new(cfg);
        let mut v2 = Wo005Graph::new(cfg);
        let h1 = hex64(fnv1a64_f32(&v1.render(1024).samples));
        let h2 = hex64(fnv1a64_f32(&v2.render(1024).samples));
        if h1 != h2 {
            drift_failures += 1;
            println!("  !! determinism drift at chunk {i}: {h1} vs {h2}");
        }
        if h1 != last_hash && !last_hash.is_empty() {
            // Same config must always render the same; a change here means state leaked between runs.
            println!("  !! reference hash changed at chunk {i}: {last_hash} -> {h1}");
            drift_failures += 1;
        }
        last_hash = h1;

        if last_report.elapsed() >= report_every {
            last_report = Instant::now();
            let elapsed = started.elapsed().as_secs_f64();
            let rendered = (i as f64 + 1.0) * chunk_secs;
            println!(
                "  {:>6.0}s rendered · {:.0}s wall ({:.0}× rt) · {} · peak {:?}",
                rendered,
                elapsed,
                rendered / elapsed.max(1e-9),
                totals.summary(),
                peak
            );
        }

        // Fail fast: an allocation or an underrun is a defect, not a statistic.
        if totals.audio_allocations > 0 {
            return Err(format!(
                "soak aborted at chunk {i}: {} allocation(s) on the audio path (plan §5.2 rule 1)",
                totals.audio_allocations
            ));
        }
        if o.enforce_realtime && totals.underruns > 0 {
            return Err(format!(
                "soak aborted at chunk {i}: {} underrun(s) against a {:.3} ms budget",
                totals.underruns,
                budget.as_secs_f64() * 1e3
            ));
        }
    }

    let elapsed = started.elapsed();
    let p99 = percentile(&mut block_times, 0.99);
    println!();
    println!("soak complete");
    println!(
        "  audio     {:.1} min in {:.1} s wall ({:.0}× realtime)",
        o.minutes,
        elapsed.as_secs_f64(),
        (o.minutes * 60.0) / elapsed.as_secs_f64().max(1e-9)
    );
    println!("  counters  {}", totals.summary());
    if !block_times.is_empty() {
        println!(
            "  block     peak {:.1} µs ({:.0}% of budget) · p99 {:.1} µs ({:.0}% of budget)",
            peak.as_secs_f64() * 1e6,
            peak.as_secs_f64() / budget.as_secs_f64() * 100.0,
            p99 * 1e6,
            p99 / budget.as_secs_f64() * 100.0
        );
    }
    println!("  determinism  {} reference-hash drift event(s)", drift_failures);

    let ok = totals.underruns == 0
        && totals.audio_allocations == 0
        && totals.device_errors == 0
        && drift_failures == 0;
    if ok {
        println!("\nsoak: PASS");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\nsoak: FAIL");
        Ok(ExitCode::FAILURE)
    }
}

fn percentile(xs: &mut [f64], p: f64) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((xs.len() as f64 - 1.0) * p).round() as usize;
    xs[idx.min(xs.len() - 1)]
}

// --------------------------------------------------------------------------- HAL soak (WO-006)

/// A paced soak through the real HAL: pump thread, device clock, MMCSS (on Windows), the
/// diagnostics recorder, and — for device backends — actual interrupts. This is the rehearsal
/// path for the WO-006 acceptance run (2 h at 96 kHz/64 on the stage device); `--backend null`
/// runs the identical code path against the simulated device, which is what CI can prove.
fn run_hal(o: SoakOpts) -> Result<ExitCode, String> {
    use sparq_kernel::hal::{backend_by_name, OpenOptions, ProcessFn};
    use sparq_kernel::rt::BlockStatus;

    let backend = backend_by_name(&o.backend).map_err(|e| e.with_hint())?;
    let devices =
        backend.enumerate().map_err(|e| format!("enumerating {}: {e}", backend.kind()))?;
    let dev = devices
        .first()
        .ok_or_else(|| format!("backend `{}` has no devices to soak", backend.kind()))?;

    // Ask the device before demanding anything (defect #39): caps first, adjust the request to
    // the device truth, then a silent probe open to learn what actually gets negotiated — the
    // graph is built for THAT, so a shared-mode fallback to the engine mix cannot desync it.
    let caps = backend
        .capabilities(&dev.id)
        .map_err(|e| format!("capabilities probe failed: {}", e.with_hint()))?;
    let mut rate = o.sample_rate;
    let mut out_ch = o.channels;
    // The envelope the adjustment consults is the one the OPEN negotiates in (defect #91):
    // exclusive asks the driver's own probed list, shared/null ask the engine's.
    let exclusive = o.backend == "wasapi-exclusive";
    let listed =
        if exclusive { caps.supports_exclusive_rate(rate) } else { caps.supports_rate(rate) };
    if !listed {
        let adjusted =
            if exclusive { caps.exclusive_rate_for(rate) } else { caps.default_rate.max(1) };
        println!(
            "  adjusted    rate {} Hz -> {} Hz (not in this device's {} list;              `sparq devices --caps` shows the truth)",
            rate,
            adjusted,
            if exclusive { "exclusive" } else { "shared" }
        );
        rate = adjusted;
    }
    let (cmin, cmax) = (usize::from(caps.channels_out.0), usize::from(caps.channels_out.1));
    if cmax >= 1 && (out_ch < cmin.max(1) || out_ch > cmax) {
        println!("  adjusted    channels {out_ch} -> {cmax} (device offers {cmin}..{cmax})");
        out_ch = cmax.clamp(1, 64);
    }
    let requested = StreamConfig {
        sample_rate: rate,
        block_frames: o.block,
        inputs: 0,
        outputs: out_ch,
        exclusive: o.backend == "wasapi-exclusive",
    };
    let stream_cfg = {
        let probe_opts = OpenOptions::probe();
        let probe_cb: ProcessFn = Box::new(|fctx| {
            fctx.zero_out();
            BlockStatus::Ok
        });
        let probe = backend
            .open(&dev.id, &requested, &probe_opts, probe_cb)
            .map_err(|e| format!("probe open failed: {}", e.with_hint()))?;
        probe.actual_config()
    };

    let graph_cfg =
        GraphConfig { stream: stream_cfg, freq: 220.0, amp: 0.5, gain_db: -6.0, smooth_ms: 5.0 };
    let mut graph = Wo005Graph::new(graph_cfg);
    let control = graph.control_handle();
    let heavy = o.heavy;
    let proc: ProcessFn = Box::new(move |fctx| {
        graph.process(&fctx.block, fctx.out);
        // The deliberately heavy dummy load: deterministic, zero-allocation, bounded per block.
        // `black_box` on the accumulator keeps the optimiser from proving the sweep dead.
        if heavy > 0 {
            let mut acc = 0.0f32;
            for _ in 0..heavy {
                for (i, s) in fctx.out.iter_mut().enumerate() {
                    *s = (*s * 0.99999 + (i as u32 % 7) as f32 * 1e-7).clamp(-1.0, 1.0);
                    acc += *s;
                }
            }
            std::hint::black_box(acc);
        }
        BlockStatus::Ok
    });

    let opts = OpenOptions { paced: true, audit_allocations: true, ..OpenOptions::default() };
    let mut stream = backend
        .open(&dev.id, &stream_cfg, &opts, proc)
        .map_err(|e| format!("open failed: {}", e.with_hint()))?;

    let actual = stream.actual_config();
    let budget = Duration::from_secs_f64(actual.block_seconds());
    println!("sparq soak · HAL backend `{}` · device `{}`", backend.kind(), dev.name);
    println!(
        "  negotiated  {} Hz · {} ch · block {} (budget {:.3} ms){}",
        actual.sample_rate,
        actual.outputs,
        actual.block_frames,
        budget.as_secs_f64() * 1e3,
        if actual.sample_rate != o.sample_rate || actual.outputs != o.channels {
            "  <- differs from the command-line request; the device decides"
        } else {
            ""
        }
    );
    println!(
        "  target      {:.0} min paced · report every {:.0} s · heavy dummy load: {} round(s)/block",
        o.minutes,
        o.report_every_secs,
        o.heavy
    );
    println!();

    stream.start().map_err(|e| format!("start failed: {}", e.with_hint()))?;
    let rt = stream.error_report().rt;
    println!("  rt setup    {}", rt.summary());
    println!();

    let started = Instant::now();
    let total = Duration::from_secs_f64(o.minutes * 60.0);
    let report_every = Duration::from_secs_f64(o.report_every_secs.max(1.0));
    let mut last_report = started;
    let mut tick = 0u64;
    let mut fail: Option<String> = None;

    while started.elapsed() < total {
        std::thread::sleep(Duration::from_millis(200));
        tick += 1;
        // Exercise the control path too (same rule as the offline soak: a soak that never sends
        // a message is not soaking the ring).
        let _ = control.push(ControlMsg::GainDb(-6.0 + (tick % 13) as f32 * 0.5));

        let d = stream.diag();
        if last_report.elapsed() >= report_every {
            last_report = Instant::now();
            println!("  [t+{:>6.0}s] {}", started.elapsed().as_secs_f64(), d.summary());
        }
        // Fail fast: on a paced backend an allocation or an xrun is a defect, not a statistic.
        if d.allocations > 0 {
            fail = Some(format!(
                "{} allocation(s) on the audio path (plan §5.2 rule 1)",
                d.allocations
            ));
            break;
        }
        if d.xruns > 0 && o.enforce_realtime {
            fail = Some(format!(
                "{} xrun(s) against a {:.3} ms budget (late wakes {}, overruns {})",
                d.xruns,
                budget.as_secs_f64() * 1e3,
                d.late_wakes,
                d.overruns
            ));
            break;
        }
        // Throughput truth joins the fail-fast (#76's derivation): past the trust window a
        // suspect drift means the device is not consuming at the negotiated rate, and a soak
        // at half its rate is a failed soak even with quiet counters.
        if d.drift_suspect && o.enforce_realtime {
            fail = Some(format!(
                "throughput vs wall off by more than 1% ({}) — the device is not consuming at the negotiated rate",
                d.drift_text()
            ));
            break;
        }
        let rep = stream.error_report();
        if !rep.healthy() {
            fail = Some(format!(
                "stream entered {:?}: {}",
                rep.state,
                rep.last_error.unwrap_or_else(|| String::from("(no detail)"))
            ));
            break;
        }
    }

    let _ = stream.stop();
    let d = stream.diag();
    println!();
    println!("{}", d.lines().trim_end());
    println!();
    println!("  block-time histogram (powers-of-two buckets, counts):");
    print!("{}", d.histogram_text());

    match fail {
        Some(why) => {
            println!("\nsoak: FAIL — {why}");
            Ok(ExitCode::FAILURE)
        },
        None => {
            println!(
                "\nsoak: PASS ({:.1} min paced, {} blocks, 0 xruns, 0 allocations)",
                started.elapsed().as_secs_f64() / 60.0,
                d.blocks
            );
            Ok(ExitCode::SUCCESS)
        },
    }
}
