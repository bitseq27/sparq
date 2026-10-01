# sparq/util/mult — Mult

> **Generated** from `modules/util/mult/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Six-dot junction bus: each dot in or out, type set by its first connection*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | utility/multiple |
| Kind / tier / stability | utility · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `d1` | Dot 1 | in | audio | channel set: stereo | no | — | 0 |
| `d2` | Dot 2 | in | audio | channel set: stereo | no | — | 0 |
| `d3` | Dot 3 | in | audio | channel set: stereo | no | — | 0 |
| `d4` | Dot 4 | in | audio | channel set: stereo | no | — | 0 |
| `d5` | Dot 5 | in | audio | channel set: stereo | no | — | 0 |
| `d6` | Dot 6 | in | audio | channel set: stereo | no | — | 0 |

## Parameters

_None declared._

## State

Schema `sparq/util/mult/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
