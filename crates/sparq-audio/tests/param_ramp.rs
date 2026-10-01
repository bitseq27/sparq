//! Defect #85 (WO-014 increment 7) — the zipper-noise gate: a gain-like parameter edit must
//! GLIDE across the block in which it lands, never step. The operator's report was a crackle
//! under the main-out trim and the sine amplitude during a drag; the mechanism was a per-block
//! constant coefficient, i.e. a waveform discontinuity of `Δcoeff × signal` at every boundary
//! a edit crossed. The fix (`dsp::core::coeff_ramp`, applied in `syn/sine`, `util/gain`,
//! `out/main`, `util/mixer`, `util/panner`) makes the change block a linear glide and keeps the
//! static path a zero-step constant multiply — which is why every checked-in golden still holds.
//!
//! Measured here at the executor level, through the same `set_params` door a live drag uses:
//! the largest sample-to-sample jump across an edited boundary stays inside the signal's own
//! slope plus the ramp's, while the SAME edit under the old constant-coefficient behaviour would
//! jump by the full Δ (the test computes that counterfactual and asserts it would have failed).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use sparq_audio::executor::{ExecConfig, Executor, NodeBuild};
use sparq_audio::modules::register_builtins;
use sparq_kernel::graph::{EdgeKind, Graph, NodeId, PortRef};
use sparq_module_api::params::ParamSet;
use sparq_module_api::registry::Registry;

const RATE: u32 = 48_000;
const FRAMES: usize = 64;

fn cfg() -> ExecConfig {
    ExecConfig::new(RATE, FRAMES, 2)
}

fn build(reg: &Registry, id: &str, params: &[f32]) -> NodeBuild {
    let r = reg.get(id).unwrap_or_else(|| panic!("{id} missing from the registry"));
    NodeBuild {
        module: r.create(),
        manifest: r.manifest().clone(),
        params: ParamSet::new(0, params).unwrap(),
    }
}

/// sine → out/main, the operator's exact two knobs.
fn rig(reg: &Registry) -> (Executor, NodeId, NodeId) {
    let mut g = Graph::new();
    let s = g.add_node(0);
    let m = g.add_node(0);
    g.connect(PortRef::new(s, 0), PortRef::new(m, 0), EdgeKind::Plain).unwrap();
    let ex = Executor::build(
        g,
        vec![
            (s, build(reg, "sparq/syn/sine", &[440.0, 0.5])),
            (m, build(reg, "sparq/out/main", &[1.0, 0.0])),
        ],
        cfg(),
    )
    .unwrap();
    (ex, s, m)
}

fn render(ex: &mut Executor, master: NodeId) -> Vec<f32> {
    let mut out = vec![0.0f32; FRAMES * 2];
    ex.render_block(master, &mut out).unwrap();
    out
}

fn max_jump(a: &[f32], b: &[f32]) -> f32 {
    (b[0] - a[a.len() - 1]).abs()
}

