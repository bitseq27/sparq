//! The runtime half's integration tests — instrument-host.md §7's test plan as code.
//!
//! These ride the `instrument-host` feature and therefore compile where wasmtime compiles: the
//! device (≥ 2 GB, the run-sheet's §D) and CI's feature-on cell (ubuntu-latest). The 1 GB build
//! sandbox REFUSES them by memory, categorically (`docs/instrument-host.md` §8's measurement:
//! cranelift-codegen alone exceeds the box) — this file's existence there is the recipe, not the
//! proof; the proof runs where the gate says.
//!
//! What is proven here, per §7:
//! 1. the smoke mirror over the noop fixture (the JS harness's 11 checks, Rust face);
//! 2. the noop component's sha pin (a fixture that moves is a recorded rebuild, never a drift);
//! 3. fuel → `BlockStatus::Overrun` through the real `Module` face (the watchdog's vocabulary);
//! 4. the identity cross-check refuses a lying component in words (§2.3);
//! 5. the port-less v1 loader refuses a ported package in words (never a half path);
//! 6. the OBSERVATORY golden: the wasm path's IR hashes to the HARNESS's pinned sha — the
//!    cross-boundary anchor (two producers of one interchange, byte-equal or the gate fails);
//! 7. two independent loads render bit-identically (the reload determinism half of §7.4);
//! 8. the doors in isolation (assets scoping, the audio instance's closed sources door, the
//!    token bundle's bytes) — no component needed, just the host side of the contract;
//! 9. stage 6 end-to-end on a TEMP copy of the Observatory: preview.svg regenerated through the
//!    shell's own painter, the at-rest IR published, the ceilings measured.

#![cfg(feature = "instrument-host")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use sparq_host_wasm::runtime::load::{InstrumentRuntime, LoadContext};
use sparq_host_wasm::runtime::stages::{golden_stage, smoke_stage, visual_stage, RuntimeContext};
use sparq_host_wasm::sources::ReplayProvider;
use sparq_module_api::module::{BlockStatus, Module};

/// The noop fixture's sha256 (instrument-host §7.2: rebuilt and pinned; the pin moves only with
/// a recorded rebuild, like every golden). Built 2026-10-06 in the INC5 sandbox:
/// `cargo build --release --target wasm32-unknown-unknown -p noop-instrument` (SDK 1.1.0,
/// rustc 1.99.0) → `wasm-tools component new` (1.261.0) → 53 747 B.
const NOOP_SHA256: &str = "df14d18f781ea477596db552e60bdb9df06344bc673593c38148e28a3d546c65";

/// The Observatory golden IR's sha256 — the HARNESS's pin (instruments-src golden.rs), asserted
/// here over the WASM path: the shipped component, driven through the real doors over the real
/// fixtures, renders the same bytes the pure core renders. The cross-boundary anchor.
const OBSERVATORY_GOLDEN_IR_SHA256: &str =
    "bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn noop_pkg() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/noop")
}

fn observatory_pkg() -> PathBuf {
    root().join("instruments/observatory")
}

fn fixtures() -> PathBuf {
    root().join("reference/fixtures/observatory")
}

fn temp_dir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("sparq-rt-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn copy_package(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_file() {
            std::fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
        }
    }
}

fn runtime_ctx(
    fixtures_dir: PathBuf,
    atrest: Option<PathBuf>,
    write_previews: bool,
) -> RuntimeContext {
    RuntimeContext {
        fixtures_dir,
        cache_dir: None, // tests never touch the user cache
        atrest_path: atrest,
        write_previews,
        sample_rate: 48_000,
        block_frames: 64,
    }
}

fn provider_now() -> (Arc<ReplayProvider>, i64) {
    let p = ReplayProvider::load(&fixtures()).expect("the checked-in fixtures replay");
    let now = p.now();
    (Arc::new(p), now)
}

fn load_ctx(now_unix: i64) -> LoadContext {
    let (provider, _) = provider_now();
    LoadContext {
        provider,
        now_unix,
        seed_root: 42,
        node_path: vec!["test".into(), "node-0".into()],
        sample_rate: 48_000,
        block_frames: 64,
    }
}

// ── §7.2 the fixture pin ───────────────────────────────────────────────────────────────────────

#[test]
fn the_noop_fixture_component_is_the_pinned_bytes() {
    let bytes = std::fs::read(noop_pkg().join("noop.wasm")).unwrap();
    let got = sparq_host_wasm::runtime::convert::sha256_hex(&bytes);
    assert_eq!(
        got, NOOP_SHA256,
        "the noop fixture moved — a fixture rebuild is a deliberate act: re-pin NOOP_SHA256 with \
         the toolchain versions recorded beside it (instrument-host §7.2)"
    );
}

