//! `sparq exec` — render a patch through the WO-008 executor: registry → factory → graph →
//! blocks → WAV. This is the command that makes the whole WO-007 + WO-008 chain audible without
//! a sound card: the same offline determinism the golden path is built on, now driven by real
//! contract modules loaded from real manifests.
//!
//! Increment 1 renders the built-in `demo` patch (`sparq_audio::modules::demo_patch`: sine →
//! gain → master, rms analysis tap). Patch FILES arrive with WO-011's project format; until
//! then a `--patch` name other than `demo` is refused in words rather than guessed at.

use std::path::PathBuf;
use std::process::ExitCode;

use sparq_audio::executor::ExecConfig;
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::{builtin_sources, demo_patch, first_party, register_builtins};
use sparq_audio::wav::{write_wav, SampleFormat};
use sparq_module_api::discover;
use sparq_module_api::discovery::{Origin, Source};
use sparq_module_api::registry::Registry;

use crate::modules::scan;

/// A command's failure is a message shown verbatim to the operator.
type Result<T> = std::result::Result<T, String>;

/// Parsed `sparq exec` options.
#[derive(Clone, Debug)]
pub struct ExecOpts {
    /// Output WAV path.
    pub out: String,
    /// Seconds to render.
    pub seconds: f64,
    /// Sample rate.
    pub rate: u32,
    /// Block size.
    pub block: usize,
    /// Which patch (`demo` is the only one increment 1 has).
    pub patch: String,
    /// WAV sample format.
    pub format: String,
}

impl Default for ExecOpts {
    fn default() -> Self {
        Self {
            out: "exec.wav".to_string(),
            seconds: 5.0,
            rate: 48_000,
            block: 64,
            patch: "demo".to_string(),
            format: "f32".to_string(),
        }
    }
}

/// Parses `sparq exec` arguments.
///
/// # Errors
/// A message naming the unrecognised or unparseable flag.
pub fn parse(args: &[String]) -> Result<ExecOpts> {
    let mut o = ExecOpts::default();
    let mut i = 0;
    while i < args.len() {
        let key = args[i].as_str();
        let val = |i: &mut usize| -> Result<String> {
            *i += 1;
            args.get(*i).cloned().ok_or_else(|| format!("{key} needs a value"))
        };
        match key {
            "--out" => o.out = val(&mut i)?,
            "--seconds" => o.seconds = parse_num(&val(&mut i)?, key)?,
            "--rate" => o.rate = parse_num(&val(&mut i)?, key)?,
            "--block" => o.block = parse_num(&val(&mut i)?, key)?,
            "--patch" => o.patch = val(&mut i)?,
            "--format" => o.format = val(&mut i)?,
            other => return Err(format!("unknown exec option `{other}`")),
        }
        i += 1;
    }
    Ok(o)
}

fn parse_num<T: std::str::FromStr>(s: &str, key: &str) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    s.parse::<T>().map_err(|e| format!("{key}: `{s}` is not a valid number ({e})"))
}

fn sample_format(name: &str) -> Result<SampleFormat> {
    match name {
        "f32" => Ok(SampleFormat::Float32),
        "pcm16" => Ok(SampleFormat::Pcm16),
        "pcm24" => Ok(SampleFormat::Pcm24),
        "pcm32" => Ok(SampleFormat::Pcm32),
        other => Err(format!("unknown format `{other}` (try f32, pcm16, pcm24, pcm32)")),
    }
}

