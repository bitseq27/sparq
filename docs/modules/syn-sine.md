# sparq/syn/sine — Sine

> **Generated** from `modules/syn/sine/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Exact-frequency sine oscillator*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | synth/oscillator/sine |
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
| 0 | `freq` | Frequency | float | Hz | 0 | 24000 | 440 |
| 1 | `amp` | Amplitude | float | ratio | 0 | 1 | 0.5 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/syn/sine/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
