# WO-014 increment 6 — `util/mixer` 0.2.0: the cv merge side (plan, 2026-09-27)

Checklist sandbox-track item 8, first entry: "mixer's cv merge side (retires the last of the
cv-fan-in refusal)". LATER.md §WO-009's spec: "Adding cv-in/cv-out ports to the mixer (or a
dedicated `util/cv-mix`) is a small increment; until then cv fan-in stays refused in words."

## What "retires the refusal" means — precisely

The RULE stays forever: cv fan-in is never implicit (compat-matrix G-cell `fan-in`: verdict
`adapter`, adapter `util/mixer`, "no implicit summing"). What retires is the refusal's
**"not built yet" half**: after this increment the named merge EXISTS, so two cv wires aimed at
one input are refused *with a working remedy* — wire each source to its own mixer cv input and
feed the destination from the mixer's cv output. The executor sentence is re-worded to say
exactly that; `tests/contract_v1.rs`'s pin ("names the merge module") keeps passing because the
new text still names `util/mixer`.

## Design decisions (recorded, not discovered)

1. **Mixer grows the ports, not a new module.** The matrix cell names `util/mixer`; the drift
   gate pins table ⇔ `port.rs` (`Adapter::Merge → "util/mixer"`); a dedicated `util/cv-mix`
   would fork that name and make the set eighteen against the WO's seventeen. The mixer manifest
   header always called the cv side its own ("cv inputs summing to a cv output").
2. **4 cv in → 1 cv out** (not 4×4): a 4×4 cv matrix is 16 more params = 36 > MAX_PARAMS(32).
   The merge bus the fan-in rule needs is N→1; four inputs with per-input gains mirror the audio
   side's philosophy (explicit, visible gains) at 4 params — 24 of 32 total.
3. **Block rate, unipolar, `cv_reduce = "mean"`.** Block: the merge serves modulation consumers
   (`flt/svf` cutoff-mod is block-rate); audio-rate sources (lfo, env/ad, tap.wave→no: bipolar)
   arrive host-reduced by the DECLARED policy — `mean` is the honest block summary of a merge
   (a `last` sample-and-hold of a fast envelope is an arbitrary pick); declared in the manifest,
   never re-decided in code (G3). Unipolar: every shipped modulation source and consumer is
   unipolar (lfo, env/ad, rms out; svf cutoff-mod in) — a bipolar port would refuse ALL of them
   today (`connect_cv` range mismatch → `util/range`, Phase 1, not built). Bipolar merge waits
   for `util/range`; declared, not faked.
4. **Values clamp at the merge.** Inputs clamp to the declared range per input (the svf
   precedent: a receiver clamps to what it declares); the f64 sum clamps to 0..1 on publish —
   the module's own documented behaviour (like the lfo's "shapes mapped into 0..1"), not a host
   transformation. Gains 0..2 like the audio cells.
5. **Identity default.** `cvm0 = 1.0`, `cvm1..3 = 0.0`: an untouched cv side is a bit-exact
   wire from cv-0 — the audio side's identity-default philosophy, and it makes the golden story
   additive. Params APPEND LAST (20..23) so every existing snapshot keeps its indices and every
   audio golden keeps its bits (the svf-0.2.0 discipline). Short snapshots read 0.0 past their
   end (`ParamSet` zero-fills) — existing 20-value tests render unchanged audio by construction.
6. **Status decoupling, stated.** `BlockStatus` remains the AUDIO contract: all audio inputs
   unconnected ⇒ audio outputs zeroed ⇒ `Silenced` — even while the cv side merges and
   publishes. A cv-only mixer is a legitimate patch citizen. Pinned by a test.
7. **Wrong-rate presentation is a host bug** ⇒ `Failed`, following the `env/ad` precedent
   (declared block-rate, an `Audio` slot means the host broke its own contract).
8. **`Adapter::Merge.first_phase` stays `Phase::One`.** The MODULE exists at Phase 0 — but the
   compiled connect functions never return `Merge` (the fan-in cell is prose), and the canvas's
   one-tap insert mechanism fits INLINE pair adapters, not a three-wire merge re-patch. The
   auto-insert OFFER for a merge stays Phase 1; declared here so the distinction is a decision,
   not an oversight.

