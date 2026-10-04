# ROUND6-HANDOFF.md — fourteenth session handoff: WO-017 close, contract v1.1 FROZEN (2026-10-05)

**Purpose:** the first file the NEXT session reads (besides CHECKLIST.md's new head paragraph).
This round **CLOSED WO-017**: the instrument contract v1.1 is frozen, the freeze checklist is
resolved down to two WO-018-owned implementation debts, and the parallel-build gate of ADR-010
decision 7 is open. The round-4 seal (`sparq-update-2026-10-02.zip` rev 4, device rounds owed)
and the round-5 planning artefacts stand underneath everything below.

---

## 0. READ THIS FIRST — state of the tree, delivery, provenance

**What this tree is:** a fresh clone of `github.com/bitseq27/sparq` taken **2026-10-05** (head
`6f842d7` — origin had gone PUBLIC again and carries the round-5 tree the device applied and
pushed) **plus the WO-017 close round** written on top in this sandbox across 2026-10-05.

**No incidents this session** — the first clean round since the defect-#94 class started:
* The clone arrived with a complete `.git` (no history rebuild needed; this round's commits are
  real commits on `main`, listed in §8) and with `modules/out/main/sparqmod.toml` present (the
  standing rule was honoured: `ls modules/out/main/` FIRST — the file is the round-5
  reconstruction, 1952 B; the device adopted it, so the stamp mismatch below is the KNOWN one).
* The sandbox environment was rebuilt from nothing per ROUND5-HANDOFF §7 (~25 min: rust 1.99.0
  standalone + both wasm std faces + ziglang linker + jco); `.cache` did not persist from the
  previous session, as predicted. TWO recipe deltas, recorded in §7 (wasm-tools install; the
  componentize correction).

**Delivery artefacts (this box, `/home/user/handoff/`, checksums in the sibling
`sha256sums.txt`):**
| artefact | what it is |
|---|---|
| `sparq-update-2026-10-05.zip` | FULL-TREE overlay: every source file as verified below. Excludes `.git/`, all `target/` dirs, and `Cargo.lock` (deliberate — §4). Extract AT THE REPO ROOT, overwriting. |
| `sparq-handoff-2026-10-05.bundle` | `git bundle` of `main`: the clone's history + this round's five commits. For review / `git pull` into a scratch clone; NOT needed for the device apply. |
| `sha256sums.txt` | checksums of both. |

**Every number in §5 was measured on THIS box today (2026-10-05), rustc/cargo 1.99.0, `-j 2`,
zig-cc as the linker.** Nothing is quoted from an earlier session.

---

## 1. The operator rulings this round implements

Asked and confirmed via the question tool, 2026-10-05 (the freeze checklist's open contract
items — ROUND5-HANDOFF §6):

1. **`data-value`/`data-record` RATIFIED AS-IS** at the v1.1 freeze. The one vocabulary with no
   native arbiter (data ports: ADR-005-specified, unimplemented; native landing Phase 5) is
   frozen exactly as drafted — §4's dtype set 1:1, including the f64 `vecN` carrier (the one
   judgement call, confirmed). Recheck trigger recorded inline in types.wit: when the native
   data-port work lands, compare and reshape only through the versioning rules. Until the host
   routes data at all, an instrument declaring a `data` port is **refused at load in words**
   (WO-018 validator) — vocabulary frozen, routing pending, never half-working.
2. **`t-wall-ns` REMOVED from `host.time-info`.** The checklist asked whether it belongs in the
   guest world "at all given the host-stamping rule"; the answer is no: `frame-context.time-sec`
   already carries the draw phase's animation clock (ADR-006's UI clock), outgoing data records
   are host-stamped anyway, and inside `process` the only lawful clock is `block-input.t-sample`
   — no lawful guest consumer of wall time remained. `now()` is fully deterministic; the wall
   clock stays host-side forever. `data-record.t-wall-ns` (the host-owned field riding the
   record) is unchanged, its doc now says the guest writes 0 on output.
3. **Browser grouping: data side this round, chrome in WO-018.** `BrowserItem` carries `Layer`
   and the bridge populates it from the validated manifest, but ranking/rows/geometry do NOT
   read it — pinned by `the_layer_is_data_only_ranking_does_not_read_it`, so every audit-pinned
   geometry stands and `ui --audit` cannot have moved. Visual grouping + badging chrome land
   with WO-018, which owns the instrument browser surface (this sandbox has no ui-window stack).
