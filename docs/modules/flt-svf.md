# sparq/flt/svf — SVF

> **Generated** from `modules/flt/svf/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*State-variable filter, ZDF trapezoidal, five modes*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | filter/svf |
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
| 0 | `cutoff` | Cutoff | float | Hz | 10 | 20000 | 1000 |
| 1 | `resonance` | Resonance | float | ratio | 0 | 1 | 0.2 |
| 2 | `mode` | Mode (0 lp 1 hp 2 bp 3 notch 4 peak) | int | x | 0 | 4 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/flt/svf/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: light
