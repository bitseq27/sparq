# Instrument WIT contract — skeleton v0 (WO-017 freeze set, artefact 4)

**Status:** v0.2 **SKELETON**, drafted 2026-10-04 from the operator ruling (ADR-010 / D-14), then **aligned field-for-field against the native contract** the same day: `module::{BlockStatus, Resources, ModuleError, Oversampling, CvIn, CvOut}`, `params::ParamSet`, `event::{EventKind, Event}` in `crates/sparq-module-api/src/` are now mirrored exactly, and the checklist in §5 records what is settled versus what still lacks a native arbiter. It parses clean (official `wit-parser`, via `jco` 1.35 — see §7) and generates bindings for every interface, but it is **not frozen**: freeze happens at WO-017 close, after which this directory joins the plan's §6.7 machine-checked API-surface snapshot test.

**What this is:** the WebAssembly Interface Types binding of module-API v1 for the **instrument tier** — the ABI a handed-in instrument component speaks (ADR-010 decision 2). One contract, two bindings: the native Rust `Module` trait (`sparq-module-api`) for backbone T1, this WIT for T2 instruments. The manifest is tier-independent (ADR-002), so an instrument promoted to T1 is a packaging change, not a redesign.

**Companions:** [`../module-api-v1.md`](../module-api-v1.md) (the contract proper) · [`../manifest-schema.md`](../manifest-schema.md) · [`../../../MODULE-BUILD-GUIDE.md`](../../../MODULE-BUILD-GUIDE.md) (the author-facing text) · [`../../adr/010-instrument-layer.md`](../../adr/010-instrument-layer.md) · `reference/instrument-template/instrument.rs` (the SDK shape, Rust mirror of `guest`).

---

## 1. The world

```
world instrument                              (sparq:instrument@1.1.0)
│
├─ import tokens     token bundle at runtime ── control thread, fetched at prepare
├─ import host       clocks · seed tree · log · api-version
├─ import assets     capability-scoped reads of hash-declared assets ── control thread
├─ import sources    bound stream snapshots for displays ── UI thread
│
└─ export guest      id · prepare · activate · deactivate · configure · save-state
                     message · process (audio thread) · draw (UI thread)
```

Nothing else. **No WASI imports, no ambient authority** — capability-scoping is expressed by *absence*: what is not in this world does not exist for a guest (decision E).

## 2. File map

| File | Interface | Side | Owns |
|---|---|---|---|
| `wit/types.wit` | `types` | shared | semver, errors, block status, channel sets, resources, param/event/data values — the closed vocabularies |
| `wit/audio.wit` | `audio` | shared | per-block marshalling: `block-input` / `block-output`, audio & cv buffers, timed events |
| `wit/display.wit` | `display` | shared | display list v1 (2D) + scene descriptor v1 (3D) + `frame-context` + `surface` |
| `wit/tokens.wit` | `tokens` | import | token-bundle version + serialised bundle |
| `wit/host.wit` | `host` | import | three clocks, transport, seed-tree roots, diagnostics |
| `wit/assets.wit` | `assets` | import | `resource asset` + hash-declared open, capability errors |
| `wit/sources.wit` | `sources` | import | analysis-ring / scalar / data-window snapshots for `draw` |
| `wit/guest.wit` | `guest` | **export** | the lifecycle + `process` + `draw` the component implements |
| `wit/world.wit` | — | world | the `instrument` world: 4 imports, 1 export |

## 3. Design decisions (the skeleton's opinions, and where each is settled)

