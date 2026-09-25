//! Token → egui adapter (WO-012 task 3): the ONLY place egui's style types meet sparq's tokens.
//!
//! Acceptance criterion: "an egui style adapter generated from tokens so a token change
//! restyles everything" — and its enforcement half: "no hard-coded colour/size/spacing values
//! in widget code". This module is where that discipline physically lives: every number below is
//! a `sparq_ui::tokens` constant, and widget code (`shell_ui.rs`) reads either this module or the
//! tokens directly. `tools/token_audit.py` R6 scans widget sources for literals; this file and
//! the generated tokens are the only places colour values may appear.
//!
//! When the Phase 6 custom shell replaces egui, this file is deleted and its *content* — the
//! mapping from tokens to concrete style decisions — moves to the new renderer's equivalent. The
//! tokens themselves do not change; that is the point of the whole exercise (ADR-003).

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Stroke, TextStyle, Visuals};
use sparq_ui::geom::{Rect as SpRect, Vec2 as SpVec2};
use sparq_ui::tokens::*;

/// Which colour theme the shell renders. Both are token-generated (`colors.toml`): the HC theme
/// is *derived* — ground/ hairline / text overrides plus a +8 % oklab lightness lift on every
/// accent — by `tools/token_gen.py`, not hand-picked here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    /// The default phosphor-dark theme.
    PhosphorDark,
    /// The high-contrast theme (`[theme.contrast-high]`), for stage lighting and low vision.
    ContrastHigh,
}

impl ThemeChoice {
    /// Flip to the other theme.
    #[must_use]
    pub const fn toggled(self) -> Self {
        match self {
            Self::PhosphorDark => Self::ContrastHigh,
            Self::ContrastHigh => Self::PhosphorDark,
        }
    }
}

/// The resolved palette: every colour the shell may paint with, chosen by theme.
///
/// Widget code receives one of these and reads fields — it never selects constants itself and
/// never sees a hex value. Adding a colour to the shell means adding it here, which means adding
/// it to the TOML first.
#[derive(Clone, Copy, Debug)]
pub struct Palette {
    /// Deepest ground (window/app background behind everything).
    pub ground_base: Color32,
    /// Canvas ground (full-bleed visual area).
    pub ground_canvas: Color32,
    /// Panel ground.
    pub ground_panel: Color32,
    /// Raised panel elements (buttons, chips, headers).
    pub ground_panel_alt: Color32,
    /// Inset ground (fields, wells).
    pub ground_inset: Color32,
    /// Overlay ground (dialogs, menus).
    pub ground_overlay: Color32,
    /// The colour hairlines are drawn in (alpha tiers below modulate it).
    pub hairline_colour: Color32,
    /// Hairline alpha tiers: structure at rest.
    pub hairline_faint: f32,
    /// Hairline alpha tiers: normal borders and dividers.
    pub hairline_regular: f32,
    /// Hairline alpha tiers: emphasised borders, focus.
    pub hairline_strong: f32,
    /// Primary text.
    pub text_primary: Color32,
    /// Secondary text.
    pub text_secondary: Color32,
    /// Tertiary text (placeholders, units).
    pub text_tertiary: Color32,
    /// Disabled text (decorative only — excluded from contrast guarantees by design).
    pub text_disabled: Color32,
    /// Text on a filled accent.
    #[allow(dead_code)] // text on filled accents — used by WO-013 filled controls
    pub text_inverse: Color32,
    /// Signal-class accent: audio.
    pub audio: Color32,
    /// Signal-class accent: CV.
    pub cv: Color32,
    /// Signal-class accent: event.
    #[allow(dead_code)] // signal-class accent — wires and ports, WO-013
    pub event: Color32,
    /// Signal-class accent: data.
    #[allow(dead_code)] // signal-class accent — wires and ports, WO-013
    pub data: Color32,
    /// Signal-class accent: spatial.
    #[allow(dead_code)] // signal-class accent — wires and ports, WO-013
    pub spatial: Color32,
    /// State: playing / OK.
    pub playing: Color32,
    /// State: warning.
    pub warning: Color32,
    /// State: error / over.
    pub error: Color32,
    /// State: selected.
    pub selected: Color32,
}

