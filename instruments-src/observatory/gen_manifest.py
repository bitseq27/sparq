#!/usr/bin/env python3
"""
instruments-src/observatory/gen_manifest.py — the Observatory's generator (WO-020 INC3).

The manifest's 16 × 24-option enum walls are GENERATED from the registry
(`crates/sparq-streams/streams.toml` — the single source of truth for stream ids, plan D4/§10),
never hand-typed: hand-typing 384 option rows is a drift bug waiting for its symptom. The same
run also emits the guest-side mirror of the registry metadata
(`core/src/streams_table.rs` — labels, views, units, cadences, schemas — the cell chrome and
the renderer dispatch read it, because the guest has no filesystem and no network: the
registry reaches it as compiled-in generated data).

Two §10 draft gaps this generator closes (found by INC2's representative manifest, recorded in
WO020-STATE.md — the draft of record is a SHAPE, the contract is the law):

  * `unit = "min"` is NOT in the closed UNITS vocabulary (`sparq_module_api::manifest::UNITS`:
    Hz s ms samples dB % st cents ratio ch LUFS deg m bpm ticks x). The HISTORY span is
    therefore declared in SECONDS: min 600 (10 min), max 604800 (7 d), default 10800 (3 h),
    curve exp — the same span, in a legal unit.
  * bool `default = true` is not a float: the decoder wants `1.0`/`0.0` (the snapshot's
    bool-as-f32 rule, types.wit `param-set`).

Everything else follows the §10 draft of record exactly: 26 params (16 cells + selected_cell +
layout + 5 toggles + 3 floats — inside the 32 cap), port-less (D5), one `display_list` display
with 16 `param:` stream bindings (D4), the §5.2 toolbar as generated panel widgets (8 px grid,
touch_class M, the sixteen STREAM pickers gated by `visible_if` on `selected_cell` — one
visible at a time, reading as a single dropdown, D7).

Usage:
    python3 instruments-src/observatory/gen_manifest.py              # write both artefacts
    python3 instruments-src/observatory/gen_manifest.py --check      # gate: fail if either is stale
    python3 instruments-src/observatory/gen_manifest.py --stdout     # print the manifest, write nothing

Determinism: pure function of streams.toml — fixed order (registry file order), no timestamps,
LF newlines. `--check` is the token_gen.py discipline: CI fails if a checked-in artefact moved
apart from its generator.
"""
from __future__ import annotations

import argparse
import pathlib
import sys
import tomllib

# Windows consoles and redirected logs default to the ANSI code page (defect #37); every tool
# reconfigures its streams (tools/README.md -> 'Text I/O portability'; check_text_io.py gate).
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError):
        pass

ENCODING = "utf-8"
HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
REGISTRY = ROOT / "crates" / "sparq-streams" / "streams.toml"
DEFAULT_MANIFEST_OUT = HERE / "package" / "sparqmod.toml"
DEFAULT_RUST_OUT = HERE / "core" / "src" / "streams_table.rs"

# The §5.4 default state: cells 1..16 fill the domains (aurora map-heat, quakes map-points,
# Bz timeseries, WWV text, ISS map, EONET map-points, weather timeseries, Kp bars, flares
# timeline, NEO timeline, tides timeseries, alerts map-points, solar wind timeseries, GDACS
# map-points, air gauge, POWER daily timeseries). Data, not code-path folklore.
DEFAULT_CELL_STREAMS = [
    "swpc.aurora",
    "geo.quakes-hour",
    "swpc.bz",
    "swpc.wwv",
    "sat.iss",
    "geo.eonet",
    "wx.openmeteo",
    "swpc.kp",
    "swpc.xray-flares",
    "space.neo",
    "wx.tides",
    "wx.alerts",
    "swpc.solar-wind",
    "geo.gdacs",
    "wx.air",
    "space.power",
]

LAYOUTS = [
    ("2x2", "2 × 2 (4)"),
    ("4x2", "4 × 2 (8)"),
    ("3x3", "3 × 3 (9)"),
    ("4x3", "4 × 3 (12)"),
    ("4x4", "4 × 4 (16)"),
]
LAYOUT_DEFAULT = 4  # 4x4 — the full wall ships as the default face (§5.4)

OFF_LABEL = "— OFF —"


def read_text(path) -> str:
    return pathlib.Path(path).read_text(encoding=ENCODING)


