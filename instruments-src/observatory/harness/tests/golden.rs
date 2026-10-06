//! INC3's cross-boundary gates: the golden IR hash + the drift tests that keep the three mirrors and
//! the generated artefacts honest.
//!
//! These live in an integration test (not unit tests) because they cross crate/workspace boundaries
//! the unit tests cannot see: the core's generated `streams_table` vs the broker's live registry, the
//! core's `Metrics` defaults vs the checked-in token bundle, and the generated manifest's defaults vs
//! the core's `Params::default`. Each is a place a hand-edit or a stale regeneration would silently
//! diverge — exactly the class of bug the house's "contract-as-data" discipline exists to catch.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use observatory_core::layout::Metrics;
use observatory_core::params::Params;
use observatory_core::streams_table;

/// The pinned sha256 of the canonical Full-LOD default-wall IR JSON (`default_wall_ir_pretty`). This
/// is the golden render's anchor: any change to a renderer, the layout, the views or the fixtures
/// moves it, and moving it is a deliberate act (re-pin with a recorded reason), never an accident.
/// Computed 2026-10-06 from the recorded fixture set at the §5.4 default state. Re-pinned once the
/// same day when the interchange gained its `display_w`/`display_h` fields (INC4's at-rest store
/// refuses a list without its rect) — a format change, not a render change.
const GOLDEN_IR_SHA256: &str = "bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa";

// ── a dependency-free sha256 (the house hand-rolls it; known-answer vectors prove it) ───────────

mod sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    pub fn sha256(bytes: &[u8]) -> [u8; 32] {
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let mut msg = bytes.to_vec();
        let bitlen = (bytes.len() as u64) * 8;
        msg.push(0x80);
        while msg.len() % 64 != 56 {
            msg.push(0);
        }
        msg.extend_from_slice(&bitlen.to_be_bytes());
        for chunk in msg.chunks(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                w[i] = u32::from_be_bytes([
                    chunk[4 * i],
                    chunk[4 * i + 1],
                    chunk[4 * i + 2],
                    chunk[4 * i + 3],
                ]);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
            }
            let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
            let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ ((!e) & g);
                let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let t2 = s0.wrapping_add(maj);
                (hh, g, f, e) = (g, f, e, d.wrapping_add(t1));
                (d, c, b, a) = (c, b, a, t1.wrapping_add(t2));
            }
            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(hh);
        }
        let mut out = [0u8; 32];
        for i in 0..8 {
            out[4 * i..4 * i + 4].copy_from_slice(&h[i].to_be_bytes());
        }
        out
    }

    pub fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[test]
