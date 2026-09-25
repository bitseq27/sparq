//! The layout audit: every interactive element measured against its declared touch class
//! (WO-012 acceptance: "Every interactive element ≥ 44 px — automated audit passes").
//!
//! This is the same philosophy as the allocation gate and the golden references: a rule that is
//! not measured will be broken, and a rule measured by eyeball will be broken *confidently*.
//! Widget code registers what it drew; the audit fails the build on anything a gloved finger
//! could not hit.
//!
//! Class minimums come from the tokens (`layout.toml [touch]`): S 44 · M 56 · L 72 · XL 96.
//! The Design-mode dense exception (32 px, non-destructive controls only) is *badged*, never
//! silently accepted — and it does not exist in Perform mode, where nothing below class L is
//! interactive at all (input-model §4).

use crate::geom::Rect;
use crate::shell::ShellMode;
use crate::tokens::{
    LAYOUT_TOUCH_MIN_TARGET_DENSE, LAYOUT_TOUCH_TARGET_L, LAYOUT_TOUCH_TARGET_M,
    LAYOUT_TOUCH_TARGET_S, LAYOUT_TOUCH_TARGET_XL,
};

/// The touch class an interactive element declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TouchClass {
    /// 44 px: dense Design-mode controls, list rows, port capture.
    S,
    /// 56 px: inspector sliders, chips, tab items.
    M,
    /// 72 px: canvas node bodies, browser tiles, scene cells.
    L,
    /// 96 px: Perform-mode macros, transport, panic.
    XL,
}

impl TouchClass {
    /// The minimum shorter side, in logical px, from the tokens.
    #[must_use]
    pub const fn min_px(self) -> f32 {
        match self {
            Self::S => LAYOUT_TOUCH_TARGET_S as f32,
            Self::M => LAYOUT_TOUCH_TARGET_M as f32,
            Self::L => LAYOUT_TOUCH_TARGET_L as f32,
            Self::XL => LAYOUT_TOUCH_TARGET_XL as f32,
        }
    }
}

/// One interactive element as drawn, registered by the widget layer for auditing.
#[derive(Clone, Debug)]
pub struct InteractiveElement {
    /// Stable diagnostic id ("rail/transport/play", "inspector/slider/cutoff", …).
    pub id: String,
    /// Declared class.
    pub class: TouchClass,
    /// The rect it actually occupies, logical px.
    pub rect: Rect,
    /// Whether the Design-mode dense exception (32 px floor) may apply. Must be false for
    /// anything destructive — the token says so, and this struct is where that is enforced.
    pub dense_allowed: bool,
}

/// An element that failed its class minimum.
#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
    /// Which element.
    pub id: String,
    /// What it needed.
    pub required: f32,
    /// What it measured (shorter side).
    pub actual: f32,
    /// Why, in words — the report is read by humans under time pressure.
    pub reason: String,
}

/// An element inside the dense exception: legal, but visible.
#[derive(Clone, Debug, PartialEq)]
pub struct DenseBadge {
    /// Which element.
    pub id: String,
    /// Measured size.
    pub actual: f32,
}

/// The audit result.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuditReport {
    /// Elements checked.
    pub checked: usize,
    /// Hard failures.
    pub violations: Vec<Violation>,
    /// Dense-exception passes, badged.
    pub dense_badges: Vec<DenseBadge>,
}

impl AuditReport {
    /// Did everything pass?
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// The gate-table rendering.
    #[must_use]
    pub fn format_report(&self) -> String {
        let mut s = format!("layout audit · {} element(s) checked\n", self.checked);
        for v in &self.violations {
            s.push_str(&format!(
                "  [FAIL] {}: needs {:.0} px (class), measured {:.0} px — {}\n",
                v.id, v.required, v.actual, v.reason
            ));
        }
        for b in &self.dense_badges {
            s.push_str(&format!(
                "  [DENSE] {}: {:.0} px under the Design-mode dense exception (min {:.0})\n",
                b.id, b.actual, LAYOUT_TOUCH_MIN_TARGET_DENSE as f32
            ));
        }
        s.push_str(&format!(
            "layout audit: {} ({} violation(s), {} dense badge(s))\n",
            if self.is_clean() { "PASS" } else { "FAIL" },
            self.violations.len(),
            self.dense_badges.len()
        ));
        s
    }
}

