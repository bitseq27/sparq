# WO-007 task 1 — review packet

**Status: PARTIALLY RATIFIED — see §0. Three decisions taken 2026-09-22; §1 lists what remains. No
code starts until §1 is closed.**

This is the artefact PHASE0-WORKORDERS WO-007 task 1 asks for — *"write the manifest schema and the
compatibility matrix as **data/tables first**, review, then code"* — plus everything that had to be
settled or surfaced before the tables could be written. It exists because two consumers must read
one table: host validation (which emits `E-*` codes) and the canvas affordance layer, which ADR-005
says is *"a direct rendering of this table."* If the matrix lives in Rust match arms, the UI grows a
second copy, and the drift is invisible until a patch refuses to connect on screen for a reason the
host happily accepted.

**Companion files (both parse as TOML, both new, neither consumed by code yet):**

| File | Contents |
|---|---|
| `docs/api/compat-matrix.toml` | 6 port types · 21 same-type cases · 5 verdicts · 4 cross-type adapters · **7 gaps (G1–G7)** where ADR-005 is silent |
| `docs/api/manifest-fields.toml` | 80 field rows (22 required) with type, domain, cross-field rules and the error code each violation produces · **3 gaps (G8–G10)** |

Nothing in `sparq/` was edited to produce these. `README.md`, `SYNC.md`, ADR-005 and
`module-api-v1.md` are untouched: the packet proposes their amendments (§2, §6) rather than
pre-empting your ratification.

---

## 0. Decisions ratified 2026-09-22 (by the author, in session)

| Item | Decision | Consequence |
|---|---|---|
| **G2** — modulator range mismatch | **Refuse, and offer a converter.** Nothing invisible ever happens to the sound | Adds a third trivial module, `util/range` (bipolar ↔ unipolar). The `cv→cv` range-mismatch cell moves from `gap` to `adapter: util/range` |
| **§4** — adapters that don't exist | **Build the two tiny ones now, in Phase 1** | `util/offset` and `util/gate-to-cv` join Appendix B's Phase-1 list. `dat/mapper` stays Phase 5–7, and `data→cv` degrades from `adapter` to `refused` until it ships. With `util/range` that is **three** new trivial Phase-1 modules |
| **Q2 / G10** — sub-block processing | **Forbidden in v0** | `resources.internal_rate` is **removed** from `manifest-schema.md` §8: the field existed to answer a question whose answer is now "no". G10 closes as a removal, not a clarification |

**What "forbidden in v0" actually costs — measured, not assumed.**

* **Grain onsets stay sample-accurate.** §9 already guarantees events arrive pre-sorted by sample
  offset, and a module renders into the block buffer, so a granular cloud can still start a grain at
  sample 37 of 64. No musical loss there — this was the worry behind the §14 note, and it does not
  bite.
* **The escape hatch is cheap and already built.** `BlockContext.frames` is a runtime field, not a
  constant, and the WO-006 HAL already carries a FIFO chunker mapping the device period onto the
  sparq block. A module that genuinely needs finer time resolution gets it by shrinking the *host*
  block. Cost, from the §5 dispatch measurement: 100 modules at 64 frames is ~1.9 µs per block ⇒
  **0.13 %** of a core at 44.1 kHz; at 16 frames there are 4× the blocks ⇒ **~0.52 %**. Roughly
  0.4 % CPU buys 4× the resolution — and that is the sandbox's number, so SATURN should be better.
* **The real constraint, stated plainly:** parameter *movement* inside a block is unavailable in v0 —
  §9 makes the parameter snapshot immutable for the block. A granular cloud must therefore derive its
  per-grain values from that one snapshot by internal interpolation. If that ever proves musically
  insufficient, the remedy is the host block size above, **not** a contract change. That is precisely
  why forbidding it now is safe: the fix does not require reopening a frozen document.

## 0.1 Second round of decisions (2026-09-22, same session)

