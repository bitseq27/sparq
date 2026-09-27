# CHECKLIST.md — session handoff (live document)

**Purpose:** the first file a new session reads. Says what is done, what is in flight, and what to do
next. Updated at the end of every session (and mid-session when state changes). This file supersedes
`RESUME.md`'s handoff role (RESUME.md is the 2026-09-22 snapshot; keep it as history). For deep
history see `PHASE0-WORKORDERS.md` §2.1 (status table) and its build-log sections — read **by line
range**, never whole (see `RESUME.md` §1 for why).

**Last updated:** 2026-09-26 (WO-014 inc 4 built, sealed, zipped — the fourth increment today.
The user confirmed ALL previous bundles are APPLIED on SATURN: wo013-inc3, wo006-inc13,
wo008-inc3, wo014-inc2, wo008-inc4, wo014-inc3, wo009-inc1. One bundle waits: wo014-inc4.)
wo006-inc13, wo008-inc3, wo014-inc2)

---

## Current position

- **Device state:** the user confirmed 2026-09-26 that **all seven sealed bundles are applied on
  SATURN** (wo013-inc3 → wo006-inc13 → wo008-inc3 → wo014-inc2 → wo008-inc4 → wo014-inc3 →
  wo009-inc1). ONE new bundle waits: **`../sync-wo014-inc4.zip`** (12 entries). The pending
  device runs are unchanged and now cover four increments of baseline: `test004` (exclusive
  acceptance — the #79 fix, still the 🔴 blocker), `test006` (canvas window session),
  `gates.bat` (**expected test count is now 665**, stamp = whatever `SYNC-STAMP.txt` says,
  `sparq modules --strict` lists **14**; `log_check.py`'s BASELINE moves with every seal since
  defect #83), and the WO-008 **loaded soak** (200 modules, 30 min) when the HAL and the machine
  are both free. WO-009's two device boxes: the **30-minute drift measurement** and a **listened
  tempo sweep**. Evidence artefacts: `sparq exec --patch drum-demo` (transport-driven, hash
  `f2303f13aa0cf299`) and `--patch mod-demo` (`1621e1f65b1b64e1`).
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

## 🔴 THE BLOCKER (device track — waiting on SATURN, not on code)

**WO-006 exclusive acceptance = the `test004` re-run** (bundles through `sync-wo008-inc4.zip`
applied). History: `test004` attempt 1 (2026-09-24) passed the caps checkpoint (#77's format fix
verified on hardware) and failed every rung at `Initialize (exclusive): HRESULT 0x88890020` =
`AUDCLNT_E_INVALID_DEVICE_PERIOD` — the open asked the 64-fr block (666.7 µs @ 96 kHz) as the
device period against a 10 ms engine (**defect #79**, logged and fixed in inc 1.3). On the re-run,
the [04] open line should read
`i24-in-32 (converting) · device period 960 fr (10.00 ms) · sparq block 64 fr`; a second
`INVALID_DEVICE_PERIOD` against a 10 ms ask would mean the driver wants a number it is not
reporting — the period probe table in the error is the diagnosis either way. When [04]+[05] pass
and the **2 h zero-xrun soak @ 96 kHz/64 exclusive** finishes clean: WO-006 acceptance closes →
ADR-008 exit fires → **next sandbox increment: delete the cpal bootstrap, HAL becomes `play`'s
default** (that deletion is gated on this device run — do not do it early).

## Next session's task list, in order

**Device track (when SATURN is reachable — every bundle is sealed and waiting):**
1. All bundles through `sync-wo009-inc1.zip` are APPLIED (user-confirmed 2026-09-26); apply
   `sync-wo014-inc4.zip`; run `scripts\test004.bat` (the exclusive acceptance — pass shape in
   `WO006-INC13-RUN-SHEET.md`) and `scripts\test006.bat` (the inc-3 window session; step [07]
   is the mechanical claim: slider edit must change the canvas-render.wav hash).
   `scripts\gates.bat` now expects **665 passed / 0 failed / 1 ignored** and **14** modules,
   with the stamp `log_check.py` in the bundle already knows (defect #83's fix made moving it a
   seal step). Wanted evidence: `sparq exec --patch mod-demo` (`cv wire` line, cutoff > 200 Hz,
   hash `1621e1f65b1b64e1`), `sparq exec --patch drum-demo` (transport-driven; beat triggers on
   the 120-BPM grid, hash `f2303f13aa0cf299`), and — when the HAL is free — the two WO-009
   device boxes: a LISTENED tempo sweep and the 30-min drift run. Wanted back: `test004.log`,
   `test006.log`, `gates.log`, `logs\ui.log`.
2. If test004's 2 h soak passes → **WO-006 acceptance closes** → sandbox increment: ADR-008 exit
   (delete the cpal bootstrap, HAL becomes `play`'s default), then WO-006 increment 2 backlog
   (ASIO, duplex, round-trip measurement, STA retry for the #75 property-store `E_ACCESSDENIED`,
   clock-drift re-derivation per #76).

**Sandbox track (buildable now, in value order):**
3. **Kernel hot-swap primitive** (ADR-009 d3's literal pointer swap, epoch retirement) — the
   allowlisted-`unsafe` increment (`sparq-kernel::sync`, `docs/unsafe-allowlist.md`) that makes
   `Engine` cross-thread and publishes meters/analysis through the lock-free rings (decision 8)
   — which then unblocks the LAST module-side waits: `ana/tap`, `dsp/scope`, and WO-013's live
   wire levels. The only remaining increment with a proofs burden this heavy; everything after it
   is modules and UI.
4. **WO-013 increment 4** — live wire levels **only with** the executor taps (do not fake them;
   the per-port readers `node_cv_block`/`node_cv_audio`/`node_events`/`meter` exist now); else
   rename text entry (shared with the browser's provisional keyboard feed), inspector scrolling,
   LOD visual iteration vs `design-mode.svg`. Canvas cv wires: the connect layer already
   delegates cv verdicts to `connect_cv`; drawing the wire class is the increment's UI half.
5. **WO-014 increment 5 — the last three modules** when their contracts land: `ana/tap` +
   `dsp/scope` (after the rings), `out/main` (with the canvas master handover). Plus the LFO
   beat-rate decision (per-frame tick view vs module-side bpm derivation — LATER.md) which also
   unlocks the delay's `set_tempo_sync` path.
6. Studio-session evidence still open: real-finger canvas touch-test, DPI matrix walk
   (WO-012/WO-013), WO-001 hardware sheets, dispatch-bench C/D numbers for ADR-009, and the
   WO-008 **loaded soak** (200 modules, 30 min, zero xruns — the WO's hardware acceptance).
7. Small, well-specified, declared in LATER.md: mixer's cv merge side (retires the last of the
   cv-fan-in refusal); `cv_interp = "spline"` (refused at build until implemented); host-side
   `required`-unconnected enforcement (moves stress refusal counters — its own increment on
   purpose); the compat-matrix review packet (`reviewed = false`, defect #82's ordering question
   inside it); loop-relative beat phase for unaligned loop regions (WO-009's declared limit).

## Done (most recent first)

- [x] **WO-014 inc 4** (2026-09-26, this session) — `mod/lfo` + `mod/clk-div` (14 modules),
      lfo→svf bit-identical sweep, 16ths→÷4→membrane chain golden `914d9063ce9d8a0f`, seeded
      probability reproducibility, binary-exact-rate arithmetic gates. 665 tests, all goldens
      unchanged. Sealed `sync wo014-inc4`.
- [x] **WO-009 inc 1** (2026-09-26, this session) — piecewise kernel Clock (ramps + exact
      bisection), `sparq-music` (broker + transport + zero-alloc queue), the executor's musical
      door, transport-driven drum-demo == the batch-3 golden, ±0-sample acceptance vs an
      independent integral, ADR-006 addendum. 655 tests. Sealed `sync wo009-inc1`.
- [x] **WO-014 inc 3** (2026-09-26, this session) — membrane + env/ad + mixer (12 modules),
      drum-demo golden with kicks on exact frames, defect #84 (TOPS table-first), 627 tests,
      goldens unchanged. Sealed `sync wo014-inc3`.
- [x] **WO-008 inc 4 — CONTRACT V1** (2026-09-26, this session) — multi-port `AudioCtx`,
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

- **`.git` is NOT durable in this workspace** (observed three times now: 2026-09-25 twice,
  2026-09-26 the clone arrived without history and was re-inited). Do not rely on git for state;
  the integrity tool is `sync_check.py` (content hashes) and the seal is the zip. Re-init for
  in-session diffs if useful, expect it gone next session.
- Toolchain wipes **between and mid-sessions** (again 2026-09-26: `/opt` arrived empty — rustup,
  cargo, gcc and the apt packages all reinstalled from scratch in ~3 min with warm apt). Lives in
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
  `CARGO_PROFILE_DEV_DEBUG=0` (proven again this session, 2 m 11 s clean). `ui --audit` +
  goldens: run release with `CARGO_PROFILE_RELEASE_LTO=false CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`
  — SATURN builds with the repo profile. The MSVC×ui-window cell does not fit even single-job
  (`windows` crate OOM).
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

- **#79 — LOGGED + fixed in sandbox (inc 1.3), device-verified: pending** (test004 re-run).
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
- WO-008 declared limits (parked in `LATER.md` §WO-008): single-owner Engine until the kernel
  hot-swap primitive lands; compensation aligns PLAIN audio fan-in arms only; `required`-
  unconnected inputs not host-enforced (deliberate — the stress baseline moves with that change);
  `spline` interp refused until implemented; watchdog→UI red hairline is a WO-013-side painter
  increment; meters not yet ring-published cross-thread
- MSVC × `ui-window` clippy cell: never run anywhere (sandbox OOM, no CI cell)
- Device-side baselines that MOVE with this increment: gates.log test count **665** (log_check
  already moved — #83's discipline), stamp = whatever `SYNC-STAMP.txt` says, `sparq modules`
  count **14**, exec has three patch names (`demo`, `mod-demo`, `drum-demo` — the last
  transport-driven)
