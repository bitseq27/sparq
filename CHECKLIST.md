# CHECKLIST.md — session handoff (live document)

**Purpose:** the first file a new session reads. Says what is done, what is in flight, and what to do
next. Updated at the end of every session (and mid-session when state changes). This file supersedes
`RESUME.md`'s handoff role (RESUME.md is the 2026-09-22 snapshot; keep it as history). For deep
history see `PHASE0-WORKORDERS.md` §2.1 (status table) and its build-log sections — read **by line
range**, never whole (see `RESUME.md` §1 for why).

**Last updated:** 2026-09-28, fifth session (handoff. The device moved: **test004 attempt 2 ran
on SATURN** — both waiting bundles confirmed APPLIED by its build line (`sync_check: OK - 149
files match sync wo008-inc6`) — and it split like attempt 1 did, only further along: **exclusive
OPENED for the first time** (`i24-in-32 (converting)` @ 96 kHz on the UMC 204HD; caps, shared
unplug→`Removed`, recovery, the 2 h shared soak and all three conformance suites PASSED) — and
then the stream **stalled**: the open landed at **288 fr (3.00 ms)**, the driver's reported
minimum/alignment granularity, and the driver could not sustain what it had accepted (one ~33 ms
stall every ~62.5 ms, 159 late wakes / 10 s, HALF throughput, drift −49 %, no clean tone,
rc 1). **Defect #89** — the third lying-`min` face, and the worst: `Initialize` says yes,
runtime says no, and no open-path probe can see it. **WO-006 increment 1.4** was built the same
day and sealed as **`sync-wo006-inc14.zip` (15 entries)** — all three doors the 3 ms ask could
have come through are closed (default-first ladder for coarse engines; the alignment two-step
rounds the ask UP to the granularity instead of adopting it; a silent-shrink guard with an
honest open line), each as pure data in `hal/period.rs`, Linux-pinned on the exact attempt-2
device shape. The session also shipped the operator's ask: **`tools/log_digest.py`** — the test
scripts now end by writing a compact `-digest.log` copy (verdict lines byte-identical; only
periodic status runs, cargo chatter and duplicate runs collapse; attempt 2's 67.6 KB log →
31.0 KB) and **the digest is what gets sent back**. Sandbox-green: **757 tests**, goldens
untouched by construction, both MSVC `hal-wasapi` cells clean. **Next step is DEVICE EVIDENCE:
apply `sync-wo006-inc14.zip`, re-run `test004`, send back `test004-digest.log`.**
Previous handoff (fourth session): **WO-008 increment 6** built and sealed as
**`sync-wo008-inc6.zip` (17 entries)** — now APPLIED on the device: the host now PERFORMS the whole G4 vocabulary — `spline`
is the parabola through the last three block values (equivalently a cubic Hermite with causal
second-order tangents; one compiled copy in `CvInterp`, the same sentence in the matrix cell /
§17 / the schema row / the author guide, pinned by the widened drift gate), exact for
quadratic-in-block-index sweeps, bit-identical to `linear` on collinear knots, `linear`'s arrival
timing, f64 with one rounding per frame, clamped to the wire's declared range (host-made momentum
overshoot is bounded there and nowhere else — an out-of-range knot under hold/linear stays a
visible SOURCE bug). The build refusal retired; `CvPlan` grew the two-slot history + the range;
still zero audio-path allocations. `tests/cv_spline.rs` (8 gates) + 2 `port.rs` unit pins +
contract_v1's refusal gate replaced by a behaviour gate. Sandbox-green: **752 tests**, every
pre-existing golden unchanged, stress hash unmoved (debug AND release), all runnable clippy cells
clean, selftest 9/9, ui-audit PASS (25 smokes), stamp **`src 92f/1986012B`**.
**`sync-wo008-inc6.zip` is WAITING for the device, stacked on the still-WAITING
`sync-wo014-inc6.zip`.** **The next step is still DEVICE EVIDENCE, not code**: apply both waiting
bundles, then `test004` (the 🔴 exclusive-acceptance blocker), `test006` (steps F–I), and
`gates.bat` on SATURN, with the logs wanted back.)

---

## Current position

