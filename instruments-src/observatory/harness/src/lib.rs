//! `observatory-harness` — the native proof path (WO-020 plan D13).
//!
//! The harness is the sandbox's door to the instrument without wasmtime anywhere: it replays the
//! recorded fixtures (`reference/fixtures/observatory/`, captured by `tools/streams_record.py`) into
//! frozen `data-record` windows via the broker's hermetic half, feeds them through the PURE CORE, and
//! emits the two artefacts the increment is judged on:
//!
//! * the **display-list JSON interchange** (D13) — the canonical, hashable render output the goldens
//!   pin and INC4's host painters will consume;
//! * the **SVG** at the three breakpoints — the review artefact eyeballed against
//!   `design/mockups/display-sheet.svg` (§5.6's exemplar) before the device ever runs.
//!
//! Because it links `observatory-core` (never the SDK glue), the harness proves the SAME logic that
//! ships in the component, and because the SVG painter resolves the SAME token bundle the host will,
//! it demonstrates WO-019's re-theme property in the sandbox: change a token, the wall re-themes,
//! zero package bytes move.
//!
//! # Determinism (D11)
//!
//! Every input is a checked-in fixture and a fixed evaluation clock; no socket, no wall clock. The
//! same fixtures produce byte-identical IR (and a stable golden hash) run after run.

pub mod ir_json;
pub mod svg;
pub mod to_core;
pub mod tokens;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use observatory_core::ir::{Lod, Surface};
use observatory_core::layout::Metrics;
use observatory_core::params::Params;
use observatory_core::wall::{Budget, Wall};
use sparq_streams::registry::Registry;
use sparq_streams::replay;
use sparq_streams::window::Window;

/// The display's declared size (the manifest's `ui.displays[0].min_size`, §5.1) — the wall is drawn
/// at this logical size at every breakpoint; the breakpoint changes the LOD, not the display rect.
pub const DISPLAY_W: f32 = 2176.0;
/// The display's declared height.
pub const DISPLAY_H: f32 = 1120.0;

/// The path the sparq shell reads the at-rest render from (WO-020 INC4 §8.2): `$SPARQ_ATREST`
/// when set, else the shell's cache-dir convention (`<id with / → _>.ir.json` under
/// `$SPARQ_ATREST_DIR` / `$HOME/.cache/sparq/at-rest`). Mirrors `sparq_app::ui::atrest` so the
/// harness can publish exactly what the shell looks for.
#[must_use]
pub fn atrest_path() -> PathBuf {
    if let Ok(p) = std::env::var("SPARQ_ATREST") {
        return PathBuf::from(p);
    }
    let dir = match std::env::var("SPARQ_ATREST_DIR") {
        Ok(d) => PathBuf::from(d),
        Err(_) => {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".cache/sparq/at-rest")
        },
    };
    dir.join("dat_observatory.ir.json")
}

/// The default fixture directory (the recorder's output, checked in).
#[must_use]
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../reference/fixtures/observatory")
}

/// The three breakpoints the acceptance reviews (§5.7's LOD ladder maps to the layout tokens'
/// large/laptop/tablet breakpoints): the wall is the same display at three detail levels. `(name,
/// lod, the screen it stands for)`.
#[must_use]
pub fn breakpoints() -> [(&'static str, Lod, &'static str); 3] {
    [
        ("wall-large", Lod::Full, "2560×1600 (display wall, zoom 1.0 — LOD full)"),
        ("wall-laptop", Lod::Reduced, "1440×900 (laptop — LOD reduced)"),
        ("wall-tablet", Lod::Minimal, "1280×800 (stage tablet — LOD minimal)"),
    ]
}

