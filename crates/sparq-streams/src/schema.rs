//! The observatory's five private-namespace record schemas (plan §6.1), as typed builders.
//!
//! Every stream normalizes into exactly one of these five shapes, and every shape is a fixed
//! channel order over the frozen [`crate::record::DataValue`] carriers. Declaring them here — as
//! data (the id + version + channel list) *and* as constructors — is what makes the normalizers
//! thin and the renderers (INC3) able to read a window without guessing: a consumer that knows
//! `schema_id`@`schema_version` knows the channel order, and an unknown schema is refused, never
//! misread (the contract's own rule).
//!
//! These are **private-namespace** schemas (`observatory/…`), the schema doc's §4 mechanism for a
//! first-party instrument's own shapes; they are not host-registered standards (`sparq/…`). The
//! `@1` version is the schema's own, independent of the contract's `host_api` pin.
//!
//! # Severity and visibility are enums, not raw values
//!
//! The events schema's `severity` and the position schema's `visibility` ride the `enumerated`
//! carrier — an index into the options declared *here*, never a raw number. Each normalizer maps
//! its stream's native severity (a USGS magnitude, a GOES flare class, a GDACS alert level, an
//! EONET category) onto the shared [`SEVERITY_UNKNOWN`]…[`SEVERITY_SEVERE`] tier scale, and the
//! stream-specific detail ("M 5.1", "X1.2", "Orange", "Wildfires") rides the `headline` text
//! channel. That is the non-colour-encoding discipline (token rule 7): the tier is machine
//! orderable, the headline is human readable, and neither depends on a hue.

use crate::record::{DataRecord, DataValue};

/// Schema version shared by all five observatory schemas (they were born together at WO-020 v1).
pub const VERSION: u32 = 1;

/// `observatory/timeseries@1` — a scalar sampled over time.
pub const TIMESERIES_ID: &str = "observatory/timeseries";
/// `observatory/events@1` — a discrete event at a time and (usually) a place.
pub const EVENTS_ID: &str = "observatory/events";
/// `observatory/grid@1` — a regular lat/lon grid of values (heat cells).
pub const GRID_ID: &str = "observatory/grid";
/// `observatory/position@1` — a moving object's state at a time.
pub const POSITION_ID: &str = "observatory/position";
/// `observatory/text@1` — a raw text payload with an issue time.
pub const TEXT_ID: &str = "observatory/text";

// --- the shared severity tier scale (events schema, `severity` channel) -------------------------

/// Severity tier 0: unknown or unclassified.
pub const SEVERITY_UNKNOWN: u32 = 0;
/// Severity tier 1: low / informational (GDACS Green, GOES B-class, USGS M < 2.5).
pub const SEVERITY_LOW: u32 = 1;
/// Severity tier 2: moderate (GDACS Orange, GOES C-class, USGS M 2.5…4.4, NWS "advisory/watch").
pub const SEVERITY_MODERATE: u32 = 2;
/// Severity tier 3: high (GDACS Red, GOES M-class, USGS M 4.5…5.9, NWS "warning").
pub const SEVERITY_HIGH: u32 = 3;
/// Severity tier 4: severe/extreme (GOES X-class, USGS M ≥ 6, NWS "emergency"/"extreme").
pub const SEVERITY_SEVERE: u32 = 4;

/// The severity tier vocabulary, in index order — the `enumerated` options list for the events
/// schema. A renderer or validator that needs the option labels reads this, never hard-codes them.
pub const SEVERITY_OPTIONS: [&str; 5] = ["unknown", "low", "moderate", "high", "severe"];

// --- the shared visibility scale (position schema, `visibility` channel) ------------------------

/// Visibility 0: unknown.
pub const VISIBILITY_UNKNOWN: u32 = 0;
/// Visibility 1: in daylight (sunlit).
pub const VISIBILITY_DAYLIGHT: u32 = 1;
/// Visibility 2: eclipsed (in Earth's shadow).
pub const VISIBILITY_ECLIPSED: u32 = 2;
/// Visibility 3: night (over the dark side, not eclipsed).
pub const VISIBILITY_NIGHT: u32 = 3;

/// The visibility vocabulary, in index order — the `enumerated` options for the position schema.
pub const VISIBILITY_OPTIONS: [&str; 4] = ["unknown", "daylight", "eclipsed", "night"];

/// Maps a wheretheiss.at `visibility` string ("daylight" / "eclipsed" / "night") to its tier index.
/// Anything unrecognised is [`VISIBILITY_UNKNOWN`] — an unknown word is never guessed at.
#[must_use]
pub fn visibility_from_str(s: &str) -> u32 {
    match s.trim().to_ascii_lowercase().as_str() {
        "daylight" => VISIBILITY_DAYLIGHT,
        "eclipsed" => VISIBILITY_ECLIPSED,
        "night" => VISIBILITY_NIGHT,
        _ => VISIBILITY_UNKNOWN,
    }
}

