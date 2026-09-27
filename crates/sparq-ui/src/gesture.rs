//! The gesture recogniser: pointers in, **intents** out (WO-012 task 2, `docs/ui/input-model.md` §3).
//!
//! The rule that shapes everything: *a gesture produces an intent, not an effect*. Intents are
//! dispatched to the focused surface, which decides what they mean — that is how the same
//! two-finger pan scrolls a browser in Design mode and morphs a scene in Perform mode without
//! the recogniser knowing either exists.
//!
//! Toolkit-independent by requirement (input-model §1: "the gesture model must not be swapped
//! with the renderer"): no egui, no winit, no clock. Timestamps arrive inside
//! [`PointerEvent::t_ms`]; time-based recognition (long press, five-finger hold) happens in
//! [`GestureRecognizer::advance`], which the host calls once per frame. That makes the whole
//! recogniser deterministically testable — feed it a synthetic event trace, assert the intents.
//!
//! # Conflict resolution (input-model §3, verbatim rules)
//!
//! * Longest-press wins over drag when movement is under threshold; once a drag has started, a
//!   long press can no longer fire for that contact ("a long press never fires a drag, and a
//!   drag never fires a long press" — the WO-012 acceptance criterion).
//! * A recogniser that has fired a **global** intent (undo, all-sound-off, recovery, panel
//!   toggle) locks out surface gestures until all pointers lift.
//! * Every suppression is recorded ([`Suppression`]) — "an input that did nothing must always
//!   be explainable".

use crate::geom::{Rect, Vec2};
use crate::pointer::{PointerEvent, PointerKind, PointerPhase};

use crate::tokens::{
    LAYOUT_TOUCH_GESTURE_DOUBLE_TAP_MS, LAYOUT_TOUCH_GESTURE_FINE_RESOLUTION_FACTOR,
    LAYOUT_TOUCH_GESTURE_LONG_PRESS_MS, LAYOUT_TOUCH_GESTURE_PALM_REJECT_MAX_CONTACT_AREA_MM2,
    LAYOUT_TOUCH_GESTURE_PINCH_MIN_SPAN_PX,
};

/// Which screen edge an edge-swipe came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// Left edge (rail side).
    Left,
    /// Right edge (inspector side).
    Right,
    /// Top edge.
    Top,
    /// Bottom edge (dock side).
    Bottom,
}

/// A recognised gesture, expressed as what the *user meant*, not what the widget should do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureIntent {
    /// Tap: the universal "do it" (target is the surface under `pos`).
    Activate {
        /// Where the tap landed.
        pos: Vec2,
    },
    /// Double tap: `zoom_to_fit` on canvas, `toggle_expand` elsewhere.
    DoubleTap {
        /// Where.
        pos: Vec2,
    },
    /// Long press: the context menu. Replaces right-click and hover menus entirely.
    Context {
        /// Where.
        pos: Vec2,
    },
    /// A drag began (move node, draw wire, draw envelope, sketch wavetable…).
    DragStart {
        /// Where.
        pos: Vec2,
    },
    /// Drag continuation. `scale` is 1.0 normally, `1/fine_resolution_factor` while a second
    /// finger is down — the ×10 precision assist (§14.3 rule 5).
    DragUpdate {
        /// Movement since the previous update, logical px.
        delta: Vec2,
        /// Precision scale; surfaces multiply their sensitivity by this.
        scale: f32,
    },
    /// The fine-resolution state changed (second finger down/up mid-drag). Surfaces show or hide
    /// the "fine" hint; the first-use discoverability hint lives here too.
    DragFineChanged {
        /// Whether fine mode is now active.
        active: bool,
    },
    /// The drag ended (also emitted on `Cancel`, so surfaces can always clean up; `cancelled`
    /// distinguishes a system abort, which must not commit e.g. a wire connection).
    DragEnd {
        /// Where it ended.
        pos: Vec2,
        /// Whether the system cancelled it.
        cancelled: bool,
    },
    /// Two-finger pan.
    Pan {
        /// Centre-of-mass movement since the previous event, logical px.
        delta: Vec2,
        /// Midpoint of the two pointers. A surface scrolls itself when the pan happens OVER it
        /// (the inspector, WO-013 increment 5) and otherwise passes the camera through — same
        /// reason [`Self::Zoom`] has carried its centre since WO-012: where the fingers are is
        /// part of what the gesture means, and only the recogniser knows that.
        center: Vec2,
    },
    /// Two-finger pinch/spread. `factor` is the *incremental* span ratio since the previous
    /// event; surfaces accumulate it into their zoom.
    Zoom {
        /// Span ratio since the last update (>1 spreading, <1 pinching).
        factor: f32,
        /// Midpoint of the two pointers.
        center: Vec2,
    },
    /// Two-finger tangential motion (rig map, geometry displays).
    Rotate {
        /// Radians since the last update, positive counter-clockwise in logical space.
        delta_rad: f32,
        /// Midpoint of the two pointers.
        center: Vec2,
    },
    /// Three-finger tap: undo. Global.
    Undo,
    /// Three-finger swipe down: all sound off. Global, Perform mode.
    AllSoundOff,
    /// Five-finger hold: recovery menu. Global.
    RecoveryMenu,
    /// Fast one-finger release: flick (scene morph in Perform mode).
    Flick {
        /// Normalised direction.
        dir: Vec2,
        /// Speed in px/ms at release.
        velocity_px_per_ms: f32,
    },
    /// Edge swipe inward: toggle the panel on that edge. Design mode only (the consumer filters).
    TogglePanel {
        /// The edge the swipe started from.
        edge: Edge,
    },
}

/// An input that was seen and deliberately produced nothing. The diagnostics view shows these;
/// "my touch did nothing" must never be a mystery on stage.
#[derive(Clone, Debug, PartialEq)]
pub struct Suppression {
    /// When.
    pub t_ms: u64,
    /// Which pointer.
    pub pointer: u64,
    /// Why, in words.
    pub reason: String,
}