def write_text(path, content: str) -> None:
    pp = pathlib.Path(path)
    pp.parent.mkdir(parents=True, exist_ok=True)
    with pp.open("w", encoding=ENCODING, newline="\n") as fh:
        fh.write(content)


def load_streams() -> list[dict]:
    with REGISTRY.open("rb") as fh:
        data = tomllib.load(fh)
    rows = data.get("stream", [])
    if not rows:
        raise SystemExit(f"{REGISTRY}: no [[stream]] rows — refusing to generate from an empty registry")
    ids = [r["id"] for r in rows]
    if len(set(ids)) != len(ids):
        raise SystemExit(f"{REGISTRY}: duplicate stream ids — the registry must be unambiguous")
    missing = [s for s in DEFAULT_CELL_STREAMS if s not in ids]
    if missing:
        raise SystemExit(
            f"the §5.4 default state names stream(s) absent from the registry: {missing} — "
            "fix the defaults or the registry; never generate a manifest whose defaults do not resolve"
        )
    return rows


def esc(s: str) -> str:
    """TOML basic-string escaping (the registry's labels carry em-dashes and middle dots —
    legal UTF-8, no escaping needed; quotes/backslashes would be)."""
    return s.replace("\\", "\\\\").replace('"', '\\"')


def option_rows(rows: list[dict]) -> str:
    """The 24-option wall: OFF + the registry in file order (option index = 1 + position —
    the same index the ParamSet carries and `visible_if.equals` compares)."""
    opts = ['  { value = "", label = "%s" }' % OFF_LABEL]
    for r in rows:
        opts.append('  { value = "%s", label = "%s" }' % (esc(r["id"]), esc(r["label"])))
    # One option per line, commas between (none after the last — the checked-in file reads
    # cleaner than a 400-char inline array, and the house parser spans lines fine).
    return "\n".join(f"{o}," if i < len(opts) - 1 else o for i, o in enumerate(opts))


