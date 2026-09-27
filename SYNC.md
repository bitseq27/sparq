# Sync manifest — WO-014 increment 5 (the module set completes at seventeen) + WO-013 increment 4 (live wire levels)

**Current bundle: `sync-wo014-inc5+wo013-inc4.zip` (29 entries, listed below).** Applies on top of
**`sync-wo008-inc5.zip`** — which is itself still waiting, along with **`sync-wo014-inc4.zip`**.
Apply order: **wo014-inc4 → wo008-inc5 → this one**. The user confirmed 2026-09-26 that
everything through wo009-inc1 is APPLIED on SATURN (wo013-inc3 → wo006-inc13 → wo008-inc3 →
wo014-inc2 → wo008-inc4 → wo014-inc3 → wo009-inc1). Extract at the repo root
`Q:\morphosis\code\sparq`, overwriting. No drift-repair copy step this time: the pristine clone
now passes `sync_check --quiet` against its own committed stamp (defect #86 was repaired in the
wo008-inc5 bundle and the pushed tree is consistent).

**The stamp changes: `src 91f/1889639B`** (`SYNC-STAMP.txt` regenerated for the whole **148-file**
covered set — one new `src` file, `crates/sparq-ui/src/canvas/levels.rs`, and three new
`modules/**/sparqmod.toml` manifests, over the 144 of wo008-inc5).

## What this is

TWO increments from one 2026-09-27 session, sealed together because they ship together.

**WO-014 increment 5 — the last three modules; the first-party set is complete at SEVENTEEN.**
The build-log entry in `PHASE0-WORKORDERS.md` (search **WO-014 increment 5**) carries the full
story. Headline:

1. **`ana/tap`** — the generic signal tap for displays: audio-rate bipolar `wave` (the mono monitor
   mix) plus block-rate unipolar `peak`/`rms`. Its wave golden `3f325d4f99ca2a01` CROSS-VALIDATES
   against the `syn/sine` 1 s golden — the mono mix of a mono-fanned sine is the sine itself, bit
   for bit, so the tap colours nothing. `gain` scales the WAVE only; peak/rms report the TRUE
   signal (a tap used for modulation stays honest about level).
2. **`dsp/scope`** — the first real visual module, and the set's only module whose `process` is a
   deliberate NO-OP: its zero-audio-thread-cost acceptance is architecture, not optimisation. The
   proof is a patch-level golden — adding a tap+scope to a render leaves the master output
   BIT-IDENTICAL, so the scope-rig golden EQUALS the out/main golden `75bc7f2f18cac9d5`. Two
   audio-rate bipolar cv inputs (`x`, `y`) are the display BINDING; the UI resolves each to its
   source tap and reads the published waveform.
3. **`out/main`** — the master output with the metering hook: a unity-BIT-EXACT pass-through with
   trim + a hard mute that writes exact zeros. Because the executor meters every node, its peak/rms
   ARE the master meters. It has an audio OUTPUT because the executor renders the master node's
   first audio output — a pure sink would render silence. Ships with the WO-013-side master-handover
   rule (below) so the MASTER badge names the output module rather than guessing the last gain.
4. **The analysis-payload CONTRACT they waited on** — `sparq-audio::engine::AnalysisUpdate` extends
   ADR-009 d8's publication from meters (peak/rms) to WAVEFORMS. `AudioEngine::publish_analysis`
   pushes every audio-rate `cv` output onto a bounded `SpscRing` once per block;
   `SharedEngine::read_analysis` drains it control-side. `Executor::with_audio_rate_cv_out` is the
   allocation-free hook. `tests/analysis_pub.rs`: the waveform crosses to the control side, an
   absent reader is counted-not-queued, and the audio thread allocates NOTHING while publishing.
5. **The LFO beat-rate DECISION** (the WO asked for it before building): **module-side bpm
   derivation, NOT a per-frame tick view** — recorded with its reasoning in LATER.md, **and shipped
   the same session**. A contract change to buy sample-accurate tempo almost no module needs, versus
   reading the block-start `tick`/`ppqn` the executor already carries (one block = 1.3 ms of lag on
   a tempo EDIT). `mod/lfo` 0.2.0 (`sync-mode` + `division`) and `util/delay` 0.2.0 (`tempo-sync` +
   `division`) derive bpm through a shared `TempoFollower` and the delay now reaches
   `DelayLine::set_tempo_sync`; both fall back to their free-running parameter with no transport, and
   both are additive (defaults byte-identical, goldens unchanged). `tests/tempo_sync.rs` (7 gates,
   golden `db4013f41d1fa678`): the tempo-synced delay renders BIT-IDENTICAL to a hand-timed 250 ms.

