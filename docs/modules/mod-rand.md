# sparq/mod/rand — Rand Step

> **Generated** from `modules/mod/rand/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*Seeded random-step ring: triggers walk it, value and cursor ride block-rate cv outs*

| | |
|---|---|
| Version | 0.1.0 |
| Host API | 1…1 |
| Category | modulation/random |
| Kind / tier / stability | source · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `trig-in` | Trig In | in | event | — | no | — | 0 |
| `value` | Value | out | cv | rate: block, range: unipolar | — | — | 0 |
| `trig-out` | Trig Out | out | event | — | — | — | 0 |
| `pos` | Position | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `steps` | Steps | int | x | 3 | 16 | 8 |
| 1 | `seed` | Seed | int | x | 0 | 65535 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/mod/rand/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
