//! WO-008 increment 6 gates: `cv_interp = "spline"` — the last word of the G4 vocabulary, now
//! PERFORMED by the host instead of refused at build. The curve is the parabola through the last
//! three block values (`CvInterp::Spline`'s doc is the one compiled copy; the compat-matrix cell
//! carries the same sentence and `sparq-module-api/tests/compat_matrix.rs` pins table ⇔ code).
//!
//! The rules this file proves, in words — properties as arithmetic against hand-computed values,
//! the way contract_v1.rs and mixer_cv.rs established:
//!
//! * **the curve is the declared parabola** — a scripted dyadic knot sequence chosen so every
//!   coefficient is a small binary rational and every frame is f32-EXACT: every frame of every
//!   block compared `==` against the independently simplified polynomials, including the
//!   clamped overshoot frames and the momentum dip;
//! * **constants stay constant** — once the history is full, a flat wire is flat bit-exactly
//!   (the Lagrange coefficients sum to 1);
//! * **collinear knots degenerate to the exact line** — a block-rate ramp renders
//!   bit-identically to the same wire declaring `linear` (the parabola through three points on
//!   a line IS the line);
//! * **quadratic sweeps are reproduced exactly** — a source publishing `v[N] = N²/64` arrives at
//!   every fractional frame position as the same parabola, bit-exact from the third block on,
//!   with the startup transient named rather than hidden;
//! * **C⁰ at the knots** — every block's frame 0 is the previous knot bit-exactly, over a
//!   deterministic pseudo-random sequence: the arrival contract `linear`'s doc sentence pins,
//!   kept by `spline` (declaring the curve never moves the timing);
//! * **startup is the declared zero history, and deterministic** — both history slots start at
//!   0.0 (the same convention `linear` has always had), the first two blocks match the
//!   hand-computed ramp-from-zero, and a rebuild renders byte-identical (ADR-007 at wire scale);
//! * **overshoot clamps to the declared range on BOTH polarities** — exactly at the bound, with
//!   the un-clamped interior frames of the same curve keeping their hand-computed values (the
//!   clamp bounds; it never reshapes);
//! * **zero allocations** — the spline wire over 1 000 rendered blocks, counted, not commented.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_kernel::alloc::{allocation_count, start_counting, stop_counting, CountingAllocator};
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::manifest::{
    Classification, Identity, Manifest, ParamSpec, PortSpec, ResourceDecl, StateDecl,
};
use sparq_module_api::module::{AudioCtx, BlockStatus, CvIn, Module, ModuleError, Resources};
use sparq_module_api::params::ParamSet;

#[global_allocator]
static ALLOC: CountingAllocator<std::alloc::System> = CountingAllocator::new(std::alloc::System);

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

fn cfg(ch: usize) -> ExecConfig {
    ExecConfig::new(RATE, FRAMES, ch)
}

fn measure<F: FnOnce()>(f: F) -> u64 {
    let base = start_counting();
    f();
    let made = allocation_count().saturating_sub(base);
    stop_counting();
    made
}

// ------------------------------------------------------------------ manifest helpers
// (the contract_v1.rs shapes — each test binary carries its own copy by design)

fn base(id: &str, ports: Vec<PortSpec>, params: Vec<ParamSpec>) -> Manifest {
    let mut m = Manifest {
        identity: Identity {
            id: Some(format!("sparq/test/{id}")),
            version: Some("0.1.0".into()),
            host_api_min: Some(1),
            host_api_max: Some(1),
            display_name: Some(id.into()),
            summary: Some("cv-spline test module".into()),
            authors: vec!["sparq".into()],
            license: Some("MIT".into()),
        },
        classification: Classification {
            category: Some(format!("utility/{id}")),
            top: Some("util".into()),
            kind: Some("processor".into()),
            tier: Some("t1".into()),
            stability: Some("experimental".into()),
        },
        state: StateDecl {
            schema_id: Some(format!("sparq/test/{id}/state")),
            schema_version: Some(1),
        },
        resources: ResourceDecl {
            latency_samples: Some(0),
            cpu_class: Some("trivial".into()),
            ..ResourceDecl::default()
        },
        voices_policy: Some("none".into()),
        ..Manifest::default()
    };
    m.ports = ports;
    m.params = params;
    m
}

