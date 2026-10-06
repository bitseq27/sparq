# ADR-011 — The stream plane: host-side broker and stream-source bindings (WO-020)

**Status:** accepted · **Date:** 2026-10-06 · **Related:** extends ADR-010 (the instrument layer), ADR-007 (determinism/journal), ADR-005 (port types); amends `docs/api/manifest-schema.md` §9, `docs/api/manifest-fields.toml`, `MODULE-BUILD-GUIDE.md` §5; implemented by `crates/sparq-streams` + the WO-020 validator additions

## Context

The Observatory (WO-020, the first instrument) is a wall of **live** Earth-and-space data: aurora grids, earthquakes, solar wind, satellite positions, weather, raw-text bulletins. An instrument is a T2 wasm guest, and the T2 world has **no network door** — capability by absence (ADR-010 decision 2, instrument-wit decision E). So the live data cannot be fetched by the guest. It has to arrive from the host, through a door the frozen contract already has.

Three constraints shaped this:

1. **The guest never touches the network.** Not a policy — a property of the world. The validator already honours only `network = "none"` for T2; there is no socket to declare.
2. **The contract is frozen at v1.1** (`docs/api/instrument-wit/wit/*` is hash-pinned by `wit_snapshot.rs`). WO-020 runs while amendment is still cheap (ADR-010's own review trigger), but an amendment is a last resort, not a first move.
3. **Determinism is non-negotiable** (ADR-007). Live data must have no path to the audio thread, the journal, or a golden render — or replay diverges and the reproducibility promise is broken.

The question: how does live external data reach an instrument's display without a guest network capability, without moving the frozen WIT, and without touching the audio path?

## Decision

**Live external data is a host service, delivered through the contract's existing `sources` door, bound by a new manifest key.** Five parts:

1. **A host-side broker, not a guest capability.** A new crate `sparq-streams` (the *stream plane*) polls a checked-in registry of public feeds on polite cadences, normalizes each into the frozen `data-record` vocabulary (`types.wit`), keeps rolling windows + a disk cache, and serves them on the **UI/control thread**. The guest never sees a socket; the broker is where the network lives. Its hermetic half (registry, normalizers, windows, cache, replay) is dependency-free; the live transport (`ureq` + TLS) rides a `streams-net` feature and is a device increment.

2. **Stream-source bindings: one additive manifest key, WIT untouched.** A display source binding (`ui.displays[].sources[]`) already carries `{id, port}`. This ADR adds **one optional key, `stream`**, mutually exclusive with `port`:

   ```toml
   [[ui.displays.sources]]
   id = "cell-07"
   stream = "swpc.kp"          # a fixed registry id …
   # — or —
   stream = "param:cell_07"    # … whatever enum param `cell_07` currently selects
   ```

   The guest keeps calling `sources.snapshot(display-id, source-id)` with the **same opaque strings**; the host resolves the `stream` binding to a window at snapshot time (it owns both the registry and the `ParamSet`, so the `param:` indirection — per-cell runtime stream selection — costs **zero** WIT change). `window(list<data-record>)` already exists in `stream-snapshot`. This is the whole trick: **runtime-selectable live streams from a static manifest and a frozen contract.**

3. **The registry is checked-in data, shared by three readers.** `crates/sparq-streams/streams.toml` is the single source of truth for stream ids. The broker reads it (what to poll), the validator reads it (does a binding name a real stream? → `E-STREAM-UNKNOWN`), and the manifest generator reads it (the enum option walls are emitted from it, never hand-typed). A stream catalogue rots (the DONKI lesson), so the registry is *probed*, recorded as fixtures, and re-recordable in one command — drift becomes a recorded fact, not a draw-time surprise.

4. **Keys are env-scoped and optional.** NASA endpoints run on `DEMO_KEY`; `SPARQ_NASA_API_KEY` raises the ceiling; `SPARQ_FIRMS_KEY` (free) lights FIRMS. A stream whose key is unset is **not fetched** and reports **KEY NEEDED in words** — never a silent hole, never a crash. No config-file surface in v1.

5. **The determinism firewall is a property, not a comment.** Stream data reaches only the UI thread's `sources` door. It has **no path** to `process`, the audio thread, the journal, or a golden. Golden renders run from **recorded fixture windows** with `time-sec` pinned, bit-exact twice. A network outage changes *what is displayed* (STALE → OFFLINE, in words) and nothing else — the set keeps playing, the render stays deterministic. The broker reads a wall clock (to stamp `age_s`); the guest does not (its timestamps are the records' own host-stamped UTC).

### Why no WIT amendment was needed (the §7.4 proof, recorded so the next instrument does not re-litigate it)

`stream` is a **manifest** key resolved **host-side**. `sources.snapshot` keeps its opaque `(display-id, source-id)` signature — the source-id is already an arbitrary string, and the host already decides what a binding resolves to (a port's ring today, a broker window now). `stream-snapshot.window(list<data-record>)` already exists. The display-list vocabulary already covers every renderer the Observatory needs. So the frozen `wit/*` bytes are **hash-identical** after this change (`wit_snapshot` passes untouched), and the SDK vendor drift test stays green. The contract did not need to grow; the host's *resolution* of an existing door did.

## Alternatives considered and rejected

* **Give the guest a network capability.** Destroys capability-by-absence — the whole reason a bad instrument cannot kill a set or exfiltrate from the audio thread. Rejected outright.
* **Pull the Phase-5 native data-port arbiter forward.** Bigger, and it blocks the instrument on the audio-path routing it does not need (v1 is display-only, zero ports). The schemas already ride the frozen `data-value` vocabulary precisely so the arbiter landing is a *routing* change, not a reshape.
* **A sidecar T3 process for the poll loop.** `process_spawn` weight (and a T3 door) for what is a background thread on the host's own control plane. v1 spawns nothing.
* **Write normalized feeds to disk, read them via `assets`.** Assets are hash-declared and immutable — a live door they are not.

## Consequences

* **New validation surface (all static, all sandbox-provable):** `stream`/`port` mutual exclusivity (`E-CROSS-FIELD`), registry-id resolution and `param:` target rules (`E-STREAM-UNKNOWN` + `E-CROSS-FIELD`), and `visible_if.param` existence. The decoder shape-checks; `sparq mod validate` stage 1 resolves (it owns the registry + params; the contract crate must not depend on the broker crate).
* **`visible_if` semantics are specified here** (the schema named the field but nothing defined it): v1 is **one equality predicate on one param value** — `{ param = "<id>", equals = <value> }` (an enum option index, a float within 1e-6, or a bool as 0.0/1.0). Anything richer (AND/OR, ranges) is a later minor. This is what lets sixteen per-cell STREAM pickers read as one dropdown that edits the selected cell.
* **A layout token joins the contract-as-data:** `layout.canvas.node_width_instrument_max = 2400` — the instrument card ceiling (the backbone's 480 is untouched), with the card-sizing rule (card = display `min_size` + chrome) recorded in `layout.toml` and enforced by the layout audit.
* **The broker crate is a new workspace member,** dependency-free by default; `serde_json` rides `streams`, `ureq` rides `streams-net`. The zero-dep default build is unchanged (`cargo tree` verified).

## Review trigger

When the **native data-port arbiter lands (Phase 5)**, re-express broker streams as `data`-port sources: the binding *syntax* survives (a `stream` becomes a `port` whose source is the broker), and the **guest code does not change** — it still calls `sources.snapshot(display, source)` and reads `window(list<data-record>)`. That is the point of routing everything through the frozen `data-record` vocabulary now: the sonification increment (deferred, WO-020 INC6) and the arbiter landing are routing changes, not reshapes.
