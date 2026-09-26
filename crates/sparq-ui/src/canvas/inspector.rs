//! The inspector: computed parameter-panel geometry for one selected node (WO-013 increment 3).
//!
//! Same contract as [`super::layout`]: pure computation from (node, panel rect) to row rects,
//! slider tracks and the x→value mapping. The painter draws exactly these rects; the gesture path
//! hit-tests exactly these rects; the audit measures exactly these rects. The drawn track is a
//! thin line, but the **touch target is the full row height** — the port-capture trick applied to
//! sliders, so a finger-sized grab never needs pixel aim.
//!
//! v0 limits, stated rather than hidden:
//! * no scrolling — rows below the panel bottom are clipped by the painter and **not touchable**
//!   ([`row_at`] refuses them), so an invisible control can never be hit by accident;
//! * only the kinds [`ParamDesc::editable`] names are adjustable; `enum`/`text`/`blob` rows are
//!   shown greyed with their kind named, because inventing an options editor before the manifest
//!   schema grows `options[]` would be a lie the user could act on.

use crate::canvas::model::{Node, NodeId, ParamDesc};
use crate::geom::{Rect, Vec2};
use crate::tokens::{LAYOUT_SPACE_PADDING_PANEL, LAYOUT_TOUCH_ROW_HEIGHT_LIST};

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
    /// The title row (node title + module id).
    pub title: Rect,
    /// One row per parameter, in manifest order.
    pub rows: Vec<ParamRow>,
}

/// Column splits of the content width, as fractions: label | track | value.
const LABEL_FRAC: f32 = 0.38;
const TRACK_FRAC: f32 = 0.40;

/// Compute the inspector panel for `node` inside `rect` (screen px, from the shell layout).
#[must_use]
pub fn compute(node: &Node, rect: Rect) -> InspectorLayout {
    let pad = LAYOUT_SPACE_PADDING_PANEL as f32;
    let row_h = LAYOUT_TOUCH_ROW_HEIGHT_LIST as f32;
    let content_x = rect.min.x + pad;
    let content_w = (rect.width() - 2.0 * pad).max(0.0);

    let title =
        Rect::from_min_size(Vec2::new(content_x, rect.min.y + pad), Vec2::new(content_w, row_h));

    let mut rows = Vec::with_capacity(node.spec.params.len());
    for (i, desc) in node.spec.params.iter().enumerate() {
        let y = title.max.y + i as f32 * row_h;
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
    InspectorLayout { node: node.id, rect, title, rows }
}

impl InspectorLayout {
    /// The row a screen point lands on, if any. Rows (or parts of rows) outside the panel rect do
    /// not count — an invisible control must not be touchable.
    #[must_use]
    pub fn row_at(&self, pos: Vec2) -> Option<usize> {
        if !self.rect.contains(pos) {
            return None;
        }
        self.rows.iter().position(|r| r.track.contains(pos) || r.label.contains(pos))
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
}
