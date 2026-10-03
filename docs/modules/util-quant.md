# sparq/util/quant — Quantizer

> **Generated** from `modules/util/quant/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Pitch quantizer: snaps a 0..1 pitch cv to the Custom mask or the standard fifteen scales*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | utility/quantizer |
| Kind / tier / stability | processor · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `pitch-in` | Pitch In | in | cv | rate: audio, range: unipolar | — | — | 0 |
| `trig-in` | Trig In | in | event | — | no | — | 0 |
| `pitch-out` | Pitch Out | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `scale` | Scale | int | x | 0 | 15 | 0 |
| 1 | `custom-mask` | Custom Scale | int | x | 0 | 4095 | 4095 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/util/quant/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
