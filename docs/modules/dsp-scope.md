# sparq/dsp/scope — Scope

> **Generated** from `modules/dsp/scope/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Waveform and X/Y display from tokens and colour maps - zero audio-thread cost*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | display/scope |
| Kind / tier / stability | display · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `x` | X / Wave | in | cv | rate: audio, range: bipolar | no | — | 0 |
| `y` | Y | in | cv | rate: audio, range: bipolar | no | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `timebase` | Timebase | float | ms | 1 | 500 | 20 |
| 1 | `mode` | Mode (0 wave 1 X/Y) | int | x | 0 | 1 | 0 |
| 2 | `trigger` | Trigger Level | float | ratio | -1 | 1 | 0 |
| 3 | `gain` | Vertical Gain | float | ratio | 0 | 8 | 1 |
| 4 | `colormap` | Colour Map | int | x | 0 | 7 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/dsp/scope/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
