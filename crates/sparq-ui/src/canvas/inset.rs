//! The node inset displays (WO-012 increment 5, D1): which well a node wears, and the pure
//! param → shape mappings the wells draw — toolkit-independent and unit-tested, the
//! `scope`/`levels` precedent.
//!
//! The registry replaces the painter's hard-coded `if module_id == …` chain: [`inset_well`] is
//! the single place that says which module wears which well (the `OUT_MAIN_ID`/`SCOPE_ID`
//! named-constant discipline, extended), and [`well_for`] adds the ONE port-derived rule the
//! registry cannot know — *every node with an audio output wears meter bars*, because the ring
//! already carries per-port stereo peaks for all of them (WO-012 increment 4 painted only
//! `out/main`; the registry lookup wins over the fallback, so `flt/svf` wears its curve, not
//! bars).
//!
//! Honesty rules (the CHECKLIST's, made concrete):
//!
//! * **LIVE where the ring carries it, at rest otherwise.** The Meters well reads the session's
//!   [`crate::canvas::levels::LiveMeters`] (audio outputs) or the published cv port level
//!   ([`crate::canvas::levels::NodeLevels::port`], `ana/rms`/`ana/tap`) — empty map, empty
//!   well. The Envelope / Sparkline / Curve wells ship their PARAM-DERIVED shapes: a
//!   param-shaped envelope triangle is the module's declared response, exactly as the svf curve
//!   is, and it is honest before the first block. The LIVE cv-history overlay (a `Sparklines`
//!   session set, the `ScopeTraces` discipline) is the declared next half — it waits on a ring
//!   that carries per-block cv history (D1′ of the plan of record).
//! * **Mirrored, pinned, single-sourced.** The shape math below mirrors the DSP's declared
//!   behaviour — `env.rs`'s 5-time-constant exponential segments, `modules.rs`'s `Lfo::shape`
//!   wave table — the way `scope.rs` mirrors the manifest's param ranges: constants here, the
//!   param ORDER pinned by a `sparq-app` test, so the two cannot drift silently. The svf
//!   thumbnail plots the SAME grid the inspector does (`super::response`), computed by the
//!   filter's own `magnitude_at` — one source, two readers.

use crate::canvas::model::NodeSpec;
use crate::geom::{Rect, Vec2};
use sparq_module_api::port::{Direction, PortType};

/// The stable id of the attack/decay envelope (its well is the Envelope triangle).
pub const ENV_AD_ID: &str = "sparq/env/ad";
/// The stable id of the LFO (its well is the one-period Sparkline outline).
pub const LFO_ID: &str = "sparq/mod/lfo";
/// The stable id of the RMS/peak follower (its well is a single cv bar from `level`).
pub const RMS_ID: &str = "sparq/ana/rms";
/// The stable id of the signal tap (its well is a single cv bar from `peak`).
pub const TAP_ID: &str = "sparq/ana/tap";
/// The stable id of the state-variable filter (its well is the response-curve thumbnail, and
/// it is the first — and so far only — module that declares an inspector response curve).
pub const SVF_ID: &str = "sparq/flt/svf";

/// The kinds of inset well a node body can wear. `dsp/scope`'s well (WO-013 increment 6) and
/// `out/main`'s meter bars (WO-012 increment 4) are the shipped patterns; this increment
/// generalises the dispatch and adds the mockup's remaining wells.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Well {
    /// The waveform well (`dsp/scope`), drawn from the analysis ring's traces.
    Scope,
    /// Meter bars: stereo bars from [`crate::canvas::levels::LiveMeters`] for an audio output,
    /// a single bar from the published cv port level for `ana/rms`/`ana/tap`.
    Meters,
    /// The attack/decay envelope triangle ([`envelope_polyline`]).
    Envelope,
    /// The response-curve thumbnail ([`super::response`], no marker — the marker lives in the
    /// inspector's plot, D3).
    Curve,
    /// One LFO period, outlined ([`lfo_period_polyline`]).
    Sparkline,
}

