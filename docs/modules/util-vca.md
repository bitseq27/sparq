# sparq/util/vca — VCA

> **Generated** from `modules/util/vca/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Voltage-controlled amplifier: audio gain = knob + bipolar control voltage*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | utility/vca |
| Kind / tier / stability | processor · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Audio In | in | audio | channel set: stereo | — | — | 0 |
| `cv` | Control In | in | cv | rate: audio, range: bipolar | no | — | 0 |
| `out` | Audio Out | out | audio | channel set: stereo | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `level` | Level | float | ratio | 0 | 2 | 1 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/util/vca/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
