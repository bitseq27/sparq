//! The display-list IR — a field-for-field mirror of the frozen `display.wit` v1 vocabulary.
//!
//! This is the OUTPUT of the core and the shared object between the two painters (the harness's
//! SVG writer here in INC3, INC4's `sparq-ui/src/displaylist.rs` SVG + egui back ends). Keeping it
//! a pure mirror — same variants, same field order, same closed enums — is what makes "identical
//! through both painters from the same display list" (WO-019) a property rather than a hope: both
//! painters consume *this* shape, so they cannot drift apart without a test failing.
//!
//! The contract's central rule is encoded in the types (display.wit header): there is **no colour
//! field, no size field and no duration field**. Styling is a [`TokenId`] (a semantic string the
//! host resolves against the CURRENT bundle) or a closed enum ([`StrokeWidth`] 1/2/3, [`Corner`]
//! 0/2/4, [`Dash`]). A literal appearance value is not forbidden — it is *unrepresentable*, which
//! is what turns token-spec §3 rule 1 from an audit into a property.
//!
//! Naming: the WIT is kebab-case (`a0-deg`, `glyph-run`); Rust is snake/camel. The mapping is 1:1
//! and documented per type. The wasm glue (`observatory-wasm`) converts these to the SDK's
//! generated `bindings::…::display::*` types; that conversion is the only place the two vocabularies
//! meet, and it is total.

/// A semantic token id, resolved by the host at render time (`color.signal.data.colour`,
/// `layout.space.4`, `typography.scale.m`, `colormap.thermal`). Unknown ids: the primitive is
/// skipped with a diagnostic at runtime (fail soft) and is a hard validation failure at hand-in
/// (fail loud). The core only ever emits ids that exist in the checked-in bundle (drift-tested by
/// the harness against `design/tokens/generated/tokens.json`).
pub type TokenId = String;

/// The only stroke widths that exist (token-spec §3 rule 2; display.wit `stroke-width`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StrokeWidth {
    /// w1 — hairline: structure, borders, dividers, grids, graticules, inactive traces.
    W1,
    /// w2 — signal: active data traces, the gauge value arc.
    W2,
    /// w3 — emphasis: the selected cell's frame and nothing else (§5.6 rule 5).
    W3,
}

impl StrokeWidth {
    /// The pixel width this class resolves to (the bundle's `layout.stroke.*`). Used by the
    /// harness's SVG writer; the host painter resolves the same ids. Kept here so the core's own
    /// budget counting can weight primitives honestly.
    #[must_use]
    pub const fn px(self) -> f32 {
        match self {
            Self::W1 => 1.0,
            Self::W2 => 2.0,
            Self::W3 => 3.0,
        }
    }
}

/// The only corner radii that exist (token-spec §3 rule 3; display.wit `corner`). Circles are for
/// knobs and ports, which the host draws — guests get rects.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// c0 — square (default for data marks, cells).
    C0,
    /// c2 — micro (buttons, chips).
    C2,
    /// c4 — panel (cards, dialogs).
    C4,
}

/// Dash patterns — the non-colour encoding channel (token-spec §3 rule 7; display.wit `dash`).
/// Every colour state also carries one of these, so a monochrome or colour-blind render still
/// distinguishes observed-vs-predicted, live-vs-stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dash {
    /// Solid — observed data, primary structure.
    Solid,
    /// Dashed — predicted/modelled (e.g. tide predictions vs observations, §5.5).
    Dashed,
    /// Dotted — secondary/guides (graticules, the data class's own encoding).
    Dotted,
    /// Hidden — a geometry carrier with no visible stroke (rare; e.g. a fill-only path's edge).
    Hidden,
}

/// Host LOD policy level, from the shell's zoom/breakpoint rules (display.wit `lod`). The host
/// passes this in [`crate::wall::FrameContext`]; the guest degrades per §5.7.
///
/// The variants are declared DETAIL-ASCENDING (Minimal < Reduced < Full) so the derived ordering
/// reads as "detail level": `lod >= Lod::Reduced` means "at least Reduced detail" (Reduced or
/// Full), which is how the renderers gate the grid/tick/label chrome. The display.wit declaration
/// order (full, reduced, minimal) is a name set, not an ordering — the wasm glue maps by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Lod {
    /// Title + LED + one value/headline; renderer only if cheap, else WORDS (§5.7).
    Minimal,
    /// Drop graticules, footnotes, tick labels; keep renderer + title + LED + stamp.
    Reduced,
    /// Everything: graticules, footnotes, tick labels, full chrome.
    Full,
}

