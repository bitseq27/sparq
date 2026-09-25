# WO-004 — mockup set, review protocol and findings log

**Mockups** (all drawn from `design/tokens/*`; CI audit confirms zero off-token colours and only approved stroke weights):

| File | Breakpoint | Shows |
|---|---|---|
| `design-mode.svg` | 2560 × 1600 (large / display wall) | Graph canvas with a 10-node patch, signal-class wire encoding, selection state, bypass state (pattern-encoded), inspector with ports/params/modulation/response/resources, dock with module browser, top-bar diagnostics, rail with transport/mode/browse |
| `perform-mode.svg` | 2560 × 1600 (projector) | Full-bleed sonogram + scope trace, hero `bar.beat.tick` at 88 px, scene matrix, 8 XL macro pads, transport at 96 px, 8-channel metering, system readout, panic |
| `display-sheet.svg` | 2560 × 1652 (large) | **Generated** by `tools/make_display_sheet.py`: 12 display modules — scope (+X/Y inset), spectrum, sonogram, phase portrait, vector field, geometry (L-system + rule-30 CA), rig map, graph view, DNA view, streams inspector, meters, matrix |

### Patch shown in `design-mode.svg` (it is a real, if crude, sparq patch)

`mod/clk-div` (implied trigger source) → `syn/membrane` → `flt/svf` ← `env/ad`, `mod/lfo`; `syn/polyblep` → `flt/svf` → `util/gain` → {`ana/rms`, `fx/bitcrush` (bypassed), `out/main`, `dsp/scope`}; `syn/noise` → `fx/bitcrush`; and **`ana/rms` → `flt/svf.cv cutoff`** — the analysis-as-control-source principle, drawn as a cyan wire back into the filter. That single wire is the thesis of the whole instrument; it should appear in every public screenshot.

---

## Display sheet — remaining work

`display-sheet.svg` covers 12 of the 14 modules at the large breakpoint. Still to do:

* **`dsp/numeric`** is demonstrated by the hero readouts in `perform-mode.svg` (88 px `bar.beat.tick`, 44 px tempo) rather than in a tile — record that as satisfied.
* **`dsp/particles`** and **`dsp/feedback`** need real GPU shaders; drawing them statically would misrepresent them. Defer to Phase 6 and capture them as screen recordings, not mockups.
* **Tablet (1280×800)** and **wall (3840×2160)** variants: the same objects re-flowed. Generate them by parameterising `make_display_sheet.py` with a breakpoint argument — do **not** hand-draw a second set.

The full module list for reference:

`dsp/scope` (waveform + X/Y with phosphor persistence) · `dsp/spectrum` (magnitude + sonogram) · `dsp/phase-portrait` (delay-embedded attractor) · `dsp/vector-field` · `dsp/geometry` (L-system turtle, CA grid, Voronai/Delaunay, lattice) · `dsp/rig-map` (overhead speaker layout + object trajectories, violet class) · `dsp/graph-view` (the patch itself with live levels on edges — the signature image) · `dsp/dna-view` (mutation lineage tree, greyscale depth encoding) · `dsp/streams` (every stream: value, rate, jitter, sparkline, quality flags) · `dsp/meters` · `dsp/matrix` (N×M mapping grid) · `dsp/numeric` (hero readouts) · `dsp/particles` · `dsp/feedback` (ping-pong buffer visual).

Constraints to verify while drawing: every display must be legible with the high-contrast theme; every display must survive the monochrome test; no display may need a colour that isn't a token; axis labels follow `typography.toml [format]` (units always present).

---

## Review protocol (run each mockup through all five)

1. **Blind identity test.** Show at full resolution, no title, to someone who doesn't know the project. Ask *"what kind of software is this?"* **Pass** if the answer lands in {scientific, laboratory, engineering, measurement, research, instrumentation}. **Fail** if {music, game, consumer, DJ}. Record verbatim answers below.
2. **Distance test.** Perform mode at 3 m on the target display: every primary numeral readable; every macro pad identifiable; no Design-mode chrome accidentally legible.
3. **Dark-room test.** Near-darkness with one practical light: no blooming whites, hairlines still visible, states still distinguishable by pattern.
4. **Glove/sweat test** (on the stage device, not the mockup): wire-drag, long-press menu, macro pads work with a thin glove and damp fingertips; palm rejection holds while dragging across the canvas.
5. **Monochrome test.** Convert to greyscale: signal classes still distinguishable by weight/dash, states by pattern, hierarchy intact.

## Measurement tables (fill in during WO-004)

**Blind identity test**