**WO-013 increment 4 — live wire levels, the "signature sparq image".** The park note said wires
drew at rest until the executor taps existed; they exist now (WO-008: `Executor::meter`, and the
cross-thread `SharedEngine::read_meters`), so this ships the painter half — and does NOT fake it:

1. **`sparq-ui::canvas::levels`** (toolkit-independent, computed not drawn): `NodeLevels` (per-node
   0..1, clamped, NaN-to-rest, deterministic `BTreeMap`) + `wire_level` (a wire carries its SOURCE
   node's level). Unit-tested.
2. **`bridge::node_levels`** — renders a short preview and reads each node's PEAK meter, mapping
   kernel nodes back to canvas nodes. The ONLY source of a wire level: the executor's own meter,
   never a value invented from canvas data. Two bridge tests prove the levels are real AND follow
   the signal (turn the gain down, the level falls).
3. **The painter** (`canvas_ui.rs`) modulates each wire from its class colour toward its class glow
   by level, plus a soft under-glow — both endpoint colours are tokens, the blend invents nothing.
   `CanvasState.levels` is a transient (non-undoable) field the shell refreshes from the bridge
   after RENDER WAV; empty until the first render, so wires draw exactly as before until then.
4. **Audit smoke 20** (`ui --audit` now 20 smokes): RENDER WAV populates live wire levels from the
   executor's meters, hot where signal flowed.
5. **Master handover** (`interact::resolve_master`): a wired `out/main` node supersedes the default
   highest-id-terminus rule (an explicit SET MASTER still wins; an unwired out/main does not count).
   Three new tests. `OUT_MAIN_ID` is a named constant so the rule and the id cannot drift.

**DECLARED LIMIT (not faked):** a node's meter folds its AUDIO outputs, so a `cv` wire out of a
cv-only source (`mod/lfo`, `env/ad`, `ana/rms`) reads at rest — the level animation tracks AUDIO
signal flow. Per-port / per-cv meters (the rings can carry port ids) are the LATER.md item that
would light cv wires from their own values. **NOT in this increment** (the task list's "Else"
column, declared for the next pass): rename text entry, inspector scrolling, LOD visual iteration
against `design-mode.svg`.

**Measured in sandbox:** **713 tests** (was 682; +31 — batch-5's 13, analysis_pub's 3, tempo_sync's
7, the master-handover 3, the levels 3, the bridge 2) · fmt clean · clippy clean in the default,
`bootstrap-audio`, `ui`, native `ui-window`/gles (check) and MSVC×(audio,ui) cells · ALL
pre-existing goldens unchanged and re-verified (`ba577186c988db21`, `0f5c3e86c7f117a9`,
`53de3b1f3f40e3c9`, `3f325d4f99ca2a01`, `1621e1f65b1b64e1`, `f2303f13aa0cf299`, `914d9063ce9d8a0f`,
batch-2's six + chain `9170415cd3852736`) · stress hash `b42068ec7b206789` unchanged (debug AND
release) · NEW goldens `75bc7f2f18cac9d5` (out/main master, and the scope rig), `3f325d4f99ca2a01`
(tap wave == sine) and `db4013f41d1fa678` (beat-locked lfo at 120 bpm) · `selftest --golden`
**9/9** · `ui --audit` PASS **20 smokes** · `modules --strict` **17/17** · 5 Python gates +
`sync_check --self-test` 11/11 + `log_check --self-test` 25/25 · stamp `--write` then `--quiet`
clean. NOTE: `mod/lfo` and `util/delay` are now **v0.2.0** (5 and 6 params) — additive; at their
defaults they render byte-identical to v0.1.0, which the unchanged batch-2/batch-4 goldens prove.

## Files in this zip (29)

Covered by the stamp — 16:

```
crates/sparq-audio/src/modules.rs            (ana/tap + dsp/scope + out/main; BUILTINS 14 -> 17; TempoFollower; lfo/delay beat-lock; header)
crates/sparq-audio/src/engine.rs             (AnalysisUpdate + the analysis ring + publish_analysis)
crates/sparq-audio/src/executor.rs           (with_audio_rate_cv_out — the publication hook)
crates/sparq-ui/src/canvas/mod.rs            (OUT_MAIN_ID const; levels module + re-exports)
crates/sparq-ui/src/canvas/levels.rs         (NEW — the live-wire-level model, toolkit-independent)
crates/sparq-ui/src/canvas/interact.rs       (master handover to out/main; CanvasState.levels; tests)
crates/sparq-app/src/bridge.rs               (build_with_map + node_levels — meters -> wire levels)
crates/sparq-app/src/ui/canvas_ui.rs         (draw_wires level modulation + the two colour helpers)
crates/sparq-app/src/ui/shell_ui.rs          (RENDER WAV refreshes canvas.levels from the bridge)
crates/sparq-app/src/ui/headless.rs          (audit smoke 20 — levels populate from real meters)
modules/ana/tap/sparqmod.toml                (NEW)
modules/dsp/scope/sparqmod.toml              (NEW)
modules/out/main/sparqmod.toml               (NEW)
modules/mod/lfo/sparqmod.toml                (0.2.0 — sync-mode + division params)
modules/util/delay/sparqmod.toml             (0.2.0 — tempo-sync + division params)
tools/log_check.py                           (BASELINE moved: 713 tests / src 91f/…; fixture stamp)
```

Tests — 3 (not stamp-covered; `gates.bat` runs them and the log's test count depends on them):

```
crates/sparq-audio/tests/modules_batch5.rs   (NEW — the 13 batch-5 gates incl. the two goldens)
crates/sparq-audio/tests/analysis_pub.rs     (NEW — the analysis-ring publication, 3 gates)
crates/sparq-audio/tests/tempo_sync.rs       (NEW — the beat-locked lfo + tempo-synced delay, 7 gates)
```

Generated docs — 5 (not stamp-covered; `module_docs.py --check` gates them):

```
docs/modules/ana-tap.md   docs/modules/dsp-scope.md   docs/modules/out-main.md
docs/modules/mod-lfo.md   docs/modules/util-delay.md   (the two 0.2.0 regenerations)
```

Documents — 5 (excluded from the stamp on purpose):

```
CHECKLIST.md   LATER.md   PHASE0-WORKORDERS.md   SYNC.md   SYNC-STAMP.txt
```

Heading = list = contents = **29** (16 + 3 + 5 + 5), checked against the zip's namelist below.

## What SATURN runs

No new device script — the standing order stacks, with baselines moved (`log_check.py` in the
bundle already expects them):

1. `scripts\test004.bat` — the WO-006 exclusive acceptance (**still the 🔴 blocker**; pass shape in
   `WO006-INC13-RUN-SHEET.md`).
2. `scripts\test006.bat` — the WO-013 window session (step [07]: slider edit changes the
   canvas-render.wav hash). **New this increment:** after RENDER WAV, the wires of the sine/gain
   chain should light with live levels (the signature image) — eyeball evidence wanted.
3. `scripts\gates.bat` — full table. **Expected in `gates.log`: 713 passed / 0 failed / 1 ignored**,
   stamp `src 91f/1889639B`, `sparq modules --strict` lists **17**, the selftest line reads
   **PASS (9 gates)**, and `ui --audit` reads **PASS (20 smokes)** (smoke 20 = live wire levels).
4. Evidence, **with the pinned durations** (defect #85): `sparq exec --patch mod-demo --seconds 1`
   (`1621e1f65b1b64e1`), `sparq exec --patch drum-demo --seconds 2` (`f2303f13aa0cf299`),
   `sparq exec --patch demo --seconds 2.8` (`53de3b1f3f40e3c9`), and — when the HAL is free — the
   two WO-009 device boxes (a LISTENED tempo sweep and the 30-min drift run).
5. Still wanted: dispatch-bench `C. VERDICT` + both `D. PER-CALL COST` lines (ADR-009),
   `logs\ui.log`, the DPI matrix, and the WO-008 **loaded soak** (200 modules, 30 min, zero xruns).

## Superseded bundles

`sync-wo008-inc5.zip` (22 — WAITING), `sync-wo014-inc4.zip` (12 — WAITING: apply both before this
one), then the applied chain: `sync-wo009-inc1.zip` (26), `sync-wo014-inc3.zip` (19),
`sync-wo008-inc4.zip` (38), `sync-wo014-inc2.zip` (29), `sync-wo008-inc3.zip` (15),
`sync-wo006-inc13.zip` (10), `sync-wo013-inc3.zip` (19), `sync-wo013-inc2b.zip`, `sync-p1c.zip`,
`sync-p1b.zip`, `sync-p1-fixes.zip`, `sync-wo007-complete.zip`, `sync-wo007-task1.zip`,
`sync-wo012-inc1.zip`, `sync-wo006-inc11g.zip` — each applied on top of the previous, in that order.
