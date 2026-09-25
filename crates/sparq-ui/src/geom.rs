//! Minimal 2-D geometry for the toolkit-independent UI core (WO-012).
//!
//! Deliberately *not* `egui::Pos2`/`Rect`: `docs/ui/input-model.md` §1 requires the gesture and
//! layout model to survive the Phase 6 renderer swap, so they are built on sparq's own types and
//! the toolkit adapters convert at their boundary. The same reason `Wo005Graph` knows nothing
//! about cpal.
//!
//! Logical pixels throughout, y down (the convention every 2-D toolkit in scope uses). The DPI
//! transform lives in the shell, never here: one space, one meaning.

/// A 2-D vector / point in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vec2 {
    /// The zero vector.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Construct from components.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Component-wise addition.
    #[must_use]
    pub const fn add(self, rhs: Self) -> Self {
        Self { x: self.x + rhs.x, y: self.y + rhs.y }
    }

    /// Component-wise subtraction.
    #[must_use]
    pub const fn sub(self, rhs: Self) -> Self {
        Self { x: self.x - rhs.x, y: self.y - rhs.y }
    }

    /// Uniform scale.
    #[must_use]
    pub fn scale(self, s: f32) -> Self {
        Self { x: self.x * s, y: self.y * s }
    }

    /// Dot product.
    #[must_use]
    pub const fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    /// Euclidean length.
    #[must_use]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    /// Distance to another point.
    #[must_use]
    pub fn distance(self, rhs: Self) -> f32 {
        self.sub(rhs).length()
    }
}

impl std::ops::Add for Vec2 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::add(self, rhs)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::sub(self, rhs)
    }
}

/// An axis-aligned rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// Minimum corner (top-left with y down).
    pub min: Vec2,
    /// Maximum corner (bottom-right with y down).
    pub max: Vec2,
}

impl Rect {
    /// The empty rectangle at the origin.
    pub const NOTHING: Self = Self { min: Vec2::ZERO, max: Vec2::ZERO };

    /// Construct from corners.
    #[must_use]
    pub const fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    /// Construct from a corner and a size.
    #[must_use]
    pub fn from_min_size(min: Vec2, size: Vec2) -> Self {
        Self { min, max: min + size }
    }

    /// Width.
    #[must_use]
    pub fn width(&self) -> f32 {
        (self.max.x - self.min.x).max(0.0)
    }

    /// Height.
    #[must_use]
    pub fn height(&self) -> f32 {
        (self.max.y - self.min.y).max(0.0)
    }

    /// Size as a vector.
    #[must_use]
    pub fn size(&self) -> Vec2 {
        Vec2::new(self.width(), self.height())
    }

    /// Centre point.
    #[must_use]
    pub fn center(&self) -> Vec2 {
        Vec2::new((self.min.x + self.max.x) / 2.0, (self.min.y + self.max.y) / 2.0)
    }

    /// Does the rect contain the point? Half-open: min inclusive, max exclusive, so adjacent
    /// panels do not both claim their shared border.
    #[must_use]
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x && p.x < self.max.x && p.y >= self.min.y && p.y < self.max.y
    }

    /// The shortest side, which is what the touch-target audit measures against the class minimum.
    #[must_use]
    pub fn min_side(&self) -> f32 {
        self.width().min(self.height())
    }

    /// Snap both corners to a `grid`-pixel lattice (the shell's 8 px rule).
    #[must_use]
    pub fn snapped_to_grid(&self, grid: f32) -> Self {
        debug_assert!(grid > 0.0);
        let snap = |v: f32| (v / grid).round() * grid;
        Self::new(
            Vec2::new(snap(self.min.x), snap(self.min.y)),
            Vec2::new(snap(self.max.x), snap(self.max.y)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains_is_half_open() {
        let r = Rect::new(Vec2::new(10.0, 10.0), Vec2::new(20.0, 20.0));
        assert!(r.contains(Vec2::new(10.0, 10.0)), "min corner is inside");
        assert!(!r.contains(Vec2::new(20.0, 20.0)), "max corner is outside (half-open)");
        assert!(r.contains(Vec2::new(19.9, 19.9)));
        assert!(!r.contains(Vec2::new(9.9, 15.0)));
    }

    #[test]
    fn snapping_is_to_the_nearest_grid_point() {
        let r = Rect::new(Vec2::new(3.0, 5.0), Vec2::new(263.0, 121.0));
        let s = r.snapped_to_grid(8.0);
        assert_eq!((s.min.x, s.min.y), (0.0, 8.0));
        assert_eq!((s.max.x, s.max.y), (264.0, 120.0));
    }

    #[test]
    fn min_side_is_what_the_audit_measures() {
        let wide = Rect::from_min_size(Vec2::ZERO, Vec2::new(500.0, 40.0));
        assert_eq!(wide.min_side(), 40.0);
    }
}
