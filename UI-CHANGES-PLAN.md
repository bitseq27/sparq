# UI change round — operator rulings 2026-10-01 (run sheet, DONE)

One round of operator-requested changes. Every item below is implemented, tested, and
audited; the gates at the foot of this sheet all pass.

| # | Request | Where it lives |
|---|---------|----------------|
| 1 | Sine sliders follow the mouse | `inspector.rs`: `is_log_scale` — Hz params with min > 0 map x↔value logarithmically (`value_from_x`/`knob_x`, one inverse pair); both surfaces (inspector + node card) read it |
| 2 | Covered ports not interactable | `layout.rs hit_test`: draw-order occlusion — a port or wire-end grab under a LATER card's body is untouchable (new tests `a_port_covered_by_a_later_card_is_not_interactable`, `a_wire_end_under_a_later_card_is_not_grabbable`) |
| 3 | Colour bar/margin 2 px | `canvas_ui.rs draw_node_box`: stripe = `LAYOUT_STROKE_SIGNAL` (2) |
| 4 | Smaller library tiles, name + colour ref | `layout.toml library_card_h` 128 → 32; `shell_ui.rs draw_library_card`: class dot + name only; touch class S with the dense badge |
| 5 | Group toggle switches at the library head | `shell_ui.rs`: one switch chip per top category (`Action::ToggleLibraryGroup(usize)`, `library_groups_off`); `library_categories` no longer carries a synthetic ALL |
| 6 | Ports float beside the window | `layout.rs`: port x = card edge ± `canvas.node_port_offset` (4 px world) |
| 7 | IN/OUT meters out of the workspace | `shell_ui.rs`: `draw_master_strip` deleted; legend owns the right edge; master meters stay on the master's own card well; smoke 50 rewritten to assert the strip is gone |
| 8 | Selection highlight white | `colors.toml state.selected` = #FFFFFF with the ONE documented pure-white exemption (`forbidden.pure_white_allowlist`, enforced both ways by token_gen check 7); regenerated tokens |
| 9 | Toggles for binary settings | `inspector.rs is_binary` + `ParamRow.toggle`; `interact.rs`: tap flips, drag flips once, one undo step; switches drawn on node cards and in the inspector (`canvas_ui::draw_toggle`) |
| 10 | Sine frequency 0.1 – 10 k | `modules/syn/sine/sparqmod.toml`; module docs regenerated |
| 11 | Main Out = permanent driver window | `bridge.rs` catalogue excludes out/main; `interact.rs` refuses spawn/duplicate/delete (menu, DEL, select-all) in words; `layout.rs info_band` + `canvas_ui` driver lines from `live.rs driver_lines` (backend · device / rate · ch · block · 32-bit float), at-rest words when no session |
| 12 | Bigger scope, measurements, grid | `layout.toml node_well_height_scope` = 160; `scope.rs measure`/`graticule`/`ms_per_div` (pure, tested); `canvas_ui draw_scope_display`: 10 × 8 graticule, centre cross one tier up, measurement line (ms/div · Vpp · RMS · PK), NO SIGNAL in words for an empty window |
| 13 | Wheel = zoom, right-drag = pan | `window.rs`: wheel → `Zoom` about the cursor over the canvas, → `Pan` scroll over inspector/library, nothing over other chrome; right press + travel → `Pan`, right press without travel → `Context` (unchanged menu) |
| 14 | DEL deletes selection | `gesture.rs GestureIntent::Delete` (host-synthesised); `interact.rs delete_selection` (wires then nodes, protections speak, refusal with remedy on empty selection); gated off while rename/sheets/library-search own the keyboard |

## Docs touched

* `docs/ui/gestures.md`: §2 table gains the mouse-hand intents; new §2b (the mouse hand),
  §3c (floating ports + covered-is-untouchable), §3d (the permanent Main Out), §4c bullets
  (log map + toggles), §4e (the driver window).

## Verification (all pass, this machine)

* `cargo fmt --all --check`
* `cargo clippy --workspace --all-targets -- -D warnings` and the `ui`, `ui-window`,
  `bootstrap-audio` cells — clean
* `cargo test --workspace` — all green (new sparq-ui tests: occlusion ×2, delete/protections ×3,
  toggle flip, log map round-trip, binary classification, measure, graticule)
* `cargo run -p sparq-app --features ui -- ui --audit` — layout matrix (5 viewports × 4 DPI
  scales) + every smoke: PASS, 0 failures
