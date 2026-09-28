# sparq module manifest — schema reference (v0.1 draft)

Companion to `module-api-v1.md`. Format: **TOML**, file name `sparqmod.toml`, embedded in wasm components and in T3 process packages.
Rules: every field below is either required or has a documented default. Unknown keys are a **validation error** (typo protection), except inside tables marked `extensible`. Values marked *stable id* can never be renamed without an alias entry.

---

## 1. `identity`

| Key | Type | Req | Notes |
|---|---|---|---|
| `id` | string | yes | `namespace/category/name`, e.g. `sparq/syn/wavetable`. Stable id |
| `version` | semver | yes | module version |
| `host_api` | `{min, max}` | yes | sparq module-API version range |
| `display_name` | string | yes | shown in the browser |
| `summary` | string ≤ 120 ch | yes | one line, used in tooltips and search |
| `description` | string | no | long form, markdown |
| `authors` | [string] | yes | |
| `license` | SPDX | yes | |
| `repository`, `docs_url`, `issues_url` | url | no | |
| `tags` | [string] | no | free-text search facets |
| `signature` | object | conditional | required for Perform mode and registry publish (§7) |

## 2. `classification`

| Key | Type | Req | Values |
|---|---|---|---|
| `category` | string | yes | taxonomy path, e.g. `synth/oscillator/wavetable` |
| `top` | enum | yes | `syn smp flt fx dyn ana spa seq gen harm dat io dsp util ml out` |
| `kind` | enum | yes | `source processor utility analysis spatial display data generative io` |
| `tier` | enum | yes | `t1 t2 t3` |
| `stability` | enum | yes | `experimental stable deprecated` |

## 3. `ports[]`

| Key | Type | Req | Notes |
|---|---|---|---|
| `id` | string | yes | stable id |
| `name` | string | yes | |
| `direction` | enum | yes | `in out` |
| `type` | enum | yes | `audio cv event data gpu atom` (closed set, ADR-005) |
| `required` | bool | no (default true) | optional inputs get an explicit unconnected signal |
| `channel_set` | string \| [string] \| `"variable"` | if audio | `mono stereo quad 5.1 7.1.4 ambisonics:N objects:K raw:M` |
| `rate` | enum | if cv | `audio block` |
| `range` | enum | if cv | `bipolar unipolar` |
| `cv_reduce` | enum | no | `last first mean min max peak` (default `last`) — how an audio-rate `cv` source collapses to one value for this block-rate input. **Declared here, never chosen in code:** `first` and `last` produce different audio, so an undeclared choice would make a journal replay diverge from the original (ADR-007). Added by WO-007 decision G3, 2026-09-22 |
| `cv_interp` | enum | no | `hold linear spline` (default `hold`) — how the host expands a block-rate `cv` source for this audio-rate input. The host performs it; the module only declares which. Added by WO-007 decision G4/Q3, 2026-09-22. All three are performed since WO-008 increment 6: `spline` is the parabola through the last three block values, clamped to the declared range (the one compiled definition: `CvInterp` in `sparq-module-api/src/port.rs`) |
| `event_kinds` | [enum] | if event | `ump osc trigger gate note clock` |
| `data_schema` | `{id, version}` | if data | registered schema (§5) |
| `read_policy` | [enum] | if data in | `latest interpolated window accumulate on_change` |
| `stale_policy` | enum | if data in | `hold decay zero error` (+ `decay_ms`) |
| `multiplicity` | enum | no | `single multi_in multi_out` |
| `latency_contribution` | int | no | samples |
| `default_connected` | bool | no | UI hint |

## 4. `params[]`

| Key | Type | Req | Notes |
|---|---|---|---|
| `id`, `name` | string | yes | `id` stable |
| `type` | enum | yes | `float int bool enum text blob` |
| `unit` | enum | yes if numeric | closed vocabulary (module-api §5) |
| `min`, `max`, `default` | number | yes if numeric | default must be in range |
| `steps` | int | if int/enum | enum requires `options[]` of `{value, label}` |
| `curve` | string | no | `lin exp bipolar custom:<lut>` |
| `smoothing` | string | no | `none one_pole:<ms> lin:<ms> lag:<ms>` |
| `self_smoothed` | bool | no | module smooths internally |
| `mod_depth` | float | no | default when mapped |
| `automatable`, `randomisable`, `lockable`, `hidden` | bool | no | |
| `per_voice` | bool | no | requires `voices.policy != none` |
| `morph` | enum | no | `continuous discrete` (default continuous) |
| `group` | string | no | panel grouping |
| `aliases` | [string] | no | previous ids, for migration |

## 5. `data_schemas[]` (declared or referenced)

`{ id, version, rate_class, clock_domain, channels[] }` where each channel is `{ id, dtype, unit, range, interpolation }`.
Standard schemas are provided by the host (`sparq/imu9@1`, `sparq/optical-flow@1`, `sparq/loudness@1`, `sparq/pitch@1`, `sparq/onset@1`, `sparq/timeseries@1`, `sparq/pose2d@1`, `sparq/telemetry@1`); a module may declare a private schema with its own namespace.

## 6. `voices`

`policy (none|mono|poly:<n>|mpe|multitimbral:<n>) · per_voice_params[] · allocation · unison{count,detune_curve,spread} · note_events[]`

## 7. `state`

