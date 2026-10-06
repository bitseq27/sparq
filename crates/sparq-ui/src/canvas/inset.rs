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
    /// The clock wheel (`mod/clk`, operator round 4 D4): four concentric divisor rings —
    /// 4/8/16/32 centre-out ([`clock_ring_rects`]) — each wearing a phase dot turned by the
    /// module's published `phase` cv-out ([`ring_point`]). At rest: static, phase 0 (the
    /// declared at-rest rule — the wheel turns only while the clock runs).
    ClockWheel,
    /// The rolling level graph (`ana/rms`, round 4 D13): a display-side history of the
    /// level-port readings behind the existing single bar. The history never enters the
    /// engine or the journal, and a restart starts it empty (declared).
    Graph,
    /// The note quantizer's keyboard (`util/quant`, round 4 D11): twelve keys across the
    /// well band; the passing pitch lights its key ([`keyboard_key`]), and in Custom mode
    /// the keys show mask membership — a tap flips it through `layout::Hit::Key`.
    Keyboard,
    /// The random-step bars (`mod/rand`, round 4 D12): the module's step values as a bar
    /// row, MIRRORED from the pinned seed hash (the display never re-rolls), with the
    /// cursor bar lit from the published position.
    Steps,
    /// The instrument display band (WO-020 INC4 §8.2): the wall-class surface an instrument-layer
    /// node wears INSTEAD of a well — the guest's display list, painted by the host (the at-rest
    /// JSON while the loader is absent, the live instance's frames once INC5 lands). The band's
    /// geometry is [`super::layout::instrument_display_band`]; its content is the painter's.
    Display,
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
        // The round-4 wells (operator rulings 2026-10-02): the clock wheel turns on D4's
        // phase publication, rms wears its display-side graph (D13 — a graph, NOT a meter
        // bar: the 2026-09-30 ruling that bars are out/main's alone stands), the quantizer
        // its keyboard (D11) and the random step source its bars (D12).
        crate::canvas::CLK_ID => Some(Well::ClockWheel),
        crate::canvas::RMS_ID => Some(Well::Graph),
        crate::canvas::QUANT_ID => Some(Well::Keyboard),
        crate::canvas::RAND_ID => Some(Well::Steps),
        _ => None,
    }
}