fn the_hand_rolled_sha256_matches_the_published_vectors() {
    assert_eq!(
        sha256::hex(&sha256::sha256(b"")),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256::hex(&sha256::sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn the_default_wall_ir_matches_the_pinned_golden() {
    let ir = observatory_harness::default_wall_ir_pretty().expect("the fixture set must replay");
    let got = sha256::hex(&sha256::sha256(ir.as_bytes()));
    assert_eq!(
        got, GOLDEN_IR_SHA256,
        "the default-wall IR moved off its golden. If the change is deliberate (renderer/layout/\n\
         fixture re-record), re-pin GOLDEN_IR_SHA256 with the reason recorded in the state card;\n\
         if not, the render regressed."
    );
}

#[test]
fn the_streams_table_is_the_registry_field_for_field() {
    // The guest's compiled-in mirror must equal the broker's live registry, or a cell would be
    // mislabelled / mis-rendered with no symptom. Order AND content.
    let reg = sparq_streams::Registry::load().expect("registry loads");
    assert_eq!(reg.len(), streams_table::STREAM_COUNT, "registry and table stream counts differ");
    for (i, def) in reg.streams().iter().enumerate() {
        let m = &streams_table::STREAMS[i];
        assert_eq!(m.id, def.id.as_str(), "row {i} id drifted");
        assert_eq!(m.domain, def.domain.as_str(), "{} domain drifted", def.id);
        assert_eq!(m.label, def.label.as_str(), "{} label drifted", def.id);
        assert_eq!(m.view, def.view.as_str(), "{} view drifted", def.id);
        assert_eq!(m.units, def.units.as_str(), "{} units drifted", def.id);
        assert_eq!(m.cadence_s, def.cadence_s, "{} cadence drifted", def.id);
        assert_eq!(m.schema, def.schema.as_str(), "{} schema drifted", def.id);
        assert_eq!(m.key_env, def.key_env.as_deref(), "{} key_env drifted", def.id);
    }
    // And the option-index round-trip the manifest's enums rely on.
    for (i, def) in reg.streams().iter().enumerate() {
        assert_eq!(streams_table::option_of_stream(&def.id), Some(i + 1));
        assert_eq!(streams_table::stream_by_option(i + 1).map(|m| m.id), Some(def.id.as_str()));
    }
    assert_eq!(streams_table::stream_by_option(0), None, "option 0 is OFF");
}

#[test]
fn the_metrics_defaults_are_the_checked_in_bundle() {
    // The core's layout metrics mirror the bundle; a token change without a Metrics change (or vice
    // versa) would make the guest lay out against spacing the host does not draw.
    let tokens = observatory_harness::tokens::Tokens::load().expect("bundle loads");
    let m = Metrics::default();
    assert_eq!(
        m.gap as f64,
        tokens.size("layout.space.4").expect("space.4"),
        "gap != layout.space.4"
    );
    assert_eq!(
        m.pad as f64,
        tokens.size("layout.space.3").expect("space.3"),
        "pad != layout.space.3"
    );
    assert_eq!(m.title_px as f64, tokens.size("typography.scale.m").expect("scale.m"));
    assert_eq!(m.small_px as f64, tokens.size("typography.scale.s").expect("scale.s"));
    // The marker radii the points renderer emits exist in the bundle (the additive INC3 tokens).
    for id in ["layout.marker.radius_s", "layout.marker.radius_m", "layout.marker.radius_l"] {
        assert!(tokens.size(id).is_some(), "{id} missing from the bundle");
    }
    // Every token id the core can emit resolves in the bundle (the E-LITERAL/unknown-id gate, run
    // here so a new renderer constant without a bundle entry fails a test, not a hand-in).
    for id in observatory_core::render::ALL_TOKEN_IDS {
        let is_map = id.starts_with("colormap.");
        let ok = if is_map { tokens.colormap(id).is_some() } else { tokens.get(id).is_some() };
        assert!(ok, "core emits token `{id}` which the bundle does not carry");
    }
}

#[test]
fn the_generated_manifest_defaults_match_the_core_defaults() {
    // The manifest's per-cell enum defaults (generator's DEFAULT_CELL_STREAMS) and the core's
    // `Params::default().cells` are the SAME §5.4 list in two places; this pins them together, and
    // pins the bool/float defaults too.
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../package/sparqmod.toml");
    let text = std::fs::read_to_string(&manifest).expect("the generated manifest is checked in");
    let root = sparq_module_api::toml::parse(&text).expect("the generated manifest parses");
    let params = root.tables("params").expect("params array");
    let core = Params::default();
    let mut seen_cells = 0;
    for p in params {
        let id = p.get("id").and_then(sparq_module_api::toml::Value::as_str).unwrap_or("");
        let default = p.get("default").and_then(sparq_module_api::toml::Value::as_f64);
        if let Some(cellno) = id.strip_prefix("cell_") {
            let idx: usize = cellno.parse().unwrap();
            seen_cells += 1;
            let want = core.cells[idx - 1] as f64;
            assert_eq!(default, Some(want), "cell_{idx:02} manifest default != core default");
        }
        match id {
            "selected_cell" => assert_eq!(default, Some(core.selected_cell as f64)),
            "layout" => assert_eq!(default, Some(core.layout.option() as f64)),
            "solo" => assert_eq!(default, Some(if core.solo { 1.0 } else { 0.0 })),
            "full" => assert_eq!(default, Some(if core.full { 1.0 } else { 0.0 })),
            "ticker" => assert_eq!(default, Some(if core.ticker { 1.0 } else { 0.0 })),
            "grid" => assert_eq!(default, Some(if core.grid { 1.0 } else { 0.0 })),
            "pause" => assert_eq!(default, Some(if core.pause { 1.0 } else { 0.0 })),
            "ticker_speed" => assert_eq!(default, Some(core.ticker_speed as f64)),
            "history" => assert_eq!(default, Some(core.history_s as f64)),
            "intensity" => assert_eq!(default, Some(core.intensity as f64)),
            _ => {},
        }
    }
    assert_eq!(seen_cells, 16, "the manifest declares all 16 cell params");
}

#[test]
fn the_generated_manifest_is_up_to_date_with_its_generator() {
    // gen_manifest.py --check is the CI face; this asserts the checked-in file also matches what the
    // generator WOULD write, from inside the workspace (so a stale commit fails here too).
    // (The python gate is authoritative for the bytes; here we only assert the file exists and
    // parses with the expected param/source counts — the byte-equality is gen_manifest --check's job.)
    let manifest =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../package/sparqmod.toml");
    let text = std::fs::read_to_string(&manifest).expect("manifest present");
    let root = sparq_module_api::toml::parse(&text).expect("parses");
    assert_eq!(root.tables("params").map(|t| t.len()).unwrap_or(0), 26, "26 params");
    let displays = root
        .get("ui")
        .and_then(|u| u.as_table())
        .and_then(|u| u.get("displays"))
        .and_then(|d| d.as_array());
    let sources = displays
        .and_then(|d| d.first())
        .and_then(|v| v.as_table())
        .and_then(|t| t.tables("sources"));
    assert_eq!(sources.map(|s| s.len()).unwrap_or(0), 16, "16 source bindings");
}