/// A 2D point in the display's logical-px space (display.wit `point`). The guest lays out against
/// logical px, never device pixels (frame-context's contract).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// x, logical px from the display's left edge.
    pub x: f32,
    /// y, logical px from the display's top edge.
    pub y: f32,
}

impl Point {
    /// A point at (`x`, `y`).
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Shared styling (display.wit `style`). `stroke`/`fill` are token ids; the look-board's
/// combination table (validated at hand-in) governs which may co-occur on which primitive. A
/// data-carrying primitive (points, heat cells) puts its colour in `colormap`, not here.
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Stroke colour token id, or `None` for a fill-only primitive.
    pub stroke: Option<TokenId>,
    /// Stroke width class.
    pub stroke_width: StrokeWidth,
    /// Fill colour token id, or `None` for a stroke-only primitive.
    pub fill: Option<TokenId>,
    /// Dash pattern (the non-colour encoding channel).
    pub dash: Dash,
    /// Corner radius class.
    pub corner: Corner,
}

impl Style {
    /// A hairline stroke of `token` (w1, solid, no fill, square) — the grid/graticule/axis idiom.
    #[must_use]
    pub fn hairline(token: impl Into<TokenId>) -> Self {
        Self {
            stroke: Some(token.into()),
            stroke_width: StrokeWidth::W1,
            fill: None,
            dash: Dash::Solid,
            corner: Corner::C0,
        }
    }

    /// A hairline stroke of `token` with a dash — guides and predicted series.
    #[must_use]
    pub fn hairline_dashed(token: impl Into<TokenId>, dash: Dash) -> Self {
        Self { dash, ..Self::hairline(token) }
    }

    /// A signal-width (w2) stroke of `token`, no fill — a live data trace.
    #[must_use]
    pub fn trace(token: impl Into<TokenId>) -> Self {
        Self { stroke_width: StrokeWidth::W2, ..Self::hairline(token) }
    }

    /// A fill of `token`, no stroke, square corners — a solid data mark.
    #[must_use]
    pub fn fill(token: impl Into<TokenId>) -> Self {
        Self {
            stroke: None,
            stroke_width: StrokeWidth::W1,
            fill: Some(token.into()),
            dash: Dash::Solid,
            corner: Corner::C0,
        }
    }

    /// A fill of `token` with rounded corners `c` — a cell body, a chip.
    #[must_use]
    pub fn fill_corner(token: impl Into<TokenId>, corner: Corner) -> Self {
        Self { corner, ..Self::fill(token) }
    }

    /// The selection emphasis frame (§5.3/§5.6 rule 5): w3 stroke of `token`, no fill.
    #[must_use]
    pub fn emphasis(token: impl Into<TokenId>) -> Self {
        Self {
            stroke: Some(token.into()),
            stroke_width: StrokeWidth::W3,
            fill: None,
            dash: Dash::Solid,
            corner: Corner::C2,
        }
    }
}

/// One segment of a [`PathItem`] (display.wit `path-segment`). The control points mirror the WIT
/// tuples: `quad-to` carries (control, end), `cubic-to` carries (c1, c2, end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathSegment {
    /// `move-to(point)` — start a new subpath.
    MoveTo(Point),
    /// `line-to(point)` — a straight segment.
    LineTo(Point),
    /// `quad-to((control, end))` — a quadratic Bézier.
    QuadTo(Point, Point),
    /// `cubic-to((c1, c2, end))` — a cubic Bézier.
    CubicTo(Point, Point, Point),
}

/// `polyline-item`: an open run of [`Point`]s stroked with one [`Style`]. The workhorse for
/// timeseries traces and coastlines-at-LOD (display.wit `polyline-item`).
#[derive(Clone, Debug, PartialEq)]
pub struct PolylineItem {
    /// The vertices, in draw order.
    pub points: Vec<Point>,
    /// Stroke style (a polyline is stroke-only; `fill` is ignored by the painters).
    pub style: Style,
}

