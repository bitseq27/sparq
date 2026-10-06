//! The guest-side mirror of the stream registry's guest-visible metadata (WO-020 INC3).
//!
//! GENERATED from `crates/sparq-streams/streams.toml` by
//! `instruments-src/observatory/gen_manifest.py` — DO NOT EDIT. Regenerate:
//!   python3 instruments-src/observatory/gen_manifest.py
//!
//! The guest has no filesystem and no network (T2 capability-by-absence), so the registry
//! reaches it as compiled-in data — embedded in the wasm data section like the coastline
//! asset (D12). Drift gate: the harness test `streams_table_matches_the_registry` compares
//! this table against `sparq_streams::Registry` field by field, so a registry edit without a
//! regeneration fails a test rather than silently mislabelling a cell.

/// One registry row's guest-visible metadata (cell chrome + renderer dispatch read this).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamMeta {
    /// The stable stream id — what a manifest `stream` binding names.
    pub id: &'static str,
    /// Domain code: SW / SAT / GEO / WX / SP.
    pub domain: &'static str,
    /// The dropdown/cell-title label.
    pub label: &'static str,
    /// The renderer hint (§5.5): map-heat · map-points · map · timeseries · bars ·
    /// bars+gauge · gauge · timeline-scatter · timeline · text.
    pub view: &'static str,
    /// The unit string the cell's axes/footer must carry (token rule 9). May be empty.
    pub units: &'static str,
    /// The broker's minimum poll interval — the cell description shows it.
    pub cadence_s: u32,
    /// The frozen record schema this stream's windows carry (`observatory/<kind>@1`).
    pub schema: &'static str,
    /// The env var holding an API key, when the stream needs one (D14's KEY NEEDED words).
    pub key_env: Option<&'static str>,
}

/// The registry's stream count (v1: 23).
pub const STREAM_COUNT: usize = 23;

/// The OFF sentinel's option index: every cell enum's option 0 is `""` = cell OFF.
pub const OFF_OPTION: usize = 0;

/// The OFF option's label (mirrors gen_manifest.py's `— OFF —`).
pub const OFF_LABEL: &str = "— OFF —";

