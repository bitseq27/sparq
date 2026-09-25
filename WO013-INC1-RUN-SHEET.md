# WO-013 increment 1 — SATURN run sheet (graph canvas)

Apply, verify headless, then touch-test the canvas over RDP. Same shape as the P1 run sheets.
Scratch companion to the build log in `PHASE0-WORKORDERS.md` — fold the device findings back there.

---

## 0. What this is

The graph canvas (WO-013 increment 1): `sparq-ui::canvas` (model + invertible ops + undo/redo,
camera + LOD, computed layout + hit-testing, connect verdicts via `sparq-module-api`, the
intent→op table) and the egui painter `sparq-app/src/ui/canvas_ui.rs`, wired into the shell with a
demo patch (sine → gain → rms + a free Gain). No audio path touched; goldens unchanged.

## 1. Apply

**Do not `git clone` onto SATURN** (defect #71: a clone can carry the stray `decode (2).rs` and
poison the build stamp). Sync by zip:

1. Copy `sync-wo013-inc1.zip` to SATURN.
2. Extract **at the repo root**, over the existing tree (it only adds/replaces the files below).
3. Confirm no stray artefacts landed:
   ```
   dir /s /b crates\sparq-module-api\src\*("*.rs" | findstr /i "(2)"
   ```
   (should print nothing).

Files in the zip (repo-root-relative):

```
design/tokens/layout.toml
design/tokens/generated/tokens.rs
design/tokens/generated/tokens.json
design/tokens/generated/tokens.css
crates/sparq-ui/Cargo.toml
crates/sparq-ui/src/lib.rs
crates/sparq-ui/src/gesture.rs
crates/sparq-ui/src/canvas/mod.rs
crates/sparq-ui/src/canvas/model.rs
crates/sparq-ui/src/canvas/camera.rs
crates/sparq-ui/src/canvas/layout.rs
crates/sparq-ui/src/canvas/connect.rs
crates/sparq-ui/src/canvas/interact.rs
crates/sparq-app/src/ui/mod.rs
crates/sparq-app/src/ui/canvas_ui.rs
crates/sparq-app/src/ui/shell_ui.rs
crates/sparq-app/src/ui/headless.rs
docs/ui/gestures.md
PHASE0-WORKORDERS.md
README.md
LATER.md
WO013-INC1-RUN-SHEET.md
```

## 2. Verify headless (no GPU, no digitiser needed — proves the logic on MSVC)

```
scripts\build.bat
scripts\gates.bat
cargo run -p sparq-app --features ui -- ui --audit
```

Expected:
- Build + gates green (the `ui` cell is the one that matters here; `bootstrap-audio`/`hal-wasapi`
  are unchanged by this increment).
- `ui --audit` → **PASS (0 failures)**: 40 matrix cells + DPI-invariance + **14 smokes** (look for
  the 5 canvas lines: *drag a node … snaps … fine*, *three-finger tap undoes*, *cv-out → audio-in
  refused*, *stereo-out → free stereo-in connects*, *two-finger spread zooms*). Design cells report
  **24 touch targets**, 0 violations.
- Goldens unchanged: `selftest --golden` still reports `ba577186c988db21`.

If `ui --audit` FAILs on SATURN but passed in the sandbox, that is a real find — capture the output.

## 3. Build the window and touch-test over RDP

```
cargo run --release -p sparq-app --features ui-window -- ui
```

The shell opens in **Design mode** with the demo patch. Touch-test this checklist (every line is a
logic+render fact that survives RDP — see §4 for what does not):

| # | Gesture | Expect |
|---|---|---|
| 1 | Tap a node body | it selects (selection-accent border); the intent log says `canvas: 1 selected` |
| 2 | Drag a node | it moves; on release it **snaps to the 8 px grid**; log `canvas: move 1 node(s)` |
| 3 | During a node drag, add a **second finger** | `FINE x10` appears top-right; movement gets ×10 finer |
| 4 | Drag from an **output port** toward an **input port** | a wire follows the finger; compatible ports **glow**, others **dim**; release on a glowing port → wire connects |
| 5 | Drag from `RMS.LEVEL` (cv, cyan) onto an **audio** input | refused — log names it (`canvas refused: cv → audio …`); **no wire appears** |
| 6 | Release a wire on empty canvas | log `wire dropped on empty canvas — connection cancelled` |
| 7 | Drag an output onto an **already-fed** single input | the old wire is **replaced**; log `replaced the wire on that single input` |
| 8 | **Long-press** a node | context menu (DUPLICATE / BYPASS / MUTE / LOCK / DELETE); tap DUPLICATE → a copy appears offset on the grid |
| 9 | Long-press a node → LOCK, then try to drag it | refused in words (`node is locked …`); DELETE row shows `DELETE (LOCKED)`, disabled |
| 10 | **Two-finger pan** | the graph slides with the fingers |
| 11 | **Two-finger pinch / spread** | zoom out / in about the pinch centre; zooming out past the thresholds drops node detail (LOD: labels → boxes → dots) |
| 12 | **Double-tap** the canvas | zoom-to-fit frames the whole graph |
| 13 | **Three-finger tap** | undo — the last move/connect/delete reverses |
| 14 | Drag on empty canvas | rubber-band marquee; on release, the covered nodes are selected |
| 15 | Tap the top-bar **HC** button | high-contrast theme restyles the canvas too (no per-widget colour code) |

Watch the intent log (bottom-left of the canvas) — **every** action and every refusal is stated in
words there. If a touch "did nothing", the log must say why; if it doesn't, that is a defect.

## 4. What RDP-from-iPad CANNOT prove (do not sign these off remotely)

- **60 fps at 200 nodes** — the window renders via **WARP** over RDP, not the stage GPU. Frame
  timing here is not the device number.
- **Real palm rejection while wiring** — iPadOS filters the palm *before* Windows sees it, so an RDP
  session exercises the iPad's rejection, not sparq's. sparq's own path needs **WM_POINTER contact
  area** (increment 2). The recogniser's palm logic is proven with synthetic areas in CI only.
- **The full DPI matrix / mixed-DPI dual monitor** — RDP presents one virtual display.

These stay on the WO-012/WO-013 device-evidence list (`RESUME.md` §4) for the physical stage run.

## 5. Report

Capture, for the build log:
- `ui --audit` output (the whole tail).
- Any checklist line that misbehaves, with the **intent-log text** at the moment (the refusal/hint
  wording is part of the contract, so a wrong message is itself a defect).
- A screenshot or two of the canvas at Full and Dot LOD, and of a context menu, if convenient —
  they go next to `design-mode.svg` for the WO-004 side-by-side.

Defects get numbered from **#74** (the increment shipped #73, the recogniser release-position fix).
