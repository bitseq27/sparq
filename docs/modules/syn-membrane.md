# sparq/syn/membrane — Membrane

> **Generated** from `modules/syn/membrane/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Drum-membrane voice - pitched sine drop plus noise burst, the techno kick*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | synth/drum/membrane |
| Kind / tier / stability | source · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `trig` | Trigger | in | event | — | no | — | 0 |
| `out` | Output | out | audio | channel set: mono | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `pitch` | Pitch | float | Hz | 20 | 200 | 50 |
| 1 | `punch` | Punch (transient pitch rise) | float | Hz | 0 | 400 | 130 |
| 2 | `decay` | Decay | float | ms | 20 | 1000 | 260 |
| 3 | `noise` | Noise Burst | float | ratio | 0 | 1 | 0.5 |
| 4 | `damp` | Damp (body lowpass) | float | Hz | 80 | 2000 | 320 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/syn/membrane/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: light
