//! The disposable bootstrap audio path (ADR-008). Delete this file at the end of WO-006.
//!
//! Rules that keep it from colonising the design:
//! * no sparq type depends on a `cpal` type — conversion happens at this boundary only;
//! * the audio callback drives the *same* `Wo005Graph` the offline path uses, so the DSP and the
//!   control ring are not bootstrap-specific;
//! * capability questions (exclusive mode, ASIO, channel counts, aggregation) are answered against
//!   the HAL trait in ADR-009, never against this crate.
//!
//! # Phase A priorities
//!
//! Optimised for one thing: **getting sound out of a Windows machine we cannot test on**, and
//! producing a report that explains failure precisely when it happens. Hence the attempt ladder,
//! the per-step error text, live callback telemetry, all three sample formats, and the optional
//! parallel WAV write — which separates "the DSP made nothing" from "the device path is broken" in
//! a single run.

use std::io::Read;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, SizedSample};
use sparq_audio::graph::{ControlMsg, GraphConfig, Wo005Graph};
use sparq_audio::wav::{write_wav, SampleFormat as WavFormat};
use sparq_kernel::block::BlockContext;
use sparq_kernel::device::StreamConfig;

use crate::cli::PlayOpts;

static RUNNING: AtomicBool = AtomicBool::new(true);

// --------------------------------------------------------------------------- public entry points