fn cv_port(id: &str, dir: &str, rate: &str, range: &str) -> PortSpec {
    PortSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        direction: Some(dir.into()),
        port_type: Some("cv".into()),
        rate: Some(rate.into()),
        range: Some(range.into()),
        required: Some(false),
        ..PortSpec::default()
    }
}

fn audio_port(id: &str, dir: &str, set: &str) -> PortSpec {
    PortSpec {
        id: Some(id.into()),
        name: Some(id.into()),
        direction: Some(dir.into()),
        port_type: Some("audio".into()),
        channel_set: Some(set.into()),
        required: Some(false),
        ..PortSpec::default()
    }
}

fn validated(m: Manifest) -> sparq_module_api::ValidatedManifest {
    m.validate().unwrap_or_else(|r| panic!("{r}"))
}

// ------------------------------------------------------------------ test modules

/// Publishes a scripted sequence of block values, one per block, holding the last past the end.
/// The knots every gate below hand-computes against; the internal counter is test-local state,
/// never engine state.
struct CvSeq {
    seq: Vec<f32>,
    block: usize,
}
impl Module for CvSeq {
    fn id(&self) -> &str {
        "sparq/test/cv-seq"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let v = self.seq[self.block.min(self.seq.len() - 1)];
        self.block += 1;
        if let Some(mut cv) = ctx.cv_out(0) {
            cv.set(v);
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("cv-seq takes no messages"))
    }
}

/// Renders whatever its audio-rate cv input carries into a mono audio output, so a test can READ
/// the expanded wire frame by frame (the contract_v1 probe's discipline, markers included: an
/// unwritten buffer must not read as a value).
struct CvProbe;
impl Module for CvProbe {
    fn id(&self) -> &str {
        "sparq/test/cv-probe"
    }
    fn configure(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Ok(())
    }
    fn prepare(&mut self, _: &Resources) -> Result<(), ModuleError> {
        Ok(())
    }
    fn process(&mut self, ctx: &mut AudioCtx<'_>) -> BlockStatus {
        let cv = ctx.cv_in(0);
        let out = ctx.output();
        match cv {
            None => {
                for s in out.iter_mut() {
                    *s = -3.0;
                }
            },
            Some(CvIn::Unconnected) => {
                for s in out.iter_mut() {
                    *s = -2.0;
                }
            },
            Some(cv) => {
                if let Some(src) = cv.audio() {
                    let n = out.len().min(src.len());
                    out[..n].copy_from_slice(&src[..n]);
                } else {
                    let v = cv.at(0);
                    for s in out.iter_mut() {
                        *s = v;
                    }
                }
            },
        }
        BlockStatus::Ok
    }
    fn message(&mut self, _: &[u8]) -> Result<(), ModuleError> {
        Err(ModuleError::Message("cv-probe takes no messages"))
    }
}

fn seq_manifest(range: &str) -> Manifest {
    base("cv-seq", vec![cv_port("level", "out", "block", range)], vec![])
}

fn probe_manifest(interp: &str, range: &str) -> Manifest {
    let mut p = cv_port("mod", "in", "audio", range);
    p.cv_interp = Some(interp.to_string());
    base("cv-probe", vec![p, audio_port("out", "out", "mono")], vec![])
}

/// One seq → probe spline wire; returns the built executor and the probe node.
fn spline_rig(seq: Vec<f32>, range: &str) -> (Executor, NodeId) {
    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let ex = Executor::build(
        g,
        vec![
            (
                src,
                NodeBuild {
                    module: Box::new(CvSeq { seq, block: 0 }),
                    manifest: validated(seq_manifest(range)),
                    params: ParamSet::new(1, &[]).unwrap(),
                },
            ),
            (
                probe,
                NodeBuild {
                    module: Box::new(CvProbe),
                    manifest: validated(probe_manifest("spline", range)),
                    params: ParamSet::new(1, &[]).unwrap(),
                },
            ),
        ],
        cfg(1),
    )
    .unwrap();
    (ex, probe)
}

