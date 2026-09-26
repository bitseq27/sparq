# sparq/syn/noise — Noise

> **Generated** from `modules/syn/noise/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Seeded coloured noise: white, pink, brown, blue, violet*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | synth/noise |
| Kind / tier / stability | source · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `out` | Output | out | audio | channel set: mono | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `amp` | Amplitude | float | ratio | 0 | 1 | 0.25 |
| 1 | `colour` | Colour (0 white 1 pink 2 brown 3 blue 4 violet) | int | x | 0 | 4 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/syn/noise/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