/// Splits a `"observatory/timeseries@1"` schema reference into its id and version. A reference with
/// no `@` is version [`VERSION`] by default; a non-numeric version is refused (`None`) rather than
/// read as 0, because a schema whose version we cannot parse is a schema we cannot honour.
#[must_use]
pub fn parse_schema(reference: &str) -> Option<(&str, u32)> {
    match reference.split_once('@') {
        Some((id, ver)) => ver.parse::<u32>().ok().map(|v| (id, v)),
        None => Some((reference, VERSION)),
    }
}

/// True when `schema_id` is one of the five observatory schemas. The normalizers and the validator
/// (INC2) both ask this before trusting a channel order.
#[must_use]
pub fn is_observatory_schema(schema_id: &str) -> bool {
    matches!(schema_id, TIMESERIES_ID | EVENTS_ID | GRID_ID | POSITION_ID | TEXT_ID)
}

// --- typed builders -----------------------------------------------------------------------------
//
// Each builder stamps the record's advisory wall clock from its own `t_utc` (unix seconds) so the
// record is self-describing before the host re-stamps it, and computes nothing else: `age_s` is
// passed in, because the broker — not the guest — owns the clock (plan D8). A builder never reads
// `SystemTime`; determinism is the whole point (the plan's D11 firewall).

/// Builds an `observatory/timeseries@1` record: `{t_utc: i64, value: f64, age_s: f64}`.
#[must_use]
pub fn timeseries(t_utc: i64, value: f64, age_s: f64) -> DataRecord {
    DataRecord::new(
        TIMESERIES_ID,
        VERSION,
        vec![DataValue::Int64(t_utc), DataValue::Double(value), DataValue::Double(age_s)],
    )
    .stamped(t_utc)
}

/// Builds an `observatory/events@1` record:
/// `{t_utc: i64, lat: f64, lon: f64, magnitude: f64, depth_km: f64, severity: enum, headline: str,
/// age_s: f64}`. A non-geolocated event (a flare, a close approach) passes `f64::NAN` for lat/lon —
/// the map renderer treats NaN as "no position", the contract's own no-cell sentinel.
// The eight arguments ARE the schema's eight channels, in frozen order; collapsing them into a
// struct would hide the 1:1 mapping to the channel list, which is this builder's whole job.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn event(
    t_utc: i64,
    lat: f64,
    lon: f64,
    magnitude: f64,
    depth_km: f64,
    severity: u32,
    headline: &str,
    age_s: f64,
) -> DataRecord {
    DataRecord::new(
        EVENTS_ID,
        VERSION,
        vec![
            DataValue::Int64(t_utc),
            DataValue::Double(lat),
            DataValue::Double(lon),
            DataValue::Double(magnitude),
            DataValue::Double(depth_km),
            DataValue::Enumerated(severity),
            DataValue::Text(headline.to_string()),
            DataValue::Double(age_s),
        ],
    )
    .stamped(t_utc)
}

/// Builds an `observatory/grid@1` record:
/// `{cols: i32, rows: i32, lat0: f64, lon0: f64, step_deg: f64, values: vecN, t_utc: i64,
/// age_s: f64}`. `values` is row-major, `rows × cols`, on the f64 carrier; a missing cell is
/// `f64::NAN` (the contract's no-cell sentinel — NaN, never a zero that reads as data).
// Eight arguments = the schema's eight channels, in frozen order (see [`event`]).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn grid(
    cols: i32,
    rows: i32,
    lat0: f64,
    lon0: f64,
    step_deg: f64,
    values: Vec<f64>,
    t_utc: i64,
    age_s: f64,
) -> DataRecord {
    DataRecord::new(
        GRID_ID,
        VERSION,
        vec![
            DataValue::Int32(cols),
            DataValue::Int32(rows),
            DataValue::Double(lat0),
            DataValue::Double(lon0),
            DataValue::Double(step_deg),
            DataValue::Vec(values),
            DataValue::Int64(t_utc),
            DataValue::Double(age_s),
        ],
    )
    .stamped(t_utc)
}

