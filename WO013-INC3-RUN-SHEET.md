# WO-013 increment 3 — SATURN run sheet (test006)

**Sync bundle:** `sync-wo013-inc3.zip` → extract at the repo root `Q:\morphosis\code\sparq`,
overwriting. Applies on top of the `wo013-inc2b` tree (the one `test005` verified on 2026-09-25).
**The build stamp changes** (`SYNC-STAMP.txt` carries the new fingerprint): `build.bat`'s guard will
force exactly one rebuild — that is defect #49's remedy working, not a failure.

Companion to the build-log entry in `PHASE0-WORKORDERS.md` (search **increment 3**).

---

## 0. What this increment is

The three surfaces the canvas promised: the **module browser** (long-press empty canvas →
ADD MODULE → fuzzy search → tap a row → the module spawns where you pressed), the **inspector**
(one selected node → touch sliders for its params; edits are undoable and the bridge renders
them), and **wire-end re-patch** (drag the small ring near a wire end onto another port; one
three-finger tap restores the original wire). Live wire levels are **not** in this increment —
they need the executor's analysis taps (WO-008) and were parked rather than faked.

Sandbox evidence (2026-09-25): **519 tests** (was 474), clippy clean in the default / `ui` /
`bootstrap-audio` / MSVC×3 cells, `sparq ui --audit` **PASS with 19 smokes** (the three new ones
drive browser spawn, slider-drag + undo and re-patch + undo through the real recogniser with
synthetic fingers), goldens **unchanged** (`ba577186c988db21` matching, determinism
`0f5c3e86c7f117a9` identical), `selftest --golden` 8/8, `sparq exec` PASS with the
cross-validated rms tap 0.16621882.

## 1. The run

One script, one log back — the test005 contract, extended with a window session:

```bat
scripts\test006.bat
```

Steps [00]–[04] are the proven no-device chain (sync check → build + stamp guard →
`modules --strict` → `exec` render → `ui --audit`). Step [05] hashes the audit's
default-param `canvas-render.wav` as the **baseline**. Step [06] opens the window for the
**real-finger checklist** (A–E, printed on screen; answers are recorded in the log). Step [07]
makes the one mechanical claim of this increment: after you edit a slider and tap RENDER WAV,
the hash **must change** — an unchanged hash with C=y is a defect and the script says so in
words.

Expected summary block:

```
[00b] tree matches SYNC-STAMP.txt : PASS rc=0
[01] build + stamp guard          : PASS rc=0
[02] modules --strict             : PASS rc=0
[03] exec offline render          : PASS rc=0
[04] ui --audit, 19 smokes        : PASS rc=0
[07] param edit reached the render : CHANGED
```

## 2. Send back

* `test006.log` (repo root) — the one file.
* `logs\ui.log` — the window session's own log.
* If step [07] says `UNCHANGED` after answering C=y: also keep
  `canvas-render-baseline.wav` and `canvas-render.wav` and say so — that pair is the defect.

## 3. Known limits, stated before you find them

* Over RDP the rasteriser is WARP — the window is slow; 60 fps claims wait for the physical
  screen (unchanged policy).
* The browser's text entry is a **provisional keyboard feed** (egui input events while the sheet
  is modal). If characters double, drop or arrive out of order, that is this feed — record it and
  keep going; the ranking itself is deterministic and sandbox-proven.
* The inspector shows params for **exactly one** selected node; multi-select inspects nothing
  (v0, parked in `LATER.md`). `enum`/`text`/`blob` params are greyed with the kind named — not
  editable until manifest v1 grows `options[]`.
* Inspector rows below the panel bottom are clipped and **not touchable** (no scrolling yet —
  parked).
* Wire-end rings are small; the grabbable zone is 24 px around them (the same capture as ports).
  If a drag from a ring starts a NEW wire instead of moving the end, you grabbed the port itself
  — grab the ring, 32 px out along the wire.
