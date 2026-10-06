//! WO-020 INC2 — the stream-binding + `visible_if` validation the contract learns (plan §7.3, D4,
//! D7). These tests drive `sparq_host_wasm::validate` directly (the library door `sparq mod
//! validate` and the browser's launch path both call), over a REPRESENTATIVE Observatory manifest
//! draft: the §10 shape (a port-less `dat` instrument, per-cell `enum` params whose options are
//! registry stream ids, `ui.displays[].sources[].stream = "param:cell_NN"` indirections plus a fixed
//! registry binding, and panel widgets gated by `visible_if`) with a handful of cells instead of the
//! full 16 the INC3 generator emits.
//!
//! The acceptance these prove: the legal draft passes the static stages with its real bindings, and
//! each hostile variant fails ONE rule in words with the right code — `E-STREAM-UNKNOWN` for an
//! unregistered id (bare or inside a `param:` enum's options), `E-CROSS-FIELD` for a `param:` that
//! misses / is not an enum / has no options, for `visible_if` on an undeclared param, and for a
//! binding that declares both `port` and `stream`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use sparq_host_wasm::package;
use sparq_host_wasm::validate::{self, Stage, Verdict};
use sparq_module_api::error::{CodeKind, ValidationReport};

/// The representative Observatory manifest draft (valid units + real registry stream ids throughout;
/// the §10 draft's `unit = "min"` and `default = true` are NOT valid contract values and are flagged
/// for the INC3 generator — see WO020-STATE.md). Port-less on purpose (plan D5): a display-only
/// instrument declares no ports, and the static gate must accept that.
const OBSERVATORY: &str = r#"[identity]
id = "dat/observatory"
version = "0.1.0"
host_api = { min = 1, max = 1 }
display_name = "The Observatory"
summary = "A wall of live Earth-and-space data (WO-020 INC2 representative draft)"
authors = ["sparq first-party (WO-020)"]
license = "MIT"

[classification]
category = "data/observatory"
top = "dat"
kind = "display"
tier = "t2"
layer = "instrument"
stability = "experimental"

[[params]]
id = "cell_01"
name = "Cell 01 stream"
type = "enum"
default = 1
options = [ { value = "", label = "OFF" }, { value = "swpc.aurora", label = "Aurora" }, { value = "geo.quakes-hour", label = "Quakes hour" } ]

[[params]]
id = "cell_02"
name = "Cell 02 stream"
type = "enum"
default = 1
options = [ { value = "", label = "OFF" }, { value = "swpc.kp", label = "Kp" }, { value = "swpc.bz", label = "Bz" } ]

[[params]]
id = "selected_cell"
name = "Selected cell"
type = "enum"
default = 0
options = [ { value = "1", label = "Cell 01" }, { value = "2", label = "Cell 02" } ]

[[params]]
id = "ticker"
name = "Global ticker"
type = "bool"
default = 1.0

[[params]]
id = "intensity"
name = "Colormap intensity"
type = "float"
unit = "ratio"
min = 0.5
max = 2.0
default = 1.0

[state]
schema_id = "dat/observatory/state"
schema_version = 1

[resources]
latency = 0
cpu_class = "light"
gpu_class = "medium"

[capabilities]
fs_read = []
network = "none"
max_fuel = 2000000
max_memory_mb = 64

[ui]
colour_class = "dat"

[[ui.displays]]
id = "wall"
kind = "display_list"
min_size = [2176, 1120]
lod = "auto"
colormap = "colormap.thermal"

[[ui.displays.sources]]
id = "cell-01"
stream = "param:cell_01"

[[ui.displays.sources]]
id = "cell-02"
stream = "param:cell_02"

[[ui.displays.sources]]
id = "fixed-wwv"
stream = "swpc.wwv"

[[ui.panel.widgets]]
kind = "enum_select"
param = "cell_01"
visible_if = { param = "selected_cell", equals = 1 }

