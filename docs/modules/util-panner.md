# sparq/util/panner — Panner

> **Generated** from `modules/util/panner/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Mono-to-stereo panner with equal-power and linear laws*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | utility/pan |
| Kind / tier / stability | spatial · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Input | in | audio | channel set: mono | — | — | 0 |
| `out` | Output | out | audio | channel set: stereo | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `pan` | Pan | float | % | 0 | 100 | 50 |
| 1 | `law` | Law (0 equal-power 1 linear) | int | x | 0 | 1 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/util/panner/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