fn render(ex: &mut Executor, master: NodeId, blocks: usize) -> Vec<f32> {
    let mut out = vec![0.0f32; FRAMES];
    let mut all = Vec::with_capacity(blocks * FRAMES);
    for _ in 0..blocks {
        ex.render_block(master, &mut out).unwrap();
        all.extend_from_slice(&out);
    }
    all
}

// ------------------------------------------------------------------ gates

#[test]
fn spline_is_the_hand_computed_parabola_over_a_scripted_sequence() {
    // Knots: 0.5, 1.0, 0.75, 0.75, 0.25 — every value dyadic, every resulting coefficient a
    // small binary rational, so both the host's f64 Lagrange form and the INDEPENDENTLY
    // simplified polynomials below are exact and `==` is the honest comparison. The sequence
    // visits all three curve shapes: the overshoot-and-clamp (block 2), the momentum dip
    // (block 3) and the plain descent (block 4), after two startup-transient blocks.
    let seq = [0.5f32, 1.0, 0.75, 0.75, 0.25];
    let (mut ex, probe) = spline_rig(seq.to_vec(), "unipolar");
    let out = render(&mut ex, probe, seq.len());

    // v(t) per block, simplified by hand from prev2·t(t−1)/2 + prev·(1−t²) + cur·t(t+1)/2:
    fn want(b: usize, t: f64) -> f64 {
        match b {
            0 => 0.25 * (t * t + t), // (0, 0, 0.5): the zero-history startup ramp
            1 => 0.5 + 0.5 * t,      // (0, 0.5, 1.0): collinear — the parabola IS the line
            2 => (1.0 + t / 8.0 - 3.0 * t * t / 8.0).min(1.0), // (0.5, 1.0, 0.75): overshoot, clamped
            3 => 0.75 + 0.125 * t * t - 0.125 * t, // (1.0, 0.75, 0.75): momentum dips below both knots
            4 => 0.75 - 0.25 * t - 0.25 * t * t,   // (0.75, 0.75, 0.25): plain descent
            _ => 0.0, // unreachable: exactly seq.len() blocks are rendered
        }
    }
    for b in 0..seq.len() {
        for i in 0..FRAMES {
            let t = i as f64 / FRAMES as f64;
            assert_eq!(out[b * FRAMES + i], want(b, t) as f32, "block {b} frame {i}");
        }
    }
}

#[test]
fn a_constant_signal_expands_bit_exact_flat_once_history_is_full() {
    // The identity: with prev2 == prev == cur the coefficients sum to 1 exactly (dyadic
    // arithmetic), so a flat wire is flat bit-for-bit — no ripple, no drift, no rounding walk.
    let (mut ex, probe) = spline_rig(vec![0.5], "unipolar");
    let out = render(&mut ex, probe, 4);
    // Blocks 0–1 are the declared startup transient (gate `startup_is_...` pins them); from
    // block 2 the history is full of the constant.
    for b in 2..4 {
        for i in 0..FRAMES {
            assert_eq!(out[b * FRAMES + i], 0.5, "block {b} frame {i} of a constant wire");
        }
    }
}

