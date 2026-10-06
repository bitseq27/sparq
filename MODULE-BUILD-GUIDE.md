# sparq module build guide — instruments and the hand-off system

**Audience:** you, building an **instrument** for sparq — as a hand-off from the core team, or as a third party who has never seen the inside of the engine. **Version:** v1.0 — **frozen with contract v1.1** at WO-017 close (2026-10-05; ADR-010, WO-017). **Status of the machinery:** the contract documents are frozen and machine-pinned; the loader, validator and dev harness are ticketed (WO-018/WO-019) — until they ship, this guide is the specification they are built to.

**The test this document has to pass** (inherited from module-api §15): a simple module is a **two-hour** task and a complex instrument is a **two-day** task. If any step below needs engine knowledge, the contract has failed and *this document gets amended* — that is the rule, not a courtesy.

Companion documents: [`docs/module-author-guide-v0.md`](docs/module-author-guide-v0.md) (the DSP contract in depth, written for first-party backbone modules but normative for instruments too) · [`docs/api/module-api-v1.md`](docs/api/module-api-v1.md) · [`docs/api/manifest-schema.md`](docs/api/manifest-schema.md) · [`design/token-spec.md`](design/token-spec.md) · [`design/look-board.md`](design/look-board.md) · [`docs/adr/010-instrument-layer.md`](docs/adr/010-instrument-layer.md) · skeleton package in [`reference/instrument-template/`](reference/instrument-template/).

---

## 1. The two-layer library, and where you fit

sparq's library is two layers:

* **Backbone modules** — small utilities, control modules and basic sources (`util/gain`, `mod/lfo`, `flt/svf`, …). First-party, native (T1), on the critical DSP path. They are the substrate.
* **Instruments** — the performance and control layer. Complex, visually interesting modules with graphical displays and unique control UI: a grid sequencer you play with both hands, an FM voice with a 3D terrain view, a generative rhythm engine with walking displays. **This is what you are building.**

An instrument is one node in the graph. It speaks to everything else through typed ports (`audio`, `cv`, `event`, `data`) exactly like a backbone module — it is bigger, it looks like more, and it runs in the sandbox, but it is not a different species. The port/param/state contract you read in the module API is the whole contract; nothing in this guide replaces it.

Instruments run as **WebAssembly components** (T2 tier, ADR-002/ADR-010): crash-isolated, fuel-metered, capability-scoped, hot-reloadable at block boundaries. You may write one in any language that compiles to the component model — Rust is the shortest path because the `sparq-module-guest` SDK exists.

## 2. The one rule: the host owns the look

You declare **structure**; the host decides **appearance**.

* Your panel is data: widgets from a closed vocabulary on an 8 px grid, each with a touch class. The host draws it with its widget set.
* Your graphical displays emit **scene data** — display lists (2D) and scene descriptors (3D) — not pixels. The host renders them with its own GPU pipeline.
* Every colour, stroke width, corner radius, font size, spacing and motion curve resolves from **design tokens handed to you at runtime**. You write `color.signal.cv`, never `#4FD8E8`.

This is the deal that makes the system work for you: when the main app's design changes — new palette, new shell renderer, new widget chrome — **your instrument follows automatically, with zero intervention on your side**, because there is no copy of the aesthetic inside your package to go stale. In exchange, literal appearance values anywhere in your package are a **validation error**, machine-rejected at hand-in. The eleven hard rules of `design/token-spec.md` §3 apply to you in full; §9 below restates the ones that bite instrument authors most.

## 3. The package: five files, dropped in a folder

A handed-in instrument is a directory of **no more than five files**, dropped into `instruments/` at the project root (or the per-user instruments directory). It is discovered at launch — no registration, no config edit, no restart ritual, no intervention:

```
instruments/
└─ my-fm-terrain/
   ├─ sparqmod.toml        # 1 — the manifest (§4)
   ├─ fm-terrain.wasm      # 2 — the component; small assets embed in its data section
   ├─ example.sparqpatch   # 3 — your instrument doing its job
   ├─ preview.svg          # 4 — the library-card render (validator-generated, never hand-drawn)
   └─ README.md            # 5 — what it is, what it needs, licence
```

Rules at the door, all pre-existing contract behaviour (§11 of the module API): a bad manifest never loads; one broken package cannot hide the others; every failure is reported verbatim in the module browser with the file it came from. Conflicts resolve by explicit version pin in the project, never by "latest wins".

