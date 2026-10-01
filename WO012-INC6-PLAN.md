# WO-012 increment 6 — the Persistent-Nodes convergence (plan of record, 2026-09-30)

Operator reference: https://kageproduction.com/persistentnodes (screenshot archived at
`/home/user/ref/pn0.png` in the build sandbox; the convergence sheet pairs it with the shell).
Operator rulings via question round: **warm-shift the tokens** (grounds/text/hairlines go warm
black-brown, primary accent orange; the five signal-class colours stay), **PN-style node cards
with inline parameter sliders**, **all three new surfaces** (left NODE LIBRARY sidebar with
search + category filter + miniature preview cards, replacing the dock palette; the second
toolbar row; right-edge vertical master IN/OUT meters), **project buttons parked** (SAVE /
OPEN / HISTORY / NODES COMPILE do not exist yet and will not be faked — LATER holds them).

The patterns this increment copies are the ones the tree already enforces: computed-not-drawn
(models in `sparq-ui`, painters in `sparq-app` read numbers and invent none), tokens as the only
colour/size source (`token_gen` + `token_audit` R6), honesty at rest (empty meters, declared
parked doors), one source of truth, and every new control registered in the layout audit.

## D1 — The warm shift, through the token file and nowhere else.

`design/tokens/colors.toml` moves: ground/base/canvas/panel/panel-alt/inset/overlay to the warm
black-brown ladder PN reads as (#0B0A08 … #201C18 family), the hairline colour to a warm grey,
the text ladder to warm whites, and `state.selected` (the chrome accent: selection, active tab
underline, marker) to the orange primary. The five signal classes KEEP their hues (operator
choice; they already sit where PN's port colours sit — amber audio, cyan control). Regenerate
(`token_gen`), re-audit (`token_audit`), and re-hex the three mockups' ground/text/hairline
values to the new tokens so R1 keeps passing — the mockups stay the layout authority; for
anatomy, the PN screenshot is the new reference board (finding 25). `preview.html` regenerates.

## D2 — Node card anatomy: header, inline params, well, port band.

`layout::node_size` grows a PARAM BAND between the header and the port band: one row per
parameter in manifest order, capped at `NODE_PARAM_ROWS` (token-scale constant 8) with a
"+N MORE" dim word row when capped (the inspector keeps the whole list — the cap is a reading
limit, not an editing one). Row anatomy, PN's: label left (xs, text_tertiary, uppercased),
value right (xs, the param's dominant class colour), slider track under the label line with a
ROUND knob (the mockup's block thumb stays in the inspector; the node's knob is PN's round
one — two vocabularies would be a lie, so the inspector's block thumb moves to the round knob
everywhere, finding 26). The well band (scope/meters/envelope/sparkline/curve) sits under the
param band, unchanged in vocabulary. Port band unchanged at the bottom; ports keep their
lettered class circles.
Header gains PN's right side: the category word + the node id, dim, xs (the title keeps the
left; the MASTER/flag badges keep their precedence). The left category stripe is the node's
DOMINANT SIGNAL CLASS at signal weight (sparq's §4 rule — colour says what it carries — reads
as PN's stripe without adopting PN's category accents; finding 27).
Touch: an inline slider row is a control → class-S dense (32 px row, non-destructive, badged),
registered ONLY while its on-screen height clears the dense floor (the node-body gating rule);
below that zoom the row is a reading, the inspector is the door.

## D3 — Inline slider interaction reuses the Param drag, never a second op.

`layout::hit_test` gains `Hit::Param(node, index)` ahead of `Hit::Node` (a row under the finger
is the row you edit — the inspector's rule, on the canvas). `interact::drag_start` routes it
into the EXISTING `Interaction::Param` with the row's track rect as the source (the inspector
and the node share one tap-to-set / drag / coalesce / undo path — one door, two surfaces); the
live session's param ring traffic is unchanged because the op is unchanged.

## D4 — NODE LIBRARY sidebar: the dock palette promoted, searched and previewed.

`shell::compute` inserts a left column (token `shell.library_width`, default 240, collapsible
from the top bar's LIB button) between the rail and the canvas; the dock LOSES its MODULES tab
(the palette's content moved; the dock keeps LOG and the parked tabs) — a tab whose content
moved would be a stub wearing a live tab's clothes. Sidebar content, PN's order: search field
(the browser's fuzzy `rank` over the registry catalogue — one ranking, two surfaces), a
category select (the manifest top categories present in the registry, "ALL" first), then the
card list, scrolled by the wheel-over-panel routing the inspector owns (view state on
`CanvasState`, reset like the inspector scroll). Cards: a MINIATURE of the node the registry
describes — header dot + display name, port letter columns both edges, the first three param
rows, the well shape where the registry declares one — drawn by the same painter helpers at a
fixed card scale from the manifest alone (no state, no session: a card is a promise, not a
reading), plus the name and `IN n | OUT m` under it. A card tap spawns at canvas centre through
the browser's own op path (undoable, ledger-marked) — the dock card's door, moved and upgraded.
Overflow (list longer than the sidebar) scrolls; the count is said in words when filtered.

## D5 — Toolbar row 2: the canvas's own controls, in words and numbers.

Under the top bar, over the canvas column: FIT (zoom-to-fit), RESET (camera home), ARRANGE
(new `Op::Arrange`: grid-arrange the graph in registry-order rows, ONE undo step carrying the
orig positions — an arrangement is one decision), zoom − / percent / + (the camera's step, the
percent read from the camera), the wire-curve select (SMOOTH bezier / STRAIGHT — a display
option on `CanvasState`, the painter's sampler reads it; a third style is parked, not faked),
the live count "N NODES - M CONNECTIONS" from the graph, and the hint words (moved off the
canvas bottom). Every one is a registered class-S dense control; every one already has its
effect elsewhere in the model — the toolbar is a door, not a duplicate.

## D6 — Right-edge master meters: the ring's truth, vertical.

A non-interactive strip inside the canvas's right edge: IN and OUT well pairs (L/R each, the
stereo vocabulary sideways). OUT = the resolved master's first audio output port's
`LiveMeters` entry; IN = the meter of the port FEEDING the master's input (the same ring
entry the wire lights from — pre-trim, honest; an unwired master reads an empty IN well and
says so by being empty). Peak-hold blocks decay on audio time as everywhere. At rest: empty
wells. Nothing to touch, nothing registered — a reading.

## D7 — What does NOT move.

The audio path, the executor, the rings, the goldens (display-side increment: layout rects,
painter, tokens — no DSP, no contract). The inspector keeps selection fine control, the
response plot and its marker. The rail keeps transport. Perform stays removed (5c). Save /
open / history / compile stay parked (ruling). The breakpoint reflow keeps its rule and gains
the library column in its collapse order (library yields after the inspector, before the
rail — it is the widest of the three leftovers).

## Acceptance (sandbox)

* New unit tests: layout param-band geometry + cap + hit-test precedence; arrange op invertibility;
  library ranking/filter determinism; card miniature purity (same manifest → same card);
  meter-strip mapping; token regeneration clean.
* Smokes (count grows from 44): spawn from a searched library card; inline slider drag edits the
  param (one undo step, live ring traffic when playing); arrange + one undo restores positions;
  zoom buttons move the camera percent; wire-style select changes the drawn wire points; master
  strip hot after PLAY+pump and empty at rest; library wheel scrolls, canvas wheel still pans.
* Standing gates: fmt · clippy every runnable cell · workspace tests · 5 python gates · release
  goldens bit-identical · selftest 9/9 · 17/17 · `ui --audit` (matrix incl. the library column and
  the SMALL-REFLOW cell, DPI-invariant) · convergence sheet paired with the PN screenshot
  (`design/mockups/convergence-wo012-inc6.png`).
* Seal only when complete and green: `sync-wo012-inc6.zip`, stamp moved once, BASELINE moved.

## Out of scope (declared)

Project save/open/history/compile (ruling) · a third wire style · port-type filter in the
library (search + category ship; the port filter is parked) · node resize · PN's preset strip
(no preset concept exists yet) · mockup anatomy restyle beyond the warm re-hex (the mockups
keep sparq's own layout authority; PN is the anatomy reference, recorded as finding 25).

## Postscript — the acceptance, measured (2026-09-30, tenth session round 4)

Built to this plan, D1–D7 as decided; two decisions moved during the build and are recorded
here: the reflow order stays inspector-first (at 1440×900 and 1280×800 the inspector yields to
the canvas floor while the library keeps its column — params live on the node cards now, so the
card is the editor and the inspector is the fine-control surface; collapsing the library brings
the inspector back at any width), and the fuzzy search's subsequence looseness ("sine" also
ranks `util/gain` through s-par-q/ut-i-l/gai-n-e) is embraced rather than tightened: the smoke
asserts the ranking puts the exact name FIRST and the count word matches what the list shows.

* **804 tests (+1 ignored)** — unchanged count (the round added layout/interact/shell tests and
  retired none; the 5c count holds), **50 smokes** (was 44: library search/filter/scroll/count,
  inline slider edit + one undo, ARRANGE + one undo, zoom buttons + RESET, wire-style restyle,
  master strip rest/hot; smoke 2's load-in-Design and the inc5 inspector-scroll smoke moved to
  viewports where their subject lives).
* **Gates:** fmt · clippy clean in default, `bootstrap-audio`, `ui`, native `ui-window`, MSVC × 6
  (MSVC `ui-window` still the declared sandbox OOM) · 5 python gates (token_audit now scans the
  warm palette; `make_display_sheet.py` lost its last hard-coded hexes — the generator reads the
  token file, so the next palette move cannot re-freeze it) · release goldens bit-identical
  (`ba577186c988db21`, `dd975a24f03b19c1`, stress `7bb06379bd6845e5`, determinism
  `0f5c3e86c7f117a9`) · selftest 9/9 · 17/17 · 411.1× realtime · `ui --audit` PASS, matrix
  0 violations with the library column and the new dense controls measured, DPI-invariant.
* **Visual:** `design/mockups/convergence-wo012-inc6.png` pairs the Persistent-Nodes reference
  screenshot with the shell's `--review` state: warm grounds, the library sidebar with preview
  cards, the toolbar row, node cards with inline sliders and round knobs, the well band, the
  flagged bare Delay, the right-edge master strip. Findings 25–28 declare the deltas from
  sparq's own mockups (warm palette, round knob everywhere, class stripe, label truncation).
* **Seal:** `sync-wo012-inc6.zip` (27 entries — heading, namelist and archive counts verified
  equal), stamp moved once at completion; `log_check.py` BASELINE moved with it.
