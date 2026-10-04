//! The WIT API-surface snapshot gate (WO-017 close; freeze checklist item 7, plan §6.7).
//!
//! Plan §6.7: *"The host API is versioned and frozen per minor; there is a machine-checked API
//! surface snapshot test in CI so accidental breaking changes fail the build."* For the native
//! contract that gate is `api_snapshot.rs` (compile-time fn-pointer pins). The instrument
//! contract's surface is TEXT — the nine WIT files under `docs/api/instrument-wit/wit/`, frozen
//! at v1.1 on 2026-10-05 — so its gate is textual: the sha256 of every file, pinned below.
//!
//! The discipline this enforces, and its honest limits:
//!
//! * An ACCIDENTAL edit (a stray keystroke, a well-meaning "fix", a merge artefact) fails here
//!   immediately, in `cargo test`, on every platform CI runs.
//! * A DELIBERATE amendment repins the hash IN THE SAME COMMIT — the review diff then shows the
//!   contract text and its pin moving together, which is the visible, reviewable act the freeze
//!   exists to force. The versioning rules ride along (additive = minor; removals/renames =
//!   major with an alias table; the `sparq:instrument@X.Y.Z` package line lives inside the
//!   hashed text, so it cannot move without the pin moving).
//! * Hashes detect CHANGE, not MEANING: whether an amendment is legal under the rules above is
//!   a review question, exactly like `api_snapshot.rs` leaving "is this signature change
//!   allowed" to the human who updates the pin. Parseability is the jco gate's job
//!   (`docs/api/instrument-wit/README.md` §7); this gate's job is that nothing moves silently.
//!
//! A new or deleted file fails the directory check too — gaining an interface IS a contract
//! change and gets the same deliberate treatment.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::sha256::{hex, sha256};

const WIT_DIR: &str = "../../docs/api/instrument-wit/wit";

/// The frozen v1.1 surface (2026-10-05, WO-017 close): file name → sha256 of its exact bytes.
const PINNED: [(&str, &str); 9] = [
    ("assets.wit", "7b64009767cbc955a2db2e13761f87d54437125936ebae935022e2aaceed2aee"),
    ("audio.wit", "67618b273d7fdc21764d9cc762366bb3ef346f6bfba1f81909b40c930224bda1"),
    ("display.wit", "5c0872ef023e67cbe04ac58e36cc21a7dd5faeeecbd2ff036c0eaf3d5dfb321c"),
    ("guest.wit", "b54da3b4e8ebb1004fcb385070cc14581e9a732541df3689193b467c36aceaa3"),
    ("host.wit", "51d4d93a2d1036e094b2ffe8608765b922397b4cc95e30330d67b2738cd39fb1"),
    ("sources.wit", "c7ecc92c28a267561b6be17b4ed7faa4e9d85ec79b6d787a38e92f8e1bd3890e"),
    ("tokens.wit", "e7c8bd64adb2d6f848883848a43f64d7dab54a3655f269e01b277e95e52e81e5"),
    ("types.wit", "4ad2a5d0af0c7b4ad9fc19663809ad09dcbedf548e8556376c82755b1fc56376"),
    ("world.wit", "45c7a4e9a9eea304161ccf351fdc3bf3c064d9fbd7801d6e30e702bb50d7db3f"),
];

#[test]
fn the_frozen_wit_surface_has_not_moved() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(WIT_DIR);

    // The directory holds exactly the pinned set: a new interface file or a deleted one is a
    // contract change and must be pinned deliberately, not smuggled in.
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let mut pinned: Vec<String> = PINNED.iter().map(|(name, _)| (*name).to_string()).collect();
    pinned.sort();
    assert_eq!(
        on_disk, pinned,
        "the wit directory gained or lost a file — that is a contract change: amend the WIT \
         version per the rules and repin PINNED in this file, in the same commit"
    );

    for (name, want) in PINNED {
        let bytes = common::repo_file(&format!("{WIT_DIR}/{name}"));
        let got = hex(&sha256(&bytes));
        assert_eq!(
            got, want,
            "{name} moved (frozen v1.1 surface). An ACCIDENTAL edit is exactly what this gate \
             exists to catch — revert it. A DELIBERATE amendment follows the versioning rules \
             (additive = minor, removals/renames = major + alias table, package line included) \
             and repins this hash in the same commit, so the review diff shows the contract and \
             its pin moving together."
        );
    }
}