**The cap governs the distributed package, not your source project.** Your source is an ordinary project (cargo + the guest SDK, or any component-model toolchain) with as many files as you like; `sparq mod package` emits the five-file directory from it.

**Bulk assets do not break the cap.** Sample libraries, wavetable banks, models and other large data are referenced **by hash** in the manifest and fetched from the content-addressed library on first load (§13.1 of the plan). Small assets — a few LUTs, a default preset bank — embed in the wasm data section and need no declaration beyond their hashes.

## 4. The manifest: what an instrument adds

You write the same `sparqmod.toml` every module writes (full schema: `docs/api/manifest-schema.md`). The instrument-relevant parts:

```toml
[classification]
category = "synth/fm/terrain"        # taxonomy unchanged — layer is orthogonal to category
top = "syn"
kind = "source"
tier = "t2"                          # the sandbox
layer = "instrument"                 # ← the field that makes it an instrument
stability = "experimental"

[identity]
host_api = { min = 1, max = 1 }      # your pin; the host refuses incompatible loads with words

[ui]
# panel{ widgets[], layout } — closed vocabulary: slider knob xy_pad matrix enum_select
#   toggle numeric_entry label meter_attach display_slot button waveview grid_pad
# displays[] — your graphical surfaces (§5)

[capabilities]                       # enforced, not advisory, in T2
fs_read = []                         # asset scopes only; declare or don't read
network = "none"
max_fuel = 2_000_000                 # per block; the watchdog's budget, not a suggestion
max_memory_mb = 64

[resources]
cpu_class = "medium"
gpu_class = "light"                  # scene-data budget class the renderer schedules against
```

Signing follows plan §15: an unsigned package loads **badged** and is disabled by default in Perform mode. You don't need a signature to be usable; you need one to be stage-ready.

## 5. Displays: the two vocabularies

A display is declared in the manifest (`ui.displays[]`: `id`; `kind` — `display_list` (2D) or `scene` (3D); source bindings `sources[] = {id, port|stream}` — you name the binding and the port/analysis it reads (`port`) or the live broker stream it reads (`stream`, below), and your `draw` fetches snapshots by `(display-id, source-id)`; `colormap` — a colormap token id, wherever colour encodes data; `min_size = [w, h]` in logical px; `lod = auto|full|reduced|minimal`) and drawn by your per-frame draw export, which receives the frame rect, the UI LOD level, the bound stream snapshots and the token bundle, and writes a **display list** or **scene descriptor** into the frame buffer the host provides. The exact ABI is WIT-frozen in contract v1.1 (**frozen 2026-10-05 at WO-017 close**, hash-pinned in CI): [`docs/api/instrument-wit/`](docs/api/instrument-wit/README.md); the vocabularies are:

**Display list v1 (2D)** — every primitive resolves style from tokens; you pass semantic ids, never values:

| Primitive | Notes |
|---|---|
| `path` / `polyline` | stroke token (`stroke.hairline`, `signal.audio`, …), width class 1/2/3, dash pattern for non-colour encoding |
| `rect` / `arc` | fill or hairline stroke; corners 0/2/4 px only |
| `glyph_run` | text via the host type scale, tabular numerals, **always with units** |
| `points` | point cloud; size class, colour by signal class or colormap-by-data-kind |
| `heat_cells` | gridded magnitude/signed data filled from a declared **colormap token** (`colormap.spectrum.sonogram`, …) |
| `trace` | rolling/phosphor trace with the host's motion tokens (trace decay is host-side) |

**Scene descriptor v1 (3D)** — you describe a scene; the host renders it (camera included) with tokened materials and the project's colour-map bindings:

| Element | Notes |
|---|---|
| `camera` | one per scene; orbit/orthographic; the host clamps to the display frame |
| `points` / `lines` | vertex buffers with per-vertex data channels bound to colormaps |
| `mesh` | indexed triangles; flat or data-shaded; no custom shaders |
| `heightfield` | grid + height channel — the terrain/wavetable/function case |

Budget: vertex/instance caps come from your declared `gpu_class`; over budget, the host degrades LOD, and at Full-LOD refusal your display shows *words*, not garbage. A colormap sample is a **data encoding, never a style** (token-spec rule 11): it may fill a heat cell or stroke a data trace; it may never colour text or chrome.

