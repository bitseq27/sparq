# sparq/ana/tap — Tap

> **Generated** from `modules/ana/tap/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Generic signal tap for displays - waveform at audio rate plus block peak and rms*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | analysis/tap/waveform |
| Kind / tier / stability | analysis · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Input | in | audio | channel set: stereo | no | — | 0 |
| `wave` | Wave | out | cv | rate: audio, range: bipolar | — | — | 0 |
| `peak` | Peak | out | cv | rate: block, range: unipolar | — | — | 0 |
| `rms` | RMS | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `gain` | Wave Gain | float | ratio | 0 | 4 | 1 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/ana/tap/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
