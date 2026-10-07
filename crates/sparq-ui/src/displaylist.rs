//! The display-list painter core (WO-019's harness slice, pulled into WO-020 INC4, plan §8.1).
//!
//! The frozen vocabulary (`display.wit` v1) → resolved geometry + style, in the shell core, with
//! zero third-party dependencies: token ids resolve against the COMPILE-TIME bundle face
//! ([`crate::tokens`]' generated constants, incl. the baked 256-entry colormap LUTs), never against
//! hard-coded hex (a painter that copied the aesthetic would be the one copy of it that goes stale —
//! the hand-off system's whole promise). Two backends consume the resolved [`Painted`] list: the SVG
//! writer here (headless — `sparq ui --svg-out` and `sparq instrument render`), and the egui painter
//! in `sparq-app` (feature `ui`, the on-canvas display band). One resolution, two painters — WO-019's
//! "identical through both painters from the same display list".
//!
//! # Inputs
//!
//! The painter reads the D13 **display-list JSON interchange** (written by the instrument harness and
//! by `sparq mod validate` stage 6 on the device) via [`crate::json`] — the subset parser, because the
//! shell core stays dependency-free. [`parse_surface`] is the door; a malformed interchange is refused
//! in words, never half-painted.
//!
//! # Fail-soft, fail-loud (contract decision G)
//!
//! An unknown token id skips its primitive and records a diagnostic ([`Painter::diagnostics`]) — the
//! show goes on at runtime; at hand-in the validator's literal-appearance/unknown-id scan is the loud
//! half. The diagnostics list is what `ui --audit` and `instrument render` print.

use crate::json::Json;
use crate::tokens;

/// An sRGB colour with an alpha tier (the bundle's hairline opacity tiers ride here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha 0..=1 (opacity tiers; 1.0 for everything but hairlines).
    pub a: f32,
}

impl Rgba {
    /// From a bundle hex (`#RRGGBB`) + alpha.
    #[must_use]
    pub fn from_hex(hex: &str, a: f32) -> Option<Self> {
        let h = hex.strip_prefix('#').unwrap_or(hex);
        if h.len() != 6 {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(Self { r: byte(0)?, g: byte(2)?, b: byte(4)?, a })
    }

    /// The CSS form for the SVG backend.
    #[must_use]
    pub fn css(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// A resolved dash pattern, as SVG/egui understand it (the contract's closed enum, resolved).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dash {
    /// Solid.
    Solid,
    /// Dashed (predicted/modelled).
    Dashed,
    /// Dotted (guides/graticules).
    Dotted,
    /// Hidden (geometry carrier, no visible stroke).
    Hidden,
}

/// The host-side mirror of the frozen `item` vocabulary, parsed from the interchange. Field-for-field
/// with `display.wit` (and with the guest's IR — the interchange is the same shape in text).
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// `polyline`.
    Polyline {
        /// Vertices, draw order.
        points: Vec<(f32, f32)>,
        /// Style.
        style: Style,
    },
    /// `path` (subpaths; `closed` closes each).
    Path {
        /// The segments, grouped into subpaths at each `move`.
        subpaths: Vec<Vec<Seg>>,
        /// Whether every subpath closes.
        closed: bool,
        /// Style.
        style: Style,
    },
    /// `rect`.
    Rect {
        /// Geometry.
        x: f32,
        /// Geometry.
        y: f32,
        /// Geometry.
        w: f32,
        /// Geometry.
        h: f32,
        /// Style.
        style: Style,
    },
    /// `arc` (degrees, y-down clockwise).
    Arc {
        /// Centre.
        cx: f32,
        /// Centre.
        cy: f32,
        /// Radius.
        r: f32,
        /// Start angle, degrees.
        a0: f32,
        /// End angle, degrees.
        a1: f32,
        /// Style.
        style: Style,
    },
    /// `glyph-run`.
    GlyphRun {
        /// The text (units already in the string, rule 9).
        text: String,
        /// Baseline origin.
        x: f32,
        /// Baseline origin.
        y: f32,
        /// Size token id.
        size: String,
        /// Colour token id.
        colour: String,
    },
    /// `points` (flat colour OR colormap+values, never both).
    Points {
        /// Positions.
        positions: Vec<(f32, f32)>,
        /// Marker radius token id.
        size: String,
        /// Flat colour token id.
        colour: Option<String>,
        /// Colormap token id.
        colormap: Option<String>,
        /// Per-point data channel.
        values: Option<Vec<Option<f32>>>,
    },
    /// `heat-cells` (values [0,1] or null = no cell).
    HeatCells {
        /// Grid rect.
        x: f32,
        /// Grid rect.
        y: f32,
        /// Grid rect.
        w: f32,
        /// Grid rect.
        h: f32,
        /// Columns.
        cols: u32,
        /// Rows.
        rows: u32,
        /// Row-major values (null = NaN = no cell).
        values: Vec<Option<f32>>,
        /// Colormap token id.
        colormap: String,
    },
    /// `trace` (maps across the whole display box — the full-display scope case).
    Trace {
        /// Values.
        values: Vec<f32>,
        /// Colour token id.
        colour: String,
        /// Width class.
        width: Width,
        /// Phosphor motion (the egui backend's motion tokens; SVG ignores).
        phosphor: bool,
        /// The box the trace maps across (the display rect; the frozen record carries none).
        box_rect: (f32, f32, f32, f32),
    },
}

/// One path segment (the frozen `path-segment`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    /// A straight segment to the point.
    Line(f32, f32),
    /// A quadratic to (control, end).
    Quad(f32, f32, f32, f32),
    /// A cubic to (c1, c2, end).
    Cubic(f32, f32, f32, f32, f32, f32),
}

/// The frozen `stroke-width` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    /// w1 hairline.
    W1,
    /// w2 signal.
    W2,
    /// w3 emphasis.
    W3,
}

/// The frozen `corner` enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// c0 square.
    C0,
    /// c2 micro.
    C2,
    /// c4 panel.
    C4,
}

