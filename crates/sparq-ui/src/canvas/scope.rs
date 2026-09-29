//! The scope model (WO-013 increment 6): rolling waveform accumulation, trigger alignment and
//! display geometry for `dsp/scope` — toolkit-independent and unit-tested, the `levels`/`entry`
//! precedent. The contract is the module manifest's own words: **the wire is the binding, the
//! ring is the payload** — a scope's `x`/`y` input wires name the source `(node, port)` whose
//! published `AnalysisUpdate` waveform the trace accumulates; the UI owns the buffer ("display
//! state, not project state"); the audio thread computes no pixel (`Scope::process` is a no-op
//! and stays one).
//!
//! Honesty rules, declared not buried:
//!
//! * An UNBOUND input keeps an empty trace and the display shows the flat rest line — "a flat
//!   line, not a crash" (the manifest). The rest state and a zero signal look the same because
//!   they make the same claim.
//! * The trigger aligns the window to the LAST rising crossing of its level; no crossing in
//!   reach falls back to free-run for that frame — a trace that waits forever for a crossing
//!   that never comes would be a blank screen pretending to be a bug.
//! * Decimation is stride-based (≤ one point per pixel column): a spike narrower than a column
//!   can be stepped over. Min/max envelope rendering is a declared later refinement.
//! * Non-finite samples are sanitised to zero at the geometry boundary — a misbehaving module
//!   gets a flat line, never NaN pixels.

use std::collections::BTreeMap;

use crate::canvas::model::NodeId;
use crate::geom::{Rect, Vec2};

/// The hard sample cap of one axis trace. At 96 kHz this is ~683 ms — the manifest's 500 ms
/// maximum timebase fits at every rate the HAL negotiates. The clamp is declared, not silent:
/// a timebase beyond it shows the newest `MAX_TRACE` samples and no more.
pub const MAX_TRACE: usize = 65_536;

/// The manifest's param ranges, mirrored as the model's clamp bounds (the manifest is the
/// source; a sparq-app test pins the param ORDER so the two cannot drift silently).
pub const TIMEBASE_MS_MIN: f32 = 1.0;
/// See [`TIMEBASE_MS_MIN`].
pub const TIMEBASE_MS_MAX: f32 = 500.0;
/// See [`TIMEBASE_MS_MIN`].
pub const TRIGGER_MIN: f32 = -1.0;
/// See [`TIMEBASE_MS_MIN`].
pub const TRIGGER_MAX: f32 = 1.0;
/// See [`TIMEBASE_MS_MIN`].
pub const GAIN_MIN: f32 = 0.0;
/// See [`TIMEBASE_MS_MIN`].
pub const GAIN_MAX: f32 = 8.0;

/// A rolling waveform buffer for one axis of one scope: `push` appends drained block payloads
/// in ring order (oldest falls off the front), and the capacity is the timebase sized at the
/// session's negotiated sample rate ([`ScopeView::capacity_at`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TraceBuf {
    samples: Vec<f32>,
    cap: usize,
}

impl TraceBuf {
    /// An empty buffer with no capacity — pushes are dropped until it is sized (a session
    /// sizes every trace it feeds; an unsized buffer is the at-rest shape).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty buffer holding at most `cap` samples (clamped to `1..=MAX_TRACE`).
    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self { samples: Vec::new(), cap: cap.clamp(1, MAX_TRACE) }
    }

    /// Re-cap, retaining the NEWEST samples that fit — a timebase edit is a zoom, not a reset
    /// (declared: growing keeps everything, shrinking keeps the tail).
    pub fn resize(&mut self, cap: usize) {
        self.cap = cap.clamp(1, MAX_TRACE);
        if self.samples.len() > self.cap {
            let excess = self.samples.len() - self.cap;
            self.samples.drain(0..excess);
        }
    }

    /// Append one drained payload, rolling. Ring order is block order, so the buffer is
    /// time-ordered by construction.
    pub fn push(&mut self, wave: &[f32]) {
        if self.cap == 0 {
            return; // unsized (at rest): the payload is honestly dropped — nothing feeds this axis
        }
        self.samples.extend_from_slice(wave);
        if self.samples.len() > self.cap {
            let excess = self.samples.len() - self.cap;
            self.samples.drain(0..excess);
        }
    }

    /// The accumulated samples, oldest first.
    #[must_use]
    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    /// Samples held.
    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Nothing accumulated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The capacity this buffer rolls at.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    /// Peak |sample| — the glow's level input (the wires' own vocabulary).
    #[must_use]
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0.0f32, |a, &s| a.max(s.abs()))
    }

    /// Drop everything (a rebind: the new source's signal must not inherit the old one's tail).
    pub fn clear(&mut self) {
        self.samples.clear();
    }
}

