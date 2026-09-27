# sparq/util/mixer — Mixer

> **Generated** from `modules/util/mixer/sparqmod.toml` by `tools/module_docs.py`. Do not edit by hand —
> regenerate with `python3 tools/module_docs.py`; CI fails on a stale doc (`--check`).


*4x4 stereo matrix + 4-to-1 cv merge, per-cell gain - the explicit merge, never implicit summing*

| | |
|---|---|
| Version | 0.2.0 |
| Host API | 1…1 |
| Category | utility/mixer |
| Kind / tier / stability | processor · t1 · stable |
| Authors | sparq |
| License | MIT |

## Ports

| ID | Name | Dir | Type | Shape | Required | Multiplicity | Latency |
|---|---|---|---|---|---|---|---|
| `in-0` | Input 1 | in | audio | channel set: stereo | no | — | 0 |
| `in-1` | Input 2 | in | audio | channel set: stereo | no | — | 0 |
| `in-2` | Input 3 | in | audio | channel set: stereo | no | — | 0 |
| `in-3` | Input 4 | in | audio | channel set: stereo | no | — | 0 |
| `out-0` | Output 1 | out | audio | channel set: stereo | — | — | 0 |
| `out-1` | Output 2 | out | audio | channel set: stereo | — | — | 0 |
| `out-2` | Output 3 | out | audio | channel set: stereo | — | — | 0 |
| `out-3` | Output 4 | out | audio | channel set: stereo | — | — | 0 |
| `cv-0` | Cv 1 | in | cv | rate: block, range: unipolar | no | — | 0 |
| `cv-1` | Cv 2 | in | cv | rate: block, range: unipolar | no | — | 0 |
| `cv-2` | Cv 3 | in | cv | rate: block, range: unipolar | no | — | 0 |
| `cv-3` | Cv 4 | in | cv | rate: block, range: unipolar | no | — | 0 |
| `cv-out` | Cv Merge | out | cv | rate: block, range: unipolar | — | — | 0 |

## Parameters

| # | ID | Name | Type | Unit | Min | Max | Default |
|---|---|---|---|---|---|---|---|
| 0 | `c00` | In 1 → Out 1 | float | ratio | 0 | 2 | 1 |
| 1 | `c01` | In 1 → Out 2 | float | ratio | 0 | 2 | 0 |
| 2 | `c02` | In 1 → Out 3 | float | ratio | 0 | 2 | 0 |
| 3 | `c03` | In 1 → Out 4 | float | ratio | 0 | 2 | 0 |
| 4 | `c10` | In 2 → Out 1 | float | ratio | 0 | 2 | 0 |
| 5 | `c11` | In 2 → Out 2 | float | ratio | 0 | 2 | 1 |
| 6 | `c12` | In 2 → Out 3 | float | ratio | 0 | 2 | 0 |
| 7 | `c13` | In 2 → Out 4 | float | ratio | 0 | 2 | 0 |
| 8 | `c20` | In 3 → Out 1 | float | ratio | 0 | 2 | 0 |
| 9 | `c21` | In 3 → Out 2 | float | ratio | 0 | 2 | 0 |
| 10 | `c22` | In 3 → Out 3 | float | ratio | 0 | 2 | 1 |
| 11 | `c23` | In 3 → Out 4 | float | ratio | 0 | 2 | 0 |
| 12 | `c30` | In 4 → Out 1 | float | ratio | 0 | 2 | 0 |
| 13 | `c31` | In 4 → Out 2 | float | ratio | 0 | 2 | 0 |
| 14 | `c32` | In 4 → Out 3 | float | ratio | 0 | 2 | 0 |
| 15 | `c33` | In 4 → Out 4 | float | ratio | 0 | 2 | 1 |
| 16 | `trim0` | Out 1 Trim | float | ratio | 0 | 2 | 1 |
| 17 | `trim1` | Out 2 Trim | float | ratio | 0 | 2 | 1 |
| 18 | `trim2` | Out 3 Trim | float | ratio | 0 | 2 | 1 |
| 19 | `trim3` | Out 4 Trim | float | ratio | 0 | 2 | 1 |
| 20 | `cvm0` | Cv 1 Gain | float | ratio | 0 | 2 | 1 |
| 21 | `cvm1` | Cv 2 Gain | float | ratio | 0 | 2 | 0 |
| 22 | `cvm2` | Cv 3 Gain | float | ratio | 0 | 2 | 0 |
| 23 | `cvm3` | Cv 4 Gain | float | ratio | 0 | 2 | 0 |

Parameter order is the snapshot order: `param(i)` in `process` reads row `i`.

## State

Schema `sparq/util/mixer/state` v1 — the blob `configure` accepts; a project save/restore round-trips through it.

## Resources

- Declared latency: 0 samples
- CPU class: trivial
