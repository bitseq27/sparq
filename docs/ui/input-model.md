# sparq input model — gestures, touch classes, and the toolkit-independent API

**Status:** implemented — increment 1 of WO-012 (2026-09-21). The recogniser, pointer model,
shell layout and audit below are code in `sparq-ui` (`gesture.rs`, `pointer.rs`, `shell.rs`,
`audit.rs` — zero dependencies, 37 tests) consumed by the egui shell in `sparq-app/src/ui/`
(`--features ui` headless / `ui-window` on screen). §3's table maps 1:1 onto `GestureIntent`;
every threshold in §3/§4 is a `layout.toml` token via `GestureConfig::default()` and
`TouchClass::min_px()`. Device-side acceptance (real finger, real DPI matrix, palm rejection with
a real contact area — winit reports none, increment 2 goes to WM_POINTER) is tracked in
`docs/ui/windows-dpi-notes.md` §4 and the WO-012 build-log entry.
**Related:** ADR-003, `design/tokens/layout.toml [touch]`, `design/look-board.md`
**Why this exists:** the renderer will be swapped in Phase 6 (egui → custom `wgpu` shell). The **gesture model must not be swapped with it.** Everything here is toolkit-independent; egui is a consumer in Phase 0–2, the custom shell is a consumer from Phase 6.

---

## 1. Inputs that exist

| Class | Sources | Notes |
|---|---|---|
| **Pointer** | finger, pen, mouse | normalised into one model (§2). A finger and a mouse are the same *pointer* with different capabilities |
| **Multi-pointer** | 2–10 simultaneous contacts | pan, pinch, fine-resolution, multi-finger gestures |
| **Keyboard** | physical + on-screen | parity for every action; the only fast path in Design mode |
| **Controller** | MIDI 2.0, OSC, HID, serial, BLE | never hard-wired to a module — always through a Controller Map (plan §7.4) |
| **Data stream** | any `data` channel | can drive any parameter; a stream is an input device |

Rule: **a gesture produces an intent, not an effect.** Intents are dispatched to the focused surface, which decides the effect. This is what makes the same gesture work in Design, Perform and Install modes with different results.

## 2. Pointer normalisation

A pointer is `{ id, position, pressure, contact_area, kind (finger|pen|mouse), phase (down|moved|up|cancel), t }`.

* Mouse hover is *synthesised* as a zero-pressure pointer that never enters `down` — so hover affordances are an **optional enhancement**, never a requirement (pillar 6).
* Pen pressure and tilt map to parameter value where a surface declares `pressure_sensitive`.
* Palm rejection: contacts with `contact_area > touch.gesture.palm_reject_max_contact_area_mm2` are ignored for gestures but still recorded in diagnostics (so you can see *why* a touch did nothing on stage).
* Coordinate space is logical px at the current DPI scale; the shell owns the DPI transform (per-monitor, Windows: see `docs/hardware/stage-baseline.md` §6).

## 3. Gesture recognisers

| Gesture | Recognition | Intent emitted | Notes |
|---|---|---|---|
| **Tap** | down→up < 300 ms, movement < 8 px, single pointer | `activate(target)` | the universal "do it" |
| **Double tap** | two taps < `double_tap_ms` (300) | `zoom_to_fit` on canvas; `toggle_expand` elsewhere | |
| **Long press** | down held ≥ `long_press_ms` (350), movement < 8 px | `context(target)` | **replaces right-click and hover menus entirely** |
| **Drag** | down + movement ≥ 8 px | `drag(target, delta)` | also: move node, draw wire, draw envelope, sketch wavetable, draw trajectory |
| **Drag + second finger** | second pointer down during a drag | `drag(..., scale = 1/fine_resolution_factor)` | ×10 precision — the answer to "touch can't be precise" |
| **Pan** | 2 pointers, similar direction | `pan(delta)` | |
| **Pinch / spread** | 2 pointers, opposing radial motion, span ≥ 40 px | `zoom(factor, centre)` | |
| **Rotate** | 2 pointers, tangential motion | `rotate(delta)` | reserved; used by the rig map and geometry displays |
| **3-finger tap** | 3 pointers, < 200 ms | `undo` | global |
| **3-finger swipe down** | | `panic` (all sound off) | global, Perform mode |
| **5-finger hold** | 5 pointers held ≥ 800 ms | `recovery_menu` | global |
| **Swipe (1 finger, fast)** | velocity > 1.5 px/ms, released | `flick(target)` | scene morph in Perform mode |
| **Edge swipe** | from a screen edge inward | `toggle_panel(side)` | Design mode only |