/// The well registry: which module wears which well, by stable id. THE single place this is
/// decided — no module-id literal reaches the painter. `None` = unregistered, and [`well_for`]
/// adds nothing else: the registry is the only door.
#[must_use]
pub fn inset_well(module_id: &str) -> Option<Well> {
    match module_id {
        crate::canvas::SCOPE_ID => Some(Well::Scope),
        crate::canvas::OUT_MAIN_ID => Some(Well::Meters),
        ENV_AD_ID => Some(Well::Envelope),
        LFO_ID => Some(Well::Sparkline),
        SVF_ID => Some(Well::Curve),
        _ => None,
    }
}

/// The well a NODE wears: the registry, and nothing else. Operator ruling 2026-09-30 (the
/// device look at increment 5): meter bars are `out/main`'s alone — a bar on every audio node
/// read as noise, so increment 5's audio-output fallback is GONE. Everything unregistered
/// wears nothing: the body is the box, honest.
#[must_use]
pub fn well_for(spec: &NodeSpec) -> Option<Well> {
    inset_well(&spec.module_id)
}

/// Whether a module declares a frequency-response curve (D2's contract, display side): the
/// registry's `Well::Curve`, so the node thumbnail and the inspector plot cannot disagree
/// about who has a curve. The DSP side of the dispatch lives in `sparq-app` (the only layer
/// that sees `sparq-audio`), keyed on the same [`SVF_ID`] constant.
#[must_use]
pub fn declares_curve(module_id: &str) -> bool {
    inset_well(module_id) == Some(Well::Curve)
}

/// The index of a spec's first audio OUTPUT port (the stereo meter bars' source), or `None`.
#[must_use]
pub fn first_audio_out(spec: &NodeSpec) -> Option<usize> {
    spec.ports
        .iter()
        .enumerate()
        .find(|(_, p)| p.direction == Direction::Out && p.port_type == PortType::Audio)
        .map(|(i, _)| i)
}

// ------------------------------------------------------------------ env/ad: the AD triangle
//
// The manifest's param ranges, mirrored as the model's clamp bounds (the manifest is the
// source; a sparq-app test pins the param ORDER so the two cannot drift silently — the
// scope.rs discipline).

/// `env/ad` attack minimum, ms (manifest).
pub const ENV_ATTACK_MIN: f32 = 0.1;
/// `env/ad` attack maximum, ms (manifest).
pub const ENV_ATTACK_MAX: f32 = 2_000.0;
/// `env/ad` attack default, ms (manifest).
pub const ENV_ATTACK_DEFAULT: f32 = 2.0;
/// `env/ad` decay minimum, ms (manifest).
pub const ENV_DECAY_MIN: f32 = 1.0;
/// `env/ad` decay maximum, ms (manifest).
pub const ENV_DECAY_MAX: f32 = 4_000.0;
/// `env/ad` decay default, ms (manifest).
pub const ENV_DECAY_DEFAULT: f32 = 200.0;

/// The exponential segment's time-constant count: `env.rs`'s `exp_coeff` sizes its one-pole
/// coefficient so the segment covers FIVE time constants in its declared time — the display
/// draws the same `1 − e^(−5u)` / `e^(−5u)` shapes, reaching 99.3 % at the segment end. (The
/// DSP's 1e-5 peak-snap makes the audible exp attack run ~2.3× its declared time; the DECLARED
/// shape is what the well shows — the manifest's promise, drawn to scale.)
pub const ENV_TIME_CONSTANTS: f64 = 5.0;

/// Sample points per envelope segment (two segments + the seam vertex).
pub const ENVELOPE_POINTS: usize = 33;

