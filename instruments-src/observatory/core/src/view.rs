//! Window→view transforms (plan D13): a stream's record window + its registry metadata → the typed
//! [`CellView`] a renderer draws.
//!
//! This is the layer that reads the frozen `data-record`s (via [`crate::record`]'s schema accessors)
//! and shapes them into what each of the seven renderers (§5.5) needs, plus the [`CellChrome`] every
//! cell shows regardless of renderer (title, description, the LIVE/STALE/OFFLINE/KEY-NEEDED status,
//! the footer's UTC stamp · age · headline). Keeping it separate from [`crate::render`] means the
//! renderers are pure geometry (view → IR) and this module is pure data (records → view), so each is
//! testable alone and the golden-IR tests exercise both through one seam.
//!
//! # Clocks (D8/D11 — the determinism firewall)
//!
//! The guest has NO wall clock. Every time this module needs — the x-window's right edge, the footer
//! stamp, the age — comes from the RECORDS themselves (`t_utc`, `age_s` channels, host-stamped). The
//! host's `frame-context.time-sec` is used ONLY for animation phase (ticker scroll), never to
//! position data. So the same window draws identically today and tomorrow: a golden is bit-exact.

use crate::record::{
    self, DataFlags, DataRecord, EVENTS_ID, GRID_ID, POSITION_ID, TEXT_ID, TIMESERIES_ID,
};
use crate::streams_table::StreamMeta;

/// The freshness/key state of a cell, decided HOST-side (the broker's window policy + the registry's
/// key state) and passed in. The core never computes it (that would need a clock and an env read —
/// both host-side). The LED word and the body's refuse-in-words both read this.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellStatus {
    /// A fresh record within 3 × cadence.
    Live,
    /// No fresh record for 3–10 × cadence (records still draw, flagged STALE).
    Stale,
    /// No fresh record for ≥ 10 × cadence, or never fetched.
    Offline,
    /// The stream needs an API key that is not set (plan D14) — never fetched, says so in words.
    /// Carries the env var name so the cell can name the fix.
    KeyNeeded(&'static str),
}

impl CellStatus {
    /// The LED word (§5.3). Always a word, never a bare colour — the refuse-in-words rule reaches
    /// the LED too.
    #[must_use]
    pub fn as_words(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::Stale => "STALE",
            Self::Offline => "OFFLINE",
            Self::KeyNeeded(_) => "KEY NEEDED",
        }
    }

    /// Whether the cell has data to draw (Live/Stale draw; Offline/KeyNeeded show WORDS).
    #[must_use]
    pub fn has_data(self) -> bool {
        matches!(self, Self::Live | Self::Stale)
    }
}

/// One timeseries sample: `(t_utc, value)`.
pub type SeriesPoint = (i64, f64);

/// The timeseries-schema view (drawn by the timeseries, bars+gauge and gauge renderers per the
/// registry's `view` hint).
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesView {
    /// Samples in window order (oldest → newest, the `Window::iter` convention).
    pub points: Vec<SeriesPoint>,
    /// The newest value, if any.
    pub latest: Option<f64>,
    /// Whether the series is signed (crosses zero — e.g. Bz), which the renderer draws bipolar.
    pub bipolar: bool,
}

/// One event, for the map-points and timeline renderers.
#[derive(Clone, Debug, PartialEq)]
pub struct EventPt {
    /// Event time, unix seconds.
    pub t_utc: i64,
    /// Latitude, degrees (NaN = no position — a flare, a close approach).
    pub lat: f64,
    /// Longitude, degrees (NaN = no position).
    pub lon: f64,
    /// Magnitude (stream scale).
    pub magnitude: f64,
    /// Depth, km.
    pub depth_km: f64,
    /// Severity tier index (see `crate::record::SEVERITY_*`).
    pub severity: u32,
    /// Record age, seconds.
    pub age_s: f64,
    /// The headline (a place, a flare class, a title).
    pub headline: String,
}

/// The events-schema view (drawn by the map-points and timeline renderers).
#[derive(Clone, Debug, PartialEq)]
pub struct EventsView {
    /// The events, in window order.
    pub events: Vec<EventPt>,
}

