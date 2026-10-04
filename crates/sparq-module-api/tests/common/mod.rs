//! Shared helpers for the contract's textual snapshot gates (`token_bundle.rs`,
//! `wit_snapshot.rs`). Lives under `tests/common/` so cargo does not build it as its own
//! test binary; each gate includes it with `mod common;`.
#![allow(dead_code)]

pub mod sha256;

/// Reads a repo file relative to this crate, the `compat_matrix.rs` discipline: read from
/// `CARGO_MANIFEST_DIR`, never from the process's working directory, so the gate cannot be
/// fooled (or broken) by where cargo was invoked. `read_to_string` (not the clippy-banned
/// `fs::read`) — every artefact these gates pin is UTF-8 text, and a non-UTF-8 contract file
/// SHOULD fail loudly.
pub fn repo_file(relative: &str) -> Vec<u8> {
    repo_text(relative).into_bytes()
}

/// The same file as UTF-8 text.
pub fn repo_text(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}