/// Runs the render. Prints the evidence lines (budget, meters, hash) and writes the WAV.
///
/// # Errors
/// A sentence for the operator: bad options, a registry that will not build, an executor
/// refusal, or a file that will not write.
pub fn run(opts: ExecOpts) -> Result<ExitCode> {
    let fmt = sample_format(&opts.format)?;
    if opts.patch != "demo" {
        return Err(format!(
            "unknown patch `{}` — increment 1 ships only `demo`; patch FILES arrive with the \
             WO-011 project format, and until then a name we cannot honour is refused rather \
             than guessed at",
            opts.patch
        ));
    }
    if opts.seconds <= 0.0 || opts.rate == 0 || opts.block == 0 {
        return Err("seconds, rate and block must all be positive".to_string());
    }

    println!("sparq exec · WO-008 executor render · patch `{}`", opts.patch);

    // ---- 1. the registry: built-ins first (§11 precedence), then whatever modules/ holds.
    let mut registry = Registry::new();
    let builtins = register_builtins(&mut registry)
        .map_err(|e| format!("built-in registration failed: {e}"))?;
    println!("  registry    {builtins} built-in module(s) compiled in");

    // Discovery over the on-disk manifests: the same files the built-ins embed. A duplicate id
    // here is EXPECTED (built-in wins, §11) and is reported as a shadow, not an error. A manifest
    // with no T1 factory in this build is reported honestly — T2/T3 tiers do not exist yet.
    let root = PathBuf::from("modules");
    if root.is_dir() {
        let files = scan(&root);
        let mut sources: Vec<Source> = builtin_sources();
        sources.extend(files.iter().map(|(path, text)| {
            Source::new(Origin::UserModules, path.display().to_string(), text.clone())
        }));
        let report = discover(sources);
        for f in &report.failed {
            println!("  REFUSED     {} — {}", f.label, f.report);
        }
        let mut shadowed = 0usize;
        let mut no_factory = 0usize;
        for loaded in &report.loaded {
            if loaded.origin != Origin::BuiltIn {
                let id = loaded.manifest.id();
                if first_party(id).is_some() {
                    shadowed += 1; // same id as a built-in: precedence resolves it, no action
                } else {
                    no_factory += 1;
                    println!(
                        "  NOTE        `{id}` loaded from {} but has no compiled-in factory — \
                         tier t2/t3 loading does not exist yet (ADR-002)",
                        loaded.label
                    );
                }
            }
        }
        println!(
            "  discovery   {} manifest(s) scanned under `{}` · {} shadowed by built-ins · {} without a factory",
            files.len(),
            root.display(),
            shadowed,
            no_factory
        );
    } else {
        println!("  discovery   no `modules/` directory here — running on built-ins alone");
    }

    // ---- 2. the patch: registry → factories → kernel graph → executor.
    let cfg = ExecConfig::new(opts.rate, opts.block, 2);
    let mut demo = demo_patch(&registry, cfg).map_err(|e| format!("patch build failed: {e}"))?;
    println!(
        "  patch       sine(440 Hz, 0.5) → gain(0.5) → master · rms analysis tap · {} node(s), {} edge(s)",
        demo.executor.order().len(),
        demo.executor.graph().edge_count()
    );
    // ADR-009 decision 4: the memory budget is printed at patch load.
    println!(
        "  budget      {} bytes of audio buffers, all pre-allocated",
        demo.executor.memory_budget_bytes()
    );

    // ---- 3. render.
    let frames = opts.block;
    let total_frames =
        ((opts.seconds * f64::from(opts.rate)).ceil() as usize / frames).max(1) * frames;
    let blocks = total_frames / frames;
    let mut samples: Vec<f32> = Vec::with_capacity(total_frames * cfg.device_channels);
    let mut out = vec![0.0f32; frames * cfg.device_channels];
    for _ in 0..blocks {
        demo.executor
            .render_block(demo.gain, &mut out)
            .map_err(|e| format!("render failed: {e}"))?;
        samples.extend_from_slice(&out);
    }

    // ---- 4. evidence: the analysis tap proves analysis-as-control-source, the hash is the
    // golden identity of this render (tests/modules_golden.rs asserts it). The tap VALUE is
    // output[0] of the rms node (the v0 cv convention); the master's meter is the audio one.
    let tap = demo.executor.node_output(demo.rms).and_then(|b| b.first().copied());
    let master = demo.executor.meter(demo.gain);
    println!(
        "  meters      rms tap value {:?} (0.25 amp sine, 0.59-cycle window → ≈ 0.16..0.19) · master peak {:?} rms {:?}",
        tap,
        master.map(|m| m.peak),
        master.map(|m| m.rms)
    );
    let hash = fnv1a64_f32(&samples);
    println!("  rendered    {blocks} blocks · {total_frames} frames · hash {}", hex64(hash));

    // ---- 5. write.
    let path = PathBuf::from(&opts.out);
    let written = write_wav(&path, opts.rate, cfg.device_channels as u16, fmt, &samples)
        .map_err(|e| format!("cannot write `{}`: {e}", path.display()))?;
    println!(
        "  wrote       {} ({} bytes of sample data, {} · {} frames × {} ch)",
        path.display(),
        written,
        opts.format,
        total_frames,
        cfg.device_channels
    );
    println!("exec: PASS");
    Ok(ExitCode::SUCCESS)
}