**Live data streams (WO-020 / ADR-011).** When a display shows *live external data* — space weather, earthquakes, satellites, weather, raw bulletins — bind its source with `stream` instead of `port`. The guest has **no network capability** (the T2 world has no socket door), so the fetching lives in a host-side **broker** (`sparq-streams`): it polls a checked-in registry of free public feeds on polite cadences, normalizes each into the *same* frozen `data-record` vocabulary, and serves it through the *same* `sources.snapshot(display-id, source-id)` door — your `draw` reads a stream window exactly like a data-port window. A `stream` value is either a **registry id** (`stream = "swpc.kp"`, a fixed feed) or a **`param:` indirection** (`stream = "param:cell_01"`, which binds whatever stream that `enum` param currently selects — how a per-cell dropdown swaps a feed at runtime from a *static* manifest, host-resolved at snapshot time). The registry — the single source of truth for stream ids, cadences and schemas — is **`crates/sparq-streams/streams.toml`**; an id that is not there is refused `E-STREAM-UNKNOWN`, so a new feed is a registry row (+ a normalizer), never a hard-coded URL in your instrument. Streams never reach audio or the journal (the determinism firewall): goldens render from recorded fixtures, and a network outage changes only *what is displayed* (STALE → OFFLINE, in words), never the set playing.

**What you cannot do, on purpose:** draw raw pixels, bring your own shaders, or emit a literal colour. These are the properties that keep a hundred third-party instruments looking like one product — the same reason the widget vocabulary is closed. If a commissioned instrument genuinely needs pixels, that is the parked escape-hatch conversation in `LATER.md`, not a field in your manifest.

## 6. Audio, logic, simulation

The real-time contract is unchanged and applies inside the sandbox: **all allocation in `prepare`, none in `process`**; no I/O on the audio path; parameters arrive as a `Copy` snapshot at block boundaries; events arrive pre-sorted by sample offset; RNG comes from the host seed tree, so your instrument is bit-reproducible under journal replay like everything else. The host's fuel meter is the watchdog: an instrument that blows its per-block budget is auto-bypassed (existing executor behaviour), and repeated bypasses surface in the validator's report.

* **Simulation inside the instrument** — physics, particle systems, generative grids, CA/L-system/chaos engines — is ordinary wasm compute. Deterministic given the seed tree; budget it against `max_fuel`.
* **Heavy or non-real-time work** — ML inference, video analysis, network fetching, minutes-long baking — does **not** go inside the instrument. It belongs to the T3 bridged-process plane; your instrument consumes its `data` streams. The host validates that a T2 package declares no capability it cannot honour.
* **Data formats** — parse anything you like *inside the sandbox*: MIDI files, Scala/KBM tunings, JSON/CSV tables, WAV/SoundFont-class sample data. File access is capability-scoped to declared asset hashes and (where granted) declared paths; there is no ambient filesystem. **`data` ports, honest status at v1.1:** the vocabulary is frozen (WIT `types.data-value`/`data-record`, ratified 2026-10-05), but the host does not ROUTE data ports yet — an instrument declaring one is refused at load **in words** until the native data-port work lands (Phase 5). Parse your files from assets; don't declare a `data` port before then.

## 7. Hand-in: the validation gate

Hand-in is mechanical. You run the same gate the host runs at launch:

```
sparq mod validate instruments/my-fm-terrain/
```

The chain, in order — every failure actionable (field, value found, values allowed, the fix):

1. **Schema** — manifest validates; `layer = instrument`; `tier = t2`; unknown keys rejected; `host_api` compatible.
2. **Package** — ≤ 5 files; file roles recognised; asset hashes present and matching.
3. **Smoke** — the component instantiates in the sandbox; exports match the WIT contract; capabilities honoured; no ambient authority.
4. **Golden render** — your declared render case through the null device, twice, bit-exact; hash recorded. This is your instrument's regression anchor forever.
5. **Budgets** — fuel per block, memory, `prepare` cost within declared classes; the real-time property test from the module guide §10.
6. **Visual conformance** — panel + every display rendered headless at the three breakpoints; screenshot audit against `design/look-board.md`; literal-appearance scan; touch targets ≥ 44 px; `preview.svg` regenerated.
7. **Signature/badge** — signed → clean load; unsigned → badged, Perform-disabled.

A package that passes validate **is** a package that loads at launch. There is no human queue between the two; the gate *is* the review.

