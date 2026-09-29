# Sync manifest — WO-012 increment 4 (the MTA audio-control thread — PLAY works in the window — plus the mouse, the master meters, and the grouped palette)

**Current bundle: `sync-wo012-inc4.zip` (23 entries, listed below).** The chain below it
(`sync-wo008-inc7.zip`, `sync-wo012-inc2.zip`, `sync-wo013-inc6.zip`, `sync-wo012-inc3.zip` on
top of the APPLIED `sync-wo006-inc15.zip`) is **APPLIED per operator report** — this is the
ONLY waiting bundle. Extract at the repo root `Q:\morphosis\code\sparq`, overwriting. No
drift-repair copy step. This bundle changes **no device contract** — test004 attempt 4 runs
exactly as `WO006-INC15-RUN-SHEET.md` says — but it MOVES the gates' numbers (smoke count,
stamp), so apply before the next `gates.bat` run.

**The stamp changes: `src 94f/2188595B`** (`SYNC-STAMP.txt` regenerated for the same
**153-file** covered set — no new covered file; the byte count moved with the reworked session,
the window adapter, the meter publish and the chrome. `Cargo.lock` stays SOFT and is NOT in the
zip. **No manifest moved** — 17/17, every audio golden keeps its bits.)

## What this is

The operator tapped PLAY on SATURN and got the sentence this bundle answers:

> `PLAY REFUSED: enumerating wasapi-shared devices: system error: this thread already
> initialised COM as single-threaded (STA); the WASAPI HAL needs MTA. Phase 0 hosts are console
> apps — if this appears inside a GUI host, the shell (WO-012) must not OleInitialize on the
> audio-control thread`

The WASAPI HAL keeps a **persistent per-thread MTA** for its control-side calls; a winit UI
thread is STA and can never become MTA. Plan of record `WO012-INC4-PLAN.md` (D1–D4). What
shipped:

* **One audio-control thread per session** (`ui/live.rs`, reworked): the ONLY thread that
  touches the backend — enumerate → probe → open → start → stop → drop, plus pump/fault/capture
  on the manual null — over a request-reply protocol with **bounded waits** (5 s: a driver that
  owns its thread longer is a device fault said in words, not a frozen shell). The worker
  becomes MTA by simply calling the HAL; it dies with the session. The UI thread keeps the
  `SharedEngine` control half (rings and hot-swap are `Send + Sync` by design), the node map,
  the drain — and reads a **health mirror** the worker polls at 50 ms, so the per-frame
  `Removed`/`Failed` gate and the top-bar diagnostics cost no round trip. `Drop` sends Stop and
  **joins**: a session that leaked its thread would leak the device. Two new session gates
  prove the teardown: back-to-back sessions, and drop-without-stop.
* **Mouse usability** (D2): unpressed mouse motion is **dropped at the window adapter** — the
  `motion for an untracked pointer` line in the operator's log was hover being read as broken
  touch; it can now only mean a real missed Down. **Right-click synthesises the recogniser's
  own `Context` intent** (the menu a finger long-presses, without the 350 ms hold); **the wheel
  synthesises `Pan`** and reuses the panel-over-panel routing — over the inspector it scrolls
  the rows, over the canvas it pans the camera; no modifier keys anywhere (§8). Both arrive
  through a new `FrameInput::extras` slice, so touch and mouse land in the SAME intent table
  and the smokes can drive them headless.
* **`out/main` meter bars like the mockup** (D3): `MeterUpdate` grew `peak_l`/`peak_r` — the
  publish pass already walks every output buffer per port, so the per-channel split is one more
  accumulator in the same cache-warm pass; mono ports duplicate (the cross-thread bit-identity
  pin grew to cover `peak_l == peak_r == peak`). The bars draw on `out/main`: green DATA-class
  fills (a meter is data *about* the signal — finding 17), amber AUDIO-class peak-hold blocks
  decaying on **audio time** (block counts at the negotiated rate — no wall clock on the
  display path), empty wells at rest. RENDER WAV does not feed them (the offline preview
  publishes no ring) — declared, like the scope's rest state.
* **The palette** (D4): dock tiles 248×80 → **112×56** (token values through the generator;
  touch class L → M, the 44 px floor holds), **grouped by manifest top category** under xs
  words, left **stripe in the dominant signal class** — §4's rule (colour says what the module
  carries; the group says what it is), never accent-by-category. All seventeen first-party
  tiles fit at 1920×1080; overflow, when a bigger registry comes, is counted in words.

## Measured in sandbox

