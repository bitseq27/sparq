# WO-008 increment 4 — multi-port `AudioCtx` (contract v1): the plan

**Date:** 2026-09-26 · **Track:** sandbox (CHECKLIST task 3 — "THE unblocking increment") ·
**Seal target:** `sync wo008-inc4` → `../sync-wo008-inc4.zip`

## What ships

1. **Contract v1 `AudioCtx`** (`sparq-module-api::module`): multi-port audio (≤ `MAX_PORTS = 8`
   per type per direction, validation-enforced like `MAX_PARAMS`), `cv` ports (block-rate value
   or audio-rate slice; `CvIn`/`CvOut`), `event` ports (pre-sorted `&[Event]` in, bounded
   `EventSink` out). New `sparq-module-api::event`: `Event { kind, sample, channel, value,
   words[4] }`, `EventKind { Ump, Osc, Trigger, Gate, Note, Clock }`, capacity
   `EVENTS_PER_BLOCK = 64` per port, overflow **counted, never grown**. v0-shaped conveniences
   stay as methods: `input()`, `output()`, `has_input()`, `frames()`, `param(i)` — modules read
   their own manifest's port order; per-type indices count only ports of that type.
2. **`Resources` v1**: per-port audio channel arrays + counts (legacy `input_channels` /
   `output_channels` stay as the first-port view). Still `Copy`.
3. **Manifest**: `event_kinds` becomes modelled data (`PortSpec` → `Port`), required on event
   ports (`E-KEY-MISSING`), closed domain (`E-ENUM-UNKNOWN:…`), and the port cap is a
   cross-field rule (`E-CROSS-FIELD:port-cap`).
4. **Executor v1** (`sparq-audio::executor`): cv/event edges **execute**; the per-node port plan
   is computed from the validated manifests. Rules, each with a refusal in words where it refuses:
   - `cv`: verdict through `port.rs::connect_cv` at `Phase::Zero` (range mismatch ⇒ refused,
     remedy names `util/range`, Phase 1, not shipped — never offer a converter that does not
     exist); fan-in ⇒ refused naming `util/mixer`; fan-out free; audio-rate source → block-rate
     input reduced by the **receiver's declared `cv_reduce`** (last/first/mean/min/max/peak);
     block-rate source → audio-rate input expanded by the receiver's `cv_interp`
     (hold/linear; **spline refused at build** — declared vocabulary, not yet implemented);
     `linear` ramps `prev → current` reaching `current` at the next block boundary (formula
     documented; `prev` is executor storage refreshed at block end).
   - `event`: producer `event_kinds` ⊆ consumer's (else refused, `E-EVENTKIND-UNACCEPTED`'s
     sentence); fan-in free, merged **pre-sorted by sample offset**, ties stable by
     (host-injection rank, then EdgeId, then insertion order) — G5's tie-break made mechanical;
     per-sink stable insertion sort + k-way linear merge (no allocation, no unstable sort);
     control-side `push_host_event(node, port, ev)` bounded queue (WO-009's transport will feed
     the same door; this increment does NOT pre-empt its clock work);
     delay edges on cv/event refused in words (Plain only).
   - `data`/`gpu`/`atom` edges: still refused, now with per-type sentences (data → the schema /
     staleness machinery; gpu → never on the audio thread, ring publication; atom →
     control-thread, `Module::message`).
   - Per-port buffers everywhere; meters aggregate over a node's audio outputs; auto-bypass
     passes through per matching audio port, zeroes cv cells, leaves event sinks empty;
     `node_output` stays (first audio out), new `node_audio_out`, `node_cv`, `node_events`,
     `event_drops`.
5. **Modules**: all nine migrate to the v1 accessors; **`ana/rms` writes its declared cv port**
   (the output[0] convention is deleted — the manifest stops lying); **`flt/svf` grows the
   modulation input** (`cutoff-mod`, block-rate unipolar cv, optional) + param 3 `mod`
   (0..1, default 0): `cutoff_eff = clamp(cutoff · 2^(2·mod·cv), 10, 20 000)` — at defaults the
   filter is bit-identical to inc 2, which the goldens pin. Manifest 0.1.0 → 0.2.0 (additive).
6. **The WO-014 acceptance item — the rms→filter modulation demo**: `sparq exec --patch
   mod-demo` (sine → svf master; sine → gain → rms → svf cv) renders a WAV and prints the tap;
   plus the **arithmetic acceptance test**: an svf driven through the cv wire renders
   **bit-identical** to the same svf whose cutoff parameter is set by hand, per block, to the
   value the contract says it must receive — the cv path proven exact, not approximately right.
   Golden hashes checked in.