| Item | Decision | Where it landed |
|---|---|---|
| **G1** object-count mismatch | **Approved as proposed** — converter required, never renumbered | matrix cell → `conditional`, adapter `spa/objects` |
| **G3** fast modulator → slow input | **The receiving module decides** *(against recommendation)* | new manifest field `ports[].cv_reduce` |
| **G4 / Q3** slow → fast | **Approved** — host interpolates, module declares | new manifest field `ports[].cv_interp` |
| **G5** event fan-in | **Approved** — free fan-in, deterministic tie-break | matrix cell → `compatible` + `tie_break` |
| **G6** data-stream version mismatch | **Load it anyway, with a warning** *(against recommendation)* | matrix cell → `conditional` + journal entry required |
| **G7 / Q5** gpu ports and tiers | **Deferred** | the only cell still `gap`; interim rule + Phase 5 trigger recorded |
| **Q1** dispatch | **Approved** — the measured hybrid | §5 stands as the ADR-005 addendum source |
| **Q4** oversampling | **Approved** — host provides, module declares need | unchanged |
| **G8 / G9** missing error codes | **Approved** — adopt the derivation rule | both marked ACCEPTED in the field table |
| **Required-field list** | **Ratified at 22** | `[validation.required_list]` in the field table |

**G3 — what "the module decides" adds.** Two new manifest fields, now in `manifest-fields.toml`
(82 rows): `ports[].cv_reduce` (`last|first|mean|min|max|peak`, default `last`) and
`ports[].cv_interp` (`hold|linear|spline`, default `hold`). **One hard constraint comes with it: the
policy must be declared in the manifest and never chosen in code.** `first` and `last` produce
different audio, so an undeclared choice would make a journal replay diverge from the original
render — that is ADR-007, a pillar, not a preference. Declaring it also keeps the golden harness
honest, because manifests are already cached by `(id, version, file hash)`.

Worth recording that this answer *improved* the contract: G3 and G4 are now symmetric, so a single
rule covers both directions — **the receiving module owns any rate change on its own input** — where
my proposal would have had the host own one direction and the module the other.