**777 tests** (+2 session gates behind `ui`) · 0 failed, 1 ignored · **37 smokes** (+3:
right-click opens the node menu and an outside tap closes it; the wheel scrolls the inspector
over the panel and pans the canvas over the canvas — on a 24-param mixer spawned from its dock
card, so there is something to scroll; `out/main`'s bars read the live per-channel ring) · the
breakpoint matrix at the new element counts with **0 violations** · fmt clean · clippy clean in
every runnable cell: default, `bootstrap-audio`, `ui`, combined audio+hal, native `ui-window`
(single-job — the window adapter changed) and all six MSVC cross-lint cells; the increment owed
three lint debts paid in words: a reasoned `#[allow(clippy::disallowed_types)]` on the
control-path mirror `Mutex` (the defect-#66 precedent; the audio thread never sees it),
`checked_div` for the channel count, and a boxed command variant for the enum-size lint · the 5
python gates · release goldens **bit-identical**: stress `7bb06379bd6845e5` · 2 383 · 7 617
debug AND release, the three pinned exec renders, `canvas-render.wav` 1 920 046 B ·
`d7ad294e…`, determinism `0f5c3e86c7f117a9` · `selftest --golden` **PASS (9 gates)** ·
`modules --strict` **17/17** · `probe_alloc` 0/5 000 · `log_check --self-test` 25/25 (BASELINE
moved: `src 94f/2188595B`) · `sync_check --write` → `--quiet` (153 files) → `--self-test` 11/11.

## Files in this zip (23)

Covered by the stamp — 9:

```
crates/sparq-app/src/ui/canvas_ui.rs   (draw_master_meters: the wells, data-class fills, audio-class holds; the meters parameter threaded like the traces)
crates/sparq-app/src/ui/headless.rs    (step_x: the extras door; the three mouse/meter smokes; 37 total)
crates/sparq-app/src/ui/live.rs        (REWORKED: the audio-control thread + protocol + health mirror + Drop-joins; stereo meter drain with audio-time holds; +2 gates)
crates/sparq-app/src/ui/shell_ui.rs    (FrameInput::extras joined into the intent table; the meters source handed to the painter)
crates/sparq-app/src/ui/window.rs      (hover dropped at the adapter; right-click → Context; wheel → Pan; the extras buffer drained per frame)
crates/sparq-audio/src/engine.rs       (MeterUpdate.peak_l/peak_r; peak_rms_channels — one cache-warm pass, mono duplicates)
crates/sparq-ui/src/canvas/levels.rs   (StereoMeter + LiveMeters: the bars' single-source type)
scripts/test006.bat                    (37 smokes; the mouse paragraph in the window session; J–K unchanged)
tools/log_check.py                     (BASELINE stamp moved: src 94f/2188595B)
```

Design + documents — 14 (excluded from the stamp on purpose):

```
CHECKLIST.md   EXTRACT-AT-REPO-ROOT.txt   LATER.md   PHASE0-WORKORDERS.md   README.md
SYNC-STAMP.txt   SYNC.md   WO012-INC4-PLAN.md   design/mockups/mockup-review.md
design/mockups/convergence-wo012-inc3.png   design/tokens/layout.toml
design/tokens/generated/tokens.rs   design/tokens/generated/tokens.json
design/tokens/generated/tokens.css
```

Heading = list = contents = **23** (9 + 14), checked against the zip's namelist below.

## What SATURN runs

Apply this bundle (the chain below it is applied), then, in order of value:

1. **Tap PLAY.** That is the acceptance this bundle exists for: the log names the backend, the
   NEGOTIATED rate · ch · block and the latency; the patch is audible at shared-mode latency;
   the top bar shows the live diagnostics; the rail's level tick fills; `out/main`'s bars move
   with green fills and amber holds; the wires animate. STOP prints the measured evidence.
   If PLAY still refuses, the refusal line names the phase (probe/open/start) and the driver's
   own words — send it back verbatim.
2. `scripts\test004.bat` — attempt 4, the 🔴 blocker, unchanged in shape.
3. `scripts\test006.bat` — F–K standing, plus the new mouse paragraph (right-click = the menu,
   wheel = inspector scroll over the panel / camera pan over the canvas, hover silent) and the
   palette: every module one tap away in its group.
4. `scripts\gates.bat` — **777 passed / 0 failed / 1 ignored**, stamp `src 94f/2188595B`,
   17 modules, selftest **PASS (9 gates)**, `ui --audit` **PASS (37 smokes)**, stress line
   `7bb06379bd6845e5` · 2 383 · 7 617.
5. The pinned-duration exec evidence and the WO-009 device boxes, as before.

**Wanted back — the DIGESTS: `test004-digest.log`, `test006-digest.log` (F–K + the mouse
notes + what still differs from the mockup on the stage screen), `gates-digest.log`,
`logs\ui-digest.log` — and one sentence: did PLAY make sound.**

## Superseded bundles

Waiting: this bundle only.
Applied chain (newest first): `sync-wo012-inc3.zip` (24), `sync-wo013-inc6.zip` (16),
`sync-wo012-inc2.zip` (20), `sync-wo008-inc7.zip` (17) — all operator-reported APPLIED
2026-09-29/30 — then `sync-wo006-inc15.zip` (15), `sync-wo006-inc14.zip` (15),
`sync-wo008-inc6.zip` (17), `sync-wo014-inc6.zip` (13), `sync-wo013-inc5.zip` (20),
`sync-wo014-inc5+wo013-inc4.zip` (29), `sync-wo008-inc5.zip` (22), `sync-wo014-inc4.zip` (12),
`sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19), `sync-wo008-inc4.zip` (38),
`sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15), `sync-wo006-inc13.zip` (10),
`sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`,
`sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`,
`sync-wo006-inc11g.zip`.

## Namelist

```
CHECKLIST.md
EXTRACT-AT-REPO-ROOT.txt
LATER.md
PHASE0-WORKORDERS.md
README.md
SYNC-STAMP.txt
SYNC.md
WO012-INC4-PLAN.md
crates/sparq-app/src/ui/canvas_ui.rs
crates/sparq-app/src/ui/headless.rs
crates/sparq-app/src/ui/live.rs
crates/sparq-app/src/ui/shell_ui.rs
crates/sparq-app/src/ui/window.rs
crates/sparq-audio/src/engine.rs
crates/sparq-ui/src/canvas/levels.rs
design/mockups/convergence-wo012-inc3.png
design/mockups/mockup-review.md
design/tokens/generated/tokens.css
design/tokens/generated/tokens.json
design/tokens/generated/tokens.rs
design/tokens/layout.toml
scripts/test006.bat
tools/log_check.py
```
