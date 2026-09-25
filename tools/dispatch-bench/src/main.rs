//! WO-007 task 2 / module-api-v1 §16 Q1 — dispatch cost, measured.
//!
//! The question: trait objects (`Box<dyn Module>`) vs a generated enum
//! (`enum Mod { Gain(Gain), Null(Null), ... }` with a `match`), for the call that happens
//! most often in the whole system. PHASE0-WORKORDERS WO-007 task 2: "Define the Rust traits
//! (or trait + enum-dispatch hybrid — measure the dispatch cost; per-sample virtual calls
//! matter)."
//!
//! The acceptance number this informs: "a graph of 100 null modules processes a 64-sample
//! block in < 20 µs (indicative target — measure and record the real number)".
//!
//! WHAT IS MEASURED. Four strategies × two granularities:
//!   block   — one dispatch per module per block (what the executor actually does)
//!   sample  — one dispatch per module per sample (the worst case the work order warns about)
//!   mono    — no dispatch at all: one statically known type, fully monomorphised (the floor)
//!
//! The module body is identical in every variant — `out[i] = in[i] * gain` over 64 samples —
//! so the difference between rows is dispatch and nothing else.
//!
//! HONESTY. This is a 2-core virtualised sandbox, not the stage device, and not Windows.
//! The *ratio* between strategies is the transferable result; the absolute microseconds are
//! not. Re-run on SATURN before the ADR-005 addendum is ratified.

// `Instant::now` is denied workspace-wide (clippy.toml) because the audio path must never read the
// wall clock. This binary is the *harness that measures* dispatch cost, not the audio path: the code
// under measurement receives only buffers and a gain, and cannot observe the clock. Same pattern and
// same justification as `crates/sparq-kernel/src/device/null.rs`.
#![allow(clippy::disallowed_methods)]

use std::hint::black_box;
use std::time::Instant;

const N_MOD: usize = 100; // the acceptance criterion's graph size
const BLOCK: usize = 64; // sparq's block size
const ITERS: usize = 500; // timed repetitions per sample
const SAMPLES: usize = 121; // odd, so the median is a real observation

// ---------------------------------------------------------------- the work itself
// Identical body everywhere. `#[inline(always)]` so the only difference between the
// variants below is how the call is dispatched, never what is called.
#[inline(always)]
fn gain_block(inp: &[f32; BLOCK], out: &mut [f32; BLOCK], g: f32) {
    for i in 0..BLOCK {
        out[i] = inp[i] * g;
    }
}

#[inline(always)]
fn gain_sample(x: f32, g: f32) -> f32 {
    x * g
}

// ---------------------------------------------------------------- variant A: trait object
trait Module {
    fn process_block(&mut self, inp: &[f32; BLOCK], out: &mut [f32; BLOCK]);
    fn process_sample(&mut self, x: f32) -> f32;
    fn name(&self) -> &'static str;
}

// Four distinct implementors, so the vtable pointer actually varies at runtime the way it
// would in a real graph. A single type would let the optimiser speculate; four is honest.
macro_rules! gain_module {
    ($t:ident, $label:literal) => {
        struct $t {
            g: f32,
        }
        impl Module for $t {
            #[inline]
            fn process_block(&mut self, inp: &[f32; BLOCK], out: &mut [f32; BLOCK]) {
                gain_block(inp, out, self.g);
            }
            #[inline]
            fn process_sample(&mut self, x: f32) -> f32 {
                gain_sample(x, self.g)
            }
            fn name(&self) -> &'static str {
                $label
            }
        }
    };
}
gain_module!(ModA, "util/gain-a");
gain_module!(ModB, "util/gain-b");
gain_module!(ModC, "util/gain-c");
gain_module!(ModD, "util/gain-d");

// ---------------------------------------------------------------- variant B: generated enum
enum Mod {
    A(ModA),
    B(ModB),
    C(ModC),
    D(ModD),
}

