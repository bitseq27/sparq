//! The canvas camera: pan, zoom, level-of-detail, and zoom-to-fit — clamped to the canvas tokens.
//!
//! The camera is the only thing that knows the world↔screen transform. Everything downstream
//! (layout, hit-testing, the painter) asks it, so there is one definition of "where is this node
//! on screen" and the Phase 6 renderer inherits it verbatim.

use crate::geom::{Rect, Vec2};
use crate::tokens::{
    LAYOUT_CANVAS_LOD_1_BELOW_ZOOM, LAYOUT_CANVAS_LOD_2_BELOW_ZOOM, LAYOUT_CANVAS_ZOOM_DEFAULT,
    LAYOUT_CANVAS_ZOOM_MAX, LAYOUT_CANVAS_ZOOM_MIN,
};

/// Level of detail, from the zoom the token spec names (`layout.toml [canvas] lod_*`).
///
/// Three levels, per WO-013: full panel detail → simplified → colour-coded dot. The risk column
/// warns that LOD can make a graph unreadable when zoomed out; the *thresholds* are tokens so the
/// visual iteration happens in data, against the mockup, without touching this code.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lod {
    /// Zoom ≥ `lod_1_below_zoom`: header text, port labels, port circles.
    #[default]
    Full,
    /// `lod_2_below_zoom` ≤ zoom < `lod_1_below_zoom`: node box, coloured ports, no text.
    Simplified,
    /// Zoom < `lod_2_below_zoom`: a colour-coded dot per node, hairline wires.
    Dot,
}

/// The canvas viewport: what world point sits at the canvas's top-left, and the zoom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    /// World coordinate at the canvas rect's `min` corner.
    pub origin: Vec2,
    /// Zoom factor, clamped to `[zoom_min, zoom_max]`.
    pub zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self { origin: Vec2::ZERO, zoom: LAYOUT_CANVAS_ZOOM_DEFAULT }
    }
}

impl Camera {
    /// A default camera (origin at world zero, zoom 1.0).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Clamp zoom into the token band.
    #[must_use]
    fn clamp_zoom(z: f32) -> f32 {
        z.clamp(LAYOUT_CANVAS_ZOOM_MIN, LAYOUT_CANVAS_ZOOM_MAX)
    }

    /// World → screen, for a canvas occupying `view` (screen px).
    #[must_use]
    pub fn to_screen(&self, world: Vec2, view: Rect) -> Vec2 {
        Vec2::new(
            view.min.x + (world.x - self.origin.x) * self.zoom,
            view.min.y + (world.y - self.origin.y) * self.zoom,
        )
    }

    /// Screen → world, for a canvas occupying `view` (screen px).
    #[must_use]
    pub fn to_world(&self, screen: Vec2, view: Rect) -> Vec2 {
        Vec2::new(
            self.origin.x + (screen.x - view.min.x) / self.zoom,
            self.origin.y + (screen.y - view.min.y) / self.zoom,
        )
    }

    /// A world distance expressed in screen px (for capture radii that must stay touch-sized).
    #[must_use]
    pub fn screen_len(&self, world_len: f32) -> f32 {
        world_len * self.zoom
    }

    /// Pan by a screen-space delta (two-finger pan). The world slides *with* the fingers, so the
    /// origin moves opposite to the drag, scaled by zoom.
    pub fn pan_by_screen(&mut self, delta_screen: Vec2) {
        self.origin.x -= delta_screen.x / self.zoom;
        self.origin.y -= delta_screen.y / self.zoom;
    }

    /// Zoom by an incremental factor about a screen focus point, keeping the world point under the
    /// focus pinned (the pinch feels anchored between the fingers, not at a corner).
    pub fn zoom_about(&mut self, factor: f32, focus_screen: Vec2, view: Rect) {
        let anchor = self.to_world(focus_screen, view);
        self.zoom = Self::clamp_zoom(self.zoom * factor);
        // Re-solve the origin so `anchor` maps back to `focus_screen` at the new zoom.
        self.origin.x = anchor.x - (focus_screen.x - view.min.x) / self.zoom;
        self.origin.y = anchor.y - (focus_screen.y - view.min.y) / self.zoom;
    }