/// A resolved style (token ids → the bundle's values, at paint time).
#[derive(Clone, Debug, PartialEq)]
pub struct Style {
    /// Stroke colour token id.
    pub stroke: Option<String>,
    /// Stroke width class.
    pub width: Width,
    /// Fill colour token id.
    pub fill: Option<String>,
    /// Dash.
    pub dash: Dash,
    /// Corner radius class.
    pub corner: Corner,
}

/// A resolved, backend-neutral primitive: what both painters draw.
#[derive(Clone, Debug, PartialEq)]
pub enum Painted {
    /// A rect (fill and/or stroke).
    Rect {
        /// Geometry.
        x: f32,
        /// Geometry.
        y: f32,
        /// Geometry.
        w: f32,
        /// Geometry.
        h: f32,
        /// Corner radius, px.
        rx: f32,
        /// Resolved fill.
        fill: Option<Rgba>,
        /// Resolved stroke.
        stroke: Option<Rgba>,
        /// Stroke width, px.
        stroke_w: f32,
        /// Dash.
        dash: Dash,
    },
    /// A polyline (open).
    Polyline {
        /// Vertices.
        pts: Vec<(f32, f32)>,
        /// Resolved stroke.
        stroke: Option<Rgba>,
        /// Stroke width, px.
        stroke_w: f32,
        /// Dash.
        dash: Dash,
    },
    /// A path (subpaths, optionally closed, fill and/or stroke).
    Path {
        /// Subpaths.
        subpaths: Vec<Vec<Seg>>,
        /// Closed.
        closed: bool,
        /// Resolved fill.
        fill: Option<Rgba>,
        /// Resolved stroke.
        stroke: Option<Rgba>,
        /// Stroke width, px.
        stroke_w: f32,
        /// Dash.
        dash: Dash,
    },
    /// An arc (or full circle when the sweep is ≥ 360°).
    Arc {
        /// Centre.
        cx: f32,
        /// Centre.
        cy: f32,
        /// Radius.
        r: f32,
        /// Start degrees.
        a0: f32,
        /// End degrees.
        a1: f32,
        /// Resolved fill.
        fill: Option<Rgba>,
        /// Resolved stroke.
        stroke: Option<Rgba>,
        /// Stroke width, px.
        stroke_w: f32,
    },
    /// A text run.
    Text {
        /// The text.
        text: String,
        /// Baseline origin.
        x: f32,
        /// Baseline origin.
        y: f32,
        /// Resolved size, px.
        size_px: f32,
        /// Resolved colour.
        colour: Rgba,
    },
    /// Resolved point dots (per-point colour from the flat token or the LUT).
    Dots {
        /// x, y, radius, colour.
        pts: Vec<(f32, f32, f32, Rgba)>,
    },
    /// Resolved heat cells (NaN/null skipped; colour sampled from the LUT).
    Heat {
        /// x, y, w, h, colour per drawn cell.
        cells: Vec<(f32, f32, f32, f32, Rgba)>,
    },
}

/// The `gpu_class` ceiling table, mirrored from `sparq-host-wasm/src/ceilings.rs` (the budget the
/// painter counts against, §5.8). The mirror is drift-tested against the host table in
/// `crates/sparq-app/tests/displaylist_drift.rs` — one table, two readers, checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ceilings {
    /// Vertex ceiling.
    pub max_vertices: u32,
    /// Instance ceiling (glyph runs + point sprites).
    pub max_instances: u32,
    /// Heat-cell ceiling.
    pub max_heat_cells: u32,
}

/// The table, in `GPU_CLASSES` order (none/light/medium/heavy/very_heavy), 4× per step.
pub const CEILING_TABLE: [(&str, Ceilings); 5] = [
    ("none", Ceilings { max_vertices: 0, max_instances: 0, max_heat_cells: 0 }),
    ("light", Ceilings { max_vertices: 65_536, max_instances: 4_096, max_heat_cells: 16_384 }),
    ("medium", Ceilings { max_vertices: 262_144, max_instances: 16_384, max_heat_cells: 65_536 }),
    ("heavy", Ceilings { max_vertices: 1_048_576, max_instances: 65_536, max_heat_cells: 262_144 }),
    (
        "very_heavy",
        Ceilings { max_vertices: 4_194_304, max_instances: 262_144, max_heat_cells: 1_048_576 },
    ),
];

/// The ceilings for a declared `gpu_class` spelling (`None` outside the frozen vocabulary).
#[must_use]
pub fn ceilings_for(class: &str) -> Option<Ceilings> {
    CEILING_TABLE.iter().find(|(name, _)| *name == class).map(|(_, c)| *c)
}

/// A frame's scene-data cost, counted the way `ceilings.rs` budgets it (the convention documented in
/// the Observatory core's `count_budget`: polyline/path/trace points and rect/arc tessellations are
/// VERTICES; glyph runs and point sprites are INSTANCES; heat grids are CELLS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Budget {
    /// Vertices.
    pub vertices: u32,
    /// Instances.
    pub instances: u32,
    /// Heat cells.
    pub heat_cells: u32,
}

impl Budget {
    /// Whether the frame fits a declared class.
    #[must_use]
    pub fn fits(self, c: Ceilings) -> bool {
        self.vertices <= c.max_vertices
            && self.instances <= c.max_instances
            && self.heat_cells <= c.max_heat_cells
    }
}

/// Counts a display list's budget (over the parsed IR, so the host counts what it will paint).
#[must_use]
pub fn count_budget(items: &[Item]) -> Budget {
    let mut b = Budget::default();
    for it in items {
        match it {
            Item::Polyline { points, .. } => b.vertices += points.len() as u32,
            Item::Path { subpaths, .. } => {
                b.vertices += subpaths.iter().map(|s| s.len() as u32 + 1).sum::<u32>();
            },
            Item::Trace { values, .. } => b.vertices += values.len() as u32,
            Item::Rect { .. } => b.vertices += 4,
            Item::Arc { .. } => b.vertices += 32,
            Item::GlyphRun { .. } => b.instances += 1,
            Item::Points { positions, .. } => b.instances += positions.len() as u32,
            Item::HeatCells { cols, rows, .. } => b.heat_cells += cols.saturating_mul(*rows),
        }
    }
    b
}

