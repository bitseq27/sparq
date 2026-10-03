# sparq/mod/seq — Trig Seq

> **Generated** from `modules/mod/seq/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*3-16 step trigger sequencer: a clock walks the ring, the pattern mask fires steps*

| | |
|---|---|
| Version | 0.2.0 |
| Host API | 1…1 |
| Category | modulation/trigger-sequencer |
| Kind / tier / stability | processor · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `clk` | Clock In | in | event | — | no | — | 0 |
| `trig` | Trig Out | out | event | — | — | — | 0 |
| `step` | Step | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `steps` | Steps | int | x | 3 | 16 | 8 |
| 1 | `pattern` | Pattern Mask | int | x | 0 | 65535 | 170 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/mod/seq/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
