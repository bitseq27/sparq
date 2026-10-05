# WO-020 — `dat/observatory` · THE OBSERVATORY: the first instrument — a live Earth-&-space data wall (plan of record, 2026-10-06)

**What this file is.** The buildable plan for sparq's FIRST INSTRUMENT — the package that proves
the ADR-010 hand-off system in the flesh, and the biggest visual object the canvas has ever
held: a wall-class instrument that nearly fills the visible canvas at 100 % zoom, split into an
array of 4–16 live data displays, each with its own stream picked from a dropdown, a toolbar of
buttons and sliders, and raw-text tickers — fed by free live public data streams from space,
geological, weather and satellite sources, NASA first. Written this session (2026-10-06) at the
operator's instruction: **plan only, no code** — a new session builds from this file. Every
endpoint in §3 was **probed live from this sandbox on 2026-10-06**; the probe log is part of the
plan, because the DONKI lesson (§3.3) is that stream catalogues rot and a plan that cites
unprobed URLs is a promise, not a specification.

**Companions.** `docs/adr/010-instrument-layer.md` (the layer) · `docs/adr/011-stream-plane.md`
(NEW, drafted by INC2, summarized in §7) · `MODULE-BUILD-GUIDE.md` (the author path this
instrument walks) · `docs/api/instrument-wit/README.md` (the frozen contract face — **not
touched by this plan**, §7.4 proves it) · `docs/instrument-host.md` (the loader spec INC5 builds
to) · `design/mockups/display-sheet.svg` (the aesthetic exemplar, §5.6) · `WO018-STATE.md` +
`ROUND8-HANDOFF.md` §3 (the wasmtime memory ruling that shapes the sequencing).

---

## 1. Operator rulings this plan is built on (2026-10-06, via the question tool)

| # | Question | Ruling |
|---|---|---|
| R1 | Build path, given the T2 runtime loader cannot compile in the ~1 GB sandbox (ROUND8 §3) | **T2 canonical.** A true five-file wasm instrument per contract v1.1. The sandbox session builds everything wasmtime-free (broker, package, painters, headless goldens); live on-canvas running lands on the device via a run-sheet (§15.2). No T1 preview twin. |
| R2 | Stream policy — NASA alone cannot cover geology (no live seismic feed) and NASA DONKI is decommissioned (§3.3) | **NASA-first + free complements.** NASA EONET/NEO/EPIC/POWER (+FIRMS behind a free key), NOAA SWPC for live space weather, USGS for earthquakes, CelesTrak/wheretheiss.at for satellites, Open-Meteo/NOAA CO-OPS/GDACS/NWS for weather. ~23 streams, all probe-verified. |
| R3 | "Nearly fills the visible canvas at 100 % zoom" — sized for which screen? | **Wall class.** Display `min_size = [2176, 1120]`, card ≈ 2208 × 1288 world px — ≥ 97 % of the canvas-rect width at 2560 × 1600 (the house `--svg-out` reference) at zoom 1.0, FIT-to-canvas elsewhere, SOLO/FULL fills any screen. Numbers and the audit rule in §5.1. |
| R4 | Identity | **`dat/observatory` — "The Observatory".** Category `data/observatory`, top `dat`, kind `display`, layer `instrument`, tier `t2`. Accent = the existing `signal.data` class (green `#8BE36A` — the token, never the literal, in guest code). |