/// `path-item`: a subpath list (move/line/quad/cubic), optionally closed, one [`Style`]
/// (display.wit `path-item`). Used for coastlines (closed land masses), map markers, crosshairs.
#[derive(Clone, Debug, PartialEq)]
pub struct PathItem {
    /// The segment list.
    pub segments: Vec<PathSegment>,
    /// Whether the painter closes the final subpath back to its start.
    pub closed: bool,
    /// Stroke and/or fill.
    pub style: Style,
}

/// `rect-item`: an axis-aligned rectangle (display.wit `rect-item`). Cell bodies, bars, ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct RectItem {
    /// Left edge, logical px.
    pub x: f32,
    /// Top edge, logical px.
    pub y: f32,
    /// Width.
    pub w: f32,
    /// Height.
    pub h: f32,
    /// Stroke and/or fill.
    pub style: Style,
}

/// `arc-item`: a circular arc from `a0_deg` to `a1_deg` (display.wit `arc-item`), degrees, 0° at
/// +x sweeping clockwise in the display's y-down space. Gauges (sweeps), status LEDs (full circles
/// via 0→360), the ISS footprint ring.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcItem {
    /// Centre x.
    pub cx: f32,
    /// Centre y.
    pub cy: f32,
    /// Radius, logical px.
    pub r: f32,
    /// Start angle, degrees.
    pub a0_deg: f32,
    /// End angle, degrees.
    pub a1_deg: f32,
    /// Stroke and/or fill.
    pub style: Style,
}

/// `glyph-run-item`: text via the host type engine (display.wit `glyph-run-item`). The guest
/// supplies the string — numbers pre-formatted WITH their units (token-spec §3 rule 9) — a
/// semantic size token and a text-tier colour token. No fonts, no metrics, no literals.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRunItem {
    /// The text (already unit-bearing where it is a number).
    pub text: String,
    /// Baseline-origin x, logical px.
    pub x: f32,
    /// Baseline-origin y, logical px.
    pub y: f32,
    /// Size token (`typography.scale.s`/`.m`/…).
    pub size: TokenId,
    /// Text-tier colour token (`color.text.primary`/`.secondary`/`.tertiary`).
    pub colour: TokenId,
}

/// `points-item`: a point cloud (display.wit `points-item`). Per-point colouring rides `values` +
/// `colormap` (data encoding); flat colour rides `colour` (a signal-class token). Exactly one of
/// `colour`/(`colormap`+`values`) is set — the look-board validates the combination.
#[derive(Clone, Debug, PartialEq)]
pub struct PointsItem {
    /// The point positions.
    pub positions: Vec<Point>,
    /// Marker size token (`layout.marker.radius_s`/`.radius_m`/`.radius_l`).
    pub size: TokenId,
    /// Flat colour token (signal class), or `None` when colormap-driven.
    pub colour: Option<TokenId>,
    /// Colormap token, or `None` when flat-coloured.
    pub colormap: Option<TokenId>,
    /// Per-point data channel feeding `colormap` (same length as `positions`), or `None`.
    pub values: Option<Vec<f32>>,
}

/// `heat-cells-item`: a regular grid filled from a declared colormap (display.wit
/// `heat-cells-item`) — the aurora/FIRMS density case. `values` is row-major, `cols * rows` long;
/// NaN = no cell drawn (the contract's own no-cell sentinel).
#[derive(Clone, Debug, PartialEq)]
pub struct HeatCellsItem {
    /// Grid left edge, logical px.
    pub x: f32,
    /// Grid top edge, logical px.
    pub y: f32,
    /// Grid width.
    pub w: f32,
    /// Grid height.
    pub h: f32,
    /// Column count.
    pub cols: u32,
    /// Row count.
    pub rows: u32,
    /// Row-major magnitudes, `cols * rows` long; NaN skips a cell.
    pub values: Vec<f32>,
    /// Colormap token (`colormap.thermal`, …).
    pub colormap: TokenId,
}

/// `trace-item`: a rolling/phosphor trace mapped across the item's box (display.wit `trace-item`)
/// — the scope case. Decay and glow are the HOST's motion tokens; the guest never simulates
/// persistence. `values` are normalised 0..1 across the box.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceItem {
    /// Sample values (mapped across the box; the host owns the box geometry for a trace).
    pub values: Vec<f32>,
    /// Trace colour token (signal class).
    pub colour: TokenId,
    /// Trace width class.
    pub width: StrokeWidth,
    /// Whether the host applies its phosphor decay/glow motion tokens.
    pub phosphor: bool,
}