/// The grid-schema view (drawn by the map-heat renderer).
#[derive(Clone, Debug, PartialEq)]
pub struct HeatView {
    /// Grid columns.
    pub cols: i32,
    /// Grid rows.
    pub rows: i32,
    /// Origin latitude (cell 0,0 centre), degrees.
    pub lat0: f64,
    /// Origin longitude, degrees.
    pub lon0: f64,
    /// Cell size, degrees.
    pub step_deg: f64,
    /// Row-major values, `rows * cols`; NaN = no cell. Already gain-scaled by [`crate::params::Params::intensity`]?
    /// No — the raw values; the renderer applies intensity (so the same view serves any gain).
    pub values: Vec<f32>,
    /// The max finite value (for the colourbar's range label); 0 if none.
    pub max: f32,
}

/// One position sample (a moving object's trail point).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PosPt {
    /// Latitude, degrees.
    pub lat: f64,
    /// Longitude, degrees.
    pub lon: f64,
    /// Altitude, km.
    pub alt_km: f64,
    /// Velocity, km/s.
    pub vel_kms: f64,
    /// Footprint radius, km.
    pub footprint_km: f64,
    /// Visibility tier index.
    pub visibility: u32,
    /// Position time, unix seconds.
    pub t_utc: i64,
}

/// The position-schema view (drawn by the map-marker renderer — the ISS case).
#[derive(Clone, Debug, PartialEq)]
pub struct PositionView {
    /// The trail, oldest → newest (the window is a timeseries-kind trail, `Window::from_schema`).
    pub trail: Vec<PosPt>,
    /// The newest position (the marker), if any.
    pub latest: Option<PosPt>,
}

/// The text-schema view (drawn by the text renderer — the raw feed, verbatim).
#[derive(Clone, Debug, PartialEq)]
pub struct TextView {
    /// The newest text record's body, split into lines (the RAW payload, plan D8).
    pub lines: Vec<String>,
    /// The issue time, unix seconds.
    pub issued_utc: i64,
}

/// The renderer-facing body of a cell — one arm per schema shape. The registry's `view` hint picks
/// WHICH renderer draws a given shape (e.g. a `SeriesView` is a timeseries, a bars+gauge or a gauge
/// depending on the stream), so the arm is the data and the hint is the presentation.
#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    /// The cell is OFF (option 0, or an out-of-range selection).
    Off,
    /// No data to draw (Offline/KeyNeeded, or an empty window) — the renderer shows WORDS.
    NoData,
    /// A timeseries-schema window.
    Series(SeriesView),
    /// An events-schema window.
    Events(EventsView),
    /// A grid-schema window.
    Heat(HeatView),
    /// A position-schema window.
    Position(PositionView),
    /// A text-schema window.
    Text(TextView),
    /// The window's schema is not one of the five the core knows — refused in words, never misread.
    UnknownSchema(String),
}

/// The chrome every cell shows regardless of renderer (§5.3): the two-line header idiom, the status
/// LED, and the footer's stamp · age · headline.
#[derive(Clone, Debug, PartialEq)]
pub struct CellChrome {
    /// The cell title — the stream's label (§5.3, type.scale.m, text.primary).
    pub title: String,
    /// The right-aligned description — domain · cadence (§5.3, scale.s, text.secondary).
    pub description: String,
    /// The status LED word + colour choice.
    pub status: CellStatus,
    /// The unit string the axes/footer carry (token rule 9: numbers WITH units). May be empty.
    pub unit: String,
    /// The newest record's UTC time, unix seconds (the footer stamp), if any.
    pub stamp_utc: Option<i64>,
    /// The newest record's age, seconds (host-computed, D8), if any.
    pub age_s: Option<f64>,
    /// The one-line footer headline (a place, a value-with-unit, a raw first line), if any.
    pub headline: Option<String>,
}

/// A fully-built cell: chrome + body. This is what a renderer + the chrome painter consume.
#[derive(Clone, Debug, PartialEq)]
pub struct CellView {
    /// The stream id this cell shows, or `None` for OFF.
    pub stream_id: Option<&'static str>,
    /// The chrome (header/status/footer).
    pub chrome: CellChrome,
    /// The renderer-facing body.
    pub body: Body,
}

/// The newest record in a window (windows read oldest → newest, so the last is newest).
fn newest(records: &[DataRecord]) -> Option<&DataRecord> {
    records.iter().last()
}

