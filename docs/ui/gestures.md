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
| `Activate` | **rename sheet** (§4d) | inside: a note — the keyboard owns the buffer; outside: cancel, stated | `Rename` / Note |
| `Activate` | **inspector slider row** | tap-to-set: the value jumps to the tapped x | `SetParam` |
| `DoubleTap` | canvas | **zoom to fit** the graph | camera |
| `Context` (long press) | node / wire / empty | open the context menu (§4); an open rename sheet cancels first | Menu |
| `Pan` (two-finger) | canvas | pan the camera | camera |
| `Pan` (two-finger) | **inspector panel** (§4c) | scroll the rows (content follows the fingers); the camera stays put; the panel speaks only at the ends | scroll |
| `Zoom` (pinch/spread) | canvas | zoom about the pinch centre (token clamp 0.25…4.0), LOD follows | camera |
| `Zoom` (pinch/spread) | **inspector panel** | declined — the panel owns the gesture and has no zoom; the canvas behind it must not move | — |
| `Zoom` (**mouse wheel**, §2b) | canvas | zoom about the cursor — the wheel is the pinch's mouse hand | camera |
| `Pan` (**mouse wheel**, §2b) | inspector / library | scroll the rows / cards (the wheel over a scrollable panel stays that panel's scroll) | scroll |
| `Pan` (**right-drag**, §2b) | canvas | pan the camera — the desktop hand's two-finger pan | camera |
| `Context` (**right press, no travel**, §2b) | node / wire / empty | the long-press menu, without the hold | Menu |
| `Delete` (**DEL key**, §2b) | the selection | delete selected wires, then selected nodes; locked nodes and the permanent Main Out refuse in words | `RemoveWire`/`RemoveNode` (one entry each) |
| `Undo` (three-finger tap) | global | pop the undo stack, apply the inverse | Undo |

`Rotate`, `Flick`, `DragFineChanged`, `TogglePanel`, `AllSoundOff`, `RecoveryMenu` are **not** canvas
concerns in increment 1 — the canvas declines them and the shell owns them (rotate is reserved for
the rig map, flick for Perform scenes).

## 2b. The mouse hand (operator ruling 2026-10-01)

The mouse rides the SAME intent vocabulary as the finger — the window adapter only translates
hardware into intents; nothing downstream knows which hand asked. The ruling's four bindings:

* **Scrollwheel = canvas zoom**, about the cursor (`Zoom`, one notch ≈ ×1.15, pixel deltas scaled
  and clamped). Over the inspector or the library the wheel stays that panel's **scroll** (`Pan`),
  because a gesture over a surface belongs to that surface; over the remaining chrome it is
  nothing at all. A two-finger **pinch** over a panel is still declined (its sequential-contact
  span wobble must not move camera *or* scroll — the wheel is the panel's scroll door for the
  mouse hand, exactly as the two-finger pan is for the touch hand).
* **Right-drag = canvas pan** (`Pan` from the cursor deltas): the desktop hand's two-finger pan,
  and it works over nodes too (the camera, not the node, moves). A right press that **never
  travels** past the drag threshold (8 px — the recogniser's own word for "drag") is the context
  menu on release, unchanged: the desktop hand's long-press.
* **DEL = delete the selection** (`Delete`, host-synthesised; the recogniser never fires it from
  pointers): wires first, then nodes, each through the same op door the long-press menu's DELETE
  rides. Protections speak: a **locked** node refuses and stays; the **permanent Main Out**
  refuses and stays (§4e); an empty selection is a refusal with the remedy; while the rename
  sheet or a menu/browser is open the key is answered in words instead of deleting behind the
  sheet.
* **Binary settings are toggle buttons, never sliders** (§4c): a `bool`, or an `int` whose whole
  domain is `[0, 1]` (the manifests' Mute / Mode shapes), draws a switch; a tap FLIPS it wherever
  it lands on the row, and a drag flips once and drags no further — one undo step per flip.

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

## 3c. Ports float beside the window; covered is untouchable (operator ruling 2026-10-01)

Connection points are drawn **6 px outside the card edge** (`canvas.node_port_offset`; widened
from 4 in operator round 4, D10 — a port must read as its own furniture at arm's length) — inputs
left of the body, outputs right — so a port is never half-buried in the window it belongs to, and
a wire visibly lands *beside* the card. The flip side of floating ports is that one card can sit
on another card's ports, so the hit-test carries the drawing order's own rule: **nodes draw in
list order, later cards on top, and what you cannot see you cannot touch** — a port (or a wire's
re-patch grab, §3b) that lies under a LATER card's body is not targetable at that point: the
click lands on the covering card. The same test applies to the port's own circle, so a buried
port stays buried for the magnet too; node bodies and param rows already resolved top-down (the
reverse-iteration rule), and this closes the last hole in "the pixel you see is the pixel you
get".

## 3d. The permanent Main Out (operator ruling 2026-10-01)
`out/main` is **not a module the user creates or deletes**: the shell's patch always starts with
it on the canvas (the demo patch seeds it), and every door
that would create another — the library tiles, the browser sheet, the dock, DUPLICATE, spawn by
id — refuses in words ("Main Out is permanent — it is already on the canvas…"). Every door that
would remove it — DELETE from the menu, the DEL key, a select-all sweep — refuses likewise ("…it
is the listener's output and cannot be deleted"). It is the one node whose presence is a contract
with the listener, not a choice on the canvas. Because it is permanent, it is also the shell's
**driver window**: an info band under its well (§4e) reads the live session's negotiated truth.

## 3e. Control wires: every float setting is an input (operator ruling 2026-10-01, round 3)

While a drag from a **cv output** is in flight, the hovered module wears a **small blue dot
beside every float parameter** — half the radius of the main in/out ports, in the ports' own
column at the row's height. They exist only while the control drag is in flight (at rest they
are not drawn, and what you cannot see you cannot touch); the dot under the cursor glows and
magnets the wire. Dropping on a dot makes a **control wire**: a dashed control-class run from
the cv source to the parameter, stored as a wire with a `param` destination, undoable like any
wire, and honoured by the executor — per block the parameter's effective value is
`clamp(knob + cv × (max−min)/2)`: the knob stays the bias, a full-scale cv sweeps half the
range either side, one block of latency (the latch is what keeps the plan independent of
topological order). The verdicts speak: only a **cv output** may modulate (audio would modulate
at a rate the block snapshot cannot carry), only a **float** parameter may be modulated (a
toggle or a menu is a decision, not a voltage), cycles are refused, and a parameter already
modulated gets its wire **replaced** atomically — one undo restores the old modulation. Control
wires are structural on the live ledger (a re-stage, like any wire), they are skipped by the
kernel graph (the modulation rides the executor's param-mod plan, not a port edge), and they are
never re-patched by their ends — delete and redraw is their door.

## 3f. The junction bus: `util/mult` (operator ruling 2026-10-01, round 3)

Six vertical dots, each an input **or** an output, the type set by the **first connection**:
the role and the class are derived from the wires (no second state to drift), and the rules are
refusals in words — a dot never flips role while a wire touches it; a bus never carries two
types; a bus never carries two sources ("every other dot copies it — sums are the mixer's
job"); cycles refused as ever. The dots wear their state: uncommitted a hollow neutral ring,
input a ring in the bus class, output a filled dot in it. `mult` is a **patching** module: its
process is a no-op and the bridge COLLAPSES it at build time — every output dot's wires become
direct kernel edges from the one source, so what the canvas draws is exactly what the executor
runs, and the copies light like the original (the levels follow the bus). The sequencer's
pattern row, meanwhile, is **sixteen step buttons** on the card (one strip) and in the
inspector (2×8, touch-sized): a tap toggles that step's bit through the param door, one undo
per flip; the clock input walks the ring as before.

## 3g. Deleting keeps the chain connected (operator ruling 2026-10-01, round 2)

Deleting a module used to leave its source dangling and its destinations silent. Now the removal
**splices**: the wire that fed each input is re-aimed at the wires its outputs fed, paired in port
order (first input's source → first output's destination, and so on), and every candidate splice
runs the SAME `connect::resolve` verdict a hand-drawn wire gets — type, range, cycle, single-input
replacement — so an incompatible pair is skipped, not forced. The removal and every accepted
splice ride ONE `Op::Batch` into history: one undo restores the node, its original wires, and
removes the splices — the deletion was one action, so it undoes as one. The shell says what it
did ("chain kept: N wire(s) spliced past the deleted node"). The protections (§3d, LOCK) run
first and refuse in words; a select-all DELETE sweep splices every deletable node the same way.

## 4. Context menu (long press)

Rows are 44 px (`touch.row_height_list`, class S), overlay ground, anchored at the press and clamped
into view. Only enabled rows are touch targets, so only they are audited (the shell's convention: a
disabled control is drawn honestly and kept out of the audit).

- **Node:** RENAME (§4d) · DUPLICATE · SET MASTER · BYPASS on/off · MUTE on/off · LOCK on/off ·
  DELETE (shown `DELETE (LOCKED)`, disabled, when locked — a locked node resists move *and*
  delete).
- **Wire:** DELETE WIRE.
- **Empty:** ADD MODULE (disabled, `ADD MODULE (NONE INSTALLED)`, when the shell supplied no
  catalogue — #58 reaches the menu too) · SELECT ALL · ZOOM TO FIT · RENDER WAV · REDO (enabled
  only when there is something to redo).

Flag states ride **patterns at every LOD** (increment 5, look-board §4 "pattern first — colour is
redundant"): BYPASS hatches the node body (the mockup's own `hatch8` encoding), MUTE dashes the
border, LOCK doubles it, MASTER chips the header (a filled square in the badge accent). At **Full**
LOD the words ride on top of the patterns (BYPASS / MUTE / LOCK / MASTER in the header) — redundant
encoding, never either alone. At **Simplified** the patterns stand alone, because the level's
contract is "no text"; at **Dot** the shapes carry it: hollow dot = bypassed, dimmed = muted,
concentric ring = locked, accent ring = master.

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
* **Frequency rides the log map (operator ruling 2026-10-01):** a param whose unit is `Hz` and
  whose minimum is strictly positive (`sparq/syn/sine`'s 0.1 Hz – 10 kHz, `flt/svf`'s cutoff)
  maps x → value **logarithmically** — equal mouse distances are equal pitch distances, which
  is what "the slider follows the mouse" means for the ear. `knob_x` is the same map inverted,
  so the knob you see is the value you have. Ranges that include zero stay linear (log(0) is
  not a mapping), and non-Hz params stay linear whatever their range.
* **Binary settings are toggle buttons (operator ruling 2026-10-01):** a `bool` row, or an `int`
  row whose whole domain is `[0, 1]` (the manifests' Mute / Mode shapes), draws a **switch** on
  the track instead of a slider — ON filled in the control accent with the knob right, OFF an
  empty well with the knob left — and its value column reads ON/OFF (the redundant word). A tap
  anywhere on the row FLIPS the value; a drag flips exactly once and starts no continuous edit;
  one flip is one `SetParam` history entry. A float on `[0, 1]` keeps its slider (it has values
  in between), and a wider int keeps its slider too.
* **Multi-choice settings are button rows (operator ruling 2026-10-01, round 2):** an `int` row
  whose domain is 3…8 discrete choices (`flt/svf` mode, `mod/lfo` shape, `dsp/scope` colour map,
  `mod/clk-div` multiply) draws **one button per choice**, the active one filled in the row's
  class colour with its number, the rest empty wells — a slider over a menu is a menu pretending
  to be a scale. A tap sets the choice under the finger; a drag steps across the buttons; the
  model's int snap keeps the value on a choice either way. Wider ints (steps, masks, bits) are
  numbers, not menus, and keep their sliders.
* One drag is **one** `SetParam` history entry: updates inside the drag coalesce, preserving the
  entry's original `from`, so one undo restores the value the finger *found* (the acceptance
  criterion's "undo restores graph **and param state**", proven by smoke 12).
* `enum`/`text`/`blob` params are shown greyed with the kind named and refuse edits **in words**
  until manifest v1 grows `options[]` — drawn honestly, never silently dead.
* **Scrolling (increment 5):** when the rows outrun the panel, a **two-finger drag inside the
  panel** scrolls them (one finger on a row is a slider edit — the panel's scroll rides the
  two-finger gesture, routed by the pan's CENTRE: `GestureIntent::Pan` carries it, like `Zoom`
  always has). The title row is a fixed header; rows slide under it; a hairline **thumb** on the
  panel's right edge shows the position — drawn only when the panel actually scrolls. The offset
  is transient view state (not undoable), clamped by the layout, and **resets when the inspected
  node changes**. The honesty rule survives the offset in both directions: a row clipped at the
  bottom is touchable through its visible sliver; a row under the header is refused by `row_at`
  and skipped by the painter — what you cannot see you cannot touch. The panel speaks only at the
  ends ("scrolled to the first/last row"); mid-scroll the moving rows are the feedback (the
  camera-pan precedent). A pinch over the panel is declined — the canvas behind it must not move.
* The bridge renders **node param state**; a never-touched node renders at its manifest defaults,
  so untouched patches are bit-identical to increment 2 (the goldens prove it).

## 4d. The rename sheet (increment 5)

**RENAME** opens the text-entry sheet at the press: a header naming the module, an inset well
carrying the buffer **pre-filled with the current title** and its end-caret, and a hint row that
states the contract — `ENTER COMMIT · ESC CANCEL · EMPTY = MODULE DEFAULT`. The sheet is the
deepest modal (it opens FROM the menu): taps inside are answered in words (the keyboard owns the
buffer), a tap outside **cancels, stated**, and louder gestures (long-press, drag) cancel it like
they dismiss the menu and browser.

* **Enter** commits through `Graph::op_rename` — an op like any other, so **one three-finger tap
  undoes the rename**. Committing an **empty** buffer clears the custom name (the module default
  is back); committing the **unchanged** buffer is a stated no-op that leaves history alone (an
  undo step that undoes nothing is a lie).
* The buffer is `sparq-ui::canvas::entry::TextEntry`: printables only, capped at
  **`ENTRY_MAX_CHARS` = 32** — the node header's budget, so a name you can type is a name the
  canvas can show. The caret is the end (v0, declared): insert appends, backspace pops.
* The keyboard path is the shell's **unified modal feed**: egui raw events are parsed ONCE into a
  normalised batch (text / backspace / arrows / Enter / Escape) that the rename sheet and the
  browser query both consume — the shared text-entry surface the browser's provisional feed was
  declared to be waiting for. Headless drivers call `rename_set_text` / `rename_commit` directly
  (the `browser_set_query` convention).

## 4e. The Main Out driver window (operator ruling 2026-10-01)

The permanent `out/main` (§3d) carries an **info band** between its meter well and its port row
(layout token `canvas.node_info_height`, reserved for this node alone). At rest it states the
at-rest fact in words — `NO SESSION - TRANSPORT'S PLAY OPENS THE DEVICE`. While a session runs it
reads the session's own negotiated truth, in two lines, every number with its unit:

* `backend · device` — the probe's names (`null (virtual device) · sparq Null Device …`,
  `WASAPI shared · …` on the stage machine);
* `rate Hz · channels ch · block fr · 32-bit float` — the negotiated config the executor was
  built for, plus the HAL's sample-format contract (interleaved f32).

Nothing is invented: the lines come from the live session (`LiveSession::driver_lines`), and a
stopped session hands the painter `None`, which is the at-rest words. The workspace's right-edge
IN/OUT master strip is **gone** with the same ruling — the master's meters live on the master's
own card (its well), and the driver truth lives beside them; a strip of bars floating over the
canvas was chrome pretending to be a reading.

## 4f. The cable node (operator round 4, D15)

A wire is itself an edit surface. Every wire carries an optional **trim** (`WireTrim { amp,
offset }` in the model, `Op::SetTrim` in the ledger — one row per change, structural: a live
engine re-stages at the block boundary, D1's rule, the command ring does not carry it).

* **Hover / light press over a clean wire** shows the painter's **ghost dot** at the arc
  midpoint; its capture is the same 24 px zoom-invariant ring as a port's.
* **Tap the ghost → insert** at identity (`amp = 1`, `offset = 0`). The bridge synthesises and
  calls nothing for an identity trim, so the render stays **bit-identical** — the node is
  furniture you see, not sound you hear (audit smoke 54 pins all three legs: insert, edit,
  removal). The insert's log note teaches the vocabulary the handle's tap relies on.
* **Drag the node**: up/down = amp (0…2), left/right = offset (−1…+1), 100 px per unit on both
  axes — the raw delta accumulates, the axes ARE the scaling. On an **audio** wire the
  horizontal axis is **inert by rule**: DC never enters the audio path (the offset is never
  read, not merely clamped). Every update coalesces into **one history entry keeping the
  drag's original `from`**: one three-finger tap undoes the whole drag back to the value the
  finger found, not its first waypoint.
* **Tap the node → remove**: the same `SetTrim` op to `None`, one undo step, wire clean again.
* **Hit rank** (Full/Simplified LOD): port > cable-node handle > wire-end grab > card body >
  wire. The handle outranks the re-patch grabs because it is DRAWN furniture while the grabs
  are invisible until you know them; it obeys the same visibility discipline — never under a
  card body, never at Dot LOD (nothing of the wire draws to scale there). Only a wire that
  already CARRIES a trim offers the handle; the clean wire's insert door is the wire tap.
* **The voice is the bridge's** (build-time, verdicts in words): an audio trim becomes an
  invisible synthesised `util/gain`; a cv trim rides the executor's own `set_cv_trim` door
  after the build; a control (param-mod) trim composes into the mod formula (identity =
  `(1.0, 0.0)`); a `util/mult` collapse composes its feed's and copy's trims affinely into ONE
  trim per collapsed chain. **Event and data wires refuse in words** (a trigger's word is its
  sample, a data stream's its payload — no amplitude to trim) and **spatial refuses in words**
  (no per-set gain module exists yet); the refusal surfaces in the shell log with the remedy
  (tap the node to remove it), never as a silent no-op.

The quantizer's **keyboard well** (round 4, D11) is the other new touch surface: twelve equal
key cells on the `util/quant` card, a tap flips scale membership — but only in **Custom** mode;
under a preset scale the tap refuses in words and names the remedy (switch the scale param to
Custom first). The membership fill, the passing-pitch light and the key taps all read the same
`custom-mask` the module reads: the display is the module.

## 5. Level of detail, snapping, and the audit

- **LOD** (from `camera.zoom`, tokens `lod_*`): **Full** ≥ 0.6 — header text, port labels (name +
  class letter), port rings, flag words over their patterns; **Simplified** 0.35…0.6 — node box +
  coloured ports, **no text at all** (flags ride the §4 patterns, the master its chip), major grid
  only; **Dot** < 0.35 — a colour-coded dot per node (state by dot SHAPE, §4), **hairline wires**
  at last (increment 5 made the rendering match the contract this file always stated), no grid, no
  grab handles, the SUM conversion word suppressed (unreadable at that scale by definition — the
  class encoding survives and the word returns on zoom-in; declared, not silently dropped), ports
  not targetable (a 24 px capture around an invisible dot would only mis-wire). The side-by-side
  review against `design-mode.svg` stays a device acceptance box (test006).
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
| event | magenta | solid, signal width (round 4, D9 — event cables are solid; the control-wire dash below is a different encoding and stays) | **E** |
| data | green | dotted 2-4 | **D** |
| spatial (audio + spatial set) | violet | double stroke | **S** |
| gpu / atom | neutral text | solid thin | — |

The painter reads the encoding *strings* from the tokens and parses the dash numbers out of them —
no stroke pattern is hard-coded in widget code (`token_audit.py` R6).

## 7. Device-side acceptance (honest list)

Proven headless in CI (sandbox, no GPU, no digitiser): the whole intent→op table above, via the
`--audit` smokes (22 gesture cells as of increment 5) and the canvas unit tests. **Needs SATURN, and cannot be proven over RDP-from-iPad:**

- 60 fps with a 200-node graph while panning/zooming (LOD engaging) — WARP over RDP is not the stage
  GPU; the frame-time histogram is a device measurement.
- Real palm rejection while drawing a wire — iPadOS filters the palm before Windows sees it, so an
  RDP session cannot exercise sparq's own `contact_area` path; that needs **WM_POINTER** on the stage
  digitiser (increment 2, `LATER.md`).
- The full DPI matrix and mixed-DPI dual monitor — RDP presents one virtual display.

Over RDP you *can* and should touch-test the gesture table (drag, connect, refuse, undo, menu,
pan/pinch, double-tap-fit, the rename sheet, the inspector's two-finger scroll) and the visual
language; those are logic + rendering facts that survive the remote session.

**Increment 5 added to this list** (`test006.bat` steps F–I): the rename sheet under real keys;
the inspector scroll under real two fingers (and the camera staying put behind it); the LOD walk
against `design-mode.svg` — Simplified shows no text with the flag patterns readable, Dot shows
hairline wires and shape-encoded dots; and a cv wire (`rms.level → svf.cutoff-mod`) lighting from
its own published value after RENDER WAV while the rms node's folded audio meter stays at rest.

**Round 4 added to this list** (`test006.bat` steps L–U): the clip LED on a genuinely hot render
(red cap + the CLIP word on the master's info band, latched while playing, cleared by STOP); the
clock's four division rings turning under PLAY and standing still at rest; the seq's walking
lights (the cursor cell over the pattern fill) and the rand card's step bars with the cursor bar
lit — same seed, same bars, the display is the module; the quantizer keyboard lit from its own
mask, a Custom-mode tap flipping a key, a preset-scale tap refusing in words; the cable node
under a real finger — hover ghost, tap-insert (silent: identity renders bit-identical), drag to
hear the amp, the audio wire's inert horizontal axis, tap-to-remove, one undo per gesture; the
`util/mult` strip at its quarter width with the centred dot column; `util/vca` accepting the lfo
wire and the tremolo it makes; `fx/fold` blooming above zero and acting as a wire at zero; the
rms card's rolling level graph beside its floor bar; and the round-4 chrome — event cables solid
magenta (D9; control wires stay dashed cyan, a different encoding), ports floating 6 px beside
the card edge (D10), and the svf's response curve moving under its cutoff-mod cv.