/// The registry, in file order — option index = 1 + position here.
pub const STREAMS: [StreamMeta; STREAM_COUNT] = [
    StreamMeta {
        id: "swpc.aurora",
        domain: "SW",
        label: "Aurora forecast — OVATION grid",
        view: "map-heat",
        units: "intensity %",
        cadence_s: 300,
        schema: "observatory/grid@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.kp",
        domain: "SW",
        label: "Planetary Kp index (3-day)",
        view: "bars+gauge",
        units: "Kp",
        cadence_s: 300,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.solar-wind",
        domain: "SW",
        label: "Solar wind speed",
        view: "timeseries",
        units: "km/s",
        cadence_s: 60,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.bz",
        domain: "SW",
        label: "Solar wind Bz (IMF)",
        view: "timeseries",
        units: "nT",
        cadence_s: 60,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.xray-flares",
        domain: "SW",
        label: "GOES X-ray flares (7-day)",
        view: "timeline-scatter",
        units: "GOES class",
        cadence_s: 300,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.flux10cm",
        domain: "SW",
        label: "10.7 cm radio flux",
        view: "timeseries",
        units: "sfu",
        cadence_s: 900,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.wwv",
        domain: "SW",
        label: "WWV geophysical alert (raw text)",
        view: "text",
        units: "",
        cadence_s: 600,
        schema: "observatory/text@1",
        key_env: None,
    },
    StreamMeta {
        id: "swpc.geomag-forecast",
        domain: "SW",
        label: "3-day geomag forecast (raw text)",
        view: "text",
        units: "",
        cadence_s: 1800,
        schema: "observatory/text@1",
        key_env: None,
    },
    StreamMeta {
        id: "sat.iss",
        domain: "SAT",
        label: "ISS — live position",
        view: "map",
        units: "km, km/s",
        cadence_s: 5,
        schema: "observatory/position@1",
        key_env: None,
    },
    StreamMeta {
        id: "sat.tle-stations",
        domain: "SAT",
        label: "Satellite TLEs — CelesTrak stations",
        view: "text",
        units: "",
        cadence_s: 3600,
        schema: "observatory/text@1",
        key_env: None,
    },
    StreamMeta {
        id: "geo.quakes-hour",
        domain: "GEO",
        label: "Earthquakes — past hour (USGS)",
        view: "map-points",
        units: "magnitude",
        cadence_s: 60,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "geo.quakes-day",
        domain: "GEO",
        label: "Earthquakes — M2.5+ past day (USGS)",
        view: "map-points",
        units: "magnitude",
        cadence_s: 300,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "geo.gdacs",
        domain: "GEO",
        label: "GDACS — EQ/TC/FL events",
        view: "map-points",
        units: "severity",
        cadence_s: 900,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "geo.eonet",
        domain: "GEO",
        label: "NASA EONET — all open events",
        view: "map-points",
        units: "category",
        cadence_s: 300,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "geo.wildfires",
        domain: "GEO",
        label: "NASA EONET — wildfires",
        view: "map-points",
        units: "category",
        cadence_s: 300,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "geo.firms",
        domain: "GEO",
        label: "NASA FIRMS — active fires 24 h",
        view: "map-points",
        units: "brightness K",
        cadence_s: 900,
        schema: "observatory/events@1",
        key_env: Some("SPARQ_FIRMS_KEY"),
    },
    StreamMeta {
        id: "wx.openmeteo",
        domain: "WX",
        label: "Weather — selected location",
        view: "timeseries",
        units: "°C",
        cadence_s: 300,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "wx.air",
        domain: "WX",
        label: "Air quality — PM2.5 / US AQI",
        view: "gauge",
        units: "US AQI",
        cadence_s: 900,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "wx.tides",
        domain: "WX",
        label: "Water level — NOAA CO-OPS station",
        view: "timeseries",
        units: "m MLLW",
        cadence_s: 900,
        schema: "observatory/timeseries@1",
        key_env: None,
    },
    StreamMeta {
        id: "wx.alerts",
        domain: "WX",
        label: "US weather alerts (NWS)",
        view: "map-points",
        units: "severity",
        cadence_s: 300,
        schema: "observatory/events@1",
        key_env: None,
    },
    StreamMeta {
        id: "space.neo",
        domain: "SP",
        label: "NASA NEO — close approaches",
        view: "timeline",
        units: "miss distance km",
        cadence_s: 1800,
        schema: "observatory/events@1",
        key_env: Some("SPARQ_NASA_API_KEY"),
    },
    StreamMeta {
        id: "space.epic",
        domain: "SP",
        label: "NASA EPIC — Earth imagery metadata",
        view: "text",
        units: "",
        cadence_s: 3600,
        schema: "observatory/text@1",
        key_env: Some("SPARQ_NASA_API_KEY"),
    },
    StreamMeta {
        id: "space.power",
        domain: "SP",
        label: "NASA POWER — daily met/solar point",
        view: "timeseries",
        units: "°C, kWh/m²",
        cadence_s: 3600,
        schema: "observatory/timeseries@1",
        key_env: Some("SPARQ_NASA_API_KEY"),
    },
];

/// The stream a cell enum's option index selects (`None` for OFF / out of range).
#[must_use]
pub fn stream_by_option(option: usize) -> Option<&'static StreamMeta> {
    if (1..=STREAM_COUNT).contains(&option) {
        Some(&STREAMS[option - 1])
    } else {
        None
    }
}

/// The option index of a stream id (1-based; `None` when the id is not in the registry).
#[must_use]
pub fn option_of_stream(id: &str) -> Option<usize> {
    STREAMS.iter().position(|s| s.id == id).map(|p| p + 1)
}

/// Looks a stream up by id (`None` when the id is not in the registry).
#[must_use]
pub fn by_id(id: &str) -> Option<&'static StreamMeta> {
    STREAMS.iter().find(|s| s.id == id)
}

impl StreamMeta {
    /// Splits `schema` (e.g. `observatory/grid@1`) into `(id, version)`. A malformed `@version`
    /// reads as version 1 (the observatory schemas' only version) rather than failing — the
    /// registry's own parse (`sparq_streams::schema::parse_schema`) already refused a bad
    /// version at load, so a generated table cannot carry one; this stays total anyway.
    #[must_use]
    pub fn schema_parts(&self) -> (&'static str, u32) {
        match self.schema.split_once('@') {
            Some((id, ver)) => (id, ver.parse::<u32>().unwrap_or(1)),
            None => (self.schema, 1),
        }
    }

    /// Whether this stream needs an API key (D14) — the cell shows KEY NEEDED in words when the
    /// host reports the key unset.
    #[must_use]
    pub fn requires_key(&self) -> bool {
        self.key_env.is_some()
    }
}
