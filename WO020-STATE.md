# WO020-STATE.md — state record (The Observatory, WO-020) — **INC1 + INC2 CLOSED, built and measured green**

**Last touched:** 2026-10-06, this build session (Linux sandbox, ~1 GB / 2 cores). Plan of record:
`WO020-OBSERVATORY-PLAN.md`. Operator rulings for this session (via the question tool): **build
INC1, then continue into INC2**, and **install Rust + compile/test for real** (not source-only).
INC3–INC6 are NOT started; INC5 is device-only by construction and INC6 is deferred/gated.

> **Environment note (survives a re-read):** the sandbox snapshot does NOT persist the installed
> toolchain or `.git`. This session re-installed rustup (via python — no curl/wget) + `gcc` (apt),
> and **recovered `.git` by re-cloning origin (which still serves the base `48d3074`) then fetching
> `handoff/sparq-wo020-inc1.bundle`** — the bundle discipline is what made the INC1 history
> recoverable after the snapshot dropped `.git` (and one working-tree file, `modules/out/main/`,
> restored from HEAD). The resume recipe below re-installs both. **Commit + bundle every increment:**
> the bundle in `handoff/` is the durable artefact, not `.git`.

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

* **INC3** — the guest source project (`instruments-src/observatory/`: core/wasm/harness), the seven
  renderers, `make_coastline.py`, `gen_manifest.py` (which must emit VALID units/defaults — see the
  two §10 gaps above), the five-file package, the wasm build attempt. The full 16-cell Observatory
  manifest (only a representative subset is proven here) is generated in INC3.
* **INC4** — host painters (`displaylist.rs` + SVG/egui), the `ReplayProvider` wiring (replay.rs is
  ready for it), the dropdown picker, camera FOCUS, canvas card sizing (`node_width_instrument_max`
  is now in the token bundle for it).
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
# 3. Gates:
export PATH="$HOME/.cargo/bin:$PATH" CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup"
cd /home/user/sparq
cargo test --workspace                                   # 971 / 0 / 1
cargo test -p sparq-streams --features streams           # 75 / 0
cargo clippy --workspace --all-targets -- -D warnings
cargo build -p sparq-app --features streams && target/debug/sparq streams list
# 4. Re-record fixtures only if the network is up and drift is suspected (they are checked in):
python3 tools/streams_record.py --all --out reference/fixtures/observatory/
```

## House notes

* `token_audit.py` FAILS on the pre-existing §18 `design/mockups/observatory-*.svg` +
  `display-sheet.svg` (resolved hex in generated SVGs). This predates INC1 — the crate adds no
  appearance values — and is out of INC1's scope; flagged for whoever owns the mockup gate.
* Commit early/often honoured: one increment commit (`7da006a`) + a seal commit (this state card,
  the CHECKLIST entry, the bundle). Bundle: `handoff/sparq-wo020-inc1.bundle` (+ sha256).
* No operator rulings were needed beyond the two that scoped this session (INC1-only, install+verify).
