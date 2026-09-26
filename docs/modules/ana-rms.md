# sparq/ana/rms — RMS

> **Generated** from `modules/ana/rms/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*RMS and peak follower as a cv source*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | analysis/level/rms |
| Kind / tier / stability | analysis · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Input | in | audio | channel set: stereo | — | — | 0 |
| `level` | Level | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `floor` | Floor | float | ratio | 0 | 1 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/ana/rms/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
