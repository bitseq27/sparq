//! `sparq exec` — render a patch through the WO-008 executor: registry → factory → graph →
//! blocks → WAV. This is the command that makes the whole WO-007 + WO-008 chain audible without
//! a sound card: the same offline determinism the golden path is built on, now driven by real
//! contract modules loaded from real manifests.
//!
//! Increment 1 renders the built-in `demo` patch (`sparq_audio::modules::demo_patch`: sine →
//! gain → master, rms analysis tap). Contract v1 (WO-008 increment 4) adds `mod-demo`
//! (`sparq_audio::modules::mod_demo_patch`): the rms→filter modulation demo — WO-014's
//! analysis-as-control-source acceptance, audible. Patch FILES arrive with WO-011's project
//! format; until then any other `--patch` name is refused in words rather than guessed at.

use std::path::PathBuf;
use std::process::ExitCode;

use sparq_audio::executor::ExecConfig;
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::modules::{
    builtin_sources, demo_patch, drum_demo_patch, first_party, mod_demo_patch, register_builtins,
};
use sparq_audio::wav::{write_wav, SampleFormat};
use sparq_module_api::discover;
use sparq_module_api::discovery::{Origin, Source};
use sparq_module_api::registry::Registry;
use sparq_music::transport::{BlockEvents, Transport};

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
    if opts.patch != "demo" && opts.patch != "mod-demo" && opts.patch != "drum-demo" {
        return Err(format!(
            "unknown patch `{}` — this build ships `demo`, `mod-demo` (the contract-v1 \
             rms→filter modulation demo) and `drum-demo` (the contract-v1 event wire: host \
             triggers → membrane → mixer); patch FILES arrive with the WO-011 project format, \
             and until then a name we cannot honour is refused rather than guessed at",
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
    let mut transport: Option<Transport> = None;
    let (mut executor, master, rms_node, drum_node, description) = if opts.patch == "mod-demo" {
        let d = mod_demo_patch(&registry, cfg).map_err(|e| format!("patch build failed: {e}"))?;
        (
            d.executor,
            d.svf,
            Some(d.rms),
            None,
            "sine(440 Hz, 0.5) → gain(0.5) → svf(bp 200 Hz, mod 1.0) = master · rms.level → \
             svf.cutoff-mod (the contract-v1 cv wire)"
                .to_string(),
        )
    } else if opts.patch == "drum-demo" {
        let d = drum_demo_patch(&registry, cfg).map_err(|e| format!("patch build failed: {e}"))?;
        // WO-009: the triggers are the TRANSPORT's — beat/bar triggers from `sparq-music`,
        // published through the executor's host-event door. The block-counter schedule this
        // command used before contract-v1's clocks landed stays pinned by the batch-3 gate
        // (`DrumDemoPatch::is_kick_block`); the golden is the same hash either way, and
        // tests/transport_drum.rs is the cross-check that says so.
        let mut tr = Transport::new(opts.rate, 120.0, 960);
        tr.play();
        transport = Some(tr);
        (
            d.executor,
            d.mixer,
            None,
            Some(d.membrane),
            "membrane → mixer(in-0 → out-0) = master · four-on-the-floor at 120 BPM, \
             transport-driven (sparq-music beat/bar triggers through the event wire)"
                .to_string(),
        )
    } else {
        let d = demo_patch(&registry, cfg).map_err(|e| format!("patch build failed: {e}"))?;
        (
            d.executor,
            d.gain,
            Some(d.rms),
            None,
            "sine(440 Hz, 0.5) → gain(0.5) → master · rms analysis tap".to_string(),
        )
    };
    println!(
        "  patch       {description} · {} node(s), {} edge(s)",
        executor.order().len(),
        executor.graph().edge_count()
    );
    // ADR-009 decision 4: the memory budget is printed at patch load.
    println!(
        "  budget      {} bytes of audio buffers, all pre-allocated",
        executor.memory_budget_bytes()
    );

    // ---- 3. render.
    let frames = opts.block;
    let total_frames =
        ((opts.seconds * f64::from(opts.rate)).ceil() as usize / frames).max(1) * frames;
    let blocks = total_frames / frames;
    let mut samples: Vec<f32> = Vec::with_capacity(total_frames * cfg.device_channels);
    let mut out = vec![0.0f32; frames * cfg.device_channels];
    let mut block_events = BlockEvents::new();
    let mut kicks = 0u64;
    for _b in 0..blocks {
        if let Some(tr) = &mut transport {
            // The transport computes; the executor carries. Tick at the block's first frame,
            // then the block's events, then the render — the driver order WO-009 specifies.
            let tick0 = tr.abs_tick().round().max(0.0) as u64;
            executor.set_musical_position(Some((tick0, 960)));
            tr.advance_block(opts.block, &mut block_events);
            if let Some(mem) = drum_node {
                for e in block_events.as_slice() {
                    executor
                        .push_host_event(mem, 0, *e)
                        .map_err(|err| format!("trigger injection failed: {err}"))?;
                    if e.channel == 0 {
                        kicks += 1;
                    }
                }
            }
        }
        executor.render_block(master, &mut out).map_err(|e| format!("render failed: {e}"))?;
        samples.extend_from_slice(&out);
    }

    // ---- 4. evidence: the analysis tap proves analysis-as-control-source, the hash is the
    // golden identity of this render (tests/modules_golden.rs asserts the demo's; tests/
    // contract_v1.rs asserts the mod-demo's). Contract v1: the tap VALUE is read from the rms
    // node's declared block-rate cv port — the v0 convention of riding in an undeclared audio
    // output[0] is gone; the master's meter is the audio one. rms.level is manifest port 1.
    let tap = rms_node.and_then(|n| executor.node_cv_block(n, 1));
    let master_m = executor.meter(master);
    if rms_node.is_some() {
        println!(
            "  meters      rms tap value {:?} (0.25 amp sine, 0.59-cycle window → ≈ 0.16..0.19) · master peak {:?} rms {:?}",
            tap,
            master_m.map(|m| m.peak),
            master_m.map(|m| m.rms)
        );
    } else {
        // The drum demo has no analysis tap; the meters are the LAST BLOCK's (a decaying hit's
        // tail reads small — the render hash and the WAV are the evidence, not this line).
        println!(
            "  meters      master (last block) peak {:?} rms {:?} · {} beat trigger(s) from the transport",
            master_m.map(|m| m.peak),
            master_m.map(|m| m.rms),
            kicks
        );
    }
    if opts.patch == "mod-demo" {
        if let Some(v) = tap {
            // The live cv wire, in numbers: the cutoff the filter ran at in the final block, by
            // the formula the module documents (cutoff · 2^(2·mod·cv)). The arithmetic PROOF
            // that the wire carries exactly this lives in tests/contract_v1.rs (a hand-driven
            // reference filter renders bit-identical); this line is the operator's evidence.
            let eff = 200.0f64 * 2.0f64.powf(2.0 * f64::from(v));
            println!(
                "  cv wire     rms.level → svf.cutoff-mod LIVE: cv {v:.6} → effective cutoff {eff:.2} Hz (from 200 Hz, mod 1.0)"
            );
        }
    }
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
