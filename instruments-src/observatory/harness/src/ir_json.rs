//! The display-list JSON interchange (plan D13): the core's [`Item`]s serialised to a stable JSON
//! form.
//!
//! This is the artefact that lets INC4's host painters render the instrument's REAL output in the
//! sandbox with no wasmtime anywhere, and it is what the golden-IR tests hash. It is an interchange
//! format, NOT a contract change — the frozen surface stays `display.wit`; this is just a text
//! rendering of the same IR so two painters (this harness's SVG writer, INC4's `displaylist.rs`) and
//! the goldens can all read one shape.
//!
//! Determinism: the JSON is built with `serde_json::Map` (a BTreeMap — keys sort), arrays keep order,
//! and floats go through serde_json's deterministic formatter, so the same IR always serialises to the
//! same bytes and hashes stably (the golden's whole requirement).

use observatory_core::ir::{Corner, Dash, Item, StrokeWidth, Style, Surface};
use serde_json::{Map, Value};

/// Serialises a whole surface, together with the display rect it was laid out against (the
/// at-rest store and the host painters need the rect to place the list in a band — an interchange
/// without it is refused in words by `sparq_app::ui::atrest`).
#[must_use]
pub fn surface_to_json(s: &Surface, display_w: f32, display_h: f32) -> Value {
    match s {
        Surface::Items(items) => {
            let mut m = Map::new();
            m.insert("surface".into(), Value::String("items".into()));
            m.insert("display_w".into(), num(display_w));
            m.insert("display_h".into(), num(display_h));
            m.insert("items".into(), Value::Array(items.iter().map(item_to_json).collect()));
            Value::Object(m)
        },
        Surface::Scene(_) => {
            // The Observatory never builds a scene in v1; serialise the tag honestly (an empty
            // element list) so the interchange is total over the Surface variant.
            let mut m = Map::new();
            m.insert("surface".into(), Value::String("scene".into()));
            m.insert("display_w".into(), num(display_w));
            m.insert("display_h".into(), num(display_h));
            m.insert("elements".into(), Value::Array(Vec::new()));
            Value::Object(m)
        },
    }
}

/// Serialises one item. The `kind` discriminant matches the WIT variant spellings (kebab → snake).
#[must_use]
pub fn item_to_json(item: &Item) -> Value {
    let mut m = Map::new();
    match item {
        Item::Polyline(p) => {
            m.insert("kind".into(), Value::String("polyline".into()));
            m.insert("points".into(), points_to_json(&p.points));
            m.insert("style".into(), style_to_json(&p.style));
        },
        Item::Path(p) => {
            m.insert("kind".into(), Value::String("path".into()));
            m.insert(
                "segments".into(),
                Value::Array(
                    p.segments
                        .iter()
                        .map(|s| {
                            let mut sm = Map::new();
                            match s {
                                observatory_core::ir::PathSegment::MoveTo(pt) => {
                                    sm.insert("op".into(), Value::String("move".into()));
                                    sm.insert("to".into(), point_to_json(*pt));
                                },
                                observatory_core::ir::PathSegment::LineTo(pt) => {
                                    sm.insert("op".into(), Value::String("line".into()));
                                    sm.insert("to".into(), point_to_json(*pt));
                                },
                                observatory_core::ir::PathSegment::QuadTo(a, b) => {
                                    sm.insert("op".into(), Value::String("quad".into()));
                                    sm.insert("c".into(), point_to_json(*a));
                                    sm.insert("to".into(), point_to_json(*b));
                                },
                                observatory_core::ir::PathSegment::CubicTo(a, b, c) => {
                                    sm.insert("op".into(), Value::String("cubic".into()));
                                    sm.insert("c1".into(), point_to_json(*a));
                                    sm.insert("c2".into(), point_to_json(*b));
                                    sm.insert("to".into(), point_to_json(*c));
                                },
                            }
                            Value::Object(sm)
                        })
                        .collect(),
                ),
            );
            m.insert("closed".into(), Value::Bool(p.closed));
            m.insert("style".into(), style_to_json(&p.style));
        },
        Item::Rect(r) => {
            m.insert("kind".into(), Value::String("rect".into()));
            m.insert("x".into(), num(r.x));
            m.insert("y".into(), num(r.y));
            m.insert("w".into(), num(r.w));
            m.insert("h".into(), num(r.h));
            m.insert("style".into(), style_to_json(&r.style));
        },
        Item::Arc(a) => {
            m.insert("kind".into(), Value::String("arc".into()));
            m.insert("cx".into(), num(a.cx));
            m.insert("cy".into(), num(a.cy));
            m.insert("r".into(), num(a.r));
            m.insert("a0_deg".into(), num(a.a0_deg));
            m.insert("a1_deg".into(), num(a.a1_deg));
            m.insert("style".into(), style_to_json(&a.style));
        },
        Item::GlyphRun(g) => {
            m.insert("kind".into(), Value::String("glyph_run".into()));
            m.insert("text".into(), Value::String(g.text.clone()));
            m.insert("x".into(), num(g.x));
            m.insert("y".into(), num(g.y));
            m.insert("size".into(), Value::String(g.size.clone()));
            m.insert("colour".into(), Value::String(g.colour.clone()));
        },
        Item::Points(p) => {
            m.insert("kind".into(), Value::String("points".into()));
            m.insert("positions".into(), points_to_json(&p.positions));
            m.insert("size".into(), Value::String(p.size.clone()));
            m.insert("colour".into(), opt_str(p.colour.as_deref()));
            m.insert("colormap".into(), opt_str(p.colormap.as_deref()));
            m.insert(
                "values".into(),
                match &p.values {
                    Some(v) => Value::Array(v.iter().map(|x| num(*x)).collect()),
                    None => Value::Null,
                },
            );
        },
        Item::HeatCells(h) => {
            m.insert("kind".into(), Value::String("heat_cells".into()));
            m.insert("x".into(), num(h.x));
            m.insert("y".into(), num(h.y));
            m.insert("w".into(), num(h.w));
            m.insert("h".into(), num(h.h));
            m.insert("cols".into(), Value::from(h.cols));
            m.insert("rows".into(), Value::from(h.rows));
            m.insert("values".into(), Value::Array(h.values.iter().map(|x| num(*x)).collect()));
            m.insert("colormap".into(), Value::String(h.colormap.clone()));
        },
        Item::Trace(t) => {
            m.insert("kind".into(), Value::String("trace".into()));
            m.insert("values".into(), Value::Array(t.values.iter().map(|x| num(*x)).collect()));
            m.insert("colour".into(), Value::String(t.colour.clone()));
            m.insert("width".into(), Value::String(width_str(t.width).into()));
            m.insert("phosphor".into(), Value::Bool(t.phosphor));
        },
    }
    Value::Object(m)
}

