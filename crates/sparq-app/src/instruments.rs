//! `sparq mod …` — the instrument half of the library door (WO-018).
//!
//! Two subcommands over `sparq-host-wasm`'s doors, with the same discipline `sparq modules`
//! established for the backbone half: **one broken package never hides the others**, and **every
//! failure keeps its path and prints verbatim** — the CLI is a thin voice over the library, not a
//! second implementation. The browser's launch path and `sparq mod validate` must be able to
//! disagree about nothing, which is why both call the same two functions (`package::is_loadable` +
//! `validate::schema_check` for the launch door, `validate::run` for the gate).
//!
//! * `sparq mod validate DIR` — the hand-in gate (MODULE-BUILD-GUIDE §7). The static stages run;
//!   the runtime stages refuse in words. **A PARTIAL gate exits non-zero**: "a package that
//!   passes validate IS a package that loads at launch" is the gate's whole promise, and while
//!   stages refuse, the CLI must not claim the promise was kept. The words say exactly which
//!   half ran.
//! * `sparq mod list [--root DIR] [--strict]` — the launch view of `instruments/` (module-api
//!   §11's slot): what would load, what is refused and why, and the advisory words for a thin
//!   library card. `--strict` exits non-zero if anything would not load, so CI can gate on a
//!   clean instrument set (the release-binary smoke step).

use std::path::PathBuf;
use std::process::ExitCode;

use sparq_host_wasm::package;
use sparq_host_wasm::validate;

/// A command's failure is a message shown verbatim to the operator, so it is a `String`.
/// (`cli`'s own alias is private; declaring it here keeps this module independent of that.)
type Result<T> = std::result::Result<T, String>;

/// What `sparq mod` was asked to do.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModCommand {
    /// Run the hand-in gate over one package directory.
    Validate { dir: String },
    /// The launch view of an `instruments/` root.
    List { root: Option<String>, strict: bool },
}

/// Parses `sparq mod` arguments.
///
/// # Errors
/// A message naming the unrecognised shape — the subcommand vocabulary is closed
/// (`validate`, `list`), like every other door in this CLI.
pub fn parse(args: &[String]) -> Result<ModCommand> {
    let (sub, rest) = args.split_first().ok_or_else(|| {
        "`sparq mod` needs a subcommand: `validate DIR` or `list [--root DIR] [--strict]`"
            .to_string()
    })?;
    match sub.as_str() {
        "validate" => {
            let dir = rest
                .first()
                .ok_or_else(|| "`sparq mod validate` needs the package DIRECTORY".to_string())?;
            if rest.len() > 1 {
                return Err(format!(
                    "`sparq mod validate` takes exactly one directory, got {} more argument(s)",
                    rest.len() - 1
                ));
            }
            Ok(ModCommand::Validate { dir: dir.clone() })
        },
        "list" => {
            let mut root = None;
            let mut strict = false;
            let mut i = 0;
            while i < rest.len() {
                match rest[i].as_str() {
                    "--root" => {
                        i += 1;
                        root = Some(
                            rest.get(i)
                                .ok_or_else(|| "`--root` needs a directory".to_string())?
                                .clone(),
                        );
                    },
                    "--strict" => strict = true,
                    other => {
                        return Err(format!("unknown argument `{other}` for `sparq mod list`"));
                    },
                }
                i += 1;
            }
            Ok(ModCommand::List { root, strict })
        },
        other => Err(format!(
            "unknown subcommand `{other}` for `sparq mod`; try `validate DIR` or `list`"
        )),
    }
}

/// Runs the parsed subcommand.
///
/// # Errors
/// Never — refusals are printed words and exit codes, not Rust errors. (The `Result` shape is
/// the CLI's uniform dispatch contract; this door simply always answers `Ok`.)
pub fn run(cmd: ModCommand) -> Result<ExitCode> {
    match cmd {
        ModCommand::Validate { dir } => run_validate(&dir),
        ModCommand::List { root, strict } => run_list(root, strict),
    }
}

