# sparq/env/ad — AD Envelope

> **Generated** from `modules/env/ad/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Attack/decay envelope - event-driven, sample-accurate, loopable*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | modulation/envelope/ad |
| Kind / tier / stability | source · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `trig` | Trigger | in | event | — | no | — | 0 |
| `out` | Envelope | out | cv | rate: audio, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `attack` | Attack | float | ms | 0.1 | 2000 | 2 |
| 1 | `decay` | Decay | float | ms | 1 | 4000 | 200 |
| 2 | `loop` | Loop | int | x | 0 | 1 | 0 |
| 3 | `curve` | Curve (0 exp 1 lin) | int | x | 0 | 1 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/env/ad/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
