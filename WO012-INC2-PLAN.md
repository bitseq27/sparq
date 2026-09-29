# WO-012 increment 2 — the live audio session: HAL stream through `SharedEngine` (plan of record, 2026-09-29)

The operator's session-6-close pivot, ratified in the CHECKLIST header: *"start fleshing out the
interface and start building patches and sounds … focus on the user experience now."* Decision (1)
re-sequenced live canvas audio ahead of WO-006's exclusive acceptance — the interim vehicle is
`wasapi-shared` at its negotiated ~22.67 ms, the most device-proven path in the project (2 h soak /
0 xruns / unplug→`Removed` / recovery, all on SATURN). Decision (2) named this increment: **live
audio + continuous meters** — checklist sandbox item 6. The ADR-008 exit stays gated on test004
attempt 4; the bootstrap is NOT deleted here. This plan settles every open question item 6 listed,
in the `WO008-INC7-PLAN.md` discipline: decisions before code, each with its reason.

## The decisions

**D1 — Name, home, and the riding along.** This is WO-012 increment 2: the shell's first engine
binding — the PLAY/STOP/PANIC stubs (`"engine binding: WO-007/008"`) become real. New file
`crates/sparq-app/src/ui/live.rs` holds `LiveSession`; `ShellUi` owns `Option<LiveSession>`.
The supporting engine additions — **ring port ids and per-port audio meters** — ride this increment
as LATER.md §WO-008 declared ("per-port meters + the ring extension ride the
live-HAL-through-`SharedEngine` increment"). The scope screen (drawing `dsp/scope` from the
analysis ring) is the named FOLLOW-ON and stays out.