/// Recognition thresholds. Defaults come from the tokens; the struct exists so tests can tighten
/// or loosen a single number without touching generated data.
#[derive(Clone, Copy, Debug)]
pub struct GestureConfig {
    /// Hold time for a long press (token: 350 ms).
    pub long_press_ms: u64,
    /// Window for a double tap (token: 300 ms).
    pub double_tap_ms: u64,
    /// Movement under this keeps a press a tap/long-press (input-model: 8 px).
    pub tap_max_move_px: f32,
    /// Movement at/over this turns a press into a drag (input-model: 8 px).
    pub drag_min_move_px: f32,
    /// Minimum two-pointer span before pinch is recognised (token: 40 px).
    pub pinch_min_span_px: f32,
    /// Precision assist divisor when a second finger joins a drag (token: 10).
    pub fine_resolution_factor: f32,
    /// Contacts larger than this are palms (token: 100 mm²).
    pub palm_reject_mm2: f32,
    /// Release speed above which a drag becomes a flick (input-model: 1.5 px/ms).
    pub flick_min_velocity: f32,
    /// How far back the flick velocity is measured over.
    pub flick_window_ms: u64,
    /// All-down-to-all-up window for a three-finger tap (input-model: 200 ms).
    pub three_finger_tap_ms: u64,
    /// Three-finger downward travel that means "all sound off".
    pub three_finger_swipe_px: f32,
    /// Hold time for the five-finger recovery menu (input-model: 800 ms).
    pub five_finger_hold_ms: u64,
    /// Distance from an edge that arms an edge swipe.
    pub edge_margin_px: f32,
    /// Inward travel that fires an edge swipe.
    pub edge_swipe_min_px: f32,
    /// Two-pointer dead zone before pan/zoom emit.
    pub two_pointer_dead_zone_px: f32,
}

impl Default for GestureConfig {
    /// The token values. If a token changes, gesture behaviour changes with it — that is the
    /// point of generating from tokens (WO-002).
    fn default() -> Self {
        Self {
            long_press_ms: LAYOUT_TOUCH_GESTURE_LONG_PRESS_MS as u64,
            double_tap_ms: LAYOUT_TOUCH_GESTURE_DOUBLE_TAP_MS as u64,
            tap_max_move_px: 8.0,
            drag_min_move_px: 8.0,
            pinch_min_span_px: LAYOUT_TOUCH_GESTURE_PINCH_MIN_SPAN_PX as f32,
            fine_resolution_factor: LAYOUT_TOUCH_GESTURE_FINE_RESOLUTION_FACTOR as f32,
            palm_reject_mm2: LAYOUT_TOUCH_GESTURE_PALM_REJECT_MAX_CONTACT_AREA_MM2 as f32,
            flick_min_velocity: 1.5,
            flick_window_ms: 100,
            three_finger_tap_ms: 200,
            three_finger_swipe_px: 40.0,
            five_finger_hold_ms: 800,
            edge_margin_px: 20.0,
            edge_swipe_min_px: 40.0,
            two_pointer_dead_zone_px: 4.0,
        }
    }
}

/// A tracked contact.
#[derive(Clone, Copy, Debug)]
struct Tracked {
    id: u64,
    palm: bool,
    start: Vec2,
    start_t: u64,
    last: Vec2,
    last_t: u64,
    /// Recent motion for flick velocity: (pos, t) ring, newest at `hist_head`.
    hist: [(Vec2, u64); 8],
    hist_head: usize,
    hist_len: usize,
}

impl Tracked {
    fn new(ev: &PointerEvent) -> Self {
        // The Down sample is history entry zero. Flick velocity measures over a *window*, and a
        // window with one sample in it must yield 0 px/ms rather than index an empty ring — the
        // difference between "no flick" and a panic in the input path.
        let mut hist = [(Vec2::ZERO, 0u64); 8];
        hist[0] = (ev.pos, ev.t_ms);
        Self {
            id: ev.id,
            palm: false,
            start: ev.pos,
            start_t: ev.t_ms,
            last: ev.pos,
            last_t: ev.t_ms,
            hist,
            hist_head: 1,
            hist_len: 1,
        }
    }

    fn record(&mut self, ev: &PointerEvent) {
        self.last = ev.pos;
        self.last_t = ev.t_ms;
        self.hist[self.hist_head] = (ev.pos, ev.t_ms);
        self.hist_head = (self.hist_head + 1) % self.hist.len();
        self.hist_len = (self.hist_len + 1).min(self.hist.len());
    }

    /// Speed in px/ms over the recent window. Returns zero for a single-sample window: a press
    /// that never moved has no velocity, and inventing one would fire spurious flicks.
    fn velocity(&self, window_ms: u64) -> (Vec2, f32) {
        if self.hist_len < 2 {
            return (Vec2::ZERO, 0.0);
        }
        let cap = self.hist.len();
        // `hist_head` is the NEXT write slot, so the newest sample is head-1 (mod cap) and each
        // step back is one more -1. (An earlier draft indexed by head+len-1, which points at an
        // untouched zero slot while the ring is not yet full — caught by the fine-resolution
        // test reporting a 130 px delta for a 10 px move.)
        let newest = self.hist[(self.hist_head + cap - 1) % cap];
        let mut oldest = newest;
        for i in 0..self.hist_len {
            let s = self.hist[(self.hist_head + cap - 1 - i) % cap];
            if newest.1.saturating_sub(s.1) > window_ms {
                break;
            }
            oldest = s;
        }
        let dt = newest.1.saturating_sub(oldest.1).max(1) as f32;
        let d = newest.0.sub(oldest.0);
        let v = d.length() / dt;
        let dir = if d.length() > 1e-4 { d.scale(1.0 / d.length()) } else { Vec2::ZERO };
        (dir, v)
    }
}

/// Which multi-pointer situation the recogniser is in.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// No contacts.
    Idle,
    /// One contact, undecided between tap / long-press / drag / edge-swipe.
    OneDown { id: u64, long_press_fired: bool, drag_fired: bool },
    /// One contact, dragging (optionally in fine mode with a second finger resting).
    Dragging { id: u64, fine: bool },
    /// Two contacts: pan / pinch / rotate.
    Two { last_center: Vec2, last_span: f32, last_angle: f32, engaged: bool },
    /// Three contacts: tap (undo) or swipe down (all sound off).
    Three { first_t: u64, start_center: Vec2, moved_beyond_tap: bool, fired: bool },
    /// Five or more contacts: hold for the recovery menu.
    Five { first_t: u64, fired: bool },
}