/// The `env/ad` well's shape: the attack/decay triangle from params — the module's declared
/// response, honest at rest. x spans `attack + decay` linearly in time, y is the 0..1 level
/// (top of `r` = 1). `linear` is the manifest's `curve` param (0 exp, 1 lin); exp segments
/// mirror [`ENV_TIME_CONSTANTS`]. NaN params sanitise to the manifest defaults, out-of-range
/// clamps — the `ScopeView::from_params` discipline. The exp attack's last point (99.3 %) and
/// the decay's first (100 %) are BOTH emitted: the DSP snaps to peak between them, and the
/// polyline says so with a hair of a vertical seam instead of hiding it.
#[must_use]
pub fn envelope_polyline(attack_ms: f32, decay_ms: f32, linear: bool, r: Rect) -> Vec<Vec2> {
    let a = f64::from(sanitise(attack_ms, ENV_ATTACK_MIN, ENV_ATTACK_MAX, ENV_ATTACK_DEFAULT));
    let d = f64::from(sanitise(decay_ms, ENV_DECAY_MIN, ENV_DECAY_MAX, ENV_DECAY_DEFAULT));
    let total = (a + d).max(f64::EPSILON);
    let pt = |t: f64, y: f64| {
        Vec2::new(
            r.min.x + (t * f64::from(r.width())) as f32,
            r.max.y - (y * f64::from(r.height())) as f32,
        )
    };
    let mut out = Vec::with_capacity(2 * ENVELOPE_POINTS);
    for i in 0..ENVELOPE_POINTS {
        let u = i as f64 / (ENVELOPE_POINTS - 1) as f64;
        let y = if linear { u } else { 1.0 - (-ENV_TIME_CONSTANTS * u).exp() };
        out.push(pt(u * a / total, y));
    }
    for i in 0..ENVELOPE_POINTS {
        let v = i as f64 / (ENVELOPE_POINTS - 1) as f64;
        let y = if linear { 1.0 - v } else { (-ENV_TIME_CONSTANTS * v).exp() };
        out.push(pt((a + v * d) / total, y));
    }
    out
}

// ------------------------------------------------------------------- mod/lfo: one period

/// The LFO manifest's param bounds (rate 0.05..50 Hz, depth 0..1), mirrored.
pub const LFO_DEPTH_MAX: f32 = 1.0;
/// See [`LFO_DEPTH_MAX`].
pub const LFO_DEPTH_DEFAULT: f32 = 1.0;

/// Outline points for the smooth LFO shapes (square gets exact vertices instead).
pub const LFO_PERIOD_POINTS: usize = 65;

/// The `mod/lfo` well's shape: ONE period of the selected wave, unipolar, scaled by `depth` —
/// the declared rest outline (the live rolling cv history is D1′'s declared next half). The
/// wave table mirrors `modules.rs`'s `Lfo::shape` exactly: 0 sine, 1 triangle, 2 saw,
/// 3 square, all mapped into 0..1; an out-of-domain shape reads the held middle, as the DSP
/// does. `rate` does not appear in the geometry because the x-axis IS one period — a faster
/// LFO does not draw a narrower wave, it redraws the same shape more often (declared).
#[must_use]
pub fn lfo_period_polyline(shape: f32, depth: f32, r: Rect) -> Vec<Vec2> {
    let shape = if shape.is_finite() { shape.round().clamp(0.0, 3.0) as i32 } else { 0 };
    let depth = f64::from(sanitise(depth, 0.0, LFO_DEPTH_MAX, LFO_DEPTH_DEFAULT));
    let pt = |p: f64, y: f64| {
        Vec2::new(
            r.min.x + (p * f64::from(r.width())) as f32,
            r.max.y - (y * f64::from(r.height())) as f32,
        )
    };
    if shape == 3 {
        // Square: exact vertices — a sampled square would round its own edge off.
        return vec![pt(0.0, depth), pt(0.5, depth), pt(0.5, 0.0), pt(1.0, 0.0)];
    }
    (0..LFO_PERIOD_POINTS)
        .map(|i| {
            let p = i as f64 / (LFO_PERIOD_POINTS - 1) as f64;
            pt(p, lfo_shape(shape, p) * depth)
        })
        .collect()
}

/// The LFO's wave table, mirrored from `modules.rs`'s `Lfo::shape` (pinned by a sparq-app
/// test): phase 0..1 → unipolar 0..1.
#[must_use]
pub fn lfo_shape(shape: i32, p: f64) -> f64 {
    match shape {
        0 => ((p * std::f64::consts::TAU).sin() + 1.0) / 2.0,
        1 => {
            if p < 0.5 {
                2.0 * p
            } else {
                2.0 - 2.0 * p
            }
        },
        2 => p,
        3 => {
            if p < 0.5 {
                1.0
            } else {
                0.0
            }
        },
        _ => 0.5, // an out-of-domain shape is a held middle, not a panic (contract §9.7)
    }
}

