//! `sparq modules` — discovery: walk a directory, read every `sparqmod.toml`, report.
//!
//! This is the half of WO-007's "filesystem scan of `modules/` + built-in registry" that touches a
//! filesystem, and it lives here rather than in `sparq-module-api` on purpose: that crate holds **no
//! I/O at all** and takes manifest text, so the contract stays testable without creating a file and
//! cannot violate the `clippy.toml` ban on I/O by accident. Discovery is a control-thread activity;
//! this is the control thread.
//!
//! Two rules shape the output. **One broken manifest never hides the others** — discovery runs at
//! startup, and a single bad download must not take the instrument's module list with it. **Every
//! failure keeps its path**, because the report is shown verbatim, so an author sees exactly which
//! file did not load and why.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sparq_module_api::discovery::{Origin, Source};
use sparq_module_api::{discover, DiscoveryReport};

/// A command's failure is a message shown verbatim to the operator, so it is a `String`.
/// (`cli`'s own alias is private; declaring it here keeps this module independent of that.)
type Result<T> = std::result::Result<T, String>;

/// The manifest file name the schema fixes.
const MANIFEST_NAME: &str = "sparqmod.toml";

/// How deep to descend. A module is a directory containing `sparqmod.toml`; the cap exists so a
/// symlink loop or an accidentally-pointed-at home directory cannot hang startup.
const MAX_DEPTH: usize = 6;

/// Options for `sparq modules`.
#[derive(Default)]
pub struct ModulesOpts {
    /// Where to look. Defaults to `modules/` under the current directory.
    pub root: Option<String>,
    /// Exit non-zero if anything failed to load, so CI can gate on a clean module set.
    pub strict: bool,
}

/// Parses `sparq modules` arguments.
///
/// # Errors
/// A message naming the unrecognised flag.
pub fn parse(args: &[String]) -> Result<ModulesOpts> {
    let mut opts = ModulesOpts::default();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                opts.root = Some(
                    args.get(i).ok_or_else(|| "`--root` needs a directory".to_string())?.clone(),
                );
            },
            "--strict" => opts.strict = true,
            other => return Err(format!("unknown argument `{other}` for `sparq modules`")),
        }
        i += 1;
    }
    Ok(opts)
}

/// Runs discovery and prints the report.
///
/// # Errors
/// Only if the root directory cannot be read.
pub fn run(opts: ModulesOpts) -> Result<ExitCode> {
    let root = PathBuf::from(opts.root.unwrap_or_else(|| "modules".to_string()));
    if !root.is_dir() {
        println!("sparq modules: no `{}` directory here — nothing to discover.", root.display());
        println!("  A module is a directory containing `{MANIFEST_NAME}`.");
        println!("  Fix: pass --root <dir>, or create `{}`.", root.display());
        return Ok(ExitCode::SUCCESS);
    }

    let files = scan(&root);
    println!("sparq modules: scanning `{}` — {} manifest(s) found", root.display(), files.len());
    let sources: Vec<Source> = files
        .iter()
        .map(|(path, text)| {
            Source::new(Origin::UserModules, path.display().to_string(), text.clone())
        })
        .collect();
    let report = discover(sources);
    print_report(&report, &root);

    if opts.strict && !report.failed.is_empty() {
        return Ok(ExitCode::FAILURE);
    }
    Ok(ExitCode::SUCCESS)
}

/// Every `sparqmod.toml` under `root`, as (path, text), sorted so the order is reproducible.
///
/// Sorted on purpose: discovery order feeds registration order, and a module list that varies
/// between runs is a journal replay that does not replay (ADR-007).
#[must_use]
pub fn scan(root: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // Never follow the build output or a VCS directory: both can contain large trees and
                // neither can contain a module anyone meant to ship.
                let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                if matches!(name.as_deref(), Some("target") | Some(".git") | Some("node_modules")) {
                    continue;
                }
                stack.push((path, depth + 1));
            } else if path.file_name().is_some_and(|n| n == MANIFEST_NAME) {
                // `read_to_string`, not `fs::read`: the text is what the contract crate consumes, and
                // a manifest that is not valid UTF-8 is reported as a failure rather than panicking.
                match std::fs::read_to_string(&path) {
                    Ok(text) => out.push((path, text)),
                    Err(e) => out.push((path, format!("[unreadable: {e}]"))),
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Prints the report: what loaded, what did not and why, and what shadows what.
fn print_report(report: &DiscoveryReport, root: &Path) {
    if report.loaded.is_empty() && report.failed.is_empty() {
        println!("  nothing to report");
        return;
    }
    println!("  loaded {}:", report.loaded.len());
    for l in &report.loaded {
        let m = &l.manifest;
        println!(
            "    {:<34} v{:<10} {} ports, {} params",
            m.id(),
            m.manifest().identity.version.as_deref().unwrap_or("?"),
            m.ports().len(),
            m.manifest().params.len()
        );
    }
    if !report.failed.is_empty() {
        println!("  refused {}:", report.failed.len());
        // `render()` is the verbatim form the module browser shows: field, value found, values
        // allowed, and the fix.
        print!("{}", report.render());
    }
    let shadowed = report.shadowed();
    if !shadowed.is_empty() {
        println!("  shadowed {} id(s):", shadowed.len());
        for s in &shadowed {
            println!("    {} — {} wins, hiding {}", s.id, s.kept, s.hidden.join(", "));
        }
        println!("  (precedence is built-in > user modules/ > project-local > instruments/ > registry cache)");
    }
    let _ = root;
}
