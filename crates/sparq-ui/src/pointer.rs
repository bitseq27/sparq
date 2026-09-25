//! Pointer normalisation (WO-012 task 2, `docs/ui/input-model.md` §2).
//!
//! Finger, pen and mouse all become **one pointer model**, because the gesture layer must not
//! care which produced it. Two rules from the input model are load-bearing here:
//!
//! * **Mouse hover is a synthesised zero-pressure pointer that never enters `Down`.** Hover
//!   affordances are therefore an optional enhancement, never a requirement (plan §14.3 rule 2:
//!   no hover-dependence). A touch-first widget that only works with a mouse is a bug.
//! * **Coordinates are logical px at the current DPI scale.** The shell owns the DPI transform;
//!   nothing downstream of it ever sees physical pixels. That is what makes the per-monitor DPI
//!   acceptance criterion a property of one module instead of every widget.
//!
//! `t_ms` is a monotonic millisecond timestamp supplied by the *adapter*, not read here: the
//! gesture layer must stay deterministic and testable, which it cannot be if it reads a clock.

use crate::geom::Vec2;

/// What produced a pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// A finger. Has a contact area; no hover; may be pressure-insensitive.
    Finger,
    /// A pen/stylus. Pressure and (later) tilt are meaningful; has hover.
    Pen,
    /// A mouse or trackpad cursor. Hover is synthesised as a zero-pressure pointer that never
    /// enters [`PointerPhase::Down`] unless a button is held.
    Mouse,
}

impl PointerKind {
    /// Can this pointer hover without contacting? Only hover-capable kinds get hover synthesis.
    #[must_use]
    pub const fn has_hover(self) -> bool {
        matches!(self, Self::Pen | Self::Mouse)
    }
}

/// Where a pointer is in its contact lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerPhase {
    /// Contact began (or a mouse button went down).
    Down,
    /// Contact continues at a new position/pressure.
    Moved,
    /// Contact ended normally.
    Up,
    /// Contact was aborted by the system (gesture stolen, palm detected, window lost focus).
    /// Must be treated as `Up` for state cleanup but must NOT emit activation gestures.
    Cancel,
}

impl PointerPhase {
    /// Is the pointer in contact (counting toward multi-pointer gestures)?
    #[must_use]
    pub const fn is_contact(self) -> bool {
        matches!(self, Self::Down | Self::Moved)
    }
}

/// One normalised pointer sample.
///
/// Deliberately `Copy` and allocation-free: this crosses from the window event loop into the
/// gesture layer, which may run on a thread that must not allocate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerEvent {
    /// Stable per-contact id. Mouse uses a fixed id (0) — a mouse is one pointer that persists.
    pub id: u64,
    /// Position in logical px, y down.
    pub pos: Vec2,
    /// 0.0 (hover/no pressure) ..= 1.0 (max). Mouse buttons report 1.0 while held.
    pub pressure: f32,
    /// Contact area in mm² for touch; 0.0 when unknown. Drives palm rejection
    /// (`layout.toml [touch] gesture.palm_reject_max_contact_area_mm2`).
    pub contact_area_mm2: f32,
    /// What produced this sample.
    pub kind: PointerKind,
    /// Lifecycle phase.
    pub phase: PointerPhase,
    /// Monotonic timestamp in milliseconds, supplied by the adapter.
    pub t_ms: u64,
}

impl PointerEvent {
    /// A touch/pen contact sample.
    #[must_use]
    pub fn contact(
        id: u64,
        pos: Vec2,
        pressure: f32,
        area_mm2: f32,
        kind: PointerKind,
        t_ms: u64,
    ) -> Self {
        Self {
            id,
            pos,
            pressure,
            contact_area_mm2: area_mm2,
            kind,
            phase: PointerPhase::Moved,
            t_ms,
        }
    }

    /// The same sample with a different phase (saves adapters repeating every field).
    #[must_use]
    pub fn with_phase(mut self, phase: PointerPhase) -> Self {
        self.phase = phase;
        self
    }

    /// The same sample with a different pressure.
    #[must_use]
    pub fn pressure(mut self, p: f32) -> Self {
        self.pressure = p;
        self
    }

    /// A mouse/pen hover sample: zero pressure, never `Down` (see module docs).
    #[must_use]
    pub fn hover(pos: Vec2, kind: PointerKind, t_ms: u64) -> Self {
        Self {
            id: 0,
            pos,
            pressure: 0.0,
            contact_area_mm2: 0.0,
            kind,
            phase: PointerPhase::Moved,
            t_ms,
        }
    }

    /// Is this sample a real contact (not hover)?
    #[must_use]
    pub fn is_contact(&self) -> bool {
        self.phase.is_contact() && (self.pressure > 0.0 || !self.kind.has_hover())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_is_never_a_contact() {
        let h = PointerEvent::hover(Vec2::new(5.0, 5.0), PointerKind::Mouse, 0);
        assert!(
            !h.is_contact(),
            "hover must not count as contact — no hover-dependence (§14.3 rule 2)"
        );
        let down = h.with_phase(PointerPhase::Down).pressure(1.0);
        assert!(down.is_contact());
    }

    #[test]
    fn finger_at_zero_pressure_is_still_a_contact() {
        // Many touchscreens report pressure 0 for a resting finger; refusing those would drop
        // real input. Fingers are contact by kind, not by pressure.
        let f = PointerEvent::contact(7, Vec2::ZERO, 0.0, 20.0, PointerKind::Finger, 10)
            .with_phase(PointerPhase::Down);
        assert!(f.is_contact());
    }

    #[test]
    fn cancel_is_contact_cleanup_but_not_activation() {
        assert!(!PointerPhase::Cancel.is_contact(), "cancel is not an active contact");
    }
}
