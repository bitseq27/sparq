//! Build identity, so "is the exe I am running the one I think it is?" is answerable in one line.
//!
//! This exists because of a real failure: `scripts\demo.bat` only built when `sparq.exe` was
//! *missing*, so after syncing new source the old Phase B binary kept being used. It rendered the
//! default patch and ignored `--pattern` entirely — but printed no error, because the old binary was
//! perfectly healthy, just old. A build stamp turns that from a mystery into a one-line check.

/// Crate version from `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Age of the newest source file this binary was built from, injected by `build.rs`.
///
/// Deliberately *not* a wall-clock build time: on a re-synced tree the build machine's clock tells
/// you nothing useful, whereas "newest source N h since epoch" changes exactly when the sources
/// change, which is the comparison that matters.
pub const BUILT_UTC: &str = env!("SPARQ_BUILT_UTC");

/// Source fingerprint: file count and total bytes of the crates, injected by `build.rs`.
///
/// Deliberately crude. A proper git hash needs git at build time and a `.git` directory, which a
/// zip-synced checkout does not have; a content fingerprint needs neither and still answers the only
/// question that matters here, which is "did the sources change since this binary was built?".
pub const SOURCE_FINGERPRINT: &str = env!("SPARQ_SOURCE_FP");

/// One line identifying this build.
#[must_use]
pub fn line() -> String {
    format!("{VERSION} · {BUILT_UTC} · src {SOURCE_FINGERPRINT}")
}