/// The recogniser. One instance per input surface (the shell owns one; a torn-off window would
/// own its own).
pub struct GestureRecognizer {
    cfg: GestureConfig,
    viewport: Rect,
    tracked: Vec<Tracked>,
    phase: Phase,
    /// A global gesture fired; surface gestures are locked out until all pointers lift.
    lockout: bool,
    last_tap: Option<(Vec2, u64)>,
    edge_candidate: Option<(Edge, Vec2)>,
    suppressions: Vec<Suppression>,
}

impl GestureRecognizer {
    /// A recogniser with the token thresholds. `viewport` arms edge swipes; until it is set the
    /// viewport is effectively infinite (edge swipes cannot fire — the safe default).
    #[must_use]
    pub fn new(cfg: GestureConfig) -> Self {
        Self {
            cfg,
            viewport: Rect::new(Vec2::new(-1e6, -1e6), Vec2::new(1e6, 1e6)),
            tracked: Vec::with_capacity(10),
            phase: Phase::Idle,
            lockout: false,
            last_tap: None,
            edge_candidate: None,
            suppressions: Vec::new(),
        }
    }

    /// Token-default recogniser.
    #[must_use]
    pub fn from_tokens() -> Self {
        Self::new(GestureConfig::default())
    }

    /// Set the logical-px viewport (the shell calls this on resize; DPI is already applied).
    pub fn set_viewport(&mut self, viewport: Rect) {
        self.viewport = viewport;
    }

    /// Feed one normalised pointer event; get back every intent it produced (usually 0 or 1).
    pub fn push(&mut self, ev: PointerEvent) -> Vec<GestureIntent> {
        let mut out = Vec::new();
        match ev.phase {
            PointerPhase::Down => self.on_down(ev, &mut out),
            PointerPhase::Moved => self.on_move(ev, &mut out),
            PointerPhase::Up => self.on_up(ev, false, &mut out),
            PointerPhase::Cancel => self.on_up(ev, true, &mut out),
        }
        out
    }

    /// Time-based recognition; call once per frame with a monotonic `now_ms`. Fires the long
    /// press and the five-finger hold, which happen *without* pointer motion.
    pub fn advance(&mut self, now_ms: u64) -> Vec<GestureIntent> {
        let mut out = Vec::new();
        match self.phase {
            Phase::OneDown { id, long_press_fired, drag_fired }
                if !long_press_fired && !drag_fired =>
            {
                // Copy out what we need before touching self.phase: holding a borrow of
                // self.tracked across the write is a borrow-check error, and cloning the whole
                // pointer table to work around it would be the worse fix.
                let snap =
                    self.tracked.iter().find(|t| t.id == id).map(|t| (t.start, t.last, t.start_t));
                if let Some((start, last, start_t)) = snap {
                    let held = now_ms.saturating_sub(start_t);
                    let still = last.distance(start) < self.cfg.tap_max_move_px;
                    if held >= self.cfg.long_press_ms && still && !self.lockout {
                        out.push(GestureIntent::Context { pos: start });
                        self.phase = Phase::OneDown { id, long_press_fired: true, drag_fired };
                    } else if held >= self.cfg.long_press_ms && still {
                        self.suppress(
                            now_ms,
                            id,
                            "long press suppressed: a global gesture holds the lock",
                        );
                    }
                }
            },
            Phase::Five { first_t, fired: false }
                if now_ms.saturating_sub(first_t) >= self.cfg.five_finger_hold_ms =>
            {
                out.push(GestureIntent::RecoveryMenu);
                self.phase = Phase::Five { first_t, fired: true };
                self.lockout = true;
            },
            _ => {},
        }
        out
    }

    /// Take and clear the suppression log (the input diagnostics view drains this).
    #[must_use]
    pub fn drain_suppressions(&mut self) -> Vec<Suppression> {
        std::mem::take(&mut self.suppressions)
    }

    /// Is a drag currently in fine (second-finger) mode? Surfaces ask this to render the hint.
    #[must_use]
    pub fn fine_mode(&self) -> bool {
        matches!(self.phase, Phase::Dragging { fine: true, .. })
    }

    // ------------------------------------------------------------------ internals

    fn suppress(&mut self, t_ms: u64, pointer: u64, reason: &str) {
        self.suppressions.push(Suppression { t_ms, pointer, reason: reason.to_string() });
    }

    fn live_contacts(&self) -> usize {
        self.tracked.iter().filter(|t| !t.palm).count()
    }

    fn on_down(&mut self, ev: PointerEvent, out: &mut Vec<GestureIntent>) {
        if self.tracked.iter().any(|t| t.id == ev.id) {
            // A duplicate Down for a tracked pointer is a platform hiccup; treat as motion.
            self.on_move(ev, out);
            return;
        }
        let t = Tracked::new(&ev);
        // Palm rejection applies to fingers only: a mouse reports no contact area and a pen's
        // nib is tiny, so testing them against a palm threshold would be meaningless — and
        // rejecting a pen because its driver reports a bogus area would be worse than useless.
        let is_palm =
            ev.kind == PointerKind::Finger && ev.contact_area_mm2 > self.cfg.palm_reject_mm2;
        if is_palm {
            let mut tp = t;
            tp.palm = true;
            self.tracked.push(tp);
            self.suppress(ev.t_ms, ev.id, "palm: contact area exceeds the rejection threshold");
            return;
        }
        self.tracked.push(t);

        match self.live_contacts() {
            1 => {
                self.edge_candidate = self.edge_at(ev.pos).map(|e| (e, ev.pos));
                self.phase =
                    Phase::OneDown { id: ev.id, long_press_fired: false, drag_fired: false };
            },
            2 => match self.phase {
                Phase::Dragging { id, fine: false } => {
                    // Second finger during a drag: the ×10 precision assist (§14.3 rule 5).
                    self.phase = Phase::Dragging { id, fine: true };
                    out.push(GestureIntent::DragFineChanged { active: true });
                },
                Phase::Dragging { .. } => {},
                Phase::OneDown { .. } => {
                    let (c, s, a) = self.two_pointer_geometry();
                    self.phase =
                        Phase::Two { last_center: c, last_span: s, last_angle: a, engaged: false };
                },
                _ => {},
            },
            3 => {
                let center = self.live_centroid();
                self.phase = Phase::Three {
                    first_t: ev.t_ms,
                    start_center: center,
                    moved_beyond_tap: false,
                    fired: false,
                };
            },
            n if n >= 5 => {
                self.phase = Phase::Five { first_t: ev.t_ms, fired: false };
            },
            _ => {},
        }
    }

