//! The native↔WIT conversions and the D13 display-list interchange writer.
//!
//! Three families, one rule each:
//!
//! * **Records** ([`record_to_wit`]) — the frozen `data-record` vocabulary has exactly two
//!   mirrors (INC1's native `sparq_streams::record`, the WIT-generated types); this is the mapping
//!   between them, field-for-field, total over both. The sources door is its only caller and the
//!   golden-render test is its drift gate: a mapping that dropped a field would move the hash.
//! * **Host inputs** ([`resources_to_wit`], [`params_defaults_from_manifest`]) — what `prepare`
//!   and the param mirror need, derived from the manifest, never hand-typed.
//! * **The interchange** ([`surface_to_ir_json`]) — a WIT `surface` to the D13 JSON, BYTE-EQUAL
//!   to what the observatory harness's `ir_json` writes for the same render. Same keys, same
//!   order-free BTreeMap sort (serde_json's `Map`), same float rule (`Number::from_f64(f64::from(x))`,
//!   non-finite → `null` — the no-cell sentinel), same enum spellings (kebab → snake). Two
//!   producers of one format is a drift risk by construction, so the drift gate is the pinned
//!   golden sha itself: stage 4 hashes THIS writer's output over the wasm path and compares
//!   against the harness's pin — a disagreement fails the hand-in, in words.
//!
//! The sha256 here is the house hand-rolled one (the golden tests' precedent, known-answer
//! vectors in the tests below) — the third copy in the tree is deliberate: a hash used as a gate
//! must not depend on the crate whose output it gates.

use serde_json::{Map, Value};

use super::bindings::sparq::instrument::{display, types};

/// Maps one native data-record to its WIT twin (field-for-field, total).
#[must_use]
pub fn record_to_wit(r: &sparq_streams::record::DataRecord) -> types::DataRecord {
    types::DataRecord {
        schema_id: r.schema_id.clone(),
        schema_version: r.schema_version,
        channels: r.channels.iter().map(value_to_wit).collect(),
        t_wall_ns: r.t_wall_ns,
        t_sample: r.t_sample,
        seq: r.seq,
        record_flags: flags_to_wit(r.record_flags),
    }
}

/// Maps one native data-value to its WIT twin.
#[must_use]
pub fn value_to_wit(v: &sparq_streams::record::DataValue) -> types::DataValue {
    use sparq_streams::record::DataValue as N;
    match v {
        N::Single(f) => types::DataValue::Single(*f),
        N::Double(f) => types::DataValue::Double(*f),
        N::Int32(i) => types::DataValue::Int32(*i),
        N::Int64(i) => types::DataValue::Int64(*i),
        N::Boolean(b) => types::DataValue::Boolean(*b),
        N::Vec(xs) => types::DataValue::Vec(xs.clone()),
        N::Enumerated(i) => types::DataValue::Enumerated(*i),
        N::Text(s) => types::DataValue::Text(s.clone()),
        N::Blob(b) => types::DataValue::Blob(b.clone()),
    }
}

/// Maps native freshness flags to the WIT twin.
#[must_use]
pub fn flags_to_wit(f: sparq_streams::record::DataFlags) -> types::DataFlags {
    use sparq_streams::record::DataFlags as N;
    match f {
        N::Ok => types::DataFlags::Ok,
        N::Stale => types::DataFlags::Stale,
        N::Discontinuity => types::DataFlags::Discontinuity,
        N::Estimated => types::DataFlags::Estimated,
    }
}

/// Maps the native oversampling vocabulary to the WIT twin (1:1 by name).
#[must_use]
pub fn oversampling_to_wit(o: sparq_module_api::module::Oversampling) -> types::Oversampling {
    use sparq_module_api::module::Oversampling as N;
    match o {
        N::None => types::Oversampling::None,
        N::X2 => types::Oversampling::X2,
        N::X4 => types::Oversampling::X4,
        N::X8 => types::Oversampling::X8,
    }
}