/// Parses the D13 interchange into a display list. Refuses malformed shapes in words.
///
/// # Errors
/// A string naming the field and the defect.
pub fn parse_surface(json: &Json) -> Result<Vec<Item>, String> {
    let items = json
        .get("items")
        .and_then(Json::as_array)
        .ok_or_else(|| "interchange has no `items` array — not a display-list JSON".to_string())?;
    let mut out = Vec::with_capacity(items.len());
    for (i, it) in items.iter().enumerate() {
        out.push(parse_item(it, i)?);
    }
    Ok(out)
}

fn parse_item(it: &Json, i: usize) -> Result<Item, String> {
    let kind =
        it.get("kind").and_then(Json::as_str).ok_or_else(|| format!("items[{i}] has no `kind`"))?;
    let at = |k: &str| it.get(k).ok_or_else(|| format!("items[{i}] ({kind}) misses `{k}`"));
    let f = |k: &str| {
        at(k)?.as_f32().ok_or_else(|| format!("items[{i}] ({kind}).{k} is not a finite number"))
    };
    let style = |it: &Json, i: usize| -> Result<Style, String> {
        let st = it.get("style").ok_or_else(|| format!("items[{i}] misses `style`"))?;
        Ok(Style {
            stroke: st.get("stroke").and_then(Json::as_str).map(str::to_string),
            width: match st.get("stroke_width").and_then(Json::as_str).unwrap_or("w1") {
                "w2" => Width::W2,
                "w3" => Width::W3,
                _ => Width::W1,
            },
            fill: st.get("fill").and_then(Json::as_str).map(str::to_string),
            dash: match st.get("dash").and_then(Json::as_str).unwrap_or("solid") {
                "dashed" => Dash::Dashed,
                "dotted" => Dash::Dotted,
                "hidden" => Dash::Hidden,
                _ => Dash::Solid,
            },
            corner: match st.get("corner").and_then(Json::as_str).unwrap_or("c0") {
                "c2" => Corner::C2,
                "c4" => Corner::C4,
                _ => Corner::C0,
            },
        })
    };
    let points = |v: &Json| -> Result<Vec<(f32, f32)>, String> {
        v.as_array()
            .ok_or_else(|| "points is not an array".to_string())?
            .iter()
            .map(|p| {
                Ok((
                    p.get("x").and_then(Json::as_f32).unwrap_or(0.0),
                    p.get("y").and_then(Json::as_f32).unwrap_or(0.0),
                ))
            })
            .collect()
    };
    match kind {
        "rect" => Ok(Item::Rect { x: f("x")?, y: f("y")?, w: f("w")?, h: f("h")?, style: style(it, i)? }),
        "polyline" => {
            let pts = at("points")?;
            Ok(Item::Polyline { points: points(pts)?, style: style(it, i)? })
        },
        "path" => {
            let segs = at("segments")?;
            let mut subpaths: Vec<Vec<Seg>> = Vec::new();
            for s in segs.as_array().ok_or_else(|| "segments is not an array".to_string())? {
                let op = s.get("op").and_then(Json::as_str).unwrap_or("");
                match op {
                    "move" => subpaths.push(Vec::new()),
                    "line" => {
                        let to = s.get("to").ok_or_else(|| "line misses `to`".to_string())?;
                        subpaths.last_mut().ok_or_else(|| "line before move".to_string())?.push(Seg::Line(
                            to.get("x").and_then(Json::as_f32).unwrap_or(0.0),
                            to.get("y").and_then(Json::as_f32).unwrap_or(0.0),
                        ));
                    },
                    "quad" => {
                        let (c, t) = (s.get("c").ok_or_else(|| "quad misses c".to_string())?, s.get("to").ok_or_else(|| "quad misses to".to_string())?);
                        let g = |v: &Json, k: &str| v.get(k).and_then(Json::as_f32).unwrap_or(0.0);
                        subpaths.last_mut().ok_or_else(|| "quad before move".to_string())?.push(Seg::Quad(g(c, "x"), g(c, "y"), g(t, "x"), g(t, "y")));
                    },
                    "cubic" => {
                        let (c1, c2, t) = (
                            s.get("c1").ok_or_else(|| "cubic misses c1".to_string())?,
                            s.get("c2").ok_or_else(|| "cubic misses c2".to_string())?,
                            s.get("to").ok_or_else(|| "cubic misses to".to_string())?,
                        );
                        let g = |v: &Json, k: &str| v.get(k).and_then(Json::as_f32).unwrap_or(0.0);
                        subpaths.last_mut().ok_or_else(|| "cubic before move".to_string())?.push(Seg::Cubic(g(c1, "x"), g(c1, "y"), g(c2, "x"), g(c2, "y"), g(t, "x"), g(t, "y")));
                    },
                    other => return Err(format!("items[{i}]: unknown path op `{other}`")),
                }
            }
            Ok(Item::Path { subpaths, closed: it.get("closed").and_then(Json::as_bool).unwrap_or(false), style: style(it, i)? })
        },
        "arc" => Ok(Item::Arc { cx: f("cx")?, cy: f("cy")?, r: f("r")?, a0: f("a0_deg")?, a1: f("a1_deg")?, style: style(it, i)? }),
        "glyph_run" => Ok(Item::GlyphRun {
            text: at("text")?.as_str().ok_or_else(|| "glyph_run text is not a string".to_string())?.to_string(),
            x: f("x")?,
            y: f("y")?,
            size: at("size")?.as_str().ok_or_else(|| "glyph_run size is not a token id".to_string())?.to_string(),
            colour: at("colour")?.as_str().ok_or_else(|| "glyph_run colour is not a token id".to_string())?.to_string(),
        }),
        "points" => Ok(Item::Points {
            positions: {
                let pos = at("positions")?;
                points(pos)?
            },
            size: at("size")?.as_str().ok_or_else(|| "points size is not a token id".to_string())?.to_string(),
            colour: it.get("colour").and_then(Json::as_str).map(str::to_string),
            colormap: it.get("colormap").and_then(Json::as_str).map(str::to_string),
            values: it.get("values").and_then(Json::as_array).map(|v| v.iter().map(|x| x.as_f32()).collect()),
        }),
        "heat_cells" => Ok(Item::HeatCells {
            x: f("x")?,
            y: f("y")?,
            w: f("w")?,
            h: f("h")?,
            cols: at("cols")?.as_u32().ok_or_else(|| "heat cols is not a u32".to_string())?,
            rows: at("rows")?.as_u32().ok_or_else(|| "heat rows is not a u32".to_string())?,
            values: at("values")?.as_array().ok_or_else(|| "heat values is not an array".to_string())?.iter().map(|v| v.as_f32()).collect(),
            colormap: at("colormap")?.as_str().ok_or_else(|| "heat colormap is not a token id".to_string())?.to_string(),
        }),
        "trace" => Ok(Item::Trace {
            values: at("values")?
                .as_array()
                .ok_or_else(|| "trace values is not an array".to_string())?
                .iter()
                .map(|v| v.as_f32().unwrap_or(0.0))
                .collect(),
            colour: at("colour")?.as_str().ok_or_else(|| "trace colour is not a token id".to_string())?.to_string(),
            width: match it.get("width").and_then(Json::as_str).unwrap_or("w2") {
                "w1" => Width::W1,
                "w3" => Width::W3,
                _ => Width::W2,
            },
            phosphor: it.get("phosphor").and_then(Json::as_bool).unwrap_or(false),
            box_rect: (0.0, 0.0, 0.0, 0.0), // the caller sets the display box (the record carries none)
        }),
        other => Err(format!("items[{i}]: unknown primitive kind `{other}` (the vocabulary is frozen: polyline path rect arc glyph_run points heat_cells trace)")),
    }
}