    /// Zoom to fit `bounds` (world) inside `view` (screen) with `pad` screen px of margin. An empty
    /// graph resets to the default camera. The zoom is clamped, so a one-node graph does not blow
    /// up past `zoom_max`.
    pub fn zoom_to_fit(&mut self, bounds: Option<Rect>, view: Rect, pad: f32) {
        let Some(b) = bounds else {
            *self = Self::default();
            return;
        };
        let avail_w = (view.width() - 2.0 * pad).max(1.0);
        let avail_h = (view.height() - 2.0 * pad).max(1.0);
        let bw = b.width().max(1.0);
        let bh = b.height().max(1.0);
        let zoom = Self::clamp_zoom((avail_w / bw).min(avail_h / bh));
        self.zoom = zoom;
        // Centre the bounds in the view: the world point at the view centre is the bounds centre.
        let c = b.center();
        self.origin.x = c.x - (view.width() / 2.0) / zoom;
        self.origin.y = c.y - (view.height() / 2.0) / zoom;
    }

    /// The level of detail for the current zoom.
    #[must_use]
    pub fn lod(&self) -> Lod {
        if self.zoom < LAYOUT_CANVAS_LOD_2_BELOW_ZOOM {
            Lod::Dot
        } else if self.zoom < LAYOUT_CANVAS_LOD_1_BELOW_ZOOM {
            Lod::Simplified
        } else {
            Lod::Full
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view() -> Rect {
        Rect::from_min_size(Vec2::new(100.0, 50.0), Vec2::new(1000.0, 700.0))
    }

    #[test]
    fn world_screen_round_trip() {
        let cam = Camera::new();
        let v = view();
        let w = Vec2::new(240.0, 160.0);
        let s = cam.to_screen(w, v);
        let back = cam.to_world(s, v);
        assert!((back.x - w.x).abs() < 1e-3 && (back.y - w.y).abs() < 1e-3);
    }

    #[test]
    fn zoom_is_clamped_to_the_token_band() {
        let mut cam = Camera::new();
        let v = view();
        for _ in 0..40 {
            cam.zoom_about(1.5, v.center(), v);
        }
        assert!(cam.zoom <= LAYOUT_CANVAS_ZOOM_MAX + 1e-4);
        for _ in 0..80 {
            cam.zoom_about(0.5, v.center(), v);
        }
        assert!(cam.zoom >= LAYOUT_CANVAS_ZOOM_MIN - 1e-4);
    }

    #[test]
    fn pinch_keeps_the_world_point_under_the_focus_pinned() {
        let mut cam = Camera::new();
        let v = view();
        let focus = Vec2::new(400.0, 300.0);
        let before = cam.to_world(focus, v);
        cam.zoom_about(1.7, focus, v);
        let after = cam.to_world(focus, v);
        assert!((before.x - after.x).abs() < 1e-2, "x pinned: {before:?} vs {after:?}");
        assert!((before.y - after.y).abs() < 1e-2, "y pinned");
    }

    #[test]
    fn lod_follows_the_zoom_tokens() {
        let mut cam = Camera::new();
        cam.zoom = 1.0;
        assert_eq!(cam.lod(), Lod::Full);
        cam.zoom = (LAYOUT_CANVAS_LOD_1_BELOW_ZOOM + LAYOUT_CANVAS_LOD_2_BELOW_ZOOM) / 2.0;
        assert_eq!(cam.lod(), Lod::Simplified);
        cam.zoom = LAYOUT_CANVAS_LOD_2_BELOW_ZOOM / 2.0;
        assert_eq!(cam.lod(), Lod::Dot);
    }

    #[test]
    fn zoom_to_fit_frames_the_bounds_and_empty_resets() {
        let mut cam = Camera::new();
        let v = view();
        cam.zoom_to_fit(Some(Rect::from_min_size(Vec2::ZERO, Vec2::new(2000.0, 1000.0))), v, 24.0);
        // A 2000×1000 world in a 1000×700 view: width-limited, zoomed out below 1.
        assert!(cam.zoom < 1.0 && cam.zoom >= LAYOUT_CANVAS_ZOOM_MIN);
        cam.zoom_to_fit(None, v, 24.0);
        assert_eq!(cam, Camera::default(), "an empty graph resets the camera");
    }
}
