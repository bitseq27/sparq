# ADR-010 — The instrument layer and the third-party hand-off system (decision D-14)

**Status:** accepted · **Date:** 2026-10-04 · **Related:** amends ADR-002 (T2 timing), ADR-003, ADR-005, `docs/api/module-api-v1.md` §10, `design/token-spec.md`, `MODULE-BUILD-GUIDE.md`

## Context

Operator ruling, 2026-10-04: the module library is **two layers, not one flat set**.

1. **Backbone modules** — basic utilities and control modules (plus basic sources): small, first-party, on the critical DSP path. They are the substrate everything else patches into. The existing 24-module set is already almost entirely this.
2. **Instruments** — the performance and control layer. Complex, visually interesting modules with graphical displays and unique control UI. Instruments are what a set is played on; backbone is what an instrument is built from.

And: instruments must be **hand-off-able**. Other people create them, simple or very complex, under tight design rules so they match the main app aesthetic; a finished instrument is **dropped into a folder in the project root and loads at launch without intervention**; and because the main app's UI will keep evolving, an added instrument must **pick up the updated design rules without intervention** too. The package is capped at **five files**, and the author-facing document is `MODULE-BUILD-GUIDE.md`.

This fires ADR-002's own review trigger — *"earlier if a collaborator needs to ship a module before then"* — so the T2 tier's Phase 5 date is amended here. Instruments also want 2D/3D rendering, simulation and assorted data formats, which the tier choice and the display contract have to absorb together.

## Decision

Seven parts, one system:

1. **Layer is manifest data.** `classification.layer = backbone | instrument` (default `backbone`). The taxonomy of categories (`syn`, `seq`, `gen`, …) is unchanged and orthogonal — layer says *how big a role the module plays*, category says *what it is*. The browser groups by it; Perform mode is built from instruments over backbone.

