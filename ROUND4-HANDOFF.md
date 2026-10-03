# ROUND4-HANDOFF.md — operator round 4, mid-round session handoff (2026-10-02)

**Purpose:** the first file the NEXT session reads (besides CHECKLIST.md's new head paragraph).

---

## 0. CONTINUATION-SESSION UPDATE (2026-10-02, later the same day) — read this first

**The git route came back, but it carries NO round-4 code.** `github.com/bitseq27/sparq` is
public/clonable again; its head commit (871d725, 14:45) added ONLY this file and
RDP-PREP-RUN-SHEET.md — the round-4 CODE (S1–S5 + the S6 model layer) never reached git (the
checkpoint zip was the only delivery path, and no zip was present in the fresh sandbox:
`/home/user/.tc`, `/home/user/sparq-recovery` and `sparq-round4-checkpoint-1.zip` all gone).
A fresh clone is therefore the **rev-3 codebase + the round-4 docs** (CHECKLIST head still the
rev-3 paragraph; `UI-ROUND4-PLAN.md` absent; sync stamp `sync-ui-round-2026-10-01`; 21 modules;
BUILTINS 21; port offset 4; vca bipolar; event encoding still `dashed-6-3`).

**Operator ruling this session (question tool):** *rebuild the S6 prerequisite on the rev-3
tree, then do S6* — S1–S5 stay OWED until a zip surfaces or they are rebuilt (S7's painter and
S8's bridge need them; S6 does not — sparq-ui is self-contained).

**S6 IS NOW DONE ON THIS TREE** — the model layer rebuilt from §2's D15 row and the whole §3
interaction batch written from this file's spec (mod.rs → inset.rs → layout.rs → interact.rs →
tests), plus the one compiler-named fix outside sparq-ui: canvas_ui.rs's `Well` match grew an
honest stub arm for the four new wells (S7 paints them; until then the reserved bands are
empty space, nothing faked). Measured, all on this box:

* **sparq-ui: 199/199** (rev-3's 182 + 17 new gates: 3 model-trim + 3 inset round-4 +
  5 layout round-4 + 6 interact round-4).
* **`cargo test --workspace`: 849 passed / 0 failed / 1 ignored** (832 + 17; the ignored one is
  `alloc.rs`'s doc test, standing).
* `cargo fmt --all --check` CLEAN · `cargo clippy --workspace --all-targets -D warnings` CLEAN ·
  `-p sparq-app --features bootstrap-audio` CLEAN (needed `apt-get install pkg-config
  libasound2-dev` — apt does not persist) · `-p sparq-app --features ui --all-targets` CLEAN.
* Python gates ALL PASS: token_gen --check (0 to write), token_audit, unsafe_audit,
  module_docs --check **21/21** (the rev-3 module set; 24/24 waits on the S1–S5 rebuild).
* **`sparq ui --audit`: PASS, 0 failures** — the S6 geometry moves (mult strip, wells on
  clk/rms/quant/rand, key cells, trim points) break NO existing smoke.
* `sync_check`: **FAIL BY DESIGN** — 6 stamp-covered files moved (model.rs, layout.rs,
  interact.rs, inset.rs, canvas/mod.rs in sparq-ui; canvas_ui.rs in sparq-app). Re-stamp is
  S9. The Cargo.lock `warn SIZE` line is PRE-EXISTING (stamp vs the re-initialised repo's
  committed lock), not this session's.
* Toolchain recovered per §7: rustup/rustc/cargo **1.99.0** into `/home/user/.tc`, gcc 12.2 via
  apt (slow mirror — parallel .deb download + `apt-get -f install` was the working route).

**Defect #95 (new, recorded; PRE-EXISTING, not round-4):** the ui-gated live test
`a_param_edit_crosses_the_command_ring_without_a_restage_and_the_level_follows`
(crates/sparq-app/src/ui/live.rs:1081) fails DETERMINISTICALLY on the PRISTINE rev-3 tree —
proven by `git stash`: same bytes with and without this session's edits
(`0.49982867 → 0.4400266`; the sine amp's glide lags the test's 4-block window — trap #6's
class). It is not part of the canonical `cargo test --workspace` gate (ui-gated), so rev-3
sealed with it latent. Operator triage owed: fix the test's window or the level's ramp; do not
paper over it inside S6.

**Judgement calls this session (the spec's silences, filled conservatively):**
1. `Interaction::Trim` accumulates RAW deltas (no fine-scale `scale` factor) — §3's literal
   `acc_screen += delta`.
2. "audio" for the pinned-0 offset is `class == SignalClass::Audio` literally — a SPATIAL wire
   currently gets a live offset; S8's bridge ruling should revisit (spatial IS an audio path).
3. `make_wires_straight` recomputes `trim_point` — not in §3's letter, required by the
   function's own contract ("the hit-test and the painter keep agreeing").
4. Tap-insert followed IMMEDIATELY by a node drag MERGES into the insert's history entry (the
   literal `apply_param` coalescing rule: top is `SetTrim` for the same wire → replace). One
   undo then removes insert+drag together. The pinned coalescing gate therefore starts from a
   pre-set trim (setup, no history), per §3's "keeping the original from".
5. The item-11 DROPDOWN (inspector list-picker) is NOT built: its 15 scale names/order live in
   the lost S1–S5 `modules.rs` (`QUANT_SCALES`) and will not be guessed. Rebuild it with S1–S5
   or from the operator's list.

**What remains of round 4:** ~~the S1–S5 rebuild~~ (DONE — see §0b below), S7 (painter — §4's spec
stands; its wells' stub arm is in place), S8 (bridge — §5), S9 (seal — §6, now also: defect #95's
row, the §0/§0b numbers, and this file's §3 marked done).

---

## 0b. S1–S5 REBUILT (2026-10-02, same continuation session) — measured, all green

The operator ruled (this session): rebuild the foundation from §2's spec on this tree. DONE —
every §2 row rebuilt: executor D1 adoption + D15 cv-trim, the engine boundary hook
(`adopt_runtime` then `inherit_runtime`; the single-owner `Engine::stage` deliberately does NOT
adopt), hotswap's `boundary` is now `FnOnce(&mut T, &mut T)`, the module batch (clk `phase`,
seq `step`, rms stateful + `slew`, delay `sync` tail-dump, `Fold`+`fold_tri`,
`Quant`+`QUANT_SCALES`+`snap_to_scale`, `RandStep`+`step_hash`, BUILTINS 24), the bridge's
canvas-key stamping + identity `(1.0, 0.0)` param-mod trim args (S8 replaces them from
`wire.trim`), the five manifest bumps + three new manifests, tokens D9/D10 regenerated,
gestures.md rows moved, docs 24/24 regenerated. Measured, all on this box:

* **`cargo test --workspace`: 886 / 0 / 1 ignored** — 832 rev-3 + 17 S6 + **37 new**:
  11 `r4_modules` + 12 `r4_new_modules` + 8 adoption + 3 executor trim + 2 mixer cv-trim +
  1 r3 control-trim. (§1's split was 33; this rebuild's is 37 — same behaviours, one extra
  allocation gate and two extra trim gates, honest count.)
* **The pinned release renders are EXACT**: `selftest --golden` PASS 9/9 with the wo005 golden
  `ba577186c988db21`; exec renders `mod-demo` 1 s `1621e1f65b1b64e1`, `drum-demo` 2 s
  `f2303f13aa0cf299`, `demo` 2.8 s `53de3b1f3f40e3c9`. The identity branches hold.
* `sparq ui --audit` (release): PASS, 0 failures. `sparq modules --strict`: 24/24.
  fmt CLEAN; clippy workspace + bootstrap-audio + ui cells CLEAN; the four python gates PASS
  (module_docs 24/24, token_gen 0-to-write).
* **mutation_stress: `8143e1ddfd8fb261` — HELD across the rebuild** (bit-identical to the
  PRISTINE base on this box, proven by stash; 10 001 blocks · 7208 swaps · 2792 refused, the
  same evidence line). PROVENANCE NOTE for S9: §1's recorded `7bb06379bd6845e5` does NOT
  reproduce on the pristine clone here (rustc 1.99 vs the stamp's 1.98 era, or the
  rebuilt-history repo's base differing from the checkpoint's) — yet the wo005 golden hash
  DOES reproduce exactly, so the difference is stress-harness-specific, recorded not hidden.
* The `step_hash` the pinned seed-42 ring forced: **lowbias32 finalizer** (the splitmix32
  family) over `seed ^ (i × 0x9E37_79B9)`, top 24 bits — the ring reproduces EXACTLY
  (`r4_new_modules` pins it; `step_hash`/`step_value`/`fold_tri`/`snap_to_scale`/`QUANT_SCALES`
  are `pub` in modules.rs for S7's mirror gates).
* Port indices match §4's painter spec: clk `phase` = 4, seq `step` = 2, quant `pitch-out` = 2
  (ports: pitch-in 0, trig-in 1), rand `value` = 1 / `pos` = 3 (trig-in 0, trig-out 2).
* Adoption's declared edges: instances cross with their INTERNAL state (including a module's
  own param-glide memory — the sine's amp glide continues, measured in the shape gate);
  PARAMS never cross (the named gate); executor-owned state (delay hists, comp lines, cv
  knots, mod scalars) crosses by wire identity; `adopt_runtime` is ALLOCATION-FREE (the
  pairing scratch lives in pre-built `ExecNode` fields — the cross_thread allocation gate
  caught the first Vec-collecting design, exactly the house way).
* `sync_check`: FAIL BY DESIGN, 17 stamp-covered files moved (11 code + 5 manifests + the FP
  line). Re-stamp at S9. Defect #95 stands untouched (the ui cell is 21/1, the 1 pre-existing).

**Owed after this:** ~~S7~~ (DONE — see §0c below), S8 (bridge: gain synthesis for audio trims,
`set_cv_trim` calls, `wire.trim` → `add_param_mod`, the lfo→vca verdict gate, the
identity-bit-exact gate), S9 (seal: the numbers below, the re-stamp, the zip), the item-11
dropdown picker (`QUANT_SCALES` is the name table it waits for; NOT on §4's S7 list — deferred
by ruling, not by oversight).

---

## 0c. S7 PAINTER BATCH DONE (2026-10-02, same continuation session) — measured, all green

§4's ten items, built in `canvas_ui.rs` + the state feeds it reads (compute-then-draw
throughout: every animation reads a publication or mirrors a declared rule; nothing invented):

1. **Clip (D2)** — `StereoMeter` grows `clip_l`/`clip_r`: latched in the session's own meter
   update (`peak ≥ CLIP_THRESHOLD = 1.0`, held because the latch lives in the session state —
   STOP ends the session, a fresh one starts unlatched, by construction). Painter: red cap
   segment AT FULL SCALE on the bar + the word `CLIP` right-aligned on the info band (the
   driver lines reserve its width). Smoke 51 pins: hot latches (caps + word), holds across
   frames, STOP clears, clean never does.
2. **Clock wheel (D4)** — `draw_clock_wheel`: four rings on `clock_ring_rects`, cardinal
   ticks, numerals 4/8/16/32 centre-out on the top-left diagonal (Full only — Simplified is
   no-text), phase dots `ring_point(c, rₖ, frac(phase × divₖ))`, `div = [1,2,4,8]`, event
   accent. Phase = the clk's 0th cv-out through `nth_cv_out` (no port literal in the
   painter); at rest the level set is empty → phase 0, static (declared rule). Smoke 52 pins
   the pipeline END-TO-END: 304 pumped blocks → the painter's level word == 19 456/24 000,
   wrapped, advancing — module → ring → drain → levels → painter, measured.
3. **Seq lights (D5)** — the step-button painter lights cell `floor(step_pub × steps)` with
   an EVENT-accent ring OVER the pattern fill (the bits stay the authority); the cursor reads
   the module's own `step` publication, never a display-side counter; no light at rest.
4. **RMS graph (D13)** — `LevelHistory`/`LevelHistories` model in sparq-ui (240-frame window
   ≈ 4 s at the nominal cadence, sanitised at the door, grow-then-scroll polyline, 4 gates);
   the shell pushes one word per LIVE frame and CLEARS at session start (restart starts
   empty, declared; a pause holds — no pushes while the level set is empty). Painter:
   hairline cv-accent trace + the live word's floor bar in CV colour — NOT the data fill:
   the meter bars are out/main's alone, and smoke 38 caught the first draft that painted it
   data-coloured. The 2026-09-30 ruling enforces itself.
5. **Quantizer keyboard (D11)** — 12 equal cells (the hit-test's own geometry — every drawn
   key lives inside the cell that routes its tap), 7 naturals full-height, 5 accidentals the
   shorter/darker overlay. FILLED = membership in the ACTIVE scale: Custom reads the node's
   `custom-mask`; a preset reads the module's own `QUANT_SCALES` (one source — the display
   can never revoice a scale). The passing pitch lights its key from the quantized-pitch
   publication through `inset::keyboard_key` — the lit key and the snapped note are the same
   arithmetic.
