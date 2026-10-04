//! The token-bundle golden gate (WO-017 close; freeze checklist item 8).
//!
//! WO-017's scope: *"Token bundle v1: the runtime serialisation of `design/tokens/*.toml`
//! handed to guests at `prepare` — one more generated artefact from `tools/token_gen.py`,
//! semver'd with the tokens, with a golden snapshot test."* The generator half lives in
//! `tools/token_gen.py` (`emit_bundle` + the round-trip invariant in its report, gated by
//! `--check` in CI and `gates.bat`). This file is the Rust half — the pin that lives inside
//! `cargo test`, so the two artefacts cannot disagree even between python runs:
//!
//! * `payload_sha256` in `token-bundle.json` IS the sha256 of the checked-in `tokens.json`
//!   bytes (the WIT contract's words: the payload is "the tokens.json artefact, verbatim");
//! * the envelope's semver triple IS the `"version"` string inside `tokens.json` (which is the
//!   `colors.toml [meta]` version — the tokens' own semver, token-spec §4);
//! * the encoding is `json` (tokens.wit's only v1 case).
//!
//! A token value change therefore propagates to bundle, tokens.rs, CSS and JSON in one commit,
//! or one of the two gates fails — the WO-017 acceptance criterion, mechanically.
//!
//! What this gate does NOT do, stated rather than implied: it cannot re-run the generator
//! (TOML → bundle needs python), so hand-editing the bundle's `payload` object to something
//! whose canonical serialisation differs from tokens.json is caught by `token_gen.py --check`
//! (staleness) and by the sha pin here (the envelope would no longer match its sibling), but
//! the two gates must both run — which CI does (`ci.yml`: python gates + cargo test).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::sha256::{hex, sha256};

const BUNDLE: &str = "../../design/tokens/generated/token-bundle.json";
const TOKENS_JSON: &str = "../../design/tokens/generated/tokens.json";

/// Extracts a `"key": "string"` value. No JSON parser in a zero-dependency workspace, and the
/// envelope is machine-written with stable formatting — a targeted find is honest here because
/// a MISSING key panics with the key's name instead of silently passing.
fn field_str<'a>(text: &'a str, key: &str) -> &'a str {
    let pat = format!("\"{key}\": \"");
    let start =
        text.find(&pat).unwrap_or_else(|| panic!("the bundle envelope lost its {key} field"))
            + pat.len();
    let end = start + text[start..].find('"').unwrap_or_else(|| panic!("unterminated {key}"));
    &text[start..end]
}

/// Extracts a `"key": <integer>` value.
fn field_u32(text: &str, key: &str) -> u32 {
    let pat = format!("\"{key}\": ");
    let start =
        text.find(&pat).unwrap_or_else(|| panic!("the bundle envelope lost its {key} field"))
            + pat.len();
    let end = start
        + text[start..]
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or_else(|| panic!("{key} is not a bare integer"));
    text[start..end].parse().unwrap_or_else(|e| panic!("{key}: {e}"))
}

#[test]
fn the_hand_rolled_sha256_matches_the_published_vectors() {
    // FIPS 180-4 known answers — the licence for this file to pin anything at all.
    assert_eq!(
        hex(&sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        hex(&sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    // The multi-block boundary case (56 bytes: padding overflows into a second block).
    assert_eq!(
        hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn the_bundle_pins_the_tokens_json_it_ships() {
    let bundle = common::repo_text(BUNDLE);
    let tokens_text = common::repo_text(TOKENS_JSON);
    let tokens = tokens_text.as_bytes();

    // 1. The payload hash is the sibling artefact's hash — "tokens.json, verbatim".
    let want = hex(&sha256(tokens));
    assert_eq!(
        field_str(&bundle, "payload_sha256"),
        want,
        "token-bundle.json's payload_sha256 is not the sha256 of the checked-in tokens.json — \
         run tools/token_gen.py and commit BOTH files (the one-commit propagation rule)"
    );

    // 2. The envelope's semver is the tokens' own version (colors.toml [meta] via tokens.json).
    let version = field_str(&tokens_text, "version");
    let parts: Vec<&str> = version.split('.').collect();
    assert_eq!(parts.len(), 3, "tokens.json version {version:?} is not a semver triple");
    assert_eq!(field_u32(&bundle, "major"), parts[0].parse::<u32>().unwrap());
    assert_eq!(field_u32(&bundle, "minor"), parts[1].parse::<u32>().unwrap());
    assert_eq!(field_u32(&bundle, "patch"), parts[2].parse::<u32>().unwrap());

    // 3. The encoding tokens.wit v1 allows exactly one of, and the payload shape the generator
    //    documents: a nested object whose canonical re-serialisation is tokens.json (the python
    //    gate proves the serialisation; this pin proves the envelope says `json`).
    assert_eq!(field_str(&bundle, "encoding"), "json");
    assert!(bundle.contains("\"payload\": {"), "the payload object is missing from the envelope");
}
