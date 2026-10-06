//! Argument parsing. Hand-rolled on purpose: the core binary has no third-party dependencies, so a
//! clap-class crate would be the only one, and it would be the first thing to break the "hermetic
//! CI on a device-less runner" property (ADR-001, plan §16.1).

use std::process::ExitCode;

use crate::{demo, devices, exec, instruments, modules, play, render, selftest, soak, streams, ui};

/// Parsed `render` options.
pub struct RenderOpts {
    pub out: String,
    pub seconds: f64,
    pub sample_rate: u32,
    pub block: usize,
    pub channels: usize,
    pub freq: f64,
    pub amp: f32,
    pub gain_db: f32,
    pub format: String,
}

impl Default for RenderOpts {
    fn default() -> Self {
        Self {
            out: String::from("sparq-render.wav"),
            seconds: 10.0,
            sample_rate: 48_000,
            block: 64,
            channels: 2,
            freq: 220.0,
            amp: 0.5,
            gain_db: -6.0,
            format: String::from("f32"),
        }
    }
}

const USAGE: &str = r#"sparq — Phase 0 (WO-005 bootstrap + WO-006 HAL)

USAGE
  sparq render  [--out FILE] [--seconds N] [--rate HZ] [--block N] [--ch N]
                [--freq HZ] [--amp A] [--gain DB] [--format f32|pcm16|pcm24|pcm32]
  sparq selftest [--golden]
  sparq demo    [--out FILE] [--bpm N] [--bars N] [--rate HZ] [--ch N] [--seed N]
                [--gain DB] [--delay-send 0..1] [--crush-bits 1..24] [--format f32|pcm16|pcm24]
                [--pattern NAME] [--mutation NAME] [--transpose N] [--show-dna] [--list-patterns]
  sparq soak    [--minutes N] [--rate HZ] [--block N] [--ch N] [--report-every S] [--no-realtime]
                [--backend offline|null|wasapi-exclusive|wasapi-shared] [--heavy N]
  sparq devices [--backend NAME] [--caps] [--conformance]
  sparq modules [--root DIR] [--strict]     discover modules/ and report every refusal verbatim
  sparq mod validate DIR               the hand-in gate (MODULE-BUILD-GUIDE §7): static stages run,
                                       runtime stages refuse in words; PARTIAL exits non-zero
  sparq mod list [--root DIR] [--strict]
                                       the launch view of instruments/: what loads, what is
                                       refused and why, verbatim (WO-018)
  sparq streams list                   the stream registry as data (WO-020): every stream's domain,
                                       cadence, view, schema and key state (KEY NEEDED in words)
  sparq streams cache [path|clear]     the disk cache location / last-good payloads (hermetic)
  sparq streams probe [--live] | fetch ID [--out FILE] | tail ID | record-fixtures --out DIR
                                       the live transport verbs; they REFUSE IN WORDS and exit
                                       non-zero without --features streams-net (the device
                                       increment, WO-020 INC5). Fixtures record with python:
                                       tools/streams_record.py (needs --features streams to list)
  sparq exec    [--out FILE] [--seconds N] [--rate HZ] [--block N] [--patch demo|mod-demo|drum-demo]
                [--format f32|pcm16|pcm24|pcm32]
                render a patch through the WO-008 executor: registry → graph → WAV, no device
  sparq golden-values        print the hash/rms/peak lines for the golden manifest
  sparq play    [--seconds N] [--rate HZ] [--block N] [--ch N] [--freq HZ] [--amp A] [--gain DB]
                [--backend NAME] [--device N] [--write-wav FILE] [--list-devices]
  sparq ui      [--headless N] [--audit] [--width PX] [--height PX] [--scale F] [--contrast]
  sparq version
  sparq help

BACKENDS (play/soak --backend)
  offline            soak only: the chunked deterministic render (the golden/determinism path)
  null               the HAL virtual device: paced pump, full diagnostics, no hardware needed
  wasapi-exclusive   the stage path: lowest latency, no mixer (Windows, --features hal-wasapi)
  wasapi-shared      the compatible path: the Windows mixer stays in charge (same feature)
  (any other name)   play only: routes to the disposable cpal bootstrap (ADR-008), e.g. `wasapi`
  asio               refused with an honest message until WO-006 increment 2