/// Run the audit over every registered interactive element.
///
/// Measurement is the **shorter side**: a 500×30 px strip is not a 500 px target, and pretending
/// otherwise is how touch UIs fail under stage gloves.
#[must_use]
pub fn audit(elements: &[InteractiveElement], mode: ShellMode) -> AuditReport {
    let mut report = AuditReport { checked: elements.len(), ..AuditReport::default() };
    let dense_floor = LAYOUT_TOUCH_MIN_TARGET_DENSE as f32;

    for e in elements {
        let actual = e.rect.min_side();

        // Perform mode: nothing below class L is interactive — its presence is the violation,
        // whatever its size (input-model §4: Design chrome "is not merely hidden — it is not
        // hit-testable").
        if mode == ShellMode::Perform && e.class < TouchClass::L {
            report.violations.push(Violation {
                id: e.id.clone(),
                required: TouchClass::L.min_px(),
                actual,
                reason: format!(
                    "class {:?} is not interactive in Perform mode (nothing below L)",
                    e.class
                ),
            });
            continue;
        }

        let required = e.class.min_px();
        if actual + 1e-3 >= required {
            continue;
        }
        // The dense exception: Design mode, non-destructive, above the dense floor. Badged.
        if mode == ShellMode::Design && e.dense_allowed && actual + 1e-3 >= dense_floor {
            report.dense_badges.push(DenseBadge { id: e.id.clone(), actual });
            continue;
        }
        report.violations.push(Violation {
            id: e.id.clone(),
            required,
            actual,
            reason: if mode == ShellMode::Design && e.dense_allowed {
                String::from("below even the Design-mode dense floor")
            } else {
                String::from("below its class minimum")
            },
        });
    }
    report
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::geom::Vec2;

    fn el(id: &str, class: TouchClass, w: f32, h: f32, dense: bool) -> InteractiveElement {
        InteractiveElement {
            id: id.to_string(),
            class,
            rect: Rect::from_min_size(Vec2::ZERO, Vec2::new(w, h)),
            dense_allowed: dense,
        }
    }

    #[test]
    fn class_minimums_are_the_token_values() {
        assert_eq!(TouchClass::S.min_px(), 44.0);
        assert_eq!(TouchClass::M.min_px(), 56.0);
        assert_eq!(TouchClass::L.min_px(), 72.0);
        assert_eq!(TouchClass::XL.min_px(), 96.0);
    }

    #[test]
    fn the_shorter_side_is_what_counts() {
        // A 500×40 strip declaring class S: 500 px of width does not make it touchable.
        let r = audit(&[el("list/row", TouchClass::S, 500.0, 40.0, false)], ShellMode::Design);
        assert_eq!(r.violations.len(), 1);
        assert_eq!(r.violations[0].actual, 40.0);
        assert_eq!(r.violations[0].required, 44.0);
    }

    #[test]
    fn compliant_elements_pass_silently() {
        let els = [
            el("rail/button", TouchClass::S, 44.0, 44.0, false),
            el("inspector/slider", TouchClass::M, 300.0, 56.0, false),
            el("canvas/node", TouchClass::L, 240.0, 72.0, false),
            el("perform/panic", TouchClass::XL, 160.0, 96.0, false),
        ];
        let r = audit(&els, ShellMode::Design);
        assert!(r.is_clean(), "{}", r.format_report());
        assert_eq!(r.checked, 4);
    }

    #[test]
    fn the_dense_exception_badges_and_never_excuses_perform_mode() {
        let small = el("design/chip", TouchClass::S, 36.0, 36.0, true);
        let design = audit(std::slice::from_ref(&small), ShellMode::Design);
        assert!(design.is_clean());
        assert_eq!(design.dense_badges.len(), 1, "dense is legal in Design...");
        let perform = audit(std::slice::from_ref(&small), ShellMode::Perform);
        assert!(!perform.is_clean(), "...and does not exist in Perform");

        // Below the dense floor it is a violation even in Design.
        let tiny = el("design/pin", TouchClass::S, 24.0, 24.0, true);
        let r = audit(&[tiny], ShellMode::Design);
        assert_eq!(r.violations.len(), 1);
        assert!(r.violations[0].reason.contains("dense floor"));

        // Destructive controls get no exception, at any size above the floor.
        let destructive = el("design/delete", TouchClass::S, 36.0, 36.0, false);
        assert_eq!(audit(&[destructive], ShellMode::Design).violations.len(), 1);
    }

    #[test]
    fn perform_mode_rejects_small_classes_regardless_of_size() {
        // A 56 px class-M chip is perfectly sized — and still not interactive in Perform.
        let chip = el("perform/chip", TouchClass::M, 56.0, 56.0, false);
        let r = audit(&[chip], ShellMode::Perform);
        assert_eq!(r.violations.len(), 1);
        assert!(r.violations[0].reason.contains("Perform mode"));
    }

    #[test]
    fn the_report_names_every_failure_with_its_numbers() {
        let r = audit(&[el("x/y", TouchClass::XL, 80.0, 80.0, false)], ShellMode::Design);
        let text = r.format_report();
        assert!(text.contains("FAIL"));
        assert!(text.contains("x/y"));
        assert!(text.contains("96"), "the required number must be in the report: {text}");
        assert!(text.contains("80"), "the measured number must be in the report: {text}");
    }
}