// ── §7.1 the smoke mirror ──────────────────────────────────────────────────────────────────────

#[test]
fn the_smoke_stage_passes_on_the_noop_fixture() {
    let pkg = sparq_host_wasm::package::open(&noop_pkg());
    let ctx = runtime_ctx(fixtures(), None, false);
    let out = smoke_stage(&pkg, &ctx);
    assert_eq!(
        out.verdict,
        sparq_host_wasm::Verdict::Pass,
        "the smoke mirror failed:\n{}",
        out.words
    );
    assert!(out.words.contains("checks pass"), "{}", out.words);
    // The mirror names its evidence: the lifecycle words carry the fuel and the shapes.
    assert!(out.words.contains("process →"), "{}", out.words);
    assert!(out.words.contains("save-state twice"), "{}", out.words);
}

// ── §7.3 fuel → the watchdog's vocabulary ──────────────────────────────────────────────────────

#[test]
fn a_fuel_overrun_becomes_the_watchdogs_overrun_in_words() {
    let (_, now) = provider_now();
    let runtime = InstrumentRuntime::new(None).unwrap();
    let mut loaded = runtime.load_package(&noop_pkg(), &load_ctx(now)).unwrap();
    // Starve the block budget: 1 fuel unit cannot run a block. The manifest declared 1 000 000;
    // the test clamps the ENFORCEMENT to prove the trap→Overrun mapping (the §3 row), which is
    // the behaviour a genuinely over-budget guest gets.
    loaded.caps.max_fuel = 1;
    let (mut instrument, _display) = loaded.split();
    let block = sparq_kernel::block::BlockContext::offline(48_000, 64, 1);
    let params = sparq_module_api::params::ParamSet::zeroed();
    let mut out = vec![0.0f32; 64];
    let status = {
        let mut ctx = sparq_module_api::module::AudioCtx::single(&block, &params, &[], &mut out);
        instrument.process(&mut ctx)
    };
    assert_eq!(
        status,
        BlockStatus::Overrun,
        "a fuel-exhausted process is the watchdog's Overrun — the executor auto-bypasses with \
         words and the set continues (instrument-host §3); diagnostics: {:?}",
        instrument.diagnostics
    );
}

#[test]
fn an_honest_block_runs_within_its_budget() {
    let (_, now) = provider_now();
    let runtime = InstrumentRuntime::new(None).unwrap();
    let loaded = runtime.load_package(&noop_pkg(), &load_ctx(now)).unwrap();
    let declared = loaded.caps.max_fuel;
    let (mut instrument, _display) = loaded.split();
    let block = sparq_kernel::block::BlockContext::offline(48_000, 64, 1);
    let params = sparq_module_api::params::ParamSet::zeroed();
    let mut out = vec![0.0f32; 64];
    let status = {
        let mut ctx = sparq_module_api::module::AudioCtx::single(&block, &params, &[], &mut out);
        instrument.process(&mut ctx)
    };
    assert_eq!(status, BlockStatus::Silenced, "the noop's honest statement");
    assert!(
        instrument.fuel_max_block < declared,
        "the honest block fits its budget: {} < {declared}",
        instrument.fuel_max_block
    );
}

// ── §2.3 / v1 limits: the load refusals ────────────────────────────────────────────────────────