- **Device state:** every sealed bundle through **`sync-wo008-inc6.zip` is APPLIED on SATURN** —
  proven by test004 attempt 2's own build line (2026-09-28): `sync_check: OK - 149 files match
  sync wo008-inc6, src 92f/1986012B`. **WAITING: `sync-wo006-inc14.zip` (15 entries, this
  session — the #89 default-first period ladder + the #90 script-honesty fix + the log digest
  tooling).** Once it lands, the device runs cover eleven increments of baseline — **`test004`
  attempt 3 (exclusive acceptance — the #89 fix, still the 🔴 blocker; send back
  `test004-digest.log`)**, `test006`
  (canvas window session — after RENDER WAV the sine/gain wires light with live levels, **and the
  steps F–I: the rename sheet under real keys, the inspector two-finger scroll, the LOD walk vs
  the mockup, the cv wire lighting from its own value**; send back the digests), `gates.bat`
  (**expected test count is now 757**, stamp = whatever `SYNC-STAMP.txt` says
  (`src 92f/2001521B`), `sparq modules --strict`
  lists **17** (the mixer row reads v0.2.0, 13 ports, 24 params), `selftest --golden` prints
  **PASS (9 gates)**, `ui --audit` **PASS (25 smokes)**; `log_check.py`'s BASELINE moves with
  every seal since defect #83 — the inc-1.4 bundle carries the moved one), and the
  WO-008 **loaded soak** (200 modules, 30 min) when the HAL and the machine are both free.
  WO-009's two device boxes: the **30-minute drift measurement** and a **listened tempo sweep**.
  Evidence artefacts — **defect #85: run them with their pinned durations or the hashes will not
  match the goldens** (the CLI default renders 5 s): `sparq exec --patch drum-demo --seconds 2`
  (transport-driven, hash `f2303f13aa0cf299`), `--patch mod-demo --seconds 1`
  (`1621e1f65b1b64e1`), `--patch demo --seconds 2.8` (`53de3b1f3f40e3c9`).
- **WO-006 increment 1.4 is BUILT and sandbox-green — the defect-#89 default-first exclusive
  period ladder + the compact log digest:** `hal/period.rs` reorders (a driver whose reported
  minimum sits ABOVE the sparq block gets asked its DEFAULT period first — the number its engine
  actually runs; the min-clamped ask is demoted to fallback; sub-block engines keep the honest
  low-latency ask first, #79's shape unchanged), the `BUFFER_SIZE_NOT_ALIGNED` two-step rounds
  the ask UP to the driver's granularity (960 fr ask × 288 fr granularity → 1152 fr = 12.000 ms)
  instead of adopting it, and a post-open guard treats an allocation < ¾ of the accepted ask as
  an undeclared granularity — one rounded-up re-ask, else the open line prints the mismatch
  (`(3.00 ms, ask 960 fr)`) rather than dressing it as clean. 6 new Linux unit gates + 1
  replaced in place (the attempt-2 device shape is pinned twice: `the_defect_89_device_asks_its_
  default_before_its_min`, `the_alignment_two_step_rounds_the_ask_up_never_down`); the pump
  untouched (Period ≠ block absorbs any period). **Defect #90** fixed beside it: `test004.bat`'s
  summary said "exclusive still refused" when attempt 2 had OPENED and run rough — it now
  distinguishes refused from opened-but-not-clean (`EXCL_OPENED`), and its banner/[02]/[04]/hint
  texts are current for the first time since inc 1.2. **And `tools/log_digest.py`** (+
  `scripts\digest.bat` + end-of-run hooks in test004/test006/gates): compact `-digest.log`
  copies of the device logs — every verdict byte-identical, three repetition classes collapsed —
  because a 2 h soak's 240 status lines were drowning the dozen lines that decide the run
  (attempt 2: 67 569 B → 30 994 B, −54 %; `--self-test` 16/16). Sealed as
  **`sync wo006-inc14`**; device contract = `scripts\test004.bat` + `WO006-INC14-RUN-SHEET.md`.
- **WO-008 increment 6 is BUILT and sandbox-green — `cv_interp = "spline"`, the G4 vocabulary
  performed in full:** the checklist's sandbox-track item 8, second entry — named there as the
  next candidate; the plan of record is `WO008-INC6-PLAN.md` (eight decisions, recorded before
  the code). **The curve:** the parabola through the last three block values —
  `v(t) = prev2·t(t−1)/2 + prev·(1−t²) + cur·t(t+1)/2`, `t = i/frames` — equivalently a cubic
  Hermite whose start tangent is the central difference `(cur−prev2)/2` and whose end tangent is
  the second-order backward estimate `(3cur−4prev+prev2)/2` (four constraints, one cubic, the same
  curve; the equivalence is stated in `CvInterp::Spline`'s doc, the ONE compiled copy). Non-causal
  splines REJECTED on the record: true Catmull-Rom needs `v[N+1]` and the natural cubic needs a
  global solve — both buy smoothness with a block of LATENCY, which would make `spline` time its
  wire differently from `linear`; PCHIP rejected too (its limiter bends parabolas near extrema,
  losing the exactness property; the clamp already provides the safety where it matters).
  Properties, each gated: **exact for quadratic-in-block-index sweeps**; **bit-identical to
  `linear` on collinear knots**; constant in, constant out (the coefficients sum to 1); and
  **`linear`'s arrival contract** — frame 0 IS the previous knot exactly, `cur` is reached at the
  next block boundary, so the declaration changes the shape of the ride, never its timing. f64
  evaluation with ONE rounding per frame (the mixer's cv-sum rule, WO-014 inc 6). **State:**
  `CvPlan` grew `prev2` beside `prev` and the wire's `range`; both history slots start at 0.0 —
  the same declared zero-history ramp `linear` has always had — so the first two blocks are a
  documented deterministic startup transient, gated. **Range discipline (decision 4):** the
  parabola carries momentum and can locally exceed the knot span (all interpolating splines do —
  the inertia IS the smoothness); the overshoot is HOST-made, so the spline branch clamps to the
  receiver's declared range (`CvRange::clamp_f64`, new), in words in all four copies of the rule.
  `hold`/`linear` deliberately do NOT clamp: they provably cannot exceed their knots, and an
  out-of-range frame there can only be a SOURCE bug, which must stay visible. **One signature,
  one copy (decision 5):** `expand` widened to `(prev2, prev, cur, range, dst)` — clamping in the
  executor instead would put the rule apart from the math it bounds; the api-snapshot pin moved on
  purpose with the reason at the pin. **The refusal retired; its philosophy is kept** — every
  other cv refusal (range, fan-in, delay-kind) stands untouched, and contract_v1's refusal gate
  was REPLACED in place by a behaviour gate. The G4 drift gate grew a pin that the matrix cell
  carries the curve's definition in the same words as the compiled copy. **`tests/cv_spline.rs`
  (8 gates):** the hand-computed parabola over a scripted dyadic sequence (every frame of every
  block `==` against independently simplified polynomials — the sequence visits the clamped
  overshoot, the momentum dip and the plain descent); the constant wire flat bit-exactly;
  spline ≡ linear bit-for-bit on a ramp with a hand-computed anchor so "equal" cannot mean
  "equally wrong"; a `v[N] = N²/64` sweep reproduced exactly at fractional frames from block 2
  on; frame 0 == the previous knot over an LCG-seeded sequence (the house seed 0xA17E); the
  startup transient hand-computed AND a rebuild byte-identical; both polarities' clamps at exactly
  ±1.0 with the un-clamped interiors keeping their hand-computed values; zero allocations over
  1 000 blocks. **Nothing else moved, by the shape of the diff (decision 8):** the hold/linear
  branches are byte-identical, no shipped module declares `spline` (grep-verified), no demo patch
  contains a block→audio cv edge, and the stress/determinism generators never synthesise an
  interp. **752 tests** (was 742; +8 cv_spline, +2 port.rs units, contract_v1 replaced 1:1),
  every pre-existing golden re-verified unchanged (`ba577186c988db21`, drum-demo
  `914d9063ce9d8a0f`, the three pinned exec renders at their `--seconds` — `1621e1f65b1b64e1`,
  `f2303f13aa0cf299`, `53de3b1f3f40e3c9`), stress `b42068ec7b206789` with identical counters
  (10 001 blocks · 7 203 swaps · 2 797 refused) in debug AND release, selftest 9/9, `ui --audit`
  PASS 25 smokes (the canvas never saw the refusal — interp lives in the executor, not in
  `connect_cv`), `modules --strict` 17/17, `module_docs --check` 17, fmt clean, clippy clean in
  default / bootstrap-audio / ui / native ui-window-gles / all six runnable MSVC cells (the two
  documented `windows`-crate OOM classes stay out), 5 python gates + `sync_check --self-test` +
  `log_check --self-test` 25/25. **Defect #88 (docs-only, fixed in passing):** the author guide's
  §11 still called `util/mixer` "Phase 1, not yet built" — stale since WO-014 inc 6; the bullet
  had to be rewritten for `spline` anyway. Sealed as **`sync wo008-inc6`** — WAITING, stacked on
  `sync-wo014-inc6.zip`. Parked and declared: the non-causal splines (a latency CONTRACT decision,
  not an implementation detail — a new word with its own gate if ever taken, never a quiet change
  to this one), and any shipped module ADOPTING `spline` (a module change moves goldens — its own
  increment, on purpose, with the audible A/B recorded).
- **WO-014 increment 6 is BUILT and sandbox-green — `util/mixer` 0.2.0, the cv merge side:** the
  sandbox track's next buildable item (item 6 — the live HAL through `SharedEngine` — stays gated
  on WO-006's device acceptance; the plan of record is `WO014-INC6-PLAN.md`). The compat-matrix's
  cv-fan-in cell always named `util/mixer` as the explicit merge; the module now IS one on both
  sides: four block-rate unipolar cv inputs (`cv-0..3`, `cv_reduce = "mean"` DECLARED per G3),
  per-input gains (params 20..23, **appended LAST** — every v0.1.0 snapshot index and every audio
  golden keeps its bits; short snapshots read 0.0 past their end, so batch-3's 20-value tests are
  unchanged BY CONSTRUCTION), one `cv-out` summing in f64 and clamped to the declared range (the
  module's own documented rule). Identity default: an untouched cv side is a bit-exact wire from
  `cv-0`. `BlockStatus` stays the AUDIO contract — all-unconnected audio ⇒ `Silenced` + exact
  zeros while the cv side merges (a cv-only mixer is a legitimate citizen, pinned). Wrong-rate
  presentation ⇒ `Failed` (the env/ad precedent). **The executor's fan-in refusal is RE-WORDED,
  not removed** (no implicit summing, ever) and now names the built remedy: each source to its own
  `cv-N`, destination fed from `cv-out` — `contract_v1`'s "names the merge module" pin passes on
  the new text. `Adapter::Merge.first_phase` stays Phase 1 ON PURPOSE (the module exists; the
  one-tap INSERT offer waits — a merge is a three-wire re-patch, not the inline pair shape the
  mechanism fits; decision 8 of the plan). **`tests/mixer_cv.rs` (9 gates):** bit-exact identity
  wire, hand-computed gain sum, exact 1.0 clamp, the declared mean reduce checked against the
  source's OWN published buffer in the same block, the audio side re-proven at 24 values, status
  decoupling, the remedy-naming refusal, zero allocations over 1 000 blocks, and the
  snapshot-order pin (a future insert-instead-of-append fails here first, in words). **A gate
  caught a real fragility mid-increment:** the inc-5 inspector-scroll smoke failed when the
  mixer's param count moved 20 → 24 — its fixed drag had been tuned to the old `max_scroll`; the
  smoke now asserts the CONTRACT (something hidden becomes visible AND touchable), not one
  module's arithmetic. **742 tests** (was 733), every pre-existing golden re-verified unchanged
  (incl. drum-demo through the mixer, the three pinned exec renders, stress `b42068ec7b206789`
  debug AND release), selftest 9/9, `ui --audit` PASS 25 smokes, `modules --strict` 17/17, docs
  regenerated (17) + `--check`, all runnable clippy cells clean (MSVC×8 now). Sealed as
  **`sync wo014-inc6`** — WAITING for the device. Parked and declared: the bipolar cv side
  (waits `util/range`), a 4×4 cv matrix (waits MAX_PARAMS), the canvas insert-offer for merges.
- **WO-013 increment 5 is BUILT and sandbox-green — the "Else column" is empty:** the four items
  increment 4 declared for its next pass all shipped, and nothing else (risk R1). **Rename text
  entry:** NEW `sparq-ui::canvas::entry` (`TextEntry` — end-caret, printables-only, 32-char cap =
  the node header's budget; `RenameState` — the sheet geometry on the browser's clamping rule),
  the RENAME menu row (first on the node menu; the one test that hard-coded DUPLICATE at row 0 now
  finds rows BY ACTION), commit through the increment-1 `Op::Rename` (undoable; EMPTY commits
  `None` = the module default; an unchanged buffer is a stated no-op with no history entry —
  catching the case `op_rename`'s equality cannot), cancel stated in words on Escape / outside tap
  / drag / long-press. The shell's keyboard path is ONE feed now: `read_keys` parses egui events
  into a normalised `KeyBatch` the rename sheet and the browser query both consume — the shared
  surface the provisional feed was declared to be waiting for. **Inspector scrolling:**
  `compute_at` with the offset clamped IN the layout, a fixed header the rows slide under, one
  visibility rule for draw/hit-test/audit (`row_visible`/`visible_rows`: a bottom sliver stays
  touchable, a row under the header is not), a hairline `thumb()` only when the panel scrolls
  (deliberately not an audit element — the gesture is the input). The gesture rides the TWO-finger
  pan (one finger on a row is a slider edit): **`GestureIntent::Pan` grew `center`** (the centroid
  the recogniser already tracked; `Zoom`'s precedent) — centre in the panel ⇒ scroll, elsewhere ⇒
  camera, exactly as before; a pinch over the panel is DECLINED so the canvas behind never moves,
  which also keeps the recogniser's sequential-contact span wobble (measured: reciprocal
  1.667/0.6 zoom factors on a straight two-finger drag) off the camera. Scroll is transient view
  state, reset on selection change; the panel speaks only at its ends. **Per-cv wire levels**
  (inc 4's DECLARED LIMIT, retired): `NodeLevels` grew per-port entries, `wire_level` prefers the
  source PORT's level and falls back to the node's fold (audio semantics untouched — pinned by a
  test), and `bridge::node_levels` reads each cv output's real published value (`node_cv_block` /
  `node_cv_audio`, magnitude rule) — **the executor was not touched**, so the stress hash
  `b42068ec7b206789` stands in debug AND release. Two bridge gates: the rms→svf thesis rig lights
  the cv wire at the metered rms while the SAME node's folded meter stays at rest, and dropping
  the sine's amp takes the level down. Per-port AUDIO meters + the ring extension stay parked for
  the live-HAL increment. **The LOD pass:** Dot wires are hairlines and the SUM word is
  suppressed there (declared); Simplified is truly text-free; states ride the look-board's
  pattern-first language at every level — hatch = bypassed (the mockup's `hatch8`), dashed border
  = muted, double border = locked, header chip / accent ring = master; at Dot, hollow = bypassed,
  dimmed = muted, concentric ring = locked. Words ride on top at Full only — the mockup's own
  redundant pairing. The side-by-side human review stays device-track (test006 step H). **Five
  new audit smokes (25 total)**; the cv smoke reads the bridge DIRECTLY on purpose — a second
  menu-driven render would overwrite `canvas-render.wav`, the file test006's step [05] hashes as
  the manifest-defaults baseline (smoke 13 owns the menu path). **733 tests** (was 713), every
  pre-existing golden re-verified unchanged, `modules --strict` 17/17, all runnable clippy cells
  clean (MSVC×ui-window and MSVC×bootstrap-audio remain the documented `windows`-crate OOM
  class). Sealed as **`sync wo013-inc5`** — APPLIED on the device 2026-09-27. Plan of record:
  `WO013-INC5-PLAN.md`.
- **WO-014 increment 5 is BUILT and sandbox-green — the module set is COMPLETE at seventeen:**
  the last three modules shipped on the contracts they waited for. **`ana/tap`** (the generic
  signal tap for displays): audio-rate bipolar `wave` (the mono monitor mix) + block-rate unipolar
  `peak`/`rms`; its wave golden **`3f325d4f99ca2a01` CROSS-VALIDATES against the `syn/sine` 1 s
  golden** — the mono mix of a mono-fanned sine is the sine itself bit for bit, so the tap colours
  nothing (two mechanisms, one bit pattern); `gain` scales the WAVE only, peak/rms report the TRUE
  signal. **`dsp/scope`** (the first real visual module): the set's only module whose `process` is
  a deliberate **NO-OP** — zero-audio-thread-cost is architecture, not optimisation, and the proof
  is a patch-level golden: adding a tap+scope to a render leaves the master output BIT-IDENTICAL,
  so the scope-rig golden **EQUALS the out/main golden `75bc7f2f18cac9d5`**; two audio-rate bipolar
  cv inputs (`x`, `y`) are the display BINDING the UI resolves to source taps. **`out/main`** (the
  master output with the metering hook): a unity-**BIT-EXACT** pass-through with trim + a hard mute
  writing exact zeros; because the executor meters every node, its peak/rms ARE the master meters;
  it has an audio OUTPUT because the executor renders the master node's first audio output (a pure
  sink would render silence). **The analysis-payload contract they waited on** shipped too:
  `sparq-audio::engine::AnalysisUpdate` extends ADR-009 d8's publication from meters to WAVEFORMS —
  `AudioEngine::publish_analysis` pushes every audio-rate `cv` output onto a bounded `SpscRing`
  per block, `SharedEngine::read_analysis` drains it control-side, `Executor::with_audio_rate_cv_out`
  is the allocation-free hook (`tests/analysis_pub.rs`: the waveform crosses, an absent reader is
  counted-not-queued, the audio thread allocates nothing). **The LFO beat-rate DECISION** the WO
  asked for before building: **module-side bpm derivation, NOT a per-frame tick view** — recorded
  with its reasoning in LATER.md (a contract change to buy sample-accurate tempo almost no module
  needs, vs reading the block-start tick the executor already carries; one block = 1.3 ms of lag on
  a tempo EDIT) — **and then SHIPPED the same session**: `mod/lfo` 0.2.0 (`sync-mode` + `division`)
  and `util/delay` 0.2.0 (`tempo-sync` + `division`, riding the once-unreachable
  `DelayLine::set_tempo_sync`) both derive bpm from the block-start tick via a shared
  `TempoFollower`, fall back to their free-running parameter with no transport, and are additive
  (defaults byte-identical, which the unchanged goldens prove). `tests/tempo_sync.rs` (7 gates):
  the derived rate is the transport's, it tracks a tempo change, the tempo-synced delay renders
  BIT-IDENTICAL to a hand-timed 250 ms, beat-locked golden `db4013f41d1fa678`, zero allocs.
  **713 tests** (was 682), every pre-existing golden re-verified unchanged (stress
  hash `b42068ec7b206789` in debug AND release), `modules --strict` **17/17**, docs regenerated
  (17), all runnable clippy cells clean. Sealed in **`sync wo014-inc5+wo013-inc4`**.
- **WO-013 increment 4 is BUILT and sandbox-green — live wire levels, the "signature sparq image":**
  the park note said wires drew at rest until the executor taps existed; they do now (WO-008's
  `Executor::meter` + the cross-thread `SharedEngine::read_meters`), so the painter half shipped —
  **and does not fake them**. **`sparq-ui::canvas::levels`** (toolkit-independent, computed not
  drawn): `NodeLevels` (per-node 0..1, clamped, NaN-to-rest, deterministic) + `wire_level` (a wire
  carries its SOURCE node's level), unit-tested. **`bridge::node_levels`** renders a short preview
  and reads each node's PEAK meter, mapping kernel nodes back to canvas nodes — the ONLY source of
  a wire level, never a value invented from canvas data; two bridge tests prove the levels are real
  AND follow the signal (turn the gain down, the level falls). **The painter** (`canvas_ui.rs`)
  modulates each wire from its class colour toward its class glow by level plus a soft under-glow —
  both endpoint colours are tokens, the blend invents nothing. `CanvasState.levels` is a transient
  (non-undoable) field the shell refreshes from the bridge after RENDER WAV; empty until the first
  render, so wires draw exactly as before until then. **Audit smoke 20** proves RENDER WAV populates
  live levels from real meters. **The master handover** (coupled to `out/main`): `resolve_master`
  now prefers a wired `out/main` node over the default highest-id-terminus rule (an explicit SET
  MASTER still wins; an unwired out/main does not count), 3 new tests, `OUT_MAIN_ID` a named
  constant so the rule and the id cannot drift. **DECLARED LIMIT (not faked):** a node's meter folds
  its AUDIO outputs, so a cv wire out of a cv-only source (lfo/env/rms) reads at rest — the
  animation tracks AUDIO signal flow; per-port/per-cv meters are the LATER.md item that would light
  cv wires from their own values. **NOT in this increment** (the task list's "Else" column, declared
  for the next pass): rename text entry, inspector scrolling, LOD visual iteration vs
  `design-mode.svg`. Sealed in **`sync wo014-inc5+wo013-inc4`**.
- **WO-008 increment 5 is BUILT and sandbox-green — the cross-thread hot-swap primitive:** the
  task list's item 3, "the only remaining increment with a proofs burden this heavy", is done.
  **`sparq-kernel::sync::hotswap`** (allowlist entry 6, shipped in `sync/` with the auditor's
  alias like entry 4's): `HotSwap::split` → `HotSwapControl` (stage complete boxed payloads,
  reclaim retired ones, counters) + `HotSwapAudio` (`boundary(inherit)` once per block — d3's
  LITERAL pointer swap, one `AtomicPtr::swap` per side). **Epoch retirement:** the boundary
  publishes the outgoing payload into one of 8 epoch-tagged slots and stores `epoch+1` Release;
  the controller may re-box a retirement only when its Acquire-loaded epoch exceeds the tag —
  the grace period is mechanical, and the drop lands control-side, so module deactivation never
  runs in a device callback. Slot exhaustion DEFERS the swap (counted) — the audio thread's
  every fallback is "keep rendering the live patch". **The contract grew one bound:
  `Module: Send`** (built control-side, rendered audio-side, retired control-side; additive,
  all 41 implementors satisfy it automatically, pinned in api_snapshot; `Sync` deliberately not
  required). **The cross-thread engine** (`sparq-audio::engine`): `LivePatch {executor, master}`
  payloads, `SharedEngine` (stage / `set_params` / `set_musical_position` / reclaim /
  `read_meters` / stats) + `AudioEngine` (per block: boundary swap with `inherit_runtime` at
  the swap point → `EngineCmd` ring drain → render → meter publication). **Decision 8's first
  consumer:** per-node `MeterUpdate`s (peak/rms/status + block clock) ride an `SpscRing` out;
  `Copy` commands ride one in; both refuse-and-count, never block, never grow. The
  single-owner `Engine` stays untouched — the determinism harness keeps its vehicle and the
  stress hash `b42068ec7b206789` its meaning (the `World::candidate_mutation` extraction that
  lets the cross-thread stress reuse the seeded schedule preserved the RNG draw order exactly —
  verified in debug AND release). **The proofs:** 9 kernel tests (epoch gate white-boxed in its
  three states, deferral without drops, teardown hygiene both orders, a paced 5 000-payload
  two-thread exchange with tearing checks; `cfg!(miri)` scales it to 200) + 7 audio tests
  (`tests/cross_thread.rs`) + **the acceptance: 10 000 seeded mutations staged from a control
  thread while the audio thread plays** — 7 400 swaps, 2 600 refusals that left no trace,
  ~43 000 rendered blocks, ZERO failed blocks, ZERO audio-thread allocations under the counting
  allocator, `deferred == superseded == 0`, retirements == swaps == reclaimed, clock never
  regressing; no golden hash ON PURPOSE (which block a mutation lands on is a scheduling fact —
  a golden over scheduling is a flake generator). **selftest gate 9** (64 paced swaps across two
  threads in-process) gives SATURN MSVC-side evidence; CI's Miri cell widened `sync::rings` →
  `sync::` (the sandbox cannot run Miri — nightly sysroot build OOM-killed at 1 GB, tried
  twice; environment, same class as MSVC×ui-window). **682 tests** (was 665), every
  pre-existing golden re-verified unchanged, `selftest --golden` 9/9, audit PASS 19 smokes,
  `modules --strict` 14/14, all clippy cells clean. **Defect #85** logged: the exec evidence
  hashes were recorded without their durations (the CLI default renders 5 s; the goldens are
  the 2.8 s/1 s/2 s renders) — the standing order now carries explicit `--seconds`. Sealed as
  **`sync wo008-inc5`**. Unblocked and waiting: `ana/tap` + `dsp/scope` (WO-014 inc 5), WO-013's
  live wire levels (inc 4), and — after WO-006's exclusive acceptance — routing the live HAL
  stream through `SharedEngine`. **Device note:** the pristine clone FAILED `sync_check
  --quiet` against its own committed stamp (defect #86 — the git history rebuild lost 230B of
  `sparq-module-api/src/lib.rs` and sparq-music from `sync_check.py`'s roots); the bundle
  carries both files as declared drift repairs, and the run sheet starts with two `copy`
  commands that save SATURN's sealed versions to `logs\` before overwriting.
- **WO-014 increment 4 is BUILT and sandbox-green — the clocks' first consumers:** the module
  set reaches **fourteen** with the two `mod`-family modules defect #84's taxonomy fix existed
  for. **`mod/lfo`**: four shapes mapped into 0..1, audio-rate UNIPOLAR cv out (the range
  decision is recorded in the manifest header: the matrix refuses bipolar→unipolar rather than
  rescale, and `util/range` is Phase 1 — a bipolar port today could not connect to a single
  shipped consumer), event phase-reset at the exact sample, 8-byte phase state. Gates use a
  **binary-exact rate** (48000/2^15 Hz) so quarter-cycle values are EQUALITIES and the wrap
  lands on frame 32768 to the bit. **`mod/clk-div`**: the first event-PROCESSING module —
  divide counts from the first input, multiply schedules sub-triggers across the MEASURED
  interval from a bounded 32-slot internal schedule (exact samples in later blocks), the
  probability gate is a seeded xorshift draw (same seed ⇒ same decisions, tested both ways;
  p=0/p=1 exact). The acceptance-shaped gates: **lfo→svf cutoff renders BIT-IDENTICAL to a
  hand-driven reference** over 200 blocks (the contract-v1 pattern reused; sweep ends at
  289.445 Hz and rising), and **host 16ths → ÷4 → membrane** lands kicks on exactly frames
  0/24576/49152/73728 with chain golden **`914d9063ce9d8a0f`**. Zero allocations: both modules
  (5 000 calls each) and the divided-drum chain (1 000 blocks, in-loop host pushes). **665
  tests** (was 655), every pre-existing golden unchanged, stress hash unchanged, selftest 8/8,
  audit PASS 19 smokes (browser meets "LFO"/"Clock Divider"; the "exactly one gain" smoke still
  passes), `modules --strict` **14/14**, docs regenerated (14), all clippy cells clean.
  Sealed as **`sync wo014-inc4`**.
- **WO-009 increment 1 is BUILT and sandbox-green — clocks + transport v0:** the kernel `Clock`
  is now ADR-006 rule 2's piecewise map (anchored segments, linear bpm ramps — derivative
  continuous by INHERITANCE, exact integer bisection for `sample_at_tick`, the constant-segment
  fast path bit-identical to v0 so the phase-b goldens could not move); **`sparq-music`** is the
  planned crate's first contents (`ClockBroker`: position + wall-drift estimator, backwards
  readings refused-and-counted; `Transport`: freeze-on-stop, tempo-as-segments, block-granular
  loop fold, tap tempo, the tick-scheduled queue, bar/beat emission, `advance_block` zero-alloc
  measured over 10 000 blocks into a bounded 64-slot collector). The executor gained
  `set_musical_position` — **the transport computes, the executor carries** — and a never-set
  door renders the static `tick = 0` every golden knows (stress hash `b42068ec7b206789` unmoved).
  The ±0-sample acceptance ran 1 000 seeded ticks — constant tempo AND across a glide + a step —
  against an **independent implementation of the segment integral inside the test**. The
  cross-check: `exec --patch drum-demo` is transport-driven (beats AND bars, live tick) and
  hashes to `f2303f13aa0cf299` — **the identical golden** WO-014 inc 3's block counter produced.
  Two mechanisms, one bit pattern. One finding caught by measurement pre-ship: proportional beat
  placement inside loops put boundary beats one sample early (23999 vs 24000) — beats now ride
  the absolute map grid; the transport header records it. **655 tests** (was 627), all goldens
  unchanged, ADR-006 addendum scores the four acceptance boxes honestly (three met in sandbox;
  the drift box's 30-min hardware half stays device-track). Sealed `sync wo009-inc1`.
- **WO-014 increment 3 is BUILT and sandbox-green — contract v1's first consumers:** the module
  set reaches **twelve** with the three modules the contract unblocked — **`syn/membrane`** (the
  Phase-B kick topology promoted to a trigger-driven voice; a trigger at sample 37 leaves frames
  0..36 at EXACT zeros and lands the hit in its own block — the WO's sample-accurate stress,
  measured), **`env/ad`** (event in → audio-rate cv out and **no audio ports at all**; loop and
  gate-off semantics per its manifest, the loop cycle measured against the wrapped `AdEnv`'s
  epsilon-arrival decay rather than the parameter faces), **`util/mixer`** (4×4 stereo matrix,
  20 of 32 snapshot params — which is WHY 4×4, declared; identity default is a bit-exact wire;
  the explicit merge the fan-in rules require; `Silenced` when all-unconnected). Plus
  **`sparq exec --patch drum-demo`**: host triggers at 120 BPM → membrane → mixer(trim 0.6),
  golden `f2303f13aa0cf299` with the four kicks asserted on EXACTLY frames 0/24000/48000/72000
  (debug == release, two builds bit-identical). **Defect #84** fixed table-first: `classification.top`'s
  closed domain was missing `env`/`mod` in BOTH copies while Appendix B's ids and WO-014's own
  artefact paths name them — `env/ad` could not declare its own top; the table moved first, then
  `TOPS` (16 → 18), then the field table gained its first code consumer as a drift pin. The
  executor's cv-fan-in refusal was re-worded (it named `util/mixer` as "not in this build" — the
  audio side now ships; the cv side is LATER.md). **627 tests** (was 614), every pre-existing
  golden unchanged, stress hash unchanged, zero allocations counted across 15 000 new-module
  `process` calls and 1000 drum-demo blocks with in-loop host pushes, selftest 8/8, audit PASS
  19 smokes (three new names checked against the fuzzy scorer), `modules --strict` **12/12**,
  docs regenerated (12), all clippy cells clean. Sealed as **`sync wo014-inc3`**.
- **WO-008 increment 4 is BUILT and sandbox-green — CONTRACT V1, the "unblocking increment":**
  the multi-port `AudioCtx` (audio ≤ 8 ports/class + `take_*` views for multi-output modules,
  `cv` block/audio-rate ports at the RECEIVER's declared rate, `event` ports pre-sorted with G5's
  stable tie-break and bounded counted-overflow sinks), the executor's cv/event wiring (all six
  `cv_reduce` policies measured against hand-computed values, `cv_interp` hold/linear with
  `spline` REFUSED in words, range-mismatch + fan-in refusals through `connect_cv` at `Phase::Zero`
  naming the converter AND its phase, `event_kinds` subset checks, the control-side
  `push_host_event` door that WO-009 will feed), data/gpu/atom still refusing per type in words.
  **`ana/rms` publishes on its declared cv port** (the output[0] convention is deleted — exec's
  tap line reads the same `0.16621882` through `node_cv_block`), **`flt/svf` 0.2.0** grows the
  `cutoff-mod` cv input + `mod` param (additive; at defaults bit-identical to 0.1.0). The
  **WO-014 rms→filter acceptance is proven with arithmetic**: the cv-wired filter renders
  **bit-identical** to a hand-driven reference over 200 blocks, and `sparq exec --patch mod-demo`
  is the audible artefact (golden `1621e1f65b1b64e1`, debug == release). The **compat-matrix
  drift gate** landed (the WO-007 debt, answered honestly — see below) and caught defects #80–#82
  on first run; `toml.rs` learned nested `[[a.b]]` arrays (the matrix had never been parsed by
  the crate that ships the parser). **614 tests** (was 559), **ALL pre-existing goldens
  UNCHANGED** (`ba577186c988db21` / `0f5c3e86c7f117a9` / `53de3b1f3f40e3c9` / `3f325d4f99ca2a01`
  / six batch-2 goldens / chain `9170415cd3852736`) and the **stress hash `b42068ec7b206789`
  UNCHANGED** (10 001 blocks · 7 203 swaps · 2 797 refusals · 0 audio-path allocations), selftest
  8/8, `ui --audit` PASS 19 smokes, `modules --strict` 9/9, clippy clean in default /
  bootstrap-audio / ui / native ui-window-gles / MSVC×5, 5 Python gates + module docs clean.
  Sealed as **`sync wo008-inc4`**, stamp **`src 86f/1679258B`** (134-file covered set verified,
  self-test re-proven failable 11/11; `log_check --self-test` 25/25).
- **The compat-matrix mirror debt, re-worded not hidden:** the gate pins the table and `port.rs`
  together (vocabularies, verdicts, adapter ids, per-type case counts, a representative outcome
  per audio/cv `when` cell). Full deletion stays open WITH A NAMED REASON: several `when` cells
  are prose ("fan-out: one cv output -> many cv inputs") — shape rules, not pair rules — so "read
  at discovery" needs the ratified table restructured into machine predicates first (a
  review-packet change; `[meta] reviewed = false` is still pending). Recorded in the ADR-005
  addendum. Defect **#82** (mono→ambisonics is "compatible fan-out" in BOTH copies — consistent,
  musically questionable) is pinned by a test and flagged for that review.
- **WO-014 increment 2 is BUILT and sandbox-green** — the six AUDIO-domain modules
  (`syn/noise` seeded+`reset`, `syn/polyblep` (stable id, bandlimited-additive impl per #12 —
  the manifest says so in words), `flt/svf` five modes, `util/delay` with the set's first
  **parametric latency** (`"param:time"`), `fx/bitcrush` seeded per-channel dither,
  `util/panner` two laws): **9 first-party modules total**, `modules --strict` 9/9, six module
  goldens + a five-module chain golden checked in, per-module 5 000-call zero-alloc gates, the
  **polyblep aliasing acceptance measured through the registry build: −166.8 dB vs the −60 dB
  bar** (printed by the test every run), the delay echo landing at exactly its declared sample.
  **`tools/module_docs.py`** ships the generated-docs acceptance (9 docs, `--check` wired into
  justfile + ci.yml + gates.bat, proven failable pre-ship). Sealed as **`sync wo014-inc2`**
  (applied on SATURN). **Of the eight modules that did not ship, THREE ARE NOW UNBLOCKED by
  contract v1** (`syn/membrane`, `env/ad`, `util/mixer` — event/cv/multi-port payloads travel;
  `tests/contract_v1.rs`'s probe modules are the existence proof); `mod/lfo`/`mod/clk-div` still
  wait on WO-009's clocks, `ana/tap`/`dsp/scope` on the ring publication, `out/main` on the
  canvas master-handover.
- **WO-008 increment 3 is BUILT and sandbox-green** (tasks 4–7): the boundary-swap `Engine`, the
  **10 000-mutation stress** (`tests/mutation_stress.rs`), kernel **per-path latency** + the
  **raw/compensated switch**, the **watchdog** (3 consecutive overruns ⇒ auto-bypass + bounded
  journal), and the **determinism harness** (`sparq_audio::determinism` — it caught a real
  HashMap-iteration-order nondeterminism on day one). Sealed as **`sync wo008-inc3`** (applied).
- **WO-006 increment 1.3 is BUILT and sandbox-green** (the defect-#79 exclusive device-period
  fix): `sparq-kernel::hal::period` (pure ladder, 7 Linux unit tests incl. the exact test004
  device shape), `open_exclusive` executes it with a fresh client + alignment two-step per
  candidate and a full period probe table on refusal, pump unchanged. Sealed as
  **`sync wo006-inc13`** (applied). Device contract = `scripts\test004.bat` +
  `WO006-INC13-RUN-SHEET.md`.
- **WO-013 increment 3 is BUILT and sandbox-green** (browser + inspector + wire re-patch).
  Sealed as **`sync wo013-inc3`** (applied). Device contract: `scripts\test006.bat` +
  `WO013-INC3-RUN-SHEET.md`.
- MSVC × `ui-window` cell: **still unrunnable anywhere** — the `windows` crate's rustc is
  OOM-killed at 1 GB (environment, not code; CI has no such cell either). The native
  `ui-window`/gles cell builds clean at `-j 1` with dev debuginfo off.
- **Miri is CI-only** (observed 2026-09-27): `cargo +nightly miri` needs a custom sysroot whose
  `core` build is OOM-killed at 1 GB — tried twice. The `sanitizers` CI job (continue-on-error)
  runs `sync::` under `-Zmiri-strict-provenance`; the hotswap suite scales itself down via
  `cfg!(miri)` so the cell stays usable. Nightly installs with `rustup toolchain install nightly
  --profile minimal --component miri` if a future sandbox has the memory.

## 🔴 THE BLOCKER (device track — waiting on SATURN, not on code)

**WO-006 exclusive acceptance = the `test004` re-run, attempt 3** (every sealed bundle through
`sync-wo008-inc6.zip` is APPLIED on the device — proven by attempt 2's own build line,
2026-09-28; **`sync-wo006-inc14.zip` is WAITING** — it IS the HAL fix). History: **attempt 1**
(2026-09-24) passed the caps checkpoint (#77 verified on hardware) and was refused at every rung
with `AUDCLNT_E_INVALID_DEVICE_PERIOD` — the open asked the 64-fr block (666.7 µs @ 96 kHz) as
the device period against a 10 ms engine (**defect #79**, fixed in inc 1.3). **Attempt 2**
(2026-09-28) got further and taught more: exclusive OPENED — `i24-in-32 (converting)` @ 96 kHz
on the UMC 204HD — but at **288 fr (3.00 ms)**, the driver's reported minimum / alignment
granularity, and the driver could not SUSTAIN what it had ACCEPTED: one ~33 ms stall every
~62.5 ms (159 late wakes / 10 s), half throughput (7732 blocks ≈ 49.1k fr/s against a 96 kHz
negotiation), drift −491 656 ppm, no clean tone, rc 1 (**defect #89** — the arithmetic in
`docs/hal/windows-notes.md` §4d; the pump was innocent: 0 budget overruns, 0 FIFO starvations).
The control experiment ran in the same session: the shared 10 ms engine on the same endpoint
soaked **2 h, 719 895 wakes, 0 xruns, max jitter 12.04 ms** — the machine and the pump are fine;
the 3 ms exclusive period is not. On attempt 3 the [04] open line should read
`i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr` **or**
`… 1152 fr (12.00 ms) …` if the driver enforces its 288 fr alignment at `Initialize` (inc 1.4's
two-step rounds the ask UP to it — never down to the granularity). **A 288 fr (3.00 ms) period
must never open silently again**: either the ladder asks the default first, or the shrink guard
prints the ask in the open line. If [04] still misbehaves, the period probe table + the
late-wake/jitter/drift triple in the digest IS the diagnosis. When [04]+[05] pass and the
**2 h zero-xrun soak @ 96 kHz/64 exclusive** finishes clean: WO-006 acceptance closes → ADR-008
exit fires → **next sandbox increment: delete the cpal bootstrap, HAL becomes `play`'s default**
(that deletion is gated on this device run — do not do it early).

## Next session's task list, in order

**Device track (SATURN — one bundle WAITING to apply, then the remaining work is EVIDENCE):**
1. **APPLY `sync-wo006-inc14.zip` (15 entries)** — everything through `sync-wo008-inc6.zip` is
   APPLIED (attempt 2's build line proved it, 2026-09-28). Then **run the device gates and send
   the DIGESTS back** — every test script now ends by making `NAME-digest.log` beside its log
   (compact copies: verdict lines byte-identical, only the periodic-status / cargo / duplicate
   repetition collapses; `scripts\digest.bat` does older logs on demand). Run
   `scripts\test004.bat` (the exclusive acceptance, attempt 3 — pass shape in
   `WO006-INC14-RUN-SHEET.md`; the 🔴 blocker) and `scripts\test006.bat` (the window session;
   step [07] is the mechanical claim: slider edit must change the canvas-render.wav hash — after
   RENDER WAV the sine/gain wires light with live levels — **and the new steps F–I: the rename
   sheet under real keys, the inspector two-finger scroll on a Mixer, the LOD walk vs
   `design-mode.svg` (write what differs into the H answer), the cv wire lighting from its own
   value**). `scripts\gates.bat` now expects **757 passed / 0 failed / 1 ignored**, **17**
   modules (the mixer row reads v0.2.0, 13 ports, 24 params), **selftest PASS (9 gates)** and
   **`ui --audit` PASS (25 smokes)** — with the stamp `log_check.py` in the bundle already knows
   (`src 92f/2001521B`; defect #83's fix made moving it a seal step). Wanted evidence, **with the pinned durations** (defect #85):
   `sparq exec --patch mod-demo --seconds 1` (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo
   --seconds 2` (`f2303f13aa0cf299`), `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`),
   and — when the HAL is free — the two WO-009 device boxes: a LISTENED tempo sweep and the 30-min
   drift run. No drift-repair copy step was needed (the pristine clone passes `sync_check --quiet`;
   defect #86 was repaired in the wo008-inc5 bundle). **Wanted back: `test004-digest.log`,
   `test006-digest.log` (with the F–I answers), `gates-digest.log`, `logs\ui-digest.log`** — the
   digests, not the full logs (the full ones stay on the device; if a diagnosis ever needs a
   collapsed line, the run sheet says which full file to send instead) — those close the
   device boxes the sandbox cannot.
2. If test004's 2 h soak passes → **WO-006 acceptance closes** → sandbox increment: ADR-008 exit
   (delete the cpal bootstrap, HAL becomes `play`'s default) **plus routing the live HAL stream
   through `SharedEngine`** — the cross-thread halves now exist (inc 5), so `play`'s callback
   can hold `AudioEngine` and the control side can stage; then WO-006 increment 2 backlog
   (ASIO, duplex, round-trip measurement, STA retry for the #75 property-store `E_ACCESSDENIED`,
   clock-drift re-derivation per #76).
3. The WO-008 **loaded soak** (200 modules, 30 min, zero xruns) — the WO's hardware acceptance,
   and since inc 5 also the paced-real-time half of allowlist entry 6's stress sentence.

**Sandbox track (buildable now, in value order):**
4. ~~**WO-013 increment 4** — live wire levels~~ **DONE (2026-09-27)** ~~and its "Else"
   column~~ **DONE TOO (2026-09-27, second session — WO-013 increment 5):** rename text entry (the
   shared `canvas::entry` surface + the unified modal key feed), inspector scrolling (the
   two-finger pan routed by its new `center`), per-cv wire levels (the bridge reads the executor's
   real cv publications; the executor untouched) and the LOD pass (Dot hairlines, text-free
   Simplified, the pattern-first state language). What remains is declared in LATER.md §WO-013:
   numeric param entry (one parse step from the same `TextEntry`), caret movement/IME, browser
   drag-scroll, per-port AUDIO meters + ring port ids and the continuous level refresh (both ride
   the live-HAL-through-`SharedEngine` increment), randomise (the seed tree), and the device
   side-by-side LOD review (test006 step H).
5. ~~**WO-014 increment 5** — the last three modules~~ **DONE (2026-09-27, this session): the
   first-party set is COMPLETE at seventeen** — `ana/tap`, `dsp/scope`, `out/main` shipped on the
   analysis-payload contract (`AnalysisUpdate` extends d8's ring from meters to waveforms), with the
   two new goldens (`75bc7f2f18cac9d5` out/main == scope rig; `3f325d4f99ca2a01` tap wave == sine)
   and the LFO beat-rate DECISION — **decided AND shipped**: `mod/lfo` 0.2.0 + `util/delay` 0.2.0
   derive bpm module-side from the block-start tick (`TempoFollower`), and the delay's
   `set_tempo_sync` path is now reached (`tests/tempo_sync.rs`, golden `db4013f41d1fa678`).
   **Remaining from inc 5:** only the <15 %-of-a-core benchmark (stage-machine acceptance — a
   sandbox CPU ratio does not transfer, so no sandbox number is recorded as evidence).
6. **The live HAL stream through `SharedEngine`** — after WO-006's exclusive acceptance: `play`'s
   callback holds `AudioEngine`, the control side stages, and the canvas's `levels` field fills from
   `read_meters` per frame instead of a preview render (the continuous live-levels half; the offline
   half shipped in WO-013 inc 4). Then the WO-006 increment 2 backlog (ASIO, duplex, round-trip
   measurement, STA retry for #75, clock-drift re-derivation per #76).
7. Studio-session evidence still open: real-finger canvas touch-test, DPI matrix walk
   (WO-012/WO-013), WO-001 hardware sheets, dispatch-bench C/D numbers for ADR-009.
8. Small, well-specified, declared in LATER.md: ~~mixer's cv merge side~~ **DONE (WO-014 inc
   6)**; ~~`cv_interp = "spline"`~~ **DONE (WO-008 inc 6, this session — the host performs the
   whole G4 vocabulary)**; **host-side `required`-unconnected enforcement** (moves stress refusal
   counters — its own increment on purpose; **the next candidate in this list**); the compat-matrix review packet (`reviewed = false`, defect #82's ordering question
   inside it); loop-relative beat phase for unaligned loop regions (WO-009's declared limit);
   from inc 5: per-port meters (would light cv wires + a scope's per-channel display), a
   multi-reader epoch (a second reader of the live patch), and narrowing analysis publication to
   declared analysis sources if a large rig's ring traffic ever wants it.

## Done (most recent first)

- [x] **WO-006 inc 1.4** (2026-09-28, fifth session) — the defect-#89 fix after test004
      attempt 2 opened exclusive at 3 ms and stalled (~33 ms every ~62.5 ms, half throughput, no
      clean tone — while the shared 10 ms engine soaked 2 h clean in the same session):
      default-first period ladder for coarse engines, round-UP alignment two-step, silent-shrink
      guard + honest open line — all pure data in `hal/period.rs`, Linux-pinned on the attempt-2
      device shape. Defect #90: test004.bat's summary said "still refused" when exclusive had
      OPENED and run rough; refused vs opened-but-not-clean now distinguished, banner current.
      **`tools/log_digest.py`** + `scripts\digest.bat` + end-of-run hooks in test004/test006/
      gates — the compact `-digest.log` copies the operator asked for (attempt 2's log: −54 %,
      verdict lines byte-identical, `--self-test` 16/16). 757 tests, goldens untouched by
      construction, both MSVC hal-wasapi cells clean. Sealed `sync wo006-inc14` — WAITING.
- [x] **WO-008 inc 6** (2026-09-27, fourth session) — `cv_interp = "spline"` PERFORMED: the
      parabola through the last three block values (causal cubic-Hermite equivalence recorded),
      exact for quadratic sweeps, bit-identical to `linear` on collinear knots, `linear`'s
      arrival timing, f64 + one rounding per frame, clamped to the wire's declared range
      (`CvRange::clamp_f64`; hold/linear deliberately unclamped — source bugs stay visible);
      the build refusal retired, the matrix cell / §17 / schema row / author guide re-worded and
      drift-pinned; `CvPlan` grew the two-slot history + range; `tests/cv_spline.rs` 8 gates +
      2 `port.rs` unit pins + contract_v1's refusal gate replaced 1:1; defect #88 (the guide's
      stale mixer clause) fixed in passing. 752 tests, every golden unchanged, stress hash
      unmoved. Sealed `sync wo008-inc6` — WAITING, stacked on wo014-inc6.
- [x] **WO-014 inc 6** (2026-09-27, third session) — `util/mixer` 0.2.0, the cv merge side:
      4→1 block-rate unipolar merge with declared mean-reduce, per-input gains appended last,
      identity default, clamped publish; the executor's fan-in refusal re-worded to its built
      remedy (the rule stands); `tests/mixer_cv.rs` 9 gates; the inc-5 scroll smoke hardened
      against param-count drift. 742 tests, every golden unchanged. Sealed `sync wo014-inc6`.
- [x] **WO-013 inc 5** (2026-09-27, second session) — the "Else column" pass: the rename text
      entry (`canvas::entry`, the RENAME row, the `Op::Rename` commit with its empty-clears and
      unchanged-no-op edges, the unified modal key feed), inspector scrolling (`compute_at` + the
      fixed-header visibility rule + the thumb; `Pan.center` routing; pinch-over-panel declined),
      per-cv wire levels (`NodeLevels` per-port entries, `wire_level` port-first, the bridge
      reading the executor's real cv values — executor untouched, stress hash unmoved), and the
      LOD pass (Dot hairline wires, text-free Simplified, the hatch/dash/double/chip state
      patterns at every level, shape-encoded dots). 733 tests, every golden unchanged, audit PASS
      25 smokes, test006 steps F–I. Sealed `sync wo013-inc5` — APPLIED 2026-09-27.
- [x] **WO-014 inc 5** (2026-09-27, this session) — the module set COMPLETE at seventeen:
      `ana/tap` (wave golden cross-validates == the sine golden), `dsp/scope` (the no-op display;
      scope-rig golden == the out/main golden, proving zero-cost bit-transparency), `out/main`
      (unity-bit-exact master with the metering hook); the `AnalysisUpdate` ring extending d8's
      publication to waveforms; and the LFO beat-rate decision DECIDED *and* SHIPPED — `mod/lfo`
      0.2.0 + `util/delay` 0.2.0 derive bpm module-side from the block-start tick (`TempoFollower`),
      reaching the delay's `set_tempo_sync`, additive so every prior golden is unchanged.
      713 tests, every golden unchanged, `modules --strict` 17/17, docs 17.
- [x] **WO-013 inc 4** (2026-09-27, this session) — live wire levels (the signature image):
      the toolkit-independent `canvas::levels` model, `bridge::node_levels` reading REAL executor
      meters, the painter's token-colour→glow blend, `CanvasState.levels` refreshed on render,
      audit smoke 20, and the `out/main` master-handover rule (3 tests). Not faked; the cv-wire
      level limit + rename/inspector-scroll/LOD declared for the next pass. Sealed together as
      `sync wo014-inc5+wo013-inc4`.
- [x] **WO-008 inc 5** (2026-09-27, this session) — the cross-thread hot-swap primitive
      (`sparq-kernel::sync::hotswap`, allowlist entry 6): d3's literal pointer swap, epoch-tagged
      retirement (grace period mechanical, drops control-side), deferral-not-forcing; `Module:
      Send` (pinned); `SharedEngine`/`AudioEngine` + d8's meter ring + the `EngineCmd` ring; the
      two-thread 10 000-mutation acceptance (0 failed blocks, 0 audio-thread allocs, ledger
      exact); selftest gate 9; CI miri cell widened to `sync::`; defect #85. 682 tests, every
      golden unchanged (stress hash in debug AND release). Sealed `sync wo008-inc5`.
- [x] **WO-014 inc 4** (2026-09-26) — `mod/lfo` + `mod/clk-div` (14 modules),
      lfo→svf bit-identical sweep, 16ths→÷4→membrane chain golden `914d9063ce9d8a0f`, seeded
      probability reproducibility, binary-exact-rate arithmetic gates. 665 tests, all goldens
      unchanged. Sealed `sync wo014-inc4`.
- [x] **WO-009 inc 1** (2026-09-26) — piecewise kernel Clock (ramps + exact
      bisection), `sparq-music` (broker + transport + zero-alloc queue), the executor's musical
      door, transport-driven drum-demo == the batch-3 golden, ±0-sample acceptance vs an
      independent integral, ADR-006 addendum. 655 tests. Sealed `sync wo009-inc1`.
- [x] **WO-014 inc 3** (2026-09-26) — membrane + env/ad + mixer (12 modules),
      drum-demo golden with kicks on exact frames, defect #84 (TOPS table-first), 627 tests,
      goldens unchanged. Sealed `sync wo014-inc3`.
- [x] **WO-008 inc 4 — CONTRACT V1** (2026-09-26) — multi-port `AudioCtx`,
      cv/event payloads travel, rms→filter acceptance bit-exact, drift gate + defects #80–#83,
      toml nested-AoT fix, log_check baseline moved. 614 tests, goldens unchanged, stress hash
      unchanged. Sealed `sync wo008-inc4`.
- [x] **WO-014 inc 2** (2026-09-26) — the audio-domain six (9 modules total), aliasing
      acceptance −166.8 dB measured, six+one goldens, zero-alloc gates, docs generator + new
      CI/gates stage, `set_phase` on the osc. Sealed `sync wo014-inc2`.
- [x] **WO-008 inc 3** (2026-09-25) — tasks 4–7: Engine + 10k mutation stress + latency map &
      compensated fan-in alignment + watchdog auto-bypass + determinism harness (caught a
      HashMap-order nondeterminism pre-ship). Sealed `sync wo008-inc3`; device work: the loaded
      soak (needs SATURN + HAL).
- [x] **WO-006 inc 1.3** (2026-09-25) — the defect-#79 exclusive device-period ladder
      (`hal/period.rs` pure + Linux-tested; `open_exclusive` rewired; test004.bat text updated;
      sealed `sync wo006-inc13`, run sheet). Device-verified: NOT yet — that is test004's re-run.
- [x] **WO-013 inc 3** (2026-09-25) — module browser + fuzzy search, inspector with touch
      sliders + per-node param state (`Op::SetParam`, one drag = one undo step), wire-end
      re-patch, shell routing + provisional keyboard feed, painter for all three, 3 new audit
      smokes, test006.bat + run sheet. Live wire levels deliberately NOT done (need executor
      taps — now they exist, the painter half is WO-013 inc 4).
- [x] WO-013 inc 2b — delivery-path fix (#78); **test005 PASS on SATURN 2026-09-25 13:37**
- [x] WO-013 inc 2 — the bridge; WO-014 inc 1 — module batch + `sparq exec`; WO-008 inc 1–2 —
      graph core + executor; WO-006 inc 1.2 — integer exclusive rungs (#77 fix)
- [x] P1/P1B/P1C device runs — first MSVC builds green (golden bit-identical, selftest 8/8, 1059× realtime)
- [x] WO-007 complete — module contract as data + code; WO-012 inc 1 — gesture core + egui shell;
      WO-013 inc 1 — canvas model + painter; WO-000…WO-005 — workspace, tokens, ADRs, mockups,
      Phase A/B/C1 (DSP toolkit, music systems)

## Environment notes (sandbox) — re-read every session

- **`.git` is NOT durable in this workspace** (observed three times: 2026-09-25 twice,
  2026-09-26 the clone arrived without history and was re-inited; the 2026-09-27 clone DID
  arrive with the rebuilt history intact and commits held all session — treat that as luck, not
  as a change of rule). Do not rely on git for state;
  the integrity tool is `sync_check.py` (content hashes) and the seal is the zip. Re-init for
  in-session diffs if useful, expect it gone next session. **And the pushed tree can drift from
  the sealed one** (defect #86, found 2026-09-27): run `sync_check.py --quiet` against the
  PRISTINE clone at session start — if it fails, the drift list IS the repair list, and the
  bundle must carry the repairs plus the device-side forensics step.
- Toolchain wipes **between and mid-sessions** (again 2026-09-26 and 2026-09-27: `/opt` arrived
  empty — rustup, cargo, gcc and the apt packages all reinstalled from scratch, ~84 s with warm
  apt; within one conversation the install persists across turns). Lives in
  `/opt`, deliberately outside the snapshot. Restore:
  ```bash
  apt-get update && apt-get install -y curl ca-certificates gcc libasound2-dev pkg-config
  export RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo   # set BEFORE rustup: $HOME is /tmp here
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
       --default-toolchain stable --component rustfmt,clippy
  export PATH=/opt/cargo/bin:$PATH
  rustup target add x86_64-pc-windows-msvc   # cross-LINT only; never links
  ```
  (Setting RUSTUP_HOME/CARGO_HOME before the installer avoids the 2026-09-25 `/tmp/.cargo`
  gotcha entirely — the installer honours them.)
- Every cargo call needs the exports above (shell state does not persist between tool calls).
- **1 GB RAM is the binding constraint.** `ui-window`/wgpu tree: build/clippy with `-j 1` and
  `CARGO_PROFILE_DEV_DEBUG=0` (proven again 2026-09-27, 1 m 33 s clean). `ui --audit` +
  goldens: run release with `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`
  **and `CARGO_PROFILE_RELEASE_DEBUG=0`** (observed this session: the repo profile's release
  debuginfo=1 OOM-kills `read-fonts` at the default -j; with all three the release ui build is
  minutes, not a corpse) — SATURN builds with the repo profile. TWO cells do not fit even
  single-job, both the `windows`-crate rustc OOM class: MSVC×ui-window (standing) and
  MSVC×bootstrap-audio (observed this session — cpal pulls `windows` for the MSVC target; the
  documented bootstrap-audio cell is the NATIVE one, which passes).
- Background processes do NOT survive between tool calls — run long builds synchronously.
- Default workspace build has zero third-party deps → `cargo test --workspace` is cheap (~70 s).
- Gates sequence (`just gates` equivalent, plain commands): `cargo fmt --all --check` → clippy
  cells → `cargo test --workspace` → 5 python gates (`tools/check_text_io.py`, `token_gen.py
  --check`, `token_audit.py`, `unsafe_audit.py`, `module_docs.py --check`) → release golden tests
  → `selftest --golden` → `ui --audit`. `sync_check.py` runs at seal time, not in gates.
- **After any product-code change, re-seal:** `python3 tools/sync_check.py --write --sync <name>
  --sent <date>` then `--quiet` (verify) and `--self-test` (prove failable), rebuild the zip, and
  keep heading = list = contents counts equal in `SYNC.md` (the off-by-one class is checked, not
  hoped). New `scripts/*.bat` files ARE stamp-covered (covered set 134 files); `fp` counts only
  the five src roots (86f). **AND move `tools/log_check.py`'s BASELINE with the seal — defect #83
  was two increments of forgetting exactly that.**
- Python text I/O must be explicit `encoding="utf-8"` (+`newline="\n"` on write) —
  `check_text_io.py` gates it. `.bat` files: pure ASCII, CRLF, no unescaped parens in echo text
  inside blocks (#42/#69).
- Avoid literal workspace paths inside Python source (sandbox rewrites them); use relative + cwd.

## Open defects / debt (tracked in PHASE0-WORKORDERS.md build-log tables)

- **#79 — LOGGED + fixed in sandbox (inc 1.3), device-verified: HALF** — attempt 2 (2026-09-28)
  proved the asking works (exclusive opened for the first time) and then exposed #89: the period
  it landed on (the driver's min/granularity, 3 ms) is accepted by `Initialize` and unsustainable
  at runtime. Superseded by #89's fix (inc 1.4); acceptance is still the test004 re-run.
- **#80 — FIXED (contract v1):** the compiled HOA adapter offer named `spa/objects`; split into
  `HoaEncode`/`HoaDecode` with the matrix's ids, direction-aware, and objects↔non-spatial now
  refuses (no named converter). Caught by the new drift gate on its first run.
- **#81 — FIXED (contract v1):** the matrix's `cv→audio` adapter row was stale
  (`syn/sine-or-offset`) against the ADR-005 addendum's settled `util/offset`; the table now
  carries a `decided =` line naming the gate run.
- **#82 — PINNED + ESCALATED (contract v1):** `mono → ambisonics:N` is "compatible fan-out" in
  BOTH copies (the table's case order and the code's match arms agree) — replicating mono into an
  AmbiX bed is not valid spatial audio. Pinned by
  `the_mono_fanout_case_precedes_the_spatial_case_in_both_copies_flagged_for_review`; the fix is
  a matrix-review decision (`reviewed = false` is pending), not an implementer's side quest.
- **#83 — FIXED (contract v1):** `log_check.py`'s BASELINE frozen at WO-007 (388/68f) through
  two seals; moved with the seal (627 at wo014-inc3) with a provenance comment, and the seal
  recipe above now names it as a step.
- **#84 — FIXED (WO-014 inc 3):** `classification.top`'s closed domain lacked `env`/`mod` in
  both copies while Appendix B's ids and WO-014's artefact paths name them — `env/ad` could not
  declare its own top. Fixed table-first (TOPS 16 → 18) with the field table's first drift pin
  (`the_tops_vocabulary_matches_the_field_table`).
- **#85 — FIXED (WO-008 inc 5):** the exec evidence hashes were recorded without their
  durations — the CLI default renders 5 s (`drum-demo` → `de216da86b2a1869`) while the goldens
  are the 2.8 s/1 s/2 s renders; all three reproduce EXACTLY at their pinned durations (verified
  debug == release). The standing order now carries explicit `--seconds` everywhere the hashes
  appear, so a device operator compares like with like.
- **#86 — FIXED-IN-BUNDLE (WO-008 inc 5): the GitHub tree drifted from the sealed tree.** The
  pristine clone fails `sync_check --quiet` against its own stamp: module-api's lib.rs is 230B
  smaller than sealed and sync_check.py lost sparq-music from its covered/fingerprint roots (86f
  vs the stamped 89f). The history rebuild dropped bytes; the zips are truth. Repaired:
  sync_check.py restored to the six-root definition, and the inc-5 bundle carries BOTH files as
  declared drift repairs — **the device must copy its sealed versions to `logs\` BEFORE
  extracting** (the run sheet says so) and send them back for the diff of what the rebuild lost.
- **#87 — FIXED (WO-014 inc 6): the inc-5 inspector-scroll smoke was overfitted to one module's
  panel.** Its fixed two-finger drag distance (480 px) had been tuned so the mixer's LAST row was
  revealed at the 20-param `max_scroll`; when mixer 0.2.0 grew to 24 params the drag fell short
  and the gate failed on a healthy feature. No device exposure: the fragile cell only fails in
  combination with mixer 0.2.0, and both halves ship in the same bundle (`sync-wo014-inc6.zip`) —
  a device that applied wo013-inc5 alone runs the old smoke against the old mixer and passes.
  Fixed the right way: the smoke now asserts the CONTRACT (the panel overflowed before the drag;
  after it, some previously-hidden row is visible AND touchable where drawn) instead of a chosen
  row index or a distance. Lesson for smoke authors: a gate that encodes one module's arithmetic
  instead of the behaviour's shape will fail on the next additive version bump — which is exactly
  when it should pass.
- **#88 — FIXED (WO-008 inc 6, docs-only, found in passing): the author guide's §11 still called
  `util/mixer` "Phase 1, not yet built"** — stale since WO-014 inc 6 shipped the cv merge side.
  The same bullet carried the spline-refusal sentence this increment had to rewrite anyway, so
  both halves now tell the truth; no code, no device exposure. The lesson, turned on docs: a
  bundle that changes a contract sentence should grep the author guide for that sentence in the
  SAME session — the guide is the copy module authors actually read.
- **#89 — FIXED in sandbox (WO-006 inc 1.4), device-verified: pending (test004 attempt 3).**
  The exclusive stream opened at 288 fr (3.00 ms) — the driver's reported minimum / alignment
  granularity — and stalled: one ~33 ms stall every ~62.5 ms (159 late wakes / 10 s), half
  throughput (49.1k fr/s vs 96 kHz), drift −491 656 ppm, no clean tone; 0 budget overruns and 0
  FIFO starvations (the pump kept every promise it could see), and the shared 10 ms engine on the
  same endpoint soaked 2 h / 719 895 wakes / 0 xruns in the same session. **The third lying-`min`
  face: `Initialize` ACCEPTS the period and the engine cannot SUSTAIN it** — invisible to every
  open-path probe. The attempt-1 log copy in the repo is truncated, so which door produced the
  3 ms ask (a `GetDevicePeriod` min now reading 3 ms, or the two-step adopting a 288-fr
  granularity against a 10 ms ask) is not pinnable from the evidence — the fix closes all three
  doors: default-first ladder when min > block, round-UP two-step, silent-shrink guard (< ¾) with
  the ask printed in the open line. Pure data in `hal/period.rs`, Linux-pinned on the attempt-2
  device shape; arithmetic + fix story in `docs/hal/windows-notes.md` §4d. Parked in LATER.md:
  runtime stall detection (a sustained wake-interval ≫ period could trigger an auto-reopen at
  the default) — the open-path fixes above make it unnecessary for this device, and it deserves
  its own increment if ever wanted.
- **#90 — FIXED (WO-006 inc 1.4): `test004.bat` misdiagnosed attempt 2 inside the log it asked
  the operator to send back.** The summary's `EXCL_OK` keyed off rc alone, so an exclusive stream
  that OPENED and ran rough printed "exclusive still refused — the [04] probe table is the
  diagnosis" (a table that, correctly, was not in the log), and the banner still announced
  "increment 1.2" two increments later. Fixed: `EXCL_OPENED` captured via `findstr` on the [04]
  output, refused vs opened-but-not-clean summary lines, hints naming #89's runtime signature,
  expectation text naming both legal periods (960 fr / 1152 fr) and the 288-fr signature. The
  lesson: an acceptance script's verdict text is part of the acceptance — a wrong diagnosis in
  the send-back log costs a round-trip just like a wrong fix would.
- #69 — `build.bat` cmd-parser death: probe ships, culprit statement not yet named (stays open)
- compat-matrix mirror in `port.rs` vs `docs/api/compat-matrix.toml` — **re-worded (contract
  v1):** the drift gate pins the two together; full deletion waits on restructuring the table's
  prose `when` cells into machine predicates (review-packet change; ADR-005 addendum records it)
- WO-002: two typefaces still `chosen = ""`; WO-004: human protocols unrun; ADR-009 still `draft`
- `sparq-app/src/modules.rs`: no unit tests (CLI smoke only); `sparq-module-api`: no golden render
  of its own
- WO-013 inc 3 v0 limits (all parked in `LATER.md` with reasons): no numeric param entry, no
  inspector scrolling, no enum/text/blob editing (manifest v1 `options[]` first), single-selection
  inspector only, provisional keyboard feed for the browser, no browser scroll gesture
- WO-008 declared limits (parked in `LATER.md` §WO-008): ~~single-owner Engine until the kernel
  hot-swap primitive lands~~ and ~~meters not yet ring-published cross-thread~~ both SHIPPED
  (inc 5); remaining: compensation aligns PLAIN audio fan-in arms only; `required`-unconnected
  inputs not host-enforced (deliberate — the stress baseline moves with that change); `spline`
  interp refused until implemented; watchdog→UI red hairline is a WO-013-side painter increment;
  new from inc 5: one control + one audio thread (multi-reader epochs later), per-node meters
  fold a node's outputs (per-port later), `play` still pumps the static WO-005 graph (HAL
  integration waits on WO-006's acceptance)
- MSVC × `ui-window` clippy cell: never run anywhere (sandbox OOM, no CI cell)
- Device-side baselines that MOVE with the waiting increment (wo006-inc14; wo008-inc6's are on
  the device): gates.log test count **757** (log_check already moved — #83's discipline), stamp
  **`src 92f/2001521B`**, `sparq modules` count **17** (unchanged), `selftest --golden` prints
  **PASS (9 gates)** (unchanged), `ui --audit` **PASS (25 smokes)** (unchanged),
  exec's three pinned evidence hashes are UNCHANGED (`demo` 2.8 s `53de3b1f3f40e3c9`, `mod-demo`
  1 s `1621e1f65b1b64e1`, `drum-demo` 2 s `f2303f13aa0cf299` — inc 1.4 touches only the
  `cfg(windows)` exclusive open and its pure-data ladder; no render path moved), no new audio
  goldens, and test006's steps F–I (wo013-inc5) are the standing window-session asks. NEW with
  this bundle: every device log gets a `-digest.log` sibling at the end of its run, and the
  digest is the send-back artefact (the full logs stay on the device).