Standing house rules this plan inherits and does not restate: commit early and often with a
bundle per increment (ROUND8 §0's loss discipline); no number is written into a record until it
is measured; refusals are in words; the default build stays zero-dependency; goldens never move.

---

## 2. The one-page spec

**The Observatory** is one node in the graph — a `dat`-class instrument, the performance-and-
control layer's first citizen. It has **no audio ports in v1** (D5): it is a pure data display,
because the host does not route `data` ports yet and this plan refuses to ship half-working
sonification. It is the biggest object on the canvas: at 100 % zoom on a wall it *is* the canvas.

Its face is a **toolbar + a wall**:

* **Toolbar (panel widgets, host-drawn):** CELL picker · STREAM picker (per-cell, dropdown) ·
  LAYOUT (2×2 / 4×2 / 3×3 / 4×3 / 4×4 → 4…16 cells) · SOLO toggle (selected cell fills the
  wall) · FULL toggle (solo + minimal chrome — the data takes everything) · TICKER toggle ·
  GRID toggle · PAUSE toggle · sliders: HISTORY span, TICKER SPEED, INTENSITY.
* **The wall (one giant `display_list` display, guest-drawn, host-rendered):** an array of
  cells; each cell shows one live stream through one of seven renderers (§5.5): MAP (points or
  heat on an equirectangular coastline), TIMESERIES (phosphor traces with unit axes),
  TIMELINE-SCATTER (events against time), BARS, GAUGE, TEXT (raw feed, wrapped). Every cell
  carries the display-sheet anatomy: title + right-aligned description, axis labels **with
  units**, a status LED (LIVE / STALE / OFFLINE / KEY NEEDED — in words), the latest record's
  UTC stamp, and a one-line headline footer. A **global ticker band** along the bottom scrolls
  the raw text records (WWV alert text, quake places, EONET titles, flare classes) — the raw
  data stream, literally, as words.
* **The data:** a host-side **stream broker** (new crate `sparq-streams`, feature-gated) polls
  the ~23-endpoint registry (§3) on polite cadences, normalizes every feed into frozen-vocabulary
  `data-record`s, keeps rolling windows + a disk cache, and serves them to the instrument's
  display through the contract's own door — `sources.snapshot(display, source)` →
  `window(list<data-record>)`. The guest never touches the network (it cannot: the T2 world has
  no network door, and the validator already refuses any other declaration).

**Why this instrument first.** It falsifies the riskiest frozen surfaces while amendment is
cheap (ADR-010's own review trigger): the display-list vocabulary under real data volumes
(heat grids, coastlines, tickers), the token-bundle re-theme property, the five-file package
path, the validator chain, the panel widget vocabulary (including round 4's OWED dropdown
picker), and the sources binding — and it does it with **zero audio-path risk**, because v1 has
no audio path. It is also the honest first answer to "what is this software?": a scientific
instrument wall that happens to live inside a synthesizer.

---

## 3. Probe log — the stream registry as probed on 2026-10-06 (this sandbox, live)

Probe method: `python3` + `urllib` from the sandbox, descriptive UA, each endpoint hit once or
twice. Result codes below are real. The registry this table defines lands as checked-in data:
`crates/sparq-streams/streams.toml` (INC1) — the broker, the validator (binding checks, D4) and
the manifest generator all read it; it is the single source of truth for stream ids.

### 3.1 The registry (v1: 23 streams)

Domain codes: `SW` space weather · `SAT` satellites · `GEO` geological & Earth events · `WX`
weather & ocean · `SP` space & planetary. View codes → §5.5 renderers. Cadence = minimum poll
interval the broker enforces (§11). "Key" = environment variable; a stream whose key is unset is
not fetched and reports **KEY NEEDED** in words (D14) — never a silent hole.

| # | stream id | dom | name (label in dropdown) | endpoint (probed 2026-10-06) | cadence | view | key | probe |
|---|---|---|---|---|---|---|---|---|
| 1 | `swpc.aurora` | SW | Aurora forecast — OVATION grid | `https://services.swpc.noaa.gov/json/ovation_aurora_latest.json` | 300 s | map-heat | — | **200, 925 838 B** — `[lon, lat, intensity]` triples, ~28 k rows; broker downsamples 1°→2° (§6.4) |
| 2 | `swpc.kp` | SW | Planetary Kp index (3-day) | `https://services.swpc.noaa.gov/products/noaa-planetary-k-index.json` | 300 s | bars+gauge | — | **200** |
| 3 | `swpc.solar-wind` | SW | Solar wind speed | `https://services.swpc.noaa.gov/products/summary/solar-wind-speed.json` | 60 s | timeseries | — | **200** |
| 4 | `swpc.bz` | SW | Solar wind Bz (IMF) | `https://services.swpc.noaa.gov/products/summary/solar-wind-mag-field.json` | 60 s | timeseries (bipolar) | — | **200** |
| 5 | `swpc.xray-flares` | SW | GOES X-ray flares (7-day) | `https://services.swpc.noaa.gov/json/goes/primary/xray-flares-7-day.json` (+ `…/xray-flares-latest.json`) | 300 s | timeline-scatter + ticker | — | **200, 11 334 B / 449 B** — `begin_class` "B4.4" etc. |
| 6 | `swpc.flux10cm` | SW | 10.7 cm radio flux | `https://services.swpc.noaa.gov/products/summary/10cm-flux.json` | 900 s | timeseries | — | **200** |
| 7 | `swpc.wwv` | SW | WWV geophysical alert (raw text) | `https://services.swpc.noaa.gov/text/wwv.txt` | 600 s | text | — | **200, 527 B** — hourly, the ticker's flagship |
| 8 | `swpc.geomag-forecast` | SW | 3-day geomag forecast (raw text) | `https://services.swpc.noaa.gov/text/3-day-geomag-forecast.txt` | 1800 s | text | — | **200, 879 B** |
| 9 | `sat.iss` | SAT | ISS — live position | `https://api.wheretheiss.at/v1/satellites/25544` | 5 s | map marker + timeseries (alt/vel) | — | **200, 311 B** — lat/lon/alt/velocity/footprint/visibility; ToS ≤ 1 req/s (broker obeys, §11) |
| 10 | `sat.tle-stations` | SAT | Satellite TLEs — CelesTrak "stations" | `https://celestrak.org/NORAD/elements/gp.php?GROUP=stations&FORMAT=json` | 3600 s | text/table (SGP4 tracks = INC6) | — | **200** |
| 11 | `geo.quakes-hour` | GEO | Earthquakes — past hour (USGS) | `https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/all_hour.geojson` | 60 s | map-points + ticker | — | **200** |
| 12 | `geo.quakes-day` | GEO | Earthquakes — M2.5+ past day (USGS) | `https://earthquake.usgs.gov/earthquakes/feed/v1.0/summary/2.5_day.geojson` | 300 s | map-points + timeline | — | **200, 31 455 B** |
| 13 | `geo.gdacs` | GEO | GDACS — EQ/TC/FL events | `https://www.gdacs.org/gdacsapi/api/events/geteventlist/SEARCH?eventlist=EQ;TC;FL&alertlevel=Green;Orange;Red&fromDate={d-7}&toDate={d}` | 900 s | map-points (severity) + ticker | — | **200, 31 387 B** |
| 14 | `geo.eonet` | GEO | NASA EONET — all open events | `https://eonet.gsfc.nasa.gov/api/v3/events?status=open&limit=200` | 300 s | map-points + bars (category) + ticker | — | **200** — wildfires, volcanoes, storms, icebergs, dust |
| 15 | `geo.wildfires` | GEO | NASA EONET — wildfires | `https://eonet.gsfc.nasa.gov/api/v3/events?categories=wildfires&status=open` | 300 s | map-points + bars | — | **200** (same API, category filter) |
| 16 | `geo.firms` | GEO | NASA FIRMS — active fires 24 h | `https://firms.modaps.eosdis.nasa.gov/api/country/csv/{KEY}/viirs_snpp_nrt/{CC}/1` | 900 s | map-points + map-heat (density) | `SPARQ_FIRMS_KEY` (free) | **400 with a bad key, as documented** — needs a free MAP_KEY; hidden until set (D14) |
| 17 | `wx.openmeteo` | WX | Weather — selected location | `https://api.open-meteo.com/v1/forecast?latitude={lat}&longitude={lon}&current=temperature_2m,wind_speed_10m,cloud_cover,precipitation,pressure_msl&hourly=temperature_2m,precipitation_probability,wind_speed_10m&forecast_days=2` | 300 s | timeseries + gauge | — | **200** — lat/lon from broker config (env override, §6.2) |
| 18 | `wx.air` | WX | Air quality — PM2.5 / US AQI | `https://air-quality-api.open-meteo.com/v1/air-quality?latitude={lat}&longitude={lon}&current=pm2_5,us_aqi` | 900 s | gauge + timeseries | — | **200, 336 B** |
| 19 | `wx.tides` | WX | Water level — NOAA CO-OPS station | `https://api.tidesandcurrents.noaa.gov/api/prod/datagetter?product=water_level&application=sparq&begin_date={d-1}&end_date={d}&datum=MLLW&station={st}&time_zone=gmt&units=metric&interval=hilo&format=json` | 900 s | timeseries (obs vs prediction) | — | **200, 16 379 B** (station 9447130 Seattle probed; station configurable) |
| 20 | `wx.alerts` | WX | US weather alerts (NWS) | `https://api.weather.gov/alerts/active` | 300 s | map-points (severity) + ticker | — | **200** (requires a descriptive User-Agent — §11; note: `api.alerts.weather.gov` did NOT resolve from this sandbox, `api.weather.gov` did — registry carries the working host) |
| 21 | `space.neo` | SP | NASA NEO — close approaches | `https://api.nasa.gov/neo/rest/v1/feed?start_date={d}&end_date={d}&api_key={KEY}` | 1800 s | timeline (miss distance) + ticker (hazardous) | `SPARQ_NASA_API_KEY` (falls back to `DEMO_KEY`) | **200 with DEMO_KEY** |
| 22 | `space.epic` | SP | NASA EPIC — Earth imagery metadata | `https://api.nasa.gov/EPIC/api/natural?api_key={KEY}` | 3600 s | text ticker + map (sub-satellite coords) | as above | **200 with DEMO_KEY** — metadata only; the imagery itself is pixels → §13 |
| 23 | `space.power` | SP | NASA POWER — daily met/solar point | `https://power.larc.nasa.gov/api/temporal/daily/point?parameters=T2M,ALLSKY_SFC_SW_DWN&community=RE&longitude={lon}&latitude={lat}&start={d-30}&end={d-3}&format=JSON` | 3600 s | timeseries (daily) | as above | **200** — ~2–3 day publication lag; labelled NEAR-REALTIME in the cell footer, honestly |

### 3.2 Coverage of the operator's four domains

* **Space:** SWPC's full live set (aurora grid, Kp, solar wind, Bz, X-ray flares, 10.7 cm flux,
  raw WWV text) + NASA NEO/EPIC — 10 streams.
* **Geological:** USGS hour/day quake feeds, GDACS (EQ/TC/FL), NASA EONET (volcanoes ride
  EONET's categories), NASA FIRMS fires (key), NASA EONET wildfires — 6 streams.
* **Weather:** Open-Meteo forecast + air quality, NOAA CO-OPS water level, NWS alerts, EONET
  severe storms, SWPC space weather (the weather *of space*) — 5+ streams.
* **Satellite:** ISS live position, CelesTrak TLE catalogue (SGP4 ground tracks in INC6),
  EPIC/DSCOVR metadata, GIBS (deferred, §13) — 2 live + 2 gated.

### 3.3 Dead ends, probed and recorded (so the build session does not re-probe ghosts)

1. **NASA DONKI is decommissioned.** Every `api.nasa.gov/DONKI/*` call — with `apiKey=DEMO_KEY`,
   with `api_key=DEMO_KEY`, and with no key — returns either `API_KEY_MISSING` or a **60 444-byte
   CCMC "Announcements" HTML page**, never JSON (probed 2026-10-06, four endpoint variants).
   The registry therefore uses NOAA SWPC for live space weather; DONKI stays out until/unless
   NASA re-publishes it. This is exactly the drift the probe-first rule exists to catch.
2. **SWPC legacy paths are gone:** `json/xray-latest.json`, `products/summary/xray-flux.json`,
   `json/goes/primary/xray-flux-latest.json`, `…/integral-protons-latest.json`,
   `products/summary/relativistic-electron-flux.json`, `products/summary/aurora-30-minutes.json`
   — all 404. The working faces are `json/goes/primary/xray-flares-*.json`,
   `json/ovation_aurora_latest.json`, and the `products/summary/*` set listed in §3.1.
3. **Smithsonian GVP weekly-volcano RSS:** 403 (bot wall). Volcanoes are covered by EONET +
   GDACS instead.
4. **NASA GIBS WMTS:** GetCapabilities answers anonymously (**200**), so the door is open — but
   GIBS serves *image tiles*, and the display vocabulary has no image primitive by design
   (ADR-010 decision 3). Imagery → heat-cells downsampling is a broker-side transform, parked
   in §13/INC6, not v1.
5. **api.alerts.weather.gov** did not resolve from this sandbox (DNS quirk); `api.weather.gov`
   did. Registry carries the resolving host; the device run-sheet re-probes both.

---

## 4. The decisions

**D1 — Identity and home.** WO-020, the first instrument. Distributed package:
`instruments/observatory/` — the five files (`sparqmod.toml`, `observatory.wasm`,
`example.sparqpatch`, `preview.svg`, `README.md`). Source project: **`instruments-src/observatory/`**
(a new root directory — the five-file cap governs the *package*, not the source; the guide says
so explicitly). `identity.id = "dat/observatory"`, `display_name = "The Observatory"`,
`category = "data/observatory"`, `top = "dat"`, `kind = "display"`, `tier = "t2"`,
`layer = "instrument"`, `colour_class = "dat"` (the class stripe and accent resolve from
`color.signal.data` — the guest writes the token id, never the green).

**D2 — T2 canonical, sequenced against the memory ruling.** No T1 twin (R1). Everything the
~1 GB sandbox can build, it builds: broker (hermetic half), contract additions, guest source +
native harness, painters, shell work. Everything that needs wasmtime to *compile* (the runtime
loader half of WO-018 + the full validate chain) is a **device increment** with a run-sheet
(§15.2) — ROUND8 §3's ruling, applied, not re-litigated. The wasm component itself is built in
the sandbox if the toolchain fits (INC3 attempts it, ~30 MB std face + one small crate); if the
box refuses, the component build moves to the device and the sandbox seals the source + harness
proofs instead. Either way the package is complete and validated-by-stages before it ever meets
a live canvas.

**D3 — The guest never touches the network; the broker is host-side.** The validator already
honours only `network = "none"` for T2 (capability by absence — the world has no network door).
Live fetching lives in `sparq-streams`, an app-process background thread behind a feature flag —
the T3 *plane's* job (guide §6: "network fetching does not go inside the instrument") pulled
into the app as a thread, not a process: v1 spawns nothing (`process_spawn` stays T3-only).
The instrument consumes the broker's normalized records through the contract's existing door.

**D4 — Stream-source bindings: the one contract addition, additive, WIT untouched.** The
manifest's `ui.displays[].sources[]` today binds `{id, port}`. This plan adds one optional key:

```toml
[[ui.displays.sources]]
id = "cell-07"                    # what the guest passes to sources.snapshot()
stream = "param:cell_07"          # NEW: "<registry-id>"  |  "param:<param-id>"
```

`stream = "swpc.kp"` binds the cell to a fixed registry stream; `stream = "param:cell_07"`
binds it to **whatever stream the enum param `cell_07` currently selects** — the host resolves
the indirection at snapshot time (it owns both the ParamSet and the registry), which is what
makes per-cell runtime stream selection possible with a *static* manifest and *zero* WIT change:
`sources.snapshot(display-id, source-id)` already takes opaque strings, and
`stream-snapshot.window(list<data-record>)` already exists in the frozen contract. Validation:
`port` and `stream` are mutually exclusive; the registry id must exist in the checked-in
`streams.toml`; a `param:` target must exist, be `type = "enum"`, and every one of its
`options[].value` must be a registry id or the empty value `""` (= cell OFF). Failures are the
existing actionable style; a new code is pre-registered: **`E-STREAM-UNKNOWN`** (field, value
found, values allowed = the registry ids, the fix). `manifest-fields.toml` grows the rows; the
drift pins update in the same commit (the contract-as-data discipline).

**D5 — v1 is display-only: zero ports, no audio path, no journal surface.** The host does not
route `data` ports (Phase 5), and `process` cannot read `sources` (UI-thread import) — so an
honest v1 declares **no ports at all** and sonification waits (INC6, gated on the native
data-port arbiter; the plan does not pre-design it beyond noting that every stream normalizes to
the frozen `data-value` vocabulary *precisely so that* the sonification increment is a routing
change, not a reshape). Verification task, early in INC3: prove a port-less node passes graph
validation, the executor, the bridge, the browser and canvas layout (no hidden "≥ 1 port"
assumption was found by grep this session — `module.rs:346` builds an empty-ports context as a
*documented* case — but the proof is a test, not a grep). If anything refuses, the fix is host
tolerance + tests, in words, never a fake port to satisfy a hidden assumption.

**D6 — Wall-class sizing, as data.** New layout tokens (INC2): `node_width_instrument_max =
2400` (the instrument ceiling; the backbone's 480 ceiling is untouched) and the sizing rule
**card = display `min_size` + chrome** (gutter 16 each side; header 32; panel band = rows ×
44-touch-floor + gaps; no port band — zero ports). The Observatory declares `min_size = [2176,
1120]` → card ≈ **2208 × 1288** world px. Measured against the house reference canvas
(2560 × 1600 `--svg-out`, design chrome: rail 56 + library 240 + top 48 + toolbar 32 + dock
260): canvas rect ≈ 2264 × 1260 → the card covers **97.5 % of the width at zoom 1.0** and FITs
at ≈ 0.98 — "nearly fills the visible canvas at 100 % zoom", as ruled, as an audited number.
On the 1280 × 800 stage tablet it FITs to ≈ 0.42 (below `lod_1_below_zoom` — the host passes
`lod = reduced/minimal` in the frame-context and the guest degrades per §5.7; SOLO + the shell's
FOCUS action (§8.5) fill any screen). The layout audit grows a row: *the Observatory card at
2560 × 1600 covers ≥ 95 % of canvas-rect width at zoom 1.0.*

**D7 — The cell model: layout enum drives count; per-cell stream enums; SOLO/FULL are guest
params.** Params (26 of 32 cap): `cell_01…cell_16` (enum: the 23 registry ids + `""` OFF,
default = the four/five defaults of §5.4), `selected_cell` (enum 1…16), `layout` (enum: `2x2`,
`4x2`, `3x3`, `4x3`, `4x4` → 4/8/9/12/16 cells; unused `cell_NN` params are ignored, honestly),
`solo` (bool), `full` (bool), `ticker` (bool), `ticker_speed` (float 0.2…3.0 ×), `grid` (bool:
graticules/guides), `pause` (bool: freeze ticker scroll + trace advance), `history` (float,
log-mapped minutes: 10 min…7 d), `intensity` (float 0.5…2.0: colormap gain). The toolbar's
STREAM picker is **16 `enum_select` widgets bound to `cell_01…cell_16`, each with
`visible_if = { param = "selected_cell", equals = N }`** — one visible at a time, reading as a
single dropdown that edits the selected cell. This plan specifies `visible_if`'s semantics
(the schema names the field but nothing defines it yet): **v1 = one equality predicate on one
param value** (enum index or float equality within 1e-6; bool as 0.0/1.0). Anything richer is a
later minor. SOLO = the selected cell fills the wall (guest re-lays-out its own display — no
host involvement). FULL = solo + the guest drops cell headers/footers to hairline minimal (the
data takes the whole surface). The *shell-side* twin — camera FOCUS (fit-to-node) so FULL
actually fills the physical screen — is generic canvas chrome (§8.5), not instrument-coupled.

**D8 — Tickers, stamps and clocks: absolute UTC, never relative.** The guest has **no wall
clock** (contract decision: `t-wall-ns` removed from `time-info` at the v1.1 freeze) — so every
timestamp the wall shows is the **host-stamped `t_wall` of the record itself**, rendered as
absolute UTC with units (`2026-10-06 14:32:05 UTC`), and the cell footer's "age" readout is
computed *host-side* and delivered as a text channel in the record window (broker stamps
`age_s` at fetch — the host may read its own clock; the guest may not). Scroll animation rides
`frame-context.time-sec` (legal: displays are never replayed), PAUSE freezes it. The global
ticker band concatenates the newest `text`/headline records of all LIVE cells, newest first,
separated by the class dash — raw words from raw feeds (WWV, USGS places, EONET titles, GDACS
headlines, flare classes), which is the operator's "text tickers of the raw data streams"
literally.

**D9 — Aesthetics: the display-sheet is the exemplar; tokens are the law.** §5.6 restates the
display-sheet.svg anatomy as buildable rules. "More aesthetic than a regular module" is achieved
*within* the token system, never outside it: scale (the wall itself), phosphor traces for live
1-min feeds, colormaps as data encoding (table in §5.6), hairline graticules, generous
`space.*` padding inside cells, the `signal.data` class stripe, tabular numerals with units
everywhere, and the two-line display-sheet header/footer idiom per cell. A literal appearance
value anywhere in the package is `E-LITERAL-APPEARANCE` at the door — the beauty is a property
of the token system, which is the point: when the app re-themes, the Observatory re-themes with
it, zero intervention (WO-019's acceptance criterion, demonstrated on this instrument first).

**D10 — The broker: hermetic core, network half, registry as data.** `sparq-streams` splits the
house way (the `ui` / `ui-window` precedent): the crate core (registry, JSON→record normalizers,
rolling windows, disk cache, stale policy, **fixture replay**) needs only `serde_json` behind
feature `streams`; the live transport (`ureq`, TLS) sits behind feature `streams-net` — so the
sandbox compiles and tests everything except the socket, and the default build compiles neither
(zero-dep promise untouched; both features join the existing feature-gated-dep family: cpal,
wasmtime, egui/wgpu, windows). Scheduler: one background thread, per-stream cadence wheel,
ETag/If-Modified-Since where served, exponential backoff to 15 min on 429/5xx, descriptive UA on
every request (§11). Windows: per stream kind (timeseries 2048 records, events 256, grids 2,
text 64). Stale policy: **STALE** after 3 × cadence without a fresh record, **OFFLINE** after
10 × or on manual disable — both surfaced as record `data-flags` *and* as words in the cell LED.
CLI: `sparq streams list` (hermetic) · `probe [--live]` · `fetch <id> --out` · `tail <id>` ·
`record-fixtures --out DIR` · `cache path|clear` — every network verb **refuses in words**
without `streams-net`, the runtime-stages precedent.

**D11 — The determinism firewall.** Live data reaches only the UI thread's `sources` door; it
never reaches `process`, the audio path, the journal, or a golden. Golden renders run from
**recorded fixture windows** (`sparq streams record-fixtures` → JSON, checked in under
`reference/fixtures/observatory/`), with `time-sec` pinned by the harness → bit-exact, twice,
like every other golden. Fuel counts are recorded in the validate report and diffed by the
nightly matrix (instrument-host §3's rule). A network outage changes *what is displayed* (words,
LEDs, stale flags) and nothing else — the set keeps playing, the render stays deterministic.

**D12 — Assets: one embedded coastline, fixtures never in the package.** The MAP renderer needs
coastlines: Natural Earth 110m (public domain), simplified by a checked-in tool
(`tools/make_coastline.py`, INC3) to ≤ ~3 500 points / ≤ 160 KB, **embedded in the wasm data
section** (the guide's small-asset rule; no hash-declared asset store needed in v1). Fixtures
ride the *harness* (`--fixture` flag / `reference/fixtures/`), never the five-file package.

**D13 — Source split: pure core, thin glue, native harness (the sandbox-proof architecture).**
`instruments-src/observatory/` = three crates: **`observatory-core`** (pure Rust, no
wit-bindgen: cell layout, the seven renderers emitting a display-list IR *shaped exactly like
the WIT `display` records*, param/state structs, window→view transforms); **`observatory-wasm`**
(the SDK glue: `sparq_instrument!`, converts core IR ↔ contract types, ~300 lines);
**`observatory-harness`** (native binary: fixture JSON → core → IR → **display-list JSON
interchange** and/or SVG via the host painter path). The IR-to-JSON interchange is a harness
artefact, not a contract change — and it is what lets INC4's host painters (SVG + egui) render
*the instrument's real output* in the sandbox with no wasmtime anywhere, satisfying WO-019's
"identical through both painters from the same display list" acceptance early, and giving the
shell a legal **at-rest wall** (the "live where the rings carry it, at rest otherwise"
precedent): the canvas can show the last-rendered list while the loader is absent or the
instance is bypassed.

**D14 — Keys: keyless by default; free keys unlock, never required.** NASA endpoints run on
`DEMO_KEY` (30 req/h budget — the registry's 1800 s/3600 s cadences fit inside it with the disk
cache absorbing restarts); `SPARQ_NASA_API_KEY` raises the ceiling; `SPARQ_FIRMS_KEY` (free
registration) lights `geo.firms`. A stream whose key is unset is listed **KEY NEEDED** in the
dropdown's label suffix and in words in its cell — never fetched, never a silent hole. Env vars
only; no config-file surface in v1 (LATER).

---

## 5. The instrument face

### 5.1 The card, by the numbers (D6)

```
display min_size          2176 × 1120 world px      (manifest ui.displays[0])
card                      2208 × ~1288              (+16 gutter each side, 32 header,
                                                     ~104 panel band, zero port band)
canvas rect @2560×1600    ≈ 2264 × 1260 (design chrome)  → 97.5 % width at zoom 1.0
canvas rect @1280×800     ≈  984 ×  460 (library open)   → FIT ≈ 0.42, LOD-reduced
cell grid (gap = space.4 = 16 px, inner pad = space.3):
  2×2  → 4 cells  1080 × 552      4×2  → 8 cells  532 × 552
  3×3  → 9 cells   715 × 363      4×3  → 12 cells 532 × 363
  4×4  → 16 cells  532 × 270      SOLO → 1 cell  2176 × 1120
```

### 5.2 The toolbar (panel band, host-drawn widgets, 8 px grid, touch ≥ 44)

Row 1 (the toolbar proper): `CELL ▾` (enum_select → `selected_cell`) · `STREAM ▾` (the
visible_if sixteen, D7) · `LAYOUT ▾` · `[SOLO]` `[FULL]` `[TICKER]` `[GRID]` `[PAUSE]`
(toggles) · status label (host-side: broker state — `LIVE 21/23 · 2 OFFLINE` — the shell's own
words, not guest-drawn). Row 2 (sliders): `HISTORY` (log) · `TICKER SPEED` · `INTENSITY`. All
class M touch targets; the dropdown picker is round 4's OWED item 11, built here (§8.4) because
this instrument cannot ship without it.

### 5.3 Cell anatomy (the display-sheet idiom, per cell)

```
┌──────────────────────────────────────────────────────────────┐
│ ▌EARTHQUAKES — PAST HOUR              USGS · map · 60 s   ◉  │  title (type.scale.m, text.primary)
│                                                              │  right-aligned description (scale.s,
│            (renderer: map / trace / bars / gauge / text)     │  text.secondary) + status LED (arc)
│                                                              │
│ 180°        90°W         0°         90°E        180°         │  axis labels, scale.s, text.tertiary,
│ ───────────────────────────────────────────────  M 2.5…7.1  │  ALWAYS with units (token rule 9)
│ 14:32:05 UTC · 47 s ago · "M 4.3 — 12 km S of Idria, CA"    │  footer: stamp · age · latest headline
└──────────────────────────────────────────────────────────────┘
```

Selection halo: `stroke.emphasis` w2 around the selected cell. LIVE = `signal.data` LED;
STALE = warn token; OFFLINE / KEY NEEDED = the words, drawn as glyph-run in the body centre (the
shell's refuse-in-words rule, guest-side). Class stripe: 2 px `signal.data` along the card top
(existing card chrome vocabulary).

### 5.4 Default state (ships in `example.sparqpatch` + state defaults)

`layout = 4x4` (16 cells — the full wall, the operator's headline case); defaults fill the
domains: `swpc.aurora` (map-heat), `geo.quakes-hour` (map-points), `swpc.bz` (timeseries),
`swpc.wwv` (text), `sat.iss` (map), `geo.eonet` (map-points), `wx.openmeteo` (timeseries),
`swpc.kp` (bars), `swpc.xray-flares` (timeline), `space.neo` (timeline), `wx.tides`
(timeseries), `wx.alerts` (map-points), `swpc.solar-wind` (timeseries), `geo.gdacs`
(map-points), `wx.air` (gauge), `space.power` (timeseries) — cells 17+ n/a at 4×4; `""` (OFF)
for any cell beyond the layout's count. The example patch: the Observatory alone beside a
minimal backbone ambient chain (`mod/clk → mod/seq → syn/sine → util/vca → out/main`) — proving
canvas coexistence, FIT behaviour with mixed card sizes, and that the wall is silent-but-live
while the set plays.

### 5.5 The seven renderers (display-list vocabulary → data kinds)

| view | primitives used | streams | encoding rules |
|---|---|---|---|
| **timeseries** | `trace` (phosphor = live ≤ 60 s feeds) or `polyline`, hairline `rect` grid, `glyph-run` axes | solar wind, Bz, Kp history, 10.7 cm, weather hourly, tides (obs solid / prediction `dash = dashed`), POWER daily, ISS alt/vel | x = history param window, y = auto-range with units; signed data (Bz) crosses zero at cell mid, `colormap.bipolar` for the trace value channel where the stream is signed |
| **map-points** | coastline `path` (embedded asset), graticule hairlines every 30°, `points` (size class by magnitude, `colormap.inferno-class` by depth / severity) + event glyph markers | quakes, EONET, GDACS, alerts, NEO ground track n/a, ISS marker (`arc` + crosshair `path` + `glyph-run` label) | equirectangular; newest events brightest (values channel = age); legend line = magnitude scale with units |
| **map-heat** | coastline + `heat_cells` | OVATION aurora (2° grid, `colormap.thermal`), FIRMS density (0.25° bins, `colormap.thermal`) | NaN = no cell (contract); intensity gain = INTENSITY param; colourbar strip with units |
| **timeline-scatter** | axes + `points` by (t, class) | X-ray flares (class → y: B/C/M/X), quake magnitudes vs time | class letters as `glyph-run` annotations (non-colour encoding, token rule 7) |
| **bars** | `rect` per bin + `glyph-run` values | EONET categories, Kp 3-day (bar per 3 h, `colormap.bipolar` by Kp level) | bars from declared colormap only where the value is data; never decorative |
| **gauge** | `arc` sweep + `glyph-run` value-with-units + threshold ticks | Kp now, AQI now, temperature now | thresholds = registry metadata (e.g. Kp ≥ 5 storm line, AQI bands) — data, not taste |
| **text** | wrapped `glyph-run` lines, scrolling by `time-sec × ticker_speed` | WWV raw, 3-day forecast raw, TLE table, EPIC metadata lines | the RAW payload, verbatim, monospace-tabular; PAUSE freezes; overflow scrolls, oldest falls off |

The **global ticker band** (bottom 40 px of the display, toggle): one scrolling `glyph-run`
marquee merging every LIVE cell's newest headline/text record, each prefixed by its cell's
stream label — `◂ USGS·HOUR  M 4.3 12 km S of Idria, CA 14:32:05Z · WWV  Solar-terrestrial
indices for 06 Oct… · EONET  Prescribed Fire D3 Bear RX, Greenlee AZ ◂ …`.

### 5.6 The aesthetic rules (display-sheet.svg as exemplar — buildable, not vibes)

1. **Anatomy per cell = anatomy per mockup card:** title left / description right on one line;
   axis captions with units at the bottom corners; a footnote line under the axes. (Compare
   `dsp/scope`'s card in display-sheet.svg: "waveform + persistence · audio class", "0 ms …
   21.3 ms", the footnote "persistence half-life 300 ms · glow ∝ amplitude^1.5".)
2. **Ground and depth:** cell bodies sit on `color.ground.inset`; the card on the canvas
   ground; separation by ground tint + hairline opacity tiers only (elevation token: no
   shadows, ever).
3. **Colormaps are data encoding, never style** (token rule 11): `thermal` for aurora/fire
   heat, `bipolar` for signed (Bz, Kp-vs-baseline, anomalies), `inferno-class` for depth/
   severity, `phosphor` for live scope-like traces, `greyscale` for age/secondary channels,
   `categorical-6` for EONET/GDACS category dots. The LUTs ride the token bundle
   (`token-bundle.json` payload verified this session to carry `colormaps` + `tokens`).
4. **Motion is the host's:** `trace.phosphor = true` and the host's motion tokens do decay/glow;
   the guest never simulates persistence (display.wit's own rule) — except the ticker scroll,
   which is content position, not decoration.
5. **Stroke discipline:** hairline (w1) grids/graticules/axes; w2 data traces; w3 the selected
   cell's emphasis and nothing else; corners c2 cells / c4 the card; dashes encode
   prediction-vs-observed and non-colour channels (rule 7).
6. **Numbers always with units** (rule 9): `282 km/s`, `−4.1 nT`, `Kp 5−`, `M 4.3`,
   `38 µg/m³`, `2.71 m MLLW`. Tabular numerals are the host type engine's job.
7. **The wall reads as one object:** uniform cell chrome, one type scale (s/m only inside
   cells), the `signal.data` green appearing ONLY as class identity (stripe, LEDs, selection
   accents) — never as a data colour, which is what colormaps are for.

### 5.7 LOD degradation (host passes `lod` in frame-context)

| lod | cell content |
|---|---|
| `full` | everything above |
| `reduced` | drop graticules, footnotes, tick labels; keep renderer + title + LED + stamp |
| `minimal` | title + LED + one glyph-run value/headline; renderer drawn only if its primitive count < 500, else WORDS ("16 cells — zoom in for data") |

The global ticker survives all levels (it is the cheapest, most legible element); PAUSE state is
always visible (a paused ticker shows `❚❚` at the band's left — host glyph vocabulary).

### 5.8 Budgets (declared, then measured — the harness reports, the validator enforces)

`gpu_class = "medium"` (262 144 vertices / 16 384 instances / **65 536 heat cells** — the
ceilings table this session read in `sparq-host-wasm/ceilings.rs`): worst case ≈ 4 map cells ×
3.5 k coastline points + one 180 × 90 aurora grid (16 200 cells) + ≤ 2 k points + ≤ 300 glyph
runs — inside medium with ~4× headroom on cells; `heavy` is NOT declared (a wall that fits
medium must stay medium — the ceilings file's own movement rule). `max_fuel = 2 000 000`
per block (`process` is a no-op returning `Ok`; fuel is essentially all `draw`, metered by the
UI-thread epoch deadline), `max_memory_mb = 64` (16 windows + coastline + layout ≈ 8 MB
estimated; the aurora grid's 925 KB payload is downsampled broker-side, D10/§6.4, so the guest
never holds it), `cpu_class = "light"`, `latency = 0`.

---

## 6. The broker (`crates/sparq-streams`)

### 6.1 Shape

```
crates/sparq-streams/
├─ Cargo.toml            # optional deps: serde_json (feature "streams"), ureq (feature "streams-net")
├─ streams.toml          # THE REGISTRY (§3.1 as data: id, domain, label, endpoint template,
│                        #   cadence_s, view, schema, key_env, units, attribution, config knobs)
├─ src/registry.rs       # load/query the registry; the validator reads this too (D4)
├─ src/fetch.rs          # streams-net half: ureq transport, ETag, backoff, UA, timeouts
├─ src/normalize.rs      # per-stream JSON/CSV/GeoJSON/text → data-record (frozen vocabulary)
├─ src/window.rs         # rolling windows per stream kind + stale/offline flags
├─ src/cache.rs          # disk cache (last-good payloads + fetch metadata), user data dir
├─ src/replay.rs         # fixture windows from recorded JSON (the hermetic door: goldens, tests)
└─ src/lib.rs
```

Records carry the frozen `data-value` carriers only: `double`/`single` (measurements), `int32`
(counts), `text` (headlines, raw feeds), `enumerated` (severity, flare class), `vec` (grid rows,
f64 carrier — the ratified judgement call), `blob` (TLE blocks). Per-stream channel schemas are
declared private-namespace (§4 of the schema doc): `observatory/timeseries@1` `{t_utc: i64,
value: f64, age_s: f64}`, `observatory/events@1` `{t_utc, lat, lon, magnitude, depth_km,
severity: enum, headline: text, age_s}`, `observatory/grid@1` `{cols: i32, rows: i32, lat0,
lon0, step_deg, values: vec, t_utc, age_s}`, `observatory/position@1` `{lat, lon, alt_km,
vel_kms, footprint_km, visibility: enum, t_utc, age_s}`, `observatory/text@1` `{text, issued_utc,
age_s}`. `age_s` is the D8 host-side stamp. The host re-stamps `t_wall_ns`/`seq` when serving
windows (the WIT stamping rule) — the broker's own stamps are advisory, exactly as the contract
says.

### 6.2 Configuration (v1 = env + registry defaults, no config UI)

`SPARQ_NASA_API_KEY`, `SPARQ_FIRMS_KEY`, `SPARQ_OBSERVATORY_LOC` (`"lat,lon"` — default
51.5,−0.1 London; used by `wx.*` and `space.power`), `SPARQ_OBSERVATORY_TIDE_STATION`
(default `9447130`), `SPARQ_STREAMS_CACHE` (default: OS user-data dir + `/sparq/streams-cache`).
All documented in the package README + WINDOWS.md.

### 6.3 Threading and lifetime

One `std::thread` (named `sparq-streams`), started lazily on first instrument instantiation
(feature on), joined at shutdown. UI thread reads windows through a `std::sync::RwLock` (UI
thread, not audio thread — legal); the audio thread never touches the crate (there is no audio
path, D5). Broker failure of any kind degrades to stale windows + flags — the app never dies
because a feed died.

### 6.4 The one broker-side transform: aurora downsampling

`ovation_aurora_latest.json` arrives ~925 KB / ~28 k `[lon, lat, intensity]` triples at 1°
resolution; the broker bins to a 2° grid (180 × 90 = 16 200 cells, ≤ 130 KB in-window) before
the guest ever sees it — keeps the wasm memory honest and the heat-cells budget at ~25 % of
medium. The transform is declared in the registry row (`transform = "regrid:2deg"`) — data, not
code-path folklore.

---

## 7. Contract additions (INC2) — and the proof nothing frozen moves

### 7.1 ADR-011 (new, one page): "The stream plane — host-side broker and stream-source bindings"

Decision: (a) live external data is a **host service**, never a guest capability; (b) it reaches
instruments through the existing `sources` import via a new manifest binding key `stream`
(registry id or `param:` indirection); (c) the registry is checked-in data shared by broker,
validator and manifest generator; (d) keys are env-scoped and optional; (e) the determinism
firewall (D11) is a property: stream data has no path to audio/journal in v1. Alternatives
considered and rejected: guest network capability (destroys capability-by-absence); pulling the
Phase-5 data-port arbiter forward (bigger, blocks the instrument on the audio path it doesn't
need); a sidecar T3 process (process_spawn weight for a poll loop); writing the normalized feeds
to disk and reading them via `assets` (assets are hash-declared, not a live door). **Review
trigger:** when the native data-port arbiter lands (Phase 5), re-express broker streams as
data-port sources — the binding syntax survives; the guest code does not change.

### 7.2 Manifest schema deltas (all additive)

1. `ui.displays[].sources[]`: `port` and new `stream` (mutually exclusive; exactly one).
2. `ui.panel.widgets[].visible_if`: semantics specified (D7) — `{ param = "<id>", equals = <value> }`.
3. `docs/api/manifest-fields.toml`: the new rows (+ the field-table drift test moves in the same
   commit — the contract-as-data discipline).
4. Validation codes: **`E-STREAM-UNKNOWN`** pre-registered (the five-code precedent); reuse
   `E-CROSS-FIELD` for port/stream exclusivity and `param:` target rules.
5. Layout tokens: `node_width_instrument_max = 2400` + the card-sizing rule (D6) + audit row.
6. `MODULE-BUILD-GUIDE.md` §5: one paragraph — stream bindings, with the registry's location.
   (The guide amends itself when an instrument falsifies it — this is the guide's own rule, and
   the Observatory is the falsifier-in-residence.)

### 7.3 What the validator learns (INC2, zero-dep half)

`sparq mod validate` stage 1–2 gain: stream-binding resolution against `streams.toml`;
`param:` target checks; `visible_if` param existence; port-less node legality (D5); the
instrument-card sizing rule in the visual-conformance stage spec (stage 6 runs where painters
exist). All static — the sandbox runs them.

### 7.4 The frozen surface does not move — the proof obligation

`docs/api/instrument-wit/wit/*` bytes are hash-pinned (`wit_snapshot.rs`) and **this plan
changes none of them**: `stream` is a *manifest* key resolved host-side; `sources.snapshot`
keeps its opaque `(display-id, source-id)` signature; `window(list<data-record>)` already
exists; the display-list vocabulary already covers every renderer in §5.5. INC2's acceptance
includes: `wit_snapshot` hashes UNCHANGED, the SDK vendor drift test green, and a sentence in
the ADR recording why no WIT amendment was needed (so the next instrument doesn't re-litigate
it).

---

## 8. Host/shell increments (the sandbox-buildable majority)

### 8.1 The display-list painter, twice (the WO-019 harness slice, pulled into this WO)

`sparq-ui/src/displaylist.rs` (pure, zero-dep): the frozen vocabulary → resolved geometry+style
(token ids → values via the generated token bundle JSON; colormap LUTs via
`design/tokens/generated/colormaps.json`), LOD degradation hooks, budget counting against the
ceilings table. Two back ends: **SVG** (headless — extends the `--svg-out` family:
`sparq instrument render <dir> --fixture <json> --list <json> --svg-out <svg> [--width --height]`,
rendering either a package dir or a raw display-list JSON interchange file per D13) and
**egui** (feature `ui`, the on-canvas painter for instrument display surfaces). Unknown token
ids skip the primitive with a diagnostic (contract decision G), hard-fail at validate.

### 8.2 The instrument card in the canvas

`layout.rs`: display-`min_size`-driven card sizing (D6) + `node_width_instrument_max`; the well
band becomes a **display band** for instrument-layer nodes; the at-rest surface shows the last
display-list JSON (D13) or WORDS if none exists yet. `inset.rs`/`canvas_ui.rs`: the display
band paints via §8.1's egui back end. Browser/dock: the instrument layer groups under its own
header (WO-018's noted "browser visual grouping"), library card shows `preview.svg`.

### 8.3 Sources door, hermetic provider

The binding-resolution logic of D4 lands as a host-side provider trait with **two
implementations**: `ReplayProvider` (fixture windows — sandbox, tests, at-rest renders, goldens)
and `BrokerProvider` (`streams-net` — device). The wasmtime `sources` import (INC5) is a thin
door over the same trait — which is why the sandbox can prove every binding behaviour without a
runtime.

### 8.4 The dropdown picker (round 4's OWED item 11, discharged here)

`enum_select` for inspector + panel widgets: 44 px rows, options list from the manifest, the
selected row in the class accent, scroll for long lists (24 options), keyboard-free (touch
first), refuse-to-render below touch floor in words. This was owed; the Observatory cannot ship
without it; it lands as its own tested increment inside INC4.

### 8.5 Camera FOCUS (generic canvas chrome)

Right-click menu + toolbar + header double-tap on any node: **FOCUS** = camera fit-to-node
(the existing `zoom_to_fit` machinery, bounds = one node). For the Observatory this is the
physical-screen twin of the guest's FULL param (D7): FULL maximizes data inside the card, FOCUS
maximizes the card inside the screen. Generic, no instrument coupling, one audit row.

---

## 9. The increments

> Sequencing rule: each increment ends green (its own tests + the standing gates), commits, and
> bundles — the ROUND8 loss discipline. INC1–INC4 are sandbox-buildable; INC5 is the device
> run-sheet; INC6 is deferred/gated.

### INC1 — the broker hermetic half + the registry as data

* **Scope:** `crates/sparq-streams` (D10 shape) minus `fetch.rs`; `streams.toml` (all 23 rows +
  the §3.3 dead ends as comments); normalizers for every stream **from recorded payloads**;
  windows/stale/cache/replay; `tools/streams_record.py` (urllib-based recorder — the sandbox has
  python and network; it writes `reference/fixtures/observatory/<stream-id>.json` + a
  `PROBE-LOG-2026-10-06.md` with statuses, sizes, timestamps — the §3 table's machine sibling);
  app CLI `sparq streams list|cache` + the network verbs refusing in words without `streams-net`;
  workspace wiring (feature `streams`, dep `serde_json` behind it; default build untouched).
* **Tests:** normalizer unit tests per stream against checked-in fixtures (23 minimum); window
  eviction/stale transitions; cache round-trip; registry parse + unknown-id words; CLI refusal
  words; the default-feature build stays zero-dep (the existing CI cells prove it).
* **Acceptance:** `cargo test --workspace` green with +N tests recorded; `sparq streams list`
  prints the 23-row registry with domains and cadences; every fixture ≤ its recorded size × 1.2;
  `streams-net` absence refuses fetch/probe/tail/record in words, exit non-zero.
* **Artefacts:** crate, registry, fixtures, probe log, CLI.

### INC2 — the contract additions (ADR-011, bindings, tokens, validator, guide)

* **Scope:** §7 in full: ADR-011; schema doc + `manifest-fields.toml` rows; `decode.rs`
  (`stream` key, `visible_if` shape); `validate.rs` (stream resolution vs registry, `param:`
  rules, `E-STREAM-UNKNOWN`, port-less legality); layout tokens (`node_width_instrument_max`,
  sizing rule) + `token_gen`/audit regeneration; MODULE-BUILD-GUIDE §5 paragraph; drift gates.
* **Tests:** decode/validate cases per rule (legal binding, unknown stream, param: wrong type,
  port+stream both, options ⊄ registry); token regeneration determinism; **wit_snapshot hashes
  UNCHANGED** (§7.4); api_snapshot/manifest drift pins updated in-commit.
* **Acceptance:** the Observatory manifest draft (§10) passes validator stage 1–2 **static**
  with its real bindings; a hostile manifest fails each new rule in words; guide + schema + fields
  + ADR all land in one reviewable commit family.

### INC3 — the guest: source project, package, harness, wasm attempt

* **Scope:** `instruments-src/observatory/` (D13 three crates); `tools/make_coastline.py` +
  the embedded simplified coastline (D12); the seven renderers; cell layout + solo/full + LOD;
  the manifest **generator** (`gen_manifest.py` — the 16 × 24-option enum walls are generated
  from `streams.toml`, never hand-typed; drift-gated against the registry);
  `example.sparqpatch` (§5.4); package README (keys, attribution §11.3, licence);
  `tools/observatory_package.py` (assemble the five-file dir → `instruments/observatory/`);
  harness: fixture → core → **display-list JSON interchange** + SVG at the three breakpoints
  (via INC4's painter if landed, else the harness's own minimal SVG writer — same IR);
  **wasm component build attempt** (`rustup target add wasm32-wasip1` + `cargo build` +
  `wasm-tools component new` — or `cargo component` if it installs cleanly; risk §12.2: if the
  box refuses, the recipe + source seal and the component builds on the device).
* **Tests (native, hermetic):** renderer golden-IR tests per view (fixture in → IR JSON out,
  hashed); layout math (the §5.1 numbers as tests); param cap arithmetic (26 ≤ 32); state
  save/restore determinism; the port-less-node proof (D5) against kernel/executor/bridge as
  integration tests in the workspace; fuel-free draw bounds (primitive counts vs the medium
  ceilings table).
* **Acceptance:** `sparq mod validate instruments/observatory/` passes static stages (schema,
  package ≤ 5 files, capability honour: `network = "none"` ✓) and REFUSES runtime stages in
  words (sandbox honesty); harness SVGs of the 16-cell wall at 2560 × 1600 + the two smaller
  breakpoints reviewed against display-sheet.svg; component builds OR the device fallback is
  recorded with the exact recipe.

### INC4 — the host painters + shell (the sandbox's visible half)

* **Scope:** §8 in full: `displaylist.rs` + SVG/egui back ends; `sparq instrument render` CLI;
  canvas card sizing + display band + at-rest list rendering; the enum_select dropdown picker
  (§8.4); camera FOCUS (§8.5); browser/dock instrument grouping; `ReplayProvider` wired so the
  live egui shell shows the **fixture-fed wall** (at-rest lists refreshed per frame from replay
  windows — the shell demo without wasmtime); audit rows (card coverage ≥ 95 % width at
  2560 × 1600; touch targets; LOD transitions).
* **Tests:** painter primitive-coverage tests (every variant of the frozen `item` enum renders
  to both back ends); token resolution (unknown id → skip + diagnostic); colormap LUT sampling;
  dropdown picker geometry/behaviour tests; FOCUS camera tests; `ui --audit` grows its lines and
  stays PASS; convergence-sheet regeneration showing the Observatory in the shell.
* **Acceptance:** `sparq ui --svg-out` renders a patch containing the Observatory node with its
  wall painted from the replay provider; the dropdown picker discharges round 4's OWED item
  (recorded in CHECKLIST); the audit's coverage row measures ≥ 95 %; existing 925 + INC1–3 tests
  all green; `--features ui` and `ui-window` clippy cells clean.

### INC5 — the device increment: runtime loader + live acceptance (run-sheet §15.2)

* **Scope:** the WO-018 runtime half per `docs/instrument-host.md` §2–§4 (wasmtime engine,
  compile+cache, two-instance ordering, fuel/epoch ↔ watchdog, memory limiter), discovery loading
  `instruments/` at launch, the `sources`/`tokens`/`host`/`assets` doors, `BrokerProvider`
  behind `streams-net`, validate's runtime stages (smoke, golden render from fixtures ×2
  bit-exact, budgets measured, visual conformance via §8.1, `preview.svg` regeneration), and the
  live wall: real streams, real dropdowns, real SOLO/FULL/FOCUS, offline-in-words.
* **Acceptance (device-measured, recorded not invented):** the full validate chain PASS on the
  sealed package; goldens bit-exact twice; fuel/memory reports inside declared classes; ≥ 60 fps
  frame-time histogram with the wall + 50 backbone nodes live; the token re-theme demonstration
  (change `color.signal.audio`→ the wall's phosphor traces re-theme with **zero package bytes
  changed** — WO-019's criterion, proven on instrument #1); every §3.1 endpoint re-probed from
  the device with statuses recorded (the sandbox-DNS asterisks of §3.3 cleared); the four
  operator-facing behaviours filmed/screenshotted for the checklist (wall at 100 %, dropdown
  swap live, solo/full, network-pull → STALE → OFFLINE in words).
* **Prerequisite:** host ≥ 2 GB (ROUND8 §3); CI ubuntu-latest runs the same proofs headless.

### INC6 — deferred, gated (not built by this plan)

Sonification (event/cv outs driven by streams — **gated on the Phase-5 native data-port
arbiter**; the schemas already ride the frozen vocabulary so the increment is routing, not
reshape) · SGP4 ground tracks from `sat.tle-stations` (a small deterministic propagator inside
the guest — ordinary wasm compute, fuel-budgeted) · GIBS imagery → broker-side heat-cells
downsampling (§3.3.4) · FIRMS global layer with key · per-user stream config UI · Perform-mode
full-bleed wall.

---

## 10. The manifest (draft of record — the generator emits exactly this shape)

```toml
# instruments/observatory/sparqmod.toml  (file 1 of 5 — generated by instruments-src/observatory/gen_manifest.py)
[identity]
id = "dat/observatory"
version = "0.1.0"
host_api = { min = 1, max = 1 }
display_name = "The Observatory"
summary = "A wall of live Earth-and-space data: 4-16 streams, maps, charts, heat grids, raw-text tickers"
authors = ["sparq first-party (WO-020)"]
license = "MIT"
tags = ["instrument", "data", "live", "nasa", "usgs", "noaa", "wall"]

[classification]
category = "data/observatory"
top = "dat"
kind = "display"
tier = "t2"
layer = "instrument"
stability = "experimental"

# ports: NONE in v1 (D5) — display-only until the data-port arbiter lands.

[[params]]  # ×16, generated: cell_01 … cell_16
id = "cell_01"
name = "Cell 01 stream"
type = "enum"
default = 0                       # option index; options generated from streams.toml (+ "" OFF)
options = [ { value = "", label = "— OFF —" }, { value = "swpc.aurora", label = "Aurora — OVATION" }, … ]

[[params]]
id = "selected_cell"
name = "Selected cell"
type = "enum"
default = 0
options = [ { value = "1", label = "Cell 01" }, … 16 ]

[[params]]
id = "layout"
name = "Layout"
type = "enum"
default = 4                       # 4x4 — the full wall ships as the default face
options = [ { value = "2x2", label = "2 × 2 (4)" }, { value = "4x2", label = "4 × 2 (8)" },
            { value = "3x3", label = "3 × 3 (9)" }, { value = "4x3", label = "4 × 3 (12)" },
            { value = "4x4", label = "4 × 4 (16)" } ]

[[params]]
id = "solo"
name = "Solo selected cell"
type = "bool"
default = false

[[params]]
id = "full"
name = "Full (solo + minimal chrome)"
type = "bool"
default = false

[[params]]
id = "ticker"
name = "Global ticker"
type = "bool"
default = true

[[params]]
id = "grid"
name = "Graticules & guides"
type = "bool"
default = true

[[params]]
id = "pause"
name = "Pause motion"
type = "bool"
default = false

[[params]]
id = "ticker_speed"
name = "Ticker speed"
type = "float"
unit = "ratio"
min = 0.2
max = 3.0
default = 1.0

[[params]]
id = "history"
name = "History span"
type = "float"
unit = "min"
min = 10.0
max = 10080.0
default = 180.0
curve = "exp"

[[params]]
id = "intensity"
name = "Colormap intensity"
type = "float"
unit = "ratio"
min = 0.5
max = 2.0
default = 1.0

[state]
schema_id = "dat/observatory/state"
schema_version = 1

[ui]
colour_class = "dat"
# panel.widgets: toolbar row (CELL/STREAM×16-with-visible_if/LAYOUT enums, SOLO/FULL/TICKER/GRID/PAUSE
# buttons, status label) + slider row (history, ticker_speed, intensity) — full generated table, 8 px
# grid, touch_class M everywhere, per §5.2.

[[ui.displays]]
id = "wall"
kind = "display_list"
min_size = [2176, 1120]
lod = "auto"
colormap = "colormap.thermal"      # the display's declared default map (per-cell maps ride items)

[[ui.displays.sources]]            # ×16, generated
id = "cell-01"
stream = "param:cell_01"           # D4 — host resolves the enum to a registry id at snapshot time

[resources]
latency = 0
cpu_class = "light"
gpu_class = "medium"               # §5.8 — measured by the harness; movement needs an operator ruling

[capabilities]
fs_read = []
network = "none"                   # the broker is host-side (D3); the validator enforces exactly this
max_fuel = 2_000_000
max_memory_mb = 64

[distribution]
package_format = "wasm"
entrypoint = "observatory.wasm"
platforms = ["any"]
arch = ["wasm32"]
min_host_version = "0.0.0-phase0"
channel = "beta"
```

---

## 11. Etiquette, keys, attribution (the broker's manners are a feature)

### 11.1 Polling discipline
Cadences of §3.1 are **floors**, not targets: the scheduler never polls faster; ETag /
If-Modified-Since where served; exponential backoff (×2 to 15 min) on 429/5xx/timeouts; a
global concurrency of 2 in-flight fetches; all fetches jitter-start (no thundering herd at
launch). DEMO_KEY budget arithmetic: NEO (1800 s) + EPIC (3600 s) + POWER (3600 s) ≈ 7 calls/h
≪ 30/h — restart storms are absorbed by the disk cache (cache-first boot: show last-good with
STALE flags while the first live round runs).

### 11.2 Identity
Every request carries `User-Agent: sparq-observatory/<version> (+https://github.com/bitseq27/sparq; WO-020)`.
NWS **requires** a UA (its 400 on missing/odd params was probed); wheretheiss.at's free tier is
≤ 1 req/s (registry cadence 5 s honours it); USGS/SWPC/NASA ask only for attribution.

### 11.3 Attribution & licences (carried in the package README, file 5)
USGS, NOAA/SWPC, NASA (EONET/NEO/EPIC/POWER/FIRMS), GDACS (UN/JRC), CelesTrak, wheretheiss.at:
free public data — attributed per source terms. **Open-Meteo: free for non-commercial use with
attribution — if sparq ever becomes commercial, a paid licence or replacement is required
(flagged in LATER.md).** Natural Earth coastline: public domain. The package itself: MIT, like
the repo.

### 11.4 Keys (D14)
`SPARQ_NASA_API_KEY` (free, api.nasa.gov — lifts DEMO_KEY limits) · `SPARQ_FIRMS_KEY` (free,
NASA FIRMS registration — lights `geo.firms`). Missing key = KEY NEEDED in words; never a
silent hole, never a crash.

---

## 12. Risks (honest, with mitigations)

1. **wasmtime compile memory** — settled by ROUND8 §3: device/CI only; INC5 is a device
   increment by construction. The plan's sandbox half never needs it (D13/§8.3 exist precisely
   for this).
2. **The wasm component build in the sandbox** — `wasm32-wasip1` std face (~30 MB) + one small
   crate + wit-bindgen codegen: *probably* fits in ~1 GB, not guaranteed. Mitigation: INC3
   attempts it early; on OOM the recipe + source seal and the component builds on the device
   (the harness proves the logic meanwhile). `cargo component` is optional sugar —
   `cargo build --target wasm32-wasip1` + `wasm-tools component new` is the documented fallback
   (the SDK's own README says exactly this).
3. **wit-bindgen native compile for the harness** — avoided by construction: the harness links
   `observatory-core`, never the SDK glue (D13).
4. **`ureq`/rustls(ring) compile memory in the sandbox** — mitigated by the `streams` /
   `streams-net` split (D10): the sandbox compiles the hermetic half (serde_json only); the TLS
   transport is device/CI. If even serde_json is unwanted, the normalizers can move to the
   hand-rolled decoder precedent (`decode.rs`) — declared fallback, not the plan.
5. **Port-less node tolerance** — unverified assumption; verification is an INC3 test *before*
   renderers are built on it; host-tolerance fixes are additive.
6. **API drift between plan and build** — the DONKI class. Mitigation: `streams.toml` is data;
   `tools/streams_record.py` re-probes and re-records in one command; fixtures make every
   downstream test drift-immune; dead feeds degrade to OFFLINE words, never crashes.
7. **Sandbox network variance** — fixtures are recorded in INC1 while the network is proven;
   all later increments are hermetic.
8. **Rate-limit pressure on shared sandbox IPs** — DEMO_KEY 30/h + cache-first + backoff; live
   tests are `#[ignore]`-gated and run deliberately, never in CI loops.
9. **The 925 KB aurora payload** — broker-side regrid (D10/§6.4) before any window or guest.
10. **16 live cells at 60 fps** — primitive budgets measured by the harness against the medium
    ceilings; LOD degradation (§5.7); decimation rules per renderer (≤ 1 point/px column, the
    scope precedent); the frame-time histogram is an INC5 acceptance line, not a hope.
11. **Manifest bloat** (16 × 24 enum options) — generated, drift-gated; never hand-edited.
12. **Aesthetic drift from the mockups** — the harness SVGs are reviewed against
    display-sheet.svg at three breakpoints *in the sandbox* (INC3/4), before device; the
    look-board audit rows land in INC4.

---

## 13. Out of scope (declared) → LATER.md entries

Satellite **imagery as pixels** (GIBS/GOES photography — no image primitive by design; the
broker-downsample-to-heat-cells experiment is INC6-gated, and a badged pixel escape hatch stays
parked where ADR-010 parked it) · **sonification** (INC6, Phase-5-gated) · **3D globe scene**
(scene-descriptor v1 could carry it; a later instrument or an INC7) · per-user **stream config
UI** / adding streams without a registry edit · the **cloud stream registry** · multi-instrument
wall sync / Perform full-bleed mode · **signing** the package (unsigned badged is legal and is
how it ships) · Windows service mode for the broker · proxy/TLS-corp-network handling beyond
ureq's defaults.

---

## 14. File map (what the build session creates/touches)

```
NEW  WO020-OBSERVATORY-PLAN.md                       ← this file (already landed)
NEW  docs/adr/011-stream-plane.md                    ← INC2
NEW  crates/sparq-streams/**                          ← INC1 (registry, normalizers, windows,
                                                        cache, replay; fetch.rs = streams-net)
NEW  reference/fixtures/observatory/*.json + PROBE-LOG-2026-10-06.md   ← INC1 recorder output
NEW  tools/streams_record.py                          ← INC1
NEW  instruments-src/observatory/**                   ← INC3 (core / wasm / harness crates,
                                                        gen_manifest.py, make_coastline.py,
                                                        coastline asset, example.sparqpatch src)
NEW  instruments/observatory/  (the five files)       ← INC3 (assembled by tools/observatory_package.py)
NEW  tools/observatory_package.py                     ← INC3
NEW  crates/sparq-ui/src/displaylist.rs + painters    ← INC4
MOD  crates/sparq-module-api/src/{manifest,decode}.rs ← INC2 (stream bindings, visible_if, E-code)
MOD  crates/sparq-host-wasm/src/validate.rs           ← INC2 (static stream rules)
MOD  docs/api/manifest-schema.md + manifest-fields.toml ← INC2 (+ drift tests in-commit)
MOD  design/tokens/layout.toml + generated/**         ← INC2 (instrument sizing tokens)
MOD  crates/sparq-ui/src/canvas/{layout,inset,levels}.rs ← INC4 (card sizing, display band, LOD)
MOD  crates/sparq-app/src/{ui/canvas_ui,instruments,cli,main}.rs ← INC4 (picker, FOCUS, render CLI, grouping)
MOD  MODULE-BUILD-GUIDE.md §5                         ← INC2 (one paragraph)
MOD  PHASE0-WORKORDERS.md §3b                         ← the WO-020 ticket (§16 text)
MOD  CHECKLIST.md · SYNC.md · README.md status line   ← per house discipline at close
MOD  LATER.md                                         ← §13 entries
MOD  scripts/test007.bat (NEW) + gates.bat            ← INC5 device run-sheet (§15.2)
MUST NOT MOVE: docs/api/instrument-wit/wit/* (hash-pinned) · existing goldens · the 24 module
manifests · the default zero-dep feature set · SYNC-STAMP.txt (device re-stamps).
```

---

## 15. Run recipes

### 15.1 Sandbox session (INC1 → INC4)

1. Environment: ROUND5-HANDOFF §7 + ROUND6 §7 (+ ROUND8's MSVC std face if crosscheck cells run).
   `export CARGO_HOME=… PATH=… CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=zigcc-direct`.
2. **Baseline before any edit:** `cargo test --workspace` = 925/0/1-ignored (the round-8 seal).
   Record it; a moved baseline is a stop-the-world.
3. INC1 → gates → commit → bundle. INC2 → gates (+ wit_snapshot unchanged proof) → commit →
   bundle. INC3 → harness SVG review → commit → bundle. INC4 → `ui --audit` + svg-out
   convergence sheet → commit → bundle. (`cargo clippy --workspace --all-targets -- -D warnings`
   + `cargo fmt --check` + the python gates at every step — `just gates` where it runs.)
4. Live recording (INC1 only, while the network is proven): `python3 tools/streams_record.py
   --all --out reference/fixtures/observatory/`.
5. Seal: overlay zip + sha256s (SYNC.md's exclusion list: `.git/ target/ Cargo.lock logs/ *.wav
   SYNC-STAMP.txt`), handoff paragraph in the round's handoff file, CHECKLIST entry.

### 15.2 Device run-sheet — `scripts/test007.bat` skeleton (INC5)

A. `scripts\build.bat` (features incl. `instrument-host`, `streams`, `streams-net`) — the stamp
mismatch words are EXPECTED until re-stamp. B. `sparq streams probe --live` — all 23 statuses
recorded (DONKI stays dead; §3.3 asterisks cleared). C. Build/fetch `observatory.wasm` if the
sandbox sealed source-only (§12.2 recipe). D. `sparq mod validate instruments\observatory\` —
FULL chain: smoke, golden ×2 bit-exact, budgets, visual conformance, `preview.svg` regenerated.
E. Launch UI: the wall loads from `instruments\`, at-rest → LIVE as windows fill. F. Operator
eyes: dropdown swaps a cell's stream live · LAYOUT 4↔16 · SOLO/FULL + canvas FOCUS · tickers
scroll, PAUSE freezes · stamps are UTC-with-units. G. Network pull (airplane mode): STALE at
3× cadence → OFFLINE at 10×, in words; audio unaffected; restore → LIVE. H. Frame-time
histogram: wall + 50 backbone nodes ≥ 60 fps. I. Token re-theme demonstration (WO-019's
criterion). J. Fuel/memory report vs declared classes. K. `scripts\gates.bat` full. L. Re-stamp:
`python tools\sync_check.py --write --sync sparq-wo020-observatory-<date>`. Send back
`test007-digest.log` + `logs\ui-digest.log` + screenshots of F/G/I.

---

## 16. Ticket text for `PHASE0-WORKORDERS.md` §3b (paste at build time)

> ### WO-020 — The Observatory: the first instrument, and the stream plane
> **Objective.** Ship instrument #1 (`dat/observatory`) through the public five-file path, and
> the host machinery it falsifies: the stream broker (ADR-011), stream-source bindings, the
> display-list painters, the dropdown picker, instrument-card sizing. **Depends on.** WO-018
> (static half — shipped; runtime half — this WO's INC5 discharges it on a ≥ 2 GB host),
> WO-017's frozen contract (untouched — §7.4 of the plan proves it). **In scope.** Plan of
> record: `WO020-OBSERVATORY-PLAN.md` (INC1–INC5; INC6 deferred/gated). **Acceptance
> criteria.** The plan's §9 acceptance lines, device-measured where marked; the four operator
> rulings R1–R4 honoured; zero WIT bytes moved; default build zero-dep; goldens untouched.
> **Artefacts.** The package, the broker crate, ADR-011, painters, fixtures + probe log,
> test007 run-sheet + digest. **Risks.** The plan's §12, all mitigated there.

---

## 17. Sync note for THIS plan file

This plan landed as overlay pack **`sparq-update-2026-10-06.zip`** (one file:
`WO020-OBSERVATORY-PLAN.md`, workspace-relative — extract at the repo root; exclusions per
SYNC.md's standing list; the stamp does NOT ride and does NOT move — the build session
re-stamps on the owning machine per §15.1/§15.2). The zip's sha256 was recorded in the delivery
message beside this file. No other file in the tree changed this session: the plan is the whole
delivery, by operator instruction ("just write the plan to .md").

*End of WO-020 plan of record. Decisions before code — the `WO008-INC7-PLAN.md` discipline.*

---

## 18. Addendum — the four plan-review mockups (landed 2026-10-06, same session)

Operator instruction after §17: *"make 4 mockup images of the new module."* Landed as
generated SVG (the house mockup format — `design-mode.svg`/`perform-mode.svg`/
`display-sheet.svg` are generated SVG too) + PNG review siblings:

| file | view |
|---|---|
| `design/mockups/observatory-wall.svg/.png` | **1 of 4 — THE WALL**: 4 × 4, 16 live cells, full design-mode shell at the 2560 × 1600 reference, FIT 97 % — the §5.1 sizing claim, drawn: header + class stripe, toolbar (CELL/STREAM/LAYOUT enums, SOLO/FULL/TICKER/GRID/PAUSE, three sliders, broker status), the seven renderers, per-cell anatomy (§5.3), the global raw-text ticker, 14 LIVE · 1 STALE · 1 OFFLINE in words |
| `design/mockups/observatory-full.svg/.png` | **2 of 4 — SOLO + FULL**: `swpc.aurora` takes the whole wall — the OVATION 2° heat grid on the Natural Earth 110m coastlines, `colormap.thermal`, graticule, geomagnetic-pole + sub-solar annotations, minimal chrome (D7) |
| `design/mockups/observatory-picker.svg/.png` | **3 of 4 — the toolbar + STREAM picker**: 2 × 2 layout, cell 02 selected (white corner ticks), the dropdown open with domain group headers, the current row checked, a hover row, and `geo.firms` listing **KEY NEEDED** (D14) — round 4's OWED item 11, drawn as specified |
| `design/mockups/observatory-canvas.svg/.png` | **4 of 4 — the wall in a patch**: zoom 45 %, the example patch (§5.4) beside it — backbone chain with living wells (clock rings, seq walking light, scope trace, master meters), event cables solid / control wire dashed, cable node, the Observatory selected and patched to nothing (D5) |

**Generator:** `tools/make_observatory_mockups.py` — the `make_display_sheet.py` discipline:
every colour, stroke, spacing and chrome metric is READ from `design/tokens/*.toml` + the baked
`colormaps.json` LUTs at generation time (this script holds no appearance value of its own);
coastlines come from Natural Earth 110m (public domain) cached in `tools/data/` (fetch recipe
in the script header); stream data is illustrative and SEEDED (deterministic output, like every
house generator). PNGs render with `rsvg-convert` when present and are review artefacts only —
the SVGs are the source. Regenerate after any token change.

**What the mockups settle visually** (operator-review list): the wall's scale against the shell
and against the backbone (mockups 1 & 4); the cell anatomy and the ticker idiom inherited from
`display-sheet.svg`; the toolbar's widget grammar and the dropdown's group/current/hover/
KEY-NEEDED states (mockup 3); FULL's minimal-chrome face (mockup 2); the honest states
(STALE / OFFLINE in words) living inside an otherwise beautiful wall. Nothing in the mockups
invents an appearance token — every hue in them resolves from the token files, which is the
point: when the app re-themes, these four images are regenerable, not stale.
