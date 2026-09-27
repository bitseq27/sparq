//! The inspector: computed parameter-panel geometry for one selected node (WO-013 increment 3).
//!
//! Same contract as [`super::layout`]: pure computation from (node, panel rect) to row rects,
//! slider tracks and the x→value mapping. The painter draws exactly these rects; the gesture path
//! hit-tests exactly these rects; the audit measures exactly these rects. The drawn track is a
//! thin line, but the **touch target is the full row height** — the port-capture trick applied to
//! sliders, so a finger-sized grab never needs pixel aim.
//!
//! Scrolling (increment 5): [`compute_at`] lays the rows out under a scroll offset, clamped into
//! `[0, max_scroll]`; the title row is a FIXED header and the rows slide under it. The honesty
//! rule survives the offset in both directions: a row clipped at the panel bottom is touchable
//! through its visible sliver (you can see it), while a row scrolled under the header is refused
//! by [`row_at`] and skipped by the painter (you cannot). One finger on a row still edits the
//! slider, so the scroll GESTURE is the two-finger pan whose centre lands in the panel — the
//! recogniser's `Pan` carries its centre for exactly this routing.
//!
//! v0 limits, stated rather than hidden:
//! * only the kinds [`ParamDesc::editable`] names are adjustable; `enum`/`text`/`blob` rows are
//!   shown greyed with their kind named, because inventing an options editor before the manifest
//!   schema grows `options[]` would be a lie the user could act on.

use crate::canvas::model::{Node, NodeId, ParamDesc};
use crate::geom::{Rect, Vec2};
use crate::tokens::{
    LAYOUT_SPACE_1, LAYOUT_SPACE_4, LAYOUT_SPACE_PADDING_PANEL, LAYOUT_TOUCH_ROW_HEIGHT_LIST,
};

/// One parameter's row, laid out inside the panel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamRow {
    /// Index into the node's [`crate::canvas::model::NodeSpec::params`].
    pub index: usize,
    /// Where the name is drawn (left column).
    pub label: Rect,
    /// The slider track — the TOUCH target, full row height (the drawn line is thinner).
    pub track: Rect,
    /// Where the value text is drawn (right column).
    pub value: Rect,
    /// Whether a drag on `track` changes the value (see [`ParamDesc::editable`]).
    pub editable: bool,
}

/// The whole panel for one node.
#[derive(Clone, Debug, PartialEq)]
pub struct InspectorLayout {
    /// Which node this inspects.
    pub node: NodeId,
    /// The panel rect the layout was computed for.
    pub rect: Rect,
    /// The title row (node title + module id). FIXED under scrolling: the rows slide under it.
    pub title: Rect,
    /// One row per parameter, in manifest order, at their SCROLLED positions.
    pub rows: Vec<ParamRow>,
    /// The scroll offset these rows were laid out at, px, clamped into `[0, max_scroll]`.
    pub scroll: f32,
    /// The largest useful scroll offset: how much of the content hangs below the panel. `0.0`
    /// means every row already fits — there is nothing to scroll and the panel says so in words.
    pub max_scroll: f32,
}

/// Column splits of the content width, as fractions: label | track | value.
const LABEL_FRAC: f32 = 0.38;
const TRACK_FRAC: f32 = 0.40;

/// Compute the inspector panel for `node` inside `rect` (screen px, from the shell layout),
/// unscrolled. [`compute_at`] is the scrolling form; this is it at offset zero, kept as its own
/// name because "the panel" is what almost every caller means.
#[must_use]
pub fn compute(node: &Node, rect: Rect) -> InspectorLayout {
    compute_at(node, rect, 0.0)
}

