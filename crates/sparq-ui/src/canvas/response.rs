//! The response-plot model (WO-012 increment 5, D2/D3): log-frequency × dB axes, the
//! magnitude-grid → polyline mapping, and the probe marker's geometry — toolkit-independent and
//! unit-tested, the `scope`/`levels` precedent.
//!
//! This module CONSUMES magnitudes; it never computes them. The per-module curve contract
//! (WO012-INC5-PLAN.md D2) puts the DSP in the module that owns it — `SvfFilter::magnitude_at`
//! in `sparq-audio`, pinned to the sine sweep there — and the dispatch that feeds it in
//! `sparq-app` (the only layer that sees both crates). What lives here is the display half:
//!
//! * the **declared axes**: `10 Hz … min(20 kHz, 0.4999·fs)` — the floor is the svf manifest's
//!   own cutoff minimum, the ceiling its `recompute` clamp, so the curve always covers the full
//!   cutoff travel and never plots a frequency the filter refuses — and `−60 … +18 dB`, the
//!   resonance peak's honest headroom;
//! * a **fixed log-spaced grid** ([`GRID_POINTS`] frequencies, computed once per param change
//!   by the shell's cache, not per pixel);
//! * the grid → polyline mapping, with every non-finite magnitude sanitised to the axis floor
//!   (a misbehaving coefficient set gets a flat line at −60 dB, never NaN pixels — the scope's
//!   rule);
//! * the **marker** (D3): a read-only probe's x-geometry, its ≥ 44 px audited capture band, and
//!   the freq/dB readout interpolated on the grid — the marker MOVES NO PARAM (cutoff stays the
//!   slider's job; a marker that also wrote cutoff would be a second door to the same param).
//!
//! The word formats ([`format_hz`], [`format_db`]) live here so the painted readout and the
//! logged one cannot drift apart — one vocabulary for the same number.

use std::collections::BTreeMap;

use crate::canvas::model::NodeId;
use crate::geom::{Rect, Vec2};
use crate::tokens::LAYOUT_TOUCH_MIN_TARGET;

/// The axis floor: the svf manifest's cutoff minimum (the curve covers the full cutoff travel).
pub const FREQ_MIN_HZ: f64 = 10.0;
/// The axis ceiling before the rate clamp: the svf manifest's cutoff maximum.
pub const FREQ_MAX_HZ: f64 = 20_000.0;
/// The rate clamp factor of `SvfFilter::recompute` — the grid never plots a frequency the
/// filter itself refuses (`fc.clamp(1.0, fs · 0.4999)`).
pub const NYQUIST_GUARD: f64 = 0.4999;
/// The dB axis floor: a misbehaving or deeply-attenuated response reads here, never below.
pub const DB_FLOOR: f64 = -60.0;
/// The dB axis ceiling: the resonance peak's honest headroom (Q ≈ 10 at resonance 1.0 is
/// +20 dB; +18 plots the peak's shoulder without compressing every flat response).
pub const DB_CEILING: f64 = 18.0;
/// The fixed grid size: log-spaced points per curve, computed once per param change.
pub const GRID_POINTS: usize = 128;

/// The declared axes of one response plot: log frequency × dB, both ranges clamped to what the
/// module and the rate allow. `Copy` so the shell can hand them to the canvas per frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axes {
    /// Lowest plotted frequency, Hz.
    pub f_min: f64,
    /// Highest plotted frequency, Hz — `min(FREQ_MAX_HZ, NYQUIST_GUARD · fs)`.
    pub f_max: f64,
    /// Lowest plotted magnitude, dB.
    pub db_min: f64,
    /// Highest plotted magnitude, dB.
    pub db_max: f64,
}

impl Default for Axes {
    fn default() -> Self {
        Self::at(0)
    }
}

impl Axes {
    /// The axes for a negotiated sample rate: the declared 10 Hz … 20 kHz band, cut back to
    /// `0.4999·fs` where the rate is lower — the filter's own `recompute` clamp, so the display
    /// never plots a frequency the DSP refuses. A zero rate (a session that has not negotiated)
    /// keeps the declared band; the degenerate `f_max ≤ f_min` case collapses onto the floor and
    /// every mapping below guards its zero span.
    #[must_use]
    pub fn at(sample_rate: u32) -> Self {
        let fs = f64::from(sample_rate);
        let f_max = if fs > 0.0 { FREQ_MAX_HZ.min(NYQUIST_GUARD * fs) } else { FREQ_MAX_HZ };
        Self {
            f_min: FREQ_MIN_HZ,
            f_max: f_max.max(FREQ_MIN_HZ),
            db_min: DB_FLOOR,
            db_max: DB_CEILING,
        }
    }