impl Palette {
    /// Resolve the palette for a theme. Pure token selection — no arithmetic, no literals.
    #[must_use]
    pub fn for_theme(theme: ThemeChoice) -> Self {
        match theme {
            ThemeChoice::PhosphorDark => Self {
                ground_base: c32(&COLOR_GROUND_BASE),
                ground_canvas: c32(&COLOR_GROUND_CANVAS),
                ground_panel: c32(&COLOR_GROUND_PANEL),
                ground_panel_alt: c32(&COLOR_GROUND_PANEL_ALT),
                ground_inset: c32(&COLOR_GROUND_INSET),
                ground_overlay: c32(&COLOR_GROUND_OVERLAY),
                hairline_colour: c32(&COLOR_HAIRLINE_COLOUR),
                hairline_faint: COLOR_HAIRLINE_FAINT,
                hairline_regular: COLOR_HAIRLINE_REGULAR,
                hairline_strong: COLOR_HAIRLINE_STRONG,
                text_primary: c32(&COLOR_TEXT_PRIMARY),
                text_secondary: c32(&COLOR_TEXT_SECONDARY),
                text_tertiary: c32(&COLOR_TEXT_TERTIARY),
                text_disabled: c32(&COLOR_TEXT_DISABLED),
                text_inverse: c32(&COLOR_TEXT_INVERSE),
                audio: c32(&COLOR_SIGNAL_AUDIO_COLOUR),
                cv: c32(&COLOR_SIGNAL_CV_COLOUR),
                event: c32(&COLOR_SIGNAL_EVENT_COLOUR),
                data: c32(&COLOR_SIGNAL_DATA_COLOUR),
                spatial: c32(&COLOR_SIGNAL_SPATIAL_COLOUR),
                playing: c32(&COLOR_STATE_PLAYING_COLOUR),
                warning: c32(&COLOR_STATE_WARNING_COLOUR),
                error: c32(&COLOR_STATE_ERROR_COLOUR),
                selected: c32(&COLOR_STATE_SELECTED_COLOUR),
            },
            ThemeChoice::ContrastHigh => Self {
                // [theme.contrast-high] EXTENDS phosphor-dark: only the overridden keys and the
                // lifted accents differ. Panels keep their tint hierarchy (depth by ground tint
                // is a token rule, and the HC theme does not override it).
                ground_base: c32(&COLOR_HC_GROUND_BASE),
                ground_canvas: c32(&COLOR_HC_GROUND_BASE),
                ground_panel: c32(&COLOR_GROUND_PANEL),
                ground_panel_alt: c32(&COLOR_GROUND_PANEL_ALT),
                ground_inset: c32(&COLOR_GROUND_INSET),
                ground_overlay: c32(&COLOR_GROUND_OVERLAY),
                hairline_colour: c32(&COLOR_HAIRLINE_COLOUR),
                hairline_faint: COLOR_HAIRLINE_FAINT,
                hairline_regular: COLOR_HC_HAIRLINE_REGULAR,
                hairline_strong: COLOR_HC_HAIRLINE_STRONG,
                text_primary: c32(&COLOR_TEXT_PRIMARY),
                text_secondary: c32(&COLOR_HC_TEXT_SECONDARY),
                text_tertiary: c32(&COLOR_HC_TEXT_TERTIARY),
                text_disabled: c32(&COLOR_TEXT_DISABLED),
                text_inverse: c32(&COLOR_TEXT_INVERSE),
                audio: c32(&COLOR_HC_SIGNAL_AUDIO_COLOUR),
                cv: c32(&COLOR_HC_SIGNAL_CV_COLOUR),
                event: c32(&COLOR_HC_SIGNAL_EVENT_COLOUR),
                data: c32(&COLOR_HC_SIGNAL_DATA_COLOUR),
                spatial: c32(&COLOR_HC_SIGNAL_SPATIAL_COLOUR),
                playing: c32(&COLOR_HC_STATE_PLAYING_COLOUR),
                warning: c32(&COLOR_HC_STATE_WARNING_COLOUR),
                error: c32(&COLOR_HC_STATE_ERROR_COLOUR),
                selected: c32(&COLOR_HC_STATE_SELECTED_COLOUR),
            },
        }
    }

    /// A hairline stroke at one of the three token alpha tiers. `alpha` is the tier (faint /
    /// regular / strong), never an invented number: depth comes from tint + hairline opacity,
    /// because shadows are forbidden (`layout.toml [elevation]`).
    #[must_use]
    pub fn hairline(&self, alpha: f32, width: f32) -> Stroke {
        Stroke::new(width, self.hairline_colour.gamma_multiply(alpha))
    }
}

/// Token colour → egui colour. The only conversion in the codebase.
#[must_use]
pub fn c32(c: &sparq_ui::Color) -> Color32 {
    Color32::from_rgb(c.r, c.g, c.b)
}

/// sparq logical-px rect → egui rect (identical spaces; the conversion exists so widget code
/// never mixes the two type families by accident).
#[must_use]
pub fn egui_rect(r: SpRect) -> egui::Rect {
    egui::Rect { min: egui::pos2(r.min.x, r.min.y), max: egui::pos2(r.max.x, r.max.y) }
}

/// egui rect → sparq rect.
#[must_use]
pub fn sp_rect(r: egui::Rect) -> SpRect {
    SpRect::new(SpVec2::new(r.min.x, r.min.y), SpVec2::new(r.max.x, r.max.y))
}