/// [`compute`] at a requested scroll offset, px down the content. The offset is CLAMPED here —
/// the layout is the one place that knows the content height, so no caller can over-scroll a
/// panel into blank space. Negative requests clamp to zero (the top).
#[must_use]
pub fn compute_at(node: &Node, rect: Rect, scroll: f32) -> InspectorLayout {
    let pad = LAYOUT_SPACE_PADDING_PANEL as f32;
    let row_h = LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32;
    let content_x = rect.min.x + pad;
    let content_w = (rect.width() - 2.0 * pad).max(0.0);

    let title =
        Rect::from_min_size(Vec2::new(content_x, rect.min.y + pad), Vec2::new(content_w, row_h));

    let n = node.spec.params.len();
    // The content hangs this far below the panel when it is longer than the panel is.
    let content_bottom = title.max.y + n as f32 * row_h;
    let max_scroll = (content_bottom - rect.max.y).max(0.0);
    let scroll = if scroll.is_finite() { scroll.clamp(0.0, max_scroll) } else { 0.0 };

    let mut rows = Vec::with_capacity(n);
    for (i, desc) in node.spec.params.iter().enumerate() {
        let y = title.max.y + i as f32 * row_h - scroll;
        let row = Rect::from_min_size(Vec2::new(content_x, y), Vec2::new(content_w, row_h));
        rows.push(ParamRow {
            index: i,
            label: Rect::from_min_size(row.min, Vec2::new(content_w * LABEL_FRAC, row_h)),
            track: Rect::from_min_size(
                Vec2::new(row.min.x + content_w * LABEL_FRAC, y),
                Vec2::new(content_w * TRACK_FRAC, row_h),
            ),
            value: Rect::from_min_size(
                Vec2::new(row.min.x + content_w * (LABEL_FRAC + TRACK_FRAC), y),
                Vec2::new(content_w * (1.0 - LABEL_FRAC - TRACK_FRAC), row_h),
            ),
            editable: desc.editable(),
        });
    }
    InspectorLayout { node: node.id, rect, title, rows, scroll, max_scroll }
}

impl InspectorLayout {
    /// The row a screen point lands on, if any. Rows (or parts of rows) outside the panel rect do
    /// not count — an invisible control must not be touchable. A row scrolled under the fixed
    /// title is invisible too, so it is refused by the same rule.
    #[must_use]
    pub fn row_at(&self, pos: Vec2) -> Option<usize> {
        if !self.rect.contains(pos) || pos.y < self.title.max.y {
            return None;
        }
        self.rows.iter().position(|r| r.track.contains(pos) || r.label.contains(pos))
    }

    /// Whether a row is drawn at all: fully under the fixed title or fully past the panel bottom
    /// means the painter skips it. The painter and the hit-test read the same rule — what you
    /// cannot see you cannot touch, and what you can see (even a sliver) you can.
    #[must_use]
    pub fn row_visible(&self, row: &ParamRow) -> bool {
        row.track.min.y >= self.title.max.y && row.track.min.y < self.rect.max.y
    }

    /// The rows a screen point can reach, in order — the painter's loop bound and the audit's
    /// element list, so neither invents its own clipping rule.
    pub fn visible_rows(&self) -> impl Iterator<Item = &ParamRow> {
        self.rows.iter().filter(|r| self.row_visible(r))
    }

    /// The scrollbar thumb, or `None` when everything fits (nothing to scroll, nothing drawn —
    /// a scrollbar that cannot move is chrome pretending to be a control). Geometry only: the
    /// painter draws it, no interaction rides it in v0 (the two-finger pan is the gesture).
    #[must_use]
    pub fn thumb(&self) -> Option<Rect> {
        if self.max_scroll <= f32::EPSILON {
            return None;
        }
        let w = LAYOUT_SPACE_1 as f32;
        let track_top = self.title.max.y;
        let track_h = (self.rect.max.y - track_top).max(1.0);
        let content_h = track_h + self.max_scroll;
        let thumb_h = (track_h * track_h / content_h).max(LAYOUT_SPACE_4 as f32).min(track_h);
        let travel = track_h - thumb_h;
        let y = track_top + travel * (self.scroll / self.max_scroll);
        Some(Rect::new(Vec2::new(self.rect.max.x - w, y), Vec2::new(self.rect.max.x, y + thumb_h)))
    }

    /// The first and last row indices on screen, for the shell's log line — the panel says where
    /// it scrolled to in words rather than in pixels alone. `(None, None)` when no row is shown.
    #[must_use]
    pub fn visible_range(&self) -> (Option<usize>, Option<usize>) {
        let shown: Vec<usize> = self.visible_rows().map(|r| r.index).collect();
        (shown.first().copied(), shown.last().copied())
    }

    /// The parameter descriptor for row `i`, from `node`. `None` if the layout and the node
    /// disagree (a stale layout after a selection change — the caller recomputes per frame, so
    /// this only guards the hand-driven paths).
    #[must_use]
    pub fn desc<'n>(&self, node: &'n Node, i: usize) -> Option<&'n ParamDesc> {
        if node.id != self.node {
            return None;
        }
        node.spec.params.get(i)
    }
}