**G6 — what "load with a warning" adds, and the risk it carries.** The warning must not be
cosmetic, and it doesn't have to be: the machinery already exists. `data` records carry
`flags(ok|stale|discontinuity|estimated)` (§4), and every data *consumer* already declares a
`stale_policy` of `hold|decay|zero|error`. So a version-mismatched stream loads with **every record
flagged `stale`**, and the consumer's own declared policy decides what that means — a module that
cared enough to declare `error` still gets to refuse. No new mechanism, which is why this answer is
more survivable than it first looked. Two additions are mandatory: a **canvas badge**, and a
**journal entry** recording that the project opened with a mismatch — without it a replay would
silently differ from the original session. Route it through the existing repair dialog (§12, "repair,
never crash") as a fourth option alongside stub / substitute / cancel.

*Residual risk, stated plainly:* this is the one decision that can produce wrong sound with no
error. The mitigations make it **visible and replayable**, not impossible. If a wrong-sound bug ever
traces here, the fallback is to default `stale_policy = error` for mismatched schemas — a one-line
change, recorded now so it does not have to be invented under pressure.

**G7 — why deferring costs nothing today.** *(Corrected after checking WO-014 rather than assuming:
my first draft claimed no `gpu` module is scheduled before Phase 5–7. That is false.)* Phase 0 does
contain two `gpu`-touching modules — `ana/tap` ("generic signal tap for displays", whose stated
contract stress is **"gpu/display plumbing"**) and `dsp/scope` ("the first real visual module",
accepted at ≥ 60 fps with zero audio-thread cost). So the `gpu` **port type must be implemented in
Phase 0**, and ADR-005's paper test already names the shape: scope display is `audio`/`cv` in, `gpu`
out.

What G7 actually defers is narrower: whether a **third-party** (T2/T3) module may hold a `gpu` port.
Both Phase-0 modules are first-party T1, which every candidate answer to G7 permits — so the
deferral blocks nothing that is scheduled. No T2/T3 module appears in Appendix B before Phase 5–7,
which is where the trigger sits. The deferral is made safe by two entries in the matrix: an
**interim rule** (no `gpu` module may be T2 or T3 until decided — compatible with `dsp/scope` and
`ana/tap` being T1) and a **provisional marker** on §14's camera-optical-flow row, so nobody builds
against a promise that may be withdrawn. Trigger recorded: answer it when the wasm tier is scoped,
Phase 5.

**Consequence for task 2, from the correction:** the `gpu` port type is *not* optional in v0 even
though its tier rule is undecided. The contract must carry `gpu` handles, and `dsp/scope` will
exercise them in Phase 0. Deferring G7 defers one cell of the matrix, not the port.

## 1. What is now open

**Nothing blocking.** One cell of the matrix is still `gap` (G7), and it has an interim rule and a
Phase 5 trigger. Task 2 can start.

**Document amendments — APPLIED 2026-09-22.** Verified after editing: LF-only line endings
preserved (`.gitattributes` mandates `eol=lf`), all files valid UTF-8, markdown table column counts
match their headers, `cargo fmt --all --check` clean, **280 tests pass**, 4 Python gates clean.

| File | Amendment |
|---|---|
| `docs/api/manifest-schema.md` | **done** — `resources.internal_rate` deleted from §8 with the rejection reasoned; `cv_reduce` and `cv_interp` added to the `ports[]` table; the five derived codes plus the derivation rule and the cross-field rule ids added to the catalogue (123 → 133 lines) |
| `docs/api/module-api-v1.md` | **done** — channel-set **cascade** rule inserted at the end of §2 with its named test; `cv_reduce`/`cv_interp` added to the §3 port table; §14's granular note resolved, HOA action marked done, optical-flow row marked **PROVISIONAL**, result line updated; §16 retitled *ANSWERED* with all five answers (262 → 297 lines) |
| `docs/adr/005-port-type-system.md` | **done** — addendum appended: the measured dispatch decision, the `cv` range rule, the five closed cells as a table, the "never offer a converter that does not exist" rule, and the deferred `gpu`-tier question with its trigger (53 → 96 lines) |
| `docs/adr/009-executor-and-hal.md` | **done (added later — defect #59)** — executor decision 7 was the item that *asked for* this measurement and still said "measure … and choose"; it now records the numbers and the choice. Its Consequences bullet is upgraded from a mitigation to a **requirement**: the enum is generated from module registration, because a hand-written one would falsify WO-007's zero-engine-edits criterion |
| `SPARQ-PLAN.md` | **done** — Appendix B Phase 1 **and** §17 Phase 1 both amended, 12 → 15 modules. Both lists had to change; they are separate authoritative copies |
| `PHASE0-WORKORDERS.md` | **done** — WO-007 split out of the "not started" row into its own status row; WO-014's in-scope line corrected; build-log entry added with **defects #54–#59** and this increment's verification ledger (825 → 872 lines) |

**Defect #56 — CLOSED (2026-09-22): the module is `ana/tap`.** §17 Phase 1 called it `ana/scope-tap`
while Appendix B and WO-014 both called it `ana/tap`; §17 is corrected and all three lists agree. The
reasoning is recorded with the decision: two of three documents already said it, it matches `ana/rms`
beside it in the same family, and "scope" in the name would promise something narrower than the module
is — WO-014 describes a *generic* signal tap for displays, and the display sheet has twelve of them.
Deferring it further would have turned a one-line document fix into an alias table and a migration,
because module ids are stable (§12) and WO-014 is the work order that builds it.


## 2. The two actions the contract doc left open on itself

`module-api-v1.md` §14 ran the seven-module paper test and returned *"contract holds, with one rule
to add."* Both of its outstanding items are still outstanding.

**2.1 — HOA: the re-prepare cascade.** The §14 row carries *"**Action:** add a rule to §2 and a
test."* §2 currently states the **local** rule only: *"Any change to these ⇒ `deactivate → prepare
→ activate`."* What is missing is the **cascade** — that when one module's channel set changes, the
executor treats it as a *resource* change rather than a graph mutation, and re-prepares every
neighbour whose effective channel set depends on it. Proposed insertion at the end of §2:

> A channel-set resolution is a **resource** change, never a graph mutation. When `prepare` resolves
> a `variable` port to a concrete set, every module whose effective channel set depends on that port
> is re-prepared in topological order in the same control-thread pass; the graph structure, its
> version and its mutation counter are untouched. Resolution is idempotent, and a pass that does not
> change any resolved set terminates it.

Test to add with it (belongs to WO-008, declared here): `channel_set_change_reprepares_neighbours_without_bumping_graph_version`, plus a cycle guard — an `ambisonics:N` order change that
re-prepares a neighbour which changes its own set back is a livelock, and the test must prove the
pass converges.

**2.2 — Granular: event sorting vs sub-block scheduling.** The §14 granular row is a ✅ conditional
on *"confirm event sorting guarantee (§9) covers sub-block scheduling."* §9 does guarantee *"all
events for the block pre-sorted by sample offset"* — that settles **input ordering**. It says
nothing about a module that processes **internally** at 16 samples while the host block is 64. That
is §16 Q2, and it is still open — so the ✅ is provisional. See G10: `manifest-schema.md` §8 already
declares `resources.internal_rate (optional sub-block rate)`, i.e. the schema has committed to an
answer the contract doc lists as undecided. One of the two is wrong.

## 3. Matrix gaps G1–G7 (proposals — ratify or replace)

| # | Cell ADR-005 leaves silent | Proposal |
|---|---|---|
| **G1** | `objects:K → objects:J` | Compatible iff K == J; otherwise `adapter` via `spa/objects`. Never silently re-numbered — object count is the spatial meaning |
| **G2** | `cv` range mismatch (bipolar ↔ unipolar) | **Refuse, offer an adapter.** A silent rescale is precisely the invisible transformation ADR-005 exists to prevent. Costs one tiny `util/range` module |
| **G3** | audio-rate `cv` → block-rate input | Host reduces with a **declared** policy (`first` \| `last` \| `mean` \| `min` \| `max`), default `last`. Must be declared, not chosen: `first` and `last` produce different golden renders, so this is an ADR-007 determinism question, not an implementation detail |
| **G4** | block-rate `cv` → audio-rate input | Answer **with** §16 Q3, in one place: host interpolates, module declares `cv_interp`. Answering it twice is how the table and the code drift |
| **G5** | `event` fan-in | Free, like `cv` fan-out — but the merge must be total-ordered, and the tie-break when two events share a sample offset must be declared (proposal: stable by connection id, then by insertion order, so it is reproducible in the journal) |
| **G6** | `data` schema version mismatch (same id) | Refuse in v0. Stream migration chains are Phase 1+ work; a silent reinterpretation of `sparq/imu9@1` vs `@2` is a wrong-sound bug with no audible symptom |
| **G7** | Are `gpu` ports T1-only in v0? | Yes (§16 Q5's recommendation) — **but then the §14 optical-flow row is wrong**, because that module is T3 with a `gpu` input. Either the row's ✅ becomes "Phase 5" or `gpu` is opened to T3 now |

## 4. Adapters ADR-005 promises that do not exist

ADR-005 makes cross-type refusal acceptable by promising a one-tap adapter, and the canvas is
specified to render that offer. Three of the four named adapters are not in Appendix B at a phase
that could honour the promise:

| Edge | ADR-005 names | Appendix B has | Problem |
|---|---|---|---|
| `data→cv` | "a Mapper" | `dat/mapper`, **Phase 5–7** | The affordance cannot be offered for four phases |
| `cv→audio` | "an oscillator/offset" | `syn/sine` (Phase 1); **no `util/offset`** | A sine is a source, not an offset; the module the ADR describes does not exist |
| `audio→cv` | "an analyser" | `ana/rms` (Phase 1) | ✅ fine |
| `event→cv` | "a gate/trigger converter" | **nothing** | No module, no phase |

Proposal: add `util/offset` and `util/gate-to-cv` to Phase 1 (both are trivial — a handful of lines
each, and both are needed by the first real patches anyway), leave `dat/mapper` in Phase 5, and
record in the matrix that `data→cv` degrades from `adapter` to `refused` until `dat/mapper` ships.
The alternative — shipping the offer UI before the modules — produces a button that does nothing,
which is the exact failure the suppression log in `sparq-ui` was built to make impossible.

## 5. The five §16 open questions

**Q1 — dispatch: trait objects vs generated enum. ANSWERED BY MEASUREMENT.**
Harness: `tools/dispatch-bench/` (a standalone cargo project, excluded from the workspace), 100 modules × 64-sample block,
sparq's exact release profile, 4 distinct implementors so the vtable pointer genuinely varies, three
runs. Module body identical in all variants, so the difference is dispatch and nothing else.

| | `Box<dyn Module>` | `enum + match` | monomorphised |
|---|---|---|---|
| **per block** (1 dispatch/module) | 1.65–1.95 µs | 1.26–1.30 µs | 1.23–1.31 µs |
| **per sample** (6400 dispatches/block) | **27.1–35.6 µs** | 2.08–2.22 µs | 2.05–2.17 µs |
| ns per call, per-sample | 4.2–5.6 | 0.32–0.35 | 0.32–0.34 |

Two results, both load-bearing:

1. **Enum dispatch is free.** It is within noise of fully monomorphised code — the `match` compiles
   away. There is no cost argument against it.
2. **Per-sample trait objects blow the budget on their own.** 27–36 µs for 100 *null* modules, when
   the whole acceptance target is < 20 µs. At block granularity trait objects cost 1.3–1.5× and sit
   at ~10× headroom — affordable.

So the answer is the hybrid the work order anticipated, and the measurement says exactly where the
line goes: **`process(block)` may be `Box<dyn Module>`; nothing called per sample may be.** Per-sample
paths are enum-dispatched or generic. This becomes the ADR-005 addendum, with the numbers.

*Caveats, stated plainly:* 2-core virtualised Linux sandbox, ~1 GB RAM — **not SATURN, not
Windows**. The ordering and the ratios should transfer; the absolute nanoseconds will not. Re-run on
the stage device before the addendum is ratified. Also note the harness uses `Instant::now`, which
`clippy.toml` bans workspace-wide — if this benchmark is promoted into the repo it needs the same
targeted `#[allow]` + justification pattern as `device/null.rs`, or it will not lint.

**Q2 — sub-block processing.** Proposal: allow it via `resources.internal_rate` (the field already
exists), require that it be a power-of-two divisor of the block size, and require the module to
remain deterministic per ADR-007. Then close §2.2 and G10 together.

**Q3 — block-rate `cv` interpolation.** Ratify the doc's own recommendation (host interpolates,
module declares `cv_interp`) and make G4 point at it instead of restating it.

**Q4 — oversampling.** Ratify the recommendation: host provides, module declares need.

**Q5 — `gpu` ports T1-only in v1.** Ratify, and fix the §14 optical-flow verdict (G7).

## 6. Error catalogue: two acceptance blockers

WO-007 requires validation to reject five things *"each with an actionable error message."* The
19-code catalogue in `manifest-schema.md` covers four. Checking it field-by-field against the schema
tables produced two structural gaps:

- **G8 — no code for a missing required field.** 22 fields are required; the only two `*-MISSING`
  codes are `E-STATE-SCHEMA-MISSING` and `E-SIGNATURE-MISSING`, both field-specific. `E-UNKNOWN-KEY`
  catches extras; **nothing catches omissions**. The acceptance criterion names "missing required
  ports" explicitly, so as the catalogue stands that criterion is unmeetable.
- **G9 — no code for an invalid enum value in general.** Five specific unknown-value codes exist
  (port type, unit, channel set, ambi order, widget kind). Roughly twenty further closed
  vocabularies — `classification.top/kind/tier/stability`, `direction`, `cv` rate/range,
  `event_kinds`, read/stale policy, `multiplicity`, param type, curve, smoothing, morph, dtype,
  interpolation, rate_class, clock_domain, `voices.policy`, size_class, cpu_class, oversampling,
  touch_class, package_format, channel — have none. `tier = "t9"` has no error code to produce.

Both have the same cause: the 19 codes were accumulated case-by-case as designs were written, not
derived from the schema. Proposal — derive them, so a new field brings its own codes:

```
required field absent        -> E-KEY-MISSING:<path>
enum value not in domain     -> E-ENUM-UNKNOWN:<path>
cross-field constraint       -> E-CROSS-FIELD:<rule id>
string over max_chars        -> E-VALUE-TOO-LONG:<path>
malformed scalar             -> E-VALUE-MALFORMED:<path>
```

Cross-field rules needing ids (each currently has no code): `params[].per_voice` requires
`voices.policy != none` · `ui.custom_draw` is T1-only · `capabilities.process_spawn` is T3-only ·
`ports[].stale_policy = decay` requires `decay_ms` · `params[].type = enum` requires `options[]`.

**Also to ratify: the required-field list itself.** The prose marks 20 rows `Req: yes`; the table
marks 22 fields required. The three extras are `state.schema_id` and `state.schema_version`
(prose §7 has no Req column but says *"required; serialisation is mandatory for every module"*) and
`resources.latency` (implied by `module-api-v1.md` §8 and by the existence of `E-LATENCY-UNDECLARED`).
Required-ness is now load-bearing — it determines which `E-KEY-MISSING` codes exist — so the list
should be explicit rather than inferred.

## 7. Verification actually run this session

Measured, not asserted — including what was **not** run.

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo test --workspace` | **280 passed, 0 failed, 1 ignored** |
| clippy: default · `bootstrap-audio` · `ui` | clean, `-D warnings` (the `ui` cell did 41 s of real work) |
| Gate proven failable | injected a `needless_return` + undocumented fn into `sparq-ui` → `error: could not compile`; reverted byte-identical |
| Python gates (4) | `token_gen --check` 0 files to write · `token_audit` clean (3 svg + 12 source) · `unsafe_audit` clean (10 allowlisted, 62 `.rs` scanned) · `check_text_io` clean |

**The 279-vs-280 discrepancy in `RESUME.md` is closed.** The tree holds 288 `#[test]` attributes,
9 of them in `hal/wasapi.rs` behind `cfg(all(windows, feature = "hal-wasapi"))` ⇒ 279 compiled on
Linux. `cargo test` discovers **281**: those 279 plus **2 doctests** in `sparq-kernel`, one of which
is ignored (`crates/sparq-kernel/src/alloc.rs` line 10). 281 − 1 ignored = **280 passed**. Per
binary: audio 172 unit + 12 integration (golden 2, golden_demo 2, rt_discipline 8), kernel 58, ui
37, **app 0**. The exact match to the ledger is the evidence the restored snapshot is the tree that
produced it.

*Not re-run this session:* the three MSVC cross-check cells, and the release-dependent gates
(`golden`, `golden-demo`, `selftest --golden`, `ui --audit`) — so the golden hashes
`ba577186c988db21` / `dd975a24f03b19c1` are quoted from the ledger, **not** re-verified. *Observation,
not a defect:* `sparq-app` has zero tests; it is covered by the golden and selftest gates instead.

## 8. What follows ratification

Tasks 2–5 in the work order's order, now that the tables exist: define the traits with the Q1 hybrid
(and measure again on device) → implement `util/gain`, `syn/sine`, `ana/rms` as contract tests →
write `docs/module-author-guide-v0.md` → add the API surface snapshot test. The crate is
`sparq-module-api`, already named as planned-but-not-created in `Cargo.toml`; ADR-000 says it joins
`members` only when it has contents. It inherits `[workspace.lints]`: `unsafe_code = "forbid"`,
`missing_docs = "warn"` (an error under `-D warnings`), and every `clippy.toml` real-time denial —
so the contract's "structurally impossible to allocate or lock" criterion is partly enforced by
lint before a single type is written.