NOTES
  `render`, `selftest`, `devices`, `exec`, `soak --backend null` and `play --backend null` need
  no audio device and no features; they are the CI path.
  `play` with a cpal backend name needs `--features bootstrap-audio`; the WASAPI backends need
  `--features hal-wasapi` (Windows). scripts\build.bat enables both.
  Terminal keys during `play`:  ] / [ gain +1/-1 dB   f/F freq x2 //2   m mute   q quit
"#;

type Result<T> = std::result::Result<T, String>;

pub fn run(args: &[String]) -> Result<ExitCode> {
    let (cmd, rest) = match args.split_first() {
        None => {
            print!("{USAGE}");
            return Ok(ExitCode::SUCCESS);
        },
        Some((c, r)) => (c.as_str(), r),
    };
    match cmd {
        "help" | "--help" | "-h" => {
            print!("{USAGE}");
            Ok(ExitCode::SUCCESS)
        },
        "version" | "--version" | "-V" => {
            println!("sparq {}", crate::stamp::line());
            println!("  module api v0 (unfrozen) · kernel {}", sparq_kernel::VERSION);
            println!();
            println!("  If that source stamp is older than your last sync, this binary is stale.");
            println!("  Fix: run scripts\\build.bat, or delete target\\release\\sparq.exe.");
            Ok(ExitCode::SUCCESS)
        },
        "render" => render::run(parse_render(rest)?),
        "golden-values" => {
            print!("{}", render::golden_values(1.0));
            Ok(ExitCode::SUCCESS)
        },
        "selftest" => selftest::run(rest.contains(&"--golden".to_string())),
        "demo" => demo::run(parse_demo(rest)?),
        "soak" => soak::run(parse_soak(rest)?),
        "devices" => devices::run(parse_devices(rest)?),
        "modules" => modules::run(modules::parse(rest)?),
        "mod" => instruments::run(instruments::parse(rest)?),
        "instrument" => instruments::run_instrument(instruments::parse_instrument(rest)?),
        "streams" => streams::run(streams::parse(rest)?),
        "exec" => exec::run(exec::parse(rest)?),
        "play" => play::run(parse_play(rest)?),
        "ui" => {
            Ok(ExitCode::from(u8::try_from(ui::run(parse_ui(rest)?).clamp(0, 255)).unwrap_or(2)))
        },
        other => Err(format!("unknown command `{other}`; try `sparq help`")),
    }
}

fn parse_render(args: &[String]) -> Result<RenderOpts> {
    let mut o = RenderOpts::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let val = |i: &mut usize| -> Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--out" => o.out = val(&mut i)?,
            "--seconds" => o.seconds = parse(&val(&mut i)?, key)?,
            "--rate" => o.sample_rate = parse(&val(&mut i)?, key)?,
            "--block" => o.block = parse(&val(&mut i)?, key)?,
            "--ch" => o.channels = parse(&val(&mut i)?, key)?,
            "--freq" => o.freq = parse(&val(&mut i)?, key)?,
            "--amp" => o.amp = parse(&val(&mut i)?, key)?,
            "--gain" => o.gain_db = parse(&val(&mut i)?, key)?,
            "--format" => o.format = val(&mut i)?,
            other => return Err(format!("unknown render option `{other}`")),
        }
        i += 1;
    }
    if o.block == 0 || !o.block.is_power_of_two() {
        return Err(format!("--block must be a power of two, got {}", o.block));
    }
    if o.channels == 0 || o.channels > 64 {
        return Err(format!("--ch must be 1..=64 (plan §9.6), got {}", o.channels));
    }
    Ok(o)
}

/// The fields below are read by the bootstrap audio path, which only exists with the
/// `bootstrap-audio` feature (ADR-008). Without it they are parsed and then unused, which is
/// deliberate: the CLI surface must not change shape between configurations.
#[cfg_attr(not(feature = "bootstrap-audio"), allow(dead_code))]
pub struct PlayOpts {
    /// Stop after this many seconds (disables key input; prints a status line every 2 s).
    pub seconds: Option<f64>,
    pub sample_rate: u32,
    pub block: usize,
    pub freq: f64,
    pub amp: f32,
    pub gain_db: f32,
    /// Backend selector. HAL names (any build): `null`, `wasapi-exclusive`, `wasapi-shared`,
    /// `asio` (honest refusal until increment 2). Any other name routes to the cpal bootstrap
    /// (e.g. `wasapi`, `directsound`, `mme`) and needs `--features bootstrap-audio`.
    pub backend: Option<String>,
    /// Output device index within the chosen backend.
    pub device: Option<usize>,
    /// Requested channel count; `None` means "use the device default".
    pub channels: Option<usize>,
    /// Also capture the output to a WAV file while playing.
    pub write_wav: Option<String>,
    /// Just enumerate devices on every backend and exit.
    pub list_devices: bool,
}

