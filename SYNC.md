# Sync manifest — WO-014 increment 2: the audio-domain module batch (9 modules total)

**Current bundle: `sync-wo014-inc2.zip` (29 entries, listed below).** Applies on top of
**`sync-wo008-inc3.zip`** — which the operator confirmed applied on SATURN on 2026-09-26, with
`wo013-inc3` and `wo006-inc13` beneath it. Extract at the repo root `Q:\morphosis\code\sparq`,
overwriting.

**The stamp changes: `src 85f/1566114B`** (`SYNC-STAMP.txt` regenerated for the whole 133-file
covered set — the six new manifests and the new tool are covered files). `build.bat`'s guard
forces exactly one rebuild (#49's remedy), and its sync check names any file that did not land
before cargo runs (#78's remedy).

## What this is

**WO-014 increment 2**, sandbox-built and sandbox-green on 2026-09-26 — the module set triples to
**nine**. The build-log entry in `PHASE0-WORKORDERS.md` (search **increment 2 — the audio-domain
batch**) carries the full story. Headline:

1. **Six new modules**, each a contract-conforming wrapper of a Phase-B DSP primitive that
   already earned its measurements: `syn/noise` (five colours, seeded, `reset` replays the
   stream), `syn/polyblep` (the stable id kept — the implementation is the bandlimited additive
   that replaced the failed PolyBLEP per defect #12, and the manifest says so in words),
   `flt/svf` (five modes, defect #11's prepare-before-coefficients discipline honoured),
   `util/delay` (the set's first **parametric latency**: `latency = "param:time"`),
   `fx/bitcrush` (seeded per-channel dither), `util/panner` (equal-power + linear laws).
2. **The acceptance item measured, not inherited:** `syn/polyblep` aliasing through the
   registry-built module at 48 kHz — **−166.8 dB** below the fundamental against the WO's −60 dB
   bar; the test prints the number into every log.
3. **Goldens:** six module goldens + one five-module chain golden (`9170415cd3852736`) checked
   in; the chain renders bit-identical twice under the counting allocator; per-module 5 000-call
   zero-allocation gates. **Existing goldens UNCHANGED** (`ba577186c988db21`,
   `0f5c3e86c7f117a9`, `d46736fd9a1c48a1`) — the demo patch renders identically with a registry
   three times its size.
4. **Generated docs** (the "no hand-written duplicates" acceptance): `tools/module_docs.py`
   renders `docs/modules/*.md` from the same TOML bytes discovery and `include_str!` read;
   `--check` is a new gate wired into `justfile`, `ci.yml` **and `scripts\gates.bat`**, proven
   failable before shipping (hand-edit → `STALE` → exit 1).
5. **The eight modules that did NOT ship are declared, not forgotten** (`LATER.md` §WO-014):
   membrane / env-ad / lfo / clk-div / mixer need the multi-port `AudioCtx` (contract v1) and
   WO-009's clocks; tap / scope need the ring publication; `out/main` ships together with the
   canvas master-handover so the MASTER badge never lies.

**Measured in sandbox:** **559 tests** (was 549; +10) · fmt clean · clippy clean in the default,
`ui`, `bootstrap-audio`, MSVC×3 cells · `selftest --golden` 8/8 · `sparq ui --audit` PASS, 0
failures, 19 smokes (the browser smoke still finds exactly one "gain" — the six new
names/ids/categories were checked against the fuzzy scorer before shipping) · `sparq modules
--strict` **9/9** from disk · 6 Python gates clean.

## Files in this zip (29)

Covered by the stamp — 10:

```
crates/sparq-audio/src/dsp/osc.rs            (set_phase: the state-restore half of the phase blob)
crates/sparq-audio/src/modules.rs            (six implementations; BUILTINS 3 → 9)
modules/syn/noise/sparqmod.toml              (new)
modules/syn/polyblep/sparqmod.toml           (new)
modules/flt/svf/sparqmod.toml                (new)
modules/util/delay/sparqmod.toml             (new)
modules/fx/bitcrush/sparqmod.toml            (new)
modules/util/panner/sparqmod.toml            (new)
scripts/gates.bat                            (one new :run line — the module-docs gate)
tools/module_docs.py                         (new)
```

Tests — 2 (not stamp-covered; tests do not decide the binary, but `gates.bat` runs them and the
log's test count depends on them landing):

```
crates/sparq-audio/tests/modules_golden.rs   (registry/discovery/drift counts now BUILTINS-driven)
crates/sparq-audio/tests/modules_batch2.rs   (new — goldens, gates, the aliasing acceptance)
```

Gate wiring — 2 (not stamp-covered):

```
.github/workflows/ci.yml                     (module-docs check step)
justfile                                     (python-gates gains module_docs --check)
```

Generated docs — 9 (regenerable; shipped so `--check` passes on SATURN without running the
generator first):

```
docs/modules/ana-rms.md        docs/modules/flt-svf.md      docs/modules/fx-bitcrush.md
docs/modules/syn-noise.md      docs/modules/syn-polyblep.md docs/modules/syn-sine.md
docs/modules/util-delay.md     docs/modules/util-gain.md    docs/modules/util-panner.md
```

Documents — 6 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   README.md   SYNC.md   SYNC-STAMP.txt
```

Heading = list = contents = **29** (10 + 2 + 2 + 9 + 6), checked programmatically against the
zip's namelist at packing time.

## What SATURN runs

No new device script — the standing order stacks, with two baseline numbers moved:

1. `scripts\test004.bat` — the WO-006 exclusive acceptance (unblocked by inc 1.3; pass shape in
   `WO006-INC13-RUN-SHEET.md`).
2. `scripts\test006.bat` — the WO-013 window session (browser/inspector/re-patch under real
   fingers; step [07] is the slider→render hash claim).
3. `scripts\gates.bat` — full table, now including the module-docs gate. **Expected in
   `gates.log`: 559 passed / 0 failed / 1 ignored**, and `sparq modules --strict` lists **9**.
   (`tools\log_check.py`'s baseline expectations move with these numbers — a mismatch against an
   OLD baseline is the checker being stale, not the tree.)
4. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix if a physical screen is reachable, and — when the HAL and the
   machine are both free — the WO-008 **loaded soak** (200 modules, 30 min, zero xruns).

## Superseded bundles

`sync-wo008-inc3.zip` (15 — applied), `sync-wo006-inc13.zip` (10 — applied),
`sync-wo013-inc3.zip` (19 — applied), `sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`,
`sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`,
`sync-wo006-inc11g.zip` — each applied on top of the previous, in that order.
