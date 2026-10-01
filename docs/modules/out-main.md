# sparq/out/main — Main Out

> **Generated** from `modules/out/main/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Master output with trim and the metering hook - the node the listener hears*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | io/output/main |
| Kind / tier / stability | io · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Main In | in | audio | channel set: stereo | no | — | 0 |
| `out` | Main Out | out | audio | channel set: stereo | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `trim` | Trim | float | ratio | 0 | 2 | 1 |
| 1 | `mute` | Mute | int | x | 0 | 1 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/out/main/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