#[test]
fn for_a_block_linear_ramp_spline_and_linear_wires_agree_bit_for_bit() {
    // Three collinear knots: the unique parabola through them IS the line, so a ramp renders
    // identically under both declarations — bit for bit, not approximately. This is the
    // degeneracy pin: `spline` never adds wobble where `linear` was already exact.
    let seq = [0.0f32, 0.25, 0.5, 0.75, 1.0];
    let (mut ex_s, probe_s) = spline_rig(seq.to_vec(), "unipolar");
    let spline = render(&mut ex_s, probe_s, seq.len());

    let mut g = Graph::new();
    let src = g.add_node(0);
    let probe = g.add_node(0);
    g.connect(PortRef::new(src, 0), PortRef::new(probe, 0), EdgeKind::Plain).unwrap();
    let mut ex_l = Executor::build(
        g,
        vec![
            (
                src,
                NodeBuild {
                    module: Box::new(CvSeq { seq: seq.to_vec(), block: 0 }),
                    manifest: validated(seq_manifest("unipolar")),
                    params: ParamSet::new(1, &[]).unwrap(),
                },
            ),
            (
                probe,
                NodeBuild {
                    module: Box::new(CvProbe),
                    manifest: validated(probe_manifest("linear", "unipolar")),
                    params: ParamSet::new(1, &[]).unwrap(),
                },
            ),
        ],
        cfg(1),
    )
    .unwrap();
    let linear = render(&mut ex_l, probe, seq.len());

    // Blocks 0–1 are the startup transient, where the zero history is not yet on the ramp's
    // line (pinned separately); from block 2 both wires are the same exact line.
    for b in 2..seq.len() {
        for i in 0..FRAMES {
            assert_eq!(
                spline[b * FRAMES + i],
                linear[b * FRAMES + i],
                "block {b} frame {i}: the parabola through collinear knots is the line"
            );
        }
    }
    // And one hand-computed anchor so "equal" cannot mean "equally wrong": block 2 ramps
    // 0.25 → 0.5, so frame 32 is 0.375 exactly.
    assert_eq!(spline[2 * FRAMES + 32], 0.375, "the shared line, hand-computed");
}

#[test]
fn a_quadratic_sweep_is_reproduced_exactly_at_fractional_frames() {
    // The accuracy claim, measured: a source publishing v[N] = N²/64 IS a parabola in block
    // index, so once the three-knot history is full the expansion must reproduce it at every
    // fractional frame position — bit-exactly (all values dyadic with small numerators).
    // `linear` on the same source would staircase; this is what declaring `spline` buys.
    let seq: Vec<f32> = (0..7).map(|n: u32| (n * n) as f32 / 64.0).collect();
    let (mut ex, probe) = spline_rig(seq.clone(), "unipolar");
    let out = render(&mut ex, probe, seq.len());
    for b in 2..seq.len() {
        for i in 0..FRAMES {
            let t = i as f64 / FRAMES as f64;
            let x = (b - 1) as f64 + t; // the frame's position in block index
            assert_eq!(out[b * FRAMES + i], (x * x / 64.0) as f32, "block {b} frame {i}");
        }
    }
}

#[test]
fn every_block_starts_at_the_previous_knot_bit_exact() {
    // The arrival contract, over a deterministic pseudo-random dyadic sequence (LCG, seed
    // 0xA17E — the house seed): v(0) = prev EXACTLY for any knot values, so the wire is C⁰ at
    // every knot and `spline` never shifts the timing `linear`'s doc sentence pins.
    let mut s: u64 = 0xA17E;
    let mut seq = Vec::new();
    for _ in 0..8 {
        s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        seq.push(((s >> 40) as f32) / 16_777_216.0); // 24-bit numerator: dyadic, in 0..1
    }
    let (mut ex, probe) = spline_rig(seq.clone(), "unipolar");
    let out = render(&mut ex, probe, seq.len());
    for b in 1..seq.len() {
        assert_eq!(out[b * FRAMES], seq[b - 1], "block {b} starts at knot {} exactly", b - 1);
    }
}