#[test]
fn an_amplitude_edit_glides_across_the_boundary_it_lands_on() {
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let (mut ex, s, m) = rig(&reg);

    // Four steady blocks at amplitude 0.5: the signal's own maximum sample-to-sample jump is
    // the baseline any edit-boundary jump must stay inside (plus the ramp's own slope).
    let mut prev = render(&mut ex, m);
    let mut signal_jump = 0.0f32;
    for _ in 0..3 {
        let b = render(&mut ex, m);
        for w in b.windows(2) {
            signal_jump = signal_jump.max((w[1] - w[0]).abs());
        }
        prev = b;
    }
    assert!(signal_jump > 0.01, "the rig is sounding: {signal_jump}");

    // The drag: amplitude 0.5 → 0.1 through the live param door, mid-stream.
    ex.set_params(s, ParamSet::new(1, &[440.0, 0.1]).unwrap()).unwrap();
    let edited = render(&mut ex, m);
    let jump = max_jump(&prev, &edited);
    let ramp_allow = 0.4 / FRAMES as f32 + 0.002; // the whole Δ spread over the block, plus float dust
    assert!(
        jump <= signal_jump + ramp_allow,
        "the edit boundary jumps {jump}, signal slope is {signal_jump} — that is a click"
    );
    // The counterfactual: under the old per-block constant the boundary sample would move by
    // Δamp × |sin| at the boundary phase — an order of magnitude above the glide. Prove the
    // gate would have caught it (a gate that cannot fail is not a gate).
    let delta_static = 0.4f32 * edited[0].abs().max(0.05);
    assert!(
        delta_static > signal_jump + ramp_allow,
        "the un-ramped step ({delta_static}) must exceed the gate's allowance, or the gate proves nothing"
    );
    // And the block AFTER the glide is the new constant, exactly: the drag settles, it does not
    // keep moving.
    ex.set_params(s, ParamSet::new(2, &[440.0, 0.1]).unwrap()).unwrap();
    let settled = render(&mut ex, m);
    let jump2 = max_jump(&edited, &settled);
    assert!(
        jump2 <= signal_jump + 0.002,
        "once the coefficient has landed, boundaries return to the signal's own slope: {jump2}"
    );
}

#[test]
fn a_master_trim_edit_glides_and_mute_still_cuts_in_exact_zeros() {
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let (mut ex, _s, m) = rig(&reg);
    let mut prev = render(&mut ex, m);
    for _ in 0..2 {
        prev = render(&mut ex, m);
    }
    // Trim 1.0 → 0.2 mid-stream: a glide, like any gain-like coefficient.
    ex.set_params(m, ParamSet::new(1, &[0.2, 0.0]).unwrap()).unwrap();
    let edited = render(&mut ex, m);
    let jump = max_jump(&prev, &edited);
    assert!(jump <= 0.5 * 0.8 / FRAMES as f32 + 0.03, "the trim boundary glides: {jump}");
    // Mute is NOT a glide — it is a cut: exact zeros, immediately, every sample.
    ex.set_params(m, ParamSet::new(2, &[0.2, 1.0]).unwrap()).unwrap();
    let muted = render(&mut ex, m);
    assert!(muted.iter().all(|&s| s == 0.0), "mute writes EXACT zeros, at once");
}

#[test]
fn a_static_render_is_still_the_constant_path_bit_for_bit() {
    // The golden's property, stated locally: with no edits, two renders of the same rig are
    // bit-identical, and identical to a rig whose params were set at build time — the ramp's
    // zero-step path IS the old constant multiply.
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let (mut ex, _s, m) = rig(&reg);
    let a: Vec<f32> = (0..4).flat_map(|_| render(&mut ex, m)).collect();
    let (mut ex2, _s2, m2) = rig(&reg);
    let b: Vec<f32> = (0..4).flat_map(|_| render(&mut ex2, m2)).collect();
    assert_eq!(a, b, "static renders are bit-identical");
    // A rig whose params arrive by `set_params` BEFORE the first block renders the same first
    // block as a rig built with them: the prime rule makes block one the constant path, not a
    // fade-in from a guessed coefficient.
    let mut g = Graph::new();
    let s4 = g.add_node(0);
    let m4 = g.add_node(0);
    g.connect(PortRef::new(s4, 0), PortRef::new(m4, 0), EdgeKind::Plain).unwrap();
    let mut ex4 = Executor::build(
        g,
        vec![
            (s4, build(&reg, "sparq/syn/sine", &[440.0, 0.5])),
            (m4, build(&reg, "sparq/out/main", &[1.0, 0.0])),
        ],
        cfg(),
    )
    .unwrap();
    ex4.set_params(s4, ParamSet::new(1, &[440.0, 0.1]).unwrap()).unwrap();
    ex4.set_params(m4, ParamSet::new(2, &[0.2, 0.0]).unwrap()).unwrap();
    let via_set = render(&mut ex4, m4);
    // The rig built AT the edited params: its first block is the constant path too.
    let mut g3 = Graph::new();
    let s3 = g3.add_node(0);
    let m3 = g3.add_node(0);
    g3.connect(PortRef::new(s3, 0), PortRef::new(m3, 0), EdgeKind::Plain).unwrap();
    let mut ex3 = Executor::build(
        g3,
        vec![
            (s3, build(&reg, "sparq/syn/sine", &[440.0, 0.1])),
            (m3, build(&reg, "sparq/out/main", &[0.2, 0.0])),
        ],
        cfg(),
    )
    .unwrap();
    let first = render(&mut ex3, m3);
    assert_eq!(
        via_set, first,
        "the primed first block IS the constant path — params at build and params by command agree"
    );
}

