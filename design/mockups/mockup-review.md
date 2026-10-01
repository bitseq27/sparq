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
| 9 | The rail in `design-mode.svg` is icon-ONLY, which look-board §6/§8 forbid in Design mode; finding 3's "glyph + tooltip" remedy is hover-only, which §8 forbids on a touch instrument. The shipped shell (WO-012 inc 3) resolves it the operator's way: **glyph + permanent micro-label** (10 px, the XS type token) inside the mockup's own 44 px box at 52 px pitch — board-compliant, ~95 % of the mockup's rail | medium | no token move needed (the rail/button token pair already sized it); the board stands unamended | ✅ shell |
| 10 | The mockup's dock spans the CANVAS column only while rail and inspector run full height; the shipped shell's dock ran under everything. `shell::compute` moved to the mockup's column discipline (WO-012 inc 3); the layout test now pins rail/inspector full-height and the dock between them | minor | none (rects are token-derived) | ✅ shell |
| 11 | The mockup's inspector is 480 px wide at 2560; the token default is 400. Kept 400: at the tablet-min breakpoint 480 would force-collapse the inspector (the 60 % canvas floor), and the mockup's own tablet variant does not exist yet to say otherwise. The width is user-resizable inside 320–560, so 480 is one drag away | minor | none — declared deviation | ✅ recorded |
| 12 | The mockup's top bar shows only a status dot at right; the shipped shell keeps five WORD buttons there (DESIGN/HC/RAIL/INSP/DOCK) because they are functional controls and §6 pairs icons with labels. The dot itself shipped (paired with `LIVE`), plus the mockup's logo glyph, section dividers and top-bar diagnostics line | minor | none — declared deviation | ✅ shell |
| 13 | The mockup's rail-foot level tick is GREEN; the shipped one is the AUDIO class colour (amber), because §4 makes colour the signal class and the tick meters the master audio. Declared deviation, pattern/position per mockup | minor | none | ✅ shell |
| 14 | Visual review needed a screenshot without a GPU: `sparq ui --svg-out PATH` dumps the headless frame's vector shapes to SVG (the painter's own output, text included). `design/mockups/convergence-wo012-inc3.png` is the first convergence sheet made with it (mockup over shell, at rest — wires dim by design until signal flows) | tooling | none | ✅ |
| 23 | The unfinished-patch flag (operator ruling 2026-09-30): a node with a required input unwired wears the error token at 0.12 alpha over its body fill, an error-weight border, and the word NO IN in the badge row (error ring at Dot). Pattern-plus-colour-plus-word, per S4/S8: the tint alone would be a colour-only state | minor | none (the error token existed) | done, shell (inc 5b) |
| 24 | Perform mode removed from the runtime (operator ruling 2026-09-30): the top-bar mode word button and the rail mode glyph are gone, the shell loads straight into Design at every viewport; below tablet_min the layout reflows (inspector, then rail, collapse to keep the canvas floor) and says so once in words instead of refusing Design. perform-mode.svg stays the parked target (finding: Perform convergence) | medium | none | done, shell (inc 5c) |
| 25 | Warm shift (operator ruling 2026-09-30, the PN convergence): grounds/hairline/text tokens moved to the warm black-brown ladder and the chrome accent to orange; the five signal classes kept their hues. The three mockups were re-hexed to the new tokens so R1 keeps passing — they remain the LAYOUT authority, the PN screenshot is the ANATOMY reference (convergence-wo012-inc6.png pairs them) | major | colors.toml (ground/hairline/text/state.selected) | ✅ regenerated + rehexed |
| 26 | One slider vocabulary: the round knob everywhere (node cards AND inspector); the mockup's block thumb retired — two knob shapes for one control would be two truths | minor | none | done, shell (inc 6) |
| 27 | The node card's left stripe is the DOMINANT SIGNAL CLASS at emphasis weight — PN's category-stripe position, sparq's §4 meaning (colour says what it carries) | minor | none | done, shell (inc 6) |
| 28 | Long manifest param names truncate with an ellipsis on the card and in the inspector label column; the VALUE (number + unit) is never the thing that gets cut (§5: a number without its unit is a bug) | minor | none | done, shell (inc 6) |
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
| 15 | Operator ask: smaller, grouped, coloured palette tiles. 248x80 cards became 112x56 tiles grouped by manifest top category under xs words, stripe in the dominant SIGNAL class (S4 keeps its rule: the stripe says what the module carries, the group says what it is). Touch class L to M; the touch table's dock row is superseded | medium | dock_card_w/h moved through the generator (248/80 to 112/56) | done, shell |
| 16 | Mouse usability (operator ask): hover motion is dropped at the window adapter (the suppression line can now only mean a real missed Down); right-click synthesises the recogniser's own Context intent; the wheel synthesises Pan (the panel-over-panel routing reused - no parallel semantics, no modifier keys). Hover affordances stay parked (S8) | medium | none | done, shell |
| 17 | out/main meter bars: green DATA-class fills with amber AUDIO-class peak-hold blocks - a meter is data about the signal, which is how the mockup's green and S4 reconcile; holds decay on audio time (block counts), never a wall clock | minor | none | done, shell |
| 18 | Slice B: the RESPONSE well's curve draws in the module's DOMINANT CLASS colour (svf carries audio: amber), not the mockup's cyan - S4's rule (colour says what the module carries) postdates the mockup's accent choice; the thumbnail and the plot agree by construction | minor | none | done, shell (inc 5) |
| 19 | No glow / under-curve fill on the param-derived curve - S8 forbids glow not tied to amplitude, and a param curve has no live amplitude; the mockup's glowC + 0.06 fill are decoration the instrument refuses. The live cv-history overlay (D1-prime) may earn glow back when a level exists to encode | minor | none | done, shell (inc 5) |
| 20 | No f_max tick word ("20 k") at the 400 px inspector: it would sit on the -48 dB word's pixels. Decades (10 Hz / 100 / 1 k / 10 k) plus the two dB words carry the scale; a collision is worse than an absent label | minor | none | done, shell (inc 5) |
| 21 | The plot well is 96 px (LAYOUT_SPACE_9) not the mockup's 180: the 400 px panel also carries the port strip and the rows, and the well must stay legible at the tablet-reflow width. The mockup's 448x180 well lives in a 480 px inspector (finding: inc 3's declared 400) | minor | none | done, shell (inc 5) |
| 22 | Envelope / sparkline wells draw their param shapes as HAIRLINE-FAINT outlines at rest (the scope rest line's vocabulary); the mockup's lit wells await D1-prime's live cv-history overlay, which is parked in LATER with its discipline named | minor | none | done, shell (inc 5) |
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