fn parse_play(args: &[String]) -> Result<PlayOpts> {
    let mut o = PlayOpts {
        seconds: None,
        sample_rate: 48_000,
        block: 64,
        freq: 220.0,
        amp: 0.5,
        gain_db: -12.0,
        backend: None,
        device: None,
        channels: None,
        write_wav: None,
        list_devices: false,
    };
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let val = |i: &mut usize| -> Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--seconds" => o.seconds = Some(parse(&val(&mut i)?, key)?),
            "--rate" => o.sample_rate = parse(&val(&mut i)?, key)?,
            "--block" => o.block = parse(&val(&mut i)?, key)?,
            "--freq" => o.freq = parse(&val(&mut i)?, key)?,
            "--amp" => o.amp = parse(&val(&mut i)?, key)?,
            "--gain" => o.gain_db = parse(&val(&mut i)?, key)?,
            "--backend" | "-b" => o.backend = Some(val(&mut i)?),
            "--device" | "-d" => o.device = Some(parse(&val(&mut i)?, key)?),
            "--ch" => o.channels = Some(parse(&val(&mut i)?, key)?),
            "--write-wav" => o.write_wav = Some(val(&mut i)?),
            "--list-devices" => o.list_devices = true,
            other => return Err(format!("unknown play option `{other}`")),
        }
        i += 1;
    }
    Ok(o)
}

/// Options for `sparq demo` (the Phase B patch).
pub struct DemoOpts {
    pub out: String,
    pub bpm: f64,
    pub bars: usize,
    pub sample_rate: u32,
    pub channels: usize,
    pub seed: u64,
    pub gain_db: f32,
    pub delay_send: f64,
    pub crush_bits: u32,
    pub format: String,
    /// Also print machine-readable `key=value` lines for the golden manifest.
    pub golden_values: bool,
    /// Rhythm style name (`techno`, `breakcore`, `glitch`, `driving`).
    pub style: String,
    /// Mutation preset name, or `none`.
    pub mutation: String,
    /// Bass transpose in semitones.
    pub transpose: i8,
    /// Print the mutation lineage and exit.
    pub show_dna: bool,
    /// List the styles and mutation presets, then exit.
    pub list_patterns: bool,
}

impl Default for DemoOpts {
    fn default() -> Self {
        Self {
            out: String::from("sparq-demo.wav"),
            bpm: 138.0,
            bars: 4,
            sample_rate: 48_000,
            channels: 2,
            seed: 0xA17E,
            gain_db: -6.0,
            delay_send: 0.25,
            crush_bits: 24,
            format: String::from("f32"),
            golden_values: false,
            style: String::from("techno"),
            mutation: String::from("none"),
            transpose: 0,
            show_dna: false,
            list_patterns: false,
        }
    }
}

fn parse_demo(args: &[String]) -> Result<DemoOpts> {
    let mut o = DemoOpts::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let val = |i: &mut usize| -> Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--out" => o.out = val(&mut i)?,
            "--bpm" => o.bpm = parse(&val(&mut i)?, key)?,
            "--bars" => o.bars = parse(&val(&mut i)?, key)?,
            "--rate" => o.sample_rate = parse(&val(&mut i)?, key)?,
            "--ch" => o.channels = parse(&val(&mut i)?, key)?,
            "--seed" => o.seed = parse_hex_or_dec(&val(&mut i)?, key)?,
            "--gain" => o.gain_db = parse(&val(&mut i)?, key)?,
            "--delay-send" => o.delay_send = parse(&val(&mut i)?, key)?,
            "--crush-bits" => o.crush_bits = parse(&val(&mut i)?, key)?,
            "--format" => o.format = val(&mut i)?,
            "--golden-values" => o.golden_values = true,
            "--pattern" | "--style" => o.style = val(&mut i)?,
            "--mutation" => o.mutation = val(&mut i)?,
            "--transpose" => o.transpose = parse(&val(&mut i)?, key)?,
            "--show-dna" => o.show_dna = true,
            "--list-patterns" => o.list_patterns = true,
            other => return Err(format!("unknown demo option `{other}`; try `sparq help`")),
        }
        i += 1;
    }
    if o.bars == 0 || o.bars > 256 {
        return Err(format!("--bars must be 1..=256, got {}", o.bars));
    }
    if o.channels == 0 || o.channels > 64 {
        return Err(format!("--ch must be 1..=64, got {}", o.channels));
    }
    if !(o.crush_bits >= 1 && o.crush_bits <= 24) {
        return Err(format!("--crush-bits must be 1..=24, got {}", o.crush_bits));
    }
    if !(0.0..=1.0).contains(&o.delay_send) {
        return Err(format!("--delay-send must be 0..=1, got {}", o.delay_send));
    }
    if o.bpm <= 0.0 {
        return Err("--bpm must be positive".into());
    }
    Ok(o)
}

