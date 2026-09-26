# sparq canvas gestures — the WO-013 map

**Status:** increment 1 (2026-09-24). The recogniser is WO-012 (`gesture.rs`); this file is the
**canvas consumer** of its intents — `sparq-ui::canvas` (model / camera / layout / connect /
interact) plus the egui painter `sparq-app/src/ui/canvas_ui.rs`. Everything here is
toolkit-independent and headless-proven: `sparq ui --audit` runs 5 canvas smokes through the real
recogniser (drag-node+snap+fine, three-finger undo, incompatible-refused, compatible-connect,
two-finger zoom). What needs a real digitiser is listed at the bottom, honestly.

**Related:** `docs/ui/input-model.md` (the gesture vocabulary), `SPARQ-PLAN.md` §14.3 (touch rules),
`docs/api/compat-matrix.toml` (what may connect), `design/look-board.md` (stroke language).

---

## 1. The rule

*A gesture produces an intent; the canvas turns intents aimed at it into operations on the graph.*
The recogniser never touches the graph; the canvas never reads a pointer. Between them is the
intent, and on the far side of the canvas is an **op** — a value with an inverse, so undo is a
property of the representation, not hand-written rollback (`sparq-ui::canvas::model::Op`).

Routing (`ShellUi::route_to_canvas`): in **Design** mode, a positional intent (tap, long-press,
drag-start, double-tap) goes to the canvas when it lands inside the canvas rect; a drag in flight
stays with whoever started it; pan / zoom / three-finger-undo are always the canvas's. In **Perform**
mode the graph is inert (the stage pads own the screen) and everything routes to the shell.

## 2. Intent → canvas effect

| Intent (WO-012 recogniser) | Target under the finger | Canvas effect | Op / event |
|---|---|---|---|
| `DragStart` | **port** | begin a pending wire from that port | — (interaction = Wire) |
| `DragStart` | **node body** | select it (or keep the multi-selection), begin a move; **locked → refused in words** | — (interaction = Move) |
| `DragStart` | **wire body** | select the wire; the note names the ends as the re-patch handles | — |
| `DragStart` | **wire end** (grab handle, §3b) | detach that end; it follows the finger, ports glow/dim by verdict | — (interaction = Repatch) |
| `DragStart` | **inspector slider row** (§4c) | set the value at the grab x, begin a continuous edit | `SetParam` (one entry per drag) |
| `DragStart` | **empty** | begin a rubber-band marquee | — (interaction = Marquee) |
| `DragUpdate` | — | live move (÷ zoom) / wire + re-patch cursor with port magnet / slider value follows x / marquee grow; **second finger = ×10 fine** (scale rides in the intent) | — |
| `DragEnd` | **port** | normalise out→in, run the verdict (§3): connect, replace, insert adapter, or refuse in words | `AddWire` / `Batch` |
| `DragEnd` | **port** (re-patch) | verdict on the post-removal graph (§3b): the wire MOVES, or the refusal restores it exactly | `Batch[RemoveWire, …]` |
| `DragEnd` | **empty** (wire / re-patch) | cancelled — *stated*, never silent; the re-patched end snaps back | Note |
| `DragEnd` | — | commit the move snapped to the 8 px grid / the slider's final value / the marquee selection | `MoveNode`(s) / `SetParam` |
| `Activate` | node / wire / empty | select / select / deselect + close menu | Selection |
| `Activate` | open-menu row | run that row's action (§4) | per action |
| `Activate` | **browser row** (§4b) | spawn that module at the press, selected; the sheet closes | `AddNode` |
| `Activate` | **inspector slider row** | tap-to-set: the value jumps to the tapped x | `SetParam` |
| `DoubleTap` | canvas | **zoom to fit** the graph | camera |
| `Context` (long press) | node / wire / empty | open the context menu (§4) | Menu |
| `Pan` (two-finger) | canvas | pan the camera | camera |
| `Zoom` (pinch/spread) | canvas | zoom about the pinch centre (token clamp 0.25…4.0), LOD follows | camera |
| `Undo` (three-finger tap) | global | pop the undo stack, apply the inverse | Undo |

`Rotate`, `Flick`, `DragFineChanged`, `TogglePanel`, `AllSoundOff`, `RecoveryMenu` are **not** canvas
concerns in increment 1 — the canvas declines them and the shell owns them (rotate is reserved for
the rig map, flick for Perform scenes).

## 3. Connecting: the verdict is the contract's, not the canvas's

A wire drop calls `sparq-ui::canvas::connect::resolve`, which delegates the type/range decision to
`sparq-module-api`'s own `connect_audio` / `connect_cv` / `connect_cross` — the compiled form of
`docs/api/compat-matrix.toml`. **The matrix keeps its single copy** (the discipline WO-007 shipped);
the canvas adds only what the pure matrix does not own:

1. **Direction** — a wire runs out → in. Dragging from an input is allowed; the pair is normalised
   on drop. in→in / out→out is refused in words.
2. **Duplicate / cycle** — an identical wire is refused; a wire that would close a cycle is refused
   (§5.4 keeps the graph acyclic — the only legal cycle-closers are delay edges, and no delay module
   ships yet, so the refusal names that).
3. **Verdict** — `Compatible` connects; `Conversion` (multi→mono sum) connects and draws a **`SUM`
   word-badge** (the one documented silent conversion, flagged in words not colour); `Adapter(a)`
   connects *only if* `a` is installed (see #58 below), else refuses naming the missing module;
   `Refused` / `Undecided` refuse with the matrix reason.
4. **Fan-in** — a `single` input that is already fed gets its wire **replaced atomically** (one undo
   restores the old wire); `multi_in` accepts the extra wire. Fan-out from an output is always free.

**Defect #58, made structural:** the canvas may never offer a converter that does not exist.
`ConnectContext::adapter_spec` returns a spec *only* for a module that is really built; increment 1
ships no adapter modules, so every "adapter needed" verdict degrades to a refusal that names the
missing module. The one-tap-insert mechanism is built and unit-tested against a synthetic spec, and
lights up the moment `util/range` etc. exist.

**Affordances while dragging** (`connect::preview`, non-mutating): compatible / conversion /
installed-adapter ports **glow** (the class's token `_GLOW`), everything else **dims** (`_DIM`). The
source port is armed (filled + ringed); the cursor snaps to a hovered port (24 px capture magnet).

## 3b. Wire-end re-patch (increment 3)

Every wire carries two **grab handles** — small rings drawn on the wire exactly where the layout
says they are (`WireLayout::grab_from/grab_to`, 32 screen px along the bézier from each port, clear
of the port's own 24 px capture so "new wire from the port" and "move this wire's end" are two
distinguishable touches; at Dot LOD neither is drawn nor targetable). Dragging a handle detaches
that end:

1. The detached end follows the finger; the **fixed end stays home**; every other port glows or
   dims by `connect::preview` against the moving end (same affordance language as a fresh wire).
2. On drop, the old wire is removed **first**, then `resolve` judges the new pair — so the cycle
   check, the duplicate check and the single-input replacement rule all see the world the re-patch
   would create, not the one it is leaving.
3. Success stores `Batch[RemoveWire(old), <connect ops>]` — **one** three-finger tap restores the
   original wire, id included. A refusal (or a drop on empty canvas, or a cancel) re-applies the
   removal's inverse: the drag was a question, and "no" changes nothing.
4. Dropping the source end on an input (or the destination end on an output) is refused in words
   naming which end lives where. Dropping back on the same port is a note, not an op.

## 4. Context menu (long press)

Rows are 44 px (`touch.row_height_list`, class S), overlay ground, anchored at the press and clamped
into view. Only enabled rows are touch targets, so only they are audited (the shell's convention: a
disabled control is drawn honestly and kept out of the audit).

- **Node:** DUPLICATE · SET MASTER · BYPASS on/off · MUTE on/off · LOCK on/off · DELETE (shown
  `DELETE (LOCKED)`, disabled, when locked — a locked node resists move *and* delete).
- **Wire:** DELETE WIRE.
- **Empty:** ADD MODULE (disabled, `ADD MODULE (NONE INSTALLED)`, when the shell supplied no
  catalogue — #58 reaches the menu too) · SELECT ALL · ZOOM TO FIT · RENDER WAV · REDO (enabled
  only when there is something to redo).

Flag states are also badged on the node header in **words** (BYPASS / MUTE / LOCK), never colour
alone (look-board rule).

**RENDER WAV** (increment 2) asks the shell to render the drawn patch through the registry +
executor to `canvas-render.wav` — no audio device involved; the evidence line (master, blocks,
hash, budget) lands in the shell log, and every refusal arrives in words. The node the render
carries is the **master**: SET MASTER designates it explicitly, otherwise the documented default
rule applies (`CanvasState::resolve_master`: highest-id node with an audio output and at least one
wire, preferring a terminus — unwired spares can never win). The resolved master wears a MASTER
badge in the header, in words, so the answer is visible before the press.

## 4b. Module browser (increment 3)

**ADD MODULE** opens the browser sheet at the press: a query header and ranked rows (56 px,
class M), clamped into the canvas view, modal over the canvas like the menu. The catalogue is
supplied by the shell **from the registry** (`bridge::browser_catalog`), so a module that is not
installed cannot be offered — #58's rule, structurally, at the third consumer.

* **Fuzzy search** (`sparq-ui::canvas::browser::fuzzy_score`): case-insensitive in-order
  subsequence; contiguity outranks word starts, word starts outrank earliness; the display name
  outranks the module id, the id outranks the category. Deterministic: same catalogue + query ⇒
  same ranking, which is what makes the rows auditable and the smoke goldens stable.
* **Typing** filters (window shell: characters pipe from egui input events while the sheet is
  modal — a provisional feed, no widget owns input behind the recogniser's back; headless drivers
  call `browser_set_query`). Arrows move the selection, **Enter spawns it, Escape closes**.
* **Tap a row** → the module spawns at the press point (grid-snapped, cascading off an occupied
  spot), selected, sheet closed — one tap is one module, then you are wiring it. A no-match query
  keeps the sheet open and says **NO MATCH — CLEAR THE SEARCH**: the empty list is an answer, and
  its row slot is tappable so the refusal arrives in words instead of silence.

## 4c. Inspector params (increment 3)

Exactly one selected node + the panel open ⇒ the inspector computes its rows
(`sparq-ui::canvas::inspector`): title, then one 44 px row per manifest param — name, slider track,
value with unit. The drawn track is thin; the **touch target is the full row** (the port-capture
trick applied to sliders). Tap sets the value at the tapped x; drag edits continuously.

* Values clamp to `[min, max]` in the **model** (`Graph::op_set_param`), `int` snaps to whole
  steps, `bool` snaps to poles — one rule for every edit path, and history never holds an
  out-of-range value.
* One drag is **one** `SetParam` history entry: updates inside the drag coalesce, preserving the
  entry's original `from`, so one undo restores the value the finger *found* (the acceptance
  criterion's "undo restores graph **and param state**", proven by smoke 12).
* `enum`/`text`/`blob` params are shown greyed with the kind named and refuse edits **in words**
  until manifest v1 grows `options[]` — drawn honestly, never silently dead.
* Rows below the panel bottom are clipped and **not touchable** — an invisible control cannot be
  hit by accident (scrolling is parked in `LATER.md`).
* The bridge renders **node param state**; a never-touched node renders at its manifest defaults,
  so untouched patches are bit-identical to increment 2 (the goldens prove it).

## 5. Level of detail, snapping, and the audit

- **LOD** (from `camera.zoom`, tokens `lod_*`): **Full** ≥ 0.6 — header text, port labels (name +
  class letter), port rings; **Simplified** 0.35…0.6 — node box + coloured ports, no text, major
  grid only; **Dot** < 0.35 — a colour-coded dot per node, hairline wires, no grid, ports not
  targetable (a 24 px capture around an invisible dot would only mis-wire).
- **Snapping:** a committed move lands on the 8 px grid (`canvas.snap`). Live drag is unsnapped; the
  snap happens on `DragEnd`, so the op's `from`/`to` are both grid points and undo is exact.
- **Touch-target audit:** node bodies register as **class L (72 px)** *only when their on-screen
  shorter side actually clears 72 px* — below that (zoomed out) a node is navigated or
  marquee-selected, not individually touched, so it is not offered as a discrete target. Port
  capture circles register as **class S (44 px)** whenever ports are targetable; the capture radius
  is zoom-invariant (24 px screen), so it always clears 44. `sparq ui --audit` measures both.

## 6. Signal class → colour → redundant encoding

One accent per signal class (`colors.toml`), each with a non-colour encoding so nothing is carried
by colour alone:

| Class | Colour | Wire stroke (`_ENCODING`) | Port letter (`_LABEL`) |
|---|---|---|---|
| audio | amber | solid, signal width | **A** |
| cv | cyan | solid, thin | **C** |
| event | magenta | dashed 6-3 | **E** |
| data | green | dotted 2-4 | **D** |
| spatial (audio + spatial set) | violet | double stroke | **S** |
| gpu / atom | neutral text | solid thin | — |

The painter reads the encoding *strings* from the tokens and parses the dash numbers out of them —
no stroke pattern is hard-coded in widget code (`token_audit.py` R6).

## 7. Device-side acceptance (honest list)

Proven headless in CI (sandbox, no GPU, no digitiser): the whole intent→op table above, via the
`--audit` smokes and 31 canvas unit tests. **Needs SATURN, and cannot be proven over RDP-from-iPad:**

- 60 fps with a 200-node graph while panning/zooming (LOD engaging) — WARP over RDP is not the stage
  GPU; the frame-time histogram is a device measurement.
- Real palm rejection while drawing a wire — iPadOS filters the palm before Windows sees it, so an
  RDP session cannot exercise sparq's own `contact_area` path; that needs **WM_POINTER** on the stage
  digitiser (increment 2, `LATER.md`).
- The full DPI matrix and mixed-DPI dual monitor — RDP presents one virtual display.

Over RDP you *can* and should touch-test the gesture table (drag, connect, refuse, undo, menu,
pan/pinch, double-tap-fit) and the visual language; those are logic + rendering facts that survive
the remote session.
