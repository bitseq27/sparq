# ROUND4-HANDOFF.md — operator round 4, mid-round session handoff (2026-10-02)

**Purpose:** the first file the NEXT session reads (besides CHECKLIST.md's new head paragraph).
Operator round 4 is HALF BUILT: slices S1–S5 plus the S6 model layer are done and green;
S6's interaction batch, S7 (painter), S8 (bridge) and S9 (seal) remain, fully specified below
so nothing has to be re-discovered. The plan of record is **`UI-ROUND4-PLAN.md`** (decisions
D1–D16; the operator's verbatim 15-item list is in its preamble). Three rulings came through
the question tool: **VCA fix = manifest move to unipolar · cable nodes = per-wire trim record ·
quantizer scales = the standard 15**. The checkpoint is sealed as
**`sparq-round4-checkpoint-1.zip`** (full changed-file overlay; the GitHub repo went PRIVATE
mid-session — zips are the only delivery path, git routes are dead).

---

## 1. Tree state at the checkpoint (all measured, 2026-10-02)

* **`cargo test --workspace`: 865 passed / 0 failed / 1 ignored** (rev-3 baseline 832 + 33 new
  gates: 8 adoption + 10 `r4_modules` + 12 `r4_new_modules` + 1 r3 control-trim + 2 mixer
  cv-trim). The ignored one is `alloc.rs`'s doc test (standing).
* `cargo fmt --all --check`: CLEAN (fmt was run over the round's edits at checkpoint).
* Python gates ALL PASS: `token_gen --check` (0 to write), `token_audit` (clean),
  `unsafe_audit` (clean), `module_docs --check` **24/24**.
* **Goldens untouched**: golden suites green in debug (17 + modules_golden + determinism +
  mutation_stress hash `7bb06379bd6845e5` held — the single-owner `Engine::stage` deliberately
  does NOT adopt; only the SharedEngine boundary hook does, which is the live-session path).
  Release re-verify (`selftest --golden` → `ba577186c988db21`, the three pinned exec renders)
  is an S9 step — not re-run this session.
* **`sync_check`: FAIL BY DESIGN** — 15 stamp-covered files moved (the round's edits, listed
  in §2). The stamp re-write is the S9 seal step; until then `scripts\build.bat` printing its
  changed-file report is EXPECTED, not a failure (SYNC.md's own sentence). `build.bat
  --skip-sync-check` if a device needs to build mid-round.
* **Defect #94 (new, recorded):** the between-turn sandbox reset dropped every path component
  named `out` (defect #93's trap again) AND the repo went private — no upstream copy existed.
  `modules/out/main/sparqmod.toml` was REBUILT from its generated doc and PROVEN semantically
  identical (`module_docs.py` regeneration byte-matches the checked-in `docs/modules/out-main.md`);
  its bytes differ from the stamp's `3b0a619f…`/2544 B by comments only, so sync_check names it
  until the re-stamp. Durable copy: `/home/user/sparq-recovery/modules-out-main-sparqmod.toml`
  (OUTSIDE the repo — the #93 remedy). Its header declares the reconstruction.
* **No `.git`** — the reset took it (round 3 precedent). Nothing is committed; the tree + the
  checkpoint zip ARE the state.

## 2. What is DONE — file by file (the 15 moved + the additions)

| File | What moved | Pinned by |
|---|---|---|
| `crates/sparq-audio/src/executor.rs` | **D1 adoption**: `ExecNode.mver` (version hash), `Executor.canvas_keys` + `adopted`, `set_canvas_keys()`, `adopt_runtime()` (identity+shape-gated instance swap; delay hists / held frames / comp lines / cv knots / mod scalars cross; params deliberately DO NOT — the staged build rules), `version_hash`/`adopt_key`/`cv_trim` helpers; **D15**: `ParamMod.scale/offset` (+8-arg `add_param_mod`), `CvPlan.scale/offset`, `set_cv_trim()` with refusals in words, the trimmed cv wire pass (identity = a branch, bit-exact) | 8 adoption gates + trim gates in `tests/executor.rs`; mixer_cv trim pair; every golden |
| `crates/sparq-audio/src/engine.rs` | boundary hook: `adopt_runtime` then `inherit_runtime` (D1) | cross_thread suite (8/8 incl. the 10 000-mutation acceptance) |
| `crates/sparq-kernel/src/sync/hotswap.rs` | `boundary` closure is now `FnOnce(&mut T, &mut T)` (adoption swaps instances) | kernel suite 110 |
| `crates/sparq-audio/src/modules.rs` | **D4** clk `phase` cv-out publication (wrap-correct modulo, same counter as the ticks); **D5** seq `step` cursor publication (sink-gated advance kept); **D13** `Rms` stateful + `slew` one-pole (default 0 = raw, bit-exact); **D14** delay `sync` event in — sample-accurate tail dump via `stereo_tick_f`, gate-offs ignored; **D8/D11/D12** `Fold`+`fold_tri`, `Quant`+`QUANT_SCALES`+`snap_to_scale`, `RandStep`+`step_hash` (splitmix32) + factories; 3 manifest consts; `BUILTINS` 21→**24** | `tests/r4_modules.rs` (10), `tests/r4_new_modules.rs` (12 — the seed-42 ring is PINNED: `[1517363, 10542902, 13721998, 8088911, 6531285, 16292829, 12864735, 5502068]`/2²⁴), registry-count gate |
| `crates/sparq-app/src/bridge.rs` | canvas keys stamped on every canvas build (`set_canvas_keys`, kernel-id order); `add_param_mod` call takes identity trim `(1.0, 0.0)` for now — **S8 replaces it from `wire.trim`** | adoption-by-key gate; r3_controls unchanged verdicts |
| `crates/sparq-ui/src/canvas/model.rs` | **D15 model**: `WireTrim { amp, offset }` + `identity()`, `Wire.trim: Option<WireTrim>` (Wire lost `Eq` — f32; `PartialEq` kept), `Op::SetTrim` + inverse + label "wire trim" + apply, `Graph::op_set_trim` (clamps amp 0..2, offset −1..1) | compile + S6 tests (to write) |
| `crates/sparq-ui/src/canvas/interact.rs` | `note_patch`: `SetTrim → structural` (the door's decided sentence: trims re-stage; silent since D1; a command-ring fast path for audio trims is a declared LATER optimization) | live-session classification stays exhaustive |
| `modules/mod/clk/sparqmod.toml` | 0.2.0, `phase` cv-out APPENDED (event indices unmoved) | r4_modules phase gates; tick lists stand |
| `modules/mod/seq/sparqmod.toml` | 0.2.0, `step` cv-out APPENDED | r4_modules cursor gates |
| `modules/ana/rms/sparqmod.toml` | 0.2.0, `slew` param APPENDED (default 0 = old bits) | r4_modules slew gates |
| `modules/util/delay/sparqmod.toml` | 0.3.0, `sync` event-in APPENDED (optional) | r4_modules sync gates |
| `modules/util/vca/sparqmod.toml` | **D7**: 0.2.0, cv `range = "unipolar"` (the fix — every first-party source is unipolar; matrix cell G2 NOT moved; `util/range` stays Phase 1) | canvas verdict flips (S6/S8 gate to add: lfo→vca compatible) |
| `modules/out/main/sparqmod.toml` | RECONSTRUCTED (defect #94) — semantically proven, bytes differ | module_docs regen == checked-in doc |
| `design/tokens/colors.toml` | **D9**: `signal.event.encoding` `dashed-6-3` → `solid-stroke` (control-wire dash of §4c is a different encoding and stays) | token_audit clean; painter parses tokens |
| `design/tokens/layout.toml` | **D10**: `node_port_offset` 4 → 6 | sparq-ui 182/182 (geometry tests adapt via the token) |
| NEW `modules/fx/fold/`, `modules/util/quant/`, `modules/mod/rand/` | the three manifests (headers carry the exact semantics) | docs generated 24/24 |
| NEW `crates/sparq-audio/tests/r4_modules.rs`, `r4_new_modules.rs` | the round's module gates | counted above |
| `docs/modules/*.md` (24) | regenerated | `module_docs --check` |
| `docs/ui/gestures.md` | ports 6 px + event-solid rows moved | — |
| NEW `UI-ROUND4-PLAN.md`, `RDP-PREP-RUN-SHEET.md`, this file | plan of record · the compute-only RDP sheet (its §2 git route now dead — amended) · handoff | — |

Generated token artefacts moved with the tokens: `design/tokens/generated/{tokens.rs,json,css,svg,colormaps.json}` + `design/tokens/preview.html`.
**Mockups NOT regenerated** (design-mode.svg / perform-mode.svg still show dashed events): the
operator's D9 ruling supersedes the mockup vocabulary — declare it as a mockup-review finding
in S9 (findings 9–14 precedent).

## 3. S6 REMAINDER — the interaction batch, ready to write

All anchors verified this session. Work order: mod.rs → inset.rs → layout.rs → interact.rs →
tests. sparq-ui is zero-dependency: compile+test is seconds.

**A. `crates/sparq-ui/src/canvas/mod.rs`** — after `pub const MULT_ID` (L~70) add:
`CLK_ID = "sparq/mod/clk"`, `RMS_ID = "sparq/ana/rms"`, `QUANT_ID = "sparq/util/quant"`,
`RAND_ID = "sparq/mod/rand"` (house doc-comment style).

**B. `crates/sparq-ui/src/canvas/inset.rs`** —
* `Well` enum gains `ClockWheel` (D4), `Graph` (D13), `Keyboard` (D11), `Steps` (D12);
  `inset_well()` arms: `CLK_ID→ClockWheel`, `RMS_ID→Graph`, `QUANT_ID→Keyboard`,
  `RAND_ID→Steps` (existing: SCOPE/OUT_MAIN/ENV_AD/LFO/SVF). `node_bands` allocates the well
  height automatically via `well_for` — no layout change needed for band sizing.
* Pure helpers + gates: `clock_ring_rects(r: Rect) -> [Rect; 4]` (concentric centred squares,
  radii ¼·½·¾·1 of the half-extent — centre out: 4, 8, 16, 32); `ring_point(center, radius,
  phase) -> Vec2` (phase 0 at top, clockwise: `a = phase·TAU − FRAC_PI_2`);
  `keyboard_key(pitch01) -> usize` (`(round(p·120) % 12)` — key(0.5)=0, key(11/120)=11).
* Update the `inset_well` assertion tests beside the existing ones.

**C. `crates/sparq-ui/src/canvas/layout.rs`** —
* `WireLayout` gains `pub trim: Option<WireTrim>` + `pub trim_point: Vec2` (import `WireTrim`
  from model — the use line is `use crate::canvas::model::{Graph, Node, NodeSpec, PortRef, WireId};`).
  Construction site: the `Some(WireLayout { … })` in `compute`'s wire filter_map — add
  `trim: w.trim, trim_point: arc_midpoint(&points),`.
* `fn arc_midpoint(pts: &[Vec2]) -> Vec2` — cumulative-distance half-length walk; empty →
  `Vec2::ZERO`; single point → itself (sketch was written this session — straightforward).
* `Hit` gains `WireTrim(WireId)` and `Key(NodeId, usize)` (docs in house voice).
* `hit_test`: inside the `lod != Lod::Dot` block, AFTER `body_at` is defined and BEFORE the
  wire-ends loop: wires with `trim.is_some()` whose `trim_point` is within `capture` and not
  under a body → `Hit::WireTrim`. In the `lod == Full` block, beside the `step_cells` loop:
  `key_cells` → `Hit::Key(n.id, k)`.
* `node_size`: MULT special case — width `LAYOUT_CANVAS_NODE_WIDTH_DEFAULT × 0.25` (D6).
* `layout_node`'s two-sided port loop: MULT branch BEFORE it — all ports in ONE centred column
  at `x = pos.x + size.x·0.5`, rows on the same `header + … + r·row + row/2` stack (mult has no
  params/well so those terms are 0), `dir` as the manifest declares (all six are `in` — connect.rs's
  MULT special cases already make them bidirectional; positions move, semantics don't).
* `NodeLayout` gains `pub key_cells: Vec<Rect>` + `pub key_param: Option<usize>`; compute for
  `QUANT_ID` from the already-computed `well_band` (Option<Rect>, screen coords): 12 equal
  cells across the band, 15 % vertical inset; `key_param` = position of param id
  `"custom-mask"` (mirror the `step_cells` block's shape exactly). Add both to the
  `NodeLayout { … }` construction.
* Tests: mult dots centred at quarter width; `trim_point` is the arc midpoint of a straight
  wire; `Hit::WireTrim` wins over the grab and dies under a covering card; `Hit::Key` on a
  quant card's band; port-offset pins already adapted (they read the token).

**D. `crates/sparq-ui/src/canvas/interact.rs`** —
* Import `WireTrim` (model use line, L20).
* `Interaction` gains `Trim { wire: WireId, orig: Option<WireTrim>, acc_screen: Vec2, audio: bool }`.
* `apply_trim(graph, wire, to, coalesce) -> Vec<CanvasEvent>` — MIRROR `apply_param`'s
  coalescing exactly: `op_set_trim` → if coalesce and `history.top()` is `Op::SetTrim` for the
  same wire → `replace_top` keeping the ORIGINAL `from` + `note_patch(&op)` + return empty;
  else `commit(op)` + `Applied` text (`"wire trim: amp {:.2} · offset {:.2}"` / `"wire trim
  removed"`).
* `flip_key(graph, node, key)` — MIRROR `flip_step`: param id `"custom-mask"`, bit
  `1 << key.min(11)`; **Custom-mode gate first**: if the `scale` param's value rounds ≠ 0 →
  `Refused("the keyboard edits the CUSTOM scale — pick Custom from the scale list first")`.
* `activate()` (tap): add `Hit::Key(node, key)` arm (select node + `flip_key`); add
  `Hit::WireTrim(id)` arm → select wire + `apply_trim(…, None, false)` (tap the node = remove;
  the Note vocabulary says so on insert); extend the `Hit::Wire(id) | Hit::WireEnd(id, _)` arm:
  when the tapped wire has `trim: None` and `pos` is within `LAYOUT_TOUCH_PORT_CAPTURE_RADIUS`
  of its `trim_point` → `apply_trim(…, Some(WireTrim::identity()), false)` (tap the hover dot =
  insert); otherwise the existing selection code.
* `press_start`: `Hit::Key` → flip once, no drag (Step's rule); `Hit::WireTrim(id)` → select +
  `Interaction::Trim { orig: graph.wire(id).trim, audio: layout.wires…class == SignalClass::Audio, … }`
  + a Note naming the axes.
* `drag_update`: `Interaction::Trim` arm — accumulate `acc_screen += delta`;
  `amp = (orig.amp − acc.y/100).clamp(0,2)`; `offset` = `(orig.offset + acc.x/100).clamp(−1,1)`
  for non-audio, pinned 0 for audio (D15: DC never enters the audio path); `apply_trim(coalesce=true)`.
* `drag_end`: Trim arm → the `Applied` sentence from the graph's current trim (finish_param's shape).
* CANCEL path (find where `Interaction::Repatch` restores `orig` on cancel/Esc): Trim restores
  `orig` via `apply_trim(…, orig, false)`.
* `open_menu`'s hit match: `Hit::Key(id, _)` joins the Node arm; `Hit::WireTrim(id)` →
  `MenuTarget::Wire(id)`.
* Any OTHER exhaustive `Hit` matches the compiler names — fix each with the same vocabulary.
* Tests (mirror the existing harness in interact.rs's tests mod): insert → `Wire.trim ==
  Some(identity)` + one undo removes; drag coalescing = one history entry per gesture keeping
  the original `from`; audio-wire offset stays 0; key tap flips the mask bit through the param
  door (live ledger hears it: `patch_changes.param_nodes`); key tap in preset mode refuses in
  words; `SetTrim` marks `structural` in the ledger.

## 4. S7 — painter spec (`crates/sparq-app/src/ui/canvas_ui.rs`, feature `ui`; egui only — no wgpu needed until the `ui-window` cell)

Compute-then-draw throughout: every animation reads a published value or mirrors a declared
rule; nothing is invented. Data sources: `CanvasState.levels`/analysis drain (per-port
readings), the new cv publications (clk `phase` = manifest port 4; seq `step` = port 2; quant
`pitch` = port 2; rand `value`/`pos` = ports 1/3), and the tokens.

1. **Clip (D2)**: find the out/main meter-bar block (search `peak_l`/`peak_r`/amber hold).
   clip ⇔ `peak ≥ 1.0`: red cap segment at full scale + a per-channel LATCH held until
   STOP/session start (the peak-hold's own state machinery hosts it) + the word `CLIP` in the
   info band while latched. Audit smoke: a clipped render latches, a clean one doesn't.
2. **Clock wheel (D4)**: `Well::ClockWheel` — four rings via `clock_ring_rects` on the well
   band, numerals 4/8/16/32 centre-out, per-ring ticks, phase dot `ring_point(c, r_k,
   frac(phase × div_k))` with `div = [1,2,4,8]`; phase from the clk node's `phase`-port level
   reading; at rest: static, phase 0 (the declared at-rest rule).
3. **Seq lights (D5)**: the existing step-button painter (search `step_cells`) lights cell
   `floor(step_pub × steps)` in the event accent; pattern bits stay the authoritative drawing.
4. **RMS graph (D13)**: `Well::Graph` — a display-side rolling history (≈4 s at frame rate) of
   the level-port readings, hairline trace + the existing bar; history never enters the engine
   or the journal; restart starts empty (declared).
5. **Quantizer keyboard (D11)**: `Well::Keyboard` over `key_cells` — 12 keys (7 white + 5 black
   overlay geometry), the passing pitch lights `keyboard_key(pitch_pub)`; in Custom mode keys
   show mask membership (filled = in scale); taps already route through `Hit::Key`.
6. **Rand steps (D12)**: `Well::Steps` — `steps` bars; the UI MIRRORS `step_hash(seed, i)`
   (pure integer splitmix32 — reimplement in sparq-ui or read it from a shared spot; gate the
   display function against the PINNED seed-42 list so it can never drift from the module);
   cursor bar lit from `pos` publication.
7. **Trim nodes (D15)**: at `trim_point` — when `trim.is_some()`: a filled circle (radius ≈
   capture×0.5 while hovered/dragged, ×0.25 at rest — the shrink the operator named), amp shown
   as a fill arc/level; when `trim.is_none()` and the pointer hovers the wire (paint already
   receives the pointer pos — check how hover-silent works): a ghost dot at the midpoint.
8. **Mult (D6)**: skip port labels + name text for `MULT_ID` (the role dots stay — "the dots
   wear their role", the WORDS go); centred column comes from layout.
9. **SVF animation (D3)**: the inset `Well::Curve` and the inspector response plot recompute
   per frame from the EFFECTIVE cutoff: mirror the executor's rule `clamp(knob + cv ×
   half-range)` using the live level of the control wire whose `param` index is the cutoff
   (canvas knows `wire.param`; levels carry the source port's reading); resonance likewise if
   modulated. At rest = today's static curve (shots/goldens unmoved).
10. **Event solid (D9)**: free — the painter parses the encoding token (regen'd); VERIFY the
    legend + a control-wire regression (its dash is a separate hardcoded path around
    canvas_ui.rs L361-370 — must stay dashed).

## 5. S8 — bridge spec (`crates/sparq-app/src/bridge.rs` + gates)

In `build_with_map_at`, inside/after the wire loop, for wires with `trim: Some(t)`:
* **Audio class**: synthesize an INVISIBLE kernel `util/gain` (registry factory + manifest;
  gain param = `t.amp`; the glide discipline makes live drags zipper-free by construction):
  `kg.add_node(0)` BEFORE `Executor::build`; route `src → gain.in`, `gain.out → dst` instead of
  the direct edge; add its `NodeBuild`; give it a synthetic canvas key
  `0x8000_0000_0000_0000 | wire_id` in the keys vec (stable across rebuilds — adoption then
  keeps the gain's glide cell). Return the `wire → kernel gain node` map to the caller (extend
  the return or a small struct) so `LiveSession` can cross live amp drags as `set_params`
  (OPTIONAL fast path — the shipped behaviour is the structural re-stage, silent since D1).
* **CV class**: after `Executor::build`: `ex.set_cv_trim(dst_kid, w.to.index as u32, t.amp,
  t.offset)`.
* **Control wires** (`w.param.is_some()`): pass `(t.amp, t.offset)` into `add_param_mod`
  (replacing the identity literals); no trim → `(1.0, 0.0)`.
* Identity/no-trim → NO synthesis, no calls: renders bit-identical (gate it).
* Mult collapse interacts: a trim on a wire touching a mult dot — declare: trims ride the
  collapsed direct edge (apply per copy-edge via set_cv_trim / gain insertion the same way).
* Gates (r3_controls.rs style, registry+bridge harness): audio trim ≡ a hand-patched
  `util/gain` (hash-equal renders); identity ≡ untrimmed bit-exact; cv trim ≡ `set_cv_trim`
  arithmetic; control trim rides the D15 formula; **lfo→vca now CONNECTS** (D7's canvas
  verdict gate: `connect` verdict compatible + a rendered vca patch).

## 6. S9 — seal checklist (house discipline, in order)

1. Audit smokes (`crates/sparq-app/src/ui/headless.rs`, count moves from 43): trim-insert
   changes the render hash (the mechanical-claim shape); clip latch on a clipped render; the
   D7 vca connection smoke. Keep them hermetic (manual null).
2. Docs: `docs/ui/gestures.md` §4d — the cable-node vocabulary (hover dot, tap insert, drag
   axes, tap remove, audio offset inert, undo shape); `UI-CHANGES-PLAN.md` round-4 table;
   mockup-review finding: event dash → solid supersedes the mockups (D9).
3. `CHECKLIST.md`: the round-4 paragraph (this handoff's §1 numbers + defect #94's row beside
   #93 + the moved baselines: tests, 24 modules, audit count, device gate expectations);
   `SYNC.md`: the rev-4 paragraph + namelist; `README.md` status line (24 modules, the round's
   sentence, test count).
4. `scripts/test006.bat` grows the round-4 device steps (clip visible on a hot render, the
   wheel turns while playing, seq lights walk, keyboard lights + Custom taps, trim node
   inserted/dragged/heard, mult card quarter-width centred, vca takes the lfo wire, event
   wires solid, ports at 6 px) — the digest expectations move with them.
5. Full gate chain ON THIS BOX (1 GB — single job, ui-window cell single-job or declared):
   `cargo fmt --all --check` · `cargo clippy --workspace -- -D warnings` · `… --features ui` ·
   `… --features bootstrap-audio` · `cargo test --workspace` (record the new total) ·
   `cargo build --release` · `sparq selftest --golden` (9/9, `ba577186c988db21`) · the three
   pinned exec renders (`mod-demo` 1 s `1621e1f65b1b64e1`, `drum-demo` 2 s `f2303f13aa0cf299`,
   `demo` 2.8 s `53de3b1f3f40e3c9`) · `sparq ui --audit` (PASS, 0 failures, new smoke count) ·
   `sparq modules --strict` (24/24) · the four python gates · `just`-equivalent order per
   `scripts/gates.bat`.
6. Re-stamp: `python3 tools/sync_check.py --write --sync sparq-round4-2026-10-02` (the stamp
   does NOT ride the zip — the device re-stamps after applying, the rev-3 pattern).
7. Seal `sparq-update-2026-10-02.zip` as a FULL-TREE overlay (git is dead — private repo):
   exclude `.git`, `target/`, `Cargo.lock`, `logs/`, `*.wav`; include a READ-ME-FIRST with the
   apply steps, the gate numbers and the device asks (the operator's eyes/ears: vca patch,
   fold sound, quantizer keyboard, trim drag, wheel/step lights, clip LED, 6 px ports, solid
   events).
8. Fill `RDP-PREP-RUN-SHEET.md` §7-style verification table for the new state if a device
   digest round follows.

## 7. Environment realities (this sandbox — they bit this session)

* Toolchain lives IN the workspace: `/home/user/.tc/{cargo,rustup}` — every shell needs
  `export RUSTUP_HOME=/home/user/.tc/rustup CARGO_HOME=/home/user/.tc/cargo
  PATH=/home/user/.tc/cargo/bin:$PATH CARGO_BUILD_JOBS=1` (1 GB RAM — ALWAYS single job;
  the README's OOM note is this box).
* If a reset wipes it: no `curl` — `python3 -c "import urllib.request;
  urllib.request.urlretrieve('https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init','/tmp/rustup-init')"`,
  chmod +x, run with the CARGO_HOME/RUSTUP_HOME env; then `apt-get update && apt-get install
  -y gcc libc6-dev` (no linker otherwise; ~5 min; apt does not persist).
* `HOME` can move to `/tmp` between turns — use absolute `/home/user/...` paths in tools.
* `target/` and `.git` do NOT survive turn boundaries; `/home/user` files DO. No `ps`, `free`.
* If `modules/out/` vanishes again: restore from
  `/home/user/sparq-recovery/modules-out-main-sparqmod.toml` (hash it against
  `SYNC-STAMP.txt`'s row after the next re-stamp).
* Long builds: keep each bash call ≤ ~28 min (`timeout 1700 …` inside a 1790 s call); cargo
  resumes from `target/` within a turn.

## 8. Traps learned this round (do not re-learn them)

1. `edit_file` anchors: watch pre-existing `#[derive]` lines above structs (the Rms double-derive).
2. `usize::from(u32)` is cfg-gated — use `as usize`.
3. `ParamSet::at` out-of-range → `0.0` — appended params are safe for old-shape tests.
4. `Registry::create` → `Option`, not `Result`.
5. rms levels move per block (440 Hz ≠ integer cycles/block) — read source and destination
   from the SAME rendered block.
6. The gain glide lags a 4 Hz LFO — assert RATIOS/orderings on modulated peaks, not absolutes.
7. `EventBuf` temporaries die at statement end — bind them before builder chains.
8. Seq/rand cursors advance only when the event OUT is presented (sink-gated — house semantics).
9. Clock phase must wrap (`(interval − until) % interval`) or the beat-boundary block reads 1.0.
10. Manifests: APPEND ports/params only — per-type indices are pinned by goldens and gates.
11. `sync_check` FAIL + `build.bat`'s changed-file report are EXPECTED mid-round.
12. Adoption must NOT carry params (the staged build's canvas snapshot rules — pinned by
    `adoption_carries_state_but_params_belong_to_the_staged_build`); a cross_thread gate
    caught the first design getting this wrong.

## 9. Where the operator's 15 items stand

| # | Item | State |
|---|---|---|
| 1 | add-module transparency / delay fix (global) | **DONE** (D1 adoption; live-session e2e smoke optional in S9) |
| 2 | clipping on main meters | S7 |
| 3 | SVF graph animates with modulation | S7 |
| 4 | clock wheel (4/8/16/32, left of outs) | engine DONE (phase pub); S6 well + S7 draw |
| 5 | seq steps animate | engine DONE (step pub); S7 draw |
| 6 | mult ¼ width, centred, bidirectional, no labels | S6 layout + S7 labels-off (semantics already bidirectional — pin the gate) |
| 7 | VCA accepts control | **DONE** (D7 manifest; canvas verdict gate in S8) |
| 8 | fold module | **DONE** |
| 9 | event cables solid | **DONE** (tokens; S7 verifies legend + control-wire dash stays) |
| 10 | ports 6 px | **DONE** |
| 11 | note quantizer + dropdown + keyboard | module DONE; S6 key taps + dropdown (inspector list-picker, provisional table) + S7 keyboard |
| 12 | random step module + display | module DONE; S7 bars |
| 13 | rms slew + graph | slew DONE; S7 graph |
| 14 | delay sync input | **DONE** |
| 15 | cable nodes | model+executor DONE; S6 gestures + S7 draw + S8 bridge |

**The dropdown (item 11's inspector list-picker)** — not yet speered in code: a minimal popup
column over the `scale` row when tapped (15 names, Custom first), host-side name table keyed by
param id, documented provisional until manifest v1 `options[]`. Lives in S6/S7 (inspector.rs +
canvas_ui.rs); keep it small — buttons-and-words, no scrolling machinery beyond the sheet's.