def gen_manifest(rows: list[dict]) -> str:
    ids = [r["id"] for r in rows]
    n = len(rows)
    out: list[str] = []
    w = out.append
    w("# GENERATED by instruments-src/observatory/gen_manifest.py from")
    w("# crates/sparq-streams/streams.toml — DO NOT EDIT. Regenerate:")
    w("#   python3 instruments-src/observatory/gen_manifest.py")
    w("# The 16 cell enums carry the OFF sentinel + every registry id (file order); the")
    w("# `param:` source bindings, the toolbar widgets and the defaults are all generated")
    w("# (plan §10, D4, D7). Drift gate: `--check`, and the host validator resolves every")
    w("# stream id against the registry at stage 1 (E-STREAM-UNKNOWN).")
    w("")
    w("[identity]")
    w('id = "dat/observatory"')
    w('version = "0.1.0"')
    w("host_api = { min = 1, max = 1 }")
    w('display_name = "The Observatory"')
    w('summary = "A wall of live Earth-and-space data: 4-16 streams, maps, charts, heat grids, raw-text tickers"')
    w('authors = ["sparq first-party (WO-020)"]')
    w('license = "MIT"')
    w('tags = ["instrument", "data", "live", "nasa", "usgs", "noaa", "wall"]')
    w("")
    w("[classification]")
    w('category = "data/observatory"')
    w('top = "dat"')
    w('kind = "display"')
    w('tier = "t2"')
    w('layer = "instrument"')
    w('stability = "experimental"')
    w("")
    w("# ports: NONE in v1 (plan D5) — display-only until the native data-port arbiter lands")
    w("# (Phase 5). The port-less node proof is a test, not a grep: see the D5 integration")
    w("# tests in the root workspace (crates/sparq-audio/tests/, crates/sparq-app/tests/).")
    w("")
    # The 16 cell params.
    for i, default_id in enumerate(DEFAULT_CELL_STREAMS, start=1):
        default_idx = 1 + ids.index(default_id)
        w(f"[[params]]  # cell {i} of 16 — enum over OFF + the {n} registry ids")
        w(f'id = "cell_{i:02d}"')
        w(f'name = "Cell {i:02d} stream"')
        w('type = "enum"')
        w(f"default = {default_idx}  # {default_id} (§5.4 default state)")
        w("options = [")
        w(option_rows(rows))
        w("]")
        w("")
    w("[[params]]")
    w('id = "selected_cell"')
    w('name = "Selected cell"')
    w('type = "enum"')
    w("default = 0")
    w("options = [")
    for i in range(1, 17):
        comma = "," if i < 16 else ""
        w(f'  {{ value = "{i}", label = "Cell {i:02d}" }}{comma}')
    w("]")
    w("")
    w("[[params]]")
    w('id = "layout"')
    w('name = "Layout"')
    w('type = "enum"')
    w(f"default = {LAYOUT_DEFAULT}  # {LAYOUTS[LAYOUT_DEFAULT][0]} — the full wall ships as the default face")
    w("options = [")
    for j, (val, label) in enumerate(LAYOUTS):
        comma = "," if j < len(LAYOUTS) - 1 else ""
        w(f'  {{ value = "{val}", label = "{label}" }}{comma}')
    w("]")
    w("")
    for pid, name, default in [
        ("solo", "Solo selected cell", 0.0),
        ("full", "Full (solo + minimal chrome)", 0.0),
        ("ticker", "Global ticker", 1.0),
        ("grid", "Graticules & guides", 1.0),
        ("pause", "Pause motion", 0.0),
    ]:
        w("[[params]]")
        w(f'id = "{pid}"')
        w(f'name = "{name}"')
        w('type = "bool"')
        w(f"default = {default:.1f}  # bools ride 0.0/1.0 (types.wit param-set)")
        w("")
    w("[[params]]")
    w('id = "ticker_speed"')
    w('name = "Ticker speed"')
    w('type = "float"')
    w('unit = "ratio"')
    w("min = 0.2")
    w("max = 3.0")
    w("default = 1.0")
    w("")
    w("[[params]]")
    w('id = "history"')
    w('name = "History span"')
    w('type = "float"')
    w('unit = "s"  # the §10 draft said "min"; UNITS is closed and has no minutes — seconds')
    w("min = 600.0  # 10 min")
    w("max = 604800.0  # 7 d")
    w("default = 10800.0  # 3 h")
    w('curve = "exp"')
    w("")
    w("[[params]]")
    w('id = "intensity"')
    w('name = "Colormap intensity"')
    w('type = "float"')
    w('unit = "ratio"')
    w("min = 0.5")
    w("max = 2.0")
    w("default = 1.0")
    w("")
    w("[state]")
    w('schema_id = "dat/observatory/state"')
    w("schema_version = 1")
    w("")
    w("[ui]")
    w('colour_class = "dat"')
    w("")
    w("# The §5.2 toolbar, generated: row 0 = CELL / STREAM×16 (visible_if — one at a time,")
    w("# reading as a single dropdown editing the selected cell, D7) / LAYOUT / the five")
    w("# toggles / the host-side status label; row 1 = the three sliders. 8 px grid, all")
    w("# touch_class M (56 px floor ≥ the 44 px contract minimum).")
    widgets = []
    widgets.append(('enum_select', 'selected_cell', 0, 0, 2, 1, None))
    for i in range(1, 17):
        widgets.append(('enum_select', f'cell_{i:02d}', 2, 0, 4, 1, ('selected_cell', i - 1)))
    widgets.append(('enum_select', 'layout', 6, 0, 2, 1, None))
    for j, pid in enumerate(['solo', 'full', 'ticker', 'grid', 'pause']):
        widgets.append(('toggle', pid, 8 + j, 0, 1, 1, None))
    widgets.append(('label', None, 13, 0, 4, 1, None))
    widgets.append(('slider', 'history', 0, 1, 6, 1, None))
    widgets.append(('slider', 'ticker_speed', 6, 1, 5, 1, None))
    widgets.append(('slider', 'intensity', 11, 1, 5, 1, None))
    for kind, param, x, y, wd, h, vif in widgets:
        w("[[ui.panel.widgets]]")
        w(f'kind = "{kind}"')
        if param is not None:
            w(f'param = "{param}"')
        w(f"x = {x}")
        w(f"y = {y}")
        w(f"w = {wd}")
        w(f"h = {h}")
        w('touch_class = "M"')
        if vif is not None:
            w(f'visible_if = {{ param = "{vif[0]}", equals = {vif[1]} }}')
        w("")
    w("[[ui.displays]]")
    w('id = "wall"')
    w('kind = "display_list"')
    w("min_size = [2176, 1120]")
    w('lod = "auto"')
    w('colormap = "colormap.thermal"  # the display\'s declared default map (per-cell maps ride items)')
    w("")
    for i in range(1, 17):
        w("[[ui.displays.sources]]  # D4: the host resolves the enum to a registry id at snapshot time")
        w(f'id = "cell-{i:02d}"')
        w(f'stream = "param:cell_{i:02d}"')
        w("")
    w("[resources]")
    w("latency = 0")
    w('cpu_class = "light"')
    w('gpu_class = "medium"  # §5.8: measured by the harness against ceilings.rs; movement needs an operator ruling')
    w("")
    w("[capabilities]")
    w("fs_read = []")
    w('network = "none"  # the broker is host-side (D3); the T2 world has no network door')
    w("max_fuel = 2_000_000")
    w("max_memory_mb = 64")
    w("")
    w("[distribution]")
    w('package_format = "wasm"')
    w('entrypoint = "observatory.wasm"')
    w('platforms = ["any"]')
    w('arch = ["wasm32"]')
    w('min_host_version = "0.0.0-phase0"')
    w('channel = "beta"')
    return "\n".join(out) + "\n"


