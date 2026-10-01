# Sync manifest — WO-014 increment 7c (the one-pole glide: the fast-drag bumps, dead)

**Current bundle: `sync-wo014-inc7c.zip` (11 entries, listed below).** It stacks on
`sync-wo014-inc7b.zip` (applied). Extract at the repo root `Q:\morphosis\code\sparq`,
overwriting. No drift-repair copy step. No device contract moves. Gate numbers: tests stay
**808** (the sweep gate replaced the 7b geometry gate; the gain unit test was rewritten to the
glide contract), smokes stay **50** (live smoke 27 now pumps past two glide taus — the lag is
declared, not hidden), the stamp moves.

**The stamp changes: see `SYNC-STAMP.txt`** (same **155-file** covered set, new byte count —
`core.rs`, `modules.rs`, `param_ramp.rs`, `headless.rs` moved). `Cargo.lock` stays SOFT and is
NOT in the zip. No manifest moved — 17/17.

## What this is

Your third ear-report named the shape error exactly: bumps under a FAST drag. The 7b ramp was
continuous but staircase-shaped — it reached each snapshot's target in one block (1.3 ms) and
held until the next UI snapshot (~16 ms), so a quick fader move produced sharp kinks at the
update rate. Kinks at 60 Hz are bumps.

The ramp is now a **one-pole glide** (`dsp::core::glide`, tau = `GLIDE_TAU_MS` = 25 ms): the
coefficient chases the moving target every sample (per frame on the multi-channel modules —
channel count still cannot change how far a glide travels, 7b's fix stands inside this one).
The envelope is smooth inside every block and continuous across every boundary; you get a
motorised-fader feel (~25 ms of lag, declared) instead of a staircase. A **snap at −80 dB**
lands settled edits EXACTLY on the target, so settled and static renders are the constant
multiply again — every checked-in golden stays bit-exact, and the first-block prime rule is
unchanged. Mute still cuts in exact zeros immediately.

The new gate drives a faster-than-any-hand drag (trim 1.0 → 0.0, one edit per block) and
asserts: no boundary jump beyond the signal's own slope anywhere in the sweep; block peaks fall
monotonically (no overshoot, no snap-back); both channels of every frame share one coefficient;
settled at zero the output is exact silence. On the staircase code the boundary jumps and the
hold-then-jump envelope fail it.

**Measured:** 808 tests (+1 ignored) · release goldens bit-identical (`ba577186c988db21`,
`dd975a24f03b19c1`, stress `7bb06379bd6845e5`) · selftest 9/9 · 17/17 · 434.8× realtime · every
clippy cell runnable in the sandbox · 5 python gates · `ui --audit` PASS, 50 smokes.

**THE ASK, one line:** drag the trim FAST again. The staircase kinks were the bumps; the glide
has none. Then, stage time permitting: test004 attempt 4 (still the WO-006 acceptance; the
ADR-008 exit stays unfired) and the gates digest at 808 / the stamp below.

**Waiting:** this bundle only.
Applied chain (newest first): `sync-wo014-inc7b.zip` (9), `sync-wo014-inc7.zip` (12),
`sync-wo012-inc6.zip` (27), `sync-wo012-inc5c.zip` (16), `sync-wo012-inc5b.zip` (21),
`sync-wo012-inc5.zip` (24), `sync-wo012-inc4.zip` (23, operator-confirmed: PLAY makes sound),
`sync-wo012-inc3.zip` (24), `sync-wo013-inc6.zip` (16), `sync-wo012-inc2.zip` (20),
`sync-wo008-inc7.zip` (17) — all operator-reported APPLIED — then `sync-wo006-inc15.zip` (15),
`sync-wo006-inc14.zip` (15), `sync-wo008-inc6.zip` (17), `sync-wo014-inc6.zip` (13),
`sync-wo013-inc5.zip` (20), `sync-wo014-inc5+wo013-inc4.zip` (29), `sync-wo008-inc5.zip` (22),
`sync-wo014-inc4.zip` (12), `sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19),
`sync-wo008-inc4.zip` (38), `sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15),
`sync-wo006-inc13.zip` (10), `sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`,
`sync-p1b.zip`, `sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`,
`sync-wo012-inc1.zip`, `sync-wo006-inc11g.zip`.

## Namelist

```
CHECKLIST.md
EXTRACT-AT-REPO-ROOT.txt
README.md
SYNC-STAMP.txt
SYNC.md
WO014-INC7-PLAN.md
crates/sparq-app/src/ui/headless.rs
crates/sparq-audio/src/dsp/core.rs
crates/sparq-audio/src/modules.rs
crates/sparq-audio/tests/param_ramp.rs
tools/log_check.py
```
