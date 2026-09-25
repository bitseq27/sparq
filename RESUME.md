# RESUME.md — where the work stands, and how to pick it up

**As of 2026-09-22, end of session.** Tree = **WO-007 tasks 1–3 built** on top of WO-012 increment 1
/ WO-006 increment 1.1g. **5 crates, 388 tests passing (+1 ignored), re-measured.** Build stamp
**`src 68f/1070593B`** (was `57f/853914B` before WO-007; +8 `.rs`, all in the new crate) — recomputed by
hand from `build.rs`'s own five roots, so SATURN's guard will force exactly one rebuild, which is
defect #49's remedy working rather than a failure. New since the restore: `WO007-TASK1-REVIEW.md`,
`docs/api/compat-matrix.toml`, `docs/api/manifest-fields.toml`, `tools/dispatch-bench/`, and
**`crates/sparq-module-api/`**. Amended: `manifest-schema.md`, `module-api-v1.md`, ADR-005, ADR-009,
`SPARQ-PLAN.md`, `PHASE0-WORKORDERS.md`, `README.md`, `tools/README.md`, `Cargo.toml`, `.gitignore`,
`justfile`, `.github/workflows/ci.yml`; defects #54–#65 logged; **WO-007 complete**, shipped as `../sync-wo007-complete.zip` (42 entries, verified). Task 1 shipped as
**`../sync-wo007-task1.zip`** (19 files) — **that zip predates the crate**, so a new increment is
needed before SATURN can build any of this.

Scratch/session doc — like `wo006-progress.md`, **not part of any sync zip**, not in README's
reading order. Fold into the build log or delete when it stops earning its place.

---

## 1. Why this file exists

The last session ended on a model failure, not on the work: history had grown to **5 MB / 1134
messages** and the final request was rejected six times with `400 InvalidParameter: Range of input
length should be [1, 983616]`. The question *"what is next on the plan"* was never answered.

**So do not load the big documents whole.** `SPARQ-PLAN.md` is 1102 lines, `PHASE0-WORKORDERS.md`
825; reading either end-to-end is what filled that context. Read by range
(`sed -n '506,536p' PHASE0-WORKORDERS.md`):

| Need | Read |
|---|---|
| Status of every WO + deviations | `PHASE0-WORKORDERS.md` §2.1, 61–347 |
| **WO-007 (next)** · WO-008 · WO-012 · WO-013 | 506–536 · 537–563 · 614–637 · 638–655 |
| Exit gate · schedule + cut list · risk watchlist | 726–752 · 791–809 · 811–825 |
| The module contract | `SPARQ-PLAN.md` §6, 297–397 |
| Module backlog to paper-check against it | `SPARQ-PLAN.md` Appendix B, 1037–1048 |
| Port types · contract draft | `docs/adr/005-port-type-system.md` (53 lines) · `docs/api/module-api-v1.md` (262 — cheap, read whole) |

Restored from snapshot `28f053ba…` (session `18d3f5a6`, touched 2026-09-22 17:03 CST). Diffed
against the older `35d3417a…`: the only entries not carried forward were `/core` (stray core dump,
banned by `.gitignore`) and `kernel/src/rt.rs` (refactored into `rt/mod.rs` + `rt/thread.rs`).
Nothing lost.

## 2. Standing constraint