    fn on_move(&mut self, ev: PointerEvent, out: &mut Vec<GestureIntent>) {
        let Some(idx) = self.tracked.iter().position(|t| t.id == ev.id) else {
            // Motion for an untracked pointer (missed Down, or a palm we chose not to track):
            // explainable, never guessed at.
            self.suppress(ev.t_ms, ev.id, "motion for an untracked pointer (missed Down?)");
            return;
        };
        self.tracked[idx].record(&ev);
        if self.tracked[idx].palm {
            return; // palms are recorded for diagnostics and nothing else
        }

        match self.phase {
            Phase::OneDown { id, long_press_fired, drag_fired } => {
                let t = self.tracked[idx];
                let moved = t.last.distance(t.start);
                // Edge swipe gets first refusal while the candidate is armed.
                if let Some((edge, origin)) = self.edge_candidate {
                    let inward = self.inward_travel(edge, origin, t.last);
                    if inward >= self.cfg.edge_swipe_min_px && !self.lockout {
                        out.push(GestureIntent::TogglePanel { edge });
                        self.edge_candidate = None;
                        self.lockout = true;
                        self.phase = Phase::OneDown { id, long_press_fired, drag_fired: true };
                        return;
                    }
                    if moved >= self.cfg.drag_min_move_px && inward > 0.0 {
                        // Still inside the edge band: wait for the swipe to commit or die.
                        return;
                    }
                    if moved >= self.cfg.drag_min_move_px {
                        self.edge_candidate = None; // moved off-edge: ordinary drag
                    }
                }
                if !drag_fired
                    && !long_press_fired
                    && moved >= self.cfg.drag_min_move_px
                    && !self.lockout
                {
                    out.push(GestureIntent::DragStart { pos: t.start });
                    out.push(GestureIntent::DragUpdate { delta: t.last.sub(t.start), scale: 1.0 });
                    self.phase = Phase::Dragging { id, fine: false };
                } else if long_press_fired && moved >= self.cfg.drag_min_move_px {
                    self.suppress(
                        ev.t_ms,
                        ev.id,
                        "movement after a long press: context owns this contact (drag suppressed)",
                    );
                } else if self.lockout && moved >= self.cfg.drag_min_move_px {
                    self.suppress(
                        ev.t_ms,
                        ev.id,
                        "drag suppressed: a global gesture holds the lock",
                    );
                }
            },
            Phase::Dragging { id, fine } => {
                let t = self.tracked[idx];
                if t.id == id {
                    let cap = t.hist.len();
                    let delta = if t.hist_len >= 2 {
                        // previous sample = two slots back from the NEXT write slot (see velocity)
                        t.last.sub(t.hist[(t.hist_head + cap - 2) % cap].0)
                    } else {
                        Vec2::ZERO
                    };
                    let scale = if fine { 1.0 / self.cfg.fine_resolution_factor } else { 1.0 };
                    if !self.lockout {
                        out.push(GestureIntent::DragUpdate { delta, scale });
                    }
                }
                // Motion of the *second* finger during fine mode is ignored for dragging —
                // its presence is the signal, not its movement (that would fight the assist).
            },
            Phase::Two { last_center, last_span, last_angle, engaged } => {
                let (center, span, angle) = self.two_pointer_geometry();
                let d_center = center.sub(last_center);
                let moved_enough = d_center.length() >= self.cfg.two_pointer_dead_zone_px
                    || (span - last_span).abs() >= self.cfg.two_pointer_dead_zone_px;
                if moved_enough && !self.lockout {
                    if engaged || d_center.length() >= self.cfg.two_pointer_dead_zone_px {
                        out.push(GestureIntent::Pan { delta: d_center, center });
                    }
                    // Pinch: only once the span itself is meaningful (token: 40 px minimum).
                    if span.max(last_span) >= self.cfg.pinch_min_span_px && last_span > 1e-3 {
                        let factor = span / last_span;
                        if (factor - 1.0).abs() >= 0.005 {
                            out.push(GestureIntent::Zoom { factor, center });
                        }
                    }
                    // Rotate: tangential motion, dead-zoned to stop finger jitter spinning things.
                    let d_angle = normalize_angle(angle - last_angle);
                    if engaged && d_angle.abs() >= 0.02 {
                        out.push(GestureIntent::Rotate { delta_rad: d_angle, center });
                    }
                    self.phase = Phase::Two {
                        last_center: center,
                        last_span: span,
                        last_angle: angle,
                        engaged: true,
                    };
                }
            },
            Phase::Three { first_t, start_center, moved_beyond_tap, fired } if !fired => {
                let center = self.live_centroid();
                let dy = center.y - start_center.y;
                let spread = self
                    .tracked
                    .iter()
                    .filter(|t| !t.palm)
                    .any(|t| t.last.distance(t.start) > self.cfg.tap_max_move_px);
                if dy >= self.cfg.three_finger_swipe_px && !self.lockout {
                    out.push(GestureIntent::AllSoundOff);
                    self.phase =
                        Phase::Three { first_t, start_center, moved_beyond_tap, fired: true };
                    self.lockout = true;
                } else if spread {
                    self.phase =
                        Phase::Three { first_t, start_center, moved_beyond_tap: true, fired };
                }
            },
            _ => {},
        }
    }

