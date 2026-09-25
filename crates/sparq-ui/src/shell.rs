//! The shell layout: rail / canvas / inspector / dock, computed — not drawn (WO-012 task 3,
//! plan §14.2, `layout.toml [shell] [breakpoint]`).
//!
//! One pure function of (viewport, state) → panel rects. Every consumer — the egui adapter now,
//! the custom renderer in Phase 6, the layout audit, the headless CI gate — computes against
//! *this*, so "the layout is the same object, re-flowed" is a property of the code and not of
//! two implementations agreeing by luck.
//!
//! All values are logical px from the generated tokens. The DPI transform lives in the shell
//! adapter; nothing here knows the scale factor exists.
//!
//! # Reflow rules (§14.2, in the order the tokens state them)
//!
//! 1. Below `tablet_min` (1280×800 logical), **Design mode is refused** (`design_refused`);
//!    Perform mode still runs, reflowed. The refusal is a returned fact, never a silent squeeze.
//! 2. The canvas always keeps **≥ 60 % of the viewport width**. When panels would starve it,
//!    the rail and the inspector collapse first (in that order) and each forced collapse is
//!    reported in [`ShellLayout::forced`] — a reflow the user did not ask for must be visible.
//! 3. User-resizable dimensions (inspector width, dock height) clamp to their token ranges and
//!    snap to the 8 px grid.

use crate::geom::{Rect, Vec2};
use crate::tokens::{
    LAYOUT_BREAKPOINT_TABLET_MIN_H, LAYOUT_BREAKPOINT_TABLET_MIN_W, LAYOUT_CANVAS_SNAP,
    LAYOUT_SHELL_DOCK_HEIGHT, LAYOUT_SHELL_DOCK_HEIGHT_MAX, LAYOUT_SHELL_DOCK_HEIGHT_MIN,
    LAYOUT_SHELL_INSPECTOR_WIDTH, LAYOUT_SHELL_INSPECTOR_WIDTH_MAX,
    LAYOUT_SHELL_INSPECTOR_WIDTH_MIN, LAYOUT_SHELL_RAIL_WIDTH, LAYOUT_SHELL_TOP_BAR_HEIGHT,
};

/// Which application mode the shell is in. The layout differs (Perform hides Design chrome);
/// the *audit* differs more (see `audit.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellMode {
    /// Patching, browsing, wiring: dense chrome allowed.
    Design,
    /// Playing: nothing below touch class L is interactive (plan §14.3).
    Perform,
}

/// The user-controllable part of the shell state. Everything else is derived.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellState {
    /// Current mode.
    pub mode: ShellMode,
    /// The left rail is hidden.
    pub rail_collapsed: bool,
    /// The right inspector is hidden.
    pub inspector_collapsed: bool,
    /// The bottom dock is hidden.
    pub dock_collapsed: bool,
    /// Requested inspector width (clamped + snapped by `compute`).
    pub inspector_width: f32,
    /// Requested dock height (clamped + snapped by `compute`).
    pub dock_height: f32,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            mode: ShellMode::Design,
            rail_collapsed: false,
            inspector_collapsed: false,
            dock_collapsed: false,
            inspector_width: LAYOUT_SHELL_INSPECTOR_WIDTH as f32,
            dock_height: LAYOUT_SHELL_DOCK_HEIGHT as f32,
        }
    }
}

/// The computed layout: where every panel lives, and what had to be forced to get there.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellLayout {
    /// Always present: transport, mode, diagnostics live here (§14.2).
    pub top_bar: Rect,
    /// Left rail; `None` when collapsed (by the user or by reflow).
    pub rail: Option<Rect>,
    /// The canvas — the only panel that never disappears.
    pub canvas: Rect,
    /// Right inspector; `None` when collapsed.
    pub inspector: Option<Rect>,
    /// Bottom dock; `None` when collapsed.
    pub dock: Option<Rect>,
    /// Viewport is below `tablet_min` and the mode is Design: the app must refuse Design mode
    /// (and offer Perform). Reported, never silently ignored.
    pub design_refused: bool,
    /// Reflows the layout forced (panel collapse the user did not request), for diagnostics.
    pub forced: Vec<ForcedCollapse>,
}

