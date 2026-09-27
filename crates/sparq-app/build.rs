//! Injects the build stamp. See `src/stamp.rs` for why it exists.
//!
//! No subprocesses: `clippy::disallowed_methods` denies `Command::new` workspace-wide (no
//! subprocesses on the audio path). A build script is not the audio path, so an `#[allow]` would
//! be legitimate — but dropping the dependency is cleaner, more portable, and lets the "when"
//! field carry a *better* answer. Instead of the build machine's clock, which is meaningless on
//! a re-synced tree, it reports the newest mtime among the sources: that is precisely "how old
//! is the code this binary was built from", which is the question the stamp exists to answer.
//!
//! # The fingerprint is RECURSIVE, and `scripts\build.bat` re-computes it (defect #41)
//!
//! The first version of this stamp counted only *top-level* `.rs` files in each `src/`. That was
//! fine while every module was a top-level file; the moment the HAL landed in `src/hal/`, the
//! modules most likely to change between syncs (`hal/wasapi.rs`, `rt/thread.rs`, `play/hal.rs`)
//! were invisible to the fingerprint — a stale binary could match a fresh tree exactly. Zip-based
//! syncing then made this lethal: extraction restores archive timestamps, cargo's mtime freshness
//! check saw "old" sources and skipped the build, and the top-level-only stamp agreed with the
//! stale binary. `build.bat` now recomputes this same fingerprint (recursively, in PowerShell)
//! and refuses to proceed on a mismatch — the definition of the fingerprint therefore lives in
//! BOTH files and must be changed in both: every `*.rs` under the six `src` roots, count + total
//! bytes, rendered `Nf/NB`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    for d in [
        "../sparq-kernel/src",
        "../sparq-audio/src",
        "../sparq-ui/src",
        "../sparq-module-api/src",
        "../sparq-music/src",
        "src",
    ] {
        println!("cargo:rerun-if-changed={d}");
    }

    let roots = [
        "../sparq-kernel/src",
        "../sparq-audio/src",
        "../sparq-ui/src",
        "../sparq-module-api/src",
        "../sparq-music/src",
        "src",
    ];
    let mut files = 0usize;
    let mut bytes = 0usize;
    let mut newest: Option<std::time::SystemTime> = None;

    // Recursive walk, no dependencies: a stack of directories, counting every *.rs underneath.
    let mut dirs: Vec<std::path::PathBuf> = roots.iter().map(std::path::PathBuf::from).collect();
    while let Some(dir) = dirs.pop() {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
                continue;
            }
            if !path.extension().is_some_and(|x| x == "rs") {
                continue;
            }
            if let Ok(meta) = std::fs::metadata(&path) {
                files += 1;
                bytes += meta.len() as usize;
                if let Ok(m) = meta.modified() {
                    if newest.map_or(true, |n| m > n) {
                        newest = Some(m);
                    }
                }
            }
        }
    }
    println!("cargo:rustc-env=SPARQ_SOURCE_FP={files}f/{bytes}B");

    // Newest source mtime, rendered without a date library: whole days since the Unix epoch are
    // enough to answer "is this older than my sync?", and pulling in `chrono` for a build stamp
    // would be the first third-party dependency in the core build. NOTE: on a zip-synced tree the
    // mtimes are the ARCHIVE's, so this line answers "how old was the code when the sender
    // touched it", not "when was this built" — which is still the useful question, and the
    // reason the fingerprint (content-sized, timestamp-free) is the staleness authority.
    let stamp = match newest {
        Some(t) => match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => format!("newest source {} d since epoch", d.as_secs() / 86_400),
            Err(_) => "newest source mtime unavailable".to_string(),
        },
        None => "no sources found".to_string(),
    };
    println!("cargo:rustc-env=SPARQ_BUILT_UTC={stamp}");
}
