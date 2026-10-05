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