**Conflict resolution:** longest-press wins over drag when movement is under threshold; a recogniser that has already fired locks out siblings until all pointers lift; global gestures (3-finger, 5-finger) always win over surface gestures. Every suppression is logged in the input diagnostics view — an input that "did nothing" must always be explainable.

## 4. Touch classes and the audit

Every interactive element declares a class; the shell refuses to render it below its minimum, and `token_audit`/the layout audit fail the build.

| Class | Min size | Used for |
|---|---|---|
| `S` | 44 × 44 px | Design-mode dense controls, list rows, port capture |
| `M` | 56 × 56 px | inspector sliders, chips, tab items |
| `L` | 72 × 72 px | canvas node bodies, browser tiles, scene cells |
| `XL` | 96 × 96 px (typical 120–160) | Perform-mode macros, transport, panic |

Additional hard numbers: port drawn radius 6 px with a **24 px capture radius**; wire hit width 20 px; list row height 44 px (browser 56 px).

**Perform-mode rule:** nothing below class `L` is interactive. Design-mode chrome is not merely hidden — it is not hit-testable.

## 5. Keyboard and controller parity

* Every intent has a keyboard binding and can be bound to a controller source. Bindings live in the **Controller Map asset** (`.sparqmap`), never in module code.
* **Map mode:** press *Map* → touch a target → wiggle a source → binding created with auto-detected range. Works for MIDI/OSC/HID/sensor/data identically.
* **Blind mode** (plan §12.4): labels hidden, numerals only — the keyboard/controller bindings are what you actually play with.
* A binding may be `momentary`, `latching`, `toggle`, or `relative` (encoder-style with acceleration).

## 6. Haptics and feedback

Where the device supports it: a detent tick when crossing a parameter's centre or a scale degree; a confirmation pulse on latch; a distinct double pulse on panic. Every haptic has a **visual equivalent** — haptics are redundant, never load-bearing.

## 7. Accessibility requirements (non-negotiable)

1. Type scale to **200 %** with no broken layout (token: `typography.toml [zoom]`).
2. High-contrast theme generated from tokens (`colors.toml [theme.contrast-high]`).
3. **No state encoded by colour alone** — pattern, dash, shape or label always present (`look-board.md` §4).
4. Full keyboard navigation with a visible focus ring (2 px, `text.primary`).
5. Reduce-motion and reduce-glow switches; traces and meters always animate (they are data).
6. Colour maps have `.cb` alternatives for every default.
7. Nothing requires hover, a modifier key, a right-click, or simultaneous three-hand operation.
8. Every error is stated in words with a remedy, not just a colour change.

## 8. What Phase 0 must prove (WO-012/WO-013 acceptance)

* [ ] Per-monitor DPI correctness at 100/125/150/200 % and on a mixed-DPI dual-monitor setup — touch coordinates map exactly.
* [ ] Long press (350 ms) never fires a drag, and a drag never fires a long press.
* [ ] Second-finger fine resolution is discoverable (an on-screen hint on first use) and actually usable one-handed.
* [ ] A 12-module patch is buildable by touch in < 3 minutes by a first-time user.
* [ ] Palm resting on the screen while drawing a wire causes zero spurious input.
* [ ] Every interactive element measures ≥ its declared class minimum (automated audit, not eyeballing).
* [ ] 60 fps sustained with the full shell + a 50-node canvas on the stage device; frame-time histogram visible in diagnostics.
* [ ] **Escalation trigger (ADR-003):** if egui cannot deliver any of the above by end of week 5, the custom shell is pulled forward and ADR-003 is superseded with evidence.
