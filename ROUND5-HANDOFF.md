# ROUND5-HANDOFF.md — thirteenth session handoff: the instrument-layer planning round (2026-10-04)

**Purpose:** the first file the NEXT session reads (besides CHECKLIST.md's new head paragraph).
"Round 5" counts sandbox handoffs, NOT operator UI rounds — this round changed **no engine
code**; it is the plan/contract round for the two-layer library and the third-party hand-off
system. The round-4 seal (`sparq-update-2026-10-02.zip` rev 4, 899/24, device rounds owed)
stands unchanged underneath everything below.

---

## 0. READ THIS FIRST — state of the tree, delivery, provenance

**What this tree is:** a fresh clone of `github.com/bitseq27/sparq` taken **2026-10-03 18:08**
(stamp `sync-ui-round-2026-10-03`, the round-4-sealed codebase, 24 modules) **plus the whole
instrument-layer planning round** written on top of it in this sandbox across 2026-10-03 19:16 →
2026-10-04 09:16.

**Provenance warning (the incidents, short form — details in §3):**
1. The sandbox **lost its `.git` object store** between turns, AND origin **went private**
   (anonymous access now answers 404 on the repo page / 401 on `info/refs`), so the upstream
   commits are **unreachable from this box**. The local history was rebuilt as a single root
   **snapshot commit `9809593`** (340 files, full tree) + this handoff's commit on top. The
   snapshot is NOT a fast-forward of upstream; the device's own repo remains the canonical
   history. Delivery to the device is the **full-tree overlay zip** (the rev-4/defect-#94
   pattern: the pack does not depend on a base the receiver might not have).
2. `modules/out/` was **stripped again by the snapshot layer** (defect #94's class: directories
   named `out` are excluded from workspace snapshots). `modules/out/main/sparqmod.toml` was
   **reconstructed** from its generated doc (`docs/modules/out-main.md`) — machine-verified, but
   its comments differ, so it no longer matches the sync stamp byte-for-byte (§4).

**Delivery artefacts (this box, `/home/user/handoff/`, checksums in the sibling
`sha256sums.txt`):**
| artefact | what it is |
|---|---|
| `sparq-update-2026-10-04.zip` | FULL-TREE overlay: every source file as verified below. Excludes `.git/`, all `target/` dirs, and `Cargo.lock` (deliberate — see §4). Extract AT THE REPO ROOT, overwriting. |
| `sparq-handoff-2026-10-04.bundle` | `git bundle` of the rebuilt local history (snapshot + handoff commits). For review/`git pull` into a scratch clone; NOT needed for the device apply. |
| `sha256sums.txt` | checksums of both. |

**Every number in §5 was re-measured on THIS box today (2026-10-04), rustc/cargo 1.99.0,
`-j 2`, zig-cc as the linker.** Nothing below is quoted from an earlier session.

---

## 1. The operator rulings this round implements

Asked and confirmed via the question tool, 2026-10-03:

1. **Two-layer library.** Basic first-party modules stay the **backbone** (utilities/control,
   T1 native). The new **instrument** layer is the performance/control tier: complex, visually
   rich, third-party-authorable, running in the **T2 WASM sandbox pulled forward from Phase 5**
   (ADR-002's review trigger — "earlier if a collaborator needs to ship a module before then" —
   formally fired). Recorded as **ADR-010 / decision D-14**.
2. **Displays are host-rendered scene data.** Guests emit display lists (2D items) / scene
   descriptors (3D) styled by **token ids**; the host renders with the live tokens. Literal
   appearance (raw hex colours, px font sizes, …) is unrepresentable by construction and a
   validation error (`E-LITERAL-APPEARANCE`). This is the mechanism that makes instruments
   **re-theme automatically when the app design changes — zero intervention**, which was the
   operator's hard requirement for the hand-off system. Custom-draw (pixel surfaces) stays
   T1-first-party-only, parked in `LATER.md`.
3. **Hand-off system.** ≤**5-file** `.sparqmod` packages dropped into **`instruments/`** at the
   project root, discovered at launch, validated before load, no engine edits, no operator
   intervention. `MODULE-BUILD-GUIDE.md` (root, v1 draft) is the author-facing contract text.
4. **Full plan edit** (not a side note): SPARQ-PLAN, ADRs, schema, work orders — inventory §2.

**The parallel-build question** (can instruments and the core app be built at the same time,
given instruments need 2D/3D rendering, simulation and data formats?): **YES — conditionally,
and the condition is WO-017's freeze set.** Answer recorded in ADR-010 decision 7 + SPARQ-PLAN
§6.9: nothing may be built in parallel *against a moving contract*; once the freeze set lands
(WIT + token bundle + package spec + display/scene vocabularies + SDK), instrument authors and
the core team work concurrently and integrate **mechanically** (validator + conformance audit +
golden renders + `host_api` pins + the nightly CI matrix), **never merge-based**. Rendering is
safe to parallelise precisely because of ruling 2: guests never touch pixels, so the Phase 6
shell can evolve underneath them. Data formats (`data` ports) are the one honest gap: no native
arbiter exists yet (§6, freeze checklist).

---

## 2. What changed — the complete file inventory

**Session A, 2026-10-03 evening — the plan amendment (all docs, no code):**

* NEW `docs/adr/010-instrument-layer.md` (D-14; decisions 1–8 incl. the parallel-build ruling)
* `docs/adr/002-module-tiers.md` amended (T2 pull-forward, trigger fired) · `docs/adr/README.md`
* `SPARQ-PLAN.md`: §6.1 D-2 amendment · §6.6 (host-rendered displays; custom-draw T1-only) ·
  NEW §6.9 (two-layer library + hand-off system) · §14.5 · §15 · §17 (freeze set → Phase 1,
  instrument host → Phase 2, Phase 5 rescoped) · §20 layout (`instruments/`) · Appendix A
  (`.sparqmod` 5-file cap) · §22 D-14 row
* NEW root `MODULE-BUILD-GUIDE.md` v1 draft (the author-facing guide; fm-terrain walkthrough)
* `PHASE0-WORKORDERS.md`: NEW §3b — **WO-017** (contract v1.1 freeze set, Phase 1),
  **WO-018** (instrument host/loader/validator, Phase 2), **WO-019** (dev harness + reference
  instruments + first hand-off, Phase 2 closing); index rows; build-log entries. Phase 0's exit
  gate is UNTOUCHED — these are pre-ticketed Phase 1–2 work.
* `docs/api/manifest-schema.md`: `classification.layer` · `ui.displays[]` · package note · six
  new error codes (`E-LAYER-MISMATCH`, `E-PACKAGE-FILECOUNT`, `E-LITERAL-APPEARANCE`,
  `E-DISPLAY-PRIMITIVE-UNKNOWN`, `E-CAPABILITY-UNDECLARED`, `E-TOKEN-BUNDLE-VERSION`)
* `docs/api/module-api-v1.md` §10 amendment · `docs/module-author-guide-v0.md` pointer ·
  `design/token-spec.md` (token-bundle output row) · `LATER.md` parking · `README.md` pointer
* NEW `reference/instrument-template/` — the skeleton package (`sparqmod.toml`,
  `instrument.rs`, `example.sparqpatch`, `README.md`)

**Session A→B boundary, 2026-10-03 late / 2026-10-04 08:38–08:42 — WIT skeleton + alignment:**

* NEW `docs/api/instrument-wit/` — package `sparq:instrument@1.1.0`, world `instrument`
  (imports `tokens host assets sources`, exports `guest`), 9 files under `wit/` + README
  (decisions A–H, WIT↔native↔manifest mapping, §5 freeze checklist, §7 verification recipe)
* **v0.2 alignment pass** against `crates/sparq-module-api/src/` as at `1f7d814`:
  `block-status` 4 variants (fuel-trap → `overrun`) · `resources` mirrors native field-for-field
  (`sample-rate: u32`, resolved per-port channel COUNTS; `channel-set` variant REMOVED;
  `fuel-per-block` + `token-bundle` the only tier additions) · `param-set {version, values}`
  replaces the invented `param-value` variant · `event` flattened to native's exact
  `{kind, sample, channel, value, words[4]}` · `cv-in-buf` gains `unconnected` · `cv-out-buf`
  ADDED (v0.1 had no cv output path) · caps cited (`MAX_PORTS_PER_CLASS=8`, `MAX_PARAMS=32`)

**Session B, 2026-10-04 08:48–09:16 — WO-017 artefacts (the interrupted turn's work, reviewed
and verified complete by this handoff):**

* NEW `tools/sparq-module-guest/` — **the guest SDK v0.1 skeleton** (own workspace, `exclude`d
  from the root on the `dispatch-bench` precedent): `wit_bindgen::generate!` embeds the WIT
  **from `../../docs/api/instrument-wit/wit`** (one contract, no copy, cannot drift within a
  checkout) · flat re-exports of every contract type · `InstrumentModule` trait (mirrors the
  native `Module` trait's discipline across the sandbox) · `sparq_instrument!` macro
  (thread_local instance of YOUR type — no `dyn`, no init-order folklore) · helpers
  (`param`, `seed`, `log`, `token_bundle`, stream snapshots) · `#![forbid(unsafe_code)]` ·
  303 lines, honest "what v0.1 is NOT" scope block (not frozen; no display-builder ergonomics;
  no packaging tooling — WO-018)
* NEW `tools/sparq-module-guest/examples/noop-instrument/` — the compile-verified reference:
  silence in the exact negotiated shapes, at-rest empty display, state refused **in words**.
  Builds to wasm (both `wasm32-unknown-unknown` and `wasm32-wasip1` faces were produced).
* NEW `docs/api/instrument-wit/harness/smoke.mjs` — the first host: drives the componentized
  noop through the full lifecycle from JS (jco bindings), 11 checks incl. error-path wording.
* `Cargo.toml` (root): ONE exclude line + comment (`tools/sparq-module-guest`) — stamp-covered.
* `.gitignore`: `/tools/sparq-module-guest/target` (dispatch-bench precedent) — added by this
  handoff session.
* `modules/out/main/sparqmod.toml`: **RECONSTRUCTED** (§3.2) — stamp-covered.

Nothing else moved. `crates/` is **byte-identical to the clone** — no engine code exists in
this round, so PLAY MAKES SOUND, the goldens, and round-4's device obligations are unaffected
except where §4 says (the stamp).

---

## 3. The two incidents, in full

### 3.1 Git history loss + origin went private

The sandbox snapshot layer did not carry `.git` objects across a turn boundary (the working
tree survived; the object store did not — `.git/` was left as an empty skeleton). Recovery by
re-fetch is **impossible from this box**: `github.com/bitseq27/sparq` answered the 2026-10-03
clone publicly, but as of 2026-10-04 ~15:00 anonymous access returns **404** (repo page/API)
and **401** (`info/refs?service=git-upload-pack`) — the classic private-repo signature. (The
project has flipped visibility before; round 4's handoff records the same dance.)

Consequences, all handled:
* Local history rebuilt: root snapshot commit **`9809593`** (author `sparq-agent
  <agent@localhost>` — re-author freely on the device: `git rebase -i --root` or commit fresh
  under your own identity; NOTHING here depends on the sandbox authorship) + one commit adding
  this handoff's doc updates.
* The device repo (`Q:\morphosis\code\sparq`) keeps the canonical history. Apply the overlay
  zip there, review with your normal `git diff`, commit under your own identity, push per your
  `git push.bat` flow if you want the round in GitHub.
* **Next sandbox sessions: commit early, commit often, and `git bundle` to `/home/user` after
  every meaningful commit** — the object store is NOT durable across turn boundaries, and a
  bundle outside the repo survives.

### 3.2 `modules/out/` stripped again — defect #94's class, second occurrence

The workspace snapshot layer excludes directories named `out`. `modules/out/main/` (one file:
the manifest) was silently stripped between turns. The interrupted turn detected it and
**reconstructed the manifest field-for-field** from `docs/modules/out-main.md` (the generated
doc that `tools/module_docs.py --check` proves byte-identical to what the original manifest
renders) + the WO-014 sibling conventions; a reconstruction note is embedded in the file's
header comment. Machine-verified this session: `module_docs --check` **24/24 match**, registry
cross-check and full `cargo test --workspace` green (the manifest is `include_str!`-compiled
into `sparq-audio` — a wrong value would fail loudly).

What is NOT identical: the **comments** (1952 B vs the stamped 4087 B) → the sync stamp's
byte-hash for this file can never match again from this tree (§4). Every *parsed value* is
verified; only prose bytes differ. If the device still has the original file, **prefer the
device's copy** and skip the overlay's version of this one path — then only `Cargo.toml` moves
the stamp.

**Standing rule for future sandbox sessions (write it into the session template):** after any
snapshot/turn boundary, `ls modules/out/main/` FIRST; the strip is silent.

---

## 4. `sync_check` status: FAIL BY DESIGN — device re-stamps

```
sync_check: FAIL - this tree is NOT sync sync-ui-round-2026-10-03 (2 file(s) differ)
  SIZE  Cargo.toml                        disk 4398B, stamp 4187B   <- the exclude line (intentional)
  SIZE  modules/out/main/sparqmod.toml    disk 1952B, stamp 4087B   <- the reconstruction (§3.2)
  warn SIZE  Cargo.lock                   disk 96970B, stamp 96979B <- sandbox regeneration
```

* This is the round-4 pattern ("FAIL BY DESIGN … re-stamp is the device's job"). The pack
  carries **no stamp**; after applying the overlay and reviewing, run
  `python tools/sync_check.py --write` on the device, then `scripts\gates.bat`.
* **`Cargo.lock` is deliberately EXCLUDED from the zip.** It is `.gitignore`d, this round added
  **zero** dependencies to the root workspace (the SDK is an excluded workspace with its own
  lock), and the device's stamped lock (96979 B) remains exactly right. The sandbox's
  9-byte-different regeneration is a cargo-version artefact, not a dependency change. Keeping
  the device's copy makes the lock's `warn` line disappear after re-stamp.
* The gates-digest expectation moves (Cargo.toml + manifest bytes changed): after re-stamp,
  record the new `src N/B` fp the stamp prints. Test counts do NOT move: **899** stands.

---

## 5. Verification matrix — all re-run on THIS box, 2026-10-04

| gate | result |
|---|---|
| `cargo fmt --all --check` | CLEAN |
| `cargo clippy --workspace --all-targets -- -D warnings` | CLEAN |
| `cargo test --workspace` | **899 passed / 0 failed / 1 ignored** (the ignored one is `alloc.rs`'s doc test, standing) — matches the round-4 digest exactly |
| `tools/token_gen.py --check` | PASS — would write 0 files |
| `tools/token_audit.py` | CLEAN — no violations |
| `tools/module_docs.py --check` | **24/24** docs match manifests (this is the reconstruction's machine proof) |
| `tools/unsafe_audit.py` | CLEAN — every `unsafe` allowlisted |
| `tools/check_text_io.py` | CLEAN |
| `tools/sync_check.py` | FAIL BY DESIGN (§4) |
| WIT parse (`npx jco types docs/api/instrument-wit/wit`, official wit-parser via jco 1.35) | CLEAN — `.d.ts` generated for all interfaces + world |
| SDK `cargo test` (host face, in `tools/sparq-module-guest/`) | 3 passed (2 unit + 1 doc-test; 2 doc-tests `ignore`d by design) |
| SDK wasm build | `noop_instrument.wasm` release, `wasm32-unknown-unknown` (49 719 B) + `wasm32-wasip1` face both produced |
| Round-trip smoke (`jco componentize` → JS host → `harness/smoke.mjs`) | **11/11 PASS** — full lifecycle: `id`, `prepare` (incl. the 0-frames refusal **in words**), `process` → `silenced` with exact shapes (1×2ch×64f zeros, cv-out per-block 0.0), `draw` → at-rest empty items, `configure`/`save_state`/`message` refusals in words |
| `sparq ui --audit` | **NOT RE-RUN here** — needs `--features ui-window` (egui/wgpu), not built in this sandbox and not cached. Honest carry-forward: last PASS was round 4 (59 lines, 0 failures); this round changed **no UI code** (only docs, one exclude line, and manifest *comments*), and the audit re-runs on the device via `gates.bat` after apply. |

Round-4 device obligations stand UNCHANGED and are not re-listed here: test006 steps A–U,
test004 attempt 4 (ADR-008 exit gate), defect #95 triage, the dropdown picker (owed).

---

## 6. Where WO-017 stands — the freeze checklist

Settled by the alignment pass (all native-arbiter items; see `docs/api/instrument-wit/README.md`
§5 for the full text): `block-status` · `module-error` · `resources` · `param-set` · `event` ·
`cv-in-buf`/`cv-out-buf` · caps · voices-deferral.

**Still open at freeze (WO-017 close / WO-018 owns):**
1. `data-value`/`data-record` — the one vocabulary with **no native arbiter** (data ports are
   ADR-005-specified, unimplemented). Ratify as-is or reshape when native lands.
2. `time-info`/`transport-state` vs the WO-009 transport as built (tempo/bar/beat
   representation; does `t-wall-ns` belong in the guest world at all?).
3. The six new validation codes wired to exact shapes (WO-018 validator).
4. `gpu_class` → vertex/instance/cell ceiling table (WO-018 renderer side).
5. Two-instance coherence (decision D): `configure` application ordering across audio/display
   instances at the boundary swap (WO-018 loader).
6. `wasmtime` version pin + component-model/WASI baseline (candidate noted during the round:
   wasmtime 49.0.2); token bundle `bundle-encoding` confirmed as JSON-of-`tokens.json`.
7. `docs/api/instrument-wit/` added to the machine-checked API-surface snapshot test (plan
   §6.7) so an accidental WIT edit fails CI.
8. Token bundle v1 generator output from `tools/token_gen.py` + golden snapshot (WO-017 task,
   not yet started).
9. SDK at freeze: vendor a WIT snapshot into the published crate + carry the contract version
   in the crate version (the skeleton deliberately points into the repo instead — fine
   pre-freeze, wrong for publishing; noted in the SDK's own header).

**Next work orders, in order:** WO-017 close (items above) → WO-018 loader/validator design
notes + host-side half of the round-trip test (the guest half now EXISTS — the smoke harness is
its stand-in until the Rust host lands) → WO-019 harness + first real hand-off.

---

## 7. Sandbox environment recipe (rebuilt from nothing, ~25 min; `.cache` MAY not persist)

Reachable from the sandbox: `static.rust-lang.org`, `crates.io`, `static.crates.io`,
`registry.npmjs.org`, `pypi.org`. **BLOCKED: `static.rustup.rs`** (no rustup — use the
standalone tarballs). No system `cc`/`ld` → zig is the linker.

```sh
# 1. Rust 1.99.0 standalone (rustc/cargo/clippy/rustfmt) -> /home/user/.cache/toolchain
#    (downloaded here as .cache/rustdl/rust.tar.gz; canonical URL:)
#    https://static.rust-lang.org/dist/rust-1.99.0-x86_64-unknown-linux-gnu.tar.gz
#    extract, then ./install.sh --prefix=/home/user/.cache/toolchain
# 2. wasm std faces (for the SDK example):
#    https://static.rust-lang.org/dist/rust-std-1.99.0-wasm32-wasip1.tar.gz
#    https://static.rust-lang.org/dist/rust-std-1.99.0-wasm32-unknown-unknown.tar.gz
#    install.sh both with the same --prefix
# 3. linker: pip install ziglang   (zig 0.16.0)
printf '#!/bin/sh\nexec python3 -m ziglang cc "$@"\n' > /home/user/.cache/bin/zigcc && chmod +x /home/user/.cache/bin/zigcc
# 4. per-shell env for every cargo invocation:
export CARGO_HOME=/home/user/.cache/cargo
export PATH=/home/user/.cache/toolchain/bin:/home/user/.cache/bin:$PATH
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/home/user/.cache/bin/zigcc
# 5. WIT tooling: node is preinstalled (v20); in a scratch dir (~/.cache/witcheck):
#    npm i @bytecodealliance/jco@1.35   then
#    npx jco types <repo>/docs/api/instrument-wit/wit -o types        # parse check
#    npx jco componentize noop_instrument.wasm --wit <wit dir> ...    # core wasm -> component
#    npx jco transpile noop-component.wasm -o gen                     # JS bindings
#    node smoke.mjs                                                   # 11/11
# 6. memory: ~1 GB / 2 cores -> always cargo -j 2. ui-window feature (egui/wgpu) does NOT fit
#    comfortably; skip ui --audit in-sandbox and say so honestly (as this round did).
```

Gotchas that bit this round: WIT rejects doc comments on repeated `package` items (package line
first, comments after); `stream` is a WIT keyword (the seed function's parameter is
`stream-name`); snapshot strips `out`/`target`/`.cache`/`.git/config` — bundle early (§3.1).

---

## 8. Git artefacts of this handoff (local, rebuilt history)

```
9809593  Workspace snapshot: instrument-layer planning round (ADR-010/D-14) + WO-017 artefacts
<HEAD>   Session handoff: ROUND5-HANDOFF.md + CHECKLIST head + build-log entry
         (a doc cannot quote the hash of its own commit by construction — `git log` on the
         bundle/tree gives it; everything below refers to the pair, not the digits)
```

Author `sparq-agent <agent@localhost>` — placeholder, re-author at will (§3.1). The bundle in
`/home/user/handoff/` carries exactly these commits. `Cargo.lock` is NOT in git (upstream
`.gitignore`) and NOT in the zip (§4).

**Device apply (suggested, rev-4 pattern):**
1. Extract `sparq-update-2026-10-04.zip` at `Q:\morphosis\code\sparq`, overwriting.
   Optionally keep the device's `modules/out/main/sparqmod.toml` if it still exists (§3.2).
2. `git diff` / `git status` — review the round (everything is listed in §2).
3. `python tools\sync_check.py --write` (re-stamp), then `scripts\gates.bat` (expect 899/24,
   audit PASS; the new `src` fp gets recorded by the re-stamp).
4. Commit under your own identity; push per your flow. The round-4 device rounds (test006 A–U,
   test004 attempt 4) remain owed and untouched by this round.
