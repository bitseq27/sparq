# Sync manifest — WO-007 complete: the module contract, built

Applies on top of the **WO-012 increment 1** tree and supersedes `sync-wo007-task1.zip`, which carried
only task 1's documents. Extract at the repo root, same as before.

**The build stamp changes: `src 68f/1070593B`** (was `57f/853914B`). `build.bat`'s guard will report a
mismatch against the 1.1g binary and force a rebuild — that is defect #49's remedy working, not a
failure. `build.rs` now walks **five** `src` roots because a fifth crate exists; defect #60 was that it
walked four, and would have stamped a binary current while ignoring the new crate entirely.

## What this is

**WO-007 is complete**: five tasks, six acceptance criteria addressed, three increments.

1. **The contract as data (task 1).** `docs/api/compat-matrix.toml` (6 port types, 21 same-type cases,
   5 verdicts, 4 adapters, **one cell still open**: G7, the `gpu` tier question, deferred to Phase 5
   with an interim rule) and `docs/api/manifest-fields.toml` (82 field rows, 22 required, an error code
   per violation). Ten decisions ratified, in `WO007-TASK1-REVIEW.md` and in the documents they change.
   The matrix is still *mirrored* in `port.rs` rather than read at runtime — that mirror is the one
   remaining copy that can drift, and deleting it is the next thing worth doing.
2. **The contract as code (tasks 2, 3, 5).** `crates/sparq-module-api`: the closed vocabulary as types,
   the connection rules as functions, the derived error catalogue, manifest validation returning
   *every* failure at once, `Copy` param snapshots over the kernel's lock-free ring, the
   dyn-compatible `Module` trait, a **dependency-free TOML subset parser**, a registry that refuses a
   module which disagrees about its own id, and discovery that takes text. Three conforming modules as
   contract tests: `util/gain`, `syn/sine`, `ana/rms`.
3. **The author guide (task 4).** `docs/module-author-guide-v0.md`, written from those three modules
   rather than from the spec.
4. **A log comparator.** `tools/log_check.py` reads a device log directory and diffs every number the
   gates assert on against the measured sandbox baseline, with HARD/SOFT/DECISION verdicts and an
   encoding ladder for whatever Windows wrote. `--self-test` proves it without logs.
5. **Discovery on the app side.** `sparq modules [--root DIR] [--strict]` walks a directory, reads
   every `sparqmod.toml`, and prints what loaded, what was refused and why (verbatim, with the fix),
   and what shadows what. `--strict` exits non-zero if anything failed, so CI can gate on a clean
   module set. The contract crate still performs **no I/O at all**; this command is the only place a
   directory is read.

**Acceptance criteria:** (1) zero engine edits — `tests/throwaway_module.rs` adds a module that exists
nowhere else, then walks every `.rs` under every `crates/*/src` asserting none of them mentions it ·
(2) all five named rejections, from real TOML text · (3) met **as far as Rust allows**, and the log
says so: no capability offered, no allocation measured, but `Vec::new()` cannot be forbidden by types
alone · (4) the param swap is atomic at the boundary and a mid-block change is provably deferred ·
(5) the API snapshot — ~90 fn-pointer pins, proven failable by changing `Origin::precedence` to `u16`,
which compiles everywhere else and fails only the pin · (6) dispatch measured.

## Sandbox verification (measured, not asserted)

**388 tests** (was 280 before WO-007) · fmt clean · clippy clean (`--workspace --all-targets`, MSVC
`sparq-module-api`, the `ui` cell) · 4 Python gates clean · **0 allocations across 15 000 `process`
calls through `Box<dyn Module>`**, with the kernel's counting allocator installed as the test binary's
global allocator · three gates **proven failable** and reverted byte-identical (sha256 before and
after) · `sparq modules` exercised end to end against a scratch tree: 3 manifests found, 2 loaded,
1 refused with an actionable message, `--strict` exit 1.

**Re-measured at the end of the session, so nothing here is quoted any more:** `selftest --golden`
**PASS (8 gates)** with golden **`ba577186c988db21` matching** · both release golden tests pass (the
WO-005 chain and the Phase B demo, each with its within-run reproducibility check) · `ui --audit`
**PASS, 0 failures** (the full viewport × DPI × mode matrix, DPI-invariance, all nine gesture smokes) ·
determinism bit-identical across two renders (`0f5c3e86c7f117a9`) · **0 allocations** on the audio path
with the allocation gate proven live · **315× realtime** for one minute at 96 kHz/64 · HAL null
conformance 9 checks, reopen-leak 8 cycles with 0 outstanding. One caveat stated rather than buried:
`ui --audit` had to be built with relaxed codegen in the sandbox (`lto=false`, `codegen-units=16`)
because `read-fonts` is OOM-killed at 1 GB under the repo's release profile — SATURN builds with the
repo profile, so a faster build there is not a difference in the code. **Still never run anywhere:**
the MSVC `ui-window` clippy cell, which P1 of the SATURN run sheet supersedes for real.

## What to run on SATURN

```bat
scripts\build.bat          (forces a rebuild: the stamp changed)
scripts\gates.bat          (388 tests now; nothing in the audio path changed)
sparq modules --root modules --strict    (harmless if modules\ does not exist yet)
```

