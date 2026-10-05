# WO018-STATE.md — state record (fifteenth session, 2026-10-05) — **CLOSED: folded into `ROUND8-HANDOFF.md`**

> **CLOSED 2026-10-05, sixteenth session.** The work this file recorded as "written but NOT
> compiled" was **lost with its sandbox** before reaching origin (the loss and its evidence:
> `ROUND8-HANDOFF.md` §0). It was then **REBUILT from this file + `ROUND7-HANDOFF.md`** on a
> clean clone — the inventory table below is the checklist the rebuild was verified against,
> every row present — and **compiled, tested and gated for real**: 925/0/1-ignored, fmt+clippy
> clean, python gates clean, MSVC default cells clean, committed `450c38b…51b25aa` + records.
> The desk-audit risk list below stayed useful: risk 1 (borrow shapes) and the `r.len()`-after-move
> shape it cousins were the only real compile failures; risks 2–7 passed as written. The
> wasmtime probe was re-run and MEASURED (`docs/instrument-host.md` §8): the dependency tree is
> green to `cranelift-codegen`, which OOMs on a 1.06 GB host — memory, categorically. **Keep
> this file as the rebuild's source record; the live state is `ROUND8-HANDOFF.md`.**

---

**Why this file exists:** mid-round, the sandbox HOST hit sustained thread-table exhaustion
(`/proc/loadavg` pinned at `1/1129` against `kernel.threads-max = 1129`; the exhaustion is
OUTSIDE this container's PID namespace — our own namespace holds ~20 threads). Every external
process spawn became a lottery: `rustc` ICEs with `__clone` backtraces, zig's LLD aborts with
`thread constructor failed: Resource temporarily unavailable`, and most tool calls die at
`spawn /bin/bash EAGAIN`. This is the same pressure that killed the wasmtime probe build
earlier in the round (recorded in `docs/instrument-host.md` §8). **Nothing below is compiled
yet.** The code is written and desk-audited; the compiler has not seen it. House rule: no
commit is presented as green until the gates ran — so the tree is UNCOMMITTED on purpose.

## What is on disk (all of it uncommitted, on top of `15492fe` = the WO-017-close head)

**The zero-dependency half of WO-018, complete as text:**

| path | what |
|---|---|
| `Cargo.toml` (root) | `sparq-host-wasm` member; `wasmtime = "=49.0.2"` workspace dep (default-features off: std, runtime, cranelift, component-model), exact-pin rationale comment |
| `crates/sparq-host-wasm/Cargo.toml` | the crate: dep = sparq-module-api (+ optional wasmtime); feature `instrument-host` |
| `crates/sparq-host-wasm/src/lib.rs` | crate doc + module decls + re-exports |
| `crates/sparq-host-wasm/src/ceilings.rs` | gpu_class → vertex/instance/cell ceiling table (freeze debt #1 CLOSED as data; anchors + movement rule in the header; 2 tests) |
| `crates/sparq-host-wasm/src/package.rs` | the five-file model: Inventory/roles/cap (E-PACKAGE-FILECOUNT)/launch-vs-strict profiles/is_loadable/discover (5 tests) |
| `crates/sparq-host-wasm/src/validate.rs` | the §7 chain runner: stages 1–2 + budgets-static + signature run; 3/4/6 and budgets-measurement REFUSE IN WORDS (`Verdict::Refused`, `complete()` flag, PARTIAL verdict); capability-honour static rules; scene↔gpu_class cross-check; data-port routing refusal; host_api pin check (`HOST_MODULE_API = 1`) |
| `crates/sparq-host-wasm/tests/package_gate.rs` | the throwaway-package test (the WO-007 pattern extended): legal package passes/refuses correctly; the acceptance breakages (sixth file, bad manifest, backbone-in-instruments/, data port, network=full, scene-without-budget, hostile host_api, empty wasm); discovery independence; launch-profile loadability; CLI-independent (5 test fns) |
| `crates/sparq-module-api/src/error.rs` | +5 catalogue codes (E-PACKAGE-FILECOUNT, E-LITERAL-APPEARANCE, E-DISPLAY-PRIMITIVE-UNKNOWN, E-CAPABILITY-UNDECLARED, E-TOKEN-BUNDLE-VERSION); ALL 25→30; count test updated |
| `crates/sparq-module-api/src/manifest.rs` | `GPU_CLASSES` const; gpu_class↔field-table drift-pin test |
| `crates/sparq-module-api/src/decode.rs` | `check_gpu_class` (domain check at decode, the displays precedent — NOT modelled into ResourceDecl); +1 test |
| `crates/sparq-module-api/tests/api_snapshot.rs` | GPU_CLASSES value pin |
| `docs/api/manifest-fields.toml` | gpu_class domain row (none/light/medium/heavy/very_heavy) |
| `crates/sparq-app/src/instruments.rs` | `sparq mod validate DIR` + `sparq mod list [--root] [--strict]` (+2 parse-test fns) |
| `crates/sparq-app/src/{main.rs,cli.rs,modules.rs}` | mod decl; USAGE lines; dispatch arm; §11 precedence line updated |
| `crates/sparq-app/Cargo.toml` | sparq-host-wasm dep + `instrument-host` passthrough feature |
| `instruments/README.md` | the layout artefact (empty by design until WO-019) |
| `docs/instrument-host.md` | the WO-018 design notes: loader architecture, fuel/epoch↔watchdog mapping, the TWO-INSTANCE ORDERING (freeze debt #2 CLOSED as spec), ceiling rationale, code-wiring matrix, discovery, the build-blockage record + retry recipe, the runtime increment's test plan |
| `.github/workflows/ci.yml` | feature-on Linux cell (clippy+test) + `sparq mod list --strict` smoke + MSVC crosscheck clippy for the feature |
| `PHASE0-WORKORDERS.md` | WO-018 status line: IN PROGRESS, zero-dep half written, runtime half blocked-on-environment |

## Resume recipe (first thing, in a healthy environment)

```sh
export CARGO_HOME=/home/user/.cache/cargo
export PATH=/home/user/.cache/toolchain/bin:/home/user/.cache/bin:$PATH
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/home/user/.cache/bin/zigcc-direct
cd /home/user/sparq
cargo fmt --all                                    # the blind-written code needs its first fmt
cargo test -p sparq-module-api -p sparq-host-wasm -j 2
cargo test --workspace -j 2                        # expect 909 + ~15 new; 0 failed
cargo clippy --workspace --all-targets -j 2 -- -D warnings
python3 tools/sync_check.py                        # FAIL by design; note the file list
```

If the environment is a FRESH sandbox: rebuild it per ROUND5-HANDOFF §7 first (~25 min), and
note `.cache` (toolchain, cargo registry incl. the fetched wasmtime tree) may not have
survived; `.git` survival is the defect-class risk from ROUND5 §3.1 — if the object store is
gone, the working tree still holds everything (this round made NO commits) and the round-6
bundle `/home/user/handoff/sparq-handoff-2026-10-05.bundle` carries history through `15492fe`.

## Desk-audit risk points (what to look at first if it does not compile)

1. `validate.rs` — the borrow shapes around `parsed.as_ref()` chains (rewritten once already to
   avoid assuming `Value: Clone`).
2. `package.rs::is_loadable` — matches on `e.kind`/`e.path` (public fields per house precedent).
3. `decode.rs::check_gpu_class` — `Value::Str` pattern against `Option<&Value>` (binding modes).
4. `instruments.rs` — the local `let run = …` shadowing the fn name `run` (legal, but look here
   if the compiler disagrees about something in that fn).
5. `error.rs` — `ALL` array length 30 must match the variant count exactly (const-eval will
   scream if not).
6. Clippy denies `unwrap_used`/`expect_used` workspace-wide: src code was written without them;
   test modules carry the house `#![allow(...)]` headers.
7. `Cargo.toml` wasmtime features (`std`,`runtime`,`cranelift`,`component-model`) resolved fine
   during the probe (the dependency tree fetched and began compiling before the OOM/thread
   kills), so metadata is valid even for feature-off builds.

## What is NOT done (beyond the runtime half)

* Nothing is committed; no bundle of this round exists yet.
* CHECKLIST.md head + build-log entry + the round handoff (ROUND7) await the measured numbers —
  they must not be written with invented ones.
* The wasmtime half (`runtime.rs`), the launch-loading wiring, the browser's visual grouping,
  hot reload: all per `docs/instrument-host.md` §2–§4 designs, awaiting a build environment.