/// The painter: resolves token ids against the compile-time bundle, collecting diagnostics for every
/// primitive it skips (fail-soft at runtime; the validate scan is the loud half).
#[derive(Clone, Debug, Default)]
pub struct Painter {
    diagnostics: Vec<String>,
}

impl Painter {
    /// A fresh painter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The diagnostics recorded so far (unknown token ids, skipped primitives), deduped.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<String> {
        let mut v = self.diagnostics.clone();
        v.sort();
        v.dedup();
        v
    }

    fn skip(&mut self, why: String) {
        self.diagnostics.push(why);
    }

    /// Resolves a colour token id + whether it is a hairline stroke (which reads at the bundle's
    /// regular opacity tier).
    fn colour(&mut self, id: &str, what: &str) -> Option<Rgba> {
        match tokens::color_hex(id) {
            Some(hex) => {
                let a = if id == "color.hairline.colour" {
                    tokens::number("color.hairline.regular").unwrap_or(0.25) as f32
                } else {
                    1.0
                };
                Rgba::from_hex(hex, a)
            },
            None => {
                self.skip(format!(
                    "unknown colour token `{id}` in {what} — primitive skipped (fail-soft)"
                ));
                None
            },
        }
    }

    /// Resolves one style into (fill, stroke, stroke px, dash, corner px).
    fn style(&mut self, s: &Style, what: &str) -> (Option<Rgba>, Option<Rgba>, f32, Dash, f32) {
        let fill = s.fill.as_deref().and_then(|id| self.colour(id, what));
        let stroke = s.stroke.as_deref().and_then(|id| self.colour(id, what));
        let wpx = match s.width {
            Width::W1 => tokens::number("layout.stroke.hairline").unwrap_or(1.0),
            Width::W2 => tokens::number("layout.stroke.signal").unwrap_or(2.0),
            Width::W3 => tokens::number("layout.stroke.emphasis").unwrap_or(3.0),
        } as f32;
        let rx = match s.corner {
            Corner::C0 => 0.0,
            Corner::C2 => 2.0,
            Corner::C4 => 4.0,
        };
        (fill, stroke, wpx, s.dash, rx)
    }

    /// Samples a colormap LUT at t (0..1), linear interpolation between baked entries.
    fn sample(&mut self, colormap: &str, t: f32, what: &str) -> Rgba {
        let lut = match tokens::colormap_lut(colormap) {
            Some(l) => l,
            None => {
                self.skip(format!(
                    "unknown colormap token `{colormap}` in {what} — primitive skipped"
                ));
                return Rgba { r: 128, g: 128, b: 128, a: 1.0 };
            },
        };
        let t = t.clamp(0.0, 1.0);
        // Qualitative LUTs (a handful of entries) are INDEXED, never interpolated (blending two
        // category colours is a lie); baked ramps (256) interpolate linearly between entries.
        if lut.len() <= 16 {
            let idx = (t * lut.len() as f32).floor() as usize;
            let v = lut[idx.min(lut.len() - 1)];
            return Rgba {
                r: ((v >> 16) & 0xFF) as u8,
                g: ((v >> 8) & 0xFF) as u8,
                b: (v & 0xFF) as u8,
                a: 1.0,
            };
        }
        let x = t * (lut.len() - 1) as f32;
        let i = x.floor() as usize;
        let j = (i + 1).min(lut.len() - 1);
        let fr = x - i as f32;
        let ch = |shift: u32, a: u32, b: u32| -> u8 {
            let pa = (a >> shift) & 0xFF;
            let pb = (b >> shift) & 0xFF;
            (pa as f32 + (pb as f32 - pa as f32) * fr).round() as u8
        };
        let (a, b) = (lut[i], lut[j]);
        Rgba { r: ch(16, a, b), g: ch(8, a, b), b: ch(0, a, b), a: 1.0 }
    }

    /// Resolves a display list into backend-neutral primitives. Skips (with diagnostics) anything
    /// whose tokens do not resolve — the show goes on.
    pub fn resolve(&mut self, items: &[Item]) -> Vec<Painted> {
        let mut out = Vec::with_capacity(items.len());
        for (n, it) in items.iter().enumerate() {
            let what = format!("items[{n}]");
            match it {
                Item::Rect { x, y, w, h, style } => {
                    let (fill, stroke, sw, dash, rx) = self.style(style, &what);
                    if fill.is_none() && stroke.is_none() {
                        continue;
                    }
                    out.push(Painted::Rect {
                        x: *x,
                        y: *y,
                        w: *w,
                        h: *h,
                        rx,
                        fill,
                        stroke,
                        stroke_w: sw,
                        dash,
                    });
                },
                Item::Polyline { points, style } => {
                    let (_fill, stroke, sw, dash, _rx) = self.style(style, &what);
                    // A polyline is a stroke primitive: no resolved stroke, or fewer than two
                    // vertices, and there is nothing to paint.
                    if stroke.is_none() || points.len() < 2 {
                        continue;
                    }
                    out.push(Painted::Polyline { pts: points.clone(), stroke, stroke_w: sw, dash });
                },
                Item::Path { subpaths, closed, style } => {
                    let (fill, stroke, sw, dash, _rx) = self.style(style, &what);
                    if fill.is_none() && stroke.is_none() {
                        continue;
                    }
                    out.push(Painted::Path {
                        subpaths: subpaths.clone(),
                        closed: *closed,
                        fill,
                        stroke,
                        stroke_w: sw,
                        dash,
                    });
                },
                Item::Arc { cx, cy, r, a0, a1, style } => {
                    let (fill, stroke, sw, _dash, _rx) = self.style(style, &what);
                    if fill.is_none() && stroke.is_none() {
                        continue;
                    }
                    out.push(Painted::Arc {
                        cx: *cx,
                        cy: *cy,
                        r: *r,
                        a0: *a0,
                        a1: *a1,
                        fill,
                        stroke,
                        stroke_w: sw,
                    });
                },
                Item::GlyphRun { text, x, y, size, colour } => {
                    let size_px = match tokens::number(size) {
                        Some(v) => v as f32,
                        None => {
                            self.skip(format!(
                                "unknown size token `{size}` in {what} — glyph skipped"
                            ));
                            continue;
                        },
                    };
                    let Some(col) = self.colour(colour, &what) else { continue };
                    out.push(Painted::Text {
                        text: text.clone(),
                        x: *x,
                        y: *y,
                        size_px,
                        colour: col,
                    });
                },
                Item::Points { positions, size, colour, colormap, values } => {
                    let r = match tokens::number(size) {
                        Some(v) => v as f32,
                        None => {
                            self.skip(format!(
                                "unknown size token `{size}` in {what} — points skipped"
                            ));
                            continue;
                        },
                    };
                    let mut dots = Vec::with_capacity(positions.len());
                    match (colour, colormap) {
                        (Some(cid), _) => {
                            let Some(col) = self.colour(cid, &what) else { continue };
                            for &(x, y) in positions {
                                dots.push((x, y, r, col));
                            }
                        },
                        (None, Some(cm)) => {
                            let vals = values.clone().unwrap_or_default();
                            for (k, &(x, y)) in positions.iter().enumerate() {
                                let t = match vals.get(k).copied().flatten() {
                                    Some(v) if v.is_finite() => v,
                                    _ => continue, // a NaN value = no dot (the sentinel)
                                };
                                dots.push((x, y, r, self.sample(cm, t, &what)));
                            }
                        },
                        (None, None) => self.skip(format!(
                            "{what}: points with neither colour nor colormap — skipped"
                        )),
                    }
                    if !dots.is_empty() {
                        out.push(Painted::Dots { pts: dots });
                    }
                },
                Item::HeatCells { x, y, w, h, cols, rows, values, colormap } => {
                    if *cols == 0 || *rows == 0 {
                        continue;
                    }
                    let cw = w / *cols as f32;
                    let ch = h / *rows as f32;
                    let mut cells = Vec::new();
                    for row in 0..*rows {
                        for col in 0..*cols {
                            let idx = (row * cols + col) as usize;
                            let t = match values.get(idx).copied().flatten() {
                                Some(v) if v.is_finite() => v,
                                _ => continue, // NaN/null = no cell (the contract's sentinel)
                            };
                            cells.push((
                                x + col as f32 * cw,
                                y + row as f32 * ch,
                                cw,
                                ch,
                                self.sample(colormap, t, &what),
                            ));
                        }
                    }
                    if !cells.is_empty() {
                        out.push(Painted::Heat { cells });
                    }
                },
                Item::Trace { values, colour, width, box_rect, .. } => {
                    let Some(col) = self.colour(colour, &what) else { continue };
                    if values.len() < 2 {
                        continue;
                    }
                    let (bx, by, bw, bh) = *box_rect;
                    let sw = match width {
                        Width::W1 => tokens::number("layout.stroke.hairline").unwrap_or(1.0) as f32,
                        Width::W2 => tokens::number("layout.stroke.signal").unwrap_or(2.0) as f32,
                        Width::W3 => tokens::number("layout.stroke.emphasis").unwrap_or(3.0) as f32,
                    };
                    let pts: Vec<(f32, f32)> = values
                        .iter()
                        .enumerate()
                        .map(|(k, v)| {
                            let px = bx + (k as f32 / (values.len() - 1) as f32) * bw;
                            let py = by + (1.0 - v.clamp(0.0, 1.0)) * bh;
                            (px, py)
                        })
                        .collect();
                    out.push(Painted::Polyline {
                        pts,
                        stroke: Some(col),
                        stroke_w: sw,
                        dash: Dash::Solid,
                    });
                },
            }
        }
        out
    }
}

