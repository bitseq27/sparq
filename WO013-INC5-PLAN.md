# WO-013 increment 5 — the "Else column" pass (plan, 2026-09-27)

Increment 4 shipped live wire levels and declared, in words, what it did NOT ship. This increment
ships exactly that declared list — the four "Else" items — and nothing else (risk R1).

## Scope

### A. Rename text entry (the shared text-entry surface)
- NEW `sparq-ui::canvas::entry`: `TextEntry` (buffer + end-caret, printables-only, capped) —
  toolkit-independent, the surface the browser's provisional keyboard feed was always waiting to
  be replaced by. The shell parses egui events ONCE (`KeyBatch`) and dispatches to whichever
  modal is open: rename first (deepest), then the browser.
- `MenuAction::Rename` + a RENAME row on the node menu (WO-013's context list names it first).
- `RenameState { node, entry, anchor }` in `interact.rs`: modal sheet (3 rows: header, entry,
  hint), geometry clamped by the shared `clamp_origin`. Commit = `Op::Rename` (undoable, already
  in the model since increment 1); EMPTY buffer commits `None` = back to the module default
  (declared in the hint row); Escape or an outside tap cancels.
- Painter draws the sheet; audit registers the entry row. Headless path: `rename_set_text`
  (the same convention as `browser_set_query`).

### B. Inspector scrolling
- `InspectorLayout` grows `scroll` (effective, clamped) + `max_scroll` + `thumb()` (scrollbar
  geometry); NEW `compute_at(node, rect, requested)`; `compute` = `compute_at(…, 0.0)` so every
  existing caller and test is untouched. The title row is FIXED; rows scroll under it; `row_at`
  refuses rows not fully below the title or past the panel bottom — an invisible control stays
  untouchable (increment 3's honesty rule, extended not relaxed).
- Gesture: ONE finger on a row edits the slider, so scrolling rides the TWO-finger `Pan` —
  `GestureIntent::Pan` grows `center` (the centroid the recogniser already tracks; `Zoom` has
  carried its center since WO-012). A pan whose center lands in the inspector content rect
  scrolls the panel; anywhere else pans the camera, exactly as before.
- Scroll is transient view state on `CanvasState` (like `levels`, not undoable); it RESETS when
  the inspected node changes. A panel with nothing to scroll says so in words.

### C. Per-cv wire levels (the inc-4 DECLARED LIMIT, retired)
- `NodeLevels` grows per-port entries `(NodeId, port_index) → level`; `wire_level` prefers the
  SOURCE PORT's level and falls back to the source NODE's folded level — audio wires keep the
  increment-4 semantics bit for bit (the bridge publishes no audio port levels yet).
- The level SOURCE stays the executor — never faked: `bridge::node_levels` now also reads each
  cv OUTPUT port's real published value after the preview render (`node_cv_block` for block
  rate, `node_cv_audio` peak for audio rate). Rule, declared: a cv wire's level is the MAGNITUDE
  of the value its source publishes (bipolar swings light by absolute value), clamped 0..1.
- Painter needs NO change (it already modulates by `wire_level`) — a cv wire out of `mod/lfo`,
  `env/ad`, `ana/rms` now lights from its own signal.
- Still parked (declared, not faked): per-port meters for AUDIO ports and the ring extension
  (port ids on `MeterUpdate`) ride the live-HAL-through-`SharedEngine` increment; continuous
  play-time refresh likewise.

### D. LOD visual iteration vs `design-mode.svg`
The declared contract (camera.rs): Full = header text, port labels, port circles; Simplified =
node box, coloured ports, NO TEXT; Dot = colour-coded dot per node, HAIRLINE wires. The
first-pass painter violated both reduced levels (badges drew text at Simplified; wires kept
class width at Dot). This pass aligns the renderings and adopts the mockup's state language
(look-board §4: "states use PATTERN first — colour is redundant"):
- **Bypass** = 45° hatch over the body (the mockup's `hatch8`: 8 px spacing, hairline at 0.35
  alpha) at Full AND Simplified; at Dot the dot is dimmed + hollow (stroke-only ring).
- **Mute** = dashed border (space tokens for on/off) at Full/Simplified; dimmed fill at Dot.
- **Lock** = double border (inset hairline) at Full/Simplified; double ring at Dot.
- **Master** = a filled chip on the header's right at Full/Simplified (the MASTER word stays at
  Full only, redundant with the chip); a `selected`-colour hairline ring at Dot (selection's own
  ring stays emphasis-width, so weight distinguishes them).
- **Dot wires** = hairline width; grab handles already off; the SUM conversion word is
  suppressed at Dot (unreadable at that scale — the dash/class encoding survives; declared).
- Side-by-side mockup review stays a DEVICE acceptance box (test006 gains the steps); the
  sandbox proves the frame logic runs at all three LODs and the audit stays clean.

## Proof burden
- Unit tests: entry (buffer/cap/trim), inspector (scroll clamp/shift/hidden-row refusal/thumb),
  levels (port-wins/fallback/clamp), interact (menu row, commit/undo/cancel, pan routing,
  scroll reset), gesture (Pan carries its center).
- Bridge tests: the cv level is REAL (rms→svf patch: the cv wire lights at the metered rms) and
  FOLLOWS the signal (drop the sine amp, the cv level falls) — the inc-4 "not faked" discipline.
- New audit smokes: rename flow (menu → entry → commit → title changes → undo restores);
  inspector scroll at 1280×800 with a 20-param mixer (rows move, a clipped row becomes
  touchable, scrolling back restores); cv wire lights from its own port after RENDER WAV; the
  LOD walk (zoom out → Simplified → Dot logged; a port drag at Dot refuses to wire).
- Every pre-existing golden UNCHANGED (nothing in the render path moves — the executor is
  untouched); stress hash `b42068ec7b206789` unchanged; selftest 9/9; all clippy cells; the
  5 python gates; `log_check.py` BASELINE moves WITH the seal (defect #83's discipline).

## Out of scope (parked in LATER.md, unchanged)
Numeric param entry (rides the same `TextEntry` next), enum/text/blob editing (manifest
`options[]` first), multi-select inspection, browser drag-scroll, randomise (seed tree),
per-port AUDIO meters + ring port ids (live-HAL increment), continuous level refresh,
palm-rejection hardware work, grid-dot draw cost.