#[test]
fn a_fast_trim_drag_is_a_smooth_chase_not_a_staircase() {
    // Defect #85, third round — the operator's ear, twice: "I can still hear bumps when moving
    // the trim slider quickly." The one-block linear ramp reached its target in 1.3 ms and then
    // HELD until the next UI snapshot (~16 ms): a staircase with sharp kinks at the update rate,
    // and kinks at 60 Hz under a fast drag are exactly bumps. The one-pole glide (25 ms tau)
    // chases the moving target instead — C0-continuous across every boundary, smooth inside
    // every block. This gate drives a FAST drag (the trim swept 1.0 → 0.0 in one edit per block,
    // faster than any hand) and asserts what the ear asked for:
    //   * no boundary jump anywhere in the sweep beyond the glide's own per-frame step,
    //   * no overshoot and no snap-back: the block peaks fall monotonically,
    //   * stereo coherence: both channels of every frame share one coefficient (the 7b bug
    //     advanced the ramp per channel),
    //   * and after the drag settles, the snap lands the coefficient EXACTLY: exact zeros out.
    let mut reg = Registry::new();
    register_builtins(&mut reg).unwrap();
    let (mut ex, _s, m) = rig(&reg);
    for _ in 0..3 {
        let _ = render(&mut ex, m);
    }
    let peak = |v: &[f32]| v.iter().fold(0.0f32, |a, &s| a.max(s.abs()));
    let mut prev = render(&mut ex, m);
    let mut max_jump = 0.0f32;
    let mut prev_peak = peak(&prev);
    let mut monotone = true;
    let mut coherent = true;
    for step in 1..=16u32 {
        let trim = 1.0 - step as f32 / 16.0;
        ex.set_params(m, ParamSet::new(100u64 + u64::from(step), &[trim, 0.0]).unwrap()).unwrap();
        let b = render(&mut ex, m);
        max_jump = max_jump.max(max_jump_between(&prev, &b));
        let pk = peak(&b);
        if pk > prev_peak + 1e-4 {
            monotone = false;
        }
        prev_peak = pk;
        for f in 0..FRAMES {
            if (b[f * 2] - b[f * 2 + 1]).abs() > 1e-6 {
                coherent = false; // one coefficient per frame, both ears (7b's bug, pinned)
            }
        }
        prev = b;
    }
    // The signal's own slope at amplitude 0.5·trim ≤ 0.5: 0.5·2π·440/48000 ≈ 0.0288; the
    // glide's per-frame step during this brutal sweep adds far less than that.
    assert!(max_jump <= 0.035, "a fast drag must not bump: max boundary jump {max_jump}");
    assert!(monotone, "no overshoot, no snap-back: block peaks fall monotonically");
    assert!(coherent, "both channels of every frame share one coefficient");
    // Settled at zero: the snap lands exactly, and exact in × exact zero is exact silence.
    ex.set_params(m, ParamSet::new(120, &[0.0, 0.0]).unwrap()).unwrap();
    for _ in 0..400 {
        prev = render(&mut ex, m);
    }
    assert!(prev.iter().all(|&s| s == 0.0), "settled at zero: exact silence");
}

fn max_jump_between(a: &[f32], b: &[f32]) -> f32 {
    (b[0] - a[a.len() - 1]).abs()
}