7. **compat-matrix coordination** (the debt carried since WO-007): a new drift gate
   (`sparq-module-api/tests/compat_matrix.rs`) parses `docs/api/compat-matrix.toml` with the
   crate's own parser and pins vocabulary, verdicts, adapters and case counts against
   `port.rs` — the mirror can no longer drift silently. Full deletion is **escalated, not
   done**: several `when` cells are prose ("fan-out: one cv output → many cv inputs"), so
   "read at discovery" needs the ratified artefact restructured first — recorded in the build
   log and the debt line re-worded. The executor's new cv verdicts go through `connect_cv`
   (one compiled copy; the UI already does).
8. **Gates that exist to catch this edit fire and are updated deliberately**: `api_snapshot.rs`
   (new pins incl. `MAX_PORTS`, `Event`, accessors; typed call sites moved to the constructors),
   `throwaway_module.rs` (the walk still proves zero engine edits), contract.rs's three
   reference modules migrate. Docs: author guide v1 section, executor/module.rs doc headers,
   ADR-005 addendum one-liner (the matrix now has two consumers in code + a drift gate),
   module-api-v1.md status note, `module_docs.py` regeneration for svf, LATER.md,
   PHASE0-WORKORDERS (WO-008 + WO-014 rows, build-log entry), CHECKLIST, SYNC.md, stamp, zip.

## Red lines (must not move)

- **Goldens unchanged**: `ba577186c988db21` (selftest) · `0f5c3e86c7f117a9` (determinism) ·
  `d46736fd9a1c48a1` (phase-b) · `53de3b1f3f40e3c9` (exec demo) · `3f325d4f99ca2a01` (sine) ·
  the six batch-2 module goldens + chain `9170415cd3852736`. rms leaves the audio path in the
  demo/stress worlds (it was already cv-only in the determinism world's master rule), and svf at
  `mod = 0` is the inc-2 filter — so every existing render stays bit-identical. **Verified, not
  assumed**: the gates run before the seal.
- **Zero allocations on the audio path** — every new wire (cv reduce/interp, event merge) works
  in build-time-allocated storage; the counting-allocator tests extend over cv/event graphs.
- **Stress/determinism**: the world only wires sine/gain/rms audio ports; hashes stay
  debug==release (expected `b42068ec7b206789` unchanged — verified by the run, not asserted
  blind).
- `required`-unconnected inputs stay **unenforced at build** (the determinism world's spare
  gain depends on it; it is v0 behaviour and changing it now would move refusals in the stress
  counters). Declared in the executor docs as an open item, not discovered later.
- No pre-emption of WO-009 (no clocks, no tick scheduling), WO-013 inc 4 (no live wire levels),
  ring publication (meters stay relaxed atomics), or the eight pending modules (they land in
  WO-014 inc 3 against this contract — membrane/env-ad/mixer unblocked, lfo/clk-div still wait
  on WO-009, tap/scope on the rings, out/main on the canvas handover).

## Order of work

A. module-api: `event.rs`, `module.rs` v1 (`AudioCtx`, `CvIn`/`CvOut`, `Resources`, `MAX_PORTS`).
B. module-api: manifest/decode `event_kinds` + port-cap validation.
C. module-api tests: contract.rs, throwaway_module.rs, api_snapshot.rs.
D. executor v1 (the big one) + its unit tests.
E. modules.rs migration + rms cv + svf mod + manifest + mod-demo patch + docs regen.
F. app: exec tap/mod-demo; selftest/bridge untouched semantics.
G. sparq-audio tests: executor.rs refusal flips, modules_golden.rs, new `contract_v1.rs`
   (cv reduce/interp table-driven · event merge/tie-break/overflow · refusals · zero-alloc ·
   the bit-identical modulation acceptance).
H. compat-matrix drift gate.
I. Full gate sequence (fmt · clippy default/ui/bootstrap/MSVC×3 · workspace tests · 6 python
   gates · release goldens · selftest 8/8 · ui --audit · modules --strict · exec both patches).
J. Docs, seal (`sync_check.py --write --sync wo008-inc4 --sent 2026-09-26`, verify, self-test),
   zip + namelist, SYNC.md/CHECKLIST/WORKORDERS/LATER updates.
