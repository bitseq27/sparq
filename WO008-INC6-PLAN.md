# WO-008 increment 6 — `cv_interp = "spline"`: the host performs the last word of the G4 vocabulary (plan, 2026-09-27)

Checklist sandbox-track item 8, second entry — named there as "the next candidate in this list":
`cv_interp = "spline"` (refused at build until implemented). LATER.md §WO-008's spec: "declared in
the vocabulary and REFUSED at build until the host implements it (hold and linear ship).
Implementing it is a small, well-specified increment; faking it with a hold is exactly the silent
transformation the refusal exists to prevent." WO014-INC6-PLAN's out-of-scope column declared it
"its own increment". This is that increment.

## What "implements it" means — precisely

The G4 cell (compat-matrix, DECIDED 2026-09-22): "the host interpolates; the receiving module
declares ports[].cv_interp (hold|linear|spline); default hold — module declares, host performs."
The vocabulary was frozen in WO-007 with `spline` in it and the host refusing it in words. After
this increment the host PERFORMS all three spellings, the build refusal retires, and the curve the
word names is written down in one compiled copy (port.rs), one machine-readable copy (the matrix
cell) and the two prose copies (module-api §17, author guide) — pinned against each other by the
drift gate, the same discipline the fan-in remedy got in inc 6 of WO-014.

## Design decisions (recorded, not discovered)

