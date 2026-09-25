# Windows DPI notes — the shell's coordinate contract (WO-012 task 1)

**Status:** increment 1 — the code contract is implemented and headless-proven; the device matrix below is **awaiting SATURN measurements**. Nothing in the "measured" column may be filled in from the sandbox.
**Related:** `docs/ui/input-model.md` §2, `crates/sparq-app/src/ui/window.rs`, `crates/sparq-ui/src/shell.rs`.

---

## 1. The rule: one transform, one space

Physical pixels exist in exactly one place: the winit boundary in `window.rs`. Everything
downstream — gesture recogniser, shell layout, touch audit, widget code — lives in **logical px**
and never sees a scale factor. The transform is `logical = physical / scale_factor`, applied per
event (`to_logical`), with `scale_factor` re-read from the window (it changes when the window
moves to a monitor with different DPI).

Why this shape: the per-monitor-DPI trap on Windows is not the math, it is *which* of the nine
combinations of (physical, logical, DPI-unaware) each API speaks. winit normalises the window
side (physical sizes + a scale factor per monitor); egui normalises the drawing side
(`pixels_per_point`); if the input path applies its own transform anywhere else, touch lands
offset from visuals — the classic symptom this WO exists to prevent.

## 2. What winit does on Windows (verified by reading its source, not by running it)

* Sets **Per-Monitor V2** DPI awareness for the process at startup (`SetProcessDpiAwarenessContext`
  with fallbacks to Per-Monitor v1 / System aware on older builds). No manifest editing needed —
  but a manifest claiming a *different* awareness would fight it; the project ships no manifest.
* Delivers `ScaleFactorChanged` when the window crosses monitors, then `Resized` in physical px.
  Both are handled: the painter reconfigures the surface, egui-winit picks up the new factor, and
  — because everything downstream is logical — **no other code changes at all**.
* `Touch.location` is physical px (divided once at the boundary); `CursorMoved.position` likewise.

## 3. What the headless gate proves about DPI (and what it can't)

`just ui-audit` runs the matrix **5 viewports × 4 scales (100/125/150/200 %) × 2 modes** and
asserts *DPI invariance*: at every scale the layout rects, element count, violations and dense
badges are byte-identical, because the model only ever sees logical px. That is the machine-
checkable half of the acceptance criterion.

It cannot prove the other half: that winit's physical→logical division matches what the monitor
actually does — mixed-DPI multi-monitor, fractional scales on the digitiser, RDP's synthetic
display. Those are device facts, recorded below.

## 4. Device acceptance matrix — **to be measured on SATURN (and the stage device)**

Run `scripts\ui.bat`, then for each row: move/resize as described, tap the named target, and
record. A row fails if the tap does not hit the visual under the finger, or if panels change
size when only the monitor changed.

| # | Setup | Scale | Check | Result (fill in) |
|---|---|---|---|---|
| 1 | Primary monitor | 100 % | tap every rail button; tap dock collapse | |
| 2 | Primary monitor | 125 % | same | |
| 3 | Primary monitor | 150 % | same | |
| 4 | Primary monitor | 200 % | same | |
| 5 | Mixed-DPI dual monitor | e.g. 150 % + 100 % | drag window across; taps land on visuals on **both**; no resize-on-cross | |
| 6 | RDP session | whatever the client negotiates | window opens (WARP), mouse taps land | |
| 7 | Touch device (stage tablet) | native | finger taps, two-finger pan/pinch, long-press context | |
| 8 | Any of the above | — | window resize keeps the 8 px grid and the 60 % canvas floor (visible: no panel overlap, dock/inspector snap) | |

Also record for each row: `sparq ui --headless 60 --scale <S>` frame stats, and — on the physical
machine — whether the window *feels* 60 fps (the real fps measurement belongs to the WO-012
acceptance run with the 50-node canvas, WO-013).

## 5. Known limitations (stated, not hidden)

* **Contact area**: winit's `Touch` event carries no area, so palm rejection never triggers from
  real touches yet (it is proven with synthetic areas in the recogniser tests). Increment 2:
  WM_POINTER / `GetPointerFrameInfo` via a thin `windows-sys` path in `window.rs` (same pattern
  as the HAL's hand-written COM: allowlist entry, `SAFETY:` comment).
* **Pen tilt/rotation**: not surfaced by winit; pressure is (when the digitiser reports it).
* **High-DPI font metrics**: egui rasterises at `pixels_per_point`; the typeface is still the
  bundled default (WO-002 `chosen = ""` open item) — sizes are token-correct, the face is not final.
* **Continuous repaint**: the prototype requests redraw every frame (simple, honest, measurable).
  Damage-driven repaint (`FullOutput.platform_output` repaint requests) is a battery/CPU
  optimisation for increment 2, not a correctness issue.
