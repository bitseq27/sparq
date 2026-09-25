//! `sparq demo` — render the Phase B patch to WAV.
//!
//! This is the RDP-friendly path described in `WINDOWS.md`: it needs no audio device, it is
//! deterministic, and the resulting WAV can be auditioned anywhere.

use std::process::ExitCode;
use std::time::Instant;

use sparq_audio::dna::DnaTree;
use sparq_audio::hash::{fnv1a64_f32, hex64};
use sparq_audio::mutation::preset_chains;
use sparq_audio::patch::{render_demo, DemoConfig};
use sparq_audio::presets::{chain_label, mutation_names, resolve_chain, Style};
use sparq_audio::wav::{write_wav, SampleFormat};

use crate::cli::DemoOpts;

pub fn run(o: DemoOpts) -> Result<ExitCode, String> {
    if o.list_patterns {
        return list_patterns();
    }
    let format = match o.format.as_str() {
        "f32" | "float" | "float32" => SampleFormat::Float32,
        "pcm16" | "16" => SampleFormat::Pcm16,
        "pcm24" | "24" => SampleFormat::Pcm24,
        other => return Err(format!("unknown --format `{other}` (f32|pcm16|pcm24)")),
    };

    let style = Style::parse(&o.style).ok_or_else(|| {
        format!(
            "unknown --pattern `{}`. Available: {}",
            o.style,
            Style::all().iter().map(Style::name).collect::<Vec<_>>().join(", ")
        )
    })?;
    let mutation = resolve_chain(&o.mutation).ok_or_else(|| {
        format!(
            "unknown --mutation `{}`. Available: none, {}",
            o.mutation,
            mutation_names().join(", ")
        )
    })?;

    let cfg = DemoConfig {
        bpm: o.bpm,
        bars: o.bars,
        sample_rate: o.sample_rate,
        channels: o.channels,
        seed: o.seed,
        gain_db: o.gain_db,
        delay_send: o.delay_send as f32,
        hat_crush_bits: o.crush_bits,
        style,
        mutation: mutation.clone(),
        transpose: o.transpose,
        ..DemoConfig::default()
    };

    if o.show_dna {
        print_dna(&cfg, &mutation);
    }

    let started = Instant::now();
    let samples = render_demo(&cfg);
    let wall = started.elapsed();

    write_wav(&o.out, cfg.sample_rate, cfg.channels as u16, format, &samples)
        .map_err(|e| format!("writing {}: {e}", o.out))?;

    let frames = samples.len() / cfg.channels.max(1);
    let seconds = frames as f64 / f64::from(cfg.sample_rate);
    let peak = samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    let rms = (samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum::<f64>()
        / samples.len().max(1) as f64)
        .sqrt();

    println!("sparq demo · Phase C1 patch · build {}", crate::stamp::line());
    println!("  out        {}", o.out);
    println!("  stream     {} Hz · {} ch · {:?}", cfg.sample_rate, cfg.channels, format);
    println!(
        "  music      {:.2} bpm · {} bars of 4/4 · 16 steps/bar · {:.3} s",
        cfg.bpm, cfg.bars, seconds
    );
    println!("  pattern    {} — {}", style.name(), style.description());
    println!("  mutation   {} · seed 0x{:X}", chain_label(&mutation), cfg.seed);
    if cfg.transpose != 0 {
        println!("  transpose  {:+} semitones", cfg.transpose);
    }
    println!("  seed       0 = the preset exactly; anything else mutates each lane from its own");
    println!("             branch of the seed tree, so re-seeding the hat cannot change the kick");
    println!("  topology   kick: sine+pitch-env, noise burst -> svf(lp) -> env/ad");
    println!("             bass: additive saw (48 partials) -> svf(lp, env cutoff) -> env/adsr");
    println!(
        "             hat : brown noise -> svf(hp {}) -> bitcrush({} bit) -> env/ad",
        7200, cfg.hat_crush_bits
    );
    println!(
        "             bus : gain {:+.1} dB, delay send {:.2} (dotted 8th, fb 0.34, damp 0.55)",
        cfg.gain_db, cfg.delay_send
    );
    println!("  level      peak {:.1} dB · rms {:.1} dB", db(peak), db(rms as f32));
    println!(
        "  timing     rendered in {:.0} ms ({:.0}× realtime)",
        wall.as_secs_f64() * 1e3,
        seconds * 1e3 / wall.as_secs_f64().max(1e-9)
    );
    println!("  hash       {}", hex64(fnv1a64_f32(&samples)));
    println!();
    println!("  Listen to {}. Same seed + same settings = bit-identical file, so you can", o.out);
    println!("  A/B a change by hash. Things worth trying:");
    println!(
        "    --pattern breakcore --mutation grid-break --seed 7   (a different loop every seed)"
    );
    println!("    --pattern glitch --mutation chaos --seed 3 --show-dna");
    println!("    --mutation densify --crush-bits 6 --delay-send 0.5");
    println!("    --list-patterns                                      (every style and mutation)");
    println!("  --seed 0 means NO mutation: the preset exactly, which is the golden reference.");
    println!("  This render used no audio device: it is the path that works over RDP.");

    if o.golden_values {
        // Machine-readable lines for reference/golden/phaseb-demo/manifest.txt. Regeneration is
        // explicit and reviewed (ADR-007): never regenerate just to make a test pass.
        println!();
        println!("hash={}", hex64(fnv1a64_f32(&samples)));
        println!("rms={:.12}", rms);
        println!("peak={:.9}", peak);
        println!("frames={frames}");
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

/// Print every style and every mutation preset.
fn list_patterns() -> Result<ExitCode, String> {
    println!("sparq patterns");
    println!();
    println!("  --pattern NAME      percussion lanes + bass line");
    for style in Style::all() {
        let set = style.percussion(960);
        let kick = set.get("kick").map_or_else(|| String::from("?"), |p| p.to_text());
        let hat = set.get("hat").map_or_else(|| String::from("?"), |p| p.to_text());
        let cycle = set.cycle_steps();
        println!("    {:<11} {}", style.name(), style.description());
        println!("        kick {:<18} hat {:<18} cycle {} steps", kick, hat, cycle);
    }
    println!();
    println!("  --mutation NAME     operator chain applied to every percussion lane");
    for (name, ops) in preset_chains() {
        println!("    {:<16} {}", name, chain_label(&ops));
    }
    println!("    {:<16} the preset exactly (also what --seed 0 does)", "none");
    println!();
    println!("  Each lane gets its own branch of the seed tree, so re-seeding the hat never");
    println!("  changes the kick. --show-dna prints the lineage that reproduces a render.");
    Ok(ExitCode::SUCCESS)
}

/// Build the DNA tree for this render and print it.
///
/// The tree is the navigable form of "how did I get this rhythm": every node records the operator,
/// the seed branch and a fingerprint of the pattern *at that point*, so a leaf can be reproduced
/// from the printed score alone (plan §10.5's "export a lineage as a generative score").
///
/// The chain is walked step by step rather than applied in one go, so each node's fingerprint is the
/// real intermediate state. An earlier version recorded the root's text at every node and only the
/// final one correctly, which made the tree look like nothing happened until the last step.
fn print_dna(cfg: &DemoConfig, mutation: &[sparq_audio::mutation::Op]) {
    use sparq_audio::mutation::apply_chain;
    use sparq_kernel::seed::SeedTree;

    let base = cfg.style.percussion(960);
    let fingerprint = |set: &sparq_audio::patterns::PatternSet| {
        set.lanes.iter().map(|(n, p)| format!("{n}={}", p.to_text())).collect::<Vec<_>>().join(" ")
    };

    let tree_seed = if cfg.seed == 0 { 0xA17E } else { cfg.seed };
    let tree = SeedTree::new(tree_seed);
    let mut dna = DnaTree::new();
    let root = dna.root(&fingerprint(&base), tree_seed);

    let mut parent = root;
    let mut current = base.clone();
    for (depth, op) in mutation.iter().enumerate() {
        // One branch per operator per depth, matching how `presets::variation` derives lane seeds:
        // each lane gets its own stream, so an operator applied to two lanes uses two seeds. Here we
        // record the chain itself, which is lane-independent, so one seed per step is the honest
        // representation of the score.
        let op_seed = tree.child("chain").child(depth.to_string()).seed();
        let mut rng = sparq_kernel::seed::SplitMix64::new(op_seed);
        let mut next = sparq_audio::patterns::PatternSet::new();
        for (name, pat) in &current.lanes {
            next.set(name, sparq_audio::mutation::apply(*op, pat, &mut rng));
        }
        current = next;
        parent = dna.record(parent, op.label(), op_seed, &fingerprint(&current)).unwrap_or(parent);
    }

    // The final node records what the render actually used, including the per-lane seeds.
    let (set, log) = sparq_audio::presets::variation(cfg.style, 960, mutation, cfg.seed);
    let last =
        dna.record(parent, "<rendered>".into(), cfg.seed, &fingerprint(&set)).unwrap_or(parent);
    dna.set_kept(last, true);

    println!();
    println!("  ---- dna ----");
    for line in dna.render_text().lines() {
        println!("  {line}");
    }
    println!();
    println!("    per-lane results (each lane has its own seed branch):");
    for entry in &log {
        println!("      {entry}");
    }
    println!();
    println!("  ---- generative score (reproduces this render) ----");
    for line in dna.export_score(last).lines() {
        println!("  {line}");
    }
    let _ = apply_chain;
}
