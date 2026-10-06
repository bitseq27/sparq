//! The token resolver for the harness's SVG painter — reads the CHECKED-IN bundle
//! (`design/tokens/generated/tokens.json` + `colormaps.json`) and turns the core's semantic token
//! ids into concrete paint values.
//!
//! This is the sandbox stand-in for INC4's `sparq-ui/src/displaylist.rs` painter: the SAME IR, the
//! SAME token bundle, resolved the SAME way (id = bundle key; unknown id → skip with a diagnostic,
//! display.wit's fail-soft rule). Every value comes from the files, never from this module's memory
//! (the make_display_sheet / make_observatory_mockups discipline) — so a token change re-themes the
//! harness SVG with zero code change, which is WO-019's re-theme property demonstrated in the
//! sandbox before the device ever runs it.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The repo root, from this crate's manifest dir (`instruments-src/observatory/harness`).
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// The generated token bundle directory.
#[must_use]
pub fn token_dir() -> PathBuf {
    repo_root().join("design/tokens/generated")
}

/// An RGB triple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    /// Red 0..255.
    pub r: u8,
    /// Green 0..255.
    pub g: u8,
    /// Blue 0..255.
    pub b: u8,
}

impl Rgb {
    /// The `#RRGGBB` form.
    #[must_use]
    pub fn hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// Parses a `#RGB`/`#RRGGBB` hex string. Returns `None` on a malformed value (the painter then skips
/// the primitive — fail-soft, never a guessed colour).
#[must_use]
pub fn parse_hex(s: &str) -> Option<Rgb> {
    let s = s.trim().strip_prefix('#').unwrap_or(s);
    let (r, g, b) = match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1], 16).ok()?;
            let g = u8::from_str_radix(&s[1..2], 16).ok()?;
            let b = u8::from_str_radix(&s[2..3], 16).ok()?;
            (r * 17, g * 17, b * 17)
        },
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            (r, g, b)
        },
        _ => return None,
    };
    Some(Rgb { r, g, b })
}

/// The loaded token bundle + colormap LUTs.
#[derive(Clone, Debug)]
pub struct Tokens {
    /// token id → JSON value (from tokens.json's `tokens` object).
    tokens: HashMap<String, serde_json::Value>,
    /// colormap name (underscored, e.g. `inferno_class`) → baked LUT of hex strings.
    maps: HashMap<String, Vec<String>>,
    /// Diagnostic sink: token ids that failed to resolve (the fail-soft skip list).
    unresolved: std::cell::RefCell<Vec<String>>,
}

impl Tokens {
    /// Loads the bundle from the checked-in generated files.
    ///
    /// # Errors
    /// A string naming the file/parse problem if the bundle is absent or malformed.
    pub fn load() -> Result<Self, String> {
        Self::load_from(&token_dir())
    }

    /// Loads the bundle from an explicit directory (tests point this at the repo's generated dir).
    ///
    /// # Errors
    /// A string naming the file/parse problem.
    pub fn load_from(dir: &Path) -> Result<Self, String> {
        let tj = std::fs::read_to_string(dir.join("tokens.json"))
            .map_err(|e| format!("cannot read tokens.json in {}: {e}", dir.display()))?;
        let cj = std::fs::read_to_string(dir.join("colormaps.json"))
            .map_err(|e| format!("cannot read colormaps.json in {}: {e}", dir.display()))?;
        let tv: serde_json::Value =
            serde_json::from_str(&tj).map_err(|e| format!("tokens.json is malformed: {e}"))?;
        let cv: serde_json::Value =
            serde_json::from_str(&cj).map_err(|e| format!("colormaps.json is malformed: {e}"))?;
        let mut tokens = HashMap::new();
        if let Some(obj) = tv.get("tokens").and_then(serde_json::Value::as_object) {
            for (k, v) in obj {
                tokens.insert(k.clone(), v.clone());
            }
        }
        let mut maps = HashMap::new();
        if let Some(obj) = cv.get("maps").and_then(serde_json::Value::as_object) {
            for (k, v) in obj {
                if let Some(arr) = v.as_array() {
                    maps.insert(
                        k.clone(),
                        arr.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
                    );
                }
            }
        }
        if tokens.is_empty() {
            return Err("tokens.json carried no `tokens` object — wrong bundle?".to_string());
        }
        Ok(Self { tokens, maps, unresolved: std::cell::RefCell::new(Vec::new()) })
    }