| # | Decision | Why | Settled by |
|---|---|---|---|
| **A** | **By-value marshalling** (canonical ABI lift/lower copies) for v1 block I/O | At 64-frame blocks the copies are hundreds of bytes and sit inside the fuel budget; a zero-copy path (host writing into exported guest memory) is a later *minor* — the record shapes don't change | WO-018 measurement on the stage device |
| **B** | **Appearance is unrepresentable**: every styled field is a `token-id` (semantic string) or a closed enum (`w1/w2/w3`, `c0/c2/c4`, dash patterns). There is no colour/size/duration literal field anywhere in `display.wit` | Turns token-spec §3 rule 1 from an audit into a *property*; `E-LITERAL-APPEARANCE` then only has to police manifests, never frames. This is ADR-010's "design changes propagate without intervention", mechanically | frozen here; validator (WO-018) enforces the id vocabulary against the bundle |
| **C** | **Determinism lives at the boundary**: `block-input.t-sample` is the only lawful clock inside `process`; the host re-stamps every outgoing `data-record`; there is no ambient RNG — guests take rooted seeds (`host.random-seed`, control-thread) and run their own deterministic PRNG | Journal replay stays bit-exact across the sandbox (ADR-007); a guest asking for fresh entropy per block would be a replay bug by construction | golden-render gate in `sparq mod validate` |
| **D** | **One component, two host-managed instances**: an audio instance (control + audio threads) and a display instance (UI thread), kept coherent by the same configure/state stream at block boundaries | A `draw` can never race a `process`; a wedged display instance can never xrun the audio; mirrors the shell's existing MTA/STA thread ruling | WO-018 loader design (boundary-swap engine integration) |
| **E** | **No WASI, capability by absence**; assets exist only as hash-declared, control-thread reads | The sandbox promise (ADR-010 decision 2) is only real if the world itself is minimal; data-format freedom comes from parsing entitled *bytes*, not from filesystem access | frozen here |
| **F** | **Scalar-only param snapshots**; `text`/`blob` params ride `configure`/`message` on change | Not an opinion — native proof: `ParamSet` is a fixed `[f32; 32]` array with a version. `bool` rides 0.0/1.0, `enum` its option index; the manifest's declared domain gives each f32 its meaning. The snapshot must stay fixed-shape and atomically swappable | **settled** by `params.rs` (alignment pass 2026-10-04) |
| **G** | **Fail soft at runtime, fail loud at hand-in**: unknown token ids / kind mismatches skip the primitive with a diagnostic live, but are hard `sparq mod validate` failures | The show goes on; the bad package never ships | WO-018 validator |
| **H** | **One world for the tier** (`instrument`); backbone never becomes wasm (T1 admission rule, ADR-002); a second world would arrive as a *minor* | Keeps the package id honest and the freeze surface small | ADR-010 |

**Versioning (load-bearing, because WIT is stricter than it looks).** In the canonical ABI, adding a variant case or record field to an *existing* interface breaks old guests — so "additive-only evolution" (pillar 4) means: **within a major, existing interfaces are frozen byte-for-byte**; growth happens by *new interfaces* (a `display2`, a `sources` extension), *new optional imports*, or *new worlds*, with the old shapes untouched and `host_api = {min, max}` negotiating which a component speaks. Removals/renames are majors with the plan's deprecation window and alias table. The package semver (`@1.1.0`) tracks the contract version, not a release cadence.

## 4. Mapping: WIT ↔ native contract ↔ manifest