/// A panel the reflow rules collapsed to protect the canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ForcedCollapse {
    /// The inspector gave up its width.
    Inspector,
    /// The rail gave up its width.
    Rail,
    /// The dock gave up its height (extreme aspect ratios).
    Dock,
}

/// The canvas-width floor, as a fraction (§14.2: "the canvas always keeps >= 60 % of the width").
pub const CANVAS_MIN_WIDTH_FRACTION: f32 = 0.60;

/// Compute the shell layout for a viewport (logical px, origin at the viewport's min corner).
///
/// Pure and total: any viewport, any state, no allocation beyond the (usually empty) `forced`
/// list. Deterministic — the headless audit runs this at the whole breakpoint matrix in CI.
#[must_use]
pub fn compute(viewport: Rect, state: &ShellState) -> ShellLayout {
    let grid = LAYOUT_CANVAS_SNAP as f32;
    let top_h = LAYOUT_SHELL_TOP_BAR_HEIGHT as f32;

    let mut rail_w = if state.rail_collapsed { 0.0 } else { LAYOUT_SHELL_RAIL_WIDTH as f32 };
    // Clamping is unconditional; SNAPPING applies to user-resized values only. The token
    // defaults are the designed numbers and win verbatim — the dock default (260) is not on the
    // 8 px grid, and rounding a token to fit a rule would quietly redesign the shell. (The rule
    // exists so *drags* land on the grid, not so tokens get rewritten.)
    let snap_or_default = |v: f32, default: f32| {
        if (v - default).abs() < 0.001 {
            default
        } else {
            (v / grid).round() * grid
        }
    };
    let mut insp_w = if state.inspector_collapsed {
        0.0
    } else {
        snap_or_default(
            state.inspector_width.clamp(
                LAYOUT_SHELL_INSPECTOR_WIDTH_MIN as f32,
                LAYOUT_SHELL_INSPECTOR_WIDTH_MAX as f32,
            ),
            LAYOUT_SHELL_INSPECTOR_WIDTH as f32,
        )
    };
    let mut dock_h = if state.dock_collapsed {
        0.0
    } else {
        snap_or_default(
            state
                .dock_height
                .clamp(LAYOUT_SHELL_DOCK_HEIGHT_MIN as f32, LAYOUT_SHELL_DOCK_HEIGHT_MAX as f32),
            LAYOUT_SHELL_DOCK_HEIGHT as f32,
        )
    };

    let vw = viewport.width();
    let vh = viewport.height();
    let mut forced = Vec::new();

    // Canvas-width floor: collapse the inspector first, then the rail, then the dock.
    // (Order from §14.2: "Rail and inspector collapse first"; between the two width panels the
    // inspector goes first because it is the widest and the rail carries the transport.)
    let floor = vw * CANVAS_MIN_WIDTH_FRACTION;
    if vw - rail_w - insp_w < floor && insp_w > 0.0 {
        insp_w = 0.0;
        if !state.inspector_collapsed {
            forced.push(ForcedCollapse::Inspector);
        }
    }
    if vw - rail_w - insp_w < floor && rail_w > 0.0 {
        rail_w = 0.0;
        if !state.rail_collapsed {
            forced.push(ForcedCollapse::Rail);
        }
    }
    // Degenerate viewports (shorter than top bar + minimum dock): the dock yields too.
    if vh - top_h - dock_h < 1.0 && dock_h > 0.0 {
        dock_h = 0.0;
        if !state.dock_collapsed {
            forced.push(ForcedCollapse::Dock);
        }
    }

    let x0 = viewport.min.x;
    let y0 = viewport.min.y;

    let top_bar = Rect::new(Vec2::new(x0, y0), Vec2::new(x0 + vw, y0 + top_h));
    let mid_y0 = y0 + top_h;
    let mid_h = (vh - top_h - dock_h).max(0.0);

    let rail = (rail_w > 0.0)
        .then(|| Rect::new(Vec2::new(x0, mid_y0), Vec2::new(x0 + rail_w, mid_y0 + mid_h)));
    let canvas_x0 = x0 + rail_w;
    let canvas_x1 = x0 + vw - insp_w;
    let canvas = Rect::new(
        Vec2::new(canvas_x0, mid_y0),
        Vec2::new(canvas_x1.max(canvas_x0), mid_y0 + mid_h),
    );
    let inspector = (insp_w > 0.0).then(|| {
        Rect::new(Vec2::new(x0 + vw - insp_w, mid_y0), Vec2::new(x0 + vw, mid_y0 + mid_h))
    });
    let dock = (dock_h > 0.0)
        .then(|| Rect::new(Vec2::new(x0, y0 + vh - dock_h), Vec2::new(x0 + vw, y0 + vh)));

    let design_refused = state.mode == ShellMode::Design
        && (vw < LAYOUT_BREAKPOINT_TABLET_MIN_W as f32
            || vh < LAYOUT_BREAKPOINT_TABLET_MIN_H as f32);

    ShellLayout { top_bar, rail, canvas, inspector, dock, design_refused, forced }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn vp(w: f32, h: f32) -> Rect {
        Rect::new(Vec2::ZERO, Vec2::new(w, h))
    }

    #[test]
    fn default_desktop_layout_matches_the_tokens_exactly() {
        let l = compute(vp(1920.0, 1080.0), &ShellState::default());
        assert!(!l.design_refused);
        assert!(l.forced.is_empty());
        // top bar: full width, token height
        assert_eq!(l.top_bar.width(), 1920.0);
        assert_eq!(l.top_bar.height(), LAYOUT_SHELL_TOP_BAR_HEIGHT as f32);
        // rail: token width, from below the top bar to above the dock
        let rail = l.rail.unwrap();
        assert_eq!(rail.width(), LAYOUT_SHELL_RAIL_WIDTH as f32);
        assert_eq!(rail.min.y, LAYOUT_SHELL_TOP_BAR_HEIGHT as f32);
        // inspector: token width on the right
        let insp = l.inspector.unwrap();
        assert_eq!(insp.width(), LAYOUT_SHELL_INSPECTOR_WIDTH as f32);
        assert_eq!(insp.max.x, 1920.0);
        // dock: token height at the bottom, full width
        let dock = l.dock.unwrap();
        assert_eq!(dock.height(), LAYOUT_SHELL_DOCK_HEIGHT as f32);
        assert_eq!(dock.max.y, 1080.0);
        // canvas: what remains, and panels never overlap it
        assert_eq!(l.canvas.min.x, rail.max.x);
        assert_eq!(l.canvas.max.x, insp.min.x);
        assert_eq!(l.canvas.min.y, l.top_bar.max.y);
        assert_eq!(l.canvas.max.y, dock.min.y);
    }

    #[test]
    fn canvas_keeps_at_least_sixty_percent_of_the_width() {
        // 1024 px wide: rail 56 + inspector 400 = 456 chrome → canvas 568 = 55 % → the rules
        // must collapse the inspector (first) to restore the floor.
        let l = compute(vp(1024.0, 900.0), &ShellState::default());
        assert!(l.canvas.width() >= 1024.0 * CANVAS_MIN_WIDTH_FRACTION - 0.001);
        assert!(l.inspector.is_none(), "inspector collapses first");
        assert!(
            l.forced.contains(&ForcedCollapse::Inspector),
            "and the forced collapse is reported"
        );
        assert!(l.rail.is_some(), "the rail survives: 56 px does not break the floor here");
    }

    #[test]
    fn extreme_narrow_viewports_collapse_in_the_documented_order() {
        // 600 px: floor 360; even rail+nothing = 544 canvas... wait 600-56=544 ≥ 360, so only the
        // inspector goes. Push to 400: 400-56=344 < 240? floor=240 → rail survives (344≥240).
        // At 300: floor=180; 300-56=244 ≥ 180 → rail still survives. At 200: floor=120;
        // 200-56=144 ≥ 120 → survives. At 120: floor=72; 120-56=64 < 72 → rail collapses too.
        let l = compute(vp(120.0, 800.0), &ShellState::default());
        assert!(l.rail.is_none() && l.inspector.is_none());
        assert_eq!(l.forced, vec![ForcedCollapse::Inspector, ForcedCollapse::Rail]);
        assert!(l.canvas.width() >= 120.0 * CANVAS_MIN_WIDTH_FRACTION - 0.001);
    }

    #[test]
    fn design_mode_is_refused_below_the_tablet_breakpoint_and_perform_is_not() {
        let small = vp(1024.0, 700.0);
        let design = compute(small, &ShellState::default());
        assert!(design.design_refused, "below tablet_min, Design must be refused — loudly");
        let perform =
            compute(small, &ShellState { mode: ShellMode::Perform, ..ShellState::default() });
        assert!(!perform.design_refused, "Perform still runs, reflowed");
        // exactly at the breakpoint is fine
        assert!(!compute(vp(1280.0, 800.0), &ShellState::default()).design_refused);
    }

    #[test]
    fn resizable_panels_clamp_and_snap_to_the_8px_grid() {
        let s = ShellState { inspector_width: 9999.0, dock_height: 1.0, ..ShellState::default() };
        let l = compute(vp(2560.0, 1440.0), &s);
        assert_eq!(l.inspector.unwrap().width(), LAYOUT_SHELL_INSPECTOR_WIDTH_MAX as f32);
        assert_eq!(l.dock.unwrap().height(), LAYOUT_SHELL_DOCK_HEIGHT_MIN as f32);
        // an off-grid request lands on the grid
        let s = ShellState { inspector_width: 403.0, dock_height: 259.0, ..ShellState::default() };
        let l = compute(vp(2560.0, 1440.0), &s);
        assert_eq!(l.inspector.unwrap().width(), 400.0, "403 snaps to the 8 px grid");
        assert_eq!(l.dock.unwrap().height(), 256.0, "259 snaps to the 8 px grid");
        // ...but the token defaults are used verbatim, even off-grid (dock default is 260):
        let l = compute(vp(2560.0, 1440.0), &ShellState::default());
        assert_eq!(l.dock.unwrap().height(), LAYOUT_SHELL_DOCK_HEIGHT as f32);
    }

    #[test]
    fn user_collapses_are_not_reported_as_forced() {
        let s = ShellState { inspector_collapsed: true, ..ShellState::default() };
        let l = compute(vp(1024.0, 900.0), &s);
        assert!(l.inspector.is_none());
        assert!(l.forced.is_empty(), "a collapse the user asked for is not a reflow surprise");
    }

    #[test]
    fn panels_never_overlap() {
        // Sweep a matrix of viewports and states; overlap is the bug class this test exists for.
        for &(w, h) in
            &[(1920.0, 1080.0), (1280.0, 800.0), (1024.0, 700.0), (3840.0, 2160.0), (400.0, 900.0)]
        {
            for rail_c in [false, true] {
                for insp_c in [false, true] {
                    for dock_c in [false, true] {
                        let s = ShellState {
                            rail_collapsed: rail_c,
                            inspector_collapsed: insp_c,
                            dock_collapsed: dock_c,
                            ..ShellState::default()
                        };
                        let l = compute(vp(w, h), &s);
                        let panels: Vec<Rect> = [l.rail, l.inspector, l.dock]
                            .iter()
                            .flatten()
                            .copied()
                            .chain([l.top_bar, l.canvas])
                            .collect();
                        for (i, a) in panels.iter().enumerate() {
                            for b in panels.iter().skip(i + 1) {
                                let overlap = a.min.x < b.max.x
                                    && b.min.x < a.max.x
                                    && a.min.y < b.max.y
                                    && b.min.y < a.max.y;
                                assert!(!overlap, "overlap at {w}x{h} rail={rail_c} insp={insp_c} dock={dock_c}: {a:?} vs {b:?}");
                            }
                        }
                        assert!(
                            l.canvas.width() >= w * CANVAS_MIN_WIDTH_FRACTION - 0.001,
                            "canvas floor violated at {w}x{h} rail={rail_c} insp={insp_c}"
                        );
                    }
                }
            }
        }
    }
}