    fn on_up(&mut self, ev: PointerEvent, cancelled: bool, out: &mut Vec<GestureIntent>) {
        let Some(idx) = self.tracked.iter().position(|t| t.id == ev.id) else {
            self.suppress(ev.t_ms, ev.id, "release for an untracked pointer");
            return;
        };
        // The release event carries the finger's FINAL position; record it before reading `t.last`
        // so `DragEnd`/`Activate` report where the finger actually lifted, not the last sampled
        // `Moved` point. On Windows, pointer frames are coalesced, so a wire released over a port
        // can otherwise be dropped at a stale point and miss its 24 px capture. For a tap the
        // release is within the slop anyway, so this only ever sharpens the reported position.
        self.tracked[idx].record(&ev);
        let t = self.tracked[idx];
        self.tracked.remove(idx);

        if t.palm {
            if self.tracked.iter().all(|x| x.palm) || self.tracked.is_empty() {
                self.phase = Phase::Idle;
            }
            return;
        }

        match self.phase {
            Phase::OneDown { id, long_press_fired, drag_fired } if id == ev.id => {
                if !cancelled && !drag_fired && !long_press_fired && !self.lockout {
                    let dist = t.last.distance(t.start);
                    let dur = ev.t_ms.saturating_sub(t.start_t);
                    if dist < self.cfg.tap_max_move_px && dur < self.cfg.long_press_ms {
                        out.push(GestureIntent::Activate { pos: t.last });
                        if let Some((prev_pos, prev_t)) = self.last_tap {
                            if ev.t_ms.saturating_sub(prev_t) <= self.cfg.double_tap_ms
                                && t.last.distance(prev_pos) < self.cfg.tap_max_move_px * 2.0
                            {
                                out.push(GestureIntent::DoubleTap { pos: t.last });
                                self.last_tap = None;
                            } else {
                                self.last_tap = Some((t.last, ev.t_ms));
                            }
                        } else {
                            self.last_tap = Some((t.last, ev.t_ms));
                        }
                    } else if dist < self.cfg.tap_max_move_px {
                        self.suppress(ev.t_ms, ev.id, "press too slow to be a tap and too still to be a drag (long-press window passed)");
                    }
                } else if drag_fired && !self.lockout {
                    // It was an edge-swipe drag; the intent already fired.
                } else if self.lockout {
                    self.suppress(ev.t_ms, ev.id, "release while a global gesture holds the lock");
                }
                if long_press_fired {
                    self.suppress(
                        ev.t_ms,
                        ev.id,
                        "release after long press: context already owns it (no activation)",
                    );
                }
                self.finish_all_up(ev.t_ms, out);
            },
            Phase::Dragging { id, fine } => {
                if id == ev.id {
                    if !cancelled && !self.lockout {
                        let (dir, v) = t.velocity(self.cfg.flick_window_ms);
                        if v >= self.cfg.flick_min_velocity {
                            out.push(GestureIntent::Flick { dir, velocity_px_per_ms: v });
                        }
                    } else if cancelled {
                        self.suppress(
                            ev.t_ms,
                            ev.id,
                            "drag cancelled by the system: no flick, no commit",
                        );
                    }
                    out.push(GestureIntent::DragEnd { pos: t.last, cancelled });
                    if fine {
                        out.push(GestureIntent::DragFineChanged { active: false });
                    }
                }
                // The second finger lifting returns the drag to 1:1 (handled in the count logic).
                self.recount_after_up(ev.t_ms, out);
            },
            Phase::Two { .. } => {
                self.recount_after_up(ev.t_ms, out);
            },
            Phase::Three { first_t, fired, .. } => {
                if self.live_contacts() == 0 {
                    let stayed_put = !self.phase_three_moved();
                    if !fired
                        && !cancelled
                        && !self.lockout
                        && stayed_put
                        && ev.t_ms.saturating_sub(first_t) <= self.cfg.three_finger_tap_ms
                    {
                        out.push(GestureIntent::Undo);
                        self.lockout = true;
                    }
                    self.finish_all_up(ev.t_ms, out);
                }
            },
            Phase::Five { fired, .. } => {
                if self.live_contacts() == 0 {
                    if !fired && !cancelled {
                        self.suppress(
                            ev.t_ms,
                            ev.id,
                            "five-finger hold released before the recovery threshold",
                        );
                    }
                    self.finish_all_up(ev.t_ms, out);
                }
            },
            // A OneDown whose guard did not match (a non-primary contact lifting — palms are
            // filtered earlier, so this is the belt-and-braces arm) and the Idle case: nothing
            // to decide, the phase logic above already ran or never applied.
            Phase::OneDown { .. } | Phase::Idle => {},
        }
    }

    /// After a lift in a multi-pointer phase: fall back to the right smaller phase.
    fn recount_after_up(&mut self, t_ms: u64, out: &mut Vec<GestureIntent>) {
        match self.live_contacts() {
            0 => self.finish_all_up(t_ms, out),
            1 => {
                let id = self.tracked.iter().find(|x| !x.palm).map(|x| x.id).unwrap_or(0);
                let was_fine = matches!(self.phase, Phase::Dragging { fine: true, .. });
                let was_drag = matches!(self.phase, Phase::Dragging { .. });
                if was_fine {
                    out.push(GestureIntent::DragFineChanged { active: false });
                }
                if was_drag && !was_fine {
                    self.suppress(t_ms, id, "drag primary lifted with a contact remaining: the remainder is inert until it lifts (no surprise drags)");
                }
                self.phase = if was_drag {
                    Phase::Dragging { id, fine: false }
                } else {
                    Phase::OneDown { id, long_press_fired: false, drag_fired: false }
                };
            },
            2 => {
                let (c, s, a) = self.two_pointer_geometry();
                self.phase =
                    Phase::Two { last_center: c, last_span: s, last_angle: a, engaged: false };
            },
            _ => {},
        }
    }

    /// Did the three-finger sequence move beyond a tap? (Read at release, from the phase.)
    fn phase_three_moved(&self) -> bool {
        matches!(self.phase, Phase::Three { moved_beyond_tap: true, .. })
    }

    /// All pointers are up: reset the sequence state.
    fn finish_all_up(&mut self, _t_ms: u64, _out: &mut Vec<GestureIntent>) {
        if self.live_contacts() == 0 {
            self.phase = Phase::Idle;
            self.lockout = false;
            self.edge_candidate = None;
        }
    }

    /// Centre/span/angle of the two most recent live contacts (or all, when more are down).
    fn two_pointer_geometry(&self) -> (Vec2, f32, f32) {
        let live: Vec<&Tracked> = self.tracked.iter().filter(|t| !t.palm).collect();
        if live.len() < 2 {
            let c = live.first().map_or(Vec2::ZERO, |t| t.last);
            return (c, 0.0, 0.0);
        }
        let (a, b) = (live[live.len() - 2], live[live.len() - 1]);
        let center = Vec2::new((a.last.x + b.last.x) / 2.0, (a.last.y + b.last.y) / 2.0);
        let d = b.last.sub(a.last);
        (center, d.length(), d.y.atan2(d.x))
    }

