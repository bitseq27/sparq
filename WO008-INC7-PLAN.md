# WO-008 increment 7 — host-side `required`-unconnected enforcement (plan of record)

**Status:** decisions recorded BEFORE the code (the inc-4/inc-6 discipline). The sandbox-track
item the checklist names "the next candidate in this list": small, well-specified, declared in
LATER.md §WO-008 — and deliberately its own increment, because it is the one change that MOVES
the stress baseline (`b42068ec7b206789`, 10 001 blocks · 7 203 swaps · 2 797 refused) and the
determinism world's shape. The movement is the point, recorded not hidden.

**What exists today.** `Port::required` (module-api) is decoded, defaulted (`true`), validated —
and consumed by nobody: the executor's header says it in words ("a `required` input that is
unconnected is NOT refused at build: the module sees the explicit unconnected signal and answers
with the status it chooses"). The manifest vocabulary is already deliberate across the seventeen:
the six single-input processors (`ana/rms`, `flt/svf`, `fx/bitcrush`, `util/delay`,
`util/gain`, `util/panner`) declare required `in` (by default-true); every source, matrix,
display, sink and trigger input (`mixer` in-0..3/cv-0..3, `tap.in`, `scope.x/y`, `out/main.in`,
`ad.trig`, `membrane.trig`, `clk-div.in`, `lfo.sync`) is explicitly `required = false`. The
field table's note is the semantics: "optional inputs get an explicit unconnected signal, never
silence-by-accident" — and the compiled doc on `Port::required` already scopes it: "**whether
an unconnected INPUT is an error**".

**What ships.** The host keeps the manifest's promise: `Executor::build` refuses a patch in
which any node has a `required` input port with zero incoming edges. A module that said "I need
this" is never built without it.

---

## The decisions

1. **Enforcement point: `Executor::build`, after the edge-rules pass, before buffer
   allocation.** The gauntlet's order stays "the most useful refusal first": node sets →
   identity/caps → edge resolution → payload/matrix rules per edge → **required inputs per
   node** → buffers → prepare/activate. Rationale: an ILLEGAL wire is a more specific defect
   than a MISSING one (the user can see the wire they drew), so edge rules answer first; and the
   check needs the resolved-edge picture, so it cannot run earlier. It runs before any buffer or
   module work so a refused build never touches a module. Fail-fast with ONE `ExecError`, in
   execution order (graph order, manifest port order — first violation wins): the build's
   existing shape, and deterministic by construction. The kernel's connect layer cannot host
   this check — it is structural and knows no manifests (the recorded layering decision); the
   bridge already surfaces executor refusals verbatim in the shell log, so the canvas gets the
   sentence for free.