/// Parse a number that may be decimal or `0x`-prefixed hex (seeds are usually quoted in hex).
fn parse_hex_or_dec(s: &str, key: &str) -> Result<u64> {
    let t = s.trim();
    let (body, radix) = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        (h, 16)
    } else {
        (t, 10)
    };
    u64::from_str_radix(body, radix)
        .map_err(|e| format!("{key}: `{s}` is not a valid integer ({e})"))
}

/// Parse `sparq ui` options. `--headless N` and `--audit` never open a window (CI-safe);
/// with neither, the window host runs (needs `--features ui-window`).
fn parse_ui(args: &[String]) -> Result<ui::UiOptions> {
    let mut o = ui::UiOptions::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let val = |i: &mut usize| -> Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--headless" => o.headless_frames = parse(&val(&mut i)?, key)?,
            "--audit" => o.audit = true,
            "--width" => o.width = parse(&val(&mut i)?, key)?,
            "--height" => o.height = parse(&val(&mut i)?, key)?,
            "--scale" => o.scale = parse(&val(&mut i)?, key)?,
            "--contrast" => o.contrast = true,
            "--svg-out" => o.svg_out = Some(val(&mut i)?),
            "--review" => o.review = true,
            other => return Err(format!("unknown ui option `{other}`")),
        }
        i += 1;
    }
    if o.width <= 0.0 || o.height <= 0.0 {
        return Err("--width/--height must be positive".into());
    }
    if o.scale <= 0.0 {
        return Err("--scale must be positive".into());
    }
    Ok(o)
}

fn parse_soak(args: &[String]) -> Result<soak::SoakOpts> {
    let mut o = soak::SoakOpts::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let val = |i: &mut usize| -> Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--minutes" => o.minutes = parse(&val(&mut i)?, key)?,
            "--rate" => o.sample_rate = parse(&val(&mut i)?, key)?,
            "--block" => o.block = parse(&val(&mut i)?, key)?,
            "--ch" => o.channels = parse(&val(&mut i)?, key)?,
            "--report-every" => o.report_every_secs = parse(&val(&mut i)?, key)?,
            "--no-realtime" => o.enforce_realtime = false,
            "--backend" => o.backend = val(&mut i)?,
            "--heavy" => o.heavy = parse(&val(&mut i)?, key)?,
            other => return Err(format!("unknown soak option `{other}`")),
        }
        i += 1;
    }
    if o.block == 0 || !o.block.is_power_of_two() {
        return Err(format!("--block must be a power of two, got {}", o.block));
    }
    match o.backend.as_str() {
        "offline" | "null" | "wasapi-exclusive" | "wasapi-shared" | "wasapi-x" | "wasapi-s" => {},
        other => {
            return Err(format!(
                "unknown soak --backend `{other}`. The soak knows: offline (default), null, \
                 wasapi-exclusive, wasapi-shared. (`play` also accepts cpal host names; the \
                 soak deliberately does not — it measures the HAL, not the bootstrap.)"
            ));
        },
    }
    Ok(o)
}

fn parse_devices(args: &[String]) -> Result<devices::DevicesOpts> {
    let mut o = devices::DevicesOpts::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        match key {
            "--backend" => {
                i += 1;
                o.backend =
                    Some(args.get(i).cloned().ok_or_else(|| format!("{key} needs a value"))?);
            },
            "--caps" => o.caps = true,
            "--conformance" => o.conformance = true,
            other => {
                return Err(format!(
                    "unknown devices option `{other}` (try --backend, --caps, --conformance)"
                ))
            },
        }
        i += 1;
    }
    Ok(o)
}

fn parse<T: std::str::FromStr>(s: &str, key: &str) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    s.parse::<T>().map_err(|e| format!("{key}: `{s}` is not a valid number ({e})"))
}