| Native (`sparq-module-api`) | WIT | Manifest anchor |
|---|---|---|
| `Module::id()` + registration cross-check | `guest.id()` (host compares, refuses on disagreement) | `identity.id` |
| `prepare(&Resources)` (resolved per-port channel COUNTS; `ChannelSet` resolves host-side) | `guest.prepare(resources)` — same fields + `fuel-per-block`, `token-bundle` | `resources.*`, `ports[].channel_set` |
| `process(&mut AudioCtx) -> BlockStatus` | `guest.process(block-input) -> block-output` | `ports[]`, `params[]` declaration order |
| `ParamSet` snapshot ring (version + `[f32; 32]`, block boundary) | `block-input.params` → `param-set {version, values}` (decision F, native-proved) | `params[]` declaration order |
| `CvIn` (`Unconnected`/`Block`/`Audio`) · `CvOut` (`Block`/`Audio`), host-performed `cv_interp`/`cv_reduce` | `cv-in-buf.{unconnected, per-block, per-frame}` · `cv-out-buf.{per-block, per-frame}` | `ports[].cv_interp`, `ports[].cv_reduce`, `ports[].rate` |
| `event::Event` (`Copy`: kind/sample/channel/value/words[4]), pre-sorted by offset (§9), `EventBuf` drop counting | flat `event` record, `events-in` host-sorted; `events-out` re-sorted + drop-counted | `ports[].event_kinds` |
| data records, `t_wall`, stale policy (§7.2) | `block-input.data-in` / host-stamped `data-out` | `ports[].data_schema`, `read_policy`, `stale_policy` |
| `configure(&[u8])` / state serialisation | `guest.configure` / `guest.save-state` (deterministic bytes) | `state.schema_id`, `schema_version`, `size_class` |
| `message(&[u8])`, honest refusal | `guest.message` → `module-error.{message, unsupported}` (native's own cases) | `atom` ports |
| `activate` / `deactivate` (trait defaults) | `guest.activate/deactivate` (SDK defaults — a WIT world has no optional exports) | `lifecycle.*` |
| seed tree allocation (§5.6) | `host.random-seed(stream-name)` | — |
| three clocks (ADR-006) | `host.now()` → `time-info`; `block-input.t-sample`; `frame-context.time-sec` | — |
| `BlockStatus::{Ok, Silenced, Overrun, Failed}` + watchdog auto-bypass (ADR-009 d6) | `block-status` — all four; a fuel-exhaustion trap maps to `overrun` | `capabilities.max_fuel`, `max_cpu_ms_per_block` |
| analysis rings (WO-012 inc 5) | `sources.snapshot(display, source)` | `ui.displays[].sources` |
| `tokens.rs` (generated) | `tokens.get-bundle()` (serialised, versioned) | `design/tokens/*.toml` |
| panel descriptor (host-drawn widgets) | *not in WIT* — panels stay manifest data, host-rendered | `ui.panel` |
| `custom_draw` (T1 first-party only) | *not in WIT* — instruments get `guest.draw` → `surface` instead | `ui.displays[]` |
| `gpu` ports (host-side objects) | *deliberately absent* (ADR-010 resolves module-api §16.5) | `ports[].type = gpu` (T1 only) |

## 5. Freeze checklist — the WO-017 alignment pass

**Settled by the alignment pass (2026-10-04)** — arbiter: `crates/sparq-module-api/src/` as at commit `1f7d814`:

- [x] `block-status` = `ok | silenced | overrun | failed` — `module::BlockStatus`, all four variants, native doc semantics verbatim; the wasm fuel-trap → `overrun` mapping recorded (types.wit).
- [x] `module-error` = `unsupported | state | resources | message` — `module::ModuleError`, same four cases; string reasons legal because every return path is control-thread (types.wit).
- [x] `resources` mirrors `module::Resources` field-for-field: `sample-rate: u32` (not f64 — v0.1 was wrong), `block-frames`, resolved per-port channel COUNTS (`audio-in-channels`/`audio-out-channels`, 0 = unconnected), `oversampling`, `voices`, `arena-bytes`. The v0.1 `channel-set` variant is **removed** — `ChannelSet` is manifest vocabulary that resolves host-side before `prepare`; guests see counts, exactly like native. `fuel-per-block` + `token-bundle` are the only tier additions, and they are labelled as such.
- [x] `param-set` = `{version: u64, values: list<f32>}` — `params::ParamSet` proved decision F: the snapshot is a fixed f32 array in manifest order (cap `MAX_PARAMS = 32`, manifest-validated as `E-CROSS-FIELD:params-cap`); `bool` rides 0.0/1.0, `enum` its option index; `text`/`blob` cannot ride it and arrive via `configure`/`message`. The v0.1 `param-value` variant is **removed** — native has no per-value type tag, and inventing one would have been drift.
- [x] `event` = flat `{kind, sample, channel: u16, value: f32, words: (u32,u32,u32,u32)}` — `event::Event` verbatim, including the per-kind semantics table copied from `event.rs`; `event-kind` in ADR-005 order. The v0.1 rich payload variants (`gate-payload`, `note-payload`, …) are **removed**: native is deliberately flat and `Copy`, and the WIT mirror must be too. Host re-sorts `events-out` and counts drops like `EventBuf` (`dropped`/`dropped_total`).
- [x] `cv-in-buf` gains the missing third state — `module::CvIn` is `Unconnected | Block(f32) | Audio(&[f32])`, and "an explicit signal, never 0.0-by-accident" is a contract property, not a comment. `cv-out-buf` **added** (v0.1 had no cv output path at all; native `CvOut` does).
- [x] Caps cited from native constants: `MAX_PORTS_PER_CLASS = 8` per type per direction, `MAX_PARAMS = 32` (types.wit/audio.wit comments).
- [x] Voices: the count rides `resources.voices`; per-voice param delivery does not exist in the native snapshot either — it defers *with* module-api §7 (Phase 1 voices), so it is not a WIT gap.

**Still open at freeze (WO-017 close / WO-018):**

- [ ] `data-value`/`data-record`: the one vocabulary with **no native arbiter** (`data` ports are specified in ADR-005/§7.2 but unimplemented in `sparq-module-api`). Ratify as-is or reshape when the native data-port work lands — marked inline in types.wit.
- [ ] `time-info`/`transport-state` fields vs `sparq-kernel`'s transport (WO-009 as built): tempo/bar/beat representation, and whether `t-wall-ns` belongs in the guest world at all given the host-stamping rule.
- [ ] The six new validation codes (manifest-schema catalogue) wired to these exact shapes: `E-LAYER-MISMATCH`, `E-PACKAGE-FILECOUNT`, `E-LITERAL-APPEARANCE`, `E-DISPLAY-PRIMITIVE-UNKNOWN`, `E-CAPABILITY-UNDECLARED`, `E-TOKEN-BUNDLE-VERSION` (WO-018 validator).
- [ ] `gpu_class` → vertex/instance/cell ceiling table (renderer side, WO-018).
- [ ] Two-instance coherence (decision D): exact ordering of `configure` application across audio/display instances at the boundary swap (WO-018 loader).
- [ ] `wasmtime` version pin + component-model/WASI baseline recorded (WO-018 `Cargo.toml`); token bundle `bundle-encoding` confirmed as JSON-of-`tokens.json`.
- [ ] This directory added to the machine-checked API-surface snapshot test (plan §6.7) so an accidental WIT edit fails CI (WO-017 close).

## 6. What is deliberately NOT here

* **Pixel surfaces, shaders, textures, lights** — ADR-010 decision 3; the badged escape hatch is parked in `LATER.md`.
* **`gpu` ports** — host-side objects; instruments speak scene data (module-api §16.5, resolved).
* **Zero-copy buffer sharing** — decision A; measured first, added as a minor if needed.
* **WASI clocks/filesystem/sockets** — decision E. If a guest needs the network or a process, that is the T3 plane, not a wider world.
* **A registry/publish ABI** — Phase 5 (plan §13.4); hand-in is a folder and a validator until then.

## 7. Verification

Parsed and binding-generated with the official `wit-parser` (via `@bytecodealliance/jco` 1.35.0) on 2026-10-04 — all eight interfaces + the world, doc comments carried into the generated bindings. Re-verified after the same day's alignment pass (v0.2): the parser caught two real bugs during drafting (doc comments on repeated `package` items are rejected; `stream` is a WIT keyword — the seed function's parameter is `stream-name`), which is the point of checking rather than eyeballing:

```sh
npx jco types docs/api/instrument-wit/wit --world-name instrument   # parse + generate TS bindings
# equivalent once the Rust side exists:
wasm-tools component wit ...   # or cargo component check in the guest SDK project
```

CI gate from WO-017: parse the world, diff the generated surface against the checked-in snapshot (the same discipline as `tokens.rs` staleness — generated artefacts are checked in and CI fails if they drift).