def gen_rust_table(rows: list[dict]) -> str:
    out: list[str] = []
    w = out.append
    w("//! The guest-side mirror of the stream registry's guest-visible metadata (WO-020 INC3).")
    w("//!")
    w("//! GENERATED from `crates/sparq-streams/streams.toml` by")
    w("//! `instruments-src/observatory/gen_manifest.py` — DO NOT EDIT. Regenerate:")
    w("//!   python3 instruments-src/observatory/gen_manifest.py")
    w("//!")
    w("//! The guest has no filesystem and no network (T2 capability-by-absence), so the registry")
    w("//! reaches it as compiled-in data — embedded in the wasm data section like the coastline")
    w("//! asset (D12). Drift gate: the harness test `streams_table_matches_the_registry` compares")
    w("//! this table against `sparq_streams::Registry` field by field, so a registry edit without a")
    w("//! regeneration fails a test rather than silently mislabelling a cell.")
    w("")
    w("/// One registry row's guest-visible metadata (cell chrome + renderer dispatch read this).")
    w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    w("pub struct StreamMeta {")
    w("    /// The stable stream id — what a manifest `stream` binding names.")
    w("    pub id: &'static str,")
    w("    /// Domain code: SW / SAT / GEO / WX / SP.")
    w("    pub domain: &'static str,")
    w("    /// The dropdown/cell-title label.")
    w("    pub label: &'static str,")
    w("    /// The renderer hint (§5.5): map-heat · map-points · map · timeseries · bars ·")
    w("    /// bars+gauge · gauge · timeline-scatter · timeline · text.")
    w("    pub view: &'static str,")
    w("    /// The unit string the cell's axes/footer must carry (token rule 9). May be empty.")
    w("    pub units: &'static str,")
    w("    /// The broker's minimum poll interval — the cell description shows it.")
    w("    pub cadence_s: u32,")
    w("    /// The frozen record schema this stream's windows carry (`observatory/<kind>@1`).")
    w("    pub schema: &'static str,")
    w("    /// The env var holding an API key, when the stream needs one (D14's KEY NEEDED words).")
    w("    pub key_env: Option<&'static str>,")
    w("}")
    w("")
    w(f"/// The registry's stream count (v1: {len(rows)}).")
    w(f"pub const STREAM_COUNT: usize = {len(rows)};")
    w("")
    w("/// The OFF sentinel's option index: every cell enum's option 0 is `\"\"` = cell OFF.")
    w("pub const OFF_OPTION: usize = 0;")
    w("")
    w(f'/// The OFF option\'s label (mirrors gen_manifest.py\'s `{OFF_LABEL}`).')
    w(f'pub const OFF_LABEL: &str = "{OFF_LABEL}";')
    w("")
    w("/// The registry, in file order — option index = 1 + position here.")
    w(f"pub const STREAMS: [StreamMeta; STREAM_COUNT] = [")
    for r in rows:
        key_env = r.get("key_env")
        key_txt = f'Some("{esc(key_env)}")' if key_env else "None"
        w("    StreamMeta {")
        w(f'        id: "{esc(r["id"])}",')
        w(f'        domain: "{esc(r["domain"])}",')
        w(f'        label: "{esc(r["label"])}",')
        w(f'        view: "{esc(r["view"])}",')
        w(f'        units: "{esc(r.get("units", ""))}",')
        w(f'        cadence_s: {r["cadence_s"]},')
        w(f'        schema: "{esc(r["schema"])}",')
        w(f"        key_env: {key_txt},")
        w("    },")
    w("];")
    w("")
    w("/// The stream a cell enum's option index selects (`None` for OFF / out of range).")
    w("#[must_use]")
    w("pub fn stream_by_option(option: usize) -> Option<&'static StreamMeta> {")
    w("    if (1..=STREAM_COUNT).contains(&option) {")
    w("        Some(&STREAMS[option - 1])")
    w("    } else {")
    w("        None")
    w("    }")
    w("}")
    w("")
    w("/// The option index of a stream id (1-based; `None` when the id is not in the registry).")
    w("#[must_use]")
    w("pub fn option_of_stream(id: &str) -> Option<usize> {")
    w("    STREAMS.iter().position(|s| s.id == id).map(|p| p + 1)")
    w("}")
    w("")
    w("/// Looks a stream up by id (`None` when the id is not in the registry).")
    w("#[must_use]")
    w("pub fn by_id(id: &str) -> Option<&'static StreamMeta> {")
    w("    STREAMS.iter().find(|s| s.id == id)")
    w("}")
    w("")
    w("impl StreamMeta {")
    w("    /// Splits `schema` (e.g. `observatory/grid@1`) into `(id, version)`. A malformed `@version`")
    w("    /// reads as version 1 (the observatory schemas' only version) rather than failing — the")
    w("    /// registry's own parse (`sparq_streams::schema::parse_schema`) already refused a bad")
    w("    /// version at load, so a generated table cannot carry one; this stays total anyway.")
    w("    #[must_use]")
    w("    pub fn schema_parts(&self) -> (&'static str, u32) {")
    w("        match self.schema.split_once('@') {")
    w("            Some((id, ver)) => (id, ver.parse::<u32>().unwrap_or(1)),")
    w("            None => (self.schema, 1),")
    w("        }")
    w("    }")
    w("")
    w("    /// Whether this stream needs an API key (D14) — the cell shows KEY NEEDED in words when the")
    w("    /// host reports the key unset.")
    w("    #[must_use]")
    w("    pub fn requires_key(&self) -> bool {")
    w("        self.key_env.is_some()")
    w("    }")
    w("}")
    w("")
    return "\n".join(out)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generate the Observatory manifest + streams table (WO-020 INC3).")
    ap.add_argument("--manifest-out", type=pathlib.Path, default=DEFAULT_MANIFEST_OUT)
    ap.add_argument("--rust-out", type=pathlib.Path, default=DEFAULT_RUST_OUT)
    ap.add_argument("--stdout", action="store_true", help="print the manifest and write nothing")
    ap.add_argument("--check", action="store_true", help="fail if either checked-in artefact is stale")
    args = ap.parse_args(argv)

    rows = load_streams()
    manifest = gen_manifest(rows)
    rust = gen_rust_table(rows)

    if args.stdout:
        sys.stdout.write(manifest)
        return 0

    if args.check:
        stale = []
        for path, content in ((args.manifest_out, manifest), (args.rust_out, rust)):
            if not path.is_file():
                stale.append(f"{path}: MISSING (generate it first)")
            elif read_text(path) != content:
                stale.append(f"{path}: STALE (regenerate: python3 instruments-src/observatory/gen_manifest.py)")
        if stale:
            print("gen_manifest --check: FAIL")
            for s in stale:
                print(f"  {s}")
            return 1
        n_opts = len(rows) + 1
        print(
            f"gen_manifest --check: up to date ({len(rows)} streams → 16 × {n_opts}-option enums; "
            f"{args.manifest_out.name} + {args.rust_out.name})"
        )
        return 0

    write_text(args.manifest_out, manifest)
    write_text(args.rust_out, rust)
    n_params = 16 + 2 + 5 + 3
    print(
        f"gen_manifest: wrote {args.manifest_out} ({n_params} params ≤ 32 cap, "
        f"{len(rows) + 1} options per cell enum) and {args.rust_out}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