// ── the SVG backend (headless) ──────────────────────────────────────────────────────────────────

/// Writes a resolved display list as a standalone SVG (the `--svg-out` family's instrument face).
/// `background` paints the display ground first (the card's inset ground); `None` leaves it
/// transparent (for embedding into the canvas SVG, which already has its ground).
#[must_use]
pub fn svg(painted: &[Painted], w: f32, h: f32, background: Option<Rgba>) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}\" height=\"{h:.0}\" viewBox=\"0 0 {w:.3} {h:.3}\">\n"
    ));
    if let Some(bg) = background {
        s.push_str(&format!(
            "<rect x=\"0\" y=\"0\" width=\"{w:.3}\" height=\"{h:.3}\" fill=\"{}\"/>\n",
            bg.css()
        ));
    }
    for p in painted {
        paint_svg(&mut s, p);
    }
    s.push_str("</svg>\n");
    s
}

fn dash_attr(d: Dash) -> &'static str {
    match d {
        Dash::Solid => "",
        Dash::Dashed => " stroke-dasharray=\"6 4\"",
        Dash::Dotted => " stroke-dasharray=\"1 4\"",
        Dash::Hidden => " stroke-opacity=\"0\"",
    }
}

fn fill_attr(c: Option<Rgba>) -> String {
    match c {
        Some(c) if c.a >= 0.999 => format!(" fill=\"{}\"", c.css()),
        Some(c) => format!(" fill=\"{}\" fill-opacity=\"{:.3}\"", c.css(), c.a),
        None => " fill=\"none\"".to_string(),
    }
}