    /// A frequency clamped into the plotted range; non-finite asks read as the floor.
    #[must_use]
    pub fn clamp_freq(&self, hz: f64) -> f64 {
        if hz.is_finite() {
            hz.clamp(self.f_min, self.f_max)
        } else {
            self.f_min
        }
    }

    /// A dB value clamped into the plotted range; non-finite reads as the floor.
    #[must_use]
    pub fn clamp_db(&self, db: f64) -> f64 {
        if db.is_finite() {
            db.clamp(self.db_min, self.db_max)
        } else {
            self.db_min
        }
    }

    /// A linear magnitude as plotted dB: `20·log10`, sanitised — zero, negative, non-finite and
    /// sub-floor values all read [`DB_FLOOR`], so a curve can never emit a NaN pixel.
    #[must_use]
    pub fn db_of(&self, mag: f64) -> f64 {
        if mag.is_finite() && mag > 0.0 {
            self.clamp_db(20.0 * mag.log10())
        } else {
            self.db_min
        }
    }

    fn log_span(&self) -> f64 {
        (self.f_max / self.f_min).ln().max(f64::EPSILON)
    }

    /// The screen x of a frequency inside `r` (log mapping, clamped into the rect).
    #[must_use]
    pub fn x_of_freq(&self, r: Rect, hz: f64) -> f32 {
        let f = self.clamp_freq(hz);
        let t = (f / self.f_min).ln() / self.log_span();
        let t = if t.is_finite() { t.clamp(0.0, 1.0) } else { 0.0 };
        r.min.x + t as f32 * r.width()
    }

    /// The frequency at a screen x inside `r` (the inverse of [`Self::x_of_freq`]; the marker
    /// drag's mapping — clamped, so a finger outside the well still reads a legal frequency).
    #[must_use]
    pub fn freq_of_x(&self, r: Rect, x: f32) -> f64 {
        let w = r.width().max(f32::EPSILON);
        let t = ((x - r.min.x) / w).clamp(0.0, 1.0);
        if t <= 0.0 {
            return self.f_min;
        }
        if t >= 1.0 {
            return self.f_max;
        }
        self.clamp_freq(self.f_min * (f64::from(t) * self.log_span()).exp())
    }

    /// The screen y of a dB value inside `r` (`db_max` at the top, `db_min` at the bottom).
    #[must_use]
    pub fn y_of_db(&self, r: Rect, db: f64) -> f32 {
        let db = self.clamp_db(db);
        let span = (self.db_max - self.db_min).max(f64::EPSILON);
        let t = ((self.db_max - db) / span) as f32;
        r.min.y + t * r.height()
    }

    /// The dB value at a screen y inside `r` (the inverse of [`Self::y_of_db`]).
    #[must_use]
    pub fn db_of_y(&self, r: Rect, y: f32) -> f64 {
        let h = r.height().max(f32::EPSILON);
        let t = f64::from(((y - r.min.y) / h).clamp(0.0, 1.0));
        self.clamp_db(self.db_max - t * (self.db_max - self.db_min))
    }
}

/// The fixed log-spaced frequency grid: [`GRID_POINTS`] frequencies from `f_min` to `f_max`
/// inclusive, constant ratio between neighbours. Computed once per param change (the shell's
/// cache), never per pixel.
#[must_use]
pub fn grid(axes: &Axes) -> Vec<f64> {
    let span = (axes.f_max / axes.f_min).ln();
    (0..GRID_POINTS)
        .map(|i| {
            let t = i as f64 / (GRID_POINTS - 1) as f64;
            let f = axes.f_min * (t * span).exp();
            axes.clamp_freq(f)
        })
        .collect()
}

/// Map a magnitude grid to a polyline inside `r`: x log-frequency, y dB, every magnitude
/// through [`Axes::db_of`] (non-finite → the floor). Parallel slices; a length mismatch
/// truncates to the shorter — the grid and its magnitudes come from one dispatch and cannot
/// legitimately differ.
#[must_use]
pub fn polyline(freqs: &[f64], mags: &[f64], axes: &Axes, r: Rect) -> Vec<Vec2> {
    freqs
        .iter()
        .zip(mags.iter())
        .map(|(&f, &m)| Vec2::new(axes.x_of_freq(r, f), axes.y_of_db(r, axes.db_of(m))))
        .collect()
}

