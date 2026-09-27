# sparq/util/delay — Delay

> **Generated** from `modules/util/delay/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Tempo-syncable feedback delay with damping and dry/wet mix*

| | |
|---|---|
| Version | 0.2.0 |
| Host API | 1…1 |
| Category | utility/delay |
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
| 0 | `time` | Time | float | ms | 1 | 2000 | 250 |
| 1 | `feedback` | Feedback | float | ratio | 0 | 0.95 | 0.3 |
| 2 | `damp` | Damping | float | ratio | 0 | 1 | 0.5 |
| 3 | `mix` | Mix | float | ratio | 0 | 1 | 0.5 |
| 4 | `tempo-sync` | Tempo Sync (0 ms 1 beat) | int | x | 0 | 1 | 0 |
| 5 | `division` | Division (beats) | float | x | 0 | 16 | 0.5 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/util/delay/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: **parametric** — follows `time` (the executor cannot compensate it statically, and says so).
- CPU class: light
