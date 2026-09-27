# sparq/mod/clk-div — Clock Divider

> **Generated** from `modules/mod/clk-div/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Event clock divider/multiplier with a seeded probability gate - triggers out*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | modulation/clock-divider |
| Kind / tier / stability | processor · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in` | Clock In | in | event | — | no | — | 0 |
| `out` | Trig Out | out | event | — | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `divide` | Divide | int | x | 1 | 16 | 1 |
| 1 | `multiply` | Multiply | int | x | 1 | 8 | 1 |
| 2 | `probability` | Probability | float | ratio | 0 | 1 | 1 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/mod/clk-div/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
