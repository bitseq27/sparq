//! The throwaway-PACKAGE test — WO-018's acceptance criterion 1, made mechanical on the
//! WO-007 pattern (`sparq-module-api/tests/throwaway_module.rs`, extended).
//!
//! A five-file instrument package that appears nowhere else in the repository is built in a
//! scratch directory, and the door is run over it: the legal package passes the static gate (and
//! the gate says PARTIAL in words, because the runtime stages refuse rather than pretend), while
//! every acceptance breakage — the sixth file, the bad manifest, the backbone module, the data
//! port, the unhonourable capability, the unbudgeted scene, the hostile `host_api` pin, the empty
//! component — never loads, each with the failure named verbatim. Then
//! [`nothing_in_the_engine_names_the_probe_package`] walks every crate's `src/` and asserts that
//! not one file mentions the probe: adding an instrument requires ZERO engine edits, and that
//! claim is a gate, not an intention.
//!
//! The gate is CLI-independent on purpose: this file drives the library doors directly
//! (`sparq_host_wasm::{package, validate}`), because `sparq mod validate` is one caller of them
//! and the browser's launch path is another — both must get the same words.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use sparq_host_wasm::package::{self, Profile};
use sparq_host_wasm::validate::{self, Stage, Verdict};
use sparq_module_api::error::CodeKind;

/// The probe's id. Nothing under any `crates/*/src` may contain it (or the wasm stem, or the
/// display name) — the walk at the bottom fails the build if adding this package ever required
/// an engine edit.
const PROBE_ID: &str = "probe/syn/five-file";

/// The probe's manifest — a legal instrument manifest exactly as an author would write it
/// (the template's shape; the frozen vocabularies throughout).
const PROBE_MANIFEST: &str = r#"[identity]
id = "probe/syn/five-file"
version = "0.1.0"
host_api = { min = 1, max = 1 }
display_name = "Five-file probe"
summary = "Added by a CI test to prove the instrument door needs no engine edits"
authors = ["the package-gate test"]
license = "MIT"

[classification]
category = "synth/probe"
top = "syn"
kind = "source"
tier = "t2"
layer = "instrument"
stability = "experimental"

[[ports]]
id = "out"
name = "Output"
direction = "out"
type = "audio"
channel_set = "mono"

[[ports]]
id = "gate"
name = "Gate"
direction = "in"
type = "event"
event_kinds = ["trigger", "gate"]

[[params]]
id = "freq"
name = "Frequency"
type = "float"
unit = "Hz"
min = 10.0
max = 10000.0
default = 440.0

[state]
schema_id = "probe/syn/five-file/state"
schema_version = 1

[resources]
latency = 0
cpu_class = "trivial"
gpu_class = "light"

[ui]
colour_class = "syn"

[[ui.displays]]
id = "view"
kind = "display_list"
min_size = [160, 96]
lod = "auto"

[capabilities]
fs_read = []
network = "none"
max_fuel = 1000000
max_memory_mb = 32

[distribution]
package_format = "wasm"
entrypoint = "five_file_probe.wasm"
platforms = ["any"]
arch = ["wasm32"]
min_host_version = "0.0.0-phase0"
channel = "beta"
"#;