**D2 — Transport placement and touch shape: no new chrome.** The existing PLAY/STOP/PANIC actions
(top rail in Design, the cluster in Perform — the same `Action` enum, already audited at 44 px)
bind to the session. PLAY starts, STOP stops, PANIC stops immediately — a stopped stream IS "all
sound off"; no separate mute state is invented. While running, PLAY carries the `pal.playing`
accent in both modes (the Perform cluster's existing accent language), and every state change logs
a line — the shell explains itself. Musical transport (position UI, arrangement, loop) stays
WO-009's; `SetMusical` exists on the command ring and this increment neither binds nor fakes it.

**D3 — The honesty lines, in the house shape.** Open: the backend's `display_name`, the NEGOTIATED
rate · ch · block (the two-phase probe open, reused from `play/hal.rs` — the graph is built for the
negotiated truth, never the request), and `latency_report().summary()`. Stop: blocks rendered,
boundary swaps, xruns, audio-thread allocations (the diag measured them; the claim is only true if
something counted), commands applied/refused, meter/analysis refusals — every number read, none
invented. `Removed`/`Failed`: ONE line naming the state and the stream's `last_error`; the session
ends and the canvas is untouched (every audio-side fallback stays "keep rendering the live patch").

**D4 — Backend choice: null manual for the audit, `wasapi-shared` on the device.** Headless and
`ui --audit` run the **null backend in MANUAL mode** (`paced = false`): the smoke pumps blocks
itself through `AudioStream::pump` — deterministic, hermetic, no wall-clock timing on a loaded CI
runner, and `capture_frames` proves real samples reached the device buffer. Window mode uses
`wasapi-shared` when the `hal-wasapi` feature is compiled in on Windows; otherwise null PACED with
an honest line ("no device backend in this build — running against the null device"). The null
request is 48 kHz · 64 · 2-out; `capture_frames` stays 0 in the shell (bounded in smokes only).

**D5 — The executor is built for the negotiated config.** NEW `bridge::build_with_map_at(graph,
master, registry, cfg)` where `cfg` carries rate/block/channels; `build_with_map` delegates with
the `RENDER_*` constants — signature unchanged, every existing caller, test and golden untouched
(the increment-5 `compute_at` discipline: the new door takes the parameter, the old door keeps its
behaviour bit for bit). `LiveSession` builds through the new door at the probe's `actual_config`.

**D6 — Audio-thread ownership and teardown.** The `AudioEngine` MOVES into the `ProcessFn`
closure; the callback does `render_block(fctx.out)` and nothing else — no clock, no allocation, no
blocking (the pump measures all of it, `audit_allocations: true`). On STOP: `stream.stop()` joins
the pump; the session then drops the stream **on the control thread** — the drop IS the teardown
point `AudioEngine::shutdown`'s docs demand ("not inside a device callback") — drains
`reclaim()` to `None` (retirement hygiene, exactly once), and logs the evidence line.

**D7 — ONE op→sync door, and it classifies by op kind.** `CanvasState` grows the change ledger the
door reads: every op-application site (the `history.push` family, `apply_param`'s coalescing
replace, undo, redo) marks a `PatchChanges` value — `structural: bool` + the set of param-edited
node ids — classified in ONE function from the `Op` itself: `AddNode`/`RemoveNode`/`AddWire`/
`RemoveWire`/`SetFlags` (which carries bypass/mute/lock AND the master flag) → structural;
`SetParam` → param-dirty(node); `MoveNode`/`Rename` → audio-invisible, mark nothing. The shell
takes the ledger once per frame while a session runs: param-dirty nodes → `set_params(mapped
kernel node, node.effective_params())` over the command ring (**no re-stage** — a re-stage per
drag frame would reset module state and click; the ring exists for exactly this); structural →
rebuild through `build_with_map_at` → `stage(executor, master)` (the boundary swap inherits the
transport clock). Undo/redo fall out of the same door for free — the checklist's prediction,
confirmed by the code: ops apply to the model first, the door reads what they were.

**D8 — A refused stage is visible and retried, never silent.** `stage` returns false when the
hot-swap slot still holds an unconsumed patch (a control side faster than the audio side). The
session keeps a pending flag and retries rebuild+stage EVERY FRAME until accepted (control-thread
work; canvas rigs are small), logging the refusal line and the eventual swap line. Canvas-vs-audio
divergence is always in the log, never hidden.

**D9 — Ring port ids: `MeterUpdate` grows `port: u32`; publication goes per-port.** One entry per
audio OUTPUT port (NEW additive executor accessor `with_audio_out(node, f)`, the mirror of
`with_audio_rate_cv_out`; the folded `meter()` API does not move). A node with NO audio outputs
publishes ONE folded entry at `port = MeterUpdate::FOLDED` (`u32::MAX`) — a cv-only node's status
must not vanish from the ring. Every entry carries the node's status byte. Single-output nodes
publish the same numbers they publish today — pinned by test, because "additive" is a claim, not
a hope.

**D10 — Live cv-port levels come from the analysis ring, by the inc-5 rule.** `publish_analysis`
extends to BLOCK-rate cv outputs as `len = 1` waveforms (audio-rate publication unchanged); the
session drain computes peak |wave| per `(node, port)` → `NodeLevels::set_port` — the SAME language
as the offline path (magnitude, clamped 0..1), so a cv wire lights identically whether the level
came from a preview render or the live ring. Never faked, in both directions. The ring-traffic
growth is bounded and counted (`analysis_refusals`); a dropped waveform is a dropped frame of a
display, which is what a real-time display is allowed to do.

**D11 — The per-frame drain is bounded, batched, and last-wins.** Once per `frame()` while running:
drain `read_meters` in fixed scratch batches (`[MeterUpdate; 64]`, a hard budget of 4096 updates
per frame) and `read_analysis` likewise (`[AnalysisUpdate; 16]` — those payloads are big); last
write wins per `(node, port)` into session-owned scratch (no fresh Vecs per frame — control-side
allocation is legal but sloppiness is not), then map kernel→canvas through the stored map and
OVERWRITE `CanvasState.levels`: per-port entries from port meters, the folded node level = max of
its port peaks (identical to today's folded semantics for the single-output case), cv ports from
the analysis drain. The painter path is DONE (inc 4/5) — this swaps the SOURCE, not the drawing.
After STOP the levels hold their last drained values: transient view state, like `levels` always
was.

**D12 — RENDER WAV does not move.** The offline preview path stays exactly as shipped (a separate
concern with its own evidence line). While a session runs, the next frame's live drain simply
overwrites the offline refresh — declared, not special-cased; two writers to one transient field
resolve by cadence, and the live one is the truth while it runs.

**D13 — The audit smokes: manual null, pumped by the smoke, asserted on STATE.** (a) PLAY → pump →
the sine→gain wire level is REAL (peak > 0 from the meter ring) and the capture holds non-zero
samples — the callback really rendered the canvas patch; (b) a param edit while live → the command
ring crosses (cmd_applied moves) and the level FOLLOWS the signal (amp down → level down — inc 4's
discipline, live); (c) a structural edit while live → the swap is counted and the capture changes;
(d) `inject_fault(Unplug)` → `Removed`: one honest line, the session ends, the canvas is untouched;
(e) STOP → the evidence line carries the measured numbers. Log lines are never the assertion — the
state is. The smoke count moves 25 → 25+N at seal.

**D14 — What must NOT move.** Every audio golden (no render path moves — `render_wav`,
`node_levels`, the bridge's offline doors are untouched); stress `7bb06379bd6845e5` · 2 383 swaps ·
7 617 refused (executor semantics unchanged); `canvas-render.wav` 1 920 046 B · sha256
`d7ad294e…`; determinism `0f5c3e86c7f117a9`; selftest 9/9; `modules --strict` 17/17; the existing
25 audit smokes; the bootstrap (ADR-008 gated — do not delete early); test004's device contract.
The test-count baseline (762) moves with the new gates, and `tools/log_check.py`'s BASELINE moves
WITH the seal (defect #83's discipline, twice-forgotten, never again). `cross_thread.rs`'s meter
test gains the port field additively — its ledger counts (blocks · staged = swaps = reclaimed) do
not move.

**D15 — The device side, at seal time.** test006 gains the live steps: PLAY on `wasapi-shared` →
the demo patch is HEARD at the negotiated latency → wires animate while it plays → a slider edit
changes the sound with no stop/start → STOP prints the evidence line; optionally unplug mid-play →
the `Removed` line and an untouched canvas. The run-sheet amendment ships in the bundle; the
exclusive-acceptance and loaded-soak boxes stay device-track, unchanged.

## Implementation order (one increment, five commits' worth)

1. **Engine + executor** — `MeterUpdate::port` + `FOLDED`; `Executor::with_audio_out`; per-port
   `publish_meters`; block-rate cv in `publish_analysis`; unit tests; `cross_thread.rs` updated
   additively.
2. **Bridge** — `build_with_map_at` (+ its config type); `build_with_map` delegates; a test that
   the delegation is bit-identical to today's behaviour.
3. **`LiveSession`** (`ui/live.rs`) — open/start/stop/panic, the D7 door, the D11 drain, the D3
   lines; integration tests on the manual null (pump, capture, faults).
4. **Shell binding** — `Action::Play/Stop/Panic` → the session; the per-frame take+drain inside
   `frame()`; the playing accent; `PatchChanges` in the canvas core with its own unit tests.
5. **Audit smokes + seal** — the D13 smokes, then the full gate run, LATER.md moves, SYNC.md /
   CHECKLIST.md / PHASE0 build log, the run-sheet steps, `sync_check --write`, log_check BASELINE,
   the zip.

## Postscript — D8 corrected by the code (2026-09-29, during the build)

D8 assumed `stage` could refuse, and prescribed a per-frame retry. The hot-swap's actual contract
(read at implementation time, `sparq-kernel/src/sync/hotswap.rs`): `stage` ALWAYS lands — it
superswaps the waiting payload, drops the superseded box on the control thread, and counts it
("the timeline never skips — what goes live next is always exactly one boundary away"). The
return value means SUPERSEDED, not refused. D8 is therefore replaced, not implemented:

**D8′ — there is no stage refusal to retry.** A supersede is counted and SAID in the re-stage log
line. What can refuse is the BUILD (`build_with_map_at` — e.g. increment 7's bare-required-input
enforcement on a half-drawn patch): the live patch keeps playing, the executor's own sentence is
logged with the remedy, and the next structural change tries again. The `pending_stage` retry
flag does not exist. The first live.rs test run proved the refusal path with a real sentence
("edge 2 attaches to port 'in' from the wrong side") before the test's own wire direction was
fixed — the honest path fired before the happy one was ever asserted.

## Postscript — the acceptance, measured (2026-09-29, after the run)

Every box the plan named, on the final tree: `cargo fmt --all --check` clean · clippy clean in
the default, `ui`, `bootstrap-audio`, combined audio+hal and native `ui-window` (single-job)
cells and all six runnable MSVC cells · **766 tests, 0 failed, 1 ignored** (the +4: the
per-port ring gate, the block-rate analysis gate, the ledger gate, the config-door delegation
pin; plus 5 session tests and 5 smokes behind `ui`) · the 5 python gates clean · release goldens
bit-identical (the wo005 + phase-b manifests; the three pinned exec renders
`1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9` re-rendered to the bit) ·
`selftest --golden` **PASS (9 gates)** — golden `ba577186c988db21`, determinism
`0f5c3e86c7f117a9`, throughput 262.8× realtime, reopen-leak 0 · `sparq modules --strict`
**17/17** · `ui --audit` **PASS, 30 smokes** (25 + the five D13 ones) · `canvas-render.wav`
1 920 046 B · sha256 `d7ad294e…` UNCHANGED · stress **`7bb06379bd6845e5` · 2 383 swaps ·
7 617 refused · 0 allocations, debug AND release** · the cross-thread ledger holds (2 299
staged = swaps = reclaimed) · `probe_alloc` 0 across 5 000 blocks · stamp `src 93f/2081612B`,
`log_check.py` BASELINE moved with the seal and its self-test 25/25, `sync_check` 152 files →
`--self-test` 11/11 failable · sealed **`sync-wo012-inc2.zip` (20 entries — heading = list =
contents, checked programmatically)**. One declared deviation from the plan text: D13's smoke
(a) asserts the capture peak and the LEVELS; the "wires animate" claim itself is the painter
path inc 4 already proved — the smokes prove the SOURCE swap, the device run (test006 J–K)
proves the animation. What the sandbox cannot prove, and says so: device sound (steps J–K),
`wasapi-shared` negotiation on real hardware, and the unplug with a real cable.

## Acceptance (sandbox)

`cargo fmt --all --check` · every runnable clippy cell · `cargo test --workspace` green at the new
count (762 + the new gates) · the 5 python gates · release goldens bit-identical ·
`selftest --golden` 9/9 · `sparq modules --strict` 17/17 · `ui --audit` PASS at 25+N smokes ·
the stress evidence line reads `7bb06379bd6845e5` · 2 383 · 7 617 in debug AND release.