## Changes

* `modules/util/mixer/sparqmod.toml` — 0.2.0: header + summary; ports `cv-0..cv-3` (in, block,
  unipolar, `cv_reduce = "mean"`, optional) and `cv-out` (out, block, unipolar), appended after
  `out-3`; params `cvm0..cvm3` ("Cv N Gain", 0..2, defaults 1/0/0/0) appended after `trim3`.
* `crates/sparq-audio/src/modules.rs` — `Mixer::process` grows the cv merge (params 20..23,
  f64 accumulate, clamped publish via `take_cv_out(0).set`), computed BEFORE the audio-side
  `Silenced` early return; doc comment updated (v0.2.0 shape, the clamp + decoupling rules).
* `crates/sparq-audio/src/executor.rs` — the fan-in refusal re-worded to the working remedy;
  the crate header's "`cv` fan-in is refused naming `util/mixer` (same reason)" line corrected
  (the reason is no longer "not in this build").
* `crates/sparq-audio/tests/mixer_cv.rs` — NEW gate file (the `tempo_sync.rs` pattern for a
  module version bump).
* `docs/modules/util-mixer.md` — regenerated (`module_docs.py`), never hand-edited.
* LATER.md — the mixer item moves to shipped; the 4×4-not-8×8 note gains the cv side's numbers.

## Proof burden (tests/mixer_cv.rs)

1. **Identity default is a bit-exact cv wire**: rms → cv-0, defaults ⇒ `cv-out` == the rms
   published value, bit for bit.
2. **The merge sums by the declared gains, hand-computed**: two rms chains (different amps) into
   cv-0/cv-1 with dyadic gains ⇒ out == the exact f32 of the f64 arithmetic.
3. **The output clamps to the declared range** (gain 2.0 × a hot source ⇒ exactly 1.0).
4. **An audio-rate source arrives reduced by the DECLARED policy**: lfo → cv-0 ⇒ `cv-out` ==
   the arithmetic mean of the lfo's own published buffer in the same block (read through
   `node_cv_audio` — hand-computed, not trusted).
5. **The audio side is untouched**: the v0.1.0 identity/cell behaviour re-asserted at 0.2.0
   with a FULL 24-value snapshot (and batch-3's 20-value goldens keep passing unchanged — the
   additive proof).
6. **Status decoupling**: no audio connections + a live cv merge ⇒ audio out exact zeros,
   status `Silenced`, cv-out hot.
7. **The refusal now carries the remedy**: two cv edges into one input ⇒ `EdgeRefused` naming
   `util/mixer` AND its `cv-N`/`cv-out` ports.
8. **Zero allocations**: the cv merge over 5 000 process calls with connected cv (the batch
   discipline).
9. **The snapshot order pin**: params 20..23 are `cvm0..cvm3`, ports 8..12 are the cv side —
   the drift pin that keeps "appended LAST" true.

## Gates

Every pre-existing golden UNCHANGED (batch-2/3/4/5, drum-demo `914d9063ce9d8a0f`, the three
pinned exec renders, stress `b42068ec7b206789`) — the audio path and the executor's render path
do not move; `modules --strict` 17/17 (no new module); docs regenerated + `--check`; fmt/clippy
cells; the 5 python gates; selftest 9/9; `ui --audit` PASS 25 smokes (the canvas is untouched —
mixer cv ports appear in the browser's specs automatically, #58-style, from the manifest).
Seal: `sync wo014-inc6`, log_check BASELINE moves (test count + stamp), SYNC.md/CHECKLIST.md/
PHASE0 build log updated, test006/gates expectations unchanged except the test count.

## Out of scope (declared)

Bipolar cv merge (waits `util/range`, Phase 1); a 4×4 cv matrix (waits MAX_PARAMS); the canvas
auto-insert offer for merges (three-wire re-patch shape, Phase 1); `cv_interp = "spline"` (its
own increment); per-port audio meters (live-HAL increment).