/// Replays every fixture into a window, returning the registry, the id→window map, and the single
/// evaluation clock (the newest fetch across the set, so every window reads LIVE at its fetch — the
/// §5.4 default wall). Hermetic: reads checked-in files, no socket.
///
/// # Errors
/// A string naming the fixture/replay problem if the recorded set is missing or a stream refuses.
pub fn load_windows(dir: &Path) -> Result<(Registry, HashMap<String, Window>, i64), String> {
    let registry = Registry::load().map_err(|e| format!("registry: {e}"))?;
    let replayed = replay::replay_all(&registry, dir, None);
    let mut windows = HashMap::new();
    let mut now = 0i64;
    let mut failures = Vec::new();
    for (id, res) in replayed {
        match res {
            Ok(w) => {
                if let Some(f) = w.last_fetch_unix() {
                    now = now.max(f);
                }
                windows.insert(id, w);
            },
            Err(e) => failures.push(format!("{id}: {e}")),
        }
    }
    if !failures.is_empty() {
        // Refuse in words (never a half-built wall): a fixture that will not replay is a real drift.
        return Err(format!(
            "{} fixture(s) failed to replay (record them first: python3 tools/streams_record.py --all):\n  {}",
            failures.len(),
            failures.join("\n  ")
        ));
    }
    Ok((registry, windows, now))
}

/// Renders the wall for one breakpoint: the default params, the replayed windows, the given LOD.
/// Returns the core [`Surface`] (the caller serialises it to IR JSON and/or paints it to SVG).
#[must_use]
pub fn render_wall(
    params: &Params,
    registry: &Registry,
    windows: &HashMap<String, Window>,
    lod: Lod,
    now_unix: i64,
    time_sec: f64,
) -> Surface {
    let data = to_core::build_cell_data(params, windows, registry, now_unix);
    let inputs = to_core::as_inputs(&data);
    let mut wall = Wall::with_params(*params, Metrics::default());
    let frame = observatory_core::wall::FrameContext {
        display_id: observatory_core::DISPLAY_ID.to_string(),
        width_px: DISPLAY_W,
        height_px: DISPLAY_H,
        lod,
        time_sec,
    };
    wall.draw(&frame, &inputs)
}

/// The canonical Full-LOD default-wall IR, pretty-printed — the golden's subject. Both
/// [`emit_default_wall`] (which writes it to disk) and the golden test hash THIS string, so the
/// pinned hash and the written artefact cannot disagree.
///
/// # Errors
/// A string naming the replay problem if the fixture set will not load.
pub fn default_wall_ir_pretty() -> Result<String, String> {
    let (registry, windows, now) = load_windows(&fixtures_dir())?;
    let params = Params::default();
    let surface = render_wall(&params, &registry, &windows, Lod::Full, now, 0.0);
    serde_json::to_string_pretty(&ir_json::surface_to_json(&surface, DISPLAY_W, DISPLAY_H))
        .map_err(|e| format!("IR serialise: {e}"))
}

/// The budget of a rendered wall (the §5.8 assertion the harness reports and the validator enforces).
#[must_use]
pub fn budget_of(surface: &Surface) -> Budget {
    match surface {
        Surface::Items(items) => observatory_core::wall::count_budget(items),
        Surface::Scene(_) => Budget::default(),
    }
}

/// The medium `gpu_class` ceilings (mirrored from `sparq-host-wasm/ceilings.rs`; the harness cannot
/// link that crate without pulling the host, so the numbers are restated here and drift-checked
/// against the ceilings table's recorded anchors in `tests/golden.rs`).
pub const MEDIUM_MAX_VERTICES: u32 = 262_144;
/// The medium instance ceiling.
pub const MEDIUM_MAX_INSTANCES: u32 = 16_384;
/// The medium heat-cell ceiling.
pub const MEDIUM_MAX_HEAT_CELLS: u32 = 65_536;

