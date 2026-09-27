# sparq/mod/lfo — LFO

> **Generated** from `modules/mod/lfo/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Low-frequency oscillator - four shapes, event phase-reset, unipolar audio-rate cv*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | modulation/lfo |
| Kind / tier / stability | source · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `sync` | Sync | in | event | — | no | — | 0 |
| `out` | Out | out | cv | rate: audio, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `rate` | Rate | float | Hz | 0.05 | 50 | 1 |
| 1 | `shape` | Shape (0 sine 1 tri 2 saw 3 sqr) | int | x | 0 | 3 | 0 |
| 2 | `depth` | Depth | float | ratio | 0 | 1 | 1 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/mod/lfo/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