/// Map a screen x inside `track` to a parameter value: normalise, clamp, scale into
/// `[min, max]`. Snapping (`Int` steps, `Bool` poles) is the MODEL's job ([`crate::canvas::model::
/// Graph::op_set_param`]) so every edit path obeys one rule — this returns the continuous value
/// the finger is over.
#[must_use]
pub fn value_from_x(desc: &ParamDesc, track: Rect, x: f32) -> f32 {
    let w = track.width().max(f32::EPSILON);
    let t = ((x - track.min.x) / w).clamp(0.0, 1.0);
    let lo = desc.min as f32;
    let hi = desc.max as f32;
    lo + t * (hi - lo)
}

/// The screen x of a value's knob inside `track` — the painter's side of the same mapping, so the
/// knob you see is the value you have (round-trips with [`value_from_x`] up to snapping).
#[must_use]
pub fn knob_x(desc: &ParamDesc, track: Rect, value: f32) -> f32 {
    let lo = desc.min as f32;
    let hi = desc.max as f32;
    let span = hi - lo;
    let t = if span.abs() <= f32::EPSILON { 0.0 } else { ((value - lo) / span).clamp(0.0, 1.0) };
    track.min.x + t * track.width()
}

/// The value text the painter shows: unit-suffixed for numerics, ON/OFF for bools, the kind name
/// for the non-editable rows (never an empty cell — "no value" and "value 0" must look different).
#[must_use]
pub fn value_text(desc: &ParamDesc, value: f32) -> String {
    use sparq_module_api::manifest::ParamKind;
    match desc.kind {
        ParamKind::Bool => {
            if value >= 0.5 {
                "ON".to_string()
            } else {
                "OFF".to_string()
            }
        },
        ParamKind::Float => match &desc.unit {
            Some(u) => format!("{value:.2} {u}"),
            None => format!("{value:.2}"),
        },
        ParamKind::Int => match &desc.unit {
            Some(u) => format!("{:.0} {u}", value.round()),
            None => format!("{:.0}", value.round()),
        },
        k => format!("{k:?} · v1"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::canvas::model::{Graph, NodeSpec, ParamDesc, ParamKind};

    fn desc(id: &str, kind: ParamKind, min: f64, max: f64, def: f64) -> ParamDesc {
        ParamDesc {
            id: id.into(),
            name: id.into(),
            kind,
            unit: Some("Hz".into()),
            min,
            max,
            default: def,
        }
    }

    fn node_with_params() -> Node {
        let spec = NodeSpec::new("sparq/syn/sine", "Sine", vec![]).with_params(vec![
            desc("freq", ParamKind::Float, 0.0, 24_000.0, 440.0),
            desc("amp", ParamKind::Float, 0.0, 1.0, 0.5),
            ParamDesc {
                id: "mode".into(),
                name: "mode".into(),
                kind: ParamKind::Enum,
                unit: None,
                min: 0.0,
                max: 0.0,
                default: 0.0,
            },
        ]);
        let mut g = Graph::new();
        g.op_add_node(spec, Vec2::ZERO);
        g.nodes()[0].clone()
    }

    fn panel() -> Rect {
        Rect::from_min_size(Vec2::new(800.0, 100.0), Vec2::new(400.0, 600.0))
    }

    #[test]
    fn rows_clear_the_touch_minimum() {
        let il = compute(&node_with_params(), panel());
        for r in &il.rows {
            assert!(r.track.height() >= 44.0, "slider touch target {} px", r.track.height());
        }
    }

    #[test]
    fn one_row_per_param_in_manifest_order_and_editability_from_the_kind() {
        let il = compute(&node_with_params(), panel());
        assert_eq!(il.rows.len(), 3);
        assert!(il.rows[0].editable && il.rows[1].editable);
        assert!(!il.rows[2].editable, "enum is shown, not editable, in v0");
    }

    #[test]
    fn row_hit_testing_round_trips_and_refuses_outside_the_panel() {
        let il = compute(&node_with_params(), panel());
        assert_eq!(il.row_at(il.rows[1].track.center()), Some(1));
        assert_eq!(il.row_at(il.rows[0].label.center()), Some(0));
        assert_eq!(il.row_at(Vec2::new(10.0, 10.0)), None, "outside the panel");
    }

    #[test]
    fn the_part_of_a_row_below_the_panel_bottom_is_not_touchable() {
        let n = node_with_params();
        // Panel bottom at y=190: title (116..160) fully visible, row 0 (160..204) clipped.
        let clipped = Rect::from_min_size(panel().min, Vec2::new(400.0, 90.0));
        let il = compute(&n, clipped);
        assert_eq!(il.rows.len(), 3, "rows are computed…");
        let x = il.rows[0].track.center().x;
        assert_eq!(
            il.row_at(Vec2::new(x, 165.0)),
            Some(0),
            "…and the VISIBLE sliver of a clipped row is touchable"
        );
        assert_eq!(
            il.row_at(Vec2::new(x, 195.0)),
            None,
            "the clipped-away part of the row is NOT — an invisible control must not be hittable"
        );
        assert_eq!(il.row_at(il.rows[1].track.center()), None, "row 1 is entirely below the clip");
    }

    #[test]
    fn value_mapping_clamps_and_round_trips_with_the_knob() {
        let d = desc("freq", ParamKind::Float, 0.0, 24_000.0, 440.0);
        let track = Rect::from_min_size(Vec2::new(100.0, 0.0), Vec2::new(200.0, 44.0));
        assert_eq!(value_from_x(&d, track, 100.0), 0.0);
        assert_eq!(value_from_x(&d, track, 300.0), 24_000.0);
        assert_eq!(value_from_x(&d, track, 500.0), 24_000.0, "past the end clamps");
        assert_eq!(value_from_x(&d, track, 200.0), 12_000.0);
        let x = knob_x(&d, track, 12_000.0);
        assert!((x - 200.0).abs() < 1e-3, "knob round-trips");
        assert_eq!(value_text(&d, 440.0), "440.00 Hz");
    }

    #[test]
    fn bool_value_text_names_the_poles() {
        let mut d = desc("on", ParamKind::Bool, 0.0, 1.0, 0.0);
        d.unit = None;
        assert_eq!(value_text(&d, 0.0), "OFF");
        assert_eq!(value_text(&d, 1.0), "ON");
    }

    #[test]
    fn a_stale_layout_refuses_to_read_a_different_node() {
        let n = node_with_params();
        let il = compute(&n, panel());
        let other = Node { id: n.id + 1, ..n.clone() };
        assert!(il.desc(&other, 0).is_none());
        assert!(il.desc(&n, 0).is_some());
    }

    // ------------------------------------------------------------ scrolling (increment 5)

    /// A 20-parameter node — the `util/mixer` shape the parked item named ("a 32-param module
    /// shows only its first ~10"): more rows than a real panel is tall.
    fn node_with_many(count: usize) -> Node {
        let params =
            (0..count).map(|i| desc(&format!("p{i}"), ParamKind::Float, 0.0, 1.0, 0.5)).collect();
        let spec = NodeSpec::new("sparq/util/mixer", "Mixer", vec![]).with_params(params);
        let mut g = Graph::new();
        g.op_add_node(spec, Vec2::ZERO);
        g.nodes()[0].clone()
    }

    /// A panel that fits 10 of the 20 rows: content hangs below, scrolling is real.
    fn short_panel() -> Rect {
        Rect::from_min_size(Vec2::new(800.0, 100.0), Vec2::new(400.0, 512.0))
    }

    #[test]
    fn compute_is_compute_at_zero_and_a_fitting_panel_has_nothing_to_scroll() {
        let n = node_with_params();
        assert_eq!(compute(&n, panel()), compute_at(&n, panel(), 0.0));
        let il = compute(&n, panel());
        assert_eq!(il.max_scroll, 0.0, "three rows fit the 600 px panel");
        assert_eq!(il.scroll, 0.0);
        assert!(il.thumb().is_none(), "a scrollbar that cannot move is not drawn");
        // A requested scroll on a fitting panel clamps to zero — no blank space, ever.
        assert_eq!(compute_at(&n, panel(), 400.0).scroll, 0.0);
    }

    #[test]
    fn a_long_panel_clamps_the_offset_and_the_bottom_rows_come_into_view() {
        let n = node_with_many(20);
        let il0 = compute_at(&n, short_panel(), 0.0);
        // content = title 44 + 20×44 = 924 below the title's top; the panel is 512 tall.
        assert!(il0.max_scroll > 0.0, "20 rows in a 10-row panel must scroll");
        assert!(!il0.row_visible(&il0.rows[19]), "the last row starts below the panel");
        assert_eq!(il0.visible_range(), (Some(0), Some(10)), "rows 0..=10 on screen");

        let il1 = compute_at(&n, short_panel(), il0.max_scroll);
        assert_eq!(il1.scroll, il0.max_scroll);
        assert!(il1.row_visible(&il1.rows[19]), "scrolled to the end: the last row is drawn");
        assert!(!il1.row_visible(&il1.rows[0]), "…and row 0 is under the header");
        assert_eq!(il1.visible_range().1, Some(19));

        let over = compute_at(&n, short_panel(), il0.max_scroll + 999.0);
        assert_eq!(over.scroll, il0.max_scroll, "the layout clamps — no over-scroll into blank");
        let under = compute_at(&n, short_panel(), -50.0);
        assert_eq!(under.scroll, 0.0, "negative requests clamp to the top");
        let nan = compute_at(&n, short_panel(), f32::NAN);
        assert_eq!(nan.scroll, 0.0, "NaN reads as the top, never propagates into geometry");
    }

    #[test]
    fn the_hit_test_follows_the_scroll_and_the_header_hides_what_it_covers() {
        let n = node_with_many(20);
        let il0 = compute_at(&n, short_panel(), 0.0);
        // Row 12 is below the fold: computed, but NOT touchable (increment 3's rule, unrelaxed).
        assert_eq!(il0.row_at(il0.rows[12].track.center()), None);

        // Scroll so row 12 is on screen: its track centre is now a live touch target.
        let il1 = compute_at(&n, short_panel(), il0.max_scroll);
        let probe = Vec2::new(il1.rows[12].track.center().x, il1.rows[12].track.center().y);
        assert!(il1.rect.contains(probe) && probe.y >= il1.title.max.y, "probe setup");
        assert_eq!(il1.row_at(probe), Some(12), "a scrolled-into-view row is touchable");

        // A row scrolled UNDER the fixed title is hidden, so it is refused — even where its
        // stored rect still overlaps the panel — while a bottom sliver stays touchable.
        let hidden = il1.rows.first().expect("row 0 exists");
        assert!(hidden.track.min.y < il1.title.max.y, "row 0 is under the header");
        let under_header = Vec2::new(hidden.track.center().x, il1.title.max.y - 2.0);
        assert_eq!(il1.row_at(under_header), None, "the header covers it: not touchable");
        let near_end = compute_at(&n, short_panel(), il0.max_scroll - 20.0);
        let last = near_end.rows.last().expect("row 19 exists");
        assert!(last.track.max.y > near_end.rect.max.y, "row 19 straddles the panel bottom");
        let sliver = Vec2::new(last.track.center().x, near_end.rect.max.y - 2.0);
        assert_eq!(near_end.row_at(sliver), Some(19), "a visible bottom sliver IS touchable");
    }

    #[test]
    fn the_thumb_tracks_the_scroll_proportionally() {
        let n = node_with_many(20);
        let top = compute_at(&n, short_panel(), 0.0);
        let thumb_top = top.thumb().expect("a scrollable panel draws its thumb");
        assert!((thumb_top.min.y - top.title.max.y).abs() < 1e-3, "at the top of the content");
        assert!(thumb_top.height() < (top.rect.max.y - top.title.max.y));
        assert!(thumb_top.height() >= LAYOUT_SPACE_4 as f32, "the thumb stays touch-visible");

        let bottom = compute_at(&n, short_panel(), top.max_scroll);
        let thumb_bottom = bottom.thumb().expect("still scrollable");
        assert!(
            (thumb_bottom.max.y - bottom.rect.max.y).abs() < 1e-3,
            "at max scroll the thumb ends at the panel bottom"
        );
        assert!(thumb_bottom.min.y > thumb_top.min.y, "the thumb moved down with the content");

        let mid = compute_at(&n, short_panel(), top.max_scroll / 2.0);
        let thumb_mid = mid.thumb().expect("mid");
        assert!(thumb_mid.min.y > thumb_top.min.y && thumb_mid.max.y < thumb_bottom.max.y);
        assert_eq!(
            (thumb_top.width(), thumb_mid.width(), thumb_bottom.width()),
            (LAYOUT_SPACE_1 as f32, LAYOUT_SPACE_1 as f32, LAYOUT_SPACE_1 as f32),
            "the thumb width is the token, at every offset"
        );
    }
}