/// Whether a timeseries stream is signed (bipolar). The registry does not carry a "bipolar" flag, so
/// this is the honest guest-side rule: a stream whose recent values span both signs is drawn
/// bipolar. Bz is the canonical case (§5.5); the rule generalises without a per-stream hard-code.
fn is_bipolar(points: &[SeriesPoint]) -> bool {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &(_, v) in points {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    lo < 0.0 && hi > 0.0
}

/// Builds the [`CellChrome`] from the metadata, the status and the newest record. The headline is
/// schema-appropriate: an event's place, a series' value-with-unit, a text's first line, a position's
/// alt·vel, a grid's peak.
fn build_chrome(meta: &StreamMeta, status: CellStatus, records: &[DataRecord]) -> CellChrome {
    let title = meta.label.to_string();
    let description = format!("{} · {} s", meta.domain, meta.cadence_s);
    let unit = meta.units.to_string();
    let new = newest(records);
    let (stamp_utc, age_s) = match new {
        Some(r) => (stamp_of(r), age_of(r)),
        None => (None, None),
    };
    let headline = new.and_then(|r| headline_for(r, &unit));
    CellChrome { title, description, status, unit, stamp_utc, age_s, headline }
}

/// A record's own UTC time (the schema's `t_utc`/`issued_utc` channel), for the footer stamp. Never
/// the host's `t_wall_ns` (D8: the guest renders the record's own time as absolute UTC).
fn stamp_of(r: &DataRecord) -> Option<i64> {
    match r.schema_id.as_str() {
        TIMESERIES_ID => record::ts_channels(r).map(|(t, _, _)| t),
        EVENTS_ID => record::event_channels(r).map(|e| e.t_utc),
        GRID_ID => record::grid_channels(r).map(|g| g.t_utc),
        POSITION_ID => record::position_channels(r).map(|p| p.t_utc),
        TEXT_ID => record::text_channels(r).map(|(_, issued, _)| issued),
        _ => None,
    }
}

/// A record's host-computed age, seconds (the `age_s` channel — the host may read its clock, the
/// guest may not, D8).
fn age_of(r: &DataRecord) -> Option<f64> {
    match r.schema_id.as_str() {
        TIMESERIES_ID => record::ts_channels(r).map(|(_, _, a)| a),
        EVENTS_ID => record::event_channels(r).map(|e| e.age_s),
        GRID_ID => record::grid_channels(r).map(|g| g.age_s),
        POSITION_ID => record::position_channels(r).map(|p| p.age_s),
        TEXT_ID => record::text_channels(r).map(|(_, _, a)| a),
        _ => None,
    }
}

/// The footer headline for a record (§5.3's one-liner), schema-appropriate, WITH units (rule 9).
fn headline_for(r: &DataRecord, unit: &str) -> Option<String> {
    match r.schema_id.as_str() {
        EVENTS_ID => record::event_headline(r).map(str::to_string),
        TIMESERIES_ID => record::ts_channels(r).map(|(_, v, _)| format_with_unit(v, unit)),
        POSITION_ID => record::position_channels(r)
            .map(|p| format!("{:.0} km · {:.2} km/s", p.alt_km, p.vel_kms)),
        GRID_ID => record::grid_channels(r).map(|g| {
            let max =
                g.values.iter().filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            if max.is_finite() {
                format!("peak {}", format_with_unit(max, unit))
            } else {
                "no cells".to_string()
            }
        }),
        TEXT_ID => record::text_channels(r).map(|(body, _, _)| {
            body.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string()
        }),
        _ => None,
    }
}

/// Formats a number WITH its unit (token rule 9), trimming to a readable precision. An empty unit
/// (raw text, category counts) yields the bare number.
#[must_use]
pub fn format_with_unit(v: f64, unit: &str) -> String {
    if !v.is_finite() {
        return "—".to_string();
    }
    let a = v.abs();
    // 3 significant-ish digits: no decimals at >=100, one at >=1, two below (rule 9: readable).
    let s = if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 1.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.2}")
    };
    if unit.is_empty() {
        s
    } else {
        format!("{s} {unit}")
    }
}