[[ui.panel.widgets]]
kind = "enum_select"
param = "cell_02"
visible_if = { param = "selected_cell", equals = 2 }

[distribution]
package_format = "wasm"
entrypoint = "observatory.wasm"
platforms = ["any"]
arch = ["wasm32"]
min_host_version = "0.0.0-phase0"
channel = "beta"
"#;

static SEQ: AtomicUsize = AtomicUsize::new(0);

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sparq-streams-{}-{tag}-{}",
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

/// Runs stage 1 (schema) over a manifest-only directory — the stage where stream resolution lives.
fn schema_report(manifest: &str) -> ValidationReport {
    let dir = scratch("schema");
    put(&dir, "sparqmod.toml", manifest.as_bytes());
    let pkg = package::open(&dir);
    let report = validate::schema_check(&pkg);
    let _ = std::fs::remove_dir_all(&dir);
    report
}

/// Builds the full five-file package so stages 1 AND 2 both run.
fn build_package(dir: &Path, manifest: &str) {
    put(dir, "sparqmod.toml", manifest.as_bytes());
    put(
        dir,
        "observatory.wasm",
        b"\0asm\x01\0\0\0 (stand-in bytes; stage 3 is the runtime half's)",
    );
    put(dir, "example.sparqpatch", b"[meta]\nformat = \"sparqpatch\"\nversion = 0\n");
    put(dir, "preview.svg", b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>");
    put(dir, "README.md", b"# The Observatory\n\nWO-020 stream-binding test package.\n");
}

#[test]
fn the_observatory_draft_passes_the_static_stages_with_real_bindings() {
    // Stage 1 (schema) resolves every stream binding against the checked-in registry and every
    // visible_if against the declared params — and the port-less draft (D5) is accepted.
    let r = schema_report(OBSERVATORY);
    assert!(r.is_empty(), "the legal Observatory draft must pass stage 1:\n{r}");

    // Stages 1 AND 2 over the full five-file package: both PASS; the runtime stages refuse in words
    // (PARTIAL), exactly as WO-018 established — the stream rules are static and ran.
    let dir = scratch("legal");
    build_package(&dir, OBSERVATORY);
    let gate = validate::run(&dir);
    let schema = gate.outcomes.iter().find(|o| o.stage == Stage::Schema).unwrap();
    let package_out = gate.outcomes.iter().find(|o| o.stage == Stage::Package).unwrap();
    assert_eq!(schema.verdict, Verdict::Pass, "stage 1:\n{}", gate.render());
    assert_eq!(package_out.verdict, Verdict::Pass, "stage 2:\n{}", gate.render());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_fixed_stream_id_is_refused_with_e_stream_unknown() {
    let bad = OBSERVATORY.replace("stream = \"swpc.wwv\"", "stream = \"nope.not-a-stream\"");
    let r = schema_report(&bad);
    assert!(r.has(CodeKind::StreamUnknown), "an unregistered id must raise E-STREAM-UNKNOWN:\n{r}");
    assert!(r.has_path("ui.displays[0].sources[2].stream"), "the failure names the binding: {r}");
    let e = r.first_at("ui.displays[0].sources[2].stream").unwrap();
    assert_eq!(e.code(), "E-STREAM-UNKNOWN");
    let s = e.to_string();
    assert!(
        s.contains("nope.not-a-stream") && s.contains("swpc.aurora"),
        "names the value + allowed ids: {s}"
    );
}

#[test]
fn a_param_binding_to_an_undeclared_param_is_refused() {
    let bad = OBSERVATORY.replace("stream = \"param:cell_01\"", "stream = \"param:cell_99\"");
    let r = schema_report(&bad);
    assert!(r.has(CodeKind::CrossField), "a param: that misses must raise E-CROSS-FIELD:\n{r}");
    assert!(r.has_path("ui.displays[0].sources[0].stream"), "{r}");
}

#[test]
fn a_param_binding_to_a_non_enum_param_is_refused() {
    // `intensity` is a float param — a stream indirection must target an ENUM (its options are the
    // selectable streams).
    let bad = OBSERVATORY.replace("stream = \"param:cell_02\"", "stream = \"param:intensity\"");
    let r = schema_report(&bad);
    assert!(r.has(CodeKind::CrossField), "a param: to a non-enum must raise E-CROSS-FIELD:\n{r}");
    let e = r.first_at("ui.displays[0].sources[1].stream").unwrap();
    assert!(e.to_string().contains("ENUM"), "the fix says enum: {e}");
}

#[test]
fn a_param_enum_with_an_option_outside_the_registry_is_refused() {
    // cell_01's options must all be registry ids (or "" for OFF); poison one.
    let bad = OBSERVATORY.replace(
        "{ value = \"swpc.aurora\", label = \"Aurora\" }",
        "{ value = \"dead.feed\", label = \"Dead\" }",
    );
    let r = schema_report(&bad);
    assert!(
        r.has(CodeKind::StreamUnknown),
        "an option outside the registry must raise E-STREAM-UNKNOWN:\n{r}"
    );
    let e = r.first_at("ui.displays[0].sources[0].stream").unwrap();
    assert!(e.to_string().contains("dead.feed"), "names the offending option value: {e}");
}

#[test]
fn the_empty_off_option_is_legal_in_a_stream_enum() {
    // A cell param whose ONLY option is "" (OFF) is legal — the empty value is the OFF sentinel.
    let off_only = OBSERVATORY.replace(
        "options = [ { value = \"\", label = \"OFF\" }, { value = \"swpc.aurora\", label = \"Aurora\" }, { value = \"geo.quakes-hour\", label = \"Quakes hour\" } ]",
        "options = [ { value = \"\", label = \"OFF\" } ]",
    );
    let r = schema_report(&off_only);
    assert!(r.is_empty(), "an OFF-only stream enum is legal:\n{r}");
}

#[test]
fn visible_if_on_an_undeclared_param_is_refused() {
    let bad = OBSERVATORY.replace(
        "visible_if = { param = \"selected_cell\", equals = 1 }",
        "visible_if = { param = \"no_such_param\", equals = 1 }",
    );
    let r = schema_report(&bad);
    assert!(
        r.has(CodeKind::CrossField),
        "visible_if on an undeclared param must raise E-CROSS-FIELD:\n{r}"
    );
    assert!(r.has_path("ui.panel.widgets[0].visible_if.param"), "{r}");
}

#[test]
fn a_binding_with_both_port_and_stream_is_refused() {
    // The decoder enforces the mutual exclusivity; it surfaces in stage 1 (which runs the decode).
    let bad = OBSERVATORY.replace(
        "[[ui.displays.sources]]\nid = \"fixed-wwv\"\nstream = \"swpc.wwv\"",
        "[[ui.displays.sources]]\nid = \"fixed-wwv\"\nport = \"out\"\nstream = \"swpc.wwv\"",
    );
    let r = schema_report(&bad);
    assert!(r.has(CodeKind::CrossField), "port+stream together must raise E-CROSS-FIELD:\n{r}");
}

#[test]
fn every_registry_id_is_accepted_as_a_fixed_binding() {
    // The other half of the registry<->contract drift gate: binding EACH of the 23 registry ids as a
    // fixed stream must pass stage 1. If a registry id were rejected here, the contract and the
    // registry would have drifted.
    let reg = sparq_streams::Registry::load().unwrap();
    for def in reg.streams() {
        let m = OBSERVATORY.replace("stream = \"swpc.wwv\"", &format!("stream = \"{}\"", def.id));
        let r = schema_report(&m);
        assert!(
            !r.has(CodeKind::StreamUnknown),
            "registry id `{}` was refused as a stream binding — contract/registry drift:\n{r}",
            def.id
        );
    }
}
