# WO-014 increment 7 — the zipper-noise fix: per-block coefficient ramps (plan of record, 2026-09-30)

**Operator report (device run of inc6):** *"when adjusting the main out volume or sine
amplitude the audio crackles — fix that."* The mechanism, named: parameter edits cross the
command ring and land as a fresh snapshot at a block boundary; every gain-like module then
multiplies the whole next block by the NEW coefficient while the previous block used the OLD
one — a hard step of `Δcoeff × signal` in the waveform, 750 times a second during a drag.
That is zipper noise, and it is exactly what a crackle under a fader sounds like.

## D1 — The ramp, and the bit-exact static path.

`dsp::core::coeff_ramp(cur, target, frames) -> (start, step)`: the block starts at the
currently-applied coefficient and ends at the target, linearly; `step` is **zero** when the
coefficient already equals the target. The zero-step case is a constant multiply — bit-identical
to today's code — which is what keeps every checked-in golden (all rendered from static params)
bit-exact. First block after construction primes `applied = target` (NaN sentinel), so a patch
that loads with non-default params starts constant, never fades in from zero.

One block of ramp (64 frames ≈ 1.3 ms at 48 kHz) turns a step of up to 2.0 into a per-sample
slope of 0.031 — below the signal's own slope, i.e. inaudible as an event; a drag becomes a
continuous glide because `applied` carries across blocks.

## D2 — The class of coefficients that get it (the named two, plus their kin).

* `syn/sine` amplitude (the operator's named case);
* `out/main` trim (the operator's named case) — **mute keeps its immediate exact zeros**: a
  stage cut is a safety, not a fade, and its zeros are bit-exact by design;
* `util/gain` gain;
* `util/mixer` the 16 cell gains + 4 output trims;
* `util/panner` the two law coefficients.

Frequency/phase params, filter coefficients and cv-side gains are NOT ramped here: a frequency
step is a slope change, not an amplitude discontinuity (no click), and SVF coefficient ramps are
a design of their own (state continuity) — declared, not forgotten.

## D3 — The measurement.

`tests/param_ramp.rs`: an executor render of `sine → out/main` at amplitude, editing amp and
trim mid-render through the same command door a drag uses; the assertion is on the WAVEFORM:
the largest sample-to-sample jump across the edited block boundary stays under the signal's own
maximum slope plus the ramp's, where the un-ramped step would be the full Δ. A second gate
asserts the static render is bit-identical to a direct constant-coefficient computation (the
golden's property, stated locally).

## Acceptance

* `tests/param_ramp.rs` green; every existing golden bit-identical (the static path is the
  zero-step path); workspace tests, clippy cells, selftest, `modules --strict` green.
* Device ask: drag the main-out trim and a sine amplitude again — the crackle is the thing
  that must be gone.

## Addendum — increment 7b: the stereo master's ramp advanced per channel, not per frame

The operator's second report was precise: *"the main out trim still crackles when moved, using a
pure sine tone."* The ear was right and the first draft of the fix was wrong in one line: the
`out/main` ramp advanced inside the `stereo_tick_f` closure on every call — and the closure runs
once per CHANNEL per frame. On a stereo device the glide therefore travelled twice its distance
each block (cur → past the target → the stored end value), and the snap back to the target at
the block boundary was a step per block under a drag: the crackle, surviving on exactly the knob
that was reported. The mono modules (sine, gain) and the per-frame loops (mixer, panner) were
correct; the master was not.

The fix is one condition — advance on the frame's first channel (`f > 0 && c == 0`) — and the
gate that would have caught it: `tests/param_ramp.rs` now recovers the per-sample coefficient
from the deterministic sine (`out = sin · amp · g`) and pins the change block's geometry: it
STARTS at the old coefficient and ENDS at the target, no overshoot, and the boundary into the
settled block carries only the signal's own slope. On the buggy code the recovered end
coefficient is 0.0 against a target of 0.5 — the gate fails by a mile, which is the proof it is
a gate and not a comment. Device channel count must not change how far a glide travels; that
sentence is now in the code.

Measured: 808 tests (+1) · release goldens bit-identical · selftest 9/9 · every clippy cell ·
5 python gates · `ui --audit` 50 smokes unchanged. Sealed `sync-wo014-inc7b.zip`.

## Addendum 2 — increment 7c: the one-pole glide (the operator's ear, third round)

*"I can still hear bumps when moving the trim slider quickly."* The 7b ramp was continuous but
wrong-shaped: a linear ramp that reaches its target in one block (1.3 ms) and then HOLDS until
the next UI snapshot (~16 ms) is a STAIRCASE under a fast drag — sharp kinks at the update rate,
and kinks at 60 Hz are exactly bumps. The fix replaces the per-block linear ramp with a
**one-pole glide** (`dsp::core::glide`, tau = 25 ms, declared in `GLIDE_TAU_MS`): the coefficient
chases the moving target every sample (per sample on mono modules, once per frame on the
multi-channel ones — device channel count still cannot change how far a glide travels), so the
envelope is smooth inside every block and continuous across every boundary, at the cost of a
declared fader lag well inside "motorised fader" feel. A **snap at −80 dB** (`glide_snap`,
coefficient distance 1e-4) lands settled edits EXACTLY, so settled and static renders are the
constant multiply again — every checked-in golden stays bit-exact, and the prime rule (NaN
sentinel, first block constant) is unchanged.

The gate that names the bump: `tests/param_ramp.rs` now drives a FAST drag — the trim swept
1.0 → 0.0 with one edit per block, faster than any hand — and asserts (a) no boundary jump in
the sweep beyond the signal's own slope, (b) block peaks fall monotonically (no overshoot, no
snap-back), (c) both channels of every frame share one coefficient (7b's bug, still pinned),
(d) settled at zero the output is EXACT silence. The gain unit test now asserts the glide
contract (strictly between the coefficients after one block, bit-exact once settled), and live
smoke 27 pumps past two time constants before asking whether the level arrived — the glide's
lag is declared, param_ramp gates its shape.

Measured: 808 tests · release goldens bit-identical · selftest 9/9 · 434.8× realtime · every
clippy cell runnable in the sandbox · 5 python gates · `ui --audit` PASS, 50 smokes. Sealed
`sync-wo014-inc7c.zip`.