/// Builds an `observatory/position@1` record:
/// `{lat: f64, lon: f64, alt_km: f64, vel_kms: f64, footprint_km: f64, visibility: enum,
/// t_utc: i64, age_s: f64}`.
// Eight arguments = the schema's eight channels, in frozen order (see [`event`]).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn position(
    lat: f64,
    lon: f64,
    alt_km: f64,
    vel_kms: f64,
    footprint_km: f64,
    visibility: u32,
    t_utc: i64,
    age_s: f64,
) -> DataRecord {
    DataRecord::new(
        POSITION_ID,
        VERSION,
        vec![
            DataValue::Double(lat),
            DataValue::Double(lon),
            DataValue::Double(alt_km),
            DataValue::Double(vel_kms),
            DataValue::Double(footprint_km),
            DataValue::Enumerated(visibility),
            DataValue::Int64(t_utc),
            DataValue::Double(age_s),
        ],
    )
    .stamped(t_utc)
}

/// Builds an `observatory/text@1` record: `{text: str, issued_utc: i64, age_s: f64}`. The `text`
/// channel is the **raw payload, verbatim** (a WWV bulletin, a forecast block, a TLE table) — the
/// ticker shows it as words because the operator asked for the raw stream literally (plan D8).
#[must_use]
pub fn text(body: &str, issued_utc: i64, age_s: f64) -> DataRecord {
    DataRecord::new(
        TEXT_ID,
        VERSION,
        vec![
            DataValue::Text(body.to_string()),
            DataValue::Int64(issued_utc),
            DataValue::Double(age_s),
        ],
    )
    .stamped(issued_utc)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::record::DataValue;

    #[test]
    fn timeseries_channel_order_is_frozen() {
        let r = timeseries(1_700_000_000, 282.0, 12.5);
        assert_eq!(r.schema_id, TIMESERIES_ID);
        assert_eq!(r.schema_version, VERSION);
        assert_eq!(r.channels.len(), 3);
        assert_eq!(r.channel(0), Some(&DataValue::Int64(1_700_000_000)));
        assert_eq!(r.channel(1), Some(&DataValue::Double(282.0)));
        assert_eq!(r.channel(2), Some(&DataValue::Double(12.5)));
        assert_eq!(r.t_wall_ns, 1_700_000_000_000_000_000);
    }

    #[test]
    fn event_carries_the_severity_tier_and_a_headline() {
        let r = event(
            100,
            35.0,
            -118.0,
            4.3,
            12.0,
            SEVERITY_MODERATE,
            "M 4.3 — 12 km S of Idria, CA",
            3.0,
        );
        assert_eq!(r.channels.len(), 8);
        assert_eq!(r.channel(5), Some(&DataValue::Enumerated(SEVERITY_MODERATE)));
        assert_eq!(r.channel(6).and_then(DataValue::as_str), Some("M 4.3 — 12 km S of Idria, CA"));
    }

    #[test]
    fn grid_values_ride_the_f64_vec_carrier() {
        let r = grid(2, 2, 90.0, -180.0, 2.0, vec![1.0, 2.0, f64::NAN, 4.0], 500, 1.0);
        match r.channel(5) {
            Some(DataValue::Vec(v)) => {
                assert_eq!(v.len(), 4);
                assert!(v[2].is_nan(), "a missing cell is NaN, never a zero that reads as data");
            },
            other => panic!("grid values channel must be a Vec, got {other:?}"),
        }
    }

    #[test]
    fn position_and_text_builders_are_well_formed() {
        let p = position(1.0, 2.0, 408.0, 7.66, 2200.0, VISIBILITY_DAYLIGHT, 900, 5.0);
        assert_eq!(p.channels.len(), 8);
        assert_eq!(p.channel(5), Some(&DataValue::Enumerated(VISIBILITY_DAYLIGHT)));
        let t = text("Solar-terrestrial indices…", 800, 4.0);
        assert_eq!(t.channels.len(), 3);
        assert_eq!(t.schema_id, TEXT_ID);
    }

    #[test]
    fn schema_reference_parsing_honours_the_version() {
        assert_eq!(parse_schema("observatory/grid@1"), Some(("observatory/grid", 1)));
        assert_eq!(parse_schema("observatory/grid"), Some(("observatory/grid", VERSION)));
        assert_eq!(
            parse_schema("observatory/grid@x"),
            None,
            "an unparsable version is refused, not read as 0"
        );
        assert!(is_observatory_schema(EVENTS_ID));
        assert!(!is_observatory_schema("sparq/imu9"));
    }

    #[test]
    fn visibility_words_map_to_stable_indices() {
        assert_eq!(visibility_from_str("Daylight"), VISIBILITY_DAYLIGHT);
        assert_eq!(visibility_from_str("eclipsed"), VISIBILITY_ECLIPSED);
        assert_eq!(visibility_from_str("night"), VISIBILITY_NIGHT);
        assert_eq!(visibility_from_str("???"), VISIBILITY_UNKNOWN);
        assert_eq!(SEVERITY_OPTIONS.len(), 5);
        assert_eq!(VISIBILITY_OPTIONS.len(), 4);
    }
}
