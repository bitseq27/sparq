//! The at-rest display-list store (WO-020 INC4 §8.2, D13's "at-rest wall").
//!
//! A display instance is not always running: the loader is absent in the sandbox shell, an instance
//! may be bypassed, and a fresh canvas has never drawn. The shell's rule — "live where the rings
//! carry it, AT REST otherwise" — extends to instruments: the card shows the LAST rendered display
//! list (the harness's interchange JSON, or stage 6's output on a device), or WORDS when none
//! exists. Never a frozen lie, never garbage.
//!
//! Where the at-rest JSON lives (precedence):
//! 1. `$SPARQ_ATREST` — an explicit file path (the review/run-sheet door: point it at a harness
//!    artefact and the shell paints exactly that render);
//! 2. the user cache dir (`$SPARQ_ATREST_DIR`, else `$HOME/.cache/sparq/at-rest` / the Windows
//!    equivalent) as `<module-id with / → _>.ir.json` — where `sparq mod validate` stage 6 will
//!    write the device's last render (INC5).
//!
//! The store is loaded once at shell startup (control thread, filesystem legal) and repainted every
//! frame from memory — the paint path never touches a disk.

#[cfg(feature = "ui")]
use std::collections::HashMap;
use std::path::PathBuf;

use sparq_ui::displaylist::Item;

/// One at-rest render: the parsed display list plus the display rect it was laid out against
/// (the interchange carries geometry in the display's logical px).
#[derive(Clone, Debug, PartialEq)]
pub struct AtRest {
    /// The display list, in the host painter's parsed form.
    pub items: Vec<Item>,
    /// The display's logical width the list was laid out against.
    pub w: f32,
    /// The display's logical height.
    pub h: f32,
}

/// The shell's at-rest store, keyed by module id.
#[derive(Clone, Debug, Default)]
pub struct AtRestStore {
    /// The loaded surfaces. UI-face only: the `instrument render` CLI re-reads the file instead,
    /// so the default (no-`ui`) build carries no store.
    #[cfg(feature = "ui")]
    surfaces: HashMap<String, AtRest>,
    /// Per-module host-side status words (the §5.2 toolbar label from the stream provider),
    /// refreshed per frame by the shell; painted into the instrument's panel band.
    #[cfg(feature = "ui")]
    status: HashMap<String, String>,
}

/// The cache directory for at-rest renders (rule 2 above).
#[must_use]
pub fn atrest_dir() -> PathBuf {
    if let Ok(d) = std::env::var("SPARQ_ATREST_DIR") {
        return PathBuf::from(d);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    if cfg!(windows) {
        PathBuf::from(std::env::var("APPDATA").unwrap_or(home)).join("sparq/at-rest")
    } else {
        PathBuf::from(home).join(".cache/sparq/at-rest")
    }
}

/// The path the loader would read first for a module id (the explicit env door, else the cache
/// file) — what `instrument render --dir` re-reads after the store parsed it.
#[must_use]
pub fn atrest_path_used(module_id: &str) -> PathBuf {
    candidate_paths(module_id).into_iter().next().unwrap_or_else(|| atrest_dir().join("none"))
}

fn candidate_paths(module_id: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Ok(explicit) = std::env::var("SPARQ_ATREST") {
        v.push(PathBuf::from(explicit));
    }
    v.push(atrest_dir().join(format!("{}.ir.json", module_id.replace('/', "_"))));
    v
}

impl AtRestStore {
    /// Loads the at-rest render for one module id, if any candidate path holds a parseable
    /// interchange. A malformed file is words in the returned `None`'s stead — the store keeps a
    /// diagnostic the shell can show (never a half-parse).
    pub fn load(module_id: &str) -> (Option<AtRest>, Option<String>) {
        for path in candidate_paths(module_id) {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let json = match sparq_ui::json::parse(&text) {
                Ok(j) => j,
                Err(e) => {
                    return (None, Some(format!("{}: {e}", path.display())));
                },
            };
            match parse_atrest(&json) {
                Ok(a) => return (Some(a), None),
                Err(e) => return (None, Some(format!("{}: {e}", path.display()))),
            }
        }
        (None, None)
    }

    /// An empty store.
    #[cfg(feature = "ui")]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts an at-rest render (tests, and the device loader's door).
    #[cfg(feature = "ui")]
    pub fn insert(&mut self, module_id: &str, at: AtRest) {
        self.surfaces.insert(module_id.to_string(), at);
    }

    /// The at-rest render for a module, if loaded.
    #[cfg(feature = "ui")]
    #[must_use]
    pub fn get(&self, module_id: &str) -> Option<&AtRest> {
        self.surfaces.get(module_id)
    }

    /// Sets the host-side status words for a module (the §5.2 label). Streams-feature only: the
    /// label is the provider's sentence; without streams the shell has no provider to quote.
    #[cfg(all(feature = "ui", feature = "streams"))]
    pub fn set_status(&mut self, module_id: &str, words: String) {
        self.status.insert(module_id.to_string(), words);
    }

    /// The status words for a module, if any.
    #[cfg(feature = "ui")]
    #[must_use]
    pub fn status(&self, module_id: &str) -> Option<&str> {
        self.status.get(module_id).map(String::as_str)
    }
}

/// Parses the interchange's top level: the item list plus the display rect it was laid out
/// against (the harness writes `display_w`/`display_h`; absent, the items' own extents are
/// unknown to the parser, so the store refuses — an at-rest list without its rect cannot be
/// placed in a band).
fn parse_atrest(json: &sparq_ui::json::Json) -> Result<AtRest, String> {
    let items = sparq_ui::displaylist::parse_surface(json)?;
    let w = json.get("display_w").and_then(|v| v.as_f32()).ok_or_else(|| {
        "no `display_w` — an at-rest render carries the display rect it was laid out against"
            .to_string()
    })?;
    let h = json.get("display_h").and_then(|v| v.as_f32()).ok_or_else(|| {
        "no `display_h` — an at-rest render carries the display rect it was laid out against"
            .to_string()
    })?;
    Ok(AtRest { items, w, h })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn a_missing_at_rest_is_none_without_words() {
        let (a, e) = AtRestStore::load("dat/does-not-exist-anywhere");
        assert!(a.is_none() && e.is_none());
    }

    #[test]
    fn an_interchange_without_its_rect_is_refused_in_words() {
        let j = sparq_ui::json::parse(r#"{"items":[]}"#).unwrap();
        let e = parse_atrest(&j).unwrap_err();
        assert!(e.contains("display_w"), "{e}");
    }

    #[test]
    fn an_interchange_with_a_rect_parses() {
        let j =
            sparq_ui::json::parse(r#"{"items":[],"display_w":2176.0,"display_h":1120.0}"#).unwrap();
        let a = parse_atrest(&j).unwrap();
        assert_eq!((a.w, a.h), (2176.0, 1120.0));
        assert!(a.items.is_empty());
    }
}