    fn live_centroid(&self) -> Vec2 {
        let live: Vec<&Tracked> = self.tracked.iter().filter(|t| !t.palm).collect();
        if live.is_empty() {
            return Vec2::ZERO;
        }
        let sum = live.iter().fold(Vec2::ZERO, |acc, t| acc.add(t.last));
        sum.scale(1.0 / live.len() as f32)
    }

    /// Which edge (if any) a down position arms an edge swipe on.
    fn edge_at(&self, p: Vec2) -> Option<Edge> {
        let m = self.cfg.edge_margin_px;
        let v = self.viewport;
        if p.x - v.min.x <= m {
            Some(Edge::Left)
        } else if v.max.x - p.x <= m {
            Some(Edge::Right)
        } else if p.y - v.min.y <= m {
            Some(Edge::Top)
        } else if v.max.y - p.y <= m {
            Some(Edge::Bottom)
        } else {
            None
        }
    }

    /// Inward travel from the edge origin, negative when moving away.
    fn inward_travel(&self, edge: Edge, origin: Vec2, now: Vec2) -> f32 {
        match edge {
            Edge::Left => now.x - origin.x,
            Edge::Right => origin.x - now.x,
            Edge::Top => now.y - origin.y,
            Edge::Bottom => origin.y - now.y,
        }
    }
}