impl Mod {
    #[inline]
    fn process_block(&mut self, inp: &[f32; BLOCK], out: &mut [f32; BLOCK]) {
        match self {
            Mod::A(m) => m.process_block(inp, out),
            Mod::B(m) => m.process_block(inp, out),
            Mod::C(m) => m.process_block(inp, out),
            Mod::D(m) => m.process_block(inp, out),
        }
    }
    #[inline]
    fn process_sample(&mut self, x: f32) -> f32 {
        match self {
            Mod::A(m) => m.process_sample(x),
            Mod::B(m) => m.process_sample(x),
            Mod::C(m) => m.process_sample(x),
            Mod::D(m) => m.process_sample(x),
        }
    }
}

// ---------------------------------------------------------------- the graph fixture
struct Graph {
    dyn_mods: Vec<Box<dyn Module>>,
    enum_mods: Vec<Mod>,
    // One input and one output buffer per module, as a real graph has one buffer per edge.
    // 100 x 2 x 64 x 4 B = 50 kB, so the fixture is L2-resident and the measurement is not
    // dominated by memory traffic that a real graph would not have either.
    ins: Vec<[f32; BLOCK]>,
    outs: Vec<[f32; BLOCK]>,
}

impl Graph {
    fn new() -> Self {
        let mut dyn_mods: Vec<Box<dyn Module>> = Vec::with_capacity(N_MOD);
        let mut enum_mods: Vec<Mod> = Vec::with_capacity(N_MOD);
        for i in 0..N_MOD {
            let g = 0.5 + (i as f32) * 0.001;
            let (d, e): (Box<dyn Module>, Mod) = match i % 4 {
                0 => (Box::new(ModA { g }), Mod::A(ModA { g })),
                1 => (Box::new(ModB { g }), Mod::B(ModB { g })),
                2 => (Box::new(ModC { g }), Mod::C(ModC { g })),
                _ => (Box::new(ModD { g }), Mod::D(ModD { g })),
            };
            dyn_mods.push(d);
            enum_mods.push(e);
        }
        let ins = (0..N_MOD).map(|i| [i as f32 * 0.001; BLOCK]).collect();
        let outs = vec![[0.0f32; BLOCK]; N_MOD];
        Graph {
            dyn_mods,
            enum_mods,
            ins,
            outs,
        }
    }
}

// ---------------------------------------------------------------- timing harness
fn pct(mut v: Vec<f64>, p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[idx]
}

/// Returns (median, p99, min) in nanoseconds for one full pass over the graph.
fn time_pass<F: FnMut(&mut Graph)>(label: &str, mut f: F) -> (f64, f64, f64) {
    let mut g = Graph::new();
    for _ in 0..20 {
        f(&mut g); // warm: page in, fill branch predictors, let the CPU settle
    }
    let mut ns: Vec<f64> = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let t = Instant::now();
        for _ in 0..ITERS {
            f(&mut g);
            black_box(&g.outs);
        }
        ns.push(t.elapsed().as_nanos() as f64 / ITERS as f64);
    }
    let (med, p99, min) = (pct(ns.clone(), 0.5), pct(ns.clone(), 0.99), pct(ns, 0.0));
    println!(
        "  {:<34} median {:>9.0} ns   p99 {:>9.0} ns   min {:>9.0} ns",
        label, med, p99, min
    );
    (med, p99, min)
}

