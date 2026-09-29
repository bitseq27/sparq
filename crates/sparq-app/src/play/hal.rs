//! `sparq play` through the real HAL (WO-006): null, WASAPI exclusive, WASAPI shared.
//!
//! What this path has that the bootstrap (ADR-008, `bootstrap.rs`) does not:
//!
//! * **No lock on the audio path.** The graph is *owned* by the callback closure; the terminal
//!   thread talks to it through the `Arc` control-ring handle. The bootstrap's
//!   `Mutex<Wo005Graph>` — the dated, documented violation in clippy.toml — does not exist here.
//! * **RT discipline applied and reported.** MMCSS/priority/working-set results print at start
//!   (Windows) and ride in every status line, so a machine where the boost silently did nothing
//!   says so.
//! * **Real diagnostics**: xruns, callback p50/p99/max, wake jitter, device-clock drift,
//!   allocations on the audio path — the [`sparq_kernel::hal::diag`] recorder, not ad-hoc atomics.
//! * **Honest negotiation, in two phases**: a silent probe open learns what the device actually
//!   negotiates (shared-mode ladders can land on the engine's mix format — the RDP lesson), the
//!   graph is built for *that*, and the real stream reports request vs actual on one line.
//!
//! `--write-wav` works on the null backend (capture is a stream feature there). On WASAPI it is
//! refused with a pointer to increment 2 (loopback capture) and `scripts\diag.bat` — a silent
//! refusal would recreate exactly the "did sparq make sound?" ambiguity Phase A existed to kill.

use std::io::Read;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use sparq_audio::graph::{ControlMsg, GraphConfig, Wo005Graph};
use sparq_audio::wav::{write_wav, SampleFormat as WavFormat};
use sparq_kernel::device::StreamConfig;
use sparq_kernel::hal::{backend_by_kind, BackendKind, OpenOptions, ProcessFn};
use sparq_kernel::rt::BlockStatus;

use crate::cli::PlayOpts;