#[test]
fn startup_is_the_declared_zero_history_and_a_rebuild_is_identical() {
    // Both history slots start at 0.0 — the same declared convention `linear` has always had
    // ("block 0 ramps from the initial 0.0" is contract_v1's pin). Block 0 is the parabola
    // through (0, 0, 0.5) = 0.25·(t²+t); block 1 through (0, 0.5, 0.5) = 0.5 + 0.25t − 0.25t².
    let (mut ex, probe) = spline_rig(vec![0.5], "unipolar");
    let out = render(&mut ex, probe, 2);
    assert_eq!(out[0], 0.0, "block 0 frame 0: the zero history, not the knot");
    assert_eq!(out[32], 0.1875, "block 0 frame 32: 0.25·(0.25+0.5), hand-computed");
    assert_eq!(out[63], 8001.0 / 16384.0, "block 0 frame 63: 0.25·(t²+t) at t = 63/64");
    assert_eq!(out[FRAMES], 0.5, "block 1 frame 0 arrives at block 0's knot, exactly");
    assert_eq!(out[FRAMES + 32], 0.5625, "block 1 frame 32: 0.5 + 0.125 − 0.0625");
    assert_eq!(out[FRAMES + 63], 8255.0 / 16384.0, "block 1 frame 63, hand-computed");
    // Determinism (ADR-007 at wire scale): the same graph, rebuilt, renders byte-identical.
    let (mut ex2, probe2) = spline_rig(vec![0.5], "unipolar");
    let again = render(&mut ex2, probe2, 2);
    assert_eq!(out, again, "a rebuild renders the identical bytes");
}

#[test]
fn overshoot_clamps_to_the_declared_range_on_both_polarities() {
    // Unipolar: knots (0.5, 1.0, 0.75) give v(t) = 1 + t/8 − 3t²/8, above the bound for every
    // t < 1/3 — frames 1..=21 clamp to EXACTLY 1.0, frame 22 is the polynomial again
    // (8181/8192, hand-computed), and the interior is untouched: the clamp bounds the wire, it
    // never reshapes a curve that already fits.
    let (mut ex, probe) = spline_rig(vec![0.5, 1.0, 0.75], "unipolar");
    let out = render(&mut ex, probe, 3);
    let b2 = &out[2 * FRAMES..3 * FRAMES];
    assert_eq!(b2[0], 1.0, "frame 0 is the knot itself");
    for (i, &v) in b2[1..=21].iter().enumerate() {
        assert_eq!(v, 1.0, "frame {}: the host-made overshoot stops at the declared bound", i + 1);
    }
    assert_eq!(b2[22], 8181.0 / 8192.0, "frame 22: the polynomial, exact, un-clamped");
    assert!(b2[1..].iter().all(|&v| v <= 1.0), "no frame ever exceeds the declared range");
    // Bipolar mirror: knots (−0.5, −1.0, −0.75) give v(t) = −(1 + t/8 − 3t²/8), below −1 for
    // t < 1/3 — frames 1..=21 clamp to EXACTLY −1.0, frame 22 is −8181/8192 (the same
    // arithmetic as the unipolar case, at the lower bound: the discipline is the declared
    // range, not a direction).
    let (mut ex, probe) = spline_rig(vec![-0.5, -1.0, -0.75], "bipolar");
    let out = render(&mut ex, probe, 3);
    let b2 = &out[2 * FRAMES..3 * FRAMES];
    for (i, &v) in b2[1..=21].iter().enumerate() {
        assert_eq!(v, -1.0, "frame {}: the lower bound clamps exactly", i + 1);
    }
    assert_eq!(b2[22], -8181.0 / 8192.0, "frame 22: the polynomial again, exact");
    assert!(b2.iter().all(|&v| v >= -1.0), "no frame ever under-runs the declared range");
}

#[test]
fn spline_expansion_over_1000_blocks_allocates_nothing() {
    // The audio path allocates nothing — the widened expand and the two-slot history are plain
    // f32/f64 arithmetic over build-allocated buffers, measured under the counting allocator.
    let (mut ex, probe) = spline_rig(vec![0.5, 1.0, 0.75], "unipolar");
    let mut out = vec![0.0f32; FRAMES];
    let made = measure(|| {
        for _ in 0..1000 {
            ex.render_block(probe, &mut out).unwrap();
        }
    });
    assert_eq!(made, 0, "the spline wire allocated on the audio path: {made} times");
}