/// Writes the review + golden artefacts for the default wall into `out_dir`: three breakpoint SVGs
/// and the canonical (Full-LOD) IR JSON. Returns the paths written and the Full-LOD budget.
///
/// # Errors
/// A string naming the I/O or replay problem.
pub fn emit_default_wall(out_dir: &Path) -> Result<(Vec<PathBuf>, Budget), String> {
    let fixtures = fixtures_dir();
    let (registry, windows, now) = load_windows(&fixtures)?;
    let tokens = tokens::Tokens::load()?;
    let params = Params::default();
    std::fs::create_dir_all(out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;

    let mut written = Vec::new();
    let mut full_budget = Budget::default();
    for (name, lod, _screen) in breakpoints() {
        let surface = render_wall(&params, &registry, &windows, lod, now, 0.0);
        if lod == Lod::Full {
            full_budget = budget_of(&surface);
        }
        // The IR JSON interchange (canonical for the Full LOD; the golden pins its hash).
        let json_path = out_dir.join(format!("{name}.ir.json"));
        let text = if lod == Lod::Full {
            default_wall_ir_pretty()?
        } else {
            serde_json::to_string_pretty(&ir_json::surface_to_json(&surface, DISPLAY_W, DISPLAY_H))
                .map_err(|e| format!("IR serialise: {e}"))?
        };
        write_text(&json_path, &text)?;
        written.push(json_path);
        // The SVG review artefact.
        let (svg, unresolved) = svg::render_svg_report(&surface, DISPLAY_W, DISPLAY_H, &tokens);
        let svg_path = out_dir.join(format!("{name}.svg"));
        write_text(&svg_path, &svg)?;
        written.push(svg_path);
        if !unresolved.is_empty() {
            // Fail-soft diagnostics, surfaced (never a silent hole).
            eprintln!(
                "note: {name}: {} unresolved token id(s): {}",
                unresolved.len(),
                unresolved.join(", ")
            );
        }
    }
    Ok((written, full_budget))
}

/// A UTF-8/LF text write (the house text-I/O rule, so artefacts are byte-stable across platforms).
fn write_text(path: &Path, content: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f =
        std::fs::File::create(path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    f.write_all(content.as_bytes()).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn the_fixture_set_replays_into_a_full_wall() {
        let (registry, windows, now) = load_windows(&fixtures_dir()).unwrap();
        assert!(windows.len() >= 23, "every recorded stream replayed");
        let params = Params::default();
        let surface = render_wall(&params, &registry, &windows, Lod::Full, now, 0.0);
        match surface {
            Surface::Items(items) => {
                assert!(items.len() > 100, "a 16-cell wall has many primitives")
            },
            Surface::Scene(_) => panic!("display-list only"),
        }
    }

    #[test]
    fn the_default_wall_fits_the_medium_gpu_budget() {
        // §5.8: the wall must fit `medium` with headroom. This is the fuel-free draw-bound the plan
        // asks INC3 to measure (the validator enforces the same ceilings on the device).
        let (registry, windows, now) = load_windows(&fixtures_dir()).unwrap();
        let params = Params::default();
        let surface = render_wall(&params, &registry, &windows, Lod::Full, now, 0.0);
        let b = budget_of(&surface);
        assert!(
            b.vertices <= MEDIUM_MAX_VERTICES,
            "vertices {} > {}",
            b.vertices,
            MEDIUM_MAX_VERTICES
        );
        assert!(
            b.instances <= MEDIUM_MAX_INSTANCES,
            "instances {} > {}",
            b.instances,
            MEDIUM_MAX_INSTANCES
        );
        assert!(
            b.heat_cells <= MEDIUM_MAX_HEAT_CELLS,
            "heat cells {} > {}",
            b.heat_cells,
            MEDIUM_MAX_HEAT_CELLS
        );
    }

    #[test]
    fn the_render_is_deterministic_across_runs() {
        // D11: the same fixtures produce byte-identical IR (the golden's requirement).
        let (registry, windows, now) = load_windows(&fixtures_dir()).unwrap();
        let params = Params::default();
        let a = render_wall(&params, &registry, &windows, Lod::Full, now, 0.0);
        let b = render_wall(&params, &registry, &windows, Lod::Full, now, 0.0);
        let ja =
            serde_json::to_string(&ir_json::surface_to_json(&a, DISPLAY_W, DISPLAY_H)).unwrap();
        let jb =
            serde_json::to_string(&ir_json::surface_to_json(&b, DISPLAY_W, DISPLAY_H)).unwrap();
        assert_eq!(ja, jb, "two renders of the same fixtures are byte-identical");
    }

    #[test]
    fn a_fixed_time_gives_a_stable_ticker_scroll() {
        // time_sec drives only the ticker/scroll phase; a fixed time gives a fixed render.
        let (registry, windows, now) = load_windows(&fixtures_dir()).unwrap();
        let params = Params::default();
        let a = render_wall(&params, &registry, &windows, Lod::Full, now, 42.0);
        let b = render_wall(&params, &registry, &windows, Lod::Full, now, 42.0);
        assert_eq!(
            serde_json::to_string(&ir_json::surface_to_json(&a, DISPLAY_W, DISPLAY_H)).unwrap(),
            serde_json::to_string(&ir_json::surface_to_json(&b, DISPLAY_W, DISPLAY_H)).unwrap()
        );
    }
}
