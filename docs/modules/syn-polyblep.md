# sparq/syn/polyblep — Polyblep

> **Generated** from `modules/syn/polyblep/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Bandlimited saw/square/pulse (truncated additive - see defect #12)*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | synth/oscillator/polyblep |
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
| 0 | `freq` | Frequency | float | Hz | 0 | 24000 | 220 |
| 1 | `shape` | Shape (0 saw 1 square 2 pulse) | int | x | 0 | 2 | 0 |
| 2 | `pw` | Pulse width | float | ratio | 0.05 | 0.95 | 0.5 |
| 3 | `amp` | Amplitude | float | ratio | 0 | 1 | 0.5 |
| 4 | `partials` | Max partials (0 = Nyquist cap) | int | x | 0 | 128 | 48 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/syn/polyblep/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: medium