/// The window the trigger selects: `trigger == 0` (or no rising crossing in reach) is
/// free-run — the whole buffer, which the capacity already bounds to the newest `timebase`.
/// A non-zero trigger starts the window at the LAST rising crossing (`prev < t && s >= t`),
/// scanning back from the newest sample.
#[must_use]
pub fn window(buf: &TraceBuf, trigger: f32) -> &[f32] {
    let s = buf.samples();
    if trigger == 0.0 || s.len() < 2 {
        return s;
    }
    for i in (1..s.len()).rev() {
        if s[i - 1] < trigger && s[i] >= trigger {
            return &s[i..];
        }
    }
    s // no crossing in reach: free-run fallback, declared
}

/// The scope's display parameters, read from the node per frame and CLAMPED here — the model
/// does not trust that the snapshot arrived through the inspector's clamp (belt, braces, and
/// a unit test): a NaN or out-of-range value becomes the nearest declared bound, never odd
/// geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScopeView {
    /// Display timebase in ms (manifest: 1..500, default 20).
    pub timebase_ms: f32,
    /// X/Y mode (the manifest's `mode` param: 0 = waveform, ≥ 0.5 = X/Y).
    pub mode_xy: bool,
    /// Trigger level (0 = free-run).
    pub trigger: f32,
    /// Vertical gain (display zoom; does not alter the published signal).
    pub gain: f32,
}

impl Default for ScopeView {
    fn default() -> Self {
        Self { timebase_ms: 20.0, mode_xy: false, trigger: 0.0, gain: 1.0 }
    }
}

impl ScopeView {
    /// Build from the raw param values, clamping each to the manifest's declared range.
    /// Non-finite inputs fall back to the manifest defaults — a NaN param is a bug somewhere
    /// upstream, and the display answers it with the default view, not with NaN pixels.
    #[must_use]
    pub fn from_params(timebase_ms: f32, mode: f32, trigger: f32, gain: f32) -> Self {
        let d = Self::default();
        let sane = |v: f32, fallback: f32| {
            if v.is_finite() {
                v
            } else {
                fallback
            }
        };
        Self {
            timebase_ms: sane(timebase_ms, d.timebase_ms).clamp(TIMEBASE_MS_MIN, TIMEBASE_MS_MAX),
            mode_xy: sane(mode, 0.0) >= 0.5,
            trigger: sane(trigger, d.trigger).clamp(TRIGGER_MIN, TRIGGER_MAX),
            gain: sane(gain, d.gain).clamp(GAIN_MIN, GAIN_MAX),
        }
    }

    /// The trace capacity for this timebase at the negotiated rate: whole blocks, clamped to
    /// [`MAX_TRACE`]. The session sizes buffers with THIS — the same "build for the negotiated
    /// truth" discipline as the executor.
    #[must_use]
    pub fn capacity_at(&self, sample_rate: u32, block_frames: usize) -> usize {
        let want = (f64::from(self.timebase_ms) / 1000.0 * f64::from(sample_rate)).ceil() as usize;
        let block = block_frames.max(1);
        let whole = want.div_ceil(block) * block;
        whole.clamp(block, MAX_TRACE)
    }
}