/// The probe's component stand-in. The static gate checks presence and non-emptiness; whether
/// these bytes instantiate is stage 3's, and stage 3 refuses in words until the runtime lands.
const PROBE_WASM: &[u8] = b"\0asm\x01\0\0\0 (stand-in bytes; stage 3 is the runtime half's)";

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sparq-gate-{}-{name}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn put(dir: &Path, name: &str, bytes: &[u8]) {
    let mut f = std::fs::File::create(dir.join(name)).unwrap();
    f.write_all(bytes).unwrap();
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Builds the legal five-file package, with the manifest text and wasm bytes substitutable for
/// the breakage cases.
fn build(dir: &Path, manifest: &str, wasm: &[u8]) {
    put(dir, "sparqmod.toml", manifest.as_bytes());
    put(dir, "five_file_probe.wasm", wasm);
    put(dir, "example.sparqpatch", b"[meta]\nformat = \"sparqpatch\"\nversion = 0\n");
    put(dir, "preview.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    put(dir, "README.md", b"# Five-file probe\n\nThe package-gate test's throwaway instrument.\n");
}

fn outcome(g: &validate::GateReport, stage: Stage) -> &validate::StageOutcome {
    g.outcomes.iter().find(|o| o.stage == stage).unwrap()
}

#[test]
fn a_legal_five_file_package_passes_the_static_gate() {
    let dir = scratch("legal");
    build(&dir, PROBE_MANIFEST, PROBE_WASM);

    let gate = validate::run(&dir);
    assert!(gate.passed(), "the legal package must pass every stage that ran:\n{}", gate.render());
    assert!(!gate.complete(), "the static half must NOT claim to be the whole gate");

    // The stages that ran, passed; the stages that need a runtime, refused — and the refusal
    // list is exactly the runtime half's (smoke, golden render, budgets' measurement, visual
    // conformance). Stage 5 counts as refused: its static half ran, its measurement did not.
    assert_eq!(outcome(&gate, Stage::Schema).verdict, Verdict::Pass);
    assert_eq!(outcome(&gate, Stage::Package).verdict, Verdict::Pass);
    assert_eq!(outcome(&gate, Stage::SignatureBadge).verdict, Verdict::Pass);
    assert_eq!(
        gate.refused_stages(),
        vec![Stage::Smoke, Stage::GoldenRender, Stage::Budgets, Stage::VisualConformance]
    );

    // The report says PARTIAL in words — a partial gate never reads as a full one.
    let rendered = gate.render();
    assert!(rendered.contains("GATE: PARTIAL"), "{rendered}");
    assert!(rendered.contains("[REFUSED] 3 smoke"), "{rendered}");
    assert!(rendered.contains("runtime"), "the refusals name what would make them run: {rendered}");
    // Unsigned loads badged, in words (plan §15).
    assert!(outcome(&gate, Stage::SignatureBadge).words.contains("BADGED"), "{rendered}");
    // The declared budgets were statically checked before the measurement refused.
    assert!(outcome(&gate, Stage::Budgets).words.contains("1000000"), "{rendered}");

    // A signed variant: the badge line changes, the verdict does not — and the stage says
    // outright that the VALUE's verification is the runtime keyring's.
    let signed = PROBE_MANIFEST.replace(
        "[classification]",
        "[signature]\nkey_id = \"probe-key-1\"\nalgorithm = \"ed25519\"\nvalue = \"c3RhbmQtaW4=\"\nsigned_at = \"2026-10-05T00:00:00Z\"\n\n[classification]",
    );
    let dir2 = scratch("legal-signed");
    build(&dir2, &signed, PROBE_WASM);
    let gate2 = validate::run(&dir2);
    assert!(gate2.passed(), "{}", gate2.render());
    let sig = outcome(&gate2, Stage::SignatureBadge);
    assert_eq!(sig.verdict, Verdict::Pass);
    assert!(sig.words.contains("probe-key-1") && sig.words.contains("keyring"), "{sig:#?}");
    cleanup(&dir);
    cleanup(&dir2);
}

#[test]
fn the_acceptance_breakages_never_load() {
    // Every breakage the acceptance criterion names (WO-018 ticket + WO018-STATE's list), each
    // refused with its code and a fix that names the door that CAN serve. Case shape:
    // (name, manifest, wasm bytes, sixth file?, failing stage, expected code kind, expected
    // failure path ("" = any), words the rendered report must contain).
    // Manifest variants — built once, referenced by the cases.
    let backbone = PROBE_MANIFEST
        .replace("layer = \"instrument\"", "layer = \"backbone\"")
        .replace("tier = \"t2\"", "tier = \"t1\"");
    let data_port = PROBE_MANIFEST.replace(
        "[[params]]",
        "[[ports]]\nid = \"imu\"\nname = \"IMU\"\ndirection = \"in\"\ntype = \"data\"\nrequired = false\n\n[[params]]",
    );
    let networked = PROBE_MANIFEST.replace("network = \"none\"", "network = \"internet\"");
    let unbudgeted_scene = PROBE_MANIFEST
        .replace("gpu_class = \"light\"", "gpu_class = \"none\"")
        .replace("kind = \"display_list\"", "kind = \"scene\"");
    let hostile_api = PROBE_MANIFEST
        .replace("host_api = { min = 1, max = 1 }", "host_api = { min = 99, max = 99 }");

    // type_complexity allowed ON PURPOSE (the api_snapshot.rs precedent): the table's columns
    // are documented at its head, and factoring it into a named struct would only hide the
    // shape this test exists to show.
    #[allow(clippy::type_complexity)]
    let cases: Vec<(&str, &str, &[u8], bool, Stage, CodeKind, &str, &str)> = vec![
        (
            "the sixth file",
            PROBE_MANIFEST,
            PROBE_WASM,
            true,
            Stage::Package,
            CodeKind::PackageFilecount,
            "package",
            "E-PACKAGE-FILECOUNT",
        ),
        (
            "the bad manifest",
            "[identity\nid = [[[ not toml",
            PROBE_WASM,
            false,
            Stage::Schema,
            CodeKind::ValueMalformed,
            "",
            "schema failure(s)",
        ),
        (
            "the backbone module in instruments/",
            &backbone,
            PROBE_WASM,
            false,
            Stage::Schema,
            CodeKind::CrossField,
            "classification.layer",
            "instruments/ carries",
        ),
        (
            "the data port",
            &data_port,
            PROBE_WASM,
            false,
            Stage::Schema,
            CodeKind::CrossField,
            "ports[2].type",
            "does not route data ports",
        ),
        (
            "the unhonourable network capability",
            &networked,
            PROBE_WASM,
            false,
            Stage::Schema,
            CodeKind::CrossField,
            "capabilities.network",
            "T3",
        ),
        (
            "the scene without a budget",
            &unbudgeted_scene,
            PROBE_WASM,
            false,
            Stage::Schema,
            CodeKind::CrossField,
            "ui.displays[0].kind",
            "ceiling",
        ),
        (
            "the hostile host_api pin",
            &hostile_api,
            PROBE_WASM,
            false,
            Stage::Schema,
            CodeKind::HostApiIncompatible,
            "identity.host_api",
            "host speaks 1",
        ),
        (
            "the empty component",
            PROBE_MANIFEST,
            b"",
            false,
            Stage::Package,
            CodeKind::ValueMalformed,
            "package/five_file_probe.wasm",
            "EMPTY",
        ),
    ];
    for (name, manifest, wasm, extra, stage, kind, path, words) in cases {
        let dir = scratch("breakage");
        build(&dir, manifest, wasm);
        if extra {
            put(&dir, "notes.txt", b"one file too many");
        }
        let gate = validate::run(&dir);
        assert!(!gate.passed(), "{name}: the breakage loaded — gate said:\n{}", gate.render());
        let o = outcome(&gate, stage);
        assert_eq!(o.verdict, Verdict::Fail, "{name}: wrong stage failed:\n{}", gate.render());
        assert!(o.report.has(kind), "{name}: expected {kind:?} in:\n{}", o.report);
        if !path.is_empty() {
            assert!(o.report.has_path(path), "{name}: expected a failure at {path}:\n{}", o.report);
        }
        let rendered = gate.render();
        assert!(rendered.contains(words), "{name}: the report should say `{words}`:\n{rendered}");
        // And at the launch door, independently: a breakage that fails the gate must also fail
        // `is_loadable` where the profiles agree (cap/manifest/component) — the schema half is
        // the gate's and the browser's shared stage 1, so the gate check above covers it.
        cleanup(&dir);
    }
}

#[test]
fn one_broken_package_never_hides_the_others() {
    // Discovery independence (module-api §11's rule, at the instruments/ slot): the broken
    // package is REPORTED — with the directory that produced it — and the good one still loads.
    let root = scratch("discovery");
    let good = root.join("a-good");
    let broken = root.join("b-broken");
    std::fs::create_dir_all(&good).unwrap();
    std::fs::create_dir_all(&broken).unwrap();
    build(&good, PROBE_MANIFEST, PROBE_WASM);
    put(&broken, "sparqmod.toml", b"[identity\nid = [[[ not toml");
    // A third "package" that is an empty directory: also broken, also named, also not fatal.
    std::fs::create_dir_all(root.join("c-empty")).unwrap();

    let found = package::discover(&root);
    assert_eq!(found.len(), 3, "every directory is inventoried, whatever its state");

    let good_pkg = found.iter().find(|p| p.dir == good).unwrap();
    assert!(
        package::is_loadable(good_pkg).is_ok(),
        "the good package loads beside the broken ones"
    );
    assert!(validate::run(&good).passed());

    let broken_pkg = found.iter().find(|p| p.dir == broken).unwrap();
    let err = package::is_loadable(broken_pkg).unwrap_err();
    assert!(!err.is_empty(), "the broken manifest is refused");
    let empty_pkg = found.iter().find(|p| p.dir == root.join("c-empty")).unwrap();
    let err = package::is_loadable(empty_pkg).unwrap_err();
    assert!(err.has_path("sparqmod.toml"), "the empty directory names what it lacks: {err}");
    assert!(err.has_path("*.wasm"), "{err}");
    cleanup(&root);
}

#[test]
fn the_launch_door_and_the_hand_in_door_disagree_only_about_words() {
    // Launch profile: manifest + component + cap are the fatal set; README/example are words.
    // Hand-in (the gate's stage 2 is strict): the package must be complete. preview.svg is
    // required by NEITHER — it is stage 6's output.
    let dir = scratch("profiles");
    put(&dir, "sparqmod.toml", PROBE_MANIFEST.as_bytes());
    put(&dir, "five_file_probe.wasm", PROBE_WASM);
    let pkg = package::open(&dir);

    assert!(package::is_loadable(&pkg).is_ok(), "manifest + component load at launch");
    let words = package::advisories(&pkg);
    assert!(words.iter().any(|w| w.contains("README.md")), "{words:?}");
    assert!(words.iter().any(|w| w.contains("example.sparqpatch")), "{words:?}");

    let gate = validate::run(&dir);
    assert!(!gate.passed(), "the hand-in door wants the complete package:\n{}", gate.render());
    let o = outcome(&gate, Stage::Package);
    assert_eq!(o.verdict, Verdict::Fail);
    assert!(
        o.report.has_path("README.md") && o.report.has_path("example.sparqpatch"),
        "{}",
        o.report
    );
    assert!(outcome(&gate, Stage::Schema).verdict == Verdict::Pass, "the manifest itself is legal");

    // Complete the package: the hand-in door's stage 2 passes, and the preview's absence was
    // never an ERROR — the honest advisory words may name it (stage 6 would generate it), but
    // no failure line may.
    put(&dir, "README.md", b"# probe\n");
    put(&dir, "example.sparqpatch", b"[meta]\nformat = \"sparqpatch\"\n");
    let gate = validate::run(&dir);
    assert!(gate.passed(), "{}", gate.render());
    let o = outcome(&gate, Stage::Package);
    assert_eq!(o.verdict, Verdict::Pass);
    for stage_outcome in &gate.outcomes {
        assert!(
            !stage_outcome.report.has_path("preview.svg")
                && !stage_outcome.report.iter().any(|e| e.path.contains("preview.svg")),
            "the preview is stage 6's OUTPUT — it must never be a required input:\n{}",
            gate.render()
        );
    }
    cleanup(&dir);

    // The strict profile's cap is the launch profile's cap: the sixth file is fatal at BOTH.
    let dir = scratch("cap-both");
    build(&dir, PROBE_MANIFEST, PROBE_WASM);
    put(&dir, "bonus.wav", b"RIFF....");
    let pkg = package::open(&dir);
    assert!(package::is_loadable(&pkg).is_err(), "the cap is fatal at launch");
    assert!(package::check(&pkg, Profile::Strict).has(CodeKind::PackageFilecount));
    cleanup(&dir);
}

#[test]
fn nothing_in_the_engine_names_the_probe_package() {
    // The WO-007 walk, extended to packages: not one engine source file may mention the probe,
    // its component or its display name. If a future change makes dropping in an instrument
    // require a line in some registry or match arm, this test names the file.
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut sources: Vec<PathBuf> = Vec::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&crates).unwrap().flatten() {
        let src = entry.path().join("src");
        if src.is_dir() {
            stack.push(src);
        }
    }
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                sources.push(path);
            }
        }
    }
    sources.sort();
    assert!(
        sources.len() > 40,
        "the walk found only {} files — it is not walking what it thinks",
        sources.len()
    );

    let mut offenders = Vec::new();
    for path in &sources {
        let text = std::fs::read_to_string(path).unwrap();
        for needle in [PROBE_ID, "five_file_probe", "Five-file probe"] {
            if text.contains(needle) {
                offenders.push(format!("{} (`{needle}`)", path.display()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "adding this instrument package required editing engine code, which is the one thing \
         the hand-off system forbids (ADR-010 decision 7): {}",
        offenders.join(", ")
    );
}