6. **Rand steps (D12)** — bars mirror `inset::rand_step_value` (the display-side copy of the
   module's pinned hash — sparq-ui is zero-dependency, so the mirror lives there and is
   pinned from BOTH sides: the seed-42 ring in each crate's own gate PLUS the cross-crate
   sweep in `bridge.rs`'s tests, 6 seeds × 64 steps + the span constants equal). Cursor bar
   from the `pos` publication in the class glow; zero values keep a visible floor tick.
7. **Trim nodes (D15)** — filled handle at `trim_point`: capture × 0.25 at rest, × 0.5 under
   the pointer or mid-drag (the shrink the operator named), amp as the inner level disc
   (identity = half-full, the 2.0 rail = full). A CLEAN wire shows the hover GHOST at the
   same point and the same capture radius the tap-insert door uses — the drawing and the
   gesture read one geometry. Pointer feed: the shell's new `hover` field (last contact
   position; mouse hover counts — pointer.rs's rule). Dot-LOD gated like the hit-test.
8. **Mult (D6)** — title, category word and port names/letters suppressed: the dots wear
   their role, the WORDS are gone (the centred column came with S6's layout).
9. **SVF animation (D3)** — `rebuild_curves` computes EFFECTIVE params: every control wire
   landed on a float param applies the executor's own `clamp(knob + cv × half-range)` with
   the wire's published source level (`wire_level`); the cache key carries the effective
   params, so a modulated curve recomputes per frame and a still one doesn't. At rest (empty
   level set) the params are the knob snapshot — today's static curve; every pre-existing
   response gate stands.
10. **Event solid (D9)** — the painter parses the regenerated token (free); smoke 53 pins:
    the event encoding is `solid-stroke` with no dash numbers, the five-class table stands,
    and the control-wire dash's token pair stands — the hardcoded §4c control path untouched.

**Measured (all on this box):** `cargo test --workspace` **893 / 0 / 1 ignored** (886 + 6
sparq-ui model gates + 1 bridge mirror pin); sparq-ui 205/205; the ui-feature cell 22+1 / 1
(the 1 = defect #95, pre-existing, unchanged); `sparq ui --audit` **PASS 0 failures, 58 smoke
lines** (55 + the 3 round-4 smokes); release `selftest --golden` PASS 9/9 `ba577186c988db21`;
exec renders EXACT (`1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9`);
`modules --strict` 24/24; fmt CLEAN; clippy workspace + bootstrap-audio + ui cells 0
diagnostics; the four python gates PASS. `sync_check`: FAIL BY DESIGN, 21 files (S9 re-stamps).

**Remaining:** ~~S8~~ (DONE — see §0d below), S9 (seal), the dropdown picker, defect #95's
triage.

---

## 0d. S8 BRIDGE BATCH DONE (2026-10-02, same continuation session) — measured, all green

§5 built in `bridge.rs` (+ its own test mod — sparq-app has NO lib target, so bridge gates
live in-src, the house precedent):

* **Audio trims** synthesise an invisible `util/gain` BEFORE `Executor::build`: registry
  module + manifest unmodified (the hand-patch IS the synthesis — the ≡ gate measures the
  sentence), `t.amp` as its param, `src → gain.in`, `gain.out → dst` replacing the direct
  edge. The offset is NEVER READ for audio — the DC rule is structural, not a promise.
* **Synthetic canvas keys** `0x8000_0000_0000_0000 | wire_id` in the keys vec, stable across
  rebuilds: a re-staged amp drag ADOPTS the running gain (its glide cell mid-chase), so live
  drags are zipper-free by construction — defect #85's glide doing exactly the job it was
  fixed for. Canvas ids are u32, so the high bit is a collision-free namespace.
* **`TrimGainMap`** (wire id → kernel gain) returned from both map doors (`build_with_map`,
  `build_with_map_at` now 4-tuples); both live-session call sites DROP it with the declared
  sentence: the shipped behaviour is the structural re-stage (silent since D1), the
  command-ring fast path is LATER's and the map is the door it walks through.
* **CV trims** record `(dst_kid, port, amp, offset)` and ride `set_cv_trim` after the build
  (the wire must exist before a trim can ride it). **Control trims** hand `(t.amp, t.offset)`
  to `add_param_mod`; absent/identity → `(1.0, 0.0)`, bit-exact through the executor's branch.
* **Mult collapse** (the declared interaction): feed and copy trims COMPOSE affinely onto the
  one direct edge — `(v·a₁+o₁)·a₂+o₂ = v·(a₁a₂)+(o₁a₂+o₂)` (`compose_trims`); audio copies
  synthesise per copy-edge, cv copies ride `set_cv_trim`, control copies ride their own
  wire's param-mod (never a kernel edge).
* **Refusals in words**: event/data cable nodes ("no amplitude to trim — a trigger's word is
  its sample"), spatial ("no per-set gain module yet — the spatial phase ships one"),
  neutral/gpu/atom ("report this"). Mono-sink channel arithmetic is DECLARED in-code: the
  insertion is the hand-patch equivalent down to the matrix's documented conversions (every
  first-party audio sink is stereo-or-variable, so today's leg is transparent).

**The six gates** (bridge.rs tests): audio ≡ hand-patched gain HASH-FOR-HASH (+ audible vs
untrimmed); identity synthesises NOTHING (the map is the mechanical claim) and renders
bit-identical; cv trim ≡ the manual `set_cv_trim` door (right node/port/values, + audible);
control trim ≡ the executor-level `add_param_mod` replication HASH-IDENTICAL; **the vca takes
the lfo wire** (D7: verdict Compatible — the matrix unmoved — and the rendered patch peaks in
(0.85, 1.0], the window not the instant, trap #6 honoured); trimmed mult copy ≡ the composed
direct wire (+ non-vacuous). TWO gates first measured nothing — the svf's `mod` depth default
0 silences its cv input — caught by the non-vacuous asserts and fixed at depth 1.0: the
house pattern earning its keep inside one session.

**Measured:** `cargo test --workspace` **899 / 0 / 1 ignored** (893 + 6); sparq-app bin 19/19;
the ui-feature cell 28 + 1 (the 1 = defect #95, pre-existing, unchanged); fmt CLEAN; clippy
workspace + ui + bootstrap-audio cells 0 diagnostics; python gates 4 PASS; release
`selftest --golden` 9/9 `ba577186c988db21`, exec renders `1621e1f65b1b64e1` /
`f2303f13aa0cf299` / `53de3b1f3f40e3c9` EXACT; `sparq ui --audit` PASS 0 failures (58 smoke
lines); `modules --strict` 24/24; `sync_check` FAIL BY DESIGN (21 files — S9 re-stamps).

**Remaining:** S9 (seal — §6: the trim-insert audit smoke is now possible and owed, the
gestures.md §4d cable-node vocabulary, UI-CHANGES-PLAN round-4 table, CHECKLIST/SYNC/README
paragraphs, test006 device steps, the full chain on this box, the re-stamp, the full-tree zip,
the mockup-review finding), the dropdown picker, defect #95's triage, the mutation-hash
provenance note (0b).

## 0e. S9 SEAL DONE — ROUND 4 IS SEALED (2026-10-02, same continuation session) — measured, all green

§6's checklist completed in order:

1. **Audit smoke 54** — the trim-insert mechanical claim through the REAL gesture door: hover →
   tap-insert at identity → amp drag → render hash MOVES (and identity-insert renders
   bit-identical), tap-remove restores, one coalesced undo. Unwrap-free, clippy-clean. Audit
   total: **PASS, 0 failures, 59 [PASS] lines** (51–53 from S7: clip latch, the phase pipeline
   `19456/24000`, the encoding pin).
2. **Docs** — `docs/ui/gestures.md` §4f (the cable-node vocabulary: hover dot, tap insert, drag
   axes with the audio-offset inert rule, tap remove, undo shape; §4d in the checklist's plan,
   §4f where the section actually landed) + §6's event row + §7's round-4 paragraph;
   `UI-CHANGES-PLAN.md`'s round-4 sheet; `mockup-review.md` findings 25–26 (event solid
   supersedes the mockups, 6 px ports); CHECKLIST round-4 paragraph + Last-updated; SYNC.md
   rev-4 head + namelist + THE ASK (rev-3 demoted to Previous bundle); README status line
   (24 modules); `RDP-PREP-RUN-SHEET.md` §7b table; `scripts/test006.bat` steps **L–U** (10
   prompts, ASCII-clean per `check_text_io`, digest expectations 899 / 59 / 24).
3. **The full gate chain ON THIS BOX:** fmt CLEAN · clippy workspace + `ui` + `bootstrap-audio`
   cells 0 diagnostics · `cargo test --workspace` **899 / 0 / 1 ignored** (sparq-ui 205,
   sparq-app bin 19, the ui-feature cell 28 + 1 = defect #95 pre-existing) · release
   `selftest --golden` **9/9 `ba577186c988db21`** · the three pinned exec renders EXACT
   (`1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9`) · `ui --audit` PASS ·
   `modules --strict` **24/24** · the four python gates PASS · mutation-stress
   **`8143e1ddfd8fb261`** re-verified on the final sealed tree (the 0b provenance note stands:
   the handoff's `7bb06379bd6845e5` does not reproduce on this box).
4. **Re-stamp:** `sparq-round4-2026-10-02`, **162 files, `src 96f/2694477B`**, `sync_check` OK.
   `READ-ME-FIRST.txt` overwritten with the rev-4 pack instructions (apply steps, the gate
   numbers, THE ASK, the provenance notes).

**Defect #96 (new, REMEDIED — the #93 trap's third strike, at the S9 wrap-up boundary):** the
between-turn reset dropped `modules/out/` from the SEALED tree (plus `.git`, `target/` and the
apt packages, all documented). `sync_check` named exactly ONE file of 162 MISSING — every other
stamped file came back byte-identical, which is the seal's own proof that nothing else moved.
No byte-exact copy survived anywhere: repo 404 (private — raw AND api), the three checkpoint
zips are changed-file overlays that never carried the file, and the sandbox's undo snapshot
applies the same `out/` exclusion (checked). Remedied per #94: REBUILT from
`docs/modules/out-main.md` + the `OutMain` implementation, its header declares the
reconstruction, and PROVEN semantically identical at both layers — `module_docs.py`
regeneration byte-matches the checked-in doc (24/24) and the real Rust `decode::decode` (the
call `Registry::register` and `modules --strict` ride, run from a throwaway crate OUTSIDE the
tree with a path-dep on `sparq-module-api`) validates **24/24**, out/main's parsed vocabulary
eyeballed field-for-field against the doc. Re-stamped: the row moved (`3b0a619f…`/2 544 B →
`86608e10…`/4 087 B — comments are the only difference); the fp did NOT (`src 96f/2694477B`
walks `.rs` files only — the reason every doc quoting the fp stays true). #93's durable-copy
remedy RE-ESTABLISHED: `/home/user/sparq-recovery/modules-out-main-sparqmod.toml` (outside the
repo, sha256 `86608e10…` = the stamp row). Docs updated with the row: CHECKLIST (the #96 row +
the round-4 paragraph's "NOT APPLICABLE to this tree" sentence corrected — it was falsified by
this very turn), SYNC.md, READ-ME-FIRST (three provenance notes), this §0e. Environment
footnote, recorded: this sandbox's cargo ENOENTs on absolute `/home/user/...` path-deps that
bash demonstrably sees — relative paths resolve; the throwaway verifier used them.

**The zip:** `sparq-update-2026-10-02.zip` sealed AFTER this section (the pack carries this
file, so a byte-size quoted inside itself cannot converge — the build's asserts are the
record): full tree at repo-root-relative paths; excluded `.git`, `target/`, `logs/`,
`__pycache__`, `Cargo.lock`, `SYNC-STAMP.txt` (the stamp never rides — the device re-stamps,
rev-3 pattern), `*.wav`, `*.pyc`; asserted present: `modules/out/main/sparqmod.toml` (the #93
trap's own assert), `READ-ME-FIRST.txt`, `crates/sparq-app/src/bridge.rs`, the three new
manifests (`fx/fold`, `util/quant`, `mod/rand`), both r4 test files; asserted absent: the stamp,
the lock, any wav.

**Rollback points kept:** `/home/user/sync.zip` (S8-era overlay), `sync-s6-checkpoint.zip`,
`sync-s7-checkpoint.zip` — all predate the seal; the update zip supersedes them.

**Owed after round 4 (unchanged):** the dropdown picker (item 11's list-picker — `QUANT_SCALES`
exists, the widget awaits an operator go); defect #95's operator triage (move the window or
snap the glide — a silent fix either way would be a lie); D15's command-ring fast path stays
declared LATER behind the bridge's `TrimGainMap` door; device rounds: test006 **A–U** +
test004 attempt 4 on SATURN, evidence = `test006-digest.log` + `logs\ui-digest.log`.

**ROUND 4 IS SEALED.** The full gate chain is green on the final tree, the stamp is written, the
pack is built, and every refusal and provenance note is in words where the receiver will read it.

---
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

**DONE 2026-10-02 (continuation session): this whole section was built from this spec on the
rev-3 tree and is green — see §0 for the measured numbers and the judgement calls. The text
below is kept verbatim as the build record.**

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