The user is **remote on SATURN from an iPad until they say otherwise** (stated 2026-09-22). Over
RDP: no real-time measurement, exclusive mode honestly refused, the window renders via WARP, and
`IAudioClock` drift is untrustworthy (already `SUSPECT`-labelled, defect #45). §4 is therefore
**parked, not skipped** — sandbox work continues regardless.

## 3. Ledger — last *measured* state

280 tests green · clippy clean in **six** cells (default / bootstrap / ui / ui-window-gles native /
MSVC+hal-wasapi / MSVC+ui-window) · fmt clean · 4 Python gates · selftest **8/8** `--golden` ·
goldens unchanged (`ba577186c988db21`, `dd975a24f03b19c1`) · `sparq ui --audit` **PASS, 0 failures**
(40 matrix cells + DPI invariance + 9 gesture smokes) · headless frame med 94 µs / p99 113 µs ·
`probe_alloc` 0 · reopen-leak **0 outstanding on hardware** (#48 closed on the machine that found
it).

**Re-verified in the sandbox on 2026-09-22**, end of session: fmt clean · **388 passed, 0 failed,
1 ignored** · clippy clean in **seven** cells (default workspace, `bootstrap-audio`, `ui`, native
`ui-window`/gles, and MSVC × 3 including the new crate) · 4 Python gates clean · the clippy gate
**proven failable** by injecting a violation and reverting byte-identical · **0 allocations across
15 000 `process` calls** through `Box<dyn Module>`. **Re-measured at the end of the session:** `selftest --golden` **PASS (8 gates)** with golden **`ba577186c988db21` matching**, both release golden tests passing, `ui --audit` **PASS (0 failures)**, determinism bit-identical (`0f5c3e86c7f117a9`), 0 allocations with the allocation gate proven live, **315× realtime** for 1 min at 96 kHz/64, HAL null conformance 9 checks with reopen-leak 0 outstanding. `ui --audit` needed relaxed sandbox codegen (`lto=false`, `codegen-units=16`) because `read-fonts` is OOM-killed at 1 GB under the repo profile; SATURN builds with the repo profile. **Still never run anywhere:** the MSVC `ui-window` clippy cell.

**The 279-vs-280 discrepancy is CLOSED (measured 2026-09-22).** The tree holds 288 `#[test]`
attributes, 9 of them in `hal/wasapi.rs` behind `cfg(all(windows, feature = "hal-wasapi"))` ⇒ 279
compiled on Linux. `cargo test --workspace` discovers **281**: those 279 plus **2 doctests** in
`sparq-kernel`, one ignored (`crates/sparq-kernel/src/alloc.rs` line 10). 281 − 1 = **280 passed**,
exactly the ledger's number. Per binary: audio 172 unit + 12 integration, kernel 58, ui 37, **app 0**.
That exact match is the evidence the restored snapshot is the tree that produced the ledger. With
`sparq-module-api` added the total is **388** = those 280 plus 96 unit tests (46 contract types + 20 parser + 10 decoder) and 12 contract tests;
the new crate's 9 windows-gated equivalents are zero, since it has no platform code.

**Sandbox reality.** The toolchain is wiped **without warning — it went missing mid-session on
2026-09-22**, not only between sessions, because `/opt` is not part of the persisted snapshot. So
does `target/`: a wipe costs a full dependency rebuild as well as the reinstall. It was reinstalled twice on 2026-09-22
(cargo/rustc 1.98.1; ~20 s with a warm apt cache, **4.5 min cold**) into **`/opt/rustup` + `/opt/cargo` — deliberately outside `/home/user`**, so
~1 GB of toolchain cannot crowd the repo out of the 128 MB / 10 k-file workspace snapshot. It will
therefore be gone again next session. To restore (~20 s with a warm apt cache, several minutes
cold):

```bash
apt-get install -y curl ca-certificates gcc libasound2-dev pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal \
     --default-toolchain stable --component rustfmt,clippy
export RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:$PATH
rustup target add x86_64-pc-windows-msvc   # lints the Windows HAL blind; does not link
```

`just` was never installed and isn't needed — every recipe is a plain command; `gates` = fmt-check →
clippy → test → python-gates → golden → golden-demo → selftest → ui-audit. **1 GB RAM is the binding
constraint**: wgpu's `ash`/Vulkan and release-LTO `read_fonts` both OOM, hence per-target backends
and relaxed sandbox codegen. SATURN builds with the repo profile. **Background processes do not survive between tool
calls** (`setsid nohup` included — the log stops and the process is gone), so run long builds
synchronously with a generous timeout. And **avoid the literal workspace path inside Python source**:
the sandbox rewrites it, so use relative paths with `cwd` set.


## 4. Blocked on the physical machine

| Track | Waiting for |
|---|---|
| **WO-006 acceptance** | 2 h zero-xrun soak at 96 kHz/64 exclusive · latency table · unplug test · UMC204HD multichannel verdict. Then increment 2: ASIO, duplex, round-trip measurement, STA retry for the `E_ACCESSDENIED` property store (#40) |
| **WO-012 device evidence** | DPI matrix — taps at 100/125/150/200 % (`docs/ui/windows-dpi-notes.md` §4) · real-finger gestures · palm rejection (needs **WM_POINTER contact area**; winit reports none → increment 2) · 60 fps with a rasteriser · mixed-DPI dual monitor |
| **WO-001** | The sheets in `docs/hardware/stage-baseline.md`. Everything measured so far is Linux/x86-64 and **does not transfer to Windows** |

## 5. Next step: **WO-008 — graph and executor** (WO-007 is complete)

**All five tasks are done and all six acceptance criteria are addressed** — criterion 3 only as far as
Rust allows, and the build log says so rather than rounding it up to "impossible". Four increments:
1 the contract as data · 2 the crate, three reference modules and the TOML reader · 3 the registry,
discovery and task 5's two gates · 4 the author guide and `sparq modules`. **388 tests**, stamp
`src 68f/1070593B`, shipped as `../sync-wo007-complete.zip`.

**WO-008 — graph and executor** is next on the critical path, and it does **not** need the stage
machine: its dependency on WO-006 is on the HAL's *shape*, which the conformance suite froze, and the
null backend exists precisely so every layer above the HAL stays testable in CI. It inherits decisions
rather than re-making them — the channel-set cascade rule with its named test
(`channel_set_change_reprepares_neighbours_without_bumping_graph_version`), the dispatch hybrid
(block-level trait objects, never per-sample), and the buffer pooling that `AudioCtx` currently stubs
as one interleaved input and one output.

**One thing WO-007 left open on purpose, for WO-008 to pick up:** `connect_audio` / `connect_cv` /
`connect_cross` in `port.rs` *mirror* `docs/api/compat-matrix.toml`. Now that a TOML parser exists they
can read it at discovery instead, deleting the only copy of the matrix that can drift. Until then the
mirror is documented as temporary in both places.

**Not tested as well as it should be:** `crates/sparq-app/src/modules.rs` has no unit tests — the scan
is covered by a CLI smoke run against a scratch tree, which is weaker than a gate and is recorded as
such. And `sparq-module-api` has no golden render of its own; its determinism claims rest on
`syn/sine`'s phase test and the contract crate's bit-identical double render.

## 6. Loose ends and doc debt

- **Defect #56 is CLOSED (2026-09-22):** the module is **`ana/tap`**. §17 said `ana/scope-tap` while
  Appendix B and WO-014 said `ana/tap`; §17 is corrected and all three lists agree. Chosen because two
  of three documents already said it, it matches `ana/rms` beside it, and "scope" in the name would
  promise something narrower than the module is — it is a generic tap for all twelve display modules.
- ~~`README.md` said "206 tests passing"~~ — **fixed**: status and code lines de-staled (280 tests,
  six clippy cells), the contract tables added to the doc index, and the release-profile figures
  explicitly dated as *not re-measured* instead of left looking current.
- ~~`SYNC.md` said "Files in this zip (35)" while holding 36~~ — **superseded**: `SYNC.md` was
  rewritten for this increment and its count is now checked programmatically against the zip's
  namelist (heading = list = contents = 19), so that off-by-one cannot recur silently.
- **WO-002**: both typefaces still `chosen = ""`; the shell runs on egui's default face as a result.
- **WO-004**: human protocols unrun (blind identity, distance, dark-room, glove, monochrome) plus
  the display-sheet tablet/wall breakpoints.
- **ADR-009** still `draft` — ratify with the WO-006/WO-008 measurements.
- **WO-005**: `play` has never produced sound in the sandbox (no device). The bootstrap cpal path is
  deleted only at the **end** of WO-006, after hardware acceptance.
- WO-012 increment 2+ parking list: `LATER.md` §"WO-012 increment 2+".

## 7. Sync bundles

One level **above** the repo. **Current: `sync-wo007-task1.zip` (19 files)** — documents, the two
data tables and the dispatch harness; no Rust, so the stamp is unchanged. Superseded:
`sync-wo012-inc1.zip` (36) and `sync-wo006-inc11g.zip` (29). All extract at the repo root,
`Q:\morphosis\code\sparq`, overwriting, each applying on top of the previous.

For this increment SATURN needs only `scripts\gates.bat` (should behave exactly as in 1.1g) **plus**
the one new thing the harness ships for:

```bat
cd tools\dispatch-bench
cargo run --release
```

Wanted back: the `C. VERDICT` block and the two `D. PER-CALL COST` lines — the numbers ADR-009
executor decision 7 cites, measured here on a 2-core virtualised sandbox. The earlier asks are
unchanged: `logs\ui.log` after a window session, the DPI matrix if reachable, and the WO-006
acceptance runs (2 h soak, latency table, unplug, UMC204HD).