/// `sparq mod validate DIR` — the whole gate, printed stage by stage.
fn run_validate(dir: &str) -> Result<ExitCode> {
    let path = PathBuf::from(dir);
    if !path.is_dir() {
        println!("sparq mod validate: `{dir}` is not a directory.");
        println!("  A package is a DIRECTORY of at most five files (MODULE-BUILD-GUIDE §3).");
        println!("  Fix: point this at the package folder, e.g. `sparq mod validate instruments/my-fm-terrain/`.");
        return Ok(ExitCode::FAILURE);
    }
    let gate = validate::run(&path);
    println!("sparq mod validate `{dir}` — the hand-in gate (MODULE-BUILD-GUIDE §7)");
    print!("{}", gate.render());

    // The launch-time advisory words ride along: a gate report that passed the shape but has a
    // thin library card says so here, not only in the browser.
    let pkg = package::open(&path);
    for w in package::advisories(&pkg) {
        println!("  note: {w}");
    }

    if !gate.passed() {
        return Ok(ExitCode::FAILURE);
    }
    if !gate.complete() {
        // PARTIAL exits non-zero ON PURPOSE: the gate's promise is "passes validate ⇒ loads at
        // launch", and while stages 3/4/6 and the budget measurement refuse, this build cannot
        // keep that promise. The render above names every stage that refused and what would
        // make it run — WO-018's runtime increment.
        println!("  A partial gate is not a hand-in: exit 1 until the runtime stages land.");
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

/// `sparq mod list` — the launch view: what loads, what is refused and why, in words.
fn run_list(root: Option<String>, strict: bool) -> Result<ExitCode> {
    let path = PathBuf::from(root.unwrap_or_else(|| "instruments".to_string()));
    if !path.is_dir() {
        println!("sparq mod list: no `{}` directory here — nothing to discover.", path.display());
        println!("  An instrument is a DIRECTORY of at most five files (MODULE-BUILD-GUIDE §3).");
        println!("  Fix: pass --root <dir>, or create `{}` — an absent instruments/ is a legal state of a young project.", path.display());
        return Ok(ExitCode::SUCCESS);
    }

    // A package misdropped without its directory is NAMED, not silently ignored — an invisible
    // failure is the one thing the house forbids. (The folder's own README is exempt: layout.)
    let loose = package::loose_files(&path);
    if !loose.is_empty() {
        println!(
            "sparq mod list: loose file(s) in `{}` — NOT packages: {}",
            path.display(),
            loose.join(", ")
        );
        println!(
            "  Each package is its own directory of at most five files (MODULE-BUILD-GUIDE §3)."
        );
    }

    let packages = package::discover(&path);
    println!("sparq mod list: scanning `{}` — {} package(s) found", path.display(), packages.len());
    let mut refused = 0usize;
    if !loose.is_empty() && strict {
        refused += loose.len();
    }
    for pkg in &packages {
        let label = pkg.dir.display();
        // The launch door: shape first (package::is_loadable), then the static schema rules —
        // exactly what the browser runs, in the same order, with the same words.
        if let Err(report) = package::is_loadable(pkg) {
            refused += 1;
            println!("    {label} — REFUSED (package):");
            print_indented(&report.to_string());
            continue;
        }
        let schema = validate::schema_check(pkg);
        if !schema.is_empty() {
            refused += 1;
            println!("    {label} — REFUSED (schema):");
            print_indented(&schema.to_string());
            continue;
        }
        // Loaded — say what it is, from its own manifest.
        let (id, version) = pkg
            .manifest_text
            .as_deref()
            .and_then(|t| sparq_module_api::decode(t).ok())
            .map(|v| {
                (
                    v.id().to_string(),
                    v.manifest().identity.version.clone().unwrap_or_else(|| "?".to_string()),
                )
            })
            .unwrap_or_else(|| ("?".to_string(), "?".to_string()));
        println!("    {id:<34} v{version:<10} {label} — loadable");
        for w in package::advisories(pkg) {
            println!("        note: {w}");
        }
    }
    if packages.is_empty() && loose.is_empty() {
        println!("  nothing to report");
        println!("  (precedence is built-in > user modules/ > project-local > instruments/ > registry cache)");
    }
    if refused > 0 {
        println!("  {refused} refusal(s) — every one printed verbatim above, with its fix.");
        if strict {
            return Ok(ExitCode::FAILURE);
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Prints a report indented under its package line — the verbatim house shape.
fn print_indented(report: &str) {
    for line in report.lines() {
        println!("      {line}");
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn parse_reads_the_two_subcommands() {
        assert_eq!(
            parse(&args("validate instruments/my-fm-terrain/")).unwrap(),
            ModCommand::Validate { dir: "instruments/my-fm-terrain/".to_string() }
        );
        assert_eq!(parse(&args("list")).unwrap(), ModCommand::List { root: None, strict: false });
        assert_eq!(
            parse(&args("list --root somewhere --strict")).unwrap(),
            ModCommand::List { root: Some("somewhere".to_string()), strict: true }
        );
        assert_eq!(
            parse(&args("list --strict")).unwrap(),
            ModCommand::List { root: None, strict: true }
        );
    }

    #[test]
    fn parse_refuses_unknown_shapes_in_words() {
        // Every refusal names the shape it wanted — the CLI vocabulary is closed, and a typo
        // gets the fix, not a shrug.
        let e = parse(&args("")).unwrap_err();
        assert!(e.contains("validate") && e.contains("list"), "{e}");
        let e = parse(&args("install")).unwrap_err();
        assert!(e.contains("unknown subcommand `install`"), "{e}");
        let e = parse(&args("validate")).unwrap_err();
        assert!(e.contains("needs the package DIRECTORY"), "{e}");
        let e = parse(&args("validate a b")).unwrap_err();
        assert!(e.contains("exactly one directory"), "{e}");
        let e = parse(&args("list --roost x")).unwrap_err();
        assert!(e.contains("unknown argument `--roost`"), "{e}");
        let e = parse(&args("list --root")).unwrap_err();
        assert!(e.contains("`--root` needs a directory"), "{e}");
    }
}

// --------------------------------------------------------------------- `sparq instrument render`

/// `sparq instrument render` — the headless painter's door (WO-020 INC4 §8.1): an interchange file
/// (or a package's at-rest render) → resolved display list → SVG, with the §5.8 budget reported
/// against the manifest's declared `gpu_class`. No runtime, no window: the same command works on a
/// CI runner and on the stage device.
#[derive(Clone, Debug, PartialEq)]
pub enum InstrumentCommand {
    /// Render one display list to SVG.
    Render {
        /// The interchange JSON file (`--list`), or a package dir whose at-rest render is used.
        list: Option<String>,
        /// The package directory (at-rest + declared gpu_class), when not rendering a raw list.
        dir: Option<String>,
        /// The SVG output path.
        svg_out: String,
        /// Display width override (else the interchange's `display_w`).
        width: Option<f32>,
        /// Display height override.
        height: Option<f32>,
    },
}

/// Parses `sparq instrument …`. The subcommand vocabulary is closed (`render`).
///
/// # Errors
/// A message naming the unrecognised shape, in the house CLI style.
pub fn parse_instrument(args: &[String]) -> Result<InstrumentCommand> {
    let Some(sub) = args.first() else {
        return Err(
            "`sparq instrument` needs a subcommand: `render --list FILE|--dir PKG --svg-out OUT`"
                .to_string(),
        );
    };
    match sub.as_str() {
        "render" => {
            let mut list = None;
            let mut dir = None;
            let mut svg_out: Option<String> = None;
            let mut width = None;
            let mut height = None;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--list" => {
                        i += 1;
                        list = Some(args.get(i).cloned().ok_or("--list needs a file")?);
                    },
                    "--dir" => {
                        i += 1;
                        dir = Some(args.get(i).cloned().ok_or("--dir needs a package directory")?);
                    },
                    "--svg-out" => {
                        i += 1;
                        svg_out = Some(args.get(i).cloned().ok_or("--svg-out needs a path")?);
                    },
                    "--width" => {
                        i += 1;
                        width = Some(
                            args.get(i)
                                .and_then(|v| v.parse().ok())
                                .ok_or("--width needs a number")?,
                        );
                    },
                    "--height" => {
                        i += 1;
                        height = Some(
                            args.get(i)
                                .and_then(|v| v.parse().ok())
                                .ok_or("--height needs a number")?,
                        );
                    },
                    other => {
                        return Err(format!("unknown `sparq instrument render` flag `{other}`"))
                    },
                }
                i += 1;
            }
            let svg_out = svg_out.ok_or("`sparq instrument render` needs --svg-out OUT")?;
            if list.is_none() && dir.is_none() {
                return Err(
                    "choose the input: --list FILE (interchange JSON) or --dir PKG (at-rest)"
                        .to_string(),
                );
            }
            Ok(InstrumentCommand::Render { list, dir, svg_out, width, height })
        },
        other => Err(format!("unknown subcommand `{other}`; try `render`")),
    }
}

/// Runs `sparq instrument render`.
///
/// # Errors
/// A message naming the I/O or parse defect, in words.
pub fn run_instrument(cmd: InstrumentCommand) -> Result<ExitCode> {
    let InstrumentCommand::Render { list, dir, svg_out, width, height } = cmd;
    let (text, gpu_class, w0, h0) = match (&list, &dir) {
        (Some(file), _) => {
            let text =
                std::fs::read_to_string(file).map_err(|e| format!("cannot read {file}: {e}"))?;
            (text, "medium".to_string(), None, None)
        },
        (None, Some(d)) => {
            let pkg = sparq_host_wasm::package::open(std::path::Path::new(d));
            let mtext =
                pkg.manifest_text.ok_or_else(|| format!("{d}: no readable sparqmod.toml"))?;
            // gpu_class is validated from the raw table (the contract model does not carry it
            // yet — validate.rs reads it the same way), so read it the same way here.
            let gpu = sparq_module_api::toml::parse(&mtext)
                .ok()
                .and_then(|t| t.get("resources").and_then(|v| v.as_table()).cloned())
                .and_then(|r| r.get("gpu_class").and_then(|v| v.as_str()).map(str::to_string))
                .unwrap_or_else(|| "none".to_string());
            let id =
                sparq_module_api::decode(&mtext).map(|m| m.id().to_string()).unwrap_or_default();
            let (at, words) = crate::ui::atrest::AtRestStore::load(&id);
            let at = at.ok_or_else(|| {
                format!(
                    "{d}: no at-rest render for `{id}`{}",
                    words.map(|w| format!(" ({w})")).unwrap_or_else(|| {
                        " — run the instrument's harness, or `sparq mod validate` on a device, \
                         or point --list at an interchange JSON"
                            .to_string()
                    })
                )
            })?;
            // Re-serialise is wasteful; instead return the parsed pieces via a second channel:
            // rebuild the JSON text from the store is not available, so re-read the file path.
            let path = crate::ui::atrest::atrest_path_used(&id);
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot re-read {}: {e}", path.display()))?;
            (text, gpu, Some(at.w), Some(at.h))
        },
        (None, None) => unreachable!("parse requires one of --list/--dir"),
    };
    let json = sparq_ui::json::parse(&text).map_err(|e| format!("interchange JSON: {e}"))?;
    let w =
        width.or(w0).or_else(|| json.get("display_w").and_then(|v| v.as_f32())).unwrap_or(2176.0);
    let h =
        height.or(h0).or_else(|| json.get("display_h").and_then(|v| v.as_f32())).unwrap_or(1120.0);
    let items =
        sparq_ui::displaylist::parse_surface(&json).map_err(|e| format!("display list: {e}"))?;
    let mut painter = sparq_ui::displaylist::Painter::new();
    let painted = painter.resolve(&items);
    let diag = painter.diagnostics();
    let budget = sparq_ui::displaylist::count_budget(&items);
    let bg = sparq_ui::displaylist::Rgba::from_hex(
        sparq_ui::tokens::color_hex("color.ground.panel").unwrap_or("#151210"),
        1.0,
    );
    let svg = sparq_ui::displaylist::svg(&painted, w, h, bg);
    std::fs::write(&svg_out, svg).map_err(|e| format!("cannot write {svg_out}: {e}"))?;
    println!("sparq instrument render: {svg_out} ({w:.0}×{h:.0}, {} primitives)", painted.len());
    println!(
        "  budget: {} vertices / {} instances / {} heat cells (declared gpu_class `{gpu_class}`)",
        budget.vertices, budget.instances, budget.heat_cells
    );
    if let Some(c) = sparq_ui::displaylist::ceilings_for(&gpu_class) {
        println!(
            "  fits declared class: {}",
            if budget.fits(c) { "YES" } else { "NO — over budget" }
        );
    }
    for d in &diag {
        println!("  diagnostic: {d}");
    }
    Ok(ExitCode::SUCCESS)
}
