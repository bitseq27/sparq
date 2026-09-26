# sparq/util/gain — Gain

> **Generated** from `modules/util/gain/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Gain and trim*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | utility/gain |
| Kind / tier / stability | processor · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Input | in | audio | channel set: stereo | — | — | 0 |
| `out` | Output | out | audio | channel set: stereo | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `gain` | Gain | float | ratio | 0 | 2 | 1 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/util/gain/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