/// Font sizes straight off the type scale (typography.toml): XS 10 · S 11 · M 13 · L 15 ·
/// XL 18 · XXL 24 · STAGE 44. Mono is the primary family token; until the typefaces are chosen
/// (WO-002 open item: `chosen = ""`) egui's bundled monospace stands in — the SIZES are already
/// correct, so swapping the face later changes nothing about layout.
#[must_use]
pub fn font_xs() -> FontId {
    FontId::new(TYPOGRAPHY_SCALE_XS as f32, FontFamily::Monospace)
}
/// Scale S (11 px) monospace.
#[must_use]
pub fn font_s() -> FontId {
    FontId::new(TYPOGRAPHY_SCALE_S as f32, FontFamily::Monospace)
}
/// Scale M (13 px) monospace — body text.
#[must_use]
pub fn font_m() -> FontId {
    FontId::new(TYPOGRAPHY_SCALE_M as f32, FontFamily::Monospace)
}
/// Scale L (15 px) monospace.
#[must_use]
pub fn font_l() -> FontId {
    FontId::new(TYPOGRAPHY_SCALE_L as f32, FontFamily::Monospace)
}
/// Scale XL (18 px) monospace — headings.
#[must_use]
pub fn font_xl() -> FontId {
    FontId::new(TYPOGRAPHY_SCALE_XL as f32, FontFamily::Monospace)
}

/// Configure an egui context entirely from tokens: visuals, spacing, text styles. Called once at
/// startup and again on every theme switch — "a token change restyles everything" includes the
/// runtime theme switch, which is just re-running this with the other palette.
pub fn apply_style(ctx: &egui::Context, theme: ThemeChoice) {
    let pal = Palette::for_theme(theme);

    let mut v = Visuals::dark();
    v.panel_fill = pal.ground_panel;
    v.window_fill = pal.ground_overlay;
    v.extreme_bg_color = pal.ground_inset;
    v.faint_bg_color = pal.ground_panel_alt;
    v.code_bg_color = pal.ground_inset;
    v.override_text_color = Some(pal.text_primary);
    v.warn_fg_color = pal.warning;
    v.error_fg_color = pal.error;
    v.hyperlink_color = pal.cv;
    v.window_corner_radius = CornerRadius::same(LAYOUT_CORNER_PANEL as u8);
    v.menu_corner_radius = CornerRadius::same(LAYOUT_CORNER_PANEL as u8);
    v.window_stroke = pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32);
    // No shadows, ever (layout.toml [elevation] shadow = "forbidden").
    v.window_shadow = egui::epaint::Shadow::NONE;
    v.selection.bg_fill = pal.audio.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(LAYOUT_STROKE_EMPHASIS as f32, pal.audio);
    for w in [
        &mut v.widgets.noninteractive,
        &mut v.widgets.inactive,
        &mut v.widgets.hovered,
        &mut v.widgets.active,
        &mut v.widgets.open,
    ] {
        w.bg_fill = pal.ground_panel_alt;
        w.weak_bg_fill = pal.ground_panel;
        w.bg_stroke = pal.hairline(pal.hairline_regular, LAYOUT_STROKE_HAIRLINE as f32);
        w.corner_radius = CornerRadius::same(LAYOUT_CORNER_MICRO as u8);
        w.fg_stroke = Stroke::new(LAYOUT_STROKE_HAIRLINE as f32, pal.text_primary);
    }
    v.widgets.hovered.bg_stroke = pal.hairline(pal.hairline_strong, LAYOUT_STROKE_HAIRLINE as f32);
    v.widgets.active.bg_fill = pal.ground_inset;
    ctx.set_visuals(v);

    ctx.all_styles_mut(|s| {
        // Spacing off the 8 px space scale.
        s.spacing.item_spacing = egui::vec2(LAYOUT_SPACE_2 as f32, LAYOUT_SPACE_2 as f32);
        s.spacing.button_padding = egui::vec2(LAYOUT_SPACE_3 as f32, LAYOUT_SPACE_2 as f32);
        s.spacing.window_margin = Margin::same(LAYOUT_SPACE_PADDING_PANEL as i8);
        // egui's own minimum interactive size = the token touch minimum. Any stock egui widget
        // that sneaks into the shell inherits the 44 px floor for free; the layout audit checks
        // that nothing bypassed it.
        s.spacing.interact_size =
            egui::vec2(LAYOUT_TOUCH_MIN_TARGET as f32, LAYOUT_TOUCH_MIN_TARGET as f32);
        // The type scale, mapped onto egui's text styles.
        let mut ts = std::collections::BTreeMap::new();
        ts.insert(TextStyle::Small, font_s());
        ts.insert(TextStyle::Body, font_m());
        ts.insert(TextStyle::Monospace, font_m());
        ts.insert(TextStyle::Button, font_m());
        ts.insert(TextStyle::Heading, font_xl());
        s.text_styles = ts;
    });
}