4. **Contract v1.1 stamped FROZEN.** The WIT README, the build guide (v1 draft → v1.0) and this
   handoff carry the stamp; the nine WIT files are hash-pinned in `cargo test` (§2). The two
   remaining checklist items (`gpu_class` ceiling table, two-instance `configure` ordering) were
   ruled **WO-018 implementation debts, not contract gaps** — the freeze stands without them.

Not asked, recorded: the **wasmtime pin 49.0.2** (crates.io max-stable at 2026-10-05 — matches
the planning round's candidate) + **Component Model / WASI 0.2** baseline (ADR-010 decision 2).
It becomes the `Cargo.toml` line when `crates/sparq-host-wasm` exists; if wasmtime has moved by
then, record the delta — the contract face is the component model, not the runtime's version.

---

## 2. What changed — the complete file inventory (five commits, §8)

**Contract text (commit 1):**
* `docs/api/instrument-wit/wit/host.wit` — `t-wall-ns` removed; header rule 1 + `time-info` doc
  rewritten around the ruling (every WO-009 citation re-verified against the code this session:
  `BlockContext.{sample_offset,tick,ppqn}`, `Clock.{bpm,effective_bpm_at,bar_beat_tick}`,
  `Transport::is_playing` all exist as documented).
* `docs/api/instrument-wit/wit/types.wit` — data vocabulary RATIFIED notes (incl. the vecN
  carrier ruling); `data-record.t-wall-ns` doc: guest writes 0, host stamps both edges.
* `docs/api/instrument-wit/wit/tokens.wit` — bundle-encoding doc names the checked-in golden
  (`token-bundle.json`) and both gates. **NOTE: all three files moved AFTER this commit's
  hashes were taken — the pins in `wit_snapshot.rs` (commit 3) are of the FINAL bytes.**
* `docs/api/module-api-v1.md` §11 — the `instruments/` discovery slot (project root, then
  per-user; cross-layer id collision refused in words) + `E-LAYER-MISMATCH` in the rejection
  list.
* `docs/api/manifest-schema.md` §9 — `displays[]` frozen shape (`sources[]{id, port}`, lod
  vocabulary `auto|full|reduced|minimal`, min_size, colormap token rule) + the data-port
  routing status. **The sources-binding shape was specified nowhere before this round; defining
  it was forced by the validator work — flagged for operator review.**
* `MODULE-BUILD-GUIDE.md` — header v1.0-frozen; §5 WIT status line + the sources shape (paper-
  walkthrough amendment 2); §6 data-port honest status (amendment 1); both logged as the guide's
  own rule demands.

**Native contract crate (commit 2) — `crates/sparq-module-api` + consumers:**
* `src/error.rs` — `CodeKind::LayerMismatch` → `E-LAYER-MISMATCH`; `ALL` 24→25 (20 catalogue +
  5 derived); count test updated.
* `src/manifest.rs` — `LAYERS` const, typed `Layer {Backbone (default), Instrument}` with
  `parse`/`as_str`, `Classification.layer`, `ValidatedManifest::layer()`, the layer rule in
  `check_classification`, the cross-field rule in `check_cross` (fires only when BOTH spellings
  are present — one mistake, one actionable line), 4 new unit tests + the LAYERS↔field-table
  drift pin (defect #84's discipline).
* `src/decode.rs` — `CLASSIFICATION_KEYS`+`layer`, `UI_KEYS`+`displays`, the four DISPLAY_*
  vocabulary consts, `check_displays()` (the first `ui`-section CONTENT validation in the tree —
  its header states exactly what is checked here vs what is WO-018's), 2 new tests incl. **the
  template acceptance criterion** (`include_str!` of the checked-in skeleton manifest).
* `crates/sparq-ui/src/canvas/browser.rs` — `BrowserItem.layer` + the audit-neutrality pin;
  `interact.rs` test catalogue; `crates/sparq-app/src/bridge.rs` — `browser_catalog` populates
  layer, catalogue test asserts all 24 built-ins are backbone (fails on purpose the day an
  instrument registers).
* 8 `Classification` literals gain `layer: None` (mechanical); `docs/api/manifest-fields.toml`
  gains the `classification.layer` + `ui.displays[]` rows; `tests/api_snapshot.rs` gains the
  layer pins (LAYERS value pin, parse/as_str/default, `layer()`).

**Token bundle + snapshot gates (commit 3):**
* `tools/token_gen.py` — `emit_bundle()`; the round-trip invariant as a report line; docstring.
* NEW `design/tokens/generated/token-bundle.json` — the bundle v1 golden (55 066 B; envelope =
  tokens' semver {0,1,0} + `encoding: json` + payload object + payload sha256).
* `design/tokens/preview.html` — regenerated (embeds the report, which grew a line — legitimate
  generated movement, `--check` clean after).
* NEW `crates/sparq-module-api/tests/common/{mod.rs,sha256.rs}` — the test-only hand-rolled
  SHA-256 (zero-dependency workspace by design; proven against FIPS 180-4 vectors before it
  pins anything) + the repo-file readers (`read_to_string` — clippy's `fs::read` ban honoured).
* NEW `crates/sparq-module-api/tests/token_bundle.rs` — sha256 ↔ tokens.json, semver ↔ tokens
  version, encoding json.
* NEW `crates/sparq-module-api/tests/wit_snapshot.rs` — the nine WIT files + the directory set,
  sha256-pinned; the header states the discipline AND its limits (hashes detect change, not
  meaning; parseability is the jco recipe's job).

**SDK at freeze (commit 4) — `tools/sparq-module-guest/`:**
* NEW `wit/` — the frozen contract vendored byte-for-byte (nine files).
* `src/lib.rs` — `generate!` re-pointed at `./wit`; header v0.1-SKELETON → v1.1-FROZEN with the
  contract-carrying version rule; the vendor drift test (skips with a printed note outside a
  checkout — that is what vendoring is for).
* `Cargo.toml` — version 0.1.0 → **1.1.0** (carries the contract version; SDK-only fixes move
  the patch).

**Docs close-out (commit 5):**
* `docs/api/instrument-wit/README.md` — FROZEN v1.1 status head; §4 clock row; §5 rewritten:
  SEVEN items closed with full resolution text, two remain as WO-018 debts; §7 verification
  re-run record + the CORRECTED recipe.
* `PHASE0-WORKORDERS.md` — WO-017 `**Status: CLOSED 2026-10-05**`; acceptance criteria 4/4
  ticked with evidence; the build-log entry (fourteenth session).
* `CHECKLIST.md` — new head paragraph (round-4 Previous-update paragraph dropped; its detail
  paragraph + ROUND4-HANDOFF.md carry the history).
* NEW `ROUND6-HANDOFF.md` (this file).

Nothing else moved. No first-party manifest changed (the layer default absorbs all 24);
`modules/` is untouched; PLAY MAKES SOUND is unaffected (no engine behaviour changed — the
native diffs are validation/decode/data-carrying only, and the executor/kernel/audio paths are
byte-identical except `engine.rs`'s one test literal).

---

## 3. Incidents

**None.** (Recorded because the last three rounds each had one.) The two near-misses, for the
log: (a) the first wasm build attempt failed because build scripts compile HOST-side and need
the zigcc env even for wasm targets — recipe delta, §7; (b) `cargo install wasm-tools
--no-default-features` installs an EMPTY cli (every subcommand is feature-gated) — `--all-features`
is the working invocation, §7.

---

## 4. `sync_check` status: FAIL BY DESIGN — device re-stamps

```
sync_check: FAIL - this tree is NOT sync sync-ui-round-2026-10-03 (11 file(s) differ)
  SIZE  Cargo.toml                          (round 5's exclude line — unchanged this round)
  SIZE  crates/sparq-app/src/bridge.rs      SIZE  crates/sparq-audio/src/engine.rs
  SIZE  crates/sparq-module-api/src/decode.rs     SIZE  .../error.rs    SIZE  .../manifest.rs
  SIZE  crates/sparq-ui/src/canvas/browser.rs     SIZE  .../interact.rs
  SIZE  modules/out/main/sparqmod.toml      (round 5's reconstruction — known, unchanged)
  SIZE  tools/token_gen.py
  FP    src fingerprint: tree 96f/2717264B, stamp 96f/2694477B
  warn  SIZE Cargo.lock (sandbox regeneration, 9 B — cargo-version artefact, NOT a dep change)
```

* The pack carries **no stamp**; after applying the overlay and reviewing, run
  `python tools\sync_check.py --write` on the device, then `scripts\gates.bat`.
* **`Cargo.lock` stays EXCLUDED from the zip** (round-5 reasoning stands, doubly: this round
  added zero dependencies to the root workspace — the new test files use only std + the
  crate's own code).
* **Digest expectations move:** test count **899 → 909** (+10: 4 manifest, 2 decode, 1 browser,
  2 token_bundle, 1 wit_snapshot — the ignored alloc doc-test stands), module docs **24/24**
  unchanged, `src` fp **96f/2717264B** (file COUNT unchanged — the new .rs files are under
  `tests/`, which the fp does not count; seven src files resized). Record the new fp at re-stamp.

---

## 5. Verification matrix — all re-run on THIS box, 2026-10-05

| gate | result |
|---|---|
| baseline BEFORE any edit (fresh clone) | 899/0/1-ignored + fmt CLEAN — the round-5 tree verified green on this box first |
| `cargo fmt --all --check` | CLEAN |
| `cargo clippy --workspace --all-targets -- -D warnings` | CLEAN (exit 0, zero diagnostics — the `fs::read` ban caught the first draft of the test helper; `read_to_string` is the sanctioned pattern) |
| `cargo test --workspace` | **909 passed / 0 failed / 1 ignored** |
| `tools/token_gen.py --check` | PASS — would write 0 files; new report line: `token-bundle payload round-trip … sha256 a9c3fec912468d3c…` PASS |
| `tools/token_audit.py` / `unsafe_audit.py` / `check_text_io.py` | CLEAN / CLEAN / CLEAN (8 scripts) |
| `tools/module_docs.py --check` | **24/24** — no manifest changed |
| `tools/sync_check.py` | FAIL BY DESIGN (§4) |
| WIT parse (`npx jco types … --world-name instrument`, jco 1.35, official wit-parser) | CLEAN — `.d.ts` for all interfaces + world; generated `TimeInfo` confirmed wall-free |
| SDK `cargo test` (host face) | 3 unit (incl. the new drift gate) + 1 doc-test; 2 doc-tests ignored by design |
| SDK wasm builds | `noop_instrument.wasm` release, `wasm32-unknown-unknown` **49 697 B** + `wasm32-wasip1` 80 776 B — built from the VENDORED wit |
| Round-trip smoke (`wasm-tools component new` → `jco transpile` → `harness/smoke.mjs`) | **11/11 PASS** against the edited, frozen contract (refusals still in words, exact shapes) |
| Gates proven FAILABLE (house discipline) | wit_snapshot: one appended comment line → FAILED with the revert/amend wording · token_bundle: tokens.json version bumped → FAILED naming the one-commit rule · SDK drift: appended comment → FAILED naming the re-copy rule. All restored, all green after. |
| `sparq ui --audit` | **NOT RE-RUN here** (no ui-window stack — honest carry-forward, round-5 pattern). This round's only UI-adjacent change is `BrowserItem`'s data field; ranking/geometry are pinned audit-neutral by test. Device re-runs in `gates.bat`. |

Round-4 device obligations stand UNCHANGED and are not re-listed: test006 steps A–U, test004
attempt 4 (ADR-008 exit gate), defect #95 triage, the dropdown picker.

---

## 6. Where things stand — WO-017 CLOSED, WO-018 next

**WO-017: CLOSED.** Acceptance criteria 4/4 ticked with evidence in PHASE0-WORKORDERS §3b.
The freeze checklist (WIT README §5): seven items closed with resolutions; **two remain, both
WO-018-owned implementation debts** (operator-ruled not contract gaps):
1. `gpu_class` → vertex/instance/cell ceiling table (renderer side).
2. Two-instance coherence (decision D): exact `configure` ordering across audio/display
   instances at the boundary swap (loader).
Plus the five remaining validation codes' wiring (`E-PACKAGE-FILECOUNT`, `E-LITERAL-APPEARANCE`,
`E-DISPLAY-PRIMITIVE-UNKNOWN`, `E-CAPABILITY-UNDECLARED`, `E-TOKEN-BUNDLE-VERSION` — shapes
specified in manifest-schema §7; `E-LAYER-MISMATCH` is already native) and the data-port load
refusal, all inside WO-018's validator ticket.

**WO-018 (the instrument host) is unblocked and is the next work order:** `crates/sparq-host-wasm`
(wasmtime 49.0.2 behind the `instrument-host` feature), `instruments/` discovery (module-api
§11's slot is now text), `sparq mod validate`, signing/badging, the asset-hash cache, the
host-side half of the round-trip test (the guest half exists and passes — `harness/smoke.mjs`
is its stand-in until the Rust host lands), the browser's visual grouping + chrome (ruling 3),
and the two debts above. **WO-019** (dev harness + the two reference instruments + the first
hand-off) follows it. The parallel-build gate (ADR-010 decision 7) is OPEN: instrument authors
can now build against the frozen face.

**One flagged judgement call for operator review** (made under the freeze's time pressure,
reversible as a schema clarification): the `ui.displays[].sources[]` binding shape `{id, port}`
— nowhere specified before this round, needed by the validator, now frozen in manifest-schema
§9 + the field table + the WIT's (display-id, source-id) keying. If the operator wants a
different shape (e.g. analysis-slot references), it is a pre-anything amendment: no instrument
exists yet.

---

## 7. Sandbox environment recipe — ROUND5 §7 stands, with these deltas

1. **wasm-tools** (new, needed for the component step): `cargo install wasm-tools
   --all-features --force -j 2 --root /home/user/.cache/toolchain` (~8 min; v1.261.0).
   `--no-default-features` installs an EMPTY binary — every subcommand is feature-gated.
2. **The componentize correction:** `jco componentize` takes a **JS/TS source**, NOT a core wasm
   (round-5 §7's line was wrong). The Rust path is: `cargo build --release --target
   wasm32-unknown-unknown -p noop-instrument` → `wasm-tools component new
   target/.../noop_instrument.wasm -o noop-component.wasm` (the wit-bindgen export macro embeds
   the WIT; no `--wit` needed) → `npx jco transpile noop-component.wasm -o gen` → `node
   smoke.mjs` (copied beside `gen/`).
3. **zigcc env is needed for EVERY cargo invocation, including wasm-target builds** — build
   scripts (proc-macro2, serde_core from wit-bindgen's tree) compile host-side and need the
   linker. Symptom of forgetting: "could not compile `serde_core` (build script) … No such file
   or directory".
4. Everything else as ROUND5 §7: rust standalone tarballs from static.rust-lang.org (rustup
   blocked), both wasm std faces, `pip install ziglang` + the zigcc shim, `CARGO_HOME` in
   `.cache`, node 20 preinstalled, jco 1.35 in a scratch npm dir, `-j 2`, no ui-window.

---

## 8. Git artefacts of this round (real commits on the clone's `main`)

```
6f842d7  (clone head — the round-5 tree as the device pushed it)
1ae05a3  WO-017 close (start): operator rulings applied to the contract text
e3bdcc8  WO-017 close (native): classification.layer + ui.displays across schema, validator, registry, browser data
03676e8  WO-017 close (token bundle + snapshot gates): freeze items 7 and 8 land
7c1a6f3  WO-017 close (SDK at freeze): vendored WIT snapshot + contract-carrying crate version
<HEAD>   WO-017 close (docs): the freeze record  (a doc cannot quote its own commit hash —
         `git log` on the bundle gives it)
```

Author `sparq-agent <agent@localhost>` (placeholder — re-author at will; nothing depends on
sandbox authorship). The bundle in `/home/user/handoff/` carries the clone's history + these
commits. NOT pushed to origin from the sandbox (no credentials; the device stays canonical).

**Device apply (suggested, rev-4 pattern):**
1. Extract `sparq-update-2026-10-05.zip` at `Q:\morphosis\code\sparq`, overwriting. (Or: pull
   the bundle into a scratch clone and review commit-by-commit — this round's history is real.)
2. `git diff` / `git status` — review the round (everything is listed in §2).
3. `python tools\sync_check.py --write` (re-stamp), then `scripts\gates.bat` — expect
   **909 tests / 24 module docs / audit PASS**, new `src` fp **96f/2717264B**.
4. Commit under your own identity; push per your flow. The round-4 device rounds (test006 A–U,
   test004 attempt 4, defect #95, dropdown picker) remain owed and untouched by this round.