2. **Error shape: a new `ExecError::RequiredUnconnected { node, port, module }`**, rendered as
   one sentence with both remedies (plan §7.8's rule: every message carries a fix): connect a
   source, or remove the node — and the manifest-side truth (a genuinely optional port is
   `required = false` in the module's manifest, which is a module change, not a patch change).
   The variant names node id, port id and module id so a canvas log line points at exactly one
   node on screen.
3. **Connected = one or more incoming edges of any carried type.** Audio fan-in is legal
   (summed at the destination), event multi-source is legal (G5's tie-break), cv fan-in is
   refused by the EXISTING edge rules before this check ever sees it — so the required check
   never legitimises a wiring the matrix refuses. Outputs are out of scope by the compiled
   contract's own words (`Port::required` = "whether an unconnected INPUT is an error"); an
   unconnected output (a sinkless `out/main`, an undrawn cv tap) stays legal — the demo patch's
   undrawn `rms.level` wire is the standing example.
4. **No bypass/mute exemption.** The build enforces the manifest; runtime states do not excuse
   structural ones. A bare required-input node is a patch error whether or not it is bypassed —
   and bypassed nodes are still built (the watchdog's auto-bypass is a RUNTIME passthrough, not
   a build exclusion). The canvas consequence is declared, not discovered: adding a processor
   and rendering before wiring it refuses IN WORDS in the shell log (decision 1's path); a
   red-hairline badge for the unconnected-required node is a WO-013-side painter increment,
   parked in LATER.md beside the watchdog hairline.
5. **The determinism world wires its spare.** `World::initial()` becomes sine → gain → rms plus
   the spare gain FED FROM THE SINE (a plain fan-out; 4 nodes, 3 edges). The spare stays a gain
   (it is the retune target, a master candidate and a connect destination — the reasons it
   exists); only its illegal bareness moves. `the_no_mutation_script_is_the_reference_chain`'s
   pin moves 2 → 3 edges with the comment updated. Disconnect mutations can still bare the
   spare — and now that candidate's rebuild is REFUSED and counted, which is the enforcement
   exercising exactly the path the harness exists to stress.
6. **The stress baseline moves ON PURPOSE and is recorded.** Add-gain/add-rms mutations now
   build-refuse until wired; disconnects of a required feed refuse; refusals rise, swaps fall,
   the hash moves. The acceptance's invariants are the assertions (10 001 blocks, 0 audio-path
   allocations, swaps > 1 000, refused > 0, superseded 0, bit-identical replay) — they are
   rate-independent of the baseline and must all still pass with margin. The NEW hash + counters
   are measured in debug AND release, recorded in the build-log entry and the checklist's live
   baselines; historical entries keep the old numbers as history (they were true then);
   `cross_thread.rs`'s doc comment (which cites the hash as the single-owner claim's home) and
   its spare-status comment move with it. README's stale "Measured" paragraph is NOT touched —
   it has been stale since before this increment (559 tests quoted against 761 shipped);
   reviving it is its own docs pass, declared here so the omission is a decision.
7. **The unconnected-signal semantics survive, at their new home: OPTIONAL ports.**
   `an_unconnected_input_is_silenced_and_reported` (tests/executor.rs) keeps testing "the
   module SEES unconnected and answers with its status" — through a probe manifest that declares
   the input `required = false`, which is where that behaviour legally lives after this
   increment. A NEW gate beside it proves the refusal: bare required input → build fails, the
   sentence names node + module + port + both remedies, and a compliant twin (same patch, one
   wire added) builds. Probe manifests in the other test files are checked against the same
   rule: any probe whose input is required-by-default and deliberately bare becomes optional
   (a one-word manifest change in the test, semantics preserved) — the suite run is the census.
8. **Docs move in the same session (#88's lesson, applied preemptively):** the executor header's
   "NOT refused at build" bullet becomes the enforcement sentence; `docs/api/module-api-v1.md`'s
   "still declared open" list loses the item (with the reason it was open — the harness
   comparability — recorded as discharged by decision 6); `docs/api/manifest-fields.toml`'s
   `ports[].required` note grows the host-enforcement clause (a note edit, not a vocabulary
   change — the field, its default and its domain are untouched, so no table-first move is
   needed; `the_required_field_list_matches_the_ratified_table` pins paths, not notes);
   `docs/api/manifest-schema.md`'s row likewise. LATER.md's §WO-008 declared-limit entry is
   struck through and the checklist's WO-008 limits line updated.

## What must NOT move (the shape-of-the-diff proof)

Every audio golden: the three demo patches were read and are compliant (their only undrawn
wires are OUTPUTS — `demo`'s `rms.level`), the Wo005Graph/phase-B bootstrap path never passes
through `Executor::build`, the canvas default/demo patches render through the bridge and must
stay green (the suite is the proof), the three pinned exec renders keep their hashes at their
durations (`1621e1f65b1b64e1` / `f2303f13aa0cf299` / `53de3b1f3f40e3c9`), `selftest --golden`'s
wo005 manifest hashes are untouched, `modules --strict` stays 17/17 (no manifest changes — the
vocabulary was already declared; only its enforcement ships), and the executor's render path is
byte-identical (enforcement lives entirely in `build`).

## Acceptance (sandbox)

* The refusal gate + the migrated unconnected-signal gate pass; the full suite green with the
  new total recorded.
* `mutation_stress` passes its invariants and prints the NEW evidence line; the hash + counters
  match across debug and release (determinism's actual claim, re-proven at the new baseline).
* `the_no_mutation_script_is_the_reference_chain` pins the wired world (4 nodes, 3 edges).
* Every pre-existing audio golden re-verified unchanged; fmt + all runnable clippy cells + the
  5 python gates clean; `log_check.py` BASELINE moved with the seal (#83's discipline).
* Sealed as `sync wo008-inc7`, stacked on the device's applied `sync-wo006-inc15` chain.

---

## Postscript — the acceptance, measured (2026-09-29, after the run)

Every box above, with its number: **762 tests** (was 761; +1 `a_required_input_with_no_wire_
is_refused_in_words`; the silenced-report test and the reference-chain pin migrated in place),
0 failed, 1 ignored · the stress invariants all pass and the NEW evidence line reads
**`10001 blocks · 2383 swaps · 7617 refused · 2 nodes / 0 edges final · max path latency 94 ·
hash 7bb06379bd6845e5`** — identical in debug AND release (was `b42068ec7b206789` · 7 203 ·
2 797; the refusal/swap mix flipped exactly as decision 6 predicted: bare-add and
bare-disconnect candidates are now build refusals) · the reference chain pins 4 nodes / 3 edges
· every pre-existing audio golden re-verified unchanged in release (the three pinned exec
renders at their durations, wo005, phase-b, all module goldens) · fmt + all runnable clippy
cells + 5 python gates clean · `selftest --golden` PASS (9 gates) · `ui --audit` PASS (25
smokes) · `modules --strict` 17/17 · `canvas-render.wav` 1 920 046 bytes (the device
baseline's exact size), sha256 `d7ad294ea0b5e6e960f8dbc7f231dcbd84edec327e6352e80dc2f6ed786ec0b7`
· cross-thread ledger: 21 509 blocks · 2 299 staged = swaps = reclaimed · 7 701 refused ·
0 allocs · 0 errors · `log_check --self-test` 25/25 with the BASELINE moved (762 /
`src 92f/2021628B`) · sealed **`sync wo008-inc7`** (17 entries: 5 stamp-covered + 2 test
sources + 10 documents). One deviation from the plan, recorded: the census found NINE touched
tests, not eight (the headless smoke 7 needed the demo's free input, which decision 5's world
wiring had removed — resolved by the demo's fourth node becoming a bare `out/main`, an addendum
to decision 5 covering the canvas mirror; see `bridge.rs::demo_graph`'s doc). Sealed anyway as
planned; defect #93 (the `out`-dir snapshot trap that ate `modules/out/main/sparqmod.toml`
mid-increment, recovered byte-exact via the operator) logged in the CHECKLIST.