/// Builds the WIT `resources` for `prepare` from the native resolution plus the manifest's
/// declared fuel budget and the token bundle's version (instrument-host §2.4: `fuel-per-block`
/// IS the manifest's `max_fuel`).
#[must_use]
pub fn resources_to_wit(
    res: &sparq_module_api::module::Resources,
    fuel_per_block: u64,
    token_bundle: &types::Semver,
) -> types::Resources {
    types::Resources {
        sample_rate: res.sample_rate,
        block_frames: u32::try_from(res.block_frames).unwrap_or(u32::MAX),
        audio_in_channels: res.audio_in_channels[..res.audio_in_count]
            .iter()
            .map(|&c| u32::try_from(c).unwrap_or(u32::MAX))
            .collect(),
        audio_out_channels: res.audio_out_channels[..res.audio_out_count]
            .iter()
            .map(|&c| u32::try_from(c).unwrap_or(u32::MAX))
            .collect(),
        oversampling: oversampling_to_wit(res.oversampling),
        voices: res.voices,
        arena_bytes: u64::try_from(res.arena_bytes).unwrap_or(u64::MAX),
        fuel_per_block,
        token_bundle: token_bundle.clone(),
    }
}

/// The manifest's `[[params]]` defaults, in declaration order — the host-side param snapshot a
/// freshly loaded instance starts with (and what stage 4 draws with: the golden is the DEFAULT
/// wall). Floats are the raw domain values (the native precedent); bools ride 0/1; a missing
/// default is 0.
///
/// # Errors
/// A sentence naming the parse defect.
pub fn params_defaults_from_manifest(text: &str) -> Result<Vec<f32>, String> {
    let root = sparq_module_api::toml::parse(text).map_err(|e| format!("manifest TOML: {e}"))?;
    let rows = root.tables("params").unwrap_or_default();
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let default =
            row.get("default").and_then(sparq_module_api::toml::Value::as_f64).unwrap_or(0.0);
        out.push(default as f32);
    }
    Ok(out)
}

// ── the D13 interchange writer (byte-equal to the harness's ir_json) ──────────────────────────

/// A whole surface to the interchange JSON, with the display rect it was laid out against (the
/// at-rest store refuses a list without its rect — the harness's rule, carried).
#[must_use]
pub fn surface_to_ir_json(s: &display::Surface, display_w: f32, display_h: f32) -> Value {
    let mut m = Map::new();
    match s {
        display::Surface::Items(items) => {
            m.insert("surface".into(), Value::String("items".into()));
            m.insert("display_w".into(), num(display_w));
            m.insert("display_h".into(), num(display_h));
            m.insert("items".into(), Value::Array(items.iter().map(item_to_ir_json).collect()));
        },
        display::Surface::Scene(_) => {
            // The Observatory never builds a scene in v1; the tag serialises honestly with an
            // empty element list (the harness's rule — total over the variant).
            m.insert("surface".into(), Value::String("scene".into()));
            m.insert("display_w".into(), num(display_w));
            m.insert("display_h".into(), num(display_h));
            m.insert("elements".into(), Value::Array(Vec::new()));
        },
    }
    Value::Object(m)
}