and the measurement ADR-009 executor decision 7 asks for:

```bat
cd tools\dispatch-bench
cargo run --release
```

**What I need back:** the `C. VERDICT` block and the two `D. PER-CALL COST` lines. The sandbox numbers
are a 2-core virtualised Linux box — the ratios should transfer, the absolute nanoseconds will not.
Plus `gates.log` only if gates fail, and unchanged from earlier increments: the WO-006 acceptance runs
(2 h soak at 96 kHz/64 exclusive, latency table, unplug test, UMC204HD verdict) and the WO-012 DPI
matrix, none of which can be done over RDP.

## Files in this zip (43)

```
EXTRACT-AT-REPO-ROOT.txt                      read me first
.github/workflows/ci.yml                        
.gitignore                                      
Cargo.lock                                      
Cargo.toml                                      fifth member + workspace dep + the dispatch-bench exclude
PHASE0-WORKORDERS.md                            WO-007 status row + build-log entries for three increments, defects #54-#65
README.md                                       de-staled: 5 crates, 388 tests, the contract tables indexed
SPARQ-PLAN.md                                   Appendix B AND §17 Phase 1: 12 -> 15 modules
SYNC.md                                         this manifest
WO007-TASK1-REVIEW.md                           NEW the ten ratified decisions and their consequences
crates/sparq-app/Cargo.toml                     depends on sparq-module-api
crates/sparq-app/build.rs                       fingerprint walks FIVE src roots now (defect #60: it walked four)
crates/sparq-app/src/cli.rs                     the `modules` command + USAGE
crates/sparq-app/src/main.rs                    mod modules
crates/sparq-app/src/modules.rs                 NEW `sparq modules`: the only place a directory is read
crates/sparq-module-api/Cargo.toml              
crates/sparq-module-api/src/decode.rs           NEW text -> ValidatedManifest; unmodelled sections still key-checked
crates/sparq-module-api/src/discovery.rs        NEW precedence per module-api §11; shadowing reported, not resolved quietly
crates/sparq-module-api/src/error.rs            NEW 19 legacy codes + the 5 derived ones (defect #55)
crates/sparq-module-api/src/lib.rs              NEW crate: the contract, its v0 limits stated up front
crates/sparq-module-api/src/manifest.rs         NEW every required field an Option, so absence is reportable
crates/sparq-module-api/src/module.rs           NEW the Module trait; AudioCtx offers no allocator, clock, fs or lock
crates/sparq-module-api/src/params.rs           NEW Copy snapshots over the kernel's lock-free ring
crates/sparq-module-api/src/port.rs             NEW the closed vocabulary + the connection rules as functions
crates/sparq-module-api/src/registry.rs         NEW refuses a module that disagrees about its own id; never latest-wins
crates/sparq-module-api/src/toml.rs             NEW dependency-free TOML subset parser (defects #62-#63 were here)
crates/sparq-module-api/tests/api_snapshot.rs   NEW task 5: ~90 fn-pointer pins, proven failable
crates/sparq-module-api/tests/contract.rs       NEW util/gain, syn/sine, ana/rms + the 0-allocation measurement
crates/sparq-module-api/tests/throwaway_module.rs 
docs/adr/005-port-type-system.md                addendum: dispatch, the cv range rule, five closed cells
docs/adr/009-executor-and-hal.md                executor decision 7 now holds the measurement it asked for
docs/api/compat-matrix.toml                     NEW what may connect to what, as data; one cell still open (G7)
docs/api/manifest-fields.toml                   NEW every field, its domain, its error code
docs/api/manifest-schema.md                     internal_rate DELETED · cv_reduce/cv_interp added · derived codes
docs/api/module-api-v1.md                       §2 cascade rule · §3 the two new port fields · §14 · §16 all five answered
docs/module-author-guide-v0.md                  NEW task 4, written from the reference modules
justfile                                        
tools/README.md                                 
tools/log_check.py                              NEW reads a device log set and diffs it against the measured sandbox baseline
tools/dispatch-bench/Cargo.lock                 
tools/dispatch-bench/Cargo.toml                 
tools/dispatch-bench/RESULT.md                  
tools/dispatch-bench/src/main.rs                NEW the harness ADR-009 decision 7 asked for; re-run it on SATURN
```

**Deliberately not in this zip:** `RESUME.md` — a sandbox session document, like `wo006-progress.md`,
and marked as such in its own header.

## The usual discipline

* Extract at the repo root (`Q:\morphosis\code\sparq`), overwriting.
* The stamp guard should read **`src 68f/1070593B`** on both sides afterwards.
* **Defect #56 is CLOSED — the module is `ana/tap`.** §17 Phase 1 said `ana/scope-tap` while Appendix B
  and WO-014 said `ana/tap`; §17 is corrected in this zip, so all three lists agree. Nothing to do on
  SATURN — it is recorded here only so the change in §17 is not a surprise.
* Defects logged this work order: **#54–#65** in `PHASE0-WORKORDERS.md` §2.1. Two of them, #59 and
  #61, were this author's own and are recorded as such.