    /// The raw JSON value for a token id, if present.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&serde_json::Value> {
        self.tokens.get(id)
    }

    /// A numeric token (typography.scale.*, layout.marker.*, layout.space.*), or `None`.
    #[must_use]
    pub fn size(&self, id: &str) -> Option<f64> {
        self.tokens.get(id).and_then(|v| v.as_f64().or_else(|| v.as_i64().map(|n| n as f64)))
    }

    /// A colour token → RGB. Records an unresolved diagnostic on a miss (absent key OR a malformed
    /// hex) so the painter's skip is visible, never a silent hole (fail-soft, display.wit).
    pub fn color(&self, id: &str) -> Option<Rgb> {
        let rgb = self.tokens.get(id).and_then(serde_json::Value::as_str).and_then(parse_hex);
        match rgb {
            Some(c) => Some(c),
            None => {
                self.unresolved.borrow_mut().push(id.to_string());
                None
            },
        }
    }

    /// The opacity to paint a stroke/fill token at. The house convention (the mockups' idiom, and
    /// what INC4's painter will do): a hairline stroke reads at `color.hairline.regular`; everything
    /// else is opaque. This is the ONE place an opacity is derived, and it is derived from the
    /// bundle, not hard-coded.
    #[must_use]
    pub fn opacity_for(&self, id: &str) -> f32 {
        if id == "color.hairline.colour" {
            self.size("color.hairline.regular").unwrap_or(0.25) as f32
        } else {
            1.0
        }
    }

    /// Normalises a colormap token id (`colormap.inferno-class`) to its baked-LUT key
    /// (`inferno_class`): strip the `colormap.` prefix, dashes → underscores.
    #[must_use]
    pub fn map_key(id: &str) -> String {
        id.strip_prefix("colormap.").unwrap_or(id).replace('-', "_")
    }

    /// The baked LUT for a colormap token id, or `None`.
    #[must_use]
    pub fn colormap(&self, id: &str) -> Option<&[String]> {
        self.maps.get(&Self::map_key(id)).map(Vec::as_slice)
    }

    /// Samples a colormap at `t` (0..1), linearly interpolating the baked LUT. Returns a mid-grey on
    /// an unknown map (the painter's fail-soft), and records the miss.
    pub fn sample(&self, id: &str, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let Some(lut) = self.colormap(id) else {
            self.unresolved.borrow_mut().push(id.to_string());
            return Rgb { r: 128, g: 128, b: 128 };
        };
        if lut.is_empty() {
            return Rgb { r: 128, g: 128, b: 128 };
        }
        let x = t * (lut.len() - 1) as f32;
        let i = x.floor() as usize;
        let j = (i + 1).min(lut.len() - 1);
        let f = x - i as f32;
        let a = parse_hex(&lut[i]).unwrap_or(Rgb { r: 0, g: 0, b: 0 });
        let b = parse_hex(&lut[j]).unwrap_or(a);
        let lerp = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * f).round() as u8;
        Rgb { r: lerp(a.r, b.r), g: lerp(a.g, b.g), b: lerp(a.b, b.b) }
    }

    /// The token ids that failed to resolve during painting (the fail-soft skip list). The harness
    /// prints these so an unresolved id is a visible diagnostic, never a silent hole.
    #[must_use]
    pub fn unresolved(&self) -> Vec<String> {
        let mut v = self.unresolved.borrow().clone();
        v.sort();
        v.dedup();
        v
    }

    /// Every token id in the bundle (for the drift test: the core's emitted ids must all be here).
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.tokens.keys().cloned().collect();
        v.sort();
        v
    }

    /// Every colormap LUT key in the bundle.
    #[must_use]
    pub fn map_keys(&self) -> Vec<String> {
        let mut v: Vec<String> = self.maps.keys().cloned().collect();
        v.sort();
        v
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn the_checked_in_bundle_loads() {
        let t = Tokens::load().unwrap();
        assert!(t.size("layout.space.4") == Some(16.0), "space.4 = 16");
        assert!(t.color("color.signal.data.colour").is_some());
        assert!(t.colormap("colormap.thermal").is_some());
    }

    #[test]
    fn hex_parsing_handles_both_widths() {
        assert_eq!(parse_hex("#FFF"), Some(Rgb { r: 255, g: 255, b: 255 }));
        assert_eq!(parse_hex("#8BE36A"), Some(Rgb { r: 0x8B, g: 0xE3, b: 0x6A }));
        assert_eq!(parse_hex("nope"), None);
    }

    #[test]
    fn colormap_keys_normalise_dashes_to_underscores() {
        assert_eq!(Tokens::map_key("colormap.inferno-class"), "inferno_class");
        assert_eq!(Tokens::map_key("colormap.categorical-6"), "categorical_6");
        assert_eq!(Tokens::map_key("colormap.thermal"), "thermal");
    }

    #[test]
    fn sampling_a_colormap_is_clamped_and_interpolated() {
        let t = Tokens::load().unwrap();
        let lo = t.sample("colormap.thermal", 0.0);
        let hi = t.sample("colormap.thermal", 1.0);
        assert_ne!(lo, hi, "the thermal ramp's ends differ");
        // Out-of-range clamps rather than panicking.
        assert_eq!(t.sample("colormap.thermal", -5.0), lo);
        assert_eq!(t.sample("colormap.thermal", 99.0), hi);
    }

    #[test]
    fn an_unknown_colour_records_a_diagnostic() {
        let t = Tokens::load().unwrap();
        assert!(t.color("color.nope.nope").is_none());
        // (colour misses are recorded inside color(); a colormap miss is recorded in sample().)
        let _ = t.sample("colormap.nope", 0.5);
        assert!(t.unresolved().contains(&"colormap.nope".to_string()));
    }

    #[test]
    fn hairline_reads_at_the_bundle_opacity() {
        let t = Tokens::load().unwrap();
        let reg = t.size("color.hairline.regular").unwrap() as f32;
        assert!((t.opacity_for("color.hairline.colour") - reg).abs() < 1e-6);
        assert_eq!(t.opacity_for("color.text.primary"), 1.0);
    }
}