fn main() {
    println!(
        "WO-007 dispatch benchmark — {} modules x {}-sample block, release profile (opt-level 3, thin LTO, cg-units 1)",
        N_MOD, BLOCK
    );
    let probe = Graph::new();
    let ids: Vec<&str> = (0..4).map(|i| probe.dyn_mods[i].name()).collect();
    drop(probe);
    println!(
        "  fixture: {} modules over {} distinct implementors ({}) — the vtable pointer varies at runtime, as it would in a real graph",
        N_MOD, 4, ids.join(", ")
    );
    println!(
        "host: {} cores visible, {} — NOT the stage device; ratios transfer, absolutes do not\n",
        std::thread::available_parallelism()
            .map(|n| n.to_string())
            .unwrap_or_else(|_| "?".into()),
        std::env::consts::OS
    );

    println!("A. ONE DISPATCH PER MODULE PER BLOCK (what the executor does)");
    let dyn_blk = time_pass("A1  Box<dyn Module>  (trait object)", |g| {
        for i in 0..N_MOD {
            let inp = g.ins[i];
            g.dyn_mods[i].process_block(&inp, &mut g.outs[i]);
        }
    });
    let enum_blk = time_pass("A2  enum + match  (generated)", |g| {
        for i in 0..N_MOD {
            let inp = g.ins[i];
            g.enum_mods[i].process_block(&inp, &mut g.outs[i]);
        }
    });
    let mono_blk = time_pass("A3  monomorphised  (no dispatch)", |g| {
        let mut m = ModA { g: 0.75 };
        for i in 0..N_MOD {
            let inp = g.ins[i];
            m.process_block(&inp, &mut g.outs[i]);
        }
    });

    println!("\nB. ONE DISPATCH PER MODULE PER SAMPLE (the case the work order warns about)");
    let n_calls = (N_MOD * BLOCK) as f64;
    let dyn_smp = time_pass("B1  Box<dyn Module>  per sample", |g| {
        for i in 0..N_MOD {
            let mut acc = 0.0f32;
            for s in 0..BLOCK {
                acc += g.dyn_mods[i].process_sample(g.ins[i][s]);
            }
            g.outs[i][0] = acc;
        }
    });
    let enum_smp = time_pass("B2  enum + match  per sample", |g| {
        for i in 0..N_MOD {
            let mut acc = 0.0f32;
            for s in 0..BLOCK {
                acc += g.enum_mods[i].process_sample(g.ins[i][s]);
            }
            g.outs[i][0] = acc;
        }
    });
    let mono_smp = time_pass("B3  monomorphised  per sample", |g| {
        let mut m = ModA { g: 0.75 };
        for i in 0..N_MOD {
            let mut acc = 0.0f32;
            for s in 0..BLOCK {
                acc += m.process_sample(g.ins[i][s]);
            }
            g.outs[i][0] = acc;
        }
    });

    println!("\nC. VERDICT AGAINST THE ACCEPTANCE TARGET (< 20 us per block of 100 modules)");
    for (label, r) in [
        ("Box<dyn Module>, block dispatch", dyn_blk),
        ("enum + match, block dispatch", enum_blk),
        ("monomorphised, block dispatch", mono_blk),
    ] {
        let us = r.0 / 1000.0;
        let headroom = 20.0 / us;
        println!(
            "  {:<34} {:>7.3} us/block   {:>6.1}x headroom vs the 20 us target",
            label, us, headroom
        );
    }
    println!("\nD. PER-CALL COST (the number that decides Q1)");
    println!(
        "  block dispatch: dyn {:.1} ns/call · enum {:.1} ns/call · mono {:.1} ns/call  (dyn/enum = {:.2}x)",
        dyn_blk.0 / N_MOD as f64,
        enum_blk.0 / N_MOD as f64,
        mono_blk.0 / N_MOD as f64,
        dyn_blk.0 / enum_blk.0
    );
    println!(
        "  sample dispatch: dyn {:.2} ns/call · enum {:.2} ns/call · mono {:.2} ns/call  (dyn/enum = {:.2}x)",
        dyn_smp.0 / n_calls,
        enum_smp.0 / n_calls,
        mono_smp.0 / n_calls,
        dyn_smp.0 / enum_smp.0
    );
    println!(
        "\n  full per-sample graph cost: dyn {:.3} us/block vs enum {:.3} us/block ({} calls)",
        dyn_smp.0 / 1000.0,
        enum_smp.0 / 1000.0,
        n_calls as usize
    );
}
