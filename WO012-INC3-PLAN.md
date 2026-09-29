# WO-012 increment 3 — mockup conformance, chrome: the shell draws like `design-mode.svg` (plan of record, 2026-09-29)

The operator applied all three waiting bundles on SATURN and then asked for the interface
itself: *"I would like it looking like the design-mode.svg."* The mockup is the spec; the
look-board is the conformance authority; `mockup-review.md` is the findings log. Reconnaissance
measured the mockup against the shipped shell: the SKELETON already matches (the tokens were
derived from the mockup — top bar 48, rail 56 with 44 px buttons at 52 pitch, inspector 480,
dock 260, node 240 wide, 32 header), and every delta is INSIDE the panels. This increment
closes the chrome deltas; the node inset displays and the inspector response plot are
increment 4 (the operator's two-increment slice).

Measured deltas (mockup @2560×1600 vs the shipped painter, this session):

| # | Surface | Mockup | Shipped today |
|---|---|---|---|
| 1 | rail | 1px glyph buttons in hairline boxes, grouped by hairline dividers; a level-meter tick and a magenta ADD at the rail's foot | six TEXT buttons, one run, no dividers, no meter, no add |
| 2 | top bar | star glyph at left, hairline section dividers, diagnostics + a status dot at right | wordmark + a stale "phase 0 shell prototype" subtitle + five text buttons |
| 3 | canvas | floating wire-encoding legend, top-right (five classes, sample + word) | a one-line affordance hint at bottom-left only |
| 4 | dock | module browser as a CARD GRID (248×80 cards, class dot, name, category) | header + five DISABLED tab words + a prose stub |
| 5 | inspector | port-dot summary, cyan control sliders with block thumbs | title row + neutral hairline sliders, no port summary |

## The decisions

**D1 — The rail keeps the mockup's footprint and the board's rule: glyph + permanent micro-
label inside the existing 44×44 button.** The operator chose this between the mockup's icon-only
rail (which look-board §8 forbids in Design mode, and whose "tooltips" remedy — review finding 3
— are hover-only, which §8 forbids too) and today's text. Glyph on top (1 px geometric stroke),
the word under it at the smallest type token, both inside the 44 px target at the 52 px pitch:
the rail's 56 px width and every reflow number stay exactly as designed. Groups get the
mockup's hairline dividers: TRANSPORT (PLAY triangle, STOP square, PANIC circle — the circle
stays red-before-pressed with its word, colour never alone) | VIEW (MODE, MODS, DIAG) | and at
the rail's foot, the mockup's two bottom elements made real: the **level tick** (a 36×4 well
filled by the live master peak — data, at rest an empty well, never a frozen bar) and the
magenta **ADD** cross, which opens the module browser at the canvas centre through a new
documented driver door (`CanvasState::browser_open_at`) — the sheet the long-press already
opens, from a button a finger can find.

