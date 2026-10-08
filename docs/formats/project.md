# sparq project format — `.sparq` v0 (draft)

**Status:** draft for WO-011. Versioned from day 1; never write an unversioned container.

## Container

A `.sparq` file is a **content-addressed container**: a manifest plus a set of immutable parts, each stored under its BLAKE3 hash. Rationale: deduplication, corruption detection, trivially idempotent sync (plan §13.1), and stable references from journals and presets.

```
project.sparq
├─ header            magic "SPRQ", format version, engine version, created/modified, host platform
├─ manifest          ordered list of parts: {role, hash, size, media type}
├─ parts/
│  ├─ graph          the patch (text-diffable representation, see below)
│  ├─ scenes         scene list + scene matrix + macro definitions
│  ├─ params         parameter values + automation lanes (references graph node ids)
│  ├─ transport      tempo map, time signatures, loop regions, tick resolution
│  ├─ seeds          seed tree state (session seed + per-module allocations)
│  ├─ modules        exact module versions + manifest hashes required to open this project
│  ├─ assets         asset references by hash (never paths); missing ⇒ repair dialog
│  ├─ rigs           spatial rig presets referenced (layout, calibration, correction FIR hashes)
│  ├─ visuals        visual presets, colour map choices, display-slot config
│  ├─ controller_map source→target bindings (plan §7.4)
│  ├─ journal_head   pointer/hash of the journal segment this project continues
│  └─ meta           title, author, notes, tags, licence, provenance summary
└─ signature         optional, for shared/registry projects
```

**Writing is atomic:** write to a temp container, fsync, rename; keep the previous version as `.sparq.prev` for one generation. A partially written file is never observable.

## The graph part must be text-diffable

Requirement (WO-011): two versions of a project diff legibly in code review. Therefore the graph is stored as a deterministic, ordered textual structure — stable key order, one node/edge per logical line, ids sorted canonically, no timestamps or random ordering inside the part. Binary blobs (automation curve data, large tables) are separate parts referenced by hash so the diff stays readable.

Nodes carry: `id, module (id@version), position (grid units), params (id → value or automation ref), port connections, voice config, bypass/mute/lock flags, colour class override (never a literal colour — a token name)`.
Nodes MAY carry an optional `size = [w, h]` line (RESERVED, WO-020 INC6 D15): the instrument card's display-band size in world px, clamped to the ruled window (≥ 480×248, ≤ the manifest's declared `min_size`; the canvas snap grid on commit). Absent = the layout's default for the spec (an instrument defaults to HALF its declared face, ruling O-1). Instruments only — a backbone card's size is its spec's, never state. The in-memory model carries this today (`Node.size`, undoable via `Op::ResizeNode`); the line is cut into the format now so the container lands with the door already open.
Edges carry: `from (node.port), to (node.port), kind (audio|cv|event|data|gpu|atom), delay edge flag (unit_delay|block_delay), compensate flag`.

## Related formats

| Extension | Contents |
|---|---|
| `.sparqpatch` | a subgraph with an explicit port surface (reusable instrument/effect chain) |
| `.sparqpreset` | one module's parameter vector (+ morph-compatible siblings noted) |
| `.sparqpack` | shareable collection: samples/presets/patches/visuals + manifest + licences + provenance |
| `.sparqrig` | spatial rig: layout, per-speaker distance/azimuth/elevation/gain/delay/polarity, calibration IR hashes, correction FIR hashes, venue notes |
| `.sparqmap` | controller map: source→target bindings with amount/curve/bipolar |
| `.sparqjournal` | append-only session journal, chunk-checksummed |
| `.sparqbounce` | render sidecar: channel layout, sample rate/bit depth, loudness targets and measured values, provenance (project hash + journal hash + module versions) |
| `.sparqmod` | module package: manifest + binary/wasm/process bundle + assets + docs + example patch |

## Journal record types (v0)

`graph_mutation · param_change · automation_point · scene_launch · transport · seed_alloc · module_load · asset_ref · device_config · xrun · degradation · watchdog_bypass · stream_record (opt-in) · user_marker (take) · error`

Each record: `t_sample`, `t_wall`, `t_musical`, type, payload, and a running chunk checksum. Stream recording is **off by default** and shows an estimated size before enabling (ADR-007).

## Compatibility rules

* Opening a newer format version than the host supports ⇒ read-only mode with a clear message, never a silent downgrade.
* Missing/incompatible module ⇒ **repair dialog**: `stub` (preserves routing, distinct hairline in canvas) / `substitute` (host suggests an equivalent) / `cancel`. Never a crash, never a silent wrong sound.
* Migration runs the module state chains and the format chain, then re-saves, preserving the original file.