* python gates: `token_gen --check`, `token_audit`, `unsafe_audit`, `module_docs --check`,
  `check_text_io`, `sync_check --self-test`
* `sparq selftest --golden` — 9 gates PASS (golden reference unchanged: DSP untouched)
* screenshots: `docs/ui/shots/operator-round-2026-10-01-default.svg` and
  `docs/ui/shots/operator-round-2026-10-01-review.svg` (regenerate with
  `sparq ui --svg-out … [--review]`; the review sheet now also rigs tap → scope so the
  enlarged scope screen, its graticule and its measurements are visible hot)

* The round is also declared in the live handoff docs, house style: a new round paragraph in
  `CHECKLIST.md` (with the updated `Last updated` line) and a new current-bundle section in
  `SYNC.md` (the inc7c section kept below it as history).

## Deliberate non-changes

* `SYNC-STAMP.txt` left at the committed stamp: it is the cross-machine sync contract and must
  be re-written on the source machine (it would otherwise carry this sandbox's `Cargo.lock`).
* The mockups (`design/mockups/*.svg`) are untouched; R1–R6 audit them, and the white selection
  exemption lives in the token gate, not the mockups. The committed shots live under
  `docs/ui/shots/` (outside the audit's default paths) because a screenshot of the running shell
  legitimately carries runtime-composited colours, glow widths and the white selection frame —
  it is a capture, not a design original.

---

# Round 2 — same day, second operator list (DONE)

| # | Request | Where it lives |
|---|---------|----------------|
| 1 | sine/polyblep 10 Hz – 10 kHz | both manifests (log map from round 1 applies unchanged) |
| 2 | clock module: bpm, 4th/8th/16th/32nds | `modules/mod/clk/sparqmod.toml` + `modules.rs Clock`: four sample-accurate event grids, tempo edits re-space from the next tick, 32-byte state |
| 3 | 3–16 beat trigger sequencer | `modules/mod/seq/sparqmod.toml` + `modules.rs TrigSeq`: ring walks per qualifying trigger, bitmask fires at the input's sample, 8-byte step state |
| 4 | buttons for multi-choice settings | `inspector.rs is_choice/choice_count` (int domain 3…8) + `canvas_ui::draw_choices` on cards and inspector; tap sets, drag steps |
| 5 | card sliders sometimes dead | `layout.rs hit_test`: a press inside a card body never belongs to a port or wire grab (the capture ring reached back over the card edge); regression test on both row ends |
| 6 | zoom-relative module content | `canvas_ui`: one zoom factor scales card text, knobs, ports, switches, wells, strokes; touch targets and the selection frame stay screen-sized |
| 7 | Main Out band breaks frame; stereo in/out | char-boundary clip + composed-to-fit line two (`48 kHz · 2 ch · 64 fr · f32`); `out/main` in = stereo |
| 8 | control wave display blue | `draw_sparkline_well`: the lfo period draws in the control-class accent at signal weight |
| 9 | delete keeps the chain connected | `interact.rs remove_node_spliced`: port-ordered splices through `connect::resolve`, removal + splices in one `Op::Batch`, one undo, splice count stated |

Tests: `crates/sparq-audio/tests/mod_clk_seq.rs` (exact tick lists for all four grids, downbeat
coincidence, tempo re-space, mask/wrap semantics, zero allocations) + the layout regression test.
Docs: `gestures.md` §3e (splice) and the §4c button-row bullet; README set count 17 → 19; the
review sheet rigs tap → scope and clock-16ths → seq. Gates: 825 tests (+1 ignored), goldens
untouched, clippy clean in every runnable cell, python gates clean, `ui --audit` PASS 0 failures.

---

# Round 3 — same day, third operator list (DONE)

| # | Request | Where it lives |
|---|---------|----------------|
| 1 | Control-connection dots; all float params controllable | `layout.rs` sink dots on every float row (half port radius, ports' column) shown only while a cv drag is in flight (`draw_pending_wire`); `connect::resolve_param` + `Wire.param` destination + `Hit`-free sink magnet; executor `add_param_mod` per-block plan (`clamp(knob + cv·half-range)`, one-block latch); bridge wires the plan; dashed control-colour wire + sink dot painters |
| 2 | VCA module | `modules/util/vca` + `modules.rs Vca`: out = in × (level + cv), glide discipline, optional cv in |
| 3 | No zoom past 100 % | `layout.toml zoom_max` 4.0 → 1.0 (token, regenerated); smoke 8 re-pointed at the ceiling |
| 4 | Scope accepts any audio/control/data/event source | `connect.rs` scope-binding exception (matrix governs signal paths, not display bindings); bridge keeps scope-input edges out of the kernel; `engine.rs publish_analysis` also publishes audio outputs' first channel (strided); event/data bindings say NO WAVEFORM in words |
| 5 | LFO graph blue | scope trace wears the SOURCE's class colour (an lfo/tap on the scope draws control blue); the lfo well has been blue since round 2 |
| 6 | Trig seq: 16 buttons, input steps them | `layout.rs step_cells` (card strip of 16) + inspector 2×8 chips; `Hit::Step` + `flip_step` through the param door; painters on card and inspector; clock input walks the ring as before |
| 7 | Mult: 6 vertical dots, in/out by first connection, type by first connection | `modules/util/mult` (no-op DSP, patching junction); `connect.rs resolve_mult` (role/type/source rules, refusals in words); `Graph::mult_port_role/defining_port` derived from wires; bridge COLLAPSES the bus into direct kernel edges; levels follow the bus; role-wearing dots in the painter |

New gates: `crates/sparq-app/tests/r3_controls.rs` (modulation audible + clamped, cv-only/float-only
refusals, sink hit-geometry, bus role/type/source refusals, collapse agreement, step-button
flips) + headless smokes 51–52 (the bus collapses and every copy lights like the source; a
control wire applies through the driver door). `analysis_pub` re-pinned for the audio waveforms
now crossing the ring (4 publications/block in that world, refusals still counted).

Sandbox note (stated in CHECKLIST/SYNC): a reset between turns wiped `/tmp`, the `/usr` additions
and the workspace `.git`; the toolchain now lives in `.tc/` inside the workspace (gitignored),
and rev 3 ships as a FULL-TREE overlay pack (no patch base survives here).

---

# UI change round 4 — operator rulings 2026-10-02 (run sheet, DONE except one declared piece)

The second round of operator rulings (the 15-item list, transcribed in `ROUND4-HANDOFF.md` §9),
built on the round-3 tree. **Provenance:** the round-4 code itself was lost with the sandbox that
wrote it (only the docs reached git); this run sheet describes the REBUILD from
`ROUND4-HANDOFF.md` on a clean clone of `871d725`, re-measured end to end — see the round-4
paragraph in `CHECKLIST.md` for the numbers and the two provenance notes (mutation-stress hash,
defect #95).

| # | Request | Where it lives |
|---|---------|----------------|
| 1 | Adding a module while playing must not blank/re-stage the whole graph | `executor.rs adopt_runtime` — allocation-free adoption of unchanged nodes across a hotswap (per-node keys, in-node scratch), hooked at the kernel boundary (`engine.rs`, `hotswap.rs` boundary is now `FnOnce(&mut T, &mut T)`); the command-ring fast path stays declared LATER, its door is the bridge's `TrimGainMap` |
| 2 | Clipping visible on the main meters | `levels.rs StereoMeter.clip_l/clip_r` latched from the live frame (`live.rs`), painted as the red cap + the CLIP word on the master's info band (`canvas_ui.rs`); the latch holds while playing and clears at STOP/session start |
| 3 | The SVF graph animates with modulation | `shell_ui.rs rebuild_curves` — the response curve is drawn at the EFFECTIVE cutoff `clamp(knob + cv × half-range)` read from the analysis ring, per frame |
| 4 | Clock wheel (4/8/16/32, left of the outs) | `inset.rs` ClockWheel well + `clock_ring_rects`/`ring_point` — four division rings driven by the engine's published wrap-correct `phase`, turning under PLAY, standing still at rest |
| 5 | Seq steps animate | `inset.rs` Steps well — the cursor cell carries the event-accent ring over the pattern fill; the engine publishes `step` sink-gated |
| 6 | `util/mult` a quarter width, centred, bidirectional, no labels | `layout.rs` quarter-width strip + centred dot column (S6), words off on the card (S7); the bidirectional semantics were already the bus's and stay pinned by its gate |
| 7 | VCA accepts control | `util/vca` manifest D7 (cv in = amplitude modulator); the canvas-verdict gate lives in `bridge.rs` tests: verdict Compatible and the modulated render audible |
| 8 | Fold module | `modules.rs Fold + fold_tri` (waveshaper, tri-fold at amount 1), `modules/fx/fold/sparqmod.toml`, doc generated |
| 9 | Event cables solid | `colors.toml` — the event encoding token is `solid-stroke` at signal width (D9); legend and painter parse the token, the control wire's dash STAYS (different encoding); mockup-review finding 25 records the mockup supersession |
| 10 | Ports 6 px beside the window | `layout.toml canvas.node_port_offset` 4 → 6 (D10), tokens regenerated |
| 11 | Note quantizer + dropdown + keyboard | `modules.rs Quant + QUANT_SCALES + snap_to_scale` (15 scales + Custom reading `custom-mask`), `modules/util/quant/sparqmod.toml`; S6 `key_cells`/`key_param`/`flip_key` + `Hit::Key` with the Custom-mode gate (a preset-scale tap refuses in words and names the remedy); S7 the 12-cell keyboard well — membership fill + passing-pitch light, the display is the module. **The dropdown (inspector list-picker over the 15 names) is the one DECLARED OWED piece** — `QUANT_SCALES` is the name table it waits for; it was not on the S7 painter list and the seal ships without it, named here and in CHECKLIST |
| 12 | Random step module + display | `modules.rs RandStep + step_hash` (lowbias32 over `seed ^ i·φ`, top 24 bits; the seed-42 ring is pinned in BOTH `sparq-audio` and the painter's mirror — cross-crate), `modules/mod/rand/sparqmod.toml`; S7 the step-bars well: bars + lit cursor, same seed same bars |
| 13 | RMS slew + rolling graph | `modules.rs` rms stateful + `slew` param (per-block slew limit); `levels.rs LevelHistory/LevelHistories` — a 240-frame rolling graph the shell pushes per live frame, painted beside the rms card's floor bar, cleared at session start |
| 14 | Delay sync input | `modules.rs` delay `sync` — a trigger dumps the tail and re-arms, sample-accurate; manifest bumped |
| 15 | Cable nodes | Model `WireTrim`/`Op::SetTrim` + structural ledger row (S6); gestures: hover ghost, tap-insert at identity, drag axes (up/down amp 0…2, left/right offset −1…+1, 100 px per unit, the audio offset inert by the DC rule), tap-handle removes, one coalesced undo per drag; hit rank port > handle > grab > body > wire, never at Dot LOD; S7 the trim node painted (capture ×0.25 rest / ×0.5 hot, amp level disc, hover ghost); S8 the bridge's voice — audio → invisible synthesised `util/gain`, cv → `set_cv_trim`, control → param-mod args, mult-collapse composed affinely, event/data/spatial refused IN WORDS with the remedy, identity renders bit-identical; audit smoke 54 pins insert/edit/remove end to end (`docs/ui/gestures.md` §4f carries the vocabulary) |

## Docs touched (round 4)

* `docs/ui/gestures.md`: new §4f (the cable node + the quantizer keyboard tap), §6's event row
  (solid, D9), §7's round-4 device list (test006 steps L–U).
* `design/mockups/mockup-review.md`: findings 25–26 (the event-solid supersession; the round-4
  wells and the 6 px float declared as operator rulings the mockup set predates).
* `docs/modules/*`: 24/24 regenerated; `CHECKLIST.md` / `SYNC.md` / `README.md` round-4
  paragraphs; `scripts/test006.bat` steps L–U; `ROUND4-HANDOFF.md` §0–§0e session record.

## Verification (all pass, this machine — Linux sandbox, rustc 1.99.0, single-job)

* `cargo fmt --all --check` — clean
* `cargo clippy` workspace + `ui` + `bootstrap-audio` cells, `--all-targets -D warnings` — 0
  diagnostics (the `ui-window` cell is device-only, as always)
* `cargo test --workspace` — **899 passed / 0 failed / 1 ignored**
* `cargo build --release` + `sparq selftest --golden` — 9/9, `ba577186c988db21`; the three pinned
  exec renders exact: `mod-demo` `1621e1f65b1b64e1`, `drum-demo` `f2303f13aa0cf299`, `demo`
  `53de3b1f3f40e3c9`
* `sparq ui --audit` — PASS, 0 failures (59 smoke lines: the round-3 set + clip latch, phase
  pipeline, encoding pin, cable-node render hashes)
* `sparq modules --strict` — 24/24
* python gates — token_gen --check, token_audit, unsafe_audit, module_docs 24/24, check_text_io,
  sync_check (re-stamped `sparq-round4-2026-10-02`)
* Known-failing, declared: defect **#95** (`ui::live` glide-lag live test) — pre-existing on the
  pristine clone, stash-proven not a round-4 regression; the ui-feature cell reads 28/1 until the
  operator triage lands.