fn style_to_json(s: &Style) -> Value {
    let mut m = Map::new();
    m.insert("stroke".into(), opt_str(s.stroke.as_deref()));
    m.insert("stroke_width".into(), Value::String(width_str(s.stroke_width).into()));
    m.insert("fill".into(), opt_str(s.fill.as_deref()));
    m.insert("dash".into(), Value::String(dash_str(s.dash).into()));
    m.insert("corner".into(), Value::String(corner_str(s.corner).into()));
    Value::Object(m)
}

fn points_to_json(pts: &[observatory_core::ir::Point]) -> Value {
    Value::Array(pts.iter().map(|p| point_to_json(*p)).collect())
}

fn point_to_json(p: observatory_core::ir::Point) -> Value {
    let mut m = Map::new();
    m.insert("x".into(), num(p.x));
    m.insert("y".into(), num(p.y));
    Value::Object(m)
}

fn opt_str(s: Option<&str>) -> Value {
    match s {
        Some(v) => Value::String(v.into()),
        None => Value::Null,
    }
}

/// A float as a JSON number. Non-finite values (NaN/infinity — the no-cell sentinel in heat values)
/// are not representable in JSON, so they serialise as `null` (and the painter skips them, matching
/// the contract's NaN = no cell). This keeps the interchange valid JSON without losing the sentinel's
/// meaning (null == no cell).
fn num(x: f32) -> Value {
    if x.is_finite() {
        serde_json::Number::from_f64(f64::from(x)).map_or(Value::Null, Value::Number)
    } else {
        Value::Null
    }
}

fn width_str(w: StrokeWidth) -> &'static str {
    match w {
        StrokeWidth::W1 => "w1",
        StrokeWidth::W2 => "w2",
        StrokeWidth::W3 => "w3",
    }
}

fn dash_str(d: Dash) -> &'static str {
    match d {
        Dash::Solid => "solid",
        Dash::Dashed => "dashed",
        Dash::Dotted => "dotted",
        Dash::Hidden => "hidden",
    }
}

fn corner_str(c: Corner) -> &'static str {
    match c {
        Corner::C0 => "c0",
        Corner::C2 => "c2",
        Corner::C4 => "c4",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use observatory_core::ir::{GlyphRunItem, HeatCellsItem, Point, RectItem};

    #[test]
    fn a_rect_serialises_with_its_style() {
        let it = Item::Rect(RectItem {
            x: 1.0,
            y: 2.0,
            w: 3.0,
            h: 4.0,
            style: Style::fill("color.ground.inset"),
        });
        let v = item_to_json(&it);
        assert_eq!(v["kind"], "rect");
        assert_eq!(v["w"], 3.0);
        assert_eq!(v["style"]["fill"], "color.ground.inset");
        assert_eq!(v["style"]["stroke"], Value::Null);
    }

    #[test]
    fn a_nan_heat_value_serialises_as_null() {
        let it = Item::HeatCells(HeatCellsItem {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            cols: 2,
            rows: 1,
            values: vec![0.5, f32::NAN],
            colormap: "colormap.thermal".into(),
        });
        let v = item_to_json(&it);
        assert_eq!(
            v["values"][1],
            Value::Null,
            "NaN (no-cell) is JSON null, not an invalid number"
        );
        assert_eq!(v["values"][0], 0.5);
    }

    #[test]
    fn serialisation_is_deterministic() {
        let it = Item::GlyphRun(GlyphRunItem {
            text: "hi".into(),
            x: 1.0,
            y: 2.0,
            size: "typography.scale.s".into(),
            colour: "color.text.primary".into(),
        });
        let a = serde_json::to_string(&item_to_json(&it)).unwrap();
        let b = serde_json::to_string(&item_to_json(&it)).unwrap();
        assert_eq!(a, b, "the same IR serialises to the same bytes (the golden's requirement)");
    }

    #[test]
    fn json_keys_sort_for_stability() {
        // A BTreeMap-backed object sorts keys, so two logically-equal items serialise identically.
        let p = Item::Polyline(observatory_core::ir::PolylineItem {
            points: vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)],
            style: Style::trace("color.signal.data.colour"),
        });
        let s = serde_json::to_string(&item_to_json(&p)).unwrap();
        assert!(s.contains("\"style\""), "style present");
        assert!(s.contains("\"points\""), "points present");
    }
}