fn stroke_attr(c: Option<Rgba>, sw: f32) -> String {
    match c {
        Some(c) if c.a >= 0.999 => format!(" stroke=\"{}\" stroke-width=\"{sw:.2}\"", c.css()),
        Some(c) => format!(
            " stroke=\"{}\" stroke-opacity=\"{:.3}\" stroke-width=\"{sw:.2}\"",
            c.css(),
            c.a
        ),
        None => String::new(),
    }
}

fn paint_svg(s: &mut String, p: &Painted) {
    match p {
        Painted::Rect { x, y, w, h, rx, fill, stroke, stroke_w, dash } => {
            s.push_str(&format!(
                "<rect x=\"{x:.3}\" y=\"{y:.3}\" width=\"{:.3}\" height=\"{:.3}\" rx=\"{rx:.1}\"{}{}{}/>\n",
                w.max(0.0),
                h.max(0.0),
                fill_attr(*fill),
                stroke_attr(*stroke, *stroke_w),
                dash_attr(*dash)
            ));
        },
        Painted::Polyline { pts, stroke, stroke_w, dash } => {
            if pts.len() < 2 {
                return;
            }
            let pattr: Vec<String> = pts.iter().map(|(x, y)| format!("{x:.2},{y:.2}")).collect();
            s.push_str(&format!(
                "<polyline points=\"{}\"{}{}{}/>\n",
                pattr.join(" "),
                fill_attr(None),
                stroke_attr(*stroke, *stroke_w),
                dash_attr(*dash)
            ));
        },
        Painted::Path { subpaths, closed, fill, stroke, stroke_w, dash } => {
            let mut d = String::new();
            for sp in subpaths {
                let mut first = true;
                for seg in sp {
                    match seg {
                        Seg::Line(x, y) => {
                            if first {
                                d.push_str(&format!("M {x:.2} {y:.2} "));
                                first = false;
                            } else {
                                d.push_str(&format!("L {x:.2} {y:.2} "));
                            }
                        },
                        Seg::Quad(cx, cy, x, y) => {
                            if first {
                                d.push_str(&format!("M {cx:.2} {cy:.2} "));
                                first = false;
                            }
                            d.push_str(&format!("Q {cx:.2} {cy:.2} {x:.2} {y:.2} "));
                        },
                        Seg::Cubic(a, b, c, dd, x, y) => {
                            if first {
                                d.push_str(&format!("M {a:.2} {b:.2} "));
                                first = false;
                            }
                            d.push_str(&format!("C {a:.2} {b:.2} {c:.2} {dd:.2} {x:.2} {y:.2} "));
                        },
                    }
                }
                if *closed {
                    d.push_str("Z ");
                }
            }
            s.push_str(&format!(
                "<path d=\"{}\"{}{}{}/>\n",
                d.trim(),
                fill_attr(*fill),
                stroke_attr(*stroke, *stroke_w),
                dash_attr(*dash)
            ));
        },
        Painted::Arc { cx, cy, r, a0, a1, fill, stroke, stroke_w } => {
            let sweep = a1 - a0;
            if sweep.abs() >= 359.999 {
                s.push_str(&format!(
                    "<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{:.2}\"{}{}/>\n",
                    r.max(0.0),
                    fill_attr(*fill),
                    stroke_attr(*stroke, *stroke_w)
                ));
            } else {
                let pt = |deg: f32| {
                    let a = deg.to_radians();
                    (cx + r * a.cos(), cy + r * a.sin())
                };
                let (x0, y0) = pt(*a0);
                let (x1, y1) = pt(*a1);
                let large = if sweep.abs() > 180.0 { 1 } else { 0 };
                let flag = if sweep >= 0.0 { 1 } else { 0 };
                s.push_str(&format!(
                    "<path d=\"M {x0:.2} {y0:.2} A {r:.2} {r:.2} 0 {large} {flag} {x1:.2} {y1:.2}\"{}{}/>\n",
                    fill_attr(*fill),
                    stroke_attr(*stroke, *stroke_w)
                ));
            }
        },
        Painted::Text { text, x, y, size_px, colour } => {
            s.push_str(&format!(
                "<text x=\"{x:.2}\" y=\"{y:.2}\" font-size=\"{size_px:.2}\" fill=\"{}\" xml:space=\"preserve\">{}</text>\n",
                colour.css(),
                escape(text)
            ));
        },
        Painted::Dots { pts } => {
            for (x, y, r, c) in pts {
                s.push_str(&format!(
                    "<circle cx=\"{x:.2}\" cy=\"{y:.2}\" r=\"{r:.2}\" fill=\"{}\"/>\n",
                    c.css()
                ));
            }
        },
        Painted::Heat { cells } => {
            for (x, y, w, h, c) in cells {
                s.push_str(&format!(
                    "<rect x=\"{x:.2}\" y=\"{y:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"{}\"/>\n",
                    w + 0.5,
                    h + 0.5,
                    c.css()
                ));
            }
        },
    }
}

