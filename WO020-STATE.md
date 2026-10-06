# WO020-STATE.md — state record (The Observatory, WO-020) — **INC1–INC4 CLOSED, built and measured green**

**Last touched:** 2026-10-06, delivery session (same sandbox, after the INC4 seal) — **INC1–INC4
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
