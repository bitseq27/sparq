# sparq/fx/bitcrush — Bitcrush

> **Generated** from `modules/fx/bitcrush/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Depth and rate reduction with dither - aliasing as a feature*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | fx/distortion/bitcrush |
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
| 0 | `depth` | Depth (bits) | int | x | 1 | 16 | 8 |
| 1 | `rate_div` | Rate divider | int | x | 1 | 64 | 4 |
| 2 | `anti_alias` | Anti-alias | float | ratio | 0 | 1 | 0 |
| 3 | `dither` | Dither | bool | — | — | — | 1 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/fx/bitcrush/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
