# Sync manifest — WO-014 increment 4: `mod/lfo` + `mod/clk-div` (WO-009's first consumers)

**Current bundle: `sync-wo014-inc4.zip` (12 entries, listed below).** Applies on top of
**`sync-wo009-inc1.zip`** — and the user confirmed 2026-09-26 that everything through
wo009-inc1 is APPLIED on SATURN (wo013-inc3 → wo006-inc13 → wo008-inc3 → wo014-inc2 →
wo008-inc4 → wo014-inc3 → wo009-inc1). Extract at the repo root `Q:\morphosis\code\sparq`,
overwriting.

**The stamp changes: `src 89f/1782456B`** (`SYNC-STAMP.txt` regenerated for the whole 143-file
covered set — the two new manifests are covered files; no new `src` file, the modules live in
`sparq-audio::modules` like every batch before them).

## What this is

**WO-014 increment 4**, sandbox-built and sandbox-green on 2026-09-26 — the module set reaches
**fourteen** with the two `mod`-family modules the clocks unblocked (and the taxonomy family
defect #84 added to the closed domain exists for). The build-log entry in
`PHASE0-WORKORDERS.md` (search **increment 4 — the two modules the clocks unblocked**) carries
the full story. Headline:

1. **`mod/lfo`** — four shapes (sine/tri/saw/square) mapped into 0..1, an **audio-rate unipolar
   `cv` output**, phase reset from the `phase-reset` message or a `sync` event at its EXACT
   sample (the transport's beat triggers are the intended clock). Two decisions recorded in the
   manifest header, not buried: **unipolar** (the matrix refuses bipolar→unipolar rather than
   rescale — G2 — and its converter `util/range` is Phase 1, not built; a bipolar port today
   could not connect to a single shipped consumer), and **sync = event-driven phase reset**
   (continuous tick-derived phase needs a per-frame tick view or a module-side bpm derivation —
   LATER.md, decide-before-building). State is the 8-byte phase (the `syn/sine` promise).
2. **`mod/clk-div`** — the set's first event-PROCESSING module (events in, events out). Divide
   counts from the first input (a divider never swallows the downbeat of its own count);
   multiply schedules M−1 sub-triggers evenly across the interval to the next passing input,
   MEASURED from the previous arrival, fired from a bounded 32-slot internal schedule on exact
   samples in later blocks (overflow dropped-and-counted, never grown); the probability gate is
   an independent seeded draw (xorshift64 from the 8-byte state seed — same seed, same decision
   stream; `reset` replays). All semantics are in the manifest header AND pinned by exact sample
   lists in the gates.
3. **The acceptance-shaped gates** (`tests/modules_batch4.rs`, 10 of them): **lfo → svf cutoff
   renders BIT-IDENTICAL to a hand-driven reference** over 200 blocks (audio-rate cv reduced
   `last` into the block-rate input — the contract-v1 acceptance pattern reused; the sweep ends
   at 289.445 Hz and rising, so it is a real sweep, not a constant); **host 16ths → clk-div ÷4 →
   membrane** lands kicks on exactly frames 0/24576/49152/73728 with the chain golden
   **`914d9063ce9d8a0f`** (2 s, debug == release). The LFO's arithmetic gates run at a
   **binary-exact rate** (48000/2^15 Hz) so quarter-cycle values are equalities and the wrap
   lands on frame 32768 to the bit — the first draft used 2 Hz and its wrap assert was one
   accumulated-ulp coin-flip from failing; an arithmetic gate should not be a coin-flip.
4. **Zero allocations, measured**: both modules across 5 000 `process` calls each, and the
   divided-drum chain across 1 000 rendered blocks with in-loop host-event pushes.

**Measured in sandbox:** **665 tests** (was 655; +10) · fmt clean · clippy clean in the default,
`bootstrap-audio`, `ui`, native `ui-window`/gles and MSVC×6 cells · **ALL pre-existing goldens
unchanged** (`ba577186c988db21`, `0f5c3e86c7f117a9`, `53de3b1f3f40e3c9`, `3f325d4f99ca2a01`,
batch-2's six + chain `9170415cd3852736`, `1621e1f65b1b64e1`, `1d1c84bd37c99b91`,
`f2303f13aa0cf299`) and the stress hash `b42068ec7b206789` unchanged · NEW golden
`914d9063ce9d8a0f` · `selftest --golden` 8/8 · `sparq ui --audit` PASS, 0 failures, 19 smokes
(the browser meets "LFO" and "Clock Divider"; the "exactly one gain" smoke still passes) ·
`sparq modules --strict` **14/14** · exec all three patches (unchanged hashes) · 5 Python gates
+ module docs regenerated (14).

## Files in this zip (12)

Covered by the stamp — 4:

```
crates/sparq-audio/src/modules.rs      (Lfo + ClkDiv + factories; BUILTINS 12 → 14)
modules/mod/lfo/sparqmod.toml          (new — the range/sync decisions are in its header)
modules/mod/clk-div/sparqmod.toml      (new — the exact semantics are in its header)
tools/log_check.py                     (BASELINE moved: 665 tests / src 89f/1782456B)
```

Tests — 1 (not stamp-covered; `gates.bat` runs them and the log's test count depends on it):

```
crates/sparq-audio/tests/modules_batch4.rs   (new — the 10 gates described above)
```

Generated docs — 2 (regenerable; shipped so `module_docs --check` passes on SATURN without
running the generator first; the other twelve are byte-identical):

```
docs/modules/mod-lfo.md   docs/modules/mod-clk-div.md
```

Documents — 5 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   SYNC.md   SYNC-STAMP.txt
```

Heading = list = contents = **12** (4 + 1 + 2 + 5), checked programmatically against the zip's
namelist at packing time.

## What SATURN runs

No new device script — the standing order stacks, with baselines moved (`log_check.py` in the
bundle already expects them):

1. `scripts\test004.bat` — the WO-006 exclusive acceptance (**still the 🔴 blocker**; pass shape
   in `WO006-INC13-RUN-SHEET.md`).
2. `scripts\test006.bat` — the WO-013 window session (step [07]: slider edit changes the
   canvas-render.wav hash).
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 665 passed / 0 failed / 1
   ignored**, stamp `src 89f/1782456B`, `sparq modules --strict` lists **14**.
4. Evidence, wanted: `sparq exec --patch drum-demo` (transport-driven; hash `f2303f13aa0cf299`),
   `--patch mod-demo` (hash `1621e1f65b1b64e1`), and WO-009's two device boxes — a LISTENED
   tempo sweep and the 30-min drift run when the HAL is free.
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero
   xruns).

## Superseded bundles

`sync-wo009-inc1.zip` (26 — applied), `sync-wo014-inc3.zip` (19 — applied),
`sync-wo008-inc4.zip` (38 — applied), `sync-wo014-inc2.zip` (29 — applied),
`sync-wo008-inc3.zip` (15 — applied), `sync-wo006-inc13.zip` (10 — applied),
`sync-wo013-inc3.zip` (19 — applied), `sync-wo013-inc2b.zip`, `sync-p1c.zip`, `sync-p1b.zip`,
`sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`, `sync-wo012-inc1.zip`,
`sync-wo006-inc11g.zip` — each applied on top of the previous, in that order.