/// Builds a cell's typed body from its schema-shaped records. `meta.schema` selects the accessor; a
/// schema the core does not know yields [`Body::UnknownSchema`] (refused in words, never misread).
fn build_body(
    meta: &StreamMeta,
    status: CellStatus,
    records: &[DataRecord],
    selected_stream: Option<&str>,
) -> Body {
    // OFF / no-selection first: nothing to read.
    let Some(_sid) = selected_stream else {
        return Body::Off;
    };
    // A stream that is not fetched (key needed) or has no window yet shows WORDS, not an empty chart.
    if !status.has_data() || records.is_empty() {
        return Body::NoData;
    }
    let (schema_id, _) = meta.schema_parts();
    match schema_id {
        TIMESERIES_ID => {
            let points: Vec<SeriesPoint> =
                records.iter().filter_map(record::ts_channels).map(|(t, v, _)| (t, v)).collect();
            if points.is_empty() {
                return Body::NoData;
            }
            let latest = points.last().map(|&(_, v)| v);
            let bipolar = is_bipolar(&points);
            Body::Series(SeriesView { points, latest, bipolar })
        },
        EVENTS_ID => {
            let events = records
                .iter()
                .filter_map(|r| {
                    let e = record::event_channels(r)?;
                    Some(EventPt {
                        t_utc: e.t_utc,
                        lat: e.lat,
                        lon: e.lon,
                        magnitude: e.magnitude,
                        depth_km: e.depth_km,
                        severity: e.severity,
                        age_s: e.age_s,
                        headline: record::event_headline(r).unwrap_or("").to_string(),
                    })
                })
                .collect::<Vec<_>>();
            if events.is_empty() {
                return Body::NoData;
            }
            Body::Events(EventsView { events })
        },
        GRID_ID => {
            // Grids window as the 2 newest frames; draw the latest.
            let Some(g) = newest(records).and_then(record::grid_channels) else {
                return Body::NoData;
            };
            let max =
                g.values.iter().filter(|v| v.is_finite()).fold(f64::NEG_INFINITY, |a, &b| a.max(b));
            let max = if max.is_finite() { max as f32 } else { 0.0 };
            Body::Heat(HeatView {
                cols: g.cols,
                rows: g.rows,
                lat0: g.lat0,
                lon0: g.lon0,
                step_deg: g.step_deg,
                values: g.values.iter().map(|&v| v as f32).collect(),
                max,
            })
        },
        POSITION_ID => {
            let trail = records
                .iter()
                .filter_map(record::position_channels)
                .map(|p| PosPt {
                    lat: p.lat,
                    lon: p.lon,
                    alt_km: p.alt_km,
                    vel_kms: p.vel_kms,
                    footprint_km: p.footprint_km,
                    visibility: p.visibility,
                    t_utc: p.t_utc,
                })
                .collect::<Vec<_>>();
            let latest = trail.last().copied();
            if trail.is_empty() {
                return Body::NoData;
            }
            Body::Position(PositionView { trail, latest })
        },
        TEXT_ID => {
            let Some((body, issued)) = newest(records).and_then(|r| {
                let (b, i, _) = record::text_channels(r)?;
                Some((b.to_string(), i))
            }) else {
                return Body::NoData;
            };
            let lines = body.lines().map(str::to_string).collect::<Vec<_>>();
            Body::Text(TextView { lines, issued_utc: issued })
        },
        other => Body::UnknownSchema(other.to_string()),
    }
}

/// Builds the full [`CellView`] for one cell: the stream it shows (from the resolved param), the
/// chrome, and the typed body. `selected_stream` is the guest-side resolution of the cell's enum
/// param (`None` = OFF); `records` is the window the host served for that cell's source binding.
#[must_use]
pub fn build_view(
    meta: Option<&StreamMeta>,
    status: CellStatus,
    records: &[DataRecord],
    selected_stream: Option<&'static str>,
) -> CellView {
    // No stream selected (OFF) — a minimal chrome with the OFF body.
    let Some(meta) = meta else {
        return CellView {
            stream_id: None,
            chrome: CellChrome {
                title: crate::streams_table::OFF_LABEL.to_string(),
                description: String::new(),
                status,
                unit: String::new(),
                stamp_utc: None,
                age_s: None,
                headline: None,
            },
            body: Body::Off,
        };
    };
    let chrome = build_chrome(meta, status, records);
    // A stale record set still draws (flagged); the status is carried for the LED.
    let body = build_body(meta, status, records, selected_stream);
    CellView { stream_id: Some(meta.id), chrome, body }
}