1. **The curve: the parabola through the last three block values.** When block N expands
   (knots `v[N-2] = prev2`, `v[N-1] = prev`, `v[N] = cur`), frame `i` of `n` reads
   `t = i/n` on the unique quadratic through `(−1, prev2), (0, prev), (1, cur)`:

   ```
   v(t) = prev2 · t(t−1)/2  +  prev · (1−t²)  +  cur · t(t+1)/2
   ```

   Equivalently: a cubic Hermite whose start tangent is the central difference `(cur−prev2)/2` and
   whose end tangent is the second-order backward difference `(3·cur − 4·prev + prev2)/2` — the two
   formulations are the same curve (both match value and slope at both ends of the interval, and
   four constraints determine one cubic). Chosen properties, each pinned by a gate:
   * **exact for any signal quadratic in block index** (three knots determine the parabola) — the
     strongest accuracy claim a causal three-knot scheme can make;
   * **degenerates to the exact line for collinear knots** (a block-rate ramp renders
     bit-identically to the same wire declaring `linear`);
   * **C⁰ at the knots and the SAME arrival contract as `linear`**: `v(0) = prev` exactly, the
     curve reaches `cur` at the next block boundary — declaring `spline` changes the shape of the
     ride, never its timing (no added latency, no phase shift against the docs' linear sentence);
   * **constant in ⇒ constant out, bit-exact** (the coefficients sum to 1).
2. **Causal, not delayed.** True Catmull-Rom needs `v[N+1]` (next block's value — does not exist
   yet); a natural cubic spline needs a global solve over future knots. The alternatives both buy
   smoothness with a block of LATENCY, which would make `spline` time its wire differently from
   `linear` — rejected. The one-sided end tangent is the standard second-order backward estimate;
   the curve carries momentum from history and may locally exceed the knot span (all interpolating
   splines do — the inertia IS the smoothness), which decision 4 bounds. A monotone-preserving
   variant (Fritsch–Carlson/PCHIP) was considered and rejected: it fails the quadratic-exactness
   property (its limiter bends parabolas near extrema) and adds branching per frame for a safety
   property the declared-range clamp already provides where it matters (the wire's range).
3. **State: one more f32 per edge, zero allocations.** `CvPlan` grows `prev2: f32` beside `prev`
   and `range: CvRange` (the destination's declared range, captured at build where G2 already
   proved source and destination compatible). Both history slots start at **0.0** — the same
   declared zero-history ramp `linear` has always had ("block 0 ramps from the initial 0.0" is an
   existing pin); the first two blocks are a documented startup transient, deterministic and
   gated. The audio path allocates nothing, as before.
4. **Range discipline: the spline branch clamps to the wire's declared range; hold/linear do
   not.** The host must never hand a receiver frames outside the range the receiver declared —
   that is what declaring a range means. `hold` and `linear` cannot exceed their knots (repeat /
   convex combination), so for them an out-of-range frame can only come from an out-of-range
   SOURCE — a module bug that must stay visible, never silently clamped away. `spline`'s overshoot
   is host-made, so the host bounds it: clamp to `range` inside the same branch, documented in
   words in all four copies. This is part of performing the declared interpolation, not a silent
   transformation — the same distinction the mixer's own publish-clamp drew in WO-014 inc 6.
5. **One signature, one copy of the rule.** `CvInterp::expand` grows to
   `expand(self, prev2, prev, cur, range, dst)`. Clamping in the executor instead was rejected:
   the rule would live beside the math it bounds, and any second consumer of `expand` would have
   to re-implement it — "answering it twice, in two places, is how the table and the code drift"
   (the matrix's own G4 gap note). The api-snapshot pin moves on purpose, with the reason at the
   pin; the executor is the only caller.
6. **f64 arithmetic, one cast per frame.** The parabola is evaluated in f64 and rounded to f32
   once — the rule inc 6 established for the mixer's cv sum ("host-side cv transformations
   accumulate in f64"). Rust never contracts into FMA, so every operation is IEEE-exact per
   instruction and the render is bit-identical across sandbox and SATURN (ADR-007).
7. **The refusal retires; its philosophy is kept.** The build refusal existed because the
   vocabulary promised what the host did not perform. The host now performs it, so the refusal
   block is deleted — every other cv refusal (range, fan-in, delay-kind, rate) stands untouched.
   `contract_v1.rs`'s refusal gate is REPLACED by a behaviour gate at the same spot (the file's
   expansion test keeps hold + linear; the spline's proofs get their own gate file, the
   `mixer_cv.rs` precedent), and the drift gate grows a pin that the matrix cell now carries the
   curve's definition in words.
8. **Nothing else moves — by construction.** `hold`/`linear` branches keep their exact code, no
   shipped module declares `spline` (grep-verified), no demo patch contains a block→audio cv edge,
   and the stress/determinism generators build from the registry's manifests and never synthesise
   an interp — so every pre-existing golden, the stress hash `b42068ec7b206789` and the three
   pinned exec renders are unchanged not by luck but by the shape of the diff.

## Changes

* `crates/sparq-module-api/src/port.rs` — `CvInterp::Spline` doc: the definition (was "NOT yet
  implemented"); `expand`: new signature, the Spline branch (closed form + clamp), the Hold arm's
  "spline never reaches here" comment retired; two unit tests (closed form + clamp, collinear
  degeneracy).
* `crates/sparq-module-api/tests/api_snapshot.rs` — the `expand` pin moves (signature widened on
  purpose; reason recorded at the pin).
* `crates/sparq-module-api/tests/compat_matrix.rs` — the G4 pin grows: the cell's condition text
  must carry the curve's definition, not just the three names.
* `crates/sparq-audio/src/executor.rs` — the spline refusal block deleted; `CvPlan` grows
  `prev2` + `range`; the build pass captures the destination's declared range; the wire pass
  shifts the two-slot history and calls the widened `expand`; crate-header and `EdgeRefused` doc
  lines corrected (the refusal inventory no longer lists spline).
* `crates/sparq-audio/tests/cv_spline.rs` — NEW gate file (8 gates, below).
* `crates/sparq-audio/tests/contract_v1.rs` — `a_receiver_declaring_spline_is_refused_at_build_
  in_words` replaced by `a_receiver_declaring_spline_gets_the_declared_curve` (builds now; the
  first blocks match the hand-computed parabola; deep gates live in `cv_spline.rs`).
* `docs/api/compat-matrix.toml` — the G4 cell's condition gains the definition sentence.
* `docs/api/module-api-v1.md` §17 — "`spline` refused until implemented" → performed, with the
  definition and its gate names.
* `docs/api/manifest-schema.md` — the `cv_interp` row gains the definition pointer.
* `docs/module-author-guide-v0.md` — both "refused at build" mentions rewritten to the performed
  rule (what declaring `spline` gets you, including the clamp and the startup transient).
* `tools/log_check.py` — BASELINE moves with the seal (test count + stamp; defect #83's lesson).
* LATER.md — the spline bullet moves to shipped. PHASE0-WORKORDERS.md — §2.1 row + build-log
  entry. CHECKLIST.md / SYNC.md / SYNC-STAMP.txt — the seal.

## Proof burden (tests/cv_spline.rs)

1. **The hand-computed parabola**: a scripted dyadic knot sequence (0.5, 1.0, 0.75, 0.75, 0.25 —
   chosen so every coefficient is a small dyadic and every frame is f32-exact) rendered through a
   spline wire; EVERY frame of EVERY block compared `==` against the independently simplified
   polynomials (block 2 carries the clamped overshoot frames, block 3 the momentum dip).
2. **A constant signal expands bit-exact flat** (the identity: coefficients sum to 1).
3. **For a block-linear ramp, spline and linear wires agree bit-for-bit** — two graphs differing
   only in the declared spelling, frames compared exactly (the degeneracy pin).
4. **A quadratic sweep is reproduced exactly**: the source publishes `v[N] = N²/64`; from the
   third block on (history full) every frame equals `((N−1)+t)²/64` bit-exactly — the
   exactness claim measured, with the startup transient named, not hidden.
5. **Every block starts at the previous knot**: over a deterministic pseudo-random dyadic
   sequence, `frame 0 == prev` bit-exact across blocks (C⁰ at the knots = the arrival contract).
6. **Startup is the declared zero history, and deterministic**: the first two blocks match the
   hand-computed ramp-from-(0,0); a rebuild renders byte-identical (ADR-007 at wire scale).
7. **Overshoot clamps to the declared range on BOTH polarities**: unipolar frames clamp to
   exactly 1.0 (never above), the mirrored bipolar sequence clamps to exactly −1.0, and the
   un-clamped interior frames of the same curve keep their hand-computed values (the clamp
   bounds; it does not reshape).
8. **Zero allocations**: the spline wire over 1 000 rendered blocks under the counting allocator.

## Gates

Every pre-existing golden UNCHANGED (decision 8 — by the shape of the diff); stress hash
`b42068ec7b206789` unchanged (debug AND release); the three pinned exec renders at their
`--seconds` unchanged; `modules --strict` 17/17 (no module touched); fmt/clippy cells (the two
documented `windows`-crate MSVC OOM cells stay out, everything else runs); the 5 python gates;
selftest 9/9; `ui --audit` PASS 25 smokes (the canvas never saw the refusal — interp lives in the
executor, not in `connect_cv`). Seal: `sync wo008-inc6`, log_check BASELINE moves, SYNC.md /
CHECKLIST.md / PHASE0 build log updated; `gates.bat` expectations change ONLY in the test count
and the stamp (both live in `log_check.py`, which travels in the bundle).

## Out of scope (declared)

Any shipped module adopting `spline` (a module change moves goldens — its own increment, on
purpose, with the audible A/B recorded); ADR-006's stream-bus `spline` (data channels, Phase 5);
per-frame range clamping for hold/linear (decision 4 — source bugs stay visible); monotone
(PCHIP) variants; the live-HAL increment (still gated on WO-006's device acceptance).