/// Map the trigger-selected window onto the display rect: x = time (oldest left, newest
/// right — the sonogram/scroll convention), y = `sample × gain` around the vertical centre,
/// CLAMPED at the rect's edges (driven past the edges the trace flattens there — the honest
/// oscilloscope behaviour). Stride-decimated to ≤ one point per pixel column. An empty window
/// yields an empty polyline; the painter draws [`rest_line`] for it.
#[must_use]
pub fn trace_polyline(buf: &TraceBuf, view: &ScopeView, rect: Rect) -> Vec<Vec2> {
    let w = window(buf, view.trigger);
    if w.is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Vec::new();
    }
    let cols = rect.width().max(1.0) as usize;
    let stride = w.len().div_ceil(cols).max(1);
    let half_h = rect.height() / 2.0;
    let cy = rect.center().y;
    let mut out = Vec::with_capacity(w.len() / stride + 1);
    let last = w.len() - 1;
    for (n, i) in (0..w.len()).step_by(stride).enumerate() {
        let s = sanitise(w[i]);
        let x = rect.min.x + rect.width() * (i as f32 / last.max(1) as f32);
        let y = (cy - s * view.gain * half_h).clamp(rect.min.y, rect.max.y);
        let _ = n;
        out.push(Vec2::new(x, y));
    }
    out
}

/// The X/Y (Lissajous) polyline: `x[i]` against `y[i]` over the SHORTER of the two buffers
/// (the axes are independent publications; pairing stops where either runs out — declared),
/// both gain-scaled around the rect's centre and clamped to it. The trigger is ignored (a
/// figure with no time axis has nothing to align).
#[must_use]
pub fn xy_polyline(x: &TraceBuf, y: &TraceBuf, view: &ScopeView, rect: Rect) -> Vec<Vec2> {
    let (xs, ys) = (x.samples(), y.samples());
    let n = xs.len().min(ys.len());
    if n == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return Vec::new();
    }
    let cols = rect.width().max(1.0) as usize;
    let stride = n.div_ceil(cols).max(1);
    let (hw, hh) = (rect.width() / 2.0, rect.height() / 2.0);
    let c = rect.center();
    let mut out = Vec::with_capacity(n / stride + 1);
    for i in (0..n).step_by(stride) {
        let px = (c.x + sanitise(xs[i]) * view.gain * hw).clamp(rect.min.x, rect.max.x);
        let py = (c.y - sanitise(ys[i]) * view.gain * hh).clamp(rect.min.y, rect.max.y);
        out.push(Vec2::new(px, py));
    }
    out
}

/// The rest line: the flat centre line the display shows when a window is empty (unbound,
/// at rest, or a zero signal — the same claim, the same pixels).
#[must_use]
pub fn rest_line(rect: Rect) -> [Vec2; 2] {
    let cy = rect.center().y;
    [Vec2::new(rect.min.x, cy), Vec2::new(rect.max.x, cy)]
}

/// One scope node's two axis traces.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScopeTrace {
    /// The `x` input's accumulation (waveform mode: the signal; X/Y mode: horizontal).
    pub x: TraceBuf,
    /// The `y` input's accumulation (X/Y mode: vertical; waveform mode ignores it).
    pub y: TraceBuf,
}

/// The transient, session-fed set the painter reads: scope node → its traces. `BTreeMap` for
/// deterministic iteration (the `NodeLevels` discipline). Lives on `CanvasState` beside
/// `levels` — NOT undoable, NOT project state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScopeTraces {
    map: BTreeMap<NodeId, ScopeTrace>,
}

impl ScopeTraces {
    /// An empty set: every scope shows its rest line until a session feeds it.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The trace entry for a scope node, created (or re-capped to `cap`) on demand. A capacity
    /// change retains the newest samples that fit ([`TraceBuf::resize`]).
    pub fn ensure(&mut self, node: NodeId, cap: usize) -> &mut ScopeTrace {
        let entry = self.map.entry(node).or_default();
        if entry.x.capacity() != cap.clamp(1, MAX_TRACE) {
            entry.x.resize(cap);
            entry.y.resize(cap);
        }
        entry
    }

    /// A scope node's traces, if any were ever fed.
    #[must_use]
    pub fn get(&self, node: NodeId) -> Option<&ScopeTrace> {
        self.map.get(&node)
    }

    /// Mutable access (the session's feed path).
    pub fn get_mut(&mut self, node: NodeId) -> Option<&mut ScopeTrace> {
        self.map.get_mut(&node)
    }

    /// Drop one node's traces (a rebind of BOTH axes, a deleted node).
    pub fn remove(&mut self, node: NodeId) {
        self.map.remove(&node);
    }