2. **Instruments run in the T2 sandbox** (WebAssembly, WASI 0.2 + Component Model, `wasmtime`, capability-scoped, fuel-metered) — pulled forward from Phase 5. Backbone stays T1 (native, first-party, golden-tested, per ADR-002's admission rule). Heavy non-real-time work (ML inference, video analysis, big simulations) stays T3 and may be *driven by* an instrument, not hosted inside one. A third party never ships native code into the live process: a bad pointer must not be able to kill a set.

3. **Displays are host-rendered scene data — instruments never own pixels.** The v1 contract (§10 of module-api, amended) gains two declarative vocabularies: a **display list** (2D: paths, polylines, rects, arcs, glyph runs, point clouds, heat cells) and a **scene descriptor** (3D: camera, points, lines, triangle meshes, heightfields). The instrument's draw export emits these per frame; the host renders them with its own WebGPU pipeline **using the current design tokens**. This is the mechanism that makes "the app changed its design, so your instrument changed" a property rather than a promise: there are no instrument-side colours, strokes or metrics to go stale. The custom-pixel escape hatch stays **first-party T1-only**; a badged third-party pixel surface is parked in `LATER.md`.

4. **Tokens cross the boundary at runtime.** At `prepare`, the host hands the guest a versioned **token bundle** serialised from the same `design/tokens/*.toml` that generates `tokens.rs`/`tokens.css` — one source of truth, now with a third consumer. Guest display code resolves *semantic token ids* (`color.signal.audio`, `stroke.hairline`, `space.4`); literal colours/sizes in any declared UI or emitted display list are a **validation error**, machine-checked, extending the token-spec §3 hard rules to guests.

5. **The package is five files, dropped in `instruments/` at the project root** and discovered at launch in module-api §11's existing precedence order, with its existing rule that a bad manifest never loads and every failure is reported verbatim:

   | # | File | Role |
   |---|---|---|
   | 1 | `sparqmod.toml` | the manifest: identity, layer, ports, params, panel, displays, capabilities, asset hashes |
   | 2 | `<name>.wasm` | the component; small assets embed in its data section |
   | 3 | `example.sparqpatch` | the instrument doing its job |
   | 4 | `preview.svg` | the library-card render, **validator-generated**, never hand-drawn |
   | 5 | `README.md` | what it is, what it needs, licence |

   Large asset packs do not break the cap: they are referenced **by hash** (manifest §7) from the content-addressed library and fetched on first load. The five-file cap applies to the distributed package; an author's *source* project is an ordinary cargo/wit project using the `sparq-module-guest` SDK.

6. **Hand-in is a gate, not a review queue.** `sparq mod validate <dir>` runs the whole conformance chain — schema validation, sandboxed instantiation smoke, golden render, real-time/fuel budget check, panel + display screenshot against the look-board audit, preview regeneration, file-count check — and emits a report in the existing actionable style (field, value found, values allowed, fix). A package that passes validation **is** a package that loads at launch; signing/badging follows plan §15 (unsigned loads badged, disabled by default in Perform mode). `sparq mod dev` runs the same chain headless — no core repo, no sound card — which is what makes authorship parallelisable.

7. **Core and instruments are built in parallel behind a frozen contract face.** The freeze set — contract v1.1 (layer field, display-list + scene vocabularies, token-bundle hand-off, WIT binding of the v1 module API), the package spec, `MODULE-BUILD-GUIDE.md`, the validator and the dev harness — lands first (WO-017/WO-018). After that, instrument authors never touch core code and core never touches instrument code: integration is mechanical (validator + conformance audit + golden renders + `host_api` pins + a nightly CI matrix running every handed-in package against latest core). The first one or two instruments are built **first-party, through the public path**, as reference implementations (WO-019) — the guide is proven before it is handed off. Contract evolution stays additive-only (pillar 4); removals/renames get the ≥2-minor deprecation window and alias table the plan already mandates.

## Consequences

* `wasmtime` enters the dependency tree — feature-gated (`instrument-host`) exactly like egui/wgpu, so the default zero-dependency build, the CI gates and every golden stay untouched. The wasm guest SDK is a separate published crate; the core workspace does not depend on it.
* Instrument audio costs ~1.2–2× native (ADR-002's measured expectation). That is acceptable for the performance layer sitting *above* the backbone; if an instrument's inner DSP is too hot, the answer is to factor the hot path into a backbone utility or petition for first-party T1 promotion — which, because the manifest is tier-independent, is a **packaging change, not a redesign** (ADR-002's own rule, now load-bearing).
* ADR-002's interim rule "no `gpu` module may be T2 or T3" is resolved rather than violated: instruments consume and emit *scene data* through the display contract; `gpu` ports and textures remain host-side objects.
* The display vocabularies become part of the frozen contract and inherit its versioning discipline: additive primitives are minor; removals are major with a deprecation window. They are audited like tokens — a display-list primitive nobody uses for two phases gets deprecated, not carried.
* Determinism (ADR-007) extends to guests: fuel is deterministic for identical code, RNG comes from the seed tree through the existing contract, and a golden render of an instrument is reproducible bit-exactly like any T1 module's.
* Risk accepted: freezing the display contract before the Phase 6 custom shell exists. Mitigation: the vocabularies are renderer-independent *data* (the same bet the token system already made), and the two reference instruments exist precisely to falsify the vocabulary early, while amending it is still cheap.

## Alternatives considered

* **Native dynamic libraries (cdylib) for instruments.** Full speed and direct GPU — and a crash in someone else's pointer arithmetic kills the live set, the C ABI becomes permanent the day the first instrument ships, and design-rule conformance degrades to trust. Rejected for the hand-off layer; this is exactly what the T2 sandbox exists for.
* **Instruments as T3 bridged processes.** Perfect isolation and full native power, but the performance layer wants block-rate coupling and hot reload at block boundaries; per-instrument OS processes make a dense patch a process farm. T3 stays where ADR-002 put it: the heavy, slow, non-real-time plane.
* **Host-provided pixel surface (wasm canvas / WebGPU pass-through).** Maximum author freedom, and it destroys the one property the operator ruling requires: when the app's design changes, pixels drawn by the guest do not follow. Also a shader-injection surface. Parked in `LATER.md` as a badged, look-board-reviewed escape hatch for a later contract version.
* **Hosting VST3/CLAP as the instrument format.** Already rejected as the native module system in ADR-002 (fixed in/out, parameter model too narrow, no data/geometry ports); still available later as an optional T3 wrapper.
* **Waiting for Phase 5 as ADR-002 scheduled.** Rejected: the hand-off system is now a product requirement, and every week of instrument authorship before the freeze adds packages built against a contract that does not exist yet.

## Review trigger

Before the first external hand-off (the guide, template and validator must have shipped one reference instrument through the full gate); if the display vocabularies prove insufficient for a commissioned instrument; if wasmtime's cost or the fuel watchdog misbehaves on the stage device; at Phase 6 when the custom shell lands and the renderer-independence of the display contract gets its real test.
