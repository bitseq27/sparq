# sparq/mod/clk — Clock

> **Generated** from `modules/mod/clk/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Free-running tempo clock: trigger outs at 4ths, 8ths, 16ths and 32nds, plus quarter phase*

| | |
|---|---|
| Version | 0.2.0 |
| Host API | 1…1 |
| Category | modulation/clock |
| Kind / tier / stability | source · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `4th` | Quarter | out | event | — | — | — | 0 |
| `8th` | Eighth | out | event | — | — | — | 0 |
| `16th` | Sixteenth | out | event | — | — | — | 0 |
| `32nd` | Thirty-second | out | event | — | — | — | 0 |
| `phase` | Phase | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `bpm` | Tempo | float | bpm | 20 | 300 | 120 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/mod/clk/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