/// The well a NODE wears: the registry, and nothing else. Operator ruling 2026-09-30 (the
/// device look at increment 5): meter bars are `out/main`'s alone — a bar on every audio node
/// read as noise, so increment 5's audio-output fallback is GONE. Everything unregistered
/// wears nothing: the body is the box, honest.
#[must_use]
pub fn well_for(spec: &NodeSpec) -> Option<Well> {
    // An instrument's display band outranks the well registry (a wall is not a well); the registry
    // stays the only door for backbone modules (operator ruling 2026-09-30 stands).
    if spec.layer == sparq_module_api::manifest::Layer::Instrument && spec.display.is_some() {
        return Some(Well::Display);
    }
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

/// The manifest index of a spec's Nth cv OUTPUT port — the wells' reading path into the live
/// level set (operator round 4): the clock's `phase` is its 0th, the quantizer's quantized
/// `pitch` its 0th, the random step's `value` its 0th and its cursor `pos` its 1st. Port-index
/// vocabulary instead of module literals (the registry decides WHO wears a well; this decides
/// WHERE the word lives), and the manifest's append discipline keeps the indices stable.
#[must_use]
pub fn nth_cv_out(spec: &NodeSpec, n: usize) -> Option<usize> {
    spec.ports
        .iter()
        .enumerate()
        .filter(|(_, p)| p.direction == Direction::Out && p.port_type == PortType::Cv)
        .nth(n)
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

// ------------------------------------------- round-4 wells: the pure geometry the painter reads
//
// The wells themselves draw in sparq-app's painter (S7); these are the shared pure mappings
// that keep the drawing a READING of published values instead of an invention — the envelope
// and LFO precedent, extended to the clock wheel (D4) and the quantizer keyboard (D11).

/// How many rings the clock wheel wears (D4): the 4/8/16/32 divisors, centre-out.
pub const CLOCK_RINGS: usize = 4;

/// How many keys the quantizer keyboard wears (D11): the twelve pitch classes.
pub const KEYBOARD_KEYS: usize = 12;

/// Semitones in the quantizer's published pitch span (D11): `pitch` runs 0..1 across ten
/// octaves, so the pitch class is `round(p × 120) mod 12`. Mirrored from the module's
/// publication here (the module is the source; a sparq-app test pins the order when the
/// round-4 module batch lands — the scope.rs discipline).
pub const QUANT_PITCH_SEMITONES: f64 = 120.0;

/// The clock wheel's ring rects (D4): concentric CENTRED SQUARES inside the well band at
/// ¼·½·¾·1 of the band's half-extent — index 0 is the innermost ring (the 4 divisor), index
/// [`CLOCK_RINGS`]−1 the outermost (the 32). The half-extent is the SHORTER side's, so the
/// wheel always inscribes in the band, however wide the card.
#[must_use]
pub fn clock_ring_rects(r: Rect) -> [Rect; CLOCK_RINGS] {
    let c = r.center();
    let half = r.width().min(r.height()) * 0.5;
    let mut out = [r; CLOCK_RINGS];
    for (k, slot) in out.iter_mut().enumerate() {
        let rad = half * (k + 1) as f32 / CLOCK_RINGS as f32;
        *slot = Rect::new(Vec2::new(c.x - rad, c.y - rad), Vec2::new(c.x + rad, c.y + rad));
    }
    out
}

/// A point on a ring (D4): `phase` 0..1 starts at the TOP and runs CLOCKWISE (screen y-down),
/// so the wheel turns like a clock, not like a unit circle — `a = phase·τ − π/2`. Phase wraps
/// through the trig (3.25 reads 0.25; no pre-normalisation to forget), and a non-finite
/// phase or radius reads the honest degenerate: phase 0, centre point.
#[must_use]
pub fn ring_point(center: Vec2, radius: f32, phase: f32) -> Vec2 {
    let phase = if phase.is_finite() { f64::from(phase) } else { 0.0 };
    let radius = if radius.is_finite() { f64::from(radius) } else { 0.0 };
    let a = phase * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
    Vec2::new(center.x + (radius * a.cos()) as f32, center.y + (radius * a.sin()) as f32)
}

/// The keyboard key a passing pitch lights (D11): `round(pitch01 × 120) mod 12` — the
/// quantizer publishes pitch as 0..1 over its [`QUANT_PITCH_SEMITONES`] span, so the pitch
/// class falls out of the same arithmetic the module itself snaps with. `keyboard_key(0.5)`
/// is 0 (middle C's class), `keyboard_key(11/120)` is 11. The modulo is EUCLIDEAN, so an
/// out-of-span (negative) pitch still lands on its class, never on a negative index, and a
/// non-finite pitch reads key 0 (never a NaN index).
#[must_use]
pub fn keyboard_key(pitch01: f32) -> usize {
    let p = if pitch01.is_finite() { f64::from(pitch01) } else { 0.0 };
    let semitone = (p * QUANT_PITCH_SEMITONES).round() as i64;
    semitone.rem_euclid(KEYBOARD_KEYS as i64) as usize
}

/// The random-step ring MIRRORED for the display (operator round 4, D12): the SAME pure
/// integer hash the `mod/rand` module walks — the lowbias32 finalizer (the splitmix32 family)
/// over `seed ^ (i × 0x9E37_79B9)`, top 24 bits over 2²⁴. `sparq-ui` is zero-dependency, so
/// the mirror lives here and is pinned from BOTH sides: the seed-42 ring in this crate's own
/// gate, and a cross-crate equality pin in `sparq-app` (the module is the source; the bars
/// show the ring the patch will actually walk, never a re-roll).
#[must_use]
pub fn rand_step_value(seed: u32, i: u32) -> f32 {
    let mut z = seed ^ i.wrapping_mul(0x9e37_79b9);
    z ^= z >> 16;
    z = z.wrapping_mul(0x7feb_352d);
    z ^= z >> 15;
    z = z.wrapping_mul(0x846c_a68b);
    z ^= z >> 16;
    (z >> 8) as f32 / 16_777_216.0
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
        assert_eq!(inset_well(SVF_ID), Some(Well::Curve));
        // The round-4 registry rows (operator rulings 2026-10-02): the clock wheel turns on
        // the D4 phase publication, rms wears a display again — a GRAPH (D13), NOT a meter
        // bar, so the 2026-09-30 ruling (bars are out/main's alone) stands — and the
        // quantizer and the random-step source wear the keyboard and the bars.
        assert_eq!(inset_well(crate::canvas::CLK_ID), Some(Well::ClockWheel));
        assert_eq!(inset_well(crate::canvas::RMS_ID), Some(Well::Graph));
        assert_eq!(inset_well(crate::canvas::QUANT_ID), Some(Well::Keyboard));
        assert_eq!(inset_well(crate::canvas::RAND_ID), Some(Well::Steps));
        // The tap analyser stays unregistered: its cv bar is gone with the 2026-09-30 ruling.
        assert_eq!(inset_well(TAP_ID), None);
        assert_eq!(inset_well("sparq/syn/sine"), None, "unregistered: no well by id");
        assert_eq!(inset_well(""), None);
        // The id constants are the manifest's ids, pinned (a drift here moves wells silently).
        assert_eq!(ENV_AD_ID, "sparq/env/ad");
        assert_eq!(LFO_ID, "sparq/mod/lfo");
        assert_eq!(TAP_ID, "sparq/ana/tap");
        assert_eq!(SVF_ID, "sparq/flt/svf");
        assert_eq!(crate::canvas::CLK_ID, "sparq/mod/clk");
        assert_eq!(crate::canvas::RMS_ID, "sparq/ana/rms");
        assert_eq!(crate::canvas::QUANT_ID, "sparq/util/quant");
        assert_eq!(crate::canvas::RAND_ID, "sparq/mod/rand");
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
        let rms = spec(
            crate::canvas::RMS_ID,
            vec![audio("in", Direction::In), cv("level", Direction::Out)],
        );
        assert_eq!(
            well_for(&rms),
            Some(Well::Graph),
            "D13 gives rms a display again — a rolling GRAPH, still not a meter bar"
        );
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
        // The round-4 helpers: a collapsed band gives every ring the one point, all finite.
        let rings = clock_ring_rects(collapsed);
        assert!(rings.iter().all(|r| r.min.x.is_finite() && r.max.y.is_finite()));
        assert!(rings.iter().all(|r| r.width() == 0.0), "no extent, no ring");
        assert!(ring_point(collapsed.center(), 0.0, 0.37).x.is_finite());
    }

    // ------------------------------------------------- round 4: the clock wheel + the keyboard

    #[test]
    fn the_clock_rings_are_concentric_squares_centre_out() {
        let r = rect(); // 100 × 50 — the shorter side rules the half-extent
        let rings = clock_ring_rects(r);
        assert_eq!(rings.len(), CLOCK_RINGS);
        let c = r.center();
        let half = r.height() * 0.5;
        for (k, ring) in rings.iter().enumerate() {
            let rad = half * (k + 1) as f32 / CLOCK_RINGS as f32;
            assert!(ring.center().distance(c) < 1e-3, "ring {k} shares the band's centre");
            assert!(
                (ring.width() - 2.0 * rad).abs() < 1e-3,
                "ring {k} radius is {}/{} of the half-extent",
                k + 1,
                CLOCK_RINGS
            );
            assert!((ring.height() - ring.width()).abs() < 1e-3, "ring {k} is a centred square");
        }
        // Centre out: strictly growing, the outermost inscribed in the band (touching the
        // shorter side's edges — the wheel never overflows the well).
        for w in rings.windows(2) {
            assert!(w[0].width() < w[1].width(), "centre-out ordering");
        }
        assert!((rings[CLOCK_RINGS - 1].min.y - r.min.y).abs() < 1e-3);
        assert!((rings[CLOCK_RINGS - 1].max.y - r.max.y).abs() < 1e-3);
    }

    #[test]
    fn ring_point_starts_at_the_top_and_runs_clockwise() {
        let c = V::new(100.0, 50.0);
        let rad = 20.0;
        // Phase 0 is the TOP (screen y-down), ¼ is the RIGHT: the wheel turns like a clock.
        let top = ring_point(c, rad, 0.0);
        assert!((top.x - c.x).abs() < 1e-3 && (top.y - (c.y - rad)).abs() < 1e-3, "{top:?}");
        let right = ring_point(c, rad, 0.25);
        assert!((right.x - (c.x + rad)).abs() < 1e-3 && (right.y - c.y).abs() < 1e-3, "{right:?}");
        let bottom = ring_point(c, rad, 0.5);
        assert!((bottom.y - (c.y + rad)).abs() < 1e-3, "½ is the bottom: {bottom:?}");
        let left = ring_point(c, rad, 0.75);
        assert!((left.x - (c.x - rad)).abs() < 1e-3, "¾ is the left: {left:?}");
        // Every phase lands ON the ring, and the wrap rides the trig — no normalisation to
        // forget (the clock's phase publication wraps at the source, but the geometry must
        // not depend on it).
        for i in 0..32 {
            let p = ring_point(c, rad, i as f32 / 32.0);
            assert!((p.distance(c) - rad).abs() < 1e-2, "phase {i}/32 is on the circle");
        }
        assert!(ring_point(c, rad, 1.25).distance(right) < 1e-3, "1.25 reads 0.25");
        // Non-finite reads the honest degenerate: the centre point, the top.
        assert_eq!(ring_point(c, f32::NAN, 0.3), c, "a NaN radius is the centre");
        assert!(ring_point(c, rad, f32::NAN).distance(top) < 1e-3, "a NaN phase is the top");
    }

    #[test]
    fn keyboard_key_maps_pitch_onto_the_twelve_classes() {
        // The two pinned examples from the D11 ruling.
        assert_eq!(keyboard_key(0.5), 0, "the span's middle is class 0");
        assert_eq!(
            keyboard_key(11.0 / 120.0),
            11,
            "one semitone under the top of the first octave"
        );
        // The whole chromatic run, in order, across an octave boundary.
        for k in 0..KEYBOARD_KEYS {
            let pitch = (60.0 + k as f32) / 120.0;
            assert_eq!(keyboard_key(pitch), k % 12, "semitone {k} of the span");
        }
        // The span wraps: 1.0 is ten octaves up — class 0 again.
        assert_eq!(keyboard_key(1.0), 0);
        // A pitch below the span lands on its CLASS (Euclidean), never a negative index.
        assert_eq!(keyboard_key(-1.0 / 120.0), 11);
        // Non-finite reads key 0 — never a NaN index into the cells.
        assert_eq!(keyboard_key(f32::NAN), 0);
        assert_eq!(keyboard_key(f32::INFINITY), 0);
    }

    #[test]
    fn nth_cv_out_reads_the_publication_port_by_class_not_by_literal() {
        // The clock's shape: four event outs, then the appended phase cv — the manifest index
        // is 4, but its PER-CLASS index is 0, and the level set is keyed by manifest index.
        let clk = spec(
            crate::canvas::CLK_ID,
            vec![
                port("4th", Direction::Out, PortType::Event),
                port("8th", Direction::Out, PortType::Event),
                port("16th", Direction::Out, PortType::Event),
                port("32nd", Direction::Out, PortType::Event),
                cv("phase", Direction::Out),
            ],
        );
        assert_eq!(nth_cv_out(&clk, 0), Some(4), "the D4 publication is manifest port 4");
        assert_eq!(nth_cv_out(&clk, 1), None, "…and it is the clock's only cv word");
        // The rand shape: value is its 0th cv out (manifest 1), pos its 1st (manifest 3).
        let rand = spec(
            crate::canvas::RAND_ID,
            vec![
                port("trig-in", Direction::In, PortType::Event),
                cv("value", Direction::Out),
                port("trig-out", Direction::Out, PortType::Event),
                cv("pos", Direction::Out),
            ],
        );
        assert_eq!(nth_cv_out(&rand, 0), Some(1), "value");
        assert_eq!(nth_cv_out(&rand, 1), Some(3), "pos");
        // A module with no cv output reads None — the well stays empty, never guesses.
        let sine = spec("sparq/syn/sine", vec![audio("out", Direction::Out)]);
        assert_eq!(nth_cv_out(&sine, 0), None);
    }

    #[test]
    fn the_rand_mirror_is_pinned_to_the_modules_seed_42_ring() {
        // The display side of the D12 pin: the SAME checked-in ring the module's own gate
        // holds (tests/r4_new_modules.rs), so a drift on either side fails a test — and the
        // sparq-app cross-crate pin proves the two files agree everywhere, not just here.
        const SEED42_RING: [u32; 8] =
            [1517363, 10542902, 13721998, 8088911, 6531285, 16292829, 12864735, 5502068];
        for (i, want) in SEED42_RING.iter().enumerate() {
            assert_eq!(
                rand_step_value(42, i as u32),
                *want as f32 / 16_777_216.0,
                "mirror step {i}"
            );
        }
        // The mirror is a pure function into 0..1.
        for seed in [0u32, 1, 42, 65_535] {
            for i in 0..16u32 {
                let v = rand_step_value(seed, i);
                assert!((0.0..1.0).contains(&v), "seed {seed} step {i}: {v}");
                assert_eq!(v, rand_step_value(seed, i), "pure");
            }
        }
    }
}