/// The marker's hairline x inside `r` — [`Axes::x_of_freq`], named for the display vocabulary.
#[must_use]
pub fn marker_x(hz: f64, axes: &Axes, r: Rect) -> f32 {
    axes.x_of_freq(r, hz)
}

/// The marker's capture band (D3): full plot height, at least [`LAYOUT_TOUCH_MIN_TARGET`] wide
/// (or the whole plot where the well is narrower — the audit then measures what exists and says
/// so), centred on the marker's x and kept inside `r`. Zoom-invariant by construction: the
/// inspector is screen-space chrome, and this rect is what the layout audit measures as the
/// marker's class-S touch target.
#[must_use]
pub fn marker_capture(hz: f64, axes: &Axes, r: Rect) -> Rect {
    let min_w = LAYOUT_TOUCH_MIN_TARGET as f32;
    let w = min_w.min(r.width().max(0.0));
    let cx = marker_x(hz, axes, r).clamp(r.min.x + w / 2.0, (r.max.x - w / 2.0).max(r.min.x));
    Rect::new(Vec2::new(cx - w / 2.0, r.min.y), Vec2::new(cx + w / 2.0, r.max.y))
}

/// The marker readout: the frequency (clamped into the axes) and the curve's dB there,
/// interpolated between the two bracketing grid points linearly in log-f (the axis's own
/// spacing) and in dB (what the eye compares). `None` when the grid is empty or its slices
/// disagree — the painter then shows no readout rather than inventing one.
#[must_use]
pub fn readout(freqs: &[f64], mags: &[f64], hz: f64, axes: &Axes) -> Option<(f64, f64)> {
    if freqs.is_empty() || freqs.len() != mags.len() {
        return None;
    }
    let f = axes.clamp_freq(hz);
    if freqs.len() == 1 {
        return Some((f, axes.db_of(mags[0])));
    }
    // The bracketing pair: the last grid point at or below f (the grid is ascending). The walk
    // stops with `i + 1` a legal index, so an f past the last point brackets onto the last pair
    // and clamps there — never off the end.
    let mut i = 0usize;
    while i + 2 < freqs.len() && freqs[i + 1] <= f {
        i += 1;
    }
    let (f0, f1) = (freqs[i], freqs[i + 1]);
    let (d0, d1) = (axes.db_of(mags[i]), axes.db_of(mags[i + 1]));
    let span = (f1 / f0).ln();
    let t = if span.abs() > f64::EPSILON && f0 > 0.0 {
        ((f / f0).ln() / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some((f, d0 + t * (d1 - d0)))
}

/// One module's computed response curve — the payload the shell hands the painter and the
/// canvas's marker mapping (`ScopeTraces` discipline: at rest the map is empty and the plot
/// shows its honest no-curve state, never a faked flat line).
#[derive(Clone, Debug, PartialEq)]
pub struct ResponseFrame {
    /// The axes the grid was computed for (they carry the negotiated rate's ceiling).
    pub axes: Axes,
    /// The log-spaced grid, ascending — [`grid`] of `axes`, stored so the painter and the
    /// readout interpolate on exactly the frequencies the dispatch evaluated.
    pub freqs: Vec<f64>,
    /// The linear magnitudes, parallel to `freqs`.
    pub mags: Vec<f64>,
}

impl ResponseFrame {
    /// A frame from a dispatch's magnitudes over the axes' own grid. `mags` shorter or longer
    /// than [`GRID_POINTS`] is a caller bug the consumers truncate honestly (zip), not a panic.
    #[must_use]
    pub fn new(axes: Axes, mags: Vec<f64>) -> Self {
        Self { axes, freqs: grid(&axes), mags }
    }

    /// The curve's dB at `hz` — the marker readout's model half.
    #[must_use]
    pub fn db_at(&self, hz: f64) -> Option<f64> {
        readout(&self.freqs, &self.mags, hz, &self.axes).map(|(_, db)| db)
    }
}

/// The per-frame curve set, keyed by canvas node id — every node whose module declares a curve
/// (the `inset` registry's `Well::Curve`), computed by the shell's dispatch. `BTreeMap` for
/// deterministic iteration (the `NodeLevels`/`LiveMeters` discipline).
pub type Curves = BTreeMap<NodeId, ResponseFrame>;

/// A frequency in the readout's words, in the look-board's numeric discipline (§5): Hz below
/// 10 000, the SI prefix at or above it (`12.4 kHz`, not `12400 Hz`) — one vocabulary for the
/// painted readout and the log line, so they cannot drift.
#[must_use]
pub fn format_hz(hz: f64) -> String {
    let f = if hz.is_finite() { hz.max(0.0) } else { 0.0 };
    if f < 10_000.0 {
        format!("{f:.1} Hz")
    } else {
        format!("{:.1} kHz", f / 1000.0)
    }
}

/// A magnitude in the readout's words: signed dB, one decimal.
#[must_use]
pub fn format_db(db: f64) -> String {
    let d = if db.is_finite() { db } else { DB_FLOOR };
    format!("{d:+.1} dB")
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    fn rect() -> Rect {
        Rect::new(Vec2::new(100.0, 50.0), Vec2::new(500.0, 146.0)) // 400 × 96
    }

    #[test]
    fn axes_carry_the_declared_band_and_the_rate_clamp() {
        let a48 = Axes::at(48_000);
        assert_eq!(a48.f_min, FREQ_MIN_HZ);
        assert_eq!(a48.f_max, FREQ_MAX_HZ, "0.4999·48k = 23 995 > 20 kHz: the declared ceiling");
        assert_eq!((a48.db_min, a48.db_max), (DB_FLOOR, DB_CEILING));

        let a22 = Axes::at(22_050);
        let want = NYQUIST_GUARD * 22_050.0;
        assert!((a22.f_max - want).abs() < 1e-9, "a lower rate cuts the band back: {want}");
        assert!(a22.f_max < FREQ_MAX_HZ);

        assert_eq!(Axes::at(96_000).f_max, FREQ_MAX_HZ, "higher rates keep the declared ceiling");
        assert_eq!(Axes::at(0).f_max, FREQ_MAX_HZ, "an unnegotiated rate keeps the band, no crash");
        // The manifest's own cutoff range must always fit inside the axes.
        assert!(a48.f_min <= 10.0 && a48.f_max >= 20_000.0);
    }

    #[test]
    fn freq_and_db_mappings_round_trip_and_clamp() {
        let a = Axes::at(48_000);
        let r = rect();
        for f in [10.0f64, 31.6, 100.0, 440.0, 1_000.0, 10_000.0, 20_000.0] {
            let x = a.x_of_freq(r, f);
            assert!(x >= r.min.x && x <= r.max.x, "{f} Hz maps inside the rect");
            let back = a.freq_of_x(r, x);
            assert!((back / f - 1.0).abs() < 1e-6, "round trip at {f} Hz: got {back}");
        }
        // The log mapping is not linear: 10 → 100 spans the same px as 1k → 10k.
        let decade_lo = a.x_of_freq(r, 100.0) - a.x_of_freq(r, 10.0);
        let decade_hi = a.x_of_freq(r, 10_000.0) - a.x_of_freq(r, 1_000.0);
        assert!((decade_lo - decade_hi).abs() < 1e-3, "equal decades, equal px");
        let mid = a.x_of_freq(r, 447.2136); // sqrt(10·20 000) ≈ geometric centre of the band
        assert!((mid - (r.min.x + r.width() / 2.0)).abs() < 2.0, "log centre is the rect centre");

        // Clamping: outside asks land on the ends, never outside the rect.
        assert_eq!(a.x_of_freq(r, 1.0), r.min.x, "below the floor clamps to the left edge");
        assert_eq!(a.x_of_freq(r, 1.0e9), r.max.x, "above the ceiling clamps to the right edge");
        assert_eq!(a.freq_of_x(r, r.min.x - 500.0), FREQ_MIN_HZ);
        assert_eq!(a.freq_of_x(r, r.max.x + 500.0), FREQ_MAX_HZ);
        assert!(a.clamp_freq(f64::NAN) == FREQ_MIN_HZ, "a NaN ask reads the floor");

        // dB axis: max at the top, min at the bottom, round trip, clamp.
        assert_eq!(a.y_of_db(r, DB_CEILING), r.min.y);
        assert_eq!(a.y_of_db(r, DB_FLOOR), r.max.y);
        for db in [-60.0f64, -24.0, 0.0, 12.0, 18.0] {
            let y = a.y_of_db(r, db);
            // The geometry is f32 (screen px); a hundredth of a dB is the honest round trip.
            assert!((a.db_of_y(r, y) - db).abs() < 1e-2, "dB round trip at {db}");
        }
        assert_eq!(a.y_of_db(r, 1.0e6), r.min.y, "over the ceiling clamps");
        assert!(a.clamp_db(f64::NAN) == DB_FLOOR);
    }

    #[test]
    fn db_of_sanitises_every_lie_to_the_floor() {
        let a = Axes::at(48_000);
        assert!((a.db_of(1.0) - 0.0).abs() < 1e-12, "unity is 0 dB");
        assert_eq!(a.db_of(0.0), DB_FLOOR, "zero magnitude reads the floor");
        assert_eq!(a.db_of(-1.0), DB_FLOOR, "negative magnitude reads the floor");
        assert_eq!(a.db_of(f64::NAN), DB_FLOOR, "NaN reads the floor");
        assert_eq!(a.db_of(f64::INFINITY), DB_FLOOR, "non-finite sanitises to the floor");
        assert_eq!(a.db_of(1.0e-9), DB_FLOOR, "−180 dB clamps into the declared band");
        assert!((a.db_of(10.0) - DB_CEILING).abs() < 1e-9, "+20 dB clamps to the declared ceiling");
        assert!((a.db_of(2.0) - 6.0206).abs() < 1e-3, "×2 is +6 dB, in band");
    }

    #[test]
    fn the_grid_is_fixed_log_spaced_and_covers_the_band() {
        let a = Axes::at(48_000);
        let g = grid(&a);
        assert_eq!(g.len(), GRID_POINTS);
        assert_eq!(g[0], a.f_min, "the grid starts exactly at the floor");
        assert!((g[GRID_POINTS - 1] / a.f_max - 1.0).abs() < 1e-9, "…and ends at the ceiling");
        let ratio = g[1] / g[0];
        for w in g.windows(2) {
            assert!(w[1] > w[0], "strictly ascending");
            assert!((w[1] / w[0] - ratio).abs() < 1e-9, "constant ratio = log spacing");
        }
        // A degenerate band (a silly rate) collapses onto the floor instead of dividing by zero.
        let mut flat = Axes::at(48_000);
        flat.f_max = flat.f_min;
        let g2 = grid(&flat);
        assert_eq!(g2.len(), GRID_POINTS);
        assert!(g2.iter().all(|f| *f == flat.f_min), "a collapsed band is all floor, never NaN");
    }

    #[test]
    fn polyline_maps_the_grid_and_never_emits_a_nan_pixel() {
        let a = Axes::at(48_000);
        let r = rect();
        let freqs = grid(&a);
        let mags: Vec<f64> = freqs.iter().map(|&f| 1.0 / (1.0 + f / 1_000.0)).collect();
        let pts = polyline(&freqs, &mags, &a, r);
        assert_eq!(pts.len(), GRID_POINTS);
        assert!(pts.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
        assert!((pts[0].x - r.min.x).abs() < 1e-3);
        assert!((pts[GRID_POINTS - 1].x - r.max.x).abs() < 1e-3);
        assert!(pts.windows(2).all(|w| w[1].x >= w[0].x), "x ascends with frequency");

        // A misbehaving coefficient set: NaN, infinity and negative magnitudes all land on the
        // floor line — flat at −60 dB, never a NaN pixel (the scope's rule).
        let bad: Vec<f64> = (0..GRID_POINTS)
            .map(|i| match i % 3 {
                0 => f64::NAN,
                1 => f64::NEG_INFINITY,
                _ => -1.0,
            })
            .collect();
        let floored = polyline(&freqs, &bad, &a, r);
        assert!(
            floored.iter().all(|p| (p.y - r.max.y).abs() < 1e-3),
            "every bad point sits on the floor"
        );

        // Length mismatch truncates to the shorter — no panic, no invented tail.
        let short = polyline(&freqs, &mags[..10], &a, r);
        assert_eq!(short.len(), 10);
    }

    #[test]
    fn the_marker_capture_is_a_touch_target_that_stays_in_the_well() {
        let a = Axes::at(48_000);
        let r = rect();
        for hz in [10.0f64, 440.0, 1_000.0, 20_000.0, 1.0e9, -5.0] {
            let c = marker_capture(hz, &a, r);
            assert!(
                c.width() >= LAYOUT_TOUCH_MIN_TARGET as f32 - 1e-3,
                "the capture band holds the 44 px floor at {hz} Hz: {} px",
                c.width()
            );
            assert!(c.min.x >= r.min.x - 1e-3 && c.max.x <= r.max.x + 1e-3, "inside the well");
            assert_eq!((c.min.y, c.max.y), (r.min.y, r.max.y), "full plot height");
            let mx = marker_x(hz, &a, r);
            let centre = (c.min.x + c.max.x) / 2.0;
            let want = mx.clamp(r.min.x + c.width() / 2.0, r.max.x - c.width() / 2.0);
            assert!((centre - want).abs() < 1e-3, "centred on the (clamped) marker");
        }
        // A well narrower than the floor offers what exists — the audit measures THAT and says
        // so; the model does not lie about a 44 px band that is not there.
        let tiny = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(30.0, 96.0));
        let c = marker_capture(1_000.0, &a, tiny);
        assert!((c.width() - 30.0).abs() < 1e-3, "the capture is the whole narrow well");
    }

    #[test]
    fn the_readout_interpolates_on_the_grid_and_refuses_a_broken_one() {
        let a = Axes::at(48_000);
        // A synthetic grid where dB is linear in log-f: the interpolation must be exact.
        let freqs = vec![10.0f64, 100.0, 1_000.0, 10_000.0];
        let mags = vec![1.0, 0.1, 0.01, 0.001]; // 0, −20, −40, −60 dB
        let (_, db) = readout(&freqs, &mags, 100.0, &a).unwrap();
        assert!((db + 20.0).abs() < 1e-9, "on a grid point: that point's dB");
        let (f, db) = readout(&freqs, &mags, 31.622_776, &a).unwrap(); // sqrt(10·100)
        assert!((f - 31.622_776).abs() < 1e-6);
        assert!((db + 10.0).abs() < 1e-3, "the log-f midpoint reads −10 dB");
        // Clamped at both ends.
        assert!((readout(&freqs, &mags, 1.0, &a).unwrap().1).abs() < 1e-9, "below: the first dB");
        let (_, top) = readout(&freqs, &mags, 1.0e9, &a).unwrap();
        assert!((top + 60.0).abs() < 1e-9 || top == DB_FLOOR, "above: the last dB (floored)");
        // Broken grids refuse rather than invent.
        assert!(readout(&[], &[], 440.0, &a).is_none());
        assert!(readout(&freqs, &mags[..2], 440.0, &a).is_none(), "mismatched slices: None");
        // A single-point grid reads its own dB.
        let one = readout(&[440.0], &[0.5], 440.0, &a).unwrap();
        assert!((one.1 - Axes::at(48_000).db_of(0.5)).abs() < 1e-9);
    }

    #[test]
    fn a_response_frame_reads_its_own_curve() {
        let a = Axes::at(48_000);
        let frame = ResponseFrame::new(a, vec![1.0; GRID_POINTS]); // flat unity
        assert_eq!(frame.freqs.len(), GRID_POINTS, "the frame stores the grid it was built on");
        assert_eq!(frame.freqs, grid(&a), "…exactly");
        let db = frame.db_at(1_000.0).unwrap();
        assert!(db.abs() < 1e-9, "a flat unity curve reads 0 dB everywhere");
        let broken = ResponseFrame { axes: a, freqs: grid(&a), mags: Vec::new() };
        assert!(broken.db_at(1_000.0).is_none(), "a broken frame refuses, never invents");
    }

    #[test]
    fn the_readout_words_have_one_vocabulary() {
        assert_eq!(format_hz(440.0), "440.0 Hz");
        assert_eq!(
            format_hz(1_240.0),
            "1240.0 Hz",
            "the mockup's own readout, below the prefix line"
        );
        assert_eq!(format_hz(9_999.5), "9999.5 Hz", "just below the prefix line stays Hz");
        assert_eq!(format_hz(10_000.0), "10.0 kHz", "the prefix starts at 10 000 (look-board §5)");
        assert_eq!(format_hz(12_345.0), "12.3 kHz");
        assert_eq!(format_hz(f64::NAN), "0.0 Hz", "a NaN reads as zero, never 'NaN kHz'");
        assert_eq!(format_db(0.0), "+0.0 dB");
        assert_eq!(format_db(-3.456), "-3.5 dB");
        assert_eq!(format_db(12.0), "+12.0 dB");
        assert_eq!(format_db(f64::NAN), "-60.0 dB", "non-finite reads the floor");
    }
}