**D2 — The top bar becomes the mockup's: star glyph + wordmark, section dividers, live
diagnostics, status dot — and the five working buttons stay, words and all.** The stale
"phase 0 shell prototype" subtitle is replaced by the DIAGNOSTICS the mockup's own review table
names ("top-bar diagnostics, 13 px"): while a session runs, the negotiated truth with units
(`48.0 kHz · 64 fr · 22.7 ms · xrun 0`), else the build stamp line — measured or stamped, never
decorative. The status dot (mockup's red dot) is the session state: absent at rest, green+`LIVE`
while playing, red+`PANIC` for one second after a panic — always paired with its word (§4:
colour never the only encoding). The right-hand text buttons (DESIGN/HC/RAIL/INSP/DOCK) are
functional controls and keep their words per §6.

**D3 — The wire legend is chrome drawn from the canvas painter's own encoding table, top-right
of the canvas, Design mode, every LOD.** One hairline box, five rows: the sample (the exact
stroke the class draws — solid/dashed/dotted/double, class colour) and the class WORD. It is
drawn by a new `canvas_ui::draw_wire_legend` so the legend and the wires cannot drift: same
`class_encoding`/`class_colour` source. Not interactive → no audit element. Perform mode:
absent (the stage surface owns the screen).

**D4 — The dock's MODULES tab becomes the card grid, for real.** Cards are the registry
catalogue (the same `BrowserItem` list the browser sheet ranks — #58's rule reaches the dock:
it cannot offer what is not installed): 248×80 (the review's own touch table, class L), class
dot + display name + category + version, hairline border, the selected card's border one tier
up. Tap spawns at the canvas centre through a new driver door (`CanvasState::spawn_module`)
that rides the SAME op path as the browser's row tap — undoable, ledger-marked, one undo step.
Cards that do not fit the dock height are not drawn silently: a count line in words says how
many wait below ("+ N modules — resize the dock or search with MODS"). The other four tabs
(LIBRARY/STREAMS/SCENES/JOURNAL) stay DISABLED words — drawing them live with nothing behind
them is the lie the stub exists to avoid.

**D5 — The inspector gains the port summary and the control-cyan slider; the parked things
stay parked.** Under the title: one row of class-coloured dots, inputs then outputs, in
manifest order — the node's ports at a glance, from the spec (data, not decoration). Sliders:
the track draws in the control class (cyan — a slider IS a control), the thumb becomes the
mockup's vertical block (space-token wide, row-high); value + unit right-aligned stays exactly
as the numeric discipline requires. The mockup's segmented boxes are numeric entry — PARKED in
LATER.md since increment 3 — and are NOT drawn; a fake control is worse than an absent one.

**D6 — No new tokens, no layout-token moves.** Every new dimension derives from the space
scale, the type scale and the existing shell tokens (the review's rule: a fix that cannot be
expressed as a token is wrong — these fixes need no new token because the mockup and the shell
already share them). `token_audit` R6 keeps jurisdiction over every painter line touched.

**D7 — Ids and smokes stay stable; the audit matrix counts move and say so.** Every existing
registered id (`rail/transport/play`, `topbar/mode`, …) survives; new targets get new ids
(`rail/add`, `dock/card/<i>`). The breakpoint matrix's element counts move with the dock cards
— the audit gates violations and cleanliness, not counts, and the seal notes the new numbers.
Two new smokes: (a) a dock-card tap spawns the module and one three-finger undo removes it;
(b) the legend's mode/LD contract in shape counts (present Design Full and Simplified, absent
Perform).

## Proof burden

The five existing chrome smokes (dock toggle, mode taps, long-press context) pass UNCHANGED —
their ids and rects survive; the 27 other smokes never touch chrome geometry that moves. New:
the two smokes above. Everything else is the standing gate table: fmt, every runnable clippy
cell, workspace tests at the new count, the 5 python gates (R6 over the new painter code),
release goldens bit-identical, selftest 9/9, `ui --audit` PASS at the new smoke count, stress
and exec renders untouched (no audio path moves — this increment is pixels and chrome only).

## Postscript — measured, and the deviations declared (2026-09-29)

**Gates on the final tree:** fmt clean · clippy clean in the default, `bootstrap-audio`, `ui`,
combined audio+hal, native `ui-window` (single-job) and all six runnable MSVC cells · **777
workspace tests, 0 failed, 1 ignored** (unchanged count: this increment is pixels and chrome —
its new gates are the two chrome smokes and the moved layout test) · the 5 python gates —
`token_audit` R6 caught the SVG dumper's page-ground hex on its first pass and the dumper now
reads `COLOR_GROUND_BASE.hex`, which is exactly the jurisdiction R6 exists for · release goldens
bit-identical (stress `7bb06379bd6845e5`, the three pinned exec renders, `canvas-render.wav`
1 920 046 B · `d7ad294e…`) · selftest 9/9 · `modules --strict` 17/17 · `ui --audit` **PASS, 34
smokes** · the breakpoint matrix grew with the dock cards (33–42 elements per Design cell by
viewport, 0 violations, the 44 px floor holding) · stamp `src 94f/2155888B`.

**New instrument:** `sparq ui --svg-out PATH` — the headless frame's vector shapes as SVG, a
screenshot without a GPU. The convergence sheet `design/mockups/convergence-wo012-inc3.png`
(mockup over shell) is its first artefact and the review protocol's first *repeatable* visual
comparison; findings 9–14 in `mockup-review.md` record every declared deviation from the mockup
(rail micro-labels, inspector 400 vs 480, top-bar word buttons, the amber level tick) and the
two conformance moves (dock column discipline, port-dot strip).

**What the dump showed that the numbers could not:** at rest the shell reads one tint step
darker than the mockup's *depicted* state because the mockup depicts a LIVE patch (glowing
wires, lit meters) and the dump shows the honest rest state — "silence looks silent" is the
board's rule, not a deviation. Increment 4 (node inset displays) closes the remaining depicted
gap with real data, not paint.

## Out of scope (increment 4 and beyond, declared)

Node inset displays (the mockup's per-module sparkline/meter/scope wells beyond the shipped
scope screen), the inspector response plot with its draggable marker, the mockup's measure
glyphs, Perform-mode convergence, the tablet/wall mockup variants, phosphor persistence.