/// Whether any record in a window carries the [`DataFlags::Stale`] flag (so a renderer can dash or
/// dim a stale series without re-deriving freshness — the flag and the LED word are the same
/// statement in two vocabularies, §6.1).
#[must_use]
pub fn window_is_stale(records: &[DataRecord]) -> bool {
    records.iter().any(|r| r.record_flags == DataFlags::Stale)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::record::{DataValue, SEVERITY_HIGH};

    fn meta(id: &str, schema: &str, view: &str) -> StreamMeta {
        StreamMeta {
            id: leak(id),
            domain: "GEO",
            label: leak(id),
            view: leak(view),
            units: "",
            cadence_s: 60,
            schema: leak(schema),
            key_env: None,
        }
    }
    fn leak(s: &str) -> &'static str {
        Box::leak(s.to_string().into_boxed_str())
    }

    fn ts_rec(t: i64, v: f64) -> DataRecord {
        DataRecord {
            schema_id: TIMESERIES_ID.into(),
            schema_version: 1,
            channels: vec![DataValue::Int64(t), DataValue::Double(v), DataValue::Double(1.0)],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: DataFlags::Ok,
        }
    }

    #[test]
    fn off_selection_builds_an_off_body() {
        let m = meta("swpc.kp", "observatory/timeseries@1", "bars+gauge");
        let v = build_view(Some(&m), CellStatus::Live, &[], None);
        assert_eq!(v.body, Body::Off);
    }

    #[test]
    fn key_needed_shows_words_not_an_empty_chart() {
        let m = meta("geo.firms", "observatory/events@1", "map-points");
        let v =
            build_view(Some(&m), CellStatus::KeyNeeded("SPARQ_FIRMS_KEY"), &[], Some("geo.firms"));
        assert_eq!(v.body, Body::NoData);
        assert_eq!(v.chrome.status.as_words(), "KEY NEEDED");
    }

    #[test]
    fn a_series_view_detects_bipolar() {
        let m = meta("swpc.bz", "observatory/timeseries@1", "timeseries");
        let recs = vec![ts_rec(100, -4.1), ts_rec(160, 2.0)];
        let v = build_view(Some(&m), CellStatus::Live, &recs, Some("swpc.bz"));
        match &v.body {
            Body::Series(s) => {
                assert!(s.bipolar, "Bz spans both signs → bipolar");
                assert_eq!(s.latest, Some(2.0));
                assert_eq!(s.points.len(), 2);
            },
            other => panic!("expected Series, got {other:?}"),
        }
    }

    #[test]
    fn chrome_stamp_and_age_come_from_the_record_not_a_clock() {
        let m = meta("swpc.kp", "observatory/timeseries@1", "bars+gauge");
        let recs = vec![ts_rec(1_700_000_000, 5.0)];
        let v = build_view(Some(&m), CellStatus::Live, &recs, Some("swpc.kp"));
        assert_eq!(v.chrome.stamp_utc, Some(1_700_000_000));
        assert_eq!(v.chrome.age_s, Some(1.0));
        assert_eq!(v.chrome.headline.as_deref(), Some("5.0"));
    }

    #[test]
    fn an_unknown_schema_is_refused_in_words() {
        let m = meta("x.y", "sparq/imu9@1", "timeseries");
        let recs = vec![ts_rec(1, 2.0)];
        let v = build_view(Some(&m), CellStatus::Live, &recs, Some("x.y"));
        assert!(matches!(v.body, Body::UnknownSchema(ref s) if s == "sparq/imu9"));
    }

    #[test]
    fn format_with_unit_always_carries_the_unit() {
        assert_eq!(format_with_unit(282.0, "km/s"), "282 km/s");
        assert_eq!(format_with_unit(-4.1, "nT"), "-4.1 nT");
        assert_eq!(format_with_unit(5.0, ""), "5.0");
        assert_eq!(format_with_unit(f64::NAN, "Kp"), "—", "a non-finite never prints as a number");
    }

    #[test]
    fn an_event_body_carries_severity_and_headline() {
        let m = meta("geo.quakes-hour", "observatory/events@1", "map-points");
        let r = DataRecord {
            schema_id: EVENTS_ID.into(),
            schema_version: 1,
            channels: vec![
                DataValue::Int64(500),
                DataValue::Double(35.0),
                DataValue::Double(-118.0),
                DataValue::Double(4.3),
                DataValue::Double(12.0),
                DataValue::Enumerated(SEVERITY_HIGH),
                DataValue::Text("M 4.3 — 12 km S of Idria, CA".into()),
                DataValue::Double(3.0),
            ],
            t_wall_ns: 0,
            t_sample: None,
            seq: 0,
            record_flags: DataFlags::Ok,
        };
        let v = build_view(Some(&m), CellStatus::Live, &[r], Some("geo.quakes-hour"));
        match &v.body {
            Body::Events(e) => {
                assert_eq!(e.events.len(), 1);
                assert_eq!(e.events[0].severity, SEVERITY_HIGH);
                assert!(e.events[0].headline.contains("Idria"));
            },
            other => panic!("expected Events, got {other:?}"),
        }
        assert_eq!(v.chrome.headline.as_deref(), Some("M 4.3 — 12 km S of Idria, CA"));
    }
}