    /// Keep only the nodes the predicate accepts — the session's per-frame prune of scopes
    /// that left the graph.
    pub fn retain(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        self.map.retain(|id, _| keep(*id));
    }

    /// Drop everything (session stop: the traces are facts about a stream that ended).
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Scope nodes with traces.
    #[must_use]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// No scope has ever been fed.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// Non-finite samples are sanitised at the geometry boundary — the model's declared rule.
fn sanitise(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn rect() -> Rect {
        Rect::new(Vec2::new(100.0, 100.0), Vec2::new(300.0, 200.0))
    }

    #[test]
    fn the_buffer_rolls_and_keeps_the_newest() {
        let mut b = TraceBuf::with_capacity(4);
        b.push(&[1.0, 2.0, 3.0]);
        b.push(&[4.0, 5.0, 6.0]);
        assert_eq!(b.samples(), &[3.0, 4.0, 5.0, 6.0], "the oldest fall off the front");
        assert_eq!(b.len(), 4);
        assert_eq!(b.peak(), 6.0);
    }

    #[test]
    fn an_unsized_buffer_drops_payloads_honestly() {
        let mut b = TraceBuf::new();
        b.push(&[1.0, 2.0]);
        assert!(b.is_empty(), "nothing feeds an unsized axis; the payload is dropped, not queued");
    }

    #[test]
    fn resize_is_a_zoom_not_a_reset() {
        let mut b = TraceBuf::with_capacity(8);
        b.push(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        b.resize(3);
        assert_eq!(b.samples(), &[3.0, 4.0, 5.0], "shrinking keeps the tail");
        b.resize(8);
        assert_eq!(b.samples(), &[3.0, 4.0, 5.0], "growing keeps everything it has");
    }

    #[test]
    fn capacity_comes_from_the_negotiated_truth_and_clamps() {
        let v = ScopeView { timebase_ms: 20.0, ..ScopeView::default() };
        // 20 ms at 48 kHz = 960 samples = 15 whole 64-frame blocks.
        assert_eq!(v.capacity_at(48_000, 64), 960);
        // 20 ms at 44.1 kHz = 882 → rounds UP to whole 96-frame blocks: 9 × 96 = 960.
        assert_eq!(v.capacity_at(44_100, 96), 960);
        let huge = ScopeView { timebase_ms: 500.0, ..ScopeView::default() };
        assert_eq!(huge.capacity_at(96_000, 64), 48_000, "500 ms at 96 kHz fits under the cap");
        assert!(huge.capacity_at(96_000, 64) <= MAX_TRACE);
        assert_eq!(TraceBuf::with_capacity(usize::MAX).capacity(), MAX_TRACE, "the clamp is real");
    }

    #[test]
    fn the_trigger_aligns_to_the_last_rising_crossing() {
        // A hand-built buffer: crossings at index 1 (−1→+1) and index 3 (+... rising again).
        let mut b = TraceBuf::with_capacity(16);
        b.push(&[-1.0, 1.0, 0.5, -0.5, 0.9, 0.2]);
        // trigger 0.8: rising crossings at i=1 (−1→1 ≥ 0.8) and i=4 (−0.5→0.9 ≥ 0.8).
        // The LAST one wins: the window starts at index 4.
        let w = window(&b, 0.8);
        assert_eq!(w, &[0.9, 0.2], "the window starts at the last rising crossing");
        // No crossing in reach (1.2 is above every sample) → the free-run fallback.
        assert_eq!(window(&b, 1.2).len(), 6, "no crossing: free-run, never a blank screen");
        // trigger == 0 is free-run by definition.
        assert_eq!(window(&b, 0.0).len(), 6);
    }

    #[test]
    fn the_polyline_maps_the_window_onto_the_rect_and_clamps() {
        let r = rect(); // 200 × 100 at (100,100): centre y = 150, half-height 50
        let mut b = TraceBuf::with_capacity(4);
        b.push(&[0.0, 1.0, -1.0, 0.0]);
        let v = ScopeView::default(); // gain 1, free-run
        let pts = trace_polyline(&b, &v, r);
        assert_eq!(pts.len(), 4);
        assert!((pts[0].x - 100.0).abs() < 1e-3 && (pts[3].x - 300.0).abs() < 1e-3);
        assert!((pts[0].y - 150.0).abs() < 1e-3, "zero sits on the centre line");
        assert!((pts[1].y - 100.0).abs() < 1e-3, "+1 × gain 1 reaches the top edge");
        assert!((pts[2].y - 200.0).abs() < 1e-3, "−1 reaches the bottom edge");
        // Gain 8 drives past the edges: the trace FLATTENS at the boundary, honestly.
        let hot = ScopeView { gain: 8.0, ..ScopeView::default() };
        let pts = trace_polyline(&b, &hot, r);
        assert_eq!(pts[1].y, r.min.y, "clamped at the top");
        assert_eq!(pts[2].y, r.max.y, "clamped at the bottom");
        // Empty window → empty polyline (the painter draws the rest line).
        assert!(trace_polyline(&TraceBuf::new(), &v, r).is_empty());
    }

    #[test]
    fn decimation_bounds_the_points_by_the_rect_width() {
        let r = rect(); // 200 px wide
        let mut b = TraceBuf::with_capacity(10_000);
        let wave: Vec<f32> = (0..10_000).map(|i| (i as f32 * 0.01).sin()).collect();
        b.push(&wave);
        let pts = trace_polyline(&b, &ScopeView::default(), r);
        assert!(pts.len() <= 201, "one point per pixel column (+ the final sample): {}", pts.len());
    }

    #[test]
    fn xy_pairs_over_the_shorter_window_and_ignores_the_trigger() {
        let r = rect();
        let mut x = TraceBuf::with_capacity(8);
        let mut y = TraceBuf::with_capacity(8);
        x.push(&[1.0, 0.0, -1.0, 0.0]);
        y.push(&[0.0, 1.0, 0.0]);
        let v = ScopeView { mode_xy: true, trigger: 0.7, ..ScopeView::default() };
        let pts = xy_polyline(&x, &y, &v, r);
        assert_eq!(pts.len(), 3, "pairing stops where the shorter axis runs out");
        // x=1 → right edge; y=0 → centre.
        assert!((pts[0].x - 300.0).abs() < 1e-3 && (pts[0].y - 150.0).abs() < 1e-3);
        assert!(xy_polyline(&x, &TraceBuf::new(), &v, r).is_empty(), "an unbound axis: rest");
    }

    #[test]
    fn params_clamp_and_nan_falls_back_to_defaults() {
        let v = ScopeView::from_params(9_000.0, 1.0, -4.0, f32::NAN);
        assert_eq!(v.timebase_ms, TIMEBASE_MS_MAX);
        assert!(v.mode_xy);
        assert_eq!(v.trigger, TRIGGER_MIN);
        assert_eq!(v.gain, 1.0, "a NaN gain is the default gain, never NaN geometry");
        let v2 = ScopeView::from_params(f32::NEG_INFINITY, 0.4, 0.0, 0.0);
        assert_eq!(v2.timebase_ms, 20.0, "a non-finite timebase is the DEFAULT, per the doc rule");
        assert!(!v2.mode_xy, "0.4 rounds toward waveform mode");
    }

    #[test]
    fn the_set_ensures_prunes_and_clears() {
        let mut t = ScopeTraces::new();
        assert!(t.is_empty());
        t.ensure(3, 960).x.push(&[0.1; 4]);
        t.ensure(1, 960);
        assert_eq!(t.len(), 2);
        // ensure re-caps in place (a timebase edit), retaining the tail.
        t.ensure(3, 2).x.push(&[0.2]);
        assert_eq!(t.get(3).unwrap().x.samples(), &[0.1, 0.2]);
        t.retain(|id| id == 3);
        assert_eq!(t.len(), 1);
        t.clear();
        assert!(t.is_empty(), "session stop: the traces are facts about a stream that ended");
    }

    #[test]
    fn the_rest_line_is_the_centre_flat_line() {
        let [a, b] = rest_line(rect());
        assert_eq!(a.y, b.y);
        assert!((a.y - 150.0).abs() < 1e-3);
        assert_eq!(a.x, 100.0);
        assert_eq!(b.x, 300.0);
    }
}