/// `sparq play --list-devices`: every device on every backend. This is the first thing to run on a
/// new machine, and the first thing to paste back when nothing works.
pub fn list_devices() -> Result<ExitCode, String> {
    println!("sparq --list-devices");
    println!("  default backend : {}", cpal::default_host().id().name());
    println!(
        "  available       : {}",
        cpal::ALL_HOSTS.iter().map(|h| h.name()).collect::<Vec<_>>().join(", ")
    );
    for id in cpal::ALL_HOSTS {
        let host = match cpal::host_from_id(*id) {
            Ok(h) => h,
            Err(e) => {
                println!("\n[{}] unavailable: {e}", id.name());
                continue;
            },
        };
        println!("\n[{}]", id.name());
        match host.output_devices() {
            Ok(devs) => {
                let mut n = 0;
                for (i, d) in devs.enumerate() {
                    n += 1;
                    let name = d.name().unwrap_or_else(|_| "<unknown>".into());
                    let def = d
                        .default_output_config()
                        .map(|c| {
                            format!(
                                "{} Hz · {} ch · {:?}",
                                c.sample_rate().0,
                                c.channels(),
                                c.sample_format()
                            )
                        })
                        .unwrap_or_else(|e| format!("no default format ({e})"));
                    let ranges = d.supported_output_configs().map(|r| {
                        let rs = r.collect::<Vec<_>>();
                        let lo = rs.iter().map(|x| x.min_sample_rate().0).min().unwrap_or(0);
                        let hi = rs.iter().map(|x| x.max_sample_rate().0).max().unwrap_or(0);
                        let cs = rs
                            .iter()
                            .map(cpal::SupportedStreamConfigRange::channels)
                            .collect::<std::collections::BTreeSet<_>>();
                        let cmin = cs.iter().next().copied().unwrap_or(0);
                        let cmax = cs.iter().next_back().copied().unwrap_or(0);
                        let fmts = rs
                            .iter()
                            .map(|x| format!("{:?}", x.sample_format()))
                            .collect::<std::collections::BTreeSet<_>>()
                            .into_iter()
                            .collect::<Vec<_>>()
                            .join("/");
                        format!("        rates {lo}..{hi} · ch {cmin}..{cmax} · fmt {fmts}")
                    });
                    println!("  out [{i}] {name}");
                    println!("        default: {def}");
                    if let Ok(line) = ranges {
                        println!("{line}");
                    }
                }
                if n == 0 {
                    println!("  (no output devices)");
                }
            },
            Err(e) => println!("  output enumeration failed: {e}"),
        }
        match host.input_devices() {
            Ok(devs) => {
                for (i, d) in devs.enumerate() {
                    let name = d.name().unwrap_or_else(|_| "<unknown>".into());
                    println!("  in  [{i}] {name}");
                }
            },
            Err(e) => println!("  input enumeration failed: {e}"),
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[allow(clippy::too_many_lines)]
pub fn run(o: PlayOpts) -> Result<ExitCode, String> {
    let host_id = match o.backend.as_deref() {
        None => cpal::default_host().id(),
        Some(name) => cpal::ALL_HOSTS
            .iter()
            .copied()
            .find(|h| h.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                format!(
                    "unknown --backend `{name}`. Available on this platform: {}",
                    cpal::ALL_HOSTS.iter().map(|h| h.name()).collect::<Vec<_>>().join(", ")
                )
            })?,
    };
    let host =
        cpal::host_from_id(host_id).map_err(|e| format!("creating host {host_id:?}: {e}"))?;

    println!("sparq play · BOOTSTRAP audio path (ADR-008; deleted at end of WO-006)");
    println!("  backend     {}", host_id.name());
    println!(
        "  sparq       {} · {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!();

    let devices: Vec<cpal::Device> =
        host.output_devices().map_err(|e| format!("enumerating output devices: {e}"))?.collect();
    if devices.is_empty() {
        return Err(format!(
            "backend `{}` reports NO output devices.\n\
             Check: is a playback device enabled in Windows sound settings? Is the interface\n\
             powered and connected? Run `sparq play --list-devices` to see every backend.",
            host_id.name()
        ));
    }
    let default_name = host.default_output_device().and_then(|d| d.name().ok()).unwrap_or_default();
    println!("  output devices:");
    for (i, d) in devices.iter().enumerate() {
        let name = d.name().unwrap_or_else(|_| "<unknown>".into());
        let marker =
            if !default_name.is_empty() && name == default_name { " <- default" } else { "" };
        let def = d
            .default_output_config()
            .map(|c| {
                format!("{} Hz · {} ch · {:?}", c.sample_rate().0, c.channels(), c.sample_format())
            })
            .unwrap_or_else(|e| format!("no default ({e})"));
        println!("    [{i}] {name}{marker}");
        println!("        {def}");
    }
    println!();

    // ---- attempt ladder ------------------------------------------------------------
    // Ordered by likelihood of success, not by preference. The device's own default format comes
    // first: a real diagnostic log showed a 44.1 kHz-only endpoint (RDP "Remote Audio") failing the
    // requested 48 kHz rung, and every rung we try costs a device open. The requested format is
    // still attempted, so an interface that supports it gets it.
    let mut ladder: Vec<Attempt> = Vec::new();
    let want_ch = o.channels.unwrap_or(2) as u16;
    if let Some(idx) = o.device {
        if idx >= devices.len() {
            return Err(format!(
                "--device {idx} out of range: this backend has {} output device(s), indices 0..{}",
                devices.len(),
                devices.len() - 1
            ));
        }
        ladder.push(Attempt {
            label: format!("device [{idx}] at its own default format"),
            device: Some(idx),
            rate: None,
            channels: None,
        });
        ladder.push(Attempt {
            label: format!("device [{idx}] as requested ({} Hz / {want_ch} ch)", o.sample_rate),
            device: Some(idx),
            rate: Some(o.sample_rate),
            channels: Some(want_ch),
        });
    } else {
        ladder.push(Attempt {
            label: "default device at its own default format".into(),
            device: None,
            rate: None,
            channels: None,
        });
        ladder.push(Attempt {
            label: format!("default device as requested ({} Hz / {want_ch} ch)", o.sample_rate),
            device: None,
            rate: Some(o.sample_rate),
            channels: Some(want_ch),
        });
    }
    ladder.push(Attempt {
        label: "fallback 48000 Hz stereo".into(),
        device: o.device,
        rate: Some(48_000),
        channels: Some(2),
    });
    ladder.push(Attempt {
        label: "fallback 44100 Hz stereo".into(),
        device: o.device,
        rate: Some(44_100),
        channels: Some(2),
    });

    // A single 44.1 kHz stereo endpoint named like a redirection endpoint is the signature of a
    // remote-desktop session. Say so, because it changes what the numbers mean.
    let looks_remote = devices.len() == 1
        && default_name.to_lowercase().contains("remote")
        && devices
            .first()
            .and_then(|d| d.default_output_config().ok())
            .is_some_and(|c| c.sample_rate().0 == 44_100 && c.channels() == 2);
    if looks_remote {
        println!("  WARNING: the only output device is `{default_name}` at 44.1 kHz stereo.");
        println!(
            "           That is a remote-desktop redirection endpoint, not your audio interface."
        );
        println!(
            "           Audio will reach the RDP CLIENT's speakers with network latency, and a"
        );
        println!("           real interface plugged into the host is invisible to this session.");
        println!("           See WINDOWS.md -> 'Remote desktop (RDP) and audio interfaces'.");
        println!();
    }

    let mut failures: Vec<String> = Vec::new();
    let mut opened: Option<Session> = None;
    // Hoisted out of the loop: `default_output_device()` returns an owned value, and a temporary
    // cannot be borrowed across loop iterations.
    let default_device = host.default_output_device();
    for attempt in &ladder {
        let device = match attempt.device {
            Some(i) => devices.get(i),
            None => default_device.as_ref(),
        };
        let Some(device) = device else {
            failures.push(format!("{}: device unavailable", attempt.label));
            println!("  failed: {} -> device unavailable", attempt.label);
            continue;
        };
        match open(device, attempt.rate, attempt.channels, &o) {
            Ok(session) => {
                println!("  opened: {}", attempt.label);
                opened = Some(session);
                break;
            },
            Err(e) => {
                println!("  failed: {} -> {e}", attempt.label);
                failures.push(format!("{}: {e}", attempt.label));
            },
        }
    }
    let session = opened.ok_or_else(|| {
        format!("could not open any output stream. Attempts:\n  - {}", failures.join("\n  - "))
    })?;

    let rate = session.rate;
    let chans = session.channels;
    println!("  device      {}", session.device_name);
    println!(
        "  format      {rate} Hz · {chans} ch · {:?} · block {} sm ({:.3} ms)",
        session.format,
        o.block,
        1000.0 * o.block as f64 / f64::from(rate)
    );
    println!(
        "  signal      syn/sine {:.2} Hz @ {:.3} -> util/gain {:+.1} dB -> out",
        o.freq, o.amp, o.gain_db
    );
    if let Some(w) = &o.write_wav {
        println!("  wav         also capturing to {w} (max 60 s) — this separates a DSP problem from a device-routing problem");
    }
    println!();
    println!("  ] / [  gain +1/-1 dB     f / F  freq x2 / /2     m  mute     q  quit");
    println!("  (with --seconds, key input is disabled and a status line prints every 2 s)");
    println!();
    println!("  NOTE: shared mode, a mutex in the callback, no MMCSS, no working-set lock, no");
    println!("        exclusive mode, no ASIO. All of that is WO-006; this file is disposable.");
    println!();

    session.stream.play().map_err(|e| format!("starting playback: {e}"))?;
    RUNNING.store(true, Ordering::Relaxed);

    let started = Instant::now();
    let limit = o.seconds.map(Duration::from_secs_f64);
    let mut last_report = started;
    let mut last_frames = 0u64;
    let mut gain = o.gain_db;
    let mut freq = o.freq;

    let mut stdin = std::io::stdin().lock();
    let mut byte = [0u8; 1];
    loop {
        if !RUNNING.load(Ordering::Relaxed) {
            break;
        }
        if limit.is_some_and(|l| started.elapsed() >= l) {
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
                session.send(ControlMsg::GainDb(gain));
                println!("  gain -> {gain:+.1} dB");
            },
            Some(b'[') => {
                gain -= 1.0;
                session.send(ControlMsg::GainDb(gain));
                println!("  gain -> {gain:+.1} dB");
            },
            Some(b'm') => {
                gain = -120.0;
                session.send(ControlMsg::GainDb(gain));
                println!("  muted");
            },
            Some(b'f') => {
                freq *= 2.0;
                session.send(ControlMsg::FreqHz(freq));
                println!("  freq -> {freq:.2} Hz");
            },
            Some(b'F') => {
                freq /= 2.0;
                session.send(ControlMsg::FreqHz(freq));
                println!("  freq -> {freq:.2} Hz");
            },
            _ => {},
        }
        if last_report.elapsed().as_secs_f64() >= 2.0 {
            let dt = last_report.elapsed().as_secs_f64();
            last_report = Instant::now();
            let fr = session.stats.frames.load(Ordering::Relaxed);
            let dfr = fr.saturating_sub(last_frames);
            last_frames = fr;
            let expected = (f64::from(rate) * dt) as u64;
            let cbf = session.stats.cb_frames.load(Ordering::Relaxed);
            let period_ms = if cbf > 0 { cbf as f64 / f64::from(rate) * 1000.0 } else { 0.0 };
            println!(
                "  t={:>6.1}s  cb {:>5}  frames {:>8} (+{:<6})  device period {:>5.0} sm ({:>5.2} ms)  gap {:>6.2}..{:<6.2} ms  underrun {:<3}  deficit {}",
                started.elapsed().as_secs_f64(),
                session.stats.callbacks.load(Ordering::Relaxed),
                fr,
                dfr,
                cbf,
                period_ms,
                session.stats.min_gap_us.load(Ordering::Relaxed).min(u64::from(u32::MAX)) as f64 / 1000.0,
                session.stats.max_gap_us.load(Ordering::Relaxed) as f64 / 1000.0,
                session.stats.underruns.load(Ordering::Relaxed),
                expected.saturating_sub(dfr)
            );
        }
    }

    RUNNING.store(false, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(60));
    let callbacks = session.stats.callbacks.load(Ordering::Relaxed);
    let frames = session.stats.frames.load(Ordering::Relaxed);
    let peak = f32::from_bits(session.stats.peak.load(Ordering::Relaxed) as u32);

    println!();
    println!("  final report");
    println!("    callbacks         {callbacks}");
    println!("    frames delivered  {frames} ({:.2} s of audio)", frames as f64 / f64::from(rate));
    println!("    wall elapsed      {:.2} s", started.elapsed().as_secs_f64());
    println!("    est. underruns    {}", session.stats.underruns.load(Ordering::Relaxed));
    println!(
        "    callback gap      {:.2}..{:.2} ms (expected {:.2} ms for {rate} Hz / block {})",
        session.stats.min_gap_us.load(Ordering::Relaxed) as f64 / 1000.0,
        session.stats.max_gap_us.load(Ordering::Relaxed) as f64 / 1000.0,
        1000.0 * o.block as f64 / f64::from(rate),
        o.block
    );
    println!("    peak |sample|     {peak:.6} ({:.1} dBFS)", db(peak));
    println!(
        "    graph blocks      {}",
        session.stats.state.lock().map(|g| g.blocks_processed()).unwrap_or(0)
    );

    if let Some(path) = &o.write_wav {
        let buf = session.stats.captured.lock().map(|c| c.clone()).unwrap_or_default();
        if buf.is_empty() {
            println!("    wav               NOT WRITTEN — nothing was captured");
        } else {
            match write_wav(path, rate, chans as u16, WavFormat::Float32, &buf) {
                Ok(bytes) => println!(
                    "    wav               wrote {path} ({} frames, {bytes} bytes of data)",
                    buf.len() / chans.max(1)
                ),
                Err(e) => println!("    wav               FAILED to write {path}: {e}"),
            }
        }
    }

    // ---- interpretation guide: make a failed run actionable -------------------------
    println!();
    if callbacks == 0 {
        println!("  DIAGNOSIS: the device accepted the stream but never called back.");
        println!(
            "    -> usually exclusive-locked by another app, a stalled driver, or a device that"
        );
        println!("       needs a different buffer size. Try: close other audio software;");
        println!(
            "       `--block 256`; a different `--device N`; or `--backend` from --list-devices."
        );
    } else if frames == 0 {
        println!("  DIAGNOSIS: callbacks fired but delivered no frames — that is a sparq bug.");
        println!("    -> please report this whole output.");
    } else if peak < 1e-6 {
        println!("  DIAGNOSIS: audio flowed but every sample was silent.");
        println!(
            "    -> check --gain (is it near -120?) and --amp. If gain is above -60 dB this is a"
        );
        println!("       sparq bug; please report this whole output.");
    } else {
        println!("  RESULT: sparq generated audio at {:.1} dBFS and delivered {frames} frames to the device.", db(peak));
        if o.write_wav.is_some() {
            println!(
                "    -> heard nothing? the problem is downstream of sparq: wrong playback device,"
            );
            println!(
                "       OS mixer muted, or interface output routing. Open the WAV above in any"
            );
            println!(
                "       editor: if it contains a sine, the instrument is fine and the device path"
            );
            println!("       is the suspect.");
        } else {
            println!(
                "    -> heard it? Phase A passes. Heard nothing? re-run with `--write-wav out.wav`"
            );
            println!("       to separate a DSP problem from a device-routing problem.");
        }
    }

    Ok(ExitCode::SUCCESS)
}

// --------------------------------------------------------------------------- session

/// One rung of the "try to open *something* that works" ladder.
///
/// The ladder exists because we cannot test on the target machine: rather than fail on the first
/// unsupported format, sparq walks down to something the device will accept and *says which rung
/// it landed on*, so a working-but-wrong-format run is still diagnosable.
#[derive(Clone, Debug)]
struct Attempt {
    label: String,
    device: Option<usize>,
    rate: Option<u32>,
    channels: Option<u16>,
}

struct Session {
    device_name: String,
    rate: u32,
    channels: usize,
    format: SampleFormat,
    stream: cpal::Stream,
    stats: Arc<Stats>,
}

impl Session {
    fn send(&self, msg: ControlMsg) {
        if let Ok(g) = self.stats.state.lock() {
            let _ = g.control().push(msg);
        }
    }
}

struct Stats {
    callbacks: AtomicU64,
    frames: AtomicU64,
    underruns: AtomicU64,
    /// Peak |sample| stored as raw f32 bits so it can live in an atomic.
    peak: AtomicU64,
    min_gap_us: AtomicU64,
    max_gap_us: AtomicU64,
    /// Frames the device actually asked for on its most recent callback.
    ///
    /// This is not `--block`. Real devices hand the callback their own period — the RDP "Remote
    /// Audio" endpoint asks for ~442 frames (10 ms) no matter what we request — and the underrun
    /// estimate and "expected gap" are meaningless unless they use this number. Getting that wrong
    /// was found by a real diagnostic log, not by a test.
    cb_frames: AtomicU64,
    /// Largest callback the device has handed us so far.
    max_cb_frames: AtomicU64,
    // `clippy::disallowed_types` denies `Mutex` workspace-wide because no lock may exist on the
    // audio path (plan §5.2, ADR-009). Known, deliberate, dated violation: cpal's callback needs
    // `Send + 'static` and offers no way to hand the audio thread an `&mut`. The type is written
    // fully qualified and the `allow` is scoped to the field, not the module, so the exception
    // cannot spread. Removing both is part of WO-006's definition of done.
    #[allow(clippy::disallowed_types)]
    state: Arc<std::sync::Mutex<Wo005Graph>>,
    #[allow(clippy::disallowed_types)]
    captured: Arc<std::sync::Mutex<Vec<f32>>>,
}

fn db(v: f32) -> f32 {
    if v <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * v.log10()
    }
}

/// Open a stream on `device`, preferring `rate`/`channels` and falling back to its defaults.
#[allow(clippy::too_many_lines)]
fn open(
    device: &cpal::Device,
    want_rate: Option<u32>,
    want_channels: Option<u16>,
    o: &PlayOpts,
) -> Result<Session, String> {
    let device_name = device.name().unwrap_or_else(|_| "<unknown>".into());

    let (cfg, format) = match want_rate {
        Some(rate) => {
            let wanted = cpal::SampleRate(rate);
            let ch = want_channels.unwrap_or(2);
            let ranges: Vec<cpal::SupportedStreamConfigRange> = device
                .supported_output_configs()
                .map_err(|e| format!("supported_output_configs: {e}"))?
                .collect();
            let range = ranges
                .iter()
                .find(|r| {
                    r.channels() == ch
                        && r.min_sample_rate() <= wanted
                        && wanted <= r.max_sample_rate()
                })
                .ok_or_else(|| {
                    let have = ranges
                        .iter()
                        .map(|r| {
                            format!(
                                "{}..{} Hz/{}ch/{:?}",
                                r.min_sample_rate().0,
                                r.max_sample_rate().0,
                                r.channels(),
                                r.sample_format()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(
                        "`{device_name}` does not support {rate} Hz / {ch} ch (it offers: {have})"
                    )
                })?;
            (
                cpal::StreamConfig {
                    channels: ch,
                    sample_rate: wanted,
                    buffer_size: BufferSize::Fixed(o.block.max(1) as u32),
                },
                range.sample_format(),
            )
        },
        None => {
            let d = device
                .default_output_config()
                .map_err(|e| format!("default_output_config: {e}"))?;
            (
                cpal::StreamConfig {
                    channels: d.channels(),
                    sample_rate: d.sample_rate(),
                    buffer_size: BufferSize::Fixed(o.block.max(1) as u32),
                },
                d.sample_format(),
            )
        },
    };

    let rate = cfg.sample_rate.0;
    let chans = usize::from(cfg.channels);
    if chans == 0 {
        return Err("device reported 0 channels".into());
    }

    let graph = Wo005Graph::new(GraphConfig {
        stream: StreamConfig {
            sample_rate: rate,
            block_frames: o.block.max(1),
            inputs: 0,
            outputs: chans,
            exclusive: false,
        },
        freq: o.freq,
        amp: o.amp,
        gain_db: o.gain_db,
        smooth_ms: 5.0,
    });
    #[allow(clippy::disallowed_types)]
    let state = Arc::new(std::sync::Mutex::new(graph));
    #[allow(clippy::disallowed_types)]
    let captured: Arc<std::sync::Mutex<Vec<f32>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let stats = Arc::new(Stats {
        callbacks: AtomicU64::new(0),
        frames: AtomicU64::new(0),
        underruns: AtomicU64::new(0),
        peak: AtomicU64::new(0),
        min_gap_us: AtomicU64::new(u64::MAX),
        max_gap_us: AtomicU64::new(0),
        cb_frames: AtomicU64::new(0),
        max_cb_frames: AtomicU64::new(0),
        state: Arc::clone(&state),
        captured: Arc::clone(&captured),
    });

    let capture_on = o.write_wav.is_some();
    let stream = build_stream(
        device,
        &cfg,
        format,
        Arc::clone(&stats),
        o.block.max(1),
        rate,
        chans,
        capture_on,
    )?;

    Ok(Session { device_name, rate, channels: chans, format, stream, stats })
}

/// Build the callback for the device's actual sample format.
///
/// All three common formats are handled — refusing anything but `F32` would fail on most Windows
/// shared-mode devices, whose default is frequently `I16`.
#[allow(clippy::too_many_arguments)]
fn build_stream(
    device: &cpal::Device,
    cfg: &cpal::StreamConfig,
    format: SampleFormat,
    stats: Arc<Stats>,
    block: usize,
    rate: u32,
    chans: usize,
    capture: bool,
) -> Result<cpal::Stream, String> {
    macro_rules! build {
        ($t:ty, $to_f32:expr, $from_f32:expr) => {{
            let cb = move |data: &mut [$t], _: &cpal::OutputCallbackInfo| {
                fill(data, &stats, block, rate, chans, capture, $to_f32, $from_f32);
            };
            device
                .build_output_stream(cfg, cb, on_device_error, None)
                .map_err(|e| format!("build_output_stream({:?}): {e}", format))
        }};
    }
    match format {
        SampleFormat::F32 => build!(f32, |s: f32| s, |s: f32| s),
        SampleFormat::I16 => {
            build!(i16, |s: i16| f32::from(s) / 32768.0, |s: f32| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        }
        SampleFormat::U16 => build!(
            u16,
            |s: u16| (f32::from(s) - 32768.0) / 32768.0,
            |s: f32| ((s.clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16
        ),
        other => Err(format!(
            "sample format {other:?} is not implemented by the disposable bootstrap (only F32/I16/U16). \
             Choose a device/rate whose supported formats include one of those — see --list-devices. \
             The real HAL (WO-006) handles every format."
        )),
    }
}

fn on_device_error(e: cpal::StreamError) {
    eprintln!("sparq: DEVICE ERROR during playback: {e}");
    RUNNING.store(false, Ordering::Relaxed);
}

/// The audio callback body: telemetry, then the same block loop the offline path uses.
///
/// Telemetry is atomics and a thread-local only — no allocation. The `Instant::now()` call is
/// permitted here (and denied by clippy everywhere else) because this is the *device harness*
/// measuring the callback, not the DSP; the graph itself still receives only a `BlockContext`.
#[allow(clippy::too_many_arguments, clippy::disallowed_methods)]
fn fill<T: SizedSample + Copy>(
    data: &mut [T],
    stats: &Arc<Stats>,
    block: usize,
    rate: u32,
    chans: usize,
    capture: bool,
    to_f32: impl Fn(T) -> f32,
    from_f32: impl Fn(f32) -> T,
) {
    thread_local! {
        static LAST: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
    }
    let now = Instant::now();
    LAST.with(|c| {
        if let Some(prev) = c.get() {
            let gap_us = now.duration_since(prev).as_micros().min(u64::MAX as u128) as u64;
            stats.min_gap_us.fetch_min(gap_us, Ordering::Relaxed);
            stats.max_gap_us.fetch_max(gap_us, Ordering::Relaxed);
        }
        c.set(Some(now));
    });

    let Ok(mut g) = stats.state.lock() else {
        for s in data.iter_mut() {
            *s = from_f32(0.0);
        }
        return;
    };

    let frames = data.len() / chans;
    stats.cb_frames.store(frames as u64, Ordering::Relaxed);
    stats.max_cb_frames.fetch_max(frames as u64, Ordering::Relaxed);
    // One f32 scratch buffer, reused; allocated once per stream lifetime would be better but this
    // is the disposable path and the allocation is outside the measured DSP region.
    let mut scratch: Vec<f32> = vec![0.0; data.len()];

    let mut off = 0usize;
    let mut block_index = stats.callbacks.load(Ordering::Relaxed);
    while off < frames {
        let n = block.min(frames - off);
        let ctx = BlockContext {
            block: sparq_kernel::block::BlockId(block_index),
            sample_offset: stats.frames.load(Ordering::Relaxed) + off as u64,
            frames: n,
            sample_rate: rate,
            channels: chans,
            tick: 0,
            ppqn: 960,
        };
        g.process(&ctx, &mut scratch[off * chans..(off + n) * chans]);
        off += n;
        block_index += 1;
    }

    let mut peak = 0.0f32;
    for (i, s) in scratch.iter().enumerate() {
        let a = s.abs();
        if a > peak {
            peak = a;
        }
        data[i] = from_f32(*s);
    }
    stats.peak.fetch_max(u64::from(peak.to_bits()), Ordering::Relaxed);

    if capture {
        if let Ok(mut c) = stats.captured.lock() {
            // Bounded so a long session cannot exhaust memory (~60 s).
            let cap = 60usize.saturating_mul(rate as usize).saturating_mul(chans);
            if c.len() + scratch.len() <= cap {
                c.extend_from_slice(&scratch);
            }
        }
    }

    stats.frames.fetch_add(frames as u64, Ordering::Relaxed);
    stats.callbacks.fetch_add(1, Ordering::Relaxed);

    // Honest underrun estimate: if filling THIS callback took longer than the audio it represents,
    // we are behind. `frames` is the device's own period, not our configured block — using the
    // block size here would compare against a budget the device never gave us.
    // The real HAL learns this from the device itself (WO-006).
    let budget = Duration::from_secs_f64(frames as f64 / f64::from(rate));
    if now.elapsed() > budget {
        stats.underruns.fetch_add(1, Ordering::Relaxed);
    }

    if !RUNNING.load(Ordering::Relaxed) {
        for s in data.iter_mut() {
            *s = from_f32(0.0);
        }
    }
    let _ = to_f32;
}