| Key | Notes |
|---|---|
| `schema_id`, `schema_version` | required; serialisation is mandatory for every module |
| `migrations[]` | ordered `from → to` chain |
| `size_class` | `tiny small medium large` (drives save/autosave strategy) |
| `assets[]` | `{role, hash, size, format, required}` — referenced **by hash only** |

## 8. `resources`

`latency (int | "param:<id>") · tail_samples · cpu_class (trivial|light|medium|heavy|very_heavy) · mem_class · gpu_class · oversampling (none|x2|x4|x8) · requires[] (fft, midi2, spatial, ml, gpu_compute, network)`

**There is no `internal_rate`.** Sub-block processing was considered and **rejected for v0** (WO-007 decision Q2, 2026-09-22): a module may not chop the host block into finer internal passes. This field existed in the v0.1 draft for a question `module-api-v1.md` §16 still listed as open — the schema had pre-committed to an undecided answer, and the answer turned out to be no. Grain and event *onsets* stay sample-accurate regardless, because §9 delivers the block's events pre-sorted by sample offset and the module renders into the block buffer. What v0 gives up is parameter *movement* inside a block (§9 makes the snapshot immutable for the block); the remedy for a module that needs finer resolution is a smaller **host** block, not a contract change — `BlockContext.frames` is a runtime field, and 100 modules at 16 frames costs ~0.52 % of a core against ~0.13 % at 64 frames (measured, `tools/dispatch-bench`).

## 9. `ui`

`panel{ layout[], widgets[] }` where a widget is `{kind, param|port|display, x, y, w, h, touch_class (S|M|L|XL), visible_if}`; `display_slots[]{source_port, display_module, colormap, mode}`; `custom_draw (t1 only, bool + entry symbol)`; `colour_class` (defaults from `classification.top`).

Widget kinds (closed vocabulary): `slider knob xy_pad matrix enum_select toggle numeric_entry label meter_attach display_slot button waveview grid_pad`.

## 10. `lifecycle`

`thread_affinity · init_cost (ms hint) · supports_hot_reload (bool) · reset_semantics (clears state? re-prepares?) · suspendable`

## 11. `capabilities` (enforced for T2/T3, advisory for T1)

`fs_read[] (asset scopes only) · fs_write[] · network (none|local|internet) · device[] (midi, camera, serial, hid, ble) · gpu (none|compute|draw) · process_spawn (T3 only) · max_fuel / max_memory_mb / max_cpu_ms_per_block`

## 12. `distribution`

`package_format (native|wasm|process) · entrypoint · platforms[] · arch[] · min_host_version · registry_namespace · channel (stable|beta)`

## 13. `signature`

`{key_id, algorithm, value, signed_at, attestation_url}` — required for registry publish and for Perform-mode loading (plan §15).

---

## Validation error catalogue (each must have a distinct, actionable message)

`E-ID-DUP` · `E-ID-UNSTABLE-RENAME` (id changed without alias) · `E-PORT-TYPE-UNKNOWN` · `E-UNIT-UNKNOWN` · `E-PARAM-DEFAULT-OUT-OF-RANGE` · `E-CHANNELSET-UNKNOWN` · `E-AMBI-ORDER-RANGE` · `E-SCHEMA-UNREGISTERED` · `E-ASSET-HASH-MISMATCH` · `E-REQUIRES-UNSUPPORTED` · `E-HOST-API-INCOMPATIBLE` · `E-STATE-SCHEMA-MISSING` · `E-MIGRATION-CHAIN-GAP` · `E-SIGNATURE-MISSING` (Perform mode) · `E-SIGNATURE-INVALID` · `E-TOUCH-CLASS-TOO-SMALL` · `E-WIDGET-KIND-UNKNOWN` · `E-LATENCY-UNDECLARED` (measured non-zero, declared zero) · `E-UNKNOWN-KEY`.

**Derived codes (added 2026-09-22, WO-007 task 1).** The 19 above were accumulated case-by-case as the design was written, which left two holes — and both are WO-007 acceptance-criteria blockers. Nothing rejected a **missing required field**: 20 rows are marked `Req: yes`, the only two `*-MISSING` codes are field-specific, and `E-UNKNOWN-KEY` catches extras but not omissions, so the criterion "rejects missing required ports with an actionable message" was unmeetable. And nothing rejected an **invalid value** for the ~20 closed vocabularies that have no specific code — `tier = "t9"` had no error to produce. Rather than keep adding codes by hand, they are **derived from the schema**, so a new field brings its own:

`E-KEY-MISSING:<path>` (required field absent) · `E-ENUM-UNKNOWN:<path>` (value outside a closed domain) · `E-CROSS-FIELD:<rule id>` · `E-VALUE-TOO-LONG:<path>` (e.g. `summary` over 120 chars) · `E-VALUE-MALFORMED:<path>` (bad semver, bad SPDX, bad url)

Cross-field rules needing ids, none of which had a code: `params[].per_voice` requires `voices.policy != none` · `ui.custom_draw` is T1-only · `capabilities.process_spawn` is T3-only · `stale_policy = decay` requires `decay_ms` · `params[].type = enum` requires `options[]`. The machine-readable form of every field, its domain and its codes is `docs/api/manifest-fields.toml` (82 rows, 22 required).

Validation runs at discovery, is cached by (id, version, file hash), and its output is shown verbatim in the module browser so an author sees exactly why their module didn't load.