| # | Viewer (role) | Verbatim answer | Pass/Fail | Notes |
|---|---|---|---|---|
| 1 | | | | |
| 2 | | | | |
| 3 | | | | |
| 4 | Display-sheet DNA tree invented its own greyscale ramp (`#464C52…#E2E8EE`) instead of sampling the `greyscale` colour map — caught by `token_audit` R1 | **major** | none needed (token existed); the *mockup* was fixed to sample `grey_at()` from `colormaps.json` | ✅ |
| 5 | `token_audit` R1 originally accepted all 1 785 baked LUT colours, which made the rule meaningless | **major (tooling)** | R1 split into R1 (declared tokens only) + R1b (LUT samples are data encoding only); fills routed to documented human review | ✅ |
| 6 | R1b first draft forbade LUT fills on `<rect>`/`<circle>` — but a spectrogram bin *is* a `<rect>` | medium | chrome set narrowed to `rect` only when **stroked** with a LUT colour, plus `g/use/image/text/tspan`; fills on data marks permitted and review-checked | ✅ |
| 7 | Perform-mode transport glyphs needed 96 px icons; a 3 px stroke vanished | minor | see finding 1 (`stroke.hero = 4`) | ✅ |
| 8 | | | | |
| 5 | | | | |

**Distance / dark-room**

| Surface | 3 m legible? | Dark-room OK? | Bloom found? | Fix |
|---|---|---|---|---|
| Perform hero numerals (88 px) | | | | |
| Perform scene labels (15 px) | | | | |
| Perform macro values (32 px) | | | | |
| Top-bar diagnostics (13 px) | *not required at 3 m* | | | |

**Touch audit (from the mockup geometry)**

| Element | Size (px) | ≥44? | Class |
|---|---|---|---|
| Macro pad | 144 × 144 | yes | XL |
| Transport button | 96 × 96 | yes | XL |
| Scene cell | 76 × 76 | yes | L |
| Inspector slider row | 408 × 44 | yes | M |
| Dock module chip | 248 × 80 | yes | L |
| Port (drawn / capture) | 12 / 48 | capture yes | — |
| Canvas node header | 240 × 32 | **no — not interactive by itself; long-press target is the whole node (240 × 128)** | — |

## Findings and token changes (log as you go)

| # | Finding | Severity | Token/spec change | Applied |
|---|---|---|---|---|
| 1 | Perform-mode transport glyphs needed a stroke weight above 3 to read at 96 px | minor | added `stroke.hero = 4`, restricted to Perform-mode hero glyphs | ✅ `layout.toml` |
| 2 | The sonogram ramp used mid stops that weren't declared as tokens | minor | added `signal.audio.ramp` (17 stops) and bound `colormaps.toml [phosphor]` to it | ✅ `colors.toml` |
| 3 | Rail section captions ("TRANSPORT", "MODE", "BROWSE") do not fit a 56 px rail | minor | removed; rail buttons rely on glyph + tooltip, and the rail is not used in Perform mode | ✅ mockup |
| 4 | Display-sheet DNA tree invented its own greyscale ramp (`#464C52…#E2E8EE`) instead of sampling the `greyscale` colour map — caught by `token_audit` R1 | **major** | none needed (token existed); the *mockup* was fixed to sample `grey_at()` from `colormaps.json` | ✅ |
| 5 | `token_audit` R1 originally accepted all 1 785 baked LUT colours, which made the rule meaningless | **major (tooling)** | R1 split into R1 (declared tokens only) + R1b (LUT samples are data encoding only); fills routed to documented human review | ✅ |
| 6 | R1b first draft forbade LUT fills on `<rect>`/`<circle>` — but a spectrogram bin *is* a `<rect>` | medium | chrome set narrowed to `rect` only when **stroked** with a LUT colour, plus `g/use/image/text/tspan`; fills on data marks permitted and review-checked | ✅ |
| 7 | Perform-mode transport glyphs needed 96 px icons; a 3 px stroke vanished | minor | see finding 1 (`stroke.hero = 4`) | ✅ |
| 8 | | | | |

**Rule:** any finding that changes appearance changes a *token*, never a mockup-local value. If you can't express the fix as a token, the fix is wrong.

---

## Colour-map-filled regions (required by token-spec rule 11)

The auditor cannot tell a data mark from chrome, so every LUT-sampled region is declared here and review-checked.

| Mockup | Region | Colour map | Data encoded |
|---|---|---|---|
| `perform-mode.svg` | full-bleed sonogram field (36 × 80 cells) | `phosphor` | log magnitude per bin per time row |
| `display-sheet.svg` | `dsp/spectrum` bin columns + peak-hold line | `phosphor` | log magnitude |
| `display-sheet.svg` | `dsp/sonogram` cells (44 × 96) | `phosphor` | log magnitude over time |
| `display-sheet.svg` | `dsp/vector-field` arrow strokes + heads | `phosphor` | vector magnitude |
| `display-sheet.svg` | `dsp/dna-view` node strokes + edges | `greyscale` | lineage depth |
| `display-sheet.svg` | `dsp/matrix` cell fills (8 × 8) | `phosphor` | per-cell gain |
| `design-mode.svg` | none | — | Design mode uses declared accents only |

Anything LUT-sampled **not** in this table is a violation.

## Automated conformance status

```
$ python3 tools/token_audit.py
token_audit · 66 token colours · 1785 LUT samples available · stroke widths [1, 2, 3, 4] · radii [0, 2, 4]
audited 3 svg + 0 source file(s)
  LUT-sampled colours (data encoding; fills are review-checked): display-sheet.svg 236
clean — no violations
```

The auditor is proven to fail on a deliberately bad fixture (see `tools/README.md`) — a gate that cannot fail is not a gate.