#[test]
fn a_component_that_lies_about_its_id_is_refused_in_words() {
    let dir = temp_dir("lying-id");
    let pkg = dir.join("pkg");
    copy_package(&noop_pkg(), &pkg);
    // The manifest claims a different id than the component carries.
    let manifest = std::fs::read_to_string(pkg.join("sparqmod.toml")).unwrap();
    let tampered = manifest.replace("example/syn/noop", "example/syn/someone-else");
    std::fs::write(pkg.join("sparqmod.toml"), tampered).unwrap();
    let (_, now) = provider_now();
    let runtime = InstrumentRuntime::new(None).unwrap();
    let err = runtime.load_package(&pkg, &load_ctx(now)).unwrap_err();
    assert!(err.contains("example/syn/noop"), "{err}");
    assert!(err.contains("example/syn/someone-else"), "{err}");
    assert!(err.contains("provenance"), "the refusal says why it matters: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_ported_instrument_refuses_at_load_in_words() {
    let dir = temp_dir("ported");
    let pkg = dir.join("pkg");
    copy_package(&noop_pkg(), &pkg);
    let manifest = std::fs::read_to_string(pkg.join("sparqmod.toml")).unwrap();
    let ported = manifest.replace(
        "# ports: NONE (the v1 loader serves port-less instruments; plan D5).",
        r#"[[ports]]
id = "in"
name = "Input"
direction = "in"
type = "audio"
channel_set = "stereo"

[[ports]]
id = "out"
name = "Output"
direction = "out"
type = "audio"
channel_set = "stereo""#,
    );
    std::fs::write(pkg.join("sparqmod.toml"), ported).unwrap();
    let (_, now) = provider_now();
    let runtime = InstrumentRuntime::new(None).unwrap();
    let err = runtime.load_package(&pkg, &load_ctx(now)).unwrap_err();
    assert!(
        err.contains("PORT-LESS") || err.contains("port"),
        "the v1 loader refuses a ported package in words, never a half path: {err}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ── §7.4 / stage 4: the cross-boundary golden ──────────────────────────────────────────────────

#[test]
fn the_observatory_wasm_path_renders_the_harness_golden() {
    let pkg = sparq_host_wasm::package::open(&observatory_pkg());
    let ctx = runtime_ctx(fixtures(), None, false);
    let out = golden_stage(&pkg, &ctx);
    assert_eq!(
        out.verdict,
        sparq_host_wasm::Verdict::Pass,
        "the golden render failed:\n{}",
        out.words
    );
    assert!(
        out.words.contains(OBSERVATORY_GOLDEN_IR_SHA256),
        "the WASM path's IR must hash to the HARNESS's pinned golden — one interchange, two \
         producers, byte-equal (D13). Got:\n{}",
        out.words
    );
    assert!(out.words.contains("bit-exact"), "{}", out.words);
}

#[test]
fn two_independent_loads_render_bit_identically() {
    // The reload determinism half of §7.4: an unchanged package loads to unchanged renders.
    let pkg = sparq_host_wasm::package::open(&observatory_pkg());
    let ctx = runtime_ctx(fixtures(), None, false);
    let a = golden_stage(&pkg, &ctx);
    let b = golden_stage(&pkg, &ctx);
    assert_eq!(a.words, b.words, "two runs of the same gate must report identically");
}

// ── stage 6 end-to-end (on a TEMP copy — the checked-in package stays clean) ───────────────────

#[test]
fn stage6_regenerates_the_preview_and_publishes_at_rest() {
    let dir = temp_dir("stage6");
    let pkg_dir = dir.join("observatory");
    copy_package(&observatory_pkg(), &pkg_dir);
    let atrest = dir.join("at-rest").join("dat_observatory.ir.json");
    let pkg = sparq_host_wasm::package::open(&pkg_dir);
    let ctx = runtime_ctx(fixtures(), Some(atrest.clone()), true);
    let out = visual_stage(&pkg, &ctx);
    assert_eq!(
        out.verdict,
        sparq_host_wasm::Verdict::Pass,
        "visual conformance failed:\n{}",
        out.words
    );
    assert!(pkg_dir.join("preview.svg").exists(), "preview.svg regenerated");
    assert!(atrest.exists(), "the at-rest IR published (the LATER door)");
    let preview = std::fs::read_to_string(pkg_dir.join("preview.svg")).unwrap();
    assert!(preview.contains("<svg"), "the preview is the painter's SVG");
    let ir = std::fs::read_to_string(&atrest).unwrap();
    assert!(ir.contains("\"surface\""), "the at-rest file is the interchange");
    let _ = std::fs::remove_dir_all(&dir);
}

// ── the doors in isolation (no component needed) ───────────────────────────────────────────────

#[test]
fn the_assets_door_is_scoped_and_honest() {
    use sparq_host_wasm::runtime::bindings::sparq::instrument::assets::{AssetError, Host};
    use sparq_host_wasm::runtime::doors::InstrumentHost;
    let (provider, now) = provider_now();
    let mut host = InstrumentHost::new(
        64,
        provider,
        sparq_host_wasm::sources::ManifestSources::default(),
        sparq_module_api::params::ParamSet::zeroed(),
        now,
        true,
    )
    .unwrap();
    // Undeclared → Undeclared (the entitlement check).
    assert!(matches!(host.open("blake3:deadbeef".into()), Err(AssetError::Undeclared)));
    // Declared → NotFound in v1 (the asset store is Phase 3 — honest absence, never empty bytes).
    host.declared_assets.push("blake3:cafe".into());
    assert!(matches!(host.open("blake3:cafe".into()), Err(AssetError::NotFound)));
    // Outside the control thread → Denied.
    host.phase = sparq_host_wasm::runtime::Phase::Draw;
    assert!(matches!(host.open("blake3:cafe".into()), Err(AssetError::Denied)));
}

#[test]
fn the_audio_instances_sources_door_answers_at_rest_in_words() {
    use sparq_host_wasm::runtime::bindings::sparq::instrument::sources::Host as SourcesHost;
    use sparq_host_wasm::runtime::doors::InstrumentHost;
    let (provider, now) = provider_now();
    let mut audio_host = InstrumentHost::new(
        64,
        Arc::clone(&provider) as Arc<dyn sparq_host_wasm::sources::StreamProvider + Send + Sync>,
        sparq_host_wasm::sources::ManifestSources::default(),
        sparq_module_api::params::ParamSet::zeroed(),
        now,
        false, // the audio instance: sources CLOSED (§2.2)
    )
    .unwrap();
    assert!(audio_host.snapshot("wall".into(), "cell-01".into()).is_none());
    assert!(
        audio_host.violations.iter().any(|v| v.contains("AUDIO instance")),
        "…and says so in words: {:?}",
        audio_host.violations
    );
    // The display instance with a real binding serves the fixture window.
    let manifest = std::fs::read_to_string(observatory_pkg().join("sparqmod.toml")).unwrap();
    let bindings = sparq_host_wasm::sources::ManifestSources::parse(&manifest).unwrap();
    let defaults =
        sparq_host_wasm::runtime::convert::params_defaults_from_manifest(&manifest).unwrap();
    let params = sparq_module_api::params::ParamSet::new(1, &defaults).unwrap();
    let mut display_host = InstrumentHost::new(64, provider, bindings, params, now, true).unwrap();
    let snap = display_host.snapshot("wall".into(), "cell-01".into());
    assert!(snap.is_some(), "cell 1's default stream (swpc.aurora) serves its fixture window");
    assert!(display_host.sources_served > 0);
}

#[test]
fn the_token_door_serves_the_checked_in_bundle_verbatim() {
    use sparq_host_wasm::runtime::bindings::sparq::instrument::tokens::Host as TokensHost;
    use sparq_host_wasm::runtime::doors::InstrumentHost;
    let (provider, now) = provider_now();
    let mut host = InstrumentHost::new(
        64,
        provider,
        sparq_host_wasm::sources::ManifestSources::default(),
        sparq_module_api::params::ParamSet::zeroed(),
        now,
        true,
    )
    .unwrap();
    let bundle = host.get_bundle();
    let tokens_json = std::fs::read(root().join("design/tokens/generated/tokens.json")).unwrap();
    assert_eq!(bundle.payload, tokens_json, "the payload is tokens.json, verbatim");
    assert_eq!(
        (bundle.version.major, bundle.version.minor),
        (0, 1),
        "the envelope's version (the token bundle's own semver)"
    );
    let v = TokensHost::version(&mut host);
    assert_eq!(
        (v.major, v.minor, v.patch),
        (bundle.version.major, bundle.version.minor, bundle.version.patch)
    );
}

#[test]
fn the_seed_door_derives_from_the_kernel_seed_tree() {
    use sparq_host_wasm::runtime::bindings::sparq::instrument::host::Host as HostIface;
    use sparq_host_wasm::runtime::doors::InstrumentHost;
    let (provider, now) = provider_now();
    let mut host = InstrumentHost::new(
        64,
        provider,
        sparq_host_wasm::sources::ManifestSources::default(),
        sparq_module_api::params::ParamSet::zeroed(),
        now,
        true,
    )
    .unwrap();
    host.seed_root = 7;
    host.node_path = vec!["canvas".into(), "node-3".into()];
    let a = host.random_seed("gate".into());
    let b = host.random_seed("gate".into());
    let c = host.random_seed("timbre".into());
    assert_eq!(a, b, "the same stream name derives the same seed (replay-safe)");
    assert_ne!(a, c, "distinct streams, distinct seeds (the seed tree's promise)");
    assert_eq!(
        a,
        sparq_kernel::seed::derive_seed(7, &["canvas", "node-3", "gate"]),
        "the door IS the kernel's derivation — one copy"
    );
}

// ── the full gated chain, end-to-end (§7.6's bar) ──────────────────────────────────────────────

#[test]
fn the_full_chain_passes_on_the_noop_fixture() {
    let dir = temp_dir("chain");
    let pkg_dir = dir.join("noop");
    copy_package(&noop_pkg(), &pkg_dir);
    let report =
        sparq_host_wasm::validate::run_gated(&pkg_dir, Some(&runtime_ctx(fixtures(), None, true)));
    let rendered = report.render();
    assert!(report.complete(), "no stage refuses with a runtime context:\n{rendered}");
    assert!(report.passed(), "every stage passes:\n{rendered}");
    assert!(pkg_dir.join("preview.svg").exists(), "stage 6 generated the preview");
    let _ = std::fs::remove_dir_all(&dir);
}