/// Clamp a param into its manifest range; NaN sanitises to the default (never propagates into
/// geometry — the scope's rule).
fn sanitise(v: f32, lo: f32, hi: f32, default: f32) -> f32 {
    if v.is_finite() {
        v.clamp(lo, hi)
    } else {
        default
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use crate::canvas::model::NodeSpec;
    use crate::geom::Vec2 as V;
    use sparq_module_api::manifest::Port;
    use sparq_module_api::port::{ChannelSet, Multiplicity};

    fn port(id: &str, dir: Direction, ty: PortType) -> Port {
        Port {
            id: id.into(),
            direction: dir,
            port_type: ty,
            required: false,
            channel_set: Some(ChannelSet::Mono),
            channel_set_variable: false,
            cv_rate: None,
            cv_range: None,
            cv_reduce: Default::default(),
            cv_interp: Default::default(),
            event_kinds: Vec::new(),
            multiplicity: Multiplicity::Single,
            latency_contribution: 0,
        }
    }
    fn audio(id: &str, dir: Direction) -> Port {
        port(id, dir, PortType::Audio)
    }
    fn cv(id: &str, dir: Direction) -> Port {
        port(id, dir, PortType::Cv)
    }
    fn spec(id: &str, ports: Vec<Port>) -> NodeSpec {
        NodeSpec::new(id, id.rsplit('/').next().unwrap_or(id), ports)
    }
    fn rect() -> Rect {
        Rect::new(V::new(10.0, 20.0), V::new(110.0, 70.0)) // 100 × 50
    }

    #[test]
    fn the_registry_keys_every_well_on_its_named_constant() {
        assert_eq!(inset_well(crate::canvas::SCOPE_ID), Some(Well::Scope));
        assert_eq!(inset_well(crate::canvas::OUT_MAIN_ID), Some(Well::Meters));
        assert_eq!(inset_well(ENV_AD_ID), Some(Well::Envelope));
        assert_eq!(inset_well(LFO_ID), Some(Well::Sparkline));
        // The analysers' cv bars are gone with the operator's 2026-09-30 ruling: meter bars
        // are out/main's alone, so rms/tap register nothing.
        assert_eq!(inset_well(RMS_ID), None);
        assert_eq!(inset_well(TAP_ID), None);
        assert_eq!(inset_well(SVF_ID), Some(Well::Curve));
        assert_eq!(inset_well("sparq/syn/sine"), None, "unregistered: no well by id");
        assert_eq!(inset_well(""), None);
        // The id constants are the manifest's ids, pinned (a drift here moves wells silently).
        assert_eq!(ENV_AD_ID, "sparq/env/ad");
        assert_eq!(LFO_ID, "sparq/mod/lfo");
        assert_eq!(RMS_ID, "sparq/ana/rms");
        assert_eq!(TAP_ID, "sparq/ana/tap");
        assert_eq!(SVF_ID, "sparq/flt/svf");
    }

    #[test]
    fn meter_bars_are_out_mains_alone_and_the_registry_is_the_only_door() {
        // Operator ruling 2026-09-30: bars on every audio node read as noise; `out/main` is
        // the one meter the patch has.
        let out_main = spec(
            crate::canvas::OUT_MAIN_ID,
            vec![audio("in", Direction::In), audio("out", Direction::Out)],
        );
        assert_eq!(well_for(&out_main), Some(Well::Meters));
        let sine = spec("sparq/syn/sine", vec![audio("out", Direction::Out)]);
        assert_eq!(well_for(&sine), None, "a source wears no bars");
        let gain =
            spec("sparq/util/gain", vec![audio("in", Direction::In), audio("out", Direction::Out)]);
        assert_eq!(well_for(&gain), None, "a processor wears no bars");
        let rms = spec(RMS_ID, vec![audio("in", Direction::In), cv("level", Direction::Out)]);
        assert_eq!(well_for(&rms), None, "the analyser's cv bar is gone with the ruling");
        // The registry's other wells stand: svf wears its curve (audio out notwithstanding).
        let svf = spec(
            SVF_ID,
            vec![
                audio("in", Direction::In),
                audio("out", Direction::Out),
                cv("cutoff-mod", Direction::In),
            ],
        );
        assert_eq!(well_for(&svf), Some(Well::Curve));
        // A node with NO ports at all wears nothing and cannot panic.
        assert_eq!(well_for(&spec("sparq/x/y", vec![])), None);
    }

    #[test]
    fn declares_curve_is_the_registrys_curve_arm() {
        assert!(declares_curve(SVF_ID));
        assert!(!declares_curve(crate::canvas::SCOPE_ID), "a scope is a waveform, not a curve");
        assert!(
            !declares_curve("sparq/util/delay"),
            "…and a future delay comb must register first"
        );
    }

    #[test]
    fn the_envelope_triangle_is_the_declared_shape() {
        let r = rect();
        // Linear, equal times: the peak sits at the horizontal centre, at the rect's top.
        let lin = envelope_polyline(100.0, 100.0, true, r);
        assert_eq!(lin.len(), 2 * ENVELOPE_POINTS);
        let peak = lin.iter().enumerate().min_by(|a, b| a.1.y.total_cmp(&b.1.y)).unwrap();
        assert!((peak.1.x - r.center().x).abs() < 1.5, "lin peak at the centre: {:?}", peak.1);
        assert!((peak.1.y - r.min.y).abs() < 1e-3, "peak touches the top (level 1)");
        assert!(
            (lin[0].x - r.min.x).abs() < 1e-3 && (lin[0].y - r.max.y).abs() < 1e-3,
            "starts at 0"
        );
        let last = lin.last().unwrap();
        assert!((last.x - r.max.x).abs() < 1e-3 && (last.y - r.max.y).abs() < 1e-3, "ends at 0");
        // The linear attack is a straight ramp: every attack point on the rising diagonal.
        for (i, p) in lin[..ENVELOPE_POINTS].iter().enumerate() {
            let u = i as f64 / (ENVELOPE_POINTS - 1) as f64;
            let want_y = r.max.y - (u * f64::from(r.height())) as f32;
            assert!((p.y - want_y).abs() < 1e-3, "lin attack point {i}");
        }

        // Exponential: the 5-time-constant rule — 63.2 % at one time constant (u = 0.2),
        // 99.3 % at the segment end; the shape is concave (fast start, slow finish).
        let ex = envelope_polyline(100.0, 100.0, false, r);
        let at = |u: f64| {
            let i = (u * (ENVELOPE_POINTS - 1) as f64).round() as usize;
            1.0 - f64::from(ex[i].y - r.min.y) / f64::from(r.height())
        };
        let i_tau = (0.2 * (ENVELOPE_POINTS - 1) as f64).round() as usize;
        let u_tau = i_tau as f64 / (ENVELOPE_POINTS - 1) as f64;
        let one_tau = 1.0 - f64::from(ex[i_tau].y - r.min.y) / f64::from(r.height());
        let want_tau = 1.0 - (-ENV_TIME_CONSTANTS * u_tau).exp();
        assert!((one_tau - want_tau).abs() < 1e-3, "one time constant: {one_tau} vs {want_tau}");
        assert!(at(1.0) > 0.99 && at(1.0) < 1.0, "the exp attack ends at 99.3 %, below the snap");
        assert!(at(0.4) < 2.0 * at(0.2), "concave: the second τ adds less than the first");
        // The decay segment starts at the snapped peak (the seam vertex the doc declares).
        let decay_start = ex[ENVELOPE_POINTS];
        assert!((decay_start.y - r.min.y).abs() < 1e-3, "decay starts at level 1 (the DSP's snap)");

        // Asymmetric times move the peak proportionally: attack 1 : decay 3 → peak at x = 25 %.
        let asym = envelope_polyline(100.0, 300.0, true, r);
        let p = asym.iter().enumerate().min_by(|a, b| a.1.y.total_cmp(&b.1.y)).unwrap();
        assert!((p.1.x - (r.min.x + r.width() * 0.25)).abs() < 1.5, "peak at 25 %: {:?}", p.1);

        // Sanitising: NaN → the manifest defaults, out-of-range → clamped, all finite.
        let nan = envelope_polyline(f32::NAN, f32::NAN, false, r);
        let defaults = envelope_polyline(ENV_ATTACK_DEFAULT, ENV_DECAY_DEFAULT, false, r);
        assert_eq!(nan, defaults, "NaN params read the manifest defaults");
        let wild = envelope_polyline(-50.0, 1.0e9, true, r);
        assert!(wild.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        let clamped = envelope_polyline(0.0, 1.0e9, true, r);
        let at_limits = envelope_polyline(ENV_ATTACK_MIN, ENV_DECAY_MAX, true, r);
        assert_eq!(clamped, at_limits, "finite out-of-range clamps onto the manifest bounds");
        // INFINITY is NON-finite: it sanitises to the default, it does not clamp (one rule).
        let inf = envelope_polyline(0.1, f32::INFINITY, true, r);
        let dflt = envelope_polyline(0.1, ENV_DECAY_DEFAULT, true, r);
        assert_eq!(inf, dflt, "a non-finite param reads the manifest default");
    }

    #[test]
    fn the_lfo_outline_is_one_period_of_the_declared_wave() {
        let r = rect();
        // Sine (shape 0): starts and ends at the middle (unipolar 0.5), peaks at quarter-phase.
        let s = lfo_period_polyline(0.0, 1.0, r);
        assert_eq!(s.len(), LFO_PERIOD_POINTS);
        let mid_y = r.max.y - 0.5 * r.height();
        assert!((s[0].y - mid_y).abs() < 1e-3 && (s[LFO_PERIOD_POINTS - 1].y - mid_y).abs() < 1e-3);
        let q = s[(LFO_PERIOD_POINTS - 1) / 4];
        assert!((q.y - r.min.y).abs() < 1e-3, "the sine peaks at the top (unipolar 1.0)");
        // Triangle (1): ramp up to the centre, ramp down.
        let t = lfo_period_polyline(1.0, 1.0, r);
        assert!((t[(LFO_PERIOD_POINTS - 1) / 2].y - r.min.y).abs() < 1e-3, "tri peaks mid-period");
        // Saw (2): monotone rising, wraps.
        let saw = lfo_period_polyline(2.0, 1.0, r);
        assert!(
            saw.windows(2).all(|w| w[1].y <= w[0].y + 1e-6),
            "saw rises (y falls) monotonically"
        );
        assert!((saw[0].y - r.max.y).abs() < 1e-3, "saw starts at level 0");
        // Square (3): exact vertices, not a rounded sampling.
        let sq = lfo_period_polyline(3.0, 1.0, r);
        assert_eq!(sq.len(), 4, "four exact vertices");
        assert!((sq[0].y - r.min.y).abs() < 1e-3, "high first half (top of rect = level 1)");
        assert!((sq[2].y - r.max.y).abs() < 1e-3, "low second half");
        assert_eq!(sq[1].x, sq[2].x, "the edge is vertical");
        // Depth scales the swing; zero depth is a flat line at level 0.
        let half = lfo_period_polyline(0.0, 0.5, r);
        let hq = half[(LFO_PERIOD_POINTS - 1) / 4];
        assert!((hq.y - (r.max.y - 0.5 * r.height())).abs() < 1e-3, "depth 0.5 peaks at half");
        let flat = lfo_period_polyline(0.0, 0.0, r);
        assert!(flat.iter().all(|p| (p.y - r.max.y).abs() < 1e-3), "depth 0: flat at level 0");
        // Sanitising: NaN shape/demote, out-of-domain shape → the DSP's held middle.
        assert_eq!(lfo_period_polyline(f32::NAN, 1.0, r), lfo_period_polyline(0.0, 1.0, r));
        assert_eq!(lfo_shape(9, 0.25), 0.5, "out-of-domain shape holds the middle (contract §9.7)");
        assert_eq!(lfo_shape(-1, 0.25), 0.5);
        // The wave table mirrors the DSP's own: check the defining values of each shape.
        assert!(
            (lfo_shape(0, 0.0) - 0.5).abs() < 1e-12 && (lfo_shape(0, 0.25) - 1.0).abs() < 1e-12
        );
        assert_eq!((lfo_shape(1, 0.25), lfo_shape(1, 0.75)), (0.5, 0.5));
        assert_eq!(lfo_shape(2, 0.75), 0.75);
        assert_eq!((lfo_shape(3, 0.25), lfo_shape(3, 0.75)), (1.0, 0.0));
    }

    #[test]
    fn a_degenerate_well_rect_produces_no_nan_geometry() {
        let collapsed = Rect::new(V::new(5.0, 5.0), V::new(5.0, 5.0));
        assert!(envelope_polyline(2.0, 200.0, false, collapsed).iter().all(|p| p.x.is_finite()));
        assert!(lfo_period_polyline(1.0, 1.0, collapsed).iter().all(|p| p.y.is_finite()));
    }
}
