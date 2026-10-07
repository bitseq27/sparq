# WO020-STATE.md — state record (The Observatory, WO-020) — **INC1–INC4 CLOSED; INC5 slices 1–2 CLOSED in the sandbox, device run + INC5b outstanding**

**Last touched:** 2026-10-07, the INC5 session (fresh sandbox on the re-published main `a9dbf31`;
branch `wo020-inc5`) — **slice 1** landed the stream plane's live half (`streams-net`: fetch.rs,
the broker, `BrokerProvider`, the real CLI verbs — ALL MEASURED here, incl. a 22/23 live probe);
**slice 2** wrote the instrument runtime (`sparq-host-wasm::runtime`: the wasmtime loader, the four
doors, the gate's runtime stages, the noop fixture rebuilt + sha-pinned, `scripts/test007.bat`) —
device-first-compile by the §8 memory verdict, re-probed and RE-CONFIRMED this session (winch
alone still pulls cranelift). See the INC5 section below. **Previous touch:** 2026-10-06, delivery session (same sandbox, after the INC4 seal) — **INC1–INC4
PACKED FOR THE DEVICE**: `sparq-update-2026-10-06-wo020-inc4.zip` + `WO020-INC4-RUN-SHEET.md` +
the closing `handoff/sparq-wo020-delivery.bundle` (see **Delivery** below). **Previous touch:**
the INC4 session — **INC4 CLOSED**:
the host painters over INC3's IR, the dropdown picker (round-4 OWED item 11 discharged), camera
FOCUS, the D6 wall-card sizing, the browser's instrument grouping, the at-rest wall in the shell,
`sparq instrument render`. Plan of record: `WO020-OBSERVATORY-PLAN.md`. Remaining: **INC5 is
device-only by construction** (the run sheet's §4), INC6 is deferred/gated.

> **Environment note (survives a re-read):** the sandbox snapshot does NOT persist the installed
> toolchain or `.git`. This session re-installed rustup (via python — no curl/wget) + `gcc` (apt),
> and **recovered `.git` by re-cloning origin (which still serves the base `48d3074`) then fetching
> `handoff/sparq-wo020-inc1.bundle`** — the bundle discipline is what made the INC1 history
> recoverable after the snapshot dropped `.git` (and one working-tree file, `modules/out/main/`,
> restored from HEAD). The resume recipe below re-installs both. **Commit + bundle every increment:**
> the bundle in `handoff/` is the durable artefact, not `.git`.

---

## Delivery — 2026-10-06 (handoff prep: the pack, the run sheet, the closing bundle)

Operator instruction: **"prep handoff and sync zip."** The device delivery for INC1–INC4, in the
r8/rev-4 house shape:

| artefact | what |
|---|---|
| `sparq-update-2026-10-06-wo020-inc4.zip` (workspace root, one level above the repo; copy committed under `handoff/`) | **FULL-TREE pack, 471 entries** (defect #94's remedy: overlays any tree at or after the r8 pack / git `96fe57d`). Exclusions by the standing rule: `.git/`, `target/`, `Cargo.lock`, `logs/`, `*.wav`, `handoff/`, and **`SYNC-STAMP.txt` — the stamp does not ride and did not move from the sandbox** (MUST-NOT-MOVE honoured; the device re-stamps `--sync sparq-wo020-inc4-2026-10-06`, run sheet §1 step 2). sha256 + size: `handoff/sha256sums-wo020.txt` + the delivery message. |
| `WO020-INC4-RUN-SHEET.md` (ships inside the pack) | the operator's steps: apply → re-stamp → `gates.bat` (the ONE expected red: `design conformance` on the pre-existing §18 mockup set, 14 groups / 298 occurrences — on main before this pack) → the WO-020 device column (streams 23, guest tests + the budget block **exact-match** — deterministic replay, `mod validate` PARTIAL by design, the at-rest render via the `SPARQ_ATREST` env door, the shell visual row, the audit) → send-back list → what INC5 is. Both honest refusals are stated up front: validate's stages 3/4/6 and **PLAY #58** (until INC5's loader). |
| `handoff/make_pack_wo020.py` | the packer, checked in (the `make_pack.py` precedent): a **self-verifying pre-pack gate** — every entry's zipped bytes re-read and hashed against the working tree, the tree required clean vs HEAD, entry/namelist counts asserted against the numbers printed in the run sheet + SYNC.md (471 / 95 / 85 — the refusal it fired when the first artefact commit moved the namelist under the documents is the gate working, recorded in the delivery message), the MUST-NOT-MOVE surfaces (WIT, `modules/`, the stamp, `reference/fixtures`, the goldens) diffed vs base, and the exclusion rules asserted absent. It refuses to write a pack whose documents would lie about it — LATER.md's pre-pack-gate ideal, discharged for this delivery. |
| `handoff/sparq-wo020-delivery.bundle` (+ `.sha256`) | the closing bundle `96fe57d..HEAD` at the delivery seal — one fetch restores the whole WO-020 branch (INC1–INC4 + the delivery documents) into any clone of the base; supersedes the per-increment bundles for recovery (they stay committed as the increment record). |
| `SYNC.md` | new top entry (this pack: provenance, gates, the 90-path namelist vs `96fe57d`, THE ASK); rev 4 demoted to "Previous bundle" per the house pattern. |

**Windows friction found by the delivery prep (recorded, not papered over; `LATER.md` INC5 doors):**
the two at-rest cache-dir defaults disagree on Windows — the harness's `atrest_path()` writes
`%USERPROFILE%\.cache\sparq\at-rest\` while the app's `atrest_dir()` reads `%APPDATA%\sparq\at-rest\`
(the app grew the `cfg!(windows)` branch, the harness did not). Both sides honour `SPARQ_ATREST`
first, so the run sheet uses the env door; INC5 aligns the harness. Same section: the guest
workspace's `Cargo.lock` is untracked by the house `.gitignore`, so a device-side component REBUILD
resolves `wit-bindgen` unpinned — the shipped component is byte-pinned in the pack regardless.

**Branch state at the delivery seal:** `wo020-observatory`, base main `96fe57d` (= `origin/main`);
16 increment commits (`cef2385`..`44d623b`) + the delivery commits (the documents at `097a7ff`,
then the artefacts); working tree clean; nothing pushed (no credentials —
the bundles + this pack are the delivery, ROUND8's discipline).

---

## INC5 — the device increment: runtime loader + live acceptance — **IN PROGRESS (sandbox slices 1–2 CLOSED; device run + INC5b launch wiring outstanding)**

**Session:** 2026-10-07, fresh sandbox on the re-published main `a9dbf31` (which carries
INC1–INC4). Branch `wo020-inc5`. Baseline re-measured EXACTLY at the INC4 seal (root 1033/0/1,
streams 75/0, observatory 124+6, every python gate) plus one pre-existing fmt red fixed as the
first hygiene commit (`577e13d`).

### The environment verdict, re-measured (the §8 discipline)

The plan called INC5 device-only; this session split it instead: **everything that does not need
cranelift is sandbox-buildable and was built, measured, committed.** The wasmtime half stays
device-first-compile, and the blockage was RE-PROBED, not assumed: a scratch winch-only tree
(wasmtime `=49.0.2`, `default-features=false`, `features=[std,runtime,winch,component-model]`)
still SIGKILLed `cranelift-codegen` at `cargo check` — `cargo tree -e features` shows why:
`wasmtime-internal-winch` → `wasmtime-internal-cranelift` → `cranelift-codegen`. **No wasmtime
feature set avoids cranelift; the 1 GB wall is categorical** (§8's conclusion, now from two
directions). The mitigation that replaced compiling: the pinned generator itself
(`wasmtime-internal-wit-bindgen 49.0.2`) is a standalone crate — a scratch driver
(`/tmp/bindgen-dump`, recipe: `Resolve::push_dir` → `select_world(&[pkg], Some("instrument"))` →
`Opts{with: {"sparq:instrument/assets.asset": AssetRep}}.generate`) **RAN the real bindgen over
the frozen WIT and dumped the exact 2677-line host expansion**; every wasmtime signature the
loader uses was then verified against the vendored sources (`Agent::new_with_config` was the one
guess the vendored read caught and killed). The loader is written against measured API, not
memory — but its first COMPILE is the device's / CI's feature-on cell.

### Delivery — 2026-10-07 (handoff prep: the pack, the run sheet, the closing bundle)

Operator instruction: **"Prep sync zip."** Delivered in the r8/rev-4 house shape:
`sparq-update-2026-10-07-wo020-inc5.zip` (**485 entries FULL-TREE**, 6 725 644 B, sha256
`d14db3698e4908c289532813abc7d44051e4452f4bf5323a4e4044a188b3cc88` — workspace root + the
committed copy under `handoff/` + `sha256sums-wo020-inc5.txt`), `WO020-INC5-RUN-SHEET.md` (ships
inside the pack: apply → re-stamp `sparq-wo020-inc5-2026-10-07` → gates → test007 A–L → the
send-back list, with BOTH honest states up front: INC5b/PLAY #58 and the pre-existing
token_audit red), the SYNC.md top entry (INC4 demoted to Previous), the packer adapted +
self-verified (its own clean-tree gate caught an uncommitted packer fix mid-run — the gate
working, recorded), and `handoff/sparq-wo020-inc5-delivery.bundle` (`a9dbf31`..the artefact
seal, sha `3a32f9b2…`) superseding the slice bundle for recovery. Namelist vs `a9dbf31`:
40 paths (17 A / 23 M), 37 shipped; MUST-NOT-MOVE surfaces (WIT, `modules/`, the stamp,
fixtures, goldens) asserted unmoved by the packer; nothing pushed (no credentials — the bundles
+ pack ARE the delivery). NOTE: this delivery record itself post-dates the bundle it names by
one commit (a bundle cannot carry its own commit — the r8 pattern); its sha is in the sums file
beside the pack.

**Incident (recovered, no trace in the tree):** between the seal turn and this one the sandbox
snapshot dropped `.git` AGAIN (the environment note's known behaviour) — and with it
`modules/out/main/sparqmod.toml` from the working tree (the #93 trap's directory-name pattern,
second occurrence). Recovery was exactly the recorded recipe: re-clone origin (still serving the
base `a9dbf31`), fetch `handoff/sparq-wo020-inc5.bundle`, `symbolic-ref` HEAD onto the branch +
`git reset` (tree untouched), `git checkout -- modules/out/main/sparqmod.toml`, re-commit the
bundle artefacts. The reset-vs-seal diff came back EMPTY except the known drop — byte-identical
recovery, proven not assumed. Lesson re-recorded: the bundle in `handoff/` remains the durable
artefact; commit + bundle every slice, exactly as this WO has.

### Device session 1 — 2026-10-07 (the first test007 run: REFUSED at [A], root-caused from the log)

The operator applied the pack and ran `test007.bat`; the symptom reported was "the new instrument
does not show in the library". The log is unambiguous: **[A] never built** — `sync_check: FAIL,
this tree is NOT sync sparq-wo018-rebuild-2026-10-05 (27 files differ)` → build.bat's defect-#78
guard refused (the re-stamp, run-sheet §1.2, had not run yet), so **[B] `unknown command
streams`**, **[D] `unknown command mod`**, and **[E] the shell was the STALE pre-WO-020 exe** —
a shell from before the instrument browser existed. Nothing was wrong with the pack or the
discovery path; the run-sheet's own ordering was skipped and my test007 [A] note ("stamp-mismatch
words are EXPECTED") was WRONG — build.bat refuses, it does not merely warn. Found-by-running-it
fixes, landed in-tree (rides the next pack; the deployed zip is unchanged):
* `test007.bat` grows **step [0]**: `sync_check` first, fail fast with the exact re-stamp command
  printed — the trap cannot re-fire silently.
* `WO020-INC5-RUN-SHEET.md` §3: "re-stamp BEFORE test007", with this incident named.
* The CWD-relative + silent discovery defect found while diagnosing (the shell says nothing when
  `instruments\` is absent from the process CWD) stays recorded in LATER.md as INC5b's first fix —
  it was NOT this symptom's cause, but it is real and measured (root CWD: observatory in the
  convergence SVG ×2; other CWD: ×0).
Remedy given to the operator: `python tools\sync_check.py --write --sync
sparq-wo020-inc5-2026-10-07` then re-run `scripts\test007.bat` (first build 10–40 min; then
[B]/[D]/[E] run against the new binary).

### Device session 2 — 2026-10-07 (the re-stamp run: wasmtime COMPILES on SATURN; the first-compile defects, found and fixed)

Re-stamp → test007 again. Step [A] built for real: **wasmtime 49.0.2 + cranelift compiled clean
on the device** (the §8 memory wall is the sandbox's alone, as ruled), and the build reached the
runtime — **5 errors + 1 warning, all in the blind-written half** (four `*display_w/h` derefs of
by-value f32s; a format! passing `draw_fuel` positionally AND inline — which also would have
printed draw fuel as the memory high-water; a dead `full_json` initializer under -D warnings;
two unused test imports; the preview assertion not knowing svg() opens with the XML declaration).
All fixed smallest-honest-patch (`1e3fe95`), the budget-overrun report lines moved to
E-CROSS-FIELD (measured-vs-declared, not requires-unsupported), and the layers rustc never
reached were desk-checked against the vendored sources in the same pass (HasSelf, BlockStatus's
module, the sparq-ui pub faces). **Revision pack `sparq-update-2026-10-07-wo020-inc5r2.zip`**
(485 entries, sha `d6e283231252626aca1954da2d40125b09ec3de1af404e7883f1b4d60ed2947c`, stamp
`sparq-wo020-inc5r2-2026-10-07` — a second zip must not share the first's stamp name); test007
grew the fail-fast step [0] after session 1. This is the §4 loop working as designed: the device
is where the instrument-host cell compiles; every defect it finds comes back, gets fixed, and
gets recorded.

### Device session 3 — 2026-10-07 (the caret bug: a fail-fast gate that refused everyone)

The operator applied r2, re-stamped (`sync_check: OK — 175 of 175 … byte-for-byte`) and test007
STILL refused at step [0]. Root cause, in words: my step-[0] patch wrote `if errorlevel 1 ^(` —
the ESCAPED open paren. cmd treats `^(` as a literal, the block never forms, and the "guarded"
body (`echo TEST007 REFUSED … exit /b 1`) runs UNCONDITIONALLY: the stamp gate refused every
tree, healthy ones included. One character class, found on the device because a bat file is only
really parsed by cmd. Fixed in r3; `check_text_io.py` grows **rule 5** (an escaped `^(` on a
structural if/for line is a defect — rule 4's mirror image), proven failable by injection then
reverted byte-identical, per the house rule that a gate nobody has seen fail is a gate nobody
trusts. Revision pack `sparq-update-2026-10-07-wo020-inc5r3.zip`, stamp
`sparq-wo020-inc5r3-2026-10-07` (a third zip must not share the second's stamp name either).

### Device session 4 — 2026-10-07 (the zero-fuel instantiation trap; the loader's first real run)

r3 on the device: the runtime COMPILED, linked, and ran — stages 3–6 all reported. The load
refused with `instantiate: wasm trap: all fuel consumed by WebAssembly`, and my error words
mis-blamed the import list. The real defect, one line: **`consume_fuel(true)` starts the store
at ZERO fuel and instantiation itself executes wasm** (start sections, lowering trampolines) —
`spawn()` fueled the store only inside `budget_call`, which first runs AFTER instantiate. Fixed:
the store is fueled with the prepare-class budget before instantiation; the instantiate error
classifies through `trap_words` (traps keep their own words; only NON-trap failures carry the
capability-by-absence sentence — session 4's lesson: do not blame imports for a fuel
exhaustion); and instantiation fuel is now recorded in the stage-5 report (an invisible load
cost surprises someone later). Also in r4: the stages' draw calls run under a DIAGNOSTIC budget
(prepare's multiplier) and stage 5 JUDGES the measured number against the declared `max_fuel` —
a genuinely under-declared guest now produces a measured re-baselining fact instead of an
unmeasured trap (the Observatory declares 2 000 000; its real full-wall draw cost was never
measured anywhere — the sandbox has no runtime, and the harness is fuel-free). Production draw
enforcement stays the watchdog's row (§3): declared budget, frame-skip, words. The desk check
missed the zero-fuel instantiation because wasmtime's fuel semantics for `instantiate` were
never verified against the vendored source — recorded as the class it is: an API-behaviour
assumption, exactly what the device loop exists to catch. Revision pack
`sparq-update-2026-10-07-wo020-inc5r4.zip`, stamp `sparq-wo020-inc5r4-2026-10-07`.

### Device session 5 — 2026-10-07 (the epoch deadline that was never armed; [B] live-probed green)

r4 on the device: the fuel fix landed (instantiation now runs metered, budget visible in the
words), the load reached wasm — and trapped `epoch deadline exceeded` at instantiation. The
vendored doc says it outright: `epoch_deadline_trap()` arms the trap but leaves the deadline at
ZERO — "it's required to call `Store::set_epoch_deadline` or otherwise wasm will always
immediately trap." One line fixed (`set_epoch_deadline(EPOCH_DEADLINE_TICKS)`, 1 000 ticks — the
gates have no incrementer; the production watchdog's cadence lands with INC5b and turns ticks
into the §3 deadline). Session 5 also MEASURED the stream plane's device half for the first
time: **[B] the live probe ran green from SATURN — 22× HTTP 200 + FIRMS KEY NEEDED in words**
(the §3.3 asterisks are device facts now). r5: `sparq-update-2026-10-07-wo020-inc5r5.zip`, stamp
`sparq-wo020-inc5r5-2026-10-07`.

### Device session 6 — 2026-10-07 (id() is not cheap: the first call pays the guest's lazy init)

r5 on the device: instantiation SUCCEEDED (epoch armed, fuel metered, instantiation cost
recorded) and the load reached the first guest call — `guest.id()` trapped, `fuel budget
exhausted (2000000 of 2000000 burned)`. Two facts, one fix: the SDK builds its state on the
FIRST export call (for the Observatory that includes `Wall::new()` parsing the embedded
coastline), and the loader had given a load-time lifecycle call the PER-BLOCK budget. The
per-block budget is `process`'s contract and nothing else's: `id`/`activate`/`deactivate`/
`save-state` now ride the control budget (prepare's multiplier, 64×). Recorded for the guide's
eventual author-facing text: a guest's first-call cost is real and lands on whichever door opens
first. r6: `sparq-update-2026-10-07-wo020-inc5r6.zip`, stamp `sparq-wo020-inc5r6-2026-10-07`.

### Slice 1 — the stream plane's live half (`streams-net` LANDED) — commit `83b2091`, MEASURED

`sparq-streams::fetch` (the endpoint fill mirroring `streams_record.py` exactly — `{d-N}`/`{D-N}`/
`{lat}`/`{lon}`/`{st}`/`{CC}`/`{KEY}`, one-retry-on-429/5xx, key redaction: a resolved env key
exists only inside `Request::url`), `sparq-streams::broker` (cadence floors count ATTEMPTS;
**replace-vs-accumulate is registry DATA** — new `accumulate` column, exactly the three SWPC
`summary/*` rows append monotonically with dedup, INC3 finding #7's remedy; last-good cache seeds
relaunches; failures keep windows and leave words), `HttpTransport` (ureq 3.4.2 + rustls/ring +
webpki-roots — compiles AND runs in the 1 GB box), `BrokerProvider` (the live `StreamProvider`
behind the same trait, `SharedBroker` = the `HealthMirror` idiom), the CLI's real `probe
[--live]`/`fetch`/`tail` verbs, and **`record-fixtures` stays a refusal naming its owner**
(tools/streams_record.py — one owner of the fixture format; a ruling to record, reversible).
The plane's only two impure helpers (`now_unix`, `sleep_millis`) are the ADR-011 §5 wall-clock
edge, allow-with-reason per defect #66. Python recorder gained the redaction fix (a latent INC1
leak: `_meta.json` would carry a real `SPARQ_FIRMS_KEY` on a keyed device).
**Measured:** root 1033/0/1 unchanged; streams 96/0 both cells; host-wasm+streams 28/0; app
34/0; observatory 124/0 (registry drift green over the new column); every python gate; zero-dep
promise verified by `cargo tree`; **live smoke green** (real TLS fetch of swpc.kp, 200, normalized);
**live probe from the sandbox: 22/23 HTTP 200**, FIRMS KEY NEEDED in words, `wx.alerts` resolved
here this time, `geo.wildfires` still byte-identical to `geo.eonet` (INC1 drift #2 re-confirmed).

### Slice 2 — the instrument runtime (`instrument-host`) — commit `2fdb0f9`, WRITTEN, device-first-compile

`sparq-host-wasm::runtime` = instrument-host.md §2–§4 + §7 as code: the four doors (tokens serves
the checked-in bundle verbatim; host.now/random-seed — the KERNEL's `derive_seed`, one copy — /log
with RT-violation capture; assets scoped to the manifest's declared hashes, v1 answers `not-found`
honestly; sources = the D4 resolution over `StreamProvider`, serving `window.snapshot(now)` — the
flag-stamping that makes the guest's clockless status derivation equal the broker's, which is the
golden's cross-boundary parity rule), the §2 load order (compile → identity cross-check →
prepare-before-anything on BOTH instances), the memory limiter + fuel→`Overrun` mapping (§3), the
**wasmtime `cache` feature added to the pin** (the §2.1 compile cache with NO unsafe in sparq
code — `Component::deserialize` is unsafe and the allowlist forbids unsafe in Tier-2 host code;
the pin comment records the ruling), `WasmInstrument` (the `Module` face — the registry grew
`register_instrument`/`SharedFactory` for state-capturing factories, tier-blind rules unchanged),
and the gate's runtime stages: 3 smoke (the smoke.mjs mirror), 4 golden (draw ×2 bit-exact, fuel
recorded), 5 measured (fuel + memory high-water vs the declared class), 6 visual (LOD ladder vs
the gpu ceilings, the 500-primitive Minimal rule, the unknown-token scan, `preview.svg` through
the SHELL's painter, the at-rest publish — the LATER door). `validate::run_gated(dir, ctx)` runs
the FULL chain when the caller names the world; `sparq mod validate` does so under the app's
`instrument-host` feature. The noop fixture was REBUILT in this sandbox (SDK 1.1.0, rustc 1.99.0,
wasm-tools 1.261.0, unknown-unknown recipe): **53 747 B, sha `df14d18f781ea477596db552e60bdb9df06344bc673593c38148e28a3d546c65`**,
pinned in `tests/instrument_runtime.rs` with the §7 plan as 12 tests (incl. the cross-boundary
anchor: the Observatory wasm path must hash to the harness's `bdf59cdb…`). Static gates measured
green on the fixture (stages 1/2/7 PASS). `scripts/test007.bat` is the §15.2 run-sheet A–L;
`gates.bat`/`build.bat`/`ci.yml` carry the feature-on cells.

### What is NOT done (honest, in words)

1. **INC5b — the launch wiring:** discovery→`register_instrument` at launch, PLAY #58's
   discharge (the executor adopting `WasmInstrument`), and the shell's live-provider swap
   (BrokerProvider + driver thread in `ui/live.rs`). The registry door and the Module face exist
   and are tested; the app-side wiring is the next slice. Until it lands, test007's E/F steps run
   against the at-rest wall (stage 6 publishes it) and PLAY still refuses in words.
2. **The device run itself:** every `instrument-host` compile/test, the FULL validate chain on
   the sealed package, the frame-time histogram, the network story, the re-theme demo — test007
   A–L, recorded not invented.
3. Known-small gaps, declared in code: `TimeInfo`'s tempo/bar fields ride defaults inside
   `process` until the executor publishes the musical clock (the type's docs say so); a corporate
   TLS-inspecting proxy would need ureq's proxy door (webpki-roots ride the binary); the wasm
   `process` marshalling allocates (wasmtime's lifting — bounded for port-less; the native
   zero-alloc proof is the Phase-5 arena work).

---

## INC3 — the guest: source project, five-file package, harness, component — CLOSED

The plan's D13 split, built: one pure core linked by BOTH the wasm glue and the native harness, so
the shipped logic and the proven logic are the same object. The component build attempt (risk §12.2)
SUCCEEDED in the ~1 GB sandbox, so the package ships its real wasm and `sparq mod validate` runs its
static half over the assembled directory.

| path | what |
|---|---|
| `tools/make_coastline.py` + `instruments-src/observatory/assets/coastline-110m.bin` | D12: Natural Earth 110m land (public domain, cached `tools/data/`) → Visvalingam-Whyatt (one shared area threshold, binary-searched to the point target, index tie-break — deterministic) → **128 rings / 3 500 points / 14 524 B** (budgets ≤ ~3 500 pts / ≤ 160 KB). Format documented in the tool: LE, `u32` magic/version/ring-count, per ring `u32` count + `i16` centidegree lon/lat pairs, rings implicitly closed. `--check` is the staleness gate. Embedded via `include_bytes!` (the wasm data section, guide §3 small-asset rule). |
| `instruments-src/observatory/Cargo.toml` | its OWN workspace (core/wasm/harness) on the `tools/sparq-module-guest` precedent — the wasm face needs wit-bindgen and the root's default build stays zero-dep. **`exclude = ["../../tools/sparq-module-guest"]` is load-bearing**: without it, path-dep auto-membership makes `cargo fmt --all` in this workspace REFORMAT the frozen SDK (measured, twice). House lints mirrored; release profile `strip = "debuginfo"` (component 2.9 MB → 206 KB, name section kept for device trap messages). |
| `instruments-src/observatory/gen_manifest.py` | `package/sparqmod.toml` + `core/src/streams_table.rs`, both from `streams.toml` (never hand-typed): 16 cell enums × (OFF + 23 registry ids in file order), `selected_cell`, `layout`, five bools (**float defaults — the §10 `true` gap closed**), three floats (**history in SECONDS 600…604 800 default 10 800 exp — the §10 `min` gap closed**, UNITS is closed), the §5.2 toolbar as 27 generated widgets (the sixteen STREAM `enum_select`s carry `visible_if = { param = "selected_cell", equals = <option INDEX> }`), display `wall` (min_size 2176×1120, colormap.thermal) + 16 `stream = "param:cell_NN"` sources, port-less (D5), `network = "none"`, `gpu_class = "medium"`. The streams table is the guest-side registry mirror (no fs/network in T2). `--check` gates both artefacts. |
| `observatory-core` (pure, zero deps) | `ir.rs` the display-list IR, field-for-field display.wit v1 (Lod ordered detail-ascending so `>= Reduced` reads as detail); `record.rs` the third mirror of the frozen data-record (D13 — the harness and the glue own the two mappings, both drift-tested); `layout.rs` §5.1 maths as tested functions; `params.rs` the 26-param decode (floats = raw domain values, the native precedent; enums = option index; bools 0/1; clamping, NaN-index fallback) with **const-asserts proving 26 ≤ 32**; `state.rs` the deterministic blob (`OBS1` v1, magic+version+26×f32 LE = 109 B; **params-only — the scroll phase is display-instance animation and save_state runs on the audio instance, instrument-host §4**); `coastline.rs` the asset reader (refuses bad magic/version/truncation/trailing bytes in words); `view.rs` window→view (clock-free: x-windows end at the newest record's own `t_utc`); `render/*` the seven §5.5 renderers + §5.3 chrome + §5.5 ticker + §5.7 LOD ladder; `wall.rs` draw + budget counter. |
| `observatory-wasm` | the SDK glue: `process` = the port-less no-op returning `Ok` + the exact empty negotiated shapes while mirroring `block-input.params`; `draw` = 16 × `sources.snapshot("wall","cell-NN")` → core records → `Wall::draw` → IR→WIT conversion (total, tested); `configure`/`save_state` = the params blob; KEY NEEDED derived from the served window + the stream's `key_env` (D14). 10 native tests (the export macro is wasm-gated, so the glue is host-testable). |
| the component | **built in the sandbox**: `rustup target add wasm32-unknown-unknown` + `cargo build --release --target wasm32-unknown-unknown -p observatory-wasm` + `wasm-tools component new` (v1.261.0 prebuilt x86_64-linux, GitHub release — `cargo install wasm-tools` is too heavy for 1 GB) → `wasm-tools validate` CLEAN, **206 081 B**, world imports `types/sources/audio/display`, exports `sparq:instrument/guest@1.1.0`. The `wasm32-wasip1` attempt was made first and REJECTED with evidence: wasip1 std startup imports `wasi_snapshot_preview1::environ_get`, which the host deliberately does not provide (capability by absence, WIT README decision E) — `component new` refuses it. The corrected recipe (WIT README) is unknown-unknown. |
| `observatory-harness` | `to_core` (broker windows → core records, D14 key-words), `tokens` (the checked-in bundle as resolver — unknown id skips with a diagnostic, hairline opacity read from `color.hairline.regular`), `ir_json` (the D13 interchange; BTreeMap-sorted keys + deterministic floats ⇒ stable hashes; NaN → JSON null = the no-cell sentinel), `svg` (the minimal painter — INC4's `displaylist.rs` stand-in over the SAME IR; heat grids downsampled to ≤ 8192 cells FOR THE SVG ONLY, the IR keeps full resolution). Renders the §5.4 default wall at the three breakpoints' LODs; artefacts gitignored, regenerable. 22 unit tests + `tests/golden.rs` (6): **golden IR sha256 `bdf59cdb232f947091451017f50712a444687c8b1f0a62b5a630763775e974fa`** (re-pinned INC4 when the interchange gained `display_w`/`display_h` — a format change, reason recorded at the pin) (hand-rolled sha256 with published vectors), streams_table↔registry field-for-field, Metrics↔bundle, manifest-defaults↔core-defaults, all 26 emitted token ids resolve. |
| `tools/observatory_package.py` + `instruments/observatory/` | the assembler copies, never invents: refuses a stale generated manifest (`gen_manifest --check`), a missing hand-authored file, or a missing component (a half-package fails stage 2 fatally). Assembled **4 files ≤ 5 cap**: `sparqmod.toml`, `observatory.wasm`, `example.sparqpatch` (§5.4: the wall beside `mod/clk→mod/seq→syn/sine→util/vca→out/main`, silent-but-live), `README.md` (keys §11.4, attribution §11.3 incl. the Open-Meteo non-commercial flag, budgets). `preview.svg` absent BY DESIGN (stage 6 generates it; `package.rs` makes absence advisory). |
| `crates/sparq-audio/tests/portless_instrument.rs` + `bridge.rs` unit tests | the D5 proof as TESTS (+9): the shipped manifest validates port-less; registration accepts it; the executor builds a port-less-ONLY graph; the wall coexists with the audio path and the set still plays; `bridge::build` with the wall in the canvas graph; `browser_catalog` lists it (instrument layer, 26 params); `node_bands`/`node_size`/full `layout::compute` never refuse a zero-port spec. Both read the CHECKED-IN package manifest (include_str!/path), so a port-growing ship fails by construction. |
| `design/tokens/layout.toml` + regenerated | additive: `[marker] radius_s/m/l = 3/5/8` — the `points-item.size` token class §5.5's "size class by magnitude" requires (a size token id with no bundle entry is an unresolvable primitive). `token_gen --check` deterministic; `token_bundle.rs` pin test green; no new `token_audit` violation. |
| `tools/streams_record.py` (hygiene, first commit of the session) | the CI `check_text_io` gate was **RED on main** (INC1 left two bare pathlib `write_text` calls + no UTF-8 stream reconfigure). Fixed with the house helper pair; gate green; `--list` unchanged. |

**INC3 gates (all MEASURED this session):** `sparq mod validate instruments/observatory/` → stage 1
**PASS**, stage 2 **PASS** (4 files, roles recognised, component present), stages 3/4/6 **REFUSED IN
WORDS** (the runtime half), stage 5 static half PASS, stage 7 unsigned→badged; **GATE: PARTIAL,
exit 1 by design** ("a partial gate is not a hand-in"). `sparq mod list` → `dat/observatory v0.1.0
… loadable` + the preview.svg advisory. Root `cargo test --workspace` **991/0/1-ignored** (+9 over
INC2). instruments-src workspace **124 lib/unit + 6 golden/drift** tests. clippy `--all-targets
-D warnings` CLEAN in BOTH workspaces. fmt CLEAN (root `--all`; instruments-src PER PACKAGE — `--all`
there adopts the SDK). Python gates: `token_gen --check`, `unsafe_audit`, `module_docs --check`,
`check_text_io`, `sync_check --self-test`, `make_coastline --check`, `gen_manifest --check` all PASS;
`token_audit` still fails the pre-existing §18 mockup SVGs (no INC3 file flagged; that gate's owner
is the mockup work). WO-005 golden hash `ba577186c988db21` UNCHANGED (printed and compared). WIT
pins, the 24 module manifests, `SYNC-STAMP.txt` UNTOUCHED. Harness SVGs reviewed in-session at the
three LODs against the display-sheet idiom (aurora oval + colourbar, coastlines, quake/EONET/GDACS
dots, ISS footprint ring + crosshair, Kp bars + storm line, phosphor-green traces, AQI gauge sweep,
WWV raw text, the scrolling ticker; Reduced drops graticules/labels, Minimal keeps renderers under
the 500-primitive rule + the ticker).

**Found by building it (recorded, not papered over):**
1. **§5.1 table erratum:** the 4×4 cell height prints 270; the plan's own gap (`space.4` = 16) gives
   `round((1120 − 3·16)/4)` = **268**. The formula is the source of truth (layout.rs test asserts
   268 with a comment; every other table value reproduces exactly).
2. **`cargo fmt --all` adopts path-deps' workspaces** — in instruments-src it reformatted the frozen
   SDK; the workspace `exclude` fixes it, and the house rule here is per-package fmt.
3. **`wasm32-wasip1` cannot componentise** under the no-WASI host (environ_get import); the contract's
   own corrected recipe (unknown-unknown) is the path.
4. **Draw-order bug caught by LOOKING at the render:** the cell chrome painted the inset ground
   AFTER the body, burying every renderer; split into `cell_ground` (under) → body → chrome.
5. **Frozen-surface frictions → LATER.md (ADR-010 review-trigger material):** `trace-item` has no box
   (per-cell series ride polylines; phosphor motion tokens unused on walls); `rect-item` has no
   colormap channel (Kp bars encode level as height + the storm line); `glyph-run` has no anchor or
   measured advance (the guest estimates 0.6 × size for right-align/truncation); one wall-wide
   `history` param vs daily feeds (POWER shows ~1 point at the 3 h default); gauge threshold bands
   belong in `streams.toml` rows.
6. **Canvas anatomy `rows = max(1)`** draws one cosmetic port row on a port-less card — not a refusal
   (D5's bar is "nothing refuses"); the D6 zero-port-band instrument card is INC4's sizing work.
7. **Summary-cadence feeds are single-record windows** in fixtures (solar-wind, bz are `summary`
   endpoints): their timeseries cells draw a dot, not a trace, until the broker accumulates live
   history (INC5). Honest first-draw behaviour, not a defect — recorded so the device run-sheet
   expects it.

**Commit/bundle discipline honoured:** four slices (`cef2385` hygiene, `48a56c2` core+generator,
`0bd7f43` glue+harness+gates, `7c2f4a7` package+D5) + the seal commit carrying this card, the
CHECKLIST entry and the LATER rows. Bundle: `handoff/sparq-wo020-inc3.bundle` (+ sha256).

---

## INC2 — the contract additions (ADR-011, stream bindings, tokens, validator) — CLOSED

The one contract addition the stream plane needs, **additive, WIT untouched** (§7.4 proved: the
`wit_snapshot` hashes are byte-identical and the WIT files show no diff). Committed on
`wo020-observatory` after the INC1 seal `c19fb83` (see `git log`; carried by
`handoff/sparq-wo020-inc2.bundle`).

| path | what |
|---|---|
| `docs/adr/011-stream-plane.md` (+ `docs/adr/README.md` index) | **ADR-011** — the stream plane: host-side broker, the `stream` source binding, the registry as shared data, env-scoped keys, the determinism firewall; alternatives rejected; the §7.4 "why no WIT amendment" proof recorded; the Phase-5 review trigger. |
| `crates/sparq-module-api/src/error.rs` | `CodeKind::StreamUnknown` = **`E-STREAM-UNKNOWN`** (the catalogue's 26th code; `ALL` 30→31, count test moved in-commit). |
| `crates/sparq-module-api/src/decode.rs` | `DISPLAY_SOURCE_KEYS` +`stream`; the source binding is now **exactly one of `port`/`stream`** (`E-CROSS-FIELD`); `check_stream_binding` shape-checks a bare id vs `param:<id>`; `check_widget_visible_ifs` pins `visible_if` to `{param, equals}` (plan D7). +2 decode tests. |
| `crates/sparq-host-wasm/src/validate.rs` (+ `Cargo.toml` dep on `sparq-streams`) | stage 1 resolves `stream` bindings against the checked-in registry (`E-STREAM-UNKNOWN`), the `param:` rules (exists / is enum / options ⊆ registry, `E-CROSS-FIELD`+`E-STREAM-UNKNOWN`), and `visible_if.param` existence. Registry loaded lazily; port-less instruments accepted (D5). |
| `crates/sparq-host-wasm/tests/stream_bindings.rs` | **9 tests**: the representative Observatory draft passes stages 1–2 with real bindings; each hostile variant (unknown id, `param:` missing/non-enum/option-outside-registry, `visible_if` unknown param, port+stream both) fails its rule in words; **every one of the 23 registry ids accepted as a fixed binding** (the registry↔contract drift gate). |
| `design/tokens/layout.toml` (+ regenerated `generated/**`) | `node_width_instrument_max = 2400` + the card-sizing rule (D6) as data; `token_gen.py` re-run, `--check` deterministic. |
| `docs/api/manifest-schema.md` §9 + catalogue; `docs/api/manifest-fields.toml` | the `port|stream` binding, `visible_if` semantics, `E-STREAM-UNKNOWN` + the `E-CROSS-FIELD` rules, all as data (the field-table drift tests only pin layer/gpu_class/top, untouched). |
| `MODULE-BUILD-GUIDE.md` §5 + changelog | one author-facing paragraph: stream bindings + where the registry lives (the guide's own "amend when falsified" rule). |

**INC2 gates (measured):** `cargo test --workspace` **982/0/1-ignored** (+11 over INC1's 971: 2 decode + 9 validator); `sparq-streams --features streams` still **75/0**; clippy `-D warnings` CLEAN (default workspace + host-wasm + streams cell); fmt CLEAN; `token_gen --check` CLEAN; `wit_snapshot` PASS and `docs/api/instrument-wit/wit/*` shows **no git diff** (§7.4); `api_snapshot` + the three field-table drift tests PASS; goldens and the 24 module manifests untouched. `unsafe_audit` + `module_docs --check` CLEAN. (`token_audit` still fails on the pre-existing §18 mockup SVGs — no INC2 file is flagged.)

**Two §10 draft-manifest gaps found by building it** (both are generator concerns for INC3, recorded not papered over): the draft's `unit = "min"` is **not** in the closed `UNITS` vocabulary, and its bool params use `default = true` where the decoder wants a float (`1.0`). `gen_manifest.py` (INC3) must emit valid values; the representative test manifest already does.

---

## INC1 — the stream broker's hermetic half + the registry as data — CLOSED

The stream plane's **hermetic half** — everything the ~1 GB sandbox can build and prove without a
socket or wasmtime (plan D2, §9 INC1). The default build stays **zero third-party dependencies**
(`cargo tree` verified: `sparq-streams` default pulls only `sparq-module-api` → `sparq-kernel`);
`serde_json` rides the `streams` feature, `ureq`/TLS the `streams-net` feature (the device half).

| path | what |
|---|---|
| `crates/sparq-streams/streams.toml` | **THE registry** — 23 streams (id, domain, label, endpoint template, cadence, view, schema, units, attribution, key_env/fallback, transform, config knobs) + the §3.3 dead ends recorded as comments. Single source of truth for stream ids. |
| `crates/sparq-streams/src/record.rs` | Native mirror of the frozen `data-value`/`data-flags`/`data-record` vocabulary (`types.wit` v1.1), field-for-field. Advisory-stamp rules documented. |
| `crates/sparq-streams/src/schema.rs` | The five `observatory/*@1` channel schemas as typed builders + the shared severity/visibility tier vocabularies. |
| `crates/sparq-streams/src/registry.rs` | `streams.toml` via the house TOML subset parser; `KeyState` (KEY NEEDED in words, D14); the `E-STREAM-UNKNOWN` refusal message INC2 reuses. |
| `crates/sparq-streams/src/window.rs` | Bounded rolling windows (timeseries 2048 / events 256 / grid 2 / text 64) + the stale(3×)/offline(10×) policy. **Clock-free** — `now` is injected (D11 firewall). |
| `crates/sparq-streams/src/time.rs` | Dependency-free timestamp parsing (ISO-8601 family ± offset/frac, compact `YYYYMMDD`/`YYYYMMDDHHMMSS`, SWPC `:Issued:`), no chrono, no `SystemTime`. |
| `crates/sparq-streams/src/normalize.rs` *(streams)* | All 23 feeds → frozen `data-record`s, written against the **recorded** payloads. Severity tiers, ISS km/h→km/s, aurora 2° regrid (16 200 cells), POWER fill-value skip, NWS null-geometry tolerance. |
| `crates/sparq-streams/src/cache.rs` *(streams)* | Disk cache (last-good + metadata), env-resolved user-data dir; `File`/`read_to_string` only (honours the `clippy.toml` fs ban). |
| `crates/sparq-streams/src/replay.rs` *(streams)* | Fixtures → windows (the hermetic door for goldens/tests/INC4's `ReplayProvider`). |
| `crates/sparq-streams/tests/normalizers.rs` | 21 tests: 23 per-stream normalizers against real fixtures + the registry↔normalizer drift gate + determinism (same fixture twice, bit-identical). |
| `tools/streams_record.py` | urllib recorder; reads `streams.toml` (tomllib), fills endpoint templates, writes raw payloads + `_meta.json` + `PROBE-LOG-<date>.md`. |
| `reference/fixtures/observatory/` | All 23 streams recorded at HTTP 200 (FIRMS synthetic — free key unavailable), raw payloads + `_meta.json` + `PROBE-LOG-2026-10-06.md`. |
| `crates/sparq-app/src/streams.rs` | `sparq streams list|cache` (hermetic) + `probe/fetch/tail/record-fixtures` refuse in words, exit non-zero without `streams-net`. Exit codes testable via `run_code() -> u8`. |
| `Cargo.toml` (root + app) | `sparq-streams` member; `serde_json` optional workspace dep; app `streams`/`streams-net` features. |

## Gates (all MEASURED this session, not invented)

* Pristine baseline before any edit: **925 / 0 / 1-ignored** (the round-8 seal, exactly).
* `cargo test --workspace` (default, zero-dep): **971 / 0 / 1-ignored** (+46).
* `cargo test -p sparq-streams --features streams`: **75 / 0** (54 lib + 21 normalizer).
* `cargo test -p sparq-app --features streams`: green (incl. the 5 `streams::tests` CLI cases).
* `cargo clippy --workspace --all-targets -- -D warnings`: **CLEAN** (default), and CLEAN for
  `-p sparq-streams --features streams` and `-p sparq-app --features streams`.
* `cargo fmt --check`: **CLEAN**.
* Zero-dep promise: `cargo tree` shows no third-party crate in the default build of `sparq-app` or
  `sparq-streams`; `serde_json` (+ its transitive `serde_core`/`zmij`/`itoa`/`memchr`) appears only
  under `--features streams`.
* Fixture-size acceptance: every fixture is the **raw payload**, worst ratio **1.000** (≤ 1.2 ✓).
* **Untouched (as the plan requires):** `docs/api/instrument-wit/wit/*` hash pins, the goldens, the
  24 module manifests, `SYNC-STAMP.txt`.

## API drift found by recording live (the probe-first rule earning its keep, §12.6)

1. **NASA POWER 422s on dashed dates.** `power.larc.nasa.gov/.../daily/point` now rejects
   `start=2026-09-06` with `int_parsing: Input should be a valid integer`. It wants **compact
   `YYYYMMDD`**. Fix landed: the recorder + registry gained `{D-N}` (compact) placeholders
   alongside `{d-N}` (dashed); POWER's endpoint uses `{D-30}`/`{D-3}`. Re-recorded → 200, 1571 B.
2. **EONET `categories=wildfires` is ignored under `limit`.** With `limit=200`, `geo.wildfires`
   returns byte-identical content to `geo.eonet` (all open events) — EONET v3 applies `limit`
   before the category filter. Kept as its own registry row per §3.1; flagged in `streams.toml` for
   INC3/operator (drop the limit → ~5 MB, or retire the row). Not silently papered over.
3. **FIRMS needs a free key** (`SPARQ_FIRMS_KEY`); with none set it records a small **synthetic**
   CSV sample, flagged `"synthetic": true` in `_meta.json` and the probe log (D14: KEY NEEDED, never
   a silent hole). The normalizer is tested against it; a real key replaces it on the next record.

## What is NOT done (the rest of WO-020)

* **INC3** — CLOSED this session (see above).
* **INC4** — CLOSED this session (see the INC4 closeout in CHECKLIST.md). The host half in one
  sentence: `sparq-ui::displaylist` (zero-dep JSON subset → parsed IR → token-resolving `Painter` →
  SVG back end) plus the egui back end in `sparq-app` (same resolved list, dashes as segments,
  curves chorded), the at-rest store (SPARQ_ATREST / cache dir; WORDS when none), the picker
  (44 px rows, undoable select), FOCUS (menu row / header double-tap / toolbar), D6 sizing
  (2208×1288 measured; coverage audit row 97.5 % at 2560×1600), browser sections, the §5.2 status
  label from the ReplayProvider, `sparq instrument render` with the §5.8 budget vs the declared
  gpu_class.
* **INC5** — device-only: wasmtime loader, `BrokerProvider` (`streams-net` + `fetch.rs`), the full
  validate chain, live acceptance (run-sheet §15.2 / `scripts/test007.bat`).
* **INC6** — deferred/gated (sonification, SGP4 tracks, GIBS imagery).

## Resume recipe (a fresh sandbox has NO Rust and NO C linker — install both first)

```sh
# 1. Toolchain (this session installed it; it does NOT persist across sandbox snapshots).
python3 - <<'PY'   # no curl/wget in the sandbox — download rustup with python
import urllib.request,os
u='https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init'
d=urllib.request.urlopen(urllib.request.Request(u,headers={'User-Agent':'sparq'}),timeout=280).read()
open('/tmp/rustup-init','wb').write(d); os.chmod('/tmp/rustup-init',0o755)
PY
/tmp/rustup-init -y --profile minimal --default-toolchain stable --component rustfmt --component clippy --no-modify-path
# 2. A C linker (rustc needs `cc`; the sandbox has none). Root + apt works:
apt-get update -qq && apt-get install -y --no-install-recommends gcc libc6-dev
# 3. Gates (root workspace):
export PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup"
cd /home/user/sparq
cargo test --workspace                                   # 991 / 0 / 1  (INC3 seal)
cargo test -p sparq-streams --features streams           # 75 / 0
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p sparq-app --features streams && target/debug/sparq streams list
target/debug/sparq mod validate instruments/observatory/  # PARTIAL exit 1 BY DESIGN (static half)
# 4. The Observatory workspace (its own gates; fmt PER PACKAGE — --all adopts the frozen SDK):
cd instruments-src/observatory
cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings
cargo fmt -p observatory-core -p observatory-wasm -p observatory-harness --check
cargo run -q -p observatory-harness -- --report           # §5.8 budget vs medium ceilings
python3 ../../tools/make_coastline.py --check && python3 gen_manifest.py --check
# 5. The component (sandbox-proven recipe; wasm-tools PREBUILT — cargo install is too heavy for 1 GB):
rustup target add wasm32-unknown-unknown
cargo build --release --target wasm32-unknown-unknown -p observatory-wasm
python3 tools/fetch_wasm_tools.py   # prints the exec path (pins wasm-tools 1.261.0)
/tmp/wasm-tools component new target/wasm32-unknown-unknown/release/observatory_wasm.wasm -o /tmp/observatory.wasm
cd /home/user/sparq && python3 tools/observatory_package.py --component /tmp/observatory.wasm
# 6. Re-record fixtures only if the network is up and drift is suspected (they are checked in):
python3 tools/streams_record.py --all --out reference/fixtures/observatory/
```

## House notes

* **Incident (this session, no trace left in the tree):** a broken shell heredoc in the session's own
  tooling executed the resume recipe's re-record line mid-edit, re-recording all 23 fixtures live
  (~13:51 UTC). Caught by `git status` before any commit and by the golden-IR pin afterwards; the
  fixtures, `_meta.json` and the probe log were reverted to HEAD (`git checkout -- reference/fixtures`),
  the golden re-verified green. Lesson recorded for future sessions: the re-record step is DESTRUCTIVE
  to the goldens — run it only deliberately — and check `git status` after any command accident.

* `token_audit.py` FAILS on the pre-existing §18 `design/mockups/observatory-*.svg` +
  `display-sheet.svg` (resolved hex in generated SVGs). This predates INC1 — the crate adds no
  appearance values — and is out of INC1's scope; flagged for whoever owns the mockup gate.
* Commit early/often honoured: one increment commit (`7da006a`) + a seal commit (this state card,
  the CHECKLIST entry, the bundle). Bundle: `handoff/sparq-wo020-inc1.bundle` (+ sha256).
* No operator rulings were needed beyond the two that scoped this session (INC1-only, install+verify).