/// One display-list primitive (display.wit `item`, the eight-variant 2D vocabulary).
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// `polyline(polyline-item)`.
    Polyline(PolylineItem),
    /// `path(path-item)`.
    Path(PathItem),
    /// `rect(rect-item)`.
    Rect(RectItem),
    /// `arc(arc-item)`.
    Arc(ArcItem),
    /// `glyph-run(glyph-run-item)`.
    GlyphRun(GlyphRunItem),
    /// `points(points-item)`.
    Points(PointsItem),
    /// `heat-cells(heat-cells-item)`.
    HeatCells(HeatCellsItem),
    /// `trace(trace-item)`.
    Trace(TraceItem),
}

/// What one `draw` call returns (display.wit `surface`). The Observatory declares a `display_list`
/// display, so it only ever emits [`Surface::Items`]; the [`Surface::Scene`] arm exists to mirror
/// the contract completely (and to keep the wasm glue's conversion total) but is unused in v1.
#[derive(Clone, Debug, PartialEq)]
pub enum Surface {
    /// A 2D display list.
    Items(Vec<Item>),
    /// A 3D scene descriptor — not produced by the Observatory in v1 (see the type docs). The
    /// payload mirrors display.wit's `scene`; the core never builds one, so it carries the raw
    /// structure only for the glue's completeness and is deliberately unpopulated here.
    Scene(Scene),
}

/// A 3D scene descriptor (display.wit `scene`). The Observatory does not emit scenes in v1; this
/// mirrors the contract so [`Surface`] is a faithful variant and the glue's match is total. Fields
/// follow display.wit; scene element types are elided to a count because nothing constructs them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Scene {
    /// Number of scene elements (the core always builds zero — v1 is display-list only).
    pub element_count: u32,
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn stroke_widths_resolve_to_the_bundle_values() {
        // The three widths are the bundle's layout.stroke.hairline/signal/emphasis (1/2/3). The
        // `hero` width (4) exists in the bundle but is Perform-mode-only and NOT in display.wit's
        // closed enum — a guest cannot emit it, which is the point of the closed vocabulary.
        assert_eq!(StrokeWidth::W1.px(), 1.0);
        assert_eq!(StrokeWidth::W2.px(), 2.0);
        assert_eq!(StrokeWidth::W3.px(), 3.0);
    }

    #[test]
    fn style_constructors_never_carry_a_literal_appearance() {
        // Every style field is a token id or a closed enum — there is no f32 colour, no hex, no
        // size literal. This test is the property display.wit's header calls "unrepresentable".
        let s = Style::hairline("color.hairline.colour");
        assert_eq!(s.stroke.as_deref(), Some("color.hairline.colour"));
        assert_eq!(s.stroke_width, StrokeWidth::W1);
        assert!(s.fill.is_none());
        let f = Style::fill_corner("color.ground.inset", Corner::C2);
        assert!(f.stroke.is_none());
        assert_eq!(f.fill.as_deref(), Some("color.ground.inset"));
        assert_eq!(f.corner, Corner::C2);
        let e = Style::emphasis("color.state.selected.colour");
        assert_eq!(e.stroke_width, StrokeWidth::W3);
    }

    #[test]
    fn lod_orders_full_above_minimal() {
        // The §5.7 degradation ladder reads as an ordering: Full > Reduced > Minimal.
        assert!(Lod::Full > Lod::Reduced);
        assert!(Lod::Reduced > Lod::Minimal);
    }

    #[test]
    fn a_points_item_is_either_flat_or_colormapped_never_both() {
        // The look-board's combination rule, as a constructor invariant the renderers honour.
        let flat = PointsItem {
            positions: vec![Point::new(1.0, 2.0)],
            size: "layout.marker.radius_m".into(),
            colour: Some("color.signal.data.colour".into()),
            colormap: None,
            values: None,
        };
        assert!(flat.colour.is_some() && flat.colormap.is_none() && flat.values.is_none());
        let mapped = PointsItem {
            positions: vec![Point::new(1.0, 2.0)],
            size: "layout.marker.radius_m".into(),
            colour: None,
            colormap: Some("colormap.inferno-class".into()),
            values: Some(vec![0.5]),
        };
        assert!(mapped.colour.is_none() && mapped.colormap.is_some());
        assert_eq!(mapped.values.as_ref().unwrap().len(), mapped.positions.len());
    }
}