## 8. Building in parallel: the dev harness

You do not need the core repo, a sound card, or the app to be finished:

```
sparq mod dev instruments/my-fm-terrain/     # validate + golden render + panel SVG + fuel report, headless
sparq mod new my-fm-terrain                  # scaffold a source project from reference/instrument-template/
```

The harness runs your instrument against the null device and the headless painter (the same machinery as `sparq ui --svg-out`), so you can iterate on audio and visuals in a tight loop anywhere. CI on the core side runs a **nightly matrix of every handed-in package against latest core** — that matrix, plus your `host_api` pin, is the entire integration surface between you and the core team. Nobody merges anybody's code; the contract is the only shared object.

What this means practically: **core and instruments are built simultaneously.** The core team owes you a frozen contract (v1.1: this guide, the vocabularies, the token bundle, the WIT binding, the validator); you owe the core team a package that passes the gate. Between those two facts, work is parallel.

## 9. The design rules that bite (restated from token-spec §3 — all validator-enforced)

1. No literal colours, sizes, spacings, radii or durations. Token ids only.
2. Stroke widths are 1, 2 or 3. Corners are 0, 2 or 4 px. Circles only for knobs and ports.
3. No drop shadows, no decorative gradients; gradients exist inside colormaps and meter fills only.
4. Accents mean **signal class** (audio, cv, event, data, gpu) — never your taste, never your brand.
5. Every colour state also carries a non-colour encoding (dash, hatch, shape, label).
6. Every interactive element ≥ 44 px touch target (the 32 px dense exception is Design-mode-only and flagged).
7. Every number renders with its unit, tabular numerals, per the host's numeric formatting rules.
8. Colormaps are chosen by data kind (magnitude → sequential, signed → diverging, category → qualitative) and never style text or chrome.
9. `#FFFFFF` does not exist; `#000000` exists only in the high-contrast theme and full-bleed visuals.

The look-board is the conformance authority: when in doubt, your surface must be able to sit next to the canonical screenshot set without obvious dissonance. A screenshot with no title should read as scientific software — that is the aesthetic you are handing into, and the validator is how it stays true with a hundred authors.

## 10. Versioning: what "without intervention" covers

* **Covered, automatically:** re-theming (token value changes), widget chrome changes, the Phase 6 renderer swap (egui → custom shell), layout/LOD policy changes, host-side rendering of your displays. Your package is untouched; it just looks current.
* **Your responsibility:** keeping your manifest valid against the contract version you pinned (`host_api min/max`). Contract evolution is **additive-only** within a major; removals/renames get a ≥ 2-minor deprecation window and an alias table, announced in the guide's changelog. On a major bump you re-run `sparq mod validate` and fix what it names — the report, not guesswork, is the migration tool.
* Display-vocabulary and token-bundle versions are semver'd like the API: additive primitives are minor; the nightly matrix catches drift before you feel it.

## 11. Promotion, and the escape hatches

* **Too slow for the sandbox?** Factor the hot path into a backbone utility (a hand-in to the core team), or petition for **T1 promotion** — because the manifest is tier-independent, promotion is a *packaging* change, not a redesign.
* **Need pixels?** Not today. The badged custom-surface hatch is parked in `LATER.md` behind a look-board review; propose it there with the instrument that needs it.
* **Need the network, a device, a process?** Capabilities are declared and granted per package; undeclared is unavailable. T3-side helpers (ML, video, scrapers) are a core-team conversation, not a manifest field.

## 12. Changelog

| Version | Date | Change |
|---|---|---|
| v1 draft | 2026-10-04 | First cut from the operator ruling of 2026-10-04 (ADR-010, WO-017): two-layer library, five-file package, host-rendered display vocabularies, runtime token bundle, validate/dev gate. Supersedes nothing — `docs/module-author-guide-v0.md` remains the DSP-contract text for backbone modules and is normative here by reference. |
| v1.1 draft | 2026-10-06 | WO-020 / ADR-011 (the stream plane), §5: a display source may bind `stream` (a broker stream id, or `param:<id>` for a runtime-selectable feed) instead of `port` — mutually exclusive, host-resolved, **WIT unchanged**. `visible_if` on panel widgets is specified (`{param, equals}`, one equality on one param). New code `E-STREAM-UNKNOWN`; the registry is `crates/sparq-streams/streams.toml`. Additive: no existing instrument changes. |
