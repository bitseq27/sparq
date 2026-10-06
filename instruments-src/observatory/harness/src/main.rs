//! The harness CLI: replay the fixtures → render the wall → write the IR JSON + SVG review artefacts
//! and report the §5.8 budget. Run from the repo root or anywhere (paths are manifest-relative).
//!
//! ```sh
//! cargo run -p observatory-harness -- --out instruments-src/observatory/artefacts
//! cargo run -p observatory-harness -- --report          # budget only, no files
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("artefacts");
    let mut report_only = false;
    let mut atrest = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" | "--out-dir" => {
                i += 1;
                match args.get(i) {
                    Some(p) => out_dir = PathBuf::from(p),
                    None => return fail("--out needs a directory"),
                }
            },
            "--report" => report_only = true,
            "--atrest" => atrest = true,
            "--help" | "-h" => {
                println!(
                    "observatory-harness — render the wall from recorded fixtures (WO-020 D13)"
                );
                println!("  --out DIR   write artefacts to DIR (default: instruments-src/observatory/artefacts)");
                println!("  --report    print the §5.8 budget and exit without writing files");
                return ExitCode::SUCCESS;
            },
            other => return fail(&format!("unknown argument `{other}`; try --help")),
        }
        i += 1;
    }

    if report_only {
        return match report_budget() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&e),
        };
    }

    match observatory_harness::emit_default_wall(&out_dir) {
        Ok((written, budget)) => {
            if atrest {
                // The shell's at-rest door (WO-020 INC4 §8.2): copy the Full-LOD interchange into
                // the cache dir the shell reads (or $SPARQ_ATREST_DIR), so `sparq ui` paints the
                // wall without env acrobatics.
                let src = out_dir.join("wall-large.ir.json");
                let dst = observatory_harness::atrest_path();
                if let Some(parent) = dst.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                match std::fs::copy(&src, &dst) {
                    Ok(_) => println!("  at-rest render -> {}", dst.display()),
                    Err(e) => eprintln!("  at-rest copy failed: {e}"),
                }
            }
            println!(
                "observatory-harness: rendered the default 4×4 wall from the recorded fixtures"
            );
            for p in &written {
                println!("  wrote {}", p.display());
            }
            println!(
                "  budget (Full LOD): {} vertices / {} instances / {} heat cells  (medium ceilings: {} / {} / {})",
                budget.vertices,
                budget.instances,
                budget.heat_cells,
                observatory_harness::MEDIUM_MAX_VERTICES,
                observatory_harness::MEDIUM_MAX_INSTANCES,
                observatory_harness::MEDIUM_MAX_HEAT_CELLS,
            );
            let fits = budget.vertices <= observatory_harness::MEDIUM_MAX_VERTICES
                && budget.instances <= observatory_harness::MEDIUM_MAX_INSTANCES
                && budget.heat_cells <= observatory_harness::MEDIUM_MAX_HEAT_CELLS;
            println!(
                "  budget fits declared gpu_class=medium: {}",
                if fits { "YES" } else { "NO — REFUSED" }
            );
            if fits {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        },
        Err(e) => fail(&e),
    }
}

/// Prints the Full-LOD budget without writing artefacts.
fn report_budget() -> Result<(), String> {
    let (registry, windows, now) =
        observatory_harness::load_windows(&observatory_harness::fixtures_dir())?;
    let params = observatory_core::params::Params::default();
    let surface = observatory_harness::render_wall(
        &params,
        &registry,
        &windows,
        observatory_core::ir::Lod::Full,
        now,
        0.0,
    );
    let b = observatory_harness::budget_of(&surface);
    println!(
        "observatory wall budget: {} vertices / {} instances / {} heat cells",
        b.vertices, b.instances, b.heat_cells
    );
    println!(
        "medium ceilings:         {} / {} / {}",
        observatory_harness::MEDIUM_MAX_VERTICES,
        observatory_harness::MEDIUM_MAX_INSTANCES,
        observatory_harness::MEDIUM_MAX_HEAT_CELLS
    );
    Ok(())
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("observatory-harness: {msg}");
    ExitCode::FAILURE
}