/// Run `play` against a HAL backend. `kind` is already validated by the router in `play.rs`.
pub fn run(o: PlayOpts, kind: BackendKind) -> Result<ExitCode, String> {
    let backend = backend_by_kind(kind).map_err(|e| e.with_hint())?;

    println!("sparq play · HAL path (WO-006) · backend {kind}");
    println!("  sparq       {}", crate::stamp::line());
    println!();

    // ---- device ---------------------------------------------------------------------
    let devices = backend
        .enumerate()
        .map_err(|e| format!("enumerating {kind} devices: {}", e.with_hint()))?;
    if devices.is_empty() {
        return Err(format!(
            "backend `{kind}` reports NO output devices.\n  \
             Run `sparq devices` to see every backend; on Windows check the audio service and \
             whether this session hides the hardware (WINDOWS.md -> RDP)."
        ));
    }
    println!("  output devices:");
    for (i, d) in devices.iter().enumerate() {
        let marker = if d.is_default_output { " <- default" } else { "" };
        println!("    [{i}] {}{}", d.name, marker);
    }
    let dev_index = o.device.unwrap_or(0);
    let dev = devices.get(dev_index).ok_or_else(|| {
        format!(
            "--device {dev_index} out of range: `{kind}` has {} device(s), indices 0..{}",
            devices.len(),
            devices.len() - 1
        )
    })?;
    println!("  using       [{}] {}", dev_index, dev.name);

    // ---- config: ask the device, don't assume (defect #39) ----------------------------
    // Caps come first so a 44.1 kHz-only endpoint is not insulted with a 48 kHz request:
    // adjust to the device truth BEFORE opening, and say so.
    let caps = backend
        .capabilities(&dev.id)
        .map_err(|e| format!("capabilities probe failed: {}", e.with_hint()))?;
    let asked_channels = o.channels.unwrap_or(2);
    let mut rate = o.sample_rate;
    let mut out_ch = asked_channels;
    // The envelope this adjustment consults is the one the OPEN negotiates in (defect #91):
    // exclusive asks the DRIVER (its own probed rate list), shared/null ask the ENGINE. An
    // exclusive request adjusted through the shared sieve is how test004 attempt 3's 48 kHz
    // ask became the one 96 kHz stream the driver accepted and could not sustain.
    let exclusive = kind == BackendKind::WasapiExclusive;
    let listed =
        if exclusive { caps.supports_exclusive_rate(rate) } else { caps.supports_rate(rate) };
    if !listed {
        let adjusted =
            if exclusive { caps.exclusive_rate_for(rate) } else { caps.default_rate.max(1) };
        println!(
            "  adjusted    rate {} Hz -> {} Hz (this device does not list {} Hz for {} \
             operation; `sparq devices --caps` shows what it does)",
            rate,
            adjusted,
            rate,
            if exclusive { "exclusive" } else { "shared" }
        );
        rate = adjusted;
    }
    let (cmin, cmax) = (usize::from(caps.channels_out.0), usize::from(caps.channels_out.1));
    if cmax >= 1 && (out_ch < cmin.max(1) || out_ch > cmax) {
        println!("  adjusted    channels {out_ch} -> {cmax} (device offers {cmin}..{cmax})");
        out_ch = cmax.clamp(1, 64);
    }
    let req = StreamConfig {
        sample_rate: rate,
        block_frames: o.block,
        inputs: 0,
        outputs: out_ch,
        exclusive: kind == BackendKind::WasapiExclusive,
    };

    // ---- phase 1: probe open — learn what the device ACTUALLY negotiates ----------------
    // Shared-mode ladders can land on the engine's mix rate/channels (rung 5 on RDP endpoints);
    // the graph must be built for the negotiated truth, not the request. The probe is a silent
    // open that is never started and is dropped before the real open — control-thread only,
    // cheap, and it turns "surprise mid-stream format" into "known before a sample exists".
    let probe_opts = OpenOptions::probe();
    let probe_cb: ProcessFn = Box::new(|fctx| {
        fctx.zero_out();
        BlockStatus::Ok
    });
    let cfg = {
        let probe = backend
            .open(&dev.id, &req, &probe_opts, probe_cb)
            .map_err(|e| format!("open failed: {}", e.with_hint()))?;
        probe.actual_config()
    };

    // ---- phase 2: the real stream, graph built for the negotiated config ----------------
    let graph_cfg =
        GraphConfig { stream: cfg, freq: o.freq, amp: o.amp, gain_db: o.gain_db, smooth_ms: 5.0 };

    // --write-wav: capture is a null-backend feature (increment 1). Bound it like the bootstrap
    // did (60 s) so a long run cannot exhaust memory.
    let capture_frames = match (&o.write_wav, kind) {
        (Some(_), BackendKind::Null) => {
            let secs = o.seconds.unwrap_or(60.0).min(60.0);
            (secs * f64::from(cfg.sample_rate)) as usize * cfg.outputs
        },
        (Some(_), _) => {
            return Err(String::from(
                "--write-wav is not available on device backends in WO-006 increment 1: \
                 loopback capture is increment 2. To separate a DSP problem from a device-routing \
                 problem today: run `sparq play --backend null --write-wav out.wav` (proves the \
                 engine) and `scripts\\diag.bat` (probes the device path).",
            ));
        },
        (None, _) => 0,
    };

    let mut graph = Wo005Graph::new(graph_cfg);
    let control = graph.control_handle();
    let proc: ProcessFn = Box::new(move |fctx| {
        // The whole audio path: one graph call. No lock, no clock, no allocation — and the HAL
        // pump *measures* the no-allocation claim every block (OpenOptions::audit_allocations).
        graph.process(&fctx.block, fctx.out);
        BlockStatus::Ok
    });

    let opts = OpenOptions {
        audit_allocations: true,
        paced: true,
        capture_frames,
        ideal_processor: None,
        working_set_lock: true,
        log_negotiation: true,
    };

    let mut stream = backend
        .open(&dev.id, &cfg, &opts, proc)
        .map_err(|e| format!("open failed (phase 2): {}", e.with_hint()))?;

    let actual = stream.actual_config();
    if actual.sample_rate != o.sample_rate || actual.outputs != asked_channels {
        println!(
            "  NEGOTIATED  {} Hz · {} ch · block {} (asked {} Hz · {} ch) — the device gets \
             what it asked for, and this line is the proof",
            actual.sample_rate, actual.outputs, actual.block_frames, o.sample_rate, asked_channels
        );
    }
    println!("  latency     {}", stream.latency_report().summary());
    println!(
        "  signal      syn/sine {:.2} Hz @ {:.3} -> util/gain {:+.1} dB -> out",
        o.freq, o.amp, o.gain_db
    );
    if let Some(w) = &o.write_wav {
        println!("  wav         capturing to {w} (null backend, max 60 s)");
    }
    println!();
    println!("  ] / [  gain +1/-1 dB     f / F  freq x2 / /2     m  mute     q  quit");
    println!("  (with --seconds, key input is disabled and a status line prints every 2 s)");
    println!();

    stream.start().map_err(|e| format!("start failed: {}", e.with_hint()))?;
    let rt = stream.error_report().rt;
    if cfg!(windows) {
        println!("  rt setup    {}", rt.summary());
        if !rt.core_applied() {
            println!("  WARNING     MMCSS/priority not applied — xruns under load are expected;");
            println!("              see docs/hal/windows-notes.md (power plan / session type).");
        }
    }
    println!();

    // ---- control loop --------------------------------------------------------------------
    let started = Instant::now();
    let limit = o.seconds.map(Duration::from_secs_f64);
    let mut last_report = started;
    let mut gain = o.gain_db;
    let mut freq = o.freq;
    let mut muted = false;

    let mut stdin = std::io::stdin().lock();
    let mut byte = [0u8; 1];
    loop {
        if let Some(l) = limit {
            if started.elapsed() >= l {
                break;
            }
        }
        // The stream failing underneath us (unplug, driver reset) must end the loop cleanly —
        // the WO-006 acceptance criterion, exercised live.
        let report = stream.error_report();
        if !report.healthy() {
            eprintln!(
                "\n  stream entered {:?}: {}",
                report.state,
                report.last_error.unwrap_or_else(|| String::from("(no detail)"))
            );
            break;
        }

        let key = if limit.is_none() {
            match stdin.read(&mut byte) {
                Ok(0) | Err(_) => None,
                Ok(_) => Some(byte[0]),
            }
        } else {
            std::thread::sleep(Duration::from_millis(50));
            None
        };
        match key {
            Some(b'q') => break,
            Some(b']') => {
                gain += 1.0;
                muted = false;
                let _ = control.push(ControlMsg::GainDb(gain));
                println!("  gain -> {gain:+.1} dB");
            },
            Some(b'[') => {
                gain -= 1.0;
                muted = false;
                let _ = control.push(ControlMsg::GainDb(gain));
                println!("  gain -> {gain:+.1} dB");
            },
            Some(b'f') => {
                freq *= 2.0;
                let _ = control.push(ControlMsg::FreqHz(freq));
                println!("  freq -> {freq:.2} Hz");
            },
            Some(b'F') => {
                freq /= 2.0;
                let _ = control.push(ControlMsg::FreqHz(freq));
                println!("  freq -> {freq:.2} Hz");
            },
            Some(b'm') => {
                muted = !muted;
                let _ = control.push(ControlMsg::GainDb(if muted { -120.0 } else { gain }));
                println!(
                    "  {}",
                    if muted { String::from("muted") } else { format!("unmuted ({gain:+.1} dB)") }
                );
            },
            Some(b'\n') | Some(b'\r') => {},
            Some(_) => {},
            None => {},
        }

        if limit.is_some() && last_report.elapsed() >= Duration::from_secs(2) {
            last_report = Instant::now();
            println!("  [t+{:>5.1}s] {}", started.elapsed().as_secs_f64(), stream.diag().summary());
        }
    }

    // ---- teardown + final report ---------------------------------------------------------
    let _ = control.push(ControlMsg::GainDb(-120.0)); // don't click on stop
    stream.stop().map_err(|e| format!("stop reported: {e}"))?;

    let diag = stream.diag();
    let rep = stream.error_report();
    println!();
    println!("  final state {:?}", rep.state);
    println!("{}", diag.lines().trim_end());
    println!();
    println!("  latency     {}", stream.latency_report().summary());

    // The pass/fail line, in the shape the soak and the acceptance runs use. Suspect drift
    // joins the verdict: past the trust window the number is throughput truth (#76's
    // derivation), and a stream that ran at half its negotiated rate is not a passing stream
    // even when every wake was on cadence and every counter is zero.
    let clean = diag.is_clean()
        && !diag.drift_suspect
        && matches!(rep.state, sparq_kernel::hal::StreamState::Stopped);
    if clean {
        println!("\nplay: PASS (0 xruns, 0 allocations, clean stop)");
    } else {
        println!(
            "\nplay: NOT CLEAN — xruns {}, allocs {}, dev-err {}, state {:?}{}. \
             Copy this whole output into a message back.",
            diag.xruns,
            diag.allocations,
            diag.device_errors,
            rep.state,
            if diag.drift_suspect {
                ", and the throughput did not match the negotiated rate — the drift line above is the measurement"
            } else {
                ""
            }
        );
    }

    // ---- optional capture ----------------------------------------------------------------
    if let Some(path) = &o.write_wav {
        if let Some(samples) = stream.captured() {
            let actual = stream.actual_config();
            let fmt =
                if path.ends_with(".f32.wav") { WavFormat::Float32 } else { WavFormat::Pcm16 };
            write_wav(path, actual.sample_rate, actual.outputs as u16, fmt, samples)
                .map_err(|e| format!("writing {path}: {e}"))?;
            println!("  wrote {} frames to {path}", samples.len() / actual.outputs.max(1));
        } else {
            println!("  NOTE: capture was enabled but the backend produced no samples");
        }
    }

    Ok(if clean { ExitCode::SUCCESS } else { ExitCode::FAILURE })
}