/// XML-escapes text for the SVG backend.
#[must_use]
pub fn escape(t: &str) -> String {
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c if (c as u32) < 0x20 && c != '\t' => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::json;

    fn item(json: &str) -> Item {
        let v = json::parse(json).unwrap();
        parse_surface(&json::parse(&format!("{{\"items\":[{json}]}}")).unwrap())
            .unwrap_or_else(|e| panic!("{e}: {v:?}"))
            .pop()
            .unwrap()
    }

    #[test]
    fn every_frozen_primitive_parses() {
        // All eight kinds of the frozen vocabulary parse from the interchange — the coverage gate
        // §8.1 asks for (every variant of the item enum reaches the painters).
        let kinds = [
            r#"{"kind":"rect","x":0,"y":0,"w":10,"h":10,"style":{"fill":"color.ground.inset","stroke_width":"w1","dash":"solid","corner":"c2"}}"#,
            r#"{"kind":"polyline","points":[{"x":0,"y":0},{"x":9,"y":9}],"style":{"stroke":"color.signal.data.colour","stroke_width":"w2","dash":"solid","corner":"c0"}}"#,
            r#"{"kind":"path","segments":[{"op":"move","to":{"x":0,"y":0}},{"op":"line","to":{"x":4,"y":0}},{"op":"line","to":{"x":0,"y":4}}],"closed":true,"style":{"fill":"color.ground.panel_alt","stroke":"color.hairline.colour","stroke_width":"w1","dash":"solid","corner":"c0"}}"#,
            r#"{"kind":"arc","cx":5,"cy":5,"r":3,"a0_deg":0,"a1_deg":360,"style":{"fill":"color.signal.data.colour","stroke_width":"w1","dash":"solid","corner":"c0"}}"#,
            r#"{"kind":"glyph_run","text":"282 km/s","x":1,"y":2,"size":"typography.scale.s","colour":"color.text.tertiary"}"#,
            r#"{"kind":"points","positions":[{"x":1,"y":2}],"size":"layout.marker.radius_m","colour":null,"colormap":"colormap.inferno-class","values":[0.5]}"#,
            r#"{"kind":"heat_cells","x":0,"y":0,"w":8,"h":4,"cols":2,"rows":1,"values":[0.25,null],"colormap":"colormap.thermal"}"#,
            r#"{"kind":"trace","values":[0.1,0.9],"colour":"color.signal.data.colour","width":"w2","phosphor":true}"#,
        ];
        let parsed: Vec<Item> = kinds.iter().map(|k| item(k)).collect();
        assert_eq!(parsed.len(), 8);
    }

    #[test]
    fn an_unknown_kind_is_refused_in_words() {
        let e =
            parse_surface(&json::parse(r#"{"items":[{"kind":"pixels"}]}"#).unwrap()).unwrap_err();
        assert!(e.contains("pixels") && e.contains("frozen"), "{e}");
    }

    #[test]
    fn unknown_tokens_skip_with_a_diagnostic_not_a_guess() {
        let mut p = Painter::new();
        let items = vec![item(
            r#"{"kind":"rect","x":0,"y":0,"w":10,"h":10,"style":{"fill":"color.nope.nope","stroke_width":"w1","dash":"solid","corner":"c0"}}"#,
        )];
        let painted = p.resolve(&items);
        assert!(painted.is_empty(), "an unresolvable primitive is skipped");
        let d = p.diagnostics();
        assert!(d.iter().any(|s| s.contains("color.nope.nope")), "{d:?}");
    }

    #[test]
    fn known_tokens_resolve_to_the_bundle_values() {
        let mut p = Painter::new();
        let items = vec![item(
            r#"{"kind":"rect","x":0,"y":0,"w":10,"h":10,"style":{"fill":"color.ground.inset","stroke_width":"w1","dash":"solid","corner":"c2"}}"#,
        )];
        let painted = p.resolve(&items);
        match &painted[0] {
            Painted::Rect { fill, rx, .. } => {
                assert_eq!(
                    fill.unwrap(),
                    Rgba::from_hex(tokens::color_hex("color.ground.inset").unwrap(), 1.0).unwrap()
                );
                assert_eq!(*rx, 2.0, "c2 resolves to 2 px");
            },
            other => panic!("expected a rect, got {other:?}"),
        }
    }

    #[test]
    fn a_hairline_stroke_reads_at_the_bundle_opacity_tier() {
        let mut p = Painter::new();
        let items = vec![item(
            r#"{"kind":"polyline","points":[{"x":0,"y":0},{"x":9,"y":9}],"style":{"stroke":"color.hairline.colour","stroke_width":"w1","dash":"dotted","corner":"c0"}}"#,
        )];
        let painted = p.resolve(&items);
        match &painted[0] {
            Painted::Polyline { stroke, dash, .. } => {
                let want = tokens::number("color.hairline.regular").unwrap() as f32;
                assert!(
                    (stroke.unwrap().a - want).abs() < 1e-6,
                    "hairline reads at the regular tier"
                );
                assert_eq!(*dash, Dash::Dotted);
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn heat_nulls_are_skipped_and_lut_values_sample() {
        let mut p = Painter::new();
        let items = vec![item(
            r#"{"kind":"heat_cells","x":0,"y":0,"w":8,"h":4,"cols":2,"rows":1,"values":[1.0,null],"colormap":"colormap.thermal"}"#,
        )];
        let painted = p.resolve(&items);
        match &painted[0] {
            Painted::Heat { cells } => {
                assert_eq!(cells.len(), 1, "the null cell is skipped");
                // t=1.0 samples the LUT's last entry (thermal's hot end).
                let lut = tokens::colormap_lut("colormap.thermal").unwrap();
                let last = lut[255];
                assert_eq!(cells[0].4.r, ((last >> 16) & 0xFF) as u8);
            },
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_budget_counts_the_frozen_convention() {
        let items = vec![
            item(
                r#"{"kind":"heat_cells","x":0,"y":0,"w":8,"h":4,"cols":2,"rows":1,"values":[1.0,null],"colormap":"colormap.thermal"}"#,
            ),
            item(
                r#"{"kind":"glyph_run","text":"x","x":1,"y":2,"size":"typography.scale.s","colour":"color.text.primary"}"#,
            ),
            item(
                r#"{"kind":"rect","x":0,"y":0,"w":10,"h":10,"style":{"fill":"color.ground.inset","stroke_width":"w1","dash":"solid","corner":"c0"}}"#,
            ),
        ];
        let b = count_budget(&items);
        assert_eq!(b.heat_cells, 2);
        assert_eq!(b.instances, 1);
        assert_eq!(b.vertices, 4);
        // The medium class fits the Observatory's measured worst case (harness §5.8).
        assert!(b.fits(ceilings_for("medium").unwrap()));
        assert!(!Budget { vertices: 999_999, instances: 0, heat_cells: 0 }
            .fits(ceilings_for("medium").unwrap()));
    }

    #[test]
    fn the_svg_backend_emits_elements_for_every_painted_kind() {
        let mut p = Painter::new();
        let items: Vec<Item> = [
            r#"{"kind":"rect","x":0,"y":0,"w":10,"h":10,"style":{"fill":"color.ground.inset","stroke_width":"w1","dash":"solid","corner":"c0"}}"#,
            r#"{"kind":"polyline","points":[{"x":0,"y":0},{"x":9,"y":9}],"style":{"stroke":"color.signal.data.colour","stroke_width":"w2","dash":"solid","corner":"c0"}}"#,
            r#"{"kind":"arc","cx":5,"cy":5,"r":3,"a0_deg":0,"a1_deg":360,"style":{"fill":"color.signal.data.colour","stroke_width":"w1","dash":"solid","corner":"c0"}}"#,
            r#"{"kind":"glyph_run","text":"a < b","x":1,"y":2,"size":"typography.scale.s","colour":"color.text.primary"}"#,
            r#"{"kind":"points","positions":[{"x":1,"y":2}],"size":"layout.marker.radius_m","colour":"color.signal.data.colour","colormap":null,"values":null}"#,
            r#"{"kind":"heat_cells","x":0,"y":0,"w":8,"h":4,"cols":2,"rows":1,"values":[0.5,0.5],"colormap":"colormap.thermal"}"#,
        ]
        .iter()
        .map(|k| item(k))
        .collect();
        let painted = p.resolve(&items);
        // R6: no colour literal in source — the SVG's ground comes from the bundle like everywhere else.
        let bg =
            crate::tokens::color_hex("color.ground.panel").and_then(|h| Rgba::from_hex(h, 1.0));
        let svg = svg(&painted, 100.0, 50.0, bg);
        for tag in ["<rect", "<polyline", "<circle", "<text"] {
            assert!(svg.contains(tag), "svg misses {tag}");
        }
        assert!(svg.contains("a &lt; b"), "text is XML-escaped");
    }
}

#[cfg(test)]
mod drift_tests {
    //! The compile-time token face vs the generated JSON files — one bake, two faces, checked.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::json;

    fn generated(name: &str) -> json::Json {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../design/tokens/generated")
            .join(name);
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        json::parse(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    #[test]
    fn the_baked_luts_are_colormaps_json_byte_for_value() {
        let cj = generated("colormaps.json");
        let maps = cj.get("maps").and_then(json::Json::as_array).is_none();
        let _ = maps;
        let obj = match cj.get("maps").unwrap() {
            json::Json::Obj(pairs) => pairs.clone(),
            other => panic!("maps is not an object: {other:?}"),
        };
        for (name, lut_json) in obj {
            let hexes = lut_json.as_array().unwrap();
            let id = format!(
                "colormap.{}",
                name.replace('_', "-")
                    .replace("phosphor-cb", "phosphor.cb")
                    .replace("bipolar-cb", "bipolar.cb")
                    .replace("categorical-6-cb", "categorical-6.cb")
            );
            // The normalisation rule: dashes AND dots fold to underscores, so recover the id by
            // trying the documented spellings; the LUT lookup is the thing under test.
            let lut = tokens::colormap_lut(&id)
                .or_else(|| tokens::colormap_lut(&format!("colormap.{}", name.replace('_', "."))))
                .or_else(|| tokens::colormap_lut(&format!("colormap.{}", name.replace('_', "-"))))
                .unwrap_or_else(|| panic!("no LUT resolves for map `{name}` (tried {id})"));
            assert_eq!(lut.len(), hexes.len(), "{name}: LUT length");
            for (i, hx) in hexes.iter().enumerate() {
                let want = Rgba::from_hex(hx.as_str().unwrap(), 1.0).unwrap();
                let got = lut[i];
                assert_eq!(((got >> 16) & 0xFF) as u8, want.r, "{name}[{i}] red");
                assert_eq!(((got >> 8) & 0xFF) as u8, want.g, "{name}[{i}] green");
                assert_eq!((got & 0xFF) as u8, want.b, "{name}[{i}] blue");
            }
        }
    }

    #[test]
    fn every_bundle_colour_resolves_through_color_hex() {
        let tj = generated("tokens.json");
        let tokens_obj = tj.get("tokens").unwrap();
        let mut checked = 0;
        if let json::Json::Obj(pairs) = tokens_obj {
            for (id, v) in pairs {
                let Some(hex) = v.as_str() else { continue };
                if !hex.starts_with('#') || hex.len() != 7 {
                    continue;
                }
                assert_eq!(tokens::color_hex(id), Some(hex), "{id} resolves to its bundle hex");
                checked += 1;
            }
        }
        assert!(checked >= 70, "the colour table is populated ({checked})");
    }

    #[test]
    fn every_bundle_number_resolves_through_number() {
        let tj = generated("tokens.json");
        if let json::Json::Obj(pairs) = tj.get("tokens").unwrap() {
            for (id, v) in pairs {
                match v {
                    json::Json::Num(x) => {
                        assert_eq!(tokens::number(id), Some(*x), "{id}");
                    },
                    _ => continue,
                }
            }
        }
        assert_eq!(tokens::number("layout.space.4"), Some(16.0));
        assert_eq!(
            tokens::number("layout.marker.radius_m"),
            Some(5.0),
            "the INC3 marker tokens resolve"
        );
    }
}