/// One WIT item to the interchange JSON (kind spellings: kebab → snake, the harness's rule).
#[must_use]
pub fn item_to_ir_json(item: &display::Item) -> Value {
    let mut m = Map::new();
    match item {
        display::Item::Polyline(p) => {
            m.insert("kind".into(), Value::String("polyline".into()));
            m.insert("points".into(), points_to_json(&p.points));
            m.insert("style".into(), style_to_json(&p.style));
        },
        display::Item::Path(p) => {
            m.insert("kind".into(), Value::String("path".into()));
            m.insert(
                "segments".into(),
                Value::Array(
                    p.segments
                        .iter()
                        .map(|s| {
                            let mut sm = Map::new();
                            match s {
                                display::PathSegment::MoveTo(pt) => {
                                    sm.insert("op".into(), Value::String("move".into()));
                                    sm.insert("to".into(), point_to_json(pt));
                                },
                                display::PathSegment::LineTo(pt) => {
                                    sm.insert("op".into(), Value::String("line".into()));
                                    sm.insert("to".into(), point_to_json(pt));
                                },
                                display::PathSegment::QuadTo((c, to)) => {
                                    sm.insert("op".into(), Value::String("quad".into()));
                                    sm.insert("c".into(), point_to_json(c));
                                    sm.insert("to".into(), point_to_json(to));
                                },
                                display::PathSegment::CubicTo((c1, c2, to)) => {
                                    sm.insert("op".into(), Value::String("cubic".into()));
                                    sm.insert("c1".into(), point_to_json(c1));
                                    sm.insert("c2".into(), point_to_json(c2));
                                    sm.insert("to".into(), point_to_json(to));
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
        display::Item::Rect(r) => {
            m.insert("kind".into(), Value::String("rect".into()));
            m.insert("x".into(), num(r.x));
            m.insert("y".into(), num(r.y));
            m.insert("w".into(), num(r.w));
            m.insert("h".into(), num(r.h));
            m.insert("style".into(), style_to_json(&r.style));
        },
        display::Item::Arc(a) => {
            m.insert("kind".into(), Value::String("arc".into()));
            m.insert("cx".into(), num(a.cx));
            m.insert("cy".into(), num(a.cy));
            m.insert("r".into(), num(a.r));
            m.insert("a0_deg".into(), num(a.a0_deg));
            m.insert("a1_deg".into(), num(a.a1_deg));
            m.insert("style".into(), style_to_json(&a.style));
        },
        display::Item::GlyphRun(g) => {
            m.insert("kind".into(), Value::String("glyph_run".into()));
            m.insert("text".into(), Value::String(g.text.clone()));
            m.insert("x".into(), num(g.x));
            m.insert("y".into(), num(g.y));
            m.insert("size".into(), Value::String(g.size.clone()));
            m.insert("colour".into(), Value::String(g.colour.clone()));
        },
        display::Item::Points(p) => {
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
        display::Item::HeatCells(h) => {
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
        display::Item::Trace(t) => {
            m.insert("kind".into(), Value::String("trace".into()));
            m.insert("values".into(), Value::Array(t.values.iter().map(|x| num(*x)).collect()));
            m.insert("colour".into(), Value::String(t.colour.clone()));
            m.insert("width".into(), Value::String(width_str(&t.width).into()));
            m.insert("phosphor".into(), Value::Bool(t.phosphor));
        },
    }
    Value::Object(m)
}

fn style_to_json(s: &display::Style) -> Value {
    let mut m = Map::new();
    m.insert("stroke".into(), opt_str(s.stroke.as_deref()));
    m.insert("stroke_width".into(), Value::String(width_str(&s.stroke_width).into()));
    m.insert("fill".into(), opt_str(s.fill.as_deref()));
    m.insert("dash".into(), Value::String(dash_str(&s.dash).into()));
    m.insert("corner".into(), Value::String(corner_str(&s.corner).into()));
    Value::Object(m)
}

fn points_to_json(pts: &[display::Point]) -> Value {
    Value::Array(pts.iter().map(point_to_json).collect())
}

fn point_to_json(p: &display::Point) -> Value {
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

/// A float as a JSON number; non-finite → `null` (the no-cell sentinel — the harness's exact
/// rule, so heat grids with empty cells hash identically on both sides of the boundary).
fn num(x: f32) -> Value {
    if x.is_finite() {
        serde_json::Number::from_f64(f64::from(x)).map_or(Value::Null, Value::Number)
    } else {
        Value::Null
    }
}

fn width_str(w: &display::StrokeWidth) -> &'static str {
    match w {
        display::StrokeWidth::W1 => "w1",
        display::StrokeWidth::W2 => "w2",
        display::StrokeWidth::W3 => "w3",
    }
}

fn dash_str(d: &display::Dash) -> &'static str {
    match d {
        display::Dash::Solid => "solid",
        display::Dash::Dashed => "dashed",
        display::Dash::Dotted => "dotted",
        display::Dash::Hidden => "hidden",
    }
}

fn corner_str(c: &display::Corner) -> &'static str {
    match c {
        display::Corner::C0 => "c0",
        display::Corner::C2 => "c2",
        display::Corner::C4 => "c4",
    }
}

/// The interchange, pretty-printed — the golden's subject (the harness hashes
/// `serde_json::to_string_pretty` of the same value; two-space indent, serde_json's formatter).
///
/// # Errors
/// A serde_json serialisation failure (cannot happen for a value built here; kept as words
/// rather than a panic because the caller is a gate).
pub fn ir_json_pretty(v: &Value) -> Result<String, String> {
    serde_json::to_string_pretty(v).map_err(|e| format!("IR serialise: {e}"))
}

/// The house sha256 (hand-rolled, known-answer tested — the golden tests' precedent).
#[must_use]
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = bytes.to_vec();
    let bitlen = (bytes.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[4 * i],
                chunk[4 * i + 1],
                chunk[4 * i + 2],
                chunk[4 * i + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            (hh, g, f, e) = (g, f, e, d.wrapping_add(t1));
            (d, c, b, a) = (c, b, a, t1.wrapping_add(t2));
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[4 * i..4 * i + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

/// The hex spelling of a sha256.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    sha256(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn the_sha256_matches_the_published_vectors() {
        // The same known-answer vectors every house copy carries (FIPS 180-4).
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn records_map_field_for_field() {
        let native = sparq_streams::record::DataRecord {
            schema_id: "observatory/timeseries".into(),
            schema_version: 1,
            channels: vec![
                sparq_streams::record::DataValue::Double(1.5),
                sparq_streams::record::DataValue::Text("hi".into()),
            ],
            t_wall_ns: 123,
            t_sample: None,
            seq: 7,
            record_flags: sparq_streams::record::DataFlags::Stale,
        };
        let w = record_to_wit(&native);
        assert_eq!(w.schema_id, "observatory/timeseries");
        assert_eq!(w.schema_version, 1);
        assert_eq!(w.t_wall_ns, 123);
        assert_eq!(w.seq, 7);
        assert!(matches!(w.record_flags, types::DataFlags::Stale));
        assert!(matches!(w.channels[0], types::DataValue::Double(x) if x == 1.5));
        assert!(matches!(&w.channels[1], types::DataValue::Text(s) if s == "hi"));
    }

    #[test]
    fn the_manifest_defaults_decode_in_order() {
        // Two params with defaults, one without (→ 0): the positional snapshot's rule.
        let text = r#"
[identity]
id = "dat/test"

[[params]]
id = "a"
type = "float"
default = 1.5

[[params]]
id = "b"
type = "bool"
default = 1.0

[[params]]
id = "c"
type = "float"
"#;
        assert_eq!(params_defaults_from_manifest(text).unwrap(), vec![1.5, 1.0, 0.0]);
    }

    #[test]
    fn the_interchange_matches_the_harness_spellings() {
        // One of each shape the writers share; the golden sha is the real gate, this pins the
        // local rules (snake kinds, null sentinels, style object).
        let items = vec![
            display::Item::Rect(display::RectItem {
                x: 1.0,
                y: 2.0,
                w: 3.0,
                h: 4.0,
                style: display::Style {
                    stroke: None,
                    stroke_width: display::StrokeWidth::W1,
                    fill: Some("color.ground.inset".into()),
                    dash: display::Dash::Solid,
                    corner: display::Corner::C2,
                },
            }),
            display::Item::HeatCells(display::HeatCellsItem {
                x: 0.0,
                y: 0.0,
                w: 8.0,
                h: 4.0,
                cols: 2,
                rows: 1,
                values: vec![0.5, f32::NAN],
                colormap: "colormap.thermal".into(),
            }),
        ];
        let v = surface_to_ir_json(&display::Surface::Items(items), 2176.0, 1120.0);
        let s = ir_json_pretty(&v).unwrap();
        assert!(s.contains("\"surface\": \"items\""), "{s}");
        assert!(s.contains("\"display_w\": 2176.0"), "{s}");
        assert!(s.contains("\"kind\": \"rect\""), "{s}");
        assert!(s.contains("\"kind\": \"heat_cells\""), "{s}");
        assert!(s.contains("\"corner\": \"c2\""), "{s}");
        assert!(s.contains("null"), "the NaN cell is JSON null: {s}");
    }
}