fn normalize_angle(a: f32) -> f32 {
    let mut x = a;
    while x > std::f32::consts::PI {
        x -= std::f32::consts::TAU;
    }
    while x < -std::f32::consts::PI {
        x += std::f32::consts::TAU;
    }
    x
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;

    /// Finger contact helper: area 20 mm² (well under the palm threshold), pressure 1.
    fn f(id: u64, x: f32, y: f32, phase: PointerPhase, t: u64) -> PointerEvent {
        PointerEvent {
            id,
            pos: Vec2::new(x, y),
            pressure: 1.0,
            contact_area_mm2: 20.0,
            kind: PointerKind::Finger,
            phase,
            t_ms: t,
        }
    }

    fn down(r: &mut GestureRecognizer, id: u64, x: f32, y: f32, t: u64) -> Vec<GestureIntent> {
        r.push(f(id, x, y, PointerPhase::Down, t))
    }
    fn mov(r: &mut GestureRecognizer, id: u64, x: f32, y: f32, t: u64) -> Vec<GestureIntent> {
        r.push(f(id, x, y, PointerPhase::Moved, t))
    }
    fn up(r: &mut GestureRecognizer, id: u64, x: f32, y: f32, t: u64) -> Vec<GestureIntent> {
        r.push(f(id, x, y, PointerPhase::Up, t))
    }

    fn intents(v: &[GestureIntent]) -> Vec<&'static str> {
        v.iter()
            .map(|i| match i {
                GestureIntent::Activate { .. } => "activate",
                GestureIntent::DoubleTap { .. } => "double",
                GestureIntent::Context { .. } => "context",
                GestureIntent::DragStart { .. } => "drag-start",
                GestureIntent::DragUpdate { .. } => "drag-update",
                GestureIntent::DragFineChanged { .. } => "fine",
                GestureIntent::DragEnd { .. } => "drag-end",
                GestureIntent::Pan { .. } => "pan",
                GestureIntent::Zoom { .. } => "zoom",
                GestureIntent::Rotate { .. } => "rotate",
                GestureIntent::Undo => "undo",
                GestureIntent::AllSoundOff => "panic",
                GestureIntent::RecoveryMenu => "recovery",
                GestureIntent::Flick { .. } => "flick",
                GestureIntent::TogglePanel { .. } => "panel",
            })
            .collect()
    }

    #[test]
    fn tap_activates_exactly_once() {
        let mut r = GestureRecognizer::from_tokens();
        assert!(down(&mut r, 1, 100.0, 100.0, 0).is_empty());
        let out = up(&mut r, 1, 101.0, 100.0, 120); // 1 px drift, 120 ms
        assert_eq!(intents(&out), ["activate"]);
        // a second, unrelated tap later is a tap, not a double
        down(&mut r, 2, 400.0, 400.0, 2000);
        let out = up(&mut r, 2, 400.0, 400.0, 2100);
        assert_eq!(intents(&out), ["activate"]);
    }

    #[test]
    fn double_tap_emits_both_taps_and_the_double() {
        // Documented Phase 0 semantics: the first Activate is NOT withheld for 300 ms waiting
        // to see whether a double follows — a touch UI that delays every tap feels broken. The
        // double arrives as an additional intent; canvas surfaces bind it to zoom-to-fit.
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        up(&mut r, 1, 100.0, 100.0, 80);
        down(&mut r, 1, 102.0, 100.0, 200);
        let out = up(&mut r, 1, 102.0, 100.0, 260);
        assert!(intents(&out).contains(&"activate"));
        assert!(intents(&out).contains(&"double"), "second tap inside 300 ms is a double: {out:?}");
        // and a third tap far later starts over
        down(&mut r, 1, 102.0, 100.0, 5000);
        let out = up(&mut r, 1, 102.0, 100.0, 5080);
        assert!(!intents(&out).contains(&"double"));
    }

    #[test]
    fn long_press_fires_context_and_never_a_drag() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 50.0, 50.0, 0);
        assert!(
            r.advance(360).iter().any(|i| matches!(i, GestureIntent::Context { .. })),
            "350 ms hold with <8 px movement must fire Context"
        );
        // movement AFTER the context fires must not become a drag (acceptance criterion)
        let out = mov(&mut r, 1, 90.0, 50.0, 400);
        assert!(!intents(&out).contains(&"drag-start"), "context owns the contact: {out:?}");
        let out = up(&mut r, 1, 90.0, 50.0, 450);
        assert!(!intents(&out).contains(&"activate"), "a long-pressed contact never activates");
        // ...and the suppression log explains the dead input
        let s = r.drain_suppressions();
        assert!(s.iter().any(|x| x.reason.contains("context owns this contact")), "{s:?}");
    }

    #[test]
    fn drag_never_fires_long_press() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 50.0, 50.0, 0);
        let out = mov(&mut r, 1, 62.0, 50.0, 100); // 12 px > 8 px threshold, before 350 ms
        assert!(intents(&out).contains(&"drag-start"));
        // hold past the long-press time WHILE dragging: no context may fire
        mov(&mut r, 1, 70.0, 50.0, 300);
        assert!(r.advance(900).is_empty(), "a dragging contact cannot become a long press");
        let out = up(&mut r, 1, 70.0, 50.0, 950);
        assert!(intents(&out).contains(&"drag-end"));
    }

    #[test]
    fn second_finger_during_drag_engages_ten_x_fine() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        mov(&mut r, 1, 120.0, 100.0, 50); // drag started, scale 1.0
        let out = down(&mut r, 2, 300.0, 300.0, 100);
        assert!(intents(&out).contains(&"fine"), "second finger announces fine mode");
        let out = mov(&mut r, 1, 130.0, 100.0, 150);
        let upd = out.iter().find_map(|i| match i {
            GestureIntent::DragUpdate { scale, delta } => Some((*scale, delta.x)),
            _ => None,
        });
        let Some((scale, dx)) = upd else {
            panic!("fine-mode drag must emit DragUpdate, got {:?}", intents(&out));
        };
        assert!(
            (scale - 0.1).abs() < 1e-6,
            "scale must be 1/fine_resolution_factor = 0.1, got {scale}"
        );
        assert!((dx - 10.0).abs() < 1e-3, "delta stays in real px; the SURFACE scales it: {dx}");
        // second finger lifts → back to 1:1
        let out = up(&mut r, 2, 300.0, 300.0, 200);
        assert!(intents(&out).contains(&"fine"));
        assert!(!r.fine_mode());
        let out = mov(&mut r, 1, 140.0, 100.0, 250);
        let scale = out.iter().find_map(|i| match i {
            GestureIntent::DragUpdate { scale, .. } => Some(*scale),
            _ => None,
        });
        assert_eq!(scale, Some(1.0));
    }

    #[test]
    fn two_finger_motion_pans() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        down(&mut r, 2, 200.0, 200.0, 10);
        let mut out = mov(&mut r, 1, 130.0, 110.0, 60); // motion begins with finger 1
        out.extend(mov(&mut r, 2, 230.0, 210.0, 70));
        out.extend(mov(&mut r, 1, 150.0, 120.0, 120));
        assert!(
            out.iter().any(|i| matches!(i, GestureIntent::Pan { .. })),
            "two pointers moving together must pan: {:?}",
            intents(&out)
        );
        // The pan carries WHERE it happens (increment 5): a surface scrolls itself when the
        // gesture is over it, so the centre is part of the intent's meaning, not a diagnostic.
        // It is the midpoint of the two live contacts, at the moment of the emission.
        for i in &out {
            if let GestureIntent::Pan { center, .. } = i {
                assert!(
                    center.x > 100.0 && center.x < 240.0 && center.y > 100.0 && center.y < 220.0,
                    "the centre sits between the two contacts: {center:?}"
                );
            }
        }
    }

    #[test]
    fn pinch_zooms_with_incremental_factor_and_center() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 300.0, 0);
        down(&mut r, 2, 150.0, 300.0, 10); // span 50 ≥ pinch_min 40
        let out = mov(&mut r, 2, 200.0, 300.0, 60); // span 50 → 100
        let zoom = out.iter().find_map(|i| match i {
            GestureIntent::Zoom { factor, center } => Some((*factor, *center)),
            _ => None,
        });
        let Some((factor, center)) = zoom else {
            panic!("a span doubling must emit Zoom, got {:?}", intents(&out));
        };
        assert!((factor - 2.0).abs() < 0.01, "incremental factor = span ratio = 2.0, got {factor}");
        assert!((center.x - 150.0).abs() < 0.01, "centre is the midpoint: {center:?}");
    }

    #[test]
    fn tiny_spans_do_not_zoom() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        down(&mut r, 2, 120.0, 100.0, 10); // span 20 < 40 px minimum
        let out = mov(&mut r, 2, 125.0, 100.0, 60); // span 25 — still tiny
        assert!(
            !intents(&out).contains(&"zoom"),
            "pinch below the token span minimum is noise, not zoom"
        );
    }

    #[test]
    fn three_finger_quick_tap_is_undo() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        down(&mut r, 2, 150.0, 100.0, 20);
        down(&mut r, 3, 200.0, 100.0, 40);
        up(&mut r, 1, 100.0, 100.0, 90);
        up(&mut r, 2, 150.0, 100.0, 110);
        let out = up(&mut r, 3, 200.0, 100.0, 130); // all up within 200 ms of first down
        assert!(intents(&out).contains(&"undo"), "{out:?}");
    }

    #[test]
    fn three_finger_swipe_down_is_all_sound_off() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        down(&mut r, 2, 150.0, 100.0, 20);
        down(&mut r, 3, 200.0, 100.0, 40);
        // The centroid crosses the threshold partway through the swipe — on whichever finger's
        // event that happens — so accumulate every event's intents, as a consumer would.
        let mut all = mov(&mut r, 1, 100.0, 160.0, 100); // finger 1: centroid y 100 -> 120
        all.extend(mov(&mut r, 2, 150.0, 160.0, 110)); // centroid y -> 140 = threshold crossed
        all.extend(mov(&mut r, 3, 200.0, 160.0, 120));
        assert!(
            all.iter().any(|i| matches!(i, GestureIntent::AllSoundOff)),
            "panic gesture must fire: {:?}",
            intents(&all)
        );
        // and it must NOT also fire Undo on release (the lockout + fired flag)
        up(&mut r, 1, 100.0, 160.0, 150);
        up(&mut r, 2, 150.0, 160.0, 160);
        let out = up(&mut r, 3, 200.0, 160.0, 170);
        assert!(!intents(&out).contains(&"undo"));
    }

    #[test]
    fn five_finger_hold_opens_recovery() {
        let mut r = GestureRecognizer::from_tokens();
        for i in 1..=5_u64 {
            down(&mut r, i, 80.0 * i as f32, 200.0, i * 10);
        }
        assert!(r.advance(500).is_empty(), "500 ms is under the 800 ms hold");
        let out = r.advance(900);
        assert!(intents(&out).contains(&"recovery"), "{out:?}");
    }

    #[test]
    fn a_palm_is_recorded_and_never_gestured_with() {
        let mut r = GestureRecognizer::from_tokens();
        // 150 mm² heel-of-hand contact while drawing a wire with finger 1
        down(&mut r, 1, 100.0, 100.0, 0);
        mov(&mut r, 1, 120.0, 100.0, 50); // drag started
        let mut palm = f(9, 400.0, 400.0, PointerPhase::Down, 60);
        palm.contact_area_mm2 = 150.0;
        assert!(r.push(palm).is_empty(), "the palm produces no intents");
        let out = mov(&mut r, 9, 430.0, 430.0, 100);
        assert!(out.is_empty(), "palm movement is invisible to gestures");
        // the drag is unaffected: still 1:1 scale, no fine mode
        let out = mov(&mut r, 1, 140.0, 100.0, 120);
        let scale = out.iter().find_map(|i| match i {
            GestureIntent::DragUpdate { scale, .. } => Some(*scale),
            _ => None,
        });
        assert_eq!(scale, Some(1.0), "a palm is not the second finger");
        // ...and the log says why the palm did nothing
        assert!(r.drain_suppressions().iter().any(|s| s.reason.starts_with("palm")));
    }

    #[test]
    fn fast_release_is_a_flick_with_direction_and_speed() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        mov(&mut r, 1, 140.0, 100.0, 20); // 40 px in 20 ms = 2 px/ms > 1.5
        let out = up(&mut r, 1, 140.0, 100.0, 25);
        let flick = out.iter().find_map(|i| match i {
            GestureIntent::Flick { dir, velocity_px_per_ms } => Some((*dir, *velocity_px_per_ms)),
            _ => None,
        });
        let Some((dir, v)) = flick else {
            panic!("fast release must flick, got {:?}", intents(&out));
        };
        assert!(dir.x > 0.9 && dir.y.abs() < 0.1, "direction is +x: {dir:?}");
        assert!(v >= 1.5, "velocity {v} must clear the threshold");
        assert!(intents(&out).contains(&"drag-end"), "the drag still ends cleanly");
    }

    #[test]
    fn slow_release_is_not_a_flick() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        mov(&mut r, 1, 110.0, 100.0, 300); // 10 px in 300 ms: a deliberate drag
        let out = up(&mut r, 1, 110.0, 100.0, 320);
        assert!(!intents(&out).contains(&"flick"));
    }

    #[test]
    fn edge_swipe_toggles_the_panel_and_suppresses_the_drag() {
        let mut r = GestureRecognizer::from_tokens();
        r.set_viewport(Rect::new(Vec2::ZERO, Vec2::new(1280.0, 800.0)));
        down(&mut r, 1, 8.0, 400.0, 0); // within 20 px of the left edge
        let out = mov(&mut r, 1, 30.0, 400.0, 50);
        assert!(
            !intents(&out).contains(&"drag-start"),
            "inside the edge band, a drag does not start"
        );
        let out = mov(&mut r, 1, 60.0, 400.0, 100); // 52 px inward > 40 px threshold
        let panel = out.iter().find_map(|i| match i {
            GestureIntent::TogglePanel { edge } => Some(*edge),
            _ => None,
        });
        assert_eq!(panel, Some(Edge::Left));
        // further movement is locked out until lift
        let out = mov(&mut r, 1, 200.0, 400.0, 150);
        assert!(!intents(&out).contains(&"drag-start"));
    }

    #[test]
    fn cancel_cleans_up_without_committing() {
        let mut r = GestureRecognizer::from_tokens();
        down(&mut r, 1, 100.0, 100.0, 0);
        mov(&mut r, 1, 130.0, 100.0, 50); // dragging
        let out = r.push(f(1, 130.0, 100.0, PointerPhase::Cancel, 60));
        let end = out.iter().find_map(|i| match i {
            GestureIntent::DragEnd { cancelled, .. } => Some(*cancelled),
            _ => None,
        });
        assert_eq!(
            end,
            Some(true),
            "surfaces must see the cancel to roll back (e.g. a half-drawn wire)"
        );
        assert!(!intents(&out).contains(&"flick"), "a cancelled drag never flicks");
        // the recogniser is usable again immediately
        down(&mut r, 2, 10.0, 10.0, 100);
        let out = up(&mut r, 2, 10.0, 10.0, 150);
        assert!(intents(&out).contains(&"activate"));
    }

    #[test]
    fn lockout_dies_when_all_pointers_lift() {
        let mut r = GestureRecognizer::from_tokens();
        // fire a global: three-finger tap
        down(&mut r, 1, 100.0, 100.0, 0);
        down(&mut r, 2, 150.0, 100.0, 10);
        down(&mut r, 3, 200.0, 100.0, 20);
        up(&mut r, 1, 100.0, 100.0, 60);
        up(&mut r, 2, 150.0, 100.0, 70);
        up(&mut r, 3, 200.0, 100.0, 80); // Undo + lockout
                                         // a fresh tap after full lift works again
        down(&mut r, 4, 300.0, 300.0, 1000);
        let out = up(&mut r, 4, 300.0, 300.0, 1080);
        assert!(intents(&out).contains(&"activate"), "the lockout must not outlive the gesture");
    }

    #[test]
    fn a_mouse_drag_is_a_drag() {
        // Pointer normalisation proof: the recogniser never asks what produced the pointer.
        let mut r = GestureRecognizer::from_tokens();
        let m = |x: f32, y: f32, phase: PointerPhase, t: u64| PointerEvent {
            id: 0,
            pos: Vec2::new(x, y),
            pressure: 1.0,
            contact_area_mm2: 0.0,
            kind: PointerKind::Mouse,
            phase,
            t_ms: t,
        };
        r.push(m(50.0, 50.0, PointerPhase::Down, 0));
        let out = r.push(m(70.0, 50.0, PointerPhase::Moved, 40));
        assert!(intents(&out).contains(&"drag-start"));
    }
}
